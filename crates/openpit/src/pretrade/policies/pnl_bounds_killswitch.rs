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

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};

use crate::core::{HasFee, HasInstrument, HasPnl};
use crate::param::{Asset, Pnl};
use crate::pretrade::policy::request_field_access_pre_trade_reject;
use crate::pretrade::{
    CheckPreTradeStartPolicy, PreTradeContext, Reject, RejectCode, RejectScope, Rejects,
};

/// Per-settlement P&L bounds configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PnlBoundsBarrier {
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
    /// Initial accumulated P&L for the settlement asset.
    pub initial_pnl: Pnl,
}

/// Start-stage policy that blocks trading when accumulated P&L is outside a
/// configured per-settlement band.
///
/// This policy tracks accumulated P&L per settlement asset, per account-scoped
/// engine instance.
///
/// Constructor rules:
/// - at least one of `lower_bound` or `upper_bound` must be configured for each
///   barrier;
/// - constructor does not validate signs of bounds;
/// - constructor does not validate ordering (`lower_bound <= upper_bound`);
/// - constructor does not validate whether `initial_pnl` starts inside the
///   configured band.
///
/// Runtime notes:
/// - if `initial_pnl` is outside the band at construction, the very first
///   `start_pre_trade` is rejected with `RejectCode::PnlKillSwitchTriggered`;
/// - if `lower_bound > upper_bound`, every `start_pre_trade` is rejected until
///   `apply_execution_report` moves accumulated P&L inside the bounds or the
///   engine is rebuilt.
pub struct PnlBoundsKillSwitchPolicy {
    barriers: RefCell<HashMap<Asset, PnlBoundsBarrier>>,
    realized: RefCell<HashMap<Asset, Pnl>>,
}

/// Errors returned by [`PnlBoundsKillSwitchPolicy`] operations.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PnlBoundsKillSwitchPolicyError {
    /// Both lower and upper bounds are omitted for one settlement asset.
    NoBoundsConfigured { settlement_asset: Asset },
    /// Realized PnL accumulation overflowed.
    PnlAccumulationOverflow { settlement: Asset },
}

impl Display for PnlBoundsKillSwitchPolicyError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoBoundsConfigured { settlement_asset } => write!(
                formatter,
                "at least one of lower_bound or upper_bound must be configured for settlement asset {settlement_asset}"
            ),
            Self::PnlAccumulationOverflow { settlement } => {
                write!(
                    formatter,
                    "pnl accumulation overflow for settlement asset {settlement}"
                )
            }
        }
    }
}

impl std::error::Error for PnlBoundsKillSwitchPolicyError {}

impl PnlBoundsKillSwitchPolicy {
    /// Stable policy name.
    pub const NAME: &'static str = "PnlBoundsKillSwitchPolicy";

    /// Creates a P&L bounds kill-switch policy with at least one barrier.
    pub fn new(
        initial_barrier: PnlBoundsBarrier,
        additional_barriers: impl IntoIterator<Item = PnlBoundsBarrier>,
    ) -> Result<Self, PnlBoundsKillSwitchPolicyError> {
        let mut barriers = HashMap::new();
        let mut realized = HashMap::new();

        Self::insert_barrier(&mut barriers, &mut realized, initial_barrier)?;
        for barrier in additional_barriers {
            Self::insert_barrier(&mut barriers, &mut realized, barrier)?;
        }

        Ok(Self {
            barriers: RefCell::new(barriers),
            realized: RefCell::new(realized),
        })
    }

    /// Adds or replaces settlement bounds.
    ///
    /// Replacing an existing barrier also resets the accumulated P&L for that
    /// settlement asset to `initial_pnl` from the new barrier.
    pub fn set_barrier(
        &self,
        barrier: PnlBoundsBarrier,
    ) -> Result<(), PnlBoundsKillSwitchPolicyError> {
        Self::validate_barrier(&barrier)?;
        self.realized
            .borrow_mut()
            .insert(barrier.settlement_asset.clone(), barrier.initial_pnl);
        self.barriers
            .borrow_mut()
            .insert(barrier.settlement_asset.clone(), barrier);
        Ok(())
    }

    /// Accumulates a realized P&L delta for the given settlement asset.
    pub fn report_realized_pnl(
        &self,
        settlement: &Asset,
        pnl_delta: Pnl,
    ) -> Result<(), PnlBoundsKillSwitchPolicyError> {
        let current = self.realized_pnl(settlement);
        let updated = match current.checked_add(pnl_delta) {
            Ok(value) => value,
            Err(_) => {
                return Err(PnlBoundsKillSwitchPolicyError::PnlAccumulationOverflow {
                    settlement: settlement.clone(),
                });
            }
        };
        self.realized
            .borrow_mut()
            .insert(settlement.clone(), updated);
        Ok(())
    }

