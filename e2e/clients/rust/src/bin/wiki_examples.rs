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

use std::ops::Deref;
use std::time::Duration;

use openpit::param::{
    AccountId, Asset, Fee, Leverage, Pnl, PositionSide, Price, Quantity, Side, TradeAmount,
    Volume,
};
use openpit::pretrade::policies::{
    OrderSizeLimit, OrderSizeLimitPolicy, OrderValidationPolicy, PnlKillSwitchPolicy,
    RateLimitPolicy,
};
use openpit::pretrade::{
    CheckPreTradeStartPolicy, Context, Mutations, Policy, Reject, RejectCode, RejectScope,
};
use openpit::{
    Engine, ExecutionReportOperation, FinancialImpact, HasOrderPrice, HasTradeAmount, Instrument,
    OrderOperation, RequestFields, WithExecutionReportOperation, WithFinancialImpact,
};

type PitExecutionReport = WithExecutionReportOperation<WithFinancialImpact<()>>;

struct NotionalCapPolicy {
    max_abs_notional: Volume,
}

impl<O, R> Policy<O, R> for NotionalCapPolicy
where
    O: HasTradeAmount + HasOrderPrice,
{
    fn name(&self) -> &'static str {
        "NotionalCapPolicy"
    }

    fn perform_pre_trade_check(
        &self,
        ctx: &Context<'_, O>,
        _mutations: &mut Mutations,
        rejects: &mut Vec<Reject>,
    ) {
        let order = ctx.order();
        let trade_amount = match order.trade_amount() {
            Ok(trade_amount) => trade_amount,
            Err(error) => {
                rejects.push(Reject::new(
                    <Self as Policy<O, R>>::name(self),
                    RejectScope::Order,
                    RejectCode::MissingRequiredField,
                    "required order field missing",
                    error.to_string(),
                ));
                return;
            }
        };
        let price = match order.price() {
            Ok(price) => price,
            Err(error) => {
                rejects.push(Reject::new(
                    <Self as Policy<O, R>>::name(self),
                    RejectScope::Order,
                    RejectCode::MissingRequiredField,
                    "required order field missing",
                    error.to_string(),
                ));
                return;
            }
        };
        let requested_notional = match (trade_amount, price) {
            (TradeAmount::Volume(volume), _) => volume,
            (TradeAmount::Quantity(quantity), Some(price)) => {
                match price.calculate_volume(quantity) {
                    Ok(v) => v,
                    Err(_) => {
                        rejects.push(Reject::new(
                            <Self as Policy<O, R>>::name(self),
                            RejectScope::Order,
                            RejectCode::OrderValueCalculationFailed,
                            "order value calculation failed",
                            "price and quantity could not be used to evaluate notional",
                        ));
                        return;
                    }
                }
            }
            (TradeAmount::Quantity(_), None) => {
                rejects.push(Reject::new(
                    <Self as Policy<O, R>>::name(self),
                    RejectScope::Order,
                    RejectCode::OrderValueCalculationFailed,
                    "order value calculation failed",
                    "price not provided for evaluating cash flow/notional/volume",
                ));
                return;
            }
            _ => {
                rejects.push(Reject::new(
                    <Self as Policy<O, R>>::name(self),
                    RejectScope::Order,
                    RejectCode::UnsupportedOrderType,
                    "unsupported order type",
                    "custom trade amount variant is not supported by this policy",
                ));
                return;
            }
        };

        if requested_notional > self.max_abs_notional {
            rejects.push(Reject::new(
                <Self as Policy<O, R>>::name(self),
                RejectScope::Order,
                RejectCode::RiskLimitExceeded,
                "strategy cap exceeded",
                format!(
                    "requested notional {}, max allowed: {}",
                    requested_notional, self.max_abs_notional
                ),
            ));
        }
    }

    fn apply_execution_report(&self, _report: &R) -> bool {
        false
    }
}

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

trait HasStrategyTag {
    fn strategy_tag(&self) -> &'static str;
}

impl HasStrategyTag for StrategyOrder {
    fn strategy_tag(&self) -> &'static str {
        self.strategy_tag
    }
}

struct StrategyTagPolicy;

impl<O, R> Policy<O, R> for StrategyTagPolicy
where
    O: HasStrategyTag + HasTradeAmount,
{
    fn name(&self) -> &'static str {
        "StrategyTagPolicy"
    }

    fn perform_pre_trade_check(
        &self,
        ctx: &Context<'_, O>,
        _mutations: &mut Mutations,
        rejects: &mut Vec<Reject>,
    ) {
        if ctx.order().strategy_tag() == "blocked" {
            rejects.push(Reject::new(
                <Self as Policy<O, R>>::name(self),
                RejectScope::Order,
                RejectCode::ComplianceRestriction,
                "strategy blocked",
                "project strategy tag blocked",
            ));
        }
    }

    fn apply_execution_report(&self, report: &R) -> bool {
        let _ = report;
        false
    }
}

