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

const ACCOUNT: u64 = 99_224_416;
const ONE_PLUS_EPSILON: &str = "1.0000000000000000000000000001";

fn policy_with_position_limit(
    account_id: AccountId,
    limited_asset: Asset,
    limit: &str,
    limit_mode: SpotFundsLimitMode,
) -> TestPolicy {
    let builder = engine_builder();
    let mut settings = settings(0);
    settings.set_global_limit_mode(limit_mode);
    settings.set_position_limit(account_id, limited_asset, Some(qty(limit)));
    SpotFundsPolicy::new(settings, None, builder.storage_builder())
}

fn set_runtime_position_limit(
    policy: &TestPolicy,
    account_id: AccountId,
    limited_asset: Asset,
    limit: Option<&str>,
) {
    use crate::pretrade::ConfigurablePolicy;
    use crate::storage::ConfigCell;

    let cell =
        <TestPolicy as ConfigurablePolicy<crate::storage::FullLocking>>::settings_cell(policy);
    cell.update(|settings| {
        settings.set_position_limit(account_id, limited_asset, limit.map(qty));
        Ok::<(), SpotFundsConfigError>(())
    })
    .expect("position-limit update must publish");
}

fn assert_position_limit_reject(rejects: &Rejects) {
    assert_eq!(rejects[0].scope, RejectScope::Order);
    assert_eq!(rejects[0].code, RejectCode::PositionLimitExceeded);
    assert_eq!(rejects[0].reason, "position limit exceeded");
}

fn assert_position_limit_arithmetic_reject(
    rejects: &Rejects,
    asset: &str,
    stage: &str,
    cause: &str,
) {
    assert_eq!(rejects[0].scope, RejectScope::Order);
    assert_eq!(rejects[0].code, RejectCode::ArithmeticOverflow);
    assert_eq!(
        rejects[0].details,
        format!(
            "position-limit arithmetic failure: asset {asset}, stage {stage}, \
             cause {cause}",
        )
    );
}

fn order(account_id: AccountId, side: Side, quantity: &str, price: &str) -> TestOrder {
    make_order(
        account_id,
        instr("AAPL", "USD"),
        side,
        TradeAmount::Quantity(qty(quantity)),
        Some(px(price)),
    )
}

#[test]
fn inclusive_boundary_and_smallest_decimal_step() {
    let account_id = account(ACCOUNT);
    let policy =
        policy_with_position_limit(account_id, asset("AAPL"), "1", SpotFundsLimitMode::Enforce);
    seed(&policy, account_id, asset("USD"), "10");

    let mut boundary = Mutations::with_capacity(2);
    pre_trade_check(
        &policy,
        &order(account_id, Side::Buy, "1", "1"),
        &mut boundary,
    )
    .expect("a projected position equal to the limit must pass");
    let _ = boundary.rollback_all();

    let mut mutations = Mutations::with_capacity(2);
    let rejects = pre_trade_check(
        &policy,
        &order(account_id, Side::Buy, ONE_PLUS_EPSILON, "1"),
        &mut mutations,
    )
    .expect_err("the smallest representable step beyond the limit must reject");
    assert_position_limit_reject(&rejects);
    let _ = mutations.rollback_all();
}

#[test]
fn position_projection_overflow_is_an_order_reject() {
    let account_id = account(ACCOUNT);
    let maximum = rust_decimal::Decimal::MAX.to_string();
    let policy = policy_with_position_limit(
        account_id,
        asset("AAPL"),
        &maximum,
        SpotFundsLimitMode::Enforce,
    );
    seed(&policy, account_id, asset("AAPL"), &maximum);
    seed(&policy, account_id, asset("USD"), "10");

    let mut mutations = Mutations::with_capacity(2);
    let rejects = pre_trade_check(
        &policy,
        &order(account_id, Side::Buy, "1", "1"),
        &mut mutations,
    )
    .expect_err("overflowing the projected position must reject");
    assert_position_limit_arithmetic_reject(&rejects, "AAPL", "projection", "overflow");
    let _ = mutations.rollback_all();
}

#[test]
fn inexact_position_projection_fails_closed() {
    let account_id = account(ACCOUNT);
    let policy = policy_with_position_limit(
        account_id,
        asset("AAPL"),
        "100000000000",
        SpotFundsLimitMode::Enforce,
    );
    seed(&policy, account_id, asset("AAPL"), "100000000000");

    let mut mutations = Mutations::with_capacity(2);
    let rejects = pre_trade_check(
        &policy,
        &order(account_id, Side::Buy, "0.000000000000000001", "0"),
        &mut mutations,
    )
    .expect_err("an inexact position projection must fail closed");
    assert_position_limit_arithmetic_reject(&rejects, "AAPL", "projection", "inexact");
    let _ = mutations.rollback_all();
}

