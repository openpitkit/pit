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

//! Runtime wrapper for the account-adjustment operation group.

use openpit::param::{AdjustmentAmount, Asset, Leverage, PositionMode, Price};
use openpit::{
    HasAccountAdjustmentBalance, HasAccountAdjustmentBalanceAverageEntryPrice,
    HasAccountAdjustmentPnlOperation, HasAccountAdjustmentPositionLeverage, HasAverageEntryPrice,
    HasBalanceAsset, HasCollateralAsset, HasPositionInstrument, HasPositionMode, Instrument,
    PnlState, RequestFieldAccessError,
};

/// Populated balance-adjustment operation with individually-optional fields.
///
/// Each field is stored as [`Option`]. A `Some` value returns `Ok`; a `None`
/// required field returns `Err(RequestFieldAccessError)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PopulatedBalanceOperation {
    /// Balance asset (required).
    pub asset: Option<Asset>,
    /// Average entry price (optional, denominated in the account currency).
    pub average_entry_price: Option<Price>,
    /// Realized PnL correction (optional, denominated in the account currency).
    pub realized_pnl: Option<PnlState>,
}

/// Populated position-adjustment operation with individually-optional fields.
///
/// Each field is stored as [`Option`]. A `Some` value returns `Ok`; a `None`
/// required field returns `Err(RequestFieldAccessError)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PopulatedPositionOperation {
    /// Position instrument (required).
    pub instrument: Option<Instrument>,
    /// Position collateral asset (required).
    pub collateral_asset: Option<Asset>,
    /// Position average entry price (required).
    pub average_entry_price: Option<Price>,
    /// Position mode (required).
    pub mode: Option<PositionMode>,
    /// Leverage (optional).
    pub leverage: Option<Leverage>,
}

/// Populated account-wide PnL adjustment operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PopulatedAccountPnlOperation {
    /// Replacement PnL state.
    pub state: PnlState,
}

/// Populated account-adjustment operation group.
///
/// The `Balance` variant carries a balance-adjustment payload;
/// the `Position` variant carries a position-adjustment payload.
/// Optional fields that are not present return `Ok(None)`; required fields
/// that do not apply to a given variant return `Err`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PopulatedAccountAdjustmentOperation {
    /// Physical-balance adjustment operation.
    Balance(PopulatedBalanceOperation),
    /// Derivatives-position adjustment operation.
    Position(PopulatedPositionOperation),
    /// Account-wide realized-PnL adjustment operation.
    AccountPnl(PopulatedAccountPnlOperation),
}

impl HasAccountAdjustmentBalance for PopulatedAccountAdjustmentOperation {
    fn balance(&self) -> Result<Option<AdjustmentAmount>, RequestFieldAccessError> {
        Ok(None)
    }

    fn balance_realized_pnl(&self) -> Result<Option<PnlState>, RequestFieldAccessError> {
        match self {
            Self::Balance(operation) => Ok(operation.realized_pnl),
            Self::Position(_) | Self::AccountPnl(_) => Ok(None),
        }
    }
}

impl HasBalanceAsset for PopulatedAccountAdjustmentOperation {
    fn balance_asset(&self) -> Result<&Asset, RequestFieldAccessError> {
        match self {
            Self::Balance(op) => op
                .asset
                .as_ref()
                .ok_or_else(|| RequestFieldAccessError::new("operation.balance_asset")),
            Self::Position(_) | Self::AccountPnl(_) => {
                Err(RequestFieldAccessError::new("operation.balance_asset"))
            }
        }
    }
}

impl HasAccountAdjustmentBalanceAverageEntryPrice for PopulatedAccountAdjustmentOperation {
    fn balance_average_entry_price(&self) -> Result<Option<Price>, RequestFieldAccessError> {
        match self {
            Self::Balance(operation) => Ok(operation.average_entry_price),
            Self::Position(_) | Self::AccountPnl(_) => Ok(None),
        }
    }
}

