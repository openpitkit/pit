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

//! Review dispatch for realized P&L and every unrealized/total axis.

use super::*;
use crate::{HasOrderPrice, HasTradeAmount, PnlHaltReason, PnlState};
use std::sync::atomic::AtomicUsize;

/// Order without `HasConfirmed` or `HasAlreadyHeld`, proving Spot Funds needs
/// neither bound.
/// `OrderOperation` implements both, so the wrapper is required.
#[derive(Clone)]
struct ProbeOrder {
    inner: OrderOperation,
}

impl HasInstrument for ProbeOrder {
    fn instrument(&self) -> Result<&Instrument, RequestFieldAccessError> {
        self.inner.instrument()
    }
}

impl HasAccountId for ProbeOrder {
    fn account_id(&self) -> Result<AccountId, RequestFieldAccessError> {
        self.inner.account_id()
    }
}

impl HasSide for ProbeOrder {
    fn side(&self) -> Result<Side, RequestFieldAccessError> {
        self.inner.side()
    }
}

impl HasTradeAmount for ProbeOrder {
    fn trade_amount(&self) -> Result<TradeAmount, RequestFieldAccessError> {
        self.inner.trade_amount()
    }
}

impl HasOrderPrice for ProbeOrder {
    fn price(&self) -> Result<Option<Price>, RequestFieldAccessError> {
        self.inner.price()
    }
}

fn order() -> TestOrder {
    account_pnl_probe_order(account(1))
}

type ReviewEngine = crate::FullSyncEngine<TestOrder, TestReport, AccountPnlAdjustment>;

#[test]
fn review_pnl_no_account_barrier_leaves_currency_unresolved() {
    type SeenSnapshot =
        Arc<std::sync::Mutex<Option<crate::OperationSnapshot<crate::storage::FullLocking>>>>;
    struct CaptureSnapshot(SeenSnapshot);
    impl PreTradePolicy<ProbeOrder, TestReport, AccountPnlAdjustment, FullSync> for CaptureSnapshot {
        fn name(&self) -> &str {
            "capture-snapshot"
        }

        fn perform_pre_trade_check(
            &self,
            ctx: &PreTradeContext<crate::storage::FullLocking>,
            _: &ProbeOrder,
            _: &mut Mutations,
        ) -> Result<Option<PolicyPreTradeResult>, Rejects> {
            *self.0.lock().unwrap() = Some(ctx.operation_snapshot());
            Ok(None)
        }
    }

    for axis in axes() {
        for mode in [
            None,
            Some(RuleMode::Reject),
            Some(RuleMode::ReviewWithHold),
            Some(RuleMode::ReviewWithoutHold),
        ] {
            for (dry_run, drop_copy) in [(false, false), (true, false), (false, true)] {
                let mut configured = settings(0);
                if let Some(mode) = mode {
                    let entries = [SpotFundsPnlBoundsAccountBarrier {
                        account_id: account(2),
                        barrier: SpotFundsPnlBoundsBarrier {
                            mode,
                            ..bounds("-10")
                        },
                    }];
                    match axis {
                        Some(axis) => configured.set_pnl_axis_account_barriers(axis, entries),
                        None => configured.set_pnl_account_barriers(entries),
                    }
                    .unwrap();
                }
                let builder =
                    crate::Engine::builder::<ProbeOrder, TestReport, AccountPnlAdjustment>()
                        .full_sync();
                let policy = TestPolicy::new(configured, None, builder.storage_builder());
                seed(&policy, account(1), asset("USD"), "1000");
                let seen: SeenSnapshot = Arc::new(std::sync::Mutex::new(None));
                let engine = builder
                    .pre_trade(policy)
                    .pre_trade(CaptureSnapshot(seen.clone()))
                    .build()
                    .unwrap();
                let accounts = engine.accounts();
                accounts.register_group(&[account(1)], group(7)).unwrap();
                accounts.set_group_currency(group(7), asset("USD"));
                let request = ProbeOrder { inner: order() };
                if dry_run {
                    assert!(engine.execute_pre_trade_dry_run(request.clone()).is_pass());
                } else if drop_copy {
                    engine.apply_drop_copy(request.clone()).unwrap().commit();
                } else {
                    engine.execute_pre_trade(request.clone()).unwrap().commit();
                }

                accounts.set_group_currency(group(7), asset("EUR"));
                let snapshot = seen.lock().unwrap().take().unwrap();
                assert_eq!(
                    snapshot.account_currency(),
                    Some(asset("EUR")),
                    "no account barrier must leave currency unresolved: \
                     {axis:?}, {mode:?}, dry_run={dry_run}, drop_copy={drop_copy}"
                );
            }
        }
    }
}