    /// Resets accumulated P&L for settlement asset.
    ///
    /// If the settlement asset has configured bounds, resets to that barrier's
    /// `initial_pnl`; otherwise resets to zero.
    pub fn reset_pnl(&self, settlement: &Asset) {
        let reset_value = self
            .barrier(settlement)
            .map(|barrier| barrier.initial_pnl)
            .unwrap_or(Pnl::ZERO);
        self.realized
            .borrow_mut()
            .insert(settlement.clone(), reset_value);
    }

    /// Returns accumulated realized P&L for settlement asset.
    pub fn realized_pnl(&self, settlement: &Asset) -> Pnl {
        self.realized
            .borrow()
            .get(settlement)
            .copied()
            .unwrap_or(Pnl::ZERO)
    }

    fn insert_barrier(
        barriers: &mut HashMap<Asset, PnlBoundsBarrier>,
        realized: &mut HashMap<Asset, Pnl>,
        barrier: PnlBoundsBarrier,
    ) -> Result<(), PnlBoundsKillSwitchPolicyError> {
        Self::validate_barrier(&barrier)?;
        realized.insert(barrier.settlement_asset.clone(), barrier.initial_pnl);
        barriers.insert(barrier.settlement_asset.clone(), barrier);
        Ok(())
    }

    fn validate_barrier(barrier: &PnlBoundsBarrier) -> Result<(), PnlBoundsKillSwitchPolicyError> {
        if barrier.lower_bound.is_none() && barrier.upper_bound.is_none() {
            return Err(PnlBoundsKillSwitchPolicyError::NoBoundsConfigured {
                settlement_asset: barrier.settlement_asset.clone(),
            });
        }
        Ok(())
    }

    fn barrier(&self, settlement: &Asset) -> Option<PnlBoundsBarrier> {
        self.barriers.borrow().get(settlement).cloned()
    }

    fn breach_sides(&self, settlement: &Asset) -> Option<Vec<&'static str>> {
        let barrier = self.barrier(settlement)?;
        let realized = self.realized_pnl(settlement);
        let mut breached = Vec::new();

        if let Some(lower_bound) = barrier.lower_bound {
            if realized < lower_bound {
                breached.push("lower");
            }
        }
        if let Some(upper_bound) = barrier.upper_bound {
            if realized > upper_bound {
                breached.push("upper");
            }
        }

        Some(breached)
    }

    fn is_outside_bounds(&self, settlement: &Asset) -> bool {
        match self.breach_sides(settlement) {
            Some(sides) => !sides.is_empty(),
            None => false,
        }
    }
}

impl<O, R> CheckPreTradeStartPolicy<O, R> for PnlBoundsKillSwitchPolicy
where
    O: HasInstrument,
    R: HasInstrument + HasPnl + HasFee,
{
    fn name(&self) -> &str {
        Self::NAME
    }

    fn check_pre_trade_start(&self, _ctx: &PreTradeContext, order: &O) -> Result<(), Rejects> {
        let instrument = order
            .instrument()
            .map_err(|e| Rejects::from(request_field_access_pre_trade_reject(Self::NAME, &e)))?;

        let settlement = instrument.settlement_asset();
        let barrier = match self.barrier(settlement) {
            Some(value) => value,
            None => {
                return Err(Reject::new(
                    Self::NAME,
                    RejectScope::Order,
                    RejectCode::RiskConfigurationMissing,
                    "pnl bounds barrier missing",
                    format!("settlement asset {settlement} has no configured pnl bounds barrier"),
                )
                .into());
            }
        };

        let breached_sides = self.breach_sides(settlement).unwrap_or_default();
        if !breached_sides.is_empty() {
            let breach_description = breached_sides.join(" and ");
            return Err(Reject::new(
                Self::NAME,
                RejectScope::Account,
                RejectCode::PnlKillSwitchTriggered,
                "pnl kill switch triggered",
                format!(
                    "{breach_description} bound breached: realized pnl {}, lower_bound {:?}, upper_bound {:?}, settlement asset {settlement}",
                    self.realized_pnl(settlement),
                    barrier.lower_bound,
                    barrier.upper_bound,
                ),
            )
            .into());
        }

        Ok(())
    }

    /// Applies a post-trade report to the accumulated realized P&L.
    ///
    /// The report contract expects `pnl` plus explicit `fee`.
    ///
    /// The engine adds fee impact to `pnl` before accumulation.
    fn apply_execution_report(&self, report: &R) -> bool {
        let instrument = match report.instrument() {
            Ok(i) => i,
            Err(_) => return false,
        };
        let mut pnl_delta = match report.pnl() {
            Ok(p) => p,
            Err(_) => return false,
        };
        let fee = match report.fee() {
            Ok(f) => f,
            Err(_) => return false,
        };
        match pnl_delta.checked_add(fee.to_pnl()) {
            Ok(value) => pnl_delta = value,
            Err(_) => return true,
        }

        let settlement = instrument.settlement_asset();
        if self.report_realized_pnl(settlement, pnl_delta).is_err() {
            return true;
        }

        self.is_outside_bounds(settlement)
    }
}

