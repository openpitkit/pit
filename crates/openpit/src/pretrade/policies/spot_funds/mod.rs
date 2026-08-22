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

//! Pre-trade policy that gates spot orders on sufficient self-funds.

mod limit_mode;
pub use limit_mode::SpotFundsLimitMode;

use crate::core::sync_mode::SyncMode;
use crate::core::{
    HasAccountAdjustmentBalance, HasAccountAdjustmentBalanceAverageEntryPrice,
    HasAccountAdjustmentBalanceLowerBound, HasAccountAdjustmentBalanceUpperBound,
    HasAccountAdjustmentHeld, HasAccountAdjustmentHeldLowerBound,
    HasAccountAdjustmentHeldUpperBound, HasAccountAdjustmentIncoming,
    HasAccountAdjustmentIncomingLowerBound, HasAccountAdjustmentIncomingUpperBound,
    HasAccountAdjustmentPnlOperation, HasAccountId, HasBalanceAsset, HasExecutionReportFillFee,
    HasExecutionReportIsFinal, HasExecutionReportLastTrade, HasInstrument, HasOrderPrice,
    HasPreTradeLock, HasRemainingReservedQuantity, HasSide, HasTradeAmount,
};
use crate::marketdata::MarketDataSync;
use crate::param::{AccountGroupId, AccountId, Asset, Pnl};
use crate::pretrade::holdings::HoldingsStore;
use crate::pretrade::policy::{PolicyGroupId, PolicyName};
use crate::pretrade::ConfigurablePolicy;
use crate::pretrade::PreTradePolicy;
use crate::pretrade::{
    PolicyAccountAdjustmentResult, PolicyConfigurationResult, PolicyPreTradeResult,
    PolicyRuntimeConfiguration, PostTradeResult, PreTradeContext, Rejects,
};
use crate::storage::{CreateStorageFor, LockingPolicyFactory, Storage, StorageBuilder};
use crate::{AccountAdjustmentContext, Mutations};

mod adjustment;
mod execution;
mod market_data;
mod market_order_pricer;
mod pnl;
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
pub use pnl::{
    SpotFundsPnlBoundsAccountBarrier, SpotFundsPnlBoundsAccountGroupBarrier,
    SpotFundsPnlBoundsBarrier,
};

const SPOT_FUNDS_POLICY_NAME: &str = "SpotFundsPolicy";

// known cost: every call site does `holdings.with_mut((id, asset.clone()), ...)` -
// Asset::clone per lookup. SmolStr is allocator-free for tickers ≤22 bytes.
pub(super) type HoldingsKey = (AccountId, Asset);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AccountPnlEntry {
    pub(super) state: crate::PnlState,
    pub(super) assertion_token: Option<u64>,
}

impl AccountPnlEntry {
    fn zero() -> Self {
        Self {
            state: crate::PnlState::Value(Pnl::ZERO),
            assertion_token: None,
        }
    }
}

#[derive(Clone, Copy)]
struct AccountPnlLease {
    owner_id: u64,
    depth: usize,
    /// Thread that took the lease, kept only to catch a wait that can never
    /// end (see [`SpotFundsPolicy::acquire_account_pnl_lease`]). Debug-only, so
    /// release builds pay neither the field nor the thread-local read.
    #[cfg(debug_assertions)]
    owner_thread: std::thread::ThreadId,
}

impl AccountPnlLease {
    fn new(owner_id: u64) -> Self {
        Self {
            owner_id,
            depth: 1,
            #[cfg(debug_assertions)]
            owner_thread: std::thread::current().id(),
        }
    }
}

/// Shared account-scoped PnL accumulator.
pub(crate) type AccountPnlStorage<LockingPolicyFactory> =
    <LockingPolicyFactory as crate::storage::LockingPolicyFactory>::Shared<
        Storage<
            AccountId,
            AccountPnlEntry,
            <LockingPolicyFactory as crate::storage::LockingPolicyFactory>::Policy,
        >,
    >;

pub(crate) type SpotFundsHoldingsStorage<LockingPolicyFactory> =
    <LockingPolicyFactory as crate::storage::LockingPolicyFactory>::Shared<
        HoldingsStore<<LockingPolicyFactory as crate::storage::LockingPolicyFactory>::Policy>,
    >;

pub(crate) type ActiveHoldingsMutationStorage<LockingPolicyFactory> =
    <LockingPolicyFactory as crate::storage::LockingPolicyFactory>::Shared<
        Storage<
            AccountId,
            usize,
            <LockingPolicyFactory as crate::storage::LockingPolicyFactory>::Policy,
        >,
    >;

