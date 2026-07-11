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

//! Runtime built-in policy configuration for the JS binding.

use js_sys::Reflect;
use openpit::pretrade::policies::{
    OrderSizeLimitPolicyError, PnlBoundsKillSwitchPolicyError, RateLimitPolicyError,
    SpotFundsConfigError,
};
use wasm_bindgen::convert::TryFromJsValue;
use wasm_bindgen::prelude::*;

use crate::domain::{
    clone_wrapper_value, is_plain_object, parse_asset, parse_bounded_number, read_field,
    resolve_account_group_id, resolve_account_id, resolve_pnl,
};
use crate::engine::EngineLocking;
use crate::error::{configure_error_to_js, make_error, ErrorKind};
use crate::policy::order_size_limit::{
    JsOrderSizeAccountAssetBarrier, JsOrderSizeAssetBarrier, JsOrderSizeBrokerBarrier,
};
use crate::policy::pnl_killswitch::{
    JsPnlBoundsAccountAssetBarrierUpdate, JsPnlBoundsBrokerBarrier,
};
use crate::policy::rate_limit::{
    JsRateLimitAccountAssetBarrier, JsRateLimitAccountBarrier, JsRateLimitAssetBarrier,
    JsRateLimitBrokerBarrier,
};
use crate::policy::spot_funds::{
    parse_limit_mode, parse_pricing_source, JsSpotFundsOverride,
    JsSpotFundsPnlBoundsAccountBarrierUpdate, JsSpotFundsPnlBoundsAccountGroupBarrier,
    JsSpotFundsPnlBoundsBarrier,
};

#[wasm_bindgen(typescript_custom_section)]
const CONFIGURE_TS: &'static str = r#"
/** Runtime rate-limit configuration options. */
export interface RateLimitConfigureOptions {
  broker?: RateLimitBrokerBarrier | null;
  clearBroker?: boolean;
  assetBarriers?: Iterable<RateLimitAssetBarrier> | null;
  accountBarriers?: Iterable<RateLimitAccountBarrier> | null;
  accountAssetBarriers?: Iterable<RateLimitAccountAssetBarrier> | null;
}

/** Runtime generic P&L-bounds kill-switch configuration options. */
export interface PnlBoundsKillswitchConfigureOptions {
  brokerBarriers?: Iterable<PnlBoundsBrokerBarrier> | null;
  accountBarriers?: Iterable<PnlBoundsAccountAssetBarrierUpdate> | null;
}

/** Runtime generic P&L accumulator assignment options. */
export interface SetAccountPnlOptions {
  account: AccountId | number | bigint | string;
  settlementAsset: string;
  pnl: Pnl | string | number | bigint;
}

/** Runtime order-size-limit configuration options. */
export interface OrderSizeLimitConfigureOptions {
  broker?: OrderSizeBrokerBarrier | null;
  clearBroker?: boolean;
  assetBarriers?: Iterable<OrderSizeAssetBarrier> | null;
  accountAssetBarriers?: Iterable<OrderSizeAccountAssetBarrier> | null;
}

/** Spot-funds per-account limit-mode runtime entry. */
export interface SpotFundsLimitModeAccountEntry {
  accountId: AccountId | number | bigint | string;
  mode?: import("../types.js").SpotFundsLimitMode | null;
}

/** Spot-funds per-account-group limit-mode runtime entry. */
export interface SpotFundsLimitModeAccountGroupEntry {
  accountGroupId: AccountGroupId | number | bigint | string;
  mode?: import("../types.js").SpotFundsLimitMode | null;
}

/** Runtime spot-funds slippage/pricing/limit-mode options. */
export interface SpotFundsConfigureOptions {
  globalSlippageBps?: number;
  pricingSource?: import("../types.js").SpotFundsPricingSource | null;
  overrides?: Iterable<SpotFundsOverride> | null;
  globalLimitMode?: import("../types.js").SpotFundsLimitMode | null;
  accountLimitModes?: Iterable<SpotFundsLimitModeAccountEntry> | null;
  accountGroupLimitModes?: Iterable<SpotFundsLimitModeAccountGroupEntry> | null;
}

/**
 * Runtime spot-funds P&L-bounds axis configuration options.
 * Omitted axes stay unchanged; a supplied iterable replaces its axis, and an
 * empty iterable clears it.
 */
