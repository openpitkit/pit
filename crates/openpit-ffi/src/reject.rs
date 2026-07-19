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
// Please see https://github.com/openpitkit and the OWNERS file for details.

use crate::OpenPitStringView;
use openpit::pretrade::{AccountBlock, Reject, RejectCode, RejectScope, Rejects};
use std::ffi::c_void;

/// Raw reject-scope code accepted from C callers.
///
/// Zero is not valid; callers must set this field explicitly.
pub type OpenPitPretradeRejectScope = u8;

/// The reject applies to one order or order-like request.
pub const OPENPIT_PRETRADE_REJECT_SCOPE_ORDER: OpenPitPretradeRejectScope = 1;
/// The reject applies to account state rather than to one order only.
pub const OPENPIT_PRETRADE_REJECT_SCOPE_ACCOUNT: OpenPitPretradeRejectScope = 2;

/// Raw stable classification code for a reject.
///
/// Read this first when you need machine-readable handling. The textual fields
/// in [`OpenPitPretradeReject`] provide operator-facing explanation and extra context.
///
/// Valid codes are `1..=42`, `254` (`Custom`), and `255` (`Other`). Unknown
/// incoming codes are mapped to `Other` (`255`).
pub type OpenPitPretradeRejectCode = u16;

/// A required field is absent.
pub const OPENPIT_PRETRADE_REJECT_CODE_MISSING_REQUIRED_FIELD: OpenPitPretradeRejectCode = 1;
/// A field cannot be parsed from the supplied wire value.
pub const OPENPIT_PRETRADE_REJECT_CODE_INVALID_FIELD_FORMAT: OpenPitPretradeRejectCode = 2;
/// A field is syntactically valid but semantically unacceptable.
pub const OPENPIT_PRETRADE_REJECT_CODE_INVALID_FIELD_VALUE: OpenPitPretradeRejectCode = 3;
/// The requested order type is not supported.
pub const OPENPIT_PRETRADE_REJECT_CODE_UNSUPPORTED_ORDER_TYPE: OpenPitPretradeRejectCode = 4;
/// The requested time-in-force is not supported.
pub const OPENPIT_PRETRADE_REJECT_CODE_UNSUPPORTED_TIME_IN_FORCE: OpenPitPretradeRejectCode = 5;
/// Another order attribute is unsupported.
pub const OPENPIT_PRETRADE_REJECT_CODE_UNSUPPORTED_ORDER_ATTRIBUTE: OpenPitPretradeRejectCode = 6;
/// The client order identifier duplicates an active order.
pub const OPENPIT_PRETRADE_REJECT_CODE_DUPLICATE_CLIENT_ORDER_ID: OpenPitPretradeRejectCode = 7;
/// The order arrived after the allowed entry deadline.
pub const OPENPIT_PRETRADE_REJECT_CODE_TOO_LATE_TO_ENTER: OpenPitPretradeRejectCode = 8;
/// Trading is closed for the relevant venue or session.
pub const OPENPIT_PRETRADE_REJECT_CODE_EXCHANGE_CLOSED: OpenPitPretradeRejectCode = 9;
/// The instrument cannot be resolved.
pub const OPENPIT_PRETRADE_REJECT_CODE_UNKNOWN_INSTRUMENT: OpenPitPretradeRejectCode = 10;
/// The account cannot be resolved.
pub const OPENPIT_PRETRADE_REJECT_CODE_UNKNOWN_ACCOUNT: OpenPitPretradeRejectCode = 11;
/// The venue cannot be resolved.
pub const OPENPIT_PRETRADE_REJECT_CODE_UNKNOWN_VENUE: OpenPitPretradeRejectCode = 12;
/// The clearing account cannot be resolved.
pub const OPENPIT_PRETRADE_REJECT_CODE_UNKNOWN_CLEARING_ACCOUNT: OpenPitPretradeRejectCode = 13;
/// The collateral asset cannot be resolved.
pub const OPENPIT_PRETRADE_REJECT_CODE_UNKNOWN_COLLATERAL_ASSET: OpenPitPretradeRejectCode = 14;
/// Available balance is insufficient.
pub const OPENPIT_PRETRADE_REJECT_CODE_INSUFFICIENT_FUNDS: OpenPitPretradeRejectCode = 15;
/// Available margin is insufficient.
pub const OPENPIT_PRETRADE_REJECT_CODE_INSUFFICIENT_MARGIN: OpenPitPretradeRejectCode = 16;
/// Available position is insufficient.
pub const OPENPIT_PRETRADE_REJECT_CODE_INSUFFICIENT_POSITION: OpenPitPretradeRejectCode = 17;
/// A credit limit was exceeded.
pub const OPENPIT_PRETRADE_REJECT_CODE_CREDIT_LIMIT_EXCEEDED: OpenPitPretradeRejectCode = 18;
/// A risk limit was exceeded.
pub const OPENPIT_PRETRADE_REJECT_CODE_RISK_LIMIT_EXCEEDED: OpenPitPretradeRejectCode = 19;
/// The order exceeds a generic configured limit.
pub const OPENPIT_PRETRADE_REJECT_CODE_ORDER_EXCEEDS_LIMIT: OpenPitPretradeRejectCode = 20;
/// The order quantity exceeds a configured limit.
pub const OPENPIT_PRETRADE_REJECT_CODE_ORDER_QTY_EXCEEDS_LIMIT: OpenPitPretradeRejectCode = 21;
/// The order notional exceeds a configured limit.
pub const OPENPIT_PRETRADE_REJECT_CODE_ORDER_NOTIONAL_EXCEEDS_LIMIT: OpenPitPretradeRejectCode = 22;
/// The resulting position exceeds a configured limit.
pub const OPENPIT_PRETRADE_REJECT_CODE_POSITION_LIMIT_EXCEEDED: OpenPitPretradeRejectCode = 23;
/// Concentration constraints were violated.
pub const OPENPIT_PRETRADE_REJECT_CODE_CONCENTRATION_LIMIT_EXCEEDED: OpenPitPretradeRejectCode = 24;
/// Leverage constraints were violated.
pub const OPENPIT_PRETRADE_REJECT_CODE_LEVERAGE_LIMIT_EXCEEDED: OpenPitPretradeRejectCode = 25;
/// The request rate exceeded a configured limit.
pub const OPENPIT_PRETRADE_REJECT_CODE_RATE_LIMIT_EXCEEDED: OpenPitPretradeRejectCode = 26;
/// A loss barrier has blocked further risk-taking.
pub const OPENPIT_PRETRADE_REJECT_CODE_PNL_KILL_SWITCH_TRIGGERED: OpenPitPretradeRejectCode = 27;
/// The account is blocked.
pub const OPENPIT_PRETRADE_REJECT_CODE_ACCOUNT_BLOCKED: OpenPitPretradeRejectCode = 28;
/// The account is not authorized for this action.
pub const OPENPIT_PRETRADE_REJECT_CODE_ACCOUNT_NOT_AUTHORIZED: OpenPitPretradeRejectCode = 29;
/// A compliance restriction blocked the action.
pub const OPENPIT_PRETRADE_REJECT_CODE_COMPLIANCE_RESTRICTION: OpenPitPretradeRejectCode = 30;
/// The instrument is restricted.
pub const OPENPIT_PRETRADE_REJECT_CODE_INSTRUMENT_RESTRICTED: OpenPitPretradeRejectCode = 31;
/// A jurisdiction restriction blocked the action.
pub const OPENPIT_PRETRADE_REJECT_CODE_JURISDICTION_RESTRICTION: OpenPitPretradeRejectCode = 32;
/// The action would violate wash-trade prevention.
pub const OPENPIT_PRETRADE_REJECT_CODE_WASH_TRADE_PREVENTION: OpenPitPretradeRejectCode = 33;
/// The action would violate self-match prevention.
pub const OPENPIT_PRETRADE_REJECT_CODE_SELF_MATCH_PREVENTION: OpenPitPretradeRejectCode = 34;
/// Short-sale restriction blocked the action.
pub const OPENPIT_PRETRADE_REJECT_CODE_SHORT_SALE_RESTRICTION: OpenPitPretradeRejectCode = 35;
/// Required risk configuration is missing.
pub const OPENPIT_PRETRADE_REJECT_CODE_RISK_CONFIGURATION_MISSING: OpenPitPretradeRejectCode = 36;
/// Required reference data is unavailable.
pub const OPENPIT_PRETRADE_REJECT_CODE_REFERENCE_DATA_UNAVAILABLE: OpenPitPretradeRejectCode = 37;
/// The system could not compute an order value needed for validation.
pub const OPENPIT_PRETRADE_REJECT_CODE_ORDER_VALUE_CALCULATION_FAILED: OpenPitPretradeRejectCode =
    38;
