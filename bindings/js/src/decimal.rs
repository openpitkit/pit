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

//! Decimal boundary parsing for the JS binding.
//!
//! `DecimalInput = string | number | bigint`. A decimal crosses the boundary
//! as one of those JS types and is normalized to a canonical decimal `String`
//! that the value-type factories feed into the core `from_str`/`from_f64`
//! constructors. Strings and `bigint` are lossless. The generic `number` form
//! is accepted only for safe integers; callers that intentionally need an
//! imprecise fractional float must use the explicit `fromFloat` factory.
//!
//! Rounding strategy parsing maps the public string aliases onto
//! [`openpit::param::RoundingStrategy`].

use js_sys::BigInt;
use openpit::param::RoundingStrategy;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use crate::error::{make_error, ErrorKind};

/// A value passed to a numeric factory: `string`, `number`, or `bigint`.
///
/// The recommended form is a decimal `string` ("100.50"); a `number` must be
/// a safe integer, while `bigint` is exact for larger integer magnitudes.
/// Intentional imprecise fractional input uses the explicit `fromFloat`
/// factories rather than this generic boundary.
pub enum DecimalInput {
    /// A canonical decimal string parsed losslessly via `from_str`.
    Text(String),
}

/// Largest IEEE-754 double that represents an integer exactly
/// (`Number.MAX_SAFE_INTEGER`, `2^53 - 1`).
const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;

/// Classifies a `JsValue` argument as one of the `DecimalInput` forms.
///
/// `string` -> `DecimalInput::Text` (lossless), `bigint` -> a decimal string
/// (exact). A `number` that is an integer within the safe-integer range is
/// routed to `DecimalInput::Text` as well (the value is exact, matching the
/// documented promise that integers are presented exactly). Fractional,
/// non-finite, and unsafe `number` values are rejected so an overloaded generic
/// boundary never silently selects an imprecise conversion path. Any other JS
/// type is a native `TypeError`.
pub fn parse_decimal_input(value: &JsValue) -> Result<DecimalInput, JsValue> {
    if let Some(text) = value.as_string() {
        return Ok(DecimalInput::Text(text));
    }
    if value.is_bigint() {
        let bigint: BigInt = value.clone().unchecked_into();
        let text = bigint_to_decimal_string(&bigint)?;
        return Ok(DecimalInput::Text(text));
    }
    if let Some(number) = value.as_f64() {
        // A safe-integer-valued double is exact; render it through the lossless
        // string path so the boundary never drops to the imprecise float route
        // for values JS itself represents exactly.
        if number.fract() == 0.0 && number.abs() <= MAX_SAFE_INTEGER {
            return Ok(DecimalInput::Text(format!("{}", number as i64)));
        }
        return Err(make_error(
            ErrorKind::Range,
            "decimal number input must be a finite safe integer; use a decimal string or the explicit fromFloat factory",
            None,
        ));
    }
    Err(invalid_decimal_input())
}

/// Converts a JS `BigInt` to its base-10 string form for exact `from_str`.
fn bigint_to_decimal_string(bigint: &BigInt) -> Result<String, JsValue> {
    // `BigInt.prototype.toString(10)` is infallible for radix 10; the JS string
    // is then a valid decimal integer literal for `Decimal::from_str`.
    let radix = 10_u8;
    match bigint.to_string(radix) {
        Ok(js_string) => Ok(String::from(js_string)),
        Err(_) => Err(invalid_decimal_input()),
    }
}

/// Builds the `ParamError` raised for an unsupported `DecimalInput` JS type.
fn invalid_decimal_input() -> JsValue {
    make_error(
        ErrorKind::Type,
        "decimal input must be a string, number, or bigint",
        None,
    )
}

/// Parses a public rounding-strategy string into the core strategy.
///
/// Accepted aliases mirror the SDK: `"default"`/`"banker"` and the explicit
/// `"midpointNearestEven"` map to [`RoundingStrategy::MidpointNearestEven`];
/// `"conservativeProfit"`/`"conservativeLoss"` and `"down"` map to
/// [`RoundingStrategy::Down`]; `"midpointAwayFromZero"` and `"up"` map to their
/// namesakes. Comparison is case-insensitive. An unknown value throws a native
/// `RangeError`.
pub fn parse_rounding_strategy(strategy: &str) -> Result<RoundingStrategy, JsValue> {
    match strategy.to_ascii_lowercase().as_str() {
        "default" | "banker" | "midpointnearesteven" => Ok(RoundingStrategy::MidpointNearestEven),
        "conservativeprofit" | "conservativeloss" | "down" => Ok(RoundingStrategy::Down),
        "midpointawayfromzero" => Ok(RoundingStrategy::MidpointAwayFromZero),
        "up" => Ok(RoundingStrategy::Up),
        _ => Err(make_error(
            ErrorKind::Range,
            "unknown rounding strategy",
            None,
        )),
    }
}
