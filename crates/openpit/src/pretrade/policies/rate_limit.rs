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
// Please see https://openpit.dev and the OWNERS file for details.

use std::collections::{HashMap, VecDeque};
use std::fmt::{Display, Formatter};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::core::{HasAccountId, HasInstrument};
use crate::param::{AccountId, Asset};
use crate::pretrade::policy::{missing_required_field_reject, PolicyGroupId, PolicyName};
use crate::pretrade::start_pre_trade_time::start_pre_trade_now;
use crate::pretrade::ConfigurablePolicy;
use crate::pretrade::DEFAULT_POLICY_GROUP_ID;
use crate::pretrade::{PreTradeContext, PreTradePolicy, Reject, RejectCode, RejectScope, Rejects};
use crate::storage::{ConfigCell, Storage, StorageBuilder};

type StoragePolicy<LPF> = <LPF as crate::storage::LockingPolicyFactory>::Policy;
type TimestampStorage<K, LPF> = Storage<K, VecDeque<Instant>, StoragePolicy<LPF>>;

// Single source of truth for the policy name, shared by the associated
// `RateLimitPolicy::NAME` const and the free reject builder.
const RATE_LIMIT_POLICY_NAME: &str = "RateLimitPolicy";

/// Core rate-limit configuration shared by all barrier wrapper types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateLimit {
    /// Maximum number of orders accepted within `window`.
    pub max_orders: usize,
    /// Duration of the rolling time window.
    pub window: Duration,
}

/// Broker-wide rate-limit barrier.
///
/// Applies to every order regardless of account and settlement asset.
/// Shares a single approximate fixed-window counter across all callers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateLimitBrokerBarrier {
    /// Rate-limit configuration for this broker barrier.
    pub limit: RateLimit,
}

/// Per-settlement-asset rate-limit barrier.
///
/// Applies to every order whose settlement asset matches `settlement_asset`,
/// shared across all accounts trading that asset.
/// Uses the same approximate fixed-window counter as the broker barrier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateLimitAssetBarrier {
    /// Rate-limit configuration for this asset barrier.
    pub limit: RateLimit,
    /// Settlement asset this barrier applies to.
    pub settlement_asset: Asset,
}

/// Per-account rate-limit barrier.
///
/// Applies to every order from `account_id`, regardless of settlement asset.
/// Uses a precise sliding-window log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateLimitAccountBarrier {
    /// Rate-limit configuration for this account barrier.
    pub limit: RateLimit,
    /// Account this barrier applies to.
    pub account_id: AccountId,
}

/// Per-(account, settlement-asset) rate-limit barrier.
///
/// Applies to orders matching both `account_id` and `settlement_asset`.
/// Uses a precise sliding-window log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateLimitAccountAssetBarrier {
    /// Rate-limit configuration for this account+asset barrier.
    pub limit: RateLimit,
    /// Account this barrier applies to.
    pub account_id: AccountId,
    /// Settlement asset this barrier applies to.
    pub settlement_asset: Asset,
}

/// Errors returned by [`RateLimitPolicy`] construction and by the
/// runtime setters on [`RateLimitSettings`].
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RateLimitPolicyError {
    /// No barriers were provided across all four axes.
    NoBarriersConfigured,
    /// A barrier window is zero or exceeds the maximum representable
    /// nanoseconds.
    InvalidWindow { window: Duration },
}

impl Display for RateLimitPolicyError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoBarriersConfigured => write!(
                f,
                "at least one broker, asset, account, or account+asset barrier must be configured"
            ),
            Self::InvalidWindow { window } => write!(
                f,
                "rate limit window must be positive and fit in u64 nanoseconds, got {window:?}"
            ),
        }
    }
}

impl std::error::Error for RateLimitPolicyError {}

// Limit plus the live fixed-window counter enforcing it. The counter is
// behind `Arc` so a transactional settings clone keeps pushing into the
// same window, and a replace-shaped setter can carry it over for keys
// that survive the replacement.
#[derive(Debug, Clone)]
struct RateLimitSlot {
    counter: Arc<AtomicWindowCounter>,
    limit: RateLimit,
}

/// Runtime-updatable rate-limit configuration for [`RateLimitPolicy`].
///
/// Holds the limit configuration for all four axes. It is the only state
/// behind the policy's [`ConfigCell`]. The broker and asset axes carry their
/// live fixed-window counters here, behind `Arc`, so the hot path reads limits
/// and mutates counters under a single settings-cell read. The account axes
/// keep no slot: their sliding-window logs live in the policy's storages, keyed
/// lazily per account.
///
/// All four axes are replace-shaped and fully runtime-updatable: a setter can
/// add, remove, or retune any barrier on its axis. A key that survives a
/// replacement keeps its live counter (`Arc::clone`), so a retune never resets
/// the window and flood protection persists across it. The account-axis
/// sliding logs survive replacements by construction, since they live in the
/// policy keyed per account rather than in these settings; a removed account
/// key leaves its idle log behind (bounded by its window content) and the log
/// resumes if the key is re-added.
///
/// # Dynamic limits (no counter rebuild)
///
/// Every axis honors an add, remove, or limit/window change *going forward*,
/// from the next order onward, with no counter reset on surviving keys:
///
/// * Broker and asset axes use approximate fixed-window counters that read
///   their window from these settings on each push; changing the window
///   only shifts when the current window is considered expired.
/// * Account and account+asset axes use precise sliding-window logs that
///   trim by the configured window on each push, so a new window applies
///   immediately to the existing log.
#[derive(Debug, Clone)]
pub struct RateLimitSettings {
    asset_limits: HashMap<Asset, RateLimitSlot>,
    account_limits: HashMap<AccountId, RateLimit>,
    account_asset_limits: HashMap<(AccountId, Asset), RateLimit>,
    broker: Option<RateLimitSlot>,
}

impl RateLimitSettings {
    /// Builds a validated rate-limit configuration.
    ///
    /// At least one barrier must be provided across all four axes. If all
    /// are `None` or empty, returns
    /// [`RateLimitPolicyError::NoBarriersConfigured`]. Each window is
    /// validated; a zero or oversized window returns
    /// [`RateLimitPolicyError::InvalidWindow`]. Duplicate keys within an
    /// axis: last-write-wins (no error).
    ///
    pub fn new(
        broker: Option<RateLimitBrokerBarrier>,
        asset_barriers: impl IntoIterator<Item = RateLimitAssetBarrier>,
        account_barriers: impl IntoIterator<Item = RateLimitAccountBarrier>,
        account_asset_barriers: impl IntoIterator<Item = RateLimitAccountAssetBarrier>,
    ) -> Result<Self, RateLimitPolicyError> {
        let broker = broker
            .map(|b| validate_limit(b.limit).map(fresh_slot))
            .transpose()?;

        let asset_limits: HashMap<Asset, RateLimitSlot> = asset_barriers
            .into_iter()
            .map(|b| validate_limit(b.limit).map(|limit| (b.settlement_asset, fresh_slot(limit))))
            .collect::<Result<_, _>>()?;

        let account_limits: HashMap<AccountId, RateLimit> = account_barriers
            .into_iter()
            .map(|b| validate_limit(b.limit).map(|limit| (b.account_id, limit)))
            .collect::<Result<_, _>>()?;

        let account_asset_limits: HashMap<(AccountId, Asset), RateLimit> = account_asset_barriers
            .into_iter()
            .map(|b| {
                validate_limit(b.limit).map(|limit| ((b.account_id, b.settlement_asset), limit))
            })
            .collect::<Result<_, _>>()?;

        if broker.is_none()
            && asset_limits.is_empty()
            && account_limits.is_empty()
            && account_asset_limits.is_empty()
        {
            return Err(RateLimitPolicyError::NoBarriersConfigured);
        }

        Ok(Self {
            asset_limits,
            account_limits,
            account_asset_limits,
            broker,
        })
    }

