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
// Source: pit.wiki/Policies.md

use std::ops::Deref;

use openpit::param::{Asset, Fee, Pnl, Price, Quantity, Side, TradeAmount, Volume};
use openpit::pretrade::{
    CheckPreTradeStartPolicy, Context, Mutations, Policy, Reject, RejectCode, RejectScope,
};
use openpit::{
    Engine, ExecutionReportOperation, FinancialImpact, HasOrderPrice, HasTradeAmount, Instrument,
    OrderOperation, WithExecutionReportOperation, WithFinancialImpact,
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
        let requested_notional = match (order.trade_amount(), order.price()) {
            (TradeAmount::Volume(volume), _) => volume,
            (TradeAmount::Quantity(quantity), Some(price)) => match price
                .calculate_volume(quantity)
            {
                Ok(v) => v,
                Err(_) => {
                    rejects.push(Reject::new(
                        self.name(),
                        RejectScope::Order,
                        RejectCode::OrderValueCalculationFailed,
                        "order value calculation failed",
                        "price and quantity could not be used to evaluate notional",
                    ));
                    return;
                }
            },
            (TradeAmount::Quantity(_), None) => {
                rejects.push(Reject::new(
                    self.name(),
                    RejectScope::Order,
                    RejectCode::OrderValueCalculationFailed,
                    "order value calculation failed",
                    "price not provided for evaluating cash flow/notional/volume",
                ));
                return;
            }
        };

        if requested_notional > self.max_abs_notional {
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
        }
    }

    fn apply_execution_report(&self, _report: &R) -> bool {
        false
    }
}

#[derive(Clone)]
struct StrategyOrder {
    base: OrderOperation,
    // Project field: this field is added by the host application, not by the SDK.
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
    // Project field: this field is added by the host application, not by the SDK.
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
        let order = ctx.order();
        if order.strategy_tag() == "blocked" {
            rejects.push(Reject::new(
                self.name(),
                RejectScope::Order,
                RejectCode::ComplianceRestriction,
                "strategy blocked",
                "project strategy tag blocked",
            ));
        }
    }

    fn apply_execution_report(&self, _report: &R) -> bool {
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
                self.name(),
                RejectScope::Order,
                RejectCode::ComplianceRestriction,
                "strategy blocked",
                "project strategy tag blocked",
            ));
        }
        Ok(())
    }

    fn apply_execution_report(&self, _report: &R) -> bool {
        false
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    run_notional_cap_policy_example()?;
    run_strategy_tag_policy_example()?;
    Ok(())
}

fn run_notional_cap_policy_example() -> Result<(), Box<dyn std::error::Error>> {
    let engine = Engine::<OrderOperation, PitExecutionReport>::builder()
        .pre_trade_policy(NotionalCapPolicy {
            max_abs_notional: Volume::from_str("1000")?,
        })
        .build()?;

    let order_within_limit = OrderOperation {
        instrument: Instrument::new(
            Asset::new("AAPL")?,
            Asset::new("USD")?,
        ),
        side: Side::Buy,
        trade_amount: TradeAmount::Quantity(
            Quantity::from_str("10")?,
        ),
        price: Some(Price::from_str("25")?),
    };

    let request = engine
        .start_pre_trade(order_within_limit)?;
    let reservation = request.execute()?;
    reservation.commit();

    let order_above_limit = OrderOperation {
        instrument: Instrument::new(
            Asset::new("AAPL")?,
            Asset::new("USD")?,
        ),
        side: Side::Buy,
        trade_amount: TradeAmount::Quantity(
            Quantity::from_str("100")?,
        ),
        price: Some(Price::from_str("25")?),
    };

    let request = engine
        .start_pre_trade(order_above_limit)?;
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

    let order = StrategyOrder {
        base: OrderOperation {
            instrument: Instrument::new(
                Asset::new("AAPL")?,
                Asset::new("USD")?,
            ),
            side: Side::Buy,
            trade_amount: TradeAmount::Quantity(
                Quantity::from_str("10")?,
            ),
            price: Some(Price::from_str("25")?),
        },
        // Set this to "blocked" to trigger the same policy reject path as in Python.
        strategy_tag: "allowed",
    };

    let request = engine
        .start_pre_trade(order)?;
    let reservation = request.execute()?;
    reservation.commit();

    let report = StrategyExecutionReport {
        base: PitExecutionReport {
            inner: WithFinancialImpact {
                inner: (),
                financial_impact: FinancialImpact {
                    pnl: Pnl::from_str("5")?,
                    fee: Fee::from_str("1")?,
                },
            },
            operation: ExecutionReportOperation {
                instrument: Instrument::new(
                    Asset::new("AAPL")?,
                    Asset::new("USD")?,
                ),
                side: Side::Buy,
            },
        },
        report_tag: "fill-1",
    };
    let post_trade = engine.apply_execution_report(&report);
    assert!(!post_trade.kill_switch_triggered);

    let blocked_engine = Engine::<StrategyOrder, StrategyExecutionReport>::builder()
        .check_pre_trade_start_policy(StrategyTagStartPolicy)
        .build()?;
    let blocked_order = StrategyOrder {
        base: OrderOperation {
            instrument: Instrument::new(
                Asset::new("AAPL")?,
                Asset::new("USD")?,
            ),
            side: Side::Buy,
            trade_amount: TradeAmount::Quantity(
                Quantity::from_str("10")?,
            ),
            price: Some(Price::from_str("25")?),
        },
        strategy_tag: "blocked",
    };
    let reject = match blocked_engine.start_pre_trade(blocked_order) {
        Ok(_) => panic!("start stage must reject"),
        Err(reject) => reject,
    };
    assert_eq!(reject.code, RejectCode::ComplianceRestriction);
    Ok(())
}