/// A required service or subsystem is unavailable.
pub const OPENPIT_PRETRADE_REJECT_CODE_SYSTEM_UNAVAILABLE: OpenPitPretradeRejectCode = 39;
/// Required mark price is unavailable.
pub const OPENPIT_PRETRADE_REJECT_CODE_MARK_PRICE_UNAVAILABLE: OpenPitPretradeRejectCode = 40;
/// Account adjustment would violate configured bounds.
pub const OPENPIT_PRETRADE_REJECT_CODE_ACCOUNT_ADJUSTMENT_BOUNDS_EXCEEDED:
    OpenPitPretradeRejectCode = 41;
/// Underlying decimal arithmetic overflowed during evaluation.
pub const OPENPIT_PRETRADE_REJECT_CODE_ARITHMETIC_OVERFLOW: OpenPitPretradeRejectCode = 42;
/// Reserved code for caller-defined reject classes.
pub const OPENPIT_PRETRADE_REJECT_CODE_CUSTOM: OpenPitPretradeRejectCode = 254;
/// A catch-all code for rejects that do not fit a more specific class.
pub const OPENPIT_PRETRADE_REJECT_CODE_OTHER: OpenPitPretradeRejectCode = 255;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Single rejection record returned by checks.
pub struct OpenPitPretradeReject {
    /// Policy name that produced the reject.
    pub policy: OpenPitStringView,
    /// Human-readable reject reason.
    pub reason: OpenPitStringView,
    /// Case-specific reject details.
    pub details: OpenPitStringView,
    /// Opaque caller-defined token.
    ///
    /// The SDK never inspects, dereferences, or frees this value. Its meaning,
    /// lifetime, and thread-safety are the caller's responsibility. `0` / null
    /// means "not set". See the project Threading Contract for the full lifetime
    /// model.
    ///
    /// The token flows through every reject path the SDK exposes (start-stage,
    /// main-stage, account-adjustment, batch results) and is preserved on
    /// `Clone`.
    pub user_data: *mut c_void,
    /// Stable machine-readable reject code.
    pub code: OpenPitPretradeRejectCode,
    /// Reject scope.
    pub scope: OpenPitPretradeRejectScope,
}

impl OpenPitPretradeReject {
    pub(crate) fn from_reject(inner: &Reject) -> Self {
        Self {
            policy: OpenPitStringView::from_utf8(inner.policy.as_str()),
            reason: OpenPitStringView::from_utf8(inner.reason.as_str()),
            details: OpenPitStringView::from_utf8(inner.details.as_str()),
            user_data: inner.user_data as *mut c_void,
            code: export_reject_code(inner.code),
            scope: export_reject_scope(inner.scope.clone()),
        }
    }

    pub(crate) fn to_reject(self) -> Option<Reject> {
        Some(
            Reject::new(
                import_string(self.policy),
                import_reject_scope(self.scope)?,
                import_reject_code(self.code),
                import_string(self.reason),
                import_string(self.details),
            )
            .with_user_data(self.user_data as usize),
        )
    }
}

/// Caller-owned list of rejects.
pub struct OpenPitPretradeRejectList {
    pub(crate) items: Vec<Reject>,
}

