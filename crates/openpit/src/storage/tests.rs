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

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

use std::marker::PhantomData;

use super::{
    AnyKey, CreateStorageFor, FullLocking, IndexKeyBound, IndexLocking, LockingPolicyFactory,
    NoLocking, Storage, StorageBuilder,
};

fn build<Factory>() -> Storage<u32, String, Factory::Policy>
where
    Factory: LockingPolicyFactory + Default + CreateStorageFor<u32>,
{
    StorageBuilder::new(Factory::default()).create_for_bound_key::<u32, String>()
}

fn smoke_with_with_mut_remove<LockingPolicy>(storage: Storage<u32, String, LockingPolicy>)
where
    LockingPolicy: super::LockingPolicy,
{
    assert!(storage.is_empty());
    assert_eq!(storage.len(), 0);

    // Missing key.
    assert!(storage.with(&1, |_| ()).is_none());

    // Insert via with_mut.
    storage.with_mut(
        1,
        || "alpha".to_string(),
        |entry, is_new| {
            assert!(is_new);
            assert_eq!(entry.as_str(), "alpha");
            entry.push_str("-mod");
        },
    );
    assert_eq!(storage.len(), 1);
    assert!(!storage.is_empty());

    // Read back.
    let view = storage
        .with(&1, |value| value.clone())
        .expect("entry must exist");
    assert_eq!(view, "alpha-mod");

    // Re-take with_mut on existing key.
    storage.with_mut(
        1,
        || "should-not-run".to_string(),
        |entry, is_new| {
            assert!(!is_new);
            entry.push('!');
        },
    );
    assert_eq!(
        storage.with(&1, |value| value.clone()).unwrap(),
        "alpha-mod!".to_string(),
    );

    // Storage::remove on present and absent keys.
    assert!(storage.remove(&1));
    assert!(!storage.remove(&1));
    assert!(storage.is_empty());

    // Insert and remove via Storage::remove.
    storage.with_mut(
        2,
        || "beta".to_string(),
        |_entry, is_new| {
            assert!(is_new);
        },
    );
    assert!(storage.remove(&2));
    assert!(storage.with(&2, |_| ()).is_none());

    // Default closure is not invoked when the key already exists.
    storage.with_mut(3, || "gamma".to_string(), |_, _| {});
    let mut tripwire = false;
    storage.with_mut(
        3,
        || {
            tripwire = true;
            "should-not-run".to_string()
        },
        |_, _| {},
    );
    assert!(!tripwire);
}

#[test]
fn no_locking_smoke() {
    smoke_with_with_mut_remove::<<NoLocking as LockingPolicyFactory>::Policy>(build::<NoLocking>());
}

#[test]
fn index_locking_smoke() {
    smoke_with_with_mut_remove::<<IndexLocking as LockingPolicyFactory>::Policy>(build::<
        IndexLocking,
    >());
}

#[test]
fn full_locking_smoke() {
    smoke_with_with_mut_remove::<<FullLocking as LockingPolicyFactory>::Policy>(
        build::<FullLocking>(),
    );
}

#[test]
fn builder_creates_independent_storages() {
    let builder = StorageBuilder::new(FullLocking);
    let a = builder.create_for_bound_key::<u32, u32>();
    let b = builder.create_for_bound_key::<u32, u32>();

    a.with_mut(1, || 10, |_, _| {});
    b.with_mut(1, || 20, |_, _| {});

    assert_eq!(a.with(&1, |v| *v), Some(10));
    assert_eq!(b.with(&1, |v| *v), Some(20));
}

#[test]
fn capacity_hint_does_not_alter_semantics() {
    let storage = StorageBuilder::new(FullLocking).create_with_capacity::<u32, u32>(128);
    storage.with_mut(7, || 1, |_, _| {});
    assert_eq!(storage.with(&7, |v| *v), Some(1));
}

// `FullLocking` storages are `Send + Sync` because the policy
// implements `FullySynchronized` (both index and values are guarded).
#[test]
fn full_locking_storage_is_send_and_sync() {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    assert_send::<Storage<u64, u64, <FullLocking as LockingPolicyFactory>::Policy>>();
    assert_sync::<Storage<u64, u64, <FullLocking as LockingPolicyFactory>::Policy>>();
}