    /// Replaces the broker barrier (add, remove, or retune).
    ///
    /// A surviving broker keeps its live counter, so a retune never resets the
    /// window. Passing `None` removes the barrier.
    ///
    /// # Errors
    ///
    /// [`RateLimitPolicyError::InvalidWindow`] if `limit.window` is zero or
    /// oversized; [`RateLimitPolicyError::NoBarriersConfigured`] if the change
    /// would leave all axes empty. On error `self` is left untouched.
    pub fn set_broker(
        &mut self,
        broker: Option<RateLimitBrokerBarrier>,
    ) -> Result<(), RateLimitPolicyError> {
        let next = match broker {
            Some(b) => {
                let limit = validate_limit(b.limit)?;
                // Carry the live counter over so a retune keeps the window.
                let counter = match self.broker.as_ref() {
                    Some(slot) => Arc::clone(&slot.counter),
                    None => Arc::new(AtomicWindowCounter::new(0)),
                };
                Some(RateLimitSlot { counter, limit })
            }
            None => None,
        };

        if next.is_none()
            && self.asset_limits.is_empty()
            && self.account_limits.is_empty()
            && self.account_asset_limits.is_empty()
        {
            return Err(RateLimitPolicyError::NoBarriersConfigured);
        }

        self.broker = next;
        Ok(())
    }

    /// Replaces the full set of per-asset barriers (add, remove, or retune).
    ///
    /// Each surviving asset keeps its live counter, so a retune never resets
    /// the window; new keys start a fresh window. Duplicate keys within one
    /// call: last-write-wins.
    ///
    /// # Errors
    ///
    /// [`RateLimitPolicyError::InvalidWindow`] if any `limit.window` is zero or
    /// oversized; [`RateLimitPolicyError::NoBarriersConfigured`] if the change
    /// would leave all axes empty. On error `self` is left untouched.
    pub fn set_asset_barriers(
        &mut self,
        barriers: impl IntoIterator<Item = RateLimitAssetBarrier>,
    ) -> Result<(), RateLimitPolicyError> {
        let mut asset_limits: HashMap<Asset, RateLimitSlot> = HashMap::new();
        for barrier in barriers {
            let limit = validate_limit(barrier.limit)?;
            // Carry the counter over for a surviving asset; start fresh for a
            // new one.
            let counter = match self.asset_limits.get(&barrier.settlement_asset) {
                Some(slot) => Arc::clone(&slot.counter),
                None => Arc::new(AtomicWindowCounter::new(0)),
            };
            asset_limits.insert(barrier.settlement_asset, RateLimitSlot { counter, limit });
        }

        if self.broker.is_none()
            && asset_limits.is_empty()
            && self.account_limits.is_empty()
            && self.account_asset_limits.is_empty()
        {
            return Err(RateLimitPolicyError::NoBarriersConfigured);
        }

        self.asset_limits = asset_limits;
        Ok(())
    }

    /// Replaces the full set of per-account barriers (add, remove, or retune).
    ///
    /// The sliding-window logs live in the policy keyed per account, so they
    /// survive a replacement; a surviving account keeps counting without a
    /// reset. Duplicate keys within one call: last-write-wins.
    ///
    /// # Errors
    ///
    /// [`RateLimitPolicyError::InvalidWindow`] if any `limit.window` is zero or
    /// oversized; [`RateLimitPolicyError::NoBarriersConfigured`] if the change
    /// would leave all axes empty. On error `self` is left untouched.
    pub fn set_account_barriers(
        &mut self,
        barriers: impl IntoIterator<Item = RateLimitAccountBarrier>,
    ) -> Result<(), RateLimitPolicyError> {
        let mut account_limits: HashMap<AccountId, RateLimit> = HashMap::new();
        for barrier in barriers {
            account_limits.insert(barrier.account_id, validate_limit(barrier.limit)?);
        }

        if self.broker.is_none()
            && self.asset_limits.is_empty()
            && account_limits.is_empty()
            && self.account_asset_limits.is_empty()
        {
            return Err(RateLimitPolicyError::NoBarriersConfigured);
        }

        self.account_limits = account_limits;
        Ok(())
    }

    /// Replaces the full set of per-(account, asset) barriers (add, remove, or
    /// retune).
    ///
    /// The sliding-window logs live in the policy keyed per pair, so they
    /// survive a replacement; a surviving pair keeps counting without a reset.
    /// Duplicate keys within one call: last-write-wins.
    ///
    /// # Errors
    ///
    /// [`RateLimitPolicyError::InvalidWindow`] if any `limit.window` is zero or
    /// oversized; [`RateLimitPolicyError::NoBarriersConfigured`] if the change
    /// would leave all axes empty. On error `self` is left untouched.
    pub fn set_account_asset_barriers(
        &mut self,
        barriers: impl IntoIterator<Item = RateLimitAccountAssetBarrier>,
    ) -> Result<(), RateLimitPolicyError> {
        let mut account_asset_limits: HashMap<(AccountId, Asset), RateLimit> = HashMap::new();
        for barrier in barriers {
            account_asset_limits.insert(
                (barrier.account_id, barrier.settlement_asset),
                validate_limit(barrier.limit)?,
            );
        }

        if self.broker.is_none()
            && self.asset_limits.is_empty()
            && self.account_limits.is_empty()
            && account_asset_limits.is_empty()
        {
            return Err(RateLimitPolicyError::NoBarriersConfigured);
        }

        self.account_asset_limits = account_asset_limits;
        Ok(())
    }

    // Replaces every live counter (broker + each asset slot) with a fresh one,
    // detaching this Settings from any counters shared with a caller-held
    // clone. Each policy owns counters tied to its own epoch.
    fn rearm(&mut self) {
        if let Some(slot) = self.broker.as_mut() {
            slot.counter = Arc::new(AtomicWindowCounter::new(0));
        }
        for slot in self.asset_limits.values_mut() {
            slot.counter = Arc::new(AtomicWindowCounter::new(0));
        }
    }
}

// Pairs a validated limit with a fresh counter whose zeroed window_start makes
// the first push start a new window.
fn fresh_slot(limit: RateLimit) -> RateLimitSlot {
    RateLimitSlot {
        counter: Arc::new(AtomicWindowCounter::new(0)),
        limit,
    }
}

// Approximate fixed-window counter backed by two AtomicU64s.
//
// `window_start_nanos` holds nanoseconds elapsed since the epoch captured at
// policy construction. On window rollover, `window_start_nanos` is CAS-updated
// and `count` is reset to 1. At the window boundary, two concurrent threads can
// both observe the window as expired and both reset `count`; the observed burst
// can briefly reach up to `2 * max_orders` worst case. This is a deliberate
// trade-off for lock-free shared accounting.
//
// The counter holds no limit of its own: the window length is supplied on each
// `push` from the current settings, so a window change applies going forward
// with no reset. The counter lives inside the settings cell behind an `Arc`,
// shared with any transactional clone and carried across replace-shaped
// setters for surviving keys, so a retune never resets the live window.
#[derive(Debug)]
struct AtomicWindowCounter {
    count: AtomicU64,
    window_start_nanos: AtomicU64,
}

impl AtomicWindowCounter {
    fn new(now_nanos: u64) -> Self {
        Self {
            count: AtomicU64::new(0),
            window_start_nanos: AtomicU64::new(now_nanos),
        }
    }

    fn push(&self, now_nanos: u64, window_nanos: u64) -> u64 {
        let win_start = self.window_start_nanos.load(Ordering::Relaxed);
        if now_nanos.wrapping_sub(win_start) >= window_nanos
            && self
                .window_start_nanos
                .compare_exchange(win_start, now_nanos, Ordering::AcqRel, Ordering::Relaxed)
                .is_ok()
        {
            self.count.store(1, Ordering::Release);
            return 1;
        }
        self.count.fetch_add(1, Ordering::AcqRel) + 1
    }

