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

//! Parameter value types for trading operations.
//!
//! This module provides domain-specific, type-safe financial values.
//! Decimal-based numeric types use exact decimal arithmetic and never use
//! floating-point arithmetic internally.
//! [`Leverage`] is represented as fixed-point integer with scale `10`.
//!
//! Prefer exact constructors such as `from_str` or `from_decimal_rounded` in
//! domain code. `from_f64` and related helpers exist for integration
//! boundaries that already expose floating-point inputs.
//!
//! # Type categories
//!
//! - **Unsigned types** ([`Quantity`], [`Volume`], [`Notional`]) — cannot be negative.
//! - **Signed types** ([`Price`], [`Pnl`], [`CashFlow`], [`PositionSize`], [`Fee`]) — can be negative.
//! - **Fixed-point types** ([`Leverage`]) — scaled integer domain values.
//! - **Identifiers** ([`Asset`], [`AccountId`], [`Side`], [`PositionSide`]) — non-numeric types.
//!
//! # Rounding
//!
//! Value types support rounding during construction via [`RoundingStrategy`].
//! Scale and rounding strategy must be explicitly provided when using rounded constructors.
//!
//! ```
//! use openpit::param::{Price, RoundingStrategy};
//!
//! let price = Price::from_str_rounded("100.126", 2, RoundingStrategy::DEFAULT)?;
//! assert_eq!(price.to_string(), "100.13");
//!
//! let price = Price::from_str_rounded("100.125", 2, RoundingStrategy::BANKER)?;
//! assert_eq!(price.to_string(), "100.12");
//!
//! let profit = Price::from_str_rounded("123.456", 2, RoundingStrategy::CONSERVATIVE_PROFIT)?;
//! assert_eq!(profit.to_string(), "123.45");
//! # Ok::<(), openpit::param::Error>(())
//! ```
//!
//! # Error model
//!
//! Every checked arithmetic operation on decimal value types returns
//! [`Result<T, Error>`].
//! Arithmetic and validation failures are contextual and always carry the
//! parameter type through [`ParamKind`].
//!
//! - [`Error::Negative`] includes the failing [`ParamKind`].
//! - [`Error::DivisionByZero`] includes the failing [`ParamKind`].
//! - [`Error::Overflow`] includes the failing [`ParamKind`].
//! - [`Error::Underflow`] includes the failing [`ParamKind`].
//! - [`Error::InvalidLeverage`] reports invalid fixed-point leverage values.
//!
//! There are no panicking arithmetic operators in this module.
//! All arithmetic is exposed through `checked_*` methods.
//!
//! # Arithmetic operations
//!
//! Numeric types provide checked operations:
//!
//! - `checked_add(other)` — addition
//! - `checked_sub(other)` — subtraction
//! - `checked_mul_i64(scalar)` — multiplication by `i64`
//! - `checked_mul_u64(scalar)` — multiplication by `u64`
//! - `checked_mul_f64(scalar)` — multiplication by `f64`
//! - `checked_div_i64(divisor)` — division by `i64`
//! - `checked_div_u64(divisor)` — division by `u64`
//! - `checked_div_f64(divisor)` — division by `f64`
//! - `checked_rem_i64(divisor)` — remainder by `i64`
//! - `checked_rem_u64(divisor)` — remainder by `u64`
//! - `checked_rem_f64(divisor)` — remainder by `f64`
//!
//! # Examples
//!
//! ```
//! use openpit::param::{Error, ParamKind, Price, Quantity};
//!
//! let price = Price::from_str("100")?;
//! let qty = Quantity::from_str("10")?;
//! let volume = price.calculate_volume(qty)?;
//! assert_eq!(volume.to_string(), "1000");
//!
//! let err = Quantity::from_str("-1").expect_err("negative quantity must be rejected");
//! assert_eq!(err, Error::Negative { param: ParamKind::Quantity });
//! # Ok::<(), Error>(())
//! ```

use rust_decimal::Decimal;
use std::fmt::{Display, Formatter};

pub mod account_id;
pub mod adjustment_amount;
pub mod asset;
pub mod cash_flow;
pub mod fee;
pub mod fill_type;
pub mod leverage;
pub mod notional;
pub mod pnl;
pub mod position_effect;
pub mod position_mode;
pub mod position_side;
pub mod position_size;
pub mod price;
pub mod quantity;
pub mod side;
pub mod trade;
pub mod trade_amount;
pub mod volume;

pub use account_id::{AccountId, AccountIdError};
pub use adjustment_amount::AdjustmentAmount;
pub use asset::{Asset, AssetError};
pub use cash_flow::CashFlow;
pub use fee::Fee;
pub use fill_type::FillType;
pub use leverage::Leverage;
pub use notional::Notional;
pub use pnl::Pnl;
pub use position_effect::PositionEffect;
pub use position_mode::PositionMode;
pub use position_side::PositionSide;
pub use position_size::PositionSize;
pub use price::Price;
pub use quantity::Quantity;
pub use side::Side;
pub use trade::Trade;
pub use trade_amount::TradeAmount;
pub use volume::Volume;

/// Identifies a parameter type that caused a validation or arithmetic error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ParamKind {
    Quantity,
    Volume,
    Notional,
    Price,
    Pnl,
    CashFlow,
    PositionSize,
    Fee,
    Leverage,
}

impl Display for ParamKind {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Quantity => formatter.write_str("Quantity"),
            Self::Volume => formatter.write_str("Volume"),
            Self::Notional => formatter.write_str("Notional"),
            Self::Price => formatter.write_str("Price"),
            Self::Pnl => formatter.write_str("Pnl"),
            Self::CashFlow => formatter.write_str("CashFlow"),
            Self::PositionSize => formatter.write_str("PositionSize"),
            Self::Fee => formatter.write_str("Fee"),
            Self::Leverage => formatter.write_str("Leverage"),
        }
    }
}

/// Rounding strategy for decimal values.
///
/// Provides domain-specific constants for common financial rounding scenarios.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RoundingStrategy {
    /// Round half to nearest even number.
    ///
    /// When the value is exactly halfway between two numbers, rounds to the nearest even number.
    /// This minimizes systematic bias in repeated calculations and is the IEEE 754 default.
    ///
    /// Also known as banker's rounding or round-half-to-even.
    ///
    /// # Examples
    ///
    /// - 0.5 -> 0 (rounds to even)
    /// - 1.5 -> 2 (rounds to even)
    /// - 2.5 -> 2 (rounds to even)
    MidpointNearestEven,
    /// Round half away from zero.
    ///
    /// When the value is exactly halfway between two numbers, rounds away from zero.
    /// This is the traditional "round half up" for positive numbers.
    ///
    /// # Examples
    ///
    /// - 0.5 -> 1
    /// - 1.5 -> 2
    /// - -0.5 -> -1
    MidpointAwayFromZero,
    /// Always round towards positive infinity (ceiling).
    ///
    /// # Examples
    ///
    /// - 0.1 -> 1
    /// - 0.9 -> 1
    /// - -0.1 -> 0
    Up,
    /// Always round towards negative infinity (floor).
    ///
    /// # Examples
    ///
    /// - 0.1 -> 0
    /// - 0.9 -> 0
    /// - -0.1 -> -1
    Down,
}

impl RoundingStrategy {
    /// Default rounding strategy (rounds half to nearest even).
    ///
    /// Minimizes systematic bias in repeated calculations.
    /// This is the recommended strategy for most financial calculations.
    ///
    /// Same as [`RoundingStrategy::MidpointNearestEven`].
    pub const DEFAULT: Self = Self::MidpointNearestEven;

