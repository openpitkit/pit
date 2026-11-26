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
use crate::{
    HasAlreadyHeld, HasConfirmed, HasOrderPrice, HasTradeAmount, OrderReview, WithOrderReview,
};

fn reviewed(inner: TestOrder, confirmed: bool) -> WithOrderReview<TestOrder> {
    WithOrderReview {
        inner,
        review: OrderReview {
            confirmed,
            already_held: true,
        },
    }
}

fn buy() -> TestOrder {
    make_order(
        account(1),
        instr("AAPL", "USD"),
        Side::Buy,
        TradeAmount::Quantity(qty("2")),
        Some(px("10")),
    )
}

fn commit_hold(policy: &TestPolicy, order: &TestOrder) -> PolicyPreTradeResult {
    let mut mutations = Mutations::new();
    let outcome = pre_trade_full(policy, order, &mut mutations).unwrap();
    assert!(!mutations.is_empty());
    assert!(!mutations.commit_all().failed());
    outcome
}

pub(super) fn run_rerun_checks<Snapshot: std::fmt::Debug + PartialEq>(
    policy: &TestPolicy,
    order: &WithOrderReview<TestOrder>,
    snapshot: impl Fn() -> Snapshot,
) -> Result<[Result<Option<PolicyPreTradeResult>, Rejects>; 2], String> {
    let before = snapshot();
    let ctx = PreTradeContext::new_with_order_review(None, order);
    let mut mutations = Mutations::new();
    let dry = review_dry_run(policy, &ctx, order, &mut mutations);
    if !mutations.is_empty() {
        return Err("dry rerun pushed mutations".into());
    }
    let after_dry = snapshot();
    if after_dry != before {
        return Err(format!(
            "dry rerun changed holdings: {before:?} -> {after_dry:?}"
        ));
    }
    let live = review_check(policy, &ctx, order, &mut mutations);
    if !mutations.is_empty() {
        return Err("live rerun pushed mutations".into());
    }
    let after_live = snapshot();
    if after_live != before {
        return Err(format!(
            "live rerun changed holdings: {before:?} -> {after_live:?}"
        ));
    }
    Ok([dry, live])
}

fn rerun(policy: &TestPolicy, order: &WithOrderReview<TestOrder>) -> Result<(), Rejects> {
    let [dry, live] = run_rerun_checks(policy, order, || snapshot(policy, &order.inner))
        .unwrap_or_else(|message| panic!("{message}"));
    match (dry, live) {
        (Ok(None), Ok(None)) => Ok(()),
        (Err(dry), Err(live)) => {
            assert_eq!(dry, live);
            Err(live)
        }
        (dry, live) => panic!("reruns must return None or identical rejects: {dry:?}, {live:?}"),
    }
}

#[test]
fn reruns_preserve_every_holding_field() {
    let policy = build_policy_from_settings(settings(0), None);
    let order = buy();
    seed_with_avg(&policy, account(1), asset("USD"), "100", px("3"));
    seed_with_avg(&policy, account(1), asset("AAPL"), "5", px("7"));
    let mut mutations = Mutations::new();
    apply_adj(
        &policy,
        account(1),
        &adj_with_realized_pnl(asset("AAPL"), pnl_value("11")),
        &mut mutations,
    )
    .unwrap();
    assert!(!mutations.commit_all().failed());
    commit_hold(&policy, &order);
    let before = snapshot(&policy, &order);
    assert_balance(&policy, account(1), "USD", "80", "20");
    assert_balance(&policy, account(1), "AAPL", "5", "0");
    let usd = before[1].unwrap();
    let aapl = before[0].unwrap();
    assert_eq!(usd.incoming(), PositionSize::ZERO);
    assert_eq!(usd.avg_entry_price(), Some(px("3")));
    assert_eq!(usd.realized_pnl(), Some(Pnl::ZERO));
    assert_eq!(aapl.avg_entry_price(), Some(px("7")));
    assert_eq!(aapl.realized_pnl(), Some(pnl_value("11")));
    assert_eq!(
        holdings_of(&policy, account(1), &asset("AAPL"))
            .unwrap()
            .incoming(),
        ps("2")
    );
    rerun(&policy, &reviewed(order.clone(), true)).unwrap();
    assert_eq!(snapshot(&policy, &order), before);
}

