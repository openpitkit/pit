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

//! Engine checks for unrealized and total P&L bounds.

use super::*;
use crate::{PnlHaltReason, PnlState};

const MAX: &str = "79228162514264337593543950335";

fn retune_position() -> TestPolicy {
    let policy = build_policy_with_market_data(instr("AAPL", "USD"), px("90"), 0);
    let mut settings = settings(0);
    settings.set_global_limit_mode(SpotFundsLimitMode::TrackOnly);
    configure(&policy, settings);
    seed_with_avg(&policy, account(1), asset("AAPL"), "2", px("100"));
    policy
}

#[test]
fn retune_checks_only_changed_axes_in_the_defined_order() {
    for changed in SpotFundsPnlBoundsAxis::ALL {
        let policy = retune_position();
        let mut initial = settings(0);
        initial.set_global_limit_mode(SpotFundsLimitMode::TrackOnly);
        for axis in SpotFundsPnlBoundsAxis::ALL {
            initial
                .set_pnl_axis_global_barrier(
                    axis,
                    Some(if axis == changed {
                        barrier(Some("-100"), Some("100"))
                    } else {
                        barrier(Some("-10"), Some("10"))
                    }),
                )
                .unwrap();
        }
        configure(&policy, initial);
        let engine = engine(policy, Some("USD"), None);
        let result = engine
            .configure()
            .spot_funds::<SpotFundsConfigError>(TestPolicy::NAME, |settings| {
                settings
                    .set_pnl_axis_global_barrier(changed, Some(barrier(Some("-10"), Some("10"))))
            })
            .unwrap();
        assert_eq!(
            result.account_blocks.len(),
            1,
            "changed axis must block on retune"
        );
        assert_axis_block(&result.account_blocks[0].block, changed, "-20");
    }
    let engine = engine(retune_position(), Some("USD"), None);
    let result = engine
        .configure()
        .spot_funds::<SpotFundsConfigError>(TestPolicy::NAME, |settings| {
            for axis in SpotFundsPnlBoundsAxis::ALL.into_iter().rev() {
                settings
                    .set_pnl_axis_global_barrier(axis, Some(barrier(Some("-10"), Some("10"))))?;
            }
            Ok(())
        })
        .unwrap();
    assert_eq!(result.account_blocks.len(), 1, "first changed axis wins");
    assert_axis_block(
        &result.account_blocks[0].block,
        SpotFundsPnlBoundsAxis::AccountUnrealized,
        "-20",
    );
}

#[test]
fn retune_arms_or_narrows_each_axis_and_returns_its_recorded_block() {
    for axis in SpotFundsPnlBoundsAxis::ALL {
        for narrowing in [false, true] {
            for mode in [SpotFundsLimitMode::Enforce, SpotFundsLimitMode::TrackOnly] {
                let policy = retune_position();
                let mut initial = settings(0);
                initial.set_global_limit_mode(mode);
                if narrowing {
                    initial
                        .set_pnl_axis_global_barrier(axis, Some(barrier(Some("-100"), Some("100"))))
                        .unwrap();
                }
                configure(&policy, initial);
                let engine = engine(policy, Some("USD"), None);
                let outcome = engine
                    .configure()
                    .spot_funds::<SpotFundsConfigError>(TestPolicy::NAME, |settings| {
                        settings.set_pnl_axis_global_barrier(
                            axis,
                            Some(barrier(Some("-10"), Some("10"))),
                        )
                    })
                    .unwrap();
                assert_eq!(
                    outcome.account_blocks.len(),
                    1,
                    "retune must return the {axis:?} block, narrowing {narrowing}, {mode:?}"
                );
                let block = &outcome.account_blocks[0];
                assert_eq!(block.account_id, account(1));
                assert_axis_block(&block.block, axis, "-20");
                assert_eq!(
                    engine.inner.blocked_accounts.account_block(account(1)),
                    Some(block.block.clone()),
                    "retune must record its returned block"
                );
            }
        }
    }
}

#[test]
fn retune_unchanged_effective_axes_do_not_reblock_an_operator_unblocked_account() {
    for axis in SpotFundsPnlBoundsAxis::ALL {
        for change in 0..5 {
            let policy = retune_position();
            let mut initial = configured(axis, Some("-10"), Some("10"));
            if change == 4 {
                initial
                    .set_pnl_axis_account_barriers(
                        axis,
                        [SpotFundsPnlBoundsAccountBarrier {
                            account_id: account(1),
                            barrier: barrier(Some("-10"), Some("10")),
                        }],
                    )
                    .unwrap();
            }
            configure(&policy, initial);
            let engine = engine(policy, Some("USD"), None);
            engine
                .execute_pre_trade(account_pnl_probe_order(account(1)))
                .err()
                .expect("initial breach must block");
            engine.accounts().unblock(account(1));
            let outcome = engine
                .configure()
                .spot_funds::<SpotFundsConfigError>(TestPolicy::NAME, |settings| {
                    match change {
                        0 => settings.set_pnl_axis_global_barrier(
                            axis,
                            Some(barrier(Some("-10"), Some("10"))),
                        )?,
                        1 => settings.set_global_slippage_bps(12)?,
                        2 => settings.set_global_limit_mode(SpotFundsLimitMode::Enforce),
                        3 => {
                            let other = SpotFundsPnlBoundsAxis::ALL
                                .into_iter()
                                .find(|other| *other != axis)
                                .unwrap();
                            settings.set_pnl_axis_account_barriers(
                                other,
                                [SpotFundsPnlBoundsAccountBarrier {
                                    account_id: account(2),
                                    barrier: barrier(Some("-10"), Some("10")),
                                }],
                            )?;
                        }
                        4 => settings.set_pnl_axis_global_barrier(
                            axis,
                            Some(barrier(Some("-5"), Some("5"))),
                        )?,
                        _ => unreachable!(),
                    }
                    Ok(())
                })
                .unwrap();
            assert!(
                outcome.account_blocks.is_empty(),
                "unchanged effective {axis:?} must return no block, change {change}"
            );
            assert!(
                engine
                    .inner
                    .blocked_accounts
                    .account_block(account(1))
                    .is_none(),
                "unchanged effective {axis:?} must stay unblocked, change {change}"
            );
        }
    }
}

#[test]
fn retune_axis_candidates_follow_global_group_and_account_tiers() {
    for axis in SpotFundsPnlBoundsAxis::ALL {
        for tier in 0..3 {
            let policy = retune_position();
            for id in 2..=4 {
                seed_with_avg(&policy, account(id), asset("AAPL"), "2", px("100"));
            }
            let mut initial = settings(0);
            initial.set_global_limit_mode(SpotFundsLimitMode::TrackOnly);
            initial
                .set_pnl_axis_account_barriers(
                    axis,
                    [SpotFundsPnlBoundsAccountBarrier {
                        account_id: account(4),
                        barrier: barrier(Some("-100"), Some("100")),
                    }],
                )
                .unwrap();
            configure(&policy, initial);
            let engine = engine(policy, Some("USD"), None);
            for id in 2..=4 {
                engine.accounts().set_currency(account(id), asset("USD"));
            }
            engine
                .accounts()
                .register_group(
                    &[account(1), account(2)],
                    AccountGroupId::from_u32(7).unwrap(),
                )
                .unwrap();
            let outcome = engine
                .configure()
                .spot_funds::<SpotFundsConfigError>(TestPolicy::NAME, |settings| {
                    let narrowed = barrier(Some("-10"), Some("10"));
                    match tier {
                        0 => settings.set_pnl_axis_global_barrier(axis, Some(narrowed)),
                        1 => settings.set_pnl_axis_account_group_barriers(
                            axis,
                            [SpotFundsPnlBoundsAccountGroupBarrier {
                                account_group_id: AccountGroupId::from_u32(7).unwrap(),
                                barrier: narrowed,
                            }],
                        ),
                        2 => settings.set_pnl_axis_account_barriers(
                            axis,
                            [
                                SpotFundsPnlBoundsAccountBarrier {
                                    account_id: account(3),
                                    barrier: narrowed,
                                },
                                SpotFundsPnlBoundsAccountBarrier {
                                    account_id: account(4),
                                    barrier: barrier(Some("-100"), Some("100")),
                                },
                            ],
                        ),
                        _ => unreachable!(),
                    }
                })
                .unwrap();
            let expected = match tier {
                0 => vec![account(1), account(2), account(3)],
                1 => vec![account(1), account(2)],
                2 => vec![account(3)],
                _ => unreachable!(),
            };
            assert_eq!(
                outcome
                    .account_blocks
                    .iter()
                    .map(|entry| entry.account_id)
                    .collect::<Vec<_>>(),
                expected,
                "retune {axis:?} tier {tier} must reach exactly its candidates"
            );
            for id in 1..=4 {
                assert_eq!(
                    engine
                        .inner
                        .blocked_accounts
                        .account_block(account(id))
                        .is_some(),
                    expected.contains(&account(id)),
                    "retune must record only selected accounts"
                );
            }
        }
    }
}