    /// Banker's rounding (same as DEFAULT).
    ///
    /// Rounds .5 to nearest even number to minimize cumulative error.
    /// Traditional choice in financial institutions for neutral rounding.
    ///
    /// Same as [`RoundingStrategy::MidpointNearestEven`].
    pub const BANKER: Self = Self::MidpointNearestEven;

    /// Conservative profit estimation: always rounds down.
    ///
    /// Use when calculating profits to avoid overestimation.
    /// Ensures reported profits are never higher than actual.
    ///
    /// Same as [`RoundingStrategy::Down`].
    ///
    /// # Examples
    ///
    /// ```
    /// use openpit::param::{Pnl, RoundingStrategy};
    ///
    /// // Profit of 123.456 rounds down to 123.45
    /// let profit = Pnl::from_str_rounded("123.456", 2, RoundingStrategy::CONSERVATIVE_PROFIT)?;
    /// assert_eq!(profit.to_string(), "123.45");
    /// # Ok::<(), openpit::param::Error>(())
    /// ```
    pub const CONSERVATIVE_PROFIT: Self = Self::Down;

    /// Conservative loss estimation: always rounds up (towards more negative).
    ///
    /// Use when calculating losses or risk exposure to avoid underestimation.
    /// Ensures reported losses/risks are never lower than actual.
    ///
    /// Same as [`RoundingStrategy::Down`] (floor rounds towards more negative).
    ///
    /// # Examples
    ///
    /// ```
    /// use openpit::param::{Pnl, RoundingStrategy};
    ///
    /// // Loss of -123.456 rounds to -123.46 (more negative)
    /// let loss = Pnl::from_str_rounded("-123.456", 2, RoundingStrategy::CONSERVATIVE_LOSS)?;
    /// assert_eq!(loss.to_string(), "-123.46");
    /// # Ok::<(), openpit::param::Error>(())
    /// ```
    pub const CONSERVATIVE_LOSS: Self = Self::Down;
}

impl From<RoundingStrategy> for rust_decimal::RoundingStrategy {
    fn from(strategy: RoundingStrategy) -> Self {
        match strategy {
            RoundingStrategy::MidpointNearestEven => Self::MidpointNearestEven,
            RoundingStrategy::MidpointAwayFromZero => Self::MidpointAwayFromZero,
            RoundingStrategy::Up => Self::ToPositiveInfinity,
            RoundingStrategy::Down => Self::ToNegativeInfinity,
        }
    }
}

/// Stable error code for parameter validation and arithmetic failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
#[non_exhaustive]
pub enum ErrorCode {
    /// Error code is not specified.
    Unspecified = 0,
    /// Value must be non-negative.
    Negative = 1,
    /// Division by zero.
    DivisionByZero = 2,
    /// Arithmetic overflow.
    Overflow = 3,
    /// Arithmetic underflow.
    Underflow = 4,
    /// Invalid float value.
    InvalidFloat = 5,
    /// Invalid textual format.
    InvalidFormat = 6,
    /// Invalid price value.
    InvalidPrice = 7,
    /// Invalid leverage value.
    InvalidLeverage = 8,
    /// Catch-all code for unknown cases.
    Other = u32::MAX,
}

/// Errors for parameter value validation and arithmetic.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[repr(u32)]
#[non_exhaustive]
pub enum Error {
    /// Value must be non-negative for this parameter type.
    Negative { param: ParamKind } = ErrorCode::Negative as u32,
    /// Division by zero.
    DivisionByZero { param: ParamKind } = ErrorCode::DivisionByZero as u32,
    /// Arithmetic overflow occurred.
    Overflow { param: ParamKind } = ErrorCode::Overflow as u32,
    /// Arithmetic underflow resulted in negative value for unsigned type.
    Underflow { param: ParamKind } = ErrorCode::Underflow as u32,
    /// Conversion from `f64` failed.
    InvalidFloat = ErrorCode::InvalidFloat as u32,
    /// Failed to parse string into decimal value.
    InvalidFormat { param: ParamKind, input: Box<str> } = ErrorCode::InvalidFormat as u32,
    /// Price has invalid value.
    InvalidPrice = ErrorCode::InvalidPrice as u32,
    /// Leverage has invalid value.
    InvalidLeverage = ErrorCode::InvalidLeverage as u32,
    /// Catch-all error variant reserved for forward-compatible mapping.
    Other = ErrorCode::Other as u32,
}

impl Error {
    /// Returns stable machine-readable error code.
    pub const fn code(&self) -> ErrorCode {
        match self {
            Self::Negative { .. } => ErrorCode::Negative,
            Self::DivisionByZero { .. } => ErrorCode::DivisionByZero,
            Self::Overflow { .. } => ErrorCode::Overflow,
            Self::Underflow { .. } => ErrorCode::Underflow,
            Self::InvalidFloat => ErrorCode::InvalidFloat,
            Self::InvalidFormat { .. } => ErrorCode::InvalidFormat,
            Self::InvalidPrice => ErrorCode::InvalidPrice,
            Self::InvalidLeverage => ErrorCode::InvalidLeverage,
            Self::Other => ErrorCode::Other,
        }
    }
}

impl Display for Error {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Negative { param } => {
                write!(formatter, "value must be non-negative for {param}")
            }
            Self::DivisionByZero { param } => {
                write!(formatter, "division by zero in {param}")
            }
            Self::Overflow { param } => write!(formatter, "arithmetic overflow in {param}"),
            Self::Underflow { param } => write!(formatter, "arithmetic underflow in {param}"),
            Self::InvalidFloat => formatter.write_str("invalid float value"),
            Self::InvalidPrice => formatter.write_str("invalid price value"),
            Self::InvalidLeverage => formatter.write_str("invalid leverage value"),
            Self::InvalidFormat { param, input } => {
                write!(formatter, "invalid format for {param}: '{input}'")
            }
            Self::Other => formatter.write_str("other parameter error"),
        }
    }
}

impl std::error::Error for Error {}

/// Converts `f64` to `Decimal`, rejecting NaN and infinity.
fn decimal_from_f64(value: f64) -> Result<Decimal, Error> {
    if !value.is_finite() {
        return Err(Error::InvalidFloat);
    }
    Decimal::try_from(value).map_err(|_| Error::InvalidFloat)
}

#[inline]
fn ensure_non_negative(value: Decimal, param: ParamKind) -> Result<Decimal, Error> {
    if value < Decimal::ZERO {
        return Err(Error::Negative { param });
    }
    Ok(value)
}