fn import_reject_code(value: OpenPitPretradeRejectCode) -> RejectCode {
    match value {
        OPENPIT_PRETRADE_REJECT_CODE_MISSING_REQUIRED_FIELD => RejectCode::MissingRequiredField,
        OPENPIT_PRETRADE_REJECT_CODE_INVALID_FIELD_FORMAT => RejectCode::InvalidFieldFormat,
        OPENPIT_PRETRADE_REJECT_CODE_INVALID_FIELD_VALUE => RejectCode::InvalidFieldValue,
        OPENPIT_PRETRADE_REJECT_CODE_UNSUPPORTED_ORDER_TYPE => RejectCode::UnsupportedOrderType,
        OPENPIT_PRETRADE_REJECT_CODE_UNSUPPORTED_TIME_IN_FORCE => {
            RejectCode::UnsupportedTimeInForce
        }
        OPENPIT_PRETRADE_REJECT_CODE_UNSUPPORTED_ORDER_ATTRIBUTE => {
            RejectCode::UnsupportedOrderAttribute
        }
        OPENPIT_PRETRADE_REJECT_CODE_DUPLICATE_CLIENT_ORDER_ID => {
            RejectCode::DuplicateClientOrderId
        }
        OPENPIT_PRETRADE_REJECT_CODE_TOO_LATE_TO_ENTER => RejectCode::TooLateToEnter,
        OPENPIT_PRETRADE_REJECT_CODE_EXCHANGE_CLOSED => RejectCode::ExchangeClosed,
        OPENPIT_PRETRADE_REJECT_CODE_UNKNOWN_INSTRUMENT => RejectCode::UnknownInstrument,
        OPENPIT_PRETRADE_REJECT_CODE_UNKNOWN_ACCOUNT => RejectCode::UnknownAccount,
        OPENPIT_PRETRADE_REJECT_CODE_UNKNOWN_VENUE => RejectCode::UnknownVenue,
        OPENPIT_PRETRADE_REJECT_CODE_UNKNOWN_CLEARING_ACCOUNT => RejectCode::UnknownClearingAccount,
        OPENPIT_PRETRADE_REJECT_CODE_UNKNOWN_COLLATERAL_ASSET => RejectCode::UnknownCollateralAsset,
        OPENPIT_PRETRADE_REJECT_CODE_INSUFFICIENT_FUNDS => RejectCode::InsufficientFunds,
        OPENPIT_PRETRADE_REJECT_CODE_INSUFFICIENT_MARGIN => RejectCode::InsufficientMargin,
        OPENPIT_PRETRADE_REJECT_CODE_INSUFFICIENT_POSITION => RejectCode::InsufficientPosition,
        OPENPIT_PRETRADE_REJECT_CODE_CREDIT_LIMIT_EXCEEDED => RejectCode::CreditLimitExceeded,
        OPENPIT_PRETRADE_REJECT_CODE_RISK_LIMIT_EXCEEDED => RejectCode::RiskLimitExceeded,
        OPENPIT_PRETRADE_REJECT_CODE_ORDER_EXCEEDS_LIMIT => RejectCode::OrderExceedsLimit,
        OPENPIT_PRETRADE_REJECT_CODE_ORDER_QTY_EXCEEDS_LIMIT => RejectCode::OrderQtyExceedsLimit,
        OPENPIT_PRETRADE_REJECT_CODE_ORDER_NOTIONAL_EXCEEDS_LIMIT => {
            RejectCode::OrderNotionalExceedsLimit
        }
        OPENPIT_PRETRADE_REJECT_CODE_POSITION_LIMIT_EXCEEDED => RejectCode::PositionLimitExceeded,
        OPENPIT_PRETRADE_REJECT_CODE_CONCENTRATION_LIMIT_EXCEEDED => {
            RejectCode::ConcentrationLimitExceeded
        }
        OPENPIT_PRETRADE_REJECT_CODE_LEVERAGE_LIMIT_EXCEEDED => RejectCode::LeverageLimitExceeded,
        OPENPIT_PRETRADE_REJECT_CODE_RATE_LIMIT_EXCEEDED => RejectCode::RateLimitExceeded,
        OPENPIT_PRETRADE_REJECT_CODE_PNL_KILL_SWITCH_TRIGGERED => {
            RejectCode::PnlKillSwitchTriggered
        }
        OPENPIT_PRETRADE_REJECT_CODE_ACCOUNT_BLOCKED => RejectCode::AccountBlocked,
        OPENPIT_PRETRADE_REJECT_CODE_ACCOUNT_NOT_AUTHORIZED => RejectCode::AccountNotAuthorized,
        OPENPIT_PRETRADE_REJECT_CODE_COMPLIANCE_RESTRICTION => RejectCode::ComplianceRestriction,
        OPENPIT_PRETRADE_REJECT_CODE_INSTRUMENT_RESTRICTED => RejectCode::InstrumentRestricted,
        OPENPIT_PRETRADE_REJECT_CODE_JURISDICTION_RESTRICTION => {
            RejectCode::JurisdictionRestriction
        }
        OPENPIT_PRETRADE_REJECT_CODE_WASH_TRADE_PREVENTION => RejectCode::WashTradePrevention,
        OPENPIT_PRETRADE_REJECT_CODE_SELF_MATCH_PREVENTION => RejectCode::SelfMatchPrevention,
        OPENPIT_PRETRADE_REJECT_CODE_SHORT_SALE_RESTRICTION => RejectCode::ShortSaleRestriction,
        OPENPIT_PRETRADE_REJECT_CODE_RISK_CONFIGURATION_MISSING => {
            RejectCode::RiskConfigurationMissing
        }
        OPENPIT_PRETRADE_REJECT_CODE_REFERENCE_DATA_UNAVAILABLE => {
            RejectCode::ReferenceDataUnavailable
        }
        OPENPIT_PRETRADE_REJECT_CODE_ORDER_VALUE_CALCULATION_FAILED => {
            RejectCode::OrderValueCalculationFailed
        }
        OPENPIT_PRETRADE_REJECT_CODE_SYSTEM_UNAVAILABLE => RejectCode::SystemUnavailable,
        OPENPIT_PRETRADE_REJECT_CODE_MARK_PRICE_UNAVAILABLE => RejectCode::MarkPriceUnavailable,
        OPENPIT_PRETRADE_REJECT_CODE_ACCOUNT_ADJUSTMENT_BOUNDS_EXCEEDED => {
            RejectCode::AccountAdjustmentBoundsExceeded
        }
        OPENPIT_PRETRADE_REJECT_CODE_ARITHMETIC_OVERFLOW => RejectCode::ArithmeticOverflow,
        OPENPIT_PRETRADE_REJECT_CODE_CUSTOM => RejectCode::Custom,
        OPENPIT_PRETRADE_REJECT_CODE_OTHER => RejectCode::Other,
        _ => RejectCode::Other,
    }
}

