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

use smallvec::SmallVec;

use crate::core::account_outcome::AccountOutcomeEntry;
use crate::param::Price;

/// Per-policy result of [`crate::pretrade::PreTradePolicy::perform_pre_trade_check`].
///
/// Carries two independent payloads:
///
/// - `account_adjustments` - per-asset balance/held/incoming outcome entries
///   produced by the policy. The engine attaches the policy's
///   [`crate::pretrade::PolicyGroupId`] when assembling the final
///   [`crate::core::AccountAdjustmentOutcome`] list.
/// - `lock_prices` - prices the policy needs to persist for the order's
///   [`crate::pretrade::PreTradeLock`]. Each price is pushed under the policy's
///   [`crate::pretrade::PolicyGroupId`] in insertion order; the policy never sets the
///   group itself.
#[derive(Clone, Debug, Default)]
pub struct PolicyPreTradeResult {
    pub account_adjustments: SmallVec<[AccountOutcomeEntry; 1]>,
    pub lock_prices: SmallVec<[Price; 1]>,
}

impl PolicyPreTradeResult {
    /// Returns an empty value with no preallocated storage.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Returns a value with the requested preallocated capacities.
    pub fn with_capacity(adjustments: usize, prices: usize) -> Self {
        Self {
            account_adjustments: SmallVec::with_capacity(adjustments),
            lock_prices: SmallVec::with_capacity(prices),
        }
    }
}
