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

use std::collections::HashMap;
use std::fmt::{Display, Formatter};

use crate::core::{HasAccountId, HasFee, HasInstrument, HasPnl};
use crate::param::{AccountId, Asset, Pnl};
use crate::pretrade::policy::{
    missing_required_field_account_block, missing_required_field_reject, PolicyGroupId, PolicyName,
};
use crate::pretrade::DEFAULT_POLICY_GROUP_ID;
use crate::pretrade::{
    AccountBlock, PostTradeResult, PreTradeContext, PreTradePolicy, Reject, RejectCode,
    RejectScope, Rejects,
};
use crate::storage::{ConfigCell, Storage, StorageBuilder};

/// Per-settlement P&L bounds configuration for the broker barrier.
///
/// Defines the loss/profit limits for a settlement asset, applied as a broker
/// barrier across all accounts. Use [`PnlBoundsAccountAssetBarrier`] to add tighter
/// per-account+asset bounds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PnlBoundsBrokerBarrier {
    /// Settlement asset whose accumulated P&L is being monitored.
    pub settlement_asset: Asset,
    /// Optional lower bound.
    ///
    /// `lower_bound` is typically negative; it represents the loss limit.
    pub lower_bound: Option<Pnl>,
    /// Optional upper bound.
    ///
    /// `upper_bound` is typically positive; it represents the profit-taking
    /// limit.
    pub upper_bound: Option<Pnl>,
}

/// Per-(account, settlement-asset) P&L bounds refinement.
///
/// Pairs a [`PnlBoundsBrokerBarrier`] (the settlement and bounds configuration) with
/// an account identity and a starting P&L. Both the broker barrier (per
/// settlement asset) and this account+asset barrier are evaluated on every
/// check; the order passes only if neither is breached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PnlBoundsAccountAssetBarrier {
    /// Settlement asset and bounds for this account+asset barrier.
    pub barrier: PnlBoundsBrokerBarrier,
    /// Account this barrier applies to.
    pub account_id: AccountId,
    /// Starting accumulated P&L for the account, consumed at construction only.
    ///
    /// Seeds P&L accrued before the engine started.
    pub initial_pnl: Pnl,
}

/// Runtime replacement for a per-(account, settlement-asset) P&L barrier.
///
/// Runtime updates cannot seed or reset accumulated P&L. The live accumulator
/// is preserved and evaluated against the replacement barrier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PnlBoundsAccountAssetBarrierUpdate {
    /// Settlement asset and replacement bounds for this account.
    pub barrier: PnlBoundsBrokerBarrier,
    /// Account this replacement barrier applies to.
    pub account_id: AccountId,
}

/// Errors returned by [`PnlBoundsKillSwitchPolicy`] and
/// [`PnlBoundsKillSwitchSettings`] operations.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PnlBoundsKillSwitchPolicyError {
    /// Neither broker nor account+asset barriers were provided to the constructor.
    NoBarriersConfigured,
    /// Both lower and upper bounds are omitted for one settlement asset.
    NoBoundsConfigured { settlement_asset: Asset },
}

impl Display for PnlBoundsKillSwitchPolicyError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoBarriersConfigured => write!(
                f,
                "at least one broker or account+asset barrier must be configured"
            ),
            Self::NoBoundsConfigured { settlement_asset } => write!(
                f,
                "at least one of lower_bound or upper_bound must be configured \
                 for settlement asset {settlement_asset}"
            ),
        }
    }
}

impl std::error::Error for PnlBoundsKillSwitchPolicyError {}

/// Runtime-updatable settings for [`PnlBoundsKillSwitchPolicy`].
///
/// Holds the barrier configuration that can be replaced at runtime via
/// [`crate::storage::ConfigCell::update`]. The policy reads this on every hot
/// path via its cell; `realized` P&L storage is held outside the cell and is
/// never touched by a settings update.
///
/// `group_id` is set at construction time and cannot be changed via a settings
/// update; it is part of the settings so that the engine can distribute a single
/// clone of the cell while the group tag travels with the settings snapshot.
///
/// # Realized P&L accumulates independently of barrier configuration
///
/// Realized P&L (`pnl + fee` from each execution report) is tracked for
/// **every** `(account, settlement asset)` pair regardless of whether a barrier
/// exists for that pair at the time of the report. When a barrier is added at
/// runtime, the live accumulator is already authoritative. The deliberate
/// trade-off is that the accumulator retains an entry for every pair ever
/// traded.
///
/// # `initial_pnl` is construction-only
///
/// The `initial_pnl` field on each [`PnlBoundsAccountAssetBarrier`] is consumed
/// only once at [`PnlBoundsKillSwitchPolicy::new`] to seed the realized P&L
/// storage. Runtime account-barrier replacement accepts only
/// [`PnlBoundsAccountAssetBarrierUpdate`], which cannot carry a seed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PnlBoundsKillSwitchSettings {
    account_barriers: HashMap<AccountId, HashMap<Asset, PnlBoundsAccountAssetBarrierUpdate>>,
    broker_barriers: HashMap<Asset, PnlBoundsBrokerBarrier>,
    initial_pnl: HashMap<(AccountId, Asset), Pnl>,
    group_id: PolicyGroupId,
}

impl PnlBoundsKillSwitchSettings {
    /// Creates validated settings from raw barrier iterables.
    ///
    /// At least one barrier must appear across `broker_barriers` and
    /// `account_barriers`. Each barrier must have at least one bound
    /// configured; `group_id` defaults to [`DEFAULT_POLICY_GROUP_ID`].
    pub fn new(
        broker_barriers: impl IntoIterator<Item = PnlBoundsBrokerBarrier>,
        account_barriers: impl IntoIterator<Item = PnlBoundsAccountAssetBarrier>,
    ) -> Result<Self, PnlBoundsKillSwitchPolicyError> {
        let mut broker = HashMap::new();
        for barrier in broker_barriers {
            validate_bounds(
                &barrier.lower_bound,
                &barrier.upper_bound,
                &barrier.settlement_asset,
            )?;
            broker.insert(barrier.settlement_asset.clone(), barrier);
        }

        let mut account = HashMap::new();
        let mut initial_pnl = HashMap::new();
        for barrier in account_barriers {
            validate_bounds(
                &barrier.barrier.lower_bound,
                &barrier.barrier.upper_bound,
                &barrier.barrier.settlement_asset,
            )?;
            let account_id = barrier.account_id;
            let settlement_asset = barrier.barrier.settlement_asset.clone();
            initial_pnl.insert((account_id, settlement_asset.clone()), barrier.initial_pnl);
            account
                .entry(account_id)
                .or_insert_with(HashMap::new)
                .insert(
                    settlement_asset,
                    PnlBoundsAccountAssetBarrierUpdate {
                        barrier: barrier.barrier,
                        account_id,
                    },
                );
        }

        if broker.is_empty() && account.is_empty() {
            return Err(PnlBoundsKillSwitchPolicyError::NoBarriersConfigured);
        }

        Ok(Self {
            account_barriers: account,
            broker_barriers: broker,
            initial_pnl,
            group_id: DEFAULT_POLICY_GROUP_ID,
        })
    }

    /// Replaces the broker barriers.
    ///
    /// Validates every barrier before applying; returns an error (leaving
    /// settings unchanged) if any barrier has neither bound set or if the
    /// resulting combined set would be empty.
    pub fn set_broker_barriers(
        &mut self,
        barriers: impl IntoIterator<Item = PnlBoundsBrokerBarrier>,
    ) -> Result<(), PnlBoundsKillSwitchPolicyError> {
        let mut broker = HashMap::new();
        for barrier in barriers {
            validate_bounds(
                &barrier.lower_bound,
                &barrier.upper_bound,
                &barrier.settlement_asset,
            )?;
            broker.insert(barrier.settlement_asset.clone(), barrier);
        }
        if broker.is_empty() && self.account_barriers.is_empty() {
            return Err(PnlBoundsKillSwitchPolicyError::NoBarriersConfigured);
        }
        self.broker_barriers = broker;
        Ok(())
    }

