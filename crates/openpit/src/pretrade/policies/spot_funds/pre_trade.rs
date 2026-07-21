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
use super::SpotFundsLimitMode;
use crate::param::{AccountId, Asset, PositionSize, Price, Quantity, Side, TradeAmount};
use crate::pretrade::holdings::{HoldError, Holdings};
use crate::pretrade::policy::missing_required_field_reject;
use crate::pretrade::{
    PolicyPreTradeResult, PreTradeContext, Reject, RejectCode, RejectScope, Rejects,
};
use crate::storage::ConfigCell;
use crate::Mutations;

use super::rejects::{
    account_pnl_halted_reject, arithmetic_overflow_reject, insufficient_funds_reject,
    order_value_calculation_failed_reject,
};
use super::views::OrderRequestView;
use super::SpotFundsPolicy;

impl<Sync, MarketDataSyncMode> SpotFundsPolicy<Sync, MarketDataSyncMode>
where
    Sync: SyncMode,
    Sync::StorageLockingPolicyFactory: crate::storage::LockingPolicyFactory,
    MarketDataSyncMode: MarketDataSync,
{
    pub(super) fn reject_halted_account_pnl(
        &self,
        account_id: AccountId,
        account_info: &impl AccountInfo,
    ) -> Result<(), Rejects> {
        let barrier = self.settings.with(|settings| {
            settings
                .pnl_barrier_for(account_id, account_info.group())
                .cloned()
        });
        let Some(barrier) = barrier else {
            return Ok(());
        };
        let state = self.account_pnl_state(account_id);
        match state {
            crate::PnlState::Halted(reason) => Err(Rejects::from(account_pnl_halted_reject(
                Self::NAME,
                account_id,
                reason,
            ))),
            crate::PnlState::Value(_) => {
                if let Some(block) =
                    super::rejects::account_pnl_block_for_state(account_id, state, &barrier, None)
                {
                    Err(Rejects::from(Reject::from(block)))
                } else {
                    Ok(())
                }
            }
        }
    }

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
    /// In parallel with the `held` outflow legs the order projects the acquiring
    /// leg's expected inflow into `incoming` (purely informational, never
    /// gating): a buy projects `base_incoming = q` base units, a priced sell
    /// projects `settlement_incoming = max(0, p*q)` quote proceeds.
    ///
    /// The returned [`ReservationLegs::lock_price`] is the effective settlement
    /// price the execution path must persist to reconcile the settlement `held`
    /// or `incoming` later. It is always `Some`: every accepted order resolves a
    /// price (buys and sells alike), so a dropped lock on a later fill/cancel is
    /// always caught as an account block. A Quantity sell with no order price and
    /// no market-data price is unpriceable and rejected here with
    /// [`RejectCode::MarkPriceUnavailable`], never accepted without a lock.
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
                let (settlement_notional, base_incoming) = match trade_amount {
                    TradeAmount::Quantity(q) => {
                        let notional = buy_price
                            .calculate_position_size(q)
                            .map_err(|_| order_value_calculation_failed_reject(Self::NAME, ""))?;
                        (notional, q.to_position_size())
                    }
                    // A Volume buy spends `v` of settlement, carrying the price
                    // sign: positive price pays `v`, negative price receives it.
                    // The acquired base quantity is `v / |p|`, well-defined
                    // for any non-zero price; only a zero price leaves it
                    // undefined, so the base inflow projection is then empty
                    // (informational only, gates nothing) without introducing
                    // a new reject.
                    TradeAmount::Volume(v) => {
                        let base_incoming = if buy_price.is_zero() {
                            PositionSize::ZERO
                        } else {
                            v.calculate_quantity(buy_price)
                                .map_err(|_| order_value_calculation_failed_reject(Self::NAME, ""))?
                                .to_position_size()
                        };
                        (signed_volume(v, buy_price), base_incoming)
                    }
                };
                Ok(ReservationLegs {
                    underlying: PositionSize::ZERO,
                    settlement: non_negative(settlement_notional),
                    base_incoming,
                    settlement_incoming: PositionSize::ZERO,
                    // Buys always persist the lock price so a missing lock on a
                    // later fill/cancel is treated as a reconciliation error.
                    lock_price: Some(buy_price),
                })
            }
            Side::Sell => {
                // Every accepted sell must resolve a price: a Quantity sell uses
                // its order price or the market-data bundle, rejecting as
                // unpriceable when neither is present (exactly like the buy path);
                // a Volume sell needs a price to size the quantity. This
                // guarantees a recorded lock price the settlement leg can
                // reconcile against later, and a missing lock on a later
                // fill/cancel is then a reconciliation error, not a valid order.
                let sell_price = match order_price {
                    Some(p) => p,
                    None => self.compute_sell_with_md(instrument, account_id, account_info)?,
                };
                let quantity = match trade_amount {
                    TradeAmount::Quantity(q) => q,
                    TradeAmount::Volume(v) => {
                        // Sizing a Volume sell needs `v / |p|`, well-defined for
                        // any non-zero price. A zero price cannot size the
                        // quantity (`v / 0`), so the sell reserves nothing and
                        // still passes - a zero price is treated like any other,
                        // never as an error.
                        if sell_price.is_zero() {
                            Quantity::ZERO
                        } else {
                            v.calculate_quantity(sell_price).map_err(|_| {
                                order_value_calculation_failed_reject(Self::NAME, "")
                            })?
                        }
                    }
                };
                let notional = sell_price
                    .calculate_position_size(quantity)
                    .map_err(|_| order_value_calculation_failed_reject(Self::NAME, ""))?;
                // Settlement owed = max(0, -p*q): positive only when the sell
                // price is negative (paying to dispose of the asset). The mirror
                // projection settlement incoming = max(0, p*q) is the expected
                // proceeds, positive only for a non-negative price. The two are
                // mutually exclusive by the price sign.
                let settlement_owed = non_negative(neg(notional));
                let settlement_incoming = non_negative(notional);
                Ok(ReservationLegs {
                    underlying: quantity.to_position_size(),
                    settlement: settlement_owed,
                    base_incoming: PositionSize::ZERO,
                    settlement_incoming,
                    // Every accepted sell records its resolved price so the
                    // fill/cancel path can reconcile the settlement leg. A sell
                    // execution report arriving without this lock is a
                    // reconciliation error and blocks the account, symmetric to a
                    // buy.
                    lock_price: Some(sell_price),
                })
            }
        }
    }

    pub(super) fn perform_pre_trade_check_impl<Order>(
        &self,
        ctx: &PreTradeContext<<Sync as SyncMode>::StorageLockingPolicyFactory>,
        order: &Order,
        mutations: &mut Mutations,
    ) -> Result<Option<PolicyPreTradeResult>, Rejects>
    where
        Order: HasInstrument + HasAccountId + HasSide + HasTradeAmount + HasOrderPrice,
        <<Sync as SyncMode>::StorageLockingPolicyFactory as crate::storage::LockingPolicyFactory>::Policy: 'static,
    {
        let request = self.read_order_request(order)?;
        let pnl_rejects = self.reject_halted_account_pnl(request.account_id, ctx);
        if ctx.is_drop_copy() {
            if let Err(rejects) = &pnl_rejects {
                if let Some(reject) = rejects
                    .iter()
                    .find(|reject| reject.scope == RejectScope::Account)
                {
                    ctx.record_drop_copy_account_block(
                        reject.account_block_with_code(RejectCode::AccountBlocked),
                    );
                }
            }
        } else {
            pnl_rejects?;
        }

        let legs = self
            .compute_reservation_legs(
                request.side,
                request.trade_amount,
                request.price,
                request.instrument,
                request.account_id,
                ctx,
            )
            .map_err(Rejects::from)?;

        let underlying_asset = request.instrument.underlying_asset().clone();
        let settlement_asset = request.instrument.settlement_asset().clone();

        // The funds limit mode resolves once per order along the account-centric
        // cascade (account -> account group -> global) and gates only the `held`
        // legs below; `incoming`, position, average-entry and realized-PnL
        // bookkeeping are mode-independent.
        let limit_mode = if ctx.is_drop_copy() {
            SpotFundsLimitMode::TrackOnly
        } else {
            self.settings
                .with(|s| s.limit_mode_for(request.account_id, ctx))
        };

        let mut outcome =
            PolicyPreTradeResult::with_capacity(2, legs.lock_price.is_some() as usize);

        // Each reserved asset is held independently and registers its own
        // delta-based rollback. If a later step's hold fails the engine rolls
        // back every mutation pushed so far (see the pre-trade pipeline), undoing
        // earlier steps, so partial reservations never escape. The step order is
        // shared with the dry-run twin so emission order cannot drift.
        let steps = reservation_steps(&legs, &underlying_asset, &settlement_asset, request.side);
        for step in steps {
            self.reserve_asset(
                request.account_id,
                &step.asset,
                step.held,
                step.incoming,
                limit_mode,
                ctx.account_control.clone(),
                mutations,
                &mut outcome,
            )?;
        }

        if let Some(p) = legs.lock_price {
            outcome.lock_prices.push(p);
        }

        Ok(Some(outcome))
    }

    /// Read-only twin of [`Self::perform_pre_trade_check_impl`] for dry-runs.
    ///
    /// Reuses [`Self::read_order_request`] and
    /// [`Self::compute_reservation_legs`], then computes the would-be holds
    /// through the pure [`Holdings::try_hold`] - **without** writing them back.
    /// A temporary per-call view carries the first leg into the second when
    /// both legs touch the same account/asset key, matching the mutating path's
    /// ordered checks while leaving engine state untouched. It pushes nothing
    /// to storage and nothing to `mutations`.
    pub(super) fn perform_pre_trade_check_dry_run_impl<Order>(
        &self,
        account_info: &impl AccountInfo,
        order: &Order,
    ) -> Result<Option<PolicyPreTradeResult>, Rejects>
    where
        Order: HasInstrument + HasAccountId + HasSide + HasTradeAmount + HasOrderPrice,
    {
        let request = self.read_order_request(order)?;
        self.reject_halted_account_pnl(request.account_id, account_info)?;

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

        // Resolve the funds limit mode once, exactly as the mutating path does,
        // so a track-only dry-run mirrors a track-only reservation byte for byte.
        let limit_mode = self
            .settings
            .with(|s| s.limit_mode_for(request.account_id, account_info));

        let mut outcome =
            PolicyPreTradeResult::with_capacity(2, legs.lock_price.is_some() as usize);

        // Same ordered steps as the mutating path. When both steps touch the
        // same asset (synthetic base == settlement), the first step's would-be
        // holdings are threaded into the second so it observes the first step's
        // held and incoming, matching the mutating path byte-for-byte.
        let steps = reservation_steps(&legs, &underlying_asset, &settlement_asset, request.side);
        let [first, second] = steps;
        let first_after = self.reserve_asset_dry_run(
            request.account_id,
            &first.asset,
            first.held,
            first.incoming,
            limit_mode,
            None,
            &mut outcome,
        )?;
        let second_current = if first.asset == second.asset {
            first_after
        } else {
            None
        };
        self.reserve_asset_dry_run(
            request.account_id,
            &second.asset,
            second.held,
            second.incoming,
            limit_mode,
            second_current,
            &mut outcome,
        )?;

        if let Some(p) = legs.lock_price {
            outcome.lock_prices.push(p);
        }

        Ok(Some(outcome))
    }

    /// Read-only twin of [`Self::reserve_asset`].
    ///
    /// Computes the would-be hold of `amount` for one asset leg through the pure
    /// [`Holdings::try_hold`] + [`Holdings::reserve_incoming`] on the supplied
    /// temporary holdings or current storage holdings, emits the same outcome
    /// entry and the same rejects as [`Self::reserve_asset`], and mutates
    /// nothing. A step with both amounts zero is a clean no-op.
    ///
    /// `limit_mode` selects the hold semantics exactly as the mutating path:
    /// [`SpotFundsLimitMode::Enforce`] gates on available funds through
    /// [`Holdings::try_hold`]; [`SpotFundsLimitMode::TrackOnly`] records the hold
    /// even when it drives `available` negative through
    /// [`Holdings::hold_allow_negative`], still surfacing arithmetic overflow.
    #[allow(clippy::too_many_arguments)]
    fn reserve_asset_dry_run(
        &self,
        account_id: AccountId,
        asset: &Asset,
        held_amount: PositionSize,
        incoming_amount: PositionSize,
        limit_mode: SpotFundsLimitMode,
        current: Option<Holdings>,
        outcome: &mut PolicyPreTradeResult,
    ) -> Result<Option<Holdings>, Rejects> {
        if held_amount.is_zero() && incoming_amount.is_zero() {
            return Ok(current);
        }

        let current = current.unwrap_or_else(|| {
            self.holdings
                .get(&(account_id, asset.clone()))
                .unwrap_or_else(Holdings::zero)
        });
        let new_holdings = if held_amount.is_zero() {
            current
        } else {
            hold_with_mode(&current, held_amount, limit_mode)
                .map_err(|err| reserve_hold_reject(Self::NAME, err, asset, held_amount))?
        };
        // `reserve_incoming` has no solvency gate; the only failure is overflow,
        // mapped to a pre-trade reject like the hold overflow.
        let new_holdings = new_holdings
            .reserve_incoming(incoming_amount)
            .map_err(|_| {
                Rejects::from(arithmetic_overflow_reject(
                    Self::NAME,
                    RejectScope::Order,
                    format!(
                        "pre-trade incoming overflow: asset {asset}, \
                         requested {incoming_amount}",
                    ),
                ))
            })?;

        outcome.account_adjustments.push(reservation_outcome_entry(
            asset,
            held_amount,
            incoming_amount,
            &new_holdings,
        ));
        Ok(Some(new_holdings))
    }

    /// Holds `held_amount` from `available` to `held` and projects
    /// `incoming_amount` into `incoming` for one asset, pushes the matching delta
    /// rollback, and appends the asset's outcome entry.
    ///
    /// A step with both amounts zero is a clean no-op: no slot is created, no
    /// rollback is registered, and no outcome entry is emitted. This makes
    /// "reserve nothing" (e.g. a buy's base step at a zero price) pass the gate
    /// without side effects.
    #[allow(clippy::too_many_arguments)]
    fn reserve_asset(
        &self,
        account_id: AccountId,
        asset: &Asset,
        held_amount: PositionSize,
        incoming_amount: PositionSize,
        limit_mode: SpotFundsLimitMode,
        account_control: Option<AccountControl<<Sync as SyncMode>::StorageLockingPolicyFactory>>,
        mutations: &mut Mutations,
        outcome: &mut PolicyPreTradeResult,
    ) -> Result<(), Rejects>
    where
        <<Sync as SyncMode>::StorageLockingPolicyFactory as crate::storage::LockingPolicyFactory>::Policy: 'static,
    {
        if held_amount.is_zero() && incoming_amount.is_zero() {
            return Ok(());
        }

        let key = (account_id, asset.clone());
        let key_for_remove = key.clone();
        let (new_holdings, was_new) = self.holdings.with_mut_or_insert_prune_new_if_zero(
            key.clone(),
            Holdings::zero,
            |slot, is_new| {
                // Hold then project incoming within one slot mutation so no
                // concurrent check observes a half-applied step. The hold honors
                // the resolved limit mode: Enforce gates on available funds,
                // TrackOnly records the hold even into a negative available.
                let held = if held_amount.is_zero() {
                    Ok(*slot)
                } else {
                    hold_with_mode(slot, held_amount, limit_mode)
                        .map_err(|err| reserve_hold_reject(Self::NAME, err, asset, held_amount))
                }?;

                held.reserve_incoming(incoming_amount)
                    .map_err(|_| {
                        Rejects::from(arithmetic_overflow_reject(
                            Self::NAME,
                            RejectScope::Order,
                            format!(
                                "pre-trade incoming overflow: asset {asset}, \
                                 requested {incoming_amount}",
                            ),
                        ))
                    })
                    .map(|new| {
                        *slot = new;
                        (new, is_new)
                    })
            },
        )?;
        // Mirror `mutate_slot`: the prune-new variant only handles freshly
        // inserted entries, so an existing slot that ends up at zero needs an
        // explicit follow-up removal to avoid a phantom entry. An incoming-only
        // step keeps a non-zero `incoming`, so its slot stays alive here.
        if new_holdings.is_zero() && !was_new {
            self.holdings.remove_if_zero(&key_for_remove);
        }
        self.register_hold_rollback(
            mutations,
            account_control,
            key,
            held_amount,
            incoming_amount,
        );

        outcome.account_adjustments.push(reservation_outcome_entry(
            asset,
            held_amount,
            incoming_amount,
            &new_holdings,
        ));
        Ok(())
    }
}