#[test]
fn review_pnl_each_axis_keeps_reject_rules_and_ignores_other_accounts() {
    for axis in axes() {
        let engine = fixture(
            axis,
            &[(RuleMode::Reject, "-10"), (RuleMode::ReviewWithHold, "-10")],
            false,
            false,
        );
        let report = engine.execute_pre_trade_dry_run(order());
        assert!(!report.is_pass(), "Reject rules remain effective");
        assert_eq!(report.rejects().unwrap()[0].mode, RuleMode::Reject);

        let engine = fixture(axis, &[], false, false);
        engine
            .configure()
            .spot_funds::<SpotFundsConfigError>(TestPolicy::NAME, |settings| {
                let entries = [SpotFundsPnlBoundsAccountBarrier {
                    account_id: account(2),
                    barrier: SpotFundsPnlBoundsBarrier {
                        mode: RuleMode::ReviewWithHold,
                        ..bounds("-10")
                    },
                }];
                match axis {
                    Some(axis) => settings.set_pnl_axis_account_barriers(axis, entries),
                    None => settings.set_pnl_account_barriers(entries),
                }
            })
            .unwrap();
        let request = order();
        assert!(engine.execute_pre_trade_dry_run(request.clone()).is_pass());
        assert!(engine.execute_pre_trade(request.clone()).is_ok());
    }
}

#[test]
fn review_pnl_each_axis_passes_on_or_inside_review_bounds() {
    for axis in axes() {
        for lower in ["-20", "-100"] {
            let engine = fixture(axis, &[(RuleMode::ReviewWithHold, lower)], false, false);
            let report = engine.execute_pre_trade_dry_run(order());
            assert!(report.is_pass());
            assert!(
                report.reviews().is_empty(),
                "a value on or inside the bound must not mark"
            );
            let reservation = engine.execute_pre_trade(order()).unwrap();
            assert!(reservation.reviews().is_empty());
            assert!(!reservation.account_adjustments().is_empty());
        }
    }
}

#[test]
fn review_pnl_each_axis_non_order_halts_still_block() {
    for axis in axes() {
        for adjustment in [false, true] {
            let engine = fixture(axis, &[(RuleMode::ReviewWithHold, "-10")], true, false);
            let state = PnlState::Halted(PnlHaltReason::MissingFx);
            let blocks = if adjustment {
                engine
                    .apply_account_adjustment(account(1), &[account_pnl_adjustment(state)])
                    .unwrap()
                    .account_blocks
            } else {
                engine
                    .configure()
                    .set_spot_funds_account_pnl(TestPolicy::NAME, account(1), state)
                    .unwrap()
                    .account_blocks
            };
            assert_eq!(blocks.len(), 1, "halt must block on force-set: {axis:?}");
            assert_eq!(blocks[0].code, RejectCode::PnlKillSwitchTriggered);
        }
        let engine = fixture(axis, &[(RuleMode::ReviewWithHold, "-10")], true, false);
        let result = engine.apply_execution_report(&fill_with_fee(
            account(1),
            instr("AAPL", "USD"),
            Side::Sell,
            "90",
            "1",
            money_fee("0", "USD"),
        ));
        assert_eq!(
            result.account_blocks.len(),
            1,
            "halt must block on fill: {axis:?}"
        );
        assert_eq!(
            result.account_blocks[0].code,
            RejectCode::PnlKillSwitchTriggered
        );
    }
}

