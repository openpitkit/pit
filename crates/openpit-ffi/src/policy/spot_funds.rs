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

#![allow(clippy::missing_safety_doc, clippy::not_unsafe_ptr_arg_deref)]

use openpit::param::{AccountGroupId, AccountId, Pnl};
use openpit::pretrade::policies::{
    SpotFundsPnlBoundsAccountBarrier, SpotFundsPnlBoundsAccountBarrierUpdate,
    SpotFundsPnlBoundsAccountGroupBarrier, SpotFundsPnlBoundsBarrier, SpotFundsPolicy,
    SpotFundsSettings,
};
use openpit::pretrade::SpotFundsLimitMode;
use openpit::{
    InstrumentId, PolicyGroupId, SpotFundsConfigError, SpotFundsMarketData, SpotFundsOverride,
    SpotFundsOverrideTarget, SpotFundsPricingSource,
};
use openpit_interop::{EngineLocking, SyncMode};

use crate::account_group_id::OpenPitParamAccountGroupId;
use crate::engine::{write_configure_error, OpenPitConfigureError};
use crate::marketdata::{OpenPitMarketDataInstrumentId, OpenPitMarketDataService};
use crate::param::{OpenPitParamAccountId, OpenPitParamPnl, OpenPitParamPnlOptional};

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
/// Selects how the spot-funds control reacts to insufficient available funds.
///
/// The default is `Enforce`, matching the core
/// [`SpotFundsLimitMode`] default.
pub enum OpenPitPretradePoliciesSpotFundsLimitMode {
    /// Reject a reservation when available funds are insufficient; the
    /// reservation is not recorded.
    #[default]
    Enforce = 0,
    /// Always record the reservation; `available` may go negative and a
    /// shortfall never rejects. Arithmetic overflow is still surfaced.
    TrackOnly = 1,
}

pub(crate) fn import_spot_funds_limit_mode(
    value: u8,
) -> Result<SpotFundsLimitMode, OpenPitConfigureError> {
    match value {
        0 => Ok(SpotFundsLimitMode::Enforce),
        1 => Ok(SpotFundsLimitMode::TrackOnly),
        other => Err(OpenPitConfigureError::validation(format!(
            "spot funds limit_mode must be 0 (Enforce) or 1 (TrackOnly), got {other}"
        ))),
    }
}

use super::*;

/// Maps the `u8` pricing-source contract to the core enum for a configure
/// function, returning an [`OpenPitConfigureError`] on an invalid selector.
fn configure_pricing_source(value: u8) -> Result<SpotFundsPricingSource, OpenPitConfigureError> {
    match value {
        0 => Ok(SpotFundsPricingSource::Mark),
        1 => Ok(SpotFundsPricingSource::BookTop),
        other => Err(OpenPitConfigureError::validation(format!(
            "pricing_source must be 0 (Mark) or 1 (BookTop), got {other}"
        ))),
    }
}

fn configure_spot_funds_limit_mode(
    value: u8,
    out_error: *mut *mut OpenPitConfigureError,
) -> Option<SpotFundsLimitMode> {
    match import_spot_funds_limit_mode(value) {
        Ok(mode) => Some(mode),
        Err(err) => {
            write_configure_error(out_error, err);
            None
        }
    }
}

fn parse_configure_optional_pnl(
    bound: OpenPitParamPnlOptional,
    label: &str,
    index: usize,
    field: &str,
) -> Result<Option<Pnl>, OpenPitConfigureError> {
    if !bound.is_set {
        return Ok(None);
    }
    bound.value.to_param().map(Some).map_err(|e| {
        OpenPitConfigureError::validation(format!("{label}[{index}] {field} is invalid: {e}"))
    })
}

/// Tagged target variants for a spot-funds slippage override.
///
/// Spot funds overrides use an explicit tagged hierarchy matching the Rust
/// [`SpotFundsOverrideTarget`](openpit::SpotFundsOverrideTarget) variants:
/// `Instrument`, `InstrumentAccount`, and `InstrumentAccountGroup`.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenPitPretradePoliciesSpotFundsOverrideTargetTag {
    /// Instrument-level override.
    Instrument = 0,
    /// Override for one instrument and account.
    InstrumentAccount = 1,
    /// Override for one instrument and account group.
    InstrumentAccountGroup = 2,
}

/// Payload for an instrument-level spot-funds override target.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct OpenPitPretradePoliciesSpotFundsOverrideTargetInstrument {
    /// Registered market-data instrument id.
    pub instrument_id: OpenPitMarketDataInstrumentId,
}

/// Payload for an instrument-and-account spot-funds override target.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct OpenPitPretradePoliciesSpotFundsOverrideTargetInstrumentAccount {
    /// Registered market-data instrument id.
    pub instrument_id: OpenPitMarketDataInstrumentId,
    /// Account the override applies to.
    pub account_id: OpenPitParamAccountId,
}

/// Payload for an instrument-and-account-group spot-funds override target.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct OpenPitPretradePoliciesSpotFundsOverrideTargetInstrumentAccountGroup {
    /// Registered market-data instrument id.
    pub instrument_id: OpenPitMarketDataInstrumentId,
    /// Account group the override applies to.
    pub account_group_id: OpenPitParamAccountGroupId,
}

/// Variant payload for a tagged spot-funds override target.
#[repr(C)]
#[derive(Clone, Copy)]
pub union OpenPitPretradePoliciesSpotFundsOverrideTargetPayload {
    /// Payload used with the `Instrument` tag.
    pub instrument: OpenPitPretradePoliciesSpotFundsOverrideTargetInstrument,
    /// Payload used with the `InstrumentAccount` tag.
    pub instrument_account: OpenPitPretradePoliciesSpotFundsOverrideTargetInstrumentAccount,
    /// Payload used with the `InstrumentAccountGroup` tag.
    pub instrument_account_group:
        OpenPitPretradePoliciesSpotFundsOverrideTargetInstrumentAccountGroup,
}

/// Explicit tagged target for a spot-funds slippage override.
///
/// The `tag` selects exactly one union payload. Unknown tags are rejected
/// through the function's existing error channel before the payload is read.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct OpenPitPretradePoliciesSpotFundsOverrideTarget {
    /// One of [`OpenPitPretradePoliciesSpotFundsOverrideTargetTag`].
    ///
    /// Stored as `u8` so unknown C values can be rejected without constructing
    /// an invalid Rust enum discriminant.
    pub tag: u8,
    /// Payload selected by `tag`.
    pub payload: OpenPitPretradePoliciesSpotFundsOverrideTargetPayload,
}

/// Slippage override entry for the spot funds policy.
///
/// `target` mirrors the three variants of
/// [`SpotFundsOverrideTarget`](openpit::SpotFundsOverrideTarget). When
/// `has_slippage_bps` is `true`, `slippage_bps` is used for the selected
/// target. When it is `false`, construction ignores the entry and runtime
/// configuration clears the selected override. Slippage resolves account ->
/// account group -> instrument -> global for each order.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct OpenPitPretradePoliciesSpotFundsOverride {
    /// Explicit tagged override target.
    pub target: OpenPitPretradePoliciesSpotFundsOverrideTarget,
    /// Slippage in basis points, used only when `has_slippage_bps` is `true`.
    pub slippage_bps: u16,
    /// Whether `slippage_bps` carries a value.
    pub has_slippage_bps: bool,
}

/// Spot-funds account-currency P&L bounds.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpenPitPretradePoliciesSpotFundsPnlBoundsBarrier {
    /// Account currency whose accumulated P&L is monitored.
    pub account_currency: OpenPitStringView,
    /// Optional lower bound for accumulated P&L.
    pub lower_bound: OpenPitParamPnlOptional,
    /// Optional upper bound for accumulated P&L.
    pub upper_bound: OpenPitParamPnlOptional,
}

/// Account-group spot-funds P&L bounds refinement.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpenPitPretradePoliciesSpotFundsPnlBoundsAccountGroupBarrier {
    /// Account group the barrier applies to.
    pub account_group_id: OpenPitParamAccountGroupId,
    /// Account currency and bounds for this group.
    pub barrier: OpenPitPretradePoliciesSpotFundsPnlBoundsBarrier,
}

/// Account spot-funds P&L bounds refinement with construction-time seed.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpenPitPretradePoliciesSpotFundsPnlBoundsAccountBarrier {
    /// Account the barrier applies to.
    pub account_id: OpenPitParamAccountId,
    /// Account currency and bounds for this account.
    pub barrier: OpenPitPretradePoliciesSpotFundsPnlBoundsBarrier,
    /// Initial accumulated P&L, consumed only while adding the policy.
    pub initial_pnl: OpenPitParamPnl,
}

