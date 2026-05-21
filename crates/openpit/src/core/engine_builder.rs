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
// Please see https://github.com/openpitkit and the OWNERS file for details.

//! Engine builder types.

use std::collections::HashSet;
use std::fmt::{Display, Formatter};
use std::marker::PhantomData;

use super::engine::{Engine, EngineInner};
use super::engine_trait::EngineTraitOf;
use super::sync_mode::{AccountSync, FullSync, LocalSync, SyncMode};
use super::BlockedAccounts;
use crate::pretrade::PreTradePolicy;
use crate::storage::StorageBuilder;

// ─── IntoPolicyObject ────────────────────────────────────────────────────────

/// Converts a concrete policy into the trait-object shape selected by a
/// [`SyncMode`].
///
/// Three blanket impls exist for the policy trait:
///
/// - `Target = dyn PreTradePolicy<...>`: `Policy: PreTradePolicy + 'static`.
/// - `Target = dyn PreTradePolicy<...> + Send`: `Policy: PreTradePolicy +
///   Send + 'static`.
/// - `Target = dyn PreTradePolicy<...> + Send + Sync`:
///   `Policy: PreTradePolicy + Send + Sync + 'static`.
///
/// Custom modes using another policy-object shape must provide a matching
/// implementation.
pub trait IntoPolicyObject<Target: ?Sized>: 'static {
    /// Converts `self` into a boxed policy object.
    fn into_policy_object(self) -> Box<Target>;
}

impl<
        Order: 'static,
        ExecutionReport: 'static,
        AccountAdjustment: 'static,
        Policy: crate::pretrade::PreTradePolicy<Order, ExecutionReport, AccountAdjustment> + 'static,
    >
    IntoPolicyObject<dyn crate::pretrade::PreTradePolicy<Order, ExecutionReport, AccountAdjustment>>
    for Policy
{
    fn into_policy_object(
        self,
    ) -> Box<dyn crate::pretrade::PreTradePolicy<Order, ExecutionReport, AccountAdjustment>> {
        Box::new(self)
    }
}

impl<
        Order: 'static,
        ExecutionReport: 'static,
        AccountAdjustment: 'static,
        Policy: crate::pretrade::PreTradePolicy<Order, ExecutionReport, AccountAdjustment> + Send + 'static,
    >
    IntoPolicyObject<
        dyn crate::pretrade::PreTradePolicy<Order, ExecutionReport, AccountAdjustment> + Send,
    > for Policy
{
    fn into_policy_object(
        self,
    ) -> Box<dyn crate::pretrade::PreTradePolicy<Order, ExecutionReport, AccountAdjustment> + Send>
    {
        Box::new(self)
    }
}

impl<
        Order: 'static,
        ExecutionReport: 'static,
        AccountAdjustment: 'static,
        Policy: crate::pretrade::PreTradePolicy<Order, ExecutionReport, AccountAdjustment>
            + Send
            + Sync
            + 'static,
    >
    IntoPolicyObject<
        dyn crate::pretrade::PreTradePolicy<Order, ExecutionReport, AccountAdjustment>
            + Send
            + Sync,
    > for Policy
{
    fn into_policy_object(
        self,
    ) -> Box<
        dyn crate::pretrade::PreTradePolicy<Order, ExecutionReport, AccountAdjustment>
            + Send
            + Sync,
    > {
        Box::new(self)
    }
}

// ─── EngineBuildError ────────────────────────────────────────────────────────

/// Errors returned by [`ReadyEngineBuilder::build`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum EngineBuildError {
    /// Duplicate policy name across registered policy sets.
    DuplicatePolicyName { name: String },
}

impl Display for EngineBuildError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicatePolicyName { name } => {
                write!(formatter, "duplicate policy name: {name}")
            }
        }
    }
}

impl std::error::Error for EngineBuildError {}

// ─── EngineBuilder ───────────────────────────────────────────────────────────

