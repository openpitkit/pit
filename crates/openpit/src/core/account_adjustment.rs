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
use crate::{impl_request_has_field, impl_request_has_field_passthrough, PnlState};

use super::{
    HasAccountAdjustmentBalance, HasAccountAdjustmentBalanceAverageEntryPrice,
    HasAccountAdjustmentBalanceLowerBound, HasAccountAdjustmentBalanceUpperBound,
    HasAccountAdjustmentHeld, HasAccountAdjustmentHeldLowerBound,
    HasAccountAdjustmentHeldUpperBound, HasAccountAdjustmentIncoming,
    HasAccountAdjustmentIncomingLowerBound, HasAccountAdjustmentIncomingUpperBound,
    HasAccountAdjustmentPnlOperation, HasAccountAdjustmentPositionLeverage, HasAverageEntryPrice,
    HasBalanceAsset, HasCollateralAsset, HasPositionInstrument, HasPositionMode, Instrument,
    RequestFieldAccessError,
};

/// Direct adjustment of the account-wide PnL state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountAdjustmentAccountPnlOperation {
    /// Replacement account-PnL state.
    pub state: PnlState,
}

impl HasAccountAdjustmentPnlOperation for AccountAdjustmentAccountPnlOperation {
    fn account_adjustment_pnl_operation(
        &self,
    ) -> Result<Option<PnlState>, crate::RequestFieldAccessError> {
        Ok(Some(self.state))
    }
}

impl HasAccountAdjustmentBalanceAverageEntryPrice for AccountAdjustmentAccountPnlOperation {
    fn balance_average_entry_price(&self) -> Result<Option<Price>, RequestFieldAccessError> {
        Ok(None)
    }
}

impl HasBalanceAsset for AccountAdjustmentAccountPnlOperation {
    fn balance_asset(&self) -> Result<&Asset, RequestFieldAccessError> {
        Err(RequestFieldAccessError::new("account_pnl.balance_asset"))
    }
}

impl HasAccountAdjustmentBalance for AccountAdjustmentAccountPnlOperation {
    fn balance(&self) -> Result<Option<AdjustmentAmount>, RequestFieldAccessError> {
        Ok(None)
    }
}

impl HasAccountAdjustmentHeld for AccountAdjustmentAccountPnlOperation {
    fn held(&self) -> Result<Option<AdjustmentAmount>, RequestFieldAccessError> {
        Ok(None)
    }
}

impl HasAccountAdjustmentIncoming for AccountAdjustmentAccountPnlOperation {
    fn incoming(&self) -> Result<Option<AdjustmentAmount>, RequestFieldAccessError> {
        Ok(None)
    }
}

impl HasAccountAdjustmentBalanceUpperBound for AccountAdjustmentAccountPnlOperation {
    fn balance_upper(&self) -> Result<Option<PositionSize>, RequestFieldAccessError> {
        Ok(None)
    }
}

impl HasAccountAdjustmentBalanceLowerBound for AccountAdjustmentAccountPnlOperation {
    fn balance_lower(&self) -> Result<Option<PositionSize>, RequestFieldAccessError> {
        Ok(None)
    }
}

impl HasAccountAdjustmentHeldUpperBound for AccountAdjustmentAccountPnlOperation {
    fn held_upper(&self) -> Result<Option<PositionSize>, RequestFieldAccessError> {
        Ok(None)
    }
}

impl HasAccountAdjustmentHeldLowerBound for AccountAdjustmentAccountPnlOperation {
    fn held_lower(&self) -> Result<Option<PositionSize>, RequestFieldAccessError> {
        Ok(None)
    }
}

impl HasAccountAdjustmentIncomingUpperBound for AccountAdjustmentAccountPnlOperation {
    fn incoming_upper(&self) -> Result<Option<PositionSize>, RequestFieldAccessError> {
        Ok(None)
    }
}

impl HasAccountAdjustmentIncomingLowerBound for AccountAdjustmentAccountPnlOperation {
    fn incoming_lower(&self) -> Result<Option<PositionSize>, RequestFieldAccessError> {
        Ok(None)
    }
}

