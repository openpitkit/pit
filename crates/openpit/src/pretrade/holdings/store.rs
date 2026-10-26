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

use std::collections::BTreeSet;
#[cfg(test)]
use std::sync::{Arc, Barrier};

use crate::param::{AccountId, Asset};
use crate::storage::{CreateStorageFor, LockingPolicyFactory, Storage, StorageBuilder};

use super::holdings::Holdings;

/// Per-`(AccountId, Asset)` storage of [`Holdings`] entries.
///
/// Thin wrapper around [`crate::storage::Storage`] that pins the key
/// and value types and exposes a minimal closure-based API. The store is
/// parameterised by the engine's [`crate::storage::LockingPolicy`] so
/// callers pick up the appropriate synchronisation flavor automatically.
///
/// Construct it via [`HoldingsStore::new`] using a [`StorageBuilder`]
/// obtained from the engine builder.
pub struct HoldingsStore<LockingPolicy>
where
    LockingPolicy: crate::storage::LockingPolicy,
{
    inner: Storage<(AccountId, Asset), Holdings, LockingPolicy>,
    index: Storage<AccountId, BTreeSet<Asset>, LockingPolicy>,
    #[cfg(test)]
    index_update_hook: parking_lot::Mutex<Option<(Arc<Barrier>, Arc<Barrier>)>>,
}

