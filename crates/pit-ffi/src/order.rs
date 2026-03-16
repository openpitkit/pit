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

use openpit::{OrderMargin, OrderOperation, OrderPosition};

/// Order payload used by FFI integrations.
///
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Order {
    /// Main order operation payload.
    pub operation: Option<OrderOperation>,

    /// Derivatives position-management group.
    pub position: Option<OrderPosition>,

    /// Margin-trading extension group.
    pub margin: Option<OrderMargin>,
}

#[cfg(test)]
mod tests {
    use super::Order;
    use openpit::param::{Asset, Leverage, PositionSide, Price, Quantity, Side, TradeAmount};
    use openpit::Instrument;
    use openpit::{OrderMargin, OrderOperation, OrderPosition};

    #[test]
    fn order_exposes_core_and_extensions() {
        let order = Order {
            operation: Some(OrderOperation {
                instrument: Instrument::new(
                    Asset::new("AAPL").expect("asset code must be valid"),
                    Asset::new("USD").expect("asset code must be valid"),
                ),
                side: Side::Buy,
                trade_amount: TradeAmount::Quantity(
                    Quantity::from_str("2").expect("quantity must be valid"),
                ),
                price: Some(Price::from_str("100").expect("price must be valid")),
            }),
            position: Some(OrderPosition {
                position_side: Some(PositionSide::Long),
                reduce_only: true,
                close_position: false,
            }),
            margin: Some(OrderMargin {
                leverage: Some(Leverage::from_u16(20).expect("leverage must be valid")),
                collateral_asset: Some(Asset::new("USD").expect("asset code must be valid")),
                auto_borrow: true,
            }),
        };

        assert_eq!(
            order.operation.expect("operation must be present").side,
            Side::Buy
        );
        assert_eq!(
            order
                .position
                .expect("position must be present")
                .position_side,
            Some(PositionSide::Long)
        );
        assert!(order.margin.expect("margin must be present").auto_borrow);
    }

    #[test]
    fn order_returns_none_for_absent_optional_groups() {
        let order = Order {
            operation: None,
            position: None,
            margin: None,
        };

        assert!(order.operation.is_none());
        assert!(order.position.is_none());
        assert!(order.margin.is_none());
    }
}
