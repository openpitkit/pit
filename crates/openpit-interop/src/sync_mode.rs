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

use std::sync::Arc;

use openpit::storage::{FullLocking, IndexLocking, LockingPolicy, LockingPolicyFactory};
use openpit::{AccountKey, AccountKeyConstraint};

// ─── Engine handle types ──────────────────────────────────────────────────────

/// Strong handle to an engine constructed through the binding layer.
///
/// Carrying the engine across binding boundaries (Python, Go, C) requires
/// a smart-pointer pair that the type system tracks as thread-shareable
/// under the SDK threading contract. This handle plays the strong half
/// of that pair; [`EngineHandleWeak`] plays the weak half.
///
/// Per the SDK threading contract, public methods on the same handle
/// must not be invoked concurrently when the underlying sync mode does
/// not promise full synchronization. Sequential invocation across OS
/// threads is supported for every sync mode. The full-synchronization
/// modes (e.g. `full_sync()` in any binding) make the underlying
/// engine fully thread-safe and admit concurrent invocation as well.
pub struct EngineHandle<T: ?Sized>(pub(crate) Arc<T>);

impl<T: ?Sized> Clone for EngineHandle<T> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl<T: ?Sized> std::ops::Deref for EngineHandle<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

// SAFETY:
// `EngineHandle<T>` wraps `Arc<T>`. The standard library's `Arc<T>: Send +
// Sync` requires `T: Send + Sync`. We claim `Send + Sync` with only `T: Send`
// (no `Sync`) because the safety story is the language-binding threading
// contract:
//
// - Under `SyncMode::Full`, the inner storage is fully synchronized at
//   runtime through `FullLocking`'s rwlocks. Multiple binding threads
//   observing `&EngineInner` concurrently see data-race-free state at
//   runtime even though the Rust type system would conservatively reject
//   `T: !Sync` shared across threads.
// - Under `SyncMode::Local` and `SyncMode::Account`, the binding caller
//   serialises per-handle invocation per the SDK threading contract
//   documented in Threading-Contract.md and the `SyncMode` enum's variant
//   docs. Only one binding thread observes `&EngineInner` at a time; no
//   Sync is needed in practice.
//
// Each engine method takes `&self`; FFI dispatch takes a shared borrow of
// the handle, so multiple sequential threads observe the same inner state
// without aliasing `&mut`. The `Arc` refcount is thread-safe.
//
// Violating the per-mode contract from the binding caller is undefined
// behavior at the contract level.
unsafe impl<T: ?Sized + Send> Send for EngineHandle<T> {}
unsafe impl<T: ?Sized + Send> Sync for EngineHandle<T> {}

/// Weak counterpart of [`EngineHandle`].
///
/// Used by deferred request and reservation handles to detect engine
/// destruction without keeping the engine alive.
pub struct EngineHandleWeak<T: ?Sized>(pub(crate) std::sync::Weak<T>);

impl<T: ?Sized> Clone for EngineHandleWeak<T> {
    fn clone(&self) -> Self {
        Self(std::sync::Weak::clone(&self.0))
    }
}

// SAFETY: same reasoning as `EngineHandle` above: the weak half only
// holds a non-owning reference; its Send/Sync follow from the strong
// side's contract.
unsafe impl<T: ?Sized + Send> Send for EngineHandleWeak<T> {}
unsafe impl<T: ?Sized + Send> Sync for EngineHandleWeak<T> {}

// ─── EngineLocking ───────────────────────────────────────────────────────────

/// `openpit::SyncMode` for engines constructed through the binding layer.
///
/// The resulting engine handle is `Send + Sync` regardless of the
/// runtime-selected `SyncMode`. Pure Rust SDK clients should use
/// `openpit::LocalSync`, `openpit::AccountSync`, or `openpit::FullSync`
/// directly via `EngineBuilder::no_sync`, `EngineBuilder::full_sync`, or
/// `EngineBuilder::account_sync`; this type is for the binding layer only.
pub struct EngineLocking {
    pub(crate) mode: SyncMode,
}

impl EngineLocking {
    /// Creates a binding-layer synchronization mode.
    pub fn new(mode: SyncMode) -> Self {
        Self { mode }
    }
}

impl openpit::SyncMode for EngineLocking {
    type Strong<T: 'static> = EngineHandle<T>;
    type Weak<T: 'static> = EngineHandleWeak<T>;
    type StorageLockingPolicyFactory = StorageLockingPolicyFactory;
    type PreTradePolicyObject<
        Order: 'static,
        ExecutionReport: 'static,
        AccountAdjustment: 'static,
    > = dyn openpit::pretrade::PreTradePolicy<Order, ExecutionReport, AccountAdjustment> + Send;

    fn new_strong<T: 'static>(inner: T) -> Self::Strong<T> {
        EngineHandle(Arc::new(inner))
    }

    fn downgrade<T: 'static>(s: &Self::Strong<T>) -> Self::Weak<T> {
        EngineHandleWeak(Arc::downgrade(&s.0))
    }

    fn upgrade<T: 'static>(w: &Self::Weak<T>) -> Option<Self::Strong<T>> {
        w.0.upgrade().map(EngineHandle)
    }

    fn storage_locking_policy_factory(&self) -> Self::StorageLockingPolicyFactory {
        StorageLockingPolicyFactory { mode: self.mode }
    }
}

