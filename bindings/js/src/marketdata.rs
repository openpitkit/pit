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

//! Live market-data service bindings.
//!
//! Covers instrument registration, the TTL cascade (account / group /
//! instrument and their crossings), the push family (default-bucket,
//! per-instrument, and targeted fan-out), and the `get`/`getOrErr` reads.
//! Decimals cross as `Price` value objects; durations cross as JS `number`
//! milliseconds.
//!
//! The service is built with the binding-layer [`EngineLocking`] in no-sync
//! mode. On `wasm32` the runtime is single-threaded, so the builder does not
//! expose a sync-mode choice.
//!
//! `accountInfo` (the third argument to `get`/`getOrErr`) is any JS object
//! exposing an `accountGroup` getter that returns an `AccountGroupId` or
//! `null`/`undefined`. A probe keeps the lookup lazy while ensuring that an
//! arbitrary JS getter runs only after the core releases its internal guards.
//! The engine context types (stage 3) satisfy this shape.

use std::{cell::Cell, time::Duration};

use js_sys::{Object, Reflect};
use openpit::marketdata::{
    AlreadyRegistered, MarketDataError, PushForError, RegistrationError, UnknownInstrumentId,
};
use openpit::param::AccountGroupId;
use openpit::{
    AccountInfo, Instrument, MarketDataBuilder, MarketDataService, Quote, QuoteResolution, QuoteTtl,
};
use openpit_interop::{EngineHandle, EngineLocking, SyncMode};
use wasm_bindgen::prelude::*;

use crate::domain::{
    extract_cloned_wrapper, is_plain_object, parse_asset, resolve_account_group_id,
    resolve_account_id, resolve_instrument_id, resolve_optional_price, AccountGroupIdLike,
    AccountIdLike, InstrumentIdLike,
};
use crate::error::{make_error, make_error_with, make_quote_expired_error, ErrorKind};
use crate::param::ids::{JsAccountGroupId, JsInstrumentId};
use crate::param::value_types::JsPrice;

#[wasm_bindgen(typescript_custom_section)]
const MARKET_DATA_INIT_TS: &'static str = r#"
/**
 * Plain-object form of {@link Instrument}. Both assets are required.
 */
export interface InstrumentInit {
  underlyingAsset: string;
  settlementAsset: string;
}

/**
 * Options accepted by the {@link Quote} constructor. Each price field accepts a
 * `Price` wrapper or a `DecimalInput`; an omitted field is absent.
 */
export interface QuoteInit {
  mark?: Price | string | number | bigint;
  bid?: Price | string | number | bigint;
  ask?: Price | string | number | bigint;
}

/**
 * The read-side account context for a market-data lookup: anything exposing an
 * `accountGroup` getter. The engine's policy contexts satisfy this shape.
 */
export interface AccountInfo {
  readonly accountGroup?: AccountGroupId | null;
}
"#;

#[wasm_bindgen]
extern "C" {
    /// An `Instrument` wrapper or its plain-object form.
    #[wasm_bindgen(typescript_type = "Instrument | InstrumentInit")]
    pub type InstrumentLike;

    /// Options for the `Quote` constructor: a `QuoteInit` literal or nothing.
    #[wasm_bindgen(typescript_type = "QuoteInit | null | undefined")]
    pub type QuoteInitLike;

    /// A `Quote` wrapper or a plain `QuoteInit` literal.
    #[wasm_bindgen(typescript_type = "Quote | QuoteInit")]
    pub type QuoteLike;

    /// A `QuoteResolution` wrapper or its wire string.
    #[wasm_bindgen(
        typescript_type = "QuoteResolution | \"ACCOUNT_ONLY\" | \"ACCOUNT_THEN_GROUP\" | \"ACCOUNT_THEN_GROUP_THEN_DEFAULT\""
    )]
    pub type QuoteResolutionLike;

    /// Anything exposing an `accountGroup` getter (the engine contexts qualify);
    /// `null`/`undefined` means no group.
    #[wasm_bindgen(typescript_type = "AccountInfo | null | undefined")]
    pub type AccountInfoLike;

    /// An iterable of account ids (`AccountId` or numeric/string).
    #[wasm_bindgen(typescript_type = "Iterable<AccountId | number | bigint | string>")]
    pub type AccountIdIterable;

    /// An iterable of account-group ids (`AccountGroupId` or numeric/string).
    #[wasm_bindgen(typescript_type = "Iterable<AccountGroupId | number | bigint | string>")]
    pub type AccountGroupIdIterable;
}

/// Shared market-data service handle type (binding-layer locking).
type Service = EngineHandle<MarketDataService<EngineLocking>>;

// ─── Errors ────────────────────────────────────────────────────────────────

/// Builds the tagged JS error for an `AlreadyRegistered` registration failure.
fn already_registered_to_js(error: &AlreadyRegistered) -> JsValue {
    let payload = Object::new();
    let _ = Reflect::set(
        &payload,
        &JsValue::from_str("instrument"),
        &JsInstrument::from_inner(error.instrument.clone()).into(),
    );
    make_error_with(
        ErrorKind::AlreadyRegistered,
        &error.to_string(),
        None,
        payload.into(),
        JsValue::UNDEFINED,
    )
}

/// Builds the tagged JS error for an explicit-id registration conflict.
fn registration_error_to_js(
    error: &RegistrationError,
    instrument_id: openpit::InstrumentId,
    instrument: &Instrument,
) -> JsValue {
    let payload = Object::new();
    match error {
        RegistrationError::DuplicateId { .. } => {
            let _ = Reflect::set(
                &payload,
                &JsValue::from_str("kind"),
                &JsValue::from_str("DuplicateId"),
            );
            let _ = Reflect::set(
                &payload,
                &JsValue::from_str("instrumentId"),
                &JsInstrumentId::from_inner(instrument_id).into(),
            );
        }
        RegistrationError::DuplicateInstrument { .. } => {
            let _ = Reflect::set(
                &payload,
                &JsValue::from_str("kind"),
                &JsValue::from_str("DuplicateInstrument"),
            );
            let _ = Reflect::set(
                &payload,
                &JsValue::from_str("instrument"),
                &JsInstrument::from_inner(instrument.clone()).into(),
            );
        }
        _ => {}
    }
    make_error_with(
        ErrorKind::Registration,
        &error.to_string(),
        None,
        payload.into(),
        JsValue::UNDEFINED,
    )
}

