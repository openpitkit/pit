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

//! TTL cascade primitives for [`MarketDataService`](super::MarketDataService).
//!
//! The service resolves a quote's lifetime from several axes (instrument,
//! account, group) plus a global default. Each axis cell is either *unset*
//! (inherit, fall through the cascade) or *set* to an explicit
//! [`TtlSetting`]. A set value stops the cascade; only when every higher tier
//! is unset does the global default apply.

use std::time::Duration;

use super::quote::QuoteTtl;

/// An explicit, *set* per-axis quote lifetime.
///
/// The cascade's "unset / inherit" state is encoded by the *absence* of a
/// `TtlSetting` (a missing map entry, or `None` for the instrument-level
/// cell), so this type carries only the two ways a lifetime can be explicitly
/// pinned:
///
/// - [`Infinite`](Self::Infinite) - explicitly never expires; stops the
///   cascade at "no expiry".
/// - [`Finite`](Self::Finite) - expires the given duration after the push.
///
/// [`QuoteTtl`] maps onto this type at the setter boundary via
/// [`from_quote_ttl`](Self::from_quote_ttl).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TtlSetting {
    /// Explicitly never expires; stops the cascade with no expiry.
    Infinite,
    /// Expires `duration` after the push that wrote the quote.
    Finite(Duration),
}

impl TtlSetting {
    /// Maps the public [`QuoteTtl`] onto an explicit, set lifetime.
    pub(crate) fn from_quote_ttl(ttl: QuoteTtl) -> Self {
        match ttl {
            QuoteTtl::Infinite => Self::Infinite,
            QuoteTtl::Within(duration) => Self::Finite(duration),
        }
    }

    /// Returns the finite lifetime, or `None` when this setting is infinite.
    pub(crate) fn as_duration(self) -> Option<Duration> {
        match self {
            Self::Infinite => None,
            Self::Finite(duration) => Some(duration),
        }
    }
}
