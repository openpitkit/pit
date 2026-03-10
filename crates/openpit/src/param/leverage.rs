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

use super::{Error, ParamKind};

/// Leverage multiplier used to calculate required margin.
///
/// Stored as fixed-point with scale `100`.
/// The real leverage is `stored / 100`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Leverage(u16);

impl Leverage {
    /// Fixed-point scale for stored leverage values.
    pub const SCALE: u16 = 100;

    /// Minimum valid stored value (`0.01x`).
    pub const MIN_RAW: u16 = 1;

    /// Maximum valid stored value (`655.35x`).
    pub const MAX_RAW: u16 = u16::MAX;

    /// Creates leverage from an integer multiplier.
    ///
    /// The stored value is calculated as:
    ///
    /// `stored = multiplier * 100`
    ///
    /// # Examples
    ///
    /// ```
    /// use openpit::param::Leverage;
    ///
    /// let lev = Leverage::new(100)?;
    /// assert_eq!(lev.value(), 100.0);
    /// # Ok::<(), openpit::param::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidLeverage`] when `multiplier` is `0`.
    /// Returns [`Error::Overflow`] with [`ParamKind::Leverage`] when the scaled
    /// value does not fit into `u16`.
    pub fn new(multiplier: u16) -> Result<Self, Error> {
        Self::from_multiplier(multiplier)
    }

    /// Creates leverage from raw fixed-point storage.
    ///
    /// A valid stored leverage must satisfy:
    ///
    /// `stored > 0`
    ///
    /// # Examples
    ///
    /// ```
    /// use openpit::param::Leverage;
    ///
    /// let lev = Leverage::from_stored(10_000)?;
    /// assert_eq!(lev.value(), 100.0);
    /// # Ok::<(), openpit::param::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidLeverage`] when `raw` is `0`.
    pub fn from_stored(raw: u16) -> Result<Self, Error> {
        if raw == 0 {
            return Err(Error::InvalidLeverage);
        }
        Ok(Self(raw))
    }

    /// Creates leverage from an integer multiplier.
    ///
    /// The stored value is calculated as:
    ///
    /// `stored = multiplier * 100`
    ///
    /// # Examples
    ///
    /// ```
    /// use openpit::param::Leverage;
    ///
    /// let lev = Leverage::from_multiplier(100)?;
    /// assert_eq!(lev.value(), 100.0);
    /// # Ok::<(), openpit::param::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidLeverage`] when `multiplier` is `0`.
    /// Returns [`Error::Overflow`] with [`ParamKind::Leverage`] when the scaled
    /// value does not fit into `u16`.
    pub fn from_multiplier(multiplier: u16) -> Result<Self, Error> {
        let raw = multiplier.checked_mul(Self::SCALE).ok_or(Error::Overflow {
            param: ParamKind::Leverage,
        })?;
        Self::from_stored(raw)
    }

    /// Returns the leverage value as floating-point multiplier.
    ///
    /// The value is calculated as:
    ///
    /// `leverage = stored / 100`
    ///
    /// # Examples
    ///
    /// ```
    /// use openpit::param::Leverage;
    ///
    /// let lev = Leverage::from_stored(12_345)?;
    /// assert_eq!(lev.value(), 123.45);
    /// # Ok::<(), openpit::param::Error>(())
    /// ```
    pub fn value(&self) -> f32 {
        f32::from(self.0) / f32::from(Self::SCALE)
    }

    /// Returns the margin required for a given notional exposure.
    ///
    /// The margin is calculated as:
    ///
    /// `margin = notional / leverage`
    ///
    /// # Examples
    ///
    /// ```
    /// use openpit::param::Leverage;
    ///
    /// let lev = Leverage::new(100)?;
    /// let margin = lev.margin_required(1000.0);
    /// assert_eq!(margin, 10.0);
    /// # Ok::<(), openpit::param::Error>(())
    /// ```
    pub fn margin_required(&self, notional: f64) -> f64 {
        notional * f64::from(Self::SCALE) / f64::from(self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::Leverage;
    use crate::param::{Error, ParamKind};

    #[test]
    fn new_creates_valid_leverage() {
        let lev = Leverage::new(100).expect("leverage must be valid");

        assert_eq!(lev.value(), 100.0);
    }

    #[test]
    fn new_rejects_zero() {
        assert_eq!(Leverage::new(0), Err(Error::InvalidLeverage));
    }

    #[test]
    fn from_multiplier_scales_value() {
        let lev = Leverage::from_multiplier(100).expect("leverage must be valid");

        assert_eq!(lev.value(), 100.0);
    }

    #[test]
    fn from_multiplier_rejects_zero_multiplier() {
        assert_eq!(Leverage::from_multiplier(0), Err(Error::InvalidLeverage));
    }

    #[test]
    fn margin_required_calculates_expected_value() {
        let lev = Leverage::from_multiplier(100).expect("leverage must be valid");

        assert_eq!(lev.margin_required(1000.0), 10.0);
    }

    #[test]
    fn validates_boundaries() {
        let min = Leverage::from_stored(Leverage::MIN_RAW).expect("min leverage must be valid");
        let max = Leverage::from_stored(Leverage::MAX_RAW).expect("max leverage must be valid");

        assert_eq!(min.value(), 0.01);
        assert_eq!(max.value(), 655.35);
    }

    #[test]
    fn from_multiplier_reports_overflow() {
        assert_eq!(
            Leverage::from_multiplier(656),
            Err(Error::Overflow {
                param: ParamKind::Leverage
            })
        );
    }
}
