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

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::core::Instrument;
use crate::param::{AccountGroupId, AccountId, DEFAULT_ACCOUNT_GROUP};

use super::builder::MarketDataSync;
use super::error::{
    AlreadyRegistered, MarketDataError, PushForError, RegistrationError, UnknownInstrumentId,
};
use super::instrument_id::InstrumentId;
use super::internals::{QuoteState, Slot, SlotQuotes, SlotTtls};
use super::lock::{MarketDataLock, ServiceTtlGate};
use super::quote::{Quote, QuoteTtl};
use super::resolution::{AccountInfo, QuoteResolution};
use super::ttl::TtlSetting;

// ─── InstrumentRegistry ───────────────────────────────────────────────────────

/// Internal instrument registration state.
pub(crate) struct InstrumentRegistry {
    /// External ID -> internal compact slot index.
    by_id: HashMap<InstrumentId, u32>,
    /// Instrument name -> external ID (for `resolve` and `push_by_instrument`).
    by_instrument: HashMap<Instrument, InstrumentId>,
    /// Counter for auto-assigned IDs. Advanced past any custom IDs that
    /// would collide.
    ///
    /// The slot index is a `u32`, capping the service at ~4.29e9 distinct
    /// instruments.
    next_auto_id: u64,
    /// High-water mark for slot allocation (u32, see note on `next_auto_id`).
    next_slot: u32,
}

impl InstrumentRegistry {
    pub(crate) fn new() -> Self {
        Self {
            by_id: HashMap::new(),
            by_instrument: HashMap::new(),
            next_slot: 0,
            next_auto_id: 0,
        }
    }

    fn alloc_slot(&mut self) -> u32 {
        let slot = self.next_slot;
        self.next_slot += 1;
        slot
    }

    /// Returns the next unused auto-assigned id, skipping any values already
    /// present in `by_id` (inserted by caller-supplied registrations).
    fn next_auto_id(&mut self) -> InstrumentId {
        loop {
            let candidate = InstrumentId(self.next_auto_id);
            self.next_auto_id += 1;
            if !self.by_id.contains_key(&candidate) {
                return candidate;
            }
        }
    }

    fn insert_id(&mut self, instrument_id: InstrumentId, slot: u32) {
        self.by_id.insert(instrument_id, slot);
        if self.next_auto_id == instrument_id.0 {
            self.next_auto_id += 1;
        }
    }
}

// ─── MarketDataService ────────────────────────────────────────────────────────

