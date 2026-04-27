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

use super::{define_non_negative_value_type, Error, Leverage, ParamKind, Price, Quantity, Volume};

define_non_negative_value_type!(
    /// Monetary position exposure used for margin and risk calculation.
    ///
    /// Notional is the absolute monetary value of a position in the settlement
    /// currency: `|price| × quantity`. It is always non-negative and represents
    /// the full face value of the position regardless of leverage.
    ///
    /// Margin required to hold the position equals `notional / leverage`.
    Notional,
    ParamKind::Notional
);

impl Notional {
    /// Computes notional from price and quantity.
    ///
    /// Uses the absolute value of price so the result is always non-negative,
    /// consistent with face-value semantics. Both long and short exposures of
    /// the same magnitude produce identical notional.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Overflow`] with [`ParamKind::Price`] when multiplication
    /// overflows.
    pub fn from_price_quantity(price: Price, quantity: Quantity) -> Result<Self, Error> {
        let notional = price
            .to_decimal()
            .abs()
            .checked_mul(quantity.to_decimal())
            .ok_or(Error::Overflow {
                param: ParamKind::Price,
            })?;
        Ok(Self::new_unchecked(notional))
    }

    /// Converts trade volume into position notional.
    ///
    /// Both types represent monetary amounts in the settlement currency; this
    /// cast changes the semantic context from "order size" to "position
    /// exposure".
    pub fn from_volume(volume: Volume) -> Self {
        Self::new_unchecked(volume.to_decimal())
    }

    /// Converts position notional back into settlement volume.
    ///
    /// The inverse of [`Self::from_volume`]; the numeric value is preserved.
    pub fn to_volume(self) -> Volume {
        Volume::new_unchecked(self.to_decimal())
    }

    /// Computes the margin required to hold this position at the given leverage.
    ///
    /// Formula: `margin = notional / leverage`.
    ///
    /// Uses exact decimal arithmetic to avoid rounding accumulation.
    ///
    /// # Examples
    ///
    /// ```
    /// use openpit::param::{Leverage, Notional};
    ///
    /// let notional = Notional::from_str("10000")?;
    /// let leverage = Leverage::from_u16(100)?;
    /// let margin = notional.calculate_margin_required(leverage)?;
    /// assert_eq!(margin.to_string(), "100");
    /// # Ok::<(), openpit::param::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`Error::Overflow`] with [`ParamKind::Notional`] on arithmetic
    /// overflow.
    pub fn calculate_margin_required(self, leverage: Leverage) -> Result<Self, Error> {
        leverage.calculate_margin_required(self)
    }
}

#[cfg(test)]
mod tests {
    use super::Notional;
    use crate::param::{Error, Leverage, ParamKind, Price, Quantity, Volume};
    use rust_decimal::Decimal;

    fn d(value: &str) -> Decimal {
        value
            .parse()
            .expect("decimal literal in tests must be valid")
    }

    #[test]
    fn from_price_quantity_computes_absolute_notional() {
        let price = Price::from_str("42350.75").expect("must be valid");
        let qty = Quantity::from_str("0.15").expect("must be valid");

        let notional = Notional::from_price_quantity(price, qty).expect("must be valid");

        assert_eq!(notional.to_decimal(), d("6352.6125"));
    }

    #[test]
    fn from_price_quantity_uses_absolute_price() {
        let price = Price::from_str("-100.0").expect("must be valid");
        let qty = Quantity::from_str("2").expect("must be valid");

        let notional = Notional::from_price_quantity(price, qty).expect("must be valid");

        assert_eq!(notional.to_decimal(), d("200"));
    }

    #[test]
    fn from_price_quantity_reports_overflow() {
        let price = Price::new(Decimal::MAX);
        let qty = Quantity::from_str("2").expect("must be valid");

        assert_eq!(
            Notional::from_price_quantity(price, qty),
            Err(Error::Overflow {
                param: ParamKind::Price
            })
        );
    }

    #[test]
    fn from_volume_and_to_volume_roundtrip() {
        let vol = Volume::from_str("1234.56").expect("must be valid");

        let notional = Notional::from_volume(vol);
        let back = notional.to_volume();

        assert_eq!(back.to_decimal(), vol.to_decimal());
    }

    #[test]
    fn calculate_margin_required_divides_by_leverage() {
        let notional = Notional::from_str("10000").expect("must be valid");
        let leverage = Leverage::from_u16(100).expect("must be valid");

        let margin = notional
            .calculate_margin_required(leverage)
            .expect("must be valid");

        assert_eq!(margin.to_decimal(), d("100"));
    }

    #[test]
    fn calculate_margin_required_with_fractional_leverage() {
        let notional = Notional::from_str("1050").expect("must be valid");
        let leverage = Leverage::from_f64(10.5).expect("must be valid");

        // 1050 * 10 / 105 = 100 exactly
        let margin = notional
            .calculate_margin_required(leverage)
            .expect("must be valid");

        assert_eq!(margin.to_decimal(), d("100"));
    }

    #[test]
    fn calculate_margin_required_with_min_leverage() {
        let notional = Notional::from_str("5000").expect("must be valid");
        let leverage = Leverage::from_u16(1).expect("must be valid");

        let margin = notional
            .calculate_margin_required(leverage)
            .expect("must be valid");

        assert_eq!(margin.to_decimal(), d("5000"));
    }

    #[test]
    fn from_price_quantity_zero_quantity_yields_zero() {
        let price = Price::from_str("50000").expect("must be valid");
        let qty = Quantity::ZERO;

        let notional = Notional::from_price_quantity(price, qty).expect("must be valid");

        assert!(notional.is_zero());
    }
}
