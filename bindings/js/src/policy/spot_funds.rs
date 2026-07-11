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

//! Builtin spot-funds policy builder and its market-data configuration.
//!
//! `buildSpotFunds()` returns a limit-only builder (market orders are rejected
//! `UnsupportedOrderType` until market data is configured). `withPolicyGroupId`
//! chains, and `marketData(service, defaultSlippageBps, pricingSource?,
//! overrides?)` attaches a live `MarketDataService` plus the slippage cascade,
//! returning a ready builder. Either form is an opaque token passed to
//! `builder.builtin(token)`.
//!
//! Initial balances are seeded through the account-adjustment pipeline (not the
//! builder). The core `SpotFundsPolicy::new(market_orders, storage_builder)`
//! takes the optional market-data bundle and the engine's storage builder.

use openpit::param::{Asset, Pnl};
use openpit::pretrade::policies::{
    SpotFundsLimitMode, SpotFundsMarketData, SpotFundsOverride, SpotFundsOverrideTarget,
    SpotFundsPnlBoundsAccountBarrier, SpotFundsPnlBoundsAccountBarrierUpdate,
    SpotFundsPnlBoundsAccountGroupBarrier, SpotFundsPnlBoundsBarrier, SpotFundsPolicy,
    SpotFundsPricingSource, SpotFundsSettings,
};
use openpit::pretrade::PolicyGroupId;
use openpit_interop::EngineLocking;
use wasm_bindgen::prelude::*;

use crate::domain::{
    collect_cloned_wrappers, extract_cloned_wrapper, parse_asset, parse_bounded_number,
    resolve_account_group_id, resolve_account_id, resolve_instrument_id,
    resolve_optional_account_group_id, resolve_optional_account_id, resolve_optional_pnl,
    resolve_pnl, AccountGroupIdLike, AccountIdLike, InstrumentIdLike, IntegerNumber,
    OptionalAccountGroupIdLike, OptionalAccountIdLike, OptionalIntegerNumber, OptionalPnlLike,
    PnlLike,
};
use crate::error::{engine_build_configuration_error, make_error, ErrorKind};
use crate::marketdata::JsMarketDataService;
use crate::param::ids::{JsAccountGroupId, JsAccountId};

#[wasm_bindgen]
extern "C" {
    /// A spot-funds pricing-source wire string (or omitted for the `"Mark"`
    /// default).
    #[wasm_bindgen(typescript_type = "\"Mark\" | \"BookTop\" | null | undefined")]
    pub type SpotFundsPricingSourceLike;

    /// An iterable of `SpotFundsOverride` (or omitted for none).
    #[wasm_bindgen(typescript_type = "Iterable<SpotFundsOverride> | null | undefined")]
    pub type SpotFundsOverrideIterable;

    /// A spot-funds limit mode wire string.
    #[wasm_bindgen(typescript_type = "\"Enforce\" | \"TrackOnly\" | null | undefined")]
    pub type SpotFundsLimitModeLike;
}

/// Live market-data bundle type for the binding-layer locking mode.
type MarketDataBundle = SpotFundsMarketData<EngineLocking>;

/// Parses a `SpotFundsPricingSource` wire string (`"Mark"`/`"BookTop"`).
///
/// An omitted/`null`/`undefined` value defaults to `Mark`.
///
/// # Errors
///
/// Throws `TypeError` for a non-string or `RangeError` for an unknown value.
pub(crate) fn parse_pricing_source(value: &JsValue) -> Result<SpotFundsPricingSource, JsValue> {
    if value.is_undefined() || value.is_null() {
        return Ok(SpotFundsPricingSource::Mark);
    }
    let text = value
        .as_string()
        .ok_or_else(|| make_error(ErrorKind::Type, "pricingSource must be a string", None))?;
    match text.trim() {
        "Mark" | "MARK" => Ok(SpotFundsPricingSource::Mark),
        "BookTop" | "BOOK_TOP" => Ok(SpotFundsPricingSource::BookTop),
        _ => Err(pricing_source_error()),
    }
}