#[test]
fn retune_realized_block_precedes_axes_and_prevents_market_data_reads() {
    for axis in SpotFundsPnlBoundsAxis::ALL {
        let reads = Arc::new(AtomicUsize::new(0));
        let policy = counted_position(settings(0), &reads);
        let service = &policy.market_orders.as_ref().unwrap().market_data;
        let instrument = service.resolve(&instr("AAPL", "USD")).unwrap();
        service
            .push(instrument, Quote::new().with_mark(px("90")), Duration::ZERO)
            .unwrap();
        policy.set_account_pnl_state(account(1), PnlState::Value(pnl_value("-20")));
        let engine = crate::Engine::builder::<TestOrder, TestReport, TestAdjustment>()
            .full_sync()
            .pre_trade(policy)
            .build()
            .unwrap();
        engine.accounts().set_currency(account(1), asset("USD"));
        reads.store(0, Ordering::Relaxed);
        let outcome = engine
            .configure()
            .spot_funds::<SpotFundsConfigError>(TestPolicy::NAME, |settings| {
                settings.set_pnl_global_barrier(Some(barrier(Some("-10"), Some("10"))))?;
                settings.set_pnl_axis_global_barrier(axis, Some(barrier(Some("-10"), Some("10"))))
            })
            .unwrap();
        assert_eq!(
            outcome.account_blocks.len(),
            1,
            "realized retune must return one block"
        );
        assert_eq!(
            outcome.account_blocks[0].block.details,
            "lower bound breached: realized pnl -20, lower bound -10, upper bound 10, currency USD"
        );
        assert_eq!(
            reads.load(Ordering::Relaxed),
            0,
            "realized retune block must prevent every axis market-data read"
        );
    }
}

#[test]
fn retune_checks_axes_for_accounts_that_pass_realized_despite_other_realized_blocks() {
    let policy = retune_position();
    seed_with_avg(&policy, account(2), asset("AAPL"), "2", px("100"));
    policy.set_account_pnl_state(account(1), PnlState::Value(pnl_value("-20")));
    policy.set_account_pnl_state(account(2), PnlState::Value(Pnl::ZERO));
    let engine = engine(policy, Some("USD"), None);
    engine.accounts().set_currency(account(2), asset("USD"));
    let axis = SpotFundsPnlBoundsAxis::AccountUnrealized;
    let outcome = engine
        .configure()
        .spot_funds::<SpotFundsConfigError>(TestPolicy::NAME, |settings| {
            settings.set_pnl_global_barrier(Some(barrier(Some("-10"), Some("10"))))?;
            settings.set_pnl_axis_global_barrier(axis, Some(barrier(Some("-10"), Some("10"))))
        })
        .unwrap();
    assert_eq!(
        outcome.account_blocks.len(),
        2,
        "realized blocks must not skip axes for accounts that pass realized"
    );
    let realized = &outcome.account_blocks[0];
    assert_eq!(realized.account_id, account(1));
    assert_eq!(realized.block.code, RejectCode::PnlKillSwitchTriggered);
    assert_eq!(realized.block.reason, "pnl kill switch triggered");
    assert_eq!(
        realized.block.details,
        "lower bound breached: realized pnl -20, lower bound -10, upper bound 10, currency USD"
    );
    let unrealized = &outcome.account_blocks[1];
    assert_eq!(unrealized.account_id, account(2));
    assert_axis_block(&unrealized.block, axis, "-20");
    for outcome in outcome.account_blocks {
        assert_eq!(
            engine
                .inner
                .blocked_accounts
                .account_block(outcome.account_id),
            Some(outcome.block),
            "retune must record both realized and axis blocks"
        );
    }
}

#[test]
fn retune_account_total_provenance_is_invalidated_but_other_axes_survive_rollback() {
    for axis in SpotFundsPnlBoundsAxis::ALL {
        let policy = retune_position();
        policy.set_account_pnl_state(account(1), PnlState::Value(Pnl::ZERO));
        let pnl = policy.pnl.clone();
        let entered = Arc::new(std::sync::Barrier::new(2));
        let release = Arc::new(std::sync::Barrier::new(2));
        let engine = Arc::new(
            crate::Engine::builder::<TestOrder, TestReport, AccountPnlAdjustment>()
                .full_sync()
                .pre_trade(policy)
                .pre_trade(BlockingAdjustmentRejectPolicy {
                    entered: Arc::clone(&entered),
                    release: Arc::clone(&release),
                })
                .build()
                .unwrap(),
        );
        engine.accounts().set_currency(account(1), asset("USD"));
        let adjustment = {
            let engine = Arc::clone(&engine);
            std::thread::spawn(move || {
                engine.apply_account_adjustment(
                    account(1),
                    &[account_pnl_adjustment(PnlState::Value(pnl_value("-5")))],
                )
            })
        };
        entered.wait();
        let token = pnl
            .with(&account(1), |entry| entry.assertion_token)
            .flatten()
            .unwrap();
        let retuned = engine
            .configure()
            .spot_funds::<SpotFundsConfigError>(TestPolicy::NAME, |settings| {
                settings.set_pnl_axis_global_barrier(axis, Some(barrier(Some("-10"), Some("10"))))
            })
            .unwrap();
        let before_rollback = engine.inner.blocked_accounts.account_block(account(1));
        release.wait();
        adjustment
            .join()
            .unwrap()
            .expect_err("held batch must reject");
        assert_eq!(
            retuned.account_blocks.len(),
            1,
            "retune must block during assertion"
        );
        let block = &retuned.account_blocks[0].block;
        let account_total = axis == SpotFundsPnlBoundsAxis::AccountTotal;
        assert_axis_block(block, axis, if account_total { "-25" } else { "-20" });
        assert_eq!(
            block.provenance(),
            account_total.then_some(token),
            "only account total retune blocks carry the asserted record token"
        );
        assert_eq!(
            before_rollback,
            Some(block.clone()),
            "retune must block before rollback"
        );
        assert_eq!(
            pnl.with(&account(1), |entry| entry.state),
            Some(PnlState::Value(Pnl::ZERO))
        );
        assert_eq!(
            engine.inner.blocked_accounts.account_block(account(1)),
            (!account_total).then(|| block.clone()),
            "rollback invalidates only the account total retune block"
        );
    }
}

#[test]
fn retune_without_changed_axes_reads_no_market_data() {
    for axis in SpotFundsPnlBoundsAxis::ALL {
        let reads = Arc::new(AtomicUsize::new(0));
        let policy = counted_position(configured(axis, Some("-10"), Some("10")), &reads);
        let engine = crate::Engine::builder::<TestOrder, TestReport, TestAdjustment>()
            .full_sync()
            .pre_trade(policy)
            .build()
            .unwrap();
        engine.accounts().set_currency(account(1), asset("USD"));
        reads.store(0, Ordering::Relaxed);
        let result = engine
            .configure()
            .spot_funds::<SpotFundsConfigError>(TestPolicy::NAME, |settings| {
                settings.set_global_slippage_bps(12)?;
                settings.set_pnl_axis_global_barrier(axis, Some(barrier(Some("-10"), Some("10"))))
            })
            .unwrap();
        assert_eq!(
            reads.load(Ordering::Relaxed),
            0,
            "retune without changed axes must read no market data"
        );
        assert!(
            result.account_blocks.is_empty(),
            "unchanged axes must dispatch no check"
        );
    }
}

