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

//! Locking policy that synchronizes only the index domain.

use std::marker::PhantomData;

use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::storage::key_bound::AnyKey;
use crate::storage::policy::{LockingPolicy, LockingPolicyFactory};

/// Sync-mode-aware shared handle for [`IndexLocking`]-based engines.
///
/// Mirrors `AccountSyncHandle`: wraps `Arc<T>` for thread-safe
/// reference-counting while being deliberately `!Sync` (via
/// `PhantomData<Cell<()>>`) to match the `AccountSync` engine contract
/// that guarantees sequential per-handle invocation.
pub struct IndexShared<T>(
    std::sync::Arc<T>,
    std::marker::PhantomData<std::cell::Cell<()>>,
);

impl<T> Clone for IndexShared<T> {
    fn clone(&self) -> Self {
        Self(std::sync::Arc::clone(&self.0), std::marker::PhantomData)
    }
}

impl<T> std::ops::Deref for IndexShared<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

// SAFETY: `IndexLocking` engines guarantee sequential per-handle
// invocation: only one logical thread holds a reference at a time, so
// shared `&T` never races with mutation. The `Arc` refcount is
// thread-safe. `!Sync` (enforced by `PhantomData<Cell<()>>`) prevents
// sharing `&IndexShared<T>` across threads, matching the `AccountSync`
// engine contract.
unsafe impl<T: Send> Send for IndexShared<T> {}

/// Locking policy factory that synchronizes the **index** but leaves
/// values unsynchronized.
///
/// A [`Storage`](crate::storage::Storage) configured with `IndexLocking`
/// allows concurrent inserts, removals, and lookups of keys (the index
/// is protected by a reader-writer lock), but reads and writes of the
/// values themselves are not synchronized. The caller must ensure
/// values are either thread-safe internally or accessed under their own
/// external discipline (for example, only one thread mutates values).
///
/// # Aliasing safety
///
/// The closure-based access methods on
/// [`Storage`](crate::storage::Storage) confine references to the call
/// that produced them. Read-from-read re-entry is allowed because it
/// creates only shared references. Any re-entry path involving a write
/// hold would alias `&mut Value` while another values borrow is still
/// live; it is detected by `Storage` in debug builds and panics with a
/// clear message. Release builds skip the check.
///
/// # Compile-time key bound
///
/// The `KeyBound` type parameter selects which keys this factory will
/// produce storages for. The default,
/// [`AnyKey`](crate::storage::AnyKey), accepts any
/// [`Hash`](std::hash::Hash) + [`Eq`] key. Embeddings can substitute a
/// stricter marker that implements
/// [`IndexKeyBound`](crate::storage::IndexKeyBound) for only a subset
/// of keys; attempting to construct a storage with a key the marker
/// does not admit then becomes a compile-time error.
///
/// `KeyBound` carries no runtime data (it lives behind a
/// `PhantomData<fn() -> KeyBound>` so it does not affect [`Send`] /
/// [`Sync`] inheritance) and exists purely to gate
/// [`StorageBuilder::create`](crate::storage::StorageBuilder::create).
///
/// # Thread safety
///
/// Storages built with `IndexLocking` are [`Send`] (provided
/// `Key: Send` and `Value: Send`) but **not** [`Sync`]. The values
/// domain is left unsynchronized, so sharing `&Storage` across threads
/// would race on values; the type system rejects that at compile time.
/// To use the storage from multiple threads, transfer ownership (move
/// it across a channel, wrap in a `Mutex<Storage<..>>`, etc.) or
/// switch to [`FullLocking`](crate::storage::FullLocking), which
/// implements
/// [`FullySynchronized`](crate::storage::FullySynchronized).
///
/// The following must therefore fail to compile:
///
/// ```compile_fail
/// use std::sync::Arc;
/// use std::thread;
/// use openpit::Engine;
///
/// let builder = Engine::builder::<(), ()>().account_sync();
/// let storage = Arc::new(builder.storage_builder().create::<u64, u64>());
/// let s2 = Arc::clone(&storage);
/// thread::spawn(move || {
///     let _ = s2.with(&1, |v| *v);
/// });
/// ```
pub struct IndexLocking<KeyBound: 'static = AnyKey> {
    _key_bound: PhantomData<fn() -> KeyBound>,
}

impl<KeyBound: 'static> Default for IndexLocking<KeyBound> {
    #[inline]
    fn default() -> Self {
        Self {
            _key_bound: PhantomData,
        }
    }
}

impl<KeyBound: 'static> Clone for IndexLocking<KeyBound> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}

impl<KeyBound: 'static> Copy for IndexLocking<KeyBound> {}

impl<KeyBound: 'static> std::fmt::Debug for IndexLocking<KeyBound> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IndexLocking").finish()
    }
}

impl<KeyBound: 'static> LockingPolicyFactory for IndexLocking<KeyBound> {
    type Policy = IndexLockingPolicy;

    /// Multi-thread regime; `AtomicBool` provides lock-free synchronization.
    type IndexFlag = std::sync::atomic::AtomicBool;

    type Shared<T: 'static> = IndexShared<T>;

    fn create_policy(&self) -> Self::Policy {
        IndexLockingPolicy {
            index: RwLock::new(()),
        }
    }

    fn new_shared<T: 'static>(value: T) -> IndexShared<T> {
        IndexShared(std::sync::Arc::new(value), std::marker::PhantomData)
    }
}

/// Concrete index-only policy installed in storages built from
/// [`IndexLocking`].
///
/// Not re-exported from the crate; name it via the associated type if
/// needed: `<IndexLocking as LockingPolicyFactory>::Policy`.
pub struct IndexLockingPolicy {
    index: RwLock<()>,
}

// SAFETY: `read_index`/`write_index` delegate to a
// `parking_lot::RwLock`, which upholds the standard reader-writer
// invariants. The values domain has no synchronization, but the
// storage-level public API confines value references to closure calls.
// Values guards are no-ops because this policy deliberately leaves the
// values domain unsynchronized.
unsafe impl LockingPolicy for IndexLockingPolicy {
    type IndexSharedGuard<'a> = RwLockReadGuard<'a, ()>;
    type IndexExclusiveGuard<'a> = RwLockWriteGuard<'a, ()>;
    type ValuesSharedGuard<'a> = ();
    type ValuesExclusiveGuard<'a> = ();

    #[inline]
    fn read_index(&self) -> Self::IndexSharedGuard<'_> {
        self.index.read()
    }

    #[inline]
    fn write_index(&self) -> Self::IndexExclusiveGuard<'_> {
        self.index.write()
    }

    #[inline]
    fn read_values<Key>(&self, _key: &Key) -> Self::ValuesSharedGuard<'_> {}

    #[inline]
    fn write_values<Key>(&self, _key: &Key) -> Self::ValuesExclusiveGuard<'_> {}
}
