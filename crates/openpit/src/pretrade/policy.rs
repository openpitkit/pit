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

use super::{AccountBlock, PreTradeContext, Reject, RejectCode, RejectScope, Rejects};
use crate::param::AccountId;
use crate::{AccountAdjustmentContext, Mutations};

/// Pre-trade policy contract.
///
/// Policies are registered once and can participate in any engine stage:
/// start-stage admission, main-stage reservation, post-trade feedback, and
/// account-adjustment validation. Stage hooks default to no-op behavior so a
/// policy can implement only the hooks it needs.
///
/// Start-stage hooks run in [`crate::Engine::start_pre_trade`] before the engine
/// creates a deferred request. Main-stage hooks run during
/// [`crate::pretrade::PreTradeRequest::execute`] after a request has already
/// passed start-stage checks. Account-adjustment hooks run in
/// [`crate::Engine::apply_account_adjustment`] and validate each adjustment
/// atomically before the caller applies any external effects.
///
/// All registered policies are evaluated in registration order. Stage-specific
/// rejects are merged before the engine returns to the caller.
///
/// # Rollback safety
///
/// Mutations registered during main-stage pre-trade checks may be committed or
/// rolled back after external systems have already observed intermediate state
/// (for example, a venue accepted an order based on a reserved notional). Avoid
/// absolute-value rollback in this pipeline; prefer delta-based undo or capture
/// the value to restore at registration time.
///
/// Account-adjustment hooks run within a single engine borrow. Intermediate
/// state is never visible to external systems (venues, risk aggregators), so
/// rollback by absolute value is safe there.
///
/// `O` is the order contract type visible in callbacks. `R` is the
/// execution report contract type used for post-trade updates. `A` is the
/// account-adjustment contract type visible to account-adjustment hooks.
///
/// # Examples
///
/// ```rust
/// use openpit::pretrade::{PreTradeContext, PreTradePolicy, Rejects};
/// use openpit::Mutations;
///
/// struct NoopPolicy;
///
/// impl<Order, ExecutionReport, AccountAdjustment>
///     PreTradePolicy<Order, ExecutionReport, AccountAdjustment> for NoopPolicy
/// {
///     fn name(&self) -> &str {
///         "NoopPolicy"
///     }
/// }
/// ```
pub trait PreTradePolicy<Order, ExecutionReport, AccountAdjustment = ()> {
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
    fn check_pre_trade_start(&self, _ctx: &PreTradeContext, _order: &Order) -> Result<(), Rejects> {
        Ok(())
    }

    /// Performs main-stage checks and can emit mutations or rejects.
    ///
    /// Policies may inspect the order, append mutations to be committed or
    /// rolled back later, and return one or more rejects.
    ///
    /// # Rollback safety
    ///
    /// In this pre-trade pipeline, rollback may happen after external systems
    /// observed intermediate reserved state. Avoid absolute-value rollback in
    /// mutations registered here; prefer delta-based undo or restore values
    /// captured at registration time.
    fn perform_pre_trade_check(
        &self,
        _ctx: &PreTradeContext,
        _order: &Order,
        _mutations: &mut Mutations,
    ) -> Result<(), Rejects> {
        Ok(())
    }

    /// Applies post-trade updates from execution reports.
    ///
    /// The engine calls this hook from [`crate::Engine::apply_execution_report`]
    /// so that a main-stage policy can maintain post-trade state.
    ///
    /// Returns account blocks representing the kill-switch state.
    ///
    /// An empty list means no kill-switch condition. A non-empty list means this
    /// policy entered a blocked state after the report was applied. The engine
    /// merges blocks from all policies into a single list.
    fn apply_execution_report(&self, _report: &ExecutionReport) -> Vec<AccountBlock> {
        vec![]
    }

    /// Validates a single account adjustment.
    ///
    /// `account_id` is the identifier passed to
    /// [`crate::Engine::apply_account_adjustment`].
    ///
    /// # Rollback safety
    ///
    /// In this account-adjustment pipeline, rollback by absolute value is
    /// safe because validation and mutation execution happen within a single
    /// engine borrow and no external system observes intermediate state.
    ///
    /// # Errors
    ///
    /// Returns [`Rejects`] when the adjustment violates policy constraints.
    fn apply_account_adjustment(
        &self,
        _ctx: &AccountAdjustmentContext,
        _account_id: AccountId,
        _adjustment: &AccountAdjustment,
        _mutations: &mut Mutations,
    ) -> Result<(), Rejects> {
        Ok(())
    }
}

/// Exposes a policy's stable name to error builders without coupling them to
/// the generic [`PreTradePolicy`] trait.
pub(crate) trait PolicyName {
    fn policy_name(&self) -> &str;
}

