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

use std::collections::hash_map::Entry;
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

/// Optional order quantity and notional caps for a single order.
///
/// Each cap is independent. Asset-keyed barriers resolve `max_quantity` by
/// the instrument's underlying asset and `max_notional` by its settlement
/// asset. A missing cap constrains nothing, and a limit with both caps missing
/// is rejected by [`OrderSizeLimitSettings`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OrderSizeLimit {
    /// Maximum allowed order quantity, or no quantity constraint.
    pub max_quantity: Option<Quantity>,
    /// Maximum allowed order notional, or no notional constraint.
    pub max_notional: Option<Volume>,
}

/// Broker-wide order size limit.
///
/// Applies to every order regardless of account or instrument assets. Each
/// cap may be absent and then constrains nothing, but at least one cap must be
/// present.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderSizeBrokerBarrier {
    /// Size limit for this broker barrier.
    pub limit: OrderSizeLimit,
}

/// Per-asset order size limit.
///
/// [`OrderSizeLimit::max_quantity`] applies when the instrument's underlying
/// asset matches [`Self::asset`]. [`OrderSizeLimit::max_notional`] applies when
/// the instrument's settlement asset matches it. The barrier is shared across
/// all accounts. A missing cap constrains nothing. This asset level is the last
/// level of the resolution chain, so a cap absent here leaves the metric with no
/// asset-chain cap; only an additive broker barrier can still constrain it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderSizeAssetBarrier {
    /// Size limit for this asset barrier.
    pub limit: OrderSizeLimit,
    /// Asset key used independently for quantity and notional lookup.
    pub asset: Asset,
}

/// Per-(account, asset) order size limit.
///
/// For the matching account, [`OrderSizeLimit::max_quantity`] applies when the
/// instrument's underlying asset matches [`Self::asset`], while
/// [`OrderSizeLimit::max_notional`] applies when its settlement asset matches.
/// A missing cap constrains nothing, so the metric's resolution chain continues
/// to the matching asset-level barrier.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderSizeAccountAssetBarrier {
    /// Size limit for this account+asset barrier.
    pub limit: OrderSizeLimit,
    /// Account this barrier applies to.
    pub account_id: AccountId,
    /// Asset key used independently for quantity and notional lookup.
    pub asset: Asset,
}

/// Errors returned by [`OrderSizeLimitPolicy`] construction and by the
/// runtime setters on [`OrderSizeLimitSettings`].
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrderSizeLimitPolicyError {
    /// No barriers were provided across all axes.
    NoBarriersConfigured,
    /// Neither quantity nor notional was configured for a limit.
    NoCapsConfigured,
    /// An asset key was repeated on the asset axis.
    DuplicateAssetBarrier {
        /// Asset whose key was repeated.
        asset: Asset,
    },
    /// An (account, asset) key was repeated on the account+asset axis.
    DuplicateAccountAssetBarrier {
        /// Asset whose (account, asset) key was repeated.
        asset: Asset,
    },
}

impl Display for OrderSizeLimitPolicyError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoBarriersConfigured => write!(
                f,
                "at least one broker, asset, or account+asset barrier \
                 must be configured"
            ),
            Self::NoCapsConfigured => write!(
                f,
                "at least one of max_quantity or max_notional \
                 must be configured"
            ),
            Self::DuplicateAssetBarrier { asset } => {
                write!(f, "duplicate asset barrier for asset {asset}")
            }
            Self::DuplicateAccountAssetBarrier { asset } => {
                write!(f, "duplicate account+asset barrier for asset {asset}")
            }
        }
    }
}

impl std::error::Error for OrderSizeLimitPolicyError {}

/// Runtime-updatable settings for [`OrderSizeLimitPolicy`].
///
/// Holds the full barrier configuration across all three axes. Construction
/// and every setter require at least one barrier across all axes and at least
/// one cap in every limit.
///
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderSizeLimitSettings {
    account_asset_limits: HashMap<(AccountId, Asset), OrderSizeLimit>,
    asset_limits: HashMap<Asset, OrderSizeLimit>,
    broker: Option<OrderSizeBrokerBarrier>,
}

impl OrderSizeLimitSettings {
    fn validate_limit(limit: &OrderSizeLimit) -> Result<(), OrderSizeLimitPolicyError> {
        if limit.max_quantity.is_none() && limit.max_notional.is_none() {
            return Err(OrderSizeLimitPolicyError::NoCapsConfigured);
        }
        Ok(())
    }

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
    /// Returns [`OrderSizeLimitPolicyError::NoBarriersConfigured`] if all axes
    /// are empty, or [`OrderSizeLimitPolicyError::NoCapsConfigured`] if any
    /// supplied limit has neither cap. A duplicate key within either collection
    /// is rejected.
    pub fn new(
        broker: Option<OrderSizeBrokerBarrier>,
        asset_barriers: impl IntoIterator<Item = OrderSizeAssetBarrier>,
        account_asset_barriers: impl IntoIterator<Item = OrderSizeAccountAssetBarrier>,
    ) -> Result<Self, OrderSizeLimitPolicyError> {
        if let Some(barrier) = &broker {
            Self::validate_limit(&barrier.limit)?;
        }

        let mut asset_limits = HashMap::new();
        for barrier in asset_barriers {
            Self::validate_limit(&barrier.limit)?;
            match asset_limits.entry(barrier.asset) {
                Entry::Vacant(entry) => {
                    entry.insert(barrier.limit);
                }
                Entry::Occupied(entry) => {
                    return Err(OrderSizeLimitPolicyError::DuplicateAssetBarrier {
                        asset: entry.key().clone(),
                    });
                }
            }
        }

        let mut account_asset_limits = HashMap::new();
        for barrier in account_asset_barriers {
            Self::validate_limit(&barrier.limit)?;
            match account_asset_limits.entry((barrier.account_id, barrier.asset)) {
                Entry::Vacant(entry) => {
                    entry.insert(barrier.limit);
                }
                Entry::Occupied(entry) => {
                    return Err(OrderSizeLimitPolicyError::DuplicateAccountAssetBarrier {
                        asset: entry.key().1.clone(),
                    });
                }
            }
        }

        Self::validate(&broker, &asset_limits, &account_asset_limits)?;

        Ok(Self {
            account_asset_limits,
            asset_limits,
            broker,
        })
    }

