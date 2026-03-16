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
use std::thread;
use std::time::Duration;

use openpit::param::TradeAmount;
use openpit::param::{Asset, Fee, Pnl, Price, Quantity, Side, Volume};
use openpit::pretrade::policies::pnl_killswitch::PnlKillSwitchError;
use openpit::pretrade::policies::OrderValidationPolicy;
use openpit::pretrade::policies::PnlKillSwitchPolicy;
use openpit::pretrade::policies::RateLimitPolicy;
use openpit::pretrade::policies::{OrderSizeLimit, OrderSizeLimitPolicy};
use openpit::pretrade::{
    CheckPreTradeStartPolicy, Context, Mutation, Mutations, Policy, Reject, RejectCode,
    RejectScope, Rejects, RiskMutation,
};
use openpit::{
    Engine, EngineBuildError, HasClosePosition, HasFee, HasInstrument, HasPnl, HasReduceOnly,
    Instrument, OrderOperation, OrderPosition, WithOrderOperation, WithOrderPosition,
};
use rust_decimal::Decimal;

type TestOrder = OrderOperation;

struct TestReport {
    instrument: Instrument,
    pnl: Pnl,
    fee: Fee,
}

impl HasInstrument for TestReport {
    fn instrument(&self) -> &Instrument {
        &self.instrument
    }
}

impl HasPnl for TestReport {
    fn pnl(&self) -> Pnl {
        self.pnl
    }
}

impl HasFee for TestReport {
    fn fee(&self) -> Fee {
        self.fee
    }
}

