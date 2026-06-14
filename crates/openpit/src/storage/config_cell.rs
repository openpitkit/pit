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

//! Sync-mode-aware read-mostly cell for a policy's runtime settings.
//!
//! A [`ConfigCell`] holds a single settings value that is read on the hot
//! path and replaced rarely (only by the engine). Two implementations are
//! available:
//!
//! * [`LocalConfigCell`] - `Rc<RefCell<Rc<T>>>`, for explicitly
//!   single-threaded consumers. Zero atomics.
//! * [`ArcSwapConfigCell`] - `Arc`-shared `arc_swap::ArcSwap<T>` for full and
//!   account locking modes. Lock-free reads and serialized transactional
//!   updates. No-locking mode uses [`LocalConfigCell`].
//!
//! Both cells are read-mostly: [`ConfigCell::with`] never clones the
//! shared value, and [`ConfigCell::update`] is transactional - it mutates
//! a private clone and publishes it only on success, so a closure that
//! returns `Err` or panics leaves the previously published value intact.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

/// Read-mostly cell for a policy's runtime settings.
///
/// Reads ([`Self::with`]) happen on the hot path and must not clone the
/// stored value; updates ([`Self::update`]) are engine-only and rare.
///
/// Cloning a cell yields another handle to the *same* underlying value:
/// an [`Self::update`] performed through one clone is observed by
/// [`Self::with`] through every other clone. This shared-ownership
/// property is what lets a settings handle be distributed across policy
/// clones while a single engine-side handle retains the right to publish
/// new values.
///
/// # Transactionality
///
/// [`Self::update`] runs the caller closure against a private copy of the
/// current value and publishes the result only when the closure returns
/// `Ok`. If the closure returns `Err` or panics, no new value is
/// published and the previously stored value remains visible to readers.
pub trait ConfigCell<T: 'static>: Clone + 'static {
    /// Creates a cell holding `value`.
    fn new(value: T) -> Self;

    /// Reads the current value without cloning it, returning whatever the
    /// closure computes from a shared reference.
    fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R;

    /// Replaces the current value transactionally.
    ///
    /// The closure receives a mutable reference to a private copy of the
    /// current value. The copy is published as the new value only if the
    /// closure returns `Ok`; on `Err` (or panic) the previously stored
    /// value is left untouched and the error is propagated.
    fn update<E>(&self, f: impl FnOnce(&mut T) -> Result<(), E>) -> Result<(), E>;
}

/// Single-threaded [`ConfigCell`] backed by `Rc<RefCell<Rc<T>>>`.
///
/// Performs no atomic operations. Cloning shares the same underlying cell.
pub struct LocalConfigCell<T>(Rc<RefCell<Rc<T>>>);

impl<T> Clone for LocalConfigCell<T> {
    /// Clones the handle, not the value: both handles see the same cell.
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}

impl<T: Clone + 'static> ConfigCell<T> for LocalConfigCell<T> {
    fn new(value: T) -> Self {
        Self(Rc::new(RefCell::new(Rc::new(value))))
    }

    #[inline]
    fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        let guard = self.0.borrow();
        f(&guard)
    }

    fn update<E>(&self, f: impl FnOnce(&mut T) -> Result<(), E>) -> Result<(), E> {
        // Mutate a private clone so a failing closure cannot leave a
        // half-updated value visible. The borrow is taken only after the
        // closure succeeds, keeping `with` re-entrant during the call.
        let current = Rc::clone(&self.0.borrow());
        let mut next = (*current).clone();
        f(&mut next)?;
        *self.0.borrow_mut() = Rc::new(next);
        Ok(())
    }
}

/// Thread-shared [`ConfigCell`] backed by [`arc_swap::ArcSwap`].
///
/// Runtime-configurable policies use this cell in full and account locking
/// modes; no-locking mode uses [`LocalConfigCell`]. Reads are lock-free and
/// never clone the inner `Arc`. A writer-only mutex
/// serializes the rare transactional updates so two concurrent
/// read-modify-write closures cannot overwrite each other.
///
/// The cell is `Send + Sync` whenever `T: Send + Sync`.
pub struct ArcSwapConfigCell<T>(Arc<ArcSwapConfigInner<T>>);

struct ArcSwapConfigInner<T> {
    value: arc_swap::ArcSwap<T>,
    writer: parking_lot::Mutex<()>,
}

impl<T> Clone for ArcSwapConfigCell<T> {
    /// Clones the handle, not the value: both handles see the same cell.
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl<T: Clone + 'static> ConfigCell<T> for ArcSwapConfigCell<T> {
    fn new(value: T) -> Self {
        Self(Arc::new(ArcSwapConfigInner {
            value: arc_swap::ArcSwap::from_pointee(value),
            writer: parking_lot::Mutex::new(()),
        }))
    }

