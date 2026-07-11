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

//! Builtin rate-limit policy builder and its barrier data types.
//!
//! `buildRateLimit()` returns a configuring builder. Calling any barrier setter
//! (`brokerBarrier`, `assetBarriers`, `accountBarriers`,
//! `accountAssetBarriers`) produces a ready-builder token that can be passed to
//! `builder.builtin(token)`. `withPolicyGroupId` preserves the current stage.
//! The shared `RateLimit` carries `{ maxOrders, windowMs }`; the window crosses
//! as a `number` of milliseconds and is stored internally as a [`Duration`].

use std::time::Duration;

use openpit::param::Asset;
use openpit::pretrade::policies::{
    RateLimit, RateLimitAccountAssetBarrier, RateLimitAccountBarrier, RateLimitAssetBarrier,
    RateLimitBrokerBarrier, RateLimitPolicy, RateLimitSettings,
};
use openpit::pretrade::PolicyGroupId;
use wasm_bindgen::prelude::*;

use crate::domain::{
    collect_cloned_wrappers, extract_cloned_wrapper, is_plain_object, parse_asset,
    parse_bounded_number, resolve_account_id, AccountIdLike, IntegerNumber,
};
use crate::error::{engine_build_configuration_error, make_error, ErrorKind};
use crate::param::ids::JsAccountId;

#[wasm_bindgen(typescript_custom_section)]
const RATE_LIMIT_INIT_TS: &'static str = r#"
/**
 * Plain-object form of {@link RateLimit}. `windowMs` is the rolling-window
 * length in milliseconds.
 */
export interface RateLimitInit {
  maxOrders: number;
  windowMs: number;
}
"#;

#[wasm_bindgen]
extern "C" {
    /// A `RateLimit` wrapper or its plain-object form.
    #[wasm_bindgen(typescript_type = "RateLimit | RateLimitInit")]
    pub type RateLimitLike;
}

/// Rate-limit configuration: a maximum order count over a rolling window.
#[wasm_bindgen(js_name = RateLimit)]
#[derive(Clone, Copy)]
pub struct JsRateLimit {
    max_orders: usize,
    window: Duration,
}

#[wasm_bindgen(js_class = RateLimit)]
impl JsRateLimit {
    /// Constructs a rate limit from a max order count and a window.
    ///
    /// `windowMs` is the rolling-window length in milliseconds. Fractional
    /// milliseconds retain nanosecond precision.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` when `windowMs` cannot be represented as a Rust
    /// duration. Positive and maximum-window validation is performed by the
    /// core settings constructor when the policy is built.
    #[wasm_bindgen(constructor)]
    pub fn new(max_orders: IntegerNumber, window_ms: f64) -> Result<JsRateLimit, JsValue> {
        let max_orders =
            parse_bounded_number(max_orders.into(), usize::MAX as u64, "maxOrders")? as usize;
        Self::from_values(max_orders, window_ms)
    }

    /// The maximum number of orders accepted within the window.
    #[wasm_bindgen(getter, js_name = maxOrders)]
    pub fn max_orders(&self) -> usize {
        self.max_orders
    }

    /// The rolling-window length in milliseconds.
    #[wasm_bindgen(getter, js_name = windowMs)]
    pub fn window_ms(&self) -> f64 {
        self.window.as_secs_f64() * 1000.0
    }

    /// Returns a fresh copy of this rate limit.
    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsRateLimit {
        *self
    }
}

impl JsRateLimit {
    /// Builds a rate limit after the integer boundary has been validated.
    fn from_values(max_orders: usize, window_ms: f64) -> Result<Self, JsValue> {
        let window = Duration::try_from_secs_f64(window_ms / 1000.0).map_err(|_| {
            make_error(
                ErrorKind::Range,
                "windowMs must be finite, non-negative, and representable as a duration",
                None,
            )
        })?;
        Ok(Self { max_orders, window })
    }

    /// Returns the core rate limit.
    fn to_core(self) -> RateLimit {
        RateLimit {
            max_orders: self.max_orders,
            window: self.window,
        }
    }