macro_rules! define_non_negative_value_type {
    ($(#[$meta:meta])* $name:ident, $kind:expr) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        $(#[$meta])*
        pub struct $name(rust_decimal::Decimal);

        impl $name {
            /// Zero value.
            pub const ZERO: Self = Self(rust_decimal::Decimal::ZERO);
            const KIND: super::ParamKind = $kind;

            /// Creates a new value from a [`rust_decimal::Decimal`].
            ///
            /// # Errors
            ///
            /// Returns [`Error::Negative`](super::Error::Negative)
            /// when `value` is below zero.
            pub fn new(value: rust_decimal::Decimal) -> Result<Self, super::Error> {
                super::ensure_non_negative(value, Self::KIND).map(Self)
            }

            /// Creates a value from a 64-bit floating-point number.
            ///
            /// # Errors
            ///
            /// Returns [`Error::InvalidFloat`](super::Error::InvalidFloat) when value is NaN or infinite.
            /// Returns [`Error::Negative`](super::Error::Negative) when value is negative.
            pub fn from_f64(value: f64) -> Result<Self, super::Error> {
                let decimal = super::decimal_from_f64(value)?;
                Self::new(decimal)
            }

            /// Creates a value from a string representation.
            ///
            /// # Examples
            ///
            /// ```
            /// # use openpit::param::Quantity;
            /// let qty = Quantity::from_str("123.45")?;
            /// assert_eq!(qty.to_string(), "123.45");
            /// # Ok::<(), openpit::param::Error>(())
            /// ```
            ///
            /// # Errors
            ///
            /// Returns [`Error::InvalidFormat`](super::Error::InvalidFormat) when string cannot be parsed.
            /// Returns [`Error::Negative`](super::Error::Negative) when parsed value is negative.
            #[allow(clippy::should_implement_trait)]
            pub fn from_str(s: &str) -> Result<Self, super::Error> {
                let decimal = s.parse::<rust_decimal::Decimal>().map_err(|_| {
                    super::Error::InvalidFormat {
                        param: Self::KIND,
                        input: s.into(),
                    }
                })?;
                Self::new(decimal)
            }

            /// Creates a value from a string representation with rounding.
            ///
            /// The value is rounded to the specified scale using the provided rounding strategy.
            ///
            /// # Examples
            ///
            /// ```
            /// # use openpit::param::{Quantity, RoundingStrategy};
            /// let qty = Quantity::from_str_rounded(
            ///     "123.123456789",
            ///     8,
            ///     RoundingStrategy::DEFAULT
            /// )?;
            /// assert_eq!(qty.to_string(), "123.12345679");
            ///
            /// // Conservative profit
            /// let qty = Quantity::from_str_rounded(
            ///     "123.125",
            ///     2,
            ///     RoundingStrategy::CONSERVATIVE_PROFIT
            /// )?;
            /// assert_eq!(qty.to_string(), "123.12");
            /// # Ok::<(), openpit::param::Error>(())
            /// ```
            ///
            /// # Errors
            ///
            /// Returns [`Error::InvalidFormat`](super::Error::InvalidFormat) when string cannot be parsed.
            /// Returns [`Error::Negative`](super::Error::Negative) when parsed value is negative after rounding.
            pub fn from_str_rounded(
                s: &str,
                scale: u32,
                rounding: super::RoundingStrategy,
            ) -> Result<Self, super::Error> {
                let mut decimal = s.parse::<rust_decimal::Decimal>().map_err(|_| {
                    super::Error::InvalidFormat {
                        param: Self::KIND,
                        input: s.into(),
                    }
                })?;
                let strategy: rust_decimal::RoundingStrategy = rounding.into();
                decimal = decimal.round_dp_with_strategy(scale, strategy);

                Self::new(decimal)
            }

            /// Creates a value from a 64-bit floating-point number with rounding.
            ///
            /// The value is rounded to the specified scale using the provided rounding strategy.
            ///
            /// # Errors
            ///
            /// Returns [`Error::InvalidFloat`](super::Error::InvalidFloat) when value is NaN or infinite.
            /// Returns [`Error::Negative`](super::Error::Negative) when value is negative after rounding.
            pub fn from_f64_rounded(
                value: f64,
                scale: u32,
                rounding: super::RoundingStrategy,
            ) -> Result<Self, super::Error> {
                let mut decimal = super::decimal_from_f64(value)?;
                let strategy: rust_decimal::RoundingStrategy = rounding.into();
                decimal = decimal.round_dp_with_strategy(scale, strategy);

                Self::new(decimal)
            }

            /// Creates a value from a [`rust_decimal::Decimal`] with rounding.
            ///
            /// The value is rounded to the specified scale using the provided rounding strategy.
            ///
            /// # Errors
            ///
            /// Returns [`Error::Negative`](super::Error::Negative) when value is negative after rounding.
            pub fn from_decimal_rounded(
                mut decimal: rust_decimal::Decimal,
                scale: u32,
                rounding: super::RoundingStrategy,
            ) -> Result<Self, super::Error> {
                let strategy: rust_decimal::RoundingStrategy = rounding.into();
                decimal = decimal.round_dp_with_strategy(scale, strategy);
                Self::new(decimal)
            }

            pub(crate) fn new_unchecked(value: rust_decimal::Decimal) -> Self {
                #[cfg(debug_assertions)]
                assert!(value >= rust_decimal::Decimal::ZERO);
                Self(value)
            }

            /// Returns the underlying decimal value.
            pub fn to_decimal(&self) -> rust_decimal::Decimal {
                self.0
            }

            /// Returns `true` when the value is exactly zero.
            pub fn is_zero(&self) -> bool {
                self.0 == rust_decimal::Decimal::ZERO
            }

            /// Safely adds two values.
            ///
            /// # Errors
            ///
            /// Returns [`Error::Overflow`](super::Error::Overflow) on overflow.
            pub fn checked_add(self, other: Self) -> Result<Self, super::Error> {
                self.0
                    .checked_add(other.0)
                    .map(Self::new_unchecked)
                    .ok_or(super::Error::Overflow { param: Self::KIND })
            }

            /// Safely subtracts two values.
            ///
            /// # Errors
            ///
            /// Returns [`Error::Underflow`](super::Error::Underflow) when result would be negative.
            pub fn checked_sub(self, other: Self) -> Result<Self, super::Error> {
                let result = self
                    .0
                    .checked_sub(other.0)
                    .ok_or(super::Error::Overflow { param: Self::KIND })?;
                Self::new(result).map_err(|_| super::Error::Underflow { param: Self::KIND })
            }

            /// Safely multiplies by an `i64` scalar.
            ///
            /// # Errors
            ///
            /// Returns [`Error::Negative`](super::Error::Negative) when scalar is negative.
            /// Returns [`Error::Overflow`](super::Error::Overflow) on overflow.
            pub fn checked_mul_i64(self, scalar: i64) -> Result<Self, super::Error> {
                if scalar < 0 {
                    return Err(super::Error::Negative { param: Self::KIND });
                }
                self.0
                    .checked_mul(rust_decimal::Decimal::from(scalar))
                    .map(Self::new_unchecked)
                    .ok_or(super::Error::Overflow { param: Self::KIND })
            }

            /// Safely multiplies by a `u64` scalar.
            ///
            /// # Errors
            ///
            /// Returns [`Error::Overflow`](super::Error::Overflow) on overflow.
            pub fn checked_mul_u64(self, scalar: u64) -> Result<Self, super::Error> {
                self.0
                    .checked_mul(rust_decimal::Decimal::from(scalar))
                    .map(Self::new_unchecked)
                    .ok_or(super::Error::Overflow { param: Self::KIND })
            }

            /// Safely multiplies by an `f64` factor.
            ///
            /// # Errors
            ///
            /// Returns [`Error::InvalidFloat`](super::Error::InvalidFloat) when factor is NaN or infinite.
            /// Returns [`Error::Negative`](super::Error::Negative) when factor is negative.
            /// Returns [`Error::Overflow`](super::Error::Overflow) on overflow.
            pub fn checked_mul_f64(self, factor: f64) -> Result<Self, super::Error> {
                let factor = super::decimal_from_f64(factor)?;
                if factor < rust_decimal::Decimal::ZERO {
                    return Err(super::Error::Negative { param: Self::KIND });
                }
                self.0
                    .checked_mul(factor)
                    .map(Self::new_unchecked)
                    .ok_or(super::Error::Overflow { param: Self::KIND })
            }

            /// Safely divides by an `i64` divisor.
            ///
            /// # Errors
            ///
            /// Returns [`Error::DivisionByZero`](super::Error::DivisionByZero) when divisor is zero.
            /// Returns [`Error::Negative`](super::Error::Negative) when divisor is negative.
            /// Returns [`Error::Overflow`](super::Error::Overflow) on overflow.
            pub fn checked_div_i64(self, divisor: i64) -> Result<Self, super::Error> {
                if divisor == 0 {
                    return Err(super::Error::DivisionByZero { param: Self::KIND });
                }
                if divisor < 0 {
                    return Err(super::Error::Negative { param: Self::KIND });
                }
                self.0
                    .checked_div(rust_decimal::Decimal::from(divisor))
                    .map(Self::new_unchecked)
                    .ok_or(super::Error::Overflow { param: Self::KIND })
            }

            /// Safely divides by a `u64` divisor.
            ///
            /// # Errors
            ///
            /// Returns [`Error::DivisionByZero`](super::Error::DivisionByZero) when divisor is zero.
            /// Returns [`Error::Overflow`](super::Error::Overflow) on overflow.
            pub fn checked_div_u64(self, divisor: u64) -> Result<Self, super::Error> {
                if divisor == 0 {
                    return Err(super::Error::DivisionByZero { param: Self::KIND });
                }
                self.0
                    .checked_div(rust_decimal::Decimal::from(divisor))
                    .map(Self::new_unchecked)
                    .ok_or(super::Error::Overflow { param: Self::KIND })
            }

            /// Safely divides by an `f64` divisor.
            ///
            /// # Errors
            ///
            /// Returns [`Error::InvalidFloat`](super::Error::InvalidFloat) when divisor is NaN or infinite.
            /// Returns [`Error::DivisionByZero`](super::Error::DivisionByZero) when divisor is zero.
            /// Returns [`Error::Negative`](super::Error::Negative) when divisor is negative.
            /// Returns [`Error::Overflow`](super::Error::Overflow) on overflow.
            pub fn checked_div_f64(self, divisor: f64) -> Result<Self, super::Error> {
                let divisor = super::decimal_from_f64(divisor)?;
                if divisor == rust_decimal::Decimal::ZERO {
                    return Err(super::Error::DivisionByZero { param: Self::KIND });
                }
                if divisor < rust_decimal::Decimal::ZERO {
                    return Err(super::Error::Negative { param: Self::KIND });
                }
                self.0
                    .checked_div(divisor)
                    .map(Self::new_unchecked)
                    .ok_or(super::Error::Overflow { param: Self::KIND })
            }

            /// Safely computes remainder by `i64` divisor.
            ///
            /// # Errors
            ///
            /// Returns [`Error::DivisionByZero`](super::Error::DivisionByZero) when divisor is zero.
            /// Returns [`Error::Negative`](super::Error::Negative) when divisor is negative.
            /// Returns [`Error::Overflow`](super::Error::Overflow) on overflow.
            pub fn checked_rem_i64(self, divisor: i64) -> Result<Self, super::Error> {
                if divisor == 0 {
                    return Err(super::Error::DivisionByZero { param: Self::KIND });
                }
                if divisor < 0 {
                    return Err(super::Error::Negative { param: Self::KIND });
                }
                self.0
                    .checked_rem(rust_decimal::Decimal::from(divisor))
                    .map(Self::new_unchecked)
                    .ok_or(super::Error::Overflow { param: Self::KIND })
            }

            /// Safely computes remainder by `u64` divisor.
            ///
            /// # Errors
            ///
            /// Returns [`Error::DivisionByZero`](super::Error::DivisionByZero) when divisor is zero.
            /// Returns [`Error::Overflow`](super::Error::Overflow) on overflow.
            pub fn checked_rem_u64(self, divisor: u64) -> Result<Self, super::Error> {
                if divisor == 0 {
                    return Err(super::Error::DivisionByZero { param: Self::KIND });
                }
                self.0
                    .checked_rem(rust_decimal::Decimal::from(divisor))
                    .map(Self::new_unchecked)
                    .ok_or(super::Error::Overflow { param: Self::KIND })
            }

            /// Safely computes remainder by `f64` divisor.
            ///
            /// # Errors
            ///
            /// Returns [`Error::InvalidFloat`](super::Error::InvalidFloat) when divisor is NaN or infinite.
            /// Returns [`Error::DivisionByZero`](super::Error::DivisionByZero) when divisor is zero.
            /// Returns [`Error::Negative`](super::Error::Negative) when divisor is negative.
            /// Returns [`Error::Overflow`](super::Error::Overflow) on overflow.
            pub fn checked_rem_f64(self, divisor: f64) -> Result<Self, super::Error> {
                let divisor = super::decimal_from_f64(divisor)?;
                if divisor == rust_decimal::Decimal::ZERO {
                    return Err(super::Error::DivisionByZero { param: Self::KIND });
                }
                if divisor < rust_decimal::Decimal::ZERO {
                    return Err(super::Error::Negative { param: Self::KIND });
                }
                self.0
                    .checked_rem(divisor)
                    .map(Self::new_unchecked)
                    .ok_or(super::Error::Overflow { param: Self::KIND })
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                std::fmt::Display::fmt(&self.0, formatter)
            }
        }
    };
}

macro_rules! define_signed_value_type {
    ($(#[$meta:meta])* $name:ident, $kind:expr) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        $(#[$meta])*
        pub struct $name(rust_decimal::Decimal);

        impl $name {
            /// Zero value.
            pub const ZERO: Self = Self(rust_decimal::Decimal::ZERO);
            const KIND: super::ParamKind = $kind;

            /// Creates a new value from a [`rust_decimal::Decimal`].
            pub fn new(value: rust_decimal::Decimal) -> Self {
                Self(value)
            }

            /// Creates a value from a 64-bit floating-point number.
            ///
            /// # Errors
            ///
            /// Returns [`Error::InvalidFloat`](super::Error::InvalidFloat) when value is NaN or infinite.
            pub fn from_f64(value: f64) -> Result<Self, super::Error> {
                let decimal = super::decimal_from_f64(value)?;
                Ok(Self::new(decimal))
            }

            /// Creates a value from a string representation.
            ///
            /// # Examples
            ///
            /// ```
            /// # use openpit::param::Price;
            /// let price = Price::from_str("42350.75")?;
            /// assert_eq!(price.to_string(), "42350.75");
            ///
            /// let negative_price = Price::from_str("-10.5")?;
            /// assert_eq!(negative_price.to_string(), "-10.5");
            /// # Ok::<(), openpit::param::Error>(())
            /// ```
            ///
            /// # Errors
            ///
            /// Returns [`Error::InvalidFormat`](super::Error::InvalidFormat) when string cannot be parsed.
            #[allow(clippy::should_implement_trait)]
            pub fn from_str(s: &str) -> Result<Self, super::Error> {
                let decimal = s.parse::<rust_decimal::Decimal>().map_err(|_| {
                    super::Error::InvalidFormat {
                        param: Self::KIND,
                        input: s.into(),
                    }
                })?;
                Ok(Self::new(decimal))
            }

            /// Creates a value from a string representation with rounding.
            ///
            /// The value is rounded to the specified scale using the provided rounding strategy.
            ///
            /// # Errors
            ///
            /// Returns [`Error::InvalidFormat`](super::Error::InvalidFormat) when string cannot be parsed.
            pub fn from_str_rounded(
                s: &str,
                scale: u32,
                rounding: super::RoundingStrategy,
            ) -> Result<Self, super::Error> {
                let mut decimal = s.parse::<rust_decimal::Decimal>().map_err(|_| {
                    super::Error::InvalidFormat {
                        param: Self::KIND,
                        input: s.into(),
                    }
                })?;
                let strategy: rust_decimal::RoundingStrategy = rounding.into();
                decimal = decimal.round_dp_with_strategy(scale, strategy);

                Ok(Self::new(decimal))
            }

            /// Creates a value from a 64-bit floating-point number with rounding.
            ///
            /// The value is rounded to the specified scale using the provided rounding strategy.
            ///
            /// # Errors
            ///
            /// Returns [`Error::InvalidFloat`](super::Error::InvalidFloat) when value is NaN or infinite.
            pub fn from_f64_rounded(
                value: f64,
                scale: u32,
                rounding: super::RoundingStrategy,
            ) -> Result<Self, super::Error> {
                let mut decimal = super::decimal_from_f64(value)?;
                let strategy: rust_decimal::RoundingStrategy = rounding.into();
                decimal = decimal.round_dp_with_strategy(scale, strategy);

                Ok(Self::new(decimal))
            }

            /// Creates a value from a [`rust_decimal::Decimal`] with rounding.
            ///
            /// The value is rounded to the specified scale using the provided rounding strategy.
            ///
            /// # Errors
            ///
            /// This method is infallible for signed types as rounding cannot produce errors.
            pub fn from_decimal_rounded(
                mut decimal: rust_decimal::Decimal,
                scale: u32,
                rounding: super::RoundingStrategy,
            ) -> Result<Self, super::Error> {
                let strategy: rust_decimal::RoundingStrategy = rounding.into();
                decimal = decimal.round_dp_with_strategy(scale, strategy);
                Ok(Self::new(decimal))
            }

            /// Returns the underlying decimal value.
            pub fn to_decimal(&self) -> rust_decimal::Decimal {
                self.0
            }

            /// Returns `true` when the value is exactly zero.
            pub fn is_zero(&self) -> bool {
                self.0 == rust_decimal::Decimal::ZERO
            }

            /// Safely adds two values.
            ///
            /// # Errors
            ///
            /// Returns [`Error::Overflow`](super::Error::Overflow) on overflow.
            pub fn checked_add(self, other: Self) -> Result<Self, super::Error> {
                self.0
                    .checked_add(other.0)
                    .map(Self)
                    .ok_or(super::Error::Overflow { param: Self::KIND })
            }

            /// Safely subtracts two values.
            ///
            /// # Errors
            ///
            /// Returns [`Error::Overflow`](super::Error::Overflow) on overflow or underflow.
            pub fn checked_sub(self, other: Self) -> Result<Self, super::Error> {
                self.0
                    .checked_sub(other.0)
                    .map(Self)
                    .ok_or(super::Error::Overflow { param: Self::KIND })
            }

            /// Safely negates the value.
            ///
            /// # Errors
            ///
            /// Returns [`Error::Overflow`](super::Error::Overflow) if negation would overflow.
            pub fn checked_neg(self) -> Result<Self, super::Error> {
                rust_decimal::Decimal::ZERO
                    .checked_sub(self.0)
                    .map(Self)
                    .ok_or(super::Error::Overflow { param: Self::KIND })
            }

            /// Safely multiplies by an `i64` scalar.
            ///
            /// # Errors
            ///
            /// Returns [`Error::Overflow`](super::Error::Overflow) on overflow.
            pub fn checked_mul_i64(self, scalar: i64) -> Result<Self, super::Error> {
                self.0
                    .checked_mul(rust_decimal::Decimal::from(scalar))
                    .map(Self)
                    .ok_or(super::Error::Overflow { param: Self::KIND })
            }

            /// Safely multiplies by a `u64` scalar.
            ///
            /// # Errors
            ///
            /// Returns [`Error::Overflow`](super::Error::Overflow) on overflow.
            pub fn checked_mul_u64(self, scalar: u64) -> Result<Self, super::Error> {
                self.0
                    .checked_mul(rust_decimal::Decimal::from(scalar))
                    .map(Self)
                    .ok_or(super::Error::Overflow { param: Self::KIND })
            }

            /// Safely multiplies by an `f64` factor.
            ///
            /// # Errors
            ///
            /// Returns [`Error::InvalidFloat`](super::Error::InvalidFloat) when factor is NaN or infinite.
            /// Returns [`Error::Overflow`](super::Error::Overflow) on overflow.
            pub fn checked_mul_f64(self, factor: f64) -> Result<Self, super::Error> {
                let factor = super::decimal_from_f64(factor)?;
                self.0
                    .checked_mul(factor)
                    .map(Self)
                    .ok_or(super::Error::Overflow { param: Self::KIND })
            }

            /// Safely divides by an `i64` divisor.
            ///
            /// # Errors
            ///
            /// Returns [`Error::DivisionByZero`](super::Error::DivisionByZero) when divisor is zero.
            /// Returns [`Error::Overflow`](super::Error::Overflow) on overflow.
            pub fn checked_div_i64(self, divisor: i64) -> Result<Self, super::Error> {
                if divisor == 0 {
                    return Err(super::Error::DivisionByZero { param: Self::KIND });
                }
                self.0
                    .checked_div(rust_decimal::Decimal::from(divisor))
                    .map(Self)
                    .ok_or(super::Error::Overflow { param: Self::KIND })
            }

            /// Safely divides by a `u64` divisor.
            ///
            /// # Errors
            ///
            /// Returns [`Error::DivisionByZero`](super::Error::DivisionByZero) when divisor is zero.
            /// Returns [`Error::Overflow`](super::Error::Overflow) on overflow.
            pub fn checked_div_u64(self, divisor: u64) -> Result<Self, super::Error> {
                if divisor == 0 {
                    return Err(super::Error::DivisionByZero { param: Self::KIND });
                }
                self.0
                    .checked_div(rust_decimal::Decimal::from(divisor))
                    .map(Self)
                    .ok_or(super::Error::Overflow { param: Self::KIND })
            }

            /// Safely divides by an `f64` divisor.
            ///
            /// # Errors
            ///
            /// Returns [`Error::InvalidFloat`](super::Error::InvalidFloat) when divisor is NaN or infinite.
            /// Returns [`Error::DivisionByZero`](super::Error::DivisionByZero) when divisor is zero.
            /// Returns [`Error::Overflow`](super::Error::Overflow) on overflow.
            pub fn checked_div_f64(self, divisor: f64) -> Result<Self, super::Error> {
                let divisor = super::decimal_from_f64(divisor)?;
                if divisor == rust_decimal::Decimal::ZERO {
                    return Err(super::Error::DivisionByZero { param: Self::KIND });
                }
                self.0
                    .checked_div(divisor)
                    .map(Self)
                    .ok_or(super::Error::Overflow { param: Self::KIND })
            }

            /// Safely computes remainder by `i64` divisor.
            ///
            /// # Errors
            ///
            /// Returns [`Error::DivisionByZero`](super::Error::DivisionByZero) when divisor is zero.
            /// Returns [`Error::Overflow`](super::Error::Overflow) on overflow.
            pub fn checked_rem_i64(self, divisor: i64) -> Result<Self, super::Error> {
                if divisor == 0 {
                    return Err(super::Error::DivisionByZero { param: Self::KIND });
                }
                self.0
                    .checked_rem(rust_decimal::Decimal::from(divisor))
                    .map(Self)
                    .ok_or(super::Error::Overflow { param: Self::KIND })
            }

            /// Safely computes remainder by `u64` divisor.
            ///
            /// # Errors
            ///
            /// Returns [`Error::DivisionByZero`](super::Error::DivisionByZero) when divisor is zero.
            /// Returns [`Error::Overflow`](super::Error::Overflow) on overflow.
            pub fn checked_rem_u64(self, divisor: u64) -> Result<Self, super::Error> {
                if divisor == 0 {
                    return Err(super::Error::DivisionByZero { param: Self::KIND });
                }
                self.0
                    .checked_rem(rust_decimal::Decimal::from(divisor))
                    .map(Self)
                    .ok_or(super::Error::Overflow { param: Self::KIND })
            }

            /// Safely computes remainder by `f64` divisor.
            ///
            /// # Errors
            ///
            /// Returns [`Error::InvalidFloat`](super::Error::InvalidFloat) when divisor is NaN or infinite.
            /// Returns [`Error::DivisionByZero`](super::Error::DivisionByZero) when divisor is zero.
            /// Returns [`Error::Overflow`](super::Error::Overflow) on overflow.
            pub fn checked_rem_f64(self, divisor: f64) -> Result<Self, super::Error> {
                let divisor = super::decimal_from_f64(divisor)?;
                if divisor == rust_decimal::Decimal::ZERO {
                    return Err(super::Error::DivisionByZero { param: Self::KIND });
                }
                self.0
                    .checked_rem(divisor)
                    .map(Self)
                    .ok_or(super::Error::Overflow { param: Self::KIND })
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                std::fmt::Display::fmt(&self.0, formatter)
            }
        }
    };
}

pub(crate) use define_non_negative_value_type;
pub(crate) use define_signed_value_type;

#[cfg(test)]
macro_rules! test_value_type_common_methods {
    (unsigned: $type:ident, $test_mod:ident, $sample_value:expr) => {
        mod $test_mod {
            use super::$type;

            #[test]
            fn display_works() {
                let val = <$type>::from_str($sample_value).expect("must be valid");
                assert_eq!(val.to_string(), $sample_value);
            }

            #[test]
            fn string_roundtrip() {
                let val = <$type>::from_str($sample_value).expect("must be valid");
                assert_eq!(val.to_string(), $sample_value);
            }

            #[test]
            fn is_zero_for_non_zero() {
                let val = <$type>::from_str($sample_value).expect("must be valid");
                assert!(!val.is_zero());
            }

            #[test]
            fn is_zero_for_zero_constant() {
                assert!(<$type>::ZERO.is_zero());
            }

            #[test]
            fn zero_constant_has_zero_decimal() {
                assert_eq!(<$type>::ZERO.to_decimal(), rust_decimal::Decimal::ZERO);
            }

            #[test]
            fn debug_contains_value() {
                let val = <$type>::from_str($sample_value).expect("must be valid");
                let debug_str = format!("{val:?}");
                assert!(debug_str.contains($sample_value));
            }

            #[test]
            fn clone_creates_equal_value() {
                let val = <$type>::from_str($sample_value).expect("must be valid");
                #[allow(clippy::clone_on_copy)]
                let cloned = val.clone();
                assert_eq!(val, cloned);
            }

            #[test]
            fn copy_works() {
                let val = <$type>::from_str($sample_value).expect("must be valid");
                let copied = val;
                assert_eq!(val.to_decimal(), copied.to_decimal());
            }

            #[test]
            fn ordering_works() {
                let small = <$type>::from_str("10").expect("must be valid");
                let large = <$type>::from_str("100").expect("must be valid");
                assert!(small < large);
                assert!(large > small);
                assert!(small <= large);
                assert!(large >= small);
            }

            #[test]
            fn hash_is_deterministic() {
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};

                let val1 = <$type>::from_str($sample_value).expect("must be valid");
                let val2 = <$type>::from_str($sample_value).expect("must be valid");

                let mut h1 = DefaultHasher::new();
                val1.hash(&mut h1);

                let mut h2 = DefaultHasher::new();
                val2.hash(&mut h2);

                assert_eq!(h1.finish(), h2.finish());
            }
        }
    };
    (signed: $type:ident, $test_mod:ident, $pos_value:expr, $neg_value:expr) => {
        mod $test_mod {
            use super::$type;

            #[test]
            fn display_works_positive() {
                let val = <$type>::from_str($pos_value).expect("must be valid");
                assert_eq!(val.to_string(), $pos_value);
            }

            #[test]
            fn display_works_negative() {
                let val = <$type>::from_str($neg_value).expect("must be valid");
                assert_eq!(val.to_string(), $neg_value);
            }

            #[test]
            fn string_roundtrip() {
                let val = <$type>::from_str($pos_value).expect("must be valid");
                assert_eq!(val.to_string(), $pos_value);
            }

            #[test]
            fn is_zero_for_non_zero() {
                let val = <$type>::from_str($pos_value).expect("must be valid");
                assert!(!val.is_zero());
            }

            #[test]
            fn is_zero_for_zero_constant() {
                assert!(<$type>::ZERO.is_zero());
            }

            #[test]
            fn zero_constant_has_zero_decimal() {
                assert_eq!(<$type>::ZERO.to_decimal(), rust_decimal::Decimal::ZERO);
            }

            #[test]
            fn debug_contains_value() {
                let val = <$type>::from_str($pos_value).expect("must be valid");
                let debug_str = format!("{val:?}");
                assert!(debug_str.contains($pos_value));
            }

            #[test]
            fn clone_creates_equal_value() {
                let val = <$type>::from_str($pos_value).expect("must be valid");
                #[allow(clippy::clone_on_copy)]
                let cloned = val.clone();
                assert_eq!(val, cloned);
            }

            #[test]
            fn copy_works() {
                let val = <$type>::from_str($pos_value).expect("must be valid");
                let copied = val;
                assert_eq!(val.to_decimal(), copied.to_decimal());
            }

            #[test]
            fn ordering_works() {
                let small = <$type>::from_str("10").expect("must be valid");
                let large = <$type>::from_str("100").expect("must be valid");
                assert!(small < large);
                assert!(large > small);
            }

            #[test]
            fn ordering_negative_vs_positive() {
                let negative = <$type>::from_str($neg_value).expect("must be valid");
                let positive = <$type>::from_str($pos_value).expect("must be valid");
                assert!(negative < positive);
            }

            #[test]
            fn hash_is_deterministic() {
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};

                let val1 = <$type>::from_str($pos_value).expect("must be valid");
                let val2 = <$type>::from_str($pos_value).expect("must be valid");

                let mut h1 = DefaultHasher::new();
                val1.hash(&mut h1);

                let mut h2 = DefaultHasher::new();
                val2.hash(&mut h2);

                assert_eq!(h1.finish(), h2.finish());
            }
        }
    };
}

