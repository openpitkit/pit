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

//! Internal-lock abstraction for [`MarketDataService`](super::MarketDataService).
//!
//! [`MarketDataLock`] abstracts the per-field synchronization primitive that
//! the market-data service uses for its registry, slot vector, and per-slot
//! state. The concrete primitive is selected by a
//! [`MarketDataSync`](super::MarketDataSync) mode: a genuine no-op for
//! single-thread embeddings, a `parking_lot::RwLock` for thread-safe
//! embeddings, or a runtime-branched lock chosen by the binding layer.
//!
//! [`ServiceTtlGate`] abstracts the monotonic "any service-level TTL set?"
//! flag the service consults to skip the service-level cascade tiers: a plain
//! [`Cell`](std::cell::Cell) for single-thread modes, an
//! [`AtomicBool`](std::sync::atomic::AtomicBool) for the thread-shared modes.

use std::cell::{Cell, UnsafeCell};
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicBool, Ordering};

use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};

/// Read/write lock over a `T` used by the market-data service internals.
///
/// # Safety
///
/// The service relies on the lock to uphold the following invariants:
///
/// * While any [`Self::ReadGuard`] is alive, no thread mutates the protected
///   `T`.
/// * While any [`Self::WriteGuard`] is alive, no other thread holds any guard
///   for the same lock.
///
/// The built-in implementations supplied with this crate
/// ([`NoopLock`] and [`RwLock`]) honour these invariants. A no-op
/// implementation honours them vacuously by constraining the owning service
/// to single-threaded use (the service handle is `!Send`/`!Sync` in that
/// mode); getting the invariants wrong leads to undefined behaviour.
pub unsafe trait MarketDataLock<T> {
    /// Guard returned by [`Self::read`].
    type ReadGuard<'a>: Deref<Target = T>
    where
        Self: 'a,
        T: 'a;
    /// Guard returned by [`Self::write`].
    type WriteGuard<'a>: DerefMut<Target = T>
    where
        Self: 'a,
        T: 'a;

    /// Acquires shared (read) access to the protected value.
    fn read(&self) -> Self::ReadGuard<'_>;

    /// Acquires exclusive (write) access to the protected value.
    fn write(&self) -> Self::WriteGuard<'_>;
}

// ─── NoopLock ────────────────────────────────────────────────────────────────

/// No-synchronization lock used by single-thread market-data services.
///
/// Stores the value in an [`UnsafeCell`] and hands out `&T` / `&mut T`
/// directly with no runtime synchronization. Sound only because the owning
/// service is constrained to single-threaded use (its handle is `Rc`, hence
/// `!Send`/`!Sync`, in [`LocalSync`](crate::LocalSync) mode; the binding
/// layer enforces the same constraint at runtime in no-sync mode).
pub struct NoopLock<T> {
    value: UnsafeCell<T>,
}

impl<T> NoopLock<T> {
    /// Wraps `value` in a no-op lock.
    pub fn new(value: T) -> Self {
        Self {
            value: UnsafeCell::new(value),
        }
    }
}

/// Read guard handed out by [`NoopLock`].
pub struct NoopReadGuard<'a, T> {
    value: &'a T,
}

impl<T> Deref for NoopReadGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.value
    }
}

/// Write guard handed out by [`NoopLock`].
pub struct NoopWriteGuard<'a, T> {
    value: &'a mut T,
}

impl<T> Deref for NoopWriteGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.value
    }
}

impl<T> DerefMut for NoopWriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.value
    }
}

// SAFETY: every access is a no-op that hands out a `&T`/`&mut T` borrowed
// from the `UnsafeCell`. The owning service is constrained to a single
// thread (`Rc` handle is `!Send`/`!Sync` under `LocalSync`; the binding
// layer constrains to single-threaded use in no-sync mode), so no two
// threads ever observe the same `NoopLock` concurrently. The guard
// lifetimes confine the borrows to the duration of the access, and the
// service never holds overlapping `write` guards on the same lock, so the
// `&mut T` never aliases. This is the same `UnsafeCell` contract that
// `NoLocking` relies on for storage.
unsafe impl<T> MarketDataLock<T> for NoopLock<T> {
    type ReadGuard<'a>
        = NoopReadGuard<'a, T>
    where
        Self: 'a,
        T: 'a;
    type WriteGuard<'a>
        = NoopWriteGuard<'a, T>
    where
        Self: 'a,
        T: 'a;

