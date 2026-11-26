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

use super::*;
use crate::core::{AccountGroups, AccountGroupsHandle};
use crate::storage::{FullLocking, LockingPolicyFactory, StorageBuilder};
use crate::{OrderReview, WithOrderReview};

const REVIEW_MODES: [(SpotFundsLimitMode, RuleMode); 2] = [
    (SpotFundsLimitMode::ReviewWithHold, RuleMode::ReviewWithHold),
    (
        SpotFundsLimitMode::ReviewWithoutHold,
        RuleMode::ReviewWithoutHold,
    ),
];

fn order(side: Side, same_asset: bool) -> WithOrderReview<TestOrder> {
    WithOrderReview {
        inner: make_order(
            account(1),
            instr(if same_asset { "USD" } else { "AAPL" }, "USD"),
            side,
            TradeAmount::Quantity(qty("10")),
            Some(px("0.5")),
        ),
        review: OrderReview::default(),
    }
}

fn parts(
    result: Result<Option<PolicyPreTradeResult>, Rejects>,
) -> (Vec<Reject>, Option<PolicyPreTradeResult>) {
    match result {
        Ok(outcome) => (Vec::new(), outcome),
        Err(rejects) => rejects.into_parts(),
    }
}

fn outcome_values(
    outcome: Option<PolicyPreTradeResult>,
) -> Option<(Vec<crate::AccountOutcomeEntry>, Vec<Price>)> {
    outcome.map(|outcome| {
        (
            outcome.account_adjustments.to_vec(),
            outcome.lock_prices.to_vec(),
        )
    })
}

// Compare the carried policy results explicitly: Rejects equality compares only
// the marks. Dry-run must leave even the set of stored slots unchanged.
fn paired_check(
    policy: &TestPolicy,
    ctx: &PreTradeContext<FullLocking>,
    order: &WithOrderReview<TestOrder>,
    mutations: &mut Mutations,
) -> (Vec<Reject>, Option<PolicyPreTradeResult>) {
    let before = snapshot(policy, &order.inner);
    let keys = policy.holdings.keys();
    let mut dry_mutations = Mutations::new();
    let dry = review_dry_run(policy, ctx, order, &mut dry_mutations);
    assert!(
        dry_mutations.is_empty(),
        "dry run must register no rollback"
    );
    assert_eq!(
        snapshot(policy, &order.inner),
        before,
        "dry run must write nothing"
    );
    assert_eq!(policy.holdings.keys(), keys, "dry run must create no slots");
    let live = review_check(policy, ctx, order, mutations);
    assert_eq!(
        dry.is_ok(),
        live.is_ok(),
        "live and dry verdicts must agree"
    );
    let dry = parts(dry);
    let live = parts(live);
    assert_eq!(dry.0, live.0, "live and dry marks must agree");
    assert_eq!(
        outcome_values(dry.1),
        outcome_values(live.1.clone()),
        "live and dry outcomes must agree"
    );
    live
}

#[test]
fn sufficient_review_funds_match_enforce() {
    for side in [Side::Buy, Side::Sell] {
        let order = order(side, false);
        let mut expected = None;
        for mode in [
            SpotFundsLimitMode::Enforce,
            SpotFundsLimitMode::ReviewWithHold,
            SpotFundsLimitMode::ReviewWithoutHold,
        ] {
            let policy = build_policy_with_limit_mode(mode);
            seed(&policy, account(1), asset("USD"), "20");
            seed(&policy, account(1), asset("AAPL"), "20");
            let mut mutations = Mutations::new();
            let (marks, outcome) = paired_check(
                &policy,
                &account_context(false, order.review.confirmed, order.review.already_held),
                &order,
                &mut mutations,
            );
            assert!(marks.is_empty(), "sufficient funds must not produce a mark");
            assert!(!mutations.is_empty(), "sufficient funds must be reserved");
            let actual = (
                outcome_values(Some(outcome.expect("funded order has an outcome"))),
                snapshot(&policy, &order.inner),
            );
            if let Some(expected) = &expected {
                assert_eq!(&actual, expected, "review bookkeeping must match Enforce");
            } else {
                expected = Some(actual);
            }
            assert!(!mutations.rollback_all().failure().failed());
        }
    }
}

