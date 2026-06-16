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

use std::collections::HashMap;
use std::fmt::{Display, Formatter};

use crate::core::HasAccountId;
use crate::param::{AccountId, Asset, Price, Quantity, TradeAmount, Volume};
use crate::pretrade::policy::{missing_required_field_reject, PolicyGroupId, PolicyName};
use crate::pretrade::DEFAULT_POLICY_GROUP_ID;
use crate::pretrade::{
    ConfigurablePolicy, PreTradeContext, PreTradePolicy, Reject, RejectCode, RejectScope, Rejects,
};
use crate::storage::ConfigCell;
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

/// Errors returned by [`OrderSizeLimitPolicy`] construction and by the
/// runtime setters on [`OrderSizeLimitSettings`].
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
                "at least one broker, asset, or account+asset barrier \
                 must be configured"
            ),
        }
    }
}

impl std::error::Error for OrderSizeLimitPolicyError {}

/// Runtime-updatable settings for [`OrderSizeLimitPolicy`].
///
/// Holds the full barrier configuration across all three axes. Validated
/// on construction and on every setter: at least one barrier across all
/// axes must always be present.
///
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderSizeLimitSettings {
    account_asset_limits: HashMap<(AccountId, Asset), OrderSizeLimit>,
    asset_limits: HashMap<Asset, OrderSizeLimit>,
    broker: Option<OrderSizeBrokerBarrier>,
}

impl OrderSizeLimitSettings {
    /// Validates that the barrier combination is non-empty.
    fn validate(
        broker: &Option<OrderSizeBrokerBarrier>,
        asset_limits: &HashMap<Asset, OrderSizeLimit>,
        account_asset_limits: &HashMap<(AccountId, Asset), OrderSizeLimit>,
    ) -> Result<(), OrderSizeLimitPolicyError> {
        if broker.is_none() && asset_limits.is_empty() && account_asset_limits.is_empty() {
            return Err(OrderSizeLimitPolicyError::NoBarriersConfigured);
        }
        Ok(())
    }

    /// Creates settings from explicit barrier iterables.
    ///
    /// Returns [`OrderSizeLimitPolicyError::NoBarriersConfigured`] if
    /// all axes are empty. Duplicate keys within an axis: last-write-wins.
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

        Self::validate(&broker, &asset_limits, &account_asset_limits)?;

        Ok(Self {
            account_asset_limits,
            asset_limits,
            broker,
        })
    }

    /// Replaces the broker barrier.
    ///
    /// Returns [`OrderSizeLimitPolicyError::NoBarriersConfigured`] if the
    /// new value would leave all axes empty.
    pub fn set_broker(
        &mut self,
        broker: Option<OrderSizeBrokerBarrier>,
    ) -> Result<(), OrderSizeLimitPolicyError> {
        Self::validate(&broker, &self.asset_limits, &self.account_asset_limits)?;
        self.broker = broker;
        Ok(())
    }

    /// Replaces the full set of per-asset barriers.
    ///
    /// Returns [`OrderSizeLimitPolicyError::NoBarriersConfigured`] if the
    /// new set would leave all axes empty. Duplicate keys: last-write-wins.
    pub fn set_asset_barriers(
        &mut self,
        barriers: impl IntoIterator<Item = OrderSizeAssetBarrier>,
    ) -> Result<(), OrderSizeLimitPolicyError> {
        let asset_limits: HashMap<Asset, OrderSizeLimit> = barriers
            .into_iter()
            .map(|b| (b.settlement_asset, b.limit))
            .collect();
        Self::validate(&self.broker, &asset_limits, &self.account_asset_limits)?;
        self.asset_limits = asset_limits;
        Ok(())
    }

    /// Replaces the full set of per-(account, asset) barriers.
    ///
    /// Returns [`OrderSizeLimitPolicyError::NoBarriersConfigured`] if the
    /// new set would leave all axes empty. Duplicate keys: last-write-wins.
    pub fn set_account_asset_barriers(
        &mut self,
        barriers: impl IntoIterator<Item = OrderSizeAccountAssetBarrier>,
    ) -> Result<(), OrderSizeLimitPolicyError> {
        let account_asset_limits: HashMap<(AccountId, Asset), OrderSizeLimit> = barriers
            .into_iter()
            .map(|b| ((b.account_id, b.settlement_asset), b.limit))
            .collect();
        Self::validate(&self.broker, &self.asset_limits, &account_asset_limits)?;
        self.account_asset_limits = account_asset_limits;
        Ok(())
    }
}

