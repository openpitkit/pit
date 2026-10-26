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

/// P&L measure bounded by a runtime spot-funds configuration.
///
/// Unrealized P&L of a position is the amount a full close of its owned
/// quantity at the current price would add to realized P&L.
///
/// Each axis is configured at the global, account-group and account tiers
/// and resolves its barrier like the realized bounds: the account tier
/// applies whatever its currency; account-group, then global barriers apply
/// only when their currency matches a known account currency. Without a known
/// account currency, the first in-scope barrier applies. Bounds are compared
/// as stored and are never FX-converted.
///
/// Axes are checked on pre-trade, dry run, a fill (a trade or non-zero fee,
/// when the report gave no block of its own), an account P&L force-set, and a
/// retune that changes the axis's effective barrier. An account-group
/// membership change does not check axes; the next order catches a breach.
///
/// A breach or halted value of an axis with an effective barrier blocks the
/// account with `PnlKillSwitchTriggered` in every limit mode. An account-tier
/// barrier in a different currency than a known account currency also blocks
/// it. A position axis compares each position.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SpotFundsPnlBoundsAxis {
    /// Sum of unrealized P&L over the account's positions, in the account
    /// currency.
    AccountUnrealized,
    /// Realized account P&L plus account unrealized P&L.
    AccountTotal,
    /// Unrealized P&L of each position, each position compared with the same
    /// bounds.
    PositionUnrealized,
    /// Realized plus unrealized P&L of each position, each position compared
    /// with the same bounds.
    PositionTotal,
}

impl SpotFundsPnlBoundsAxis {
    pub(crate) const ALL: [Self; 4] = [
        Self::AccountUnrealized,
        Self::AccountTotal,
        Self::PositionUnrealized,
        Self::PositionTotal,
    ];
}

/// Account P&L bounds used by the spot-funds policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpotFundsPnlBoundsBarrier {
    /// Currency matched when the account has an effective currency.
    ///
    /// With a known effective account currency, an account-tier mismatch fails
    /// closed and blocks the account; account-group and global mismatches are
    /// skipped. A known-currency account with no account barrier and no matching
    /// fallback has no effective barrier, but its realized P&L keeps
    /// accumulating and publishing. Without an effective currency, the first
    /// in-scope barrier applies. Bounds are compared as stored and are never
    /// FX-converted.
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
