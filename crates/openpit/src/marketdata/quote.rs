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

use std::time::Duration;

use crate::param::Price;

/// Current market snapshot for an instrument.
///
/// Every field is optional: producers publish only the fields that exist in
/// one observation. A publication always replaces the stored snapshot, so an
/// absent field clears the previous value rather than retaining it. A publisher
/// that needs to combine observations must merge them itself, because it owns
/// both the observation relationship and the combined source age.
///
/// The service records the current monotonic publish instant and the source age
/// supplied by the publisher. Reads advance that source age by the elapsed time
/// since publication when evaluating freshness.
///
/// `#[non_exhaustive]` keeps the door open for further optional fields in
/// future releases.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Quote {
    /// Mark price.
    pub mark: Option<Price>,
    /// Best-bid price.
    pub bid: Option<Price>,
    /// Best-ask price.
    pub ask: Option<Price>,
}

impl Quote {
    /// Creates an empty quote with all fields unset.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the mark price.
    pub fn with_mark(mut self, mark: Price) -> Self {
        self.mark = Some(mark);
        self
    }

    /// Sets the best-bid price.
    pub fn with_bid(mut self, bid: Price) -> Self {
        self.bid = Some(bid);
        self
    }

    /// Sets the best-ask price.
    pub fn with_ask(mut self, ask: Price) -> Self {
        self.ask = Some(ask);
        self
    }
}

/// Maximum age allowed for a stored quote before it is treated as
/// unavailable.
///
/// `QuoteTtl` is the public, two-state lifetime callers supply at the setter
/// and registration boundaries. It maps onto the internal cascade as follows:
///
/// - As the service-wide default on
///   [`MarketDataBuilder`](super::builder::MarketDataBuilder) it is the lowest
///   cascade tier, applied only when no more specific axis is set.
/// - At registration via
///   [`register_with_ttl`](super::service::MarketDataService::register_with_ttl)
///   /
///   [`register_with_id_and_ttl`](super::service::MarketDataService::register_with_id_and_ttl)
///   it becomes the instrument-level setting.
/// - The per-account, per-group, and instrument-qualified setters
///   (`set_*_ttl`) pin the matching axis cell.
///
/// The effective lifetime for a read is resolved by the cascade for the
/// requested `(account, group)`; see
/// [`MarketDataService`](super::service::MarketDataService) for the tier
/// order. A publication supplies the quote's age at the source. The quote is
/// observable through [`get`](super::service::MarketDataService::get) only
/// while that source age plus the elapsed time since publication remains below
/// the effective lifetime. Expired entries remain stored and are returned in
/// the expired-quote error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuoteTtl {
    /// Quotes never expire on their own; only
    /// [`clear`](super::service::MarketDataService::clear) or a new push can
    /// change visibility.
    Infinite,
    /// Quotes expire when their source age plus time elapsed since publication
    /// reaches `duration`.
    Within(Duration),
}

impl QuoteTtl {
    /// Returns the per-quote lifetime, if finite.
    pub fn as_duration(self) -> Option<Duration> {
        match self {
            Self::Infinite => None,
            Self::Within(d) => Some(d),
        }
    }
}