#[test]
fn inexact_short_position_projection_fails_closed() {
    let account_id = account(ACCOUNT);
    let policy = policy_with_position_limit(
        account_id,
        asset("AAPL"),
        "100000000000",
        SpotFundsLimitMode::TrackOnly,
    );
    seed(&policy, account_id, asset("AAPL"), "-100000000000");

    let mut mutations = Mutations::with_capacity(2);
    let rejects = pre_trade_check(
        &policy,
        &order(account_id, Side::Sell, "0.000000000000000001", "0"),
        &mut mutations,
    )
    .expect_err("an inexact short position projection must fail closed");
    assert_position_limit_arithmetic_reject(&rejects, "AAPL", "projection", "inexact");
    let _ = mutations.rollback_all();
}

#[test]
fn mirrored_inexact_position_projection_fails_closed() {
    let account_id = account(ACCOUNT);
    let policy = build_policy_with_limit_mode(SpotFundsLimitMode::TrackOnly);
    seed(&policy, account_id, asset("AAPL"), ONE_PLUS_EPSILON);

    let mut open_buy = Mutations::with_capacity(2);
    pre_trade_check(
        &policy,
        &order(account_id, Side::Buy, "100", "0"),
        &mut open_buy,
    )
    .expect("the reservation is opened before the limit is configured");
    let _ = open_buy.commit_all();
    set_runtime_position_limit(&policy, account_id, asset("AAPL"), Some("102"));

    let mut mutations = Mutations::with_capacity(2);
    let rejects = pre_trade_check(
        &policy,
        &order(account_id, Side::Buy, "1", "0"),
        &mut mutations,
    )
    .expect_err("an inexact projection with the finer operand first must reject");
    assert_position_limit_arithmetic_reject(&rejects, "AAPL", "projection", "inexact");
    let _ = mutations.rollback_all();
}

#[test]
fn open_buy_counts_until_its_final_cancel() {
    let account_id = account(ACCOUNT);
    let instrument = instr("AAPL", "USD");
    let policy =
        policy_with_position_limit(account_id, asset("AAPL"), "10", SpotFundsLimitMode::Enforce);
    seed(&policy, account_id, asset("USD"), "100");

    let first = order(account_id, Side::Buy, "6", "1");
    let mut first_mutations = Mutations::with_capacity(2);
    pre_trade_check(&policy, &first, &mut first_mutations).expect("first buy must fit");
    let _ = first_mutations.commit_all();
    assert_eq!(incoming_of(&policy, account_id, "AAPL"), ps("6"));

    let second = order(account_id, Side::Buy, "5", "1");
    let mut rejected_mutations = Mutations::with_capacity(2);
    let rejects = pre_trade_check(&policy, &second, &mut rejected_mutations)
        .expect_err("the open first buy must make the second exceed the limit");
    assert_position_limit_reject(&rejects);
    let _ = rejected_mutations.rollback_all();

    let cancel = make_report(
        account_id,
        instrument,
        Side::Buy,
        None,
        qty("6"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("1"),
        )])),
    );
    assert!(report_blocks(&policy, &cancel).is_empty());
    assert_eq!(incoming_of(&policy, account_id, "AAPL"), PositionSize::ZERO);

    let mut after_cancel = Mutations::with_capacity(2);
    pre_trade_check(&policy, &second, &mut after_cancel)
        .expect("the second buy must fit after the first is cancelled");
    let _ = after_cancel.rollback_all();
}

#[test]
fn filled_position_still_counts_after_incoming_is_drained() {
    let account_id = account(ACCOUNT);
    let instrument = instr("AAPL", "USD");
    let policy =
        policy_with_position_limit(account_id, asset("AAPL"), "10", SpotFundsLimitMode::Enforce);
    seed(&policy, account_id, asset("USD"), "100");

    let mut mutations = Mutations::with_capacity(2);
    pre_trade_check(
        &policy,
        &order(account_id, Side::Buy, "10", "1"),
        &mut mutations,
    )
    .expect("the initial buy must fit");
    let _ = mutations.commit_all();
    let fill = make_report(
        account_id,
        instrument,
        Side::Buy,
        Some(Trade {
            price: px("1"),
            quantity: qty("10"),
        }),
        Quantity::ZERO,
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("1"),
        )])),
    );
    assert!(report_blocks(&policy, &fill).is_empty());
    let holdings = holdings_of(&policy, account_id, &asset("AAPL")).expect("slot must exist");
    assert_eq!(holdings.available(), ps("10"));
    assert_eq!(holdings.incoming(), PositionSize::ZERO);

    let mut rejected_mutations = Mutations::with_capacity(2);
    let rejects = pre_trade_check(
        &policy,
        &order(account_id, Side::Buy, "1", "1"),
        &mut rejected_mutations,
    )
    .expect_err("the filled position must keep consuming the limit");
    assert_position_limit_reject(&rejects);
    let _ = rejected_mutations.rollback_all();
}