fn axes() -> impl Iterator<Item = Option<SpotFundsPnlBoundsAxis>> {
    std::iter::once(None).chain(SpotFundsPnlBoundsAxis::ALL.into_iter().map(Some))
}

fn bounds(lower: &str) -> SpotFundsPnlBoundsBarrier {
    SpotFundsPnlBoundsBarrier {
        currency: asset("USD"),
        lower_bound: Some(pnl_value(lower)),
        upper_bound: None,
        mode: RuleMode::Reject,
    }
}

fn set_rule(
    settings: &mut SpotFundsSettings,
    axis: Option<SpotFundsPnlBoundsAxis>,
    mode: RuleMode,
    lower: &str,
) -> Result<(), SpotFundsConfigError> {
    match axis {
        Some(axis) => settings.set_pnl_axis_global_barriers(
            axis,
            [SpotFundsPnlBoundsBarrier {
                mode,
                ..bounds(lower)
            }],
        ),
        None => settings.set_pnl_global_barriers([SpotFundsPnlBoundsBarrier {
            mode,
            ..bounds(lower)
        }]),
    }
}

fn fixture(
    axis: Option<SpotFundsPnlBoundsAxis>,
    rules: &[(RuleMode, &str)],
    halted: bool,
    mismatch: bool,
) -> ReviewEngine {
    let policy = if halted && axis.is_some() {
        build_policy(None, None)
    } else {
        build_policy_with_market_data(instr("AAPL", "USD"), px("90"), 0)
    };
    seed_with_avg(&policy, account(1), asset("AAPL"), "2", px("100"));
    seed(&policy, account(1), asset("USD"), "1000");
    let engine = crate::Engine::builder::<TestOrder, TestReport, AccountPnlAdjustment>()
        .with_order_review()
        .full_sync()
        .pre_trade(policy)
        .build()
        .unwrap();
    engine.accounts().set_currency(account(1), asset("USD"));
    let state = if halted && axis.is_none() {
        PnlState::Halted(PnlHaltReason::MissingFx)
    } else if axis.is_none() {
        PnlState::Value(pnl_value("-20"))
    } else {
        PnlState::Value(Pnl::ZERO)
    };
    engine
        .configure()
        .set_spot_funds_account_pnl(TestPolicy::NAME, account(1), state)
        .unwrap();
    engine
        .configure()
        .spot_funds::<SpotFundsConfigError>(TestPolicy::NAME, |settings| {
            let barriers = rules
                .iter()
                .map(|&(mode, lower)| SpotFundsPnlBoundsBarrier {
                    mode,
                    ..bounds(lower)
                });
            match axis {
                Some(axis) => settings.set_pnl_axis_global_barriers(axis, barriers)?,
                None => settings.set_pnl_global_barriers(barriers)?,
            }
            if mismatch {
                let entries = rules.iter().map(|&(mode, lower)| {
                    let mut barrier = bounds(lower);
                    barrier.currency = asset("EUR");
                    SpotFundsPnlBoundsAccountBarrier {
                        account_id: account(1),
                        barrier: SpotFundsPnlBoundsBarrier { mode, ..barrier },
                    }
                });
                match axis {
                    Some(axis) => settings.set_pnl_axis_account_barriers(axis, entries)?,
                    None => settings.set_pnl_account_barriers(entries)?,
                }
            }
            Ok(())
        })
        .unwrap();
    // Retunes deliberately judge the current state. Unblock so this test
    // observes the order-time decision independently of that prior decision.
    engine.accounts().unblock(account(1));
    engine
}

