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

/// Identifies an instrument across OpenPit subsystems.
///
/// The underlying value maps naturally to database primary keys, sequence
/// numbers, and external integer identifiers. Callers can assign an explicit
/// value when registering an instrument with a [`ReferenceBook`] or a
/// market-data service.
///
/// [`ReferenceBook`]: super::ReferenceBook
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct InstrumentId(pub(crate) u64);

impl InstrumentId {
    /// Wraps an integer as an instrument identity.
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    /// Returns the underlying integer.
    pub const fn as_u64(self) -> u64 {
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
