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

use super::reject::AccountBlock;
use crate::core::account_outcome::AccountAdjustmentOutcome;

/// Aggregated post-trade processing result.
///
/// # Atomicity
///
/// Post-trade processing is **not atomic**. Each policy applies its internal
/// state changes immediately and directly (no deferred mutations, no rollback).
/// If one policy produces [`Self::account_blocks`] and another does not, the
/// non-blocking policy's state changes are already committed to storage and
/// will not be reversed.
///
/// Callers must apply all entries in [`Self::account_adjustments`] to their
/// downstream systems regardless of whether [`Self::account_blocks`] is
/// non-empty, because those entries reflect storage that has already been
/// mutated.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PostTradeResult {
    /// Account blocks reported by policies after the post-trade was performed.
    ///
    /// Non-empty when at least one policy entered a blocked state. The engine
    /// merges blocks from all policies in registration order.
    ///
    /// A non-empty value here does **not** mean that [`Self::account_adjustments`]
    /// were rolled back — see the type-level atomicity note.
    pub account_blocks: Vec<AccountBlock>,

    /// Account position outcomes reported by policies, in policy registration
    /// order.
    ///
    /// Contains zero or more entries. A single asset may appear more than once;
    /// the exact content depends on which policies the engine was configured
    /// with and how those policies report.
    ///
    /// These entries reflect storage mutations that have already been applied.
    /// Callers must propagate them to downstream systems even when
    /// [`Self::account_blocks`] is non-empty — see the type-level atomicity note.
    pub account_adjustments: Vec<AccountAdjustmentOutcome>,
}

impl PostTradeResult {
    /// Returns true when no policy produced account blocks or account adjustments.
    pub fn is_empty(&self) -> bool {
        self.account_blocks.is_empty() && self.account_adjustments.is_empty()
    }

    /// Builds a result that carries only account blocks.
    ///
    /// Convenience for policies that report blocking errors without any
    /// account adjustments, such as failures to access required fields on the
    /// execution report.
    pub fn blocks_only(account_blocks: Vec<AccountBlock>) -> Self {
        Self {
            account_blocks,
            account_adjustments: Vec::new(),
        }
    }
}

impl From<Vec<AccountBlock>> for PostTradeResult {
    fn from(account_blocks: Vec<AccountBlock>) -> Self {
        Self::blocks_only(account_blocks)
    }
}
