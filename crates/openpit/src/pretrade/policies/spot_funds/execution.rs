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

//! Execution-report fixation path for [`SpotFundsPolicy`].

use crate::core::account_outcome::{AccountAdjustmentOutcome, OutcomeAmount, PnlOutcomeAmount};
use rust_decimal::Decimal;

use crate::core::sync_mode::SyncMode;
use crate::core::{
    AccountOutcomeEntry, HasAccountId, HasExecutionReportIsFinal, HasExecutionReportLastTrade,
    HasInstrument, HasLeavesQuantity, HasPreTradeLock, HasSide, Instrument,
};
use crate::marketdata::{MarketDataError, MarketDataSync, Quote, QuoteResolution};
use crate::param::{AccountId, Asset, Pnl, PositionSize, Price, Quantity, Side, Trade};
use crate::pretrade::holdings::{AdjustmentOverflowError, Holdings};
use crate::pretrade::policy::{missing_required_field_account_block, PolicyGroupId};
use crate::pretrade::{AccountBlock, PostTradeContext, PostTradeResult, PreTradeLock, RejectCode};

use super::rejects::arithmetic_overflow_account_block;
use super::views::{ExecutionRequestView, FillCancelDeltas, LegDelta, LegKind};
use super::{HoldingsKey, SpotFundsPolicy};

