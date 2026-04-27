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
use std::collections::{HashMap, HashSet};
use std::fmt::{Display, Formatter};

use crate::core::{HasFee, HasInstrument, HasPnl};
use crate::param::{Asset, Pnl};
use crate::pretrade::policy::request_field_access_pre_trade_reject;
use crate::pretrade::{
    CheckPreTradeStartPolicy, PreTradeContext, Reject, RejectCode, RejectScope, Rejects,
};

/// Start-stage policy that blocks trading after crossing configured loss limits.
///
/// Tracks realized P&L per settlement asset and rejects orders when accumulated
/// losses reach the configured barrier. The kill switch stays active until
/// [`PnlKillSwitchPolicy::reset_pnl`] is called explicitly.
///
/// # Examples
///
/// ```rust
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use openpit::param::{Asset, Fee, Pnl, Price, Quantity, Side};
/// use openpit::pretrade::policies::PnlKillSwitchPolicy;
/// use openpit::pretrade::{CheckPreTradeStartPolicy, PreTradeContext};
/// use openpit::{HasFee, HasInstrument, HasPnl, Instrument, OrderOperation};
/// use openpit::param::TradeAmount;
///
/// let usd = Asset::new("USD")?;
/// let policy = PnlKillSwitchPolicy::new(
///     (usd.clone(), Pnl::from_str("500")?),
///     [],
/// )?;
///
/// // Order passes when P&L is above the barrier.
/// let order = OrderOperation {
///     instrument: Instrument::new(
///         Asset::new("AAPL")?,
///         usd.clone(),
///     ),
///     account_id: openpit::param::AccountId::from_u64(99224416),
///     side: Side::Buy,
///     trade_amount: TradeAmount::Quantity(
///         Quantity::from_str("1")?,
///     ),
///     price: Some(Price::from_str("100")?),
/// };
///
/// assert!(
///     <PnlKillSwitchPolicy as CheckPreTradeStartPolicy<
///         OrderOperation,
///         Report,
///     >>::check_pre_trade_start(&policy, &PreTradeContext::new(), &order)
///     .is_ok()
/// );
///
/// // Report a loss that crosses the barrier.
/// struct Report {
///     instrument: Instrument,
///     pnl: Pnl,
///     fee: Fee,
/// }
/// impl HasInstrument for Report {
///     fn instrument(&self) -> Result<&Instrument, openpit::RequestFieldAccessError> {
///         Ok(&self.instrument)
///     }
/// }
/// impl HasPnl for Report {
///     fn pnl(&self) -> Result<Pnl, openpit::RequestFieldAccessError> {
///         Ok(self.pnl)
///     }
/// }
/// impl HasFee for Report {
///     fn fee(&self) -> Result<Fee, openpit::RequestFieldAccessError> {
///         Ok(self.fee)
///     }
/// }
/// let report = Report {
///     instrument: Instrument::new(
///         Asset::new("AAPL")?,
///         usd.clone(),
///     ),
///     pnl: Pnl::from_str("-600")?,
///     fee: Fee::ZERO,
/// };
///
/// let triggered =
///     <PnlKillSwitchPolicy as CheckPreTradeStartPolicy<
///         OrderOperation,
///         Report,
///     >>::apply_execution_report(&policy, &report)
/// ;
/// assert!(triggered);
///
/// // Orders are now rejected until reset.
/// assert!(
///     <PnlKillSwitchPolicy as CheckPreTradeStartPolicy<
///         OrderOperation,
///         Report,
///     >>::check_pre_trade_start(&policy, &PreTradeContext::new(), &order)
///     .is_err()
/// );
///
/// policy.reset_pnl(&usd);
/// assert!(
///     <PnlKillSwitchPolicy as CheckPreTradeStartPolicy<
///         OrderOperation,
///         Report,
///     >>::check_pre_trade_start(&policy, &PreTradeContext::new(), &order)
///     .is_ok()
/// );
/// # Ok(())
/// # }
/// ```
pub struct PnlKillSwitchPolicy {
    barriers: RefCell<HashMap<Asset, Pnl>>,
    realized: RefCell<HashMap<Asset, Pnl>>,
    triggered: RefCell<HashSet<Asset>>,
}

