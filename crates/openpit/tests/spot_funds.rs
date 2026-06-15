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

use openpit::param::{
    AccountId, AdjustmentAmount, Asset, Pnl, PositionSize, Price, Quantity, Side, Trade,
    TradeAmount,
};
use openpit::pretrade::policies::{
    RateLimit, RateLimitBrokerBarrier, RateLimitPolicy, RateLimitSettings, SpotFundsPolicy,
    SpotFundsPricingSource, SpotFundsSettings,
};
use openpit::pretrade::{PreTradeDryRunReport, PreTradeLock, RejectCode, DEFAULT_POLICY_GROUP_ID};
use openpit::{
    Engine, FullSync, FullSyncEngine, HasAccountAdjustmentBalance,
    HasAccountAdjustmentBalanceAverageEntryPrice, HasAccountAdjustmentBalanceLowerBound,
    HasAccountAdjustmentBalanceRealizedPnl, HasAccountAdjustmentBalanceUpperBound,
    HasAccountAdjustmentHeld, HasAccountAdjustmentHeldLowerBound,
    HasAccountAdjustmentHeldUpperBound, HasAccountAdjustmentIncoming,
    HasAccountAdjustmentIncomingLowerBound, HasAccountAdjustmentIncomingUpperBound, HasAccountId,
    HasBalanceAsset, HasExecutionReportIsFinal, HasExecutionReportLastTrade, HasInstrument,
    HasLeavesQuantity, HasPreTradeLock, HasSide, Instrument, OrderOperation,
    RequestFieldAccessError, SpotFundsMarketData,
};

type TestOrder = OrderOperation;
type TestEngine = FullSyncEngine<TestOrder, TestReport, TestAdjustment>;

const ACC: u64 = 99224416;

// ── TestReport ────────────────────────────────────────────────────────────────

struct TestReport {
    instrument: Instrument,
    account_id: AccountId,
    side: Side,
    last_trade: Option<Trade>,
    leaves_quantity: Quantity,
    is_final: bool,
    lock: PreTradeLock,
}

impl HasInstrument for TestReport {
    fn instrument(&self) -> Result<&Instrument, RequestFieldAccessError> {
        Ok(&self.instrument)
    }
}

impl HasAccountId for TestReport {
    fn account_id(&self) -> Result<AccountId, RequestFieldAccessError> {
        Ok(self.account_id)
    }
}

impl HasSide for TestReport {
    fn side(&self) -> Result<Side, RequestFieldAccessError> {
        Ok(self.side)
    }
}

impl HasExecutionReportLastTrade for TestReport {
    fn last_trade(&self) -> Result<Option<Trade>, RequestFieldAccessError> {
        Ok(self.last_trade)
    }
}

impl HasLeavesQuantity for TestReport {
    fn leaves_quantity(&self) -> Result<Quantity, RequestFieldAccessError> {
        Ok(self.leaves_quantity)
    }
}

impl HasExecutionReportIsFinal for TestReport {
    fn is_final(&self) -> Result<bool, RequestFieldAccessError> {
        Ok(self.is_final)
    }
}

impl HasPreTradeLock for TestReport {
    fn lock(&self) -> Result<PreTradeLock, RequestFieldAccessError> {
        Ok(self.lock.clone())
    }
}

// ── TestAdjustment ────────────────────────────────────────────────────────────

struct TestAdjustment {
    asset: Asset,
    balance: Option<AdjustmentAmount>,
    balance_average_entry_price: Option<Price>,
    balance_realized_pnl: Option<Pnl>,
    balance_lower: Option<PositionSize>,
    balance_upper: Option<PositionSize>,
    held: Option<AdjustmentAmount>,
}

impl HasBalanceAsset for TestAdjustment {
    fn balance_asset(&self) -> Result<&Asset, RequestFieldAccessError> {
        Ok(&self.asset)
    }
}

