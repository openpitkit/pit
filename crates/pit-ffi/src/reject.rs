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

use crate::PitStringView;
use openpit::pretrade::{Reject, RejectCode, RejectScope, Rejects};
use std::ffi::c_void;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Broad area to which a reject applies.
///
/// Valid values: `Order` (1), `Account` (2). Zero is not a valid scope value;
/// the caller must always set this field explicitly.
pub enum PitRejectScope {
    /// The reject applies to one order or order-like request.
    Order = 1,
    /// The reject applies to account state rather than to one order only.
    Account = 2,
}

#[repr(u16)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Stable classification code for a reject.
///
/// Read this first when you need machine-readable handling. The textual fields
/// in [`PitReject`] provide operator-facing explanation and extra context.
///
/// Valid codes are `1..=39` and `255` (`Other`). Unknown incoming codes are
/// mapped to `Other` (`255`).
pub enum PitRejectCode {
    /// A required field is absent.
    MissingRequiredField = 1,
    /// A field cannot be parsed from the supplied wire value.
    InvalidFieldFormat = 2,
    /// A field is syntactically valid but semantically unacceptable.
    InvalidFieldValue = 3,
    /// The requested order type is not supported.
    UnsupportedOrderType = 4,
    /// The requested time-in-force is not supported.
    UnsupportedTimeInForce = 5,
    /// Another order attribute is unsupported.
    UnsupportedOrderAttribute = 6,
    /// The client order identifier duplicates an active order.
    DuplicateClientOrderId = 7,
    /// The order arrived after the allowed entry deadline.
    TooLateToEnter = 8,
    /// Trading is closed for the relevant venue or session.
    ExchangeClosed = 9,
    /// The instrument cannot be resolved.
    UnknownInstrument = 10,
    /// The account cannot be resolved.
    UnknownAccount = 11,
    /// The venue cannot be resolved.
    UnknownVenue = 12,
    /// The clearing account cannot be resolved.
    UnknownClearingAccount = 13,
    /// The collateral asset cannot be resolved.
    UnknownCollateralAsset = 14,
    /// Available balance is insufficient.
    InsufficientFunds = 15,
    /// Available margin is insufficient.
    InsufficientMargin = 16,
    /// Available position is insufficient.
    InsufficientPosition = 17,
    /// A credit limit was exceeded.
    CreditLimitExceeded = 18,
    /// A risk limit was exceeded.
    RiskLimitExceeded = 19,
    /// The order exceeds a generic configured limit.
    OrderExceedsLimit = 20,
    /// The order quantity exceeds a configured limit.
    OrderQtyExceedsLimit = 21,
    /// The order notional exceeds a configured limit.
    OrderNotionalExceedsLimit = 22,
    /// The resulting position exceeds a configured limit.
    PositionLimitExceeded = 23,
    /// Concentration constraints were violated.
    ConcentrationLimitExceeded = 24,
    /// Leverage constraints were violated.
    LeverageLimitExceeded = 25,
    /// The request rate exceeded a configured limit.
    RateLimitExceeded = 26,
    /// A loss barrier has blocked further risk-taking.
    PnlKillSwitchTriggered = 27,
    /// The account is blocked.
    AccountBlocked = 28,
    /// The account is not authorized for this action.
    AccountNotAuthorized = 29,
    /// A compliance restriction blocked the action.
    ComplianceRestriction = 30,
    /// The instrument is restricted.
    InstrumentRestricted = 31,
    /// A jurisdiction restriction blocked the action.
    JurisdictionRestriction = 32,
    /// The action would violate wash-trade prevention.
    WashTradePrevention = 33,
    /// The action would violate self-match prevention.
    SelfMatchPrevention = 34,
    /// Short-sale restriction blocked the action.
    ShortSaleRestriction = 35,
    /// Required risk configuration is missing.
    RiskConfigurationMissing = 36,
    /// Required reference data is unavailable.
    ReferenceDataUnavailable = 37,
    /// The system could not compute an order value needed for validation.
    OrderValueCalculationFailed = 38,
    /// A required service or subsystem is unavailable.
    SystemUnavailable = 39,
    /// Reserved discriminant for caller-defined reject classes.
    ///
    /// Use together with `Reject::with_user_data` to attach a caller-defined
    /// payload that the receiving code can decode. The SDK does not interpret
    /// this code beyond mapping it to FFI value 254.
    Custom = 254,
    /// A catch-all code for rejects that do not fit a more specific class.
    Other = 255,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Single rejection record returned by checks.
pub struct PitReject {
    /// Policy name that produced the reject.
    pub policy: PitStringView,
    /// Human-readable reject reason.
    pub reason: PitStringView,
    /// Case-specific reject details.
    pub details: PitStringView,
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
    pub code: PitRejectCode,
    /// Reject scope.
    pub scope: PitRejectScope,
}

impl PitReject {
    pub(crate) fn from_reject(inner: &Reject) -> Self {
        Self {
            policy: PitStringView::from_utf8(inner.policy.as_str()),
            reason: PitStringView::from_utf8(inner.reason.as_str()),
            details: PitStringView::from_utf8(inner.details.as_str()),
            user_data: inner.user_data as *mut c_void,
            code: PitRejectCode::from(inner.code),
            scope: export_reject_scope(inner.scope.clone()),
        }
    }

