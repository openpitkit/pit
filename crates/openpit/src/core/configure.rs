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

use super::account_groups::AccountGroupsHandle;
use super::engine::EngineInner;
use super::engine_trait::EngineTrait;
use super::sync_mode::SyncMode;
use crate::param::{AccountId, Asset, Pnl};
use crate::pretrade::policies::{
    account_pnl_barrier_change_blocks, account_pnl_membership_change_block, AccountPnlStorage,
    ActiveHoldingsMutationStorage, OrderSizeLimitSettings, PnlBoundsKillSwitchSettings,
    RateLimitSettings, RealizedPnlStorage, SpotFundsHoldingsStorage, SpotFundsSettings,
};
use crate::pretrade::{
    AccountBlockOutcome, AccountBlockOutcomes, PolicyConfigurationResult,
    PolicyRuntimeConfiguration, PreTradePolicy,
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
    ///
    /// Carries the settings cell (barrier retune), shared account P&L ledger,
    /// and holdings keys, so a barrier change can evaluate every known account
    /// in the same call.
    SpotFunds {
        /// Settings cell shared with the running policy.
        settings: Factory::Config<SpotFundsSettings>,
        /// Serializes publication with its barrier-transition sweep.
        transition: parking_lot::Mutex<()>,
        /// Live account P&L ledger shared with the running policy.
        pnl: AccountPnlStorage<Factory>,
        /// Live position holdings shared with the running policy.
        holdings: SpotFundsHoldingsStorage<Factory>,
        /// Accounts with provisional holdings that a finalizer may restore.
        active_holdings_mutations: ActiveHoldingsMutationStorage<Factory>,
    },
    /// Order-size-limit policy settings.
    OrderSizeLimit(Factory::Config<OrderSizeLimitSettings>),
}

fn update_serialized_transition<T, Cell, Error>(
    transition: &parking_lot::Mutex<()>,
    cell: &Cell,
    update: impl FnOnce(&mut T) -> Result<(), Error>,
    observe: impl FnOnce(&T, &T),
) -> Result<(), Error>
where
    T: Clone + 'static,
    Cell: ConfigCell<T>,
{
    let _transition = transition.lock();
    let previous = cell.with(Clone::clone);
    let mut current = previous.clone();
    update(&mut current)?;
    let published = current.clone();
    cell.update(|stored| {
        *stored = current;
        Ok(())
    })?;
    observe(&previous, &published);
    Ok(())
}