    /// Replaces the account+asset barriers without changing accumulated P&L.
    ///
    /// Validates every barrier before applying; returns an error (leaving
    /// settings unchanged) if any barrier has neither bound set or if the
    /// resulting combined set would be empty. The update DTO has no
    /// `initial_pnl`: runtime replacement always evaluates the live
    /// accumulator.
    pub fn set_account_barriers(
        &mut self,
        barriers: impl IntoIterator<Item = PnlBoundsAccountAssetBarrierUpdate>,
    ) -> Result<(), PnlBoundsKillSwitchPolicyError> {
        let mut account = HashMap::new();
        for barrier in barriers {
            validate_bounds(
                &barrier.barrier.lower_bound,
                &barrier.barrier.upper_bound,
                &barrier.barrier.settlement_asset,
            )?;
            account
                .entry(barrier.account_id)
                .or_insert_with(HashMap::new)
                .insert(barrier.barrier.settlement_asset.clone(), barrier);
        }
        if self.broker_barriers.is_empty() && account.is_empty() {
            return Err(PnlBoundsKillSwitchPolicyError::NoBarriersConfigured);
        }
        self.account_barriers = account;
        Ok(())
    }
}

/// Shared handle to the per-`(account, settlement asset)` realized P&L ledger.
///
/// The policy and the runtime configurator hold clones of the same handle, so
/// a force-set writes through the same ledger the hot path reads. The `Shared`
/// wrapper is `Arc`/`Rc` per sync mode; its `Deref` is free, so the hot path
/// pays no extra cost.
pub(crate) type RealizedPnlStorage<LockingPolicyFactory> =
    <LockingPolicyFactory as crate::storage::LockingPolicyFactory>::Shared<
        Storage<
            (AccountId, Asset),
            Pnl,
            <LockingPolicyFactory as crate::storage::LockingPolicyFactory>::Policy,
        >,
    >;

/// P&L kill switch with per-settlement-asset bounds.
///
/// Accumulates realized P&L (`pnl + fee` from execution reports) per
/// `(account, settlement asset)` and blocks the account when the running
/// total crosses a configured lower or upper bound.
///
/// Two barrier kinds can be configured, independently or together:
/// - **broker barrier** - one set of bounds per settlement asset, applied to
///   every account trading that asset;
/// - **account+asset barrier** - tighter bounds for a specific
///   `(account, settlement asset)` pair, with an explicit starting P&L.
///
/// An order is blocked when the realized P&L on its settlement asset is
/// outside **either** applicable barrier. If neither barrier covers the
/// `(account, settlement)` pair, the order passes.
///
/// Barriers are read from a [`crate::storage::ConfigCell`] on every hot-path
/// call, so they can be replaced at runtime via
/// [`crate::storage::ConfigCell::update`] without restarting the engine.
/// Accumulated realized P&L lives outside the cell and is never reset by a
/// settings update.
///
/// Constructor rules:
/// - at least one barrier (broker or account+asset) must be configured;
/// - at least one of `lower_bound` or `upper_bound` must be configured for
///   each barrier;
/// - constructor does not validate signs of bounds;
/// - constructor does not validate ordering (`lower_bound <= upper_bound`).
pub struct PnlBoundsKillSwitchPolicy<LockingPolicyFactory>
where
    LockingPolicyFactory: crate::storage::LockingPolicyFactory,
{
    settings: LockingPolicyFactory::Config<PnlBoundsKillSwitchSettings>,
    realized: RealizedPnlStorage<LockingPolicyFactory>,
}

impl<LockingPolicyFactory> PnlBoundsKillSwitchPolicy<LockingPolicyFactory>
where
    LockingPolicyFactory: crate::storage::LockingPolicyFactory,
{
    /// Stable policy name.
    pub const NAME: &'static str = "PnlBoundsKillSwitchPolicy";

    /// Creates a P&L bounds kill-switch policy.
    ///
    /// `settings` carries the validated barrier configuration and the group
    /// tag. `storage_builder` must be obtained from the engine builder so that
    /// the per-account P&L storage shares the factory type with the engine's
    /// synchronization mode.
    ///
    /// The `initial_pnl` supplied through each construction-time
    /// [`PnlBoundsAccountAssetBarrier`] is consumed once here to seed the
    /// realized P&L storage. Runtime barrier updates cannot carry a seed and
    /// therefore cannot reset accumulated P&L.
    pub fn new(
        mut settings: PnlBoundsKillSwitchSettings,
        storage_builder: &StorageBuilder<LockingPolicyFactory>,
    ) -> Self
    where
        LockingPolicyFactory: crate::storage::CreateStorageFor<(AccountId, Asset)>,
    {
        let realized = storage_builder.create_for_bound_key();
        let initial_pnl = std::mem::take(&mut settings.initial_pnl);

        for ((account_id, settlement_asset), initial_pnl) in initial_pnl {
            realized.with_mut(
                (account_id, settlement_asset),
                || Pnl::ZERO,
                |entry, _is_new| {
                    *entry = initial_pnl;
                },
            );
        }

        // Seed first, then share: the configurator and the hot path observe the
        // same ledger through this handle.
        let realized = LockingPolicyFactory::new_shared(realized);

        Self {
            settings: LockingPolicyFactory::new_config(settings),
            realized,
        }
    }

    /// Assigns a group tag to this policy instance.
    ///
    /// Updates the group ID inside the settings cell. See [`PolicyGroupId`]
    /// and [`DEFAULT_POLICY_GROUP_ID`] for details.
    pub fn with_policy_group_id(self, id: PolicyGroupId) -> Self {
        self.settings
            .update::<std::convert::Infallible>(|s| {
                s.group_id = id;
                Ok(())
            })
            .unwrap_or_else(|e| match e {});
        self
    }
}

impl<LockingPolicyFactory> PolicyName for PnlBoundsKillSwitchPolicy<LockingPolicyFactory>
where
    LockingPolicyFactory: crate::storage::LockingPolicyFactory,
{
    fn policy_name(&self) -> &str {
        Self::NAME
    }
}

impl<LockingPolicyFactory> crate::pretrade::ConfigurablePolicy<LockingPolicyFactory>
    for PnlBoundsKillSwitchPolicy<LockingPolicyFactory>
where
    LockingPolicyFactory: crate::storage::LockingPolicyFactory,
{
    type Settings = PnlBoundsKillSwitchSettings;

    /// Returns a clone of the policy's own settings cell.
    ///
    /// Both the returned clone and the policy's internal field point at the
    /// same underlying value. The engine can publish a new settings value
    /// through the registry clone and the running policy will observe it on
    /// its next hot-path read.
    fn settings_cell(&self) -> LockingPolicyFactory::Config<PnlBoundsKillSwitchSettings> {
        self.settings.clone()
    }
}

impl<Order, ExecutionReport, AccountAdjustment, LockingPolicyFactory, Sync>
    PreTradePolicy<Order, ExecutionReport, AccountAdjustment, Sync>
    for PnlBoundsKillSwitchPolicy<LockingPolicyFactory>