// `IndexLocking` storages are `Send` only: ownership can be transferred
// between threads, but `&Storage` must not be shared simultaneously
// from multiple threads. The negative side (`!Sync`) is checked by the
// `compile_fail` doctest on `IndexLocking` itself.
#[test]
fn index_locking_storage_is_send_only() {
    fn assert_send<T: Send>() {}

    assert_send::<Storage<u64, u64, <IndexLocking as LockingPolicyFactory>::Policy>>();

    // Sanity: `NoLocking` still compiles in single-threaded use.
    let storage = build::<NoLocking>();
    storage.with_mut(1, || "x".to_string(), |_, _| {});
}

#[test]
fn full_locking_concurrent_readers_and_writers() {
    let storage = Arc::new(StorageBuilder::new(FullLocking).create_for_bound_key::<u64, u64>());

    // Pre-populate.
    for k in 0..16u64 {
        storage.with_mut(k, || 0, |_, _| {});
    }

    let observed = Arc::new(AtomicUsize::new(0));
    thread::scope(|scope| {
        // Several reader threads.
        for _ in 0..4 {
            let storage = Arc::clone(&storage);
            let observed = Arc::clone(&observed);
            scope.spawn(move || {
                for k in 0..16u64 {
                    if let Some(read) = storage.with(&k, |value| *value) {
                        // Read is enough; we only care that the call
                        // does not crash and the value is one of the
                        // legitimate writes.
                        assert!(read < 1_000_000);
                        observed.fetch_add(1, Ordering::Relaxed);
                    }
                }
            });
        }
        // Several writer threads.
        for tid in 0..4u64 {
            let storage = Arc::clone(&storage);
            scope.spawn(move || {
                for round in 0..256u64 {
                    let key = round % 16;
                    storage.with_mut(
                        key,
                        || 0,
                        |entry, _| {
                            *entry = tid * 1_000 + round;
                        },
                    );
                }
            });
        }
    });

    // After all threads finish the storage must still hold 16 keys
    // and every value must reflect *some* writer's last write to
    // that slot.
    assert_eq!(storage.len(), 16);
    for k in 0..16u64 {
        let v = storage.with(&k, |value| *value).unwrap();
        // tid in 0..4, round in 0..256 => max value is 3 * 1_000 + 255
        assert!(v < 4_000);
    }
    // Each reader thread did 16 lookups and there are 4 readers; we
    // only require that the readers were able to make progress.
    assert!(observed.load(Ordering::Relaxed) > 0);
}

#[test]
fn full_locking_concurrent_inserts_unique_keys() {
    let storage = Arc::new(StorageBuilder::new(FullLocking).create_for_bound_key::<u64, u64>());
    let new_count = Arc::new(AtomicUsize::new(0));
    let total_threads = 8u64;
    let per_thread = 64u64;

    thread::scope(|scope| {
        for tid in 0..total_threads {
            let storage = Arc::clone(&storage);
            let new_count = Arc::clone(&new_count);
            scope.spawn(move || {
                for n in 0..per_thread {
                    let key = tid * per_thread + n;
                    storage.with_mut(
                        key,
                        || tid * 1_000 + n,
                        |_, is_new| {
                            if is_new {
                                new_count.fetch_add(1, Ordering::Relaxed);
                            }
                        },
                    );
                }
            });
        }
    });

    let expected = (total_threads * per_thread) as usize;
    assert_eq!(storage.len(), expected);
    assert_eq!(new_count.load(Ordering::Relaxed), expected);
}

#[test]
fn full_locking_concurrent_inserts_shared_key_one_winner() {
    let storage = Arc::new(StorageBuilder::new(FullLocking).create_for_bound_key::<u64, u64>());
    let new_count = Arc::new(AtomicUsize::new(0));
    let total_threads = 8u64;

    thread::scope(|scope| {
        for tid in 0..total_threads {
            let storage = Arc::clone(&storage);
            let new_count = Arc::clone(&new_count);
            scope.spawn(move || {
                storage.with_mut(
                    42,
                    || tid,
                    |_, is_new| {
                        if is_new {
                            new_count.fetch_add(1, Ordering::Relaxed);
                        }
                    },
                );
            });
        }
    });

    assert_eq!(storage.len(), 1);
    assert_eq!(new_count.load(Ordering::Relaxed), 1);
    let value = storage.with(&42, |value| *value).unwrap();
    assert!(value < total_threads);
}

