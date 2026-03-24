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
use std::collections::HashSet;
use std::fmt::{Display, Formatter};
use std::marker::PhantomData;
use std::rc::{Rc, Weak};
use std::time::Instant;

use crate::param::AccountId;
use crate::pretrade::handle::{RequestHandleImpl, ReservationHandleImpl};
use crate::pretrade::start_pre_trade_time::with_start_pre_trade_now;
use crate::pretrade::{
    AccountAdjustmentPolicy, CheckPreTradeStartPolicy, Context, Mutations, Policy, PostTradeResult,
    Reject, RejectCode, RejectScope, Rejects, Request, Reservation,
};

struct EngineInner<O, R, A> {
    check_pre_trade_start_policies: Vec<Box<dyn CheckPreTradeStartPolicy<O, R>>>,
    pre_trade_policies: Vec<Box<dyn Policy<O, R>>>,
    account_adjustment_policies: Vec<Box<dyn AccountAdjustmentPolicy<A>>>,
}

/// Errors returned by [`EngineBuilder::build`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum EngineBuildError {
    /// Duplicate policy name across registered policy sets.
    DuplicatePolicyName { name: &'static str },
}

impl Display for EngineBuildError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicatePolicyName { name } => {
                write!(formatter, "duplicate policy name: {name}")
            }
        }
    }
}

impl std::error::Error for EngineBuildError {}

/// Error returned when account-adjustment batch validation fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountAdjustmentBatchError {
    /// Zero-based index of the failing adjustment.
    pub index: usize,
    /// The reject produced by the policy.
    pub reject: Reject,
}

impl Display for AccountAdjustmentBatchError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "account adjustment batch rejected at index {}: {}",
            self.index, self.reject
        )
    }
}

impl std::error::Error for AccountAdjustmentBatchError {}

/// Risk engine orchestrating start-stage and main-stage pre-trade checks.
///
/// Build the engine once during platform initialization using
/// [`Engine::builder`], then share it across order submissions.
///
/// Generic parameters:
/// - `O`: order contract type used by `start_pre_trade`;
/// - `R`: execution-report contract type used by `apply_execution_report`;
/// - `A`: account-adjustment contract type used by `apply_account_adjustment`.
///
/// # Thread safety
///
/// `Engine` is `!Send + !Sync`. All calls must happen on the same thread
/// that created it. Synchronization across threads is the caller's
/// responsibility.
///
/// # Examples
///
/// ```rust
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use openpit::param::{Asset, Price, Quantity, Side, TradeAmount};
/// use openpit::{Engine, Instrument, OrderOperation, WithOrderOperation};
/// use openpit::{FinancialImpact, ExecutionReportOperation, WithFinancialImpact, WithExecutionReportOperation};
///
/// type MyOrder = WithOrderOperation<()>;
/// type MyReport = WithExecutionReportOperation<WithFinancialImpact<()>>;
///
/// let engine = Engine::<MyOrder, MyReport>::builder().build()?;
///
/// let order = WithOrderOperation {
///     inner: (),
///     operation: OrderOperation {
///         instrument: Instrument::new(Asset::new("AAPL")?, Asset::new("USD")?),
///         account_id: openpit::param::AccountId::from_u64(12345),
///         side: Side::Buy,
///         trade_amount: TradeAmount::Quantity(Quantity::from_str("100")?),
///         price: Some(Price::from_str("185")?),
///     },
/// };
///
/// let request = engine.start_pre_trade(order)?;
/// let reservation = request.execute()?;
/// reservation.commit();
/// # Ok(())
/// # }
/// ```
pub struct Engine<O, R = (), A = ()> {
    inner: Rc<RefCell<EngineInner<O, R, A>>>,
}

impl<O: 'static, R: 'static, A: 'static> Engine<O, R, A> {
    /// Creates an engine builder.
    pub fn builder() -> EngineBuilder<O, R, A> {
        EngineBuilder::new()
    }

    /// Executes start-stage checks and creates a deferred [`Request`].
    ///
    /// Start-stage policies run in registration order and stop at the first reject.
    ///
    /// The engine does not enforce optional order extensions (for example
    /// `instrument` or `side`). Policies that depend on extension fields must
    /// validate their presence.
    ///
    /// # Errors
    ///
    /// Returns [`Reject`] when any start-stage policy rejects the order.
    pub fn start_pre_trade(&self, order: O) -> Result<Request<O>, Reject> {
        let now: Instant = Instant::now();
        with_start_pre_trade_now(now, || {
            let inner = self.inner.borrow();
            for policy in &inner.check_pre_trade_start_policies {
                policy.check_pre_trade_start(&order)?;
            }
            Ok::<(), Reject>(())
        })?;

        let engine = Rc::downgrade(&self.inner);
        let request_handle =
            RequestHandleImpl::<O>::new(Box::new(move || execute_request(engine, order)));

        Ok(Request::from_handle(Box::new(request_handle)))
    }

    /// Applies post-trade updates and aggregates kill-switch status across all policies.
    ///
    /// Returns [`PostTradeResult::kill_switch_triggered`] `true` when at least one policy
    /// reports a kill-switch condition.
    pub fn apply_execution_report(&self, report: &R) -> PostTradeResult {
        let inner = self.inner.borrow();
        let mut kill_switch_triggered = false;

        for policy in &inner.check_pre_trade_start_policies {
            kill_switch_triggered |= policy.apply_execution_report(report);
        }
        for policy in &inner.pre_trade_policies {
            kill_switch_triggered |= policy.apply_execution_report(report);
        }

        PostTradeResult {
            kill_switch_triggered,
        }
    }

    /// Validates an account-adjustment batch atomically.
    ///
    /// Policies are evaluated in registration order for each adjustment, and
    /// adjustments are traversed in slice order.
    ///
    /// # Errors
    ///
    /// Returns [`AccountAdjustmentBatchError`] for the first rejected element.
    /// The `index` field points to the failing adjustment in `adjustments`.
    pub fn apply_account_adjustment(
        &self,
        account_id: AccountId,
        adjustments: &[A],
    ) -> Result<(), AccountAdjustmentBatchError> {
        if adjustments.is_empty() {
            return Ok(());
        }

        let inner = self.inner.borrow();
        let mut mutations = Mutations::new();
        let mut batch_error: Option<AccountAdjustmentBatchError> = None;

        'outer: for (index, adjustment) in adjustments.iter().enumerate() {
            for policy in &inner.account_adjustment_policies {
                if let Err(reject) =
                    policy.apply_account_adjustment(account_id, adjustment, &mut mutations)
                {
                    batch_error = Some(AccountAdjustmentBatchError { index, reject });
                    break 'outer;
                }
            }
        }

        if let Some(err) = batch_error {
            mutations.rollback_all();
            return Err(err);
        }

        mutations.commit_all();
        Ok(())
    }
}

/// Fluent builder for [`Engine`].
///
/// Policies are evaluated in registration order. Policy names must be unique
/// across start-stage, main-stage, and account-adjustment sets;
/// [`EngineBuilder::build`] returns [`EngineBuildError::DuplicatePolicyName`]
/// otherwise.
///
/// # Examples
///
/// ```rust
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use std::time::Duration;
/// use openpit::{WithExecutionReportOperation, WithFinancialImpact, WithOrderOperation};
/// use openpit::pretrade::policies::{PnlKillSwitchPolicy, RateLimitPolicy};
/// use openpit::Engine;
/// use openpit::param::{Asset, Pnl};
///
/// type MyOrder = WithOrderOperation<()>;
/// type MyReport = WithFinancialImpact<WithExecutionReportOperation<()>>;
///
/// let pnl_policy = PnlKillSwitchPolicy::new(
///     (
///         Asset::new("USD")?,
///         Pnl::from_str("500")?,
///     ),
///     [],
/// )?;
///
/// let rate_policy = RateLimitPolicy::new(100, Duration::from_secs(1));
///
/// let engine = Engine::<MyOrder, MyReport>::builder()
///     .check_pre_trade_start_policy(pnl_policy)
///     .check_pre_trade_start_policy(rate_policy)
///     .build()?;
/// let _ = engine;
/// # Ok(())
/// # }
/// ```
pub struct EngineBuilder<O, R = (), A = ()> {
    check_pre_trade_start_policies: Vec<Box<dyn CheckPreTradeStartPolicy<O, R>>>,
    pre_trade_policies: Vec<Box<dyn Policy<O, R>>>,
    account_adjustment_policies: Vec<Box<dyn AccountAdjustmentPolicy<A>>>,
    marker: PhantomData<fn(O, R, A)>,
}

impl<O, R, A> EngineBuilder<O, R, A> {
    /// Creates a new builder.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            check_pre_trade_start_policies: Vec::new(),
            pre_trade_policies: Vec::new(),
            account_adjustment_policies: Vec::new(),
            marker: PhantomData,
        }
    }

    /// Registers a start-stage policy.
    pub fn check_pre_trade_start_policy<P>(mut self, policy: P) -> Self
    where
        P: CheckPreTradeStartPolicy<O, R> + 'static,
    {
        self.check_pre_trade_start_policies.push(Box::new(policy));
        self
    }

    /// Registers a main-stage policy.
    pub fn pre_trade_policy<P>(mut self, policy: P) -> Self
    where
        P: Policy<O, R> + 'static,
    {
        self.pre_trade_policies.push(Box::new(policy));
        self
    }

    /// Registers an account-adjustment policy.
    ///
    /// Policy names must remain unique across start-stage, main-stage, and
    /// account-adjustment policy sets.
    pub fn account_adjustment_policy<P>(mut self, policy: P) -> Self
    where
        P: AccountAdjustmentPolicy<A> + 'static,
    {
        self.account_adjustment_policies.push(Box::new(policy));
        self
    }

    /// Builds the engine.
    pub fn build(self) -> Result<Engine<O, R, A>, EngineBuildError>
    where
        O: 'static,
        R: 'static,
        A: 'static,
    {
        ensure_unique_policy_names(
            &self.check_pre_trade_start_policies,
            &self.pre_trade_policies,
            &self.account_adjustment_policies,
        )?;

        Ok(Engine {
            inner: Rc::new(RefCell::new(EngineInner {
                check_pre_trade_start_policies: self.check_pre_trade_start_policies,
                pre_trade_policies: self.pre_trade_policies,
                account_adjustment_policies: self.account_adjustment_policies,
            })),
        })
    }
}

