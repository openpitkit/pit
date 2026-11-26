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

// Engine-level behavior of `ReviewRulePolicy`, written from the business rules:
// a rule turns every matching order into the verdict of its registration mode
// (reject or review mark), everything else the engine and the other policies do
// stays as it was, and rules of different levels and instances only add up.

use openpit::param::{
    AccountGroupId, AccountId, AdjustmentAmount, Asset, PositionSize, Price, Quantity, Side,
    TradeAmount,
};
use openpit::pretrade::policies::{
    ReviewRuleAccountGroupRule, ReviewRuleAccountRule, ReviewRuleInstruments, ReviewRulePolicy,
    ReviewRuleSettings, SpotFundsPolicy, SpotFundsPricingSource, SpotFundsSettings,
};
use openpit::pretrade::{
    PreTradePolicy, PreTradeReservation, Reject, RejectCode, RejectScope, Rejects, RuleMode,
};
use openpit::storage::FullLocking;
use openpit::{
    AccountAdjustmentAmount, AccountAdjustmentBalanceOperation, AccountAdjustmentBounds,
    AccountRetirementError, AccountRetirementPolicyRefusal, AccountRetirementRefusal, Engine,
    EngineBuildError, FullSync, FullSyncEngine, HasAccountId, HasAlreadyHeld, HasConfirmed,
    HasInstrument, Instrument, OrderOperation, OrderReview, PolicyInstance, ReadyEngineBuilder,
    RequestFieldAccessError, SpotFundsMarketData, WithAccountAdjustmentAmount,
    WithAccountAdjustmentBalanceOperation, WithAccountAdjustmentBounds,
    WithExecutionReportFillDetails, WithExecutionReportOperation, WithOrderOperation,
    WithOrderReview,
};

type Order = WithOrderOperation<WithOrderReview<()>>;
type PlainEngine = FullSyncEngine<Order, (), ()>;
type SpotReport = WithExecutionReportOperation<WithExecutionReportFillDetails<()>>;
type SpotAdjustment = WithAccountAdjustmentAmount<
    WithAccountAdjustmentBounds<WithAccountAdjustmentBalanceOperation<AccountAdjustmentAmount>>,
>;
type SpotEngine = FullSyncEngine<Order, SpotReport, SpotAdjustment>;

const ACCOUNT: u64 = 1001;
const OTHER_ACCOUNT: u64 = 1002;
const GROUP: u32 = 7;

const FIRST_RUN: OrderReview = OrderReview {
    confirmed: false,
    already_held: false,
};
const CONFIRMED: OrderReview = OrderReview {
    confirmed: true,
    already_held: false,
};
const HELD_RERUN: OrderReview = OrderReview {
    confirmed: true,
    already_held: true,
};

const REVIEW_MODES: [RuleMode; 2] = [RuleMode::ReviewWithHold, RuleMode::ReviewWithoutHold];
const ALL_MODES: [RuleMode; 3] = [
    RuleMode::ReviewWithHold,
    RuleMode::ReviewWithoutHold,
    RuleMode::Reject,
];

// ── values ────────────────────────────────────────────────────────────────────

fn instrument(underlying: &str, settlement: &str) -> Instrument {
    Instrument::new(
        Asset::new(underlying).expect("valid asset"),
        Asset::new(settlement).expect("valid asset"),
    )
}

fn account(id: u64) -> AccountId {
    AccountId::from_u64(id)
}

fn group() -> AccountGroupId {
    AccountGroupId::from_u32(GROUP).expect("valid group")
}

fn position(value: &str) -> PositionSize {
    PositionSize::from_str(value).expect("valid position size")
}

/// Buy 10 at a limit of 200: a 2000 settlement hold when Spot Funds is present.
fn order(account_id: u64, underlying: &str, settlement: &str, review: OrderReview) -> Order {
    WithOrderOperation {
        operation: OrderOperation {
            instrument: instrument(underlying, settlement),
            account_id: account(account_id),
            side: Side::Buy,
            trade_amount: TradeAmount::Quantity(Quantity::from_str("10").expect("valid quantity")),
            price: Some(Price::from_str("200").expect("valid price")),
        },
        inner: WithOrderReview { inner: (), review },
    }
}

fn aapl_usd(account_id: u64) -> Order {
    order(account_id, "AAPL", "USD", FIRST_RUN)
}

fn aapl_eur(account_id: u64) -> Order {
    order(account_id, "AAPL", "EUR", FIRST_RUN)
}

