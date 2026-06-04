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

#![allow(clippy::not_unsafe_ptr_arg_deref)]

//! FFI surface for the market-data service.
//!
//! A market-data service can be shared between the engine's policies and an
//! external quote feed: handles are safe to use concurrently, so a feed can
//! push quotes while the engine reads them.

use std::ffi::c_void;
use std::time::Duration;

use openpit::marketdata::{
    AccountInfo, AlreadyRegistered, MarketDataBuilder, MarketDataService, PushForError, Quote,
    QuoteResolution, QuoteTtl, RegistrationError, UnknownInstrumentId,
};
use openpit::param::{AccountGroupId, AccountId, Price, DEFAULT_ACCOUNT_GROUP};
use openpit::InstrumentId;
use openpit_interop::{EngineHandle, EngineLocking, SyncMode};

use crate::account_group_id::OpenPitParamAccountGroupId;
use crate::instrument::{import_instrument, OpenPitInstrument};
use crate::last_error::{write_error, OpenPitOutError};
use crate::param::{OpenPitParamAccountId, OpenPitParamPrice, OpenPitParamPriceOptional};

//--------------------------------------------------------------------------------------------------
// InstrumentId

/// Stable instrument identifier for FFI payloads.
pub type OpenPitMarketDataInstrumentId = u64;

//--------------------------------------------------------------------------------------------------
// Quote

/// Market snapshot value passed across the FFI boundary.
///
/// Every field is optional (`is_set == false` means the producer did not
/// publish that field). Mirrors [`Quote`].
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct OpenPitMarketDataQuote {
    /// Mark price.
    pub mark: OpenPitParamPriceOptional,
    /// Best-bid price.
    pub bid: OpenPitParamPriceOptional,
    /// Best-ask price.
    pub ask: OpenPitParamPriceOptional,
}

fn export_price(price: Price) -> OpenPitParamPrice {
    OpenPitParamPrice(crate::param::OpenPitParamDecimal::from_decimal(
        price.to_decimal(),
    ))
}

fn export_optional_price(price: Option<Price>) -> OpenPitParamPriceOptional {
    match price {
        Some(price) => OpenPitParamPriceOptional {
            value: export_price(price),
            is_set: true,
        },
        None => OpenPitParamPriceOptional::default(),
    }
}

impl OpenPitMarketDataQuote {
    fn from_quote(quote: Quote) -> Self {
        Self {
            mark: export_optional_price(quote.mark),
            bid: export_optional_price(quote.bid),
            ask: export_optional_price(quote.ask),
        }
    }

    /// Validates each set field and assembles a core [`Quote`].
    ///
    /// `Quote` is `#[non_exhaustive]`, so it is built field-by-field via the
    /// `Quote::new().with_*` chain.
    fn to_quote(self) -> Result<Quote, String> {
        let mut quote = Quote::new();
        if self.mark.is_set {
            quote = quote.with_mark(
                self.mark
                    .value
                    .to_param()
                    .map_err(|e| format!("mark: {e}"))?,
            );
        }
        if self.bid.is_set {
            quote = quote.with_bid(self.bid.value.to_param().map_err(|e| format!("bid: {e}"))?);
        }
        if self.ask.is_set {
            quote = quote.with_ask(self.ask.value.to_param().map_err(|e| format!("ask: {e}"))?);
        }
        Ok(quote)
    }
}

/// Returns an empty quote with every field unset.
///
/// This function never fails.
#[no_mangle]
pub extern "C" fn openpit_create_marketdata_quote() -> OpenPitMarketDataQuote {
    OpenPitMarketDataQuote::default()
}

//--------------------------------------------------------------------------------------------------
// QuoteTtl

/// Service-wide / per-instrument quote lifetime for FFI payloads.
///
/// `is_infinite == true` means quotes never expire on their own. Otherwise the
/// quote expires `secs` + `nanos` after the push that wrote it. Mirrors
/// [`QuoteTtl`].
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpenPitMarketDataQuoteTtl {
    /// Whole-seconds part of the finite lifetime (ignored when infinite).
    pub secs: u64,
    /// Sub-second nanoseconds part of the finite lifetime (ignored when
    /// infinite).
    pub nanos: u32,
    /// When `true`, quotes never expire on their own.
    pub is_infinite: bool,
}

impl OpenPitMarketDataQuoteTtl {
    fn to_quote_ttl(self) -> QuoteTtl {
        if self.is_infinite {
            QuoteTtl::Infinite
        } else {
            QuoteTtl::Within(Duration::new(self.secs, self.nanos))
        }
    }
}

/// Builds an infinite quote lifetime.
///
/// This function never fails.
#[no_mangle]
pub extern "C" fn openpit_create_marketdata_quote_ttl_infinite() -> OpenPitMarketDataQuoteTtl {
    OpenPitMarketDataQuoteTtl {
        secs: 0,
        nanos: 0,
        is_infinite: true,
    }
}

/// Builds a finite quote lifetime of `secs` seconds plus `nanos` nanoseconds.
///
/// This function never fails.
#[no_mangle]
pub extern "C" fn openpit_create_marketdata_quote_ttl_within(
    secs: u64,
    nanos: u32,
) -> OpenPitMarketDataQuoteTtl {
    OpenPitMarketDataQuoteTtl {
        secs,
        nanos,
        is_infinite: false,
    }
}

//--------------------------------------------------------------------------------------------------
// QuoteResolution

/// Selects which quote buckets a read will consult, in order.
///
/// When the more-specific bucket has no quote, the read falls through
/// to the next bucket permitted by this value.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenPitMarketDataQuoteResolution {
    /// Consult only the per-account bucket for the reading account.
    AccountOnly = 0,
    /// Consult the per-account bucket, then fall back to the account's group
    /// bucket when the account bucket has no quote.
    AccountThenGroup = 1,
    /// Consult the per-account bucket, then the account's group bucket, then
    /// the default-group ("everyone-else") bucket, in that order.
    AccountThenGroupThenDefault = 2,
}

impl From<OpenPitMarketDataQuoteResolution> for QuoteResolution {
    fn from(value: OpenPitMarketDataQuoteResolution) -> Self {
        match value {
            OpenPitMarketDataQuoteResolution::AccountOnly => QuoteResolution::AccountOnly,
            OpenPitMarketDataQuoteResolution::AccountThenGroup => QuoteResolution::AccountThenGroup,
            OpenPitMarketDataQuoteResolution::AccountThenGroupThenDefault => {
                QuoteResolution::AccountThenGroupThenDefault
            }
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Get status

/// Result of a market-data read.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenPitMarketDataGetStatus {
    /// A usable quote was found; `out_quote` was written.
    Found = 0,
    /// The instrument is registered but no usable quote is available
    /// (never pushed, cleared, or aged past its TTL).
    Unavailable = 1,
    /// The instrument id is not registered with the service.
    UnknownInstrument = 2,
}

//--------------------------------------------------------------------------------------------------
// Register / update status

/// Result of a market-data registration or update.
///
/// Each operation returns only the subset of variants it can produce; see the
/// per-function contract for the variants it may report.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenPitMarketDataRegisterStatus {
    /// The operation succeeded; any associated output was written.
    Ok = 0,
    /// The instrument is already registered with the service.
    AlreadyRegistered = 1,
    /// The supplied id is already registered with the service.
    DuplicateId = 2,
    /// The supplied instrument is already registered under a different id.
    DuplicateInstrument = 3,
    /// The supplied instrument id is not registered with the service.
    UnknownInstrument = 4,
    /// A boundary failure occurred (null pointer or an invalid payload); when
    /// `out_error` is not null, a caller-owned error string was written.
    Error = 5,
    /// A targeted push (`push_for` / `push_for_patch`) was called with both the
    /// account list and the group list empty.
    NoTarget = 6,
}

//--------------------------------------------------------------------------------------------------
// Handles

/// Opaque shared market-data service handle.
///
/// Duplicate it with `openpit_marketdata_service_clone` to hand the same
/// service to both a policy and a feed.
pub struct OpenPitMarketDataService {
    pub(crate) handle: EngineHandle<MarketDataService<EngineLocking>>,
    /// The synchronization mode the service was built with.
    ///
    /// Stored here so the spot-funds policy builder can detect a mismatch
    /// between the engine mode and the market-data mode before building.
    pub(crate) mode: SyncMode,
}

impl OpenPitMarketDataService {
    pub(crate) fn from_handle(
        handle: EngineHandle<MarketDataService<EngineLocking>>,
        mode: SyncMode,
    ) -> *mut Self {
        Box::into_raw(Box::new(Self { handle, mode }))
    }