export interface SpotFundsPnlBoundsKillswitchConfigureOptions {
  globalBarriers?: Iterable<SpotFundsPnlBoundsBarrier> | null;
  accountGroupBarriers?: Iterable<SpotFundsPnlBoundsAccountGroupBarrier> | null;
  accountBarriers?: Iterable<SpotFundsPnlBoundsAccountBarrierUpdate> | null;
}

/** Runtime spot-funds P&L accumulator assignment options. */
export interface SetSpotFundsAccountPnlOptions {
  account: AccountId | number | bigint | string;
  accountCurrency: string;
  pnl: Pnl | string | number | bigint;
}
"#;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "RateLimitConfigureOptions")]
    pub type RateLimitConfigureOptionsLike;

    #[wasm_bindgen(typescript_type = "PnlBoundsKillswitchConfigureOptions")]
    pub type PnlBoundsKillswitchConfigureOptionsLike;

    #[wasm_bindgen(typescript_type = "SetAccountPnlOptions")]
    pub type SetAccountPnlOptionsLike;

    #[wasm_bindgen(typescript_type = "OrderSizeLimitConfigureOptions")]
    pub type OrderSizeLimitConfigureOptionsLike;

    #[wasm_bindgen(typescript_type = "SpotFundsConfigureOptions")]
    pub type SpotFundsConfigureOptionsLike;

    #[wasm_bindgen(typescript_type = "SpotFundsPnlBoundsKillswitchConfigureOptions")]
    pub type SpotFundsPnlBoundsKillswitchConfigureOptionsLike;

    #[wasm_bindgen(typescript_type = "SetSpotFundsAccountPnlOptions")]
    pub type SetSpotFundsAccountPnlOptionsLike;
}

/// Runtime built-in policy configurator.
#[wasm_bindgen(js_name = Configurator)]
#[derive(Clone)]
pub struct JsConfigurator {
    inner: openpit::Configurator<EngineLocking>,
}

type SpotFundsLimitModeEntry<Id> = (Id, Option<openpit::pretrade::SpotFundsLimitMode>);

#[wasm_bindgen(js_class = Configurator)]
impl JsConfigurator {
    /// Retunes a registered rate-limit policy at runtime.
    ///
    /// # Errors
    ///
    /// Throws `PolicyConfigureError` on unknown policy, type mismatch, or
    /// validation failure. Throws `ParamError` for malformed options.
    #[wasm_bindgen(js_name = rateLimit)]
    pub fn rate_limit(
        &self,
        name: &str,
        options: RateLimitConfigureOptionsLike,
    ) -> Result<(), JsValue> {
        let options = options.into();
        require_object(&options, "rateLimit options")?;
        let broker = optional_wrapper_field::<JsRateLimitBrokerBarrier>(&options, "broker")?
            .map(|barrier| barrier.to_core());
        let clear_broker = optional_bool_field(&options, "clearBroker")?;
        if broker.is_some() && clear_broker.unwrap_or(false) {
            return Err(make_error(
                ErrorKind::Param,
                "broker and clearBroker cannot be used together",
                Some("Other"),
            ));
        }
        let assets =
            optional_wrapper_iter_field::<JsRateLimitAssetBarrier>(&options, "assetBarriers")?;
        let accounts =
            optional_wrapper_iter_field::<JsRateLimitAccountBarrier>(&options, "accountBarriers")?;
        let account_assets = optional_wrapper_iter_field::<JsRateLimitAccountAssetBarrier>(
            &options,
            "accountAssetBarriers",
        )?;

        self.inner
            .rate_limit(name, |settings| {
                if let Some(broker) = broker {
                    settings.set_broker(Some(broker))?;
                } else if clear_broker.unwrap_or(false) {
                    settings.set_broker(None)?;
                }
                if let Some(barriers) = assets {
                    settings.set_asset_barriers(
                        barriers.iter().map(JsRateLimitAssetBarrier::to_core),
                    )?;
                }
                if let Some(barriers) = accounts {
                    settings.set_account_barriers(
                        barriers
                            .iter()
                            .copied()
                            .map(JsRateLimitAccountBarrier::to_core),
                    )?;
                }
                if let Some(barriers) = account_assets {
                    settings.set_account_asset_barriers(
                        barriers.iter().map(JsRateLimitAccountAssetBarrier::to_core),
                    )?;
                }
                Ok::<(), RateLimitPolicyError>(())
            })
            .map_err(configure_error_to_js)
    }

