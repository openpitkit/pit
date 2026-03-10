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
use std::rc::{Rc, Weak};
use std::time::Instant;

use crate::core::Order;
use crate::param::{Asset, CashFlow, Side, Volume};
use crate::pretrade::handles::{RequestHandleImpl, ReservationHandleImpl};
use crate::pretrade::start_pre_trade_time::with_start_pre_trade_now;
use crate::pretrade::{
    CheckPreTradeStartPolicy, Context, ExecutionReport, Mutations, Policy, PostTradeResult, Reject,
    RejectCode, RejectScope, Request, Reservation, RiskMutation,
};

struct EngineInner {
    check_pre_trade_start_policies: Vec<Box<dyn CheckPreTradeStartPolicy>>,
    pre_trade_policies: Vec<Box<dyn Policy>>,
    state: EngineState,
}

#[derive(Default)]
struct EngineState {
    reserved_notional: HashMap<Asset, Volume>,
    kill_switch: HashMap<&'static str, bool>,
}

/// Errors returned by [`EngineBuilder::build`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum EngineBuildError {
    /// Duplicate policy name across start-stage and main-stage policy sets.
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

/// Risk engine orchestrating start-stage and main-stage pre-trade checks.
///
/// Build the engine once during platform initialization using [`Engine::builder`],
/// then share it across order submissions.
///
/// # Thread safety
///
/// `Engine` is `!Send + !Sync`. All calls must happen on the same thread
/// that created it. Synchronization across threads is the caller's
/// responsibility.
///
/// # Examples
///
/// ```
/// use openpit::param::{Asset, Price, Quantity, Side};
/// use openpit::core::Instrument;
/// use openpit::{Engine, Order};
///
/// let engine = Engine::builder().build().expect("valid config");
///
/// let order = Order {
///     instrument: Instrument::new(
///         Asset::new("AAPL").expect("asset literal must be valid"),
///         Asset::new("USD").expect("asset literal must be valid"),
///     ),
///     side: Side::Buy,
///     quantity: Quantity::from_str("100").expect("valid"),
///     price: Price::from_str("185").expect("valid"),
/// };
///
/// let request = engine.start_pre_trade(order).expect("must pass");
/// let reservation = request.execute().expect("must pass");
/// reservation.commit();
/// ```
pub struct Engine {
    inner: Rc<RefCell<EngineInner>>,
}

impl Engine {
    /// Creates an engine builder.
    pub fn builder() -> EngineBuilder {
        EngineBuilder::new()
    }

    /// Executes start-stage checks and creates a deferred [`Request`].
    ///
    /// Start-stage policies run in registration order and stop at the first reject.
    /// Notional is precomputed before the main-stage policies run.
    ///
    /// # Errors
    ///
    /// Returns [`Reject`] when any start-stage policy rejects the order, or when
    /// the notional calculation overflows.
    pub fn start_pre_trade(&self, order: Order) -> Result<Request, Reject> {
        let now: Instant = Instant::now();
        with_start_pre_trade_now(now, || {
            let inner = self.inner.borrow();
            for policy in &inner.check_pre_trade_start_policies {
                policy.check_pre_trade_start(&order)?;
            }
            Ok::<(), Reject>(())
        })?;

        let notional = calculate_notional(&order)?;
        let engine = Rc::downgrade(&self.inner);
        let request_handle =
            RequestHandleImpl::new(Box::new(move || execute_request(engine, order, notional)));

        Ok(Request::from_handle(Box::new(request_handle)))
    }

