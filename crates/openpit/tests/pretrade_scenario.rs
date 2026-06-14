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
use std::ops::Deref;
use std::rc::Rc;
use std::time::Duration;

use openpit::param::TradeAmount;
use openpit::param::{AccountId, Asset, Fee, Pnl, Price, Quantity, Side, Volume};
use openpit::pretrade::policies::OrderValidationPolicy;
use openpit::pretrade::policies::PnlBoundsBrokerBarrier;
use openpit::pretrade::policies::{
    OrderSizeAssetBarrier, OrderSizeLimit, OrderSizeLimitPolicy, OrderSizeLimitSettings,
};
use openpit::pretrade::policies::{PnlBoundsKillSwitchPolicy, PnlBoundsKillSwitchSettings};
use openpit::pretrade::policies::{
    RateLimit, RateLimitBrokerBarrier, RateLimitPolicy, RateLimitSettings,
};
use openpit::pretrade::{
    PolicyPreTradeResult, PostTradeContext, PreTradeContext, PreTradePolicy, Reject, RejectCode,
    RejectScope, Rejects,
};
use openpit::storage::NoLocking;
use openpit::{
    Engine, EngineBuildError, ExecutionReportOperation, FinancialImpact, HasAccountId,
    HasClosePosition, HasFee, HasInstrument, HasPnl, HasReduceOnly, HasTradeAmount, Instrument,
    LocalSync, Mutation, Mutations, OrderOperation, OrderPosition, WithExecutionReportOperation,
    WithFinancialImpact, WithOrderOperation, WithOrderPosition,
};
use rust_decimal::Decimal;

type TestOrder = OrderOperation;

type TestPnlPolicy = PnlBoundsKillSwitchPolicy<NoLocking>;

struct TestReport {
    instrument: Instrument,
    account_id: AccountId,
    pnl: Pnl,
    fee: Fee,
}

impl HasInstrument for TestReport {
    fn instrument(&self) -> Result<&Instrument, openpit::RequestFieldAccessError> {
        Ok(&self.instrument)
    }
}

impl HasAccountId for TestReport {
    fn account_id(&self) -> Result<AccountId, openpit::RequestFieldAccessError> {
        Ok(self.account_id)
    }
}

impl HasPnl for TestReport {
    fn pnl(&self) -> Result<Pnl, openpit::RequestFieldAccessError> {
        Ok(self.pnl)
    }
}

impl HasFee for TestReport {
    fn fee(&self) -> Result<Fee, openpit::RequestFieldAccessError> {
        Ok(self.fee)
    }
}

#[test]
fn integration_scenario_rate_limit_then_kill_switch() {
    let usd = Asset::new("USD").expect("asset code must be valid");
    let builder = Engine::builder().no_sync();
    let shared_pnl = Rc::new(PnlBoundsKillSwitchPolicy::new(
        PnlBoundsKillSwitchSettings::new(
            [pnl_bounds_barrier(usd.clone(), Some(pnl("-500")), None)],
            [],
        )
        .expect("pnl settings must be valid"),
        builder.storage_builder(),
    ));
    let rate_policy = RateLimitPolicy::new(
        RateLimitSettings::new(
            Some(RateLimitBrokerBarrier {
                limit: RateLimit {
                    max_orders: 1,
                    window: Duration::from_millis(500),
                },
            }),
            [],
            [],
            [],
        )
        .expect("rate limit settings must be valid"),
        builder.storage_builder(),
    );

    let engine = builder
        .pre_trade(SharedPnlPolicy::new(Rc::clone(&shared_pnl)))
        .pre_trade(rate_policy)
        .build()
        .expect("engine must build");

    let _first_aapl_order = engine
        .start_pre_trade(order_aapl_usd("100", "1"))
        .expect("first AAPL order must pass");

    let rate_limit_reject = match engine.start_pre_trade(order_aapl_usd("100", "1")) {
        Ok(_) => panic!("second AAPL order must hit rate limit"),
        Err(reject) => reject,
    };
    let rate_limit_reject = &rate_limit_reject[0];
    assert_eq!(rate_limit_reject.scope, RejectScope::Order);
    assert_eq!(rate_limit_reject.code, RejectCode::RateLimitExceeded);
    assert_eq!(
        rate_limit_reject.reason,
        "rate limit exceeded: broker barrier"
    );
    assert_eq!(
        rate_limit_reject.details,
        "submitted 2 orders in 500ms window, max allowed: 1"
    );

    let post_trade = engine.apply_execution_report(&execution_report_spx_usd("-600"));
    assert!(!post_trade.account_blocks.is_empty());

    let kill_switch_reject = match engine.start_pre_trade(order_aapl_usd("99.5", "1")) {
        Ok(_) => panic!("AAPL order must be blocked by kill switch"),
        Err(reject) => reject,
    };
    let kill_switch_reject = &kill_switch_reject[0];
    assert_eq!(kill_switch_reject.scope, RejectScope::Account);
    assert_eq!(kill_switch_reject.code, RejectCode::PnlKillSwitchTriggered);
    assert_eq!(kill_switch_reject.reason, "pnl kill switch triggered");
}