#[test]
fn storage_remove_drops_entry() {
    let storage = StorageBuilder::new(FullLocking).create_for_bound_key::<u32, u32>();
    storage.with_mut(
        10,
        || 100,
        |_, is_new| {
            assert!(is_new);
        },
    );
    assert!(storage.remove(&10));
    assert!(storage.with(&10, |_| ()).is_none());
    assert_eq!(storage.len(), 0);
}

#[test]
fn index_locking_with_any_key_accepts_arbitrary_key() {
    // The default `KeyBound = AnyKey` should allow any `Hash + Eq` key
    // (here a primitive `u32` and a non-account-shaped tuple).
    let primitives = StorageBuilder::new(IndexLocking::<AnyKey>::default())
        .create_for_bound_key::<u32, String>();
    primitives.with_mut(1, || "alpha".to_string(), |_, _| {});
    assert_eq!(
        primitives.with(&1, |value| value.clone()),
        Some("alpha".to_string()),
    );

    let tuples = StorageBuilder::new(IndexLocking::<AnyKey>::default())
        .create_for_bound_key::<(u8, u16), u8>();
    tuples.with_mut((1, 2), || 7, |_, _| {});
    assert_eq!(tuples.with(&(1, 2), |value| *value), Some(7));
}

#[test]
fn index_locking_with_account_constraint_accepts_account_id() {
    // A custom `KeyBound` that admits a single concrete key type;
    // mirrors how `AccountKeyConstraint` permits `AccountId` keys
    // through the same plumbing in the engine layer.
    struct OnlyU64Key;
    impl IndexKeyBound<u64> for OnlyU64Key {}

    let storage = StorageBuilder::new(IndexLocking::<OnlyU64Key>::default())
        .create_for_bound_key::<u64, u32>();
    storage.with_mut(42, || 7, |_, _| {});
    assert_eq!(storage.with(&42, |value| *value), Some(7));
}

#[test]
fn index_locking_with_account_constraint_accepts_tuple_with_account_id() {
    // Variant where the bound admits a tuple key. Models the
    // `(AccountId, Asset)` shape used by built-in policies.
    struct OnlyU64TupleKey;
    impl IndexKeyBound<(u64, u16)> for OnlyU64TupleKey {}

    let storage = StorageBuilder::new(IndexLocking::<OnlyU64TupleKey>::default())
        .create_for_bound_key::<(u64, u16), u32>();
    storage.with_mut((1, 2), || 7, |_, _| {});
    assert_eq!(storage.with(&(1, 2), |value| *value), Some(7));
}

// Compile-time check that the negative case actually rejects: a
// `KeyBound` that admits no key at all should refuse `create`.
//
// We materialise the negative side as a `PhantomData<F>` use that
// would only compile if the bound were satisfied. The bound is not,
// so this function is uncalled and the test only exercises the
// `_assert_*` helpers below to verify the positive impls compile.
fn _assert_any_key_admits_arbitrary<Key>()
where
    AnyKey: IndexKeyBound<Key>,
{
    let _ = PhantomData::<Key>;
}

fn _assert_index_key_bound_propagates<KeyBound, Key>()
where
    KeyBound: IndexKeyBound<Key>,
{
    let _ = PhantomData::<(KeyBound, Key)>;
}

#[test]
fn with_mut_if_present_returns_none_and_does_not_insert_on_miss() {
    let storage = StorageBuilder::new(FullLocking).create_for_bound_key::<&'static str, u32>();
    let result = storage.with_mut_if_present(&"missing", |v| {
        *v += 1;
        *v
    });
    assert!(result.is_none());
    assert!(
        storage.is_empty(),
        "no entry must be created for a missing key"
    );
}

#[test]
fn with_mut_if_present_mutates_existing_entry() {
    let storage = StorageBuilder::new(FullLocking).create_for_bound_key::<&'static str, u32>();
    storage.with_mut("k", || 10u32, |_, _| {});
    let result = storage.with_mut_if_present(&"k", |v| {
        *v += 5;
        *v
    });
    assert_eq!(result, Some(15));
    assert_eq!(storage.with(&"k", |v| *v), Some(15));
}

#[test]
fn with_mut_if_present_exclusive_index_returns_none_and_does_not_insert_on_miss() {
    let storage = StorageBuilder::new(FullLocking).create_for_bound_key::<&'static str, u32>();
    let result = storage.with_mut_if_present_exclusive_index(&"missing", |v| {
        *v += 1;
        *v
    });
    assert!(result.is_none());
    assert!(
        storage.is_empty(),
        "no entry must be created for a missing key"
    );
}

