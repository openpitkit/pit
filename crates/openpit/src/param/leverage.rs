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

use super::{Error, Notional, ParamKind};
use rust_decimal::Decimal;
use std::fmt::{Display, Formatter};

/// Leverage multiplier used to calculate required margin.
///
/// Stored internally as fixed-point with scale `10`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Leverage(u16);

impl Leverage {
    /// Fixed-point scale for internal leverage storage.
    pub const SCALE: u16 = 10;

    /// Minimum valid leverage in whole units (`1x`).
    pub const MIN: u16 = 1;

    /// Maximum valid leverage in whole units (`3000x`).
    pub const MAX: u16 = 3000;

    /// Supported leverage step.
    pub const STEP: f32 = 0.1;

    const MIN_RAW: u16 = Self::MIN * Self::SCALE;
    const MAX_RAW: u16 = Self::MAX * Self::SCALE;

    /// Creates leverage from fixed-point raw value with scale [`Self::SCALE`].
    ///
    /// For example:
    /// - `10` means `1.0x`
    /// - `11` means `1.1x`
    /// - `1005` means `100.5x`
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidLeverage`] when `raw` is outside
    /// `10..=30000`.
    pub fn from_raw(raw: u16) -> Result<Self, Error> {
        if !(Self::MIN_RAW..=Self::MAX_RAW).contains(&raw) {
            return Err(Error::InvalidLeverage);
        }
        Ok(Self(raw))
    }

    /// Returns fixed-point raw value with scale [`Self::SCALE`].
    pub const fn raw(&self) -> u16 {
        self.0
    }

    /// Creates leverage from an integer multiplier.
    ///
    /// # Examples
    ///
    /// ```
    /// use openpit::param::Leverage;
    ///
    /// let lev = Leverage::from_u16(100)?;
    /// assert_eq!(lev.value(), 100.0);
    /// # Ok::<(), openpit::param::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidLeverage`] when `multiplier` is outside
    /// `1..=3000`.
    pub fn from_u16(multiplier: u16) -> Result<Self, Error> {
        let raw = multiplier.checked_mul(Self::SCALE).ok_or(Error::Overflow {
            param: ParamKind::Leverage,
        })?;
        Self::from_raw(raw)
    }

    /// Creates leverage from floating-point multiplier.
    ///
    /// Value must be finite, in range `1.0..=3000.0`, and aligned to step
    /// `0.1`.
    ///
    /// # Examples
    ///
    /// ```
    /// use openpit::param::Leverage;
    ///
    /// let lev = Leverage::from_f64(100.5)?;
    /// assert_eq!(lev.value(), 100.5);
    /// # Ok::<(), openpit::param::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidLeverage`] when `multiplier` is not finite,
    /// outside `1.0..=3000.0`, or not aligned to `0.1` step.
    pub fn from_f64(multiplier: f64) -> Result<Self, Error> {
        if !multiplier.is_finite() {
            return Err(Error::InvalidLeverage);
        }

        let scaled = multiplier * f64::from(Self::SCALE);
        let rounded = scaled.round();
        if (scaled - rounded).abs() > 1e-9 {
            return Err(Error::InvalidLeverage);
        }

        let raw_i64 = rounded as i64;
        if raw_i64 < i64::from(Self::MIN_RAW) || raw_i64 > i64::from(Self::MAX_RAW) {
            return Err(Error::InvalidLeverage);
        }

        Self::from_raw(raw_i64 as u16)
    }

    /// Returns the leverage value as floating-point multiplier.
    ///
    /// # Examples
    ///
    /// ```
    /// use openpit::param::Leverage;
    ///
    /// let lev = Leverage::from_f64(123.4)?;
    /// assert_eq!(lev.value(), 123.4);
    /// # Ok::<(), openpit::param::Error>(())
    /// ```
    pub fn value(&self) -> f32 {
        f32::from(self.0) / f32::from(Self::SCALE)
    }

    /// Computes the margin required to hold `notional` at this leverage.
    ///
    /// Formula: `margin = notional / leverage`.
    ///
    /// Uses exact decimal arithmetic.
    ///
    /// # Examples
    ///
    /// ```
    /// use openpit::param::{Leverage, Notional};
    ///
    /// let lev = Leverage::from_u16(100)?;
    /// let notional = Notional::from_str("10000")?;
    /// let margin = lev.calculate_margin_required(notional)?;
    /// assert_eq!(margin.to_string(), "100");
    /// # Ok::<(), openpit::param::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`Error::Overflow`] with [`ParamKind::Notional`] on arithmetic
    /// overflow.
    pub fn calculate_margin_required(&self, notional: Notional) -> Result<Notional, Error> {
        let raw = Decimal::from(self.raw());
        let scale = Decimal::from(Self::SCALE);
        // margin = notional × SCALE / raw  ≡  notional / (raw / SCALE)
        let numerator = notional
            .to_decimal()
            .checked_mul(scale)
            .ok_or(Error::Overflow {
                param: ParamKind::Notional,
            })?;
        let margin = numerator.checked_div(raw).ok_or(Error::Overflow {
            param: ParamKind::Notional,
        })?;
        Ok(Notional::new_unchecked(margin))
    }
}

