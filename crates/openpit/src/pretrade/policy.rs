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

pub use crate::core::{PolicyGroupId, DEFAULT_POLICY_GROUP_ID};

use super::{
    AccountBlock, PolicyAccountAdjustmentResult, PolicyConfigurationResult, PolicyPreTradeResult,
    PolicyRuntimeConfiguration, PostTradeContext, PostTradeResult, PreTradeContext, Reject,
    RejectCode, RejectScope, Rejects,
};
use crate::core::SyncMode;
use crate::param::AccountId;
use crate::{AccountAdjustmentContext, Mutations};

// Note: each built-in policy carries its own `group_id: PolicyGroupId` field plus a
// `with_policy_group_id` fluent setter and a `fn policy_group_id(&self) -> PolicyGroupId` trait
// override. A declarative macro was considered to factor out only the getter
// (the setter cannot be macroified because it returns `Self` and the concrete
// type varies), but the saving would be three lines per struct while adding
// the macro definition plus the call site. Furthermore, every policy embeds
// the getter inside a generic `impl<O, E, A, ...> PreTradePolicy<...>` block,
// so a macro would either need to wrap the entire impl block or duplicate the
// generics at each call site. The boilerplate is therefore kept explicit so
// every policy structure stays self-contained and trivially inspectable.

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
/// `Order` is the order contract type visible in callbacks. `ExecutionReport` is the
/// execution report contract type used for post-trade updates. `AccountAdjustment` is the
/// account-adjustment contract type visible to account-adjustment hooks.
///
/// # Examples
///
/// ```rust
/// use openpit::pretrade::{PreTradeContext, PreTradePolicy, Rejects};
/// use openpit::SyncMode;
///
/// struct NoopPolicy;
///
/// impl<Order, ExecutionReport, AccountAdjustment, Sync>
///     PreTradePolicy<Order, ExecutionReport, AccountAdjustment, Sync> for NoopPolicy
/// where
///     Sync: SyncMode,
/// {
///     fn name(&self) -> &str {
///         "NoopPolicy"
///     }
/// }
/// ```
pub trait PreTradePolicy<Order, ExecutionReport, AccountAdjustment, Sync>
where
    Sync: SyncMode + ?Sized,
{
    /// Stable policy name.
    ///
    /// Policy names must be unique across all policies registered in the same
    /// engine instance.
    fn name(&self) -> &str;

    /// Returns the group tag assigned to this policy instance.
    ///
    /// The engine embeds this value in every [`crate::AccountAdjustmentOutcome`]
    /// produced by this policy so callers can filter or route outcomes without
    /// inspecting policy names. A single tag may be shared by multiple policies
    /// to form a logical group.
    ///
    /// The default implementation returns [`DEFAULT_POLICY_GROUP_ID`].
    fn policy_group_id(&self) -> PolicyGroupId {
        DEFAULT_POLICY_GROUP_ID
    }

    #[doc(hidden)]
    #[allow(private_interfaces)]
    // Built-in policies override this to register their runtime settings cell.
    fn built_in_config_entry(
        &self,
    ) -> Option<crate::core::ConfigEntry<<Sync as SyncMode>::StorageLockingPolicyFactory>> {
        None
    }

    #[doc(hidden)]
    fn apply_runtime_configuration(
        &self,
        _configuration: PolicyRuntimeConfiguration,
    ) -> PolicyConfigurationResult {
        PolicyConfigurationResult::default()
    }

    /// Performs start-stage checks against an order.
    ///
    /// Returning `Ok(())` allows the engine to continue building the deferred
    /// request. Returning [`Rejects`] contributes rejects to the start-stage
    /// reject result.
    fn check_pre_trade_start(
        &self,
        _ctx: &PreTradeContext<<Sync as SyncMode>::StorageLockingPolicyFactory>,
        _order: &Order,
    ) -> Result<(), Rejects> {
        Ok(())
    }

    /// Performs a non-mutating start-stage check for a pre-trade dry-run.
    ///
    /// A dry-run reports the verdict the order *would* receive without moving
    /// any engine state. The engine invokes this hook instead of
    /// [`Self::check_pre_trade_start`] on the dry-run path and never records the
    /// account block a real start-stage reject would latch.
    ///
    /// The default implementation delegates to [`Self::check_pre_trade_start`],
    /// which is correct for any policy whose start-stage hook produces no
    /// immediate side effect. A policy whose [`Self::check_pre_trade_start`]
    /// mutates immediately (for example, consuming a rate-limit budget) MUST
    /// override this method with a read-only variant that returns the same
    /// verdict without the mutation.
    fn check_pre_trade_start_dry_run(
        &self,
        ctx: &PreTradeContext<<Sync as SyncMode>::StorageLockingPolicyFactory>,
        order: &Order,
    ) -> Result<(), Rejects> {
        self.check_pre_trade_start(ctx, order)
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
        _ctx: &PreTradeContext<<Sync as SyncMode>::StorageLockingPolicyFactory>,
        _order: &Order,
        _mutations: &mut Mutations,
    ) -> Result<Option<PolicyPreTradeResult>, Rejects> {
        Ok(None)
    }

    /// Performs a non-mutating main-stage check for a pre-trade dry-run.
    ///
    /// A dry-run reports the verdict and the lock / account adjustments the
    /// order *would* produce, without moving any engine state. The engine
    /// invokes this hook instead of [`Self::perform_pre_trade_check`] on the
    /// dry-run path and never commits the mutations collected there - they are
    /// dropped, never committed and never rolled back.
    ///
    /// The default implementation delegates to
    /// [`Self::perform_pre_trade_check`], which is correct for any policy whose
    /// main-stage hook produces no immediate side effect (it only registers
    /// commit/rollback mutations that stay inert until finalized). A policy
    /// whose [`Self::perform_pre_trade_check`] mutates storage immediately (for
    /// example, applying a hold before registering its delta rollback) MUST
    /// override this method with a read-only emulation that reports the same
    /// verdict, lock prices, and outcome entries while pushing nothing to
    /// `mutations` and touching no storage.
    fn perform_pre_trade_check_dry_run(
        &self,
        ctx: &PreTradeContext<<Sync as SyncMode>::StorageLockingPolicyFactory>,
        order: &Order,
        mutations: &mut Mutations,
    ) -> Result<Option<PolicyPreTradeResult>, Rejects> {
        self.perform_pre_trade_check(ctx, order, mutations)
    }

    /// Applies post-trade updates from execution reports.
    ///
    /// The engine calls this hook from [`crate::Engine::apply_execution_report`]
    /// so that a main-stage policy can maintain post-trade state. `ctx` exposes
    /// the report account's [`AccountGroupId`](crate::param::AccountGroupId)
    /// through [`PostTradeContext::account_group`].
    ///
    /// Returns `Some(`[`PostTradeResult`]`)` when this policy produced account
    /// blocks, account-level PnL outcomes, or account adjustments, or `None`
    /// when the report caused no state change. The engine only merges results
    /// that are `Some`.
    fn apply_execution_report(
        &self,
        _ctx: &PostTradeContext<<Sync as SyncMode>::StorageLockingPolicyFactory>,
        _report: &ExecutionReport,
    ) -> Option<PostTradeResult> {
        None
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
        _ctx: &AccountAdjustmentContext<<Sync as SyncMode>::StorageLockingPolicyFactory>,
        _account_id: AccountId,
        _adjustment: &AccountAdjustment,
        _mutations: &mut Mutations,
    ) -> Result<PolicyAccountAdjustmentResult, Rejects> {
        Ok(PolicyAccountAdjustmentResult::default())
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

pub(crate) fn missing_required_field_account_adjustment_reject<Policy: PolicyName + ?Sized>(
    policy: &Policy,
    field_name: &str,
    error: &crate::RequestFieldAccessError,
) -> Reject {
    Reject::new(
        policy.policy_name(),
        RejectScope::Account,
        RejectCode::MissingRequiredField,
        format!("failed to access required field '{field_name}'"),
        error.to_string(),
    )
}

pub(crate) fn field_access_error_account_adjustment_reject<Policy: PolicyName + ?Sized>(
    policy: &Policy,
    field_name: &str,
    error: &crate::RequestFieldAccessError,
) -> Reject {
    Reject::new(
        policy.policy_name(),
        RejectScope::Account,
        RejectCode::InvalidFieldFormat,
        format!("failed to access field '{field_name}'"),
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
    use crate::pretrade::{PolicyPreTradeResult, RejectCode, RejectScope, Rejects};
    use crate::{Mutations, PolicyAccountAdjustmentResult, RequestFieldAccessError};

    use crate::core::{LocalSync, SyncMode};
    use crate::storage::NoLocking;

    use super::{
        missing_required_field_reject, PolicyName, PostTradeContext, PreTradeContext,
        PreTradePolicy,
    };

    type TestOrder = OrderOperation;
    type TestReport = WithExecutionReportOperation<WithFinancialImpact<()>>;

    struct MainPolicyNoop;

    struct StartPolicyNoop;

    struct AccountAdjustmentHookNoop;

    impl<Sync: SyncMode> PreTradePolicy<TestOrder, TestReport, (), Sync> for StartPolicyNoop {
        fn name(&self) -> &str {
            "StartPolicyNoop"
        }

        fn check_pre_trade_start(
            &self,
            _ctx: &PreTradeContext<<Sync as SyncMode>::StorageLockingPolicyFactory>,
            _order: &TestOrder,
        ) -> Result<(), Rejects> {
            Ok(())
        }
    }

    impl<Sync: SyncMode> PreTradePolicy<TestOrder, TestReport, (), Sync> for MainPolicyNoop {
        fn name(&self) -> &str {
            "MainPolicyNoop"
        }

        fn perform_pre_trade_check(
            &self,
            _ctx: &PreTradeContext<<Sync as SyncMode>::StorageLockingPolicyFactory>,
            _order: &TestOrder,
            _mutations: &mut Mutations,
        ) -> Result<Option<PolicyPreTradeResult>, Rejects> {
            Ok(None)
        }
    }

    impl<Sync: SyncMode> PreTradePolicy<TestOrder, TestReport, (), Sync> for AccountAdjustmentHookNoop {
        fn name(&self) -> &str {
            "AccountAdjustmentHookNoop"
        }

        fn apply_account_adjustment(
            &self,
            _ctx: &crate::AccountAdjustmentContext<<Sync as SyncMode>::StorageLockingPolicyFactory>,
            _account_id: AccountId,
            _adjustment: &(),
            _mutations: &mut Mutations,
        ) -> Result<PolicyAccountAdjustmentResult, Rejects> {
            Ok(PolicyAccountAdjustmentResult::default())
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

        assert!(
            <MainPolicyNoop as PreTradePolicy<TestOrder, TestReport, (), LocalSync>>::apply_execution_report(
                &MainPolicyNoop,
                &PostTradeContext::new(),
                &report,
            )
            .is_none()
        );
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

        assert!(
            <StartPolicyNoop as PreTradePolicy<TestOrder, TestReport, (), LocalSync>>::apply_execution_report(
                &StartPolicyNoop,
                &PostTradeContext::new(),
                &report,
            )
            .is_none()
        );
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

        assert_eq!(
            <MainPolicyNoop as PreTradePolicy<TestOrder, TestReport, (), LocalSync>>::name(
                &MainPolicyNoop
            ),
            "MainPolicyNoop"
        );
        let result = <MainPolicyNoop as PreTradePolicy<TestOrder, TestReport, (), LocalSync>>::perform_pre_trade_check(
            &MainPolicyNoop,
            &PreTradeContext::<NoLocking>::new(None),
            &order,
            &mut mutations,
        );
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

        assert_eq!(
            <StartPolicyNoop as PreTradePolicy<TestOrder, TestReport, (), LocalSync>>::name(
                &StartPolicyNoop
            ),
            "StartPolicyNoop"
        );
        assert!(
            <StartPolicyNoop as PreTradePolicy<TestOrder, TestReport, (), LocalSync>>::check_pre_trade_start(
                &StartPolicyNoop,
                &PreTradeContext::<NoLocking>::new(None),
                &order,
            )
            .is_ok()
        );
    }

    #[test]
    fn required_account_adjustment_trait_methods_can_be_invoked_without_side_effects() {
        let mut mutations = Mutations::new();

        assert_eq!(
            <AccountAdjustmentHookNoop as PreTradePolicy<TestOrder, TestReport, (), LocalSync>>::name(
                &AccountAdjustmentHookNoop
            ),
            "AccountAdjustmentHookNoop"
        );
        use crate::core::account_control::BlockedAccounts;
        use crate::core::{AccountBlockHandle, AccountControl};
        use crate::storage::{LockingPolicyFactory, NoLocking, StorageBuilder};
        let builder = StorageBuilder::new(NoLocking);
        let handle =
            AccountBlockHandle::from_inner(NoLocking::new_shared(BlockedAccounts::new(&builder)));
        let ctrl = AccountControl::new(handle, AccountId::from_u64(99224416));
        let result = <AccountAdjustmentHookNoop as PreTradePolicy<
            TestOrder,
            TestReport,
            (),
            LocalSync,
        >>::apply_account_adjustment(
            &AccountAdjustmentHookNoop,
            &crate::AccountAdjustmentContext::new_test(ctrl),
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
