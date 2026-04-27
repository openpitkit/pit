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

use super::{PreTradeContext, Rejects};

/// Start-stage pre-trade policy contract.
///
/// Start-stage policies run in [`crate::Engine::start_pre_trade`] before the
/// engine creates a deferred request. They are intended for cheap gating logic
/// such as session checks, static order validation, and stateful throttles.
///
/// Policies execute in registration order. The engine evaluates all
/// start-stage policies and merges every returned reject list before deciding
/// whether to create a deferred request. State changes performed here are not
/// rolled back by the engine.
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
/// use openpit::pretrade::{CheckPreTradeStartPolicy, PreTradeContext, Reject, RejectCode, RejectScope, Rejects};
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
///     fn name(&self) -> &str {
///         Self::NAME
///     }
///
///     fn check_pre_trade_start(&self, _ctx: &PreTradeContext, _order: &O) -> Result<(), Rejects> {
///         if !self.active.get() {
///             return Err(Rejects::new(vec![Reject::new(
///                 Self::NAME,
///                 RejectScope::Account,
///                 RejectCode::Other,
///                 "session inactive",
///                 "trading session is closed",
///             )]));
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
    fn name(&self) -> &str;

    /// Performs start-stage checks against an order.
    ///
    /// Returning `Ok(())` allows the engine to continue building the deferred
    /// request. Returning [`Rejects`] contributes rejects to the start-stage
    /// reject result.
    fn check_pre_trade_start(&self, ctx: &PreTradeContext, order: &O) -> Result<(), Rejects>;

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
    use crate::pretrade::{CheckPreTradeStartPolicy, PreTradeContext, Rejects};

    struct StartPolicyNoop;

    type TestOrder = OrderOperation;
    type TestReport = WithExecutionReportOperation<WithFinancialImpact<()>>;

    impl CheckPreTradeStartPolicy<TestOrder, TestReport> for StartPolicyNoop {
        fn name(&self) -> &str {
            "StartPolicyNoop"
        }

        fn check_pre_trade_start(
            &self,
            _ctx: &PreTradeContext,
            _order: &TestOrder,
        ) -> Result<(), Rejects> {
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
        assert!(StartPolicyNoop
            .check_pre_trade_start(&PreTradeContext::new(), &order)
            .is_ok());
    }
}
