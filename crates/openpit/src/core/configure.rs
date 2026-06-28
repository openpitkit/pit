// Copyright The Pit Project Owners. All rights reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
// Please see https://openpit.dev and the OWNERS file for details.

//! Runtime reconfiguration for built-in pre-trade policies.
//!
//! At build time the engine captures a clone of each supported built-in
//! policy's settings cell. The registry stores those cells as concrete enum
//! variants keyed by policy name. A [`ConfigCell`](crate::storage::ConfigCell)
//! clone shares the running policy's value, so updates are observed on the next
//! hot-path read without adding synchronization to order checks.
//!
//! Runtime reconfiguration of custom policies is not supported in this
//! release.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};

use super::sync_mode::SyncMode;
use crate::param::{AccountId, Asset, Pnl};
use crate::pretrade::policies::{
    OrderSizeLimitSettings, PnlBoundsKillSwitchSettings, RateLimitSettings, RealizedPnlStorage,
    SpotFundsSettings,
};
use crate::storage::{ConfigCell, LockingPolicyFactory};

// ─── ConfigEntry ────────────────────────────────────────────────────────────

/// Settings cells supported by the built-in runtime configurator.
pub(crate) enum ConfigEntry<Factory: LockingPolicyFactory> {
    /// Rate-limit policy settings.
    RateLimit(Factory::Config<RateLimitSettings>),
    /// P&L bounds kill-switch policy handles.
    ///
    /// Carries both the settings cell (bounds retune) and the shared realized
    /// P&L ledger (force-set of accumulated P&L), so the configurator can reach
    /// either without an extra registry lookup.
    PnlBoundsKillSwitch {
        /// Settings cell shared with the running policy.
        settings: Factory::Config<PnlBoundsKillSwitchSettings>,
        /// Live accumulated P&L ledger shared with the running policy.
        realized: RealizedPnlStorage<Factory>,
    },
    /// Spot-funds policy handles.
    SpotFunds {
        /// Settings cell shared with the running policy.
        settings: Factory::Config<SpotFundsSettings>,
        /// Live accumulated self-computed account-currency P&L ledger.
        pnl: RealizedPnlStorage<Factory>,
    },
    /// Order-size-limit policy settings.
    OrderSizeLimit(Factory::Config<OrderSizeLimitSettings>),
}

impl<Factory: LockingPolicyFactory> ConfigEntry<Factory> {
    fn settings_type_name(&self) -> &'static str {
        match self {
            Self::RateLimit(_) => std::any::type_name::<Factory::Config<RateLimitSettings>>(),
            Self::PnlBoundsKillSwitch { .. } => {
                std::any::type_name::<Factory::Config<PnlBoundsKillSwitchSettings>>()
            }
            Self::SpotFunds { .. } => std::any::type_name::<Factory::Config<SpotFundsSettings>>(),
            Self::OrderSizeLimit(_) => {
                std::any::type_name::<Factory::Config<OrderSizeLimitSettings>>()
            }
        }
    }
}

// ─── ConfigureError ──────────────────────────────────────────────────────────

/// Error returned by [`Configurator`] when a runtime reconfiguration fails.
///
/// Every variant leaves the live settings unchanged: an unknown or mismatched
/// policy is never touched, and a rejected update is rolled back before
/// publication (the [`ConfigCell`](crate::storage::ConfigCell) update is
/// transactional).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ConfigureError {
    /// No registered policy carries the requested name.
    UnknownPolicy {
        /// Requested policy name.
        name: String,
    },
    /// A policy is registered under `name`, but its settings type differs from
    /// the one the called method targets.
    PolicyTypeMismatch {
        /// Requested policy name.
        name: String,
        /// Settings cell type the method expected.
        expected: &'static str,
        /// Settings cell type actually registered.
        found: &'static str,
    },
    /// The update closure rejected the new value; the prior value still
    /// applies.
    Validation {
        /// Requested policy name.
        name: String,
        /// Rendered error returned by the closure.
        message: String,
    },
    /// A configuration call was issued from within another configuration
    /// callback on the same thread.
    ///
    /// Configuration is non-reentrant: a [`Configurator`] method runs its
    /// update closure while it owns the settings cell's writer lock, and that
    /// lock is not reentrant. Re-entering configuration for the same engine
    /// from inside such a closure - whether for the same policy or a different
    /// one - is rejected before any lock is taken, so policy authors do not
    /// have to reason about configuration lock ordering. Configuration from
    /// other threads is unaffected and still serializes. The live settings are
    /// left unchanged.
    NestedConfiguration,
}

