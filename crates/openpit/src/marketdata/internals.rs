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

use std::collections::HashMap;
use std::time::Duration;

use crate::param::{AccountGroupId, AccountId};
use crate::time::Instant;

use super::builder::MarketDataSync;
use super::quote::Quote;
use super::ttl::TtlSetting;

/// Quote payload, its source age, and the instant at which it was published.
#[derive(Clone, Copy, Debug)]
pub(crate) struct QuoteState {
    pub(crate) quote: Quote,
    pub(crate) pushed_at: Instant,
    pub(crate) source_age: Duration,
}

/// Per-instrument quote storage across the three conceptual buckets.
///
/// Quotes for one instrument live in three places: a map keyed by
/// [`AccountId`] (the per-account buckets), and a single map keyed by
/// [`AccountGroupId`] (the per-group buckets) in which the entry at the
/// default group ([`DEFAULT_ACCOUNT_GROUP`]) is both the default-group bucket
/// and the "everyone-else" bucket. The ergonomic no-target push writes the
/// default entry of `groups`.
///
/// Memory is intentionally traded for lookup speed: every bucket is a direct
/// hash-map probe with no fallback chasing inside the storage itself; the
/// service walks the buckets in the order the [`QuoteResolution`] dictates.
///
/// [`QuoteResolution`]: super::QuoteResolution
/// [`DEFAULT_ACCOUNT_GROUP`]: crate::param::DEFAULT_ACCOUNT_GROUP
pub(crate) struct SlotQuotes {
    /// Per-account quote buckets.
    pub(crate) accounts: HashMap<AccountId, QuoteState>,
    /// Per-group quote buckets; the entry at the default group is the
    /// default / everyone-else bucket.
    pub(crate) groups: HashMap<AccountGroupId, QuoteState>,
}

impl SlotQuotes {
    fn new() -> Self {
        Self {
            accounts: HashMap::new(),
            groups: HashMap::new(),
        }
    }
}

/// Per-instrument TTL settings across the instrument, account, and group axes.
///
/// Presence of a key means "set"; absence means "inherit" (fall through the
/// cascade). The entry at the default group
/// ([`DEFAULT_ACCOUNT_GROUP`](crate::param::DEFAULT_ACCOUNT_GROUP)) in `groups`
/// is the instrument × default-group cell.
pub(crate) struct SlotTtls {
    /// Per-account TTL overrides for this instrument (cascade tier 1).
    pub(crate) accounts: HashMap<AccountId, TtlSetting>,
    /// Per-group TTL overrides for this instrument (cascade tiers 2-3); the
    /// entry at the default group is the instrument × default-group cell.
    pub(crate) groups: HashMap<AccountGroupId, TtlSetting>,
    /// Instrument-level TTL (cascade tier 7); `None` means inherit the global
    /// default. Set by the TTL-carrying registration variants and by
    /// [`set_instrument_ttl`](super::MarketDataService::set_instrument_ttl).
    pub(crate) instrument: Option<TtlSetting>,
}

impl SlotTtls {
    fn new(instrument: Option<TtlSetting>) -> Self {
        Self {
            accounts: HashMap::new(),
            groups: HashMap::new(),
            instrument,
        }
    }
}

/// Quotes and TTL settings observed atomically for one instrument.
pub(crate) struct SlotState {
    /// The instrument's quote buckets.
    pub(crate) quotes: SlotQuotes,
    /// The instrument's TTL settings across all axes.
    pub(crate) ttls: SlotTtls,
}

impl SlotState {
    fn new(instrument_ttl: Option<TtlSetting>) -> Self {
        Self {
            quotes: SlotQuotes::new(),
            ttls: SlotTtls::new(instrument_ttl),
        }
    }
}

/// Internal per-instrument storage slot.
///
/// Holds the three quote buckets and the per-slot TTL settings behind one
/// mode-selected [`MarketDataSync::Lock`]: a genuine no-op under
/// [`LocalSync`](crate::LocalSync) and a real `parking_lot::RwLock` under
/// [`FullSync`](crate::FullSync).
///
/// Keeping both domains under one guard makes quote selection and TTL cascade
/// resolution one atomic read. A reader can therefore never pair a quote copied
/// before a concurrent publication with TTL settings written after it.
pub(crate) struct Slot<Sync: MarketDataSync> {
    /// The instrument's atomically observed quote and TTL state.
    pub(crate) state: Sync::Lock<SlotState>,
}

impl<Sync: MarketDataSync> Slot<Sync> {
    pub(crate) fn new(sync: &Sync, instrument_ttl: Option<TtlSetting>) -> Self {
        Self {
            state: sync.new_lock(SlotState::new(instrument_ttl)),
        }
    }
}
