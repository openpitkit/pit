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
    pub fn new<Factory>(builder: &StorageBuilder<Factory>) -> Self
    where
        Factory: LockingPolicyFactory<Policy = LockingPolicy>,
        Factory: CreateStorageFor<(AccountId, Asset)>,
    {
        Self {
            inner: builder.create::<(AccountId, Asset), Holdings>(),
        }
    }

    /// Reads the holdings under `key`, if any.
    ///
    /// `Holdings` is [`Copy`], so the value is returned by value.
    /// Returns `None` if the key is not present.
    pub fn get(&self, key: &(AccountId, Asset)) -> Option<Holdings> {
        self.inner.with(key, |holdings| *holdings)
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
        self.inner.with_mut(key, default, mutator)
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
        self.inner.with_mut_or_insert(key, default, mutator)
    }

    /// Removes the entry under `key`. Returns `true` if it was present.
    pub fn remove(&self, key: &(AccountId, Asset)) -> bool {
        self.inner.remove(key)
    }

    pub fn remove_if_zero(&self, key: &(AccountId, Asset)) -> bool {
        self.inner.remove_if(key, Holdings::is_zero)
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
        self.inner
            .with_mut_or_insert_prune_new_if(key, default, Holdings::is_zero, mutator)
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// `true` if no entries.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use crate::param::{AccountId, Asset, PositionSize};
    use crate::storage::{LockingPolicyFactory, NoLocking};

    use super::{Holdings, HoldingsStore};

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
}