#[test]
fn partial_fill_then_final_cancel_keeps_fill_and_releases_incoming() {
    let account_id = account(ACCOUNT);
    let instrument = instr("AAPL", "USD");
    let policy =
        policy_with_position_limit(account_id, asset("AAPL"), "10", SpotFundsLimitMode::Enforce);
    seed(&policy, account_id, asset("USD"), "20");

    let mut mutations = Mutations::with_capacity(2);
    pre_trade_check(
        &policy,
        &order(account_id, Side::Buy, "10", "1"),
        &mut mutations,
    )
    .expect("the initial buy must fit");
    let _ = mutations.commit_all();

    let partial = make_report(
        account_id,
        instrument.clone(),
        Side::Buy,
        Some(Trade {
            price: px("1"),
            quantity: qty("4"),
        }),
        qty("6"),
        false,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("1"),
        )])),
    );
    assert!(report_blocks(&policy, &partial).is_empty());
    let after_partial = holdings_of(&policy, account_id, &asset("AAPL")).expect("slot must exist");
    assert_eq!(after_partial.available(), ps("4"));
    assert_eq!(after_partial.incoming(), ps("6"));

    let cancel = make_report(
        account_id,
        instrument,
        Side::Buy,
        None,
        qty("6"),
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("1"),
        )])),
    );
    assert!(report_blocks(&policy, &cancel).is_empty());
    let after_cancel = holdings_of(&policy, account_id, &asset("AAPL")).expect("slot must exist");
    assert_eq!(after_cancel.available(), ps("4"));
    assert_eq!(after_cancel.incoming(), PositionSize::ZERO);

    let mut boundary = Mutations::with_capacity(2);
    pre_trade_check(
        &policy,
        &order(account_id, Side::Buy, "6", "1"),
        &mut boundary,
    )
    .expect("only the filled quantity must remain in the position");
    let _ = boundary.rollback_all();

    let mut rejected = Mutations::with_capacity(2);
    let rejects = pre_trade_check(
        &policy,
        &order(account_id, Side::Buy, "7", "1"),
        &mut rejected,
    )
    .expect_err("the filled quantity must still consume the position limit");
    assert_position_limit_reject(&rejects);
    let _ = rejected.rollback_all();
}

#[test]
fn track_only_short_boundary_is_inclusive() {
    let account_id = account(ACCOUNT);
    let policy = policy_with_position_limit(
        account_id,
        asset("AAPL"),
        "1",
        SpotFundsLimitMode::TrackOnly,
    );

    let mut boundary = Mutations::with_capacity(2);
    pre_trade_check(
        &policy,
        &order(account_id, Side::Sell, "1", "1"),
        &mut boundary,
    )
    .expect("a short projected exactly to the limit must pass");
    let _ = boundary.rollback_all();

    let mut mutations = Mutations::with_capacity(2);
    let rejects = pre_trade_check(
        &policy,
        &order(account_id, Side::Sell, ONE_PLUS_EPSILON, "1"),
        &mut mutations,
    )
    .expect_err("the smallest step beyond the short limit must reject");
    assert_position_limit_reject(&rejects);
    let _ = mutations.rollback_all();
}

#[test]
fn position_reducing_orders_pass_after_runtime_limit_reduction() {
    let account_id = account(ACCOUNT);

    let long_policy = build_policy_with_limit_mode(SpotFundsLimitMode::TrackOnly);
    seed(&long_policy, account_id, asset("AAPL"), "20");
    set_runtime_position_limit(&long_policy, account_id, asset("AAPL"), Some("10"));
    let mut long_mutations = Mutations::with_capacity(2);
    pre_trade_check(
        &long_policy,
        &order(account_id, Side::Sell, "1", "1"),
        &mut long_mutations,
    )
    .expect("a sell reducing an oversized long must pass");
    let _ = long_mutations.rollback_all();

    let short_policy = build_policy_with_limit_mode(SpotFundsLimitMode::TrackOnly);
    seed(&short_policy, account_id, asset("AAPL"), "-20");
    set_runtime_position_limit(&short_policy, account_id, asset("AAPL"), Some("10"));
    let mut short_mutations = Mutations::with_capacity(2);
    pre_trade_check(
        &short_policy,
        &order(account_id, Side::Buy, "1", "1"),
        &mut short_mutations,
    )
    .expect("a buy reducing an oversized short must pass");
    let _ = short_mutations.rollback_all();
}