#[cfg(test)]
#[allow(clippy::wrong_self_convention)]
mod tests {
    use super::{Error, ParamKind, RoundingStrategy};
    use rust_decimal::Decimal;

    define_non_negative_value_type!(TestUnsigned, ParamKind::Quantity);
    define_signed_value_type!(TestSigned, ParamKind::Price);

    #[test]
    fn error_display_messages_are_stable() {
        assert_eq!(
            Error::Negative {
                param: ParamKind::Quantity
            }
            .to_string(),
            "value must be non-negative for Quantity"
        );
        assert_eq!(
            Error::DivisionByZero {
                param: ParamKind::Price
            }
            .to_string(),
            "division by zero in Price"
        );
        assert_eq!(Error::InvalidFloat.to_string(), "invalid float value");
        assert_eq!(Error::InvalidPrice.to_string(), "invalid price value");
        assert_eq!(Error::InvalidLeverage.to_string(), "invalid leverage value");
        assert_eq!(
            Error::InvalidFormat {
                param: ParamKind::Quantity,
                input: "abc".into()
            }
            .to_string(),
            "invalid format for Quantity: 'abc'"
        );
        assert_eq!(
            Error::Overflow {
                param: ParamKind::Quantity
            }
            .to_string(),
            "arithmetic overflow in Quantity"
        );
        assert_eq!(
            Error::Underflow {
                param: ParamKind::Volume
            }
            .to_string(),
            "arithmetic underflow in Volume"
        );
    }