impl<LockingPolicy> HoldingsStore<LockingPolicy>
where
    LockingPolicy: crate::storage::LockingPolicy,
{
    /// Creates a new store from an engine-issued [`StorageBuilder`].
    ///
    /// The `Factory` type parameter is the
    /// [`LockingPolicyFactory`] supplied by the engine builder; the
    /// store's `Policy` is `Factory::Policy`.
    /// It must implement [`CreateStorageFor`] for both `(AccountId, Asset)`
    /// and `AccountId` keys.
    pub fn new<Factory>(builder: &StorageBuilder<Factory>) -> Self
    where
        Factory: LockingPolicyFactory<Policy = LockingPolicy>,
        Factory: CreateStorageFor<(AccountId, Asset)> + CreateStorageFor<AccountId>,
    {
        Self {
            inner: builder.create_for_bound_key::<(AccountId, Asset), Holdings>(),
            index: builder.create_for_bound_key::<AccountId, BTreeSet<Asset>>(),
            #[cfg(test)]
            index_update_hook: parking_lot::Mutex::new(None),
        }
    }

    /// Reads the holdings under `key`, if any.
    ///
    /// `Holdings` is [`Copy`], so the value is returned by value.
    /// Returns `None` if the key is not present.
    pub fn get(&self, key: &(AccountId, Asset)) -> Option<Holdings> {
        self.inner.with(key, |holdings| *holdings)
    }

    /// Copies the assets of this account's slots, or `None` without slots.
    ///
    /// A slot is listed once its creating call has returned. A slot whose
    /// removal is in flight may still be listed. The index is exact at
    /// quiescence, and no empty account set survives.
    pub(crate) fn account_assets(&self, account_id: &AccountId) -> Option<BTreeSet<Asset>> {
        self.index.with(account_id, Clone::clone)
    }

    /// Read-only scoped access.
    ///
    /// Mirrors [`Storage::with`] semantics.
    pub fn with<Reader, Output>(&self, key: &(AccountId, Asset), reader: Reader) -> Option<Output>
    where
        Reader: FnOnce(&Holdings) -> Output,
    {
        self.inner.with(key, reader)
    }

    /// Read/write scoped access; **does not insert on miss**.
    ///
    /// Mirrors [`Storage::with_mut_if_present`] semantics.
    pub fn with_mut_if_present<Mutator, Output>(
        &self,
        key: &(AccountId, Asset),
        mutator: Mutator,
    ) -> Option<Output>
    where
        Mutator: FnOnce(&mut Holdings) -> Output,
    {
        self.inner.with_mut_if_present(key, mutator)
    }

    /// Read/write scoped access; inserts on demand.
    ///
    /// Mirrors [`Storage::with_mut`] semantics.
    pub fn with_mut<Mutator, Output, Initializer>(
        &self,
        key: (AccountId, Asset),
        default: Initializer,
        mutator: Mutator,
    ) -> Output
    where
        Mutator: FnOnce(&mut Holdings, bool) -> Output,
        Initializer: FnOnce() -> Holdings,
    {
        let mut was_new = false;
        let output = self.inner.with_mut(key.clone(), default, |slot, is_new| {
            was_new = is_new;
            mutator(slot, is_new)
        });
        if was_new {
            self.index_created_slot(&key);
        }
        output
    }

    /// Read/write scoped access; inserts on demand and rolls back on error.
    ///
    /// Mirrors [`Storage::with_mut_or_insert`] semantics.
    pub fn with_mut_or_insert<Mutator, Output, Error, Initializer>(
        &self,
        key: (AccountId, Asset),
        default: Initializer,
        mutator: Mutator,
    ) -> Result<Output, Error>
    where
        Mutator: FnOnce(&mut Holdings, bool) -> Result<Output, Error>,
        Initializer: FnOnce() -> Holdings,
    {
        let mut was_new = false;
        let result = self
            .inner
            .with_mut_or_insert(key.clone(), default, |slot, is_new| {
                was_new = is_new;
                mutator(slot, is_new)
            });
        if was_new && result.is_ok() {
            self.index_created_slot(&key);
        }
        result
    }

    /// Removes the entry under `key`. Returns `true` if it was present.
    pub fn remove(&self, key: &(AccountId, Asset)) -> bool {
        let removed = self.inner.remove(key);
        if removed {
            self.unindex_removed_slot(key);
        }
        removed
    }

    pub(crate) fn remove_if(
        &self,
        key: &(AccountId, Asset),
        predicate: impl FnOnce(&Holdings) -> bool,
    ) -> bool {
        let removed = self.inner.remove_if(key, predicate);
        if removed {
            self.unindex_removed_slot(key);
        }
        removed
    }

    pub fn remove_if_zero(&self, key: &(AccountId, Asset)) -> bool {
        self.remove_if(key, Holdings::is_zero)
    }

    /// Like [`with_mut_or_insert`](Self::with_mut_or_insert) but atomically removes a newly-inserted
    /// slot when the mutation leaves it all-zero. The check and removal happen
    /// under the same exclusive-index lock that performed the insertion, so
    /// the zero-valued slot is never visible to other threads.
    ///
    /// For slots that were already present, behaviour is identical to
    /// [`with_mut_or_insert`](Self::with_mut_or_insert); pruning of existing slots is the caller's
    /// responsibility via [`remove_if_zero`](Self::remove_if_zero).
    pub fn with_mut_or_insert_prune_new_if_zero<Mutator, Output, Error, Initializer>(
        &self,
        key: (AccountId, Asset),
        default: Initializer,
        mutator: Mutator,
    ) -> Result<Output, Error>
    where
        Mutator: FnOnce(&mut Holdings, bool) -> Result<Output, Error>,
        Initializer: FnOnce() -> Holdings,
    {
        let mut was_new = false;
        let mut was_pruned = false;
        let result = self.inner.with_mut_or_insert_prune_new_if(
            key.clone(),
            default,
            |slot| {
                was_pruned = slot.is_zero();
                was_pruned
            },
            |slot, is_new| {
                was_new = is_new;
                mutator(slot, is_new)
            },
        );
        if was_new && result.is_ok() && !was_pruned {
            self.index_created_slot(&key);
        }
        result
    }

    // Call only after releasing holdings locks: nesting is index -> holdings.
    fn index_created_slot(&self, key: &(AccountId, Asset)) {
        #[cfg(test)]
        self.wait_index_update_hook();
        let _: Result<(), std::convert::Infallible> = self.index.with_mut_or_insert_prune_new_if(
            key.0,
            BTreeSet::new,
            BTreeSet::is_empty,
            |assets, _| {
                if self.inner.with(key, |_| ()).is_some() {
                    assets.insert(key.1.clone());
                }
                Ok(())
            },
        );
    }

    fn unindex_removed_slot(&self, key: &(AccountId, Asset)) {
        #[cfg(test)]
        self.wait_index_update_hook();
        let is_empty = self.index.with_mut_if_present(&key.0, |assets| {
            if self.inner.with(key, |_| ()).is_none() {
                assets.remove(&key.1);
            }
            assets.is_empty()
        });
        if is_empty == Some(true) {
            self.index.remove_if(&key.0, BTreeSet::is_empty);
        }
    }

    #[cfg(test)]
    fn set_index_update_hook(&self, entered: Arc<Barrier>, release: Arc<Barrier>) {
        *self.index_update_hook.lock() = Some((entered, release));
    }

    #[cfg(test)]
    fn wait_index_update_hook(&self) {
        let hook = self.index_update_hook.lock().take();
        if let Some((entered, release)) = hook {
            entered.wait();
            release.wait();
        }
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// `true` if no entries.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub(crate) fn keys(&self) -> Vec<(AccountId, Asset)> {
        self.inner.keys()
    }
}

