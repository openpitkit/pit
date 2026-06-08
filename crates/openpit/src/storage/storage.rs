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

//! Generic key-value storage with pluggable synchronization.

use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::hash::Hash;

#[cfg(debug_assertions)]
mod reentry {
    use std::cell::RefCell;
    use std::collections::HashMap;

    std::thread_local! {
        static LOCAL_VALUES_HOLDS: RefCell<HashMap<usize, LocalValuesState>> =
            RefCell::new(HashMap::new());
    }

    /// Debug-only `Storage`-level re-entry tracker.
    ///
    /// Tracks closure re-entry on the values domain of one storage
    /// instance. Multiple read holds may coexist; a write hold is
    /// exclusive against every other hold (read or write).
    ///
    /// Per-storage, per-thread tracker. The thread-local map distinguishes
    /// recursive access (same OS thread) from concurrent access (different
    /// OS threads): a writer waiting on a parking_lot rwlock from another
    /// thread is not a contract violation, but a read-from-write inside the
    /// same closure call stack is.
    pub(super) struct ReentryFlag {
        _address: u8,
    }

    impl ReentryFlag {
        pub(super) fn new() -> Self {
            Self { _address: 0 }
        }

        pub(super) fn acquire_values_read(&self) -> ReentryReadGuard<'_> {
            let key = self.key();
            LOCAL_VALUES_HOLDS.with(|holds| {
                let mut holds = holds.borrow_mut();
                let state = holds.entry(key).or_default();
                assert!(
                    !state.write_held,
                    "openpit::storage: closure re-entered the same storage: \
                     Storage::with called from inside a Storage::with_mut \
                     closure on the same storage; a write hold conflicts \
                     with any other hold on the values domain"
                );
                state.read_depth += 1;
            });
            ReentryReadGuard {
                key,
                _marker: std::marker::PhantomData,
            }
        }

        pub(super) fn acquire_values_write(&self) -> ReentryWriteGuard<'_> {
            let key = self.key();
            LOCAL_VALUES_HOLDS.with(|holds| {
                let mut holds = holds.borrow_mut();
                let state = holds.entry(key).or_default();
                assert!(
                    state.read_depth == 0,
                    "openpit::storage: closure re-entered the same storage: \
                     Storage::with_mut called from inside a Storage::with \
                     closure on the same storage; a write hold conflicts \
                     with any read hold on the values domain"
                );
                assert!(
                    !state.write_held,
                    "openpit::storage: closure re-entered the same storage: \
                     Storage::with_mut called from inside another \
                     Storage::with_mut closure on the same storage; write \
                     holds do not stack"
                );
                state.write_held = true;
            });
            ReentryWriteGuard {
                key,
                _marker: std::marker::PhantomData,
            }
        }

        fn key(&self) -> usize {
            self as *const Self as usize
        }
    }

    #[derive(Clone, Copy, Default)]
    struct LocalValuesState {
        read_depth: usize,
        write_held: bool,
    }

    pub(super) struct ReentryReadGuard<'a> {
        key: usize,
        _marker: std::marker::PhantomData<&'a ReentryFlag>,
    }

    impl Drop for ReentryReadGuard<'_> {
        fn drop(&mut self) {
            LOCAL_VALUES_HOLDS.with(|holds| {
                let mut holds = holds.borrow_mut();
                if let Some(state) = holds.get_mut(&self.key) {
                    state.read_depth -= 1;
                    if state.read_depth == 0 && !state.write_held {
                        holds.remove(&self.key);
                    }
                }
            });
        }
    }

    pub(super) struct ReentryWriteGuard<'a> {
        key: usize,
        _marker: std::marker::PhantomData<&'a ReentryFlag>,
    }

    impl Drop for ReentryWriteGuard<'_> {
        fn drop(&mut self) {
            LOCAL_VALUES_HOLDS.with(|holds| {
                let mut holds = holds.borrow_mut();
                if let Some(state) = holds.get_mut(&self.key) {
                    state.write_held = false;
                    if state.read_depth == 0 {
                        holds.remove(&self.key);
                    }
                }
            });
        }
    }
}