impl HasPositionInstrument for AccountAdjustmentAccountPnlOperation {
    fn position_instrument(&self) -> Result<&Instrument, RequestFieldAccessError> {
        Err(RequestFieldAccessError::new(
            "account_pnl.position_instrument",
        ))
    }
}

impl HasCollateralAsset for AccountAdjustmentAccountPnlOperation {
    fn collateral_asset(&self) -> Result<&Asset, RequestFieldAccessError> {
        Err(RequestFieldAccessError::new("account_pnl.collateral_asset"))
    }
}

impl HasAverageEntryPrice for AccountAdjustmentAccountPnlOperation {
    fn average_entry_price(&self) -> Result<Price, RequestFieldAccessError> {
        Err(RequestFieldAccessError::new(
            "account_pnl.average_entry_price",
        ))
    }
}

impl HasPositionMode for AccountAdjustmentAccountPnlOperation {
    fn position_mode(&self) -> Result<PositionMode, RequestFieldAccessError> {
        Err(RequestFieldAccessError::new("account_pnl.position_mode"))
    }
}

impl HasAccountAdjustmentPositionLeverage for AccountAdjustmentAccountPnlOperation {
    fn position_leverage(&self) -> Result<Option<Leverage>, RequestFieldAccessError> {
        Ok(None)
    }
}

/// Grouped balance/held/incoming adjustment payload.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AccountAdjustmentAmount {
    /// Free (available) funds after applying the adjustment. Written directly
    /// to the `available` bucket of the internal holdings slot; does not
    /// include amounts reserved in `held`.
    ///
    /// For cash accounts this is the spendable cash balance; for instrument
    /// accounts it is the unencumbered position size.
    pub balance: Option<AdjustmentAmount>,

    /// Funds reserved in pending orders and not available for new
    /// commitments. Written to the `held` bucket, which is separate from
    /// `balance`. A manager-initiated adjustment may set `held` to any value,
    /// including negative (indicating a venue-side shortfall).
    ///
    /// This is not a component of `balance`: `balance` and `held` are tracked
    /// independently.
    pub held: Option<AdjustmentAmount>,

    /// Expected future inflow not yet settled into `balance`. Covers both
    /// working buy orders that would add to `balance` on fill and trades
    /// awaiting incoming settlement (T+N).
    pub incoming: Option<AdjustmentAmount>,
}

/// Adds grouped balance/held/incoming adjustment payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WithAccountAdjustmentAmount<T> {
    pub inner: T,
    pub amount: AccountAdjustmentAmount,
}

impl_request_has_field!(
    AccountAdjustmentAmount,
    WithAccountAdjustmentAmount,
    amount,
    HasAccountAdjustmentHeld, held, Option<AdjustmentAmount>, held;
    HasAccountAdjustmentIncoming, incoming, Option<AdjustmentAmount>, incoming;
);
impl HasAccountAdjustmentBalance for AccountAdjustmentAmount {
    fn balance(&self) -> Result<Option<AdjustmentAmount>, RequestFieldAccessError> {
        Ok(self.balance)
    }
}
impl<T> HasAccountAdjustmentBalance for WithAccountAdjustmentAmount<T>
where
    T: HasAccountAdjustmentBalance,
{
    fn balance(&self) -> Result<Option<AdjustmentAmount>, RequestFieldAccessError> {
        Ok(self.amount.balance)
    }

    fn balance_realized_pnl(&self) -> Result<Option<PnlState>, RequestFieldAccessError> {
        self.inner.balance_realized_pnl()
    }
}
impl_request_has_field_passthrough!(
    WithAccountAdjustmentAmount,
    inner,
    HasAccountAdjustmentPnlOperation, account_adjustment_pnl_operation, Option<PnlState>;
    HasBalanceAsset, balance_asset, &Asset;
    HasAccountAdjustmentBalanceAverageEntryPrice, balance_average_entry_price, Option<Price>;
    HasPositionInstrument, position_instrument, &Instrument;
    HasCollateralAsset, collateral_asset, &Asset;
    HasAverageEntryPrice, average_entry_price, Price;
    HasPositionMode, position_mode, PositionMode;
    HasAccountAdjustmentPositionLeverage, position_leverage, Option<Leverage>;
    HasAccountAdjustmentBalanceUpperBound, balance_upper, Option<PositionSize>;
    HasAccountAdjustmentBalanceLowerBound, balance_lower, Option<PositionSize>;
    HasAccountAdjustmentHeldUpperBound, held_upper, Option<PositionSize>;
    HasAccountAdjustmentHeldLowerBound, held_lower, Option<PositionSize>;
    HasAccountAdjustmentIncomingUpperBound, incoming_upper, Option<PositionSize>;
    HasAccountAdjustmentIncomingLowerBound, incoming_lower, Option<PositionSize>;
);