/// Start-stage policy enforcing per-settlement order size limits.
///
/// Three configurable barrier axes - broker (all orders), per settlement
/// asset, and per (account, settlement asset).
///
/// Resolution and check semantics for an order with `(account, settlement)`:
///
/// 1. **Asset-axis chain (override).** Picks at most one asset-axis limit:
///    - if an `account+asset` barrier covers `(account, settlement)`, that
///      limit wins;
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
///     OrderSizeLimitSettings,
/// };
/// use openpit::storage::NoLocking;
/// use openpit::{Engine, Instrument, OrderOperation};
///
/// let settings = OrderSizeLimitSettings::new(
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
/// let policy = OrderSizeLimitPolicy::<NoLocking>::new(settings);
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
pub struct OrderSizeLimitPolicy<LockingPolicyFactory>
where
    LockingPolicyFactory: crate::storage::LockingPolicyFactory,
{
    group_id: PolicyGroupId,
    settings: <LockingPolicyFactory as crate::storage::LockingPolicyFactory>::Config<
        OrderSizeLimitSettings,
    >,
}

impl<LockingPolicyFactory> OrderSizeLimitPolicy<LockingPolicyFactory>
where
    LockingPolicyFactory: crate::storage::LockingPolicyFactory,
{
    /// Stable policy name.
    pub const NAME: &'static str = "OrderSizeLimitPolicy";

    /// Creates an order-size policy from validated settings.
    pub fn new(settings: OrderSizeLimitSettings) -> Self {
        Self {
            group_id: DEFAULT_POLICY_GROUP_ID,
            settings: <LockingPolicyFactory as crate::storage::LockingPolicyFactory>::new_config(
                settings,
            ),
        }
    }

    /// Assigns a group tag to this policy instance.
    ///
    /// See [`PolicyGroupId`] and [`DEFAULT_POLICY_GROUP_ID`] for details.
    pub fn with_policy_group_id(mut self, id: PolicyGroupId) -> Self {
        self.group_id = id;
        self
    }
}

impl<LockingPolicyFactory> PolicyName for OrderSizeLimitPolicy<LockingPolicyFactory>
where
    LockingPolicyFactory: crate::storage::LockingPolicyFactory,
{
    fn policy_name(&self) -> &str {
        Self::NAME
    }
}

impl<LockingPolicyFactory, Order, ExecutionReport, AccountAdjustment, Sync>
    PreTradePolicy<Order, ExecutionReport, AccountAdjustment, Sync>
    for OrderSizeLimitPolicy<LockingPolicyFactory>
