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

//! Engine-owned table read from every account's call path.

use std::hash::Hash;

use super::Storage;

/// Engine-owned table that calls for any account may read concurrently.
///
/// Under [`IndexLocking`](super::IndexLocking) the values domain of a
/// [`Storage`] is unsynchronized, and the account-sync contract pins only each
/// account's own entries to one caller at a time. A table such as a group
/// currency or a block cause is instead read from every account's call path,
/// so an in-place write under a shared index would hand out `&mut Value` while
/// another thread holds `&Value`.
///
/// This wrapper exposes no such write. Reads take the index in shared mode, as
/// [`Storage::with`] does; every write takes the index exclusively before it
/// forms a mutable reference, and the first-wins insertion forms none on a
/// hit. Reads stay as cheap as before; writes are rare administrative
/// operations and kill-switch triggers.
pub(crate) struct EngineWideStorage<Key, Value, LockingPolicy>
where
    LockingPolicy: super::LockingPolicy,
{
    inner: Storage<Key, Value, LockingPolicy>,
}

impl<Key, Value, LockingPolicy> EngineWideStorage<Key, Value, LockingPolicy>
where
    LockingPolicy: super::LockingPolicy,
{
    pub(super) fn new(inner: Storage<Key, Value, LockingPolicy>) -> Self {
        Self { inner }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl<Key, Value, LockingPolicy> EngineWideStorage<Key, Value, LockingPolicy>
where
    Key: Hash + Eq,
    LockingPolicy: super::LockingPolicy,
{
    pub(crate) fn with<Reader, Output>(&self, key: &Key, reader: Reader) -> Option<Output>
    where
        Reader: FnOnce(&Value) -> Output,
    {
        self.inner.with(key, reader)
    }

    /// Records `default()` under `key` unless the key already holds a value;
    /// returns whether this call inserted it.
    pub(crate) fn insert_if_absent<Initializer>(&self, key: Key, default: Initializer) -> bool
    where
        Initializer: FnOnce() -> Value,
    {
        self.inner.insert_if_absent(key, default)
    }

    /// Inserts or rewrites the value under `key` with the index held
    /// exclusively; `mutator` learns whether the entry was just inserted.
    pub(crate) fn upsert<Mutator, Output, Initializer>(
        &self,
        key: Key,
        default: Initializer,
        mutator: Mutator,
    ) -> Output
    where
        Mutator: FnOnce(&mut Value, bool) -> Output,
        Initializer: FnOnce() -> Value,
        Key: Clone,
    {
        self.inner.with_mut_exclusive_index(key, default, mutator)
    }

    /// Rewrites an existing value with the index held exclusively; `None` when
    /// `key` is absent, without inserting.
    pub(crate) fn with_mut_if_present<Mutator, Output>(
        &self,
        key: &Key,
        mutator: Mutator,
    ) -> Option<Output>
    where
        Mutator: FnOnce(&mut Value) -> Output,
    {
        self.inner.with_mut_if_present_exclusive_index(key, mutator)
    }

    pub(crate) fn remove(&self, key: &Key) -> bool {
        self.inner.remove(key)
    }
}