impl HasAccountAdjustmentBalance for TestAdjustment {
    fn balance(&self) -> Result<Option<AdjustmentAmount>, RequestFieldAccessError> {
        Ok(self.balance)
    }
}

impl HasAccountAdjustmentBalanceAverageEntryPrice for TestAdjustment {
    fn balance_average_entry_price(&self) -> Result<Option<Price>, RequestFieldAccessError> {
        Ok(self.balance_average_entry_price)
    }
}

impl HasAccountAdjustmentBalanceRealizedPnl for TestAdjustment {
    fn balance_realized_pnl(&self) -> Result<Option<Pnl>, RequestFieldAccessError> {
        Ok(self.balance_realized_pnl)
    }
}

impl HasAccountAdjustmentBalanceLowerBound for TestAdjustment {
    fn balance_lower(&self) -> Result<Option<PositionSize>, RequestFieldAccessError> {
        Ok(self.balance_lower)
    }
}

impl HasAccountAdjustmentBalanceUpperBound for TestAdjustment {
    fn balance_upper(&self) -> Result<Option<PositionSize>, RequestFieldAccessError> {
        Ok(self.balance_upper)
    }
}

impl HasAccountAdjustmentHeld for TestAdjustment {
    fn held(&self) -> Result<Option<AdjustmentAmount>, RequestFieldAccessError> {
        Ok(self.held)
    }
}

impl HasAccountAdjustmentHeldLowerBound for TestAdjustment {
    fn held_lower(&self) -> Result<Option<PositionSize>, RequestFieldAccessError> {
        Ok(None)
    }
}

impl HasAccountAdjustmentHeldUpperBound for TestAdjustment {
    fn held_upper(&self) -> Result<Option<PositionSize>, RequestFieldAccessError> {
        Ok(None)
    }
}

impl HasAccountAdjustmentIncoming for TestAdjustment {
    fn incoming(&self) -> Result<Option<AdjustmentAmount>, RequestFieldAccessError> {
        Ok(None)
    }
}

impl HasAccountAdjustmentIncomingLowerBound for TestAdjustment {
    fn incoming_lower(&self) -> Result<Option<PositionSize>, RequestFieldAccessError> {
        Ok(None)
    }
}

impl HasAccountAdjustmentIncomingUpperBound for TestAdjustment {
    fn incoming_upper(&self) -> Result<Option<PositionSize>, RequestFieldAccessError> {
        Ok(None)
    }
}

// ── value helpers ─────────────────────────────────────────────────────────────

fn asset(s: &str) -> Asset {
    Asset::new(s).expect("valid asset")
}

fn instr(under: &str, sett: &str) -> Instrument {
    Instrument::new(asset(under), asset(sett))
}

fn ps(s: &str) -> PositionSize {
    PositionSize::from_str(s).expect("valid position size")
}

fn px(s: &str) -> Price {
    Price::from_str(s).expect("valid price")
}

fn pnl(s: &str) -> Pnl {
    Pnl::from_str(s).expect("valid pnl")
}

fn qty(s: &str) -> Quantity {
    Quantity::from_str(s).expect("valid quantity")
}

fn account() -> AccountId {
    AccountId::from_u64(ACC)
}

// ── request builders ──────────────────────────────────────────────────────────

fn make_order(
    side: Side,
    instrument: Instrument,
    trade_amount: TradeAmount,
    price: Option<Price>,
) -> TestOrder {
    OrderOperation {
        instrument,
        account_id: account(),
        side,
        trade_amount,
        price,
    }
}

fn make_report(
    instrument: Instrument,
    side: Side,
    last_trade: Option<Trade>,
    leaves: Quantity,
    is_final: bool,
    order_price: Option<Price>,
) -> TestReport {
    let lock = order_price
        .map(|p| PreTradeLock::from_entries([(DEFAULT_POLICY_GROUP_ID, p)]))
        .unwrap_or_default();
    TestReport {
        instrument,
        account_id: account(),
        side,
        last_trade,
        leaves_quantity: leaves,
        is_final,
        lock,
    }
}