#[test]
fn group_membership_changes_defer_axis_breaches_until_the_next_order() {
    for axis in SpotFundsPnlBoundsAxis::ALL {
        for unregister in [false, true] {
            let policy = retune_position();
            let mut initial = settings(0);
            initial.set_global_limit_mode(SpotFundsLimitMode::TrackOnly);
            if unregister {
                initial
                    .set_pnl_axis_global_barrier(axis, Some(barrier(Some("-10"), Some("10"))))
                    .unwrap();
            }
            initial
                .set_pnl_axis_account_group_barriers(
                    axis,
                    [SpotFundsPnlBoundsAccountGroupBarrier {
                        account_group_id: AccountGroupId::from_u32(7).unwrap(),
                        barrier: if unregister {
                            barrier(Some("-100"), Some("100"))
                        } else {
                            barrier(Some("-10"), Some("10"))
                        },
                    }],
                )
                .unwrap();
            configure(&policy, initial);
            let engine = engine(
                policy,
                Some("USD"),
                unregister.then_some(AccountGroupId::from_u32(7).unwrap()),
            );
            if unregister {
                engine
                    .accounts()
                    .unregister_group(&[account(1)], AccountGroupId::from_u32(7).unwrap())
                    .unwrap();
            } else {
                engine
                    .accounts()
                    .register_group(&[account(1)], AccountGroupId::from_u32(7).unwrap())
                    .unwrap();
            }
            assert!(
                engine
                    .inner
                    .blocked_accounts
                    .account_block(account(1))
                    .is_none(),
                "membership change must remain realized-only"
            );
            let rejects = engine
                .execute_pre_trade(account_pnl_probe_order(account(1)))
                .err()
                .expect("next order must catch the axis breach");
            assert!(
                rejects
                    .iter()
                    .any(|reject| reject.code == RejectCode::PnlKillSwitchTriggered),
                "next order must reject with the P&L kill switch"
            );
            let block = engine
                .inner
                .blocked_accounts
                .account_block(account(1))
                .expect("next order must record the axis block");
            assert_eq!(block.code, RejectCode::AccountBlocked);
            assert_eq!(
                block.details,
                format!(
                    "lower bound breached: {} -20, lower bound -10, upper bound 10, currency USD{}",
                    label(axis),
                    suffix(axis),
                )
            );
        }
    }
}

fn assert_axis_block(
    block: &crate::pretrade::AccountBlock,
    axis: SpotFundsPnlBoundsAxis,
    value: &str,
) {
    assert_eq!(block.code, RejectCode::PnlKillSwitchTriggered);
    assert_eq!(block.reason, "pnl kill switch triggered");
    assert_eq!(
        block.details,
        format!(
            "lower bound breached: {} {value}, lower bound -10, upper bound 10, currency USD{}",
            label(axis),
            suffix(axis)
        )
    );
}

#[test]
fn trade_fill_checks_each_axis_in_both_limit_modes() {
    for axis in SpotFundsPnlBoundsAxis::ALL {
        for mode in [SpotFundsLimitMode::Enforce, SpotFundsLimitMode::TrackOnly] {
            let policy = marked(axis, "90");
            let mut settings = configured(axis, Some("-10"), Some("10"));
            settings.set_global_limit_mode(mode);
            configure(&policy, settings);
            seed(&policy, account(1), asset("USD"), "1000");
            let engine = engine(policy, Some("USD"), None);
            let result = engine.apply_execution_report(&fill_with_fee(
                account(1),
                instr("AAPL", "USD"),
                Side::Buy,
                "100",
                "2",
                money_fee("0", "USD"),
            ));
            assert_eq!(
                result.account_blocks.len(),
                1,
                "fill must check {axis:?} in {mode:?}"
            );
            assert_axis_block(&result.account_blocks[0], axis, "-20");
            assert_eq!(result.account_blocks[0].provenance, None);
            assert!(engine
                .inner
                .blocked_accounts
                .account_block(account(1))
                .is_some());
        }
    }
}

#[test]
fn nonzero_fee_only_fill_checks_an_axis_in_both_limit_modes() {
    let axis = SpotFundsPnlBoundsAxis::AccountUnrealized;
    for mode in [SpotFundsLimitMode::Enforce, SpotFundsLimitMode::TrackOnly] {
        let policy = marked(axis, "90");
        let mut settings = configured(axis, Some("-10"), Some("10"));
        settings.set_global_limit_mode(mode);
        configure(&policy, settings);
        seed_with_avg(&policy, account(1), asset("AAPL"), "2", px("100"));
        seed(&policy, account(1), asset("USD"), "1000");
        let engine = engine(policy, Some("USD"), None);
        let result = engine.apply_execution_report(&fee_only_report(
            account(1),
            instr("AAPL", "USD"),
            Side::Buy,
            "0",
            false,
            money_fee("1", "USD"),
        ));
        assert_eq!(
            result.account_blocks.len(),
            1,
            "fee-only fill must check the axis"
        );
        assert_axis_block(&result.account_blocks[0], axis, "-20");
        assert_eq!(result.account_blocks[0].provenance, None);
    }
}

#[test]
fn realized_fill_block_prevents_an_additional_axis_block() {
    let axis = SpotFundsPnlBoundsAxis::AccountUnrealized;
    let policy = marked(axis, "90");
    let mut settings = configured(axis, Some("-10"), Some("10"));
    settings
        .set_pnl_global_barrier(Some(barrier(Some("-1"), None)))
        .unwrap();
    configure(&policy, settings);
    let engine = engine(policy, Some("USD"), None);
    let result = engine.apply_execution_report(&fill_with_fee(
        account(1),
        instr("AAPL", "USD"),
        Side::Buy,
        "100",
        "2",
        money_fee("2", "USD"),
    ));
    assert_eq!(
        result.account_blocks.len(),
        1,
        "realized fill block must suppress axis evaluation"
    );
    assert_eq!(result.account_blocks[0].reason, "pnl kill switch triggered");
    assert_eq!(
        result.account_blocks[0].details,
        "lower bound breached: realized pnl -2, lower bound -1, currency USD"
    );
}

fn counted_position(
    settings: SpotFundsSettings,
    reads: &Arc<AtomicUsize>,
) -> SpotFundsPolicy<FullSync, CountingMarketData> {
    let service =
        MarketDataBuilder::with_sync(CountingMarketData(Arc::clone(reads)), QuoteTtl::Infinite)
            .build();
    service.register(instr("AAPL", "USD")).unwrap();
    let builder = crate::Engine::builder::<TestOrder, TestReport, TestAdjustment>().full_sync();
    let policy = SpotFundsPolicy::<FullSync, CountingMarketData>::new(
        settings,
        Some(SpotFundsMarketData::new(service)),
        builder.storage_builder(),
    );
    policy
        .holdings
        .with_mut((account(1), asset("AAPL")), Holdings::zero, |slot, _| {
            *slot =
                Holdings::new(ps("2"), PositionSize::ZERO).with_avg_entry_price(Some(px("100")));
        });
    policy
}

#[test]
fn report_without_trade_or_nonzero_fee_reads_no_axis_market_data() {
    for fee in [None, Some(money_fee("0", "USD"))] {
        let reads = Arc::new(AtomicUsize::new(0));
        let policy = counted_position(
            configured(
                SpotFundsPnlBoundsAxis::AccountUnrealized,
                Some("-10"),
                Some("10"),
            ),
            &reads,
        );
        let engine = crate::Engine::builder::<TestOrder, TestReport, TestAdjustment>()
            .full_sync()
            .pre_trade(policy)
            .build()
            .unwrap();
        engine.accounts().set_currency(account(1), asset("USD"));
        let mut report = make_report(
            account(1),
            instr("AAPL", "USD"),
            Side::Buy,
            None,
            qty("0"),
            true,
            None,
        );
        report.fee = fee;
        reads.store(0, Ordering::Relaxed);
        let result = engine.apply_execution_report(&report);
        assert_eq!(
            reads.load(Ordering::Relaxed),
            0,
            "an inert report must not evaluate axes"
        );
        assert!(result.account_blocks.is_empty());
    }
}