/// Direct adjustment of a physical asset balance without hedge/netting semantics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountAdjustmentBalanceOperation {
    pub asset: Asset,
    /// Optional account-currency cost basis for the adjusted physical balance.
    ///
    /// When present, this force-sets the slot's average entry price to the
    /// caller-supplied account-currency value. `None` leaves the average entry
    /// price untouched; an untracked average remains absent until force-set.
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
impl HasAccountAdjustmentPnlOperation for AccountAdjustmentBalanceOperation {
    fn account_adjustment_pnl_operation(
        &self,
    ) -> Result<Option<PnlState>, crate::RequestFieldAccessError> {
        Ok(None)
    }
}
impl<T> HasAccountAdjustmentPnlOperation for WithAccountAdjustmentBalanceOperation<T> {
    fn account_adjustment_pnl_operation(
        &self,
    ) -> Result<Option<PnlState>, crate::RequestFieldAccessError> {
        self.operation.account_adjustment_pnl_operation()
    }
}
impl_request_has_field!(
    AccountAdjustmentBalanceOperation,
    WithAccountAdjustmentBalanceOperation,
    operation,
    HasAccountAdjustmentBalanceAverageEntryPrice, balance_average_entry_price, Option<Price>, average_entry_price;
);
impl<T> HasAccountAdjustmentBalance for WithAccountAdjustmentBalanceOperation<T>
where
    T: HasAccountAdjustmentBalance,
{
    fn balance(&self) -> Result<Option<AdjustmentAmount>, RequestFieldAccessError> {
        self.inner.balance()
    }

    fn balance_realized_pnl(&self) -> Result<Option<PnlState>, RequestFieldAccessError> {
        self.inner.balance_realized_pnl()
    }
}
impl_request_has_field_passthrough!(
    WithAccountAdjustmentBalanceOperation,
    inner,
    HasAverageEntryPrice, average_entry_price, Price;
    HasAccountAdjustmentHeld, held, Option<AdjustmentAmount>;
    HasAccountAdjustmentIncoming, incoming, Option<AdjustmentAmount>;
    HasAccountAdjustmentBalanceUpperBound, balance_upper, Option<PositionSize>;
    HasAccountAdjustmentBalanceLowerBound, balance_lower, Option<PositionSize>;
    HasAccountAdjustmentHeldUpperBound, held_upper, Option<PositionSize>;
    HasAccountAdjustmentHeldLowerBound, held_lower, Option<PositionSize>;
    HasAccountAdjustmentIncomingUpperBound, incoming_upper, Option<PositionSize>;
    HasAccountAdjustmentIncomingLowerBound, incoming_lower, Option<PositionSize>;
    HasPositionInstrument, position_instrument, &Instrument;
    HasCollateralAsset, collateral_asset, &Asset;
    HasPositionMode, position_mode, PositionMode;
    HasAccountAdjustmentPositionLeverage, position_leverage, Option<Leverage>;
);

/// Adds an optional realized-PnL correction to a balance adjustment.
///
/// This wrapper preserves the wrapped balance fields and adds only the
/// realized-PnL correction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WithAccountAdjustmentBalanceRealizedPnl<T> {
    pub inner: T,
    pub realized_pnl: PnlState,
}