/// Builds the error raised for an invalid pricing-source string.
fn pricing_source_error() -> JsValue {
    make_error(
        ErrorKind::Range,
        "pricingSource must be \"Mark\" or \"BookTop\"",
        None,
    )
}

/// Slippage override applied at one tier of the spot-funds cascade.
///
/// `accountId` and `accountGroupId` are mutually exclusive; setting neither
/// targets the instrument default. Resolution order is
/// (instrument, account) -> (instrument, group) -> instrument -> global.
#[wasm_bindgen(js_name = SpotFundsOverride)]
#[derive(Clone)]
pub struct JsSpotFundsOverride {
    target: SpotFundsOverrideTarget,
    slippage_bps: Option<u16>,
}

#[wasm_bindgen(js_class = SpotFundsOverride)]
impl JsSpotFundsOverride {
    /// Constructs a slippage override.
    ///
    /// `accountId` and `accountGroupId` are mutually exclusive. `slippageBps`
    /// is the override slippage in basis points, or `null`/`undefined` to defer
    /// to the next cascade tier.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` when both `accountId` and `accountGroupId` are set.
    #[wasm_bindgen(constructor)]
    pub fn new(
        instrument: InstrumentIdLike,
        account_id: OptionalAccountIdLike,
        account_group_id: OptionalAccountGroupIdLike,
        slippage_bps: OptionalIntegerNumber,
    ) -> Result<JsSpotFundsOverride, JsValue> {
        let instrument = resolve_instrument_id(instrument.into())?;
        let account_id = resolve_optional_account_id(account_id.into())?;
        let account_group_id = resolve_optional_account_group_id(account_group_id.into())?;
        let target = match (account_id, account_group_id) {
            (Some(_), Some(_)) => {
                return Err(make_error(
                    ErrorKind::Param,
                    "accountId and accountGroupId are mutually exclusive",
                    Some("Other"),
                ));
            }
            (Some(account_id), None) => {
                SpotFundsOverrideTarget::InstrumentAccount(instrument, account_id)
            }
            (None, Some(account_group_id)) => {
                SpotFundsOverrideTarget::InstrumentAccountGroup(instrument, account_group_id)
            }
            (None, None) => SpotFundsOverrideTarget::Instrument(instrument),
        };
        let slippage_bps: JsValue = slippage_bps.into();
        let slippage_bps = if slippage_bps.is_null() || slippage_bps.is_undefined() {
            None
        } else {
            Some(parse_bounded_number(slippage_bps, u64::from(u16::MAX), "slippageBps")? as u16)
        };
        Ok(Self {
            target,
            slippage_bps,
        })
    }

    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsSpotFundsOverride {
        self.clone()
    }
}

impl JsSpotFundsOverride {
    /// Returns the core `(target, override)` cascade entry.
    pub(crate) fn to_core_entry(&self) -> (SpotFundsOverrideTarget, SpotFundsOverride) {
        (
            self.target,
            SpotFundsOverride {
                slippage_bps: self.slippage_bps,
            },
        )
    }
}

/// Parses a spot-funds limit mode wire string.
pub(crate) fn parse_limit_mode(value: &JsValue) -> Result<Option<SpotFundsLimitMode>, JsValue> {
    if value.is_undefined() || value.is_null() {
        return Ok(None);
    }
    let value = value
        .as_string()
        .ok_or_else(|| make_error(ErrorKind::Type, "limit mode must be a string", None))?;
    match value.as_str() {
        "Enforce" => Ok(Some(SpotFundsLimitMode::Enforce)),
        "TrackOnly" => Ok(Some(SpotFundsLimitMode::TrackOnly)),
        _ => Err(make_error(
            ErrorKind::Range,
            "limit mode must be \"Enforce\" or \"TrackOnly\"",
            None,
        )),
    }
}

