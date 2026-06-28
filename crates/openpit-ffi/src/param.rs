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

#![allow(clippy::missing_safety_doc)]

use openpit::param::{
    AccountId, Asset, CashFlow, Fee, Leverage, MonetaryAmount, Notional, Pnl, PositionEffect,
    PositionMode, PositionSide, PositionSize, Price, Quantity, RoundingStrategy, Side, TradeAmount,
    Volume,
};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use std::cmp::Ordering;

use crate::instrument::parse_asset_view;
use crate::last_error::{
    consume_param_error_with_code, write_param_error_unspecified, OpenPitOutParamError,
};
use crate::string::OpenPitSharedString;
use crate::OpenPitStringView;

//--------------------------------------------------------------------------------------------------

/// Leverage multiplier for FFI payloads.
///
/// Uses fixed-point scale `10` in integer units:
/// - `10` means `1.0x`
/// - `11` means `1.1x`
/// - `1005` means `100.5x`
///
/// Valid range: `10..=30000`.
///
/// A value of `OPENPIT_PARAM_LEVERAGE_NOT_SET` (`0`) means leverage is not
/// specified.
pub type OpenPitParamLeverage = u16;

/// Sentinel value indicating leverage is not set.
pub const OPENPIT_PARAM_LEVERAGE_NOT_SET: OpenPitParamLeverage = 0;
/// Fixed-point scale used by leverage payloads.
pub const OPENPIT_PARAM_LEVERAGE_SCALE: OpenPitParamLeverage = Leverage::SCALE;
/// Minimum leverage in whole units.
pub const OPENPIT_PARAM_LEVERAGE_MIN: OpenPitParamLeverage = Leverage::MIN;
/// Maximum leverage in whole units.
pub const OPENPIT_PARAM_LEVERAGE_MAX: OpenPitParamLeverage = Leverage::MAX;
/// Supported leverage increment step.
pub const OPENPIT_PARAM_LEVERAGE_STEP: f32 = Leverage::STEP;

pub(crate) fn import_leverage(value: OpenPitParamLeverage) -> Option<Leverage> {
    if value == OPENPIT_PARAM_LEVERAGE_NOT_SET {
        return None;
    }
    Leverage::from_raw(value).ok()
}

pub(crate) fn export_leverage(value: Option<Leverage>) -> OpenPitParamLeverage {
    value
        .map(|v| v.raw())
        .unwrap_or(OPENPIT_PARAM_LEVERAGE_NOT_SET)
}

/// Stable account identifier type for FFI payloads.
///
/// WARNING:
/// Use exactly one account-id source model per runtime:
/// - either purely numeric IDs (`openpit_create_param_account_id_from_uint64`),
/// - or purely string-derived IDs (`openpit_create_param_account_id_from_string`).
///
/// Do not mix both models in the same runtime state. A hashed string value can
/// coincide with a direct numeric ID, and then two distinct accounts become one
/// logical key in maps and engine state.
pub type OpenPitParamAccountId = u64;

pub(crate) fn import_side(value: OpenPitParamSide) -> Option<Side> {
    match value {
        OpenPitParamSide::Buy => Some(Side::Buy),
        OpenPitParamSide::Sell => Some(Side::Sell),
        OpenPitParamSide::NotSet => None,
    }
}

pub(crate) fn export_side(value: Side) -> OpenPitParamSide {
    match value {
        Side::Buy => OpenPitParamSide::Buy,
        Side::Sell => OpenPitParamSide::Sell,
    }
}

pub(crate) fn import_position_side(value: OpenPitParamPositionSide) -> Option<PositionSide> {
    match value {
        OpenPitParamPositionSide::Long => Some(PositionSide::Long),
        OpenPitParamPositionSide::Short => Some(PositionSide::Short),
        OpenPitParamPositionSide::NotSet => None,
    }
}

pub(crate) fn export_position_side(value: PositionSide) -> OpenPitParamPositionSide {
    match value {
        PositionSide::Long => OpenPitParamPositionSide::Long,
        PositionSide::Short => OpenPitParamPositionSide::Short,
    }
}

pub(crate) fn import_position_effect(value: OpenPitParamPositionEffect) -> Option<PositionEffect> {
    match value {
        OpenPitParamPositionEffect::Open => Some(PositionEffect::Open),
        OpenPitParamPositionEffect::Close => Some(PositionEffect::Close),
        OpenPitParamPositionEffect::NotSet => None,
    }
}

pub(crate) fn export_position_effect(value: PositionEffect) -> OpenPitParamPositionEffect {
    match value {
        PositionEffect::Open => OpenPitParamPositionEffect::Open,
        PositionEffect::Close => OpenPitParamPositionEffect::Close,
    }
}

pub(crate) fn import_position_mode(value: OpenPitParamPositionMode) -> Option<PositionMode> {
    match value {
        OpenPitParamPositionMode::Netting => Some(PositionMode::Netting),
        OpenPitParamPositionMode::Hedged => Some(PositionMode::Hedged),
        OpenPitParamPositionMode::NotSet => None,
    }
}

pub(crate) fn export_position_mode(value: PositionMode) -> OpenPitParamPositionMode {
    match value {
        PositionMode::Netting => OpenPitParamPositionMode::Netting,
        PositionMode::Hedged => OpenPitParamPositionMode::Hedged,
    }
}

pub(crate) fn import_bool(value: OpenPitTriBool) -> Option<bool> {
    match value {
        OpenPitTriBool::False => Some(false),
        OpenPitTriBool::True => Some(true),
        OpenPitTriBool::NotSet => None,
    }
}