#[test]
fn short_review_funds_preserve_enforce_details_and_outcomes() {
    for side in [Side::Buy, Side::Sell] {
        for initial in [None, Some("1")] {
            let order = order(side, false);
            let held_asset = if side == Side::Buy {
                asset("USD")
            } else {
                asset("AAPL")
            };
            let enforce = build_policy_with_limit_mode(SpotFundsLimitMode::Enforce);
            if let Some(initial) = initial {
                seed(&enforce, account(1), held_asset.clone(), initial);
            }
            let mut mutations = Mutations::new();
            let (expected, _) = paired_check(
                &enforce,
                &account_context(false, order.review.confirmed, order.review.already_held),
                &order,
                &mut mutations,
            );
            assert_eq!(expected[0].code, RejectCode::InsufficientFunds);
            for (mode, rule) in REVIEW_MODES {
                let policy = build_policy_with_limit_mode(mode);
                if let Some(initial) = initial {
                    seed(&policy, account(1), held_asset.clone(), initial);
                }
                let before = snapshot(&policy, &order.inner);
                let keys = policy.holdings.keys();
                let mut mutations = Mutations::new();
                let (marks, outcome) = paired_check(
                    &policy,
                    &account_context(false, order.review.confirmed, order.review.already_held),
                    &order,
                    &mut mutations,
                );
                assert_eq!(
                    marks,
                    vec![expected[0].clone().with_mode(rule)],
                    "review must preserve the enforcing reject payload"
                );
                let outcome = outcome.expect("review must carry its policy result");
                assert_eq!(outcome.account_adjustments.len(), 2);
                assert_eq!(outcome.lock_prices.to_vec(), vec![px("0.5")]);
                let expected_held = if side == Side::Buy { ps("5") } else { ps("10") };
                let available = initial
                    .map(ps)
                    .unwrap_or(PositionSize::ZERO)
                    .checked_sub(expected_held)
                    .unwrap();
                let entry = &outcome.account_adjustments[0];
                assert_eq!(entry.held.unwrap().absolute, expected_held);
                assert_eq!(entry.balance.unwrap().absolute, available);
                if mode == SpotFundsLimitMode::ReviewWithHold {
                    let held = holdings_of(&policy, account(1), &held_asset).unwrap();
                    assert_eq!(
                        held.held(),
                        expected_held,
                        "review-with-hold must write the full hold"
                    );
                    assert_eq!(held.available(), available);
                    assert!(!mutations.is_empty(), "a written hold needs rollback");
                    assert_eq!(
                        outcome.account_adjustments[1].incoming.unwrap().absolute,
                        if side == Side::Buy { ps("10") } else { ps("5") }
                    );
                } else {
                    assert_eq!(
                        snapshot(&policy, &order.inner),
                        before,
                        "without-hold must write neither the short nor the following leg"
                    );
                    assert_eq!(
                        policy.holdings.keys(),
                        keys,
                        "without-hold must leave no new slots"
                    );
                    assert!(
                        mutations.is_empty(),
                        "without-hold must register no rollback"
                    );
                    assert!(policy.active_holdings_mutations.keys().is_empty());
                }
                assert!(!mutations.rollback_all().failure().failed());
                assert_eq!(
                    snapshot(&policy, &order.inner),
                    before,
                    "review rollback must restore both assets"
                );
            }
        }
    }
}

#[test]
fn same_asset_reviews_thread_pure_holdings_into_position_checks() {
    for side in [Side::Buy, Side::Sell] {
        for initial in ["0", "1", "20"] {
            for limit in ["4", "5", "10"] {
                let order = order(side, true);
                let mut track = None;
                for mode in [
                    SpotFundsLimitMode::TrackOnly,
                    SpotFundsLimitMode::ReviewWithHold,
                    SpotFundsLimitMode::ReviewWithoutHold,
                ] {
                    let mut configured = settings(0);
                    configured.set_global_limit_mode(mode);
                    configured.set_position_limit(
                        account(1),
                        asset("USD"),
                        RuleMode::Reject,
                        Some(qty(limit)),
                    );
                    let policy = build_policy_from_settings(configured, None);
                    seed(&policy, account(1), asset("USD"), initial);
                    let adjustment = held_adj(
                        asset("USD"),
                        Some(AdjustmentAmount::Absolute(ps("8"))),
                        None,
                        None,
                    );
                    let mut seed_mutations = Mutations::new();
                    apply_adj(&policy, account(1), &adjustment, &mut seed_mutations).unwrap();
                    assert!(!seed_mutations.commit_all().failed());
                    let before = snapshot(&policy, &order.inner);
                    let mut mutations = Mutations::new();
                    let (marks, outcome) = paired_check(
                        &policy,
                        &account_context(false, order.review.confirmed, order.review.already_held),
                        &order,
                        &mut mutations,
                    );
                    let rejects: Vec<_> = marks
                        .iter()
                        .filter(|mark| mark.mode == RuleMode::Reject)
                        .cloned()
                        .collect();
                    let actual = (rejects.clone(), outcome_values(outcome.clone()));
                    if let Some(track) = &track {
                        assert_eq!(
                            &actual, track,
                            "same-asset verdict and outcomes must match TrackOnly"
                        );
                    } else {
                        track = Some(actual);
                    }
                    if mode == SpotFundsLimitMode::ReviewWithoutHold && initial == "0" {
                        assert_eq!(
                            snapshot(&policy, &order.inner),
                            before,
                            "same-asset pure checks must not write"
                        );
                        assert!(mutations.is_empty());
                    }
                    if rejects.is_empty() && initial == "0" && mode != SpotFundsLimitMode::TrackOnly
                    {
                        assert_eq!(marks.len(), 1);
                        assert_eq!(marks[0].code, RejectCode::InsufficientFunds);
                    }
                    assert!(!mutations.rollback_all().failure().failed());
                }
            }
        }
    }
}