impl Display for Leverage {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        let integer = self.0 / Self::SCALE;
        let fractional = self.0 % Self::SCALE;
        if fractional == 0 {
            write!(formatter, "{integer}")
        } else {
            write!(formatter, "{integer}.{fractional}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Leverage;
    use crate::param::{Error, Notional, ParamKind};

    #[test]
    fn from_u16_creates_valid_leverage() {
        let lev = Leverage::from_u16(100).expect("leverage must be valid");

        assert_eq!(lev.value(), 100.0);
    }

    #[test]
    fn from_u16_scales_value() {
        let lev = Leverage::from_u16(100).expect("leverage must be valid");

        assert_eq!(lev.value(), 100.0);
    }

    #[test]
    fn from_raw_and_raw_roundtrip() {
        let lev = Leverage::from_raw(1005).expect("leverage must be valid");
        assert_eq!(lev.raw(), 1005);
        assert_eq!(lev.value(), 100.5);
    }

    #[test]
    fn from_raw_supports_fractional_table() {
        let cases = [
            (11_u16, 1.1_f32),
            (1005_u16, 100.5_f32),
            (29999_u16, 2999.9_f32),
        ];
        for (raw, expected) in cases {
            let lev = Leverage::from_raw(raw).expect("leverage must be valid");
            assert_eq!(lev.value(), expected);
            assert_eq!(lev.raw(), raw);
        }
    }

    #[test]
    fn from_raw_boundaries_table() {
        let cases = [(10_u16, 1.0_f32), (30000_u16, 3000.0_f32)];
        for (raw, expected) in cases {
            let lev = Leverage::from_raw(raw).expect("boundary leverage must be valid");
            assert_eq!(lev.value(), expected);
            assert_eq!(lev.raw(), raw);
        }
    }

    #[test]
    fn from_raw_rejects_invalid_range_table() {
        let cases = [9_u16, 30001_u16];
        for raw in cases {
            assert_eq!(Leverage::from_raw(raw), Err(Error::InvalidLeverage));
        }
    }

    #[test]
    fn from_u16_rejects_zero() {
        assert_eq!(Leverage::from_u16(0), Err(Error::InvalidLeverage));
    }

    #[test]
    fn from_u16_rejects_values_above_business_limit() {
        assert_eq!(Leverage::from_u16(3001), Err(Error::InvalidLeverage));
    }

    #[test]
    fn from_u16_reports_overflow() {
        assert_eq!(
            Leverage::from_u16(7000),
            Err(Error::Overflow {
                param: ParamKind::Leverage
            })
        );
    }

    #[test]
    fn from_float_creates_fractional_values_table() {
        let cases = [
            (1.1_f64, 1.1_f32),
            (100.5_f64, 100.5_f32),
            (2999.9_f64, 2999.9_f32),
        ];

        for (input, expected) in cases {
            let leverage = Leverage::from_f64(input).expect("fractional leverage must be valid");
            assert_eq!(leverage.value(), expected);
        }
    }

    #[test]
    fn from_float_rejects_invalid_step_or_range_table() {
        let cases = [
            0.0_f64,
            0.9_f64,
            1.11_f64,
            3000.1_f64,
            f64::NAN,
            f64::INFINITY,
        ];

        for input in cases {
            assert_eq!(Leverage::from_f64(input), Err(Error::InvalidLeverage));
        }
    }

    #[test]
    fn boundaries_are_valid_in_table() {
        let cases = [(Leverage::MIN, 1.0_f32), (Leverage::MAX, 3000.0_f32)];

        for (input, expected) in cases {
            let leverage = Leverage::from_u16(input).expect("boundary leverage must be valid");
            assert_eq!(leverage.value(), expected);
        }
    }

    #[test]
    fn calculate_margin_required_calculates_expected_value() {
        let lev = Leverage::from_u16(100).expect("leverage must be valid");
        let notional = Notional::from_str("1000").expect("notional must be valid");

        assert_eq!(
            lev.calculate_margin_required(notional)
                .expect("margin must be valid")
                .to_string(),
            "10"
        );
    }

    #[test]
    fn display_omits_trailing_fractional_zeroes() {
        assert_eq!(
            Leverage::from_u16(100)
                .expect("leverage must be valid")
                .to_string(),
            "100"
        );
        assert_eq!(
            Leverage::from_f64(100.5)
                .expect("leverage must be valid")
                .to_string(),
            "100.5"
        );
        assert_eq!(
            Leverage::from_u16(2500)
                .expect("leverage must be valid")
                .to_string(),
            "2500"
        );
    }

    #[test]
    fn supports_max_business_leverage() {
        let leverage = Leverage::from_u16(3000).expect("leverage must be valid");

        assert_eq!(leverage.value(), 3000.0);
    }

    #[test]
    fn from_f64_raw_from_raw_value_is_symmetric() {
        let cases = [1.1_f64, 100.5_f64, 2999.9_f64, 1.0_f64, 3000.0_f64];
        for input in cases {
            let from_float = Leverage::from_f64(input).expect("leverage must be valid");
            let from_raw =
                Leverage::from_raw(from_float.raw()).expect("raw leverage must be valid");
            assert_eq!(from_raw.raw(), from_float.raw());
            assert_eq!(from_raw.value(), from_float.value());
        }
    }
}
