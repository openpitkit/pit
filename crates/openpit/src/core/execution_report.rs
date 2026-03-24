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
use crate::param::{AccountId, Fee, Pnl, PositionEffect, PositionSide, Quantity, Side, Trade};
use crate::pretrade::Lock;

use super::{
    HasAccountId, HasExecutionReportIsTerminal, HasExecutionReportLastTrade,
    HasExecutionReportPositionEffect, HasExecutionReportPositionSide, HasFee, HasInstrument,
    HasLeavesQuantity, HasPnl, HasSide, Instrument,
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
    HasInstrument,
    instrument,
    &Instrument,
    ExecutionReportOperation,
    instrument,
    WithExecutionReportOperation,
    operation,
);

impl_request_has_field!(
    HasAccountId,
    account_id,
    AccountId,
    ExecutionReportOperation,
    account_id,
    WithExecutionReportOperation,
    operation,
);

impl_request_has_field!(
    HasSide,
    side,
    Side,
    ExecutionReportOperation,
    side,
    WithExecutionReportOperation,
    operation,
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
    HasPnl,
    pnl,
    Pnl,
    FinancialImpact,
    pnl,
    WithFinancialImpact,
    financial_impact,
);
impl_request_has_field!(
    HasFee,
    fee,
    Fee,
    FinancialImpact,
    fee,
    WithFinancialImpact,
    financial_impact,
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
    pub lock: Lock,
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
    HasExecutionReportLastTrade,
    last_trade,
    Option<Trade>,
    ExecutionReportFillDetails,
    last_trade,
    WithExecutionReportFillDetails,
    fill,
);

impl_request_has_field!(
    HasLeavesQuantity,
    leaves_quantity,
    Quantity,
    ExecutionReportFillDetails,
    leaves_quantity,
    WithExecutionReportFillDetails,
    fill,
);

impl_request_has_field!(
    HasLock,
    lock,
    Lock,
    ExecutionReportFillDetails,
    lock,
    WithExecutionReportFillDetails,
    fill,
);

impl_request_has_field!(
    HasExecutionReportIsTerminal,
    is_terminal,
    bool,
    ExecutionReportFillDetails,
    is_terminal,
    WithExecutionReportFillDetails,
    fill,
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
    HasExecutionReportPositionEffect,
    position_effect,
    Option<PositionEffect>,
    ExecutionReportPositionImpact,
    position_effect,
    WithExecutionReportPositionImpact,
    position_impact,
);

impl_request_has_field!(
    HasExecutionReportPositionSide,
    position_side,
    Option<PositionSide>,
    ExecutionReportPositionImpact,
    position_side,
    WithExecutionReportPositionImpact,
    position_impact,
);

//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crate::param::{AccountId, Quantity};
    use crate::pretrade::Lock;

    use super::{
        ExecutionReportFillDetails, ExecutionReportOperation, WithExecutionReportOperation,
    };

    fn fill() -> ExecutionReportFillDetails {
        ExecutionReportFillDetails {
            last_trade: None,
            leaves_quantity: Quantity::from_str("0").expect("must be valid"),
            lock: Lock::default(),
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
                Asset::new("BTC").expect("must be valid"),
                Asset::new("USD").expect("must be valid"),
            ),
            account_id: id,
            side: Side::Sell,
        };
        assert_eq!(op.account_id(), id);

        let wrapped = WithExecutionReportOperation {
            inner: (),
            operation: op,
        };
        assert_eq!(wrapped.account_id(), id);
    }

    #[test]
    fn fill_defaults_are_stable() {
        let f = fill();
        assert_eq!(f.last_trade, None);
        assert_eq!(
            f.leaves_quantity,
            Quantity::from_str("0").expect("must be valid")
        );
        assert_eq!(f.lock, Lock::default());
        assert!(!f.is_terminal);
    }
}