struct StrategyTagStartPolicy;

impl<O, R> CheckPreTradeStartPolicy<O, R> for StrategyTagStartPolicy
where
    O: HasStrategyTag,
{
    fn name(&self) -> &'static str {
        "StrategyTagStartPolicy"
    }

    fn check_pre_trade_start(&self, order: &O) -> Result<(), Reject> {
        if order.strategy_tag() == "blocked" {
            return Err(Reject::new(
                <Self as CheckPreTradeStartPolicy<O, R>>::name(self),
                RejectScope::Order,
                RejectCode::ComplianceRestriction,
                "strategy blocked",
                "project strategy tag blocked",
            ));
        }
        Ok(())
    }

    fn apply_execution_report(&self, report: &R) -> bool {
        let _ = report;
        false
    }
}

#[derive(RequestFields)]
struct DerivedOrder<T> {
    inner: T,
    #[openpit(HasInstrument, HasAccountId, HasTradeAmount, HasOrderPrice, HasSide)]
    operation: OrderOperation,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    run_domain_types_examples()?;
    run_getting_started_examples()?;
    run_pre_trade_pipeline_examples()?;
    run_notional_cap_policy_example()?;
    run_strategy_tag_policy_example()?;
    run_request_fields_example()?;
    Ok(())
}

fn run_domain_types_examples() -> Result<(), Box<dyn std::error::Error>> {
    let asset = Asset::new("AAPL")?;
    let quantity = Quantity::from_str("10.5")?;
    let price = Price::from_str("185")?;
    let pnl = Pnl::from_str("-12.5")?;

    assert_eq!(asset.as_ref(), "AAPL");
    assert_eq!(quantity.to_string(), "10.5");
    assert_eq!(price.to_string(), "185");
    assert_eq!(pnl.to_string(), "-12.5");

    assert_eq!(Side::Buy.opposite(), Side::Sell);
    assert_eq!(Side::Sell.sign(), -1);
    assert_eq!(PositionSide::Long.opposite(), PositionSide::Short);

    let from_multiplier = Leverage::from_u16(100)?;
    let from_float = Leverage::from_f64(100.5)?;
    assert_eq!(from_multiplier.value(), 100.0);
    assert_eq!(from_float.value(), 100.5);
    Ok(())
}

fn run_getting_started_examples() -> Result<(), Box<dyn std::error::Error>> {
    let usd = Asset::new("USD")?;

    let pnl_policy = PnlKillSwitchPolicy::new((usd.clone(), Pnl::from_str("1000")?), [])?;
    let rate_limit_policy = RateLimitPolicy::new(100, Duration::from_secs(1));
    let size_policy = OrderSizeLimitPolicy::new(
        OrderSizeLimit {
            settlement_asset: usd.clone(),
            max_quantity: Quantity::from_str("500")?,
            max_notional: Volume::from_str("100000")?,
        },
        [],
    );

    let engine = Engine::<OrderOperation, PitExecutionReport>::builder()
        .check_pre_trade_start_policy(OrderValidationPolicy::new())
        .check_pre_trade_start_policy(pnl_policy)
        .check_pre_trade_start_policy(rate_limit_policy)
        .check_pre_trade_start_policy(size_policy)
        .build()?;

    let order = aapl_usd_order("100", "185");
    let request = match engine.start_pre_trade(order) {
        Ok(request) => request,
        Err(reject) => panic!(
            "unexpected start-stage reject: {} [{}] {} ({})",
            reject.policy, reject.code, reject.reason, reject.details
        ),
    };

    let reservation = match request.execute() {
        Ok(reservation) => reservation,
        Err(rejects) => panic!("unexpected main-stage rejects: {}", rejects.len()),
    };
    reservation.commit();

    let report = aapl_usd_report("-50", "3");
    let result = engine.apply_execution_report(&report);
    assert!(!result.kill_switch_triggered);
    Ok(())
}