#[cfg(test)]
mod tests {
    use crate::param::{AccountId, Asset, PositionSize};
    use crate::storage::{LockingPolicyFactory, NoLocking};

    use super::{BTreeSet, Holdings, HoldingsStore};

    type TestStore = HoldingsStore<<NoLocking as LockingPolicyFactory>::Policy>;

    fn test_builder() -> crate::SyncedEngineBuilder<(), (), (), crate::LocalSync> {
        crate::Engine::builder().no_sync()
    }

    fn store() -> TestStore {
        let builder = test_builder();
        HoldingsStore::new(builder.storage_builder())
    }

    fn ps(value: &str) -> PositionSize {
        PositionSize::from_str(value).expect("position size literal must be valid")
    }

    fn holdings(available: &str, held: &str) -> Holdings {
        Holdings::new(ps(available), ps(held))
    }

    fn aapl() -> Asset {
        Asset::new("AAPL").expect("asset literal must be valid")
    }

    fn usd() -> Asset {
        Asset::new("USD").expect("asset literal must be valid")
    }

    fn account(id: u64) -> AccountId {
        AccountId::from_u64(id)
    }

    #[test]
    fn new_store_is_empty() {
        let store = store();

        assert_eq!(store.len(), 0);
        assert!(store.is_empty());
    }

    #[test]
    fn get_returns_none_for_missing_key() {
        let store = store();

        assert_eq!(store.get(&(account(1), aapl())), None);
    }

    #[test]
    fn get_returns_inserted_value() {
        let store = store();
        let key = (account(1), aapl());
        let value = holdings("10", "5");

        store.with_mut(
            key.clone(),
            || value,
            |slot, is_new| {
                assert!(is_new);
                assert_eq!(*slot, value);
            },
        );

        assert_eq!(store.get(&key), Some(value));
    }

    #[test]
    fn with_reads_existing_value_without_changing_state() {
        let store = store();
        let key = (account(1), aapl());
        let value = holdings("10", "5");

        store.with_mut(key.clone(), || value, |_, _| {});

        let available = store
            .with(&key, |slot| slot.available())
            .expect("entry must exist");

        assert_eq!(available, ps("10"));
        assert_eq!(store.get(&key), Some(value));
    }

    #[test]
    fn with_mut_inserts_on_demand() {
        let store = store();
        let key = (account(1), aapl());

        let output = store.with_mut(
            key.clone(),
            || holdings("10", "5"),
            |slot, is_new| {
                assert!(is_new);
                assert_eq!(*slot, holdings("10", "5"));
                slot.available()
            },
        );

        assert_eq!(output, ps("10"));
        assert_eq!(store.get(&key), Some(holdings("10", "5")));
        assert_eq!(store.len(), 1);
        assert!(!store.is_empty());
    }

    #[test]
    fn with_mut_reaccesses_existing_entry() {
        let store = store();
        let key = (account(1), aapl());

        store.with_mut(
            key.clone(),
            || holdings("10", "5"),
            |_, is_new| {
                assert!(is_new);
            },
        );

        store.with_mut(
            key.clone(),
            || holdings("99", "99"),
            |slot, is_new| {
                assert!(!is_new);
                assert_eq!(*slot, holdings("10", "5"));
            },
        );

        assert_eq!(store.get(&key), Some(holdings("10", "5")));
    }