fn export_reject_code(value: RejectCode) -> OpenPitPretradeRejectCode {
    match value {
        RejectCode::MissingRequiredField => OPENPIT_PRETRADE_REJECT_CODE_MISSING_REQUIRED_FIELD,
        RejectCode::InvalidFieldFormat => OPENPIT_PRETRADE_REJECT_CODE_INVALID_FIELD_FORMAT,
        RejectCode::InvalidFieldValue => OPENPIT_PRETRADE_REJECT_CODE_INVALID_FIELD_VALUE,
        RejectCode::UnsupportedOrderType => OPENPIT_PRETRADE_REJECT_CODE_UNSUPPORTED_ORDER_TYPE,
        RejectCode::UnsupportedTimeInForce => {
            OPENPIT_PRETRADE_REJECT_CODE_UNSUPPORTED_TIME_IN_FORCE
        }
        RejectCode::UnsupportedOrderAttribute => {
            OPENPIT_PRETRADE_REJECT_CODE_UNSUPPORTED_ORDER_ATTRIBUTE
        }
        RejectCode::DuplicateClientOrderId => {
            OPENPIT_PRETRADE_REJECT_CODE_DUPLICATE_CLIENT_ORDER_ID
        }
        RejectCode::TooLateToEnter => OPENPIT_PRETRADE_REJECT_CODE_TOO_LATE_TO_ENTER,
        RejectCode::ExchangeClosed => OPENPIT_PRETRADE_REJECT_CODE_EXCHANGE_CLOSED,
        RejectCode::UnknownInstrument => OPENPIT_PRETRADE_REJECT_CODE_UNKNOWN_INSTRUMENT,
        RejectCode::UnknownAccount => OPENPIT_PRETRADE_REJECT_CODE_UNKNOWN_ACCOUNT,
        RejectCode::UnknownVenue => OPENPIT_PRETRADE_REJECT_CODE_UNKNOWN_VENUE,
        RejectCode::UnknownClearingAccount => OPENPIT_PRETRADE_REJECT_CODE_UNKNOWN_CLEARING_ACCOUNT,
        RejectCode::UnknownCollateralAsset => OPENPIT_PRETRADE_REJECT_CODE_UNKNOWN_COLLATERAL_ASSET,
        RejectCode::InsufficientFunds => OPENPIT_PRETRADE_REJECT_CODE_INSUFFICIENT_FUNDS,
        RejectCode::InsufficientMargin => OPENPIT_PRETRADE_REJECT_CODE_INSUFFICIENT_MARGIN,
        RejectCode::InsufficientPosition => OPENPIT_PRETRADE_REJECT_CODE_INSUFFICIENT_POSITION,
        RejectCode::CreditLimitExceeded => OPENPIT_PRETRADE_REJECT_CODE_CREDIT_LIMIT_EXCEEDED,
        RejectCode::RiskLimitExceeded => OPENPIT_PRETRADE_REJECT_CODE_RISK_LIMIT_EXCEEDED,
        RejectCode::OrderExceedsLimit => OPENPIT_PRETRADE_REJECT_CODE_ORDER_EXCEEDS_LIMIT,
        RejectCode::OrderQtyExceedsLimit => OPENPIT_PRETRADE_REJECT_CODE_ORDER_QTY_EXCEEDS_LIMIT,
        RejectCode::OrderNotionalExceedsLimit => {
            OPENPIT_PRETRADE_REJECT_CODE_ORDER_NOTIONAL_EXCEEDS_LIMIT
        }
        RejectCode::PositionLimitExceeded => OPENPIT_PRETRADE_REJECT_CODE_POSITION_LIMIT_EXCEEDED,
        RejectCode::ConcentrationLimitExceeded => {
            OPENPIT_PRETRADE_REJECT_CODE_CONCENTRATION_LIMIT_EXCEEDED
        }
        RejectCode::LeverageLimitExceeded => OPENPIT_PRETRADE_REJECT_CODE_LEVERAGE_LIMIT_EXCEEDED,
        RejectCode::RateLimitExceeded => OPENPIT_PRETRADE_REJECT_CODE_RATE_LIMIT_EXCEEDED,
        RejectCode::PnlKillSwitchTriggered => {
            OPENPIT_PRETRADE_REJECT_CODE_PNL_KILL_SWITCH_TRIGGERED
        }
        RejectCode::AccountBlocked => OPENPIT_PRETRADE_REJECT_CODE_ACCOUNT_BLOCKED,
        RejectCode::AccountNotAuthorized => OPENPIT_PRETRADE_REJECT_CODE_ACCOUNT_NOT_AUTHORIZED,
        RejectCode::ComplianceRestriction => OPENPIT_PRETRADE_REJECT_CODE_COMPLIANCE_RESTRICTION,
        RejectCode::InstrumentRestricted => OPENPIT_PRETRADE_REJECT_CODE_INSTRUMENT_RESTRICTED,
        RejectCode::JurisdictionRestriction => {
            OPENPIT_PRETRADE_REJECT_CODE_JURISDICTION_RESTRICTION
        }
        RejectCode::WashTradePrevention => OPENPIT_PRETRADE_REJECT_CODE_WASH_TRADE_PREVENTION,
        RejectCode::SelfMatchPrevention => OPENPIT_PRETRADE_REJECT_CODE_SELF_MATCH_PREVENTION,
        RejectCode::ShortSaleRestriction => OPENPIT_PRETRADE_REJECT_CODE_SHORT_SALE_RESTRICTION,
        RejectCode::RiskConfigurationMissing => {
            OPENPIT_PRETRADE_REJECT_CODE_RISK_CONFIGURATION_MISSING
        }
        RejectCode::ReferenceDataUnavailable => {
            OPENPIT_PRETRADE_REJECT_CODE_REFERENCE_DATA_UNAVAILABLE
        }
        RejectCode::OrderValueCalculationFailed => {
            OPENPIT_PRETRADE_REJECT_CODE_ORDER_VALUE_CALCULATION_FAILED
        }
        RejectCode::SystemUnavailable => OPENPIT_PRETRADE_REJECT_CODE_SYSTEM_UNAVAILABLE,
        RejectCode::MarkPriceUnavailable => OPENPIT_PRETRADE_REJECT_CODE_MARK_PRICE_UNAVAILABLE,
        RejectCode::AccountAdjustmentBoundsExceeded => {
            OPENPIT_PRETRADE_REJECT_CODE_ACCOUNT_ADJUSTMENT_BOUNDS_EXCEEDED
        }
        RejectCode::ArithmeticOverflow => OPENPIT_PRETRADE_REJECT_CODE_ARITHMETIC_OVERFLOW,
        RejectCode::Custom => OPENPIT_PRETRADE_REJECT_CODE_CUSTOM,
        RejectCode::Other => OPENPIT_PRETRADE_REJECT_CODE_OTHER,
        _ => OPENPIT_PRETRADE_REJECT_CODE_OTHER,
    }
}