impl HasPositionInstrument for PopulatedAccountAdjustmentOperation {
    fn position_instrument(&self) -> Result<&Instrument, RequestFieldAccessError> {
        match self {
            Self::Position(op) => op
                .instrument
                .as_ref()
                .ok_or_else(|| RequestFieldAccessError::new("operation.position_instrument")),
            Self::Balance(_) | Self::AccountPnl(_) => Err(RequestFieldAccessError::new(
                "operation.position_instrument",
            )),
        }
    }
}

impl HasCollateralAsset for PopulatedAccountAdjustmentOperation {
    fn collateral_asset(&self) -> Result<&Asset, RequestFieldAccessError> {
        match self {
            Self::Position(op) => op
                .collateral_asset
                .as_ref()
                .ok_or_else(|| RequestFieldAccessError::new("operation.collateral_asset")),
            Self::Balance(_) | Self::AccountPnl(_) => {
                Err(RequestFieldAccessError::new("operation.collateral_asset"))
            }
        }
    }
}

impl HasAverageEntryPrice for PopulatedAccountAdjustmentOperation {
    fn average_entry_price(&self) -> Result<Price, RequestFieldAccessError> {
        match self {
            Self::Position(op) => op
                .average_entry_price
                .ok_or_else(|| RequestFieldAccessError::new("operation.average_entry_price")),
            Self::Balance(_) | Self::AccountPnl(_) => Err(RequestFieldAccessError::new(
                "operation.average_entry_price",
            )),
        }
    }
}

impl HasPositionMode for PopulatedAccountAdjustmentOperation {
    fn position_mode(&self) -> Result<PositionMode, RequestFieldAccessError> {
        match self {
            Self::Position(op) => op
                .mode
                .ok_or_else(|| RequestFieldAccessError::new("operation.position_mode")),
            Self::Balance(_) | Self::AccountPnl(_) => {
                Err(RequestFieldAccessError::new("operation.position_mode"))
            }
        }
    }
}

impl HasAccountAdjustmentPositionLeverage for PopulatedAccountAdjustmentOperation {
    fn position_leverage(&self) -> Result<Option<Leverage>, RequestFieldAccessError> {
        match self {
            Self::Position(op) => Ok(op.leverage),
            Self::Balance(_) | Self::AccountPnl(_) => Ok(None),
        }
    }
}

impl HasAccountAdjustmentPnlOperation for PopulatedAccountAdjustmentOperation {
    fn account_adjustment_pnl_operation(
        &self,
    ) -> Result<Option<PnlState>, RequestFieldAccessError> {
        match self {
            Self::Balance(_) | Self::Position(_) => Ok(None),
            Self::AccountPnl(operation) => Ok(Some(operation.state)),
        }
    }
}

/// Runtime access to an account adjustment's operation group.
///
/// Use [`AccountAdjustmentOperationAccess::Populated`] when the group is
/// present, [`AccountAdjustmentOperationAccess::Absent`] when it is not.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AccountAdjustmentOperationAccess {
    /// The operation group is present.
    Populated(PopulatedAccountAdjustmentOperation),
    /// The operation group is absent.
    Absent,
}

impl HasAccountAdjustmentBalance for AccountAdjustmentOperationAccess {
    fn balance(&self) -> Result<Option<AdjustmentAmount>, RequestFieldAccessError> {
        Ok(None)
    }

    fn balance_realized_pnl(&self) -> Result<Option<PnlState>, RequestFieldAccessError> {
        match self {
            Self::Populated(operation) => operation.balance_realized_pnl(),
            Self::Absent => Ok(None),
        }
    }
}

impl HasBalanceAsset for AccountAdjustmentOperationAccess {
    fn balance_asset(&self) -> Result<&Asset, RequestFieldAccessError> {
        match self {
            Self::Populated(op) => op.balance_asset(),
            Self::Absent => Err(RequestFieldAccessError::new("operation.balance_asset")),
        }
    }
}

