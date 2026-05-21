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

//! Bool-cell whose synchronization matches the engine's storage index domain.

use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};

/// Bool cell with acquire/release publishing semantics.
///
/// `IndexFlag` is the synchronization counterpart of the storage **index**
/// domain: implementations must guarantee that a successful
/// [`store`](Self::store) happens-before any subsequent
/// [`load`](Self::load) observing the stored value, across the threads that
/// the engine's storage factory admits as observers of the storage index domain.
///
/// The trait is open. Built-in implementations are provided for
/// [`std::cell::Cell<bool>`] (single-thread) and
/// [`std::sync::atomic::AtomicBool`] (multi-thread).
///
/// # Future companion
///
/// A parallel `ValuesFlag` for the **values** domain will be added when a use
/// case appears. Under [`LocalSync`](crate::LocalSync)/
/// [`AccountSync`](crate::AccountSync) it would be `Cell<bool>` (the values
/// domain is single-observer in both modes); only
/// [`FullSync`](crate::FullSync) would map it to `AtomicBool`.
pub trait IndexFlag: 'static {
    /// Creates a new cell with the given initial value.
    fn new(initial: bool) -> Self;

    /// Loads the current value with Acquire ordering semantics.
    fn load(&self) -> bool;

    /// Stores `value` with Release ordering semantics.
    fn store(&self, value: bool);
}

impl IndexFlag for Cell<bool> {
    fn new(initial: bool) -> Self {
        Cell::new(initial)
    }

    fn load(&self) -> bool {
        self.get()
    }

    fn store(&self, value: bool) {
        self.set(value);
    }
}

impl IndexFlag for AtomicBool {
    fn new(initial: bool) -> Self {
        AtomicBool::new(initial)
    }

    fn load(&self) -> bool {
        AtomicBool::load(self, Ordering::Acquire)
    }

    fn store(&self, value: bool) {
        AtomicBool::store(self, value, Ordering::Release);
    }
}