/// Builds the tagged JS error for an unknown instrument id.
fn unknown_instrument_id_to_js(error: UnknownInstrumentId) -> JsValue {
    let payload = Object::new();
    let _ = Reflect::set(
        &payload,
        &JsValue::from_str("instrumentId"),
        &JsInstrumentId::from_inner(error.instrument_id).into(),
    );
    make_error_with(
        ErrorKind::UnknownInstrumentId,
        &error.to_string(),
        None,
        payload.into(),
        JsValue::UNDEFINED,
    )
}

/// Maps a market-data read failure onto its tagged JS error.
fn market_data_error_to_js(error: MarketDataError) -> JsValue {
    let message = error.to_string();
    match error {
        MarketDataError::UnknownInstrument => {
            make_error(ErrorKind::UnknownInstrument, &message, None)
        }
        MarketDataError::QuoteUnavailable => {
            make_error(ErrorKind::QuoteUnavailable, &message, None)
        }
        MarketDataError::QuoteExpired(quote) => {
            make_quote_expired_error(&message, JsQuote::from_inner(quote).into())
        }
        // `MarketDataError` is `#[non_exhaustive]`: genuinely future variants
        // degrade to the base error rather than panicking on the boundary.
        _ => make_error(ErrorKind::MarketData, &message, None),
    }
}

/// Maps a targeted fan-out push failure onto its tagged JS error.
fn push_for_error_to_js(error: PushForError) -> JsValue {
    match error {
        PushForError::UnknownInstrument { instrument_id } => {
            unknown_instrument_id_to_js(UnknownInstrumentId { instrument_id })
        }
        PushForError::NoTarget => make_error(ErrorKind::Range, &error.to_string(), None),
        // `PushForError` is `#[non_exhaustive]`: future variants degrade to the
        // base error rather than panicking on the binding boundary.
        _ => make_error(ErrorKind::MarketData, &error.to_string(), None),
    }
}

// ─── Instrument ────────────────────────────────────────────────────────────

/// Instrument definition: an underlying (traded) asset settled in another.
///
/// This is the canonical `Instrument` value object consumed by the market-data
/// registration and resolution methods. Later stages reuse it via
/// [`JsInstrument::inner`].
#[wasm_bindgen(js_name = Instrument)]
#[derive(Clone)]
pub struct JsInstrument {
    inner: Instrument,
}

#[wasm_bindgen(js_class = Instrument)]
impl JsInstrument {
    /// Constructs an instrument from its underlying and settlement assets.
    ///
    /// `underlyingAsset` is the asset that is bought or sold;
    /// `settlementAsset` is the asset used for cash flow, fees, and P&L.
    ///
    /// # Errors
    ///
    /// Throws `AssetError` when either asset is empty or whitespace-only.
    #[wasm_bindgen(constructor)]
    pub fn new(underlying_asset: &str, settlement_asset: &str) -> Result<JsInstrument, JsValue> {
        let underlying = parse_asset(underlying_asset)?;
        let settlement = parse_asset(settlement_asset)?;
        Ok(Self {
            inner: Instrument::new(underlying, settlement),
        })
    }

    /// Returns the underlying (traded) asset string.
    #[wasm_bindgen(getter, js_name = underlyingAsset)]
    pub fn underlying_asset(&self) -> String {
        self.inner.underlying_asset().to_string()
    }

    /// Returns the settlement asset string.
    #[wasm_bindgen(getter, js_name = settlementAsset)]
    pub fn settlement_asset(&self) -> String {
        self.inner.settlement_asset().to_string()
    }

    /// Returns `true` when both instruments have the same asset pair.
    #[wasm_bindgen(js_name = equals)]
    pub fn equals(&self, other: &JsInstrument) -> bool {
        self.inner == other.inner
    }

    /// Returns a human-readable `Instrument(underlying/settlement)` string.
    #[wasm_bindgen(js_name = toString)]
    pub fn to_js_string(&self) -> String {
        format!(
            "Instrument({}/{})",
            self.inner.underlying_asset(),
            self.inner.settlement_asset()
        )
    }

    /// Returns a fresh `Instrument` holding the same asset pair.
    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsInstrument {
        self.clone()
    }
}

impl JsInstrument {
    /// Wraps a core [`Instrument`].
    pub fn from_inner(inner: Instrument) -> Self {
        Self { inner }
    }

    /// Returns a clone of the wrapped core [`Instrument`].
    pub fn inner(&self) -> Instrument {
        self.inner.clone()
    }

    /// Resolves an `Instrument | InstrumentInit` argument into a core
    /// [`Instrument`].
    ///
    /// A wrapper instance is taken as-is; a plain object literal
    /// `{ underlyingAsset, settlementAsset }` is assembled.
    ///
    /// # Errors
    ///
    /// Throws `AssetError` on an empty asset, or `TypeError` when the value is
    /// neither an `Instrument` nor a plain object with both assets.
    fn coerce(value: JsValue) -> Result<Instrument, JsValue> {
        if let Some(wrapped) = extract_cloned_wrapper::<JsInstrument>(&value)? {
            return Ok(wrapped.inner());
        }
        if is_plain_object(&value) {
            let underlying = read_required_asset(&value, "underlyingAsset")?;
            let settlement = read_required_asset(&value, "settlementAsset")?;
            return Ok(Instrument::new(underlying, settlement));
        }
        Err(make_error(
            ErrorKind::Type,
            "instrument must be an Instrument or { underlyingAsset, settlementAsset }",
            None,
        ))
    }
}

/// Reads a required asset-string field off a plain object literal.
///
/// # Errors
///
/// Throws `AssetError` when the field is missing/empty or not a string.
fn read_required_asset(value: &JsValue, field: &str) -> Result<openpit::param::Asset, JsValue> {
    let field_value = Reflect::get(value, &JsValue::from_str(field))?;
    match field_value.as_string() {
        Some(text) => parse_asset(&text),
        None => Err(make_error(
            ErrorKind::Type,
            &format!("instrument.{field} is required and must be a string"),
            None,
        )),
    }
}

// ─── QuoteTtl ──────────────────────────────────────────────────────────────

/// Quote lifetime policy: infinite, or finite within a duration.
#[wasm_bindgen(js_name = QuoteTtl)]
#[derive(Clone, Copy)]
pub struct JsQuoteTtl {
    inner: QuoteTtl,
}