impl HasAccountAdjustmentBalanceAverageEntryPrice for AccountAdjustmentOperationAccess {
    fn balance_average_entry_price(&self) -> Result<Option<Price>, RequestFieldAccessError> {
        match self {
            Self::Populated(operation) => operation.balance_average_entry_price(),
            Self::Absent => Ok(None),
        }
    }
}

impl HasPositionInstrument for AccountAdjustmentOperationAccess {
    fn position_instrument(&self) -> Result<&Instrument, RequestFieldAccessError> {
        match self {
            Self::Populated(op) => op.position_instrument(),
            Self::Absent => Err(RequestFieldAccessError::new(
                "operation.position_instrument",
            )),
        }
    }
}

impl HasCollateralAsset for AccountAdjustmentOperationAccess {
    fn collateral_asset(&self) -> Result<&Asset, RequestFieldAccessError> {
        match self {
            Self::Populated(op) => op.collateral_asset(),
            Self::Absent => Err(RequestFieldAccessError::new("operation.collateral_asset")),
        }
    }
}

impl HasAverageEntryPrice for AccountAdjustmentOperationAccess {
    fn average_entry_price(&self) -> Result<Price, RequestFieldAccessError> {
        match self {
            Self::Populated(op) => op.average_entry_price(),
            Self::Absent => Err(RequestFieldAccessError::new(
                "operation.average_entry_price",
            )),
        }
    }
}

impl HasPositionMode for AccountAdjustmentOperationAccess {
    fn position_mode(&self) -> Result<PositionMode, RequestFieldAccessError> {
        match self {
            Self::Populated(op) => op.position_mode(),
            Self::Absent => Err(RequestFieldAccessError::new("operation.position_mode")),
        }
    }
}

impl HasAccountAdjustmentPositionLeverage for AccountAdjustmentOperationAccess {
    fn position_leverage(&self) -> Result<Option<Leverage>, RequestFieldAccessError> {
        match self {
            Self::Populated(op) => op.position_leverage(),
            Self::Absent => Ok(None),
        }
    }
}