    pub(crate) fn to_reject(self) -> Reject {
        Reject::new(
            import_string(self.policy),
            import_reject_scope(self.scope),
            RejectCode::from(self.code),
            import_string(self.reason),
            import_string(self.details),
        )
        .with_user_data(self.user_data as usize)
    }
}

/// Caller-owned list of rejects.
pub struct PitRejectList {
    pub(crate) items: Vec<Reject>,
}

impl From<PitRejectCode> for RejectCode {
    fn from(value: PitRejectCode) -> Self {
        match value {
            PitRejectCode::MissingRequiredField => Self::MissingRequiredField,
            PitRejectCode::InvalidFieldFormat => Self::InvalidFieldFormat,
            PitRejectCode::InvalidFieldValue => Self::InvalidFieldValue,
            PitRejectCode::UnsupportedOrderType => Self::UnsupportedOrderType,
            PitRejectCode::UnsupportedTimeInForce => Self::UnsupportedTimeInForce,
            PitRejectCode::UnsupportedOrderAttribute => Self::UnsupportedOrderAttribute,
            PitRejectCode::DuplicateClientOrderId => Self::DuplicateClientOrderId,
            PitRejectCode::TooLateToEnter => Self::TooLateToEnter,
            PitRejectCode::ExchangeClosed => Self::ExchangeClosed,
            PitRejectCode::UnknownInstrument => Self::UnknownInstrument,
            PitRejectCode::UnknownAccount => Self::UnknownAccount,
            PitRejectCode::UnknownVenue => Self::UnknownVenue,
            PitRejectCode::UnknownClearingAccount => Self::UnknownClearingAccount,
            PitRejectCode::UnknownCollateralAsset => Self::UnknownCollateralAsset,
            PitRejectCode::InsufficientFunds => Self::InsufficientFunds,
            PitRejectCode::InsufficientMargin => Self::InsufficientMargin,
            PitRejectCode::InsufficientPosition => Self::InsufficientPosition,
            PitRejectCode::CreditLimitExceeded => Self::CreditLimitExceeded,
            PitRejectCode::RiskLimitExceeded => Self::RiskLimitExceeded,
            PitRejectCode::OrderExceedsLimit => Self::OrderExceedsLimit,
            PitRejectCode::OrderQtyExceedsLimit => Self::OrderQtyExceedsLimit,
            PitRejectCode::OrderNotionalExceedsLimit => Self::OrderNotionalExceedsLimit,
            PitRejectCode::PositionLimitExceeded => Self::PositionLimitExceeded,
            PitRejectCode::ConcentrationLimitExceeded => Self::ConcentrationLimitExceeded,
            PitRejectCode::LeverageLimitExceeded => Self::LeverageLimitExceeded,
            PitRejectCode::RateLimitExceeded => Self::RateLimitExceeded,
            PitRejectCode::PnlKillSwitchTriggered => Self::PnlKillSwitchTriggered,
            PitRejectCode::AccountBlocked => Self::AccountBlocked,
            PitRejectCode::AccountNotAuthorized => Self::AccountNotAuthorized,
            PitRejectCode::ComplianceRestriction => Self::ComplianceRestriction,
            PitRejectCode::InstrumentRestricted => Self::InstrumentRestricted,
            PitRejectCode::JurisdictionRestriction => Self::JurisdictionRestriction,
            PitRejectCode::WashTradePrevention => Self::WashTradePrevention,
            PitRejectCode::SelfMatchPrevention => Self::SelfMatchPrevention,
            PitRejectCode::ShortSaleRestriction => Self::ShortSaleRestriction,
            PitRejectCode::RiskConfigurationMissing => Self::RiskConfigurationMissing,
            PitRejectCode::ReferenceDataUnavailable => Self::ReferenceDataUnavailable,
            PitRejectCode::OrderValueCalculationFailed => Self::OrderValueCalculationFailed,
            PitRejectCode::SystemUnavailable => Self::SystemUnavailable,
            PitRejectCode::Custom => Self::Custom,
            PitRejectCode::Other => Self::Other,
        }
    }
}

impl From<RejectCode> for PitRejectCode {
    fn from(value: RejectCode) -> Self {
        match value {
            RejectCode::MissingRequiredField => Self::MissingRequiredField,
            RejectCode::InvalidFieldFormat => Self::InvalidFieldFormat,
            RejectCode::InvalidFieldValue => Self::InvalidFieldValue,
            RejectCode::UnsupportedOrderType => Self::UnsupportedOrderType,
            RejectCode::UnsupportedTimeInForce => Self::UnsupportedTimeInForce,
            RejectCode::UnsupportedOrderAttribute => Self::UnsupportedOrderAttribute,
            RejectCode::DuplicateClientOrderId => Self::DuplicateClientOrderId,
            RejectCode::TooLateToEnter => Self::TooLateToEnter,
            RejectCode::ExchangeClosed => Self::ExchangeClosed,
            RejectCode::UnknownInstrument => Self::UnknownInstrument,
            RejectCode::UnknownAccount => Self::UnknownAccount,
            RejectCode::UnknownVenue => Self::UnknownVenue,
            RejectCode::UnknownClearingAccount => Self::UnknownClearingAccount,
            RejectCode::UnknownCollateralAsset => Self::UnknownCollateralAsset,
            RejectCode::InsufficientFunds => Self::InsufficientFunds,
            RejectCode::InsufficientMargin => Self::InsufficientMargin,
            RejectCode::InsufficientPosition => Self::InsufficientPosition,
            RejectCode::CreditLimitExceeded => Self::CreditLimitExceeded,
            RejectCode::RiskLimitExceeded => Self::RiskLimitExceeded,
            RejectCode::OrderExceedsLimit => Self::OrderExceedsLimit,
            RejectCode::OrderQtyExceedsLimit => Self::OrderQtyExceedsLimit,
            RejectCode::OrderNotionalExceedsLimit => Self::OrderNotionalExceedsLimit,
            RejectCode::PositionLimitExceeded => Self::PositionLimitExceeded,
            RejectCode::ConcentrationLimitExceeded => Self::ConcentrationLimitExceeded,
            RejectCode::LeverageLimitExceeded => Self::LeverageLimitExceeded,
            RejectCode::RateLimitExceeded => Self::RateLimitExceeded,
            RejectCode::PnlKillSwitchTriggered => Self::PnlKillSwitchTriggered,
            RejectCode::AccountBlocked => Self::AccountBlocked,
            RejectCode::AccountNotAuthorized => Self::AccountNotAuthorized,
            RejectCode::ComplianceRestriction => Self::ComplianceRestriction,
            RejectCode::InstrumentRestricted => Self::InstrumentRestricted,
            RejectCode::JurisdictionRestriction => Self::JurisdictionRestriction,
            RejectCode::WashTradePrevention => Self::WashTradePrevention,
            RejectCode::SelfMatchPrevention => Self::SelfMatchPrevention,
            RejectCode::ShortSaleRestriction => Self::ShortSaleRestriction,
            RejectCode::RiskConfigurationMissing => Self::RiskConfigurationMissing,
            RejectCode::ReferenceDataUnavailable => Self::ReferenceDataUnavailable,
            RejectCode::OrderValueCalculationFailed => Self::OrderValueCalculationFailed,
            RejectCode::SystemUnavailable => Self::SystemUnavailable,
            RejectCode::Custom => Self::Custom,
            RejectCode::Other => Self::Other,
            _ => Self::Other,
        }
    }
}

fn export_reject_scope(value: RejectScope) -> PitRejectScope {
    match value {
        RejectScope::Order => PitRejectScope::Order,
        RejectScope::Account => PitRejectScope::Account,
    }
}

fn import_reject_scope(value: PitRejectScope) -> RejectScope {
    match value {
        PitRejectScope::Order => RejectScope::Order,
        PitRejectScope::Account => RejectScope::Account,
    }
}

fn import_string(ptr: PitStringView) -> String {
    if ptr.ptr.is_null() {
        return String::default();
    }

    let bytes = unsafe { std::slice::from_raw_parts(ptr.ptr, ptr.len) };
    String::from_utf8_lossy(bytes).into_owned()
}

pub(crate) fn rejects_to_list_owned(values: Rejects) -> PitRejectList {
    let mut out = Vec::with_capacity(values.len());
    for reject in values.iter().cloned() {
        out.push(reject);
    }
    PitRejectList { items: out }
}

#[no_mangle]
/// Creates a caller-owned reject list with preallocated capacity.
///
/// `reserve` is the requested number of elements to preallocate.
///
/// Contract:
/// - returns a new caller-owned list;
/// - release it with `pit_destroy_reject_list`;
/// - this function always succeeds.
pub extern "C" fn pit_create_reject_list(reserve: usize) -> *mut PitRejectList {
    Box::into_raw(Box::new(PitRejectList {
        items: Vec::with_capacity(reserve),
    }))
}

#[no_mangle]
/// Releases a caller-owned reject list.
///
/// Contract:
/// - passing null is allowed;
/// - this function always succeeds.
pub extern "C" fn pit_destroy_reject_list(rejects: *mut PitRejectList) {
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
/// - this function never fails;
/// - violating the pointer contract aborts the call.
pub extern "C" fn pit_reject_list_push(list: *mut PitRejectList, reject: PitReject) {
    assert!(!list.is_null(), "reject list pointer is null");
    let list = unsafe { &mut *list };
    list.items.push(reject.to_reject());
}

#[no_mangle]
/// Returns the number of rejects in the list.
///
/// Contract:
/// - `list` must be a valid non-null pointer;
/// - this function never fails;
/// - violating the pointer contract aborts the call.
pub extern "C" fn pit_reject_list_len(list: *const PitRejectList) -> usize {
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
pub extern "C" fn pit_reject_list_get(
    list: *const PitRejectList,
    index: usize,
    out_reject: *mut PitReject,
) -> bool {
    assert!(!list.is_null(), "reject list pointer is null");
    assert!(!out_reject.is_null(), "reject output pointer is null");
    let list = unsafe { &*list };
    let Some(reject) = list.items.get(index) else {
        return false;
    };
    unsafe { *out_reject = PitReject::from_reject(reject) };
    true
}

#[cfg(test)]
mod tests {
    use crate::PitStringView;
    use openpit::pretrade::{Reject, RejectCode, RejectScope};

    use super::{
        pit_create_reject_list, pit_destroy_reject_list, pit_reject_list_get, pit_reject_list_len,
        pit_reject_list_push, PitReject, PitRejectCode, PitRejectScope,
    };

    fn string_view_to_string(view: PitStringView) -> String {
        if view.ptr.is_null() {
            return String::new();
        }
        let bytes = unsafe { std::slice::from_raw_parts(view.ptr, view.len) };
        std::str::from_utf8(bytes).expect("utf8").to_string()
    }

    #[test]
    fn reject_list_destroy_is_null_safe() {
        pit_destroy_reject_list(std::ptr::null_mut());
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
        let exported = PitReject::from_reject(&reject);
        assert_eq!(string_view_to_string(exported.policy), "test_policy");
        assert_eq!(string_view_to_string(exported.reason), "reason");
        assert_eq!(string_view_to_string(exported.details), "details");
        assert_eq!(exported.user_data, std::ptr::null_mut());
    }

    #[test]
    fn reject_list_push_len_get_roundtrip() {
        let list = pit_create_reject_list(1);
        let reject = PitReject {
            policy: PitStringView::from_utf8("policy"),
            reason: PitStringView::from_utf8("reason"),
            details: PitStringView::from_utf8("details"),
            user_data: 55usize as *mut std::ffi::c_void,
            code: PitRejectCode::Other,
            scope: PitRejectScope::Order,
        };
        pit_reject_list_push(list, reject);
        assert_eq!(pit_reject_list_len(list), 1);
        let stored = unsafe { &*list };
        assert_eq!(stored.items[0].user_data, 55usize);
        let mut first = PitReject {
            policy: PitStringView::not_set(),
            reason: PitStringView::not_set(),
            details: PitStringView::not_set(),
            user_data: std::ptr::null_mut(),
            code: PitRejectCode::Other,
            scope: PitRejectScope::Order,
        };
        assert!(pit_reject_list_get(list, 0, &mut first));
        assert_eq!(first.code, PitRejectCode::Other);
        assert_eq!(first.user_data, 55usize as *mut std::ffi::c_void);
        assert_eq!(string_view_to_string(first.policy), "policy");
        assert!(!pit_reject_list_get(list, 1, &mut first));
        pit_destroy_reject_list(list);
    }

    #[test]
    fn import_reject_copies_view_payload() {
        let view = PitReject {
            policy: PitStringView::from_utf8("policy"),
            reason: PitStringView::from_utf8("reason"),
            details: PitStringView::from_utf8("details"),
            user_data: 77usize as *mut std::ffi::c_void,
            code: PitRejectCode::RateLimitExceeded,
            scope: PitRejectScope::Account,
        };
        let imported = view.to_reject();
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
            PitRejectCode::MissingRequiredField,
            PitRejectCode::InvalidFieldFormat,
            PitRejectCode::InvalidFieldValue,
            PitRejectCode::UnsupportedOrderType,
            PitRejectCode::UnsupportedTimeInForce,
            PitRejectCode::UnsupportedOrderAttribute,
            PitRejectCode::DuplicateClientOrderId,
            PitRejectCode::TooLateToEnter,
            PitRejectCode::ExchangeClosed,
            PitRejectCode::UnknownInstrument,
            PitRejectCode::UnknownAccount,
            PitRejectCode::UnknownVenue,
            PitRejectCode::UnknownClearingAccount,
            PitRejectCode::UnknownCollateralAsset,
            PitRejectCode::InsufficientFunds,
            PitRejectCode::InsufficientMargin,
            PitRejectCode::InsufficientPosition,
            PitRejectCode::CreditLimitExceeded,
            PitRejectCode::RiskLimitExceeded,
            PitRejectCode::OrderExceedsLimit,
            PitRejectCode::OrderQtyExceedsLimit,
            PitRejectCode::OrderNotionalExceedsLimit,
            PitRejectCode::PositionLimitExceeded,
            PitRejectCode::ConcentrationLimitExceeded,
            PitRejectCode::LeverageLimitExceeded,
            PitRejectCode::RateLimitExceeded,
            PitRejectCode::PnlKillSwitchTriggered,
            PitRejectCode::AccountBlocked,
            PitRejectCode::AccountNotAuthorized,
            PitRejectCode::ComplianceRestriction,
            PitRejectCode::InstrumentRestricted,
            PitRejectCode::JurisdictionRestriction,
            PitRejectCode::WashTradePrevention,
            PitRejectCode::SelfMatchPrevention,
            PitRejectCode::ShortSaleRestriction,
            PitRejectCode::RiskConfigurationMissing,
            PitRejectCode::ReferenceDataUnavailable,
            PitRejectCode::OrderValueCalculationFailed,
            PitRejectCode::SystemUnavailable,
            PitRejectCode::Custom,
            PitRejectCode::Other,
        ];
        for code in all {
            let domain = RejectCode::from(code);
            let ffi = PitRejectCode::from(domain);
            assert_eq!(ffi, code);
        }
    }
}
