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

//! Pre-trade policy that gates spot orders on sufficient self-funds.

use crate::core::sync_mode::SyncMode;
use crate::core::AccountOutcomeEntry;
use crate::core::{
    HasAccountAdjustmentBalance, HasAccountAdjustmentBalanceLowerBound,
    HasAccountAdjustmentBalanceUpperBound, HasAccountAdjustmentHeld,
    HasAccountAdjustmentHeldLowerBound, HasAccountAdjustmentHeldUpperBound,
    HasAccountAdjustmentIncoming, HasAccountAdjustmentIncomingLowerBound,
    HasAccountAdjustmentIncomingUpperBound, HasAccountId, HasBalanceAsset,
    HasExecutionReportIsFinal, HasExecutionReportLastTrade, HasInstrument, HasLeavesQuantity,
    HasOrderPrice, HasPreTradeLock, HasSide, HasTradeAmount,
};
use crate::marketdata::MarketDataSync;
use crate::param::{AccountId, Asset};
use crate::pretrade::holdings::HoldingsStore;
use crate::pretrade::policy::{PolicyGroupId, PolicyName};
use crate::pretrade::ConfigurablePolicy;
use crate::pretrade::PreTradePolicy;
use crate::pretrade::{PolicyPreTradeResult, PostTradeResult, PreTradeContext, Rejects};
use crate::storage::{ConfigCell, CreateStorageFor, StorageBuilder};
use crate::{AccountAdjustmentContext, Mutations};

mod adjustment;
mod execution;
mod market_data;
mod market_order_pricer;
mod pre_trade;
mod rejects;
mod rollback;
mod views;

#[cfg(test)]
mod tests;

pub use market_data::{
    SpotFundsConfigError, SpotFundsMarketData, SpotFundsOverride, SpotFundsOverrideTarget,
    SpotFundsPricingSource, SpotFundsSettings,
};

// known cost: every call site does `holdings.with_mut((id, asset.clone()), ...)` —
// Asset::clone per lookup. SmolStr is allocator-free for tickers ≤22 bytes.
pub(super) type HoldingsKey = (AccountId, Asset);

/// Pre-trade policy that gates spot orders on sufficient self-funds.
///
/// Tracks `(account, asset) -> Holdings` (available + held). Order
/// reservation moves funds from `available` to `held`; execution
/// reports consume `held` (outflow side) and credit `available`
/// (inflow side); cancellation releases unfilled remainder back to
/// `available`. Account adjustments are applied through the
/// `apply_account_adjustment` hook on [`PreTradePolicy`].
///
/// Initial balances are always seeded through the
/// `apply_account_adjustment` pipeline. Missing `(account, asset)`
/// holdings are treated as zero and fail reservations through the
/// regular [`crate::pretrade::RejectCode::InsufficientFunds`] path.
///
/// The runtime-updatable slippage / pricing / override cascade and the policy
/// group tag live in [`SpotFundsSettings`], stored behind a settings cell read
/// allocation-free on the hot path. Market-order support is enabled by passing
/// a [`SpotFundsMarketData`](crate::pretrade::SpotFundsMarketData) to
/// [`new`](SpotFundsPolicy::new); it carries only the service handle, which is
/// fixed for the policy's lifetime. Without it, market orders (those with
/// `price=None`) are rejected with
/// [`crate::pretrade::RejectCode::UnsupportedOrderType`].
pub struct SpotFundsPolicy<Sync, MarketDataSyncMode>
where
    Sync: SyncMode,
    MarketDataSyncMode: MarketDataSync,
{
    pub(super) holdings: <<Sync as SyncMode>::StorageLockingPolicyFactory
        as crate::storage::LockingPolicyFactory>::Shared<
        HoldingsStore<
            <<Sync as SyncMode>::StorageLockingPolicyFactory
                as crate::storage::LockingPolicyFactory>::Policy,
        >,
    >,
    pub(super) settings: <Sync::StorageLockingPolicyFactory
        as crate::storage::LockingPolicyFactory>::Config<SpotFundsSettings>,
    pub(super) market_orders: Option<SpotFundsMarketData<MarketDataSyncMode>>,
}