/// Account-currency P&L-bounds barrier for spot funds.
#[wasm_bindgen(js_name = SpotFundsPnlBoundsBarrier)]
#[derive(Clone)]
pub struct JsSpotFundsPnlBoundsBarrier {
    account_currency: Asset,
    lower_bound: Option<Pnl>,
    upper_bound: Option<Pnl>,
}

#[wasm_bindgen(js_class = SpotFundsPnlBoundsBarrier)]
impl JsSpotFundsPnlBoundsBarrier {
    /// Constructs a global spot-funds P&L-bounds barrier.
    ///
    /// # Errors
    ///
    /// Throws `AssetError` when `accountCurrency` is empty or `ParamError` on
    /// an invalid bound.
    #[wasm_bindgen(constructor)]
    pub fn new(
        account_currency: &str,
        lower_bound: OptionalPnlLike,
        upper_bound: OptionalPnlLike,
    ) -> Result<JsSpotFundsPnlBoundsBarrier, JsValue> {
        Ok(Self {
            account_currency: parse_asset(account_currency)?,
            lower_bound: resolve_optional_pnl(lower_bound.into())?,
            upper_bound: resolve_optional_pnl(upper_bound.into())?,
        })
    }

    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsSpotFundsPnlBoundsBarrier {
        self.clone()
    }
}

impl JsSpotFundsPnlBoundsBarrier {
    pub(crate) fn to_core(&self) -> SpotFundsPnlBoundsBarrier {
        SpotFundsPnlBoundsBarrier {
            account_currency: self.account_currency.clone(),
            lower_bound: self.lower_bound,
            upper_bound: self.upper_bound,
        }
    }
}

/// Account-group spot-funds P&L-bounds barrier.
#[wasm_bindgen(js_name = SpotFundsPnlBoundsAccountGroupBarrier)]
#[derive(Clone)]
pub struct JsSpotFundsPnlBoundsAccountGroupBarrier {
    barrier: JsSpotFundsPnlBoundsBarrier,
    account_group_id: JsAccountGroupId,
}

#[wasm_bindgen(js_class = SpotFundsPnlBoundsAccountGroupBarrier)]
impl JsSpotFundsPnlBoundsAccountGroupBarrier {
    /// Constructs an account-group spot-funds P&L-bounds barrier.
    ///
    /// # Errors
    ///
    /// Throws `AssetError`/`ParamError` on invalid input.
    #[wasm_bindgen(constructor)]
    pub fn new(
        account_group_id: AccountGroupIdLike,
        account_currency: &str,
        lower_bound: OptionalPnlLike,
        upper_bound: OptionalPnlLike,
    ) -> Result<JsSpotFundsPnlBoundsAccountGroupBarrier, JsValue> {
        Ok(Self {
            barrier: JsSpotFundsPnlBoundsBarrier::new(account_currency, lower_bound, upper_bound)?,
            account_group_id: JsAccountGroupId::from_inner(resolve_account_group_id(
                account_group_id.into(),
            )?),
        })
    }

    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsSpotFundsPnlBoundsAccountGroupBarrier {
        self.clone()
    }
}

impl JsSpotFundsPnlBoundsAccountGroupBarrier {
    pub(crate) fn to_core(&self) -> SpotFundsPnlBoundsAccountGroupBarrier {
        SpotFundsPnlBoundsAccountGroupBarrier {
            barrier: self.barrier.to_core(),
            account_group_id: self.account_group_id.inner(),
        }
    }
}

/// Build-time account spot-funds P&L-bounds barrier with an initial P&L seed.
#[wasm_bindgen(js_name = SpotFundsPnlBoundsAccountBarrier)]
#[derive(Clone)]
pub struct JsSpotFundsPnlBoundsAccountBarrier {
    barrier: JsSpotFundsPnlBoundsBarrier,
    account_id: JsAccountId,
    initial_pnl: Pnl,
}

