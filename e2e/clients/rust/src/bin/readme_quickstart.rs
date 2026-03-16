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
// Source: crates/openpit/README.md

use std::time::Duration;

use openpit::param::{Asset, Fee, Pnl, Price, Quantity, Side, Volume};
use openpit::pretrade::policies::OrderValidationPolicy;
use openpit::pretrade::policies::PnlKillSwitchPolicy;
use openpit::pretrade::policies::RateLimitPolicy;
use openpit::pretrade::policies::{OrderSizeLimit, OrderSizeLimitPolicy};
use openpit::{Engine, ExecutionReportRecord, Instrument, OrderRecord, TradeAmount};

fn main() {
    let usd = Asset::new("USD").expect("asset code must be valid");

    let pnl = PnlKillSwitchPolicy::new(
        (
            usd.clone(),
            Pnl::from_str("1000").expect("valid pnl literal"),
        ),
        [],
    )
    .expect("policy config must be valid");

    let rate_limit = RateLimitPolicy::new(100, Duration::from_secs(1));

    let size = OrderSizeLimitPolicy::new(
        OrderSizeLimit {
            settlement_asset: usd.clone(),
            max_quantity: Quantity::from_str("500").expect("valid quantity literal"),
            max_notional: Volume::from_str("100000").expect("valid volume literal"),
        },
        [],
    );

    let engine = Engine::builder()
        .check_pre_trade_start_policy(OrderValidationPolicy::new())
        .check_pre_trade_start_policy(pnl)
        .check_pre_trade_start_policy(rate_limit)
        .check_pre_trade_start_policy(size)
        .build()
        .expect("engine config must be valid");

    let order = OrderRecord {
        instrument: Instrument::new(
            Asset::new("AAPL").expect("asset code must be valid"),
            usd.clone(),
        ),
        side: Side::Buy,
        trade_amount: Some(TradeAmount::Quantity(
            Quantity::from_f64(100.0).expect("valid quantity value"),
        )),
        price: Some(Price::from_f64(185.0).expect("valid price value")),
    };

    let request = engine
        .start_pre_trade(order)
        .expect("start-stage checks must pass");

    let reservation = request.execute().expect("main-stage checks must pass");
    reservation.commit();

    let report = ExecutionReportRecord {
        instrument: Instrument::new(Asset::new("AAPL").expect("asset code must be valid"), usd),
        pnl: Pnl::from_f64(-50.0).expect("valid pnl value"),
        fee: Fee::from_f64(3.4).expect("valid fee value"),
    };

    let result = engine.apply_execution_report(&report);
    assert!(!result.kill_switch_triggered);
}
