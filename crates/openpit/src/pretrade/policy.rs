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

use super::{Context, Mutations, Reject, RejectCode, RejectScope};

/// Main-stage pre-trade policy contract.
///
/// Main-stage policies run during [`crate::pretrade::Request::execute`] after a
/// request has already passed start-stage checks. They are intended for work
/// that may need to reserve or mutate engine state and therefore must
/// participate in commit/rollback handling.
///
/// All registered policies are evaluated even when one policy already emitted a
/// reject. If any reject is produced, the engine rolls back accumulated
/// mutations in reverse order before returning the reject list to the caller.
///
/// # Rollback safety
///
/// Mutations registered during pre-trade checks may be committed or
/// rolled back after external systems have already observed intermediate
/// state (for example, a venue accepted an order based on a reserved
/// notional). Avoid absolute-value rollback in this pipeline; prefer
/// delta-based undo or capture the value to restore at registration
/// time.
///
/// `O` is the order contract type visible through [`Context`]. `R` is the
/// execution report contract type used for post-trade updates.
///
/// # Examples
///
/// ```rust
/// use openpit::pretrade::{Context, Mutations, Policy, Reject};
///
/// struct NoopPolicy;
///
/// impl<O, R> Policy<O, R> for NoopPolicy {
///     fn name(&self) -> &'static str {
///         "NoopPolicy"
///     }
///
///     fn perform_pre_trade_check(
///         &self,
///         _ctx: &Context<'_, O>,
///         _mutations: &mut Mutations,
///         _rejects: &mut Vec<Reject>,
///     ) {
///     }
///
///     fn apply_execution_report(&self, _report: &R) -> bool {
///         false
///     }
/// }
/// ```
pub trait Policy<O, R> {
    /// Stable policy name.
    ///
    /// Policy names must be unique across all policies registered in the same
    /// engine instance.
    fn name(&self) -> &'static str;

    /// Performs main-stage checks and can emit mutations or rejects.
    ///
    /// Policies may inspect the immutable request [`Context`], append mutations
    /// to be committed or rolled back later, and push one or more rejects into
    /// `rejects`.
    ///
    /// # Rollback safety
    ///
    /// In this pre-trade pipeline, rollback may happen after external systems
    /// observed intermediate reserved state. Avoid absolute-value rollback in
    /// mutations registered here; prefer delta-based undo or restore values
    /// captured at registration time.
    fn perform_pre_trade_check(
        &self,
        ctx: &Context<'_, O>,
        mutations: &mut Mutations,
        rejects: &mut Vec<Reject>,
    );

    /// Applies post-trade updates from execution reports.
    ///
    /// The engine calls this hook from [`crate::Engine::apply_execution_report`]
    /// so that a main-stage policy can maintain post-trade state.
    ///
    /// Returns `true` when this policy reports kill-switch trigger.
    fn apply_execution_report(&self, report: &R) -> bool;
}

pub(crate) fn request_field_access_reject(
    policy_name: &'static str,
    err: &crate::RequestFieldAccessError,
) -> Reject {
    Reject::new(
        policy_name,
        RejectScope::Order,
        RejectCode::MissingRequiredField,
        "failed to access required field",
        err.to_string(),
    )
}

#[cfg(test)]
mod tests {
    use crate::core::{
        ExecutionReportOperation, FinancialImpact, OrderOperation, WithExecutionReportOperation,
        WithFinancialImpact,
    };
    use crate::param::{AccountId, Asset, Fee, Pnl, Quantity, Side, TradeAmount};
    use crate::pretrade::{Reject, RejectCode, RejectScope};
    use crate::RequestFieldAccessError;

    use super::{request_field_access_reject, Context, Mutations, Policy};

    type TestOrder = OrderOperation;
    type TestReport = WithExecutionReportOperation<WithFinancialImpact<()>>;

    struct MainPolicyNoop;

    impl Policy<TestOrder, TestReport> for MainPolicyNoop {
        fn name(&self) -> &'static str {
            "MainPolicyNoop"
        }

        fn perform_pre_trade_check(
            &self,
            _ctx: &Context<'_, TestOrder>,
            _mutations: &mut Mutations,
            _rejects: &mut Vec<Reject>,
        ) {
        }

        fn apply_execution_report(&self, _report: &TestReport) -> bool {
            false
        }
    }

    #[test]
    fn apply_execution_report_hook_returns_false_for_noop_main_policy() {
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

        assert!(!MainPolicyNoop.apply_execution_report(&report));
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
        let ctx = Context::new(&order);
        let mut mutations = Mutations::new();
        let mut rejects = Vec::new();

        assert_eq!(MainPolicyNoop.name(), "MainPolicyNoop");
        MainPolicyNoop.perform_pre_trade_check(&ctx, &mut mutations, &mut rejects);
        assert!(mutations.is_empty());
        assert!(rejects.is_empty());
    }

    #[test]
    fn request_field_access_error_is_mapped_to_reject_payload() {
        let err = RequestFieldAccessError::new("instrument");
        let reject = request_field_access_reject("TestPolicy", &err);

        assert_eq!(reject.policy, "TestPolicy");
        assert_eq!(reject.scope, RejectScope::Order);
        assert_eq!(reject.code, RejectCode::MissingRequiredField);
        assert_eq!(reject.reason, "failed to access required field");
        assert_eq!(reject.details, "failed to access field 'instrument'");
    }
}
