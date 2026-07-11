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

//! Pre-trade `Lock` binding with full serialization parity.
//!
//! A lock groups `(policyGroupId, price)` records under their policy-group
//! identifier. It supports the three self-describing serde formats defined by
//! the core `PreTradeLock`: JSON, MessagePack, and CBOR. Each format preserves
//! the shared SDK wire contract.
//!
//! `policyGroupId` crosses the boundary as a JS `number` in `0..=65535`; a
//! value outside that range is a `ParamError`. Prices cross as `Price` value
//! objects.

use openpit::param::Price;
use openpit::pretrade::PreTradeLock;
use openpit::{PolicyGroupId, DEFAULT_POLICY_GROUP_ID};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use crate::domain::{
    extract_cloned_wrapper, parse_bounded_number, resolve_price, IntegerNumber, PriceLike,
};
use crate::error::{make_error, ErrorKind};
use crate::param::value_types::JsPrice;

#[wasm_bindgen(typescript_custom_section)]
const LOCK_ENTRIES_TS: &'static str = r#"
/**
 * A single `[policyGroupId, price]` lock entry. The price accepts a `Price`
 * wrapper or a `DecimalInput`.
 */
export type LockEntry = readonly [number, Price | string | number | bigint];
"#;

#[wasm_bindgen]
extern "C" {
    /// Lock seed: another `Lock` or an iterable of `[policyGroupId, price]`
    /// entries.
    #[wasm_bindgen(typescript_type = "Lock | Iterable<LockEntry> | null | undefined")]
    pub type LockEntriesLike;

    /// An iterable of `[policyGroupId, price]` entries.
    #[wasm_bindgen(typescript_type = "Iterable<LockEntry>")]
    pub type LockEntryIterable;

    /// An iterable of prices (`Price` wrappers or `DecimalInput`).
    #[wasm_bindgen(typescript_type = "Iterable<Price | string | number | bigint>")]
    pub type PriceIterable;
}

/// The default policy-group identifier (`0`).
///
/// Records pushed under this identifier take the lock's hot path.
#[wasm_bindgen(js_name = DEFAULT_POLICY_GROUP_ID)]
pub fn default_policy_group_id() -> u16 {
    DEFAULT_POLICY_GROUP_ID.value()
}

/// Parses a JS `number` policy-group identifier into the core type.
///
/// # Errors
///
/// Throws `ParamError` when `value` is outside `0..=65535`.
pub fn parse_policy_group_id(value: JsValue) -> Result<PolicyGroupId, JsValue> {
    let raw = parse_bounded_number(value, u64::from(u16::MAX), "policyGroupId")? as u16;
    Ok(PolicyGroupId::new(raw))
}

/// Serializable grouped `(policyGroupId, price)` reservation context.
#[wasm_bindgen(js_name = Lock)]
#[derive(Clone, Default)]
pub struct JsLock {
    inner: PreTradeLock,
}

#[wasm_bindgen(js_class = Lock)]
impl JsLock {
    /// Constructs a lock, optionally seeded from another lock or from an
    /// iterable of `[policyGroupId, Price]` pairs.
    ///
    /// Passing `undefined`/`null` yields an empty lock. Records with the same
    /// `policyGroupId` accumulate in insertion order.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` on an invalid `policyGroupId`, a malformed entry, or
    /// a non-iterable argument.
    #[wasm_bindgen(constructor)]
    pub fn new(entries: LockEntriesLike) -> Result<JsLock, JsValue> {
        let entries: JsValue = entries.into();
        if entries.is_undefined() || entries.is_null() {
            return Ok(Self::default());
        }
        // Accept another Lock instance directly (copy constructor).
        if let Some(other) = extract_cloned_wrapper::<JsLock>(&entries)? {
            return Ok(Self { inner: other.inner });
        }
        let mut inner = PreTradeLock::new();
        push_pairs_into_lock(&mut inner, &entries)?;
        Ok(Self { inner })
    }

    /// Stores `price` under `policyGroupId`, preserving prior prices.
    ///
    /// `price` accepts a `Price` object or a `DecimalInput`.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` on an invalid `policyGroupId` or price.
    #[wasm_bindgen(js_name = push)]
    pub fn push(
        &mut self,
        policy_group_id: IntegerNumber,
        price: PriceLike,
    ) -> Result<(), JsValue> {
        let policy_group_id = parse_policy_group_id(policy_group_id.into())?;
        let price = resolve_price(price.into())?;
        self.inner.push(policy_group_id, price);
        Ok(())
    }