#[test]
fn integration_scenario_rate_limit_then_kill_switch_then_reset_resume() {
    let usd = Asset::new("USD").expect("asset code must be valid");
    let shared_pnl = Rc::new(
        PnlKillSwitchPolicy::new((usd.clone(), pnl("500")), [])
            .expect("pnl policy must be configured"),
    );

    let engine = Engine::<TestOrder, TestReport>::builder()
        .check_pre_trade_start_policy(SharedPnlPolicy::new(Rc::clone(&shared_pnl)))
        .check_pre_trade_start_policy(RateLimitPolicy::new(1, Duration::from_millis(500)))
        .build()
        .expect("engine must build");

    let _first_aapl_order = engine
        .start_pre_trade(order_aapl_usd("100", "1"))
        .expect("first AAPL order must pass");

    let rate_limit_reject = match engine.start_pre_trade(order_aapl_usd("100", "1")) {
        Ok(_) => panic!("second AAPL order must hit rate limit"),
        Err(reject) => reject,
    };
    assert_eq!(rate_limit_reject.scope, RejectScope::Order);
    assert_eq!(rate_limit_reject.code, RejectCode::RateLimitExceeded);
    assert_eq!(rate_limit_reject.reason, "rate limit exceeded");
    assert_eq!(
        rate_limit_reject.details,
        "submitted 2 orders in 500ms window, max allowed: 1"
    );

    let post_trade = engine.apply_execution_report(&execution_report_spx_usd("-600"));
    assert!(post_trade.kill_switch_triggered);
    assert_eq!(shared_pnl.realized_pnl(&usd), pnl("-600"));

    let kill_switch_reject = match engine.start_pre_trade(order_aapl_usd("99.5", "1")) {
        Ok(_) => panic!("AAPL order must be blocked by kill switch"),
        Err(reject) => reject,
    };
    assert_eq!(kill_switch_reject.scope, RejectScope::Account);
    assert_eq!(kill_switch_reject.code, RejectCode::PnlKillSwitchTriggered);
    assert_eq!(kill_switch_reject.reason, "pnl kill switch triggered");
    assert_eq!(
        kill_switch_reject.details,
        "realized pnl -600, max allowed loss: 500, settlement asset USD"
    );

    shared_pnl.reset_pnl(&usd);
    assert_eq!(shared_pnl.realized_pnl(&usd), pnl("0"));

    thread::sleep(Duration::from_millis(700));

    let reservation = engine
        .start_pre_trade(order_aapl_usd("101", "2"))
        .expect("trading must resume after reset and window expiry")
        .execute()
        .expect("execute must pass");
    reservation.commit();
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
            name: "missing",
            configure_limit: false,
            quantity: "1",
            price: "100",
            expected_reject: Some((
                RejectCode::RiskConfigurationMissing,
                "order size limit missing",
                "settlement asset USD has no configured limit",
            )),
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
            OrderSizeLimitPolicy::new(order_size_limit_usd("10", "1000"), [])
        } else {
            OrderSizeLimitPolicy::new(order_size_limit_eur("10", "1000"), [])
        };

        let engine = Engine::<TestOrder, TestReport>::builder()
            .check_pre_trade_start_policy(size_limit)
            .build()
            .expect("engine must build");

        let result = engine.start_pre_trade(order_aapl_usd(case.price, case.quantity));
        match case.expected_reject {
            Some((expected_code, expected_reason, expected_details)) => {
                let reject = match result {
                    Ok(_) => panic!("{}", case.name),
                    Err(reject) => reject,
                };
                assert_eq!(reject.scope, RejectScope::Order, "{}", case.name);
                assert_eq!(reject.code, expected_code, "{}", case.name);
                assert_eq!(reject.reason, expected_reason, "{}", case.name);
                assert_eq!(reject.details, expected_details, "{}", case.name);
            }
            None => {
                let reservation = result
                    .expect(case.name)
                    .execute()
                    .expect("boundary order must execute");
                reservation.rollback();
            }
        }
    }

    let size_limit = OrderSizeLimitPolicy::new(order_size_limit_usd("100", "1000"), []);
    let overflow_engine = Engine::<TestOrder, TestReport>::builder()
        .check_pre_trade_start_policy(size_limit)
        .build()
        .expect("overflow engine must build");
    let overflow_order = OrderOperation {
        instrument: Instrument::new(
            Asset::new("AAPL").expect("asset code must be valid"),
            Asset::new("USD").expect("asset code must be valid"),
        ),
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
    let engine = Engine::<TestOrder, TestReport>::builder()
        .check_pre_trade_start_policy(OrderValidationPolicy::new())
        .build()
        .expect("engine must build");

    let zero_quantity_order = OrderOperation {
        instrument: Instrument::new(
            Asset::new("AAPL").expect("asset code must be valid"),
            Asset::new("USD").expect("asset code must be valid"),
        ),
        side: Side::Buy,
        trade_amount: TradeAmount::Quantity(Quantity::ZERO),
        price: Some(Price::from_str("10").expect("price literal must be valid")),
    };
    let reject = match engine.start_pre_trade(zero_quantity_order) {
        Ok(_) => panic!("zero quantity order must reject"),
        Err(reject) => reject,
    };
    assert_eq!(reject.reason, "order quantity must be non-zero");
    assert_eq!(reject.details, "requested quantity 0 is not allowed");

    let valid_quantity_order = OrderOperation {
        instrument: Instrument::new(
            Asset::new("AAPL").expect("asset code must be valid"),
            Asset::new("USD").expect("asset code must be valid"),
        ),
        side: Side::Buy,
        trade_amount: TradeAmount::Quantity(
            Quantity::from_str("5").expect("quantity literal must be valid"),
        ),
        price: None,
    };
    let reservation = engine
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
        side: Side::Buy,
        trade_amount: TradeAmount::Volume(
            Volume::from_str("10").expect("volume literal must be valid"),
        ),
        price: None,
    };
    let reservation = engine
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
        side: Side::Buy,
        trade_amount: TradeAmount::Quantity(
            Quantity::from_str("2").expect("quantity literal must be valid"),
        ),
        price: Some(Price::from_str("-1").expect("price literal must be valid")),
    };
    let reservation = engine
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
        let engine = Engine::<TestOrder, TestReport>::builder()
            .pre_trade_policy(NotionalCapPolicy::new(
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
                let reservation = request.execute().expect("execute must pass");
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
    let reservation = Engine::<TestOrder, TestReport>::builder()
        .build()
        .expect("builder must build")
        .start_pre_trade(order_aapl_usd("100", "1"))
        .expect("engine::builder must build operational engine")
        .execute()
        .expect("engine::builder request must execute");
    reservation.rollback();

    let reservation = Engine::<TestOrder, TestReport>::builder()
        .build()
        .expect("builder must build")
        .start_pre_trade(order_aapl_usd("100", "1"))
        .expect("builder request must start")
        .execute()
        .expect("builder request must execute");
    reservation.commit();

    let duplicate_start = Engine::<TestOrder, TestReport>::builder()
        .check_pre_trade_start_policy(
            PnlKillSwitchPolicy::new(
                (
                    Asset::new("USD").expect("asset code must be valid"),
                    pnl("100"),
                ),
                vec![],
            )
            .expect("policy config must be valid"),
        )
        .check_pre_trade_start_policy(
            PnlKillSwitchPolicy::new(
                (
                    Asset::new("USD").expect("asset code must be valid"),
                    pnl("100"),
                ),
                vec![],
            )
            .expect("policy config must be valid"),
        )
        .build();
    assert!(matches!(
        duplicate_start,
        Err(EngineBuildError::DuplicatePolicyName {
            name: "PnlKillSwitchPolicy",
        })
    ));

    let duplicate_main = Engine::<TestOrder, TestReport>::builder()
        .pre_trade_policy(NotionalCapPolicy::new(
            "MainDup",
            volume("1000"),
            Rc::new(RefCell::new(Vec::new())),
        ))
        .pre_trade_policy(NotionalCapPolicy::new(
            "MainDup",
            volume("2000"),
            Rc::new(RefCell::new(Vec::new())),
        ))
        .build();
    assert!(matches!(
        duplicate_main,
        Err(EngineBuildError::DuplicatePolicyName { name: "MainDup" })
    ));

    let engine = Engine::<TestOrder, TestReport>::builder()
        .pre_trade_policy(NotionalCapPolicy::new(
            "MainDefault",
            volume("1000000"),
            Rc::new(RefCell::new(Vec::new())),
        ))
        .build()
        .expect("engine must build");
    let post_trade = engine.apply_execution_report(&execution_report_spx_usd("0"));
    assert!(!post_trade.kill_switch_triggered);

    let overflow_engine = Engine::<TestOrder, TestReport>::builder()
        .build()
        .expect("overflow engine must build");
    let overflow_order = OrderOperation {
        instrument: Instrument::new(
            Asset::new("AAPL").expect("asset code must be valid"),
            Asset::new("USD").expect("asset code must be valid"),
        ),
        side: Side::Buy,
        trade_amount: TradeAmount::Quantity(
            Quantity::from_str("2").expect("quantity literal must be valid"),
        ),
        price: Some(Price::new(Decimal::MAX)),
    };
    let reservation = overflow_engine
        .start_pre_trade(overflow_order)
        .expect("engine no longer precomputes notional and must allow request creation")
        .execute()
        .expect("without rejecting policies the request must execute");
    reservation.rollback();

    let pnl_policy = PnlKillSwitchPolicy::new(
        (
            Asset::new("EUR").expect("asset code must be valid"),
            pnl("100"),
        ),
        vec![],
    )
    .expect("policy config must be valid");
    assert!(!<PnlKillSwitchPolicy as CheckPreTradeStartPolicy<
        TestOrder,
        TestReport,
    >>::apply_execution_report(
        &pnl_policy,
        &execution_report_spx_usd("-10")
    ));
    let missing_barrier = <PnlKillSwitchPolicy as CheckPreTradeStartPolicy<
        TestOrder,
        TestReport,
    >>::check_pre_trade_start(&pnl_policy, &order_aapl_usd("100", "1"))
    .expect_err("missing barrier must reject");
    assert_eq!(missing_barrier.scope, RejectScope::Order);
    assert_eq!(missing_barrier.code, RejectCode::RiskConfigurationMissing);
    assert_eq!(missing_barrier.reason, "pnl barrier missing");
    assert_eq!(
        missing_barrier.details,
        "settlement asset USD has no configured loss barrier"
    );

    let usd = Asset::new("USD").expect("asset code must be valid");
    let overflow_policy = PnlKillSwitchPolicy::new((usd.clone(), pnl("100")), vec![])
        .expect("policy config must be valid");
    overflow_policy
        .set_barrier(&usd, pnl("90"))
        .expect("set_barrier must accept positive values");
    let set_barrier_error = overflow_policy
        .set_barrier(&usd, pnl("0"))
        .expect_err("set_barrier must reject non-positive values");
    assert_eq!(
        set_barrier_error.to_string(),
        "barrier must be positive for settlement asset USD, got 0"
    );
    overflow_policy
        .report_realized_pnl(&usd, Pnl::new(Decimal::MAX))
        .expect("initial accumulation must succeed");
    let overflow = overflow_policy
        .report_realized_pnl(&usd, Pnl::new(Decimal::MAX))
        .expect_err("second accumulation must overflow");
    assert_eq!(
        overflow,
        PnlKillSwitchError::PnlAccumulationOverflow { settlement: usd }
    );
    assert_eq!(
        overflow.to_string(),
        "pnl accumulation overflow for settlement asset USD"
    );
}

#[test]
fn integration_custom_order_strategy_tag_policy() {
    trait HasStrategyTag {
        fn strategy_tag(&self) -> &str;
    }

    #[derive(Debug)]
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

    impl HasStrategyTag for StrategyOrder {
        fn strategy_tag(&self) -> &str {
            self.strategy_tag
        }
    }

    struct StrategyTagPolicy {
        allowed_tags: &'static [&'static str],
    }

    impl<O, R> CheckPreTradeStartPolicy<O, R> for StrategyTagPolicy
    where
        O: HasStrategyTag,
    {
        fn name(&self) -> &'static str {
            "StrategyTagPolicy"
        }

        fn check_pre_trade_start(&self, order: &O) -> Result<(), Reject> {
            let tag = order.strategy_tag();
            if !self.allowed_tags.contains(&tag) {
                return Err(Reject::new(
                    "StrategyTagPolicy",
                    RejectScope::Order,
                    RejectCode::Other,
                    "strategy tag not allowed",
                    format!("tag '{tag}' is not in the allowed list"),
                ));
            }
            Ok(())
        }

        fn apply_execution_report(&self, _report: &R) -> bool {
            false
        }
    }

    let engine = Engine::<StrategyOrder, TestReport>::builder()
        .check_pre_trade_start_policy(StrategyTagPolicy {
            allowed_tags: &["alpha", "beta"],
        })
        .build()
        .expect("engine must build");

    let allowed_order = StrategyOrder {
        base: order_aapl_usd("100", "1"),
        strategy_tag: "alpha",
    };
    let reservation = engine
        .start_pre_trade(allowed_order)
        .expect("allowed strategy tag must pass")
        .execute()
        .expect("execute must pass");
    reservation.commit();

    let another_allowed = StrategyOrder {
        base: order_aapl_usd("100", "1"),
        strategy_tag: "beta",
    };
    let reservation = engine
        .start_pre_trade(another_allowed)
        .expect("second allowed strategy tag must pass")
        .execute()
        .expect("execute must pass");
    reservation.rollback();

    let disallowed_order = StrategyOrder {
        base: order_aapl_usd("100", "1"),
        strategy_tag: "gamma",
    };
    let reject = engine
        .start_pre_trade(disallowed_order)
        .expect_err("disallowed strategy tag must reject");
    assert_eq!(reject.scope, RejectScope::Order);
    assert_eq!(reject.code, RejectCode::Other);
    assert_eq!(reject.reason, "strategy tag not allowed");
    assert_eq!(reject.details, "tag 'gamma' is not in the allowed list");
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
            side: Side::Sell,
            trade_amount: TradeAmount::Quantity(
                Quantity::from_str("3").expect("quantity literal must be valid"),
            ),
            price: Some(Price::from_str("150").expect("price literal must be valid")),
        },
    };

    assert!(
        order.inner.reduce_only(),
        "reduce_only must be accessible via HasReduceOnly on the inner WithOrderPosition"
    );
    assert!(!order.inner.close_position());

    let engine = Engine::<CompositeOrder, TestReport>::builder()
        .build()
        .expect("engine must build");

    let reservation = engine
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
            side: Side::Buy,
            trade_amount: TradeAmount::Quantity(
                Quantity::from_str("1").expect("quantity literal must be valid"),
            ),
            price: None,
        },
    };
    assert!(!non_reduce_order.inner.reduce_only());

    let reservation = engine
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

