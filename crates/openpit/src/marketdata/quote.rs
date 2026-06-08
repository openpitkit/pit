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

use std::time::Duration;

use crate::param::Price;

/// Current market snapshot for an instrument.
///
/// Every field is optional: producers publish only the fields they actually
/// have. How a `Quote` interacts with the slot's previously stored value is
/// chosen by the publisher when calling the service:
///
/// - [`MarketDataService::push`](super::service::MarketDataService::push)
///   replaces the entire snapshot - any field left as `None` in the new quote
///   is cleared from the slot.
/// - [`MarketDataService::push_patch`](super::service::MarketDataService::push_patch)
///   merges the new quote into the existing snapshot - `None` fields preserve
///   the prior value, `Some` fields overwrite it.
///
/// In either case the slot's publish instant is bumped to the current time.
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

    /// Merges `patch` into `self`: every `Some` field of `patch` overwrites
    /// the matching field of `self`; every `None` field leaves `self`
    /// unchanged.
    pub(crate) fn patched_with(self, patch: Quote) -> Quote {
        Quote {
            mark: patch.mark.or(self.mark),
            bid: patch.bid.or(self.bid),
            ask: patch.ask.or(self.ask),
        }
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
/// order. After a successful [`push`](super::service::MarketDataService::push)
/// or [`push_patch`](super::service::MarketDataService::push_patch) the quote
/// is observable through [`get`](super::service::MarketDataService::get) until
/// at least the effective lifetime has elapsed; reads after that point return
/// `None` (the entry is not removed from storage, only hidden from consumers).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuoteTtl {
    /// Quotes never expire on their own; only
    /// [`clear`](super::service::MarketDataService::clear) or a new push can
    /// change visibility.
    Infinite,
    /// Quotes expire `duration` after the push that wrote them.
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