    /// Stores every price under `policyGroupId`, preserving prior prices.
    ///
    /// `prices` is an iterable of `Price` objects or `DecimalInput` values.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` on an invalid `policyGroupId` or price.
    #[wasm_bindgen(js_name = pushMany)]
    pub fn push_many(
        &mut self,
        policy_group_id: IntegerNumber,
        prices: PriceIterable,
    ) -> Result<(), JsValue> {
        let policy_group_id = parse_policy_group_id(policy_group_id.into())?;
        let prices = collect_prices(prices.into())?;
        self.inner.push_many(policy_group_id, prices);
        Ok(())
    }

    /// Appends `[policyGroupId, price]` entries from an iterable.
    ///
    /// Each price accepts a `Price` object or a `DecimalInput`.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` on an invalid entry or a non-iterable argument.
    #[wasm_bindgen(js_name = extend)]
    pub fn extend(&mut self, entries: LockEntryIterable) -> Result<(), JsValue> {
        push_pairs_into_lock(&mut self.inner, &entries.into())
    }

    /// Appends every record from `other` into this lock.
    #[wasm_bindgen(js_name = merge)]
    pub fn merge(&mut self, other: &JsLock) {
        self.inner.merge(&other.inner);
    }

    /// Returns the number of stored price records.
    #[wasm_bindgen(getter, js_name = length)]
    pub fn length(&self) -> usize {
        self.inner.len()
    }

    /// Returns the number of stored price records.
    #[wasm_bindgen(js_name = size)]
    pub fn size(&self) -> usize {
        self.inner.len()
    }

    /// Returns `true` when the lock holds no price records.
    #[wasm_bindgen(getter, js_name = isEmpty)]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Returns every price stored under `policyGroupId`, in insertion order.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` on an invalid `policyGroupId`.
    #[wasm_bindgen(js_name = pricesOf)]
    pub fn prices_of(&self, policy_group_id: IntegerNumber) -> Result<Vec<JsPrice>, JsValue> {
        let policy_group_id = parse_policy_group_id(policy_group_id.into())?;
        Ok(self
            .inner
            .prices_of(policy_group_id)
            .map(JsPrice::from_inner)
            .collect())
    }

    /// Returns every stored price, in iteration order.
    ///
    /// Default-group records come first, then each non-default group in
    /// insertion order.
    #[wasm_bindgen(js_name = prices)]
    pub fn prices(&self) -> Vec<JsPrice> {
        self.inner
            .entries()
            .map(|(_group, price)| JsPrice::from_inner(price))
            .collect()
    }

    /// Returns every `[policyGroupId, Price]` pair as a `[number, Price]`
    /// array.
    ///
    /// Default-group records come first, then each non-default group in
    /// insertion order.
    #[wasm_bindgen(js_name = entries, unchecked_return_type = "[number, Price][]")]
    pub fn entries(&self) -> js_sys::Array {
        let entries = js_sys::Array::new();
        for (policy_group_id, price) in self.inner.entries() {
            let pair = js_sys::Array::new();
            pair.push(&JsValue::from_f64(f64::from(policy_group_id.value())));
            pair.push(&JsValue::from(JsPrice::from_inner(price)));
            entries.push(&pair);
        }
        entries
    }

