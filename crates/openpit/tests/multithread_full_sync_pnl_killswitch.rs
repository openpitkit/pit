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

// Verifies concurrent correctness of PnlBoundsKillSwitchPolicy<FullLocking> under
// two scenarios:
//
// 1. No lost updates: multiple threads interleave apply_execution_report and
//    check_pre_trade_start on the same (account, settlement) pair.  The total
//    accumulated P&L after joining must equal the deterministic sum of all
//    applied deltas, verified by a boundary-crossing probe applied post-join.
//
// 2. Kill-switch monotonicity: once the accumulated P&L breaches the configured
//    bound during concurrent execution, all subsequent check_pre_trade_start
//    calls for the affected (account, settlement) must reject permanently with
//    PnlKillSwitchTriggered.  Accounts not subject to the triggered barrier
//    must continue to accept.

use std::sync::Arc;
use std::thread;

use openpit::param::{AccountId, Asset, Fee, Pnl, Quantity, Side, TradeAmount};
use openpit::pretrade::policies::{
    PnlBoundsAccountAssetBarrier, PnlBoundsBrokerBarrier, PnlBoundsKillSwitchPolicy,
};
use openpit::pretrade::{PreTradeContext, PreTradePolicy, RejectCode};
use openpit::storage::FullLocking;
use openpit::{Engine, Instrument, OrderOperation, RequestFieldAccessError};

type TestPolicy = PnlBoundsKillSwitchPolicy<FullLocking>;

const TOTAL_THREADS: usize = 8;
const PER_THREAD_REPORTS: usize = 5;
const PNL_PER_REPORT: i64 = 10;

struct TestReport {
    instrument: Instrument,
    account_id: AccountId,
    pnl: Pnl,
    fee: Fee,
}

impl openpit::HasInstrument for TestReport {
    fn instrument(&self) -> Result<&Instrument, RequestFieldAccessError> {
        Ok(&self.instrument)
    }
}

impl openpit::HasAccountId for TestReport {
    fn account_id(&self) -> Result<AccountId, RequestFieldAccessError> {
        Ok(self.account_id)
    }
}

impl openpit::HasPnl for TestReport {
    fn pnl(&self) -> Result<Pnl, RequestFieldAccessError> {
        Ok(self.pnl)
    }
}

impl openpit::HasFee for TestReport {
    fn fee(&self) -> Result<Fee, RequestFieldAccessError> {
        Ok(self.fee)
    }
}

fn usd() -> Asset {
    Asset::new("USD").expect("asset code must be valid")
}

fn account(id: u64) -> AccountId {
    AccountId::from_u64(id)
}

fn pnl(s: &str) -> Pnl {
    Pnl::from_str(s).expect("pnl literal must be valid")
}

fn build_order(account_id: AccountId) -> OrderOperation {
    OrderOperation {
        instrument: Instrument::new(Asset::new("AAPL").expect("asset code must be valid"), usd()),
        account_id,
        side: Side::Buy,
        trade_amount: TradeAmount::Quantity(
            Quantity::from_str("1").expect("quantity literal must be valid"),
        ),
        price: None,
    }
}

fn build_report(account_id: AccountId, pnl_val: Pnl) -> TestReport {
    TestReport {
        instrument: Instrument::new(Asset::new("AAPL").expect("asset code must be valid"), usd()),
        account_id,
        pnl: pnl_val,
        fee: Fee::ZERO,
    }
}

fn check_start(
    policy: &TestPolicy,
    order: &OrderOperation,
) -> Result<(), openpit::pretrade::Rejects> {
    <TestPolicy as PreTradePolicy<OrderOperation, TestReport>>::check_pre_trade_start(
        policy,
        &PreTradeContext::new(),
        order,
    )
}

fn apply_report(policy: &TestPolicy, report: &TestReport) -> bool {
    !<TestPolicy as PreTradePolicy<OrderOperation, TestReport>>::apply_execution_report(
        policy, report,
    )
    .is_empty()
}