    #[test]
    fn with_mut_can_mutate_slot() {
        let store = store();
        let key = (account(1), aapl());

        store.with_mut(key.clone(), Holdings::zero, |slot, is_new| {
            assert!(is_new);
            *slot = holdings("3", "2");
        });

        assert_eq!(store.get(&key), Some(holdings("3", "2")));
    }

    #[test]
    fn with_mut_if_present_returns_none_and_does_not_insert_for_missing_key() {
        let store = store();
        let key = (account(1), aapl());

        let result = store.with_mut_if_present(&key, |slot| slot.held());

        assert!(result.is_none());
        assert!(store.is_empty(), "no phantom entry must be created");
    }

    #[test]
    fn with_mut_if_present_mutates_existing_entry() {
        let store = store();
        let key = (account(1), usd());
        store.with_mut(key.clone(), || holdings("10", "5"), |_, _| {});

        let result = store.with_mut_if_present(&key, |slot| slot.held());

        assert_eq!(result, Some(ps("5")));
    }

    #[test]
    fn remove_existing_entry() {
        let store = store();
        let key = (account(1), aapl());

        store.with_mut(key.clone(), || holdings("10", "5"), |_, _| {});

        assert!(store.remove(&key));
        assert_eq!(store.get(&key), None);
        assert!(store.is_empty());
    }

    #[test]
    fn remove_missing_entry_returns_false() {
        let store = store();

        assert!(!store.remove(&(account(1), usd())));
    }

    #[test]
    fn round_trip_updates_holdings_in_place() {
        let store = store();
        let key = (account(1), usd());

        store.with_mut(
            key.clone(),
            || holdings("10", "5"),
            |slot, _| {
                *slot = slot.try_hold(ps("3")).expect("must hold");
            },
        );

        assert_eq!(store.get(&key), Some(holdings("7", "8")));
    }

    #[test]
    fn prune_new_if_zero_missing_key_zero_result_no_entry_created() {
        let store = store();
        let key = (account(1), aapl());

        let result = store.with_mut_or_insert_prune_new_if_zero(
            key.clone(),
            Holdings::zero,
            |slot, _is_new| Ok::<Holdings, ()>(*slot),
        );

        assert!(result.is_ok());
        assert!(store.is_empty(), "zero-valued new entry must not be stored");
    }

    #[test]
    fn prune_new_if_zero_missing_key_nonzero_result_entry_created() {
        let store = store();
        let key = (account(1), aapl());

        let result = store.with_mut_or_insert_prune_new_if_zero(
            key.clone(),
            Holdings::zero,
            |slot, _is_new| {
                *slot = holdings("5", "0");
                Ok::<Holdings, ()>(*slot)
            },
        );

        assert!(result.is_ok());
        assert_eq!(store.get(&key), Some(holdings("5", "0")));
    }

    #[test]
    fn prune_new_if_zero_existing_key_zero_result_entry_not_removed() {
        let store = store();
        let key = (account(1), aapl());
        store.with_mut(key.clone(), || holdings("5", "0"), |_, _| {});

        let result = store.with_mut_or_insert_prune_new_if_zero(
            key.clone(),
            Holdings::zero,
            |slot, _is_new| {
                *slot = Holdings::zero();
                Ok::<Holdings, ()>(*slot)
            },
        );

        assert!(result.is_ok());
        assert_eq!(
            store.get(&key),
            Some(Holdings::zero()),
            "helper must not remove existing entry; caller is responsible"
        );
    }

    #[test]
    fn prune_new_if_zero_missing_key_err_no_entry_created() {
        let store = store();
        let key = (account(1), aapl());

        let result = store.with_mut_or_insert_prune_new_if_zero(
            key.clone(),
            Holdings::zero,
            |_slot, _is_new| Err::<Holdings, &str>("fail"),
        );

        assert!(result.is_err());
        assert!(
            store.is_empty(),
            "rollback must remove the new entry on Err"
        );
    }

