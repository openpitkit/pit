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

//! Pre-trade reservation path for [`SpotFundsPolicy`].

use crate::core::account_outcome::OutcomeAmount;
use crate::core::instrument::Instrument;
use crate::core::sync_mode::SyncMode;
use crate::core::{
    AccountControl, AccountOutcomeEntry, HasAccountId, HasInstrument, HasOrderPrice, HasSide,
    HasTradeAmount,
};
use crate::marketdata::{AccountInfo, MarketDataSync};

use super::market_data::SpotFundsPriceError;
use crate::param::{AccountId, Asset, PositionSize, Price, Side, TradeAmount};
use crate::pretrade::holdings::{HoldError, Holdings};
use crate::pretrade::policy::missing_required_field_reject;
use crate::pretrade::{PolicyPreTradeResult, Reject, RejectCode, RejectScope, Rejects};
use crate::storage::ConfigCell;
use crate::Mutations;

use super::rejects::{
    arithmetic_overflow_reject, insufficient_funds_reject, order_value_calculation_failed_reject,
};
use super::views::OrderRequestView;
use super::SpotFundsPolicy;

impl<Sync, MarketDataSyncMode> SpotFundsPolicy<Sync, MarketDataSyncMode>
where
    Sync: SyncMode,
    MarketDataSyncMode: MarketDataSync,
{
    /// Returns a [`Reject`] appropriate for the given [`SpotFundsPriceError`].
    pub(super) fn reject_from_price_err(err: SpotFundsPriceError) -> Reject {
        match err {
            SpotFundsPriceError::QuoteUnavailable => Reject::new(
                Self::NAME,
                RejectScope::Order,
                RejectCode::MarkPriceUnavailable,
                "mark price unavailable",
                "",
            ),
            SpotFundsPriceError::CalculationFailed => Reject::new(
                Self::NAME,
                RejectScope::Order,
                RejectCode::OrderValueCalculationFailed,
                "price calculation failed",
                "",
            ),
        }
    }

    /// Returns the market-orders-unsupported reject.
    pub(super) fn reject_market_orders_unsupported() -> Reject {
        Reject::new(
            Self::NAME,
            RejectScope::Order,
            RejectCode::UnsupportedOrderType,
            "market orders not supported",
            "",
        )
    }

    /// Resolves the effective buy price for a market order.
    ///
    /// The instrument is resolved and the quote fetched through the
    /// market-data service handle, then the slippage cascade is applied by
    /// reading the settings cell on the hot path - allocation-free and
    /// without acquiring a per-order lock.
    ///
    /// `account_id` and `account_info` select the per-account / per-group /
    /// default quote and the TTL cascade tier (widest [`QuoteResolution`]).
    ///
    /// [`QuoteResolution`]: crate::marketdata::QuoteResolution
    pub(super) fn compute_buy_with_md(
        &self,
        instrument: &Instrument,
        account_id: AccountId,
        account_info: &impl AccountInfo,
    ) -> Result<Price, Reject> {
        let bundle = self
            .market_orders
            .as_ref()
            .ok_or_else(Self::reject_market_orders_unsupported)?;
        let instrument_id = bundle
            .resolve(instrument)
            .ok_or_else(|| Self::reject_from_price_err(SpotFundsPriceError::QuoteUnavailable))?;
        let quote = bundle
            .quote(instrument_id, account_id, account_info)
            .ok_or_else(|| Self::reject_from_price_err(SpotFundsPriceError::QuoteUnavailable))?;
        self.settings
            .with(|s| s.effective_buy_price(&quote, instrument_id, account_id, account_info))
            .map_err(Self::reject_from_price_err)
    }

    /// Resolves the effective sell price for a market order.
    ///
    /// The instrument is resolved and the quote fetched through the
    /// market-data service handle, then the slippage cascade is applied by
    /// reading the settings cell on the hot path - allocation-free and
    /// without acquiring a per-order lock.
    ///
    /// `account_id` and `account_info` select the per-account / per-group /
    /// default quote and the TTL cascade tier (widest [`QuoteResolution`]).
    ///
    /// [`QuoteResolution`]: crate::marketdata::QuoteResolution
    pub(super) fn compute_sell_with_md(
        &self,
        instrument: &Instrument,
        account_id: AccountId,
        account_info: &impl AccountInfo,
    ) -> Result<Price, Reject> {
        let bundle = self
            .market_orders
            .as_ref()
            .ok_or_else(Self::reject_market_orders_unsupported)?;
        let instrument_id = bundle
            .resolve(instrument)
            .ok_or_else(|| Self::reject_from_price_err(SpotFundsPriceError::QuoteUnavailable))?;
        let quote = bundle
            .quote(instrument_id, account_id, account_info)
            .ok_or_else(|| Self::reject_from_price_err(SpotFundsPriceError::QuoteUnavailable))?;
        self.settings
            .with(|s| s.effective_sell_price(&quote, instrument_id, account_id, account_info))
            .map_err(Self::reject_from_price_err)
    }

    pub(super) fn read_order_request<'i, Order>(
        &self,
        order: &'i Order,
    ) -> Result<OrderRequestView<'i>, Rejects>
    where
        Order: HasInstrument + HasAccountId + HasSide + HasTradeAmount + HasOrderPrice,
    {
        let instrument = order
            .instrument()
            .map_err(|e| Rejects::from(missing_required_field_reject(self, "instrument", &e)))?;
        let account_id = order
            .account_id()
            .map_err(|e| Rejects::from(missing_required_field_reject(self, "account ID", &e)))?;
        let side = order
            .side()
            .map_err(|e| Rejects::from(missing_required_field_reject(self, "side", &e)))?;
        let trade_amount = order
            .trade_amount()
            .map_err(|e| Rejects::from(missing_required_field_reject(self, "trade amount", &e)))?;
        let price = order
            .price()
            .map_err(|e| Rejects::from(missing_required_field_reject(self, "price", &e)))?;
        Ok(OrderRequestView {
            instrument,
            account_id,
            side,
            trade_amount,
            price,
        })
    }

    /// Computes the two signed settlement legs reserved for an order.
    ///
    /// A spot order owes, per asset, the net outflow it would incur:
    /// `max(0, amount_owed)`. The underlying leg is owed only when giving the
    /// asset away (sells); the settlement leg is owed only when net cash flows
    /// out. A negative or zero price is fully legitimate and never rejected:
    /// it merely flips which legs carry a positive reservation.
    ///
    /// - `Buy`:  underlying owed `0`; settlement owed `max(0, p*q)`.
    /// - `Sell`: underlying owed `q`; settlement owed `max(0, -p*q)`.
    ///
    /// The returned [`ReservationLegs::lock_price`] is the effective settlement
    /// price the execution path must persist to reconcile the settlement
    /// `held` later. It is `Some` whenever the settlement leg participates in
    /// `held` reconciliation: always for buys (so a dropped lock is caught as a
    /// block), and for sells only when the price is negative (the only sell
    /// case that reserves settlement).
    pub(super) fn compute_reservation_legs(
        &self,
        side: Side,
        trade_amount: TradeAmount,
        order_price: Option<Price>,
        instrument: &Instrument,
        account_id: AccountId,
        account_info: &impl AccountInfo,
    ) -> Result<ReservationLegs, Reject> {
        match side {
            Side::Buy => {
                let buy_price = match order_price {
                    Some(p) => p,
                    None => self.compute_buy_with_md(instrument, account_id, account_info)?,
                };
                let settlement_notional = match trade_amount {
                    TradeAmount::Quantity(q) => buy_price
                        .calculate_position_size(q)
                        .map_err(|_| order_value_calculation_failed_reject(Self::NAME, ""))?,
                    // A Volume buy spends `v` of settlement, carrying the price
                    // sign: positive price pays `v`, negative price receives it.
                    TradeAmount::Volume(v) => signed_volume(v, buy_price),
                };
                Ok(ReservationLegs {
                    underlying: PositionSize::ZERO,
                    settlement: non_negative(settlement_notional),
                    // Buys always persist the lock price so a missing lock on a
                    // later fill/cancel is treated as a reconciliation error.
                    lock_price: Some(buy_price),
                })
            }
            Side::Sell => {
                let (quantity, sell_price) = match trade_amount {
                    TradeAmount::Quantity(q) => {
                        // Limit sells carry their own price; market sells use the
                        // bundle when present. Without either, the settlement
                        // sign is unknown, so only the underlying leg is gated -
                        // the signed execution path still settles cash correctly.
                        let price = match order_price {
                            Some(p) => Some(p),
                            None => self
                                .compute_sell_with_md(instrument, account_id, account_info)
                                .ok(),
                        };
                        (q, price)
                    }
                    TradeAmount::Volume(v) => {
                        let price = match order_price {
                            Some(p) => p,
                            None => {
                                self.compute_sell_with_md(instrument, account_id, account_info)?
                            }
                        };
                        // Sizing a Volume sell needs `v / |p|`; a zero price
                        // leaves the quantity undefined (division by zero) and
                        // surfaces as a calculation failure, not a sign reject.
                        let quantity = v
                            .calculate_quantity(price)
                            .map_err(|_| order_value_calculation_failed_reject(Self::NAME, ""))?;
                        (quantity, Some(price))
                    }
                };
                let settlement_owed = match sell_price {
                    Some(price) => {
                        let notional = price
                            .calculate_position_size(quantity)
                            .map_err(|_| order_value_calculation_failed_reject(Self::NAME, ""))?;
                        // Settlement owed = max(0, -p*q): positive only when the
                        // sell price is negative (paying to dispose of the asset).
                        non_negative(neg(notional))
                    }
                    None => PositionSize::ZERO,
                };
                Ok(ReservationLegs {
                    underlying: quantity.to_position_size(),
                    settlement: settlement_owed,
                    // Persist the price only when the settlement leg is actually
                    // reserved (negative price); a non-negative sell reserves no
                    // settlement and keeps the historical "sell ignores lock"
                    // contract.
                    lock_price: sell_price.filter(|_| !settlement_owed.is_zero()),
                })
            }
        }
    }

    pub(super) fn perform_pre_trade_check_impl<Order>(
        &self,
        account_control: Option<AccountControl<<Sync as SyncMode>::StorageLockingPolicyFactory>>,
        account_info: &impl AccountInfo,
        order: &Order,
        mutations: &mut Mutations,
    ) -> Result<Option<PolicyPreTradeResult>, Rejects>
    where
        Order: HasInstrument + HasAccountId + HasSide + HasTradeAmount + HasOrderPrice,
        <<Sync as SyncMode>::StorageLockingPolicyFactory as crate::storage::LockingPolicyFactory>::Policy: 'static,
    {
        let request = self.read_order_request(order)?;

        let legs = self
            .compute_reservation_legs(
                request.side,
                request.trade_amount,
                request.price,
                request.instrument,
                request.account_id,
                account_info,
            )
            .map_err(Rejects::from)?;

        let underlying_asset = request.instrument.underlying_asset().clone();
        let settlement_asset = request.instrument.settlement_asset().clone();

        let mut outcome =
            PolicyPreTradeResult::with_capacity(2, legs.lock_price.is_some() as usize);

        // Each reserved leg is held independently and registers its own
        // delta-based rollback. If the second leg's hold fails the engine
        // rolls back every mutation pushed so far (see the pre-trade pipeline),
        // undoing the first leg, so partial reservations never escape.
        self.reserve_leg(
            request.account_id,
            &underlying_asset,
            legs.underlying,
            account_control.clone(),
            mutations,
            &mut outcome,
        )?;
        self.reserve_leg(
            request.account_id,
            &settlement_asset,
            legs.settlement,
            account_control,
            mutations,
            &mut outcome,
        )?;

        if let Some(p) = legs.lock_price {
            outcome.lock_prices.push(p);
        }

        Ok(Some(outcome))
    }

    /// Read-only twin of [`Self::perform_pre_trade_check_impl`] for dry-runs.
    ///
    /// Reuses [`Self::read_order_request`] and
    /// [`Self::compute_reservation_legs`], then for each leg reads the current
    /// holdings and computes the would-be hold through the pure
    /// [`Holdings::try_hold`] - **without** writing it back. It pushes nothing to
    /// storage and nothing to `mutations`, so a dry-run never moves engine
    /// state. The reject and outcome it produces match the normal path byte for
    /// byte for the same order and holdings, because both derive them from the
    /// same `try_hold` result.
    pub(super) fn perform_pre_trade_check_dry_run_impl<Order>(
        &self,
        account_info: &impl AccountInfo,
        order: &Order,
    ) -> Result<Option<PolicyPreTradeResult>, Rejects>
    where
        Order: HasInstrument + HasAccountId + HasSide + HasTradeAmount + HasOrderPrice,
    {
        let request = self.read_order_request(order)?;

        let legs = self
            .compute_reservation_legs(
                request.side,
                request.trade_amount,
                request.price,
                request.instrument,
                request.account_id,
                account_info,
            )
            .map_err(Rejects::from)?;

        let underlying_asset = request.instrument.underlying_asset().clone();
        let settlement_asset = request.instrument.settlement_asset().clone();

        let mut outcome =
            PolicyPreTradeResult::with_capacity(2, legs.lock_price.is_some() as usize);

        self.reserve_leg_dry_run(
            request.account_id,
            &underlying_asset,
            legs.underlying,
            &mut outcome,
        )?;
        self.reserve_leg_dry_run(
            request.account_id,
            &settlement_asset,
            legs.settlement,
            &mut outcome,
        )?;

        if let Some(p) = legs.lock_price {
            outcome.lock_prices.push(p);
        }

        Ok(Some(outcome))
    }

    /// Read-only twin of [`Self::reserve_leg`].
    ///
    /// Computes the would-be hold of `amount` for one asset leg through the pure
    /// [`Holdings::try_hold`] on the current (or zero) holdings, emits the same
    /// outcome entry and the same rejects as [`Self::reserve_leg`], and mutates
    /// nothing. A zero `amount` is a clean no-op, identical to the normal path.
    fn reserve_leg_dry_run(
        &self,
        account_id: AccountId,
        asset: &Asset,
        amount: PositionSize,
        outcome: &mut PolicyPreTradeResult,
    ) -> Result<(), Rejects> {
        if amount.is_zero() {
            return Ok(());
        }

        let current = self
            .holdings
            .get(&(account_id, asset.clone()))
            .unwrap_or_else(Holdings::zero);
        let new_holdings = current.try_hold(amount).map_err(|err| {
            Rejects::from(match err {
                HoldError::ArithmeticOverflow => arithmetic_overflow_reject(
                    Self::NAME,
                    RejectScope::Order,
                    format!(
                        "pre-trade hold overflow: account {account_id}, \
                         asset {asset}, requested {amount}",
                    ),
                ),
                HoldError::InsufficientAvailable {
                    available,
                    requested,
                } => insufficient_funds_reject(Self::NAME, asset, account_id, available, requested),
            })
        })?;

        outcome.account_adjustments.push(AccountOutcomeEntry {
            asset: asset.clone(),
            balance: Some(OutcomeAmount {
                delta: neg(amount),
                absolute: new_holdings.available(),
            }),
            held: Some(OutcomeAmount {
                delta: amount,
                absolute: new_holdings.held(),
            }),
            incoming: None,
            // Reservation moves funds between available and held only; it
            // realizes no PnL and does not change the average entry price.
            realized_pnl: None,
            average_entry_price: None,
        });
        Ok(())
    }

    /// Holds `amount` from `available` to `held` for one asset leg, pushes the
    /// matching delta rollback, and appends the leg's outcome entry.
    ///
    /// A zero `amount` is a clean no-op: no slot is created, no rollback is
    /// registered, and no outcome entry is emitted. This makes "reserve nothing"
    /// (e.g. a buy at a negative price) pass the gate without side effects.
    fn reserve_leg(
        &self,
        account_id: AccountId,
        asset: &Asset,
        amount: PositionSize,
        account_control: Option<AccountControl<<Sync as SyncMode>::StorageLockingPolicyFactory>>,
        mutations: &mut Mutations,
        outcome: &mut PolicyPreTradeResult,
    ) -> Result<(), Rejects>
    where
        <<Sync as SyncMode>::StorageLockingPolicyFactory as crate::storage::LockingPolicyFactory>::Policy: 'static,
    {
        if amount.is_zero() {
            return Ok(());
        }

        let key = (account_id, asset.clone());
        let key_for_remove = key.clone();
        let (new_holdings, was_new) = self.holdings.with_mut_or_insert_prune_new_if_zero(
            key.clone(),
            Holdings::zero,
            |slot, is_new| {
                slot.try_hold(amount)
                    .map(|new| {
                        *slot = new;
                        (new, is_new)
                    })
                    .map_err(|err| {
                        Rejects::from(match err {
                            HoldError::ArithmeticOverflow => arithmetic_overflow_reject(
                                Self::NAME,
                                RejectScope::Order,
                                format!(
                                    "pre-trade hold overflow: account {account_id}, \
                                     asset {asset}, requested {amount}",
                                ),
                            ),
                            HoldError::InsufficientAvailable {
                                available,
                                requested,
                            } => insufficient_funds_reject(
                                Self::NAME,
                                asset,
                                account_id,
                                available,
                                requested,
                            ),
                        })
                    })
            },
        )?;
        // Mirror `mutate_slot`: the prune-new variant only handles freshly
        // inserted entries, so an existing slot that ends up at zero needs an
        // explicit follow-up removal to avoid a phantom entry.
        if new_holdings.is_zero() && !was_new {
            self.holdings.remove_if_zero(&key_for_remove);
        }
        self.register_hold_rollback(mutations, account_control, key, amount);

        outcome.account_adjustments.push(AccountOutcomeEntry {
            asset: asset.clone(),
            balance: Some(OutcomeAmount {
                delta: neg(amount),
                absolute: new_holdings.available(),
            }),
            held: Some(OutcomeAmount {
                delta: amount,
                absolute: new_holdings.held(),
            }),
            incoming: None,
            // Reservation moves funds between available and held only; it
            // realizes no PnL and does not change the average entry price.
            realized_pnl: None,
            average_entry_price: None,
        });
        Ok(())
    }
}

