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

use crate::param::{AccountGroupId, AccountId, Asset, Pnl};

/// Account P&L bounds used by the spot-funds policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpotFundsPnlBoundsBarrier {
    /// Currency matched when the account has an effective currency.
    ///
    /// With a known effective account currency, an account-tier mismatch fails
    /// closed and blocks the account; account-group and global mismatches are
    /// skipped. A known-currency account with no account barrier and no matching
    /// fallback has no effective barrier, but its P&L keeps accumulating and
    /// publishing. Without an effective currency, the first in-scope barrier
    /// applies. Bounds are compared as stored and are never FX-converted.
    pub currency: Asset,
    /// Optional lower bound, typically a negative loss limit.
    pub lower_bound: Option<Pnl>,
    /// Optional upper bound, typically a positive profit-taking limit.
    pub upper_bound: Option<Pnl>,
}

/// Account-group P&L bounds refinement for spot funds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpotFundsPnlBoundsAccountGroupBarrier {
    /// Bounds for the group.
    pub barrier: SpotFundsPnlBoundsBarrier,
    /// Account group this barrier applies to.
    pub account_group_id: AccountGroupId,
}

/// Account P&L bounds refinement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpotFundsPnlBoundsAccountBarrier {
    /// Bounds for the account.
    pub barrier: SpotFundsPnlBoundsBarrier,
    /// Account this barrier applies to.
    pub account_id: AccountId,
}
