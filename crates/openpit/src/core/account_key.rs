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

//! Compile-time bound that ties account-sharded storages to keys identifying
//! an account.
//!
//! [`AccountKey`] is the engine-level marker that says "this storage key
//! identifies an account, and so the storage may be sharded by account".
//! [`AccountKeyConstraint`] adapts that marker to the storage layer's
//! [`IndexKeyBound`](crate::storage::IndexKeyBound) interface so that the
//! engine builder's
//! [`account_sync`](crate::EngineBuilder::account_sync) entry
//! point can pin its storages to keys that satisfy [`AccountKey`] at
//! compile time.

use crate::param::AccountId;
use crate::storage::IndexKeyBound;

/// Storage-key marker for keys that identify an account.
///
/// Implemented automatically for [`AccountId`] and tuples whose first
/// element is [`AccountId`] up to a reasonable arity. Custom storage keys
/// must implement this trait explicitly to be usable under
/// [`account_sync`](crate::EngineBuilder::account_sync).
///
/// The trait's only method, [`AccountKey::account_id`], reports the
/// account the key belongs to. Implementations are expected to be
/// constant-time.
#[diagnostic::on_unimplemented(
    message = "engine is in account-sync mode (`account_sync`), but the storage key `{Self}` does not identify an account",
    label = "this key cannot be sharded by account",
    note = "include `AccountId` in the storage key (e.g. `(AccountId, ...)`), implement `AccountKey` for your custom key, or switch to `full_sync` / `no_sync`"
)]
pub trait AccountKey {
    /// Returns the account this key belongs to.
    fn account_id(&self) -> AccountId;
}

impl AccountKey for AccountId {
    #[inline]
    fn account_id(&self) -> AccountId {
        *self
    }
}

impl<T> AccountKey for (AccountId, T) {
    #[inline]
    fn account_id(&self) -> AccountId {
        self.0
    }
}

impl<T1, T2> AccountKey for (AccountId, T1, T2) {
    #[inline]
    fn account_id(&self) -> AccountId {
        self.0
    }
}

impl<T1, T2, T3> AccountKey for (AccountId, T1, T2, T3) {
    #[inline]
    fn account_id(&self) -> AccountId {
        self.0
    }
}

/// Engine-level adapter that maps [`AccountKey`] onto the storage layer's
/// neutral [`IndexKeyBound`](crate::storage::IndexKeyBound).
///
/// Used as the `KeyBound` parameter of
/// [`IndexLocking`](crate::storage::IndexLocking) under
/// [`AccountSync`](crate::AccountSync). Storages built through
/// the [`account_sync`](crate::EngineBuilder::account_sync) flow
/// inherit this bound, which forces every key type used by registered
/// trading policies to satisfy [`AccountKey`] at compile time.
///
/// Keys identifying an account compile:
///
/// ```rust
/// use openpit::param::{AccountId, Asset};
/// use openpit::Engine;
///
/// let builder = Engine::builder::<(), (), ()>().account_sync();
/// let _ = builder.storage_builder().create::<AccountId, u64>();
/// let _ = builder.storage_builder().create::<(AccountId, Asset), u64>();
/// ```
///
/// Keys that do not identify an account are rejected at compile time
/// (the diagnostic message is emitted by
/// [`#[diagnostic::on_unimplemented]`](AccountKey) on [`AccountKey`]):
///
/// ```compile_fail
/// use openpit::Engine;
///
/// // `u32` does not implement `AccountKey`, so account-sync mode
/// // refuses to create a storage keyed by it.
/// let builder = Engine::builder::<(), (), ()>().account_sync();
/// let _ = builder.storage_builder().create::<u32, u64>();
/// ```
#[derive(Debug, Default, Clone, Copy)]
pub struct AccountKeyConstraint;

impl<Key> IndexKeyBound<Key> for AccountKeyConstraint where Key: AccountKey + 'static {}