    #[test]
    fn with_mut_lists_created_asset() {
        let store = store();
        let key = (account(1), aapl());

        let output = store.with_mut(key, Holdings::zero, |slot, is_new| {
            assert!(is_new);
            *slot = holdings("5", "0");
            7
        });

        assert_eq!(output, 7);
        assert_eq!(
            store.account_assets(&account(1)),
            Some(BTreeSet::from([aapl()])),
            "created asset must be indexed"
        );
    }

    #[test]
    fn with_mut_or_insert_lists_created_asset_on_success() {
        let store = store();

        let result =
            store.with_mut_or_insert((account(1), aapl()), Holdings::zero, |slot, is_new| {
                assert!(is_new);
                *slot = holdings("5", "0");
                Ok::<_, ()>(7)
            });

        assert_eq!(result, Ok(7));
        assert_eq!(
            store.account_assets(&account(1)),
            Some(BTreeSet::from([aapl()])),
            "successful insertion must index the asset"
        );
    }

    #[test]
    fn prune_new_if_zero_lists_nonzero_asset() {
        let store = store();

        let result = store.with_mut_or_insert_prune_new_if_zero(
            (account(1), aapl()),
            Holdings::zero,
            |slot, is_new| {
                assert!(is_new);
                *slot = holdings("5", "0");
                Ok::<_, ()>(7)
            },
        );

        assert_eq!(result, Ok(7));
        assert_eq!(
            store.account_assets(&account(1)),
            Some(BTreeSet::from([aapl()])),
            "nonzero new slot must be indexed"
        );
    }

    #[test]
    fn with_mut_or_insert_error_leaves_no_account_index() {
        let store = store();
        let key = (account(1), aapl());

        let result = store.with_mut_or_insert(key.clone(), Holdings::zero, |slot, is_new| {
            assert!(is_new);
            *slot = holdings("5", "0");
            Err::<(), _>("fail")
        });

        assert_eq!(result, Err("fail"));
        assert_eq!(store.get(&key), None);
        assert_eq!(
            store.account_assets(&account(1)),
            None,
            "rolled-back new slot must leave no account index"
        );
    }

    #[test]
    fn prune_new_if_zero_leaves_no_account_index() {
        let store = store();
        let key = (account(1), aapl());

        let result = store.with_mut_or_insert_prune_new_if_zero(
            key.clone(),
            || holdings("5", "0"),
            |slot, is_new| {
                assert!(is_new);
                *slot = Holdings::zero();
                Ok::<_, ()>(7)
            },
        );

        assert_eq!(result, Ok(7));
        assert_eq!(store.get(&key), None);
        assert_eq!(
            store.account_assets(&account(1)),
            None,
            "pruned new zero slot must leave no account index"
        );
    }

    #[test]
    fn prune_new_if_zero_error_leaves_no_account_index() {
        let store = store();
        let key = (account(1), aapl());

        let result =
            store.with_mut_or_insert_prune_new_if_zero(key.clone(), Holdings::zero, |slot, _| {
                *slot = holdings("5", "0");
                Err::<(), _>("fail")
            });

        assert_eq!(result, Err("fail"));
        assert_eq!(store.get(&key), None);
        assert_eq!(
            store.account_assets(&account(1)),
            None,
            "failed prune-new insertion must leave no account index"
        );
    }

    #[test]
    fn remove_drops_asset_and_keeps_account_with_another_slot() {
        let store = store();
        let key = (account(1), aapl());
        store.with_mut(key.clone(), Holdings::zero, |_, _| {});
        store.with_mut((account(1), usd()), Holdings::zero, |_, _| {});

        assert!(store.remove(&key));
        assert!(!store.remove(&key));
        assert_eq!(
            store.account_assets(&account(1)),
            Some(BTreeSet::from([usd()])),
            "removed asset must leave the other live asset indexed"
        );
    }

    #[test]
    fn remove_drops_account_index_with_last_slot() {
        let store = store();
        let key = (account(1), aapl());
        store.with_mut(key.clone(), Holdings::zero, |_, _| {});

        assert!(store.remove(&key));
        assert_eq!(
            store.account_assets(&account(1)),
            None,
            "last slot removal must drop the empty account set"
        );
    }

