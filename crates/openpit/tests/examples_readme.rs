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

use std::time::Duration;

use openpit::param::{AccountId, Asset, Fee, Pnl, Price, Quantity, Side, TradeAmount, Volume};
use openpit::pretrade::policies::{
    OrderSizeAssetBarrier, OrderSizeBrokerBarrier, OrderSizeLimit, OrderSizeLimitPolicy,
    OrderSizeLimitSettings, OrderValidationPolicy, PnlBoundsBrokerBarrier,
    PnlBoundsKillSwitchPolicy, PnlBoundsKillSwitchSettings, RateLimit, RateLimitBrokerBarrier,
    RateLimitPolicy, RateLimitPolicyError, RateLimitSettings,
};
use openpit::storage::NoLocking;
use openpit::{Engine, Instrument};
use openpit::{
    ExecutionReportOperation, FinancialImpact, OrderOperation, WithExecutionReportOperation,
    WithFinancialImpact,
};

// Mirrors public examples from:
// - crates/openpit/README.md
// - ../pit.wiki/Getting-Started.md
// If this test changes, update every linked documentation snippet.

#[test]
fn example_readme_configure_builtin_policy() -> Result<(), Box<dyn std::error::Error>> {
    // Source: crates/openpit/README.md - Engine
    // Keep this example in sync with the README.
    let builder = Engine::builder::<OrderOperation, (), ()>().no_sync();
    let policy = RateLimitPolicy::new(
        RateLimitSettings::new(
            Some(RateLimitBrokerBarrier {
                limit: RateLimit {
                    max_orders: 100,
                    window: Duration::from_secs(1),
                },
            }),
            [],
            [],
            [],
        )?,
        builder.storage_builder(),
    );
    let engine = builder.pre_trade(policy).build()?;

    engine.configure().rate_limit::<RateLimitPolicyError>(
        RateLimitPolicy::<NoLocking>::NAME,
        |settings| {
            settings.set_broker(Some(RateLimitBrokerBarrier {
                limit: RateLimit {
                    max_orders: 10,
                    window: Duration::from_secs(1),
                },
            }))
        },
    )?;
    Ok(())
}

#[test]
fn example_readme_quickstart() -> Result<(), Box<dyn std::error::Error>> {
    // Source: crates/openpit/README.md - Usage
    // Shared with: pit.wiki/Getting-Started.md
    // Keep README and wiki versions of this example in sync.
    let usd = Asset::new("USD")?;

    // 1. Build the engine builder.
    type Report = WithExecutionReportOperation<WithFinancialImpact<()>>;
    let builder = Engine::builder::<OrderOperation, Report, ()>().no_sync();

    // 2. Configure policies.
    let pnl_policy = PnlBoundsKillSwitchPolicy::new(
        PnlBoundsKillSwitchSettings::new(
            [PnlBoundsBrokerBarrier {
                settlement_asset: usd.clone(),
                lower_bound: Some(Pnl::from_str("-1000")?),
                upper_bound: None,
            }],
            [],
        )?,
        builder.storage_builder(),
    );

    let rate_limit_policy = RateLimitPolicy::new(
        RateLimitSettings::new(
            Some(RateLimitBrokerBarrier {
                limit: RateLimit {
                    max_orders: 100,
                    window: Duration::from_secs(1),
                },
            }),
            [],
            [],
            [],
        )?,
        builder.storage_builder(),
    );

    // 3. Build the engine (one time at the platform initialization).
    let engine = builder
        .pre_trade(OrderValidationPolicy::new())
        .pre_trade(pnl_policy)
        .pre_trade(rate_limit_policy)
        .pre_trade(OrderSizeLimitPolicy::<NoLocking>::new(
            OrderSizeLimitSettings::new(
                Some(OrderSizeBrokerBarrier {
                    limit: OrderSizeLimit {
                        max_quantity: Quantity::from_str("500")?,
                        max_notional: Volume::from_str("100000")?,
                    },
                }),
                [OrderSizeAssetBarrier {
                    limit: OrderSizeLimit {
                        max_quantity: Quantity::from_str("500")?,
                        max_notional: Volume::from_str("100000")?,
                    },
                    settlement_asset: usd.clone(),
                }],
                [],
            )?,
        ))
        .build()?;

    // 3. Check an order.
    let order = OrderOperation {
        instrument: Instrument::new(Asset::new("AAPL")?, usd.clone()),
        account_id: AccountId::from_u64(99224416),
        side: Side::Buy,
        trade_amount: TradeAmount::Quantity(Quantity::from_f64(100.0)?),
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
    let mut reservation = request.execute()?;

    // Optional shortcut for the same two-stage flow:
    // let reservation = engine.execute_pre_trade(order)?;

    // 6. If the request is successfully sent to the venue, it must be committed.
    // The rollback must be called otherwise to revert all performed reservations.
    reservation.commit();

    // 7. The order goes to the venue and returns with an execution report.
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

    // 8. After each execution report is applied, the system may report that it has
    // been determined in advance that all subsequent requests will be rejected if
    // the account status does not change.
    assert!(result.account_blocks.is_empty());
    Ok(())
}
