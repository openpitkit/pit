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

//! Builtin P&L-bounds kill-switch policy builder and its barrier data types.
//!
//! `buildPnlBoundsKillswitch()` returns a configuring builder. Calling either
//! barrier setter (`brokerBarriers`, `accountBarriers`) produces a ready-builder
//! token that can be passed to `builder.builtin(token)`. `withPolicyGroupId`
//! preserves the current stage. Each barrier carries optional
//! `lowerBound`/`upperBound` P&L bounds (at least one per barrier); decimals
//! cross as `Pnl` value objects or `DecimalInput`.

use openpit::param::{Asset, Pnl};
use openpit::pretrade::policies::{
    PnlBoundsAccountAssetBarrier, PnlBoundsAccountAssetBarrierUpdate, PnlBoundsBrokerBarrier,
    PnlBoundsKillSwitchPolicy, PnlBoundsKillSwitchSettings,
};
use openpit::pretrade::PolicyGroupId;
use wasm_bindgen::prelude::*;

use crate::domain::{
    collect_cloned_wrappers, parse_asset, resolve_account_id, resolve_optional_pnl, resolve_pnl,
    AccountIdLike, IntegerNumber, OptionalPnlLike, PnlLike,
};
use crate::error::engine_build_configuration_error;
use crate::param::ids::JsAccountId;

/// Broker-wide P&L-bounds barrier (at least one bound required).
#[wasm_bindgen(js_name = PnlBoundsBrokerBarrier)]
#[derive(Clone)]
pub struct JsPnlBoundsBrokerBarrier {
    settlement_asset: Asset,
    lower_bound: Option<Pnl>,
    upper_bound: Option<Pnl>,
}

#[wasm_bindgen(js_class = PnlBoundsBrokerBarrier)]
impl JsPnlBoundsBrokerBarrier {
    /// Constructs a broker barrier from a settlement asset and bounds.
    ///
    /// `lowerBound` and `upperBound` each accept a `Pnl` value object, a
    /// `DecimalInput`, or `null`/`undefined`. At least one bound must be set;
    /// the core validates this on build.
    ///
    /// # Errors
    ///
    /// Throws `AssetError` when `settlementAsset` is empty or `ParamError` on
    /// an invalid bound.
    #[wasm_bindgen(constructor)]
    pub fn new(
        settlement_asset: &str,
        lower_bound: OptionalPnlLike,
        upper_bound: OptionalPnlLike,
    ) -> Result<JsPnlBoundsBrokerBarrier, JsValue> {
        Ok(Self {
            settlement_asset: parse_asset(settlement_asset)?,
            lower_bound: resolve_optional_pnl(lower_bound.into())?,
            upper_bound: resolve_optional_pnl(upper_bound.into())?,
        })
    }

    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsPnlBoundsBrokerBarrier {
        self.clone()
    }
}

impl JsPnlBoundsBrokerBarrier {
    /// Returns the core broker barrier.
    pub(crate) fn to_core(&self) -> PnlBoundsBrokerBarrier {
        PnlBoundsBrokerBarrier {
            settlement_asset: self.settlement_asset.clone(),
            lower_bound: self.lower_bound,
            upper_bound: self.upper_bound,
        }
    }
}

/// Runtime per-(account, settlement-asset) P&L-bounds barrier update.
#[wasm_bindgen(js_name = PnlBoundsAccountAssetBarrierUpdate)]
#[derive(Clone)]
pub struct JsPnlBoundsAccountAssetBarrierUpdate {
    account_id: JsAccountId,
    settlement_asset: Asset,
    lower_bound: Option<Pnl>,
    upper_bound: Option<Pnl>,
}