pub(crate) fn missing_required_field_reject<Policy: PolicyName + ?Sized>(
    policy: &Policy,
    field_name: &str,
    error: &crate::RequestFieldAccessError,
) -> Reject {
    Reject::new(
        policy.policy_name(),
        RejectScope::Order,
        RejectCode::MissingRequiredField,
        format!("failed to access required field '{field_name}'"),
        error.to_string(),
    )
}

pub(crate) fn missing_required_field_account_block<Policy: PolicyName + ?Sized>(
    policy: &Policy,
    field_name: &str,
    error: &crate::RequestFieldAccessError,
) -> AccountBlock {
    AccountBlock::new(
        policy.policy_name(),
        RejectCode::MissingRequiredField,
        format!("failed to access required field '{field_name}'"),
        error.to_string(),
    )
}

#[cfg(test)]
mod tests {
    use crate::core::{
        ExecutionReportOperation, FinancialImpact, OrderOperation, WithExecutionReportOperation,
        WithFinancialImpact,
    };
    use crate::param::{AccountId, Asset, Fee, Pnl, Quantity, Side, TradeAmount};
    use crate::pretrade::{RejectCode, RejectScope, Rejects};
    use crate::{Mutations, RequestFieldAccessError};

    use super::{missing_required_field_reject, PolicyName, PreTradeContext, PreTradePolicy};

    type TestOrder = OrderOperation;
    type TestReport = WithExecutionReportOperation<WithFinancialImpact<()>>;

    struct MainPolicyNoop;

    struct StartPolicyNoop;

    struct AccountAdjustmentHookNoop;

    impl PreTradePolicy<TestOrder, TestReport> for StartPolicyNoop {
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
    }

    impl PreTradePolicy<TestOrder, TestReport> for MainPolicyNoop {
        fn name(&self) -> &str {
            "MainPolicyNoop"
        }

        fn perform_pre_trade_check(
            &self,
            _ctx: &PreTradeContext,
            _order: &TestOrder,
            _mutations: &mut Mutations,
        ) -> Result<(), Rejects> {
            Ok(())
        }
    }

    impl PreTradePolicy<TestOrder, TestReport> for AccountAdjustmentHookNoop {
        fn name(&self) -> &str {
            "AccountAdjustmentHookNoop"
        }

        fn apply_account_adjustment(
            &self,
            _ctx: &crate::AccountAdjustmentContext,
            _account_id: AccountId,
            _adjustment: &(),
            _mutations: &mut Mutations,
        ) -> Result<(), Rejects> {
            Ok(())
        }
    }

    #[test]
    fn apply_execution_report_hook_returns_empty_for_noop_main_policy() {
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

        assert!(MainPolicyNoop.apply_execution_report(&report).is_empty());
    }

    #[test]
    fn apply_execution_report_hook_returns_empty_for_noop_start_policy() {
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

        assert!(StartPolicyNoop.apply_execution_report(&report).is_empty());
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
        let mut mutations = Mutations::new();

        assert_eq!(MainPolicyNoop.name(), "MainPolicyNoop");
        let result =
            MainPolicyNoop.perform_pre_trade_check(&PreTradeContext::new(), &order, &mut mutations);
        assert!(mutations.is_empty());
        assert!(result.is_ok());
    }

    #[test]
    fn required_start_trait_methods_can_be_invoked_without_side_effects() {
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

    #[test]
    fn required_account_adjustment_trait_methods_can_be_invoked_without_side_effects() {
        let mut mutations = Mutations::new();

        assert_eq!(
            AccountAdjustmentHookNoop.name(),
            "AccountAdjustmentHookNoop"
        );
        let result = AccountAdjustmentHookNoop.apply_account_adjustment(
            &crate::AccountAdjustmentContext::new(),
            AccountId::from_u64(99224416),
            &(),
            &mut mutations,
        );
        assert!(mutations.is_empty());
        assert!(result.is_ok());
    }

    #[test]
    fn missing_required_field_reject_builds_payload_from_policy_field_and_error() {
        struct TestPolicyTag;
        impl PolicyName for TestPolicyTag {
            fn policy_name(&self) -> &str {
                "TestPolicy"
            }
        }

        let error = RequestFieldAccessError::new("instrument");
        let reject = missing_required_field_reject(&TestPolicyTag, "instrument", &error);

        assert_eq!(reject.policy, "TestPolicy");
        assert_eq!(reject.scope, RejectScope::Order);
        assert_eq!(reject.code, RejectCode::MissingRequiredField);
        assert_eq!(
            reject.reason,
            "failed to access required field 'instrument'"
        );
        assert_eq!(reject.details, "failed to access field 'instrument'");
    }
}
