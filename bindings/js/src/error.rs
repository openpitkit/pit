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

//! JavaScript error mapping for the binding boundary.
//!
//! Fallible exports return `Result<T, JsValue>`. Each error is a real instance
//! of a public `Error` subclass from the TypeScript `errors` module: the
//! boundary calls the imported [`make_error`] JS factory, which switches on the
//! stable `name` discriminator and constructs the matching subclass directly.
//! Because that factory is the same module the public surface re-exports,
//! `instanceof` works in both the Node and browser builds. For value-validation
//! failures the `code` field carries a stable [`openpit::param::ErrorCode`]; for
//! account-block failures it carries the `AccountBlockErrorKind` discriminant.
//!
//! No reachable path panics: panics become unrecoverable wasm traps under
//! `panic = "abort"`, so every failure flows out as a `JsValue`.

use js_sys::{Object, Reflect};
use openpit::param::{Error as ParamError, ErrorCode};
use wasm_bindgen::prelude::*;

use crate::param::ids::{JsAccountGroupId, JsAccountId};

#[wasm_bindgen(module = "/src-ts/wasm-snippets/errors_snippet.js")]
extern "C" {
    /// Constructs the concrete `OpenpitError` subclass for a stable error
    /// `name`. Defined in `src-ts/errors.ts` and re-exported by the snippet so
    /// the engine and the public surface share one class identity.
    #[wasm_bindgen(js_name = makeError)]
    fn make_error_js(
        name: &str,
        message: &str,
        code: Option<String>,
        payload: JsValue,
        cause: JsValue,
    ) -> JsValue;

    /// Constructs a `QuoteExpired` carrying the selected stale quote.
    #[wasm_bindgen(js_name = makeQuoteExpiredError)]
    fn make_quote_expired_error_js(message: &str, quote: JsValue) -> JsValue;
}

/// Stable `name` discriminators carried on thrown JS errors.
///
/// The TypeScript layer maps these names onto exported `Error` subclasses so
/// callers can branch with `instanceof`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    /// Numeric validation or arithmetic failure (`openpit::param::Error`).
    Param,
    /// Asset identifier validation failure.
    Asset,
    /// Account identifier validation failure.
    AccountId,
    /// Base market-data read failure (`openpit::MarketDataError`).
    MarketData,
    /// Market-data read against an unregistered instrument id.
    UnknownInstrument,
    /// Market-data read with no usable quote (never pushed, cleared, aged out).
    QuoteUnavailable,
    /// Market-data read whose selected quote aged past its TTL.
    QuoteExpired,
    /// Registration of an instrument that is already registered.
    AlreadyRegistered,
    /// Explicit-id registration that conflicts with an existing entry.
    Registration,
    /// Operation referencing an instrument id that is not registered.
    UnknownInstrumentId,
    /// Reference-book registration that conflicts with an existing entry.
    ReferenceBookRegistration,
    /// Reference-book operation referencing an unknown instrument id.
    UnknownReferenceBookInstrumentId,
    /// Account-group register/unregister conflict.
    AccountGroupRegistration,
    /// Admin block/unblock/replace-reason failure.
    AccountBlock,
    /// Misuse of a single-use lifecycle handle (double execute/commit/rollback,
    /// or a stale account-control handle).
    Lifecycle,
    /// Engine build failure (for example a duplicate policy name).
    EngineBuild,
    /// Runtime policy reconfiguration failure.
    PolicyConfigure,
    /// Exception raised by a custom JavaScript policy callback.
    PolicyCallback,
    /// JavaScript wrong-type validation failure.
    Type,
    /// JavaScript range/value validation failure.
    Range,
}