    // Read-only counterpart of `push`: the count a `push` would return right
    // now, without mutating the counter. Used by the dry-run check so it can
    // report a would-be rate-limit breach without consuming budget. If the
    // window has rolled over the next push would reset to 1; otherwise it would
    // be the live count plus one.
    fn peek(&self, now_nanos: u64, window_nanos: u64) -> u64 {
        let win_start = self.window_start_nanos.load(Ordering::Relaxed);
        if now_nanos.wrapping_sub(win_start) >= window_nanos {
            return 1;
        }
        self.count.load(Ordering::Acquire) + 1
    }
}

/// Tracks order rates across four independent axes in time windows.
///
/// Four configurable barrier axes - broker (all orders), per settlement
/// asset, per account, and per (account, settlement asset). Every applicable
/// axis is incremented and checked on every call to `check_pre_trade_start`.
/// An order is rejected if any applicable axis breaches its `max_orders` over
/// its `window`. Every call, including rejected ones, consumes a slot in every
/// applicable axis so that flood attempts cannot bypass any counter.
///
/// Broker and asset axes use approximate fixed-window counters backed by
/// `AtomicU64` pairs. They are atomic and lock-free; at a window boundary, the
/// observed burst can briefly reach up to `2 * max_orders` worst case. Account
/// and account+asset axes use precise sliding-window logs via [`Storage`].
///
/// The limit configuration for all axes lives in [`RateLimitSettings`] behind
/// a sync-mode-aware [`ConfigCell`]; the hot path reads it through that cell
/// and the engine can add, remove, or retune barriers at runtime through
/// [`Engine::configure`](crate::Engine::configure). The broker and asset
/// fixed-window counters ride inside that cell behind `Arc`; the account-axis
/// sliding-window logs live in the policy's storages, keyed lazily per account.
///
/// Constructor rules:
/// - at least one barrier across all four axes must be configured;
/// - if all are omitted, [`RateLimitSettings::new`] returns
///   [`RateLimitPolicyError::NoBarriersConfigured`];
/// - duplicate keys within an axis: last-write-wins (no error).
///
/// # Examples
///
/// ```rust
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use std::time::Duration;
/// use openpit::param::{Asset, Price, Quantity, Side};
/// use openpit::pretrade::policies::{
///     RateLimit, RateLimitBrokerBarrier, RateLimitPolicy, RateLimitSettings,
/// };
/// use openpit::{Engine, Instrument};
/// use openpit::OrderOperation;
/// use openpit::param::TradeAmount;
///
/// let builder = Engine::builder::<OrderOperation, (), ()>().no_sync();
/// let settings = RateLimitSettings::new(
///     Some(RateLimitBrokerBarrier {
///         limit: RateLimit { max_orders: 2, window: Duration::from_secs(60) },
///     }),
///     [],
///     [],
///     [],
/// )?;
/// let policy = RateLimitPolicy::new(settings, builder.storage_builder());
/// let engine = builder
///     .pre_trade(policy)
///     .build()?;
///
/// let order = OrderOperation {
///     instrument: Instrument::new(Asset::new("AAPL")?, Asset::new("USD")?),
///     account_id: openpit::param::AccountId::from_u64(99224416),
///     side: Side::Buy,
///     trade_amount: TradeAmount::Quantity(Quantity::from_str("1")?),
///     price: Some(Price::from_str("100")?),
/// };
/// // Two orders are allowed within the window.
/// assert!(engine.start_pre_trade(order.clone()).is_ok());
/// assert!(engine.start_pre_trade(order.clone()).is_ok());
///
/// // Third order is rejected.
/// assert!(engine.start_pre_trade(order).is_err());
/// # Ok(())
/// # }
/// ```
pub struct RateLimitPolicy<LockingPolicyFactory>
where
    LockingPolicyFactory: crate::storage::LockingPolicyFactory,
{
    group_id: PolicyGroupId,
    epoch: Instant,
    settings:
        <LockingPolicyFactory as crate::storage::LockingPolicyFactory>::Config<RateLimitSettings>,
    per_account_timestamps: TimestampStorage<AccountId, LockingPolicyFactory>,
    per_account_asset_timestamps: TimestampStorage<(AccountId, Asset), LockingPolicyFactory>,
}

impl<LockingPolicyFactory> RateLimitPolicy<LockingPolicyFactory>
where
    LockingPolicyFactory: crate::storage::LockingPolicyFactory,
{
    /// Stable policy name.
    pub const NAME: &'static str = RATE_LIMIT_POLICY_NAME;

    /// Creates a rate-limit policy from a validated [`RateLimitSettings`].
    ///
    /// `storage_builder` must be obtained from the engine builder so that the
    /// per-account and per-(account, asset) timestamp storages share the
    /// factory type with the engine's synchronization mode. The sliding-window
    /// logs are created lazily per key on the hot path, so barriers added at
    /// runtime start counting immediately; the settings (including the broker
    /// and asset fixed-window counters) are moved into a [`ConfigCell`] for
    /// hot-path reads and runtime reconfiguration.
    pub fn new(
        mut settings: RateLimitSettings,
        storage_builder: &StorageBuilder<LockingPolicyFactory>,
    ) -> Self
    where
        LockingPolicyFactory: crate::storage::CreateStorageFor<AccountId>
            + crate::storage::CreateStorageFor<(AccountId, Asset)>,
    {
        let epoch = Instant::now();

        // Detach counters from any caller-held clones: each policy owns
        // counters tied to its own epoch.
        settings.rearm();

        let per_account_timestamps = storage_builder.create_for_bound_key();
        let per_account_asset_timestamps = storage_builder.create_for_bound_key();

        Self {
            group_id: DEFAULT_POLICY_GROUP_ID,
            epoch,
            settings: <LockingPolicyFactory as crate::storage::LockingPolicyFactory>::new_config(
                settings,
            ),
            per_account_timestamps,
            per_account_asset_timestamps,
        }
    }

    /// Assigns a group tag to this policy instance.
    ///
    /// See [`PolicyGroupId`] and [`DEFAULT_POLICY_GROUP_ID`] for details.
    pub fn with_policy_group_id(mut self, id: PolicyGroupId) -> Self {
        self.group_id = id;
        self
    }
}

impl<LockingPolicyFactory> PolicyName for RateLimitPolicy<LockingPolicyFactory>
where
    LockingPolicyFactory: crate::storage::LockingPolicyFactory,
{
    fn policy_name(&self) -> &str {
        Self::NAME
    }
}

impl<LockingPolicyFactory> ConfigurablePolicy<LockingPolicyFactory>
    for RateLimitPolicy<LockingPolicyFactory>
where
    LockingPolicyFactory: crate::storage::LockingPolicyFactory,
{
    type Settings = RateLimitSettings;

    fn settings_cell(
        &self,
    ) -> <LockingPolicyFactory as crate::storage::LockingPolicyFactory>::Config<RateLimitSettings>
    {
        self.settings.clone()
    }
}

impl<Order, ExecutionReport, AccountAdjustment, LockingPolicyFactory, Sync>
    PreTradePolicy<Order, ExecutionReport, AccountAdjustment, Sync>
    for RateLimitPolicy<LockingPolicyFactory>
