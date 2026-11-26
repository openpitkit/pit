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

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use proptest::prelude::*;

use openpit::param::{
    AccountGroupId, AccountId, Asset, Pnl, PositionSize, Price, Quantity, Side, TradeAmount,
};
use openpit::pretrade::policies::{
    RateLimit, RateLimitBrokerBarrier, RateLimitPolicy, RateLimitSettings, SpotFundsLimitMode,
    SpotFundsPnlBoundsAccountGroupBarrier, SpotFundsPnlBoundsAxis, SpotFundsPnlBoundsBarrier,
    SpotFundsPolicy, SpotFundsPricingSource, SpotFundsSettings,
};
use openpit::pretrade::{
    AccountBlock, PolicyPreTradeResult, PostTradeContext, PostTradeResult, PreTradeContext,
    PreTradePolicy, Reject, RejectCode, RejectScope, Rejects, RuleMode,
};
use openpit::storage::NoLocking;
use openpit::{
    AccountAdjustmentAccountPnlOperation, AccountOutcomeEntry, ConfigureError, Engine,
    EngineBuildError, ExecutionReportOperation, HasAccountId, HasAlreadyHeld, HasConfirmed,
    Instrument, LocalEngine, LocalSync, Mutation, Mutations, OrderOperation, OrderReview,
    OutcomeAmount, PnlState, PolicyGroupId, PolicyInstance, RequestFieldAccessError,
    SpotFundsMarketData, WithExecutionReportFillDetails, WithOrderOperation, WithOrderReview,
};

