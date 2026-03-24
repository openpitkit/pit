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
use openpit::{AccountAdjustmentBatchError, Engine};

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

macro_rules! map_pit_reject_codes {
    ($( $pit:ident => $rust:ident => $name:literal ),+ $(,)?) => {
        impl From<PitRejectCode> for RejectCode {
            fn from(value: PitRejectCode) -> Self {
                match value {
                    $(PitRejectCode::$pit => Self::$rust,)+
                }
            }
        }

        impl From<RejectCode> for PitRejectCode {
            fn from(value: RejectCode) -> Self {
                match value {
                    $(RejectCode::$rust => Self::$pit,)+
                    _ => Self::Other,
                }
            }
        }

        #[no_mangle]
        pub extern "C" fn pit_reject_code_to_cstr(code: PitRejectCode) -> *const c_char {
            match code {
                $(PitRejectCode::$pit => cstr_ptr(concat!($name, "\0").as_bytes()),)+
            }
        }
    };
}

map_pit_reject_codes! {
    MissingRequiredField => MissingRequiredField => "MissingRequiredField",
    InvalidFieldFormat => InvalidFieldFormat => "InvalidFieldFormat",
    InvalidFieldValue => InvalidFieldValue => "InvalidFieldValue",
    UnsupportedOrderType => UnsupportedOrderType => "UnsupportedOrderType",
    UnsupportedTimeInForce => UnsupportedTimeInForce => "UnsupportedTimeInForce",
    UnsupportedOrderAttribute => UnsupportedOrderAttribute => "UnsupportedOrderAttribute",
    DuplicateClientOrderId => DuplicateClientOrderId => "DuplicateClientOrderId",
    TooLateToEnter => TooLateToEnter => "TooLateToEnter",
    ExchangeClosed => ExchangeClosed => "ExchangeClosed",
    UnknownInstrument => UnknownInstrument => "UnknownInstrument",
    UnknownAccount => UnknownAccount => "UnknownAccount",
    UnknownVenue => UnknownVenue => "UnknownVenue",
    UnknownClearingAccount => UnknownClearingAccount => "UnknownClearingAccount",
    UnknownCollateralAsset => UnknownCollateralAsset => "UnknownCollateralAsset",
    InsufficientFunds => InsufficientFunds => "InsufficientFunds",
    InsufficientMargin => InsufficientMargin => "InsufficientMargin",
    InsufficientPosition => InsufficientPosition => "InsufficientPosition",
    CreditLimitExceeded => CreditLimitExceeded => "CreditLimitExceeded",
    RiskLimitExceeded => RiskLimitExceeded => "RiskLimitExceeded",
    OrderExceedsLimit => OrderExceedsLimit => "OrderExceedsLimit",
    OrderQtyExceedsLimit => OrderQtyExceedsLimit => "OrderQtyExceedsLimit",
    OrderNotionalExceedsLimit => OrderNotionalExceedsLimit => "OrderNotionalExceedsLimit",
    PositionLimitExceeded => PositionLimitExceeded => "PositionLimitExceeded",
    ConcentrationLimitExceeded => ConcentrationLimitExceeded => "ConcentrationLimitExceeded",
    LeverageLimitExceeded => LeverageLimitExceeded => "LeverageLimitExceeded",
    RateLimitExceeded => RateLimitExceeded => "RateLimitExceeded",
    PnlKillSwitchTriggered => PnlKillSwitchTriggered => "PnlKillSwitchTriggered",
    AccountBlocked => AccountBlocked => "AccountBlocked",
    AccountNotAuthorized => AccountNotAuthorized => "AccountNotAuthorized",
    ComplianceRestriction => ComplianceRestriction => "ComplianceRestriction",
    InstrumentRestricted => InstrumentRestricted => "InstrumentRestricted",
    JurisdictionRestriction => JurisdictionRestriction => "JurisdictionRestriction",
    WashTradePrevention => WashTradePrevention => "WashTradePrevention",
    SelfMatchPrevention => SelfMatchPrevention => "SelfMatchPrevention",
    ShortSaleRestriction => ShortSaleRestriction => "ShortSaleRestriction",
    RiskConfigurationMissing => RiskConfigurationMissing => "RiskConfigurationMissing",
    ReferenceDataUnavailable => ReferenceDataUnavailable => "ReferenceDataUnavailable",
    OrderValueCalculationFailed => OrderValueCalculationFailed => "OrderValueCalculationFailed",
    SystemUnavailable => SystemUnavailable => "SystemUnavailable",
    Other => Other => "Other",
}

type PitEngine = Engine<Order, ExecutionReport, AccountAdjustment>;

#[repr(C)]
pub struct PitBatchResult {
    pub ok: bool,
    pub failed_index: usize,
    pub reject_code: PitRejectCode,
    pub reject_reason: *const c_char,
}

fn leak_c_string_ptr(value: &str) -> *const c_char {
    CString::new(value)
        .unwrap_or_default()
        .into_raw()
        .cast::<c_char>()
}

