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

//! Storage builder.

use super::key_bound::CreateStorageFor;
use super::storage::Storage;

/// Builder for [`Storage`] instances configured with a specific
/// [`LockingPolicyFactory`](super::LockingPolicyFactory).
///
/// # Purpose
///
/// Trading policies built on `openpit` typically maintain in-memory state
/// (reserved margin, position tracking, rate-limit counters, …) that must
/// be thread-safe in multi-threaded embeddings yet have zero overhead in
/// single-threaded ones. Rather than implementing ad hoc synchronization
/// in every policy, the recommended pattern is:
///
/// 1. Accept a `&StorageBuilder<Factory>` obtained from an engine builder
///    at policy construction time.
/// 2. Call [`create_for_bound_key`](Self::create_for_bound_key) for each internal data table.
/// 3. Operate on the resulting [`Storage`] instances exclusively through
///    [`Storage::with`](super::Storage::with) and
///    [`Storage::with_mut`](super::Storage::with_mut).
///
/// The synchronization mode is then entirely determined by the engine's
/// synchronization mode, not by the policy. Switching from no-sync to
/// fully-synchronized execution only requires changing the engine builder's
/// sync mode - the policy logic is untouched.
///
/// # Lifetime discipline
///
/// `StorageBuilder` is intentionally **not [`Clone`]**. The intended usage
/// is for the engine builder to own it during the application's
/// initialization phase. Application code passes
/// [`SyncedEngineBuilder::storage_builder`](crate::SyncedEngineBuilder::storage_builder)
/// by shared reference to every policy that needs to create storages.
///
/// Storing a `StorageBuilder` inside a policy — even by value through
/// unsafe code — is a misuse: the builder is only meaningful during
/// initialization, after which the storages it produced are already live
/// and the builder itself carries no additional information.
///
/// # Examples
///
/// ```
/// use openpit::Engine;
///
/// let engine_builder = Engine::builder::<(), (), ()>().full_sync();
/// let users = engine_builder.storage_builder().create_for_bound_key::<u64, String>();
/// let orders = engine_builder.storage_builder().create_for_bound_key::<u64, Vec<u8>>();
/// // `users` and `orders` are unrelated storages; locking one does
/// // not affect the other.
/// users.with_mut(1, || "alice".to_string(), |_, _| {});
/// orders.with_mut(42, Vec::new, |_, _| {});
/// ```
pub struct StorageBuilder<LockingPolicyFactory>
where
    LockingPolicyFactory: super::policy::LockingPolicyFactory,
{
    locking_policy_factory: LockingPolicyFactory,
}

impl<LockingPolicyFactory> StorageBuilder<LockingPolicyFactory>
where
    LockingPolicyFactory: super::policy::LockingPolicyFactory,
{
    /// Creates a builder from a factory instance.
    #[inline]
    pub(crate) fn new(factory: LockingPolicyFactory) -> Self {
        Self {
            locking_policy_factory: factory,
        }
    }

    /// Creates an empty storage configured by this builder's policy
    /// factory.
    ///
    /// Each call obtains a fresh [`LockingPolicy`](super::LockingPolicy)
    /// instance from the factory; the resulting storage's locks are
    /// independent of any other storage created by this builder.
    ///
    /// The factory must declare it supports `Key` by implementing
    /// [`CreateStorageFor<Key>`](super::CreateStorageFor). All built-in
    /// factories ship with the appropriate impls.
    #[inline]
    pub fn create_for_bound_key<Key, Value>(
        &self,
    ) -> Storage<Key, Value, LockingPolicyFactory::Policy>
    where
        LockingPolicyFactory: CreateStorageFor<Key>,
    {
        Storage::with_locking_policy(self.locking_policy_factory.create_policy())
    }

    /// Creates an empty storage configured by this builder's policy factory,
    /// bypassing the [`CreateStorageFor<Key>`](super::CreateStorageFor) key
    /// gate.
    ///
    /// Unlike [`create_for_bound_key`](Self::create_for_bound_key), this does
    /// **not** require the factory to admit `Key`. Any key type may be used,
    /// including those that
    /// [`CreateStorageFor<Key>`](super::CreateStorageFor) would reject under a
    /// stricter bound. [`AnyKey`](super::AnyKey) is the loosest bound and
    /// admits every key; this method bypasses the
    /// [`CreateStorageFor<Key>`](super::CreateStorageFor) gate entirely.
    #[inline]
    pub(crate) fn create_for_any_key<Key, Value>(
        &self,
    ) -> Storage<Key, Value, LockingPolicyFactory::Policy> {
        Storage::with_locking_policy(self.locking_policy_factory.create_policy())
    }

    /// Creates a fresh standalone [`LockingPolicy`](super::LockingPolicy)
    /// instance from this builder's factory, independent of any storage.
    ///
    /// Engine-internal facilities that must serialize a multi-key mutation
    /// as a single all-or-nothing section use the returned policy's index
    /// domain as a whole-map guard: the guard is a zero-cost no-op under the
    /// single-thread regime and a real reader-writer lock under the
    /// fully-synchronized regime, matching the engine's synchronization mode
    /// without introducing a separate lock primitive.
    #[inline]
    pub(crate) fn create_policy(&self) -> LockingPolicyFactory::Policy {
        self.locking_policy_factory.create_policy()
    }

    /// Creates an empty storage wrapped in a [`LockingPolicyFactory::Shared`](crate::storage::LockingPolicyFactory::Shared)
    /// handle.
    ///
    /// Equivalent to `Factory::new_shared(self.create_for_bound_key::<Key, Value>())`.
    /// Use this when the storage needs to be shared across clones of the
    /// same policy.
    #[inline]
    pub fn create_shared<Key, Value>(
        &self,
    ) -> LockingPolicyFactory::Shared<Storage<Key, Value, LockingPolicyFactory::Policy>>
    where
        LockingPolicyFactory: CreateStorageFor<Key>,
    {
        <LockingPolicyFactory as super::policy::LockingPolicyFactory>::new_shared(
            self.create_for_bound_key::<Key, Value>(),
        )
    }

    /// Creates an empty storage with an initial capacity hint for the
    /// underlying map.
    ///
    /// Otherwise identical to [`Self::create_for_bound_key`]; useful when the
    /// rough number of entries is known up front and the caller wants to
    /// avoid rehashing during the warm-up phase.
    #[inline]
    pub fn create_with_capacity<Key, Value>(
        &self,
        capacity: usize,
    ) -> Storage<Key, Value, LockingPolicyFactory::Policy>
    where
        LockingPolicyFactory: CreateStorageFor<Key>,
    {
        Storage::with_locking_policy_and_capacity(
            self.locking_policy_factory.create_policy(),
            capacity,
        )
    }
}
