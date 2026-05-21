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

use std::collections::HashMap;
use std::fmt::{Display, Formatter};

use crate::core::HasAccountId;
use crate::param::{AccountId, Asset, Price, Quantity, TradeAmount, Volume};
use crate::pretrade::policy::{missing_required_field_reject, PolicyName};
use crate::pretrade::{PreTradeContext, PreTradePolicy, Reject, RejectCode, RejectScope, Rejects};
use crate::HasInstrument;
use crate::{HasOrderPrice, HasTradeAmount};

/// Order size limits: maximum quantity and notional for a single order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderSizeLimit {
    /// Maximum allowed order quantity.
    pub max_quantity: Quantity,
    /// Maximum allowed order notional.
    pub max_notional: Volume,
}

/// Broker-wide order size limit.
///
/// Applies to every order regardless of account and settlement asset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderSizeBrokerBarrier {
    /// Size limit for this broker barrier.
    pub limit: OrderSizeLimit,
}

/// Per-settlement-asset order size limit.
///
/// Applies to every order whose settlement asset matches `settlement_asset`,
/// shared across all accounts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderSizeAssetBarrier {
    /// Size limit for this asset barrier.
    pub limit: OrderSizeLimit,
    /// Settlement asset this barrier applies to.
    pub settlement_asset: Asset,
}

/// Per-(account, settlement-asset) order size limit.
///
/// Applies to orders matching both `account_id` and `settlement_asset`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderSizeAccountAssetBarrier {
    /// Size limit for this account+asset barrier.
    pub limit: OrderSizeLimit,
    /// Account this barrier applies to.
    pub account_id: AccountId,
    /// Settlement asset this barrier applies to.
    pub settlement_asset: Asset,
}

/// Errors returned by [`OrderSizeLimitPolicy`] construction.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrderSizeLimitPolicyError {
    /// No barriers were provided across all axes.
    NoBarriersConfigured,
}

impl Display for OrderSizeLimitPolicyError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoBarriersConfigured => write!(
                f,
                "at least one broker, asset, or account+asset barrier must be configured"
            ),
        }
    }
}

impl std::error::Error for OrderSizeLimitPolicyError {}

/// Start-stage policy enforcing per-settlement order size limits.
///
/// Three configurable barrier axes — broker (all orders), per settlement asset,
/// and per (account, settlement asset).
///
/// Resolution and check semantics for an order with `(account, settlement)`:
///
/// 1. **Asset-axis chain (override).** Picks at most one asset-axis limit:
///    - if an `account+asset` barrier covers `(account, settlement)`, that limit
///      wins;
///    - else if an `asset` barrier covers `settlement`, that limit applies;
///    - else no asset-axis limit applies.
///
/// 2. **Broker axis (additive).** If a broker barrier is configured, it also
///    applies in addition to whatever the asset-axis chain produced.
///
/// 3. If both apply and both fail, the asset-axis breach is reported first.
///
/// 4. No applicable limits → pass with no reject.
///
/// Constructor rules:
/// - at least one barrier across all three axes must be configured;
/// - if all are omitted, the constructor returns
///   [`OrderSizeLimitPolicyError::NoBarriersConfigured`];
/// - duplicate keys within an axis: last-write-wins (no error).
///
/// # Examples
///
/// ```rust
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use openpit::param::{Asset, Price, Quantity, Side, TradeAmount, Volume};
/// use openpit::pretrade::policies::{
///     OrderSizeAssetBarrier, OrderSizeLimit, OrderSizeLimitPolicy,
/// };
/// use openpit::{Engine, Instrument, OrderOperation};
///
/// let policy = OrderSizeLimitPolicy::new(
///     None,
///     [OrderSizeAssetBarrier {
///         limit: OrderSizeLimit {
///             max_quantity: Quantity::from_f64(100.0)?,
///             max_notional: Volume::from_f64(50000.0)?,
///         },
///         settlement_asset: Asset::new("USD")?,
///     }],
///     [],
/// )?;
///
/// let engine = Engine::builder::<OrderOperation, (), ()>()
///     .no_sync()
///     .pre_trade(policy)
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
    broker: Option<OrderSizeBrokerBarrier>,
    asset_limits: HashMap<Asset, OrderSizeLimit>,
    account_asset_limits: HashMap<(AccountId, Asset), OrderSizeLimit>,
}

