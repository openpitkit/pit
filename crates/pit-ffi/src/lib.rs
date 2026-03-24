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

use std::ffi::c_char;
use std::ffi::CString;
use std::sync::OnceLock;

use openpit::param::AccountId;

pub mod account_adjustment;
pub mod execution_report;
pub mod order;

pub use account_adjustment::{AccountAdjustment, AccountAdjustmentOperation};
pub use execution_report::{
    ExecutionReport, FillDetailsData, FinancialImpactData, PositionImpactData,
};
pub use order::Order;

use openpit::pretrade::policies::OrderSizeLimitPolicy;
use openpit::pretrade::policies::PnlKillSwitchPolicy;
use openpit::pretrade::policies::RateLimitPolicy;
use openpit::pretrade::RejectCode;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PitRejectCode {
    MissingRequiredField,
    InvalidFieldFormat,
    InvalidFieldValue,
    UnsupportedOrderType,
    UnsupportedTimeInForce,
    UnsupportedOrderAttribute,
    DuplicateClientOrderId,
    TooLateToEnter,
    ExchangeClosed,
    UnknownInstrument,
    UnknownAccount,
    UnknownVenue,
    UnknownClearingAccount,
    UnknownCollateralAsset,
    InsufficientFunds,
    InsufficientMargin,
    InsufficientPosition,
    CreditLimitExceeded,
    RiskLimitExceeded,
    OrderExceedsLimit,
    OrderQtyExceedsLimit,
    OrderNotionalExceedsLimit,
    PositionLimitExceeded,
    ConcentrationLimitExceeded,
    LeverageLimitExceeded,
    RateLimitExceeded,
    PnlKillSwitchTriggered,
    AccountBlocked,
    AccountNotAuthorized,
    ComplianceRestriction,
    InstrumentRestricted,
    JurisdictionRestriction,
    WashTradePrevention,
    SelfMatchPrevention,
    ShortSaleRestriction,
    RiskConfigurationMissing,
    ReferenceDataUnavailable,
    OrderValueCalculationFailed,
    SystemUnavailable,
    Other,
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
            PitRejectCode::Other => Self::Other,
        }
    }
}

/// Constructs an account identifier from a 64-bit integer.
/// No hashing. No collision risk.
#[no_mangle]
pub extern "C" fn pit_account_id_from_u64(value: u64) -> u64 {
    AccountId::from_u64(value).as_u64()
}

/// Constructs an account identifier by hashing a UTF-8 string with FNV-1a 64.
///
/// `ptr` must point to valid UTF-8; `len` is the byte length (no null
/// terminator required). The string is not retained after the call returns.
///
/// Hash collisions are possible. For n distinct account strings the
/// probability of at least one collision is approximately n^2 / 2^65.
/// If collision risk is unacceptable, maintain a collision-free
/// string-to-integer mapping on your side and use pit_account_id_from_u64.
///
/// # Safety
///
/// `ptr` must be non-null and point to at least `len` bytes of valid UTF-8.
#[no_mangle]
pub unsafe extern "C" fn pit_account_id_from_str(ptr: *const c_char, len: usize) -> u64 {
    let bytes = unsafe { std::slice::from_raw_parts(ptr.cast::<u8>(), len) };
    let s = std::str::from_utf8(bytes).unwrap_or("");
    AccountId::from_str(s).as_u64()
}

const fn cstr_ptr(bytes: &'static [u8]) -> *const c_char {
    bytes.as_ptr().cast()
}

fn policy_name_cstr(name: &str) -> CString {
    CString::new(name).unwrap_or_default()
}

#[no_mangle]
pub extern "C" fn pit_policy_name_pnl_killswitch() -> *const c_char {
    static NAME: OnceLock<CString> = OnceLock::new();
    NAME.get_or_init(|| policy_name_cstr(PnlKillSwitchPolicy::NAME))
        .as_ptr()
}

#[no_mangle]
pub extern "C" fn pit_policy_name_rate_limit() -> *const c_char {
    static NAME: OnceLock<CString> = OnceLock::new();
    NAME.get_or_init(|| policy_name_cstr(RateLimitPolicy::NAME))
        .as_ptr()
}

