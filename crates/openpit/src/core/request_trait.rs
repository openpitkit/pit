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
    AccountId, AdjustmentAmount, Asset, Fee, Leverage, Pnl, PositionEffect, PositionMode,
    PositionSide, PositionSize, Price, Quantity, Side, Trade, TradeAmount,
};
use crate::pretrade::PreTradeLock;

use super::Instrument;

/// Returned when a request field could not be delivered to the caller.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestFieldAccessError {
    pub field: &'static str,
}

impl RequestFieldAccessError {
    pub fn new(field: &'static str) -> Self {
        Self { field }
    }
}

impl std::fmt::Display for RequestFieldAccessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "failed to access field '{}'", self.field)
    }
}

impl std::error::Error for RequestFieldAccessError {}

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
            fn $method(
                &self,
            ) -> ::std::result::Result<$ret, $crate::RequestFieldAccessError>;
        }

        impl<T> $trait for T
        where
            T: std::ops::Deref,
            T::Target: $trait,
        {
            fn $method(
                &self,
            ) -> ::std::result::Result<$ret, $crate::RequestFieldAccessError> {
                self.deref().$method()
            }
        }
    };
}

has_request_field_trait!(HasAccountId, account_id -> AccountId);

has_request_field_trait!(HasBalanceAsset, balance_asset -> &Asset);

has_request_field_trait!(HasCollateralAsset, collateral_asset -> &Asset);

has_request_field_trait!(HasInstrument, instrument -> &Instrument);

has_request_field_trait!(HasPositionInstrument, position_instrument -> &Instrument);

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
    lock -> PreTradeLock
);

has_request_field_trait!(
    /// Position accounting mode where exposure is represented either as one net
    /// position or as separate hedged legs.
    HasPositionMode,
    position_mode -> PositionMode
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

has_request_field_trait!(HasExecutionReportIsFinal, is_final -> bool);

has_request_field_trait!(
    HasExecutionReportPositionEffect,
    position_effect -> Option<PositionEffect>
);

has_request_field_trait!(HasExecutionReportPositionSide, position_side -> Option<PositionSide>);

has_request_field_trait!( HasAverageEntryPrice, average_entry_price -> Price);

has_request_field_trait!(
    HasAccountAdjustmentBalanceAverageEntryPrice,
    balance_average_entry_price -> Option<Price>
);

has_request_field_trait!(
    /// Actual resulting balance/position value after applying the adjustment.
    HasAccountAdjustmentTotal,
    total -> Option<AdjustmentAmount>
);

has_request_field_trait!(
    /// Amount earmarked for outgoing settlement and unavailable for immediate use.
    HasAccountAdjustmentReserved,
    reserved -> Option<AdjustmentAmount>
);

has_request_field_trait!(
    /// Amount in-flight for incoming acquisition and not yet finalized.
    HasAccountAdjustmentPending,
    pending -> Option<AdjustmentAmount>
);

has_request_field_trait!(
    /// Allowed post-adjustment range for the corresponding component.
    HasAccountAdjustmentTotalUpperBound,
    total_upper -> Option<PositionSize>
);

has_request_field_trait!(
    /// Allowed post-adjustment range for the corresponding component.
    HasAccountAdjustmentTotalLowerBound,
    total_lower -> Option<PositionSize>
);

has_request_field_trait!(
    /// Allowed post-adjustment range for the corresponding component.
    HasAccountAdjustmentReservedUpperBound,
    reserved_upper -> Option<PositionSize>
);

has_request_field_trait!(
    /// Allowed post-adjustment range for the corresponding component.
    HasAccountAdjustmentReservedLowerBound,
    reserved_lower -> Option<PositionSize>
);

has_request_field_trait!(
    /// Allowed post-adjustment range for the corresponding component.
    HasAccountAdjustmentPendingUpperBound,
    pending_upper -> Option<PositionSize>
);

has_request_field_trait!(
    /// Allowed post-adjustment range for the corresponding component.
    HasAccountAdjustmentPendingLowerBound,
    pending_lower -> Option<PositionSize>
);

has_request_field_trait!(HasAccountAdjustmentPositionLeverage, position_leverage -> Option<Leverage>);

#[cfg(test)]
mod tests {
    use super::{HasSide, RequestFieldAccessError};
    use crate::core::order::OrderOperation;
    use crate::param::{Asset, Quantity, Side, TradeAmount};
    use crate::Instrument;

    fn operation() -> OrderOperation {
        use crate::param::AccountId;
        OrderOperation {
            instrument: Instrument::new(
                Asset::new("SPX").expect("must be valid"),
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
        assert_eq!(boxed.side(), Ok(Side::Buy));
    }

    #[test]
    fn display_is_stable() {
        let err = RequestFieldAccessError::new("instrument");
        assert_eq!(err.to_string(), "failed to access field 'instrument'");
    }

    #[test]
    fn equality() {
        assert_eq!(
            RequestFieldAccessError::new("side"),
            RequestFieldAccessError::new("side")
        );
        assert_ne!(
            RequestFieldAccessError::new("side"),
            RequestFieldAccessError::new("instrument")
        );
    }
}
