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

use crate::core::HasTradeAmount;
use crate::param::TradeAmount;
use crate::pretrade::policy::{missing_required_field_reject, PolicyName};
use crate::pretrade::{PreTradeContext, PreTradePolicy, Reject, RejectCode, RejectScope, Rejects};

/// Start-stage policy for basic order field validation.
///
/// The current implementation validates only explicitly provided fields:
/// `trade_amount` must be non-zero when present.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OrderValidationPolicy;

impl OrderValidationPolicy {
    /// Stable policy name.
    pub const NAME: &'static str = "OrderValidationPolicy";

    /// Creates a new order validation policy.
    pub fn new() -> Self {
        Self
    }
}

impl PolicyName for OrderValidationPolicy {
    fn policy_name(&self) -> &str {
        Self::NAME
    }
}

impl<Order, ExecutionReport, AccountAdjustment>
    PreTradePolicy<Order, ExecutionReport, AccountAdjustment> for OrderValidationPolicy
where
    Order: HasTradeAmount,
{
    fn name(&self) -> &str {
        Self::NAME
    }

    fn check_pre_trade_start(&self, _ctx: &PreTradeContext, order: &Order) -> Result<(), Rejects> {
        match order
            .trade_amount()
            .map_err(|e| Rejects::from(missing_required_field_reject(self, "trade amount", &e)))?
        {
            TradeAmount::Quantity(quantity) if quantity.is_zero() => {
                return Err(Reject::new(
                    Self::NAME,
                    RejectScope::Order,
                    RejectCode::InvalidFieldValue,
                    "order quantity must be non-zero",
                    "requested quantity 0 is not allowed",
                )
                .into());
            }
            TradeAmount::Volume(volume) if volume.is_zero() => {
                return Err(Reject::new(
                    Self::NAME,
                    RejectScope::Order,
                    RejectCode::InvalidFieldValue,
                    "order volume must be non-zero",
                    "requested volume 0 is not allowed",
                )
                .into());
            }
            _ => {}
        }
        Ok(())
    }

    fn apply_execution_report(
        &self,
        _report: &ExecutionReport,
    ) -> Vec<crate::pretrade::AccountBlock> {
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use crate::core::{Instrument, OrderOperation};
    use crate::param::{AccountId, Asset, Price, Quantity, Side, TradeAmount, Volume};
    use crate::pretrade::{PreTradeContext, PreTradePolicy, RejectCode, RejectScope};
    use crate::RequestFieldAccessError;

    use super::OrderValidationPolicy;

    #[test]
    fn rejects_zero_quantity() {
        let policy = OrderValidationPolicy::new();
        let order = OrderOperation {
            instrument: Instrument::new(
                Asset::new("AAPL").expect("asset code must be valid"),
                Asset::new("USD").expect("asset code must be valid"),
            ),
            account_id: AccountId::from_u64(99224416),
            side: Side::Buy,
            trade_amount: TradeAmount::Quantity(Quantity::ZERO),
            price: Some(Price::from_str("10").expect("price must be valid")),
        };

        let reject =
            <OrderValidationPolicy as PreTradePolicy<OrderOperation, ()>>::check_pre_trade_start(
                &policy,
                &PreTradeContext::new(),
                &order,
            )
            .expect_err("zero quantity must be rejected");
        let reject = &reject[0];
        assert_eq!(reject.scope, RejectScope::Order);
        assert_eq!(reject.code, RejectCode::InvalidFieldValue);
        assert_eq!(reject.reason, "order quantity must be non-zero");
        assert_eq!(reject.details, "requested quantity 0 is not allowed");
    }

    #[test]
    fn rejects_zero_volume() {
        let policy = OrderValidationPolicy::new();
        let order = OrderOperation {
            instrument: Instrument::new(
                Asset::new("AAPL").expect("asset code must be valid"),
                Asset::new("USD").expect("asset code must be valid"),
            ),
            account_id: AccountId::from_u64(99224416),
            side: Side::Buy,
            trade_amount: TradeAmount::Volume(Volume::ZERO),
            price: None,
        };

        let reject =
            <OrderValidationPolicy as PreTradePolicy<OrderOperation, ()>>::check_pre_trade_start(
                &policy,
                &PreTradeContext::new(),
                &order,
            )
            .expect_err("zero volume must be rejected");
        let reject = &reject[0];
        assert_eq!(reject.code, RejectCode::InvalidFieldValue);
        assert_eq!(reject.reason, "order volume must be non-zero");
    }

    #[test]
    fn allows_missing_price_when_volume_is_present() {
        let policy = OrderValidationPolicy::new();
        let order = OrderOperation {
            instrument: Instrument::new(
                Asset::new("AAPL").expect("asset code must be valid"),
                Asset::new("USD").expect("asset code must be valid"),
            ),
            account_id: AccountId::from_u64(99224416),
            side: Side::Buy,
            trade_amount: TradeAmount::Volume(
                Volume::from_str("100").expect("volume must be valid"),
            ),
            price: None,
        };

        assert!(
            <OrderValidationPolicy as PreTradePolicy<OrderOperation, ()>>::check_pre_trade_start(
                &policy,
                &PreTradeContext::new(),
                &order
            )
            .is_ok()
        );
    }

    #[test]
    fn allows_zero_and_negative_price() {
        let policy = OrderValidationPolicy::new();
        let zero_price_order = OrderOperation {
            instrument: Instrument::new(
                Asset::new("AAPL").expect("asset code must be valid"),
                Asset::new("USD").expect("asset code must be valid"),
            ),
            account_id: AccountId::from_u64(99224416),
            side: Side::Buy,
            trade_amount: TradeAmount::Quantity(
                Quantity::from_str("10").expect("quantity must be valid"),
            ),
            price: Some(Price::ZERO),
        };

        assert!(
            <OrderValidationPolicy as PreTradePolicy<OrderOperation, ()>>::check_pre_trade_start(
                &policy,
                &PreTradeContext::new(),
                &zero_price_order
            )
            .is_ok()
        );

        let negative_price_order = OrderOperation {
            instrument: Instrument::new(
                Asset::new("AAPL").expect("asset code must be valid"),
                Asset::new("USD").expect("asset code must be valid"),
            ),
            account_id: AccountId::from_u64(99224416),
            side: Side::Buy,
            trade_amount: TradeAmount::Volume(
                Volume::from_str("100").expect("volume must be valid"),
            ),
            price: Some(Price::from_str("-5").expect("price must be valid")),
        };

        assert!(
            <OrderValidationPolicy as PreTradePolicy<OrderOperation, ()>>::check_pre_trade_start(
                &policy,
                &PreTradeContext::new(),
                &negative_price_order
            )
            .is_ok()
        );
    }

    #[test]
    fn policy_name_is_stable() {
        let policy = OrderValidationPolicy::new();

        assert_eq!(
            <OrderValidationPolicy as PreTradePolicy<OrderOperation, ()>>::name(&policy),
            OrderValidationPolicy::NAME
        );
    }

    #[test]
    fn apply_execution_report_returns_false() {
        let policy = OrderValidationPolicy::new();
        let order = OrderOperation {
            instrument: Instrument::new(
                Asset::new("AAPL").expect("asset code must be valid"),
                Asset::new("USD").expect("asset code must be valid"),
            ),
            account_id: AccountId::from_u64(99224416),
            side: Side::Buy,
            trade_amount: TradeAmount::Quantity(
                Quantity::from_str("1").expect("quantity must be valid"),
            ),
            price: Some(Price::from_str("10").expect("price must be valid")),
        };

        assert!(
            <OrderValidationPolicy as PreTradePolicy<OrderOperation, ()>>::apply_execution_report(
                &policy,
                &()
            )
            .is_empty()
        );
        assert!(
            <OrderValidationPolicy as PreTradePolicy<OrderOperation, ()>>::check_pre_trade_start(
                &policy,
                &PreTradeContext::new(),
                &order
            )
            .is_ok()
        );
    }

    #[test]
    fn maps_trade_amount_access_error_to_missing_required_field() {
        struct InvalidOrder;

        impl crate::HasTradeAmount for InvalidOrder {
            fn trade_amount(&self) -> Result<TradeAmount, RequestFieldAccessError> {
                Err(RequestFieldAccessError::new("trade_amount"))
            }
        }

        let policy = OrderValidationPolicy::new();
        let reject =
            <OrderValidationPolicy as PreTradePolicy<InvalidOrder, ()>>::check_pre_trade_start(
                &policy,
                &PreTradeContext::new(),
                &InvalidOrder,
            )
            .expect_err("field access error must reject");
        let reject = &reject[0];
        assert_eq!(reject.scope, RejectScope::Order);
        assert_eq!(reject.code, RejectCode::MissingRequiredField);
        assert_eq!(
            reject.reason,
            "failed to access required field 'trade amount'"
        );
        assert_eq!(reject.details, "failed to access field 'trade_amount'");
    }
}
