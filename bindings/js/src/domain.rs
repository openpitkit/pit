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

//! Shared parsing helpers for the domain payload bindings.
//!
//! These helpers convert boundary inputs (asset strings, optional value
//! objects) into core types, mapping failures to the tagged JS errors in
//! [`crate::error`]. They are reused by the order, execution-report, and
//! account-adjustment group wrappers so the validation behavior stays
//! identical across them.
//!
//! Every input resolver accepts the idiomatic JS forms uniformly:
//! - the typed wrapper instance (cloned at the JS boundary before
//!   `try_from_js_value`, so coercion never invalidates the caller's handle),
//! - a primitive for scalar/value types (a `DecimalInput` for the decimal value
//!   types; a `number | bigint | string` for the identifier/leverage types),
//! - a plain object literal for the group/record types (read field by field via
//!   [`js_sys::Reflect`] and assembled through the existing field setters).
//!
//! The primitive and object-literal paths are non-consuming: primitives and
//! plain objects are not wasm handles, so the wasm move-consumption footgun
//! does not apply to idiomatic JS input.

use js_sys::BigInt;
use openpit::marketdata::InstrumentId;
use openpit::param::{
    AccountGroupId, AccountId, Asset, Fee, Leverage, Pnl, PositionSize, Price, Quantity, Volume,
};
use wasm_bindgen::convert::TryFromJsValue;
use wasm_bindgen::prelude::*;

use crate::error::{account_id_error_to_js, asset_error_to_js, make_error, ErrorKind};
use crate::param::ids::{JsAccountGroupId, JsAccountId, JsInstrumentId};
use crate::param::leverage::JsLeverage;
use crate::param::value_types::{JsFee, JsPnl, JsPositionSize, JsPrice, JsQuantity, JsVolume};

#[wasm_bindgen(inline_js = r#"
export function isPlainRecord(value) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    return false;
  }
  try {
    if (Object.prototype.hasOwnProperty.call(value, "__wbg_ptr")) {
      return false;
    }
    return Object.prototype.toString.call(value) === "[object Object]";
  } catch {
    return false;
  }
}
"#)]
extern "C" {
    #[wasm_bindgen(js_name = isPlainRecord)]
    fn is_plain_record_js(value: &JsValue) -> bool;
}

/// Validates an asset string and wraps it in the core [`Asset`] type.
///
/// # Errors
///
/// Throws `AssetError` when `value` is empty or whitespace-only.
pub fn parse_asset(value: &str) -> Result<Asset, JsValue> {
    Asset::new(value).map_err(|error| asset_error_to_js(&error.to_string()))
}

/// Generates a resolver that accepts either a value-type object or a
/// `DecimalInput` and yields the wrapped core type.
///
/// The setters/constructors on the order, execution-report, and adjustment
/// groups accept both forms so callers may pass a constructed `Price` (etc.) or
/// a raw decimal string/number/bigint.
macro_rules! define_value_resolver {
    ($resolve:ident, $js_type:ty, $core_type:ty, $label:literal) => {
        #[doc = concat!("Resolves a `", $label, " | DecimalInput` into the core type.")]
        ///
        /// # Errors
        ///
        /// Throws `TypeError`/`RangeError` for an invalid JS boundary value, or
        /// `ParamError` when the normalized domain value is invalid.
        pub fn $resolve(value: JsValue) -> Result<$core_type, JsValue> {
            if let Some(cloned) = clone_wrapper_value(&value)? {
                if let Ok(wrapped) = <$js_type>::try_from_js_value(cloned) {
                    return Ok(wrapped.inner());
                }
            }
            // The value-type constructor accepts a `DecimalInput`; the typed
            // `DecimalInputLike` wrapper is a transparent `JsValue` newtype.
            <$js_type>::new(value.unchecked_into()).map(|wrapped| wrapped.inner())
        }
    };
}