#[test]
fn integration_table_order_size_limit_paths() {
    struct Case {
        name: &'static str,
        configure_limit: bool,
        quantity: &'static str,
        price: &'static str,
        expected_reject: Option<(RejectCode, &'static str, &'static str)>,
    }

    let cases = [
        Case {
            name: "no_applicable_limit",
            configure_limit: false,
            quantity: "1",
            price: "100",
            expected_reject: None,
        },
        Case {
            name: "quantity",
            configure_limit: true,
            quantity: "11",
            price: "90",
            expected_reject: Some((
                RejectCode::OrderQtyExceedsLimit,
                "order quantity exceeded",
                "requested 11, max allowed: 10",
            )),
        },
        Case {
            name: "notional",
            configure_limit: true,
            quantity: "10",
            price: "101",
            expected_reject: Some((
                RejectCode::OrderNotionalExceedsLimit,
                "order notional exceeded",
                "requested 1010, max allowed: 1000",
            )),
        },
        Case {
            name: "both",
            configure_limit: true,
            quantity: "11",
            price: "100",
            expected_reject: Some((
                RejectCode::OrderExceedsLimit,
                "order size exceeded",
                "requested quantity 11, max allowed: 10; requested notional 1100, max allowed: 1000",
            )),
        },
        Case {
            name: "boundary",
            configure_limit: true,
            quantity: "10",
            price: "100",
            expected_reject: None,
        },
    ];

    for case in cases {
        let size_limit = if case.configure_limit {
            OrderSizeLimitPolicy::<NoLocking>::new(
                OrderSizeLimitSettings::new(None, [order_size_limit_usd("10", "1000")], [])
                    .expect("valid config"),
            )
        } else {
            OrderSizeLimitPolicy::<NoLocking>::new(
                OrderSizeLimitSettings::new(None, [order_size_limit_eur("10", "1000")], [])
                    .expect("valid config"),
            )
        };

        let engine = Engine::builder::<TestOrder, TestReport, ()>()
            .no_sync()
            .pre_trade(size_limit)
            .build()
            .expect("engine must build");

        let result = engine.start_pre_trade(order_aapl_usd(case.price, case.quantity));
        match case.expected_reject {
            Some((expected_code, expected_reason, expected_details)) => {
                let reject = match result {
                    Ok(_) => panic!("{}", case.name),
                    Err(reject) => reject,
                };
                let reject = &reject[0];
                assert_eq!(reject.scope, RejectScope::Order, "{}", case.name);
                assert_eq!(reject.code, expected_code, "{}", case.name);
                assert_eq!(reject.reason, expected_reason, "{}", case.name);
                assert_eq!(reject.details, expected_details, "{}", case.name);
            }
            None => {
                let mut reservation = result
                    .expect(case.name)
                    .execute()
                    .expect("boundary order must execute");
                reservation.rollback();
            }
        }
    }

    let size_limit = OrderSizeLimitPolicy::<NoLocking>::new(
        OrderSizeLimitSettings::new(None, [order_size_limit_usd("100", "1000")], [])
            .expect("valid config"),
    );
    let overflow_engine = Engine::builder::<TestOrder, TestReport, ()>()
        .no_sync()
        .pre_trade(size_limit)
        .build()
        .expect("overflow engine must build");
    let overflow_order = OrderOperation {
        instrument: Instrument::new(
            Asset::new("AAPL").expect("asset code must be valid"),
            Asset::new("USD").expect("asset code must be valid"),
        ),
        account_id: AccountId::from_u64(99224416),
        side: Side::Buy,
        trade_amount: TradeAmount::Quantity(
            Quantity::from_str("2").expect("quantity literal must be valid"),
        ),
        price: Some(Price::new(Decimal::MAX)),
    };
    let overflow_reject = match overflow_engine.start_pre_trade(overflow_order) {
        Ok(_) => panic!("overflow order must reject"),
        Err(reject) => reject,
    };
    let overflow_reject = &overflow_reject[0];
    assert_eq!(
        overflow_reject.code,
        RejectCode::OrderValueCalculationFailed
    );
    assert_eq!(overflow_reject.reason, "order value calculation failed");
    assert_eq!(
        overflow_reject.details,
        "price or quantity could not be used to evaluate order notional"
    );
}