    /// Applies post-trade updates and aggregates kill-switch status across all policies.
    ///
    /// Returns [`PostTradeResult::kill_switch_triggered`] `true` when at least one policy
    /// reports a kill-switch condition.
    pub fn apply_execution_report(&self, report: &ExecutionReport) -> PostTradeResult {
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
}

/// Fluent builder for [`Engine`].
///
/// Policies are evaluated in registration order. Policy names must be unique
/// across both start-stage and main-stage sets; [`EngineBuilder::build`] returns
/// [`EngineBuildError::DuplicatePolicyName`] otherwise.
///
/// # Examples
///
/// ```
/// use std::time::Duration;
/// use openpit::param::{Asset, Pnl, Price, Quantity, Side};
/// use openpit::pretrade::policies::PnlKillSwitchPolicy;
/// use openpit::pretrade::policies::RateLimitPolicy;
/// use openpit::core::Instrument;
/// use openpit::{Engine, Order};
///
/// let pnl_policy = PnlKillSwitchPolicy::new(
///     (
///         Asset::new("USD").expect("asset code must be valid"),
///         Pnl::from_str("500").expect("valid"),
///     ),
///     [],
/// )
/// .expect("policy config must be valid");
///
/// let engine = Engine::builder()
///     .check_pre_trade_start_policy(pnl_policy)
///     .check_pre_trade_start_policy(RateLimitPolicy::new(100, Duration::from_secs(1)))
///     .build()
///     .expect("engine config must be valid");
/// ```
pub struct EngineBuilder {
    check_pre_trade_start_policies: Vec<Box<dyn CheckPreTradeStartPolicy>>,
    pre_trade_policies: Vec<Box<dyn Policy>>,
}

impl EngineBuilder {
    /// Creates a new builder.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            check_pre_trade_start_policies: Vec::new(),
            pre_trade_policies: Vec::new(),
        }
    }

    /// Registers a start-stage policy.
    pub fn check_pre_trade_start_policy<P>(mut self, policy: P) -> Self
    where
        P: CheckPreTradeStartPolicy + 'static,
    {
        self.check_pre_trade_start_policies.push(Box::new(policy));
        self
    }

    /// Registers a main-stage policy.
    pub fn pre_trade_policy<P>(mut self, policy: P) -> Self
    where
        P: Policy + 'static,
    {
        self.pre_trade_policies.push(Box::new(policy));
        self
    }

    /// Builds the engine.
    pub fn build(self) -> Result<Engine, EngineBuildError> {
        ensure_unique_policy_names(
            &self.check_pre_trade_start_policies,
            &self.pre_trade_policies,
        )?;

        Ok(Engine {
            inner: Rc::new(RefCell::new(EngineInner {
                check_pre_trade_start_policies: self.check_pre_trade_start_policies,
                pre_trade_policies: self.pre_trade_policies,
                state: EngineState::default(),
            })),
        })
    }
}

fn ensure_unique_policy_names(
    check_pre_trade_start_policies: &[Box<dyn CheckPreTradeStartPolicy>],
    pre_trade_policies: &[Box<dyn Policy>],
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

    Ok(())
}

fn execute_request(
    engine: Weak<RefCell<EngineInner>>,
    order: Order,
    notional: CashFlow,
) -> Result<Reservation, Vec<Reject>> {
    let Some(engine_ref) = engine.upgrade() else {
        return Err(vec![Reject::new(
            "Engine",
            RejectScope::Order,
            RejectCode::SystemUnavailable,
            "engine is no longer available",
            "request handle outlived engine instance".to_owned(),
        )]);
    };
    let mut inner = engine_ref.borrow_mut();

    let mut mutations = Mutations::new();
    let mut rejects = Vec::new();
    let ctx = Context::new(&order, notional);

    for policy in &inner.pre_trade_policies {
        policy.perform_pre_trade_check(&ctx, &mut mutations, &mut rejects);
    }

    if !rejects.is_empty() {
        for mutation in mutations.as_slice().iter().rev() {
            apply_mutation(&mut inner.state, &mutation.rollback);
        }
        return Err(rejects);
    }

    drop(inner);

    let reservation_engine = engine;
    let reservation_handle = ReservationHandleImpl::new(
        mutations.into_vec(),
        Box::new(move |mutation| {
            let Some(engine_ref) = reservation_engine.upgrade() else {
                return;
            };
            let mut inner = engine_ref.borrow_mut();
            apply_mutation(&mut inner.state, mutation);
        }),
    );

    Ok(Reservation::from_handle(Box::new(reservation_handle)))
}