    #[test]
    fn param_kind_display_is_stable() {
        assert_eq!(ParamKind::Quantity.to_string(), "Quantity");
        assert_eq!(ParamKind::Volume.to_string(), "Volume");
        assert_eq!(ParamKind::Notional.to_string(), "Notional");
        assert_eq!(ParamKind::Price.to_string(), "Price");
        assert_eq!(ParamKind::Pnl.to_string(), "Pnl");
        assert_eq!(ParamKind::CashFlow.to_string(), "CashFlow");
        assert_eq!(ParamKind::PositionSize.to_string(), "PositionSize");
        assert_eq!(ParamKind::Fee.to_string(), "Fee");
        assert_eq!(ParamKind::Leverage.to_string(), "Leverage");
    }

    #[test]
    fn rounding_strategy_constants_and_conversion_are_stable() {
        assert_eq!(
            RoundingStrategy::DEFAULT,
            RoundingStrategy::MidpointNearestEven
        );
        assert_eq!(
            RoundingStrategy::BANKER,
            RoundingStrategy::MidpointNearestEven
        );
        assert_eq!(
            RoundingStrategy::CONSERVATIVE_PROFIT,
            RoundingStrategy::Down
        );
        assert_eq!(RoundingStrategy::CONSERVATIVE_LOSS, RoundingStrategy::Down);

        assert_eq!(
            rust_decimal::RoundingStrategy::from(RoundingStrategy::Up),
            rust_decimal::RoundingStrategy::ToPositiveInfinity
        );
        assert_eq!(
            rust_decimal::RoundingStrategy::from(RoundingStrategy::Down),
            rust_decimal::RoundingStrategy::ToNegativeInfinity
        );
    }