pub(super) struct ActiveHoldingsMutationGuard<StorageFactory>
where
    StorageFactory: LockingPolicyFactory,
{
    active: ActiveHoldingsMutationStorage<StorageFactory>,
    account_id: AccountId,
}

impl<StorageFactory> Drop for ActiveHoldingsMutationGuard<StorageFactory>
where
    StorageFactory: LockingPolicyFactory,
{
    fn drop(&mut self) {
        self.active.with_mut_if_present(&self.account_id, |count| {
            *count = count.saturating_sub(1);
        });
        self.active.remove_if(&self.account_id, |count| *count == 0);
    }
}

type AccountPnlLeaseStorage<LockingPolicyFactory> =
    <LockingPolicyFactory as crate::storage::LockingPolicyFactory>::Shared<
        Storage<
            AccountId,
            Option<AccountPnlLease>,
            <LockingPolicyFactory as crate::storage::LockingPolicyFactory>::Policy,
        >,
    >;

#[cfg(test)]
type AccountPnlLeaseWaitHookStorage<LockingPolicyFactory> =
    <LockingPolicyFactory as crate::storage::LockingPolicyFactory>::Shared<
        Storage<
            AccountId,
            Option<std::sync::Arc<std::sync::Barrier>>,
            <LockingPolicyFactory as crate::storage::LockingPolicyFactory>::Policy,
        >,
    >;

#[cfg(test)]
type PreTradeReservationHookStorage<LockingPolicyFactory> =
    <LockingPolicyFactory as crate::storage::LockingPolicyFactory>::Shared<
        Storage<
            AccountId,
            Option<(
                std::sync::Arc<std::sync::Barrier>,
                std::sync::Arc<std::sync::Barrier>,
            )>,
            <LockingPolicyFactory as crate::storage::LockingPolicyFactory>::Policy,
        >,
    >;

pub(super) struct AccountPnlLeaseGuard<StorageFactory>
where
    StorageFactory: LockingPolicyFactory,
{
    leases: AccountPnlLeaseStorage<StorageFactory>,
    account_id: AccountId,
    owner_id: u64,
}

impl<StorageFactory> Drop for AccountPnlLeaseGuard<StorageFactory>
where
    StorageFactory: LockingPolicyFactory,
{
    fn drop(&mut self) {
        release_account_pnl_lease::<StorageFactory>(&self.leases, self.account_id, self.owner_id);
    }
}

/// Re-evaluates known accounts against an account P&L barrier change.
///
/// A barrier is a control, so arming it, narrowing it, or moving an account
/// onto a different tier of the cascade must decide the account's fate in that
/// same call rather than waiting for the account's next fill. Accounts are
/// evaluated only when their effective barrier changes or an unchanged
/// account-tier barrier changes currency-match status. An unrelated settings
/// retune or otherwise unchanged group/global barrier therefore never re-blocks
/// an account an operator has just unblocked. Removing the last effective
/// barrier yields no block. Clearing an override can expose a fallback barrier,
/// which is evaluated normally and can record a block. Neither operation
/// releases an existing block - a block belongs to the engine and only an
/// explicit operator action lifts it.
///
/// A global barrier change sweeps every account known through P&L, holdings
/// (including provisional state), explicit account barriers, group membership,
/// or account currency. An
/// account/group-only change is narrower: it checks changed account targets
/// and members of changed groups. Missing P&L is the implicit numeric zero and
/// is never materialized by this read-only sweep.
pub(crate) fn account_pnl_barrier_change_blocks<StorageFactory>(
    pnl: &AccountPnlStorage<StorageFactory>,
    holdings: &SpotFundsHoldingsStorage<StorageFactory>,
    previous: &SpotFundsSettings,
    current: &SpotFundsSettings,
    memberships: &[(AccountId, AccountGroupId)],
    known_accounts: impl IntoIterator<Item = AccountId>,
    resolve_account_currency: impl Fn(AccountId, Option<AccountGroupId>) -> Option<Asset>,
) -> Vec<(AccountId, crate::pretrade::AccountBlock)>
where
    StorageFactory: LockingPolicyFactory,
{
    let global_changed = previous.pnl_global_barrier() != current.pnl_global_barrier();
    let mut changed_accounts = std::collections::BTreeSet::new();
    for account_id in previous
        .pnl_account_barriers()
        .keys()
        .chain(current.pnl_account_barriers().keys())
    {
        if previous.pnl_account_barriers().get(account_id)
            != current.pnl_account_barriers().get(account_id)
        {
            changed_accounts.insert(*account_id);
        }
    }
    let mut changed_groups = std::collections::BTreeSet::new();
    for group_id in previous
        .pnl_account_group_barriers()
        .keys()
        .chain(current.pnl_account_group_barriers().keys())
    {
        if previous.pnl_account_group_barriers().get(group_id)
            != current.pnl_account_group_barriers().get(group_id)
        {
            changed_groups.insert(*group_id);
        }
    }

    if !global_changed && changed_accounts.is_empty() && changed_groups.is_empty() {
        return Vec::new();
    }

    let memberships = memberships
        .iter()
        .copied()
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut candidates = changed_accounts;
    if global_changed {
        candidates.extend(pnl.keys());
        candidates.extend(holdings.keys().into_iter().map(|(account, _)| account));
        candidates.extend(previous.pnl_account_barriers().keys().copied());
        candidates.extend(current.pnl_account_barriers().keys().copied());
        candidates.extend(memberships.keys().copied());
        candidates.extend(known_accounts);
    } else if !changed_groups.is_empty() {
        candidates.extend(
            memberships
                .iter()
                .filter_map(|(account, group)| changed_groups.contains(group).then_some(*account)),
        );
    }

    let mut blocks = Vec::new();
    for account_id in candidates {
        let account_group_id = memberships.get(&account_id).copied();
        let account_currency = resolve_account_currency(account_id, account_group_id);
        let Some(current_barrier) =
            current.pnl_barrier_for(account_id, account_group_id, account_currency.as_ref())
        else {
            continue;
        };
        if previous.pnl_barrier_for(account_id, account_group_id, account_currency.as_ref())
            == Some(current_barrier)
        {
            continue;
        }
        let (state, provenance) = pnl
            .with(&account_id, |entry| (entry.state, entry.assertion_token))
            .unwrap_or((crate::PnlState::Value(Pnl::ZERO), None));
        let Some(block) = rejects::account_pnl_block_for_state(
            account_currency.as_ref(),
            state,
            current_barrier,
            provenance,
        ) else {
            continue;
        };
        blocks.push((account_id, block));
    }
    blocks
}

