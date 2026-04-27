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

use super::{define_signed_value_type, Fee, ParamKind, Pnl, Volume};

define_signed_value_type!(
    /// Cash flow value where positive is inflow and negative is outflow.
    CashFlow,
    ParamKind::CashFlow
);

impl CashFlow {
    /// Creates a cash flow contribution from a P&L value.
    pub fn from_pnl(pnl: Pnl) -> Self {
        Self::new(pnl.to_decimal())
    }

    /// Creates a cash flow contribution from a fee.
    pub fn from_fee(fee: Fee) -> Self {
        Self::new(-fee.to_decimal())
    }

    /// Creates a cash flow contribution from a volume inflow.
    pub fn from_volume_inflow(volume: Volume) -> Self {
        Self::new(volume.to_decimal())
    }

    /// Creates a cash flow contribution from a volume outflow (negates the volume amount).
    pub fn from_volume_outflow(volume: Volume) -> Self {
        Self::new(-volume.to_decimal())
    }
}

#[cfg(test)]
mod tests {
    use super::CashFlow;
    use crate::param::{Fee, Pnl, Volume};
    use rust_decimal::Decimal;

    fn d(value: &str) -> Decimal {
        value
            .parse()
            .expect("decimal literal in tests must be valid")
    }

    #[test]
    fn converts_from_pnl_and_fee() {
        assert_eq!(
            CashFlow::from_pnl(Pnl::new(d("1.25"))).to_decimal(),
            d("1.25")
        );
        assert_eq!(
            CashFlow::from_fee(Fee::new(d("1.25"))).to_decimal(),
            d("-1.25")
        );
    }

    #[test]
    fn converts_from_volume_inflow_and_outflow() {
        assert_eq!(
            CashFlow::from_volume_inflow(Volume::new(d("1.25")).expect("volume must be valid"))
                .to_decimal(),
            d("1.25")
        );
        assert_eq!(
            CashFlow::from_volume_outflow(Volume::new(d("1.25")).expect("volume must be valid"))
                .to_decimal(),
            d("-1.25")
        );
    }
}