fn export_reject_scope(value: RejectScope) -> OpenPitPretradeRejectScope {
    match value {
        RejectScope::Order => OPENPIT_PRETRADE_REJECT_SCOPE_ORDER,
        RejectScope::Account => OPENPIT_PRETRADE_REJECT_SCOPE_ACCOUNT,
    }
}

fn import_reject_scope(value: OpenPitPretradeRejectScope) -> Option<RejectScope> {
    match value {
        OPENPIT_PRETRADE_REJECT_SCOPE_ORDER => Some(RejectScope::Order),
        OPENPIT_PRETRADE_REJECT_SCOPE_ACCOUNT => Some(RejectScope::Account),
        _ => None,
    }
}

fn import_string(ptr: OpenPitStringView) -> String {
    if ptr.ptr.is_null() {
        return String::default();
    }

    let bytes = unsafe { std::slice::from_raw_parts(ptr.ptr, ptr.len) };
    String::from_utf8_lossy(bytes).into_owned()
}

pub(crate) fn rejects_to_list_owned(values: Rejects) -> OpenPitPretradeRejectList {
    let mut out = Vec::with_capacity(values.len());
    for reject in values.iter().cloned() {
        out.push(reject);
    }
    OpenPitPretradeRejectList { items: out }
}

#[no_mangle]
/// Creates a caller-owned reject list with preallocated capacity.
///
/// `reserve` is the requested number of elements to preallocate.
///
/// Contract:
/// - returns a new caller-owned list;
/// - release it with `openpit_pretrade_destroy_reject_list`;
/// - this function always succeeds.
pub extern "C" fn openpit_pretrade_create_reject_list(
    reserve: usize,
) -> *mut OpenPitPretradeRejectList {
    Box::into_raw(Box::new(OpenPitPretradeRejectList {
        items: Vec::with_capacity(reserve),
    }))
}

#[no_mangle]
/// Releases a caller-owned reject list.
///
/// Contract:
/// - passing null is allowed;
/// - this function always succeeds.
pub extern "C" fn openpit_pretrade_destroy_reject_list(rejects: *mut OpenPitPretradeRejectList) {
    if rejects.is_null() {
        return;
    }
    unsafe { drop(Box::from_raw(rejects)) };
}

#[no_mangle]
/// Appends one reject to the list by copying its payload.
///
/// Contract:
/// - `list` must be a valid non-null pointer;
/// - string views in `reject` are copied before this function returns;
/// - returns `true` after appending a reject with a valid scope;
/// - returns `false` for an unknown scope and leaves the list unchanged;
/// - violating the pointer contract aborts the call.
pub extern "C" fn openpit_pretrade_reject_list_push(
    list: *mut OpenPitPretradeRejectList,
    reject: OpenPitPretradeReject,
) -> bool {
    assert!(!list.is_null(), "reject list pointer is null");
    let Some(reject) = reject.to_reject() else {
        return false;
    };
    let list = unsafe { &mut *list };
    list.items.push(reject);
    true
}

#[no_mangle]
/// Returns the number of rejects in the list.
///
/// Contract:
/// - `list` must be a valid non-null pointer;
/// - this function never fails;
/// - violating the pointer contract aborts the call.
pub extern "C" fn openpit_pretrade_reject_list_len(
    list: *const OpenPitPretradeRejectList,
) -> usize {
    assert!(!list.is_null(), "reject list pointer is null");
    let list = unsafe { &*list };
    list.items.len()
}

#[no_mangle]
/// Copies a non-owning reject view at `index` into `out_reject`.
///
/// The copied view borrows string memory from `list`.
///
/// Contract:
/// - `list` must be a valid non-null pointer;
/// - `out_reject` must be a valid non-null pointer;
/// - returns `true` when a value exists and was copied;
/// - returns `false` when `index` is out of bounds and does not write
///   `out_reject`;
/// - the copied view remains valid while `list` is alive and unchanged;
/// - this function never fails;
/// - violating the pointer contract aborts the call.
pub extern "C" fn openpit_pretrade_reject_list_get(
    list: *const OpenPitPretradeRejectList,
    index: usize,
    out_reject: *mut OpenPitPretradeReject,
) -> bool {
    assert!(!list.is_null(), "reject list pointer is null");
    assert!(!out_reject.is_null(), "reject output pointer is null");
    let list = unsafe { &*list };
    let Some(reject) = list.items.get(index) else {
        return false;
    };
    unsafe { *out_reject = OpenPitPretradeReject::from_reject(reject) };
    true
}

/// Single account-block record returned by kill-switch checks.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpenPitPretradeAccountBlock {
    /// Policy name that produced the block.
    pub policy: OpenPitStringView,
    /// Human-readable reject reason.
    pub reason: OpenPitStringView,
    /// Case-specific reject details.
    pub details: OpenPitStringView,
    /// Opaque caller-defined token.
    ///
    /// The SDK never inspects, dereferences, or frees this value. Its meaning,
    /// lifetime, and thread-safety are the caller's responsibility. `0` / null
    /// means "not set". See the project Threading Contract for the full lifetime
    /// model.
    pub user_data: *mut c_void,
    /// Stable machine-readable reject code.
    pub code: OpenPitPretradeRejectCode,
}

impl OpenPitPretradeAccountBlock {
    pub(crate) fn from_block(inner: &AccountBlock) -> Self {
        Self {
            policy: OpenPitStringView::from_utf8(inner.policy.as_str()),
            reason: OpenPitStringView::from_utf8(inner.reason.as_str()),
            details: OpenPitStringView::from_utf8(inner.details.as_str()),
            user_data: inner.user_data as *mut c_void,
            code: export_reject_code(inner.code),
        }
    }

    pub(crate) fn to_block(self) -> AccountBlock {
        AccountBlock::new(
            import_string(self.policy),
            import_reject_code(self.code),
            import_string(self.reason),
            import_string(self.details),
        )
        .with_user_data(self.user_data as usize)
    }
}

/// Caller-owned list of account blocks.
pub struct OpenPitPretradeAccountBlockList {
    pub(crate) items: Vec<AccountBlock>,
}