fn msft_usd(account_id: u64) -> Order {
    order(account_id, "MSFT", "USD", FIRST_RUN)
}

/// An order whose fields can fail to read, as a caller's order type may.
#[derive(Clone)]
struct FieldOrder {
    account: Result<AccountId, &'static str>,
    instrument: Result<Instrument, &'static str>,
}

impl HasAccountId for FieldOrder {
    fn account_id(&self) -> Result<AccountId, RequestFieldAccessError> {
        self.account.map_err(RequestFieldAccessError::new)
    }
}

impl HasInstrument for FieldOrder {
    fn instrument(&self) -> Result<&Instrument, RequestFieldAccessError> {
        self.instrument
            .as_ref()
            .map_err(|message| RequestFieldAccessError::new(message))
    }
}

impl HasConfirmed for FieldOrder {
    fn confirmed(&self) -> Result<bool, RequestFieldAccessError> {
        Ok(false)
    }
}

impl HasAlreadyHeld for FieldOrder {
    fn already_held(&self) -> Result<bool, RequestFieldAccessError> {
        Ok(false)
    }
}

// ── rules and engines ─────────────────────────────────────────────────────────

fn all() -> ReviewRuleInstruments {
    ReviewRuleInstruments::All
}

fn exact(pairs: &[(&str, &str)]) -> ReviewRuleInstruments {
    ReviewRuleInstruments::Exact(
        pairs
            .iter()
            .map(|(underlying, settlement)| instrument(underlying, settlement))
            .collect(),
    )
}

fn account_rule(id: u64, instruments: ReviewRuleInstruments) -> ReviewRuleAccountRule {
    ReviewRuleAccountRule {
        instruments,
        account_id: account(id),
    }
}

fn group_rule(instruments: ReviewRuleInstruments) -> ReviewRuleAccountGroupRule {
    ReviewRuleAccountGroupRule {
        instruments,
        account_group_id: group(),
    }
}

/// One registered instance of the policy: its name, mode and initial rules.
struct Instance {
    name: &'static str,
    mode: RuleMode,
    settings: ReviewRuleSettings,
}

fn instance(
    name: &'static str,
    mode: RuleMode,
    global: Option<ReviewRuleInstruments>,
    groups: Vec<ReviewRuleAccountGroupRule>,
    accounts: Vec<ReviewRuleAccountRule>,
) -> Instance {
    Instance {
        name,
        mode,
        settings: ReviewRuleSettings::new(global, groups, accounts).expect("valid rules"),
    }
}

/// An instance holding one account rule.
fn account_instance(
    name: &'static str,
    mode: RuleMode,
    id: u64,
    instruments: ReviewRuleInstruments,
) -> Instance {
    instance(
        name,
        mode,
        None,
        vec![],
        vec![account_rule(id, instruments)],
    )
}

fn register<Order, Report, Adjustment>(
    mut builder: ReadyEngineBuilder<Order, Report, Adjustment, FullSync>,
    instances: impl IntoIterator<Item = Instance>,
) -> ReadyEngineBuilder<Order, Report, Adjustment, FullSync>
where
    Order: 'static,
    Report: 'static,
    Adjustment: 'static,
    ReviewRulePolicy<FullLocking>: PreTradePolicy<Order, Report, Adjustment, FullSync>,
{
    for instance in instances {
        builder = builder.pre_trade_instance(
            ReviewRulePolicy::<FullLocking>::new(instance.settings),
            PolicyInstance {
                name: Some(instance.name.to_owned()),
                mode: instance.mode,
            },
        );
    }
    builder
}

/// An engine with order review enabled and the given instances only.
fn engine_of<Order>(instances: Vec<Instance>) -> FullSyncEngine<Order, (), ()>
where
    Order: HasInstrument + HasAccountId + HasConfirmed + HasAlreadyHeld + 'static,
{
    let mut instances = instances.into_iter();
    let first = instances.next().expect("at least one instance");
    let builder = Engine::builder::<Order, (), ()>()
        .with_order_review()
        .full_sync()
        .pre_trade_instance(
            ReviewRulePolicy::<FullLocking>::new(first.settings),
            PolicyInstance {
                name: Some(first.name.to_owned()),
                mode: first.mode,
            },
        );
    register(builder, instances)
        .build()
        .expect("engine must build")
}

fn plain_engine(instances: Vec<Instance>) -> PlainEngine {
    engine_of::<Order>(instances)
}