#[test]
fn integration_order_validation_checks_only_provided_fields() {
    let engine = Engine::builder::<TestOrder, TestReport, ()>()
        .no_sync()
        .pre_trade(OrderValidationPolicy::new())
        .build()
        .expect("engine must build");

    let zero_quantity_order = OrderOperation {
        instrument: Instrument::new(
            Asset::new("AAPL").expect("asset code must be valid"),
            Asset::new("USD").expect("asset code must be valid"),
        ),
        account_id: AccountId::from_u64(99224416),
        side: Side::Buy,
        trade_amount: TradeAmount::Quantity(Quantity::ZERO),
        price: Some(Price::from_str("10").expect("price literal must be valid")),
    };
    let reject = match engine.start_pre_trade(zero_quantity_order) {
        Ok(_) => panic!("zero quantity order must reject"),
        Err(reject) => reject,
    };
    let reject = &reject[0];
    assert_eq!(reject.reason, "order quantity must be non-zero");
    assert_eq!(reject.details, "requested quantity 0 is not allowed");

    let valid_quantity_order = OrderOperation {
        instrument: Instrument::new(
            Asset::new("AAPL").expect("asset code must be valid"),
            Asset::new("USD").expect("asset code must be valid"),
        ),
        account_id: AccountId::from_u64(99224416),
        side: Side::Buy,
        trade_amount: TradeAmount::Quantity(
            Quantity::from_str("5").expect("quantity literal must be valid"),
        ),
        price: None,
    };
    let mut reservation = engine
        .start_pre_trade(valid_quantity_order)
        .expect("valid quantity without price must pass validation")
        .execute()
        .expect("main stage must pass");
    reservation.rollback();

    let volume_order = OrderOperation {
        instrument: Instrument::new(
            Asset::new("AAPL").expect("asset code must be valid"),
            Asset::new("USD").expect("asset code must be valid"),
        ),
        account_id: AccountId::from_u64(99224416),
        side: Side::Buy,
        trade_amount: TradeAmount::Volume(
            Volume::from_str("10").expect("volume literal must be valid"),
        ),
        price: None,
    };
    let mut reservation = engine
        .start_pre_trade(volume_order)
        .expect("volume-only order must pass validation")
        .execute()
        .expect("main stage must pass");
    reservation.rollback();

    let negative_price_order = OrderOperation {
        instrument: Instrument::new(
            Asset::new("AAPL").expect("asset code must be valid"),
            Asset::new("USD").expect("asset code must be valid"),
        ),
        account_id: AccountId::from_u64(99224416),
        side: Side::Buy,
        trade_amount: TradeAmount::Quantity(
            Quantity::from_str("2").expect("quantity literal must be valid"),
        ),
        price: Some(Price::from_str("-1").expect("price literal must be valid")),
    };
    let mut reservation = engine
        .start_pre_trade(negative_price_order)
        .expect("negative price must pass validation")
        .execute()
        .expect("main stage must pass");
    reservation.rollback();
}

