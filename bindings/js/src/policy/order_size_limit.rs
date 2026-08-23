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

//! Builtin order-size-limit policy builder and its barrier data types.
//!
//! `buildOrderSizeLimit()` returns a configuring builder. Calling any barrier
//! setter (`brokerBarrier`, `assetBarriers`, `accountAssetBarriers`) produces a
//! ready-builder token that can be passed to `builder.builtin(token)`.
//! `withPolicyGroupId` preserves the current stage. Each `OrderSizeLimit` cap
//! is optional: asset-keyed quantity resolves by underlying asset and notional
//! by settlement asset. A metric skips a matching barrier without its cap. An
//! absent cap constrains nothing, while zero rejects positive metric values and
//! admits a value of exactly zero.

use openpit::param::{Asset, Quantity, Volume};
use openpit::pretrade::policies::{
    OrderSizeAccountAssetBarrier, OrderSizeAssetBarrier, OrderSizeBrokerBarrier, OrderSizeLimit,
    OrderSizeLimitPolicy, OrderSizeLimitSettings,
};
use openpit::pretrade::PolicyGroupId;
use wasm_bindgen::prelude::*;

use crate::domain::{
    collect_cloned_wrappers, extract_cloned_wrapper, is_plain_object, parse_asset, read_field,
    resolve_account_id, resolve_optional_quantity, resolve_optional_volume, AccountIdLike,
    IntegerNumber, OptionalQuantityLike, OptionalVolumeLike,
};
use crate::error::{engine_build_configuration_error, make_error, ErrorKind};
use crate::param::ids::JsAccountId;

#[wasm_bindgen(typescript_custom_section)]
const ORDER_SIZE_INIT_TS: &'static str = r#"
/**
 * Plain-object form of {@link OrderSizeLimit}.
 *
 * `maxQuantity` is keyed by underlying asset and `maxNotional` by settlement
 * asset. Either cap may be absent and then constrains nothing. A resolution
 * chain skips a matching barrier that omits its metric. Zero is a present cap
 * and rejects positive metric values while admitting a value of exactly zero.
 */
export interface OrderSizeLimitInit {
  maxQuantity?: Quantity | string | number | bigint | null;
  maxNotional?: Volume | string | number | bigint | null;
}
"#;

#[wasm_bindgen]
extern "C" {
    /// An `OrderSizeLimit` wrapper or its plain-object form.
    #[wasm_bindgen(typescript_type = "OrderSizeLimit | OrderSizeLimitInit")]
    pub type OrderSizeLimitLike;
}

/// Independent quantity and notional caps for one order.
///
/// On asset-keyed barriers, quantity resolves by the instrument's underlying
/// asset and notional by its settlement asset. A cap may be absent and then
/// constrains nothing; its chain skips a matching barrier without that cap. A
/// cap set to zero rejects positive metric values and admits a value of exactly
/// zero.
#[wasm_bindgen(js_name = OrderSizeLimit)]
#[derive(Clone, Copy)]
pub struct JsOrderSizeLimit {
    max_quantity: Option<Quantity>,
    max_notional: Option<Volume>,
}

#[wasm_bindgen(js_class = OrderSizeLimit)]
impl JsOrderSizeLimit {
    /// Constructs an order-size limit.
    ///
    /// Each cap accepts its value object, a `DecimalInput`, or
    /// `null`/`undefined`. The core rejects a limit with neither cap when the
    /// containing policy is built.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` on an invalid value.
    #[wasm_bindgen(constructor)]
    pub fn new(
        max_quantity: OptionalQuantityLike,
        max_notional: OptionalVolumeLike,
    ) -> Result<JsOrderSizeLimit, JsValue> {
        Ok(Self {
            max_quantity: resolve_optional_quantity(max_quantity.into())?,
            max_notional: resolve_optional_volume(max_notional.into())?,
        })
    }

    /// The maximum order quantity, or `undefined` when quantity is
    /// unconstrained.
    #[wasm_bindgen(getter, js_name = maxQuantity)]
    pub fn max_quantity(&self) -> Option<crate::param::value_types::JsQuantity> {
        self.max_quantity
            .map(crate::param::value_types::JsQuantity::from_inner)
    }

    /// The maximum notional, or `undefined` when notional is unconstrained.
    #[wasm_bindgen(getter, js_name = maxNotional)]
    pub fn max_notional(&self) -> Option<crate::param::value_types::JsVolume> {
        self.max_notional
            .map(crate::param::value_types::JsVolume::from_inner)
    }

    /// Returns a fresh copy of this order-size limit.
    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsOrderSizeLimit {
        *self
    }
}

impl JsOrderSizeLimit {
    /// Returns the core order-size limit.
    fn to_core(self) -> OrderSizeLimit {
        OrderSizeLimit {
            max_quantity: self.max_quantity,
            max_notional: self.max_notional,
        }
    }

