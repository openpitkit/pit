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

//! FFI surface for core instrument reference data.

#![allow(clippy::not_unsafe_ptr_arg_deref)]

use openpit::{
    InstrumentId, ReferenceBook, ReferenceBookRegistrationError, SettlementLag, SettlementScheme,
    SettlementUnit, UnknownReferenceBookInstrumentId,
};

use crate::instrument::{import_instrument, OpenPitInstrument};
use crate::last_error::{write_error, OpenPitOutError};
use crate::marketdata::OpenPitInstrumentId;

/// Raw settlement-unit code for FFI payloads.
///
/// The value is validated before it is converted into the Rust
/// [`SettlementUnit`].
pub type OpenPitSettlementUnit = u8;

/// Business-day settlement delay.
pub const OPENPIT_SETTLEMENT_UNIT_BUSINESS_DAYS: OpenPitSettlementUnit = 0;
/// Calendar-day settlement delay.
pub const OPENPIT_SETTLEMENT_UNIT_CALENDAR_DAYS: OpenPitSettlementUnit = 1;

/// Flat settlement delay payload.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct OpenPitSettlementLag {
    /// Number of elapsed settlement units.
    pub n: u64,
    /// One of the `OPENPIT_SETTLEMENT_UNIT_*` codes.
    pub unit: OpenPitSettlementUnit,
}

/// Flat settlement payload with independent delivery and payment legs.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct OpenPitSettlementScheme {
    /// Settlement delay for delivery of the traded asset.
    pub delivery: OpenPitSettlementLag,
    /// Settlement delay for payment in the settlement asset.
    pub payment: OpenPitSettlementLag,
}

impl OpenPitSettlementScheme {
    fn to_scheme(self) -> Result<SettlementScheme, String> {
        Ok(SettlementScheme::new(
            settlement_lag_from_raw(self.delivery, "delivery")?,
            settlement_lag_from_raw(self.payment, "payment")?,
        ))
    }

    fn from_scheme(value: SettlementScheme) -> Self {
        Self {
            delivery: settlement_lag_to_raw(value.delivery()),
            payment: settlement_lag_to_raw(value.payment()),
        }
    }
}

fn settlement_lag_from_raw(
    value: OpenPitSettlementLag,
    field: &str,
) -> Result<SettlementLag, String> {
    let unit = match value.unit {
        OPENPIT_SETTLEMENT_UNIT_BUSINESS_DAYS => SettlementUnit::BusinessDays,
        OPENPIT_SETTLEMENT_UNIT_CALENDAR_DAYS => SettlementUnit::CalendarDays,
        raw => {
            return Err(format!(
                "{field}.unit has invalid settlement-unit value {raw}"
            ))
        }
    };
    Ok(SettlementLag::new(value.n, unit))
}

fn settlement_lag_to_raw(value: SettlementLag) -> OpenPitSettlementLag {
    OpenPitSettlementLag {
        n: value.n(),
        unit: match value.unit() {
            SettlementUnit::BusinessDays => OPENPIT_SETTLEMENT_UNIT_BUSINESS_DAYS,
            SettlementUnit::CalendarDays => OPENPIT_SETTLEMENT_UNIT_CALENDAR_DAYS,
        },
    }
}

/// Registration result for a reference book.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenPitReferenceBookRegisterStatus {
    /// The instrument was registered and `out_id` was populated.
    Ok = 0,
    /// The supplied ID is already registered.
    DuplicateId = 1,
    /// The supplied instrument is already registered.
    DuplicateInstrument = 2,
    /// The input payload or handle was invalid.
    Error = 255,
}

/// Result for reference-book attribute updates.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenPitReferenceBookStatus {
    /// The operation completed successfully.
    Ok = 0,
    /// The requested instrument ID is not registered.
    UnknownInstrument = 1,
    /// The input payload or handle was invalid.
    Error = 255,
}

/// Opaque handle to a core instrument reference book.
pub struct OpenPitReferenceBook {
    inner: ReferenceBook,
}

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

/// Creates an empty core instrument reference book.
///
/// The returned handle is caller-owned and must be released with
/// [`openpit_destroy_reference_book`].
#[no_mangle]
pub extern "C" fn openpit_create_reference_book() -> *mut OpenPitReferenceBook {
    Box::into_raw(Box::new(OpenPitReferenceBook {
        inner: ReferenceBook::new(),
    }))
}

/// Releases a caller-owned reference-book handle.
///
/// Passing null is allowed and has no effect.
#[no_mangle]
pub extern "C" fn openpit_destroy_reference_book(book: *mut OpenPitReferenceBook) {
    if book.is_null() {
        return;
    }
    unsafe { drop(Box::from_raw(book)) };
}