#[test]
fn review_pnl_each_axis_preserves_marks_outcomes_and_dry_run_parity() {
    for axis in axes() {
        for mode in [RuleMode::ReviewWithHold, RuleMode::ReviewWithoutHold] {
            let engine = fixture(
                axis,
                &[(mode, "-10"), (RuleMode::Reject, "-100")],
                false,
                false,
            );
            let request = order();
            let report = engine.execute_pre_trade_dry_run(request.clone());
            assert!(
                report.is_pass(),
                "review breach must pass dry run: {axis:?}"
            );
            assert!(
                report.account_block().is_none(),
                "a review mark must never be a block"
            );
            assert_eq!(
                report.reviews().len(),
                1,
                "one violated review rule must mark"
            );
            let mark = &report.reviews()[0];
            assert_eq!(mark.mode, mode);
            assert_eq!(mark.code, RejectCode::PnlKillSwitchTriggered);
            assert_eq!(mark.scope, RejectScope::Account);
            let rejecting_engine = fixture(axis, &[(RuleMode::Reject, "-10")], false, false);
            let rejects = rejecting_engine
                .execute_pre_trade(request.clone())
                .err()
                .unwrap();
            assert_eq!(rejects.len(), 1);
            assert_eq!(
                mark.clone().with_mode(RuleMode::Reject),
                rejects[0],
                "review payload must match Reject payload: {axis:?}"
            );
            let mut reservation = engine.execute_pre_trade(request.clone()).unwrap();
            assert_eq!(
                reservation.reviews(),
                report.reviews(),
                "dry and live marks must match"
            );
            if mode == RuleMode::ReviewWithHold {
                assert!(
                    !reservation.account_adjustments().is_empty(),
                    "review must carry reservation outcomes"
                );
                assert!(
                    !reservation.lock().is_empty(),
                    "review must carry reservation lock"
                );
                assert_eq!(
                    reservation.account_adjustments(),
                    report.account_adjustments()
                );
            } else {
                assert!(
                    reservation.account_adjustments().is_empty(),
                    "without-hold must roll back"
                );
                assert!(reservation.lock().is_empty());
            }
            reservation.rollback();
            assert!(
                engine.start_pre_trade(order()).is_ok(),
                "a mark must not latch a block"
            );
        }
    }
}

#[test]
fn review_pnl_each_axis_reject_breach_wins_over_marks() {
    for axis in axes() {
        let engine = fixture(
            axis,
            &[(RuleMode::ReviewWithHold, "-10"), (RuleMode::Reject, "-10")],
            false,
            false,
        );
        let report = engine.execute_pre_trade_dry_run(order());
        assert!(!report.is_pass(), "Reject breach must reject: {axis:?}");
        assert!(
            report.reviews().is_empty(),
            "effective reject must discard marks"
        );
        let rejects = engine.execute_pre_trade(order()).err().unwrap();
        assert_eq!(rejects.len(), 1);
        assert_eq!(rejects[0].mode, RuleMode::Reject);
        assert_eq!(rejects[0].code, RejectCode::PnlKillSwitchTriggered);
        assert!(
            engine.start_pre_trade(order()).is_err(),
            "Reject breach must block"
        );
        engine
            .configure()
            .spot_funds::<SpotFundsConfigError>(TestPolicy::NAME, |settings| {
                let mut barrier = bounds("-10");
                barrier.currency = asset("EUR");
                let entries = [SpotFundsPnlBoundsAccountBarrier {
                    account_id: account(1),
                    barrier: SpotFundsPnlBoundsBarrier {
                        mode: RuleMode::ReviewWithHold,
                        ..barrier
                    },
                }];
                match axis {
                    Some(axis) => settings.set_pnl_axis_account_barriers(axis, entries),
                    None => settings.set_pnl_account_barriers(entries),
                }
            })
            .unwrap();
        engine.accounts().unblock(account(1));
        let report = engine.execute_pre_trade_dry_run(order());
        assert_eq!(
            report.rejects().unwrap()[0].code,
            RejectCode::PnlKillSwitchTriggered,
            "Reject breach must precede the same axis's review mismatch"
        );
        assert_eq!(
            report.rejects().unwrap()[0].reason,
            "pnl kill switch triggered",
            "Reject breach must precede the same axis's review mismatch"
        );
    }
    // A later-axis effective reject must also discard a prior realized mark.
    let engine = fixture(None, &[(RuleMode::ReviewWithHold, "-10")], false, false);
    engine
        .configure()
        .spot_funds::<SpotFundsConfigError>(TestPolicy::NAME, |settings| {
            set_rule(
                settings,
                Some(SpotFundsPnlBoundsAxis::AccountUnrealized),
                RuleMode::Reject,
                "-10",
            )
        })
        .unwrap();
    engine.accounts().unblock(account(1));
    let result = engine.execute_pre_trade(order());
    assert!(
        result.is_err(),
        "later-axis effective reject must win over a realized mark"
    );
    let rejects = result.err().unwrap();
    assert_eq!(rejects.len(), 1);
    assert_eq!(rejects[0].mode, RuleMode::Reject);
    assert!(rejects[0].details.contains("account unrealized"));
}

