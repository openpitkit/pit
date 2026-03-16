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

use std::rc::Rc;

use openpit::pretrade::policies::PnlKillSwitchPolicy;
use openpit::pretrade::{CheckPreTradeStartPolicy, Reject, RejectCode, RejectScope};
use openpit::{HasFee, HasInstrument, HasPnl};

use crate::{ExecutionReportGroupAccess, OrderGroupAccess};

/// Runtime-validated wrapper around [`PnlKillSwitchPolicy`].
///
/// Checks that the order carries the operation group (for instrument
/// lookup) before the start-stage check, and that the execution report
/// carries both operation and financial-impact groups before applying
/// post-trade feedback.
pub struct GuardedPnlKillSwitch {
    inner: Rc<PnlKillSwitchPolicy>,
}

impl GuardedPnlKillSwitch {
    pub fn new(inner: Rc<PnlKillSwitchPolicy>) -> Self {
        Self { inner }
    }

    /// Returns a reference to the inner policy for direct operations
    /// like `reset_pnl`.
    pub fn inner(&self) -> &PnlKillSwitchPolicy {
        &self.inner
    }
}

impl<O, R> CheckPreTradeStartPolicy<O, R> for GuardedPnlKillSwitch
where
    O: HasInstrument + OrderGroupAccess,
    R: HasInstrument + HasPnl + HasFee + ExecutionReportGroupAccess,
{
    fn name(&self) -> &'static str {
        PnlKillSwitchPolicy::NAME
    }

    fn check_pre_trade_start(&self, order: &O) -> Result<(), Reject> {
        if !order.has_operation() {
            return Err(Reject::new(
                PnlKillSwitchPolicy::NAME,
                RejectScope::Order,
                RejectCode::MissingRequiredField,
                "insufficient order data",
                "order operation group is required for P&L kill switch evaluation",
            ));
        }
        <PnlKillSwitchPolicy as CheckPreTradeStartPolicy<O, R>>::check_pre_trade_start(
            &self.inner,
            order,
        )
    }

    fn apply_execution_report(&self, report: &R) -> bool {
        if !report.has_operation() || !report.has_financial_impact() {
            // Cannot evaluate P&L without instrument and financial data.
            // Returning false (kill switch not triggered) is the safe
            // default: the policy simply skips this incomplete report.
            return false;
        }
        <PnlKillSwitchPolicy as CheckPreTradeStartPolicy<O, R>>::apply_execution_report(
            &self.inner,
            report,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openpit::param::{Asset, Fee, Pnl, Price, Quantity, Side, TradeAmount};
    use openpit::pretrade::RejectCode;
    use openpit::{
        ExecutionReportOperation, HasFee, HasInstrument, HasPnl, Instrument, OrderOperation,
    };
    use std::rc::Rc;

    struct FakeOrder {
        operation: Option<OrderOperation>,
    }

    impl OrderGroupAccess for FakeOrder {
        fn has_operation(&self) -> bool {
            self.operation.is_some()
        }
    }

    impl HasInstrument for FakeOrder {
        fn instrument(&self) -> &Instrument {
            &self
                .operation
                .as_ref()
                .expect("internal error: test order must have operation set")
                .instrument
        }
    }

    struct FakeReport {
        operation: Option<ExecutionReportOperation>,
        has_financial: bool,
        pnl: Pnl,
        fee: Fee,
    }

    impl ExecutionReportGroupAccess for FakeReport {
        fn has_operation(&self) -> bool {
            self.operation.is_some()
        }
        fn has_financial_impact(&self) -> bool {
            self.has_financial
        }
    }

    impl HasInstrument for FakeReport {
        fn instrument(&self) -> &Instrument {
            &self
                .operation
                .as_ref()
                .expect("internal error: test report must have operation set")
                .instrument
        }
    }

    impl HasPnl for FakeReport {
        fn pnl(&self) -> Pnl {
            self.pnl
        }
    }

    impl HasFee for FakeReport {
        fn fee(&self) -> Fee {
            self.fee
        }
    }

    fn check_start(
        guard: &GuardedPnlKillSwitch,
        order: &FakeOrder,
    ) -> Result<(), openpit::pretrade::Reject> {
        <GuardedPnlKillSwitch as CheckPreTradeStartPolicy<FakeOrder, FakeReport>>::check_pre_trade_start(guard, order)
    }

    fn apply_report(guard: &GuardedPnlKillSwitch, report: &FakeReport) -> bool {
        <GuardedPnlKillSwitch as CheckPreTradeStartPolicy<FakeOrder, FakeReport>>::apply_execution_report(guard, report)
    }

    fn usd() -> Asset {
        Asset::new("USD").expect("valid")
    }

    fn sample_order(settlement: &str) -> OrderOperation {
        OrderOperation {
            instrument: Instrument::new(
                Asset::new("AAPL").expect("valid"),
                Asset::new(settlement).expect("valid"),
            ),
            side: Side::Buy,
            trade_amount: TradeAmount::Quantity(Quantity::from_str("1").expect("valid")),
            price: Some(Price::from_str("100").expect("valid")),
        }
    }

    fn make_guard() -> GuardedPnlKillSwitch {
        let policy = PnlKillSwitchPolicy::new((usd(), Pnl::from_str("500").expect("valid")), [])
            .expect("policy must be valid");
        GuardedPnlKillSwitch::new(Rc::new(policy))
    }

    #[test]
    fn rejects_when_order_operation_missing() {
        let guard = make_guard();
        let order = FakeOrder { operation: None };
        let reject =
            check_start(&guard, &order).expect_err("must reject when operation group is absent");
        assert_eq!(reject.code, RejectCode::MissingRequiredField);
    }

    #[test]
    fn delegates_when_groups_present() {
        let guard = make_guard();
        let order = FakeOrder {
            operation: Some(sample_order("USD")),
        };
        assert!(check_start(&guard, &order).is_ok());
    }

    #[test]
    fn name_is_pnl_kill_switch_policy_name() {
        let guard = make_guard();
        assert_eq!(
            <GuardedPnlKillSwitch as CheckPreTradeStartPolicy<FakeOrder, FakeReport>>::name(&guard),
            PnlKillSwitchPolicy::NAME
        );
    }

    #[test]
    fn inner_returns_reference_to_inner_policy() {
        let guard = make_guard();
        let _inner: &PnlKillSwitchPolicy = guard.inner();
    }

    #[test]
    fn skips_apply_when_only_operation_missing() {
        let guard = make_guard();
        let report = FakeReport {
            operation: None,
            has_financial: true,
            pnl: Pnl::from_str("-600").expect("valid"),
            fee: Fee::ZERO,
        };
        assert!(!apply_report(&guard, &report));
    }

    #[test]
    fn skips_apply_when_only_financial_missing() {
        let guard = make_guard();
        let report = FakeReport {
            operation: Some(ExecutionReportOperation {
                instrument: Instrument::new(
                    Asset::new("AAPL").expect("valid"),
                    Asset::new("USD").expect("valid"),
                ),
                side: Side::Buy,
            }),
            has_financial: false,
            pnl: Pnl::from_str("-600").expect("valid"),
            fee: Fee::ZERO,
        };
        assert!(!apply_report(&guard, &report));
    }

    #[test]
    fn skips_apply_when_report_groups_missing() {
        let guard = make_guard();
        let report = FakeReport {
            operation: None,
            has_financial: false,
            pnl: Pnl::from_str("-600").expect("valid"),
            fee: Fee::ZERO,
        };
        assert!(!apply_report(&guard, &report));
    }

    #[test]
    fn apply_delegates_when_both_groups_present() {
        let guard = make_guard();
        let report = FakeReport {
            operation: Some(ExecutionReportOperation {
                instrument: Instrument::new(
                    Asset::new("AAPL").expect("valid"),
                    Asset::new("USD").expect("valid"),
                ),
                side: Side::Buy,
            }),
            has_financial: true,
            pnl: Pnl::from_str("100").expect("valid"),
            fee: Fee::ZERO,
        };
        let result = apply_report(&guard, &report);
        assert!(!result, "kill switch should not trigger on a gain");
    }
}