/// Spot Funds first, the rule instances after it, as the policy is meant to be
/// registered. USD 10000 and AAPL 3 are seeded for the account 1001.
fn spot_engine(instances: Vec<Instance>, usd: &str) -> SpotEngine {
    let builder = Engine::builder::<Order, SpotReport, SpotAdjustment>()
        .with_order_review()
        .full_sync();
    let spot = SpotFundsPolicy::<FullSync, FullSync>::new(
        SpotFundsSettings::new(0, SpotFundsPricingSource::Mark, []).expect("valid settings"),
        None::<SpotFundsMarketData<FullSync>>,
        builder.storage_builder(),
    );
    let engine = register(builder.pre_trade(spot), instances)
        .build()
        .expect("engine must build");
    seed(&engine, "USD", usd);
    seed(&engine, "AAPL", "3");
    engine
}

fn adjustment(
    asset_code: &str,
    balance: Option<AdjustmentAmount>,
    held: Option<AdjustmentAmount>,
    incoming: Option<AdjustmentAmount>,
) -> SpotAdjustment {
    WithAccountAdjustmentAmount {
        inner: WithAccountAdjustmentBounds {
            inner: WithAccountAdjustmentBalanceOperation {
                inner: AccountAdjustmentAmount::default(),
                operation: AccountAdjustmentBalanceOperation {
                    asset: Asset::new(asset_code).expect("valid asset"),
                    average_entry_price: None,
                },
            },
            bounds: AccountAdjustmentBounds::default(),
        },
        amount: AccountAdjustmentAmount {
            balance,
            held,
            incoming,
        },
    }
}

fn seed(engine: &SpotEngine, asset_code: &str, amount: &str) {
    let seed = adjustment(
        asset_code,
        Some(AdjustmentAmount::Absolute(position(amount))),
        None,
        None,
    );
    engine
        .apply_account_adjustment(account(ACCOUNT), &[seed])
        .expect("seed must succeed");
}

/// Reads `[available, held, incoming]` of USD and AAPL through zero deltas.
fn assert_balances(engine: &SpotEngine, usd: [&str; 3], aapl: [&str; 3]) {
    let zero = Some(AdjustmentAmount::Delta(PositionSize::ZERO));
    let reads = ["USD", "AAPL"].map(|code| adjustment(code, zero, zero, zero));
    let result = engine
        .apply_account_adjustment(account(ACCOUNT), &reads)
        .expect("zero-delta balance read must succeed");
    for (code, [available, held, incoming]) in [("USD", usd), ("AAPL", aapl)] {
        let entry = &result
            .outcomes
            .iter()
            .find(|outcome| outcome.entry.asset == Asset::new(code).expect("valid asset"))
            .expect("asset outcome must be present")
            .entry;
        assert_eq!(
            entry.balance.expect("available must be present").absolute,
            position(available),
            "{code} available"
        );
        assert_eq!(
            entry.held.expect("held must be present").absolute,
            position(held),
            "{code} held"
        );
        assert_eq!(
            entry.incoming.expect("incoming must be present").absolute,
            position(incoming),
            "{code} incoming"
        );
    }
}

const UNTOUCHED: ([&str; 3], [&str; 3]) = (["10000", "0", "0"], ["3", "0", "0"]);
const HELD: ([&str; 3], [&str; 3]) = (["8000", "2000", "0"], ["3", "0", "10"]);

fn assert_untouched(engine: &SpotEngine) {
    assert_balances(engine, UNTOUCHED.0, UNTOUCHED.1);
}

fn assert_held(engine: &SpotEngine) {
    assert_balances(engine, HELD.0, HELD.1);
}

// ── outcomes ──────────────────────────────────────────────────────────────────

fn reserve<Order, Report, Adjustment>(
    engine: &FullSyncEngine<Order, Report, Adjustment>,
    order: Order,
) -> PreTradeReservation
where
    Order: HasAccountId + 'static,
    Report: 'static,
    Adjustment: 'static,
{
    match engine.execute_pre_trade(order) {
        Ok(reservation) => reservation,
        Err(rejects) => panic!("the order must pass, got {:?}", rejects.to_vec()),
    }
}

fn rejected<Order, Report, Adjustment>(
    engine: &FullSyncEngine<Order, Report, Adjustment>,
    order: Order,
) -> Rejects
where
    Order: HasAccountId + 'static,
    Report: 'static,
    Adjustment: 'static,
{
    match engine.execute_pre_trade(order) {
        Ok(reservation) => panic!(
            "the order must be rejected, got marks {:?}",
            reservation.reviews()
        ),
        Err(rejects) => rejects,
    }
}