impl OrderSizeLimitPolicy {
    /// Stable policy name.
    pub const NAME: &'static str = "OrderSizeLimitPolicy";

    /// Creates an order-size policy.
    ///
    /// At least one barrier must be provided across all axes. If all are `None`
    /// or empty, returns [`OrderSizeLimitPolicyError::NoBarriersConfigured`].
    /// Duplicate keys within an axis: last-write-wins (no error).
    pub fn new(
        broker: Option<OrderSizeBrokerBarrier>,
        asset_barriers: impl IntoIterator<Item = OrderSizeAssetBarrier>,
        account_asset_barriers: impl IntoIterator<Item = OrderSizeAccountAssetBarrier>,
    ) -> Result<Self, OrderSizeLimitPolicyError> {
        let asset_limits: HashMap<Asset, OrderSizeLimit> = asset_barriers
            .into_iter()
            .map(|b| (b.settlement_asset, b.limit))
            .collect();

        let account_asset_limits: HashMap<(AccountId, Asset), OrderSizeLimit> =
            account_asset_barriers
                .into_iter()
                .map(|b| ((b.account_id, b.settlement_asset), b.limit))
                .collect();

        if broker.is_none() && asset_limits.is_empty() && account_asset_limits.is_empty() {
            return Err(OrderSizeLimitPolicyError::NoBarriersConfigured);
        }

        Ok(Self {
            broker,
            asset_limits,
            account_asset_limits,
        })
    }
}

impl PolicyName for OrderSizeLimitPolicy {
    fn policy_name(&self) -> &str {
        Self::NAME
    }
}

impl<Order, ExecutionReport, AccountAdjustment>
    PreTradePolicy<Order, ExecutionReport, AccountAdjustment> for OrderSizeLimitPolicy
