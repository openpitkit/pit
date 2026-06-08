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
use crate::param::{Asset, PositionSize};

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