fn batch_error_result(error: AccountAdjustmentBatchError) -> PitBatchResult {
    PitBatchResult {
        ok: false,
        failed_index: error.index,
        reject_code: error.reject.code.into(),
        reject_reason: leak_c_string_ptr(error.reject.reason.as_str()),
    }
}

fn invalid_batch_input_result(reason: &str) -> PitBatchResult {
    PitBatchResult {
        ok: false,
        failed_index: 0,
        reject_code: PitRejectCode::MissingRequiredField,
        reject_reason: leak_c_string_ptr(reason),
    }
}

#[no_mangle]
/// Validates a batch of account adjustments using the provided engine.
///
/// Returns an FFI-friendly batch result with either success or the first
/// rejection details.
///
/// If the returned result has `ok == false`, the caller must release
/// `reject_reason` by calling [`pit_free_batch_result`] once on that result.
///
/// # Safety
///
/// - `engine` must point to a valid `Engine<Order, ExecutionReport, AccountAdjustment>`.
/// - If `count > 0`, `adjustments` must be non-null and point to at least
///   `count` valid `AccountAdjustment` values.
pub unsafe extern "C" fn pit_apply_account_adjustment(
    engine: *const PitEngine,
    account_id: u64,
    adjustments: *const AccountAdjustment,
    count: usize,
) -> PitBatchResult {
    if engine.is_null() {
        return invalid_batch_input_result("engine is null");
    }
    if adjustments.is_null() && count > 0 {
        return invalid_batch_input_result("adjustments is null while count > 0");
    }

    let engine = unsafe { &*engine };
    let account_id = AccountId::from_u64(account_id);
    let batch = if count == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(adjustments, count) }
    };

    match engine.apply_account_adjustment(account_id, batch) {
        Ok(()) => PitBatchResult {
            ok: true,
            failed_index: 0,
            reject_code: PitRejectCode::Other,
            reject_reason: std::ptr::null(),
        },
        Err(error) => batch_error_result(error),
    }
}