/// Live market-data service.
///
/// Quotes are stored per instrument in three conceptual buckets: a per-account
/// bucket for every targeted account, a per-group bucket for every targeted
/// group, and the default ("everyone-else") bucket - which is the bucket of the
/// reserved [`DEFAULT_ACCOUNT_GROUP`](crate::param::DEFAULT_ACCOUNT_GROUP).
///
/// Instruments must be registered before quotes can be pushed by id. Use
/// [`register`](Self::register) (or the `_with_ttl` / `_with_id` /
/// `_with_id_and_ttl` variants) to add an instrument. All registration calls
/// are **strict**: they return an error if the instrument name or id is already
/// taken — they never silently return an existing entry.
///
/// Quotes are published on the hot path via:
/// - [`push`](Self::push) / [`push_patch`](Self::push_patch) — by
///   [`InstrumentId`] into the default bucket; the id must have been registered
///   beforehand.
/// - [`push_for`](Self::push_for) / [`push_for_patch`](Self::push_for_patch) —
///   by id, fanned out into the per-account bucket of every listed account and
///   the per-group bucket of every listed group (the default bucket is
///   targetable by listing [`DEFAULT_ACCOUNT_GROUP`]).
/// - [`push_by_instrument`](Self::push_by_instrument) /
///   [`push_by_instrument_patch`](Self::push_by_instrument_patch) — by
///   instrument name into the default bucket; auto-registers a named slot on
///   first sight.
///
/// Consumers poll via [`get`](Self::get) / [`get_or_err`](Self::get_or_err),
/// supplying the reading account, an [`AccountInfo`], and a
/// [`QuoteResolution`] that picks which buckets to consult; quotes older than
/// their effective TTL surface as "unavailable".
///
/// # TTL cascade
///
/// The effective lifetime of a quote is resolved per read for the requested
/// `(instrument, account, group)` by walking, and stopping at the FIRST set
/// value:
///
/// 1. instrument × account
/// 2. instrument × group (when the source has a group)
/// 3. instrument × default-group
/// 4. account (service-level)
/// 5. group (service-level, when the source has a group)
/// 6. default-group (service-level)
/// 7. instrument-level (the registration TTL)
/// 8. the service-wide default from the builder
///
/// A set [`QuoteTtl::Infinite`] resolves the cascade to "no expiry". The group
/// is resolved lazily: [`AccountInfo::group`] is called only when a group tier
/// is reached and every earlier tier was unset. The effective TTL is resolved
/// by the requested `(account, group)`, independent of which bucket the quote
/// was actually found in.
///
/// Settable explicitly via [`set_account_ttl`](Self::set_account_ttl),
/// [`set_account_group_ttl`](Self::set_account_group_ttl),
/// [`set_instrument_ttl`](Self::set_instrument_ttl),
/// [`set_instrument_account_ttl`](Self::set_instrument_account_ttl), and
/// [`set_instrument_account_group_ttl`](Self::set_instrument_account_group_ttl);
/// each level can be reverted to "inherit" via the matching `clear_*_ttl`
/// method.
///
/// Create via [`MarketDataBuilder`](super::builder::MarketDataBuilder).
///
/// # Concurrency
///
/// Push and read hot paths hold `slots.read()` for the duration of the
/// per-slot mutation. Concurrent instrument registration takes `slots.write()`
/// and is therefore serialized against the entire hot path. This is acceptable
/// for the typical workload (many pushes, infrequent registrations); high
/// registration churn would benefit from a different layout.
///
/// The internal locks are selected by the `Sync` mode: genuine no-ops under
/// [`LocalSync`](crate::LocalSync) (strictly single-threaded, zero overhead)
/// and real `parking_lot::RwLock`s under [`FullSync`](crate::FullSync) (a
/// concurrent producer is supported).
pub struct MarketDataService<Sync: MarketDataSync> {
    /// Service-wide default quote lifetime (cascade tier 8); `None` means
    /// infinite.
    pub(crate) default_ttl: Option<Duration>,
    /// Service-level per-account TTL settings (cascade tier 4).
    pub(crate) account_ttl: Sync::Lock<HashMap<AccountId, TtlSetting>>,
    /// Service-level per-group TTL settings (cascade tiers 5-6); the entry at
    /// [`DEFAULT_ACCOUNT_GROUP`] is the service-level default-group cell.
    pub(crate) group_ttl: Sync::Lock<HashMap<AccountGroupId, TtlSetting>>,
    /// Instrument registration state.
    pub(crate) registry: Sync::Lock<InstrumentRegistry>,
    /// Per-instrument quote slots, indexed by internal compact slot index.
    pub(crate) slots: Sync::Lock<Vec<Slot<Sync>>>,
    /// Sync mode instance, retained so post-build slot registration can build
    /// mode-correct per-slot locks.
    pub(crate) sync: Sync,
    /// Monotonic gate over the service-level TTL tiers (4-6): unset until the
    /// first insert into `account_ttl` or `group_ttl`. The common deployment
    /// never sets a service-level TTL, so `effective_ttl` can then skip
    /// read-locking both maps (each lock would otherwise cost a shared
    /// acquisition on every read that reached the service tiers). The gate is
    /// non-atomic under the single-thread mode and atomic under the
    /// thread-shared modes; see [`ServiceTtlGate`] for the monotonicity and
    /// release/acquire contract.
    pub(crate) has_service_level_ttl: Sync::Gate,
}

impl<Sync: MarketDataSync> MarketDataService<Sync> {
    // ── Registration ──────────────────────────────────────────────────────────

    /// Registers `instrument` with the service-wide default TTL and returns an
    /// auto-assigned [`InstrumentId`].
    ///
    /// # Errors
    ///
    /// Returns [`AlreadyRegistered`] if `instrument` is already registered.
    pub fn register(&self, instrument: Instrument) -> Result<InstrumentId, AlreadyRegistered> {
        self.register_inner(instrument, None)
    }

    /// Registers `instrument` with an instrument-level TTL (cascade tier 7) and
    /// returns an auto-assigned [`InstrumentId`].
    ///
    /// The supplied [`QuoteTtl`] becomes the instrument-level setting: it wins
    /// over the service-wide default but is still overridden by any
    /// account/group axis set for this instrument.
    ///
    /// # Errors
    ///
    /// Returns [`AlreadyRegistered`] if `instrument` is already registered.
    pub fn register_with_ttl(
        &self,
        instrument: Instrument,
        ttl: QuoteTtl,
    ) -> Result<InstrumentId, AlreadyRegistered> {
        self.register_inner(instrument, Some(TtlSetting::from_quote_ttl(ttl)))
    }