    #[test]
    fn non_negative_macro_error_paths_are_covered() {
        let one = TestUnsigned::from_str("1").expect("must be valid");
        let two = TestUnsigned::from_str("2").expect("must be valid");

        assert_eq!(one.to_decimal(), Decimal::ONE);
        assert_eq!(
            TestUnsigned::from_f64(1.0)
                .expect("must be valid")
                .to_decimal(),
            Decimal::ONE
        );
        assert_eq!(
            TestUnsigned::from_str("-1"),
            Err(Error::Negative {
                param: ParamKind::Quantity
            })
        );

        assert_eq!(
            one.checked_mul_i64(-1),
            Err(Error::Negative {
                param: ParamKind::Quantity
            })
        );
        assert_eq!(
            one.checked_mul_i64(2).expect("must be valid").to_decimal(),
            Decimal::from(2)
        );
        assert_eq!(
            one.checked_mul_f64(2.5)
                .expect("must be valid")
                .to_decimal(),
            Decimal::from_str_exact("2.5").expect("must be valid")
        );
        assert_eq!(one.checked_mul_f64(f64::NAN), Err(Error::InvalidFloat));
        assert_eq!(
            one.checked_mul_f64(-1.0),
            Err(Error::Negative {
                param: ParamKind::Quantity
            })
        );

        assert_eq!(
            one.checked_div_i64(0),
            Err(Error::DivisionByZero {
                param: ParamKind::Quantity
            })
        );
        assert_eq!(
            one.checked_div_i64(-1),
            Err(Error::Negative {
                param: ParamKind::Quantity
            })
        );
        assert_eq!(
            one.checked_div_u64(0),
            Err(Error::DivisionByZero {
                param: ParamKind::Quantity
            })
        );
        assert_eq!(
            one.checked_div_u64(2).expect("must be valid").to_decimal(),
            Decimal::from_str_exact("0.5").expect("must be valid")
        );
        assert_eq!(
            one.checked_div_f64(0.0),
            Err(Error::DivisionByZero {
                param: ParamKind::Quantity
            })
        );
        assert_eq!(one.checked_div_f64(f64::NAN), Err(Error::InvalidFloat));
        assert_eq!(
            one.checked_div_f64(-1.0),
            Err(Error::Negative {
                param: ParamKind::Quantity
            })
        );

        assert_eq!(
            one.checked_rem_i64(0),
            Err(Error::DivisionByZero {
                param: ParamKind::Quantity
            })
        );
        assert_eq!(
            one.checked_rem_i64(-1),
            Err(Error::Negative {
                param: ParamKind::Quantity
            })
        );
        assert_eq!(
            one.checked_rem_u64(0),
            Err(Error::DivisionByZero {
                param: ParamKind::Quantity
            })
        );
        assert_eq!(
            one.checked_rem_f64(0.0),
            Err(Error::DivisionByZero {
                param: ParamKind::Quantity
            })
        );
        assert_eq!(one.checked_rem_f64(f64::NAN), Err(Error::InvalidFloat));
        assert_eq!(
            one.checked_rem_f64(-1.0),
            Err(Error::Negative {
                param: ParamKind::Quantity
            })
        );
        assert_eq!(
            TestUnsigned::from_f64(f64::INFINITY),
            Err(Error::InvalidFloat)
        );
        assert_eq!(
            TestUnsigned::from_str("abc"),
            Err(Error::InvalidFormat {
                param: ParamKind::Quantity,
                input: "abc".into()
            })
        );
        assert_eq!(
            TestUnsigned::from_str_rounded("bad", 2, RoundingStrategy::DEFAULT),
            Err(Error::InvalidFormat {
                param: ParamKind::Quantity,
                input: "bad".into()
            })
        );
        assert_eq!(
            TestUnsigned::from_f64_rounded(f64::NAN, 2, RoundingStrategy::DEFAULT),
            Err(Error::InvalidFloat)
        );
        assert_eq!(
            one.checked_sub(TestUnsigned(Decimal::MAX)),
            Err(Error::Underflow {
                param: ParamKind::Quantity
            })
        );
        assert_eq!(
            TestUnsigned(Decimal::MIN).checked_sub(one),
            Err(Error::Overflow {
                param: ParamKind::Quantity
            })
        );
        assert_eq!(
            one.checked_div_i64(2).expect("must be valid").to_decimal(),
            Decimal::from_str_exact("0.5").expect("must be valid")
        );
        assert_eq!(
            one.checked_div_f64(2.0)
                .expect("must be valid")
                .to_decimal(),
            Decimal::from_str_exact("0.5").expect("must be valid")
        );
        assert_eq!(
            two.checked_rem_i64(2).expect("must be valid").to_decimal(),
            Decimal::ZERO
        );
        assert_eq!(
            two.checked_rem_u64(2).expect("must be valid").to_decimal(),
            Decimal::ZERO
        );
        assert_eq!(
            one.checked_rem_f64(0.5)
                .expect("must be valid")
                .to_decimal(),
            Decimal::ZERO
        );
    }

