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

use std::collections::{HashMap, VecDeque};
use std::fmt::{Display, Formatter};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use crate::core::{HasAccountId, HasInstrument};
use crate::param::{AccountId, Asset};
use crate::pretrade::policy::{missing_required_field_reject, PolicyGroupId, PolicyName};
use crate::pretrade::start_pre_trade_time::start_pre_trade_now;
use crate::pretrade::DEFAULT_POLICY_GROUP_ID;
use crate::pretrade::{PreTradeContext, PreTradePolicy, Reject, RejectCode, RejectScope, Rejects};
use crate::storage::{Storage, StorageBuilder};

type StoragePolicy<LPF> = <LPF as crate::storage::LockingPolicyFactory>::Policy;
type TimestampStorage<K, LPF> = Storage<K, VecDeque<Instant>, StoragePolicy<LPF>>;

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

/// Errors returned by [`RateLimitPolicy`] construction.
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

// Approximate fixed-window counter backed by two AtomicU64s.
//
// `window_start_nanos` holds nanoseconds elapsed since the epoch captured at
// policy construction. On window rollover, `window_start_nanos` is CAS-updated
// and `count` is reset to 1. At the window boundary, two concurrent threads can
// both observe the window as expired and both reset `count`; the observed burst
// can briefly reach up to `2 * max_orders` worst case. This is a deliberate
// trade-off for lock-free shared accounting.
struct AtomicWindowCounter {
    count: AtomicU64,
    window_start_nanos: AtomicU64,
    limit: RateLimit,
}

impl AtomicWindowCounter {
    fn new(limit: RateLimit, now_nanos: u64) -> Self {
        Self {
            count: AtomicU64::new(0),
            window_start_nanos: AtomicU64::new(now_nanos),
            limit,
        }
    }

    fn push(&self, now_nanos: u64) -> u64 {
        let window_nanos = self.limit.window.as_nanos() as u64;
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
}

/// Tracks order rates across four independent axes in time windows.
///
/// Four configurable barrier axes — broker (all orders), per settlement asset,
/// per account, and per (account, settlement asset). Every applicable axis is
/// incremented and checked on every call to `check_pre_trade_start`. An order
/// is rejected if any applicable axis breaches its `max_orders` over its
/// `window`. Every call, including rejected ones, consumes a slot in every
/// applicable axis so that flood attempts cannot bypass any counter.
///
/// Broker and asset axes use approximate fixed-window counters backed by
/// `AtomicU64` pairs. They are atomic and lock-free; at a window boundary, the
/// observed burst can briefly reach up to `2 * max_orders` worst case. Account
/// and account+asset axes use precise sliding-window logs via [`Storage`].
///
/// Constructor rules:
/// - at least one barrier across all four axes must be configured;
/// - if all are omitted, the constructor returns
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
///     RateLimit, RateLimitBrokerBarrier, RateLimitPolicy,
/// };
/// use openpit::{Engine, Instrument};
/// use openpit::OrderOperation;
/// use openpit::param::TradeAmount;
///
/// let builder = Engine::builder::<OrderOperation, (), ()>().no_sync();
/// let policy = RateLimitPolicy::new(
///     Some(RateLimitBrokerBarrier {
///         limit: RateLimit { max_orders: 2, window: Duration::from_secs(60) },
///     }),
///     [],
///     [],
///     [],
///     builder.storage_builder(),
/// )?;
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
    epoch: Instant,
    broker_counter: Option<AtomicWindowCounter>,
    asset_counters: HashMap<Asset, AtomicWindowCounter>,
    account_barriers: HashMap<AccountId, RateLimit>,
    per_account_timestamps: Option<TimestampStorage<AccountId, LockingPolicyFactory>>,
    account_asset_barriers: HashMap<(AccountId, Asset), RateLimit>,
    per_account_asset_timestamps:
        Option<TimestampStorage<(AccountId, Asset), LockingPolicyFactory>>,
    group_id: PolicyGroupId,
}