#[wasm_bindgen(js_class = SpotFundsPnlBoundsAccountBarrier)]
impl JsSpotFundsPnlBoundsAccountBarrier {
    /// Constructs a build-time account spot-funds P&L-bounds barrier.
    ///
    /// `initialPnl` seeds live accumulated P&L at policy construction time.
    ///
    /// # Errors
    ///
    /// Throws `AssetError`/`ParamError` on invalid input.
    #[wasm_bindgen(constructor)]
    pub fn new(
        account_id: AccountIdLike,
        account_currency: &str,
        initial_pnl: PnlLike,
        lower_bound: OptionalPnlLike,
        upper_bound: OptionalPnlLike,
    ) -> Result<JsSpotFundsPnlBoundsAccountBarrier, JsValue> {
        Ok(Self {
            barrier: JsSpotFundsPnlBoundsBarrier::new(account_currency, lower_bound, upper_bound)?,
            account_id: JsAccountId::from_inner(resolve_account_id(account_id.into())?),
            initial_pnl: resolve_pnl(initial_pnl.into())?,
        })
    }

    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsSpotFundsPnlBoundsAccountBarrier {
        self.clone()
    }
}

impl JsSpotFundsPnlBoundsAccountBarrier {
    pub(crate) fn to_core(&self) -> SpotFundsPnlBoundsAccountBarrier {
        SpotFundsPnlBoundsAccountBarrier {
            barrier: self.barrier.to_core(),
            account_id: self.account_id.inner(),
            initial_pnl: self.initial_pnl,
        }
    }
}

/// Runtime account spot-funds P&L-bounds barrier update.
#[wasm_bindgen(js_name = SpotFundsPnlBoundsAccountBarrierUpdate)]
#[derive(Clone)]
pub struct JsSpotFundsPnlBoundsAccountBarrierUpdate {
    barrier: JsSpotFundsPnlBoundsBarrier,
    account_id: JsAccountId,
}

#[wasm_bindgen(js_class = SpotFundsPnlBoundsAccountBarrierUpdate)]
impl JsSpotFundsPnlBoundsAccountBarrierUpdate {
    /// Constructs a runtime account spot-funds P&L-bounds update.
    ///
    /// Unlike `SpotFundsPnlBoundsAccountBarrier`, this type has no
    /// `initialPnl`: runtime updates preserve the live accumulated P&L.
    ///
    /// # Errors
    ///
    /// Throws `AssetError`/`ParamError` on invalid input.
    #[wasm_bindgen(constructor)]
    pub fn new(
        account_id: AccountIdLike,
        account_currency: &str,
        lower_bound: OptionalPnlLike,
        upper_bound: OptionalPnlLike,
    ) -> Result<JsSpotFundsPnlBoundsAccountBarrierUpdate, JsValue> {
        Ok(Self {
            barrier: JsSpotFundsPnlBoundsBarrier::new(account_currency, lower_bound, upper_bound)?,
            account_id: JsAccountId::from_inner(resolve_account_id(account_id.into())?),
        })
    }

    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsSpotFundsPnlBoundsAccountBarrierUpdate {
        self.clone()
    }
}

impl JsSpotFundsPnlBoundsAccountBarrierUpdate {
    pub(crate) fn to_core(&self) -> SpotFundsPnlBoundsAccountBarrierUpdate {
        SpotFundsPnlBoundsAccountBarrierUpdate {
            barrier: self.barrier.to_core(),
            account_id: self.account_id.inner(),
        }
    }
}

/// Resolved market-data configuration for the spot-funds policy.
#[derive(Clone)]
struct MarketDataConfig {
    service: JsMarketDataService,
    default_slippage_bps: u16,
    pricing_source: SpotFundsPricingSource,
    overrides: Vec<JsSpotFundsOverride>,
}

/// Ready-builder token for the builtin spot-funds policy.
#[wasm_bindgen(js_name = SpotFundsBuilder)]
#[derive(Clone)]
pub struct JsSpotFundsBuilder {
    policy_group_id: u16,
    market_data: Option<MarketDataConfig>,
}

