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

use crate::param::{
    AccountId, Asset, Fee, Leverage, Pnl, PositionEffect, PositionSide, Price, Quantity, Side,
    Trade, TradeAmount,
};
use crate::pretrade::Lock;

use super::Instrument;

/// A macro to generate the trait that requests a specific field from the request.
#[macro_export]
macro_rules! has_request_field_trait {
    (
        $(#[$meta:meta])*
        $trait:ident,
        $method:ident -> $ret:ty
    ) => {
        $(#[$meta])*
        pub trait $trait {
            fn $method(&self) -> $ret;
        }

        impl<T> $trait for T
        where
            T: std::ops::Deref,
            T::Target: $trait,
        {
            fn $method(&self) -> $ret {
                self.deref().$method()
            }
        }
    };
}

has_request_field_trait!(HasAccountId, account_id -> AccountId);

has_request_field_trait!(HasInstrument, instrument -> &Instrument);

has_request_field_trait!(HasSide, side -> Side);

has_request_field_trait!(HasTradeAmount, trade_amount -> TradeAmount);

has_request_field_trait!(HasReduceOnly, reduce_only -> bool);

has_request_field_trait!(HasClosePosition, close_position -> bool);

has_request_field_trait!(HasAutoBorrow, auto_borrow -> bool);

has_request_field_trait!(HasPnl, pnl -> Pnl);

has_request_field_trait!(HasFee, fee -> Fee);

has_request_field_trait!(
    /// Remaining order quantity after the fill.
    HasLeavesQuantity,
    leaves_quantity -> Quantity
);

has_request_field_trait!(
    /// Reservation lock context captured during pre-trade.
    ///
    /// This is not generic user metadata. It is policy-produced context that
    /// must be preserved across the order lifecycle when later execution-report
    /// handling depends on reservation-time details.
    HasLock,
    lock -> Lock
);

has_request_field_trait!(
    /// Requested worst execution price used for size translation and price-sensitive checks.
    ///
    /// `None` means the order should execute at market price.
    HasOrderPrice,
    price -> Option<Price>
);

has_request_field_trait!(HasOrderPositionSide, position_side -> Option<PositionSide>);

has_request_field_trait!(HasOrderLeverage, leverage -> Option<Leverage>);

has_request_field_trait!(HasOrderCollateralAsset, collateral_asset -> Option<&Asset>);

has_request_field_trait!(HasExecutionReportLastTrade, last_trade -> Option<Trade>);

has_request_field_trait!(HasExecutionReportIsTerminal, is_terminal -> bool);

has_request_field_trait!(HasExecutionReportPositionEffect, position_effect -> Option<PositionEffect>);

has_request_field_trait!(HasExecutionReportPositionSide, position_side -> Option<PositionSide>);

#[cfg(test)]
mod tests {
    use super::HasSide;
    use crate::core::order::OrderOperation;
    use crate::param::{Asset, Quantity, Side, TradeAmount};
    use crate::Instrument;

    fn operation() -> OrderOperation {
        use crate::param::AccountId;
        OrderOperation {
            instrument: Instrument::new(
                Asset::new("BTC").expect("must be valid"),
                Asset::new("USD").expect("must be valid"),
            ),
            account_id: AccountId::from_u64(99224416),
            side: Side::Buy,
            trade_amount: TradeAmount::Quantity(Quantity::from_str("1").expect("must be valid")),
            price: None,
        }
    }

    #[test]
    fn deref_dispatch_calls_method_on_target() {
        let boxed: Box<OrderOperation> = Box::new(operation());
        assert_eq!(boxed.side(), Side::Buy);
    }
}