#[cfg(test)]
mod tests {
    use crate::core::{HasFee, HasInstrument, HasPnl, Instrument, OrderOperation};
    use crate::param::TradeAmount;
    use crate::param::{AccountId, Asset, Fee, Pnl, Price, Quantity, Side};
    use crate::pretrade::{CheckPreTradeStartPolicy, PreTradeContext, RejectCode, RejectScope};
    use crate::RequestFieldAccessError;
    use rust_decimal::Decimal;

    use super::{PnlBoundsBarrier, PnlBoundsKillSwitchPolicy, PnlBoundsKillSwitchPolicyError};

    struct TestReport {
        instrument: Instrument,
        pnl: Pnl,
        fee: Fee,
    }

    impl HasInstrument for TestReport {
        fn instrument(&self) -> Result<&Instrument, crate::RequestFieldAccessError> {
            Ok(&self.instrument)
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

    #[test]
    fn happy_path_order_passes_inside_bounds() {
        let policy = policy_usd(Some(pnl("-100")), Some(pnl("50")), pnl("-10"));
        let result = check_start(&policy, &order("USD"));
        assert!(result.is_ok());
    }

    #[test]
    fn lower_bound_breach_rejects_with_lower_side() {
        let policy = policy_usd(Some(pnl("-100")), Some(pnl("50")), pnl("-101"));
        let reject = check_start(&policy, &order("USD")).expect_err("must reject");
        let reject = &reject[0];
        assert_eq!(reject.scope, RejectScope::Account);
        assert_eq!(reject.code, RejectCode::PnlKillSwitchTriggered);
        assert!(reject.details.contains("lower bound breached"));
    }

    #[test]
    fn upper_bound_breach_rejects_with_upper_side() {
        let policy = policy_usd(Some(pnl("-100")), Some(pnl("50")), pnl("51"));
        let reject = check_start(&policy, &order("USD")).expect_err("must reject");
        let reject = &reject[0];
        assert_eq!(reject.scope, RejectScope::Account);
        assert_eq!(reject.code, RejectCode::PnlKillSwitchTriggered);
        assert!(reject.details.contains("upper bound breached"));
    }

    #[test]
    fn missing_bounds_rejected_by_constructor() {
        let usd = Asset::new("USD").expect("asset code must be valid");
        let err = match PnlBoundsKillSwitchPolicy::new(
            PnlBoundsBarrier {
                settlement_asset: usd.clone(),
                lower_bound: None,
                upper_bound: None,
                initial_pnl: pnl("0"),
            },
            vec![],
        ) {
            Ok(_) => panic!("must fail"),
            Err(err) => err,
        };

        assert_eq!(
            err,
            PnlBoundsKillSwitchPolicyError::NoBoundsConfigured {
                settlement_asset: usd,
            }
        );
    }

    #[test]
    fn constructor_does_not_validate_ordering_and_first_check_rejects_if_outside() {
        let policy = policy_usd(Some(pnl("10")), Some(pnl("5")), pnl("7"));
        let reject = check_start(&policy, &order("USD")).expect_err("must reject");
        let reject = &reject[0];
        assert_eq!(reject.code, RejectCode::PnlKillSwitchTriggered);
        assert!(reject.details.contains("lower and upper bound breached"));
    }

    #[test]
    fn missing_settlement_bounds_returns_risk_configuration_missing() {
        let policy = policy_usd(Some(pnl("-100")), Some(pnl("50")), pnl("0"));
        let reject = check_start(&policy, &order("EUR")).expect_err("must reject");
        let reject = &reject[0];
        assert_eq!(reject.scope, RejectScope::Order);
        assert_eq!(reject.code, RejectCode::RiskConfigurationMissing);
        assert_eq!(reject.reason, "pnl bounds barrier missing");
    }

    #[test]
    fn apply_execution_report_updates_realized_pnl_and_reports_trigger_state() {
        let policy = policy_usd(Some(pnl("-100")), Some(pnl("50")), pnl("0"));
        let report = TestReport {
            instrument: Instrument::new(
                Asset::new("AAPL").expect("asset code must be valid"),
                Asset::new("USD").expect("asset code must be valid"),
            ),
            pnl: pnl("55"),
            fee: Fee::ZERO,
        };

        let triggered = apply_report(&policy, &report);
        assert!(triggered);
        assert_eq!(
            policy.realized_pnl(&Asset::new("USD").expect("asset code must be valid")),
            pnl("55")
        );
    }

    #[test]
    fn set_barrier_replaces_initial_pnl_for_settlement() {
        let policy = policy_usd(Some(pnl("-100")), Some(pnl("50")), pnl("10"));
        let usd = Asset::new("USD").expect("asset code must be valid");

        policy
            .set_barrier(PnlBoundsBarrier {
                settlement_asset: usd.clone(),
                lower_bound: Some(pnl("-200")),
                upper_bound: Some(pnl("100")),
                initial_pnl: pnl("33"),
            })
            .expect("reconfiguration must pass");

        assert_eq!(policy.realized_pnl(&usd), pnl("33"));
    }

    #[test]
    fn reset_pnl_resets_to_barrier_initial_pnl() {
        let policy = policy_usd(Some(pnl("-100")), Some(pnl("50")), pnl("20"));
        let usd = Asset::new("USD").expect("asset code must be valid");

        policy
            .report_realized_pnl(&usd, pnl("-15"))
            .expect("accumulation must pass");
        assert_eq!(policy.realized_pnl(&usd), pnl("5"));

        policy.reset_pnl(&usd);
        assert_eq!(policy.realized_pnl(&usd), pnl("20"));
    }

    #[test]
    fn report_realized_pnl_reports_overflow() {
        let usd = Asset::new("USD").expect("asset code must be valid");
        let policy = policy_usd(Some(pnl("-100")), Some(pnl("50")), pnl("0"));

        policy
            .report_realized_pnl(&usd, Pnl::new(Decimal::MAX))
            .expect("initial accumulation must pass");
        let err = policy
            .report_realized_pnl(&usd, Pnl::new(Decimal::MAX))
            .expect_err("must overflow");

        assert_eq!(
            err,
            PnlBoundsKillSwitchPolicyError::PnlAccumulationOverflow { settlement: usd }
        );
    }

    #[test]
    fn check_pre_trade_start_maps_instrument_access_error() {
        struct InvalidOrder;

        impl HasInstrument for InvalidOrder {
            fn instrument(&self) -> Result<&Instrument, RequestFieldAccessError> {
                Err(RequestFieldAccessError::new("instrument"))
            }
        }

        let policy = policy_usd(Some(pnl("-100")), Some(pnl("50")), pnl("0"));
        let reject = <PnlBoundsKillSwitchPolicy as CheckPreTradeStartPolicy<
            InvalidOrder,
            TestReport,
        >>::check_pre_trade_start(
            &policy, &PreTradeContext::new(), &InvalidOrder
        )
        .expect_err("field access error must reject");
        let reject = &reject[0];
        assert_eq!(reject.scope, RejectScope::Order);
        assert_eq!(reject.code, RejectCode::MissingRequiredField);
        assert_eq!(reject.reason, "failed to access required field");
        assert_eq!(reject.details, "failed to access field 'instrument'");
    }

    fn check_start(
        policy: &PnlBoundsKillSwitchPolicy,
        order: &OrderOperation,
    ) -> Result<(), crate::pretrade::Rejects> {
        <PnlBoundsKillSwitchPolicy as CheckPreTradeStartPolicy<OrderOperation, TestReport>>::check_pre_trade_start(policy, &PreTradeContext::new(), order)
    }

    fn apply_report(policy: &PnlBoundsKillSwitchPolicy, report: &TestReport) -> bool {
        <PnlBoundsKillSwitchPolicy as CheckPreTradeStartPolicy<OrderOperation, TestReport>>::apply_execution_report(policy, report)
    }

    fn policy_usd(
        lower_bound: Option<Pnl>,
        upper_bound: Option<Pnl>,
        initial_pnl: Pnl,
    ) -> PnlBoundsKillSwitchPolicy {
        PnlBoundsKillSwitchPolicy::new(
            PnlBoundsBarrier {
                settlement_asset: Asset::new("USD").expect("asset code must be valid"),
                lower_bound,
                upper_bound,
                initial_pnl,
            },
            vec![],
        )
        .expect("policy must be valid")
    }

    fn order(settlement: &str) -> OrderOperation {
        OrderOperation {
            instrument: Instrument::new(
                Asset::new("AAPL").expect("asset code must be valid"),
                Asset::new(settlement).expect("asset code must be valid"),
            ),
            account_id: AccountId::from_u64(99224416),
            side: Side::Buy,
            trade_amount: TradeAmount::Quantity(
                Quantity::from_str("1").expect("quantity literal must be valid"),
            ),
            price: Some(Price::from_str("100").expect("price literal must be valid")),
        }
    }

    fn pnl(value: &str) -> crate::param::Pnl {
        crate::param::Pnl::from_str(value).expect("pnl literal must be valid")
    }
}