#[wasm_bindgen(js_class = SpotFundsBuilder)]
impl JsSpotFundsBuilder {
    /// Stable name registered by the builtin spot-funds policy.
    #[wasm_bindgen(getter, js_name = NAME)]
    pub fn name() -> String {
        SpotFundsPolicy::<EngineLocking, EngineLocking>::NAME.to_owned()
    }

    /// Assigns the policy group id and returns the builder for chaining.
    #[wasm_bindgen(
        js_name = withPolicyGroupId,
        unchecked_return_type = "SpotFundsReadyBuilder"
    )]
    pub fn with_policy_group_id(
        &self,
        policy_group_id: IntegerNumber,
    ) -> Result<JsSpotFundsBuilder, JsValue> {
        let mut next = self.clone();
        next.policy_group_id = crate::lock::parse_policy_group_id(policy_group_id.into())?.value();
        Ok(next)
    }

    /// Enables market orders against `service` and returns the ready builder.
    ///
    /// `defaultSlippageBps` is the global worst-case slippage; `pricingSource`
    /// selects the base price (`"Mark"` default or `"BookTop"`); `overrides`
    /// is an array of `SpotFundsOverride`. Without this call the policy is
    /// limit-only (market orders rejected `UnsupportedOrderType`).
    ///
    /// # Errors
    ///
    /// Throws `ParamError` when `defaultSlippageBps` is out of range or
    /// `pricingSource` is invalid.
    #[wasm_bindgen(
        js_name = marketData,
        unchecked_return_type = "SpotFundsReadyBuilder"
    )]
    pub fn market_data(
        &self,
        service: &JsMarketDataService,
        default_slippage_bps: IntegerNumber,
        pricing_source: SpotFundsPricingSourceLike,
        overrides: SpotFundsOverrideIterable,
    ) -> Result<JsSpotFundsBuilder, JsValue> {
        let default_slippage_bps = parse_bounded_number(
            default_slippage_bps.into(),
            u64::from(u16::MAX),
            "defaultSlippageBps",
        )? as u16;
        let pricing_source = parse_pricing_source(&pricing_source.into())?;
        let overrides = collect_overrides(overrides.into())?;
        let mut next = self.clone();
        next.market_data = Some(MarketDataConfig {
            service: service.clone(),
            default_slippage_bps,
            pricing_source,
            overrides,
        });
        Ok(next)
    }

    /// Returns an independent builder with the same configuration.
    #[wasm_bindgen(js_name = clone, unchecked_return_type = "SpotFundsReadyBuilder")]
    pub fn js_clone(&self) -> JsSpotFundsBuilder {
        self.clone()
    }
}

impl JsSpotFundsBuilder {
    /// Builds the core policy from this token and the engine's storage
    /// builder.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` when the configured slippage is out of range.
    pub(crate) fn build_policy(
        &self,
        storage_builder: &crate::engine::StorageBuilderRef,
    ) -> Result<SpotFundsPolicy<EngineLocking, EngineLocking>, JsValue> {
        let market_orders: Option<MarketDataBundle> = self
            .market_data
            .as_ref()
            .map(|config| SpotFundsMarketData::<EngineLocking>::new(config.service.handle()));
        let settings = match self.market_data.as_ref() {
            Some(config) => SpotFundsSettings::new(
                config.default_slippage_bps,
                config.pricing_source,
                config
                    .overrides
                    .iter()
                    .map(JsSpotFundsOverride::to_core_entry),
            )
            .map_err(|error| make_error(ErrorKind::Param, &error.to_string(), Some("Other")))?,
            None => SpotFundsSettings::new(0, SpotFundsPricingSource::Mark, std::iter::empty())
                .map_err(|error| make_error(ErrorKind::Param, &error.to_string(), Some("Other")))?,
        };
        Ok(SpotFundsPolicy::<EngineLocking, EngineLocking>::new(
            settings,
            market_orders,
            storage_builder,
        )
        .with_policy_group_id(PolicyGroupId::new(self.policy_group_id)))
    }
}