/// Performs one reserve hold under the resolved funds limit mode.
///
/// [`SpotFundsLimitMode::Enforce`] gates on available funds via
/// [`Holdings::try_hold`], so an insufficient hold yields
/// [`HoldError::InsufficientAvailable`]. [`SpotFundsLimitMode::TrackOnly`]
/// records the hold even into a negative `available` via
/// [`Holdings::hold_allow_negative`], so the only error it can return is
/// [`HoldError::ArithmeticOverflow`] - the integrity guard that no mode
/// suppresses.
fn hold_with_mode(
    holdings: &Holdings,
    amount: PositionSize,
    limit_mode: SpotFundsLimitMode,
) -> Result<Holdings, HoldError> {
    match limit_mode {
        SpotFundsLimitMode::Enforce => holdings.try_hold(amount),
        SpotFundsLimitMode::TrackOnly => holdings.hold_allow_negative(amount),
    }
}

/// Maps a [`HoldError`] from the reserve hold into the policy reject.
fn reserve_hold_reject(
    policy: &str,
    err: HoldError,
    asset: &Asset,
    amount: PositionSize,
) -> Rejects {
    Rejects::from(match err {
        HoldError::ArithmeticOverflow => arithmetic_overflow_reject(
            policy,
            RejectScope::Order,
            format!(
                "pre-trade hold overflow: asset {asset}, \
                 requested {amount}",
            ),
        ),
        HoldError::InsufficientAvailable {
            available,
            requested,
        } => insufficient_funds_reject(policy, asset, available, requested),
    })
}