#[test]
fn integration_table_main_stage_paths() {
    enum Finalization {
        Commit,
        Drop,
        Reject,
    }

    struct Case {
        name: &'static str,
        side: Side,
        quantity: &'static str,
        price: &'static str,
        max_abs_notional: &'static str,
        finalization: Finalization,
        expected_context_notional: &'static str,
        expected_reject: Option<(RejectCode, &'static str, &'static str)>,
    }

    let cases = [
        Case {
            name: "commit_success",
            side: Side::Sell,
            quantity: "5",
            price: "100",
            max_abs_notional: "700",
            finalization: Finalization::Commit,
            expected_context_notional: "500",
            expected_reject: None,
        },
        Case {
            name: "drop_success",
            side: Side::Buy,
            quantity: "3",
            price: "100",
            max_abs_notional: "700",
            finalization: Finalization::Drop,
            expected_context_notional: "-300",
            expected_reject: None,
        },
        Case {
            name: "immediate_reject",
            side: Side::Buy,
            quantity: "8",
            price: "100",
            max_abs_notional: "700",
            finalization: Finalization::Reject,
            expected_context_notional: "-800",
            expected_reject: Some((
                RejectCode::RiskLimitExceeded,
                "strategy cap exceeded",
                "requested notional 800, max allowed: 700",
            )),
        },
    ];

    for case in cases {
        let journal = Rc::new(RefCell::new(Vec::new()));
        let engine = Engine::builder()
            .no_sync()
            .pre_trade(NotionalCapPolicy::new(
                "NotionalCapPolicy",
                volume(case.max_abs_notional),
                Rc::clone(&journal),
            ))
            .build()
            .expect("engine must build");

        let request = engine
            .start_pre_trade(order_aapl_usd_with_side(
                case.price,
                case.quantity,
                case.side,
            ))
            .expect(case.name);

        match case.finalization {
            Finalization::Commit => {
                let mut reservation = request.execute().expect("execute must pass");
                reservation.commit();
            }
            Finalization::Drop => {
                let reservation = request.execute().expect("execute must pass");
                drop(reservation);
            }
            Finalization::Reject => {
                let rejects: Rejects = match request.execute() {
                    Ok(_) => panic!("execute must reject"),
                    Err(rejects) => rejects,
                };
                assert_eq!(rejects.len(), 1, "{}", case.name);
                let (expected_code, expected_reason, expected_details) =
                    case.expected_reject.expect("reject expectation");
                assert_eq!(rejects[0].policy, "NotionalCapPolicy", "{}", case.name);
                assert_eq!(rejects[0].code, expected_code, "{}", case.name);
                assert_eq!(rejects[0].reason, expected_reason, "{}", case.name);
                assert_eq!(rejects[0].scope, RejectScope::Order, "{}", case.name);
                assert_eq!(rejects[0].details, expected_details, "{}", case.name);
            }
        }

        let journal = journal.borrow();
        assert_eq!(journal.len(), 1, "{}", case.name);
        assert_eq!(journal[0].underlying, "AAPL", "{}", case.name);
        assert_eq!(journal[0].settlement, "USD", "{}", case.name);
        assert_eq!(
            journal[0].notional, case.expected_context_notional,
            "{}",
            case.name
        );
    }
}