/// An order that is neither rejected nor marked.
fn assert_clean<Order, Report, Adjustment>(
    engine: &FullSyncEngine<Order, Report, Adjustment>,
    order: Order,
) where
    Order: HasAccountId + 'static,
    Report: 'static,
    Adjustment: 'static,
{
    let mut reservation = reserve(engine, order);
    assert!(
        reservation.reviews().is_empty(),
        "no rule covers the order, got marks {:?}",
        reservation.reviews()
    );
    reservation.commit();
}

/// The one record a matching rule produces: order scope, never an account
/// block.
fn assert_rule_record(record: &Reject, mode: RuleMode) {
    assert_eq!(record.policy, ReviewRulePolicy::<FullLocking>::NAME);
    assert_eq!(record.code, RejectCode::ComplianceRestriction);
    assert_eq!(record.scope, RejectScope::Order);
    assert_eq!(record.mode, mode);
}

fn assert_marked(mut reservation: PreTradeReservation, mode: RuleMode) {
    assert_eq!(reservation.reviews().len(), 1, "one mark per instance");
    assert_rule_record(&reservation.reviews()[0], mode);
    reservation.commit();
}

fn assert_field_failure(rejects: &Rejects) {
    assert_eq!(rejects.len(), 1);
    assert_eq!(rejects[0].policy, ReviewRulePolicy::<FullLocking>::NAME);
    assert_eq!(rejects[0].code, RejectCode::MissingRequiredField);
    assert_eq!(rejects[0].scope, RejectScope::Order);
    assert_eq!(rejects[0].mode, RuleMode::Reject);
}

fn marks_of(reviews: &[Reject], mode: RuleMode) -> usize {
    reviews.iter().filter(|mark| mark.mode == mode).count()
}

// ── reject instance ───────────────────────────────────────────────────────────

#[test]
fn reject_instance_rejects_the_matching_order_only() {
    let engine = plain_engine(vec![account_instance(
        "reject",
        RuleMode::Reject,
        ACCOUNT,
        all(),
    )]);

    let rejects = rejected(&engine, aapl_usd(ACCOUNT));
    assert_eq!(rejects.len(), 1);
    assert_rule_record(&rejects[0], RuleMode::Reject);

    assert_clean(&engine, aapl_usd(OTHER_ACCOUNT));
}

#[test]
fn reject_does_not_block_the_account() {
    let engine = plain_engine(vec![account_instance(
        "reject",
        RuleMode::Reject,
        ACCOUNT,
        exact(&[("AAPL", "USD")]),
    )]);

    let rejects = rejected(&engine, aapl_usd(ACCOUNT));
    assert_eq!(rejects.len(), 1);
    assert_rule_record(&rejects[0], RuleMode::Reject);

    // The record is order-scoped: the account is not latched blocked, so an
    // order no rule covers still goes through, and the rule still applies.
    assert_clean(&engine, msft_usd(ACCOUNT));
    assert_clean(&engine, aapl_eur(ACCOUNT));
    assert_eq!(rejected(&engine, aapl_usd(ACCOUNT)).len(), 1);
}

// ── review instance ───────────────────────────────────────────────────────────

#[test]
fn review_instance_marks_the_matching_order_in_its_mode() {
    for mode in REVIEW_MODES {
        let engine = plain_engine(vec![account_instance("review", mode, ACCOUNT, all())]);

        assert_marked(reserve(&engine, aapl_usd(ACCOUNT)), mode);
        assert_clean(&engine, aapl_usd(OTHER_ACCOUNT));
    }
}

#[test]
fn review_mark_does_not_block_the_account() {
    for mode in REVIEW_MODES {
        let engine = plain_engine(vec![account_instance(
            "review",
            mode,
            ACCOUNT,
            exact(&[("AAPL", "USD")]),
        )]);

        assert_marked(reserve(&engine, aapl_usd(ACCOUNT)), mode);
        assert_clean(&engine, msft_usd(ACCOUNT));
        assert_marked(reserve(&engine, aapl_usd(ACCOUNT)), mode);
    }
}

#[test]
fn group_rule_covers_the_members_of_the_group() {
    for mode in REVIEW_MODES {
        let engine = plain_engine(vec![instance(
            "group-review",
            mode,
            None,
            vec![group_rule(all())],
            vec![],
        )]);
        engine
            .accounts()
            .register_group(&[account(ACCOUNT)], group())
            .expect("register group");

        assert_marked(reserve(&engine, aapl_usd(ACCOUNT)), mode);
        assert_clean(&engine, aapl_usd(OTHER_ACCOUNT));
    }
}

