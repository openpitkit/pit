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

use super::{AccountBlock, PreTradeLock, Rejects};
use crate::core::account_outcome::AccountAdjustmentOutcome;

/// Inert verdict of a pre-trade dry-run.
///
/// A dry-run runs the applicable pre-trade stages against current engine state
/// and reports what *would* happen, with zero effect on that state: no rate-limit
/// budget is spent, no reservation or hold is applied, and no account is
/// blocked. Repeating a dry-run never moves engine state.
///
/// Unlike [`PreTradeReservation`](crate::pretrade::PreTradeReservation), this
/// report carries no commit/rollback capability. It is inert by construction -
/// there is nothing to finalize - so it exposes no `commit`/`rollback` methods.
/// The fields describe the outcome the equivalent real call would have produced:
///
/// - [`is_pass`](Self::is_pass) / [`rejects`](Self::rejects): whether the stages
///   produce no effective rejects, and the rejects collected otherwise.
/// - [`reviews`](Self::reviews): review marks requiring a human decision even
///   when `is_pass()` is true. Callers inspect these before submission.
/// - [`lock`](Self::lock): the [`PreTradeLock`] the main stage would have
///   produced (empty when the start stage would have rejected, or when no
///   policy locks anything, or a without-hold review is returned).
/// - [`account_adjustments`](Self::account_adjustments): the per-asset
///   `held`/`available` outcomes the main stage would have produced, with the
///   same numbers a real reservation would report for the same order and state.
/// - [`account_block`](Self::account_block): the [`AccountBlock`] an
///   account-scope reject would have latched - reported here, but *not* recorded
///   in the engine's blocked-accounts registry.
///
/// # Examples
///
/// ```rust
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use openpit::param::{Asset, Price, Quantity, Side, TradeAmount};
/// use openpit::{Engine, Instrument, OrderOperation};
/// use openpit::pretrade::policies::OrderValidationPolicy;
///
/// let engine = Engine::builder::<OrderOperation, (), ()>()
///     .no_sync()
///     .pre_trade(OrderValidationPolicy::new())
///     .build()?;
/// let order = OrderOperation {
///     instrument: Instrument::new(Asset::new("AAPL")?, Asset::new("USD")?),
///     account_id: openpit::param::AccountId::from_u64(99224416),
///     side: Side::Buy,
///     trade_amount: TradeAmount::Quantity(Quantity::from_str("10")?),
///     price: Some(Price::from_str("185")?),
/// };
///
/// // A dry-run leaves the engine untouched: a later real call behaves as if
/// // the dry-run never happened.
/// let report = engine.execute_pre_trade_dry_run(order.clone());
/// if report.is_pass() && report.reviews().is_empty() {
///     let mut reservation = engine.start_pre_trade(order)?.execute()?;
///     if reservation.reviews().is_empty() {
///         // Send the order to the venue only when no human decision is needed.
///     }
///     // Commit both paths. Review keeps holds only for all with-hold marks.
///     reservation.commit();
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreTradeDryRunReport {
    would_be_lock: PreTradeLock,
    would_be_account_adjustments: Vec<AccountAdjustmentOutcome>,
    reviews: Vec<super::Reject>,
    would_be_account_block: Option<AccountBlock>,
    rejects: Option<Rejects>,
}

impl PreTradeDryRunReport {
    pub(crate) fn new(
        rejects: Option<Rejects>,
        would_be_lock: PreTradeLock,
        would_be_account_adjustments: Vec<AccountAdjustmentOutcome>,
        would_be_account_block: Option<AccountBlock>,
        reviews: Vec<super::Reject>,
    ) -> Self {
        Self {
            would_be_lock,
            would_be_account_adjustments,
            would_be_account_block,
            rejects,
            reviews,
        }
    }

    /// Returns `true` when the executed stages produce no effective rejects.
    ///
    /// Equivalent to `self.rejects().is_none()`. Review marks do not change this
    /// value; inspect [`Self::reviews`] before deciding to send the order.
    pub fn is_pass(&self) -> bool {
        self.rejects.is_none()
    }

    /// Returns the rejects the order would have collected, or `None` when it
    /// would have passed.
    pub fn rejects(&self) -> Option<&Rejects> {
        self.rejects.as_ref()
    }

    /// Returns review marks from the stages that would run, in policy order.
    ///
    /// Effective rejects discard all marks. Any without-hold mark leaves the
    /// reported lock and account adjustments empty.
    pub fn reviews(&self) -> &[super::Reject] {
        &self.reviews
    }

    /// Returns the lock context the main stage would have produced.
    ///
    /// Empty when the start stage would have rejected (the main stage never
    /// runs in that case), any without-hold review is returned, or no policy
    /// locks a price.
    pub fn lock(&self) -> &PreTradeLock {
        &self.would_be_lock
    }

    /// Returns the account position modifications the main stage would have
    /// produced, grouped by [`super::PolicyGroupId`].
    ///
    /// The numbers match what a real reservation would report for the same order
    /// and engine state. Empty when the start stage would have rejected or when
    /// any without-hold review is returned, or no policy reports an adjustment.
    pub fn account_adjustments(&self) -> &[AccountAdjustmentOutcome] {
        &self.would_be_account_adjustments
    }

    /// Returns the account block an account-scope reject would have latched.
    ///
    /// A real call records this block in the engine's blocked-accounts registry;
    /// a dry-run reports it here without recording it.
    pub fn account_block(&self) -> Option<&AccountBlock> {
        self.would_be_account_block.as_ref()
    }
}
