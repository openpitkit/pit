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

use crate::core::request_trait::HasLock;
use crate::impl_request_has_field;
use crate::impl_request_has_field_passthrough;
use crate::param::{AccountId, Fee, Pnl, PositionEffect, PositionSide, Quantity, Side, Trade};
use crate::pretrade::PreTradeLock;

use super::{
    HasAccountId, HasAutoBorrow, HasClosePosition, HasExecutionReportIsTerminal,
    HasExecutionReportLastTrade, HasExecutionReportPositionEffect, HasExecutionReportPositionSide,
    HasFee, HasInstrument, HasLeavesQuantity, HasOrderCollateralAsset, HasOrderLeverage,
    HasOrderPositionSide, HasOrderPrice, HasPnl, HasReduceOnly, HasSide, HasTradeAmount,
    Instrument,
};

//--------------------------------------------------------------------------------------------------

/// Data: main operation parameters reported by the execution.
/// No `#[non_exhaustive]`: these are client-facing convenience structs meant to be constructed via
/// struct literals from external crates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutionReportOperation {
    pub instrument: Instrument,
    pub account_id: AccountId,
    /// Economic direction of the reported execution event.
    pub side: Side,
}

/// Adds main operation parameters reported by the execution.
/// No `#[non_exhaustive]`: these are client-facing convenience structs meant to be constructed via
/// struct literals from external crates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WithExecutionReportOperation<T> {
    pub inner: T,
    pub operation: ExecutionReportOperation,
}

impl_request_has_field!(
    ExecutionReportOperation,
    WithExecutionReportOperation,
    operation,
    HasInstrument, instrument, &Instrument, instrument;
);
impl_request_has_field_passthrough!(
    WithExecutionReportOperation,
    inner,
    HasReduceOnly, reduce_only, bool;
    HasClosePosition, close_position, bool;
    HasAutoBorrow, auto_borrow, bool;
    HasPnl, pnl, Pnl;
    HasFee, fee, Fee;
    HasLeavesQuantity, leaves_quantity, Quantity;
    HasLock, lock, PreTradeLock;
    HasOrderPrice, price, Option<crate::param::Price>;
    HasOrderPositionSide, position_side, Option<PositionSide>;
    HasOrderLeverage, leverage, Option<crate::param::Leverage>;
    HasOrderCollateralAsset, collateral_asset, Option<&crate::param::Asset>;
    HasExecutionReportLastTrade, last_trade, Option<Trade>;
    HasExecutionReportIsTerminal, is_terminal, bool;
    HasExecutionReportPositionEffect, position_effect, Option<PositionEffect>;
    HasExecutionReportPositionSide, position_side, Option<PositionSide>;
);
impl_request_has_field!(
    ExecutionReportOperation,
    WithExecutionReportOperation,
    operation,
    HasAccountId, account_id, AccountId, account_id;
    HasSide, side, Side, side;
);
impl_request_has_field_passthrough!(
    WithFinancialImpact,
    inner,
    HasInstrument, instrument, &Instrument;
);
impl_request_has_field_passthrough!(
    WithFinancialImpact,
    inner,
    HasAccountId, account_id, AccountId;
    HasSide, side, Side;
    HasTradeAmount, trade_amount, crate::param::TradeAmount;
    HasReduceOnly, reduce_only, bool;
    HasClosePosition, close_position, bool;
    HasAutoBorrow, auto_borrow, bool;
    HasLeavesQuantity, leaves_quantity, Quantity;
    HasLock, lock, PreTradeLock;
    HasOrderPrice, price, Option<crate::param::Price>;
    HasOrderPositionSide, position_side, Option<PositionSide>;
    HasOrderLeverage, leverage, Option<crate::param::Leverage>;
    HasOrderCollateralAsset, collateral_asset, Option<&crate::param::Asset>;
    HasExecutionReportLastTrade, last_trade, Option<Trade>;
    HasExecutionReportIsTerminal, is_terminal, bool;
    HasExecutionReportPositionEffect, position_effect, Option<PositionEffect>;
    HasExecutionReportPositionSide, position_side, Option<PositionSide>;
);

//--------------------------------------------------------------------------------------------------

/// Data: financial impact parameters reported by the execution.
/// No `#[non_exhaustive]`: these are client-facing convenience structs meant to be constructed via
/// struct literals from external crates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FinancialImpact {
    /// Realized trading result contributed by this report.
    ///
    /// Positive values for gains, negative values for losses. Fees can be included in this value,
    /// but if included, the included value must be excluded from `fee` value.
    pub pnl: Pnl,
    /// Fee or rebate associated with this report event.
    ///
    /// Negative values for fees, positive values for rebates.
    pub fee: Fee,
}

/// Adds financial impact parameters reported by the execution.
/// No `#[non_exhaustive]`: these are client-facing convenience structs meant to be constructed via
/// struct literals from external crates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WithFinancialImpact<T> {
    pub inner: T,
    pub financial_impact: FinancialImpact,
}

impl_request_has_field!(
    FinancialImpact,
    WithFinancialImpact,
    financial_impact,
    HasPnl, pnl, Pnl, pnl;
    HasFee, fee, Fee, fee;
);
impl_request_has_field_passthrough!(
    WithExecutionReportFillDetails,
    inner,
    HasInstrument, instrument, &Instrument;
);
impl_request_has_field_passthrough!(
    WithExecutionReportFillDetails,
    inner,
    HasAccountId, account_id, AccountId;
    HasSide, side, Side;
    HasTradeAmount, trade_amount, crate::param::TradeAmount;
    HasReduceOnly, reduce_only, bool;
    HasClosePosition, close_position, bool;
    HasAutoBorrow, auto_borrow, bool;
    HasPnl, pnl, Pnl;
    HasFee, fee, Fee;
    HasOrderPrice, price, Option<crate::param::Price>;
    HasOrderPositionSide, position_side, Option<PositionSide>;
    HasOrderLeverage, leverage, Option<crate::param::Leverage>;
    HasOrderCollateralAsset, collateral_asset, Option<&crate::param::Asset>;
    HasExecutionReportPositionEffect, position_effect, Option<PositionEffect>;
    HasExecutionReportPositionSide, position_side, Option<PositionSide>;
);

