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

use std::cell::RefCell;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::pretrade::start_pre_trade_time::start_pre_trade_now;
use crate::pretrade::{CheckPreTradeStartPolicy, Reject, RejectCode, RejectScope};

/// Start-stage policy that limits order rate in a sliding time window.
///
/// Every call to `check_pre_trade_start`, including rejected ones, consumes a
/// slot in the window. This ensures flood attempts cannot bypass the counter.
///
/// # Examples
///
/// ```rust
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use std::time::Duration;
/// use openpit::param::{Asset, Price, Quantity, Side};
/// use openpit::pretrade::policies::RateLimitPolicy;
/// use openpit::{Engine, Instrument};
/// use openpit::OrderOperation;
/// use openpit::param::TradeAmount;
///
/// let engine = Engine::<OrderOperation>::builder()
///     .check_pre_trade_start_policy(RateLimitPolicy::new(2, Duration::from_secs(60)))
///     .build()?;
///
/// let order = OrderOperation {
///     instrument: Instrument::new(
///         Asset::new("AAPL")?,
///         Asset::new("USD")?,
///     ),
///     account_id: openpit::param::AccountId::from_u64(99224416),
///     side: Side::Buy,
///     trade_amount: TradeAmount::Quantity(
///         Quantity::from_str("1")?,
///     ),
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
pub struct RateLimitPolicy {
    timestamps: RefCell<VecDeque<Instant>>,
    window: Duration,
    max_orders: usize,
}

impl RateLimitPolicy {
    /// Stable policy name.
    pub const NAME: &'static str = "RateLimitPolicy";

    /// Creates a rate-limit policy with `max_orders` in a `window`.
    pub fn new(max_orders: usize, window: Duration) -> Self {
        Self {
            timestamps: RefCell::new(VecDeque::new()),
            window,
            max_orders,
        }
    }
}

impl<O, R> CheckPreTradeStartPolicy<O, R> for RateLimitPolicy {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn check_pre_trade_start(&self, _order: &O) -> Result<(), Reject> {
        let now = start_pre_trade_now();
        let mut timestamps = self.timestamps.borrow_mut();

        while let Some(oldest) = timestamps.front().copied() {
            match now.checked_duration_since(oldest) {
                Some(elapsed) if elapsed >= self.window => {
                    timestamps.pop_front();
                }
                _ => break,
            }
        }

        timestamps.push_back(now);
        if timestamps.len() > self.max_orders {
            return Err(Reject::new(
                Self::NAME,
                RejectScope::Order,
                RejectCode::RateLimitExceeded,
                "rate limit exceeded",
                format!(
                    "submitted {} orders in {:?} window, max allowed: {}",
                    timestamps.len(),
                    self.window,
                    self.max_orders
                ),
            ));
        }

        Ok(())
    }

    fn apply_execution_report(&self, _report: &R) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use crate::core::OrderOperation;
    use crate::param::{AccountId, Asset, Quantity, Side, TradeAmount};
    use crate::pretrade::start_pre_trade_time::with_start_pre_trade_now;
    use crate::pretrade::{CheckPreTradeStartPolicy, Reject, RejectCode, RejectScope};

    use super::RateLimitPolicy;

    #[test]
    fn sliding_window_rejects_when_limit_is_exceeded() {
        let policy = RateLimitPolicy::new(2, Duration::from_secs(10));
        let order = order("USD");
        let base = Instant::now();

        assert!(check_at(&policy, &order, base).is_ok());
        assert!(check_at(&policy, &order, base + Duration::from_secs(1)).is_ok());

        let reject = check_at(&policy, &order, base + Duration::from_secs(2))
            .expect_err("third order in window must be rejected");
        assert_eq!(reject.scope, RejectScope::Order);
        assert_eq!(reject.code, RejectCode::RateLimitExceeded);
        assert_eq!(reject.reason, "rate limit exceeded");
        assert_eq!(
            reject.details,
            "submitted 3 orders in 10s window, max allowed: 2"
        );
    }

    #[test]
    fn expired_timestamps_leave_sliding_window() {
        let policy = RateLimitPolicy::new(2, Duration::from_secs(10));
        let order = order("USD");
        let base = Instant::now();

        assert!(check_at(&policy, &order, base).is_ok());
        assert!(check_at(&policy, &order, base + Duration::from_secs(1)).is_ok());
        assert!(check_at(&policy, &order, base + Duration::from_secs(11)).is_ok());
    }

    #[test]
    fn rejected_attempts_are_counted_and_not_rolled_back() {
        let policy = RateLimitPolicy::new(1, Duration::from_secs(3));
        let order = order("USD");
        let base = Instant::now();

        assert!(check_at(&policy, &order, base).is_ok());
        assert!(check_at(&policy, &order, base + Duration::from_secs(1)).is_err());

        let reject = check_at(&policy, &order, base + Duration::from_millis(3500))
            .expect_err("rejected attempt must stay counted in the window");
        assert_eq!(reject.scope, RejectScope::Order);
        assert_eq!(reject.code, RejectCode::RateLimitExceeded);
        assert_eq!(reject.reason, "rate limit exceeded");
        assert_eq!(
            reject.details,
            "submitted 2 orders in 3s window, max allowed: 1"
        );
    }

    fn check_at(
        policy: &RateLimitPolicy,
        order: &OrderOperation,
        now: Instant,
    ) -> Result<(), Reject> {
        with_start_pre_trade_now(now, || {
            <RateLimitPolicy as CheckPreTradeStartPolicy<OrderOperation, ()>>::check_pre_trade_start(
                policy, order,
            )
        })
    }

    fn order(settlement: &str) -> OrderOperation {
        OrderOperation {
            instrument: crate::Instrument::new(
                Asset::new("AAPL").expect("asset code must be valid"),
                Asset::new(settlement).expect("asset code must be valid"),
            ),
            account_id: AccountId::from_u64(99224416),
            side: Side::Buy,
            trade_amount: TradeAmount::Quantity(
                Quantity::from_str("1").expect("quantity literal must be valid"),
            ),
            price: None,
        }
    }
}