    pub(crate) fn handle_clone(&self) -> EngineHandle<MarketDataService<EngineLocking>> {
        self.handle.clone()
    }
}

//--------------------------------------------------------------------------------------------------
// Internal helpers

/// Maps a raw FFI `u32` group id to a core [`AccountGroupId`].
///
/// `0` (the reserved sentinel `OPENPIT_DEFAULT_ACCOUNT_GROUP`) is mapped to
/// `DEFAULT_ACCOUNT_GROUP`. Any non-zero value that fails the `AccountGroupId`
/// constructor (currently impossible for non-zero `u32`) also falls back to the
/// default group.
#[inline]
fn import_group(raw: OpenPitParamAccountGroupId) -> AccountGroupId {
    if raw == 0 {
        DEFAULT_ACCOUNT_GROUP
    } else {
        AccountGroupId::from_u32(raw).unwrap_or(DEFAULT_ACCOUNT_GROUP)
    }
}

//--------------------------------------------------------------------------------------------------
// Account-group resolver callback

/// Resolves the reading account's group on demand.
///
/// Returns `true` and writes the group id to `out_account_group_id` when the
/// account belongs to a group; returns `false` when it has none. Invoked lazily
/// by `openpit_marketdata_service_get` — only when the resolution mode would
/// consult the group or default-group bucket and the per-account bucket has no
/// quote.
///
/// The function pointer must not be null; see the contract on
/// `openpit_marketdata_service_get`.
pub type OpenPitMarketDataAccountGroupResolver = Option<
    extern "C" fn(
        user_data: *mut c_void,
        out_account_group_id: *mut OpenPitParamAccountGroupId,
    ) -> bool,
>;

/// Adapter that wraps a `(resolve, user_data)` callback pair and implements
/// the core [`AccountInfo`] trait so the callback is only invoked when the
/// resolution logic actually needs the group.
struct CallbackAccountInfo {
    resolve: extern "C" fn(*mut c_void, *mut OpenPitParamAccountGroupId) -> bool,
    user_data: *mut c_void,
}

impl AccountInfo for CallbackAccountInfo {
    fn group(&self) -> Option<AccountGroupId> {
        let mut raw: OpenPitParamAccountGroupId = 0;
        if (self.resolve)(self.user_data, &mut raw) {
            Some(import_group(raw))
        } else {
            None
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Service creation

/// Creates a market-data service with the chosen synchronization mode.
///
/// `mode` uses the same byte convention as `openpit_create_engine_builder`:
/// - `0` = `None` (no internal synchronization: no-op locks, zero overhead,
///   single-threaded use only);
/// - `1` = `Full` (full synchronization: real `RwLock`, safe for a concurrent
///   quote feed).
///
/// Only `None` (0) and `Full` (1) are valid for a market-data service.
/// Passing `2` (`Account`) or any other byte is an error.
///
/// Success:
/// - returns a non-null caller-owned `OpenPitMarketDataService` handle.
///
/// Error:
/// - returns null when `mode` is not `0` or `1`; if `out_error` is not null,
///   writes a caller-owned `OpenPitSharedString` error handle that MUST be
///   released with `openpit_destroy_shared_string`.
///
/// Cleanup:
/// - the returned service handle MUST be released with
///   `openpit_destroy_marketdata_service` exactly once.
#[no_mangle]
pub extern "C" fn openpit_create_marketdata_service(
    mode: u8,
    default_ttl: OpenPitMarketDataQuoteTtl,
    out_error: OpenPitOutError,
) -> *mut OpenPitMarketDataService {
    let sync_mode = match mode {
        0 => SyncMode::None,
        1 => SyncMode::Full,
        invalid => {
            write_error(
                out_error,
                &format!(
                    "openpit_create_marketdata_service: invalid mode byte {invalid}, \
                     expected 0 (None) or 1 (Full)"
                ),
            );
            return std::ptr::null_mut();
        }
    };
    let handle =
        MarketDataBuilder::with_sync(EngineLocking::new(sync_mode), default_ttl.to_quote_ttl())
            .build();
    OpenPitMarketDataService::from_handle(handle, sync_mode)
}

//--------------------------------------------------------------------------------------------------
// Service lifecycle

/// Releases a market-data service handle.
///
/// Contract:
/// - passing null is allowed;
/// - releases this handle; the underlying service stays
///   alive while other handles to it exist;
/// - after this call the pointer is invalid;
/// - this function always succeeds.
#[no_mangle]
pub extern "C" fn openpit_destroy_marketdata_service(service: *mut OpenPitMarketDataService) {
    if service.is_null() {
        return;
    }
    unsafe { drop(Box::from_raw(service)) };
}

/// Returns a new handle referring to the same market-data service.
///
/// Use this to hand the same service to a policy and a feed.
///
/// Success:
/// - returns a non-null caller-owned handle to the same service.
///
/// Error:
/// - returns null when `service` is null.
///
/// Cleanup:
/// - the returned handle MUST be released with
///   `openpit_destroy_marketdata_service` exactly once.
#[no_mangle]
pub extern "C" fn openpit_marketdata_service_clone(
    service: *const OpenPitMarketDataService,
) -> *mut OpenPitMarketDataService {
    if service.is_null() {
        return std::ptr::null_mut();
    }
    let svc = unsafe { &*service };
    OpenPitMarketDataService::from_handle(svc.handle_clone(), svc.mode)
}

//--------------------------------------------------------------------------------------------------
// Registration

fn import_required_instrument(
    instrument: &OpenPitInstrument,
    out_error: OpenPitOutError,
) -> Option<openpit::Instrument> {
    match import_instrument(instrument) {
        Ok(Some(instrument)) => Some(instrument),
        Ok(None) => {
            write_error(out_error, "instrument is not set");
            None
        }
        Err(message) => {
            write_error(out_error, message.as_str());
            None
        }
    }
}

/// Registers `instrument` with the service-wide default TTL.
///
/// Status:
/// - `Ok`: registered; the auto-assigned id was written to `out_id`;
/// - `AlreadyRegistered`: the instrument is already registered;
/// - `Error`: `service`/`out_id` is null or the instrument payload is invalid;
///   if `out_error` is not null, a caller-owned `OpenPitSharedString` error
///   handle was written that MUST be released with
///   `openpit_destroy_shared_string`.
#[no_mangle]
pub extern "C" fn openpit_marketdata_service_register(
    service: *const OpenPitMarketDataService,
    instrument: *const OpenPitInstrument,
    out_id: *mut OpenPitMarketDataInstrumentId,
    out_error: OpenPitOutError,
) -> OpenPitMarketDataRegisterStatus {
    if service.is_null() {
        write_error(out_error, "market-data service is null");
        return OpenPitMarketDataRegisterStatus::Error;
    }
    if instrument.is_null() {
        write_error(out_error, "instrument is null");
        return OpenPitMarketDataRegisterStatus::Error;
    }
    if out_id.is_null() {
        write_error(out_error, "out_id is null");
        return OpenPitMarketDataRegisterStatus::Error;
    }
    let Some(parsed) = import_required_instrument(unsafe { &*instrument }, out_error) else {
        return OpenPitMarketDataRegisterStatus::Error;
    };
    match unsafe { &*service }.handle.register(parsed) {
        Ok(id) => {
            unsafe { *out_id = id.as_u64() };
            OpenPitMarketDataRegisterStatus::Ok
        }
        Err(AlreadyRegistered { .. }) => OpenPitMarketDataRegisterStatus::AlreadyRegistered,
    }
}

/// Registers `instrument` with a per-instrument TTL override.
///
/// Behaves like `openpit_marketdata_service_register` otherwise.
#[no_mangle]
pub extern "C" fn openpit_marketdata_service_register_with_ttl(
    service: *const OpenPitMarketDataService,
    instrument: *const OpenPitInstrument,
    ttl: OpenPitMarketDataQuoteTtl,
    out_id: *mut OpenPitMarketDataInstrumentId,
    out_error: OpenPitOutError,
) -> OpenPitMarketDataRegisterStatus {
    if service.is_null() {
        write_error(out_error, "market-data service is null");
        return OpenPitMarketDataRegisterStatus::Error;
    }
    if instrument.is_null() {
        write_error(out_error, "instrument is null");
        return OpenPitMarketDataRegisterStatus::Error;
    }
    if out_id.is_null() {
        write_error(out_error, "out_id is null");
        return OpenPitMarketDataRegisterStatus::Error;
    }
    let Some(parsed) = import_required_instrument(unsafe { &*instrument }, out_error) else {
        return OpenPitMarketDataRegisterStatus::Error;
    };
    match unsafe { &*service }
        .handle
        .register_with_ttl(parsed, ttl.to_quote_ttl())
    {
        Ok(id) => {
            unsafe { *out_id = id.as_u64() };
            OpenPitMarketDataRegisterStatus::Ok
        }
        Err(AlreadyRegistered { .. }) => OpenPitMarketDataRegisterStatus::AlreadyRegistered,
    }
}

/// Registers `instrument` under the caller-supplied `instrument_id` with the
/// service-wide default TTL.
///
/// Status:
/// - `Ok`: registered; `instrument_id` was written to `out_id`;
/// - `DuplicateInstrument`: the instrument name is already registered under a
///   different id;
/// - `DuplicateId`: `instrument_id` is already registered;
/// - `Error`: `service`/`out_id` is null or the instrument payload is invalid;
///   if `out_error` is not null, a caller-owned `OpenPitSharedString` error
///   handle was written.
#[no_mangle]
pub extern "C" fn openpit_marketdata_service_register_with_id(
    service: *const OpenPitMarketDataService,
    instrument: *const OpenPitInstrument,
    instrument_id: OpenPitMarketDataInstrumentId,
    out_id: *mut OpenPitMarketDataInstrumentId,
    out_error: OpenPitOutError,
) -> OpenPitMarketDataRegisterStatus {
    if service.is_null() {
        write_error(out_error, "market-data service is null");
        return OpenPitMarketDataRegisterStatus::Error;
    }
    if instrument.is_null() {
        write_error(out_error, "instrument is null");
        return OpenPitMarketDataRegisterStatus::Error;
    }
    if out_id.is_null() {
        write_error(out_error, "out_id is null");
        return OpenPitMarketDataRegisterStatus::Error;
    }
    let Some(parsed) = import_required_instrument(unsafe { &*instrument }, out_error) else {
        return OpenPitMarketDataRegisterStatus::Error;
    };
    match unsafe { &*service }
        .handle
        .register_with_id(parsed, InstrumentId::new(instrument_id))
    {
        Ok(id) => {
            unsafe { *out_id = id.as_u64() };
            OpenPitMarketDataRegisterStatus::Ok
        }
        Err(RegistrationError::DuplicateId { .. }) => OpenPitMarketDataRegisterStatus::DuplicateId,
        Err(RegistrationError::DuplicateInstrument { .. }) => {
            OpenPitMarketDataRegisterStatus::DuplicateInstrument
        }
        // `RegistrationError` is `#[non_exhaustive]`; treat any future variant
        // as a boundary failure with a generic error string.
        Err(error) => {
            write_error(out_error, error.to_string().as_str());
            OpenPitMarketDataRegisterStatus::Error
        }
    }
}

/// Registers `instrument` under the caller-supplied `instrument_id` with a
/// per-instrument TTL override.
///
/// Behaves like `openpit_marketdata_service_register_with_id` otherwise.
#[no_mangle]
pub extern "C" fn openpit_marketdata_service_register_with_id_and_ttl(
    service: *const OpenPitMarketDataService,
    instrument: *const OpenPitInstrument,
    instrument_id: OpenPitMarketDataInstrumentId,
    ttl: OpenPitMarketDataQuoteTtl,
    out_id: *mut OpenPitMarketDataInstrumentId,
    out_error: OpenPitOutError,
) -> OpenPitMarketDataRegisterStatus {
    if service.is_null() {
        write_error(out_error, "market-data service is null");
        return OpenPitMarketDataRegisterStatus::Error;
    }
    if instrument.is_null() {
        write_error(out_error, "instrument is null");
        return OpenPitMarketDataRegisterStatus::Error;
    }
    if out_id.is_null() {
        write_error(out_error, "out_id is null");
        return OpenPitMarketDataRegisterStatus::Error;
    }
    let Some(parsed) = import_required_instrument(unsafe { &*instrument }, out_error) else {
        return OpenPitMarketDataRegisterStatus::Error;
    };
    match unsafe { &*service }.handle.register_with_id_and_ttl(
        parsed,
        InstrumentId::new(instrument_id),
        ttl.to_quote_ttl(),
    ) {
        Ok(id) => {
            unsafe { *out_id = id.as_u64() };
            OpenPitMarketDataRegisterStatus::Ok
        }
        Err(RegistrationError::DuplicateId { .. }) => OpenPitMarketDataRegisterStatus::DuplicateId,
        Err(RegistrationError::DuplicateInstrument { .. }) => {
            OpenPitMarketDataRegisterStatus::DuplicateInstrument
        }
        // `RegistrationError` is `#[non_exhaustive]`; treat any future variant
        // as a boundary failure with a generic error string.
        Err(error) => {
            write_error(out_error, error.to_string().as_str());
            OpenPitMarketDataRegisterStatus::Error
        }
    }
}

//--------------------------------------------------------------------------------------------------
// TTL update / clear — account-level

/// Pins the service-level TTL for `account_id`.
///
/// Applies to every instrument for `account_id` that does not have a more
/// specific instrument × account TTL cell.
///
/// Contract:
/// - `service` must be a valid non-null handle; passing null aborts the call;
/// - this function never fails.
#[no_mangle]
pub extern "C" fn openpit_marketdata_service_set_account_ttl(
    service: *const OpenPitMarketDataService,
    account_id: OpenPitParamAccountId,
    ttl: OpenPitMarketDataQuoteTtl,
) {
    assert!(!service.is_null(), "market-data service must be non-null");
    unsafe { &*service }
        .handle
        .set_account_ttl(AccountId::from_u64(account_id), ttl.to_quote_ttl());
}

/// Reverts the service-level TTL for `account_id` back to "inherit".
///
/// Contract:
/// - `service` must be a valid non-null handle; passing null aborts the call;
/// - this function never fails.
#[no_mangle]
pub extern "C" fn openpit_marketdata_service_clear_account_ttl(
    service: *const OpenPitMarketDataService,
    account_id: OpenPitParamAccountId,
) {
    assert!(!service.is_null(), "market-data service must be non-null");
    unsafe { &*service }
        .handle
        .clear_account_ttl(AccountId::from_u64(account_id));
}

//--------------------------------------------------------------------------------------------------
// TTL update / clear — group-level

/// Pins the service-level TTL for `account_group_id`.
///
/// Pass `OPENPIT_DEFAULT_ACCOUNT_GROUP` (`0`) to set the service-level
/// default-group TTL.
///
/// Contract:
/// - `service` must be a valid non-null handle; passing null aborts the call;
/// - this function never fails.
#[no_mangle]
pub extern "C" fn openpit_marketdata_service_set_account_group_ttl(
    service: *const OpenPitMarketDataService,
    account_group_id: OpenPitParamAccountGroupId,
    ttl: OpenPitMarketDataQuoteTtl,
) {
    assert!(!service.is_null(), "market-data service must be non-null");
    unsafe { &*service }
        .handle
        .set_account_group_ttl(import_group(account_group_id), ttl.to_quote_ttl());
}

/// Reverts the service-level TTL for `account_group_id` back to "inherit".
///
/// Pass `OPENPIT_DEFAULT_ACCOUNT_GROUP` (`0`) to clear the default-group TTL.
///
/// Contract:
/// - `service` must be a valid non-null handle; passing null aborts the call;
/// - this function never fails.
#[no_mangle]
pub extern "C" fn openpit_marketdata_service_clear_account_group_ttl(
    service: *const OpenPitMarketDataService,
    account_group_id: OpenPitParamAccountGroupId,
) {
    assert!(!service.is_null(), "market-data service must be non-null");
    unsafe { &*service }
        .handle
        .clear_account_group_ttl(import_group(account_group_id));
}

//--------------------------------------------------------------------------------------------------
// TTL update / clear — instrument-level

/// Updates the instrument-level TTL for an already-registered instrument.
///
/// This replaces the removed `openpit_marketdata_service_set_ttl`.
///
/// Status:
/// - `Ok`: updated; the new TTL takes effect on the next read;
/// - `UnknownInstrument`: `instrument_id` is not registered.
///
/// Contract:
/// - `service` must be a valid non-null handle; passing null aborts the call.
#[no_mangle]
pub extern "C" fn openpit_marketdata_service_set_instrument_ttl(
    service: *const OpenPitMarketDataService,
    instrument_id: OpenPitMarketDataInstrumentId,
    ttl: OpenPitMarketDataQuoteTtl,
) -> OpenPitMarketDataRegisterStatus {
    assert!(!service.is_null(), "market-data service must be non-null");
    match unsafe { &*service }
        .handle
        .set_instrument_ttl(InstrumentId::new(instrument_id), ttl.to_quote_ttl())
    {
        Ok(()) => OpenPitMarketDataRegisterStatus::Ok,
        Err(UnknownInstrumentId { .. }) => OpenPitMarketDataRegisterStatus::UnknownInstrument,
    }
}

/// Reverts the instrument-level TTL for `instrument_id` back to "inherit".
///
/// Status:
/// - `Ok`: cleared;
/// - `UnknownInstrument`: `instrument_id` is not registered.
///
/// Contract:
/// - `service` must be a valid non-null handle; passing null aborts the call.
#[no_mangle]
pub extern "C" fn openpit_marketdata_service_clear_instrument_ttl(
    service: *const OpenPitMarketDataService,
    instrument_id: OpenPitMarketDataInstrumentId,
) -> OpenPitMarketDataRegisterStatus {
    assert!(!service.is_null(), "market-data service must be non-null");
    match unsafe { &*service }
        .handle
        .clear_instrument_ttl(InstrumentId::new(instrument_id))
    {
        Ok(()) => OpenPitMarketDataRegisterStatus::Ok,
        Err(UnknownInstrumentId { .. }) => OpenPitMarketDataRegisterStatus::UnknownInstrument,
    }
}

//--------------------------------------------------------------------------------------------------
// TTL update / clear — instrument × account

/// Pins the instrument × account TTL cell for `(instrument_id, account_id)`.
///
/// This is the highest-priority TTL tier (overrides all group and
/// instrument-level cells for this account).
///
/// Status:
/// - `Ok`: pinned;
/// - `UnknownInstrument`: `instrument_id` is not registered.
///
/// Contract:
/// - `service` must be a valid non-null handle; passing null aborts the call.
#[no_mangle]
pub extern "C" fn openpit_marketdata_service_set_instrument_account_ttl(
    service: *const OpenPitMarketDataService,
    instrument_id: OpenPitMarketDataInstrumentId,
    account_id: OpenPitParamAccountId,
    ttl: OpenPitMarketDataQuoteTtl,
) -> OpenPitMarketDataRegisterStatus {
    assert!(!service.is_null(), "market-data service must be non-null");
    match unsafe { &*service }.handle.set_instrument_account_ttl(
        InstrumentId::new(instrument_id),
        AccountId::from_u64(account_id),
        ttl.to_quote_ttl(),
    ) {
        Ok(()) => OpenPitMarketDataRegisterStatus::Ok,
        Err(UnknownInstrumentId { .. }) => OpenPitMarketDataRegisterStatus::UnknownInstrument,
    }
}

/// Reverts the instrument × account TTL cell for `(instrument_id, account_id)`
/// back to "inherit".
///
/// Status:
/// - `Ok`: cleared;
/// - `UnknownInstrument`: `instrument_id` is not registered.
///
/// Contract:
/// - `service` must be a valid non-null handle; passing null aborts the call.
#[no_mangle]
pub extern "C" fn openpit_marketdata_service_clear_instrument_account_ttl(
    service: *const OpenPitMarketDataService,
    instrument_id: OpenPitMarketDataInstrumentId,
    account_id: OpenPitParamAccountId,
) -> OpenPitMarketDataRegisterStatus {
    assert!(!service.is_null(), "market-data service must be non-null");
    match unsafe { &*service }.handle.clear_instrument_account_ttl(
        InstrumentId::new(instrument_id),
        AccountId::from_u64(account_id),
    ) {
        Ok(()) => OpenPitMarketDataRegisterStatus::Ok,
        Err(UnknownInstrumentId { .. }) => OpenPitMarketDataRegisterStatus::UnknownInstrument,
    }
}

//--------------------------------------------------------------------------------------------------
// TTL update / clear — instrument × group

/// Pins the instrument × group TTL cell for `(instrument_id, account_group_id)`.
///
/// Pass `OPENPIT_DEFAULT_ACCOUNT_GROUP` (`0`) for `account_group_id` to target
/// the instrument's default-group TTL cell.
///
/// Status:
/// - `Ok`: pinned;
/// - `UnknownInstrument`: `instrument_id` is not registered.
///
/// Contract:
/// - `service` must be a valid non-null handle; passing null aborts the call.
#[no_mangle]
pub extern "C" fn openpit_marketdata_service_set_instrument_account_group_ttl(
    service: *const OpenPitMarketDataService,
    instrument_id: OpenPitMarketDataInstrumentId,
    account_group_id: OpenPitParamAccountGroupId,
    ttl: OpenPitMarketDataQuoteTtl,
) -> OpenPitMarketDataRegisterStatus {
    assert!(!service.is_null(), "market-data service must be non-null");
    match unsafe { &*service }
        .handle
        .set_instrument_account_group_ttl(
            InstrumentId::new(instrument_id),
            import_group(account_group_id),
            ttl.to_quote_ttl(),
        ) {
        Ok(()) => OpenPitMarketDataRegisterStatus::Ok,
        Err(UnknownInstrumentId { .. }) => OpenPitMarketDataRegisterStatus::UnknownInstrument,
    }
}

/// Reverts the instrument × group TTL cell for `(instrument_id, account_group_id)`
/// back to "inherit".
///
/// Pass `OPENPIT_DEFAULT_ACCOUNT_GROUP` (`0`) for `account_group_id` to clear
/// the instrument's default-group TTL cell.
///
/// Status:
/// - `Ok`: cleared;
/// - `UnknownInstrument`: `instrument_id` is not registered.
///
/// Contract:
/// - `service` must be a valid non-null handle; passing null aborts the call.
#[no_mangle]
pub extern "C" fn openpit_marketdata_service_clear_instrument_account_group_ttl(
    service: *const OpenPitMarketDataService,
    instrument_id: OpenPitMarketDataInstrumentId,
    account_group_id: OpenPitParamAccountGroupId,
) -> OpenPitMarketDataRegisterStatus {
    assert!(!service.is_null(), "market-data service must be non-null");
    match unsafe { &*service }
        .handle
        .clear_instrument_account_group_ttl(
            InstrumentId::new(instrument_id),
            import_group(account_group_id),
        ) {
        Ok(()) => OpenPitMarketDataRegisterStatus::Ok,
        Err(UnknownInstrumentId { .. }) => OpenPitMarketDataRegisterStatus::UnknownInstrument,
    }
}

//--------------------------------------------------------------------------------------------------
// Clear

/// Clears the stored quote for `instrument_id`.
///
/// Contract:
/// - `service` must be a valid non-null handle; passing null aborts the call;
/// - a no-op if `instrument_id` is not registered;
/// - this function never fails.
#[no_mangle]
pub extern "C" fn openpit_marketdata_service_clear(
    service: *const OpenPitMarketDataService,
    instrument_id: OpenPitMarketDataInstrumentId,
) {
    assert!(!service.is_null(), "market-data service must be non-null");
    unsafe { &*service }
        .handle
        .clear(InstrumentId::new(instrument_id));
}

//--------------------------------------------------------------------------------------------------
// Push by id — default bucket

/// Publishes a quote for `instrument_id`, replacing the entire stored snapshot.
///
/// Status:
/// - `Ok`: the snapshot was replaced;
/// - `UnknownInstrument`: `instrument_id` is not registered;
/// - `Error`: `service` is null or `quote` carries an invalid price; if
///   `out_error` is not null, a caller-owned `OpenPitSharedString` error handle
///   was written.
#[no_mangle]
pub extern "C" fn openpit_marketdata_service_push(
    service: *const OpenPitMarketDataService,
    instrument_id: OpenPitMarketDataInstrumentId,
    quote: OpenPitMarketDataQuote,
    out_error: OpenPitOutError,
) -> OpenPitMarketDataRegisterStatus {
    if service.is_null() {
        write_error(out_error, "market-data service is null");
        return OpenPitMarketDataRegisterStatus::Error;
    }
    let parsed = match quote.to_quote() {
        Ok(parsed) => parsed,
        Err(message) => {
            write_error(out_error, message.as_str());
            return OpenPitMarketDataRegisterStatus::Error;
        }
    };
    match unsafe { &*service }
        .handle
        .push(InstrumentId::new(instrument_id), parsed)
    {
        Ok(()) => OpenPitMarketDataRegisterStatus::Ok,
        Err(UnknownInstrumentId { .. }) => OpenPitMarketDataRegisterStatus::UnknownInstrument,
    }
}

/// Publishes a partial update for `instrument_id`, merging it into the stored
/// snapshot.
///
/// Behaves like `openpit_marketdata_service_push` otherwise.
#[no_mangle]
pub extern "C" fn openpit_marketdata_service_push_patch(
    service: *const OpenPitMarketDataService,
    instrument_id: OpenPitMarketDataInstrumentId,
    quote: OpenPitMarketDataQuote,
    out_error: OpenPitOutError,
) -> OpenPitMarketDataRegisterStatus {
    if service.is_null() {
        write_error(out_error, "market-data service is null");
        return OpenPitMarketDataRegisterStatus::Error;
    }
    let parsed = match quote.to_quote() {
        Ok(parsed) => parsed,
        Err(message) => {
            write_error(out_error, message.as_str());
            return OpenPitMarketDataRegisterStatus::Error;
        }
    };
    match unsafe { &*service }
        .handle
        .push_patch(InstrumentId::new(instrument_id), parsed)
    {
        Ok(()) => OpenPitMarketDataRegisterStatus::Ok,
        Err(UnknownInstrumentId { .. }) => OpenPitMarketDataRegisterStatus::UnknownInstrument,
    }
}

//--------------------------------------------------------------------------------------------------
// Push by id — targeted fan-out

/// Publishes a quote for `instrument_id` into the per-account bucket of every
/// account in `account_ids` and the per-group bucket of every group in
/// `account_group_ids`, replacing each target's snapshot.
///
/// A null pointer with a matching length of `0` is a valid empty list.
///
/// Status:
/// - `Ok`: all targets were written;
/// - `UnknownInstrument`: `instrument_id` is not registered;
/// - `NoTarget`: both `account_ids` and `account_group_ids` are empty; use
///   `openpit_marketdata_service_push` to write the default bucket;
/// - `Error`: `service` is null or `quote` carries an invalid price; if
///   `out_error` is not null, a caller-owned `OpenPitSharedString` error handle
///   was written.
#[no_mangle]
pub extern "C" fn openpit_marketdata_service_push_for(
    service: *const OpenPitMarketDataService,
    instrument_id: OpenPitMarketDataInstrumentId,
    quote: OpenPitMarketDataQuote,
    account_ids: *const OpenPitParamAccountId,
    account_ids_len: usize,
    account_group_ids: *const OpenPitParamAccountGroupId,
    account_group_ids_len: usize,
    out_error: OpenPitOutError,
) -> OpenPitMarketDataRegisterStatus {
    if service.is_null() {
        write_error(out_error, "market-data service is null");
        return OpenPitMarketDataRegisterStatus::Error;
    }
    let parsed = match quote.to_quote() {
        Ok(parsed) => parsed,
        Err(message) => {
            write_error(out_error, message.as_str());
            return OpenPitMarketDataRegisterStatus::Error;
        }
    };
    // SAFETY: `AccountId` is `#[repr(transparent)]` over `u64` (the underlying
    // type of `OpenPitParamAccountId`) and `AccountId::from_u64` is a pure
    // value-wrap, so the raw array is bit-identical to `&[AccountId]`. Likewise
    // `AccountGroupId` is `#[repr(transparent)]` over `u32` and `import_group`
    // is the identity value-map (raw `0` == `DEFAULT_ACCOUNT_GROUP`; every
    // other value passes through unchanged), so the raw array is bit-identical
    // to `&[AccountGroupId]`. Each borrow is valid for its declared length; an
    // empty list (null pointer or zero length) becomes `&[]`.
    let accounts: &[AccountId] = if account_ids_len > 0 {
        unsafe { std::slice::from_raw_parts(account_ids as *const AccountId, account_ids_len) }
    } else {
        &[]
    };
    let groups: &[AccountGroupId] = if account_group_ids_len > 0 {
        unsafe {
            std::slice::from_raw_parts(
                account_group_ids as *const AccountGroupId,
                account_group_ids_len,
            )
        }
    } else {
        &[]
    };
    match unsafe { &*service }.handle.push_for(
        InstrumentId::new(instrument_id),
        parsed,
        accounts,
        groups,
    ) {
        Ok(()) => OpenPitMarketDataRegisterStatus::Ok,
        Err(PushForError::UnknownInstrument { .. }) => {
            OpenPitMarketDataRegisterStatus::UnknownInstrument
        }
        Err(PushForError::NoTarget) => OpenPitMarketDataRegisterStatus::NoTarget,
        // `PushForError` is `#[non_exhaustive]`; treat any future variant as
        // a boundary failure.
        Err(error) => {
            write_error(out_error, error.to_string().as_str());
            OpenPitMarketDataRegisterStatus::Error
        }
    }
}

/// Publishes a partial update for `instrument_id` into the per-account bucket
/// of every account in `account_ids` and the per-group bucket of every group in
/// `account_group_ids`, merging independently into each target's existing
/// snapshot.
///
/// Behaves like `openpit_marketdata_service_push_for` otherwise.
#[no_mangle]
pub extern "C" fn openpit_marketdata_service_push_for_patch(
    service: *const OpenPitMarketDataService,
    instrument_id: OpenPitMarketDataInstrumentId,
    quote: OpenPitMarketDataQuote,
    account_ids: *const OpenPitParamAccountId,
    account_ids_len: usize,
    account_group_ids: *const OpenPitParamAccountGroupId,
    account_group_ids_len: usize,
    out_error: OpenPitOutError,
) -> OpenPitMarketDataRegisterStatus {
    if service.is_null() {
        write_error(out_error, "market-data service is null");
        return OpenPitMarketDataRegisterStatus::Error;
    }
    let parsed = match quote.to_quote() {
        Ok(parsed) => parsed,
        Err(message) => {
            write_error(out_error, message.as_str());
            return OpenPitMarketDataRegisterStatus::Error;
        }
    };
    // SAFETY: `AccountId` is `#[repr(transparent)]` over `u64` (the underlying
    // type of `OpenPitParamAccountId`) and `AccountId::from_u64` is a pure
    // value-wrap, so the raw array is bit-identical to `&[AccountId]`. Likewise
    // `AccountGroupId` is `#[repr(transparent)]` over `u32` and `import_group`
    // is the identity value-map (raw `0` == `DEFAULT_ACCOUNT_GROUP`; every
    // other value passes through unchanged), so the raw array is bit-identical
    // to `&[AccountGroupId]`. Each borrow is valid for its declared length; an
    // empty list (null pointer or zero length) becomes `&[]`.
    let accounts: &[AccountId] = if account_ids_len > 0 {
        unsafe { std::slice::from_raw_parts(account_ids as *const AccountId, account_ids_len) }
    } else {
        &[]
    };
    let groups: &[AccountGroupId] = if account_group_ids_len > 0 {
        unsafe {
            std::slice::from_raw_parts(
                account_group_ids as *const AccountGroupId,
                account_group_ids_len,
            )
        }
    } else {
        &[]
    };
    match unsafe { &*service }.handle.push_for_patch(
        InstrumentId::new(instrument_id),
        parsed,
        accounts,
        groups,
    ) {
        Ok(()) => OpenPitMarketDataRegisterStatus::Ok,
        Err(PushForError::UnknownInstrument { .. }) => {
            OpenPitMarketDataRegisterStatus::UnknownInstrument
        }
        Err(PushForError::NoTarget) => OpenPitMarketDataRegisterStatus::NoTarget,
        // `PushForError` is `#[non_exhaustive]`; treat any future variant as
        // a boundary failure.
        Err(error) => {
            write_error(out_error, error.to_string().as_str());
            OpenPitMarketDataRegisterStatus::Error
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Push by instrument name

/// Publishes a quote for `instrument`, replacing the stored snapshot.
///
/// If `instrument` is unregistered, a named slot is created with the
/// service-default TTL.
///
/// Success:
/// - returns `true` and writes the instrument's id to `out_id`.
///
/// Error:
/// - returns `false` when `service`/`out_id` is null, the instrument payload
///   is invalid, or `quote` carries an invalid price;
/// - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
///   error handle.
#[no_mangle]
pub extern "C" fn openpit_marketdata_service_push_by_instrument(
    service: *const OpenPitMarketDataService,
    instrument: *const OpenPitInstrument,
    quote: OpenPitMarketDataQuote,
    out_id: *mut OpenPitMarketDataInstrumentId,
    out_error: OpenPitOutError,
) -> bool {
    if service.is_null() {
        write_error(out_error, "market-data service is null");
        return false;
    }
    if instrument.is_null() {
        write_error(out_error, "instrument is null");
        return false;
    }
    if out_id.is_null() {
        write_error(out_error, "out_id is null");
        return false;
    }
    let Some(parsed_instrument) = import_required_instrument(unsafe { &*instrument }, out_error)
    else {
        return false;
    };
    let parsed_quote = match quote.to_quote() {
        Ok(parsed) => parsed,
        Err(message) => {
            write_error(out_error, message.as_str());
            return false;
        }
    };
    let id = unsafe { &*service }
        .handle
        .push_by_instrument(&parsed_instrument, parsed_quote);
    unsafe { *out_id = id.as_u64() };
    true
}

/// Publishes a partial update for `instrument`, merging it into the stored
/// snapshot.
///
/// Behaves like `openpit_marketdata_service_push_by_instrument` otherwise.
#[no_mangle]
pub extern "C" fn openpit_marketdata_service_push_by_instrument_patch(
    service: *const OpenPitMarketDataService,
    instrument: *const OpenPitInstrument,
    quote: OpenPitMarketDataQuote,
    out_id: *mut OpenPitMarketDataInstrumentId,
    out_error: OpenPitOutError,
) -> bool {
    if service.is_null() {
        write_error(out_error, "market-data service is null");
        return false;
    }
    if instrument.is_null() {
        write_error(out_error, "instrument is null");
        return false;
    }
    if out_id.is_null() {
        write_error(out_error, "out_id is null");
        return false;
    }
    let Some(parsed_instrument) = import_required_instrument(unsafe { &*instrument }, out_error)
    else {
        return false;
    };
    let parsed_quote = match quote.to_quote() {
        Ok(parsed) => parsed,
        Err(message) => {
            write_error(out_error, message.as_str());
            return false;
        }
    };
    let id = unsafe { &*service }
        .handle
        .push_by_instrument_patch(&parsed_instrument, parsed_quote);
    unsafe { *out_id = id.as_u64() };
    true
}

//--------------------------------------------------------------------------------------------------
// Get / resolve

/// Reads the latest quote for `(instrument_id, account_id)` under the given
/// resolution.
///
/// `resolve_account_group` is a **required** callback that supplies the reading
/// account's group **lazily** — it is invoked only when the resolution mode
/// would consult a group or default-group bucket and the per-account bucket has
/// no quote. The callback receives the caller-supplied `user_data`
/// context pointer and, when the account belongs to a group, writes the group id
/// to `out_account_group_id` and returns `true`; when the account has no group
/// it returns `false`. Pass `OPENPIT_DEFAULT_ACCOUNT_GROUP` (`0`) to target the
/// default group bucket.
///
/// `resolution` controls which buckets are consulted, in order, when the
/// more-specific bucket has no quote.
///
/// Status:
/// - `Found`: a usable quote was written to `out_quote`;
/// - `Unavailable`: registered but no usable quote (never pushed, cleared, or
///   aged past TTL);
/// - `UnknownInstrument`: `instrument_id` is not registered.
///
/// Contract:
/// - `service`, `resolve_account_group`, and `out_quote` must be valid non-null
///   pointers; passing null for any of them aborts the call.
#[no_mangle]
pub extern "C" fn openpit_marketdata_service_get(
    service: *const OpenPitMarketDataService,
    instrument_id: OpenPitMarketDataInstrumentId,
    account_id: OpenPitParamAccountId,
    resolve_account_group: OpenPitMarketDataAccountGroupResolver,
    user_data: *mut c_void,
    resolution: OpenPitMarketDataQuoteResolution,
    out_quote: *mut OpenPitMarketDataQuote,
) -> OpenPitMarketDataGetStatus {
    assert!(!service.is_null(), "market-data service must be non-null");
    assert!(
        resolve_account_group.is_some(),
        "resolve_account_group must be non-null"
    );
    assert!(!out_quote.is_null(), "out_quote must be non-null");
    let adapter = CallbackAccountInfo {
        resolve: resolve_account_group.unwrap(),
        user_data,
    };
    match unsafe { &*service }.handle.get_or_err(
        InstrumentId::new(instrument_id),
        AccountId::from_u64(account_id),
        &adapter,
        resolution.into(),
    ) {
        Ok(quote) => {
            unsafe { *out_quote = OpenPitMarketDataQuote::from_quote(quote) };
            OpenPitMarketDataGetStatus::Found
        }
        Err(openpit::marketdata::MarketDataError::UnknownInstrument) => {
            OpenPitMarketDataGetStatus::UnknownInstrument
        }
        Err(openpit::marketdata::MarketDataError::QuoteUnavailable) => {
            OpenPitMarketDataGetStatus::Unavailable
        }
        // `MarketDataError` is `#[non_exhaustive]`; treat any future variant as
        // "no usable quote available".
        Err(_) => OpenPitMarketDataGetStatus::Unavailable,
    }
}

/// Resolves `instrument` to its registered id.
///
/// Success:
/// - returns `true` and writes the id to `out_id` when `instrument` is
///   registered by name;
/// - returns `false` (without writing `out_id`) when the instrument is not
///   registered, the instrument payload is invalid, or `service`/`out_id` is
///   null.
///
/// This call does not use `out_error`: a `false` result simply means
/// "not resolved".
#[no_mangle]
pub extern "C" fn openpit_marketdata_service_resolve(
    service: *const OpenPitMarketDataService,
    instrument: *const OpenPitInstrument,
    out_id: *mut OpenPitMarketDataInstrumentId,
) -> bool {
    if service.is_null() || instrument.is_null() || out_id.is_null() {
        return false;
    }
    let parsed = match import_instrument(unsafe { &*instrument }) {
        Ok(Some(parsed)) => parsed,
        _ => return false,
    };
    match unsafe { &*service }.handle.resolve(&parsed) {
        Some(id) => {
            unsafe { *out_id = id.as_u64() };
            true
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::string::{openpit_destroy_shared_string, OpenPitSharedString};
    use crate::OpenPitStringView;

    fn null_error() -> *mut OpenPitSharedString {
        std::ptr::null_mut()
    }

    fn instrument(underlying: &'static str, settlement: &'static str) -> OpenPitInstrument {
        OpenPitInstrument {
            underlying_asset: OpenPitStringView::from_utf8(underlying),
            settlement_asset: OpenPitStringView::from_utf8(settlement),
        }
    }

    fn price(value: &str) -> OpenPitParamPrice {
        let parsed = Price::from_str(value).expect("price must be valid");
        export_price(parsed)
    }

    fn quote_with_mark(mark_value: &str) -> OpenPitMarketDataQuote {
        OpenPitMarketDataQuote {
            mark: OpenPitParamPriceOptional {
                value: price(mark_value),
                is_set: true,
            },
            ..Default::default()
        }
    }

    /// Resolver that always returns false — the account has no group.
    extern "C" fn no_group_resolver(
        _user_data: *mut c_void,
        _out: *mut OpenPitParamAccountGroupId,
    ) -> bool {
        false
    }

    /// Resolver that always returns group id 1 (a fixed non-default group).
    extern "C" fn fixed_group_resolver(
        _user_data: *mut c_void,
        out: *mut OpenPitParamAccountGroupId,
    ) -> bool {
        unsafe { *out = 1 };
        true
    }

    /// Calls `openpit_marketdata_service_get` with no group (via
    /// `no_group_resolver`) and the broadest resolution.
    fn get_default(
        service: *const OpenPitMarketDataService,
        id: u64,
        out: *mut OpenPitMarketDataQuote,
    ) -> OpenPitMarketDataGetStatus {
        openpit_marketdata_service_get(
            service,
            id,
            0,
            Some(no_group_resolver),
            std::ptr::null_mut(),
            OpenPitMarketDataQuoteResolution::AccountThenGroupThenDefault,
            out,
        )
    }

    /// Creates a None-mode MD service for tests (byte 0 = None/no-sync).
    fn build_service() -> *mut OpenPitMarketDataService {
        let mut err = null_error();
        let service = openpit_create_marketdata_service(
            0,
            openpit_create_marketdata_quote_ttl_infinite(),
            &mut err,
        );
        assert!(!service.is_null());
        assert!(err.is_null());
        service
    }

    /// Creates a Full-mode MD service for tests (byte 1 = Full).
    fn build_full_service() -> *mut OpenPitMarketDataService {
        let mut err = null_error();
        let service = openpit_create_marketdata_service(
            1,
            openpit_create_marketdata_quote_ttl_infinite(),
            &mut err,
        );
        assert!(!service.is_null());
        assert!(err.is_null());
        service
    }

    #[test]
    fn build_register_push_get_roundtrip() {
        let service = build_service();
        let inst = instrument("AAPL", "USD");
        let mut id: u64 = 0;
        let mut err = null_error();
        assert_eq!(
            openpit_marketdata_service_register(service, &inst, &mut id, &mut err),
            OpenPitMarketDataRegisterStatus::Ok
        );
        assert!(err.is_null());

        let quote = quote_with_mark("200");
        assert_eq!(
            openpit_marketdata_service_push(service, id, quote, &mut err),
            OpenPitMarketDataRegisterStatus::Ok
        );
        assert!(err.is_null());

        let mut out_quote = OpenPitMarketDataQuote::default();
        let status = get_default(service, id, &mut out_quote);
        assert_eq!(status, OpenPitMarketDataGetStatus::Found);
        assert!(out_quote.mark.is_set);
        assert!(!out_quote.bid.is_set);

        openpit_destroy_marketdata_service(service);
    }

    #[test]
    fn get_unknown_instrument() {
        let service = build_service();
        let mut out_quote = OpenPitMarketDataQuote::default();
        let status = get_default(service, 999, &mut out_quote);
        assert_eq!(status, OpenPitMarketDataGetStatus::UnknownInstrument);
        openpit_destroy_marketdata_service(service);
    }

    #[test]
    fn get_unavailable_after_clear() {
        let service = build_service();
        let inst = instrument("AAPL", "USD");
        let mut id: u64 = 0;
        let mut err = null_error();
        openpit_marketdata_service_register(service, &inst, &mut id, &mut err);
        openpit_marketdata_service_push(service, id, quote_with_mark("200"), &mut err);
        openpit_marketdata_service_clear(service, id);

        let mut out_quote = OpenPitMarketDataQuote::default();
        let status = get_default(service, id, &mut out_quote);
        assert_eq!(status, OpenPitMarketDataGetStatus::Unavailable);
        openpit_destroy_marketdata_service(service);
    }

    #[test]
    fn clone_shares_underlying_service() {
        let service = build_service();
        let cloned = openpit_marketdata_service_clone(service);
        assert!(!cloned.is_null());

        let inst = instrument("AAPL", "USD");
        let mut id: u64 = 0;
        let mut err = null_error();
        openpit_marketdata_service_register(service, &inst, &mut id, &mut err);
        openpit_marketdata_service_push(service, id, quote_with_mark("200"), &mut err);

        // The clone observes the registration/push made through `service`.
        let mut out_quote = OpenPitMarketDataQuote::default();
        let status = get_default(cloned, id, &mut out_quote);
        assert_eq!(status, OpenPitMarketDataGetStatus::Found);

        openpit_destroy_marketdata_service(cloned);
        openpit_destroy_marketdata_service(service);
    }

    #[test]
    fn push_by_instrument_auto_registers_and_resolves() {
        let service = build_service();
        let inst = instrument("BTC", "USD");
        let quote = OpenPitMarketDataQuote {
            bid: OpenPitParamPriceOptional {
                value: price("50000"),
                is_set: true,
            },
            ask: OpenPitParamPriceOptional {
                value: price("50001"),
                is_set: true,
            },
            ..Default::default()
        };
        let mut id: u64 = u64::MAX;
        let mut err = null_error();
        assert!(openpit_marketdata_service_push_by_instrument(
            service, &inst, quote, &mut id, &mut err
        ));
        assert!(err.is_null());

        let mut resolved: u64 = 0;
        assert!(openpit_marketdata_service_resolve(
            service,
            &inst,
            &mut resolved
        ));
        assert_eq!(resolved, id);

        openpit_destroy_marketdata_service(service);
    }

    #[test]
    fn register_duplicate_returns_already_registered() {
        let service = build_service();
        let inst = instrument("AAPL", "USD");
        let mut id: u64 = 0;
        let mut err = null_error();
        assert_eq!(
            openpit_marketdata_service_register(service, &inst, &mut id, &mut err),
            OpenPitMarketDataRegisterStatus::Ok
        );
        let mut err2 = null_error();
        assert_eq!(
            openpit_marketdata_service_register(service, &inst, &mut id, &mut err2),
            OpenPitMarketDataRegisterStatus::AlreadyRegistered
        );
        // Domain variants do not write the generic error string.
        assert!(err2.is_null());
        openpit_destroy_marketdata_service(service);
    }

    #[test]
    fn register_with_id_duplicate_id_returns_status() {
        let service = build_service();
        let inst_a = instrument("AAPL", "USD");
        let inst_b = instrument("MSFT", "USD");
        let mut out_id: u64 = 0;
        let mut err = null_error();
        assert_eq!(
            openpit_marketdata_service_register_with_id(service, &inst_a, 7, &mut out_id, &mut err),
            OpenPitMarketDataRegisterStatus::Ok
        );
        let mut err2 = null_error();
        assert_eq!(
            openpit_marketdata_service_register_with_id(
                service,
                &inst_b,
                7,
                &mut out_id,
                &mut err2
            ),
            OpenPitMarketDataRegisterStatus::DuplicateId
        );
        assert!(err2.is_null());
        openpit_destroy_marketdata_service(service);
    }

    #[test]
    fn register_with_id_duplicate_instrument_returns_status() {
        let service = build_service();
        let inst = instrument("AAPL", "USD");
        let mut out_id: u64 = 0;
        let mut err = null_error();
        assert_eq!(
            openpit_marketdata_service_register_with_id(service, &inst, 1, &mut out_id, &mut err),
            OpenPitMarketDataRegisterStatus::Ok
        );
        let mut err2 = null_error();
        assert_eq!(
            openpit_marketdata_service_register_with_id(service, &inst, 2, &mut out_id, &mut err2),
            OpenPitMarketDataRegisterStatus::DuplicateInstrument
        );
        assert!(err2.is_null());
        openpit_destroy_marketdata_service(service);
    }

    #[test]
    fn set_instrument_ttl_unknown_returns_status() {
        let service = build_service();
        let status = openpit_marketdata_service_set_instrument_ttl(
            service,
            42,
            openpit_create_marketdata_quote_ttl_within(5, 0),
        );
        assert_eq!(status, OpenPitMarketDataRegisterStatus::UnknownInstrument);
        openpit_destroy_marketdata_service(service);
    }

    #[test]
    fn push_unknown_returns_status() {
        let service = build_service();
        let quote = OpenPitMarketDataQuote::default();
        let mut err = null_error();
        let status = openpit_marketdata_service_push(service, 99, quote, &mut err);
        assert_eq!(status, OpenPitMarketDataRegisterStatus::UnknownInstrument);
        assert!(err.is_null());
        openpit_destroy_marketdata_service(service);
    }

    #[test]
    fn register_null_service_returns_error() {
        let inst = instrument("AAPL", "USD");
        let mut id: u64 = 0;
        let mut err = null_error();
        let status =
            openpit_marketdata_service_register(std::ptr::null(), &inst, &mut id, &mut err);
        assert_eq!(status, OpenPitMarketDataRegisterStatus::Error);
        assert!(!err.is_null());
        openpit_destroy_shared_string(err);
    }

    #[test]
    fn destroy_and_clone_tolerate_null() {
        openpit_destroy_marketdata_service(std::ptr::null_mut());
        assert!(openpit_marketdata_service_clone(std::ptr::null()).is_null());
    }

    #[test]
    fn create_md_service_invalid_mode_returns_null_with_error() {
        let mut err = null_error();
        let service = openpit_create_marketdata_service(
            2, // Account byte — invalid for MD
            openpit_create_marketdata_quote_ttl_infinite(),
            &mut err,
        );
        assert!(service.is_null());
        assert!(!err.is_null());
        openpit_destroy_shared_string(err);
    }

    #[test]
    fn create_md_service_invalid_mode_tolerates_null_out_error() {
        let service = openpit_create_marketdata_service(
            99,
            openpit_create_marketdata_quote_ttl_infinite(),
            std::ptr::null_mut(),
        );
        assert!(service.is_null());
    }

    #[test]
    fn create_full_and_local_md_service_directly() {
        // None mode (byte 0).
        let mut err = null_error();
        let none_svc = openpit_create_marketdata_service(
            0,
            openpit_create_marketdata_quote_ttl_infinite(),
            &mut err,
        );
        assert!(!none_svc.is_null());
        assert!(err.is_null());
        assert_eq!(unsafe { &*none_svc }.mode, SyncMode::None);
        openpit_destroy_marketdata_service(none_svc);

        // Full mode (byte 1).
        let full_svc = openpit_create_marketdata_service(
            1,
            openpit_create_marketdata_quote_ttl_infinite(),
            &mut err,
        );
        assert!(!full_svc.is_null());
        assert!(err.is_null());
        assert_eq!(unsafe { &*full_svc }.mode, SyncMode::Full);
        openpit_destroy_marketdata_service(full_svc);
    }

    #[test]
    fn local_md_service_is_functional() {
        let service = build_full_service();
        let inst = instrument("ETH", "USD");
        let mut id: u64 = 0;
        let mut err = null_error();
        assert_eq!(
            openpit_marketdata_service_register(service, &inst, &mut id, &mut err),
            OpenPitMarketDataRegisterStatus::Ok
        );
        openpit_destroy_marketdata_service(service);
    }

    // ── New tests for the redesigned API ─────────────────────────────────────

    #[test]
    fn push_for_per_account_vs_default_under_resolutions() {
        let service = build_service();
        let inst = instrument("AAPL", "USD");
        let mut id: u64 = 0;
        let mut err = null_error();
        openpit_marketdata_service_register(service, &inst, &mut id, &mut err);

        // Push the default bucket with mark=100.
        openpit_marketdata_service_push(service, id, quote_with_mark("100"), &mut err);

        // Push per-account (account=42) bucket with mark=200.
        let account: u64 = 42;
        let status = openpit_marketdata_service_push_for(
            service,
            id,
            quote_with_mark("200"),
            &account,
            1,
            std::ptr::null(),
            0,
            &mut err,
        );
        assert_eq!(status, OpenPitMarketDataRegisterStatus::Ok);
        assert!(err.is_null());

        // AccountOnly for account=42 -> per-account bucket -> mark=200.
        let mut out = OpenPitMarketDataQuote::default();
        let status = openpit_marketdata_service_get(
            service,
            id,
            account,
            Some(no_group_resolver),
            std::ptr::null_mut(),
            OpenPitMarketDataQuoteResolution::AccountOnly,
            &mut out,
        );
        assert_eq!(status, OpenPitMarketDataGetStatus::Found);
        assert!(out.mark.is_set);

        // AccountThenGroupThenDefault for account=99 (no per-account quote)
        // falls through to default bucket -> mark=100.
        let mut out2 = OpenPitMarketDataQuote::default();
        let status2 = openpit_marketdata_service_get(
            service,
            id,
            99,
            Some(no_group_resolver),
            std::ptr::null_mut(),
            OpenPitMarketDataQuoteResolution::AccountThenGroupThenDefault,
            &mut out2,
        );
        assert_eq!(status2, OpenPitMarketDataGetStatus::Found);
        assert!(out2.mark.is_set);

        openpit_destroy_marketdata_service(service);
    }

    #[test]
    fn push_for_empty_lists_returns_no_target() {
        let service = build_service();
        let inst = instrument("AAPL", "USD");
        let mut id: u64 = 0;
        let mut err = null_error();
        openpit_marketdata_service_register(service, &inst, &mut id, &mut err);

        let status = openpit_marketdata_service_push_for(
            service,
            id,
            quote_with_mark("100"),
            std::ptr::null(),
            0,
            std::ptr::null(),
            0,
            &mut err,
        );
        assert_eq!(status, OpenPitMarketDataRegisterStatus::NoTarget);

        openpit_destroy_marketdata_service(service);
    }

    #[test]
    fn set_instrument_account_ttl_shadows_default() {
        let service = build_service();
        let inst = instrument("AAPL", "USD");
        let mut id: u64 = 0;
        let mut err = null_error();
        openpit_marketdata_service_register(service, &inst, &mut id, &mut err);
        openpit_marketdata_service_push(service, id, quote_with_mark("100"), &mut err);

        // Pin a zero-duration TTL for account=1 — the quote will appear stale
        // immediately on the next read for that account.
        let zero_ttl = openpit_create_marketdata_quote_ttl_within(0, 0);
        let status =
            openpit_marketdata_service_set_instrument_account_ttl(service, id, 1, zero_ttl);
        assert_eq!(status, OpenPitMarketDataRegisterStatus::Ok);

        // Account=1 should see the quote as unavailable (TTL=0 => already stale).
        let mut out = OpenPitMarketDataQuote::default();
        let status = openpit_marketdata_service_get(
            service,
            id,
            1,
            Some(no_group_resolver),
            std::ptr::null_mut(),
            OpenPitMarketDataQuoteResolution::AccountOnly,
            &mut out,
        );
        assert_eq!(status, OpenPitMarketDataGetStatus::Unavailable);

        // Account=2 should still find the quote via the default bucket with
        // the service-wide infinite TTL.
        let mut out2 = OpenPitMarketDataQuote::default();
        let status2 = get_default(service, id, &mut out2);
        assert_eq!(status2, OpenPitMarketDataGetStatus::Found);

        openpit_destroy_marketdata_service(service);
    }

    #[test]
    fn get_with_no_group_resolver_falls_through_to_default_bucket() {
        let service = build_service();
        let inst = instrument("AAPL", "USD");
        let mut id: u64 = 0;
        let mut err = null_error();
        openpit_marketdata_service_register(service, &inst, &mut id, &mut err);
        openpit_marketdata_service_push(service, id, quote_with_mark("100"), &mut err);

        // no_group_resolver returns false → group is None; AccountThenGroup
        // can't fall through to a group bucket, but AccountThenGroupThenDefault
        // can still fall through to the default bucket.
        let mut out = OpenPitMarketDataQuote::default();
        let status = openpit_marketdata_service_get(
            service,
            id,
            7, // account_id
            Some(no_group_resolver),
            std::ptr::null_mut(),
            OpenPitMarketDataQuoteResolution::AccountThenGroupThenDefault,
            &mut out,
        );
        assert_eq!(status, OpenPitMarketDataGetStatus::Found);
        assert!(out.mark.is_set);

        openpit_destroy_marketdata_service(service);
    }

    #[test]
    fn get_with_fixed_group_resolver_uses_group_bucket() {
        let service = build_service();
        let inst = instrument("AAPL", "USD");
        let mut id: u64 = 0;
        let mut err = null_error();
        openpit_marketdata_service_register(service, &inst, &mut id, &mut err);

        // Push group=1 bucket with mark=300.
        let group1: OpenPitParamAccountGroupId = 1;
        openpit_marketdata_service_push_for(
            service,
            id,
            quote_with_mark("300"),
            std::ptr::null(),
            0,
            &group1,
            1,
            &mut err,
        );
        assert!(err.is_null());

        // fixed_group_resolver returns group=1; account=77 has no per-account
        // quote so AccountThenGroup falls through to the group bucket.
        let mut out = OpenPitMarketDataQuote::default();
        let status = openpit_marketdata_service_get(
            service,
            id,
            77, // account_id — no per-account quote
            Some(fixed_group_resolver),
            std::ptr::null_mut(),
            OpenPitMarketDataQuoteResolution::AccountThenGroup,
            &mut out,
        );
        assert_eq!(status, OpenPitMarketDataGetStatus::Found);
        assert!(out.mark.is_set);

        openpit_destroy_marketdata_service(service);
    }
}
