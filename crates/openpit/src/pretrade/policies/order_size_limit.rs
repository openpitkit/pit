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
use std::collections::HashMap;

use crate::param::{Asset, Price, Quantity, TradeAmount, Volume};
use crate::pretrade::policy::request_field_access_reject;
use crate::pretrade::{CheckPreTradeStartPolicy, Reject, RejectCode, RejectScope};
use crate::HasInstrument;
use crate::{HasOrderPrice, HasTradeAmount};

/// Per-settlement order size limits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderSizeLimit {
    /// Maximum allowed order notional for the settlement asset.
    pub max_notional: Volume,
    /// Maximum allowed order quantity for the settlement asset.
    pub max_quantity: Quantity,
    /// Settlement asset the limit applies to.
    pub settlement_asset: Asset,
}

/// Start-stage policy enforcing per-settlement order size limits.
///
/// Limits are configured per settlement asset. Orders for assets without a
/// configured limit are always rejected with [`RejectScope::Order`].
///
/// # Examples
///
/// ```rust
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use openpit::param::{Asset, Price, Quantity, Side, TradeAmount, Volume};
/// use openpit::pretrade::policies::{OrderSizeLimit, OrderSizeLimitPolicy};
/// use openpit::{Engine, Instrument, OrderOperation};
///
/// let policy = OrderSizeLimitPolicy::new(
///     OrderSizeLimit {
///         settlement_asset: Asset::new("USD")?,
///         max_quantity: Quantity::from_str("100")?,
///         max_notional: Volume::from_str("50000")?,
///     },
///     [],
/// );
///
/// let engine = Engine::<OrderOperation, ()>::builder()
///     .check_pre_trade_start_policy(policy)
///     .build()?;
///
/// let order = OrderOperation {
///     instrument: Instrument::new(Asset::new("AAPL")?, Asset::new("USD")?),
///     account_id: openpit::param::AccountId::from_u64(99224416),
///     side: Side::Buy,
///     trade_amount: TradeAmount::Quantity(Quantity::from_str("10")?),
///     price: Some(Price::from_str("200")?),
/// };
/// assert!(engine.start_pre_trade(order).is_ok());
/// # Ok(())
/// # }
/// ```
pub struct OrderSizeLimitPolicy {
    limits: RefCell<HashMap<Asset, OrderSizeLimit>>,
}

impl OrderSizeLimitPolicy {
    /// Stable policy name.
    pub const NAME: &'static str = "OrderSizeLimitPolicy";

    /// Creates an order-size policy with at least one configured limit.
    pub fn new(
        initial_limit: OrderSizeLimit,
        additional_limits: impl IntoIterator<Item = OrderSizeLimit>,
    ) -> Self {
        let mut limits = HashMap::new();
        limits.insert(initial_limit.settlement_asset.clone(), initial_limit);
        for limit in additional_limits {
            limits.insert(limit.settlement_asset.clone(), limit);
        }
        Self {
            limits: RefCell::new(limits),
        }
    }

    /// Registers or replaces a limit for `limit.settlement_asset`.
    pub fn set_limit(&self, limit: OrderSizeLimit) {
        self.limits
            .borrow_mut()
            .insert(limit.settlement_asset.clone(), limit);
    }
}

impl<O, R> CheckPreTradeStartPolicy<O, R> for OrderSizeLimitPolicy
where
    O: HasInstrument + HasTradeAmount + HasOrderPrice,
{
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn check_pre_trade_start(&self, order: &O) -> Result<(), Reject> {
        let limits = self.limits.borrow();
        let instrument = order
            .instrument()
            .map_err(|e| request_field_access_reject(Self::NAME, &e))?;
        let trade_amount = order
            .trade_amount()
            .map_err(|e| request_field_access_reject(Self::NAME, &e))?;
        let price = order
            .price()
            .map_err(|e| request_field_access_reject(Self::NAME, &e))?;
        check_pre_trade_start_with_limits(
            Self::NAME,
            &limits,
            instrument.settlement_asset(),
            trade_amount,
            price,
        )
    }

    fn apply_execution_report(&self, _report: &R) -> bool {
        false
    }
}