#[test]
fn fill_account_total_breach_uses_assertion_token_but_position_breach_survives_rollback() {
    for axis in [
        SpotFundsPnlBoundsAxis::AccountTotal,
        SpotFundsPnlBoundsAxis::PositionUnrealized,
    ] {
        let policy = marked(axis, "90");
        let pnl = policy.pnl.clone();
        let entered = Arc::new(std::sync::Barrier::new(2));
        let release = Arc::new(std::sync::Barrier::new(2));
        let engine = Arc::new(
            crate::Engine::builder::<TestOrder, TestReport, AccountPnlAdjustment>()
                .full_sync()
                .pre_trade(policy)
                .pre_trade(BlockingAdjustmentRejectPolicy {
                    entered: Arc::clone(&entered),
                    release: Arc::clone(&release),
                })
                .build()
                .unwrap(),
        );
        engine.accounts().set_currency(account(1), asset("USD"));
        let adjustment = {
            let engine = Arc::clone(&engine);
            std::thread::spawn(move || {
                engine.apply_account_adjustment(
                    account(1),
                    &[account_pnl_adjustment(PnlState::Value(pnl_value("-5")))],
                )
            })
        };
        entered.wait();
        let token = pnl
            .with(&account(1), |entry| entry.assertion_token)
            .flatten()
            .unwrap();
        let result = engine.apply_execution_report(&fill_with_fee(
            account(1),
            instr("AAPL", "USD"),
            Side::Buy,
            "100",
            "2",
            money_fee("1", "USD"),
        ));
        release.wait();
        adjustment
            .join()
            .unwrap()
            .expect_err("batch must roll back");
        assert_eq!(result.account_blocks.len(), 1);
        let block = &result.account_blocks[0];
        let account_total = axis == SpotFundsPnlBoundsAxis::AccountTotal;
        assert_axis_block(block, axis, if account_total { "-26" } else { "-20" });
        assert_eq!(
            block.provenance,
            account_total.then_some(token),
            "only account total depends on the asserted record"
        );
        assert_eq!(
            pnl.with(&account(1), |entry| (entry.state, entry.assertion_token)),
            Some((PnlState::Value(pnl_value("-1")), None))
        );
        let survivor = engine.inner.blocked_accounts.account_block(account(1));
        if account_total {
            assert!(
                survivor.is_none(),
                "rollback must invalidate the account total block"
            );
        } else {
            let survivor = survivor.expect("position block must survive account P&L rollback");
            assert_eq!(survivor.provenance(), None);
            assert_eq!(survivor.details, block.details);
        }
    }
}

#[test]
fn fill_on_asserted_stored_account_total_halt_is_invalidated_by_rollback() {
    let policy = marked(SpotFundsPnlBoundsAxis::AccountTotal, "90");
    let pnl = policy.pnl.clone();
    let entered = Arc::new(std::sync::Barrier::new(2));
    let release = Arc::new(std::sync::Barrier::new(2));
    let engine = Arc::new(
        crate::Engine::builder::<TestOrder, TestReport, AccountPnlAdjustment>()
            .full_sync()
            .pre_trade(policy)
            .pre_trade(BlockingAdjustmentRejectPolicy {
                entered: Arc::clone(&entered),
                release: Arc::clone(&release),
            })
            .build()
            .unwrap(),
    );
    engine.accounts().set_currency(account(1), asset("USD"));
    let reason = PnlHaltReason::MissingInitialPnl;
    let adjustment = {
        let engine = Arc::clone(&engine);
        std::thread::spawn(move || {
            engine.apply_account_adjustment(
                account(1),
                &[account_pnl_adjustment(PnlState::Halted(reason))],
            )
        })
    };
    entered.wait();
    let asserted = pnl.with(&account(1), |entry| (entry.state, entry.assertion_token));
    let result = engine.apply_execution_report(&fill_with_fee(
        account(1),
        instr("AAPL", "USD"),
        Side::Buy,
        "100",
        "2",
        money_fee("1", "USD"),
    ));
    let stored = engine.inner.blocked_accounts.account_block(account(1));
    release.wait();
    adjustment
        .join()
        .unwrap()
        .expect_err("batch must roll back");
    let (state, token) = asserted.unwrap();
    assert_eq!(state, PnlState::Halted(reason));
    let token = token.expect("in-flight assertion must have a token");
    assert_eq!(result.account_blocks.len(), 1);
    let block = &result.account_blocks[0];
    assert_eq!(block.reason, "account pnl calculation halted");
    assert_eq!(
        block.details,
        "account total pnl: halt reason MissingInitialPnl"
    );
    assert_eq!(block.provenance, Some(token));
    assert_eq!(stored.unwrap().provenance(), Some(token));
    assert!(
        engine
            .inner
            .blocked_accounts
            .account_block(account(1))
            .is_none(),
        "rollback must invalidate the stored account total halt block"
    );
}

#[test]
fn fill_that_newly_halts_account_total_has_no_provenance() {
    let axis = SpotFundsPnlBoundsAxis::AccountTotal;
    let engine = engine(marked(axis, "90"), Some("USD"), None);
    let result = engine.apply_execution_report(&fill_with_fee(
        account(1),
        instr("AAPL", "USD"),
        Side::Buy,
        "100",
        "2",
        money_fee("1", "EUR"),
    ));
    assert_eq!(result.account_blocks.len(), 1);
    let block = &result.account_blocks[0];
    assert_eq!(block.reason, "account pnl calculation halted");
    assert_eq!(block.details, "account total pnl: halt reason MissingFx");
    assert_eq!(block.provenance, None);
}

fn force_set_breach(axis: SpotFundsPnlBoundsAxis, adjustment_route: bool) {
    let policy = marked(axis, "90");
    let value = if axis == SpotFundsPnlBoundsAxis::AccountTotal {
        "-20"
    } else {
        seed_with_avg(&policy, account(1), asset("AAPL"), "2", px("100"));
        "0"
    };
    let engine = crate::Engine::builder::<TestOrder, TestReport, AccountPnlAdjustment>()
        .full_sync()
        .pre_trade(policy)
        .build()
        .unwrap();
    engine.accounts().set_currency(account(1), asset("USD"));
    let state = PnlState::Value(pnl_value(value));
    let blocks = if adjustment_route {
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
        1,
        "force-set must check {axis:?}, adjustment route {adjustment_route}"
    );
    assert_axis_block(&blocks[0], axis, "-20");
    assert_eq!(blocks[0].provenance, None);
    assert!(engine
        .inner
        .blocked_accounts
        .account_block(account(1))
        .is_some());
}

#[test]
fn configured_account_pnl_force_set_checks_each_axis_without_provenance() {
    for axis in SpotFundsPnlBoundsAxis::ALL {
        force_set_breach(axis, false);
    }
}

#[test]
fn adjusted_account_pnl_force_set_checks_each_axis_without_provenance() {
    for axis in SpotFundsPnlBoundsAxis::ALL {
        force_set_breach(axis, true);
    }
}

#[test]
fn realized_force_set_block_prevents_axis_reads_on_both_routes() {
    for adjustment_route in [false, true] {
        for axis in SpotFundsPnlBoundsAxis::ALL {
            let reads = Arc::new(AtomicUsize::new(0));
            let mut settings = configured(axis, Some("-10"), Some("10"));
            settings
                .set_pnl_global_barrier(Some(barrier(Some("-10"), None)))
                .unwrap();
            let policy = counted_position(settings, &reads);
            let engine = crate::Engine::builder::<TestOrder, TestReport, AccountPnlAdjustment>()
                .full_sync()
                .pre_trade(policy)
                .build()
                .unwrap();
            engine.accounts().set_currency(account(1), asset("USD"));
            reads.store(0, Ordering::Relaxed);
            let state = PnlState::Value(pnl_value("-20"));
            let blocks = if adjustment_route {
                engine
                    .apply_account_adjustment(account(1), &[account_pnl_adjustment(state)])
                    .unwrap()
                    .account_blocks
            } else {
                engine
                    .configure()
                    .set_spot_funds_account_pnl(
                        SpotFundsPolicy::<FullSync, CountingMarketData>::NAME,
                        account(1),
                        state,
                    )
                    .unwrap()
                    .account_blocks
            };
            assert_eq!(blocks.len(), 1);
            assert_eq!(blocks[0].reason, "pnl kill switch triggered");
            assert_eq!(
                blocks[0].details,
                "lower bound breached: realized pnl -20, lower bound -10, currency USD"
            );
            assert_eq!(blocks[0].provenance, None);
            assert_eq!(
                reads.load(Ordering::Relaxed),
                0,
                "realized force-set block must suppress axes, adjustment route {adjustment_route}"
            );
        }
    }
}

fn label(axis: SpotFundsPnlBoundsAxis) -> &'static str {
    match axis {
        SpotFundsPnlBoundsAxis::AccountUnrealized => "account unrealized pnl",
        SpotFundsPnlBoundsAxis::AccountTotal => "account total pnl",
        SpotFundsPnlBoundsAxis::PositionUnrealized => "position unrealized pnl",
        SpotFundsPnlBoundsAxis::PositionTotal => "position total pnl",
    }
}

