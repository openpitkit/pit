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
    AccountOutcomeEntry, HasAccountId, HasExecutionReportFillFee, HasExecutionReportIsFinal,
    HasExecutionReportLastTrade, HasInstrument, HasLeavesQuantity, HasPreTradeLock, HasSide,
    Instrument,
};
use crate::marketdata::{MarketDataError, MarketDataSync, Quote, QuoteResolution};
use crate::param::{
    AccountId, Asset, MonetaryAmount, Pnl, PositionSize, Price, Quantity, Side, Trade,
};
use crate::pretrade::holdings::{AdjustmentOverflowError, Holdings};
use crate::pretrade::policy::{missing_required_field_account_block, PolicyGroupId};
use crate::pretrade::{AccountBlock, PostTradeContext, PostTradeResult, PreTradeLock, RejectCode};
use crate::storage::ConfigCell;

use super::rejects::arithmetic_overflow_account_block;
use super::views::{ExecutionRequestView, FillCancelDeltas, LegDelta, LegKind};
use super::{HoldingsKey, SpotFundsPolicy};
use crate::pretrade::policies::pnl_bounds;

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

    fn account_currency_factor(
        &self,
        account_id: AccountId,
        ctx: &PostTradeContext<<Sync as SyncMode>::StorageLockingPolicyFactory>,
        source_asset: &Asset,
        account_currency: &Asset,
    ) -> Result<Option<Decimal>, AccountBlock>
    where
        <<Sync as SyncMode>::StorageLockingPolicyFactory as crate::storage::LockingPolicyFactory>::Policy: 'static,
    {
        if source_asset == account_currency {
            return Ok(Some(Decimal::ONE));
        }

        let direct = Instrument::new(source_asset.clone(), account_currency.clone());
        if let Some(mark) = self
            .accounting_quote(account_id, ctx, &direct)
            .and_then(|quote| quote.mark)
        {
            return Ok(Some(mark.to_decimal()));
        }

        let inverse = Instrument::new(account_currency.clone(), source_asset.clone());
        if let Some(mark) = self
            .accounting_quote(account_id, ctx, &inverse)
            .and_then(|quote| quote.mark)
        {
            let Some(factor) = Decimal::ONE.checked_div(mark.to_decimal()) else {
                return Ok(None);
            };
            return Ok(Some(factor));
        }

        Ok(None)
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
        let Some(factor) =
            self.account_currency_factor(account_id, ctx, quote_asset, account_currency)?
        else {
            return Ok(None);
        };
        trade_price_with_factor(Self::NAME, trade_price, factor).map(Some)
    }

    fn pnl_barrier_for(
        &self,
        account_id: AccountId,
        account_group_id: Option<crate::param::AccountGroupId>,
        account_currency: &Asset,
    ) -> Option<super::SpotFundsPnlBoundsBarrier> {
        self.settings.with(|settings| {
            settings
                .pnl_barrier_for(account_id, account_group_id, account_currency)
                .cloned()
        })
    }

    fn pnl_missing_fx_block(
        &self,
        account_id: AccountId,
        source_asset: &Asset,
        account_currency: &Asset,
    ) -> AccountBlock {
        pnl_bounds::pnl_calculation_failed_block(
            self,
            "pnl calculation failed",
            format!(
                "pnl calculation failed: PnL could not be computed due to \
                 missing FX for account {account_id}, source asset \
                 {source_asset}, account currency {account_currency}"
            ),
        )
    }

    fn pnl_arithmetic_failed_block(&self, details: String) -> AccountBlock {
        pnl_bounds::pnl_calculation_failed_block(self, "pnl calculation failed", details)
    }

    fn fee_pnl_delta(
        &self,
        account_id: AccountId,
        ctx: &PostTradeContext<<Sync as SyncMode>::StorageLockingPolicyFactory>,
        fee: &MonetaryAmount,
        account_currency: &Asset,
    ) -> Result<Option<Pnl>, AccountBlock>
    where
        <<Sync as SyncMode>::StorageLockingPolicyFactory as crate::storage::LockingPolicyFactory>::Policy: 'static,
    {
        let Some(factor) =
            self.account_currency_factor(account_id, ctx, &fee.currency, account_currency)?
        else {
            return Ok(None);
        };
        let Some(value) = fee.amount.to_pnl().to_decimal().checked_mul(factor) else {
            return Err(self.pnl_arithmetic_failed_block(format!(
                "fee pnl conversion overflow: account {account_id}, fee {} {}, \
                 account currency {account_currency}",
                fee.amount, fee.currency
            )));
        };
        Ok(Some(Pnl::new(value)))
    }

    fn fee_pnl_delta_if_controlled(
        &self,
        account_id: AccountId,
        ctx: &PostTradeContext<<Sync as SyncMode>::StorageLockingPolicyFactory>,
        fee: &MonetaryAmount,
        account_currency: &Asset,
        pnl_barrier: Option<&super::SpotFundsPnlBoundsBarrier>,
    ) -> Result<Option<Pnl>, AccountBlock>
    where
        <<Sync as SyncMode>::StorageLockingPolicyFactory as crate::storage::LockingPolicyFactory>::Policy: 'static,
    {
        if fee.amount.is_zero() || pnl_barrier.is_none() {
            return Ok(None);
        }
        match self.fee_pnl_delta(account_id, ctx, fee, account_currency)? {
            Some(delta) => Ok(Some(delta)),
            None => Err(self.pnl_missing_fx_block(account_id, &fee.currency, account_currency)),
        }
    }

    fn fee_pnl_delta_for_tracked_slot(
        &self,
        account_id: AccountId,
        ctx: &PostTradeContext<<Sync as SyncMode>::StorageLockingPolicyFactory>,
        underlying_asset: &Asset,
        fee: &MonetaryAmount,
        account_currency: &Asset,
        deltas: &FillCancelDeltas,
    ) -> Result<Option<Pnl>, AccountBlock>
    where
        <<Sync as SyncMode>::StorageLockingPolicyFactory as crate::storage::LockingPolicyFactory>::Policy: 'static,
    {
        if fee.amount.is_zero() {
            return Ok(None);
        }
        let tracked = deltas
            .underlying
            .final_holdings
            .or_else(|| self.holdings.get(&(account_id, underlying_asset.clone())))
            .and_then(|holdings| holdings.realized_pnl())
            .is_some();
        if !tracked {
            return Ok(None);
        }
        self.fee_pnl_delta(account_id, ctx, fee, account_currency)
    }

    /// Folds an execution-report fee's account-currency P&L delta into the
    /// underlying position's realized P&L, updating both the slot's cumulative
    /// value and the reported delta in `deltas.underlying`.
    ///
    /// The fee follows the same tracking rule as position realized P&L: it is
    /// folded only when the underlying slot's realized P&L is tracked (its
    /// `realized_pnl` is `Some`) and an FX-converted `fee_pnl_delta` is
    /// available. An untracked slot leaves realized P&L untracked and keeps the
    /// fee as a pure balance debit. When the fee moves realized P&L the
    /// underlying leg's outcome is (re)populated so [`push_leg_outcome`] emits
    /// it. Account-level kill-switch accounting remains independent of whether
    /// the slot is tracked.
    fn fold_fee_into_realized_pnl(
        &self,
        account_id: AccountId,
        underlying_asset: &Asset,
        fee_pnl_delta: Option<Pnl>,
        deltas: &mut FillCancelDeltas,
    ) -> Result<(), AccountBlock>
    where
        <<Sync as SyncMode>::StorageLockingPolicyFactory as crate::storage::LockingPolicyFactory>::Policy: 'static,
    {
        let Some(fee_delta) = fee_pnl_delta.filter(|delta| !delta.is_zero()) else {
            return Ok(());
        };
        // Whether the slot was tracked and thus actually took the fee. Captured
        // out of the mutate_slot closure, which runs exactly once synchronously.
        let existing_delta = deltas.underlying.pnl_delta;
        let mut reported = None;
        let new_h = self
            .mutate_slot((account_id, underlying_asset.clone()), |h| {
                match h.realized_pnl() {
                    Some(current) => {
                        let net_delta = match existing_delta {
                            Some(existing) => existing
                                .checked_add(fee_delta)
                                .map_err(|_| AdjustmentOverflowError::ArithmeticOverflow)?,
                            None => fee_delta,
                        };
                        let updated = current
                            .checked_add(fee_delta)
                            .map_err(|_| AdjustmentOverflowError::ArithmeticOverflow)?;
                        reported = Some(net_delta);
                        Ok(h.with_realized_pnl(updated))
                    }
                    None => Ok(h),
                }
            })
            .map_err(|_| {
                self.pnl_arithmetic_failed_block(format!(
                    "spot-funds fee realized pnl overflow: fee pnl {fee_delta}, \
                     account {account_id}, asset {underlying_asset}"
                ))
            })?;
        let Some(reported) = reported else {
            return Ok(());
        };
        deltas.underlying.pnl_delta = Some(reported);
        deltas.underlying.final_holdings = Some(new_h);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn apply_fee_debit(
        &self,
        account_id: AccountId,
        underlying_asset: &Asset,
        settlement_asset: &Asset,
        fee: &MonetaryAmount,
        deltas: &mut FillCancelDeltas,
    ) -> Result<(), AccountBlock>
    where
        <<Sync as SyncMode>::StorageLockingPolicyFactory as crate::storage::LockingPolicyFactory>::Policy: 'static,
    {
        let amount = fee.amount.to_position_size();
        if amount.is_zero() {
            return Ok(());
        }
        let new_h = self
            .mutate_slot((account_id, fee.currency.clone()), |h| {
                h.apply_fill_inflow(amount)
            })
            .map_err(|_| {
                arithmetic_overflow_account_block(
                    Self::NAME,
                    format!(
                        "fee debit overflow: account {account_id}, currency {}, \
                         fee {}",
                        fee.currency, fee.amount
                    ),
                )
            })?;
        let leg = deltas.fee_leg_mut(&fee.currency, underlying_asset, settlement_asset);
        leg.balance_delta = leg.balance_delta.checked_add(amount).map_err(|_| {
            arithmetic_overflow_account_block(
                Self::NAME,
                format!(
                    "fee balance delta overflow: account {account_id}, \
                     currency {}, fee {}",
                    fee.currency, fee.amount
                ),
            )
        })?;
        leg.final_holdings = Some(new_h);
        Ok(())
    }

    /// Applies a structured execution-report fee as a standalone economic
    /// event. A venue may report a commission correction without a trade, so
    /// fee accounting must not depend on `last_trade` being present.
    #[allow(clippy::too_many_arguments)]
    fn apply_execution_fee(
        &self,
        ctx: &PostTradeContext<<Sync as SyncMode>::StorageLockingPolicyFactory>,
        account_id: AccountId,
        underlying_asset: &Asset,
        settlement_asset: &Asset,
        fee: &MonetaryAmount,
        deltas: &mut FillCancelDeltas,
    ) -> Result<(), AccountBlock>
    where
        <<Sync as SyncMode>::StorageLockingPolicyFactory as crate::storage::LockingPolicyFactory>::Policy: 'static,
    {
        let account_currency = ctx.account_currency();
        let pnl_barrier = account_currency
            .as_ref()
            .and_then(|currency| self.pnl_barrier_for(account_id, ctx.account_group(), currency));
        let fee_pnl_delta = match account_currency.as_ref() {
            Some(account_currency) if pnl_barrier.is_some() => self.fee_pnl_delta_if_controlled(
                account_id,
                ctx,
                fee,
                account_currency,
                pnl_barrier.as_ref(),
            )?,
            Some(account_currency) => self.fee_pnl_delta_for_tracked_slot(
                account_id,
                ctx,
                underlying_asset,
                fee,
                account_currency,
                deltas,
            )?,
            None => None,
        };

        self.apply_fee_debit(account_id, underlying_asset, settlement_asset, fee, deltas)?;
        let fold_block = self
            .fold_fee_into_realized_pnl(account_id, underlying_asset, fee_pnl_delta, deltas)
            .err();
        let account_pnl_block = match (
            account_currency.as_ref(),
            pnl_barrier.as_ref(),
            fee_pnl_delta,
        ) {
            (Some(account_currency), Some(barrier), Some(delta)) => {
                self.apply_account_pnl_delta(account_id, account_currency, barrier, delta)
            }
            _ => None,
        };
        if let Some(block) = fold_block.or(account_pnl_block) {
            return Err(block);
        }
        Ok(())
    }

    fn apply_account_pnl_delta(
        &self,
        account_id: AccountId,
        account_currency: &Asset,
        barrier: &super::SpotFundsPnlBoundsBarrier,
        delta: Pnl,
    ) -> Option<AccountBlock>
    where
        <<Sync as SyncMode>::StorageLockingPolicyFactory as crate::storage::LockingPolicyFactory>::Policy: 'static,
    {
        self.pnl.with_mut(
            (account_id, account_currency.clone()),
            || Pnl::ZERO,
            |entry, _is_new| {
                let previous = *entry;
                let updated = match previous.checked_add(delta) {
                    Ok(updated) => updated,
                    Err(_) => {
                        return Some(self.pnl_arithmetic_failed_block(format!(
                            "spot-funds account pnl overflow: previous {previous}, \
                             delta {delta}, account currency {account_currency}, \
                             account {account_id}"
                        )));
                    }
                };
                *entry = updated;
                let sides =
                    pnl_bounds::breached_sides(barrier.lower_bound, barrier.upper_bound, updated);
                if sides.is_empty() {
                    None
                } else {
                    Some(pnl_bounds::pnl_breach_account_block(
                        Self::NAME,
                        format!(
                            "{} bound breached: realized pnl {updated}, \
                             lower_bound {:?}, upper_bound {:?}, \
                             account currency {account_currency}, account {account_id}",
                            sides.join(" and "),
                            barrier.lower_bound,
                            barrier.upper_bound
                        ),
                    ))
                }
            },
        )
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
            + HasExecutionReportFillFee
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
        let fee = report
            .fill_fee()
            .map_err(|e| missing_required_field_account_block(self, "fill fee", &e))?;
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
            fee,
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
    /// Any [`AccountBlock`] returned (e.g. overflow, or a missing lock price on
    /// either side) is propagated up to [`Self::apply_execution_report_impl`] and
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
        fee: Option<&MonetaryAmount>,
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
        let account_currency = ctx.account_currency();
        let pnl_barrier = account_currency
            .as_ref()
            .and_then(|currency| self.pnl_barrier_for(account_id, ctx.account_group(), currency));
        let account_currency_price = if touches_position_accounting {
            match account_currency.as_ref() {
                Some(account_currency) => match self.account_currency_price(
                    account_id,
                    ctx,
                    settlement_asset,
                    account_currency,
                    trade.price,
                )? {
                    Some(price) => Some(price),
                    None if pnl_barrier.is_some() => {
                        return Err(self.pnl_missing_fx_block(
                            account_id,
                            settlement_asset,
                            account_currency,
                        ));
                    }
                    None => None,
                },
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

        // Incoming reconciliation: the acquiring leg drains the projected inflow
        // for this fill. A buy acquires base units, so the underlying leg drains
        // `filled_q` (quantity-based, no price); a priced sell acquires quote
        // proceeds, so the settlement leg drains `max(0, lock*filled_q)`. The
        // non-acquiring leg drains nothing. `incoming` never feeds the available
        // credit - it is reconciled independently.
        let underlying_incoming_consume = match side {
            Side::Buy => qty_pos,
            Side::Sell => PositionSize::ZERO,
        };
        let settlement_incoming_consume =
            self.settlement_incoming_amount(account_id, settlement_asset, side, trade, lock)?;

        let controlled_fee_pnl_delta = match (account_currency.as_ref(), fee) {
            (Some(account_currency), Some(fee)) => self.fee_pnl_delta_if_controlled(
                account_id,
                ctx,
                fee,
                account_currency,
                pnl_barrier.as_ref(),
            )?,
            _ => None,
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
            underlying_incoming_consume,
            account_currency_price,
        );
        let settlement_leg = (
            LegKind::Settlement,
            settlement_asset,
            settlement_consume,
            settlement_flow,
            settlement_incoming_consume,
            None,
        );
        let ordered = match side {
            Side::Buy => [settlement_leg, underlying_leg],
            Side::Sell => [underlying_leg, settlement_leg],
        };
        for (kind, asset, consume, flow, incoming_consume, realize_price) in ordered {
            self.settle_fill_leg(
                account_id,
                asset,
                kind,
                consume,
                flow,
                incoming_consume,
                realize_price,
                deltas,
            )?;
        }
        let fee_pnl_delta = match (controlled_fee_pnl_delta, account_currency.as_ref(), fee) {
            (Some(delta), _, _) => Some(delta),
            (None, Some(account_currency), Some(fee)) => self.fee_pnl_delta_for_tracked_slot(
                account_id,
                ctx,
                underlying_asset,
                fee,
                account_currency,
                deltas,
            )?,
            _ => None,
        };
        if let Some(fee) = fee {
            self.apply_fee_debit(account_id, underlying_asset, settlement_asset, fee, deltas)?;
        }
        let account_pnl_delta = match (pnl_barrier.as_ref(), fee_pnl_delta) {
            (Some(_), Some(fee_delta)) => {
                let position_delta = deltas.underlying.pnl_delta.unwrap_or(Pnl::ZERO);
                Some(position_delta.checked_add(fee_delta).map_err(|_| {
                    self.pnl_arithmetic_failed_block(format!(
                        "spot-funds fill pnl + fee pnl overflow: pnl {position_delta}, \
                         fee pnl {fee_delta}, account {account_id}"
                    ))
                })?)
            }
            (Some(_), None) => Some(deltas.underlying.pnl_delta.unwrap_or(Pnl::ZERO)),
            (None, _) => None,
        };
        // Folding enriches the per-position cumulative value and outcome. It
        // does not replace the independent account-level fee path above.
        let fold_block = self
            .fold_fee_into_realized_pnl(account_id, underlying_asset, fee_pnl_delta, deltas)
            .err();
        let account_pnl_block = if let (Some(account_currency), Some(barrier), Some(delta)) = (
            account_currency.as_ref(),
            pnl_barrier.as_ref(),
            account_pnl_delta,
        ) {
            self.apply_account_pnl_delta(account_id, account_currency, barrier, delta)
        } else {
            None
        };
        if let Some(block) = fold_block.or(account_pnl_block) {
            return Err(block);
        }
        Ok(())
    }

    /// Reconciles one asset leg of a fill: `held -= consume`,
    /// `available += consume + flow_received`, and `incoming -= incoming_consume`,
    /// recorded into `deltas`.
    ///
    /// `consume` is the (non-negative) reserved portion this fill releases from
    /// `held`; `flow_received` is the signed cash/asset flow into `available`
    /// (positive inflow, negative outflow); `incoming_consume` drains the
    /// acquiring leg's projected inflow (never folded into `available`). When all
    /// three are zero the leg is left untouched and no outcome is emitted.
    #[allow(clippy::too_many_arguments)]
    fn settle_fill_leg(
        &self,
        account_id: AccountId,
        asset: &Asset,
        kind: LegKind,
        consume: PositionSize,
        flow_received: PositionSize,
        incoming_consume: PositionSize,
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
        // `incoming_consume` is reconciled separately and never enters this sum.
        let balance_credit = consume.checked_add(flow_received).map_err(|_| {
            arithmetic_overflow_account_block(
                Self::NAME,
                format!(
                    "fill balance credit overflow: account {account_id}, asset {asset}, \
                     consume {consume}, flow {flow_received}"
                ),
            )
        })?;
        if consume.is_zero() && balance_credit.is_zero() && incoming_consume.is_zero() {
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
                let after_credit = if balance_credit.is_zero() {
                    after_outflow
                } else {
                    after_outflow.apply_fill_inflow(balance_credit)?
                };
                // Drain the projected inflow independently of the available
                // credit; this never adds back to available.
                if incoming_consume.is_zero() {
                    Ok(after_credit)
                } else {
                    after_credit.consume_incoming(incoming_consume)
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
        leg.incoming_delta = leg
            .incoming_delta
            .checked_sub(incoming_consume)
            .map_err(|_| {
                arithmetic_overflow_account_block(
                    Self::NAME,
                    format!(
                        "fill incoming delta overflow: account {account_id}, asset {asset}, \
                     incoming {incoming_consume}"
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
    /// Returns `max(0, settlement_outflow_at_lock)` for the fill quantity. Both
    /// sides require a single lock price (a missing or duplicate price is an
    /// account-blocking error); a sell's `held` outflow is positive only at a
    /// negative price, zero otherwise.
    fn settlement_fill_consume(
        &self,
        account_id: AccountId,
        settlement_asset: &Asset,
        side: Side,
        trade: Trade,
        lock: &PreTradeLock,
    ) -> Result<PositionSize, AccountBlock> {
        let lock_price =
            settlement_lock_price(Self::NAME, lock, self.group_id(), "settlement fill")?;
        settlement_reserved_amount(
            Self::NAME,
            side,
            lock_price,
            trade.quantity,
            account_id,
            settlement_asset,
        )
    }

    /// Computes the settlement `incoming` consumed by one fill: the projected
    /// proceeds `max(0, lock_price * fill_qty)` for a sell, zero for a buy. Lock
    /// handling mirrors [`Self::settlement_fill_consume`] - the lock price is
    /// mandatory, and a missing lock blocks the account.
    fn settlement_incoming_amount(
        &self,
        account_id: AccountId,
        settlement_asset: &Asset,
        side: Side,
        trade: Trade,
        lock: &PreTradeLock,
    ) -> Result<PositionSize, AccountBlock> {
        let lock_price = settlement_lock_price(Self::NAME, lock, self.group_id(), "sell fill")?;
        settlement_incoming_proceeds(
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
        // Resolve the settlement lock up-front, before any leg mutation, so a
        // missing lock blocks the account with no holdings touched - symmetric to
        // the fill path. The settlement release amounts depend on the mandatory
        // lock price; computing them first keeps the block ahead of the
        // underlying mutation below.
        let settlement_held_release =
            self.settlement_release(account_id, settlement_asset, side, leaves_quantity, lock)?;
        let settlement_incoming_release = self.settlement_incoming_release(
            account_id,
            settlement_asset,
            side,
            leaves_quantity,
            lock,
        )?;

        // Underlying release: only sells reserved underlying held, by quantity;
        // only buys projected base incoming, by quantity. The unfilled remainder
        // of each is released here.
        let underlying_held_release = match side {
            Side::Buy => PositionSize::ZERO,
            Side::Sell => leaves_quantity.to_position_size(),
        };
        let underlying_incoming_release = match side {
            Side::Buy => leaves_quantity.to_position_size(),
            Side::Sell => PositionSize::ZERO,
        };
        self.release_leg(
            account_id,
            underlying_asset,
            LegKind::Underlying,
            underlying_held_release,
            underlying_incoming_release,
            deltas,
        )?;

        // Settlement release: the unfilled reserved settlement held remainder
        // (negative-price case) and the projected quote-incoming remainder
        // (priced sell).
        self.release_leg(
            account_id,
            settlement_asset,
            LegKind::Settlement,
            settlement_held_release,
            settlement_incoming_release,
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
            settlement_lock_price(Self::NAME, lock, self.group_id(), "settlement release")?;
        settlement_reserved_amount(
            Self::NAME,
            side,
            lock_price,
            leaves_quantity,
            account_id,
            settlement_asset,
        )
    }

    /// Computes the settlement `incoming` released on cancel: the projected
    /// proceeds remainder `max(0, lock_price * leaves_quantity)` for a priced
    /// sell, zero otherwise. Mirrors [`Self::settlement_incoming_amount`].
    fn settlement_incoming_release(
        &self,
        account_id: AccountId,
        settlement_asset: &Asset,
        side: Side,
        leaves_quantity: Quantity,
        lock: &PreTradeLock,
    ) -> Result<PositionSize, AccountBlock> {
        let lock_price = settlement_lock_price(Self::NAME, lock, self.group_id(), "sell release")?;
        settlement_incoming_proceeds(
            Self::NAME,
            side,
            lock_price,
            leaves_quantity,
            account_id,
            settlement_asset,
        )
    }

    /// Reconciles one asset leg of a cancel: `held -= held_release`,
    /// `available += held_release`, and `incoming -= incoming_release`, recorded
    /// into `deltas`. When both releases are zero the leg is a no-op. Held and
    /// incoming are folded into a single slot mutation so no concurrent check
    /// observes a half-released leg.
    fn release_leg(
        &self,
        account_id: AccountId,
        asset: &Asset,
        kind: LegKind,
        held_release: PositionSize,
        incoming_release: PositionSize,
        deltas: &mut FillCancelDeltas,
    ) -> Result<(), AccountBlock>
    where
        <<Sync as SyncMode>::StorageLockingPolicyFactory as crate::storage::LockingPolicyFactory>::Policy: 'static,
    {
        if held_release.is_zero() && incoming_release.is_zero() {
            return Ok(());
        }
        let new_h = self
            .mutate_slot((account_id, asset.clone()), |h| {
                let after_held = h.release(held_release)?;
                if incoming_release.is_zero() {
                    Ok(after_held)
                } else {
                    after_held.consume_incoming(incoming_release)
                }
            })
            .map_err(|_| {
                arithmetic_overflow_account_block(
                    Self::NAME,
                    format!(
                        "cancel release overflow: account {account_id}, asset {asset}, \
                         held {held_release}, incoming {incoming_release}"
                    ),
                )
            })?;
        let leg = deltas.leg_mut(kind);
        leg.held_delta = leg.held_delta.checked_sub(held_release).map_err(|_| {
            arithmetic_overflow_account_block(
                Self::NAME,
                format!(
                    "cancel held delta overflow: account {account_id}, asset {asset}, \
                     release {held_release}"
                ),
            )
        })?;
        leg.balance_delta = leg.balance_delta.checked_add(held_release).map_err(|_| {
            arithmetic_overflow_account_block(
                Self::NAME,
                format!(
                    "cancel balance delta overflow: account {account_id}, asset {asset}, \
                     release {held_release}"
                ),
            )
        })?;
        leg.incoming_delta = leg
            .incoming_delta
            .checked_sub(incoming_release)
            .map_err(|_| {
                arithmetic_overflow_account_block(
                    Self::NAME,
                    format!(
                        "cancel incoming delta overflow: account {account_id}, asset {asset}, \
                     release {incoming_release}"
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
            + HasExecutionReportFillFee
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
                request.fee.as_ref(),
                &request.lock,
                &mut deltas,
            ) {
                account_blocks.push(block);
            }
        } else if let Some(fee) = request.fee.as_ref() {
            // A structured fee belongs to the execution report, not to its
            // optional last trade. Fee-only corrections must reach balances
            // and account PnL through the same accounting rules.
            if let Err(block) = self.apply_execution_fee(
                ctx,
                request.account_id,
                &underlying_asset,
                &settlement_asset,
                fee,
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
        if let Some((fee_asset, fee_delta)) = deltas.fee {
            push_leg_outcome(
                &mut adjustments,
                group_id,
                fee_asset,
                &fee_delta,
                LegKind::Settlement,
            );
        }

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
/// Both sides require a lock price and block with
/// [`RejectCode::MissingRequiredField`] if it is missing. A buy reserved
/// settlement `held` and base `incoming`; a sell reserved settlement `incoming`
/// (or `held` at a negative price); both can only be reconciled at the recorded
/// price. Pre-trade records a lock price for every accepted order, so a missing
/// lock here is a reconciliation error, not a valid price-less order. The
/// caller converts the price into a signed per-unit outflow via the side.
fn settlement_lock_price(
    policy: &str,
    lock: &PreTradeLock,
    group_id: PolicyGroupId,
    purpose: &str,
) -> Result<Option<Price>, AccountBlock> {
    Ok(Some(single_lock_price(policy, lock, group_id, purpose)?))
}

/// Computes the reserved settlement `held` amount for `quantity`, given the lock
/// price and side: `max(0, settlement_outflow)`, where the outflow is
/// `+price*qty` for a buy and `-price*qty` for a sell. The lock price is
/// mandatory for every accepted order; the `None` guard is a defensive zero that
/// the strict lock resolution upstream no longer reaches.
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

/// Computes the projected settlement `incoming` for `quantity` given the lock
/// price and side: `max(0, +price*qty)`, the expected proceeds. Positive only
/// for a sell with a non-negative price; a buy reserves no settlement incoming.
/// The lock price is mandatory for every accepted order; the `None` guard is a
/// defensive zero that the strict lock resolution upstream no longer reaches.
fn settlement_incoming_proceeds(
    policy: &str,
    side: Side,
    lock_price: Option<Price>,
    quantity: Quantity,
    account_id: AccountId,
    settlement_asset: &Asset,
) -> Result<PositionSize, AccountBlock> {
    let Side::Sell = side else {
        return Ok(PositionSize::ZERO);
    };
    let Some(price) = lock_price else {
        return Ok(PositionSize::ZERO);
    };
    let notional = price.calculate_position_size(quantity).map_err(|_| {
        arithmetic_overflow_account_block(
            policy,
            format!(
                "settlement proceeds overflow: account {account_id}, \
                 asset {settlement_asset}, lock_px {price}, qty {quantity}"
            ),
        )
    })?;
    Ok(non_negative(notional))
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
                incoming: nonzero_outcome(leg.incoming_delta, h.incoming()),
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