    #[test]
    fn remove_if_false_keeps_asset_indexed() {
        let store = store();
        let key = (account(1), aapl());
        store.with_mut(key.clone(), Holdings::zero, |_, _| {});

        assert!(!store.remove_if(&key, |_| false));
        assert_eq!(store.get(&key), Some(Holdings::zero()));
        assert_eq!(
            store.account_assets(&account(1)),
            Some(BTreeSet::from([aapl()]))
        );
    }

    #[test]
    fn remove_if_true_drops_asset_and_then_last_account_slot() {
        let store = store();
        let key = (account(1), aapl());
        let other = (account(1), usd());
        store.with_mut(key.clone(), Holdings::zero, |_, _| {});
        store.with_mut(other.clone(), Holdings::zero, |_, _| {});

        assert!(store.remove_if(&key, Holdings::is_zero));
        assert_eq!(
            store.account_assets(&account(1)),
            Some(BTreeSet::from([usd()])),
            "conditional removal must unindex only the removed asset"
        );
        assert!(store.remove_if(&other, Holdings::is_zero));
        assert_eq!(store.account_assets(&account(1)), None);
    }

    #[test]
    fn remove_if_zero_drops_only_zero_assets() {
        let store = store();
        let nonzero = (account(1), aapl());
        let zero = (account(1), usd());
        store.with_mut(nonzero.clone(), || holdings("5", "0"), |_, _| {});
        store.with_mut(zero.clone(), Holdings::zero, |_, _| {});

        assert!(!store.remove_if_zero(&nonzero));
        assert_eq!(
            store.account_assets(&account(1)),
            Some(BTreeSet::from([aapl(), usd()]))
        );
        assert!(store.remove_if_zero(&zero));
        assert_eq!(
            store.account_assets(&account(1)),
            Some(BTreeSet::from([aapl()])),
            "zero slot removal must unindex only the removed asset"
        );
        store.with_mut_if_present(&nonzero, |slot| *slot = Holdings::zero());
        assert!(store.remove_if_zero(&nonzero));
        assert_eq!(store.account_assets(&account(1)), None);
    }

    #[test]
    fn accounts_index_only_their_own_assets_in_sorted_order() {
        let store = store();
        store.with_mut((account(1), usd()), Holdings::zero, |_, _| {});
        store.with_mut((account(1), aapl()), Holdings::zero, |_, _| {});
        store.with_mut((account(2), usd()), Holdings::zero, |_, _| {});

        assert_eq!(
            store
                .account_assets(&account(1))
                .map(|assets| assets.into_iter().collect::<Vec<_>>()),
            Some(vec![aapl(), usd()])
        );
        assert_eq!(
            store.account_assets(&account(2)),
            Some(BTreeSet::from([usd()]))
        );
        assert_eq!(store.len(), 3);
        assert_eq!(store.index.len(), 2);
        assert!(store.remove(&(account(2), usd())));
        assert_eq!(store.account_assets(&account(2)), None);
        assert_eq!(
            store.account_assets(&account(1)),
            Some(BTreeSet::from([aapl(), usd()]))
        );
    }