#[test]
fn integration_engine_builder_defaults_and_guardrails() {
    let mut reservation = Engine::builder::<TestOrder, TestReport, ()>()
        .no_sync()
        .pre_trade(OrderValidationPolicy::new())
        .build()
        .expect("builder must build")
        .start_pre_trade(order_aapl_usd("100", "1"))
        .expect("engine::builder must build operational engine")
        .execute()
        .expect("engine::builder request must execute");
    reservation.rollback();

    let mut reservation = Engine::builder::<TestOrder, TestReport, ()>()
        .no_sync()
        .pre_trade(OrderValidationPolicy::new())
        .build()
        .expect("builder must build")
        .start_pre_trade(order_aapl_usd("100", "1"))
        .expect("builder request must start")
        .execute()
        .expect("builder request must execute");
    reservation.commit();

    let dup_builder = Engine::builder::<TestOrder, TestReport, ()>().no_sync();
    let first_duplicate_policy = PnlBoundsKillSwitchPolicy::new(
        PnlBoundsKillSwitchSettings::new(
            [pnl_bounds_barrier(
                Asset::new("USD").expect("asset code must be valid"),
                Some(pnl("-100")),
                None,
            )],
            vec![],
        )
        .expect("policy config must be valid"),
        dup_builder.storage_builder(),
    );
    let second_duplicate_policy = PnlBoundsKillSwitchPolicy::new(
        PnlBoundsKillSwitchSettings::new(
            [pnl_bounds_barrier(
                Asset::new("USD").expect("asset code must be valid"),
                Some(pnl("-100")),
                None,
            )],
            vec![],
        )
        .expect("policy config must be valid"),
        dup_builder.storage_builder(),
    );
    let duplicate_start = dup_builder
        .pre_trade(first_duplicate_policy)
        .pre_trade(second_duplicate_policy)
        .build();
    assert!(matches!(
        duplicate_start,
        Err(EngineBuildError::DuplicatePolicyName { name }) if name == "PnlBoundsKillSwitchPolicy"
    ));

    let duplicate_main = Engine::builder()
        .no_sync()
        .pre_trade(NotionalCapPolicy::new(
            "MainDup",
            volume("1000"),
            Rc::new(RefCell::new(Vec::new())),
        ))
        .pre_trade(NotionalCapPolicy::new(
            "MainDup",
            volume("2000"),
            Rc::new(RefCell::new(Vec::new())),
        ))
        .build();
    assert!(matches!(
        duplicate_main,
        Err(EngineBuildError::DuplicatePolicyName { name }) if name == "MainDup"
    ));

    let engine = Engine::builder()
        .no_sync()
        .pre_trade(NotionalCapPolicy::new(
            "MainDefault",
            volume("1000000"),
            Rc::new(RefCell::new(Vec::new())),
        ))
        .build()
        .expect("engine must build");
    let post_trade = engine.apply_execution_report(&execution_report_spx_usd("0"));
    assert!(post_trade.account_blocks.is_empty());

    let overflow_engine = Engine::builder::<TestOrder, TestReport, ()>()
        .no_sync()
        .pre_trade(OrderValidationPolicy::new())
        .build()
        .expect("overflow engine must build");
    let overflow_order = OrderOperation {
        instrument: Instrument::new(
            Asset::new("AAPL").expect("asset code must be valid"),
            Asset::new("USD").expect("asset code must be valid"),
        ),
        account_id: AccountId::from_u64(99224416),
        side: Side::Buy,
        trade_amount: TradeAmount::Quantity(
            Quantity::from_str("2").expect("quantity literal must be valid"),
        ),
        price: Some(Price::new(Decimal::MAX)),
    };
    let mut reservation = overflow_engine
        .start_pre_trade(overflow_order)
        .expect("engine no longer precomputes notional and must allow request creation")
        .execute()
        .expect("without rejecting policies the request must execute");
    reservation.rollback();

    let misc_builder = Engine::builder::<TestOrder, TestReport, ()>().no_sync();
    let pnl_policy: TestPnlPolicy = PnlBoundsKillSwitchPolicy::new(
        PnlBoundsKillSwitchSettings::new(
            [pnl_bounds_barrier(
                Asset::new("EUR").expect("asset code must be valid"),
                Some(pnl("-100")),
                None,
            )],
            vec![],
        )
        .expect("policy config must be valid"),
        misc_builder.storage_builder(),
    );
    assert!(
        <TestPnlPolicy as PreTradePolicy<TestOrder, TestReport, (), LocalSync>>::apply_execution_report(
            &pnl_policy,
            &PostTradeContext::<NoLocking>::new(),
            &execution_report_spx_usd("-10")
        )
        .is_none_or(|r| r.is_empty())
    );
    // EUR-only policy has no barrier for USD: order passes.
    <TestPnlPolicy as PreTradePolicy<TestOrder, TestReport, (), LocalSync>>::check_pre_trade_start(
        &pnl_policy,
        &PreTradeContext::new(None),
        &order_aapl_usd("100", "1"),
    )
    .expect("no barrier configured for USD: order must pass");

    // Arithmetic overflow inside apply_execution_report returns true (kill switch).
    // Two MAX applications: first stores MAX, second overflows → true.
    let overflow_account = AccountId::from_u64(1);
    let overflow_policy: TestPnlPolicy = PnlBoundsKillSwitchPolicy::new(
        PnlBoundsKillSwitchSettings::new(
            [pnl_bounds_barrier(
                Asset::new("USD").expect("asset code must be valid"),
                Some(pnl("-100")),
                None,
            )],
            vec![],
        )
        .expect("policy config must be valid"),
        misc_builder.storage_builder(),
    );
    let report_max = TestReport {
        instrument: Instrument::new(
            Asset::new("AAPL").expect("must be valid"),
            Asset::new("USD").expect("must be valid"),
        ),
        account_id: overflow_account,
        pnl: Pnl::new(Decimal::MAX),
        fee: Fee::ZERO,
    };
    assert!(
        <TestPnlPolicy as PreTradePolicy<TestOrder, TestReport, (), LocalSync>>::apply_execution_report(
            &overflow_policy,
            &PostTradeContext::<NoLocking>::new(),
            &report_max,
        )
        .is_none_or(|r| r.is_empty())
    );
    let triggered =
        <TestPnlPolicy as PreTradePolicy<TestOrder, TestReport, (), LocalSync>>::apply_execution_report(
            &overflow_policy,
            &PostTradeContext::<NoLocking>::new(),
            &report_max,
        );
    assert!(!triggered.is_none_or(|r| r.is_empty()));
}

