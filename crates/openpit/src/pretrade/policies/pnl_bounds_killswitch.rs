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
use crate::storage::{Storage, StorageBuilder};

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
    /// Starting accumulated P&L for the account.
    ///
    /// Pre-loaded into storage at construction; accumulation starts from this value.
    pub initial_pnl: Pnl,
}

/// P&L kill switch with per-settlement-asset bounds.
///
/// Accumulates realized P&L (`pnl + fee` from execution reports) per
/// `(account, settlement asset)` and blocks the account when the running
/// total crosses a configured lower or upper bound.
///
/// Two barrier kinds can be configured, independently or together:
/// - **broker barrier** — one set of bounds per settlement asset, applied to
///   every account trading that asset;
/// - **account+asset barrier** — tighter bounds for a specific
///   `(account, settlement asset)` pair, with an explicit starting P&L.
///
/// An order is blocked when the realized P&L on its settlement asset is
/// outside **either** applicable barrier. If neither barrier covers the
/// `(account, settlement)` pair, the order passes.
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
    broker_barriers: HashMap<Asset, PnlBoundsBrokerBarrier>,
    account_barriers: HashMap<AccountId, HashMap<Asset, PnlBoundsAccountAssetBarrier>>,
    group_id: PolicyGroupId,
    realized: Storage<
        (AccountId, Asset),
        Pnl,
        <LockingPolicyFactory as crate::storage::LockingPolicyFactory>::Policy,
    >,
}

/// Errors returned by [`PnlBoundsKillSwitchPolicy`] operations.
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
                "at least one of lower_bound or upper_bound must be configured for settlement asset {settlement_asset}"
            ),
        }
    }
}

impl std::error::Error for PnlBoundsKillSwitchPolicyError {}