#[wasm_bindgen(js_class = PnlBoundsAccountAssetBarrierUpdate)]
impl JsPnlBoundsAccountAssetBarrierUpdate {
    /// Constructs a runtime account-asset barrier update from its parts.
    ///
    /// Unlike `PnlBoundsAccountAssetBarrier`, this type has no `initialPnl`:
    /// runtime updates preserve the live accumulated P&L.
    ///
    /// # Errors
    ///
    /// Throws `AssetError` when `settlementAsset` is empty or `ParamError` on
    /// an invalid bound.
    #[wasm_bindgen(constructor)]
    pub fn new(
        account_id: AccountIdLike,
        settlement_asset: &str,
        lower_bound: OptionalPnlLike,
        upper_bound: OptionalPnlLike,
    ) -> Result<JsPnlBoundsAccountAssetBarrierUpdate, JsValue> {
        Ok(Self {
            account_id: JsAccountId::from_inner(resolve_account_id(account_id.into())?),
            settlement_asset: parse_asset(settlement_asset)?,
            lower_bound: resolve_optional_pnl(lower_bound.into())?,
            upper_bound: resolve_optional_pnl(upper_bound.into())?,
        })
    }

    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsPnlBoundsAccountAssetBarrierUpdate {
        self.clone()
    }
}

impl JsPnlBoundsAccountAssetBarrierUpdate {
    pub(crate) fn to_core(&self) -> PnlBoundsAccountAssetBarrierUpdate {
        PnlBoundsAccountAssetBarrierUpdate {
            barrier: PnlBoundsBrokerBarrier {
                settlement_asset: self.settlement_asset.clone(),
                lower_bound: self.lower_bound,
                upper_bound: self.upper_bound,
            },
            account_id: self.account_id.inner(),
        }
    }
}

/// Per-(account, settlement-asset) P&L-bounds barrier.
#[wasm_bindgen(js_name = PnlBoundsAccountAssetBarrier)]
#[derive(Clone)]
pub struct JsPnlBoundsAccountAssetBarrier {
    account_id: JsAccountId,
    settlement_asset: Asset,
    initial_pnl: Pnl,
    lower_bound: Option<Pnl>,
    upper_bound: Option<Pnl>,
}

#[wasm_bindgen(js_class = PnlBoundsAccountAssetBarrier)]
impl JsPnlBoundsAccountAssetBarrier {
    /// Constructs an account-asset barrier from its parts.
    ///
    /// `initialPnl` seeds the account's running P&L; `lowerBound`/`upperBound`
    /// accept a `Pnl`, a `DecimalInput`, or `null`/`undefined` (at least one
    /// bound required, validated on build).
    ///
    /// # Errors
    ///
    /// Throws `AssetError` when `settlementAsset` is empty or `ParamError` on
    /// an invalid value.
    #[wasm_bindgen(constructor)]
    pub fn new(
        account_id: AccountIdLike,
        settlement_asset: &str,
        initial_pnl: PnlLike,
        lower_bound: OptionalPnlLike,
        upper_bound: OptionalPnlLike,
    ) -> Result<JsPnlBoundsAccountAssetBarrier, JsValue> {
        Ok(Self {
            account_id: JsAccountId::from_inner(resolve_account_id(account_id.into())?),
            settlement_asset: parse_asset(settlement_asset)?,
            initial_pnl: resolve_pnl(initial_pnl.into())?,
            lower_bound: resolve_optional_pnl(lower_bound.into())?,
            upper_bound: resolve_optional_pnl(upper_bound.into())?,
        })
    }

    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsPnlBoundsAccountAssetBarrier {
        self.clone()
    }
}

impl JsPnlBoundsAccountAssetBarrier {
    /// Returns the core account-asset barrier.
    pub(crate) fn to_core(&self) -> PnlBoundsAccountAssetBarrier {
        PnlBoundsAccountAssetBarrier {
            barrier: PnlBoundsBrokerBarrier {
                settlement_asset: self.settlement_asset.clone(),
                lower_bound: self.lower_bound,
                upper_bound: self.upper_bound,
            },
            account_id: self.account_id.inner(),
            initial_pnl: self.initial_pnl,
        }
    }
}

/// Configuring builder for the builtin P&L-bounds kill-switch policy.
#[wasm_bindgen(js_name = PnlBoundsKillswitchBuilder)]
#[derive(Clone, Default)]
pub struct JsPnlBoundsKillswitchBuilder {
    policy_group_id: u16,
    broker_barriers: Vec<JsPnlBoundsBrokerBarrier>,
    account_barriers: Vec<JsPnlBoundsAccountAssetBarrier>,
}