#[test]
fn integration_custom_order_strategy_tag_policy() {
    trait HasStrategyTag {
        fn strategy_tag(&self) -> &'static str;
    }

    type PitExecutionReport = WithExecutionReportOperation<WithFinancialImpact<()>>;

    #[derive(Clone)]
    struct StrategyOrder {
        base: OrderOperation,
        strategy_tag: &'static str,
    }

    impl Deref for StrategyOrder {
        type Target = OrderOperation;
        fn deref(&self) -> &Self::Target {
            &self.base
        }
    }

    struct StrategyExecutionReport {
        base: PitExecutionReport,
        report_tag: &'static str,
    }

    impl Deref for StrategyExecutionReport {
        type Target = PitExecutionReport;

        fn deref(&self) -> &Self::Target {
            &self.base
        }
    }

    impl HasStrategyTag for StrategyOrder {
        fn strategy_tag(&self) -> &'static str {
            self.strategy_tag
        }
    }

    struct StrategyTagPolicy;

    impl<O, R, A, Sync> PreTradePolicy<O, R, A, Sync> for StrategyTagPolicy
    where
        O: HasStrategyTag + HasTradeAmount,
        Sync: openpit::SyncMode,
    {
        fn name(&self) -> &str {
            "StrategyTagPolicy"
        }

        fn perform_pre_trade_check(
            &self,
            _ctx: &PreTradeContext<<Sync as openpit::SyncMode>::StorageLockingPolicyFactory>,
            order: &O,
            _mutations: &mut Mutations,
        ) -> Result<Option<PolicyPreTradeResult>, Rejects> {
            if order.strategy_tag() == "blocked" {
                return Err(Rejects::from(Reject::new(
                    "StrategyTagPolicy",
                    RejectScope::Order,
                    RejectCode::ComplianceRestriction,
                    "strategy blocked",
                    "project strategy tag blocked",
                )));
            }
            Ok(None)
        }

        fn apply_execution_report(
            &self,
            _ctx: &PostTradeContext<<Sync as openpit::SyncMode>::StorageLockingPolicyFactory>,
            _report: &R,
        ) -> Option<openpit::PostTradeResult> {
            None
        }
    }

    let engine = Engine::builder::<StrategyOrder, StrategyExecutionReport, ()>()
        .no_sync()
        .pre_trade(StrategyTagPolicy)
        .build()
        .expect("engine must build");

    let allowed_order = StrategyOrder {
        base: order_aapl_usd("100", "1"),
        strategy_tag: "allowed",
    };
    let reservation = engine
        .start_pre_trade(allowed_order)
        .expect("start must pass");
    let mut reservation = reservation.execute().expect("execute must pass");
    reservation.commit();

    let disallowed_order = StrategyOrder {
        base: order_aapl_usd("100", "1"),
        strategy_tag: "blocked",
    };
    let request = engine
        .start_pre_trade(disallowed_order)
        .expect("start must pass for blocked order");
    let rejects = match request.execute() {
        Ok(_) => panic!("blocked strategy tag must reject"),
        Err(rejects) => rejects,
    };
    assert_eq!(rejects.len(), 1);
    assert_eq!(rejects[0].scope, RejectScope::Order);
    assert_eq!(rejects[0].code, RejectCode::ComplianceRestriction);
    assert_eq!(rejects[0].reason, "strategy blocked");
    assert_eq!(rejects[0].details, "project strategy tag blocked");

    let report = StrategyExecutionReport {
        base: WithExecutionReportOperation {
            inner: WithFinancialImpact {
                inner: (),
                financial_impact: FinancialImpact {
                    pnl: Pnl::from_str("5").expect("must be valid"),
                    fee: Fee::from_str("1").expect("must be valid"),
                },
            },
            operation: ExecutionReportOperation {
                instrument: Instrument::new(
                    Asset::new("AAPL").expect("asset code must be valid"),
                    Asset::new("USD").expect("asset code must be valid"),
                ),
                side: Side::Buy,
                account_id: AccountId::from_u64(99224416),
            },
        },
        report_tag: "fill-1",
    };
    let _ = report.report_tag;
    let post_trade = engine.apply_execution_report(&report);
    assert!(post_trade.account_blocks.is_empty());
}