/// Fluent builder for [`Engine`].
///
/// Policies are evaluated in registration order. Policy names must be unique
/// across start-stage, main-stage, and account-adjustment sets;
/// [`ReadyEngineBuilder::build`] returns [`EngineBuildError::DuplicatePolicyName`]
/// otherwise.
///
/// # Examples
///
/// ```rust
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use std::time::Duration;
/// use openpit::{WithExecutionReportOperation, WithFinancialImpact, WithOrderOperation};
/// use openpit::pretrade::policies::{
///     PnlBoundsAccountAssetBarrier, PnlBoundsBrokerBarrier, PnlBoundsKillSwitchPolicy,
///     RateLimit, RateLimitBrokerBarrier, RateLimitPolicy,
/// };
/// use openpit::Engine;
/// use openpit::param::{AccountId, Asset, Pnl};
///
/// type MyOrder = WithOrderOperation<()>;
/// type MyReport = WithFinancialImpact<WithExecutionReportOperation<()>>;
///
/// let builder = Engine::builder::<MyOrder, MyReport, ()>().no_sync();
///
/// let pnl_policy = PnlBoundsKillSwitchPolicy::new(
///     [PnlBoundsBrokerBarrier {
///         settlement_asset: Asset::new("USD")?,
///         lower_bound: Some(Pnl::from_str("-500")?),
///         upper_bound: None,
///     }],
///     [PnlBoundsAccountAssetBarrier {
///         barrier: PnlBoundsBrokerBarrier {
///             settlement_asset: Asset::new("USD")?,
///             lower_bound: Some(Pnl::from_str("-200")?),
///             upper_bound: None,
///         },
///         account_id: AccountId::from_u64(99224416),
///         initial_pnl: Pnl::from_str("-50")?,
///     }],
///     builder.storage_builder(),
/// )?;
///
/// let rate_policy = RateLimitPolicy::new(
///     Some(RateLimitBrokerBarrier {
///         limit: RateLimit { max_orders: 100, window: Duration::from_secs(1) },
///     }),
///     [],
///     [],
///     [],
///     builder.storage_builder(),
/// )?;
///
/// let engine = builder
///     .pre_trade(pnl_policy)
///     .pre_trade(rate_policy)
///     .build()?;
/// let _ = engine;
/// # Ok(())
/// # }
/// ```
pub struct EngineBuilder<Order: 'static, ExecutionReport: 'static, AccountAdjustment: 'static> {
    _marker: PhantomData<(Order, ExecutionReport, AccountAdjustment)>,
}

impl<Order: 'static, ExecutionReport: 'static, AccountAdjustment: 'static>
    EngineBuilder<Order, ExecutionReport, AccountAdjustment>
{
    /// Creates a new engine builder.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }

    /// Applies a custom synchronization mode and advances to
    /// [`SyncedEngineBuilder`].
    ///
    /// The mode must implement [`SyncMode`] and is typically zero-sized. For the
    /// built-in regimes, prefer [`full_sync`](Self::full_sync),
    /// [`no_sync`](Self::no_sync), or
    /// [`account_sync`](Self::account_sync).
    pub fn sync<Sync>(
        self,
        sync: Sync,
    ) -> SyncedEngineBuilder<Order, ExecutionReport, AccountAdjustment, Sync>
    where
        Sync: SyncMode,
    {
        SyncedEngineBuilder {
            pre_trade_policies: Vec::new(),
            storage_builder: StorageBuilder::new(sync.storage_locking_policy_factory()),
            _marker: PhantomData,
        }
    }

    /// Applies full thread-safety synchronization and advances to
    /// [`SyncedEngineBuilder`].
    ///
    /// Storage tables created by registered policies will use
    /// [`FullLocking`]: index and value domains are each protected by an
    /// independent reader-writer lock.
    ///
    /// [`FullLocking`]: crate::storage::FullLocking
    pub fn full_sync(
        self,
    ) -> SyncedEngineBuilder<Order, ExecutionReport, AccountAdjustment, FullSync> {
        self.sync(FullSync)
    }

    /// Applies single-thread (no-sync) synchronization and advances to
    /// [`SyncedEngineBuilder`].
    ///
    /// Storage tables created by registered policies will use
    /// [`NoLocking`]: no synchronization primitives are allocated. The
    /// resulting storages are `!Send + !Sync`; this option is for
    /// single-threaded embeddings where synchronization overhead must be
    /// zero.
    ///
    /// [`NoLocking`]: crate::storage::NoLocking
    pub fn no_sync(
        self,
    ) -> SyncedEngineBuilder<Order, ExecutionReport, AccountAdjustment, LocalSync> {
        self.sync(LocalSync)
    }

    /// Applies account-index synchronization and advances to
    /// [`SyncedEngineBuilder`].
    ///
    /// Storage tables created by registered policies will use
    /// [`IndexLocking`]: one reader-writer lock guards key insertions and
    /// removals; per-value access is the caller's responsibility. The engine
    /// handle is `Send + !Sync`: ownership may move between OS threads
    /// sequentially, but concurrent invocation on the same handle is not
    /// supported.
    ///
    /// [`IndexLocking`]: crate::storage::IndexLocking
    pub fn account_sync(
        self,
    ) -> SyncedEngineBuilder<Order, ExecutionReport, AccountAdjustment, AccountSync> {
        self.sync(AccountSync)
    }
}