pub(crate) fn account_pnl_membership_change_block<StorageFactory>(
    pnl: &AccountPnlStorage<StorageFactory>,
    settings: &SpotFundsSettings,
    account_id: AccountId,
    previous_group: Option<crate::param::AccountGroupId>,
    current_group: Option<crate::param::AccountGroupId>,
    previous_currency: Option<&Asset>,
    current_currency: Option<&Asset>,
) -> Option<crate::pretrade::AccountBlock>
where
    StorageFactory: LockingPolicyFactory,
{
    let previous = settings.pnl_barrier_for(account_id, previous_group, previous_currency);
    let current = settings.pnl_barrier_for(account_id, current_group, current_currency);
    // A currency change matters only when it flips the account-barrier currency
    // mismatch: the same barrier under a still-matching (or still-unknown)
    // currency controls the account exactly as before, and re-asserting the
    // stored P&L against it would block on a condition that did not change.
    let mismatches = |barrier: Option<&SpotFundsPnlBoundsBarrier>, currency: Option<&Asset>| {
        matches!((barrier, currency), (Some(barrier), Some(currency))
            if &barrier.currency != currency)
    };
    if previous == current
        && mismatches(previous, previous_currency) == mismatches(current, current_currency)
    {
        return None;
    }
    let current = current?;
    let (state, provenance) = pnl
        .with(&account_id, |entry| (entry.state, entry.assertion_token))
        .unwrap_or((crate::PnlState::Value(Pnl::ZERO), None));
    rejects::account_pnl_block_for_state(current_currency, state, current, provenance)
}