// ─────────────────────────────────────────────────────────────────────────────

/// Runtime selector for the engine's storage synchronization policy.
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SyncMode {
    /// Concurrent invocation of public methods on the same handle is safe.
    /// Sequential cross-thread access is also safe. Use this when the engine
    /// is shared across threads.
    Full = 0,
    /// The handle stays on the OS thread that created it. Use this for
    /// single-threaded embeddings where synchronization overhead must be zero.
    Local = 1,
    /// Sequential cross-thread access on the same handle is safe; the caller
    /// pins each account to a single processing chain (one queue or one
    /// worker at a time). Concurrent invocation on the same handle is not
    /// supported in this mode.
    Account = 2,
}

// ─── Guards ───────────────────────────────────────────────────────────────────

type FullPolicyAlias = <FullLocking as LockingPolicyFactory>::Policy;
type FullIndexShared<'a> = <FullPolicyAlias as LockingPolicy>::IndexSharedGuard<'a>;
type FullIndexExclusive<'a> = <FullPolicyAlias as LockingPolicy>::IndexExclusiveGuard<'a>;
type FullValuesShared<'a> = <FullPolicyAlias as LockingPolicy>::ValuesSharedGuard<'a>;
type FullValuesExclusive<'a> = <FullPolicyAlias as LockingPolicy>::ValuesExclusiveGuard<'a>;

type AccountPolicyAlias = <IndexLocking<AccountKeyConstraint> as LockingPolicyFactory>::Policy;
type AccountIndexShared<'a> = <AccountPolicyAlias as LockingPolicy>::IndexSharedGuard<'a>;
type AccountIndexExclusive<'a> = <AccountPolicyAlias as LockingPolicy>::IndexExclusiveGuard<'a>;

pub enum IndexSharedGuard<'a> {
    FullLocked(FullIndexShared<'a>),
    AccountLocked(AccountIndexShared<'a>),
    Noop,
}

pub enum IndexExclusiveGuard<'a> {
    FullLocked(FullIndexExclusive<'a>),
    AccountLocked(AccountIndexExclusive<'a>),
    Noop,
}

pub enum ValuesSharedGuard<'a> {
    FullLocked(FullValuesShared<'a>),
    Noop,
}

pub enum ValuesExclusiveGuard<'a> {
    FullLocked(FullValuesExclusive<'a>),
    Noop,
}

// ─── LockingPolicy ───────────────────────────────────────────────────────────

enum PolicyImpl {
    Full(FullPolicyAlias),
    Local,
    Account(AccountPolicyAlias),
}

/// Locking policy produced by [`StorageLockingPolicyFactory`].
///
/// The concrete locking strategy is chosen by [`StorageLockingPolicyFactory`]
/// at construction time and stored as a `PolicyImpl` discriminant. Each
/// trait method dispatches through a `match` on that discriminant and
/// delegates to the appropriate primitive.
pub struct StorageLockingPolicy {
    inner: PolicyImpl,
}

// SAFETY: every variant delegates to a primitive that itself upholds
// `LockingPolicy`'s invariants:
// - Full: built-in `FullLockingPolicy` (two `parking_lot::RwLock`s).
// - Local: vacuous; the storage that owns this policy is constrained
//   by the binding layer to single-threaded use.
// - Account: built-in `IndexLockingPolicy` (one `parking_lot::RwLock`
//   on the index; values are left to the upstream contract described
//   on `SyncMode::Account`).
unsafe impl LockingPolicy for StorageLockingPolicy {
    type IndexSharedGuard<'a> = IndexSharedGuard<'a>;
    type IndexExclusiveGuard<'a> = IndexExclusiveGuard<'a>;
    type ValuesSharedGuard<'a> = ValuesSharedGuard<'a>;
    type ValuesExclusiveGuard<'a> = ValuesExclusiveGuard<'a>;

    fn read_index(&self) -> IndexSharedGuard<'_> {
        match &self.inner {
            PolicyImpl::Full(policy) => IndexSharedGuard::FullLocked(policy.read_index()),
            PolicyImpl::Local => IndexSharedGuard::Noop,
            PolicyImpl::Account(policy) => IndexSharedGuard::AccountLocked(policy.read_index()),
        }
    }

    fn write_index(&self) -> IndexExclusiveGuard<'_> {
        match &self.inner {
            PolicyImpl::Full(policy) => IndexExclusiveGuard::FullLocked(policy.write_index()),
            PolicyImpl::Local => IndexExclusiveGuard::Noop,
            PolicyImpl::Account(policy) => IndexExclusiveGuard::AccountLocked(policy.write_index()),
        }
    }

    fn read_values<Key>(&self, key: &Key) -> ValuesSharedGuard<'_> {
        match &self.inner {
            PolicyImpl::Full(policy) => ValuesSharedGuard::FullLocked(policy.read_values(key)),
            PolicyImpl::Local => ValuesSharedGuard::Noop,
            PolicyImpl::Account(_) => ValuesSharedGuard::Noop,
        }
    }

    fn write_values<Key>(&self, key: &Key) -> ValuesExclusiveGuard<'_> {
        match &self.inner {
            PolicyImpl::Full(policy) => ValuesExclusiveGuard::FullLocked(policy.write_values(key)),
            PolicyImpl::Local => ValuesExclusiveGuard::Noop,
            PolicyImpl::Account(_) => ValuesExclusiveGuard::Noop,
        }
    }
}