impl ErrorKind {
    /// Returns the stable `name` string written onto the JS error.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Param => "ParamError",
            Self::Asset => "AssetError",
            Self::AccountId => "AccountIdError",
            Self::MarketData => "MarketDataError",
            Self::UnknownInstrument => "UnknownInstrument",
            Self::QuoteUnavailable => "QuoteUnavailable",
            Self::QuoteExpired => "QuoteExpired",
            Self::AlreadyRegistered => "AlreadyRegistered",
            Self::Registration => "RegistrationError",
            Self::UnknownInstrumentId => "UnknownInstrumentId",
            Self::ReferenceBookRegistration => "ReferenceBookRegistrationError",
            Self::UnknownReferenceBookInstrumentId => "UnknownReferenceBookInstrumentId",
            Self::AccountGroupRegistration => "AccountGroupRegistrationError",
            Self::AccountBlock => "AccountBlockError",
            Self::Lifecycle => "LifecycleError",
            Self::EngineBuild => "EngineBuildError",
            Self::PolicyConfigure => "PolicyConfigureError",
            Self::PolicyCallback => "PolicyCallbackError",
            Self::Type => "TypeError",
            Self::Range => "RangeError",
        }
    }
}

/// Builds a real `OpenpitError` subclass instance with a stable `name` and
/// optional `code`.
///
/// Delegates to the imported [`make_error_js`] factory so the returned value is
/// an actual `instanceof` the public subclass, not a plain tagged `Error`. For
/// `AccountBlock` the `code` slot carries the `AccountBlockErrorKind`
/// discriminant; for the value-validation kinds it carries an [`ErrorCode`].
pub fn make_error(kind: ErrorKind, message: &str, code: Option<&str>) -> JsValue {
    make_error_js(
        kind.name(),
        message,
        code.map(str::to_owned),
        JsValue::UNDEFINED,
        JsValue::UNDEFINED,
    )
}

/// Builds a typed JS error with structured payload and optional cause.
pub(crate) fn make_error_with(
    kind: ErrorKind,
    message: &str,
    code: Option<&str>,
    payload: JsValue,
    cause: JsValue,
) -> JsValue {
    make_error_js(
        kind.name(),
        message,
        code.map(str::to_owned),
        payload,
        cause,
    )
}

/// Wraps a custom-policy exception after the core operation has completed.
///
/// `result` is the completed post-trade/account-adjustment reconciliation
/// object when one exists, otherwise `undefined`.  The original thrown value is
/// retained verbatim as the standard JavaScript `cause`.
pub(crate) fn policy_callback_error(cause: JsValue, result: JsValue) -> JsValue {
    let payload = Object::new();
    // A fresh plain object cannot reject this property definition.
    let _ = Reflect::set(&payload, &JsValue::from_str("result"), &result);
    let message = Reflect::get(&cause, &JsValue::from_str("message"))
        .ok()
        .and_then(|value| value.as_string())
        .map(|detail| format!("javascript policy callback failed: {detail}"))
        .unwrap_or_else(|| "javascript policy callback failed".to_owned());
    make_error_with(
        ErrorKind::PolicyCallback,
        &message,
        None,
        payload.into(),
        cause,
    )
}

/// Builds an engine-build error for invalid builtin-policy configuration.
pub(crate) fn engine_build_configuration_error(message: &str) -> JsValue {
    make_error_with(
        ErrorKind::EngineBuild,
        message,
        Some("InvalidConfiguration"),
        Object::new().into(),
        JsValue::UNDEFINED,
    )
}

/// Builds a `QuoteExpired` carrying the selected stale quote.
pub fn make_quote_expired_error(message: &str, quote: JsValue) -> JsValue {
    make_quote_expired_error_js(message, quote)
}

/// Maps a stable [`ErrorCode`] to its string form for the JS `code` field.
const fn error_code_str(code: ErrorCode) -> &'static str {
    match code {
        ErrorCode::Unspecified => "Unspecified",
        ErrorCode::Negative => "Negative",
        ErrorCode::DivisionByZero => "DivisionByZero",
        ErrorCode::Overflow => "Overflow",
        ErrorCode::Underflow => "Underflow",
        ErrorCode::InvalidFloat => "InvalidFloat",
        ErrorCode::InvalidFormat => "InvalidFormat",
        ErrorCode::InvalidPrice => "InvalidPrice",
        ErrorCode::InvalidLeverage => "InvalidLeverage",
        ErrorCode::AssetEmpty => "AssetEmpty",
        ErrorCode::AccountIdEmpty => "AccountIdEmpty",
        ErrorCode::Other => "Other",
        // `ErrorCode` is `#[non_exhaustive]`: future codes degrade to "Other"
        // rather than panicking on the binding boundary.
        _ => "Other",
    }
}