fn balance_adjustment(asset_code: &str, amount: AdjustmentAmount) -> TestAdjustment {
    TestAdjustment {
        asset: asset(asset_code),
        balance: Some(amount),
        balance_average_entry_price: None,
        balance_realized_pnl: None,
        balance_lower: None,
        balance_upper: None,
        held: None,
    }
}

fn held_adj(asset_code: &str, amount: AdjustmentAmount) -> TestAdjustment {
    TestAdjustment {
        asset: asset(asset_code),
        balance: None,
        balance_average_entry_price: None,
        balance_realized_pnl: None,
        balance_lower: None,
        balance_upper: None,
        held: Some(amount),
    }
}

/// Builds a balance adjustment carrying an average entry price and/or a
/// realized-PnL force-set.
fn adj_with_avg_pnl(
    asset_code: &str,
    balance: Option<AdjustmentAmount>,
    average_entry_price: Option<Price>,
    realized_pnl: Option<Pnl>,
) -> TestAdjustment {
    TestAdjustment {
        asset: asset(asset_code),
        balance,
        balance_average_entry_price: average_entry_price,
        balance_realized_pnl: realized_pnl,
        balance_lower: None,
        balance_upper: None,
        held: None,
    }
}

/// Builds a balance adjustment with an upper bound, used to force a reject.
fn bounded_balance_adjustment(
    asset_code: &str,
    amount: AdjustmentAmount,
    upper: PositionSize,
) -> TestAdjustment {
    TestAdjustment {
        asset: asset(asset_code),
        balance: Some(amount),
        balance_average_entry_price: None,
        balance_realized_pnl: None,
        balance_lower: None,
        balance_upper: Some(upper),
        held: None,
    }
}

// ── engine builder ────────────────────────────────────────────────────────────

fn build_engine() -> TestEngine {
    let builder = Engine::builder::<TestOrder, TestReport, TestAdjustment>().full_sync();
    let settings = SpotFundsSettings::new(0, SpotFundsPricingSource::Mark, std::iter::empty())
        .expect("settings must build");
    let policy = SpotFundsPolicy::<FullSync, FullSync>::new(
        settings,
        None::<SpotFundsMarketData<FullSync>>,
        builder.storage_builder(),
    );
    builder
        .pre_trade(policy)
        .build()
        .expect("engine must build")
}

fn seed(engine: &TestEngine, asset_code: &str, amount: &str) {
    let adj = balance_adjustment(asset_code, AdjustmentAmount::Absolute(ps(amount)));
    engine
        .apply_account_adjustment(account(), &[adj])
        .expect("seed must succeed");
}

// ═════════════════════════════════════════════════════════════════════════════
// Tests
// ═════════════════════════════════════════════════════════════════════════════

