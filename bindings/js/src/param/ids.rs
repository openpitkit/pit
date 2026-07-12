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

//! Identifier bindings: `AccountId`, `AccountGroupId`, `InstrumentId`.
//!
//! `AccountId` and `InstrumentId` are 64-bit, so their `value` getters surface
//! as JS `bigint` (a JS `number` would silently lose precision above
//! `2^53 - 1`). `AccountGroupId` is 32-bit and surfaces as a JS `number`.
//! String factories hash via the core FNV-1a constructors.

use openpit::marketdata::InstrumentId;
use openpit::param::{AccountGroupId, AccountId};
use wasm_bindgen::prelude::*;

use crate::domain::{parse_bounded_number, parse_u64_bigint, BigIntLike, IntegerNumber};
use crate::error::{account_id_error_to_js, make_error, ErrorKind};

/// Stable account identifier (64-bit).
#[wasm_bindgen(js_name = AccountId)]
#[derive(Clone, Copy)]
pub struct JsAccountId {
    inner: AccountId,
}

#[wasm_bindgen(js_class = AccountId)]
impl JsAccountId {
    /// Builds an account id from an exact 64-bit integer (`bigint`).
    #[wasm_bindgen(js_name = fromInt)]
    pub fn from_int(value: BigIntLike) -> Result<JsAccountId, JsValue> {
        let value = parse_u64_bigint(value.into(), "account id")?;
        Ok(Self {
            inner: AccountId::from_u64(value),
        })
    }

    /// Builds an account id by FNV-1a-64 hashing a non-empty string.
    ///
    /// # Errors
    ///
    /// Throws `AccountIdError` when `value` is empty or whitespace-only.
    #[wasm_bindgen(js_name = fromString)]
    pub fn from_string(value: &str) -> Result<JsAccountId, JsValue> {
        match AccountId::from_str(value) {
            Ok(inner) => Ok(Self { inner }),
            Err(error) => Err(account_id_error_to_js(&error.to_string())),
        }
    }

    /// Returns the raw 64-bit value as a JS `bigint`.
    #[wasm_bindgen(getter)]
    pub fn value(&self) -> u64 {
        self.inner.as_u64()
    }

    /// Returns `true` when both ids refer to the same account.
    #[wasm_bindgen(js_name = equals)]
    pub fn equals(&self, other: &JsAccountId) -> bool {
        self.inner == other.inner
    }

    /// Returns the decimal string form of the 64-bit value.
    #[wasm_bindgen(js_name = toString)]
    pub fn to_js_string(&self) -> String {
        self.inner.as_u64().to_string()
    }

    /// Returns a fresh `AccountId` holding the same value.
    ///
    /// Ordinary setters already borrow/clone wrapper inputs; this method is a
    /// convenience for callers that explicitly need another wrapper object.
    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsAccountId {
        *self
    }
}

impl JsAccountId {
    /// Wraps a core [`AccountId`].
    pub fn from_inner(inner: AccountId) -> Self {
        Self { inner }
    }

    /// Returns the wrapped core [`AccountId`].
    pub fn inner(&self) -> AccountId {
        self.inner
    }
}

/// Account-group identifier (32-bit). Group `0` is the reserved default.
#[wasm_bindgen(js_name = AccountGroupId)]
#[derive(Clone, Copy)]
pub struct JsAccountGroupId {
    inner: AccountGroupId,
}

#[wasm_bindgen(js_class = AccountGroupId)]
impl JsAccountGroupId {
    /// Returns the reserved default account group (`0`).
    #[wasm_bindgen(js_name = DEFAULT)]
    pub fn default_group() -> JsAccountGroupId {
        Self {
            inner: AccountGroupId::DEFAULT,
        }
    }

    /// Builds a group id from a 32-bit integer.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` when `value` is `0` (the reserved default group).
    #[wasm_bindgen(js_name = fromInt)]
    pub fn from_int(value: IntegerNumber) -> Result<JsAccountGroupId, JsValue> {
        let value = parse_bounded_number(value.into(), u64::from(u32::MAX), "account group id")?;
        let value = value as u32;
        match AccountGroupId::from_u32(value) {
            Ok(inner) => Ok(Self { inner }),
            Err(error) => Err(make_error(
                ErrorKind::Param,
                &error.to_string(),
                Some("Other"),
            )),
        }
    }

    /// Builds a group id by FNV-1a-32 hashing a non-empty string.
    ///
    /// The hash never collides with the reserved default group.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` when `value` is empty or whitespace-only.
    #[wasm_bindgen(js_name = fromString)]
    pub fn from_string(value: &str) -> Result<JsAccountGroupId, JsValue> {
        match AccountGroupId::from_str(value) {
            Ok(inner) => Ok(Self { inner }),
            Err(error) => Err(make_error(
                ErrorKind::Param,
                &error.to_string(),
                Some("Other"),
            )),
        }
    }

    /// Returns the raw 32-bit value as a JS `number`.
    #[wasm_bindgen(getter)]
    pub fn value(&self) -> u32 {
        self.inner.as_u32()
    }

    /// Returns `true` when both ids refer to the same group.
    #[wasm_bindgen(js_name = equals)]
    pub fn equals(&self, other: &JsAccountGroupId) -> bool {
        self.inner == other.inner
    }

    /// Returns the decimal string form of the 32-bit value.
    #[wasm_bindgen(js_name = toString)]
    pub fn to_js_string(&self) -> String {
        self.inner.as_u32().to_string()
    }

    /// Returns a fresh `AccountGroupId` holding the same value.
    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsAccountGroupId {
        *self
    }
}

impl JsAccountGroupId {
    /// Returns the wrapped core [`AccountGroupId`].
    pub fn inner(&self) -> AccountGroupId {
        self.inner
    }

    /// Wraps a core [`AccountGroupId`].
    pub fn from_inner(inner: AccountGroupId) -> Self {
        Self { inner }
    }
}

/// Stable instrument identifier shared by OpenPit subsystems (64-bit).
#[wasm_bindgen(js_name = InstrumentId)]
#[derive(Clone, Copy)]
pub struct JsInstrumentId {
    inner: InstrumentId,
}

#[wasm_bindgen(js_class = InstrumentId)]
impl JsInstrumentId {
    /// Builds an instrument id from an exact 64-bit integer (`bigint`).
    #[wasm_bindgen(constructor)]
    pub fn new(value: BigIntLike) -> Result<JsInstrumentId, JsValue> {
        let value = parse_u64_bigint(value.into(), "instrument id")?;
        Ok(Self {
            inner: InstrumentId::new(value),
        })
    }

    /// Returns the raw 64-bit value as a JS `bigint`.
    #[wasm_bindgen(getter)]
    pub fn value(&self) -> u64 {
        self.inner.as_u64()
    }

    /// Returns `true` when both ids refer to the same instrument.
    #[wasm_bindgen(js_name = equals)]
    pub fn equals(&self, other: &JsInstrumentId) -> bool {
        self.inner == other.inner
    }

    /// Returns the decimal string form of the 64-bit value.
    #[wasm_bindgen(js_name = toString)]
    pub fn to_js_string(&self) -> String {
        self.inner.as_u64().to_string()
    }

    /// Returns a fresh `InstrumentId` holding the same value.
    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsInstrumentId {
        *self
    }
}

impl JsInstrumentId {
    /// Wraps a core [`InstrumentId`].
    pub fn from_inner(inner: InstrumentId) -> Self {
        Self { inner }
    }

    /// Returns the wrapped core [`InstrumentId`].
    pub fn inner(&self) -> InstrumentId {
        self.inner
    }
}