#[test]
fn settlement_limit_reject_rolls_back_the_underlying_hold() {
    let account_id = account(ACCOUNT);
    let policy =
        policy_with_position_limit(account_id, asset("USD"), "10", SpotFundsLimitMode::Enforce);
    seed(&policy, account_id, asset("AAPL"), "1");

    let mut mutations = Mutations::with_capacity(2);
    let rejects = pre_trade_check(
        &policy,
        &order(account_id, Side::Sell, "1", "11"),
        &mut mutations,
    )
    .expect_err("sell proceeds beyond the settlement limit must reject");
    assert_position_limit_reject(&rejects);
    assert_eq!(
        holdings_of(&policy, account_id, &asset("AAPL"))
            .expect("slot must exist")
            .held(),
        ps("1"),
        "the first step must have succeeded before the second-step barrier"
    );

    let _ = mutations.rollback_all();
    let underlying = holdings_of(&policy, account_id, &asset("AAPL")).expect("slot must remain");
    assert_eq!(underlying.available(), ps("1"));
    assert_eq!(underlying.held(), PositionSize::ZERO);
}

#[test]
fn account_position_limits_are_independent() {
    let limited_account = account(ACCOUNT);
    let other_account = account(ACCOUNT + 1);
    let policy = policy_with_position_limit(
        limited_account,
        asset("AAPL"),
        "1",
        SpotFundsLimitMode::Enforce,
    );
    seed(&policy, limited_account, asset("USD"), "10");
    seed(&policy, other_account, asset("USD"), "10");

    let mut limited_mutations = Mutations::with_capacity(2);
    let rejects = pre_trade_check(
        &policy,
        &order(limited_account, Side::Buy, "2", "1"),
        &mut limited_mutations,
    )
    .expect_err("the configured account must reject");
    assert_position_limit_reject(&rejects);
    let _ = limited_mutations.rollback_all();

    let mut other_mutations = Mutations::with_capacity(2);
    pre_trade_check(
        &policy,
        &order(other_account, Side::Buy, "2", "1"),
        &mut other_mutations,
    )
    .expect("another account must not inherit the limit");
    let _ = other_mutations.rollback_all();
}

#[test]
fn never_seen_position_counts_as_zero() {
    let account_id = account(ACCOUNT);
    let policy =
        policy_with_position_limit(account_id, asset("AAPL"), "1", SpotFundsLimitMode::Enforce);
    seed(&policy, account_id, asset("USD"), "1");
    assert!(holdings_of(&policy, account_id, &asset("AAPL")).is_none());

    let mut mutations = Mutations::with_capacity(2);
    pre_trade_check(
        &policy,
        &order(account_id, Side::Buy, "1", "1"),
        &mut mutations,
    )
    .expect("an unseeded position must start at zero and reach the boundary");
    let _ = mutations.rollback_all();
}

#[test]
fn same_asset_zero_net_order_is_not_checked_per_leg() {
    let account_id = account(ACCOUNT);
    let policy =
        policy_with_position_limit(account_id, asset("USD"), "0", SpotFundsLimitMode::TrackOnly);
    let same_asset_order = make_order(
        account_id,
        instr("USD", "USD"),
        Side::Buy,
        TradeAmount::Quantity(qty("10")),
        Some(px("1")),
    );

    let mut mutations = Mutations::with_capacity(2);
    pre_trade_check(&policy, &same_asset_order, &mut mutations)
        .expect("a zero-net same-asset order must bypass the position barrier");
    let _ = mutations.rollback_all();
}

#[test]
fn same_asset_zero_net_skips_inexact_first_step_proof() {
    let account_id = account(ACCOUNT);
    let usd = asset("USD");
    let limited =
        policy_with_position_limit(account_id, usd.clone(), "0", SpotFundsLimitMode::Enforce);
    let unlimited = build_policy_with_limit_mode(SpotFundsLimitMode::Enforce);
    seed(&limited, account_id, usd.clone(), "100000000000");
    seed(&unlimited, account_id, usd.clone(), "100000000000");
    let same_asset_order = make_order(
        account_id,
        instr("USD", "USD"),
        Side::Buy,
        TradeAmount::Quantity(qty("0.000000000000000001")),
        Some(px("1")),
    );

    let mut limited_mutations = Mutations::with_capacity(2);
    let limited_outcome = pre_trade_full(&limited, &same_asset_order, &mut limited_mutations)
        .expect("a zero-net order must skip the inexact first-step proof");
    let mut unlimited_mutations = Mutations::with_capacity(2);
    let unlimited_outcome = pre_trade_full(&unlimited, &same_asset_order, &mut unlimited_mutations)
        .expect("the no-limit baseline must pass");

    assert_eq!(
        limited_outcome.account_adjustments,
        unlimited_outcome.account_adjustments
    );
    assert_eq!(limited_outcome.lock_prices, unlimited_outcome.lock_prices);
    assert_eq!(
        holdings_of(&limited, account_id, &usd),
        holdings_of(&unlimited, account_id, &usd)
    );
    let _ = limited_mutations.rollback_all();
    let _ = unlimited_mutations.rollback_all();
}