    /// Retunes a registered P&L-bounds kill-switch policy at runtime.
    #[wasm_bindgen(js_name = pnlBoundsKillswitch)]
    pub fn pnl_bounds_killswitch(
        &self,
        name: &str,
        options: PnlBoundsKillswitchConfigureOptionsLike,
    ) -> Result<(), JsValue> {
        let options = options.into();
        require_object(&options, "pnlBoundsKillswitch options")?;
        let brokers =
            optional_wrapper_iter_field::<JsPnlBoundsBrokerBarrier>(&options, "brokerBarriers")?;
        let accounts = optional_wrapper_iter_field::<JsPnlBoundsAccountAssetBarrierUpdate>(
            &options,
            "accountBarriers",
        )?;
        self.inner
            .pnl_bounds_killswitch(name, |settings| {
                if let Some(barriers) = brokers {
                    settings.set_broker_barriers(
                        barriers.iter().map(JsPnlBoundsBrokerBarrier::to_core),
                    )?;
                }
                if let Some(barriers) = accounts {
                    settings.set_account_barriers(
                        barriers
                            .iter()
                            .map(JsPnlBoundsAccountAssetBarrierUpdate::to_core),
                    )?;
                }
                Ok::<(), PnlBoundsKillSwitchPolicyError>(())
            })
            .map_err(configure_error_to_js)
    }

    /// Force-sets live accumulated P&L for one generic P&L kill-switch entry.
    #[wasm_bindgen(js_name = setAccountPnl)]
    pub fn set_account_pnl(
        &self,
        name: &str,
        options: SetAccountPnlOptionsLike,
    ) -> Result<(), JsValue> {
        let options = options.into();
        require_object(&options, "setAccountPnl options")?;
        let account = resolve_account_id(required_field(&options, "account")?)?;
        let settlement_asset = parse_asset(&required_string_field(&options, "settlementAsset")?)?;
        let pnl = resolve_pnl(required_field(&options, "pnl")?)?;
        self.inner
            .set_account_pnl(name, account, settlement_asset, pnl)
            .map_err(configure_error_to_js)
    }

    /// Retunes a registered order-size-limit policy at runtime.
    #[wasm_bindgen(js_name = orderSizeLimit)]
    pub fn order_size_limit(
        &self,
        name: &str,
        options: OrderSizeLimitConfigureOptionsLike,
    ) -> Result<(), JsValue> {
        let options = options.into();
        require_object(&options, "orderSizeLimit options")?;
        let broker = optional_wrapper_field::<JsOrderSizeBrokerBarrier>(&options, "broker")?
            .map(|barrier| barrier.to_core());
        let clear_broker = optional_bool_field(&options, "clearBroker")?;
        if broker.is_some() && clear_broker.unwrap_or(false) {
            return Err(make_error(
                ErrorKind::Param,
                "broker and clearBroker cannot be used together",
                Some("Other"),
            ));
        }
        let assets =
            optional_wrapper_iter_field::<JsOrderSizeAssetBarrier>(&options, "assetBarriers")?;
        let account_assets = optional_wrapper_iter_field::<JsOrderSizeAccountAssetBarrier>(
            &options,
            "accountAssetBarriers",
        )?;

        self.inner
            .order_size_limit(name, |settings| {
                if let Some(broker) = broker {
                    settings.set_broker(Some(broker))?;
                } else if clear_broker.unwrap_or(false) {
                    settings.set_broker(None)?;
                }
                if let Some(barriers) = assets {
                    settings.set_asset_barriers(
                        barriers.iter().map(JsOrderSizeAssetBarrier::to_core),
                    )?;
                }
                if let Some(barriers) = account_assets {
                    settings.set_account_asset_barriers(
                        barriers.iter().map(JsOrderSizeAccountAssetBarrier::to_core),
                    )?;
                }
                Ok::<(), OrderSizeLimitPolicyError>(())
            })
            .map_err(configure_error_to_js)
    }