impl<Sync, MarketDataSyncMode> SpotFundsPolicy<Sync, MarketDataSyncMode>
where
    Sync: SyncMode,
    Sync::StorageLockingPolicyFactory: crate::storage::LockingPolicyFactory,
    MarketDataSyncMode: MarketDataSync,
{
    /// Creates or modifies the slot at `key` via `mutation`, then prunes
    /// the entry if the resulting `Holdings` is all-zero.
    ///
    /// When the slot was absent, the pruning happens atomically inside the
    /// same exclusive-index lock that would have inserted it, so a zero-valued
    /// entry is never transiently visible to other threads. When the slot
    /// already existed and becomes zero, `remove_if_zero` is used for the
    /// follow-up removal.
    pub(super) fn mutate_slot<F>(
        &self,
        key: HoldingsKey,
        mutation: F,
    ) -> Result<Holdings, AdjustmentOverflowError>
    where
        F: FnOnce(Holdings) -> Result<Holdings, AdjustmentOverflowError>,
        <<Sync as SyncMode>::StorageLockingPolicyFactory as crate::storage::LockingPolicyFactory>::Policy: 'static,
    {
        let key_for_remove = key.clone();
        let (result, was_new) = self.holdings.with_mut_or_insert_prune_new_if_zero(
            key,
            Holdings::zero,
            |slot, is_new| {
                let new = mutation(*slot)?;
                *slot = new;
                Ok((new, is_new))
            },
        )?;
        // New slots that became zero were already removed atomically above.
        // Existing slots that became zero need a separate remove_if_zero.
        if result.is_zero() && !was_new {
            self.holdings.remove_if_zero(&key_for_remove);
        }
        Ok(result)
    }

    /// Releases `amount` from `held` (moving it to `available`). Creates
    /// the slot on demand. Prunes the slot when the result is all-zero.
    pub(super) fn release_held(
        &self,
        account_id: AccountId,
        asset: &Asset,
        amount: PositionSize,
    ) -> Result<Holdings, AdjustmentOverflowError>
    where
        <<Sync as SyncMode>::StorageLockingPolicyFactory as crate::storage::LockingPolicyFactory>::Policy: 'static,
    {
        self.mutate_slot((account_id, asset.clone()), |h| h.release(amount))
    }

    fn accounting_quote(
        &self,
        account_id: AccountId,
        ctx: &PostTradeContext<<Sync as SyncMode>::StorageLockingPolicyFactory>,
        instrument: &Instrument,
    ) -> Option<Quote>
    where
        <<Sync as SyncMode>::StorageLockingPolicyFactory as crate::storage::LockingPolicyFactory>::Policy: 'static,
    {
        let market_orders = self.market_orders.as_ref()?;
        let instrument_id = market_orders.resolve(instrument)?;
        match market_orders.market_data.get(
            instrument_id,
            account_id,
            ctx,
            QuoteResolution::AccountThenGroupThenDefault,
        ) {
            Ok(quote) | Err(MarketDataError::QuoteExpired(quote)) => Some(quote),
            Err(MarketDataError::QuoteUnavailable | MarketDataError::UnknownInstrument) => None,
        }
    }

    fn account_currency_price(
        &self,
        account_id: AccountId,
        ctx: &PostTradeContext<<Sync as SyncMode>::StorageLockingPolicyFactory>,
        quote_asset: &Asset,
        account_currency: &Asset,
        trade_price: Price,
    ) -> Result<Option<Price>, AccountBlock>
    where
        <<Sync as SyncMode>::StorageLockingPolicyFactory as crate::storage::LockingPolicyFactory>::Policy: 'static,
    {
        if quote_asset == account_currency {
            return Ok(Some(trade_price));
        }

        let direct = Instrument::new(quote_asset.clone(), account_currency.clone());
        if let Some(mark) = self
            .accounting_quote(account_id, ctx, &direct)
            .and_then(|quote| quote.mark)
        {
            return trade_price_with_factor(Self::NAME, trade_price, mark.to_decimal()).map(Some);
        }

        let inverse = Instrument::new(account_currency.clone(), quote_asset.clone());
        if let Some(mark) = self
            .accounting_quote(account_id, ctx, &inverse)
            .and_then(|quote| quote.mark)
        {
            let Some(factor) = Decimal::ONE.checked_div(mark.to_decimal()) else {
                return Ok(None);
            };
            return trade_price_with_factor(Self::NAME, trade_price, factor).map(Some);
        }

        Ok(None)
    }

    pub(super) fn read_execution_request<'i, ExecutionReport>(
        &self,
        report: &'i ExecutionReport,
    ) -> Result<ExecutionRequestView<'i>, AccountBlock>
    where
        ExecutionReport: HasInstrument
            + HasAccountId
            + HasSide
            + HasExecutionReportLastTrade
            + HasLeavesQuantity
            + HasExecutionReportIsFinal
            + HasPreTradeLock,
    {
        let account_id = report
            .account_id()
            .map_err(|e| missing_required_field_account_block(self, "account ID", &e))?;
        let instrument = report
            .instrument()
            .map_err(|e| missing_required_field_account_block(self, "instrument", &e))?;
        let side = report
            .side()
            .map_err(|e| missing_required_field_account_block(self, "side", &e))?;
        let last_trade = report
            .last_trade()
            .map_err(|e| missing_required_field_account_block(self, "last fill", &e))?;
        let leaves_quantity = report
            .leaves_quantity()
            .map_err(|e| missing_required_field_account_block(self, "remaining quantity", &e))?;
        let is_final = report
            .is_final()
            .map_err(|e| missing_required_field_account_block(self, "order finality", &e))?;
        let lock = report
            .lock()
            .map_err(|e| missing_required_field_account_block(self, "pre-trade lock", &e))?;
        Ok(ExecutionRequestView {
            instrument,
            account_id,
            side,
            last_trade,
            leaves_quantity,
            is_final,
            lock,
        })
    }

    /// Applies a venue-authoritative fill, reconciling both the underlying and
    /// settlement legs in signed terms.
    ///
    /// Each leg moves money in its signed flow direction: the reserved `held`
    /// is consumed by the portion of this fill that was actually reserved
    /// (`max(0, outflow)`), and `available` absorbs the net of the consumed
    /// reservation and the real signed cash flow. A leg that reserved nothing
    /// (e.g. the settlement of a buy at a negative price) simply credits the
    /// inflow to `available`.
    ///
    /// Any [`AccountBlock`] returned (e.g. overflow, or a missing buy lock
    /// price) is propagated up to [`Self::apply_execution_report_impl`] and
    /// collected into [`PostTradeResult::account_blocks`]; the engine's
    /// [`BlockedAccounts`](crate::core::BlockedAccounts) records the first
    /// block for the account, so policy code does not need to wire a
    /// separate sink.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn apply_trade_fill(
        &self,
        ctx: &PostTradeContext<<Sync as SyncMode>::StorageLockingPolicyFactory>,
        account_id: AccountId,
        underlying_asset: &Asset,
        settlement_asset: &Asset,
        side: Side,
        trade: Trade,
        lock: &PreTradeLock,
        deltas: &mut FillCancelDeltas,
    ) -> Result<(), AccountBlock>
    where
        <<Sync as SyncMode>::StorageLockingPolicyFactory as crate::storage::LockingPolicyFactory>::Policy: 'static,
    {
        let qty_pos = trade.quantity.to_position_size();
        // Signed settlement notional `price * qty`; negative when the venue
        // fills at a negative price.
        let settlement_notional = trade
            .price
            .calculate_position_size(trade.quantity)
            .map_err(|_| {
                arithmetic_overflow_account_block(
                    Self::NAME,
                    format!(
                        "fill notional volume overflow: account {account_id}, \
                         asset {settlement_asset}, px {}, qty {}",
                        trade.price, trade.quantity,
                    ),
                )
            })?;

        // Underlying leg: buys receive `+qty`, sells give `-qty`. Only sells
        // reserved underlying `held`, so only sells consume on fill.
        let (underlying_consume, underlying_flow) = match side {
            Side::Buy => (PositionSize::ZERO, qty_pos),
            Side::Sell => (qty_pos, neg(qty_pos)),
        };
        let touches_position_accounting = !underlying_flow.is_zero();
        let account_currency_price = if touches_position_accounting {
            match ctx.account_currency() {
                Some(account_currency) => self.account_currency_price(
                    account_id,
                    ctx,
                    settlement_asset,
                    &account_currency,
                    trade.price,
                )?,
                None => None,
            }
        } else {
            None
        };
        // Settlement leg: buys pay `price*qty` (flow `-notional`), sells
        // receive it (flow `+notional`). The consumed reservation is the
        // portion priced at the lock; a leg that reserved nothing consumes 0
        // and credits the full inflow.
        let settlement_consume =
            self.settlement_fill_consume(account_id, settlement_asset, side, trade, lock)?;
        let settlement_flow = match side {
            Side::Buy => neg(settlement_notional),
            Side::Sell => settlement_notional,
        };

        // Process the charge leg (the one consuming reserved `held`) before the
        // credit leg, so that if the credit leg overflows the already-applied
        // charge mutation is still reported (the non-atomicity contract). The
        // charge side is settlement for a buy and underlying for a sell.
        //
        // Only the underlying leg carries the account-currency fill price for
        // average-cost / realized-PnL accounting; its net `owned` change equals
        // `flow_received` (the signed base quantity). The settlement leg passes
        // `None` and never touches the average or realized PnL.
        let underlying_leg = (
            LegKind::Underlying,
            underlying_asset,
            underlying_consume,
            underlying_flow,
            account_currency_price,
        );
        let settlement_leg = (
            LegKind::Settlement,
            settlement_asset,
            settlement_consume,
            settlement_flow,
            None,
        );
        let ordered = match side {
            Side::Buy => [settlement_leg, underlying_leg],
            Side::Sell => [underlying_leg, settlement_leg],
        };
        for (kind, asset, consume, flow, realize_price) in ordered {
            self.settle_fill_leg(
                account_id,
                asset,
                kind,
                consume,
                flow,
                realize_price,
                deltas,
            )?;
        }
        Ok(())
    }

    /// Reconciles one asset leg of a fill: `held -= consume` and
    /// `available += consume + flow_received`, recorded into `deltas`.
    ///
    /// `consume` is the (non-negative) reserved portion this fill releases from
    /// `held`; `flow_received` is the signed cash/asset flow into `available`
    /// (positive inflow, negative outflow). When both are zero the leg is left
    /// untouched and no outcome is emitted.
    #[allow(clippy::too_many_arguments)]
    fn settle_fill_leg(
        &self,
        account_id: AccountId,
        asset: &Asset,
        kind: LegKind,
        consume: PositionSize,
        flow_received: PositionSize,
        realize_price: Option<Price>,
        deltas: &mut FillCancelDeltas,
    ) -> Result<(), AccountBlock>
    where
        <<Sync as SyncMode>::StorageLockingPolicyFactory as crate::storage::LockingPolicyFactory>::Policy: 'static,
    {
        // `balance_credit = consume + flow_received` is the net change to
        // available: the reservation handed back, plus (or minus) the real
        // signed flow. For a fully reserved outflow this is the price-
        // improvement savings; for an unreserved inflow it is the whole flow.
        let balance_credit = consume.checked_add(flow_received).map_err(|_| {
            arithmetic_overflow_account_block(
                Self::NAME,
                format!(
                    "fill balance credit overflow: account {account_id}, asset {asset}, \
                     consume {consume}, flow {flow_received}"
                ),
            )
        })?;
        if consume.is_zero() && balance_credit.is_zero() {
            return Ok(());
        }

        // Average-cost / realized-PnL accounting for the underlying leg. The
        // net `owned` change for the leg is `flow_received`, so it is the signed
        // fill quantity fed to `realize_position_fill`. The realized delta is
        // captured out of the mutate_slot closure (which runs exactly once,
        // synchronously) so it can be recorded into the leg accumulator.
        let mut pnl_delta = None;

        // Held reduction, the available credit, and the average/PnL update are
        // merged into a single mutate_slot call so no concurrent pre-trade check
        // ever observes a partially-applied leg.
        let new_h = self
            .mutate_slot((account_id, asset.clone()), |h| {
                // Realize first: the average-cost formula reads `owned` before
                // the quantity mutation, and `realize_position_fill` changes
                // only the average / realized PnL (not available/held).
                let realized = match (kind, realize_price) {
                    (LegKind::Underlying, Some(price)) => {
                        let (with_pnl, delta) = h.realize_position_fill(flow_received, price)?;
                        pnl_delta = delta;
                        with_pnl
                    }
                    (LegKind::Underlying, None) if flow_received.is_zero() => h,
                    (LegKind::Underlying, None) => h.without_position_tracking(),
                    (LegKind::Settlement, _) => h,
                };
                let after_outflow = realized.apply_fill_outflow(consume)?;
                if balance_credit.is_zero() {
                    Ok(after_outflow)
                } else {
                    after_outflow.apply_fill_inflow(balance_credit)
                }
            })
            .map_err(|_| {
                arithmetic_overflow_account_block(
                    Self::NAME,
                    format!(
                        "fill leg mutation overflow: account {account_id}, asset {asset}, \
                         consume {consume}, credit {balance_credit}"
                    ),
                )
            })?;

        let leg = deltas.leg_mut(kind);
        leg.held_delta = leg.held_delta.checked_sub(consume).map_err(|_| {
            arithmetic_overflow_account_block(
                Self::NAME,
                format!(
                    "fill held delta overflow: account {account_id}, asset {asset}, \
                     consume {consume}"
                ),
            )
        })?;
        leg.balance_delta = leg.balance_delta.checked_add(balance_credit).map_err(|_| {
            arithmetic_overflow_account_block(
                Self::NAME,
                format!(
                    "fill balance delta overflow: account {account_id}, asset {asset}, \
                     credit {balance_credit}"
                ),
            )
        })?;
        if let Some(pnl_delta) = pnl_delta {
            leg.pnl_delta = Some(match leg.pnl_delta {
                Some(current) => current.checked_add(pnl_delta).map_err(|_| {
                    arithmetic_overflow_account_block(
                        Self::NAME,
                        format!(
                            "fill pnl delta overflow: account {account_id}, asset {asset}, \
                             pnl {pnl_delta}"
                        ),
                    )
                })?,
                None => pnl_delta,
            });
        }
        leg.final_holdings = Some(new_h);
        Ok(())
    }

    /// Computes the settlement `held` consumed by one fill.
    ///
    /// Returns `max(0, settlement_outflow_at_lock)` for the fill quantity. A buy
    /// requires a single lock price (a missing or duplicate price is an
    /// account-blocking error). A sell consults the lock only when a price is
    /// present (the negative-price case that reserved settlement); without a
    /// price it reserved no settlement, so it consumes zero.
    fn settlement_fill_consume(
        &self,
        account_id: AccountId,
        settlement_asset: &Asset,
        side: Side,
        trade: Trade,
        lock: &PreTradeLock,
    ) -> Result<PositionSize, AccountBlock> {
        let lock_price =
            settlement_lock_price(Self::NAME, side, lock, self.group_id(), "buy fill")?;
        settlement_reserved_amount(
            Self::NAME,
            side,
            lock_price,
            trade.quantity,
            account_id,
            settlement_asset,
        )
    }

    /// Releases the unfilled remainder of an order back to `available`,
    /// reconciling both reserved legs.
    ///
    /// Any [`AccountBlock`] returned propagates up to
    /// [`Self::apply_execution_report_impl`] for the engine's
    /// [`BlockedAccounts`](crate::core::BlockedAccounts) to record.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn apply_cancel_release(
        &self,
        account_id: AccountId,
        underlying_asset: &Asset,
        settlement_asset: &Asset,
        side: Side,
        leaves_quantity: Quantity,
        lock: &PreTradeLock,
        deltas: &mut FillCancelDeltas,
    ) -> Result<(), AccountBlock>
    where
        <<Sync as SyncMode>::StorageLockingPolicyFactory as crate::storage::LockingPolicyFactory>::Policy: 'static,
    {
        // Underlying release: only sells reserved underlying, by quantity.
        let underlying_release = match side {
            Side::Buy => PositionSize::ZERO,
            Side::Sell => leaves_quantity.to_position_size(),
        };
        self.release_leg(
            account_id,
            underlying_asset,
            LegKind::Underlying,
            underlying_release,
            deltas,
        )?;

        // Settlement release: the unfilled reserved settlement remainder.
        let settlement_release =
            self.settlement_release(account_id, settlement_asset, side, leaves_quantity, lock)?;
        self.release_leg(
            account_id,
            settlement_asset,
            LegKind::Settlement,
            settlement_release,
            deltas,
        )?;
        Ok(())
    }

    /// Computes the settlement `held` released on cancel: the reserved
    /// remainder `max(0, settlement_outflow_at_lock)` for `leaves_quantity`.
    /// Lock handling mirrors [`Self::settlement_fill_consume`].
    fn settlement_release(
        &self,
        account_id: AccountId,
        settlement_asset: &Asset,
        side: Side,
        leaves_quantity: Quantity,
        lock: &PreTradeLock,
    ) -> Result<PositionSize, AccountBlock> {
        let lock_price =
            settlement_lock_price(Self::NAME, side, lock, self.group_id(), "buy release")?;
        settlement_reserved_amount(
            Self::NAME,
            side,
            lock_price,
            leaves_quantity,
            account_id,
            settlement_asset,
        )
    }

    /// Reconciles one asset leg of a cancel: `held -= release` and
    /// `available += release`, recorded into `deltas`. A zero release is a
    /// no-op.
    fn release_leg(
        &self,
        account_id: AccountId,
        asset: &Asset,
        kind: LegKind,
        release: PositionSize,
        deltas: &mut FillCancelDeltas,
    ) -> Result<(), AccountBlock>
    where
        <<Sync as SyncMode>::StorageLockingPolicyFactory as crate::storage::LockingPolicyFactory>::Policy: 'static,
    {
        if release.is_zero() {
            return Ok(());
        }
        let new_h = self.release_held(account_id, asset, release).map_err(|_| {
            arithmetic_overflow_account_block(
                Self::NAME,
                format!(
                    "cancel release overflow: account {account_id}, asset {asset}, \
                         requested {release}"
                ),
            )
        })?;
        let leg = deltas.leg_mut(kind);
        leg.held_delta = leg.held_delta.checked_sub(release).map_err(|_| {
            arithmetic_overflow_account_block(
                Self::NAME,
                format!(
                    "cancel held delta overflow: account {account_id}, asset {asset}, \
                     release {release}"
                ),
            )
        })?;
        leg.balance_delta = leg.balance_delta.checked_add(release).map_err(|_| {
            arithmetic_overflow_account_block(
                Self::NAME,
                format!(
                    "cancel balance delta overflow: account {account_id}, asset {asset}, \
                     release {release}"
                ),
            )
        })?;
        leg.final_holdings = Some(new_h);
        Ok(())
    }

    pub(super) fn apply_execution_report_impl<ExecutionReport>(
        &self,
        ctx: &PostTradeContext<<Sync as SyncMode>::StorageLockingPolicyFactory>,
        report: &ExecutionReport,
    ) -> Option<PostTradeResult>
    where
        ExecutionReport: HasInstrument
            + HasAccountId
            + HasSide
            + HasExecutionReportLastTrade
            + HasLeavesQuantity
            + HasExecutionReportIsFinal
            + HasPreTradeLock,
        <<Sync as SyncMode>::StorageLockingPolicyFactory as crate::storage::LockingPolicyFactory>::Policy: 'static,
    {
        let request = match self.read_execution_request(report) {
            Ok(v) => v,
            Err(block) => return Some(PostTradeResult::blocks_only(vec![block])),
        };

        let underlying_asset = request.instrument.underlying_asset().clone();
        let settlement_asset = request.instrument.settlement_asset().clone();

        let mut account_blocks: Vec<AccountBlock> = Vec::new();
        let mut deltas = FillCancelDeltas::new();

        if let Some(trade) = request.last_trade {
            if let Err(block) = self.apply_trade_fill(
                ctx,
                request.account_id,
                &underlying_asset,
                &settlement_asset,
                request.side,
                trade,
                &request.lock,
                &mut deltas,
            ) {
                account_blocks.push(block);
            }
        }

        if request.is_final && !request.leaves_quantity.is_zero() {
            if let Err(block) = self.apply_cancel_release(
                request.account_id,
                &underlying_asset,
                &settlement_asset,
                request.side,
                request.leaves_quantity,
                &request.lock,
                &mut deltas,
            ) {
                account_blocks.push(block);
            }
        }

        let group_id = self.group_id();
        let mut adjustments: Vec<AccountAdjustmentOutcome> = Vec::with_capacity(2);
        push_leg_outcome(
            &mut adjustments,
            group_id,
            underlying_asset,
            &deltas.underlying,
            LegKind::Underlying,
        );
        push_leg_outcome(
            &mut adjustments,
            group_id,
            settlement_asset,
            &deltas.settlement,
            LegKind::Settlement,
        );

        if account_blocks.is_empty() && adjustments.is_empty() {
            None
        } else {
            Some(PostTradeResult {
                account_blocks,
                account_adjustments: adjustments,
            })
        }
    }
}