where
    Order: HasAccountId + HasInstrument,
    LockingPolicyFactory:
        crate::storage::LockingPolicyFactory + crate::storage::CreateStorageFor<AccountId>,
    Sync: crate::core::SyncMode<StorageLockingPolicyFactory = LockingPolicyFactory>,
{
    fn name(&self) -> &str {
        Self::NAME
    }

    fn policy_group_id(&self) -> PolicyGroupId {
        self.group_id
    }

    #[allow(private_interfaces)]
    fn built_in_config_entry(&self) -> Option<crate::core::ConfigEntry<LockingPolicyFactory>> {
        Some(crate::core::ConfigEntry::RateLimit(
            crate::pretrade::ConfigurablePolicy::settings_cell(self),
        ))
    }

    fn check_pre_trade_start(
        &self,
        _ctx: &PreTradeContext<<Sync as crate::core::SyncMode>::StorageLockingPolicyFactory>,
        order: &Order,
    ) -> Result<(), Rejects> {
        let now = start_pre_trade_now();
        let now_nanos = now
            .checked_duration_since(self.epoch)
            .unwrap_or_default()
            .as_nanos() as u64;

        // One coherent settings read decides both applicability and the exact
        // live counter/log to push. ALL applicable axes are pushed before
        // resolving (flood semantics): every applicable axis must consume its
        // slot even when an earlier-priority axis already breaches, so a flood
        // cannot bypass a counter by tripping another. The breach is resolved
        // in priority order: broker -> asset -> account -> account+asset.
        self.settings.with(|s| -> Result<(), Rejects> {
            let needs_settlement = !s.asset_limits.is_empty() || !s.account_asset_limits.is_empty();
            let needs_account = !s.account_limits.is_empty() || !s.account_asset_limits.is_empty();

            let settlement_opt: Option<Asset> = if needs_settlement {
                Some(
                    order
                        .instrument()
                        .map_err(|e| {
                            Rejects::from(missing_required_field_reject(self, "instrument", &e))
                        })?
                        .settlement_asset()
                        .clone(),
                )
            } else {
                None
            };
            let account_id_opt: Option<AccountId> = if needs_account {
                Some(order.account_id().map_err(|e| {
                    Rejects::from(missing_required_field_reject(self, "account ID", &e))
                })?)
            } else {
                None
            };

            let broker = s.broker.as_ref().map(|slot| {
                let count = slot.counter.push(now_nanos, window_nanos(&slot.limit));
                over_limit(count, &slot.limit, RejectScope::Order, "broker barrier")
            });

            let asset = settlement_opt.as_ref().and_then(|settlement| {
                s.asset_limits.get(settlement).map(|slot| {
                    let count = slot.counter.push(now_nanos, window_nanos(&slot.limit));
                    over_limit(count, &slot.limit, RejectScope::Order, "asset barrier")
                })
            });

            let account = account_id_opt.and_then(|account_id| {
                s.account_limits.get(&account_id).map(|limit| {
                    let count = self.per_account_timestamps.with_mut(
                        account_id,
                        VecDeque::new,
                        |entry, _is_new| {
                            advance_window(entry, now, limit.window);
                            entry.push_back(now);
                            entry.len() as u64
                        },
                    );
                    over_limit(count, limit, RejectScope::Account, "account barrier")
                })
            });

            let account_asset = account_id_opt.and_then(|account_id| {
                settlement_opt.as_ref().and_then(|settlement| {
                    let key = (account_id, settlement.clone());
                    s.account_asset_limits.get(&key).map(|limit| {
                        let count = self.per_account_asset_timestamps.with_mut(
                            key,
                            VecDeque::new,
                            |entry, _is_new| {
                                advance_window(entry, now, limit.window);
                                entry.push_back(now);
                                entry.len() as u64
                            },
                        );
                        over_limit(count, limit, RejectScope::Account, "account+asset barrier")
                    })
                })
            });

            match broker
                .flatten()
                .or_else(|| asset.flatten())
                .or_else(|| account.flatten())
                .or_else(|| account_asset.flatten())
            {
                Some(reject) => Err(reject),
                None => Ok(()),
            }
        })
    }

    /// Dry-run start-stage check.
    ///
    /// Rate limiting is an immediate-side-effect policy: the normal
    /// [`check_pre_trade_start`](Self::check_pre_trade_start) consumes a slot in
    /// every applicable window counter, and rejected attempts are never rolled
    /// back. A dry-run must not move that state, so this override consumes no
    /// budget and pushes nothing into any counter. It still reports the verdict
    /// a real call would produce: each axis is *peeked* read-only for the count
    /// a push would yield, so an order that would breach an already-exhausted
    /// window is rejected here too, with no side effect.
    fn check_pre_trade_start_dry_run(
        &self,
        _ctx: &PreTradeContext<<Sync as crate::core::SyncMode>::StorageLockingPolicyFactory>,
        order: &Order,
    ) -> Result<(), Rejects> {
        let now = start_pre_trade_now();
        let now_nanos = now
            .checked_duration_since(self.epoch)
            .unwrap_or_default()
            .as_nanos() as u64;

        // Read-only mirror of `check_pre_trade_start`: same axes, same priority
        // resolution, but every count is the would-be count from a peek rather
        // than a mutating push, so no budget is consumed.
        self.settings.with(|s| -> Result<(), Rejects> {
            let needs_settlement = !s.asset_limits.is_empty() || !s.account_asset_limits.is_empty();
            let needs_account = !s.account_limits.is_empty() || !s.account_asset_limits.is_empty();

            let settlement_opt: Option<Asset> = if needs_settlement {
                Some(
                    order
                        .instrument()
                        .map_err(|e| {
                            Rejects::from(missing_required_field_reject(self, "instrument", &e))
                        })?
                        .settlement_asset()
                        .clone(),
                )
            } else {
                None
            };
            let account_id_opt: Option<AccountId> = if needs_account {
                Some(order.account_id().map_err(|e| {
                    Rejects::from(missing_required_field_reject(self, "account ID", &e))
                })?)
            } else {
                None
            };

            let broker = s.broker.as_ref().map(|slot| {
                let count = slot.counter.peek(now_nanos, window_nanos(&slot.limit));
                over_limit(count, &slot.limit, RejectScope::Order, "broker barrier")
            });

            let asset = settlement_opt.as_ref().and_then(|settlement| {
                s.asset_limits.get(settlement).map(|slot| {
                    let count = slot.counter.peek(now_nanos, window_nanos(&slot.limit));
                    over_limit(count, &slot.limit, RejectScope::Order, "asset barrier")
                })
            });

            let account = account_id_opt.and_then(|account_id| {
                s.account_limits.get(&account_id).map(|limit| {
                    let count = self
                        .per_account_timestamps
                        .with(&account_id, |entry| {
                            would_be_window_count(entry, now, limit.window)
                        })
                        .unwrap_or(1);
                    over_limit(count, limit, RejectScope::Account, "account barrier")
                })
            });

            let account_asset = account_id_opt.and_then(|account_id| {
                settlement_opt.as_ref().and_then(|settlement| {
                    let key = (account_id, settlement.clone());
                    s.account_asset_limits.get(&key).map(|limit| {
                        let count = self
                            .per_account_asset_timestamps
                            .with(&key, |entry| {
                                would_be_window_count(entry, now, limit.window)
                            })
                            .unwrap_or(1);
                        over_limit(count, limit, RejectScope::Account, "account+asset barrier")
                    })
                })
            });

            match broker
                .flatten()
                .or_else(|| asset.flatten())
                .or_else(|| account.flatten())
                .or_else(|| account_asset.flatten())
            {
                Some(reject) => Err(reject),
                None => Ok(()),
            }
        })
    }
}

fn window_nanos(limit: &RateLimit) -> u64 {
    // Validated at construction / on every setter to fit in u64 nanoseconds.
    limit.window.as_nanos() as u64
}

// Builds a reject if `count` breaches `limit`, else `None`. Keeps the
// limit read and the threshold comparison inside the single settings read.
fn over_limit(
    count: u64,
    limit: &RateLimit,
    scope: RejectScope,
    reason: &'static str,
) -> Option<Rejects> {
    (count > limit.max_orders as u64)
        .then(|| rate_limit_reject(scope, reason, count, limit.max_orders as u64, limit.window))
}

// `barrier` names the breached axis (e.g. "broker barrier"); the reject
// reason is "rate limit exceeded: <barrier>".
fn rate_limit_reject(
    scope: RejectScope,
    barrier: &'static str,
    count: u64,
    max_orders: u64,
    window: Duration,
) -> Rejects {
    Reject::new(
        RATE_LIMIT_POLICY_NAME,
        scope,
        RejectCode::RateLimitExceeded,
        format!("rate limit exceeded: {barrier}"),
        format!(
            "submitted {} orders in {:?} window, max allowed: {}",
            count, window, max_orders
        ),
    )
    .into()
}