    fn register_inner(
        &self,
        instrument: Instrument,
        instrument_ttl: Option<TtlSetting>,
    ) -> Result<InstrumentId, AlreadyRegistered> {
        {
            let guard = self.registry.read();
            if guard.by_instrument.contains_key(&instrument) {
                return Err(AlreadyRegistered { instrument });
            }
        }
        let mut reg = self.registry.write();
        if reg.by_instrument.contains_key(&instrument) {
            return Err(AlreadyRegistered { instrument });
        }
        let instrument_id = reg.next_auto_id();
        let slot_idx = reg.alloc_slot();
        reg.by_instrument.insert(instrument, instrument_id);
        reg.insert_id(instrument_id, slot_idx);
        self.ensure_slot_storage(slot_idx, instrument_ttl);
        Ok(instrument_id)
    }

    /// Registers `instrument` with the caller-supplied `id` and the
    /// service-wide default TTL.
    ///
    /// # Errors
    ///
    /// - [`RegistrationError::DuplicateInstrument`] if the instrument name is
    ///   already registered.
    /// - [`RegistrationError::DuplicateId`] if `id` is already registered.
    pub fn register_with_id(
        &self,
        instrument: Instrument,
        instrument_id: InstrumentId,
    ) -> Result<InstrumentId, RegistrationError> {
        self.register_with_id_inner(instrument, instrument_id, None)
    }

    /// Registers `instrument` with the caller-supplied `id` and an
    /// instrument-level TTL (cascade tier 7).
    ///
    /// # Errors
    ///
    /// - [`RegistrationError::DuplicateInstrument`] if the instrument name is
    ///   already registered.
    /// - [`RegistrationError::DuplicateId`] if `id` is already registered.
    pub fn register_with_id_and_ttl(
        &self,
        instrument: Instrument,
        instrument_id: InstrumentId,
        ttl: QuoteTtl,
    ) -> Result<InstrumentId, RegistrationError> {
        self.register_with_id_inner(
            instrument,
            instrument_id,
            Some(TtlSetting::from_quote_ttl(ttl)),
        )
    }

    fn register_with_id_inner(
        &self,
        instrument: Instrument,
        instrument_id: InstrumentId,
        instrument_ttl: Option<TtlSetting>,
    ) -> Result<InstrumentId, RegistrationError> {
        let mut reg = self.registry.write();
        if reg.by_instrument.contains_key(&instrument) {
            return Err(RegistrationError::DuplicateInstrument { instrument });
        }
        if reg.by_id.contains_key(&instrument_id) {
            return Err(RegistrationError::DuplicateId { instrument_id });
        }
        let slot_idx = reg.alloc_slot();
        reg.by_instrument.insert(instrument, instrument_id);
        reg.insert_id(instrument_id, slot_idx);
        self.ensure_slot_storage(slot_idx, instrument_ttl);
        Ok(instrument_id)
    }

    // ── TTL setters ───────────────────────────────────────────────────────────

    /// Pins the service-level TTL for `account_id` (cascade tier 4).
    ///
    /// Applies to every instrument that does not set a more specific
    /// instrument × account cell for `account_id`. Always succeeds.
    pub fn set_account_ttl(&self, account_id: AccountId, ttl: QuoteTtl) {
        self.account_ttl
            .write()
            .insert(account_id, TtlSetting::from_quote_ttl(ttl));
        // Publishes the insert so a reader observing the gate also sees it.
        self.has_service_level_ttl.mark_present();
    }

    /// Reverts the service-level TTL for `account_id` back to "inherit".
    pub fn clear_account_ttl(&self, account_id: AccountId) {
        self.account_ttl.write().remove(&account_id);
    }

    /// Pins the service-level TTL for `account_group_id` (cascade tiers 5-6).
    ///
    /// Pass [`DEFAULT_ACCOUNT_GROUP`] to set the service-level default-group
    /// cell (tier 6). Always succeeds.
    pub fn set_account_group_ttl(&self, account_group_id: AccountGroupId, ttl: QuoteTtl) {
        self.group_ttl
            .write()
            .insert(account_group_id, TtlSetting::from_quote_ttl(ttl));
        // Publishes the insert so a reader observing the gate also sees it.
        self.has_service_level_ttl.mark_present();
    }

    /// Reverts the service-level TTL for `account_group_id` back to "inherit".
    pub fn clear_account_group_ttl(&self, account_group_id: AccountGroupId) {
        self.group_ttl.write().remove(&account_group_id);
    }