impl Display for ConfigureError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownPolicy { name } => {
                write!(formatter, "no configurable policy named {name}")
            }
            Self::PolicyTypeMismatch {
                name,
                expected,
                found,
            } => write!(
                formatter,
                "policy {name} has settings type {found}, not {expected}"
            ),
            Self::Validation { name, message } => {
                write!(formatter, "policy {name} rejected the update: {message}")
            }
            Self::NestedConfiguration => write!(
                formatter,
                "configuration is not reentrant: cannot configure from within \
                 another configuration callback on the same thread"
            ),
        }
    }
}

impl std::error::Error for ConfigureError {}

// ─── ConfigRegistry ──────────────────────────────────────────────────────────

/// Per-engine map from policy name to its built-in settings cell.
pub(crate) struct ConfigRegistry<Factory: LockingPolicyFactory> {
    entries: HashMap<String, ConfigEntry<Factory>>,
}

impl<Factory: LockingPolicyFactory> ConfigRegistry<Factory> {
    /// Builds a registry from the builder's collected `(name, cell)` pairs.
    pub(crate) fn from_entries(entries: HashMap<String, ConfigEntry<Factory>>) -> Self {
        Self { entries }
    }

    fn entry(&self, name: &str) -> Result<&ConfigEntry<Factory>, ConfigureError> {
        self.entries
            .get(name)
            .ok_or_else(|| ConfigureError::UnknownPolicy {
                name: name.to_owned(),
            })
    }

    fn type_mismatch<Settings: Clone + 'static>(
        name: &str,
        entry: &ConfigEntry<Factory>,
    ) -> ConfigureError {
        ConfigureError::PolicyTypeMismatch {
            name: name.to_owned(),
            expected: std::any::type_name::<Factory::Config<Settings>>(),
            found: entry.settings_type_name(),
        }
    }

    fn validation<Error: std::fmt::Display>(name: &str, error: Error) -> ConfigureError {
        ConfigureError::Validation {
            name: name.to_owned(),
            message: error.to_string(),
        }
    }
}

// ─── Re-entrancy guard ───────────────────────────────────────────────────────

thread_local! {
    static CONFIGURING: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
}

/// RAII marker that a configuration is in progress on the current thread.
///
/// [`Self::enter`] fails with [`ConfigureError::NestedConfiguration`] when a
/// configuration for the same engine registry is already active on this
/// thread, so a nested call returns before it can take any cell writer lock.
/// The flag is cleared on drop - including on an early `?` return or a panic
/// unwinding through the closure - so a single thread can configure again once
/// the outer call completes.
struct ConfiguringGuard {
    identity: usize,
}

impl ConfiguringGuard {
    fn enter(identity: usize) -> Result<Self, ConfigureError> {
        CONFIGURING.with(|configuring| {
            let mut active = configuring.borrow_mut();
            if active.contains(&identity) {
                return Err(ConfigureError::NestedConfiguration);
            }
            active.push(identity);
            Ok(Self { identity })
        })
    }
}

impl Drop for ConfiguringGuard {
    fn drop(&mut self) {
        CONFIGURING.with(|configuring| {
            let mut active = configuring.borrow_mut();
            if let Some(pos) = active.iter().rposition(|id| *id == self.identity) {
                active.swap_remove(pos);
            }
        });
    }
}

// ─── Configurator ────────────────────────────────────────────────────────────