#[test]
fn global_exact_rule_covers_only_the_listed_instrument() {
    for mode in REVIEW_MODES {
        let engine = plain_engine(vec![instance(
            "global-review",
            mode,
            Some(exact(&[("AAPL", "USD")])),
            vec![],
            vec![],
        )]);

        assert_marked(reserve(&engine, aapl_usd(ACCOUNT)), mode);
        assert_marked(reserve(&engine, aapl_usd(OTHER_ACCOUNT)), mode);
        assert_clean(&engine, msft_usd(ACCOUNT));
        // The pair is matched as a whole, not by its underlying asset alone.
        assert_clean(&engine, aapl_eur(ACCOUNT));
    }
}

// ── confirmed ─────────────────────────────────────────────────────────────────

#[test]
fn confirmed_order_gets_no_mark_from_a_review_instance() {
    for mode in REVIEW_MODES {
        let engine = plain_engine(vec![account_instance("review", mode, ACCOUNT, all())]);

        let mut reservation = reserve(&engine, order(ACCOUNT, "AAPL", "USD", CONFIRMED));
        assert!(
            reservation.reviews().is_empty(),
            "a confirmed order is not marked again"
        );
        reservation.commit();
    }
}

#[test]
fn confirmed_order_is_still_rejected_by_a_reject_instance() {
    let engine = plain_engine(vec![account_instance(
        "reject",
        RuleMode::Reject,
        ACCOUNT,
        all(),
    )]);

    let rejects = rejected(&engine, order(ACCOUNT, "AAPL", "USD", CONFIRMED));
    assert_eq!(rejects.len(), 1);
    assert_rule_record(&rejects[0], RuleMode::Reject);
}

// ── field failures ────────────────────────────────────────────────────────────

fn field_order(
    account: Result<u64, &'static str>,
    instrument_value: Result<Instrument, &'static str>,
) -> FieldOrder {
    FieldOrder {
        account: account.map(AccountId::from_u64),
        instrument: instrument_value,
    }
}

#[test]
fn unreadable_account_is_an_effective_reject_in_every_mode() {
    for mode in ALL_MODES {
        let engine = engine_of::<FieldOrder>(vec![account_instance("rule", mode, ACCOUNT, all())]);
        let rejects = rejected(
            &engine,
            field_order(Err("account unreadable"), Ok(instrument("AAPL", "USD"))),
        );
        assert_field_failure(&rejects);
    }
}

#[test]
fn unreadable_instrument_under_an_exact_rule_is_an_effective_reject_in_every_mode() {
    for mode in ALL_MODES {
        let engine = engine_of::<FieldOrder>(vec![account_instance(
            "rule",
            mode,
            ACCOUNT,
            exact(&[("AAPL", "USD")]),
        )]);
        let rejects = rejected(
            &engine,
            field_order(Ok(ACCOUNT), Err("instrument unreadable")),
        );
        assert_field_failure(&rejects);
    }
}

#[test]
fn all_instruments_rule_does_not_need_the_instrument() {
    for mode in ALL_MODES {
        let engine = engine_of::<FieldOrder>(vec![account_instance("rule", mode, ACCOUNT, all())]);
        let unreadable = field_order(Ok(ACCOUNT), Err("instrument unreadable"));
        if mode == RuleMode::Reject {
            let rejects = rejected(&engine, unreadable);
            assert_eq!(rejects.len(), 1);
            assert_rule_record(&rejects[0], RuleMode::Reject);
        } else {
            assert_marked(reserve(&engine, unreadable), mode);
        }
    }
}

#[test]
fn instrument_is_not_read_for_an_account_without_a_rule() {
    for mode in ALL_MODES {
        let engine = engine_of::<FieldOrder>(vec![account_instance(
            "rule",
            mode,
            ACCOUNT,
            exact(&[("AAPL", "USD")]),
        )]);
        assert_clean(
            &engine,
            field_order(Ok(OTHER_ACCOUNT), Err("instrument unreadable")),
        );
    }
}

// ── dry-run ───────────────────────────────────────────────────────────────────

