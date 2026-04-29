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

use crate::param::{AccountId, Asset, Leverage, PositionSide, Price, Side, TradeAmount};
use crate::{impl_request_has_field, impl_request_has_field_passthrough};

use crate::{
    HasAccountId, HasAutoBorrow, HasClosePosition, HasExecutionReportIsFinal,
    HasExecutionReportLastTrade, HasExecutionReportPositionEffect, HasExecutionReportPositionSide,
    HasFee, HasInstrument, HasLeavesQuantity, HasLock, HasOrderCollateralAsset, HasOrderLeverage,
    HasOrderPositionSide, HasOrderPrice, HasPnl, HasReduceOnly, HasSide, HasTradeAmount,
    Instrument, RequestFieldAccessError,
};

//--------------------------------------------------------------------------------------------------

/// Data: main operation parameters that describe side, instrument, price, and amount.
/// No `#[non_exhaustive]`: these are client-facing convenience structs meant to be constructed via
/// struct literals from external crates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderOperation {
    pub instrument: Instrument,
    pub account_id: AccountId,
    pub trade_amount: TradeAmount,
    /// Requested worst execution price used for size translation and price-sensitive checks.
    ///
    /// `None` means the order should execute at market price.
    pub price: Option<Price>,
    pub side: Side,
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
    OrderOperation,
    WithOrderOperation,
    operation,
    HasInstrument, instrument, &Instrument, instrument;
);
impl_request_has_field_passthrough!(
    WithOrderOperation,
    inner,
    HasReduceOnly, reduce_only, bool;
    HasClosePosition, close_position, bool;
    HasAutoBorrow, auto_borrow, bool;
    HasPnl, pnl, crate::param::Pnl;
    HasFee, fee, crate::param::Fee;
    HasLeavesQuantity, leaves_quantity, crate::param::Quantity;
    HasLock, lock, crate::pretrade::PreTradeLock;
    HasOrderPositionSide, position_side, Option<PositionSide>;
    HasOrderLeverage, leverage, Option<Leverage>;
    HasOrderCollateralAsset, collateral_asset, Option<&Asset>;
    HasExecutionReportLastTrade, last_trade, Option<crate::param::Trade>;
    HasExecutionReportIsFinal, is_final, bool;
    HasExecutionReportPositionEffect, position_effect, Option<crate::param::PositionEffect>;
    HasExecutionReportPositionSide, position_side, Option<PositionSide>;
);
impl_request_has_field!(
    OrderOperation,
    WithOrderOperation,
    operation,
    HasAccountId, account_id, AccountId, account_id;
    HasTradeAmount, trade_amount, TradeAmount, trade_amount;
    HasOrderPrice, price, Option<Price>, price;
    HasSide, side, Side, side;
);
impl_request_has_field_passthrough!(
    WithOrderPosition,
    inner,
    HasInstrument, instrument, &Instrument;
);
impl_request_has_field_passthrough!(
    WithOrderPosition,
    inner,
    HasAccountId, account_id, AccountId;
    HasTradeAmount, trade_amount, TradeAmount;
    HasOrderPrice, price, Option<Price>;
    HasSide, side, Side;
    HasAutoBorrow, auto_borrow, bool;
    HasPnl, pnl, crate::param::Pnl;
    HasFee, fee, crate::param::Fee;
    HasLeavesQuantity, leaves_quantity, crate::param::Quantity;
    HasLock, lock, crate::pretrade::PreTradeLock;
    HasOrderLeverage, leverage, Option<Leverage>;
    HasOrderCollateralAsset, collateral_asset, Option<&Asset>;
    HasExecutionReportLastTrade, last_trade, Option<crate::param::Trade>;
    HasExecutionReportIsFinal, is_final, bool;
    HasExecutionReportPositionEffect, position_effect, Option<crate::param::PositionEffect>;
    HasExecutionReportPositionSide, position_side, Option<PositionSide>;
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
    OrderPosition,
    WithOrderPosition,
    position,
    HasOrderPositionSide, position_side, Option<PositionSide>, position_side;
    HasReduceOnly, reduce_only, bool, reduce_only;
    HasClosePosition, close_position, bool, close_position;
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

impl HasOrderCollateralAsset for OrderMargin {
    fn collateral_asset(&self) -> Result<Option<&Asset>, RequestFieldAccessError> {
        Ok(self.collateral_asset.as_ref())
    }
}

impl<T> HasOrderCollateralAsset for WithOrderMargin<T> {
    fn collateral_asset(&self) -> Result<Option<&Asset>, RequestFieldAccessError> {
        self.margin.collateral_asset()
    }
}

impl_request_has_field!(
    OrderMargin,
    WithOrderMargin,
    margin,
    HasOrderLeverage, leverage, Option<Leverage>, leverage;
    HasAutoBorrow, auto_borrow, bool, auto_borrow;
);
impl_request_has_field_passthrough!(
    WithOrderMargin,
    inner,
    HasInstrument, instrument, &Instrument;
);
impl_request_has_field_passthrough!(
    WithOrderMargin,
    inner,
    HasAccountId, account_id, AccountId;
    HasTradeAmount, trade_amount, TradeAmount;
    HasOrderPrice, price, Option<Price>;
    HasSide, side, Side;
    HasOrderPositionSide, position_side, Option<PositionSide>;
    HasReduceOnly, reduce_only, bool;
    HasClosePosition, close_position, bool;
    HasPnl, pnl, crate::param::Pnl;
    HasFee, fee, crate::param::Fee;
    HasLeavesQuantity, leaves_quantity, crate::param::Quantity;
    HasLock, lock, crate::pretrade::PreTradeLock;
    HasExecutionReportLastTrade, last_trade, Option<crate::param::Trade>;
    HasExecutionReportIsFinal, is_final, bool;
    HasExecutionReportPositionEffect, position_effect, Option<crate::param::PositionEffect>;
    HasExecutionReportPositionSide, position_side, Option<PositionSide>;
);

//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crate::param::Asset;

    use super::{HasOrderCollateralAsset, OrderMargin, WithOrderMargin};

    #[test]
    fn collateral_asset_returns_some_when_set() {
        let asset = Asset::new("SPX").expect("must be valid");
        let margin = OrderMargin {
            leverage: None,
            collateral_asset: Some(asset.clone()),
            auto_borrow: false,
        };
        assert_eq!(margin.collateral_asset(), Ok(Some(&asset)));
    }

    #[test]
    fn collateral_asset_returns_none_when_not_set() {
        let margin = OrderMargin {
            leverage: None,
            collateral_asset: None,
            auto_borrow: false,
        };
        assert_eq!(margin.collateral_asset(), Ok(None));
    }

    #[test]
    fn with_order_margin_delegates_collateral_asset() {
        let asset = Asset::new("AAPL").expect("must be valid");
        let w = WithOrderMargin {
            inner: (),
            margin: OrderMargin {
                leverage: None,
                collateral_asset: Some(asset.clone()),
                auto_borrow: false,
            },
        };
        assert_eq!(w.collateral_asset(), Ok(Some(&asset)));
    }

    #[test]
    fn with_order_operation_instrument_delegates_via_ref_arm() {
        use crate::param::{AccountId, Quantity, Side, TradeAmount};
        use crate::{HasInstrument, Instrument};

        let instrument = Instrument::new(
            Asset::new("SPX").expect("must be valid"),
            Asset::new("USD").expect("must be valid"),
        );
        let w = super::WithOrderOperation {
            inner: (),
            operation: super::OrderOperation {
                instrument: instrument.clone(),
                account_id: AccountId::from_u64(99224416),
                side: Side::Buy,
                trade_amount: TradeAmount::Quantity(
                    Quantity::from_str("1").expect("must be valid"),
                ),
                price: None,
            },
        };
        assert_eq!(w.instrument(), Ok(&instrument));
    }

    #[test]
    fn order_operation_account_id_via_has_account_id() {
        use crate::param::{AccountId, Quantity, Side, TradeAmount};
        use crate::{HasAccountId, Instrument};

        let id = AccountId::from_u64(42);
        let op = super::OrderOperation {
            instrument: Instrument::new(
                Asset::new("AAPL").expect("must be valid"),
                Asset::new("USD").expect("must be valid"),
            ),
            account_id: id,
            side: Side::Buy,
            trade_amount: TradeAmount::Quantity(Quantity::from_str("1").expect("must be valid")),
            price: None,
        };
        assert_eq!(op.account_id(), Ok(id));

        let wrapped = super::WithOrderOperation {
            inner: (),
            operation: op,
        };
        assert_eq!(wrapped.account_id(), Ok(id));
    }
}
