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

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

use openpit::param::{AccountId, Asset, Quantity, Side, TradeAmount};
use openpit::pretrade::policies::{
    RateLimit, RateLimitAccountBarrier, RateLimitPolicy, RateLimitSettings,
};
use openpit::pretrade::{PreTradeContext, PreTradePolicy, Rejects};
use openpit::storage::FullLocking;
use openpit::{Engine, FullSync, Instrument, OrderOperation, PolicyGroupId};

type TestPolicy = RateLimitPolicy<FullLocking>;

const TOTAL_THREADS: usize = 8;
const PER_THREAD: usize = 1_000;

struct StartBarrierPolicy {
    barrier: Arc<TimedBarrier>,
}

struct DecisionCountingRateLimitPolicy {
    inner: TestPolicy,
    accepted: Arc<AtomicUsize>,
    rejected: Arc<AtomicUsize>,
}

struct TimedBarrier {
    arrivals: Mutex<usize>,
    ready: Condvar,
    participants: usize,
}

impl TimedBarrier {
    fn new(participants: usize) -> Self {
        Self {
            arrivals: Mutex::new(0),
            ready: Condvar::new(),
            participants,
        }
    }

    fn wait(&self) {
        let mut arrivals = self
            .arrivals
            .lock()
            .expect("barrier mutex must not be poisoned");
        *arrivals += 1;
        if *arrivals == self.participants {
            self.ready.notify_all();
            return;
        }
        let (arrivals, _) = self
            .ready
            .wait_timeout_while(arrivals, Duration::from_secs(5), |arrivals| {
                *arrivals < self.participants
            })
            .expect("barrier mutex must not be poisoned");
        assert_eq!(
            *arrivals, self.participants,
            "drop-copy start barrier timed out"
        );
    }
}

impl PreTradePolicy<OrderOperation, (), (), FullSync> for StartBarrierPolicy {
    fn name(&self) -> &str {
        "start_barrier"
    }

    fn check_pre_trade_start(
        &self,
        _ctx: &PreTradeContext<FullLocking>,
        _order: &OrderOperation,
    ) -> Result<(), Rejects> {
        self.barrier.wait();
        Ok(())
    }
}

impl PreTradePolicy<OrderOperation, (), (), FullSync> for DecisionCountingRateLimitPolicy {
    fn name(&self) -> &str {
        <TestPolicy as PreTradePolicy<OrderOperation, (), (), FullSync>>::name(&self.inner)
    }

    fn policy_group_id(&self) -> PolicyGroupId {
        <TestPolicy as PreTradePolicy<OrderOperation, (), (), FullSync>>::policy_group_id(
            &self.inner,
        )
    }

    fn check_pre_trade_start(
        &self,
        ctx: &PreTradeContext<FullLocking>,
        order: &OrderOperation,
    ) -> Result<(), Rejects> {
        let result =
            <TestPolicy as PreTradePolicy<OrderOperation, (), (), FullSync>>::check_pre_trade_start(
                &self.inner,
                ctx,
                order,
            );
        if result.is_ok() {
            self.accepted.fetch_add(1, Ordering::Relaxed);
        } else {
            self.rejected.fetch_add(1, Ordering::Relaxed);
        }
        result
    }

    // Counters cover the start path only.
    fn check_pre_trade_start_dry_run(
        &self,
        ctx: &PreTradeContext<FullLocking>,
        order: &OrderOperation,
    ) -> Result<(), Rejects> {
        <TestPolicy as PreTradePolicy<OrderOperation, (), (), FullSync>>::check_pre_trade_start_dry_run(
            &self.inner,
            ctx,
            order,
        )
    }
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
    let policy: Arc<TestPolicy> = Arc::new(RateLimitPolicy::<FullLocking>::new(
        RateLimitSettings::new(None, [], account_barriers, [])
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
                    .expect("all calls within per-account limit must pass");
                }
            });
        }
    });

    for tid in 0..TOTAL_THREADS {
        let overflow_order = build_order(AccountId::from_u64(tid as u64));
        assert!(
            <TestPolicy as PreTradePolicy<OrderOperation, (), (), FullSync>>::check_pre_trade_start(
                &policy,
                &PreTradeContext::new(None),
                &overflow_order,
            )
            .is_err(),
            "account {tid}: call after exhausting per-account limit must be rejected"
        );
    }
}

#[test]
fn concurrent_drop_copy_account_limit_decisions_are_exact() {
    let account_id = AccountId::from_u64(7);
    let builder = Engine::builder::<OrderOperation, (), ()>().full_sync();
    let accepted = Arc::new(AtomicUsize::new(0));
    let rejected = Arc::new(AtomicUsize::new(0));
    let rate_limit = DecisionCountingRateLimitPolicy {
        inner: RateLimitPolicy::<FullLocking>::new(
            RateLimitSettings::new(
                None,
                [],
                [RateLimitAccountBarrier {
                    account_id,
                    limit: RateLimit {
                        max_orders: 1,
                        window: Duration::from_secs(60),
                    },
                }],
                [],
            )
            .expect("rate limit settings must be valid"),
            builder.storage_builder(),
        ),
        accepted: Arc::clone(&accepted),
        rejected: Arc::clone(&rejected),
    };
    let engine = Arc::new(
        builder
            .pre_trade(StartBarrierPolicy {
                barrier: Arc::new(TimedBarrier::new(TOTAL_THREADS)),
            })
            .pre_trade(rate_limit)
            .build()
            .expect("engine must build"),
    );

    let blocked_results = thread::scope(|scope| {
        let mut handles = Vec::with_capacity(TOTAL_THREADS);
        for _ in 0..TOTAL_THREADS {
            let engine = Arc::clone(&engine);
            handles.push(scope.spawn(move || {
                let mut operation = engine
                    .apply_drop_copy(build_order(account_id))
                    .expect("ordinary rate-limit rejects must not abort drop-copy");
                let blocked = operation.account_block().is_some();
                operation.commit();
                blocked
            }));
        }
        handles
            .into_iter()
            .map(|handle| handle.join().expect("drop-copy thread must finish"))
            .filter(|blocked| *blocked)
            .count()
    });

    assert_eq!(accepted.load(Ordering::Relaxed), 1);
    assert_eq!(
        rejected.load(Ordering::Relaxed),
        TOTAL_THREADS - 1,
        "all account-limit decisions must be counted exactly"
    );
    assert_eq!(
        blocked_results, 0,
        "order-scoped rejects must not request account blocks"
    );
}
