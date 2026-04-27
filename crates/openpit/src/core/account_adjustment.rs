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

use crate::param::{AdjustmentAmount, Asset, Leverage, PositionMode, PositionSize, Price};
use crate::{impl_request_has_field, impl_request_has_field_passthrough};

use super::{
    HasAccountAdjustmentBalanceAverageEntryPrice, HasAccountAdjustmentPending,
    HasAccountAdjustmentPendingLowerBound, HasAccountAdjustmentPendingUpperBound,
    HasAccountAdjustmentPositionLeverage, HasAccountAdjustmentReserved,
    HasAccountAdjustmentReservedLowerBound, HasAccountAdjustmentReservedUpperBound,
    HasAccountAdjustmentTotal, HasAccountAdjustmentTotalLowerBound,
    HasAccountAdjustmentTotalUpperBound, HasAverageEntryPrice, HasBalanceAsset, HasCollateralAsset,
    HasPositionInstrument, HasPositionMode, Instrument,
};

/// Grouped total/reserved/pending adjustment payload.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AccountAdjustmentAmount {
    /// Actual resulting balance/position value after applying the adjustment.
    pub total: Option<AdjustmentAmount>,
    /// Amount earmarked for outgoing settlement and unavailable for immediate use.
    pub reserved: Option<AdjustmentAmount>,
    /// Amount in-flight for incoming acquisition and not yet finalized.
    pub pending: Option<AdjustmentAmount>,
}

/// Adds grouped total/reserved/pending adjustment payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WithAccountAdjustmentAmount<T> {
    pub inner: T,
    pub amount: AccountAdjustmentAmount,
}

impl_request_has_field!(
    AccountAdjustmentAmount,
    WithAccountAdjustmentAmount,
    amount,
    HasAccountAdjustmentTotal, total, Option<AdjustmentAmount>, total;
    HasAccountAdjustmentReserved, reserved, Option<AdjustmentAmount>, reserved;
    HasAccountAdjustmentPending, pending, Option<AdjustmentAmount>, pending;
);
impl_request_has_field_passthrough!(
    WithAccountAdjustmentAmount,
    inner,
    HasBalanceAsset, balance_asset, &Asset;
    HasAccountAdjustmentBalanceAverageEntryPrice, balance_average_entry_price, Option<Price>;
    HasPositionInstrument, position_instrument, &Instrument;
    HasCollateralAsset, collateral_asset, &Asset;
    HasAverageEntryPrice, average_entry_price, Price;
    HasPositionMode, position_mode, PositionMode;
    HasAccountAdjustmentPositionLeverage, position_leverage, Option<Leverage>;
    HasAccountAdjustmentTotalUpperBound, total_upper, Option<PositionSize>;
    HasAccountAdjustmentTotalLowerBound, total_lower, Option<PositionSize>;
    HasAccountAdjustmentReservedUpperBound, reserved_upper, Option<PositionSize>;
    HasAccountAdjustmentReservedLowerBound, reserved_lower, Option<PositionSize>;
    HasAccountAdjustmentPendingUpperBound, pending_upper, Option<PositionSize>;
    HasAccountAdjustmentPendingLowerBound, pending_lower, Option<PositionSize>;
);

/// Direct adjustment of a physical asset balance without hedge/netting semantics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountAdjustmentBalanceOperation {
    pub asset: Asset,
    /// Optional cost basis for the adjusted physical balance.
    pub average_entry_price: Option<Price>,
}

/// Adds physical-balance adjustment operation payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WithAccountAdjustmentBalanceOperation<T> {
    pub inner: T,
    pub operation: AccountAdjustmentBalanceOperation,
}