//--------------------------------------------------------------------------------------------------

/// Data: trade reported by the execution.
/// No `#[non_exhaustive]`: these are client-facing convenience structs meant to be constructed via
/// struct literals from external crates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutionReportFillDetails {
    pub last_trade: Option<Trade>,
    /// Remaining order quantity after this fill.
    pub leaves_quantity: Quantity,
    /// Order lock payload.
    pub lock: PreTradeLock,
    /// Whether this report closes the report stream for the order.
    pub is_terminal: bool,
}

/// Adds financial impact parameters reported by the execution.
/// No `#[non_exhaustive]`: these are client-facing convenience structs meant to be constructed via
/// struct literals from external crates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WithExecutionReportFillDetails<T> {
    pub inner: T,
    pub fill: ExecutionReportFillDetails,
}

impl_request_has_field!(
    ExecutionReportFillDetails,
    WithExecutionReportFillDetails,
    fill,
    HasExecutionReportLastTrade, last_trade, Option<Trade>, last_trade;
    HasLeavesQuantity, leaves_quantity, Quantity, leaves_quantity;
    HasLock, lock, PreTradeLock, lock;
    HasExecutionReportIsTerminal, is_terminal, bool, is_terminal;
);
impl_request_has_field_passthrough!(
    WithExecutionReportPositionImpact,
    inner,
    HasInstrument, instrument, &Instrument;
);
impl_request_has_field_passthrough!(
    WithExecutionReportPositionImpact,
    inner,
    HasAccountId, account_id, AccountId;
    HasSide, side, Side;
    HasTradeAmount, trade_amount, crate::param::TradeAmount;
    HasReduceOnly, reduce_only, bool;
    HasClosePosition, close_position, bool;
    HasAutoBorrow, auto_borrow, bool;
    HasPnl, pnl, Pnl;
    HasFee, fee, Fee;
    HasLeavesQuantity, leaves_quantity, Quantity;
    HasLock, lock, PreTradeLock;
    HasOrderPrice, price, Option<crate::param::Price>;
    HasOrderPositionSide, position_side, Option<PositionSide>;
    HasOrderLeverage, leverage, Option<crate::param::Leverage>;
    HasOrderCollateralAsset, collateral_asset, Option<&crate::param::Asset>;
    HasExecutionReportLastTrade, last_trade, Option<Trade>;
    HasExecutionReportIsTerminal, is_terminal, bool;
);

//--------------------------------------------------------------------------------------------------

/// Data: position impact parameters reported by the execution.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
/// No `#[non_exhaustive]`: these are client-facing convenience structs meant to be constructed via
/// struct literals from external crates.
pub struct ExecutionReportPositionImpact {
    /// Whether this execution opened or closed exposure.
    pub position_effect: Option<PositionEffect>,
    /// Hedge-mode leg affected by this execution, when provided.
    pub position_side: Option<PositionSide>,
}

/// Adds position impact parameters reported by the execution.
/// No `#[non_exhaustive]`: these are client-facing convenience structs meant to be constructed via
/// struct literals from external crates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WithExecutionReportPositionImpact<T> {
    pub inner: T,
    pub position_impact: ExecutionReportPositionImpact,
}

impl_request_has_field!(
    ExecutionReportPositionImpact,
    WithExecutionReportPositionImpact,
    position_impact,
    HasExecutionReportPositionEffect, position_effect, Option<PositionEffect>, position_effect;
    HasExecutionReportPositionSide, position_side, Option<PositionSide>, position_side;
);

//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crate::param::{AccountId, Quantity};
    use crate::pretrade::PreTradeLock;

    use super::{
        ExecutionReportFillDetails, ExecutionReportOperation, WithExecutionReportOperation,
    };

    fn fill() -> ExecutionReportFillDetails {
        ExecutionReportFillDetails {
            last_trade: None,
            leaves_quantity: Quantity::from_str("0").expect("must be valid"),
            lock: PreTradeLock::default(),
            is_terminal: false,
        }
    }

    #[test]
    fn execution_report_operation_account_id_via_has_account_id() {
        use crate::param::Asset;
        use crate::param::Side;
        use crate::{HasAccountId, Instrument};

        let id = AccountId::from_u64(99);
        let op = ExecutionReportOperation {
            instrument: Instrument::new(
                Asset::new("SPX").expect("must be valid"),
                Asset::new("USD").expect("must be valid"),
            ),
            account_id: id,
            side: Side::Sell,
        };
        assert_eq!(op.account_id(), Ok(id));

        let wrapped = WithExecutionReportOperation {
            inner: (),
            operation: op,
        };
        assert_eq!(wrapped.account_id(), Ok(id));
    }

    #[test]
    fn fill_defaults_are_stable() {
        let f = fill();
        assert_eq!(f.last_trade, None);
        assert_eq!(
            f.leaves_quantity,
            Quantity::from_str("0").expect("must be valid")
        );
        assert_eq!(f.lock, PreTradeLock::default());
        assert!(!f.is_terminal);
    }
}