impl<LockingPolicyFactory> RateLimitPolicy<LockingPolicyFactory>
where
    LockingPolicyFactory: crate::storage::LockingPolicyFactory,
{
    /// Stable policy name.
    pub const NAME: &'static str = "RateLimitPolicy";

    /// Creates a rate-limit policy.
    ///
    /// `storage_builder` must be obtained from the engine builder so that the
    /// per-account and per-(account, asset) timestamp storages share the factory
    /// type with the engine's synchronization mode.
    ///
    /// At least one barrier must be provided across all four axes. If all are
    /// `None` or empty, returns [`RateLimitPolicyError::NoBarriersConfigured`].
    /// Duplicate keys within an axis: last-write-wins (no error).
    pub fn new(
        broker: Option<RateLimitBrokerBarrier>,
        asset_barriers: impl IntoIterator<Item = RateLimitAssetBarrier>,
        account_barriers: impl IntoIterator<Item = RateLimitAccountBarrier>,
        account_asset_barriers: impl IntoIterator<Item = RateLimitAccountAssetBarrier>,
        storage_builder: &StorageBuilder<LockingPolicyFactory>,
    ) -> Result<Self, RateLimitPolicyError>
    where
        LockingPolicyFactory: crate::storage::CreateStorageFor<AccountId>
            + crate::storage::CreateStorageFor<(AccountId, Asset)>,
    {
        let epoch = Instant::now();
        let now_nanos = 0u64;

        let broker_counter = broker
            .map(|b| {
                validate_limit(b.limit).map(|limit| AtomicWindowCounter::new(limit, now_nanos))
            })
            .transpose()?;

        let asset_counters: HashMap<Asset, AtomicWindowCounter> = asset_barriers
            .into_iter()
            .map(|b| {
                validate_limit(b.limit).map(|limit| {
                    (
                        b.settlement_asset,
                        AtomicWindowCounter::new(limit, now_nanos),
                    )
                })
            })
            .collect::<Result<_, _>>()?;

        let account_barriers_map: HashMap<AccountId, RateLimit> = account_barriers
            .into_iter()
            .map(|b| validate_limit(b.limit).map(|limit| (b.account_id, limit)))
            .collect::<Result<_, _>>()?;

        let account_asset_barriers_map: HashMap<(AccountId, Asset), RateLimit> =
            account_asset_barriers
                .into_iter()
                .map(|b| {
                    validate_limit(b.limit).map(|limit| ((b.account_id, b.settlement_asset), limit))
                })
                .collect::<Result<_, _>>()?;

        if broker_counter.is_none()
            && asset_counters.is_empty()
            && account_barriers_map.is_empty()
            && account_asset_barriers_map.is_empty()
        {
            return Err(RateLimitPolicyError::NoBarriersConfigured);
        }

        Ok(Self {
            epoch,
            broker_counter,
            asset_counters,
            per_account_timestamps: (!account_barriers_map.is_empty())
                .then(|| storage_builder.create_for_bound_key()),
            account_barriers: account_barriers_map,
            per_account_asset_timestamps: (!account_asset_barriers_map.is_empty())
                .then(|| storage_builder.create_for_bound_key()),
            account_asset_barriers: account_asset_barriers_map,
            group_id: DEFAULT_POLICY_GROUP_ID,
        })
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

impl<Order, ExecutionReport, AccountAdjustment, LockingPolicyFactory, Sync>
    PreTradePolicy<Order, ExecutionReport, AccountAdjustment, Sync>
    for RateLimitPolicy<LockingPolicyFactory>
where
    Order: HasAccountId + HasInstrument,
    LockingPolicyFactory: crate::storage::LockingPolicyFactory,
    Sync: crate::core::SyncMode,
{
    fn name(&self) -> &str {
        Self::NAME
    }

    fn policy_group_id(&self) -> PolicyGroupId {
        self.group_id
    }

    fn check_pre_trade_start(
        &self,
        _ctx: &PreTradeContext<<Sync as crate::core::SyncMode>::StorageLockingPolicyFactory>,
        order: &Order,
    ) -> Result<(), Rejects> {
        let settlement_opt: Option<Asset> =
            if !self.asset_counters.is_empty() || !self.account_asset_barriers.is_empty() {
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
        let account_id_opt: Option<AccountId> =
            if self.per_account_timestamps.is_some() || !self.account_asset_barriers.is_empty() {
                Some(order.account_id().map_err(|e| {
                    Rejects::from(missing_required_field_reject(self, "account ID", &e))
                })?)
            } else {
                None
            };

        let now = start_pre_trade_now();
        let now_nanos = now
            .checked_duration_since(self.epoch)
            .unwrap_or_default()
            .as_nanos() as u64;

        // Push into ALL applicable axes before checking (flood semantics).
        let broker_push = self
            .broker_counter
            .as_ref()
            .map(|c| (c.push(now_nanos), c.limit.max_orders, c.limit.window));

        let asset_push = settlement_opt.as_ref().and_then(|settlement| {
            self.asset_counters
                .get(settlement)
                .map(|c| (c.push(now_nanos), c.limit.max_orders, c.limit.window))
        });

        let account_push = account_id_opt.and_then(|account_id| {
            self.per_account_timestamps.as_ref().and_then(|storage| {
                self.account_barriers.get(&account_id).map(|barrier| {
                    storage.with_mut(account_id, VecDeque::new, |entry, _is_new| {
                        advance_window(entry, now, barrier.window);
                        entry.push_back(now);
                        (entry.len(), barrier.max_orders, barrier.window)
                    })
                })
            })
        });

        let account_asset_push = account_id_opt.and_then(|account_id| {
            settlement_opt.as_ref().and_then(|settlement| {
                self.per_account_asset_timestamps
                    .as_ref()
                    .and_then(|storage| {
                        self.account_asset_barriers
                            .get(&(account_id, settlement.clone()))
                            .map(|barrier| {
                                storage.with_mut(
                                    (account_id, settlement.clone()),
                                    VecDeque::new,
                                    |entry, _is_new| {
                                        advance_window(entry, now, barrier.window);
                                        entry.push_back(now);
                                        (entry.len(), barrier.max_orders, barrier.window)
                                    },
                                )
                            })
                    })
            })
        });

        // Check in priority order: broker → asset → account → account+asset.
        if let Some((count, max_orders, window)) = broker_push {
            if count > max_orders as u64 {
                return Err(rate_limit_reject(
                    Self::NAME,
                    RejectScope::Order,
                    "rate limit exceeded: broker barrier",
                    count,
                    max_orders as u64,
                    window,
                ));
            }
        }

        if let Some((count, max_orders, window)) = asset_push {
            if count > max_orders as u64 {
                return Err(rate_limit_reject(
                    Self::NAME,
                    RejectScope::Order,
                    "rate limit exceeded: asset barrier",
                    count,
                    max_orders as u64,
                    window,
                ));
            }
        }

        if let Some((count, max_orders, window)) = account_push {
            if count > max_orders {
                return Err(rate_limit_reject(
                    Self::NAME,
                    RejectScope::Account,
                    "rate limit exceeded: account barrier",
                    count as u64,
                    max_orders as u64,
                    window,
                ));
            }
        }

        if let Some((count, max_orders, window)) = account_asset_push {
            if count > max_orders {
                return Err(rate_limit_reject(
                    Self::NAME,
                    RejectScope::Account,
                    "rate limit exceeded: account+asset barrier",
                    count as u64,
                    max_orders as u64,
                    window,
                ));
            }
        }

        Ok(())
    }
}

fn rate_limit_reject(
    name: &'static str,
    scope: RejectScope,
    reason: &'static str,
    count: u64,
    max_orders: u64,
    window: Duration,
) -> Rejects {
    Reject::new(
        name,
        scope,
        RejectCode::RateLimitExceeded,
        reason,
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

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use crate::core::{Instrument, OrderOperation};
    use crate::param::{AccountId, Asset, Quantity, Side, TradeAmount};
    use crate::pretrade::start_pre_trade_time::with_start_pre_trade_now;
    use crate::pretrade::{PreTradeContext, PreTradePolicy, RejectCode, RejectScope, Rejects};
    use crate::storage::NoLocking;

    use super::{
        RateLimit, RateLimitAccountAssetBarrier, RateLimitAccountBarrier, RateLimitAssetBarrier,
        RateLimitBrokerBarrier, RateLimitPolicy, RateLimitPolicyError,
    };

    type TestPolicy = RateLimitPolicy<NoLocking>;

    fn test_builder() -> crate::SyncedEngineBuilder<OrderOperation, (), (), crate::LocalSync> {
        crate::Engine::builder().no_sync()
    }

    // ── constructor validation ─────────────────────────────────────────────

    #[test]
    fn zero_window_rejected_by_constructor() {
        let err = RateLimitPolicy::<NoLocking>::new(
            Some(RateLimitBrokerBarrier {
                limit: RateLimit {
                    max_orders: 1,
                    window: Duration::ZERO,
                },
            }),
            [],
            [],
            [],
            test_builder().storage_builder(),
        )
        .err()
        .expect("must fail");

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
    fn sub_microsecond_window_accepted_by_constructor() {
        let result = RateLimitPolicy::<NoLocking>::new(
            Some(RateLimitBrokerBarrier {
                limit: RateLimit {
                    max_orders: 1,
                    window: Duration::from_nanos(500),
                },
            }),
            [],
            [],
            [],
            test_builder().storage_builder(),
        );

        assert!(result.is_ok());
    }

    #[test]
    fn excessive_window_rejected_by_constructor() {
        let max_plus_one = Duration::new(u64::MAX, 0) + Duration::from_nanos(1);
        let err = RateLimitPolicy::<NoLocking>::new(
            Some(RateLimitBrokerBarrier {
                limit: RateLimit {
                    max_orders: 1,
                    window: max_plus_one,
                },
            }),
            [],
            [],
            [],
            test_builder().storage_builder(),
        )
        .err()
        .expect("must fail");

        assert!(matches!(err, RateLimitPolicyError::InvalidWindow { .. }));
    }

    #[test]
    fn no_barriers_configured_rejected_by_constructor() {
        let err =
            RateLimitPolicy::<NoLocking>::new(None, [], [], [], test_builder().storage_builder())
                .err()
                .expect("must fail");
        assert_eq!(err, RateLimitPolicyError::NoBarriersConfigured);
        assert_eq!(
            err.to_string(),
            "at least one broker, asset, account, or account+asset barrier must be configured"
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
        let policy = RateLimitPolicy::new(
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
            test_builder().storage_builder(),
        )
        .expect("valid config");
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
        let policy = RateLimitPolicy::new(
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
            test_builder().storage_builder(),
        )
        .expect("valid config");
        let base = Instant::now();

        assert!(check_at(&policy, &order(account(1)), base).is_ok());
        // Both broker and account breach — broker is reported (checked first).
        let reject = check_at(&policy, &order(account(1)), base + Duration::from_secs(1))
            .expect_err("must reject");
        assert_eq!(reject[0].scope, RejectScope::Order);
        assert_eq!(reject[0].reason, "rate limit exceeded: broker barrier");
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

    fn broker_policy(max_orders: usize, window: Duration) -> TestPolicy {
        RateLimitPolicy::new(
            Some(RateLimitBrokerBarrier {
                limit: RateLimit { max_orders, window },
            }),
            [],
            [],
            [],
            test_builder().storage_builder(),
        )
        .expect("valid config")
    }

    fn asset_policy(settlement: &str, max_orders: usize, window: Duration) -> TestPolicy {
        RateLimitPolicy::new(
            None,
            [RateLimitAssetBarrier {
                limit: RateLimit { max_orders, window },
                settlement_asset: Asset::new(settlement).expect("asset code must be valid"),
            }],
            [],
            [],
            test_builder().storage_builder(),
        )
        .expect("valid config")
    }

    fn account_policy(account_id: AccountId, max_orders: usize, window: Duration) -> TestPolicy {
        RateLimitPolicy::new(
            None,
            [],
            [RateLimitAccountBarrier {
                account_id,
                limit: RateLimit { max_orders, window },
            }],
            [],
            test_builder().storage_builder(),
        )
        .expect("valid config")
    }

    fn account_asset_policy(
        account_id: AccountId,
        settlement: &str,
        max_orders: usize,
        window: Duration,
    ) -> TestPolicy {
        RateLimitPolicy::new(
            None,
            [],
            [],
            [RateLimitAccountAssetBarrier {
                account_id,
                settlement_asset: Asset::new(settlement).expect("asset code must be valid"),
                limit: RateLimit { max_orders, window },
            }],
            test_builder().storage_builder(),
        )
        .expect("valid config")
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