    #[test]
    fn full_sync_removal_preserves_recreated_slot_index() {
        use std::sync::{Arc, Barrier};

        let builder = crate::Engine::builder::<(), (), ()>().full_sync();
        let store = HoldingsStore::new(builder.storage_builder());
        let key = (account(1), aapl());
        store.with_mut(key.clone(), Holdings::zero, |_, _| {});
        let entered = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));
        store.set_index_update_hook(entered.clone(), release.clone());

        let removed = std::thread::scope(|scope| {
            let removal = scope.spawn(|| store.remove(&key));
            entered.wait();
            store.with_mut(key.clone(), Holdings::zero, |slot, _| {
                *slot = holdings("7", "0");
            });
            release.wait();
            removal.join().unwrap()
        });

        assert!(removed);
        assert_eq!(
            store.account_assets(&key.0),
            Some(BTreeSet::from([key.1.clone()])),
            "re-created slot must remain indexed"
        );
        assert_eq!(store.get(&key), Some(holdings("7", "0")));
    }

    #[test]
    fn full_sync_creation_does_not_index_removed_slot() {
        use std::sync::{Arc, Barrier};

        let builder = crate::Engine::builder::<(), (), ()>().full_sync();
        let store = HoldingsStore::new(builder.storage_builder());
        let key = (account(1), aapl());
        let entered = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));
        store.set_index_update_hook(entered.clone(), release.clone());

        let removed = std::thread::scope(|scope| {
            let creation = scope.spawn(|| {
                store.with_mut(key.clone(), Holdings::zero, |_, _| {});
            });
            entered.wait();
            let removed = store.remove(&key);
            release.wait();
            creation.join().unwrap();
            removed
        });

        assert!(removed);
        assert_eq!(
            store.account_assets(&key.0),
            None,
            "removed slot must leave no account index"
        );
        assert_eq!(store.get(&key), None);
    }

    #[test]
    fn full_sync_index_matches_live_slots_after_concurrent_creation_and_removal() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Barrier;

        let builder = crate::Engine::builder::<(), (), ()>().full_sync();
        let store = HoldingsStore::new(builder.storage_builder());
        store.with_mut((account(2), aapl()), Holdings::zero, |_, _| {});
        let phase = Barrier::new(4);
        let exact_at_quiescence = AtomicBool::new(true);

        std::thread::scope(|scope| {
            for worker in 0..4 {
                let store = &store;
                let phase = &phase;
                let exact_at_quiescence = &exact_at_quiescence;
                scope.spawn(move || {
                    for round in 0..128 {
                        for asset in [aapl(), usd()] {
                            let key = (account(1), asset);
                            match (worker + round) % 3 {
                                0 => store.with_mut(key.clone(), Holdings::zero, |slot, _| {
                                    *slot = holdings("5", "0")
                                }),
                                1 => store
                                    .with_mut_or_insert(key.clone(), Holdings::zero, |slot, _| {
                                        *slot = holdings("5", "0");
                                        Ok::<_, ()>(())
                                    })
                                    .unwrap(),
                                _ => store
                                    .with_mut_or_insert_prune_new_if_zero(
                                        key.clone(),
                                        Holdings::zero,
                                        |slot, _| {
                                            *slot = holdings("5", "0");
                                            Ok::<_, ()>(())
                                        },
                                    )
                                    .unwrap(),
                            }
                            if (worker + round) % 4 != 0 {
                                match (worker + round) % 3 {
                                    0 => {
                                        store.remove(&key);
                                    }
                                    1 => {
                                        store.remove_if(&key, |_| true);
                                    }
                                    _ => {
                                        store.with_mut_if_present(&key, |slot| {
                                            *slot = Holdings::zero();
                                        });
                                        store.remove_if_zero(&key);
                                    }
                                }
                            }
                            store.with_mut(key, Holdings::zero, |_, _| {});
                        }
                        phase.wait();
                        if worker == 0 {
                            let expected: BTreeSet<_> = store
                                .keys()
                                .into_iter()
                                .filter_map(|(owner, asset)| (owner == account(1)).then_some(asset))
                                .collect();
                            let expected = if expected.is_empty() {
                                None
                            } else {
                                Some(expected)
                            };
                            if store.account_assets(&account(1)) != expected {
                                exact_at_quiescence.store(false, Ordering::Relaxed);
                            }
                        }
                        phase.wait();
                    }
                });
            }
        });

        assert!(
            exact_at_quiescence.load(Ordering::Relaxed),
            "concurrent index must exactly match live slots at each quiescent point"
        );
        let keys = store.keys();
        for owner in [account(1), account(2)] {
            let expected: BTreeSet<_> = keys
                .iter()
                .filter_map(|(account, asset)| (*account == owner).then_some(asset.clone()))
                .collect();
            if expected.is_empty() {
                assert_eq!(store.account_assets(&owner), None);
            } else {
                assert_eq!(store.account_assets(&owner), Some(expected));
            }
        }
        for owner in store.index.keys() {
            assert!(
                store
                    .index
                    .with(&owner, |assets| !assets.is_empty())
                    .unwrap(),
                "concurrent removal must leave no empty account set"
            );
        }
    }
}