where
    Order: HasInstrument + HasAccountId,
    ExecutionReport: HasInstrument + HasPnl + HasFee + HasAccountId,
    LockingPolicyFactory:
        crate::storage::LockingPolicyFactory + crate::storage::CreateStorageFor<AccountId>,
    Sync: crate::core::SyncMode<StorageLockingPolicyFactory = LockingPolicyFactory>,
{
    fn name(&self) -> &str {
        Self::NAME
    }

    fn policy_group_id(&self) -> PolicyGroupId {
        self.settings.with(|s| s.group_id)
    }

    #[allow(private_interfaces)]
    fn built_in_config_entry(
        &self,
    ) -> Option<
        crate::core::ConfigEntry<<Sync as crate::core::SyncMode>::StorageLockingPolicyFactory>,
    > {
        Some(crate::core::ConfigEntry::PnlBoundsKillSwitch {
            settings: crate::pretrade::ConfigurablePolicy::settings_cell(self),
            realized: self.realized.clone(),
        })
    }

    fn check_pre_trade_start(
        &self,
        _ctx: &PreTradeContext<<Sync as crate::core::SyncMode>::StorageLockingPolicyFactory>,
        order: &Order,
    ) -> Result<(), Rejects> {
        let instrument = order
            .instrument()
            .map_err(|e| Rejects::from(missing_required_field_reject(self, "instrument", &e)))?;
        let account_id = order
            .account_id()
            .map_err(|e| Rejects::from(missing_required_field_reject(self, "account ID", &e)))?;

        let settlement = instrument.settlement_asset();

        // Read barriers from the cell; no allocation on the hot path.
        let (broker_breach, account_breach) = self.settings.with(|s| {
            let broker_barrier = s.broker_barriers.get(settlement);
            let account_barrier = s
                .account_barriers
                .get(&account_id)
                .and_then(|m| m.get(settlement));

            if broker_barrier.is_none() && account_barrier.is_none() {
                return (None, None);
            }

            let current_pnl = self
                .realized
                .with(&(account_id, settlement.clone()), |entry| *entry)
                .unwrap_or(Pnl::ZERO);

            let bb = broker_barrier.and_then(|b| {
                let sides = breached_sides(b.lower_bound, b.upper_bound, current_pnl);
                if sides.is_empty() {
                    None
                } else {
                    Some(barrier_breach_reject(
                        Self::NAME,
                        "pnl kill switch triggered: broker barrier",
                        &sides,
                        b,
                        current_pnl,
                        account_id,
                    ))
                }
            });

            let ab = account_barrier.and_then(|a| {
                let sides =
                    breached_sides(a.barrier.lower_bound, a.barrier.upper_bound, current_pnl);
                if sides.is_empty() {
                    None
                } else {
                    Some(barrier_breach_reject(
                        Self::NAME,
                        "pnl kill switch triggered: account + asset barrier",
                        &sides,
                        &a.barrier,
                        current_pnl,
                        account_id,
                    ))
                }
            });

            (bb, ab)
        });

        if let Some(reject) = broker_breach {
            return Err(reject.into());
        }
        if let Some(reject) = account_breach {
            return Err(reject.into());
        }

        Ok(())
    }

    /// Applies a post-trade report to the accumulated realized P&L for the
    /// report's `(account, settlement_asset)`.
    ///
    /// The report contract expects `pnl` plus explicit `fee`. Fee impact is
    /// added to `pnl` before accumulation.
    ///
    /// Accumulation is unconditional: realized P&L is tracked for every
    /// `(account, settlement asset)` whether or not a barrier is currently
    /// configured for that pair. This ensures that a barrier added at runtime
    /// sees the true accumulated P&L rather than a stale zero.
    ///
    /// The accumulation is performed under a single write lock; no intermediate
    /// reads from the realized storage are issued.
    ///
    /// Returns an [`AccountBlock`] when any required report field cannot be
    /// accessed, when `pnl + fee` or the accumulated value overflows, or when
    /// the new accumulated value breaches a configured barrier. Returns `None`
    /// when no barrier covers the pair and no overflow occurs.
    fn apply_execution_report(
        &self,
        _ctx: &crate::pretrade::PostTradeContext<
            <Sync as crate::core::SyncMode>::StorageLockingPolicyFactory,
        >,
        report: &ExecutionReport,
    ) -> Option<PostTradeResult> {
        let instrument = match report.instrument() {
            Ok(i) => i,
            Err(e) => {
                return Some(PostTradeResult::blocks_only(vec![
                    missing_required_field_account_block(self, "instrument", &e),
                ]))
            }
        };
        let account_id = match report.account_id() {
            Ok(id) => id,
            Err(e) => {
                return Some(PostTradeResult::blocks_only(vec![
                    missing_required_field_account_block(self, "account ID", &e),
                ]))
            }
        };
        let pnl_delta = match report.pnl() {
            Ok(p) => p,
            Err(e) => {
                return Some(PostTradeResult::blocks_only(vec![
                    missing_required_field_account_block(self, "P&L", &e),
                ]))
            }
        };
        let fee = match report.fee() {
            Ok(f) => f,
            Err(e) => {
                return Some(PostTradeResult::blocks_only(vec![
                    missing_required_field_account_block(self, "fee", &e),
                ]))
            }
        };

        let settlement = instrument.settlement_asset();

        let pnl_with_fee = match pnl_delta.checked_add(fee.to_pnl()) {
            Ok(v) => v,
            Err(_) => {
                return Some(PostTradeResult::blocks_only(vec![
                    pnl_calculation_failed_block(
                        self,
                        format!(
                            "pnl + fee overflow: pnl {pnl_delta}, fee {fee}, \
                         settlement asset {settlement}, account {account_id}"
                        ),
                    ),
                ]));
            }
        };

        let block: Option<AccountBlock> = self.realized.with_mut(
            (account_id, settlement.clone()),
            || Pnl::ZERO,
            |entry, _is_new| {
                let previous = *entry;
                let updated = match previous.checked_add(pnl_with_fee) {
                    Ok(value) => value,
                    Err(_) => {
                        return Some(pnl_calculation_failed_block(
                            self,
                            format!(
                                "realized pnl + pnl_with_fee overflow: \
                                 previous {previous}, increment {pnl_with_fee}, \
                                 settlement asset {settlement}, account {account_id}"
                            ),
                        ));
                    }
                };
                *entry = updated;

                // Read barriers from the cell to determine if the new total
                // breaches a boundary.
                let outside = self.settings.with(|s| {
                    is_outside_bounds(
                        &s.broker_barriers,
                        &s.account_barriers,
                        updated,
                        settlement,
                        account_id,
                    )
                });

                if outside {
                    Some(AccountBlock::new(
                        Self::NAME,
                        RejectCode::PnlKillSwitchTriggered,
                        "pnl kill switch triggered",
                        format!(
                            "realized pnl {updated}, settlement asset {settlement}, \
                             account {account_id}"
                        ),
                    ))
                } else {
                    None
                }
            },
        );

        block.map(|b| PostTradeResult::blocks_only(vec![b]))
    }
}

fn barrier_breach_reject(
    policy_name: &'static str,
    reason: &'static str,
    breached_sides: &[&'static str],
    barrier: &PnlBoundsBrokerBarrier,
    realized: Pnl,
    account_id: AccountId,
) -> Reject {
    let desc = breached_sides.join(" and ");
    let settlement = &barrier.settlement_asset;
    let lower_bound = barrier.lower_bound;
    let upper_bound = barrier.upper_bound;
    Reject::new(
        policy_name,
        RejectScope::Account,
        RejectCode::PnlKillSwitchTriggered,
        reason,
        format!(
            "{desc} bound breached: realized pnl {realized}, \
             lower_bound {lower_bound:?}, upper_bound {upper_bound:?}, \
             settlement asset {settlement}, account {account_id}"
        ),
    )
}

fn pnl_calculation_failed_block<Policy: PolicyName + ?Sized>(
    policy: &Policy,
    details: String,
) -> AccountBlock {
    AccountBlock::new(
        policy.policy_name(),
        RejectCode::OrderValueCalculationFailed,
        "pnl accumulation overflow",
        details,
    )
}

fn is_outside_bounds(
    broker_barriers: &HashMap<Asset, PnlBoundsBrokerBarrier>,
    account_barriers: &HashMap<AccountId, HashMap<Asset, PnlBoundsAccountAssetBarrierUpdate>>,
    pnl: Pnl,
    settlement: &Asset,
    account_id: AccountId,
) -> bool {
    if let Some(b) = broker_barriers.get(settlement) {
        if !breached_sides(b.lower_bound, b.upper_bound, pnl).is_empty() {
            return true;
        }
    }
    if let Some(b) = account_barriers
        .get(&account_id)
        .and_then(|m| m.get(settlement))
    {
        if !breached_sides(b.barrier.lower_bound, b.barrier.upper_bound, pnl).is_empty() {
            return true;
        }
    }
    false
}

/// Returns the breach side labels for a given realized P&L against bounds.
fn breached_sides(
    lower_bound: Option<Pnl>,
    upper_bound: Option<Pnl>,
    realized: Pnl,
) -> Vec<&'static str> {
    let mut sides = Vec::new();
    if let Some(lb) = lower_bound {
        if realized < lb {
            sides.push("lower");
        }
    }
    if let Some(ub) = upper_bound {
        if realized > ub {
            sides.push("upper");
        }
    }
    sides
}

