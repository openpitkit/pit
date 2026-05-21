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
use openpit::pretrade::policies::{RateLimit, RateLimitAccountBarrier, RateLimitPolicy};
use openpit::pretrade::{PreTradeContext, PreTradePolicy};
use openpit::storage::FullLocking;
use openpit::{Engine, Instrument, OrderOperation};

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

// Verifies that per-account VecDeque sliding-window counters in
// RateLimitPolicy<FullLocking> track each account independently and without
// data loss when written concurrently from multiple threads.
//
// Each thread owns one AccountId and submits PER_THREAD calls.  The per-account
// limit equals PER_THREAD exactly, so every call must pass (count 1..=1000
// each <= limit).  A final extra call per account must be rejected (count 1001
// > limit), confirming that the VecDeque reached the correct length without
// losing or duplicating entries under concurrent access.
#[test]
fn rate_limit_full_sync_per_account_counter_isolated_under_concurrent_load() {
    let account_barriers: Vec<RateLimitAccountBarrier> = (0..TOTAL_THREADS as u64)
        .map(|id| RateLimitAccountBarrier {
            account_id: AccountId::from_u64(id),
            limit: RateLimit {
                max_orders: PER_THREAD,
                window: Duration::from_secs(60),
            },
        })
        .collect();

    let builder = Engine::builder::<OrderOperation, (), ()>().full_sync();
    let policy: Arc<TestPolicy> = Arc::new(
        RateLimitPolicy::<FullLocking>::new(
            None,
            [],
            account_barriers,
            [],
            builder.storage_builder(),
        )
        .expect("policy must be configured"),
    );

    thread::scope(|s| {
        for tid in 0..TOTAL_THREADS {
            let policy = Arc::clone(&policy);
            s.spawn(move || {
                let order = build_order(AccountId::from_u64(tid as u64));
                for _ in 0..PER_THREAD {
                    <TestPolicy as PreTradePolicy<OrderOperation, ()>>::check_pre_trade_start(
                        &policy,
                        &PreTradeContext::new(),
                        &order,
                    )
                    .expect("all calls within per-account limit must pass");
                }
            });
        }
    });

    for tid in 0..TOTAL_THREADS {
        let overflow_order = build_order(AccountId::from_u64(tid as u64));
        assert!(
            <TestPolicy as PreTradePolicy<OrderOperation, ()>>::check_pre_trade_start(
                &policy,
                &PreTradeContext::new(),
                &overflow_order,
            )
            .is_err(),
            "account {tid}: call after exhausting per-account limit must be rejected"
        );
    }
}
