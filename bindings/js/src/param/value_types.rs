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

//! Decimal value-type bindings (`Price`, `Quantity`, `Pnl`, ...).
//!
//! The [`define_decimal_value_type!`] macro generates one consistent decimal
//! surface for each domain type. Decimals cross the boundary as canonical
//! strings; every exported method carries an explicit camelCase `js_name`.
//!
//! Scalar arithmetic accepts `number | bigint`: a `bigint` routes through the
//! exact `checked_mul_i64`/`checked_div_i64`/`checked_rem_i64` paths, while a
//! `number` routes through the imprecise `checked_*_f64` paths. This is the
//! single sanctioned float boundary; all stored state remains exact decimal.

use openpit::param::{CashFlow, Fee, Notional, Pnl, PositionSize, Price, Quantity, Side, Volume};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use super::enums::parse_side;
use crate::decimal::{parse_decimal_input, parse_rounding_strategy, DecimalInput};
use crate::domain::{parse_bounded_number, BigIntLike, IntegerNumber};
use crate::error::{make_error, param_error_to_js, ErrorKind};

#[wasm_bindgen]
extern "C" {
    /// A decimal input: a decimal `string`, a `number`, or a `bigint`.
    ///
    /// Equivalent to the exported `DecimalInput` alias; spelled out inline so
    /// the generated `.d.ts` resolves it without a cross-module import.
    #[wasm_bindgen(typescript_type = "string | number | bigint")]
    pub type DecimalInputLike;

    /// A scalar multiplier/divisor: a `number` (imprecise) or a `bigint`
    /// (exact).
    #[wasm_bindgen(typescript_type = "number | bigint")]
    pub type ScalarLike;
}

/// A resolved scalar multiplier/divisor for value-type arithmetic.
enum Scalar {
    /// Exact integer scalar (from a JS `bigint`).
    Int(i64),
    /// Imprecise float scalar (from a JS `number`).
    Float(f64),
}

/// Parses a `number | bigint` scalar argument.
///
/// A `bigint` outside the `i64` range is a `RangeError`; any other
/// non-numeric value is a `TypeError`.
fn parse_scalar(value: &JsValue) -> Result<Scalar, JsValue> {
    if value.is_bigint() {
        let bigint: js_sys::BigInt = value.clone().unchecked_into();
        // i64 covers the exact-multiplier use case; wider magnitudes are
        // rejected rather than silently truncated.
        return match i64::try_from(bigint) {
            Ok(scalar) => Ok(Scalar::Int(scalar)),
            Err(_) => Err(make_error(
                ErrorKind::Range,
                "scalar bigint is out of the supported i64 range",
                None,
            )),
        };
    }
    if let Some(number) = value.as_f64() {
        return Ok(Scalar::Float(number));
    }
    Err(make_error(
        ErrorKind::Type,
        "scalar must be a number or bigint",
        None,
    ))
}

