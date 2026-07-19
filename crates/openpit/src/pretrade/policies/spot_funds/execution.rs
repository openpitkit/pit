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

use crate::core::account_outcome::{
    AccountAdjustmentOutcome, AccountPnlOutcome, OutcomeAmount, PnlHaltReason, PnlOutcomeAmount,
};
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
use crate::pretrade::holdings::{AdjustmentOverflowError, Holdings, PositionPnlOperation};
use crate::pretrade::policy::{missing_required_field_account_block, PolicyGroupId};
use crate::pretrade::{AccountBlock, PostTradeContext, PostTradeResult, PreTradeLock, RejectCode};
use crate::storage::ConfigCell;

struct AccountPnlApplication {
    account_pnl: Option<AccountPnlOutcome>,
    account_block: Option<AccountBlock>,
}

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
    ) -> Option<Decimal>
    where
        <<Sync as SyncMode>::StorageLockingPolicyFactory as crate::storage::LockingPolicyFactory>::Policy: 'static,
    {
        if source_asset == account_currency {
            return Some(Decimal::ONE);
        }

        let direct = Instrument::new(source_asset.clone(), account_currency.clone());
        if let Some(mark) = self
            .accounting_quote(account_id, ctx, &direct)
            .and_then(|quote| quote.mark)
        {
            return Some(mark.to_decimal());
        }

        let inverse = Instrument::new(account_currency.clone(), source_asset.clone());
        if let Some(mark) = self
            .accounting_quote(account_id, ctx, &inverse)
            .and_then(|quote| quote.mark)
        {
            let factor = Decimal::ONE.checked_div(mark.to_decimal())?;
            return Some(factor);
        }

        None
    }

    fn account_currency_price(
        &self,
        account_id: AccountId,
        ctx: &PostTradeContext<<Sync as SyncMode>::StorageLockingPolicyFactory>,
        quote_asset: &Asset,
        account_currency: &Asset,
        trade_price: Price,
    ) -> Result<Option<Price>, PnlHaltReason>
    where
        <<Sync as SyncMode>::StorageLockingPolicyFactory as crate::storage::LockingPolicyFactory>::Policy: 'static,
    {
        let Some(factor) =
            self.account_currency_factor(account_id, ctx, quote_asset, account_currency)
        else {
            return Ok(None);
        };
        trade_price_with_factor(trade_price, factor).map(Some)
    }

    pub(super) fn pnl_barrier_for(
        &self,
        account_id: AccountId,
        account_group_id: Option<crate::param::AccountGroupId>,
    ) -> Option<super::SpotFundsPnlBoundsBarrier> {
        self.settings.with(|settings| {
            settings
                .pnl_barrier_for(account_id, account_group_id)
                .cloned()
        })
    }

    fn fee_pnl_delta(
        &self,
        account_id: AccountId,
        ctx: &PostTradeContext<<Sync as SyncMode>::StorageLockingPolicyFactory>,
        fee: &MonetaryAmount,
        account_currency: &Asset,
    ) -> Result<Option<Pnl>, PnlHaltReason>
    where
        <<Sync as SyncMode>::StorageLockingPolicyFactory as crate::storage::LockingPolicyFactory>::Policy: 'static,
    {
        let Some(factor) =
            self.account_currency_factor(account_id, ctx, &fee.currency, account_currency)
        else {
            return Ok(None);
        };
        let Some(value) = fee.amount.to_pnl().to_decimal().checked_mul(factor) else {
            return Err(PnlHaltReason::ArithmeticOverflow);
        };
        Ok(Some(Pnl::new(value)))
    }

    fn fee_account_pnl_delta(
        &self,
        account_id: AccountId,
        ctx: &PostTradeContext<<Sync as SyncMode>::StorageLockingPolicyFactory>,
        fee: &MonetaryAmount,
        account_currency: &Asset,
    ) -> Result<Pnl, PnlHaltReason>
    where
        <<Sync as SyncMode>::StorageLockingPolicyFactory as crate::storage::LockingPolicyFactory>::Policy: 'static,
    {
        if fee.amount.is_zero() {
            return Ok(Pnl::ZERO);
        }
        match self.fee_pnl_delta(account_id, ctx, fee, account_currency)? {
            Some(delta) => Ok(delta),
            None => Err(PnlHaltReason::MissingFx),
        }
    }

    // A fill realizes P&L only when it reduces, closes or reverses what is
    // owned. Yields the signed owned size for such a fill, and `None` when the
    // event's realized contribution is a computable zero.
    fn position_pnl_realizing_owned(
        holdings: Holdings,
        signed_quantity: PositionSize,
    ) -> Result<Option<PositionSize>, PnlHaltReason> {
        let owned = holdings
            .available()
            .checked_add(holdings.held())
            .map_err(|_| PnlHaltReason::ArithmeticOverflow)?;
        let owned_decimal = owned.to_decimal();
        let signed_quantity = signed_quantity.to_decimal();
        let realizes = owned_decimal != Decimal::ZERO
            && signed_quantity != Decimal::ZERO
            && (owned_decimal > Decimal::ZERO) != (signed_quantity > Decimal::ZERO);
        Ok(realizes.then_some(owned))
    }

    // Halt reasons name the operator action to take, so their choice follows
    // the contract priority rather than the order of cascade evaluation.
    fn pnl_halt_reason_priority(reason: PnlHaltReason) -> u8 {
        match reason {
            PnlHaltReason::ArithmeticOverflow => 0,
            PnlHaltReason::MissingAccountCurrency => 1,
            PnlHaltReason::MissingFx => 2,
            PnlHaltReason::MissingCostBasis => 3,
            PnlHaltReason::MissingInitialPnl => 4,
        }
    }

    fn select_pnl_halt_reason(halt_reason: &mut Option<PnlHaltReason>, candidate: PnlHaltReason) {
        let replace = match *halt_reason {
            Some(current) => {
                Self::pnl_halt_reason_priority(candidate) < Self::pnl_halt_reason_priority(current)
            }
            None => true,
        };
        if replace {
            *halt_reason = Some(candidate);
        }
    }

    // Account PnL derives the event contribution from quantity and cost basis,
    // never from the position ledger's sticky state or emitted delta.
    fn account_position_pnl_delta(
        holdings: Holdings,
        signed_quantity: PositionSize,
        price: Option<Price>,
    ) -> Result<Pnl, PnlHaltReason> {
        let Some(owned) = Self::position_pnl_realizing_owned(holdings, signed_quantity)? else {
            return Ok(Pnl::ZERO);
        };
        let price = price.ok_or(PnlHaltReason::MissingFx)?;
        let owned = owned.to_decimal();
        let signed_quantity = signed_quantity.to_decimal();
        let average_entry_price = holdings
            .avg_entry_price()
            .ok_or(PnlHaltReason::MissingCostBasis)?;
        let closing_quantity = if signed_quantity.abs() <= owned.abs() {
            -signed_quantity
        } else {
            owned
        };
        let price_difference = price
            .to_decimal()
            .checked_sub(average_entry_price.to_decimal())
            .ok_or(PnlHaltReason::ArithmeticOverflow)?;
        let realized = price_difference
            .checked_mul(closing_quantity)
            .ok_or(PnlHaltReason::ArithmeticOverflow)?;
        Ok(Pnl::new(realized))
    }

    fn record_position_pnl_operation(
        deltas: &mut FillCancelDeltas,
        operation: PositionPnlOperation,
    ) -> bool {
        // A silent nonzero economic delta is valid only while the position
        // ledger is halted; account PnL computes its delta independently.
        debug_assert!(
            operation.outcome().is_some()
                || operation.holdings().realized_pnl_is_halted()
                || operation
                    .realized_delta()
                    .map_or(true, |delta| delta.is_zero())
        );
        let leg = &mut deltas.underlying;
        let mut overflowed = false;
        leg.pnl_outcome = match (leg.pnl_outcome, operation.outcome()) {
            (current, None) => current,
            (_, Some(Err(reason))) => Some(Err(reason)),
            (None, Some(Ok(amount))) => Some(Ok(amount)),
            (Some(Ok(current)), Some(Ok(next))) => match current.delta.checked_add(next.delta) {
                Ok(delta) => (!delta.is_zero()).then_some(Ok(PnlOutcomeAmount {
                    delta,
                    absolute: next.absolute,
                })),
                Err(_) => {
                    overflowed = true;
                    Some(Err(PnlHaltReason::ArithmeticOverflow))
                }
            },
            (Some(Err(reason)), Some(Ok(_))) => Some(Err(reason)),
        };
        if let Some(average_entry_price) = operation.average_entry_price() {
            leg.average_entry_price = Some(average_entry_price);
        }
        overflowed
    }

    fn fold_fee_into_realized_pnl(
        &self,
        account_id: AccountId,
        underlying_asset: &Asset,
        fee_pnl_delta: Option<Pnl>,
        deltas: &mut FillCancelDeltas,
    )
    where
        <<Sync as SyncMode>::StorageLockingPolicyFactory as crate::storage::LockingPolicyFactory>::Policy: 'static,
    {
        let Some(fee_delta) = fee_pnl_delta.filter(|delta| !delta.is_zero()) else {
            return;
        };
        let Some((holdings, operation)) =
            self.holdings
                .with_mut_if_present(&(account_id, underlying_asset.clone()), |slot| {
                    let operation = slot.add_realized_pnl(fee_delta);
                    *slot = operation.holdings();
                    (*slot, operation)
                })
        else {
            return;
        };
        let aggregation_overflowed = Self::record_position_pnl_operation(deltas, operation);
        if aggregation_overflowed {
            self.halt_position_pnl(
                account_id,
                underlying_asset,
                PnlHaltReason::ArithmeticOverflow,
                deltas,
            );
            return;
        }
        if operation.outcome().is_some() {
            deltas.underlying.final_holdings = Some(holdings);
        }
    }

    fn halt_position_pnl(
        &self,
        account_id: AccountId,
        underlying_asset: &Asset,
        halt_reason: PnlHaltReason,
        deltas: &mut FillCancelDeltas,
    )
    where
        <<Sync as SyncMode>::StorageLockingPolicyFactory as crate::storage::LockingPolicyFactory>::Policy: 'static,
    {
        let (holdings, operation) = self.holdings.with_mut(
            (account_id, underlying_asset.clone()),
            Holdings::zero,
            |slot, _is_new| {
                let operation = slot.halt_realized_pnl_preserving_average(halt_reason);
                *slot = operation.holdings();
                (*slot, operation)
            },
        );
        let _ = Self::record_position_pnl_operation(deltas, operation);
        if operation.outcome().is_some() {
            deltas.underlying.final_holdings = Some(holdings);
        }
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
                        "fee debit overflow: currency {}, \
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
                    "fee balance delta overflow: \
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
    /// fee accounting must not depend on last_trade being present.
    #[allow(clippy::too_many_arguments)]
    fn apply_execution_fee(
        &self,
        ctx: &PostTradeContext<<Sync as SyncMode>::StorageLockingPolicyFactory>,
        account_id: AccountId,
        underlying_asset: &Asset,
        settlement_asset: &Asset,
        fee: &MonetaryAmount,
        deltas: &mut FillCancelDeltas,
    ) -> Result<AccountPnlApplication, AccountBlock>
    where
        <<Sync as SyncMode>::StorageLockingPolicyFactory as crate::storage::LockingPolicyFactory>::Policy: 'static,
    {
        if fee.amount.is_zero() {
            return Ok(AccountPnlApplication {
                account_pnl: None,
                account_block: None,
            });
        }

        let account_currency = ctx.account_currency();
        let account_pnl_halt_reason_before = match self.account_pnl_state(account_id) {
            crate::PnlState::Value(_) => None,
            crate::PnlState::Halted(reason) => Some(reason),
        };
        let position_pnl_was_halted = self
            .holdings
            .with(&(account_id, underlying_asset.clone()), |holdings| {
                holdings.realized_pnl_is_halted()
            })
            .unwrap_or(false);
        let mut position_pnl_halt = false;
        let mut position_pnl_halt_reason = None;
        let pnl_barrier = self.pnl_barrier_for(account_id, ctx.account_group());
        let mut account_pnl_halt_reason = None;
        let fee_pnl_delta = match account_currency.as_ref() {
            Some(account_currency) => {
                match self.fee_account_pnl_delta(account_id, ctx, fee, account_currency) {
                    Ok(delta) => Some(delta),
                    Err(reason) => {
                        position_pnl_halt = true;
                        if !position_pnl_was_halted {
                            Self::select_pnl_halt_reason(&mut position_pnl_halt_reason, reason);
                        }
                        if account_pnl_halt_reason_before.is_none() {
                            Self::select_pnl_halt_reason(&mut account_pnl_halt_reason, reason);
                        }
                        None
                    }
                }
            }
            None => {
                position_pnl_halt = true;
                if !position_pnl_was_halted {
                    Self::select_pnl_halt_reason(
                        &mut position_pnl_halt_reason,
                        PnlHaltReason::MissingAccountCurrency,
                    );
                }
                if account_pnl_halt_reason_before.is_none() {
                    Self::select_pnl_halt_reason(
                        &mut account_pnl_halt_reason,
                        PnlHaltReason::MissingAccountCurrency,
                    );
                }
                None
            }
        };

        self.apply_fee_debit(account_id, underlying_asset, settlement_asset, fee, deltas)?;
        if let Some(reason) = position_pnl_halt_reason {
            self.halt_position_pnl(account_id, underlying_asset, reason, deltas);
        }
        self.fold_fee_into_realized_pnl(
            account_id,
            underlying_asset,
            if position_pnl_halt {
                None
            } else {
                fee_pnl_delta
            },
            deltas,
        );
        let stored_halt_block = account_pnl_halt_reason_before.and_then(|reason| {
            pnl_barrier
                .as_ref()
                .map(|_| self.account_pnl_halted_block(account_id, reason))
        });
        let (account_pnl, new_halt_block) = match (account_pnl_halt_reason, fee_pnl_delta) {
            (None, Some(delta)) if account_pnl_halt_reason_before.is_none() => {
                let (result, block) =
                    self.apply_account_pnl_delta(account_id, pnl_barrier.as_ref(), delta);
                (
                    Some(AccountPnlOutcome {
                        result,
                        account_id,
                        policy_group_id: self.group_id(),
                    }),
                    block,
                )
            }
            (Some(reason), _) if account_pnl_halt_reason_before.is_none() => {
                let reason = self.halt_account_pnl(account_id, reason);
                let block = pnl_barrier
                    .as_ref()
                    .map(|_| self.account_pnl_halted_block(account_id, reason));
                (
                    Some(AccountPnlOutcome {
                        result: Err(reason),
                        account_id,
                        policy_group_id: self.group_id(),
                    }),
                    block,
                )
            }
            _ => (None, None),
        };
        Ok(AccountPnlApplication {
            account_pnl,
            account_block: stored_halt_block.or(new_halt_block),
        })
    }

    pub(super) fn update_account_pnl(
        &self,
        account_id: AccountId,
        delta: Pnl,
    ) -> (crate::core::PnlOutcome, Option<u64>)
    where
        <<Sync as SyncMode>::StorageLockingPolicyFactory as crate::storage::LockingPolicyFactory>::Policy: 'static,
    {
        let first_attempt = self.pnl.with_mut(
            account_id,
            super::AccountPnlEntry::zero,
            |entry, _is_new| {
                let previous = match entry.state {
                    crate::PnlState::Value(previous) => previous,
                    crate::PnlState::Halted(reason) => {
                        return Some((Err(reason), entry.assertion_token));
                    }
                };
                match previous.checked_add(delta) {
                    Ok(updated) => {
                        entry.state = crate::PnlState::Value(updated);
                        Some((
                            Ok(PnlOutcomeAmount {
                                delta,
                                absolute: updated,
                            }),
                            entry.assertion_token,
                        ))
                    }
                    Err(_) => None,
                }
            },
        );
        if let Some(result) = first_attempt {
            return result;
        }

        // Overflow is an absolute transition. Wait for any provisional
        // force-set to finalize, then recompute from the standing base before
        // committing the halt.
        let owner_id = crate::core::mutation::next_mutation_owner_id();
        let _lease = self.acquire_account_pnl_lease(account_id, owner_id);
        let result = self.pnl.with_mut(
            account_id,
            super::AccountPnlEntry::zero,
            |entry, _is_new| match entry.state {
                crate::PnlState::Value(previous) => match previous.checked_add(delta) {
                    Ok(updated) => {
                        entry.state = crate::PnlState::Value(updated);
                        Ok(PnlOutcomeAmount {
                            delta,
                            absolute: updated,
                        })
                    }
                    Err(_) => {
                        *entry = super::AccountPnlEntry {
                            state: crate::PnlState::Halted(PnlHaltReason::ArithmeticOverflow),
                            assertion_token: None,
                        };
                        Err(PnlHaltReason::ArithmeticOverflow)
                    }
                },
                crate::PnlState::Halted(reason) => Err(reason),
            },
        );
        (result, None)
    }

    pub(super) fn halt_account_pnl(
        &self,
        account_id: AccountId,
        reason: PnlHaltReason,
    ) -> PnlHaltReason {
        let owner_id = crate::core::mutation::next_mutation_owner_id();
        let _lease = self.acquire_account_pnl_lease(account_id, owner_id);
        self.pnl.with_mut(
            account_id,
            super::AccountPnlEntry::zero,
            |entry, _is_new| match entry.state {
                crate::PnlState::Value(_) => {
                    *entry = super::AccountPnlEntry {
                        state: crate::PnlState::Halted(reason),
                        assertion_token: None,
                    };
                    reason
                }
                crate::PnlState::Halted(existing) => existing,
            },
        )
    }

    pub(super) fn account_pnl_halted_block(
        &self,
        account_id: AccountId,
        reason: PnlHaltReason,
    ) -> AccountBlock {
        super::rejects::account_pnl_halted_block(Self::NAME, account_id, reason)
    }

    fn apply_account_pnl_delta(
        &self,
        account_id: AccountId,
        barrier: Option<&super::SpotFundsPnlBoundsBarrier>,
        delta: Pnl,
    ) -> (crate::core::PnlOutcome, Option<AccountBlock>)
    where
        <<Sync as SyncMode>::StorageLockingPolicyFactory as crate::storage::LockingPolicyFactory>::Policy: 'static,
    {
        let (result, provenance) = self.update_account_pnl(account_id, delta);
        let state = match result {
            Ok(amount) => crate::PnlState::Value(amount.absolute),
            Err(reason) => crate::PnlState::Halted(reason),
        };
        let block = barrier.and_then(|barrier| {
            super::rejects::account_pnl_block_for_state(account_id, state, barrier, provenance)
        });
        (result, block)
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
    /// Each leg moves money in its signed flow direction: the reserved held is
    /// consumed by the portion of this fill that was actually reserved
    /// (max(0, outflow)), and available absorbs the net of the consumed
    /// reservation and the real signed cash flow. A leg that reserved nothing
    /// (for example, the settlement of a buy at a negative price) simply
    /// credits the inflow to available.
    ///
    /// Any [AccountBlock] returned (for example, overflow or a missing lock
    /// price on either side) is propagated up to
    /// [Self::apply_execution_report_impl] and collected into
    /// [PostTradeResult::account_blocks]; the engine's
    /// [BlockedAccounts](crate::core::BlockedAccounts) records the first block
    /// for the account, so policy code does not need to wire a separate sink.
    #[allow(clippy::too_many_arguments)]
    fn apply_trade_fill(
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
    ) -> Result<AccountPnlApplication, AccountBlock>
    where
        <<Sync as SyncMode>::StorageLockingPolicyFactory as crate::storage::LockingPolicyFactory>::Policy: 'static,
    {
        // A zero fee is a complete P&L no-op: keeping only the economic fee
        // here prevents that fee from engaging either ledger or requiring an
        // account currency later in this fill.
        let nonzero_fee = fee.filter(|fee| !fee.amount.is_zero());
        let qty_pos = trade.quantity.to_position_size();
        let settlement_notional = trade
            .price
            .calculate_position_size(trade.quantity)
            .map_err(|_| {
                arithmetic_overflow_account_block(
                    Self::NAME,
                    format!(
                        "fill notional volume overflow: \
                         asset {settlement_asset}, px {}, qty {}",
                        trade.price, trade.quantity,
                    ),
                )
            })?;

        let (underlying_consume, underlying_flow) = match side {
            Side::Buy => (PositionSize::ZERO, qty_pos),
            Side::Sell => (qty_pos, neg(qty_pos)),
        };
        let touches_position_accounting = !underlying_flow.is_zero();
        let account_pnl_engaged = touches_position_accounting || nonzero_fee.is_some();
        let account_pnl_halt_reason_before = match self.account_pnl_state(account_id) {
            crate::PnlState::Value(_) => None,
            crate::PnlState::Halted(reason) => Some(reason),
        };
        let position_pnl_halt_reason_before = self
            .holdings
            .with(&(account_id, underlying_asset.clone()), |h| {
                h.realized_pnl_halt_reason()
            })
            .flatten();
        let position_pnl_was_halted = position_pnl_halt_reason_before.is_some();
        let account_currency = ctx.account_currency();
        let pnl_barrier = self.pnl_barrier_for(account_id, ctx.account_group());
        let position_pnl_price_requirement = if touches_position_accounting {
            self.holdings
                .with(&(account_id, underlying_asset.clone()), |holdings| {
                    Self::position_pnl_realizing_owned(*holdings, underlying_flow)
                        .map(|owned| owned.is_some())
                })
                .unwrap_or(Ok(false))
        } else {
            Ok(false)
        };
        // The account line needs the account currency only for a contribution
        // it has to denominate in it: a non-zero fee to convert, or a fill
        // that was not established as a computable zero. The position ledger
        // is stricter: it stores a cost basis for every fill, hence the
        // separate check below.
        let account_pnl_requires_currency =
            nonzero_fee.is_some() || !matches!(position_pnl_price_requirement, Ok(false));
        let mut position_pnl_halt = false;
        let mut position_pnl_halt_reason = None;
        let mut account_pnl_halt_reason = None;
        if account_pnl_halt_reason_before.is_none()
            && account_pnl_requires_currency
            && account_currency.is_none()
        {
            Self::select_pnl_halt_reason(
                &mut account_pnl_halt_reason,
                PnlHaltReason::MissingAccountCurrency,
            );
        }
        if touches_position_accounting && account_currency.is_none() {
            position_pnl_halt = true;
            if !position_pnl_was_halted {
                Self::select_pnl_halt_reason(
                    &mut position_pnl_halt_reason,
                    PnlHaltReason::MissingAccountCurrency,
                );
            }
        }
        let position_pnl_requires_price = match position_pnl_price_requirement {
            Ok(requires_price) => requires_price,
            Err(reason) => {
                position_pnl_halt = true;
                if !position_pnl_was_halted {
                    Self::select_pnl_halt_reason(&mut position_pnl_halt_reason, reason);
                }
                Self::select_pnl_halt_reason(&mut account_pnl_halt_reason, reason);
                false
            }
        };
        let account_currency_price = if touches_position_accounting {
            match account_currency.as_ref() {
                Some(account_currency) => match self.account_currency_price(
                    account_id,
                    ctx,
                    settlement_asset,
                    account_currency,
                    trade.price,
                ) {
                    Ok(Some(price)) => Some(price),
                    Ok(None) if position_pnl_requires_price => {
                        position_pnl_halt = true;
                        if !position_pnl_was_halted {
                            Self::select_pnl_halt_reason(
                                &mut position_pnl_halt_reason,
                                PnlHaltReason::MissingFx,
                            );
                        }
                        Self::select_pnl_halt_reason(
                            &mut account_pnl_halt_reason,
                            PnlHaltReason::MissingFx,
                        );
                        None
                    }
                    Ok(None) => None,
                    Err(reason) if position_pnl_requires_price => {
                        position_pnl_halt = true;
                        if !position_pnl_was_halted {
                            Self::select_pnl_halt_reason(&mut position_pnl_halt_reason, reason);
                        }
                        Self::select_pnl_halt_reason(&mut account_pnl_halt_reason, reason);
                        None
                    }
                    Err(_) => None,
                },
                None => None,
            }
        } else {
            None
        };
        let settlement_consume =
            self.settlement_fill_consume(settlement_asset, side, trade, lock)?;
        let settlement_flow = match side {
            Side::Buy => neg(settlement_notional),
            Side::Sell => settlement_notional,
        };

        let underlying_incoming_consume = match side {
            Side::Buy => qty_pos,
            Side::Sell => PositionSize::ZERO,
        };
        let settlement_incoming_consume =
            self.settlement_incoming_amount(settlement_asset, side, trade, lock)?;

        let fee_pnl_delta = match (account_currency.as_ref(), nonzero_fee) {
            (Some(account_currency), Some(fee)) => {
                match self.fee_account_pnl_delta(account_id, ctx, fee, account_currency) {
                    Ok(delta) => Some(delta),
                    Err(reason) => {
                        position_pnl_halt = true;
                        if !position_pnl_was_halted {
                            Self::select_pnl_halt_reason(&mut position_pnl_halt_reason, reason);
                        }
                        Self::select_pnl_halt_reason(&mut account_pnl_halt_reason, reason);
                        None
                    }
                }
            }
            _ => None,
        };

        // An unpriced closing fill carries the fresh or sticky halt reason.
        // Opening and same-direction fills may remain active without FX, but
        // cannot retain an authoritative average entry price.
        let underlying_pnl_halt_reason = if touches_position_accounting {
            position_pnl_halt_reason.or(position_pnl_halt_reason_before)
        } else {
            None
        };
        let underlying_leg = (
            LegKind::Underlying,
            underlying_asset,
            underlying_consume,
            underlying_flow,
            underlying_incoming_consume,
            account_currency_price,
            underlying_pnl_halt_reason,
        );
        let settlement_leg = (
            LegKind::Settlement,
            settlement_asset,
            settlement_consume,
            settlement_flow,
            settlement_incoming_consume,
            None,
            None,
        );
        let ordered = match side {
            Side::Buy => [settlement_leg, underlying_leg],
            Side::Sell => [underlying_leg, settlement_leg],
        };
        let mut account_position_pnl_delta = None;
        for (
            kind,
            asset,
            consume,
            flow,
            incoming_consume,
            realize_price,
            position_pnl_halt_reason,
        ) in ordered
        {
            let economic_delta = self.settle_fill_leg(
                account_id,
                asset,
                kind,
                consume,
                flow,
                incoming_consume,
                realize_price,
                position_pnl_halt_reason,
                deltas,
            )?;
            if kind == LegKind::Underlying {
                account_position_pnl_delta = economic_delta;
            }
        }
        if let Some(fee) = nonzero_fee {
            self.apply_fee_debit(account_id, underlying_asset, settlement_asset, fee, deltas)?;
        }
        if let Some(reason) = position_pnl_halt_reason {
            if !matches!(deltas.underlying.pnl_outcome, Some(Err(_))) {
                self.halt_position_pnl(account_id, underlying_asset, reason, deltas);
            }
        }

        let account_pnl_delta = match (
            account_pnl_engaged,
            account_pnl_halt_reason_before.is_some(),
            account_pnl_halt_reason,
        ) {
            (true, false, None) => {
                let position_delta = if touches_position_accounting {
                    match account_position_pnl_delta {
                        Some(Ok(delta)) => Some(delta),
                        Some(Err(reason)) => {
                            Self::select_pnl_halt_reason(&mut account_pnl_halt_reason, reason);
                            None
                        }
                        None => {
                            Self::select_pnl_halt_reason(
                                &mut account_pnl_halt_reason,
                                PnlHaltReason::MissingInitialPnl,
                            );
                            None
                        }
                    }
                } else {
                    Some(Pnl::ZERO)
                };
                let fee_delta = if nonzero_fee.is_some() {
                    fee_pnl_delta
                } else {
                    Some(Pnl::ZERO)
                };
                match (position_delta, fee_delta) {
                    (Some(position_delta), Some(fee_delta)) => {
                        match position_delta.checked_add(fee_delta) {
                            Ok(delta) => Some(delta),
                            Err(_) => {
                                Self::select_pnl_halt_reason(
                                    &mut account_pnl_halt_reason,
                                    PnlHaltReason::ArithmeticOverflow,
                                );
                                None
                            }
                        }
                    }
                    _ => None,
                }
            }
            _ => None,
        };
        self.fold_fee_into_realized_pnl(
            account_id,
            underlying_asset,
            if position_pnl_halt {
                None
            } else {
                fee_pnl_delta
            },
            deltas,
        );
        let stored_halt_block = if account_pnl_engaged {
            account_pnl_halt_reason_before.and_then(|reason| {
                pnl_barrier
                    .as_ref()
                    .map(|_| self.account_pnl_halted_block(account_id, reason))
            })
        } else {
            None
        };
        let (account_pnl, new_halt_block) = match (account_pnl_halt_reason, account_pnl_delta) {
            (Some(halt_reason), _)
                if account_pnl_engaged && account_pnl_halt_reason_before.is_none() =>
            {
                let halt_reason = self.halt_account_pnl(account_id, halt_reason);
                let block = pnl_barrier
                    .as_ref()
                    .map(|_| self.account_pnl_halted_block(account_id, halt_reason));
                (
                    Some(AccountPnlOutcome {
                        result: Err(halt_reason),
                        account_id,
                        policy_group_id: self.group_id(),
                    }),
                    block,
                )
            }
            (None, Some(delta)) => {
                let (result, block) =
                    self.apply_account_pnl_delta(account_id, pnl_barrier.as_ref(), delta);
                (
                    Some(AccountPnlOutcome {
                        result,
                        account_id,
                        policy_group_id: self.group_id(),
                    }),
                    block,
                )
            }
            _ => (None, None),
        };
        Ok(AccountPnlApplication {
            account_pnl,
            account_block: stored_halt_block.or(new_halt_block),
        })
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
        position_pnl_halt_reason: Option<PnlHaltReason>,
        deltas: &mut FillCancelDeltas,
    ) -> Result<Option<Result<Pnl, PnlHaltReason>>, AccountBlock>
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
                    "fill balance credit overflow: asset {asset}, \
                     consume {consume}, flow {flow_received}"
                ),
            )
        })?;
        if consume.is_zero() && balance_credit.is_zero() && incoming_consume.is_zero() {
            return Ok(None);
        }

        let mut pnl_operation = None;
        let mut account_pnl_delta = None;
        let new_h = self
            .mutate_slot((account_id, asset.clone()), |h| {
                if kind == LegKind::Underlying {
                    account_pnl_delta = Some(Self::account_position_pnl_delta(
                        h,
                        flow_received,
                        realize_price,
                    ));
                }
                let realized = match (kind, realize_price, position_pnl_halt_reason) {
                    (LegKind::Underlying, Some(price), _) => {
                        let updated = match h.realize_position_fill(flow_received, price) {
                            Ok(updated) => updated,
                            Err(_) => h.halt_realized_pnl(PnlHaltReason::ArithmeticOverflow),
                        };
                        pnl_operation = Some(updated);
                        updated.holdings()
                    }
                    (LegKind::Underlying, None, Some(reason)) => {
                        let updated =
                            match h.halt_realized_pnl_for_unpriced_fill(flow_received, reason) {
                                Ok(updated) => updated,
                                Err(_) => h.halt_realized_pnl(PnlHaltReason::ArithmeticOverflow),
                            };
                        pnl_operation = Some(updated);
                        updated.holdings()
                    }
                    (LegKind::Underlying, None, None) => h.with_avg_entry_price(None),
                    (LegKind::Settlement, _, _) => h,
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
                        "fill leg mutation overflow: asset {asset}, \
                         consume {consume}, credit {balance_credit}"
                    ),
                )
            })?;

        if let Some(operation) = pnl_operation {
            if Self::record_position_pnl_operation(deltas, operation) {
                self.halt_position_pnl(
                    account_id,
                    asset,
                    PnlHaltReason::ArithmeticOverflow,
                    deltas,
                );
            }
        }

        let leg = deltas.leg_mut(kind);
        leg.held_delta = leg.held_delta.checked_sub(consume).map_err(|_| {
            arithmetic_overflow_account_block(
                Self::NAME,
                format!(
                    "fill held delta overflow: asset {asset}, \
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
                        "fill incoming delta overflow: asset {asset}, \
                     incoming {incoming_consume}"
                    ),
                )
            })?;
        leg.balance_delta = leg.balance_delta.checked_add(balance_credit).map_err(|_| {
            arithmetic_overflow_account_block(
                Self::NAME,
                format!(
                    "fill balance delta overflow: asset {asset}, \
                     credit {balance_credit}"
                ),
            )
        })?;
        leg.final_holdings = Some(new_h);
        Ok(account_pnl_delta)
    }

    /// Computes the settlement `held` consumed by one fill.
    ///
    /// Returns `max(0, settlement_outflow_at_lock)` for the fill quantity. Both
    /// sides require a single lock price (a missing or duplicate price is an
    /// account-blocking error); a sell's `held` outflow is positive only at a
    /// negative price, zero otherwise.
    fn settlement_fill_consume(
        &self,
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
            settlement_asset,
        )
    }

    /// Computes the settlement `incoming` consumed by one fill: the projected
    /// proceeds `max(0, lock_price * fill_qty)` for a sell, zero for a buy. Lock
    /// handling mirrors [`Self::settlement_fill_consume`] - the lock price is
    /// mandatory, and a missing lock blocks the account.
    fn settlement_incoming_amount(
        &self,
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
            self.settlement_release(settlement_asset, side, leaves_quantity, lock)?;
        let settlement_incoming_release =
            self.settlement_incoming_release(settlement_asset, side, leaves_quantity, lock)?;

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
            settlement_asset,
        )
    }

    /// Computes the settlement `incoming` released on cancel: the projected
    /// proceeds remainder `max(0, lock_price * leaves_quantity)` for a priced
    /// sell, zero otherwise. Mirrors [`Self::settlement_incoming_amount`].
    fn settlement_incoming_release(
        &self,
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
                        "cancel release overflow: asset {asset}, \
                         held {held_release}, incoming {incoming_release}"
                    ),
                )
            })?;
        let leg = deltas.leg_mut(kind);
        leg.held_delta = leg.held_delta.checked_sub(held_release).map_err(|_| {
            arithmetic_overflow_account_block(
                Self::NAME,
                format!(
                    "cancel held delta overflow: asset {asset}, \
                     release {held_release}"
                ),
            )
        })?;
        leg.balance_delta = leg.balance_delta.checked_add(held_release).map_err(|_| {
            arithmetic_overflow_account_block(
                Self::NAME,
                format!(
                    "cancel balance delta overflow: asset {asset}, \
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
                        "cancel incoming delta overflow: asset {asset}, \
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
        let mut account_pnls: Vec<AccountPnlOutcome> = Vec::with_capacity(1);
        let mut deltas = FillCancelDeltas::new();

        if let Some(trade) = request.last_trade {
            match self.apply_trade_fill(
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
                Ok(application) => {
                    if let Some(account_pnl) = application.account_pnl {
                        account_pnls.push(account_pnl);
                    }
                    if let Some(block) = application.account_block {
                        account_blocks.push(block);
                    }
                }
                Err(block) => account_blocks.push(block),
            }
        } else if let Some(fee) = request.fee.as_ref() {
            // A structured fee belongs to the execution report, not to its
            // optional last trade. Fee-only corrections must reach balances
            // and account PnL through the same accounting rules.
            match self.apply_execution_fee(
                ctx,
                request.account_id,
                &underlying_asset,
                &settlement_asset,
                fee,
                &mut deltas,
            ) {
                Ok(application) => {
                    if let Some(account_pnl) = application.account_pnl {
                        account_pnls.push(account_pnl);
                    }
                    if let Some(block) = application.account_block {
                        account_blocks.push(block);
                    }
                }
                Err(block) => account_blocks.push(block),
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

        if account_blocks.is_empty() && account_pnls.is_empty() && adjustments.is_empty() {
            None
        } else {
            Some(PostTradeResult {
                account_blocks,
                account_pnls,
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
    settlement_asset: &Asset,
) -> Result<PositionSize, AccountBlock> {
    let Some(price) = lock_price else {
        return Ok(PositionSize::ZERO);
    };
    let notional = price.calculate_position_size(quantity).map_err(|_| {
        arithmetic_overflow_account_block(
            policy,
            format!(
                "settlement notional overflow: \
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
                "settlement proceeds overflow: \
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

fn trade_price_with_factor(price: Price, factor: Decimal) -> Result<Price, PnlHaltReason> {
    price
        .to_decimal()
        .checked_mul(factor)
        .map(Price::new)
        .ok_or(PnlHaltReason::ArithmeticOverflow)
}

/// Appends a per-asset outcome entry for a leg, omitting zero-delta fields and
/// the entry entirely when the leg was never touched.
///
/// Realized PnL and the average entry price are emitted only for the underlying
/// leg. `realized_pnl` comes directly from that position's operation result:
/// either the changed PnL or the halt reason from this operation. The
/// settlement leg never realizes PnL and carries no average.
fn push_leg_outcome(
    adjustments: &mut Vec<AccountAdjustmentOutcome>,
    group_id: PolicyGroupId,
    asset: Asset,
    leg: &LegDelta,
    kind: LegKind,
) {
    if let Some(h) = leg.final_holdings {
        let (realized_pnl, average_entry_price) = match kind {
            LegKind::Underlying => match leg.pnl_outcome {
                Some(Err(reason)) => (Some(Err(reason)), None),
                Some(Ok(amount)) => (Some(Ok(amount)), leg.average_entry_price),
                None => (None, leg.average_entry_price),
            },
            LegKind::Settlement => (None, None),
        };
        let balance = nonzero_outcome(leg.balance_delta, h.available());
        let held = nonzero_outcome(leg.held_delta, h.held());
        let incoming = nonzero_outcome(leg.incoming_delta, h.incoming());
        if balance.is_none()
            && held.is_none()
            && incoming.is_none()
            && realized_pnl.is_none()
            && average_entry_price.is_none()
        {
            return;
        }
        adjustments.push(AccountAdjustmentOutcome {
            policy_group_id: group_id,
            entry: AccountOutcomeEntry {
                asset,
                balance,
                held,
                incoming,
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