#[wasm_bindgen(js_class = QuoteTtl)]
impl JsQuoteTtl {
    /// Returns a TTL under which quotes never expire on their own.
    #[wasm_bindgen(js_name = infinite)]
    pub fn infinite() -> JsQuoteTtl {
        Self {
            inner: QuoteTtl::Infinite,
        }
    }

    /// Returns a TTL under which quotes expire `durationMs` after each push.
    ///
    /// `durationMs` is a finite, non-negative duration in milliseconds.
    /// Fractional milliseconds retain nanosecond precision.
    ///
    /// # Errors
    ///
    /// Throws `RangeError` when `durationMs` is negative, non-finite, or too
    /// large to represent as a Rust duration.
    #[wasm_bindgen(js_name = within)]
    pub fn within(duration_ms: f64) -> Result<JsQuoteTtl, JsValue> {
        let duration = Duration::try_from_secs_f64(duration_ms / 1000.0).map_err(|_| {
            make_error(
                ErrorKind::Range,
                "durationMs must be finite, non-negative, and representable as a duration",
                None,
            )
        })?;
        Ok(Self {
            inner: QuoteTtl::Within(duration),
        })
    }

    /// Returns `true` when the quote lifetime is infinite.
    #[wasm_bindgen(getter, js_name = isInfinite)]
    pub fn is_infinite(&self) -> bool {
        matches!(self.inner, QuoteTtl::Infinite)
    }

    /// Returns the finite lifetime in milliseconds, or `undefined` when
    /// infinite.
    #[wasm_bindgen(getter, js_name = durationMs)]
    pub fn duration_ms(&self) -> Option<f64> {
        self.inner
            .as_duration()
            .map(|duration| duration.as_secs_f64() * 1000.0)
    }
}

impl JsQuoteTtl {
    /// Returns the wrapped core [`QuoteTtl`].
    pub fn inner(&self) -> QuoteTtl {
        self.inner
    }
}

// ─── Quote ─────────────────────────────────────────────────────────────────

/// Market snapshot with optional mark, bid, and ask prices.
#[wasm_bindgen(js_name = Quote)]
#[derive(Clone, Copy)]
pub struct JsQuote {
    inner: Quote,
}

#[wasm_bindgen(js_class = Quote)]
impl JsQuote {
    /// Constructs a quote from an options object `{ mark?, bid?, ask? }`.
    ///
    /// Each field accepts a `Price` value object or a `DecimalInput`
    /// (`string | number | bigint`); an omitted/`null`/`undefined` field is
    /// absent. Passing `undefined` yields an empty quote.
    ///
    /// # Errors
    ///
    /// Throws the price boundary error on an invalid price, or `TypeError` on a
    /// non-object argument.
    #[wasm_bindgen(constructor)]
    pub fn new(options: QuoteInitLike) -> Result<JsQuote, JsValue> {
        let options: JsValue = options.into();
        let mut quote = Quote::new();
        if options.is_undefined() || options.is_null() {
            return Ok(Self { inner: quote });
        }
        let mark = read_optional_field(&options, "mark")?;
        let bid = read_optional_field(&options, "bid")?;
        let ask = read_optional_field(&options, "ask")?;
        if let Some(mark) = resolve_optional_price(mark)? {
            quote = quote.with_mark(mark);
        }
        if let Some(bid) = resolve_optional_price(bid)? {
            quote = quote.with_bid(bid);
        }
        if let Some(ask) = resolve_optional_price(ask)? {
            quote = quote.with_ask(ask);
        }
        Ok(Self { inner: quote })
    }

    /// Returns the mark price, or `undefined` when absent.
    #[wasm_bindgen(getter, js_name = mark)]
    pub fn mark(&self) -> Option<JsPrice> {
        self.inner.mark.map(JsPrice::from_inner)
    }

    /// Returns the bid price, or `undefined` when absent.
    #[wasm_bindgen(getter, js_name = bid)]
    pub fn bid(&self) -> Option<JsPrice> {
        self.inner.bid.map(JsPrice::from_inner)
    }

    /// Returns the ask price, or `undefined` when absent.
    #[wasm_bindgen(getter, js_name = ask)]
    pub fn ask(&self) -> Option<JsPrice> {
        self.inner.ask.map(JsPrice::from_inner)
    }

    /// Returns `true` when both quotes carry the same prices.
    #[wasm_bindgen(js_name = equals)]
    pub fn equals(&self, other: &JsQuote) -> bool {
        self.inner == other.inner
    }

    /// Returns a fresh `Quote` holding the same prices.
    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsQuote {
        *self
    }
}

impl JsQuote {
    /// Wraps a core [`Quote`].
    pub fn from_inner(inner: Quote) -> Self {
        Self { inner }
    }

    /// Returns the wrapped core [`Quote`].
    pub fn inner(&self) -> Quote {
        self.inner
    }

    /// Resolves a `Quote | QuoteInit` argument into a core [`Quote`].
    ///
    /// A wrapper instance is taken as-is; a plain object literal
    /// `{ mark?, bid?, ask? }` is assembled through the constructor logic.
    ///
    /// # Errors
    ///
    /// Throws the price boundary error on an invalid field, or `TypeError` when
    /// the value is neither a `Quote` nor a plain object.
    fn coerce(value: JsValue) -> Result<Quote, JsValue> {
        if let Some(wrapped) = extract_cloned_wrapper::<JsQuote>(&value)? {
            return Ok(wrapped.inner());
        }
        if is_plain_object(&value) {
            return Ok(JsQuote::new(value.unchecked_into())?.inner());
        }
        Err(make_error(
            ErrorKind::Type,
            "quote must be a Quote or { mark?, bid?, ask? }",
            None,
        ))
    }
}

/// Reads `field` off a JS options object, returning `undefined` when absent.
///
/// A `Reflect::get` failure (non-object argument) is a `ParamError`.
fn read_optional_field(options: &JsValue, field: &str) -> Result<JsValue, JsValue> {
    if !options.is_object() {
        return Err(make_error(
            ErrorKind::Type,
            "quote options must be an object with optional mark/bid/ask",
            None,
        ));
    }
    Reflect::get(options, &JsValue::from_str(field))
}

// ─── QuoteResolution ───────────────────────────────────────────────────────