/// Generates the shared decimal surface for one domain value type.
///
/// `$signed` is `signed` for types exposing `neg` (and `checked_neg`) or
/// `unsigned` for non-negative types. The generated wrapper holds the core
/// value type directly and exposes camelCase factories, arithmetic, comparison,
/// and string serialization.
macro_rules! define_decimal_value_type {
    ($js_type:ident, $core_type:ty, $js_name:literal, signed) => {
        define_decimal_value_type!(@common $js_type, $core_type, $js_name);

        #[wasm_bindgen(js_class = $js_name)]
        impl $js_type {
            /// Returns the negation of this value.
            ///
            /// # Errors
            ///
            /// Throws `ParamError` on arithmetic overflow.
            #[wasm_bindgen(js_name = neg)]
            pub fn neg(&self) -> Result<$js_type, JsValue> {
                self.inner
                    .checked_neg()
                    .map(Self::from_inner)
                    .map_err(param_error_to_js)
            }
        }
    };
    ($js_type:ident, $core_type:ty, $js_name:literal, unsigned) => {
        define_decimal_value_type!(@common $js_type, $core_type, $js_name);
    };
    (@common $js_type:ident, $core_type:ty, $js_name:literal) => {
        #[doc = concat!("Decimal value type `", $js_name, "`.")]
        #[wasm_bindgen(js_name = $js_name)]
        #[derive(Clone, Copy)]
        pub struct $js_type {
            inner: $core_type,
        }

        #[wasm_bindgen(js_class = $js_name)]
        impl $js_type {
            /// Constructs from a `DecimalInput` (`string | number | bigint`).
            ///
            /// A generic `number` must be a safe integer. Use `fromFloat`
            /// explicitly for imprecise fractional input.
            ///
            /// # Errors
            ///
            /// Throws `ParamError` on an invalid value or type.
            #[wasm_bindgen(constructor)]
            pub fn new(value: DecimalInputLike) -> Result<$js_type, JsValue> {
                let value: JsValue = value.into();
                match parse_decimal_input(&value)? {
                    DecimalInput::Text(text) => Self::from_string(&text),
                }
            }

            /// Constructs losslessly from a decimal string.
            ///
            /// # Errors
            ///
            /// Throws `ParamError` on an invalid format or out-of-domain value.
            #[wasm_bindgen(js_name = fromString)]
            pub fn from_string(value: &str) -> Result<$js_type, JsValue> {
                <$core_type>::from_str(value)
                    .map(Self::from_inner)
                    .map_err(param_error_to_js)
            }

            /// Constructs from an exact integer (`bigint`).
            ///
            /// # Errors
            ///
            /// Throws `ParamError` on an out-of-domain value.
            #[wasm_bindgen(js_name = fromInt)]
            pub fn from_int(value: BigIntLike) -> Result<$js_type, JsValue> {
                let value: JsValue = value.into();
                if !value.is_bigint() {
                    return Err(make_error(
                        ErrorKind::Type,
                        concat!($js_name, ".fromInt value must be a bigint"),
                        None,
                    ));
                }
                let DecimalInput::Text(value) = parse_decimal_input(&value)?;
                <$core_type>::from_str(&value)
                    .map(Self::from_inner)
                    .map_err(param_error_to_js)
            }

            /// Constructs from a JS `number` (imprecise; prefer `fromString`).
            ///
            /// # Errors
            ///
            /// Throws `ParamError` on a non-finite or out-of-domain value.
            #[wasm_bindgen(js_name = fromFloat)]
            pub fn from_float(value: f64) -> Result<$js_type, JsValue> {
                <$core_type>::from_f64(value)
                    .map(Self::from_inner)
                    .map_err(param_error_to_js)
            }

            /// Parses a decimal string then quantizes to `scale` using
            /// `strategy`.
            ///
            /// # Errors
            ///
            /// Throws `ParamError` on an invalid value, scale, or strategy.
            #[wasm_bindgen(js_name = fromStringRounded)]
            pub fn from_string_rounded(
                value: &str,
                scale: IntegerNumber,
                strategy: &str,
            ) -> Result<$js_type, JsValue> {
                let scale = parse_bounded_number(
                    scale.into(),
                    u64::from(u32::MAX),
                    "scale",
                )? as u32;
                let strategy = parse_rounding_strategy(strategy)?;
                <$core_type>::from_str_rounded(value, scale, strategy)
                    .map(Self::from_inner)
                    .map_err(param_error_to_js)
            }

            /// Constructs from a JS `number` then quantizes to `scale`.
            ///
            /// WARNING: the float source is imprecise; prefer
            /// `fromStringRounded`.
            ///
            /// # Errors
            ///
            /// Throws `ParamError` on an invalid value, scale, or strategy.
            #[wasm_bindgen(js_name = fromFloatRounded)]
            pub fn from_float_rounded(
                value: f64,
                scale: IntegerNumber,
                strategy: &str,
            ) -> Result<$js_type, JsValue> {
                let scale = parse_bounded_number(
                    scale.into(),
                    u64::from(u32::MAX),
                    "scale",
                )? as u32;
                let strategy = parse_rounding_strategy(strategy)?;
                <$core_type>::from_f64_rounded(value, scale, strategy)
                    .map(Self::from_inner)
                    .map_err(param_error_to_js)
            }

            /// Returns the zero value.
            #[wasm_bindgen(js_name = zero)]
            pub fn zero() -> $js_type {
                Self::from_inner(<$core_type>::ZERO)
            }

            /// Returns `true` when the value is exactly zero.
            #[wasm_bindgen(js_name = isZero)]
            pub fn is_zero(&self) -> bool {
                self.inner.is_zero()
            }

            /// Returns the sum of `self` and `other`.
            ///
            /// # Errors
            ///
            /// Throws `ParamError` on overflow (or sign violation for unsigned
            /// types).
            #[wasm_bindgen(js_name = add)]
            pub fn add(&self, other: &$js_type) -> Result<$js_type, JsValue> {
                self.inner
                    .checked_add(other.inner)
                    .map(Self::from_inner)
                    .map_err(param_error_to_js)
            }

            /// Returns `self` minus `other`.
            ///
            /// # Errors
            ///
            /// Throws `ParamError` on overflow/underflow.
            #[wasm_bindgen(js_name = sub)]
            pub fn sub(&self, other: &$js_type) -> Result<$js_type, JsValue> {
                self.inner
                    .checked_sub(other.inner)
                    .map(Self::from_inner)
                    .map_err(param_error_to_js)
            }

            /// Multiplies by a `number | bigint` scalar.
            ///
            /// `bigint` is exact; `number` is imprecise.
            ///
            /// # Errors
            ///
            /// Throws `ParamError` on overflow or an invalid scalar.
            #[wasm_bindgen(js_name = mul)]
            pub fn mul(&self, scalar: ScalarLike) -> Result<$js_type, JsValue> {
                match parse_scalar(&scalar.into())? {
                    Scalar::Int(value) => self.inner.checked_mul_i64(value),
                    Scalar::Float(value) => self.inner.checked_mul_f64(value),
                }
                .map(Self::from_inner)
                .map_err(param_error_to_js)
            }

            /// Divides by a `number | bigint` scalar.
            ///
            /// # Errors
            ///
            /// Throws `ParamError` on division by zero, overflow, or an invalid
            /// scalar.
            #[wasm_bindgen(js_name = div)]
            pub fn div(&self, scalar: ScalarLike) -> Result<$js_type, JsValue> {
                match parse_scalar(&scalar.into())? {
                    Scalar::Int(value) => self.inner.checked_div_i64(value),
                    Scalar::Float(value) => self.inner.checked_div_f64(value),
                }
                .map(Self::from_inner)
                .map_err(param_error_to_js)
            }

            /// Computes the remainder against a `number | bigint` scalar.
            ///
            /// # Errors
            ///
            /// Throws `ParamError` on division by zero, overflow, or an invalid
            /// scalar.
            #[wasm_bindgen(js_name = mod)]
            pub fn rem(&self, scalar: ScalarLike) -> Result<$js_type, JsValue> {
                match parse_scalar(&scalar.into())? {
                    Scalar::Int(value) => self.inner.checked_rem_i64(value),
                    Scalar::Float(value) => self.inner.checked_rem_f64(value),
                }
                .map(Self::from_inner)
                .map_err(param_error_to_js)
            }

            /// Returns `true` when both values are equal.
            #[wasm_bindgen(js_name = equals)]
            pub fn equals(&self, other: &$js_type) -> bool {
                self.inner == other.inner
            }

            /// Compares against `other`: `-1`, `0`, or `1`.
            #[wasm_bindgen(js_name = compare)]
            pub fn compare(&self, other: &$js_type) -> i32 {
                match self.inner.cmp(&other.inner) {
                    core::cmp::Ordering::Less => -1,
                    core::cmp::Ordering::Equal => 0,
                    core::cmp::Ordering::Greater => 1,
                }
            }

            /// Returns the canonical decimal string.
            #[wasm_bindgen(js_name = toString)]
            pub fn to_js_string(&self) -> String {
                self.inner.to_string()
            }

            /// Returns the canonical decimal string for JSON serialization.
            #[wasm_bindgen(js_name = toJSON)]
            pub fn to_json(&self) -> String {
                self.inner.to_string()
            }

            /// Returns a fresh wrapper holding the same value.
            ///
            /// Value types are cheap to copy. Ordinary setters already
            /// borrow/clone wrapper inputs; this method is a convenience when
            /// an explicit second wrapper object is useful.
            #[wasm_bindgen(js_name = clone)]
            pub fn js_clone(&self) -> $js_type {
                *self
            }
        }

        impl $js_type {
            /// Wraps a core value.
            pub fn from_inner(inner: $core_type) -> Self {
                Self { inner }
            }

            /// Returns the wrapped core value.
            pub fn inner(&self) -> $core_type {
                self.inner
            }
        }
    };
}