impl_request_has_field!(
    AccountAdjustmentBalanceOperation,
    WithAccountAdjustmentBalanceOperation,
    operation,
    HasBalanceAsset, balance_asset, &Asset, asset;
);
impl_request_has_field!(
    AccountAdjustmentBalanceOperation,
    WithAccountAdjustmentBalanceOperation,
    operation,
    HasAccountAdjustmentBalanceAverageEntryPrice, balance_average_entry_price, Option<Price>, average_entry_price;
);
impl_request_has_field_passthrough!(
    WithAccountAdjustmentBalanceOperation,
    inner,
    HasAverageEntryPrice, average_entry_price, Price;
    HasAccountAdjustmentTotal, total, Option<AdjustmentAmount>;
    HasAccountAdjustmentReserved, reserved, Option<AdjustmentAmount>;
    HasAccountAdjustmentPending, pending, Option<AdjustmentAmount>;
    HasAccountAdjustmentTotalUpperBound, total_upper, Option<PositionSize>;
    HasAccountAdjustmentTotalLowerBound, total_lower, Option<PositionSize>;
    HasAccountAdjustmentReservedUpperBound, reserved_upper, Option<PositionSize>;
    HasAccountAdjustmentReservedLowerBound, reserved_lower, Option<PositionSize>;
    HasAccountAdjustmentPendingUpperBound, pending_upper, Option<PositionSize>;
    HasAccountAdjustmentPendingLowerBound, pending_lower, Option<PositionSize>;
    HasPositionInstrument, position_instrument, &Instrument;
    HasCollateralAsset, collateral_asset, &Asset;
    HasPositionMode, position_mode, PositionMode;
    HasAccountAdjustmentPositionLeverage, position_leverage, Option<Leverage>;
);

/// Direct adjustment of a derivatives-like position with explicit position mode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountAdjustmentPositionOperation {
    pub instrument: Instrument,
    /// Asset used to collateralize and settle the adjusted position state.
    ///
    /// This is the margin/collateral bucket affected by the adjustment, not
    /// the traded underlying asset itself.
    pub collateral_asset: Asset,
    /// Average entry price for the adjusted position state.
    pub average_entry_price: Price,
    /// Netting vs hedged position representation.
    pub mode: PositionMode,
    /// Optional leverage snapshot/setting carried with the position adjustment.
    pub leverage: Option<Leverage>,
}

/// Adds derivatives-position adjustment operation payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WithAccountAdjustmentPositionOperation<T> {
    pub inner: T,
    pub operation: AccountAdjustmentPositionOperation,
}

impl_request_has_field!(
    AccountAdjustmentPositionOperation,
    WithAccountAdjustmentPositionOperation,
    operation,
    HasPositionInstrument, position_instrument, &Instrument, instrument;
    HasCollateralAsset, collateral_asset, &Asset, collateral_asset;
);
impl_request_has_field!(
    AccountAdjustmentPositionOperation,
    WithAccountAdjustmentPositionOperation,
    operation,
    HasAverageEntryPrice, average_entry_price, Price, average_entry_price;
    HasPositionMode, position_mode, PositionMode, mode;
    HasAccountAdjustmentPositionLeverage, position_leverage, Option<Leverage>, leverage;
);
impl_request_has_field_passthrough!(
    WithAccountAdjustmentPositionOperation,
    inner,
    HasBalanceAsset, balance_asset, &Asset;
    HasAccountAdjustmentBalanceAverageEntryPrice, balance_average_entry_price, Option<Price>;
    HasAccountAdjustmentTotal, total, Option<AdjustmentAmount>;
    HasAccountAdjustmentReserved, reserved, Option<AdjustmentAmount>;
    HasAccountAdjustmentPending, pending, Option<AdjustmentAmount>;
    HasAccountAdjustmentTotalUpperBound, total_upper, Option<PositionSize>;
    HasAccountAdjustmentTotalLowerBound, total_lower, Option<PositionSize>;
    HasAccountAdjustmentReservedUpperBound, reserved_upper, Option<PositionSize>;
    HasAccountAdjustmentReservedLowerBound, reserved_lower, Option<PositionSize>;
    HasAccountAdjustmentPendingUpperBound, pending_upper, Option<PositionSize>;
    HasAccountAdjustmentPendingLowerBound, pending_lower, Option<PositionSize>;
);

/// Optional post-adjustment inclusive limits for account adjustment components.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AccountAdjustmentBounds {
    /// Allowed post-adjustment inclusive upper bound for total.
    pub total_upper: Option<PositionSize>,
    /// Allowed post-adjustment inclusive lower bound for total.
    pub total_lower: Option<PositionSize>,
    /// Allowed post-adjustment inclusive upper bound for reserved.
    pub reserved_upper: Option<PositionSize>,
    /// Allowed post-adjustment inclusive lower bound for reserved.
    pub reserved_lower: Option<PositionSize>,
    /// Allowed post-adjustment inclusive upper bound for pending.
    pub pending_upper: Option<PositionSize>,
    /// Allowed post-adjustment inclusive lower bound for pending.
    pub pending_lower: Option<PositionSize>,
}

/// Adds post-adjustment inclusive limits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WithAccountAdjustmentBounds<T> {
    pub inner: T,
    pub bounds: AccountAdjustmentBounds,
}