/// Generates an optional resolver over a base resolver; `undefined`/`null`
/// maps to `None`.
macro_rules! define_optional_value_resolver {
    ($resolve_opt:ident, $resolve:ident, $core_type:ty, $label:literal) => {
        #[doc = concat!(
                            "Resolves an optional `", $label, " | DecimalInput`; `undefined`/",
                            "`null` maps to `None`."
                        )]
        ///
        /// # Errors
        ///
        /// Throws the base resolver's boundary/domain error on an invalid
        /// present value.
        pub fn $resolve_opt(value: JsValue) -> Result<Option<$core_type>, JsValue> {
            if value.is_undefined() || value.is_null() {
                return Ok(None);
            }
            $resolve(value).map(Some)
        }
    };
}

define_value_resolver!(resolve_price, JsPrice, Price, "Price");
define_value_resolver!(resolve_quantity, JsQuantity, Quantity, "Quantity");
define_value_resolver!(resolve_pnl, JsPnl, Pnl, "Pnl");
define_value_resolver!(resolve_fee, JsFee, Fee, "Fee");
define_value_resolver!(resolve_volume, JsVolume, Volume, "Volume");
define_value_resolver!(
    resolve_position_size,
    JsPositionSize,
    PositionSize,
    "PositionSize"
);

define_optional_value_resolver!(resolve_optional_price, resolve_price, Price, "Price");
define_optional_value_resolver!(resolve_optional_pnl, resolve_pnl, Pnl, "Pnl");
define_optional_value_resolver!(
    resolve_optional_quantity,
    resolve_quantity,
    Quantity,
    "Quantity"
);
define_optional_value_resolver!(
    resolve_optional_position_size,
    resolve_position_size,
    PositionSize,
    "PositionSize"
);

/// Parses a JS `number` argument as a non-negative integer that fits `u64`.
///
/// Mirrors the identifier `fromInt` contract: the value must be a finite,
/// whole, non-negative safe integer. A fractional, negative, out-of-range, or
/// unsafe integer is an error built by the corresponding callback.
fn number_to_u64(
    number: f64,
    on_error: impl Fn() -> JsValue,
    on_unsafe_integer: impl Fn() -> JsValue,
) -> Result<u64, JsValue> {
    if !number.is_finite() || number.fract() != 0.0 || number < 0.0 {
        return Err(on_error());
    }
    if number > 9_007_199_254_740_991.0 {
        return Err(on_unsafe_integer());
    }
    Ok(number as u64)
}

/// Parses a JavaScript `number` as an exact non-negative integer bounded by
/// `max`. This is used instead of wasm-bindgen's narrowing integer ABI, which
/// otherwise wraps/truncates values before Rust can validate them.
pub fn parse_bounded_number(value: JsValue, max: u64, label: &str) -> Result<u64, JsValue> {
    let Some(number) = value.as_f64() else {
        return Err(make_error(
            ErrorKind::Type,
            &format!("{label} must be a number"),
            None,
        ));
    };
    if !number.is_finite()
        || number.fract() != 0.0
        || number < 0.0
        || number > max as f64
        || number > 9_007_199_254_740_991.0
    {
        return Err(make_error(
            ErrorKind::Range,
            &format!("{label} must be an integer in range 0..={max}"),
            None,
        ));
    }
    Ok(number as u64)
}

/// Parses an exact JavaScript `bigint` into `u64`.
pub fn parse_u64_bigint(value: JsValue, label: &str) -> Result<u64, JsValue> {
    if !value.is_bigint() {
        return Err(make_error(
            ErrorKind::Type,
            &format!("{label} must be a bigint"),
            None,
        ));
    }
    let bigint: BigInt = value.unchecked_into();
    u64::try_from(bigint).map_err(|_| {
        make_error(
            ErrorKind::Range,
            &format!("{label} must be an integer in range 0..={}", u64::MAX),
            None,
        )
    })
}

