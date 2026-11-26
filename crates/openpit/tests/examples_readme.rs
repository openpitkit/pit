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

use openpit::param::{AccountId, Asset, Price, Quantity, Side, TradeAmount, Volume};
use openpit::pretrade::policies::{
    OrderSizeBrokerBarrier, OrderSizeLimit, OrderSizeLimitPolicy, OrderSizeLimitSettings,
};
use openpit::storage::NoLocking;
use openpit::{Engine, Instrument, OrderOperation};

// Mirrors public examples from:
// - README.md
// - crates/openpit/README.md
// If this test changes, update every linked documentation snippet.

#[test]
fn example_readme_hello_world() -> Result<(), Box<dyn std::error::Error>> {
    // Source: README.md - Quick Start
    // Source: crates/openpit/README.md - Quick Start
    // Keep this example in sync with both READMEs.

    // One fat-finger control, wired once at startup.
    let engine = Engine::builder::<OrderOperation, (), ()>()
        .no_sync()
        .pre_trade(OrderSizeLimitPolicy::<NoLocking>::new(
            OrderSizeLimitSettings::new(
                Some(OrderSizeBrokerBarrier {
                    limit: OrderSizeLimit {
                        max_quantity: Some(Quantity::from_str("500")?),
                        max_notional: Some(Volume::from_str("100000")?),
                    },
                }),
                [],
                [],
            )?,
        ))
        .build()?;

    let order = OrderOperation {
        instrument: Instrument::new(Asset::new("AAPL")?, Asset::new("USD")?),
        account_id: AccountId::from_u64(99224416),
        side: Side::Buy,
        trade_amount: TradeAmount::Quantity(Quantity::from_str("1000")?),
        price: Some(Price::from_str("185")?),
    };

    match engine.execute_pre_trade(order) {
        Ok(mut reservation) if !reservation.reviews().is_empty() => {
            // Leave unsent. After approval, rerun with the order-review reader
            // enabled, confirmed set, and already_held set if a hold was kept.
            reservation.commit();
        }
        Ok(mut reservation) => {
            // The venue accepted the order, so the reserved state stays.
            reservation.commit();
        }
        Err(rejects) => println!("order rejected: {rejects}"),
    }

    Ok(())
}