    /// Pins the instrument-level TTL for `instrument_id` (cascade tier 7).
    ///
    /// Replaces today's `set_ttl`. The change takes effect immediately: the
    /// next read for `instrument_id` evaluates freshness against the new
    /// cascade.
    ///
    /// # Errors
    ///
    /// Returns [`UnknownInstrumentId`] if `instrument_id` is not registered.
    pub fn set_instrument_ttl(
        &self,
        instrument_id: InstrumentId,
        ttl: QuoteTtl,
    ) -> Result<(), UnknownInstrumentId> {
        self.with_slot_ttls(instrument_id, |ttls| {
            ttls.instrument = Some(TtlSetting::from_quote_ttl(ttl));
        })
    }

    /// Reverts the instrument-level TTL for `instrument_id` back to "inherit".
    ///
    /// # Errors
    ///
    /// Returns [`UnknownInstrumentId`] if `instrument_id` is not registered.
    pub fn clear_instrument_ttl(
        &self,
        instrument_id: InstrumentId,
    ) -> Result<(), UnknownInstrumentId> {
        self.with_slot_ttls(instrument_id, |ttls| {
            ttls.instrument = None;
        })
    }

    /// Pins the instrument × account TTL cell for
    /// `(instrument_id, account_id)` (cascade tier 1, the highest-priority
    /// tier).
    ///
    /// # Errors
    ///
    /// Returns [`UnknownInstrumentId`] if `instrument_id` is not registered.
    pub fn set_instrument_account_ttl(
        &self,
        instrument_id: InstrumentId,
        account_id: AccountId,
        ttl: QuoteTtl,
    ) -> Result<(), UnknownInstrumentId> {
        self.with_slot_ttls(instrument_id, |ttls| {
            ttls.accounts
                .insert(account_id, TtlSetting::from_quote_ttl(ttl));
        })
    }

    /// Reverts the instrument × account TTL cell for
    /// `(instrument_id, account_id)` back to "inherit".
    ///
    /// # Errors
    ///
    /// Returns [`UnknownInstrumentId`] if `instrument_id` is not registered.
    pub fn clear_instrument_account_ttl(
        &self,
        instrument_id: InstrumentId,
        account_id: AccountId,
    ) -> Result<(), UnknownInstrumentId> {
        self.with_slot_ttls(instrument_id, |ttls| {
            ttls.accounts.remove(&account_id);
        })
    }

    /// Pins the instrument × group TTL cell for
    /// `(instrument_id, account_group_id)` (cascade tiers 2-3).
    ///
    /// Pass [`DEFAULT_ACCOUNT_GROUP`] to set the instrument × default-group
    /// cell (tier 3).
    ///
    /// # Errors
    ///
    /// Returns [`UnknownInstrumentId`] if `instrument_id` is not registered.
    pub fn set_instrument_account_group_ttl(
        &self,
        instrument_id: InstrumentId,
        account_group_id: AccountGroupId,
        ttl: QuoteTtl,
    ) -> Result<(), UnknownInstrumentId> {
        self.with_slot_ttls(instrument_id, |ttls| {
            ttls.groups
                .insert(account_group_id, TtlSetting::from_quote_ttl(ttl));
        })
    }

    /// Reverts the instrument × group TTL cell for
    /// `(instrument_id, account_group_id)` back to "inherit".
    ///
    /// # Errors
    ///
    /// Returns [`UnknownInstrumentId`] if `instrument_id` is not registered.
    pub fn clear_instrument_account_group_ttl(
        &self,
        instrument_id: InstrumentId,
        account_group_id: AccountGroupId,
    ) -> Result<(), UnknownInstrumentId> {
        self.with_slot_ttls(instrument_id, |ttls| {
            ttls.groups.remove(&account_group_id);
        })
    }

    /// Resolves `id` to its slot and mutates the slot's TTL settings under the
    /// per-slot TTL lock.
    ///
    /// Holds `slots.read()` (not `write()`) because the per-slot TTL bundle is
    /// itself locked; the per-slot lock provides exclusion against concurrent
    /// readers without serializing the entire `slots` vector.
    fn with_slot_ttls(
        &self,
        instrument_id: InstrumentId,
        mutate: impl FnOnce(&mut SlotTtls),
    ) -> Result<(), UnknownInstrumentId> {
        let slot_idx = {
            let guard = self.registry.read();
            guard
                .by_id
                .get(&instrument_id)
                .copied()
                .ok_or(UnknownInstrumentId { instrument_id })?
        };
        let guard = self.slots.read();
        if let Some(slot) = guard.get(slot_idx as usize) {
            let mut ttls = slot.ttls.write();
            mutate(&mut ttls);
        }
        Ok(())
    }