#[test]
fn full_dry_run_reports_the_live_marks_and_the_start_dry_run_none() {
    for mode in REVIEW_MODES {
        let engine = plain_engine(vec![account_instance("review", mode, ACCOUNT, all())]);

        let mut live = reserve(&engine, aapl_usd(ACCOUNT));
        assert_eq!(live.reviews().len(), 1);

        // The policy works on the main stage, which the start dry-run does not
        // run.
        let start = engine.start_pre_trade_dry_run(aapl_usd(ACCOUNT));
        assert!(start.is_pass());
        assert!(start.reviews().is_empty());

        let full = engine.execute_pre_trade_dry_run(aapl_usd(ACCOUNT));
        assert!(full.is_pass());
        assert_eq!(full.reviews(), live.reviews());
        assert_rule_record(&full.reviews()[0], mode);
        live.commit();
    }
}

#[test]
fn dry_run_of_a_reject_instance_reports_the_reject() {
    let engine = plain_engine(vec![account_instance(
        "reject",
        RuleMode::Reject,
        ACCOUNT,
        all(),
    )]);

    assert!(engine.start_pre_trade_dry_run(aapl_usd(ACCOUNT)).is_pass());
    let full = engine.execute_pre_trade_dry_run(aapl_usd(ACCOUNT));
    assert!(!full.is_pass());
    let rejects = full.rejects().expect("the dry-run must report the reject");
    assert_eq!(rejects.len(), 1);
    assert_rule_record(&rejects[0], RuleMode::Reject);
    assert!(full.reviews().is_empty());
}

#[test]
fn dry_run_with_spot_funds_leaves_no_state_behind() {
    for mode in REVIEW_MODES {
        let engine = spot_engine(
            vec![account_instance("review", mode, ACCOUNT, all())],
            "10000",
        );

        for _ in 0..2 {
            let start = engine.start_pre_trade_dry_run(aapl_usd(ACCOUNT));
            assert!(start.is_pass());
            assert!(start.reviews().is_empty());
            let full = engine.execute_pre_trade_dry_run(aapl_usd(ACCOUNT));
            assert!(full.is_pass());
            assert_eq!(full.reviews().len(), 1);
            assert_rule_record(&full.reviews()[0], mode);
            assert_untouched(&engine);
        }

        let mut live = reserve(&engine, aapl_usd(ACCOUNT));
        let full = engine.execute_pre_trade_dry_run(aapl_usd(ACCOUNT));
        assert_eq!(full.reviews(), live.reviews());
        live.rollback();
        assert_untouched(&engine);
    }
}

// ── union across levels and instances ─────────────────────────────────────────

#[test]
fn rules_of_different_instances_add_up_and_the_account_rule_does_not_narrow_the_group() {
    let engine = plain_engine(vec![
        instance(
            "group-review",
            RuleMode::ReviewWithHold,
            None,
            vec![group_rule(all())],
            vec![],
        ),
        account_instance(
            "account-reject",
            RuleMode::Reject,
            ACCOUNT,
            exact(&[("AAPL", "USD")]),
        ),
    ]);
    engine
        .accounts()
        .register_group(&[account(ACCOUNT)], group())
        .expect("register group");

    // Both rules cover AAPL/USD of the member: the stricter verdict wins.
    let rejects = rejected(&engine, aapl_usd(ACCOUNT));
    assert_eq!(rejects.len(), 1);
    assert_rule_record(&rejects[0], RuleMode::Reject);

    // The account rule does not list MSFT/USD and must not hide the group rule.
    assert_marked(
        reserve(&engine, msft_usd(ACCOUNT)),
        RuleMode::ReviewWithHold,
    );

    // An account outside the group and without rules is untouched.
    assert_clean(&engine, aapl_usd(OTHER_ACCOUNT));
}

#[test]
fn account_rule_of_the_same_instance_does_not_narrow_the_group_rule() {
    for mode in ALL_MODES {
        let engine = plain_engine(vec![instance(
            "rules",
            mode,
            None,
            vec![group_rule(all())],
            vec![account_rule(ACCOUNT, exact(&[("AAPL", "USD")]))],
        )]);
        engine
            .accounts()
            .register_group(&[account(ACCOUNT)], group())
            .expect("register group");

        // MSFT/USD is covered only by the group rule, which the account rule
        // for the same account must not replace.
        if mode == RuleMode::Reject {
            let rejects = rejected(&engine, msft_usd(ACCOUNT));
            assert_eq!(rejects.len(), 1);
            assert_rule_record(&rejects[0], RuleMode::Reject);
        } else {
            assert_marked(reserve(&engine, msft_usd(ACCOUNT)), mode);
        }
    }
}