// Verifies the full pipeline: reserve → fill → credit.
// After a complete fill the settlement asset decreases by the filled notional
// and the underlying asset is credited by the filled quantity.
// Indirect: subsequent orders whose outcome depends on the expected state.
#[test]
fn buy_limit_full_fill_reduces_settlement_and_credits_underlying() {
    let engine = build_engine();
    seed(&engine, "USD", "10000");

    let aapl_usd = instr("AAPL", "USD");

    // Reserve Buy 10 AAPL @ 200 → holds 2000 USD; available drops to 8000.
    let mut reservation = engine
        .execute_pre_trade(make_order(
            Side::Buy,
            aapl_usd.clone(),
            TradeAmount::Quantity(qty("10")),
            Some(px("200")),
        ))
        .expect("pre-trade must accept");
    reservation.commit();

    // Final fill: 10 @ 200, leaves = 0.
    // Consumes 2000 from held; credits 10 AAPL.
    let report = make_report(
        aapl_usd.clone(),
        Side::Buy,
        Some(Trade {
            price: px("200"),
            quantity: qty("10"),
        }),
        qty("0"),
        true,
        Some(px("200")),
    );
    let post = engine.apply_execution_report(&report);
    assert!(post.account_blocks.is_empty());

    // USD available = 8000: Buy 40 @ 200 (notional 8000) must fit.
    assert!(
        engine
            .execute_pre_trade(make_order(
                Side::Buy,
                aapl_usd.clone(),
                TradeAmount::Quantity(qty("40")),
                Some(px("200")),
            ))
            .is_ok(),
        "Buy 40 @ 200 must fit USD available = 8000"
    );

    // Buy 41 @ 200 = 8200 > 8000 must be rejected.
    assert!(
        engine
            .execute_pre_trade(make_order(
                Side::Buy,
                aapl_usd.clone(),
                TradeAmount::Quantity(qty("41")),
                Some(px("200")),
            ))
            .is_err(),
        "Buy 41 @ 200 must exceed USD available = 8000"
    );

    // AAPL available = 10: Sell 10 must fit.
    assert!(
        engine
            .execute_pre_trade(make_order(
                Side::Sell,
                aapl_usd.clone(),
                TradeAmount::Quantity(qty("10")),
                None,
            ))
            .is_ok(),
        "Sell 10 AAPL must fit available = 10"
    );

    // Sell 11 AAPL = 11 > 10 must be rejected.
    assert!(
        engine
            .execute_pre_trade(make_order(
                Side::Sell,
                aapl_usd,
                TradeAmount::Quantity(qty("11")),
                None,
            ))
            .is_err(),
        "Sell 11 AAPL must exceed available = 10"
    );
}

// Verifies that a rejected order leaves holdings unchanged.
// A Buy whose notional exceeds available funds must be rejected and the
// subsequent order must still see the original available amount.
#[test]
fn buy_insufficient_funds_rejects_with_state_unchanged() {
    let engine = build_engine();
    seed(&engine, "USD", "10000");

    let aapl_usd = instr("AAPL", "USD");

    // Buy 100 AAPL @ 200 = 20000 notional > 10000 available → reject.
    let result = engine.execute_pre_trade(make_order(
        Side::Buy,
        aapl_usd.clone(),
        TradeAmount::Quantity(qty("100")),
        Some(px("200")),
    ));
    let Err(rejects) = result else {
        panic!("must reject: notional exceeds available")
    };
    assert_eq!(rejects[0].code, RejectCode::InsufficientFunds);

    // State is unchanged: USD = 10000 is still fully available after the rejection.
    assert!(
        engine
            .execute_pre_trade(make_order(
                Side::Buy,
                aapl_usd,
                TradeAmount::Quantity(qty("50")),
                Some(px("200")),
            ))
            .is_ok(),
        "Buy 50 @ 200 (= 10000 notional) must succeed after rejection: USD available = 10000"
    );
}