    /// Resolves a `RateLimit | RateLimitInit` argument.
    ///
    /// A wrapper instance is copied; a plain object literal
    /// `{ maxOrders, windowMs }` is assembled via the constructor.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` on a missing/invalid field, or when the value is
    /// neither a `RateLimit` nor a plain object.
    fn coerce(value: JsValue) -> Result<JsRateLimit, JsValue> {
        if let Some(wrapped) = extract_cloned_wrapper::<JsRateLimit>(&value)? {
            return Ok(wrapped);
        }
        if is_plain_object(&value) {
            let max_orders = read_usize_field(&value, "maxOrders")?;
            let window_ms = read_f64_field(&value, "windowMs")?;
            return JsRateLimit::from_values(max_orders, window_ms);
        }
        Err(rate_limit_error())
    }
}

/// Builds the error raised for an invalid `RateLimit` argument.
fn rate_limit_error() -> JsValue {
    make_error(
        ErrorKind::Type,
        "limit must be a RateLimit or { maxOrders, windowMs }",
        None,
    )
}

/// Reads a required non-negative integer field as `usize`.
fn read_usize_field(value: &JsValue, field: &str) -> Result<usize, JsValue> {
    let raw = js_sys::Reflect::get(value, &JsValue::from_str(field))?;
    Ok(parse_bounded_number(raw, usize::MAX as u64, field)? as usize)
}

/// Reads a required numeric field as `f64`.
fn read_f64_field(value: &JsValue, field: &str) -> Result<f64, JsValue> {
    js_sys::Reflect::get(value, &JsValue::from_str(field))?
        .as_f64()
        .ok_or_else(|| make_error(ErrorKind::Type, &format!("{field} must be a number"), None))
}

/// Broker-wide rate-limit barrier.
#[wasm_bindgen(js_name = RateLimitBrokerBarrier)]
#[derive(Clone, Copy)]
pub struct JsRateLimitBrokerBarrier {
    limit: JsRateLimit,
}

#[wasm_bindgen(js_class = RateLimitBrokerBarrier)]
impl JsRateLimitBrokerBarrier {
    /// Constructs a broker barrier from its limit.
    ///
    /// `limit` accepts a `RateLimit` object or a plain `RateLimitInit` literal.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` on an invalid limit.
    #[wasm_bindgen(constructor)]
    pub fn new(limit: RateLimitLike) -> Result<JsRateLimitBrokerBarrier, JsValue> {
        Ok(Self {
            limit: JsRateLimit::coerce(limit.into())?,
        })
    }

    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsRateLimitBrokerBarrier {
        *self
    }
}

impl JsRateLimitBrokerBarrier {
    pub(crate) fn to_core(self) -> RateLimitBrokerBarrier {
        RateLimitBrokerBarrier {
            limit: self.limit.to_core(),
        }
    }
}

/// Per-settlement-asset rate-limit barrier.
#[wasm_bindgen(js_name = RateLimitAssetBarrier)]
#[derive(Clone)]
pub struct JsRateLimitAssetBarrier {
    limit: JsRateLimit,
    settlement_asset: Asset,
}

#[wasm_bindgen(js_class = RateLimitAssetBarrier)]
impl JsRateLimitAssetBarrier {
    /// Constructs an asset barrier from its limit and settlement asset.
    ///
    /// # Errors
    ///
    /// Throws `AssetError` when `settlementAsset` is empty.
    #[wasm_bindgen(constructor)]
    pub fn new(
        limit: RateLimitLike,
        settlement_asset: &str,
    ) -> Result<JsRateLimitAssetBarrier, JsValue> {
        Ok(Self {
            limit: JsRateLimit::coerce(limit.into())?,
            settlement_asset: parse_asset(settlement_asset)?,
        })
    }

    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsRateLimitAssetBarrier {
        self.clone()
    }
}

impl JsRateLimitAssetBarrier {
    pub(crate) fn to_core(&self) -> RateLimitAssetBarrier {
        RateLimitAssetBarrier {
            limit: self.limit.to_core(),
            settlement_asset: self.settlement_asset.clone(),
        }
    }
}

/// Per-account rate-limit barrier.
#[wasm_bindgen(js_name = RateLimitAccountBarrier)]
#[derive(Clone, Copy)]
pub struct JsRateLimitAccountBarrier {
    limit: JsRateLimit,
    account_id: JsAccountId,
}