    #[test]
    fn signed_macro_error_paths_are_covered() {
        let value = TestSigned::from_str("5").expect("must be valid");
        assert_eq!(value.to_decimal(), Decimal::from(5));

        assert_eq!(
            value.checked_rem_i64(0),
            Err(Error::DivisionByZero {
                param: ParamKind::Price
            })
        );
        assert_eq!(
            value.checked_rem_u64(0),
            Err(Error::DivisionByZero {
                param: ParamKind::Price
            })
        );
        assert_eq!(
            value.checked_rem_f64(0.0),
            Err(Error::DivisionByZero {
                param: ParamKind::Price
            })
        );
        assert_eq!(value.checked_div_f64(f64::NAN), Err(Error::InvalidFloat));
        assert_eq!(value.checked_rem_f64(f64::NAN), Err(Error::InvalidFloat));
        assert_eq!(value.checked_mul_f64(f64::NAN), Err(Error::InvalidFloat));
        assert_eq!(
            TestSigned::from_f64(f64::NEG_INFINITY),
            Err(Error::InvalidFloat)
        );
        assert_eq!(
            TestSigned::from_f64(1.25)
                .expect("must be valid")
                .to_decimal(),
            Decimal::from_str_exact("1.25").expect("must be valid")
        );
        assert_eq!(
            TestSigned::from_str("bad"),
            Err(Error::InvalidFormat {
                param: ParamKind::Price,
                input: "bad".into()
            })
        );
        assert_eq!(
            TestSigned::from_str_rounded("bad", 2, RoundingStrategy::DEFAULT),
            Err(Error::InvalidFormat {
                param: ParamKind::Price,
                input: "bad".into()
            })
        );
        assert_eq!(
            TestSigned::from_f64_rounded(f64::NAN, 2, RoundingStrategy::DEFAULT),
            Err(Error::InvalidFloat)
        );
        assert_eq!(
            value.checked_div_i64(0),
            Err(Error::DivisionByZero {
                param: ParamKind::Price
            })
        );
        assert_eq!(
            value.checked_div_u64(0),
            Err(Error::DivisionByZero {
                param: ParamKind::Price
            })
        );
        assert_eq!(
            value.checked_div_f64(0.0),
            Err(Error::DivisionByZero {
                param: ParamKind::Price
            })
        );
        assert_eq!(
            value
                .checked_div_i64(2)
                .expect("must be valid")
                .to_decimal(),
            Decimal::from_str_exact("2.5").expect("must be valid")
        );
        assert_eq!(
            value
                .checked_div_u64(2)
                .expect("must be valid")
                .to_decimal(),
            Decimal::from_str_exact("2.5").expect("must be valid")
        );
        assert_eq!(
            value
                .checked_div_f64(2.0)
                .expect("must be valid")
                .to_decimal(),
            Decimal::from_str_exact("2.5").expect("must be valid")
        );
        assert_eq!(
            value
                .checked_rem_i64(2)
                .expect("must be valid")
                .to_decimal(),
            Decimal::ONE
        );
        assert_eq!(
            value
                .checked_rem_u64(2)
                .expect("must be valid")
                .to_decimal(),
            Decimal::ONE
        );
        assert_eq!(
            value
                .checked_rem_f64(2.0)
                .expect("must be valid")
                .to_decimal(),
            Decimal::ONE
        );
    }