fn release_account_pnl_lease<StorageFactory>(
    leases: &AccountPnlLeaseStorage<StorageFactory>,
    account_id: AccountId,
    owner_id: u64,
) where
    StorageFactory: LockingPolicyFactory,
{
    leases.with_mut_if_present(&account_id, |lease| {
        if let Some(existing) = lease {
            if existing.owner_id == owner_id {
                if existing.depth == 1 {
                    *lease = None;
                } else {
                    existing.depth -= 1;
                }
            }
        }
    });
}

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
/// Average entry price and realized PnL accounting uses the account currency
/// resolved by the engine account registry as calculation context. The account
/// has one PnL accumulator independent of that currency.
///
/// The account line is engaged only by an event that creates a realized
/// contribution: a fill that reduces, closes, or reverses the position, or a
/// non-zero fee. Opening, same-direction, and zero-quantity fills without a
/// non-zero fee leave it untouched - they neither publish a zero nor require
/// an account currency, and an unset accumulator stays unset. A realizing fill
/// whose contribution is an exact zero still publishes that authoritative
/// zero. Position PnL follows its own rule: every fill with a non-zero
/// quantity rewrites the cost basis and therefore needs the account currency,
/// and so does any non-zero fee.
///
/// Any unavailable input needed for the account-line aggregate calculation
/// halts it until an explicit correction.
/// A configured account PnL barrier rejects pre-trade while that accumulator is
/// halted and blocks the account after post-trade has applied. With a known
/// effective account currency, an account-tier mismatch fails closed and blocks
/// the account; account-group and global mismatches are skipped. A
/// known-currency account with no account barrier and no matching fallback has
/// no effective barrier, but its PnL keeps accumulating and publishing. Without
/// an effective currency, the first in-scope barrier applies. Bounds are
/// compared as stored and are never FX-converted.
/// Position PnL is tracked independently per asset and never participates
/// directly in that barrier. Position and account halts are both sticky until a
/// manager replaces the corresponding state.
///
/// The runtime-updatable slippage / pricing / override cascade lives in
/// [`SpotFundsSettings`], stored behind a settings cell read allocation-free
/// on the hot path. The policy group tag lives on the policy instance.
/// Market-order support is enabled by passing a
/// [`SpotFundsMarketData`](crate::pretrade::SpotFundsMarketData) to
/// [`new`](SpotFundsPolicy::new); it carries only the service handle, which is
/// fixed for the policy's lifetime. Without it, market orders (those with
/// `price=None`) are rejected with
/// [`crate::pretrade::RejectCode::UnsupportedOrderType`].
pub struct SpotFundsPolicy<Sync, MarketDataSyncMode>
where
    Sync: SyncMode,
    Sync::StorageLockingPolicyFactory: LockingPolicyFactory,
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
        as LockingPolicyFactory>::Config<SpotFundsSettings>,
    pub(super) market_orders: Option<SpotFundsMarketData<MarketDataSyncMode>>,
    pub(super) pnl: AccountPnlStorage<Sync::StorageLockingPolicyFactory>,
    active_holdings_mutations:
        ActiveHoldingsMutationStorage<Sync::StorageLockingPolicyFactory>,
    pnl_leases: AccountPnlLeaseStorage<Sync::StorageLockingPolicyFactory>,
    #[cfg(test)]
    pnl_lease_wait_hooks: AccountPnlLeaseWaitHookStorage<Sync::StorageLockingPolicyFactory>,
    #[cfg(test)]
    pre_trade_reservation_hooks:
        PreTradeReservationHookStorage<Sync::StorageLockingPolicyFactory>,
    group_id: PolicyGroupId,
}