impl HasAccountAdjustmentPnlOperation for AccountAdjustmentOperationAccess {
    fn account_adjustment_pnl_operation(
        &self,
    ) -> Result<Option<PnlState>, RequestFieldAccessError> {
        match self {
            Self::Populated(operation) => operation.account_adjustment_pnl_operation(),
            Self::Absent => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openpit::param::{Asset, PositionMode, Price};
    use openpit::{Instrument, PnlHaltReason};

    fn balance_op() -> PopulatedBalanceOperation {
        PopulatedBalanceOperation {
            asset: Some(Asset::new("USD").expect("valid")),
            average_entry_price: Some(Price::from_str("1.25").expect("valid")),
            realized_pnl: Some(PnlState::Halted(PnlHaltReason::MissingFx)),
        }
    }

    fn position_op() -> PopulatedPositionOperation {
        PopulatedPositionOperation {
            instrument: Some(Instrument::new(
                Asset::new("SPX").expect("valid"),
                Asset::new("USD").expect("valid"),
            )),
            collateral_asset: Some(Asset::new("EUR").expect("valid")),
            average_entry_price: Some(Price::from_str("50000").expect("valid")),
            mode: Some(PositionMode::Netting),
            leverage: None,
        }
    }

    fn account_pnl_op() -> PopulatedAccountPnlOperation {
        PopulatedAccountPnlOperation {
            state: PnlState::Halted(PnlHaltReason::MissingFx),
        }
    }

    #[test]
    fn balance_variant_returns_balance_fields() {
        let operation = balance_op();
        let average_entry_price = operation.average_entry_price;
        let realized_pnl = operation.realized_pnl;
        let access = AccountAdjustmentOperationAccess::Populated(
            PopulatedAccountAdjustmentOperation::Balance(operation),
        );
        assert_eq!(access.balance(), Ok(None));
        assert_eq!(
            access.balance_average_entry_price(),
            Ok(average_entry_price)
        );
        assert_eq!(access.balance_realized_pnl(), Ok(realized_pnl));
        assert!(access.balance_asset().is_ok());
        assert!(access.position_instrument().is_err());
        assert!(access.collateral_asset().is_err());
        assert!(access.average_entry_price().is_err());
        assert!(access.position_mode().is_err());
        assert_eq!(access.position_leverage(), Ok(None));
    }

    #[test]
    fn position_variant_returns_position_fields() {
        let access = AccountAdjustmentOperationAccess::Populated(
            PopulatedAccountAdjustmentOperation::Position(position_op()),
        );
        assert_eq!(access.balance(), Ok(None));
        assert_eq!(access.balance_realized_pnl(), Ok(None));
        // Not applicable to a position operation, but the field is itself
        // optional, so this is absent rather than an access error.
        assert_eq!(access.balance_average_entry_price(), Ok(None));
        assert!(access.position_instrument().is_ok());
        assert!(access.collateral_asset().is_ok());
        assert!(access.average_entry_price().is_ok());
        assert!(access.position_mode().is_ok());
        assert!(access.position_leverage().is_ok());
        assert!(access.balance_asset().is_err());
    }

    #[test]
    fn balance_missing_required_returns_err() {
        let access = AccountAdjustmentOperationAccess::Populated(
            PopulatedAccountAdjustmentOperation::Balance(PopulatedBalanceOperation {
                asset: None,
                average_entry_price: None,
                realized_pnl: None,
            }),
        );
        assert!(access.balance_asset().is_err());
        assert_eq!(access.balance_average_entry_price(), Ok(None));
        assert_eq!(access.balance_realized_pnl(), Ok(None));
    }

    #[test]
    fn position_missing_required_returns_err() {
        let access = AccountAdjustmentOperationAccess::Populated(
            PopulatedAccountAdjustmentOperation::Position(PopulatedPositionOperation {
                instrument: None,
                collateral_asset: None,
                average_entry_price: None,
                mode: None,
                leverage: None,
            }),
        );
        assert!(access.position_instrument().is_err());
        assert!(access.collateral_asset().is_err());
        assert!(access.average_entry_price().is_err());
        assert!(access.position_mode().is_err());
        assert_eq!(access.position_leverage().unwrap(), None);
        assert_eq!(access.balance_average_entry_price(), Ok(None));
        assert_eq!(access.balance_realized_pnl(), Ok(None));
    }

    #[test]
    fn account_pnl_variant_returns_only_account_pnl_state() {
        let access = AccountAdjustmentOperationAccess::Populated(
            PopulatedAccountAdjustmentOperation::AccountPnl(account_pnl_op()),
        );

        assert_eq!(access.balance(), Ok(None));
        assert_eq!(access.balance_realized_pnl(), Ok(None));
        assert_eq!(access.balance_average_entry_price(), Ok(None));
        assert_eq!(
            access.account_adjustment_pnl_operation(),
            Ok(Some(PnlState::Halted(PnlHaltReason::MissingFx)))
        );
        assert!(access.balance_asset().is_err());
        assert!(access.position_instrument().is_err());
        assert!(access.collateral_asset().is_err());
        assert!(access.average_entry_price().is_err());
        assert!(access.position_mode().is_err());
        assert_eq!(access.position_leverage(), Ok(None));
    }

    #[test]
    fn absent_returns_err_for_required_and_none_for_optional() {
        let access = AccountAdjustmentOperationAccess::Absent;
        assert_eq!(access.balance(), Ok(None));
        assert_eq!(access.balance_realized_pnl(), Ok(None));
        assert_eq!(access.balance_average_entry_price(), Ok(None));
        assert_eq!(access.account_adjustment_pnl_operation(), Ok(None));
        assert!(access.balance_asset().is_err());
        assert!(access.position_instrument().is_err());
        assert!(access.collateral_asset().is_err());
        assert!(access.average_entry_price().is_err());
        assert!(access.position_mode().is_err());
        assert_eq!(access.position_leverage(), Ok(None));
    }
}