impl<Sync, MarketDataSyncMode> SpotFundsPolicy<Sync, MarketDataSyncMode>
where
    Sync: SyncMode,
    MarketDataSyncMode: MarketDataSync,
{
    /// Stable policy name (used in rejects and logs).
    pub const NAME: &'static str = "SpotFundsPolicy";

    /// Builds the policy.
    ///
    /// `settings` carries the runtime-updatable slippage / pricing / override
    /// cascade (see [`SpotFundsSettings`]); it is stored behind the engine's
    /// settings cell and may be updated at runtime.
    ///
    /// `market_orders` enables market-order support when `Some`. It carries the
    /// shared [`MarketDataService`](crate::marketdata::MarketDataService) handle
    /// the policy prices market orders against. Pass `None` to disable market
    /// orders (they will be rejected with
    /// [`crate::pretrade::RejectCode::UnsupportedOrderType`]).
    ///
    /// `storage_builder` must come from the engine builder so the internal
    /// holdings storage uses the engine's synchronisation flavor. Initial
    /// balances are seeded at runtime via `apply_account_adjustment`.
    pub fn new(
        settings: SpotFundsSettings,
        market_orders: Option<SpotFundsMarketData<MarketDataSyncMode>>,
        storage_builder: &StorageBuilder<<Sync as SyncMode>::StorageLockingPolicyFactory>,
    ) -> Self
    where
        <Sync as SyncMode>::StorageLockingPolicyFactory: CreateStorageFor<(AccountId, Asset)>,
    {
        Self {
            holdings: <<Sync as SyncMode>::StorageLockingPolicyFactory
                as crate::storage::LockingPolicyFactory>::new_shared(
                HoldingsStore::new(storage_builder),
            ),
            settings: <Sync::StorageLockingPolicyFactory
                as crate::storage::LockingPolicyFactory>::new_config(settings),
            market_orders,
        }
    }

    /// Reads the policy group tag from the settings cell.
    ///
    /// Allocation-free and lock-free on the supported cell flavors.
    pub(super) fn group_id(&self) -> PolicyGroupId {
        self.settings.with(SpotFundsSettings::group_id)
    }

    /// Assigns a group tag to this policy instance.
    ///
    /// The tag is fixed at construction; it is recorded in the policy's
    /// [`SpotFundsSettings`] but has no runtime setter. See [`PolicyGroupId`]
    /// and [`DEFAULT_POLICY_GROUP_ID`](crate::pretrade::DEFAULT_POLICY_GROUP_ID)
    /// for details.
    pub fn with_policy_group_id(self, id: PolicyGroupId) -> Self {
        // The closure is infallible, so the transactional update always
        // publishes the new tag; `Infallible` keeps the cell's `Result` API
        // satisfied without a runtime failure path.
        self.settings
            .update::<std::convert::Infallible>(|s| {
                s.set_group_id(id);
                Ok(())
            })
            .unwrap_or_else(|e| match e {});
        self
    }
}

impl<Sync, MarketDataSyncMode> PolicyName for SpotFundsPolicy<Sync, MarketDataSyncMode>
where
    Sync: SyncMode,
    MarketDataSyncMode: MarketDataSync,
{
    fn policy_name(&self) -> &str {
        Self::NAME
    }
}

impl<Order, ExecutionReport, AccountAdjustment, Sync, MarketDataSyncMode>
    PreTradePolicy<Order, ExecutionReport, AccountAdjustment, Sync>
    for SpotFundsPolicy<Sync, MarketDataSyncMode>