/// Returns the single price recorded under `group_id`, treating a missing or
/// duplicate entry as an account-blocking error. Used where a price is
/// mandatory (the buy settlement leg).
pub(super) fn single_lock_price(
    policy: &str,
    lock: &PreTradeLock,
    group_id: PolicyGroupId,
    purpose: &str,
) -> Result<Price, AccountBlock> {
    match optional_lock_price(policy, lock, group_id, purpose)? {
        Some(price) => Ok(price),
        None => Err(AccountBlock::new(
            policy,
            RejectCode::MissingRequiredField,
            format!("pre-trade lock has no price for {purpose}"),
            format!("group {}", group_id.value()),
        )),
    }
}

/// Returns the price recorded under `group_id`, if any. `None` means no price
/// was stored (a leg that reserved no settlement); a duplicate entry is an
/// account-blocking misconfiguration (two policies sharing a `group_id`).
pub(super) fn optional_lock_price(
    policy: &str,
    lock: &PreTradeLock,
    group_id: PolicyGroupId,
    purpose: &str,
) -> Result<Option<Price>, AccountBlock> {
    let mut iter = lock.prices_of(group_id);
    match (iter.next(), iter.next()) {
        (Some(p), None) => Ok(Some(p)),
        (None, _) => Ok(None),
        (Some(_), Some(_)) => Err(AccountBlock::new(
            policy,
            RejectCode::Other,
            format!(
                "pre-trade lock has multiple prices for {purpose}; \
                 two SpotFundsPolicies share a group_id"
            ),
            format!("group {}", group_id.value()),
        )),
    }
}