#[test]
fn rerun_counts_exhausted_funds_once() {
    let policy = build_policy_from_settings(settings(0), None);
    let order = buy();
    seed(&policy, account(1), asset("USD"), "20");
    commit_hold(&policy, &order);
    assert_balance(&policy, account(1), "USD", "0", "20");
    rerun(&policy, &reviewed(order, true)).unwrap();
}

#[test]
fn rerun_counts_position_projection_once() {
    let mut configured = settings(0);
    configured.set_position_limit(account(1), asset("AAPL"), RuleMode::Reject, Some(qty("2")));
    let policy = build_policy_from_settings(configured, None);
    let order = buy();
    seed(&policy, account(1), asset("USD"), "100");
    commit_hold(&policy, &order);
    rerun(&policy, &reviewed(order, true)).unwrap();
}

#[test]
fn rerun_rebuilds_both_legs_of_the_same_asset_once() {
    let mut configured = settings(0);
    configured.set_position_limit(account(1), asset("USD"), RuleMode::Reject, Some(qty("2")));
    let policy = build_policy_from_settings(configured, None);
    let order = make_order(
        account(1),
        instr("USD", "USD"),
        Side::Buy,
        TradeAmount::Quantity(qty("2")),
        Some(px("0.5")),
    );
    seed(&policy, account(1), asset("USD"), "1");
    commit_hold(&policy, &order);
    assert_balance(&policy, account(1), "USD", "0", "1");
    assert_eq!(
        holdings_of(&policy, account(1), &asset("USD"))
            .unwrap()
            .incoming(),
        ps("2")
    );
    rerun(&policy, &reviewed(order, true)).unwrap();
}

#[test]
fn shrunk_available_rejects_in_enforce_without_releasing_the_hold() {
    let policy = build_policy_from_settings(settings(0), None);
    let order = buy();
    seed(&policy, account(1), asset("USD"), "20");
    commit_hold(&policy, &order);
    seed(&policy, account(1), asset("USD"), "-1");
    let rejects = rerun(&policy, &reviewed(order, true)).unwrap_err();
    assert_eq!(rejects.len(), 1);
    assert_eq!(rejects[0].code, RejectCode::InsufficientFunds);
    assert_eq!(rejects[0].scope, RejectScope::Order);
    // The ordinary dry-run on the independently released balance is the
    // same funds check, including its diagnostic amounts.
    let released = build_policy_from_settings(settings(0), None);
    seed(&released, account(1), asset("USD"), "19");
    assert_eq!(rejects, dry_run_check(&released, &buy()).unwrap_err());
    assert_balance(&policy, account(1), "USD", "-1", "20");
}

#[test]
fn shrunk_available_passes_in_track_only_without_writing() {
    let mut configured = settings(0);
    configured.set_global_limit_mode(SpotFundsLimitMode::TrackOnly);
    let policy = build_policy_from_settings(configured, None);
    let order = buy();
    seed(&policy, account(1), asset("USD"), "20");
    commit_hold(&policy, &order);
    seed(&policy, account(1), asset("USD"), "-1");
    rerun(&policy, &reviewed(order, true)).unwrap();
    assert_balance(&policy, account(1), "USD", "-1", "20");
}

#[test]
fn independent_position_breach_rejects_without_releasing_the_hold() {
    let mut configured = settings(0);
    configured.set_position_limit(account(1), asset("AAPL"), RuleMode::Reject, Some(qty("2")));
    let policy = build_policy_from_settings(configured, None);
    let order = buy();
    seed(&policy, account(1), asset("USD"), "100");
    commit_hold(&policy, &order);
    seed(&policy, account(1), asset("AAPL"), "1");
    let rejects = rerun(&policy, &reviewed(order, true)).unwrap_err();
    assert_eq!(rejects.len(), 1);
    assert_eq!(rejects[0].code, RejectCode::PositionLimitExceeded);
    assert_eq!(rejects[0].scope, RejectScope::Order);
    assert_balance(&policy, account(1), "USD", "80", "20");
}

