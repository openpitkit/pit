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

use openpit::pretrade::policies::RateLimitPolicy;
use openpit::pretrade::{CheckPreTradeStartPolicy, Reject};

/// Runtime-validated wrapper around [`RateLimitPolicy`].
///
/// `RateLimitPolicy` counts requests inside a sliding time window and
/// does not inspect order or execution-report fields. This guard
/// performs no runtime checks and delegates directly to the inner policy.
///
/// The guard is kept for API uniformity with other guarded policies and
/// for forward-compatibility — future versions of `RateLimitPolicy` may
/// introduce trait bounds that require runtime validation in bindings.
pub struct GuardedRateLimit {
    inner: RateLimitPolicy,
}

impl GuardedRateLimit {
    pub fn new(inner: RateLimitPolicy) -> Self {
        Self { inner }
    }
}

impl<O, R> CheckPreTradeStartPolicy<O, R> for GuardedRateLimit {
    fn name(&self) -> &'static str {
        RateLimitPolicy::NAME
    }

    fn check_pre_trade_start(&self, order: &O) -> Result<(), Reject> {
        <RateLimitPolicy as CheckPreTradeStartPolicy<O, R>>::check_pre_trade_start(
            &self.inner,
            order,
        )
    }

    fn apply_execution_report(&self, report: &R) -> bool {
        <RateLimitPolicy as CheckPreTradeStartPolicy<O, R>>::apply_execution_report(
            &self.inner,
            report,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openpit::pretrade::policies::RateLimitPolicy;
    use openpit::pretrade::RejectCode;
    use std::time::Duration;

    fn check_start(guard: &GuardedRateLimit) -> Result<(), openpit::pretrade::Reject> {
        <GuardedRateLimit as CheckPreTradeStartPolicy<(), ()>>::check_pre_trade_start(guard, &())
    }

    #[test]
    fn name_is_rate_limit_policy_name() {
        let guard = GuardedRateLimit::new(RateLimitPolicy::new(1, Duration::from_secs(60)));
        assert_eq!(
            <GuardedRateLimit as CheckPreTradeStartPolicy<(), ()>>::name(&guard),
            RateLimitPolicy::NAME
        );
    }

    #[test]
    fn delegates_to_inner_policy() {
        let guard = GuardedRateLimit::new(RateLimitPolicy::new(2, Duration::from_secs(60)));
        assert!(check_start(&guard).is_ok());
        assert!(check_start(&guard).is_ok());
        let reject = check_start(&guard).expect_err("must reject after exceeding limit");
        assert_eq!(reject.code, RejectCode::RateLimitExceeded);
    }

    #[test]
    fn apply_execution_report_returns_false_without_reports() {
        let guard = GuardedRateLimit::new(RateLimitPolicy::new(1, Duration::from_secs(60)));
        let result = <GuardedRateLimit as CheckPreTradeStartPolicy<(), ()>>::apply_execution_report(
            &guard,
            &(),
        );
        assert!(!result);
    }
}
