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
// Please see https://openpit.dev and the OWNERS file for details.

use crate::param::Asset;

/// Trading instrument definition.
///
/// `underlying_asset` is the asset that is actually bought or sold.
/// Order quantity, position size, and exposure are expressed in this asset.
///
/// `settlement_asset` is the asset used for monetary settlement.
/// P&L, fees, and cash flows are expressed in this asset.
///
/// # Examples
///
/// ```text
/// Instrument { underlying_asset: AAPL, settlement_asset: USD }
/// BUY 100 AAPL @ 200
/// -> position changes in AAPL
/// -> cash flow and P&L are in USD
/// ```
///
/// ```text
/// Instrument { underlying_asset: SPX, settlement_asset: USD }
/// -> position is tracked in SPX contracts
/// -> P&L is tracked in USD
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Instrument {
    underlying_asset: Asset,
    settlement_asset: Asset,
}

impl Instrument {
    /// Creates an instrument definition.
    ///
    /// `underlying_asset` is the traded asset (for example `AAPL` or `SPX`).
    /// `settlement_asset` is the asset used for P&L and cash settlement
    /// (for example `USD`).
    pub fn new(underlying_asset: Asset, settlement_asset: Asset) -> Self {
        Self {
            underlying_asset,
            settlement_asset,
        }
    }

    /// Returns the asset that is bought or sold.
    ///
    /// This is the asset in which order quantity and resulting position are
    /// measured.
    ///
    /// # Examples
    ///
    /// ```text
    /// Instrument { underlying_asset: AAPL, settlement_asset: USD }
    /// BUY 100 AAPL @ 200
    /// -> quantity and position are in AAPL
    /// ```
    pub fn underlying_asset(&self) -> &Asset {
        &self.underlying_asset
    }

    /// Returns the asset used for monetary settlement.
    ///
    /// This is the asset in which cash flow, fees, and P&L are measured.
    ///
    /// # Examples
    ///
    /// ```text
    /// Instrument { underlying_asset: AAPL, settlement_asset: USD }
    /// BUY 100 AAPL @ 200
    /// -> cash flow and P&L are in USD
    /// ```
    ///
    /// ```text
    /// Instrument { underlying_asset: SPX, settlement_asset: USD }
    /// -> contracts are in SPX
    /// -> settlement remains in USD
    /// ```
    pub fn settlement_asset(&self) -> &Asset {
        &self.settlement_asset
    }
}

#[cfg(test)]
mod tests {
    use crate::param::Asset;

    use super::Instrument;

    #[test]
    fn instrument_accessors_return_original_assets() {
        let instrument = Instrument::new(
            Asset::new("AAPL").expect("asset code must be valid"),
            Asset::new("USD").expect("asset code must be valid"),
        );

        assert_eq!(
            instrument.underlying_asset(),
            &Asset::new("AAPL").expect("asset code must be valid")
        );
        assert_eq!(
            instrument.settlement_asset(),
            &Asset::new("USD").expect("asset code must be valid")
        );
    }
}
