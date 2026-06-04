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

//! Builder and sync-mode trait for [`MarketDataService`].

use std::collections::HashMap;
use std::ops::Deref;
use std::rc::Rc;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use parking_lot::RwLock;

use crate::core::sync_mode::{FullSync, LocalSync};
use crate::param::{AccountGroupId, AccountId};

use super::internals::Slot;
use super::lock::{LocalTtlGate, MarketDataLock, NoopLock, ServiceTtlGate};
use super::quote::QuoteTtl;
use super::service::{InstrumentRegistry, MarketDataService};
use super::ttl::TtlSetting;

// ─── MarketDataSync ──────────────────────────────────────────────────────────

/// Sealing supertrait for [`MarketDataSync`].
///
/// This module is public so that the binding crate `openpit-interop` can
/// implement [`MarketDataSync`] for its runtime-mode dispatcher; no other
/// external implementations are intended. Pure Rust SDK clients use the
/// built-in [`LocalSync`] and [`FullSync`] modes.
pub mod sealed {
    /// Marker required to implement [`MarketDataSync`](super::MarketDataSync).
    pub trait Sealed {}
}

/// Synchronization contract for [`MarketDataService`].
///
/// A mode selects two synchronization aspects of the service:
///
/// * the shared handle ([`Self::Shared`]) used to distribute the service
///   (`Rc` for single-thread modes, `Arc` for thread-shareable modes), and
/// * the internal lock primitive ([`Self::Lock`]) used for the registry, the
///   slot vector, and every per-slot field.
///
/// The trait is sealed via [`sealed::Sealed`]; it is implemented by the
/// built-in [`LocalSync`] and [`FullSync`] modes and by the binding-layer
/// dispatcher in `openpit-interop`. It is **instance**-based: the constructor
/// methods take `&self` so that a runtime-mode dispatcher can pick a no-op or
/// a real lock per slot. Implementors are [`Clone`] (typically ZST or a
/// `Copy` runtime mode) so the service can retain its own mode instance for
/// post-build slot registration.
///
/// # Threading
///
/// - [`LocalSync`]: `Rc` handle plus genuinely no-op internal locks. The
///   service is strictly single-threaded; there is **no** internal
///   synchronization. A concurrent producer (such as a background price feed)
///   is **not** supported - upgrade to [`FullSync`] via
///   [`MarketDataBuilder::full_sync`] for that.
/// - [`FullSync`]: `Arc` handle plus real `parking_lot::RwLock` internal
///   locks. The service is `Send + Sync` and supports a concurrent feed.
pub trait MarketDataSync: sealed::Sealed + Clone + 'static {
    /// Shared pointer type used to distribute the service handle.
    type Shared<T: 'static>: Clone + Deref<Target = T>;

    /// Internal lock primitive used by the service for its registry, slot
    /// vector, and per-slot fields.
    type Lock<T>: MarketDataLock<T>;

    /// Service-level-TTL gate primitive: a non-atomic [`LocalTtlGate`] for the
    /// single-thread modes, an [`AtomicBool`] for the thread-shared modes.
    type Gate: ServiceTtlGate;

    /// Wraps `inner` in a new shared handle.
    fn new_shared<T: 'static>(&self, inner: T) -> Self::Shared<T>;

    /// Wraps `inner` in a new internal lock of this mode.
    fn new_lock<T>(&self, inner: T) -> Self::Lock<T>;

    /// Builds the service-level-TTL gate in the "unset" state.
    fn new_gate(&self) -> Self::Gate {
        Self::Gate::new_unset()
    }
}

impl sealed::Sealed for LocalSync {}
impl sealed::Sealed for FullSync {}

impl MarketDataSync for LocalSync {
    type Shared<T: 'static> = Rc<T>;
    type Lock<T> = NoopLock<T>;
    // Single-threaded: a plain `Cell`, no concurrency primitive on the read
    // path. Sound because the service is already `!Sync` under `LocalSync`.
    type Gate = LocalTtlGate;

    #[inline]
    fn new_shared<T: 'static>(&self, inner: T) -> Rc<T> {
        Rc::new(inner)
    }

    #[inline]
    fn new_lock<T>(&self, inner: T) -> NoopLock<T> {
        NoopLock::new(inner)
    }
}