    #[inline]
    fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        // `load` (not `load_full`) avoids bumping the inner `Arc`'s
        // refcount on the hot read path; the guard keeps the pointee
        // alive for the duration of the call.
        let guard = self.0.value.load();
        f(&guard)
    }

    fn update<E>(&self, f: impl FnOnce(&mut T) -> Result<(), E>) -> Result<(), E> {
        // The lock covers the entire read-modify-publish transaction. Reads
        // never take it, while concurrent writers cannot clone the same
        // snapshot and silently overwrite one another.
        //
        // The blocking `lock()` is deliberate: it serializes concurrent
        // updates from *different* threads, which is correct and must keep
        // working. A `try_lock` here would be wrong - it would spuriously
        // fail a legitimate cross-thread update merely because another thread
        // is mid-publish, turning benign contention into an error.
        //
        // Re-entrancy is ruled out one level up: the `Configurator`
        // (`core::configure`) installs a thread-local guard before reaching
        // any cell, so the closure `f` can never re-enter configuration and
        // retake this non-reentrant mutex on the same thread. This lock
        // therefore only ever blocks for a *different* thread's update.
        let _writer = self.0.writer.lock();
        let mut next = (**self.0.value.load()).clone();
        f(&mut next)?;
        self.0.value.store(Arc::new(next));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_update_visible<Cell: ConfigCell<i32>>() {
        let cell = Cell::new(1);
        cell.update::<()>(|v| {
            *v = 42;
            Ok(())
        })
        .expect("update succeeds");
        assert_eq!(cell.with(|v| *v), 42);
    }

    fn assert_err_keeps_prior<Cell: ConfigCell<i32>>() {
        let cell = Cell::new(7);
        let result = cell.update(|v| {
            *v = 99;
            Err("rejected")
        });
        assert_eq!(result, Err("rejected"));
        // The failed update must not have published its mutation.
        assert_eq!(cell.with(|v| *v), 7);
    }

    fn assert_clone_shares_underlying<Cell: ConfigCell<i32>>() {
        let cell = Cell::new(0);
        let other = cell.clone();
        cell.update::<()>(|v| {
            *v = 5;
            Ok(())
        })
        .expect("update succeeds");
        // The update via `cell` must be observed through the clone.
        assert_eq!(other.with(|v| *v), 5);
    }

    #[test]
    fn local_update_visible_through_with() {
        assert_update_visible::<LocalConfigCell<i32>>();
    }

    #[test]
    fn local_err_keeps_prior_value() {
        assert_err_keeps_prior::<LocalConfigCell<i32>>();
    }

    #[test]
    fn local_clone_shares_underlying() {
        assert_clone_shares_underlying::<LocalConfigCell<i32>>();
    }

    #[test]
    fn arc_swap_update_visible_through_with() {
        assert_update_visible::<ArcSwapConfigCell<i32>>();
    }

    #[test]
    fn arc_swap_err_keeps_prior_value() {
        assert_err_keeps_prior::<ArcSwapConfigCell<i32>>();
    }

    #[test]
    fn arc_swap_clone_shares_underlying() {
        assert_clone_shares_underlying::<ArcSwapConfigCell<i32>>();
    }

    #[test]
    fn arc_swap_panic_in_update_keeps_prior_value() {
        let cell = ArcSwapConfigCell::new(3);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            cell.update::<()>(|v| {
                *v = 100;
                panic!("boom");
            })
            .expect("unreachable: closure panics");
        }));
        assert!(result.is_err());
        cell.update::<()>(|v| {
            *v = 4;
            Ok(())
        })
        .expect("writer lock is released during unwind");
        assert_eq!(cell.with(|v| *v), 4);
    }

    // Mirrors the race test in `core/account_control.rs`: several readers
    // hammer `with` while a writer churns `update`. No reader may observe
    // a torn value - every snapshot is a complete prior/next value - and
    // the terminal value is deterministic.
    #[test]
    fn arc_swap_concurrent_store_load_no_torn_read() {
        use std::sync::Arc;
        use std::thread;

        // A two-field value: an update keeps both fields equal, so any
        // reader seeing unequal fields would prove a torn read.
        #[derive(Clone)]
        struct Paired {
            high: u64,
            low: u64,
        }

        const ROUNDS: u64 = 5_000;

        let cell = Arc::new(ArcSwapConfigCell::new(Paired { high: 0, low: 0 }));

        thread::scope(|scope| {
            // Single writer: monotonically bumps both fields in lockstep.
            let writer = Arc::clone(&cell);
            scope.spawn(move || {
                for round in 1..=ROUNDS {
                    writer
                        .update::<()>(|v| {
                            v.high = round;
                            v.low = round;
                            Ok(())
                        })
                        .expect("update succeeds");
                }
            });
            // Readers verify each observed snapshot is internally
            // consistent (both fields equal).
            for _ in 0..4 {
                let reader = Arc::clone(&cell);
                scope.spawn(move || {
                    for _ in 0..ROUNDS {
                        let torn = reader.with(|v| v.high != v.low);
                        assert!(!torn, "observed a torn read");
                    }
                });
            }
        });

        // Deterministic terminal state after all writes complete.
        assert_eq!(cell.with(|v| (v.high, v.low)), (ROUNDS, ROUNDS));
    }

    #[test]
    fn arc_swap_concurrent_disjoint_updates_are_not_lost() {
        use std::sync::{Arc, Barrier};
        use std::thread;

        #[derive(Clone, Default, PartialEq, Eq, Debug)]
        struct Pair {
            left: bool,
            right: bool,
        }

        let cell = Arc::new(ArcSwapConfigCell::new(Pair::default()));
        let writer = cell.0.writer.lock();
        let start = Arc::new(Barrier::new(3));

        thread::scope(|scope| {
            let left_cell = Arc::clone(&cell);
            let left_start = Arc::clone(&start);
            scope.spawn(move || {
                left_start.wait();
                left_cell
                    .update::<()>(|value| {
                        value.left = true;
                        Ok(())
                    })
                    .expect("left update succeeds");
            });

            let right_cell = Arc::clone(&cell);
            let right_start = Arc::clone(&start);
            scope.spawn(move || {
                right_start.wait();
                right_cell
                    .update::<()>(|value| {
                        value.right = true;
                        Ok(())
                    })
                    .expect("right update succeeds");
            });

            start.wait();
            drop(writer);
        });

        assert_eq!(
            cell.with(Clone::clone),
            Pair {
                left: true,
                right: true,
            }
        );
    }
}
