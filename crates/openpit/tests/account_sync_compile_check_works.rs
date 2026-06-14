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

//! Smoke test that the built-in stateful policies remain compatible with
//! [`AccountSync`](openpit::AccountSync) after the storage
//! layer's `IndexLocking<KeyBound>` parameterisation. Failure here means a
//! built-in policy stopped using an account-shaped storage key, which the
//! account-sync compile-time bound now forbids.

use std::time::Duration;

use openpit::param::{AccountId, Asset, Pnl};
use openpit::pretrade::policies::{
    PnlBoundsAccountAssetBarrier, PnlBoundsBrokerBarrier, PnlBoundsKillSwitchPolicy,
    PnlBoundsKillSwitchSettings, RateLimit, RateLimitAccountAssetBarrier, RateLimitAccountBarrier,
    RateLimitBrokerBarrier, RateLimitPolicy, RateLimitSettings,
};
use openpit::{Engine, WithExecutionReportOperation, WithFinancialImpact};

type Report = WithExecutionReportOperation<WithFinancialImpact<()>>;

#[test]
fn account_sync_engine_with_rate_limit_and_pnl_bounds_builds() {
    let usd = Asset::new("USD").expect("asset");
    let account = AccountId::from_u64(1);

    let builder = Engine::builder::<openpit::OrderOperation, Report, ()>().account_sync();

    let rate_limit = RateLimitPolicy::new(
        RateLimitSettings::new(
            Some(RateLimitBrokerBarrier {
                limit: RateLimit {
                    max_orders: 100,
                    window: Duration::from_secs(1),
                },
            }),
            [],
            [RateLimitAccountBarrier {
                limit: RateLimit {
                    max_orders: 10,
                    window: Duration::from_secs(1),
                },
                account_id: account,
            }],
            [RateLimitAccountAssetBarrier {
                limit: RateLimit {
                    max_orders: 5,
                    window: Duration::from_secs(1),
                },
                account_id: account,
                settlement_asset: usd.clone(),
            }],
        )
        .expect("rate-limit settings"),
        builder.storage_builder(),
    );

    let pnl_bounds = PnlBoundsKillSwitchPolicy::new(
        PnlBoundsKillSwitchSettings::new(
            [PnlBoundsBrokerBarrier {
                settlement_asset: usd.clone(),
                lower_bound: Some(Pnl::from_str("-1000").expect("pnl")),
                upper_bound: None,
            }],
            [PnlBoundsAccountAssetBarrier {
                barrier: PnlBoundsBrokerBarrier {
                    settlement_asset: usd.clone(),
                    lower_bound: Some(Pnl::from_str("-200").expect("pnl")),
                    upper_bound: None,
                },
                account_id: account,
                initial_pnl: Pnl::from_str("0").expect("pnl"),
            }],
        )
        .expect("pnl-bounds settings"),
        builder.storage_builder(),
    );

    let _engine = builder
        .pre_trade(rate_limit)
        .pre_trade(pnl_bounds)
        .build()
        .expect("engine builds");
}
