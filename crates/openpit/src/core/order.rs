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

use crate::impl_request_has_field;
use crate::param::{Asset, Leverage, PositionSide, Price, Side, TradeAmount};

use crate::{
    HasAutoBorrow, HasClosePosition, HasInstrument, HasOrderCollateralAsset, HasOrderLeverage,
    HasOrderPositionSide, HasOrderPrice, HasReduceOnly, HasSide, HasTradeAmount, Instrument,
};

//--------------------------------------------------------------------------------------------------

/// Data: main operation parameters that describe side, instrument, price, and amount.
/// No `#[non_exhaustive]`: these are client-facing convenience structs meant to be constructed via
/// struct literals from external crates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderOperation {
    pub instrument: Instrument,
    pub side: Side,
    pub trade_amount: TradeAmount,
    /// Requested worst execution price used for size translation and price-sensitive checks.
    ///
    /// `None` means the order should execute at market price.
    pub price: Option<Price>,
}

/// Adds main operation parameters that describe side, instrument, price, and amount.
/// No `#[non_exhaustive]`: these are client-facing convenience structs meant to be constructed via
/// struct literals from external crates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WithOrderOperation<T> {
    pub inner: T,
    pub operation: OrderOperation,
}

impl_request_has_field!(
    HasInstrument,
    instrument,
    &Instrument,
    OrderOperation,
    instrument,
    WithOrderOperation,
    operation,
);

impl_request_has_field!(
    HasSide,
    side,
    Side,
    OrderOperation,
    side,
    WithOrderOperation,
    operation,
);

impl_request_has_field!(
    HasTradeAmount,
    trade_amount,
    TradeAmount,
    OrderOperation,
    trade_amount,
    WithOrderOperation,
    operation,
);

impl_request_has_field!(
    HasOrderPrice,
    price,
    Option<Price>,
    OrderOperation,
    price,
    WithOrderOperation,
    operation,
);

//--------------------------------------------------------------------------------------------------

/// Data: position management parameters.
/// No `#[non_exhaustive]`: these are client-facing convenience structs meant to be constructed via
/// struct literals from external crates.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OrderPosition {
    /// Hedge-mode leg targeted by the order.
    ///
    /// `Some(...)` selects an explicit long/short leg; `None` uses one-way mode semantics.
    pub position_side: Option<PositionSide>,
    /// Restricts the order to exposure-reducing execution only.
    pub reduce_only: bool,
    /// Marks intent to close the entire open position for the targeted leg/symbol.
    pub close_position: bool,
}

/// Adds position management parameters.
/// No `#[non_exhaustive]`: these are client-facing convenience structs meant to be constructed via
/// struct literals from external crates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WithOrderPosition<T> {
    pub inner: T,
    pub position: OrderPosition,
}

impl_request_has_field!(
    HasOrderPositionSide,
    position_side,
    Option<PositionSide>,
    OrderPosition,
    position_side,
    WithOrderPosition,
    position,
);

impl_request_has_field!(
    HasReduceOnly,
    reduce_only,
    bool,
    OrderPosition,
    reduce_only,
    WithOrderPosition,
    position,
);

impl_request_has_field!(
    HasClosePosition,
    close_position,
    bool,
    OrderPosition,
    close_position,
    WithOrderPosition,
    position,
);

//--------------------------------------------------------------------------------------------------

/// Data: margin configuration parameters.
/// No `#[non_exhaustive]`: these are client-facing convenience structs meant to be constructed via
/// struct literals from external crates.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OrderMargin {
    /// Per-order leverage target used for margin requirement calculation.
    ///
    /// `None` means "use integration/account default leverage configuration".
    pub leverage: Option<Leverage>,
    /// Collateral currency intended to fund this specific order.
    ///
    /// `None` means "use default collateral asset selected by integration".
    pub collateral_asset: Option<Asset>,
    /// Whether temporary collateral shortage may be covered by auto-borrow.
    ///
    /// Defaults to `false`.
    pub auto_borrow: bool,
}

/// Adds margin configuration parameters.
/// No `#[non_exhaustive]`: these are client-facing convenience structs meant to be constructed via
/// struct literals from external crates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WithOrderMargin<T> {
    pub inner: T,
    pub margin: OrderMargin,
}

impl_request_has_field!(
    HasOrderLeverage,
    leverage,
    Option<Leverage>,
    OrderMargin,
    leverage,
    WithOrderMargin,
    margin,
);

impl HasOrderCollateralAsset for OrderMargin {
    fn collateral_asset(&self) -> Option<&Asset> {
        self.collateral_asset.as_ref()
    }
}

impl<T> HasOrderCollateralAsset for WithOrderMargin<T> {
    fn collateral_asset(&self) -> Option<&Asset> {
        self.margin.collateral_asset()
    }
}

impl_request_has_field!(
    HasAutoBorrow,
    auto_borrow,
    bool,
    OrderMargin,
    auto_borrow,
    WithOrderMargin,
    margin,
);

//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crate::param::Asset;

    use super::{HasOrderCollateralAsset, OrderMargin, WithOrderMargin};

    #[test]
    fn collateral_asset_returns_some_when_set() {
        let asset = Asset::new("BTC").expect("must be valid");
        let margin = OrderMargin {
            leverage: None,
            collateral_asset: Some(asset.clone()),
            auto_borrow: false,
        };
        assert_eq!(margin.collateral_asset(), Some(&asset));
    }

    #[test]
    fn collateral_asset_returns_none_when_not_set() {
        let margin = OrderMargin {
            leverage: None,
            collateral_asset: None,
            auto_borrow: false,
        };
        assert_eq!(margin.collateral_asset(), None);
    }

    #[test]
    fn with_order_margin_delegates_collateral_asset() {
        let asset = Asset::new("ETH").expect("must be valid");
        let w = WithOrderMargin {
            inner: (),
            margin: OrderMargin {
                leverage: None,
                collateral_asset: Some(asset.clone()),
                auto_borrow: false,
            },
        };
        assert_eq!(w.collateral_asset(), Some(&asset));
    }

    #[test]
    fn with_order_operation_instrument_delegates_via_ref_arm() {
        use crate::param::{Quantity, Side, TradeAmount};
        use crate::{HasInstrument, Instrument};

        let instrument = Instrument::new(
            Asset::new("BTC").expect("must be valid"),
            Asset::new("USD").expect("must be valid"),
        );
        let w = super::WithOrderOperation {
            inner: (),
            operation: super::OrderOperation {
                instrument: instrument.clone(),
                side: Side::Buy,
                trade_amount: TradeAmount::Quantity(
                    Quantity::from_str("1").expect("must be valid"),
                ),
                price: None,
            },
        };
        assert_eq!(w.instrument(), &instrument);
    }
}