where
    LockingPolicyFactory:
        crate::storage::LockingPolicyFactory + crate::storage::CreateStorageFor<AccountId>,
    Order: HasInstrument + HasTradeAmount + HasOrderPrice + HasAccountId,
    Sync: crate::core::SyncMode<StorageLockingPolicyFactory = LockingPolicyFactory>,
{
    fn name(&self) -> &str {
        Self::NAME
    }

    fn policy_group_id(&self) -> PolicyGroupId {
        self.group_id
    }

    #[allow(private_interfaces)]
    fn built_in_config_entry(&self) -> Option<crate::core::ConfigEntry<LockingPolicyFactory>> {
        Some(crate::core::ConfigEntry::OrderSizeLimit(
            crate::pretrade::ConfigurablePolicy::settings_cell(self),
        ))
    }

    fn check_pre_trade_start(
        &self,
        _ctx: &PreTradeContext<<Sync as crate::core::SyncMode>::StorageLockingPolicyFactory>,
        order: &Order,
    ) -> Result<(), Rejects> {
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
        // Broker axis: applied additively on top of whatever the asset-axis chain produced.
        let (axis_reject, broker_reject) = self.settings.with(|s| {
            let (axis_limit, axis_scope) = if let Some(limit) = s
                .account_asset_limits
                .get(&(account_id, settlement.clone()))
            {
                (Some(limit), RejectScope::Account)
            } else if let Some(limit) = s.asset_limits.get(settlement) {
                (Some(limit), RejectScope::Order)
            } else {
                (None, RejectScope::Order)
            };

            let broker_limit = s.broker.as_ref().map(|b| &b.limit);

            if axis_limit.is_none() && broker_limit.is_none() {
                return (None, None);
            }

            let quantity = resolve_quantity(Self::NAME, trade_amount, price);
            let notional = resolve_notional(Self::NAME, trade_amount, price);

            // Resolve both eagerly; propagate calculation errors as axis/broker
            // rejects that will surface before size-limit rejects.
            let (quantity, notional) = match (quantity, notional) {
                (Ok(q), Ok(n)) => (q, n),
                (Err(e), _) => return (Some(Err(Rejects::from(e))), None),
                (_, Err(e)) => return (Some(Err(Rejects::from(e))), None),
            };

            // Check axis limit first; if both breach, axis is reported first.
            let axis_r = axis_limit
                .and_then(|limit| {
                    check_limit_optional(Self::NAME, limit, quantity, notional, axis_scope)
                })
                .map(Rejects::from)
                .map(Err);

            let broker_r = broker_limit
                .and_then(|limit| {
                    check_limit_optional(Self::NAME, limit, quantity, notional, RejectScope::Order)
                })
                .map(Rejects::from)
                .map(Err);

            (axis_r, broker_r)
        });

        if let Some(result) = axis_reject.or(broker_reject) {
            return result;
        }

        Ok(())
    }
}

impl<LockingPolicyFactory> ConfigurablePolicy<LockingPolicyFactory>
    for OrderSizeLimitPolicy<LockingPolicyFactory>