fn ensure_unique_policy_names<O, R, A>(
    check_pre_trade_start_policies: &[Box<dyn CheckPreTradeStartPolicy<O, R>>],
    pre_trade_policies: &[Box<dyn Policy<O, R>>],
    account_adjustment_policies: &[Box<dyn AccountAdjustmentPolicy<A>>],
) -> Result<(), EngineBuildError> {
    let mut unique = HashSet::new();

    for policy in check_pre_trade_start_policies {
        let inserted = unique.insert(policy.name());
        if !inserted {
            return Err(EngineBuildError::DuplicatePolicyName {
                name: policy.name(),
            });
        }
    }

    for policy in pre_trade_policies {
        let inserted = unique.insert(policy.name());
        if !inserted {
            return Err(EngineBuildError::DuplicatePolicyName {
                name: policy.name(),
            });
        }
    }

    for policy in account_adjustment_policies {
        let inserted = unique.insert(policy.name());
        if !inserted {
            return Err(EngineBuildError::DuplicatePolicyName {
                name: policy.name(),
            });
        }
    }

    Ok(())
}

fn execute_request<O: 'static, R: 'static, A: 'static>(
    engine: Weak<RefCell<EngineInner<O, R, A>>>,
    order: O,
) -> Result<Reservation, Rejects> {
    let Some(engine_ref) = engine.upgrade() else {
        return Err(Rejects::new(vec![Reject::new(
            "Engine",
            RejectScope::Order,
            RejectCode::SystemUnavailable,
            "engine is no longer available",
            "request handle outlived engine instance".to_owned(),
        )]));
    };
    let inner = engine_ref.borrow();

    let mut mutations = Mutations::new();
    let mut rejects = Vec::new();
    let ctx = Context::new(&order);

    for policy in &inner.pre_trade_policies {
        policy.perform_pre_trade_check(&ctx, &mut mutations, &mut rejects);
    }

    drop(inner);

    if !rejects.is_empty() {
        mutations.rollback_all();
        return Err(Rejects::new(rejects));
    }

    let reservation_handle = ReservationHandleImpl::new(mutations);
    Ok(Reservation::from_handle(Box::new(reservation_handle)))
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    use crate::core::{
        ExecutionReportOperation, FinancialImpact, Instrument, OrderOperation,
        WithExecutionReportOperation, WithFinancialImpact, WithOrderOperation,
    };
    use crate::param::{AccountId, Asset, Fee, Pnl, Price, Quantity, Side, TradeAmount, Volume};
    use crate::pretrade::{
        AccountAdjustmentPolicy, CheckPreTradeStartPolicy, Context, Mutation, Mutations, Policy,
        Reject, RejectCode, RejectScope,
    };

    use super::{AccountAdjustmentBatchError, Engine, EngineBuildError};

    type TestOrder = WithOrderOperation<()>;
    type TestReport = WithFinancialImpact<WithExecutionReportOperation<()>>;
    type TestAdjustment = MockAdjustment;
    type MutationHook = Rc<dyn Fn(&mut Mutations)>;
    type AdjustmentHook = Box<dyn Fn(&mut Mutations)>;

    #[test]
    fn build_rejects_duplicate_policy_names_across_stages() {
        let result = Engine::<TestOrder, TestReport>::builder()
            .check_pre_trade_start_policy(StartPolicyMock::pass("dup"))
            .pre_trade_policy(MainPolicyMock::pass("dup"))
            .build();

        assert!(matches!(
            result,
            Err(EngineBuildError::DuplicatePolicyName { name: "dup" })
        ));
    }

    #[test]
    fn build_rejects_duplicate_policy_names_within_start_stage() {
        let result = Engine::<TestOrder, TestReport>::builder()
            .check_pre_trade_start_policy(StartPolicyMock::pass("dup"))
            .check_pre_trade_start_policy(StartPolicyMock::pass("dup"))
            .build();

        assert!(matches!(
            result,
            Err(EngineBuildError::DuplicatePolicyName { name: "dup" })
        ));
    }

    #[test]
    fn apply_account_adjustment_passes_adjustment_to_policy_for_single_element() {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let engine = Engine::<TestOrder, TestReport, TestAdjustment>::builder()
            .account_adjustment_policy(AdjustmentPolicyMock::pass("adj", Rc::clone(&seen)))
            .build()
            .expect("engine must build");

        let batch = [MockAdjustment { id: 7, amount: 120 }];
        let result = engine.apply_account_adjustment(AccountId::from_u64(99224416), &batch);

        assert!(result.is_ok());
        assert_eq!(*seen.borrow(), vec![MockAdjustment { id: 7, amount: 120 }]);
    }

    #[test]
    fn apply_account_adjustment_returns_failing_index() {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let engine = Engine::<TestOrder, TestReport, TestAdjustment>::builder()
            .account_adjustment_policy(AdjustmentPolicyMock::reject_on_id(
                "adj_reject",
                Rc::clone(&seen),
                1,
            ))
            .build()
            .expect("engine must build");

        let batch = [
            MockAdjustment { id: 0, amount: 10 },
            MockAdjustment { id: 1, amount: 20 },
            MockAdjustment { id: 2, amount: 30 },
        ];
        let result = engine.apply_account_adjustment(AccountId::from_u64(99224416), &batch);

        assert!(matches!(
            result,
            Err(AccountAdjustmentBatchError {
                index: 1,
                reject: Reject {
                    policy: "adj_reject",
                    ..
                }
            })
        ));
        assert_eq!(
            *seen.borrow(),
            vec![
                MockAdjustment { id: 0, amount: 10 },
                MockAdjustment { id: 1, amount: 20 },
            ]
        );
    }

    #[test]
    fn apply_account_adjustment_does_not_apply_partial_batch_on_reject() {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let mut applied = Vec::new();
        let engine = Engine::<TestOrder, TestReport, TestAdjustment>::builder()
            .account_adjustment_policy(AdjustmentPolicyMock::reject_on_id(
                "adj_reject",
                Rc::clone(&seen),
                2,
            ))
            .build()
            .expect("engine must build");

        let batch = [
            MockAdjustment { id: 1, amount: 10 },
            MockAdjustment { id: 2, amount: 20 },
        ];
        let result = engine.apply_account_adjustment(AccountId::from_u64(99224416), &batch);
        if result.is_ok() {
            applied.extend(batch.iter().map(|adjustment| adjustment.id));
        }

        assert!(result.is_err());
        assert!(applied.is_empty());
        assert_eq!(
            *seen.borrow(),
            vec![
                MockAdjustment { id: 1, amount: 10 },
                MockAdjustment { id: 2, amount: 20 },
            ]
        );
    }

    #[test]
    fn apply_account_adjustment_accepts_multi_element_batch_when_all_policies_pass() {
        let first_seen = Rc::new(RefCell::new(Vec::new()));
        let second_seen = Rc::new(RefCell::new(Vec::new()));
        let engine = Engine::<TestOrder, TestReport, TestAdjustment>::builder()
            .account_adjustment_policy(AdjustmentPolicyMock::pass("first", Rc::clone(&first_seen)))
            .account_adjustment_policy(AdjustmentPolicyMock::pass(
                "second",
                Rc::clone(&second_seen),
            ))
            .build()
            .expect("engine must build");

        let batch = [
            MockAdjustment { id: 10, amount: 1 },
            MockAdjustment { id: 11, amount: 2 },
            MockAdjustment { id: 12, amount: 3 },
        ];

        assert!(engine
            .apply_account_adjustment(AccountId::from_u64(99224416), &batch)
            .is_ok());
        assert_eq!(*first_seen.borrow(), batch);
        assert_eq!(*second_seen.borrow(), batch);
    }

    #[test]
    fn apply_account_adjustment_empty_batch_skips_policy_calls() {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let engine = Engine::<TestOrder, TestReport, TestAdjustment>::builder()
            .account_adjustment_policy(AdjustmentPolicyMock::pass("adj", Rc::clone(&seen)))
            .build()
            .expect("engine must build");

        assert!(engine
            .apply_account_adjustment(AccountId::from_u64(99224416), &[])
            .is_ok());
        assert!(seen.borrow().is_empty());
    }

    #[test]
    fn apply_account_adjustment_returns_reject_from_second_policy() {
        let first_seen = Rc::new(RefCell::new(Vec::new()));
        let second_seen = Rc::new(RefCell::new(Vec::new()));
        let engine = Engine::<TestOrder, TestReport, TestAdjustment>::builder()
            .account_adjustment_policy(AdjustmentPolicyMock::pass("first", Rc::clone(&first_seen)))
            .account_adjustment_policy(AdjustmentPolicyMock::reject_on_id(
                "second",
                Rc::clone(&second_seen),
                9,
            ))
            .build()
            .expect("engine must build");

        let batch = [MockAdjustment { id: 9, amount: 99 }];
        let result = engine.apply_account_adjustment(AccountId::from_u64(99224416), &batch);

        assert!(matches!(
            result,
            Err(AccountAdjustmentBatchError {
                index: 0,
                reject: Reject {
                    policy: "second",
                    ..
                }
            })
        ));
        assert_eq!(*first_seen.borrow(), batch);
        assert_eq!(*second_seen.borrow(), batch);
    }

    #[test]
    fn apply_account_adjustment_respects_policy_registration_order_for_rejects() {
        let first_seen = Rc::new(RefCell::new(Vec::new()));
        let second_seen = Rc::new(RefCell::new(Vec::new()));
        let engine = Engine::<TestOrder, TestReport, TestAdjustment>::builder()
            .account_adjustment_policy(AdjustmentPolicyMock::reject_on_id(
                "first",
                Rc::clone(&first_seen),
                77,
            ))
            .account_adjustment_policy(AdjustmentPolicyMock::reject_on_id(
                "second",
                Rc::clone(&second_seen),
                77,
            ))
            .build()
            .expect("engine must build");

        let batch = [MockAdjustment { id: 77, amount: 1 }];
        let result = engine.apply_account_adjustment(AccountId::from_u64(99224416), &batch);

        assert!(matches!(
            result,
            Err(AccountAdjustmentBatchError {
                index: 0,
                reject: Reject {
                    policy: "first",
                    ..
                }
            })
        ));
        assert_eq!(*first_seen.borrow(), batch);
        assert!(second_seen.borrow().is_empty());
    }

    #[test]
    fn build_rejects_duplicate_policy_names_between_start_and_account_adjustment() {
        let result = Engine::<TestOrder, TestReport, TestAdjustment>::builder()
            .check_pre_trade_start_policy(StartPolicyMock::pass("dup"))
            .account_adjustment_policy(AdjustmentPolicyMock::pass(
                "dup",
                Rc::new(RefCell::new(Vec::new())),
            ))
            .build();

        assert!(matches!(
            result,
            Err(EngineBuildError::DuplicatePolicyName { name: "dup" })
        ));
    }

    #[test]
    fn apply_account_adjustment_without_policies_accepts_any_batch() {
        let engine = Engine::<TestOrder, TestReport, TestAdjustment>::builder()
            .build()
            .expect("engine must build");
        let batch = [
            MockAdjustment { id: 1, amount: 11 },
            MockAdjustment { id: 2, amount: 22 },
        ];

        assert!(engine
            .apply_account_adjustment(AccountId::from_u64(99224416), &batch)
            .is_ok());
    }

    #[test]
    fn apply_account_adjustment_commits_mutations_on_success() {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let flag = Rc::new(RefCell::new(false));
        let commit_flag = Rc::clone(&flag);
        let rollback_flag = Rc::clone(&flag);
        let engine = Engine::<TestOrder, TestReport, TestAdjustment>::builder()
            .account_adjustment_policy(AdjustmentPolicyMock::with_side_effect(
                "adj_mutation",
                Rc::clone(&seen),
                move |mutations| {
                    let c = Rc::clone(&commit_flag);
                    let r = Rc::clone(&rollback_flag);
                    mutations.push(Mutation::new(
                        move || {
                            *c.borrow_mut() = true;
                        },
                        move || {
                            *r.borrow_mut() = false;
                        },
                    ));
                },
            ))
            .build()
            .expect("engine must build");

        let batch = [MockAdjustment { id: 1, amount: 10 }];
        assert!(engine
            .apply_account_adjustment(AccountId::from_u64(99224416), &batch)
            .is_ok());
        assert!(*flag.borrow());
    }

    #[test]
    fn apply_account_adjustment_rolls_back_mutations_on_reject() {
        let first_seen = Rc::new(RefCell::new(Vec::new()));
        let second_seen = Rc::new(RefCell::new(Vec::new()));
        let flag = Rc::new(RefCell::new(false));
        let commit_flag = Rc::clone(&flag);
        let rollback_flag = Rc::clone(&flag);
        let engine = Engine::<TestOrder, TestReport, TestAdjustment>::builder()
            .account_adjustment_policy(AdjustmentPolicyMock::with_side_effect(
                "adj_mutation",
                Rc::clone(&first_seen),
                move |mutations| {
                    let c = Rc::clone(&commit_flag);
                    let r = Rc::clone(&rollback_flag);
                    mutations.push(Mutation::new(
                        move || {
                            *c.borrow_mut() = true;
                        },
                        move || {
                            *r.borrow_mut() = false;
                        },
                    ));
                },
            ))
            .account_adjustment_policy(AdjustmentPolicyMock::reject_on_id(
                "adj_rejecter",
                Rc::clone(&second_seen),
                1,
            ))
            .build()
            .expect("engine must build");

        let batch = [MockAdjustment { id: 1, amount: 10 }];
        assert!(engine
            .apply_account_adjustment(AccountId::from_u64(99224416), &batch)
            .is_err());
        assert!(!(*flag.borrow()));
    }

    #[test]
    fn apply_account_adjustment_without_mutations_leaves_state_unchanged() {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let engine = Engine::<TestOrder, TestReport, TestAdjustment>::builder()
            .account_adjustment_policy(AdjustmentPolicyMock::pass("adj", Rc::clone(&seen)))
            .build()
            .expect("engine must build");

        let batch = [MockAdjustment { id: 1, amount: 10 }];
        assert!(engine
            .apply_account_adjustment(AccountId::from_u64(99224416), &batch)
            .is_ok());
    }

    #[test]
    fn engine_builder_with_default_report_type_remains_operational() {
        let engine = Engine::<TestOrder>::builder()
            .build()
            .expect("engine must build with default report type");
        let request = engine
            .start_pre_trade(order_with_settlement("USD"))
            .expect("start stage must pass");
        let reservation = request.execute().expect("execute must pass");
        reservation.rollback();

        let post_trade = engine.apply_execution_report(&());
        assert!(!post_trade.kill_switch_triggered);
    }

    #[test]
    fn builder_builds_operational_empty_engine() {
        let reservation = Engine::<TestOrder, TestReport>::builder()
            .build()
            .expect("builder must build")
            .start_pre_trade(order_with_settlement("USD"))
            .expect("built engine must allow start stage")
            .execute()
            .expect("built engine must allow execute");
        reservation.rollback();
    }

    #[test]
    fn accepts_order_without_operation_fields_when_no_policy_requires_them() {
        let engine = Engine::<(), TestReport>::builder()
            .check_pre_trade_start_policy(CoreStartPolicyMock {
                name: "core_start",
                reject: false,
            })
            .pre_trade_policy(CoreMainPolicyMock {
                name: "core_main",
                on_apply: None,
                reject: false,
            })
            .build()
            .expect("engine must build");
        let order = ();
        let reservation = engine
            .start_pre_trade(order)
            .expect("start stage must pass")
            .execute()
            .expect("main stage must pass");
        reservation.commit();

        let post_trade = engine.apply_execution_report(&execution_report("USD"));
        assert!(!post_trade.kill_switch_triggered);
    }

    #[test]
    fn order_trade_input_build_rejects_duplicate_policy_names_across_stages() {
        let result = Engine::<(), TestReport>::builder()
            .check_pre_trade_start_policy(CoreStartPolicyMock {
                name: "dup",
                reject: false,
            })
            .pre_trade_policy(CoreMainPolicyMock {
                name: "dup",
                on_apply: None,
                reject: false,
            })
            .build();

        assert!(matches!(
            result,
            Err(EngineBuildError::DuplicatePolicyName { name: "dup" })
        ));
    }

    #[test]
    fn order_trade_input_build_rejects_duplicate_policy_names_within_start_stage() {
        let result = Engine::<(), TestReport>::builder()
            .check_pre_trade_start_policy(CoreStartPolicyMock {
                name: "dup",
                reject: false,
            })
            .check_pre_trade_start_policy(CoreStartPolicyMock {
                name: "dup",
                reject: false,
            })
            .build();

        assert!(matches!(
            result,
            Err(EngineBuildError::DuplicatePolicyName { name: "dup" })
        ));
    }

    #[test]
    fn order_trade_input_start_pre_trade_rejects_before_request_is_created() {
        let engine = Engine::<(), TestReport>::builder()
            .check_pre_trade_start_policy(CoreStartPolicyMock {
                name: "core_start_reject",
                reject: true,
            })
            .build()
            .expect("engine must build");
        let order = ();

        let result = engine.start_pre_trade(order);
        assert!(matches!(
            result,
            Err(Reject {
                policy: "core_start_reject",
                code: RejectCode::Other,
                ..
            })
        ));

        let post_trade = engine.apply_execution_report(&execution_report("USD"));
        assert!(!post_trade.kill_switch_triggered);
    }

    #[test]
    fn order_core_execute_rejects_and_rolls_back_mutations() {
        let state = Rc::new(RefCell::new(None));
        let engine = Engine::<(), TestReport>::builder()
            .check_pre_trade_start_policy(CoreStartPolicyMock {
                name: "core_start",
                reject: false,
            })
            .pre_trade_policy(CoreMainPolicyMock {
                name: "core_main",
                on_apply: Some(Rc::new(shared_kill_switch_mutation(
                    Rc::clone(&state),
                    "core_order_mutation",
                    true,
                    false,
                ))),
                reject: true,
            })
            .build()
            .expect("engine must build");
        let order = ();

        let request = engine
            .start_pre_trade(order)
            .expect("start stage must create request");
        let result = request.execute();
        assert!(result.is_err(), "main stage must reject");
        let rejects = result.err().expect("rejects must be present");
        assert_eq!(rejects.len(), 1);
        assert_eq!(rejects[0].policy, "core_main");
        assert_eq!(rejects[0].code, RejectCode::Other);
        assert_eq!(*state.borrow(), Some(false));

        let post_trade = engine.apply_execution_report(&execution_report("USD"));
        assert!(!post_trade.kill_switch_triggered);
    }

    #[test]
    fn order_core_execute_commit_and_rollback_apply_mutation_callback() {
        let cases = [
            (FinalizeAction::Commit, true),
            (FinalizeAction::Rollback, false),
        ];

        for (action, expected_state) in cases {
            let state = Rc::new(RefCell::new(None));
            let engine = Engine::<(), TestReport>::builder()
                .check_pre_trade_start_policy(CoreStartPolicyMock {
                    name: "core_start",
                    reject: false,
                })
                .pre_trade_policy(CoreMainPolicyMock {
                    name: "core_main",
                    on_apply: Some(Rc::new(shared_kill_switch_mutation(
                        Rc::clone(&state),
                        "core_finalize_mutation",
                        true,
                        false,
                    ))),
                    reject: false,
                })
                .build()
                .expect("engine must build");
            let order = ();

            let reservation = engine
                .start_pre_trade(order)
                .expect("start stage must create request")
                .execute()
                .expect("main stage must pass");

            match action {
                FinalizeAction::Commit => reservation.commit(),
                FinalizeAction::Rollback => reservation.rollback(),
            }

            assert_eq!(*state.borrow(), Some(expected_state));

            let post_trade = engine.apply_execution_report(&execution_report("USD"));
            assert!(!post_trade.kill_switch_triggered);
        }
    }

    #[test]
    fn start_pre_trade_table_cases_follow_registration_order_and_stop_on_first_reject() {
        struct Case {
            reject_index: Option<usize>,
            expected_calls: [usize; 3],
            expected_main_calls: usize,
            expected_ok: bool,
        }

        let cases = [
            Case {
                reject_index: None,
                expected_calls: [1, 1, 1],
                expected_main_calls: 0,
                expected_ok: true,
            },
            Case {
                reject_index: Some(1),
                expected_calls: [1, 1, 0],
                expected_main_calls: 0,
                expected_ok: false,
            },
        ];

        for case in cases {
            let calls_0 = Rc::new(Cell::new(0));
            let calls_1 = Rc::new(Cell::new(0));
            let calls_2 = Rc::new(Cell::new(0));
            let main_calls = Rc::new(Cell::new(0));

            let start_0 = StartPolicyMock::new("s0", Rc::clone(&calls_0), false, false, None, None);
            let start_1 = StartPolicyMock::new(
                "s1",
                Rc::clone(&calls_1),
                case.reject_index == Some(1),
                false,
                None,
                None,
            );
            let start_2 = StartPolicyMock::new("s2", Rc::clone(&calls_2), false, false, None, None);

            let engine = Engine::<TestOrder, TestReport>::builder()
                .check_pre_trade_start_policy(start_0)
                .check_pre_trade_start_policy(start_1)
                .check_pre_trade_start_policy(start_2)
                .pre_trade_policy(MainPolicyMock::with_calls(
                    "m0",
                    Rc::clone(&main_calls),
                    false,
                    false,
                    None,
                ))
                .build()
                .expect("engine must build");

            let result = engine.start_pre_trade(order_with_settlement("USD"));
            assert_eq!(result.is_ok(), case.expected_ok);
            assert_eq!(calls_0.get(), case.expected_calls[0]);
            assert_eq!(calls_1.get(), case.expected_calls[1]);
            assert_eq!(calls_2.get(), case.expected_calls[2]);
            assert_eq!(main_calls.get(), case.expected_main_calls);
        }
    }

    #[test]
    fn execute_table_cases_cover_success_commit_and_reject_rollback() {
        struct Case {
            fail_first: bool,
            fail_second: bool,
            expected_rejects: usize,
            expected_kill_switch: bool,
        }

        let cases = [
            Case {
                fail_first: false,
                fail_second: false,
                expected_rejects: 0,
                expected_kill_switch: true,
            },
            Case {
                fail_first: true,
                fail_second: true,
                expected_rejects: 2,
                expected_kill_switch: false,
            },
        ];

        for case in cases {
            let state = Rc::new(RefCell::new(None));
            let engine = Engine::<TestOrder, TestReport>::builder()
                .check_pre_trade_start_policy(StartPolicyMock::pass("start"))
                .pre_trade_policy(MainPolicyMock::with_custom_mutation_and_optional_reject(
                    "m1_policy",
                    shared_kill_switch_mutation(
                        Rc::clone(&state),
                        "shared_kill_switch",
                        false,
                        false,
                    ),
                    case.fail_first,
                    RejectScope::Order,
                ))
                .pre_trade_policy(MainPolicyMock::with_custom_mutation_and_optional_reject(
                    "m2_policy",
                    shared_kill_switch_mutation(
                        Rc::clone(&state),
                        "shared_kill_switch",
                        true,
                        true,
                    ),
                    case.fail_second,
                    RejectScope::Account,
                ))
                .build()
                .expect("engine must build");

            let request = engine
                .start_pre_trade(order_with_settlement("USD"))
                .expect("start stage must pass");
            let execute_result = request.execute();

            if case.expected_rejects == 0 {
                let reservation = execute_result.expect("execute must pass");
                reservation.commit();
            } else {
                assert!(execute_result.is_err(), "execute must reject");
                let rejects = execute_result.err().expect("rejects must be present");
                assert_eq!(rejects.len(), case.expected_rejects);
                assert_eq!(rejects[0].code, RejectCode::Other);
                assert_eq!(rejects[0].scope, RejectScope::Order);
                assert_eq!(rejects[1].code, RejectCode::Other);
                assert_eq!(rejects[1].scope, RejectScope::Account);
            }

            assert_eq!(*state.borrow(), Some(case.expected_kill_switch));
        }
    }

    #[test]
    fn light_stage_changes_are_not_rolled_back_when_execute_rejects() {
        let light_counter = Rc::new(Cell::new(0));
        let engine = Engine::<TestOrder, TestReport>::builder()
            .check_pre_trade_start_policy(StartPolicyMock::with_counter(
                "start",
                Rc::clone(&light_counter),
            ))
            .pre_trade_policy(MainPolicyMock::with_mutation_and_optional_reject(
                "rejecting_main",
                "m1",
                true,
                RejectScope::Order,
            ))
            .build()
            .expect("engine must build");

        let request = engine
            .start_pre_trade(order_with_settlement("USD"))
            .expect("start stage must pass");
        assert!(request.execute().is_err(), "execute must reject");

        assert_eq!(light_counter.get(), 1);
    }

    #[test]
    fn reservation_drop_triggers_rollback_in_reverse_order() {
        let state = Rc::new(RefCell::new(None));
        let engine = Engine::<TestOrder, TestReport>::builder()
            .check_pre_trade_start_policy(StartPolicyMock::pass("start"))
            .pre_trade_policy(MainPolicyMock::with_custom_mutation_and_optional_reject(
                "m1_policy",
                shared_kill_switch_mutation(Rc::clone(&state), "shared_kill_switch", false, false),
                false,
                RejectScope::Order,
            ))
            .pre_trade_policy(MainPolicyMock::with_custom_mutation_and_optional_reject(
                "m2_policy",
                shared_kill_switch_mutation(Rc::clone(&state), "shared_kill_switch", true, true),
                false,
                RejectScope::Order,
            ))
            .build()
            .expect("engine must build");

        let request = engine
            .start_pre_trade(order_with_settlement("USD"))
            .expect("start stage must pass");
        let reservation = request.execute().expect("execute must pass");
        drop(reservation);

        assert_eq!(*state.borrow(), Some(false));
    }

    #[test]
    fn apply_execution_report_aggregates_kill_switch_triggered() {
        let engine = Engine::<TestOrder, TestReport>::builder()
            .check_pre_trade_start_policy(StartPolicyMock::new(
                "start_false",
                Rc::new(Cell::new(0)),
                false,
                false,
                None,
                None,
            ))
            .pre_trade_policy(MainPolicyMock::with_calls(
                "main_true",
                Rc::new(Cell::new(0)),
                false,
                true,
                None,
            ))
            .build()
            .expect("engine must build");

        let result = engine.apply_execution_report(&execution_report("USD"));
        assert!(result.kill_switch_triggered);
    }

    #[test]
    fn request_returns_system_unavailable_when_engine_is_dropped() {
        let request = {
            let engine = Engine::<TestOrder, TestReport>::builder()
                .build()
                .expect("engine must build");
            engine
                .start_pre_trade(order_with_settlement("USD"))
                .expect("start stage must pass")
        };

        let result = request.execute();
        assert!(
            result.is_err(),
            "request must fail when engine is unavailable"
        );
        let rejects = result
            .err()
            .expect("rejects must be present when engine is unavailable");
        assert_eq!(rejects.len(), 1);

        let reject = &rejects[0];
        assert_eq!(reject.policy, "Engine");
        assert_eq!(reject.scope, RejectScope::Order);
        assert_eq!(reject.code, RejectCode::SystemUnavailable);
        assert_eq!(reject.reason, "engine is no longer available");
        assert_eq!(reject.details, "request handle outlived engine instance");
    }

    #[test]
    fn order_core_request_returns_system_unavailable_when_engine_is_dropped() {
        let request = {
            let engine = Engine::<(), TestReport>::builder()
                .build()
                .expect("engine must build");
            engine.start_pre_trade(()).expect("start stage must pass")
        };

        let result = request.execute();
        assert!(
            result.is_err(),
            "request must fail when engine is unavailable"
        );
        let rejects = result.err().expect("rejects must be present");
        assert_eq!(rejects.len(), 1);
        assert_eq!(rejects[0].policy, "Engine");
        assert_eq!(rejects[0].scope, RejectScope::Order);
        assert_eq!(rejects[0].code, RejectCode::SystemUnavailable);
    }

    #[test]
    fn reservation_mutation_callback_is_noop_when_engine_is_dropped() {
        let state = Rc::new(RefCell::new(None));
        let reservation = {
            let engine = Engine::<TestOrder, TestReport>::builder()
                .check_pre_trade_start_policy(StartPolicyMock::pass("start"))
                .pre_trade_policy(MainPolicyMock::with_custom_mutation_and_optional_reject(
                    "main",
                    shared_kill_switch_mutation(
                        Rc::clone(&state),
                        "shared_kill_switch",
                        false,
                        false,
                    ),
                    false,
                    RejectScope::Order,
                ))
                .build()
                .expect("engine must build");

            let request = engine
                .start_pre_trade(order_with_settlement("USD"))
                .expect("start stage must pass");
            request.execute().expect("main stage must pass")
        };

        reservation.commit();
    }

    #[test]
    fn order_core_reservation_mutation_callback_is_noop_when_engine_is_dropped() {
        let state = Rc::new(RefCell::new(None));
        let reservation = {
            let engine = Engine::<(), TestReport>::builder()
                .check_pre_trade_start_policy(CoreStartPolicyMock {
                    name: "core_start",
                    reject: false,
                })
                .pre_trade_policy(CoreMainPolicyMock {
                    name: "core_main",
                    on_apply: Some(Rc::new(shared_kill_switch_mutation(
                        Rc::clone(&state),
                        "core_drop_mutation",
                        true,
                        false,
                    ))),
                    reject: false,
                })
                .build()
                .expect("engine must build");

            engine
                .start_pre_trade(())
                .expect("start stage must pass")
                .execute()
                .expect("main stage must pass")
        };

        reservation.commit();
    }

    #[test]
    fn build_error_display_is_stable() {
        let err = EngineBuildError::DuplicatePolicyName { name: "dup" };
        assert_eq!(err.to_string(), "duplicate policy name: dup");
    }

    #[test]
    fn account_adjustment_batch_error_display_is_stable() {
        let err = AccountAdjustmentBatchError {
            index: 2,
            reject: Reject::new(
                "adj_policy",
                RejectScope::Order,
                RejectCode::Other,
                "account adjustment rejected",
                "mock account adjustment policy rejected the adjustment",
            ),
        };
        assert_eq!(
            err.to_string(),
            "account adjustment batch rejected at index 2: [adj_policy] account adjustment rejected: mock account adjustment policy rejected the adjustment"
        );
    }

    #[test]
    fn main_stage_observes_settlement_assets_independently() {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let engine = Engine::<TestOrder, TestReport>::builder()
            .check_pre_trade_start_policy(StartPolicyMock::pass("start"))
            .pre_trade_policy(MainPolicyMock::with_calls(
                "collector",
                Rc::new(Cell::new(0)),
                false,
                false,
                Some(Rc::clone(&seen)),
            ))
            .build()
            .expect("engine must build");

        let request_usd = engine
            .start_pre_trade(order_with_settlement("USD"))
            .expect("USD order must pass start stage");
        let reservation_usd = request_usd.execute().expect("USD order must pass");
        reservation_usd.commit();

        let request_eur = engine
            .start_pre_trade(order_with_settlement("EUR"))
            .expect("EUR order must pass start stage");
        let reservation_eur = request_eur.execute().expect("EUR order must pass");
        reservation_eur.commit();

        let seen = seen.borrow();
        assert_eq!(seen.len(), 2);
        assert_eq!(
            seen[0],
            Asset::new("USD").expect("asset code must be valid")
        );
        assert_eq!(
            seen[1],
            Asset::new("EUR").expect("asset code must be valid")
        );
    }

    #[test]
    fn reset_like_start_policy_state_allows_trading_to_resume() {
        let blocked = Rc::new(Cell::new(true));
        let engine = Engine::<TestOrder, TestReport>::builder()
            .check_pre_trade_start_policy(StartPolicyMock::with_block_flag(
                "toggle",
                Rc::clone(&blocked),
            ))
            .build()
            .expect("engine must build");

        let first = engine.start_pre_trade(order_with_settlement("USD"));
        assert!(first.is_err());

        blocked.set(false);

        let second = engine.start_pre_trade(order_with_settlement("USD"));
        assert!(second.is_ok());
    }

    #[test]
    fn tagged_build_rejects_duplicate_policy_names_across_stages() {
        let journal = Rc::new(RefCell::new(Vec::new()));
        let seen_orders = Rc::new(RefCell::new(Vec::new()));
        let seen_reports = Rc::new(RefCell::new(Vec::new()));

        let result = Engine::<TaggedOrder, TaggedReport>::builder()
            .check_pre_trade_start_policy(CaptureTaggedStartPolicy::new(
                "dup",
                Rc::clone(&journal),
                Rc::clone(&seen_orders),
                Rc::clone(&seen_reports),
            ))
            .pre_trade_policy(CaptureTaggedMainPolicy::new(
                "dup",
                Rc::clone(&journal),
                Rc::clone(&seen_orders),
                Rc::clone(&seen_reports),
            ))
            .build();

        assert!(matches!(
            result,
            Err(EngineBuildError::DuplicatePolicyName { name: "dup" })
        ));
    }

    #[test]
    fn tagged_build_rejects_duplicate_policy_names_within_start_stage() {
        let journal = Rc::new(RefCell::new(Vec::new()));
        let seen_orders = Rc::new(RefCell::new(Vec::new()));
        let seen_reports = Rc::new(RefCell::new(Vec::new()));

        let result = Engine::<TaggedOrder, TaggedReport>::builder()
            .check_pre_trade_start_policy(CaptureTaggedStartPolicy::new(
                "dup",
                Rc::clone(&journal),
                Rc::clone(&seen_orders),
                Rc::clone(&seen_reports),
            ))
            .check_pre_trade_start_policy(SequenceFenceStartPolicy::new("dup", Rc::clone(&journal)))
            .build();

        assert!(matches!(
            result,
            Err(EngineBuildError::DuplicatePolicyName { name: "dup" })
        ));
    }

    #[test]
    fn tagged_start_pre_trade_rejects_before_request_is_created() {
        let engine = Engine::<TaggedOrder, TaggedReport>::builder()
            .check_pre_trade_start_policy(RejectTaggedStartPolicyMock {
                name: "tagged_start_reject",
            })
            .build()
            .expect("engine must build");

        let result = engine.start_pre_trade(tagged_order("ord-reject", "AAPL", "1", "10"));
        assert!(matches!(
            result,
            Err(Reject {
                policy: "tagged_start_reject",
                code: RejectCode::Other,
                ..
            })
        ));

        let post_trade =
            engine.apply_execution_report(&tagged_execution_report("rep-reject", "AAPL", "1", "1"));
        assert!(!post_trade.kill_switch_triggered);
    }

    #[test]
    fn tagged_request_returns_system_unavailable_when_engine_is_dropped() {
        let request = {
            let engine = Engine::<TaggedOrder, TaggedReport>::builder()
                .build()
                .expect("engine must build");
            engine
                .start_pre_trade(tagged_order("ord-dropped", "AAPL", "1", "10"))
                .expect("start stage must pass")
        };

        let result = request.execute();
        assert!(
            result.is_err(),
            "request must fail when engine is unavailable"
        );
        let rejects = result.err().expect("rejects must be present");
        assert_eq!(rejects.len(), 1);
        assert_eq!(rejects[0].policy, "Engine");
        assert_eq!(rejects[0].scope, RejectScope::Order);
        assert_eq!(rejects[0].code, RejectCode::SystemUnavailable);
    }

    #[test]
    fn tagged_execute_rejects_and_rolls_back_mutations() {
        let state = Rc::new(RefCell::new(None));
        let engine = Engine::<TaggedOrder, TaggedReport>::builder()
            .pre_trade_policy(TaggedMutationPolicyMock {
                name: "tagged_main",
                on_apply: Some(Rc::new(shared_kill_switch_mutation(
                    Rc::clone(&state),
                    "tagged_reject_mutation",
                    true,
                    false,
                ))),
                reject: true,
            })
            .build()
            .expect("engine must build");

        let request = engine
            .start_pre_trade(tagged_order("ord-tagged-reject", "AAPL", "2", "11"))
            .expect("start stage must create request");
        let result = request.execute();
        assert!(result.is_err(), "main stage must reject");
        let rejects = result.err().expect("rejects must be present");
        assert_eq!(rejects.len(), 1);
        assert_eq!(rejects[0].policy, "tagged_main");
        assert_eq!(rejects[0].code, RejectCode::Other);
        assert_eq!(*state.borrow(), Some(false));

        let post_trade = engine.apply_execution_report(&tagged_execution_report(
            "rep-tagged-reject",
            "AAPL",
            "2",
            "1",
        ));
        assert!(!post_trade.kill_switch_triggered);
    }

    #[test]
    fn tagged_execute_commit_and_rollback_apply_mutation_callback() {
        let cases = [
            (FinalizeAction::Commit, true),
            (FinalizeAction::Rollback, false),
        ];

        for (action, expected_state) in cases {
            let state = Rc::new(RefCell::new(None));
            let engine = Engine::<TaggedOrder, TaggedReport>::builder()
                .pre_trade_policy(TaggedMutationPolicyMock {
                    name: "tagged_main",
                    on_apply: Some(Rc::new(shared_kill_switch_mutation(
                        Rc::clone(&state),
                        "tagged_finalize_mutation",
                        true,
                        false,
                    ))),
                    reject: false,
                })
                .build()
                .expect("engine must build");

            let reservation = engine
                .start_pre_trade(tagged_order("ord-tagged-finalize", "MSFT", "3", "12"))
                .expect("start stage must create request")
                .execute()
                .expect("main stage must pass");

            match action {
                FinalizeAction::Commit => reservation.commit(),
                FinalizeAction::Rollback => reservation.rollback(),
            }

            assert_eq!(*state.borrow(), Some(expected_state));

            let post_trade = engine.apply_execution_report(&tagged_execution_report(
                "rep-tagged-finalize",
                "MSFT",
                "3",
                "1",
            ));
            assert!(!post_trade.kill_switch_triggered);
        }
    }

    #[test]
    fn tagged_reservation_mutation_callback_is_noop_when_engine_is_dropped() {
        let state = Rc::new(RefCell::new(None));
        let reservation = {
            let engine = Engine::<TaggedOrder, TaggedReport>::builder()
                .pre_trade_policy(TaggedMutationPolicyMock {
                    name: "tagged_main",
                    on_apply: Some(Rc::new(shared_kill_switch_mutation(
                        Rc::clone(&state),
                        "tagged_drop_mutation",
                        true,
                        false,
                    ))),
                    reject: false,
                })
                .build()
                .expect("engine must build");

            engine
                .start_pre_trade(tagged_order("ord-tagged-drop", "AAPL", "1", "10"))
                .expect("start stage must pass")
                .execute()
                .expect("main stage must pass")
        };

        reservation.commit();
    }

    #[test]
    fn interleaved_requests_and_reports_preserve_original_tags_across_all_policies() {
        struct Case {
            execute_order: [usize; 3],
            finalize_actions: [FinalizeAction; 3],
            report_order: [usize; 3],
        }

        let cases = [
            Case {
                execute_order: [2, 0, 1],
                finalize_actions: [
                    FinalizeAction::Rollback,
                    FinalizeAction::Commit,
                    FinalizeAction::Rollback,
                ],
                report_order: [1, 2, 0],
            },
            Case {
                execute_order: [1, 2, 0],
                finalize_actions: [
                    FinalizeAction::Commit,
                    FinalizeAction::Rollback,
                    FinalizeAction::Commit,
                ],
                report_order: [2, 0, 1],
            },
            Case {
                execute_order: [0, 2, 1],
                finalize_actions: [
                    FinalizeAction::Commit,
                    FinalizeAction::Commit,
                    FinalizeAction::Rollback,
                ],
                report_order: [0, 1, 2],
            },
        ];

        for case in cases {
            let journal = Rc::new(RefCell::new(Vec::new()));
            let start_seen_orders = Rc::new(RefCell::new(Vec::new()));
            let start_seen_reports = Rc::new(RefCell::new(Vec::new()));
            let main_seen_orders = Rc::new(RefCell::new(Vec::new()));
            let main_seen_reports = Rc::new(RefCell::new(Vec::new()));

            let engine = Engine::<TaggedOrder, TaggedReport>::builder()
                .check_pre_trade_start_policy(CaptureTaggedStartPolicy::new(
                    "capture_start",
                    Rc::clone(&journal),
                    Rc::clone(&start_seen_orders),
                    Rc::clone(&start_seen_reports),
                ))
                .check_pre_trade_start_policy(SequenceFenceStartPolicy::new(
                    "sequence_start",
                    Rc::clone(&journal),
                ))
                .pre_trade_policy(CaptureTaggedMainPolicy::new(
                    "capture_main",
                    Rc::clone(&journal),
                    Rc::clone(&main_seen_orders),
                    Rc::clone(&main_seen_reports),
                ))
                .pre_trade_policy(SequenceFenceMainPolicy::new(
                    "sequence_main",
                    Rc::clone(&journal),
                ))
                .build()
                .expect("engine must build");

            let orders = [
                tagged_order("ord-a", "AAPL", "10", "25"),
                tagged_order("ord-b", "MSFT", "11", "26"),
                tagged_order("ord-c", "TSLA", "12", "27"),
            ];
            let reports = [
                tagged_execution_report("rep-a", "AAPL", "5", "1"),
                tagged_execution_report("rep-b", "MSFT", "6", "1"),
                tagged_execution_report("rep-c", "TSLA", "7", "1"),
            ];

            let mut requests: Vec<_> = orders
                .iter()
                .cloned()
                .map(|order| {
                    Some(
                        engine
                            .start_pre_trade(order)
                            .expect("start stage must pass for tagged order"),
                    )
                })
                .collect();

            for (request_index, action) in
                case.execute_order.iter().zip(case.finalize_actions.iter())
            {
                let request = requests[*request_index]
                    .take()
                    .expect("request must be available exactly once");
                let reservation = request
                    .execute()
                    .expect("main stage must pass for tagged order");

                match action {
                    FinalizeAction::Commit => reservation.commit(),
                    FinalizeAction::Rollback => reservation.rollback(),
                }
                journal.borrow_mut().push(format!(
                    "finalize:{}:{}",
                    action.as_str(),
                    orders[*request_index].tag
                ));
            }

            for report_index in case.report_order {
                let post_trade = engine.apply_execution_report(&reports[report_index]);
                assert!(!post_trade.kill_switch_triggered);
            }

            assert_eq!(*start_seen_orders.borrow(), vec!["ord-a", "ord-b", "ord-c"]);
            assert_eq!(
                *main_seen_orders.borrow(),
                case.execute_order
                    .iter()
                    .map(|index| orders[*index].tag)
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                *start_seen_reports.borrow(),
                case.report_order
                    .iter()
                    .map(|index| reports[*index].tag)
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                *main_seen_reports.borrow(),
                case.report_order
                    .iter()
                    .map(|index| reports[*index].tag)
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                *journal.borrow(),
                expected_interleaving_journal(
                    &case.execute_order,
                    &case.finalize_actions,
                    &case.report_order,
                )
            );
        }
    }

    #[test]
    fn start_pre_trade_allows_extreme_price_without_notional_precompute() {
        let engine = Engine::<TestOrder, TestReport>::builder()
            .build()
            .expect("engine must build");
        let order = WithOrderOperation {
            inner: (),
            operation: OrderOperation {
                instrument: Instrument::new(
                    Asset::new("AAPL").expect("asset code must be valid"),
                    Asset::new("USD").expect("asset code must be valid"),
                ),
                account_id: AccountId::from_u64(99224416),
                side: Side::Buy,
                trade_amount: TradeAmount::Quantity(
                    Quantity::from_str("1").expect("quantity must be valid"),
                ),
                price: Some(Price::from_str("100").expect("price must be valid")),
            },
        };

        let reservation = engine
            .start_pre_trade(order)
            .expect("request must be created without notional precompute")
            .execute()
            .expect("execute without policies must pass");
        reservation.rollback();
    }

    #[test]
    fn sell_order_can_reserve_notional_without_engine_notional_cache() {
        let usd = Asset::new("USD").expect("asset code must be valid");
        let reserved_amount = Volume::from_str("20000").expect("volume must be valid");
        let reserved_notional = Rc::new(RefCell::new(None));
        let engine = Engine::<TestOrder, TestReport>::builder()
            .pre_trade_policy(ReserveNotionalPolicy {
                settlement: usd.clone(),
                amount: reserved_amount,
                reserved_notional: Rc::clone(&reserved_notional),
            })
            .build()
            .expect("engine must build");

        let order = WithOrderOperation {
            inner: (),
            operation: OrderOperation {
                instrument: Instrument::new(
                    Asset::new("AAPL").expect("asset code must be valid"),
                    usd.clone(),
                ),
                account_id: AccountId::from_u64(99224416),
                side: Side::Sell,
                trade_amount: TradeAmount::Quantity(
                    Quantity::from_str("100").expect("quantity must be valid"),
                ),
                price: Some(Price::from_str("200").expect("price must be valid")),
            },
        };

        let reservation = engine
            .start_pre_trade(order)
            .expect("sell order must pass start stage")
            .execute()
            .expect("sell order must pass execute");
        reservation.commit();
        assert_eq!(*reserved_notional.borrow(), Some(reserved_amount));

        let post_trade = engine.apply_execution_report(&execution_report("USD"));
        assert!(!post_trade.kill_switch_triggered);
    }

    #[test]
    #[should_panic(expected = "quantity-based order expected")]
    fn reserve_notional_policy_panics_when_volume_order_is_passed() {
        let policy = ReserveNotionalPolicy {
            settlement: Asset::new("USD").expect("asset code must be valid"),
            amount: Volume::from_str("100").expect("volume must be valid"),
            reserved_notional: Rc::new(RefCell::new(None)),
        };
        let order = WithOrderOperation {
            inner: (),
            operation: OrderOperation {
                instrument: Instrument::new(
                    Asset::new("AAPL").expect("asset code must be valid"),
                    Asset::new("USD").expect("asset code must be valid"),
                ),
                account_id: AccountId::from_u64(99224416),
                side: Side::Sell,
                trade_amount: TradeAmount::Volume(
                    Volume::from_str("100").expect("volume must be valid"),
                ),
                price: Some(Price::from_str("200").expect("price must be valid")),
            },
        };
        let ctx = Context::new(&order);
        let mut mutations = Mutations::default();
        let mut rejects = Vec::<Reject>::new();
        policy.perform_pre_trade_check(&ctx, &mut mutations, &mut rejects);
    }

    #[test]
    #[should_panic(expected = "assertion `left == right` failed")]
    fn reserve_notional_policy_panics_for_volume_based_order() {
        let policy = ReserveNotionalPolicy {
            settlement: Asset::new("USD").expect("asset code must be valid"),
            amount: Volume::from_str("100").expect("volume must be valid"),
            reserved_notional: Rc::new(RefCell::new(None)),
        };
        let order = WithOrderOperation {
            inner: (),
            operation: OrderOperation {
                instrument: Instrument::new(
                    Asset::new("AAPL").expect("asset code must be valid"),
                    Asset::new("USD").expect("asset code must be valid"),
                ),
                account_id: AccountId::from_u64(99224416),
                side: Side::Sell,
                trade_amount: TradeAmount::Quantity(
                    Quantity::from_str("100").expect("quantity must be valid"),
                ),
                price: Some(Price::from_str("200").expect("price must be valid")),
            },
        };
        let ctx = Context::new(&order);
        let mut mutations = Mutations::default();
        let mut rejects = Vec::<Reject>::new();

        policy.perform_pre_trade_check(&ctx, &mut mutations, &mut rejects);
    }

    fn order_with_settlement(settlement: &str) -> TestOrder {
        WithOrderOperation {
            inner: (),
            operation: OrderOperation {
                instrument: Instrument::new(
                    Asset::new("AAPL").expect("asset code must be valid"),
                    Asset::new(settlement).expect("asset code must be valid"),
                ),
                account_id: AccountId::from_u64(99224416),
                side: Side::Buy,
                trade_amount: TradeAmount::Quantity(
                    Quantity::from_str("1").expect("quantity must be valid"),
                ),
                price: Some(Price::from_str("100").expect("price must be valid")),
            },
        }
    }

    fn execution_report(settlement: &str) -> TestReport {
        WithFinancialImpact {
            inner: WithExecutionReportOperation {
                inner: (),
                operation: ExecutionReportOperation {
                    instrument: Instrument::new(
                        Asset::new("AAPL").expect("asset code must be valid"),
                        Asset::new(settlement).expect("asset code must be valid"),
                    ),
                    account_id: AccountId::from_u64(99224416),
                    side: Side::Buy,
                },
            },
            financial_impact: FinancialImpact {
                pnl: Pnl::from_str("-10").expect("pnl must be valid"),
                fee: Fee::from_str("1").expect("fee must be valid"),
            },
        }
    }

    #[derive(Clone, Copy)]
    enum FinalizeAction {
        Commit,
        Rollback,
    }

    impl FinalizeAction {
        fn as_str(self) -> &'static str {
            match self {
                Self::Commit => "commit",
                Self::Rollback => "rollback",
            }
        }
    }

    #[derive(Clone)]
    struct TaggedOrder {
        tag: &'static str,
    }

    #[derive(Clone)]
    struct TaggedReport {
        tag: &'static str,
    }

    struct CaptureTaggedStartPolicy {
        name: &'static str,
        journal: Rc<RefCell<Vec<String>>>,
        seen_orders: Rc<RefCell<Vec<&'static str>>>,
        seen_reports: Rc<RefCell<Vec<&'static str>>>,
    }

    impl CaptureTaggedStartPolicy {
        fn new(
            name: &'static str,
            journal: Rc<RefCell<Vec<String>>>,
            seen_orders: Rc<RefCell<Vec<&'static str>>>,
            seen_reports: Rc<RefCell<Vec<&'static str>>>,
        ) -> Self {
            Self {
                name,
                journal,
                seen_orders,
                seen_reports,
            }
        }
    }

    impl CheckPreTradeStartPolicy<TaggedOrder, TaggedReport> for CaptureTaggedStartPolicy {
        fn name(&self) -> &'static str {
            self.name
        }

        fn check_pre_trade_start(&self, order: &TaggedOrder) -> Result<(), Reject> {
            self.seen_orders.borrow_mut().push(order.tag);
            self.journal
                .borrow_mut()
                .push(format!("start:{}:{}", self.name, order.tag));
            Ok(())
        }

        fn apply_execution_report(&self, report: &TaggedReport) -> bool {
            self.seen_reports.borrow_mut().push(report.tag);
            self.journal
                .borrow_mut()
                .push(format!("report-start:{}:{}", self.name, report.tag));
            false
        }
    }

    struct SequenceFenceStartPolicy {
        name: &'static str,
        journal: Rc<RefCell<Vec<String>>>,
    }

    impl SequenceFenceStartPolicy {
        fn new(name: &'static str, journal: Rc<RefCell<Vec<String>>>) -> Self {
            Self { name, journal }
        }
    }

    impl CheckPreTradeStartPolicy<TaggedOrder, TaggedReport> for SequenceFenceStartPolicy {
        fn name(&self) -> &'static str {
            self.name
        }

        fn check_pre_trade_start(&self, order: &TaggedOrder) -> Result<(), Reject> {
            self.journal
                .borrow_mut()
                .push(format!("start:{}:{}", self.name, order.tag));
            Ok(())
        }

        fn apply_execution_report(&self, report: &TaggedReport) -> bool {
            self.journal
                .borrow_mut()
                .push(format!("report-start:{}:{}", self.name, report.tag));
            false
        }
    }

    struct CaptureTaggedMainPolicy {
        name: &'static str,
        journal: Rc<RefCell<Vec<String>>>,
        seen_orders: Rc<RefCell<Vec<&'static str>>>,
        seen_reports: Rc<RefCell<Vec<&'static str>>>,
    }

    impl CaptureTaggedMainPolicy {
        fn new(
            name: &'static str,
            journal: Rc<RefCell<Vec<String>>>,
            seen_orders: Rc<RefCell<Vec<&'static str>>>,
            seen_reports: Rc<RefCell<Vec<&'static str>>>,
        ) -> Self {
            Self {
                name,
                journal,
                seen_orders,
                seen_reports,
            }
        }
    }

    impl Policy<TaggedOrder, TaggedReport> for CaptureTaggedMainPolicy {
        fn name(&self) -> &'static str {
            self.name
        }

        fn perform_pre_trade_check(
            &self,
            ctx: &Context<'_, TaggedOrder>,
            _mutations: &mut Mutations,
            _rejects: &mut Vec<Reject>,
        ) {
            self.seen_orders.borrow_mut().push(ctx.order().tag);
            self.journal
                .borrow_mut()
                .push(format!("execute:{}:{}", self.name, ctx.order().tag));
        }

        fn apply_execution_report(&self, report: &TaggedReport) -> bool {
            self.seen_reports.borrow_mut().push(report.tag);
            self.journal
                .borrow_mut()
                .push(format!("report-main:{}:{}", self.name, report.tag));
            false
        }
    }

    struct SequenceFenceMainPolicy {
        name: &'static str,
        journal: Rc<RefCell<Vec<String>>>,
    }

    impl SequenceFenceMainPolicy {
        fn new(name: &'static str, journal: Rc<RefCell<Vec<String>>>) -> Self {
            Self { name, journal }
        }
    }

    impl Policy<TaggedOrder, TaggedReport> for SequenceFenceMainPolicy {
        fn name(&self) -> &'static str {
            self.name
        }

        fn perform_pre_trade_check(
            &self,
            ctx: &Context<'_, TaggedOrder>,
            _mutations: &mut Mutations,
            _rejects: &mut Vec<Reject>,
        ) {
            self.journal
                .borrow_mut()
                .push(format!("execute:{}:{}", self.name, ctx.order().tag));
        }

        fn apply_execution_report(&self, report: &TaggedReport) -> bool {
            self.journal
                .borrow_mut()
                .push(format!("report-main:{}:{}", self.name, report.tag));
            false
        }
    }

    struct RejectTaggedStartPolicyMock {
        name: &'static str,
    }

    impl CheckPreTradeStartPolicy<TaggedOrder, TaggedReport> for RejectTaggedStartPolicyMock {
        fn name(&self) -> &'static str {
            self.name
        }

        fn check_pre_trade_start(&self, _order: &TaggedOrder) -> Result<(), Reject> {
            Err(Reject::new(
                self.name,
                RejectScope::Order,
                RejectCode::Other,
                "tagged start reject",
                "tagged start policy rejected the order",
            ))
        }

        fn apply_execution_report(&self, _report: &TaggedReport) -> bool {
            false
        }
    }

    struct TaggedMutationPolicyMock {
        name: &'static str,
        on_apply: Option<MutationHook>,
        reject: bool,
    }

    impl Policy<TaggedOrder, TaggedReport> for TaggedMutationPolicyMock {
        fn name(&self) -> &'static str {
            self.name
        }

        fn perform_pre_trade_check(
            &self,
            _ctx: &Context<'_, TaggedOrder>,
            mutations: &mut Mutations,
            rejects: &mut Vec<Reject>,
        ) {
            if let Some(on_apply) = &self.on_apply {
                on_apply(mutations);
            }

            if self.reject {
                rejects.push(Reject::new(
                    self.name,
                    RejectScope::Order,
                    RejectCode::Other,
                    "tagged main reject",
                    "tagged mutation policy rejected the order",
                ));
            }
        }

        fn apply_execution_report(&self, _report: &TaggedReport) -> bool {
            false
        }
    }

    struct CoreStartPolicyMock {
        name: &'static str,
        reject: bool,
    }

    impl CheckPreTradeStartPolicy<(), TestReport> for CoreStartPolicyMock {
        fn name(&self) -> &'static str {
            self.name
        }

        fn check_pre_trade_start(&self, _order: &()) -> Result<(), Reject> {
            if self.reject {
                return Err(Reject::new(
                    self.name,
                    RejectScope::Order,
                    RejectCode::Other,
                    "core start reject",
                    "order core start policy rejected the order",
                ));
            }
            Ok(())
        }

        fn apply_execution_report(&self, _report: &TestReport) -> bool {
            false
        }
    }

    struct CoreMainPolicyMock {
        name: &'static str,
        on_apply: Option<MutationHook>,
        reject: bool,
    }

    impl Policy<(), TestReport> for CoreMainPolicyMock {
        fn name(&self) -> &'static str {
            self.name
        }

        fn perform_pre_trade_check(
            &self,
            _ctx: &Context<'_, ()>,
            mutations: &mut Mutations,
            rejects: &mut Vec<Reject>,
        ) {
            if let Some(on_apply) = &self.on_apply {
                on_apply(mutations);
            }

            if self.reject {
                rejects.push(Reject::new(
                    self.name,
                    RejectScope::Order,
                    RejectCode::Other,
                    "core main reject",
                    "order core main policy rejected the order",
                ));
            }
        }

        fn apply_execution_report(&self, _report: &TestReport) -> bool {
            false
        }
    }

    fn tagged_order(
        tag: &'static str,
        _underlying: &'static str,
        _quantity: &'static str,
        _price: &'static str,
    ) -> TaggedOrder {
        TaggedOrder { tag }
    }

    fn tagged_execution_report(
        tag: &'static str,
        _underlying: &'static str,
        _pnl: &'static str,
        _fee: &'static str,
    ) -> TaggedReport {
        TaggedReport { tag }
    }

    fn expected_interleaving_journal(
        execute_order: &[usize; 3],
        finalize_actions: &[FinalizeAction; 3],
        report_order: &[usize; 3],
    ) -> Vec<String> {
        let order_tags = ["ord-a", "ord-b", "ord-c"];
        let report_tags = ["rep-a", "rep-b", "rep-c"];
        let mut expected = Vec::new();

        for order_tag in order_tags {
            expected.push(format!("start:capture_start:{order_tag}"));
            expected.push(format!("start:sequence_start:{order_tag}"));
        }

        for (request_index, action) in execute_order.iter().zip(finalize_actions.iter()) {
            let order_tag = order_tags[*request_index];
            expected.push(format!("execute:capture_main:{order_tag}"));
            expected.push(format!("execute:sequence_main:{order_tag}"));
            expected.push(format!("finalize:{}:{order_tag}", action.as_str()));
        }

        for report_index in report_order {
            let report_tag = report_tags[*report_index];
            expected.push(format!("report-start:capture_start:{report_tag}"));
            expected.push(format!("report-start:sequence_start:{report_tag}"));
            expected.push(format!("report-main:capture_main:{report_tag}"));
            expected.push(format!("report-main:sequence_main:{report_tag}"));
        }

        expected
    }

    struct StartPolicyMock {
        name: &'static str,
        calls: Rc<Cell<usize>>,
        reject: bool,
        post_trade_trigger: bool,
        light_counter: Option<Rc<Cell<usize>>>,
        block_flag: Option<Rc<Cell<bool>>>,
    }

    impl StartPolicyMock {
        fn new(
            name: &'static str,
            calls: Rc<Cell<usize>>,
            reject: bool,
            post_trade_trigger: bool,
            light_counter: Option<Rc<Cell<usize>>>,
            block_flag: Option<Rc<Cell<bool>>>,
        ) -> Self {
            Self {
                name,
                calls,
                reject,
                post_trade_trigger,
                light_counter,
                block_flag,
            }
        }

        fn pass(name: &'static str) -> Self {
            Self::new(name, Rc::new(Cell::new(0)), false, false, None, None)
        }

        fn with_counter(name: &'static str, counter: Rc<Cell<usize>>) -> Self {
            Self::new(
                name,
                Rc::new(Cell::new(0)),
                false,
                false,
                Some(counter),
                None,
            )
        }

        fn with_block_flag(name: &'static str, block_flag: Rc<Cell<bool>>) -> Self {
            Self::new(
                name,
                Rc::new(Cell::new(0)),
                false,
                false,
                None,
                Some(block_flag),
            )
        }
    }

    impl CheckPreTradeStartPolicy<TestOrder, TestReport> for StartPolicyMock {
        fn name(&self) -> &'static str {
            self.name
        }

        fn check_pre_trade_start(&self, _order: &TestOrder) -> Result<(), Reject> {
            self.calls.set(self.calls.get() + 1);
            if let Some(counter) = &self.light_counter {
                counter.set(counter.get() + 1);
            }
            if let Some(block_flag) = &self.block_flag {
                if block_flag.get() {
                    return Err(Reject::new(
                        self.name,
                        RejectScope::Account,
                        RejectCode::PnlKillSwitchTriggered,
                        "pnl kill switch triggered",
                        "mock policy blocked the account",
                    ));
                }
            }
            if self.reject {
                return Err(Reject::new(
                    self.name,
                    RejectScope::Order,
                    RejectCode::Other,
                    "start reject",
                    "mock start policy rejected the order",
                ));
            }
            Ok(())
        }

        fn apply_execution_report(&self, _report: &TestReport) -> bool {
            self.post_trade_trigger
        }
    }

    struct MainPolicyMock {
        name: &'static str,
        calls: Rc<Cell<usize>>,
        reject: bool,
        reject_scope: RejectScope,
        on_apply: Option<MutationHook>,
        post_trade_trigger: bool,
        seen_settlement: Option<Rc<RefCell<Vec<Asset>>>>,
    }

    impl MainPolicyMock {
        fn pass(name: &'static str) -> Self {
            Self {
                name,
                calls: Rc::new(Cell::new(0)),
                reject: false,
                reject_scope: RejectScope::Order,
                on_apply: None,
                post_trade_trigger: false,
                seen_settlement: None,
            }
        }

        fn with_calls(
            name: &'static str,
            calls: Rc<Cell<usize>>,
            reject: bool,
            post_trade_trigger: bool,
            seen_settlement: Option<Rc<RefCell<Vec<Asset>>>>,
        ) -> Self {
            Self {
                name,
                calls,
                reject,
                reject_scope: RejectScope::Order,
                on_apply: None,
                post_trade_trigger,
                seen_settlement,
            }
        }

        fn with_mutation_and_optional_reject(
            name: &'static str,
            mutation_id: &'static str,
            reject: bool,
            reject_scope: RejectScope,
        ) -> Self {
            let state = Rc::new(RefCell::new(None));
            Self::with_custom_mutation_and_optional_reject(
                name,
                shared_kill_switch_mutation(state, mutation_id, true, false),
                reject,
                reject_scope,
            )
        }

        fn with_custom_mutation_and_optional_reject(
            name: &'static str,
            on_apply: impl Fn(&mut Mutations) + 'static,
            reject: bool,
            reject_scope: RejectScope,
        ) -> Self {
            Self {
                name,
                calls: Rc::new(Cell::new(0)),
                reject,
                reject_scope,
                on_apply: Some(Rc::new(on_apply)),
                post_trade_trigger: false,
                seen_settlement: None,
            }
        }
    }

    impl Policy<TestOrder, TestReport> for MainPolicyMock {
        fn name(&self) -> &'static str {
            self.name
        }

        fn perform_pre_trade_check(
            &self,
            ctx: &Context<'_, TestOrder>,
            mutations: &mut Mutations,
            rejects: &mut Vec<Reject>,
        ) {
            self.calls.set(self.calls.get() + 1);
            if let Some(seen_settlement) = &self.seen_settlement {
                seen_settlement
                    .borrow_mut()
                    .push(ctx.order().operation.instrument.settlement_asset().clone());
            }

            if let Some(on_apply) = &self.on_apply {
                on_apply(mutations);
            }

            if self.reject {
                rejects.push(Reject::new(
                    self.name,
                    self.reject_scope.clone(),
                    RejectCode::Other,
                    "main reject",
                    "mock main-stage policy rejected the order",
                ));
            }
        }

        fn apply_execution_report(&self, _report: &TestReport) -> bool {
            self.post_trade_trigger
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct MockAdjustment {
        id: u32,
        amount: i64,
    }

    struct AdjustmentPolicyMock {
        name: &'static str,
        seen: Rc<RefCell<Vec<MockAdjustment>>>,
        reject_on_id: Option<u32>,
        on_apply: Option<AdjustmentHook>,
    }

    impl AdjustmentPolicyMock {
        fn pass(name: &'static str, seen: Rc<RefCell<Vec<MockAdjustment>>>) -> Self {
            Self {
                name,
                seen,
                reject_on_id: None,
                on_apply: None,
            }
        }

        fn reject_on_id(
            name: &'static str,
            seen: Rc<RefCell<Vec<MockAdjustment>>>,
            reject_on_id: u32,
        ) -> Self {
            Self {
                name,
                seen,
                reject_on_id: Some(reject_on_id),
                on_apply: None,
            }
        }

        fn with_side_effect(
            name: &'static str,
            seen: Rc<RefCell<Vec<MockAdjustment>>>,
            on_apply: impl Fn(&mut Mutations) + 'static,
        ) -> Self {
            Self {
                name,
                seen,
                reject_on_id: None,
                on_apply: Some(Box::new(on_apply)),
            }
        }
    }

    impl AccountAdjustmentPolicy<MockAdjustment> for AdjustmentPolicyMock {
        fn name(&self) -> &'static str {
            self.name
        }

        fn apply_account_adjustment(
            &self,
            _account_id: AccountId,
            adjustment: &MockAdjustment,
            mutations: &mut Mutations,
        ) -> Result<(), Reject> {
            self.seen.borrow_mut().push(*adjustment);
            if let Some(ref on_apply) = self.on_apply {
                on_apply(mutations);
            }
            if self.reject_on_id == Some(adjustment.id) {
                return Err(Reject::new(
                    self.name,
                    RejectScope::Order,
                    RejectCode::Other,
                    "account adjustment rejected",
                    "mock account adjustment policy rejected the adjustment",
                ));
            }
            Ok(())
        }
    }

    struct ReserveNotionalPolicy {
        settlement: Asset,
        amount: Volume,
        reserved_notional: Rc<RefCell<Option<Volume>>>,
    }

    fn shared_kill_switch_mutation(
        state: Rc<RefCell<Option<bool>>>,
        _id: &'static str,
        commit_value: bool,
        rollback_value: bool,
    ) -> impl Fn(&mut Mutations) {
        move |mutations: &mut Mutations| {
            let c = Rc::clone(&state);
            let r = Rc::clone(&state);
            mutations.push(Mutation::new(
                move || {
                    *c.borrow_mut() = Some(commit_value);
                },
                move || {
                    *r.borrow_mut() = Some(rollback_value);
                },
            ));
        }
    }

    impl Policy<TestOrder, TestReport> for ReserveNotionalPolicy {
        fn name(&self) -> &'static str {
            "ReserveNotionalPolicy"
        }

        fn perform_pre_trade_check(
            &self,
            ctx: &Context<'_, TestOrder>,
            mutations: &mut Mutations,
            _rejects: &mut Vec<Reject>,
        ) {
            use crate::core::{HasOrderPrice, HasTradeAmount};
            assert_eq!(ctx.order().operation.side, Side::Sell);
            assert_eq!(
                ctx.order().operation.instrument.settlement_asset(),
                &self.settlement
            );
            let calculated_amount = ctx
                .order()
                .price()
                .expect("price must be present")
                .expect("price value must be Some")
                .calculate_volume(
                    match ctx
                        .order()
                        .trade_amount()
                        .expect("trade_amount must be present")
                    {
                        TradeAmount::Quantity(value) => value,
                        TradeAmount::Volume(_) => panic!("quantity-based order expected"),
                    },
                )
                .expect("volume must be calculable");
            assert_eq!(calculated_amount, self.amount);
            let commit_store = Rc::clone(&self.reserved_notional);
            let rollback_store = Rc::clone(&self.reserved_notional);
            let commit_amount = self.amount;
            mutations.push(Mutation::new(
                move || {
                    *commit_store.borrow_mut() = Some(commit_amount);
                },
                move || {
                    *rollback_store.borrow_mut() = Some(Volume::ZERO);
                },
            ));
        }

        fn apply_execution_report(&self, _report: &TestReport) -> bool {
            false
        }
    }
}