#[test]
fn global_rule_still_applies_when_a_more_specific_rule_does_not_list_the_instrument() {
    let engine = plain_engine(vec![instance(
        "rules",
        RuleMode::ReviewWithoutHold,
        Some(exact(&[("MSFT", "USD")])),
        vec![],
        vec![account_rule(ACCOUNT, exact(&[("AAPL", "USD")]))],
    )]);

    assert_marked(
        reserve(&engine, aapl_usd(ACCOUNT)),
        RuleMode::ReviewWithoutHold,
    );
    assert_marked(
        reserve(&engine, msft_usd(ACCOUNT)),
        RuleMode::ReviewWithoutHold,
    );
    assert_clean(&engine, aapl_eur(ACCOUNT));
}

#[test]
fn both_review_instances_matching_one_order_hold_nothing() {
    let engine = spot_engine(
        vec![
            account_instance("hold", RuleMode::ReviewWithHold, ACCOUNT, all()),
            instance(
                "no-hold",
                RuleMode::ReviewWithoutHold,
                None,
                vec![group_rule(all())],
                vec![],
            ),
        ],
        "10000",
    );
    engine
        .accounts()
        .register_group(&[account(ACCOUNT)], group())
        .expect("register group");

    let mut reservation = reserve(&engine, aapl_usd(ACCOUNT));
    let reviews = reservation.reviews();
    assert_eq!(reviews.len(), 2);
    assert_eq!(marks_of(reviews, RuleMode::ReviewWithHold), 1);
    assert_eq!(marks_of(reviews, RuleMode::ReviewWithoutHold), 1);
    for mark in reviews {
        assert_eq!(mark.code, RejectCode::ComplianceRestriction);
        assert_eq!(mark.scope, RejectScope::Order);
    }
    // The stricter mark wins: the funds are not held.
    assert!(reservation.lock().is_empty());
    assert!(reservation.account_adjustments().is_empty());
    assert_untouched(&engine);
    reservation.commit();
    assert_untouched(&engine);
}

// ── principle: the review does not change the other checks ────────────────────

#[test]
fn review_rule_does_not_hide_the_funds_check() {
    for mode in REVIEW_MODES {
        // 100 USD against a 2000 USD order.
        let engine = spot_engine(
            vec![account_instance("review", mode, ACCOUNT, all())],
            "100",
        );

        let rejects = rejected(&engine, aapl_usd(ACCOUNT));
        assert_eq!(rejects.len(), 1, "the mark is dropped, the reject stays");
        assert_eq!(
            rejects[0].policy,
            SpotFundsPolicy::<FullSync, FullSync>::NAME
        );
        assert_eq!(rejects[0].code, RejectCode::InsufficientFunds);
        assert_eq!(rejects[0].mode, RuleMode::Reject);
        assert_balances(&engine, ["100", "0", "0"], ["3", "0", "0"]);
    }
}

#[test]
fn review_with_hold_rule_holds_the_funds_and_marks() {
    let engine = spot_engine(
        vec![account_instance(
            "review",
            RuleMode::ReviewWithHold,
            ACCOUNT,
            all(),
        )],
        "10000",
    );

    let mut reservation = reserve(&engine, aapl_usd(ACCOUNT));
    assert_eq!(reservation.reviews().len(), 1);
    assert_rule_record(&reservation.reviews()[0], RuleMode::ReviewWithHold);
    assert!(!reservation.lock().is_empty(), "the hold carries its lock");
    reservation.commit();
    assert_held(&engine);
}

#[test]
fn review_without_hold_rule_holds_nothing_and_marks() {
    let engine = spot_engine(
        vec![account_instance(
            "review",
            RuleMode::ReviewWithoutHold,
            ACCOUNT,
            all(),
        )],
        "10000",
    );

    let mut reservation = reserve(&engine, aapl_usd(ACCOUNT));
    assert_eq!(reservation.reviews().len(), 1);
    assert_rule_record(&reservation.reviews()[0], RuleMode::ReviewWithoutHold);
    assert!(reservation.lock().is_empty());
    assert!(reservation.account_adjustments().is_empty());
    assert_untouched(&engine);
    reservation.commit();
    assert_untouched(&engine);
}

