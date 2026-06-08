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

//! Locking policy traits.
//!
//! [`LockingPolicy`] abstracts over the synchronization scheme used by a
//! [`Storage`](super::Storage) instance. A storage owns one policy instance,
//! created by a [`LockingPolicyFactory`]. The factory is the type-level
//! configuration on a [`StorageBuilder`](super::StorageBuilder); each call to
//! [`StorageBuilder::create`](super::StorageBuilder::create) produces a fresh
//! policy via [`LockingPolicyFactory::create_policy`], so every storage has
//! its own private locks.
//!
//! # Lock domains
//!
//! A policy controls two independent lock domains:
//!
//! * **Index** — the set of keys present in the storage. Acquired
//!   exclusively when keys are added or removed; acquired shared when an
//!   existing key is looked up.
//! * **Values** — the bag of all values stored under those keys. Acquired
//!   exclusively when any value is mutated; acquired shared when any value
//!   is read. The key of the entry being accessed is passed as an argument
//!   so that future implementations can apply per-key granularity; the
//!   built-in policies ignore it.
//!
//! # Acquisition order
//!
//! Storage operations always acquire **index before values**. Implementations
//! must not introduce any other ordering, otherwise deadlocks are possible.

use std::marker::PhantomData;

/// Synchronization strategy used by a [`Storage`](super::Storage).
///
/// A policy provides four lock acquisitions, one per `(domain, mode)` pair
/// where the domain is index or values and the mode is shared (multiple
/// concurrent holders, no mutators) or exclusive (single holder, no other
/// readers or writers).
///
/// The `read_values` and `write_values` methods receive the key of the
/// entry being accessed. The built-in implementations ignore this argument;
/// it is reserved for custom implementations that want to apply per-key
/// granularity. The argument is generic so that no `Key` type-parameter
/// propagates into the trait itself, keeping the trait object-capable in
/// principle and straightforward to implement for all key types at once.
///
/// # Safety
///
/// `Storage` relies on the policy to uphold the following invariants:
///
/// * While any [`Self::IndexSharedGuard`] is alive, no thread modifies the
///   set of keys present in the owning storage.
/// * While any [`Self::IndexExclusiveGuard`] is alive, no other thread holds
///   any index guard for the same storage.
/// * While any [`Self::ValuesSharedGuard`] is alive, no thread mutates any
///   value in the owning storage.
/// * While any [`Self::ValuesExclusiveGuard`] is alive, no other thread
///   holds any values guard for the same storage.
///
/// The built-in implementations supplied with this crate
/// ([`NoLocking`](super::NoLocking), [`IndexLocking`](super::IndexLocking)
/// and [`FullLocking`](super::FullLocking)) honour these invariants. Custom
/// implementations are responsible for their own correctness; getting the
/// invariants wrong leads to undefined behaviour.
///
/// `NoLocking` honours the invariants vacuously by making the storage
/// `!Sync`: the Rust type system guarantees that no two threads can ever
/// observe the storage at the same time. The caller is then responsible
/// for not creating overlapping mutable borrows within that single thread,
/// the same contract as for direct use of [`std::cell::UnsafeCell`].
pub unsafe trait LockingPolicy {
    /// Guard returned by [`Self::read_index`].
    type IndexSharedGuard<'a>
    where
        Self: 'a;
    /// Guard returned by [`Self::write_index`].
    type IndexExclusiveGuard<'a>
    where
        Self: 'a;
    /// Guard returned by [`Self::read_values`].
    type ValuesSharedGuard<'a>
    where
        Self: 'a;
    /// Guard returned by [`Self::write_values`].
    type ValuesExclusiveGuard<'a>
    where
        Self: 'a;

    /// Acquires shared (read) access to the index domain.
    ///
    /// The call blocks until any active exclusive index holder releases.
    fn read_index(&self) -> Self::IndexSharedGuard<'_>;

    /// Acquires exclusive (write) access to the index domain.
    ///
    /// The call blocks until every active index holder releases.
    fn write_index(&self) -> Self::IndexExclusiveGuard<'_>;

    /// Acquires shared (read) access to the values domain.
    ///
    /// `key` identifies the entry being accessed and is available for
    /// implementations that wish to apply per-key granularity. The
    /// built-in policies ignore it. The call blocks until any active
    /// exclusive values holder releases.
    fn read_values<Key>(&self, key: &Key) -> Self::ValuesSharedGuard<'_>;

    /// Acquires exclusive (write) access to the values domain.
    ///
    /// `key` identifies the entry being accessed and is available for
    /// implementations that wish to apply per-key granularity. The
    /// built-in policies ignore it. The call blocks until every active
    /// values holder releases.
    fn write_values<Key>(&self, key: &Key) -> Self::ValuesExclusiveGuard<'_>;
}

/// Marker that asserts a [`LockingPolicy`] synchronizes BOTH the index
/// domain AND the values domain.
///
/// A [`Storage`](super::Storage) is [`Sync`] only when its policy
/// implements `FullySynchronized`. Policies that synchronize only the
/// index domain (or nothing at all) leave the storage `Send` but
/// `!Sync`: clients may move ownership between threads or guard the
/// storage with their own [`Mutex`](std::sync::Mutex), but cannot share
/// `&Storage` concurrently from multiple threads.
///
/// # Safety
///
/// Implementor guarantees that, in addition to the [`LockingPolicy`]
/// invariants:
///
/// * [`LockingPolicy::read_values`] returns a guard that prevents
///   concurrent mutation of any value in the owning storage for the
///   guard's lifetime.
/// * [`LockingPolicy::write_values`] returns a guard that prevents
///   concurrent access (read or write) of any value in the owning
///   storage for the guard's lifetime.
///
/// Index-only synchronization (where `read_values` / `write_values`
/// are no-ops) does **not** satisfy this contract.
pub unsafe trait FullySynchronized: LockingPolicy {}

/// Factory of [`LockingPolicy`] instances.
///
/// A factory is the type-level configuration parameter of
/// [`StorageBuilder`](super::StorageBuilder). Every call to
/// [`StorageBuilder::create`](super::StorageBuilder::create) invokes
/// [`Self::create_policy`] and hands the freshly-built policy to the new
/// storage. Storages built from the same builder therefore have completely
/// independent locks.
pub trait LockingPolicyFactory {
    /// Concrete policy produced by this factory.
    type Policy: LockingPolicy;

    /// Bool-cell whose synchronization matches this factory's locking
    /// regime. Used by engine-internal book-keeping that needs to
    /// publish a single bit across the same observer set as the
    /// storage index domain.
    type IndexFlag: super::IndexFlag;

    /// Sync-mode-aware shared handle for values that need to be
    /// shared across clones (e.g. a policy's internal storage).
    ///
    /// Under `NoLocking` this is `Rc<T>` (single-threaded); under
    /// `FullLocking` this is `Arc<T>` (fully thread-safe); under
    /// `IndexLocking` this is `IndexShared<T>` (`Send` but `!Sync`,
    /// matching the `AccountSync` engine contract).
    type Shared<T: 'static>: Clone + std::ops::Deref<Target = T>;

    /// Builds a fresh policy for a new storage instance.
    fn create_policy(&self) -> Self::Policy;

    /// Wraps `value` in the sync-mode-appropriate shared handle.
    fn new_shared<T: 'static>(value: T) -> Self::Shared<T>;
}

/// Marker that opts a type out of [`Send`] and [`Sync`].
///
/// Used by [`NoLocking`](super::NoLocking) to make the resulting storage
/// thread-local at compile time, since it provides no synchronization at
/// runtime.
pub(super) type NotThreadSafe = PhantomData<*const ()>;
