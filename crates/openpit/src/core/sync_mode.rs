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

//! Engine-level synchronization configuration.
//!
//! A [`SyncMode`] selects every synchronization aspect of an
//! [`Engine`](crate::Engine): the strong and weak handle pointers, the
//! required trait-object shape for registered policies, and the storage
//! locking factory used by both user policies and engine-internal storage.

use std::ops::Deref;

use crate::param::AccountId;
use crate::pretrade::PreTradePolicy;
use crate::storage::{self, LockingPolicyFactory};

/// Synchronization mode for an [`Engine`](crate::Engine).
///
/// Implement this trait to configure all engine synchronization choices at
/// once. Built-in modes cover the standard regimes:
///
/// - [`LocalSync`]: `!Send + !Sync` engine handle, no storage locking, and
///   `'static` policies.
/// - [`FullSync`]: `Send + Sync` engine handle, fully locked storage, and
///   `Send + Sync + 'static` policies.
/// - [`AccountSync`]: `Send + !Sync` engine handle, account-keyed storage,
///   and `Send + 'static` policies.
///
/// # Custom modes
///
/// Custom modes are open to downstream crates. If
/// [`PreTradePolicyObject`](Self::PreTradePolicyObject) resolves to one of
/// the three built-in policy-object shapes, the engine builder's
/// [`pre_trade`](crate::SyncedEngineBuilder::pre_trade) method works through
/// the blanket [`IntoPolicyObject`](crate::IntoPolicyObject) implementations.
/// A custom object shape must provide its own `IntoPolicyObject` conversion.
pub trait SyncMode: 'static {
    /// Strong reference type used by the engine handle.
    type Strong<T: 'static>: Clone + Deref<Target = T>;
    /// Weak reference type captured by deferred request handles.
    type Weak<T: 'static>: 'static;

    /// Factory used for every storage created by this engine.
    type StorageLockingPolicyFactory: LockingPolicyFactory
        + storage::CreateStorageFor<AccountId>
        + 'static;

    /// Trait-object shape required for registered pre-trade policies.
    type PreTradePolicyObject<
        Order: 'static,
        ExecutionReport: 'static,
        AccountAdjustment: 'static,
    >: PreTradePolicy<Order, ExecutionReport, AccountAdjustment, Self>
        + ?Sized
        + 'static;

    /// Wraps `inner` in a new strong engine reference.
    fn new_strong<T: 'static>(inner: T) -> Self::Strong<T>;
    /// Creates a weak reference from the given strong reference.
    fn downgrade<T: 'static>(s: &Self::Strong<T>) -> Self::Weak<T>;
    /// Attempts to upgrade a weak reference to a strong reference.
    fn upgrade<T: 'static>(w: &Self::Weak<T>) -> Option<Self::Strong<T>>;

    /// Creates the storage-locking factory used by this mode.
    fn storage_locking_policy_factory(&self) -> Self::StorageLockingPolicyFactory;
}

/// Single-thread synchronization mode.
///
/// The engine handle is `!Send + !Sync`; keep it on the OS thread that
/// created it. Registered policies require only `'static`, so non-Send state
/// is supported.
///
/// Storage tables use [`NoLocking`](crate::storage::NoLocking), with no
/// runtime synchronization overhead.
///
/// # Examples
///
/// ```rust
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use openpit::{Engine, LocalEngine};
/// use openpit::pretrade::policies::OrderValidationPolicy;
/// use openpit::OrderOperation;
///
/// let engine: LocalEngine<OrderOperation> = Engine::builder()
///     .no_sync()
///     .pre_trade(OrderValidationPolicy::new())
///     .build()?;
/// # Ok(())
/// # }
/// ```
///
/// `LocalEngine<T>` is `!Send + !Sync`; attempting to send it across threads
/// does not compile:
///
/// ```compile_fail
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use openpit::{Engine, OrderOperation};
/// use openpit::pretrade::policies::OrderValidationPolicy;
///
/// fn require_send<T: Send>(_: T) {}
///
/// let engine = Engine::builder()
///     .sync(openpit::LocalSync)
///     .pre_trade(OrderValidationPolicy::new())
///     .build()?;
/// require_send(engine); // compile error: LocalSync engines are !Send
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Default, Clone, Copy)]
pub struct LocalSync;