// ─── SyncedEngineBuilder ─────────────────────────────────────────────────────

/// Engine builder with a synchronization mode applied.
///
/// Obtained from [`EngineBuilder::sync`], [`EngineBuilder::full_sync`],
/// [`EngineBuilder::no_sync`], or [`EngineBuilder::account_sync`].
///
/// This builder deliberately has **no `build` method**: at least one policy
/// must be registered before the engine can be constructed. Adding any policy
/// advances to [`ReadyEngineBuilder`], which exposes [`build`](ReadyEngineBuilder::build).
///
/// The `Sync` type parameter carries the chosen [`SyncMode`]
/// forward through the builder chain so that trading policies can create
/// correctly-synchronized [`Storage`] tables without knowing the concrete
/// factory type.
///
/// [`Storage`]: crate::storage::Storage
pub struct SyncedEngineBuilder<
    Order: 'static,
    ExecutionReport: 'static,
    AccountAdjustment: 'static,
    Sync: SyncMode,
> {
    pre_trade_policies: Vec<
        Box<<Sync as SyncMode>::PreTradePolicyObject<Order, ExecutionReport, AccountAdjustment>>,
    >,
    storage_builder: StorageBuilder<<Sync as SyncMode>::StorageLockingPolicyFactory>,
    _marker: PhantomData<(Order, ExecutionReport, AccountAdjustment)>,
}

impl<Order: 'static, ExecutionReport: 'static, AccountAdjustment: 'static, Sync>
    SyncedEngineBuilder<Order, ExecutionReport, AccountAdjustment, Sync>
where
    Sync: SyncMode,
{
    /// Returns the storage builder owned by this engine builder. Pass it (or
    /// a borrowed reference to it) to policy constructors that need internal
    /// storage tables. The factory type is shared with the engine builder's
    /// synchronization mode.
    pub fn storage_builder(
        &self,
    ) -> &StorageBuilder<<Sync as SyncMode>::StorageLockingPolicyFactory> {
        &self.storage_builder
    }
}

impl<Order: 'static, ExecutionReport: 'static, AccountAdjustment: 'static, Sync>
    SyncedEngineBuilder<Order, ExecutionReport, AccountAdjustment, Sync>