/// Runtime account spot-funds P&L bounds replacement.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpenPitPretradePoliciesSpotFundsPnlBoundsAccountBarrierUpdate {
    /// Account the barrier applies to.
    pub account_id: OpenPitParamAccountId,
    /// Account currency and replacement bounds for this account.
    pub barrier: OpenPitPretradePoliciesSpotFundsPnlBoundsBarrier,
}

fn parse_pnl_barrier_or_error(
    entry: &OpenPitPretradePoliciesSpotFundsPnlBoundsBarrier,
    label: &str,
    index: usize,
    out_error: OpenPitOutError,
) -> Option<SpotFundsPnlBoundsBarrier> {
    let account_currency = parse_asset_or_error(
        entry.account_currency,
        label,
        index,
        "account_currency",
        out_error,
    )?;
    let lower_bound = match parse_optional_pnl_or_error(
        entry.lower_bound,
        label,
        index,
        "lower_bound",
        out_error,
    ) {
        Ok(v) => v,
        Err(()) => return None,
    };
    let upper_bound = match parse_optional_pnl_or_error(
        entry.upper_bound,
        label,
        index,
        "upper_bound",
        out_error,
    ) {
        Ok(v) => v,
        Err(()) => return None,
    };
    Some(SpotFundsPnlBoundsBarrier {
        account_currency,
        lower_bound,
        upper_bound,
    })
}

fn parse_configure_pnl_barrier(
    entry: &OpenPitPretradePoliciesSpotFundsPnlBoundsBarrier,
    label: &str,
    index: usize,
) -> Result<SpotFundsPnlBoundsBarrier, OpenPitConfigureError> {
    let account_currency =
        parse_configure_asset(entry.account_currency, label, index, "account_currency")?;
    let lower_bound = parse_configure_optional_pnl(entry.lower_bound, label, index, "lower_bound")?;
    let upper_bound = parse_configure_optional_pnl(entry.upper_bound, label, index, "upper_bound")?;
    Ok(SpotFundsPnlBoundsBarrier {
        account_currency,
        lower_bound,
        upper_bound,
    })
}

fn override_target(
    entry: &OpenPitPretradePoliciesSpotFundsOverride,
) -> Result<SpotFundsOverrideTarget, String> {
    let tag = entry.target.tag;
    if tag == OpenPitPretradePoliciesSpotFundsOverrideTargetTag::Instrument as u8 {
        let payload = unsafe { entry.target.payload.instrument };
        return Ok(SpotFundsOverrideTarget::Instrument(InstrumentId::new(
            payload.instrument_id,
        )));
    }
    if tag == OpenPitPretradePoliciesSpotFundsOverrideTargetTag::InstrumentAccount as u8 {
        let payload = unsafe { entry.target.payload.instrument_account };
        return Ok(SpotFundsOverrideTarget::InstrumentAccount(
            InstrumentId::new(payload.instrument_id),
            AccountId::from_u64(payload.account_id),
        ));
    }
    if tag == OpenPitPretradePoliciesSpotFundsOverrideTargetTag::InstrumentAccountGroup as u8 {
        let payload = unsafe { entry.target.payload.instrument_account_group };
        let account_group_id = AccountGroupId::from_u32(payload.account_group_id).map_err(|e| {
            format!(
                "spot funds override account group id {} is invalid: {e}",
                payload.account_group_id
            )
        })?;
        return Ok(SpotFundsOverrideTarget::InstrumentAccountGroup(
            InstrumentId::new(payload.instrument_id),
            account_group_id,
        ));
    }
    Err(format!("spot funds override target tag {tag} is invalid"))
}

/// Pricing source selector for the spot funds policy.
///
/// Mirrors the `u8` contract: `0` = Mark, `1` = BookTop.
fn import_pricing_source(value: u8, out_error: OpenPitOutError) -> Option<SpotFundsPricingSource> {
    match value {
        0 => Some(SpotFundsPricingSource::Mark),
        1 => Some(SpotFundsPricingSource::BookTop),
        other => {
            write_error_format!(
                out_error,
                "pricing_source must be 0 (Mark) or 1 (BookTop), got {}",
                other
            );
            None
        }
    }
}

#[no_mangle]
/// Adds the built-in spot funds policy to the engine builder.
///
/// Contract:
/// - `builder` must be a valid engine builder pointer.
/// - `market_data` is a borrowed market-data service handle or null. Null
///   disables market orders entirely (limit-only mode): they are rejected
///   with `UnsupportedOrderType`. A non-null handle enables market orders;
///   the policy reads live quotes from the supplied market-data service.
/// - `market_slippage_bps` is a pointer to a `u16` or null. When
///   `market_data` is non-null it MUST be non-null too (otherwise this is a
///   configuration error and the call fails). The value is the worst-case
///   global slippage in basis points (1 bps = 0.01%). Range validation is
///   performed by the core engine.
/// - `pricing_source` selects the base price: `0` = Mark, `1` = BookTop.
/// - `instrument_overrides` / `overrides_len` describe a contiguous array of
///   slippage overrides; pass null + 0 for none. Each entry uses an explicit
///   tagged target matching `Instrument`, `InstrumentAccount`, or
///   `InstrumentAccountGroup`. An unknown tag fails the call. An entry with
///   `has_slippage_bps == false` is ignored. Slippage resolves
///   account -> account group -> instrument -> global per order.
/// - `policy_group_id` tags the policy instance.
///
/// Mismatch guard: when `market_data` is non-null and the engine is
/// multi-threaded (`Full` or `Account` sync mode) but the market-data service
/// was built in no-sync (`None`, no-op locks) mode, this call fails with a
/// descriptive error. A no-sync engine accepts both no-sync and full-sync MD
/// services.
///
/// Success: returns `true`; the builder retains the policy.
///
/// Error: returns `false`. If `out_error` is non-null, writes a
/// caller-owned `OpenPitSharedString` error handle (release with
/// `openpit_destroy_shared_string`).
pub unsafe extern "C" fn openpit_engine_builder_add_builtin_spot_funds_policy(
    builder: *mut crate::engine::OpenPitEngineBuilder,
    market_data: *const OpenPitMarketDataService,
    market_slippage_bps: *const u16,
    pricing_source: u8,
    instrument_overrides: *const OpenPitPretradePoliciesSpotFundsOverride,
    overrides_len: usize,
    policy_group_id: u16,
    out_error: OpenPitOutError,
) -> bool {
    if builder.is_null() {
        write_error(out_error, "engine builder is null");
        return false;
    }

    // The slippage / pricing-source / override cascade now lives in
    // `SpotFundsSettings`; `SpotFundsMarketData` carries only the service
    // handle. Build both here: the settings are always required, while the
    // market-data handle is `Some` only when a service is supplied (market
    // orders enabled). In limit-only mode the slippage cascade is inert, so a
    // default settings instance is used and the slippage/pricing/override
    // arguments are not consulted.
    let (market_orders, settings): (
        Option<SpotFundsMarketData<EngineLocking>>,
        SpotFundsSettings,
    ) = if market_data.is_null() {
        let settings =
            match SpotFundsSettings::new(0, SpotFundsPricingSource::Mark, std::iter::empty()) {
                Ok(v) => v,
                Err(e) => {
                    write_error_format!(out_error, "spot funds settings build failed: {}", e);
                    return false;
                }
            };
        (None, settings)
    } else {
        let svc = unsafe { &*market_data };

        // Mismatch guard: a multi-threaded engine requires a fully-locked MD
        // service. A no-sync MD service has no-op internal locks and is
        // unsound under concurrent access from a Full/Account engine.
        let engine_sync_mode = unsafe { &*builder }.sync_mode;
        if matches!(engine_sync_mode, SyncMode::Full | SyncMode::Account)
            && svc.mode == SyncMode::None
        {
            write_error(
                out_error,
                "market data service is no-sync (None) but the engine is multi-threaded; \
                 rebuild the market-data service with full_sync",
            );
            return false;
        }

        // A real service is provided: slippage is required.
        if market_slippage_bps.is_null() {
            write_error(
                out_error,
                "market_slippage_bps is required when market_data is provided",
            );
            return false;
        }
        let bps = unsafe { *market_slippage_bps };

        let source = match import_pricing_source(pricing_source, out_error) {
            Some(v) => v,
            None => return false,
        };

        let overrides_slice = match unsafe {
            try_slice_arg(
                instrument_overrides,
                overrides_len,
                "spot_funds_policy instrument_overrides",
                out_error,
            )
        } {
            Some(v) => v,
            None => return false,
        };
        let mut overrides: Vec<(SpotFundsOverrideTarget, SpotFundsOverride)> =
            Vec::with_capacity(overrides_slice.len());
        for entry in overrides_slice {
            let target = match override_target(entry) {
                Ok(target) => target,
                Err(error) => {
                    write_error(out_error, &error);
                    return false;
                }
            };
            overrides.push((
                target,
                SpotFundsOverride {
                    slippage_bps: entry.has_slippage_bps.then_some(entry.slippage_bps),
                },
            ));
        }

        let settings = match SpotFundsSettings::new(bps, source, overrides) {
            Ok(v) => v,
            Err(e) => {
                write_error_format!(out_error, "spot funds settings build failed: {}", e);
                return false;
            }
        };
        let handle = svc.handle_clone();
        (
            Some(SpotFundsMarketData::<EngineLocking>::new(handle)),
            settings,
        )
    };

    let builder_ref = unsafe { &mut *builder };
    let storage_builder = match policy_storage(builder_ref) {
        Some(s) => s,
        None => {
            write_error(out_error, "engine builder is no longer available");
            return false;
        }
    };
    let policy = SpotFundsPolicy::<EngineLocking, EngineLocking>::new(
        settings,
        market_orders,
        storage_builder,
    )
    .with_policy_group_id(PolicyGroupId::new(policy_group_id));
    match crate::engine::add_pre_trade_policy_to_builder(builder_ref, policy) {
        Ok(()) => true,
        Err(err) => {
            write_error(out_error, &err);
            false
        }
    }
}