/// Resolves a 64-bit identifier from a wrapper, `number`, `bigint`, or
/// `string`.
///
/// `from_int`/`from_string` mirror the wrapper's own integer/string factories,
/// so the primitive path produces an identical identifier without consuming a
/// wasm handle.
fn resolve_u64_id<Wrapper, Id>(
    value: JsValue,
    from_int: impl Fn(u64) -> Result<Id, JsValue>,
    from_string: impl Fn(&str) -> Result<Id, JsValue>,
    extract: impl Fn(&Wrapper) -> Id,
    on_error: impl Fn() -> JsValue + Copy,
    on_unsafe_integer: impl Fn() -> JsValue,
) -> Result<Id, JsValue>
where
    Wrapper: TryFromJsValue,
{
    if let Some(cloned) = clone_wrapper_value(&value)? {
        if let Ok(wrapped) = <Wrapper>::try_from_js_value(cloned) {
            return Ok(extract(&wrapped));
        }
    }
    if let Some(text) = value.as_string() {
        return from_string(&text);
    }
    if value.is_bigint() {
        let bigint: BigInt = value.unchecked_into();
        return match u64::try_from(bigint) {
            Ok(raw) => from_int(raw),
            Err(_) => Err(on_error()),
        };
    }
    if let Some(number) = value.as_f64() {
        let raw = number_to_u64(number, on_error, on_unsafe_integer)?;
        return from_int(raw);
    }
    Err(on_error())
}

/// Resolves an `AccountId | number | bigint | string`.
///
/// A `number`/`bigint` mirrors `AccountId.fromInt`; a `string` mirrors
/// `AccountId.fromString` (FNV-1a-64). Wrapper inputs are cloned and remain
/// usable by the caller.
///
/// # Errors
///
/// Throws `AccountIdError` on an empty string or an out-of-range integer.
pub fn resolve_account_id(value: JsValue) -> Result<AccountId, JsValue> {
    resolve_u64_id::<JsAccountId, AccountId>(
        value,
        |raw| Ok(AccountId::from_u64(raw)),
        |text| match AccountId::from_str(text) {
            Ok(inner) => Ok(inner),
            Err(error) => Err(account_id_error_to_js(&error.to_string())),
        },
        JsAccountId::inner,
        || account_id_error_to_js("account id must be a non-negative integer"),
        || {
            account_id_error_to_js(concat!(
                "account id number must not exceed Number.MAX_SAFE_INTEGER; ",
                "use bigint for larger numeric ids or a string identifier"
            ))
        },
    )
}

/// Resolves an optional `AccountId | number | bigint | string`;
/// `undefined`/`null` maps to `None`.
///
/// # Errors
///
/// Throws `AccountIdError` on an invalid present value.
pub fn resolve_optional_account_id(value: JsValue) -> Result<Option<AccountId>, JsValue> {
    if value.is_undefined() || value.is_null() {
        return Ok(None);
    }
    resolve_account_id(value).map(Some)
}

/// Resolves an `InstrumentId | number | bigint | string`.
///
/// A `number`/`bigint` mirrors the `InstrumentId` constructor; a `string`
/// parses a decimal id. Wrapper inputs are cloned and remain usable by the
/// caller.
///
/// # Errors
///
/// Throws `ParamError` on an unparsable string or an out-of-range integer.
pub fn resolve_instrument_id(value: JsValue) -> Result<InstrumentId, JsValue> {
    resolve_u64_id::<JsInstrumentId, InstrumentId>(
        value,
        |raw| Ok(InstrumentId::new(raw)),
        |text| match text.trim().parse::<u64>() {
            Ok(raw) => Ok(InstrumentId::new(raw)),
            Err(_) => Err(instrument_id_error()),
        },
        JsInstrumentId::inner,
        instrument_id_error,
        instrument_id_unsafe_number_error,
    )
}

/// Builds the error raised for an invalid `InstrumentId` primitive.
fn instrument_id_error() -> JsValue {
    make_error(
        ErrorKind::Param,
        "instrument id must be a non-negative integer or its decimal string",
        Some("Other"),
    )
}

/// Builds the error raised for an unsafe `InstrumentId` number.
fn instrument_id_unsafe_number_error() -> JsValue {
    make_error(
        ErrorKind::Param,
        concat!(
            "instrument id number must not exceed Number.MAX_SAFE_INTEGER; ",
            "use bigint or a decimal string for larger ids"
        ),
        Some("Other"),
    )
}