fn account_context(drop_copy: bool, review: bool) -> PreTradeContext<crate::storage::FullLocking> {
    super::account_context(drop_copy, review, review)
}

#[test]
fn already_held_comes_from_context_instead_of_order() {
    let policy = build_policy_from_settings(settings(0), None);
    let order = WithOrderReview {
        inner: buy(),
        review: OrderReview {
            confirmed: false,
            already_held: false,
        },
    };
    seed(&policy, account(1), asset("USD"), "100");
    commit_hold(&policy, &order.inner);
    let before = snapshot(&policy, &order.inner);
    let ctx = account_context(false, true);
    let mut mutations = Mutations::new();

    assert!(!order.already_held().unwrap());
    assert!(ctx.already_held().unwrap());
    assert!(review_dry_run(&policy, &ctx, &order, &mut mutations)
        .unwrap()
        .is_none());
    assert!(mutations.is_empty());
    assert_eq!(snapshot(&policy, &order.inner), before);
    assert!(review_check(&policy, &ctx, &order, &mut mutations)
        .unwrap()
        .is_none());
    assert!(mutations.is_empty());
    assert_eq!(snapshot(&policy, &order.inner), before);
}

#[test]
fn rerun_returns_breached_pnl_review_marks_without_a_policy_result() {
    for mode in [RuleMode::ReviewWithHold, RuleMode::ReviewWithoutHold] {
        let policy = build_policy_with_pnl_global_barrier(SpotFundsPnlBoundsBarrier {
            mode,
            currency: asset("USD"),
            lower_bound: Some(pnl_value("-10")),
            upper_bound: None,
        });
        let order = buy();
        seed(&policy, account(1), asset("USD"), "100");
        policy.set_account_pnl_state(account(1), crate::PnlState::Value(Pnl::ZERO));
        commit_hold(&policy, &order);
        policy.set_account_pnl_state(account(1), crate::PnlState::Value(pnl_value("-20")));
        let marks = dry_run_check(&policy, &order).unwrap_err();
        assert_eq!(marks.len(), 1);
        assert_eq!(marks[0].code, RejectCode::PnlKillSwitchTriggered);
        assert_eq!(marks[0].scope, RejectScope::Account);
        assert_eq!(marks[0].mode, mode);
        let order = reviewed(order, false);
        for result in run_rerun_checks(&policy, &order, || snapshot(&policy, &order.inner)).unwrap()
        {
            let (rerun_marks, outcome) = result.unwrap_err().into_parts();
            assert_eq!(rerun_marks, marks.clone().into_vec());
            assert!(
                outcome.is_none(),
                "already-held review must carry no policy result"
            );
        }
    }
}

#[test]
fn rerun_returns_breached_position_review_marks_without_a_policy_result() {
    for mode in [RuleMode::ReviewWithHold, RuleMode::ReviewWithoutHold] {
        let mut configured = settings(0);
        configured.set_position_limit(account(1), asset("AAPL"), mode, Some(qty("2")));
        let policy = build_policy_from_settings(configured, None);
        let order = buy();
        seed(&policy, account(1), asset("USD"), "100");
        commit_hold(&policy, &order);
        seed(&policy, account(1), asset("AAPL"), "1");
        let mut configured = settings(0);
        configured.set_position_limit(account(1), asset("AAPL"), mode, Some(qty("2")));
        let control = build_policy_from_settings(configured, None);
        seed(&control, account(1), asset("USD"), "100");
        seed(&control, account(1), asset("AAPL"), "1");
        let marks = dry_run_check(&control, &order).unwrap_err();
        assert_eq!(marks.len(), 1);
        assert_eq!(marks[0].code, RejectCode::PositionLimitExceeded);
        assert_eq!(marks[0].scope, RejectScope::Order);
        assert_eq!(marks[0].mode, mode);
        let order = reviewed(order, false);
        for result in run_rerun_checks(&policy, &order, || snapshot(&policy, &order.inner)).unwrap()
        {
            let (rerun_marks, outcome) = result.unwrap_err().into_parts();
            assert_eq!(rerun_marks, marks.clone().into_vec());
            assert!(
                outcome.is_none(),
                "already-held review must carry no policy result"
            );
        }
    }
}