#[no_mangle]
/// Adds the built-in spot-funds policy with account-currency P&L bounds.
///
/// This entry point builds the regular `SpotFundsPolicy` and configures only
/// its P&L-bounds axis. The policy keeps its stable built-in name
/// `"SpotFundsPolicy"`; no separate policy namespace is created.
/// It seeds the funds-limit axis as `TrackOnly` and market pricing as
/// `Mark` / 0 bps / no overrides; tune those regular spot-funds knobs after
/// build with `openpit_engine_configure_spot_funds`.
///
/// Contract:
/// - `builder` must be a valid engine builder pointer.
/// - `market_data` is a borrowed market-data service handle or null. A null
///   handle is accepted, but any controlled account that later needs FX to
///   compute P&L will be blocked by the core policy fail-safe.
/// - At least one barrier must be provided across `global`,
///   `account_group`, or `account`.
/// - Account barriers include construction-time `initial_pnl`. Runtime
///   configuration uses the update DTO without `initial_pnl` and preserves the
///   live accumulator.
///
/// Success / error: mirrors
/// `openpit_engine_builder_add_builtin_spot_funds_policy`.
pub unsafe extern "C" fn openpit_engine_builder_add_builtin_spot_funds_pnl_bounds_killswitch_policy(
    builder: *mut crate::engine::OpenPitEngineBuilder,
    market_data: *const OpenPitMarketDataService,
    policy_group_id: u16,
    global: *const OpenPitPretradePoliciesSpotFundsPnlBoundsBarrier,
    global_len: usize,
    account_group: *const OpenPitPretradePoliciesSpotFundsPnlBoundsAccountGroupBarrier,
    account_group_len: usize,
    account: *const OpenPitPretradePoliciesSpotFundsPnlBoundsAccountBarrier,
    account_len: usize,
    out_error: OpenPitOutError,
) -> bool {
    if builder.is_null() {
        write_error(out_error, "engine builder is null");
        return false;
    }
    let global_slice = match unsafe {
        try_slice_arg(
            global,
            global_len,
            "spot_funds_pnl_bounds_policy global",
            out_error,
        )
    } {
        Some(v) => v,
        None => return false,
    };
    let mut global_barriers = Vec::with_capacity(global_slice.len());
    for (index, entry) in global_slice.iter().enumerate() {
        let barrier = match parse_pnl_barrier_or_error(entry, "global", index, out_error) {
            Some(v) => v,
            None => return false,
        };
        global_barriers.push(barrier);
    }

    let account_group_slice = match unsafe {
        try_slice_arg(
            account_group,
            account_group_len,
            "spot_funds_pnl_bounds_policy account_group",
            out_error,
        )
    } {
        Some(v) => v,
        None => return false,
    };
    let mut account_group_barriers = Vec::with_capacity(account_group_slice.len());
    for (index, entry) in account_group_slice.iter().enumerate() {
        let account_group_id = match AccountGroupId::from_u32(entry.account_group_id) {
            Ok(v) => v,
            Err(e) => {
                write_error_format!(
                    out_error,
                    "account_group[{index}] account_group_id {} is invalid: {e}",
                    entry.account_group_id
                );
                return false;
            }
        };
        let barrier =
            match parse_pnl_barrier_or_error(&entry.barrier, "account_group", index, out_error) {
                Some(v) => v,
                None => return false,
            };
        account_group_barriers.push(SpotFundsPnlBoundsAccountGroupBarrier {
            barrier,
            account_group_id,
        });
    }

    let account_slice = match unsafe {
        try_slice_arg(
            account,
            account_len,
            "spot_funds_pnl_bounds_policy account",
            out_error,
        )
    } {
        Some(v) => v,
        None => return false,
    };
    let mut account_barriers = Vec::with_capacity(account_slice.len());
    for (index, entry) in account_slice.iter().enumerate() {
        let barrier = match parse_pnl_barrier_or_error(&entry.barrier, "account", index, out_error)
        {
            Some(v) => v,
            None => return false,
        };
        let initial_pnl = match entry.initial_pnl.to_param() {
            Ok(v) => v,
            Err(e) => {
                write_error_format!(
                    out_error,
                    "account[{}] initial_pnl is invalid: {}",
                    index,
                    e
                );
                return false;
            }
        };
        account_barriers.push(SpotFundsPnlBoundsAccountBarrier {
            barrier,
            account_id: AccountId::from_u64(entry.account_id),
            initial_pnl,
        });
    }

    let market_orders = if market_data.is_null() {
        None
    } else {
        let svc = unsafe { &*market_data };
        let engine_sync_mode = unsafe { &*builder }.sync_mode;
        if matches!(engine_sync_mode, SyncMode::Full | SyncMode::Account)
            && svc.mode == SyncMode::None
        {
            write_error(
                out_error,
                "market data service is no-sync (None) but the engine is multi-threaded; \
                 rebuild the market-data service with full_sync",
            );
            return false;
        }
        Some(SpotFundsMarketData::<EngineLocking>::new(
            svc.handle_clone(),
        ))
    };

    let mut settings =
        match SpotFundsSettings::new(0, SpotFundsPricingSource::Mark, std::iter::empty()) {
            Ok(v) => v,
            Err(e) => {
                write_error_format!(out_error, "spot funds settings build failed: {}", e);
                return false;
            }
        };
    settings.set_global_limit_mode(SpotFundsLimitMode::TrackOnly);
    let settings =
        match settings.with_pnl_barriers(global_barriers, account_group_barriers, account_barriers)
        {
            Ok(settings) => settings,
            Err(e) => {
                write_error_format!(out_error, "spot funds pnl barriers invalid: {}", e);
                return false;
            }
        };

    let builder_ref = unsafe { &mut *builder };
    let storage_builder = match policy_storage(builder_ref) {
        Some(s) => s,
        None => {
            write_error(out_error, "engine builder is no longer available");
            return false;
        }
    };
    let policy = SpotFundsPolicy::<EngineLocking, EngineLocking>::new(
        settings,
        market_orders,
        storage_builder,
    )
    .with_policy_group_id(PolicyGroupId::new(policy_group_id));
    match crate::engine::add_pre_trade_policy_to_builder(builder_ref, policy) {
        Ok(()) => true,
        Err(err) => {
            write_error(out_error, &err);
            false
        }
    }
}