/// Storage locking factory backing a [`Configurator`] of a given [`SyncMode`].
type RegistryFactory<Sync> = <Sync as SyncMode>::StorageLockingPolicyFactory;

/// Engine handle for retuning built-in policies at runtime.
///
/// Obtained from [`Engine::configure`](crate::Engine::configure). Each method
/// targets one supported built-in policy settings type. Updates published here
/// are observed by the running engine on its next hot-path read because the
/// registry shares each policy's settings cell rather than holding a copy.
/// Custom-policy runtime reconfiguration is planned for a later release.
///
/// # Threading
///
/// The handle's auto-traits derive from the engine's [`SyncMode`], matching the
/// engine handle:
///
/// - [`FullSync`](crate::FullSync): `Send + Sync`.
/// - [`AccountSync`](crate::AccountSync): `Send + !Sync`.
/// - [`LocalSync`](crate::LocalSync): `!Send + !Sync`.
pub struct Configurator<Sync: SyncMode> {
    registry: <<Sync as SyncMode>::StorageLockingPolicyFactory as LockingPolicyFactory>::Shared<
        ConfigRegistry<RegistryFactory<Sync>>,
    >,
}

impl<Sync: SyncMode> Clone for Configurator<Sync> {
    fn clone(&self) -> Self {
        Self {
            registry: self.registry.clone(),
        }
    }
}

impl<Sync: SyncMode> Configurator<Sync> {
    /// Wraps the engine's shared registry in a configurator handle.
    pub(crate) fn from_inner(
        registry: <<Sync as SyncMode>::StorageLockingPolicyFactory as LockingPolicyFactory>::Shared<
            ConfigRegistry<RegistryFactory<Sync>>,
        >,
    ) -> Self {
        Self { registry }
    }

    fn enter_configuration(&self) -> Result<ConfiguringGuard, ConfigureError> {
        ConfiguringGuard::enter(std::ptr::from_ref(&*self.registry) as usize)
    }

    /// Retunes the [`RateLimitPolicy`](crate::pretrade::policies::RateLimitPolicy)
    /// registered under `name`.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigureError::UnknownPolicy`] for an unknown name,
    /// [`ConfigureError::PolicyTypeMismatch`] when the name belongs to another
    /// built-in policy type, [`ConfigureError::Validation`] when `f` rejects
    /// the update, or [`ConfigureError::NestedConfiguration`] when called from
    /// within another configuration callback on the same thread.
    pub fn rate_limit<Error: std::fmt::Display>(
        &self,
        name: &str,
        f: impl FnOnce(&mut RateLimitSettings) -> Result<(), Error>,
    ) -> Result<(), ConfigureError> {
        match self.registry.entry(name)? {
            ConfigEntry::RateLimit(cell) => {
                let _guard = self.enter_configuration()?;
                cell.update(f).map_err(|error| {
                    ConfigRegistry::<RegistryFactory<Sync>>::validation(name, error)
                })
            }
            entry => Err(ConfigRegistry::<_>::type_mismatch::<RateLimitSettings>(
                name, entry,
            )),
        }
    }

    /// Retunes the
    /// [`PnlBoundsKillSwitchPolicy`](crate::pretrade::policies::PnlBoundsKillSwitchPolicy)
    /// registered under `name`.
    ///
    /// # Errors
    ///
    /// Returns the same error variants as [`Self::rate_limit`].
    pub fn pnl_bounds_killswitch<Error: std::fmt::Display>(
        &self,
        name: &str,
        f: impl FnOnce(&mut PnlBoundsKillSwitchSettings) -> Result<(), Error>,
    ) -> Result<(), ConfigureError> {
        match self.registry.entry(name)? {
            ConfigEntry::PnlBoundsKillSwitch { settings, .. } => {
                let _guard = self.enter_configuration()?;
                settings.update(f).map_err(|error| {
                    ConfigRegistry::<RegistryFactory<Sync>>::validation(name, error)
                })
            }
            entry => Err(ConfigRegistry::<_>::type_mismatch::<
                PnlBoundsKillSwitchSettings,
            >(name, entry)),
        }
    }