impl<Sync, MarketDataSyncMode> SpotFundsPolicy<Sync, MarketDataSyncMode>
where
    Sync: SyncMode,
    Sync::StorageLockingPolicyFactory: LockingPolicyFactory,
    MarketDataSyncMode: MarketDataSync,
{
    /// Stable policy name (used in rejects and logs).
    pub const NAME: &'static str = SPOT_FUNDS_POLICY_NAME;

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
        <Sync as SyncMode>::StorageLockingPolicyFactory:
            CreateStorageFor<(AccountId, Asset)> + CreateStorageFor<AccountId>,
    {
        let pnl = storage_builder.create_for_bound_key::<AccountId, AccountPnlEntry>();
        let pnl =
            <Sync::StorageLockingPolicyFactory as crate::storage::LockingPolicyFactory>::new_shared(
                pnl,
            );
        let pnl_leases =
            storage_builder.create_for_bound_key::<AccountId, Option<AccountPnlLease>>();
        let pnl_leases =
            <Sync::StorageLockingPolicyFactory as crate::storage::LockingPolicyFactory>::new_shared(
                pnl_leases,
            );
        let active_holdings_mutations = storage_builder.create_shared::<AccountId, usize>();
        #[cfg(test)]
        let pnl_lease_wait_hooks =
            <Sync::StorageLockingPolicyFactory as crate::storage::LockingPolicyFactory>::new_shared(
                storage_builder
                    .create_for_bound_key::<AccountId, Option<std::sync::Arc<std::sync::Barrier>>>(
                    ),
            );
        #[cfg(test)]
        let pre_trade_reservation_hooks =
            <Sync::StorageLockingPolicyFactory as crate::storage::LockingPolicyFactory>::new_shared(
                storage_builder.create_for_bound_key::<AccountId, Option<(
                    std::sync::Arc<std::sync::Barrier>,
                    std::sync::Arc<std::sync::Barrier>,
                )>>(),
            );
        Self {
            holdings: <<Sync as SyncMode>::StorageLockingPolicyFactory
                as crate::storage::LockingPolicyFactory>::new_shared(
                HoldingsStore::new(storage_builder),
            ),
            settings: <Sync::StorageLockingPolicyFactory
                as LockingPolicyFactory>::new_config(settings),
            market_orders,
            pnl,
            active_holdings_mutations,
            pnl_leases,
            #[cfg(test)]
            pnl_lease_wait_hooks,
            #[cfg(test)]
            pre_trade_reservation_hooks,
            group_id: crate::pretrade::DEFAULT_POLICY_GROUP_ID,
        }
    }

    fn begin_holdings_mutation(
        &self,
        account_id: AccountId,
    ) -> ActiveHoldingsMutationGuard<Sync::StorageLockingPolicyFactory> {
        self.active_holdings_mutations.with_mut(
            account_id,
            || 0,
            |count, _| *count = count.saturating_add(1),
        );
        ActiveHoldingsMutationGuard {
            active: self.active_holdings_mutations.clone(),
            account_id,
        }
    }

    /// Builds the canonical account P&L kill-switch preset.
    ///
    /// The preset keeps the policy boundary intact while centralizing the
    /// configuration shared by language bindings. It uses mark pricing with
    /// zero slippage and no overrides, and sets the funds limit mode to
    /// [`SpotFundsLimitMode::TrackOnly`]. Track-only mode disables
    /// insufficient-funds rejects while the policy continues to reconcile
    /// holdings and account P&L.
    ///
    /// At least one global, account-group, or account barrier must be
    /// configured. Use [`Self::with_policy_group_id`] on the returned policy to
    /// assign a non-default policy group.
    ///
    /// With a known effective account currency, an account-tier mismatch fails
    /// closed and blocks the account; account-group and global mismatches are
    /// skipped. A known-currency account with no account barrier and no
    /// matching fallback has no effective barrier, but its P&L keeps
    /// accumulating and publishing. Without an effective currency, the first
    /// in-scope barrier applies. Bounds are compared as stored and are never
    /// FX-converted.
    ///
    /// # Errors
    ///
    /// Returns [`SpotFundsConfigError`] when a barrier is invalid or when no
    /// barriers are configured.
    pub fn pnl_bounds_kill_switch<AccountGroupBarriers, AccountBarriers>(
        global_barrier: Option<SpotFundsPnlBoundsBarrier>,
        account_group_barriers: AccountGroupBarriers,
        account_barriers: AccountBarriers,
        market_orders: Option<SpotFundsMarketData<MarketDataSyncMode>>,
        storage_builder: &StorageBuilder<<Sync as SyncMode>::StorageLockingPolicyFactory>,
    ) -> Result<Self, SpotFundsConfigError>
    where
        AccountGroupBarriers: IntoIterator<Item = SpotFundsPnlBoundsAccountGroupBarrier>,
        AccountBarriers: IntoIterator<Item = SpotFundsPnlBoundsAccountBarrier>,
        <Sync as SyncMode>::StorageLockingPolicyFactory:
            CreateStorageFor<(AccountId, Asset)> + CreateStorageFor<AccountId>,
    {
        let mut settings = SpotFundsSettings::new(
            0,
            SpotFundsPricingSource::Mark,
            std::iter::empty::<(SpotFundsOverrideTarget, SpotFundsOverride)>(),
        )?;
        settings.set_global_limit_mode(SpotFundsLimitMode::TrackOnly);
        let settings =
            settings.with_pnl_barriers(global_barrier, account_group_barriers, account_barriers)?;
        Ok(Self::new(settings, market_orders, storage_builder))
    }

    pub(super) fn account_pnl_state(&self, account_id: AccountId) -> crate::PnlState {
        self.pnl
            .with(&account_id, |entry| entry.state)
            .unwrap_or(crate::PnlState::Value(Pnl::ZERO))
    }

    /// Takes the per-account PnL lease, re-entrant for the same `owner_id`.
    ///
    /// The wait is deliberately unbounded and has no timeout: the lease is a
    /// lock, and its holder always releases it - every guard is owned by a live
    /// stack frame or by [`Mutations`], which release it on commit, rollback or
    /// drop, including while unwinding. There is no correct action on expiry
    /// either, since proceeding without the lease would let two absolute
    /// assertions interleave, so a timeout could only trade a stall for a wrong
    /// account PnL.
    ///
    /// Progress therefore relies on the holder running on another thread. A
    /// lease held by this very thread can never be released while this call
    /// spins, so that case is a self-deadlock and is asserted against rather
    /// than waited on. Single-threaded modes
    /// ([`LocalSync`](crate::core::LocalSync), `NoLocking`) have no other
    /// thread at all, which makes any wait there exactly this assertion.
    pub(super) fn acquire_account_pnl_lease(
        &self,
        account_id: AccountId,
        owner_id: u64,
    ) -> AccountPnlLeaseGuard<Sync::StorageLockingPolicyFactory> {
        let mut attempts = 0_u32;
        loop {
            let acquired = self.pnl_leases.with_mut(
                account_id,
                || None,
                |lease, _| match lease {
                    Some(existing) if existing.owner_id == owner_id => {
                        existing.depth += 1;
                        true
                    }
                    Some(existing) => {
                        #[cfg(debug_assertions)]
                        debug_assert!(
                            existing.owner_thread != std::thread::current().id(),
                            "account PnL lease is held by this very thread under \
                             another owner: waiting for it can never end"
                        );
                        let _ = existing;
                        false
                    }
                    None => {
                        *lease = Some(AccountPnlLease::new(owner_id));
                        true
                    }
                },
            );
            if acquired {
                return AccountPnlLeaseGuard {
                    leases: self.pnl_leases.clone(),
                    account_id,
                    owner_id,
                };
            }
            #[cfg(test)]
            {
                let hook =
                    self.pnl_lease_wait_hooks
                        .with_mut(account_id, || None, |slot, _| slot.take());
                if let Some(hook) = hook {
                    hook.wait();
                }
            }
            if attempts < 8 {
                std::thread::yield_now();
            } else {
                std::thread::sleep(std::time::Duration::from_micros(50));
            }
            attempts = attempts.saturating_add(1);
        }
    }

    #[cfg(test)]
    pub(super) fn set_account_pnl_lease_wait_hook(
        &self,
        account_id: AccountId,
        hook: std::sync::Arc<std::sync::Barrier>,
    ) {
        self.pnl_lease_wait_hooks
            .with_mut(account_id, || None, |slot, _| *slot = Some(hook));
    }

    #[cfg(test)]
    pub(super) fn set_pre_trade_reservation_hook(
        &self,
        account_id: AccountId,
        entered: std::sync::Arc<std::sync::Barrier>,
        release: std::sync::Arc<std::sync::Barrier>,
    ) {
        self.pre_trade_reservation_hooks.with_mut(
            account_id,
            || None,
            |slot, _| *slot = Some((entered, release)),
        );
    }

    #[cfg(test)]
    pub(super) fn wait_pre_trade_reservation_hook(&self, account_id: AccountId) {
        let hook =
            self.pre_trade_reservation_hooks
                .with_mut(account_id, || None, |slot, _| slot.take());
        if let Some((entered, release)) = hook {
            entered.wait();
            release.wait();
        }
    }

    #[cfg(test)]
    pub(super) fn acquire_account_pnl_assertion(
        &self,
        account_id: AccountId,
        owner_id: u64,
        state: crate::PnlState,
    ) -> (
        AccountPnlEntry,
        u64,
        AccountPnlLeaseGuard<Sync::StorageLockingPolicyFactory>,
    ) {
        let lease = self.acquire_account_pnl_lease(account_id, owner_id);
        let token = crate::core::mutation::next_mutation_owner_id();
        let previous = self
            .pnl
            .with_mut(account_id, AccountPnlEntry::zero, |entry, _| {
                let previous = *entry;
                *entry = AccountPnlEntry {
                    state,
                    assertion_token: Some(token),
                };
                previous
            });
        (previous, token, lease)
    }

    pub(super) fn set_account_pnl_state(
        &self,
        account_id: AccountId,
        state: crate::PnlState,
    ) -> Option<crate::PnlState> {
        let owner_id = crate::core::mutation::next_mutation_owner_id();
        let _lease = self.acquire_account_pnl_lease(account_id, owner_id);
        self.pnl
            .with_mut(account_id, AccountPnlEntry::zero, |entry, is_new| {
                let previous = (!is_new).then_some(entry.state);
                *entry = AccountPnlEntry {
                    state,
                    assertion_token: None,
                };
                previous
            })
    }

    /// Reads the policy group tag.
    pub(super) fn group_id(&self) -> PolicyGroupId {
        self.group_id
    }

    /// Assigns a group tag to this policy instance.
    ///
    /// The tag is fixed at construction and has no runtime setter. See
    /// [`PolicyGroupId`]
    /// and [`DEFAULT_POLICY_GROUP_ID`](crate::pretrade::DEFAULT_POLICY_GROUP_ID)
    /// for details.
    pub fn with_policy_group_id(mut self, id: PolicyGroupId) -> Self {
        self.group_id = id;
        self
    }
}