#[test]
fn review_pnl_each_axis_halted_and_currency_mismatch_remain_effective_rejects() {
    for axis in axes() {
        for mode in [RuleMode::ReviewWithHold, RuleMode::ReviewWithoutHold] {
            for (halted, mismatch) in [(true, false), (false, true)] {
                let engine = fixture(axis, &[(mode, "-10")], halted, mismatch);
                let report = engine.execute_pre_trade_dry_run(order());
                assert!(
                    !report.is_pass(),
                    "review evaluation failure must reject: {axis:?}"
                );
                assert!(
                    report.account_block().is_some(),
                    "review evaluation failure must block"
                );
                assert!(report.reviews().is_empty());
                let rejects = engine.execute_pre_trade(order()).err().unwrap();
                assert_eq!(rejects[0].mode, RuleMode::Reject);
                assert_eq!(rejects[0].scope, RejectScope::Account);
                if halted {
                    assert_eq!(
                        rejects[0].code,
                        RejectCode::PnlKillSwitchTriggered,
                        "halted code must stay unchanged"
                    );
                } else {
                    assert_eq!(rejects[0].reason, "pnl barrier currency mismatch");
                }
                assert!(
                    engine.start_pre_trade(order()).is_err(),
                    "failure must latch a block"
                );
            }
        }
    }
}

#[test]
fn review_pnl_first_violation_per_mode_and_realized_then_axis_order() {
    for first in axes() {
        let engine = fixture(
            first,
            &[
                (RuleMode::ReviewWithHold, "-10"),
                (RuleMode::ReviewWithoutHold, "-10"),
            ],
            false,
            false,
        );
        let before = engine.execute_pre_trade_dry_run(order()).reviews().to_vec();
        engine
            .configure()
            .spot_funds::<SpotFundsConfigError>(TestPolicy::NAME, |settings| {
                for axis in SpotFundsPnlBoundsAxis::ALL {
                    if first.is_none() || axis as u8 >= first.unwrap() as u8 {
                        settings.set_pnl_axis_global_barriers(
                            axis,
                            [RuleMode::ReviewWithHold, RuleMode::ReviewWithoutHold].map(|mode| {
                                SpotFundsPnlBoundsBarrier {
                                    mode,
                                    ..bounds("-10")
                                }
                            }),
                        )?;
                    }
                }
                Ok(())
            })
            .unwrap();
        let report = engine.execute_pre_trade_dry_run(order());
        assert_eq!(report.reviews().len(), 2, "exactly one mark per mode");
        assert_eq!(
            report.reviews(),
            before,
            "later violations must not replace the first mark"
        );
        let reservation = engine.execute_pre_trade(order()).unwrap();
        assert_eq!(reservation.reviews(), before);
        assert!(
            reservation.account_adjustments().is_empty(),
            "without-hold wins when both modes mark"
        );
    }
}