/// Configuring builder for the builtin spot-funds P&L-bounds axis.
#[wasm_bindgen(js_name = SpotFundsPnlBoundsKillswitchBuilder)]
#[derive(Clone, Default)]
pub struct JsSpotFundsPnlBoundsKillswitchBuilder {
    policy_group_id: u16,
    market_data: Option<JsMarketDataService>,
    global_barriers: Vec<JsSpotFundsPnlBoundsBarrier>,
    account_group_barriers: Vec<JsSpotFundsPnlBoundsAccountGroupBarrier>,
    account_barriers: Vec<JsSpotFundsPnlBoundsAccountBarrier>,
}

#[wasm_bindgen(js_class = SpotFundsPnlBoundsKillswitchBuilder)]
impl JsSpotFundsPnlBoundsKillswitchBuilder {
    /// Stable name registered by the builtin spot-funds policy.
    #[wasm_bindgen(getter, js_name = NAME)]
    pub fn name() -> String {
        SpotFundsPolicy::<EngineLocking, EngineLocking>::NAME.to_owned()
    }

    /// Adds global P&L barriers and returns the builder for chaining.
    #[wasm_bindgen(
        js_name = globalBarriers,
        unchecked_return_type = "SpotFundsPnlBoundsKillswitchReadyBuilder"
    )]
    pub fn global_barriers(
        &self,
        #[wasm_bindgen(unchecked_param_type = "Iterable<SpotFundsPnlBoundsBarrier>")]
        barriers: JsValue,
    ) -> Result<JsSpotFundsPnlBoundsKillswitchBuilder, JsValue> {
        let barriers: Vec<JsSpotFundsPnlBoundsBarrier> =
            collect_cloned_wrappers(&barriers, "globalBarriers")?;
        let mut next = self.clone();
        next.global_barriers.extend(barriers);
        Ok(next)
    }

    /// Adds account-group P&L barriers and returns the builder for chaining.
    #[wasm_bindgen(
        js_name = accountGroupBarriers,
        unchecked_return_type = "SpotFundsPnlBoundsKillswitchReadyBuilder"
    )]
    pub fn account_group_barriers(
        &self,
        #[wasm_bindgen(unchecked_param_type = "Iterable<SpotFundsPnlBoundsAccountGroupBarrier>")]
        barriers: JsValue,
    ) -> Result<JsSpotFundsPnlBoundsKillswitchBuilder, JsValue> {
        let barriers: Vec<JsSpotFundsPnlBoundsAccountGroupBarrier> =
            collect_cloned_wrappers(&barriers, "accountGroupBarriers")?;
        let mut next = self.clone();
        next.account_group_barriers.extend(barriers);
        Ok(next)
    }

    /// Adds build-time account P&L barriers and returns the builder.
    #[wasm_bindgen(
        js_name = accountBarriers,
        unchecked_return_type = "SpotFundsPnlBoundsKillswitchReadyBuilder"
    )]
    pub fn account_barriers(
        &self,
        #[wasm_bindgen(unchecked_param_type = "Iterable<SpotFundsPnlBoundsAccountBarrier>")]
        barriers: JsValue,
    ) -> Result<JsSpotFundsPnlBoundsKillswitchBuilder, JsValue> {
        let barriers: Vec<JsSpotFundsPnlBoundsAccountBarrier> =
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
    ) -> Result<JsSpotFundsPnlBoundsKillswitchBuilder, JsValue> {
        let mut next = self.clone();
        next.policy_group_id = crate::lock::parse_policy_group_id(policy_group_id.into())?.value();
        Ok(next)
    }

    /// Enables market-order valuation against `service` and returns the
    /// builder for chaining.
    ///
    /// The P&L-bounds axis prices market orders with the mark price and no
    /// slippage; it remains in track-only limit mode.
    #[wasm_bindgen(js_name = marketData)]
    pub fn market_data(
        &self,
        service: &JsMarketDataService,
    ) -> JsSpotFundsPnlBoundsKillswitchBuilder {
        let mut next = self.clone();
        next.market_data = Some(service.clone());
        next
    }

    /// Returns an independent builder with the same configuration.
    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsSpotFundsPnlBoundsKillswitchBuilder {
        self.clone()
    }
}