#[test]
fn halted_account_rejects_reruns_before_reservation_checks() {
    let policy = build_policy_with_pnl_global_barrier(SpotFundsPnlBoundsBarrier {
        mode: RuleMode::Reject,
        currency: asset("USD"),
        lower_bound: Some(pnl_value("-10")),
        upper_bound: None,
    });
    let order = buy();
    seed(&policy, account(1), asset("USD"), "20");
    commit_hold(&policy, &order);
    seed(&policy, account(1), asset("USD"), "-1");
    policy.set_account_pnl_state(
        account(1),
        crate::PnlState::Halted(crate::PnlHaltReason::MissingFx),
    );
    let before = snapshot(&policy, &order);
    let ctx = account_context(false, false);
    let mut mutations = Mutations::new();
    let expected = <TestPolicy as PreTradePolicy<
        TestOrder,
        TestReport,
        TestAdjustment,
        FullSync,
    >>::perform_pre_trade_check(&policy, &ctx, &order, &mut mutations)
    .unwrap_err();
    assert_eq!(expected.len(), 1);
    assert_eq!(expected[0].code, RejectCode::PnlKillSwitchTriggered);
    assert_eq!(expected[0].scope, RejectScope::Account);
    let order = reviewed(order, true);
    let ctx = account_context(false, true);
    assert_eq!(
        review_check(&policy, &ctx, &order, &mut mutations).unwrap_err(),
        expected
    );
    assert_eq!(
        review_dry_run(&policy, &ctx, &order, &mut mutations).unwrap_err(),
        expected
    );
    assert!(mutations.is_empty());
    assert_eq!(snapshot(&policy, &order.inner), before);
}

#[test]
fn drop_copy_without_price_keeps_the_missing_field_reject() {
    let policy = build_policy_from_settings(settings(0), None);
    let mut plain = buy();
    plain.price = None;
    let ctx = account_context(true, false);
    let mut mutations = Mutations::new();
    let expected = <TestPolicy as PreTradePolicy<
        TestOrder,
        TestReport,
        TestAdjustment,
        FullSync,
    >>::perform_pre_trade_check(&policy, &ctx, &plain, &mut mutations)
    .unwrap_err();
    let order = reviewed(plain, true);
    let before = snapshot(&policy, &order.inner);
    let ctx = account_context(true, true);
    assert_eq!(
        review_check(&policy, &ctx, &order, &mut mutations).unwrap_err(),
        expected
    );
    assert_eq!(expected.len(), 1);
    assert_eq!(expected[0].code, RejectCode::MissingRequiredField);
    assert_eq!(expected[0].scope, RejectScope::Order);
    assert_eq!(expected[0].reason, "limit price is required for drop-copy");
    assert!(mutations.is_empty());
    assert_eq!(snapshot(&policy, &order.inner), before);
}

#[test]
fn priced_drop_copy_rerun_returns_none_without_writing() {
    let policy = build_policy_from_settings(settings(0), None);
    let plain = buy();
    seed(&policy, account(1), asset("USD"), "20");
    commit_hold(&policy, &plain);
    let before = snapshot(&policy, &plain);
    let order = reviewed(plain, true);
    let ctx = account_context(true, true);
    let mut mutations = Mutations::new();
    assert!(matches!(
        review_check(&policy, &ctx, &order, &mut mutations),
        Ok(None)
    ));
    assert!(mutations.is_empty());
    assert!(ctx.drop_copy_account_block().is_none());
    assert_eq!(snapshot(&policy, &order.inner), before);
}

