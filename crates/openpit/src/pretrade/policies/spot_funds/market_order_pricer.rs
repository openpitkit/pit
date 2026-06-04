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

use std::fmt::{Display, Formatter};

use rust_decimal::Decimal;

use crate::param::Price;

/// Error returned when an effective price cannot be computed.
///
/// Triggered by decimal overflow in `1 ± factor` or `mark * (…)`,
/// a non-positive mark, or a non-positive sell result after the
/// factor is applied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MarketOrderPriceError {
    /// An arithmetic operation overflowed the decimal range, or the
    /// resulting price was zero or negative.
    PriceUncomputable,
}

impl Display for MarketOrderPriceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PriceUncomputable => f.write_str("price uncomputable"),
        }
    }
}

impl std::error::Error for MarketOrderPriceError {}

/// Pricer that applies a fixed basis-point slippage.
#[derive(Clone)]
pub(super) struct WithSlippage {
    /// Slippage factor = bps / 10_000.
    factor: Decimal,
}

impl WithSlippage {
    /// Creates a pricer with the given slippage in basis points.
    pub(super) fn new(bps: u16) -> Self {
        Self {
            factor: Decimal::from(bps) / Decimal::from(10_000u32),
        }
    }

    /// Effective buy price accounting for slippage (`mark * (1 + factor)`).
    pub(super) fn effective_buy_price(&self, mark: Price) -> Result<Price, MarketOrderPriceError> {
        if mark.to_decimal() <= Decimal::ZERO {
            return Err(MarketOrderPriceError::PriceUncomputable);
        }
        let factor = Decimal::ONE
            .checked_add(self.factor)
            .ok_or(MarketOrderPriceError::PriceUncomputable)?;
        let price = mark
            .to_decimal()
            .checked_mul(factor)
            .ok_or(MarketOrderPriceError::PriceUncomputable)?;
        Ok(Price::new(price))
    }

    /// Effective sell price accounting for slippage (`mark * (1 - factor)`).
    pub(super) fn effective_sell_price(&self, mark: Price) -> Result<Price, MarketOrderPriceError> {
        let factor = Decimal::ONE
            .checked_sub(self.factor)
            .ok_or(MarketOrderPriceError::PriceUncomputable)?;
        let price = mark
            .to_decimal()
            .checked_mul(factor)
            .ok_or(MarketOrderPriceError::PriceUncomputable)?;
        if price <= Decimal::ZERO {
            return Err(MarketOrderPriceError::PriceUncomputable);
        }
        Ok(Price::new(price))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn px(s: &str) -> Price {
        Price::from_str(s).expect("valid price")
    }

    fn pricer(bps: u16) -> WithSlippage {
        WithSlippage::new(bps)
    }

    #[test]
    fn effective_buy_price_rejects_zero_mark() {
        let p = pricer(100);
        assert_eq!(
            p.effective_buy_price(px("0")),
            Err(MarketOrderPriceError::PriceUncomputable)
        );
    }

    #[test]
    fn effective_buy_price_rejects_negative_mark() {
        let p = pricer(100);
        assert_eq!(
            p.effective_buy_price(px("-1")),
            Err(MarketOrderPriceError::PriceUncomputable)
        );
    }

    #[test]
    fn effective_buy_price_applies_slippage() {
        let p = pricer(100); // 1 %
        let result = p
            .effective_buy_price(px("100"))
            .expect("must succeed for positive mark");
        assert_eq!(result, px("101"));
    }

    #[test]
    fn effective_sell_price_rejects_zero_result() {
        // 10 000 bps = 100 % slippage → factor = 0.
        let p = pricer(10_000);
        assert_eq!(
            p.effective_sell_price(px("100")),
            Err(MarketOrderPriceError::PriceUncomputable)
        );
    }
}