impl<Sync, MarketDataSyncMode> PolicyName for SpotFundsPolicy<Sync, MarketDataSyncMode>
where
    Sync: SyncMode,
    Sync::StorageLockingPolicyFactory: LockingPolicyFactory,
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
        + HasExecutionReportFillFee
        + HasRemainingReservedQuantity
        + HasExecutionReportIsFinal
        + HasPreTradeLock,
    AccountAdjustment: HasBalanceAsset
        + HasAccountAdjustmentBalance
        + HasAccountAdjustmentBalanceAverageEntryPrice
        + HasAccountAdjustmentBalanceLowerBound
        + HasAccountAdjustmentBalanceUpperBound
        + HasAccountAdjustmentHeld
        + HasAccountAdjustmentHeldLowerBound
        + HasAccountAdjustmentHeldUpperBound
        + HasAccountAdjustmentIncoming
        + HasAccountAdjustmentIncomingLowerBound
        + HasAccountAdjustmentIncomingUpperBound
        + HasAccountAdjustmentPnlOperation,
    Sync: SyncMode,
    Sync::StorageLockingPolicyFactory: LockingPolicyFactory,
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
    ) -> Option<crate::core::ConfigEntry<<Sync as SyncMode>::StorageLockingPolicyFactory>> {
        Some(crate::core::ConfigEntry::SpotFunds {
            settings: crate::pretrade::ConfigurablePolicy::settings_cell(self),
            transition: parking_lot::Mutex::new(()),
            pnl: self.pnl.clone(),
            holdings: self.holdings.clone(),
            active_holdings_mutations: self.active_holdings_mutations.clone(),
        })
    }

    fn apply_runtime_configuration(
        &self,
        configuration: PolicyRuntimeConfiguration,
    ) -> PolicyConfigurationResult {
        match configuration {
            PolicyRuntimeConfiguration::SetSpotFundsAccountPnl {
                account_id,
                account_group_id,
                account_currency,
                state,
            } => {
                self.set_account_pnl_state(account_id, state);
                let account_blocks = self
                    .pnl_barrier_for(account_id, account_group_id, account_currency.as_ref())
                    .as_ref()
                    .and_then(|barrier| {
                        rejects::account_pnl_block_for_state(
                            account_currency.as_ref(),
                            state,
                            barrier,
                            None,
                        )
                    })
                    .into_iter()
                    .collect();
                PolicyConfigurationResult { account_blocks }
            }
        }
    }

    /// Applies an account adjustment to the policy's holdings.
    ///
    /// When a field is specified in the adjustment its outcome is always
    /// emitted in the returned
    /// [`AccountOutcomeEntry`](crate::AccountOutcomeEntry), even if the
    /// resulting delta is zero. This differs from
    /// [`Self::apply_execution_report`], which omits zero-delta entries.
    fn apply_account_adjustment(
        &self,
        ctx: &AccountAdjustmentContext<<Sync as SyncMode>::StorageLockingPolicyFactory>,
        account_id: AccountId,
        adjustment: &AccountAdjustment,
        mutations: &mut Mutations,
    ) -> Result<PolicyAccountAdjustmentResult, Rejects> {
        self.apply_account_adjustment_impl(
            ctx,
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
        ctx: &crate::pretrade::PostTradeContext<
            <Sync as crate::core::SyncMode>::StorageLockingPolicyFactory,
        >,
        report: &ExecutionReport,
    ) -> Option<PostTradeResult> {
        let request = match self.read_execution_request(report) {
            Ok(request) => request,
            Err(block) => return Some(PostTradeResult::blocks_only(vec![block])),
        };
        ctx.with_state_writer(|| self.apply_execution_request_impl(ctx, request))
    }

    fn perform_pre_trade_check(
        &self,
        ctx: &PreTradeContext<<Sync as SyncMode>::StorageLockingPolicyFactory>,
        order: &Order,
        mutations: &mut Mutations,
    ) -> Result<Option<PolicyPreTradeResult>, Rejects> {
        self.perform_pre_trade_check_impl(ctx, order, mutations)
    }

    /// Dry-run main-stage check.
    ///
    /// The normal [`perform_pre_trade_check`](Self::perform_pre_trade_check)
    /// applies the hold to storage immediately before registering its
    /// delta-based rollback, so it is an immediate-side-effect policy. This
    /// override emulates the same verdict, lock prices, and outcome entries
    /// read-only: it touches no storage and pushes nothing to `mutations`, so a
    /// dry-run never moves engine state.
    fn perform_pre_trade_check_dry_run(
        &self,
        ctx: &PreTradeContext<<Sync as SyncMode>::StorageLockingPolicyFactory>,
        order: &Order,
        _mutations: &mut Mutations,
    ) -> Result<Option<PolicyPreTradeResult>, Rejects> {
        self.perform_pre_trade_check_dry_run_impl(ctx, order)
    }
}

