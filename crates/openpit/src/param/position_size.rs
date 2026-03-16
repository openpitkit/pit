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

use super::{define_signed_value_type, Error, ParamKind, Quantity, Side};

define_signed_value_type!(
    /// Signed position size where long is positive and short is negative.
    PositionSize,
    ParamKind::PositionSize
);

impl PositionSize {
    /// Creates a position size from a quantity and trading side.
    ///
    /// The resulting position size represents:
    /// - Positive value for [`Side::Buy`] (long position)
    /// - Negative value for [`Side::Sell`] (short position)
    ///
    /// This is the canonical way to construct a position from a trade execution.
    ///
    /// # Examples
    ///
    /// ```
    /// # use openpit::param::{PositionSize, Quantity, Side};
    /// let qty = Quantity::from_str("2")?;
    ///
    /// let long = PositionSize::from_quantity_and_side(qty, Side::Buy);
    /// assert_eq!(long, PositionSize::from_str("2")?);
    ///
    /// let short = PositionSize::from_quantity_and_side(qty, Side::Sell);
    /// assert_eq!(short, PositionSize::from_str("-2")?);
    /// # Ok::<(), openpit::param::Error>(())
    /// ```
    pub fn from_quantity_and_side(quantity: Quantity, side: Side) -> Self {
        let value = match side {
            Side::Buy => quantity.to_decimal(),
            Side::Sell => -quantity.to_decimal(),
        };
        Self::new(value)
    }

    /// Converts this position size into the quantity required to open it and the corresponding
    /// opening side.
    ///
    /// Semantics:
    ///
    /// - If the position size is positive, the opening side is [`Side::Buy`].
    /// - If the position size is negative, the opening side is [`Side::Sell`].
    /// - If the position size is zero, no trade is required and
    ///   [`Quantity::ZERO`] with [`Side::Buy`] is returned.
    ///
    /// The returned quantity is always non-negative and equals
    /// `abs(position_size)`.
    pub fn to_open_quantity(self) -> (Quantity, Side) {
        if self == PositionSize::ZERO {
            return (Quantity::ZERO, Side::Buy);
        }

        let side = if self > PositionSize::ZERO {
            Side::Buy
        } else {
            Side::Sell
        };

        (Quantity::new_unchecked(self.to_decimal().abs()), side)
    }

    /// Converts this position size into the quantity required to close it and the corresponding
    /// closing side.
    ///
    /// Semantics:
    ///
    /// - If the position is positive (long), the closing side is [`Side::Sell`].
    /// - If the position is negative (short), the closing side is [`Side::Buy`].
    /// - If the position is zero, no action is required and `None` is returned
    ///   as the side together with [`Quantity::ZERO`].
    ///
    /// The returned quantity is always non-negative and equals
    /// `abs(position_size)`.
    pub fn to_close_quantity(self) -> (Quantity, Option<Side>) {
        if self == PositionSize::ZERO {
            return (Quantity::ZERO, None);
        }

        let side = if self < PositionSize::ZERO {
            Side::Buy
        } else {
            Side::Sell
        };

        (Quantity::new_unchecked(self.to_decimal().abs()), Some(side))
    }

    /// Adds a `(Quantity, Side)` to a [`PositionSize`].
    ///
    /// Semantics:
    ///
    /// - [`Side::Buy`]  increases the position size.
    /// - [`Side::Sell`] decreases the position size.
    /// - `Quantity` must be non-negative.
    /// - The result may flip sign if the trade over-closes the position.
    ///
    /// This operation is purely arithmetic and does not perform validation beyond relying on
    /// [`Quantity`] invariants.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use openpit::param::{PositionSize, Quantity, Side};
    /// let pos = PositionSize::from_str("1")?;
    /// let qty = Quantity::from_str("1")?;
    ///
    /// let new_pos = pos.checked_add_quantity(qty, Side::Sell)?;
    ///
    /// assert_eq!(new_pos, PositionSize::ZERO);
    /// # Ok(())
    /// # }
    /// ```
    pub fn checked_add_quantity(self, qty: Quantity, side: Side) -> Result<Self, Error> {
        let delta = match side {
            Side::Buy => qty.to_decimal(),
            Side::Sell => -qty.to_decimal(),
        };

        let result = self
            .to_decimal()
            .checked_add(delta)
            .ok_or(Error::Overflow {
                param: ParamKind::PositionSize,
            })?;

        Ok(Self::new(result))
    }
}

#[cfg(test)]
mod tests {
    use super::PositionSize;
    use crate::param::{Error, ParamKind, Quantity, Side};
    use rust_decimal::Decimal;

    fn d(value: &str) -> Decimal {
        value
            .parse()
            .expect("decimal literal in tests must be valid")
    }

    #[test]
    fn converts_to_open_and_close_quantities() {
        let short = PositionSize::new(d("-0.5"));
        let long = PositionSize::new(d("0.5"));
        let expected_qty = Quantity::new(d("0.5")).expect("must be valid");

        assert_eq!(short.to_open_quantity(), (expected_qty, Side::Sell));
        assert_eq!(long.to_open_quantity(), (expected_qty, Side::Buy));
        assert_eq!(short.to_close_quantity(), (expected_qty, Some(Side::Buy)));
        assert_eq!(long.to_close_quantity(), (expected_qty, Some(Side::Sell)));

        assert_eq!(
            PositionSize::ZERO.to_open_quantity(),
            (Quantity::ZERO, Side::Buy)
        );
        assert_eq!(
            PositionSize::ZERO.to_close_quantity(),
            (Quantity::ZERO, None)
        );
    }

    #[test]
    fn builds_from_quantity_and_side() {
        let quantity = Quantity::new(d("2")).expect("must be valid");

        assert_eq!(
            PositionSize::from_quantity_and_side(quantity, Side::Buy),
            PositionSize::new(d("2"))
        );
        assert_eq!(
            PositionSize::from_quantity_and_side(quantity, Side::Sell),
            PositionSize::new(d("-2"))
        );
    }

    #[test]
    fn supports_add_with_quantity_and_side() {
        let start = PositionSize::new(d("1.5"));
        let qty = Quantity::new(d("0.5")).expect("must be valid");

        assert_eq!(
            start
                .checked_add_quantity(qty, Side::Buy)
                .expect("must be valid"),
            PositionSize::new(d("2.0"))
        );
        assert_eq!(
            start
                .checked_add_quantity(qty, Side::Sell)
                .expect("must be valid"),
            PositionSize::new(d("1.0"))
        );
    }

    #[test]
    fn add_with_quantity_flips_position_sign() {
        let short = PositionSize::new(d("-1.5"));
        let qty = Quantity::new(d("2.0")).expect("must be valid");

        let result: PositionSize = short
            .checked_add_quantity(qty, Side::Buy)
            .expect("must be valid");
        assert_eq!(result, PositionSize::new(d("0.5")));

        let long = PositionSize::new(d("1.0"));
        let result = long
            .checked_add_quantity(qty, Side::Sell)
            .expect("must be valid");
        assert_eq!(result, PositionSize::new(d("-1.0")));

        let zero_qty = Quantity::ZERO;
        let result = short
            .checked_add_quantity(zero_qty, Side::Buy)
            .expect("must be valid");
        assert_eq!(result, short);
    }

    #[test]
    fn checked_add_quantity_reports_overflow() {
        let position = PositionSize::new(Decimal::MAX);
        let qty = Quantity::from_str("1").expect("must be valid");

        assert_eq!(
            position.checked_add_quantity(qty, Side::Buy),
            Err(Error::Overflow {
                param: ParamKind::PositionSize
            })
        );
    }
}