pub(crate) fn blocks_to_list_owned(values: Vec<AccountBlock>) -> OpenPitPretradeAccountBlockList {
    OpenPitPretradeAccountBlockList { items: values }
}

#[no_mangle]
/// Creates a caller-owned account-block list with preallocated capacity.
///
/// `reserve` is the requested number of elements to preallocate.
///
/// Contract:
/// - returns a new caller-owned list;
/// - release it with `openpit_pretrade_destroy_account_block_list`;
/// - this function always succeeds.
pub extern "C" fn openpit_pretrade_create_account_block_list(
    reserve: usize,
) -> *mut OpenPitPretradeAccountBlockList {
    Box::into_raw(Box::new(OpenPitPretradeAccountBlockList {
        items: Vec::with_capacity(reserve),
    }))
}

#[no_mangle]
/// Releases a caller-owned account-block list.
///
/// Contract:
/// - passing null is allowed;
/// - this function always succeeds.
pub extern "C" fn openpit_pretrade_destroy_account_block_list(
    blocks: *mut OpenPitPretradeAccountBlockList,
) {
    if blocks.is_null() {
        return;
    }
    unsafe { drop(Box::from_raw(blocks)) };
}

#[no_mangle]
/// Appends one account block to the list by copying its payload.
///
/// Contract:
/// - `list` must be a valid non-null pointer;
/// - string views in `block` are copied before this function returns;
/// - this function never fails;
/// - violating the pointer contract aborts the call.
pub extern "C" fn openpit_pretrade_account_block_list_push(
    list: *mut OpenPitPretradeAccountBlockList,
    block: OpenPitPretradeAccountBlock,
) {
    assert!(!list.is_null(), "account block list pointer is null");
    let list = unsafe { &mut *list };
    list.items.push(block.to_block());
}

#[no_mangle]
/// Returns the number of account blocks in the list.
///
/// Contract:
/// - `list` must be a valid non-null pointer;
/// - this function never fails;
/// - violating the pointer contract aborts the call.
pub extern "C" fn openpit_pretrade_account_block_list_len(
    list: *const OpenPitPretradeAccountBlockList,
) -> usize {
    assert!(!list.is_null(), "account block list pointer is null");
    let list = unsafe { &*list };
    list.items.len()
}

#[no_mangle]
/// Copies a non-owning account-block view at `index` into `out_block`.
///
/// The copied view borrows string memory from `list`.
///
/// Contract:
/// - `list` must be a valid non-null pointer;
/// - `out_block` must be a valid non-null pointer;
/// - returns `true` when a value exists and was copied;
/// - returns `false` when `index` is out of bounds and does not write
///   `out_block`;
/// - the copied view remains valid while `list` is alive and unchanged;
/// - this function never fails;
/// - violating the pointer contract aborts the call.
pub extern "C" fn openpit_pretrade_account_block_list_get(
    list: *const OpenPitPretradeAccountBlockList,
    index: usize,
    out_block: *mut OpenPitPretradeAccountBlock,
) -> bool {
    assert!(!list.is_null(), "account block list pointer is null");
    assert!(!out_block.is_null(), "account block output pointer is null");
    let list = unsafe { &*list };
    let Some(block) = list.items.get(index) else {
        return false;
    };
    unsafe { *out_block = OpenPitPretradeAccountBlock::from_block(block) };
    true
}

#[cfg(test)]
mod tests {
    use crate::OpenPitStringView;
    use openpit::pretrade::{AccountBlock, Reject, RejectCode, RejectScope};

    use super::*;

    fn string_view_to_string(view: OpenPitStringView) -> String {
        if view.ptr.is_null() {
            return String::new();
        }
        let bytes = unsafe { std::slice::from_raw_parts(view.ptr, view.len) };
        std::str::from_utf8(bytes).expect("utf8").to_string()
    }

    #[test]
    fn reject_list_destroy_is_null_safe() {
        openpit_pretrade_destroy_reject_list(std::ptr::null_mut());
    }

    #[test]
    fn export_reject_keeps_borrowed_views() {
        let reject = Reject::new(
            "test_policy",
            RejectScope::Order,
            RejectCode::Other,
            "reason".to_string(),
            "details".to_string(),
        );
        let exported = OpenPitPretradeReject::from_reject(&reject);
        assert_eq!(string_view_to_string(exported.policy), "test_policy");
        assert_eq!(string_view_to_string(exported.reason), "reason");
        assert_eq!(string_view_to_string(exported.details), "details");
        assert_eq!(exported.user_data, std::ptr::null_mut());
    }

    #[test]
    fn reject_list_push_len_get_roundtrip() {
        let list = openpit_pretrade_create_reject_list(1);
        let reject = OpenPitPretradeReject {
            policy: OpenPitStringView::from_utf8("policy"),
            reason: OpenPitStringView::from_utf8("reason"),
            details: OpenPitStringView::from_utf8("details"),
            user_data: 55usize as *mut std::ffi::c_void,
            code: OPENPIT_PRETRADE_REJECT_CODE_OTHER,
            scope: OPENPIT_PRETRADE_REJECT_SCOPE_ORDER,
        };
        assert!(openpit_pretrade_reject_list_push(list, reject));
        assert_eq!(openpit_pretrade_reject_list_len(list), 1);
        let stored = unsafe { &*list };
        assert_eq!(stored.items[0].user_data, 55usize);
        let mut first = OpenPitPretradeReject {
            policy: OpenPitStringView::not_set(),
            reason: OpenPitStringView::not_set(),
            details: OpenPitStringView::not_set(),
            user_data: std::ptr::null_mut(),
            code: OPENPIT_PRETRADE_REJECT_CODE_OTHER,
            scope: OPENPIT_PRETRADE_REJECT_SCOPE_ORDER,
        };
        assert!(openpit_pretrade_reject_list_get(list, 0, &mut first));
        assert_eq!(first.code, OPENPIT_PRETRADE_REJECT_CODE_OTHER);
        assert_eq!(first.user_data, 55usize as *mut std::ffi::c_void);
        assert_eq!(string_view_to_string(first.policy), "policy");
        assert!(!openpit_pretrade_reject_list_get(list, 1, &mut first));
        openpit_pretrade_destroy_reject_list(list);
    }

