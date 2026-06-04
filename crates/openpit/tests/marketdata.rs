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

use std::sync::Arc;
use std::time::Duration;

use openpit::param::{
    AccountGroupId, AccountId, AdjustmentAmount, Asset, PositionSize, Price, Quantity, Side, Trade,
    TradeAmount, DEFAULT_ACCOUNT_GROUP,
};
use openpit::pretrade::policies::SpotFundsPolicy;
use openpit::pretrade::{PreTradeLock, RejectCode};
use openpit::{
    AccountInfo, AlreadyRegistered, Engine, FullSync, FullSyncEngine, HasAccountAdjustmentBalance,
    HasAccountAdjustmentBalanceLowerBound, HasAccountAdjustmentBalanceUpperBound,
    HasAccountAdjustmentHeld, HasAccountAdjustmentHeldLowerBound,
    HasAccountAdjustmentHeldUpperBound, HasAccountAdjustmentIncoming,
    HasAccountAdjustmentIncomingLowerBound, HasAccountAdjustmentIncomingUpperBound, HasAccountId,
    HasBalanceAsset, HasExecutionReportIsFinal, HasExecutionReportLastTrade, HasInstrument,
    HasLeavesQuantity, HasPreTradeLock, HasSide, Instrument, LocalSync, MarketDataBuilder,
    MarketDataError, MarketDataService, OrderOperation, PushForError, Quote, QuoteResolution,
    QuoteTtl, RegistrationError, RequestFieldAccessError, SpotFundsMarketData, SpotFundsOverride,
    SpotFundsOverrideTarget, SpotFundsPricingSource, UnknownInstrumentId,
};

// ── Value helpers ─────────────────────────────────────────────────────────────

fn asset(s: &str) -> Asset {
    Asset::new(s).expect("valid asset")
}

fn instr(under: &str, sett: &str) -> Instrument {
    Instrument::new(asset(under), asset(sett))
}

fn px(s: &str) -> Price {
    Price::from_str(s).expect("valid price")
}

fn ps(s: &str) -> PositionSize {
    PositionSize::from_str(s).expect("valid position size")
}

fn qty(s: &str) -> Quantity {
    Quantity::from_str(s).expect("valid quantity")
}

fn acc(id: u64) -> AccountId {
    AccountId::from_u64(id)
}

fn grp(id: u32) -> AccountGroupId {
    AccountGroupId::from_u32(id).expect("valid account group id")
}

/// A pre-resolved account info: `Some(group)` or `None`. `Option<AccountGroupId>`
/// implements `AccountInfo`, so it can be passed directly to `get`.
fn group_source(group: Option<AccountGroupId>) -> Option<AccountGroupId> {
    group
}