impl<T> HasAccountAdjustmentBalance for WithAccountAdjustmentBalanceRealizedPnl<T>
where
    T: HasAccountAdjustmentBalance,
{
    fn balance(&self) -> Result<Option<AdjustmentAmount>, RequestFieldAccessError> {
        self.inner.balance()
    }

    fn balance_realized_pnl(&self) -> Result<Option<PnlState>, RequestFieldAccessError> {
        Ok(Some(self.realized_pnl))
    }
}

impl_request_has_field_passthrough!(
    WithAccountAdjustmentBalanceRealizedPnl,
    inner,
    HasAccountAdjustmentPnlOperation, account_adjustment_pnl_operation, Option<PnlState>;
    HasBalanceAsset, balance_asset, &Asset;
    HasAccountAdjustmentBalanceAverageEntryPrice, balance_average_entry_price, Option<Price>;
    HasAccountAdjustmentHeld, held, Option<AdjustmentAmount>;
    HasAccountAdjustmentIncoming, incoming, Option<AdjustmentAmount>;
    HasAccountAdjustmentBalanceUpperBound, balance_upper, Option<PositionSize>;
    HasAccountAdjustmentBalanceLowerBound, balance_lower, Option<PositionSize>;
    HasAccountAdjustmentHeldUpperBound, held_upper, Option<PositionSize>;
    HasAccountAdjustmentHeldLowerBound, held_lower, Option<PositionSize>;
    HasAccountAdjustmentIncomingUpperBound, incoming_upper, Option<PositionSize>;
    HasAccountAdjustmentIncomingLowerBound, incoming_lower, Option<PositionSize>;
    HasPositionInstrument, position_instrument, &Instrument;
    HasCollateralAsset, collateral_asset, &Asset;
    HasAverageEntryPrice, average_entry_price, Price;
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
impl HasAccountAdjustmentPnlOperation for AccountAdjustmentPositionOperation {
    fn account_adjustment_pnl_operation(
        &self,
    ) -> Result<Option<PnlState>, crate::RequestFieldAccessError> {
        Ok(None)
    }
}
impl<T> HasAccountAdjustmentPnlOperation for WithAccountAdjustmentPositionOperation<T> {
    fn account_adjustment_pnl_operation(
        &self,
    ) -> Result<Option<PnlState>, crate::RequestFieldAccessError> {
        self.operation.account_adjustment_pnl_operation()
    }
}
impl_request_has_field!(
    AccountAdjustmentPositionOperation,
    WithAccountAdjustmentPositionOperation,
    operation,
    HasAverageEntryPrice, average_entry_price, Price, average_entry_price;
    HasPositionMode, position_mode, PositionMode, mode;
    HasAccountAdjustmentPositionLeverage, position_leverage, Option<Leverage>, leverage;
);
impl<T> HasAccountAdjustmentBalance for WithAccountAdjustmentPositionOperation<T>
where
    T: HasAccountAdjustmentBalance,
{
    fn balance(&self) -> Result<Option<AdjustmentAmount>, RequestFieldAccessError> {
        self.inner.balance()
    }

    fn balance_realized_pnl(&self) -> Result<Option<PnlState>, RequestFieldAccessError> {
        self.inner.balance_realized_pnl()
    }
}
impl_request_has_field_passthrough!(
    WithAccountAdjustmentPositionOperation,
    inner,
    HasBalanceAsset, balance_asset, &Asset;
    HasAccountAdjustmentBalanceAverageEntryPrice, balance_average_entry_price, Option<Price>;
    HasAccountAdjustmentHeld, held, Option<AdjustmentAmount>;
    HasAccountAdjustmentIncoming, incoming, Option<AdjustmentAmount>;
    HasAccountAdjustmentBalanceUpperBound, balance_upper, Option<PositionSize>;
    HasAccountAdjustmentBalanceLowerBound, balance_lower, Option<PositionSize>;
    HasAccountAdjustmentHeldUpperBound, held_upper, Option<PositionSize>;
    HasAccountAdjustmentHeldLowerBound, held_lower, Option<PositionSize>;
    HasAccountAdjustmentIncomingUpperBound, incoming_upper, Option<PositionSize>;
    HasAccountAdjustmentIncomingLowerBound, incoming_lower, Option<PositionSize>;
);

/// Optional post-adjustment inclusive limits for account adjustment components.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AccountAdjustmentBounds {
    /// Allowed post-adjustment inclusive upper bound for balance.
    pub balance_upper: Option<PositionSize>,
    /// Allowed post-adjustment inclusive lower bound for balance.
    pub balance_lower: Option<PositionSize>,
    /// Allowed post-adjustment inclusive upper bound for held.
    pub held_upper: Option<PositionSize>,
    /// Allowed post-adjustment inclusive lower bound for held.
    pub held_lower: Option<PositionSize>,
    /// Allowed post-adjustment inclusive upper bound for incoming.
    pub incoming_upper: Option<PositionSize>,
    /// Allowed post-adjustment inclusive lower bound for incoming.
    pub incoming_lower: Option<PositionSize>,
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
    HasAccountAdjustmentBalanceUpperBound, balance_upper, Option<PositionSize>, balance_upper;
    HasAccountAdjustmentBalanceLowerBound, balance_lower, Option<PositionSize>, balance_lower;
    HasAccountAdjustmentHeldUpperBound, held_upper, Option<PositionSize>, held_upper;
    HasAccountAdjustmentHeldLowerBound, held_lower, Option<PositionSize>, held_lower;
    HasAccountAdjustmentIncomingUpperBound, incoming_upper, Option<PositionSize>, incoming_upper;
    HasAccountAdjustmentIncomingLowerBound, incoming_lower, Option<PositionSize>, incoming_lower;
);
impl_request_has_field_passthrough!(
    WithAccountAdjustmentBounds,
    inner,
    HasAccountAdjustmentPnlOperation, account_adjustment_pnl_operation, Option<PnlState>;
    HasBalanceAsset, balance_asset, &Asset;
    HasAccountAdjustmentBalanceAverageEntryPrice, balance_average_entry_price, Option<Price>;
    HasPositionInstrument, position_instrument, &Instrument;
    HasCollateralAsset, collateral_asset, &Asset;
    HasAverageEntryPrice, average_entry_price, Price;
    HasPositionMode, position_mode, PositionMode;
    HasAccountAdjustmentPositionLeverage, position_leverage, Option<Leverage>;
    HasAccountAdjustmentHeld, held, Option<AdjustmentAmount>;
    HasAccountAdjustmentIncoming, incoming, Option<AdjustmentAmount>;
);
impl<T> HasAccountAdjustmentBalance for WithAccountAdjustmentBounds<T>
where
    T: HasAccountAdjustmentBalance,
{
    fn balance(&self) -> Result<Option<AdjustmentAmount>, RequestFieldAccessError> {
        self.inner.balance()
    }

    fn balance_realized_pnl(&self) -> Result<Option<PnlState>, RequestFieldAccessError> {
        self.inner.balance_realized_pnl()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AccountAdjustmentAccountPnlOperation, AccountAdjustmentAmount,
        AccountAdjustmentBalanceOperation, AccountAdjustmentBounds,
        AccountAdjustmentPositionOperation, WithAccountAdjustmentAmount,
        WithAccountAdjustmentBalanceOperation, WithAccountAdjustmentBalanceRealizedPnl,
        WithAccountAdjustmentBounds, WithAccountAdjustmentPositionOperation,
    };
    use crate::param::{AdjustmentAmount, Asset, Leverage, Pnl, PositionMode, PositionSize, Price};
    use crate::{
        HasAccountAdjustmentBalance, HasAccountAdjustmentBalanceAverageEntryPrice,
        HasAccountAdjustmentBalanceLowerBound, HasAccountAdjustmentBalanceUpperBound,
        HasAccountAdjustmentHeld, HasAccountAdjustmentHeldLowerBound,
        HasAccountAdjustmentHeldUpperBound, HasAccountAdjustmentIncoming,
        HasAccountAdjustmentIncomingLowerBound, HasAccountAdjustmentIncomingUpperBound,
        HasAccountAdjustmentPositionLeverage, HasAverageEntryPrice, HasBalanceAsset,
        HasCollateralAsset, HasPositionInstrument, HasPositionMode, Instrument, PnlState,
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
    fn direct_trait_access_for_account_pnl_operation() {
        let operation = AccountAdjustmentAccountPnlOperation {
            state: PnlState::Value(Pnl::ZERO),
        };

        assert_eq!(operation.position_leverage(), Ok(None));
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
        let balance =
            AdjustmentAmount::Absolute(PositionSize::from_str("4").expect("must be valid"));
        let held = AdjustmentAmount::Delta(PositionSize::from_str("-1").expect("must be valid"));
        let amount = AccountAdjustmentAmount {
            balance: Some(balance),
            held: Some(held),
            incoming: None,
        };

        assert_eq!(amount.balance(), Ok(Some(balance)));
        assert_eq!(amount.held(), Ok(Some(held)));
        assert_eq!(amount.incoming(), Ok(None));

        let bound = PositionSize::from_str("10").expect("must be valid");
        let bounds = AccountAdjustmentBounds {
            balance_upper: Some(bound),
            balance_lower: None,
            held_upper: None,
            held_lower: None,
            incoming_upper: None,
            incoming_lower: None,
        };

        assert_eq!(bounds.balance_upper(), Ok(Some(bound)));
        assert_eq!(bounds.incoming_lower(), Ok(None));
    }

    #[test]
    fn nested_realized_pnl_survives_outer_amount_and_bounds_wrappers() {
        let balance =
            AdjustmentAmount::Absolute(PositionSize::from_str("4").expect("must be valid"));
        let request = WithAccountAdjustmentBounds {
            inner: WithAccountAdjustmentAmount {
                inner: WithAccountAdjustmentBalanceRealizedPnl {
                    inner: AccountAdjustmentAmount::default(),
                    realized_pnl: PnlState::Value(Pnl::ZERO),
                },
                amount: AccountAdjustmentAmount {
                    balance: Some(balance),
                    held: None,
                    incoming: None,
                },
            },
            bounds: AccountAdjustmentBounds::default(),
        };

        assert_eq!(request.balance(), Ok(Some(balance)));
        assert_eq!(
            request.balance_realized_pnl(),
            Ok(Some(PnlState::Value(Pnl::ZERO)))
        );
    }

    #[test]
    fn outer_realized_pnl_wrapper_preserves_nested_amount_and_bounds() {
        let balance =
            AdjustmentAmount::Absolute(PositionSize::from_str("4").expect("must be valid"));
        let request = WithAccountAdjustmentBalanceRealizedPnl {
            inner: WithAccountAdjustmentBounds {
                inner: WithAccountAdjustmentAmount {
                    inner: AccountAdjustmentAmount::default(),
                    amount: AccountAdjustmentAmount {
                        balance: Some(balance),
                        held: None,
                        incoming: None,
                    },
                },
                bounds: AccountAdjustmentBounds::default(),
            },
            realized_pnl: PnlState::Value(Pnl::ZERO),
        };

        assert_eq!(request.balance(), Ok(Some(balance)));
        assert_eq!(
            request.balance_realized_pnl(),
            Ok(Some(PnlState::Value(Pnl::ZERO)))
        );
    }

    #[test]
    fn with_wrappers_preserve_access_chain() {
        let base = WithAccountAdjustmentAmount {
            inner: AccountAdjustmentAmount::default(),
            amount: AccountAdjustmentAmount {
                balance: Some(AdjustmentAmount::Absolute(
                    PositionSize::from_str("7").expect("must be valid"),
                )),
                held: None,
                incoming: None,
            },
        };

        let with_bounds = WithAccountAdjustmentBounds {
            inner: base,
            bounds: AccountAdjustmentBounds {
                balance_upper: Some(PositionSize::from_str("8").expect("must be valid")),
                balance_lower: None,
                held_upper: None,
                held_lower: None,
                incoming_upper: None,
                incoming_lower: None,
            },
        };

        let with_balance = WithAccountAdjustmentBalanceOperation {
            inner: with_bounds,
            operation: AccountAdjustmentBalanceOperation {
                asset: Asset::new("USD").expect("must be valid"),
                average_entry_price: None,
            },
        };

        assert!(with_balance.balance().expect("must be available").is_some());
        assert!(with_balance
            .balance_upper()
            .expect("must be available")
            .is_some());

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
        let balance =
            AdjustmentAmount::Absolute(PositionSize::from_str("2").expect("must be valid"));
        let incoming = AdjustmentAmount::Delta(PositionSize::from_str("1").expect("must be valid"));
        let balance_upper = PositionSize::from_str("5").expect("must be valid");
        let incoming_lower = PositionSize::from_str("-2").expect("must be valid");
        let incoming_upper = PositionSize::from_str("6").expect("must be valid");

        let request = WithAccountAdjustmentAmount {
            inner: WithAccountAdjustmentBounds {
                inner: WithAccountAdjustmentPositionOperation {
                    inner: AccountAdjustmentAmount::default(),
                    operation: AccountAdjustmentPositionOperation {
                        instrument: instrument.clone(),
                        collateral_asset: collateral.clone(),
                        average_entry_price: average,
                        mode: PositionMode::Hedged,
                        leverage: Some(leverage),
                    },
                },
                bounds: AccountAdjustmentBounds {
                    balance_upper: Some(balance_upper),
                    balance_lower: None,
                    held_upper: None,
                    held_lower: None,
                    incoming_upper: Some(incoming_upper),
                    incoming_lower: Some(incoming_lower),
                },
            },
            amount: AccountAdjustmentAmount {
                balance: Some(balance),
                held: None,
                incoming: Some(incoming),
            },
        };

        assert_eq!(request.balance(), Ok(Some(balance)));
        assert_eq!(request.incoming(), Ok(Some(incoming)));
        assert_eq!(request.position_instrument(), Ok(&instrument));
        assert_eq!(request.collateral_asset(), Ok(&collateral));
        assert_eq!(request.average_entry_price(), Ok(average));
        assert_eq!(request.position_mode(), Ok(PositionMode::Hedged));
        assert_eq!(request.position_leverage(), Ok(Some(leverage)));
        assert_eq!(request.balance_upper(), Ok(Some(balance_upper)));
        assert_eq!(request.incoming_upper(), Ok(Some(incoming_upper)));
        assert_eq!(request.incoming_lower(), Ok(Some(incoming_lower)));
    }

    #[test]
    fn outer_amount_wrapper_passthroughs_balance_branch_traits() {
        let asset = Asset::new("EUR").expect("must be valid");
        let average = Price::from_str("1.12").expect("must be valid");
        let held = AdjustmentAmount::Delta(PositionSize::from_str("-3").expect("must be valid"));
        let incoming =
            AdjustmentAmount::Absolute(PositionSize::from_str("4").expect("must be valid"));
        let balance_lower = PositionSize::from_str("-8").expect("must be valid");
        let held_upper = PositionSize::from_str("9").expect("must be valid");
        let held_lower = PositionSize::from_str("-1").expect("must be valid");

        let request = WithAccountAdjustmentAmount {
            inner: WithAccountAdjustmentBounds {
                inner: WithAccountAdjustmentBalanceOperation {
                    inner: AccountAdjustmentAmount::default(),
                    operation: AccountAdjustmentBalanceOperation {
                        asset: asset.clone(),
                        average_entry_price: Some(average),
                    },
                },
                bounds: AccountAdjustmentBounds {
                    balance_upper: None,
                    balance_lower: Some(balance_lower),
                    held_upper: Some(held_upper),
                    held_lower: Some(held_lower),
                    incoming_upper: None,
                    incoming_lower: None,
                },
            },
            amount: AccountAdjustmentAmount {
                balance: None,
                held: Some(held),
                incoming: Some(incoming),
            },
        };

        assert_eq!(request.held(), Ok(Some(held)));
        assert_eq!(request.incoming(), Ok(Some(incoming)));
        assert_eq!(request.balance_asset(), Ok(&asset));
        assert_eq!(request.balance_lower(), Ok(Some(balance_lower)));
        assert_eq!(request.held_upper(), Ok(Some(held_upper)));
        assert_eq!(request.held_lower(), Ok(Some(held_lower)));
    }
}