    #[test]
    fn import_reject_copies_view_payload() {
        let view = OpenPitPretradeReject {
            policy: OpenPitStringView::from_utf8("policy"),
            reason: OpenPitStringView::from_utf8("reason"),
            details: OpenPitStringView::from_utf8("details"),
            user_data: 77usize as *mut std::ffi::c_void,
            code: OPENPIT_PRETRADE_REJECT_CODE_RATE_LIMIT_EXCEEDED,
            scope: OPENPIT_PRETRADE_REJECT_SCOPE_ACCOUNT,
        };
        let imported = view.to_reject().expect("valid scope");
        assert_eq!(imported.policy, "policy");
        assert_eq!(imported.reason, "reason");
        assert_eq!(imported.details, "details");
        assert_eq!(imported.user_data, 77usize);
        assert_eq!(imported.code, RejectCode::RateLimitExceeded);
        assert_eq!(imported.scope, RejectScope::Account);
    }

    #[test]
    fn reject_code_roundtrip_covers_all_ffi_variants() {
        let all = [
            OPENPIT_PRETRADE_REJECT_CODE_MISSING_REQUIRED_FIELD,
            OPENPIT_PRETRADE_REJECT_CODE_INVALID_FIELD_FORMAT,
            OPENPIT_PRETRADE_REJECT_CODE_INVALID_FIELD_VALUE,
            OPENPIT_PRETRADE_REJECT_CODE_UNSUPPORTED_ORDER_TYPE,
            OPENPIT_PRETRADE_REJECT_CODE_UNSUPPORTED_TIME_IN_FORCE,
            OPENPIT_PRETRADE_REJECT_CODE_UNSUPPORTED_ORDER_ATTRIBUTE,
            OPENPIT_PRETRADE_REJECT_CODE_DUPLICATE_CLIENT_ORDER_ID,
            OPENPIT_PRETRADE_REJECT_CODE_TOO_LATE_TO_ENTER,
            OPENPIT_PRETRADE_REJECT_CODE_EXCHANGE_CLOSED,
            OPENPIT_PRETRADE_REJECT_CODE_UNKNOWN_INSTRUMENT,
            OPENPIT_PRETRADE_REJECT_CODE_UNKNOWN_ACCOUNT,
            OPENPIT_PRETRADE_REJECT_CODE_UNKNOWN_VENUE,
            OPENPIT_PRETRADE_REJECT_CODE_UNKNOWN_CLEARING_ACCOUNT,
            OPENPIT_PRETRADE_REJECT_CODE_UNKNOWN_COLLATERAL_ASSET,
            OPENPIT_PRETRADE_REJECT_CODE_INSUFFICIENT_FUNDS,
            OPENPIT_PRETRADE_REJECT_CODE_INSUFFICIENT_MARGIN,
            OPENPIT_PRETRADE_REJECT_CODE_INSUFFICIENT_POSITION,
            OPENPIT_PRETRADE_REJECT_CODE_CREDIT_LIMIT_EXCEEDED,
            OPENPIT_PRETRADE_REJECT_CODE_RISK_LIMIT_EXCEEDED,
            OPENPIT_PRETRADE_REJECT_CODE_ORDER_EXCEEDS_LIMIT,
            OPENPIT_PRETRADE_REJECT_CODE_ORDER_QTY_EXCEEDS_LIMIT,
            OPENPIT_PRETRADE_REJECT_CODE_ORDER_NOTIONAL_EXCEEDS_LIMIT,
            OPENPIT_PRETRADE_REJECT_CODE_POSITION_LIMIT_EXCEEDED,
            OPENPIT_PRETRADE_REJECT_CODE_CONCENTRATION_LIMIT_EXCEEDED,
            OPENPIT_PRETRADE_REJECT_CODE_LEVERAGE_LIMIT_EXCEEDED,
            OPENPIT_PRETRADE_REJECT_CODE_RATE_LIMIT_EXCEEDED,
            OPENPIT_PRETRADE_REJECT_CODE_PNL_KILL_SWITCH_TRIGGERED,
            OPENPIT_PRETRADE_REJECT_CODE_ACCOUNT_BLOCKED,
            OPENPIT_PRETRADE_REJECT_CODE_ACCOUNT_NOT_AUTHORIZED,
            OPENPIT_PRETRADE_REJECT_CODE_COMPLIANCE_RESTRICTION,
            OPENPIT_PRETRADE_REJECT_CODE_INSTRUMENT_RESTRICTED,
            OPENPIT_PRETRADE_REJECT_CODE_JURISDICTION_RESTRICTION,
            OPENPIT_PRETRADE_REJECT_CODE_WASH_TRADE_PREVENTION,
            OPENPIT_PRETRADE_REJECT_CODE_SELF_MATCH_PREVENTION,
            OPENPIT_PRETRADE_REJECT_CODE_SHORT_SALE_RESTRICTION,
            OPENPIT_PRETRADE_REJECT_CODE_RISK_CONFIGURATION_MISSING,
            OPENPIT_PRETRADE_REJECT_CODE_REFERENCE_DATA_UNAVAILABLE,
            OPENPIT_PRETRADE_REJECT_CODE_ORDER_VALUE_CALCULATION_FAILED,
            OPENPIT_PRETRADE_REJECT_CODE_SYSTEM_UNAVAILABLE,
            OPENPIT_PRETRADE_REJECT_CODE_MARK_PRICE_UNAVAILABLE,
            OPENPIT_PRETRADE_REJECT_CODE_ACCOUNT_ADJUSTMENT_BOUNDS_EXCEEDED,
            OPENPIT_PRETRADE_REJECT_CODE_ARITHMETIC_OVERFLOW,
            OPENPIT_PRETRADE_REJECT_CODE_CUSTOM,
            OPENPIT_PRETRADE_REJECT_CODE_OTHER,
        ];
        for code in all {
            let domain = import_reject_code(code);
            let ffi = export_reject_code(domain);
            assert_eq!(ffi, code);
        }
    }

    #[test]
    fn reject_list_push_rejects_unknown_scope_without_appending() {
        let list = openpit_pretrade_create_reject_list(1);
        let reject = OpenPitPretradeReject {
            policy: OpenPitStringView::from_utf8("policy"),
            reason: OpenPitStringView::from_utf8("reason"),
            details: OpenPitStringView::from_utf8("details"),
            user_data: std::ptr::null_mut(),
            code: OPENPIT_PRETRADE_REJECT_CODE_OTHER,
            scope: u8::MAX,
        };

        assert!(!openpit_pretrade_reject_list_push(list, reject));
        assert_eq!(openpit_pretrade_reject_list_len(list), 0);
        openpit_pretrade_destroy_reject_list(list);
    }