    #[inline]
    fn read(&self) -> Self::ReadGuard<'_> {
        // SAFETY: single-threaded use guarantees no concurrent writer; the
        // service does not create a `read` borrow while a `write` borrow is
        // live on the same lock.
        NoopReadGuard {
            value: unsafe { &*self.value.get() },
        }
    }

    #[inline]
    fn write(&self) -> Self::WriteGuard<'_> {
        // SAFETY: single-threaded use guarantees no concurrent access; the
        // service does not create overlapping borrows on the same lock.
        NoopWriteGuard {
            value: unsafe { &mut *self.value.get() },
        }
    }
}

// ─── RwLock (parking_lot) ─────────────────────────────────────────────────────

// SAFETY: `parking_lot::RwLock` upholds the standard reader-writer
// invariants, which is exactly what the `MarketDataLock` contract requires.
unsafe impl<T> MarketDataLock<T> for RwLock<T> {
    type ReadGuard<'a>
        = RwLockReadGuard<'a, T>
    where
        Self: 'a,
        T: 'a;
    type WriteGuard<'a>
        = RwLockWriteGuard<'a, T>
    where
        Self: 'a,
        T: 'a;

    #[inline]
    fn read(&self) -> Self::ReadGuard<'_> {
        RwLock::read(self)
    }

    #[inline]
    fn write(&self) -> Self::WriteGuard<'_> {
        RwLock::write(self)
    }
}

// ─── RuntimeLock ───────────────────────────────────────────────────────────────

/// Runtime-branched market-data lock for the binding layer.
///
/// Mirrors the storage `StorageLockingPolicy` pattern: a discriminant chosen
/// at construction time selects either a genuine no-op (single-thread no-sync
/// mode) or a real `parking_lot::RwLock` (`Full`/`Account` modes). The binding
/// crate selects the variant per service from its runtime sync mode.
pub enum RuntimeLock<T> {
    /// No synchronization. Sound only because the binding layer constrains the
    /// owning service to single-threaded use in no-sync mode.
    Noop(UnsafeCell<T>),
    /// Real reader-writer lock used in the thread-shared modes.
    Locked(RwLock<T>),
}

impl<T> RuntimeLock<T> {
    /// Builds a no-op runtime lock (binding no-sync mode).
    pub fn noop(value: T) -> Self {
        Self::Noop(UnsafeCell::new(value))
    }

    /// Builds a real reader-writer runtime lock (binding `Full`/`Account`
    /// modes).
    pub fn locked(value: T) -> Self {
        Self::Locked(RwLock::new(value))
    }
}

// SAFETY: the binding layer drives the soundness story (mirroring
// `EngineHandle`/`StorageLockingPolicy`):
// - `Locked` wraps a `parking_lot::RwLock`, which is `Send + Sync` for
//   `T: Send + Sync` and synchronizes all access at runtime.
// - `Noop` wraps an `UnsafeCell`; it is only constructed in no-sync mode,
//   where the binding layer constrains the owning service to single-threaded
//   use, so no two threads ever observe the same `RuntimeLock` concurrently.
// We require `T: Send` so ownership transfer across threads is sound; `Sync`
// is claimed under the binding threading contract even without `T: Sync`,
// exactly as the interop `EngineHandle` does for its `Arc`.
unsafe impl<T: Send> Send for RuntimeLock<T> {}
unsafe impl<T: Send> Sync for RuntimeLock<T> {}

/// Read guard handed out by [`RuntimeLock`].
pub enum RuntimeReadGuard<'a, T> {
    /// Borrow into the no-op cell.
    Noop(&'a T),
    /// Real reader guard.
    Locked(RwLockReadGuard<'a, T>),
}

impl<T> Deref for RuntimeReadGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        match self {
            Self::Noop(value) => value,
            Self::Locked(guard) => guard,
        }
    }
}

/// Write guard handed out by [`RuntimeLock`].
pub enum RuntimeWriteGuard<'a, T> {
    /// Mutable borrow into the no-op cell.
    Noop(&'a mut T),
    /// Real writer guard.
    Locked(RwLockWriteGuard<'a, T>),
}

impl<T> Deref for RuntimeWriteGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        match self {
            Self::Noop(value) => value,
            Self::Locked(guard) => guard,
        }
    }
}

impl<T> DerefMut for RuntimeWriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        match self {
            Self::Noop(value) => value,
            Self::Locked(guard) => guard,
        }
    }
}