/// Builds the reservation outcome entry for one asset, populating only the
/// fields the step actually moved: `balance` + `held` when it held, `incoming`
/// when it projected an inflow. In this policy held and incoming never co-occur
/// on the same asset, but the entry is built generically.
fn reservation_outcome_entry(
    asset: &Asset,
    held_amount: PositionSize,
    incoming_amount: PositionSize,
    new_holdings: &Holdings,
) -> AccountOutcomeEntry {
    let (balance, held) = if held_amount.is_zero() {
        (None, None)
    } else {
        (
            Some(OutcomeAmount {
                delta: neg(held_amount),
                absolute: new_holdings.available(),
            }),
            Some(OutcomeAmount {
                delta: held_amount,
                absolute: new_holdings.held(),
            }),
        )
    };
    let incoming = if incoming_amount.is_zero() {
        None
    } else {
        Some(OutcomeAmount {
            delta: incoming_amount,
            absolute: new_holdings.incoming(),
        })
    };
    AccountOutcomeEntry {
        asset: asset.clone(),
        balance,
        held,
        incoming,
        // Reservation moves funds between available, held and incoming only; it
        // realizes no PnL and does not change the average entry price.
        realized_pnl: None,
        average_entry_price: None,
    }
}

/// The signed legs a single order reserves: the `held` outflow legs plus the
/// `incoming` inflow projection of the acquiring leg.
///
/// `underlying` and `settlement` are each `max(0, amount_owed)` in their
/// respective asset units, so a held leg with no outflow reserves zero.
/// `base_incoming` and `settlement_incoming` project the expected inflow of the
/// acquiring leg: a buy acquires base units (`base_incoming = q`), a priced sell
/// acquires quote proceeds (`settlement_incoming = max(0, p*q)`). They never
/// gate the order - `incoming` is informational. `lock_price` is the effective
/// settlement price persisted for later `held` / `incoming` reconciliation (see
/// [`SpotFundsPolicy::compute_reservation_legs`]).
pub(super) struct ReservationLegs {
    pub(super) underlying: PositionSize,
    pub(super) settlement: PositionSize,
    pub(super) base_incoming: PositionSize,
    pub(super) settlement_incoming: PositionSize,
    pub(super) lock_price: Option<Price>,
}