type Order = WithOrderOperation<WithOrderReview<()>>;
type MutationEvents = Rc<RefCell<Vec<(PolicyGroupId, &'static str)>>>;
type ReviewFlags = (
    Result<bool, RequestFieldAccessError>,
    Result<bool, RequestFieldAccessError>,
);

#[derive(Clone)]
struct ReviewOrder {
    confirmed: Rc<RefCell<Result<bool, RequestFieldAccessError>>>,
    already_held: Rc<RefCell<Result<bool, RequestFieldAccessError>>>,
    reads: Rc<Cell<(usize, usize)>>,
}

impl ReviewOrder {
    fn new(
        confirmed: Result<bool, RequestFieldAccessError>,
        already_held: Result<bool, RequestFieldAccessError>,
    ) -> Self {
        Self {
            confirmed: Rc::new(RefCell::new(confirmed)),
            already_held: Rc::new(RefCell::new(already_held)),
            reads: Rc::new(Cell::new((0, 0))),
        }
    }
}

impl HasAccountId for ReviewOrder {
    fn account_id(&self) -> Result<AccountId, RequestFieldAccessError> {
        Ok(AccountId::from_u64(1))
    }
}

impl HasConfirmed for ReviewOrder {
    fn confirmed(&self) -> Result<bool, RequestFieldAccessError> {
        let (confirmed, held) = self.reads.get();
        self.reads.set((confirmed + 1, held));
        self.confirmed.borrow().clone()
    }
}

impl HasAlreadyHeld for ReviewOrder {
    fn already_held(&self) -> Result<bool, RequestFieldAccessError> {
        let (confirmed, held) = self.reads.get();
        self.reads.set((confirmed, held + 1));
        self.already_held.borrow().clone()
    }
}

#[derive(Clone, Default)]
struct ReviewFlagPolicy {
    seen: Rc<RefCell<Vec<ReviewFlags>>>,
    ignore_flags: bool,
}

impl ReviewFlagPolicy {
    fn record(&self, ctx: &PreTradeContext<NoLocking>) {
        if self.ignore_flags {
            return;
        }
        let flags = (ctx.confirmed(), ctx.already_held());
        assert_eq!(ctx.confirmed(), flags.0);
        assert_eq!(ctx.already_held(), flags.1);
        self.seen.borrow_mut().push(flags);
    }
}

impl PreTradePolicy<ReviewOrder, (), (), LocalSync> for ReviewFlagPolicy {
    fn name(&self) -> &str {
        "ReviewFlagPolicy"
    }

    fn check_pre_trade_start(
        &self,
        ctx: &PreTradeContext<NoLocking>,
        _order: &ReviewOrder,
    ) -> Result<(), Rejects> {
        self.record(ctx);
        Ok(())
    }

    fn perform_pre_trade_check(
        &self,
        ctx: &PreTradeContext<NoLocking>,
        _order: &ReviewOrder,
        _mutations: &mut Mutations,
    ) -> Result<Option<PolicyPreTradeResult>, Rejects> {
        self.record(ctx);
        Ok(None)
    }
}

#[test]
fn default_instance_uses_policy_name_and_reject_mode() {
    assert_eq!(RuleMode::default(), RuleMode::Reject);
    assert_eq!(PolicyInstance::default().name, None);
    assert_eq!(PolicyInstance::default().mode, RuleMode::Reject);
    let builder = Engine::builder::<Order, (), ()>().no_sync();
    let policy = RateLimitPolicy::new(broker_rate_limit(100), builder.storage_builder());
    let engine = builder.pre_trade(policy).build().unwrap();
    engine
        .configure()
        .rate_limit("RateLimitPolicy", |settings| {
            *settings = broker_rate_limit(1);
            Ok::<_, String>(())
        })
        .unwrap();
    drop(engine.start_pre_trade(order(false)).unwrap());
    let rejects = engine.start_pre_trade(order(false)).unwrap_err();
    assert_eq!(rejects[0].policy, "RateLimitPolicy");
    assert_eq!(rejects[0].mode, RuleMode::Reject);
}

#[test]
fn named_instances_configure_independently_and_keep_policy_name() {
    let builder = Engine::builder::<Order, (), ()>().no_sync();
    let first = RateLimitPolicy::new(broker_rate_limit(100), builder.storage_builder());
    let second = RateLimitPolicy::new(broker_rate_limit(100), builder.storage_builder());
    let engine = builder
        .pre_trade_instance(
            first,
            PolicyInstance {
                name: Some("first".to_owned()),
                ..PolicyInstance::default()
            },
        )
        .pre_trade_instance(
            second,
            PolicyInstance {
                name: Some("second".to_owned()),
                ..PolicyInstance::default()
            },
        )
        .build()
        .unwrap();
    let configure = engine.configure();
    for (name, max_orders) in [("first", 1), ("second", 2)] {
        configure
            .rate_limit(name, |settings| {
                *settings = broker_rate_limit(max_orders);
                Ok::<_, String>(())
            })
            .unwrap();
    }
    assert!(configure
        .rate_limit("RateLimitPolicy", |_| Ok::<_, String>(()))
        .is_err());
    drop(engine.start_pre_trade(order(false)).unwrap());
    let rejects = engine.start_pre_trade(order(false)).unwrap_err();
    assert_eq!(rejects.len(), 1);
    assert_eq!(rejects[0].policy, "RateLimitPolicy");
    let rejects = engine.start_pre_trade(order(false)).unwrap_err();
    assert_eq!(rejects.len(), 2);
    assert!(rejects
        .iter()
        .all(|reject| reject.policy == "RateLimitPolicy"));
}

#[test]
fn named_spot_funds_instance_supports_both_configuration_methods() {
    let builder = Engine::builder::<
        OrderOperation,
        WithExecutionReportFillDetails<ExecutionReportOperation>,
        AccountAdjustmentAccountPnlOperation,
    >()
    .no_sync();
    let settings = SpotFundsSettings::new(0, SpotFundsPricingSource::Mark, []).unwrap();
    let policy = SpotFundsPolicy::<LocalSync, LocalSync>::new(
        settings,
        None::<SpotFundsMarketData<LocalSync>>,
        builder.storage_builder(),
    );
    let engine = builder
        .pre_trade_instance(
            policy,
            PolicyInstance {
                name: Some("funds".to_owned()),
                ..PolicyInstance::default()
            },
        )
        .build()
        .unwrap();
    let configure = engine.configure();
    let account = AccountId::from_u64(1);
    let pnl = PnlState::Value(Pnl::from_str("0").unwrap());
    assert!(configure
        .spot_funds("funds", |_| Ok::<_, String>(()))
        .is_ok());
    assert!(configure
        .set_spot_funds_account_pnl("funds", account, pnl)
        .is_ok());
    let policy_name = SpotFundsPolicy::<LocalSync, LocalSync>::NAME;
    assert!(matches!(
        configure.spot_funds(policy_name, |_| Ok::<_, String>(())),
        Err(ConfigureError::UnknownPolicy { name }) if name == policy_name
    ));
    assert!(matches!(
        configure.set_spot_funds_account_pnl(policy_name, account, pnl),
        Err(ConfigureError::UnknownPolicy { name }) if name == policy_name
    ));
}

#[test]
fn duplicate_instance_names_are_rejected() {
    let result = Engine::builder::<ReviewOrder, (), ()>()
        .no_sync()
        .pre_trade(ReviewFlagPolicy::default())
        .pre_trade(ReviewFlagPolicy::default())
        .build();
    assert_eq!(
        result.err(),
        Some(EngineBuildError::DuplicatePolicyName {
            name: "ReviewFlagPolicy".to_owned(),
        })
    );
    assert!(Engine::builder::<ReviewOrder, (), ()>()
        .no_sync()
        .pre_trade_instance(
            ReviewFlagPolicy::default(),
            PolicyInstance {
                name: Some("first".to_owned()),
                ..PolicyInstance::default()
            }
        )
        .pre_trade_instance(
            ReviewFlagPolicy::default(),
            PolicyInstance {
                name: Some("second".to_owned()),
                ..PolicyInstance::default()
            }
        )
        .build()
        .is_ok());
}

#[test]
fn review_instances_require_an_order_review_reader() {
    for mode in [RuleMode::ReviewWithHold, RuleMode::ReviewWithoutHold] {
        for enabled in [false, true] {
            let builder = Engine::builder::<ReviewOrder, (), ()>();
            let builder = if enabled {
                builder.with_order_review()
            } else {
                builder
            };
            let result = builder
                .no_sync()
                .pre_trade_instance(
                    ReviewFlagPolicy::default(),
                    PolicyInstance {
                        name: Some("review".to_owned()),
                        mode,
                    },
                )
                .build();
            assert_eq!(
                result.err(),
                if enabled {
                    None
                } else {
                    Some(EngineBuildError::OrderReviewRequired {
                        name: "review".to_owned(),
                    })
                }
            );
        }
    }
}

#[test]
fn ready_builder_review_instances_require_an_order_review_reader() {
    for mode in [RuleMode::ReviewWithHold, RuleMode::ReviewWithoutHold] {
        for enabled in [false, true] {
            let builder = Engine::builder::<ReviewOrder, (), ()>();
            let builder = if enabled {
                builder.with_order_review()
            } else {
                builder
            };
            let result = builder
                .no_sync()
                .pre_trade_instance(
                    ReviewFlagPolicy::default(),
                    PolicyInstance {
                        name: Some("reject".to_owned()),
                        ..PolicyInstance::default()
                    },
                )
                .pre_trade_instance(
                    ReviewFlagPolicy::default(),
                    PolicyInstance {
                        name: Some("review".to_owned()),
                        mode,
                    },
                )
                .build();
            assert_eq!(
                result.err(),
                if enabled {
                    None
                } else {
                    Some(EngineBuildError::OrderReviewRequired {
                        name: "review".to_owned(),
                    })
                }
            );
        }
    }
}

#[test]
fn unreadable_review_flags_reject_before_any_policy_hook() {
    for label in ["confirmed", "already_held"] {
        let error = RequestFieldAccessError::new("review_flag_payload");
        let order = ReviewOrder::new(
            if label == "confirmed" {
                Err(error.clone())
            } else {
                Ok(true)
            },
            if label == "already_held" {
                Err(error.clone())
            } else {
                Ok(true)
            },
        );
        let policy = ReviewFlagPolicy::default();
        let engine = Engine::builder::<ReviewOrder, (), ()>()
            .with_order_review()
            .no_sync()
            .pre_trade(policy.clone())
            .build()
            .unwrap();
        let live = engine.start_pre_trade(order.clone()).unwrap_err();
        let start = engine.start_pre_trade_dry_run(order.clone());
        let full = engine.execute_pre_trade_dry_run(order.clone());
        for rejects in [&live, start.rejects().unwrap(), full.rejects().unwrap()] {
            assert_eq!(rejects.len(), 1);
            assert_eq!(rejects[0].policy, "Engine");
            assert_eq!(rejects[0].scope, RejectScope::Order);
            assert_eq!(rejects[0].code, RejectCode::MissingRequiredField);
            assert_eq!(rejects[0].mode, RuleMode::Reject);
            assert_eq!(rejects[0].details, format!("{label}: {error}"));
        }
        assert!(policy.seen.borrow().is_empty());
        assert_eq!(order.reads.get(), (3, 3));
    }
}

#[test]
fn drop_copy_passes_unreadable_review_flags_to_policies() {
    for flags in [
        (
            Err(RequestFieldAccessError::new("confirmed read failed")),
            Ok(true),
        ),
        (
            Ok(true),
            Err(RequestFieldAccessError::new("held read failed")),
        ),
    ] {
        for ignore_flags in [true, false] {
            let order = ReviewOrder::new(flags.0.clone(), flags.1.clone());
            let policy = ReviewFlagPolicy {
                ignore_flags,
                ..ReviewFlagPolicy::default()
            };
            let engine = Engine::builder::<ReviewOrder, (), ()>()
                .with_order_review()
                .no_sync()
                .pre_trade(policy.clone())
                .build()
                .unwrap();
            let result = engine.apply_drop_copy(order.clone());
            assert!(
                result.is_ok(),
                "drop-copy must not reject unreadable review flags: {:?}",
                result.as_ref().err()
            );
            let mut operation = result.unwrap();
            operation.commit();
            assert_eq!(order.reads.get(), (1, 1));
            assert_eq!(
                *policy.seen.borrow(),
                if ignore_flags {
                    vec![]
                } else {
                    vec![flags.clone(); 2]
                }
            );
        }
    }
}

#[test]
fn engine_context_caches_review_flags_once_per_run() {
    for enabled in [false, true] {
        for confirmed in [false, true] {
            for held in [false, true] {
                let order = ReviewOrder::new(Ok(confirmed), Ok(held));
                let policy = ReviewFlagPolicy::default();
                let builder = Engine::builder::<ReviewOrder, (), ()>();
                let builder = if enabled {
                    builder.with_order_review()
                } else {
                    builder
                };
                let engine = builder.no_sync().pre_trade(policy.clone()).build().unwrap();
                let expected = if enabled {
                    (Ok(confirmed), Ok(held))
                } else {
                    (Ok(false), Ok(false))
                };
                let request = engine.start_pre_trade(order.clone()).unwrap();
                *order.confirmed.borrow_mut() = Ok(!confirmed);
                *order.already_held.borrow_mut() = Ok(!held);
                let mut reservation = request.execute().unwrap();
                reservation.commit();
                assert_eq!(*policy.seen.borrow(), vec![expected.clone(); 2]);
                assert_eq!(order.reads.get(), if enabled { (1, 1) } else { (0, 0) });
                *order.confirmed.borrow_mut() = Ok(confirmed);
                *order.already_held.borrow_mut() = Ok(held);
                assert!(engine.start_pre_trade_dry_run(order.clone()).is_pass());
                assert!(engine.execute_pre_trade_dry_run(order.clone()).is_pass());
                assert_eq!(*policy.seen.borrow(), vec![expected; 5]);
                assert_eq!(order.reads.get(), if enabled { (3, 3) } else { (0, 0) });
            }
        }
    }
}

#[test]
fn standalone_context_review_flags_match_reader_selection() {
    for confirmed in [
        Ok(false),
        Ok(true),
        Err(RequestFieldAccessError::new("confirmed")),
    ] {
        for held in [
            Ok(false),
            Ok(true),
            Err(RequestFieldAccessError::new("already_held")),
        ] {
            let order = ReviewOrder::new(confirmed.clone(), held.clone());
            let plain = PreTradeContext::<NoLocking>::new(None, &order);
            assert_eq!(plain.confirmed(), Ok(false));
            assert_eq!(plain.already_held(), Ok(false));
            assert_eq!(order.reads.get(), (0, 0));
            let review = PreTradeContext::<NoLocking>::new_with_order_review(None, &order);
            assert_eq!(review.account_id(), order.account_id());
            for _ in 0..2 {
                assert_eq!(review.confirmed(), confirmed);
                assert_eq!(review.already_held(), held);
            }
            assert_eq!(order.reads.get(), (1, 1));
        }
    }
    struct AccountOnly;
    impl HasAccountId for AccountOnly {
        fn account_id(&self) -> Result<AccountId, RequestFieldAccessError> {
            Ok(AccountId::from_u64(7))
        }
    }
    let context = PreTradeContext::<NoLocking>::new(None, &AccountOnly);
    assert_eq!(context.account_id(), Ok(AccountId::from_u64(7)));
    assert_eq!(context.confirmed(), Ok(false));
    assert_eq!(context.already_held(), Ok(false));
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Signal {
    None,
    Reject,
    WithHold,
    WithoutHold,
    EvaluationFailure,
}

impl Signal {
    fn rejects(self) -> bool {
        matches!(self, Self::Reject | Self::EvaluationFailure)
    }

    fn mode(self) -> RuleMode {
        match self {
            Self::None | Self::Reject => RuleMode::Reject,
            Self::WithHold | Self::EvaluationFailure => RuleMode::ReviewWithHold,
            Self::WithoutHold => RuleMode::ReviewWithoutHold,
        }
    }
}

const SIGNALS: [Signal; 5] = [
    Signal::None,
    Signal::Reject,
    Signal::WithHold,
    Signal::WithoutHold,
    Signal::EvaluationFailure,
];

#[derive(Default)]
struct State {
    events: MutationEvents,
    start_calls: Cell<usize>,
    main_calls: Cell<usize>,
    held: Cell<i32>,
    commits: Cell<usize>,
    rollbacks: Cell<usize>,
}

struct TestPolicy {
    name: &'static str,
    group_id: PolicyGroupId,
    start: Vec<Signal>,
    main: Vec<Signal>,
    execution_blocks: Vec<AccountBlock>,
    scope: RejectScope,
    record_mode: Option<RuleMode>,
    state: Rc<State>,
    read_confirmation: bool,
    fail_rollback: bool,
}

impl TestPolicy {
    fn new(start: Signal, main: Signal, state: &Rc<State>) -> Self {
        Self {
            name: "ReviewTest",
            group_id: PolicyGroupId::new(7),
            start: vec![start],
            main: vec![main],
            execution_blocks: Vec::new(),
            scope: RejectScope::Order,
            record_mode: None,
            state: Rc::clone(state),
            read_confirmation: false,
            fail_rollback: false,
        }
    }

    fn verdict(&self, signals: &[Signal], stage: &str, order: &Order) -> Vec<Reject> {
        if self.read_confirmation && order.confirmed().expect("confirmation is present") {
            return Vec::new();
        }
        signals
            .iter()
            .filter(|signal| **signal != Signal::None)
            .map(|signal| {
                Reject::new(
                    self.name,
                    self.scope.clone(),
                    if *signal == Signal::EvaluationFailure {
                        RejectCode::ArithmeticOverflow
                    } else {
                        RejectCode::Other
                    },
                    stage,
                    "test rule decision",
                )
                .with_mode(self.record_mode.unwrap_or(signal.mode()))
                .with_user_data(42)
            })
            .collect()
    }

    fn main_result(&self, order: &Order) -> Result<Option<PolicyPreTradeResult>, Rejects> {
        let entries = self.verdict(&self.main, "main", order);
        let mut result = PolicyPreTradeResult::default();
        result.lock_prices.push(Price::from_str("13").unwrap());
        result.account_adjustments.push(AccountOutcomeEntry {
            asset: Asset::new("USD").unwrap(),
            balance: None,
            held: Some(OutcomeAmount {
                delta: PositionSize::from_str("5").unwrap(),
                absolute: PositionSize::from_str("5").unwrap(),
            }),
            incoming: None,
            realized_pnl: None,
            average_entry_price: None,
        });
        if entries.is_empty() {
            Ok(Some(result))
        } else {
            Err(Rejects::new(entries).with_policy_result(result))
        }
    }

    fn mutation(&self, eager: bool) -> Mutation {
        if eager {
            self.state.held.set(self.state.held.get() + 1);
        }
        let commit_state = Rc::clone(&self.state);
        let rollback_state = Rc::clone(&self.state);
        let fail = self.fail_rollback;
        let group = self.group_id;
        Mutation::new_fallible_with_error(
            move || {
                commit_state.events.borrow_mut().push((group, "commit"));
                commit_state.commits.set(commit_state.commits.get() + 1);
                Ok(())
            },
            move || {
                rollback_state.events.borrow_mut().push((group, "rollback"));
                rollback_state
                    .rollbacks
                    .set(rollback_state.rollbacks.get() + 1);
                if fail {
                    Err("compensation refused".to_owned())
                } else {
                    rollback_state.held.set(rollback_state.held.get() - 1);
                    Ok(())
                }
            },
        )
    }
}

impl<ExecutionReport> PreTradePolicy<Order, ExecutionReport, (), LocalSync> for TestPolicy {
    fn name(&self) -> &str {
        self.name
    }

    fn policy_group_id(&self) -> PolicyGroupId {
        self.group_id
    }

    fn apply_execution_report(
        &self,
        _ctx: &PostTradeContext<NoLocking>,
        _report: &ExecutionReport,
    ) -> Option<PostTradeResult> {
        Some(PostTradeResult::blocks_only(self.execution_blocks.clone()))
    }

    fn check_pre_trade_start(
        &self,
        _ctx: &PreTradeContext<NoLocking>,
        order: &Order,
    ) -> Result<(), Rejects> {
        self.state.start_calls.set(self.state.start_calls.get() + 1);
        let entries = self.verdict(&self.start, "start", order);
        if entries.is_empty() {
            Ok(())
        } else {
            Err(Rejects::new(entries))
        }
    }

    fn perform_pre_trade_check(
        &self,
        _ctx: &PreTradeContext<NoLocking>,
        order: &Order,
        mutations: &mut Mutations,
    ) -> Result<Option<PolicyPreTradeResult>, Rejects> {
        self.state.events.borrow_mut().push((self.group_id, "hook"));
        self.state.main_calls.set(self.state.main_calls.get() + 1);
        mutations.push(self.mutation(true));
        self.main_result(order)
    }

    fn perform_pre_trade_check_dry_run(
        &self,
        _ctx: &PreTradeContext<NoLocking>,
        order: &Order,
        mutations: &mut Mutations,
    ) -> Result<Option<PolicyPreTradeResult>, Rejects> {
        self.state.events.borrow_mut().push((self.group_id, "hook"));
        self.state.main_calls.set(self.state.main_calls.get() + 1);
        mutations.push(self.mutation(false));
        self.main_result(order)
    }
}

fn order(confirmed: bool) -> Order {
    WithOrderOperation {
        operation: OrderOperation {
            instrument: Instrument::new(Asset::new("AAPL").unwrap(), Asset::new("USD").unwrap()),
            account_id: AccountId::from_u64(1),
            side: Side::Buy,
            trade_amount: TradeAmount::Quantity(Quantity::from_str("1").unwrap()),
            price: Some(Price::from_str("13").unwrap()),
        },
        inner: WithOrderReview {
            inner: (),
            review: OrderReview {
                confirmed,
                already_held: false,
            },
        },
    }
}

fn assert_entries(entries: &[Reject], expected: &[(Signal, &str)]) {
    assert_eq!(entries.len(), expected.len());
    for (entry, (signal, stage)) in entries.iter().zip(expected) {
        assert_eq!(entry.policy, "ReviewTest");
        assert_eq!(entry.reason, *stage);
        assert_eq!(entry.user_data, 42);
        assert_eq!(
            entry.mode,
            if signal.rejects() {
                RuleMode::Reject
            } else {
                signal.mode()
            }
        );
        assert_eq!(
            entry.code,
            if *signal == Signal::EvaluationFailure {
                RejectCode::ArithmeticOverflow
            } else {
                RejectCode::Other
            }
        );
    }
}

fn expected_reviews(start: Signal, main: Signal) -> Vec<(Signal, &'static str)> {
    let mut expected = Vec::new();
    if matches!(start, Signal::WithHold | Signal::WithoutHold) {
        expected.push((start, "start"));
    }
    if start != Signal::WithoutHold && matches!(main, Signal::WithHold | Signal::WithoutHold) {
        expected.push((main, "main"));
    }
    expected
}

const INSTANCE_MODES: [RuleMode; 3] = [
    RuleMode::Reject,
    RuleMode::ReviewWithHold,
    RuleMode::ReviewWithoutHold,
];

#[test]
fn exhaustive_instance_classification_matrix() {
    for instance_mode in INSTANCE_MODES {
        for record_mode in INSTANCE_MODES {
            for evaluation_failure in [false, true] {
                for start_stage in [false, true] {
                    for dry_run in [false, true] {
                        let state = Rc::new(State::default());
                        let signal = if evaluation_failure {
                            Signal::EvaluationFailure
                        } else {
                            Signal::Reject
                        };
                        let mut policy = TestPolicy::new(
                            if start_stage { signal } else { Signal::None },
                            if start_stage { Signal::None } else { signal },
                            &state,
                        );
                        policy.record_mode = Some(record_mode);
                        let expected_mode = if evaluation_failure {
                            RuleMode::Reject
                        } else if record_mode == RuleMode::Reject {
                            instance_mode
                        } else {
                            record_mode
                        };
                        let rejects = evaluation_failure
                            || (instance_mode == RuleMode::Reject
                                && record_mode == RuleMode::Reject);
                        let expected = policy.verdict(
                            &[signal],
                            if start_stage { "start" } else { "main" },
                            &order(false),
                        )[0]
                        .clone()
                        .with_mode(expected_mode);
                        let engine = Engine::builder::<Order, (), ()>()
                            .with_order_review()
                            .no_sync()
                            .pre_trade_instance(
                                policy,
                                PolicyInstance {
                                    name: Some("instance".to_owned()),
                                    mode: instance_mode,
                                },
                            )
                            .build()
                            .unwrap();
                        let (actual_rejects, actual_reviews) = if dry_run {
                            let report = if start_stage {
                                engine.start_pre_trade_dry_run(order(false))
                            } else {
                                engine.execute_pre_trade_dry_run(order(false))
                            };
                            assert_eq!(report.is_pass(), !rejects);
                            (
                                report.rejects().map(|entries| entries.to_vec()),
                                report.reviews().to_vec(),
                            )
                        } else if start_stage {
                            match engine.start_pre_trade(order(false)) {
                                Ok(request) => (None, request.reviews().to_vec()),
                                Err(entries) => (Some(entries.to_vec()), Vec::new()),
                            }
                        } else {
                            match engine.execute_pre_trade(order(false)) {
                                Ok(reservation) => (None, reservation.reviews().to_vec()),
                                Err(entries) => (Some(entries.to_vec()), Vec::new()),
                            }
                        };
                        assert_eq!(
                            (actual_rejects, actual_reviews),
                            if rejects {
                                (Some(vec![expected]), Vec::new())
                            } else {
                                (None, vec![expected])
                            },
                            "instance={instance_mode:?}, record={record_mode:?}, \
                             evaluation_failure={evaluation_failure}, \
                             start_stage={start_stage}, dry_run={dry_run}"
                        );
                        if dry_run {
                            assert_eq!(state.commits.get(), 0);
                            assert_eq!(state.rollbacks.get(), 0);
                        }
                    }
                }
            }
        }
    }
}

fn segment_policy(
    index: u16,
    mode: RuleMode,
    answer: Signal,
    events: &MutationEvents,
) -> (TestPolicy, PolicyInstance, Rc<State>) {
    let state = Rc::new(State {
        events: Rc::clone(events),
        ..State::default()
    });
    let mut policy = TestPolicy::new(Signal::None, answer, &state);
    policy.group_id = PolicyGroupId::new(index);
    (
        policy,
        PolicyInstance {
            name: Some(format!("instance-{index}")),
            mode,
        },
        state,
    )
}

fn segment_cases(rejecting: bool) -> Vec<(RuleMode, Signal)> {
    let mut cases = vec![
        (RuleMode::Reject, Signal::None),
        (RuleMode::Reject, Signal::WithHold),
        (RuleMode::ReviewWithHold, Signal::Reject),
        (RuleMode::ReviewWithoutHold, Signal::WithHold),
        (RuleMode::ReviewWithHold, Signal::None),
    ];
    if rejecting {
        cases.extend([
            (RuleMode::Reject, Signal::Reject),
            (RuleMode::ReviewWithHold, Signal::EvaluationFailure),
            (RuleMode::ReviewWithoutHold, Signal::EvaluationFailure),
        ]);
    }
    cases.push((RuleMode::Reject, Signal::None));
    cases
}

fn assert_segment_results(
    lock: &openpit::pretrade::PreTradeLock,
    outcomes: &[openpit::AccountAdjustmentOutcome],
    count: usize,
    kept: &[usize],
) {
    for index in 0..count {
        assert_eq!(
            lock.prices_of(PolicyGroupId::new(index as u16))
                .collect::<Vec<_>>(),
            if kept.contains(&index) {
                vec![Price::from_str("13").unwrap()]
            } else {
                Vec::new()
            },
            "lock for instance {index}"
        );
    }
    assert_eq!(
        outcomes
            .iter()
            .map(|entry| entry.policy_group_id)
            .collect::<Vec<_>>(),
        kept.iter()
            .map(|index| PolicyGroupId::new(*index as u16))
            .collect::<Vec<_>>()
    );
    for outcome in outcomes {
        assert_eq!(outcome.entry.asset, Asset::new("USD").unwrap());
        assert_eq!(
            outcome.entry.held.as_ref().unwrap().delta,
            PositionSize::from_str("5").unwrap()
        );
    }
}

#[test]
fn main_stage_segments_rollback_before_next_hook_and_keep_valid_results() {
    for rejecting in [false, true] {
        let cases = segment_cases(rejecting);
        let events = MutationEvents::default();
        let (policy, instance, state) = segment_policy(0, cases[0].0, cases[0].1, &events);
        let mut builder = Engine::builder::<Order, (), ()>()
            .with_order_review()
            .no_sync()
            .pre_trade_instance(policy, instance);
        let mut states = vec![state];
        for (index, (mode, answer)) in cases.iter().enumerate().skip(1) {
            let (policy, instance, state) = segment_policy(index as u16, *mode, *answer, &events);
            builder = builder.pre_trade_instance(policy, instance);
            states.push(state);
        }
        let engine = builder.build().unwrap();
        let result = engine.execute_pre_trade(order(false));
        let mut expected_events = Vec::new();
        for index in 0..cases.len() {
            expected_events.push((PolicyGroupId::new(index as u16), "hook"));
            if [2, 3].contains(&index) {
                expected_events.push((PolicyGroupId::new(index as u16), "rollback"));
            }
        }
        let kept = [0, 1, 4, cases.len() - 1];
        if rejecting {
            let rejects = result.err().expect("effective rejects must fail");
            assert_eq!(rejects.len(), 3);
            assert!(rejects.iter().all(|entry| entry.mode == RuleMode::Reject));
            for index in (0..cases.len())
                .rev()
                .filter(|index| ![2, 3].contains(index))
            {
                expected_events.push((PolicyGroupId::new(index as u16), "rollback"));
            }
        } else {
            let mut reservation = result.unwrap();
            assert_eq!(reservation.reviews().len(), 3);
            assert!(reservation
                .reviews()
                .iter()
                .all(|entry| entry.mode == RuleMode::ReviewWithHold));
            assert_segment_results(
                reservation.lock(),
                reservation.account_adjustments(),
                cases.len(),
                &kept,
            );
            assert_eq!(*events.borrow(), expected_events);
            for (index, state) in states.iter().enumerate() {
                assert_eq!(state.held.get(), i32::from(kept.contains(&index)));
            }
            reservation.commit();
            reservation.rollback();
            for index in kept {
                expected_events.push((PolicyGroupId::new(index as u16), "commit"));
            }
        }
        assert_eq!(*events.borrow(), expected_events);
        for state in states {
            assert_eq!(state.commits.get() + state.rollbacks.get(), 1);
        }
    }
}

#[test]
fn dry_run_segments_match_live_results_without_callbacks() {
    for rejecting in [false, true] {
        let cases = segment_cases(rejecting);
        let events = MutationEvents::default();
        let (policy, instance, _) = segment_policy(0, cases[0].0, cases[0].1, &events);
        let mut builder = Engine::builder::<Order, (), ()>()
            .with_order_review()
            .no_sync()
            .pre_trade_instance(policy, instance);
        for (index, (mode, answer)) in cases.iter().enumerate().skip(1) {
            let (policy, instance, _) = segment_policy(index as u16, *mode, *answer, &events);
            builder = builder.pre_trade_instance(policy, instance);
        }
        let engine = builder.build().unwrap();
        let report = engine.execute_pre_trade_dry_run(order(false));
        assert_eq!(report.is_pass(), !rejecting);
        if rejecting {
            assert_eq!(report.rejects().unwrap().len(), 3);
            assert!(report.reviews().is_empty());
        } else {
            assert_eq!(report.reviews().len(), 3);
            assert!(report
                .reviews()
                .iter()
                .all(|entry| entry.mode == RuleMode::ReviewWithHold));
        }
        assert_segment_results(
            report.lock(),
            report.account_adjustments(),
            cases.len(),
            &[0, 1, 4, cases.len() - 1],
        );
        assert_eq!(
            *events.borrow(),
            (0..cases.len())
                .map(|index| (PolicyGroupId::new(index as u16), "hook"))
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn failed_review_segment_rollback_rejects_and_kills_engine_but_runs_later_hooks() {
    let events = MutationEvents::default();
    let (policy, instance, state) = segment_policy(0, RuleMode::Reject, Signal::None, &events);
    let mut states = vec![state];
    let mut builder = Engine::builder::<Order, (), ()>()
        .with_order_review()
        .no_sync()
        .pre_trade_instance(policy, instance);
    let cases = [
        (RuleMode::Reject, Signal::None),
        (RuleMode::ReviewWithHold, Signal::Reject),
        (RuleMode::Reject, Signal::None),
    ];
    for (index, (mode, answer)) in cases.into_iter().enumerate().skip(1) {
        let (mut policy, instance, state) = segment_policy(index as u16, mode, answer, &events);
        policy.fail_rollback = index == 1;
        builder = builder.pre_trade_instance(policy, instance);
        states.push(state);
    }
    let engine = builder.build().unwrap();
    let rejects = engine
        .execute_pre_trade(order(false))
        .err()
        .expect("review rollback must fail");
    assert_eq!(rejects.len(), 1);
    assert_eq!(rejects[0].policy, "Engine");
    assert_eq!(rejects[0].scope, RejectScope::Order);
    assert_eq!(rejects[0].code, RejectCode::SystemUnavailable);
    assert_eq!(rejects[0].mode, RuleMode::Reject);
    assert_eq!(rejects[0].reason, "review rollback failed");
    assert!(rejects[0].details.contains("compensation refused"));
    assert_eq!(
        *events.borrow(),
        vec![
            (PolicyGroupId::new(0), "hook"),
            (PolicyGroupId::new(1), "hook"),
            (PolicyGroupId::new(1), "rollback"),
            (PolicyGroupId::new(2), "hook"),
            (PolicyGroupId::new(2), "rollback"),
            (PolicyGroupId::new(0), "rollback"),
        ]
    );
    for account in [1, 2] {
        let mut next = order(false);
        next.operation.account_id = AccountId::from_u64(account);
        assert_eq!(
            engine.start_pre_trade(next).unwrap_err()[0].code,
            RejectCode::SystemUnavailable
        );
    }
    for state in states {
        assert_eq!(state.start_calls.get(), 1);
    }
}

#[test]
fn drop_copy_review_segments_keep_results_and_commit_unless_evaluation_fails() {
    for fatal in [false, true] {
        let mut cases = vec![
            (RuleMode::ReviewWithHold, Signal::Reject),
            (RuleMode::ReviewWithoutHold, Signal::WithHold),
            (RuleMode::Reject, Signal::Reject),
            (RuleMode::Reject, Signal::WithoutHold),
            (RuleMode::ReviewWithoutHold, Signal::None),
        ];
        if fatal {
            cases.push((RuleMode::ReviewWithoutHold, Signal::EvaluationFailure));
        }
        let events = MutationEvents::default();
        let (policy, instance, _) = segment_policy(0, cases[0].0, cases[0].1, &events);
        let mut builder = Engine::builder::<Order, (), ()>()
            .with_order_review()
            .no_sync()
            .pre_trade_instance(policy, instance);
        for (index, (mode, answer)) in cases.iter().enumerate().skip(1) {
            let (policy, instance, _) = segment_policy(index as u16, *mode, *answer, &events);
            builder = builder.pre_trade_instance(policy, instance);
        }
        let engine = builder.build().unwrap();
        let result = engine.apply_drop_copy(order(false));
        let mut expected = (0..cases.len())
            .map(|index| (PolicyGroupId::new(index as u16), "hook"))
            .collect::<Vec<_>>();
        if fatal {
            let rejects = result.expect_err("evaluation failure must abort drop-copy");
            assert_eq!(rejects.len(), 1);
            assert_eq!(rejects[0].code, RejectCode::ArithmeticOverflow);
            assert_eq!(rejects[0].mode, RuleMode::Reject);
            expected.extend(
                (0..cases.len())
                    .rev()
                    .map(|index| (PolicyGroupId::new(index as u16), "rollback")),
            );
        } else {
            let mut operation = result.unwrap();
            assert_segment_results(
                operation.lock(),
                operation.account_adjustments(),
                cases.len(),
                &(0..cases.len()).collect::<Vec<_>>(),
            );
            assert_eq!(*events.borrow(), expected);
            operation.commit();
            operation.rollback();
            expected
                .extend((0..cases.len()).map(|index| (PolicyGroupId::new(index as u16), "commit")));
        }
        assert_eq!(*events.borrow(), expected);
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 256,
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn instance_segment_finalizers_follow_registration_order(
        cases in proptest::collection::vec(
            (proptest::sample::select(INSTANCE_MODES.to_vec()), proptest::sample::select(SIGNALS.to_vec())),
            1..7,
        ),
        commit in any::<bool>(),
    ) {
        let effective_reject = cases.iter().any(|(mode, answer)| {
            *answer == Signal::EvaluationFailure || (*mode == RuleMode::Reject && *answer == Signal::Reject)
        });
        let without_hold = cases.iter().any(|(mode, answer)| {
            *answer == Signal::WithoutHold || (*mode == RuleMode::ReviewWithoutHold && *answer == Signal::Reject)
        });
        let fatal = cases.iter().any(|(_, answer)| *answer == Signal::EvaluationFailure);
        for run in ["live", "dry-run", "drop-copy"] {
            let events = MutationEvents::default();
            let (policy, instance, state) = segment_policy(0, cases[0].0, cases[0].1, &events);
            let mut states = vec![state];
            let mut builder = Engine::builder::<Order, (), ()>()
                .with_order_review().no_sync().pre_trade_instance(policy, instance);
            let mut expected = Vec::new();
            let mut kept = Vec::new();
            for (index, (mode, answer)) in cases.iter().enumerate() {
                if index > 0 {
                    let (policy, instance, state) = segment_policy(index as u16, *mode, *answer, &events);
                    builder = builder.pre_trade_instance(policy, instance);
                    states.push(state);
                }
                let group = PolicyGroupId::new(index as u16);
                expected.push((group, "hook"));
                if run == "live" && *mode != RuleMode::Reject
                    && !matches!(answer, Signal::None | Signal::EvaluationFailure)
                {
                    expected.push((group, "rollback"));
                } else {
                    kept.push(group);
                }
            }
            let engine = builder.build().unwrap();
            match run {
                "live" => {
                    let result = engine.execute_pre_trade(order(false));
                    prop_assert_eq!(result.is_err(), effective_reject);
                    if let Ok(mut reservation) = result {
                        if commit {
                            reservation.commit();
                        } else {
                            reservation.rollback();
                        }
                        reservation.rollback();
                    }
                    if effective_reject || without_hold || !commit {
                        expected.extend(kept.into_iter().rev().map(|group| (group, "rollback")));
                    } else {
                        expected.extend(kept.into_iter().map(|group| (group, "commit")));
                    }
                }
                "dry-run" => {
                    let report = engine.execute_pre_trade_dry_run(order(false));
                    prop_assert_eq!(report.is_pass(), !effective_reject);
                }
                "drop-copy" => {
                    let result = engine.apply_drop_copy(order(false));
                    prop_assert_eq!(result.is_err(), fatal);
                    if let Ok(mut operation) = result {
                        operation.commit();
                        operation.rollback();
                    }
                    if fatal {
                        expected.extend(kept.into_iter().rev().map(|group| (group, "rollback")));
                    } else {
                        expected.extend(kept.into_iter().map(|group| (group, "commit")));
                    }
                }
                _ => unreachable!(),
            }
            prop_assert_eq!(&*events.borrow(), &expected, "run={}", run);
            for state in states {
                prop_assert!(state.commits.get() + state.rollbacks.get() <= 1);
                if run == "dry-run" {
                    prop_assert_eq!(state.commits.get() + state.rollbacks.get(), 0);
                }
            }
        }
    }
}

#[test]
fn readerless_marks_reject_after_all_hooks_but_drop_copy_keeps_accounting() {
    let expected = vec![Reject::new(
        "Engine",
        RejectScope::Order,
        RejectCode::MissingRequiredField,
        "order review flags unavailable",
        "engine is built without with_order_review",
    )];
    for start_stage in [false, true] {
        for mark in [Signal::WithHold, Signal::WithoutHold] {
            let events = MutationEvents::default();
            let (first, instance, first_state) =
                segment_policy(0, RuleMode::Reject, Signal::None, &events);
            let (mut marking, mark_instance, mark_state) =
                segment_policy(1, RuleMode::Reject, mark, &events);
            marking.scope = RejectScope::Account;
            if start_stage {
                marking.start = vec![mark];
            }
            let (last, last_instance, last_state) =
                segment_policy(2, RuleMode::Reject, Signal::None, &events);
            let engine = Engine::builder::<Order, (), ()>()
                .no_sync()
                .pre_trade_instance(first, instance)
                .pre_trade_instance(marking, mark_instance)
                .pre_trade_instance(last, last_instance)
                .build()
                .unwrap();
            let report = engine.start_pre_trade_dry_run(order(false));
            if start_stage {
                assert!(!report.is_pass(), "readerless start mark must reject");
                assert_eq!(report.rejects().unwrap().to_vec(), expected);
                assert!(report.reviews().is_empty());
            } else {
                assert!(report.is_pass());
            }
            assert!(report.account_block().is_none());
            let report = engine.execute_pre_trade_dry_run(order(false));
            assert!(!report.is_pass(), "readerless mark must reject");
            assert_eq!(report.rejects().unwrap().to_vec(), expected);
            assert!(report.reviews().is_empty());
            assert!(report.account_block().is_none());
            for state in [&first_state, &mark_state, &last_state] {
                assert_eq!(state.held.get(), 0);
                assert_eq!(state.commits.get(), 0);
                assert_eq!(state.rollbacks.get(), 0);
            }
            events.borrow_mut().clear();
            let rejects = if start_stage {
                engine.start_pre_trade(order(false)).unwrap_err()
            } else {
                engine
                    .execute_pre_trade(order(false))
                    .err()
                    .expect("readerless mark must reject")
            };
            assert_eq!(rejects.to_vec(), expected);
            let mut expected_events = Vec::new();
            if !start_stage {
                for index in 0..3 {
                    expected_events.push((PolicyGroupId::new(index), "hook"));
                }
                for index in (0..3).rev() {
                    expected_events.push((PolicyGroupId::new(index), "rollback"));
                }
            }
            assert_eq!(*events.borrow(), expected_events);
            for state in [&first_state, &mark_state, &last_state] {
                assert_eq!(state.start_calls.get(), 3);
                assert_eq!(state.main_calls.get(), if start_stage { 0 } else { 2 });
                assert_eq!(state.held.get(), 0);
                assert_eq!(state.rollbacks.get(), usize::from(!start_stage));
            }
            events.borrow_mut().clear();
            let mut operation = engine.apply_drop_copy(order(false)).unwrap();
            assert_segment_results(
                operation.lock(),
                operation.account_adjustments(),
                3,
                &[0, 1, 2],
            );
            assert!(operation.account_block().is_none());
            operation.commit();
            assert_eq!(
                *events.borrow(),
                vec![
                    (PolicyGroupId::new(0), "hook"),
                    (PolicyGroupId::new(1), "hook"),
                    (PolicyGroupId::new(2), "hook"),
                    (PolicyGroupId::new(0), "commit"),
                    (PolicyGroupId::new(1), "commit"),
                    (PolicyGroupId::new(2), "commit"),
                ]
            );
            for state in [&first_state, &mark_state, &last_state] {
                assert_eq!(state.held.get(), 1);
                assert_eq!(state.commits.get(), 1);
            }
            // The policy's account-scoped mark must not latch an account block.
            assert_eq!(
                engine
                    .execute_pre_trade(order(false))
                    .err()
                    .unwrap()
                    .to_vec(),
                expected
            );
        }
    }
}

#[test]
fn readerless_runs_without_marks_keep_their_verdicts() {
    for signal in [Signal::None, Signal::Reject] {
        for start_stage in [false, true] {
            let state = Rc::new(State::default());
            let (start, main) = if start_stage {
                (signal, Signal::None)
            } else {
                (Signal::None, signal)
            };
            let engine = Engine::builder::<Order, (), ()>()
                .no_sync()
                .pre_trade(TestPolicy::new(start, main, &state))
                .build()
                .unwrap();
            match engine.execute_pre_trade(order(false)) {
                Ok(reservation) => {
                    assert_eq!(signal, Signal::None);
                    assert!(reservation.reviews().is_empty());
                }
                Err(rejects) => {
                    assert_eq!(signal, Signal::Reject, "no mark must not add a reject");
                    assert_entries(
                        &rejects,
                        &[(signal, if start_stage { "start" } else { "main" })],
                    );
                }
            }
        }
    }
}

#[test]
fn reader_keeps_the_same_policy_marks_as_reviews() {
    for start_stage in [false, true] {
        for mark in [Signal::WithHold, Signal::WithoutHold] {
            let state = Rc::new(State::default());
            let start = if start_stage { mark } else { Signal::None };
            let engine = Engine::builder::<Order, (), ()>()
                .with_order_review()
                .no_sync()
                .pre_trade(TestPolicy::new(start, mark, &state))
                .build()
                .unwrap();
            let reservation = engine
                .execute_pre_trade(order(false))
                .unwrap_or_else(|rejects| panic!("reader must preserve marks: {rejects:?}"));
            assert_entries(reservation.reviews(), &expected_reviews(start, mark));
        }
    }
}

#[test]
fn exhaustive_start_main_precedence_matrix() {
    for start in SIGNALS {
        for main in SIGNALS {
            for finalize in ["commit", "rollback", "drop"] {
                let state = Rc::new(State::default());
                let engine = Engine::builder::<Order, (), ()>()
                    .with_order_review()
                    .no_sync()
                    .pre_trade(TestPolicy::new(start, main, &state))
                    .build()
                    .unwrap();
                let request = match engine.start_pre_trade(order(false)) {
                    Err(rejects) => {
                        assert!(start.rejects(), "{start:?} x {main:?}");
                        assert_entries(&rejects, &[(start, "start")]);
                        assert_eq!(state.main_calls.get(), 0);
                        assert_eq!(state.held.get(), 0);
                        continue;
                    }
                    Ok(request) => request,
                };
                assert!(!start.rejects());
                assert_entries(request.reviews(), &expected_reviews(start, Signal::None));
                let result = request.execute();
                let main_ran = start != Signal::WithoutHold;
                assert_eq!(state.main_calls.get(), usize::from(main_ran));
                if main_ran && main.rejects() {
                    let rejects = result.err().expect("main effective reject must fail");
                    assert_entries(&rejects, &[(main, "main")]);
                    assert_eq!(state.held.get(), 0);
                    assert_eq!(state.rollbacks.get(), 1);
                    continue;
                }
                let mut reservation = result.expect("marks must not reject");
                assert_entries(reservation.reviews(), &expected_reviews(start, main));
                let holds = main_ran && main != Signal::WithoutHold;
                assert_eq!(state.held.get(), i32::from(holds));
                assert_eq!(reservation.lock().is_empty(), !holds);
                assert_eq!(reservation.account_adjustments().is_empty(), !holds);
                if holds {
                    assert_eq!(
                        reservation
                            .lock()
                            .prices_of(PolicyGroupId::new(7))
                            .collect::<Vec<_>>(),
                        vec![Price::from_str("13").unwrap()]
                    );
                    let outcome = &reservation.account_adjustments()[0];
                    assert_eq!(outcome.policy_group_id, PolicyGroupId::new(7));
                    assert_eq!(
                        outcome.entry.held.as_ref().unwrap().delta,
                        PositionSize::from_str("5").unwrap()
                    );
                }
                let rollbacks_before = state.rollbacks.get();
                match finalize {
                    "commit" => {
                        reservation.commit();
                        reservation.rollback();
                    }
                    "rollback" => {
                        reservation.rollback();
                        reservation.rollback();
                    }
                    "drop" => {}
                    _ => unreachable!(),
                }
                drop(reservation);
                assert_eq!(
                    state.commits.get(),
                    usize::from(holds && finalize == "commit")
                );
                assert_eq!(
                    state.rollbacks.get(),
                    rollbacks_before + usize::from(holds && finalize != "commit")
                );
                assert_eq!(state.held.get(), i32::from(holds && finalize == "commit"));
            }
        }
    }
}

#[test]
fn exhaustive_dry_run_precedence_matrix() {
    for start in SIGNALS {
        for main in SIGNALS {
            let state = Rc::new(State::default());
            let engine = Engine::builder::<Order, (), ()>()
                .with_order_review()
                .no_sync()
                .pre_trade(TestPolicy::new(start, main, &state))
                .build()
                .unwrap();
            let start_report = engine.start_pre_trade_dry_run(order(false));
            assert_eq!(start_report.is_pass(), !start.rejects());
            if start.rejects() {
                assert_entries(start_report.rejects().unwrap(), &[(start, "start")]);
                assert!(start_report.reviews().is_empty());
            } else {
                assert_entries(
                    start_report.reviews(),
                    &expected_reviews(start, Signal::None),
                );
            }
            assert!(start_report.lock().is_empty());
            assert!(start_report.account_adjustments().is_empty());
            assert!(start_report.account_block().is_none());
            let report = engine.execute_pre_trade_dry_run(order(false));
            let main_ran = !start.rejects() && start != Signal::WithoutHold;
            assert_eq!(state.main_calls.get(), usize::from(main_ran));
            let rejects = start.rejects() || (main_ran && main.rejects());
            assert_eq!(report.is_pass(), !rejects);
            if rejects {
                assert_entries(
                    report.rejects().unwrap(),
                    &[if start.rejects() {
                        (start, "start")
                    } else {
                        (main, "main")
                    }],
                );
                assert!(report.reviews().is_empty());
            } else {
                assert_entries(report.reviews(), &expected_reviews(start, main));
                let holds = main_ran && main != Signal::WithoutHold;
                assert_eq!(report.lock().is_empty(), !holds);
                assert_eq!(report.account_adjustments().is_empty(), !holds);
            }
            assert_eq!(state.held.get(), 0);
            assert_eq!(state.commits.get(), 0);
            assert_eq!(state.rollbacks.get(), 0);
        }
    }
}

#[test]
fn account_scope_marks_never_latch_a_block() {
    for mark in [Signal::WithHold, Signal::WithoutHold] {
        for at_start in [true, false] {
            let state = Rc::new(State::default());
            let mut policy = TestPolicy::new(
                if at_start { mark } else { Signal::None },
                if at_start { Signal::None } else { mark },
                &state,
            );
            policy.scope = RejectScope::Account;
            let engine = Engine::builder::<Order, (), ()>()
                .with_order_review()
                .no_sync()
                .pre_trade(policy)
                .build()
                .unwrap();
            let report = engine.execute_pre_trade_dry_run(order(false));
            assert!(report.is_pass());
            assert_eq!(report.reviews().len(), 1);
            assert!(report.account_block().is_none());
            for _ in 0..2 {
                let mut reservation = engine.execute_pre_trade(order(false)).unwrap();
                assert_eq!(reservation.reviews()[0].scope, RejectScope::Account);
                reservation.rollback();
            }
            assert_eq!(state.held.get(), 0);
        }
    }
}

#[test]
fn execution_report_block_matrix() {
    for mode in INSTANCE_MODES {
        for code in [
            RejectCode::PnlKillSwitchTriggered,
            RejectCode::MarkPriceUnavailable,
        ] {
            let state = Rc::new(State::default());
            let mut policy = TestPolicy::new(Signal::None, Signal::None, &state);
            let block = AccountBlock::new("ReviewTest", code, "report block", "cause");
            policy.execution_blocks.push(block.clone());
            let engine = Engine::builder::<Order, Order, ()>()
                .with_order_review()
                .no_sync()
                .pre_trade_instance(policy, PolicyInstance { name: None, mode })
                .build()
                .unwrap();
            let kept = mode == RuleMode::Reject || code.is_evaluation_failure();
            let result = engine.apply_execution_report(&order(false));
            assert_eq!(
                result.account_blocks,
                if kept { vec![block] } else { vec![] },
                "{mode:?}, {code:?}"
            );
            let next = engine.start_pre_trade(order(false));
            assert_eq!(next.is_err(), kept, "recorded block: {mode:?}, {code:?}");
            if kept {
                let rejects = next.err().unwrap();
                assert_eq!(rejects[0].code, code);
                assert_eq!(rejects[0].reason, "report block");
                assert_eq!(state.start_calls.get(), 0);
            }
        }
    }
}

#[test]
fn execution_report_mixed_blocks_keep_the_first_effective_cause() {
    let state = Rc::new(State::default());
    let mut policy = TestPolicy::new(Signal::None, Signal::None, &state);
    let ordinary = AccountBlock::new(
        "ReviewTest",
        RejectCode::PnlKillSwitchTriggered,
        "ordinary",
        "dropped",
    );
    let failure = AccountBlock::new(
        "ReviewTest",
        RejectCode::MarkPriceUnavailable,
        "failure",
        "kept",
    );
    policy.execution_blocks = vec![ordinary, failure.clone()];
    let engine = Engine::builder::<Order, Order, ()>()
        .with_order_review()
        .no_sync()
        .pre_trade_instance(
            policy,
            PolicyInstance {
                name: None,
                mode: RuleMode::ReviewWithHold,
            },
        )
        .build()
        .unwrap();
    assert_eq!(
        engine.apply_execution_report(&order(false)).account_blocks,
        vec![failure.clone()]
    );
    let rejects = engine.start_pre_trade(order(false)).err().unwrap();
    assert_eq!(rejects[0], Reject::from(failure));
    assert_eq!(state.start_calls.get(), 0);
}

fn barrier(lower: &str) -> SpotFundsPnlBoundsBarrier {
    SpotFundsPnlBoundsBarrier {
        currency: Asset::new("USD").unwrap(),
        lower_bound: Some(Pnl::from_str(lower).unwrap()),
        upper_bound: None,
    }
}

fn spot_funds_engine(
    mode: RuleMode,
    settings: SpotFundsSettings,
) -> LocalEngine<
    Order,
    WithExecutionReportFillDetails<ExecutionReportOperation>,
    AccountAdjustmentAccountPnlOperation,
> {
    let builder = Engine::builder::<
        Order,
        WithExecutionReportFillDetails<ExecutionReportOperation>,
        AccountAdjustmentAccountPnlOperation,
    >()
    .with_order_review()
    .no_sync();
    let policy = SpotFundsPolicy::<LocalSync, LocalSync>::new(
        settings,
        None::<SpotFundsMarketData<LocalSync>>,
        builder.storage_builder(),
    );
    let engine = builder
        .pre_trade_instance(
            policy,
            PolicyInstance {
                name: Some("funds".to_owned()),
                mode,
            },
        )
        .build()
        .unwrap();
    engine
        .accounts()
        .set_currency(AccountId::from_u64(1), Asset::new("USD").unwrap());
    engine
}

#[test]
fn spot_funds_force_set_filters_blocks() {
    for mode in [RuleMode::Reject, RuleMode::ReviewWithHold] {
        let mut settings = SpotFundsSettings::new(0, SpotFundsPricingSource::Mark, []).unwrap();
        settings.set_global_limit_mode(SpotFundsLimitMode::TrackOnly);
        settings
            .set_pnl_global_barrier(Some(barrier("-10")))
            .unwrap();
        let engine = spot_funds_engine(mode, settings);
        let result = engine
            .configure()
            .set_spot_funds_account_pnl(
                "funds",
                AccountId::from_u64(1),
                PnlState::Value(Pnl::from_str("-20").unwrap()),
            )
            .unwrap();
        let kept = mode == RuleMode::Reject;
        assert_eq!(
            result.account_blocks.len(),
            usize::from(kept),
            "force-set: {mode:?}"
        );
        if kept {
            assert_eq!(
                result.account_blocks[0].code,
                RejectCode::PnlKillSwitchTriggered
            );
        }
        engine
            .configure()
            .spot_funds("funds", |settings| settings.set_pnl_global_barrier(None))
            .unwrap();
        assert_eq!(
            engine.start_pre_trade(order(false)).is_err(),
            kept,
            "force-set recorded block: {mode:?}"
        );
    }
}

#[test]
fn spot_funds_retune_filters_realized_and_axis_blocks() {
    for mode in [RuleMode::Reject, RuleMode::ReviewWithoutHold] {
        for axis in [None, Some(SpotFundsPnlBoundsAxis::AccountTotal)] {
            let mut settings = SpotFundsSettings::new(0, SpotFundsPricingSource::Mark, []).unwrap();
            settings.set_global_limit_mode(SpotFundsLimitMode::TrackOnly);
            match axis {
                None => settings
                    .set_pnl_global_barrier(Some(barrier("-50")))
                    .unwrap(),
                Some(axis) => settings
                    .set_pnl_axis_global_barrier(axis, Some(barrier("-50")))
                    .unwrap(),
            }
            let engine = spot_funds_engine(mode, settings);
            assert!(engine
                .configure()
                .set_spot_funds_account_pnl(
                    "funds",
                    AccountId::from_u64(1),
                    PnlState::Value(Pnl::from_str("-40").unwrap())
                )
                .unwrap()
                .account_blocks
                .is_empty());
            let result = engine
                .configure()
                .spot_funds("funds", |settings| match axis {
                    None => settings.set_pnl_global_barrier(Some(barrier("-30"))),
                    Some(axis) => settings.set_pnl_axis_global_barrier(axis, Some(barrier("-30"))),
                })
                .unwrap();
            let kept = mode == RuleMode::Reject;
            assert_eq!(
                result.account_blocks.len(),
                usize::from(kept),
                "retune: {mode:?}, {axis:?}"
            );
            if kept {
                assert_eq!(result.account_blocks[0].account_id, AccountId::from_u64(1));
                assert_eq!(
                    result.account_blocks[0].block.code,
                    RejectCode::PnlKillSwitchTriggered
                );
            }
            engine
                .configure()
                .spot_funds("funds", |settings| match axis {
                    None => settings.set_pnl_global_barrier(None),
                    Some(axis) => settings.set_pnl_axis_global_barrier(axis, None),
                })
                .unwrap();
            assert_eq!(
                engine.start_pre_trade(order(false)).is_err(),
                kept,
                "retune recorded block: {mode:?}, {axis:?}"
            );
        }
    }
}

#[test]
fn spot_funds_membership_filters_blocks() {
    for mode in [RuleMode::Reject, RuleMode::ReviewWithHold] {
        let mut settings = SpotFundsSettings::new(0, SpotFundsPricingSource::Mark, []).unwrap();
        settings.set_global_limit_mode(SpotFundsLimitMode::TrackOnly);
        let group = AccountGroupId::from_u32(94).unwrap();
        settings
            .set_pnl_account_group_barriers([SpotFundsPnlBoundsAccountGroupBarrier {
                account_group_id: group,
                barrier: barrier("-10"),
            }])
            .unwrap();
        let engine = spot_funds_engine(mode, settings);
        assert!(engine
            .configure()
            .set_spot_funds_account_pnl(
                "funds",
                AccountId::from_u64(1),
                PnlState::Value(Pnl::from_str("-20").unwrap())
            )
            .unwrap()
            .account_blocks
            .is_empty());
        engine
            .accounts()
            .register_group(&[AccountId::from_u64(1)], group)
            .unwrap();
        engine
            .configure()
            .spot_funds("funds", |settings| {
                settings.set_pnl_account_group_barriers([])
            })
            .unwrap();
        assert_eq!(
            engine.start_pre_trade(order(false)).is_err(),
            mode == RuleMode::Reject,
            "membership recorded block: {mode:?}"
        );
    }
}

#[test]
fn spot_funds_membership_keeps_the_first_registered_block() {
    for iteration in 0..32 {
        let account = AccountId::from_u64(1);
        let group = AccountGroupId::from_u32(94).unwrap();
        let builder = Engine::builder::<
            Order,
            WithExecutionReportFillDetails<ExecutionReportOperation>,
            AccountAdjustmentAccountPnlOperation,
        >()
        .no_sync();
        let [first, second] = ["-10", "-15"].map(|lower| {
            let mut settings = SpotFundsSettings::new(0, SpotFundsPricingSource::Mark, []).unwrap();
            settings.set_global_limit_mode(SpotFundsLimitMode::TrackOnly);
            settings
                .set_pnl_account_group_barriers([SpotFundsPnlBoundsAccountGroupBarrier {
                    account_group_id: group,
                    barrier: barrier(lower),
                }])
                .unwrap();
            SpotFundsPolicy::<LocalSync, LocalSync>::new(
                settings,
                None::<SpotFundsMarketData<LocalSync>>,
                builder.storage_builder(),
            )
        });
        let engine = builder
            .pre_trade_instance(
                first,
                PolicyInstance {
                    name: Some("first".to_owned()),
                    mode: RuleMode::Reject,
                },
            )
            .pre_trade_instance(
                second,
                PolicyInstance {
                    name: Some("second".to_owned()),
                    mode: RuleMode::Reject,
                },
            )
            .build()
            .unwrap();
        engine
            .accounts()
            .set_currency(account, Asset::new("USD").unwrap());
        for name in ["first", "second"] {
            assert!(engine
                .configure()
                .set_spot_funds_account_pnl(
                    name,
                    account,
                    PnlState::Value(Pnl::from_str("-20").unwrap()),
                )
                .unwrap()
                .account_blocks
                .is_empty());
        }
        engine.accounts().register_group(&[account], group).unwrap();
        for name in ["first", "second"] {
            engine
                .configure()
                .spot_funds(name, |settings| settings.set_pnl_account_group_barriers([]))
                .unwrap();
        }
        let rejects = engine.start_pre_trade(order(false)).err().unwrap();
        assert_eq!(rejects[0].code, RejectCode::PnlKillSwitchTriggered);
        assert!(
            rejects[0]
                .details
                .contains(&format!(", lower bound {},", Pnl::from_str("-10").unwrap())),
            "registration order: iteration {iteration}, stored cause: {}",
            rejects[0].details
        );
    }
}

#[test]
fn dry_run_account_blocks_only_come_from_effective_rejects() {
    for mode in INSTANCE_MODES {
        for signal in [Signal::Reject, Signal::EvaluationFailure] {
            for at_start in [true, false] {
                let state = Rc::new(State::default());
                let mut policy = TestPolicy::new(
                    if at_start { signal } else { Signal::None },
                    if at_start { Signal::None } else { signal },
                    &state,
                );
                policy.scope = RejectScope::Account;
                let engine = Engine::builder::<Order, (), ()>()
                    .with_order_review()
                    .no_sync()
                    .pre_trade_instance(policy, PolicyInstance { name: None, mode })
                    .build()
                    .unwrap();
                let report = engine.execute_pre_trade_dry_run(order(false));
                let blocked = mode == RuleMode::Reject || signal == Signal::EvaluationFailure;
                assert_eq!(
                    report.is_pass(),
                    !blocked,
                    "dry-run: {mode:?}, {signal:?}, start={at_start}"
                );
                assert_eq!(report.reviews().len(), usize::from(!blocked));
                assert_eq!(
                    report.account_block().is_some(),
                    blocked,
                    "would-be block: {mode:?}, {signal:?}, start={at_start}"
                );
                assert_eq!(state.commits.get(), 0);
                assert_eq!(state.rollbacks.get(), 0);
            }
        }
    }
}

#[test]
fn confirmed_order_skips_the_review_rule() {
    for mark in [Signal::WithHold, Signal::WithoutHold] {
        for at_start in [true, false] {
            let state = Rc::new(State::default());
            let mut policy = TestPolicy::new(
                if at_start { mark } else { Signal::None },
                if at_start { Signal::None } else { mark },
                &state,
            );
            policy.read_confirmation = true;
            let engine = Engine::builder::<Order, (), ()>()
                .with_order_review()
                .no_sync()
                .pre_trade(policy)
                .build()
                .unwrap();
            let pending = engine.execute_pre_trade(order(false)).unwrap();
            assert_eq!(pending.reviews().len(), 1);
            drop(pending);
            let confirmed = engine.execute_pre_trade(order(true)).unwrap();
            assert!(confirmed.reviews().is_empty());
            assert!(!confirmed.lock().is_empty());
        }
    }
}

#[test]
fn a_block_latched_between_stages_beats_start_without_hold() {
    let state = Rc::new(State::default());
    let engine = Engine::builder::<Order, (), ()>()
        .with_order_review()
        .no_sync()
        .pre_trade(TestPolicy::new(Signal::WithoutHold, Signal::None, &state))
        .build()
        .unwrap();
    let request = engine.start_pre_trade(order(false)).unwrap();
    engine
        .accounts()
        .block(AccountId::from_u64(1), "operator block".to_owned());
    let rejects = request.execute().err().expect("block must win");
    assert_eq!(rejects.len(), 1);
    assert_eq!(rejects[0].code, RejectCode::AccountBlocked);
    assert_eq!(rejects[0].mode, RuleMode::Reject);
    assert_eq!(state.main_calls.get(), 0);
}

#[test]
fn failed_without_hold_compensation_returns_engine_reject_and_kills_engine() {
    let state = Rc::new(State::default());
    let mut policy = TestPolicy::new(Signal::WithHold, Signal::WithoutHold, &state);
    policy.fail_rollback = true;
    let engine = Engine::builder::<Order, (), ()>()
        .with_order_review()
        .no_sync()
        .pre_trade(policy)
        .build()
        .unwrap();
    let rejects = engine
        .execute_pre_trade(order(false))
        .err()
        .expect("rollback must fail");
    assert_eq!(rejects.len(), 1);
    let reject = &rejects[0];
    assert_eq!(reject.policy, "Engine");
    assert_eq!(reject.scope, RejectScope::Order);
    assert_eq!(reject.code, RejectCode::SystemUnavailable);
    assert_eq!(reject.mode, RuleMode::Reject);
    assert_eq!(reject.reason, "review rollback failed");
    assert!(reject.details.contains("compensation refused"));
    assert_eq!(state.rollbacks.get(), 1);
    for account in [1, 2] {
        let mut next = order(true);
        next.operation.account_id = AccountId::from_u64(account);
        let rejects = engine.start_pre_trade(next).unwrap_err();
        assert_eq!(rejects[0].code, RejectCode::SystemUnavailable);
        assert_eq!(rejects[0].mode, RuleMode::Reject);
    }
    assert_eq!(state.start_calls.get(), 1);
}

#[test]
fn drop_copy_keeps_bookkeeping_for_account_marks_and_discards_reviews() {
    for mark in [Signal::WithHold, Signal::WithoutHold] {
        let state = Rc::new(State::default());
        let mut policy = TestPolicy::new(mark, mark, &state);
        policy.scope = RejectScope::Account;
        let engine = Engine::builder::<Order, (), ()>()
            .with_order_review()
            .no_sync()
            .pre_trade(policy)
            .build()
            .unwrap();
        let mut operation = engine.apply_drop_copy(order(false)).unwrap();
        assert_eq!(state.main_calls.get(), 1);
        assert_eq!(state.held.get(), 1);
        assert!(!operation.lock().is_empty());
        assert_eq!(operation.account_adjustments().len(), 1);
        assert!(operation.account_block().is_none());
        operation.commit();
        assert_eq!(state.commits.get(), 1);
        assert!(engine.start_pre_trade(order(false)).is_ok());
    }
}

#[test]
fn drop_copy_evaluation_failure_marks_are_fatal_and_normalized() {
    for at_start in [true, false] {
        let state = Rc::new(State::default());
        let policy = TestPolicy::new(
            if at_start {
                Signal::EvaluationFailure
            } else {
                Signal::WithoutHold
            },
            if at_start {
                Signal::None
            } else {
                Signal::EvaluationFailure
            },
            &state,
        );
        let engine = Engine::builder::<Order, (), ()>()
            .no_sync()
            .pre_trade(policy)
            .build()
            .unwrap();
        let rejects = engine
            .apply_drop_copy(order(false))
            .expect_err("fatal mark");
        assert_entries(
            &rejects,
            &[(
                Signal::EvaluationFailure,
                if at_start { "start" } else { "main" },
            )],
        );
        assert_eq!(state.main_calls.get(), usize::from(!at_start));
        assert_eq!(state.held.get(), 0);
    }
}

#[test]
fn mixed_policies_report_rejects_only_and_run_later_policies() {
    for at_start in [true, false] {
        let state = Rc::new(State::default());
        let mut mark = TestPolicy::new(Signal::WithHold, Signal::WithoutHold, &state);
        mark.name = "Mark";
        mark.group_id = PolicyGroupId::new(8);
        let reject = TestPolicy::new(
            if at_start {
                Signal::Reject
            } else {
                Signal::None
            },
            Signal::Reject,
            &state,
        );
        let mut last = TestPolicy::new(Signal::None, Signal::WithHold, &state);
        last.name = "Last";
        last.group_id = PolicyGroupId::new(9);
        let engine = Engine::builder::<Order, (), ()>()
            .with_order_review()
            .no_sync()
            .pre_trade(mark)
            .pre_trade(reject)
            .pre_trade(last)
            .build()
            .unwrap();
        let rejects = engine
            .execute_pre_trade(order(false))
            .err()
            .expect("reject beats marks");
        assert_entries(
            &rejects,
            &[(Signal::Reject, if at_start { "start" } else { "main" })],
        );
        assert_eq!(state.start_calls.get(), 3);
        assert_eq!(state.main_calls.get(), if at_start { 0 } else { 3 });
        assert_eq!(state.rollbacks.get(), if at_start { 0 } else { 3 });
        assert_eq!(state.held.get(), 0);
    }
}

#[test]
fn without_hold_wins_over_with_hold_within_a_stage() {
    for at_start in [true, false] {
        let state = Rc::new(State::default());
        let mut policy = TestPolicy::new(Signal::None, Signal::None, &state);
        if at_start {
            policy.start = vec![Signal::WithHold, Signal::WithoutHold];
        } else {
            policy.main = vec![Signal::WithHold, Signal::WithoutHold];
        }
        let engine = Engine::builder::<Order, (), ()>()
            .with_order_review()
            .no_sync()
            .pre_trade(policy)
            .build()
            .unwrap();
        let mut reservation = engine.execute_pre_trade(order(false)).unwrap();
        assert_entries(
            reservation.reviews(),
            &[
                (Signal::WithHold, if at_start { "start" } else { "main" }),
                (Signal::WithoutHold, if at_start { "start" } else { "main" }),
            ],
        );
        assert!(reservation.lock().is_empty());
        assert!(reservation.account_adjustments().is_empty());
        assert_eq!(state.held.get(), 0);
        reservation.commit();
        assert_eq!(state.commits.get(), 0);
    }
}

#[test]
fn effective_reject_discards_marks_from_the_same_policy() {
    for at_start in [true, false] {
        let state = Rc::new(State::default());
        let mut policy = TestPolicy::new(Signal::None, Signal::None, &state);
        let entries = vec![Signal::WithHold, Signal::Reject, Signal::WithoutHold];
        if at_start {
            policy.start = entries;
        } else {
            policy.main = entries;
        }
        let engine = Engine::builder::<Order, (), ()>()
            .no_sync()
            .pre_trade(policy)
            .build()
            .unwrap();
        let stage = if at_start { "start" } else { "main" };
        if at_start {
            let report = engine.start_pre_trade_dry_run(order(false));
            assert_entries(report.rejects().unwrap(), &[(Signal::Reject, "start")]);
            assert!(report.reviews().is_empty());
        }
        let report = engine.execute_pre_trade_dry_run(order(false));
        assert_entries(report.rejects().unwrap(), &[(Signal::Reject, stage)]);
        assert!(report.reviews().is_empty());
        let rejects = engine
            .execute_pre_trade(order(false))
            .err()
            .expect("reject must win");
        assert_entries(&rejects, &[(Signal::Reject, stage)]);
        assert_eq!(state.held.get(), 0);
        assert_eq!(state.rollbacks.get(), usize::from(!at_start));
    }
}

fn broker_rate_limit(max_orders: usize) -> RateLimitSettings {
    RateLimitSettings::new(
        Some(RateLimitBrokerBarrier {
            limit: RateLimit {
                max_orders,
                window: Duration::from_secs(3600),
            },
        }),
        [],
        [],
        [],
    )
    .unwrap()
}

#[test]
fn blocked_registry_rejection_does_not_spend_rate_limit_budget() {
    let state = Rc::new(State::default());
    let builder = Engine::builder::<Order, (), ()>().no_sync();
    let settings = broker_rate_limit(1);
    let limit = RateLimitPolicy::new(settings, builder.storage_builder());
    let engine = builder
        .pre_trade(TestPolicy::new(Signal::None, Signal::None, &state))
        .pre_trade(limit)
        .build()
        .unwrap();
    let account = AccountId::from_u64(1);
    engine
        .accounts()
        .block(account, "operator block".to_owned());
    let rejects = engine.start_pre_trade(order(false)).unwrap_err();
    assert_eq!(rejects[0].code, RejectCode::AccountBlocked);
    assert_eq!(state.start_calls.get(), 0);

    engine.accounts().unblock(account);
    let request = engine
        .start_pre_trade(order(false))
        .expect("registry rejection must leave the first rate-limit unit available");
    assert_eq!(state.start_calls.get(), 1);
    drop(request);
    let rejects = engine.start_pre_trade(order(false)).unwrap_err();
    assert_eq!(rejects[0].code, RejectCode::RateLimitExceeded);
}

#[test]
fn execute_does_not_spend_a_second_rate_limit_unit() {
    let state = Rc::new(State::default());
    let builder = Engine::builder::<Order, (), ()>().no_sync();
    let settings = broker_rate_limit(2);
    let limit = RateLimitPolicy::new(settings, builder.storage_builder());
    let engine = builder
        .pre_trade(TestPolicy::new(Signal::None, Signal::None, &state))
        .pre_trade(limit)
        .build()
        .unwrap();

    let request = engine.start_pre_trade(order(false)).unwrap();
    assert_eq!(state.main_calls.get(), 0);
    let mut reservation = request.execute().unwrap();
    assert_eq!(state.main_calls.get(), 1);
    reservation.commit();
    assert_eq!(state.commits.get(), 1);

    let second = engine
        .start_pre_trade(order(false))
        .expect("execute must leave the second rate-limit unit available");
    drop(second);
    let rejects = engine.start_pre_trade(order(false)).unwrap_err();
    assert_eq!(rejects.len(), 1);
    assert_eq!(rejects[0].code, RejectCode::RateLimitExceeded);
}

#[test]
fn review_and_confirmed_attempts_each_spend_rate_limit_budget() {
    for max_orders in [1, 2] {
        let state = Rc::new(State::default());
        let mut review = TestPolicy::new(Signal::WithoutHold, Signal::None, &state);
        review.read_confirmation = true;
        let builder = Engine::builder::<Order, (), ()>()
            .with_order_review()
            .no_sync();
        let settings = broker_rate_limit(max_orders);
        let limit = RateLimitPolicy::new(settings, builder.storage_builder());
        let engine = builder.pre_trade(review).pre_trade(limit).build().unwrap();
        for _ in 0..2 {
            assert!(engine.execute_pre_trade_dry_run(order(false)).is_pass());
        }
        let pending = engine.start_pre_trade(order(false)).unwrap();
        assert_eq!(pending.reviews().len(), 1);
        let mut pending = pending.execute().unwrap();
        pending.commit();
        let confirmed = engine.execute_pre_trade(order(true));
        let rejects = if max_orders == 1 {
            confirmed
                .err()
                .expect("the pending attempt already spent the budget")
        } else {
            let mut confirmed = confirmed.unwrap();
            assert!(confirmed.reviews().is_empty());
            confirmed.commit();
            engine.start_pre_trade(order(true)).unwrap_err()
        };
        assert_eq!(rejects.len(), 1);
        assert_eq!(rejects[0].code, RejectCode::RateLimitExceeded);
        assert_eq!(rejects[0].mode, RuleMode::Reject);
    }
}

#[test]
fn confirmed_instance_classification_matrix() {
    for instance_mode in INSTANCE_MODES {
        for record_mode in INSTANCE_MODES {
            for evaluation_failure in [false, true] {
                for start_stage in [false, true] {
                    for dry_run in [false, true] {
                        let state = Rc::new(State::default());
                        let signal = if evaluation_failure {
                            Signal::EvaluationFailure
                        } else {
                            Signal::Reject
                        };
                        let mut policy = TestPolicy::new(
                            if start_stage { signal } else { Signal::None },
                            if start_stage { Signal::None } else { signal },
                            &state,
                        );
                        policy.record_mode = Some(record_mode);
                        let rejects = evaluation_failure
                            || (instance_mode == RuleMode::Reject
                                && record_mode == RuleMode::Reject);
                        let expected = policy.verdict(
                            &[signal],
                            if start_stage { "start" } else { "main" },
                            &order(true),
                        )[0]
                        .clone()
                        .with_mode(RuleMode::Reject);
                        let engine = Engine::builder::<Order, (), ()>()
                            .with_order_review()
                            .no_sync()
                            .pre_trade_instance(
                                policy,
                                PolicyInstance {
                                    name: None,
                                    mode: instance_mode,
                                },
                            )
                            .build()
                            .unwrap();
                        let (actual_rejects, reviews) = if dry_run {
                            let report = if start_stage {
                                engine.start_pre_trade_dry_run(order(true))
                            } else {
                                engine.execute_pre_trade_dry_run(order(true))
                            };
                            assert_eq!(report.is_pass(), !rejects);
                            (
                                report.rejects().map(|entries| entries.to_vec()),
                                report.reviews().to_vec(),
                            )
                        } else if start_stage {
                            match engine.start_pre_trade(order(true)) {
                                Ok(request) => (None, request.reviews().to_vec()),
                                Err(entries) => (Some(entries.to_vec()), Vec::new()),
                            }
                        } else {
                            match engine.execute_pre_trade(order(true)) {
                                Ok(reservation) => (None, reservation.reviews().to_vec()),
                                Err(entries) => (Some(entries.to_vec()), Vec::new()),
                            }
                        };
                        assert_eq!(
                            (actual_rejects, reviews),
                            (
                                if rejects { Some(vec![expected]) } else { None },
                                Vec::new()
                            ),
                            "instance={instance_mode:?}, record={record_mode:?}, \
                             evaluation_failure={evaluation_failure}, \
                             start_stage={start_stage}, dry_run={dry_run}"
                        );
                        assert_eq!(state.start_calls.get(), 1);
                        assert_eq!(state.main_calls.get(), usize::from(!start_stage));
                        if dry_run {
                            assert_eq!(state.commits.get(), 0);
                            assert_eq!(state.rollbacks.get(), 0);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn confirmed_segments_preserve_reject_results_and_discard_review_results() {
    let cases = segment_cases(false);
    let events = MutationEvents::default();
    let (policy, instance, state) = segment_policy(0, cases[0].0, cases[0].1, &events);
    let mut builder = Engine::builder::<Order, (), ()>()
        .with_order_review()
        .no_sync()
        .pre_trade_instance(policy, instance);
    let mut states = vec![state];
    for (index, (mode, signal)) in cases.iter().enumerate().skip(1) {
        let (policy, instance, state) = segment_policy(index as u16, *mode, *signal, &events);
        builder = builder.pre_trade_instance(policy, instance);
        states.push(state);
    }
    let engine = builder.build().unwrap();
    let request = engine.start_pre_trade(order(true)).unwrap();
    assert!(request.reviews().is_empty());
    let mut reservation = request.execute().unwrap();
    assert!(reservation.reviews().is_empty());
    let kept = [0, 1, 4, cases.len() - 1];
    assert_segment_results(
        reservation.lock(),
        reservation.account_adjustments(),
        cases.len(),
        &kept,
    );
    let mut expected = Vec::new();
    for (index, state) in states.iter().enumerate() {
        expected.push((PolicyGroupId::new(index as u16), "hook"));
        if [2, 3].contains(&index) {
            expected.push((PolicyGroupId::new(index as u16), "rollback"));
        }
        assert_eq!(state.start_calls.get(), 1);
        assert_eq!(state.main_calls.get(), 1);
        assert_eq!(state.held.get(), i32::from(kept.contains(&index)));
    }
    assert_eq!(*events.borrow(), expected);
    reservation.commit();
    for index in kept {
        expected.push((PolicyGroupId::new(index as u16), "commit"));
    }
    assert_eq!(*events.borrow(), expected);
    for (index, state) in states.iter().enumerate() {
        assert_eq!(state.commits.get(), usize::from(kept.contains(&index)));
        assert_eq!(state.rollbacks.get(), usize::from(!kept.contains(&index)));
    }
}

#[test]
fn confirmed_start_without_hold_runs_main_and_keeps_reservation() {
    for confirmed in [false, true] {
        let state = Rc::new(State::default());
        let engine = Engine::builder::<Order, (), ()>()
            .with_order_review()
            .no_sync()
            .pre_trade(TestPolicy::new(Signal::WithoutHold, Signal::None, &state))
            .build()
            .unwrap();
        let request = engine.start_pre_trade(order(confirmed)).unwrap();
        assert_eq!(request.reviews().len(), usize::from(!confirmed));
        let mut reservation = request.execute().unwrap();
        assert_eq!(reservation.reviews().len(), usize::from(!confirmed));
        assert_eq!(state.start_calls.get(), 1);
        assert_eq!(state.main_calls.get(), usize::from(confirmed));
        assert_eq!(state.held.get(), i32::from(confirmed));
        assert_eq!(reservation.lock().is_empty(), !confirmed);
        assert_eq!(
            reservation.account_adjustments().len(),
            usize::from(confirmed)
        );
        reservation.commit();
        assert_eq!(state.commits.get(), usize::from(confirmed));
        assert_eq!(state.rollbacks.get(), 0);
        let report = engine.execute_pre_trade_dry_run(order(confirmed));
        assert!(report.is_pass());
        assert_eq!(report.reviews().len(), usize::from(!confirmed));
        assert_eq!(report.lock().is_empty(), !confirmed);
        assert_eq!(state.main_calls.get(), 2 * usize::from(confirmed));
        assert_eq!(state.commits.get(), usize::from(confirmed));
        assert_eq!(state.rollbacks.get(), 0);
    }
}

#[test]
fn confirmed_main_without_hold_keeps_reservation() {
    for confirmed in [false, true] {
        for deferred in [false, true] {
            let state = Rc::new(State::default());
            let engine = Engine::builder::<Order, (), ()>()
                .with_order_review()
                .no_sync()
                .pre_trade(TestPolicy::new(Signal::None, Signal::WithoutHold, &state))
                .build()
                .unwrap();
            let mut reservation = if deferred {
                let request = engine.start_pre_trade(order(confirmed)).unwrap();
                assert!(request.reviews().is_empty());
                request.execute().unwrap()
            } else {
                engine.execute_pre_trade(order(confirmed)).unwrap()
            };
            assert_eq!(reservation.reviews().len(), usize::from(!confirmed));
            assert_eq!(reservation.lock().is_empty(), !confirmed);
            assert_eq!(
                reservation.account_adjustments().len(),
                usize::from(confirmed)
            );
            assert_eq!(state.start_calls.get(), 1);
            assert_eq!(state.main_calls.get(), 1);
            assert_eq!(state.held.get(), i32::from(confirmed));
            assert_eq!(state.commits.get(), 0);
            assert_eq!(state.rollbacks.get(), usize::from(!confirmed));
            reservation.commit();
            assert_eq!(state.held.get(), i32::from(confirmed));
            assert_eq!(state.commits.get(), usize::from(confirmed));
            assert_eq!(state.rollbacks.get(), usize::from(!confirmed));
        }

        let state = Rc::new(State::default());
        let engine = Engine::builder::<Order, (), ()>()
            .with_order_review()
            .no_sync()
            .pre_trade(TestPolicy::new(Signal::None, Signal::WithoutHold, &state))
            .build()
            .unwrap();
        let report = engine.execute_pre_trade_dry_run(order(confirmed));
        assert!(report.is_pass());
        assert_eq!(report.reviews().len(), usize::from(!confirmed));
        assert_eq!(report.lock().is_empty(), !confirmed);
        assert_eq!(report.account_adjustments().len(), usize::from(confirmed));
        assert_eq!(state.start_calls.get(), 1);
        assert_eq!(state.main_calls.get(), 1);
        assert_eq!(state.held.get(), 0);
        assert_eq!(state.commits.get(), 0);
        assert_eq!(state.rollbacks.get(), 0);
    }
}

#[test]
fn confirmed_review_rate_limit_attempt_is_counted() {
    let builder = Engine::builder::<Order, (), ()>()
        .with_order_review()
        .no_sync();
    let policy = RateLimitPolicy::new(broker_rate_limit(1), builder.storage_builder());
    let engine = builder
        .pre_trade_instance(
            policy,
            PolicyInstance {
                name: None,
                mode: RuleMode::ReviewWithHold,
            },
        )
        .build()
        .unwrap();
    let mut first = engine.execute_pre_trade(order(true)).unwrap();
    assert!(first.reviews().is_empty());
    first.commit();
    let next = engine.start_pre_trade(order(false)).unwrap();
    assert_eq!(next.reviews().len(), 1);
    assert_eq!(next.reviews()[0].code, RejectCode::RateLimitExceeded);
    assert_eq!(next.reviews()[0].mode, RuleMode::ReviewWithHold);
}
