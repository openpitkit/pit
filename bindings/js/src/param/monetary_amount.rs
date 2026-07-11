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

//! Monetary amount binding: an exact fee paired with its currency.

use openpit::param::MonetaryAmount;
use wasm_bindgen::prelude::*;

use crate::domain::{
    extract_cloned_wrapper, is_plain_object, parse_asset, read_field, resolve_fee, FeeLike,
};
use crate::error::{make_error, ErrorKind};
use crate::param::value_types::JsFee;

#[wasm_bindgen(typescript_custom_section)]
const MONETARY_AMOUNT_INIT_TS: &'static str = r#"
/**
 * Plain-object form of {@link MonetaryAmount}. Both fields are required.
 */
export interface MonetaryAmountInit {
  amount: Fee | string | number | bigint;
  currency: string;
}
"#;

#[wasm_bindgen]
extern "C" {
    /// A `MonetaryAmount` wrapper or its plain-object form.
    #[wasm_bindgen(typescript_type = "MonetaryAmount | MonetaryAmountInit | null | undefined")]
    pub type MonetaryAmountLike;
}

/// Exact fee amount paired with its validated currency.
#[wasm_bindgen(js_name = MonetaryAmount)]
#[derive(Clone)]
pub struct JsMonetaryAmount {
    inner: MonetaryAmount,
}

#[wasm_bindgen(js_class = MonetaryAmount)]
impl JsMonetaryAmount {
    /// Constructs a monetary amount from an exact fee and currency.
    ///
    /// `amount` accepts a `Fee` value object or a `DecimalInput`. `currency`
    /// uses the same validated asset-string contract as instrument assets.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` on an invalid fee or `AssetError` on an invalid
    /// currency.
    #[wasm_bindgen(constructor)]
    pub fn new(amount: FeeLike, currency: &str) -> Result<JsMonetaryAmount, JsValue> {
        Ok(Self {
            inner: MonetaryAmount {
                amount: resolve_fee(amount.into())?,
                currency: parse_asset(currency)?,
            },
        })
    }

    /// The exact fee amount.
    #[wasm_bindgen(getter, js_name = amount)]
    pub fn amount(&self) -> JsFee {
        JsFee::from_inner(self.inner.amount)
    }

    /// The currency identifier.
    #[wasm_bindgen(getter, js_name = currency)]
    pub fn currency(&self) -> String {
        self.inner.currency.to_string()
    }

    /// Returns `true` when both the amount and currency are equal.
    #[wasm_bindgen(js_name = equals)]
    pub fn equals(&self, other: &JsMonetaryAmount) -> bool {
        self.inner == other.inner
    }

    /// Returns a fresh monetary amount holding the same amount and currency.
    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsMonetaryAmount {
        self.clone()
    }
}

impl JsMonetaryAmount {
    /// Wraps a core [`MonetaryAmount`].
    pub fn from_inner(inner: MonetaryAmount) -> Self {
        Self { inner }
    }

    /// Returns the wrapped core [`MonetaryAmount`].
    pub fn inner(&self) -> MonetaryAmount {
        self.inner.clone()
    }

    /// Resolves an optional wrapper or plain-object monetary amount.
    ///
    /// # Errors
    ///
    /// Throws `ParamError`/`AssetError` when a present value cannot be
    /// marshalled into the core value types.
    pub fn resolve_optional(value: JsValue) -> Result<Option<MonetaryAmount>, JsValue> {
        if value.is_undefined() || value.is_null() {
            return Ok(None);
        }
        if let Some(wrapped) = extract_cloned_wrapper::<JsMonetaryAmount>(&value)? {
            return Ok(Some(wrapped.inner()));
        }
        if is_plain_object(&value) {
            let currency = read_field(&value, "currency")?
                .as_string()
                .ok_or_else(|| make_error(ErrorKind::Type, "currency must be a string", None))?;
            return Ok(Some(MonetaryAmount {
                amount: resolve_fee(read_field(&value, "amount")?)?,
                currency: parse_asset(&currency)?,
            }));
        }
        Err(make_error(
            ErrorKind::Type,
            "fee must be a MonetaryAmount or { amount, currency }",
            None,
        ))
    }
}