/// Converts an `openpit::param::Error` into a tagged `ParamError` `JsValue`.
///
/// The message is the core `Display` text; the `code` carries the stable
/// [`ErrorCode`] so JS callers can branch without parsing the message.
pub fn param_error_to_js(error: ParamError) -> JsValue {
    let code = error_code_str(error.code());
    let payload = Object::new();
    let param = match &error {
        ParamError::Negative { param }
        | ParamError::DivisionByZero { param }
        | ParamError::Overflow { param }
        | ParamError::Underflow { param }
        | ParamError::InvalidFormat { param, .. } => Some(param.to_string()),
        _ => None,
    };
    if let Some(param) = param {
        let _ = Reflect::set(
            &payload,
            &JsValue::from_str("param"),
            &JsValue::from_str(&param),
        );
    }
    if let ParamError::InvalidFormat { input, .. } = &error {
        let _ = Reflect::set(
            &payload,
            &JsValue::from_str("input"),
            &JsValue::from_str(input),
        );
    }
    make_error_with(
        ErrorKind::Param,
        &error.to_string(),
        Some(code),
        payload.into(),
        JsValue::UNDEFINED,
    )
}

/// Converts an asset validation error into a tagged `AssetError` `JsValue`.
pub fn asset_error_to_js(message: &str) -> JsValue {
    make_error(
        ErrorKind::Asset,
        message,
        Some(error_code_str(ErrorCode::AssetEmpty)),
    )
}

/// Converts an account-id validation error into a tagged `AccountIdError`.
pub fn account_id_error_to_js(message: &str) -> JsValue {
    make_error(
        ErrorKind::AccountId,
        message,
        Some(error_code_str(ErrorCode::AccountIdEmpty)),
    )
}

/// Maps the core [`AccountBlockError`] variant to its `kind` discriminant for
/// the JS `AccountBlockError.kind` field.
///
/// The variant set is `#[non_exhaustive]`: a future variant degrades to no
/// discriminant rather than panicking on the boundary.
const fn account_block_kind_str(error: &openpit::AccountBlockError) -> Option<&'static str> {
    use openpit::AccountBlockError::{AccountNotBlocked, GroupNotBlocked, ReservedGroup};
    match error {
        ReservedGroup => Some("ReservedGroup"),
        AccountNotBlocked { .. } => Some("AccountNotBlocked"),
        GroupNotBlocked { .. } => Some("GroupNotBlocked"),
        _ => None,
    }
}

/// Converts a core [`AccountBlockError`] into an `AccountBlockError` `JsValue`.
///
/// The message is the core `Display` text; the `code` slot carries the
/// `AccountBlockErrorKind` discriminant so JS callers can branch on
/// `err.kind` without parsing the message.
pub fn account_block_error_to_js(error: &openpit::AccountBlockError) -> JsValue {
    let payload = Object::new();
    match error {
        openpit::AccountBlockError::ReservedGroup => {}
        openpit::AccountBlockError::AccountNotBlocked { account } => {
            let _ = Reflect::set(
                &payload,
                &JsValue::from_str("accountId"),
                &JsValue::from(JsAccountId::from_inner(*account)),
            );
        }
        openpit::AccountBlockError::GroupNotBlocked { group } => {
            let _ = Reflect::set(
                &payload,
                &JsValue::from_str("accountGroupId"),
                &JsValue::from(JsAccountGroupId::from_inner(*group)),
            );
        }
        _ => {}
    }
    make_error_with(
        ErrorKind::AccountBlock,
        &error.to_string(),
        account_block_kind_str(error),
        payload.into(),
        JsValue::UNDEFINED,
    )
}