fn calculate_notional(order: &Order) -> Result<CashFlow, Reject> {
    let volume = order.price.calculate_volume(order.quantity).map_err(|_| {
        Reject::new(
            "Engine",
            RejectScope::Order,
            RejectCode::OrderValueCalculationFailed,
            "order notional overflow",
            format!(
                "requested price {}, requested quantity {}; could not calculate order notional",
                order.price, order.quantity
            ),
        )
    })?;

    let notional = match order.side {
        Side::Buy => volume.to_cash_flow_outflow(),
        Side::Sell => volume.to_cash_flow_inflow(),
    };
    Ok(notional)
}

fn apply_mutation(state: &mut EngineState, mutation: &RiskMutation) {
    match mutation {
        RiskMutation::ReserveNotional { asset, amount } => {
            state.reserved_notional.insert(asset.clone(), *amount);
        }
        RiskMutation::SetKillSwitch { id, enabled } => {
            state.kill_switch.insert(*id, *enabled);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    use crate::core::{Instrument, Order};
    use crate::param::{Asset, Fee, Pnl, Price, Quantity, Side, Volume};
    use crate::pretrade::{
        CheckPreTradeStartPolicy, Context, ExecutionReport, Mutation, Mutations, Policy, Reject,
        RejectCode, RejectScope, RiskMutation,
    };
    use rust_decimal::Decimal;

    use super::{Engine, EngineBuildError};

    #[test]
    fn build_rejects_duplicate_policy_names_across_stages() {
        let result = Engine::builder()
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
        let result = Engine::builder()
            .check_pre_trade_start_policy(StartPolicyMock::pass("dup"))
            .check_pre_trade_start_policy(StartPolicyMock::pass("dup"))
            .build();

        assert!(matches!(
            result,
            Err(EngineBuildError::DuplicatePolicyName { name: "dup" })
        ));
    }

    #[test]
    fn builder_builds_operational_empty_engine() {
        let reservation = Engine::builder()
            .build()
            .expect("builder must build")
            .start_pre_trade(order_with_settlement("USD"))
            .expect("built engine must allow start stage")
            .execute()
            .expect("built engine must allow execute");
        reservation.rollback();
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

            let start_0 = StartPolicyMock::new(
                "s0",
                Rc::clone(&calls_0),
                case.reject_index == Some(0),
                false,
            );
            let start_1 = StartPolicyMock::new(
                "s1",
                Rc::clone(&calls_1),
                case.reject_index == Some(1),
                false,
            );
            let start_2 = StartPolicyMock::new(
                "s2",
                Rc::clone(&calls_2),
                case.reject_index == Some(2),
                false,
            );

            let engine = Engine::builder()
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
            let engine = Engine::builder()
                .check_pre_trade_start_policy(StartPolicyMock::pass("start"))
                .pre_trade_policy(MainPolicyMock::with_custom_mutation_and_optional_reject(
                    "m1_policy",
                    shared_kill_switch_mutation(false, false),
                    case.fail_first,
                    RejectScope::Order,
                ))
                .pre_trade_policy(MainPolicyMock::with_custom_mutation_and_optional_reject(
                    "m2_policy",
                    shared_kill_switch_mutation(true, true),
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

            let inner = engine.inner.borrow();
            assert_eq!(
                inner.state.kill_switch.get("shared_kill_switch").copied(),
                Some(case.expected_kill_switch)
            );
        }
    }

    #[test]
    fn light_stage_changes_are_not_rolled_back_when_execute_rejects() {
        let light_counter = Rc::new(Cell::new(0));
        let engine = Engine::builder()
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
        let engine = Engine::builder()
            .check_pre_trade_start_policy(StartPolicyMock::pass("start"))
            .pre_trade_policy(MainPolicyMock::with_custom_mutation_and_optional_reject(
                "m1_policy",
                shared_kill_switch_mutation(false, false),
                false,
                RejectScope::Order,
            ))
            .pre_trade_policy(MainPolicyMock::with_custom_mutation_and_optional_reject(
                "m2_policy",
                shared_kill_switch_mutation(true, true),
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

        let inner = engine.inner.borrow();
        assert_eq!(
            inner.state.kill_switch.get("shared_kill_switch").copied(),
            Some(false)
        );
    }

    #[test]
    fn apply_execution_report_aggregates_kill_switch_triggered() {
        let engine = Engine::builder()
            .check_pre_trade_start_policy(StartPolicyMock::new(
                "start_false",
                Rc::new(Cell::new(0)),
                false,
                false,
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
            let engine = Engine::builder().build().expect("engine must build");
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
    fn reservation_mutation_callback_is_noop_when_engine_is_dropped() {
        let reservation = {
            let engine = Engine::builder()
                .check_pre_trade_start_policy(StartPolicyMock::pass("start"))
                .pre_trade_policy(MainPolicyMock::with_custom_mutation_and_optional_reject(
                    "main",
                    shared_kill_switch_mutation(false, false),
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
    fn build_error_display_is_stable() {
        let err = EngineBuildError::DuplicatePolicyName { name: "dup" };
        assert_eq!(err.to_string(), "duplicate policy name: dup");
    }

    #[test]
    fn main_stage_observes_settlement_assets_independently() {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let engine = Engine::builder()
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
        let engine = Engine::builder()
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
    fn start_pre_trade_rejects_notional_overflow() {
        let engine = Engine::builder().build().expect("engine must build");
        let order = Order {
            instrument: Instrument::new(
                Asset::new("AAPL").expect("asset code must be valid"),
                Asset::new("USD").expect("asset code must be valid"),
            ),
            side: Side::Buy,
            quantity: Quantity::from_str("2").expect("quantity must be valid"),
            price: Price::new(Decimal::MAX),
        };

        let result = engine.start_pre_trade(order);
        assert!(
            result.is_err(),
            "overflow must reject before request creation"
        );
        let reject = result.err().expect("reject must be present");
        assert_eq!(reject.policy, "Engine");
        assert_eq!(reject.scope, RejectScope::Order);
        assert_eq!(reject.code, RejectCode::OrderValueCalculationFailed);
        assert_eq!(reject.reason, "order notional overflow");
        assert_eq!(
            reject.details,
            format!(
                "requested price {}, requested quantity 2; could not calculate order notional",
                Price::new(Decimal::MAX)
            )
        );
    }

    #[test]
    fn sell_order_uses_inflow_notional_and_applies_reserve_mutation() {
        let usd = Asset::new("USD").expect("asset code must be valid");
        let expected_notional = Volume::from_str("20000")
            .expect("volume must be valid")
            .to_cash_flow_inflow();
        let reserved_amount = Volume::from_str("20000").expect("volume must be valid");
        let engine = Engine::builder()
            .pre_trade_policy(ReserveNotionalPolicy {
                expected_notional,
                settlement: usd.clone(),
                amount: reserved_amount,
            })
            .build()
            .expect("engine must build");

        let order = Order {
            instrument: Instrument::new(
                Asset::new("AAPL").expect("asset code must be valid"),
                usd.clone(),
            ),
            side: Side::Sell,
            quantity: Quantity::from_str("100").expect("quantity must be valid"),
            price: Price::from_str("200").expect("price must be valid"),
        };

        let reservation = engine
            .start_pre_trade(order)
            .expect("sell order must pass start stage")
            .execute()
            .expect("sell order must pass execute");
        reservation.commit();

        let inner = engine.inner.borrow();
        assert_eq!(
            inner.state.reserved_notional.get(&usd).copied(),
            Some(reserved_amount)
        );
        assert!(inner.state.kill_switch.is_empty());
    }

    fn order_with_settlement(settlement: &str) -> Order {
        Order {
            instrument: Instrument::new(
                Asset::new("AAPL").expect("asset code must be valid"),
                Asset::new(settlement).expect("asset code must be valid"),
            ),
            side: Side::Buy,
            quantity: Quantity::from_str("100").expect("quantity must be valid"),
            price: Price::from_str("200").expect("price must be valid"),
        }
    }

    fn execution_report(settlement: &str) -> ExecutionReport {
        ExecutionReport {
            instrument: Instrument::new(
                Asset::new("AAPL").expect("asset code must be valid"),
                Asset::new(settlement).expect("asset code must be valid"),
            ),
            pnl: Pnl::from_str("-10").expect("pnl must be valid"),
            fee: Fee::from_str("1").expect("fee must be valid"),
        }
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
        ) -> Self {
            Self {
                name,
                calls,
                reject,
                post_trade_trigger,
                light_counter: None,
                block_flag: None,
            }
        }

        fn pass(name: &'static str) -> Self {
            Self::new(name, Rc::new(Cell::new(0)), false, false)
        }

        fn with_counter(name: &'static str, counter: Rc<Cell<usize>>) -> Self {
            Self {
                name,
                calls: Rc::new(Cell::new(0)),
                reject: false,
                post_trade_trigger: false,
                light_counter: Some(counter),
                block_flag: None,
            }
        }

        fn with_block_flag(name: &'static str, block_flag: Rc<Cell<bool>>) -> Self {
            Self {
                name,
                calls: Rc::new(Cell::new(0)),
                reject: false,
                post_trade_trigger: false,
                light_counter: None,
                block_flag: Some(block_flag),
            }
        }
    }

    impl CheckPreTradeStartPolicy for StartPolicyMock {
        fn name(&self) -> &'static str {
            self.name
        }

        fn check_pre_trade_start(&self, _order: &Order) -> Result<(), Reject> {
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

        fn apply_execution_report(&self, _report: &ExecutionReport) -> bool {
            self.post_trade_trigger
        }
    }

    struct MainPolicyMock {
        name: &'static str,
        calls: Rc<Cell<usize>>,
        reject: bool,
        reject_scope: RejectScope,
        mutation: Option<Mutation>,
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
                mutation: None,
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
                mutation: None,
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
            Self::with_custom_mutation_and_optional_reject(
                name,
                Mutation {
                    commit: RiskMutation::SetKillSwitch {
                        id: mutation_id,
                        enabled: true,
                    },
                    rollback: RiskMutation::SetKillSwitch {
                        id: mutation_id,
                        enabled: false,
                    },
                },
                reject,
                reject_scope,
            )
        }

        fn with_custom_mutation_and_optional_reject(
            name: &'static str,
            mutation: Mutation,
            reject: bool,
            reject_scope: RejectScope,
        ) -> Self {
            Self {
                name,
                calls: Rc::new(Cell::new(0)),
                reject,
                reject_scope,
                mutation: Some(mutation),
                post_trade_trigger: false,
                seen_settlement: None,
            }
        }
    }

    impl Policy for MainPolicyMock {
        fn name(&self) -> &'static str {
            self.name
        }

        fn perform_pre_trade_check(
            &self,
            ctx: &Context<'_>,
            mutations: &mut Mutations,
            rejects: &mut Vec<Reject>,
        ) {
            self.calls.set(self.calls.get() + 1);
            if let Some(seen_settlement) = &self.seen_settlement {
                seen_settlement
                    .borrow_mut()
                    .push(ctx.order().instrument.settlement_asset().clone());
            }

            if let Some(mutation) = &self.mutation {
                mutations.push(mutation.clone());
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

        fn apply_execution_report(&self, _report: &ExecutionReport) -> bool {
            self.post_trade_trigger
        }
    }

    struct ReserveNotionalPolicy {
        expected_notional: crate::param::CashFlow,
        settlement: Asset,
        amount: Volume,
    }

    fn shared_kill_switch_mutation(commit_enabled: bool, rollback_enabled: bool) -> Mutation {
        Mutation {
            commit: RiskMutation::SetKillSwitch {
                id: "shared_kill_switch",
                enabled: commit_enabled,
            },
            rollback: RiskMutation::SetKillSwitch {
                id: "shared_kill_switch",
                enabled: rollback_enabled,
            },
        }
    }

    impl Policy for ReserveNotionalPolicy {
        fn name(&self) -> &'static str {
            "ReserveNotionalPolicy"
        }

        fn perform_pre_trade_check(
            &self,
            ctx: &Context<'_>,
            mutations: &mut Mutations,
            _rejects: &mut Vec<Reject>,
        ) {
            assert_eq!(ctx.notional(), self.expected_notional);
            mutations.push(Mutation {
                commit: RiskMutation::ReserveNotional {
                    asset: self.settlement.clone(),
                    amount: self.amount,
                },
                rollback: RiskMutation::ReserveNotional {
                    asset: self.settlement.clone(),
                    amount: Volume::ZERO,
                },
            });
        }
    }
}