/// Registers `instrument` under the next available reference-book ID.
///
/// `out_id` receives the assigned ID on success. The function reports duplicate
/// registrations through its return status; malformed inputs use `out_error`.
#[no_mangle]
pub extern "C" fn openpit_reference_book_register(
    book: *mut OpenPitReferenceBook,
    instrument: *const OpenPitInstrument,
    out_id: *mut OpenPitInstrumentId,
    out_error: OpenPitOutError,
) -> OpenPitReferenceBookRegisterStatus {
    if book.is_null() {
        write_error(out_error, "reference book is null");
        return OpenPitReferenceBookRegisterStatus::Error;
    }
    if instrument.is_null() {
        write_error(out_error, "instrument is null");
        return OpenPitReferenceBookRegisterStatus::Error;
    }
    if out_id.is_null() {
        write_error(out_error, "out_id is null");
        return OpenPitReferenceBookRegisterStatus::Error;
    }
    let Some(instrument) = import_required_instrument(unsafe { &*instrument }, out_error) else {
        return OpenPitReferenceBookRegisterStatus::Error;
    };
    match unsafe { &mut *book }.inner.register(instrument) {
        Ok(instrument_id) => {
            unsafe { *out_id = instrument_id.as_u64() };
            OpenPitReferenceBookRegisterStatus::Ok
        }
        Err(ReferenceBookRegistrationError::DuplicateInstrument { .. }) => {
            OpenPitReferenceBookRegisterStatus::DuplicateInstrument
        }
        Err(ReferenceBookRegistrationError::DuplicateId { .. }) => {
            OpenPitReferenceBookRegisterStatus::DuplicateId
        }
        Err(error) => {
            write_error(out_error, error.to_string().as_str());
            OpenPitReferenceBookRegisterStatus::Error
        }
    }
}

/// Registers `instrument` under a caller-assigned `instrument_id`.
///
/// `out_id` receives the same ID on success. The supplied ID can be reused in
/// an independent market-data registration.
#[no_mangle]
pub extern "C" fn openpit_reference_book_register_with_id(
    book: *mut OpenPitReferenceBook,
    instrument: *const OpenPitInstrument,
    instrument_id: OpenPitInstrumentId,
    out_id: *mut OpenPitInstrumentId,
    out_error: OpenPitOutError,
) -> OpenPitReferenceBookRegisterStatus {
    if book.is_null() {
        write_error(out_error, "reference book is null");
        return OpenPitReferenceBookRegisterStatus::Error;
    }
    if instrument.is_null() {
        write_error(out_error, "instrument is null");
        return OpenPitReferenceBookRegisterStatus::Error;
    }
    if out_id.is_null() {
        write_error(out_error, "out_id is null");
        return OpenPitReferenceBookRegisterStatus::Error;
    }
    let Some(instrument) = import_required_instrument(unsafe { &*instrument }, out_error) else {
        return OpenPitReferenceBookRegisterStatus::Error;
    };
    match unsafe { &mut *book }
        .inner
        .register_with_id(instrument, InstrumentId::new(instrument_id))
    {
        Ok(registered_id) => {
            unsafe { *out_id = registered_id.as_u64() };
            OpenPitReferenceBookRegisterStatus::Ok
        }
        Err(ReferenceBookRegistrationError::DuplicateId { .. }) => {
            OpenPitReferenceBookRegisterStatus::DuplicateId
        }
        Err(ReferenceBookRegistrationError::DuplicateInstrument { .. }) => {
            OpenPitReferenceBookRegisterStatus::DuplicateInstrument
        }
        Err(error) => {
            write_error(out_error, error.to_string().as_str());
            OpenPitReferenceBookRegisterStatus::Error
        }
    }
}

/// Resolves an instrument to its reference-book ID.
///
/// Returns `true` and populates `out_id` when the instrument is registered.
/// Returns `false` when it is absent or an input pointer is null.
#[no_mangle]
pub extern "C" fn openpit_reference_book_resolve(
    book: *const OpenPitReferenceBook,
    instrument: *const OpenPitInstrument,
    out_id: *mut OpenPitInstrumentId,
) -> bool {
    if book.is_null() || instrument.is_null() || out_id.is_null() {
        return false;
    }
    let Ok(Some(instrument)) = import_instrument(unsafe { &*instrument }) else {
        return false;
    };
    let Some(instrument_id) = unsafe { &*book }.inner.resolve(&instrument) else {
        return false;
    };
    unsafe { *out_id = instrument_id.as_u64() };
    true
}