/// Generic key-value storage whose synchronization is configured at the
/// type level by `LockingPolicy: LockingPolicy`.
///
/// # Purpose
///
/// Trading policies built on top of `openpit` often maintain internal
/// state — for example, reserved margin, open positions, and rate-limit
/// counters — that must be thread-safe when shared across engine threads.
/// Without a common abstraction each policy would have to implement its
/// own synchronization discipline: choose the right lock primitive,
/// acquire locks in a consistent order, ensure that the acquisition order
/// is respected across refactors, and handle the no-sync case for
/// single-threaded embeddings without conditional compilation.
///
/// `Storage` solves that problem once. A policy chooses a
/// [`LockingPolicyFactory`](super::LockingPolicyFactory) at construction
/// time and receives a ready-made `Storage` via
/// [`StorageBuilder::create`](super::StorageBuilder::create). From that
/// point on the policy reads and mutates its state through the scoped
/// [`Storage::with`] and [`Storage::with_mut`] methods, without ever
/// touching a lock primitive directly. If the embedding switches from
/// single-threaded to multi-threaded execution, only the factory type
/// changes; the policy logic is untouched.
///
/// # API surface
///
/// All accessors are scoped: the caller passes a closure that receives a
/// reference to the value, and the closure runs while the storage holds
/// the appropriate locks. Once the closure returns, the locks are
/// released. The reference never escapes the closure, which makes it
/// impossible for safe code to keep conflicting references to the same
/// value alive at the same time.
///
/// * [`Storage::with`] — invokes the closure with `&Value` if the key
///   exists; returns the closure's result wrapped in `Some`, or `None`
///   if the key is absent.
/// * [`Storage::with_mut`] — invokes the closure with `&mut Value`,
///   creating the entry on demand. The closure is also told whether the
///   entry was just inserted.
/// * [`Storage::remove`] — drops an entry; returns `true` if the key was
///   present.
/// * [`Storage::len`], [`Storage::is_empty`] — observers of the index.
///
/// All accessors take `&self` and acquire the locks defined by
/// `LockingPolicy`. Acquisition order is **always index before values**,
/// eliminating one common source of deadlocks.
///
/// # Closure re-entry
///
/// Calls into the same `Storage` from inside a closure passed to
/// [`Storage::with`] or [`Storage::with_mut`] follow these rules:
///
/// - A nested call to [`Storage::with`] from inside another
///   [`Storage::with`] closure on the same storage is allowed.
///   Multiple read holds on the values domain are sound: each yields
///   a shared `&Value` reference, and shared references do not alias
///   as `&mut` in safe code.
/// - A nested call to [`Storage::with_mut`] from inside any closure
///   on the same storage is forbidden. A write hold would produce
///   `&mut Value`, which conflicts with any other reference live on
///   the same value.
/// - A nested call to [`Storage::with`] from inside a
///   [`Storage::with_mut`] closure on the same storage is forbidden,
///   for the same aliasing reason.
///
/// Violations of the second and third rules are detected at runtime
/// in debug builds and panic with a clear message; release builds
/// skip the check for zero overhead and trust the contract.
///
/// For [`FullLocking`](super::FullLocking), nested read access uses
/// recursive read locks. Forbidden write re-entry may still deadlock
/// instead of panicking on release builds; the debug-only check upgrades
/// this to a clear panic during development.
///
/// # Cloning
///
/// [`Storage`] is intentionally not [`Clone`]. The synchronization
/// primitives owned by the policy are tied to one storage instance; any
/// notion of "the same storage" must be expressed by sharing through
/// `Arc<Storage<...>>`, never by copying.
///
/// # Thread safety
///
/// The thread-safety properties of [`Storage`] are inherited from the
/// policy:
///
/// * With [`NoLocking`](super::NoLocking) the storage is `!Send` and
///   `!Sync`; the Rust type system enforces single-threaded use.
/// * With [`IndexLocking`](super::IndexLocking) the storage is `Send`
///   (provided `Key` and `Value` are `Send`) but `!Sync`: the policy
///   synchronizes only the index domain. Ownership may be moved between
///   threads, but `&Storage` must not be shared simultaneously across
///   threads.
/// * With [`FullLocking`](super::FullLocking) the storage is
///   `Send + Sync` provided `Key` and `Value` themselves are
///   `Send + Sync`. `Sync` is gated on the policy implementing
///   [`FullySynchronized`](super::FullySynchronized).
pub struct Storage<Key, Value, LockingPolicy>
where
    LockingPolicy: super::policy::LockingPolicy,
{
    // Drop order: map first (drops every value), then policy (drops
    // every lock). Reversing would attempt to release locks before the
    // values they guard are gone, which is harmless in practice but
    // semantically unclean.
    data: UnsafeCell<HashMap<Key, Box<UnsafeCell<Value>>>>,
    locking_policy: LockingPolicy,
    #[cfg(debug_assertions)]
    reentry: reentry::ReentryFlag,
}