#[test]
fn review_pnl_each_axis_drop_copy_accounts_without_recording_a_review_block() {
    for axis in axes() {
        for mode in [RuleMode::ReviewWithHold, RuleMode::ReviewWithoutHold] {
            let engine = fixture(axis, &[(mode, "-10")], false, false);
            let mut operation = engine.apply_drop_copy(order()).unwrap();
            assert!(
                operation.account_block().is_none(),
                "drop-copy mark must never record a block"
            );
            assert!(!operation.is_account_blocked());
            assert!(
                !operation.account_adjustments().is_empty(),
                "drop-copy must retain accounting"
            );
            operation.commit();
            assert!(engine.start_pre_trade(order()).is_ok());
        }
    }
}

#[test]
fn review_pnl_each_axis_non_order_paths_ignore_breaches_but_block_evaluation_failures() {
    for axis in axes() {
        for mode in [RuleMode::ReviewWithHold, RuleMode::ReviewWithoutHold] {
            for mismatch in [false, true] {
                for adjustment in [false, true] {
                    let engine = fixture(axis, &[(mode, "-10")], false, mismatch);
                    let state = PnlState::Value(pnl_value("-30"));
                    let blocks = if adjustment {
                        engine
                            .apply_account_adjustment(account(1), &[account_pnl_adjustment(state)])
                            .unwrap()
                            .account_blocks
                    } else {
                        engine
                            .configure()
                            .set_spot_funds_account_pnl(TestPolicy::NAME, account(1), state)
                            .unwrap()
                            .account_blocks
                    };
                    assert_eq!(
                        blocks.len(),
                        usize::from(mismatch),
                        "force-set must distinguish review breach from mismatch: {axis:?}"
                    );
                }
                let engine = fixture(axis, &[(mode, "-10")], false, mismatch);
                let result = engine.apply_execution_report(&fill_with_fee(
                    account(1),
                    instr("AAPL", "USD"),
                    Side::Sell,
                    "90",
                    "1",
                    money_fee("0", "USD"),
                ));
                assert_eq!(
                    result.account_blocks.len(),
                    usize::from(mismatch),
                    "fill must distinguish review breach from mismatch: {axis:?}"
                );
                let engine = fixture(axis, &[(mode, "-10")], false, mismatch);
                let result = engine.apply_execution_report(&fill_with_fee(
                    account(1),
                    instr("AAPL", "USD"),
                    Side::Buy,
                    "90",
                    "0",
                    money_fee("1", "USD"),
                ));
                assert_eq!(
                    result.account_blocks.len(),
                    usize::from(mismatch),
                    "fee-only must distinguish review breach from mismatch: {axis:?}"
                );
            }
        }
    }
}

#[test]
fn review_pnl_each_axis_retune_and_clear_preserve_block_rules() {
    for axis in axes() {
        for mode in [RuleMode::ReviewWithHold, RuleMode::ReviewWithoutHold] {
            for (halted, mismatch) in [(false, false), (true, false), (false, true)] {
                let engine = fixture(axis, &[(mode, "-100")], halted, mismatch);
                let result = engine
                    .configure()
                    .spot_funds::<SpotFundsConfigError>(TestPolicy::NAME, |settings| {
                        if mismatch {
                            let mut barrier = bounds("-10");
                            barrier.currency = asset("EUR");
                            let entries = [SpotFundsPnlBoundsAccountBarrier {
                                account_id: account(1),
                                barrier: SpotFundsPnlBoundsBarrier { mode, ..barrier },
                            }];
                            match axis {
                                Some(axis) => settings.set_pnl_axis_account_barriers(axis, entries),
                                None => settings.set_pnl_account_barriers(entries),
                            }
                        } else {
                            set_rule(settings, axis, mode, "-10")
                        }
                    })
                    .unwrap();
                assert_eq!(
                    result.account_blocks.len(),
                    usize::from(halted || mismatch),
                    "retune must only block review evaluation failures: {axis:?}"
                );
                engine
                    .configure()
                    .spot_funds::<SpotFundsConfigError>(TestPolicy::NAME, |settings| match axis {
                        Some(axis) => {
                            settings.set_pnl_axis_global_barriers(axis, [])?;
                            settings.set_pnl_axis_account_barriers(axis, [])
                        }
                        None => {
                            settings.set_pnl_global_barriers([])?;
                            settings.set_pnl_account_barriers([])
                        }
                    })
                    .unwrap();
                assert_eq!(
                    engine.start_pre_trade(order()).is_err(),
                    halted || mismatch,
                    "clearing review rules must never release an existing block"
                );
            }
        }
    }
}