impl MarketDataSync for FullSync {
    type Shared<T: 'static> = Arc<T>;
    type Lock<T> = RwLock<T>;
    // Thread-shared: an atomic gate keeps the service `Send + Sync`.
    type Gate = AtomicBool;

    #[inline]
    fn new_shared<T: 'static>(&self, inner: T) -> Arc<T> {
        Arc::new(inner)
    }

    #[inline]
    fn new_lock<T>(&self, inner: T) -> RwLock<T> {
        RwLock::new(inner)
    }
}

// ─── MarketDataBuilder ───────────────────────────────────────────────────────

/// Builder for [`MarketDataService`].
///
/// The service-wide quote lifetime ([`QuoteTtl`]) is a required constructor
/// argument - either [`QuoteTtl::Infinite`] or [`QuoteTtl::Within`]. The
/// chosen default applies to every instrument unless overridden at
/// registration time via
/// [`MarketDataService::register_with_ttl`](super::service::MarketDataService::register_with_ttl)
/// or
/// [`MarketDataService::register_with_id_and_ttl`](super::service::MarketDataService::register_with_id_and_ttl).
///
/// The `Sync` parameter controls both the handle wrapper (`Rc` vs `Arc`) that
/// distributes the service and the internal lock primitive. Under
/// [`LocalSync`] the internal locks are genuine no-ops and the service is
/// strictly single-threaded; under [`FullSync`] they are real
/// `parking_lot::RwLock`s and a concurrent producer thread is supported.
pub struct MarketDataBuilder<Sync: MarketDataSync> {
    default_ttl: QuoteTtl,
    sync: Sync,
}

impl<Sync: MarketDataSync + Default> MarketDataBuilder<Sync> {
    /// Creates a new builder with the given default quote lifetime, using the
    /// default instance of the sync mode.
    ///
    /// Available for the zero-sized compile-time modes ([`LocalSync`],
    /// [`FullSync`]). The binding-layer runtime-mode dispatcher carries data
    /// and must use [`with_sync`](Self::with_sync) instead.
    pub fn new(default_ttl: QuoteTtl) -> Self {
        Self::with_sync(Sync::default(), default_ttl)
    }
}

impl<Sync: MarketDataSync> MarketDataBuilder<Sync> {
    /// Creates a new builder for the given sync-mode instance and default
    /// quote lifetime.
    ///
    /// The binding layer uses this to pass a runtime-mode dispatcher; pure
    /// Rust clients of the zero-sized modes use [`new`](Self::new).
    pub fn with_sync(sync: Sync, default_ttl: QuoteTtl) -> Self {
        Self { default_ttl, sync }
    }

    /// Consumes the builder and produces a shared [`MarketDataService`] handle.
    ///
    /// The returned service starts with no registered instruments. Register
    /// instruments at any time via
    /// [`MarketDataService::register`](super::service::MarketDataService::register).
    pub fn build(self) -> Sync::Shared<MarketDataService<Sync>> {
        let account_ttl: Sync::Lock<HashMap<AccountId, TtlSetting>> =
            self.sync.new_lock(HashMap::new());
        let group_ttl: Sync::Lock<HashMap<AccountGroupId, TtlSetting>> =
            self.sync.new_lock(HashMap::new());
        let registry = self.sync.new_lock(InstrumentRegistry::new());
        let slots: Sync::Lock<Vec<Slot<Sync>>> = self.sync.new_lock(Vec::new());
        let has_service_level_ttl = self.sync.new_gate();
        let service = MarketDataService {
            default_ttl: self.default_ttl.as_duration(),
            account_ttl,
            group_ttl,
            registry,
            slots,
            sync: self.sync.clone(),
            has_service_level_ttl,
        };
        self.sync.new_shared(service)
    }
}

impl MarketDataBuilder<LocalSync> {
    /// Upgrades this builder to produce a [`MarketDataService<FullSync>`].
    ///
    /// A [`LocalSync`] service is strictly single-threaded (no-op internal
    /// locks, `Rc` handle). This helper switches both the handle wrapper to
    /// [`Arc`] and the internal locks to real `parking_lot::RwLock`s so the
    /// service can be shared across OS threads and accept a concurrent
    /// producer (such as a background feed).
    pub fn full_sync(self) -> MarketDataBuilder<FullSync> {
        MarketDataBuilder {
            default_ttl: self.default_ttl,
            sync: FullSync,
        }
    }
}