// SAFETY: `Storage`'s synchronization is provided by `LockingPolicy`. The
// map is only ever touched while the corresponding policy guards are
// held, and every reference handed to user closures is bounded by the
// closure's lifetime - it cannot outlive the call that produced it.
//
// Sharing `&Storage` between threads is sound only when the policy
// synchronizes BOTH the index domain AND the values domain. That
// stronger guarantee is witnessed at the type level by the
// `FullySynchronized` marker (see the `Sync` impl below), so a policy
// that synchronizes only the index (or nothing at all) yields a
// `Storage` that is `Send` but `!Sync`: ownership can move between
// threads or be guarded by the caller's own `Mutex`, but `&Storage`
// cannot be shared concurrently.
//
// `NoLockingPolicy` carries a `*const ()` marker that suppresses both
// `Send` and `Sync` for the storage, making it strictly thread-local
// at compile time.
unsafe impl<Key, Value, LockingPolicy> Send for Storage<Key, Value, LockingPolicy>
where
    LockingPolicy: super::policy::LockingPolicy + Send,
    Key: Send,
    Value: Send,
{
}

// SAFETY: see the comment block above; `FullySynchronized` is the
// type-level witness that the policy synchronizes both lock domains.
unsafe impl<Key, Value, LockingPolicy> Sync for Storage<Key, Value, LockingPolicy>
where
    LockingPolicy: super::policy::FullySynchronized + Sync,
    Key: Send + Sync,
    Value: Send + Sync,
{
}

