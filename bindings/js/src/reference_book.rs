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

//! Core instrument reference-book bindings.

use js_sys::{Object, Reflect};
use openpit::{
    ReferenceBook, ReferenceBookRegistrationError, SettlementLag, SettlementScheme, SettlementUnit,
    UnknownReferenceBookInstrumentId,
};
use wasm_bindgen::prelude::*;

use crate::domain::{parse_u64_bigint, resolve_instrument_id, BigIntLike, InstrumentIdLike};
use crate::error::{make_error_with, ErrorKind};
use crate::marketdata::{InstrumentLike, JsInstrument};
use crate::param::ids::JsInstrumentId;

/// Unit used to measure one settlement delay.
#[wasm_bindgen(js_name = SettlementUnit)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JsSettlementUnit {
    /// Business days in the caller-supplied settlement calendar.
    BusinessDays,
    /// Consecutive calendar days.
    CalendarDays,
}

impl From<JsSettlementUnit> for SettlementUnit {
    fn from(value: JsSettlementUnit) -> Self {
        match value {
            JsSettlementUnit::BusinessDays => Self::BusinessDays,
            JsSettlementUnit::CalendarDays => Self::CalendarDays,
        }
    }
}

impl From<SettlementUnit> for JsSettlementUnit {
    fn from(value: SettlementUnit) -> Self {
        match value {
            SettlementUnit::BusinessDays => Self::BusinessDays,
            SettlementUnit::CalendarDays => Self::CalendarDays,
        }
    }
}

/// Delay between trade time and settlement of one leg.
#[wasm_bindgen(js_name = SettlementLag)]
#[derive(Clone, Copy)]
pub struct JsSettlementLag {
    inner: SettlementLag,
}

#[wasm_bindgen(js_class = SettlementLag)]
impl JsSettlementLag {
    /// Creates a settlement delay with an exact non-negative 64-bit duration.
    #[wasm_bindgen(constructor)]
    pub fn new(n: BigIntLike, unit: JsSettlementUnit) -> Result<Self, JsValue> {
        let n = parse_u64_bigint(n.into(), "settlement lag")?;
        Ok(Self {
            inner: SettlementLag::new(n, unit.into()),
        })
    }

    /// Returns the number of elapsed settlement units as a JS `bigint`.
    #[wasm_bindgen(getter)]
    pub fn n(&self) -> u64 {
        self.inner.n()
    }

    /// Returns the unit used to measure the delay.
    #[wasm_bindgen(getter)]
    pub fn unit(&self) -> JsSettlementUnit {
        self.inner.unit().into()
    }

    /// Returns `true` when both delays have identical values.
    #[wasm_bindgen(js_name = equals)]
    pub fn equals(&self, other: &Self) -> bool {
        self.inner == other.inner
    }

    /// Returns an independent copy of this settlement delay.
    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> Self {
        *self
    }
}

impl JsSettlementLag {
    fn from_inner(inner: SettlementLag) -> Self {
        Self { inner }
    }

    fn inner(&self) -> SettlementLag {
        self.inner
    }
}

/// Independent delivery and payment settlement delays.
#[wasm_bindgen(js_name = SettlementScheme)]
#[derive(Clone, Copy)]
pub struct JsSettlementScheme {
    inner: SettlementScheme,
}

#[wasm_bindgen(js_class = SettlementScheme)]
impl JsSettlementScheme {
    /// Creates a scheme with independently configured delivery and payment legs.
    #[wasm_bindgen(constructor)]
    pub fn new(delivery: &JsSettlementLag, payment: &JsSettlementLag) -> Self {
        Self {
            inner: SettlementScheme::new(delivery.inner(), payment.inner()),
        }
    }

    /// Creates a scheme with both legs settling after `n` business days.
    #[wasm_bindgen(js_name = uniform)]
    pub fn uniform(n: BigIntLike) -> Result<Self, JsValue> {
        let n = parse_u64_bigint(n.into(), "settlement lag")?;
        Ok(Self {
            inner: SettlementScheme::uniform(n),
        })
    }

    /// Returns the delivery-leg settlement delay.
    #[wasm_bindgen(getter)]
    pub fn delivery(&self) -> JsSettlementLag {
        JsSettlementLag::from_inner(self.inner.delivery())
    }

    /// Returns the payment-leg settlement delay.
    #[wasm_bindgen(getter)]
    pub fn payment(&self) -> JsSettlementLag {
        JsSettlementLag::from_inner(self.inner.payment())
    }

    /// Returns `true` when both schemes have identical settlement legs.
    #[wasm_bindgen(js_name = equals)]
    pub fn equals(&self, other: &Self) -> bool {
        self.inner == other.inner
    }

    /// Returns an independent copy of this settlement scheme.
    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> Self {
        *self
    }
}

impl JsSettlementScheme {
    fn from_inner(inner: SettlementScheme) -> Self {
        Self { inner }
    }

    fn inner(&self) -> SettlementScheme {
        self.inner
    }
}