fn check_pre_trade_start_with_limits(
    policy: &'static str,
    limits: &HashMap<Asset, OrderSizeLimit>,
    settlement: &Asset,
    trade_amount: TradeAmount,
    price: Option<Price>,
) -> Result<(), Reject> {
    let limit = match limits.get(settlement).cloned() {
        Some(value) => value,
        None => return Err(missing_order_size_limit_reject(policy, settlement)),
    };

    let quantity = resolve_quantity(policy, trade_amount, price)?;
    let requested_notional = resolve_notional(policy, trade_amount, price)?;
    let quantity_exceeded = quantity > limit.max_quantity;
    let notional_exceeded = requested_notional > limit.max_notional;

    match (quantity_exceeded, notional_exceeded) {
        (false, false) => Ok(()),
        (true, false) => Err(Reject::new(
            policy,
            RejectScope::Order,
            RejectCode::OrderQtyExceedsLimit,
            "order quantity exceeded",
            format!("requested {quantity}, max allowed: {}", limit.max_quantity),
        )),
        (false, true) => Err(order_notional_reject(policy, &limit, requested_notional)),
        (true, true) => Err(order_size_reject(
            policy,
            quantity,
            &limit,
            requested_notional,
        )),
    }
}

fn resolve_notional(
    policy: &'static str,
    trade_amount: TradeAmount,
    price: Option<Price>,
) -> Result<Volume, Reject> {
    match (trade_amount, price) {
        (TradeAmount::Volume(volume), _) => Ok(volume),
        (TradeAmount::Quantity(quantity), Some(price)) => {
            price.calculate_volume(quantity).map_err(|_| {
                order_value_calculation_failed_reject(
                    policy,
                    "price or quantity could not be used to evaluate order notional",
                )
            })
        }
        (TradeAmount::Quantity(_), None) => Err(order_value_calculation_failed_reject(
            policy,
            "price not provided for evaluating cash flow/notional/volume",
        )),
    }
}

fn resolve_quantity(
    policy: &'static str,
    trade_amount: TradeAmount,
    price: Option<Price>,
) -> Result<Quantity, Reject> {
    match (trade_amount, price) {
        (TradeAmount::Quantity(quantity), _) => Ok(quantity),
        (TradeAmount::Volume(volume), Some(price)) => {
            volume.calculate_quantity(price).map_err(|_| {
                order_value_calculation_failed_reject(
                    policy,
                    "price or volume could not be used to evaluate order quantity",
                )
            })
        }
        (TradeAmount::Volume(_), None) => Err(order_value_calculation_failed_reject(
            policy,
            "price not provided for evaluating cash flow/notional/volume",
        )),
    }
}

fn missing_order_size_limit_reject(policy: &'static str, settlement: &Asset) -> Reject {
    Reject::new(
        policy,
        RejectScope::Order,
        RejectCode::RiskConfigurationMissing,
        "order size limit missing",
        format!("settlement asset {settlement} has no configured limit"),
    )
}

fn order_value_calculation_failed_reject(policy: &'static str, details: &'static str) -> Reject {
    Reject::new(
        policy,
        RejectScope::Order,
        RejectCode::OrderValueCalculationFailed,
        "order value calculation failed",
        details,
    )
}

fn order_notional_reject(
    policy: &'static str,
    limit: &OrderSizeLimit,
    requested_notional: Volume,
) -> Reject {
    let details = format!(
        "requested {requested_notional}, max allowed: {}",
        limit.max_notional
    );

    Reject::new(
        policy,
        RejectScope::Order,
        RejectCode::OrderNotionalExceedsLimit,
        "order notional exceeded",
        details,
    )
}

fn order_size_reject(
    policy: &'static str,
    quantity: crate::param::Quantity,
    limit: &OrderSizeLimit,
    requested_notional: Volume,
) -> Reject {
    Reject::new(
        policy,
        RejectScope::Order,
        RejectCode::OrderExceedsLimit,
        "order size exceeded",
        format!(
            "requested quantity {quantity}, max allowed: {}; requested notional {requested_notional}, max allowed: {}",
            limit.max_quantity,
            limit.max_notional
        ),
    )
}

#[cfg(test)]
mod tests {
    use crate::core::{Instrument, OrderOperation};
    use crate::param::TradeAmount;
    use crate::param::{AccountId, Asset, Price, Quantity, Side, Volume};
    use crate::pretrade::{CheckPreTradeStartPolicy, RejectCode, RejectScope};
    use crate::{HasInstrument, HasOrderPrice, HasTradeAmount, RequestFieldAccessError};
    use rust_decimal::Decimal;

    use super::{OrderSizeLimit, OrderSizeLimitPolicy};

    type TestOrder = OrderOperation;