    /// Serializes the lock to its compact JSON wire form.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` if encoding fails (not reachable for a valid lock).
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&self.inner).map_err(|error| encode_error("json", &error.to_string()))
    }

    /// Parses a lock from its JSON wire form.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` on malformed input.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(text: &str) -> Result<JsLock, JsValue> {
        let inner: PreTradeLock =
            serde_json::from_str(text).map_err(|error| decode_error("json", &error.to_string()))?;
        Ok(Self { inner })
    }

    /// Serializes the lock to its MessagePack wire form (`Uint8Array`).
    ///
    /// # Errors
    ///
    /// Throws `ParamError` if encoding fails (not reachable for a valid lock).
    #[wasm_bindgen(js_name = toMsgpack)]
    pub fn to_msgpack(&self) -> Result<js_sys::Uint8Array, JsValue> {
        let bytes = rmp_serde::to_vec(&self.inner)
            .map_err(|error| encode_error("msgpack", &error.to_string()))?;
        Ok(js_sys::Uint8Array::from(bytes.as_slice()))
    }

    /// Parses a lock from its MessagePack wire form (`Uint8Array`).
    ///
    /// # Errors
    ///
    /// Throws `ParamError` on malformed input.
    #[wasm_bindgen(js_name = fromMsgpack)]
    pub fn from_msgpack(data: &[u8]) -> Result<JsLock, JsValue> {
        let inner: PreTradeLock = rmp_serde::from_slice(data)
            .map_err(|error| decode_error("msgpack", &error.to_string()))?;
        Ok(Self { inner })
    }

    /// Serializes the lock to its CBOR wire form (`Uint8Array`).
    ///
    /// # Errors
    ///
    /// Throws `ParamError` if encoding fails (not reachable for a valid lock).
    #[wasm_bindgen(js_name = toCbor)]
    pub fn to_cbor(&self) -> Result<js_sys::Uint8Array, JsValue> {
        let mut buffer = Vec::new();
        ciborium::ser::into_writer(&self.inner, &mut buffer)
            .map_err(|error| encode_error("cbor", &error.to_string()))?;
        Ok(js_sys::Uint8Array::from(buffer.as_slice()))
    }

    /// Parses a lock from its CBOR wire form (`Uint8Array`).
    ///
    /// # Errors
    ///
    /// Throws `ParamError` on malformed input.
    #[wasm_bindgen(js_name = fromCbor)]
    pub fn from_cbor(data: &[u8]) -> Result<JsLock, JsValue> {
        let inner: PreTradeLock = ciborium::de::from_reader(data)
            .map_err(|error| decode_error("cbor", &error.to_string()))?;
        Ok(Self { inner })
    }

    /// Returns `true` when both locks hold the same records in the same order.
    #[wasm_bindgen(js_name = equals)]
    pub fn equals(&self, other: &JsLock) -> bool {
        self.inner == other.inner
    }

    /// Returns a deep copy of this lock, holding the same records.
    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsLock {
        self.clone()
    }
}

impl JsLock {
    /// Wraps a core [`PreTradeLock`].
    pub fn from_inner(inner: PreTradeLock) -> Self {
        Self { inner }
    }

    /// Returns a clone of the wrapped core [`PreTradeLock`].
    pub fn inner(&self) -> PreTradeLock {
        self.inner.clone()
    }
}

/// Builds the error raised when a lock fails to encode.
fn encode_error(format: &str, detail: &str) -> JsValue {
    make_error(
        ErrorKind::Param,
        &format!("lock {format} encode failed: {detail}"),
        Some("InvalidFormat"),
    )
}

/// Builds the error raised when a lock fails to decode.
fn decode_error(format: &str, detail: &str) -> JsValue {
    make_error(
        ErrorKind::Param,
        &format!("lock {format} decode failed: {detail}"),
        Some("InvalidFormat"),
    )
}

/// Pushes `[policyGroupId, price]` pairs from a JS iterable into `lock`.
///
/// The price element accepts a `Price` object or a `DecimalInput`.
fn push_pairs_into_lock(lock: &mut PreTradeLock, entries: &JsValue) -> Result<(), JsValue> {
    let iterator = js_sys::try_iter(entries)?.ok_or_else(invalid_entries)?;
    for item in iterator {
        let item = item?;
        let pair: js_sys::Array = item.dyn_into().map_err(|_| invalid_entries())?;
        if pair.length() != 2 {
            return Err(invalid_entries());
        }
        let policy_group_id = parse_policy_group_id(pair.get(0))?;
        let price = resolve_price(pair.get(1))?;
        lock.push(policy_group_id, price);
    }
    Ok(())
}

/// Collects an iterable of prices (`Price` objects or `DecimalInput`) into a
/// vector of core prices.
///
/// # Errors
///
/// Throws `ParamError` when the argument is not iterable or a price is invalid.
fn collect_prices(prices: JsValue) -> Result<Vec<Price>, JsValue> {
    let iterator = js_sys::try_iter(&prices)?.ok_or_else(|| {
        make_error(
            ErrorKind::Type,
            "prices must be an iterable of Price or DecimalInput",
            None,
        )
    })?;
    let mut result = Vec::new();
    for item in iterator {
        result.push(resolve_price(item?)?);
    }
    Ok(result)
}

/// Builds the error raised for malformed `[policyGroupId, Price]` entries.
fn invalid_entries() -> JsValue {
    make_error(
        ErrorKind::Type,
        "entries must be an iterable of [policyGroupId, Price] pairs",
        None,
    )
}
