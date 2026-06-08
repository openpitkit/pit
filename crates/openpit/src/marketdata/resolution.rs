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

//! Quote-resolution mode and the group source consulted on a read.

use crate::param::AccountGroupId;

// ─── QuoteResolution ──────────────────────────────────────────────────────────

/// Selects how a read resolves a quote across the per-account,
/// per-account-group, and default ("everyone-else") buckets.
///
/// A quote is published into one of three conceptual buckets per instrument:
/// the per-account bucket, the per-account-group bucket, and the default group
/// bucket ([`DEFAULT_ACCOUNT_GROUP`](crate::param::DEFAULT_ACCOUNT_GROUP)),
/// which doubles as the "everyone-else" bucket. A [`QuoteResolution`] chooses
/// which of those buckets a reader is willing to fall through to, in order,
/// when a more specific bucket has no quote.
///
/// The mode controls only quote *selection*. The freshness check that follows
/// is governed by the TTL cascade for the requested `(account, group)` and is
/// independent of which bucket the quote was found in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuoteResolution {
    /// Consult only the per-account bucket for the reading account.
    AccountOnly,
    /// Consult the per-account bucket, then fall back to the account's group
    /// bucket when the account bucket has no quote.
    AccountThenGroup,
    /// Consult the per-account bucket, then the account's group bucket, then
    /// the default-group ("everyone-else") bucket, in that order.
    AccountThenGroupThenDefault,
}

// ─── AccountInfo ─────────────────────────────────────────────────────────────

/// Supplies account information to the market-data service, starting with the
/// account group to consult for group-level quote and TTL resolution.
///
/// Reads take `&impl AccountInfo` so the group can be resolved lazily: the
/// service calls [`group`](Self::group) only when the per-account bucket misses
/// and the [`QuoteResolution`] (or a TTL cascade tier) actually needs the
/// group. A pre-resolved group can be passed directly via the
/// `Option<AccountGroupId>` / [`AccountGroupId`] impls; engine callers pass a
/// lazy lookup such as
/// [`PreTradeContext`](crate::pretrade::PreTradeContext).
pub trait AccountInfo {
    /// The account group to consult for group-level quote/TTL resolution, or
    /// `None` when the bound account has no group.
    fn group(&self) -> Option<AccountGroupId>;
}

impl AccountInfo for Option<AccountGroupId> {
    #[inline]
    fn group(&self) -> Option<AccountGroupId> {
        *self
    }
}

impl AccountInfo for AccountGroupId {
    #[inline]
    fn group(&self) -> Option<AccountGroupId> {
        Some(*self)
    }
}
