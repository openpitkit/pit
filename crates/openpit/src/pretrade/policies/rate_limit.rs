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
use std::hash::Hash;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::core::{AccountRetirementRefusal, HasAccountId, HasInstrument};
use crate::param::{AccountId, Asset};
use crate::pretrade::policy::{missing_required_field_reject, PolicyGroupId, PolicyName};
use crate::pretrade::start_pre_trade_time::start_pre_trade_now;
use crate::pretrade::ConfigurablePolicy;
use crate::pretrade::DEFAULT_POLICY_GROUP_ID;
use crate::pretrade::{PreTradeContext, PreTradePolicy, Reject, RejectCode, RejectScope, Rejects};
use crate::storage::{ConfigCell, Storage, StorageBuilder};
use crate::time::Instant;
use crate::{Mutation, Mutations};

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
    /// A barrier window is zero or exceeds the maximum representable
    /// nanoseconds.
    InvalidWindow { window: Duration },
}

impl Display for RateLimitPolicyError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
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
/// policy keyed per account rather than in these settings. Clearing an account
/// or account+asset barrier leaves that key's log in storage, including when
/// the last barrier is cleared. Re-adding the key resumes counting the orders
/// still inside its configured window unless the account was retired:
/// [`Engine::retire_account`](crate::Engine::retire_account) deletes its logs
/// once no barrier names it, so re-added barriers start with empty logs.
/// A re-added broker or asset barrier starts a fresh window.
///
/// All axes may be empty; the policy then admits every order. Barriers can
/// be added later through [`Engine::configure`](crate::Engine::configure).
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
    /// All axes may be empty; the policy then admits every order. Barriers
    /// can be added later through
    /// [`Engine::configure`](crate::Engine::configure).
    /// Each window is validated; a zero or oversized window returns
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
    /// window. Passing `None` clears the axis, including the last barrier.
    ///
    /// # Errors
    ///
    /// [`RateLimitPolicyError::InvalidWindow`] if `limit.window` is zero or
    /// oversized. On error `self` is left untouched.
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

        self.broker = next;
        Ok(())
    }

    /// Replaces the full set of per-asset barriers (add, remove, or retune).
    ///
    /// Each surviving asset keeps its live counter, so a retune never resets
    /// the window; new keys start a fresh window. Duplicate keys within one
    /// call: last-write-wins. An empty set clears the axis, including the
    /// last barrier.
    ///
    /// # Errors
    ///
    /// [`RateLimitPolicyError::InvalidWindow`] if any `limit.window` is zero or
    /// oversized. On error `self` is left untouched.
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

        self.asset_limits = asset_limits;
        Ok(())
    }

    /// Replaces the full set of per-account barriers (add, remove, or retune).
    ///
    /// The sliding-window logs live in the policy keyed per account, so they
    /// survive a replacement; a surviving account keeps counting without a
    /// reset. Duplicate keys within one call: last-write-wins. An empty set
    /// clears the axis, including the last barrier.
    ///
    /// # Errors
    ///
    /// [`RateLimitPolicyError::InvalidWindow`] if any `limit.window` is zero or
    /// oversized. On error `self` is left untouched.
    pub fn set_account_barriers(
        &mut self,
        barriers: impl IntoIterator<Item = RateLimitAccountBarrier>,
    ) -> Result<(), RateLimitPolicyError> {
        let mut account_limits: HashMap<AccountId, RateLimit> = HashMap::new();
        for barrier in barriers {
            account_limits.insert(barrier.account_id, validate_limit(barrier.limit)?);
        }

        self.account_limits = account_limits;
        Ok(())
    }

    /// Replaces the full set of per-(account, asset) barriers (add, remove, or
    /// retune).
    ///
    /// The sliding-window logs live in the policy keyed per pair, so they
    /// survive a replacement; a surviving pair keeps counting without a reset.
    /// Duplicate keys within one call: last-write-wins. An empty set clears
    /// the axis, including the last barrier.
    ///
    /// # Errors
    ///
    /// [`RateLimitPolicyError::InvalidWindow`] if any `limit.window` is zero or
    /// oversized. On error `self` is left untouched.
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

