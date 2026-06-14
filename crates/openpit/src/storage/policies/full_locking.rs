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

//! Locking policy that synchronizes both index and values.

use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::storage::policy::{FullySynchronized, LockingPolicy, LockingPolicyFactory};
use crate::storage::{ArcSwapConfigCell, ConfigCell};

/// Locking policy factory that synchronizes **both** the index and the
/// values domain.
///
/// A [`Storage`](crate::storage::Storage) configured with `FullLocking` is
/// fully thread-safe:
///
/// * Many threads may concurrently look up existing keys.
/// * A thread inserting a new key or removing an existing one blocks
///   every other index access until it completes.
/// * Many threads may concurrently read values.
/// * A thread mutating any value blocks every other value access (read or
///   write) until it completes.
///
/// Note that the values lock is a single coarse reader-writer lock shared
/// by all keys, not a per-key lock. A writer to one value blocks readers
/// of every other value. The trade-off keeps the policy independent of
/// the key type and avoids the per-entry allocation overhead of per-key
/// locks.
///
/// The storage type is `Send + Sync` provided `Key: Send + Sync` and
/// `Value: Send + Sync`.
#[derive(Debug, Default, Clone, Copy)]
pub struct FullLocking;

impl LockingPolicyFactory for FullLocking {
    type Policy = FullLockingPolicy;

    /// Multi-thread regime; `AtomicBool` provides lock-free synchronization.
    type IndexFlag = std::sync::atomic::AtomicBool;

    type Shared<T: 'static> = std::sync::Arc<T>;

    type Config<Settings: Clone + 'static> = ArcSwapConfigCell<Settings>;

    fn create_policy(&self) -> Self::Policy {
        FullLockingPolicy {
            index: RwLock::new(()),
            values: RwLock::new(()),
        }
    }

    fn new_shared<T: 'static>(value: T) -> std::sync::Arc<T> {
        std::sync::Arc::new(value)
    }

    fn new_config<Settings: Clone + 'static>(value: Settings) -> ArcSwapConfigCell<Settings> {
        ArcSwapConfigCell::new(value)
    }
}

/// Concrete full-locking policy installed in storages built from [`FullLocking`].
///
/// Not re-exported from the crate; name it via the associated type if
/// needed: `<FullLocking as LockingPolicyFactory>::Policy`.
pub struct FullLockingPolicy {
    index: RwLock<()>,
    values: RwLock<()>,
}

// SAFETY: both domains are protected by independent `parking_lot::RwLock`s.
// Each lock upholds the standard reader-writer invariants, which is
// exactly what the `LockingPolicy` contract requires. The storage takes
// the index lock before the values lock for every operation, eliminating
// deadlock risk between domains. Shared access uses `read_recursive()` to
// support the storage-level read-from-read re-entry contract even when a
// writer is queued.
unsafe impl LockingPolicy for FullLockingPolicy {
    type IndexSharedGuard<'a> = RwLockReadGuard<'a, ()>;
    type IndexExclusiveGuard<'a> = RwLockWriteGuard<'a, ()>;
    type ValuesSharedGuard<'a> = RwLockReadGuard<'a, ()>;
    type ValuesExclusiveGuard<'a> = RwLockWriteGuard<'a, ()>;

    #[inline]
    fn read_index(&self) -> Self::IndexSharedGuard<'_> {
        self.index.read_recursive()
    }

    #[inline]
    fn write_index(&self) -> Self::IndexExclusiveGuard<'_> {
        self.index.write()
    }

    #[inline]
    fn read_values<Key>(&self, _key: &Key) -> Self::ValuesSharedGuard<'_> {
        self.values.read_recursive()
    }

    #[inline]
    fn write_values<Key>(&self, _key: &Key) -> Self::ValuesExclusiveGuard<'_> {
        self.values.write()
    }
}

// SAFETY: `FullLockingPolicy` holds an independent `RwLock` for the
// values domain whose guards block every other read or write across the
// whole storage for the guard's lifetime, satisfying the
// `FullySynchronized` contract.
unsafe impl FullySynchronized for FullLockingPolicy {}
