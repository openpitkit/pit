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

use openpit::pretrade::policies::OrderValidationPolicy;
use openpit::pretrade::{CheckPreTradeStartPolicy, Reject, RejectCode, RejectScope};
use openpit::HasTradeAmount;

use crate::OrderGroupAccess;

/// Runtime-validated wrapper around [`OrderValidationPolicy`].
///
/// Checks that the order carries the operation group before delegating
/// to the inner policy. Without the operation group, `trade_amount` is
/// unavailable and the inner policy would not be able to validate
/// the order.
pub struct GuardedOrderValidation {
    inner: OrderValidationPolicy,
}

impl GuardedOrderValidation {
    pub fn new() -> Self {
        Self {
            inner: OrderValidationPolicy::new(),
        }
    }
}

impl Default for GuardedOrderValidation {
    fn default() -> Self {
        Self::new()
    }
}

impl<O, R> CheckPreTradeStartPolicy<O, R> for GuardedOrderValidation
where
    O: HasTradeAmount + OrderGroupAccess,
{
    fn name(&self) -> &'static str {
        OrderValidationPolicy::NAME
    }

    fn check_pre_trade_start(&self, order: &O) -> Result<(), Reject> {
        if !order.has_operation() {
            return Err(Reject::new(
                OrderValidationPolicy::NAME,
                RejectScope::Order,
                RejectCode::MissingRequiredField,
                "insufficient order data",
                "order operation group is required for order validation",
            ));
        }
        <OrderValidationPolicy as CheckPreTradeStartPolicy<O, R>>::check_pre_trade_start(
            &self.inner,
            order,
        )
    }

    fn apply_execution_report(&self, report: &R) -> bool {
        <OrderValidationPolicy as CheckPreTradeStartPolicy<O, R>>::apply_execution_report(
            &self.inner,
            report,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openpit::param::{AccountId, Asset, Price, Quantity, Side, TradeAmount};
    use openpit::pretrade::RejectCode;
    use openpit::{HasTradeAmount, Instrument, OrderOperation};

    struct FakeOrder {
        operation: Option<OrderOperation>,
    }

    impl OrderGroupAccess for FakeOrder {
        fn has_operation(&self) -> bool {
            self.operation.is_some()
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

    fn check_start(
        guard: &GuardedOrderValidation,
        order: &FakeOrder,
    ) -> Result<(), openpit::pretrade::Reject> {
        <GuardedOrderValidation as CheckPreTradeStartPolicy<FakeOrder, ()>>::check_pre_trade_start(
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

    #[test]
    fn rejects_when_operation_missing() {
        let guard = GuardedOrderValidation::new();
        let order = FakeOrder { operation: None };
        let reject =
            check_start(&guard, &order).expect_err("must reject when operation group is absent");
        assert_eq!(reject.code, RejectCode::MissingRequiredField);
    }

    #[test]
    fn default_is_same_as_new() {
        let guard = GuardedOrderValidation::default();
        let order = FakeOrder {
            operation: Some(sample_operation()),
        };
        assert!(check_start(&guard, &order).is_ok());
    }

    #[test]
    fn name_is_order_validation_policy_name() {
        let guard = GuardedOrderValidation::new();
        assert_eq!(
            <GuardedOrderValidation as CheckPreTradeStartPolicy<FakeOrder, ()>>::name(&guard),
            OrderValidationPolicy::NAME
        );
    }

    #[test]
    fn apply_execution_report_always_returns_false() {
        let guard = GuardedOrderValidation::new();
        let result =
            <GuardedOrderValidation as CheckPreTradeStartPolicy<FakeOrder, ()>>::apply_execution_report(
                &guard, &(),
            );
        assert!(!result);
    }

    #[test]
    fn delegates_when_operation_present() {
        let guard = GuardedOrderValidation::new();
        let order = FakeOrder {
            operation: Some(sample_operation()),
        };
        assert!(check_start(&guard, &order).is_ok());
    }

    #[test]
    fn delegates_rejection_of_zero_quantity() {
        let guard = GuardedOrderValidation::new();
        let order = FakeOrder {
            operation: Some(OrderOperation {
                account_id: AccountId::from_u64(99224416),
                instrument: Instrument::new(
                    Asset::new("AAPL").expect("valid"),
                    Asset::new("USD").expect("valid"),
                ),
                side: Side::Buy,
                trade_amount: TradeAmount::Quantity(Quantity::ZERO),
                price: Some(Price::from_str("185").expect("valid")),
            }),
        };
        let reject = check_start(&guard, &order)
            .expect_err("zero quantity must be rejected by inner policy");
        assert_eq!(reject.code, RejectCode::InvalidFieldValue);
    }

    #[test]
    fn delegates_rejection_of_zero_volume() {
        use openpit::param::Volume;
        let guard = GuardedOrderValidation::new();
        let order = FakeOrder {
            operation: Some(OrderOperation {
                account_id: AccountId::from_u64(99224416),
                instrument: Instrument::new(
                    Asset::new("AAPL").expect("valid"),
                    Asset::new("USD").expect("valid"),
                ),
                side: Side::Buy,
                trade_amount: TradeAmount::Volume(Volume::ZERO),
                price: None,
            }),
        };
        let reject =
            check_start(&guard, &order).expect_err("zero volume must be rejected by inner policy");
        assert_eq!(reject.code, RejectCode::InvalidFieldValue);
    }
}