#[test]
fn same_asset_check_reconstructs_pre_order_holdings() {
    let account_id = account(ACCOUNT);
    let policy =
        policy_with_position_limit(account_id, asset("USD"), "5", SpotFundsLimitMode::Enforce);
    seed(&policy, account_id, asset("USD"), "20");
    let same_asset_order = make_order(
        account_id,
        instr("USD", "USD"),
        Side::Buy,
        TradeAmount::Quantity(qty("10")),
        Some(px("2")),
    );

    let mut mutations = Mutations::with_capacity(2);
    pre_trade_check(&policy, &same_asset_order, &mut mutations).expect(
        "the net reduction must be checked against holdings before the first same-asset leg",
    );
    let _ = mutations.rollback_all();
}

#[test]
fn same_asset_check_attaches_to_the_last_nonzero_step() {
    let account_id = account(ACCOUNT);
    let policy =
        policy_with_position_limit(account_id, asset("USD"), "0", SpotFundsLimitMode::TrackOnly);
    let same_asset_sell = make_order(
        account_id,
        instr("USD", "USD"),
        Side::Sell,
        TradeAmount::Quantity(qty("1")),
        Some(px("0")),
    );

    let mut mutations = Mutations::with_capacity(2);
    let rejects = pre_trade_check(&policy, &same_asset_sell, &mut mutations)
        .expect_err("the nonzero first leg must carry the aggregate position check");
    assert_position_limit_reject(&rejects);
    assert!(mutations.is_empty());
}

#[test]
fn same_asset_second_only_step_is_checked() {
    let account_id = account(ACCOUNT);
    let policy =
        policy_with_position_limit(account_id, asset("USD"), "0", SpotFundsLimitMode::TrackOnly);
    let same_asset_buy = make_order(
        account_id,
        instr("USD", "USD"),
        Side::Buy,
        TradeAmount::Quantity(qty("1")),
        Some(Price::ZERO),
    );

    let mut mutations = Mutations::with_capacity(2);
    let rejects = pre_trade_check(&policy, &same_asset_buy, &mut mutations)
        .expect_err("the incoming-only second step must carry the position check");
    assert_position_limit_reject(&rejects);
    assert!(mutations.is_empty());
}

#[test]
fn same_asset_track_only_checks_both_arms_with_negative_available() {
    let account_id = account(ACCOUNT);
    for (side, expected_side) in [(Side::Buy, "long"), (Side::Sell, "short")] {
        let policy = policy_with_position_limit(
            account_id,
            asset("USD"),
            "4",
            SpotFundsLimitMode::TrackOnly,
        );
        let same_asset_order = make_order(
            account_id,
            instr("USD", "USD"),
            side,
            TradeAmount::Quantity(qty("10")),
            Some(px("0.5")),
        );

        let mut mutations = Mutations::with_capacity(2);
        let rejects = pre_trade_check(&policy, &same_asset_order, &mut mutations)
            .expect_err("the aggregate same-asset net must exceed the limit");
        assert_position_limit_reject(&rejects);
        assert!(rejects[0].details.contains(expected_side));
        assert!(
            holdings_of(&policy, account_id, &asset("USD"))
                .expect("the first step must be reserved")
                .available()
                < PositionSize::ZERO
        );
        let _ = mutations.rollback_all();
    }
}

#[test]
fn same_asset_dry_run_matches_mutating_verdict_without_state_change() {
    let account_id = account(ACCOUNT);
    let policy =
        policy_with_position_limit(account_id, asset("USD"), "1", SpotFundsLimitMode::TrackOnly);
    let adjustment = all_fields_adj(
        asset("USD"),
        Some(AdjustmentAmount::Absolute(ps("0"))),
        Some(AdjustmentAmount::Absolute(ps("3"))),
        Some(AdjustmentAmount::Absolute(ps("0"))),
    );
    let mut adjustment_mutations = Mutations::with_capacity(1);
    let _ = run_adjustment(&policy, account_id, &adjustment, &mut adjustment_mutations);
    let _ = adjustment_mutations.commit_all();
    let before = holdings_of(&policy, account_id, &asset("USD")).expect("slot must exist");
    let same_asset_order = make_order(
        account_id,
        instr("USD", "USD"),
        Side::Sell,
        TradeAmount::Quantity(qty("3")),
        Some(px("0.5")),
    );

    let dry_rejects =
        dry_run_check(&policy, &same_asset_order).expect_err("dry-run must reject the net");
    assert_position_limit_reject(&dry_rejects);
    assert_eq!(
        holdings_of(&policy, account_id, &asset("USD")),
        Some(before)
    );

    let mut mutations = Mutations::with_capacity(2);
    let real_rejects = pre_trade_check(&policy, &same_asset_order, &mut mutations)
        .expect_err("mutating pre-trade must return the same reject");
    assert_eq!(real_rejects[0], dry_rejects[0]);
    let _ = mutations.rollback_all();
    assert_eq!(
        holdings_of(&policy, account_id, &asset("USD")),
        Some(before)
    );
}