#[wasm_bindgen(js_class = RateLimitAccountBarrier)]
impl JsRateLimitAccountBarrier {
    /// Constructs an account barrier from its limit and account id.
    ///
    /// `limit` accepts a `RateLimit` object or a plain `RateLimitInit` literal;
    /// `accountId` accepts an `AccountId` or a numeric/string identifier.
    ///
    /// # Errors
    ///
    /// Throws `ParamError`/`AccountIdError` on an invalid limit or identifier.
    #[wasm_bindgen(constructor)]
    pub fn new(
        limit: RateLimitLike,
        account_id: AccountIdLike,
    ) -> Result<JsRateLimitAccountBarrier, JsValue> {
        Ok(Self {
            limit: JsRateLimit::coerce(limit.into())?,
            account_id: JsAccountId::from_inner(resolve_account_id(account_id.into())?),
        })
    }

    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsRateLimitAccountBarrier {
        *self
    }
}

impl JsRateLimitAccountBarrier {
    pub(crate) fn to_core(self) -> RateLimitAccountBarrier {
        RateLimitAccountBarrier {
            limit: self.limit.to_core(),
            account_id: self.account_id.inner(),
        }
    }
}

/// Per-(account, settlement-asset) rate-limit barrier.
#[wasm_bindgen(js_name = RateLimitAccountAssetBarrier)]
#[derive(Clone)]
pub struct JsRateLimitAccountAssetBarrier {
    limit: JsRateLimit,
    account_id: JsAccountId,
    settlement_asset: Asset,
}

#[wasm_bindgen(js_class = RateLimitAccountAssetBarrier)]
impl JsRateLimitAccountAssetBarrier {
    /// Constructs an account-asset barrier from its parts.
    ///
    /// # Errors
    ///
    /// Throws `AssetError` when `settlementAsset` is empty.
    #[wasm_bindgen(constructor)]
    pub fn new(
        limit: RateLimitLike,
        account_id: AccountIdLike,
        settlement_asset: &str,
    ) -> Result<JsRateLimitAccountAssetBarrier, JsValue> {
        Ok(Self {
            limit: JsRateLimit::coerce(limit.into())?,
            account_id: JsAccountId::from_inner(resolve_account_id(account_id.into())?),
            settlement_asset: parse_asset(settlement_asset)?,
        })
    }

    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsRateLimitAccountAssetBarrier {
        self.clone()
    }
}

impl JsRateLimitAccountAssetBarrier {
    pub(crate) fn to_core(&self) -> RateLimitAccountAssetBarrier {
        RateLimitAccountAssetBarrier {
            limit: self.limit.to_core(),
            account_id: self.account_id.inner(),
            settlement_asset: self.settlement_asset.clone(),
        }
    }
}

/// Configuring builder for the builtin rate-limit policy.
#[wasm_bindgen(js_name = RateLimitBuilder)]
#[derive(Clone, Default)]
pub struct JsRateLimitBuilder {
    policy_group_id: u16,
    broker: Option<JsRateLimitBrokerBarrier>,
    asset_barriers: Vec<JsRateLimitAssetBarrier>,
    account_barriers: Vec<JsRateLimitAccountBarrier>,
    account_asset_barriers: Vec<JsRateLimitAccountAssetBarrier>,
}

#[wasm_bindgen(js_class = RateLimitBuilder)]
impl JsRateLimitBuilder {
    /// Stable name registered by the builtin rate-limit policy.
    #[wasm_bindgen(getter, js_name = NAME)]
    pub fn name() -> String {
        RateLimitPolicy::<crate::engine::StorageFactory>::NAME.to_owned()
    }