where
    LockingPolicyFactory: crate::storage::LockingPolicyFactory,
{
    type Settings = OrderSizeLimitSettings;

    fn settings_cell(
        &self,
    ) -> <LockingPolicyFactory as crate::storage::LockingPolicyFactory>::Config<
        OrderSizeLimitSettings,
    > {
        self.settings.clone()
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
    use crate::storage::NoLocking;
    use crate::{HasInstrument, HasOrderPrice, HasTradeAmount, RequestFieldAccessError};
    use rust_decimal::Decimal;

    use super::{
        OrderSizeAccountAssetBarrier, OrderSizeAssetBarrier, OrderSizeBrokerBarrier,
        OrderSizeLimit, OrderSizeLimitPolicy, OrderSizeLimitPolicyError, OrderSizeLimitSettings,
    };

    type TestPolicy = OrderSizeLimitPolicy<NoLocking>;
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

    fn settings(
        broker: Option<OrderSizeBrokerBarrier>,
        asset_barriers: impl IntoIterator<Item = OrderSizeAssetBarrier>,
        account_asset_barriers: impl IntoIterator<Item = OrderSizeAccountAssetBarrier>,
    ) -> OrderSizeLimitSettings {
        OrderSizeLimitSettings::new(broker, asset_barriers, account_asset_barriers)
            .expect("settings must be valid in helper")
    }

    fn policy(
        broker: Option<OrderSizeBrokerBarrier>,
        asset_barriers: impl IntoIterator<Item = OrderSizeAssetBarrier>,
        account_asset_barriers: impl IntoIterator<Item = OrderSizeAccountAssetBarrier>,
    ) -> TestPolicy {
        TestPolicy::new(settings(broker, asset_barriers, account_asset_barriers))
    }

    fn check(p: &TestPolicy, order: &TestOrder) -> Result<(), crate::pretrade::Rejects> {
        <TestPolicy as PreTradePolicy<TestOrder, (), (), crate::core::LocalSync>>::check_pre_trade_start(
            p,
            &PreTradeContext::<NoLocking>::new(None),
            order,
        )
    }

    // ── settings validation ────────────────────────────────────────────────

    #[test]
    fn no_barriers_configured_rejected_by_settings_constructor() {
        let err = OrderSizeLimitSettings::new(None, [], []).expect_err("must fail");
        assert_eq!(err, OrderSizeLimitPolicyError::NoBarriersConfigured);
        assert_eq!(
            err.to_string(),
            "at least one broker, asset, or account+asset barrier \
             must be configured"
        );
    }

    #[test]
    fn set_broker_to_none_rejected_when_other_axes_empty() {
        let mut s = settings(Some(broker_barrier("5", "500")), [], []);
        let err = s.set_broker(None).expect_err("must fail");
        assert_eq!(err, OrderSizeLimitPolicyError::NoBarriersConfigured);
        // Original broker is still present after failed mutation.
        assert!(s.broker.is_some());
    }

    #[test]
    fn set_asset_barriers_to_empty_rejected_when_other_axes_empty() {
        let mut s = settings(None, [asset_barrier("USD", "10", "1000")], []);
        let err = s.set_asset_barriers([]).expect_err("must fail");
        assert_eq!(err, OrderSizeLimitPolicyError::NoBarriersConfigured);
    }

    // ── constructor validation (no_barriers) ──────────────────────────────

    #[test]
    fn no_barriers_configured_rejected_by_constructor() {
        let err = OrderSizeLimitSettings::new(None, [], []).expect_err("must fail");
        assert_eq!(err, OrderSizeLimitPolicyError::NoBarriersConfigured);
    }

    // ── asset barrier ──────────────────────────────────────────────────────

    #[test]
    fn quantity_violation_returns_order_quantity_exceeded() {
        let p = policy(None, [asset_barrier("USD", "10", "1000")], []);

        let reject = check(&p, &order("USD", "11", "90")).expect_err("quantity must be rejected");
        let reject = &reject[0];
        assert_eq!(reject.scope, RejectScope::Order);
        assert_eq!(reject.code, RejectCode::OrderQtyExceedsLimit);
        assert_eq!(reject.reason, "order quantity exceeded");
        assert_eq!(reject.details, "requested 11, max allowed: 10");
    }

    #[test]
    fn notional_violation_returns_order_notional_exceeded() {
        let p = policy(None, [asset_barrier("USD", "10", "1000")], []);

        let reject = check(&p, &order("USD", "10", "101")).expect_err("notional must be rejected");
        let reject = &reject[0];
        assert_eq!(reject.scope, RejectScope::Order);
        assert_eq!(reject.code, RejectCode::OrderNotionalExceedsLimit);
        assert_eq!(reject.reason, "order notional exceeded");
        assert_eq!(reject.details, "requested 1010, max allowed: 1000");
    }

    #[test]
    fn both_violations_are_returned_in_single_reject() {
        let p = policy(None, [asset_barrier("USD", "10", "1000")], []);

        let reject = check(&p, &order("USD", "11", "100"))
            .expect_err("quantity and notional must be rejected");
        let reject = &reject[0];
        assert_eq!(reject.scope, RejectScope::Order);
        assert_eq!(reject.code, RejectCode::OrderExceedsLimit);
        assert_eq!(reject.reason, "order size exceeded");
        assert_eq!(
            reject.details,
            "requested quantity 11, max allowed: 10; \
             requested notional 1100, max allowed: 1000"
        );
    }

    #[test]
    fn no_applicable_limit_passes_silently() {
        let p = policy(None, [asset_barrier("EUR", "10", "1000")], []);
        assert!(check(&p, &order("USD", "1", "1")).is_ok());
    }

    #[test]
    fn boundary_values_are_accepted() {
        let p = policy(None, [asset_barrier("USD", "10", "1000")], []);
        assert!(check(&p, &order("USD", "10", "100")).is_ok());
    }

    // ── broker barrier ─────────────────────────────────────────────────────

    #[test]
    fn broker_barrier_applies_regardless_of_settlement() {
        let p = policy(Some(broker_barrier("5", "500")), [], []);

        let reject = check(&p, &order("USD", "6", "10")).expect_err("broker barrier must reject");
        assert_eq!(reject[0].scope, RejectScope::Order);
        assert_eq!(reject[0].code, RejectCode::OrderQtyExceedsLimit);

        let reject2 =
            check(&p, &order("EUR", "6", "10")).expect_err("broker barrier applies to EUR too");
        assert_eq!(reject2[0].scope, RejectScope::Order);
    }

    // ── account+asset override semantics ───────────────────────────────────

    #[test]
    fn account_asset_barrier_overrides_asset_barrier() {
        // Asset barrier allows 10, account+asset barrier allows 5.
        // Order from account 99224416 for USD should use the account+asset limit.
        let p = policy(
            None,
            [asset_barrier("USD", "10", "10000")],
            [OrderSizeAccountAssetBarrier {
                limit: limit("5", "10000"),
                account_id: AccountId::from_u64(99224416),
                settlement_asset: Asset::new("USD").unwrap(),
            }],
        );

        let reject = check(&p, &order("USD", "6", "10"))
            .expect_err("account+asset barrier (max 5) must override asset barrier (max 10)");
        assert_eq!(reject[0].scope, RejectScope::Account);
        assert_eq!(reject[0].code, RejectCode::OrderQtyExceedsLimit);
    }

    #[test]
    fn account_asset_barrier_with_looser_limit_overrides_asset_baseline() {
        let p = policy(
            None,
            [asset_barrier("USD", "5", "10000")],
            [OrderSizeAccountAssetBarrier {
                limit: limit("100", "10000"),
                account_id: AccountId::from_u64(99224416),
                settlement_asset: Asset::new("USD").unwrap(),
            }],
        );

        assert!(check(
            &p,
            &order_for_account("USD", "10", "10", AccountId::from_u64(99224416))
        )
        .is_ok());

        let reject = check(
            &p,
            &order_for_account("USD", "10", "10", AccountId::from_u64(2)),
        )
        .expect_err("asset baseline must reject unmatched account");
        assert_eq!(reject[0].scope, RejectScope::Order);
        assert_eq!(reject[0].code, RejectCode::OrderQtyExceedsLimit);
    }

    #[test]
    fn unknown_settlement_passes_when_no_broker_or_account_asset_match() {
        let p = policy(None, [asset_barrier("EUR", "10", "1000")], []);
        assert!(check(&p, &order("USD", "1", "1")).is_ok());
    }

    #[test]
    fn axis_reject_reported_before_broker_reject_when_both_breach() {
        // Broker limit: max_qty=5. Asset limit: max_qty=3. Order: qty=6.
        // Both breach, but asset axis is reported first.
        let p = policy(
            Some(broker_barrier("5", "100000")),
            [asset_barrier("USD", "3", "100000")],
            [],
        );

        let reject = check(&p, &order("USD", "6", "10")).expect_err("must reject");
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
        let p = policy(
            None,
            vec![
                asset_barrier("USD", "10", "1000"),
                asset_barrier("EUR", "5", "500"),
                asset_barrier("GBP", "3", "300"),
            ],
            [],
        );

        assert!(check(&p, &order("EUR", "5", "100")).is_ok());
        assert!(check(&p, &order("GBP", "3", "100")).is_ok());

        let reject =
            check(&p, &order("EUR", "6", "10")).expect_err("exceeding EUR limit must reject");
        assert_eq!(reject[0].code, RejectCode::OrderQtyExceedsLimit);
        assert_eq!(reject[0].details, "requested 6, max allowed: 5");
    }

    // ── policy name and apply ──────────────────────────────────────────────

    #[test]
    fn policy_name_is_stable() {
        let p = policy(None, [asset_barrier("USD", "10", "1000")], []);
        assert_eq!(
            <TestPolicy as PreTradePolicy<TestOrder, (), (), crate::core::LocalSync>>::name(&p),
            OrderSizeLimitPolicy::<NoLocking>::NAME
        );
    }

    #[test]
    fn apply_execution_report_returns_false() {
        let p = policy(None, [asset_barrier("USD", "10", "1000")], []);
        assert!(<TestPolicy as PreTradePolicy<
            TestOrder,
            (),
            (),
            crate::core::LocalSync,
        >>::apply_execution_report(
            &p, &crate::pretrade::PostTradeContext::new(), &()
        )
        .is_none());
    }

    // ── ConfigurablePolicy ─────────────────────────────────────────────────

    #[test]
    fn settings_cell_clone_shares_underlying_value() {
        use crate::pretrade::ConfigurablePolicy;
        use crate::storage::ConfigCell;

        let p = policy(None, [asset_barrier("USD", "10", "1000")], []);
        let cell = p.settings_cell();

        // Update through the policy's own cell (via update on the clone).
        // Notional limit set to 3000 so that 21*100=2100 stays under it,
        // allowing the qty-only exceed path to fire.
        cell.update::<OrderSizeLimitPolicyError>(|s| {
            s.set_asset_barriers([asset_barrier("USD", "20", "3000")])
        })
        .expect("update must succeed");

        // The running policy observes the new limit through its own field.
        // qty=15 < new limit 20 → passes.
        assert!(check(&p, &order("USD", "15", "100")).is_ok());
        // qty=21 > new limit 20, notional=2100 < limit 3000 → qty-only reject.
        let reject = check(&p, &order("USD", "21", "100")).expect_err("21 exceeds new limit of 20");
        assert_eq!(reject[0].code, RejectCode::OrderQtyExceedsLimit);
        assert!(reject[0].details.contains("max allowed: 20"));
    }

    // ── resolve helpers ────────────────────────────────────────────────────

    #[test]
    fn resolve_notional_covers_volume_and_missing_price_paths() {
        let from_volume = super::resolve_notional(
            OrderSizeLimitPolicy::<NoLocking>::NAME,
            TradeAmount::Volume(Volume::from_str("123").expect("volume literal must be valid")),
            None,
        )
        .expect("volume amount should resolve notional without price");
        assert_eq!(
            from_volume,
            Volume::from_str("123").expect("volume literal must be valid")
        );

        let missing_price = super::resolve_notional(
            OrderSizeLimitPolicy::<NoLocking>::NAME,
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
        let p = policy(None, [asset_barrier("USD", "100", "10000")], []);
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
        let reject = check(&p, &order_val).expect_err("volume order without price must reject");
        let reject = &reject[0];
        assert_eq!(reject.code, RejectCode::OrderValueCalculationFailed);
    }

    #[test]
    fn resolve_quantity_covers_invalid_volume_conversion_and_missing_price_paths() {
        let conversion_failed = super::resolve_quantity(
            OrderSizeLimitPolicy::<NoLocking>::NAME,
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
            OrderSizeLimitPolicy::<NoLocking>::NAME,
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
        let p = policy(None, [asset_barrier("USD", "100", "1000")], []);

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
            check(&p, &order_val).expect_err("overflow must be treated as calculation failed");
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

        let p = policy(None, [asset_barrier("USD", "10", "1000")], []);
        let order_val = InstrumentAccessErrorOrder;
        let reject = <TestPolicy as PreTradePolicy<
            InstrumentAccessErrorOrder,
            (),
            (),
            crate::core::LocalSync,
        >>::check_pre_trade_start(
            &p, &PreTradeContext::<NoLocking>::new(None), &order_val
        )
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

        let p = policy(None, [asset_barrier("USD", "10", "1000")], []);
        let order_val = TradeAmountAccessErrorOrder {
            instrument: Instrument::new(
                Asset::new("AAPL").expect("asset code must be valid"),
                Asset::new("USD").expect("asset code must be valid"),
            ),
        };
        let reject = <TestPolicy as PreTradePolicy<
            TradeAmountAccessErrorOrder,
            (),
            (),
            crate::core::LocalSync,
        >>::check_pre_trade_start(
            &p, &PreTradeContext::<NoLocking>::new(None), &order_val
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

        let p = policy(None, [asset_barrier("USD", "10", "1000")], []);
        let order_val = PriceAccessErrorOrder {
            instrument: Instrument::new(
                Asset::new("AAPL").expect("asset code must be valid"),
                Asset::new("USD").expect("asset code must be valid"),
            ),
        };
        let reject = <TestPolicy as PreTradePolicy<
            PriceAccessErrorOrder,
            (),
            (),
            crate::core::LocalSync,
        >>::check_pre_trade_start(
            &p, &PreTradeContext::<NoLocking>::new(None), &order_val
        )
        .expect_err("field access error must reject");
        let reject = &reject[0];
        assert_eq!(reject.scope, RejectScope::Order);
        assert_eq!(reject.code, RejectCode::MissingRequiredField);
        assert_eq!(reject.reason, "failed to access required field 'price'");
        assert_eq!(reject.details, "failed to access field 'price'");
    }
}