/// Sets the settlement scheme for a registered instrument.
///
/// Invalid raw settlement-unit codes are rejected before a core enum is
/// constructed. `UnknownInstrument` means the book has no matching ID.
#[no_mangle]
pub extern "C" fn openpit_reference_book_set_settlement_scheme(
    book: *mut OpenPitReferenceBook,
    instrument_id: OpenPitInstrumentId,
    settlement_scheme: OpenPitSettlementScheme,
    out_error: OpenPitOutError,
) -> OpenPitReferenceBookStatus {
    if book.is_null() {
        write_error(out_error, "reference book is null");
        return OpenPitReferenceBookStatus::Error;
    }
    let scheme = match settlement_scheme.to_scheme() {
        Ok(scheme) => scheme,
        Err(error) => {
            write_error(out_error, error.as_str());
            return OpenPitReferenceBookStatus::Error;
        }
    };
    match unsafe { &mut *book }
        .inner
        .set_settlement_scheme(InstrumentId::new(instrument_id), scheme)
    {
        Ok(()) => OpenPitReferenceBookStatus::Ok,
        Err(UnknownReferenceBookInstrumentId { .. }) => {
            OpenPitReferenceBookStatus::UnknownInstrument
        }
    }
}

/// Clears the settlement scheme for a registered instrument.
#[no_mangle]
pub extern "C" fn openpit_reference_book_clear_settlement_scheme(
    book: *mut OpenPitReferenceBook,
    instrument_id: OpenPitInstrumentId,
    out_error: OpenPitOutError,
) -> OpenPitReferenceBookStatus {
    if book.is_null() {
        write_error(out_error, "reference book is null");
        return OpenPitReferenceBookStatus::Error;
    }
    match unsafe { &mut *book }
        .inner
        .clear_settlement_scheme(InstrumentId::new(instrument_id))
    {
        Ok(()) => OpenPitReferenceBookStatus::Ok,
        Err(UnknownReferenceBookInstrumentId { .. }) => {
            OpenPitReferenceBookStatus::UnknownInstrument
        }
    }
}