// Verifies that a partial fill followed by a cancel releases only the unfilled
// portion from held back to available, and that the filled underlying is credited.
#[test]
fn cancel_with_leftover_releases_unfilled_held() {
    // Init: USD = 10000.
    // Reserve Buy 10 @ 200 → held = 2000, available = 8000.
    // Partial fill 4 @ 200: consume 800 from held → held = 1200; AAPL available = 4.
    // Cancel leaves = 6: release 6 * 200 = 1200 → held = 0, available = 8000 + 1200 = 9200.
    let engine = build_engine();
    seed(&engine, "USD", "10000");

    let aapl_usd = instr("AAPL", "USD");

    let mut reservation = engine
        .execute_pre_trade(make_order(
            Side::Buy,
            aapl_usd.clone(),
            TradeAmount::Quantity(qty("10")),
            Some(px("200")),
        ))
        .expect("pre-trade must accept");
    reservation.commit();

    // Partial fill 4 @ 200, not final.
    engine.apply_execution_report(&make_report(
        aapl_usd.clone(),
        Side::Buy,
        Some(Trade {
            price: px("200"),
            quantity: qty("4"),
        }),
        qty("6"),
        false,
        Some(px("200")),
    ));

    // Cancel: leaves = 6, final, no new trade.
    engine.apply_execution_report(&make_report(
        aapl_usd.clone(),
        Side::Buy,
        None,
        qty("6"),
        true,
        Some(px("200")),
    ));

    // USD available = 9200: Buy 46 @ 200 (notional 9200) must fit.
    assert!(
        engine
            .execute_pre_trade(make_order(
                Side::Buy,
                aapl_usd.clone(),
                TradeAmount::Quantity(qty("46")),
                Some(px("200")),
            ))
            .is_ok(),
        "USD available must be 9200 after release"
    );

    // Buy 47 @ 200 = 9400 > 9200 must be rejected.
    assert!(
        engine
            .execute_pre_trade(make_order(
                Side::Buy,
                aapl_usd.clone(),
                TradeAmount::Quantity(qty("47")),
                Some(px("200")),
            ))
            .is_err(),
        "Buy 47 @ 200 must exceed USD available = 9200"
    );

    // AAPL available = 4: Sell 4 must fit.
    assert!(
        engine
            .execute_pre_trade(make_order(
                Side::Sell,
                aapl_usd.clone(),
                TradeAmount::Quantity(qty("4")),
                None,
            ))
            .is_ok(),
        "AAPL available must be 4 from partial fill"
    );

    // Sell 5 AAPL > 4 must be rejected.
    assert!(
        engine
            .execute_pre_trade(make_order(
                Side::Sell,
                aapl_usd,
                TradeAmount::Quantity(qty("5")),
                None,
            ))
            .is_err(),
        "Sell 5 AAPL must exceed available = 4"
    );
}

// Verifies that a Delta adjustment adds to available funds.
// After Delta(+5000) on an account with USD = 10000 the total becomes 15000.
#[test]
fn limits_adjustment_delta_adds_to_available() {
    let engine = build_engine();
    seed(&engine, "USD", "10000");

    let adj = balance_adjustment("USD", AdjustmentAmount::Delta(ps("5000")));
    engine
        .apply_account_adjustment(account(), &[adj])
        .expect("delta adjustment must succeed");

    let aapl_usd = instr("AAPL", "USD");

    // USD available = 15000: Buy 75 @ 200 (notional 15000) must pass; 76 must fail.
    assert!(
        engine
            .execute_pre_trade(make_order(
                Side::Buy,
                aapl_usd.clone(),
                TradeAmount::Quantity(qty("75")),
                Some(px("200")),
            ))
            .is_ok(),
        "Buy 75 @ 200 must fit USD available = 15000"
    );
    assert!(
        engine
            .execute_pre_trade(make_order(
                Side::Buy,
                aapl_usd,
                TradeAmount::Quantity(qty("76")),
                Some(px("200")),
            ))
            .is_err(),
        "Buy 76 @ 200 must exceed USD available = 15000"
    );
}

// Verifies that an Absolute adjustment creates a new asset entry from scratch.
// An account that had no EUR balance can receive funds via Absolute adjustment
// and subsequently use those funds in EUR-settled orders.
#[test]
fn limits_adjustment_absolute_creates_entry() {
    let engine = build_engine();

    // No EUR seeded initially; create entry via Absolute(1000).
    let adj = balance_adjustment("EUR", AdjustmentAmount::Absolute(ps("1000")));
    engine
        .apply_account_adjustment(account(), &[adj])
        .expect("absolute adjustment must succeed");

    let aapl_eur = instr("AAPL", "EUR");

    // EUR available = 1000: Buy 5 @ 200 EUR (notional 1000) must pass; 6 must fail.
    assert!(
        engine
            .execute_pre_trade(make_order(
                Side::Buy,
                aapl_eur.clone(),
                TradeAmount::Quantity(qty("5")),
                Some(px("200")),
            ))
            .is_ok(),
        "Buy 5 @ 200 EUR must fit newly created EUR balance = 1000"
    );
    assert!(
        engine
            .execute_pre_trade(make_order(
                Side::Buy,
                aapl_eur,
                TradeAmount::Quantity(qty("6")),
                Some(px("200")),
            ))
            .is_err(),
        "Buy 6 @ 200 EUR must exceed EUR available = 1000"
    );
}

