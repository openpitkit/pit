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

/// Identifies a registered instrument within a
/// [`MarketDataService`](super::service::MarketDataService).
///
/// Assigned by
/// [`MarketDataService::register`](super::service::MarketDataService::register)
/// or
/// [`MarketDataService::register_with_id`](super::service::MarketDataService::register_with_id)
/// and valid for the lifetime of the instrument's registration. Use the id
/// for all hot-path lookups to avoid hash-map overhead.
///
/// The underlying value is a `u64` that maps naturally to primary keys in
/// most databases, sequence numbers, and other integer identifiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct InstrumentId(pub(crate) u64);

impl InstrumentId {
    /// Wraps the given integer as an `InstrumentId`.
    ///
    /// Callers using a caller-assigned identity scheme (database primary keys,
    /// sequence numbers, external IDs) can pass any `u64` value here and feed
    /// it to
    /// [`MarketDataService::register_with_id`](super::service::MarketDataService::register_with_id).
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    /// Returns the underlying integer.
    pub fn as_u64(self) -> u64 {
        self.0
    }
}

impl From<u64> for InstrumentId {
    fn from(id: u64) -> Self {
        Self(id)
    }
}

impl From<InstrumentId> for u64 {
    fn from(id: InstrumentId) -> Self {
        id.0
    }
}