impl<LockingPolicyFactory> PnlBoundsKillSwitchPolicy<LockingPolicyFactory>
where
    LockingPolicyFactory: crate::storage::LockingPolicyFactory,
{
    /// Stable policy name.
    pub const NAME: &'static str = "PnlBoundsKillSwitchPolicy";

    /// Creates a P&L bounds kill-switch policy.
    ///
    /// `storage_builder` must be obtained from the engine builder so that the
    /// per-account P&L storage shares the factory type with the engine's
    /// synchronization mode.
    ///
    /// At least one barrier must be provided across `broker_barriers` and
    /// `account_barriers`; if both are empty, returns `NoBarriersConfigured`.
    /// Each barrier must have at least one bound configured.
    pub fn new(
        broker_barriers: impl IntoIterator<Item = PnlBoundsBrokerBarrier>,
        account_barriers: impl IntoIterator<Item = PnlBoundsAccountAssetBarrier>,
        storage_builder: &StorageBuilder<LockingPolicyFactory>,
    ) -> Result<Self, PnlBoundsKillSwitchPolicyError>
    where
        LockingPolicyFactory: crate::storage::CreateStorageFor<(AccountId, Asset)>,
    {
        let mut broker = HashMap::new();
        for barrier in broker_barriers {
            Self::validate_bounds(
                &barrier.lower_bound,
                &barrier.upper_bound,
                &barrier.settlement_asset,
            )?;
            broker.insert(barrier.settlement_asset.clone(), barrier);
        }

        let mut account: HashMap<AccountId, HashMap<Asset, PnlBoundsAccountAssetBarrier>> =
            HashMap::new();
        for barrier in account_barriers {
            Self::validate_bounds(
                &barrier.barrier.lower_bound,
                &barrier.barrier.upper_bound,
                &barrier.barrier.settlement_asset,
            )?;
            account
                .entry(barrier.account_id)
                .or_default()
                .insert(barrier.barrier.settlement_asset.clone(), barrier);
        }

        if broker.is_empty() && account.is_empty() {
            return Err(PnlBoundsKillSwitchPolicyError::NoBarriersConfigured);
        }

        let realized = storage_builder.create_for_bound_key();
        for (account_id, by_settlement) in &account {
            for (settlement_asset, barrier) in by_settlement {
                realized.with_mut(
                    (*account_id, settlement_asset.clone()),
                    || Pnl::ZERO,
                    |entry, _is_new| {
                        *entry = barrier.initial_pnl;
                    },
                );
            }
        }

        Ok(Self {
            broker_barriers: broker,
            account_barriers: account,
            realized,
            group_id: DEFAULT_POLICY_GROUP_ID,
        })
    }

    /// Assigns a group tag to this policy instance.
    ///
    /// See [`PolicyGroupId`] and [`DEFAULT_POLICY_GROUP_ID`] for details.
    pub fn with_policy_group_id(mut self, id: PolicyGroupId) -> Self {
        self.group_id = id;
        self
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
}

impl<LockingPolicyFactory> PolicyName for PnlBoundsKillSwitchPolicy<LockingPolicyFactory>
where
    LockingPolicyFactory: crate::storage::LockingPolicyFactory,
{
    fn policy_name(&self) -> &str {
        Self::NAME
    }
}

impl<Order, ExecutionReport, AccountAdjustment, LockingPolicyFactory, Sync>
    PreTradePolicy<Order, ExecutionReport, AccountAdjustment, Sync>
    for PnlBoundsKillSwitchPolicy<LockingPolicyFactory>
where
    Order: HasInstrument + HasAccountId,
    ExecutionReport: HasInstrument + HasPnl + HasFee + HasAccountId,
    LockingPolicyFactory: crate::storage::LockingPolicyFactory,
    Sync: crate::core::SyncMode,
{
    fn name(&self) -> &str {
        Self::NAME
    }

    fn policy_group_id(&self) -> PolicyGroupId {
        self.group_id
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

        let broker_barrier = self.broker_barriers.get(settlement);
        let account_barrier = self
            .account_barriers
            .get(&account_id)
            .and_then(|m| m.get(settlement));

        if broker_barrier.is_none() && account_barrier.is_none() {
            return Ok(());
        }

        let current_pnl = self
            .realized
            .with(&(account_id, settlement.clone()), |entry| *entry)
            .unwrap_or(Pnl::ZERO);

        if let Some(broker_barrier) = broker_barrier {
            let breached = breached_sides(
                broker_barrier.lower_bound,
                broker_barrier.upper_bound,
                current_pnl,
            );
            if !breached.is_empty() {
                return Err(barrier_breach_reject(
                    self,
                    "pnl kill switch triggered: broker barrier",
                    &breached,
                    broker_barrier,
                    current_pnl,
                    account_id,
                )
                .into());
            }
        }

        if let Some(account_barrier) = account_barrier {
            let breached = breached_sides(
                account_barrier.barrier.lower_bound,
                account_barrier.barrier.upper_bound,
                current_pnl,
            );
            if !breached.is_empty() {
                return Err(barrier_breach_reject(
                    self,
                    "pnl kill switch triggered: account + asset barrier",
                    &breached,
                    &account_barrier.barrier,
                    current_pnl,
                    account_id,
                )
                .into());
            }
        }

        Ok(())
    }

    /// Applies a post-trade report to the accumulated realized P&L for the
    /// report's `(account, settlement_asset)`.
    ///
    /// The report contract expects `pnl` plus explicit `fee`. Fee impact is
    /// added to `pnl` before accumulation.
    ///
    /// The accumulation is performed under a single write lock; no intermediate
    /// reads from the realized storage are issued.
    ///
    /// Returns an [`AccountBlock`] when any required report field cannot be
    /// accessed, when `pnl + fee` or the accumulated value overflows, or when
    /// the new accumulated value breaches a configured barrier.
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

        // Only track accounts/settlements that have a configured barrier.
        let has_barrier = self.broker_barriers.contains_key(settlement)
            || self
                .account_barriers
                .get(&account_id)
                .is_some_and(|m| m.contains_key(settlement));
        if !has_barrier {
            return None;
        }

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
                if is_outside_bounds(
                    &self.broker_barriers,
                    &self.account_barriers,
                    updated,
                    settlement,
                    account_id,
                ) {
                    Some(AccountBlock::new(
                        self.policy_name(),
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

fn barrier_breach_reject<Policy: PolicyName + ?Sized>(
    policy: &Policy,
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
        policy.policy_name(),
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
    account_barriers: &HashMap<AccountId, HashMap<Asset, PnlBoundsAccountAssetBarrier>>,
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

#[cfg(test)]
mod tests {
    use crate::core::{HasAccountId, HasFee, HasInstrument, HasPnl, Instrument, OrderOperation};
    use crate::param::TradeAmount;
    use crate::param::{AccountId, Asset, Fee, Pnl, Price, Quantity, Side};
    use crate::pretrade::{PreTradeContext, PreTradePolicy, RejectCode, RejectScope};
    use crate::storage::NoLocking;
    use crate::RequestFieldAccessError;

    use super::{
        PnlBoundsAccountAssetBarrier, PnlBoundsBrokerBarrier, PnlBoundsKillSwitchPolicy,
        PnlBoundsKillSwitchPolicyError,
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

    // apply_execution_report detects a breach → returns AccountBlock and the
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
        let policy: TestPolicy = PnlBoundsKillSwitchPolicy::new(
            [barrier_usd(Some(pnl("-100")), Some(pnl("50")))],
            [PnlBoundsAccountAssetBarrier {
                barrier: barrier("USD", Some(pnl("-500")), None),
                account_id: account(1),
                initial_pnl: pnl("-101"),
            }],
            test_builder().storage_builder(),
        )
        .expect("policy must be valid");
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
        let policy: TestPolicy = PnlBoundsKillSwitchPolicy::new(
            [barrier_usd(Some(pnl("-100")), Some(pnl("50")))],
            [PnlBoundsAccountAssetBarrier {
                barrier: barrier("USD", None, Some(pnl("500"))),
                account_id: account(1),
                initial_pnl: pnl("51"),
            }],
            test_builder().storage_builder(),
        )
        .expect("policy must be valid");
        let reject = check_start(&policy, &order("USD", account(1))).expect_err("must reject");
        assert_eq!(
            reject[0].reason,
            "pnl kill switch triggered: broker barrier"
        );
        assert!(reject[0].details.contains("upper bound breached"));
    }

    #[test]
    fn inverted_bounds_breach_detected_at_check() {
        let policy: TestPolicy = PnlBoundsKillSwitchPolicy::new(
            [barrier_usd(Some(pnl("10")), Some(pnl("5")))],
            [PnlBoundsAccountAssetBarrier {
                barrier: barrier("USD", None, Some(pnl("500"))),
                account_id: account(1),
                initial_pnl: pnl("7"),
            }],
            test_builder().storage_builder(),
        )
        .expect("policy must be valid");
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
        let policy: TestPolicy = PnlBoundsKillSwitchPolicy::new(
            [barrier_usd(Some(pnl("-500")), None)],
            [PnlBoundsAccountAssetBarrier {
                barrier: barrier("USD", Some(pnl("-100")), None),
                account_id: account(1),
                initial_pnl: pnl("-90"),
            }],
            test_builder().storage_builder(),
        )
        .expect("policy must be valid");

        // initial_pnl -90 is within account bounds [-100, ∞): passes before any report
        assert!(check_start(&policy, &order("USD", account(1))).is_ok());
        // account 2 has no account+asset barrier, starts at 0: passes broker
        assert!(check_start(&policy, &order("USD", account(2))).is_ok());
    }

    #[test]
    fn account_barrier_breach_detected_at_check_specific_reason() {
        // initial_pnl -200 violates account bound [-100] but NOT global [-500].
        let policy: TestPolicy = PnlBoundsKillSwitchPolicy::new(
            [barrier_usd(Some(pnl("-500")), None)],
            [PnlBoundsAccountAssetBarrier {
                barrier: barrier("USD", Some(pnl("-100")), None),
                account_id: account(1),
                initial_pnl: pnl("-200"),
            }],
            test_builder().storage_builder(),
        )
        .expect("policy must be valid");

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
        let policy: TestPolicy = PnlBoundsKillSwitchPolicy::new(
            [barrier_usd(Some(pnl("-500")), None)],
            [PnlBoundsAccountAssetBarrier {
                barrier: barrier("USD", Some(pnl("-100")), None),
                account_id: account(1),
                initial_pnl: Pnl::ZERO,
            }],
            test_builder().storage_builder(),
        )
        .expect("policy must be valid");

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
        let policy: TestPolicy = PnlBoundsKillSwitchPolicy::new(
            [barrier_usd(Some(pnl("-100")), None)],
            [PnlBoundsAccountAssetBarrier {
                barrier: barrier("USD", Some(pnl("-500")), None),
                account_id: account(1),
                initial_pnl: Pnl::ZERO,
            }],
            test_builder().storage_builder(),
        )
        .expect("policy must be valid");

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
        let err =
            PnlBoundsKillSwitchPolicy::<NoLocking>::new([], [], test_builder().storage_builder())
                .err()
                .expect("must fail");
        assert_eq!(err, PnlBoundsKillSwitchPolicyError::NoBarriersConfigured);
        assert_eq!(
            err.to_string(),
            "at least one broker or account+asset barrier must be configured"
        );
    }

    #[test]
    fn missing_bounds_rejected_by_constructor() {
        let usd = Asset::new("USD").expect("asset code must be valid");
        let err = PnlBoundsKillSwitchPolicy::<NoLocking>::new(
            [PnlBoundsBrokerBarrier {
                settlement_asset: usd.clone(),
                lower_bound: None,
                upper_bound: None,
            }],
            [],
            test_builder().storage_builder(),
        )
        .err()
        .expect("must fail");

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
        let err = PnlBoundsKillSwitchPolicy::<NoLocking>::new(
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
            test_builder().storage_builder(),
        )
        .err()
        .expect("must fail");

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
        let policy: TestPolicy = PnlBoundsKillSwitchPolicy::new(
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
            test_builder().storage_builder(),
        )
        .expect("policy must be valid");

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

    #[test]
    fn untracked_settlement_ignores_pnl_plus_fee_overflow() {
        use rust_decimal::Decimal;
        let policy = policy_usd(None, Some(Pnl::new(Decimal::MAX)));

        // EUR has no barrier; overflow in pnl+fee must not block the account.
        let triggered = apply_report(
            &policy,
            &report_with_fee(
                "EUR",
                account(1),
                Pnl::new(Decimal::MAX),
                Fee::new(-Decimal::ONE),
            ),
        );
        assert!(
            !triggered,
            "untracked settlement must not trigger kill switch"
        );
        assert!(check_start(&policy, &order("EUR", account(1))).is_ok());
    }

    // ── account-only barrier ─────────────────────────────────────────────────

    #[test]
    fn account_only_barrier_without_global() {
        let policy: TestPolicy = PnlBoundsKillSwitchPolicy::new(
            [],
            [PnlBoundsAccountAssetBarrier {
                barrier: barrier("USD", Some(pnl("-100")), None),
                account_id: account(1),
                initial_pnl: Pnl::ZERO,
            }],
            test_builder().storage_builder(),
        )
        .expect("policy must be valid");

        // apply detects breach → returns AccountBlock for account(1) USD.
        let blocks = apply_report_blocks(&policy, &report("USD", account(1), pnl("-150")));
        assert_eq!(blocks.len(), 1);
        let reject = check_start(&policy, &order("USD", account(1))).expect_err("must reject");
        assert_eq!(reject[0].code, RejectCode::PnlKillSwitchTriggered);
        assert_eq!(
            reject[0].reason,
            "pnl kill switch triggered: account + asset barrier"
        );

        // Other account, same settlement: no barrier → passes.
        assert!(check_start(&policy, &order("USD", account(2))).is_ok());
        // Same account, different settlement: no barrier → passes.
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
        PnlBoundsKillSwitchPolicy::new(
            [barrier_usd(lower_bound, upper_bound)],
            [],
            test_builder().storage_builder(),
        )
        .expect("policy must be valid")
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
        let policy: TestPolicy = PnlBoundsKillSwitchPolicy::new(
            [barrier("USD", Some(pnl("10")), None)],
            [],
            test_builder().storage_builder(),
        )
        .expect("policy must be valid");

        let reject = check_start(&policy, &order("USD", account(1))).expect_err("must reject");
        let reject = &reject[0];
        assert_eq!(reject.code, RejectCode::PnlKillSwitchTriggered);
        assert_eq!(reject.reason, "pnl kill switch triggered: broker barrier");
        assert!(reject.details.contains("lower bound breached"));
    }
}