fn validate_bounds(
    lower_bound: &Option<Pnl>,
    upper_bound: &Option<Pnl>,
    settlement_asset: &Asset,
) -> Result<(), PnlBoundsKillSwitchPolicyError> {
    if lower_bound.is_none() && upper_bound.is_none() {
        return Err(PnlBoundsKillSwitchPolicyError::NoBoundsConfigured {
            settlement_asset: settlement_asset.clone(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::core::{HasAccountId, HasFee, HasInstrument, HasPnl, Instrument, OrderOperation};
    use crate::param::TradeAmount;
    use crate::param::{AccountId, Asset, Fee, Pnl, Price, Quantity, Side};
    use crate::pretrade::{PreTradeContext, PreTradePolicy, RejectCode, RejectScope};
    use crate::storage::{ConfigCell, NoLocking};
    use crate::RequestFieldAccessError;

    use super::{
        PnlBoundsAccountAssetBarrier, PnlBoundsAccountAssetBarrierUpdate, PnlBoundsBrokerBarrier,
        PnlBoundsKillSwitchPolicy, PnlBoundsKillSwitchPolicyError, PnlBoundsKillSwitchSettings,
    };

    type TestPolicy = PnlBoundsKillSwitchPolicy<NoLocking>;

    fn test_builder() -> crate::SyncedEngineBuilder<OrderOperation, TestReport, (), crate::LocalSync>
    {
        crate::Engine::builder().no_sync()
    }

    struct TestReport {
        instrument: Instrument,
        account_id: AccountId,
        pnl: Pnl,
        fee: Fee,
    }

    impl HasInstrument for TestReport {
        fn instrument(&self) -> Result<&Instrument, crate::RequestFieldAccessError> {
            Ok(&self.instrument)
        }
    }

    impl HasAccountId for TestReport {
        fn account_id(&self) -> Result<AccountId, crate::RequestFieldAccessError> {
            Ok(self.account_id)
        }
    }

    impl HasPnl for TestReport {
        fn pnl(&self) -> Result<Pnl, crate::RequestFieldAccessError> {
            Ok(self.pnl)
        }
    }

    impl HasFee for TestReport {
        fn fee(&self) -> Result<Fee, crate::RequestFieldAccessError> {
            Ok(self.fee)
        }
    }

    // ── happy-path ──────────────────────────────────────────────────────────

    #[test]
    fn happy_path_order_passes_inside_bounds() {
        let policy = policy_usd(Some(pnl("-100")), Some(pnl("50")));
        apply_report(&policy, &report("USD", account(1), pnl("-10")));
        assert!(check_start(&policy, &order("USD", account(1))).is_ok());
    }

    #[test]
    fn different_accounts_track_pnl_independently() {
        let policy = policy_usd(Some(pnl("-100")), Some(pnl("50")));

        apply_report(&policy, &report("USD", account(1), pnl("40")));
        apply_report(&policy, &report("USD", account(2), pnl("-90")));

        assert!(check_start(&policy, &order("USD", account(1))).is_ok());
        assert!(check_start(&policy, &order("USD", account(2))).is_ok());

        let triggered = apply_report(&policy, &report("USD", account(1), pnl("15")));
        assert!(triggered); // 40+15=55 > upper bound 50
        assert!(check_start(&policy, &order("USD", account(1))).is_err());
        assert!(check_start(&policy, &order("USD", account(2))).is_ok());
    }

    // ── global bound breaches ───────────────────────────────────────────────

    // apply_execution_report detects a breach -> returns AccountBlock and the
    // stored realized P&L stays at the breaching value, so subsequent
    // check_pre_trade_start re-reports the breach.
    #[test]
    fn apply_breach_blocks_account_lower_bound() {
        let policy = policy_usd(Some(pnl("-100")), Some(pnl("50")));
        let blocks = apply_report_blocks(&policy, &report("USD", account(1), pnl("-101")));
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].code, RejectCode::PnlKillSwitchTriggered);
        assert_eq!(blocks[0].reason, "pnl kill switch triggered");

        let reject = check_start(&policy, &order("USD", account(1))).expect_err("must reject");
        let reject = &reject[0];
        assert_eq!(reject.scope, RejectScope::Account);
        assert_eq!(reject.code, RejectCode::PnlKillSwitchTriggered);
        assert_eq!(reject.reason, "pnl kill switch triggered: broker barrier");
    }

    #[test]
    fn apply_breach_blocks_account_upper_bound() {
        let policy = policy_usd(Some(pnl("-100")), Some(pnl("50")));
        let blocks = apply_report_blocks(&policy, &report("USD", account(1), pnl("51")));
        assert_eq!(blocks.len(), 1);

        let reject = check_start(&policy, &order("USD", account(1))).expect_err("must reject");
        assert_eq!(
            reject[0].reason,
            "pnl kill switch triggered: broker barrier"
        );
    }

    // check_pre_trade_start detects breach using the current stored P&L; the
    // reason always carries the specific barrier (broker / account+asset),
    // because the policy no longer maintains an internal "blocked" sentinel.
    #[test]
    fn check_detects_lower_bound_breach_with_specific_reason() {
        // Use initial_pnl to place PnL outside bounds before any apply.
        let settings = PnlBoundsKillSwitchSettings::new(
            [barrier_usd(Some(pnl("-100")), Some(pnl("50")))],
            [PnlBoundsAccountAssetBarrier {
                barrier: barrier("USD", Some(pnl("-500")), None),
                account_id: account(1),
                initial_pnl: pnl("-101"),
            }],
        )
        .expect("settings must be valid");
        let policy: TestPolicy =
            PnlBoundsKillSwitchPolicy::new(settings, test_builder().storage_builder());
        let reject = check_start(&policy, &order("USD", account(1))).expect_err("must reject");
        let reject = &reject[0];
        assert_eq!(reject.scope, RejectScope::Account);
        assert_eq!(reject.code, RejectCode::PnlKillSwitchTriggered);
        assert_eq!(reject.reason, "pnl kill switch triggered: broker barrier");
        assert!(reject.details.contains("lower bound breached"));
        // Second check: policy is stateless, returns the same breach reason.
        let reject2 = check_start(&policy, &order("USD", account(1))).expect_err("must reject");
        assert_eq!(
            reject2[0].reason,
            "pnl kill switch triggered: broker barrier"
        );
    }

    #[test]
    fn check_detects_upper_bound_breach_with_specific_reason() {
        let settings = PnlBoundsKillSwitchSettings::new(
            [barrier_usd(Some(pnl("-100")), Some(pnl("50")))],
            [PnlBoundsAccountAssetBarrier {
                barrier: barrier("USD", None, Some(pnl("500"))),
                account_id: account(1),
                initial_pnl: pnl("51"),
            }],
        )
        .expect("settings must be valid");
        let policy: TestPolicy =
            PnlBoundsKillSwitchPolicy::new(settings, test_builder().storage_builder());
        let reject = check_start(&policy, &order("USD", account(1))).expect_err("must reject");
        assert_eq!(
            reject[0].reason,
            "pnl kill switch triggered: broker barrier"
        );
        assert!(reject[0].details.contains("upper bound breached"));
    }

    #[test]
    fn inverted_bounds_breach_detected_at_check() {
        let settings = PnlBoundsKillSwitchSettings::new(
            [barrier_usd(Some(pnl("10")), Some(pnl("5")))],
            [PnlBoundsAccountAssetBarrier {
                barrier: barrier("USD", None, Some(pnl("500"))),
                account_id: account(1),
                initial_pnl: pnl("7"),
            }],
        )
        .expect("settings must be valid");
        let policy: TestPolicy =
            PnlBoundsKillSwitchPolicy::new(settings, test_builder().storage_builder());
        let reject = check_start(&policy, &order("USD", account(1))).expect_err("must reject");
        assert_eq!(reject[0].code, RejectCode::PnlKillSwitchTriggered);
        assert_eq!(
            reject[0].reason,
            "pnl kill switch triggered: broker barrier"
        );
        assert!(reject[0].details.contains("lower and upper bound breached"));
    }

    // ── account+asset barrier ───────────────────────────────────────────────

    #[test]
    fn account_barrier_initial_pnl_pre_loaded_into_storage() {
        let settings = PnlBoundsKillSwitchSettings::new(
            [barrier_usd(Some(pnl("-500")), None)],
            [PnlBoundsAccountAssetBarrier {
                barrier: barrier("USD", Some(pnl("-100")), None),
                account_id: account(1),
                initial_pnl: pnl("-90"),
            }],
        )
        .expect("settings must be valid");
        let policy: TestPolicy =
            PnlBoundsKillSwitchPolicy::new(settings, test_builder().storage_builder());

        // initial_pnl -90 is within account bounds [-100, ∞): passes before any report
        assert!(check_start(&policy, &order("USD", account(1))).is_ok());
        // account 2 has no account+asset barrier, starts at 0: passes broker
        assert!(check_start(&policy, &order("USD", account(2))).is_ok());
    }

    #[test]
    fn account_barrier_breach_detected_at_check_specific_reason() {
        // initial_pnl -200 violates account bound [-100] but NOT global [-500].
        let settings = PnlBoundsKillSwitchSettings::new(
            [barrier_usd(Some(pnl("-500")), None)],
            [PnlBoundsAccountAssetBarrier {
                barrier: barrier("USD", Some(pnl("-100")), None),
                account_id: account(1),
                initial_pnl: pnl("-200"),
            }],
        )
        .expect("settings must be valid");
        let policy: TestPolicy =
            PnlBoundsKillSwitchPolicy::new(settings, test_builder().storage_builder());

        let reject = check_start(&policy, &order("USD", account(1))).expect_err("must reject");
        let reject = &reject[0];
        assert_eq!(reject.code, RejectCode::PnlKillSwitchTriggered);
        assert_eq!(
            reject.reason,
            "pnl kill switch triggered: account + asset barrier"
        );
        assert!(reject.details.contains("lower bound breached"));
    }

    #[test]
    fn account_barrier_apply_breach_blocks() {
        let settings = PnlBoundsKillSwitchSettings::new(
            [barrier_usd(Some(pnl("-500")), None)],
            [PnlBoundsAccountAssetBarrier {
                barrier: barrier("USD", Some(pnl("-100")), None),
                account_id: account(1),
                initial_pnl: Pnl::ZERO,
            }],
        )
        .expect("settings must be valid");
        let policy: TestPolicy =
            PnlBoundsKillSwitchPolicy::new(settings, test_builder().storage_builder());

        // -200 violates account bound [-100], apply detects and blocks.
        let blocks = apply_report_blocks(&policy, &report("USD", account(1), pnl("-200")));
        assert_eq!(blocks.len(), 1);
        let reject = check_start(&policy, &order("USD", account(1))).expect_err("must reject");
        assert_eq!(
            reject[0].reason,
            "pnl kill switch triggered: account + asset barrier"
        );
    }

    #[test]
    fn global_barrier_breach_blocks_even_within_account_bounds() {
        let settings = PnlBoundsKillSwitchSettings::new(
            [barrier_usd(Some(pnl("-100")), None)],
            [PnlBoundsAccountAssetBarrier {
                barrier: barrier("USD", Some(pnl("-500")), None),
                account_id: account(1),
                initial_pnl: Pnl::ZERO,
            }],
        )
        .expect("settings must be valid");
        let policy: TestPolicy =
            PnlBoundsKillSwitchPolicy::new(settings, test_builder().storage_builder());

        // -200 is within account bound [-500] but violates global [-100].
        let blocks = apply_report_blocks(&policy, &report("USD", account(1), pnl("-200")));
        assert_eq!(blocks.len(), 1);
        let reject = check_start(&policy, &order("USD", account(1))).expect_err("must reject");
        assert_eq!(reject[0].code, RejectCode::PnlKillSwitchTriggered);
        assert_eq!(
            reject[0].reason,
            "pnl kill switch triggered: broker barrier"
        );
    }

    // ── constructor validation ──────────────────────────────────────────────

    #[test]
    fn no_barriers_configured_rejected_by_constructor() {
        let err = PnlBoundsKillSwitchSettings::new([], []).expect_err("must fail");
        assert_eq!(err, PnlBoundsKillSwitchPolicyError::NoBarriersConfigured);
        assert_eq!(
            err.to_string(),
            "at least one broker or account+asset barrier must be configured"
        );
    }

    #[test]
    fn missing_bounds_rejected_by_constructor() {
        let usd = Asset::new("USD").expect("asset code must be valid");
        let err = PnlBoundsKillSwitchSettings::new(
            [PnlBoundsBrokerBarrier {
                settlement_asset: usd.clone(),
                lower_bound: None,
                upper_bound: None,
            }],
            [],
        )
        .expect_err("must fail");

        assert_eq!(
            err,
            PnlBoundsKillSwitchPolicyError::NoBoundsConfigured {
                settlement_asset: usd,
            }
        );
    }

    #[test]
    fn missing_account_bounds_rejected_by_constructor() {
        let usd = Asset::new("USD").expect("must be valid");
        let err = PnlBoundsKillSwitchSettings::new(
            [barrier_usd(Some(pnl("-100")), None)],
            [PnlBoundsAccountAssetBarrier {
                barrier: PnlBoundsBrokerBarrier {
                    settlement_asset: usd.clone(),
                    lower_bound: None,
                    upper_bound: None,
                },
                account_id: account(1),
                initial_pnl: Pnl::ZERO,
            }],
        )
        .expect_err("must fail");

        assert_eq!(
            err,
            PnlBoundsKillSwitchPolicyError::NoBoundsConfigured {
                settlement_asset: usd
            }
        );
    }

    #[test]
    fn apply_execution_report_accumulates_pnl_and_reports_trigger() {
        let policy = policy_usd(Some(pnl("-100")), Some(pnl("50")));
        assert!(!apply_report(
            &policy,
            &report("USD", account(1), pnl("40"))
        )); // within bounds
        assert!(apply_report(&policy, &report("USD", account(1), pnl("15")))); // 55 > 50
    }

    // ── no-barrier-for-settlement ───────────────────────────────────────────

    #[test]
    fn no_barrier_for_settlement_order_passes() {
        let policy = policy_usd(Some(pnl("-100")), Some(pnl("50")));
        assert!(check_start(&policy, &order("EUR", account(1))).is_ok());
    }

    // ── multi-settlement isolation ──────────────────────────────────────────

    #[test]
    fn accumulation_and_breach_detection_independent_per_settlement() {
        let settings = PnlBoundsKillSwitchSettings::new(
            [
                barrier("USD", Some(pnl("-100")), Some(pnl("50"))),
                barrier("EUR", Some(pnl("-200")), Some(pnl("100"))),
            ],
            [
                PnlBoundsAccountAssetBarrier {
                    barrier: barrier("USD", Some(pnl("-50")), None),
                    account_id: account(1),
                    initial_pnl: Pnl::ZERO,
                },
                PnlBoundsAccountAssetBarrier {
                    barrier: barrier("EUR", Some(pnl("-100")), None),
                    account_id: account(1),
                    initial_pnl: Pnl::ZERO,
                },
            ],
        )
        .expect("settings must be valid");
        let policy: TestPolicy =
            PnlBoundsKillSwitchPolicy::new(settings, test_builder().storage_builder());

        apply_report(&policy, &report("USD", account(1), pnl("-30")));
        apply_report(&policy, &report("EUR", account(1), pnl("40")));

        // Both within their respective bounds.
        assert!(check_start(&policy, &order("USD", account(1))).is_ok());
        assert!(check_start(&policy, &order("EUR", account(1))).is_ok());

        // Breach USD account+asset barrier (-30 + -25 = -55 < account -50).
        apply_report(&policy, &report("USD", account(1), pnl("-25")));
        assert!(check_start(&policy, &order("USD", account(1))).is_err());
        // EUR remains unaffected by the USD breach.
        assert!(check_start(&policy, &order("EUR", account(1))).is_ok());
    }

    // ── repeated reports ─────────────────────────────────────────────────────

    // The policy is stateless about blocking: it reports a block on every
    // apply where the resulting accumulated P&L is outside bounds, and no
    // block when the running total falls back inside. The engine is the one
    // that latches the account-level block after the first AccountBlock.
    #[test]
    fn repeated_reports_track_running_total() {
        let policy = policy_usd(Some(pnl("-100")), Some(pnl("50")));

        let blocks = apply_report_blocks(&policy, &report("USD", account(1), pnl("-101")));
        assert_eq!(blocks.len(), 1);
        assert!(check_start(&policy, &order("USD", account(1))).is_err());

        // -101 + 5 = -96, which is back inside [-100, 50], so the policy
        // itself stops reporting a block. The engine remains responsible for
        // keeping the account latched.
        let blocks = apply_report_blocks(&policy, &report("USD", account(1), pnl("5")));
        assert!(blocks.is_empty());
        assert!(check_start(&policy, &order("USD", account(1))).is_ok());
    }

    // ── overflow ─────────────────────────────────────────────────────────────

    #[test]
    fn accumulator_overflow_reports_calculation_failure() {
        use rust_decimal::Decimal;
        let policy = policy_usd(Some(pnl("-100")), None);

        // First MAX: stores MAX, within [-100, ∞) — no block.
        let first =
            apply_report_blocks(&policy, &report("USD", account(1), Pnl::new(Decimal::MAX)));
        assert!(first.is_empty());

        // Second MAX: previous (MAX) + MAX overflows the accumulator and is
        // surfaced as OrderValueCalculationFailed; the engine receives the
        // AccountBlock and latches the account itself.
        let second =
            apply_report_blocks(&policy, &report("USD", account(1), Pnl::new(Decimal::MAX)));
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].code, RejectCode::OrderValueCalculationFailed);
        assert_eq!(second[0].reason, "pnl accumulation overflow");
        assert!(second[0]
            .details
            .contains("realized pnl + pnl_with_fee overflow"));
    }

    #[test]
    fn negative_accumulator_overflow_reports_calculation_failure() {
        use rust_decimal::Decimal;
        let policy = policy_usd(None, Some(pnl("100")));

        let first =
            apply_report_blocks(&policy, &report("USD", account(1), Pnl::new(Decimal::MIN)));
        assert!(first.is_empty());
        let second =
            apply_report_blocks(&policy, &report("USD", account(1), Pnl::new(Decimal::MIN)));
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].code, RejectCode::OrderValueCalculationFailed);
    }

    // Fee is a rebate (negative fee), so fee.to_pnl() = +1; MAX + 1 overflows
    // at the pnl + fee stage, before the accumulator is even touched.
    #[test]
    fn pnl_plus_fee_overflow_reports_calculation_failure() {
        use rust_decimal::Decimal;
        let policy = policy_usd(None, Some(Pnl::new(Decimal::MAX)));

        let blocks = apply_report_blocks(
            &policy,
            &report_with_fee(
                "USD",
                account(1),
                Pnl::new(Decimal::MAX),
                Fee::new(-Decimal::ONE),
            ),
        );
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].code, RejectCode::OrderValueCalculationFailed);
        assert_eq!(blocks[0].reason, "pnl accumulation overflow");
        assert!(blocks[0].details.contains("pnl + fee overflow"));
    }

    // The accumulator is not corrupted on overflow: subsequent reports are
    // evaluated against the last known good value (or zero if none).
    #[test]
    fn overflow_does_not_corrupt_subsequent_reports() {
        use rust_decimal::Decimal;
        let policy = policy_usd(None, Some(Pnl::new(Decimal::MAX)));

        let first = apply_report_blocks(
            &policy,
            &report_with_fee(
                "USD",
                account(1),
                Pnl::new(Decimal::MAX),
                Fee::new(-Decimal::ONE),
            ),
        );
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].code, RejectCode::OrderValueCalculationFailed);

        // ZERO + ZERO accumulates cleanly and stays within bounds.
        let second = apply_report_blocks(&policy, &report("USD", account(1), Pnl::ZERO));
        assert!(second.is_empty());
    }

    // Realized P&L is now accumulated unconditionally. An overflow in pnl+fee
    // for an untracked settlement still produces an AccountBlock so that the
    // engine can latch the account - the barrier-presence check no longer
    // short-circuits before the overflow guard.
    #[test]
    fn untracked_settlement_pnl_plus_fee_overflow_produces_block() {
        use rust_decimal::Decimal;
        let policy = policy_usd(None, Some(Pnl::new(Decimal::MAX)));

        let blocks = apply_report_blocks(
            &policy,
            &report_with_fee(
                "EUR",
                account(1),
                Pnl::new(Decimal::MAX),
                Fee::new(-Decimal::ONE),
            ),
        );
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].code, RejectCode::OrderValueCalculationFailed);
        assert_eq!(blocks[0].reason, "pnl accumulation overflow");
        assert!(blocks[0].details.contains("pnl + fee overflow"));
    }

    // ── account-only barrier ─────────────────────────────────────────────────

    #[test]
    fn account_only_barrier_without_global() {
        let settings = PnlBoundsKillSwitchSettings::new(
            [],
            [PnlBoundsAccountAssetBarrier {
                barrier: barrier("USD", Some(pnl("-100")), None),
                account_id: account(1),
                initial_pnl: Pnl::ZERO,
            }],
        )
        .expect("settings must be valid");
        let policy: TestPolicy =
            PnlBoundsKillSwitchPolicy::new(settings, test_builder().storage_builder());

        // apply detects breach -> returns AccountBlock for account(1) USD.
        let blocks = apply_report_blocks(&policy, &report("USD", account(1), pnl("-150")));
        assert_eq!(blocks.len(), 1);
        let reject = check_start(&policy, &order("USD", account(1))).expect_err("must reject");
        assert_eq!(reject[0].code, RejectCode::PnlKillSwitchTriggered);
        assert_eq!(
            reject[0].reason,
            "pnl kill switch triggered: account + asset barrier"
        );

        // Other account, same settlement: no barrier -> passes.
        assert!(check_start(&policy, &order("USD", account(2))).is_ok());
        // Same account, different settlement: no barrier -> passes.
        assert!(check_start(&policy, &order("EUR", account(1))).is_ok());
    }

    // ── error paths ─────────────────────────────────────────────────────────

    #[test]
    fn check_pre_trade_start_maps_instrument_access_error() {
        struct InvalidOrder;

        impl HasInstrument for InvalidOrder {
            fn instrument(&self) -> Result<&Instrument, RequestFieldAccessError> {
                Err(RequestFieldAccessError::new("instrument"))
            }
        }

        impl HasAccountId for InvalidOrder {
            fn account_id(&self) -> Result<AccountId, RequestFieldAccessError> {
                Ok(account(1))
            }
        }

        let policy = policy_usd(Some(pnl("-100")), Some(pnl("50")));
        let reject = <TestPolicy as PreTradePolicy<
            InvalidOrder,
            TestReport,
            (),
            crate::core::LocalSync,
        >>::check_pre_trade_start(
            &policy,
            &PreTradeContext::<NoLocking>::new(None),
            &InvalidOrder,
        )
        .expect_err("field access error must reject");
        let reject = &reject[0];
        assert_eq!(reject.scope, RejectScope::Order);
        assert_eq!(reject.code, RejectCode::MissingRequiredField);
        assert_eq!(
            reject.reason,
            "failed to access required field 'instrument'"
        );
        assert_eq!(reject.details, "failed to access field 'instrument'");
    }

    // ── settings-cell tests ─────────────────────────────────────────────────

    #[test]
    fn settings_cell_clone_shares_underlying_value() {
        use crate::pretrade::ConfigurablePolicy;

        let policy = policy_usd(Some(pnl("-100")), Some(pnl("50")));
        let cell = policy.settings_cell();

        // Update through the clone; the policy must observe the change.
        cell.update::<PnlBoundsKillSwitchPolicyError>(|s| {
            s.set_broker_barriers([barrier_usd(Some(pnl("-200")), Some(pnl("200")))])
        })
        .expect("valid update");

        // Old bound was 50; new is 200. 100 was outside before, inside now.
        apply_report(&policy, &report("USD", account(1), pnl("100")));
        assert!(
            check_start(&policy, &order("USD", account(1))).is_ok(),
            "updated barrier must be observed by the policy"
        );
    }

    #[test]
    fn settings_update_does_not_reset_realized_pnl() {
        use crate::pretrade::ConfigurablePolicy;

        let policy = policy_usd(Some(pnl("-100")), Some(pnl("50")));

        // Accumulate some P&L.
        apply_report(&policy, &report("USD", account(1), pnl("40")));

        // Tighten the upper bound to 30 via the cell; P&L (40) is now outside.
        let cell = policy.settings_cell();
        cell.update::<PnlBoundsKillSwitchPolicyError>(|s| {
            s.set_broker_barriers([barrier_usd(Some(pnl("-100")), Some(pnl("30")))])
        })
        .expect("valid update");

        // The accumulated 40 must still be present (not reset by update).
        let reject = check_start(&policy, &order("USD", account(1))).expect_err("must reject");
        assert_eq!(reject[0].code, RejectCode::PnlKillSwitchTriggered);
        assert_eq!(
            reject[0].reason,
            "pnl kill switch triggered: broker barrier"
        );
    }

    // `Configurator::set_account_pnl` force-sets the live accumulator and the
    // override is observed on the next hot-path check: forcing one account past
    // a bound rejects its next order, while forcing a different (never-breached)
    // account inside the bound lets its order through. Driven through a real
    // engine because the override lives on the configurator, not the policy.
    //
    // The two directions use different accounts on purpose: a breach is a kill
    // switch, so once account 1 is rejected the engine latches its account-level
    // block, which force-setting the ledger back inside the bound does not clear
    // (only an explicit admin unblock does). The passing direction is therefore
    // exercised on account 2.
    #[test]
    fn set_account_pnl_overrides_live_accumulator() {
        use crate::Engine;

        let builder = Engine::builder::<OrderOperation, TestReport, ()>().no_sync();
        let settings =
            PnlBoundsKillSwitchSettings::new([barrier_usd(Some(pnl("-100")), Some(pnl("50")))], [])
                .expect("settings must be valid");
        let policy: TestPolicy =
            PnlBoundsKillSwitchPolicy::new(settings, builder.storage_builder());
        let engine = builder
            .pre_trade(policy)
            .build()
            .expect("engine must build");
        let name = PnlBoundsKillSwitchPolicy::<NoLocking>::NAME;
        let usd = Asset::new("USD").expect("asset code must be valid");

        // No history: the order passes against bounds [-100, 50].
        engine
            .execute_pre_trade(order("USD", account(1)))
            .expect("order must pass with zero P&L");

        // Force account 1's accumulator below the lower bound; its next order
        // rejects and the engine latches the account-level block.
        engine
            .configure()
            .set_account_pnl(name, account(1), usd.clone(), pnl("-150"))
            .expect("force-set must publish");
        let reject = engine
            .execute_pre_trade(order("USD", account(1)))
            .err()
            .expect("order must reject after the override breaches the bound");
        assert_eq!(reject[0].code, RejectCode::PnlKillSwitchTriggered);
        assert_eq!(
            reject[0].reason,
            "pnl kill switch triggered: broker barrier"
        );

        // Force account 2's accumulator inside the bounds; its order passes,
        // proving the override is observed in the passing direction too.
        engine
            .configure()
            .set_account_pnl(name, account(2), usd, pnl("-10"))
            .expect("force-set inside bounds must publish");
        engine
            .execute_pre_trade(order("USD", account(2)))
            .expect("order must pass after the accumulator is set inside bounds");
    }

    // Realized P&L accumulated before a barrier is added must be respected once
    // the barrier is installed at runtime. Construct with a barrier on account B
    // / USD only (to satisfy the "at least one barrier" constructor rule), then
    // feed reports for account A / USD (no barrier yet) to build up a loss.
    #[test]
    fn barrier_added_at_runtime_sees_pre_accumulated_pnl() {
        use crate::pretrade::ConfigurablePolicy;

        let settings = PnlBoundsKillSwitchSettings::new(
            [],
            [PnlBoundsAccountAssetBarrier {
                barrier: barrier("USD", Some(pnl("-500")), None),
                account_id: account(2),
                initial_pnl: Pnl::ZERO,
            }],
        )
        .expect("settings must be valid");
        let policy: TestPolicy =
            PnlBoundsKillSwitchPolicy::new(settings, test_builder().storage_builder());

        apply_report(&policy, &report("USD", account(1), pnl("-80")));
        apply_report(&policy, &report("USD", account(1), pnl("-40")));
        assert!(check_start(&policy, &order("USD", account(1))).is_ok());

        let cell = policy.settings_cell();
        cell.update::<PnlBoundsKillSwitchPolicyError>(|settings| {
            settings.set_account_barriers([
                PnlBoundsAccountAssetBarrierUpdate {
                    barrier: barrier("USD", Some(pnl("-500")), None),
                    account_id: account(2),
                },
                PnlBoundsAccountAssetBarrierUpdate {
                    barrier: barrier("USD", Some(pnl("-100")), None),
                    account_id: account(1),
                },
            ])
        })
        .expect("barrier add must succeed");

        let reject = check_start(&policy, &order("USD", account(1))).expect_err("must reject");
        assert_eq!(
            reject[0].reason,
            "pnl kill switch triggered: account + asset barrier"
        );
    }

    #[test]
    fn settings_update_invalid_leaves_prior_value() {
        use crate::pretrade::ConfigurablePolicy;

        let policy = policy_usd(Some(pnl("-100")), Some(pnl("50")));
        let cell = policy.settings_cell();

        // Attempt to clear all barriers via set_broker_barriers; this will
        // fail because account_barriers is also empty.
        let result = cell.update::<PnlBoundsKillSwitchPolicyError>(|s| s.set_broker_barriers([]));
        assert_eq!(
            result,
            Err(PnlBoundsKillSwitchPolicyError::NoBarriersConfigured)
        );

        // Original barrier still applies.
        apply_report(&policy, &report("USD", account(1), pnl("51")));
        let reject = check_start(&policy, &order("USD", account(1))).expect_err("must reject");
        assert_eq!(reject[0].code, RejectCode::PnlKillSwitchTriggered);
    }

    #[test]
    fn set_account_barriers_validated_before_apply() {
        let mut settings =
            PnlBoundsKillSwitchSettings::new([barrier_usd(Some(pnl("-100")), None)], [])
                .expect("valid");

        let usd = Asset::new("USD").expect("valid");
        let err = settings
            .set_account_barriers([PnlBoundsAccountAssetBarrierUpdate {
                barrier: PnlBoundsBrokerBarrier {
                    settlement_asset: usd.clone(),
                    lower_bound: None,
                    upper_bound: None,
                },
                account_id: account(1),
            }])
            .expect_err("must fail");
        assert_eq!(
            err,
            PnlBoundsKillSwitchPolicyError::NoBoundsConfigured {
                settlement_asset: usd,
            }
        );
        assert!(settings
            .broker_barriers
            .contains_key(&Asset::new("USD").expect("valid")));
    }

    #[test]
    fn with_policy_group_id_observed_via_policy_group_id() {
        use crate::pretrade::PolicyGroupId;

        let policy =
            policy_usd(Some(pnl("-100")), None).with_policy_group_id(PolicyGroupId::new(7));
        let observed = <TestPolicy as PreTradePolicy<
            OrderOperation,
            TestReport,
            (),
            crate::core::LocalSync,
        >>::policy_group_id(&policy);
        assert_eq!(observed, PolicyGroupId::new(7));
    }

    // ── helpers ─────────────────────────────────────────────────────────────

    fn check_start(
        policy: &TestPolicy,
        order: &OrderOperation,
    ) -> Result<(), crate::pretrade::Rejects> {
        <TestPolicy as PreTradePolicy<OrderOperation, TestReport, (), crate::core::LocalSync>>::check_pre_trade_start(
            policy,
            &PreTradeContext::<NoLocking>::new(None),
            order,
        )
    }

    fn apply_report(policy: &TestPolicy, report: &TestReport) -> bool {
        !apply_report_blocks(policy, report).is_empty()
    }

    fn apply_report_blocks(
        policy: &TestPolicy,
        report: &TestReport,
    ) -> Vec<crate::pretrade::AccountBlock> {
        <TestPolicy as PreTradePolicy<OrderOperation, TestReport, (), crate::core::LocalSync>>::apply_execution_report(
            policy,
            &crate::pretrade::PostTradeContext::new(),
            report,
        )
        .map(|r| r.account_blocks)
        .unwrap_or_default()
    }

    fn report(settlement: &str, account_id: AccountId, pnl_val: Pnl) -> TestReport {
        TestReport {
            instrument: Instrument::new(
                Asset::new("AAPL").expect("must be valid"),
                Asset::new(settlement).expect("must be valid"),
            ),
            account_id,
            pnl: pnl_val,
            fee: Fee::ZERO,
        }
    }

    fn report_with_fee(
        settlement: &str,
        account_id: AccountId,
        pnl_val: Pnl,
        fee: Fee,
    ) -> TestReport {
        TestReport {
            instrument: Instrument::new(
                Asset::new("AAPL").expect("must be valid"),
                Asset::new(settlement).expect("must be valid"),
            ),
            account_id,
            pnl: pnl_val,
            fee,
        }
    }

    fn policy_usd(lower_bound: Option<Pnl>, upper_bound: Option<Pnl>) -> TestPolicy {
        let settings =
            PnlBoundsKillSwitchSettings::new([barrier_usd(lower_bound, upper_bound)], [])
                .expect("settings must be valid");
        PnlBoundsKillSwitchPolicy::new(settings, test_builder().storage_builder())
    }

    fn barrier_usd(lower_bound: Option<Pnl>, upper_bound: Option<Pnl>) -> PnlBoundsBrokerBarrier {
        barrier("USD", lower_bound, upper_bound)
    }

    fn barrier(
        settlement: &str,
        lower_bound: Option<Pnl>,
        upper_bound: Option<Pnl>,
    ) -> PnlBoundsBrokerBarrier {
        PnlBoundsBrokerBarrier {
            settlement_asset: Asset::new(settlement).expect("must be valid"),
            lower_bound,
            upper_bound,
        }
    }

    fn order(settlement: &str, account_id: AccountId) -> OrderOperation {
        OrderOperation {
            instrument: Instrument::new(
                Asset::new("AAPL").expect("must be valid"),
                Asset::new(settlement).expect("must be valid"),
            ),
            account_id,
            side: Side::Buy,
            trade_amount: TradeAmount::Quantity(
                Quantity::from_str("1").expect("quantity literal must be valid"),
            ),
            price: Some(Price::from_str("100").expect("price literal must be valid")),
        }
    }

    fn account(id: u64) -> AccountId {
        AccountId::from_u64(id)
    }

    fn pnl(value: &str) -> Pnl {
        Pnl::from_str(value).expect("pnl literal must be valid")
    }

    // ── missing report fields ────────────────────────────────────────────────

    #[test]
    fn missing_instrument_in_report_returns_account_block() {
        struct NoInstrument {
            account_id: AccountId,
            pnl: Pnl,
            fee: Fee,
        }
        impl HasInstrument for NoInstrument {
            fn instrument(&self) -> Result<&Instrument, RequestFieldAccessError> {
                Err(RequestFieldAccessError::new("instrument"))
            }
        }
        impl HasAccountId for NoInstrument {
            fn account_id(&self) -> Result<AccountId, RequestFieldAccessError> {
                Ok(self.account_id)
            }
        }
        impl HasPnl for NoInstrument {
            fn pnl(&self) -> Result<Pnl, RequestFieldAccessError> {
                Ok(self.pnl)
            }
        }
        impl HasFee for NoInstrument {
            fn fee(&self) -> Result<Fee, RequestFieldAccessError> {
                Ok(self.fee)
            }
        }

        let policy = policy_usd(Some(pnl("-100")), Some(pnl("50")));
        let report = NoInstrument {
            account_id: account(1),
            pnl: pnl("0"),
            fee: Fee::ZERO,
        };
        let blocks = <TestPolicy as PreTradePolicy<
            OrderOperation,
            NoInstrument,
            (),
            crate::core::LocalSync,
        >>::apply_execution_report(
            &policy, &crate::pretrade::PostTradeContext::new(), &report
        )
        .map(|r| r.account_blocks)
        .unwrap_or_default();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].code, RejectCode::MissingRequiredField);
        assert_eq!(
            blocks[0].reason,
            "failed to access required field 'instrument'"
        );
        assert_eq!(blocks[0].details, "failed to access field 'instrument'");
    }

    #[test]
    fn missing_account_id_in_report_returns_account_block() {
        struct NoAccount {
            instrument: Instrument,
            pnl: Pnl,
            fee: Fee,
        }
        impl HasInstrument for NoAccount {
            fn instrument(&self) -> Result<&Instrument, RequestFieldAccessError> {
                Ok(&self.instrument)
            }
        }
        impl HasAccountId for NoAccount {
            fn account_id(&self) -> Result<AccountId, RequestFieldAccessError> {
                Err(RequestFieldAccessError::new("account_id"))
            }
        }
        impl HasPnl for NoAccount {
            fn pnl(&self) -> Result<Pnl, RequestFieldAccessError> {
                Ok(self.pnl)
            }
        }
        impl HasFee for NoAccount {
            fn fee(&self) -> Result<Fee, RequestFieldAccessError> {
                Ok(self.fee)
            }
        }

        let policy = policy_usd(Some(pnl("-100")), Some(pnl("50")));
        let report = NoAccount {
            instrument: Instrument::new(
                Asset::new("AAPL").expect("must be valid"),
                Asset::new("USD").expect("must be valid"),
            ),
            pnl: pnl("0"),
            fee: Fee::ZERO,
        };
        let blocks = <TestPolicy as PreTradePolicy<
            OrderOperation,
            NoAccount,
            (),
            crate::core::LocalSync,
        >>::apply_execution_report(
            &policy, &crate::pretrade::PostTradeContext::new(), &report
        )
        .map(|r| r.account_blocks)
        .unwrap_or_default();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].code, RejectCode::MissingRequiredField);
        assert_eq!(
            blocks[0].reason,
            "failed to access required field 'account ID'"
        );
        assert_eq!(blocks[0].details, "failed to access field 'account_id'");
    }

    #[test]
    fn missing_pnl_in_report_returns_account_block() {
        struct NoPnl {
            instrument: Instrument,
            account_id: AccountId,
            fee: Fee,
        }
        impl HasInstrument for NoPnl {
            fn instrument(&self) -> Result<&Instrument, RequestFieldAccessError> {
                Ok(&self.instrument)
            }
        }
        impl HasAccountId for NoPnl {
            fn account_id(&self) -> Result<AccountId, RequestFieldAccessError> {
                Ok(self.account_id)
            }
        }
        impl HasPnl for NoPnl {
            fn pnl(&self) -> Result<Pnl, RequestFieldAccessError> {
                Err(RequestFieldAccessError::new("pnl"))
            }
        }
        impl HasFee for NoPnl {
            fn fee(&self) -> Result<Fee, RequestFieldAccessError> {
                Ok(self.fee)
            }
        }

        let policy = policy_usd(Some(pnl("-100")), Some(pnl("50")));
        let report = NoPnl {
            instrument: Instrument::new(
                Asset::new("AAPL").expect("must be valid"),
                Asset::new("USD").expect("must be valid"),
            ),
            account_id: account(1),
            fee: Fee::ZERO,
        };
        let blocks = <TestPolicy as PreTradePolicy<
            OrderOperation,
            NoPnl,
            (),
            crate::core::LocalSync,
        >>::apply_execution_report(
            &policy, &crate::pretrade::PostTradeContext::new(), &report
        )
        .map(|r| r.account_blocks)
        .unwrap_or_default();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].code, RejectCode::MissingRequiredField);
        assert_eq!(blocks[0].reason, "failed to access required field 'P&L'");
        assert_eq!(blocks[0].details, "failed to access field 'pnl'");
    }

    #[test]
    fn missing_fee_in_report_returns_account_block() {
        struct NoFee {
            instrument: Instrument,
            account_id: AccountId,
            pnl: Pnl,
        }
        impl HasInstrument for NoFee {
            fn instrument(&self) -> Result<&Instrument, RequestFieldAccessError> {
                Ok(&self.instrument)
            }
        }
        impl HasAccountId for NoFee {
            fn account_id(&self) -> Result<AccountId, RequestFieldAccessError> {
                Ok(self.account_id)
            }
        }
        impl HasPnl for NoFee {
            fn pnl(&self) -> Result<Pnl, RequestFieldAccessError> {
                Ok(self.pnl)
            }
        }
        impl HasFee for NoFee {
            fn fee(&self) -> Result<Fee, RequestFieldAccessError> {
                Err(RequestFieldAccessError::new("fee"))
            }
        }

        let policy = policy_usd(Some(pnl("-100")), Some(pnl("50")));
        let report = NoFee {
            instrument: Instrument::new(
                Asset::new("AAPL").expect("must be valid"),
                Asset::new("USD").expect("must be valid"),
            ),
            account_id: account(1),
            pnl: pnl("0"),
        };
        let blocks = <TestPolicy as PreTradePolicy<
            OrderOperation,
            NoFee,
            (),
            crate::core::LocalSync,
        >>::apply_execution_report(
            &policy, &crate::pretrade::PostTradeContext::new(), &report
        )
        .map(|r| r.account_blocks)
        .unwrap_or_default();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].code, RejectCode::MissingRequiredField);
        assert_eq!(blocks[0].reason, "failed to access required field 'fee'");
        assert_eq!(blocks[0].details, "failed to access field 'fee'");
    }

    // ── fast-path bug fix coverage ───────────────────────────────────────────

    #[test]
    fn broker_barrier_excluding_zero_rejects_account_without_history() {
        let settings =
            PnlBoundsKillSwitchSettings::new([barrier("USD", Some(pnl("10")), None)], [])
                .expect("settings must be valid");
        let policy: TestPolicy =
            PnlBoundsKillSwitchPolicy::new(settings, test_builder().storage_builder());

        let reject = check_start(&policy, &order("USD", account(1))).expect_err("must reject");
        let reject = &reject[0];
        assert_eq!(reject.code, RejectCode::PnlKillSwitchTriggered);
        assert_eq!(reject.reason, "pnl kill switch triggered: broker barrier");
        assert!(reject.details.contains("lower bound breached"));
    }
}