/// Parses a `QuoteResolution` wire string into the core enum.
///
/// Accepts the canonical names `ACCOUNT_ONLY`, `ACCOUNT_THEN_GROUP`, and
/// `ACCOUNT_THEN_GROUP_THEN_DEFAULT` (case-insensitive).
fn parse_quote_resolution(value: &str) -> Result<QuoteResolution, JsValue> {
    match value.to_ascii_uppercase().as_str() {
        "ACCOUNT_ONLY" => Ok(QuoteResolution::AccountOnly),
        "ACCOUNT_THEN_GROUP" => Ok(QuoteResolution::AccountThenGroup),
        "ACCOUNT_THEN_GROUP_THEN_DEFAULT" => Ok(QuoteResolution::AccountThenGroupThenDefault),
        _ => Err(make_error(
            ErrorKind::Range,
            "resolution must be \"ACCOUNT_ONLY\", \"ACCOUNT_THEN_GROUP\", or \
             \"ACCOUNT_THEN_GROUP_THEN_DEFAULT\"",
            None,
        )),
    }
}

/// Resolution mode controlling which quote buckets a read may fall through to.
///
/// Exposes the canonical wire strings as static singletons; reads accept the
/// same strings directly.
#[wasm_bindgen(js_name = QuoteResolution)]
#[derive(Clone, Copy)]
pub struct JsQuoteResolution {
    inner: QuoteResolution,
}

#[wasm_bindgen(js_class = QuoteResolution)]
impl JsQuoteResolution {
    /// Parses a resolution from its wire string.
    ///
    /// # Errors
    ///
    /// Throws `RangeError` on an unknown value.
    #[wasm_bindgen(js_name = fromString)]
    pub fn from_string(value: &str) -> Result<JsQuoteResolution, JsValue> {
        Ok(Self {
            inner: parse_quote_resolution(value)?,
        })
    }

    /// Consults only the per-account bucket.
    #[wasm_bindgen(js_name = ACCOUNT_ONLY)]
    pub fn account_only() -> JsQuoteResolution {
        Self {
            inner: QuoteResolution::AccountOnly,
        }
    }

    /// Consults the per-account bucket, then the account's group bucket.
    #[wasm_bindgen(js_name = ACCOUNT_THEN_GROUP)]
    pub fn account_then_group() -> JsQuoteResolution {
        Self {
            inner: QuoteResolution::AccountThenGroup,
        }
    }

    /// Consults the per-account bucket, then the group bucket, then default.
    #[wasm_bindgen(js_name = ACCOUNT_THEN_GROUP_THEN_DEFAULT)]
    pub fn account_then_group_then_default() -> JsQuoteResolution {
        Self {
            inner: QuoteResolution::AccountThenGroupThenDefault,
        }
    }

    /// Returns the canonical wire string for this resolution.
    #[wasm_bindgen(js_name = toString)]
    pub fn to_js_string(&self) -> String {
        match self.inner {
            QuoteResolution::AccountOnly => "ACCOUNT_ONLY".to_owned(),
            QuoteResolution::AccountThenGroup => "ACCOUNT_THEN_GROUP".to_owned(),
            QuoteResolution::AccountThenGroupThenDefault => {
                "ACCOUNT_THEN_GROUP_THEN_DEFAULT".to_owned()
            }
        }
    }

    /// Returns a fresh resolution wrapper holding the same mode.
    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsQuoteResolution {
        *self
    }
}

impl JsQuoteResolution {
    /// Returns the wrapped core [`QuoteResolution`].
    pub fn inner(&self) -> QuoteResolution {
        self.inner
    }
}

/// Resolves a resolution argument that is either a wire string or a
/// `QuoteResolution` value object.
fn resolve_resolution(value: JsValue) -> Result<QuoteResolution, JsValue> {
    if let Some(text) = value.as_string() {
        return parse_quote_resolution(&text);
    }
    if let Some(wrapped) = extract_cloned_wrapper::<JsQuoteResolution>(&value)? {
        return Ok(wrapped.inner());
    }
    Err(make_error(
        ErrorKind::Type,
        "resolution must be a QuoteResolution or a resolution string",
        None,
    ))
}

/// Reads the `accountGroup` getter off an `accountInfo` argument.
///
/// Returns `None` when the property is missing, `null`, or `undefined`. A
/// present value that is not an `AccountGroupId` is a `TypeError`.
fn resolve_account_group(account_info: &JsValue) -> Result<Option<AccountGroupId>, JsValue> {
    if account_info.is_undefined() || account_info.is_null() {
        return Ok(None);
    }
    let group = js_sys::Reflect::get(account_info, &JsValue::from_str("accountGroup"))?;
    if group.is_undefined() || group.is_null() {
        return Ok(None);
    }
    match extract_cloned_wrapper::<JsAccountGroupId>(&group)? {
        Some(wrapped) => Ok(Some(wrapped.inner())),
        None => Err(make_error(
            ErrorKind::Type,
            "accountInfo.accountGroup must be an AccountGroupId or null",
            None,
        )),
    }
}

/// Records whether a core lookup needs the account group without invoking JS.
#[derive(Default)]
struct AccountGroupProbe {
    requested: Cell<bool>,
}

impl AccountGroupProbe {
    fn was_requested(&self) -> bool {
        self.requested.get()
    }
}

impl AccountInfo for AccountGroupProbe {
    fn group(&self) -> Option<AccountGroupId> {
        self.requested.set(true);
        None
    }
}

/// Reads a quote while keeping arbitrary JS outside core lock guards.
fn get_with_account_info(
    service: &Service,
    instrument_id: openpit::marketdata::InstrumentId,
    account_id: openpit::param::AccountId,
    account_info: &JsValue,
    resolution: QuoteResolution,
) -> Result<Result<Quote, MarketDataError>, JsValue> {
    if account_info.is_null() || account_info.is_undefined() {
        return Ok(service.get(
            instrument_id,
            account_id,
            &None::<AccountGroupId>,
            resolution,
        ));
    }

    let probe = AccountGroupProbe::default();
    let probed = service.get(instrument_id, account_id, &probe, resolution);
    if !probe.was_requested() {
        return Ok(probed);
    }

    let account_group = resolve_account_group(account_info)?;
    Ok(service.get(instrument_id, account_id, &account_group, resolution))
}

// ─── MarketDataService ─────────────────────────────────────────────────────

