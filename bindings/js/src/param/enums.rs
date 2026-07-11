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

//! Trading enums exposed to JavaScript.
//!
//! The boundary contract is string-valued: every enum round-trips through its
//! canonical wire string (`Side` -> `"BUY"|"SELL"`, `PositionSide` ->
//! `"LONG"|"SHORT"`, `PositionEffect` -> `"OPEN"|"CLOSE"`, `PositionMode` ->
//! lowercase `"netting"|"hedged"`). The exported wrappers carry the domain
//! methods (`isBuy`, `opposite`, `sign`, ...) on top of that string contract,
//! rather than numeric `wasm-bindgen` enums, so the public TypeScript surface
//! stays a string union.

use openpit::param::{PositionEffect, PositionMode, PositionSide, Side};
use wasm_bindgen::prelude::*;

use crate::error::{make_error, ErrorKind};

/// Builds the `RangeError` raised when an enum string is outside its value set.
fn invalid_enum(message: &str) -> JsValue {
    make_error(ErrorKind::Range, message, None)
}

/// Parses a `Side` wire string (`"BUY"`/`"SELL"`, case-insensitive).
pub fn parse_side(value: &str) -> Result<Side, JsValue> {
    match value.to_ascii_uppercase().as_str() {
        "BUY" => Ok(Side::Buy),
        "SELL" => Ok(Side::Sell),
        _ => Err(invalid_enum("side must be \"BUY\" or \"SELL\"")),
    }
}

/// Parses a `PositionSide` wire string (`"LONG"`/`"SHORT"`).
pub fn parse_position_side(value: &str) -> Result<PositionSide, JsValue> {
    match value.to_ascii_uppercase().as_str() {
        "LONG" => Ok(PositionSide::Long),
        "SHORT" => Ok(PositionSide::Short),
        _ => Err(invalid_enum("position side must be \"LONG\" or \"SHORT\"")),
    }
}

/// Parses a `PositionEffect` wire string (`"OPEN"`/`"CLOSE"`).
pub fn parse_position_effect(value: &str) -> Result<PositionEffect, JsValue> {
    match value.to_ascii_uppercase().as_str() {
        "OPEN" => Ok(PositionEffect::Open),
        "CLOSE" => Ok(PositionEffect::Close),
        _ => Err(invalid_enum(
            "position effect must be \"OPEN\" or \"CLOSE\"",
        )),
    }
}

/// Parses a `PositionMode` wire string (lowercase `"netting"`/`"hedged"`).
pub fn parse_position_mode(value: &str) -> Result<PositionMode, JsValue> {
    match value.to_ascii_lowercase().as_str() {
        "netting" => Ok(PositionMode::Netting),
        "hedged" => Ok(PositionMode::Hedged),
        _ => Err(invalid_enum(
            "position mode must be \"netting\" or \"hedged\"",
        )),
    }
}

/// Resolves an optional `Side` wire string; `undefined`/`null` maps to `None`.
///
/// # Errors
///
/// Throws `TypeError` for a non-string or `RangeError` for an unknown value.
pub fn resolve_optional_side(value: JsValue) -> Result<Option<Side>, JsValue> {
    match optional_enum_string(&value, "side")? {
        Some(text) => parse_side(&text).map(Some),
        None => Ok(None),
    }
}

/// Resolves an optional `PositionSide` wire string; `undefined`/`null` maps to
/// `None`.
///
/// # Errors
///
/// Throws `TypeError` for a non-string or `RangeError` for an unknown value.
pub fn resolve_optional_position_side(value: JsValue) -> Result<Option<PositionSide>, JsValue> {
    match optional_enum_string(&value, "position side")? {
        Some(text) => parse_position_side(&text).map(Some),
        None => Ok(None),
    }
}

/// Resolves an optional `PositionEffect` wire string; `undefined`/`null` maps
/// to `None`.
///
/// # Errors
///
/// Throws `TypeError` for a non-string or `RangeError` for an unknown value.
pub fn resolve_optional_position_effect(value: JsValue) -> Result<Option<PositionEffect>, JsValue> {
    match optional_enum_string(&value, "position effect")? {
        Some(text) => parse_position_effect(&text).map(Some),
        None => Ok(None),
    }
}