// Verifies that apply_execution_report does not lose or duplicate updates when
// called concurrently from multiple threads on the same (account, settlement).
//
// Strategy: set the account+asset upper bound equal to the deterministic total
// (TOTAL_THREADS * PER_THREAD_REPORTS * PNL_PER_REPORT = 400).
//
// During the test, FullLocking serializes writes so realized grows from 0 to 400
// in steps of 10; the bound is never exceeded (400 > 400 is false) and all
// intermediate checks pass.
//
// After joining, a probe of +1 is applied.  The breach condition is "realized > 400".
//
// - If no updates were lost: realized = 400 → after probe: 401 > 400 → triggers.
// - If N ≥ 1 updates lost:   realized ≤ 390 → after probe: ≤ 391 ≤ 400 → no trigger.
//
// The probe therefore distinguishes the two cases exactly.
#[test]
fn pnl_full_sync_no_lost_updates_under_concurrent_apply() {
    let expected_total = TOTAL_THREADS * PER_THREAD_REPORTS * (PNL_PER_REPORT as usize);
    // Upper bound equal to the expected total: realized == 400 does NOT trigger;
    // realized == 401 DOES.  This lets all 40 reports pass, then the probe fires.
    let upper_bound_str = expected_total.to_string();

    let builder = Engine::builder::<OrderOperation, TestReport, ()>().full_sync();
    let policy: Arc<TestPolicy> = Arc::new(
        PnlBoundsKillSwitchPolicy::new(
            [PnlBoundsBrokerBarrier {
                settlement_asset: usd(),
                lower_bound: Some(pnl("-100000")),
                upper_bound: Some(pnl("100000")),
            }],
            [PnlBoundsAccountAssetBarrier {
                barrier: PnlBoundsBrokerBarrier {
                    settlement_asset: usd(),
                    lower_bound: Some(pnl("-100000")),
                    upper_bound: Some(
                        Pnl::from_str(&upper_bound_str).expect("upper bound must be valid"),
                    ),
                },
                account_id: account(1),
                initial_pnl: Pnl::ZERO,
            }],
            builder.storage_builder(),
        )
        .expect("policy must be configured"),
    );

    let pnl_delta = Pnl::from_str(&PNL_PER_REPORT.to_string()).expect("pnl delta must be valid");

    thread::scope(|s| {
        for _ in 0..TOTAL_THREADS {
            let policy = Arc::clone(&policy);
            let order = build_order(account(1));
            s.spawn(move || {
                for _ in 0..PER_THREAD_REPORTS {
                    apply_report(&policy, &build_report(account(1), pnl_delta));
                    check_start(&policy, &order)
                        .expect("check must pass: accumulated pnl within bounds during test");
                }
            });
        }
    });

    let probe = build_report(account(1), pnl("1"));
    let triggered = apply_report(&policy, &probe);
    assert!(
        triggered,
        "probe report must breach upper bound {expected_total}: \
         this failure means updates were lost under concurrent load"
    );
}

// Verifies that the kill switch is monotonic: once a breach is detected under
// concurrent apply_execution_report, all subsequent check_pre_trade_start calls
// reject with PnlKillSwitchTriggered, and accounts not subject to the triggered
// barrier continue to accept.
#[test]
fn pnl_full_sync_kill_switch_is_monotonic_and_visible_to_subsequent_checks() {
    // Tight upper bound: guaranteed to be exceeded during concurrent execution.
    // TOTAL_THREADS * PER_THREAD_REPORTS * PNL_PER_REPORT = 400 >> 50.
    let builder = Engine::builder::<OrderOperation, TestReport, ()>().full_sync();
    let policy: Arc<TestPolicy> = Arc::new(
        PnlBoundsKillSwitchPolicy::new(
            [PnlBoundsBrokerBarrier {
                settlement_asset: usd(),
                lower_bound: Some(pnl("-100000")),
                upper_bound: Some(pnl("100000")),
            }],
            [PnlBoundsAccountAssetBarrier {
                barrier: PnlBoundsBrokerBarrier {
                    settlement_asset: usd(),
                    lower_bound: Some(pnl("-100000")),
                    upper_bound: Some(pnl("50")),
                },
                account_id: account(1),
                initial_pnl: Pnl::ZERO,
            }],
            builder.storage_builder(),
        )
        .expect("policy must be configured"),
    );

    let pnl_delta = Pnl::from_str(&PNL_PER_REPORT.to_string()).expect("pnl delta must be valid");

    thread::scope(|s| {
        for _ in 0..TOTAL_THREADS {
            let policy = Arc::clone(&policy);
            s.spawn(move || {
                for _ in 0..PER_THREAD_REPORTS {
                    apply_report(&policy, &build_report(account(1), pnl_delta));
                    // Intermediate checks may succeed (before breach) or fail (after breach);
                    // the result is intentionally not asserted here — the post-join assertion
                    // below verifies the permanent latched state.
                    let _ = check_start(&policy, &build_order(account(1)));
                }
            });
        }
    });

    let reject = check_start(&policy, &build_order(account(1)))
        .expect_err("kill switch must be triggered after concurrent breach");
    assert_eq!(
        reject[0].code,
        RejectCode::PnlKillSwitchTriggered,
        "reject code must be PnlKillSwitchTriggered"
    );

    // Account 2 has no account+asset barrier and its PnL is 0 (no reports applied);
    // the broker barrier is wide, so account 2 must continue to accept.
    check_start(&policy, &build_order(account(2)))
        .expect("account without triggered barrier must still accept");
}