    /// Clears every stored quote for `instrument_id` across all three buckets.
    ///
    /// After [`clear`](Self::clear), reads return no quote until the next push.
    /// The instrument remains in the registry and its slot is not freed;
    /// subsequent pushes on the same id work normally. No-op if `instrument_id`
    /// is not registered.
    pub fn clear(&self, instrument_id: InstrumentId) {
        let slot_idx = {
            let guard = self.registry.read();
            match guard.by_id.get(&instrument_id).copied() {
                Some(idx) => idx,
                None => return,
            }
        };
        let guard = self.slots.read();
        if let Some(slot) = guard.get(slot_idx as usize) {
            let mut quotes = slot.quotes.write();
            quotes.accounts.clear();
            quotes.groups.clear();
        }
    }

    // ── Push by id (default bucket) ───────────────────────────────────────────

    /// Publishes a new quote for `id` into the default ("everyone-else")
    /// bucket, **replacing** the entire stored snapshot.
    ///
    /// Every field of `quote` (including `None` fields) becomes the new stored
    /// value; any field previously set on the default bucket but absent from
    /// `quote` is cleared. The publish instant is bumped to the current time.
    ///
    /// # Errors
    ///
    /// Returns [`UnknownInstrumentId`] if `instrument_id` has not been
    /// registered.
    ///
    /// Use [`push_patch`](Self::push_patch) to overwrite only the fields you
    /// have new values for, or [`push_for`](Self::push_for) to target specific
    /// accounts/groups.
    pub fn push(
        &self,
        instrument_id: InstrumentId,
        quote: Quote,
    ) -> Result<(), UnknownInstrumentId> {
        self.store_default_by_id(instrument_id, |_prev| quote)
    }

    /// Publishes a partial update for `instrument_id` into the default
    /// ("everyone-else") bucket, **merging** it into the existing snapshot.
    ///
    /// For each field of `quote`: `Some` overwrites the prior value, `None`
    /// leaves it intact. If the default bucket was empty the patch becomes the
    /// new snapshot as-is. The publish instant is bumped to the current time.
    ///
    /// # Errors
    ///
    /// Returns [`UnknownInstrumentId`] if `instrument_id` has not been
    /// registered.
    pub fn push_patch(
        &self,
        instrument_id: InstrumentId,
        quote: Quote,
    ) -> Result<(), UnknownInstrumentId> {
        self.store_default_by_id(instrument_id, |prev| {
            prev.unwrap_or_default().patched_with(quote)
        })
    }

    fn store_default_by_id(
        &self,
        instrument_id: InstrumentId,
        build: impl FnOnce(Option<Quote>) -> Quote,
    ) -> Result<(), UnknownInstrumentId> {
        let slot_idx = self.slot_for_id(instrument_id)?;
        let now = Instant::now();
        let guard = self.slots.read();
        if let Some(slot) = guard.get(slot_idx as usize) {
            let mut quotes = slot.quotes.write();
            store_into(&mut quotes.groups, DEFAULT_ACCOUNT_GROUP, now, build);
        }
        Ok(())
    }

    // ── Targeted fan-out push by id ───────────────────────────────────────────

    /// Publishes a new quote for `id` into the per-account bucket of every
    /// account in `accounts` and the per-group bucket of every group in
    /// `groups`, **replacing** each target's snapshot.
    ///
    /// All targets share one `pushed_at` instant. The default bucket is
    /// targetable by listing [`DEFAULT_ACCOUNT_GROUP`] in `groups`.
    ///
    /// # Errors
    ///
    /// - [`PushForError::UnknownInstrument`] if `instrument_id` is not
    ///   registered.
    /// - [`PushForError::NoTarget`] if both `account_ids` and
    ///   `account_group_ids` are empty — this is a caller bug; use
    ///   [`push`](Self::push) for the no-target case.
    pub fn push_for(
        &self,
        instrument_id: InstrumentId,
        quote: Quote,
        account_ids: &[AccountId],
        account_group_ids: &[AccountGroupId],
    ) -> Result<(), PushForError> {
        self.store_for(instrument_id, account_ids, account_group_ids, |_prev| quote)
    }