/// Converts an account-group membership failure with its structured payload.
pub fn account_group_error_to_js(error: &openpit::AccountGroupError) -> JsValue {
    let payload = Object::new();
    let kind = match error {
        openpit::AccountGroupError::ReservedGroup => "ReservedGroup",
        openpit::AccountGroupError::AlreadyRegistered {
            account,
            current_group,
        } => {
            let _ = Reflect::set(
                &payload,
                &JsValue::from_str("accountId"),
                &JsValue::from(JsAccountId::from_inner(*account)),
            );
            let _ = Reflect::set(
                &payload,
                &JsValue::from_str("currentGroupId"),
                &JsValue::from(JsAccountGroupId::from_inner(*current_group)),
            );
            "AlreadyRegistered"
        }
        openpit::AccountGroupError::NotInGroup {
            account,
            requested_group,
            current_group,
        } => {
            let _ = Reflect::set(
                &payload,
                &JsValue::from_str("accountId"),
                &JsValue::from(JsAccountId::from_inner(*account)),
            );
            let _ = Reflect::set(
                &payload,
                &JsValue::from_str("requestedGroupId"),
                &JsValue::from(JsAccountGroupId::from_inner(*requested_group)),
            );
            if let Some(current_group) = current_group {
                let _ = Reflect::set(
                    &payload,
                    &JsValue::from_str("currentGroupId"),
                    &JsValue::from(JsAccountGroupId::from_inner(*current_group)),
                );
            }
            "NotInGroup"
        }
        _ => "NotInGroup",
    };
    make_error_with(
        ErrorKind::AccountGroupRegistration,
        &error.to_string(),
        Some(kind),
        payload.into(),
        JsValue::UNDEFINED,
    )
}

/// Maps a core configure error to its stable cross-binding discriminant.
fn configure_error_kind_str(error: &openpit::ConfigureError) -> &'static str {
    match error {
        openpit::ConfigureError::UnknownPolicy { .. } => "UNKNOWN",
        openpit::ConfigureError::PolicyTypeMismatch { .. } => "TYPE_MISMATCH",
        openpit::ConfigureError::Validation { .. } => "VALIDATION",
        openpit::ConfigureError::NestedConfiguration => "NESTED_CONFIGURATION",
        _ => "VALIDATION",
    }
}

/// Converts a core [`ConfigureError`](openpit::ConfigureError) into a
/// `PolicyConfigureError` `JsValue`.
pub fn configure_error_to_js(error: openpit::ConfigureError) -> JsValue {
    let payload = Object::new();
    match &error {
        openpit::ConfigureError::UnknownPolicy { name } => {
            let _ = Reflect::set(
                &payload,
                &JsValue::from_str("name"),
                &JsValue::from_str(name),
            );
        }
        openpit::ConfigureError::PolicyTypeMismatch {
            name,
            expected,
            found,
        } => {
            let _ = Reflect::set(
                &payload,
                &JsValue::from_str("name"),
                &JsValue::from_str(name),
            );
            let _ = Reflect::set(
                &payload,
                &JsValue::from_str("expected"),
                &JsValue::from_str(expected),
            );
            let _ = Reflect::set(
                &payload,
                &JsValue::from_str("found"),
                &JsValue::from_str(found),
            );
        }
        openpit::ConfigureError::Validation { name, message } => {
            let _ = Reflect::set(
                &payload,
                &JsValue::from_str("name"),
                &JsValue::from_str(name),
            );
            let _ = Reflect::set(
                &payload,
                &JsValue::from_str("validationMessage"),
                &JsValue::from_str(message),
            );
        }
        openpit::ConfigureError::NestedConfiguration => {}
        _ => {}
    }
    make_error_with(
        ErrorKind::PolicyConfigure,
        &error.to_string(),
        Some(configure_error_kind_str(&error)),
        payload.into(),
        JsValue::UNDEFINED,
    )
}