#[no_mangle]
pub extern "C" fn pit_policy_name_order_size_limit() -> *const c_char {
    static NAME: OnceLock<CString> = OnceLock::new();
    NAME.get_or_init(|| policy_name_cstr(OrderSizeLimitPolicy::NAME))
        .as_ptr()
}

#[no_mangle]
pub extern "C" fn pit_reject_code_to_cstr(code: PitRejectCode) -> *const c_char {
    match code {
        PitRejectCode::MissingRequiredField => cstr_ptr(b"MissingRequiredField\0"),
        PitRejectCode::InvalidFieldFormat => cstr_ptr(b"InvalidFieldFormat\0"),
        PitRejectCode::InvalidFieldValue => cstr_ptr(b"InvalidFieldValue\0"),
        PitRejectCode::UnsupportedOrderType => cstr_ptr(b"UnsupportedOrderType\0"),
        PitRejectCode::UnsupportedTimeInForce => cstr_ptr(b"UnsupportedTimeInForce\0"),
        PitRejectCode::UnsupportedOrderAttribute => cstr_ptr(b"UnsupportedOrderAttribute\0"),
        PitRejectCode::DuplicateClientOrderId => cstr_ptr(b"DuplicateClientOrderId\0"),
        PitRejectCode::TooLateToEnter => cstr_ptr(b"TooLateToEnter\0"),
        PitRejectCode::ExchangeClosed => cstr_ptr(b"ExchangeClosed\0"),
        PitRejectCode::UnknownInstrument => cstr_ptr(b"UnknownInstrument\0"),
        PitRejectCode::UnknownAccount => cstr_ptr(b"UnknownAccount\0"),
        PitRejectCode::UnknownVenue => cstr_ptr(b"UnknownVenue\0"),
        PitRejectCode::UnknownClearingAccount => cstr_ptr(b"UnknownClearingAccount\0"),
        PitRejectCode::UnknownCollateralAsset => cstr_ptr(b"UnknownCollateralAsset\0"),
        PitRejectCode::InsufficientFunds => cstr_ptr(b"InsufficientFunds\0"),
        PitRejectCode::InsufficientMargin => cstr_ptr(b"InsufficientMargin\0"),
        PitRejectCode::InsufficientPosition => cstr_ptr(b"InsufficientPosition\0"),
        PitRejectCode::CreditLimitExceeded => cstr_ptr(b"CreditLimitExceeded\0"),
        PitRejectCode::RiskLimitExceeded => cstr_ptr(b"RiskLimitExceeded\0"),
        PitRejectCode::OrderExceedsLimit => cstr_ptr(b"OrderExceedsLimit\0"),
        PitRejectCode::OrderQtyExceedsLimit => cstr_ptr(b"OrderQtyExceedsLimit\0"),
        PitRejectCode::OrderNotionalExceedsLimit => cstr_ptr(b"OrderNotionalExceedsLimit\0"),
        PitRejectCode::PositionLimitExceeded => cstr_ptr(b"PositionLimitExceeded\0"),
        PitRejectCode::ConcentrationLimitExceeded => cstr_ptr(b"ConcentrationLimitExceeded\0"),
        PitRejectCode::LeverageLimitExceeded => cstr_ptr(b"LeverageLimitExceeded\0"),
        PitRejectCode::RateLimitExceeded => cstr_ptr(b"RateLimitExceeded\0"),
        PitRejectCode::PnlKillSwitchTriggered => cstr_ptr(b"PnlKillSwitchTriggered\0"),
        PitRejectCode::AccountBlocked => cstr_ptr(b"AccountBlocked\0"),
        PitRejectCode::AccountNotAuthorized => cstr_ptr(b"AccountNotAuthorized\0"),
        PitRejectCode::ComplianceRestriction => cstr_ptr(b"ComplianceRestriction\0"),
        PitRejectCode::InstrumentRestricted => cstr_ptr(b"InstrumentRestricted\0"),
        PitRejectCode::JurisdictionRestriction => cstr_ptr(b"JurisdictionRestriction\0"),
        PitRejectCode::WashTradePrevention => cstr_ptr(b"WashTradePrevention\0"),
        PitRejectCode::SelfMatchPrevention => cstr_ptr(b"SelfMatchPrevention\0"),
        PitRejectCode::ShortSaleRestriction => cstr_ptr(b"ShortSaleRestriction\0"),
        PitRejectCode::RiskConfigurationMissing => cstr_ptr(b"RiskConfigurationMissing\0"),
        PitRejectCode::ReferenceDataUnavailable => cstr_ptr(b"ReferenceDataUnavailable\0"),
        PitRejectCode::OrderValueCalculationFailed => cstr_ptr(b"OrderValueCalculationFailed\0"),
        PitRejectCode::SystemUnavailable => cstr_ptr(b"SystemUnavailable\0"),
        PitRejectCode::Other => cstr_ptr(b"Other\0"),
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::CStr;

    use openpit::pretrade::RejectCode;

    use super::{
        pit_account_id_from_str, pit_account_id_from_u64, pit_policy_name_order_size_limit,
        pit_policy_name_pnl_killswitch, pit_policy_name_rate_limit, pit_reject_code_to_cstr,
        PitRejectCode,
    };

    #[test]
    fn account_id_from_u64_returns_value() {
        assert_eq!(pit_account_id_from_u64(99224416), 99224416);
        assert_eq!(pit_account_id_from_u64(u64::MIN), u64::MIN);
        assert_eq!(pit_account_id_from_u64(u64::MAX), u64::MAX);
    }

    #[test]
    fn account_id_from_str_empty_returns_fnv1a_offset_basis() {
        let result = unsafe { pit_account_id_from_str(c"".as_ptr(), 0) };
        assert_eq!(result, 14_695_981_039_346_656_037);
    }

    #[test]
    fn exports_policy_names_without_instances() {
        let pnl = unsafe { CStr::from_ptr(pit_policy_name_pnl_killswitch()) };
        let rate = unsafe { CStr::from_ptr(pit_policy_name_rate_limit()) };
        let size = unsafe { CStr::from_ptr(pit_policy_name_order_size_limit()) };

        assert_eq!(pnl.to_str().expect("utf8"), "PnlKillSwitchPolicy");
        assert_eq!(rate.to_str().expect("utf8"), "RateLimitPolicy");
        assert_eq!(size.to_str().expect("utf8"), "OrderSizeLimitPolicy");
    }

    #[test]
    fn reject_code_strings_are_stable() {
        let cases = [
            (
                PitRejectCode::MissingRequiredField,
                RejectCode::MissingRequiredField,
                "MissingRequiredField",
            ),
            (
                PitRejectCode::InvalidFieldFormat,
                RejectCode::InvalidFieldFormat,
                "InvalidFieldFormat",
            ),
            (
                PitRejectCode::InvalidFieldValue,
                RejectCode::InvalidFieldValue,
                "InvalidFieldValue",
            ),
            (
                PitRejectCode::UnsupportedOrderType,
                RejectCode::UnsupportedOrderType,
                "UnsupportedOrderType",
            ),
            (
                PitRejectCode::UnsupportedTimeInForce,
                RejectCode::UnsupportedTimeInForce,
                "UnsupportedTimeInForce",
            ),
            (
                PitRejectCode::UnsupportedOrderAttribute,
                RejectCode::UnsupportedOrderAttribute,
                "UnsupportedOrderAttribute",
            ),
            (
                PitRejectCode::DuplicateClientOrderId,
                RejectCode::DuplicateClientOrderId,
                "DuplicateClientOrderId",
            ),
            (
                PitRejectCode::TooLateToEnter,
                RejectCode::TooLateToEnter,
                "TooLateToEnter",
            ),
            (
                PitRejectCode::ExchangeClosed,
                RejectCode::ExchangeClosed,
                "ExchangeClosed",
            ),
            (
                PitRejectCode::UnknownInstrument,
                RejectCode::UnknownInstrument,
                "UnknownInstrument",
            ),
            (
                PitRejectCode::UnknownAccount,
                RejectCode::UnknownAccount,
                "UnknownAccount",
            ),
            (
                PitRejectCode::UnknownVenue,
                RejectCode::UnknownVenue,
                "UnknownVenue",
            ),
            (
                PitRejectCode::UnknownClearingAccount,
                RejectCode::UnknownClearingAccount,
                "UnknownClearingAccount",
            ),
            (
                PitRejectCode::UnknownCollateralAsset,
                RejectCode::UnknownCollateralAsset,
                "UnknownCollateralAsset",
            ),
            (
                PitRejectCode::InsufficientFunds,
                RejectCode::InsufficientFunds,
                "InsufficientFunds",
            ),
            (
                PitRejectCode::InsufficientMargin,
                RejectCode::InsufficientMargin,
                "InsufficientMargin",
            ),
            (
                PitRejectCode::InsufficientPosition,
                RejectCode::InsufficientPosition,
                "InsufficientPosition",
            ),
            (
                PitRejectCode::CreditLimitExceeded,
                RejectCode::CreditLimitExceeded,
                "CreditLimitExceeded",
            ),
            (
                PitRejectCode::RiskLimitExceeded,
                RejectCode::RiskLimitExceeded,
                "RiskLimitExceeded",
            ),
            (
                PitRejectCode::OrderExceedsLimit,
                RejectCode::OrderExceedsLimit,
                "OrderExceedsLimit",
            ),
            (
                PitRejectCode::OrderQtyExceedsLimit,
                RejectCode::OrderQtyExceedsLimit,
                "OrderQtyExceedsLimit",
            ),
            (
                PitRejectCode::OrderNotionalExceedsLimit,
                RejectCode::OrderNotionalExceedsLimit,
                "OrderNotionalExceedsLimit",
            ),
            (
                PitRejectCode::PositionLimitExceeded,
                RejectCode::PositionLimitExceeded,
                "PositionLimitExceeded",
            ),
            (
                PitRejectCode::ConcentrationLimitExceeded,
                RejectCode::ConcentrationLimitExceeded,
                "ConcentrationLimitExceeded",
            ),
            (
                PitRejectCode::LeverageLimitExceeded,
                RejectCode::LeverageLimitExceeded,
                "LeverageLimitExceeded",
            ),
            (
                PitRejectCode::RateLimitExceeded,
                RejectCode::RateLimitExceeded,
                "RateLimitExceeded",
            ),
            (
                PitRejectCode::PnlKillSwitchTriggered,
                RejectCode::PnlKillSwitchTriggered,
                "PnlKillSwitchTriggered",
            ),
            (
                PitRejectCode::AccountBlocked,
                RejectCode::AccountBlocked,
                "AccountBlocked",
            ),
            (
                PitRejectCode::AccountNotAuthorized,
                RejectCode::AccountNotAuthorized,
                "AccountNotAuthorized",
            ),
            (
                PitRejectCode::ComplianceRestriction,
                RejectCode::ComplianceRestriction,
                "ComplianceRestriction",
            ),
            (
                PitRejectCode::InstrumentRestricted,
                RejectCode::InstrumentRestricted,
                "InstrumentRestricted",
            ),
            (
                PitRejectCode::JurisdictionRestriction,
                RejectCode::JurisdictionRestriction,
                "JurisdictionRestriction",
            ),
            (
                PitRejectCode::WashTradePrevention,
                RejectCode::WashTradePrevention,
                "WashTradePrevention",
            ),
            (
                PitRejectCode::SelfMatchPrevention,
                RejectCode::SelfMatchPrevention,
                "SelfMatchPrevention",
            ),
            (
                PitRejectCode::ShortSaleRestriction,
                RejectCode::ShortSaleRestriction,
                "ShortSaleRestriction",
            ),
            (
                PitRejectCode::RiskConfigurationMissing,
                RejectCode::RiskConfigurationMissing,
                "RiskConfigurationMissing",
            ),
            (
                PitRejectCode::ReferenceDataUnavailable,
                RejectCode::ReferenceDataUnavailable,
                "ReferenceDataUnavailable",
            ),
            (
                PitRejectCode::OrderValueCalculationFailed,
                RejectCode::OrderValueCalculationFailed,
                "OrderValueCalculationFailed",
            ),
            (
                PitRejectCode::SystemUnavailable,
                RejectCode::SystemUnavailable,
                "SystemUnavailable",
            ),
            (PitRejectCode::Other, RejectCode::Other, "Other"),
        ];

        for (pit_code, rust_code, expected_name) in cases {
            let code = unsafe { CStr::from_ptr(pit_reject_code_to_cstr(pit_code)) };
            assert_eq!(code.to_str().expect("utf8"), expected_name);
            let converted: RejectCode = pit_code.into();
            assert_eq!(converted, rust_code);
        }
    }
}