#[no_mangle]
/// Retunes the built-in spot-funds policy registered under `name`.
///
/// This is a partial update (PATCH): the global slippage, pricing source, and
/// each supplied override are applied only when their corresponding `has_*`
/// flag is `true`. The market-data service handle is fixed at build time and
/// cannot be changed here; this function only tunes the slippage / pricing
/// cascade that lives in the settings cell.
///
/// Contract:
/// - `engine` must be a valid non-null engine pointer.
/// - `name` selects the policy; it is interpreted as UTF-8. A built-in
///   policy added via `openpit_engine_builder_add_builtin_spot_funds_policy`
///   registers under its fixed name `"SpotFundsPolicy"`, so pass that string
///   here.
/// - When `has_global_slippage_bps` is `true`, the global slippage is set to
///   `global_slippage_bps`.
/// - When `has_pricing_source` is `true`, the pricing source is set from
///   `pricing_source` (`0` = Mark, `1` = BookTop).
/// - When `has_overrides` is `true`, each of the `overrides_len` entries at
///   `instrument_overrides` is applied via insert-or-clear: an entry with
///   `has_slippage_bps == false` clears any override at its explicit tagged
///   target. Unknown target tags fail the call.
/// - A `has_*` flag set to `false` leaves that dimension untouched.
///
/// Success:
/// - returns `true`; the new cascade applies from the next market order onward.
///
/// Error:
/// - returns `false`; if `out_error` is non-null, writes a caller-owned
///   `OpenPitConfigureError` (release with `openpit_destroy_configure_error`).
/// - a null `engine` returns `false` and, when `out_error` is non-null, writes
///   a caller-owned `OpenPitConfigureError` (`Validation`) that must be released
///   with `openpit_destroy_configure_error`.
pub unsafe extern "C" fn openpit_engine_configure_spot_funds(
    engine: *mut crate::engine::OpenPitEngine,
    name: OpenPitStringView,
    global_slippage_bps: u16,
    has_global_slippage_bps: bool,
    pricing_source: u8,
    has_pricing_source: bool,
    instrument_overrides: *const OpenPitPretradePoliciesSpotFundsOverride,
    overrides_len: usize,
    has_overrides: bool,
    out_error: *mut *mut OpenPitConfigureError,
) -> bool {
    if engine.is_null() {
        write_configure_error(
            out_error,
            OpenPitConfigureError::validation("engine is null".to_owned()),
        );
        return false;
    }
    let name = match unsafe { cstr_arg(name) } {
        Some(name) => name,
        None => {
            write_configure_error(
                out_error,
                OpenPitConfigureError::validation(
                    "policy name is null or invalid UTF-8".to_owned(),
                ),
            );
            return false;
        }
    };

    let source = if has_pricing_source {
        match configure_pricing_source(pricing_source) {
            Ok(v) => Some(v),
            Err(e) => {
                write_configure_error(out_error, e);
                return false;
            }
        }
    } else {
        None
    };

    let overrides: Vec<(SpotFundsOverrideTarget, SpotFundsOverride)> = if has_overrides {
        let slice = match unsafe {
            try_slice_arg(
                instrument_overrides,
                overrides_len,
                "spot_funds instrument_overrides",
                std::ptr::null_mut(),
            )
        } {
            Some(v) => v,
            None => {
                write_configure_error(
                    out_error,
                    OpenPitConfigureError::validation(
                        "spot_funds instrument_overrides is null".to_owned(),
                    ),
                );
                return false;
            }
        };
        let mut out = Vec::with_capacity(slice.len());
        for entry in slice {
            let target = match override_target(entry) {
                Ok(target) => target,
                Err(error) => {
                    write_configure_error(out_error, OpenPitConfigureError::validation(error));
                    return false;
                }
            };
            out.push((
                target,
                SpotFundsOverride {
                    slippage_bps: entry.has_slippage_bps.then_some(entry.slippage_bps),
                },
            ));
        }
        out
    } else {
        Vec::new()
    };

    let result = unsafe { &*engine }.configurator().spot_funds(
        &name,
        |settings| -> Result<(), SpotFundsConfigError> {
            if has_global_slippage_bps {
                settings.set_global_slippage_bps(global_slippage_bps)?;
            }
            if let Some(source) = source {
                settings.set_pricing_source(source);
            }
            for (target, ovr) in &overrides {
                settings.set_override(*target, *ovr)?;
            }
            Ok(())
        },
    );
    match result {
        Ok(()) => true,
        Err(err) => {
            write_configure_error(out_error, OpenPitConfigureError::new(err));
            false
        }
    }
}

#[no_mangle]
/// Retunes the P&L-bounds axis of the built-in spot-funds policy registered
/// under `name`.
///
/// This is a partial update (PATCH): each supplied axis is replaced only when
/// its `has_*` flag is true. Account barriers use the runtime update DTO with
/// no `initial_pnl`; live accumulated P&L is preserved.
pub unsafe extern "C" fn openpit_engine_configure_spot_funds_pnl_bounds_killswitch(
    engine: *mut crate::engine::OpenPitEngine,
    name: OpenPitStringView,
    global: *const OpenPitPretradePoliciesSpotFundsPnlBoundsBarrier,
    global_len: usize,
    has_global: bool,
    account_group: *const OpenPitPretradePoliciesSpotFundsPnlBoundsAccountGroupBarrier,
    account_group_len: usize,
    has_account_group: bool,
    account: *const OpenPitPretradePoliciesSpotFundsPnlBoundsAccountBarrierUpdate,
    account_len: usize,
    has_account: bool,
    out_error: *mut *mut OpenPitConfigureError,
) -> bool {
    let name = match unsafe { configure_spot_funds_name(engine, name, out_error) } {
        Some(name) => name,
        None => return false,
    };

    let global_barriers: Vec<SpotFundsPnlBoundsBarrier> = if has_global {
        let slice = match unsafe {
            try_slice_arg(
                global,
                global_len,
                "spot_funds_pnl_bounds global",
                std::ptr::null_mut(),
            )
        } {
            Some(v) => v,
            None => {
                write_configure_error(
                    out_error,
                    OpenPitConfigureError::validation(
                        "spot_funds_pnl_bounds global is null".to_owned(),
                    ),
                );
                return false;
            }
        };
        let mut out = Vec::with_capacity(slice.len());
        for (index, entry) in slice.iter().enumerate() {
            match parse_configure_pnl_barrier(entry, "global", index) {
                Ok(v) => out.push(v),
                Err(e) => {
                    write_configure_error(out_error, e);
                    return false;
                }
            }
        }
        out
    } else {
        Vec::new()
    };

    let account_group_barriers: Vec<SpotFundsPnlBoundsAccountGroupBarrier> = if has_account_group {
        let slice = match unsafe {
            try_slice_arg(
                account_group,
                account_group_len,
                "spot_funds_pnl_bounds account_group",
                std::ptr::null_mut(),
            )
        } {
            Some(v) => v,
            None => {
                write_configure_error(
                    out_error,
                    OpenPitConfigureError::validation(
                        "spot_funds_pnl_bounds account_group is null".to_owned(),
                    ),
                );
                return false;
            }
        };
        let mut out = Vec::with_capacity(slice.len());
        for (index, entry) in slice.iter().enumerate() {
            let account_group_id = match AccountGroupId::from_u32(entry.account_group_id) {
                Ok(v) => v,
                Err(e) => {
                    write_configure_error(
                        out_error,
                        OpenPitConfigureError::validation(format!(
                            "account_group[{index}] account_group_id {} is invalid: {e}",
                            entry.account_group_id
                        )),
                    );
                    return false;
                }
            };
            let barrier = match parse_configure_pnl_barrier(&entry.barrier, "account_group", index)
            {
                Ok(v) => v,
                Err(e) => {
                    write_configure_error(out_error, e);
                    return false;
                }
            };
            out.push(SpotFundsPnlBoundsAccountGroupBarrier {
                barrier,
                account_group_id,
            });
        }
        out
    } else {
        Vec::new()
    };

    let account_barriers: Vec<SpotFundsPnlBoundsAccountBarrierUpdate> = if has_account {
        let slice = match unsafe {
            try_slice_arg(
                account,
                account_len,
                "spot_funds_pnl_bounds account",
                std::ptr::null_mut(),
            )
        } {
            Some(v) => v,
            None => {
                write_configure_error(
                    out_error,
                    OpenPitConfigureError::validation(
                        "spot_funds_pnl_bounds account is null".to_owned(),
                    ),
                );
                return false;
            }
        };
        let mut out = Vec::with_capacity(slice.len());
        for (index, entry) in slice.iter().enumerate() {
            let barrier = match parse_configure_pnl_barrier(&entry.barrier, "account", index) {
                Ok(v) => v,
                Err(e) => {
                    write_configure_error(out_error, e);
                    return false;
                }
            };
            out.push(SpotFundsPnlBoundsAccountBarrierUpdate {
                barrier,
                account_id: AccountId::from_u64(entry.account_id),
            });
        }
        out
    } else {
        Vec::new()
    };

    let result = unsafe { &*engine }.configurator().spot_funds(
        &name,
        |settings| -> Result<(), SpotFundsConfigError> {
            if has_global {
                settings.set_pnl_global_barriers(global_barriers.iter().cloned())?;
            }
            if has_account_group {
                settings.set_pnl_account_group_barriers(account_group_barriers.iter().cloned())?;
            }
            if has_account {
                settings.set_pnl_account_barriers(account_barriers.iter().cloned())?;
            }
            Ok(())
        },
    );
    finish_configure_spot_funds(result, out_error)
}