impl<Sync, MarketDataSyncMode> ConfigurablePolicy<<Sync as SyncMode>::StorageLockingPolicyFactory>
    for SpotFundsPolicy<Sync, MarketDataSyncMode>
where
    Sync: SyncMode,
    Sync::StorageLockingPolicyFactory: LockingPolicyFactory,
    MarketDataSyncMode: MarketDataSync,
{
    type Settings = SpotFundsSettings;

    fn settings_cell(
        &self,
    ) -> <Sync::StorageLockingPolicyFactory as LockingPolicyFactory>::Config<SpotFundsSettings>
    {
        self.settings.clone()
    }
}

#[cfg(test)]
mod lease_tests {
    use std::panic::{catch_unwind, AssertUnwindSafe};

    use crate::core::mutation::MutationRollbackResult;
    use crate::storage::{LockingPolicyFactory, NoLocking, StorageBuilder};
    use crate::{Mutation, Mutations};

    use super::{AccountPnlLease, AccountPnlLeaseGuard, AccountPnlLeaseStorage};

    fn lease_storage() -> AccountPnlLeaseStorage<NoLocking> {
        let builder = StorageBuilder::new(NoLocking);
        let storage =
            builder.create_for_bound_key::<crate::param::AccountId, Option<AccountPnlLease>>();
        <NoLocking as LockingPolicyFactory>::new_shared(storage)
    }