where
    Order: HasInstrument + HasTradeAmount + HasOrderPrice + HasAccountId,
{
    fn name(&self) -> &str {
        Self::NAME
    }

    fn check_pre_trade_start(&self, _ctx: &PreTradeContext, order: &Order) -> Result<(), Rejects> {
        let instrument = order
            .instrument()
            .map_err(|e| Rejects::from(missing_required_field_reject(self, "instrument", &e)))?;
        let account_id = order
            .account_id()
            .map_err(|e| Rejects::from(missing_required_field_reject(self, "account ID", &e)))?;
        let trade_amount = order
            .trade_amount()
            .map_err(|e| Rejects::from(missing_required_field_reject(self, "trade amount", &e)))?;
        let price = order
            .price()
            .map_err(|e| Rejects::from(missing_required_field_reject(self, "price", &e)))?;

        let settlement = instrument.settlement_asset();

        // Asset-axis chain (override): account+asset wins over asset; otherwise nothing.
        let (axis_limit, axis_scope) = if let Some(limit) = self
            .account_asset_limits
            .get(&(account_id, settlement.clone()))
        {
            (Some(limit), RejectScope::Account)
        } else if let Some(limit) = self.asset_limits.get(settlement) {
            (Some(limit), RejectScope::Order)
        } else {
            (None, RejectScope::Order)
        };

        let broker_limit = self.broker.as_ref().map(|b| &b.limit);

        if axis_limit.is_none() && broker_limit.is_none() {
            return Ok(());
        }

        let quantity = resolve_quantity(Self::NAME, trade_amount, price).map_err(Rejects::from)?;
        let notional = resolve_notional(Self::NAME, trade_amount, price).map_err(Rejects::from)?;

        // Check axis limit first; if both breach, axis is reported first.
        let axis_reject = axis_limit.and_then(|limit| {
            check_limit_optional(Self::NAME, limit, quantity, notional, axis_scope)
        });

        let broker_reject = broker_limit.and_then(|limit| {
            check_limit_optional(Self::NAME, limit, quantity, notional, RejectScope::Order)
        });

        if let Some(reject) = axis_reject.or(broker_reject) {
            return Err(Rejects::from(reject));
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

fn check_limit_optional(
    policy: &str,
    limit: &OrderSizeLimit,
    quantity: Quantity,
    notional: Volume,
    scope: RejectScope,
) -> Option<Reject> {
    let qty_exceeded = quantity > limit.max_quantity;
    let notional_exceeded = notional > limit.max_notional;
    match (qty_exceeded, notional_exceeded) {
        (false, false) => None,
        (true, false) => Some(Reject::new(
            policy,
            scope,
            RejectCode::OrderQtyExceedsLimit,
            "order quantity exceeded",
            format!("requested {quantity}, max allowed: {}", limit.max_quantity),
        )),
        (false, true) => Some(Reject::new(
            policy,
            scope,
            RejectCode::OrderNotionalExceedsLimit,
            "order notional exceeded",
            format!("requested {notional}, max allowed: {}", limit.max_notional),
        )),
        (true, true) => Some(Reject::new(
            policy,
            scope,
            RejectCode::OrderExceedsLimit,
            "order size exceeded",
            format!(
                "requested quantity {quantity}, max allowed: {}; \
                 requested notional {notional}, max allowed: {}",
                limit.max_quantity, limit.max_notional
            ),
        )),
    }
}

fn resolve_notional(
    policy: &str,
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
    policy: &str,
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

fn order_value_calculation_failed_reject(policy: &str, details: &'static str) -> Reject {
    Reject::new(
        policy,
        RejectScope::Order,
        RejectCode::OrderValueCalculationFailed,
        "order value calculation failed",
        details,
    )
}

#[cfg(test)]
mod tests {
    use crate::core::{HasAccountId, Instrument, OrderOperation};
    use crate::param::TradeAmount;
    use crate::param::{AccountId, Asset, Price, Quantity, Side, Volume};
    use crate::pretrade::{PreTradeContext, PreTradePolicy, RejectCode, RejectScope};
    use crate::{HasInstrument, HasOrderPrice, HasTradeAmount, RequestFieldAccessError};
    use rust_decimal::Decimal;

    use super::{
        OrderSizeAccountAssetBarrier, OrderSizeAssetBarrier, OrderSizeBrokerBarrier,
        OrderSizeLimit, OrderSizeLimitPolicy, OrderSizeLimitPolicyError,
    };

    type TestOrder = OrderOperation;

    fn order(settlement: &str, quantity: &str, price: &str) -> TestOrder {
        order_for_account(settlement, quantity, price, AccountId::from_u64(99224416))
    }

    fn order_for_account(
        settlement: &str,
        quantity: &str,
        price: &str,
        account_id: AccountId,
    ) -> TestOrder {
        OrderOperation {
            instrument: Instrument::new(
                Asset::new("AAPL").expect("asset code must be valid"),
                Asset::new(settlement).expect("asset code must be valid"),
            ),
            account_id,
            side: Side::Buy,
            trade_amount: TradeAmount::Quantity(
                Quantity::from_str(quantity).expect("quantity literal must be valid"),
            ),
            price: Some(Price::from_str(price).expect("price literal must be valid")),
        }
    }

    fn limit(max_quantity: &str, max_notional: &str) -> OrderSizeLimit {
        OrderSizeLimit {
            max_quantity: Quantity::from_str(max_quantity)
                .expect("max quantity literal must be valid"),
            max_notional: Volume::from_str(max_notional)
                .expect("max notional literal must be valid"),
        }
    }

    fn asset_barrier(
        settlement: &str,
        max_quantity: &str,
        max_notional: &str,
    ) -> OrderSizeAssetBarrier {
        OrderSizeAssetBarrier {
            limit: limit(max_quantity, max_notional),
            settlement_asset: Asset::new(settlement).expect("asset code must be valid"),
        }
    }

    fn broker_barrier(max_quantity: &str, max_notional: &str) -> OrderSizeBrokerBarrier {
        OrderSizeBrokerBarrier {
            limit: limit(max_quantity, max_notional),
        }
    }

    // ── constructor validation ─────────────────────────────────────────────

    #[test]
    fn no_barriers_configured_rejected_by_constructor() {
        let err = OrderSizeLimitPolicy::new(None, [], [])
            .err()
            .expect("must fail");
        assert_eq!(err, OrderSizeLimitPolicyError::NoBarriersConfigured);
        assert_eq!(
            err.to_string(),
            "at least one broker, asset, or account+asset barrier must be configured"
        );
    }

    // ── asset barrier ──────────────────────────────────────────────────────

    #[test]
    fn quantity_violation_returns_order_quantity_exceeded() {
        let policy =
            OrderSizeLimitPolicy::new(None, [asset_barrier("USD", "10", "1000")], []).unwrap();

        let reject =
            <OrderSizeLimitPolicy as PreTradePolicy<TestOrder, ()>>::check_pre_trade_start(
                &policy,
                &PreTradeContext::new(),
                &order("USD", "11", "90"),
            )
            .expect_err("quantity must be rejected");
        let reject = &reject[0];
        assert_eq!(reject.scope, RejectScope::Order);
        assert_eq!(reject.code, RejectCode::OrderQtyExceedsLimit);
        assert_eq!(reject.reason, "order quantity exceeded");
        assert_eq!(reject.details, "requested 11, max allowed: 10");
    }

    #[test]
    fn notional_violation_returns_order_notional_exceeded() {
        let policy =
            OrderSizeLimitPolicy::new(None, [asset_barrier("USD", "10", "1000")], []).unwrap();

        let reject =
            <OrderSizeLimitPolicy as PreTradePolicy<TestOrder, ()>>::check_pre_trade_start(
                &policy,
                &PreTradeContext::new(),
                &order("USD", "10", "101"),
            )
            .expect_err("notional must be rejected");
        let reject = &reject[0];
        assert_eq!(reject.scope, RejectScope::Order);
        assert_eq!(reject.code, RejectCode::OrderNotionalExceedsLimit);
        assert_eq!(reject.reason, "order notional exceeded");
        assert_eq!(reject.details, "requested 1010, max allowed: 1000");
    }

    #[test]
    fn both_violations_are_returned_in_single_reject() {
        let policy =
            OrderSizeLimitPolicy::new(None, [asset_barrier("USD", "10", "1000")], []).unwrap();

        let reject =
            <OrderSizeLimitPolicy as PreTradePolicy<TestOrder, ()>>::check_pre_trade_start(
                &policy,
                &PreTradeContext::new(),
                &order("USD", "11", "100"),
            )
            .expect_err("quantity and notional must be rejected");
        let reject = &reject[0];
        assert_eq!(reject.scope, RejectScope::Order);
        assert_eq!(reject.code, RejectCode::OrderExceedsLimit);
        assert_eq!(reject.reason, "order size exceeded");
        assert_eq!(
            reject.details,
            "requested quantity 11, max allowed: 10; requested notional 1100, max allowed: 1000"
        );
    }

    #[test]
    fn no_applicable_limit_passes_silently() {
        let policy =
            OrderSizeLimitPolicy::new(None, [asset_barrier("EUR", "10", "1000")], []).unwrap();

        let result = <OrderSizeLimitPolicy as PreTradePolicy<TestOrder, ()>>::check_pre_trade_start(
            &policy,
            &PreTradeContext::new(),
            &order("USD", "1", "1"),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn boundary_values_are_accepted() {
        let policy =
            OrderSizeLimitPolicy::new(None, [asset_barrier("USD", "10", "1000")], []).unwrap();

        let result = <OrderSizeLimitPolicy as PreTradePolicy<TestOrder, ()>>::check_pre_trade_start(
            &policy,
            &PreTradeContext::new(),
            &order("USD", "10", "100"),
        );
        assert!(result.is_ok());
    }

    // ── broker barrier ─────────────────────────────────────────────────────

    #[test]
    fn broker_barrier_applies_regardless_of_settlement() {
        let policy = OrderSizeLimitPolicy::new(Some(broker_barrier("5", "500")), [], []).unwrap();

        // Broker limit applies to any settlement
        let reject =
            <OrderSizeLimitPolicy as PreTradePolicy<TestOrder, ()>>::check_pre_trade_start(
                &policy,
                &PreTradeContext::new(),
                &order("USD", "6", "10"),
            )
            .expect_err("broker barrier must reject over-quantity order");
        assert_eq!(reject[0].scope, RejectScope::Order);
        assert_eq!(reject[0].code, RejectCode::OrderQtyExceedsLimit);

        let reject2 =
            <OrderSizeLimitPolicy as PreTradePolicy<TestOrder, ()>>::check_pre_trade_start(
                &policy,
                &PreTradeContext::new(),
                &order("EUR", "6", "10"),
            )
            .expect_err("broker barrier applies to EUR too");
        assert_eq!(reject2[0].scope, RejectScope::Order);
    }

    // ── account+asset override semantics ───────────────────────────────────

    #[test]
    fn account_asset_barrier_overrides_asset_barrier() {
        // Asset barrier allows 10, account+asset barrier allows 5.
        // Order from account 99224416 for USD should use the account+asset limit.
        let policy = OrderSizeLimitPolicy::new(
            None,
            [asset_barrier("USD", "10", "10000")],
            [OrderSizeAccountAssetBarrier {
                limit: limit("5", "10000"),
                account_id: AccountId::from_u64(99224416),
                settlement_asset: Asset::new("USD").unwrap(),
            }],
        )
        .unwrap();

        let reject =
            <OrderSizeLimitPolicy as PreTradePolicy<TestOrder, ()>>::check_pre_trade_start(
                &policy,
                &PreTradeContext::new(),
                &order("USD", "6", "10"),
            )
            .expect_err("account+asset barrier (max 5) must override asset barrier (max 10)");
        assert_eq!(reject[0].scope, RejectScope::Account);
        assert_eq!(reject[0].code, RejectCode::OrderQtyExceedsLimit);
    }

    #[test]
    fn account_asset_barrier_with_looser_limit_overrides_asset_baseline() {
        let policy = OrderSizeLimitPolicy::new(
            None,
            [asset_barrier("USD", "5", "10000")],
            [OrderSizeAccountAssetBarrier {
                limit: limit("100", "10000"),
                account_id: AccountId::from_u64(99224416),
                settlement_asset: Asset::new("USD").unwrap(),
            }],
        )
        .unwrap();

        assert!(
            <OrderSizeLimitPolicy as PreTradePolicy<TestOrder, ()>>::check_pre_trade_start(
                &policy,
                &PreTradeContext::new(),
                &order_for_account("USD", "10", "10", AccountId::from_u64(99224416)),
            )
            .is_ok()
        );

        let reject =
            <OrderSizeLimitPolicy as PreTradePolicy<TestOrder, ()>>::check_pre_trade_start(
                &policy,
                &PreTradeContext::new(),
                &order_for_account("USD", "10", "10", AccountId::from_u64(2)),
            )
            .expect_err("asset baseline must reject unmatched account");
        assert_eq!(reject[0].scope, RejectScope::Order);
        assert_eq!(reject[0].code, RejectCode::OrderQtyExceedsLimit);
    }

    #[test]
    fn unknown_settlement_passes_when_no_broker_or_account_asset_match() {
        let policy =
            OrderSizeLimitPolicy::new(None, [asset_barrier("EUR", "10", "1000")], []).unwrap();

        let result = <OrderSizeLimitPolicy as PreTradePolicy<TestOrder, ()>>::check_pre_trade_start(
            &policy,
            &PreTradeContext::new(),
            &order("USD", "1", "1"),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn axis_reject_reported_before_broker_reject_when_both_breach() {
        // Broker limit: max_qty=5. Asset limit: max_qty=3. Order: qty=6.
        // Both breach, but asset axis is reported first.
        let policy = OrderSizeLimitPolicy::new(
            Some(broker_barrier("5", "100000")),
            [asset_barrier("USD", "3", "100000")],
            [],
        )
        .unwrap();

        let reject =
            <OrderSizeLimitPolicy as PreTradePolicy<TestOrder, ()>>::check_pre_trade_start(
                &policy,
                &PreTradeContext::new(),
                &order("USD", "6", "10"),
            )
            .expect_err("must reject");
        // Asset axis breach is reported first
        assert_eq!(reject[0].scope, RejectScope::Order);
        assert_eq!(reject[0].code, RejectCode::OrderQtyExceedsLimit);
        assert!(
            reject[0].details.contains("max allowed: 3"),
            "should report asset barrier limit"
        );
    }

    // ── multiple asset barriers ────────────────────────────────────────────

    #[test]
    fn additional_asset_barriers_at_construction_are_applied() {
        let policy = OrderSizeLimitPolicy::new(
            None,
            vec![
                asset_barrier("USD", "10", "1000"),
                asset_barrier("EUR", "5", "500"),
                asset_barrier("GBP", "3", "300"),
            ],
            [],
        )
        .unwrap();

        assert!(
            <OrderSizeLimitPolicy as PreTradePolicy<TestOrder, ()>>::check_pre_trade_start(
                &policy,
                &PreTradeContext::new(),
                &order("EUR", "5", "100")
            )
            .is_ok()
        );
        assert!(
            <OrderSizeLimitPolicy as PreTradePolicy<TestOrder, ()>>::check_pre_trade_start(
                &policy,
                &PreTradeContext::new(),
                &order("GBP", "3", "100")
            )
            .is_ok()
        );

        let reject =
            <OrderSizeLimitPolicy as PreTradePolicy<TestOrder, ()>>::check_pre_trade_start(
                &policy,
                &PreTradeContext::new(),
                &order("EUR", "6", "10"),
            )
            .expect_err("exceeding EUR limit must reject");
        assert_eq!(reject[0].code, RejectCode::OrderQtyExceedsLimit);
        assert_eq!(reject[0].details, "requested 6, max allowed: 5");
    }

    // ── policy name and apply ──────────────────────────────────────────────

    #[test]
    fn policy_name_is_stable() {
        let policy =
            OrderSizeLimitPolicy::new(None, [asset_barrier("USD", "10", "1000")], []).unwrap();
        assert_eq!(
            <OrderSizeLimitPolicy as PreTradePolicy<TestOrder, ()>>::name(&policy),
            OrderSizeLimitPolicy::NAME
        );
    }

    #[test]
    fn apply_execution_report_returns_false() {
        let policy =
            OrderSizeLimitPolicy::new(None, [asset_barrier("USD", "10", "1000")], []).unwrap();
        assert!(
            <OrderSizeLimitPolicy as PreTradePolicy<TestOrder, ()>>::apply_execution_report(
                &policy,
                &()
            )
            .is_empty()
        );
    }

    // ── resolve helpers ────────────────────────────────────────────────────

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
        let policy =
            OrderSizeLimitPolicy::new(None, [asset_barrier("USD", "100", "10000")], []).unwrap();
        let order_val = OrderOperation {
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
            <OrderSizeLimitPolicy as PreTradePolicy<TestOrder, ()>>::check_pre_trade_start(
                &policy,
                &PreTradeContext::new(),
                &order_val,
            )
            .expect_err("volume order without price must reject");
        let reject = &reject[0];
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
    fn volume_overflow_is_treated_as_calculation_failed() {
        let policy =
            OrderSizeLimitPolicy::new(None, [asset_barrier("USD", "100", "1000")], []).unwrap();

        let order_val = OrderOperation {
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
            <OrderSizeLimitPolicy as PreTradePolicy<TestOrder, ()>>::check_pre_trade_start(
                &policy,
                &PreTradeContext::new(),
                &order_val,
            )
            .expect_err("overflow must be treated as calculation failed");
        let reject = &reject[0];
        assert_eq!(reject.scope, RejectScope::Order);
        assert_eq!(reject.code, RejectCode::OrderValueCalculationFailed);
        assert_eq!(reject.reason, "order value calculation failed");
        assert_eq!(
            reject.details,
            "price or quantity could not be used to evaluate order notional"
        );
    }

    // ── field access error paths ───────────────────────────────────────────

    #[test]
    fn maps_instrument_access_error_to_missing_required_field() {
        struct InstrumentAccessErrorOrder;

        impl HasInstrument for InstrumentAccessErrorOrder {
            fn instrument(&self) -> Result<&Instrument, RequestFieldAccessError> {
                Err(RequestFieldAccessError::new("instrument"))
            }
        }
        impl HasAccountId for InstrumentAccessErrorOrder {
            fn account_id(&self) -> Result<AccountId, crate::RequestFieldAccessError> {
                Ok(AccountId::from_u64(1))
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

        let policy =
            OrderSizeLimitPolicy::new(None, [asset_barrier("USD", "10", "1000")], []).unwrap();
        let order_val = InstrumentAccessErrorOrder;
        let reject = <OrderSizeLimitPolicy as PreTradePolicy<
            InstrumentAccessErrorOrder,
            (),
        >>::check_pre_trade_start(&policy, &PreTradeContext::new(), &order_val)
        .expect_err("field access error must reject");
        let reject = &reject[0];
        assert_eq!(reject.scope, RejectScope::Order);
        assert_eq!(reject.code, RejectCode::MissingRequiredField);
        assert_eq!(
            reject.reason,
            "failed to access required field 'instrument'"
        );
        assert_eq!(reject.details, "failed to access field 'instrument'");
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
        impl HasAccountId for TradeAmountAccessErrorOrder {
            fn account_id(&self) -> Result<AccountId, crate::RequestFieldAccessError> {
                Ok(AccountId::from_u64(1))
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

        let policy =
            OrderSizeLimitPolicy::new(None, [asset_barrier("USD", "10", "1000")], []).unwrap();
        let order_val = TradeAmountAccessErrorOrder {
            instrument: Instrument::new(
                Asset::new("AAPL").expect("asset code must be valid"),
                Asset::new("USD").expect("asset code must be valid"),
            ),
        };
        let reject = <OrderSizeLimitPolicy as PreTradePolicy<
            TradeAmountAccessErrorOrder,
            (),
        >>::check_pre_trade_start(&policy, &PreTradeContext::new(), &order_val)
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
        impl HasAccountId for PriceAccessErrorOrder {
            fn account_id(&self) -> Result<AccountId, crate::RequestFieldAccessError> {
                Ok(AccountId::from_u64(1))
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

        let policy =
            OrderSizeLimitPolicy::new(None, [asset_barrier("USD", "10", "1000")], []).unwrap();
        let order_val = PriceAccessErrorOrder {
            instrument: Instrument::new(
                Asset::new("AAPL").expect("asset code must be valid"),
                Asset::new("USD").expect("asset code must be valid"),
            ),
        };
        let reject = <OrderSizeLimitPolicy as PreTradePolicy<
            PriceAccessErrorOrder,
            (),
        >>::check_pre_trade_start(&policy, &PreTradeContext::new(), &order_val)
        .expect_err("field access error must reject");
        let reject = &reject[0];
        assert_eq!(reject.scope, RejectScope::Order);
        assert_eq!(reject.code, RejectCode::MissingRequiredField);
        assert_eq!(reject.reason, "failed to access required field 'price'");
        assert_eq!(reject.details, "failed to access field 'price'");
    }
}