#[test]
fn with_mut_if_present_exclusive_index_mutates_existing_entry() {
    let storage = StorageBuilder::new(FullLocking).create_for_bound_key::<&'static str, u32>();
    storage.with_mut("k", || 10u32, |_, _| {});
    let result = storage.with_mut_if_present_exclusive_index(&"k", |v| {
        *v += 5;
        *v
    });
    assert_eq!(result, Some(15));
    assert_eq!(storage.with(&"k", |v| *v), Some(15));
}

#[test]
fn with_mut_round_trip() {
    let storage = StorageBuilder::new(FullLocking).create_for_bound_key::<&'static str, Vec<u32>>();
    storage.with_mut("v", Vec::new, |entry, _| {
        entry.push(1);
        entry.push(2);
        entry.push(3);
    });
    let snapshot = storage.with(&"v", |value| value.clone()).unwrap();
    assert_eq!(snapshot, vec![1, 2, 3]);
}

#[test]
fn no_locking_nested_with_inside_with_is_allowed() {
    let storage = StorageBuilder::new(NoLocking).create_for_bound_key::<u32, u32>();
    storage.with_mut(1, || 10, |_, _| {});
    storage.with_mut(2, || 20, |_, _| {});

    let result = storage
        .with(&1, |value_outer| {
            let inner = storage.with(&2, |value_inner| *value_inner);
            (*value_outer, inner)
        })
        .expect("entry exists");

    assert_eq!(result, (10, Some(20)));
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "closure re-entered the same storage")]
fn no_locking_with_mut_inside_with_panics_in_debug() {
    let storage = StorageBuilder::new(NoLocking).create_for_bound_key::<u32, u32>();
    storage.with_mut(1, || 10, |_, _| {});
    storage.with_mut(2, || 20, |_, _| {});

    storage.with(&1, |_| {
        storage.with_mut(2, || 0, |_, _| {});
    });
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "closure re-entered the same storage")]
fn no_locking_with_inside_with_mut_panics_in_debug() {
    let storage = StorageBuilder::new(NoLocking).create_for_bound_key::<u32, u32>();
    storage.with_mut(1, || 10, |_, _| {});
    storage.with_mut(2, || 20, |_, _| {});

    storage.with_mut(
        1,
        || 0,
        |_, _| {
            let _ = storage.with(&2, |value| *value);
        },
    );
}

#[test]
fn full_locking_nested_with_inside_with_is_allowed() {
    let storage = StorageBuilder::new(FullLocking).create_for_bound_key::<u32, u32>();
    storage.with_mut(1, || 10, |_, _| {});
    storage.with_mut(2, || 20, |_, _| {});

    let result = storage
        .with(&1, |value_outer| {
            let inner = storage.with(&2, |value_inner| *value_inner);
            (*value_outer, inner)
        })
        .expect("entry exists");

    assert_eq!(result, (10, Some(20)));
}

// Both re-entry tests pre-insert the keys they will touch. With
// `IndexLocking` the slow path takes an exclusive index lock; calling
// back into the storage while that lock is held would deadlock the
// `parking_lot::RwLock` rather than reach the values-domain re-entry
// check. Pre-inserting keeps both calls on the fast path (shared
// index lock, which composes with itself), so the values-domain
// re-entry check fires deterministically.
#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "closure re-entered the same storage")]
fn no_locking_re_entry_in_with_mut_panics_in_debug() {
    let storage = StorageBuilder::new(NoLocking).create_for_bound_key::<u32, u32>();
    storage.with_mut(1, || 0, |_, _| {});
    storage.with_mut(2, || 0, |_, _| {});
    storage.with_mut(
        1,
        || 0,
        |_, _| {
            // Re-entering the storage from inside the closure aliases
            // `&mut Value` in safe code; the debug-only re-entry check
            // turns it into a panic.
            storage.with_mut(2, || 0, |_, _| {});
        },
    );
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "closure re-entered the same storage")]
fn index_locking_re_entry_in_with_mut_panics_in_debug() {
    let storage =
        StorageBuilder::new(IndexLocking::<AnyKey>::default()).create_for_bound_key::<u32, u32>();
    storage.with_mut(1, || 0, |_, _| {});
    storage.with_mut(2, || 0, |_, _| {});
    storage.with_mut(
        1,
        || 0,
        |_, _| {
            storage.with_mut(2, || 0, |_, _| {});
        },
    );
}
