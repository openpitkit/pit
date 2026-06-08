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

//! Key-bound markers and creation permissions for [`Storage`](super::Storage).
//!
//! The storage layer is intentionally domain-neutral: it knows nothing about
//! `AccountId` or any other application concept. To let layers above attach
//! compile-time bounds on the storage key without leaking domain types into
//! `storage`, two opaque markers are exposed here:
//!
//! * [`IndexKeyBound`] is the contract a key must satisfy under a given
//!   [`IndexLocking`](super::IndexLocking) parameterisation. Concrete bounds
//!   are introduced by users of the storage (for example, the engine layer
//!   adds `AccountKeyConstraint` for account-sharded storages).
//! * [`AnyKey`] is the default bound: every key satisfies it.
//! * [`CreateStorageFor`] is a permission marker on
//!   [`LockingPolicyFactory`](super::LockingPolicyFactory) that controls
//!   which key types a factory can produce storages for through
//!   [`StorageBuilder::create`](super::StorageBuilder::create).

use super::policies::{FullLocking, IndexLocking, NoLocking};
use super::policy::LockingPolicyFactory;

/// Type-level marker that constrains which keys an [`IndexLocking`] storage
/// may be created with.
///
/// The storage layer treats this trait as opaque: it never inspects the
/// implementations and imposes no bound of its own. Layers above (engine,
/// custom embeddings) define concrete markers and decide which keys those
/// markers admit. See `AccountKeyConstraint` in the engine layer for the
/// canonical example.
///
/// The `'static` super-bound mirrors the lifetime requirements of the
/// keys actually stored in the [`Storage`](super::Storage) and lets the
/// marker types live in the
/// [`StorageLockingPolicyFactory`](crate::SyncMode::StorageLockingPolicyFactory)
/// associated type without dangling lifetime parameters.
///
/// [`IndexLocking`]: super::IndexLocking
pub trait IndexKeyBound<Key>: 'static {}

/// Default key bound: imposes no requirement and is satisfied by every key.
///
/// This is the default type parameter of [`IndexLocking`](super::IndexLocking),
/// so storages built without an explicit constraint accept any
/// [`Hash`](std::hash::Hash) + [`Eq`] key.
#[derive(Debug, Default, Clone, Copy)]
pub struct AnyKey;

impl<Key> IndexKeyBound<Key> for AnyKey {}

/// Permission marker: locking-policy factories that implement this trait can
/// produce [`Storage`](super::Storage) instances with the given key type via
/// [`StorageBuilder::create`](super::StorageBuilder::create) and
/// [`StorageBuilder::create_with_capacity`](super::StorageBuilder::create_with_capacity).
///
/// The trait is the compile-time gate that surfaces a key-bound mismatch as
/// an error on `StorageBuilder::create`, with a domain-specific diagnostic
/// (see `AccountKeyConstraint` in the engine layer) when the bound is
/// stricter than `AnyKey`.
///
/// All built-in factories implement it:
///
/// * [`NoLocking`](super::NoLocking) and [`FullLocking`](super::FullLocking)
///   accept every key.
/// * [`IndexLocking<KeyBound>`](super::IndexLocking) accepts a key when
///   `KeyBound: IndexKeyBound<Key>`. With the default `AnyKey` this is
///   every key; with a stricter marker it is only the keys that satisfy
///   that marker.
///
/// Custom factories that wish to be usable through `StorageBuilder::create`
/// must add a one-line blanket impl:
///
/// ```ignore
/// impl<Key> openpit::storage::CreateStorageFor<Key> for MyCustomFactory {}
/// ```
pub trait CreateStorageFor<Key>: LockingPolicyFactory {}

impl<Key> CreateStorageFor<Key> for NoLocking {}

impl<Key> CreateStorageFor<Key> for FullLocking {}

impl<Key, KeyBound> CreateStorageFor<Key> for IndexLocking<KeyBound> where
    KeyBound: IndexKeyBound<Key>
{
}
