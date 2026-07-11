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

//! Reject and account-block payload bindings.
//!
//! `Reject` is a read-only record the engine produces inside result objects;
//! `AccountBlock` is constructible and returned by a custom policy's
//! kill-switch path. The `userData` token is the opaque caller payload and
//! crosses the boundary as a JS `bigint`. The core token is `usize`, so on
//! wasm32 it holds a 32-bit value; a `bigint` above `u32::MAX` is rejected
//! rather than truncated.

use openpit::pretrade::RejectCode;
use wasm_bindgen::prelude::*;

use crate::domain::{parse_u64_bigint, BigIntLike};
use crate::error::{make_error, ErrorKind};

/// Every stable reject code, used to validate `AccountBlock.code` strings.
///
/// `RejectCode` is `#[non_exhaustive]` and exposes no iterator, so the binding
/// keeps this explicit table. A future core code that is missing here simply
/// fails validation rather than being silently accepted.
const REJECT_CODES: &[RejectCode] = &[
    RejectCode::MissingRequiredField,
    RejectCode::InvalidFieldFormat,
    RejectCode::InvalidFieldValue,
    RejectCode::UnsupportedOrderType,
    RejectCode::UnsupportedTimeInForce,
    RejectCode::UnsupportedOrderAttribute,
    RejectCode::DuplicateClientOrderId,
    RejectCode::TooLateToEnter,
    RejectCode::ExchangeClosed,
    RejectCode::UnknownInstrument,
    RejectCode::UnknownAccount,
    RejectCode::UnknownVenue,
    RejectCode::UnknownClearingAccount,
    RejectCode::UnknownCollateralAsset,
    RejectCode::InsufficientFunds,
    RejectCode::InsufficientMargin,
    RejectCode::InsufficientPosition,
    RejectCode::CreditLimitExceeded,
    RejectCode::RiskLimitExceeded,
    RejectCode::OrderExceedsLimit,
    RejectCode::OrderQtyExceedsLimit,
    RejectCode::OrderNotionalExceedsLimit,
    RejectCode::PositionLimitExceeded,
    RejectCode::ConcentrationLimitExceeded,
    RejectCode::LeverageLimitExceeded,
    RejectCode::RateLimitExceeded,
    RejectCode::PnlKillSwitchTriggered,
    RejectCode::AccountBlocked,
    RejectCode::AccountNotAuthorized,
    RejectCode::ComplianceRestriction,
    RejectCode::InstrumentRestricted,
    RejectCode::JurisdictionRestriction,
    RejectCode::WashTradePrevention,
    RejectCode::SelfMatchPrevention,
    RejectCode::ShortSaleRestriction,
    RejectCode::RiskConfigurationMissing,
    RejectCode::ReferenceDataUnavailable,
    RejectCode::OrderValueCalculationFailed,
    RejectCode::SystemUnavailable,
    RejectCode::MarkPriceUnavailable,
    RejectCode::AccountAdjustmentBoundsExceeded,
    RejectCode::ArithmeticOverflow,
    RejectCode::Custom,
    RejectCode::Other,
];

/// Validates a reject-code wire string against the stable code set.
///
/// # Errors
///
/// Throws `ParamError` when `value` is not a recognized reject code.
pub(crate) fn parse_reject_code(value: &str) -> Result<RejectCode, JsValue> {
    REJECT_CODES
        .iter()
        .copied()
        .find(|code| code.as_str() == value)
        .ok_or_else(|| {
            make_error(
                ErrorKind::Param,
                &format!("unknown reject code: {value}"),
                Some("Other"),
            )
        })
}

/// Read-only rejection record produced by the engine.
#[wasm_bindgen(js_name = Reject)]
#[derive(Clone)]
pub struct JsReject {
    code: String,
    reason: String,
    details: String,
    policy: String,
    scope: String,
    user_data: u64,
}

#[wasm_bindgen(js_class = Reject)]
impl JsReject {
    /// The stable machine-readable reject code.
    #[wasm_bindgen(
        getter,
        js_name = code,
        unchecked_return_type = "import(\"../types.js\").RejectCode"
    )]
    pub fn code(&self) -> String {
        self.code.clone()
    }

    /// The human-readable reject reason.
    #[wasm_bindgen(getter, js_name = reason)]
    pub fn reason(&self) -> String {
        self.reason.clone()
    }

    /// Case-specific reject details.
    #[wasm_bindgen(getter, js_name = details)]
    pub fn details(&self) -> String {
        self.details.clone()
    }

    /// The name of the policy that produced the reject.
    #[wasm_bindgen(getter, js_name = policy)]
    pub fn policy(&self) -> String {
        self.policy.clone()
    }

    /// The reject scope (`"order"` or `"account"`).
    #[wasm_bindgen(
        getter,
        js_name = scope,
        unchecked_return_type = "import(\"../types.js\").RejectScope"
    )]
    pub fn scope(&self) -> String {
        self.scope.clone()
    }

    /// The opaque caller-defined token (`0` means unset) as a `bigint`.
    ///
    /// The core token is `usize`, so on wasm32 this is a 32-bit value.
    #[wasm_bindgen(getter, js_name = userData)]
    pub fn user_data(&self) -> u64 {
        self.user_data
    }
}