    /// Publishes a partial update for `instrument_id` into the per-account
    /// bucket of every account in `account_ids` and the per-group bucket of
    /// every group in `account_group_ids`, **merging** independently into each
    /// target's existing snapshot.
    ///
    /// All targets share one `pushed_at` instant. The default bucket is
    /// targetable by listing [`DEFAULT_ACCOUNT_GROUP`] in `account_group_ids`.
    ///
    /// # Errors
    ///
    /// - [`PushForError::UnknownInstrument`] if `instrument_id` is not
    ///   registered.
    /// - [`PushForError::NoTarget`] if both `account_ids` and
    ///   `account_group_ids` are empty.
    pub fn push_for_patch(
        &self,
        instrument_id: InstrumentId,
        quote: Quote,
        account_ids: &[AccountId],
        account_group_ids: &[AccountGroupId],
    ) -> Result<(), PushForError> {
        self.store_for(instrument_id, account_ids, account_group_ids, |prev| {
            prev.unwrap_or_default().patched_with(quote)
        })
    }

    fn store_for(
        &self,
        instrument_id: InstrumentId,
        account_ids: &[AccountId],
        account_group_ids: &[AccountGroupId],
        build: impl Fn(Option<Quote>) -> Quote,
    ) -> Result<(), PushForError> {
        if account_ids.is_empty() && account_group_ids.is_empty() {
            return Err(PushForError::NoTarget);
        }
        let slot_idx = {
            let guard = self.registry.read();
            guard
                .by_id
                .get(&instrument_id)
                .copied()
                .ok_or(PushForError::UnknownInstrument { instrument_id })?
        };
        let now = Instant::now();
        let guard = self.slots.read();
        if let Some(slot) = guard.get(slot_idx as usize) {
            let mut quotes = slot.quotes.write();
            for &account_id in account_ids {
                store_into(&mut quotes.accounts, account_id, now, &build);
            }
            for &account_group_id in account_group_ids {
                store_into(&mut quotes.groups, account_group_id, now, &build);
            }
        }
        Ok(())
    }

    // ── Push by instrument name (default bucket) ──────────────────────────────

    /// Publishes a new quote for `instrument` into the default bucket
    /// (replace semantics).
    ///
    /// If `instrument` has not been registered before, a named slot is created
    /// (inheriting the service-default TTL) and the new id is returned. If
    /// `instrument` is already registered its existing id is reused.
    ///
    /// There is no account/group targeting on the by-instrument path; use
    /// [`push_for`](Self::push_for) for that.
    pub fn push_by_instrument(&self, instrument: &Instrument, quote: Quote) -> InstrumentId {
        let (instrument_id, slot_idx) = self.resolve_or_register_named(instrument);
        self.store_default_at(slot_idx, |_prev| quote);
        instrument_id
    }

    /// Publishes a partial update for `instrument` into the default bucket
    /// (patch semantics).
    ///
    /// If `instrument` has not been registered before, a named slot is created
    /// (inheriting the service-default TTL) and the new id is returned. If
    /// `instrument` is already registered its existing id is reused.
    pub fn push_by_instrument_patch(&self, instrument: &Instrument, quote: Quote) -> InstrumentId {
        let (instrument_id, slot_idx) = self.resolve_or_register_named(instrument);
        self.store_default_at(slot_idx, |prev| {
            prev.unwrap_or_default().patched_with(quote)
        });
        instrument_id
    }

    // ── Get ───────────────────────────────────────────────────────────────────

    /// Returns the latest quote for `(instrument_id, account_id)` under
    /// `resolution`, or `None` if the id is unknown, no candidate quote exists,
    /// or the selected quote has aged past its effective TTL.
    ///
    /// `account_info` supplies the account's group for the group/default tiers;
    /// it is consulted lazily (only when the per-account bucket misses and the
    /// mode or TTL cascade needs the group).
    pub fn get(
        &self,
        instrument_id: InstrumentId,
        account_id: AccountId,
        account_info: &impl AccountInfo,
        resolution: QuoteResolution,
    ) -> Option<Quote> {
        self.get_or_err(instrument_id, account_id, account_info, resolution)
            .ok()
    }