/// The two signed settlement legs a single order reserves.
///
/// `underlying` and `settlement` are each `max(0, amount_owed)` in their
/// respective asset units, so a leg with no outflow reserves zero. `lock_price`
/// is the effective settlement price persisted for later `held` reconciliation
/// (see [`SpotFundsPolicy::compute_reservation_legs`]).
pub(super) struct ReservationLegs {
    pub(super) underlying: PositionSize,
    pub(super) settlement: PositionSize,
    pub(super) lock_price: Option<Price>,
}

/// Returns `max(0, value)`: the non-negative outflow that must be reserved.
fn non_negative(value: PositionSize) -> PositionSize {
    value.max(PositionSize::ZERO)
}

/// Negates a position size (sign flip of a settlement flow).
fn neg(value: PositionSize) -> PositionSize {
    -value
}

/// Converts a non-negative `Volume` magnitude into a signed settlement notional
/// carrying the price's sign.
fn signed_volume(volume: crate::param::Volume, price: Price) -> PositionSize {
    let magnitude = volume.to_position_size();
    if price > Price::ZERO {
        // Positive price: the volume is a settlement outflow (cash paid).
        magnitude
    } else {
        // Zero price costs nothing; a negative price is a cash inflow. Either
        // way the buy owes no settlement, so the signed notional is <= 0 and
        // `non_negative` collapses it to a zero reservation.
        neg(magnitude)
    }
}