pub(crate) fn export_bool(value: bool) -> OpenPitTriBool {
    if value {
        OpenPitTriBool::True
    } else {
        OpenPitTriBool::False
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
/// Order side.
pub enum OpenPitParamSide {
    /// Value is absent.
    #[default]
    NotSet = 0,
    /// Buy side.
    Buy = 1,
    /// Sell side.
    Sell = 2,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
/// Position direction.
pub enum OpenPitParamPositionSide {
    /// Value is absent.
    #[default]
    NotSet = 0,
    /// Long exposure.
    Long = 1,
    /// Short exposure.
    Short = 2,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
/// Position accounting mode.
pub enum OpenPitParamPositionMode {
    /// Value is absent.
    #[default]
    NotSet = 0,
    /// Opposite trades net into one position.
    Netting = 1,
    /// Long and short positions are tracked separately.
    Hedged = 2,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
/// Whether a trade opens or closes exposure.
pub enum OpenPitParamPositionEffect {
    /// Value is absent.
    #[default]
    NotSet = 0,
    /// The trade opens or increases exposure.
    Open = 1,
    /// The trade closes or reduces exposure.
    Close = 2,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
/// Selects how one trade-amount numeric value should be interpreted.
pub enum OpenPitParamTradeAmountKind {
    /// No amount field is selected.
    #[default]
    NotSet = 0,
    /// The value is instrument quantity.
    Quantity = 1,
    /// The value is settlement volume.
    Volume = 2,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
/// Decimal rounding strategy for typed parameter constructors.
pub enum OpenPitParamRoundingStrategy {
    /// Round half to nearest even number.
    #[default]
    MidpointNearestEven = 0,
    /// Round half away from zero.
    MidpointAwayFromZero = 1,
    /// Round towards positive infinity.
    Up = 2,
    /// Round towards negative infinity.
    Down = 3,
}

/// Default rounding strategy alias.
pub const OPENPIT_PARAM_ROUNDING_STRATEGY_DEFAULT: OpenPitParamRoundingStrategy =
    OpenPitParamRoundingStrategy::MidpointNearestEven;
/// Banker's rounding alias.
pub const OPENPIT_PARAM_ROUNDING_STRATEGY_BANKER: OpenPitParamRoundingStrategy =
    OpenPitParamRoundingStrategy::MidpointNearestEven;
/// Conservative profit rounding alias.
pub const OPENPIT_PARAM_ROUNDING_STRATEGY_CONSERVATIVE_PROFIT: OpenPitParamRoundingStrategy =
    OpenPitParamRoundingStrategy::Down;
/// Conservative loss rounding alias.
pub const OPENPIT_PARAM_ROUNDING_STRATEGY_CONSERVATIVE_LOSS: OpenPitParamRoundingStrategy =
    OpenPitParamRoundingStrategy::Down;

const _: () = assert!(
    OPENPIT_PARAM_ROUNDING_STRATEGY_DEFAULT as u8
        == export_rounding_strategy(RoundingStrategy::DEFAULT) as u8
);
const _: () = assert!(
    OPENPIT_PARAM_ROUNDING_STRATEGY_BANKER as u8
        == export_rounding_strategy(RoundingStrategy::BANKER) as u8
);
const _: () = assert!(
    OPENPIT_PARAM_ROUNDING_STRATEGY_CONSERVATIVE_PROFIT as u8
        == export_rounding_strategy(RoundingStrategy::CONSERVATIVE_PROFIT) as u8
);
const _: () = assert!(
    OPENPIT_PARAM_ROUNDING_STRATEGY_CONSERVATIVE_LOSS as u8
        == export_rounding_strategy(RoundingStrategy::CONSERVATIVE_LOSS) as u8
);

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
/// One trade-amount value plus its interpretation mode.
///
/// The numeric value is interpreted according to `kind`:
/// - `Quantity` means instrument quantity;
/// - `Volume` means settlement notional volume.
pub struct OpenPitParamTradeAmount {
    /// Non-negative numeric payload.
    pub value: OpenPitParamDecimal,
    /// Interpretation mode for `value`.
    pub kind: OpenPitParamTradeAmountKind,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
/// Signed fee-style amount plus an explicit currency.
pub struct OpenPitParamMonetaryAmount {
    /// Signed amount.
    pub amount: OpenPitParamFee,
    /// Currency the amount is denominated in.
    pub currency: OpenPitStringView,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
/// Tri-state boolean value.
pub enum OpenPitTriBool {
    /// Value is absent.
    #[default]
    NotSet = 0,
    /// Boolean false.
    False = 1,
    /// Boolean true.
    True = 2,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
/// Selects how an account-adjustment amount should be interpreted.
pub enum OpenPitParamAdjustmentAmountKind {
    /// No amount is specified.
    #[default]
    NotSet = 0,
    /// Change current state by the supplied signed amount.
    Delta = 1,
    /// Set current state to the supplied signed amount.
    Absolute = 2,
}

//--------------------------------------------------------------------------------------------------

/// Decimal value represented as `mantissa * 10^-scale`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct OpenPitParamDecimal {
    /// Lower 64 bits of the i128 mantissa.
    pub mantissa_lo: i64,
    /// Upper 64 bits of the i128 mantissa (sign-extended).
    pub mantissa_hi: i64,
    /// Decimal scale.
    pub scale: i32,
}

impl From<Decimal> for OpenPitParamDecimal {
    fn from(value: Decimal) -> Self {
        Self::from_decimal(value)
    }
}

impl OpenPitParamDecimal {
    pub(crate) fn from_decimal(d: Decimal) -> Self {
        let m = d.mantissa();
        Self {
            mantissa_lo: m as i64,
            mantissa_hi: (m >> 64) as i64,
            scale: d.scale() as i32,
        }
    }

    pub(crate) fn to_mantissa(self) -> i128 {
        (self.mantissa_hi as i128) << 64 | (self.mantissa_lo as u64 as i128)
    }
}

fn import_decimal(value: OpenPitParamDecimal) -> Result<Decimal, String> {
    let scale: u32 = value
        .scale
        .try_into()
        .map_err(|e| format!("invalid decimal scale {} for decimal: {}", value.scale, e))?;
    Ok(Decimal::from_i128_with_scale(value.to_mantissa(), scale))
}

unsafe fn parse_string_view(value: OpenPitStringView) -> Result<String, String> {
    if value.ptr.is_null() {
        if value.len == 0 {
            return Ok(String::new());
        }
        return Err("string view pointer is null".to_string());
    }

    let bytes = unsafe { std::slice::from_raw_parts(value.ptr, value.len) };
    std::str::from_utf8(bytes)
        .map(|text| text.to_owned())
        .map_err(|error| format!("string view is not valid utf-8: {error}"))
}

fn compare_decimals(lhs: Decimal, rhs: Decimal) -> i8 {
    match lhs.cmp(&rhs) {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    }
}

fn import_rounding_strategy(value: OpenPitParamRoundingStrategy) -> RoundingStrategy {
    match value {
        OpenPitParamRoundingStrategy::MidpointNearestEven => RoundingStrategy::MidpointNearestEven,
        OpenPitParamRoundingStrategy::MidpointAwayFromZero => {
            RoundingStrategy::MidpointAwayFromZero
        }
        OpenPitParamRoundingStrategy::Up => RoundingStrategy::Up,
        OpenPitParamRoundingStrategy::Down => RoundingStrategy::Down,
    }
}

const fn export_rounding_strategy(value: RoundingStrategy) -> OpenPitParamRoundingStrategy {
    match value {
        RoundingStrategy::MidpointNearestEven => OpenPitParamRoundingStrategy::MidpointNearestEven,
        RoundingStrategy::MidpointAwayFromZero => {
            OpenPitParamRoundingStrategy::MidpointAwayFromZero
        }
        RoundingStrategy::Up => OpenPitParamRoundingStrategy::Up,
        RoundingStrategy::Down => OpenPitParamRoundingStrategy::Down,
    }
}

trait IntoParamResult<T> {
    fn into_param_result(self, type_name: &str) -> Result<T, String>;
}

impl<T> IntoParamResult<T> for T {
    fn into_param_result(self, _type_name: &str) -> Result<T, String> {
        Ok(self)
    }
}

impl<T, E: std::fmt::Display> IntoParamResult<T> for Result<T, E> {
    fn into_param_result(self, type_name: &str) -> Result<T, String> {
        self.map_err(|e| format!("invalid typed param.{} value: {}", type_name, e))
    }
}

/// Abstracts over the two shapes of `<$domain>::new(Decimal)`:
/// infallible (signed types return `Self`) and fallible (unsigned types
/// return `Result<Self, openpit::param::Error>`), preserving the typed SDK
/// error in the fallible case.
trait IntoTypedParamResult<T> {
    fn into_typed_param_result(self) -> Result<T, openpit::param::Error>;
}

impl<T> IntoTypedParamResult<T> for T {
    fn into_typed_param_result(self) -> Result<T, openpit::param::Error> {
        Ok(self)
    }
}

impl<T> IntoTypedParamResult<T> for Result<T, openpit::param::Error> {
    fn into_typed_param_result(self) -> Result<T, openpit::param::Error> {
        self
    }
}

macro_rules! define_decimal_param_wrapper {
    (
        wrapper = $wrapper:ident,
        domain = $domain:ty,
        about = $about:literal,
        create_fn = $create_fn:ident,
        get_decimal_fn = $get_decimal_fn:ident
    ) => {
        #[doc = concat!(
                                                            "Validated `",
                                                            stringify!($domain),
                                                            "` value wrapper."
                                                            )]
        #[repr(transparent)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
        pub struct $wrapper(pub OpenPitParamDecimal);

        impl $wrapper {
            #[doc = concat!(
                                                                "Converts validated `",
                                                                stringify!($domain),
                                                                "` wrapper into the semantic value."
                                                            )]
            pub(crate) fn to_param(self) -> Result<$domain, String> {
                let scale: u32 = self.0.scale.try_into().map_err(|e| {
                    format!(
                        "invalid decimal scale {} for typed param.{}: {}",
                        self.0.scale,
                        stringify!($domain),
                        e
                    )
                })?;
                let decimal = Decimal::from_i128_with_scale(self.0.to_mantissa(), scale);
                <$domain>::new(decimal).into_param_result(stringify!($domain))
            }

            #[doc = concat!(
                                                                "Converts validated `",
                                                                stringify!($domain),
                                                                "` wrapper into the semantic value, preserving the typed SDK error."
                                                            )]
            ///
            /// The scale transport failure has no SDK `Error` counterpart, so it
            /// is reported as `Err(None)`; an SDK validation failure is reported
            /// as `Err(Some(error))`.
            pub(crate) fn to_param_typed(
                self,
            ) -> Result<$domain, Option<openpit::param::Error>> {
                let scale: u32 = self.0.scale.try_into().map_err(|_| None)?;
                let decimal = Decimal::from_i128_with_scale(self.0.to_mantissa(), scale);
                let result: Result<$domain, openpit::param::Error> =
                    <$domain>::new(decimal).into_typed_param_result();
                result.map_err(Some)
            }
        }

        #[doc = concat!("Validates a decimal and returns a `", stringify!($domain), "` wrapper.")]
        #[doc = ""]
        #[doc = concat!("Meaning: ", $about)]
        ///
        /// Success:
        /// - returns `true` and writes a validated wrapper to `out`.
        ///
        /// Error:
        /// - returns `false` when `out` is null or when the decimal
        ///   does not satisfy the rules of this type;
        /// - on error read `out_error` for the message.
        #[no_mangle]
        pub unsafe extern "C" fn $create_fn(
            value: OpenPitParamDecimal,
            out: *mut $wrapper,
            out_error: OpenPitOutParamError,
        ) -> bool {
            if out.is_null() {
                write_param_error_unspecified(out_error, "result place pointer is null");
                return false;
            }
            match $wrapper(value).to_param_typed() {
                Ok(_) => {
                    unsafe { *out = $wrapper(value) };
                    true
                }
                Err(Some(error)) => {
                    consume_param_error_with_code(out_error, error);
                    false
                }
                Err(None) => {
                    write_param_error_unspecified(
                        out_error,
                        "invalid decimal scale for typed param",
                    );
                    false
                }
            }
        }

        #[doc = concat!(
                                                    "Returns the decimal stored in `",
                                                    stringify!($domain),
                                                    "`."
                                                    )]
        #[no_mangle]
        pub extern "C" fn $get_decimal_fn(value: $wrapper) -> OpenPitParamDecimal {
            if let Ok(inner) = value.to_param() {
                OpenPitParamDecimal::from_decimal(inner.to_decimal())
            } else {
                value.0
            }
        }
    };
}

define_decimal_param_wrapper!(
    wrapper = OpenPitParamPnl,
    domain = Pnl,
    about = "Profit and loss value; positive means profit, negative means loss.",
    create_fn = openpit_create_param_pnl,
    get_decimal_fn = openpit_param_pnl_get_decimal
);

define_decimal_param_wrapper!(
    wrapper = OpenPitParamPrice,
    domain = Price,
    about = "Price per one instrument unit; may be negative in some derivative markets.",
    create_fn = openpit_create_param_price,
    get_decimal_fn = openpit_param_price_get_decimal
);

define_decimal_param_wrapper!(
    wrapper = OpenPitParamQuantity,
    domain = Quantity,
    about = "Instrument quantity; non-negative amount in instrument units.",
    create_fn = openpit_create_param_quantity,
    get_decimal_fn = openpit_param_quantity_get_decimal
);

define_decimal_param_wrapper!(
    wrapper = OpenPitParamVolume,
    domain = Volume,
    about = "Settlement notional volume; non-negative amount in settlement units.",
    create_fn = openpit_create_param_volume,
    get_decimal_fn = openpit_param_volume_get_decimal
);

define_decimal_param_wrapper!(
    wrapper = OpenPitParamCashFlow,
    domain = CashFlow,
    about = "Cash flow contribution; positive is inflow, negative is outflow.",
    create_fn = openpit_create_param_cash_flow,
    get_decimal_fn = openpit_param_cash_flow_get_decimal
);

define_decimal_param_wrapper!(
    wrapper = OpenPitParamPositionSize,
    domain = PositionSize,
    about = "Signed position size; long is positive, short is negative.",
    create_fn = openpit_create_param_position_size,
    get_decimal_fn = openpit_param_position_size_get_decimal
);

define_decimal_param_wrapper!(
    wrapper = OpenPitParamFee,
    domain = Fee,
    about = "Fee amount; can be negative for rebates or reconciliation adjustments.",
    create_fn = openpit_create_param_fee,
    get_decimal_fn = openpit_param_fee_get_decimal
);

define_decimal_param_wrapper!(
    wrapper = OpenPitParamNotional,
    domain = Notional,
    about = "Monetary position exposure for margin and risk calculation; always non-negative.",
    create_fn = openpit_create_param_notional,
    get_decimal_fn = openpit_param_notional_get_decimal
);

fn write_out<T: Copy>(out: *mut T, value: T, out_error: OpenPitOutParamError) -> bool {
    if out.is_null() {
        write_param_error_unspecified(out_error, "result place pointer is null");
        return false;
    }
    unsafe {
        *out = value;
    }
    true
}

macro_rules! define_decimal_param_ffi_common {
    (
        wrapper = $wrapper:ident,
        domain = $domain:ty,
        type_name = $type_name:literal,
        from_string_fn = $from_string_fn:ident,
        from_f64_fn = $from_f64_fn:ident,
        from_int64_fn = $from_int64_fn:ident,
        from_uint64_fn = $from_uint64_fn:ident,
        from_string_rounded_fn = $from_string_rounded_fn:ident,
        from_f64_rounded_fn = $from_f64_rounded_fn:ident,
        from_decimal_rounded_fn = $from_decimal_rounded_fn:ident,
        to_f64_fn = $to_f64_fn:ident,
        is_zero_fn = $is_zero_fn:ident,
        compare_fn = $compare_fn:ident,
        to_string_fn = $to_string_fn:ident,
        checked_add_fn = $checked_add_fn:ident,
        checked_sub_fn = $checked_sub_fn:ident,
        checked_mul_i64_fn = $checked_mul_i64_fn:ident,
        checked_mul_u64_fn = $checked_mul_u64_fn:ident,
        checked_mul_f64_fn = $checked_mul_f64_fn:ident,
        checked_div_i64_fn = $checked_div_i64_fn:ident,
        checked_div_u64_fn = $checked_div_u64_fn:ident,
        checked_div_f64_fn = $checked_div_f64_fn:ident,
        checked_rem_i64_fn = $checked_rem_i64_fn:ident,
        checked_rem_u64_fn = $checked_rem_u64_fn:ident,
        checked_rem_f64_fn = $checked_rem_f64_fn:ident
    ) => {
        #[no_mangle]
        pub unsafe extern "C" fn $from_string_fn(
            value: OpenPitStringView,
            out: *mut $wrapper,
            out_error: OpenPitOutParamError,
        ) -> bool {
            let text = match unsafe { parse_string_view(value) } {
                Ok(text) => text,
                Err(error) => {
                    write_param_error_unspecified(out_error, error.as_str());
                    return false;
                }
            };

            match <$domain>::from_str(text.as_str()) {
                Ok(parsed) => write_out(
                    out,
                    $wrapper(OpenPitParamDecimal::from_decimal(parsed.to_decimal())),
                    out_error,
                ),
                Err(error) => {
                    consume_param_error_with_code(out_error, error);
                    false
                }
            }
        }

        #[no_mangle]
        pub unsafe extern "C" fn $from_f64_fn(
            value: f64,
            out: *mut $wrapper,
            out_error: OpenPitOutParamError,
        ) -> bool {
            match <$domain>::from_f64(value) {
                Ok(parsed) => write_out(
                    out,
                    $wrapper(OpenPitParamDecimal::from_decimal(parsed.to_decimal())),
                    out_error,
                ),
                Err(error) => {
                    consume_param_error_with_code(out_error, error);
                    false
                }
            }
        }

        #[no_mangle]
        pub unsafe extern "C" fn $from_int64_fn(
            value: i64,
            out: *mut $wrapper,
            out_error: OpenPitOutParamError,
        ) -> bool {
            let result: Result<$domain, openpit::param::Error> =
                <$domain>::new(Decimal::from(value)).into_typed_param_result();
            match result {
                Ok(parsed) => write_out(
                    out,
                    $wrapper(OpenPitParamDecimal::from_decimal(parsed.to_decimal())),
                    out_error,
                ),
                Err(error) => {
                    consume_param_error_with_code(out_error, error);
                    false
                }
            }
        }

        #[no_mangle]
        pub unsafe extern "C" fn $from_uint64_fn(
            value: u64,
            out: *mut $wrapper,
            out_error: OpenPitOutParamError,
        ) -> bool {
            let result: Result<$domain, openpit::param::Error> =
                <$domain>::new(Decimal::from(value)).into_typed_param_result();
            match result {
                Ok(parsed) => write_out(
                    out,
                    $wrapper(OpenPitParamDecimal::from_decimal(parsed.to_decimal())),
                    out_error,
                ),
                Err(error) => {
                    consume_param_error_with_code(out_error, error);
                    false
                }
            }
        }

        #[no_mangle]
        pub unsafe extern "C" fn $from_string_rounded_fn(
            value: OpenPitStringView,
            scale: u32,
            rounding: OpenPitParamRoundingStrategy,
            out: *mut $wrapper,
            out_error: OpenPitOutParamError,
        ) -> bool {
            let text = match unsafe { parse_string_view(value) } {
                Ok(text) => text,
                Err(error) => {
                    write_param_error_unspecified(out_error, error.as_str());
                    return false;
                }
            };

            match <$domain>::from_str_rounded(
                text.as_str(),
                scale,
                import_rounding_strategy(rounding),
            ) {
                Ok(parsed) => write_out(
                    out,
                    $wrapper(OpenPitParamDecimal::from_decimal(parsed.to_decimal())),
                    out_error,
                ),
                Err(error) => {
                    consume_param_error_with_code(out_error, error);
                    false
                }
            }
        }

        #[no_mangle]
        pub unsafe extern "C" fn $from_f64_rounded_fn(
            value: f64,
            scale: u32,
            rounding: OpenPitParamRoundingStrategy,
            out: *mut $wrapper,
            out_error: OpenPitOutParamError,
        ) -> bool {
            match <$domain>::from_f64_rounded(value, scale, import_rounding_strategy(rounding)) {
                Ok(parsed) => write_out(
                    out,
                    $wrapper(OpenPitParamDecimal::from_decimal(parsed.to_decimal())),
                    out_error,
                ),
                Err(error) => {
                    consume_param_error_with_code(out_error, error);
                    false
                }
            }
        }

        #[no_mangle]
        pub unsafe extern "C" fn $from_decimal_rounded_fn(
            value: OpenPitParamDecimal,
            scale: u32,
            rounding: OpenPitParamRoundingStrategy,
            out: *mut $wrapper,
            out_error: OpenPitOutParamError,
        ) -> bool {
            let decimal = match import_decimal(value) {
                Ok(decimal) => decimal,
                Err(error) => {
                    write_param_error_unspecified(out_error, error.as_str());
                    return false;
                }
            };
            match <$domain>::from_decimal_rounded(
                decimal,
                scale,
                import_rounding_strategy(rounding),
            ) {
                Ok(parsed) => write_out(
                    out,
                    $wrapper(OpenPitParamDecimal::from_decimal(parsed.to_decimal())),
                    out_error,
                ),
                Err(error) => {
                    consume_param_error_with_code(out_error, error);
                    false
                }
            }
        }

        #[no_mangle]
        pub unsafe extern "C" fn $to_f64_fn(
            value: $wrapper,
            out: *mut f64,
            out_error: OpenPitOutParamError,
        ) -> bool {
            let parsed = match value.to_param() {
                Ok(parsed) => parsed,
                Err(error) => {
                    write_param_error_unspecified(out_error, error.as_str());
                    return false;
                }
            };
            let as_f64 = match parsed.to_decimal().to_f64() {
                Some(value) => value,
                None => {
                    write_param_error_unspecified(
                        out_error,
                        "decimal cannot be represented as f64",
                    );
                    return false;
                }
            };
            write_out(out, as_f64, out_error)
        }

        #[no_mangle]
        pub unsafe extern "C" fn $is_zero_fn(
            value: $wrapper,
            out: *mut bool,
            out_error: OpenPitOutParamError,
        ) -> bool {
            let parsed = match value.to_param() {
                Ok(parsed) => parsed,
                Err(error) => {
                    write_param_error_unspecified(out_error, error.as_str());
                    return false;
                }
            };
            write_out(out, parsed.is_zero(), out_error)
        }

        #[no_mangle]
        pub unsafe extern "C" fn $compare_fn(
            lhs: $wrapper,
            rhs: $wrapper,
            out: *mut i8,
            out_error: OpenPitOutParamError,
        ) -> bool {
            let lhs = match lhs.to_param() {
                Ok(parsed) => parsed,
                Err(error) => {
                    write_param_error_unspecified(out_error, error.as_str());
                    return false;
                }
            };
            let rhs = match rhs.to_param() {
                Ok(parsed) => parsed,
                Err(error) => {
                    write_param_error_unspecified(out_error, error.as_str());
                    return false;
                }
            };
            write_out(
                out,
                compare_decimals(lhs.to_decimal(), rhs.to_decimal()),
                out_error,
            )
        }

        #[no_mangle]
        pub unsafe extern "C" fn $to_string_fn(
            value: $wrapper,
            out_error: OpenPitOutParamError,
        ) -> *mut OpenPitSharedString {
            match value.to_param() {
                Ok(parsed) => OpenPitSharedString::new_handle(parsed.to_string().as_str()),
                Err(error) => {
                    write_param_error_unspecified(out_error, error.as_str());
                    std::ptr::null_mut()
                }
            }
        }

        #[no_mangle]
        pub unsafe extern "C" fn $checked_add_fn(
            lhs: $wrapper,
            rhs: $wrapper,
            out: *mut $wrapper,
            out_error: OpenPitOutParamError,
        ) -> bool {
            let lhs = match lhs.to_param() {
                Ok(value) => value,
                Err(error) => {
                    write_param_error_unspecified(out_error, error.as_str());
                    return false;
                }
            };
            let rhs = match rhs.to_param() {
                Ok(value) => value,
                Err(error) => {
                    write_param_error_unspecified(out_error, error.as_str());
                    return false;
                }
            };
            match lhs.checked_add(rhs) {
                Ok(parsed) => write_out(
                    out,
                    $wrapper(OpenPitParamDecimal::from_decimal(parsed.to_decimal())),
                    out_error,
                ),
                Err(error) => {
                    consume_param_error_with_code(out_error, error);
                    false
                }
            }
        }

        #[no_mangle]
        pub unsafe extern "C" fn $checked_sub_fn(
            lhs: $wrapper,
            rhs: $wrapper,
            out: *mut $wrapper,
            out_error: OpenPitOutParamError,
        ) -> bool {
            let lhs = match lhs.to_param() {
                Ok(value) => value,
                Err(error) => {
                    write_param_error_unspecified(out_error, error.as_str());
                    return false;
                }
            };
            let rhs = match rhs.to_param() {
                Ok(value) => value,
                Err(error) => {
                    write_param_error_unspecified(out_error, error.as_str());
                    return false;
                }
            };
            match lhs.checked_sub(rhs) {
                Ok(parsed) => write_out(
                    out,
                    $wrapper(OpenPitParamDecimal::from_decimal(parsed.to_decimal())),
                    out_error,
                ),
                Err(error) => {
                    consume_param_error_with_code(out_error, error);
                    false
                }
            }
        }

        #[no_mangle]
        pub unsafe extern "C" fn $checked_mul_i64_fn(
            value: $wrapper,
            scalar: i64,
            out: *mut $wrapper,
            out_error: OpenPitOutParamError,
        ) -> bool {
            let value = match value.to_param() {
                Ok(value) => value,
                Err(error) => {
                    write_param_error_unspecified(out_error, error.as_str());
                    return false;
                }
            };
            match value.checked_mul_i64(scalar) {
                Ok(parsed) => write_out(
                    out,
                    $wrapper(OpenPitParamDecimal::from_decimal(parsed.to_decimal())),
                    out_error,
                ),
                Err(error) => {
                    consume_param_error_with_code(out_error, error);
                    false
                }
            }
        }

        #[no_mangle]
        pub unsafe extern "C" fn $checked_mul_u64_fn(
            value: $wrapper,
            scalar: u64,
            out: *mut $wrapper,
            out_error: OpenPitOutParamError,
        ) -> bool {
            let value = match value.to_param() {
                Ok(value) => value,
                Err(error) => {
                    write_param_error_unspecified(out_error, error.as_str());
                    return false;
                }
            };
            match value.checked_mul_u64(scalar) {
                Ok(parsed) => write_out(
                    out,
                    $wrapper(OpenPitParamDecimal::from_decimal(parsed.to_decimal())),
                    out_error,
                ),
                Err(error) => {
                    consume_param_error_with_code(out_error, error);
                    false
                }
            }
        }

        #[no_mangle]
        pub unsafe extern "C" fn $checked_mul_f64_fn(
            value: $wrapper,
            scalar: f64,
            out: *mut $wrapper,
            out_error: OpenPitOutParamError,
        ) -> bool {
            let value = match value.to_param() {
                Ok(value) => value,
                Err(error) => {
                    write_param_error_unspecified(out_error, error.as_str());
                    return false;
                }
            };
            match value.checked_mul_f64(scalar) {
                Ok(parsed) => write_out(
                    out,
                    $wrapper(OpenPitParamDecimal::from_decimal(parsed.to_decimal())),
                    out_error,
                ),
                Err(error) => {
                    consume_param_error_with_code(out_error, error);
                    false
                }
            }
        }

        #[no_mangle]
        pub unsafe extern "C" fn $checked_div_i64_fn(
            value: $wrapper,
            divisor: i64,
            out: *mut $wrapper,
            out_error: OpenPitOutParamError,
        ) -> bool {
            let value = match value.to_param() {
                Ok(value) => value,
                Err(error) => {
                    write_param_error_unspecified(out_error, error.as_str());
                    return false;
                }
            };
            match value.checked_div_i64(divisor) {
                Ok(parsed) => write_out(
                    out,
                    $wrapper(OpenPitParamDecimal::from_decimal(parsed.to_decimal())),
                    out_error,
                ),
                Err(error) => {
                    consume_param_error_with_code(out_error, error);
                    false
                }
            }
        }

        #[no_mangle]
        pub unsafe extern "C" fn $checked_div_u64_fn(
            value: $wrapper,
            divisor: u64,
            out: *mut $wrapper,
            out_error: OpenPitOutParamError,
        ) -> bool {
            let value = match value.to_param() {
                Ok(value) => value,
                Err(error) => {
                    write_param_error_unspecified(out_error, error.as_str());
                    return false;
                }
            };
            match value.checked_div_u64(divisor) {
                Ok(parsed) => write_out(
                    out,
                    $wrapper(OpenPitParamDecimal::from_decimal(parsed.to_decimal())),
                    out_error,
                ),
                Err(error) => {
                    consume_param_error_with_code(out_error, error);
                    false
                }
            }
        }

        #[no_mangle]
        pub unsafe extern "C" fn $checked_div_f64_fn(
            value: $wrapper,
            divisor: f64,
            out: *mut $wrapper,
            out_error: OpenPitOutParamError,
        ) -> bool {
            let value = match value.to_param() {
                Ok(value) => value,
                Err(error) => {
                    write_param_error_unspecified(out_error, error.as_str());
                    return false;
                }
            };
            match value.checked_div_f64(divisor) {
                Ok(parsed) => write_out(
                    out,
                    $wrapper(OpenPitParamDecimal::from_decimal(parsed.to_decimal())),
                    out_error,
                ),
                Err(error) => {
                    consume_param_error_with_code(out_error, error);
                    false
                }
            }
        }

        #[no_mangle]
        pub unsafe extern "C" fn $checked_rem_i64_fn(
            value: $wrapper,
            divisor: i64,
            out: *mut $wrapper,
            out_error: OpenPitOutParamError,
        ) -> bool {
            let value = match value.to_param() {
                Ok(value) => value,
                Err(error) => {
                    write_param_error_unspecified(out_error, error.as_str());
                    return false;
                }
            };
            match value.checked_rem_i64(divisor) {
                Ok(parsed) => write_out(
                    out,
                    $wrapper(OpenPitParamDecimal::from_decimal(parsed.to_decimal())),
                    out_error,
                ),
                Err(error) => {
                    consume_param_error_with_code(out_error, error);
                    false
                }
            }
        }

        #[no_mangle]
        pub unsafe extern "C" fn $checked_rem_u64_fn(
            value: $wrapper,
            divisor: u64,
            out: *mut $wrapper,
            out_error: OpenPitOutParamError,
        ) -> bool {
            let value = match value.to_param() {
                Ok(value) => value,
                Err(error) => {
                    write_param_error_unspecified(out_error, error.as_str());
                    return false;
                }
            };
            match value.checked_rem_u64(divisor) {
                Ok(parsed) => write_out(
                    out,
                    $wrapper(OpenPitParamDecimal::from_decimal(parsed.to_decimal())),
                    out_error,
                ),
                Err(error) => {
                    consume_param_error_with_code(out_error, error);
                    false
                }
            }
        }

        #[no_mangle]
        pub unsafe extern "C" fn $checked_rem_f64_fn(
            value: $wrapper,
            divisor: f64,
            out: *mut $wrapper,
            out_error: OpenPitOutParamError,
        ) -> bool {
            let value = match value.to_param() {
                Ok(value) => value,
                Err(error) => {
                    write_param_error_unspecified(out_error, error.as_str());
                    return false;
                }
            };
            match value.checked_rem_f64(divisor) {
                Ok(parsed) => write_out(
                    out,
                    $wrapper(OpenPitParamDecimal::from_decimal(parsed.to_decimal())),
                    out_error,
                ),
                Err(error) => {
                    consume_param_error_with_code(out_error, error);
                    false
                }
            }
        }
    };
}

macro_rules! define_decimal_param_ffi_signed {
    (
        wrapper = $wrapper:ident,
        domain = $domain:ty,
        type_name = $type_name:literal,
        checked_neg_fn = $checked_neg_fn:ident
    ) => {
        #[no_mangle]
        pub unsafe extern "C" fn $checked_neg_fn(
            value: $wrapper,
            out: *mut $wrapper,
            out_error: OpenPitOutParamError,
        ) -> bool {
            let value = match value.to_param() {
                Ok(value) => value,
                Err(error) => {
                    write_param_error_unspecified(out_error, error.as_str());
                    return false;
                }
            };
            match value.checked_neg() {
                Ok(parsed) => write_out(
                    out,
                    $wrapper(OpenPitParamDecimal::from_decimal(parsed.to_decimal())),
                    out_error,
                ),
                Err(error) => {
                    consume_param_error_with_code(out_error, error);
                    false
                }
            }
        }
    };
}

define_decimal_param_ffi_common!(
    wrapper = OpenPitParamPnl,
    domain = Pnl,
    type_name = "Pnl",
    from_string_fn = openpit_create_param_pnl_from_string,
    from_f64_fn = openpit_create_param_pnl_from_f64,
    from_int64_fn = openpit_create_param_pnl_from_int64,
    from_uint64_fn = openpit_create_param_pnl_from_uint64,
    from_string_rounded_fn = openpit_create_param_pnl_from_string_rounded,
    from_f64_rounded_fn = openpit_create_param_pnl_from_f64_rounded,
    from_decimal_rounded_fn = openpit_create_param_pnl_from_decimal_rounded,
    to_f64_fn = openpit_param_pnl_to_f64,
    is_zero_fn = openpit_param_pnl_is_zero,
    compare_fn = openpit_param_pnl_compare,
    to_string_fn = openpit_param_pnl_to_string,
    checked_add_fn = openpit_param_pnl_checked_add,
    checked_sub_fn = openpit_param_pnl_checked_sub,
    checked_mul_i64_fn = openpit_param_pnl_checked_mul_i64,
    checked_mul_u64_fn = openpit_param_pnl_checked_mul_u64,
    checked_mul_f64_fn = openpit_param_pnl_checked_mul_f64,
    checked_div_i64_fn = openpit_param_pnl_checked_div_i64,
    checked_div_u64_fn = openpit_param_pnl_checked_div_u64,
    checked_div_f64_fn = openpit_param_pnl_checked_div_f64,
    checked_rem_i64_fn = openpit_param_pnl_checked_rem_i64,
    checked_rem_u64_fn = openpit_param_pnl_checked_rem_u64,
    checked_rem_f64_fn = openpit_param_pnl_checked_rem_f64
);
define_decimal_param_ffi_signed!(
    wrapper = OpenPitParamPnl,
    domain = Pnl,
    type_name = "Pnl",
    checked_neg_fn = openpit_param_pnl_checked_neg
);

define_decimal_param_ffi_common!(
    wrapper = OpenPitParamPrice,
    domain = Price,
    type_name = "Price",
    from_string_fn = openpit_create_param_price_from_string,
    from_f64_fn = openpit_create_param_price_from_f64,
    from_int64_fn = openpit_create_param_price_from_int64,
    from_uint64_fn = openpit_create_param_price_from_uint64,
    from_string_rounded_fn = openpit_create_param_price_from_string_rounded,
    from_f64_rounded_fn = openpit_create_param_price_from_f64_rounded,
    from_decimal_rounded_fn = openpit_create_param_price_from_decimal_rounded,
    to_f64_fn = openpit_param_price_to_f64,
    is_zero_fn = openpit_param_price_is_zero,
    compare_fn = openpit_param_price_compare,
    to_string_fn = openpit_param_price_to_string,
    checked_add_fn = openpit_param_price_checked_add,
    checked_sub_fn = openpit_param_price_checked_sub,
    checked_mul_i64_fn = openpit_param_price_checked_mul_i64,
    checked_mul_u64_fn = openpit_param_price_checked_mul_u64,
    checked_mul_f64_fn = openpit_param_price_checked_mul_f64,
    checked_div_i64_fn = openpit_param_price_checked_div_i64,
    checked_div_u64_fn = openpit_param_price_checked_div_u64,
    checked_div_f64_fn = openpit_param_price_checked_div_f64,
    checked_rem_i64_fn = openpit_param_price_checked_rem_i64,
    checked_rem_u64_fn = openpit_param_price_checked_rem_u64,
    checked_rem_f64_fn = openpit_param_price_checked_rem_f64
);
define_decimal_param_ffi_signed!(
    wrapper = OpenPitParamPrice,
    domain = Price,
    type_name = "Price",
    checked_neg_fn = openpit_param_price_checked_neg
);

define_decimal_param_ffi_common!(
    wrapper = OpenPitParamQuantity,
    domain = Quantity,
    type_name = "Quantity",
    from_string_fn = openpit_create_param_quantity_from_string,
    from_f64_fn = openpit_create_param_quantity_from_f64,
    from_int64_fn = openpit_create_param_quantity_from_int64,
    from_uint64_fn = openpit_create_param_quantity_from_uint64,
    from_string_rounded_fn = openpit_create_param_quantity_from_string_rounded,
    from_f64_rounded_fn = openpit_create_param_quantity_from_f64_rounded,
    from_decimal_rounded_fn = openpit_create_param_quantity_from_decimal_rounded,
    to_f64_fn = openpit_param_quantity_to_f64,
    is_zero_fn = openpit_param_quantity_is_zero,
    compare_fn = openpit_param_quantity_compare,
    to_string_fn = openpit_param_quantity_to_string,
    checked_add_fn = openpit_param_quantity_checked_add,
    checked_sub_fn = openpit_param_quantity_checked_sub,
    checked_mul_i64_fn = openpit_param_quantity_checked_mul_i64,
    checked_mul_u64_fn = openpit_param_quantity_checked_mul_u64,
    checked_mul_f64_fn = openpit_param_quantity_checked_mul_f64,
    checked_div_i64_fn = openpit_param_quantity_checked_div_i64,
    checked_div_u64_fn = openpit_param_quantity_checked_div_u64,
    checked_div_f64_fn = openpit_param_quantity_checked_div_f64,
    checked_rem_i64_fn = openpit_param_quantity_checked_rem_i64,
    checked_rem_u64_fn = openpit_param_quantity_checked_rem_u64,
    checked_rem_f64_fn = openpit_param_quantity_checked_rem_f64
);

define_decimal_param_ffi_common!(
    wrapper = OpenPitParamVolume,
    domain = Volume,
    type_name = "Volume",
    from_string_fn = openpit_create_param_volume_from_string,
    from_f64_fn = openpit_create_param_volume_from_f64,
    from_int64_fn = openpit_create_param_volume_from_int64,
    from_uint64_fn = openpit_create_param_volume_from_uint64,
    from_string_rounded_fn = openpit_create_param_volume_from_string_rounded,
    from_f64_rounded_fn = openpit_create_param_volume_from_f64_rounded,
    from_decimal_rounded_fn = openpit_create_param_volume_from_decimal_rounded,
    to_f64_fn = openpit_param_volume_to_f64,
    is_zero_fn = openpit_param_volume_is_zero,
    compare_fn = openpit_param_volume_compare,
    to_string_fn = openpit_param_volume_to_string,
    checked_add_fn = openpit_param_volume_checked_add,
    checked_sub_fn = openpit_param_volume_checked_sub,
    checked_mul_i64_fn = openpit_param_volume_checked_mul_i64,
    checked_mul_u64_fn = openpit_param_volume_checked_mul_u64,
    checked_mul_f64_fn = openpit_param_volume_checked_mul_f64,
    checked_div_i64_fn = openpit_param_volume_checked_div_i64,
    checked_div_u64_fn = openpit_param_volume_checked_div_u64,
    checked_div_f64_fn = openpit_param_volume_checked_div_f64,
    checked_rem_i64_fn = openpit_param_volume_checked_rem_i64,
    checked_rem_u64_fn = openpit_param_volume_checked_rem_u64,
    checked_rem_f64_fn = openpit_param_volume_checked_rem_f64
);

define_decimal_param_ffi_common!(
    wrapper = OpenPitParamCashFlow,
    domain = CashFlow,
    type_name = "CashFlow",
    from_string_fn = openpit_create_param_cash_flow_from_string,
    from_f64_fn = openpit_create_param_cash_flow_from_f64,
    from_int64_fn = openpit_create_param_cash_flow_from_int64,
    from_uint64_fn = openpit_create_param_cash_flow_from_uint64,
    from_string_rounded_fn = openpit_create_param_cash_flow_from_string_rounded,
    from_f64_rounded_fn = openpit_create_param_cash_flow_from_f64_rounded,
    from_decimal_rounded_fn = openpit_create_param_cash_flow_from_decimal_rounded,
    to_f64_fn = openpit_param_cash_flow_to_f64,
    is_zero_fn = openpit_param_cash_flow_is_zero,
    compare_fn = openpit_param_cash_flow_compare,
    to_string_fn = openpit_param_cash_flow_to_string,
    checked_add_fn = openpit_param_cash_flow_checked_add,
    checked_sub_fn = openpit_param_cash_flow_checked_sub,
    checked_mul_i64_fn = openpit_param_cash_flow_checked_mul_i64,
    checked_mul_u64_fn = openpit_param_cash_flow_checked_mul_u64,
    checked_mul_f64_fn = openpit_param_cash_flow_checked_mul_f64,
    checked_div_i64_fn = openpit_param_cash_flow_checked_div_i64,
    checked_div_u64_fn = openpit_param_cash_flow_checked_div_u64,
    checked_div_f64_fn = openpit_param_cash_flow_checked_div_f64,
    checked_rem_i64_fn = openpit_param_cash_flow_checked_rem_i64,
    checked_rem_u64_fn = openpit_param_cash_flow_checked_rem_u64,
    checked_rem_f64_fn = openpit_param_cash_flow_checked_rem_f64
);
define_decimal_param_ffi_signed!(
    wrapper = OpenPitParamCashFlow,
    domain = CashFlow,
    type_name = "CashFlow",
    checked_neg_fn = openpit_param_cash_flow_checked_neg
);

define_decimal_param_ffi_common!(
    wrapper = OpenPitParamPositionSize,
    domain = PositionSize,
    type_name = "PositionSize",
    from_string_fn = openpit_create_param_position_size_from_string,
    from_f64_fn = openpit_create_param_position_size_from_f64,
    from_int64_fn = openpit_create_param_position_size_from_int64,
    from_uint64_fn = openpit_create_param_position_size_from_uint64,
    from_string_rounded_fn = openpit_create_param_position_size_from_string_rounded,
    from_f64_rounded_fn = openpit_create_param_position_size_from_f64_rounded,
    from_decimal_rounded_fn = openpit_create_param_position_size_from_decimal_rounded,
    to_f64_fn = openpit_param_position_size_to_f64,
    is_zero_fn = openpit_param_position_size_is_zero,
    compare_fn = openpit_param_position_size_compare,
    to_string_fn = openpit_param_position_size_to_string,
    checked_add_fn = openpit_param_position_size_checked_add,
    checked_sub_fn = openpit_param_position_size_checked_sub,
    checked_mul_i64_fn = openpit_param_position_size_checked_mul_i64,
    checked_mul_u64_fn = openpit_param_position_size_checked_mul_u64,
    checked_mul_f64_fn = openpit_param_position_size_checked_mul_f64,
    checked_div_i64_fn = openpit_param_position_size_checked_div_i64,
    checked_div_u64_fn = openpit_param_position_size_checked_div_u64,
    checked_div_f64_fn = openpit_param_position_size_checked_div_f64,
    checked_rem_i64_fn = openpit_param_position_size_checked_rem_i64,
    checked_rem_u64_fn = openpit_param_position_size_checked_rem_u64,
    checked_rem_f64_fn = openpit_param_position_size_checked_rem_f64
);
define_decimal_param_ffi_signed!(
    wrapper = OpenPitParamPositionSize,
    domain = PositionSize,
    type_name = "PositionSize",
    checked_neg_fn = openpit_param_position_size_checked_neg
);

define_decimal_param_ffi_common!(
    wrapper = OpenPitParamFee,
    domain = Fee,
    type_name = "Fee",
    from_string_fn = openpit_create_param_fee_from_string,
    from_f64_fn = openpit_create_param_fee_from_f64,
    from_int64_fn = openpit_create_param_fee_from_int64,
    from_uint64_fn = openpit_create_param_fee_from_uint64,
    from_string_rounded_fn = openpit_create_param_fee_from_string_rounded,
    from_f64_rounded_fn = openpit_create_param_fee_from_f64_rounded,
    from_decimal_rounded_fn = openpit_create_param_fee_from_decimal_rounded,
    to_f64_fn = openpit_param_fee_to_f64,
    is_zero_fn = openpit_param_fee_is_zero,
    compare_fn = openpit_param_fee_compare,
    to_string_fn = openpit_param_fee_to_string,
    checked_add_fn = openpit_param_fee_checked_add,
    checked_sub_fn = openpit_param_fee_checked_sub,
    checked_mul_i64_fn = openpit_param_fee_checked_mul_i64,
    checked_mul_u64_fn = openpit_param_fee_checked_mul_u64,
    checked_mul_f64_fn = openpit_param_fee_checked_mul_f64,
    checked_div_i64_fn = openpit_param_fee_checked_div_i64,
    checked_div_u64_fn = openpit_param_fee_checked_div_u64,
    checked_div_f64_fn = openpit_param_fee_checked_div_f64,
    checked_rem_i64_fn = openpit_param_fee_checked_rem_i64,
    checked_rem_u64_fn = openpit_param_fee_checked_rem_u64,
    checked_rem_f64_fn = openpit_param_fee_checked_rem_f64
);
define_decimal_param_ffi_signed!(
    wrapper = OpenPitParamFee,
    domain = Fee,
    type_name = "Fee",
    checked_neg_fn = openpit_param_fee_checked_neg
);

define_decimal_param_ffi_common!(
    wrapper = OpenPitParamNotional,
    domain = Notional,
    type_name = "Notional",
    from_string_fn = openpit_create_param_notional_from_string,
    from_f64_fn = openpit_create_param_notional_from_f64,
    from_int64_fn = openpit_create_param_notional_from_int64,
    from_uint64_fn = openpit_create_param_notional_from_uint64,
    from_string_rounded_fn = openpit_create_param_notional_from_string_rounded,
    from_f64_rounded_fn = openpit_create_param_notional_from_f64_rounded,
    from_decimal_rounded_fn = openpit_create_param_notional_from_decimal_rounded,
    to_f64_fn = openpit_param_notional_to_f64,
    is_zero_fn = openpit_param_notional_is_zero,
    compare_fn = openpit_param_notional_compare,
    to_string_fn = openpit_param_notional_to_string,
    checked_add_fn = openpit_param_notional_checked_add,
    checked_sub_fn = openpit_param_notional_checked_sub,
    checked_mul_i64_fn = openpit_param_notional_checked_mul_i64,
    checked_mul_u64_fn = openpit_param_notional_checked_mul_u64,
    checked_mul_f64_fn = openpit_param_notional_checked_mul_f64,
    checked_div_i64_fn = openpit_param_notional_checked_div_i64,
    checked_div_u64_fn = openpit_param_notional_checked_div_u64,
    checked_div_f64_fn = openpit_param_notional_checked_div_f64,
    checked_rem_i64_fn = openpit_param_notional_checked_rem_i64,
    checked_rem_u64_fn = openpit_param_notional_checked_rem_u64,
    checked_rem_f64_fn = openpit_param_notional_checked_rem_f64
);

define_optional!(
    optional = OpenPitParamNotionalOptional,
    value = OpenPitParamNotional
);
define_optional!(optional = OpenPitParamPnlOptional, value = OpenPitParamPnl);
define_optional!(
    optional = OpenPitParamPriceOptional,
    value = OpenPitParamPrice
);
define_optional!(
    optional = OpenPitParamQuantityOptional,
    value = OpenPitParamQuantity
);
define_optional!(
    optional = OpenPitParamVolumeOptional,
    value = OpenPitParamVolume
);
define_optional!(
    optional = OpenPitParamCashFlowOptional,
    value = OpenPitParamCashFlow
);
define_optional!(
    optional = OpenPitParamPositionSizeOptional,
    value = OpenPitParamPositionSize
);
define_optional!(optional = OpenPitParamFeeOptional, value = OpenPitParamFee);
define_optional!(
    optional = OpenPitParamMonetaryAmountOptional,
    value = OpenPitParamMonetaryAmount
);
define_optional!(
    optional = OpenPitParamAccountIdOptional,
    value = OpenPitParamAccountId
);

pub(crate) fn import_monetary_amount(
    value: OpenPitParamMonetaryAmount,
) -> Result<MonetaryAmount, String> {
    let currency = parse_asset_view(value.currency, "monetary_amount.currency")?
        .ok_or_else(|| "monetary_amount.currency is not set".to_string())?;
    Ok(MonetaryAmount {
        amount: value.amount.to_param()?,
        currency,
    })
}

pub(crate) fn export_monetary_amount(value: &MonetaryAmount) -> OpenPitParamMonetaryAmount {
    OpenPitParamMonetaryAmount {
        amount: OpenPitParamFee(value.amount.to_decimal().into()),
        currency: OpenPitStringView::from_utf8(value.currency.as_ref()),
    }
}

pub(crate) fn import_trade_amount(
    value: OpenPitParamTradeAmount,
) -> Result<Option<TradeAmount>, String> {
    match value.kind {
        OpenPitParamTradeAmountKind::NotSet => Ok(None),
        OpenPitParamTradeAmountKind::Quantity => Ok(Some(TradeAmount::Quantity(
            OpenPitParamQuantity(value.value).to_param()?,
        ))),
        OpenPitParamTradeAmountKind::Volume => Ok(Some(TradeAmount::Volume(
            OpenPitParamVolume(value.value).to_param()?,
        ))),
    }
}

pub(crate) fn export_trade_amount(value: Option<TradeAmount>) -> OpenPitParamTradeAmount {
    match value {
        Some(TradeAmount::Quantity(quantity)) => OpenPitParamTradeAmount {
            value: quantity.to_decimal().into(),
            kind: OpenPitParamTradeAmountKind::Quantity,
        },
        Some(TradeAmount::Volume(volume)) => OpenPitParamTradeAmount {
            value: volume.to_decimal().into(),
            kind: OpenPitParamTradeAmountKind::Volume,
        },
        _ => OpenPitParamTradeAmount::default(),
    }
}

//--------------------------------------------------------------------------------------------------

#[no_mangle]
pub unsafe extern "C" fn openpit_param_leverage_calculate_margin_required(
    leverage: OpenPitParamLeverage,
    notional: OpenPitParamNotional,
    out: *mut OpenPitParamNotional,
    out_error: OpenPitOutParamError,
) -> bool {
    let leverage = match import_leverage(leverage) {
        Some(lev) => lev,
        None => {
            write_param_error_unspecified(out_error, "leverage is not set");
            return false;
        }
    };
    let notional = match notional.to_param() {
        Ok(parsed) => parsed,
        Err(error) => {
            write_param_error_unspecified(out_error, error.as_str());
            return false;
        }
    };
    match leverage.calculate_margin_required(notional) {
        Ok(margin) => write_out(
            out,
            OpenPitParamNotional(OpenPitParamDecimal::from_decimal(margin.to_decimal())),
            out_error,
        ),
        Err(error) => {
            consume_param_error_with_code(out_error, error);
            false
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn openpit_param_price_calculate_volume(
    price: OpenPitParamPrice,
    quantity: OpenPitParamQuantity,
    out: *mut OpenPitParamVolume,
    out_error: OpenPitOutParamError,
) -> bool {
    let price = match price.to_param() {
        Ok(value) => value,
        Err(error) => {
            write_param_error_unspecified(out_error, error.as_str());
            return false;
        }
    };
    let quantity = match quantity.to_param() {
        Ok(value) => value,
        Err(error) => {
            write_param_error_unspecified(out_error, error.as_str());
            return false;
        }
    };
    match price.calculate_volume(quantity) {
        Ok(volume) => write_out(
            out,
            OpenPitParamVolume(OpenPitParamDecimal::from_decimal(volume.to_decimal())),
            out_error,
        ),
        Err(error) => {
            consume_param_error_with_code(out_error, error);
            false
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn openpit_param_price_calculate_position_size(
    price: OpenPitParamPrice,
    quantity: OpenPitParamQuantity,
    out: *mut OpenPitParamPositionSize,
    out_error: OpenPitOutParamError,
) -> bool {
    let price = match price.to_param() {
        Ok(value) => value,
        Err(error) => {
            write_param_error_unspecified(out_error, error.as_str());
            return false;
        }
    };
    let quantity = match quantity.to_param() {
        Ok(value) => value,
        Err(error) => {
            write_param_error_unspecified(out_error, error.as_str());
            return false;
        }
    };
    match price.calculate_position_size(quantity) {
        Ok(position_size) => write_out(
            out,
            OpenPitParamPositionSize(OpenPitParamDecimal::from_decimal(
                position_size.to_decimal(),
            )),
            out_error,
        ),
        Err(error) => {
            consume_param_error_with_code(out_error, error);
            false
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn openpit_param_quantity_calculate_volume(
    quantity: OpenPitParamQuantity,
    price: OpenPitParamPrice,
    out: *mut OpenPitParamVolume,
    out_error: OpenPitOutParamError,
) -> bool {
    let quantity = match quantity.to_param() {
        Ok(value) => value,
        Err(error) => {
            write_param_error_unspecified(out_error, error.as_str());
            return false;
        }
    };
    let price = match price.to_param() {
        Ok(value) => value,
        Err(error) => {
            write_param_error_unspecified(out_error, error.as_str());
            return false;
        }
    };
    match quantity.calculate_volume(price) {
        Ok(volume) => write_out(
            out,
            OpenPitParamVolume(OpenPitParamDecimal::from_decimal(volume.to_decimal())),
            out_error,
        ),
        Err(error) => {
            consume_param_error_with_code(out_error, error);
            false
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn openpit_param_volume_calculate_quantity(
    volume: OpenPitParamVolume,
    price: OpenPitParamPrice,
    out: *mut OpenPitParamQuantity,
    out_error: OpenPitOutParamError,
) -> bool {
    let volume = match volume.to_param() {
        Ok(value) => value,
        Err(error) => {
            write_param_error_unspecified(out_error, error.as_str());
            return false;
        }
    };
    let price = match price.to_param() {
        Ok(value) => value,
        Err(error) => {
            write_param_error_unspecified(out_error, error.as_str());
            return false;
        }
    };
    match volume.calculate_quantity(price) {
        Ok(quantity) => write_out(
            out,
            OpenPitParamQuantity(OpenPitParamDecimal::from_decimal(quantity.to_decimal())),
            out_error,
        ),
        Err(error) => {
            consume_param_error_with_code(out_error, error);
            false
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn openpit_param_pnl_to_cash_flow(
    value: OpenPitParamPnl,
    out: *mut OpenPitParamCashFlow,
    out_error: OpenPitOutParamError,
) -> bool {
    let parsed = match value.to_param() {
        Ok(parsed) => parsed,
        Err(error) => {
            write_param_error_unspecified(out_error, error.as_str());
            return false;
        }
    };
    write_out(
        out,
        OpenPitParamCashFlow(OpenPitParamDecimal::from_decimal(
            parsed.to_cash_flow().to_decimal(),
        )),
        out_error,
    )
}

#[no_mangle]
pub unsafe extern "C" fn openpit_param_pnl_to_position_size(
    value: OpenPitParamPnl,
    out: *mut OpenPitParamPositionSize,
    out_error: OpenPitOutParamError,
) -> bool {
    let parsed = match value.to_param() {
        Ok(parsed) => parsed,
        Err(error) => {
            write_param_error_unspecified(out_error, error.as_str());
            return false;
        }
    };
    write_out(
        out,
        OpenPitParamPositionSize(OpenPitParamDecimal::from_decimal(
            parsed.to_position_size().to_decimal(),
        )),
        out_error,
    )
}

#[no_mangle]
pub unsafe extern "C" fn openpit_param_quantity_to_position_size(
    value: OpenPitParamQuantity,
    out: *mut OpenPitParamPositionSize,
    out_error: OpenPitOutParamError,
) -> bool {
    let parsed = match value.to_param() {
        Ok(parsed) => parsed,
        Err(error) => {
            write_param_error_unspecified(out_error, error.as_str());
            return false;
        }
    };
    write_out(
        out,
        OpenPitParamPositionSize(OpenPitParamDecimal::from_decimal(
            parsed.to_position_size().to_decimal(),
        )),
        out_error,
    )
}

#[no_mangle]
pub unsafe extern "C" fn openpit_param_volume_to_position_size(
    value: OpenPitParamVolume,
    out: *mut OpenPitParamPositionSize,
    out_error: OpenPitOutParamError,
) -> bool {
    let parsed = match value.to_param() {
        Ok(parsed) => parsed,
        Err(error) => {
            write_param_error_unspecified(out_error, error.as_str());
            return false;
        }
    };
    write_out(
        out,
        OpenPitParamPositionSize(OpenPitParamDecimal::from_decimal(
            parsed.to_position_size().to_decimal(),
        )),
        out_error,
    )
}

#[no_mangle]
pub unsafe extern "C" fn openpit_param_pnl_from_fee(
    fee: OpenPitParamFee,
    out: *mut OpenPitParamPnl,
    out_error: OpenPitOutParamError,
) -> bool {
    let parsed = match fee.to_param() {
        Ok(parsed) => parsed,
        Err(error) => {
            write_param_error_unspecified(out_error, error.as_str());
            return false;
        }
    };
    write_out(
        out,
        OpenPitParamPnl(OpenPitParamDecimal::from_decimal(
            Pnl::from_fee(parsed).to_decimal(),
        )),
        out_error,
    )
}

#[no_mangle]
pub unsafe extern "C" fn openpit_param_cash_flow_from_pnl(
    pnl: OpenPitParamPnl,
    out: *mut OpenPitParamCashFlow,
    out_error: OpenPitOutParamError,
) -> bool {
    let parsed = match pnl.to_param() {
        Ok(parsed) => parsed,
        Err(error) => {
            write_param_error_unspecified(out_error, error.as_str());
            return false;
        }
    };
    write_out(
        out,
        OpenPitParamCashFlow(OpenPitParamDecimal::from_decimal(
            CashFlow::from_pnl(parsed).to_decimal(),
        )),
        out_error,
    )
}

#[no_mangle]
pub unsafe extern "C" fn openpit_param_cash_flow_from_fee(
    fee: OpenPitParamFee,
    out: *mut OpenPitParamCashFlow,
    out_error: OpenPitOutParamError,
) -> bool {
    let parsed = match fee.to_param() {
        Ok(parsed) => parsed,
        Err(error) => {
            write_param_error_unspecified(out_error, error.as_str());
            return false;
        }
    };
    write_out(
        out,
        OpenPitParamCashFlow(OpenPitParamDecimal::from_decimal(
            CashFlow::from_fee(parsed).to_decimal(),
        )),
        out_error,
    )
}

#[no_mangle]
pub unsafe extern "C" fn openpit_param_cash_flow_from_volume_inflow(
    volume: OpenPitParamVolume,
    out: *mut OpenPitParamCashFlow,
    out_error: OpenPitOutParamError,
) -> bool {
    let parsed = match volume.to_param() {
        Ok(parsed) => parsed,
        Err(error) => {
            write_param_error_unspecified(out_error, error.as_str());
            return false;
        }
    };
    write_out(
        out,
        OpenPitParamCashFlow(OpenPitParamDecimal::from_decimal(
            CashFlow::from_volume_inflow(parsed).to_decimal(),
        )),
        out_error,
    )
}

#[no_mangle]
pub unsafe extern "C" fn openpit_param_cash_flow_from_volume_outflow(
    volume: OpenPitParamVolume,
    out: *mut OpenPitParamCashFlow,
    out_error: OpenPitOutParamError,
) -> bool {
    let parsed = match volume.to_param() {
        Ok(parsed) => parsed,
        Err(error) => {
            write_param_error_unspecified(out_error, error.as_str());
            return false;
        }
    };
    write_out(
        out,
        OpenPitParamCashFlow(OpenPitParamDecimal::from_decimal(
            CashFlow::from_volume_outflow(parsed).to_decimal(),
        )),
        out_error,
    )
}

#[no_mangle]
pub unsafe extern "C" fn openpit_param_fee_to_pnl(
    fee: OpenPitParamFee,
    out: *mut OpenPitParamPnl,
    out_error: OpenPitOutParamError,
) -> bool {
    let parsed = match fee.to_param() {
        Ok(parsed) => parsed,
        Err(error) => {
            write_param_error_unspecified(out_error, error.as_str());
            return false;
        }
    };
    write_out(
        out,
        OpenPitParamPnl(OpenPitParamDecimal::from_decimal(
            parsed.to_pnl().to_decimal(),
        )),
        out_error,
    )
}

#[no_mangle]
pub unsafe extern "C" fn openpit_param_fee_to_position_size(
    fee: OpenPitParamFee,
    out: *mut OpenPitParamPositionSize,
    out_error: OpenPitOutParamError,
) -> bool {
    let parsed = match fee.to_param() {
        Ok(parsed) => parsed,
        Err(error) => {
            write_param_error_unspecified(out_error, error.as_str());
            return false;
        }
    };
    write_out(
        out,
        OpenPitParamPositionSize(OpenPitParamDecimal::from_decimal(
            parsed.to_position_size().to_decimal(),
        )),
        out_error,
    )
}

#[no_mangle]
pub unsafe extern "C" fn openpit_param_fee_to_cash_flow(
    fee: OpenPitParamFee,
    out: *mut OpenPitParamCashFlow,
    out_error: OpenPitOutParamError,
) -> bool {
    let parsed = match fee.to_param() {
        Ok(parsed) => parsed,
        Err(error) => {
            write_param_error_unspecified(out_error, error.as_str());
            return false;
        }
    };
    write_out(
        out,
        OpenPitParamCashFlow(OpenPitParamDecimal::from_decimal(
            parsed.to_cash_flow().to_decimal(),
        )),
        out_error,
    )
}

#[no_mangle]
pub unsafe extern "C" fn openpit_param_volume_to_cash_flow_inflow(
    volume: OpenPitParamVolume,
    out: *mut OpenPitParamCashFlow,
    out_error: OpenPitOutParamError,
) -> bool {
    let parsed = match volume.to_param() {
        Ok(parsed) => parsed,
        Err(error) => {
            write_param_error_unspecified(out_error, error.as_str());
            return false;
        }
    };
    write_out(
        out,
        OpenPitParamCashFlow(OpenPitParamDecimal::from_decimal(
            parsed.to_cash_flow_inflow().to_decimal(),
        )),
        out_error,
    )
}

#[no_mangle]
pub unsafe extern "C" fn openpit_param_volume_to_cash_flow_outflow(
    volume: OpenPitParamVolume,
    out: *mut OpenPitParamCashFlow,
    out_error: OpenPitOutParamError,
) -> bool {
    let parsed = match volume.to_param() {
        Ok(parsed) => parsed,
        Err(error) => {
            write_param_error_unspecified(out_error, error.as_str());
            return false;
        }
    };
    write_out(
        out,
        OpenPitParamCashFlow(OpenPitParamDecimal::from_decimal(
            parsed.to_cash_flow_outflow().to_decimal(),
        )),
        out_error,
    )
}

#[no_mangle]
pub unsafe extern "C" fn openpit_param_position_size_from_pnl(
    pnl: OpenPitParamPnl,
    out: *mut OpenPitParamPositionSize,
    out_error: OpenPitOutParamError,
) -> bool {
    let parsed = match pnl.to_param() {
        Ok(parsed) => parsed,
        Err(error) => {
            write_param_error_unspecified(out_error, error.as_str());
            return false;
        }
    };
    write_out(
        out,
        OpenPitParamPositionSize(OpenPitParamDecimal::from_decimal(
            PositionSize::from_pnl(parsed).to_decimal(),
        )),
        out_error,
    )
}

#[no_mangle]
pub unsafe extern "C" fn openpit_param_position_size_from_fee(
    fee: OpenPitParamFee,
    out: *mut OpenPitParamPositionSize,
    out_error: OpenPitOutParamError,
) -> bool {
    let parsed = match fee.to_param() {
        Ok(parsed) => parsed,
        Err(error) => {
            write_param_error_unspecified(out_error, error.as_str());
            return false;
        }
    };
    write_out(
        out,
        OpenPitParamPositionSize(OpenPitParamDecimal::from_decimal(
            PositionSize::from_fee(parsed).to_decimal(),
        )),
        out_error,
    )
}

#[no_mangle]
pub unsafe extern "C" fn openpit_param_position_size_from_quantity_and_side(
    quantity: OpenPitParamQuantity,
    side: OpenPitParamSide,
    out: *mut OpenPitParamPositionSize,
    out_error: OpenPitOutParamError,
) -> bool {
    let parsed = match quantity.to_param() {
        Ok(parsed) => parsed,
        Err(error) => {
            write_param_error_unspecified(out_error, error.as_str());
            return false;
        }
    };
    let parsed_side = match import_side(side) {
        Some(side) => side,
        None => {
            write_param_error_unspecified(out_error, "side is not set");
            return false;
        }
    };
    write_out(
        out,
        OpenPitParamPositionSize(OpenPitParamDecimal::from_decimal(
            PositionSize::from_quantity_and_side(parsed, parsed_side).to_decimal(),
        )),
        out_error,
    )
}

#[no_mangle]
pub unsafe extern "C" fn openpit_param_position_size_to_open_quantity(
    value: OpenPitParamPositionSize,
    out_quantity: *mut OpenPitParamQuantity,
    out_side: *mut OpenPitParamSide,
    out_error: OpenPitOutParamError,
) -> bool {
    if out_quantity.is_null() || out_side.is_null() {
        write_param_error_unspecified(out_error, "result place pointer is null");
        return false;
    }

    let position_size = match value.to_param() {
        Ok(value) => value,
        Err(error) => {
            write_param_error_unspecified(out_error, error.as_str());
            return false;
        }
    };
    let (quantity, side) = position_size.to_open_quantity();
    unsafe {
        *out_quantity =
            OpenPitParamQuantity(OpenPitParamDecimal::from_decimal(quantity.to_decimal()));
        *out_side = export_side(side);
    }
    true
}

#[no_mangle]
pub unsafe extern "C" fn openpit_param_position_size_to_close_quantity(
    value: OpenPitParamPositionSize,
    out_quantity: *mut OpenPitParamQuantity,
    out_side: *mut OpenPitParamSide,
    out_error: OpenPitOutParamError,
) -> bool {
    if out_quantity.is_null() || out_side.is_null() {
        write_param_error_unspecified(out_error, "result place pointer is null");
        return false;
    }
    let position_size = match value.to_param() {
        Ok(value) => value,
        Err(error) => {
            write_param_error_unspecified(out_error, error.as_str());
            return false;
        }
    };
    let (quantity, side) = position_size.to_close_quantity();
    unsafe {
        *out_quantity =
            OpenPitParamQuantity(OpenPitParamDecimal::from_decimal(quantity.to_decimal()));
        *out_side = side.map(export_side).unwrap_or(OpenPitParamSide::NotSet);
    }
    true
}

#[no_mangle]
pub unsafe extern "C" fn openpit_param_position_size_checked_add_quantity(
    value: OpenPitParamPositionSize,
    quantity: OpenPitParamQuantity,
    side: OpenPitParamSide,
    out: *mut OpenPitParamPositionSize,
    out_error: OpenPitOutParamError,
) -> bool {
    let value = match value.to_param() {
        Ok(value) => value,
        Err(error) => {
            write_param_error_unspecified(out_error, error.as_str());
            return false;
        }
    };
    let quantity = match quantity.to_param() {
        Ok(value) => value,
        Err(error) => {
            write_param_error_unspecified(out_error, error.as_str());
            return false;
        }
    };
    let side = match import_side(side) {
        Some(side) => side,
        None => {
            write_param_error_unspecified(out_error, "side is not set");
            return false;
        }
    };
    match value.checked_add_quantity(quantity, side) {
        Ok(position) => write_out(
            out,
            OpenPitParamPositionSize(OpenPitParamDecimal::from_decimal(position.to_decimal())),
            out_error,
        ),
        Err(error) => {
            consume_param_error_with_code(out_error, error);
            false
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn openpit_param_price_calculate_notional(
    price: OpenPitParamPrice,
    quantity: OpenPitParamQuantity,
    out: *mut OpenPitParamNotional,
    out_error: OpenPitOutParamError,
) -> bool {
    let price = match price.to_param() {
        Ok(value) => value,
        Err(error) => {
            write_param_error_unspecified(out_error, error.as_str());
            return false;
        }
    };
    let quantity = match quantity.to_param() {
        Ok(value) => value,
        Err(error) => {
            write_param_error_unspecified(out_error, error.as_str());
            return false;
        }
    };
    match Notional::from_price_quantity(price, quantity) {
        Ok(notional) => write_out(
            out,
            OpenPitParamNotional(OpenPitParamDecimal::from_decimal(notional.to_decimal())),
            out_error,
        ),
        Err(error) => {
            consume_param_error_with_code(out_error, error);
            false
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn openpit_param_quantity_calculate_notional(
    quantity: OpenPitParamQuantity,
    price: OpenPitParamPrice,
    out: *mut OpenPitParamNotional,
    out_error: OpenPitOutParamError,
) -> bool {
    let quantity = match quantity.to_param() {
        Ok(value) => value,
        Err(error) => {
            write_param_error_unspecified(out_error, error.as_str());
            return false;
        }
    };
    let price = match price.to_param() {
        Ok(value) => value,
        Err(error) => {
            write_param_error_unspecified(out_error, error.as_str());
            return false;
        }
    };
    match Notional::from_price_quantity(price, quantity) {
        Ok(notional) => write_out(
            out,
            OpenPitParamNotional(OpenPitParamDecimal::from_decimal(notional.to_decimal())),
            out_error,
        ),
        Err(error) => {
            consume_param_error_with_code(out_error, error);
            false
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn openpit_param_notional_from_volume(
    volume: OpenPitParamVolume,
    out: *mut OpenPitParamNotional,
    out_error: OpenPitOutParamError,
) -> bool {
    let parsed = match volume.to_param() {
        Ok(parsed) => parsed,
        Err(error) => {
            write_param_error_unspecified(out_error, error.as_str());
            return false;
        }
    };
    write_out(
        out,
        OpenPitParamNotional(OpenPitParamDecimal::from_decimal(
            Notional::from_volume(parsed).to_decimal(),
        )),
        out_error,
    )
}

#[no_mangle]
pub unsafe extern "C" fn openpit_param_notional_to_volume(
    notional: OpenPitParamNotional,
    out: *mut OpenPitParamVolume,
    out_error: OpenPitOutParamError,
) -> bool {
    let parsed = match notional.to_param() {
        Ok(parsed) => parsed,
        Err(error) => {
            write_param_error_unspecified(out_error, error.as_str());
            return false;
        }
    };
    write_out(
        out,
        OpenPitParamVolume(OpenPitParamDecimal::from_decimal(
            parsed.to_volume().to_decimal(),
        )),
        out_error,
    )
}

#[no_mangle]
pub unsafe extern "C" fn openpit_param_notional_calculate_margin_required(
    notional: OpenPitParamNotional,
    leverage: OpenPitParamLeverage,
    out: *mut OpenPitParamNotional,
    out_error: OpenPitOutParamError,
) -> bool {
    let notional = match notional.to_param() {
        Ok(parsed) => parsed,
        Err(error) => {
            write_param_error_unspecified(out_error, error.as_str());
            return false;
        }
    };
    let leverage = match import_leverage(leverage) {
        Some(lev) => lev,
        None => {
            write_param_error_unspecified(out_error, "leverage is not set");
            return false;
        }
    };
    match notional.calculate_margin_required(leverage) {
        Ok(margin) => write_out(
            out,
            OpenPitParamNotional(OpenPitParamDecimal::from_decimal(margin.to_decimal())),
            out_error,
        ),
        Err(error) => {
            consume_param_error_with_code(out_error, error);
            false
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn openpit_param_volume_from_notional(
    notional: OpenPitParamNotional,
    out: *mut OpenPitParamVolume,
    out_error: OpenPitOutParamError,
) -> bool {
    let parsed = match notional.to_param() {
        Ok(parsed) => parsed,
        Err(error) => {
            write_param_error_unspecified(out_error, error.as_str());
            return false;
        }
    };
    write_out(
        out,
        OpenPitParamVolume(OpenPitParamDecimal::from_decimal(
            Volume::from_notional(parsed).to_decimal(),
        )),
        out_error,
    )
}

//--------------------------------------------------------------------------------------------------

#[no_mangle]
/// Constructs an account identifier from a 64-bit integer.
///
/// This is a direct numeric mapping with no collision risk.
///
/// WARNING:
/// Do not mix IDs produced by this function with IDs produced by
/// `openpit_create_param_account_id_from_string` in the same runtime state.
///
/// Contract:
/// - returns a stable account identifier value;
/// - this function always succeeds.
pub extern "C" fn openpit_create_param_account_id_from_uint64(value: u64) -> OpenPitParamAccountId {
    AccountId::from_u64(value).as_u64()
}

#[no_mangle]
/// Constructs an account identifier from a UTF-8 byte sequence.
///
/// The bytes are read only for the duration of the call. No trailing zero byte
/// is required.
///
/// Collision note:
/// - different account strings can map to the same identifier;
/// - for `n` distinct account strings the probability of at least one collision
///   is approximately `n^2 / 2^65`.
/// - if collision risk is unacceptable, keep your own collision-free
///   string-to-integer mapping and use `openpit_create_param_account_id_from_uint64`.
///
/// The previous sentence is why this helper is suitable for stable adapter-side
/// mapping, but not for workflows that require guaranteed uniqueness.
///
/// WARNING:
/// Do not mix IDs produced by this function with IDs produced by
/// `openpit_create_param_account_id_from_uint64` in the same runtime state.
///
/// Contract:
/// - returns `true` and writes a stable account identifier to `out` on success;
/// - returns `false` on invalid input and optionally writes `OpenPitParamError`.
///
/// # Safety
///
/// `value.ptr` must be non-null and point to at least `value.len` readable
/// UTF-8 bytes.
pub unsafe extern "C" fn openpit_create_param_account_id_from_string(
    value: OpenPitStringView,
    out: *mut OpenPitParamAccountId,
    out_error: OpenPitOutParamError,
) -> bool {
    let bytes: &[u8] = if value.ptr.is_null() || value.len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(value.ptr, value.len) }
    };
    let utf8 = String::from_utf8_lossy(bytes);
    let s = utf8.as_ref();
    match AccountId::from_str(s) {
        Ok(id) => write_out(out, id.as_u64(), out_error),
        Err(error) => {
            consume_param_error_with_code(out_error, error.into());
            false
        }
    }
}

#[no_mangle]
/// Validates and copies an asset identifier into a caller-owned shared-string handle.
///
/// The returned handle must be destroyed with `openpit_destroy_param_asset`.
pub unsafe extern "C" fn openpit_create_param_asset_from_string(
    value: OpenPitStringView,
    out_error: OpenPitOutParamError,
) -> *mut OpenPitSharedString {
    let text = match unsafe { parse_string_view(value) } {
        Ok(text) => text,
        Err(error) => {
            write_param_error_unspecified(out_error, error.as_str());
            return std::ptr::null_mut();
        }
    };
    match Asset::new(text.as_str()) {
        Ok(asset) => OpenPitSharedString::new_handle(asset.as_ref()),
        Err(error) => {
            consume_param_error_with_code(out_error, error.into());
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
/// Destroys a caller-owned asset handle created by `openpit_create_param_asset_from_string`.
pub extern "C" fn openpit_destroy_param_asset(handle: *mut OpenPitSharedString) {
    crate::string::openpit_destroy_shared_string(handle);
}

//--------------------------------------------------------------------------------------------------

/// Renders an order side into a caller-owned shared string.
///
/// Returns null and writes `out_error` when the side is not set.
#[no_mangle]
pub unsafe extern "C" fn openpit_param_side_to_string(
    value: OpenPitParamSide,
    out_error: OpenPitOutParamError,
) -> *mut OpenPitSharedString {
    match import_side(value) {
        Some(side) => OpenPitSharedString::new_handle(side.to_string().as_str()),
        None => {
            write_param_error_unspecified(out_error, "side is not set");
            std::ptr::null_mut()
        }
    }
}

/// Renders a position side into a caller-owned shared string.
///
/// Returns null and writes `out_error` when the position side is not set.
#[no_mangle]
pub unsafe extern "C" fn openpit_param_position_side_to_string(
    value: OpenPitParamPositionSide,
    out_error: OpenPitOutParamError,
) -> *mut OpenPitSharedString {
    match import_position_side(value) {
        Some(position_side) => OpenPitSharedString::new_handle(position_side.to_string().as_str()),
        None => {
            write_param_error_unspecified(out_error, "position side is not set");
            std::ptr::null_mut()
        }
    }
}

/// Renders a position effect into a caller-owned shared string.
///
/// Returns null and writes `out_error` when the position effect is not set.
#[no_mangle]
pub unsafe extern "C" fn openpit_param_position_effect_to_string(
    value: OpenPitParamPositionEffect,
    out_error: OpenPitOutParamError,
) -> *mut OpenPitSharedString {
    match import_position_effect(value) {
        Some(position_effect) => {
            OpenPitSharedString::new_handle(position_effect.to_string().as_str())
        }
        None => {
            write_param_error_unspecified(out_error, "position effect is not set");
            std::ptr::null_mut()
        }
    }
}

/// Renders a position mode into a caller-owned shared string.
///
/// Returns null and writes `out_error` when the position mode is not set.
#[no_mangle]
pub unsafe extern "C" fn openpit_param_position_mode_to_string(
    value: OpenPitParamPositionMode,
    out_error: OpenPitOutParamError,
) -> *mut OpenPitSharedString {
    match import_position_mode(value) {
        Some(position_mode) => OpenPitSharedString::new_handle(position_mode.to_string().as_str()),
        None => {
            write_param_error_unspecified(out_error, "position mode is not set");
            std::ptr::null_mut()
        }
    }
}

/// Renders an account identifier into a caller-owned shared string.
///
/// This conversion always succeeds.
#[no_mangle]
pub extern "C" fn openpit_param_account_id_to_string(
    value: OpenPitParamAccountId,
) -> *mut OpenPitSharedString {
    OpenPitSharedString::new_handle(AccountId::from_u64(value).to_string().as_str())
}

/// Renders a trade amount into a caller-owned shared string.
///
/// Returns null and writes `out_error` when the trade amount is not set or its
/// numeric value cannot be decoded.
#[no_mangle]
pub unsafe extern "C" fn openpit_param_trade_amount_to_string(
    value: OpenPitParamTradeAmount,
    out_error: OpenPitOutParamError,
) -> *mut OpenPitSharedString {
    match import_trade_amount(value) {
        Ok(Some(trade_amount)) => {
            OpenPitSharedString::new_handle(trade_amount.to_string().as_str())
        }
        Ok(None) => {
            write_param_error_unspecified(out_error, "trade amount is not set");
            std::ptr::null_mut()
        }
        Err(error) => {
            write_param_error_unspecified(out_error, error.as_str());
            std::ptr::null_mut()
        }
    }
}

//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crate::string::{openpit_destroy_shared_string, openpit_shared_string_view};
    use crate::OpenPitStringView;

    use super::*;
    use openpit::param::{PositionEffect, PositionMode, PositionSide, RoundingStrategy, Side};

    fn view_to_string(view: OpenPitStringView) -> String {
        if view.ptr.is_null() {
            return String::new();
        }
        let bytes = unsafe { std::slice::from_raw_parts(view.ptr, view.len) };
        std::str::from_utf8(bytes)
            .expect("error string must be valid utf-8")
            .to_owned()
    }

    #[test]
    fn leverage_constants_match_openpit() {
        assert_eq!(OPENPIT_PARAM_LEVERAGE_SCALE, Leverage::SCALE);
        assert_eq!(OPENPIT_PARAM_LEVERAGE_MIN, Leverage::MIN);
        assert_eq!(OPENPIT_PARAM_LEVERAGE_MAX, Leverage::MAX);
        assert_eq!(OPENPIT_PARAM_LEVERAGE_STEP, Leverage::STEP);
    }

    #[test]
    fn rounding_strategy_aliases_match_openpit() {
        assert_eq!(
            OPENPIT_PARAM_ROUNDING_STRATEGY_DEFAULT as u8,
            RoundingStrategy::DEFAULT as u8
        );
        assert_eq!(
            OPENPIT_PARAM_ROUNDING_STRATEGY_BANKER as u8,
            RoundingStrategy::BANKER as u8
        );
        assert_eq!(
            OPENPIT_PARAM_ROUNDING_STRATEGY_CONSERVATIVE_PROFIT as u8,
            RoundingStrategy::CONSERVATIVE_PROFIT as u8
        );
        assert_eq!(
            OPENPIT_PARAM_ROUNDING_STRATEGY_CONSERVATIVE_LOSS as u8,
            RoundingStrategy::CONSERVATIVE_LOSS as u8
        );

        assert_eq!(
            import_rounding_strategy(OPENPIT_PARAM_ROUNDING_STRATEGY_DEFAULT),
            RoundingStrategy::DEFAULT
        );
        assert_eq!(
            import_rounding_strategy(OPENPIT_PARAM_ROUNDING_STRATEGY_BANKER),
            RoundingStrategy::BANKER
        );
        assert_eq!(
            import_rounding_strategy(OPENPIT_PARAM_ROUNDING_STRATEGY_CONSERVATIVE_PROFIT),
            RoundingStrategy::CONSERVATIVE_PROFIT
        );
        assert_eq!(
            import_rounding_strategy(OPENPIT_PARAM_ROUNDING_STRATEGY_CONSERVATIVE_LOSS),
            RoundingStrategy::CONSERVATIVE_LOSS
        );
    }

    #[test]
    fn typed_param_create_get_roundtrip_by_value() {
        let mut pnl = OpenPitParamPnl::default();
        let ok = unsafe {
            openpit_create_param_pnl(
                OpenPitParamDecimal {
                    mantissa_lo: 100,
                    mantissa_hi: 0,
                    scale: 0,
                },
                &mut pnl,
                std::ptr::null_mut(),
            )
        };
        assert!(ok);
        assert_eq!(
            openpit_param_pnl_get_decimal(pnl),
            OpenPitParamDecimal {
                mantissa_lo: 100,
                mantissa_hi: 0,
                scale: 0
            }
        );

        let mut fee = super::OpenPitParamFee::default();
        let ok = unsafe {
            openpit_create_param_fee(
                OpenPitParamDecimal {
                    mantissa_lo: -5,
                    mantissa_hi: -1,
                    scale: 0,
                },
                &mut fee,
                std::ptr::null_mut(),
            )
        };
        assert!(ok);
        assert_eq!(
            openpit_param_fee_get_decimal(fee),
            OpenPitParamDecimal {
                mantissa_lo: -5,
                mantissa_hi: -1,
                scale: 0
            }
        );
    }

    #[test]
    fn typed_param_create_invalid_returns_default() {
        let mut value = OpenPitParamPnl(OpenPitParamDecimal {
            mantissa_lo: 7,
            mantissa_hi: 0,
            scale: 0,
        });
        let ok = unsafe {
            openpit_create_param_pnl(
                OpenPitParamDecimal {
                    mantissa_lo: 1,
                    mantissa_hi: 0,
                    scale: -1,
                },
                &mut value,
                std::ptr::null_mut(),
            )
        };
        assert!(!ok);
        assert_eq!(
            value,
            OpenPitParamPnl(OpenPitParamDecimal {
                mantissa_lo: 7,
                mantissa_hi: 0,
                scale: 0
            })
        );
        let mut out_error = std::ptr::null_mut();
        let ok = unsafe {
            openpit_create_param_pnl(
                OpenPitParamDecimal {
                    mantissa_lo: 1,
                    mantissa_hi: 0,
                    scale: -1,
                },
                &mut value,
                &mut out_error,
            )
        };
        assert!(!ok);
        assert!(!out_error.is_null());
        unsafe { crate::last_error::openpit_destroy_param_error(out_error) };
    }

    #[test]
    fn account_id_from_string_does_not_collapse_invalid_utf8_to_empty() {
        let bytes = [0xF0_u8, 0x28, 0x8C, 0x28];
        let mut id_invalid = 0_u64;
        let id_invalid_ok = unsafe {
            openpit_create_param_account_id_from_string(
                OpenPitStringView {
                    ptr: bytes.as_ptr(),
                    len: bytes.len(),
                },
                &mut id_invalid,
                std::ptr::null_mut(),
            )
        };
        assert!(id_invalid_ok);

        let mut out_error = std::ptr::null_mut();
        let mut id_empty = 0_u64;
        let id_empty_ok = unsafe {
            openpit_create_param_account_id_from_string(
                OpenPitStringView::not_set(),
                &mut id_empty,
                &mut out_error,
            )
        };
        assert!(!id_empty_ok);
        assert!(!out_error.is_null());
        assert_eq!(
            unsafe { (*out_error).code },
            crate::last_error::OpenPitParamErrorCode::AccountIdEmpty
        );
        let error_message = openpit_shared_string_view(unsafe { (*out_error).message });
        assert_eq!(
            view_to_string(error_message),
            "account id string must not be empty"
        );
        unsafe { crate::last_error::openpit_destroy_param_error(out_error) };
    }

    #[test]
    fn account_id_from_string_rejects_whitespace() {
        let mut out = 0_u64;
        let mut out_error = std::ptr::null_mut();
        let ok = unsafe {
            openpit_create_param_account_id_from_string(
                OpenPitStringView::from_utf8("   "),
                &mut out,
                &mut out_error,
            )
        };
        assert!(!ok);
        assert!(!out_error.is_null());
        assert_eq!(
            unsafe { (*out_error).code },
            crate::last_error::OpenPitParamErrorCode::AccountIdEmpty
        );
        let error_message = openpit_shared_string_view(unsafe { (*out_error).message });
        assert_eq!(
            view_to_string(error_message),
            "account id string must not be empty"
        );
        unsafe { crate::last_error::openpit_destroy_param_error(out_error) };
    }

    #[test]
    fn account_id_from_uint64_is_stable_passthrough() {
        assert_eq!(openpit_create_param_account_id_from_uint64(0), 0);
        assert_eq!(openpit_create_param_account_id_from_uint64(7), 7);
        assert_eq!(
            openpit_create_param_account_id_from_uint64(u64::MAX),
            u64::MAX
        );
    }

    #[test]
    fn asset_from_string_returns_owned_handle_when_valid() {
        let handle = unsafe {
            openpit_create_param_asset_from_string(
                OpenPitStringView::from_utf8("USD"),
                std::ptr::null_mut(),
            )
        };
        assert!(!handle.is_null());
        let value = view_to_string(openpit_shared_string_view(handle));
        assert_eq!(value, "USD");
        openpit_destroy_param_asset(handle);
    }

    #[test]
    fn asset_from_string_rejects_empty_and_whitespace() {
        let mut out_error = std::ptr::null_mut();
        let empty = unsafe {
            openpit_create_param_asset_from_string(OpenPitStringView::not_set(), &mut out_error)
        };
        assert!(empty.is_null());
        assert!(!out_error.is_null());
        assert_eq!(
            unsafe { (*out_error).code },
            crate::last_error::OpenPitParamErrorCode::AssetEmpty
        );
        let message = view_to_string(openpit_shared_string_view(unsafe { (*out_error).message }));
        assert_eq!(message, "asset must not be empty");
        unsafe { crate::last_error::openpit_destroy_param_error(out_error) };

        let mut out_error = std::ptr::null_mut();
        let whitespace = unsafe {
            openpit_create_param_asset_from_string(
                OpenPitStringView::from_utf8("   "),
                &mut out_error,
            )
        };
        assert!(whitespace.is_null());
        assert!(!out_error.is_null());
        assert_eq!(
            unsafe { (*out_error).code },
            crate::last_error::OpenPitParamErrorCode::AssetEmpty
        );
        let message = view_to_string(openpit_shared_string_view(unsafe { (*out_error).message }));
        assert_eq!(message, "asset must not be empty");
        unsafe { crate::last_error::openpit_destroy_param_error(out_error) };
    }

    #[test]
    fn import_monetary_amount_round_trips_when_fully_set() {
        let amount = OpenPitParamFee(OpenPitParamDecimal {
            mantissa_lo: 100,
            mantissa_hi: 0,
            scale: 0,
        });
        let value = OpenPitParamMonetaryAmount {
            amount,
            currency: OpenPitStringView::from_utf8("USD"),
        };
        let imported = import_monetary_amount(value).expect("fully-set amount must import");
        assert_eq!(imported.currency.as_ref(), "USD");
        assert_eq!(imported.amount, amount.to_param().expect("valid fee"));
    }

    #[test]
    fn import_monetary_amount_rejects_missing_currency() {
        // The amount is a valid fee, but the currency view is unset: a partially
        // set C struct must be rejected rather than importing a defaulted asset.
        let partial = OpenPitParamMonetaryAmount {
            amount: OpenPitParamFee(OpenPitParamDecimal {
                mantissa_lo: 100,
                mantissa_hi: 0,
                scale: 0,
            }),
            currency: OpenPitStringView::not_set(),
        };
        let err = import_monetary_amount(partial).expect_err("missing currency must reject");
        assert_eq!(err, "monetary_amount.currency is not set");
    }

    #[test]
    fn enum_import_export_cover_all_public_branches() {
        assert_eq!(import_side(OpenPitParamSide::Buy), Some(Side::Buy));
        assert_eq!(import_side(OpenPitParamSide::Sell), Some(Side::Sell));
        assert_eq!(import_side(OpenPitParamSide::NotSet), None);
        assert_eq!(export_side(Side::Buy), OpenPitParamSide::Buy);
        assert_eq!(export_side(Side::Sell), OpenPitParamSide::Sell);

        assert_eq!(
            import_position_side(OpenPitParamPositionSide::Long),
            Some(PositionSide::Long)
        );
        assert_eq!(
            import_position_side(OpenPitParamPositionSide::Short),
            Some(PositionSide::Short)
        );
        assert_eq!(import_position_side(OpenPitParamPositionSide::NotSet), None);
        assert_eq!(
            export_position_side(PositionSide::Long),
            OpenPitParamPositionSide::Long
        );
        assert_eq!(
            export_position_side(PositionSide::Short),
            OpenPitParamPositionSide::Short
        );

        assert_eq!(
            import_position_effect(OpenPitParamPositionEffect::Open),
            Some(PositionEffect::Open)
        );
        assert_eq!(
            import_position_effect(OpenPitParamPositionEffect::Close),
            Some(PositionEffect::Close)
        );
        assert_eq!(
            import_position_effect(OpenPitParamPositionEffect::NotSet),
            None
        );
        assert_eq!(
            export_position_effect(PositionEffect::Open),
            OpenPitParamPositionEffect::Open
        );
        assert_eq!(
            export_position_effect(PositionEffect::Close),
            OpenPitParamPositionEffect::Close
        );

        assert_eq!(
            import_position_mode(OpenPitParamPositionMode::Netting),
            Some(PositionMode::Netting)
        );
        assert_eq!(
            import_position_mode(OpenPitParamPositionMode::Hedged),
            Some(PositionMode::Hedged)
        );
        assert_eq!(import_position_mode(OpenPitParamPositionMode::NotSet), None);
        assert_eq!(
            export_position_mode(PositionMode::Netting),
            OpenPitParamPositionMode::Netting
        );
        assert_eq!(
            export_position_mode(PositionMode::Hedged),
            OpenPitParamPositionMode::Hedged
        );

        assert_eq!(import_bool(OpenPitTriBool::NotSet), None);
        assert_eq!(import_bool(OpenPitTriBool::False), Some(false));
        assert_eq!(import_bool(OpenPitTriBool::True), Some(true));
        assert_eq!(export_bool(false), OpenPitTriBool::False);
        assert_eq!(export_bool(true), OpenPitTriBool::True);
    }

    #[test]
    fn decimal_wrapper_create_get_roundtrip_for_all_public_ffi_types() {
        let raw = OpenPitParamDecimal {
            mantissa_lo: 123,
            mantissa_hi: 0,
            scale: 0,
        };

        let mut price = OpenPitParamPrice::default();
        assert!(unsafe { openpit_create_param_price(raw, &mut price, std::ptr::null_mut()) });
        assert_eq!(openpit_param_price_get_decimal(price), raw);

        let mut qty = OpenPitParamQuantity::default();
        assert!(unsafe { openpit_create_param_quantity(raw, &mut qty, std::ptr::null_mut()) });
        assert_eq!(openpit_param_quantity_get_decimal(qty), raw);

        let mut volume = OpenPitParamVolume::default();
        assert!(unsafe { openpit_create_param_volume(raw, &mut volume, std::ptr::null_mut()) });
        assert_eq!(openpit_param_volume_get_decimal(volume), raw);

        let mut cash_flow = OpenPitParamCashFlow::default();
        assert!(unsafe {
            openpit_create_param_cash_flow(raw, &mut cash_flow, std::ptr::null_mut())
        });
        assert_eq!(openpit_param_cash_flow_get_decimal(cash_flow), raw);

        let mut pos_size = OpenPitParamPositionSize::default();
        assert!(unsafe {
            openpit_create_param_position_size(raw, &mut pos_size, std::ptr::null_mut())
        });
        assert_eq!(openpit_param_position_size_get_decimal(pos_size), raw);

        let mut fee = OpenPitParamFee::default();
        assert!(unsafe { openpit_create_param_fee(raw, &mut fee, std::ptr::null_mut()) });
        assert_eq!(openpit_param_fee_get_decimal(fee), raw);
    }

    #[test]
    fn decimal_wrapper_get_decimal_handles_invalid_wrappers_without_panic() {
        let invalid = OpenPitParamDecimal {
            mantissa_lo: 1,
            mantissa_hi: 0,
            scale: -1,
        };
        assert_eq!(
            openpit_param_price_get_decimal(OpenPitParamPrice(invalid)),
            invalid
        );
        assert_eq!(
            openpit_param_quantity_get_decimal(OpenPitParamQuantity(invalid)),
            invalid
        );
        assert_eq!(
            openpit_param_volume_get_decimal(OpenPitParamVolume(invalid)),
            invalid
        );
        assert_eq!(
            openpit_param_cash_flow_get_decimal(OpenPitParamCashFlow(invalid)),
            invalid
        );
        assert_eq!(
            openpit_param_position_size_get_decimal(OpenPitParamPositionSize(invalid)),
            invalid
        );
        assert_eq!(
            openpit_param_fee_get_decimal(OpenPitParamFee(invalid)),
            invalid
        );
    }

    fn sv(value: &str) -> OpenPitStringView {
        OpenPitStringView::from_utf8(value)
    }

    fn string_view_to_string(view: OpenPitStringView) -> String {
        let bytes = unsafe { std::slice::from_raw_parts(view.ptr, view.len) };
        std::str::from_utf8(bytes)
            .expect("must be valid utf-8")
            .to_owned()
    }

    fn shared_string_to_string(handle: *mut OpenPitSharedString) -> String {
        let text = string_view_to_string(openpit_shared_string_view(handle));
        openpit_destroy_shared_string(handle);
        text
    }

    macro_rules! exercise_signed_surface {
        (
            wrapper = $wrapper:ident,
            from_string = $from_str:ident,
            from_f64 = $from_f64:ident,
            from_int64 = $from_i64:ident,
            from_uint64 = $from_u64:ident,
            from_string_rounded = $from_str_rounded:ident,
            from_f64_rounded = $from_f64_rounded:ident,
            from_decimal_rounded = $from_decimal_rounded:ident,
            to_f64 = $to_f64:ident,
            is_zero = $is_zero:ident,
            compare = $compare:ident,
            to_string = $to_string:ident,
            checked_add = $checked_add:ident,
            checked_sub = $checked_sub:ident,
            checked_neg = $checked_neg:ident,
            checked_mul_i64 = $checked_mul_i64:ident,
            checked_mul_u64 = $checked_mul_u64:ident,
            checked_mul_f64 = $checked_mul_f64:ident,
            checked_div_i64 = $checked_div_i64:ident,
            checked_div_u64 = $checked_div_u64:ident,
            checked_div_f64 = $checked_div_f64:ident,
            checked_rem_i64 = $checked_rem_i64:ident,
            checked_rem_u64 = $checked_rem_u64:ident,
            checked_rem_f64 = $checked_rem_f64:ident
        ) => {
            let mut a = $wrapper::default();
            assert!(unsafe { $from_str(sv("3"), &mut a, std::ptr::null_mut()) });
            let mut b = $wrapper::default();
            assert!(unsafe { $from_f64(2.5, &mut b, std::ptr::null_mut()) });
            let mut c = $wrapper::default();
            assert!(unsafe { $from_i64(-4, &mut c, std::ptr::null_mut()) });
            assert!(unsafe { $from_u64(4, &mut c, std::ptr::null_mut()) });
            assert!(unsafe {
                $from_str_rounded(
                    sv("1.255"),
                    2,
                    OpenPitParamRoundingStrategy::MidpointNearestEven,
                    &mut c,
                    std::ptr::null_mut(),
                )
            });
            assert!(unsafe {
                $from_f64_rounded(
                    1.255,
                    2,
                    OpenPitParamRoundingStrategy::MidpointNearestEven,
                    &mut c,
                    std::ptr::null_mut(),
                )
            });
            assert!(unsafe {
                $from_decimal_rounded(
                    OpenPitParamDecimal {
                        mantissa_lo: 1255,
                        mantissa_hi: 0,
                        scale: 3,
                    },
                    2,
                    OpenPitParamRoundingStrategy::MidpointNearestEven,
                    &mut c,
                    std::ptr::null_mut(),
                )
            });

            let mut out_f64 = 0.0_f64;
            assert!(unsafe { $to_f64(a, &mut out_f64, std::ptr::null_mut()) });
            let mut out_is_zero = false;
            assert!(unsafe {
                $is_zero($wrapper::default(), &mut out_is_zero, std::ptr::null_mut())
            });
            let mut out_compare = 0_i8;
            assert!(unsafe { $compare(a, b, &mut out_compare, std::ptr::null_mut()) });

            let out_text = unsafe { $to_string(a, std::ptr::null_mut()) };
            assert!(!out_text.is_null());
            assert!(!shared_string_to_string(out_text).is_empty());

            let mut out = $wrapper::default();
            assert!(unsafe { $checked_add(a, b, &mut out, std::ptr::null_mut()) });
            assert!(unsafe { $checked_sub(a, b, &mut out, std::ptr::null_mut()) });
            assert!(unsafe { $checked_neg(a, &mut out, std::ptr::null_mut()) });
            assert!(unsafe { $checked_mul_i64(a, 2, &mut out, std::ptr::null_mut()) });
            assert!(unsafe { $checked_mul_u64(a, 2, &mut out, std::ptr::null_mut()) });
            assert!(unsafe { $checked_mul_f64(a, 2.0, &mut out, std::ptr::null_mut()) });
            assert!(unsafe { $checked_div_i64(a, 2, &mut out, std::ptr::null_mut()) });
            assert!(unsafe { $checked_div_u64(a, 2, &mut out, std::ptr::null_mut()) });
            assert!(unsafe { $checked_div_f64(a, 2.0, &mut out, std::ptr::null_mut()) });
            assert!(unsafe { $checked_rem_i64(a, 2, &mut out, std::ptr::null_mut()) });
            assert!(unsafe { $checked_rem_u64(a, 2, &mut out, std::ptr::null_mut()) });
            assert!(unsafe { $checked_rem_f64(a, 2.0, &mut out, std::ptr::null_mut()) });
        };
    }

    macro_rules! exercise_unsigned_surface {
        (
            wrapper = $wrapper:ident,
            from_string = $from_str:ident,
            from_f64 = $from_f64:ident,
            from_int64 = $from_i64:ident,
            from_uint64 = $from_u64:ident,
            from_string_rounded = $from_str_rounded:ident,
            from_f64_rounded = $from_f64_rounded:ident,
            from_decimal_rounded = $from_decimal_rounded:ident,
            to_f64 = $to_f64:ident,
            is_zero = $is_zero:ident,
            compare = $compare:ident,
            to_string = $to_string:ident,
            checked_add = $checked_add:ident,
            checked_sub = $checked_sub:ident,
            checked_mul_i64 = $checked_mul_i64:ident,
            checked_mul_u64 = $checked_mul_u64:ident,
            checked_mul_f64 = $checked_mul_f64:ident,
            checked_div_i64 = $checked_div_i64:ident,
            checked_div_u64 = $checked_div_u64:ident,
            checked_div_f64 = $checked_div_f64:ident,
            checked_rem_i64 = $checked_rem_i64:ident,
            checked_rem_u64 = $checked_rem_u64:ident,
            checked_rem_f64 = $checked_rem_f64:ident
        ) => {
            let mut a = $wrapper::default();
            assert!(unsafe { $from_str(sv("3"), &mut a, std::ptr::null_mut()) });
            let mut b = $wrapper::default();
            assert!(unsafe { $from_f64(2.5, &mut b, std::ptr::null_mut()) });
            let mut c = $wrapper::default();
            assert!(unsafe { $from_i64(4, &mut c, std::ptr::null_mut()) });
            assert!(unsafe { $from_u64(4, &mut c, std::ptr::null_mut()) });
            assert!(unsafe {
                $from_str_rounded(
                    sv("1.255"),
                    2,
                    OpenPitParamRoundingStrategy::MidpointNearestEven,
                    &mut c,
                    std::ptr::null_mut(),
                )
            });
            assert!(unsafe {
                $from_f64_rounded(
                    1.255,
                    2,
                    OpenPitParamRoundingStrategy::MidpointNearestEven,
                    &mut c,
                    std::ptr::null_mut(),
                )
            });
            assert!(unsafe {
                $from_decimal_rounded(
                    OpenPitParamDecimal {
                        mantissa_lo: 1255,
                        mantissa_hi: 0,
                        scale: 3,
                    },
                    2,
                    OpenPitParamRoundingStrategy::MidpointNearestEven,
                    &mut c,
                    std::ptr::null_mut(),
                )
            });

            let mut out_f64 = 0.0_f64;
            assert!(unsafe { $to_f64(a, &mut out_f64, std::ptr::null_mut()) });
            let mut out_is_zero = false;
            assert!(unsafe {
                $is_zero($wrapper::default(), &mut out_is_zero, std::ptr::null_mut())
            });
            let mut out_compare = 0_i8;
            assert!(unsafe { $compare(a, b, &mut out_compare, std::ptr::null_mut()) });

            let out_text = unsafe { $to_string(a, std::ptr::null_mut()) };
            assert!(!out_text.is_null());
            assert!(!shared_string_to_string(out_text).is_empty());

            let mut out = $wrapper::default();
            assert!(unsafe { $checked_add(a, b, &mut out, std::ptr::null_mut()) });
            assert!(unsafe { $checked_sub(a, b, &mut out, std::ptr::null_mut()) });
            assert!(unsafe { $checked_mul_i64(a, 2, &mut out, std::ptr::null_mut()) });
            assert!(unsafe { $checked_mul_u64(a, 2, &mut out, std::ptr::null_mut()) });
            assert!(unsafe { $checked_mul_f64(a, 2.0, &mut out, std::ptr::null_mut()) });
            assert!(unsafe { $checked_div_i64(a, 2, &mut out, std::ptr::null_mut()) });
            assert!(unsafe { $checked_div_u64(a, 2, &mut out, std::ptr::null_mut()) });
            assert!(unsafe { $checked_div_f64(a, 2.0, &mut out, std::ptr::null_mut()) });
            assert!(unsafe { $checked_rem_i64(a, 2, &mut out, std::ptr::null_mut()) });
            assert!(unsafe { $checked_rem_u64(a, 2, &mut out, std::ptr::null_mut()) });
            assert!(unsafe { $checked_rem_f64(a, 2.0, &mut out, std::ptr::null_mut()) });
        };
    }

    #[test]
    fn typed_param_ffi_signed_surface_happy_path() {
        exercise_signed_surface!(
            wrapper = OpenPitParamPnl,
            from_string = openpit_create_param_pnl_from_string,
            from_f64 = openpit_create_param_pnl_from_f64,
            from_int64 = openpit_create_param_pnl_from_int64,
            from_uint64 = openpit_create_param_pnl_from_uint64,
            from_string_rounded = openpit_create_param_pnl_from_string_rounded,
            from_f64_rounded = openpit_create_param_pnl_from_f64_rounded,
            from_decimal_rounded = openpit_create_param_pnl_from_decimal_rounded,
            to_f64 = openpit_param_pnl_to_f64,
            is_zero = openpit_param_pnl_is_zero,
            compare = openpit_param_pnl_compare,
            to_string = openpit_param_pnl_to_string,
            checked_add = openpit_param_pnl_checked_add,
            checked_sub = openpit_param_pnl_checked_sub,
            checked_neg = openpit_param_pnl_checked_neg,
            checked_mul_i64 = openpit_param_pnl_checked_mul_i64,
            checked_mul_u64 = openpit_param_pnl_checked_mul_u64,
            checked_mul_f64 = openpit_param_pnl_checked_mul_f64,
            checked_div_i64 = openpit_param_pnl_checked_div_i64,
            checked_div_u64 = openpit_param_pnl_checked_div_u64,
            checked_div_f64 = openpit_param_pnl_checked_div_f64,
            checked_rem_i64 = openpit_param_pnl_checked_rem_i64,
            checked_rem_u64 = openpit_param_pnl_checked_rem_u64,
            checked_rem_f64 = openpit_param_pnl_checked_rem_f64
        );
        exercise_signed_surface!(
            wrapper = OpenPitParamPrice,
            from_string = openpit_create_param_price_from_string,
            from_f64 = openpit_create_param_price_from_f64,
            from_int64 = openpit_create_param_price_from_int64,
            from_uint64 = openpit_create_param_price_from_uint64,
            from_string_rounded = openpit_create_param_price_from_string_rounded,
            from_f64_rounded = openpit_create_param_price_from_f64_rounded,
            from_decimal_rounded = openpit_create_param_price_from_decimal_rounded,
            to_f64 = openpit_param_price_to_f64,
            is_zero = openpit_param_price_is_zero,
            compare = openpit_param_price_compare,
            to_string = openpit_param_price_to_string,
            checked_add = openpit_param_price_checked_add,
            checked_sub = openpit_param_price_checked_sub,
            checked_neg = openpit_param_price_checked_neg,
            checked_mul_i64 = openpit_param_price_checked_mul_i64,
            checked_mul_u64 = openpit_param_price_checked_mul_u64,
            checked_mul_f64 = openpit_param_price_checked_mul_f64,
            checked_div_i64 = openpit_param_price_checked_div_i64,
            checked_div_u64 = openpit_param_price_checked_div_u64,
            checked_div_f64 = openpit_param_price_checked_div_f64,
            checked_rem_i64 = openpit_param_price_checked_rem_i64,
            checked_rem_u64 = openpit_param_price_checked_rem_u64,
            checked_rem_f64 = openpit_param_price_checked_rem_f64
        );
        exercise_signed_surface!(
            wrapper = OpenPitParamCashFlow,
            from_string = openpit_create_param_cash_flow_from_string,
            from_f64 = openpit_create_param_cash_flow_from_f64,
            from_int64 = openpit_create_param_cash_flow_from_int64,
            from_uint64 = openpit_create_param_cash_flow_from_uint64,
            from_string_rounded = openpit_create_param_cash_flow_from_string_rounded,
            from_f64_rounded = openpit_create_param_cash_flow_from_f64_rounded,
            from_decimal_rounded = openpit_create_param_cash_flow_from_decimal_rounded,
            to_f64 = openpit_param_cash_flow_to_f64,
            is_zero = openpit_param_cash_flow_is_zero,
            compare = openpit_param_cash_flow_compare,
            to_string = openpit_param_cash_flow_to_string,
            checked_add = openpit_param_cash_flow_checked_add,
            checked_sub = openpit_param_cash_flow_checked_sub,
            checked_neg = openpit_param_cash_flow_checked_neg,
            checked_mul_i64 = openpit_param_cash_flow_checked_mul_i64,
            checked_mul_u64 = openpit_param_cash_flow_checked_mul_u64,
            checked_mul_f64 = openpit_param_cash_flow_checked_mul_f64,
            checked_div_i64 = openpit_param_cash_flow_checked_div_i64,
            checked_div_u64 = openpit_param_cash_flow_checked_div_u64,
            checked_div_f64 = openpit_param_cash_flow_checked_div_f64,
            checked_rem_i64 = openpit_param_cash_flow_checked_rem_i64,
            checked_rem_u64 = openpit_param_cash_flow_checked_rem_u64,
            checked_rem_f64 = openpit_param_cash_flow_checked_rem_f64
        );
        exercise_signed_surface!(
            wrapper = OpenPitParamPositionSize,
            from_string = openpit_create_param_position_size_from_string,
            from_f64 = openpit_create_param_position_size_from_f64,
            from_int64 = openpit_create_param_position_size_from_int64,
            from_uint64 = openpit_create_param_position_size_from_uint64,
            from_string_rounded = openpit_create_param_position_size_from_string_rounded,
            from_f64_rounded = openpit_create_param_position_size_from_f64_rounded,
            from_decimal_rounded = openpit_create_param_position_size_from_decimal_rounded,
            to_f64 = openpit_param_position_size_to_f64,
            is_zero = openpit_param_position_size_is_zero,
            compare = openpit_param_position_size_compare,
            to_string = openpit_param_position_size_to_string,
            checked_add = openpit_param_position_size_checked_add,
            checked_sub = openpit_param_position_size_checked_sub,
            checked_neg = openpit_param_position_size_checked_neg,
            checked_mul_i64 = openpit_param_position_size_checked_mul_i64,
            checked_mul_u64 = openpit_param_position_size_checked_mul_u64,
            checked_mul_f64 = openpit_param_position_size_checked_mul_f64,
            checked_div_i64 = openpit_param_position_size_checked_div_i64,
            checked_div_u64 = openpit_param_position_size_checked_div_u64,
            checked_div_f64 = openpit_param_position_size_checked_div_f64,
            checked_rem_i64 = openpit_param_position_size_checked_rem_i64,
            checked_rem_u64 = openpit_param_position_size_checked_rem_u64,
            checked_rem_f64 = openpit_param_position_size_checked_rem_f64
        );
        exercise_signed_surface!(
            wrapper = OpenPitParamFee,
            from_string = openpit_create_param_fee_from_string,
            from_f64 = openpit_create_param_fee_from_f64,
            from_int64 = openpit_create_param_fee_from_int64,
            from_uint64 = openpit_create_param_fee_from_uint64,
            from_string_rounded = openpit_create_param_fee_from_string_rounded,
            from_f64_rounded = openpit_create_param_fee_from_f64_rounded,
            from_decimal_rounded = openpit_create_param_fee_from_decimal_rounded,
            to_f64 = openpit_param_fee_to_f64,
            is_zero = openpit_param_fee_is_zero,
            compare = openpit_param_fee_compare,
            to_string = openpit_param_fee_to_string,
            checked_add = openpit_param_fee_checked_add,
            checked_sub = openpit_param_fee_checked_sub,
            checked_neg = openpit_param_fee_checked_neg,
            checked_mul_i64 = openpit_param_fee_checked_mul_i64,
            checked_mul_u64 = openpit_param_fee_checked_mul_u64,
            checked_mul_f64 = openpit_param_fee_checked_mul_f64,
            checked_div_i64 = openpit_param_fee_checked_div_i64,
            checked_div_u64 = openpit_param_fee_checked_div_u64,
            checked_div_f64 = openpit_param_fee_checked_div_f64,
            checked_rem_i64 = openpit_param_fee_checked_rem_i64,
            checked_rem_u64 = openpit_param_fee_checked_rem_u64,
            checked_rem_f64 = openpit_param_fee_checked_rem_f64
        );
    }

    #[test]
    fn typed_param_ffi_unsigned_surface_happy_path() {
        exercise_unsigned_surface!(
            wrapper = OpenPitParamQuantity,
            from_string = openpit_create_param_quantity_from_string,
            from_f64 = openpit_create_param_quantity_from_f64,
            from_int64 = openpit_create_param_quantity_from_int64,
            from_uint64 = openpit_create_param_quantity_from_uint64,
            from_string_rounded = openpit_create_param_quantity_from_string_rounded,
            from_f64_rounded = openpit_create_param_quantity_from_f64_rounded,
            from_decimal_rounded = openpit_create_param_quantity_from_decimal_rounded,
            to_f64 = openpit_param_quantity_to_f64,
            is_zero = openpit_param_quantity_is_zero,
            compare = openpit_param_quantity_compare,
            to_string = openpit_param_quantity_to_string,
            checked_add = openpit_param_quantity_checked_add,
            checked_sub = openpit_param_quantity_checked_sub,
            checked_mul_i64 = openpit_param_quantity_checked_mul_i64,
            checked_mul_u64 = openpit_param_quantity_checked_mul_u64,
            checked_mul_f64 = openpit_param_quantity_checked_mul_f64,
            checked_div_i64 = openpit_param_quantity_checked_div_i64,
            checked_div_u64 = openpit_param_quantity_checked_div_u64,
            checked_div_f64 = openpit_param_quantity_checked_div_f64,
            checked_rem_i64 = openpit_param_quantity_checked_rem_i64,
            checked_rem_u64 = openpit_param_quantity_checked_rem_u64,
            checked_rem_f64 = openpit_param_quantity_checked_rem_f64
        );
        exercise_unsigned_surface!(
            wrapper = OpenPitParamVolume,
            from_string = openpit_create_param_volume_from_string,
            from_f64 = openpit_create_param_volume_from_f64,
            from_int64 = openpit_create_param_volume_from_int64,
            from_uint64 = openpit_create_param_volume_from_uint64,
            from_string_rounded = openpit_create_param_volume_from_string_rounded,
            from_f64_rounded = openpit_create_param_volume_from_f64_rounded,
            from_decimal_rounded = openpit_create_param_volume_from_decimal_rounded,
            to_f64 = openpit_param_volume_to_f64,
            is_zero = openpit_param_volume_is_zero,
            compare = openpit_param_volume_compare,
            to_string = openpit_param_volume_to_string,
            checked_add = openpit_param_volume_checked_add,
            checked_sub = openpit_param_volume_checked_sub,
            checked_mul_i64 = openpit_param_volume_checked_mul_i64,
            checked_mul_u64 = openpit_param_volume_checked_mul_u64,
            checked_mul_f64 = openpit_param_volume_checked_mul_f64,
            checked_div_i64 = openpit_param_volume_checked_div_i64,
            checked_div_u64 = openpit_param_volume_checked_div_u64,
            checked_div_f64 = openpit_param_volume_checked_div_f64,
            checked_rem_i64 = openpit_param_volume_checked_rem_i64,
            checked_rem_u64 = openpit_param_volume_checked_rem_u64,
            checked_rem_f64 = openpit_param_volume_checked_rem_f64
        );
    }

    #[test]
    fn typed_param_cross_type_surface_happy_path() {
        let mut price = OpenPitParamPrice::default();
        let mut quantity = OpenPitParamQuantity::default();
        let mut volume = OpenPitParamVolume::default();
        let mut pnl = OpenPitParamPnl::default();
        let mut fee = OpenPitParamFee::default();
        let mut position = OpenPitParamPositionSize::default();

        assert!(unsafe {
            openpit_create_param_price_from_string(sv("10"), &mut price, std::ptr::null_mut())
        });
        assert!(unsafe {
            openpit_create_param_quantity_from_string(sv("2"), &mut quantity, std::ptr::null_mut())
        });
        assert!(unsafe {
            openpit_create_param_volume_from_string(sv("20"), &mut volume, std::ptr::null_mut())
        });
        assert!(unsafe {
            openpit_create_param_pnl_from_string(sv("5"), &mut pnl, std::ptr::null_mut())
        });
        assert!(unsafe {
            openpit_create_param_fee_from_string(sv("1"), &mut fee, std::ptr::null_mut())
        });
        assert!(unsafe {
            openpit_create_param_position_size_from_string(
                sv("3"),
                &mut position,
                std::ptr::null_mut(),
            )
        });

        let mut calculated_volume = OpenPitParamVolume::default();
        assert!(unsafe {
            openpit_param_price_calculate_volume(
                price,
                quantity,
                &mut calculated_volume,
                std::ptr::null_mut(),
            )
        });
        assert!(unsafe {
            openpit_param_quantity_calculate_volume(
                quantity,
                price,
                &mut calculated_volume,
                std::ptr::null_mut(),
            )
        });

        let mut calculated_quantity = OpenPitParamQuantity::default();
        assert!(unsafe {
            openpit_param_volume_calculate_quantity(
                volume,
                price,
                &mut calculated_quantity,
                std::ptr::null_mut(),
            )
        });

        let mut out_cash_flow = OpenPitParamCashFlow::default();
        let mut out_position_size = OpenPitParamPositionSize::default();
        let mut out_pnl = OpenPitParamPnl::default();
        assert!(unsafe {
            openpit_param_pnl_to_cash_flow(pnl, &mut out_cash_flow, std::ptr::null_mut())
        });
        assert!(unsafe {
            openpit_param_pnl_to_position_size(pnl, &mut out_position_size, std::ptr::null_mut())
        });
        assert!(unsafe { openpit_param_pnl_from_fee(fee, &mut out_pnl, std::ptr::null_mut()) });
        assert!(unsafe {
            openpit_param_cash_flow_from_pnl(pnl, &mut out_cash_flow, std::ptr::null_mut())
        });
        assert!(unsafe {
            openpit_param_cash_flow_from_fee(fee, &mut out_cash_flow, std::ptr::null_mut())
        });
        assert!(unsafe {
            openpit_param_cash_flow_from_volume_inflow(
                volume,
                &mut out_cash_flow,
                std::ptr::null_mut(),
            )
        });
        assert!(unsafe {
            openpit_param_cash_flow_from_volume_outflow(
                volume,
                &mut out_cash_flow,
                std::ptr::null_mut(),
            )
        });
        assert!(unsafe { openpit_param_fee_to_pnl(fee, &mut out_pnl, std::ptr::null_mut()) });
        assert!(unsafe {
            openpit_param_fee_to_position_size(fee, &mut out_position_size, std::ptr::null_mut())
        });
        assert!(unsafe {
            openpit_param_fee_to_cash_flow(fee, &mut out_cash_flow, std::ptr::null_mut())
        });
        assert!(unsafe {
            openpit_param_volume_to_cash_flow_inflow(
                volume,
                &mut out_cash_flow,
                std::ptr::null_mut(),
            )
        });
        assert!(unsafe {
            openpit_param_volume_to_cash_flow_outflow(
                volume,
                &mut out_cash_flow,
                std::ptr::null_mut(),
            )
        });
        assert!(unsafe {
            openpit_param_position_size_from_pnl(pnl, &mut out_position_size, std::ptr::null_mut())
        });
        assert!(unsafe {
            openpit_param_position_size_from_fee(fee, &mut out_position_size, std::ptr::null_mut())
        });
        assert!(unsafe {
            openpit_param_position_size_from_quantity_and_side(
                quantity,
                OpenPitParamSide::Buy,
                &mut out_position_size,
                std::ptr::null_mut(),
            )
        });

        let mut out_quantity = OpenPitParamQuantity::default();
        let mut out_side = OpenPitParamSide::NotSet;
        assert!(unsafe {
            openpit_param_position_size_to_open_quantity(
                position,
                &mut out_quantity,
                &mut out_side,
                std::ptr::null_mut(),
            )
        });
        assert!(unsafe {
            openpit_param_position_size_to_close_quantity(
                position,
                &mut out_quantity,
                &mut out_side,
                std::ptr::null_mut(),
            )
        });

        let mut out_position = OpenPitParamPositionSize::default();
        assert!(unsafe {
            openpit_param_position_size_checked_add_quantity(
                position,
                quantity,
                OpenPitParamSide::Buy,
                &mut out_position,
                std::ptr::null_mut(),
            )
        });
    }
}