#[test]
fn review_pnl_each_axis_drop_copy_preserves_historical_accounting() {
    for axis in axes() {
        for mode in [RuleMode::ReviewWithHold, RuleMode::ReviewWithoutHold] {
            let engine = fixture(axis, &[(mode, "-10")], false, false);
            let request = order();
            let result = engine.apply_drop_copy(request.clone());
            assert!(
                result.is_ok(),
                "drop-copy must admit a review breach: {axis:?}, {mode:?}"
            );
            let mut operation = result.unwrap();
            assert!(operation.account_block().is_none());
            assert!(!operation.is_account_blocked());
            assert!(
                !operation.account_adjustments().is_empty(),
                "drop-copy must keep historical accounting: {axis:?}, {mode:?}"
            );
            let adjustments = operation.account_adjustments().to_vec();
            assert_eq!(adjustments.len(), 2);
            assert_eq!(adjustments[0].entry.held.unwrap().absolute, ps("1"));
            assert_eq!(adjustments[1].entry.incoming.unwrap().absolute, ps("1"));
            operation.commit();
            engine
                .configure()
                .spot_funds::<SpotFundsConfigError>(TestPolicy::NAME, |settings| match axis {
                    Some(axis) => settings.set_pnl_axis_global_barriers(axis, []),
                    None => settings.set_pnl_global_barriers([]),
                })
                .unwrap();
            let after = engine.execute_pre_trade_dry_run(order());
            assert_eq!(
                after.account_adjustments()[0].entry.held.unwrap().absolute,
                ps("2"),
                "drop-copy accounting must persist: {axis:?}, {mode:?}"
            );
        }
    }
}

#[test]
fn review_pnl_each_axis_drop_copy_restores_safety_blocks() {
    for axis in axes() {
        for mode in [RuleMode::ReviewWithHold, RuleMode::ReviewWithoutHold] {
            for (halted, mismatch) in [(true, false), (false, true)] {
                let engine = fixture(axis, &[(mode, "-10")], halted, mismatch);
                let request = order();
                let mut operation = engine.apply_drop_copy(request.clone()).unwrap();
                assert!(
                    operation.account_block().is_some(),
                    "drop-copy block: {axis:?}, {mode:?}, halted={halted}"
                );
                assert!(operation.is_account_blocked());
                assert!(!operation.account_adjustments().is_empty());
                operation.commit();
                assert!(
                    engine.start_pre_trade(order()).is_err(),
                    "drop-copy safety block must persist"
                );
            }
        }
    }
}

#[test]
fn review_pnl_each_axis_skips_global_currency_mismatch() {
    for axis in axes() {
        for mode in [RuleMode::ReviewWithHold, RuleMode::ReviewWithoutHold] {
            let policy = build_policy(None, None);
            seed(&policy, account(1), asset("USD"), "1000");
            let engine = crate::Engine::builder::<ProbeOrder, TestReport, AccountPnlAdjustment>()
                .full_sync()
                .pre_trade(policy)
                .build()
                .unwrap();
            engine.accounts().set_currency(account(1), asset("EUR"));
            engine
                .configure()
                .spot_funds::<SpotFundsConfigError>(TestPolicy::NAME, |settings| {
                    set_rule(settings, axis, mode, "-10")
                })
                .unwrap();
            let request = ProbeOrder { inner: order() };
            assert!(
                engine.execute_pre_trade_dry_run(request.clone()).is_pass(),
                "skipped USD review: {axis:?}, {mode:?}"
            );
            assert!(engine.execute_pre_trade(request.clone()).is_ok());
        }
    }
}