/// Retrieves the settlement scheme for a registered instrument.
///
/// `Ok` with `out_is_set == false` means that the instrument is registered but
/// has no settlement scheme. `UnknownInstrument` means that `instrument_id`
/// is not registered. On `Ok` with `out_is_set == true`, `out_scheme` receives
/// the configured value.
#[no_mangle]
pub extern "C" fn openpit_reference_book_get_settlement_scheme(
    book: *const OpenPitReferenceBook,
    instrument_id: OpenPitInstrumentId,
    out_scheme: *mut OpenPitSettlementScheme,
    out_is_set: *mut bool,
    out_error: OpenPitOutError,
) -> OpenPitReferenceBookStatus {
    if book.is_null() {
        write_error(out_error, "reference book is null");
        return OpenPitReferenceBookStatus::Error;
    }
    if out_scheme.is_null() {
        write_error(out_error, "out_scheme is null");
        return OpenPitReferenceBookStatus::Error;
    }
    if out_is_set.is_null() {
        write_error(out_error, "out_is_set is null");
        return OpenPitReferenceBookStatus::Error;
    }
    match unsafe { &*book }
        .inner
        .settlement_scheme(InstrumentId::new(instrument_id))
    {
        Ok(Some(scheme)) => {
            unsafe {
                *out_scheme = OpenPitSettlementScheme::from_scheme(scheme);
                *out_is_set = true;
            }
            OpenPitReferenceBookStatus::Ok
        }
        Ok(None) => {
            unsafe { *out_is_set = false };
            OpenPitReferenceBookStatus::Ok
        }
        Err(UnknownReferenceBookInstrumentId { .. }) => {
            OpenPitReferenceBookStatus::UnknownInstrument
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ptr;

    use super::{
        openpit_create_reference_book, openpit_destroy_reference_book,
        openpit_reference_book_get_settlement_scheme, openpit_reference_book_register,
        openpit_reference_book_register_with_id, openpit_reference_book_resolve,
        openpit_reference_book_set_settlement_scheme, OpenPitInstrumentId,
        OpenPitReferenceBookRegisterStatus, OpenPitReferenceBookStatus, OpenPitSettlementLag,
        OpenPitSettlementScheme, OPENPIT_SETTLEMENT_UNIT_BUSINESS_DAYS,
        OPENPIT_SETTLEMENT_UNIT_CALENDAR_DAYS,
    };
    use crate::instrument::OpenPitInstrument;
    use crate::last_error::OpenPitOutError;
    use crate::string::OpenPitStringView;

    fn instrument() -> OpenPitInstrument {
        OpenPitInstrument {
            underlying_asset: OpenPitStringView::from_utf8("AAPL"),
            settlement_asset: OpenPitStringView::from_utf8("USD"),
        }
    }

    fn scheme() -> OpenPitSettlementScheme {
        OpenPitSettlementScheme {
            delivery: OpenPitSettlementLag {
                n: 2,
                unit: OPENPIT_SETTLEMENT_UNIT_BUSINESS_DAYS,
            },
            payment: OpenPitSettlementLag {
                n: 1,
                unit: OPENPIT_SETTLEMENT_UNIT_CALENDAR_DAYS,
            },
        }
    }

    #[test]
    fn reference_book_round_trips_ids_and_typed_settlement() {
        let book = openpit_create_reference_book();
        let instrument = instrument();
        let mut id: OpenPitInstrumentId = 0;

        assert_eq!(
            openpit_reference_book_register(
                book,
                &instrument,
                &mut id,
                ptr::null_mut::<_>() as OpenPitOutError,
            ),
            OpenPitReferenceBookRegisterStatus::Ok
        );
        let mut resolved = 0;
        assert!(openpit_reference_book_resolve(
            book,
            &instrument,
            &mut resolved
        ));
        assert_eq!(resolved, id);
        assert_eq!(
            openpit_reference_book_set_settlement_scheme(
                book,
                id,
                scheme(),
                ptr::null_mut::<_>() as OpenPitOutError,
            ),
            OpenPitReferenceBookStatus::Ok
        );

        let mut exported = OpenPitSettlementScheme::default();
        let mut is_set = false;
        assert_eq!(
            openpit_reference_book_get_settlement_scheme(
                book,
                id,
                &mut exported,
                &mut is_set,
                ptr::null_mut::<_>() as OpenPitOutError,
            ),
            OpenPitReferenceBookStatus::Ok
        );
        assert!(is_set);
        assert_eq!(exported, scheme());
        assert_eq!(
            openpit_reference_book_get_settlement_scheme(
                book,
                id + 1,
                &mut exported,
                &mut is_set,
                ptr::null_mut::<_>() as OpenPitOutError,
            ),
            OpenPitReferenceBookStatus::UnknownInstrument
        );
        openpit_destroy_reference_book(book);
    }

    #[test]
    fn reference_book_distinguishes_missing_scheme_from_unknown_instrument() {
        let book = openpit_create_reference_book();
        let instrument = instrument();
        let mut id: OpenPitInstrumentId = 0;
        assert_eq!(
            openpit_reference_book_register(
                book,
                &instrument,
                &mut id,
                ptr::null_mut::<_>() as OpenPitOutError,
            ),
            OpenPitReferenceBookRegisterStatus::Ok
        );

        let mut exported = OpenPitSettlementScheme::default();
        let mut is_set = true;
        assert_eq!(
            openpit_reference_book_get_settlement_scheme(
                book,
                id,
                &mut exported,
                &mut is_set,
                ptr::null_mut::<_>() as OpenPitOutError,
            ),
            OpenPitReferenceBookStatus::Ok
        );
        assert!(!is_set);
        assert_eq!(
            openpit_reference_book_get_settlement_scheme(
                book,
                id + 1,
                &mut exported,
                &mut is_set,
                ptr::null_mut::<_>() as OpenPitOutError,
            ),
            OpenPitReferenceBookStatus::UnknownInstrument
        );
        openpit_destroy_reference_book(book);
    }

    #[test]
    fn reference_book_rejects_duplicate_and_invalid_inputs() {
        let book = openpit_create_reference_book();
        let instrument = instrument();
        let mut id = 0;
        assert_eq!(
            openpit_reference_book_register_with_id(
                book,
                &instrument,
                42,
                &mut id,
                ptr::null_mut::<_>() as OpenPitOutError,
            ),
            OpenPitReferenceBookRegisterStatus::Ok
        );
        assert_eq!(
            openpit_reference_book_register_with_id(
                book,
                &instrument,
                43,
                &mut id,
                ptr::null_mut::<_>() as OpenPitOutError,
            ),
            OpenPitReferenceBookRegisterStatus::DuplicateInstrument
        );
        let mut invalid = scheme();
        invalid.payment.unit = 99;
        assert_eq!(
            openpit_reference_book_set_settlement_scheme(
                book,
                42,
                invalid,
                ptr::null_mut::<_>() as OpenPitOutError,
            ),
            OpenPitReferenceBookStatus::Error
        );
        assert_eq!(
            openpit_reference_book_set_settlement_scheme(
                book,
                99,
                scheme(),
                ptr::null_mut::<_>() as OpenPitOutError,
            ),
            OpenPitReferenceBookStatus::UnknownInstrument
        );
        openpit_destroy_reference_book(book);
    }
}