define_decimal_value_type!(JsPrice, Price, "Price", signed);
define_decimal_value_type!(JsQuantity, Quantity, "Quantity", unsigned);
define_decimal_value_type!(JsPnl, Pnl, "Pnl", signed);
define_decimal_value_type!(JsFee, Fee, "Fee", signed);
define_decimal_value_type!(JsVolume, Volume, "Volume", unsigned);
define_decimal_value_type!(JsNotional, Notional, "Notional", unsigned);
define_decimal_value_type!(JsCashFlow, CashFlow, "CashFlow", signed);
define_decimal_value_type!(JsPositionSize, PositionSize, "PositionSize", signed);

// Cross-type converters expose the corresponding core operations.

#[wasm_bindgen(js_class = Price)]
impl JsPrice {
    /// Computes the signed volume as `price * quantity`.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` on arithmetic overflow.
    #[wasm_bindgen(js_name = calculateVolume)]
    pub fn calculate_volume(&self, quantity: &JsQuantity) -> Result<JsVolume, JsValue> {
        self.inner()
            .calculate_volume(quantity.inner())
            .map(JsVolume::from_inner)
            .map_err(param_error_to_js)
    }

    /// Computes the signed position size as `price * quantity`.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` on arithmetic overflow.
    #[wasm_bindgen(js_name = calculatePositionSize)]
    pub fn calculate_position_size(
        &self,
        quantity: &JsQuantity,
    ) -> Result<JsPositionSize, JsValue> {
        self.inner()
            .calculate_position_size(quantity.inner())
            .map(JsPositionSize::from_inner)
            .map_err(param_error_to_js)
    }
}