    fn order(settlement: &str, quantity: &str, price: &str) -> TestOrder {
        OrderOperation {
            instrument: Instrument::new(
                Asset::new("AAPL").expect("asset code must be valid"),
                Asset::new(settlement).expect("asset code must be valid"),
            ),
            account_id: AccountId::from_u64(99224416),
            side: Side::Buy,
            trade_amount: TradeAmount::Quantity(
                Quantity::from_str(quantity).expect("quantity literal must be valid"),
            ),
            price: Some(Price::from_str(price).expect("price literal must be valid")),
        }
    }

    #[test]
    fn quantity_violation_returns_order_quantity_exceeded() {
        let policy = OrderSizeLimitPolicy::new(limit("USD", "10", "1000"), no_limits());

        let reject =
            <OrderSizeLimitPolicy as CheckPreTradeStartPolicy<TestOrder, ()>>::check_pre_trade_start(
                &policy,
                &order("USD", "11", "90"),
            )
            .expect_err("quantity must be rejected");
        assert_eq!(reject.scope, RejectScope::Order);
        assert_eq!(reject.code, RejectCode::OrderQtyExceedsLimit);
        assert_eq!(reject.reason, "order quantity exceeded");
        assert_eq!(reject.details, "requested 11, max allowed: 10");
    }

    #[test]
    fn notional_violation_returns_order_notional_exceeded() {
        let policy = OrderSizeLimitPolicy::new(limit("USD", "10", "1000"), no_limits());

        let reject =
            <OrderSizeLimitPolicy as CheckPreTradeStartPolicy<TestOrder, ()>>::check_pre_trade_start(
                &policy,
                &order("USD", "10", "101"),
            )
            .expect_err("notional must be rejected");
        assert_eq!(reject.scope, RejectScope::Order);
        assert_eq!(reject.code, RejectCode::OrderNotionalExceedsLimit);
        assert_eq!(reject.reason, "order notional exceeded");
        assert_eq!(reject.details, "requested 1010, max allowed: 1000");
    }

    #[test]
    fn both_violations_are_returned_in_single_reject() {
        let policy = OrderSizeLimitPolicy::new(limit("USD", "10", "1000"), no_limits());

        let reject =
            <OrderSizeLimitPolicy as CheckPreTradeStartPolicy<TestOrder, ()>>::check_pre_trade_start(
                &policy,
                &order("USD", "11", "100"),
            )
            .expect_err("quantity and notional must be rejected");
        assert_eq!(reject.scope, RejectScope::Order);
        assert_eq!(reject.code, RejectCode::OrderExceedsLimit);
        assert_eq!(reject.reason, "order size exceeded");
        assert_eq!(
            reject.details,
            "requested quantity 11, max allowed: 10; requested notional 1100, max allowed: 1000"
        );
    }

    #[test]
    fn missing_limit_returns_order_size_limit_missing() {
        let policy = OrderSizeLimitPolicy::new(limit("EUR", "10", "1000"), no_limits());

        let reject =
            <OrderSizeLimitPolicy as CheckPreTradeStartPolicy<TestOrder, ()>>::check_pre_trade_start(
                &policy,
                &order("USD", "1", "1"),
            )
            .expect_err("missing limit must reject");
        assert_eq!(reject.scope, RejectScope::Order);
        assert_eq!(reject.code, RejectCode::RiskConfigurationMissing);
        assert_eq!(reject.reason, "order size limit missing");
        assert_eq!(
            reject.details,
            "settlement asset USD has no configured limit"
        );
    }

    #[test]
    fn boundary_values_are_accepted() {
        let policy = OrderSizeLimitPolicy::new(limit("USD", "10", "1000"), no_limits());

        let result =
            <OrderSizeLimitPolicy as CheckPreTradeStartPolicy<TestOrder, ()>>::check_pre_trade_start(
                &policy,
                &order("USD", "10", "100"),
            );
        assert!(result.is_ok());
    }

    #[test]
    fn unconfigured_settlement_rejects_when_limit_is_missing() {
        let policy = OrderSizeLimitPolicy::new(limit("EUR", "10", "1000"), no_limits());

        let reject =
            <OrderSizeLimitPolicy as CheckPreTradeStartPolicy<TestOrder, ()>>::check_pre_trade_start(
                &policy,
                &order("USD", "1", "1"),
            )
            .expect_err("default policy must reject without configured limits");
        assert_eq!(reject.code, RejectCode::RiskConfigurationMissing);
        assert_eq!(reject.reason, "order size limit missing");
        assert_eq!(
            reject.details,
            "settlement asset USD has no configured limit"
        );
    }