    /// Force-sets the live accumulated P&L for a `(account, settlement_asset)`
    /// entry of the
    /// [`PnlBoundsKillSwitchPolicy`](crate::pretrade::policies::PnlBoundsKillSwitchPolicy)
    /// registered under `name`.
    ///
    /// This is an absolute assignment (upsert): the entry is created if it does
    /// not exist yet, exactly as a construction-time seed would. It is distinct
    /// from [`Self::pnl_bounds_killswitch`], which retunes the bounds and never
    /// touches accumulated P&L. The new value is evaluated against the live
    /// bounds on the next hot-path read.
    ///
    /// Unlike the settings retune, this does not take the configuration
    /// re-entrancy guard and does not touch the settings writer mutex: it writes
    /// only the realized-P&L storage (its own lock) and runs no user closure, so
    /// it can neither deadlock nor re-enter configuration.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigureError::UnknownPolicy`] for an unknown name or
    /// [`ConfigureError::PolicyTypeMismatch`] when the name belongs to another
    /// built-in policy type. The operation is otherwise infallible: an absolute
    /// assignment performs no validation and cannot overflow.
    pub fn set_account_pnl(
        &self,
        name: &str,
        account: AccountId,
        settlement_asset: Asset,
        pnl: Pnl,
    ) -> Result<(), ConfigureError> {
        match self.registry.entry(name)? {
            ConfigEntry::PnlBoundsKillSwitch { realized, .. } => {
                realized.with_mut(
                    (account, settlement_asset),
                    || Pnl::ZERO,
                    |entry, _is_new| *entry = pnl,
                );
                Ok(())
            }
            entry => Err(ConfigRegistry::<_>::type_mismatch::<
                PnlBoundsKillSwitchSettings,
            >(name, entry)),
        }
    }

    /// Retunes the [`SpotFundsPolicy`](crate::pretrade::policies::SpotFundsPolicy)
    /// registered under `name`.
    ///
    /// # Errors
    ///
    /// Returns the same error variants as [`Self::rate_limit`].
    pub fn spot_funds<Error: std::fmt::Display>(
        &self,
        name: &str,
        f: impl FnOnce(&mut SpotFundsSettings) -> Result<(), Error>,
    ) -> Result<(), ConfigureError> {
        match self.registry.entry(name)? {
            ConfigEntry::SpotFunds { settings, .. } => {
                let _guard = self.enter_configuration()?;
                settings.update(f).map_err(|error| {
                    ConfigRegistry::<RegistryFactory<Sync>>::validation(name, error)
                })
            }
            entry => Err(ConfigRegistry::<_>::type_mismatch::<SpotFundsSettings>(
                name, entry,
            )),
        }
    }

    /// Force-sets the live accumulated account-currency P&L for a spot-funds
    /// policy.
    ///
    /// This updates only the account-scoped current P&L ledger. It does not
    /// retune bounds and does not reset any other accumulator.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigureError::UnknownPolicy`] for an unknown name or
    /// [`ConfigureError::PolicyTypeMismatch`] when the name belongs to another
    /// built-in policy type.
    pub fn set_spot_funds_account_pnl(
        &self,
        name: &str,
        account: AccountId,
        account_currency: Asset,
        pnl: Pnl,
    ) -> Result<(), ConfigureError> {
        match self.registry.entry(name)? {
            ConfigEntry::SpotFunds {
                pnl: account_pnl, ..
            } => {
                account_pnl.with_mut(
                    (account, account_currency),
                    || Pnl::ZERO,
                    |entry, _is_new| *entry = pnl,
                );
                Ok(())
            }
            entry => Err(ConfigRegistry::<_>::type_mismatch::<SpotFundsSettings>(
                name, entry,
            )),
        }
    }