#[test]
fn review_funds_follow_account_group_global_cascade() {
    let order = order(Side::Buy, false);
    for (mode, rule) in REVIEW_MODES {
        for tier in ["account", "group", "global"] {
            let mut configured = settings(0);
            configured.set_global_limit_mode(if tier == "global" {
                mode
            } else {
                SpotFundsLimitMode::Enforce
            });
            if tier != "global" {
                configured.set_account_group_limit_mode(
                    group(7),
                    Some(if tier == "group" {
                        mode
                    } else {
                        SpotFundsLimitMode::Enforce
                    }),
                );
            }
            if tier == "account" {
                configured.set_account_limit_mode(account(1), Some(mode));
            }
            let policy = build_policy_from_settings(configured, None);
            let mut mutations = Mutations::new();
            let (marks, outcome) = paired_check(
                &policy,
                &ctx_with_group(account(1), group(7)),
                &order,
                &mut mutations,
            );
            assert_eq!(marks.len(), 1, "resolved review tier must produce a mark");
            assert_eq!(
                marks[0].mode, rule,
                "the most specific configured tier wins"
            );
            assert_eq!(marks[0].code, RejectCode::InsufficientFunds);
            assert!(outcome.is_some());
            assert_eq!(
                mutations.is_empty(),
                mode == SpotFundsLimitMode::ReviewWithoutHold
            );
            assert!(!mutations.rollback_all().failure().failed());
        }
    }
}

#[test]
fn confirmed_review_funds_hold_as_track_only_without_marks() {
    for side in [Side::Buy, Side::Sell] {
        let mut order = order(side, false);
        order.review.confirmed = true;
        let mut track = None;
        for mode in [
            SpotFundsLimitMode::TrackOnly,
            SpotFundsLimitMode::ReviewWithHold,
            SpotFundsLimitMode::ReviewWithoutHold,
        ] {
            let policy = build_policy_with_limit_mode(mode);
            let mut mutations = Mutations::new();
            let (marks, outcome) = paired_check(
                &policy,
                &account_context(false, order.review.confirmed, order.review.already_held),
                &order,
                &mut mutations,
            );
            assert!(
                marks.is_empty(),
                "confirmed review must not mark a shortfall"
            );
            assert!(!mutations.is_empty(), "confirmed review must reserve funds");
            let actual = (
                outcome_values(Some(outcome.unwrap())),
                snapshot(&policy, &order.inner),
            );
            if let Some(track) = &track {
                assert_eq!(&actual, track, "confirmed review must match TrackOnly");
            } else {
                track = Some(actual);
            }
            assert!(!mutations.rollback_all().failure().failed());
        }
    }
}