#[test]
fn halted_drop_copy_records_the_same_account_block_without_a_hold_write() {
    let policy = build_policy_with_pnl_global_barrier(SpotFundsPnlBoundsBarrier {
        mode: RuleMode::Reject,
        currency: asset("USD"),
        lower_bound: Some(pnl_value("-10")),
        upper_bound: None,
    });
    let plain = buy();
    seed(&policy, account(1), asset("USD"), "20");
    commit_hold(&policy, &plain);
    policy.set_account_pnl_state(
        account(1),
        crate::PnlState::Halted(crate::PnlHaltReason::MissingFx),
    );
    let normal_ctx = account_context(true, false);
    let mut normal_mutations = Mutations::new();
    <TestPolicy as PreTradePolicy<TestOrder, TestReport, TestAdjustment, FullSync>>::perform_pre_trade_check(
        &policy, &normal_ctx, &plain, &mut normal_mutations,
    ).unwrap();
    let expected = normal_ctx.drop_copy_account_block().unwrap();
    assert!(!normal_mutations.rollback_all().callback_failed());
    let before = snapshot(&policy, &plain);
    let ctx = account_context(true, true);
    let order = reviewed(plain, true);
    let mut mutations = Mutations::new();
    assert!(matches!(
        review_check(&policy, &ctx, &order, &mut mutations),
        Ok(None)
    ));
    assert_eq!(ctx.drop_copy_account_block(), Some(expected));
    assert!(mutations.is_empty());
    assert_eq!(snapshot(&policy, &order.inner), before);
}

#[test]
fn rebuild_overflow_rejects_without_writing() {
    let policy = build_policy_from_settings(settings(0), None);
    let order = buy();
    let slot = Holdings::new(position_size_max(), ps("20"));
    policy
        .holdings
        .with_mut((account(1), asset("USD")), Holdings::zero, |current, _| {
            *current = slot;
        });
    let rejects = rerun(&policy, &reviewed(order, true)).unwrap_err();
    assert_eq!(rejects.len(), 1);
    assert_eq!(rejects[0].code, RejectCode::ArithmeticOverflow);
    assert_eq!(rejects[0].scope, RejectScope::Order);
    assert_eq!(
        rejects[0].details,
        format!(
            "pre-trade hold rebuild rollback overflow: asset USD, \
             held 20, incoming 0, slot {slot:?}"
        )
    );
}

#[test]
fn same_asset_rebuild_sum_overflow_rejects_without_writing() {
    let policy = build_policy_from_settings(settings(0), None);
    let order = make_order(
        account(1),
        instr("USD", "USD"),
        Side::Sell,
        TradeAmount::Quantity(qty("40000000000000000000000000000")),
        Some(px("-1")),
    );
    seed(&policy, account(1), asset("USD"), "20");
    let rejects = rerun(&policy, &reviewed(order, true)).unwrap_err();
    assert_eq!(rejects.len(), 1);
    assert_eq!(rejects[0].code, RejectCode::ArithmeticOverflow);
    assert_eq!(rejects[0].scope, RejectScope::Order);
    assert_eq!(
        rejects[0].details,
        "pre-trade hold rebuild sum overflow: asset USD, \
         held 40000000000000000000000000000 + 40000000000000000000000000000"
    );
}

#[test]
fn zero_legs_need_no_existing_hold_and_create_no_slots() {
    let policy = build_policy_from_settings(settings(0), None);
    let mut order = buy();
    order.trade_amount = TradeAmount::Quantity(Quantity::ZERO);
    rerun(&policy, &reviewed(order.clone(), false)).unwrap();
    assert_eq!(snapshot(&policy, &order), [None, None]);
}

#[test]
fn confirmed_does_not_change_the_already_held_verdict() {
    let policy = build_policy_from_settings(settings(0), None);
    let order = buy();
    seed(&policy, account(1), asset("USD"), "20");
    commit_hold(&policy, &order);
    rerun(&policy, &reviewed(order.clone(), true)).unwrap();
    rerun(&policy, &reviewed(order.clone(), false)).unwrap();
    seed(&policy, account(1), asset("USD"), "-1");
    assert_eq!(
        rerun(&policy, &reviewed(order.clone(), true)),
        rerun(&policy, &reviewed(order, false))
    );
}