/// Resolves the lock price governing the settlement leg of a fill or cancel.
///
/// A buy requires a lock price and blocks if it is missing; a sell consults the
/// lock only when a price was stored (the negative-price case that reserved
/// settlement) and returns `None` otherwise. The caller converts the price into
/// a signed per-unit outflow via the side.
fn settlement_lock_price(
    policy: &str,
    side: Side,
    lock: &PreTradeLock,
    group_id: PolicyGroupId,
    purpose: &str,
) -> Result<Option<Price>, AccountBlock> {
    match side {
        Side::Buy => Ok(Some(single_lock_price(policy, lock, group_id, purpose)?)),
        Side::Sell => optional_lock_price(policy, lock, group_id, purpose),
    }
}

/// Computes the reserved settlement `held` amount for `quantity`, given the lock
/// price and side: `max(0, settlement_outflow)`, where the outflow is
/// `+price*qty` for a buy and `-price*qty` for a sell. Returns zero when no lock
/// price governs the leg (a sell that reserved no settlement).
fn settlement_reserved_amount(
    policy: &str,
    side: Side,
    lock_price: Option<Price>,
    quantity: Quantity,
    account_id: AccountId,
    settlement_asset: &Asset,
) -> Result<PositionSize, AccountBlock> {
    let Some(price) = lock_price else {
        return Ok(PositionSize::ZERO);
    };
    let notional = price.calculate_position_size(quantity).map_err(|_| {
        arithmetic_overflow_account_block(
            policy,
            format!(
                "settlement notional overflow: account {account_id}, \
                 asset {settlement_asset}, lock_px {price}, qty {quantity}"
            ),
        )
    })?;
    let outflow = match side {
        Side::Buy => notional,
        Side::Sell => neg(notional),
    };
    Ok(non_negative(outflow))
}