    #[test]
    fn volume_overflow_is_treated_as_notional_exceeded() {
        let policy = OrderSizeLimitPolicy::new(limit("USD", "100", "1000"), no_limits());

        let order = OrderOperation {
            instrument: Instrument::new(
                Asset::new("AAPL").expect("asset code must be valid"),
                Asset::new("USD").expect("asset code must be valid"),
            ),
            account_id: AccountId::from_u64(99224416),
            side: crate::param::Side::Buy,
            trade_amount: TradeAmount::Quantity(
                Quantity::from_str("2").expect("quantity literal must be valid"),
            ),
            price: Some(crate::param::Price::new(Decimal::MAX)),
        };

        let reject =
            <OrderSizeLimitPolicy as CheckPreTradeStartPolicy<TestOrder, ()>>::check_pre_trade_start(
                &policy,
                &order,
            )
            .expect_err("overflow must be treated as notional exceeded");
        assert_eq!(reject.scope, RejectScope::Order);
        assert_eq!(reject.code, RejectCode::OrderValueCalculationFailed);
        assert_eq!(reject.reason, "order value calculation failed");
        assert_eq!(
            reject.details,
            "price or quantity could not be used to evaluate order notional"
        );
    }

    #[test]
    fn volume_overflow_with_quantity_violation_returns_order_size_exceeded() {
        let policy = OrderSizeLimitPolicy::new(limit("USD", "1", "1000"), no_limits());

        let order = OrderOperation {
            instrument: Instrument::new(
                Asset::new("AAPL").expect("asset code must be valid"),
                Asset::new("USD").expect("asset code must be valid"),
            ),
            account_id: AccountId::from_u64(99224416),
            side: crate::param::Side::Buy,
            trade_amount: TradeAmount::Quantity(
                Quantity::from_str("2").expect("quantity literal must be valid"),
            ),
            price: Some(crate::param::Price::new(Decimal::MAX)),
        };

        let reject =
            <OrderSizeLimitPolicy as CheckPreTradeStartPolicy<TestOrder, ()>>::check_pre_trade_start(
                &policy,
                &order,
            )
            .expect_err("overflow plus quantity violation must be order size exceeded");
        assert_eq!(reject.scope, RejectScope::Order);
        assert_eq!(reject.code, RejectCode::OrderValueCalculationFailed);
        assert_eq!(reject.reason, "order value calculation failed");
        assert_eq!(
            reject.details,
            "price or quantity could not be used to evaluate order notional"
        );
    }

    #[test]
    fn additional_limits_and_set_limit_are_applied() {
        let policy = OrderSizeLimitPolicy::new(
            limit("USD", "10", "1000"),
            vec![limit("EUR", "5", "500"), limit("GBP", "3", "300")],
        );

        assert!(<OrderSizeLimitPolicy as CheckPreTradeStartPolicy<TestOrder, ()>>::check_pre_trade_start(&policy, &order("EUR", "5", "100")).is_ok());
        assert!(<OrderSizeLimitPolicy as CheckPreTradeStartPolicy<TestOrder, ()>>::check_pre_trade_start(&policy, &order("GBP", "3", "100")).is_ok());

        policy.set_limit(limit("EUR", "1", "100"));
        let reject = <OrderSizeLimitPolicy as CheckPreTradeStartPolicy<TestOrder, ()>>::check_pre_trade_start(&policy, &order("EUR", "2", "10"))
            .expect_err("updated limit must be enforced");
        assert_eq!(reject.code, RejectCode::OrderQtyExceedsLimit);
        assert_eq!(reject.details, "requested 2, max allowed: 1");
    }

    #[test]
    fn policy_name_is_stable() {
        let policy = OrderSizeLimitPolicy::new(limit("USD", "10", "1000"), no_limits());
        assert_eq!(
            <OrderSizeLimitPolicy as CheckPreTradeStartPolicy<TestOrder, ()>>::name(&policy),
            OrderSizeLimitPolicy::NAME
        );
    }

    #[test]
    fn apply_execution_report_returns_false() {
        let policy = OrderSizeLimitPolicy::new(limit("USD", "10", "1000"), no_limits());

        assert!(!<OrderSizeLimitPolicy as CheckPreTradeStartPolicy<
            TestOrder,
            (),
        >>::apply_execution_report(&policy, &()));
    }