fn suffix(axis: SpotFundsPnlBoundsAxis) -> &'static str {
    match axis {
        SpotFundsPnlBoundsAxis::AccountUnrealized | SpotFundsPnlBoundsAxis::AccountTotal => "",
        SpotFundsPnlBoundsAxis::PositionUnrealized | SpotFundsPnlBoundsAxis::PositionTotal => {
            ", asset AAPL"
        }
    }
}

fn barrier(lower: Option<&str>, upper: Option<&str>) -> SpotFundsPnlBoundsBarrier {
    SpotFundsPnlBoundsBarrier {
        currency: asset("USD"),
        lower_bound: lower.map(pnl_value),
        upper_bound: upper.map(pnl_value),
    }
}

fn configured(
    axis: SpotFundsPnlBoundsAxis,
    lower: Option<&str>,
    upper: Option<&str>,
) -> SpotFundsSettings {
    let mut configured = settings(0);
    configured.set_global_limit_mode(SpotFundsLimitMode::TrackOnly);
    configured
        .set_pnl_axis_global_barrier(axis, Some(barrier(lower, upper)))
        .unwrap();
    configured
}

fn configure(policy: &TestPolicy, configured: SpotFundsSettings) {
    policy
        .settings
        .update::<std::convert::Infallible>(|settings| {
            *settings = configured;
            Ok(())
        })
        .unwrap();
}

fn marked(axis: SpotFundsPnlBoundsAxis, mark: &str) -> TestPolicy {
    let policy = build_policy_with_market_data(instr("AAPL", "USD"), px(mark), 0);
    configure(&policy, configured(axis, Some("-10"), Some("10")));
    policy
}

fn engine(
    policy: TestPolicy,
    currency: Option<&str>,
    group: Option<AccountGroupId>,
) -> crate::FullSyncEngine<TestOrder, TestReport, TestAdjustment> {
    let engine = crate::Engine::builder::<TestOrder, TestReport, TestAdjustment>()
        .full_sync()
        .pre_trade(policy)
        .build()
        .unwrap();
    if let Some(currency) = currency {
        engine.accounts().set_currency(account(1), asset(currency));
    }
    if let Some(group) = group {
        engine
            .accounts()
            .register_group(&[account(1)], group)
            .unwrap();
    }
    engine
}

fn assert_reject(rejects: &Rejects, reason: &str, details: &str) {
    assert_eq!(rejects.len(), 1);
    assert_eq!(rejects[0].code, RejectCode::PnlKillSwitchTriggered);
    assert_eq!(rejects[0].scope, RejectScope::Account);
    assert_eq!(rejects[0].reason, reason);
    assert_eq!(rejects[0].details, details);
}

fn check(
    policy: TestPolicy,
    currency: Option<&str>,
    group: Option<AccountGroupId>,
    expected: Option<(&str, String)>,
) {
    let engine = engine(policy, currency, group);
    let order = account_pnl_probe_order(account(1));
    let report = engine.execute_pre_trade_dry_run(order.clone());
    match &expected {
        Some((reason, details)) => {
            assert!(!report.is_pass(), "axis must reject dry run: {details}");
            assert_reject(report.rejects().unwrap(), reason, details);
            assert_eq!(report.account_block().unwrap().provenance, None);
        }
        None => assert!(
            report.is_pass(),
            "dry run must pass: {:?}",
            report.rejects()
        ),
    }
    assert!(
        engine.start_pre_trade(order.clone()).is_ok(),
        "dry run must not persist its block"
    );
    let result = engine.execute_pre_trade(order);
    match expected {
        Some((reason, details)) => {
            let rejects = result.err().expect("axis must reject pre-trade");
            assert_reject(&rejects, reason, &details);
        }
        None => assert!(result.is_ok(), "pre-trade must pass"),
    }
}

fn halt(axis: SpotFundsPnlBoundsAxis, reason: PnlHaltReason) -> Option<(&'static str, String)> {
    Some((
        "account pnl calculation halted",
        format!("{}: halt reason {reason:?}{}", label(axis), suffix(axis)),
    ))
}

#[test]
fn each_axis_breaches_lower_and_upper_bounds_with_exact_labels_in_both_limit_modes() {
    for axis in SpotFundsPnlBoundsAxis::ALL {
        for mode in [SpotFundsLimitMode::Enforce, SpotFundsLimitMode::TrackOnly] {
            for (mark, value, side) in [("90", "-20", "lower"), ("110", "20", "upper")] {
                let policy = marked(axis, mark);
                let mut settings = configured(axis, Some("-10"), Some("10"));
                settings.set_global_limit_mode(mode);
                configure(&policy, settings);
                seed_with_avg(&policy, account(1), asset("AAPL"), "2", px("100"));
                seed(&policy, account(1), asset("USD"), "100");
                check(policy, Some("USD"), None, Some(("pnl kill switch triggered", format!("{side} bound breached: {} {value}, lower bound -10, upper bound 10, currency USD{}", label(axis), suffix(axis)))));
            }
        }
    }
}

#[test]
fn each_axis_omits_unconfigured_bounds_from_details() {
    for axis in SpotFundsPnlBoundsAxis::ALL {
        let policy = marked(axis, "90");
        configure(&policy, configured(axis, Some("-10"), None));
        seed_with_avg(&policy, account(1), asset("AAPL"), "2", px("100"));
        check(
            policy,
            Some("USD"),
            None,
            Some((
                "pnl kill switch triggered",
                format!(
                    "lower bound breached: {} -20, lower bound -10, currency USD{}",
                    label(axis),
                    suffix(axis)
                ),
            )),
        );
    }
}

#[test]
fn each_axis_halts_when_neither_direct_nor_inverse_quote_exists() {
    for axis in SpotFundsPnlBoundsAxis::ALL {
        let policy = build_policy(None, None);
        configure(&policy, configured(axis, Some("-10"), Some("10")));
        seed_with_avg(&policy, account(1), asset("AAPL"), "2", px("100"));
        check(
            policy,
            Some("USD"),
            None,
            halt(axis, PnlHaltReason::MissingFx),
        );
    }
}

#[test]
fn each_axis_halts_for_a_halted_slot_without_cost_basis_when_price_exists() {
    for axis in SpotFundsPnlBoundsAxis::ALL {
        let policy = marked(axis, "100");
        seed_halted_position_with_average(
            &policy,
            account(1),
            asset("AAPL"),
            PnlHaltReason::MissingCostBasis,
        );
        policy
            .holdings
            .with_mut_if_present(&(account(1), asset("AAPL")), |slot| {
                *slot = slot.with_avg_entry_price(None);
            });
        check(
            policy,
            Some("USD"),
            None,
            halt(axis, PnlHaltReason::MissingCostBasis),
        );
    }
}

#[test]
fn each_axis_halts_on_full_close_arithmetic_overflow() {
    for axis in SpotFundsPnlBoundsAxis::ALL {
        let policy = marked(axis, MAX);
        seed_with_avg(&policy, account(1), asset("AAPL"), "2", px("0"));
        check(
            policy,
            Some("USD"),
            None,
            halt(axis, PnlHaltReason::ArithmeticOverflow),
        );
    }
}

#[test]
fn each_axis_halts_without_an_effective_account_currency() {
    for axis in SpotFundsPnlBoundsAxis::ALL {
        let policy = marked(axis, "100");
        seed_with_avg(&policy, account(1), asset("AAPL"), "2", px("100"));
        check(
            policy,
            None,
            None,
            halt(axis, PnlHaltReason::MissingAccountCurrency),
        );
    }
}

#[test]
fn each_axis_records_a_drop_copy_block_without_rejecting_the_order() {
    for axis in SpotFundsPnlBoundsAxis::ALL {
        let policy = marked(axis, "110");
        seed_with_avg(&policy, account(1), asset("AAPL"), "2", px("100"));
        let engine = engine(policy, Some("USD"), None);
        let mut operation = engine
            .apply_drop_copy(account_pnl_probe_order(account(1)))
            .unwrap();
        let block = operation
            .account_block()
            .expect("drop copy must record the axis block");
        assert_eq!(block.code, RejectCode::AccountBlocked);
        assert_eq!(block.reason, "pnl kill switch triggered");
        assert_eq!(
            block.details,
            format!(
                "upper bound breached: {} 20, lower bound -10, upper bound 10, currency USD{}",
                label(axis),
                suffix(axis)
            )
        );
        assert_eq!(block.provenance, None);
        assert!(operation.is_account_blocked());
        operation.commit();
        assert!(engine
            .start_pre_trade(account_pnl_probe_order(account(1)))
            .is_err());
    }
}

