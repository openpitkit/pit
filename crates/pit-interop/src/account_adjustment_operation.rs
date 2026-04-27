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

use openpit::param::{Asset, Leverage, PositionMode, Price};
use openpit::{
    AccountAdjustmentBalanceOperation, AccountAdjustmentPositionOperation,
    HasAccountAdjustmentBalanceAverageEntryPrice, HasAccountAdjustmentPositionLeverage,
    HasAverageEntryPrice, HasBalanceAsset, HasCollateralAsset, HasPositionInstrument,
    HasPositionMode, Instrument, RequestFieldAccessError,
};

/// Populated account-adjustment operation group.
///
/// The `Balance` variant carries a balance-adjustment payload;
/// the `Position` variant carries a position-adjustment payload.
/// Traits that do not apply to a given variant return `Err`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PopulatedAccountAdjustmentOperation {
    /// Physical-balance adjustment operation.
    Balance(AccountAdjustmentBalanceOperation),
    /// Derivatives-position adjustment operation.
    Position(AccountAdjustmentPositionOperation),
}

impl HasBalanceAsset for PopulatedAccountAdjustmentOperation {
    fn balance_asset(&self) -> Result<&Asset, RequestFieldAccessError> {
        match self {
            Self::Balance(op) => Ok(&op.asset),
            Self::Position(_) => Err(RequestFieldAccessError::new("operation.balance_asset")),
        }
    }
}

impl HasAccountAdjustmentBalanceAverageEntryPrice for PopulatedAccountAdjustmentOperation {
    fn balance_average_entry_price(&self) -> Result<Option<Price>, RequestFieldAccessError> {
        match self {
            Self::Balance(op) => Ok(op.average_entry_price),
            Self::Position(_) => Err(RequestFieldAccessError::new(
                "operation.balance_average_entry_price",
            )),
        }
    }
}

impl HasPositionInstrument for PopulatedAccountAdjustmentOperation {
    fn position_instrument(&self) -> Result<&Instrument, RequestFieldAccessError> {
        match self {
            Self::Position(op) => Ok(&op.instrument),
            Self::Balance(_) => Err(RequestFieldAccessError::new(
                "operation.position_instrument",
            )),
        }
    }
}

impl HasCollateralAsset for PopulatedAccountAdjustmentOperation {
    fn collateral_asset(&self) -> Result<&Asset, RequestFieldAccessError> {
        match self {
            Self::Position(op) => Ok(&op.collateral_asset),
            Self::Balance(_) => Err(RequestFieldAccessError::new("operation.collateral_asset")),
        }
    }
}

impl HasAverageEntryPrice for PopulatedAccountAdjustmentOperation {
    fn average_entry_price(&self) -> Result<Price, RequestFieldAccessError> {
        match self {
            Self::Position(op) => Ok(op.average_entry_price),
            Self::Balance(_) => Err(RequestFieldAccessError::new(
                "operation.average_entry_price",
            )),
        }
    }
}

impl HasPositionMode for PopulatedAccountAdjustmentOperation {
    fn position_mode(&self) -> Result<PositionMode, RequestFieldAccessError> {
        match self {
            Self::Position(op) => Ok(op.mode),
            Self::Balance(_) => Err(RequestFieldAccessError::new("operation.position_mode")),
        }
    }
}

impl HasAccountAdjustmentPositionLeverage for PopulatedAccountAdjustmentOperation {
    fn position_leverage(&self) -> Result<Option<Leverage>, RequestFieldAccessError> {
        match self {
            Self::Position(op) => Ok(op.leverage),
            Self::Balance(_) => Err(RequestFieldAccessError::new("operation.position_leverage")),
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
            Self::Populated(op) => op.balance_average_entry_price(),
            Self::Absent => Err(RequestFieldAccessError::new(
                "operation.balance_average_entry_price",
            )),
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
            Self::Absent => Err(RequestFieldAccessError::new("operation.position_leverage")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openpit::param::{Asset, PositionMode};
    use openpit::{
        AccountAdjustmentBalanceOperation, AccountAdjustmentPositionOperation, Instrument,
    };

    fn balance_op() -> AccountAdjustmentBalanceOperation {
        AccountAdjustmentBalanceOperation {
            asset: Asset::new("USD").expect("valid"),
            average_entry_price: None,
        }
    }

    fn position_op() -> AccountAdjustmentPositionOperation {
        use openpit::param::Price;
        AccountAdjustmentPositionOperation {
            instrument: Instrument::new(
                Asset::new("SPX").expect("valid"),
                Asset::new("USD").expect("valid"),
            ),
            collateral_asset: Asset::new("EUR").expect("valid"),
            average_entry_price: Price::from_str("50000").expect("valid"),
            mode: PositionMode::Netting,
            leverage: None,
        }
    }

    #[test]
    fn balance_variant_returns_balance_fields() {
        let access = AccountAdjustmentOperationAccess::Populated(
            PopulatedAccountAdjustmentOperation::Balance(balance_op()),
        );
        assert!(access.balance_asset().is_ok());
        assert!(access.balance_average_entry_price().is_ok());
        assert!(access.position_instrument().is_err());
        assert!(access.collateral_asset().is_err());
        assert!(access.average_entry_price().is_err());
        assert!(access.position_mode().is_err());
        assert!(access.position_leverage().is_err());
    }

    #[test]
    fn position_variant_returns_position_fields() {
        let access = AccountAdjustmentOperationAccess::Populated(
            PopulatedAccountAdjustmentOperation::Position(position_op()),
        );
        assert!(access.position_instrument().is_ok());
        assert!(access.collateral_asset().is_ok());
        assert!(access.average_entry_price().is_ok());
        assert!(access.position_mode().is_ok());
        assert!(access.position_leverage().is_ok());
        assert!(access.balance_asset().is_err());
        assert!(access.balance_average_entry_price().is_err());
    }

    #[test]
    fn absent_returns_err_for_all() {
        let access = AccountAdjustmentOperationAccess::Absent;
        assert!(access.balance_asset().is_err());
        assert!(access.balance_average_entry_price().is_err());
        assert!(access.position_instrument().is_err());
        assert!(access.collateral_asset().is_err());
        assert!(access.average_entry_price().is_err());
        assert!(access.position_mode().is_err());
        assert!(access.position_leverage().is_err());
    }
}