// Verifies that a manager-initiated held=-2000 adjustment reduces the
// spendable capacity below available. Even though available=2000, the net
// spendable is 0 and any buy must be rejected with InsufficientFunds.
#[test]
fn negative_held_blocks_buy_despite_positive_available() {
    let engine = build_engine();

    // Seed balance=2000 USD.
    seed(&engine, "USD", "2000");

    // Manager sets held=-2000 USD (indicates a shortfall / reconciliation
    // adjustment). Net spendable = available(2000) + held(-2000) = 0.
    let adj = held_adj("USD", AdjustmentAmount::Absolute(ps("-2000")));
    engine
        .apply_account_adjustment(account(), &[adj])
        .expect("held adjustment must be accepted");

    let aapl_usd = instr("AAPL", "USD");

    // Any buy, however small, must be rejected since spendable = 0.
    let result = engine.execute_pre_trade(make_order(
        Side::Buy,
        aapl_usd,
        TradeAmount::Quantity(qty("1")),
        Some(px("1")),
    ));
    let Err(rejects) = result else {
        panic!("buy must be rejected when held=-2000 cancels out available=2000")
    };
    assert_eq!(rejects[0].code, RejectCode::InsufficientFunds);
}

// ── pre-trade dry-run idempotency (rate-limit + spot-funds) ────────────────────

// Engine wiring both an immediate-side-effect start-stage policy (rate limit,
// broker max 1) and an immediate-side-effect main-stage policy (spot funds).
fn build_rate_limited_engine() -> TestEngine {
    let builder = Engine::builder::<TestOrder, TestReport, TestAdjustment>().full_sync();
    let rate_limit = RateLimitPolicy::new(
        RateLimitSettings::new(
            Some(RateLimitBrokerBarrier {
                limit: RateLimit {
                    max_orders: 1,
                    window: std::time::Duration::from_secs(60),
                },
            }),
            [],
            [],
            [],
        )
        .expect("rate-limit settings must build"),
        builder.storage_builder(),
    );
    let spot_funds = SpotFundsPolicy::<FullSync, FullSync>::new(
        SpotFundsSettings::new(0, SpotFundsPricingSource::Mark, std::iter::empty())
            .expect("spot-funds settings must build"),
        None::<SpotFundsMarketData<FullSync>>,
        builder.storage_builder(),
    );
    builder
        .pre_trade(rate_limit)
        .pre_trade(spot_funds)
        .build()
        .expect("engine must build")
}

fn seed_rate_limited(engine: &TestEngine, asset_code: &str, amount: &str) {
    let adj = balance_adjustment(asset_code, AdjustmentAmount::Absolute(ps(amount)));
    engine
        .apply_account_adjustment(account(), &[adj])
        .expect("seed must succeed");
}