impl Policy<TestOrder, TestReport> for NotionalCapPolicy {
    fn name(&self) -> &'static str {
        self.name
    }

    fn perform_pre_trade_check(
        &self,
        ctx: &Context<'_, TestOrder>,
        mutations: &mut Mutations,
        rejects: &mut Vec<Reject>,
    ) {
        let order = ctx.order();
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
            rejects.push(Reject::new(
                self.name(),
                RejectScope::Order,
                RejectCode::RiskLimitExceeded,
                "strategy cap exceeded",
                format!(
                    "requested notional {}, max allowed: {}",
                    requested_notional, self.max_abs_notional
                ),
            ));
            return;
        }

        mutations.push(Mutation {
            commit: RiskMutation::SetKillSwitch {
                id: "integration.noop",
                enabled: false,
            },
            rollback: RiskMutation::SetKillSwitch {
                id: "integration.noop",
                enabled: false,
            },
        });
    }

    fn apply_execution_report(&self, _report: &TestReport) -> bool {
        false
    }
}

struct SharedPnlPolicy {
    inner: Rc<PnlKillSwitchPolicy>,
}

impl SharedPnlPolicy {
    fn new(inner: Rc<PnlKillSwitchPolicy>) -> Self {
        Self { inner }
    }
}

impl CheckPreTradeStartPolicy<TestOrder, TestReport> for SharedPnlPolicy {
    fn name(&self) -> &'static str {
        <PnlKillSwitchPolicy as CheckPreTradeStartPolicy<TestOrder, TestReport>>::name(&self.inner)
    }

    fn check_pre_trade_start(&self, order: &TestOrder) -> Result<(), Reject> {
        <PnlKillSwitchPolicy as CheckPreTradeStartPolicy<
            TestOrder,
            TestReport,
        >>::check_pre_trade_start(&self.inner, order)
    }

    fn apply_execution_report(&self, report: &TestReport) -> bool {
        <PnlKillSwitchPolicy as CheckPreTradeStartPolicy<
            TestOrder,
            TestReport,
        >>::apply_execution_report(&self.inner, report)
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

fn order_size_limit_usd(max_quantity: &str, max_notional: &str) -> OrderSizeLimit {
    OrderSizeLimit {
        max_notional: volume(max_notional),
        max_quantity: Quantity::from_str(max_quantity).expect("max quantity literal must be valid"),
        settlement_asset: Asset::new("USD").expect("asset code must be valid"),
    }
}

fn order_size_limit_eur(max_quantity: &str, max_notional: &str) -> OrderSizeLimit {
    OrderSizeLimit {
        settlement_asset: Asset::new("EUR").expect("asset code must be valid"),
        max_quantity: Quantity::from_str(max_quantity).expect("max quantity literal must be valid"),
        max_notional: Volume::from_str(max_notional).expect("max notional literal must be valid"),
    }
}