#[test]
fn already_held_review_funds_drop_marks_without_a_second_hold() {
    for side in [Side::Buy, Side::Sell] {
        for (mode, _) in REVIEW_MODES {
            let policy = build_policy_with_limit_mode(SpotFundsLimitMode::TrackOnly);
            let mut order = order(side, false);
            let mut mutations = Mutations::new();
            let (marks, _) = paired_check(
                &policy,
                &account_context(false, order.review.confirmed, order.review.already_held),
                &order,
                &mut mutations,
            );
            assert!(marks.is_empty());
            assert!(!mutations.commit_all().failed());
            let mut mutations = Mutations::new();
            let before = snapshot(&policy, &order.inner);
            set_runtime_global_limit_mode(&policy, mode);
            order.review.already_held = true;
            for confirmed in [false, true] {
                order.review.confirmed = confirmed;
                let (marks, outcome) = paired_check(
                    &policy,
                    &account_context(false, order.review.confirmed, order.review.already_held),
                    &order,
                    &mut mutations,
                );
                assert!(marks.is_empty(), "already-held review must drop every mark");
                assert!(
                    outcome.is_none(),
                    "already-held review must return Ok(None)"
                );
                assert!(
                    mutations.is_empty(),
                    "already-held review must not register rollback"
                );
                assert_eq!(
                    snapshot(&policy, &order.inner),
                    before,
                    "already-held review must not hold twice"
                );
            }
            set_runtime_global_limit_mode(&policy, SpotFundsLimitMode::Enforce);
            let (rejects, outcome) = paired_check(
                &policy,
                &account_context(false, order.review.confirmed, order.review.already_held),
                &order,
                &mut mutations,
            );
            assert_eq!(
                rejects[0].code,
                RejectCode::InsufficientFunds,
                "Enforce must still reject the rebuilt short state"
            );
            assert_eq!(rejects[0].mode, RuleMode::Reject);
            assert!(outcome.is_none());
            assert_eq!(snapshot(&policy, &order.inner), before);
        }
    }
}

#[test]
fn review_funds_deduplicate_after_pnl_before_position_and_reject_still_wins() {
    let order = order(Side::Buy, false);
    for (mode, rule) in REVIEW_MODES {
        for pnl_first in [false, true] {
            for effective_reject in [false, true] {
                let mut configured = settings(0);
                configured.set_global_limit_mode(mode);
                configured.set_position_limit(account(1), asset("USD"), rule, Some(qty("1")));
                configured.set_position_limit(
                    account(1),
                    asset("AAPL"),
                    if effective_reject {
                        RuleMode::Reject
                    } else {
                        rule
                    },
                    Some(qty("1")),
                );
                if pnl_first {
                    configured
                        .set_pnl_global_barriers([SpotFundsPnlBoundsBarrier {
                            mode: rule,
                            currency: asset("USD"),
                            lower_bound: Some(pnl_value("-10")),
                            upper_bound: None,
                        }])
                        .unwrap();
                }
                let policy = build_policy_from_settings(configured, None);
                policy.set_account_pnl_state(account(1), crate::PnlState::Value(pnl_value("-20")));
                let mut mutations = Mutations::new();
                let (marks, outcome) = paired_check(
                    &policy,
                    &account_context(false, order.review.confirmed, order.review.already_held),
                    &order,
                    &mut mutations,
                );
                assert_eq!(marks.len(), 1, "only the first mark of a mode survives");
                if effective_reject {
                    assert_eq!(
                        marks[0].code,
                        RejectCode::PositionLimitExceeded,
                        "effective reject must defeat all review marks"
                    );
                    assert_eq!(marks[0].mode, RuleMode::Reject);
                    assert!(outcome.is_none());
                } else {
                    assert_eq!(marks[0].mode, rule);
                    assert_eq!(
                        marks[0].code,
                        if pnl_first {
                            RejectCode::PnlKillSwitchTriggered
                        } else {
                            RejectCode::InsufficientFunds
                        },
                        "keep P&L before funds before position"
                    );
                    assert!(outcome.is_some());
                }
                if mode == SpotFundsLimitMode::ReviewWithHold {
                    assert_eq!(
                        holdings_of(&policy, account(1), &asset("USD"))
                            .unwrap()
                            .held(),
                        ps("5"),
                        "deduplication must not skip the hold"
                    );
                } else {
                    assert!(
                        policy.holdings.keys().is_empty(),
                        "deduplication must not enable a without-hold write"
                    );
                    assert!(mutations.is_empty());
                }
                assert!(!mutations.rollback_all().failure().failed());
            }
        }
    }
}