fn active_holdings_accounts<Factory: LockingPolicyFactory>(
    active: &ActiveHoldingsMutationStorage<Factory>,
) -> Vec<AccountId> {
    active
        .keys()
        .into_iter()
        .filter(|account| active.with(account, |count| *count != 0).unwrap_or(false))
        .collect()
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
    /// Configuration callbacks are non-reentrant. Re-entering configuration
    /// for the same engine from inside such a callback - whether for the same
    /// policy or a different one - is rejected before the nested call takes a
    /// serialization gate or settings-cell writer lock. Configuration from
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

    #[cfg(test)]
    pub(crate) fn empty() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub(crate) fn account_pnl_membership_change_blocks(
        &self,
        account: AccountId,
        previous_group: Option<crate::param::AccountGroupId>,
        current_group: Option<crate::param::AccountGroupId>,
        previous_currency: Option<Asset>,
        current_currency: Option<Asset>,
    ) -> Vec<crate::pretrade::AccountBlock> {
        self.entries
            .values()
            .filter_map(|entry| {
                let ConfigEntry::SpotFunds { settings, pnl, .. } = entry else {
                    return None;
                };
                settings.with(|settings| {
                    account_pnl_membership_change_block::<Factory>(
                        pnl,
                        settings,
                        account,
                        previous_group,
                        current_group,
                        previous_currency.as_ref(),
                        current_currency.as_ref(),
                    )
                })
            })
            .collect()
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
/// thread, so a nested call returns before it can wait on configuration
/// serialization or publish a settings value.
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

/// Storage locking factory backing a [`Configurator`] of a given engine type.
type RegistryFactory<Trait> =
    <<Trait as EngineTrait>::Sync as SyncMode>::StorageLockingPolicyFactory;

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
pub struct Configurator<Trait: EngineTrait> {
    inner: <Trait::Sync as SyncMode>::Strong<EngineInner<Trait>>,
}

impl<Trait: EngineTrait> Clone for Configurator<Trait> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<Trait: EngineTrait> Configurator<Trait> {
    /// Wraps the engine's shared state in a configurator handle.
    pub(crate) fn from_inner(inner: <Trait::Sync as SyncMode>::Strong<EngineInner<Trait>>) -> Self {
        Self { inner }
    }

    fn registry(&self) -> &ConfigRegistry<RegistryFactory<Trait>> {
        &self.inner.config_registry
    }

    fn enter_configuration(&self) -> Result<ConfiguringGuard, ConfigureError> {
        ConfiguringGuard::enter(std::ptr::from_ref(self.registry()) as usize)
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
        match self.registry().entry(name)? {
            ConfigEntry::RateLimit(cell) => {
                let _guard = self.enter_configuration()?;
                cell.update(f).map_err(|error| {
                    ConfigRegistry::<RegistryFactory<Trait>>::validation(name, error)
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
        match self.registry().entry(name)? {
            ConfigEntry::PnlBoundsKillSwitch { settings, .. } => {
                let _guard = self.enter_configuration()?;
                settings.update(f).map_err(|error| {
                    ConfigRegistry::<RegistryFactory<Trait>>::validation(name, error)
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
        match self.registry().entry(name)? {
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
    /// An update that arms, narrows, or otherwise changes an account's
    /// effective account P&L barrier evaluates that barrier against the stored
    /// account P&L before returning: an account already halted or already
    /// beyond the new barrier is blocked here rather than at its next fill.
    /// The returned blocks are the ones the engine has already recorded. An
    /// update that leaves an account's effective barrier as it was does not
    /// re-evaluate it, so retuning slippage, pricing, or the limit mode never
    /// re-blocks an account an operator has unblocked. Each returned
    /// [`AccountBlockOutcome`] identifies the engine-selected account that owns
    /// its newly inserted block. Removing the last effective barrier reports no
    /// block, while clearing an override can expose a fallback barrier that is
    /// evaluated normally and can record a block. Neither operation releases a
    /// previously recorded block.
    ///
    /// # Errors
    ///
    /// Returns the same error variants as [`Self::rate_limit`].
    pub fn spot_funds<Error: std::fmt::Display>(
        &self,
        name: &str,
        f: impl FnOnce(&mut SpotFundsSettings) -> Result<(), Error>,
    ) -> Result<AccountBlockOutcomes, ConfigureError> {
        match self.registry().entry(name)? {
            ConfigEntry::SpotFunds {
                settings,
                transition,
                pnl,
                holdings,
                active_holdings_mutations,
                ..
            } => {
                let _guard = self.enter_configuration()?;
                let account_groups = AccountGroupsHandle::<
                    <Trait::Sync as SyncMode>::StorageLockingPolicyFactory,
                >::from_inner(
                    self.inner.account_groups.clone()
                );
                let mut account_blocks = Vec::new();
                update_serialized_transition(transition, settings, f, |previous, current| {
                    self.inner.account_currencies.with_state_transition(|| {
                        let blocks = account_pnl_barrier_change_blocks::<RegistryFactory<Trait>>(
                            pnl,
                            holdings,
                            previous,
                            current,
                            &account_groups.memberships(),
                            self.inner
                                .account_currencies
                                .known_account_keys()
                                .into_iter()
                                .chain(active_holdings_accounts::<RegistryFactory<Trait>>(
                                    active_holdings_mutations,
                                )),
                            |account, group| {
                                self.inner.account_currencies.currency_of(account, group)
                            },
                        );
                        account_blocks.reserve(blocks.len());
                        for (account, block) in blocks {
                            if self
                                .inner
                                .blocked_accounts
                                .block_account(account, block.clone())
                            {
                                account_blocks.push(AccountBlockOutcome {
                                    account_id: account,
                                    block,
                                });
                            }
                        }
                    });
                })
                .map_err(|error| {
                    ConfigRegistry::<RegistryFactory<Trait>>::validation(name, error)
                })?;
                Ok(AccountBlockOutcomes { account_blocks })
            }
            entry => Err(ConfigRegistry::<_>::type_mismatch::<SpotFundsSettings>(
                name, entry,
            )),
        }
    }

    /// Force-sets the live accumulated account P&L state for a spot-funds policy.
    ///
    /// This updates only the account-scoped current P&L ledger. It does not
    /// retune bounds and does not reset any other accumulator. The policy
    /// reports an account block when the replacement violates an effective P&L
    /// barrier, either by exceeding a bound or by being halted. The engine
    /// processes that block before returning it. If the account is already
    /// blocked, its first stored cause remains unchanged and the result still
    /// contains the policy-reported block.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigureError::UnknownPolicy`] for an unknown name or
    /// [`ConfigureError::PolicyTypeMismatch`] when the name belongs to another
    /// built-in policy type. On success returns the policy-reported account
    /// blocks, including an empty list when the replacement does not violate an
    /// effective barrier.
    pub fn set_spot_funds_account_pnl(
        &self,
        name: &str,
        account: AccountId,
        state: crate::PnlState,
    ) -> Result<PolicyConfigurationResult, ConfigureError> {
        match self.registry().entry(name)? {
            ConfigEntry::SpotFunds { .. } => {
                let policy = self
                    .inner
                    .pre_trade_policies
                    .iter()
                    .find(|policy| policy.name() == name)
                    .ok_or_else(|| ConfigureError::UnknownPolicy {
                        name: name.to_owned(),
                    })?;
                self.inner.account_currencies.with_state_writer(|| {
                    let account_groups = AccountGroupsHandle::<
                        <Trait::Sync as SyncMode>::StorageLockingPolicyFactory,
                    >::from_inner(
                        self.inner.account_groups.clone()
                    );
                    let account_group_id = account_groups.group_of(account);
                    let configuration = PolicyRuntimeConfiguration::SetSpotFundsAccountPnl {
                        account_id: account,
                        account_group_id,
                        account_currency: self
                            .inner
                            .account_currencies
                            .currency_of(account, account_group_id),
                        state,
                    };
                    let result = policy.apply_runtime_configuration(configuration);
                    for block in &result.account_blocks {
                        self.inner
                            .blocked_accounts
                            .block_account(account, block.clone());
                    }
                    Ok(result)
                })
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
        match self.registry().entry(name)? {
            ConfigEntry::OrderSizeLimit(cell) => {
                let _guard = self.enter_configuration()?;
                cell.update(f).map_err(|error| {
                    ConfigRegistry::<RegistryFactory<Trait>>::validation(name, error)
                })
            }
            entry => Err(ConfigRegistry::<_>::type_mismatch::<OrderSizeLimitSettings>(name, entry)),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use crate::param::{AccountId, Asset, Quantity, Side, TradeAmount};
    use crate::pretrade::policies::{
        OrderSizeBrokerBarrier, OrderSizeLimit, OrderSizeLimitPolicy, OrderSizeLimitPolicyError,
        OrderSizeLimitSettings, RateLimit, RateLimitBrokerBarrier, RateLimitPolicy,
        RateLimitPolicyError, RateLimitSettings,
    };
    use crate::storage::{ConfigCell, FullLocking, StorageBuilder};
    use crate::{Engine, FullSyncEngine, Instrument, OrderOperation};

    use super::{active_holdings_accounts, update_serialized_transition, ConfigureError};

    #[derive(Clone)]
    struct BlockingReadConfigCell<T>(Arc<Mutex<T>>);

    impl<T: Clone + 'static> ConfigCell<T> for BlockingReadConfigCell<T> {
        fn new(value: T) -> Self {
            Self(Arc::new(Mutex::new(value)))
        }

        fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
            let value = self.0.lock().expect("test config mutex must not poison");
            f(&value)
        }

        fn update<E>(&self, f: impl FnOnce(&mut T) -> Result<(), E>) -> Result<(), E> {
            let mut value = self.0.lock().expect("test config mutex must not poison");
            let mut next = value.clone();
            f(&mut next)?;
            *value = next;
            Ok(())
        }
    }

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
                max_quantity: Some(
                    Quantity::from_str(max_quantity).expect("quantity literal must be valid"),
                ),
                max_notional: None,
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

    #[test]
    fn spot_funds_transition_gate_prevents_a_stale_observer_from_overtaking() {
        use std::sync::mpsc;

        let gate = Arc::new(parking_lot::Mutex::new(()));
        let cell = BlockingReadConfigCell::new(0_i32);
        let (first_observer_tx, first_observer_rx) = mpsc::sync_channel(1);
        let (release_first_tx, release_first_rx) = mpsc::sync_channel(1);
        let (second_observer_tx, second_observer_rx) = mpsc::sync_channel(1);

        std::thread::scope(|scope| {
            let first_gate = Arc::clone(&gate);
            let first_cell = cell.clone();
            let first_update_cell = cell.clone();
            let first_observer_cell = cell.clone();
            scope.spawn(move || {
                update_serialized_transition(
                    &first_gate,
                    &first_cell,
                    |value| {
                        assert_eq!(first_update_cell.with(|value| *value), 0);
                        *value = 1;
                        Ok::<_, ()>(())
                    },
                    |previous, current| {
                        assert_eq!(first_observer_cell.with(|value| *value), 1);
                        first_observer_tx
                            .send((*previous, *current))
                            .expect("first transition must be observed");
                        release_first_rx
                            .recv()
                            .expect("first observer must be released");
                    },
                )
                .expect("first update must succeed");
            });

            assert_eq!(
                first_observer_rx
                    .recv()
                    .expect("first observer must publish"),
                (0, 1)
            );

            let second_gate = Arc::clone(&gate);
            let second_cell = cell.clone();
            scope.spawn(move || {
                update_serialized_transition(
                    &second_gate,
                    &second_cell,
                    |value| {
                        *value = 2;
                        Ok::<_, ()>(())
                    },
                    |previous, current| {
                        second_observer_tx
                            .send((*previous, *current))
                            .expect("second transition must be observed");
                    },
                )
                .expect("second update must succeed");
            });

            assert_eq!(cell.with(|value| *value), 1);
            assert!(second_observer_rx
                .recv_timeout(Duration::from_millis(50))
                .is_err());
            release_first_tx
                .send(())
                .expect("first observer must still be waiting");
            assert_eq!(
                second_observer_rx
                    .recv_timeout(Duration::from_secs(1))
                    .expect("second observer must run last"),
                (1, 2)
            );
        });

        assert_eq!(cell.with(|value| *value), 2);
    }

    #[test]
    fn spot_funds_transition_helper_keeps_rejected_update_transactional() {
        let gate = parking_lot::Mutex::new(());
        let cell = BlockingReadConfigCell::new(7_i32);
        let mut observed = false;

        let rejected = update_serialized_transition(
            &gate,
            &cell,
            |value| {
                *value = 8;
                Err("rejected")
            },
            |_, _| observed = true,
        );
        assert_eq!(rejected, Err("rejected"));
        assert_eq!(cell.with(|value| *value), 7);
        assert!(!observed);

        update_serialized_transition(
            &gate,
            &cell,
            |value| {
                *value = 9;
                Ok::<_, ()>(())
            },
            |previous, current| assert_eq!((*previous, *current), (7, 9)),
        )
        .expect("gate must remain usable after rejection");
        assert_eq!(cell.with(|value| *value), 9);
    }

    #[test]
    fn inactive_holdings_marker_is_not_a_retune_candidate() {
        let active = StorageBuilder::new(FullLocking).create_shared::<AccountId, usize>();
        let account = AccountId::from_u64(42);
        active.with_mut(account, || 0, |_, _| {});

        assert!(active_holdings_accounts::<FullLocking>(&active).is_empty());
    }

    // A configuration call issued from within another configuration callback on
    // the same thread must be rejected with `NestedConfiguration` rather than
    // recursively entering configuration serialization. The fact that this
    // test terminates is itself the no-hang proof.
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