where
    Sync: SyncMode,
{
    /// Registers a policy and advances to [`ReadyEngineBuilder`].
    ///
    /// The required bound on `Policy` is determined by the `SyncMode`'s
    /// policy-object shape:
    ///
    /// - [`LocalSync`] (from `no_sync`): `'static` only; `!Send`
    ///   policy state is accepted.
    /// - [`AccountSync`] (from `account_sync`): `Send + 'static`.
    /// - [`FullSync`] (from `full_sync`): `Send + Sync + 'static`.
    pub fn pre_trade<Policy>(
        mut self,
        policy: Policy,
    ) -> ReadyEngineBuilder<Order, ExecutionReport, AccountAdjustment, Sync>
    where
        Policy: IntoPolicyObject<
            <Sync as SyncMode>::PreTradePolicyObject<Order, ExecutionReport, AccountAdjustment>,
        >,
    {
        self.pre_trade_policies.push(policy.into_policy_object());
        ReadyEngineBuilder {
            pre_trade_policies: self.pre_trade_policies,
            storage_builder: self.storage_builder,
            _marker: PhantomData,
        }
    }
}

// ─── ReadyEngineBuilder ──────────────────────────────────────────────────────

/// Engine builder with a synchronization mode and at least one trading
/// policy registered. Can produce an [`Engine`] via [`build`](Self::build).
///
/// Obtained from the `add_policy` methods on [`SyncedEngineBuilder`] or
/// from the chained `add_policy` methods on this type itself.
///
/// The `Sync` type parameter carries the chosen [`SyncMode`]
/// to any code that needs to create additional [`Storage`] tables with the
/// same synchronization regime.
///
/// [`Storage`]: crate::storage::Storage
pub struct ReadyEngineBuilder<
    Order: 'static,
    ExecutionReport: 'static,
    AccountAdjustment: 'static,
    Sync: SyncMode,
> {
    pre_trade_policies: Vec<
        Box<<Sync as SyncMode>::PreTradePolicyObject<Order, ExecutionReport, AccountAdjustment>>,
    >,
    storage_builder: StorageBuilder<<Sync as SyncMode>::StorageLockingPolicyFactory>,
    _marker: PhantomData<(Order, ExecutionReport, AccountAdjustment)>,
}

impl<Order: 'static, ExecutionReport: 'static, AccountAdjustment: 'static, Sync>
    ReadyEngineBuilder<Order, ExecutionReport, AccountAdjustment, Sync>
where
    Sync: SyncMode,
{
    /// Returns the storage builder owned by this engine builder. Pass it (or
    /// a borrowed reference to it) to policy constructors that need internal
    /// storage tables. The factory type is shared with the engine builder's
    /// synchronization mode.
    pub fn storage_builder(
        &self,
    ) -> &StorageBuilder<<Sync as SyncMode>::StorageLockingPolicyFactory> {
        &self.storage_builder
    }
}

impl<Order: 'static, ExecutionReport: 'static, AccountAdjustment: 'static, Sync>
    ReadyEngineBuilder<Order, ExecutionReport, AccountAdjustment, Sync>
where
    Sync: SyncMode,
{
    /// Registers an additional policy.
    pub fn pre_trade<Policy>(mut self, policy: Policy) -> Self
    where
        Policy: IntoPolicyObject<
            <Sync as SyncMode>::PreTradePolicyObject<Order, ExecutionReport, AccountAdjustment>,
        >,
    {
        self.pre_trade_policies.push(policy.into_policy_object());
        self
    }

    /// Builds the engine.
    pub fn build(
        self,
    ) -> Result<
        Engine<EngineTraitOf<Order, ExecutionReport, AccountAdjustment, Sync>>,
        EngineBuildError,
    > {
        ensure_unique_policy_names(self.pre_trade_policies.iter().map(|p| p.name()))?;
        let blocked_accounts = BlockedAccounts::new(&self.storage_builder);
        Ok(Engine::from_inner(<Sync as SyncMode>::new_strong(
            EngineInner {
                pre_trade_policies: self.pre_trade_policies,
                blocked_accounts,
            },
        )))
    }
}

fn ensure_unique_policy_names<'a>(
    names: impl Iterator<Item = &'a str>,
) -> Result<(), EngineBuildError> {
    let mut unique = HashSet::new();
    for name in names {
        if !unique.insert(name.to_owned()) {
            return Err(EngineBuildError::DuplicatePolicyName {
                name: name.to_owned(),
            });
        }
    }

    Ok(())
}
