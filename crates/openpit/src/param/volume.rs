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

use super::{
    define_non_negative_value_type, CashFlow, Error, Notional, ParamKind, Price, Quantity,
};

define_non_negative_value_type!(
    /// Notional volume value.
    Volume,
    ParamKind::Volume
);

impl Volume {
    /// Converts position notional into settlement volume.
    ///
    /// Both types represent monetary amounts in the settlement currency; this
    /// cast changes the semantic context from "position exposure" to "order
    /// size".
    pub fn from_notional(notional: Notional) -> Self {
        Self::new_unchecked(notional.to_decimal())
    }

    /// Converts volume into a cash flow inflow.
    pub fn to_cash_flow_inflow(self) -> CashFlow {
        CashFlow::new(self.to_decimal())
    }

    /// Converts volume into a cash flow outflow.
    pub fn to_cash_flow_outflow(self) -> CashFlow {
        CashFlow::new(-self.to_decimal())
    }

    /// Calculates quantity from volume and price.
    ///
    /// Uses absolute value of price to ensure quantity is non-negative.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidPrice`] when price is zero.
    /// Returns [`Error::Overflow`] with [`ParamKind::Volume`] when division overflows.
    /// Returns [`Error::Underflow`] with [`ParamKind::Quantity`] when result cannot be
    /// represented as [`Quantity`].
    pub fn calculate_quantity(self, price: Price) -> Result<Quantity, Error> {
        if price.is_zero() {
            return Err(Error::InvalidPrice);
        }

        let quantity_decimal = self
            .to_decimal()
            .checked_div(price.to_decimal().abs())
            .ok_or(Error::Overflow {
                param: ParamKind::Volume,
            })?;

        Quantity::new(quantity_decimal).map_err(|_| Error::Underflow {
            param: ParamKind::Quantity,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Volume;
    use crate::param::{CashFlow, Error, Notional, ParamKind, Price, Quantity};
    use rust_decimal::Decimal;

    fn d(value: &str) -> Decimal {
        value
            .parse()
            .expect("decimal literal in tests must be valid")
    }

    fn v(value: &str) -> Volume {
        Volume::from_str(value).expect("volume literal in tests must be valid")
    }

    #[test]
    fn calculates_quantity() {
        let volume = v("6352.6125");
        let price = Price::new(d("42350.75"));

        assert_eq!(
            volume.calculate_quantity(price).expect("must be valid"),
            Quantity::new(d("0.15")).expect("must be valid")
        );
    }

    #[test]
    fn calculate_quantity_reports_invalid_zero_price() {
        let volume = v("1");
        let zero_price = Price::new(Decimal::ZERO);

        assert_eq!(
            volume.calculate_quantity(zero_price),
            Err(Error::InvalidPrice)
        );
    }

    #[test]
    fn converts_to_cash_flow() {
        let volume = v("10.5");

        assert_eq!(volume.to_cash_flow_inflow(), CashFlow::new(d("10.5")));
        assert_eq!(volume.to_cash_flow_outflow(), CashFlow::new(d("-10.5")));
    }

    #[test]
    fn converts_from_notional() {
        let notional = Notional::from_str("999.75").expect("must be valid");

        let volume = Volume::from_notional(notional);

        assert_eq!(volume.to_decimal(), notional.to_decimal());
    }

    #[test]
    fn calculate_quantity_reports_overflow_for_extreme_ratio() {
        let volume = Volume::new(Decimal::MAX).expect("must be valid");
        let tiny_price = Price::new(Decimal::new(1, 28));

        assert_eq!(
            volume.calculate_quantity(tiny_price),
            Err(Error::Overflow {
                param: ParamKind::Volume
            })
        );
    }

    #[test]
    fn calculate_quantity_maps_internal_negative_result_to_underflow() {
        let invalid_volume = Volume(d("-1"));
        let price = Price::new(d("10"));

        assert_eq!(
            invalid_volume.calculate_quantity(price),
            Err(Error::Underflow {
                param: ParamKind::Quantity
            })
        );
    }
}
