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

use super::{define_signed_value_type, CashFlow, Fee, ParamKind, PositionSize};

define_signed_value_type!(
    /// Profit and loss (P&L) value that can be positive or negative.
    Pnl,
    ParamKind::Pnl
);

impl Pnl {
    /// Converts P&L into a cash flow contribution.
    pub fn to_cash_flow(self) -> CashFlow {
        CashFlow::from_pnl(self)
    }

    /// Converts P&L into a position size for instruments where P&L is denominated in the base
    /// asset.
    ///
    /// Positive P&L becomes a positive position size (profit increases position),
    /// negative P&L becomes a negative position size (loss decreases position).
    pub fn to_position_size(self) -> super::position_size::PositionSize {
        PositionSize::new(self.to_decimal())
    }

    /// Creates a P&L value from a fee (negates the fee amount).
    pub fn from_fee(fee: Fee) -> Self {
        Self::new(-fee.to_decimal())
    }
}

#[cfg(test)]
mod tests {
    use super::Pnl;
    use crate::param::{CashFlow, Fee, PositionSize};
    use rust_decimal::Decimal;

    fn d(value: &str) -> Decimal {
        value
            .parse()
            .expect("decimal literal in tests must be valid")
    }

    #[test]
    fn converts_to_cash_flow() {
        let pnl = Pnl::new(d("-10"));
        assert_eq!(pnl.to_cash_flow(), CashFlow::new(d("-10")));
    }

    #[test]
    fn converts_to_position_size() {
        let profit = Pnl::new(d("10"));
        let loss = Pnl::new(d("-10"));

        assert_eq!(profit.to_position_size(), PositionSize::new(d("10")));
        assert_eq!(loss.to_position_size(), PositionSize::new(d("-10")));
    }

    #[test]
    fn builds_from_fee() {
        assert_eq!(Pnl::from_fee(Fee::new(d("3.18"))), Pnl::new(d("-3.18")));
    }
}