    #[test]
    fn unknown_reject_code_maps_to_other() {
        assert_eq!(import_reject_code(u16::MAX), RejectCode::Other);

        let block = OpenPitPretradeAccountBlock {
            policy: OpenPitStringView::from_utf8("policy"),
            reason: OpenPitStringView::from_utf8("reason"),
            details: OpenPitStringView::from_utf8("details"),
            user_data: std::ptr::null_mut(),
            code: u16::MAX,
        };
        assert_eq!(block.to_block().code, RejectCode::Other);
    }

    #[test]
    fn account_block_list_destroy_is_null_safe() {
        openpit_pretrade_destroy_account_block_list(std::ptr::null_mut());
    }

    #[test]
    fn account_block_list_push_len_get_roundtrip() {
        use super::{
            openpit_pretrade_account_block_list_get, openpit_pretrade_account_block_list_len,
            openpit_pretrade_account_block_list_push,
        };

        let list = openpit_pretrade_create_account_block_list(1);
        let block = OpenPitPretradeAccountBlock {
            policy: OpenPitStringView::from_utf8("policy"),
            reason: OpenPitStringView::from_utf8("reason"),
            details: OpenPitStringView::from_utf8("details"),
            user_data: 42usize as *mut std::ffi::c_void,
            code: OPENPIT_PRETRADE_REJECT_CODE_PNL_KILL_SWITCH_TRIGGERED,
        };
        openpit_pretrade_account_block_list_push(list, block);
        assert_eq!(openpit_pretrade_account_block_list_len(list), 1);
        let stored = unsafe { &*list };
        assert_eq!(stored.items[0].user_data, 42usize);
        let mut out = OpenPitPretradeAccountBlock {
            policy: OpenPitStringView::not_set(),
            reason: OpenPitStringView::not_set(),
            details: OpenPitStringView::not_set(),
            user_data: std::ptr::null_mut(),
            code: OPENPIT_PRETRADE_REJECT_CODE_OTHER,
        };
        assert!(openpit_pretrade_account_block_list_get(list, 0, &mut out));
        assert_eq!(
            out.code,
            OPENPIT_PRETRADE_REJECT_CODE_PNL_KILL_SWITCH_TRIGGERED
        );
        assert_eq!(out.user_data, 42usize as *mut std::ffi::c_void);
        assert_eq!(string_view_to_string(out.policy), "policy");
        assert!(!openpit_pretrade_account_block_list_get(list, 1, &mut out));
        openpit_pretrade_destroy_account_block_list(list);
    }

    #[test]
    fn account_block_roundtrip_preserves_all_fields() {
        use super::OpenPitPretradeAccountBlock;

        let block = AccountBlock::new(
            "PnlKillSwitch",
            RejectCode::PnlKillSwitchTriggered,
            "daily loss limit breached",
            "loss exceeded configured threshold",
        )
        .with_user_data(99usize);

        let exported = OpenPitPretradeAccountBlock::from_block(&block);
        assert_eq!(string_view_to_string(exported.policy), "PnlKillSwitch");
        assert_eq!(
            string_view_to_string(exported.reason),
            "daily loss limit breached"
        );
        assert_eq!(
            string_view_to_string(exported.details),
            "loss exceeded configured threshold"
        );
        assert_eq!(exported.user_data, 99usize as *mut std::ffi::c_void);
        assert_eq!(
            exported.code,
            OPENPIT_PRETRADE_REJECT_CODE_PNL_KILL_SWITCH_TRIGGERED
        );

        let imported = exported.to_block();
        assert_eq!(imported.policy, "PnlKillSwitch");
        assert_eq!(imported.reason, "daily loss limit breached");
        assert_eq!(imported.details, "loss exceeded configured threshold");
        assert_eq!(imported.user_data, 99usize);
        assert_eq!(imported.code, RejectCode::PnlKillSwitchTriggered);
    }

    // A real spot-funds insufficient-funds reject, produced by the core engine
    // for a sentinel account and then exported across the C ABI, must not carry
    // the account id in its reason/details views. The digit run 424242 is the
    // sentinel account id; the order operands never contain it.
    #[test]
    fn exported_reject_does_not_leak_account_id() {
        use openpit::param::{AccountId, Asset, Price, Quantity, Side, TradeAmount};
        use openpit::pretrade::policies::{
            SpotFundsPolicy, SpotFundsPricingSource, SpotFundsSettings,
        };
        use openpit::{Engine, FullSync, Instrument, OrderOperation, SpotFundsMarketData};

        let builder = Engine::builder::<
            OrderOperation,
            crate::execution_report::ExecutionReport,
            crate::account_adjustment::AccountAdjustment,
        >()
        .full_sync();
        let settings = SpotFundsSettings::new(0, SpotFundsPricingSource::Mark, std::iter::empty())
            .expect("spot-funds settings must build");
        let policy = SpotFundsPolicy::<FullSync, FullSync>::new(
            settings,
            None::<SpotFundsMarketData<FullSync>>,
            builder.storage_builder(),
        );
        let engine = builder
            .pre_trade(policy)
            .build()
            .expect("engine must build");

        // Buy 4 AAPL @ 200 = 800 notional against an unfunded account rejects
        // with insufficient funds.
        let order = OrderOperation {
            instrument: Instrument::new(
                Asset::new("AAPL").expect("valid asset"),
                Asset::new("USD").expect("valid asset"),
            ),
            account_id: AccountId::from_u64(424242),
            side: Side::Buy,
            trade_amount: TradeAmount::Quantity(Quantity::from_str("4").expect("valid quantity")),
            price: Some(Price::from_str("200").expect("valid price")),
        };
        let Err(rejects) = engine.execute_pre_trade(order) else {
            panic!("under-funded buy must reject");
        };
        assert_eq!(rejects[0].code, RejectCode::InsufficientFunds);

        let exported = OpenPitPretradeReject::from_reject(&rejects[0]);
        let reason = string_view_to_string(exported.reason);
        let details = string_view_to_string(exported.details);
        assert!(
            !reason.contains("424242"),
            "reason leaked account id: {reason}"
        );
        assert!(
            !details.contains("424242"),
            "details leaked account id: {details}"
        );
    }
}
