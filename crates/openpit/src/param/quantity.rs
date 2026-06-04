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

use super::{define_non_negative_value_type, Error, ParamKind, PositionSize, Price, Volume};

define_non_negative_value_type!(
    /// Quantity of an instrument.
    Quantity,
    ParamKind::Quantity
);

impl Quantity {
    /// Calculates volume from quantity and price.
    ///
    /// Delegates to [`Price::calculate_volume`] for the actual computation.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Overflow`] with [`ParamKind::Price`] if multiplication overflows.
    pub fn calculate_volume(self, price: Price) -> Result<Volume, Error> {
        price.calculate_volume(self)
    }

    /// Converts this quantity into a [`PositionSize`], preserving its non-negative value.
    ///
    /// Used when a quantity must be expressed as a magnitude in position-management
    /// operations (holds, releases) where no directional context is needed.
    pub fn to_position_size(self) -> PositionSize {
        PositionSize::new(self.to_decimal())
    }
}

#[cfg(test)]
mod tests {
    use super::Quantity;
    use crate::param::{Error, ParamKind, Price};
    use rust_decimal::Decimal;

    fn d(value: &str) -> Decimal {
        value
            .parse()
            .expect("decimal literal in tests must be valid")
    }

    fn q(value: &str) -> Quantity {
        Quantity::from_str(value).expect("quantity literal in tests must be valid")
    }

    #[test]
    fn calculate_volume_is_commutative_with_price() {
        let quantity = q("0.15");
        let price = Price::new(d("42350.75"));

        assert_eq!(
            quantity
                .calculate_volume(price)
                .expect("volume must be calculable")
                .to_decimal(),
            d("6352.6125")
        );
    }

    #[test]
    fn calculate_volume_propagates_price_overflow() {
        let quantity = q("2");
        let price = Price::new(Decimal::MAX);

        assert_eq!(
            quantity.calculate_volume(price),
            Err(Error::Overflow {
                param: ParamKind::Price
            })
        );
    }
}