impl<Key, Value, LockingPolicy> Storage<Key, Value, LockingPolicy>
where
    LockingPolicy: super::policy::LockingPolicy,
{
    /// Builds an empty storage with the given policy.
    pub(super) fn with_locking_policy(locking_policy: LockingPolicy) -> Self {
        Self {
            locking_policy,
            data: UnsafeCell::new(HashMap::new()),
            #[cfg(debug_assertions)]
            reentry: reentry::ReentryFlag::new(),
        }
    }

    /// Builds an empty storage with the given policy and an initial
    /// capacity hint for the underlying map.
    pub(super) fn with_locking_policy_and_capacity(
        locking_policy: LockingPolicy,
        capacity: usize,
    ) -> Self {
        Self {
            locking_policy,
            data: UnsafeCell::new(HashMap::with_capacity(capacity)),
            #[cfg(debug_assertions)]
            reentry: reentry::ReentryFlag::new(),
        }
    }

    /// Returns the number of entries currently in the storage.
    ///
    /// Acquires the index domain in shared mode for the duration of the
    /// call. The returned count is a snapshot at the moment of the
    /// call; with thread-safe policies it may be stale by the time the
    /// caller inspects it.
    pub fn len(&self) -> usize {
        let _index = self.locking_policy.read_index();
        // SAFETY: index shared lock prevents concurrent structural
        // mutation of the map, which is all `len` needs.
        unsafe { (*self.data.get()).len() }
    }

    /// Returns `true` if the storage holds no entries.
    ///
    /// See [`Storage::len`] for the synchronization semantics.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<Key, Value, LockingPolicy> Storage<Key, Value, LockingPolicy>
where
    Key: Hash + Eq,
    LockingPolicy: super::policy::LockingPolicy,
{
    /// Read-only scoped access to one entry.
    ///
    /// `reader` is invoked with a shared reference to the value if the
    /// key is present. The storage holds the index and values domains
    /// in shared mode for the duration of the call. Returns
    /// `Some(reader's result)` on a hit, `None` on a miss.
    ///
    /// `reader` may call [`Storage::with`] back into the same storage
    /// on any key (read-from-read is sound). `reader` must NOT call
    /// [`Storage::with_mut`] on the same storage; see the
    /// [closure re-entry note on `Storage`](Storage#closure-re-entry).
    pub fn with<Reader, Output>(&self, key: &Key, reader: Reader) -> Option<Output>
    where
        Reader: FnOnce(&Value) -> Output,
    {
        #[cfg(debug_assertions)]
        let _reentry = self.reentry.acquire_values_read();

        let _index_guard = self.locking_policy.read_index();
        // SAFETY: index shared lock blocks structural mutation, so the
        // map stays put for the duration of the call.
        let data = unsafe { &*self.data.get() };
        let value_box = data.get(key)?;
        let value_cell: &UnsafeCell<Value> = value_box.as_ref();
        let _values_guard = self.locking_policy.read_values(key);
        // SAFETY: values shared lock blocks any concurrent mutation of
        // the value, so reading `&Value` through the `UnsafeCell` for
        // the duration of `_values_guard` is sound. The reference is
        // confined to `reader`'s call and does not escape.
        let value: &Value = unsafe { &*value_cell.get() };
        Some(reader(value))
    }

    /// Read/write scoped access to one entry; **does not insert on miss**.
    ///
    /// If the entry is present, `mutator` is invoked with an exclusive
    /// reference to the value. The storage holds the index domain in
    /// shared mode and the values domain exclusively for the duration of
    /// the call. Returns `Some(mutator's result)` on a hit, `None` on a
    /// miss without inserting any default.
    ///
    /// Use this when the operation is only meaningful for entries that
    /// already exist — for example, consuming from a hold that may not
    /// exist — so that absent keys never create phantom entries.
    ///
    /// `mutator` must not call [`Storage::with`] or
    /// [`Storage::with_mut`] back into the same storage; see the
    /// [closure re-entry note on `Storage`](Storage#closure-re-entry).
    pub fn with_mut_if_present<Mutator, Output>(
        &self,
        key: &Key,
        mutator: Mutator,
    ) -> Option<Output>
    where
        Mutator: FnOnce(&mut Value) -> Output,
    {
        #[cfg(debug_assertions)]
        let _reentry = self.reentry.acquire_values_write();

        let _index_guard = self.locking_policy.read_index();
        // SAFETY: index shared lock blocks structural mutation, so the
        // map stays put for the duration of the call.
        let data = unsafe { &*self.data.get() };
        let value_box = data.get(key)?;
        let value_cell: &UnsafeCell<Value> = value_box.as_ref();
        let _values_guard = self.locking_policy.write_values(key);
        // SAFETY: index shared + values exclusive uphold the aliasing
        // invariant for the `&mut Value` exposed to `mutator`. The `Box`
        // indirection ensures address stability for the duration of the
        // shared index lock. The reference is confined to `mutator`'s
        // call and does not escape.
        let value: &mut Value = unsafe { &mut *value_cell.get() };
        Some(mutator(value))
    }

    /// Read/write scoped access to one entry; inserts on demand.
    ///
    /// `mutator` is always invoked. It receives a mutable reference to
    /// the value and a flag telling whether the entry was just inserted
    /// (`true`) or already existed (`false`). `default` is called at
    /// most once and only when the entry needs to be created.
    ///
    /// The storage holds the values domain exclusively for the duration
    /// of the call, plus the index domain in either shared mode
    /// (existing entry) or exclusive mode (newly inserted entry).
    ///
    /// `Key: Clone` is required to support the slow path: when the
    /// entry has to be inserted, the key is moved into the map and the
    /// implementation needs an independent reference to it for the
    /// re-lookup.
    ///
    /// `mutator` must not call [`Storage::with`] or
    /// [`Storage::with_mut`] back into the same storage; see the
    /// [closure re-entry note on `Storage`](Storage#closure-re-entry).
    pub fn with_mut<Mutator, Output, Initializer>(
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
        #[cfg(debug_assertions)]
        let _reentry = self.reentry.acquire_values_write();

        // Fast path: take the index shared and look the key up. Most
        // callers hit this path repeatedly, so it should not block
        // concurrent readers on other entries.
        {
            let index_guard = self.locking_policy.read_index();
            // SAFETY: index shared lock blocks structural mutation.
            let data = unsafe { &*self.data.get() };
            if let Some(value_box) = data.get(&key) {
                let value_cell: &UnsafeCell<Value> = value_box.as_ref();
                let _values_guard = self.locking_policy.write_values(&key);
                // SAFETY: index shared + values exclusive uphold the
                // aliasing invariant for the `&mut Value` exposed to
                // `mutator`. The `Box` indirection keeps the value
                // address stable independently of any rehash that
                // would happen if the index were ever taken
                // exclusively (it cannot be while we hold the shared
                // index lock). The reference is confined to
                // `mutator`'s call and does not escape.
                let value: &mut Value = unsafe { &mut *value_cell.get() };
                let result = mutator(value, false);
                drop(index_guard);
                return result;
            }
        }

        // Slow path: the key was absent under the shared index lock.
        // Re-take the index exclusively, which also serializes us
        // against any other thread that may have raced to insert in
        // the meantime.
        let _index_guard = self.locking_policy.write_index();
        // SAFETY: index exclusive lock; we have unique access to the
        // map and may rehash freely.
        let data = unsafe { &mut *self.data.get() };
        // `data.entry(key)` moves `key` into the map; clone it first so we can
        // pass the same key to `write_values` below.
        use std::collections::hash_map::Entry;

        let lookup_key = key.clone();
        let (value_cell, already_present): (&UnsafeCell<Value>, bool) = match data.entry(key) {
            Entry::Occupied(entry) => (&**entry.into_mut(), true),
            Entry::Vacant(entry) => (&**entry.insert(Box::new(UnsafeCell::new(default()))), false),
        };
        // After this point no further structural mutation happens;
        // bucket addresses inside the map are stable for as long as we
        // hold the exclusive index lock.
        let _values_guard = self.locking_policy.write_values(&lookup_key);
        // SAFETY: index exclusive + values exclusive => no other
        // thread can observe or touch this entry; the `&mut Value`
        // exposed to `mutator` is exclusive for the duration of the
        // call. Address stability is guaranteed by holding the
        // exclusive index lock until the values guard drops.
        let value: &mut Value = unsafe { &mut *value_cell.get() };
        mutator(value, !already_present)
    }

    /// Read/write scoped access to one entry; inserts on demand and rolls
    /// back the insert when `mutator` returns `Err`.
    ///
    /// Behaves like [`with_mut`](Self::with_mut) but the `mutator` closure
    /// returns `Result<Output, Error>`:
    ///
    /// * **Fast path** (existing entry): acquires index shared + values
    ///   exclusive. `mutator` receives `&mut Value`; any mutations it makes
    ///   are visible whether it returns `Ok` or `Err` — storage does **not**
    ///   snapshot or restore the value. If `mutator` needs to leave the entry
    ///   untouched on failure it must avoid writing before it knows the
    ///   operation will succeed.
    /// * **Slow path** (new entry): acquires index exclusive, inserts
    ///   `default()`, acquires values exclusive, and invokes `mutator`.
    ///   If `mutator` returns `Err`, the just-inserted entry is removed
    ///   under the same exclusive index lock — no phantom entry remains.
    ///   If `mutator` returns `Ok`, the entry is kept.
    ///
    /// `Key: Clone` is required for the same reason as in `with_mut`.
    ///
    /// `mutator` must not call [`Storage::with`] or [`Storage::with_mut`]
    /// back into the same storage; see the
    /// [closure re-entry note on `Storage`](Storage#closure-re-entry).
    pub fn with_mut_or_insert<Mutator, Output, Error, Initializer>(
        &self,
        key: Key,
        default: Initializer,
        mutator: Mutator,
    ) -> Result<Output, Error>
    where
        Mutator: FnOnce(&mut Value, bool) -> Result<Output, Error>,
        Initializer: FnOnce() -> Value,
        Key: Clone,
    {
        self.with_mut_or_insert_prune_new_if(key, default, |_| false, mutator)
    }

    /// Like [`with_mut_or_insert`](Self::with_mut_or_insert) but, when the entry was **newly inserted**
    /// and the mutation returned `Ok`, calls `prune_if` on the current slot
    /// value and removes the entry atomically under the same exclusive-index
    /// lock if `prune_if` returns `true`.
    ///
    /// This eliminates the window in which a just-inserted entry with an
    /// unwanted value (e.g. all-zero) could be observed by another thread.
    /// For entries that were already present before the call, behaviour is
    /// identical to [`with_mut_or_insert`](Self::with_mut_or_insert); pruning of existing entries that
    /// satisfy the predicate after mutation is the caller's responsibility
    /// (see [`remove_if`](Self::remove_if)).
    pub fn with_mut_or_insert_prune_new_if<Mutator, Output, Error, Initializer, Pred>(
        &self,
        key: Key,
        default: Initializer,
        prune_if: Pred,
        mutator: Mutator,
    ) -> Result<Output, Error>
    where
        Mutator: FnOnce(&mut Value, bool) -> Result<Output, Error>,
        Initializer: FnOnce() -> Value,
        Pred: FnOnce(&Value) -> bool,
        Key: Clone,
    {
        #[cfg(debug_assertions)]
        let _reentry = self.reentry.acquire_values_write();

        // Fast path: take the index shared and look the key up. Most
        // callers hit this path repeatedly, so it should not block
        // concurrent readers on other entries.
        {
            let index_guard = self.locking_policy.read_index();
            // SAFETY: index shared lock blocks structural mutation, so the
            // map stays put for the duration of the call.
            let data = unsafe { &*self.data.get() };
            if let Some(value_box) = data.get(&key) {
                let value_cell: &UnsafeCell<Value> = value_box.as_ref();
                let _values_guard = self.locking_policy.write_values(&key);
                // SAFETY: index shared + values exclusive uphold the
                // aliasing invariant for the `&mut Value` exposed to
                // `mutator`. The `Box` indirection keeps the value address
                // stable independently of any rehash that would happen if the
                // index were ever taken exclusively (it cannot be while we
                // hold the shared index lock). The reference is confined to
                // `mutator`'s call and does not escape.
                let result = {
                    let value: &mut Value = unsafe { &mut *value_cell.get() };
                    mutator(value, false)
                };
                drop(index_guard);
                return result;
            }
        }

        // Slow path: the key was absent under the shared index lock.
        // Re-take the index exclusively, which also serializes us against
        // any other thread that may have raced to insert in the meantime.
        let _index_guard = self.locking_policy.write_index();
        // SAFETY: index exclusive lock; we have unique access to the map
        // and may rehash freely.
        let data = unsafe { &mut *self.data.get() };
        // `data.entry(key)` moves `key` into the map; clone it first so we
        // can pass the same key to `write_values` below.
        use std::collections::hash_map::Entry;

        let lookup_key = key.clone();
        let (value_cell, already_present): (&UnsafeCell<Value>, bool) = match data.entry(key) {
            Entry::Occupied(entry) => (&**entry.into_mut(), true),
            Entry::Vacant(entry) => (&**entry.insert(Box::new(UnsafeCell::new(default()))), false),
        };
        // After this point no further structural mutation happens; bucket
        // addresses inside the map are stable for as long as we hold the
        // exclusive index lock.
        let _values_guard = self.locking_policy.write_values(&lookup_key);
        // SAFETY: index exclusive + values exclusive => no other thread can
        // observe or touch this entry; the `&mut Value` exposed to `mutator`
        // is exclusive for the duration of the call. Address stability is
        // guaranteed by holding the exclusive index lock until `value` drops.
        let result = {
            let value: &mut Value = unsafe { &mut *value_cell.get() };
            mutator(value, !already_present)
            // `value` drops here, before the potential remove below.
        };
        if !already_present {
            // Under the same write_index lock: remove the just-inserted entry
            // on Err (rollback) or when prune_if approves on Ok.
            // SAFETY: `value` was dropped above; reading the slot via
            // `value_cell` for the prune predicate does not alias any live
            // `&mut Value`.
            let should_remove = match &result {
                Err(_) => true,
                Ok(_) => prune_if(unsafe { &*value_cell.get() }),
            };
            if should_remove {
                data.remove(&lookup_key);
            }
        }
        result
    }

    /// Removes the entry under `key`, returning whether it was present.
    ///
    /// Acquires the index and values domains exclusively for the
    /// duration of the call.
    pub fn remove(&self, key: &Key) -> bool {
        let _index_guard = self.locking_policy.write_index();
        let _values_guard = self.locking_policy.write_values(key);
        // SAFETY: both domains held exclusively => unique access to
        // the map and to every value currently inside it.
        let data = unsafe { &mut *self.data.get() };
        data.remove(key).is_some()
    }

    /// Removes the entry under `key` only if `predicate` returns `true`
    /// for its current value.
    ///
    /// The check and the removal happen under the same exclusive lock
    /// acquisition, so there is no window for another thread to change
    /// the value between the test and the removal.
    ///
    /// Returns `true` when the entry existed and `predicate` approved the
    /// removal; `false` when the entry was absent or `predicate` declined.
    pub fn remove_if<Pred>(&self, key: &Key, predicate: Pred) -> bool
    where
        Pred: FnOnce(&Value) -> bool,
    {
        let _index_guard = self.locking_policy.write_index();
        let _values_guard = self.locking_policy.write_values(key);
        // SAFETY: both domains held exclusively => unique access to
        // the map and to every value currently inside it.
        let data = unsafe { &mut *self.data.get() };
        match data.get(key) {
            Some(value_box) if predicate(unsafe { &*value_box.get() }) => {
                data.remove(key);
                true
            }
            _ => false,
        }
    }
}