#[test]
fn confirming_a_held_review_order_takes_no_second_hold_and_marks_nothing() {
    let engine = spot_engine(
        vec![account_instance(
            "review",
            RuleMode::ReviewWithHold,
            ACCOUNT,
            all(),
        )],
        "10000",
    );

    let mut first = reserve(&engine, aapl_usd(ACCOUNT));
    assert_eq!(first.reviews().len(), 1);
    first.commit();
    assert_held(&engine);

    let mut rerun = reserve(&engine, order(ACCOUNT, "AAPL", "USD", HELD_RERUN));
    assert!(
        rerun.reviews().is_empty(),
        "the confirmed rerun is not marked"
    );
    assert!(rerun.lock().is_empty(), "the rerun takes no second hold");
    assert!(rerun.account_adjustments().is_empty());
    rerun.commit();
    assert_held(&engine);
}

// ── retirement ────────────────────────────────────────────────────────────────

fn refusal_names(error: AccountRetirementError) -> Vec<(String, AccountRetirementRefusal)> {
    match error {
        AccountRetirementError::Refused { refusals } => refusals
            .into_iter()
            .map(|AccountRetirementPolicyRefusal { policy, refusal }| (policy, refusal))
            .collect(),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn account_rule_refuses_retirement_until_the_account_rules_are_cleared() {
    let engine = plain_engine(vec![
        account_instance("account-rules", RuleMode::ReviewWithHold, ACCOUNT, all()),
        instance(
            "wide-rules",
            RuleMode::ReviewWithoutHold,
            Some(all()),
            vec![group_rule(all())],
            vec![],
        ),
    ]);

    for _ in 0..2 {
        assert_eq!(
            refusal_names(
                engine
                    .retire_account(account(ACCOUNT))
                    .expect_err("an account rule references the account")
            ),
            vec![(
                ReviewRulePolicy::<FullLocking>::NAME.to_owned(),
                AccountRetirementRefusal::ConfigurationReferencesAccount,
            )],
            "only the instance holding the account rule refuses"
        );
    }
    // The group and the global rule do not hold an account they do not name.
    assert_eq!(engine.retire_account(account(OTHER_ACCOUNT)), Ok(()));

    engine
        .configure()
        .review_rule::<openpit::pretrade::policies::ReviewRulePolicyError>(
            "account-rules",
            |settings| settings.set_account_rules([]),
        )
        .expect("clearing the account rules must publish");
    assert_eq!(engine.retire_account(account(ACCOUNT)), Ok(()));
}

#[test]
fn group_and_global_rules_alone_do_not_refuse_retirement() {
    let engine = plain_engine(vec![instance(
        "wide-rules",
        RuleMode::ReviewWithHold,
        Some(all()),
        vec![group_rule(all())],
        vec![],
    )]);
    engine
        .accounts()
        .register_group(&[account(ACCOUNT)], group())
        .expect("register group");

    assert_eq!(engine.retire_account(account(ACCOUNT)), Ok(()));
}

// ── build ─────────────────────────────────────────────────────────────────────

#[test]
fn review_instance_without_order_review_fails_the_build() {
    for mode in ALL_MODES {
        let instance = account_instance("review", mode, ACCOUNT, all());
        let result = Engine::builder::<Order, (), ()>()
            .full_sync()
            .pre_trade_instance(
                ReviewRulePolicy::<FullLocking>::new(instance.settings),
                PolicyInstance {
                    name: Some(instance.name.to_owned()),
                    mode: instance.mode,
                },
            )
            .build();
        assert_eq!(
            result.err(),
            if mode == RuleMode::Reject {
                None
            } else {
                Some(EngineBuildError::OrderReviewRequired {
                    name: "review".to_owned(),
                })
            }
        );
    }
}

// ── drop-copy ─────────────────────────────────────────────────────────────────

#[test]
fn drop_copy_is_not_subject_to_the_rules() {
    for mode in ALL_MODES {
        // The order was already executed: a matching rule neither rejects nor
        // marks it.
        let engine = plain_engine(vec![account_instance("rule", mode, ACCOUNT, all())]);
        assert!(
            engine.apply_drop_copy(aapl_usd(ACCOUNT)).is_ok(),
            "drop-copy must pass in {mode:?}"
        );

        // A failure to read a field the rule needs is fatal in drop-copy, so a
        // policy that did not skip the order would surface it here.
        let engine = engine_of::<FieldOrder>(vec![account_instance(
            "rule",
            mode,
            ACCOUNT,
            exact(&[("AAPL", "USD")]),
        )]);
        let unreadable = field_order(Ok(ACCOUNT), Err("instrument unreadable"));
        assert!(
            engine.apply_drop_copy(unreadable).is_ok(),
            "drop-copy must not read the order in {mode:?}"
        );
    }
}