    /// Sets the broker-wide barrier and returns the builder for chaining.
    #[wasm_bindgen(
        js_name = brokerBarrier,
        unchecked_return_type = "RateLimitReadyBuilder"
    )]
    pub fn broker_barrier(&self, barrier: &JsRateLimitBrokerBarrier) -> JsRateLimitBuilder {
        let mut next = self.clone();
        next.broker = Some(*barrier);
        next
    }

    /// Adds per-asset barriers and returns the builder for chaining.
    ///
    /// `barriers` is an array of `RateLimitAssetBarrier`.
    #[wasm_bindgen(
        js_name = assetBarriers,
        unchecked_return_type = "RateLimitReadyBuilder"
    )]
    pub fn asset_barriers(
        &self,
        #[wasm_bindgen(unchecked_param_type = "Iterable<RateLimitAssetBarrier>")] barriers: JsValue,
    ) -> Result<JsRateLimitBuilder, JsValue> {
        let barriers: Vec<JsRateLimitAssetBarrier> =
            collect_cloned_wrappers(&barriers, "assetBarriers")?;
        let mut next = self.clone();
        next.asset_barriers.extend(barriers);
        Ok(next)
    }

    /// Adds per-account barriers and returns the builder for chaining.
    ///
    /// `barriers` is an array of `RateLimitAccountBarrier`.
    #[wasm_bindgen(
        js_name = accountBarriers,
        unchecked_return_type = "RateLimitReadyBuilder"
    )]
    pub fn account_barriers(
        &self,
        #[wasm_bindgen(unchecked_param_type = "Iterable<RateLimitAccountBarrier>")]
        barriers: JsValue,
    ) -> Result<JsRateLimitBuilder, JsValue> {
        let barriers: Vec<JsRateLimitAccountBarrier> =
            collect_cloned_wrappers(&barriers, "accountBarriers")?;
        let mut next = self.clone();
        next.account_barriers.extend(barriers);
        Ok(next)
    }

    /// Adds per-(account, asset) barriers and returns the builder for chaining.
    ///
    /// `barriers` is an array of `RateLimitAccountAssetBarrier`.
    #[wasm_bindgen(
        js_name = accountAssetBarriers,
        unchecked_return_type = "RateLimitReadyBuilder"
    )]
    pub fn account_asset_barriers(
        &self,
        #[wasm_bindgen(unchecked_param_type = "Iterable<RateLimitAccountAssetBarrier>")]
        barriers: JsValue,
    ) -> Result<JsRateLimitBuilder, JsValue> {
        let barriers: Vec<JsRateLimitAccountAssetBarrier> =
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
    ) -> Result<JsRateLimitBuilder, JsValue> {
        let mut next = self.clone();
        next.policy_group_id = crate::lock::parse_policy_group_id(policy_group_id.into())?.value();
        Ok(next)
    }

    /// Returns an independent builder with the same configuration.
    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsRateLimitBuilder {
        self.clone()
    }
}

impl JsRateLimitBuilder {
    /// Builds the core policy from this token and the engine's storage
    /// builder.
    ///
    /// # Errors
    ///
    /// Throws `EngineBuildError` when no barrier is configured or a limit is
    /// invalid.
    pub(crate) fn build_policy(
        &self,
        storage_builder: &crate::engine::StorageBuilderRef,
    ) -> Result<RateLimitPolicy<crate::engine::StorageFactory>, JsValue> {
        let broker = self.broker.map(|barrier| RateLimitBrokerBarrier {
            limit: barrier.limit.to_core(),
        });
        let asset: Vec<RateLimitAssetBarrier> = self
            .asset_barriers
            .iter()
            .map(JsRateLimitAssetBarrier::to_core)
            .collect();
        let account: Vec<RateLimitAccountBarrier> = self
            .account_barriers
            .iter()
            .copied()
            .map(JsRateLimitAccountBarrier::to_core)
            .collect();
        let account_asset: Vec<RateLimitAccountAssetBarrier> = self
            .account_asset_barriers
            .iter()
            .map(JsRateLimitAccountAssetBarrier::to_core)
            .collect();

        let settings = RateLimitSettings::new(broker, asset, account, account_asset)
            .map_err(|error| engine_build_configuration_error(&error.to_string()))?;
        Ok(RateLimitPolicy::new(settings, storage_builder)
            .with_policy_group_id(PolicyGroupId::new(self.policy_group_id)))
    }
}

/// Creates a fresh rate-limit configuring builder.
#[wasm_bindgen(js_name = buildRateLimit)]
pub fn build_rate_limit() -> JsRateLimitBuilder {
    JsRateLimitBuilder::default()
}