/// Resolves an `AccountGroupId | number | bigint | string`.
///
/// A `number`/`bigint` mirrors `AccountGroupId.fromInt` (rejecting `0`); a
/// `string` mirrors `AccountGroupId.fromString` (FNV-1a-32). Wrapper inputs are
/// cloned and remain usable by the caller.
///
/// # Errors
///
/// Throws `ParamError` on `0`, an empty string, or an out-of-range integer.
pub fn resolve_account_group_id(value: JsValue) -> Result<AccountGroupId, JsValue> {
    if let Some(cloned) = clone_wrapper_value(&value)? {
        if let Ok(wrapped) = JsAccountGroupId::try_from_js_value(cloned) {
            return Ok(wrapped.inner());
        }
    }
    if let Some(text) = value.as_string() {
        return AccountGroupId::from_str(&text).map_err(account_group_id_error);
    }
    let raw = resolve_u32(&value, account_group_id_value_error)?;
    AccountGroupId::from_u32(raw).map_err(account_group_id_error)
}

/// Resolves an optional `AccountGroupId | number | bigint | string`;
/// `undefined`/`null` maps to `None`.
///
/// # Errors
///
/// Throws `ParamError` on an invalid present value.
pub fn resolve_optional_account_group_id(
    value: JsValue,
) -> Result<Option<AccountGroupId>, JsValue> {
    if value.is_undefined() || value.is_null() {
        return Ok(None);
    }
    resolve_account_group_id(value).map(Some)
}

/// Builds the `ParamError` for a failed `AccountGroupId` core conversion.
fn account_group_id_error(error: impl ToString) -> JsValue {
    make_error(ErrorKind::Param, &error.to_string(), Some("Other"))
}

/// Builds the `ParamError` for an `AccountGroupId` primitive of the wrong type.
fn account_group_id_value_error() -> JsValue {
    make_error(
        ErrorKind::Param,
        "account group id must be a non-negative integer or a string",
        Some("Other"),
    )
}

/// Parses a JS `number`/`bigint` argument as an integer that fits `u32`.
///
/// A `bigint` is narrowed through `u64` first (js-sys only converts `BigInt` to
/// the 64-/128-bit integer types).
fn resolve_u32(value: &JsValue, on_error: impl Fn() -> JsValue + Copy) -> Result<u32, JsValue> {
    if value.is_bigint() {
        let bigint: BigInt = value.clone().unchecked_into();
        let wide = u64::try_from(bigint).map_err(|_| on_error())?;
        return u32::try_from(wide).map_err(|_| on_error());
    }
    if let Some(number) = value.as_f64() {
        if !number.is_finite() || number.fract() != 0.0 || number < 0.0 {
            return Err(on_error());
        }
        return u32::try_from(number as u64).map_err(|_| on_error());
    }
    Err(on_error())
}

/// Resolves a `Leverage | number | bigint | string`.
///
/// A `bigint`/integer `number` mirrors `Leverage.fromInt`; a fractional
/// `number` mirrors `Leverage.fromFloat`; a `string` parses the multiplier.
/// Wrapper inputs are cloned and remain usable by the caller.
///
/// # Errors
///
/// Throws `ParamError` on an out-of-range or unparsable value.
pub fn resolve_leverage(value: JsValue) -> Result<Leverage, JsValue> {
    if let Some(cloned) = clone_wrapper_value(&value)? {
        if let Ok(wrapped) = JsLeverage::try_from_js_value(cloned) {
            return Ok(wrapped.inner());
        }
    }
    if let Some(text) = value.as_string() {
        return match text.trim().parse::<u16>() {
            Ok(raw) => JsLeverage::from_int_raw(raw).map(|wrapped| wrapped.inner()),
            // Non-integer multipliers (e.g. "2.5") flow through the float path.
            Err(_) => match text.trim().parse::<f64>() {
                Ok(number) => JsLeverage::from_float(number).map(|wrapped| wrapped.inner()),
                Err(_) => Err(leverage_error()),
            },
        };
    }
    if value.is_bigint() {
        let bigint: BigInt = value.unchecked_into();
        // js-sys narrows `BigInt` only to 64-/128-bit integers; step down to
        // the `u16` leverage multiplier through `u64`.
        return match u64::try_from(bigint)
            .ok()
            .and_then(|wide| u16::try_from(wide).ok())
        {
            Some(raw) => JsLeverage::from_int_raw(raw).map(|wrapped| wrapped.inner()),
            None => Err(leverage_error()),
        };
    }
    if let Some(number) = value.as_f64() {
        // An integer-valued float routes through `fromInt`; a fractional value
        // through `fromFloat`. Both mirror the wrapper's own factories.
        if number.is_finite() && number.fract() == 0.0 && (0.0..=65535.0).contains(&number) {
            return JsLeverage::from_int_raw(number as u16).map(|wrapped| wrapped.inner());
        }
        return JsLeverage::from_float(number).map(|wrapped| wrapped.inner());
    }
    Err(leverage_error())
}