    /// Retunes a registered spot-funds policy at runtime.
    #[wasm_bindgen(js_name = spotFunds)]
    pub fn spot_funds(
        &self,
        name: &str,
        options: SpotFundsConfigureOptionsLike,
    ) -> Result<(), JsValue> {
        let options = options.into();
        require_object(&options, "spotFunds options")?;
        let global_slippage_bps = optional_u16_field(&options, "globalSlippageBps")?;
        let pricing_source = match read_field(&options, "pricingSource")? {
            value if value.is_undefined() || value.is_null() => None,
            value => Some(parse_pricing_source(&value)?),
        };
        let overrides = optional_wrapper_iter_field::<JsSpotFundsOverride>(&options, "overrides")?;
        let global_limit_mode = parse_limit_mode(&read_field(&options, "globalLimitMode")?)?;
        let account_limit_modes = optional_limit_mode_entries(
            &options,
            "accountLimitModes",
            "accountId",
            resolve_account_id,
        )?;
        let account_group_limit_modes = optional_limit_mode_entries(
            &options,
            "accountGroupLimitModes",
            "accountGroupId",
            resolve_account_group_id,
        )?;

        self.inner
            .spot_funds(name, |settings| {
                if let Some(bps) = global_slippage_bps {
                    settings.set_global_slippage_bps(bps)?;
                }
                if let Some(source) = pricing_source {
                    settings.set_pricing_source(source);
                }
                if let Some(entries) = overrides {
                    for entry in entries {
                        let (target, override_value) = entry.to_core_entry();
                        settings.set_override(target, override_value)?;
                    }
                }
                if let Some(mode) = global_limit_mode {
                    settings.set_global_limit_mode(mode);
                }
                if let Some(entries) = account_limit_modes {
                    for (account, mode) in entries {
                        settings.set_account_limit_mode(account, mode);
                    }
                }
                if let Some(entries) = account_group_limit_modes {
                    for (group, mode) in entries {
                        settings.set_account_group_limit_mode(group, mode);
                    }
                }
                Ok::<(), SpotFundsConfigError>(())
            })
            .map_err(configure_error_to_js)
    }

    /// Retunes the spot-funds account-currency P&L-bounds axis.
    #[wasm_bindgen(js_name = spotFundsPnlBoundsKillswitch)]
    pub fn spot_funds_pnl_bounds_killswitch(
        &self,
        name: &str,
        options: SpotFundsPnlBoundsKillswitchConfigureOptionsLike,
    ) -> Result<(), JsValue> {
        let options = options.into();
        require_object(&options, "spotFundsPnlBoundsKillswitch options")?;
        let global =
            optional_wrapper_iter_field::<JsSpotFundsPnlBoundsBarrier>(&options, "globalBarriers")?;
        let account_group = optional_wrapper_iter_field::<JsSpotFundsPnlBoundsAccountGroupBarrier>(
            &options,
            "accountGroupBarriers",
        )?;
        let account = optional_wrapper_iter_field::<JsSpotFundsPnlBoundsAccountBarrierUpdate>(
            &options,
            "accountBarriers",
        )?;

        self.inner
            .spot_funds(name, |settings| {
                if let Some(barriers) = global {
                    settings.set_pnl_global_barriers(
                        barriers.iter().map(JsSpotFundsPnlBoundsBarrier::to_core),
                    )?;
                }
                if let Some(barriers) = account_group {
                    settings.set_pnl_account_group_barriers(
                        barriers
                            .iter()
                            .map(JsSpotFundsPnlBoundsAccountGroupBarrier::to_core),
                    )?;
                }
                if let Some(barriers) = account {
                    settings.set_pnl_account_barriers(
                        barriers
                            .iter()
                            .map(JsSpotFundsPnlBoundsAccountBarrierUpdate::to_core),
                    )?;
                }
                Ok::<(), SpotFundsConfigError>(())
            })
            .map_err(configure_error_to_js)
    }

    /// Force-sets live accumulated spot-funds account-currency P&L.
    #[wasm_bindgen(js_name = setSpotFundsAccountPnl)]
    pub fn set_spot_funds_account_pnl(
        &self,
        name: &str,
        options: SetSpotFundsAccountPnlOptionsLike,
    ) -> Result<(), JsValue> {
        let options = options.into();
        require_object(&options, "setSpotFundsAccountPnl options")?;
        let account = resolve_account_id(required_field(&options, "account")?)?;
        let account_currency = parse_asset(&required_string_field(&options, "accountCurrency")?)?;
        let pnl = resolve_pnl(required_field(&options, "pnl")?)?;
        self.inner
            .set_spot_funds_account_pnl(name, account, account_currency, pnl)
            .map_err(configure_error_to_js)
    }
}

impl JsConfigurator {
    pub(crate) fn from_inner(inner: openpit::Configurator<EngineLocking>) -> Self {
        Self { inner }
    }
}