    #[test]
    fn resolve_notional_covers_volume_and_missing_price_paths() {
        let from_volume = super::resolve_notional(
            OrderSizeLimitPolicy::NAME,
            TradeAmount::Volume(Volume::from_str("123").expect("volume literal must be valid")),
            None,
        )
        .expect("volume amount should resolve notional without price");
        assert_eq!(
            from_volume,
            Volume::from_str("123").expect("volume literal must be valid")
        );

        let missing_price = super::resolve_notional(
            OrderSizeLimitPolicy::NAME,
            TradeAmount::Quantity(Quantity::from_str("1").expect("quantity literal must be valid")),
            None,
        )
        .expect_err("quantity amount without price must reject");
        assert_eq!(missing_price.code, RejectCode::OrderValueCalculationFailed);
        assert_eq!(
            missing_price.details,
            "price not provided for evaluating cash flow/notional/volume"
        );
    }

    #[test]
    fn volume_order_without_price_propagates_resolve_quantity_error() {
        let policy = OrderSizeLimitPolicy::new(limit("USD", "100", "10000"), no_limits());
        let order = OrderOperation {
            instrument: Instrument::new(
                Asset::new("AAPL").expect("asset code must be valid"),
                Asset::new("USD").expect("asset code must be valid"),
            ),
            account_id: AccountId::from_u64(99224416),
            side: Side::Buy,
            trade_amount: TradeAmount::Volume(
                Volume::from_str("100").expect("volume literal must be valid"),
            ),
            price: None,
        };
        let reject =
            <OrderSizeLimitPolicy as CheckPreTradeStartPolicy<TestOrder, ()>>::check_pre_trade_start(
                &policy, &order,
            )
            .expect_err("volume order without price must reject");
        assert_eq!(reject.code, RejectCode::OrderValueCalculationFailed);
    }

    #[test]
    fn resolve_quantity_covers_invalid_volume_conversion_and_missing_price_paths() {
        let conversion_failed = super::resolve_quantity(
            OrderSizeLimitPolicy::NAME,
            TradeAmount::Volume(Volume::from_str("10").expect("volume literal must be valid")),
            Some(Price::from_str("0").expect("zero price literal must be valid")),
        )
        .expect_err("volume-to-quantity conversion with zero price must reject");
        assert_eq!(
            conversion_failed.code,
            RejectCode::OrderValueCalculationFailed
        );
        assert_eq!(
            conversion_failed.details,
            "price or volume could not be used to evaluate order quantity"
        );

        let missing_price = super::resolve_quantity(
            OrderSizeLimitPolicy::NAME,
            TradeAmount::Volume(Volume::from_str("10").expect("volume literal must be valid")),
            None,
        )
        .expect_err("volume amount without price must reject");
        assert_eq!(missing_price.code, RejectCode::OrderValueCalculationFailed);
        assert_eq!(
            missing_price.details,
            "price not provided for evaluating cash flow/notional/volume"
        );
    }

    #[test]
    fn maps_instrument_access_error_to_missing_required_field() {
        struct InstrumentAccessErrorOrder;

        impl HasInstrument for InstrumentAccessErrorOrder {
            fn instrument(&self) -> Result<&Instrument, RequestFieldAccessError> {
                Err(RequestFieldAccessError::new("instrument"))
            }
        }
        impl HasTradeAmount for InstrumentAccessErrorOrder {
            fn trade_amount(&self) -> Result<TradeAmount, RequestFieldAccessError> {
                Ok(TradeAmount::Quantity(
                    Quantity::from_str("1").expect("quantity literal must be valid"),
                ))
            }
        }
        impl HasOrderPrice for InstrumentAccessErrorOrder {
            fn price(&self) -> Result<Option<Price>, RequestFieldAccessError> {
                Ok(Some(
                    Price::from_str("1").expect("price literal must be valid"),
                ))
            }
        }

        let policy = OrderSizeLimitPolicy::new(limit("USD", "10", "1000"), no_limits());
        let order = InstrumentAccessErrorOrder;
        let reject = <OrderSizeLimitPolicy as CheckPreTradeStartPolicy<
            InstrumentAccessErrorOrder,
            (),
        >>::check_pre_trade_start(&policy, &order)
        .expect_err("field access error must reject");
        assert_eq!(reject.scope, RejectScope::Order);
        assert_eq!(reject.code, RejectCode::MissingRequiredField);
        assert_eq!(reject.reason, "failed to access required field");
        assert_eq!(reject.details, "failed to access field 'instrument'");
        assert_eq!(
            order.trade_amount(),
            Ok(TradeAmount::Quantity(
                Quantity::from_str("1").expect("quantity literal must be valid")
            ))
        );
        assert_eq!(
            order.price(),
            Ok(Some(
                Price::from_str("1").expect("price literal must be valid")
            ))
        );
    }