impl JsReject {
    /// Builds a reject record from the core [`openpit::pretrade::Reject`].
    pub(crate) fn from_core(reject: &openpit::pretrade::Reject) -> Self {
        use openpit::pretrade::RejectScope;
        Self {
            code: reject.code.as_str().to_owned(),
            reason: reject.reason.clone(),
            details: reject.details.clone(),
            policy: reject.policy.clone(),
            scope: match reject.scope {
                RejectScope::Order => "order",
                RejectScope::Account => "account",
            }
            .to_owned(),
            user_data: reject.user_data as u64,
        }
    }
}

/// Account block returned by a custom policy's kill-switch path.
#[wasm_bindgen(js_name = AccountBlock)]
#[derive(Clone)]
pub struct JsAccountBlock {
    code: String,
    policy: String,
    reason: String,
    details: String,
    user_data: u64,
}

#[wasm_bindgen(js_class = AccountBlock)]
impl JsAccountBlock {
    /// Constructs an account block.
    ///
    /// `userData` defaults to `0`. The `code` is validated against the stable
    /// reject-code set.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` when `code` is not a recognized reject code.
    #[wasm_bindgen(constructor)]
    pub fn new(
        policy: String,
        code: String,
        reason: String,
        details: String,
        user_data: Option<BigIntLike>,
    ) -> Result<JsAccountBlock, JsValue> {
        parse_reject_code(&code)?;
        let user_data = if let Some(user_data) = user_data {
            let value = parse_u64_bigint(user_data.into(), "userData")?;
            usize::try_from(value).map_err(|_| {
                make_error(
                    ErrorKind::Range,
                    "userData exceeds the supported token range",
                    None,
                )
            })? as u64
        } else {
            0
        };
        Ok(Self {
            code,
            policy,
            reason,
            details,
            user_data,
        })
    }

    /// The reject code.
    #[wasm_bindgen(
        getter,
        js_name = code,
        unchecked_return_type = "import(\"../types.js\").RejectCode"
    )]
    pub fn code(&self) -> String {
        self.code.clone()
    }

    /// The name of the policy that produced the block.
    #[wasm_bindgen(getter, js_name = policy)]
    pub fn policy(&self) -> String {
        self.policy.clone()
    }

    /// The human-readable block reason.
    #[wasm_bindgen(getter, js_name = reason)]
    pub fn reason(&self) -> String {
        self.reason.clone()
    }

    /// Case-specific block details.
    #[wasm_bindgen(getter, js_name = details)]
    pub fn details(&self) -> String {
        self.details.clone()
    }

    /// The opaque caller-defined token as a `bigint`.
    ///
    /// The core token is `usize`, so on wasm32 it must fit `u32`; a larger
    /// value is rejected when the block is converted to its core form.
    #[wasm_bindgen(getter, js_name = userData)]
    pub fn user_data(&self) -> u64 {
        self.user_data
    }

    /// Returns a fresh copy of this account block.
    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsAccountBlock {
        self.clone()
    }
}

impl JsAccountBlock {
    /// Builds an account block from the core
    /// [`openpit::pretrade::AccountBlock`].
    pub(crate) fn from_core(block: &openpit::pretrade::AccountBlock) -> Self {
        Self {
            code: block.code.as_str().to_owned(),
            policy: block.policy.clone(),
            reason: block.reason.clone(),
            details: block.details.clone(),
            user_data: block.user_data as u64,
        }
    }

    /// Builds the core [`openpit::pretrade::AccountBlock`] from this wrapper.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` if the stored code is no longer recognized, or if
    /// the `userData` token exceeds the supported range (the core token is
    /// `usize`, so on wasm32 it must fit `u32`).
    pub(crate) fn to_core(&self) -> Result<openpit::pretrade::AccountBlock, JsValue> {
        let code = parse_reject_code(&self.code)?;
        // The core token is `usize` (32-bit on wasm32); reject an out-of-range
        // value instead of truncating it silently, mirroring `read_user_data`.
        let user_data = usize::try_from(self.user_data).map_err(|_| {
            make_error(
                ErrorKind::Range,
                "userData exceeds the supported token range",
                None,
            )
        })?;
        Ok(openpit::pretrade::AccountBlock::new(
            self.policy.clone(),
            code,
            self.reason.clone(),
            self.details.clone(),
        )
        .with_user_data(user_data))
    }
}