/// Thread-shareable live market-data service.
///
/// Reads use `get(instrumentId, accountId, accountInfo, resolution)` and
/// `getOrErr(...)`. `accountInfo` is any object exposing an `accountGroup`
/// getter returning an `AccountGroupId` or `null`; the group is read only when
/// the read needs it.
#[wasm_bindgen(js_name = MarketDataService)]
#[derive(Clone)]
pub struct JsMarketDataService {
    inner: Service,
}

#[wasm_bindgen(js_class = MarketDataService)]
impl JsMarketDataService {
    // ── Registration ───────────────────────────────────────────────────────

    /// Registers `instrument` and returns its assigned id.
    ///
    /// `instrument` accepts an `Instrument` object or a plain `InstrumentInit`
    /// literal.
    ///
    /// # Errors
    ///
    /// Throws `AlreadyRegistered` when the instrument is already registered, or
    /// `AssetError`/`ParamError` on an invalid literal.
    #[wasm_bindgen(js_name = register)]
    pub fn register(&self, instrument: InstrumentLike) -> Result<JsInstrumentId, JsValue> {
        let instrument = JsInstrument::coerce(instrument.into())?;
        self.inner
            .register(instrument)
            .map(JsInstrumentId::from_inner)
            .map_err(|error| already_registered_to_js(&error))
    }

    /// Registers `instrument` with a per-instrument TTL and returns its id.
    ///
    /// `instrument` accepts an `Instrument` object or a plain `InstrumentInit`
    /// literal.
    ///
    /// # Errors
    ///
    /// Throws `AlreadyRegistered` when the instrument is already registered, or
    /// `AssetError`/`ParamError` on an invalid literal.
    #[wasm_bindgen(js_name = registerWithTtl)]
    pub fn register_with_ttl(
        &self,
        instrument: InstrumentLike,
        ttl: &JsQuoteTtl,
    ) -> Result<JsInstrumentId, JsValue> {
        let instrument = JsInstrument::coerce(instrument.into())?;
        self.inner
            .register_with_ttl(instrument, ttl.inner())
            .map(JsInstrumentId::from_inner)
            .map_err(|error| already_registered_to_js(&error))
    }

    /// Registers `instrument` under an explicit `id` and returns it.
    ///
    /// `instrument` accepts an `Instrument` object or a plain `InstrumentInit`
    /// literal; `id` accepts an `InstrumentId` or a numeric/string identifier.
    ///
    /// # Errors
    ///
    /// Throws `RegistrationError` when the id or instrument already exists, or
    /// `AssetError`/`ParamError` on an invalid input.
    #[wasm_bindgen(js_name = registerWithId)]
    pub fn register_with_id(
        &self,
        instrument: InstrumentLike,
        id: InstrumentIdLike,
    ) -> Result<JsInstrumentId, JsValue> {
        let instrument = JsInstrument::coerce(instrument.into())?;
        let id = resolve_instrument_id(id.into())?;
        self.inner
            .register_with_id(instrument.clone(), id)
            .map(JsInstrumentId::from_inner)
            .map_err(|error| registration_error_to_js(&error, id, &instrument))
    }

    /// Registers `instrument` under an explicit `id` with a TTL.
    ///
    /// `instrument` accepts an `Instrument` object or a plain `InstrumentInit`
    /// literal; `id` accepts an `InstrumentId` or a numeric/string identifier.
    ///
    /// # Errors
    ///
    /// Throws `RegistrationError` when the id or instrument already exists, or
    /// `AssetError`/`ParamError` on an invalid input.
    #[wasm_bindgen(js_name = registerWithIdAndTtl)]
    pub fn register_with_id_and_ttl(
        &self,
        instrument: InstrumentLike,
        id: InstrumentIdLike,
        ttl: &JsQuoteTtl,
    ) -> Result<JsInstrumentId, JsValue> {
        let instrument = JsInstrument::coerce(instrument.into())?;
        let id = resolve_instrument_id(id.into())?;
        self.inner
            .register_with_id_and_ttl(instrument.clone(), id, ttl.inner())
            .map(JsInstrumentId::from_inner)
            .map_err(|error| registration_error_to_js(&error, id, &instrument))
    }

    /// Returns the id `instrument` is registered under, or `undefined`.
    ///
    /// `instrument` accepts an `Instrument` object or a plain `InstrumentInit`
    /// literal.
    ///
    /// # Errors
    ///
    /// Throws `AssetError`/`ParamError` on an invalid literal.
    #[wasm_bindgen(js_name = resolve)]
    pub fn resolve(&self, instrument: InstrumentLike) -> Result<Option<JsInstrumentId>, JsValue> {
        let instrument = JsInstrument::coerce(instrument.into())?;
        Ok(self
            .inner
            .resolve(&instrument)
            .map(JsInstrumentId::from_inner))
    }

    // ── TTL setters / clearers ─────────────────────────────────────────────

    /// Sets the TTL override for `accountId`.
    ///
    /// `accountId` accepts an `AccountId` or a numeric/string identifier.
    ///
    /// # Errors
    ///
    /// Throws `AccountIdError` on an invalid identifier.
    #[wasm_bindgen(js_name = setAccountTtl)]
    pub fn set_account_ttl(
        &self,
        account_id: AccountIdLike,
        ttl: &JsQuoteTtl,
    ) -> Result<(), JsValue> {
        self.inner
            .set_account_ttl(resolve_account_id(account_id.into())?, ttl.inner());
        Ok(())
    }

    /// Clears the TTL override for `accountId`.
    ///
    /// `accountId` accepts an `AccountId` or a numeric/string identifier.
    ///
    /// # Errors
    ///
    /// Throws `AccountIdError` on an invalid identifier.
    #[wasm_bindgen(js_name = clearAccountTtl)]
    pub fn clear_account_ttl(&self, account_id: AccountIdLike) -> Result<(), JsValue> {
        self.inner
            .clear_account_ttl(resolve_account_id(account_id.into())?);
        Ok(())
    }

    /// Sets the TTL override for `accountGroupId`.
    ///
    /// `accountGroupId` accepts an `AccountGroupId` or a numeric/string
    /// identifier. Passing `AccountGroupId.DEFAULT` targets the service-level
    /// default-group ("everyone-else") bucket.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` on an invalid identifier.
    #[wasm_bindgen(js_name = setAccountGroupTtl)]
    pub fn set_account_group_ttl(
        &self,
        account_group_id: AccountGroupIdLike,
        ttl: &JsQuoteTtl,
    ) -> Result<(), JsValue> {
        self.inner.set_account_group_ttl(
            resolve_account_group_id(account_group_id.into())?,
            ttl.inner(),
        );
        Ok(())
    }