/// Resolves an optional `Leverage | number | bigint | string`;
/// `undefined`/`null` maps to `None`.
///
/// # Errors
///
/// Throws `ParamError` on an invalid present value.
pub fn resolve_optional_leverage(value: JsValue) -> Result<Option<Leverage>, JsValue> {
    if value.is_undefined() || value.is_null() {
        return Ok(None);
    }
    resolve_leverage(value).map(Some)
}

/// Builds the `ParamError` raised for an invalid `Leverage` primitive.
fn leverage_error() -> JsValue {
    make_error(
        ErrorKind::Param,
        "leverage must be a Leverage, a number, a bigint, or a string",
        Some("Other"),
    )
}

/// Returns a non-consuming copy of a wasm wrapper instance by invoking its JS
/// `clone()` method.
///
/// `try_from_js_value` (the wasm-bindgen conversion) *moves* a wrapper out of
/// JS, leaving the caller's handle null. The engine entry points borrow their
/// argument (the caller must be able to reuse it), so they clone the wrapper at
/// the JS level first and extract the owned clone, leaving the original intact.
/// Every value/group wrapper exposes a `clone()` method, so this succeeds for
/// any of them; a value lacking `clone()` (for example a plain object) yields
/// `None` and the caller falls back to its object-literal path.
pub fn clone_wrapper_value(value: &JsValue) -> Result<Option<JsValue>, JsValue> {
    if !value.is_object() {
        return Ok(None);
    }

    // Only wasm-bindgen wrappers carry this pointer slot. Do not invoke an
    // arbitrary `clone` method on a user supplied record that happens to use
    // the same property name.
    let pointer = js_sys::Reflect::get(value, &JsValue::from_str("__wbg_ptr"))?;
    if pointer.is_undefined() || pointer.is_null() {
        return Ok(None);
    }

    let clone_fn = js_sys::Reflect::get(value, &JsValue::from_str("clone"))?;
    let clone_fn = clone_fn.dyn_into::<js_sys::Function>().map_err(|_| {
        make_error(
            ErrorKind::Type,
            "wasm wrapper does not expose a callable clone() method",
            None,
        )
    })?;
    clone_fn.call0(value).map(Some)
}

/// Attempts to extract a typed wasm wrapper without moving the caller's JS
/// object. Getter and `clone()` exceptions are propagated unchanged.
pub fn extract_cloned_wrapper<T>(value: &JsValue) -> Result<Option<T>, JsValue>
where
    T: TryFromJsValue,
{
    let Some(cloned) = clone_wrapper_value(value)? else {
        return Ok(None);
    };
    Ok(T::try_from_js_value(cloned).ok())
}

/// Collects an iterable of exported wasm classes without moving any caller-
/// owned wrapper. Every element is cloned before the owned Rust conversion.
///
/// # Errors
///
/// Throws `TypeError` when `value` is not iterable or an element is not the
/// expected wrapper type. Iterator and wrapper `clone()` exceptions propagate
/// unchanged.
pub fn collect_cloned_wrappers<T>(value: &JsValue, label: &str) -> Result<Vec<T>, JsValue>
where
    T: TryFromJsValue,
{
    let iterator = js_sys::try_iter(value)?
        .ok_or_else(|| make_error(ErrorKind::Type, &format!("{label} must be iterable"), None))?;
    let mut wrappers = Vec::new();
    for item in iterator {
        let item = item?;
        let wrapper = extract_cloned_wrapper::<T>(&item)?.ok_or_else(|| {
            make_error(
                ErrorKind::Type,
                &format!("{label} contains an invalid entry"),
                None,
            )
        })?;
        wrappers.push(wrapper);
    }
    Ok(wrappers)
}