/// Returns `max(0, value)`: the non-negative portion of a signed outflow.
fn non_negative(value: PositionSize) -> PositionSize {
    value.max(PositionSize::ZERO)
}

/// Negates a position size.
fn neg(value: PositionSize) -> PositionSize {
    -value
}

fn trade_price_with_factor(
    policy_name: &str,
    price: Price,
    factor: Decimal,
) -> Result<Price, AccountBlock> {
    price
        .to_decimal()
        .checked_mul(factor)
        .map(Price::new)
        .ok_or_else(|| {
            arithmetic_overflow_account_block(
                policy_name,
                format!("account-currency price conversion overflow: px {price}, factor {factor}"),
            )
        })
}

/// Appends a per-asset outcome entry for a leg, omitting zero-delta fields and
/// the entry entirely when the leg was never touched.
///
/// Realized PnL and the average entry price are emitted only for the underlying
/// leg while tracking is active: `realized_pnl` carries the realized delta
/// (omitted when zero, like the quantity fields) against the cumulative
/// realized PnL, and `average_entry_price` is the absolute current average of
/// the net position. When account currency or FX is unavailable, both are
/// `None`. The settlement leg never realizes PnL and carries no average, so
/// both are `None` there.
fn push_leg_outcome(
    adjustments: &mut Vec<AccountAdjustmentOutcome>,
    group_id: PolicyGroupId,
    asset: Asset,
    leg: &LegDelta,
    kind: LegKind,
) {
    if let Some(h) = leg.final_holdings {
        let (realized_pnl, average_entry_price) = match kind {
            LegKind::Underlying => match (leg.pnl_delta, h.realized_pnl()) {
                (Some(delta), Some(absolute)) => {
                    (nonzero_pnl_outcome(delta, absolute), h.avg_entry_price())
                }
                _ => (None, None),
            },
            LegKind::Settlement => (None, None),
        };
        adjustments.push(AccountAdjustmentOutcome {
            policy_group_id: group_id,
            entry: AccountOutcomeEntry {
                asset,
                balance: nonzero_outcome(leg.balance_delta, h.available()),
                held: nonzero_outcome(leg.held_delta, h.held()),
                incoming: None,
                realized_pnl,
                average_entry_price,
            },
        });
    }
}

fn nonzero_outcome(delta: PositionSize, absolute: PositionSize) -> Option<OutcomeAmount> {
    if delta.is_zero() {
        None
    } else {
        Some(OutcomeAmount { delta, absolute })
    }
}

fn nonzero_pnl_outcome(delta: Pnl, absolute: Pnl) -> Option<PnlOutcomeAmount> {
    if delta.is_zero() {
        None
    } else {
        Some(PnlOutcomeAmount { delta, absolute })
    }
}