#[no_mangle]
/// Force-sets the live accumulated account-currency P&L for the spot-funds
/// policy registered under `name`.
///
/// This is an absolute assignment and is separate from barrier retuning, which
/// never resets the accumulator.
pub unsafe extern "C" fn openpit_engine_configure_spot_funds_set_account_pnl(
    engine: *mut crate::engine::OpenPitEngine,
    name: OpenPitStringView,
    account_id: OpenPitParamAccountId,
    account_currency: OpenPitStringView,
    pnl: OpenPitParamPnl,
    out_error: *mut *mut OpenPitConfigureError,
) -> bool {
    let name = match unsafe { configure_spot_funds_name(engine, name, out_error) } {
        Some(name) => name,
        None => return false,
    };
    let account_currency = match parse_configure_asset(
        account_currency,
        "spot_funds_account_pnl",
        0,
        "account_currency",
    ) {
        Ok(v) => v,
        Err(e) => {
            write_configure_error(out_error, e);
            return false;
        }
    };
    let pnl = match pnl.to_param() {
        Ok(v) => v,
        Err(e) => {
            write_configure_error(
                out_error,
                OpenPitConfigureError::validation(format!("pnl is invalid: {e}")),
            );
            return false;
        }
    };

    let result = unsafe { &*engine }
        .configurator()
        .set_spot_funds_account_pnl(
            &name,
            AccountId::from_u64(account_id),
            account_currency,
            pnl,
        );
    finish_configure_spot_funds(result, out_error)
}

/// Sets the global spot-funds limit mode for the policy registered under
/// `name`.
///
/// The global mode applies to every order that resolves to neither a
/// per-account nor a per-account-group override.
///
/// Contract:
/// - `engine` must be a valid non-null engine pointer.
/// - `name` selects the policy; it is interpreted as UTF-8. A built-in policy
///   added via `openpit_engine_builder_add_builtin_spot_funds_policy`
///   registers under its fixed name `"SpotFundsPolicy"`.
/// - `mode` selects `Enforce` (0; reject on insufficient funds) or `TrackOnly`
///   (1; always record, allow negative available).
///
/// Success:
/// - returns `true`; the new global mode applies from the next order onward.
///
/// Error:
/// - returns `false`; if `out_error` is non-null, writes a caller-owned
///   `OpenPitConfigureError` (release with
///   `openpit_destroy_configure_error`).
/// - a null `engine` or null / invalid-UTF-8 `name` returns `false` and, when
///   `out_error` is non-null, writes a caller-owned `OpenPitConfigureError`
///   (`Validation`) that must be released with
///   `openpit_destroy_configure_error`.
/// - an invalid `mode` returns `false` and writes `Validation`.
#[no_mangle]
pub unsafe extern "C" fn openpit_engine_configure_spot_funds_global_limit_mode(
    engine: *mut crate::engine::OpenPitEngine,
    name: OpenPitStringView,
    mode: u8,
    out_error: *mut *mut OpenPitConfigureError,
) -> bool {
    let name = match unsafe { configure_spot_funds_name(engine, name, out_error) } {
        Some(name) => name,
        None => return false,
    };
    let mode = match configure_spot_funds_limit_mode(mode, out_error) {
        Some(mode) => mode,
        None => return false,
    };
    let result = unsafe { &*engine }.configurator().spot_funds(
        &name,
        |settings| -> Result<(), SpotFundsConfigError> {
            settings.set_global_limit_mode(mode);
            Ok(())
        },
    );
    finish_configure_spot_funds(result, out_error)
}

/// Pins or clears the spot-funds limit mode for one account on the policy
/// registered under `name`.
///
/// The per-account override wins over the account-group and global tiers.
///
/// Contract:
/// - `engine` must be a valid non-null engine pointer.
/// - `name` selects the policy; see
///   `openpit_engine_configure_spot_funds_global_limit_mode`.
/// - `account_id` is the account the override applies to.
/// - When `has_mode` is `true`, the account is pinned to `mode`. When
///   `has_mode` is `false`, any existing per-account override is cleared and
///   the cascade falls through to the account-group and global tiers;
///   `mode` is ignored. When `has_mode` is `true`, `mode` must select
///   `Enforce` (0) or `TrackOnly` (1).
///
/// Success / error: as
/// `openpit_engine_configure_spot_funds_global_limit_mode`.
#[no_mangle]
pub unsafe extern "C" fn openpit_engine_configure_spot_funds_account_limit_mode(
    engine: *mut crate::engine::OpenPitEngine,
    name: OpenPitStringView,
    account_id: OpenPitParamAccountId,
    mode: u8,
    has_mode: bool,
    out_error: *mut *mut OpenPitConfigureError,
) -> bool {
    let name = match unsafe { configure_spot_funds_name(engine, name, out_error) } {
        Some(name) => name,
        None => return false,
    };
    let account_id = AccountId::from_u64(account_id);
    let mode = if has_mode {
        match configure_spot_funds_limit_mode(mode, out_error) {
            Some(mode) => Some(mode),
            None => return false,
        }
    } else {
        None
    };
    let result = unsafe { &*engine }.configurator().spot_funds(
        &name,
        |settings| -> Result<(), SpotFundsConfigError> {
            settings.set_account_limit_mode(account_id, mode);
            Ok(())
        },
    );
    finish_configure_spot_funds(result, out_error)
}

/// Pins or clears the spot-funds limit mode for one account group on the
/// policy registered under `name`.
///
/// The override applies to every account in the group that has no per-account
/// override.
///
/// Contract:
/// - `engine` must be a valid non-null engine pointer.
/// - `name` selects the policy; see
///   `openpit_engine_configure_spot_funds_global_limit_mode`.
/// - `account_group_id` is the account group the override applies to; an
///   invalid id fails the call with `Validation`.
/// - When `has_mode` is `true`, the group is pinned to `mode`. When `has_mode`
///   is `false`, any existing per-account-group override is cleared and the
///   cascade falls through to the global tier; `mode` is ignored. When
///   `has_mode` is `true`, `mode` must select `Enforce` (0) or `TrackOnly` (1).
///
/// Success / error: as
/// `openpit_engine_configure_spot_funds_global_limit_mode`.
#[no_mangle]
pub unsafe extern "C" fn openpit_engine_configure_spot_funds_account_group_limit_mode(
    engine: *mut crate::engine::OpenPitEngine,
    name: OpenPitStringView,
    account_group_id: OpenPitParamAccountGroupId,
    mode: u8,
    has_mode: bool,
    out_error: *mut *mut OpenPitConfigureError,
) -> bool {
    let name = match unsafe { configure_spot_funds_name(engine, name, out_error) } {
        Some(name) => name,
        None => return false,
    };
    let account_group_id = match AccountGroupId::from_u32(account_group_id) {
        Ok(id) => id,
        Err(error) => {
            write_configure_error(
                out_error,
                OpenPitConfigureError::validation(format!(
                    "spot funds account group id {account_group_id} is invalid: {error}"
                )),
            );
            return false;
        }
    };
    let mode = if has_mode {
        match configure_spot_funds_limit_mode(mode, out_error) {
            Some(mode) => Some(mode),
            None => return false,
        }
    } else {
        None
    };
    let result = unsafe { &*engine }.configurator().spot_funds(
        &name,
        |settings| -> Result<(), SpotFundsConfigError> {
            settings.set_account_group_limit_mode(account_group_id, mode);
            Ok(())
        },
    );
    finish_configure_spot_funds(result, out_error)
}

/// Validates the `engine` pointer and `name` shared by the spot-funds
/// limit-mode configure entry points, returning the decoded name or writing a
/// `Validation` error and returning `None`.
unsafe fn configure_spot_funds_name(
    engine: *mut crate::engine::OpenPitEngine,
    name: OpenPitStringView,
    out_error: *mut *mut OpenPitConfigureError,
) -> Option<String> {
    if engine.is_null() {
        write_configure_error(
            out_error,
            OpenPitConfigureError::validation("engine is null".to_owned()),
        );
        return None;
    }
    match unsafe { cstr_arg(name) } {
        Some(name) => Some(name),
        None => {
            write_configure_error(
                out_error,
                OpenPitConfigureError::validation(
                    "policy name is null or invalid UTF-8".to_owned(),
                ),
            );
            None
        }
    }
}