#[test]
fn same_asset_fresh_slot_dry_run_matches_mutating_reject_and_rollback() {
    let account_id = account(ACCOUNT);
    let usd = asset("USD");
    let policy =
        policy_with_position_limit(account_id, usd.clone(), "4", SpotFundsLimitMode::TrackOnly);
    let same_asset_order = make_order(
        account_id,
        instr("USD", "USD"),
        Side::Buy,
        TradeAmount::Quantity(qty("10")),
        Some(px("0.5")),
    );
    assert!(holdings_of(&policy, account_id, &usd).is_none());

    let dry_rejects =
        dry_run_check(&policy, &same_asset_order).expect_err("dry-run must reject the net");
    assert_position_limit_reject(&dry_rejects);
    assert!(holdings_of(&policy, account_id, &usd).is_none());

    let mut mutations = Mutations::with_capacity(2);
    let real_rejects = pre_trade_check(&policy, &same_asset_order, &mut mutations)
        .expect_err("mutating pre-trade must return the same reject");
    assert_eq!(real_rejects[0], dry_rejects[0]);
    assert!(
        holdings_of(&policy, account_id, &usd).is_some(),
        "the rejected second step must leave the first step for rollback"
    );
    let _ = mutations.rollback_all();
    assert!(holdings_of(&policy, account_id, &usd).is_none());
}

#[test]
fn same_asset_short_inexact_first_step_transition_fails_closed() {
    let account_id = account(ACCOUNT);
    let usd = asset("USD");
    let policy = policy_with_position_limit(
        account_id,
        usd.clone(),
        "79228162514.264337593543950332",
        SpotFundsLimitMode::TrackOnly,
    );
    seed(
        &policy,
        account_id,
        usd.clone(),
        "-79228162514.264337593543950325",
    );
    let before = holdings_of(&policy, account_id, &usd).expect("slot must exist");
    let same_asset_sell = make_order(
        account_id,
        instr("USD", "USD"),
        Side::Sell,
        TradeAmount::Quantity(qty("0.000000000000000016")),
        Some(px("0.5")),
    );

    let dry_rejects = dry_run_check(&policy, &same_asset_sell)
        .expect_err("the dry-run first-step subtraction must fail closed");
    assert_position_limit_arithmetic_reject(
        &dry_rejects,
        "USD",
        "first-step transition",
        "inexact",
    );
    assert_eq!(holdings_of(&policy, account_id, &usd), Some(before));

    let mut mutations = Mutations::with_capacity(2);
    let real_rejects = pre_trade_check(&policy, &same_asset_sell, &mut mutations)
        .expect_err("the mutating first-step subtraction must fail closed");
    assert_eq!(real_rejects[0], dry_rejects[0]);
    assert!(mutations.is_empty());
    assert_eq!(holdings_of(&policy, account_id, &usd), Some(before));
}

#[test]
fn same_asset_net_delta_overflow_is_an_order_reject() {
    let account_id = account(ACCOUNT);
    let maximum = rust_decimal::Decimal::MAX.to_string();
    let policy = policy_with_position_limit(
        account_id,
        asset("USD"),
        &maximum,
        SpotFundsLimitMode::TrackOnly,
    );
    let adjustment = all_fields_adj(
        asset("USD"),
        Some(AdjustmentAmount::Absolute(ps(&maximum))),
        Some(AdjustmentAmount::Absolute(ps("-1"))),
        None,
    );
    let mut adjustment_mutations = Mutations::with_capacity(1);
    let _ = run_adjustment(&policy, account_id, &adjustment, &mut adjustment_mutations);
    let _ = adjustment_mutations.commit_all();

    let same_asset_sell = make_order(
        account_id,
        instr("USD", "USD"),
        Side::Sell,
        TradeAmount::Quantity(qty("1")),
        Some(px(&format!("-{maximum}"))),
    );
    let mut mutations = Mutations::with_capacity(2);
    let rejects = pre_trade_check(&policy, &same_asset_sell, &mut mutations)
        .expect_err("the aggregate net overflow must reject");
    assert_position_limit_arithmetic_reject(&rejects, "USD", "net", "overflow");
    let _ = mutations.rollback_all();
}