impl_request_has_field!(
    AccountAdjustmentBounds,
    WithAccountAdjustmentBounds,
    bounds,
    HasAccountAdjustmentTotalUpperBound, total_upper, Option<PositionSize>, total_upper;
    HasAccountAdjustmentTotalLowerBound, total_lower, Option<PositionSize>, total_lower;
    HasAccountAdjustmentReservedUpperBound, reserved_upper, Option<PositionSize>, reserved_upper;
    HasAccountAdjustmentReservedLowerBound, reserved_lower, Option<PositionSize>, reserved_lower;
    HasAccountAdjustmentPendingUpperBound, pending_upper, Option<PositionSize>, pending_upper;
    HasAccountAdjustmentPendingLowerBound, pending_lower, Option<PositionSize>, pending_lower;
);
impl_request_has_field_passthrough!(
    WithAccountAdjustmentBounds,
    inner,
    HasBalanceAsset, balance_asset, &Asset;
    HasAccountAdjustmentBalanceAverageEntryPrice, balance_average_entry_price, Option<Price>;
    HasPositionInstrument, position_instrument, &Instrument;
    HasCollateralAsset, collateral_asset, &Asset;
    HasAverageEntryPrice, average_entry_price, Price;
    HasPositionMode, position_mode, PositionMode;
    HasAccountAdjustmentPositionLeverage, position_leverage, Option<Leverage>;
    HasAccountAdjustmentTotal, total, Option<AdjustmentAmount>;
    HasAccountAdjustmentReserved, reserved, Option<AdjustmentAmount>;
    HasAccountAdjustmentPending, pending, Option<AdjustmentAmount>;
);

#[cfg(test)]
mod tests {
    use super::{
        AccountAdjustmentAmount, AccountAdjustmentBalanceOperation, AccountAdjustmentBounds,
        AccountAdjustmentPositionOperation, WithAccountAdjustmentAmount,
        WithAccountAdjustmentBalanceOperation, WithAccountAdjustmentBounds,
        WithAccountAdjustmentPositionOperation,
    };
    use crate::param::{AdjustmentAmount, Asset, Leverage, PositionMode, PositionSize, Price};
    use crate::{
        HasAccountAdjustmentBalanceAverageEntryPrice, HasAccountAdjustmentPending,
        HasAccountAdjustmentPendingLowerBound, HasAccountAdjustmentPendingUpperBound,
        HasAccountAdjustmentPositionLeverage, HasAccountAdjustmentReserved,
        HasAccountAdjustmentReservedLowerBound, HasAccountAdjustmentReservedUpperBound,
        HasAccountAdjustmentTotal, HasAccountAdjustmentTotalLowerBound,
        HasAccountAdjustmentTotalUpperBound, HasAverageEntryPrice, HasBalanceAsset,
        HasCollateralAsset, HasPositionInstrument, HasPositionMode, Instrument,
    };

    #[test]
    fn direct_trait_access_for_balance_operation() {
        let asset = Asset::new("USD").expect("must be valid");
        let average = Price::from_str("1.25").expect("must be valid");
        let operation = AccountAdjustmentBalanceOperation {
            asset: asset.clone(),
            average_entry_price: Some(average),
        };

        assert_eq!(operation.balance_asset(), Ok(&asset));
        assert_eq!(operation.balance_average_entry_price(), Ok(Some(average)));
    }

    #[test]
    fn direct_trait_access_for_position_operation() {
        let instrument = Instrument::new(
            Asset::new("AAPL").expect("must be valid"),
            Asset::new("USD").expect("must be valid"),
        );
        let collateral = Asset::new("USD").expect("must be valid");
        let leverage = Leverage::from_u16(25).expect("must be valid");

        let operation = AccountAdjustmentPositionOperation {
            instrument: instrument.clone(),
            collateral_asset: collateral.clone(),
            average_entry_price: Price::from_str("100").expect("must be valid"),
            mode: PositionMode::Hedged,
            leverage: Some(leverage),
        };

        assert_eq!(operation.position_instrument(), Ok(&instrument));
        assert_eq!(operation.collateral_asset(), Ok(&collateral));
        assert_eq!(
            operation.average_entry_price(),
            Ok(Price::from_str("100").expect("must be valid"))
        );
        assert_eq!(operation.position_mode(), Ok(PositionMode::Hedged));
        assert_eq!(operation.position_leverage(), Ok(Some(leverage)));
    }