    /// Clears the TTL override for `accountGroupId`.
    ///
    /// `accountGroupId` accepts an `AccountGroupId` or a numeric/string
    /// identifier. Passing `AccountGroupId.DEFAULT` targets the service-level
    /// default-group ("everyone-else") bucket.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` on an invalid identifier.
    #[wasm_bindgen(js_name = clearAccountGroupTtl)]
    pub fn clear_account_group_ttl(
        &self,
        account_group_id: AccountGroupIdLike,
    ) -> Result<(), JsValue> {
        self.inner
            .clear_account_group_ttl(resolve_account_group_id(account_group_id.into())?);
        Ok(())
    }

    /// Sets the per-instrument TTL override.
    ///
    /// `instrumentId` accepts an `InstrumentId` or a numeric/string identifier.
    ///
    /// # Errors
    ///
    /// Throws `UnknownInstrumentId` when `instrumentId` is not registered, or
    /// `ParamError` on an invalid identifier.
    #[wasm_bindgen(js_name = setInstrumentTtl)]
    pub fn set_instrument_ttl(
        &self,
        instrument_id: InstrumentIdLike,
        ttl: &JsQuoteTtl,
    ) -> Result<(), JsValue> {
        let instrument_id = resolve_instrument_id(instrument_id.into())?;
        self.inner
            .set_instrument_ttl(instrument_id, ttl.inner())
            .map_err(unknown_instrument_id_to_js)
    }

    /// Clears the per-instrument TTL override.
    ///
    /// `instrumentId` accepts an `InstrumentId` or a numeric/string identifier.
    ///
    /// # Errors
    ///
    /// Throws `UnknownInstrumentId` when `instrumentId` is not registered, or
    /// `ParamError` on an invalid identifier.
    #[wasm_bindgen(js_name = clearInstrumentTtl)]
    pub fn clear_instrument_ttl(&self, instrument_id: InstrumentIdLike) -> Result<(), JsValue> {
        let instrument_id = resolve_instrument_id(instrument_id.into())?;
        self.inner
            .clear_instrument_ttl(instrument_id)
            .map_err(unknown_instrument_id_to_js)
    }

    /// Sets the per-instrument, per-account TTL override.
    ///
    /// `instrumentId`/`accountId` accept the wrapper or a numeric/string id.
    ///
    /// # Errors
    ///
    /// Throws `UnknownInstrumentId` when `instrumentId` is not registered, or
    /// `ParamError`/`AccountIdError` on an invalid identifier.
    #[wasm_bindgen(js_name = setInstrumentAccountTtl)]
    pub fn set_instrument_account_ttl(
        &self,
        instrument_id: InstrumentIdLike,
        account_id: AccountIdLike,
        ttl: &JsQuoteTtl,
    ) -> Result<(), JsValue> {
        let instrument_id = resolve_instrument_id(instrument_id.into())?;
        let account_id = resolve_account_id(account_id.into())?;
        self.inner
            .set_instrument_account_ttl(instrument_id, account_id, ttl.inner())
            .map_err(unknown_instrument_id_to_js)
    }

    /// Clears the per-instrument, per-account TTL override.
    ///
    /// `instrumentId`/`accountId` accept the wrapper or a numeric/string id.
    ///
    /// # Errors
    ///
    /// Throws `UnknownInstrumentId` when `instrumentId` is not registered, or
    /// `ParamError`/`AccountIdError` on an invalid identifier.
    #[wasm_bindgen(js_name = clearInstrumentAccountTtl)]
    pub fn clear_instrument_account_ttl(
        &self,
        instrument_id: InstrumentIdLike,
        account_id: AccountIdLike,
    ) -> Result<(), JsValue> {
        let instrument_id = resolve_instrument_id(instrument_id.into())?;
        let account_id = resolve_account_id(account_id.into())?;
        self.inner
            .clear_instrument_account_ttl(instrument_id, account_id)
            .map_err(unknown_instrument_id_to_js)
    }

    /// Sets the per-instrument, per-group TTL override.
    ///
    /// `instrumentId`/`accountGroupId` accept the wrapper or a numeric/string
    /// id. Passing `AccountGroupId.DEFAULT` targets the instrument-level
    /// default-group bucket.
    ///
    /// # Errors
    ///
    /// Throws `UnknownInstrumentId` when `instrumentId` is not registered, or
    /// `ParamError` on an invalid identifier.
    #[wasm_bindgen(js_name = setInstrumentAccountGroupTtl)]
    pub fn set_instrument_account_group_ttl(
        &self,
        instrument_id: InstrumentIdLike,
        account_group_id: AccountGroupIdLike,
        ttl: &JsQuoteTtl,
    ) -> Result<(), JsValue> {
        let instrument_id = resolve_instrument_id(instrument_id.into())?;
        let account_group_id = resolve_account_group_id(account_group_id.into())?;
        self.inner
            .set_instrument_account_group_ttl(instrument_id, account_group_id, ttl.inner())
            .map_err(unknown_instrument_id_to_js)
    }

    /// Clears the per-instrument, per-group TTL override.
    ///
    /// `instrumentId`/`accountGroupId` accept the wrapper or a numeric/string
    /// id. Passing `AccountGroupId.DEFAULT` targets the instrument-level
    /// default-group bucket.
    ///
    /// # Errors
    ///
    /// Throws `UnknownInstrumentId` when `instrumentId` is not registered, or
    /// `ParamError` on an invalid identifier.
    #[wasm_bindgen(js_name = clearInstrumentAccountGroupTtl)]
    pub fn clear_instrument_account_group_ttl(
        &self,
        instrument_id: InstrumentIdLike,
        account_group_id: AccountGroupIdLike,
    ) -> Result<(), JsValue> {
        let instrument_id = resolve_instrument_id(instrument_id.into())?;
        let account_group_id = resolve_account_group_id(account_group_id.into())?;
        self.inner
            .clear_instrument_account_group_ttl(instrument_id, account_group_id)
            .map_err(unknown_instrument_id_to_js)
    }

    // ── Push / clear ───────────────────────────────────────────────────────