/// Reads the default ("everyone-else") bucket: any account, no group, widest
/// resolution so the per-account and per-group buckets miss and the read falls
/// through to the default group.
fn get_default<Sync: openpit::MarketDataSync>(
    svc: &MarketDataService<Sync>,
    id: openpit::InstrumentId,
) -> Option<Quote> {
    svc.get(
        id,
        acc(1),
        &group_source(None),
        QuoteResolution::AccountThenGroupThenDefault,
    )
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test 1 — LocalSync: register, push, get
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn local_sync_register_push_get() {
    let svc = MarketDataBuilder::<LocalSync>::new(QuoteTtl::Infinite).build();
    let aapl = instr("AAPL", "USD");
    let id = svc
        .register(aapl.clone())
        .expect("first register must succeed");

    assert!(get_default(&svc, id).is_none());

    let q = Quote::new().with_mark(px("150"));
    svc.push(id, q)
        .expect("push must succeed for registered id");

    let got = get_default(&svc, id).expect("quote must be present");
    assert_eq!(got, q);
    assert_eq!(got.mark, Some(px("150")));
    assert_eq!(got.bid, None);
    assert_eq!(got.ask, None);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test 2 — FullSync: concurrent reads under push storm
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn full_sync_concurrent_reads_under_push_storm() {
    let svc: Arc<MarketDataService<FullSync>> =
        MarketDataBuilder::<FullSync>::new(QuoteTtl::Infinite).build();
    let id = svc
        .register(instr("AAPL", "USD"))
        .expect("register must succeed");

    svc.push(id, Quote::new().with_mark(px("100")))
        .expect("initial push must succeed");

    let num_readers = 4;
    let reads_each = 1_000;
    let mut handles = Vec::with_capacity(num_readers);

    for _ in 0..num_readers {
        let svc_clone = Arc::clone(&svc);
        handles.push(std::thread::spawn(move || {
            for _ in 0..reads_each {
                let q = get_default(&svc_clone, id);
                assert!(
                    q.is_some(),
                    "quote should always be present after initial push"
                );
            }
        }));
    }

    let svc_producer = Arc::clone(&svc);
    let producer = std::thread::spawn(move || {
        for i in 0u32..1_000 {
            let p = Price::from_str(&(100 + i).to_string()).unwrap_or(px("100"));
            svc_producer
                .push(id, Quote::new().with_mark(p))
                .expect("push must succeed");
        }
    });

    producer.join().expect("producer must finish");
    for h in handles {
        h.join().expect("reader must finish");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test 3 — Quote carries optional bid/ask
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn quote_carries_optional_bid_ask() {
    let svc = MarketDataBuilder::<LocalSync>::new(QuoteTtl::Infinite).build();
    let id = svc
        .register(instr("AAPL", "USD"))
        .expect("register must succeed");

    let q = Quote::new()
        .with_mark(px("150"))
        .with_bid(px("149.5"))
        .with_ask(px("150.5"));
    svc.push(id, q).expect("push must succeed");

    let got = get_default(&svc, id).expect("quote must be present");
    assert_eq!(got.mark, Some(px("150")));
    assert_eq!(got.bid, Some(px("149.5")));
    assert_eq!(got.ask, Some(px("150.5")));
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test 4 — Infinite TTL keeps quote visible indefinitely
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn infinite_ttl_keeps_quote_visible() {
    let svc = MarketDataBuilder::<LocalSync>::new(QuoteTtl::Infinite).build();
    let id = svc
        .register(instr("AAPL", "USD"))
        .expect("register must succeed");
    svc.push(id, Quote::new().with_mark(px("100")))
        .expect("push must succeed");

    std::thread::sleep(Duration::from_millis(80));
    assert!(
        get_default(&svc, id).is_some(),
        "infinite TTL must not expire quotes"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test 5 — Finite TTL hides aged quote
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn finite_ttl_hides_aged_quote() {
    let ttl = Duration::from_millis(500);
    let svc = MarketDataBuilder::<LocalSync>::new(QuoteTtl::Within(ttl)).build();
    let id = svc
        .register(instr("AAPL", "USD"))
        .expect("register must succeed");
    svc.push(id, Quote::new().with_mark(px("100")))
        .expect("push must succeed");

    assert!(
        get_default(&svc, id).is_some(),
        "quote must be visible right after push"
    );

    std::thread::sleep(ttl + Duration::from_millis(50));
    assert!(
        get_default(&svc, id).is_none(),
        "quote must be hidden after TTL elapses"
    );

    let err = svc
        .get_or_err(
            id,
            acc(1),
            &group_source(None),
            QuoteResolution::AccountThenGroupThenDefault,
        )
        .expect_err("get_or_err must surface staleness");
    assert_eq!(err, MarketDataError::QuoteUnavailable);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test 6 — Per-instrument TTL override beats service default
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn per_instrument_ttl_override_beats_service_default() {
    let short = Duration::from_millis(500);
    let svc = MarketDataBuilder::<LocalSync>::new(QuoteTtl::Within(short)).build();

    let short_id = svc
        .register(instr("AAPL", "USD"))
        .expect("register must succeed");
    let infinite_id = svc
        .register_with_ttl(instr("MSFT", "USD"), QuoteTtl::Infinite)
        .expect("register_with_ttl must succeed");

    svc.push(short_id, Quote::new().with_mark(px("100")))
        .expect("push must succeed");
    svc.push(infinite_id, Quote::new().with_mark(px("200")))
        .expect("push must succeed");

    std::thread::sleep(short + Duration::from_millis(50));
    assert!(
        get_default(&svc, short_id).is_none(),
        "service-default TTL must expire its quote"
    );
    assert!(
        get_default(&svc, infinite_id).is_some(),
        "per-instrument Infinite override must keep its quote visible"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test 7 — Push after TTL expiry restores visibility
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn push_after_ttl_expiry_restores_visibility() {
    let ttl = Duration::from_millis(400);
    let svc = MarketDataBuilder::<LocalSync>::new(QuoteTtl::Within(ttl)).build();
    let id = svc
        .register(instr("AAPL", "USD"))
        .expect("register must succeed");
    svc.push(id, Quote::new().with_mark(px("100")))
        .expect("push must succeed");

    std::thread::sleep(ttl + Duration::from_millis(50));
    assert!(get_default(&svc, id).is_none());

    svc.push(id, Quote::new().with_mark(px("110")))
        .expect("re-push must succeed");
    let q = get_default(&svc, id).expect("fresh push must be visible again");
    assert_eq!(q.mark, Some(px("110")));
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test 8 — Clear hides quote without touching the registry
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn clear_hides_quote_without_removing_instrument() {
    let svc = MarketDataBuilder::<LocalSync>::new(QuoteTtl::Infinite).build();
    let aapl = instr("AAPL", "USD");
    let id = svc.register(aapl.clone()).expect("register must succeed");
    svc.push(id, Quote::new().with_mark(px("100")))
        .expect("push must succeed");
    assert!(get_default(&svc, id).is_some());

    svc.clear(id);
    assert!(get_default(&svc, id).is_none());
    assert_eq!(
        svc.resolve(&aapl),
        Some(id),
        "registry entry must remain after clear"
    );

    svc.push(id, Quote::new().with_mark(px("105")))
        .expect("re-push must succeed");
    assert_eq!(
        get_default(&svc, id).expect("re-push must be visible").mark,
        Some(px("105"))
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test — push (replace semantics): missing fields are cleared
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn push_replaces_all_fields() {
    let svc = MarketDataBuilder::<LocalSync>::new(QuoteTtl::Infinite).build();
    let id = svc
        .register(instr("AAPL", "USD"))
        .expect("register must succeed");

    svc.push(
        id,
        Quote::new()
            .with_mark(px("100"))
            .with_bid(px("99"))
            .with_ask(px("101")),
    )
    .expect("push must succeed");

    svc.push(id, Quote::new().with_bid(px("98")))
        .expect("second push must succeed");

    let got = get_default(&svc, id).expect("quote must be present");
    assert_eq!(got.mark, None, "replace must drop mark");
    assert_eq!(got.bid, Some(px("98")));
    assert_eq!(got.ask, None, "replace must drop ask");
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test — push_patch (merge semantics): missing fields preserve prior values
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn push_patch_preserves_unspecified_fields() {
    let svc = MarketDataBuilder::<LocalSync>::new(QuoteTtl::Infinite).build();
    let id = svc
        .register(instr("AAPL", "USD"))
        .expect("register must succeed");

    svc.push(
        id,
        Quote::new()
            .with_mark(px("100"))
            .with_bid(px("99"))
            .with_ask(px("101")),
    )
    .expect("push must succeed");

    svc.push_patch(id, Quote::new().with_mark(px("105")))
        .expect("push_patch must succeed");

    let got = get_default(&svc, id).expect("quote must be present");
    assert_eq!(
        got.mark,
        Some(px("105")),
        "patch must replace specified field"
    );
    assert_eq!(got.bid, Some(px("99")), "patch must keep prior bid");
    assert_eq!(got.ask, Some(px("101")), "patch must keep prior ask");
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test — push_patch on empty slot stores the patch verbatim
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn push_patch_on_empty_slot_stores_partial_quote() {
    let svc = MarketDataBuilder::<LocalSync>::new(QuoteTtl::Infinite).build();
    let id = svc
        .register(instr("AAPL", "USD"))
        .expect("register must succeed");

    svc.push_patch(id, Quote::new().with_bid(px("99")))
        .expect("push_patch must succeed");

    let got = get_default(&svc, id).expect("patch must establish a quote");
    assert_eq!(got.mark, None);
    assert_eq!(got.bid, Some(px("99")));
    assert_eq!(got.ask, None);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test — push_patch refreshes the publish instant even without changes
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn push_patch_bumps_publish_instant() {
    let ttl = Duration::from_millis(60);
    let svc = MarketDataBuilder::<LocalSync>::new(QuoteTtl::Within(ttl)).build();
    let id = svc
        .register(instr("AAPL", "USD"))
        .expect("register must succeed");

    svc.push(id, Quote::new().with_mark(px("100")))
        .expect("push must succeed");
    std::thread::sleep(Duration::from_millis(40));

    svc.push_patch(id, Quote::new())
        .expect("push_patch must succeed");
    std::thread::sleep(Duration::from_millis(40));

    let got =
        get_default(&svc, id).expect("patch must keep the quote alive past the original deadline");
    assert_eq!(got.mark, Some(px("100")));
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test — register_with_id and duplicate detection
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn register_with_id_duplicates_are_rejected() {
    use openpit::InstrumentId;

    let svc = MarketDataBuilder::<LocalSync>::new(QuoteTtl::Infinite).build();
    let _ = svc
        .register(instr("AAPL", "USD"))
        .expect("register must succeed");

    let msft = instr("MSFT", "USD");
    let custom = svc
        .register_with_id(msft.clone(), InstrumentId::new(42))
        .expect("first register_with_id must succeed");
    assert_eq!(custom.as_u64(), 42);

    let dup_id = svc.register_with_id(instr("TSLA", "USD"), InstrumentId::new(42));
    assert!(matches!(dup_id, Err(RegistrationError::DuplicateId { .. })));

    let dup_instrument = svc.register_with_id(msft, InstrumentId::new(43));
    assert!(matches!(
        dup_instrument,
        Err(RegistrationError::DuplicateInstrument { .. })
    ));
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test — push on unregistered id returns Err(UnknownInstrumentId)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn push_on_unregistered_id_returns_error() {
    use openpit::InstrumentId;

    let svc = MarketDataBuilder::<LocalSync>::new(QuoteTtl::Infinite).build();
    let unknown = InstrumentId::new(99);

    let err = svc
        .push(unknown, Quote::new().with_mark(px("100")))
        .expect_err("push on unknown id must return Err");
    assert_eq!(
        err,
        UnknownInstrumentId {
            instrument_id: unknown
        }
    );

    let err_patch = svc
        .push_patch(unknown, Quote::new().with_bid(px("50")))
        .expect_err("push_patch on unknown id must return Err");
    assert_eq!(
        err_patch,
        UnknownInstrumentId {
            instrument_id: unknown
        }
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test — register / register_with_ttl on duplicate instrument return error
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn register_duplicate_instrument_returns_error() {
    let svc = MarketDataBuilder::<LocalSync>::new(QuoteTtl::Infinite).build();
    let aapl = instr("AAPL", "USD");

    svc.register(aapl.clone())
        .expect("first register must succeed");

    let err = svc
        .register(aapl.clone())
        .expect_err("second register must return Err");
    assert_eq!(
        err,
        AlreadyRegistered {
            instrument: aapl.clone()
        },
        "expected AlreadyRegistered for duplicate register"
    );

    let err_ttl = svc
        .register_with_ttl(aapl.clone(), QuoteTtl::Infinite)
        .expect_err("register_with_ttl on duplicate must return Err");
    assert_eq!(
        err_ttl,
        AlreadyRegistered {
            instrument: aapl.clone()
        },
        "expected AlreadyRegistered for duplicate register_with_ttl"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test — push_by_instrument auto-registers on first sight and reuses id later
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn push_by_instrument_auto_registers_and_reuses_id() {
    let svc = MarketDataBuilder::<LocalSync>::new(QuoteTtl::Infinite).build();
    let aapl = instr("AAPL", "USD");

    // First call: instrument unknown → auto-register + push.
    let id_first = svc.push_by_instrument(&aapl, Quote::new().with_mark(px("100")));

    // resolve must now find the instrument.
    assert_eq!(
        svc.resolve(&aapl),
        Some(id_first),
        "push_by_instrument must register the instrument by name"
    );

    // get must return the pushed quote.
    let got = get_default(&svc, id_first).expect("quote must be present after push");
    assert_eq!(got.mark, Some(px("100")));

    // Second call with the same name: must reuse the same id.
    let id_second = svc.push_by_instrument(&aapl, Quote::new().with_mark(px("110")));
    assert_eq!(
        id_second, id_first,
        "push_by_instrument must reuse existing id on second call"
    );

    // Quote must reflect the second push.
    let got2 = get_default(&svc, id_first).expect("quote must be updated");
    assert_eq!(got2.mark, Some(px("110")));
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test — set_instrument_ttl changes freshness; unknown id returns error
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn set_instrument_ttl_changes_freshness_and_errors_on_unknown_id() {
    use openpit::InstrumentId;

    let long_ttl = Duration::from_secs(60);
    let svc = MarketDataBuilder::<LocalSync>::new(QuoteTtl::Within(long_ttl)).build();
    let id = svc
        .register(instr("AAPL", "USD"))
        .expect("register must succeed");

    svc.push(id, Quote::new().with_mark(px("200")))
        .expect("push must succeed");
    assert!(
        get_default(&svc, id).is_some(),
        "quote must be visible immediately after push"
    );

    // Shorten the instrument-level TTL to something very short.
    let short = Duration::from_millis(50);
    svc.set_instrument_ttl(id, QuoteTtl::Within(short))
        .expect("set_instrument_ttl on registered id must succeed");

    // Wait for the short TTL to elapse.
    std::thread::sleep(short + Duration::from_millis(100));
    assert!(
        get_default(&svc, id).is_none(),
        "quote must be hidden after the shortened TTL elapses"
    );

    // Unknown id must return an error.
    let unknown = InstrumentId::new(999);
    let err = svc
        .set_instrument_ttl(unknown, QuoteTtl::Infinite)
        .expect_err("set_instrument_ttl on unknown id must return Err");
    assert_eq!(
        err,
        UnknownInstrumentId {
            instrument_id: unknown
        }
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test 10 — SpotFundsPolicy with MarketData: market order passes with slippage
// ═══════════════════════════════════════════════════════════════════════════════

struct SfTestReport {
    instrument: Instrument,
    account_id: AccountId,
    side: Side,
    last_trade: Option<Trade>,
    leaves_quantity: Quantity,
    is_final: bool,
    lock: PreTradeLock,
}

impl HasInstrument for SfTestReport {
    fn instrument(&self) -> Result<&Instrument, RequestFieldAccessError> {
        Ok(&self.instrument)
    }
}
impl HasAccountId for SfTestReport {
    fn account_id(&self) -> Result<AccountId, RequestFieldAccessError> {
        Ok(self.account_id)
    }
}
impl HasSide for SfTestReport {
    fn side(&self) -> Result<Side, RequestFieldAccessError> {
        Ok(self.side)
    }
}
impl HasExecutionReportLastTrade for SfTestReport {
    fn last_trade(&self) -> Result<Option<Trade>, RequestFieldAccessError> {
        Ok(self.last_trade)
    }
}
impl HasLeavesQuantity for SfTestReport {
    fn leaves_quantity(&self) -> Result<Quantity, RequestFieldAccessError> {
        Ok(self.leaves_quantity)
    }
}
impl HasExecutionReportIsFinal for SfTestReport {
    fn is_final(&self) -> Result<bool, RequestFieldAccessError> {
        Ok(self.is_final)
    }
}
impl HasPreTradeLock for SfTestReport {
    fn lock(&self) -> Result<PreTradeLock, RequestFieldAccessError> {
        Ok(self.lock.clone())
    }
}

struct SfTestAdjustment {
    asset: Asset,
    balance: Option<AdjustmentAmount>,
}

impl HasBalanceAsset for SfTestAdjustment {
    fn balance_asset(&self) -> Result<&Asset, RequestFieldAccessError> {
        Ok(&self.asset)
    }
}
impl HasAccountAdjustmentBalance for SfTestAdjustment {
    fn balance(&self) -> Result<Option<AdjustmentAmount>, RequestFieldAccessError> {
        Ok(self.balance)
    }
}
impl HasAccountAdjustmentBalanceLowerBound for SfTestAdjustment {
    fn balance_lower(&self) -> Result<Option<PositionSize>, RequestFieldAccessError> {
        Ok(None)
    }
}
impl HasAccountAdjustmentBalanceUpperBound for SfTestAdjustment {
    fn balance_upper(&self) -> Result<Option<PositionSize>, RequestFieldAccessError> {
        Ok(None)
    }
}
impl HasAccountAdjustmentHeld for SfTestAdjustment {
    fn held(&self) -> Result<Option<AdjustmentAmount>, RequestFieldAccessError> {
        Ok(None)
    }
}
impl HasAccountAdjustmentHeldLowerBound for SfTestAdjustment {
    fn held_lower(&self) -> Result<Option<PositionSize>, RequestFieldAccessError> {
        Ok(None)
    }
}
impl HasAccountAdjustmentHeldUpperBound for SfTestAdjustment {
    fn held_upper(&self) -> Result<Option<PositionSize>, RequestFieldAccessError> {
        Ok(None)
    }
}
impl HasAccountAdjustmentIncoming for SfTestAdjustment {
    fn incoming(&self) -> Result<Option<AdjustmentAmount>, RequestFieldAccessError> {
        Ok(None)
    }
}
impl HasAccountAdjustmentIncomingLowerBound for SfTestAdjustment {
    fn incoming_lower(&self) -> Result<Option<PositionSize>, RequestFieldAccessError> {
        Ok(None)
    }
}
impl HasAccountAdjustmentIncomingUpperBound for SfTestAdjustment {
    fn incoming_upper(&self) -> Result<Option<PositionSize>, RequestFieldAccessError> {
        Ok(None)
    }
}

type SfEngine = FullSyncEngine<OrderOperation, SfTestReport, SfTestAdjustment>;

fn build_sf_engine_with_market_orders(
    aapl_usd: &Instrument,
    mark_price: Price,
    slippage_bps: u16,
) -> SfEngine {
    let builder = Engine::builder::<OrderOperation, SfTestReport, SfTestAdjustment>().full_sync();
    let svc = builder.market_data(QuoteTtl::Infinite).build();
    let id = svc
        .register(aapl_usd.clone())
        .expect("register must succeed");
    svc.push(id, Quote::new().with_mark(mark_price))
        .expect("push must succeed");
    let bundle = SpotFundsMarketData::new(
        Arc::clone(&svc),
        slippage_bps,
        SpotFundsPricingSource::Mark,
        std::iter::empty(),
    )
    .expect("bundle must build");
    let policy =
        SpotFundsPolicy::<FullSync, FullSync>::new(Some(bundle), builder.storage_builder());
    builder
        .pre_trade(policy)
        .build()
        .expect("engine must build")
}

fn build_sf_engine_no_market_orders() -> SfEngine {
    let builder = Engine::builder::<OrderOperation, SfTestReport, SfTestAdjustment>().full_sync();
    let policy = SpotFundsPolicy::<FullSync, FullSync>::new(
        None::<SpotFundsMarketData<FullSync>>,
        builder.storage_builder(),
    );
    builder
        .pre_trade(policy)
        .build()
        .expect("engine must build")
}

fn sf_seed_balance(engine: &SfEngine, asset_code: &str, amount: &str) {
    let adj = SfTestAdjustment {
        asset: asset(asset_code),
        balance: Some(AdjustmentAmount::Absolute(ps(amount))),
    };
    let acc = AccountId::from_u64(12345);
    engine
        .apply_account_adjustment(acc, &[adj])
        .expect("seed must succeed");
}

#[test]
fn spot_funds_policy_with_market_data_market_order_passes() {
    let aapl_usd = instr("AAPL", "USD");
    let engine = build_sf_engine_with_market_orders(&aapl_usd, px("200"), 1500);
    sf_seed_balance(&engine, "USD", "10000");

    let order = OrderOperation {
        instrument: aapl_usd,
        account_id: AccountId::from_u64(12345),
        side: Side::Buy,
        trade_amount: TradeAmount::Quantity(qty("5")),
        price: None,
    };

    let result = engine.execute_pre_trade(order);
    assert!(
        result.is_ok(),
        "market order with mark price must pass pre-trade check"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test 11 — SpotFundsPolicy without market orders rejects market order
// ═══════════════════════════════════════════════════════════════════════════════

// ═══════════════════════════════════════════════════════════════════════════════
// Test — book-top pricing source: market buy uses ask side of the book
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn spot_funds_book_top_uses_ask_for_market_buy() {
    let aapl = instr("AAPL", "USD");

    let builder = Engine::builder::<OrderOperation, SfTestReport, SfTestAdjustment>().full_sync();
    let svc = builder.market_data(QuoteTtl::Infinite).build();
    let id = svc.register(aapl.clone()).expect("register must succeed");
    // mark=100, ask=200: with source=Mark a 1-unit buy would charge 100;
    // with source=BookTop it charges 200.
    svc.push(
        id,
        Quote::new()
            .with_mark(px("100"))
            .with_bid(px("90"))
            .with_ask(px("200")),
    )
    .expect("push must succeed");
    let bundle = SpotFundsMarketData::new(
        Arc::clone(&svc),
        0,
        SpotFundsPricingSource::BookTop,
        std::iter::empty(),
    )
    .expect("bundle must build");
    let policy =
        SpotFundsPolicy::<FullSync, FullSync>::new(Some(bundle), builder.storage_builder());
    let engine = builder
        .pre_trade(policy)
        .build()
        .expect("engine must build");

    // Balance of 150 USD covers mark (100) but not ask (200).
    sf_seed_balance(&engine, "USD", "150");
    let order = OrderOperation {
        instrument: aapl,
        account_id: AccountId::from_u64(12345),
        side: Side::Buy,
        trade_amount: TradeAmount::Quantity(qty("1")),
        price: None,
    };
    let result = engine.execute_pre_trade(order);
    let Err(rejects) = result else {
        panic!("BookTop must price the buy at the ask, exceeding the 150 USD balance")
    };
    assert!(
        rejects
            .iter()
            .any(|r| r.code == RejectCode::InsufficientFunds),
        "expected InsufficientFunds, got: {:?}",
        rejects.iter().map(|r| r.code).collect::<Vec<_>>()
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test — book-top without ask rejects with MarkPriceUnavailable (no fallback)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn spot_funds_book_top_without_ask_rejects_market_buy() {
    let aapl = instr("AAPL", "USD");

    let builder = Engine::builder::<OrderOperation, SfTestReport, SfTestAdjustment>().full_sync();
    let svc = builder.market_data(QuoteTtl::Infinite).build();
    let id = svc.register(aapl.clone()).expect("register must succeed");
    // Push mark only - no ask available.
    svc.push(id, Quote::new().with_mark(px("100")))
        .expect("push must succeed");
    let bundle = SpotFundsMarketData::new(
        Arc::clone(&svc),
        0,
        SpotFundsPricingSource::BookTop,
        std::iter::empty(),
    )
    .expect("bundle must build");
    let policy =
        SpotFundsPolicy::<FullSync, FullSync>::new(Some(bundle), builder.storage_builder());
    let engine = builder
        .pre_trade(policy)
        .build()
        .expect("engine must build");

    sf_seed_balance(&engine, "USD", "10000");
    let order = OrderOperation {
        instrument: aapl,
        account_id: AccountId::from_u64(12345),
        side: Side::Buy,
        trade_amount: TradeAmount::Quantity(qty("1")),
        price: None,
    };
    let Err(rejects) = engine.execute_pre_trade(order) else {
        panic!("must reject")
    };
    assert!(
        rejects
            .iter()
            .any(|r| r.code == RejectCode::MarkPriceUnavailable),
        "BookTop without ask must reject with MarkPriceUnavailable, got: {:?}",
        rejects.iter().map(|r| r.code).collect::<Vec<_>>()
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test — per-instrument slippage override beats the global setting
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn spot_funds_per_instrument_override_only_affects_its_id() {
    let aapl = instr("AAPL", "USD");
    let msft = instr("MSFT", "USD");

    let builder = Engine::builder::<OrderOperation, SfTestReport, SfTestAdjustment>().full_sync();
    let svc = builder.market_data(QuoteTtl::Infinite).build();
    let aapl_id = svc
        .register(aapl.clone())
        .expect("register aapl must succeed");
    let msft_id = svc
        .register(msft.clone())
        .expect("register msft must succeed");
    svc.push(aapl_id, Quote::new().with_mark(px("100")))
        .expect("push aapl must succeed");
    svc.push(msft_id, Quote::new().with_mark(px("100")))
        .expect("push msft must succeed");

    // Global slippage 0; AAPL override charges 100 % slippage (10_000 bps).
    let overrides = [(
        SpotFundsOverrideTarget::Instrument(aapl_id),
        SpotFundsOverride {
            slippage_bps: Some(10_000),
        },
    )];
    let bundle =
        SpotFundsMarketData::new(Arc::clone(&svc), 0, SpotFundsPricingSource::Mark, overrides)
            .expect("bundle must build");
    let policy =
        SpotFundsPolicy::<FullSync, FullSync>::new(Some(bundle), builder.storage_builder());
    let engine = builder
        .pre_trade(policy)
        .build()
        .expect("engine must build");

    // 150 USD covers mark (100) but not mark * (1 + 100 %) = 200.
    sf_seed_balance(&engine, "USD", "150");

    // MSFT: no override -> charged at 100.
    let msft_order = OrderOperation {
        instrument: msft,
        account_id: AccountId::from_u64(12345),
        side: Side::Buy,
        trade_amount: TradeAmount::Quantity(qty("1")),
        price: None,
    };
    assert!(
        engine.execute_pre_trade(msft_order).is_ok(),
        "MSFT without override must use global 0 % slippage"
    );

    // AAPL: override -> charged at 200, exceeds balance.
    let aapl_order = OrderOperation {
        instrument: aapl,
        account_id: AccountId::from_u64(12345),
        side: Side::Buy,
        trade_amount: TradeAmount::Quantity(qty("1")),
        price: None,
    };
    let Err(rejects) = engine.execute_pre_trade(aapl_order) else {
        panic!("AAPL override must reject")
    };
    assert!(rejects
        .iter()
        .any(|r| r.code == RejectCode::InsufficientFunds));
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test — out-of-range override slippage is rejected at construction
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn spot_funds_instrument_override_out_of_range_returns_error() {
    let svc = MarketDataBuilder::<FullSync>::new(QuoteTtl::Infinite).build();
    let id = svc
        .register(instr("AAPL", "USD"))
        .expect("register must succeed");

    let result = SpotFundsMarketData::<FullSync>::new(
        Arc::clone(&svc),
        0,
        SpotFundsPricingSource::Mark,
        [(
            SpotFundsOverrideTarget::Instrument(id),
            SpotFundsOverride {
                slippage_bps: Some(10_001),
            },
        )],
    );
    let Err(err) = result else {
        panic!("over-range override must fail")
    };
    assert_eq!(
        err,
        openpit::SpotFundsConfigError::SlippageOutOfRange { bps: 10_001 }
    );
}

#[test]
fn spot_funds_policy_without_market_orders_rejects_market_order() {
    let aapl_usd = instr("AAPL", "USD");
    let engine = build_sf_engine_no_market_orders();
    sf_seed_balance(&engine, "USD", "10000");

    let order = OrderOperation {
        instrument: aapl_usd,
        account_id: AccountId::from_u64(12345),
        side: Side::Buy,
        trade_amount: TradeAmount::Quantity(qty("5")),
        price: None,
    };

    let Err(rejects) = engine.execute_pre_trade(order) else {
        panic!("market order without bundle must be rejected")
    };
    assert!(
        rejects
            .iter()
            .any(|r| r.code == RejectCode::UnsupportedOrderType),
        "expected UnsupportedOrderType reject, got: {:?}",
        rejects.iter().map(|r| r.code).collect::<Vec<_>>()
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test — push_for fans out to per-account and per-group buckets
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn push_for_fans_out_to_account_and_group_buckets() {
    let svc = MarketDataBuilder::<LocalSync>::new(QuoteTtl::Infinite).build();
    let id = svc
        .register(instr("AAPL", "USD"))
        .expect("register must succeed");

    // Default bucket carries 100; the fan-out targets account 7 (200) and
    // group 3 (300).
    svc.push(id, Quote::new().with_mark(px("100")))
        .expect("default push must succeed");
    svc.push_for(id, Quote::new().with_mark(px("200")), &[acc(7)], &[])
        .expect("account fan-out must succeed");
    svc.push_for(id, Quote::new().with_mark(px("300")), &[], &[grp(3)])
        .expect("group fan-out must succeed");

    // AccountOnly: account 7 sees its own bucket; account 8 sees nothing.
    assert_eq!(
        svc.get(
            id,
            acc(7),
            &group_source(None),
            QuoteResolution::AccountOnly
        )
        .expect("account 7 has its own quote")
        .mark,
        Some(px("200"))
    );
    assert!(
        svc.get(
            id,
            acc(8),
            &group_source(None),
            QuoteResolution::AccountOnly
        )
        .is_none(),
        "account 8 has no per-account quote"
    );

    // AccountThenGroup: account 8 in group 3 falls through to the group bucket.
    assert_eq!(
        svc.get(
            id,
            acc(8),
            &group_source(Some(grp(3))),
            QuoteResolution::AccountThenGroup,
        )
        .expect("account 8 falls through to group 3")
        .mark,
        Some(px("300"))
    );

    // AccountThenGroup with no group: account 8 sees nothing (no default fall).
    assert!(
        svc.get(
            id,
            acc(8),
            &group_source(None),
            QuoteResolution::AccountThenGroup,
        )
        .is_none(),
        "AccountThenGroup must not fall through to the default bucket"
    );

    // AccountThenGroupThenDefault: account 8 with an unrelated group 9 falls all
    // the way through to the default bucket.
    assert_eq!(
        svc.get(
            id,
            acc(8),
            &group_source(Some(grp(9))),
            QuoteResolution::AccountThenGroupThenDefault,
        )
        .expect("account 8 falls through to default")
        .mark,
        Some(px("100"))
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test — push_for targets the default bucket via DEFAULT_ACCOUNT_GROUP
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn push_for_default_group_writes_everyone_else_bucket() {
    let svc = MarketDataBuilder::<LocalSync>::new(QuoteTtl::Infinite).build();
    let id = svc
        .register(instr("AAPL", "USD"))
        .expect("register must succeed");

    svc.push_for(
        id,
        Quote::new().with_mark(px("123")),
        &[],
        &[DEFAULT_ACCOUNT_GROUP],
    )
    .expect("default-group fan-out must succeed");

    // The plain default-bucket read sees it.
    assert_eq!(
        get_default(&svc, id)
            .expect("default bucket carries the quote")
            .mark,
        Some(px("123"))
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test — push_for with both lists empty is a caller-bug error
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn push_for_with_no_targets_returns_error() {
    let svc = MarketDataBuilder::<LocalSync>::new(QuoteTtl::Infinite).build();
    let id = svc
        .register(instr("AAPL", "USD"))
        .expect("register must succeed");

    let err = svc
        .push_for(id, Quote::new().with_mark(px("100")), &[], &[])
        .expect_err("push_for with no targets must error");
    assert_eq!(err, PushForError::NoTarget);

    let err_patch = svc
        .push_for_patch(id, Quote::new().with_mark(px("100")), &[], &[])
        .expect_err("push_for_patch with no targets must error");
    assert_eq!(err_patch, PushForError::NoTarget);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test — push_for on an unregistered id returns UnknownInstrument
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn push_for_on_unregistered_id_returns_error() {
    use openpit::InstrumentId;

    let svc = MarketDataBuilder::<LocalSync>::new(QuoteTtl::Infinite).build();
    let unknown = InstrumentId::new(77);

    let err = svc
        .push_for(unknown, Quote::new().with_mark(px("100")), &[acc(1)], &[])
        .expect_err("push_for on unknown id must error");
    assert_eq!(
        err,
        PushForError::UnknownInstrument {
            instrument_id: unknown
        }
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test — push_for_patch merges into each target bucket independently
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn push_for_patch_merges_each_bucket_independently() {
    let svc = MarketDataBuilder::<LocalSync>::new(QuoteTtl::Infinite).build();
    let id = svc
        .register(instr("AAPL", "USD"))
        .expect("register must succeed");

    // Seed account 7 with a bid; seed group 3 with an ask.
    svc.push_for(id, Quote::new().with_bid(px("99")), &[acc(7)], &[])
        .expect("seed account must succeed");
    svc.push_for(id, Quote::new().with_ask(px("201")), &[], &[grp(3)])
        .expect("seed group must succeed");

    // Patch a mark into both targets; the prior per-bucket fields survive.
    svc.push_for_patch(id, Quote::new().with_mark(px("150")), &[acc(7)], &[grp(3)])
        .expect("patch fan-out must succeed");

    let account_quote = svc
        .get(
            id,
            acc(7),
            &group_source(None),
            QuoteResolution::AccountOnly,
        )
        .expect("account 7 quote present");
    assert_eq!(account_quote.mark, Some(px("150")));
    assert_eq!(account_quote.bid, Some(px("99")), "account bid preserved");
    assert_eq!(account_quote.ask, None, "account never had an ask");

    let group_quote = svc
        .get(
            id,
            acc(8),
            &group_source(Some(grp(3))),
            QuoteResolution::AccountThenGroup,
        )
        .expect("group 3 quote present");
    assert_eq!(group_quote.mark, Some(px("150")));
    assert_eq!(group_quote.ask, Some(px("201")), "group ask preserved");
    assert_eq!(group_quote.bid, None, "group never had a bid");
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test — get is unknown-instrument aware
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn get_or_err_on_unregistered_id_reports_unknown_instrument() {
    use openpit::InstrumentId;

    let svc = MarketDataBuilder::<LocalSync>::new(QuoteTtl::Infinite).build();
    let unknown = InstrumentId::new(404);

    let err = svc
        .get_or_err(
            unknown,
            acc(1),
            &group_source(None),
            QuoteResolution::AccountThenGroupThenDefault,
        )
        .expect_err("unknown id must error");
    assert_eq!(err, MarketDataError::UnknownInstrument);
}

// ═══════════════════════════════════════════════════════════════════════════════
// TTL cascade — tier 1 (instrument × account) beats every lower tier
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn ttl_cascade_instrument_account_is_highest_priority() {
    // Long service default and long instrument-level TTL; the instrument ×
    // account cell pins a tiny TTL that must win for account 7 only.
    let svc =
        MarketDataBuilder::<LocalSync>::new(QuoteTtl::Within(Duration::from_secs(60))).build();
    let id = svc
        .register_with_ttl(
            instr("AAPL", "USD"),
            QuoteTtl::Within(Duration::from_secs(60)),
        )
        .expect("register must succeed");

    // Lower tiers all set to long values to prove tier 1 dominates them.
    svc.set_account_ttl(acc(7), QuoteTtl::Within(Duration::from_secs(60)));
    svc.set_account_group_ttl(grp(3), QuoteTtl::Within(Duration::from_secs(60)));
    svc.set_instrument_account_group_ttl(id, grp(3), QuoteTtl::Within(Duration::from_secs(60)))
        .expect("instrument-group ttl must set");
    // Tier 1: tiny.
    svc.set_instrument_account_ttl(id, acc(7), QuoteTtl::Within(Duration::from_millis(40)))
        .expect("instrument-account ttl must set");

    svc.push_for(id, Quote::new().with_mark(px("100")), &[acc(7)], &[])
        .expect("push must succeed");

    std::thread::sleep(Duration::from_millis(90));
    // Account 7: tier-1 tiny TTL expired the quote.
    assert!(
        svc.get(
            id,
            acc(7),
            &group_source(Some(grp(3))),
            QuoteResolution::AccountThenGroupThenDefault,
        )
        .is_none(),
        "instrument-account TTL (tier 1) must expire the account-7 quote"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// TTL cascade — account/group axes beat the instrument-only axis (tier 7)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn ttl_cascade_account_beats_instrument_only() {
    // Instrument-level (tier 7) is INFINITE; the service-level account TTL
    // (tier 4) is tiny. Priority A means tier 4 wins over tier 7.
    let svc = MarketDataBuilder::<LocalSync>::new(QuoteTtl::Infinite).build();
    let id = svc
        .register_with_ttl(instr("AAPL", "USD"), QuoteTtl::Infinite)
        .expect("register must succeed");
    svc.set_account_ttl(acc(7), QuoteTtl::Within(Duration::from_millis(40)));

    svc.push_for(id, Quote::new().with_mark(px("100")), &[acc(7)], &[])
        .expect("push must succeed");

    std::thread::sleep(Duration::from_millis(90));
    assert!(
        svc.get(
            id,
            acc(7),
            &group_source(None),
            QuoteResolution::AccountOnly
        )
        .is_none(),
        "service account TTL (tier 4) must beat the infinite instrument-only TTL (tier 7)"
    );
}

#[test]
fn ttl_cascade_group_beats_instrument_only() {
    // Instrument-level (tier 7) INFINITE; service-level group TTL (tier 5) tiny.
    let svc = MarketDataBuilder::<LocalSync>::new(QuoteTtl::Infinite).build();
    let id = svc
        .register_with_ttl(instr("AAPL", "USD"), QuoteTtl::Infinite)
        .expect("register must succeed");
    svc.set_account_group_ttl(grp(3), QuoteTtl::Within(Duration::from_millis(40)));

    // Quote lives in the group bucket; the reader is in group 3.
    svc.push_for(id, Quote::new().with_mark(px("100")), &[], &[grp(3)])
        .expect("push must succeed");

    std::thread::sleep(Duration::from_millis(90));
    assert!(
        svc.get(
            id,
            acc(8),
            &group_source(Some(grp(3))),
            QuoteResolution::AccountThenGroup,
        )
        .is_none(),
        "service group TTL (tier 5) must beat the infinite instrument-only TTL (tier 7)"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// TTL cascade — INFINITE at a higher tier stops the cascade ("never expires")
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn ttl_cascade_set_infinite_stops_cascade() {
    // Tiny service default and tiny instrument-only TTL, but the instrument ×
    // account cell is explicitly INFINITE: the quote must stay visible.
    let svc =
        MarketDataBuilder::<LocalSync>::new(QuoteTtl::Within(Duration::from_millis(40))).build();
    let id = svc
        .register_with_ttl(
            instr("AAPL", "USD"),
            QuoteTtl::Within(Duration::from_millis(40)),
        )
        .expect("register must succeed");
    svc.set_instrument_account_ttl(id, acc(7), QuoteTtl::Infinite)
        .expect("instrument-account ttl must set");

    svc.push_for(id, Quote::new().with_mark(px("100")), &[acc(7)], &[])
        .expect("push must succeed");

    std::thread::sleep(Duration::from_millis(90));
    assert_eq!(
        svc.get(
            id,
            acc(7),
            &group_source(None),
            QuoteResolution::AccountOnly
        )
        .expect("explicit INFINITE must keep the quote visible")
        .mark,
        Some(px("100"))
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// TTL cascade — falls through to the global default (tier 8) when nothing is set
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn ttl_cascade_falls_through_to_global_default() {
    let svc =
        MarketDataBuilder::<LocalSync>::new(QuoteTtl::Within(Duration::from_millis(40))).build();
    let id = svc
        .register(instr("AAPL", "USD"))
        .expect("register must succeed");

    svc.push(id, Quote::new().with_mark(px("100")))
        .expect("push must succeed");
    assert!(get_default(&svc, id).is_some(), "fresh quote visible");

    std::thread::sleep(Duration::from_millis(90));
    assert!(
        get_default(&svc, id).is_none(),
        "global default TTL (tier 8) must expire the quote when no axis is set"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// TTL cascade — TTL is resolved by the requested (account, group), not the
// bucket the quote came from
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn ttl_cascade_uses_requested_axes_not_found_bucket() {
    // Quote lives only in the DEFAULT bucket, but the read is for account 7;
    // the instrument × account TTL for account 7 must govern freshness even
    // though the quote was found in the default bucket.
    let svc = MarketDataBuilder::<LocalSync>::new(QuoteTtl::Infinite).build();
    let id = svc
        .register(instr("AAPL", "USD"))
        .expect("register must succeed");
    svc.set_instrument_account_ttl(id, acc(7), QuoteTtl::Within(Duration::from_millis(40)))
        .expect("instrument-account ttl must set");

    // Default bucket only.
    svc.push(id, Quote::new().with_mark(px("100")))
        .expect("push must succeed");

    std::thread::sleep(Duration::from_millis(90));
    assert!(
        svc.get(
            id,
            acc(7),
            &group_source(None),
            QuoteResolution::AccountThenGroupThenDefault,
        )
        .is_none(),
        "account-7 TTL must govern a quote found in the default bucket"
    );
    // A different account with no TTL override still sees the infinite default.
    assert!(
        svc.get(
            id,
            acc(8),
            &group_source(None),
            QuoteResolution::AccountThenGroupThenDefault,
        )
        .is_some(),
        "account 8 (no override) keeps the infinite default-bucket quote"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// TTL cascade — clear_*_ttl reverts an axis back to inherit
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn ttl_cascade_clear_account_ttl_reverts_to_inherit() {
    // Service default INFINITE; pin a tiny service account TTL, then clear it.
    let svc = MarketDataBuilder::<LocalSync>::new(QuoteTtl::Infinite).build();
    let id = svc
        .register(instr("AAPL", "USD"))
        .expect("register must succeed");
    svc.set_account_ttl(acc(7), QuoteTtl::Within(Duration::from_millis(40)));
    svc.clear_account_ttl(acc(7));

    svc.push_for(id, Quote::new().with_mark(px("100")), &[acc(7)], &[])
        .expect("push must succeed");

    std::thread::sleep(Duration::from_millis(90));
    assert!(
        svc.get(
            id,
            acc(7),
            &group_source(None),
            QuoteResolution::AccountOnly
        )
        .is_some(),
        "cleared account TTL must fall through to the infinite global default"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Quote selection — a stale quote in a more-specific bucket blocks fallthrough
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn select_quote_stale_specific_bucket_blocks_fallthrough_to_default() {
    // Test - select_quote picks the FIRST NON-EMPTY bucket the resolution
    // permits, then the TTL check runs separately: a stale quote in the
    // per-account bucket is selected and fails freshness, so the read returns
    // unavailable WITHOUT falling through to a fresh default-bucket quote.
    let svc = MarketDataBuilder::<LocalSync>::new(QuoteTtl::Infinite).build();
    let id = svc
        .register(instr("AAPL", "USD"))
        .expect("register must succeed");

    // The per-account bucket for account 7 expires fast (instrument × account,
    // tier 1); the default bucket inherits the infinite service default.
    let short = Duration::from_millis(40);
    svc.set_instrument_account_ttl(id, acc(7), QuoteTtl::Within(short))
        .expect("instrument-account ttl must set");

    // Stale candidate in the per-account bucket; fresh quote in the default
    // ("everyone-else") bucket.
    svc.push_for(id, Quote::new().with_mark(px("200")), &[acc(7)], &[])
        .expect("per-account push must succeed");
    svc.push(id, Quote::new().with_mark(px("100")))
        .expect("default push must succeed");

    // Age the per-account quote past its tier-1 TTL.
    std::thread::sleep(short + Duration::from_millis(60));

    // Re-stamp the default bucket so it is unambiguously fresh at read time.
    svc.push(id, Quote::new().with_mark(px("100")))
        .expect("default re-push must succeed");

    let err = svc
        .get_or_err(
            id,
            acc(7),
            &group_source(None),
            QuoteResolution::AccountThenGroupThenDefault,
        )
        .expect_err("stale per-account quote must block fallthrough");
    assert_eq!(
        err,
        MarketDataError::QuoteUnavailable,
        "selection stops at the non-empty per-account bucket; its staleness \
         must not serve the fresh default quote"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Laziness — AccountInfo::group() is consulted at most once, and not at all
// when the per-account bucket and TTL cascade never need it
// ═══════════════════════════════════════════════════════════════════════════════

/// Counting [`AccountInfo`] spy: records how many times `group()` is invoked.
struct CountingAccountInfo {
    group: Option<AccountGroupId>,
    calls: std::cell::Cell<u32>,
}

impl CountingAccountInfo {
    fn new(group: Option<AccountGroupId>) -> Self {
        Self {
            group,
            calls: std::cell::Cell::new(0),
        }
    }

    fn calls(&self) -> u32 {
        self.calls.get()
    }
}

impl AccountInfo for CountingAccountInfo {
    fn group(&self) -> Option<AccountGroupId> {
        self.calls.set(self.calls.get() + 1);
        self.group
    }
}

#[test]
fn group_not_resolved_on_account_only_bucket_hit() {
    let svc = MarketDataBuilder::<LocalSync>::new(QuoteTtl::Infinite).build();
    let id = svc
        .register(instr("AAPL", "USD"))
        .expect("register must succeed");

    // Pin the instrument × account TTL (tier 1) so the cascade stops before any
    // group tier; combined with an AccountOnly bucket hit, the read is entirely
    // group-free.
    svc.set_instrument_account_ttl(id, acc(7), QuoteTtl::Infinite)
        .expect("instrument-account ttl must set");
    svc.push_for(id, Quote::new().with_mark(px("200")), &[acc(7)], &[])
        .expect("per-account push must succeed");

    let info = CountingAccountInfo::new(Some(grp(3)));
    let got = svc
        .get(id, acc(7), &info, QuoteResolution::AccountOnly)
        .expect("account 7 has its own quote");
    assert_eq!(got.mark, Some(px("200")));
    assert_eq!(
        info.calls(),
        0,
        "an AccountOnly per-account hit with a tier-1 TTL must never resolve \
         the group"
    );
}

#[test]
fn group_resolved_exactly_once_on_fallthrough() {
    let svc = MarketDataBuilder::<LocalSync>::new(QuoteTtl::Infinite).build();
    let id = svc
        .register(instr("AAPL", "USD"))
        .expect("register must succeed");

    // No per-account quote for account 8; the quote lives in the group bucket.
    svc.push_for(id, Quote::new().with_mark(px("300")), &[], &[grp(5)])
        .expect("group push must succeed");

    let info = CountingAccountInfo::new(Some(grp(5)));
    let got = svc
        .get(id, acc(8), &info, QuoteResolution::AccountThenGroup)
        .expect("account 8 falls through to its group bucket");
    assert_eq!(got.mark, Some(px("300")));
    // Selection resolves the group once and memoizes it; the TTL cascade reuses
    // the cached value rather than calling group() again.
    assert_eq!(
        info.calls(),
        1,
        "a fallthrough read must resolve the group exactly once across \
         selection and the TTL cascade"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// TTL cascade — instrument × group (tier 2) as the sole effective setting
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn ttl_cascade_instrument_group_is_sole_setting() {
    // Service default INFINITE; only the instrument × group cell (tier 2) is
    // set, so it alone governs expiry for a reader in that group.
    let svc = MarketDataBuilder::<LocalSync>::new(QuoteTtl::Infinite).build();
    let id = svc
        .register(instr("AAPL", "USD"))
        .expect("register must succeed");
    svc.set_instrument_account_group_ttl(id, grp(3), QuoteTtl::Within(Duration::from_millis(40)))
        .expect("instrument-group ttl must set");

    svc.push_for(id, Quote::new().with_mark(px("100")), &[], &[grp(3)])
        .expect("group push must succeed");

    std::thread::sleep(Duration::from_millis(90));
    assert!(
        svc.get(
            id,
            acc(8),
            &group_source(Some(grp(3))),
            QuoteResolution::AccountThenGroup,
        )
        .is_none(),
        "instrument-group TTL (tier 2) must expire the group-3 quote"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// TTL cascade — instrument × default-group (tier 3) as the only setting
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn ttl_cascade_instrument_default_group_is_sole_setting() {
    // Service default INFINITE; only the instrument × default-group cell
    // (tier 3) is set. A reader with no group reading the default bucket is
    // governed by it.
    let svc = MarketDataBuilder::<LocalSync>::new(QuoteTtl::Infinite).build();
    let id = svc
        .register(instr("AAPL", "USD"))
        .expect("register must succeed");
    svc.set_instrument_account_group_ttl(
        id,
        DEFAULT_ACCOUNT_GROUP,
        QuoteTtl::Within(Duration::from_millis(40)),
    )
    .expect("instrument-default-group ttl must set");

    svc.push(id, Quote::new().with_mark(px("100")))
        .expect("default push must succeed");

    std::thread::sleep(Duration::from_millis(90));
    assert!(
        get_default(&svc, id).is_none(),
        "instrument-default-group TTL (tier 3) must expire the default quote"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// TTL cascade — service-level default-group (tier 6) as the only setting
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn ttl_cascade_service_default_group_is_sole_setting() {
    // Service default INFINITE; only the service-level default-group cell
    // (tier 6) is set via set_account_group_ttl(DEFAULT_ACCOUNT_GROUP, ..). It
    // alone governs a no-group reader on the default bucket.
    let svc = MarketDataBuilder::<LocalSync>::new(QuoteTtl::Infinite).build();
    let id = svc
        .register(instr("AAPL", "USD"))
        .expect("register must succeed");
    svc.set_account_group_ttl(
        DEFAULT_ACCOUNT_GROUP,
        QuoteTtl::Within(Duration::from_millis(40)),
    );

    svc.push(id, Quote::new().with_mark(px("100")))
        .expect("default push must succeed");

    std::thread::sleep(Duration::from_millis(90));
    assert!(
        get_default(&svc, id).is_none(),
        "service default-group TTL (tier 6) must expire the default quote"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// TTL cascade — clear_account_group_ttl reverts a service-group axis to inherit
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn ttl_cascade_clear_account_group_ttl_reverts_to_inherit() {
    // Service default INFINITE; pin a tiny service group TTL, then clear it.
    let svc = MarketDataBuilder::<LocalSync>::new(QuoteTtl::Infinite).build();
    let id = svc
        .register(instr("AAPL", "USD"))
        .expect("register must succeed");
    svc.set_account_group_ttl(grp(3), QuoteTtl::Within(Duration::from_millis(40)));
    svc.clear_account_group_ttl(grp(3));

    svc.push_for(id, Quote::new().with_mark(px("100")), &[], &[grp(3)])
        .expect("group push must succeed");

    std::thread::sleep(Duration::from_millis(90));
    assert!(
        svc.get(
            id,
            acc(8),
            &group_source(Some(grp(3))),
            QuoteResolution::AccountThenGroup,
        )
        .is_some(),
        "cleared service group TTL must fall through to the infinite default"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// TTL cascade — clear_instrument_ttl reverts the instrument axis to inherit
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn ttl_cascade_clear_instrument_ttl_reverts_to_inherit() {
    // Service default INFINITE; the instrument-level TTL (tier 7) is tiny, then
    // cleared, so freshness falls through to the infinite default.
    let svc = MarketDataBuilder::<LocalSync>::new(QuoteTtl::Infinite).build();
    let id = svc
        .register_with_ttl(
            instr("AAPL", "USD"),
            QuoteTtl::Within(Duration::from_millis(40)),
        )
        .expect("register_with_ttl must succeed");
    svc.clear_instrument_ttl(id)
        .expect("clear_instrument_ttl on registered id must succeed");

    svc.push(id, Quote::new().with_mark(px("100")))
        .expect("default push must succeed");

    std::thread::sleep(Duration::from_millis(90));
    assert!(
        get_default(&svc, id).is_some(),
        "cleared instrument TTL must fall through to the infinite default"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// TTL cascade — clear_instrument_account_ttl reverts the tier-1 cell to inherit
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn ttl_cascade_clear_instrument_account_ttl_reverts_to_inherit() {
    // Service default INFINITE; pin a tiny instrument × account cell (tier 1),
    // then clear it.
    let svc = MarketDataBuilder::<LocalSync>::new(QuoteTtl::Infinite).build();
    let id = svc
        .register(instr("AAPL", "USD"))
        .expect("register must succeed");
    svc.set_instrument_account_ttl(id, acc(7), QuoteTtl::Within(Duration::from_millis(40)))
        .expect("instrument-account ttl must set");
    svc.clear_instrument_account_ttl(id, acc(7))
        .expect("clear_instrument_account_ttl must succeed");

    svc.push_for(id, Quote::new().with_mark(px("100")), &[acc(7)], &[])
        .expect("per-account push must succeed");

    std::thread::sleep(Duration::from_millis(90));
    assert!(
        svc.get(
            id,
            acc(7),
            &group_source(None),
            QuoteResolution::AccountOnly
        )
        .is_some(),
        "cleared instrument-account TTL must fall through to the infinite \
         default"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// TTL cascade — clear_instrument_account_group_ttl reverts the cell to inherit
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn ttl_cascade_clear_instrument_account_group_ttl_reverts_to_inherit() {
    // Service default INFINITE; pin a tiny instrument × group cell (tier 2),
    // then clear it.
    let svc = MarketDataBuilder::<LocalSync>::new(QuoteTtl::Infinite).build();
    let id = svc
        .register(instr("AAPL", "USD"))
        .expect("register must succeed");
    svc.set_instrument_account_group_ttl(id, grp(3), QuoteTtl::Within(Duration::from_millis(40)))
        .expect("instrument-group ttl must set");
    svc.clear_instrument_account_group_ttl(id, grp(3))
        .expect("clear_instrument_account_group_ttl must succeed");

    svc.push_for(id, Quote::new().with_mark(px("100")), &[], &[grp(3)])
        .expect("group push must succeed");

    std::thread::sleep(Duration::from_millis(90));
    assert!(
        svc.get(
            id,
            acc(8),
            &group_source(Some(grp(3))),
            QuoteResolution::AccountThenGroup,
        )
        .is_some(),
        "cleared instrument-group TTL must fall through to the infinite default"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// TTL cascade — the expiry boundary is inclusive (elapsed >= ttl)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn ttl_boundary_is_inclusive() {
    // The freshness check is `elapsed >= ttl`, so a quote whose age has reached
    // its TTL is already unavailable. Bracket the boundary: just before the
    // deadline the quote is visible; once the elapsed time reaches the TTL
    // (plus a small scheduling margin) it is gone. A margin is needed because
    // wall-clock sleeps cannot land on the boundary exactly.
    let ttl = Duration::from_millis(200);
    let svc = MarketDataBuilder::<LocalSync>::new(QuoteTtl::Within(ttl)).build();
    let id = svc
        .register(instr("AAPL", "USD"))
        .expect("register must succeed");
    svc.push(id, Quote::new().with_mark(px("100")))
        .expect("push must succeed");

    // Comfortably before the deadline: still fresh.
    std::thread::sleep(ttl - Duration::from_millis(80));
    assert!(
        get_default(&svc, id).is_some(),
        "quote must remain visible strictly before its TTL elapses"
    );

    // At/just past the inclusive deadline (the remaining 80 ms to the TTL plus
    // a 40 ms scheduling margin): gone.
    std::thread::sleep(Duration::from_millis(80) + Duration::from_millis(40));
    assert!(
        get_default(&svc, id).is_none(),
        "quote must be hidden once elapsed time reaches the TTL (inclusive >=)"
    );
}
