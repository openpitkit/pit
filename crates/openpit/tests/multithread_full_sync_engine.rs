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

// FullSyncEngine<...> is Send + Sync via Arc<EngineInner>.  These
// tests exercise two FullSync contracts at the engine-handle level:
//
// 1. Sequential cross-thread invocation through Mutex<Engine>, proving the
//    handle can move through worker threads while preserving broker-counter
//    integrity across calls.
// 2. Concurrent shared invocation through Arc<Engine>, proving FullLocking
//    policy storage makes simultaneous start-stage calls safe and does not
//    lose updates.

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use openpit::param::{AccountId, Asset, Quantity, Side, TradeAmount};
use openpit::pretrade::policies::{
    RateLimit, RateLimitAccountBarrier, RateLimitBrokerBarrier, RateLimitPolicy, RateLimitSettings,
};
use openpit::pretrade::{PreTradeRequest, RejectCode, Rejects};
use openpit::{FullSyncEngine, Instrument, OrderOperation};
use parking_lot::Mutex;

type TestEngine = FullSyncEngine<OrderOperation, ()>;

const TOTAL_THREADS: usize = 8;
const PER_THREAD: usize = 1_000;
const TEST_ACCOUNT_ID: u64 = 1;

fn account() -> AccountId {
    AccountId::from_u64(TEST_ACCOUNT_ID)
}

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

fn build_engine(total_calls: usize) -> TestEngine {
    let builder = openpit::Engine::builder().full_sync();
    let policy = RateLimitPolicy::new(
        RateLimitSettings::new(
            Some(RateLimitBrokerBarrier {
                limit: RateLimit {
                    max_orders: total_calls,
                    window: Duration::from_secs(60),
                },
            }),
            [],
            [RateLimitAccountBarrier {
                account_id: account(),
                limit: RateLimit {
                    max_orders: total_calls,
                    window: Duration::from_secs(60),
                },
            }],
            [],
        )
        .expect("rate-limit settings must be valid"),
        builder.storage_builder(),
    );

    builder
        .pre_trade(policy)
        .build()
        .expect("engine must build")
}

fn assert_rate_limit_exceeded(result: Result<PreTradeRequest<OrderOperation>, Rejects>) {
    let Err(rejects) = result else {
        panic!("call after exhausting limit must be rejected");
    };

    assert_eq!(
        rejects[0].code,
        RejectCode::RateLimitExceeded,
        "overflow probe must reject with RateLimitExceeded"
    );
    assert_eq!(
        rejects[0].reason, "rate limit exceeded: broker barrier",
        "overflow probe must reject on the broker barrier first"
    );
}

#[test]
fn engine_full_sync_sequential_cross_thread_no_lost_updates() {
    let total_calls = TOTAL_THREADS * PER_THREAD;
    let engine = Arc::new(Mutex::new(build_engine(total_calls)));

    thread::scope(|s| {
        for _ in 0..TOTAL_THREADS {
            let engine = Arc::clone(&engine);
            s.spawn(move || {
                let order = build_order(account());
                for _ in 0..PER_THREAD {
                    let result = {
                        let guard = engine.lock();
                        guard.start_pre_trade(order.clone())
                    };
                    assert!(result.is_ok(), "all calls within limit must pass");
                }
            });
        }
    });

    assert_rate_limit_exceeded(engine.lock().start_pre_trade(build_order(account())));
}

#[test]
fn engine_full_sync_concurrent_invocation_no_lost_updates() {
    let total_calls = TOTAL_THREADS * PER_THREAD;
    let engine = Arc::new(build_engine(total_calls));

    thread::scope(|s| {
        for _ in 0..TOTAL_THREADS {
            let engine = Arc::clone(&engine);
            s.spawn(move || {
                let order = build_order(account());
                for _ in 0..PER_THREAD {
                    let result = engine.start_pre_trade(order.clone());
                    assert!(result.is_ok(), "all calls within limit must pass");
                }
            });
        }
    });

    assert_rate_limit_exceeded(engine.start_pre_trade(build_order(account())));
}