    /// Replaces the broker barrier.
    ///
    /// Returns [`OrderSizeLimitPolicyError::NoBarriersConfigured`] if the new
    /// value would leave all axes empty, or
    /// [`OrderSizeLimitPolicyError::NoCapsConfigured`] if the supplied limit
    /// has neither cap.
    pub fn set_broker(
        &mut self,
        broker: Option<OrderSizeBrokerBarrier>,
    ) -> Result<(), OrderSizeLimitPolicyError> {
        if let Some(barrier) = &broker {
            Self::validate_limit(&barrier.limit)?;
        }
        Self::validate(&broker, &self.asset_limits, &self.account_asset_limits)?;
        self.broker = broker;
        Ok(())
    }

    /// Replaces the full set of per-asset barriers.
    ///
    /// Returns [`OrderSizeLimitPolicyError::NoBarriersConfigured`] if the new
    /// set would leave all axes empty, or
    /// [`OrderSizeLimitPolicyError::NoCapsConfigured`] if any supplied limit
    /// has neither cap. A duplicate asset key is rejected.
    pub fn set_asset_barriers(
        &mut self,
        barriers: impl IntoIterator<Item = OrderSizeAssetBarrier>,
    ) -> Result<(), OrderSizeLimitPolicyError> {
        let mut asset_limits = HashMap::new();
        for barrier in barriers {
            Self::validate_limit(&barrier.limit)?;
            match asset_limits.entry(barrier.asset) {
                Entry::Vacant(entry) => {
                    entry.insert(barrier.limit);
                }
                Entry::Occupied(entry) => {
                    return Err(OrderSizeLimitPolicyError::DuplicateAssetBarrier {
                        asset: entry.key().clone(),
                    });
                }
            }
        }
        Self::validate(&self.broker, &asset_limits, &self.account_asset_limits)?;
        self.asset_limits = asset_limits;
        Ok(())
    }

    /// Replaces the full set of per-(account, asset) barriers.
    ///
    /// Returns [`OrderSizeLimitPolicyError::NoBarriersConfigured`] if the new
    /// set would leave all axes empty, or
    /// [`OrderSizeLimitPolicyError::NoCapsConfigured`] if any supplied limit
    /// has neither cap. A duplicate (account, asset) key is rejected.
    pub fn set_account_asset_barriers(
        &mut self,
        barriers: impl IntoIterator<Item = OrderSizeAccountAssetBarrier>,
    ) -> Result<(), OrderSizeLimitPolicyError> {
        let mut account_asset_limits = HashMap::new();
        for barrier in barriers {
            Self::validate_limit(&barrier.limit)?;
            match account_asset_limits.entry((barrier.account_id, barrier.asset)) {
                Entry::Vacant(entry) => {
                    entry.insert(barrier.limit);
                }
                Entry::Occupied(entry) => {
                    return Err(OrderSizeLimitPolicyError::DuplicateAccountAssetBarrier {
                        asset: entry.key().1.clone(),
                    });
                }
            }
        }
        Self::validate(&self.broker, &self.asset_limits, &account_asset_limits)?;
        self.account_asset_limits = account_asset_limits;
        Ok(())
    }
}