    /// Resolves an `OrderSizeLimit | OrderSizeLimitInit` argument.
    ///
    /// A wrapper instance is copied; a plain object may omit either field.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` on an invalid present field, or when the value is
    /// neither an `OrderSizeLimit` nor a plain object. The core validates that
    /// at least one cap is present when the policy is built.
    fn coerce(value: JsValue) -> Result<JsOrderSizeLimit, JsValue> {
        if let Some(wrapped) = extract_cloned_wrapper::<JsOrderSizeLimit>(&value)? {
            return Ok(wrapped);
        }
        if is_plain_object(&value) {
            return Ok(Self {
                max_quantity: resolve_optional_quantity(read_field(&value, "maxQuantity")?)?,
                max_notional: resolve_optional_volume(read_field(&value, "maxNotional")?)?,
            });
        }
        Err(make_error(
            ErrorKind::Type,
            "limit must be an OrderSizeLimit or { maxQuantity, maxNotional }",
            None,
        ))
    }
}

/// Broker-wide order-size barrier.
///
/// Each present cap applies to every order. An absent cap constrains nothing;
/// a zero cap rejects positive metric values and admits a value of exactly zero.
#[wasm_bindgen(js_name = OrderSizeBrokerBarrier)]
#[derive(Clone, Copy)]
pub struct JsOrderSizeBrokerBarrier {
    limit: JsOrderSizeLimit,
}

#[wasm_bindgen(js_class = OrderSizeBrokerBarrier)]
impl JsOrderSizeBrokerBarrier {
    /// Constructs a broker barrier from its limit.
    ///
    /// `limit` accepts an `OrderSizeLimit` object or a plain
    /// `OrderSizeLimitInit` literal.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` on an invalid limit.
    #[wasm_bindgen(constructor)]
    pub fn new(limit: OrderSizeLimitLike) -> Result<JsOrderSizeBrokerBarrier, JsValue> {
        Ok(Self {
            limit: JsOrderSizeLimit::coerce(limit.into())?,
        })
    }

    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsOrderSizeBrokerBarrier {
        *self
    }
}

impl JsOrderSizeBrokerBarrier {
    pub(crate) fn to_core(self) -> OrderSizeBrokerBarrier {
        OrderSizeBrokerBarrier {
            limit: self.limit.to_core(),
        }
    }
}

/// Per-asset order-size barrier.
///
/// Its asset keys quantity by underlying asset and notional by settlement
/// asset. Each metric skips the barrier and continues its chain when its cap is
/// absent. Zero remains a present cap: it rejects positive metric values and
/// admits a value of exactly zero.
#[wasm_bindgen(js_name = OrderSizeAssetBarrier)]
#[derive(Clone)]
pub struct JsOrderSizeAssetBarrier {
    limit: JsOrderSizeLimit,
    asset: Asset,
}

#[wasm_bindgen(js_class = OrderSizeAssetBarrier)]
impl JsOrderSizeAssetBarrier {
    /// Constructs a barrier whose asset keys quantity by underlying and
    /// notional by settlement.
    ///
    /// # Errors
    ///
    /// Throws `AssetError` when `asset` is empty.
    #[wasm_bindgen(constructor)]
    pub fn new(limit: OrderSizeLimitLike, asset: &str) -> Result<JsOrderSizeAssetBarrier, JsValue> {
        Ok(Self {
            limit: JsOrderSizeLimit::coerce(limit.into())?,
            asset: parse_asset(asset)?,
        })
    }

    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsOrderSizeAssetBarrier {
        self.clone()
    }
}

impl JsOrderSizeAssetBarrier {
    pub(crate) fn to_core(&self) -> OrderSizeAssetBarrier {
        OrderSizeAssetBarrier {
            limit: self.limit.to_core(),
            asset: self.asset.clone(),
        }
    }
}

/// Per-(account, asset) order-size barrier.
///
/// For this account, its asset keys quantity by underlying asset and notional
/// by settlement asset. A metric whose cap is absent continues to the matching
/// asset barrier. Zero remains a present cap: it rejects positive metric values
/// and admits a value of exactly zero.
#[wasm_bindgen(js_name = OrderSizeAccountAssetBarrier)]
#[derive(Clone)]
pub struct JsOrderSizeAccountAssetBarrier {
    limit: JsOrderSizeLimit,
    account_id: JsAccountId,
    asset: Asset,
}

#[wasm_bindgen(js_class = OrderSizeAccountAssetBarrier)]
impl JsOrderSizeAccountAssetBarrier {
    /// Constructs an account-asset barrier from its parts.
    ///
    /// # Errors
    ///
    /// Throws `AssetError` when `asset` is empty.
    #[wasm_bindgen(constructor)]
    pub fn new(
        limit: OrderSizeLimitLike,
        account_id: AccountIdLike,
        asset: &str,
    ) -> Result<JsOrderSizeAccountAssetBarrier, JsValue> {
        Ok(Self {
            limit: JsOrderSizeLimit::coerce(limit.into())?,
            account_id: JsAccountId::from_inner(resolve_account_id(account_id.into())?),
            asset: parse_asset(asset)?,
        })
    }

    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsOrderSizeAccountAssetBarrier {
        self.clone()
    }
}