/// Reads a property off a JS object literal. Missing properties return
/// `undefined`; a throwing getter is propagated unchanged. A non-object
/// target yields an OpenPit-branded `TypeError` instead of the raw
/// `Reflect.get` failure, keeping every boundary error branded.
pub fn read_field(value: &JsValue, field: &str) -> Result<JsValue, JsValue> {
    // `Reflect.get` throws a bare `TypeError` on a primitive target; brand it.
    // Object and function targets pass through so a throwing getter surfaces
    // unchanged.
    if !value.is_object() && !value.is_function() {
        return Err(make_error(
            ErrorKind::Type,
            &format!("expected an object to read field \"{field}\""),
            None,
        ));
    }
    js_sys::Reflect::get(value, &JsValue::from_str(field))
}

/// Returns `true` when `value` is a record-like object: an object literal, a
/// null-prototype record, or a structural custom-class instance.
///
/// Exported wasm wrappers and built-in branded objects (`Date`, `Map`,
/// `Promise`, and similar objects) are rejected. Structural application classes
/// are retained because custom order/report payloads deliberately support their
/// prototypes and metadata.
pub fn is_plain_object(value: &JsValue) -> bool {
    is_plain_record_js(value)
}

/// Returns whether `value` carries its own (not inherited) property named
/// `field`, distinguishing a present-but-`undefined` field from an absent one.
pub fn has_own_field(value: &JsValue, field: &str) -> Result<bool, JsValue> {
    Ok(js_sys::Reflect::own_keys(value)?
        .iter()
        .any(|key| key.as_string().is_some_and(|key| key == field)))
}

/// Reads an optional string property off a JS object literal.
///
/// A missing/`undefined`/`null` value maps to `None`; any other type is a
/// `ParamError`. Used by the group resolvers for asset-string fields.
///
/// # Errors
///
/// Throws `TypeError` when the present value is not a string.
pub fn read_optional_string(value: &JsValue, field: &str) -> Result<Option<String>, JsValue> {
    let raw = read_field(value, field)?;
    if raw.is_undefined() || raw.is_null() {
        return Ok(None);
    }
    match raw.as_string() {
        Some(text) => Ok(Some(text)),
        None => Err(make_error(
            ErrorKind::Type,
            &format!("{field} must be a string"),
            None,
        )),
    }
}

/// Reads an optional boolean property off a JS object literal.
///
/// A missing/`undefined`/`null` value maps to `None` (leaving the field at its
/// default); any other non-boolean type is a `ParamError`.
///
/// # Errors
///
/// Throws `TypeError` when the present value is not a boolean.
pub fn read_optional_bool(value: &JsValue, field: &str) -> Result<Option<bool>, JsValue> {
    let raw = read_field(value, field)?;
    if raw.is_undefined() || raw.is_null() {
        return Ok(None);
    }
    match raw.as_bool() {
        Some(flag) => Ok(Some(flag)),
        None => Err(make_error(
            ErrorKind::Type,
            &format!("{field} must be a boolean"),
            None,
        )),
    }
}

