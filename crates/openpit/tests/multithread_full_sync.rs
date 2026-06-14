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

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use openpit::param::{AccountId, Asset, Quantity, Side, TradeAmount};
use openpit::pretrade::policies::{
    RateLimit, RateLimitBrokerBarrier, RateLimitPolicy, RateLimitSettings,
};
use openpit::pretrade::{PreTradeContext, PreTradePolicy};
use openpit::storage::FullLocking;
use openpit::{Engine, FullSync, Instrument, OrderOperation};

type TestPolicy = RateLimitPolicy<FullLocking>;

const TOTAL_THREADS: usize = 8;
const PER_THREAD: usize = 1_000;

fn build_order(account_id: AccountId) -> OrderOperation {
    OrderOperation {
        instrument: Instrument::new(
            Asset::new("AAPL").expect("asset code must be valid"),
            Asset::new("USD").expect("asset code must be valid"),
        ),
        account_id,
        side: Side::Buy,
        trade_amount: TradeAmount::Quantity(
            Quantity::from_str("1").expect("quantity literal must be valid"),
        ),
        price: None,
    }
}

// Verifies that the broker-wide AtomicU64 counter in RateLimitPolicy<FullLocking>
// does not lose increments under concurrent load from multiple threads.
//
// The limit is set to exactly TOTAL_THREADS * PER_THREAD so that all calls
// pass (count 1..=8000 each <= limit), and the very next call is rejected
// (count 8001 > limit), confirming no counter corruption occurred.
#[test]
fn rate_limit_full_sync_broker_counter_not_lost_under_concurrent_load() {
    let total_calls = TOTAL_THREADS * PER_THREAD;
    let builder = Engine::builder::<OrderOperation, (), ()>().full_sync();
    let policy: Arc<TestPolicy> = Arc::new(RateLimitPolicy::<FullLocking>::new(
        RateLimitSettings::new(
            Some(RateLimitBrokerBarrier {
                limit: RateLimit {
                    max_orders: total_calls,
                    window: Duration::from_secs(60),
                },
            }),
            [],
            [],
            [],
        )
        .expect("rate limit settings must be valid"),
        builder.storage_builder(),
    ));

    thread::scope(|s| {
        for tid in 0..TOTAL_THREADS {
            let policy = Arc::clone(&policy);
            s.spawn(move || {
                let order = build_order(AccountId::from_u64(tid as u64));
                for _ in 0..PER_THREAD {
                    <TestPolicy as PreTradePolicy<OrderOperation, (), (), FullSync>>::check_pre_trade_start(
                        &policy,
                        &PreTradeContext::new(None),
                        &order,
                    )
                    .expect("all calls within limit must pass");
                }
            });
        }
    });

    let overflow_order = build_order(AccountId::from_u64(99));
    <TestPolicy as PreTradePolicy<OrderOperation, (), (), FullSync>>::check_pre_trade_start(
        &policy,
        &PreTradeContext::new(None),
        &overflow_order,
    )
    .expect_err("call after exhausting limit must be rejected");
}
