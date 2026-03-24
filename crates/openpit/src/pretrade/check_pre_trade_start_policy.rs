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

use super::Reject;

/// Start-stage pre-trade policy contract.
///
/// Start-stage policies run in [`crate::Engine::start_pre_trade`] before the
/// engine creates a deferred request. They are intended for cheap gating logic
/// such as session checks, static order validation, and stateful throttles.
///
/// Policies execute in registration order. The engine stops on the first
/// reject, does not evaluate remaining start-stage policies, and does not roll
/// back any state changes performed here.
///
/// `O` is the order contract type seen by the policy. `R` is the execution
/// report contract type that will later be fed back through
/// [`CheckPreTradeStartPolicy::apply_execution_report`].
///
/// # Examples
///
/// ```rust
/// use std::cell::Cell;
///
/// use openpit::pretrade::{CheckPreTradeStartPolicy, Reject, RejectCode, RejectScope};
///
/// struct SessionPolicy {
///     active: Cell<bool>,
/// }
///
/// impl SessionPolicy {
///     const NAME: &'static str = "SessionPolicy";
/// }
///
/// impl<O, R> CheckPreTradeStartPolicy<O, R> for SessionPolicy {
///     fn name(&self) -> &'static str {
///         Self::NAME
///     }
///
///     fn check_pre_trade_start(&self, _order: &O) -> Result<(), Reject> {
///         if !self.active.get() {
///             return Err(Reject::new(
///                 Self::NAME,
///                 RejectScope::Account,
///                 RejectCode::Other,
///                 "session inactive",
///                 "trading session is closed",
///             ));
///         }
///         Ok(())
///     }
///
///     fn apply_execution_report(&self, _report: &R) -> bool {
///         false
///     }
/// }
/// ```
pub trait CheckPreTradeStartPolicy<O, R> {
    /// Stable policy name.
    ///
    /// Policy names must be unique across all policies registered in the same
    /// engine instance.
    fn name(&self) -> &'static str;

    /// Performs start-stage checks against an immutable order.
    ///
    /// Returning `Ok(())` allows the engine to continue building the deferred
    /// request. Returning [`Reject`] aborts the start stage immediately.
    fn check_pre_trade_start(&self, order: &O) -> Result<(), Reject>;

    /// Applies post-trade updates from execution reports.
    ///
    /// The engine calls this hook from [`crate::Engine::apply_execution_report`]
    /// so that a start-stage policy can maintain state from realized outcomes.
    ///
    /// Returns `true` when this policy reports kill-switch trigger.
    fn apply_execution_report(&self, report: &R) -> bool;
}

#[cfg(test)]
mod tests {
    use crate::core::{
        ExecutionReportOperation, FinancialImpact, OrderOperation, WithExecutionReportOperation,
        WithFinancialImpact,
    };
    use crate::param::{AccountId, Asset, Fee, Pnl, Quantity, Side, TradeAmount};
    use crate::pretrade::{CheckPreTradeStartPolicy, Reject};

    struct StartPolicyNoop;

    type TestOrder = OrderOperation;
    type TestReport = WithExecutionReportOperation<WithFinancialImpact<()>>;

    impl CheckPreTradeStartPolicy<TestOrder, TestReport> for StartPolicyNoop {
        fn name(&self) -> &'static str {
            "StartPolicyNoop"
        }

        fn check_pre_trade_start(&self, _order: &TestOrder) -> Result<(), Reject> {
            Ok(())
        }

        fn apply_execution_report(&self, _report: &TestReport) -> bool {
            false
        }
    }

    #[test]
    fn apply_execution_report_hook_returns_false_for_noop_start_policy() {
        let report = WithExecutionReportOperation {
            inner: WithFinancialImpact {
                inner: (),
                financial_impact: FinancialImpact {
                    pnl: Pnl::from_str("0").expect("pnl must be valid"),
                    fee: Fee::ZERO,
                },
            },
            operation: ExecutionReportOperation {
                instrument: crate::Instrument::new(
                    Asset::new("AAPL").expect("asset code must be valid"),
                    Asset::new("USD").expect("asset code must be valid"),
                ),
                account_id: AccountId::from_u64(99224416),
                side: Side::Buy,
            },
        };

        assert!(!StartPolicyNoop.apply_execution_report(&report));
    }

    #[test]
    fn required_trait_methods_can_be_invoked_without_side_effects() {
        let order = OrderOperation {
            instrument: crate::Instrument::new(
                Asset::new("AAPL").expect("asset code must be valid"),
                Asset::new("USD").expect("asset code must be valid"),
            ),
            account_id: AccountId::from_u64(99224416),
            side: Side::Buy,
            trade_amount: TradeAmount::Quantity(
                Quantity::from_str("1").expect("quantity must be valid"),
            ),
            price: None,
        };

        assert_eq!(StartPolicyNoop.name(), "StartPolicyNoop");
        assert!(StartPolicyNoop.check_pre_trade_start(&order).is_ok());
    }
}