impl SyncMode for LocalSync {
    type Strong<T: 'static> = std::rc::Rc<T>;
    type Weak<T: 'static> = std::rc::Weak<T>;
    type StorageLockingPolicyFactory = crate::storage::NoLocking;
    type PreTradePolicyObject<
        Order: 'static,
        ExecutionReport: 'static,
        AccountAdjustment: 'static,
    > = dyn PreTradePolicy<Order, ExecutionReport, AccountAdjustment, LocalSync>;

    fn new_strong<T: 'static>(inner: T) -> Self::Strong<T> {
        std::rc::Rc::new(inner)
    }

    fn downgrade<T: 'static>(s: &Self::Strong<T>) -> Self::Weak<T> {
        std::rc::Rc::downgrade(s)
    }

    fn upgrade<T: 'static>(w: &Self::Weak<T>) -> Option<Self::Strong<T>> {
        w.upgrade()
    }

    fn storage_locking_policy_factory(&self) -> Self::StorageLockingPolicyFactory {
        Default::default()
    }
}

/// Full thread-safety synchronization mode.
///
/// The engine handle inherits `Send + Sync` from its inner state and can be
/// shared across threads when registered policies are `Send + Sync`. Storage
/// tables use [`FullLocking`](crate::storage::FullLocking).
///
/// # Examples
///
/// ```rust
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use openpit::{Engine, FullSyncEngine};
/// use openpit::pretrade::policies::OrderValidationPolicy;
/// use openpit::OrderOperation;
/// use std::sync::Arc;
///
/// let engine: Arc<FullSyncEngine<OrderOperation>> = Arc::new(
///     Engine::builder()
///         .full_sync()
///         .pre_trade(OrderValidationPolicy::new())
///         .build()?,
/// );
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Default, Clone, Copy)]
pub struct FullSync;

impl SyncMode for FullSync {
    type Strong<T: 'static> = std::sync::Arc<T>;
    type Weak<T: 'static> = std::sync::Weak<T>;
    type StorageLockingPolicyFactory = crate::storage::FullLocking;
    type PreTradePolicyObject<
        Order: 'static,
        ExecutionReport: 'static,
        AccountAdjustment: 'static,
    > = dyn PreTradePolicy<Order, ExecutionReport, AccountAdjustment, FullSync> + Send + Sync;

    fn new_strong<T: 'static>(inner: T) -> Self::Strong<T> {
        std::sync::Arc::new(inner)
    }

    fn downgrade<T: 'static>(s: &Self::Strong<T>) -> Self::Weak<T> {
        std::sync::Arc::downgrade(s)
    }

    fn upgrade<T: 'static>(w: &Self::Weak<T>) -> Option<Self::Strong<T>> {
        w.upgrade()
    }

    fn storage_locking_policy_factory(&self) -> Self::StorageLockingPolicyFactory {
        Default::default()
    }
}

/// Engine handle handed out for account-sharded sequential cross-thread
/// invocation.
///
/// `Send`: ownership of the engine handle may move between OS threads
/// sequentially, with the caller serialising per-handle invocation (one
/// active public-method call per handle at a time). Concurrent invocation
/// on the same handle is forbidden by contract and not supported at the
/// type level (the handle is `!Sync`).
///
/// # Pure Rust vs binding-layer contract
///
/// This pure-Rust handle does **not** allow concurrent invocation from
/// multiple threads even when calls are partitioned by account. The `!Sync`
/// bound enforces this at compile time. Rust SDK clients who need
/// per-account concurrency must shard ownership: place each
/// `Arc<Mutex<AccountSyncEngine<...>>>` (or equivalent) behind a per-account
/// lock, or
/// move handles between threads sequentially with explicit serialisation.
pub struct AccountSyncHandle<T: ?Sized>(
    std::sync::Arc<T>,
    std::marker::PhantomData<std::cell::Cell<()>>,
);

