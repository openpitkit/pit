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
//!
//! This module exposes a small, focused storage abstraction around a
//! `HashMap<Key, Value>` whose synchronization scheme is chosen at the
//! type level via a [`LockingPolicyFactory`]. The intended use is the
//! application-bootstrap-time construction of in-memory tables that
//! later serve hot-path lookups.
//!
//! # Concepts
//!
//! * [`StorageBuilder<Factory>`] — the builder, parameterised by a
//!   [`LockingPolicyFactory`]. It is owned by the engine builder and
//!   exposed through `storage_builder()`; each storage gets its own
//!   freshly created [`LockingPolicy`].
//! * [`Storage<Key, Value, LockingPolicy>`] — the actual key-value
//!   store. Exposes scoped access methods that run a caller closure
//!   while the appropriate locks are held; references handed to the
//!   closure are confined to the closure's call. Cannot be cloned.
//!
//! # Built-in policies
//!
//! Three policies cover the common synchronization regimes; the
//! mechanism is deliberately open so that custom regimes can be added
//! by implementing [`LockingPolicy`] and [`LockingPolicyFactory`].
//! Concrete policy types are private to this crate; callers name only
//! the factory types below.
//!
//! * [`NoLocking`] — no synchronization. The resulting storage is
//!   `!Send` and `!Sync`. The closure-based access methods make safe
//!   misuse via overlapping references impossible at the type level;
//!   accidental closure re-entry on the same storage is detected by a
//!   debug-only check.
//! * [`IndexLocking`] — one reader-writer lock guards key insertions
//!   and removals; per-key value access is unsynchronized. The closure
//!   based access methods, plus the debug-only re-entry check, keep
//!   single-thread misuse impossible to express in safe code.
//! * [`FullLocking`] — one reader-writer lock guards key insertions
//!   and removals, a second reader-writer lock guards every value
//!   access. A writer to one value blocks readers of every other
//!   value; the trade-off is a small fixed amount of state per storage
//!   instead of a per-key allocation.
//!
//! # Example
//!
//! ```
//! use openpit::Engine;
//!
//! let builder = Engine::builder::<(), (), ()>().full_sync();
//! let storage = builder.storage_builder().create::<&'static str, u64>();
//!
//! // Insert.
//! storage.with_mut("ticks", || 0, |counter, is_new| {
//!     assert!(is_new);
//!     *counter += 1;
//! });
//!
//! // Read.
//! assert_eq!(storage.with(&"ticks", |v| *v), Some(1));
//!
//! // Increment under a write closure.
//! storage.with_mut("ticks", || 0, |counter, is_new| {
//!     assert!(!is_new);
//!     *counter += 41;
//! });
//! assert_eq!(storage.with(&"ticks", |v| *v), Some(42));
//!
//! // Remove.
//! assert!(storage.remove(&"ticks"));
//! assert_eq!(storage.with(&"ticks", |v| *v), None);
//! ```
//!
//! # Compile-time vs runtime configuration
//!
//! Synchronization is selected statically through the type parameter
//! `LockingPolicy`. This lets the compiler inline and specialize every
//! operation for a known policy, which is important for the no-op
//! `NoLocking` regime. Runtime selection (e.g. exposing the choice
//! through a future FFI surface) can be layered on top by providing an
//! enum that implements [`LockingPolicy`] and dispatches its methods
//! through a `match`; that layer is not part of this module.

mod builder;
mod index_flag;
mod key_bound;
mod policies;
mod policy;
#[allow(clippy::module_inception)]
mod storage;

#[cfg(test)]
mod tests;

pub use builder::StorageBuilder;
pub use index_flag::IndexFlag;
pub use key_bound::{AnyKey, CreateStorageFor, IndexKeyBound};
pub use policies::{FullLocking, IndexLocking, NoLocking};
pub use policy::{FullySynchronized, LockingPolicy, LockingPolicyFactory};
pub use storage::Storage;
