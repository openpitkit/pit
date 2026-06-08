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

use super::{define_signed_value_type, Error, ParamKind, PositionSize, Quantity, Volume};

define_signed_value_type!(
    /// Price per one instrument unit.
    ///
    /// Can be negative in certain derivative markets (e.g., options, futures with storage costs,
    /// calendar spreads).
    Price,
    ParamKind::Price
);

impl Price {
    /// Calculates the signed notional position size as `price × quantity`.
    ///
    /// Preserves the sign of the price; use this when a negative price is
    /// meaningful (e.g. crediting the buyer). For unsigned notional use
    /// [`Price::calculate_volume`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::Overflow`] with [`ParamKind::Price`] if multiplication overflows.
    pub fn calculate_position_size(self, quantity: Quantity) -> Result<PositionSize, Error> {
        self.to_decimal()
            .checked_mul(quantity.to_decimal())
            .ok_or(Error::Overflow {
                param: ParamKind::Price,
            })
            .map(PositionSize::new)
    }

    /// Calculates volume from price and quantity.
    ///
    /// Uses absolute values of both price and quantity to ensure volume is non-negative.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Overflow`] with [`ParamKind::Price`] if multiplication overflows.
    pub fn calculate_volume(self, quantity: Quantity) -> Result<Volume, Error> {
        let volume_decimal = self
            .to_decimal()
            .abs()
            .checked_mul(quantity.to_decimal().abs())
            .ok_or(Error::Overflow {
                param: ParamKind::Price,
            })?;
        Volume::new(volume_decimal)
    }
}

#[cfg(test)]
mod tests {
    use super::Price;
    use crate::param::{Error, ParamKind, Quantity};
    use rust_decimal::Decimal;

    fn d(value: &str) -> Decimal {
        value
            .parse()
            .expect("decimal literal in tests must be valid")
    }

    #[test]
    fn calculate_position_size_positive_price() {
        let price = Price::from_str("100").expect("must be valid");
        let qty = Quantity::from_str("2.5").expect("must be valid");
        let pos = price
            .calculate_position_size(qty)
            .expect("must not overflow");
        assert_eq!(pos.to_decimal(), d("250"));
    }

    #[test]
    fn calculate_position_size_negative_price_preserves_sign() {
        let price = Price::new(d("-50"));
        let qty = Quantity::from_str("3").expect("must be valid");
        let pos = price
            .calculate_position_size(qty)
            .expect("must not overflow");
        assert_eq!(pos.to_decimal(), d("-150"));
    }

    #[test]
    fn calculate_position_size_overflow_returns_error() {
        let price = Price::new(Decimal::MAX);
        let qty = Quantity::from_str("2").expect("must be valid");
        assert_eq!(
            price.calculate_position_size(qty),
            Err(Error::Overflow {
                param: ParamKind::Price
            })
        );
    }

    #[test]
    fn calculate_volume_works() {
        let price = Price::from_str("42350.75").expect("must be valid");
        let quantity = Quantity::from_str("0.15").expect("must be valid");

        let volume = price
            .calculate_volume(quantity)
            .expect("volume must be calculable");

        assert_eq!(volume.to_decimal(), d("6352.6125"));
    }

    #[test]
    fn calculate_volume_reports_overflow_for_extreme_values() {
        let price = Price::new(Decimal::MAX);
        let quantity = Quantity::from_str("2").expect("must be valid");

        assert_eq!(
            price.calculate_volume(quantity),
            Err(Error::Overflow {
                param: ParamKind::Price
            })
        );
    }
}