impl JsOrderSizeAccountAssetBarrier {
    pub(crate) fn to_core(&self) -> OrderSizeAccountAssetBarrier {
        OrderSizeAccountAssetBarrier {
            limit: self.limit.to_core(),
            account_id: self.account_id.inner(),
            asset: self.asset.clone(),
        }
    }
}

/// Configuring builder for the builtin order-size-limit policy.
#[wasm_bindgen(js_name = OrderSizeLimitBuilder)]
#[derive(Clone, Default)]
pub struct JsOrderSizeLimitBuilder {
    policy_group_id: u16,
    broker: Option<JsOrderSizeBrokerBarrier>,
    asset_barriers: Vec<JsOrderSizeAssetBarrier>,
    account_asset_barriers: Vec<JsOrderSizeAccountAssetBarrier>,
}

#[wasm_bindgen(js_class = OrderSizeLimitBuilder)]
impl JsOrderSizeLimitBuilder {
    /// Stable name registered by the builtin order-size-limit policy.
    #[wasm_bindgen(getter, js_name = NAME)]
    pub fn name() -> String {
        OrderSizeLimitPolicy::<crate::engine::StorageFactory>::NAME.to_owned()
    }

    /// Sets the broker-wide barrier and returns the builder for chaining.
    #[wasm_bindgen(
        js_name = brokerBarrier,
        unchecked_return_type = "OrderSizeLimitReadyBuilder"
    )]
    pub fn broker_barrier(&self, barrier: &JsOrderSizeBrokerBarrier) -> JsOrderSizeLimitBuilder {
        let mut next = self.clone();
        next.broker = Some(*barrier);
        next
    }

    /// Adds per-asset barriers and returns the builder for chaining.
    ///
    /// `barriers` is an array of `OrderSizeAssetBarrier`.
    #[wasm_bindgen(
        js_name = assetBarriers,
        unchecked_return_type = "OrderSizeLimitReadyBuilder"
    )]
    pub fn asset_barriers(
        &self,
        #[wasm_bindgen(unchecked_param_type = "Iterable<OrderSizeAssetBarrier>")] barriers: JsValue,
    ) -> Result<JsOrderSizeLimitBuilder, JsValue> {
        let barriers: Vec<JsOrderSizeAssetBarrier> =
            collect_cloned_wrappers(&barriers, "assetBarriers")?;
        let mut next = self.clone();
        next.asset_barriers.extend(barriers);
        Ok(next)
    }

    /// Adds per-(account, asset) barriers and returns the builder for chaining.
    ///
    /// `barriers` is an array of `OrderSizeAccountAssetBarrier`.
    #[wasm_bindgen(
        js_name = accountAssetBarriers,
        unchecked_return_type = "OrderSizeLimitReadyBuilder"
    )]
    pub fn account_asset_barriers(
        &self,
        #[wasm_bindgen(unchecked_param_type = "Iterable<OrderSizeAccountAssetBarrier>")]
        barriers: JsValue,
    ) -> Result<JsOrderSizeLimitBuilder, JsValue> {
        let barriers: Vec<JsOrderSizeAccountAssetBarrier> =
            collect_cloned_wrappers(&barriers, "accountAssetBarriers")?;
        let mut next = self.clone();
        next.account_asset_barriers.extend(barriers);
        Ok(next)
    }

    /// Assigns the policy group id and returns the builder for chaining.
    #[wasm_bindgen(js_name = withPolicyGroupId)]
    pub fn with_policy_group_id(
        &self,
        policy_group_id: IntegerNumber,
    ) -> Result<JsOrderSizeLimitBuilder, JsValue> {
        let mut next = self.clone();
        next.policy_group_id = crate::lock::parse_policy_group_id(policy_group_id.into())?.value();
        Ok(next)
    }

    /// Returns an independent builder with the same configuration.
    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsOrderSizeLimitBuilder {
        self.clone()
    }
}

impl JsOrderSizeLimitBuilder {
    /// Builds the core policy from this token.
    ///
    /// # Errors
    ///
    /// Throws `EngineBuildError` when no barrier is configured.
    pub(crate) fn build_policy(
        &self,
    ) -> Result<OrderSizeLimitPolicy<crate::engine::StorageFactory>, JsValue> {
        let broker = self.broker.map(JsOrderSizeBrokerBarrier::to_core);
        let asset: Vec<OrderSizeAssetBarrier> = self
            .asset_barriers
            .iter()
            .map(JsOrderSizeAssetBarrier::to_core)
            .collect();
        let account_asset: Vec<OrderSizeAccountAssetBarrier> = self
            .account_asset_barriers
            .iter()
            .map(JsOrderSizeAccountAssetBarrier::to_core)
            .collect();

        let settings = OrderSizeLimitSettings::new(broker, asset, account_asset)
            .map_err(|error| engine_build_configuration_error(&error.to_string()))?;
        Ok(OrderSizeLimitPolicy::new(settings)
            .with_policy_group_id(PolicyGroupId::new(self.policy_group_id)))
    }
}

/// Creates a fresh order-size-limit configuring builder.
#[wasm_bindgen(js_name = buildOrderSizeLimit)]
pub fn build_order_size_limit() -> JsOrderSizeLimitBuilder {
    JsOrderSizeLimitBuilder::default()
}