impl<T: ?Sized> Clone for AccountSyncHandle<T> {
    fn clone(&self) -> Self {
        Self(std::sync::Arc::clone(&self.0), std::marker::PhantomData)
    }
}

impl<T: ?Sized> Deref for AccountSyncHandle<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

// SAFETY:
// Sending an `Arc<T>` between threads requires `T: Send + Sync` for
// `std::sync::Arc<T>: Send`. We claim `Send` with only `T: Send`
// because the SDK's account-sync threading contract requires the
// caller to serialise per-handle invocation: ownership of the handle
// may move between OS threads sequentially, but only one thread
// observes `&EngineInner` at any moment. The `Arc` refcount is
// thread-safe; the inner state is observed by at most one thread at
// a time per the contract.
//
// The marker field deliberately keeps this handle `!Sync`: concurrent
// shared access is not supported under account-sharded synchronization,
// and the type system reflects that. Callers attempting
// `Arc<AccountSyncEngine<...>>` and concurrent invocation
// receive a compile error.
unsafe impl<T: ?Sized + Send> Send for AccountSyncHandle<T> {}

/// Weak counterpart of [`AccountSyncHandle`].
pub struct AccountSyncHandleWeak<T: ?Sized>(
    std::sync::Weak<T>,
    std::marker::PhantomData<std::cell::Cell<()>>,
);

impl<T: ?Sized> Clone for AccountSyncHandleWeak<T> {
    fn clone(&self) -> Self {
        Self(std::sync::Weak::clone(&self.0), std::marker::PhantomData)
    }
}

// SAFETY: same sequential ownership-transfer contract as
// `AccountSyncHandle` above. The weak handle owns no engine state;
// it only carries the thread-safe weak reference count.
unsafe impl<T: ?Sized + Send> Send for AccountSyncHandleWeak<T> {}

/// Account-keyed synchronization mode.
///
/// The engine handle is `Send + !Sync`: ownership may move between OS
/// threads sequentially, but concurrent invocation on the same handle is not
/// supported. Registered policies must be `Send + 'static`.
///
/// Storage tables use
/// [`IndexLocking<AccountKeyConstraint>`](crate::storage::IndexLocking),
/// which requires every storage key to identify an account.
#[derive(Debug, Default, Clone, Copy)]
pub struct AccountSync;

impl SyncMode for AccountSync {
    type Strong<T: 'static> = AccountSyncHandle<T>;
    type Weak<T: 'static> = AccountSyncHandleWeak<T>;
    type StorageLockingPolicyFactory = crate::storage::IndexLocking<crate::AccountKeyConstraint>;
    type PreTradePolicyObject<
        Order: 'static,
        ExecutionReport: 'static,
        AccountAdjustment: 'static,
    > = dyn PreTradePolicy<Order, ExecutionReport, AccountAdjustment, AccountSync> + Send;

    fn new_strong<T: 'static>(inner: T) -> Self::Strong<T> {
        AccountSyncHandle(std::sync::Arc::new(inner), std::marker::PhantomData)
    }

    fn downgrade<T: 'static>(s: &Self::Strong<T>) -> Self::Weak<T> {
        AccountSyncHandleWeak(std::sync::Arc::downgrade(&s.0), std::marker::PhantomData)
    }

    fn upgrade<T: 'static>(w: &Self::Weak<T>) -> Option<Self::Strong<T>> {
        w.0.upgrade()
            .map(|inner| AccountSyncHandle(inner, std::marker::PhantomData))
    }

    fn storage_locking_policy_factory(&self) -> Self::StorageLockingPolicyFactory {
        Default::default()
    }
}