#[test]
fn each_axis_passes_inside_and_on_the_bounds_with_untracked_realized_operands() {
    for axis in SpotFundsPnlBoundsAxis::ALL {
        for mark in ["95", "100", "105"] {
            let policy = marked(axis, mark);
            seed_unset_with_avg(&policy, account(1), asset("AAPL"), "2", px("100"));
            assert_eq!(
                holdings_of(&policy, account(1), &asset("AAPL"))
                    .unwrap()
                    .realized_pnl_outcome(),
                None
            );
            check(policy, Some("USD"), None, None);
        }
    }
}

#[test]
fn each_axis_blocks_an_account_tier_currency_mismatch_even_without_positions() {
    for axis in SpotFundsPnlBoundsAxis::ALL {
        let policy = build_policy(None, None);
        let mut settings = configured(axis, Some("-10"), Some("10"));
        let mut mismatched = barrier(Some("-10"), None);
        mismatched.currency = asset("EUR");
        settings
            .set_pnl_axis_account_barriers(
                axis,
                [SpotFundsPnlBoundsAccountBarrier {
                    account_id: account(1),
                    barrier: mismatched,
                }],
            )
            .unwrap();
        configure(&policy, settings);
        check(
            policy,
            Some("USD"),
            None,
            Some((
                "pnl barrier currency mismatch",
                format!(
                    "{}: account currency USD, barrier currency EUR",
                    label(axis)
                ),
            )),
        );
    }
}

#[test]
fn each_axis_prefers_its_currency_mismatch_to_its_breach() {
    for axis in SpotFundsPnlBoundsAxis::ALL {
        let policy = marked(axis, "110");
        let mut settings = configured(axis, Some("-10"), Some("10"));
        let mut mismatched = barrier(Some("-10"), Some("10"));
        mismatched.currency = asset("EUR");
        settings
            .set_pnl_axis_account_barriers(
                axis,
                [SpotFundsPnlBoundsAccountBarrier {
                    account_id: account(1),
                    barrier: mismatched,
                }],
            )
            .unwrap();
        configure(&policy, settings);
        seed_with_avg(&policy, account(1), asset("AAPL"), "2", px("100"));
        check(
            policy,
            Some("USD"),
            None,
            Some((
                "pnl barrier currency mismatch",
                format!(
                    "{}: account currency USD, barrier currency EUR",
                    label(axis)
                ),
            )),
        );
    }
}

#[test]
fn earlier_axis_breach_precedes_later_axis_currency_mismatch() {
    let policy = marked(SpotFundsPnlBoundsAxis::AccountUnrealized, "110");
    let mut settings = configured(
        SpotFundsPnlBoundsAxis::AccountUnrealized,
        Some("-10"),
        Some("10"),
    );
    let mut mismatched = barrier(Some("-10"), Some("10"));
    mismatched.currency = asset("EUR");
    settings
        .set_pnl_axis_account_barriers(
            SpotFundsPnlBoundsAxis::PositionTotal,
            [SpotFundsPnlBoundsAccountBarrier {
                account_id: account(1),
                barrier: mismatched,
            }],
        )
        .unwrap();
    configure(&policy, settings);
    seed_with_avg(&policy, account(1), asset("AAPL"), "2", px("100"));
    check(
        policy,
        Some("USD"),
        None,
        Some((
            "pnl kill switch triggered",
            concat!(
                "upper bound breached: account unrealized pnl 20, lower bound -10, ",
                "upper bound 10, currency USD"
            )
            .into(),
        )),
    );
}

#[test]
fn each_axis_skips_group_and_global_currency_mismatches() {
    for axis in SpotFundsPnlBoundsAxis::ALL {
        let policy = build_policy(None, None);
        let mut settings = configured(axis, Some("-10"), Some("10"));
        let mut mismatched = barrier(Some("-10"), None);
        mismatched.currency = asset("EUR");
        settings
            .set_pnl_axis_global_barrier(axis, Some(mismatched.clone()))
            .unwrap();
        settings
            .set_pnl_axis_account_group_barriers(
                axis,
                [SpotFundsPnlBoundsAccountGroupBarrier {
                    account_group_id: AccountGroupId::from_u32(7).unwrap(),
                    barrier: mismatched,
                }],
            )
            .unwrap();
        configure(&policy, settings);
        seed_with_avg(&policy, account(1), asset("AAPL"), "2", px("100"));
        check(
            policy,
            Some("USD"),
            Some(AccountGroupId::from_u32(7).unwrap()),
            None,
        );
    }
}

#[test]
fn settlement_balances_without_average_are_not_positions() {
    for axis in SpotFundsPnlBoundsAxis::ALL {
        let policy = marked(axis, "100");
        seed_with_avg(&policy, account(1), asset("AAPL"), "2", px("100"));
        seed(&policy, account(1), asset("USD"), "100");
        check(policy, Some("USD"), None, None);
    }
}

#[test]
fn account_currency_positions_price_at_one_without_a_quote() {
    for axis in SpotFundsPnlBoundsAxis::ALL {
        let policy = build_policy(None, None);
        configure(&policy, configured(axis, Some("-10"), Some("10")));
        seed_with_avg(&policy, account(1), asset("USD"), "2", px("1"));
        check(policy, Some("USD"), None, None);
    }
}

#[test]
fn inverse_quote_prices_positions() {
    for axis in SpotFundsPnlBoundsAxis::ALL {
        let policy = build_policy_with_market_data(instr("USD", "AAPL"), px("0.01"), 0);
        configure(&policy, configured(axis, Some("-10"), Some("10")));
        seed_with_avg(&policy, account(1), asset("AAPL"), "2", px("100"));
        check(policy, Some("USD"), None, None);
    }
}

#[test]
fn direct_quote_precedes_a_conflicting_inverse_quote() {
    for axis in SpotFundsPnlBoundsAxis::ALL {
        let policy = marked(axis, "100");
        let service = &policy.market_orders.as_ref().unwrap().market_data;
        let id = service.register(instr("USD", "AAPL")).unwrap();
        service
            .push(id, Quote::new().with_mark(px("1")), Duration::ZERO)
            .unwrap();
        seed_with_avg(&policy, account(1), asset("AAPL"), "2", px("100"));
        check(policy, Some("USD"), None, None);
    }
}

#[test]
fn expired_quotes_still_price_positions() {
    for axis in SpotFundsPnlBoundsAxis::ALL {
        let service =
            MarketDataBuilder::<FullSync>::new(QuoteTtl::Within(Duration::from_millis(1))).build();
        let id = service.register(instr("AAPL", "USD")).unwrap();
        service
            .push(
                id,
                Quote::new().with_mark(px("100")),
                Duration::from_secs(60),
            )
            .unwrap();
        let builder = engine_builder();
        let policy = SpotFundsPolicy::new(
            configured(axis, Some("-10"), Some("10")),
            Some(SpotFundsMarketData::new(service)),
            builder.storage_builder(),
        );
        seed_with_avg(&policy, account(1), asset("AAPL"), "2", px("100"));
        check(policy, Some("USD"), None, None);
    }
}

#[test]
fn flat_halted_positions_have_zero_unrealized_but_halted_total() {
    for axis in SpotFundsPnlBoundsAxis::ALL {
        let policy = build_policy(None, None);
        configure(&policy, configured(axis, Some("-10"), Some("10")));
        policy
            .holdings
            .with_mut((account(1), asset("AAPL")), Holdings::zero, |slot, _| {
                *slot = Holdings::zero()
                    .halt_realized_pnl_preserving_average(PnlHaltReason::MissingInitialPnl)
                    .holdings();
            });
        let expected = (axis == SpotFundsPnlBoundsAxis::PositionTotal).then(|| {
            (
                "account pnl calculation halted",
                "position total pnl: halt reason MissingInitialPnl, asset AAPL".to_string(),
            )
        });
        check(policy, Some("USD"), None, expected);
    }
}

