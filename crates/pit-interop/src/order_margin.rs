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

//! Runtime wrapper for the order margin group.

use openpit::param::{Asset, Leverage};
use openpit::{HasAutoBorrow, HasOrderCollateralAsset, HasOrderLeverage, RequestFieldAccessError};

/// Populated order-margin group.
///
/// The boolean `auto_borrow` is always set when the group is present.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PopulatedOrderMargin {
    /// Initial leverage for margin orders.
    pub leverage: Option<Leverage>,
    /// Collateral asset for margin orders.
    pub collateral_asset: Option<Asset>,
    /// Whether the exchange may automatically borrow to fund the order.
    pub auto_borrow: bool,
}

impl HasOrderLeverage for PopulatedOrderMargin {
    fn leverage(&self) -> Result<Option<Leverage>, RequestFieldAccessError> {
        Ok(self.leverage)
    }
}

impl HasOrderCollateralAsset for PopulatedOrderMargin {
    fn collateral_asset(&self) -> Result<Option<&Asset>, RequestFieldAccessError> {
        Ok(self.collateral_asset.as_ref())
    }
}

impl HasAutoBorrow for PopulatedOrderMargin {
    fn auto_borrow(&self) -> Result<bool, RequestFieldAccessError> {
        Ok(self.auto_borrow)
    }
}

/// Runtime access to an order's margin group.
///
/// Use [`OrderMarginAccess::Populated`] when the group is present,
/// [`OrderMarginAccess::Absent`] when it is not.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OrderMarginAccess {
    /// The margin group is present.
    Populated(PopulatedOrderMargin),
    /// The margin group is absent.
    Absent,
}

impl HasOrderLeverage for OrderMarginAccess {
    fn leverage(&self) -> Result<Option<Leverage>, RequestFieldAccessError> {
        match self {
            Self::Populated(m) => m.leverage(),
            Self::Absent => Ok(None),
        }
    }
}

impl HasOrderCollateralAsset for OrderMarginAccess {
    fn collateral_asset(&self) -> Result<Option<&Asset>, RequestFieldAccessError> {
        match self {
            Self::Populated(m) => m.collateral_asset(),
            Self::Absent => Ok(None),
        }
    }
}

impl HasAutoBorrow for OrderMarginAccess {
    fn auto_borrow(&self) -> Result<bool, RequestFieldAccessError> {
        match self {
            Self::Populated(m) => m.auto_borrow(),
            Self::Absent => Err(RequestFieldAccessError::new("margin.auto_borrow")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openpit::param::Asset;

    #[test]
    fn populated_returns_values() {
        let asset = Asset::new("USD").expect("valid");
        let access = OrderMarginAccess::Populated(PopulatedOrderMargin {
            leverage: None,
            collateral_asset: Some(asset.clone()),
            auto_borrow: true,
        });
        assert_eq!(access.leverage().unwrap(), None);
        assert_eq!(access.collateral_asset().unwrap(), Some(&asset));
        assert!(access.auto_borrow().unwrap());
    }

    #[test]
    fn absent_optional_returns_none() {
        let access = OrderMarginAccess::Absent;
        assert_eq!(access.leverage().unwrap(), None);
        assert_eq!(access.collateral_asset().unwrap(), None);
    }

    #[test]
    fn absent_required_returns_err() {
        let access = OrderMarginAccess::Absent;
        assert!(access.auto_borrow().is_err());
    }
}
