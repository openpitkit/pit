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

//! Single-threaded, no-synchronization locking policy.

use std::marker::PhantomData;

use crate::storage::policy::{LockingPolicy, LockingPolicyFactory, NotThreadSafe};

/// Locking policy factory that performs **no synchronization** at all.
///
/// A [`Storage`](crate::storage::Storage) configured with `NoLocking`
/// has zero runtime synchronization overhead but is not thread-safe: it
/// is `!Send` and `!Sync`. The Rust type system enforces single-threaded
/// use.
///
/// # Aliasing safety
///
/// The closure-based access methods on
/// [`Storage`](crate::storage::Storage) ensure that references handed to
/// the caller never escape the call that produced them. Read-from-read
/// re-entry is allowed because it creates only shared references. Any
/// re-entry path involving a write hold is detected by `Storage` in
/// debug builds and panics with a clear message; release builds skip the
/// check and pay nothing extra.
///
/// Use this when the storage lives entirely on the stack of one thread
/// or behind an external synchronization primitive of the caller's
/// choosing.
///
/// # Examples
///
/// ```
/// use openpit::Engine;
///
/// let builder = Engine::builder::<(), (), ()>().no_sync();
/// let storage = builder.storage_builder().create_for_bound_key::<u32, String>();
/// storage.with_mut(1, || "hello".to_string(), |entry, is_new| {
///     assert!(is_new);
///     entry.push_str(", world");
/// });
/// assert_eq!(
///     storage.with(&1, |v| v.clone()),
///     Some("hello, world".to_string()),
/// );
/// ```
#[derive(Debug, Default, Clone, Copy)]
pub struct NoLocking;

impl LockingPolicyFactory for NoLocking {
    type Policy = NoLockingPolicy;

    /// Single-thread regime; `Cell<bool>` needs no synchronization.
    type IndexFlag = std::cell::Cell<bool>;

    type Shared<T: 'static> = std::rc::Rc<T>;

    fn create_policy(&self) -> Self::Policy {
        NoLockingPolicy {
            _not_thread_safe: PhantomData,
        }
    }

    fn new_shared<T: 'static>(value: T) -> std::rc::Rc<T> {
        std::rc::Rc::new(value)
    }
}

/// Concrete no-op policy installed in storages built from
/// [`NoLocking`].
///
/// Held privately by the storage; users never name it directly. Not
/// re-exported from the crate; name it via the associated type if
/// needed: `<NoLocking as LockingPolicyFactory>::Policy`.
pub struct NoLockingPolicy {
    _not_thread_safe: NotThreadSafe,
}

// SAFETY: every locking method is a no-op. The storage that owns this
// policy is `!Sync` and `!Send` because of the `NotThreadSafe` marker,
// so the Rust type system guarantees that no two threads ever observe the
// same storage through this policy. Each method returns a no-op guard,
// matching the no-synchronization contract.
unsafe impl LockingPolicy for NoLockingPolicy {
    type IndexSharedGuard<'a> = ();
    type IndexExclusiveGuard<'a> = ();
    type ValuesSharedGuard<'a> = ();
    type ValuesExclusiveGuard<'a> = ();

    #[inline]
    fn read_index(&self) -> Self::IndexSharedGuard<'_> {}

    #[inline]
    fn write_index(&self) -> Self::IndexExclusiveGuard<'_> {}

    #[inline]
    fn read_values<Key>(&self, _key: &Key) -> Self::ValuesSharedGuard<'_> {}

    #[inline]
    fn write_values<Key>(&self, _key: &Key) -> Self::ValuesExclusiveGuard<'_> {}
}