    /// Removes every stored quote for `instrumentId`.
    ///
    /// `instrumentId` accepts an `InstrumentId` or a numeric/string identifier.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` on an invalid identifier.
    #[wasm_bindgen(js_name = clear)]
    pub fn clear(&self, instrument_id: InstrumentIdLike) -> Result<(), JsValue> {
        self.inner
            .clear(resolve_instrument_id(instrument_id.into())?);
        Ok(())
    }

    /// Publishes `quote` to the default bucket for `instrumentId`.
    ///
    /// `instrumentId` accepts an `InstrumentId` or a numeric/string identifier;
    /// `quote` accepts a `Quote` object or a plain `QuoteInit` literal.
    ///
    /// # Errors
    ///
    /// Throws `UnknownInstrumentId` when `instrumentId` is not registered, or
    /// `ParamError` on an invalid identifier or quote literal.
    #[wasm_bindgen(js_name = push)]
    pub fn push(&self, instrument_id: InstrumentIdLike, quote: QuoteLike) -> Result<(), JsValue> {
        let instrument_id = resolve_instrument_id(instrument_id.into())?;
        let quote = JsQuote::coerce(quote.into())?;
        self.inner
            .push(instrument_id, quote)
            .map_err(unknown_instrument_id_to_js)
    }

    /// Merges a partial `quote` into the default bucket for `instrumentId`.
    ///
    /// `instrumentId` accepts an `InstrumentId` or a numeric/string identifier;
    /// `quote` accepts a `Quote` object or a plain `QuoteInit` literal.
    ///
    /// # Errors
    ///
    /// Throws `UnknownInstrumentId` when `instrumentId` is not registered, or
    /// `ParamError` on an invalid identifier or quote literal.
    #[wasm_bindgen(js_name = pushPatch)]
    pub fn push_patch(
        &self,
        instrument_id: InstrumentIdLike,
        quote: QuoteLike,
    ) -> Result<(), JsValue> {
        let instrument_id = resolve_instrument_id(instrument_id.into())?;
        let quote = JsQuote::coerce(quote.into())?;
        self.inner
            .push_patch(instrument_id, quote)
            .map_err(unknown_instrument_id_to_js)
    }

    /// Publishes `quote` to `instrument`'s default bucket, auto-registering it.
    ///
    /// `instrument` accepts an `Instrument` object or a plain `InstrumentInit`
    /// literal; `quote` accepts a `Quote` object or a plain `QuoteInit`
    /// literal. Returns the resolved (or newly assigned) instrument id.
    ///
    /// # Errors
    ///
    /// Throws `AssetError`/`ParamError` on an invalid instrument or quote.
    #[wasm_bindgen(js_name = pushByInstrument)]
    pub fn push_by_instrument(
        &self,
        instrument: InstrumentLike,
        quote: QuoteLike,
    ) -> Result<JsInstrumentId, JsValue> {
        let instrument = JsInstrument::coerce(instrument.into())?;
        let quote = JsQuote::coerce(quote.into())?;
        Ok(JsInstrumentId::from_inner(
            self.inner.push_by_instrument(&instrument, quote),
        ))
    }

    /// Merges a partial `quote` into `instrument`'s default bucket.
    ///
    /// `instrument` accepts an `Instrument` object or a plain `InstrumentInit`
    /// literal; `quote` accepts a `Quote` object or a plain `QuoteInit`
    /// literal. Returns the resolved (or newly assigned) instrument id.
    ///
    /// # Errors
    ///
    /// Throws `AssetError`/`ParamError` on an invalid instrument or quote.
    #[wasm_bindgen(js_name = pushByInstrumentPatch)]
    pub fn push_by_instrument_patch(
        &self,
        instrument: InstrumentLike,
        quote: QuoteLike,
    ) -> Result<JsInstrumentId, JsValue> {
        let instrument = JsInstrument::coerce(instrument.into())?;
        let quote = JsQuote::coerce(quote.into())?;
        Ok(JsInstrumentId::from_inner(
            self.inner.push_by_instrument_patch(&instrument, quote),
        ))
    }

    /// Publishes `quote` to the given accounts and groups.
    ///
    /// `instrumentId` accepts an `InstrumentId` or a numeric/string id; `quote`
    /// accepts a `Quote` object or a plain `QuoteInit` literal; the target
    /// iterables accept `AccountId`/`AccountGroupId` or numeric/string ids. To
    /// target the default ("everyone-else") bucket, include
    /// `AccountGroupId.DEFAULT` in `accountGroupIds`.
    ///
    /// # Errors
    ///
    /// Throws `UnknownInstrumentId` when `instrumentId` is not registered,
    /// `RangeError` when both target lists are empty, or
    /// `ParamError`/`AccountIdError` on an invalid input.
    #[wasm_bindgen(js_name = pushFor)]
    pub fn push_for(
        &self,
        instrument_id: InstrumentIdLike,
        quote: QuoteLike,
        account_ids: AccountIdIterable,
        account_group_ids: AccountGroupIdIterable,
    ) -> Result<(), JsValue> {
        let instrument_id = resolve_instrument_id(instrument_id.into())?;
        let quote = JsQuote::coerce(quote.into())?;
        let accounts = collect_account_ids(account_ids.into())?;
        let groups = collect_account_group_ids(account_group_ids.into())?;
        self.inner
            .push_for(instrument_id, quote, &accounts, &groups)
            .map_err(push_for_error_to_js)
    }

    /// Merges a partial `quote` into the given accounts and groups.
    ///
    /// `instrumentId` accepts an `InstrumentId` or a numeric/string id; `quote`
    /// accepts a `Quote` object or a plain `QuoteInit` literal; the target
    /// iterables accept `AccountId`/`AccountGroupId` or numeric/string ids. To
    /// target the default ("everyone-else") bucket, include
    /// `AccountGroupId.DEFAULT` in `accountGroupIds`.
    ///
    /// # Errors
    ///
    /// Throws `UnknownInstrumentId` when `instrumentId` is not registered,
    /// `RangeError` when both target lists are empty, or
    /// `ParamError`/`AccountIdError` on an invalid input.
    #[wasm_bindgen(js_name = pushForPatch)]
    pub fn push_for_patch(
        &self,
        instrument_id: InstrumentIdLike,
        quote: QuoteLike,
        account_ids: AccountIdIterable,
        account_group_ids: AccountGroupIdIterable,
    ) -> Result<(), JsValue> {
        let instrument_id = resolve_instrument_id(instrument_id.into())?;
        let quote = JsQuote::coerce(quote.into())?;
        let accounts = collect_account_ids(account_ids.into())?;
        let groups = collect_account_group_ids(account_group_ids.into())?;
        self.inner
            .push_for_patch(instrument_id, quote, &accounts, &groups)
            .map_err(push_for_error_to_js)
    }

