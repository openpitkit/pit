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

use std::time::Duration;

use openpit::param::{AccountId, Asset, Fee, Pnl, Price, Quantity, Side, TradeAmount, Volume};
use openpit::pretrade::policies::{
    OrderSizeLimit, OrderSizeLimitPolicy, OrderValidationPolicy, PnlKillSwitchPolicy,
    RateLimitPolicy,
};
use openpit::{Engine, Instrument};
use openpit::{
    ExecutionReportOperation, FinancialImpact, OrderOperation, WithExecutionReportOperation,
    WithFinancialImpact,
};

#[test]
fn example_readme_quickstart() -> Result<(), Box<dyn std::error::Error>> {
    // Source: crates/openpit/README.md — Usage
    let usd = Asset::new("USD")?;

    // 1. Configure policies.
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

    // 2. Build the engine (one time at the platform initialization).
    type Report = WithExecutionReportOperation<WithFinancialImpact<()>>;
    let engine = Engine::<OrderOperation, Report>::builder()
        .check_pre_trade_start_policy(OrderValidationPolicy::new())
        .check_pre_trade_start_policy(pnl_policy)
        .check_pre_trade_start_policy(rate_limit_policy)
        .check_pre_trade_start_policy(size_policy)
        .build()?;

    // 3. Check an order.
    let order = OrderOperation {
        instrument: Instrument::new(Asset::new("AAPL")?, usd.clone()),
        account_id: AccountId::from_u64(99224416),
        side: Side::Buy,
        trade_amount: TradeAmount::Quantity(Quantity::from_str("100")?),
        price: Some(Price::from_str("185")?),
    };

    let request = engine.start_pre_trade(order)?;

    // 4. Quick, lightweight checks, such as fat-finger scope or enabled killswitch,
    // were performed during pre-trade request creation. The system state has not
    // yet changed, except in cases where each request, even rejected ones, must be
    // considered (for example, to prevent frequent transfers). Before the
    // heavy-duty checks, other work on the request can be performed simply by
    // holding the request object.

    // 5. Real pre-trade and risk control.
    let reservation = request.execute()?;

    // 6. If the request is successfully sent to the venue, it must be committed.
    // The rollback must be called otherwise to revert all performed reservations.
    reservation.commit();

    // 5. The order goes to the venue and returns with an execution report.
    let report = WithExecutionReportOperation {
        inner: WithFinancialImpact {
            inner: (),
            financial_impact: FinancialImpact {
                pnl: Pnl::from_str("-50")?,
                fee: Fee::from_str("3.4")?,
            },
        },
        operation: ExecutionReportOperation {
            instrument: Instrument::new(Asset::new("AAPL")?, usd),
            account_id: AccountId::from_u64(99224416),
            side: Side::Buy,
        },
    };

    let result = engine.apply_execution_report(&report);

    // 6. After each execution report is applied, the system may report that it has
    // been determined in advance that all subsequent requests will be rejected if
    // the account status does not change.
    assert!(!result.kill_switch_triggered);
    Ok(())
}