#[test]
fn integration_with_order_operation_with_order_position_reduce_only_accessible() {
    type CompositeOrder = WithOrderOperation<WithOrderPosition<()>>;

    let order = WithOrderOperation {
        inner: WithOrderPosition {
            inner: (),
            position: OrderPosition {
                position_side: None,
                reduce_only: true,
                close_position: false,
            },
        },
        operation: OrderOperation {
            instrument: Instrument::new(
                Asset::new("AAPL").expect("asset code must be valid"),
                Asset::new("USD").expect("asset code must be valid"),
            ),
            account_id: AccountId::from_u64(99224416),
            side: Side::Sell,
            trade_amount: TradeAmount::Quantity(
                Quantity::from_str("3").expect("quantity literal must be valid"),
            ),
            price: Some(Price::from_str("150").expect("price literal must be valid")),
        },
    };

    assert_eq!(
        order.inner.reduce_only(),
        Ok(true),
        "reduce_only must be accessible via HasReduceOnly on the inner WithOrderPosition"
    );
    assert_eq!(order.inner.close_position(), Ok(false));

    let engine = Engine::builder::<CompositeOrder, TestReport, ()>()
        .no_sync()
        .pre_trade(OrderValidationPolicy::new())
        .build()
        .expect("engine must build");

    let mut reservation = engine
        .start_pre_trade(order)
        .expect("composite order must pass pre-trade")
        .execute()
        .expect("composite order execute must pass");
    reservation.commit();

    let non_reduce_order = WithOrderOperation {
        inner: WithOrderPosition {
            inner: (),
            position: OrderPosition {
                position_side: None,
                reduce_only: false,
                close_position: false,
            },
        },
        operation: OrderOperation {
            instrument: Instrument::new(
                Asset::new("AAPL").expect("asset code must be valid"),
                Asset::new("USD").expect("asset code must be valid"),
            ),
            account_id: AccountId::from_u64(99224416),
            side: Side::Buy,
            trade_amount: TradeAmount::Quantity(
                Quantity::from_str("1").expect("quantity literal must be valid"),
            ),
            price: None,
        },
    };
    assert_eq!(non_reduce_order.inner.reduce_only(), Ok(false));

    let mut reservation = engine
        .start_pre_trade(non_reduce_order)
        .expect("non-reduce-only order must pass")
        .execute()
        .expect("execute must pass");
    reservation.rollback();
}

//--------------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
struct ObservedContext {
    underlying: String,
    settlement: String,
    notional: String,
}

struct NotionalCapPolicy {
    name: &'static str,
    max_abs_notional: Volume,
    journal: Rc<RefCell<Vec<ObservedContext>>>,
}

impl NotionalCapPolicy {
    fn new(
        name: &'static str,
        max_abs_notional: Volume,
        journal: Rc<RefCell<Vec<ObservedContext>>>,
    ) -> Self {
        Self {
            name,
            max_abs_notional,
            journal,
        }
    }
}

impl PreTradePolicy<TestOrder, TestReport, (), LocalSync> for NotionalCapPolicy {
    fn name(&self) -> &str {
        self.name
    }

    fn perform_pre_trade_check(
        &self,
        _ctx: &PreTradeContext<NoLocking>,
        order: &TestOrder,
        mutations: &mut Mutations,
    ) -> Result<Option<PolicyPreTradeResult>, Rejects> {
        let requested_notional = order
            .price
            .expect("price must be present")
            .calculate_volume(match order.trade_amount {
                TradeAmount::Quantity(value) => value,
                TradeAmount::Volume(_) => panic!("quantity-based order expected"),
                _ => panic!("unsupported trade amount variant"),
            })
            .expect("requested notional must be calculable");
        let signed_notional = match order.side {
            Side::Buy => requested_notional.to_cash_flow_outflow(),
            Side::Sell => requested_notional.to_cash_flow_inflow(),
        };

        self.journal.borrow_mut().push(ObservedContext {
            underlying: order.instrument.underlying_asset().to_string(),
            settlement: order.instrument.settlement_asset().to_string(),
            notional: signed_notional.to_string(),
        });

        if requested_notional.to_decimal() > self.max_abs_notional.to_decimal() {
            return Err(Rejects::from(Reject::new(
                self.name(),
                RejectScope::Order,
                RejectCode::RiskLimitExceeded,
                "strategy cap exceeded",
                format!(
                    "requested notional {}, max allowed: {}",
                    requested_notional, self.max_abs_notional
                ),
            )));
        }

        mutations.push(Mutation::new(|| {}, || {}));
        Ok(None)
    }