    #[test]
    fn direct_trait_access_for_position_operation_fractional_leverage() {
        let leverage = Leverage::from_f64(100.5).expect("must be valid");
        let operation = AccountAdjustmentPositionOperation {
            instrument: Instrument::new(
                Asset::new("AAPL").expect("must be valid"),
                Asset::new("USD").expect("must be valid"),
            ),
            collateral_asset: Asset::new("EUR").expect("must be valid"),
            average_entry_price: Price::from_str("100").expect("must be valid"),
            mode: PositionMode::Hedged,
            leverage: Some(leverage),
        };

        assert_eq!(operation.position_leverage(), Ok(Some(leverage)));
        assert_eq!(leverage.raw(), 1005);
    }

    #[test]
    fn direct_trait_access_for_amount_and_bounds() {
        let total = AdjustmentAmount::Absolute(PositionSize::from_str("4").expect("must be valid"));
        let reserved =
            AdjustmentAmount::Delta(PositionSize::from_str("-1").expect("must be valid"));
        let amount = AccountAdjustmentAmount {
            total: Some(total),
            reserved: Some(reserved),
            pending: None,
        };

        assert_eq!(amount.total(), Ok(Some(total)));
        assert_eq!(amount.reserved(), Ok(Some(reserved)));
        assert_eq!(amount.pending(), Ok(None));

        let bound = PositionSize::from_str("10").expect("must be valid");
        let bounds = AccountAdjustmentBounds {
            total_upper: Some(bound),
            total_lower: None,
            reserved_upper: None,
            reserved_lower: None,
            pending_upper: None,
            pending_lower: None,
        };

        assert_eq!(bounds.total_upper(), Ok(Some(bound)));
        assert_eq!(bounds.pending_lower(), Ok(None));
    }

    #[test]
    fn with_wrappers_preserve_access_chain() {
        let base = WithAccountAdjustmentAmount {
            inner: (),
            amount: AccountAdjustmentAmount {
                total: Some(AdjustmentAmount::Absolute(
                    PositionSize::from_str("7").expect("must be valid"),
                )),
                reserved: None,
                pending: None,
            },
        };

        let with_bounds = WithAccountAdjustmentBounds {
            inner: base,
            bounds: AccountAdjustmentBounds {
                total_upper: Some(PositionSize::from_str("8").expect("must be valid")),
                total_lower: None,
                reserved_upper: None,
                reserved_lower: None,
                pending_upper: None,
                pending_lower: None,
            },
        };

        let with_balance = WithAccountAdjustmentBalanceOperation {
            inner: with_bounds,
            operation: AccountAdjustmentBalanceOperation {
                asset: Asset::new("USD").expect("must be valid"),
                average_entry_price: None,
            },
        };

        assert!(with_balance.total().expect("must be available").is_some());
        assert!(with_balance
            .total_upper()
            .expect("must be available")
            .is_some());
        assert_eq!(with_balance.balance_average_entry_price(), Ok(None));

        let wrapped_position = WithAccountAdjustmentPositionOperation {
            inner: with_balance,
            operation: AccountAdjustmentPositionOperation {
                instrument: Instrument::new(
                    Asset::new("AAPL").expect("must be valid"),
                    Asset::new("USD").expect("must be valid"),
                ),
                collateral_asset: Asset::new("USD").expect("must be valid"),
                average_entry_price: Price::from_str("1").expect("must be valid"),
                mode: PositionMode::Netting,
                leverage: None,
            },
        };

        assert_eq!(wrapped_position.position_mode(), Ok(PositionMode::Netting));
        assert_eq!(
            wrapped_position.average_entry_price(),
            Ok(Price::from_str("1").expect("must be valid"))
        );
        assert_eq!(wrapped_position.balance_average_entry_price(), Ok(None));
        assert_eq!(wrapped_position.position_leverage(), Ok(None));
    }

    #[test]
    fn borrowed_values_come_from_original_fields() {
        let instrument = Instrument::new(
            Asset::new("AAPL").expect("must be valid"),
            Asset::new("USD").expect("must be valid"),
        );
        let collateral = Asset::new("EUR").expect("must be valid");
        let position = AccountAdjustmentPositionOperation {
            instrument: instrument.clone(),
            collateral_asset: collateral.clone(),
            average_entry_price: Price::from_str("10").expect("must be valid"),
            mode: PositionMode::Hedged,
            leverage: None,
        };

        assert_eq!(position.position_instrument(), Ok(&instrument));
        assert_eq!(position.collateral_asset(), Ok(&collateral));

        let balance = AccountAdjustmentBalanceOperation {
            asset: collateral.clone(),
            average_entry_price: None,
        };

        assert_eq!(balance.balance_asset(), Ok(&collateral));
    }