/// Errors returned by [`PnlKillSwitchPolicy`] operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PnlKillSwitchError {
    /// Barrier must be strictly positive.
    NonPositiveBarrier { settlement: Asset, barrier: Pnl },
    /// Realized PnL accumulation overflowed.
    PnlAccumulationOverflow { settlement: Asset },
}

impl Display for PnlKillSwitchError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NonPositiveBarrier {
                settlement,
                barrier,
            } => write!(
                formatter,
                "barrier must be positive for settlement asset {settlement}, got {barrier}"
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

impl std::error::Error for PnlKillSwitchError {}

impl PnlKillSwitchPolicy {
    /// Stable policy name.
    pub const NAME: &'static str = "PnlKillSwitchPolicy";

    /// Creates a P&L kill-switch policy with at least one loss barrier.
    pub fn new(
        initial_barrier: (Asset, Pnl),
        additional_barriers: impl IntoIterator<Item = (Asset, Pnl)>,
    ) -> Result<Self, PnlKillSwitchError> {
        let (initial_settlement, initial_value) = initial_barrier;
        validate_barrier(&initial_settlement, initial_value)?;
        let mut barriers = HashMap::new();
        barriers.insert(initial_settlement, initial_value);
        for (settlement, barrier) in additional_barriers {
            validate_barrier(&settlement, barrier)?;
            barriers.insert(settlement, barrier);
        }

        Ok(Self {
            barriers: RefCell::new(barriers),
            realized: RefCell::new(HashMap::new()),
            triggered: RefCell::new(HashSet::new()),
        })
    }

    /// Sets per-settlement loss barrier.
    pub fn set_barrier(&self, settlement: &Asset, barrier: Pnl) -> Result<(), PnlKillSwitchError> {
        validate_barrier(settlement, barrier)?;
        self.barriers
            .borrow_mut()
            .insert(settlement.clone(), barrier);
        Ok(())
    }

    /// Accumulates a realized P&L delta for the given settlement asset.
    pub fn report_realized_pnl(
        &self,
        settlement: &Asset,
        pnl_delta: Pnl,
    ) -> Result<(), PnlKillSwitchError> {
        let mut realized = self.realized.borrow_mut();
        let current = realized.get(settlement).copied().unwrap_or(Pnl::ZERO);
        let updated = match current.checked_add(pnl_delta) {
            Ok(value) => value,
            Err(_) => {
                self.triggered.borrow_mut().insert(settlement.clone());
                return Err(PnlKillSwitchError::PnlAccumulationOverflow {
                    settlement: settlement.clone(),
                });
            }
        };
        realized.insert(settlement.clone(), updated);
        drop(realized);

        if self.is_threshold_crossed(settlement) {
            self.triggered.borrow_mut().insert(settlement.clone());
        }
        Ok(())
    }

    /// Resets accumulated P&L and clears kill-switch trigger for settlement asset.
    pub fn reset_pnl(&self, settlement: &Asset) {
        self.realized
            .borrow_mut()
            .insert(settlement.clone(), Pnl::ZERO);
        self.triggered.borrow_mut().remove(settlement);
    }

    /// Returns accumulated realized P&L for settlement asset.
    pub fn realized_pnl(&self, settlement: &Asset) -> Pnl {
        self.realized
            .borrow()
            .get(settlement)
            .copied()
            .unwrap_or(Pnl::ZERO)
    }

    fn is_threshold_crossed(&self, settlement: &Asset) -> bool {
        let barrier = match self.barrier(settlement) {
            Some(value) => value,
            None => return false,
        };
        let threshold = Pnl::new(-barrier.to_decimal());
        let realized = self.realized_pnl(settlement);
        realized.to_decimal() <= threshold.to_decimal()
    }

    fn is_triggered(&self, settlement: &Asset) -> bool {
        self.triggered.borrow().contains(settlement)
    }

    fn barrier(&self, settlement: &Asset) -> Option<Pnl> {
        self.barriers.borrow().get(settlement).copied()
    }
}

impl<O, R> CheckPreTradeStartPolicy<O, R> for PnlKillSwitchPolicy
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
                    "pnl barrier missing",
                    format!("settlement asset {settlement} has no configured loss barrier"),
                )
                .into());
            }
        };

        if self.is_triggered(settlement) || self.is_threshold_crossed(settlement) {
            self.triggered.borrow_mut().insert(settlement.clone());
            return Err(Reject::new(
                Self::NAME,
                RejectScope::Account,
                RejectCode::PnlKillSwitchTriggered,
                "pnl kill switch triggered",
                format!(
                    "realized pnl {}, max allowed loss: {}, settlement asset {settlement}",
                    self.realized_pnl(settlement),
                    barrier
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
            Err(_) => {
                self.triggered
                    .borrow_mut()
                    .insert(instrument.settlement_asset().clone());
                return true;
            }
        }

        let settlement = instrument.settlement_asset();
        if self.report_realized_pnl(settlement, pnl_delta).is_err() {
            self.triggered.borrow_mut().insert(settlement.clone());
        }
        self.is_triggered(settlement)
    }
}

fn validate_barrier(settlement: &Asset, barrier: Pnl) -> Result<(), PnlKillSwitchError> {
    if barrier > Pnl::ZERO {
        return Ok(());
    }

    Err(PnlKillSwitchError::NonPositiveBarrier {
        settlement: settlement.clone(),
        barrier,
    })
}

#[cfg(test)]
mod tests {
    use crate::core::{HasFee, HasInstrument, HasPnl, Instrument, OrderOperation};
    use crate::param::TradeAmount;
    use crate::param::{AccountId, Asset, Fee, Pnl, Price, Quantity, Side};
    use crate::pretrade::{CheckPreTradeStartPolicy, PreTradeContext, RejectCode, RejectScope};
    use crate::RequestFieldAccessError;
    use rust_decimal::Decimal;

    use super::{PnlKillSwitchError, PnlKillSwitchPolicy};

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
    fn happy_path_order_passes_when_pnl_above_barrier() {
        let policy = PnlKillSwitchPolicy::new(
            (
                Asset::new("USD").expect("asset code must be valid"),
                pnl("100"),
            ),
            vec![],
        )
        .expect("policy must be valid");
        policy
            .report_realized_pnl(
                &Asset::new("USD").expect("asset code must be valid"),
                pnl("-20"),
            )
            .expect("accumulation must succeed");

        let result = check_start(&policy, &order("USD"));
        assert!(result.is_ok());
    }

    #[test]
    fn boundary_triggers_when_pnl_equals_negative_barrier() {
        let policy = PnlKillSwitchPolicy::new(
            (
                Asset::new("USD").expect("asset code must be valid"),
                pnl("100"),
            ),
            vec![],
        )
        .expect("policy must be valid");
        policy
            .report_realized_pnl(
                &Asset::new("USD").expect("asset code must be valid"),
                pnl("-100"),
            )
            .expect("accumulation must succeed");

        let reject = check_start(&policy, &order("USD")).expect_err("must reject on boundary");
        let reject = &reject[0];
        assert_eq!(reject.scope, RejectScope::Account);
        assert_eq!(reject.code, RejectCode::PnlKillSwitchTriggered);
        assert_eq!(reject.reason, "pnl kill switch triggered");
        assert_eq!(
            reject.details,
            "realized pnl -100, max allowed loss: 100, settlement asset USD"
        );
    }

    #[test]
    fn missing_barrier_returns_order_reject() {
        let policy = PnlKillSwitchPolicy::new(
            (
                Asset::new("EUR").expect("asset code must be valid"),
                pnl("100"),
            ),
            vec![],
        )
        .expect("policy must be valid");

        let reject =
            check_start(&policy, &order("USD")).expect_err("must reject when barrier is missing");
        let reject = &reject[0];
        assert_eq!(reject.scope, RejectScope::Order);
        assert_eq!(reject.code, RejectCode::RiskConfigurationMissing);
        assert_eq!(reject.reason, "pnl barrier missing");
        assert_eq!(
            reject.details,
            "settlement asset USD has no configured loss barrier"
        );
    }

    #[test]
    fn accumulate_realized_pnl_is_per_settlement_asset() {
        let policy = PnlKillSwitchPolicy::new(
            (
                Asset::new("USD").expect("asset code must be valid"),
                pnl("100"),
            ),
            vec![(
                Asset::new("EUR").expect("asset code must be valid"),
                pnl("100"),
            )],
        )
        .expect("policy must be valid");

        policy
            .report_realized_pnl(
                &Asset::new("USD").expect("asset code must be valid"),
                pnl("-40"),
            )
            .expect("accumulation must succeed");
        policy
            .report_realized_pnl(
                &Asset::new("USD").expect("asset code must be valid"),
                pnl("-10"),
            )
            .expect("accumulation must succeed");
        policy
            .report_realized_pnl(
                &Asset::new("EUR").expect("asset code must be valid"),
                pnl("-20"),
            )
            .expect("accumulation must succeed");

        assert_eq!(
            policy.realized_pnl(&Asset::new("USD").expect("asset code must be valid")),
            pnl("-50")
        );
        assert_eq!(
            policy.realized_pnl(&Asset::new("EUR").expect("asset code must be valid")),
            pnl("-20")
        );
    }

    #[test]
    fn trigger_is_sticky_until_reset() {
        let policy = PnlKillSwitchPolicy::new(
            (
                Asset::new("USD").expect("asset code must be valid"),
                pnl("100"),
            ),
            vec![],
        )
        .expect("policy must be valid");
        policy
            .report_realized_pnl(
                &Asset::new("USD").expect("asset code must be valid"),
                pnl("-120"),
            )
            .expect("accumulation must succeed");

        let first = check_start(&policy, &order("USD"));
        assert!(first.is_err());

        policy
            .report_realized_pnl(
                &Asset::new("USD").expect("asset code must be valid"),
                pnl("200"),
            )
            .expect("accumulation must succeed");
        let second = check_start(&policy, &order("USD"));
        assert!(second.is_err());
    }

    #[test]
    fn reset_clears_trigger_and_resets_pnl() {
        let policy = PnlKillSwitchPolicy::new(
            (
                Asset::new("USD").expect("asset code must be valid"),
                pnl("100"),
            ),
            vec![],
        )
        .expect("policy must be valid");
        policy
            .report_realized_pnl(
                &Asset::new("USD").expect("asset code must be valid"),
                pnl("-120"),
            )
            .expect("accumulation must succeed");
        assert!(check_start(&policy, &order("USD")).is_err());

        policy.reset_pnl(&Asset::new("USD").expect("asset code must be valid"));
        assert_eq!(
            policy.realized_pnl(&Asset::new("USD").expect("asset code must be valid")),
            pnl("0")
        );
        assert!(check_start(&policy, &order("USD")).is_ok());
    }

    #[test]
    fn apply_execution_report_updates_realized_pnl_and_reports_trigger() {
        let policy = PnlKillSwitchPolicy::new(
            (
                Asset::new("USD").expect("asset code must be valid"),
                pnl("100"),
            ),
            vec![],
        )
        .expect("policy must be valid");

        let report = TestReport {
            instrument: Instrument::new(
                Asset::new("AAPL").expect("asset code must be valid"),
                Asset::new("USD").expect("asset code must be valid"),
            ),
            pnl: pnl("-120"),
            fee: Fee::ZERO,
        };
        let triggered = apply_report(&policy, &report);

        assert!(triggered);
        assert_eq!(
            policy.realized_pnl(&Asset::new("USD").expect("asset code must be valid")),
            pnl("-120")
        );
    }

    #[test]
    fn unconfigured_settlement_accumulates_but_does_not_trigger() {
        let policy = PnlKillSwitchPolicy::new(
            (
                Asset::new("EUR").expect("asset code must be valid"),
                pnl("100"),
            ),
            vec![],
        )
        .expect("policy must be valid");

        policy
            .report_realized_pnl(
                &Asset::new("USD").expect("asset code must be valid"),
                pnl("-10"),
            )
            .expect("accumulation must succeed");

        assert_eq!(
            policy.realized_pnl(&Asset::new("USD").expect("asset code must be valid")),
            pnl("-10")
        );
        let reject =
            check_start(&policy, &order("USD")).expect_err("missing barrier must still reject");
        let reject = &reject[0];
        assert_eq!(reject.code, RejectCode::RiskConfigurationMissing);
        assert_eq!(reject.reason, "pnl barrier missing");
        assert_eq!(
            reject.details,
            "settlement asset USD has no configured loss barrier"
        );
    }

    #[test]
    fn set_barrier_registers_new_settlement() {
        let policy = PnlKillSwitchPolicy::new(
            (
                Asset::new("EUR").expect("asset code must be valid"),
                pnl("100"),
            ),
            vec![],
        )
        .expect("policy must be valid");
        let usd = Asset::new("USD").expect("asset code must be valid");
        policy
            .set_barrier(&usd, pnl("50"))
            .expect("barrier must be valid");
        policy
            .report_realized_pnl(&usd, pnl("-49"))
            .expect("accumulation must succeed");

        assert!(check_start(&policy, &order("USD")).is_ok());
    }

    #[test]
    fn constructor_rejects_non_positive_barrier() {
        let settlement = Asset::new("USD").expect("asset code must be valid");
        let err = PnlKillSwitchPolicy::new((settlement.clone(), pnl("0")), vec![])
            .err()
            .expect("zero barrier must be rejected");

        assert_eq!(
            err,
            PnlKillSwitchError::NonPositiveBarrier {
                settlement,
                barrier: pnl("0"),
            }
        );
    }

    #[test]
    fn constructor_rejects_non_positive_additional_barrier() {
        let initial_settlement = Asset::new("USD").expect("asset code must be valid");
        let valid_additional_settlement = Asset::new("EUR").expect("asset code must be valid");
        let invalid_additional_settlement = Asset::new("JPY").expect("asset code must be valid");
        let err = PnlKillSwitchPolicy::new(
            (initial_settlement, pnl("100")),
            vec![
                (valid_additional_settlement, pnl("50")),
                (invalid_additional_settlement.clone(), pnl("0")),
            ],
        )
        .err()
        .expect("non-positive additional barrier must be rejected");

        assert_eq!(
            err,
            PnlKillSwitchError::NonPositiveBarrier {
                settlement: invalid_additional_settlement,
                barrier: pnl("0"),
            }
        );
    }

    #[test]
    fn set_barrier_rejects_non_positive_barrier() {
        let policy = PnlKillSwitchPolicy::new(
            (
                Asset::new("EUR").expect("asset code must be valid"),
                pnl("100"),
            ),
            vec![],
        )
        .expect("policy must be valid");
        let settlement = Asset::new("USD").expect("asset code must be valid");

        let err = policy
            .set_barrier(&settlement, pnl("-1"))
            .expect_err("negative barrier must be rejected");
        assert_eq!(
            err,
            PnlKillSwitchError::NonPositiveBarrier {
                settlement,
                barrier: pnl("-1"),
            }
        );
    }

    #[test]
    fn error_display_messages_are_stable() {
        assert_eq!(
            PnlKillSwitchError::NonPositiveBarrier {
                settlement: Asset::new("USD").expect("asset code must be valid"),
                barrier: pnl("0"),
            }
            .to_string(),
            "barrier must be positive for settlement asset USD, got 0"
        );
        assert_eq!(
            PnlKillSwitchError::PnlAccumulationOverflow {
                settlement: Asset::new("USD").expect("asset code must be valid"),
            }
            .to_string(),
            "pnl accumulation overflow for settlement asset USD"
        );
    }

    #[test]
    fn report_realized_pnl_marks_triggered_on_accumulation_overflow() {
        let settlement = Asset::new("USD").expect("asset code must be valid");
        let policy = PnlKillSwitchPolicy::new((settlement.clone(), pnl("100")), vec![])
            .expect("policy must be valid");

        policy
            .report_realized_pnl(&settlement, Pnl::new(Decimal::MAX))
            .expect("initial accumulation must succeed");

        let err = policy
            .report_realized_pnl(&settlement, Pnl::new(Decimal::MAX))
            .expect_err("overflow must be reported");
        assert_eq!(
            err,
            PnlKillSwitchError::PnlAccumulationOverflow {
                settlement: settlement.clone(),
            }
        );
        assert!(policy.is_triggered(&settlement));
    }

    #[test]
    fn apply_execution_report_marks_triggered_when_accumulation_overflows() {
        let settlement = Asset::new("USD").expect("asset code must be valid");
        let policy = PnlKillSwitchPolicy::new((settlement.clone(), pnl("100")), vec![])
            .expect("policy must be valid");

        policy
            .report_realized_pnl(&settlement, Pnl::new(Decimal::MAX))
            .expect("initial accumulation must succeed");

        let report = TestReport {
            instrument: Instrument::new(
                Asset::new("AAPL").expect("asset code must be valid"),
                settlement.clone(),
            ),
            pnl: Pnl::new(Decimal::MAX),
            fee: Fee::ZERO,
        };

        assert!(apply_report(&policy, &report));
        assert!(policy.is_triggered(&settlement));
    }

    #[test]
    fn policy_name_is_stable() {
        let policy = PnlKillSwitchPolicy::new(
            (
                Asset::new("USD").expect("asset code must be valid"),
                pnl("100"),
            ),
            vec![],
        )
        .expect("policy must be valid");

        assert_eq!(
            <PnlKillSwitchPolicy as CheckPreTradeStartPolicy<OrderOperation, TestReport>>::name(
                &policy
            ),
            PnlKillSwitchPolicy::NAME
        );
    }

    #[test]
    fn apply_execution_report_marks_triggered_when_fee_addition_overflows() {
        struct FeeOverflowReport {
            instrument: Instrument,
        }
        impl HasInstrument for FeeOverflowReport {
            fn instrument(&self) -> Result<&Instrument, crate::RequestFieldAccessError> {
                Ok(&self.instrument)
            }
        }
        impl HasPnl for FeeOverflowReport {
            fn pnl(&self) -> Result<Pnl, crate::RequestFieldAccessError> {
                Ok(Pnl::new(Decimal::MIN))
            }
        }
        impl HasFee for FeeOverflowReport {
            fn fee(&self) -> Result<Fee, crate::RequestFieldAccessError> {
                Ok(Fee::from_str("1").expect("fee must be valid"))
            }
        }

        let settlement = Asset::new("USD").expect("asset code must be valid");
        let policy = PnlKillSwitchPolicy::new((settlement.clone(), pnl("100")), vec![])
            .expect("policy must be valid");
        let report = FeeOverflowReport {
            instrument: Instrument::new(
                Asset::new("AAPL").expect("asset code must be valid"),
                settlement.clone(),
            ),
        };

        let triggered = <PnlKillSwitchPolicy as CheckPreTradeStartPolicy<
            OrderOperation,
            FeeOverflowReport,
        >>::apply_execution_report(&policy, &report);
        assert!(triggered);
        assert!(policy.is_triggered(&settlement));
    }

    #[test]
    fn threshold_crossed_returns_true_when_barrier_negation_overflows() {
        let settlement = Asset::new("USD").expect("asset code must be valid");
        let policy = PnlKillSwitchPolicy::new((settlement.clone(), pnl("100")), vec![])
            .expect("policy must be valid");
        policy
            .barriers
            .borrow_mut()
            .insert(settlement.clone(), Pnl::new(Decimal::MIN));

        assert!(policy.is_threshold_crossed(&settlement));
    }

    #[test]
    fn apply_execution_report_without_fee_uses_pnl_delta_directly() {
        struct NoFeeReport {
            instrument: Instrument,
        }
        impl HasInstrument for NoFeeReport {
            fn instrument(&self) -> Result<&Instrument, crate::RequestFieldAccessError> {
                Ok(&self.instrument)
            }
        }
        impl HasPnl for NoFeeReport {
            fn pnl(&self) -> Result<Pnl, crate::RequestFieldAccessError> {
                Ok(Pnl::from_str("-10").expect("pnl must be valid"))
            }
        }
        impl HasFee for NoFeeReport {
            fn fee(&self) -> Result<Fee, crate::RequestFieldAccessError> {
                Ok(Fee::ZERO)
            }
        }

        let settlement = Asset::new("USD").expect("asset code must be valid");
        let policy = PnlKillSwitchPolicy::new((settlement.clone(), pnl("100")), vec![])
            .expect("policy must be valid");
        let report = NoFeeReport {
            instrument: Instrument::new(
                Asset::new("AAPL").expect("asset code must be valid"),
                settlement.clone(),
            ),
        };

        let triggered = <PnlKillSwitchPolicy as CheckPreTradeStartPolicy<
            OrderOperation,
            NoFeeReport,
        >>::apply_execution_report(&policy, &report);
        assert!(!triggered);
        assert_eq!(policy.realized_pnl(&settlement), pnl("-10"));
    }

    #[test]
    fn check_pre_trade_start_maps_instrument_access_error() {
        struct InvalidOrder;

        impl HasInstrument for InvalidOrder {
            fn instrument(&self) -> Result<&Instrument, RequestFieldAccessError> {
                Err(RequestFieldAccessError::new("instrument"))
            }
        }

        let policy = PnlKillSwitchPolicy::new(
            (
                Asset::new("USD").expect("asset code must be valid"),
                pnl("100"),
            ),
            vec![],
        )
        .expect("policy must be valid");
        let reject = <PnlKillSwitchPolicy as CheckPreTradeStartPolicy<InvalidOrder, TestReport>>::check_pre_trade_start(&policy, &PreTradeContext::new(), &InvalidOrder)
            .expect_err("field access error must reject");
        let reject = &reject[0];
        assert_eq!(reject.scope, RejectScope::Order);
        assert_eq!(reject.code, RejectCode::MissingRequiredField);
        assert_eq!(reject.reason, "failed to access required field");
        assert_eq!(reject.details, "failed to access field 'instrument'");
    }

    #[test]
    fn apply_execution_report_returns_false_on_field_access_errors() {
        struct InstrumentAccessErrorReport;

        impl HasInstrument for InstrumentAccessErrorReport {
            fn instrument(&self) -> Result<&Instrument, RequestFieldAccessError> {
                Err(RequestFieldAccessError::new("instrument"))
            }
        }
        impl HasPnl for InstrumentAccessErrorReport {
            fn pnl(&self) -> Result<Pnl, RequestFieldAccessError> {
                Ok(pnl("-10"))
            }
        }
        impl HasFee for InstrumentAccessErrorReport {
            fn fee(&self) -> Result<Fee, RequestFieldAccessError> {
                Ok(Fee::ZERO)
            }
        }

        let policy = PnlKillSwitchPolicy::new(
            (
                Asset::new("USD").expect("asset code must be valid"),
                pnl("100"),
            ),
            vec![],
        )
        .expect("policy must be valid");
        let report = InstrumentAccessErrorReport;

        let triggered = <PnlKillSwitchPolicy as CheckPreTradeStartPolicy<
            OrderOperation,
            InstrumentAccessErrorReport,
        >>::apply_execution_report(&policy, &report);
        assert!(!triggered);
        assert_eq!(report.pnl(), Ok(pnl("-10")));
        assert_eq!(report.fee(), Ok(Fee::ZERO));
    }

    #[test]
    fn apply_execution_report_returns_false_when_pnl_access_fails() {
        struct PnlAccessErrorReport {
            instrument: Instrument,
        }

        impl HasInstrument for PnlAccessErrorReport {
            fn instrument(&self) -> Result<&Instrument, RequestFieldAccessError> {
                Ok(&self.instrument)
            }
        }
        impl HasPnl for PnlAccessErrorReport {
            fn pnl(&self) -> Result<Pnl, RequestFieldAccessError> {
                Err(RequestFieldAccessError::new("pnl"))
            }
        }
        impl HasFee for PnlAccessErrorReport {
            fn fee(&self) -> Result<Fee, RequestFieldAccessError> {
                Ok(Fee::ZERO)
            }
        }

        let settlement = Asset::new("USD").expect("asset code must be valid");
        let policy = PnlKillSwitchPolicy::new((settlement.clone(), pnl("100")), vec![])
            .expect("policy must be valid");
        let report = PnlAccessErrorReport {
            instrument: Instrument::new(
                Asset::new("AAPL").expect("asset code must be valid"),
                settlement,
            ),
        };

        let triggered = <PnlKillSwitchPolicy as CheckPreTradeStartPolicy<
            OrderOperation,
            PnlAccessErrorReport,
        >>::apply_execution_report(&policy, &report);
        assert!(!triggered);
        assert_eq!(report.fee(), Ok(Fee::ZERO));
    }

    #[test]
    fn apply_execution_report_returns_false_when_fee_access_fails() {
        struct FeeAccessErrorReport {
            instrument: Instrument,
        }

        impl HasInstrument for FeeAccessErrorReport {
            fn instrument(&self) -> Result<&Instrument, RequestFieldAccessError> {
                Ok(&self.instrument)
            }
        }
        impl HasPnl for FeeAccessErrorReport {
            fn pnl(&self) -> Result<Pnl, RequestFieldAccessError> {
                Ok(pnl("-10"))
            }
        }
        impl HasFee for FeeAccessErrorReport {
            fn fee(&self) -> Result<Fee, RequestFieldAccessError> {
                Err(RequestFieldAccessError::new("fee"))
            }
        }

        let settlement = Asset::new("USD").expect("asset code must be valid");
        let policy = PnlKillSwitchPolicy::new((settlement.clone(), pnl("100")), vec![])
            .expect("policy must be valid");
        let report = FeeAccessErrorReport {
            instrument: Instrument::new(
                Asset::new("AAPL").expect("asset code must be valid"),
                settlement,
            ),
        };

        let triggered = <PnlKillSwitchPolicy as CheckPreTradeStartPolicy<
            OrderOperation,
            FeeAccessErrorReport,
        >>::apply_execution_report(&policy, &report);
        assert!(!triggered);
    }

    fn check_start(
        policy: &PnlKillSwitchPolicy,
        order: &OrderOperation,
    ) -> Result<(), crate::pretrade::Rejects> {
        <PnlKillSwitchPolicy as CheckPreTradeStartPolicy<OrderOperation, TestReport>>::check_pre_trade_start(policy, &PreTradeContext::new(), order)
    }

    fn apply_report(policy: &PnlKillSwitchPolicy, report: &TestReport) -> bool {
        <PnlKillSwitchPolicy as CheckPreTradeStartPolicy<OrderOperation, TestReport>>::apply_execution_report(policy, report)
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
