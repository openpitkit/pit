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
//! `withPolicyGroupId` preserves the current stage. `OrderSizeLimit` is the
//! shared `(maxQuantity, maxNotional)` value used by every barrier. Decimals
//! cross as `Quantity` / `Volume` value objects or `DecimalInput`.

use openpit::param::{Asset, Quantity, Volume};
use openpit::pretrade::policies::{
    OrderSizeAccountAssetBarrier, OrderSizeAssetBarrier, OrderSizeBrokerBarrier, OrderSizeLimit,
    OrderSizeLimitPolicy, OrderSizeLimitSettings,
};
use openpit::pretrade::PolicyGroupId;
use wasm_bindgen::prelude::*;

use crate::domain::{
    collect_cloned_wrappers, extract_cloned_wrapper, is_plain_object, parse_asset, read_field,
    resolve_account_id, resolve_quantity, resolve_volume, AccountIdLike, IntegerNumber,
    QuantityLike, VolumeLike,
};
use crate::error::{engine_build_configuration_error, make_error, ErrorKind};
use crate::param::ids::JsAccountId;

#[wasm_bindgen(typescript_custom_section)]
const ORDER_SIZE_INIT_TS: &'static str = r#"
/**
 * Plain-object form of {@link OrderSizeLimit}. Both fields are required.
 */
export interface OrderSizeLimitInit {
  maxQuantity: Quantity | string | number | bigint;
  maxNotional: Volume | string | number | bigint;
}
"#;

#[wasm_bindgen]
extern "C" {
    /// An `OrderSizeLimit` wrapper or its plain-object form.
    #[wasm_bindgen(typescript_type = "OrderSizeLimit | OrderSizeLimitInit")]
    pub type OrderSizeLimitLike;
}

/// Order-size limit: a maximum quantity and a maximum notional volume.
#[wasm_bindgen(js_name = OrderSizeLimit)]
#[derive(Clone, Copy)]
pub struct JsOrderSizeLimit {
    max_quantity: Quantity,
    max_notional: Volume,
}

#[wasm_bindgen(js_class = OrderSizeLimit)]
impl JsOrderSizeLimit {
    /// Constructs an order-size limit.
    ///
    /// Each argument accepts a value object (`Quantity` / `Volume`) or a
    /// `DecimalInput`.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` on an invalid value.
    #[wasm_bindgen(constructor)]
    pub fn new(
        max_quantity: QuantityLike,
        max_notional: VolumeLike,
    ) -> Result<JsOrderSizeLimit, JsValue> {
        Ok(Self {
            max_quantity: resolve_quantity(max_quantity.into())?,
            max_notional: resolve_volume(max_notional.into())?,
        })
    }

    /// The maximum order quantity.
    #[wasm_bindgen(getter, js_name = maxQuantity)]
    pub fn max_quantity(&self) -> crate::param::value_types::JsQuantity {
        crate::param::value_types::JsQuantity::from_inner(self.max_quantity)
    }

    /// The maximum order notional volume.
    #[wasm_bindgen(getter, js_name = maxNotional)]
    pub fn max_notional(&self) -> crate::param::value_types::JsVolume {
        crate::param::value_types::JsVolume::from_inner(self.max_notional)
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
    /// A wrapper instance is copied; a plain object literal
    /// `{ maxQuantity, maxNotional }` is assembled.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` on an invalid or missing field, or when the value is
    /// neither an `OrderSizeLimit` nor a plain object.
    fn coerce(value: JsValue) -> Result<JsOrderSizeLimit, JsValue> {
        if let Some(wrapped) = extract_cloned_wrapper::<JsOrderSizeLimit>(&value)? {
            return Ok(wrapped);
        }
        if is_plain_object(&value) {
            return Ok(Self {
                max_quantity: resolve_quantity(read_field(&value, "maxQuantity")?)?,
                max_notional: resolve_volume(read_field(&value, "maxNotional")?)?,
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

/// Per-settlement-asset order-size barrier.
#[wasm_bindgen(js_name = OrderSizeAssetBarrier)]
#[derive(Clone)]
pub struct JsOrderSizeAssetBarrier {
    limit: JsOrderSizeLimit,
    settlement_asset: Asset,
}

#[wasm_bindgen(js_class = OrderSizeAssetBarrier)]
impl JsOrderSizeAssetBarrier {
    /// Constructs an asset barrier from its limit and settlement asset.
    ///
    /// # Errors
    ///
    /// Throws `AssetError` when `settlementAsset` is empty.
    #[wasm_bindgen(constructor)]
    pub fn new(
        limit: OrderSizeLimitLike,
        settlement_asset: &str,
    ) -> Result<JsOrderSizeAssetBarrier, JsValue> {
        Ok(Self {
            limit: JsOrderSizeLimit::coerce(limit.into())?,
            settlement_asset: parse_asset(settlement_asset)?,
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
            settlement_asset: self.settlement_asset.clone(),
        }
    }
}

/// Per-(account, settlement-asset) order-size barrier.
#[wasm_bindgen(js_name = OrderSizeAccountAssetBarrier)]
#[derive(Clone)]
pub struct JsOrderSizeAccountAssetBarrier {
    limit: JsOrderSizeLimit,
    account_id: JsAccountId,
    settlement_asset: Asset,
}

#[wasm_bindgen(js_class = OrderSizeAccountAssetBarrier)]
impl JsOrderSizeAccountAssetBarrier {
    /// Constructs an account-asset barrier from its parts.
    ///
    /// # Errors
    ///
    /// Throws `AssetError` when `settlementAsset` is empty.
    #[wasm_bindgen(constructor)]
    pub fn new(
        limit: OrderSizeLimitLike,
        account_id: AccountIdLike,
        settlement_asset: &str,
    ) -> Result<JsOrderSizeAccountAssetBarrier, JsValue> {
        Ok(Self {
            limit: JsOrderSizeLimit::coerce(limit.into())?,
            account_id: JsAccountId::from_inner(resolve_account_id(account_id.into())?),
            settlement_asset: parse_asset(settlement_asset)?,
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
            settlement_asset: self.settlement_asset.clone(),
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