fn run_pre_trade_pipeline_examples() -> Result<(), Box<dyn std::error::Error>> {
    let start_engine = Engine::<OrderOperation, PitExecutionReport>::builder()
        .check_pre_trade_start_policy(OrderValidationPolicy::new())
        .build()?;

    let reject = match start_engine.start_pre_trade(aapl_usd_order("0", "185")) {
        Ok(_) => panic!("start stage must reject zero quantity"),
        Err(reject) => reject,
    };
    assert_eq!(reject.code, RejectCode::InvalidFieldValue);

    let main_engine = Engine::<OrderOperation, PitExecutionReport>::builder()
        .pre_trade_policy(NotionalCapPolicy {
            max_abs_notional: Volume::from_str("1000")?,
        })
        .build()?;

    let request = main_engine.start_pre_trade(aapl_usd_order("10", "25"))?;
    match request.execute() {
        Ok(reservation) => reservation.commit(),
        Err(rejects) => panic!("main stage must pass: {}", rejects.len()),
    }

    let blocked_request = main_engine.start_pre_trade(aapl_usd_order("100", "25"))?;
    let rejects = match blocked_request.execute() {
        Ok(_) => panic!("main stage must reject"),
        Err(rejects) => rejects,
    };
    assert_eq!(rejects[0].code, RejectCode::RiskLimitExceeded);
    Ok(())
}

fn run_notional_cap_policy_example() -> Result<(), Box<dyn std::error::Error>> {
    let engine = Engine::<OrderOperation, PitExecutionReport>::builder()
        .pre_trade_policy(NotionalCapPolicy {
            max_abs_notional: Volume::from_str("1000")?,
        })
        .build()?;

    let request = engine.start_pre_trade(aapl_usd_order("10", "25"))?;
    request.execute()?.commit();

    let request = engine.start_pre_trade(aapl_usd_order("100", "25"))?;
    let rejects = match request.execute() {
        Ok(_) => panic!("main stage must reject"),
        Err(rejects) => rejects,
    };
    assert_eq!(rejects[0].code, RejectCode::RiskLimitExceeded);
    Ok(())
}

fn run_strategy_tag_policy_example() -> Result<(), Box<dyn std::error::Error>> {
    let engine = Engine::<StrategyOrder, StrategyExecutionReport>::builder()
        .pre_trade_policy(StrategyTagPolicy)
        .build()?;

    let request = engine.start_pre_trade(StrategyOrder {
        base: aapl_usd_order("10", "25"),
        strategy_tag: "allowed",
    })?;
    let reservation = request.execute()?;
    reservation.commit();

    let report = StrategyExecutionReport {
        base: aapl_usd_report("5", "1"),
        report_tag: "fill-1",
    };
    assert_eq!(report.report_tag, "fill-1");
    let post_trade = engine.apply_execution_report(&report);
    assert!(!post_trade.kill_switch_triggered);

    let blocked_engine = Engine::<StrategyOrder, StrategyExecutionReport>::builder()
        .check_pre_trade_start_policy(StrategyTagStartPolicy)
        .build()?;
    let reject = match blocked_engine.start_pre_trade(StrategyOrder {
        base: aapl_usd_order("10", "25"),
        strategy_tag: "blocked",
    }) {
        Ok(_) => panic!("start stage must reject"),
        Err(reject) => reject,
    };
    assert_eq!(reject.code, RejectCode::ComplianceRestriction);
    Ok(())
}

fn run_request_fields_example() -> Result<(), Box<dyn std::error::Error>> {
    let engine = Engine::<DerivedOrder<()>, PitExecutionReport>::builder()
        .pre_trade_policy(NotionalCapPolicy {
            max_abs_notional: Volume::from_str("300")?,
        })
        .build()?;

    let request = engine.start_pre_trade(DerivedOrder {
        inner: (),
        operation: aapl_usd_order("10", "25"),
    })?;
    let reservation = request.execute()?;
    reservation.rollback();
    Ok(())
}

fn aapl_usd_order(quantity: &str, price: &str) -> OrderOperation {
    OrderOperation {
        instrument: Instrument::new(
            Asset::new("AAPL").expect("AAPL must be valid"),
            Asset::new("USD").expect("USD must be valid"),
        ),
        account_id: AccountId::from_u64(99224416),
        side: Side::Buy,
        trade_amount: TradeAmount::Quantity(
            Quantity::from_str(quantity).expect("quantity must be valid"),
        ),
        price: Some(Price::from_str(price).expect("price must be valid")),
    }
}

fn aapl_usd_report(pnl: &str, fee: &str) -> PitExecutionReport {
    PitExecutionReport {
        inner: WithFinancialImpact {
            inner: (),
            financial_impact: FinancialImpact {
                pnl: Pnl::from_str(pnl).expect("pnl must be valid"),
                fee: Fee::from_str(fee).expect("fee must be valid"),
            },
        },
        operation: ExecutionReportOperation {
            instrument: Instrument::new(
                Asset::new("AAPL").expect("AAPL must be valid"),
                Asset::new("USD").expect("USD must be valid"),
            ),
            account_id: AccountId::from_u64(99224416),
            side: Side::Buy,
        },
    }
}