    /// Returns the latest quote for `(instrument_id, account_id)` under
    /// `resolution`, distinguishing "not registered" from "no usable quote".
    ///
    /// Quote selection walks the buckets the `resolution` permits (per-account,
    /// then the account's group, then the default group), stopping at the first
    /// non-empty bucket. The selected quote's freshness is then checked against
    /// the TTL cascade for `(instrument_id, account_id, account_info.group())`
    /// — independent of which bucket the quote came from.
    ///
    /// # Errors
    ///
    /// - [`MarketDataError::UnknownInstrument`] - `instrument_id` is not
    ///   registered.
    /// - [`MarketDataError::QuoteUnavailable`] - registered but no usable quote
    ///   (no candidate in the consulted buckets, or the selected quote aged
    ///   past its effective TTL).
    pub fn get_or_err(
        &self,
        instrument_id: InstrumentId,
        account_id: AccountId,
        account_info: &impl AccountInfo,
        resolution: QuoteResolution,
    ) -> Result<Quote, MarketDataError> {
        let slot_idx = {
            let guard = self.registry.read();
            match guard.by_id.get(&instrument_id).copied() {
                Some(idx) => idx,
                None => return Err(MarketDataError::UnknownInstrument),
            }
        };
        let guard = self.slots.read();
        let slot = guard
            .get(slot_idx as usize)
            .ok_or(MarketDataError::QuoteUnavailable)?;

        // A single memoized group resolution shared by quote selection and the
        // TTL cascade, so `account_info.group()` runs at most once per read.
        let mut group_cell: Option<Option<AccountGroupId>> = None;

        let selected = {
            let quotes = slot.quotes.read();
            select_quote(
                &quotes,
                account_id,
                account_info,
                resolution,
                &mut group_cell,
            )
            .ok_or(MarketDataError::QuoteUnavailable)?
        };

        let effective_ttl = {
            let ttls = slot.ttls.read();
            self.effective_ttl(&ttls, account_id, account_info, &mut group_cell)
        };
        if let Some(ttl) = effective_ttl {
            if Instant::now().saturating_duration_since(selected.pushed_at) >= ttl {
                return Err(MarketDataError::QuoteUnavailable);
            }
        }
        Ok(selected.quote)
    }

    /// Walks the TTL cascade for `(slot, account_id, group)` and returns the
    /// effective lifetime: `Some(d)` for a finite deadline, `None` for "no
    /// expiry". See the type-level cascade documentation for the tier order.
    fn effective_ttl(
        &self,
        ttls: &SlotTtls,
        account_id: AccountId,
        account_info: &impl AccountInfo,
        group_cell: &mut Option<Option<AccountGroupId>>,
    ) -> Option<Duration> {
        // Tier 1: instrument × account.
        if let Some(setting) = ttls.accounts.get(&account_id) {
            return setting.as_duration();
        }
        // Tier 2: instrument × group (only when account_info has a group).
        if let Some(group) = resolve_group(account_info, group_cell) {
            if let Some(setting) = ttls.groups.get(&group) {
                return setting.as_duration();
            }
        }
        // Tier 3: instrument × default-group.
        if let Some(setting) = ttls.groups.get(&DEFAULT_ACCOUNT_GROUP) {
            return setting.as_duration();
        }
        // Tiers 4-6 live behind two service-wide locks. The gate pairs with the
        // setters' `mark_present`: if it reports "possibly present" the inserted
        // entries are visible; otherwise both maps are provably empty, so skip
        // both shared-lock acquisitions (the common no-service-TTL deployment)
        // and fall through to the instrument-level tier.
        if self.has_service_level_ttl.is_possibly_present() {
            // Tier 4: service-level account.
            if let Some(setting) = self.account_ttl.read().get(&account_id) {
                return setting.as_duration();
            }
            // Tiers 5-6: service-level group, then service-level default-group.
            let service_group_ttl = self.group_ttl.read();
            if let Some(group) = resolve_group(account_info, group_cell) {
                if let Some(setting) = service_group_ttl.get(&group) {
                    return setting.as_duration();
                }
            }
            if let Some(setting) = service_group_ttl.get(&DEFAULT_ACCOUNT_GROUP) {
                return setting.as_duration();
            }
        }
        // Tier 7: instrument-level (the registration TTL).
        if let Some(setting) = ttls.instrument {
            return setting.as_duration();
        }
        // Tier 8: the service-wide default.
        self.default_ttl
    }

    // ── Instrument resolution ─────────────────────────────────────────────────

