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

use openpit::pretrade::policies::OrderSizeLimitPolicy;
use openpit::pretrade::{CheckPreTradeStartPolicy, Reject, RejectCode, RejectScope};
use openpit::{HasInstrument, HasOrderPrice, HasTradeAmount};

use crate::OrderGroupAccess;

/// Runtime-validated wrapper around [`OrderSizeLimitPolicy`].
///
/// Checks that the order carries the operation group (instrument,
/// trade_amount, and price) before delegating to the inner policy.
pub struct GuardedOrderSizeLimit {
    inner: OrderSizeLimitPolicy,
}

impl GuardedOrderSizeLimit {
    pub fn new(inner: OrderSizeLimitPolicy) -> Self {
        Self { inner }
    }
}

impl<O, R> CheckPreTradeStartPolicy<O, R> for GuardedOrderSizeLimit
where
    O: HasInstrument + HasTradeAmount + HasOrderPrice + OrderGroupAccess,
{
    fn name(&self) -> &'static str {
        OrderSizeLimitPolicy::NAME
    }

    fn check_pre_trade_start(&self, order: &O) -> Result<(), Reject> {
        if !order.has_operation() {
            return Err(Reject::new(
                OrderSizeLimitPolicy::NAME,
                RejectScope::Order,
                RejectCode::MissingRequiredField,
                "insufficient order data",
                "order operation group is required for order size limit evaluation",
            ));
        }
        <OrderSizeLimitPolicy as CheckPreTradeStartPolicy<O, R>>::check_pre_trade_start(
            &self.inner,
            order,
        )
    }

    fn apply_execution_report(&self, report: &R) -> bool {
        <OrderSizeLimitPolicy as CheckPreTradeStartPolicy<O, R>>::apply_execution_report(
            &self.inner,
            report,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openpit::param::{AccountId, Asset, Price, Quantity, Side, TradeAmount, Volume};
    use openpit::pretrade::policies::{OrderSizeLimit, OrderSizeLimitPolicy};
    use openpit::pretrade::RejectCode;
    use openpit::{HasInstrument, HasOrderPrice, HasTradeAmount, Instrument, OrderOperation};

    struct FakeOrder {
        operation: Option<OrderOperation>,
    }

    impl OrderGroupAccess for FakeOrder {
        fn has_operation(&self) -> bool {
            self.operation.is_some()
        }
    }

    impl HasInstrument for FakeOrder {
        fn instrument(&self) -> &Instrument {
            &self
                .operation
                .as_ref()
                .expect("internal error: test order must have operation set")
                .instrument
        }
    }

    impl HasTradeAmount for FakeOrder {
        fn trade_amount(&self) -> TradeAmount {
            self.operation
                .as_ref()
                .expect("internal error: test order must have operation set")
                .trade_amount
        }
    }

    impl HasOrderPrice for FakeOrder {
        fn price(&self) -> Option<Price> {
            self.operation
                .as_ref()
                .expect("internal error: test order must have operation set")
                .price
        }
    }

    fn check_start(
        guard: &GuardedOrderSizeLimit,
        order: &FakeOrder,
    ) -> Result<(), openpit::pretrade::Reject> {
        <GuardedOrderSizeLimit as CheckPreTradeStartPolicy<FakeOrder, ()>>::check_pre_trade_start(
            guard, order,
        )
    }

    fn sample_operation() -> OrderOperation {
        OrderOperation {
            account_id: AccountId::from_u64(99224416),
            instrument: Instrument::new(
                Asset::new("AAPL").expect("valid"),
                Asset::new("USD").expect("valid"),
            ),
            side: Side::Buy,
            trade_amount: TradeAmount::Quantity(Quantity::from_str("100").expect("valid")),
            price: Some(Price::from_str("185").expect("valid")),
        }
    }

    fn make_guard() -> GuardedOrderSizeLimit {
        GuardedOrderSizeLimit::new(OrderSizeLimitPolicy::new(
            OrderSizeLimit {
                settlement_asset: Asset::new("USD").expect("valid"),
                max_quantity: Quantity::from_str("1000").expect("valid"),
                max_notional: Volume::from_str("500000").expect("valid"),
            },
            [],
        ))
    }

    #[test]
    fn rejects_when_operation_missing() {
        let guard = make_guard();
        let order = FakeOrder { operation: None };
        let reject =
            check_start(&guard, &order).expect_err("must reject when operation group is absent");
        assert_eq!(reject.code, RejectCode::MissingRequiredField);
    }

    #[test]
    fn name_is_order_size_limit_policy_name() {
        let guard = make_guard();
        assert_eq!(
            <GuardedOrderSizeLimit as CheckPreTradeStartPolicy<FakeOrder, ()>>::name(&guard),
            OrderSizeLimitPolicy::NAME
        );
    }

    #[test]
    fn apply_execution_report_always_returns_false() {
        let guard = make_guard();
        let result =
            <GuardedOrderSizeLimit as CheckPreTradeStartPolicy<FakeOrder, ()>>::apply_execution_report(
                &guard, &(),
            );
        assert!(!result);
    }

    #[test]
    fn delegates_when_operation_present() {
        let guard = make_guard();
        let order = FakeOrder {
            operation: Some(sample_operation()),
        };
        assert!(check_start(&guard, &order).is_ok());
    }

    #[test]
    fn rejects_when_order_exceeds_quantity_limit() {
        let guard = GuardedOrderSizeLimit::new(OrderSizeLimitPolicy::new(
            OrderSizeLimit {
                settlement_asset: Asset::new("USD").expect("valid"),
                max_quantity: Quantity::from_str("10").expect("valid"),
                max_notional: Volume::from_str("500000").expect("valid"),
            },
            [],
        ));
        let order = FakeOrder {
            operation: Some(OrderOperation {
                account_id: AccountId::from_u64(99224416),
                instrument: Instrument::new(
                    Asset::new("AAPL").expect("valid"),
                    Asset::new("USD").expect("valid"),
                ),
                side: Side::Buy,
                trade_amount: TradeAmount::Quantity(Quantity::from_str("100").expect("valid")),
                price: Some(Price::from_str("185").expect("valid")),
            }),
        };
        let reject =
            check_start(&guard, &order).expect_err("must reject when quantity exceeds limit");
        assert_eq!(reject.code, RejectCode::OrderQtyExceedsLimit);
    }
}