/// Frees the heap-allocated `reject_reason` inside a [`PitBatchResult`].
///
/// Must be called exactly once for every `PitBatchResult` where `ok == false`.
/// Calling on a result where `ok == true` (reject_reason is null) is safe and
/// does nothing.
///
/// # Safety
///
/// `result` must be non-null and point to a valid `PitBatchResult` whose
/// `reject_reason` was produced by [`pit_apply_account_adjustment`].
#[no_mangle]
pub unsafe extern "C" fn pit_free_batch_result(result: *mut PitBatchResult) {
    if result.is_null() {
        return;
    }
    let result = unsafe { &mut *result };
    if !result.reject_reason.is_null() {
        let _ = unsafe { CString::from_raw(result.reject_reason as *mut c_char) };
        result.reject_reason = std::ptr::null();
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

#[cfg(test)]
mod tests {
    use std::ffi::CStr;

    use openpit::param::AccountId;
    use openpit::pretrade::{AccountAdjustmentPolicy, Mutations, Reject, RejectCode, RejectScope};
    use openpit::Engine;

    use super::{
        pit_account_id_from_str, pit_account_id_from_u64, pit_apply_account_adjustment,
        pit_free_batch_result, pit_policy_name_order_size_limit, pit_policy_name_pnl_killswitch,
        pit_policy_name_rate_limit, pit_reject_code_to_cstr, AccountAdjustment, ExecutionReport,
        Order, PitRejectCode,
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
    fn account_id_from_str_invalid_utf8_falls_back_to_empty_string() {
        let bytes = [0xFF_u8];
        let result = unsafe { pit_account_id_from_str(bytes.as_ptr().cast(), bytes.len()) };
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

    struct RejectSecondPolicy;

    impl AccountAdjustmentPolicy<AccountAdjustment> for RejectSecondPolicy {
        fn name(&self) -> &'static str {
            "RejectSecondPolicy"
        }

        fn apply_account_adjustment(
            &self,
            _account_id: AccountId,
            adjustment: &AccountAdjustment,
            _mutations: &mut Mutations,
        ) -> Result<(), Reject> {
            if adjustment.amount.is_some() {
                return Err(Reject::new(
                    self.name(),
                    RejectScope::Account,
                    RejectCode::RiskLimitExceeded,
                    "blocked by test policy",
                    "second adjustment should fail",
                ));
            }
            Ok(())
        }
    }

    #[test]
    fn apply_account_adjustment_batch_success() {
        let engine = Engine::<Order, ExecutionReport, AccountAdjustment>::builder()
            .build()
            .expect("engine build should succeed");
        let batch = [AccountAdjustment {
            operation: None,
            amount: None,
            bounds: None,
        }];

        let result =
            unsafe { pit_apply_account_adjustment(&engine, 99224416, batch.as_ptr(), batch.len()) };

        assert!(result.ok);
        assert_eq!(result.failed_index, 0);
        assert_eq!(result.reject_code, PitRejectCode::Other);
        assert!(result.reject_reason.is_null());
    }

    #[test]
    fn apply_account_adjustment_batch_reject_contains_error_fields() {
        let engine = Engine::<Order, ExecutionReport, AccountAdjustment>::builder()
            .account_adjustment_policy(RejectSecondPolicy)
            .build()
            .expect("engine build should succeed");
        let batch = [
            AccountAdjustment {
                operation: None,
                amount: None,
                bounds: None,
            },
            AccountAdjustment {
                operation: None,
                amount: Some(openpit::AccountAdjustmentAmount {
                    total: None,
                    reserved: None,
                    pending: None,
                }),
                bounds: None,
            },
        ];

        let mut result =
            unsafe { pit_apply_account_adjustment(&engine, 99224416, batch.as_ptr(), batch.len()) };

        assert!(!result.ok);
        assert_eq!(result.failed_index, 1);
        assert_eq!(result.reject_code, PitRejectCode::RiskLimitExceeded);
        let reason = unsafe { CStr::from_ptr(result.reject_reason) };
        assert_eq!(reason.to_str().expect("utf8"), "blocked by test policy");

        unsafe { pit_free_batch_result(&mut result) };
        assert!(result.reject_reason.is_null());
    }

    #[test]
    fn free_batch_result_is_safe_on_success() {
        let engine = Engine::<Order, ExecutionReport, AccountAdjustment>::builder()
            .build()
            .expect("engine build should succeed");
        let batch = [AccountAdjustment {
            operation: None,
            amount: None,
            bounds: None,
        }];

        let mut result =
            unsafe { pit_apply_account_adjustment(&engine, 99224416, batch.as_ptr(), batch.len()) };
        assert!(result.ok);
        assert!(result.reject_reason.is_null());

        unsafe { pit_free_batch_result(&mut result) };
        assert!(result.reject_reason.is_null());
    }

    #[test]
    fn free_batch_result_is_double_free_safe() {
        let engine = Engine::<Order, ExecutionReport, AccountAdjustment>::builder()
            .account_adjustment_policy(RejectSecondPolicy)
            .build()
            .expect("engine build should succeed");
        let batch = [
            AccountAdjustment {
                operation: None,
                amount: None,
                bounds: None,
            },
            AccountAdjustment {
                operation: None,
                amount: Some(openpit::AccountAdjustmentAmount {
                    total: None,
                    reserved: None,
                    pending: None,
                }),
                bounds: None,
            },
        ];

        let mut result =
            unsafe { pit_apply_account_adjustment(&engine, 99224416, batch.as_ptr(), batch.len()) };
        assert!(!result.ok);
        assert!(!result.reject_reason.is_null());

        unsafe { pit_free_batch_result(&mut result) };
        assert!(result.reject_reason.is_null());

        unsafe { pit_free_batch_result(&mut result) };
        assert!(result.reject_reason.is_null());
    }

    #[test]
    fn apply_account_adjustment_null_engine_returns_error() {
        let result = unsafe {
            pit_apply_account_adjustment(std::ptr::null(), 99224416, std::ptr::null(), 0)
        };

        assert!(!result.ok);
        assert_eq!(result.reject_code, PitRejectCode::MissingRequiredField);
        let reason = unsafe { CStr::from_ptr(result.reject_reason) };
        assert_eq!(reason.to_str().expect("utf8"), "engine is null");

        let mut result = result;
        unsafe { pit_free_batch_result(&mut result) };
    }

    #[test]
    fn apply_account_adjustment_null_adjustments_with_nonzero_count_returns_error() {
        let engine = Engine::<Order, ExecutionReport, AccountAdjustment>::builder()
            .build()
            .expect("engine build should succeed");

        let result =
            unsafe { pit_apply_account_adjustment(&engine, 99224416, std::ptr::null(), 1) };

        assert!(!result.ok);
        assert_eq!(result.reject_code, PitRejectCode::MissingRequiredField);
        let reason = unsafe { CStr::from_ptr(result.reject_reason) };
        assert_eq!(
            reason.to_str().expect("utf8"),
            "adjustments is null while count > 0"
        );

        let mut result = result;
        unsafe { pit_free_batch_result(&mut result) };
    }

    #[test]
    fn apply_account_adjustment_zero_count_with_nonnull_ptr_succeeds() {
        let engine = Engine::<Order, ExecutionReport, AccountAdjustment>::builder()
            .build()
            .expect("engine build should succeed");
        let dummy = AccountAdjustment {
            operation: None,
            amount: None,
            bounds: None,
        };

        let result = unsafe { pit_apply_account_adjustment(&engine, 99224416, &dummy, 0) };

        assert!(result.ok);
    }

    #[test]
    fn free_batch_result_is_safe_on_null_ptr() {
        unsafe { pit_free_batch_result(std::ptr::null_mut()) };
    }
}
