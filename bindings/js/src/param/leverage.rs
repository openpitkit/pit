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

//! `Leverage` binding.
//!
//! Leverage is a fixed-point multiplier (scale 10, range `1..=3000`), not a
//! decimal value type. Its `value` getter reconstructs the multiplier directly
//! from the integer raw value and scale, avoiding an intermediate `f32`
//! approximation before returning the JS `number`.

use openpit::param::Leverage;
use wasm_bindgen::prelude::*;

use super::value_types::JsNotional;
use crate::domain::{parse_bounded_number, IntegerNumber};
use crate::error::param_error_to_js;

/// Fixed-point leverage multiplier (scale 10, range `1.0..=3000.0`, step 0.1).
#[wasm_bindgen(js_name = Leverage)]
#[derive(Clone, Copy)]
pub struct JsLeverage {
    inner: Leverage,
}

#[wasm_bindgen(js_class = Leverage)]
impl JsLeverage {
    /// Builds leverage from an integer multiplier (e.g. `10` for 10x).
    ///
    /// # Errors
    ///
    /// Throws `ParamError` when the multiplier is outside `1..=3000`.
    #[wasm_bindgen(js_name = fromInt)]
    pub fn from_int(value: IntegerNumber) -> Result<JsLeverage, JsValue> {
        let value = parse_bounded_number(value.into(), u64::from(u16::MAX), "leverage")? as u16;
        Self::from_int_raw(value)
    }

    /// Builds leverage from a fractional multiplier (e.g. `2.5`).
    ///
    /// WARNING: floats are imprecise; values are snapped to the scale-10 grid.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` when the value is non-finite or outside the valid
    /// range/step.
    #[wasm_bindgen(js_name = fromFloat)]
    pub fn from_float(value: f64) -> Result<JsLeverage, JsValue> {
        Leverage::from_f64(value)
            .map(|inner| Self { inner })
            .map_err(param_error_to_js)
    }

    /// Returns the normalized multiplier as a JS `number`.
    #[wasm_bindgen(getter)]
    pub fn value(&self) -> f64 {
        f64::from(self.inner.raw()) / f64::from(Leverage::SCALE)
    }

    /// Computes the margin required to support `notional` at this leverage.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` on arithmetic overflow.
    #[wasm_bindgen(js_name = calculateMarginRequired)]
    pub fn calculate_margin_required(&self, notional: &JsNotional) -> Result<JsNotional, JsValue> {
        self.inner
            .calculate_margin_required(notional.inner())
            .map(JsNotional::from_inner)
            .map_err(param_error_to_js)
    }

    /// Returns the canonical multiplier string.
    #[wasm_bindgen(js_name = toString)]
    pub fn to_js_string(&self) -> String {
        self.inner.to_string()
    }

    /// Returns a fresh `Leverage` holding the same multiplier.
    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsLeverage {
        *self
    }

    /// The fixed-point scale (`10`).
    #[wasm_bindgen(js_name = SCALE)]
    pub fn scale() -> u16 {
        Leverage::SCALE
    }

    /// The minimum integer multiplier (`1`).
    #[wasm_bindgen(js_name = MIN)]
    pub fn min() -> u16 {
        Leverage::MIN
    }

    /// The maximum integer multiplier (`3000`).
    #[wasm_bindgen(js_name = MAX)]
    pub fn max() -> u16 {
        Leverage::MAX
    }

    /// The smallest representable step (`0.1`).
    #[wasm_bindgen(js_name = STEP)]
    pub fn step() -> f64 {
        1.0 / f64::from(Leverage::SCALE)
    }
}

impl JsLeverage {
    /// Internal exact integer constructor after boundary validation.
    pub(crate) fn from_int_raw(value: u16) -> Result<Self, JsValue> {
        Leverage::from_u16(value)
            .map(|inner| Self { inner })
            .map_err(param_error_to_js)
    }

    /// Wraps a core [`Leverage`].
    pub fn from_inner(inner: Leverage) -> Self {
        Self { inner }
    }

    /// Returns the wrapped core [`Leverage`].
    pub fn inner(&self) -> Leverage {
        self.inner
    }
}