    /// Resolves an `Instrument` to its `InstrumentId`, if registered by name.
    pub fn resolve(&self, instrument: &Instrument) -> Option<InstrumentId> {
        self.registry.read().by_instrument.get(instrument).copied()
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    /// Resolves `id` to its slot index, or [`UnknownInstrumentId`].
    fn slot_for_id(&self, instrument_id: InstrumentId) -> Result<u32, UnknownInstrumentId> {
        let guard = self.registry.read();
        guard
            .by_id
            .get(&instrument_id)
            .copied()
            .ok_or(UnknownInstrumentId { instrument_id })
    }

    /// Resolves `instrument` to `(id, slot_idx)`, registering a new named slot
    /// (inheriting the service-default TTL) if the name is unknown.
    ///
    /// Uses a double-checked locking pattern: the common fast path is a
    /// read-lock; only a miss promotes to a write-lock.
    fn resolve_or_register_named(&self, instrument: &Instrument) -> (InstrumentId, u32) {
        // Fast path: instrument is already registered.
        {
            let guard = self.registry.read();
            if let Some(&instrument_id) = guard.by_instrument.get(instrument) {
                let slot_idx = guard.by_id[&instrument_id];
                return (instrument_id, slot_idx);
            }
        }
        // Slow path: register a new named slot.
        let mut reg = self.registry.write();
        // Re-check under the write lock (another thread may have raced).
        if let Some(&instrument_id) = reg.by_instrument.get(instrument) {
            let slot_idx = reg.by_id[&instrument_id];
            return (instrument_id, slot_idx);
        }
        let instrument_id = reg.next_auto_id();
        let slot_idx = reg.alloc_slot();
        reg.by_instrument.insert(instrument.clone(), instrument_id);
        reg.insert_id(instrument_id, slot_idx);
        // `ensure_slot_storage` must be called while `registry.write()` is
        // still held so that `slot_idx == slots.len()` is maintained. The
        // named-push path never carries an instrument-level TTL, so the slot
        // inherits the service-default (cascade tier 8).
        self.ensure_slot_storage(slot_idx, None);
        (instrument_id, slot_idx)
    }

    /// Writes a quote into the default bucket of the slot at `slot_idx`.
    fn store_default_at(&self, slot_idx: u32, build: impl FnOnce(Option<Quote>) -> Quote) {
        let now = Instant::now();
        let guard = self.slots.read();
        if let Some(slot) = guard.get(slot_idx as usize) {
            let mut quotes = slot.quotes.write();
            store_into(&mut quotes.groups, DEFAULT_ACCOUNT_GROUP, now, build);
        }
    }

    /// Appends a new [`Slot`] to the `slots` vector.
    ///
    /// INVARIANT: `slot_idx` must equal `slots.len()` at the point of call
    /// (i.e. the slot index returned by `alloc_slot()` and the position in
    /// `slots` where the new `Slot` lands must agree). All calls to this
    /// method must happen while `registry.write()` is held so that
    /// `alloc_slot()` and `ensure_slot_storage()` are always paired within
    /// the same critical section.
    fn ensure_slot_storage(&self, slot_idx: u32, instrument_ttl: Option<TtlSetting>) {
        let mut slots = self.slots.write();
        debug_assert_eq!(
            slot_idx as usize,
            slots.len(),
            "slot index must match slots.len()"
        );
        slots.push(Slot::new(&self.sync, instrument_ttl));
    }
}

// ─── Free helpers ──────────────────────────────────────────────────────────────

/// Stores a built quote into `map[key]`, merging with any existing entry via
/// `build` and stamping `now` as the publish instant.
fn store_into<BucketKey: std::hash::Hash + Eq>(
    map: &mut HashMap<BucketKey, QuoteState>,
    key: BucketKey,
    now: Instant,
    build: impl FnOnce(Option<Quote>) -> Quote,
) {
    let prev = map.get(&key).map(|state| state.quote);
    map.insert(
        key,
        QuoteState {
            quote: build(prev),
            pushed_at: now,
        },
    );
}

/// Resolves and memoizes the account group for the duration of one read.
///
/// `account_info.group()` is invoked at most once: the first call fills
/// `group_cell`, every later call returns the cached value.
fn resolve_group(
    account_info: &impl AccountInfo,
    group_cell: &mut Option<Option<AccountGroupId>>,
) -> Option<AccountGroupId> {
    *group_cell.get_or_insert_with(|| account_info.group())
}

/// Selects the candidate [`QuoteState`] for `(account_id, group)` under
/// `resolution`, walking the per-account, group, and default buckets in order
/// and returning the first hit. The group is resolved lazily through
/// `group_cell` only when the per-account bucket misses and the mode needs it.
fn select_quote(
    quotes: &SlotQuotes,
    account_id: AccountId,
    account_info: &impl AccountInfo,
    resolution: QuoteResolution,
    group_cell: &mut Option<Option<AccountGroupId>>,
) -> Option<QuoteState> {
    if let Some(state) = quotes.accounts.get(&account_id) {
        return Some(*state);
    }
    match resolution {
        QuoteResolution::AccountOnly => None,
        QuoteResolution::AccountThenGroup => resolve_group(account_info, group_cell)
            .and_then(|group| quotes.groups.get(&group).copied()),
        QuoteResolution::AccountThenGroupThenDefault => {
            if let Some(group) = resolve_group(account_info, group_cell) {
                if let Some(state) = quotes.groups.get(&group) {
                    return Some(*state);
                }
            }
            quotes.groups.get(&DEFAULT_ACCOUNT_GROUP).copied()
        }
    }
}