#[test]
fn flat_halted_positions_without_account_currency_have_zero_unrealized_but_halted_total() {
    for axis in SpotFundsPnlBoundsAxis::ALL {
        let policy = build_policy(None, None);
        configure(&policy, configured(axis, Some("-10"), Some("10")));
        policy
            .holdings
            .with_mut((account(1), asset("AAPL")), Holdings::zero, |slot, _| {
                *slot = Holdings::zero()
                    .halt_realized_pnl_preserving_average(PnlHaltReason::MissingInitialPnl)
                    .holdings();
            });
        assert_eq!(account_pnl_state_of(&policy, account(1)), None);
        let expected = (axis == SpotFundsPnlBoundsAxis::PositionTotal).then(|| {
            (
                "account pnl calculation halted",
                "position total pnl: halt reason MissingInitialPnl, asset AAPL".to_string(),
            )
        });
        check(policy, None, None, expected);
    }
}

#[test]
fn totals_add_their_own_realized_operand() {
    for axis in [
        SpotFundsPnlBoundsAxis::PositionTotal,
        SpotFundsPnlBoundsAxis::AccountTotal,
    ] {
        let policy = marked(axis, "103");
        seed_with_avg(&policy, account(1), asset("AAPL"), "2", px("100"));
        if axis == SpotFundsPnlBoundsAxis::PositionTotal {
            policy
                .holdings
                .with_mut_if_present(&(account(1), asset("AAPL")), |slot| {
                    *slot = slot.with_realized_pnl(pnl_value("7"));
                });
            policy.set_account_pnl_state(account(1), PnlState::Value(pnl_value("-100")));
        } else {
            policy.set_account_pnl_state(account(1), PnlState::Value(pnl_value("7")));
        }
        check(
            policy,
            Some("USD"),
            None,
            Some((
                "pnl kill switch triggered",
                format!(
                    "upper bound breached: {} 13, lower bound -10, upper bound 10, currency USD{}",
                    label(axis),
                    suffix(axis)
                ),
            )),
        );
    }
}

#[test]
fn account_total_reads_an_unset_ledger_as_zero_realized() {
    let policy = marked(SpotFundsPnlBoundsAxis::AccountTotal, "106");
    seed_with_avg(&policy, account(1), asset("AAPL"), "2", px("100"));
    assert!(policy.pnl.with(&account(1), |_| ()).is_none());
    check(
        policy,
        Some("USD"),
        None,
        Some((
            "pnl kill switch triggered",
            format!(
                concat!(
                    "upper bound breached: account total pnl 12, ",
                    "lower bound -10, upper bound 10, currency USD{}"
                ),
                suffix(SpotFundsPnlBoundsAxis::AccountTotal)
            ),
        )),
    );
}

#[test]
fn total_axes_propagate_a_halted_realized_operand() {
    for axis in [
        SpotFundsPnlBoundsAxis::PositionTotal,
        SpotFundsPnlBoundsAxis::AccountTotal,
    ] {
        let policy = marked(axis, "100");
        seed_with_avg(&policy, account(1), asset("AAPL"), "2", px("100"));
        if axis == SpotFundsPnlBoundsAxis::PositionTotal {
            seed_halted_position_with_average(
                &policy,
                account(1),
                asset("AAPL"),
                PnlHaltReason::MissingInitialPnl,
            );
        } else {
            policy.set_account_pnl_state(
                account(1),
                PnlState::Halted(PnlHaltReason::MissingInitialPnl),
            );
        }
        check(
            policy,
            Some("USD"),
            None,
            halt(axis, PnlHaltReason::MissingInitialPnl),
        );
    }
}

#[test]
fn account_unrealized_sums_positions_and_includes_held_quantity() {
    let axis = SpotFundsPnlBoundsAxis::AccountUnrealized;
    let policy = marked(axis, "104");
    seed_with_avg(&policy, account(1), asset("AAPL"), "2", px("100"));
    policy
        .holdings
        .with_mut_if_present(&(account(1), asset("AAPL")), |slot| {
            *slot = Holdings::new(ps("1"), ps("1")).with_avg_entry_price(Some(px("100")));
        });
    let service = &policy.market_orders.as_ref().unwrap().market_data;
    let id = service.register(instr("MSFT", "USD")).unwrap();
    service
        .push(id, Quote::new().with_mark(px("104")), Duration::ZERO)
        .unwrap();
    seed_with_avg(&policy, account(1), asset("MSFT"), "1", px("100"));
    check(policy, Some("USD"), None, Some(("pnl kill switch triggered", "upper bound breached: account unrealized pnl 12, lower bound -10, upper bound 10, currency USD".into())));
}

#[test]
fn account_halt_priority_prefers_overflow_to_missing_fx_across_positions() {
    for axis in [
        SpotFundsPnlBoundsAxis::AccountUnrealized,
        SpotFundsPnlBoundsAxis::AccountTotal,
    ] {
        let policy = build_policy_with_market_data(instr("MSFT", "USD"), px(MAX), 0);
        configure(&policy, configured(axis, Some("-10"), Some("10")));
        seed_with_avg(&policy, account(1), asset("AAPL"), "2", px("100"));
        seed_with_avg(&policy, account(1), asset("MSFT"), "2", px("0"));
        check(
            policy,
            Some("USD"),
            None,
            halt(axis, PnlHaltReason::ArithmeticOverflow),
        );
    }
}

#[test]
fn realized_bounds_reject_first_with_unchanged_details() {
    for axis in SpotFundsPnlBoundsAxis::ALL {
        let policy = marked(axis, "110");
        let mut settings = configured(axis, Some("-10"), Some("10"));
        settings
            .set_pnl_global_barrier(Some(barrier(Some("-10"), None)))
            .unwrap();
        configure(&policy, settings);
        seed_with_avg(&policy, account(1), asset("AAPL"), "2", px("100"));
        policy.set_account_pnl_state(account(1), PnlState::Value(pnl_value("-20")));
        check(
            policy,
            Some("USD"),
            None,
            Some((
                "pnl kill switch triggered",
                "lower bound breached: realized pnl -20, lower bound -10, currency USD".into(),
            )),
        );
    }
}

#[test]
fn each_axis_reads_its_own_barrier_and_uses_the_defined_axis_order() {
    for axis in SpotFundsPnlBoundsAxis::ALL {
        let policy = marked(axis, "110");
        let mut settings = settings(0);
        settings.set_global_limit_mode(SpotFundsLimitMode::TrackOnly);
        for other in SpotFundsPnlBoundsAxis::ALL {
            settings
                .set_pnl_axis_global_barrier(
                    other,
                    Some(if other == axis {
                        barrier(Some("-10"), Some("10"))
                    } else {
                        barrier(Some("-100"), Some("100"))
                    }),
                )
                .unwrap();
        }
        configure(&policy, settings);
        seed_with_avg(&policy, account(1), asset("AAPL"), "2", px("100"));
        check(
            policy,
            Some("USD"),
            None,
            Some((
                "pnl kill switch triggered",
                format!(
                    "upper bound breached: {} 20, lower bound -10, upper bound 10, currency USD{}",
                    label(axis),
                    suffix(axis)
                ),
            )),
        );
    }
    let policy = marked(SpotFundsPnlBoundsAxis::AccountUnrealized, "110");
    let mut settings = configured(
        SpotFundsPnlBoundsAxis::AccountUnrealized,
        Some("-10"),
        Some("10"),
    );
    for axis in SpotFundsPnlBoundsAxis::ALL {
        settings
            .set_pnl_axis_global_barrier(axis, Some(barrier(Some("-10"), Some("10"))))
            .unwrap();
    }
    configure(&policy, settings);
    seed_with_avg(&policy, account(1), asset("AAPL"), "2", px("100"));
    check(policy, Some("USD"), None, Some(("pnl kill switch triggered", "upper bound breached: account unrealized pnl 20, lower bound -10, upper bound 10, currency USD".into())));
}

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

#[derive(Clone)]
struct CountingMarketData(Arc<AtomicUsize>);

struct CountingLock<T> {
    inner: std::sync::RwLock<T>,
    reads: Arc<AtomicUsize>,
}

impl crate::marketdata::sealed::Sealed for CountingMarketData {}

impl crate::marketdata::MarketDataSync for CountingMarketData {
    type Shared<T: 'static> = Arc<T>;
    type Lock<T> = CountingLock<T>;
    type Gate = AtomicBool;