fn validate_limit(limit: RateLimit) -> Result<RateLimit, RateLimitPolicyError> {
    if limit.window.is_zero() || limit.window.as_nanos() > u128::from(u64::MAX) {
        return Err(RateLimitPolicyError::InvalidWindow {
            window: limit.window,
        });
    }
    Ok(limit)
}

fn advance_window(timestamps: &mut VecDeque<Instant>, now: Instant, window: Duration) {
    while let Some(oldest) = timestamps.front().copied() {
        match now.checked_duration_since(oldest) {
            Some(elapsed) if elapsed >= window => {
                timestamps.pop_front();
            }
            _ => break,
        }
    }
}

// Read-only would-be count for a sliding-window axis: how many entries are
// still inside `window` as of `now`, plus one for the order being dry-run.
// Mirrors `advance_window` + `push_back` + `len` without mutating storage, so a
// dry-run can report a would-be breach without consuming budget.
fn would_be_window_count(timestamps: &VecDeque<Instant>, now: Instant, window: Duration) -> u64 {
    // An entry is expired exactly when `advance_window` would pop it: a
    // measurable elapsed time that has reached the window. Anything else (still
    // inside the window, or not in the past) is live.
    let live = timestamps
        .iter()
        .filter(
            |ts| !matches!(now.checked_duration_since(**ts), Some(elapsed) if elapsed >= window),
        )
        .count() as u64;
    live + 1
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use crate::core::{Instrument, OrderOperation};
    use crate::param::{AccountId, Asset, Quantity, Side, TradeAmount};
    use crate::pretrade::start_pre_trade_time::with_start_pre_trade_now;
    use crate::pretrade::{
        ConfigurablePolicy, PreTradeContext, PreTradePolicy, RejectCode, RejectScope, Rejects,
    };
    use crate::storage::{ConfigCell, NoLocking};

    use super::{
        RateLimit, RateLimitAccountAssetBarrier, RateLimitAccountBarrier, RateLimitAssetBarrier,
        RateLimitBrokerBarrier, RateLimitPolicy, RateLimitPolicyError, RateLimitSettings,
    };

    type TestPolicy = RateLimitPolicy<NoLocking>;

    fn test_builder() -> crate::SyncedEngineBuilder<OrderOperation, (), (), crate::LocalSync> {
        crate::Engine::builder().no_sync()
    }

    fn policy_from(settings: RateLimitSettings) -> TestPolicy {
        RateLimitPolicy::new(settings, test_builder().storage_builder())
    }

    // ── settings validation ─────────────────────────────────────────────────

    #[test]
    fn zero_window_rejected_by_settings() {
        let err = RateLimitSettings::new(
            Some(RateLimitBrokerBarrier {
                limit: RateLimit {
                    max_orders: 1,
                    window: Duration::ZERO,
                },
            }),
            [],
            [],
            [],
        )
        .expect_err("must fail");

        assert_eq!(
            err,
            RateLimitPolicyError::InvalidWindow {
                window: Duration::ZERO
            }
        );
        assert_eq!(
            err.to_string(),
            "rate limit window must be positive and fit in u64 nanoseconds, got 0ns"
        );
    }

    #[test]
    fn sub_microsecond_window_accepted_by_settings() {
        let result = RateLimitSettings::new(
            Some(RateLimitBrokerBarrier {
                limit: RateLimit {
                    max_orders: 1,
                    window: Duration::from_nanos(500),
                },
            }),
            [],
            [],
            [],
        );

        assert!(result.is_ok());
    }

    #[test]
    fn excessive_window_rejected_by_settings() {
        let max_plus_one = Duration::new(u64::MAX, 0) + Duration::from_nanos(1);
        let err = RateLimitSettings::new(
            Some(RateLimitBrokerBarrier {
                limit: RateLimit {
                    max_orders: 1,
                    window: max_plus_one,
                },
            }),
            [],
            [],
            [],
        )
        .expect_err("must fail");

        assert!(matches!(err, RateLimitPolicyError::InvalidWindow { .. }));
    }

    #[test]
    fn no_barriers_configured_rejected_by_settings() {
        let err = RateLimitSettings::new(None, [], [], []).expect_err("must fail");
        assert_eq!(err, RateLimitPolicyError::NoBarriersConfigured);
        assert_eq!(
            err.to_string(),
            "at least one broker, asset, account, or account+asset barrier must be configured"
        );
    }

    // ── runtime setters ──────────────────────────────────────────────────────

    #[test]
    fn set_broker_retunes_without_resetting_counter() {
        // With a generous limit of 5, push three orders, then replace the
        // broker with a tighter limit of 2 in the same window. If the retune
        // reset the counter the fourth order would pass; its rejection proves
        // the live counter carried over across the replacement.
        let policy = broker_policy(5, Duration::from_secs(10));
        let o = order(account(1));
        let base = Instant::now();

        assert!(check_at(&policy, &o, base).is_ok());
        assert!(check_at(&policy, &o, base + Duration::from_secs(1)).is_ok());
        assert!(check_at(&policy, &o, base + Duration::from_secs(2)).is_ok());

        policy
            .settings_cell()
            .update::<RateLimitPolicyError>(|s| {
                s.set_broker(Some(RateLimitBrokerBarrier {
                    limit: RateLimit {
                        max_orders: 2,
                        window: Duration::from_secs(10),
                    },
                }))
            })
            .expect("retune must publish");

        assert!(check_at(&policy, &o, base + Duration::from_secs(3)).is_err());
    }

    #[test]
    fn set_asset_barriers_adds_axis_at_runtime() {
        // Policy starts with only a broker barrier; a USD asset barrier is
        // added at runtime through the settings cell and immediately enforced.
        let policy = broker_policy(100, Duration::from_secs(60));
        let usd = Asset::new("USD").expect("asset code must be valid");
        policy
            .settings_cell()
            .update::<RateLimitPolicyError>(|s| {
                s.set_asset_barriers([RateLimitAssetBarrier {
                    settlement_asset: usd.clone(),
                    limit: RateLimit {
                        max_orders: 1,
                        window: Duration::from_secs(60),
                    },
                }])
            })
            .expect("add must publish");

        let base = Instant::now();
        assert!(check_at(&policy, &order(account(1)), base).is_ok());
        let reject = check_at(&policy, &order(account(2)), base + Duration::from_secs(1))
            .expect_err("second USD order must breach the new asset barrier");
        assert_eq!(reject[0].reason, "rate limit exceeded: asset barrier");
    }

    #[test]
    fn set_broker_none_removes_axis_when_another_remains() {
        // Broker + asset both configured; removing the broker leaves the asset
        // barrier live, so a flood that previously tripped the broker now only
        // trips the asset axis.
        let mut settings = asset_settings("USD", 100, Duration::from_secs(60));
        settings
            .set_broker(Some(RateLimitBrokerBarrier {
                limit: RateLimit {
                    max_orders: 1,
                    window: Duration::from_secs(60),
                },
            }))
            .expect("broker add must succeed");
        let policy = policy_from(settings);

        policy
            .settings_cell()
            .update::<RateLimitPolicyError>(|s| s.set_broker(None))
            .expect("remove must publish");

        let base = Instant::now();
        // Broker limit was 1; with it gone, a second order passes the (loose)
        // asset barrier instead of being rejected by the broker.
        assert!(check_at(&policy, &order(account(1)), base).is_ok());
        assert!(check_at(&policy, &order(account(2)), base + Duration::from_secs(1)).is_ok());
    }

    #[test]
    fn set_broker_none_clearing_last_axis_fails_and_keeps_config() {
        // Broker is the only axis; removing it would leave all axes empty, so
        // the setter rejects and the prior broker barrier stays live.
        let policy = broker_policy(1, Duration::from_secs(10));
        let err = policy
            .settings_cell()
            .update::<RateLimitPolicyError>(|s| s.set_broker(None))
            .expect_err("clearing the last axis must fail");
        assert_eq!(err, RateLimitPolicyError::NoBarriersConfigured);

        // The retained broker limit of 1 still applies.
        let base = Instant::now();
        assert!(check_at(&policy, &order(account(1)), base).is_ok());
        assert!(check_at(&policy, &order(account(1)), base + Duration::from_secs(1)).is_err());
    }

    #[test]
    fn set_asset_barriers_rejects_invalid_window_and_keeps_prior() {
        let mut settings = asset_settings("USD", 1, Duration::from_secs(10));
        let usd = Asset::new("USD").expect("asset code must be valid");
        let err = settings
            .set_asset_barriers([RateLimitAssetBarrier {
                settlement_asset: usd.clone(),
                limit: RateLimit {
                    max_orders: 1,
                    window: Duration::ZERO,
                },
            }])
            .expect_err("must fail");
        assert_eq!(
            err,
            RateLimitPolicyError::InvalidWindow {
                window: Duration::ZERO
            }
        );
        // A rejected setter must leave the prior limit untouched.
        assert_eq!(
            settings
                .asset_limits
                .get(&usd)
                .expect("usd set")
                .limit
                .window,
            Duration::from_secs(10)
        );
    }

    // ── runtime retune visible on the hot path ───────────────────────────────

    #[test]
    fn cell_update_retunes_broker_limit_going_forward() {
        let policy = broker_policy(5, Duration::from_secs(10));
        let o = order(account(1));
        let base = Instant::now();

        // Generous limit of 5: three orders in the window pass (count 1..=3).
        assert!(check_at(&policy, &o, base).is_ok());
        assert!(check_at(&policy, &o, base + Duration::from_secs(1)).is_ok());
        assert!(check_at(&policy, &o, base + Duration::from_secs(2)).is_ok());

        // Tighten to max 2 in the same window. The fourth order (count 4)
        // would have passed under the old limit of 5, so its rejection proves
        // the hot path reads the new limit from the cell.
        policy
            .settings_cell()
            .update::<RateLimitPolicyError>(|s| {
                s.set_broker(Some(RateLimitBrokerBarrier {
                    limit: RateLimit {
                        max_orders: 2,
                        window: Duration::from_secs(10),
                    },
                }))
            })
            .expect("retune must publish");
        assert!(check_at(&policy, &o, base + Duration::from_secs(3)).is_err());

        // Loosen the limit; the next window admits more orders again.
        policy
            .settings_cell()
            .update::<RateLimitPolicyError>(|s| {
                s.set_broker(Some(RateLimitBrokerBarrier {
                    limit: RateLimit {
                        max_orders: 10,
                        window: Duration::from_secs(10),
                    },
                }))
            })
            .expect("retune must publish");
        let later = base + Duration::from_secs(20);
        for i in 0..10 {
            assert!(check_at(&policy, &o, later + Duration::from_millis(i)).is_ok());
        }
    }

    #[test]
    fn settings_cell_clone_shares_underlying() {
        let policy = broker_policy(2, Duration::from_secs(10));
        let handle = policy.settings_cell();
        handle
            .update::<RateLimitPolicyError>(|s| {
                s.set_broker(Some(RateLimitBrokerBarrier {
                    limit: RateLimit {
                        max_orders: 7,
                        window: Duration::from_secs(10),
                    },
                }))
            })
            .expect("retune must publish");
        // The policy's own field observes the update done through the clone.
        assert_eq!(
            policy
                .settings
                .with(|s| s.broker.as_ref().map(|b| b.limit.max_orders)),
            Some(7)
        );
    }

    // ── broker barrier ─────────────────────────────────────────────────────

    #[test]
    fn sliding_window_rejects_when_broker_limit_is_exceeded() {
        let policy = broker_policy(2, Duration::from_secs(10));
        let o = order(account(1));
        let base = Instant::now();

        assert!(check_at(&policy, &o, base).is_ok());
        assert!(check_at(&policy, &o, base + Duration::from_secs(1)).is_ok());

        let reject = check_at(&policy, &o, base + Duration::from_secs(2))
            .expect_err("third order in window must be rejected");
        let reject = &reject[0];
        assert_eq!(reject.scope, RejectScope::Order);
        assert_eq!(reject.code, RejectCode::RateLimitExceeded);
        assert_eq!(reject.reason, "rate limit exceeded: broker barrier");
        assert_eq!(
            reject.details,
            "submitted 3 orders in 10s window, max allowed: 2"
        );
    }

    #[test]
    fn expired_timestamps_leave_broker_sliding_window() {
        let policy = broker_policy(2, Duration::from_secs(10));
        let o = order(account(1));
        let base = Instant::now();

        assert!(check_at(&policy, &o, base).is_ok());
        assert!(check_at(&policy, &o, base + Duration::from_secs(1)).is_ok());
        assert!(check_at(&policy, &o, base + Duration::from_secs(11)).is_ok());
    }

    #[test]
    fn rejected_broker_attempts_are_counted_and_not_rolled_back() {
        let policy = broker_policy(1, Duration::from_secs(3));
        let o = order(account(1));
        let base = Instant::now();

        assert!(check_at(&policy, &o, base).is_ok());
        assert!(check_at(&policy, &o, base + Duration::from_secs(1)).is_err());

        let reject = check_at(&policy, &o, base + Duration::from_millis(2500))
            .expect_err("rejected attempt must stay counted in the window");
        let reject = &reject[0];
        assert_eq!(reject.scope, RejectScope::Order);
        assert_eq!(reject.code, RejectCode::RateLimitExceeded);
        assert_eq!(reject.reason, "rate limit exceeded: broker barrier");
        assert_eq!(
            reject.details,
            "submitted 3 orders in 3s window, max allowed: 1"
        );
    }

    #[test]
    fn dry_run_consumes_no_broker_budget() {
        // Broker max is 1. A flood of dry-runs while the window has budget must
        // all pass and leave the counter untouched, so the subsequent real
        // calls behave exactly as if no dry-run ever happened: the first passes
        // (count 1), the second rejects (count 2 > 1).
        let policy = broker_policy(1, Duration::from_secs(10));
        let o = order(account(1));
        let base = Instant::now();

        for i in 0..50 {
            assert!(
                check_dry_run_at(&policy, &o, base + Duration::from_millis(i)).is_ok(),
                "dry-run on a window with budget must pass"
            );
        }

        assert!(check_at(&policy, &o, base + Duration::from_secs(1)).is_ok());
        assert!(check_at(&policy, &o, base + Duration::from_secs(2)).is_err());
    }

    #[test]
    fn dry_run_reports_would_be_breach_without_consuming_budget() {
        // Broker max is 1. After a real order exhausts the window, a dry-run
        // must report the would-be reject (read-only) yet still not consume
        // budget: a real order in a fresh window then passes as if no dry-run
        // ran.
        let policy = broker_policy(1, Duration::from_secs(10));
        let o = order(account(1));
        let base = Instant::now();

        // Real order consumes the only slot.
        assert!(check_at(&policy, &o, base).is_ok());

        // Budget is now exhausted; dry-run reports the breach a real call would
        // hit, with no side effect.
        let reject = check_dry_run_at(&policy, &o, base + Duration::from_secs(1))
            .expect_err("dry-run must report a would-be rate-limit breach");
        assert_eq!(reject[0].scope, RejectScope::Order);
        assert_eq!(reject[0].code, RejectCode::RateLimitExceeded);
        assert_eq!(reject[0].reason, "rate limit exceeded: broker barrier");

        // Repeating the dry-run still reports the breach and consumes nothing.
        assert!(check_dry_run_at(&policy, &o, base + Duration::from_secs(2)).is_err());

        // A real order in a fresh window passes: the dry-runs spent no budget.
        assert!(check_at(&policy, &o, base + Duration::from_secs(11)).is_ok());
    }

    #[test]
    fn broker_barrier_applies_to_all_accounts() {
        let policy = broker_policy(2, Duration::from_secs(10));
        let base = Instant::now();

        assert!(check_at(&policy, &order(account(1)), base).is_ok());
        assert!(check_at(&policy, &order(account(2)), base + Duration::from_secs(1)).is_ok());
        let reject = check_at(&policy, &order(account(3)), base + Duration::from_secs(2))
            .expect_err("third order across all accounts must be rejected");
        assert_eq!(reject[0].scope, RejectScope::Order);
        assert_eq!(reject[0].reason, "rate limit exceeded: broker barrier");
    }

    // ── asset barrier ──────────────────────────────────────────────────────

    #[test]
    fn asset_barrier_rejects_when_limit_is_exceeded_for_matching_settlement() {
        let policy = asset_policy("USD", 1, Duration::from_secs(10));
        let base = Instant::now();

        assert!(check_at(&policy, &order(account(1)), base).is_ok());

        let reject = check_at(&policy, &order(account(2)), base + Duration::from_secs(1))
            .expect_err("second USD order must be rejected by asset barrier");
        assert_eq!(reject[0].scope, RejectScope::Order);
        assert_eq!(reject[0].code, RejectCode::RateLimitExceeded);
        assert_eq!(reject[0].reason, "rate limit exceeded: asset barrier");
    }

    #[test]
    fn asset_barrier_ignores_non_matching_settlement() {
        let policy = asset_policy("EUR", 1, Duration::from_secs(10));
        let base = Instant::now();

        // Order uses USD, policy has EUR barrier — should pass regardless of count
        assert!(check_at(&policy, &order(account(1)), base).is_ok());
        assert!(check_at(&policy, &order(account(1)), base + Duration::from_secs(1)).is_ok());
    }

    // ── account barrier ─────────────────────────────────────────────────────

    #[test]
    fn account_barrier_rejects_when_limit_is_exceeded() {
        let policy = account_policy(account(1), 2, Duration::from_secs(10));
        let o = order(account(1));
        let base = Instant::now();

        assert!(check_at(&policy, &o, base).is_ok());
        assert!(check_at(&policy, &o, base + Duration::from_secs(1)).is_ok());

        let reject = check_at(&policy, &o, base + Duration::from_secs(2))
            .expect_err("third order for account must be rejected");
        let reject = &reject[0];
        assert_eq!(reject.scope, RejectScope::Account);
        assert_eq!(reject.code, RejectCode::RateLimitExceeded);
        assert_eq!(reject.reason, "rate limit exceeded: account barrier");
        assert_eq!(
            reject.details,
            "submitted 3 orders in 10s window, max allowed: 2"
        );
    }

    #[test]
    fn different_accounts_track_independently() {
        let policy = account_policy(account(1), 1, Duration::from_secs(10));
        let base = Instant::now();

        assert!(check_at(&policy, &order(account(1)), base).is_ok());
        // account(2) has no barrier — always passes.
        assert!(check_at(&policy, &order(account(2)), base + Duration::from_secs(1)).is_ok());
        assert!(check_at(&policy, &order(account(2)), base + Duration::from_secs(2)).is_ok());
        // account(1) exhausted its window.
        assert!(check_at(&policy, &order(account(1)), base + Duration::from_secs(3)).is_err());
    }

    #[test]
    fn account_without_barrier_passes_when_only_account_barriers_configured() {
        let policy = account_policy(account(1), 1, Duration::from_secs(10));
        let base = Instant::now();

        assert!(check_at(&policy, &order(account(2)), base).is_ok());
        assert!(check_at(&policy, &order(account(2)), base + Duration::from_secs(1)).is_ok());
    }

    #[test]
    fn broker_only_config_does_not_call_instrument() {
        let policy = broker_policy(10, Duration::from_secs(60));
        let order = NoInstrumentOrder {
            account_id: account(1),
        };

        let result = <TestPolicy as PreTradePolicy<
            NoInstrumentOrder,
            (),
            (),
            crate::core::LocalSync,
        >>::check_pre_trade_start(
            &policy, &PreTradeContext::<NoLocking>::new(None), &order
        );

        assert!(result.is_ok());
    }

    #[test]
    fn account_only_config_does_not_call_instrument() {
        let policy = account_policy(account(1), 10, Duration::from_secs(60));
        let order = NoInstrumentOrder {
            account_id: account(1),
        };

        let result = <TestPolicy as PreTradePolicy<
            NoInstrumentOrder,
            (),
            (),
            crate::core::LocalSync,
        >>::check_pre_trade_start(
            &policy, &PreTradeContext::<NoLocking>::new(None), &order
        );

        assert!(result.is_ok());
    }

    // ── account+asset barrier ───────────────────────────────────────────────

    #[test]
    fn account_asset_barrier_rejects_when_limit_is_exceeded() {
        let policy = account_asset_policy(account(1), "USD", 1, Duration::from_secs(10));
        let base = Instant::now();

        assert!(check_at(&policy, &order(account(1)), base).is_ok());

        let reject = check_at(&policy, &order(account(1)), base + Duration::from_secs(1))
            .expect_err("second order for account+USD must be rejected");
        let reject = &reject[0];
        assert_eq!(reject.scope, RejectScope::Account);
        assert_eq!(reject.code, RejectCode::RateLimitExceeded);
        assert_eq!(reject.reason, "rate limit exceeded: account+asset barrier");
    }

    #[test]
    fn account_asset_barrier_ignores_different_account() {
        let policy = account_asset_policy(account(1), "USD", 1, Duration::from_secs(10));
        let base = Instant::now();

        // account(2) has no account+asset barrier for USD — passes
        assert!(check_at(&policy, &order(account(2)), base).is_ok());
        assert!(check_at(&policy, &order(account(2)), base + Duration::from_secs(1)).is_ok());
    }

    // ── broker + account combined ───────────────────────────────────────────

    #[test]
    fn both_barriers_checked_and_account_barrier_triggers_after_broker() {
        let policy = policy_from(
            RateLimitSettings::new(
                Some(RateLimitBrokerBarrier {
                    limit: RateLimit {
                        max_orders: 3,
                        window: Duration::from_secs(10),
                    },
                }),
                [],
                [RateLimitAccountBarrier {
                    account_id: account(1),
                    limit: RateLimit {
                        max_orders: 1,
                        window: Duration::from_secs(10),
                    },
                }],
                [],
            )
            .expect("valid config"),
        );
        let base = Instant::now();

        assert!(check_at(&policy, &order(account(1)), base).is_ok());
        // Broker: 2/3, account(1): 2/1 → account barrier triggers.
        let reject = check_at(&policy, &order(account(1)), base + Duration::from_secs(1))
            .expect_err("account barrier must trigger");
        assert_eq!(reject[0].scope, RejectScope::Account);
        assert_eq!(reject[0].reason, "rate limit exceeded: account barrier");
    }

    #[test]
    fn broker_reject_reported_when_both_barriers_breach() {
        let policy = policy_from(
            RateLimitSettings::new(
                Some(RateLimitBrokerBarrier {
                    limit: RateLimit {
                        max_orders: 1,
                        window: Duration::from_secs(10),
                    },
                }),
                [],
                [RateLimitAccountBarrier {
                    account_id: account(1),
                    limit: RateLimit {
                        max_orders: 1,
                        window: Duration::from_secs(10),
                    },
                }],
                [],
            )
            .expect("valid config"),
        );
        let base = Instant::now();

        assert!(check_at(&policy, &order(account(1)), base).is_ok());
        // Both broker and account breach — broker is reported (checked first).
        let reject = check_at(&policy, &order(account(1)), base + Duration::from_secs(1))
            .expect_err("must reject");
        assert_eq!(reject[0].scope, RejectScope::Order);
        assert_eq!(reject[0].reason, "rate limit exceeded: broker barrier");
    }

    // ── group id ──────────────────────────────────────────────────────────────

    #[test]
    fn group_id_is_carried_from_policy_builder() {
        use crate::pretrade::PolicyGroupId;

        let group = PolicyGroupId::new(7);
        let settings = RateLimitSettings::new(
            Some(RateLimitBrokerBarrier {
                limit: RateLimit {
                    max_orders: 1,
                    window: Duration::from_secs(10),
                },
            }),
            [],
            [],
            [],
        )
        .expect("valid config");
        let policy = policy_from(settings).with_policy_group_id(group);

        let observed = <TestPolicy as PreTradePolicy<
            OrderOperation,
            (),
            (),
            crate::core::LocalSync,
        >>::policy_group_id(&policy);
        assert_eq!(observed, group);
    }

    // ── field error path ─────────────────────────────────────────────────────

    #[test]
    fn account_id_access_error_rejects_with_missing_required_field() {
        struct NoAccountId;

        impl crate::HasAccountId for NoAccountId {
            fn account_id(&self) -> Result<AccountId, crate::RequestFieldAccessError> {
                Err(crate::RequestFieldAccessError::new("account_id"))
            }
        }

        impl crate::HasInstrument for NoAccountId {
            fn instrument(
                &self,
            ) -> Result<&crate::core::Instrument, crate::RequestFieldAccessError> {
                Err(crate::RequestFieldAccessError::new("instrument"))
            }
        }

        let policy = account_policy(account(1), 10, Duration::from_secs(60));
        let reject = <TestPolicy as PreTradePolicy<NoAccountId, (), (), crate::core::LocalSync>>::check_pre_trade_start(
            &policy,
            &PreTradeContext::<NoLocking>::new(None),
            &NoAccountId,
        )
        .expect_err("missing account_id must reject");
        let reject = &reject[0];
        assert_eq!(reject.scope, RejectScope::Order);
        assert_eq!(reject.code, RejectCode::MissingRequiredField);
        assert_eq!(
            reject.reason,
            "failed to access required field 'account ID'"
        );
        assert_eq!(reject.details, "failed to access field 'account_id'");
    }

    #[test]
    fn broker_only_config_does_not_require_account_id() {
        struct NoAccountId;

        impl crate::HasAccountId for NoAccountId {
            fn account_id(&self) -> Result<AccountId, crate::RequestFieldAccessError> {
                Err(crate::RequestFieldAccessError::new("account_id"))
            }
        }

        impl crate::HasInstrument for NoAccountId {
            fn instrument(
                &self,
            ) -> Result<&crate::core::Instrument, crate::RequestFieldAccessError> {
                Err(crate::RequestFieldAccessError::new("instrument"))
            }
        }

        let policy = broker_policy(10, Duration::from_secs(60));
        assert!(
            <TestPolicy as PreTradePolicy<NoAccountId, (), (), crate::core::LocalSync>>::check_pre_trade_start(
                &policy,
                &PreTradeContext::<NoLocking>::new(None),
                &NoAccountId,
            )
            .is_ok()
        );
    }

    #[test]
    fn asset_axis_with_no_instrument_returns_missing_required_field() {
        let policy = asset_policy("USD", 10, Duration::from_secs(60));
        let order = NoInstrumentOrder {
            account_id: account(1),
        };

        let reject = <TestPolicy as PreTradePolicy<
            NoInstrumentOrder,
            (),
            (),
            crate::core::LocalSync,
        >>::check_pre_trade_start(
            &policy, &PreTradeContext::<NoLocking>::new(None), &order
        )
        .expect_err("asset-axis policy must require instrument");
        let reject = &reject[0];
        assert_eq!(reject.scope, RejectScope::Order);
        assert_eq!(reject.code, RejectCode::MissingRequiredField);
        assert_eq!(
            reject.reason,
            "failed to access required field 'instrument'"
        );
        assert_eq!(reject.details, "failed to access field 'instrument'");
    }

    // ── helpers ─────────────────────────────────────────────────────────────

    struct NoInstrumentOrder {
        account_id: AccountId,
    }

    impl crate::HasAccountId for NoInstrumentOrder {
        fn account_id(&self) -> Result<AccountId, crate::RequestFieldAccessError> {
            Ok(self.account_id)
        }
    }

    impl crate::HasInstrument for NoInstrumentOrder {
        fn instrument(&self) -> Result<&Instrument, crate::RequestFieldAccessError> {
            Err(crate::RequestFieldAccessError::new("instrument"))
        }
    }

    fn check_at(policy: &TestPolicy, order: &OrderOperation, now: Instant) -> Result<(), Rejects> {
        with_start_pre_trade_now(now, || {
            <TestPolicy as PreTradePolicy<OrderOperation, (), (), crate::core::LocalSync>>::check_pre_trade_start(
                policy,
                &PreTradeContext::<NoLocking>::new(None),
                order,
            )
        })
    }

    fn check_dry_run_at(
        policy: &TestPolicy,
        order: &OrderOperation,
        now: Instant,
    ) -> Result<(), Rejects> {
        with_start_pre_trade_now(now, || {
            <TestPolicy as PreTradePolicy<OrderOperation, (), (), crate::core::LocalSync>>::check_pre_trade_start_dry_run(
                policy,
                &PreTradeContext::<NoLocking>::new(None),
                order,
            )
        })
    }

    fn broker_settings(max_orders: usize, window: Duration) -> RateLimitSettings {
        RateLimitSettings::new(
            Some(RateLimitBrokerBarrier {
                limit: RateLimit { max_orders, window },
            }),
            [],
            [],
            [],
        )
        .expect("valid config")
    }

    fn broker_policy(max_orders: usize, window: Duration) -> TestPolicy {
        policy_from(broker_settings(max_orders, window))
    }

    fn asset_settings(settlement: &str, max_orders: usize, window: Duration) -> RateLimitSettings {
        RateLimitSettings::new(
            None,
            [RateLimitAssetBarrier {
                limit: RateLimit { max_orders, window },
                settlement_asset: Asset::new(settlement).expect("asset code must be valid"),
            }],
            [],
            [],
        )
        .expect("valid config")
    }

    fn asset_policy(settlement: &str, max_orders: usize, window: Duration) -> TestPolicy {
        policy_from(asset_settings(settlement, max_orders, window))
    }

    fn account_settings(
        account_id: AccountId,
        max_orders: usize,
        window: Duration,
    ) -> RateLimitSettings {
        RateLimitSettings::new(
            None,
            [],
            [RateLimitAccountBarrier {
                account_id,
                limit: RateLimit { max_orders, window },
            }],
            [],
        )
        .expect("valid config")
    }

    fn account_policy(account_id: AccountId, max_orders: usize, window: Duration) -> TestPolicy {
        policy_from(account_settings(account_id, max_orders, window))
    }

    fn account_asset_settings(
        account_id: AccountId,
        settlement: &str,
        max_orders: usize,
        window: Duration,
    ) -> RateLimitSettings {
        RateLimitSettings::new(
            None,
            [],
            [],
            [RateLimitAccountAssetBarrier {
                account_id,
                settlement_asset: Asset::new(settlement).expect("asset code must be valid"),
                limit: RateLimit { max_orders, window },
            }],
        )
        .expect("valid config")
    }

    fn account_asset_policy(
        account_id: AccountId,
        settlement: &str,
        max_orders: usize,
        window: Duration,
    ) -> TestPolicy {
        policy_from(account_asset_settings(
            account_id, settlement, max_orders, window,
        ))
    }

    fn order(account_id: AccountId) -> OrderOperation {
        OrderOperation {
            instrument: Instrument::new(
                Asset::new("AAPL").expect("asset code must be valid"),
                Asset::new("USD").expect("asset code must be valid"),
            ),
            account_id,
            side: Side::Buy,
            trade_amount: TradeAmount::Quantity(
                Quantity::from_str("1").expect("quantity literal must be valid"),
            ),
            price: None,
        }
    }

    fn account(id: u64) -> AccountId {
        AccountId::from_u64(id)
    }
}