/// Resolves an optional `PositionMode` wire string; `undefined`/`null` maps to
/// `None`.
///
/// # Errors
///
/// Throws `TypeError` for a non-string or `RangeError` for an unknown value.
pub fn resolve_optional_position_mode(value: JsValue) -> Result<Option<PositionMode>, JsValue> {
    match optional_enum_string(&value, "position mode")? {
        Some(text) => parse_position_mode(&text).map(Some),
        None => Ok(None),
    }
}

/// Reads an optional enum wire string; `undefined`/`null` maps to `None`.
///
/// # Errors
///
/// Throws `TypeError` when the present value is not a string.
fn optional_enum_string(value: &JsValue, label: &str) -> Result<Option<String>, JsValue> {
    if value.is_undefined() || value.is_null() {
        return Ok(None);
    }
    match value.as_string() {
        Some(text) => Ok(Some(text)),
        None => Err(make_error(
            ErrorKind::Type,
            &format!("{label} must be a string"),
            None,
        )),
    }
}

/// Trading side: buy or sell.
#[wasm_bindgen(js_name = Side)]
#[derive(Clone, Copy)]
pub struct JsSide {
    inner: Side,
}

#[wasm_bindgen(js_class = Side)]
impl JsSide {
    /// Parses a side from its wire string (`"BUY"`/`"SELL"`).
    #[wasm_bindgen(js_name = fromString)]
    pub fn from_string(value: &str) -> Result<JsSide, JsValue> {
        Ok(Self {
            inner: parse_side(value)?,
        })
    }

    /// The buy side.
    #[wasm_bindgen(js_name = buy)]
    pub fn buy() -> JsSide {
        Self { inner: Side::Buy }
    }

    /// The sell side.
    #[wasm_bindgen(js_name = sell)]
    pub fn sell() -> JsSide {
        Self { inner: Side::Sell }
    }

    /// Returns `true` for the buy side.
    #[wasm_bindgen(js_name = isBuy)]
    pub fn is_buy(&self) -> bool {
        self.inner.is_buy()
    }

    /// Returns `true` for the sell side.
    #[wasm_bindgen(js_name = isSell)]
    pub fn is_sell(&self) -> bool {
        self.inner.is_sell()
    }

    /// Returns the opposite side.
    #[wasm_bindgen(js_name = opposite)]
    pub fn opposite(&self) -> JsSide {
        Self {
            inner: self.inner.opposite(),
        }
    }

    /// Returns the signed direction: `+1` for buy, `-1` for sell.
    #[wasm_bindgen(js_name = sign)]
    pub fn sign(&self) -> i32 {
        i32::from(self.inner.sign())
    }

    /// Returns the canonical wire string (`"BUY"`/`"SELL"`).
    #[wasm_bindgen(js_name = toString)]
    pub fn to_js_string(&self) -> String {
        self.inner.to_string()
    }

    /// Returns the wire string for JSON serialization.
    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> String {
        self.inner.to_string()
    }
}

impl JsSide {
    /// Returns the wrapped core [`Side`].
    pub fn inner(&self) -> Side {
        self.inner
    }
}

/// Position direction: long or short.
#[wasm_bindgen(js_name = PositionSide)]
#[derive(Clone, Copy)]
pub struct JsPositionSide {
    inner: PositionSide,
}

#[wasm_bindgen(js_class = PositionSide)]
impl JsPositionSide {
    /// Parses a position side from its wire string (`"LONG"`/`"SHORT"`).
    #[wasm_bindgen(js_name = fromString)]
    pub fn from_string(value: &str) -> Result<JsPositionSide, JsValue> {
        Ok(Self {
            inner: parse_position_side(value)?,
        })
    }

    /// The long side.
    #[wasm_bindgen(js_name = long)]
    pub fn long() -> JsPositionSide {
        Self {
            inner: PositionSide::Long,
        }
    }

    /// The short side.
    #[wasm_bindgen(js_name = short)]
    pub fn short() -> JsPositionSide {
        Self {
            inner: PositionSide::Short,
        }
    }