    fn new_shared<T: 'static>(&self, inner: T) -> Arc<T> {
        Arc::new(inner)
    }

    fn new_lock<T>(&self, inner: T) -> CountingLock<T> {
        CountingLock {
            inner: std::sync::RwLock::new(inner),
            reads: Arc::clone(&self.0),
        }
    }
}

// SAFETY: the standard RwLock owns all access; the counter never accesses T.
unsafe impl<T> crate::marketdata::MarketDataLock<T> for CountingLock<T> {
    type ReadGuard<'a>
        = std::sync::RwLockReadGuard<'a, T>
    where
        Self: 'a,
        T: 'a;
    type WriteGuard<'a>
        = std::sync::RwLockWriteGuard<'a, T>
    where
        Self: 'a,
        T: 'a;

    fn read(&self) -> Self::ReadGuard<'_> {
        self.reads.fetch_add(1, Ordering::Relaxed);
        self.inner.read().unwrap()
    }

    fn write(&self) -> Self::WriteGuard<'_> {
        self.inner.write().unwrap()
    }
}

#[test]
fn no_axis_scope_or_effective_barrier_reads_no_market_data() {
    for mismatched_global in [false, true] {
        let reads = Arc::new(AtomicUsize::new(0));
        let mut settings = settings(0);
        settings.set_global_limit_mode(SpotFundsLimitMode::TrackOnly);
        if mismatched_global {
            let mut mismatched = barrier(Some("-10"), None);
            mismatched.currency = asset("EUR");
            settings
                .set_pnl_axis_global_barrier(
                    SpotFundsPnlBoundsAxis::AccountUnrealized,
                    Some(mismatched),
                )
                .unwrap();
        } else {
            settings
                .set_pnl_axis_account_barriers(
                    SpotFundsPnlBoundsAxis::AccountUnrealized,
                    [SpotFundsPnlBoundsAccountBarrier {
                        account_id: account(2),
                        barrier: barrier(Some("-10"), None),
                    }],
                )
                .unwrap();
        }
        let policy = counted_position(settings, &reads);
        let engine = crate::Engine::builder::<TestOrder, TestReport, TestAdjustment>()
            .full_sync()
            .pre_trade(policy)
            .build()
            .unwrap();
        engine.accounts().set_currency(account(1), asset("USD"));
        reads.store(0, Ordering::Relaxed);
        assert!(engine
            .execute_pre_trade_dry_run(account_pnl_probe_order(account(1)))
            .is_pass());
        assert_eq!(
            reads.load(Ordering::Relaxed),
            0,
            "no effective axis barrier must read no market data on dry run"
        );
        assert!(engine
            .execute_pre_trade(account_pnl_probe_order(account(1)))
            .is_ok());
        assert_eq!(
            reads.load(Ordering::Relaxed),
            0,
            "no effective axis barrier must read no market data on pre-trade"
        );
    }
}

#[test]
fn all_four_axes_price_each_position_only_once_per_check() {
    for axes in [
        vec![SpotFundsPnlBoundsAxis::AccountUnrealized],
        SpotFundsPnlBoundsAxis::ALL.to_vec(),
    ] {
        let reads = Arc::new(AtomicUsize::new(0));
        let mut settings = settings(0);
        settings.set_global_limit_mode(SpotFundsLimitMode::TrackOnly);
        for axis in axes {
            settings
                .set_pnl_axis_global_barrier(axis, Some(barrier(Some("-10"), Some("10"))))
                .unwrap();
        }
        let policy = counted_position(settings, &reads);
        let service = &policy.market_orders.as_ref().unwrap().market_data;
        let instrument = service.resolve(&instr("AAPL", "USD")).unwrap();
        service
            .push(
                instrument,
                Quote::new().with_mark(px("100")),
                Duration::ZERO,
            )
            .unwrap();
        reads.store(0, Ordering::Relaxed);
        assert_eq!(
            policy.account_currency_factor(account(1), None, &asset("AAPL"), &asset("USD")),
            Some(px("100").to_decimal()),
        );
        let pricing_reads = reads.swap(0, Ordering::Relaxed);
        assert!(pricing_reads > 0, "one pricing must read market data");
        let engine = crate::Engine::builder::<TestOrder, TestReport, TestAdjustment>()
            .full_sync()
            .pre_trade(policy)
            .build()
            .unwrap();
        engine.accounts().set_currency(account(1), asset("USD"));
        reads.store(0, Ordering::Relaxed);
        assert!(engine
            .execute_pre_trade_dry_run(account_pnl_probe_order(account(1)))
            .is_pass());
        let dry_run_reads = reads.swap(0, Ordering::Relaxed);
        assert_eq!(dry_run_reads, pricing_reads, "dry run must price once");
        assert!(engine
            .execute_pre_trade(account_pnl_probe_order(account(1)))
            .is_ok());
        assert_eq!(
            reads.load(Ordering::Relaxed),
            pricing_reads,
            "pre-trade must price once",
        );
    }
}

#[test]
fn position_axes_return_the_first_breached_asset_in_index_order() {
    for axis in [
        SpotFundsPnlBoundsAxis::PositionUnrealized,
        SpotFundsPnlBoundsAxis::PositionTotal,
    ] {
        let policy = marked(axis, "110");
        let service = &policy.market_orders.as_ref().unwrap().market_data;
        let id = service.register(instr("MSFT", "USD")).unwrap();
        service
            .push(id, Quote::new().with_mark(px("120")), Duration::ZERO)
            .unwrap();
        seed_with_avg(&policy, account(1), asset("MSFT"), "2", px("100"));
        seed_with_avg(&policy, account(1), asset("AAPL"), "2", px("100"));
        check(policy, Some("USD"), None, Some(("pnl kill switch triggered", format!("upper bound breached: {} 20, lower bound -10, upper bound 10, currency USD, asset AAPL", label(axis)))));
    }
}

#[test]
fn price_is_required_before_missing_cost_basis_is_reported() {
    for axis in SpotFundsPnlBoundsAxis::ALL {
        let policy = build_policy(None, None);
        configure(&policy, configured(axis, Some("-10"), Some("10")));
        seed_halted_position_with_average(
            &policy,
            account(1),
            asset("AAPL"),
            PnlHaltReason::MissingCostBasis,
        );
        policy
            .holdings
            .with_mut_if_present(&(account(1), asset("AAPL")), |slot| {
                *slot = slot.with_avg_entry_price(None);
            });
        check(
            policy,
            Some("USD"),
            None,
            halt(axis, PnlHaltReason::MissingFx),
        );
    }
}

#[test]
fn total_axes_halt_when_realized_plus_unrealized_overflows() {
    for axis in [
        SpotFundsPnlBoundsAxis::AccountTotal,
        SpotFundsPnlBoundsAxis::PositionTotal,
    ] {
        let policy = marked(axis, MAX);
        seed_with_avg(&policy, account(1), asset("AAPL"), "1", px("0"));
        policy
            .holdings
            .with_mut_if_present(&(account(1), asset("AAPL")), |slot| {
                *slot = slot.with_realized_pnl(pnl_value(MAX));
            });
        policy.set_account_pnl_state(account(1), PnlState::Value(pnl_value(MAX)));
        check(
            policy,
            Some("USD"),
            None,
            halt(axis, PnlHaltReason::ArithmeticOverflow),
        );
    }
}

#[test]
fn account_sum_overflow_outweighs_an_earlier_missing_fx_halt() {
    for axis in [
        SpotFundsPnlBoundsAxis::AccountUnrealized,
        SpotFundsPnlBoundsAxis::AccountTotal,
    ] {
        let policy = build_policy_with_market_data(instr("MSFT", "USD"), px(MAX), 0);
        configure(&policy, configured(axis, Some("-10"), Some("10")));
        let service = &policy.market_orders.as_ref().unwrap().market_data;
        let id = service.register(instr("NVDA", "USD")).unwrap();
        service
            .push(id, Quote::new().with_mark(px(MAX)), Duration::ZERO)
            .unwrap();
        seed_with_avg(&policy, account(1), asset("AAPL"), "1", px("0"));
        seed_with_avg(&policy, account(1), asset("MSFT"), "1", px("0"));
        seed_with_avg(&policy, account(1), asset("NVDA"), "1", px("0"));
        check(
            policy,
            Some("USD"),
            None,
            halt(axis, PnlHaltReason::ArithmeticOverflow),
        );
    }
}
