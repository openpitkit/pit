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

use super::{define_signed_value_type, CashFlow, ParamKind, Pnl, PositionSize};

define_signed_value_type!(
    /// Fee amount.
    ///
    /// Can be negative when representing rebates or fee adjustments in reconciliation.
    Fee,
    ParamKind::Fee
);

impl Fee {
    /// Converts fee into a negative P&L contribution.
    pub fn to_pnl(self) -> Pnl {
        Pnl::new(-self.to_decimal())
    }

    /// Converts fee into a position size for instruments where fees are paid in the base asset.
    ///
    /// The resulting position size is negative (representing an outflow) for positive fees,
    /// and positive for fee rebates.
    pub fn to_position_size(self) -> PositionSize {
        PositionSize::new(-self.to_decimal())
    }

    /// Converts fee into a cash flow contribution (negates the fee amount).
    pub fn to_cash_flow(self) -> CashFlow {
        CashFlow::from_fee(self)
    }
}

#[cfg(test)]
mod tests {
    use super::Fee;
    use crate::param::{CashFlow, Pnl, PositionSize};
    use rust_decimal::Decimal;

    fn d(value: &str) -> Decimal {
        value
            .parse()
            .expect("decimal literal in tests must be valid")
    }

    #[test]
    fn converts_to_negative_pnl() {
        let fee = Fee::new(d("3.18"));

        assert_eq!(fee.to_pnl(), Pnl::new(d("-3.18")));
    }

    #[test]
    fn converts_to_position_size() {
        let fee = Fee::new(d("2.5"));
        let rebate = Fee::new(d("-2.5"));

        assert_eq!(fee.to_position_size(), PositionSize::new(d("-2.5")));
        assert_eq!(rebate.to_position_size(), PositionSize::new(d("2.5")));
    }

    #[test]
    fn converts_to_cash_flow() {
        let fee = Fee::new(d("3.18"));

        assert_eq!(fee.to_cash_flow(), CashFlow::new(d("-3.18")));
    }
}