    #[test]
    fn maps_trade_amount_access_error_to_missing_required_field() {
        struct TradeAmountAccessErrorOrder {
            instrument: Instrument,
        }

        impl HasInstrument for TradeAmountAccessErrorOrder {
            fn instrument(&self) -> Result<&Instrument, RequestFieldAccessError> {
                Ok(&self.instrument)
            }
        }
        impl HasTradeAmount for TradeAmountAccessErrorOrder {
            fn trade_amount(&self) -> Result<TradeAmount, RequestFieldAccessError> {
                Err(RequestFieldAccessError::new("trade_amount"))
            }
        }
        impl HasOrderPrice for TradeAmountAccessErrorOrder {
            fn price(&self) -> Result<Option<Price>, RequestFieldAccessError> {
                Ok(Some(
                    Price::from_str("1").expect("price literal must be valid"),
                ))
            }
        }

        let policy = OrderSizeLimitPolicy::new(limit("USD", "10", "1000"), no_limits());
        let order = TradeAmountAccessErrorOrder {
            instrument: Instrument::new(
                Asset::new("AAPL").expect("asset code must be valid"),
                Asset::new("USD").expect("asset code must be valid"),
            ),
        };
        let reject = <OrderSizeLimitPolicy as CheckPreTradeStartPolicy<
            TradeAmountAccessErrorOrder,
            (),
        >>::check_pre_trade_start(&policy, &order)
        .expect_err("field access error must reject");
        assert_eq!(reject.scope, RejectScope::Order);
        assert_eq!(reject.code, RejectCode::MissingRequiredField);
        assert_eq!(reject.reason, "failed to access required field");
        assert_eq!(reject.details, "failed to access field 'trade_amount'");
        assert_eq!(
            order.price(),
            Ok(Some(
                Price::from_str("1").expect("price literal must be valid")
            ))
        );
    }

    #[test]
    fn maps_price_access_error_to_missing_required_field() {
        struct PriceAccessErrorOrder {
            instrument: Instrument,
        }

        impl HasInstrument for PriceAccessErrorOrder {
            fn instrument(&self) -> Result<&Instrument, RequestFieldAccessError> {
                Ok(&self.instrument)
            }
        }
        impl HasTradeAmount for PriceAccessErrorOrder {
            fn trade_amount(&self) -> Result<TradeAmount, RequestFieldAccessError> {
                Ok(TradeAmount::Quantity(
                    Quantity::from_str("1").expect("quantity literal must be valid"),
                ))
            }
        }
        impl HasOrderPrice for PriceAccessErrorOrder {
            fn price(&self) -> Result<Option<Price>, RequestFieldAccessError> {
                Err(RequestFieldAccessError::new("price"))
            }
        }

        let policy = OrderSizeLimitPolicy::new(limit("USD", "10", "1000"), no_limits());
        let order = PriceAccessErrorOrder {
            instrument: Instrument::new(
                Asset::new("AAPL").expect("asset code must be valid"),
                Asset::new("USD").expect("asset code must be valid"),
            ),
        };
        let reject = <OrderSizeLimitPolicy as CheckPreTradeStartPolicy<
            PriceAccessErrorOrder,
            (),
        >>::check_pre_trade_start(&policy, &order)
        .expect_err("field access error must reject");
        assert_eq!(reject.scope, RejectScope::Order);
        assert_eq!(reject.code, RejectCode::MissingRequiredField);
        assert_eq!(reject.reason, "failed to access required field");
        assert_eq!(reject.details, "failed to access field 'price'");
        assert_eq!(
            order.trade_amount(),
            Ok(TradeAmount::Quantity(
                Quantity::from_str("1").expect("quantity literal must be valid")
            ))
        );
    }

    fn limit(settlement: &str, max_quantity: &str, max_notional: &str) -> OrderSizeLimit {
        OrderSizeLimit {
            max_notional: Volume::from_str(max_notional)
                .expect("max notional literal must be valid"),
            max_quantity: Quantity::from_str(max_quantity)
                .expect("max quantity literal must be valid"),
            settlement_asset: Asset::new(settlement).expect("asset code must be valid"),
        }
    }

    fn no_limits() -> Vec<OrderSizeLimit> {
        Vec::new()
    }
}