#[test]
fn false_flag_matches_plain_order_outcome_and_hold() {
    let plain_policy = build_policy_from_settings(settings(0), None);
    let review_policy = build_policy_from_settings(settings(0), None);
    let plain = buy();
    seed(&plain_policy, account(1), asset("USD"), "20");
    seed(&review_policy, account(1), asset("USD"), "20");
    let expected = commit_hold(&plain_policy, &plain);
    let order = WithOrderReview {
        inner: plain,
        review: OrderReview {
            confirmed: true,
            already_held: false,
        },
    };
    let ctx = PreTradeContext::new_with_order_review(None, &order);
    let mut mutations = Mutations::new();
    let dry = review_dry_run(&review_policy, &ctx, &order, &mut mutations)
        .unwrap()
        .unwrap();
    assert!(mutations.is_empty());
    let live = review_check(&review_policy, &ctx, &order, &mut mutations)
        .unwrap()
        .unwrap();
    assert!(!mutations.is_empty());
    assert_eq!(dry.account_adjustments, expected.account_adjustments);
    assert_eq!(live.account_adjustments, expected.account_adjustments);
    assert_eq!(dry.lock_prices, expected.lock_prices);
    assert_eq!(live.lock_prices, expected.lock_prices);
    assert!(!mutations.commit_all().failed());
    assert_eq!(
        snapshot(&review_policy, &order.inner),
        snapshot(&plain_policy, &order.inner)
    );
}

struct UnreadableReview(TestOrder);

impl HasAccountId for UnreadableReview {
    fn account_id(&self) -> Result<AccountId, RequestFieldAccessError> {
        self.0.account_id()
    }
}
impl HasInstrument for UnreadableReview {
    fn instrument(&self) -> Result<&Instrument, RequestFieldAccessError> {
        self.0.instrument()
    }
}
impl HasSide for UnreadableReview {
    fn side(&self) -> Result<Side, RequestFieldAccessError> {
        self.0.side()
    }
}
impl HasTradeAmount for UnreadableReview {
    fn trade_amount(&self) -> Result<TradeAmount, RequestFieldAccessError> {
        self.0.trade_amount()
    }
}
impl HasOrderPrice for UnreadableReview {
    fn price(&self) -> Result<Option<Price>, RequestFieldAccessError> {
        self.0.price()
    }
}
impl HasAlreadyHeld for UnreadableReview {
    fn already_held(&self) -> Result<bool, RequestFieldAccessError> {
        Err(RequestFieldAccessError::new("unreadable review flag"))
    }
}
impl HasConfirmed for UnreadableReview {
    fn confirmed(&self) -> Result<bool, RequestFieldAccessError> {
        Ok(false)
    }
}

#[test]
fn unreadable_flag_rejects_instead_of_defaulting_to_false() {
    let policy = build_policy_from_settings(settings(0), None);
    seed(&policy, account(1), asset("USD"), "20");
    let order = UnreadableReview(buy());
    let before = snapshot(&policy, &order.0);
    let ctx = PreTradeContext::new_with_order_review(None, &order);
    let mut mutations = Mutations::new();
    let live = <TestPolicy as PreTradePolicy<
        UnreadableReview,
        TestReport,
        TestAdjustment,
        FullSync,
    >>::perform_pre_trade_check(&policy, &ctx, &order, &mut mutations)
    .unwrap_err();
    let dry = <TestPolicy as PreTradePolicy<
        UnreadableReview,
        TestReport,
        TestAdjustment,
        FullSync,
    >>::perform_pre_trade_check_dry_run(&policy, &ctx, &order, &mut mutations)
    .unwrap_err();
    assert_eq!(live, dry);
    assert_eq!(live.len(), 1);
    assert_eq!(live[0].code, RejectCode::MissingRequiredField);
    assert_eq!(live[0].scope, RejectScope::Order);
    assert_eq!(
        live[0].reason,
        "failed to access required field 'already held'"
    );
    assert_eq!(
        live[0].details,
        RequestFieldAccessError::new("unreadable review flag").to_string()
    );
    assert!(mutations.is_empty());
    assert_eq!(snapshot(&policy, &order.0), before);
}