// Pairs a validated limit with a fresh counter. Its zeroed window_start is the
// policy epoch, so a first push less than one window after the epoch counts in
// [epoch, epoch + window), and a later first push opens a new window.
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
// Each caller reads the clock before it pushes, so a delayed thread can arrive
// with a reading earlier than `window_start_nanos`. Such a reading counts
// against the current window: it never rolls the window over, so the window
// start only moves forward and the spent budget is never discarded early.
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
        if Self::rolled_over(now_nanos, win_start, window_nanos)
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
        if Self::rolled_over(now_nanos, win_start, window_nanos) {
            return 1;
        }
        self.count.load(Ordering::Acquire) + 1
    }

    // Shared by `push` and `peek` so the dry-run answers what a push would. A
    // reading earlier than `win_start` is not a rollover: plain subtraction
    // would wrap and look like a long-expired window.
    fn rolled_over(now_nanos: u64, win_start: u64, window_nanos: u64) -> bool {
        now_nanos
            .checked_sub(win_start)
            .is_some_and(|elapsed| elapsed >= window_nanos)
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
/// - every axis may be empty; the policy then admits every order;
/// - barriers can be added later through
///   [`Engine::configure`](crate::Engine::configure);
/// - duplicate keys within an axis: last-write-wins (no error).
///
/// # Account retirement
///
/// Account or account+asset barriers refuse with
/// `ConfigurationReferencesAccount`. On success, the policy forgets the
/// account's sliding-window logs on both axes. See
/// [`Engine::retire_account`](crate::Engine::retire_account) for the
/// engine-wide contract.
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
    per_account_timestamps: <LockingPolicyFactory as crate::storage::LockingPolicyFactory>::Shared<
        TimestampStorage<AccountId, LockingPolicyFactory>,
    >,
    per_account_asset_timestamps:
        <LockingPolicyFactory as crate::storage::LockingPolicyFactory>::Shared<
            TimestampStorage<(AccountId, Asset), LockingPolicyFactory>,
        >,
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

        let per_account_timestamps =
            LockingPolicyFactory::new_shared(storage_builder.create_for_bound_key());
        let per_account_asset_timestamps =
            LockingPolicyFactory::new_shared(storage_builder.create_for_bound_key());

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
    <LockingPolicyFactory as crate::storage::LockingPolicyFactory>::Shared<
        TimestampStorage<AccountId, LockingPolicyFactory>,
    >: 'static,
    <LockingPolicyFactory as crate::storage::LockingPolicyFactory>::Shared<
        TimestampStorage<(AccountId, Asset), LockingPolicyFactory>,
    >: 'static,
{
    fn name(&self) -> &str {
        Self::NAME
    }

    fn policy_group_id(&self) -> PolicyGroupId {
        self.group_id
    }

    fn retire_account(
        &self,
        account_id: AccountId,
        mutations: &mut Mutations,
    ) -> Result<(), AccountRetirementRefusal> {
        if self.settings.with(|settings| {
            settings.account_limits.contains_key(&account_id)
                || settings
                    .account_asset_limits
                    .keys()
                    .any(|(account, _)| *account == account_id)
        }) {
            return Err(AccountRetirementRefusal::ConfigurationReferencesAccount);
        }

        let keys: Vec<_> = self
            .per_account_asset_timestamps
            .keys()
            .into_iter()
            .filter(|(account, _)| *account == account_id)
            .collect();
        let per_account_timestamps = self.per_account_timestamps.clone();
        let per_account_asset_timestamps = self.per_account_asset_timestamps.clone();
        mutations.push(Mutation::new(
            move || {
                per_account_timestamps.remove(&account_id);
                for key in keys {
                    per_account_asset_timestamps.remove(&key);
                }
            },
            || {},
        ));
        Ok(())
    }

    #[allow(private_interfaces)]
    fn built_in_config_entry(&self) -> Option<crate::core::ConfigEntry<LockingPolicyFactory>> {
        Some(crate::core::ConfigEntry::RateLimit(
            crate::pretrade::ConfigurablePolicy::settings_cell(self),
        ))
    }

    fn check_pre_trade_start(
        &self,
        ctx: &PreTradeContext<<Sync as crate::core::SyncMode>::StorageLockingPolicyFactory>,
        order: &Order,
    ) -> Result<(), Rejects> {
        self.check_start(ctx, order, true)
    }

    /// Dry-run start-stage check.
    ///
    /// Rate limiting is an immediate-side-effect policy: the normal
    /// [`check_pre_trade_start`](Self::check_pre_trade_start) consumes a slot in
    /// every applicable window counter, and rejected attempts are never rolled
    /// back. A dry-run must not move that state, so this override consumes no
    /// budget and pushes nothing into any counter. It still reports the verdict
    /// a real call would produce: it runs the real check's path without
    /// recording the order - each fixed-window counter is *peeked* read-only
    /// for the count a push would yield, and each sliding-window log is counted
    /// by the same rule as in the real check and left as it is - so an order
    /// that would breach an already-exhausted window is rejected here too, with
    /// no side effect.
    fn check_pre_trade_start_dry_run(
        &self,
        ctx: &PreTradeContext<<Sync as crate::core::SyncMode>::StorageLockingPolicyFactory>,
        order: &Order,
    ) -> Result<(), Rejects> {
        self.check_start(ctx, order, false)
    }
}

impl<LockingPolicyFactory> RateLimitPolicy<LockingPolicyFactory>
where
    LockingPolicyFactory: crate::storage::LockingPolicyFactory
        + crate::storage::CreateStorageFor<AccountId>
        + 'static,
{
    // The start-stage check behind both the real check and the dry run, so
    // the two count every axis by one rule; `record` only decides whether the
    // order consumes its slot.
    //
    // One coherent settings snapshot decides both applicability and the exact
    // live counter/log to count on. ALL applicable axes are counted before
    // resolving (flood semantics): every applicable axis must consume its slot
    // even when an earlier-priority axis already breaches, so a flood cannot
    // bypass a counter by tripping another. The breach is resolved in priority
    // order: broker -> asset -> account -> account+asset. Accessors may publish
    // new settings; this check keeps the version selected before they run.
    fn check_start<Order>(
        &self,
        ctx: &PreTradeContext<LockingPolicyFactory>,
        order: &Order,
        record: bool,
    ) -> Result<(), Rejects>
    where
        Order: HasAccountId + HasInstrument,
    {
        let now = start_pre_trade_now();
        let now_nanos = now
            .checked_duration_since(self.epoch)
            .unwrap_or_default()
            .as_nanos() as u64;

        self.settings.with_snapshot(|s| {
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
                Some(ctx.account_id().map_err(|e| {
                    Rejects::from(missing_required_field_reject(self, "account ID", &e))
                })?)
            } else {
                None
            };

            let broker = s.broker.as_ref().map(|slot| {
                let count = fixed_window_count(slot, now_nanos, record);
                over_limit(count, &slot.limit, "broker barrier")
            });

            let asset = settlement_opt.as_ref().and_then(|settlement| {
                s.asset_limits.get(settlement).map(|slot| {
                    let count = fixed_window_count(slot, now_nanos, record);
                    over_limit(count, &slot.limit, "asset barrier")
                })
            });

            let account = account_id_opt.and_then(|account_id| {
                s.account_limits.get(&account_id).map(|limit| {
                    let count = sliding_window_count(
                        &self.per_account_timestamps,
                        account_id,
                        now,
                        limit.window,
                        record,
                    );
                    over_limit(count, limit, "account barrier")
                })
            });

            let account_asset = account_id_opt.and_then(|account_id| {
                settlement_opt.as_ref().and_then(|settlement| {
                    let key = (account_id, settlement.clone());
                    s.account_asset_limits.get(&key).map(|limit| {
                        let count = sliding_window_count(
                            &self.per_account_asset_timestamps,
                            key,
                            now,
                            limit.window,
                            record,
                        );
                        over_limit(count, limit, "account+asset barrier")
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

// The count of an order on a fixed-window counter: `push` records the order,
// `peek` answers what that push would return without recording it.
fn fixed_window_count(slot: &RateLimitSlot, now_nanos: u64, record: bool) -> u64 {
    let window = window_nanos(&slot.limit);
    if record {
        slot.counter.push(now_nanos, window)
    } else {
        slot.counter.peek(now_nanos, window)
    }
}

// The count of an order on the sliding-window log under `key`, taken by
// `window_count` on both paths; only a recording call then trims the log and
// inserts the order. A dry run reads without creating the log: a key with no
// log yet holds no entries, so the order would be its first.
fn sliding_window_count<Key, LockingPolicy>(
    logs: &Storage<Key, VecDeque<Instant>, LockingPolicy>,
    key: Key,
    now: Instant,
    window: Duration,
    record: bool,
) -> u64
where
    Key: Hash + Eq + Clone,
    LockingPolicy: crate::storage::LockingPolicy,
{
    if !record {
        return logs
            .with(&key, |log| window_count(log, now, window))
            .unwrap_or(1);
    }
    logs.with_mut(key, VecDeque::new, |log, _is_new| {
        let count = window_count(log, now, window);
        advance_window(log, now, window);
        insert_in_time_order(log, now);
        count
    })
}

fn window_nanos(limit: &RateLimit) -> u64 {
    // Validated at construction / on every setter to fit in u64 nanoseconds.
    limit.window.as_nanos() as u64
}

// Builds a reject if `count` breaches `limit`, else `None`. Keeps the
// limit read and the threshold comparison inside the single settings read.
fn over_limit(count: u64, limit: &RateLimit, reason: &'static str) -> Option<Rejects> {
    (count > limit.max_orders as u64)
        .then(|| rate_limit_reject(reason, count, limit.max_orders as u64, limit.window))
}

// `barrier` names the breached axis (e.g. "broker barrier"); the reject
// reason is "rate limit exceeded: <barrier>".
fn rate_limit_reject(
    barrier: &'static str,
    count: u64,
    max_orders: u64,
    window: Duration,
) -> Rejects {
    Reject::new(
        RATE_LIMIT_POLICY_NAME,
        RejectScope::Order,
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

// The expiry rule of a sliding-window log, shared by the real check and the
// dry run: an entry expires once a full window has elapsed since it, and an
// entry that is not in the past as of `now` never does.
fn entry_expired(entry: Instant, now: Instant, window: Duration) -> bool {
    now.checked_duration_since(entry)
        .is_some_and(|elapsed| elapsed >= window)
}

fn advance_window(timestamps: &mut VecDeque<Instant>, now: Instant, window: Duration) {
    while timestamps
        .front()
        .is_some_and(|oldest| entry_expired(*oldest, now, window))
    {
        timestamps.pop_front();
    }
}

// Keeps the log in time order: a late reading is inserted at its place, not at
// the back, so the expired entries always form a prefix that `advance_window`
// removes whole and `window_count` finds by binary search. Scanning the whole
// log instead would make every call linear in a log that, under a flood, holds
// every attempt in the window.
fn insert_in_time_order(timestamps: &mut VecDeque<Instant>, now: Instant) {
    if timestamps.back().is_some_and(|newest| *newest > now) {
        let at = timestamps.partition_point(|entry| *entry <= now);
        timestamps.insert(at, now);
    } else {
        timestamps.push_back(now);
    }
}

// The count of an order on a sliding-window log as of `now`: the entries still
// inside `window`, plus one for the order itself. The expired entries are the
// prefix the time order keeps them in, so the count costs O(log n).
fn window_count(timestamps: &VecDeque<Instant>, now: Instant, window: Duration) -> u64 {
    let expired = timestamps.partition_point(|entry| entry_expired(*entry, now, window));
    (timestamps.len() - expired) as u64 + 1
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
    use crate::storage::{ConfigCell, FullLocking, LocalConfigCell, NoLocking};

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
    fn maximum_window_accepted_by_settings() {
        let result = RateLimitSettings::new(
            Some(RateLimitBrokerBarrier {
                limit: RateLimit {
                    max_orders: 1,
                    window: Duration::from_nanos(u64::MAX),
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
        let max_plus_one = Duration::from_nanos(u64::MAX) + Duration::from_nanos(1);
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
    fn empty_settings_are_accepted() {
        assert!(RateLimitSettings::new(None, [], [], []).is_ok());
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
    fn setters_can_clear_last_axis() {
        // Each settings value holds a single barrier of one order per window
        // that matches the order below. After the setter clears it, both
        // orders must pass; the removed barrier would have rejected the
        // second one.
        let window = Duration::from_secs(60);
        let o = order(account(1));

        let mut settings = broker_settings(1, window);
        assert!(settings.set_broker(None).is_ok());
        let cleared_broker = policy_from(settings);

        let mut settings = asset_settings("USD", 1, window);
        assert!(settings.set_asset_barriers([]).is_ok());
        let cleared_asset = policy_from(settings);

        let mut settings = account_settings(account(1), 1, window);
        assert!(settings.set_account_barriers([]).is_ok());
        let cleared_account = policy_from(settings);

        let mut settings = account_asset_settings(account(1), "USD", 1, window);
        assert!(settings.set_account_asset_barriers([]).is_ok());
        let cleared_account_asset = policy_from(settings);

        let base = Instant::now();
        for policy in [
            &cleared_broker,
            &cleared_asset,
            &cleared_account,
            &cleared_account_asset,
        ] {
            assert!(check_at(policy, &o, base).is_ok());
            assert!(check_at(policy, &o, base + Duration::from_secs(1)).is_ok());
        }
    }

    #[test]
    fn empty_policy_does_not_access_order_fields() {
        struct EmptyOrder(std::cell::Cell<usize>);
        impl crate::HasAccountId for EmptyOrder {
            fn account_id(&self) -> Result<AccountId, crate::RequestFieldAccessError> {
                self.0.set(self.0.get() + 1);
                Err(crate::RequestFieldAccessError::new("account ID"))
            }
        }
        impl crate::HasInstrument for EmptyOrder {
            fn instrument(&self) -> Result<&Instrument, crate::RequestFieldAccessError> {
                panic!("empty policy must not read instrument");
            }
        }
        let policy = policy_from(RateLimitSettings::new(None, [], [], []).expect("empty is valid"));
        let order = EmptyOrder(std::cell::Cell::new(0));
        let ctx = PreTradeContext::<NoLocking>::new(None, &order);
        order.0.set(0);
        assert!(<TestPolicy as PreTradePolicy<
            EmptyOrder, (), (), crate::core::LocalSync,
        >>::check_pre_trade_start(
            &policy, &ctx, &order,
        ).is_ok());
        assert!(<TestPolicy as PreTradePolicy<
            EmptyOrder, (), (), crate::core::LocalSync,
        >>::check_pre_trade_start_dry_run(
            &policy, &ctx, &order,
        ).is_ok());
        assert_eq!(order.0.get(), 0);
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

    #[test]
    fn check_pre_trade_start_accessor_can_reconfigure_without_borrow_panic() {
        struct ReconfiguringInstrumentOrder {
            settings: LocalConfigCell<RateLimitSettings>,
            instrument: Instrument,
            account_id: AccountId,
        }

        impl crate::HasInstrument for ReconfiguringInstrumentOrder {
            fn instrument(&self) -> Result<&Instrument, crate::RequestFieldAccessError> {
                self.settings
                    .update::<RateLimitPolicyError>(|settings| {
                        settings.set_asset_barriers([RateLimitAssetBarrier {
                            limit: RateLimit {
                                max_orders: 10,
                                window: Duration::from_secs(60),
                            },
                            settlement_asset: Asset::new("USD").expect("asset code must be valid"),
                        }])
                    })
                    .expect("instrument accessor must reconfigure without a borrow panic");
                Ok(&self.instrument)
            }
        }

        impl crate::HasAccountId for ReconfiguringInstrumentOrder {
            fn account_id(&self) -> Result<AccountId, crate::RequestFieldAccessError> {
                Ok(self.account_id)
            }
        }

        let policy = asset_policy("USD", 1, Duration::from_secs(60));
        let base = Instant::now();
        assert!(check_at(&policy, &order(account(1)), base).is_ok());

        let reconfiguring_order = ReconfiguringInstrumentOrder {
            settings: policy.settings_cell(),
            instrument: Instrument::new(
                Asset::new("AAPL").expect("asset code must be valid"),
                Asset::new("USD").expect("asset code must be valid"),
            ),
            account_id: account(1),
        };
        let reject = with_start_pre_trade_now(base + Duration::from_secs(1), || {
            <TestPolicy as PreTradePolicy<
                ReconfiguringInstrumentOrder,
                (),
                (),
                crate::core::LocalSync,
            >>::check_pre_trade_start(
                &policy,
                &PreTradeContext::<NoLocking>::new(None, &reconfiguring_order),
                &reconfiguring_order,
            )
        })
        .expect_err("the pre-update asset limit must reject");
        assert_eq!(reject[0].reason, "rate limit exceeded: asset barrier");
        assert_eq!(
            reject[0].details,
            "submitted 2 orders in 60s window, max allowed: 1"
        );

        assert!(check_at(&policy, &order(account(1)), base + Duration::from_secs(2),).is_ok());
    }

    /// Dry-run twin of
    /// [`check_pre_trade_start_accessor_can_reconfigure_without_borrow_panic`].
    ///
    /// The reconfiguring accessor is the instrument one, because that is the
    /// only kind the check still invokes while its settings snapshot is held:
    /// the account comes from the context, read once when the operation
    /// started and long before any snapshot is taken.
    #[test]
    fn check_pre_trade_start_dry_run_accessor_can_reconfigure_without_borrow_panic() {
        struct ReconfiguringInstrumentOrder {
            settings: LocalConfigCell<RateLimitSettings>,
            instrument: Instrument,
            account_id: AccountId,
        }

        impl crate::HasInstrument for ReconfiguringInstrumentOrder {
            fn instrument(&self) -> Result<&Instrument, crate::RequestFieldAccessError> {
                self.settings
                    .update::<RateLimitPolicyError>(|settings| {
                        settings.set_asset_barriers([RateLimitAssetBarrier {
                            limit: RateLimit {
                                max_orders: 10,
                                window: Duration::from_secs(60),
                            },
                            settlement_asset: Asset::new("USD").expect("asset code must be valid"),
                        }])
                    })
                    .expect("instrument accessor must reconfigure without a borrow panic");
                Ok(&self.instrument)
            }
        }

        impl crate::HasAccountId for ReconfiguringInstrumentOrder {
            fn account_id(&self) -> Result<AccountId, crate::RequestFieldAccessError> {
                Ok(self.account_id)
            }
        }

        let policy = asset_policy("USD", 1, Duration::from_secs(60));
        let base = Instant::now();
        assert!(check_at(&policy, &order(account(1)), base).is_ok());

        let reconfiguring_order = ReconfiguringInstrumentOrder {
            settings: policy.settings_cell(),
            instrument: Instrument::new(
                Asset::new("AAPL").expect("asset code must be valid"),
                Asset::new("USD").expect("asset code must be valid"),
            ),
            account_id: account(1),
        };
        let reject = with_start_pre_trade_now(base + Duration::from_secs(1), || {
            <TestPolicy as PreTradePolicy<
                ReconfiguringInstrumentOrder,
                (),
                (),
                crate::core::LocalSync,
            >>::check_pre_trade_start_dry_run(
                &policy,
                &PreTradeContext::<NoLocking>::new(None, &reconfiguring_order),
                &reconfiguring_order,
            )
        })
        .expect_err("the pre-update asset limit must reject");
        assert_eq!(reject[0].reason, "rate limit exceeded: asset barrier");
        assert_eq!(
            reject[0].details,
            "submitted 2 orders in 60s window, max allowed: 1"
        );

        assert!(check_at(&policy, &order(account(1)), base + Duration::from_secs(2),).is_ok());
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
    fn dry_run_creates_no_account_window_logs() {
        // A dry run counts a key with no log yet as empty and must not create
        // the log. The count alone cannot show it: an empty log counts the
        // same.
        let account_id = account(1);
        let usd = Asset::new("USD").expect("asset code must be valid");
        let limit = RateLimit {
            max_orders: 10,
            window: Duration::from_secs(60),
        };
        let policy = policy_from(
            RateLimitSettings::new(
                None,
                [],
                [RateLimitAccountBarrier {
                    limit: limit.clone(),
                    account_id,
                }],
                [RateLimitAccountAssetBarrier {
                    limit,
                    account_id,
                    settlement_asset: usd.clone(),
                }],
            )
            .expect("valid barriers"),
        );
        let o = order(account_id);
        let key = (account_id, usd);
        let now = Instant::now();

        assert!(check_dry_run_at(&policy, &o, now).is_ok());
        assert!(policy
            .per_account_timestamps
            .with(&account_id, |_| ())
            .is_none());
        assert!(policy
            .per_account_asset_timestamps
            .with(&key, |_| ())
            .is_none());

        // The same lookups see the logs once a real check records the order.
        assert!(check_at(&policy, &o, now).is_ok());
        assert!(policy
            .per_account_timestamps
            .with(&account_id, |_| ())
            .is_some());
        assert!(policy
            .per_account_asset_timestamps
            .with(&key, |_| ())
            .is_some());
    }

    #[test]
    fn late_clock_reading_counts_in_current_broker_window() {
        // A call whose clock reading predates the current window start (its
        // thread was delayed between reading the clock and pushing) counts
        // against the current window: it neither opens a new window nor moves
        // the window start backwards.
        let policy = broker_policy(2, Duration::from_secs(10));
        let o = order(account(1));
        let base = Instant::now();

        // Opens the window at base + 20s and exhausts it.
        assert!(check_at(&policy, &o, base + Duration::from_secs(20)).is_ok());
        assert!(check_at(&policy, &o, base + Duration::from_secs(21)).is_ok());

        let reject = check_at(&policy, &o, base + Duration::from_secs(15))
            .expect_err("late reading must count against the exhausted window");
        assert_eq!(reject[0].reason, "rate limit exceeded: broker barrier");
        assert_eq!(
            reject[0].details,
            "submitted 3 orders in 10s window, max allowed: 2"
        );

        // The window still starts at base + 20s: base + 29s is inside it and
        // base + 30s opens the next one.
        assert!(check_at(&policy, &o, base + Duration::from_secs(29)).is_err());
        assert!(check_at(&policy, &o, base + Duration::from_secs(30)).is_ok());
    }

    #[test]
    fn dry_run_counts_late_clock_reading_in_current_broker_window() {
        // The dry-run answers what the real check would: a reading that
        // predates the current window start sees the exhausted window.
        let policy = broker_policy(1, Duration::from_secs(10));
        let o = order(account(1));
        let base = Instant::now();

        assert!(check_at(&policy, &o, base + Duration::from_secs(20)).is_ok());

        let reject = check_dry_run_at(&policy, &o, base + Duration::from_secs(15))
            .expect_err("late dry-run must report the exhausted window");
        assert_eq!(reject[0].reason, "rate limit exceeded: broker barrier");
        assert!(check_at(&policy, &o, base + Duration::from_secs(15)).is_err());
    }

    #[test]
    fn late_clock_readings_under_full_sync_do_not_reopen_broker_window() {
        // Threads share one broker counter under FullSync. Half of them arrive
        // with readings that predate the current window start, each call
        // earlier than the previous one. No reading reaches the window end, so
        // no rollover is due and the documented boundary burst cannot occur:
        // exactly `max_orders` calls pass.
        type FullSyncPolicy = RateLimitPolicy<FullLocking>;
        const MAX_ORDERS: usize = 8;
        const THREADS: u32 = 8;
        const PER_THREAD: u32 = 200;

        let builder = crate::Engine::builder::<OrderOperation, (), ()>().full_sync();
        let policy = FullSyncPolicy::new(
            broker_settings(MAX_ORDERS, Duration::from_secs(10)),
            builder.storage_builder(),
        );
        let o = order(account(1));
        let window_start = Instant::now() + Duration::from_secs(20);
        let check = |now: Instant| {
            with_start_pre_trade_now(now, || {
                <FullSyncPolicy as PreTradePolicy<OrderOperation, (), (), crate::core::FullSync>>::check_pre_trade_start(
                    &policy,
                    &PreTradeContext::<FullLocking>::new(None, &o),
                    &o,
                )
            })
            .is_ok()
        };

        // Opens the window.
        assert!(check(window_start));

        let passed: usize = std::thread::scope(|s| {
            let workers: Vec<_> = (0..THREADS)
                .map(|tid| {
                    let check = &check;
                    s.spawn(move || {
                        (0..PER_THREAD)
                            .filter(|&i| {
                                let offset =
                                    Duration::from_micros(u64::from(tid * PER_THREAD + i + 1));
                                if tid % 2 == 0 {
                                    check(window_start + offset)
                                } else {
                                    check(window_start - offset)
                                }
                            })
                            .count()
                    })
                })
                .collect();
            workers
                .into_iter()
                .map(|worker| worker.join().expect("worker must not panic"))
                .sum()
        });

        assert_eq!(passed + 1, MAX_ORDERS);
    }

    #[test]
    fn expired_late_clock_reading_leaves_account_window_logs() {
        // A call whose clock reading predates the newest log entry still
        // leaves the log once its window has elapsed, so the dry-run and the
        // real check count the same live entries.
        let o = order(account(1));
        let base = Instant::now();
        for policy in [
            account_policy(account(1), 2, Duration::from_secs(10)),
            account_asset_policy(account(1), "USD", 2, Duration::from_secs(10)),
        ] {
            assert!(check_at(&policy, &o, base + Duration::from_secs(20)).is_ok());
            assert!(check_at(&policy, &o, base + Duration::from_secs(15)).is_ok());

            // The entry at base + 15s has expired at base + 25s.
            let now = base + Duration::from_secs(25);
            check_dry_run_at(&policy, &o, now).expect("dry-run must not count the expired entry");
            check_at(&policy, &o, now).expect("real check must not count the expired entry");
        }
    }

    #[test]
    fn late_clock_reading_does_not_revive_expired_account_log_entry() {
        // The call reading base + 5s arrives after the one reading base + 20s
        // and expires at base + 16s. A later call reading base + 7s must not
        // count it again: the window then holds 7s, 16s and 20s.
        let policy = account_policy(account(1), 3, Duration::from_secs(10));
        let o = order(account(1));
        let base = Instant::now();

        for secs in [20, 5, 16] {
            assert!(check_at(&policy, &o, base + Duration::from_secs(secs)).is_ok());
        }

        let now = base + Duration::from_secs(7);
        check_dry_run_at(&policy, &o, now).expect("dry-run must not count the expired entry");
        check_at(&policy, &o, now).expect("real check must not count the expired entry");

        // Exactly three entries are live, so a fourth order breaches.
        let reject = check_at(&policy, &o, now).expect_err("fourth live order must be rejected");
        assert_eq!(reject[0].reason, "rate limit exceeded: account barrier");
        assert_eq!(
            reject[0].details,
            "submitted 4 orders in 10s window, max allowed: 3"
        );
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

        // Order uses USD, policy has EUR barrier - should pass regardless of count
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
        assert_eq!(reject.scope, RejectScope::Order);
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
        // account(2) has no barrier - always passes.
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
            &policy,
            &PreTradeContext::<NoLocking>::new(None, &order),
            &order,
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
            &policy,
            &PreTradeContext::<NoLocking>::new(None, &order),
            &order,
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
        assert_eq!(reject.scope, RejectScope::Order);
        assert_eq!(reject.code, RejectCode::RateLimitExceeded);
        assert_eq!(reject.reason, "rate limit exceeded: account+asset barrier");
    }

    #[test]
    fn account_asset_barrier_ignores_different_account() {
        let policy = account_asset_policy(account(1), "USD", 1, Duration::from_secs(10));
        let base = Instant::now();

        // account(2) has no account+asset barrier for USD - passes
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
        assert_eq!(reject[0].scope, RejectScope::Order);
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
        // Both broker and account breach - broker is reported (checked first).
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
            &PreTradeContext::<NoLocking>::new(None, &NoAccountId),
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
                &PreTradeContext::<NoLocking>::new(None, &NoAccountId),
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
            &policy,
            &PreTradeContext::<NoLocking>::new(None, &order),
            &order,
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
                &PreTradeContext::<NoLocking>::new(None, order),
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
                &PreTradeContext::<NoLocking>::new(None, order),
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

    #[test]
    fn retire_account_window_log_is_kept_on_rollback_and_removed_on_commit() {
        let target = account(99224416);
        let other = account(11223344);
        let policy = policy_from(
            RateLimitSettings::new(
                None,
                [],
                [target, other].map(|account_id| RateLimitAccountBarrier {
                    limit: RateLimit {
                        max_orders: 10,
                        window: Duration::from_secs(3600),
                    },
                    account_id,
                }),
                [],
            )
            .expect("valid barriers"),
        );
        let now = Instant::now();
        for id in [target, other] {
            check_at(&policy, &order(id), now).expect("order passes");
        }
        assert!(policy
            .per_account_timestamps
            .with(&target, |_| ())
            .is_some());
        let mut mutations = crate::Mutations::new();
        assert_eq!(
            <TestPolicy as PreTradePolicy<OrderOperation, (), (), crate::LocalSync>>::retire_account(
                &policy, target, &mut mutations,
            ),
            Err(crate::AccountRetirementRefusal::ConfigurationReferencesAccount)
        );
        assert_eq!(
            <TestPolicy as PreTradePolicy<OrderOperation, (), (), crate::LocalSync>>::retire_account(
                &policy, target, &mut mutations,
            ),
            Err(crate::AccountRetirementRefusal::ConfigurationReferencesAccount)
        );
        policy
            .settings
            .update(|settings| settings.set_account_barriers([]))
            .expect("clear account barriers");
        assert_eq!(
            <TestPolicy as PreTradePolicy<OrderOperation, (), (), crate::LocalSync>>::retire_account(
                &policy, target, &mut mutations,
            ),
            Ok(())
        );
        let _ = mutations.rollback_all();
        assert!(policy
            .per_account_timestamps
            .with(&target, |_| ())
            .is_some());

        let mut mutations = crate::Mutations::new();
        <TestPolicy as PreTradePolicy<OrderOperation, (), (), crate::LocalSync>>::retire_account(
            &policy,
            target,
            &mut mutations,
        )
        .expect("unconfigured account retires");
        assert!(!mutations.commit_all().failed());
        assert!(policy
            .per_account_timestamps
            .with(&target, |_| ())
            .is_none());
        assert!(policy.per_account_timestamps.with(&other, |_| ()).is_some());
    }

    #[test]
    fn retire_account_asset_window_log_is_kept_on_rollback_and_removed_on_commit() {
        let target = account(99224416);
        let other = account(11223344);
        let usd = Asset::new("USD").expect("valid asset");
        let eur = Asset::new("EUR").expect("valid asset");
        let policy = policy_from(
            RateLimitSettings::new(
                None,
                [],
                [],
                [
                    (target, usd.clone()),
                    (target, eur.clone()),
                    (other, usd.clone()),
                ]
                .map(|(account_id, settlement_asset)| {
                    RateLimitAccountAssetBarrier {
                        limit: RateLimit {
                            max_orders: 10,
                            window: Duration::from_secs(3600),
                        },
                        account_id,
                        settlement_asset,
                    }
                }),
            )
            .expect("valid barriers"),
        );
        let now = Instant::now();
        for id in [target, other] {
            check_at(&policy, &order(id), now).expect("order passes");
        }
        let mut eur_order = order(target);
        eur_order.instrument =
            Instrument::new(Asset::new("AAPL").expect("valid asset"), eur.clone());
        check_at(&policy, &eur_order, now).expect("EUR order passes");
        let target_keys = [(target, usd.clone()), (target, eur)];
        let other_key = (other, usd);
        for key in &target_keys {
            assert!(policy
                .per_account_asset_timestamps
                .with(key, |_| ())
                .is_some());
        }
        let mut mutations = crate::Mutations::new();
        assert_eq!(
            <TestPolicy as PreTradePolicy<OrderOperation, (), (), crate::LocalSync>>::retire_account(
                &policy, target, &mut mutations,
            ),
            Err(crate::AccountRetirementRefusal::ConfigurationReferencesAccount)
        );
        assert_eq!(
            <TestPolicy as PreTradePolicy<OrderOperation, (), (), crate::LocalSync>>::retire_account(
                &policy, target, &mut mutations,
            ),
            Err(crate::AccountRetirementRefusal::ConfigurationReferencesAccount)
        );
        policy
            .settings
            .update(|settings| settings.set_account_asset_barriers([]))
            .expect("clear account asset barriers");
        <TestPolicy as PreTradePolicy<OrderOperation, (), (), crate::LocalSync>>::retire_account(
            &policy,
            target,
            &mut mutations,
        )
        .expect("unconfigured account retires");
        let _ = mutations.rollback_all();
        for key in &target_keys {
            assert!(policy
                .per_account_asset_timestamps
                .with(key, |_| ())
                .is_some());
        }

        let mut mutations = crate::Mutations::new();
        <TestPolicy as PreTradePolicy<OrderOperation, (), (), crate::LocalSync>>::retire_account(
            &policy,
            target,
            &mut mutations,
        )
        .expect("unconfigured account retires");
        assert!(!mutations.commit_all().failed());
        for key in &target_keys {
            assert!(policy
                .per_account_asset_timestamps
                .with(key, |_| ())
                .is_none());
        }
        assert!(policy
            .per_account_asset_timestamps
            .with(&other_key, |_| ())
            .is_some());
    }

    /// The account axis takes the operation's account from the context, so the
    /// order's own accessor is called exactly once per operation - by the
    /// engine, at the entry. A second call would both show up in this counter
    /// and let the policy meter an account the request was never admitted on.
    #[test]
    fn the_account_axis_adds_no_second_read_of_the_order_account() {
        use std::cell::Cell;
        use std::rc::Rc;

        struct CountedAccountOrder {
            instrument: Instrument,
            account_id: AccountId,
            reads: Rc<Cell<usize>>,
        }

        impl crate::HasInstrument for CountedAccountOrder {
            fn instrument(&self) -> Result<&Instrument, crate::RequestFieldAccessError> {
                Ok(&self.instrument)
            }
        }

        impl crate::HasAccountId for CountedAccountOrder {
            fn account_id(&self) -> Result<AccountId, crate::RequestFieldAccessError> {
                self.reads.set(self.reads.get() + 1);
                Ok(self.account_id)
            }
        }

        let account_id = account(1);
        let builder = crate::Engine::builder::<CountedAccountOrder, (), ()>().no_sync();
        let policy = RateLimitPolicy::new(
            account_settings(account_id, 10, Duration::from_secs(60)),
            builder.storage_builder(),
        );
        let engine = builder
            .pre_trade(policy)
            .build()
            .expect("engine must build");
        let counted = |reads: &Rc<Cell<usize>>| CountedAccountOrder {
            instrument: Instrument::new(
                Asset::new("AAPL").expect("asset code must be valid"),
                Asset::new("USD").expect("asset code must be valid"),
            ),
            account_id,
            reads: Rc::clone(reads),
        };

        let reads = Rc::new(Cell::new(0));
        engine
            .start_pre_trade(counted(&reads))
            .expect("an order within the account limit must pass the start stage");
        assert_eq!(reads.get(), 1);

        let reads = Rc::new(Cell::new(0));
        assert!(engine.start_pre_trade_dry_run(counted(&reads)).is_pass());
        assert_eq!(reads.get(), 1);
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

    // The fixed-window counter and the whole policy against independent
    // reference models, over arbitrary non-monotone clock readings.
    mod properties {
        use std::time::Duration;

        use proptest::prelude::*;

        use crate::core::{Instrument, OrderOperation};
        use crate::param::Asset;
        use crate::pretrade::policies::proptest_config::config;
        use crate::pretrade::Rejects;

        use super::super::{
            rate_limit_reject, RateLimit, RateLimitAccountAssetBarrier, RateLimitAccountBarrier,
            RateLimitAssetBarrier, RateLimitBrokerBarrier, RateLimitSettings,
        };
        use super::{account, broker_policy, check_at, check_dry_run_at, order, policy_from};

        // Sized for about 3 s per property in a debug build.
        const COUNTER_CASES: u32 = 8192;
        const POLICY_CASES: u32 = 8192;

        // The expiry rule of the fixed window and the sliding log: a reading
        // earlier than `since` never expires it.
        fn expired(now: u64, since: u64, window: Duration) -> bool {
            now >= since && Duration::from_nanos(now - since) >= window
        }

        #[derive(Debug, Clone, Copy)]
        enum CounterCall {
            Push(u64),
            Peek(u64),
        }

        proptest! {
            #![proptest_config(config(COUNTER_CASES))]

            // Late readings included, every count matches the model, whose
            // window start only moves forward. No order is allowed, so every
            // check rejects with its count.
            #[test]
            fn fixed_window_start_never_moves_backward(
                window in 1u64..=1000,
                calls in prop::collection::vec(
                    prop_oneof![
                        (0u64..=4000).prop_map(CounterCall::Push),
                        (0u64..=4000).prop_map(CounterCall::Peek),
                    ],
                    1..=64,
                ),
            ) {
                let limit = RateLimit {
                    max_orders: 0,
                    window: Duration::from_nanos(window),
                };
                let policy = broker_policy(limit.max_orders, limit.window);
                let mut model = [ModelBarrier::new(Barrier::Broker, limit)];
                let o = order(account(1));
                let at = |offset| policy.epoch + Duration::from_nanos(offset);
                for (step, call) in calls.into_iter().enumerate() {
                    let (actual, expected) = match call {
                        CounterCall::Push(offset) => (
                            check_at(&policy, &o, at(offset)),
                            model_check(&mut model, 1, "USD", offset),
                        ),
                        CounterCall::Peek(offset) => (
                            check_dry_run_at(&policy, &o, at(offset)),
                            model_check(&mut model.clone(), 1, "USD", offset),
                        ),
                    };
                    prop_assert_eq!(actual, expected, "{:?} at step {}", call, step);
                }
            }
        }

        // A barrier by axis and key. Account 3 and GBP never get one, so
        // orders also reach keys without a barrier.
        #[derive(Debug, Clone, Copy)]
        enum Barrier {
            Broker,
            Asset(&'static str),
            Account(u64),
            AccountAsset(u64, &'static str),
        }

        impl Barrier {
            fn applies(self, account_id: u64, settlement: &str) -> bool {
                match self {
                    Self::Broker => true,
                    Self::Asset(code) => code == settlement,
                    Self::Account(id) => id == account_id,
                    Self::AccountAsset(id, code) => id == account_id && code == settlement,
                }
            }

            fn name(self) -> &'static str {
                match self {
                    Self::Broker => "broker barrier",
                    Self::Asset(_) => "asset barrier",
                    Self::Account(_) => "account barrier",
                    Self::AccountAsset(..) => "account+asset barrier",
                }
            }
        }

        // Every candidate barrier, in reject priority order.
        const BARRIERS: [Barrier; 8] = [
            Barrier::Broker,
            Barrier::Asset("USD"),
            Barrier::Asset("EUR"),
            Barrier::Account(1),
            Barrier::Account(2),
            Barrier::AccountAsset(1, "USD"),
            Barrier::AccountAsset(2, "EUR"),
            Barrier::AccountAsset(1, "EUR"),
        ];

        fn barrier_limits() -> impl Strategy<Value = Vec<(Barrier, RateLimit)>> {
            let rate_limit = (0usize..=4, 1u64..=64).prop_map(|(max_orders, window)| RateLimit {
                max_orders,
                window: Duration::from_nanos(window),
            });
            prop::array::uniform8(prop::option::of(rate_limit)).prop_map(|limits| {
                BARRIERS
                    .into_iter()
                    .zip(limits)
                    .filter_map(|(barrier, limit)| limit.map(|limit| (barrier, limit)))
                    .collect()
            })
        }

        fn settings(barriers: &[(Barrier, RateLimit)]) -> RateLimitSettings {
            let mut broker = None;
            let mut assets = Vec::new();
            let mut accounts = Vec::new();
            let mut account_assets = Vec::new();
            for (barrier, limit) in barriers.iter().cloned() {
                match barrier {
                    Barrier::Broker => broker = Some(RateLimitBrokerBarrier { limit }),
                    Barrier::Asset(code) => assets.push(RateLimitAssetBarrier {
                        limit,
                        settlement_asset: asset(code),
                    }),
                    Barrier::Account(id) => accounts.push(RateLimitAccountBarrier {
                        limit,
                        account_id: account(id),
                    }),
                    Barrier::AccountAsset(id, code) => {
                        account_assets.push(RateLimitAccountAssetBarrier {
                            limit,
                            account_id: account(id),
                            settlement_asset: asset(code),
                        })
                    }
                }
            }
            RateLimitSettings::new(broker, assets, accounts, account_assets).expect("valid config")
        }

        fn asset(code: &str) -> Asset {
            Asset::new(code).expect("asset code must be valid")
        }

        // Reference state of one barrier, in nanoseconds since the policy
        // epoch: a fixed window on the broker and asset axes, a sliding log on
        // the account axes.
        #[derive(Clone)]
        struct ModelBarrier {
            barrier: Barrier,
            limit: RateLimit,
            window_start: u64,
            count: usize,
            log: Vec<u64>,
        }

        impl ModelBarrier {
            fn new(barrier: Barrier, limit: RateLimit) -> Self {
                Self {
                    barrier,
                    limit,
                    window_start: 0,
                    count: 0,
                    log: Vec::new(),
                }
            }

            // Consumes a slot and returns the new count, which is also the
            // count a dry run at the same reading must report.
            fn push(&mut self, now: u64) -> usize {
                let window = self.limit.window;
                match self.barrier {
                    Barrier::Broker | Barrier::Asset(_) => {
                        if expired(now, self.window_start, window) {
                            self.window_start = now;
                            self.count = 0;
                        }
                        self.count += 1;
                        self.count
                    }
                    Barrier::Account(_) | Barrier::AccountAsset(..) => {
                        // Trimmed as of every reading, so an entry once expired
                        // stays gone when a later call reads an earlier clock.
                        self.log.retain(|&entry| !expired(now, entry, window));
                        self.log.push(now);
                        self.log.len()
                    }
                }
            }
        }

        // Every applicable barrier consumes a slot, rejected or not; the
        // verdict is the first breach in priority order.
        fn model_check(
            model: &mut [ModelBarrier],
            account_id: u64,
            settlement: &str,
            now: u64,
        ) -> Result<(), Rejects> {
            let mut verdict = Ok(());
            for barrier in model
                .iter_mut()
                .filter(|barrier| barrier.barrier.applies(account_id, settlement))
            {
                let count = barrier.push(now);
                if verdict.is_ok() && count > barrier.limit.max_orders {
                    verdict = Err(rate_limit_reject(
                        barrier.barrier.name(),
                        u64::try_from(count).expect("order count must fit u64"),
                        u64::try_from(barrier.limit.max_orders).expect("max orders must fit u64"),
                        barrier.limit.window,
                    ));
                }
            }
            verdict
        }

        proptest! {
            #![proptest_config(config(POLICY_CASES))]

            #[test]
            fn policy_matches_reference_model(
                barriers in barrier_limits(),
                steps in prop::collection::vec(
                    (1u64..=3, prop::sample::select(vec!["USD", "EUR", "GBP"]), 0u64..=256),
                    1..=48,
                ),
            ) {
                let policy = policy_from(settings(&barriers));
                let mut model: Vec<_> = barriers
                    .into_iter()
                    .map(|(barrier, limit)| ModelBarrier::new(barrier, limit))
                    .collect();
                for (step, (account_id, settlement, offset)) in steps.into_iter().enumerate() {
                    let o = OrderOperation {
                        instrument: Instrument::new(asset("AAPL"), asset(settlement)),
                        ..order(account(account_id))
                    };
                    let now = policy.epoch + Duration::from_nanos(offset);
                    let dry = check_dry_run_at(&policy, &o, now);
                    let dry_again = check_dry_run_at(&policy, &o, now);
                    let real = check_at(&policy, &o, now);
                    let expected = model_check(&mut model, account_id, settlement, offset);
                    prop_assert_eq!(&dry_again, &dry, "dry run moved state at step {}", step);
                    prop_assert_eq!(
                        &dry,
                        &real,
                        "dry run differs from real check at step {}",
                        step
                    );
                    prop_assert_eq!(&dry, &expected, "dry run differs from model at step {}", step);
                    prop_assert_eq!(
                        &real,
                        &expected,
                        "real check differs from model at step {}",
                        step
                    );
                }
            }
        }
    }
}