    fn apply_execution_report(
        &self,
        _ctx: &PostTradeContext<NoLocking>,
        _report: &TestReport,
    ) -> Option<openpit::PostTradeResult> {
        None
    }
}

struct SharedPnlPolicy {
    inner: Rc<TestPnlPolicy>,
}

impl SharedPnlPolicy {
    fn new(inner: Rc<TestPnlPolicy>) -> Self {
        Self { inner }
    }
}

impl PreTradePolicy<TestOrder, TestReport, (), LocalSync> for SharedPnlPolicy {
    fn name(&self) -> &str {
        <TestPnlPolicy as PreTradePolicy<TestOrder, TestReport, (), LocalSync>>::name(&self.inner)
    }

    fn check_pre_trade_start(
        &self,
        _ctx: &PreTradeContext<NoLocking>,
        order: &TestOrder,
    ) -> Result<(), Rejects> {
        <TestPnlPolicy as PreTradePolicy<TestOrder, TestReport, (), LocalSync>>::check_pre_trade_start(
            &self.inner,
            &PreTradeContext::new(None),
            order,
        )
    }

    fn apply_execution_report(
        &self,
        ctx: &PostTradeContext<NoLocking>,
        report: &TestReport,
    ) -> Option<openpit::PostTradeResult> {
        <TestPnlPolicy as PreTradePolicy<TestOrder, TestReport, (), LocalSync>>::apply_execution_report(
            &self.inner,
            ctx,
            report,
        )
    }
}

fn order_aapl_usd(price: &str, quantity: &str) -> OrderOperation {
    order_aapl_usd_with_side(price, quantity, Side::Buy)
}

fn order_aapl_usd_with_side(price: &str, quantity: &str, side: Side) -> OrderOperation {
    OrderOperation {
        instrument: Instrument::new(
            Asset::new("AAPL").expect("asset code must be valid"),
            Asset::new("USD").expect("asset code must be valid"),
        ),
        account_id: AccountId::from_u64(99224416),
        side,
        trade_amount: TradeAmount::Quantity(
            Quantity::from_str(quantity).expect("quantity literal must be valid"),
        ),
        price: Some(Price::from_str(price).expect("price literal must be valid")),
    }
}

fn execution_report_spx_usd(pnl_value: &str) -> TestReport {
    TestReport {
        instrument: Instrument::new(
            Asset::new("SPX").expect("asset code must be valid"),
            Asset::new("USD").expect("asset code must be valid"),
        ),
        account_id: AccountId::from_u64(99224416),
        pnl: pnl(pnl_value),
        fee: Fee::ZERO,
    }
}

fn pnl(value: &str) -> Pnl {
    Pnl::from_str(value).expect("pnl literal must be valid")
}

fn volume(value: &str) -> Volume {
    Volume::from_str(value).expect("volume literal must be valid")
}

fn pnl_bounds_barrier(
    settlement_asset: Asset,
    lower_bound: Option<Pnl>,
    upper_bound: Option<Pnl>,
) -> PnlBoundsBrokerBarrier {
    PnlBoundsBrokerBarrier {
        settlement_asset,
        lower_bound,
        upper_bound,
    }
}

fn order_size_limit_usd(max_quantity: &str, max_notional: &str) -> OrderSizeAssetBarrier {
    OrderSizeAssetBarrier {
        limit: OrderSizeLimit {
            max_quantity: Quantity::from_str(max_quantity)
                .expect("max quantity literal must be valid"),
            max_notional: volume(max_notional),
        },
        settlement_asset: Asset::new("USD").expect("asset code must be valid"),
    }
}

fn order_size_limit_eur(max_quantity: &str, max_notional: &str) -> OrderSizeAssetBarrier {
    OrderSizeAssetBarrier {
        limit: OrderSizeLimit {
            max_quantity: Quantity::from_str(max_quantity)
                .expect("max quantity literal must be valid"),
            max_notional: Volume::from_str(max_notional)
                .expect("max notional literal must be valid"),
        },
        settlement_asset: Asset::new("EUR").expect("asset code must be valid"),
    }
}