#[test]
fn drop_copy_review_funds_force_track_only() {
    for (mode, _) in REVIEW_MODES {
        let order = order(Side::Buy, false);
        let mut expected = None;
        for mode in [SpotFundsLimitMode::TrackOnly, mode] {
            let mut configured = settings(0);
            configured.set_global_limit_mode(mode);
            configured.set_position_limit(
                account(1),
                asset("AAPL"),
                RuleMode::Reject,
                Some(qty("1")),
            );
            let policy = build_policy_from_settings(configured, None);
            let mut mutations = Mutations::new();
            let (marks, outcome) = paired_check(
                &policy,
                &account_context(true, order.review.confirmed, order.review.already_held),
                &order,
                &mut mutations,
            );
            assert!(
                marks.is_empty(),
                "drop-copy must bypass fund and position gates"
            );
            assert!(!mutations.is_empty());
            let actual = (
                outcome_values(Some(outcome.unwrap())),
                snapshot(&policy, &order.inner),
            );
            if let Some(expected) = &expected {
                assert_eq!(&actual, expected);
            } else {
                expected = Some(actual);
            }
            assert!(!mutations.rollback_all().failure().failed());
        }
    }
}

#[test]
fn review_funds_overflow_stays_an_effective_reject() {
    for (mode, _) in REVIEW_MODES {
        let policy = build_policy_with_limit_mode(mode);
        let adjustment = held_adj(
            asset("USD"),
            Some(AdjustmentAmount::Absolute(PositionSize::new(
                rust_decimal::Decimal::MAX,
            ))),
            None,
            None,
        );
        let mut mutations = Mutations::new();
        apply_adj(&policy, account(1), &adjustment, &mut mutations).unwrap();
        assert!(!mutations.commit_all().failed());
        let mut mutations = Mutations::new();
        let order = order(Side::Buy, false);
        let before = snapshot(&policy, &order.inner);
        let (rejects, outcome) = paired_check(
            &policy,
            &account_context(false, order.review.confirmed, order.review.already_held),
            &order,
            &mut mutations,
        );
        assert_eq!(rejects.len(), 1);
        assert_eq!(
            rejects[0].code,
            RejectCode::ArithmeticOverflow,
            "review must not suppress hold overflow"
        );
        assert_eq!(rejects[0].mode, RuleMode::Reject);
        assert!(outcome.is_none());
        assert!(mutations.is_empty());
        assert_eq!(snapshot(&policy, &order.inner), before);
    }
}

#[test]
fn without_hold_short_second_leg_preserves_the_first_until_rollback() {
    let policy = build_policy_with_limit_mode(SpotFundsLimitMode::ReviewWithoutHold);
    let mut order = order(Side::Sell, false);
    order.inner.price = Some(px("-0.5"));
    seed(&policy, account(1), asset("AAPL"), "10");
    let before = snapshot(&policy, &order.inner);
    let mut mutations = Mutations::new();
    let (marks, outcome) = paired_check(
        &policy,
        &account_context(false, order.review.confirmed, order.review.already_held),
        &order,
        &mut mutations,
    );
    assert_eq!(marks.len(), 1);
    assert_eq!(marks[0].mode, RuleMode::ReviewWithoutHold);
    assert!(marks[0].details.contains("USD"));
    assert_eq!(outcome.unwrap().account_adjustments.len(), 2);
    assert_eq!(
        holdings_of(&policy, account(1), &asset("AAPL"))
            .unwrap()
            .held(),
        ps("10"),
        "sufficient first leg must stay written until engine rollback"
    );
    assert!(
        holdings_of(&policy, account(1), &asset("USD")).is_none(),
        "short second leg must not leave a slot"
    );
    assert!(!mutations.is_empty(), "first leg must retain its rollback");
    assert!(!mutations.rollback_all().failure().failed());
    assert_eq!(snapshot(&policy, &order.inner), before);
}

#[test]
fn two_short_legs_keep_the_first_funds_mark_and_project_both_holds() {
    for (mode, rule) in REVIEW_MODES {
        for same_asset in [false, true] {
            let policy = build_policy_with_limit_mode(mode);
            let mut order = order(Side::Sell, same_asset);
            order.inner.price = Some(px("-0.5"));
            let mut mutations = Mutations::new();
            let (marks, outcome) = paired_check(
                &policy,
                &account_context(false, order.review.confirmed, order.review.already_held),
                &order,
                &mut mutations,
            );
            assert_eq!(
                marks.len(),
                1,
                "fund marks must deduplicate across held legs"
            );
            assert_eq!(marks[0].mode, rule);
            assert_eq!(marks[0].code, RejectCode::InsufficientFunds);
            assert!(
                marks[0]
                    .details
                    .contains(if same_asset { "USD" } else { "AAPL" }),
                "the first held asset must own the mark"
            );
            assert!(
                marks[0].details.contains("requested 10"),
                "the first held amount must own the mark"
            );
            let outcome = outcome.unwrap();
            assert_eq!(outcome.account_adjustments.len(), 2);
            assert_eq!(
                outcome.account_adjustments[1].held.unwrap().absolute,
                if same_asset { ps("15") } else { ps("5") }
            );
            if mode == SpotFundsLimitMode::ReviewWithoutHold {
                assert!(policy.holdings.keys().is_empty());
                assert!(mutations.is_empty());
            } else {
                assert_eq!(
                    holdings_of(&policy, account(1), &asset("USD"))
                        .unwrap()
                        .held(),
                    if same_asset { ps("15") } else { ps("5") }
                );
            }
            assert!(!mutations.rollback_all().failure().failed());
        }
    }
}