/// Start-stage policy enforcing order size limits by instrument asset.
///
/// Three configurable barrier axes are available: broker (all orders), asset,
/// and (account, asset). The two fields of an asset-keyed limit resolve their
/// override chains independently:
///
/// 1. **Quantity chain.** Looks up `(account, underlying asset)`, then the
///    underlying asset. The first matching barrier carrying `max_quantity`
///    supplies the cap; a barrier without it is skipped.
/// 2. **Notional chain.** Looks up `(account, settlement asset)`, then the
///    settlement asset. The first matching barrier carrying `max_notional`
///    supplies the cap; a barrier without it is skipped.
/// 3. **Broker axis (additive).** A broker barrier applies each cap it carries
///    in addition to the two asset chains. An absent cap constrains nothing.
/// 4. Asset-chain rejects are returned before broker rejects. When both asset
///    metrics fail, the combined reject names the asset supplying each cap; an
///    account-level component keeps the combined reject account-scoped.
/// 5. A metric without an applicable cap is not resolved or compared.
///
/// Drop-copy operations replay historical orders and bypass these admission
/// limits. The policy does not request their instrument, account, trade amount,
/// or price.
///
/// Constructor rules:
/// - at least one barrier across all three axes must be configured;
/// - if all are omitted, the constructor returns
///   [`OrderSizeLimitPolicyError::NoBarriersConfigured`];
/// - every limit must carry `max_quantity`, `max_notional`, or both;
/// - a limit carrying neither returns
///   [`OrderSizeLimitPolicyError::NoCapsConfigured`];
/// - duplicate keys within an axis return the corresponding duplicate-key error.
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
///     [
///         OrderSizeAssetBarrier {
///             limit: OrderSizeLimit {
///                 max_quantity: Some(Quantity::from_f64(100.0)?),
///                 max_notional: None,
///             },
///             asset: Asset::new("AAPL")?,
///         },
///         OrderSizeAssetBarrier {
///             limit: OrderSizeLimit {
///                 max_quantity: None,
///                 max_notional: Some(Volume::from_f64(50000.0)?),
///             },
///             asset: Asset::new("USD")?,
///         },
///     ],
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
        ctx: &PreTradeContext<<Sync as crate::core::SyncMode>::StorageLockingPolicyFactory>,
        order: &Order,
    ) -> Result<(), Rejects> {
        // This policy owns live admission only. Once an order has executed it
        // has no historical bookkeeping or account-control effect to apply.
        if ctx.is_drop_copy() {
            return Ok(());
        }
        let instrument = order
            .instrument()
            .map_err(|e| Rejects::from(missing_required_field_reject(self, "instrument", &e)))?;
        let account_id = order
            .account_id()
            .map_err(|e| Rejects::from(missing_required_field_reject(self, "account ID", &e)))?;
        let trade_amount = order
            .trade_amount()
            .map_err(|e| Rejects::from(missing_required_field_reject(self, "trade amount", &e)))?;

        let underlying = instrument.underlying_asset();
        let settlement = instrument.settlement_asset();

        let (quantity_axis_limit, notional_axis_limit, broker_limit) =
            self.settings.with(|settings| {
                (
                    select_asset_limit(settings, account_id, underlying, |limit| {
                        limit.max_quantity
                    }),
                    select_asset_limit(settings, account_id, settlement, |limit| {
                        limit.max_notional
                    }),
                    settings.broker.as_ref().map(|barrier| barrier.limit),
                )
            });

        let quantity_broker_limit =
            broker_limit
                .and_then(|limit| limit.max_quantity)
                .map(|maximum| SelectedMetricLimit {
                    maximum,
                    scope: RejectScope::Order,
                });
        let notional_broker_limit =
            broker_limit
                .and_then(|limit| limit.max_notional)
                .map(|maximum| SelectedMetricLimit {
                    maximum,
                    scope: RejectScope::Order,
                });

        let quantity_needed = quantity_axis_limit.is_some() || quantity_broker_limit.is_some();
        let notional_needed = notional_axis_limit.is_some() || notional_broker_limit.is_some();
        if !quantity_needed && !notional_needed {
            return Ok(());
        }

        let quantity_needs_price =
            matches!(trade_amount, TradeAmount::Volume(_)) && quantity_needed;
        let notional_needs_price =
            matches!(trade_amount, TradeAmount::Quantity(_)) && notional_needed;
        let price = if quantity_needs_price || notional_needs_price {
            order.price().map_err(|error| {
                Rejects::from(missing_required_field_reject(self, "price", &error))
            })?
        } else {
            None
        };

        let (quantity_axis, quantity_broker) =
            resolve_metric_limits(quantity_axis_limit, quantity_broker_limit, || {
                resolve_quantity(Self::NAME, trade_amount, price)
            })
            .map_err(Rejects::from)?;
        let (notional_axis, notional_broker) =
            resolve_metric_limits(notional_axis_limit, notional_broker_limit, || {
                resolve_notional(Self::NAME, trade_amount, price)
            })
            .map_err(Rejects::from)?;

        let axis_reject = check_limit_optional(
            Self::NAME,
            quantity_axis,
            notional_axis,
            Some((underlying, settlement)),
        );
        let broker_reject =
            check_limit_optional(Self::NAME, quantity_broker, notional_broker, None);

        if let Some(reject) = axis_reject.or(broker_reject) {
            return Err(Rejects::from(reject));
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

struct SelectedMetricLimit<MetricValue> {
    maximum: MetricValue,
    scope: RejectScope,
}

impl<MetricValue> SelectedMetricLimit<MetricValue> {
    fn resolve(self, requested: MetricValue) -> ResolvedMetricLimit<MetricValue> {
        ResolvedMetricLimit {
            requested,
            maximum: self.maximum,
            scope: self.scope,
        }
    }
}

struct ResolvedMetricLimit<MetricValue> {
    requested: MetricValue,
    maximum: MetricValue,
    scope: RejectScope,
}

fn select_asset_limit<MetricValue: Copy>(
    settings: &OrderSizeLimitSettings,
    account_id: AccountId,
    asset: &Asset,
    metric: impl Fn(&OrderSizeLimit) -> Option<MetricValue>,
) -> Option<SelectedMetricLimit<MetricValue>> {
    if let Some(limit) = settings
        .account_asset_limits
        .get(&(account_id, asset.clone()))
    {
        if let Some(maximum) = metric(limit) {
            return Some(SelectedMetricLimit {
                maximum,
                scope: RejectScope::Account,
            });
        }
    }

    settings.asset_limits.get(asset).and_then(|limit| {
        metric(limit).map(|maximum| SelectedMetricLimit {
            maximum,
            scope: RejectScope::Order,
        })
    })
}

type ResolvedMetricPair<MetricValue> = (
    Option<ResolvedMetricLimit<MetricValue>>,
    Option<ResolvedMetricLimit<MetricValue>>,
);

fn resolve_metric_limits<MetricValue: Copy>(
    axis_limit: Option<SelectedMetricLimit<MetricValue>>,
    broker_limit: Option<SelectedMetricLimit<MetricValue>>,
    resolve: impl FnOnce() -> Result<MetricValue, Reject>,
) -> Result<ResolvedMetricPair<MetricValue>, Reject> {
    if axis_limit.is_none() && broker_limit.is_none() {
        return Ok((None, None));
    }

    let requested = resolve()?;
    Ok((
        axis_limit.map(|limit| limit.resolve(requested)),
        broker_limit.map(|limit| limit.resolve(requested)),
    ))
}

fn check_limit_optional(
    policy: &str,
    quantity: Option<ResolvedMetricLimit<Quantity>>,
    notional: Option<ResolvedMetricLimit<Volume>>,
    assets: Option<(&Asset, &Asset)>,
) -> Option<Reject> {
    let quantity = quantity.filter(|limit| limit.requested > limit.maximum);
    let notional = notional.filter(|limit| limit.requested > limit.maximum);

    match (quantity, notional) {
        (None, None) => None,
        (Some(quantity), None) => Some(Reject::new(
            policy,
            quantity.scope,
            RejectCode::OrderQtyExceedsLimit,
            "order quantity exceeded",
            format!(
                "requested {}, max allowed: {}",
                quantity.requested, quantity.maximum
            ),
        )),
        (None, Some(notional)) => Some(Reject::new(
            policy,
            notional.scope,
            RejectCode::OrderNotionalExceedsLimit,
            "order notional exceeded",
            format!(
                "requested {}, max allowed: {}",
                notional.requested, notional.maximum
            ),
        )),
        (Some(quantity), Some(notional)) => {
            let scope = if quantity.scope == RejectScope::Account
                || notional.scope == RejectScope::Account
            {
                RejectScope::Account
            } else {
                RejectScope::Order
            };
            let details = match assets {
                Some((quantity_asset, notional_asset)) => format!(
                    "requested quantity {} for asset {quantity_asset}, \
                     max allowed: {}; requested notional {} for asset \
                     {notional_asset}, max allowed: {}",
                    quantity.requested, quantity.maximum, notional.requested, notional.maximum
                ),
                None => format!(
                    "requested quantity {}, max allowed: {}; \
                     requested notional {}, max allowed: {}",
                    quantity.requested, quantity.maximum, notional.requested, notional.maximum
                ),
            };
            Some(Reject::new(
                policy,
                scope,
                RejectCode::OrderExceedsLimit,
                "order size exceeded",
                details,
            ))
        }
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
    use crate::storage::{ConfigCell, LocalConfigCell, NoLocking};
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
        order_for_instrument("AAPL", settlement, quantity, price, account_id)
    }

    fn order_for_instrument(
        underlying: &str,
        settlement: &str,
        quantity: &str,
        price: &str,
        account_id: AccountId,
    ) -> TestOrder {
        OrderOperation {
            instrument: Instrument::new(
                Asset::new(underlying).expect("asset code must be valid"),
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
            max_quantity: Some(
                Quantity::from_str(max_quantity).expect("max quantity literal must be valid"),
            ),
            max_notional: Some(
                Volume::from_str(max_notional).expect("max notional literal must be valid"),
            ),
        }
    }

    fn quantity_limit(max_quantity: &str) -> OrderSizeLimit {
        OrderSizeLimit {
            max_quantity: Some(
                Quantity::from_str(max_quantity).expect("max quantity literal must be valid"),
            ),
            max_notional: None,
        }
    }

    fn notional_limit(max_notional: &str) -> OrderSizeLimit {
        OrderSizeLimit {
            max_quantity: None,
            max_notional: Some(
                Volume::from_str(max_notional).expect("max notional literal must be valid"),
            ),
        }
    }

    fn empty_limit() -> OrderSizeLimit {
        OrderSizeLimit {
            max_quantity: None,
            max_notional: None,
        }
    }

    fn asset_barrier(asset: &str, max_quantity: &str, max_notional: &str) -> OrderSizeAssetBarrier {
        OrderSizeAssetBarrier {
            limit: limit(max_quantity, max_notional),
            asset: Asset::new(asset).expect("asset code must be valid"),
        }
    }

    fn asset_quantity_barrier(asset: &str, max_quantity: &str) -> OrderSizeAssetBarrier {
        OrderSizeAssetBarrier {
            limit: quantity_limit(max_quantity),
            asset: Asset::new(asset).expect("asset code must be valid"),
        }
    }

    fn asset_notional_barrier(asset: &str, max_notional: &str) -> OrderSizeAssetBarrier {
        OrderSizeAssetBarrier {
            limit: notional_limit(max_notional),
            asset: Asset::new(asset).expect("asset code must be valid"),
        }
    }

    fn broker_barrier(max_quantity: &str, max_notional: &str) -> OrderSizeBrokerBarrier {
        OrderSizeBrokerBarrier {
            limit: limit(max_quantity, max_notional),
        }
    }

    fn broker_quantity_barrier(max_quantity: &str) -> OrderSizeBrokerBarrier {
        OrderSizeBrokerBarrier {
            limit: quantity_limit(max_quantity),
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
    fn empty_limit_rejected_by_settings_constructor() {
        let err = OrderSizeLimitSettings::new(
            None,
            [
                OrderSizeAssetBarrier {
                    limit: empty_limit(),
                    asset: Asset::new("AAPL").expect("asset code must be valid"),
                },
                asset_quantity_barrier("AAPL", "10"),
            ],
            [],
        )
        .expect_err("an empty duplicate must not be discarded");
        assert_eq!(err, OrderSizeLimitPolicyError::NoCapsConfigured);
        assert_eq!(
            err.to_string(),
            "at least one of max_quantity or max_notional \
             must be configured"
        );
    }

    #[test]
    fn duplicate_asset_key_rejected_by_settings_constructor() {
        let asset = Asset::new("AAPL").expect("asset code must be valid");
        let err = OrderSizeLimitSettings::new(
            None,
            [
                OrderSizeAssetBarrier {
                    limit: quantity_limit("10"),
                    asset: asset.clone(),
                },
                OrderSizeAssetBarrier {
                    limit: notional_limit("500000"),
                    asset: asset.clone(),
                },
            ],
            [],
        )
        .expect_err("duplicate asset key must fail");
        assert_eq!(
            err,
            OrderSizeLimitPolicyError::DuplicateAssetBarrier {
                asset: asset.clone()
            }
        );
        assert_eq!(err.to_string(), "duplicate asset barrier for asset AAPL");
    }

    #[test]
    fn duplicate_asset_key_rejected_by_runtime_setter() {
        let mut s = settings(Some(broker_quantity_barrier("100")), [], []);
        let original = s.clone();
        let asset = Asset::new("AAPL").expect("asset code must be valid");
        let err = s
            .set_asset_barriers([
                OrderSizeAssetBarrier {
                    limit: quantity_limit("10"),
                    asset: asset.clone(),
                },
                OrderSizeAssetBarrier {
                    limit: notional_limit("500000"),
                    asset: asset.clone(),
                },
            ])
            .expect_err("duplicate asset key must fail");
        assert_eq!(
            err,
            OrderSizeLimitPolicyError::DuplicateAssetBarrier { asset }
        );
        assert_eq!(s, original);
    }

    #[test]
    fn duplicate_account_asset_key_rejected_by_settings_constructor() {
        let account_id = AccountId::from_u64(99224416);
        let asset = Asset::new("AAPL").expect("asset code must be valid");
        let err = OrderSizeLimitSettings::new(
            None,
            [],
            [
                OrderSizeAccountAssetBarrier {
                    limit: quantity_limit("10"),
                    account_id,
                    asset: asset.clone(),
                },
                OrderSizeAccountAssetBarrier {
                    limit: notional_limit("500000"),
                    account_id,
                    asset: asset.clone(),
                },
            ],
        )
        .expect_err("duplicate account+asset key must fail");
        assert_eq!(
            err,
            OrderSizeLimitPolicyError::DuplicateAccountAssetBarrier {
                asset: asset.clone()
            }
        );
        assert_eq!(
            err.to_string(),
            "duplicate account+asset barrier for asset AAPL"
        );
    }

    #[test]
    fn duplicate_account_asset_key_rejected_by_runtime_setter() {
        let mut s = settings(Some(broker_quantity_barrier("100")), [], []);
        let original = s.clone();
        let account_id = AccountId::from_u64(99224416);
        let asset = Asset::new("AAPL").expect("asset code must be valid");
        let err = s
            .set_account_asset_barriers([
                OrderSizeAccountAssetBarrier {
                    limit: quantity_limit("10"),
                    account_id,
                    asset: asset.clone(),
                },
                OrderSizeAccountAssetBarrier {
                    limit: notional_limit("500000"),
                    account_id,
                    asset: asset.clone(),
                },
            ])
            .expect_err("duplicate account+asset key must fail");
        assert_eq!(
            err,
            OrderSizeLimitPolicyError::DuplicateAccountAssetBarrier { asset }
        );
        assert_eq!(s, original);
    }

    #[test]
    fn distinct_account_asset_keys_are_accepted() {
        let first_account = AccountId::from_u64(1);
        let second_account = AccountId::from_u64(2);
        let aapl = Asset::new("AAPL").expect("asset code must be valid");
        let usd = Asset::new("USD").expect("asset code must be valid");
        let s = OrderSizeLimitSettings::new(
            None,
            [],
            [
                OrderSizeAccountAssetBarrier {
                    limit: quantity_limit("10"),
                    account_id: first_account,
                    asset: aapl.clone(),
                },
                OrderSizeAccountAssetBarrier {
                    limit: notional_limit("500000"),
                    account_id: second_account,
                    asset: aapl,
                },
                OrderSizeAccountAssetBarrier {
                    limit: quantity_limit("20"),
                    account_id: first_account,
                    asset: usd,
                },
            ],
        )
        .expect(
            "same asset under another account and another asset under the same account are valid",
        );
        assert_eq!(s.account_asset_limits.len(), 3);
    }

    #[test]
    fn empty_limit_rejected_by_runtime_setters() {
        let mut s = settings(None, [asset_quantity_barrier("AAPL", "10")], []);
        let empty_asset = || OrderSizeAssetBarrier {
            limit: empty_limit(),
            asset: Asset::new("AAPL").expect("asset code must be valid"),
        };
        let empty_account_asset = || OrderSizeAccountAssetBarrier {
            limit: empty_limit(),
            account_id: AccountId::from_u64(99224416),
            asset: Asset::new("AAPL").expect("asset code must be valid"),
        };

        let err = s
            .set_asset_barriers([empty_asset()])
            .expect_err("asset limit without caps must fail");
        assert_eq!(err, OrderSizeLimitPolicyError::NoCapsConfigured);

        let err = s
            .set_account_asset_barriers([empty_account_asset()])
            .expect_err("account+asset limit without caps must fail");
        assert_eq!(err, OrderSizeLimitPolicyError::NoCapsConfigured);

        let err = s
            .set_broker(Some(OrderSizeBrokerBarrier {
                limit: empty_limit(),
            }))
            .expect_err("broker limit without caps must fail");
        assert_eq!(err, OrderSizeLimitPolicyError::NoCapsConfigured);
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
        let p = policy(None, [asset_quantity_barrier("AAPL", "10")], []);

        assert!(check(&p, &order("USD", "10", "1000000")).is_ok());

        let reject = check(&p, &order("USD", "11", "90")).expect_err("quantity must be rejected");
        let reject = &reject[0];
        assert_eq!(reject.scope, RejectScope::Order);
        assert_eq!(reject.code, RejectCode::OrderQtyExceedsLimit);
        assert_eq!(reject.reason, "order quantity exceeded");
        assert_eq!(reject.details, "requested 11, max allowed: 10");

        let other_instrument =
            order_for_instrument("MSFT", "USD", "11", "90", AccountId::from_u64(99224416));
        assert!(check(&p, &other_instrument).is_ok());
    }

    #[test]
    fn notional_violation_returns_order_notional_exceeded() {
        let p = policy(None, [asset_notional_barrier("USD", "1000")], []);

        assert!(check(&p, &order("USD", "1000", "1")).is_ok());

        let reject = check(&p, &order("USD", "10", "101")).expect_err("notional must be rejected");
        let reject = &reject[0];
        assert_eq!(reject.scope, RejectScope::Order);
        assert_eq!(reject.code, RejectCode::OrderNotionalExceedsLimit);
        assert_eq!(reject.reason, "order notional exceeded");
        assert_eq!(reject.details, "requested 1010, max allowed: 1000");

        let other_instrument =
            order_for_instrument("MSFT", "USD", "10", "101", AccountId::from_u64(99224416));
        let reject = check(&p, &other_instrument)
            .expect_err("USD notional limit must apply to another instrument");
        assert_eq!(reject[0].code, RejectCode::OrderNotionalExceedsLimit);
    }

    #[test]
    fn both_violations_are_returned_in_single_reject() {
        let p = policy(
            None,
            [
                asset_quantity_barrier("AAPL", "10"),
                asset_notional_barrier("USD", "1000"),
            ],
            [],
        );

        let reject = check(&p, &order("USD", "11", "100"))
            .expect_err("quantity and notional must be rejected");
        let reject = &reject[0];
        assert_eq!(reject.scope, RejectScope::Order);
        assert_eq!(reject.code, RejectCode::OrderExceedsLimit);
        assert_eq!(reject.reason, "order size exceeded");
        assert_eq!(
            reject.details,
            "requested quantity 11 for asset AAPL, max allowed: 10; \
             requested notional 1100 for asset USD, max allowed: 1000"
        );
    }

    #[test]
    fn no_applicable_limit_passes_silently() {
        let p = policy(None, [asset_barrier("EUR", "10", "1000")], []);
        assert!(check(&p, &order("USD", "1", "1")).is_ok());
    }

    #[test]
    fn boundary_values_are_accepted() {
        let p = policy(
            None,
            [
                asset_quantity_barrier("AAPL", "10"),
                asset_notional_barrier("USD", "1000"),
            ],
            [],
        );
        assert!(check(&p, &order("USD", "10", "100")).is_ok());
    }

    #[test]
    fn irrelevant_metric_does_not_require_price_conversion() {
        let quantity_policy = policy(None, [asset_quantity_barrier("AAPL", "10")], []);
        let quantity_order = OrderOperation {
            instrument: Instrument::new(
                Asset::new("AAPL").expect("asset code must be valid"),
                Asset::new("USD").expect("asset code must be valid"),
            ),
            account_id: AccountId::from_u64(99224416),
            side: Side::Buy,
            trade_amount: TradeAmount::Quantity(
                Quantity::from_str("10").expect("quantity literal must be valid"),
            ),
            price: None,
        };
        assert!(check(&quantity_policy, &quantity_order).is_ok());

        let notional_policy = policy(None, [asset_notional_barrier("USD", "100")], []);
        let notional_order = OrderOperation {
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
        assert!(check(&notional_policy, &notional_order).is_ok());
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

    #[test]
    fn broker_combined_reject_details_remain_asset_agnostic() {
        let p = policy(Some(broker_barrier("5", "500")), [], []);

        let reject =
            check(&p, &order("USD", "6", "100")).expect_err("both broker caps must reject");
        assert_eq!(reject[0].code, RejectCode::OrderExceedsLimit);
        assert_eq!(
            reject[0].details,
            "requested quantity 6, max allowed: 5; \
             requested notional 600, max allowed: 500"
        );
    }

    // ── account+asset override semantics ───────────────────────────────────

    #[test]
    fn account_asset_barrier_overrides_asset_barrier() {
        // Asset barrier allows 10, account+asset barrier allows 5.
        // The matching account should use the AAPL account+asset quantity limit.
        let p = policy(
            None,
            [asset_quantity_barrier("AAPL", "10")],
            [OrderSizeAccountAssetBarrier {
                limit: quantity_limit("5"),
                account_id: AccountId::from_u64(99224416),
                asset: Asset::new("AAPL").unwrap(),
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
            [asset_quantity_barrier("AAPL", "5")],
            [OrderSizeAccountAssetBarrier {
                limit: quantity_limit("100"),
                account_id: AccountId::from_u64(99224416),
                asset: Asset::new("AAPL").unwrap(),
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
    fn account_barrier_without_quantity_does_not_mask_asset_quantity() {
        let account_id = AccountId::from_u64(99224416);
        let p = policy(
            None,
            [asset_quantity_barrier("AAPL", "10")],
            [OrderSizeAccountAssetBarrier {
                limit: notional_limit("500"),
                account_id,
                asset: Asset::new("AAPL").expect("asset code must be valid"),
            }],
        );

        let reject = check(&p, &order_for_account("USD", "11", "1", account_id))
            .expect_err("asset quantity cap must survive the account-level gap");
        assert_eq!(reject[0].scope, RejectScope::Order);
        assert_eq!(reject[0].code, RejectCode::OrderQtyExceedsLimit);
        assert_eq!(reject[0].details, "requested 11, max allowed: 10");
    }

    #[test]
    fn account_barrier_without_notional_does_not_mask_asset_notional() {
        let account_id = AccountId::from_u64(99224416);
        let p = policy(
            None,
            [asset_notional_barrier("USD", "500")],
            [OrderSizeAccountAssetBarrier {
                limit: quantity_limit("10"),
                account_id,
                asset: Asset::new("USD").expect("asset code must be valid"),
            }],
        );

        let reject = check(&p, &order_for_account("USD", "6", "100", account_id))
            .expect_err("asset notional cap must survive the account-level gap");
        assert_eq!(reject[0].scope, RejectScope::Order);
        assert_eq!(reject[0].code, RejectCode::OrderNotionalExceedsLimit);
        assert_eq!(reject[0].details, "requested 600, max allowed: 500");
    }

    #[test]
    fn account_overrides_resolve_independently_for_both_metrics() {
        let account_id = AccountId::from_u64(99224416);
        let p = policy(
            None,
            [
                asset_quantity_barrier("AAPL", "10"),
                asset_notional_barrier("USD", "1000"),
            ],
            [
                OrderSizeAccountAssetBarrier {
                    limit: quantity_limit("5"),
                    account_id,
                    asset: Asset::new("AAPL").expect("asset code must be valid"),
                },
                OrderSizeAccountAssetBarrier {
                    limit: notional_limit("500"),
                    account_id,
                    asset: Asset::new("USD").expect("asset code must be valid"),
                },
            ],
        );

        let reject = check(&p, &order_for_account("USD", "6", "100", account_id))
            .expect_err("both account-specific limits must reject");
        assert_eq!(reject[0].scope, RejectScope::Account);
        assert_eq!(reject[0].code, RejectCode::OrderExceedsLimit);
        assert_eq!(
            reject[0].details,
            "requested quantity 6 for asset AAPL, max allowed: 5; \
             requested notional 600 for asset USD, max allowed: 500"
        );

        assert!(check(
            &p,
            &order_for_account("USD", "6", "100", AccountId::from_u64(2))
        )
        .is_ok());
    }

    #[test]
    fn combined_reject_keeps_account_scope_from_either_metric() {
        let account_id = AccountId::from_u64(99224416);
        let p = policy(
            None,
            [asset_quantity_barrier("AAPL", "5")],
            [OrderSizeAccountAssetBarrier {
                limit: notional_limit("500"),
                account_id,
                asset: Asset::new("USD").expect("asset code must be valid"),
            }],
        );

        let reject = check(&p, &order_for_account("USD", "6", "100", account_id))
            .expect_err("both asset-chain metrics must reject");
        assert_eq!(reject[0].scope, RejectScope::Account);
        assert_eq!(reject[0].code, RejectCode::OrderExceedsLimit);
    }

    #[test]
    fn same_asset_barrier_supplies_both_caps_and_scope() {
        let account_id = AccountId::from_u64(99224416);
        let p = policy(
            None,
            [],
            [OrderSizeAccountAssetBarrier {
                limit: limit("5", "500"),
                account_id,
                asset: Asset::new("AAPL").expect("asset code must be valid"),
            }],
        );

        let order = order_for_instrument("AAPL", "AAPL", "6", "100", account_id);
        let reject = check(&p, &order).expect_err("both caps from one entry must reject");
        assert_eq!(reject[0].scope, RejectScope::Account);
        assert_eq!(reject[0].code, RejectCode::OrderExceedsLimit);
        assert_eq!(
            reject[0].details,
            "requested quantity 6 for asset AAPL, max allowed: 5; \
             requested notional 600 for asset AAPL, max allowed: 500"
        );
    }

    #[test]
    fn unknown_settlement_passes_when_no_broker_or_account_asset_match() {
        // Unknown settlement JPY: the asset barrier covers EUR and the
        // account+asset barrier covers (account, USD), so neither asset-axis
        // lookup matches. The broker barrier applies to every order but its
        // high limit is not breached, so the order still passes.
        let p = policy(
            Some(broker_barrier("1000", "1000000")),
            [asset_barrier("EUR", "10", "1000")],
            [OrderSizeAccountAssetBarrier {
                limit: limit("5", "500"),
                account_id: AccountId::from_u64(99224416),
                asset: Asset::new("USD").expect("asset code must be valid"),
            }],
        );

        assert!(check(&p, &order("JPY", "1", "1")).is_ok());
    }

    #[test]
    fn axis_reject_reported_before_broker_reject_when_both_breach() {
        // Broker limit: max_qty=5. Asset limit: max_qty=3. Order: qty=6.
        // Both breach, but asset axis is reported first.
        let p = policy(
            Some(broker_quantity_barrier("5")),
            [asset_quantity_barrier("AAPL", "3")],
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
                asset_notional_barrier("USD", "1000"),
                asset_notional_barrier("EUR", "500"),
                asset_notional_barrier("GBP", "300"),
            ],
            [],
        );

        assert!(check(&p, &order("EUR", "5", "100")).is_ok());
        assert!(check(&p, &order("GBP", "3", "100")).is_ok());

        let reject = check(&p, &order("EUR", "6", "100"))
            .expect_err("exceeding EUR notional limit must reject");
        assert_eq!(reject[0].code, RejectCode::OrderNotionalExceedsLimit);
        assert_eq!(reject[0].details, "requested 600, max allowed: 500");
    }

    // ── policy name and apply ──────────────────────────────────────────────

    #[test]
    fn policy_name_is_stable() {
        let p = policy(None, [asset_barrier("AAPL", "10", "1000")], []);
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

        let p = policy(None, [asset_quantity_barrier("AAPL", "10")], []);
        let cell = p.settings_cell();

        // Update through the policy's own cell (via update on the clone).
        cell.update::<OrderSizeLimitPolicyError>(|s| {
            s.set_asset_barriers([asset_quantity_barrier("AAPL", "20")])
        })
        .expect("update must succeed");

        // The running policy observes the new limit through its own field.
        // qty=15 < new limit 20 → passes.
        assert!(check(&p, &order("USD", "15", "100")).is_ok());
        // qty=21 > new limit 20 → qty-only reject.
        let reject = check(&p, &order("USD", "21", "100")).expect_err("21 exceeds new limit of 20");
        assert_eq!(reject[0].code, RejectCode::OrderQtyExceedsLimit);
        assert!(reject[0].details.contains("max allowed: 20"));
    }

    #[test]
    fn price_accessor_can_reconfigure_policy_without_borrow_panic() {
        use crate::pretrade::ConfigurablePolicy;

        struct ReconfiguringPriceOrder {
            settings: LocalConfigCell<OrderSizeLimitSettings>,
            instrument: Instrument,
        }

        impl HasInstrument for ReconfiguringPriceOrder {
            fn instrument(&self) -> Result<&Instrument, RequestFieldAccessError> {
                Ok(&self.instrument)
            }
        }

        impl HasAccountId for ReconfiguringPriceOrder {
            fn account_id(&self) -> Result<AccountId, RequestFieldAccessError> {
                Ok(AccountId::from_u64(99224416))
            }
        }

        impl HasTradeAmount for ReconfiguringPriceOrder {
            fn trade_amount(&self) -> Result<TradeAmount, RequestFieldAccessError> {
                Ok(TradeAmount::Quantity(
                    Quantity::from_str("11").expect("quantity literal must be valid"),
                ))
            }
        }

        impl HasOrderPrice for ReconfiguringPriceOrder {
            fn price(&self) -> Result<Option<Price>, RequestFieldAccessError> {
                self.settings
                    .update::<OrderSizeLimitPolicyError>(|settings| {
                        settings.set_asset_barriers([asset_notional_barrier("USD", "2000")])
                    })
                    .expect("price accessor must reconfigure without a borrow panic");
                Ok(Some(
                    Price::from_str("100").expect("price literal must be valid"),
                ))
            }
        }

        let p = policy(None, [asset_notional_barrier("USD", "1000")], []);
        let order = ReconfiguringPriceOrder {
            settings: p.settings_cell(),
            instrument: Instrument::new(
                Asset::new("AAPL").expect("asset code must be valid"),
                Asset::new("USD").expect("asset code must be valid"),
            ),
        };

        let reject = <TestPolicy as PreTradePolicy<
            ReconfiguringPriceOrder,
            (),
            (),
            crate::core::LocalSync,
        >>::check_pre_trade_start(
            &p, &PreTradeContext::<NoLocking>::new(None), &order
        )
        .expect_err("the copied pre-update cap must reject");
        assert_eq!(reject[0].code, RejectCode::OrderNotionalExceedsLimit);
        assert_eq!(reject[0].details, "requested 1100, max allowed: 1000");

        assert!(check(
            &p,
            &order_for_account("USD", "11", "100", AccountId::from_u64(99224416),)
        )
        .is_ok());
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
        let p = policy(None, [asset_quantity_barrier("AAPL", "100")], []);
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
    fn resolve_quantity_covers_zero_price_and_missing_price_paths() {
        let zero_quantity = super::resolve_quantity(
            OrderSizeLimitPolicy::<NoLocking>::NAME,
            TradeAmount::Volume(Volume::from_str("10").expect("volume literal must be valid")),
            Some(Price::from_str("0").expect("zero price literal must be valid")),
        )
        .expect("volume-to-quantity conversion with zero price must pass");
        assert_eq!(zero_quantity, Quantity::ZERO);

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
        let p = policy(None, [asset_notional_barrier("USD", "1000")], []);

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

        let p = policy(None, [asset_notional_barrier("USD", "1000")], []);
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
