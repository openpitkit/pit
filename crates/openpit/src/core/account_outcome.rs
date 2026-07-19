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

use crate::core::PolicyGroupId;
use crate::param::{AccountId, Asset, Pnl, PositionSize, Price};
use crate::pretrade::AccountBlock;

/// A delta/absolute pair for one position field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutcomeAmount {
    /// Signed change applied by this operation relative to the field value at operation start.
    ///
    /// This is the authoritative value for position bookkeeping. Apply each operation's
    /// `delta` sequentially to an external position store to maintain consistency
    /// regardless of operation ordering or concurrency.
    pub delta: PositionSize,
    /// Field value at the moment the policy returned, **before** deferred commit.
    ///
    /// This snapshot is taken under the policy's internal lock, but it may be
    /// superseded by the time the caller reads it — a concurrent operation on
    /// another thread can change the slot between the policy write and the
    /// caller's read. Use `delta` as the source of truth for position bookkeeping;
    /// treat `absolute` as a convenience hint only.
    pub absolute: PositionSize,
}

/// A delta/absolute pair for a realized PnL field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PnlOutcomeAmount {
    /// Signed PnL change applied by this operation.
    pub delta: Pnl,
    /// Cumulative realized PnL after this operation.
    pub absolute: Pnl,
}

/// Result of one realized-PnL calculation.
///
/// The value is authoritative only when the result is `Ok`. An `Err` explains
/// why the calculation halted for the current operation.
pub type PnlOutcome = Result<PnlOutcomeAmount, PnlHaltReason>;

/// Current state of a realized-PnL accumulator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PnlState {
    /// Authoritative accumulated PnL value.
    Value(Pnl),
    /// Calculation is stopped until an explicit numeric correction re-arms it.
    /// A halted correction keeps it halted and replaces the stored reason.
    Halted(PnlHaltReason),
}

/// Account-level realized PnL outcome for one account ledger.
///
/// SpotFunds emits a halted outcome only for the operation that transitions
/// the ledger to halted. Later operations omit the unchanged halt until an
/// explicit account-PnL correction re-arms the ledger.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountPnlOutcome {
    /// Account PnL, or the reason why it is unavailable.
    pub result: PnlOutcome,
    /// Account that owns the realized-PnL ledger.
    pub account_id: AccountId,
    /// Policy-group tag of the policy that produced this outcome.
    pub policy_group_id: PolicyGroupId,
}

/// Reason why a realized-PnL calculation halted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PnlHaltReason {
    /// A required FX quote was unavailable.
    MissingFx,
    /// The current account currency was unavailable.
    MissingAccountCurrency,
    /// A non-flat position had no initial realized-PnL value.
    MissingInitialPnl,
    /// A position realization required an unavailable average cost basis.
    MissingCostBasis,
    /// Exact PnL arithmetic exceeded the supported decimal range.
    ArithmeticOverflow,
}

/// Raw outcome data produced by a policy for one asset.
///
/// Policies return `Vec<AccountOutcomeEntry>` without group information;
/// the engine attaches the policy's [`PolicyGroupId`] when assembling the final
/// [`AccountAdjustmentBatchResult`] (for batch hooks) or
/// [`crate::pretrade::PreTradeReservation::account_adjustments`] (for pre-trade).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountOutcomeEntry {
    /// Asset this outcome refers to.
    pub asset: Asset,
    /// Settled balance/position outcome; see [`OutcomeAmount`].
    pub balance: Option<OutcomeAmount>,
    /// Held (reserved) amount outcome; see [`OutcomeAmount`].
    ///
    /// Covers both working-order reservations and outgoing T+N settlements.
    pub held: Option<OutcomeAmount>,
    /// Incoming (pending inflow) amount outcome; see [`OutcomeAmount`].
    ///
    /// Covers both working-order expected fills and incoming T+N settlements.
    pub incoming: Option<OutcomeAmount>,
    /// Position realized-PnL result, denominated in the account currency.
    ///
    /// `delta` is the PnL change booked by this operation; `absolute` is the
    /// cumulative account-currency realized PnL for the `(account, asset)`
    /// holdings slot. Underlying-leg fills book `delta` as the realized PnL
    /// they produce; an asset-scoped balance adjustment can force-set the
    /// supplied account-currency value. `Err` reports
    /// the halt reason for the operation that first failed; later operations
    /// omit the value entirely while the position remains halted. It is `None`
    /// for reservations, cancels, settlement legs, zero-realized fills, and
    /// non-PnL adjustments. An asset-scoped balance adjustment is required to
    /// re-arm a halted slot.
    pub realized_pnl: Option<PnlOutcome>,
    /// Absolute account-currency average entry price of the current net
    /// position for this `(account, asset)` holdings slot, or `None` when the
    /// position is flat, average tracking is absent, or the
    /// operation does not carry an average (e.g. settlement legs, reservations,
    /// cancels, or adjustments without a balance change or an explicit average
    /// force-set). The underlying asset identifies one holdings slot even when
    /// it is traded against multiple quote currencies. A missing input that
    /// prevents only position-PnL calculation is reported through
    /// [`Self::realized_pnl`] and does not halt account PnL; later operations
    /// omit the unchanged position halt until an explicit force-set re-arms it.
    pub average_entry_price: Option<Price>,
}

/// Account position outcome with the group tag of the business entity that
/// produced it.
///
/// The engine wraps each [`AccountOutcomeEntry`] returned by a policy with
/// that business entity's [`PolicyGroupId`] before appending it to the result list.
/// Business entities sharing a group tag produce adjacent entries with the same
/// `policy_group_id`, in business entity registration order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountAdjustmentOutcome {
    /// Policy-group tag of the policy that produced this outcome.
    pub policy_group_id: PolicyGroupId,
    /// Account adjustment outcome entry.
    pub entry: AccountOutcomeEntry,
}

/// Outcome of a successful [`crate::Engine::apply_account_adjustment`] batch.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AccountAdjustmentBatchResult {
    /// Flat list of per-policy outcomes in policy registration order.
    ///
    /// A single asset may appear more than once. Policies that report nothing
    /// contribute no entries.
    pub outcomes: Vec<AccountAdjustmentOutcome>,
    /// Account blocks reported by policies after the batch was accepted.
    ///
    /// The engine records each entry for the adjusted account before returning
    /// this result. The first recorded cause remains the account's stored
    /// blocking reason.
    pub account_blocks: Vec<AccountBlock>,
}

impl IntoIterator for AccountAdjustmentBatchResult {
    type Item = AccountAdjustmentOutcome;
    type IntoIter = std::vec::IntoIter<AccountAdjustmentOutcome>;

    fn into_iter(self) -> Self::IntoIter {
        self.outcomes.into_iter()
    }
}

impl<'a> IntoIterator for &'a AccountAdjustmentBatchResult {
    type Item = &'a AccountAdjustmentOutcome;
    type IntoIter = std::slice::Iter<'a, AccountAdjustmentOutcome>;

    fn into_iter(self) -> Self::IntoIter {
        self.outcomes.iter()
    }
}