fn set_group_rule(
    settings: &mut SpotFundsSettings,
    axis: Option<SpotFundsPnlBoundsAxis>,
    mode: RuleMode,
) -> Result<(), SpotFundsConfigError> {
    let entries = [SpotFundsPnlBoundsAccountGroupBarrier {
        account_group_id: group(7),
        barrier: SpotFundsPnlBoundsBarrier {
            mode,
            ..bounds("-10")
        },
    }];
    match axis {
        Some(axis) => settings.set_pnl_axis_account_group_barriers(axis, entries),
        None => settings.set_pnl_account_group_barriers(entries),
    }
}

#[test]
fn review_pnl_each_axis_group_breach_marks() {
    for axis in axes() {
        for mode in [RuleMode::ReviewWithHold, RuleMode::ReviewWithoutHold] {
            let engine = fixture(axis, &[], false, false);
            engine
                .configure()
                .spot_funds::<SpotFundsConfigError>(TestPolicy::NAME, |settings| {
                    set_group_rule(settings, axis, mode)
                })
                .unwrap();
            engine
                .accounts()
                .register_group(&[account(1)], group(7))
                .unwrap();
            let request = order();
            let report = engine.execute_pre_trade_dry_run(request.clone());
            assert!(report.is_pass());
            assert_eq!(
                report.reviews().len(),
                1,
                "a group review breach must mark: {axis:?}, {mode:?}"
            );
            assert_eq!(report.reviews()[0].mode, mode);
            assert_eq!(report.reviews()[0].code, RejectCode::PnlKillSwitchTriggered);
            let reservation = engine.execute_pre_trade(request.clone()).unwrap();
            assert_eq!(reservation.reviews(), report.reviews());
        }
    }
}

#[test]
fn review_pnl_membership_change_blocks_only_halted_state() {
    for mode in [RuleMode::ReviewWithHold, RuleMode::ReviewWithoutHold] {
        for halted in [false, true] {
            let engine = fixture(None, &[], halted, false);
            engine
                .configure()
                .spot_funds::<SpotFundsConfigError>(TestPolicy::NAME, |settings| {
                    set_group_rule(settings, None, mode)
                })
                .unwrap();
            engine
                .accounts()
                .register_group(&[account(1)], group(7))
                .unwrap();
            assert_eq!(
                engine.start_pre_trade(order()).is_err(),
                halted,
                "membership review must block halt, not breach: {mode:?}, halted={halted}"
            );
        }
    }
}

#[test]
fn review_pnl_assertion_rollback_blocks_only_halted_state() {
    for mode in [RuleMode::ReviewWithHold, RuleMode::ReviewWithoutHold] {
        for halted in [false, true] {
            let mut configured = settings(0);
            set_rule(&mut configured, None, mode, "-10").unwrap();
            let builder =
                crate::Engine::builder::<TestOrder, TestReport, AccountPnlAdjustment>().full_sync();
            let policy = TestPolicy::new(configured, None, builder.storage_builder());
            let engine = builder
                .pre_trade(policy)
                .pre_trade(RejectSecondAdjustmentPolicy {
                    calls: AtomicUsize::new(0),
                })
                .build()
                .unwrap();
            engine.accounts().set_currency(account(1), asset("USD"));
            let previous = if halted {
                PnlState::Halted(PnlHaltReason::MissingFx)
            } else {
                PnlState::Value(pnl_value("-20"))
            };
            engine
                .configure()
                .set_spot_funds_account_pnl(TestPolicy::NAME, account(1), previous)
                .unwrap();
            engine.accounts().unblock(account(1));
            let error = engine
                .apply_account_adjustment(
                    account(1),
                    &[
                        account_pnl_adjustment(PnlState::Value(Pnl::ZERO)),
                        account_pnl_adjustment(PnlState::Value(Pnl::ZERO)),
                    ],
                )
                .unwrap_err();
            assert_eq!(error.failed_adjustment_index, 1);
            assert_eq!(
                engine
                    .start_pre_trade(account_pnl_probe_order(account(1)))
                    .is_err(),
                halted,
                "assertion rollback review must block halt, not breach: {mode:?}, halted={halted}"
            );
        }
    }
}
