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

use openpit::param::{AccountId, Asset, Price, Quantity, Side, TradeAmount, Volume};
use openpit::pretrade::policies::{
    OrderSizeBrokerBarrier, OrderSizeLimit, OrderSizeLimitPolicy, OrderSizeLimitPolicyError,
    OrderSizeLimitSettings,
};
use openpit::pretrade::{RejectCode, RejectScope, RuleMode};
use openpit::storage::NoLocking;
use openpit::{
    Engine, Instrument, LocalEngine, OrderOperation, OrderReview, PolicyInstance,
    WithOrderOperation, WithOrderReview,
};

type Order = WithOrderOperation<WithOrderReview<()>>;

fn limit(quantity: Option<&str>, notional: Option<&str>) -> OrderSizeLimit {
    OrderSizeLimit {
        max_quantity: quantity.map(|value| Quantity::from_str(value).unwrap()),
        max_notional: notional.map(|value| Volume::from_str(value).unwrap()),
    }
}

fn engine(mode: RuleMode, reject: OrderSizeLimit, review: OrderSizeLimit) -> LocalEngine<Order> {
    let policy = || {
        OrderSizeLimitPolicy::<NoLocking>::new(OrderSizeLimitSettings::new(None, [], []).unwrap())
    };
    let engine = Engine::builder::<Order, (), ()>()
        .with_order_review()
        .no_sync()
        .pre_trade_instance(
            policy(),
            PolicyInstance {
                name: Some("size-reject".to_owned()),
                mode: RuleMode::Reject,
            },
        )
        .pre_trade_instance(
            policy(),
            PolicyInstance {
                name: Some("size-review".to_owned()),
                mode,
            },
        )
        .build()
        .unwrap();
    for (name, limit) in [("size-reject", reject), ("size-review", review)] {
        engine
            .configure()
            .order_size_limit::<OrderSizeLimitPolicyError>(name, |settings| {
                settings.set_broker(Some(OrderSizeBrokerBarrier { limit }))
            })
            .unwrap();
    }
    engine
}

fn order(confirmed: bool, price: Option<&str>) -> Order {
    WithOrderOperation {
        operation: OrderOperation {
            instrument: Instrument::new(Asset::new("AAPL").unwrap(), Asset::new("USD").unwrap()),
            account_id: AccountId::from_u64(7),
            side: Side::Buy,
            trade_amount: TradeAmount::Quantity(Quantity::from_str("11").unwrap()),
            price: price.map(|value| Price::from_str(value).unwrap()),
        },
        inner: WithOrderReview {
            inner: (),
            review: OrderReview {
                confirmed,
                already_held: false,
            },
        },
    }
}

#[test]
fn review_breach_only_returns_mark() {
    for mode in [RuleMode::ReviewWithHold, RuleMode::ReviewWithoutHold] {
        for (review, code) in [
            (limit(Some("10"), None), RejectCode::OrderQtyExceedsLimit),
            (
                limit(None, Some("20")),
                RejectCode::OrderNotionalExceedsLimit,
            ),
            (limit(Some("10"), Some("20")), RejectCode::OrderExceedsLimit),
        ] {
            let engine = engine(mode, limit(Some("100"), None), review);
            let request = engine.start_pre_trade(order(false, Some("2"))).unwrap();
            let reviews = request.reviews();
            assert_eq!(reviews.len(), 1);
            assert_eq!(reviews[0].mode, mode);
            assert_eq!(reviews[0].code, code);
            assert_eq!(reviews[0].scope, RejectScope::Order);
            assert_eq!(reviews[0].policy, OrderSizeLimitPolicy::<NoLocking>::NAME);
        }
    }
}

#[test]
fn reject_instance_wins_over_review_marks() {
    for mode in [RuleMode::ReviewWithHold, RuleMode::ReviewWithoutHold] {
        let engine = engine(mode, limit(Some("10"), None), limit(Some("5"), None));
        let rejects = engine.start_pre_trade(order(false, Some("2"))).unwrap_err();
        assert_eq!(rejects.len(), 1);
        assert_eq!(rejects[0].mode, RuleMode::Reject);
        assert_eq!(rejects[0].code, RejectCode::OrderQtyExceedsLimit);
        assert_eq!(rejects[0].policy, OrderSizeLimitPolicy::<NoLocking>::NAME);
    }
}

#[test]
fn review_calculation_failure_is_a_reject() {
    for mode in [RuleMode::ReviewWithHold, RuleMode::ReviewWithoutHold] {
        let engine = engine(mode, limit(Some("100"), None), limit(None, Some("20")));
        let rejects = engine.start_pre_trade(order(false, None)).unwrap_err();
        assert_eq!(rejects.len(), 1);
        assert_eq!(rejects[0].mode, RuleMode::Reject);
        assert_eq!(rejects[0].code, RejectCode::OrderValueCalculationFailed);
    }
}

#[test]
fn confirmed_drops_marks_but_keeps_reject_instance() {
    for mode in [RuleMode::ReviewWithHold, RuleMode::ReviewWithoutHold] {
        let engine = engine(mode, limit(Some("100"), None), limit(Some("5"), None));
        let request = engine.start_pre_trade(order(true, Some("2"))).unwrap();
        assert!(request.reviews().is_empty());
        drop(request);
        engine
            .configure()
            .order_size_limit::<OrderSizeLimitPolicyError>("size-reject", |settings| {
                settings.set_broker(Some(OrderSizeBrokerBarrier {
                    limit: limit(Some("10"), None),
                }))
            })
            .unwrap();
        let rejects = engine.start_pre_trade(order(true, Some("2"))).unwrap_err();
        assert_eq!(rejects.len(), 1);
        assert_eq!(rejects[0].mode, RuleMode::Reject);
        assert_eq!(rejects[0].code, RejectCode::OrderQtyExceedsLimit);
    }
}