    /// Returns `true` for the long side.
    #[wasm_bindgen(js_name = isLong)]
    pub fn is_long(&self) -> bool {
        self.inner.is_long()
    }

    /// Returns `true` for the short side.
    #[wasm_bindgen(js_name = isShort)]
    pub fn is_short(&self) -> bool {
        self.inner.is_short()
    }

    /// Returns the opposite position side.
    #[wasm_bindgen(js_name = opposite)]
    pub fn opposite(&self) -> JsPositionSide {
        Self {
            inner: self.inner.opposite(),
        }
    }

    /// Returns the canonical wire string (`"LONG"`/`"SHORT"`).
    #[wasm_bindgen(js_name = toString)]
    pub fn to_js_string(&self) -> String {
        self.inner.to_string()
    }

    /// Returns the wire string for JSON serialization.
    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> String {
        self.inner.to_string()
    }
}

impl JsPositionSide {
    /// Returns the wrapped core [`PositionSide`].
    pub fn inner(&self) -> PositionSide {
        self.inner
    }
}

/// Position effect: opening or closing exposure.
#[wasm_bindgen(js_name = PositionEffect)]
#[derive(Clone, Copy)]
pub struct JsPositionEffect {
    inner: PositionEffect,
}

#[wasm_bindgen(js_class = PositionEffect)]
impl JsPositionEffect {
    /// Parses a position effect from its wire string (`"OPEN"`/`"CLOSE"`).
    #[wasm_bindgen(js_name = fromString)]
    pub fn from_string(value: &str) -> Result<JsPositionEffect, JsValue> {
        Ok(Self {
            inner: parse_position_effect(value)?,
        })
    }

    /// The opening effect.
    #[wasm_bindgen(js_name = open)]
    pub fn open() -> JsPositionEffect {
        Self {
            inner: PositionEffect::Open,
        }
    }

    /// The closing effect.
    #[wasm_bindgen(js_name = close)]
    pub fn close() -> JsPositionEffect {
        Self {
            inner: PositionEffect::Close,
        }
    }

    /// Returns the canonical wire string (`"OPEN"`/`"CLOSE"`).
    #[wasm_bindgen(js_name = toString)]
    pub fn to_js_string(&self) -> String {
        self.inner.to_string()
    }

    /// Returns the wire string for JSON serialization.
    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> String {
        self.inner.to_string()
    }
}

impl JsPositionEffect {
    /// Returns the wrapped core [`PositionEffect`].
    pub fn inner(&self) -> PositionEffect {
        self.inner
    }
}

/// Position bookkeeping mode: netting or hedged.
#[wasm_bindgen(js_name = PositionMode)]
#[derive(Clone, Copy)]
pub struct JsPositionMode {
    inner: PositionMode,
}

#[wasm_bindgen(js_class = PositionMode)]
impl JsPositionMode {
    /// Parses a position mode from its wire string (`"netting"`/`"hedged"`).
    #[wasm_bindgen(js_name = fromString)]
    pub fn from_string(value: &str) -> Result<JsPositionMode, JsValue> {
        Ok(Self {
            inner: parse_position_mode(value)?,
        })
    }

    /// The netting mode.
    #[wasm_bindgen(js_name = netting)]
    pub fn netting() -> JsPositionMode {
        Self {
            inner: PositionMode::Netting,
        }
    }

    /// The hedged mode.
    #[wasm_bindgen(js_name = hedged)]
    pub fn hedged() -> JsPositionMode {
        Self {
            inner: PositionMode::Hedged,
        }
    }

    /// Returns the canonical wire string (lowercase `"netting"`/`"hedged"`).
    #[wasm_bindgen(js_name = toString)]
    pub fn to_js_string(&self) -> String {
        self.inner.to_string()
    }

    /// Returns the wire string for JSON serialization.
    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> String {
        self.inner.to_string()
    }
}

impl JsPositionMode {
    /// Returns the wrapped core [`PositionMode`].
    pub fn inner(&self) -> PositionMode {
        self.inner
    }
}