#[wasm_bindgen(js_class = Quantity)]
impl JsQuantity {
    /// Computes the volume as `quantity * price`.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` on arithmetic overflow.
    #[wasm_bindgen(js_name = calculateVolume)]
    pub fn calculate_volume(&self, price: &JsPrice) -> Result<JsVolume, JsValue> {
        self.inner()
            .calculate_volume(price.inner())
            .map(JsVolume::from_inner)
            .map_err(param_error_to_js)
    }

    /// Converts this quantity into an unsigned position size.
    #[wasm_bindgen(js_name = toPositionSize)]
    pub fn to_position_size(&self) -> JsPositionSize {
        JsPositionSize::from_inner(self.inner().to_position_size())
    }
}

#[wasm_bindgen(js_class = Pnl)]
impl JsPnl {
    /// Builds a P&L value from a fee.
    #[wasm_bindgen(js_name = fromFee)]
    pub fn from_fee(fee: &JsFee) -> JsPnl {
        JsPnl::from_inner(Pnl::from_fee(fee.inner()))
    }

    /// Converts this P&L into a cash flow.
    #[wasm_bindgen(js_name = toCashFlow)]
    pub fn to_cash_flow(&self) -> JsCashFlow {
        JsCashFlow::from_inner(self.inner().to_cash_flow())
    }

    /// Converts this P&L into a position size.
    #[wasm_bindgen(js_name = toPositionSize)]
    pub fn to_position_size(&self) -> JsPositionSize {
        JsPositionSize::from_inner(self.inner().to_position_size())
    }
}