#[test]
fn dry_run_does_not_spend_rate_limit_budget_or_reserve_spot_funds() {
    let engine = build_rate_limited_engine();
    seed_rate_limited(&engine, "USD", "10000");
    let aapl_usd = instr("AAPL", "USD");

    // Many dry-runs of a valid order: every one passes (rate-limit dry-run never
    // rejects) and none moves engine state.
    for _ in 0..5 {
        let report: PreTradeDryRunReport = engine.execute_pre_trade_dry_run(make_order(
            Side::Buy,
            aapl_usd.clone(),
            TradeAmount::Quantity(qty("10")),
            Some(px("200")),
        ));
        assert!(report.is_pass(), "dry-run of a valid order must pass");
        assert!(report.account_block().is_none());
        // The would-be settlement hold is reported without being applied.
        assert_eq!(report.account_adjustments().len(), 1);
    }

    // The broker limit is 1, so if the five dry-runs had each consumed a slot,
    // this first real order would be the sixth and would be rejected. It must
    // succeed, proving the dry-runs spent no rate-limit budget. It reserving the
    // full 10000 notional also proves no spot-funds hold leaked from the
    // dry-runs (available is still the seeded 10000).
    let mut reservation = engine
        .execute_pre_trade(make_order(
            Side::Buy,
            aapl_usd.clone(),
            TradeAmount::Quantity(qty("50")),
            Some(px("200")),
        ))
        .expect("first real order must pass: dry-runs consumed no budget or funds");
    reservation.commit();

    // The broker counter now holds exactly one real order; the next real order
    // is rejected on the rate limit, confirming the budget reflects only real
    // calls.
    let Err(rejects) = engine.execute_pre_trade(make_order(
        Side::Buy,
        aapl_usd,
        TradeAmount::Quantity(qty("1")),
        Some(px("200")),
    )) else {
        panic!("second real order must breach the broker limit of 1");
    };
    assert_eq!(rejects[0].code, RejectCode::RateLimitExceeded);
}

#[test]
fn dry_run_insufficient_funds_reports_reject_and_leaves_state_for_real_call() {
    let engine = build_rate_limited_engine();
    seed_rate_limited(&engine, "USD", "1000");
    let aapl_usd = instr("AAPL", "USD");

    // Buy 10 @ 200 = 2000 notional > 1000 available: the dry-run rejects with
    // InsufficientFunds and records nothing.
    let report = engine.execute_pre_trade_dry_run(make_order(
        Side::Buy,
        aapl_usd.clone(),
        TradeAmount::Quantity(qty("10")),
        Some(px("200")),
    ));
    assert!(!report.is_pass());
    let rejects = report.rejects().expect("dry-run must report rejects");
    assert_eq!(rejects[0].code, RejectCode::InsufficientFunds);

    // A real order that fits the untouched 1000 available still succeeds, and is
    // the first to consume the broker slot.
    let mut reservation = engine
        .execute_pre_trade(make_order(
            Side::Buy,
            aapl_usd,
            TradeAmount::Quantity(qty("5")),
            Some(px("200")),
        ))
        .expect("real order within available must pass after a rejecting dry-run");
    reservation.commit();
}

// ── realized PnL / average entry price outcomes (public API) ───────────────────

// Happy path: an adjustment that force-sets both the average entry price and
// realized PnL surfaces them in the batch outcome as absolute (and delta for
// PnL) values.
#[test]
fn adjustment_outcome_surfaces_average_and_realized_pnl() {
    let engine = build_engine();
    // Seed a non-flat slot with an average and an initial realized PnL of 30.
    engine
        .apply_account_adjustment(
            account(),
            &[adj_with_avg_pnl(
                "AAPL",
                Some(AdjustmentAmount::Absolute(ps("10"))),
                Some(px("100")),
                Some(pnl("30")),
            )],
        )
        .expect("seed must succeed");

    // Force realized PnL to 50 (delta +20) and the average to 150.
    let result = engine
        .apply_account_adjustment(
            account(),
            &[adj_with_avg_pnl(
                "AAPL",
                Some(AdjustmentAmount::Delta(ps("0"))),
                Some(px("150")),
                Some(pnl("50")),
            )],
        )
        .expect("force-set must succeed");

    let entry = &result
        .outcomes
        .first()
        .expect("one outcome entry expected")
        .entry;
    assert_eq!(entry.asset, asset("AAPL"));
    assert_eq!(entry.average_entry_price, Some(px("150")));
    let pnl_outcome = entry
        .realized_pnl
        .expect("realized PnL must be surfaced on a force-set");
    assert_eq!(pnl_outcome.delta, pnl("20"));
    assert_eq!(pnl_outcome.absolute, pnl("50"));
}