/// One asset leg to reserve: its `held` outflow and `incoming` inflow amounts.
///
/// In this policy a single asset never carries both a non-zero `held` and a
/// non-zero `incoming` at once (a buy's quote leg holds, its base leg projects
/// incoming; a sell's underlying leg holds, its settlement leg either holds on a
/// negative price or projects incoming on a non-negative price). The combined
/// shape is kept so the reserve helper stays uniform.
#[derive(Clone)]
pub(super) struct ReservationStep {
    pub(super) asset: Asset,
    pub(super) held: PositionSize,
    pub(super) incoming: PositionSize,
}

/// Builds the ordered per-asset reservation steps for an order, shared by the
/// mutating and dry-run paths so their emission order and amounts cannot drift.
///
/// Ordering matches the outcome contract: a buy emits `[settlement, base]`
/// (settlement held, then base incoming); a sell emits `[underlying,
/// settlement]` (underlying held, then settlement incoming or held).
pub(super) fn reservation_steps(
    legs: &ReservationLegs,
    underlying_asset: &Asset,
    settlement_asset: &Asset,
    side: Side,
) -> [ReservationStep; 2] {
    match side {
        Side::Buy => [
            ReservationStep {
                asset: settlement_asset.clone(),
                held: legs.settlement,
                incoming: legs.settlement_incoming,
            },
            ReservationStep {
                asset: underlying_asset.clone(),
                held: legs.underlying,
                incoming: legs.base_incoming,
            },
        ],
        Side::Sell => [
            ReservationStep {
                asset: underlying_asset.clone(),
                held: legs.underlying,
                incoming: legs.base_incoming,
            },
            ReservationStep {
                asset: settlement_asset.clone(),
                held: legs.settlement,
                incoming: legs.settlement_incoming,
            },
        ],
    }
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