    #[test]
    fn unsigned_rounded_constructors_are_covered() {
        assert_eq!(
            TestUnsigned::from_str_rounded("123.125", 2, RoundingStrategy::DEFAULT)
                .expect("must be valid")
                .to_decimal(),
            Decimal::from_str_exact("123.12").expect("must be valid")
        );
        assert_eq!(
            TestUnsigned::from_f64_rounded(123.125, 2, RoundingStrategy::MidpointAwayFromZero)
                .expect("must be valid")
                .to_decimal(),
            Decimal::from_str_exact("123.13").expect("must be valid")
        );
        assert_eq!(
            TestUnsigned::from_decimal_rounded(
                Decimal::from_str_exact("123.121").expect("must be valid"),
                2,
                RoundingStrategy::Up
            )
            .expect("must be valid")
            .to_decimal(),
            Decimal::from_str_exact("123.13").expect("must be valid")
        );
        assert_eq!(
            TestUnsigned::from_str_rounded("-0.011", 2, RoundingStrategy::DEFAULT),
            Err(Error::Negative {
                param: ParamKind::Quantity
            })
        );
    }

    #[test]
    fn signed_rounded_constructors_are_covered() {
        assert_eq!(
            TestSigned::from_str_rounded("-123.456", 2, RoundingStrategy::CONSERVATIVE_LOSS)
                .expect("must be valid")
                .to_decimal(),
            Decimal::from_str_exact("-123.46").expect("must be valid")
        );
        assert_eq!(
            TestSigned::from_f64_rounded(123.456, 2, RoundingStrategy::CONSERVATIVE_PROFIT)
                .expect("must be valid")
                .to_decimal(),
            Decimal::from_str_exact("123.45").expect("must be valid")
        );
        assert_eq!(
            TestSigned::from_decimal_rounded(
                Decimal::from_str_exact("-123.121").expect("must be valid"),
                2,
                RoundingStrategy::Down
            )
            .expect("must be valid")
            .to_decimal(),
            Decimal::from_str_exact("-123.13").expect("must be valid")
        );
    }

    #[test]
    fn unsigned_common_arithmetic_paths_are_covered() {
        let one = TestUnsigned::from_str("1").expect("must be valid");
        let two = TestUnsigned::from_str("2").expect("must be valid");

        assert_eq!(
            one.checked_add(two).expect("must be valid").to_decimal(),
            Decimal::from(3)
        );
        assert_eq!(
            two.checked_sub(one).expect("must be valid").to_decimal(),
            Decimal::from(1)
        );
        assert_eq!(
            two.checked_mul_u64(3).expect("must be valid").to_decimal(),
            Decimal::from(6)
        );
    }

    #[test]
    fn signed_common_arithmetic_paths_are_covered() {
        let two = TestSigned::from_str("2").expect("must be valid");
        let one = TestSigned::from_str("1").expect("must be valid");

        assert_eq!(
            two.checked_add(one).expect("must be valid").to_decimal(),
            Decimal::from(3)
        );
        assert_eq!(
            two.checked_sub(one).expect("must be valid").to_decimal(),
            Decimal::from(1)
        );
        assert_eq!(
            two.checked_neg().expect("must be valid").to_decimal(),
            Decimal::from(-2)
        );
        assert_eq!(
            two.checked_mul_i64(-2).expect("must be valid").to_decimal(),
            Decimal::from(-4)
        );
        assert_eq!(
            two.checked_mul_u64(3).expect("must be valid").to_decimal(),
            Decimal::from(6)
        );
        assert_eq!(
            two.checked_mul_f64(1.5)
                .expect("must be valid")
                .to_decimal(),
            Decimal::from_str_exact("3").expect("must be valid")
        );
        assert_eq!(
            two.checked_div_i64(2).expect("must be valid").to_decimal(),
            Decimal::from(1)
        );
        assert_eq!(
            two.checked_div_u64(2).expect("must be valid").to_decimal(),
            Decimal::from(1)
        );
    }

    #[test]
    #[should_panic]
    fn non_negative_new_unchecked_panics_for_negative_quantity_in_debug() {
        let _ = TestUnsigned::new_unchecked(Decimal::NEGATIVE_ONE);
    }

    test_value_type_common_methods!(unsigned: TestUnsigned, test_unsigned_common_methods, "123.45");
    test_value_type_common_methods!(signed: TestSigned, test_signed_common_methods, "42.75", "-10.25");
}