// Boundary: force-setting realized PnL to exactly zero on a previously untracked
// slot still surfaces a tracked outcome (absolute 0), distinct from the
// "untracked, no outcome" case.
#[test]
fn adjustment_outcome_realized_pnl_zero_boundary_is_tracked() {
    let engine = build_engine();
    let result = engine
        .apply_account_adjustment(
            account(),
            &[adj_with_avg_pnl(
                "AAPL",
                Some(AdjustmentAmount::Absolute(ps("10"))),
                None,
                Some(pnl("0")),
            )],
        )
        .expect("force-set must succeed");

    let entry = &result
        .outcomes
        .first()
        .expect("one outcome entry expected")
        .entry;
    let pnl_outcome = entry
        .realized_pnl
        .expect("a zero force-set still surfaces a tracked realized PnL");
    assert_eq!(pnl_outcome.delta, pnl("0"));
    assert_eq!(pnl_outcome.absolute, pnl("0"));
}

// Regression for the realized-PnL rollback bug, via the public batch API: a
// batch whose later element rejects is rolled back as a whole. A non-flat slot
// that was untracked (realized_pnl = None) and force-set earlier in the batch
// must return to None, NOT Some(0) — and must therefore not auto-resume realized
// tracking on a subsequent fill.
#[test]
fn rejected_batch_rolls_untracked_realized_pnl_back_to_none() {
    let engine = build_engine();
    // Seed a long with an average but no realized-PnL tracking, plus an
    // unrelated asset whose bound the second batch element will breach.
    engine
        .apply_account_adjustment(
            account(),
            &[
                adj_with_avg_pnl(
                    "AAPL",
                    Some(AdjustmentAmount::Absolute(ps("10"))),
                    Some(px("100")),
                    None,
                ),
                balance_adjustment("USD", AdjustmentAmount::Absolute(ps("1000"))),
            ],
        )
        .expect("seed must succeed");

    // Batch: [force-set AAPL realized to 25, then a USD adjustment that breaches
    // an upper bound]. The whole batch must reject and roll back.
    let err = engine
        .apply_account_adjustment(
            account(),
            &[
                adj_with_avg_pnl(
                    "AAPL",
                    Some(AdjustmentAmount::Delta(ps("0"))),
                    None,
                    Some(pnl("25")),
                ),
                bounded_balance_adjustment("USD", AdjustmentAmount::Delta(ps("5000")), ps("2000")),
            ],
        )
        .expect_err("second element must breach its upper bound");
    assert_eq!(err.failed_adjustment_index, 1);

    // The AAPL slot must be back to untracked realized PnL. We assert this
    // through a subsequent fill: selling into the long realizes nothing and
    // reports no realized PnL, proving tracking did not auto-resume (it would
    // have if rollback had left Some(0)).
    let aapl_usd = instr("AAPL", "USD");
    let mut reservation = engine
        .execute_pre_trade(make_order(
            Side::Sell,
            aapl_usd.clone(),
            TradeAmount::Quantity(qty("4")),
            Some(px("130")),
        ))
        .expect("sell pre-trade must accept");
    reservation.commit();
    let post = engine.apply_execution_report(&make_report(
        aapl_usd,
        Side::Sell,
        Some(Trade {
            price: px("130"),
            quantity: qty("4"),
        }),
        qty("0"),
        true,
        Some(px("130")),
    ));
    let aapl_entry = post
        .account_adjustments
        .iter()
        .find(|o| o.entry.asset == asset("AAPL"))
        .expect("AAPL post-trade entry must exist");
    assert!(
        aapl_entry.entry.realized_pnl.is_none(),
        "untracked slot must not auto-resume realized-PnL tracking after rollback"
    );
}