#[test]
fn negative_held_residual_is_taken_at_face_value() {
    let account_id = account(ACCOUNT);
    let instrument = instr("AAPL", "USD");
    let policy = policy_with_position_limit(
        account_id,
        asset("AAPL"),
        "10",
        SpotFundsLimitMode::TrackOnly,
    );
    let report = make_report(
        account_id,
        instrument,
        Side::Sell,
        Some(Trade {
            price: px("1"),
            quantity: qty("10"),
        }),
        Quantity::ZERO,
        true,
        Some(PreTradeLock::from_entries([(
            DEFAULT_POLICY_GROUP_ID,
            px("1"),
        )])),
    );
    assert!(report_blocks(&policy, &report).is_empty());
    let residual = holdings_of(&policy, account_id, &asset("AAPL")).expect("slot must exist");
    assert_eq!(residual.held(), ps("-10"));

    let mut mutations = Mutations::with_capacity(2);
    let rejects = pre_trade_check(
        &policy,
        &order(account_id, Side::Sell, "1", "1"),
        &mut mutations,
    )
    .expect_err("the residual plus a further sell must exceed the short limit");
    assert_position_limit_reject(&rejects);
    assert!(mutations.is_empty());
    assert_eq!(
        holdings_of(&policy, account_id, &asset("AAPL"))
            .expect("slot must remain")
            .held(),
        ps("-10")
    );
}

#[test]
fn market_order_uses_stored_incoming_after_quote_change() {
    let account_id = account(ACCOUNT);
    let instrument = instr("AAPL", "USD");
    let builder = engine_builder();
    let service = MarketDataBuilder::<FullSync>::new(QuoteTtl::Infinite).build();
    let instrument_id = service
        .register(instrument.clone())
        .expect("instrument registration must succeed");
    service
        .push(
            instrument_id,
            Quote::new().with_mark(px("100")),
            Duration::ZERO,
        )
        .expect("first quote must publish");
    let mut settings = settings(0);
    settings.set_position_limit(account_id, asset("AAPL"), Some(qty("14")));
    let policy = SpotFundsPolicy::new(
        settings,
        Some(SpotFundsMarketData::new(Arc::clone(&service))),
        builder.storage_builder(),
    );
    seed(&policy, account_id, asset("USD"), "3000");

    let market_order = make_order(
        account_id,
        instrument,
        Side::Buy,
        TradeAmount::Volume(vol("1000")),
        None,
    );
    let mut first_mutations = Mutations::with_capacity(2);
    pre_trade_check(&policy, &market_order, &mut first_mutations)
        .expect("the first market order must project ten units");
    let _ = first_mutations.commit_all();
    assert_eq!(incoming_of(&policy, account_id, "AAPL"), ps("10"));

    service
        .push(
            instrument_id,
            Quote::new().with_mark(px("200")),
            Duration::ZERO,
        )
        .expect("second quote must publish");
    assert_eq!(incoming_of(&policy, account_id, "AAPL"), ps("10"));

    let mut second_mutations = Mutations::with_capacity(2);
    let rejects = pre_trade_check(&policy, &market_order, &mut second_mutations)
        .expect_err("stored ten plus the new five must exceed fourteen");
    assert_position_limit_reject(&rejects);
    let _ = second_mutations.rollback_all();
}

#[test]
fn dry_run_matches_mutating_reject_and_changes_no_state() {
    let account_id = account(ACCOUNT);
    let policy =
        policy_with_position_limit(account_id, asset("AAPL"), "1", SpotFundsLimitMode::Enforce);
    seed(&policy, account_id, asset("USD"), "10");
    let rejected_order = order(account_id, Side::Buy, "2", "1");
    let before = holdings_of(&policy, account_id, &asset("USD"));

    let dry_rejects = dry_run_check(&policy, &rejected_order).expect_err("dry-run must reject");
    assert_position_limit_reject(&dry_rejects);
    assert_eq!(holdings_of(&policy, account_id, &asset("USD")), before);
    assert!(holdings_of(&policy, account_id, &asset("AAPL")).is_none());

    let mut mutations = Mutations::with_capacity(2);
    let real_rejects = pre_trade_check(&policy, &rejected_order, &mut mutations)
        .expect_err("mutating path must return the same reject");
    assert_eq!(real_rejects[0], dry_rejects[0]);
    let _ = mutations.rollback_all();
    assert_eq!(holdings_of(&policy, account_id, &asset("USD")), before);
    assert!(holdings_of(&policy, account_id, &asset("AAPL")).is_none());
}

fn drop_copy_engine(
    settings: SpotFundsSettings,
) -> crate::FullSyncEngine<TestOrder, TestReport, TestAdjustment> {
    let builder = crate::Engine::builder::<TestOrder, TestReport, TestAdjustment>().full_sync();
    let policy: SpotFundsPolicy<FullSync, FullSync> =
        SpotFundsPolicy::new(settings, None, builder.storage_builder());
    builder
        .pre_trade(policy)
        .build()
        .expect("engine must build")
}