// ─── LockingPolicyFactory ─────────────────────────────────────────────────────

/// Factory of `StorageLockingPolicy` instances.
///
/// Constructs a runtime-selected locking policy for the chosen [`SyncMode`].
/// Storage created through this factory is restricted to [`AccountKey`] keys.
///
/// [`AccountKey`]: openpit::AccountKey
pub struct StorageLockingPolicyFactory {
    mode: SyncMode,
}

impl LockingPolicyFactory for StorageLockingPolicyFactory {
    type Policy = StorageLockingPolicy;
    type IndexFlag = std::sync::atomic::AtomicBool;

    fn create_policy(&self) -> StorageLockingPolicy {
        let inner = match self.mode {
            SyncMode::Full => PolicyImpl::Full(FullLocking.create_policy()),
            SyncMode::Local => PolicyImpl::Local,
            SyncMode::Account => {
                PolicyImpl::Account(IndexLocking::<AccountKeyConstraint>::default().create_policy())
            }
        };
        StorageLockingPolicy { inner }
    }
}

// Storage created through this factory is restricted at compile time to
// keys that identify an account (`openpit::AccountKey`). This is the
// universal rule for the interop layer regardless of the selected
// `SyncMode`: real clients shard per-account state by account, so the
// primary `Storage` key always carries an `AccountId`. Per-account state
// structure is then the caller's choice (e.g. a HashMap inside the value
// type). All built-in policies satisfy this by construction. Custom
// policies registered through the language bindings inherit the same
// bound when the `Storage` API is exposed to bindings.
impl<Key> openpit::storage::CreateStorageFor<Key> for StorageLockingPolicyFactory where
    Key: AccountKey + 'static
{
}

// ─── InteropEngineTrait ───────────────────────────────────────────────────────

/// [`openpit::EngineTrait`] alias for engines constructed through the binding layer.
///
/// Generic over the binding-supplied order, execution-report, and
/// account-adjustment types; the synchronization mode is fixed to
/// [`EngineLocking`], the runtime-mode dispatcher.
pub type InteropEngineTrait<Order, ExecutionReport, AccountAdjustment> =
    openpit::EngineTraitOf<Order, ExecutionReport, AccountAdjustment, EngineLocking>;

#[cfg(test)]
mod tests {
    use openpit::pretrade::policies::OrderValidationPolicy;

    use super::{EngineLocking, InteropEngineTrait, SyncMode};

    type Engine = openpit::Engine<InteropEngineTrait<openpit::OrderOperation, (), ()>>;

    fn build_engine(mode: SyncMode) -> Result<Engine, openpit::EngineBuildError> {
        openpit::EngineBuilder::<openpit::OrderOperation, (), ()>::new()
            .sync(EngineLocking::new(mode))
            .pre_trade(OrderValidationPolicy::new())
            .build()
    }

    #[test]
    fn engine_locking_new_full_builds_engine() {
        assert!(build_engine(SyncMode::Full).is_ok());
    }

    #[test]
    fn engine_locking_new_local_builds_engine() {
        assert!(build_engine(SyncMode::Local).is_ok());
    }

    #[test]
    fn engine_locking_new_account_builds_engine() {
        assert!(build_engine(SyncMode::Account).is_ok());
    }

    #[test]
    fn engine_locking_add_check_pre_trade_start_policy_builds_engine() {
        let result = openpit::EngineBuilder::<openpit::OrderOperation, (), ()>::new()
            .sync(EngineLocking::new(SyncMode::Full))
            .pre_trade(OrderValidationPolicy::new())
            .build();
        assert!(result.is_ok());
    }

    #[test]
    fn engine_locking_engine_executes_pre_trade() {
        let engine = build_engine(SyncMode::Full).expect("engine build");
        let result = engine.execute_pre_trade(openpit::OrderOperation {
            instrument: openpit::Instrument::new(
                openpit::param::Asset::new("BTC").expect("asset"),
                openpit::param::Asset::new("USD").expect("asset"),
            ),
            account_id: openpit::param::AccountId::from_u64(1),
            side: openpit::param::Side::Buy,
            trade_amount: openpit::param::TradeAmount::Quantity(
                openpit::param::Quantity::from_f64(1.0).expect("quantity"),
            ),
            price: None,
        });
        assert!(result.is_ok());
    }
}