#[wasm_bindgen(js_class = Fee)]
impl JsFee {
    /// Converts this fee into a P&L value.
    #[wasm_bindgen(js_name = toPnl)]
    pub fn to_pnl(&self) -> JsPnl {
        JsPnl::from_inner(self.inner().to_pnl())
    }

    /// Converts this fee into a position size.
    #[wasm_bindgen(js_name = toPositionSize)]
    pub fn to_position_size(&self) -> JsPositionSize {
        JsPositionSize::from_inner(self.inner().to_position_size())
    }

    /// Converts this fee into a cash flow.
    #[wasm_bindgen(js_name = toCashFlow)]
    pub fn to_cash_flow(&self) -> JsCashFlow {
        JsCashFlow::from_inner(self.inner().to_cash_flow())
    }
}

#[wasm_bindgen(js_class = Volume)]
impl JsVolume {
    /// Converts this volume into an inflow cash flow.
    #[wasm_bindgen(js_name = toCashFlowInflow)]
    pub fn to_cash_flow_inflow(&self) -> JsCashFlow {
        JsCashFlow::from_inner(self.inner().to_cash_flow_inflow())
    }

    /// Converts this volume into an outflow cash flow.
    #[wasm_bindgen(js_name = toCashFlowOutflow)]
    pub fn to_cash_flow_outflow(&self) -> JsCashFlow {
        JsCashFlow::from_inner(self.inner().to_cash_flow_outflow())
    }

    /// Converts this volume into a position size.
    #[wasm_bindgen(js_name = toPositionSize)]
    pub fn to_position_size(&self) -> JsPositionSize {
        JsPositionSize::from_inner(self.inner().to_position_size())
    }

    /// Computes the quantity as `volume / price`.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` on division by zero or overflow.
    #[wasm_bindgen(js_name = calculateQuantity)]
    pub fn calculate_quantity(&self, price: &JsPrice) -> Result<JsQuantity, JsValue> {
        self.inner()
            .calculate_quantity(price.inner())
            .map(JsQuantity::from_inner)
            .map_err(param_error_to_js)
    }
}

#[wasm_bindgen(js_class = Notional)]
impl JsNotional {
    /// Builds a notional from a volume (absolute value).
    #[wasm_bindgen(js_name = fromVolume)]
    pub fn from_volume(volume: &JsVolume) -> JsNotional {
        JsNotional::from_inner(Notional::from_volume(volume.inner()))
    }

    /// Builds a notional from `price * quantity` (absolute value).
    ///
    /// # Errors
    ///
    /// Throws `ParamError` on arithmetic overflow.
    #[wasm_bindgen(js_name = fromPriceQuantity)]
    pub fn from_price_quantity(
        price: &JsPrice,
        quantity: &JsQuantity,
    ) -> Result<JsNotional, JsValue> {
        Notional::from_price_quantity(price.inner(), quantity.inner())
            .map(JsNotional::from_inner)
            .map_err(param_error_to_js)
    }

    /// Converts this notional into a volume.
    #[wasm_bindgen(js_name = toVolume)]
    pub fn to_volume(&self) -> JsVolume {
        JsVolume::from_inner(self.inner().to_volume())
    }

    /// Computes the margin required at the given leverage.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` on arithmetic overflow.
    #[wasm_bindgen(js_name = calculateMarginRequired)]
    pub fn calculate_margin_required(
        &self,
        leverage: &super::leverage::JsLeverage,
    ) -> Result<JsNotional, JsValue> {
        self.inner()
            .calculate_margin_required(leverage.inner())
            .map(JsNotional::from_inner)
            .map_err(param_error_to_js)
    }
}

#[wasm_bindgen(js_class = CashFlow)]
impl JsCashFlow {
    /// Builds a cash flow from a P&L value.
    #[wasm_bindgen(js_name = fromPnl)]
    pub fn from_pnl(pnl: &JsPnl) -> JsCashFlow {
        JsCashFlow::from_inner(CashFlow::from_pnl(pnl.inner()))
    }