fn require_object(value: &JsValue, label: &str) -> Result<(), JsValue> {
    if is_plain_object(value) {
        Ok(())
    } else {
        Err(make_error(
            ErrorKind::Type,
            &format!("{label} must be a plain object"),
            None,
        ))
    }
}

fn required_field(value: &JsValue, field: &str) -> Result<JsValue, JsValue> {
    let field_value = read_field(value, field)?;
    if field_value.is_undefined() || field_value.is_null() {
        return Err(make_error(
            ErrorKind::Type,
            &format!("{field} is required"),
            None,
        ));
    }
    Ok(field_value)
}

fn required_string_field(value: &JsValue, field: &str) -> Result<String, JsValue> {
    required_field(value, field)?
        .as_string()
        .ok_or_else(|| make_error(ErrorKind::Type, &format!("{field} must be a string"), None))
}

fn optional_bool_field(value: &JsValue, field: &str) -> Result<Option<bool>, JsValue> {
    let field_value = read_field(value, field)?;
    if field_value.is_undefined() || field_value.is_null() {
        return Ok(None);
    }
    field_value
        .as_bool()
        .map(Some)
        .ok_or_else(|| make_error(ErrorKind::Type, &format!("{field} must be a boolean"), None))
}

fn optional_u16_field(value: &JsValue, field: &str) -> Result<Option<u16>, JsValue> {
    let field_value = read_field(value, field)?;
    if field_value.is_undefined() || field_value.is_null() {
        return Ok(None);
    }
    Ok(Some(
        parse_bounded_number(field_value, u64::from(u16::MAX), field)? as u16,
    ))
}

fn optional_wrapper_field<T>(value: &JsValue, field: &str) -> Result<Option<T>, JsValue>
where
    T: TryFromJsValue,
{
    let field_value = read_field(value, field)?;
    if field_value.is_undefined() || field_value.is_null() {
        return Ok(None);
    }
    extract_wrapper(field_value, field).map(Some)
}

fn optional_wrapper_iter_field<T>(value: &JsValue, field: &str) -> Result<Option<Vec<T>>, JsValue>
where
    T: TryFromJsValue,
{
    let field_value = read_field(value, field)?;
    if field_value.is_undefined() || field_value.is_null() {
        return Ok(None);
    }
    collect_wrappers(field_value, field).map(Some)
}

fn collect_wrappers<T>(value: JsValue, field: &str) -> Result<Vec<T>, JsValue>
where
    T: TryFromJsValue,
{
    let iterator = js_sys::try_iter(&value)?.ok_or_else(|| iterable_error(field))?;
    let mut result = Vec::new();
    for item in iterator {
        result.push(extract_wrapper(item?, field)?);
    }
    Ok(result)
}

fn extract_wrapper<T>(value: JsValue, field: &str) -> Result<T, JsValue>
where
    T: TryFromJsValue,
{
    let value = clone_wrapper_value(&value)?.unwrap_or(value);
    T::try_from_js_value(value).map_err(|_| {
        make_error(
            ErrorKind::Type,
            &format!("{field} contains an invalid entry"),
            None,
        )
    })
}

fn iterable_error(field: &str) -> JsValue {
    make_error(ErrorKind::Type, &format!("{field} must be iterable"), None)
}

fn optional_limit_mode_entries<Id>(
    options: &JsValue,
    field: &str,
    id_field: &str,
    parse_id: impl Fn(JsValue) -> Result<Id, JsValue>,
) -> Result<Option<Vec<SpotFundsLimitModeEntry<Id>>>, JsValue> {
    let value = read_field(options, field)?;
    if value.is_undefined() || value.is_null() {
        return Ok(None);
    }
    let iterator = js_sys::try_iter(&value)?.ok_or_else(|| iterable_error(field))?;
    let mut entries = Vec::new();
    for item in iterator {
        let item = item?;
        require_object(&item, field)?;
        if !has_own_property(&item, "mode")? {
            continue;
        }
        let id = parse_id(required_field(&item, id_field)?)?;
        let mode = parse_limit_mode(&read_field(&item, "mode")?)?;
        entries.push((id, mode));
    }
    Ok(Some(entries))
}

fn has_own_property(value: &JsValue, field: &str) -> Result<bool, JsValue> {
    Ok(Reflect::own_keys(value)?
        .iter()
        .any(|key| key.as_string().is_some_and(|key| key == field)))
}