#[wasm_bindgen(js_class = PnlBoundsKillswitchBuilder)]
impl JsPnlBoundsKillswitchBuilder {
    /// Stable name registered by the builtin P&L-bounds kill-switch policy.
    #[wasm_bindgen(getter, js_name = NAME)]
    pub fn name() -> String {
        PnlBoundsKillSwitchPolicy::<crate::engine::StorageFactory>::NAME.to_owned()
    }

    /// Adds broker barriers and returns the builder for chaining.
    ///
    /// `barriers` is an array of `PnlBoundsBrokerBarrier`.
    #[wasm_bindgen(
        js_name = brokerBarriers,
        unchecked_return_type = "PnlBoundsKillswitchReadyBuilder"
    )]
    pub fn broker_barriers(
        &self,
        #[wasm_bindgen(unchecked_param_type = "Iterable<PnlBoundsBrokerBarrier>")]
        barriers: JsValue,
    ) -> Result<JsPnlBoundsKillswitchBuilder, JsValue> {
        let barriers: Vec<JsPnlBoundsBrokerBarrier> =
            collect_cloned_wrappers(&barriers, "brokerBarriers")?;
        let mut next = self.clone();
        next.broker_barriers.extend(barriers);
        Ok(next)
    }

    /// Adds account barriers and returns the builder for chaining.
    ///
    /// `barriers` is an array of `PnlBoundsAccountAssetBarrier`.
    #[wasm_bindgen(
        js_name = accountBarriers,
        unchecked_return_type = "PnlBoundsKillswitchReadyBuilder"
    )]
    pub fn account_barriers(
        &self,
        #[wasm_bindgen(unchecked_param_type = "Iterable<PnlBoundsAccountAssetBarrier>")]
        barriers: JsValue,
    ) -> Result<JsPnlBoundsKillswitchBuilder, JsValue> {
        let barriers: Vec<JsPnlBoundsAccountAssetBarrier> =
            collect_cloned_wrappers(&barriers, "accountBarriers")?;
        let mut next = self.clone();
        next.account_barriers.extend(barriers);
        Ok(next)
    }

    /// Assigns the policy group id and returns the builder for chaining.
    #[wasm_bindgen(js_name = withPolicyGroupId)]
    pub fn with_policy_group_id(
        &self,
        policy_group_id: IntegerNumber,
    ) -> Result<JsPnlBoundsKillswitchBuilder, JsValue> {
        let mut next = self.clone();
        next.policy_group_id = crate::lock::parse_policy_group_id(policy_group_id.into())?.value();
        Ok(next)
    }

    /// Returns an independent builder with the same configuration.
    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsPnlBoundsKillswitchBuilder {
        self.clone()
    }
}

impl JsPnlBoundsKillswitchBuilder {
    /// Builds the core policy from this token and the engine's storage
    /// builder.
    ///
    /// # Errors
    ///
    /// Throws `EngineBuildError` when no barrier is configured or a barrier has
    /// no bound set.
    pub(crate) fn build_policy(
        &self,
        storage_builder: &crate::engine::StorageBuilderRef,
    ) -> Result<PnlBoundsKillSwitchPolicy<crate::engine::StorageFactory>, JsValue> {
        let brokers: Vec<PnlBoundsBrokerBarrier> = self
            .broker_barriers
            .iter()
            .map(JsPnlBoundsBrokerBarrier::to_core)
            .collect();
        let accounts: Vec<PnlBoundsAccountAssetBarrier> = self
            .account_barriers
            .iter()
            .map(JsPnlBoundsAccountAssetBarrier::to_core)
            .collect();
        let settings = PnlBoundsKillSwitchSettings::new(brokers.into_iter(), accounts.into_iter())
            .map_err(|error| engine_build_configuration_error(&error.to_string()))?;
        Ok(PnlBoundsKillSwitchPolicy::new(settings, storage_builder)
            .with_policy_group_id(PolicyGroupId::new(self.policy_group_id)))
    }
}

/// Creates a fresh P&L-bounds kill-switch configuring builder.
#[wasm_bindgen(js_name = buildPnlBoundsKillswitch)]
pub fn build_pnl_bounds_killswitch() -> JsPnlBoundsKillswitchBuilder {
    JsPnlBoundsKillswitchBuilder::default()
}