    #[test]
    fn outer_amount_wrapper_passthroughs_position_branch_traits() {
        let instrument = Instrument::new(
            Asset::new("AAPL").expect("must be valid"),
            Asset::new("USD").expect("must be valid"),
        );
        let collateral = Asset::new("EUR").expect("must be valid");
        let average = Price::from_str("123").expect("must be valid");
        let leverage = Leverage::from_u16(10).expect("must be valid");
        let total = AdjustmentAmount::Absolute(PositionSize::from_str("2").expect("must be valid"));
        let pending = AdjustmentAmount::Delta(PositionSize::from_str("1").expect("must be valid"));
        let total_upper = PositionSize::from_str("5").expect("must be valid");
        let pending_lower = PositionSize::from_str("-2").expect("must be valid");
        let pending_upper = PositionSize::from_str("6").expect("must be valid");

        let request = WithAccountAdjustmentAmount {
            inner: WithAccountAdjustmentBounds {
                inner: WithAccountAdjustmentPositionOperation {
                    inner: (),
                    operation: AccountAdjustmentPositionOperation {
                        instrument: instrument.clone(),
                        collateral_asset: collateral.clone(),
                        average_entry_price: average,
                        mode: PositionMode::Hedged,
                        leverage: Some(leverage),
                    },
                },
                bounds: AccountAdjustmentBounds {
                    total_upper: Some(total_upper),
                    total_lower: None,
                    reserved_upper: None,
                    reserved_lower: None,
                    pending_upper: Some(pending_upper),
                    pending_lower: Some(pending_lower),
                },
            },
            amount: AccountAdjustmentAmount {
                total: Some(total),
                reserved: None,
                pending: Some(pending),
            },
        };

        assert_eq!(request.total(), Ok(Some(total)));
        assert_eq!(request.pending(), Ok(Some(pending)));
        assert_eq!(request.position_instrument(), Ok(&instrument));
        assert_eq!(request.collateral_asset(), Ok(&collateral));
        assert_eq!(request.average_entry_price(), Ok(average));
        assert_eq!(request.position_mode(), Ok(PositionMode::Hedged));
        assert_eq!(request.position_leverage(), Ok(Some(leverage)));
        assert_eq!(request.total_upper(), Ok(Some(total_upper)));
        assert_eq!(request.pending_upper(), Ok(Some(pending_upper)));
        assert_eq!(request.pending_lower(), Ok(Some(pending_lower)));
    }

    #[test]
    fn outer_amount_wrapper_passthroughs_balance_branch_traits() {
        let asset = Asset::new("EUR").expect("must be valid");
        let average = Price::from_str("1.12").expect("must be valid");
        let reserved =
            AdjustmentAmount::Delta(PositionSize::from_str("-3").expect("must be valid"));
        let pending =
            AdjustmentAmount::Absolute(PositionSize::from_str("4").expect("must be valid"));
        let total_lower = PositionSize::from_str("-8").expect("must be valid");
        let reserved_upper = PositionSize::from_str("9").expect("must be valid");
        let reserved_lower = PositionSize::from_str("-1").expect("must be valid");

        let request = WithAccountAdjustmentAmount {
            inner: WithAccountAdjustmentBounds {
                inner: WithAccountAdjustmentBalanceOperation {
                    inner: (),
                    operation: AccountAdjustmentBalanceOperation {
                        asset: asset.clone(),
                        average_entry_price: Some(average),
                    },
                },
                bounds: AccountAdjustmentBounds {
                    total_upper: None,
                    total_lower: Some(total_lower),
                    reserved_upper: Some(reserved_upper),
                    reserved_lower: Some(reserved_lower),
                    pending_upper: None,
                    pending_lower: None,
                },
            },
            amount: AccountAdjustmentAmount {
                total: None,
                reserved: Some(reserved),
                pending: Some(pending),
            },
        };

        assert_eq!(request.reserved(), Ok(Some(reserved)));
        assert_eq!(request.pending(), Ok(Some(pending)));
        assert_eq!(request.balance_asset(), Ok(&asset));
        assert_eq!(request.balance_average_entry_price(), Ok(Some(average)));
        assert_eq!(request.total_lower(), Ok(Some(total_lower)));
        assert_eq!(request.reserved_upper(), Ok(Some(reserved_upper)));
        assert_eq!(request.reserved_lower(), Ok(Some(reserved_lower)));
    }
}