// SAFETY:
// - `Locked` delegates to `parking_lot::RwLock`, which upholds the
//   reader-writer invariants the `MarketDataLock` contract requires.
// - `Noop` hands out borrows from an `UnsafeCell`; it is constructed only in
//   the binding no-sync mode, where the binding layer constrains the owning
//   service to single-threaded use, so no two threads observe the same lock
//   and the service never holds overlapping `write` guards. Same `UnsafeCell`
//   contract as `NoopLock` and the storage no-sync policy.
unsafe impl<T> MarketDataLock<T> for RuntimeLock<T> {
    type ReadGuard<'a>
        = RuntimeReadGuard<'a, T>
    where
        Self: 'a,
        T: 'a;
    type WriteGuard<'a>
        = RuntimeWriteGuard<'a, T>
    where
        Self: 'a,
        T: 'a;

    #[inline]
    fn read(&self) -> Self::ReadGuard<'_> {
        match self {
            // SAFETY: single-threaded no-sync use; no concurrent writer and no
            // live `write` borrow on the same lock.
            Self::Noop(cell) => RuntimeReadGuard::Noop(unsafe { &*cell.get() }),
            Self::Locked(lock) => RuntimeReadGuard::Locked(lock.read()),
        }
    }

    #[inline]
    fn write(&self) -> Self::WriteGuard<'_> {
        match self {
            // SAFETY: single-threaded no-sync use; no concurrent access and no
            // overlapping borrow on the same lock.
            Self::Noop(cell) => RuntimeWriteGuard::Noop(unsafe { &mut *cell.get() }),
            Self::Locked(lock) => RuntimeWriteGuard::Locked(lock.write()),
        }
    }
}

// ─── ServiceTtlGate ────────────────────────────────────────────────────────────

/// Monotonic gate guarding the service-level TTL cascade tiers.
///
/// The market-data service keeps a single such gate, flipped from "unset" to
/// "present" the first time a service-level account/group TTL is inserted. The
/// common deployment never sets one, so the read path can consult the gate and
/// skip the service-level tiers (two map lookups behind shared locks, plus a
/// lazy group resolution) entirely.
///
/// # Contract
///
/// The flag is **strictly monotonic**: once [`mark_present`](Self::mark_present)
/// has been called it must keep reporting "present" forever. The service never
/// resets it (leaving it set after a `clear_*` is conservatively correct: it
/// only forgoes the optimization), so racing setters and readers always resolve
/// to the safe side. For the thread-shared implementation `mark_present` must
/// also publish, with release ordering, every write made before it, so that a
/// reader observing "present" (with acquire ordering) is guaranteed to see the
/// inserted entry.
///
/// The [`Send`] supertrait keeps the owning service `Send` when the mode is
/// thread-shareable; the single-thread implementation is deliberately `!Sync`,
/// matching the rest of that mode's no-synchronization primitives.
pub trait ServiceTtlGate: Send {
    /// Builds a gate in the "unset" state.
    fn new_unset() -> Self;

    /// Records that a service-level TTL is now present. Idempotent and
    /// monotonic: never reverts the gate to "unset".
    fn mark_present(&self);

    /// Returns `false` only if no service-level TTL has ever been recorded; a
    /// `true` result is conservative (the read path then consults the maps).
    fn is_possibly_present(&self) -> bool;
}

// SAFETY-of-ordering: `mark_present` stores with `Release` and
// `is_possibly_present` loads with `Acquire`, so a reader observing `true` is
// guaranteed to see every write that happened-before the setter — including the
// map insert the service performs just before calling `mark_present`.
impl ServiceTtlGate for AtomicBool {
    #[inline]
    fn new_unset() -> Self {
        AtomicBool::new(false)
    }

    #[inline]
    fn mark_present(&self) {
        self.store(true, Ordering::Release);
    }

    #[inline]
    fn is_possibly_present(&self) -> bool {
        self.load(Ordering::Acquire)
    }
}

/// Single-thread [`ServiceTtlGate`]: a plain [`Cell<bool>`](Cell), no atomics.
///
/// Sound only in the strictly single-threaded modes, where the owning service
/// is already `!Sync` (its no-op locks store state in [`UnsafeCell`]); the
/// `Cell` adds no further synchronization and needs none.
pub struct LocalTtlGate(Cell<bool>);

impl ServiceTtlGate for LocalTtlGate {
    #[inline]
    fn new_unset() -> Self {
        Self(Cell::new(false))
    }

    #[inline]
    fn mark_present(&self) {
        self.0.set(true);
    }

    #[inline]
    fn is_possibly_present(&self) -> bool {
        self.0.get()
    }
}