#[test]
fn unreadable_confirmed_rejects_only_resolved_non_drop_copy_review_modes() {
    let order = order(Side::Buy, false);
    for mode in [
        SpotFundsLimitMode::Enforce,
        SpotFundsLimitMode::TrackOnly,
        SpotFundsLimitMode::ReviewWithHold,
        SpotFundsLimitMode::ReviewWithoutHold,
    ] {
        for drop_copy in [false, true] {
            let policy = build_policy_with_limit_mode(mode);
            let builder = StorageBuilder::new(FullLocking);
            let groups = AccountGroupsHandle::from_inner(FullLocking::new_shared(
                AccountGroups::new(&builder),
            ));
            let error = RequestFieldAccessError::new("unreadable confirmed");
            let expected = crate::pretrade::policy::missing_required_field_reject(
                &policy,
                "confirmed",
                &error,
            );
            let confirmed = Err(error);
            let ctx = if drop_copy {
                let engine = build_account_pnl_test_engine(settings(0));
                PreTradeContext::with_accounts_and_drop_copy(
                    dummy_control(account(1)),
                    engine.accounts(),
                    groups,
                    account(1),
                    confirmed,
                    Ok(false),
                )
            } else {
                PreTradeContext::with_groups(None, groups, Ok(account(1)), confirmed, Ok(false))
            };
            let mut mutations = Mutations::new();
            let (marks, outcome) = paired_check(&policy, &ctx, &order, &mut mutations);
            if !drop_copy
                && matches!(
                    mode,
                    SpotFundsLimitMode::ReviewWithHold | SpotFundsLimitMode::ReviewWithoutHold
                )
            {
                assert_eq!(marks.len(), 1);
                assert_eq!(
                    marks[0].code,
                    RejectCode::MissingRequiredField,
                    "unreadable confirmed must reject instead of defaulting"
                );
                assert_eq!(marks[0].mode, RuleMode::Reject);
                assert_eq!(
                    marks,
                    vec![expected],
                    "confirmed access failure must name the required field"
                );
                assert!(outcome.is_none());
                assert!(mutations.is_empty());
            } else if !drop_copy && mode == SpotFundsLimitMode::Enforce {
                assert_eq!(
                    marks[0].code,
                    RejectCode::InsufficientFunds,
                    "Enforce must not read confirmed"
                );
            } else {
                assert!(
                    marks.is_empty(),
                    "TrackOnly and drop-copy must not read confirmed"
                );
                assert!(outcome.is_some());
            }
            assert!(!mutations.rollback_all().failure().failed());
        }
    }
}

#[test]
fn review_with_hold_final_cancel_releases_both_legs() {
    let policy = build_policy_with_limit_mode(SpotFundsLimitMode::ReviewWithHold);
    let order = order(Side::Buy, false);
    let mut mutations = Mutations::new();
    let (marks, outcome) = paired_check(
        &policy,
        &account_context(false, order.review.confirmed, order.review.already_held),
        &order,
        &mut mutations,
    );
    assert_eq!(marks[0].mode, RuleMode::ReviewWithHold);
    let outcome = outcome.unwrap();
    assert!(!mutations.commit_all().failed());
    assert_eq!(
        holdings_of(&policy, account(1), &asset("USD"))
            .unwrap()
            .held(),
        ps("5")
    );
    let report = make_report(
        account(1),
        order.inner.instrument.clone(),
        Side::Buy,
        None,
        qty("10"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            outcome.lock_prices[0],
        )])),
    );
    let result = run_report(&policy, &report);
    assert!(result.account_blocks.is_empty());
    for asset_name in ["USD", "AAPL"] {
        assert!(
            holdings_of(&policy, account(1), &asset(asset_name))
                .is_none_or(|holdings| holdings.is_zero()),
            "final cancel must release the hold and incoming"
        );
    }
}