    /// Builds a cash flow from a fee.
    #[wasm_bindgen(js_name = fromFee)]
    pub fn from_fee(fee: &JsFee) -> JsCashFlow {
        JsCashFlow::from_inner(CashFlow::from_fee(fee.inner()))
    }

    /// Builds an inflow cash flow from a volume.
    #[wasm_bindgen(js_name = fromVolumeInflow)]
    pub fn from_volume_inflow(volume: &JsVolume) -> JsCashFlow {
        JsCashFlow::from_inner(CashFlow::from_volume_inflow(volume.inner()))
    }

    /// Builds an outflow cash flow from a volume.
    #[wasm_bindgen(js_name = fromVolumeOutflow)]
    pub fn from_volume_outflow(volume: &JsVolume) -> JsCashFlow {
        JsCashFlow::from_inner(CashFlow::from_volume_outflow(volume.inner()))
    }
}

#[wasm_bindgen(js_class = PositionSize)]
impl JsPositionSize {
    /// Builds a signed position size from a quantity and a side.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` when `side` is not `"BUY"`/`"SELL"`.
    #[wasm_bindgen(js_name = fromQuantityAndSide)]
    pub fn from_quantity_and_side(
        quantity: &JsQuantity,
        side: &str,
    ) -> Result<JsPositionSize, JsValue> {
        let side: Side = parse_side(side)?;
        Ok(JsPositionSize::from_inner(
            PositionSize::from_quantity_and_side(quantity.inner(), side),
        ))
    }

    /// Builds a position size from a P&L value.
    #[wasm_bindgen(js_name = fromPnl)]
    pub fn from_pnl(pnl: &JsPnl) -> JsPositionSize {
        JsPositionSize::from_inner(PositionSize::from_pnl(pnl.inner()))
    }

    /// Builds a position size from a fee.
    #[wasm_bindgen(js_name = fromFee)]
    pub fn from_fee(fee: &JsFee) -> JsPositionSize {
        JsPositionSize::from_inner(PositionSize::from_fee(fee.inner()))
    }

    /// Returns the open `[Quantity, "BUY"|"SELL"]` pair as a 2-element array.
    #[wasm_bindgen(
        js_name = toOpenQuantity,
        unchecked_return_type = "[Quantity, \"BUY\" | \"SELL\"]"
    )]
    pub fn to_open_quantity(&self) -> js_sys::Array {
        let (quantity, side) = self.inner().to_open_quantity();
        let array = js_sys::Array::new();
        array.push(&JsValue::from(JsQuantity::from_inner(quantity)));
        array.push(&JsValue::from_str(&side.to_string()));
        array
    }

    /// Returns the close `[Quantity, ("BUY"|"SELL")|undefined]` pair.
    ///
    /// The side is `undefined` when the position is flat.
    #[wasm_bindgen(
        js_name = toCloseQuantity,
        unchecked_return_type = "[Quantity, \"BUY\" | \"SELL\" | undefined]"
    )]
    pub fn to_close_quantity(&self) -> js_sys::Array {
        let (quantity, side) = self.inner().to_close_quantity();
        let array = js_sys::Array::new();
        array.push(&JsValue::from(JsQuantity::from_inner(quantity)));
        match side {
            Some(side) => array.push(&JsValue::from_str(&side.to_string())),
            None => array.push(&JsValue::UNDEFINED),
        };
        array
    }

    /// Adds a quantity on the given side.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` on overflow or when `side` is invalid.
    #[wasm_bindgen(js_name = checkedAddQuantity)]
    pub fn checked_add_quantity(
        &self,
        quantity: &JsQuantity,
        side: &str,
    ) -> Result<JsPositionSize, JsValue> {
        let side: Side = parse_side(side)?;
        self.inner()
            .checked_add_quantity(quantity.inner(), side)
            .map(JsPositionSize::from_inner)
            .map_err(param_error_to_js)
    }
}