// Shared TypeScript argument unions. Each accepts the typed wrapper plus the
// idiomatic primitive form, so the generated `.d.ts` shows a precise union
// instead of `any`. Internally each is treated as a `JsValue` and routed
// through the matching resolver above.
#[wasm_bindgen]
extern "C" {
    /// A JS `number` validated in Rust before narrowing to an integer.
    #[wasm_bindgen(typescript_type = "number")]
    pub type IntegerNumber;

    /// An optional JS `number` validated before integer narrowing.
    #[wasm_bindgen(typescript_type = "number | null | undefined")]
    pub type OptionalIntegerNumber;

    /// An exact JS `bigint` validated in Rust before narrowing to 64 bits.
    #[wasm_bindgen(typescript_type = "bigint")]
    pub type BigIntLike;

    /// A `Price` wrapper or a `DecimalInput` (`string | number | bigint`).
    #[wasm_bindgen(typescript_type = "Price | string | number | bigint")]
    pub type PriceLike;

    /// An optional `Price` wrapper or `DecimalInput`.
    #[wasm_bindgen(typescript_type = "Price | string | number | bigint | null | undefined")]
    pub type OptionalPriceLike;

    /// A `Quantity` wrapper or a `DecimalInput` (`string | number | bigint`).
    #[wasm_bindgen(typescript_type = "Quantity | string | number | bigint")]
    pub type QuantityLike;

    /// An optional `Quantity` wrapper or `DecimalInput`.
    #[wasm_bindgen(typescript_type = "Quantity | string | number | bigint | null | undefined")]
    pub type OptionalQuantityLike;

    /// A `Pnl` wrapper or a `DecimalInput` (`string | number | bigint`).
    #[wasm_bindgen(typescript_type = "Pnl | string | number | bigint")]
    pub type PnlLike;

    /// An optional `Pnl` wrapper or `DecimalInput`.
    #[wasm_bindgen(typescript_type = "Pnl | string | number | bigint | null | undefined")]
    pub type OptionalPnlLike;

    /// A `Fee` wrapper or a `DecimalInput` (`string | number | bigint`).
    #[wasm_bindgen(typescript_type = "Fee | string | number | bigint")]
    pub type FeeLike;

    /// A `Volume` wrapper or a `DecimalInput` (`string | number | bigint`).
    #[wasm_bindgen(typescript_type = "Volume | string | number | bigint")]
    pub type VolumeLike;

    /// A `PositionSize` wrapper or a `DecimalInput`
    /// (`string | number | bigint`).
    #[wasm_bindgen(typescript_type = "PositionSize | string | number | bigint")]
    pub type PositionSizeLike;

    /// An optional `PositionSize` wrapper or `DecimalInput`.
    #[wasm_bindgen(typescript_type = "PositionSize | string | number | bigint | null | undefined")]
    pub type OptionalPositionSizeLike;

    /// An `AccountId` wrapper or a numeric/string identifier.
    #[wasm_bindgen(typescript_type = "AccountId | number | bigint | string")]
    pub type AccountIdLike;

    /// An optional `AccountId` wrapper or numeric/string identifier.
    #[wasm_bindgen(typescript_type = "AccountId | number | bigint | string | null | undefined")]
    pub type OptionalAccountIdLike;

    /// An `AccountGroupId` wrapper or a numeric/string identifier.
    #[wasm_bindgen(typescript_type = "AccountGroupId | number | bigint | string")]
    pub type AccountGroupIdLike;

    /// An optional `AccountGroupId` wrapper or numeric/string identifier.
    #[wasm_bindgen(
        typescript_type = "AccountGroupId | number | bigint | string | null | undefined"
    )]
    pub type OptionalAccountGroupIdLike;

    /// An `InstrumentId` wrapper or a numeric/string identifier.
    #[wasm_bindgen(typescript_type = "InstrumentId | number | bigint | string")]
    pub type InstrumentIdLike;

    /// A `Leverage` wrapper or a numeric/string multiplier.
    #[wasm_bindgen(typescript_type = "Leverage | number | bigint | string | null | undefined")]
    pub type LeverageLike;

    /// An optional `Side` wire string.
    #[wasm_bindgen(typescript_type = "\"BUY\" | \"SELL\" | null | undefined")]
    pub type OptionalSideLike;

    /// An optional `PositionSide` wire string.
    #[wasm_bindgen(typescript_type = "\"LONG\" | \"SHORT\" | null | undefined")]
    pub type OptionalPositionSideLike;

    /// An optional `PositionEffect` wire string.
    #[wasm_bindgen(typescript_type = "\"OPEN\" | \"CLOSE\" | null | undefined")]
    pub type OptionalPositionEffectLike;

    /// An optional `PositionMode` wire string.
    #[wasm_bindgen(typescript_type = "\"netting\" | \"hedged\" | null | undefined")]
    pub type OptionalPositionModeLike;
}