where
    Order: HasInstrument + HasAccountId + HasSide + HasTradeAmount + HasOrderPrice,
    ExecutionReport: HasInstrument
        + HasAccountId
        + HasSide
        + HasExecutionReportLastTrade
        + HasLeavesQuantity
        + HasExecutionReportIsFinal
        + HasPreTradeLock,
    AccountAdjustment: HasBalanceAsset
        + HasAccountAdjustmentBalance
        + HasAccountAdjustmentBalanceLowerBound
        + HasAccountAdjustmentBalanceUpperBound
        + HasAccountAdjustmentHeld
        + HasAccountAdjustmentHeldLowerBound
        + HasAccountAdjustmentHeldUpperBound
        + HasAccountAdjustmentIncoming
        + HasAccountAdjustmentIncomingLowerBound
        + HasAccountAdjustmentIncomingUpperBound,
    Sync: SyncMode,
    MarketDataSyncMode: MarketDataSync,
    <<Sync as SyncMode>::StorageLockingPolicyFactory as crate::storage::LockingPolicyFactory>::Policy: 'static,
{
    fn name(&self) -> &str {
        Self::NAME
    }

    fn policy_group_id(&self) -> PolicyGroupId {
        self.group_id()
    }

    #[allow(private_interfaces)]
    fn built_in_config_entry(
        &self,
    ) -> Option<
        crate::core::ConfigEntry<
            <Sync as SyncMode>::StorageLockingPolicyFactory,
        >,
    > {
        Some(crate::core::ConfigEntry::SpotFunds(
            crate::pretrade::ConfigurablePolicy::settings_cell(self),
        ))
    }

    /// Applies an account adjustment to the policy's holdings.
    ///
    /// When a field is specified in the adjustment its outcome is always
    /// emitted in the returned [`AccountOutcomeEntry`], even if the resulting
    /// delta is zero. This differs from
    /// [`Self::apply_execution_report`], which omits zero-delta entries.
    fn apply_account_adjustment(
        &self,
        ctx: &AccountAdjustmentContext<<Sync as SyncMode>::StorageLockingPolicyFactory>,
        account_id: AccountId,
        adjustment: &AccountAdjustment,
        mutations: &mut Mutations,
    ) -> Result<Vec<AccountOutcomeEntry>, Rejects> {
        self.apply_account_adjustment_impl(
            Some(ctx.account_control.clone()),
            account_id,
            adjustment,
            mutations,
        )
    }

    /// Applies a venue-authoritative execution report.
    ///
    /// Processes the outflow side (charge asset) before the inflow side
    /// (counter asset) and updates holdings in storage immediately.
    ///
    /// Processing is not atomic. If the inflow side overflows after the
    /// outflow has already been applied, the outflow mutation remains in
    /// storage and the returned [`PostTradeResult`] carries both the partial
    /// `account_adjustments` and the blocking error in `account_blocks`.
    /// Callers must propagate every entry in `account_adjustments` to
    /// downstream systems regardless of the presence of `account_blocks`.
    ///
    /// The engine's `BlockedAccounts` machinery
    /// records any [`AccountBlock`](crate::pretrade::AccountBlock) returned
    /// here, so callers do not need to wire a separate sink for execution-
    /// report fixation overflows.
    fn apply_execution_report(
        &self,
        _ctx: &crate::pretrade::PostTradeContext<
            <Sync as crate::core::SyncMode>::StorageLockingPolicyFactory,
        >,
        report: &ExecutionReport,
    ) -> Option<PostTradeResult> {
        self.apply_execution_report_impl(report)
    }

    fn perform_pre_trade_check(
        &self,
        ctx: &PreTradeContext<<Sync as SyncMode>::StorageLockingPolicyFactory>,
        order: &Order,
        mutations: &mut Mutations,
    ) -> Result<Option<PolicyPreTradeResult>, Rejects> {
        self.perform_pre_trade_check_impl(ctx.account_control.clone(), ctx, order, mutations)
    }
}

impl<Sync, MarketDataSyncMode> ConfigurablePolicy<<Sync as SyncMode>::StorageLockingPolicyFactory>
    for SpotFundsPolicy<Sync, MarketDataSyncMode>
where
    Sync: SyncMode,
    MarketDataSyncMode: MarketDataSync,
{
    type Settings = SpotFundsSettings;

    fn settings_cell(
        &self,
    ) -> <Sync::StorageLockingPolicyFactory as crate::storage::LockingPolicyFactory>::Config<
        SpotFundsSettings,
    > {
        self.settings.clone()
    }
}