impl JsSpotFundsPnlBoundsKillswitchBuilder {
    pub(crate) fn build_policy(
        &self,
        storage_builder: &crate::engine::StorageBuilderRef,
    ) -> Result<SpotFundsPolicy<EngineLocking, EngineLocking>, JsValue> {
        let market_orders: Option<MarketDataBundle> = self
            .market_data
            .as_ref()
            .map(|service| SpotFundsMarketData::<EngineLocking>::new(service.handle()));
        let mut settings = SpotFundsSettings::new(
            0,
            SpotFundsPricingSource::Mark,
            std::iter::empty::<(SpotFundsOverrideTarget, SpotFundsOverride)>(),
        )
        .map_err(|error| engine_build_configuration_error(&error.to_string()))?;
        settings.set_global_limit_mode(SpotFundsLimitMode::TrackOnly);
        let settings = settings
            .with_pnl_barriers(
                self.global_barriers
                    .iter()
                    .map(JsSpotFundsPnlBoundsBarrier::to_core),
                self.account_group_barriers
                    .iter()
                    .map(JsSpotFundsPnlBoundsAccountGroupBarrier::to_core),
                self.account_barriers
                    .iter()
                    .map(JsSpotFundsPnlBoundsAccountBarrier::to_core),
            )
            .map_err(|error| engine_build_configuration_error(&error.to_string()))?;

        Ok(SpotFundsPolicy::<EngineLocking, EngineLocking>::new(
            settings,
            market_orders,
            storage_builder,
        )
        .with_policy_group_id(PolicyGroupId::new(self.policy_group_id)))
    }
}

/// Collects an iterable of `SpotFundsOverride` into a vector.
///
/// An omitted/`null`/`undefined` argument yields an empty list.
///
/// # Errors
///
/// Throws `ParamError` when the argument is not iterable or an element is not a
/// `SpotFundsOverride`.
fn collect_overrides(overrides: JsValue) -> Result<Vec<JsSpotFundsOverride>, JsValue> {
    if overrides.is_undefined() || overrides.is_null() {
        return Ok(Vec::new());
    }
    let iterator = js_sys::try_iter(&overrides)?.ok_or_else(overrides_error)?;
    let mut result = Vec::new();
    for item in iterator {
        let item = item?;
        let entry =
            extract_cloned_wrapper::<JsSpotFundsOverride>(&item)?.ok_or_else(overrides_error)?;
        result.push(entry);
    }
    Ok(result)
}

/// Builds the error raised when `overrides` is not an iterable of overrides.
fn overrides_error() -> JsValue {
    make_error(
        ErrorKind::Type,
        "overrides must be an iterable of SpotFundsOverride",
        None,
    )
}

/// Creates a fresh spot-funds ready-builder token (limit-only by default).
#[wasm_bindgen(
    js_name = buildSpotFunds,
    unchecked_return_type = "SpotFundsReadyBuilder"
)]
pub fn build_spot_funds() -> JsSpotFundsBuilder {
    JsSpotFundsBuilder {
        policy_group_id: 0,
        market_data: None,
    }
}

/// Creates a fresh spot-funds P&L-bounds configuring builder.
#[wasm_bindgen(js_name = buildSpotFundsPnlBoundsKillswitch)]
pub fn build_spot_funds_pnl_bounds_killswitch() -> JsSpotFundsPnlBoundsKillswitchBuilder {
    JsSpotFundsPnlBoundsKillswitchBuilder::default()
}