#[test]
fn drop_copy_bypasses_limit_and_matches_no_limit_bookkeeping() {
    let account_id = account(ACCOUNT);
    let mut limited_settings = settings(0);
    limited_settings.set_position_limit(account_id, asset("AAPL"), Some(Quantity::ZERO));
    let limited = drop_copy_engine(limited_settings);
    let unlimited = drop_copy_engine(settings(0));
    let copied_order = order(account_id, Side::Buy, "2", "1");

    let mut limited_operation = limited
        .apply_drop_copy(copied_order.clone())
        .expect("drop-copy must bypass the position limit");
    let mut unlimited_operation = unlimited
        .apply_drop_copy(copied_order)
        .expect("no-limit drop-copy must pass");
    assert_eq!(
        limited_operation.account_adjustments(),
        unlimited_operation.account_adjustments()
    );
    assert_eq!(limited_operation.account_adjustments().len(), 2);
    assert_eq!(
        limited_operation.account_adjustments()[0].entry.asset,
        asset("USD")
    );
    assert_eq!(
        limited_operation.account_adjustments()[0]
            .entry
            .held
            .expect("drop-copy settlement hold must be recorded")
            .absolute,
        ps("2")
    );
    assert_eq!(
        limited_operation.account_adjustments()[1].entry.asset,
        asset("AAPL")
    );
    assert_eq!(
        limited_operation.account_adjustments()[1]
            .entry
            .incoming
            .expect("drop-copy base incoming must be recorded")
            .absolute,
        ps("2")
    );
    assert_eq!(limited_operation.lock(), unlimited_operation.lock());
    limited_operation.commit();
    unlimited_operation.commit();
}

#[test]
fn insufficient_funds_reject_precedes_position_limit() {
    let account_id = account(ACCOUNT);
    let policy =
        policy_with_position_limit(account_id, asset("USD"), "0", SpotFundsLimitMode::Enforce);
    let mut mutations = Mutations::with_capacity(2);
    let rejects = pre_trade_check(
        &policy,
        &order(account_id, Side::Buy, "1", "1"),
        &mut mutations,
    )
    .expect_err("the unfunded order must reject");
    assert_eq!(rejects[0].code, RejectCode::InsufficientFunds);
    assert!(mutations.is_empty());
}

#[test]
fn position_limit_check_observes_a_prior_committed_reservation() {
    let account_id = account(ACCOUNT);
    let builder = crate::Engine::builder::<TestOrder, TestReport, TestAdjustment>().full_sync();
    let mut configured = settings(0);
    configured.set_global_limit_mode(SpotFundsLimitMode::TrackOnly);
    configured.set_position_limit(account_id, asset("USD"), Some(qty("1")));
    let policy: TestPolicy = SpotFundsPolicy::new(configured, None, builder.storage_builder());
    let entered = Arc::new(std::sync::Barrier::new(2));
    let release = Arc::new(std::sync::Barrier::new(2));
    policy.set_pre_trade_reservation_hook(account_id, Arc::clone(&entered), Arc::clone(&release));
    let engine = Arc::new(
        builder
            .pre_trade(policy)
            .build()
            .expect("engine must build"),
    );

    let delayed = {
        let engine = Arc::clone(&engine);
        std::thread::spawn(move || {
            engine
                .execute_pre_trade(order(account_id, Side::Buy, "1", "1"))
                .err()
                .expect("the delayed check must see the prior committed reservation")
        })
    };
    entered.wait();

    let mut winning = engine
        .execute_pre_trade(order(account_id, Side::Buy, "1", "1"))
        .expect("one of the concurrent orders must reserve the boundary");
    winning.commit();
    release.wait();

    let rejects = delayed.join().expect("reservation thread must finish");
    assert_position_limit_reject(&rejects);
}

#[test]
fn position_limit_absence_preserves_existing_outcome_shape() {
    let account_id = account(ACCOUNT);
    let policy = build_policy(None, None);
    seed(&policy, account_id, asset("USD"), "10");
    let mut mutations = Mutations::with_capacity(2);
    let outcome = pre_trade_full(
        &policy,
        &order(account_id, Side::Buy, "2", "3"),
        &mut mutations,
    )
    .expect("an unconfigured position limit must not add a gate");

    assert_eq!(outcome.account_adjustments.len(), 2);
    assert_eq!(outcome.account_adjustments[0].asset, asset("USD"));
    assert_eq!(outcome.account_adjustments[1].asset, asset("AAPL"));
    assert_eq!(outcome.lock_prices.to_vec(), vec![px("3")]);
    assert_eq!(
        holdings_of(&policy, account_id, &asset("USD"))
            .expect("settlement slot must exist")
            .held(),
        ps("6")
    );
    assert_eq!(incoming_of(&policy, account_id, "AAPL"), ps("2"));
    let _ = mutations.rollback_all();
}