    /// Retunes the
    /// [`OrderSizeLimitPolicy`](crate::pretrade::policies::OrderSizeLimitPolicy)
    /// registered under `name`.
    ///
    /// # Errors
    ///
    /// Returns the same error variants as [`Self::rate_limit`].
    pub fn order_size_limit<Error: std::fmt::Display>(
        &self,
        name: &str,
        f: impl FnOnce(&mut OrderSizeLimitSettings) -> Result<(), Error>,
    ) -> Result<(), ConfigureError> {
        match self.registry.entry(name)? {
            ConfigEntry::OrderSizeLimit(cell) => {
                let _guard = self.enter_configuration()?;
                cell.update(f).map_err(|error| {
                    ConfigRegistry::<RegistryFactory<Sync>>::validation(name, error)
                })
            }
            entry => Err(ConfigRegistry::<_>::type_mismatch::<OrderSizeLimitSettings>(name, entry)),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::time::Duration;

    use crate::param::{AccountId, Asset, Quantity, Side, TradeAmount, Volume};
    use crate::pretrade::policies::{
        OrderSizeBrokerBarrier, OrderSizeLimit, OrderSizeLimitPolicy, OrderSizeLimitPolicyError,
        OrderSizeLimitSettings, RateLimit, RateLimitBrokerBarrier, RateLimitPolicy,
        RateLimitPolicyError, RateLimitSettings,
    };
    use crate::storage::FullLocking;
    use crate::{Engine, FullSyncEngine, Instrument, OrderOperation};

    use super::ConfigureError;

    fn broker_barrier(max_orders: usize) -> RateLimitBrokerBarrier {
        RateLimitBrokerBarrier {
            limit: RateLimit {
                max_orders,
                // Wide window so every order in the test shares one window.
                window: Duration::from_secs(60),
            },
        }
    }

    fn order_size_broker(max_quantity: &str) -> OrderSizeBrokerBarrier {
        OrderSizeBrokerBarrier {
            limit: OrderSizeLimit {
                max_quantity: Quantity::from_str(max_quantity)
                    .expect("quantity literal must be valid"),
                max_notional: Volume::from_str("1000000").expect("volume literal must be valid"),
            },
        }
    }

    fn build_engine(max_orders: usize) -> FullSyncEngine<OrderOperation> {
        let builder = Engine::builder::<OrderOperation, (), ()>().full_sync();
        let settings = RateLimitSettings::new(Some(broker_barrier(max_orders)), [], [], [])
            .expect("broker barrier is a valid configuration");
        let policy = RateLimitPolicy::<FullLocking>::new(settings, builder.storage_builder());
        builder
            .pre_trade(policy)
            .build()
            .expect("engine must build")
    }

    fn build_engine_with_order_size(max_orders: usize) -> FullSyncEngine<OrderOperation> {
        let builder = Engine::builder::<OrderOperation, (), ()>().full_sync();
        let rate_settings = RateLimitSettings::new(Some(broker_barrier(max_orders)), [], [], [])
            .expect("broker barrier is a valid configuration");
        let size_settings = OrderSizeLimitSettings::new(Some(order_size_broker("100")), [], [])
            .expect("order-size broker barrier is a valid configuration");
        let rate_policy =
            RateLimitPolicy::<FullLocking>::new(rate_settings, builder.storage_builder());
        let size_policy = OrderSizeLimitPolicy::<FullLocking>::new(size_settings);
        builder
            .pre_trade(rate_policy)
            .pre_trade(size_policy)
            .build()
            .expect("engine must build")
    }

    fn order(account: u64) -> OrderOperation {
        OrderOperation {
            instrument: Instrument::new(
                Asset::new("AAPL").expect("asset code must be valid"),
                Asset::new("USD").expect("asset code must be valid"),
            ),
            account_id: AccountId::from_u64(account),
            side: Side::Buy,
            trade_amount: TradeAmount::Quantity(
                Quantity::from_str("1").expect("quantity literal must be valid"),
            ),
            price: None,
        }
    }

    // A configuration call issued from within another configuration callback on
    // the same thread must be rejected with `NestedConfiguration` rather than
    // deadlocking on the settings cell's non-reentrant writer lock. The fact
    // that this test terminates is itself the no-hang proof.
    #[test]
    fn nested_same_thread_configuration_is_rejected_without_deadlock() {
        let engine = build_engine(2);
        let name = RateLimitPolicy::<FullLocking>::NAME;

        // Exhaust the broker limit of 2: the two orders pass (count 1..=2).
        for account in 0..2 {
            engine
                .execute_pre_trade(order(account))
                .expect("order within the limit must pass");
        }

        // The closure captures the engine by `&` (configure takes `&self`).
        // It first widens its private copy to 9, then re-enters configuration
        // for the same policy. The nested call must fail fast; the captured
        // error is asserted below. Returning the nested error rolls back the
        // outer transaction, so neither the widening to 9 nor the nested 100
        // is ever published.
        let nested = RefCell::new(None);
        let outer = engine
            .configure()
            .rate_limit::<ConfigureError>(name, |settings| {
                settings
                    .set_broker(Some(broker_barrier(9)))
                    .expect("widening to 9 is a valid private-copy edit");
                let inner = engine
                    .configure()
                    .rate_limit::<RateLimitPolicyError>(name, |settings| {
                        settings.set_broker(Some(broker_barrier(100)))
                    });
                let error = inner.expect_err("nested configuration must be rejected");
                *nested.borrow_mut() = Some(error.clone());
                Err(error)
            });

        // The nested call returned the new variant directly.
        assert_eq!(
            nested.into_inner(),
            Some(ConfigureError::NestedConfiguration)
        );
        // The outer call rolled back, surfacing the nested error through its
        // own validation channel.
        assert_eq!(
            outer,
            Err(ConfigureError::Validation {
                name: name.to_owned(),
                message: ConfigureError::NestedConfiguration.to_string(),
            })
        );

        // Live settings are unchanged: the limit is still 2, so a third order
        // (count 3) is rejected. A published widening would have admitted it.
        let rejects = engine
            .execute_pre_trade(order(2))
            .err()
            .expect("limit must still be 2 after the rejected retune");
        assert_eq!(rejects[0].reason, "rate limit exceeded: broker barrier");
    }

    #[test]
    fn nested_configuration_of_different_policy_in_same_engine_is_rejected() {
        let engine = build_engine_with_order_size(2);
        let rate_name = RateLimitPolicy::<FullLocking>::NAME;
        let size_name = OrderSizeLimitPolicy::<FullLocking>::NAME;

        let nested = RefCell::new(None);
        let outer = engine
            .configure()
            .rate_limit::<ConfigureError>(rate_name, |_settings| {
                let inner = engine
                    .configure()
                    .order_size_limit::<OrderSizeLimitPolicyError>(size_name, |settings| {
                        settings.set_broker(Some(order_size_broker("200")))
                    });
                let error = inner.expect_err("nested configuration must be rejected");
                *nested.borrow_mut() = Some(error.clone());
                Err(error)
            });

        assert_eq!(
            nested.into_inner(),
            Some(ConfigureError::NestedConfiguration)
        );
        assert_eq!(
            outer,
            Err(ConfigureError::Validation {
                name: rate_name.to_owned(),
                message: ConfigureError::NestedConfiguration.to_string(),
            })
        );
    }

    #[test]
    fn nested_configuration_of_independent_engine_is_allowed() {
        let engine_a = build_engine(2);
        let engine_b = build_engine(2);
        let name = RateLimitPolicy::<FullLocking>::NAME;

        engine_a
            .configure()
            .rate_limit::<RateLimitPolicyError>(name, |_settings| {
                engine_b
                    .configure()
                    .rate_limit(name, |settings| {
                        settings.set_broker(Some(broker_barrier(9)))
                    })
                    .expect("independent engine update must succeed");
                Ok(())
            })
            .expect("independent engine configuration must not be rejected");

        for account in 0..3 {
            engine_b
                .execute_pre_trade(order(account))
                .expect("engine B broker limit must have been widened");
        }
    }
}