/// Caller-owned registry of stable instrument identities and attributes.
#[wasm_bindgen(js_name = ReferenceBook)]
pub struct JsReferenceBook {
    inner: ReferenceBook,
}

impl Default for JsReferenceBook {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen(js_class = ReferenceBook)]
impl JsReferenceBook {
    /// Creates an empty reference book.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: ReferenceBook::new(),
        }
    }

    /// Registers `instrument` under the next available instrument id.
    #[wasm_bindgen(js_name = register)]
    pub fn register(&mut self, instrument: InstrumentLike) -> Result<JsInstrumentId, JsValue> {
        let instrument = JsInstrument::coerce(instrument.into())?;
        self.inner
            .register(instrument)
            .map(JsInstrumentId::from_inner)
            .map_err(reference_book_registration_error_to_js)
    }

    /// Registers `instrument` under a caller-selected stable identifier.
    #[wasm_bindgen(js_name = registerWithId)]
    pub fn register_with_id(
        &mut self,
        instrument: InstrumentLike,
        instrument_id: InstrumentIdLike,
    ) -> Result<JsInstrumentId, JsValue> {
        let instrument = JsInstrument::coerce(instrument.into())?;
        let instrument_id = resolve_instrument_id(instrument_id.into())?;
        self.inner
            .register_with_id(instrument, instrument_id)
            .map(JsInstrumentId::from_inner)
            .map_err(reference_book_registration_error_to_js)
    }

    /// Resolves `instrument` to a registered id, or returns `undefined`.
    #[wasm_bindgen(js_name = resolve)]
    pub fn resolve(&self, instrument: InstrumentLike) -> Result<Option<JsInstrumentId>, JsValue> {
        let instrument = JsInstrument::coerce(instrument.into())?;
        Ok(self
            .inner
            .resolve(&instrument)
            .map(JsInstrumentId::from_inner))
    }

    /// Assigns a typed settlement scheme to a registered instrument.
    #[wasm_bindgen(js_name = setSettlementScheme)]
    pub fn set_settlement_scheme(
        &mut self,
        instrument_id: InstrumentIdLike,
        settlement_scheme: &JsSettlementScheme,
    ) -> Result<(), JsValue> {
        let instrument_id = resolve_instrument_id(instrument_id.into())?;
        self.inner
            .set_settlement_scheme(instrument_id, settlement_scheme.inner())
            .map_err(unknown_reference_book_instrument_id_to_js)
    }

    /// Clears a registered instrument's settlement scheme.
    #[wasm_bindgen(js_name = clearSettlementScheme)]
    pub fn clear_settlement_scheme(
        &mut self,
        instrument_id: InstrumentIdLike,
    ) -> Result<(), JsValue> {
        let instrument_id = resolve_instrument_id(instrument_id.into())?;
        self.inner
            .clear_settlement_scheme(instrument_id)
            .map_err(unknown_reference_book_instrument_id_to_js)
    }

    /// Returns a registered instrument's scheme, or `undefined` when unset.
    #[wasm_bindgen(js_name = settlementScheme)]
    pub fn settlement_scheme(
        &self,
        instrument_id: InstrumentIdLike,
    ) -> Result<Option<JsSettlementScheme>, JsValue> {
        let instrument_id = resolve_instrument_id(instrument_id.into())?;
        self.inner
            .settlement_scheme(instrument_id)
            .map(|scheme| scheme.map(JsSettlementScheme::from_inner))
            .map_err(unknown_reference_book_instrument_id_to_js)
    }
}

fn reference_book_registration_error_to_js(error: ReferenceBookRegistrationError) -> JsValue {
    let message = error.to_string();
    let payload = Object::new();
    let kind = match error {
        ReferenceBookRegistrationError::DuplicateId { instrument_id } => {
            let _ = Reflect::set(
                &payload,
                &JsValue::from_str("instrumentId"),
                &JsInstrumentId::from_inner(instrument_id).into(),
            );
            "DuplicateId"
        }
        ReferenceBookRegistrationError::DuplicateInstrument { instrument } => {
            let _ = Reflect::set(
                &payload,
                &JsValue::from_str("instrument"),
                &JsInstrument::from_inner(instrument).into(),
            );
            "DuplicateInstrument"
        }
        _ => "Unknown",
    };
    let _ = Reflect::set(
        &payload,
        &JsValue::from_str("kind"),
        &JsValue::from_str(kind),
    );
    make_error_with(
        ErrorKind::ReferenceBookRegistration,
        &message,
        Some(kind),
        payload.into(),
        JsValue::UNDEFINED,
    )
}

fn unknown_reference_book_instrument_id_to_js(error: UnknownReferenceBookInstrumentId) -> JsValue {
    let payload = Object::new();
    let _ = Reflect::set(
        &payload,
        &JsValue::from_str("instrumentId"),
        &JsInstrumentId::from_inner(error.instrument_id).into(),
    );
    make_error_with(
        ErrorKind::UnknownReferenceBookInstrumentId,
        &error.to_string(),
        None,
        payload.into(),
        JsValue::UNDEFINED,
    )
}
