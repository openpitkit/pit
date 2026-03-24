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
use std::rc::Rc;
use std::thread;
use std::time::Duration;

use openpit::param::{Asset, Fee, Pnl, Price, Quantity, Side, Volume};
use openpit::pretrade::policies::PnlKillSwitchPolicy;
use openpit::pretrade::policies::RateLimitPolicy;
use openpit::pretrade::policies::{OrderSizeLimit, OrderSizeLimitPolicy};
use openpit::pretrade::{
    CheckPreTradeStartPolicy, Context, ExecutionReport, Mutation, Mutations, Policy, Reject,
    RejectCode, RejectScope,
};
use openpit::{Engine, Instrument, Order};

#[test]
fn integration_scenario_rate_limit_then_kill_switch_then_reset_resume() {
    let usd = Asset::new("USD").expect("asset code must be valid");
    let shared_pnl = Rc::new(
        PnlKillSwitchPolicy::new((usd.clone(), pnl("500")), vec![])
            .expect("pnl policy must be configured"),
    );

    let engine = Engine::builder()
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

        let engine = Engine::builder()
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
                let rejects = match request.execute() {
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

impl Policy for NotionalCapPolicy {
    fn name(&self) -> &'static str {
        self.name
    }

    fn perform_pre_trade_check(
        &self,
        ctx: &Context<'_>,
        mutations: &mut Mutations,
        rejects: &mut Vec<Reject>,
    ) {
        self.journal.borrow_mut().push(ObservedContext {
            underlying: ctx.order().instrument.underlying_asset().to_string(),
            settlement: ctx.order().instrument.settlement_asset().to_string(),
            notional: ctx.notional().to_string(),
        });

        let requested =
            Volume::new(ctx.notional().to_decimal().abs()).expect("volume must be valid");
        if requested.to_decimal() > self.max_abs_notional.to_decimal() {
            rejects.push(Reject::new(
                self.name(),
                RejectScope::Order,
                RejectCode::RiskLimitExceeded,
                "strategy cap exceeded",
                format!(
                    "requested notional {}, max allowed: {}",
                    requested, self.max_abs_notional
                ),
            ));
            return;
        }

        mutations.push(Mutation::new(|| {}, || {}));
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

impl CheckPreTradeStartPolicy for SharedPnlPolicy {
    fn name(&self) -> &'static str {
        self.inner.name()
    }

    fn check_pre_trade_start(&self, order: &Order) -> Result<(), Reject> {
        self.inner.check_pre_trade_start(order)
    }

    fn apply_execution_report(&self, report: &ExecutionReport) -> bool {
        self.inner.apply_execution_report(report)
    }
}

fn order_aapl_usd(price: &str, quantity: &str) -> Order {
    order_aapl_usd_with_side(price, quantity, Side::Buy)
}

fn order_aapl_usd_with_side(price: &str, quantity: &str, side: Side) -> Order {
    Order {
        instrument: Instrument::new(
            Asset::new("AAPL").expect("asset code must be valid"),
            Asset::new("USD").expect("asset code must be valid"),
        ),
        side,
        quantity: Quantity::from_str(quantity).expect("quantity literal must be valid"),
        price: Price::from_str(price).expect("price literal must be valid"),
    }
}

fn execution_report_spx_usd(pnl_value: &str) -> ExecutionReport {
    ExecutionReport {
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