    fn acquire_test_lease(
        leases: &AccountPnlLeaseStorage<NoLocking>,
        account_id: crate::param::AccountId,
        owner_id: u64,
    ) -> Option<AccountPnlLeaseGuard<NoLocking>> {
        let acquired = leases.with_mut(
            account_id,
            || None,
            |lease, _| {
                if lease.is_none() {
                    *lease = Some(AccountPnlLease::new(owner_id));
                    true
                } else {
                    false
                }
            },
        );
        acquired.then(|| AccountPnlLeaseGuard {
            leases: leases.clone(),
            account_id,
            owner_id,
        })
    }

    #[test]
    fn unresolved_mutations_drop_releases_account_pnl_lease() {
        let leases = lease_storage();
        let account_id = crate::param::AccountId::from_u64(1);
        let guard = acquire_test_lease(&leases, account_id, 7)
            .expect("initial account PnL lease must be acquired");
        let mut mutations = Mutations::new();
        mutations.push(Mutation::new_reporting_with_guard(
            || true,
            MutationRollbackResult::default,
            guard,
        ));

        drop(mutations);

        let subsequent = acquire_test_lease(&leases, account_id, 8)
            .expect("dropping unresolved mutations must release the lease");
        drop(subsequent);
    }

    #[test]
    fn account_pnl_lease_releases_when_rollback_unwinds() {
        let leases = lease_storage();
        let account_id = crate::param::AccountId::from_u64(1);
        let owner_id = 7;
        let guard = acquire_test_lease(&leases, account_id, owner_id)
            .expect("account PnL lease must be acquired");
        let mut mutations = Mutations::new();
        mutations.push(Mutation::new_reporting_with_guard(
            || true,
            || -> MutationRollbackResult { panic!("rollback action panic") },
            guard,
        ));

        let unwind = catch_unwind(AssertUnwindSafe(|| {
            let _ = mutations.rollback_all();
        }));

        assert!(unwind.is_err());
        assert!(leases
            .with(&account_id, Option::is_none)
            .expect("lease slot must remain present"));
    }

    #[test]
    fn account_pnl_lease_releases_when_commit_unwinds() {
        let leases = lease_storage();
        let account_id = crate::param::AccountId::from_u64(1);
        let guard =
            acquire_test_lease(&leases, account_id, 7).expect("account PnL lease must be acquired");
        let mut mutations = Mutations::new();
        mutations.push(Mutation::new_reporting_with_guard(
            || panic!("commit action panic"),
            MutationRollbackResult::default,
            guard,
        ));

        let unwind = catch_unwind(AssertUnwindSafe(|| mutations.commit_all()));

        assert!(unwind.is_err());
        assert!(leases
            .with(&account_id, Option::is_none)
            .expect("lease slot must remain present"));
    }
}