/// Maps the configurator result of a spot-funds limit-mode update to the FFI
/// boolean convention, writing a caller-owned `OpenPitConfigureError` on
/// failure.
fn finish_configure_spot_funds(
    result: Result<(), openpit::ConfigureError>,
    out_error: *mut *mut OpenPitConfigureError,
) -> bool {
    match result {
        Ok(()) => true,
        Err(err) => {
            write_configure_error(out_error, OpenPitConfigureError::new(err));
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{openpit_create_engine_builder, openpit_destroy_engine_builder};
    use crate::last_error::OpenPitOutError;
    use crate::marketdata::{
        openpit_create_marketdata_quote_ttl_infinite, openpit_create_marketdata_service,
        openpit_destroy_marketdata_service,
    };
    use crate::string::openpit_destroy_shared_string;

    fn null_out_error() -> OpenPitOutError {
        std::ptr::null_mut()
    }

    /// Creates a Full-mode (byte 1) engine builder.
    fn make_builder() -> *mut crate::engine::OpenPitEngineBuilder {
        let mut err: *mut crate::string::OpenPitSharedString = std::ptr::null_mut();
        openpit_create_engine_builder(1, &mut err as *mut _ as OpenPitOutError)
    }

    /// Creates a None-mode (byte 0) engine builder.
    fn make_local_engine_builder() -> *mut crate::engine::OpenPitEngineBuilder {
        let mut err: *mut crate::string::OpenPitSharedString = std::ptr::null_mut();
        openpit_create_engine_builder(0, &mut err as *mut _ as OpenPitOutError)
    }

    /// Creates a Full-mode MD service (byte 1 = Full).
    fn make_service() -> *mut OpenPitMarketDataService {
        let mut err: *mut crate::string::OpenPitSharedString = std::ptr::null_mut();
        let service = openpit_create_marketdata_service(
            1,
            openpit_create_marketdata_quote_ttl_infinite(),
            &mut err as *mut _ as OpenPitOutError,
        );
        assert!(!service.is_null());
        service
    }

    /// Creates a no-sync MD service (byte 0 = None/no-sync).
    fn make_no_sync_service() -> *mut OpenPitMarketDataService {
        let mut err: *mut crate::string::OpenPitSharedString = std::ptr::null_mut();
        let service = openpit_create_marketdata_service(
            0,
            openpit_create_marketdata_quote_ttl_infinite(),
            &mut err as *mut _ as OpenPitOutError,
        );
        assert!(!service.is_null());
        service
    }

    fn cstr_to_string(handle: *mut crate::string::OpenPitSharedString) -> String {
        if handle.is_null() {
            return String::new();
        }
        let view = crate::string::openpit_shared_string_view(handle);
        let result = if view.ptr.is_null() {
            String::new()
        } else {
            let bytes = unsafe { std::slice::from_raw_parts(view.ptr, view.len) };
            std::str::from_utf8(bytes).expect("utf8").to_string()
        };
        openpit_destroy_shared_string(handle);
        result
    }

    fn instrument_override(
        instrument_id: OpenPitMarketDataInstrumentId,
        slippage_bps: Option<u16>,
    ) -> OpenPitPretradePoliciesSpotFundsOverride {
        OpenPitPretradePoliciesSpotFundsOverride {
            target: OpenPitPretradePoliciesSpotFundsOverrideTarget {
                tag: OpenPitPretradePoliciesSpotFundsOverrideTargetTag::Instrument as u8,
                payload: OpenPitPretradePoliciesSpotFundsOverrideTargetPayload {
                    instrument: OpenPitPretradePoliciesSpotFundsOverrideTargetInstrument {
                        instrument_id,
                    },
                },
            },
            slippage_bps: slippage_bps.unwrap_or_default(),
            has_slippage_bps: slippage_bps.is_some(),
        }
    }

    fn account_override(
        instrument_id: OpenPitMarketDataInstrumentId,
        account_id: OpenPitParamAccountId,
        slippage_bps: u16,
    ) -> OpenPitPretradePoliciesSpotFundsOverride {
        OpenPitPretradePoliciesSpotFundsOverride {
            target: OpenPitPretradePoliciesSpotFundsOverrideTarget {
                tag: OpenPitPretradePoliciesSpotFundsOverrideTargetTag::InstrumentAccount as u8,
                payload: OpenPitPretradePoliciesSpotFundsOverrideTargetPayload {
                    instrument_account:
                        OpenPitPretradePoliciesSpotFundsOverrideTargetInstrumentAccount {
                            instrument_id,
                            account_id,
                        },
                },
            },
            slippage_bps,
            has_slippage_bps: true,
        }
    }

    fn group_override(
        instrument_id: OpenPitMarketDataInstrumentId,
        account_group_id: OpenPitParamAccountGroupId,
        slippage_bps: u16,
    ) -> OpenPitPretradePoliciesSpotFundsOverride {
        OpenPitPretradePoliciesSpotFundsOverride {
            target: OpenPitPretradePoliciesSpotFundsOverrideTarget {
                tag: OpenPitPretradePoliciesSpotFundsOverrideTargetTag::InstrumentAccountGroup
                    as u8,
                payload: OpenPitPretradePoliciesSpotFundsOverrideTargetPayload {
                    instrument_account_group:
                        OpenPitPretradePoliciesSpotFundsOverrideTargetInstrumentAccountGroup {
                            instrument_id,
                            account_group_id,
                        },
                },
            },
            slippage_bps,
            has_slippage_bps: true,
        }
    }

    #[test]
    fn add_builtin_spot_funds_policy_limit_only() {
        let builder = make_builder();
        let result = unsafe {
            openpit_engine_builder_add_builtin_spot_funds_policy(
                builder,
                std::ptr::null(),
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                0,
                null_out_error(),
            )
        };
        assert!(result);
    }

    #[test]
    fn add_builtin_spot_funds_pnl_bounds_requires_barrier_via_core() {
        let builder = make_builder();
        let mut err: *mut crate::string::OpenPitSharedString = std::ptr::null_mut();
        let result = unsafe {
            openpit_engine_builder_add_builtin_spot_funds_pnl_bounds_killswitch_policy(
                builder,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                &mut err as *mut _ as OpenPitOutError,
            )
        };

        assert!(!result);
        assert_eq!(
            cstr_to_string(err),
            "spot funds pnl barriers invalid: spot funds P&L bounds require at least one barrier"
        );
        openpit_destroy_engine_builder(builder);
    }

    #[test]
    fn add_builtin_spot_funds_policy_with_service() {
        let builder = make_builder();
        let service = make_service();
        let bps: u16 = 1500;
        let result = unsafe {
            openpit_engine_builder_add_builtin_spot_funds_policy(
                builder,
                service,
                &bps as *const u16,
                0,
                std::ptr::null(),
                0,
                7,
                null_out_error(),
            )
        };
        assert!(result);
        openpit_destroy_marketdata_service(service);
    }

    #[test]
    fn add_builtin_spot_funds_policy_with_overrides_and_booktop() {
        let builder = make_builder();
        let service = make_service();
        let bps: u16 = 1000;
        let overrides = [
            instrument_override(1, Some(500)),
            instrument_override(2, None),
        ];
        let result = unsafe {
            openpit_engine_builder_add_builtin_spot_funds_policy(
                builder,
                service,
                &bps as *const u16,
                1,
                overrides.as_ptr(),
                overrides.len(),
                0,
                null_out_error(),
            )
        };
        assert!(result);
        openpit_destroy_marketdata_service(service);
    }

    #[test]
    fn add_builtin_spot_funds_policy_with_account_override() {
        let builder = make_builder();
        let service = make_service();
        let bps: u16 = 1000;
        let overrides = [account_override(1, 99224416, 250)];
        let result = unsafe {
            openpit_engine_builder_add_builtin_spot_funds_policy(
                builder,
                service,
                &bps as *const u16,
                0,
                overrides.as_ptr(),
                overrides.len(),
                0,
                null_out_error(),
            )
        };
        assert!(result);
        openpit_destroy_marketdata_service(service);
    }

    #[test]
    fn add_builtin_spot_funds_policy_with_group_override() {
        let builder = make_builder();
        let service = make_service();
        let bps: u16 = 1000;
        let overrides = [group_override(1, 3, 250)];
        let result = unsafe {
            openpit_engine_builder_add_builtin_spot_funds_policy(
                builder,
                service,
                &bps as *const u16,
                0,
                overrides.as_ptr(),
                overrides.len(),
                0,
                null_out_error(),
            )
        };
        assert!(result);
        openpit_destroy_marketdata_service(service);
    }

    #[test]
    fn add_builtin_spot_funds_policy_override_with_invalid_tag_is_error() {
        let builder = make_builder();
        let service = make_service();
        let bps: u16 = 1000;
        let overrides = [OpenPitPretradePoliciesSpotFundsOverride {
            target: OpenPitPretradePoliciesSpotFundsOverrideTarget {
                tag: 255,
                payload: OpenPitPretradePoliciesSpotFundsOverrideTargetPayload {
                    instrument: OpenPitPretradePoliciesSpotFundsOverrideTargetInstrument {
                        instrument_id: 1,
                    },
                },
            },
            slippage_bps: 250,
            has_slippage_bps: true,
        }];
        let mut err: *mut crate::string::OpenPitSharedString = std::ptr::null_mut();
        let result = unsafe {
            openpit_engine_builder_add_builtin_spot_funds_policy(
                builder,
                service,
                &bps as *const u16,
                0,
                overrides.as_ptr(),
                overrides.len(),
                0,
                &mut err as *mut _ as OpenPitOutError,
            )
        };
        assert!(!result);
        assert!(!err.is_null());
        let msg = cstr_to_string(err);
        assert!(msg.contains("target tag 255 is invalid"));
        openpit_destroy_marketdata_service(service);
    }

    #[test]
    fn add_builtin_spot_funds_policy_override_with_invalid_group_is_error() {
        let builder = make_builder();
        let service = make_service();
        let bps: u16 = 1000;
        let overrides = [group_override(1, 0, 250)];
        let mut err: *mut crate::string::OpenPitSharedString = std::ptr::null_mut();
        let result = unsafe {
            openpit_engine_builder_add_builtin_spot_funds_policy(
                builder,
                service,
                &bps as *const u16,
                0,
                overrides.as_ptr(),
                overrides.len(),
                0,
                &mut err as *mut _ as OpenPitOutError,
            )
        };
        assert!(!result);
        assert!(!err.is_null());
        let msg = cstr_to_string(err);
        assert!(msg.contains("account group id 0 is invalid"));
        openpit_destroy_marketdata_service(service);
    }

    #[test]
    fn add_builtin_spot_funds_policy_service_without_slippage_is_config_error() {
        let builder = make_builder();
        let service = make_service();
        let mut err: *mut crate::string::OpenPitSharedString = std::ptr::null_mut();
        let result = unsafe {
            openpit_engine_builder_add_builtin_spot_funds_policy(
                builder,
                service,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                0,
                &mut err as *mut _ as OpenPitOutError,
            )
        };
        assert!(!result);
        assert!(!err.is_null());
        let msg = cstr_to_string(err);
        assert!(msg.contains("market_slippage_bps is required"));
        openpit_destroy_marketdata_service(service);
    }

    #[test]
    fn add_builtin_spot_funds_policy_invalid_pricing_source() {
        let builder = make_builder();
        let service = make_service();
        let bps: u16 = 100;
        let mut err: *mut crate::string::OpenPitSharedString = std::ptr::null_mut();
        let result = unsafe {
            openpit_engine_builder_add_builtin_spot_funds_policy(
                builder,
                service,
                &bps as *const u16,
                9,
                std::ptr::null(),
                0,
                0,
                &mut err as *mut _ as OpenPitOutError,
            )
        };
        assert!(!result);
        assert!(!err.is_null());
        let msg = cstr_to_string(err);
        assert!(msg.contains("pricing_source"));
        openpit_destroy_marketdata_service(service);
    }

    #[test]
    fn add_builtin_spot_funds_policy_slippage_out_of_range() {
        let builder = make_builder();
        let service = make_service();
        let bps: u16 = 20_000;
        let mut err: *mut crate::string::OpenPitSharedString = std::ptr::null_mut();
        let result = unsafe {
            openpit_engine_builder_add_builtin_spot_funds_policy(
                builder,
                service,
                &bps as *const u16,
                0,
                std::ptr::null(),
                0,
                0,
                &mut err as *mut _ as OpenPitOutError,
            )
        };
        assert!(!result);
        assert!(!err.is_null());
        cstr_to_string(err);
        openpit_destroy_marketdata_service(service);
    }

    #[test]
    fn add_builtin_spot_funds_policy_null_builder() {
        let mut err: *mut crate::string::OpenPitSharedString = std::ptr::null_mut();
        let result = unsafe {
            openpit_engine_builder_add_builtin_spot_funds_policy(
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                0,
                &mut err as *mut _ as OpenPitOutError,
            )
        };
        assert!(!result);
        assert!(!err.is_null());
        let msg = cstr_to_string(err);
        assert!(msg.contains("null"));
    }

    #[test]
    fn add_builtin_spot_funds_policy_overrides_null_with_positive_len() {
        let builder = make_builder();
        let service = make_service();
        let bps: u16 = 100;
        let mut err: *mut crate::string::OpenPitSharedString = std::ptr::null_mut();
        let result = unsafe {
            openpit_engine_builder_add_builtin_spot_funds_policy(
                builder,
                service,
                &bps as *const u16,
                0,
                std::ptr::null(),
                1,
                0,
                &mut err as *mut _ as OpenPitOutError,
            )
        };
        assert!(!result);
        assert!(!err.is_null());
        cstr_to_string(err);
        openpit_destroy_marketdata_service(service);
    }

    /// Full/Account engine + no-sync MD service must be rejected with a
    /// descriptive mismatch error.
    #[test]
    fn full_engine_with_local_md_service_is_rejected() {
        // Full engine builder (byte 1).
        let full_eng = make_builder();
        // No-sync MD service (byte 0 = None).
        let local_service = make_no_sync_service();

        let bps: u16 = 100;
        let mut err: *mut crate::string::OpenPitSharedString = std::ptr::null_mut();
        let result = unsafe {
            openpit_engine_builder_add_builtin_spot_funds_policy(
                full_eng,
                local_service,
                &bps as *const u16,
                0,
                std::ptr::null(),
                0,
                0,
                &mut err as *mut _ as OpenPitOutError,
            )
        };
        assert!(!result, "expected rejection due to sync mode mismatch");
        assert!(!err.is_null());
        let msg = cstr_to_string(err);
        assert!(
            msg.contains("no-sync") && msg.contains("multi-threaded"),
            "unexpected error message: {msg}"
        );

        openpit_destroy_marketdata_service(local_service);
        openpit_destroy_engine_builder(full_eng);
    }

    /// No-sync engine accepts a no-sync MD service (no mismatch).
    #[test]
    fn local_engine_with_local_md_service_is_accepted() {
        let local_eng = make_local_engine_builder();
        // No-sync MD service (byte 0 = None).
        let local_service = make_no_sync_service();

        let bps: u16 = 100;
        let result = unsafe {
            openpit_engine_builder_add_builtin_spot_funds_policy(
                local_eng,
                local_service,
                &bps as *const u16,
                0,
                std::ptr::null(),
                0,
                0,
                null_out_error(),
            )
        };
        assert!(result, "no-sync engine + no-sync MD should be accepted");

        openpit_destroy_marketdata_service(local_service);
        openpit_destroy_engine_builder(local_eng);
    }

    /// No-sync engine accepts a full-sync MD service (no mismatch - a no-sync
    /// engine imposes no locking requirements on the MD service).
    #[test]
    fn local_engine_with_full_md_service_is_accepted() {
        let local_eng = make_local_engine_builder();
        // Full MD service (byte 1 = Full).
        let full_service = make_service();

        let bps: u16 = 100;
        let result = unsafe {
            openpit_engine_builder_add_builtin_spot_funds_policy(
                local_eng,
                full_service,
                &bps as *const u16,
                0,
                std::ptr::null(),
                0,
                0,
                null_out_error(),
            )
        };
        assert!(result, "no-sync engine + full-sync MD should be accepted");

        openpit_destroy_marketdata_service(full_service);
        openpit_destroy_engine_builder(local_eng);
    }

    #[test]
    fn configure_spot_funds_rejects_null_and_invalid_utf8_names() {
        let builder = make_builder();
        assert!(unsafe {
            openpit_engine_builder_add_builtin_spot_funds_policy(
                builder,
                std::ptr::null(),
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                0,
                null_out_error(),
            )
        });
        let engine = crate::engine::openpit_engine_builder_build(
            builder,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
        assert!(!engine.is_null());
        openpit_destroy_engine_builder(builder);

        let invalid_utf8 = [0xff];
        let invalid_name = OpenPitStringView {
            ptr: invalid_utf8.as_ptr(),
            len: invalid_utf8.len(),
        };

        for name in [OpenPitStringView::default(), invalid_name] {
            let mut out_error = std::ptr::null_mut();
            let ok = unsafe {
                openpit_engine_configure_spot_funds(
                    engine,
                    name,
                    0,
                    false,
                    0,
                    false,
                    std::ptr::null(),
                    0,
                    false,
                    &mut out_error,
                )
            };
            assert!(!ok);
            assert!(!out_error.is_null());
            assert_eq!(
                crate::engine::openpit_configure_error_get_kind(out_error),
                crate::engine::OpenPitConfigureErrorKind::Validation
            );
            crate::engine::openpit_destroy_configure_error(out_error);
        }

        crate::engine::openpit_destroy_engine(engine);
    }

    #[test]
    fn configure_spot_funds_invalid_target_tag_uses_structured_error() {
        let builder = make_builder();
        assert!(unsafe {
            openpit_engine_builder_add_builtin_spot_funds_policy(
                builder,
                std::ptr::null(),
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                0,
                null_out_error(),
            )
        });
        let engine = crate::engine::openpit_engine_builder_build(
            builder,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
        assert!(!engine.is_null());
        openpit_destroy_engine_builder(builder);

        let overrides = [OpenPitPretradePoliciesSpotFundsOverride {
            target: OpenPitPretradePoliciesSpotFundsOverrideTarget {
                tag: 255,
                payload: OpenPitPretradePoliciesSpotFundsOverrideTargetPayload {
                    instrument: OpenPitPretradePoliciesSpotFundsOverrideTargetInstrument {
                        instrument_id: 1,
                    },
                },
            },
            slippage_bps: 0,
            has_slippage_bps: false,
        }];
        let mut out_error = std::ptr::null_mut();
        let ok = unsafe {
            openpit_engine_configure_spot_funds(
                engine,
                OpenPitStringView::from_utf8("SpotFundsPolicy"),
                0,
                false,
                0,
                false,
                overrides.as_ptr(),
                overrides.len(),
                true,
                &mut out_error,
            )
        };
        assert!(!ok);
        assert!(!out_error.is_null());
        assert_eq!(
            crate::engine::openpit_configure_error_get_kind(out_error),
            crate::engine::OpenPitConfigureErrorKind::Validation
        );
        crate::engine::openpit_destroy_configure_error(out_error);
        crate::engine::openpit_destroy_engine(engine);
    }

    /// Builds a None-mode engine with a limit-only spot-funds policy named
    /// `"SpotFundsPolicy"`.
    fn engine_with_spot_funds() -> *mut crate::engine::OpenPitEngine {
        let builder = make_local_engine_builder();
        assert!(unsafe {
            openpit_engine_builder_add_builtin_spot_funds_policy(
                builder,
                std::ptr::null(),
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                0,
                null_out_error(),
            )
        });
        let engine = crate::engine::openpit_engine_builder_build(
            builder,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
        assert!(!engine.is_null());
        openpit_destroy_engine_builder(builder);
        engine
    }

    #[test]
    fn configure_spot_funds_limit_modes_happy_path() {
        let engine = engine_with_spot_funds();
        let name = OpenPitStringView::from_utf8("SpotFundsPolicy");

        assert!(unsafe {
            openpit_engine_configure_spot_funds_global_limit_mode(
                engine,
                name,
                OpenPitPretradePoliciesSpotFundsLimitMode::TrackOnly as u8,
                std::ptr::null_mut(),
            )
        });
        assert!(unsafe {
            openpit_engine_configure_spot_funds_account_limit_mode(
                engine,
                name,
                42,
                OpenPitPretradePoliciesSpotFundsLimitMode::TrackOnly as u8,
                true,
                std::ptr::null_mut(),
            )
        });
        // Clear the per-account override (has_mode == false).
        assert!(unsafe {
            openpit_engine_configure_spot_funds_account_limit_mode(
                engine,
                name,
                42,
                OpenPitPretradePoliciesSpotFundsLimitMode::Enforce as u8,
                false,
                std::ptr::null_mut(),
            )
        });
        assert!(unsafe {
            openpit_engine_configure_spot_funds_account_group_limit_mode(
                engine,
                name,
                3,
                OpenPitPretradePoliciesSpotFundsLimitMode::TrackOnly as u8,
                true,
                std::ptr::null_mut(),
            )
        });
        // Clear the per-account-group override (has_mode == false).
        assert!(unsafe {
            openpit_engine_configure_spot_funds_account_group_limit_mode(
                engine,
                name,
                3,
                OpenPitPretradePoliciesSpotFundsLimitMode::Enforce as u8,
                false,
                std::ptr::null_mut(),
            )
        });

        crate::engine::openpit_destroy_engine(engine);
    }

    #[test]
    fn configure_spot_funds_global_limit_mode_null_engine_is_validation() {
        let mut out_error = std::ptr::null_mut();
        let ok = unsafe {
            openpit_engine_configure_spot_funds_global_limit_mode(
                std::ptr::null_mut(),
                OpenPitStringView::from_utf8("SpotFundsPolicy"),
                OpenPitPretradePoliciesSpotFundsLimitMode::TrackOnly as u8,
                &mut out_error,
            )
        };
        assert!(!ok);
        assert!(!out_error.is_null());
        assert_eq!(
            crate::engine::openpit_configure_error_get_kind(out_error),
            crate::engine::OpenPitConfigureErrorKind::Validation
        );
        crate::engine::openpit_destroy_configure_error(out_error);
    }

    #[test]
    fn configure_spot_funds_global_limit_mode_invalid_mode_is_validation() {
        let engine = engine_with_spot_funds();
        let mut out_error = std::ptr::null_mut();
        let ok = unsafe {
            openpit_engine_configure_spot_funds_global_limit_mode(
                engine,
                OpenPitStringView::from_utf8("SpotFundsPolicy"),
                99,
                &mut out_error,
            )
        };
        assert!(!ok);
        assert!(!out_error.is_null());
        assert_eq!(
            crate::engine::openpit_configure_error_get_kind(out_error),
            crate::engine::OpenPitConfigureErrorKind::Validation
        );
        crate::engine::openpit_destroy_configure_error(out_error);
        crate::engine::openpit_destroy_engine(engine);
    }

    #[test]
    fn configure_spot_funds_account_limit_mode_ignores_mode_when_clearing() {
        let engine = engine_with_spot_funds();
        let ok = unsafe {
            openpit_engine_configure_spot_funds_account_limit_mode(
                engine,
                OpenPitStringView::from_utf8("SpotFundsPolicy"),
                42,
                99,
                false,
                std::ptr::null_mut(),
            )
        };
        assert!(ok);
        crate::engine::openpit_destroy_engine(engine);
    }

    #[test]
    fn configure_spot_funds_account_limit_mode_invalid_mode_is_validation() {
        let engine = engine_with_spot_funds();
        let mut out_error = std::ptr::null_mut();
        let ok = unsafe {
            openpit_engine_configure_spot_funds_account_limit_mode(
                engine,
                OpenPitStringView::from_utf8("SpotFundsPolicy"),
                42,
                99,
                true,
                &mut out_error,
            )
        };
        assert!(!ok);
        assert!(!out_error.is_null());
        assert_eq!(
            crate::engine::openpit_configure_error_get_kind(out_error),
            crate::engine::OpenPitConfigureErrorKind::Validation
        );
        crate::engine::openpit_destroy_configure_error(out_error);
        crate::engine::openpit_destroy_engine(engine);
    }

    #[test]
    fn configure_spot_funds_account_group_limit_mode_ignores_mode_when_clearing() {
        let engine = engine_with_spot_funds();
        let ok = unsafe {
            openpit_engine_configure_spot_funds_account_group_limit_mode(
                engine,
                OpenPitStringView::from_utf8("SpotFundsPolicy"),
                3,
                99,
                false,
                std::ptr::null_mut(),
            )
        };
        assert!(ok);
        crate::engine::openpit_destroy_engine(engine);
    }

    #[test]
    fn configure_spot_funds_account_group_limit_mode_invalid_mode_is_validation() {
        let engine = engine_with_spot_funds();
        let mut out_error = std::ptr::null_mut();
        let ok = unsafe {
            openpit_engine_configure_spot_funds_account_group_limit_mode(
                engine,
                OpenPitStringView::from_utf8("SpotFundsPolicy"),
                3,
                99,
                true,
                &mut out_error,
            )
        };
        assert!(!ok);
        assert!(!out_error.is_null());
        assert_eq!(
            crate::engine::openpit_configure_error_get_kind(out_error),
            crate::engine::OpenPitConfigureErrorKind::Validation
        );
        crate::engine::openpit_destroy_configure_error(out_error);
        crate::engine::openpit_destroy_engine(engine);
    }

    #[test]
    fn configure_spot_funds_account_group_limit_mode_invalid_group_is_validation() {
        let engine = engine_with_spot_funds();
        let mut out_error = std::ptr::null_mut();
        let ok = unsafe {
            openpit_engine_configure_spot_funds_account_group_limit_mode(
                engine,
                OpenPitStringView::from_utf8("SpotFundsPolicy"),
                0,
                OpenPitPretradePoliciesSpotFundsLimitMode::TrackOnly as u8,
                true,
                &mut out_error,
            )
        };
        assert!(!ok);
        assert!(!out_error.is_null());
        assert_eq!(
            crate::engine::openpit_configure_error_get_kind(out_error),
            crate::engine::OpenPitConfigureErrorKind::Validation
        );
        crate::engine::openpit_destroy_configure_error(out_error);
        crate::engine::openpit_destroy_engine(engine);
    }

    #[test]
    fn configure_spot_funds_limit_mode_unknown_policy_name() {
        let engine = engine_with_spot_funds();
        let mut out_error = std::ptr::null_mut();
        let ok = unsafe {
            openpit_engine_configure_spot_funds_global_limit_mode(
                engine,
                OpenPitStringView::from_utf8("NoSuchPolicy"),
                OpenPitPretradePoliciesSpotFundsLimitMode::TrackOnly as u8,
                &mut out_error,
            )
        };
        assert!(!ok);
        assert!(!out_error.is_null());
        crate::engine::openpit_destroy_configure_error(out_error);
        crate::engine::openpit_destroy_engine(engine);
    }
}