    // ── Reads ──────────────────────────────────────────────────────────────

    /// Returns the latest quote for `(instrumentId, accountId)`, or
    /// `undefined`.
    ///
    /// `instrumentId`/`accountId` accept the wrapper or a numeric/string id.
    /// `accountInfo` is any object exposing an `accountGroup` getter; the group
    /// is consulted only when the read needs it. `resolution` is a
    /// `QuoteResolution` or its wire string.
    ///
    /// # Errors
    ///
    /// Throws `ParamError`/`AccountIdError` on an invalid identifier,
    /// `resolution`, or a consulted `accountInfo` shape. Unknown instruments,
    /// unavailable quotes, and expired quotes return `undefined`.
    #[wasm_bindgen(js_name = get)]
    pub fn get(
        &self,
        instrument_id: InstrumentIdLike,
        account_id: AccountIdLike,
        account_info: AccountInfoLike,
        resolution: QuoteResolutionLike,
    ) -> Result<Option<JsQuote>, JsValue> {
        let instrument_id = resolve_instrument_id(instrument_id.into())?;
        let account_id = resolve_account_id(account_id.into())?;
        let resolution = resolve_resolution(resolution.into())?;
        let account_info = account_info.into();
        let result = get_with_account_info(
            &self.inner,
            instrument_id,
            account_id,
            &account_info,
            resolution,
        )?;
        match result {
            Ok(quote) => Ok(Some(JsQuote::from_inner(quote))),
            Err(
                MarketDataError::UnknownInstrument
                | MarketDataError::QuoteUnavailable
                | MarketDataError::QuoteExpired(_),
            ) => Ok(None),
            Err(error) => Err(market_data_error_to_js(error)),
        }
    }

    /// Returns the latest quote for `(instrumentId, accountId)`, throwing when
    /// no usable quote exists.
    ///
    /// `instrumentId`/`accountId` accept the wrapper or a numeric/string id.
    /// `accountInfo` is any object exposing an `accountGroup` getter;
    /// `resolution` is a `QuoteResolution` or its wire string.
    ///
    /// # Errors
    ///
    /// Throws `UnknownInstrument` when `instrumentId` is not registered,
    /// `QuoteUnavailable` when no quote is available, `QuoteExpired` with the
    /// selected stale quote when its TTL elapsed, and `ParamError`/
    /// `AccountIdError` on an invalid identifier, `resolution`, or `accountInfo`
    /// shape.
    #[wasm_bindgen(js_name = getOrErr)]
    pub fn get_or_err(
        &self,
        instrument_id: InstrumentIdLike,
        account_id: AccountIdLike,
        account_info: AccountInfoLike,
        resolution: QuoteResolutionLike,
    ) -> Result<JsQuote, JsValue> {
        let instrument_id = resolve_instrument_id(instrument_id.into())?;
        let account_id = resolve_account_id(account_id.into())?;
        let resolution = resolve_resolution(resolution.into())?;
        let account_info = account_info.into();
        let result = get_with_account_info(
            &self.inner,
            instrument_id,
            account_id,
            &account_info,
            resolution,
        )?;
        result
            .map(JsQuote::from_inner)
            .map_err(market_data_error_to_js)
    }
}

/// Collects an iterable of account ids (wrappers or numeric/string) into a
/// vector of core ids.
///
/// # Errors
///
/// Throws `TypeError` when the argument is not iterable, or
/// `AccountIdError`/`ParamError` on an invalid element.
fn collect_account_ids(value: JsValue) -> Result<Vec<openpit::param::AccountId>, JsValue> {
    let iterator = js_sys::try_iter(&value)?.ok_or_else(|| {
        make_error(
            ErrorKind::Type,
            "accountIds must be an iterable of AccountId",
            None,
        )
    })?;
    let mut ids = Vec::new();
    for item in iterator {
        ids.push(resolve_account_id(item?)?);
    }
    Ok(ids)
}

/// Collects an iterable of account-group ids (wrappers or numeric/string) into
/// a vector of core ids.
///
/// # Errors
///
/// Throws `TypeError` when the argument is not iterable or `ParamError` on an
/// invalid element.
fn collect_account_group_ids(value: JsValue) -> Result<Vec<AccountGroupId>, JsValue> {
    let iterator = js_sys::try_iter(&value)?.ok_or_else(|| {
        make_error(
            ErrorKind::Type,
            "accountGroupIds must be an iterable of AccountGroupId",
            None,
        )
    })?;
    let mut ids = Vec::new();
    for item in iterator {
        ids.push(resolve_account_group_id(item?)?);
    }
    Ok(ids)
}

impl JsMarketDataService {
    /// Wraps a built service handle (used by the engine builder in stage 3).
    pub fn from_handle(inner: Service) -> Self {
        Self { inner }
    }

    /// Returns a clone of the wrapped service handle.
    pub fn handle(&self) -> Service {
        self.inner.clone()
    }
}

// ─── MarketDataBuilder ──────────────────────────────────────────────────────
/// Builder for a [`JsMarketDataService`].
///
/// Obtained from the engine builder via `marketData(defaultTtl)` (stage 3).
/// On `wasm32` the runtime is single-threaded, so it always builds a no-sync
/// service without exposing a sync-mode choice.
#[wasm_bindgen(js_name = MarketDataBuilder)]
#[derive(Clone, Copy)]
pub struct JsMarketDataBuilder {
    default_ttl: QuoteTtl,
}

#[wasm_bindgen(js_class = MarketDataBuilder)]
impl JsMarketDataBuilder {
    /// Builds the market-data service.
    #[wasm_bindgen(js_name = build)]
    pub fn build(&self) -> JsMarketDataService {
        let handle =
            MarketDataBuilder::with_sync(EngineLocking::new(SyncMode::None), self.default_ttl)
                .build();
        JsMarketDataService::from_handle(handle)
    }
}

impl JsMarketDataBuilder {
    /// Builds a market-data builder with the given default TTL.
    ///
    /// Used by the engine builder's `marketData(defaultTtl)` entry point in
    /// stage 3.
    pub fn with_default_ttl(default_ttl: QuoteTtl) -> Self {
        Self { default_ttl }
    }
}
