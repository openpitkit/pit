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

#![allow(clippy::missing_safety_doc, clippy::not_unsafe_ptr_arg_deref)]

use openpit::param::{AccountGroupId, AccountId, Pnl};
use openpit::pretrade::policies::{
    SpotFundsPnlBoundsAccountBarrier, SpotFundsPnlBoundsAccountGroupBarrier,
    SpotFundsPnlBoundsAxis, SpotFundsPnlBoundsBarrier, SpotFundsPolicy, SpotFundsSettings,
};
use openpit::pretrade::SpotFundsLimitMode;
use openpit::{
    InstrumentId, PolicyGroupId, SpotFundsConfigError, SpotFundsMarketData, SpotFundsOverride,
    SpotFundsOverrideTarget, SpotFundsPricingSource,
};
use openpit_interop::{EngineLocking, SyncMode};

use crate::account_group_id::OpenPitParamAccountGroupId;
use crate::account_outcome::{import_pnl_state, OpenPitPnlState};
use crate::engine::{write_configure_error, OpenPitConfigureError};
use crate::marketdata::{OpenPitMarketDataInstrumentId, OpenPitMarketDataService};
use crate::param::{OpenPitParamAccountId, OpenPitParamPnlOptional, OpenPitParamQuantity};
use crate::reject::{
    block_outcomes_to_list_owned, blocks_to_list_owned, import_rule_mode,
    OpenPitPretradeAccountBlockList, OpenPitPretradeAccountBlockOutcomeList,
    OpenPitPretradeRuleMode,
};

/// Raw selector for the spot-funds insufficient-funds behavior.
pub type OpenPitPretradePoliciesSpotFundsLimitMode = u8;

/// Reject a reservation when available funds are insufficient; the
/// reservation is not recorded. This is the default mode.
pub const OPENPIT_PRETRADE_POLICIES_SPOT_FUNDS_LIMIT_MODE_ENFORCE:
    OpenPitPretradePoliciesSpotFundsLimitMode = 0;
/// Always record the reservation; `available` may go negative and a shortfall
/// never rejects. Arithmetic overflow is still surfaced.
pub const OPENPIT_PRETRADE_POLICIES_SPOT_FUNDS_LIMIT_MODE_TRACK_ONLY:
    OpenPitPretradePoliciesSpotFundsLimitMode = 1;

pub(crate) fn import_spot_funds_limit_mode(
    value: OpenPitPretradePoliciesSpotFundsLimitMode,
) -> Result<SpotFundsLimitMode, OpenPitConfigureError> {
    match value {
        OPENPIT_PRETRADE_POLICIES_SPOT_FUNDS_LIMIT_MODE_ENFORCE => Ok(SpotFundsLimitMode::Enforce),
        OPENPIT_PRETRADE_POLICIES_SPOT_FUNDS_LIMIT_MODE_TRACK_ONLY => {
            Ok(SpotFundsLimitMode::TrackOnly)
        }
        other => Err(OpenPitConfigureError::validation(format!(
            "spot funds limit_mode must be 0 (Enforce) or 1 (TrackOnly), got {other}"
        ))),
    }
}

/// Raw selector for a runtime spot-funds P&L measure, with values 0 through 3.
///
/// - 0 (`AccountUnrealized`): sum of unrealized P&L in account currency.
/// - 1 (`AccountTotal`): realized account P&L plus account unrealized P&L.
/// - 2 (`PositionUnrealized`): unrealized P&L of each position, compared alone.
/// - 3 (`PositionTotal`): realized plus unrealized P&L of each position.
///
/// Unrealized P&L is the amount a full close of owned quantity at the current
/// price would add to realized P&L. Every position uses the same bounds.
pub type OpenPitPretradePoliciesSpotFundsPnlBoundsAxis = u8;

/// Bound the sum of unrealized position P&L in the account currency.
pub const OPENPIT_PRETRADE_POLICIES_SPOT_FUNDS_PNL_BOUNDS_AXIS_ACCOUNT_UNREALIZED:
    OpenPitPretradePoliciesSpotFundsPnlBoundsAxis = 0;
/// Bound realized account P&L plus account unrealized P&L.
pub const OPENPIT_PRETRADE_POLICIES_SPOT_FUNDS_PNL_BOUNDS_AXIS_ACCOUNT_TOTAL:
    OpenPitPretradePoliciesSpotFundsPnlBoundsAxis = 1;
/// Bound the unrealized P&L of each position against the same bounds.
pub const OPENPIT_PRETRADE_POLICIES_SPOT_FUNDS_PNL_BOUNDS_AXIS_POSITION_UNREALIZED:
    OpenPitPretradePoliciesSpotFundsPnlBoundsAxis = 2;
/// Bound realized plus unrealized P&L of each position against the same bounds.
pub const OPENPIT_PRETRADE_POLICIES_SPOT_FUNDS_PNL_BOUNDS_AXIS_POSITION_TOTAL:
    OpenPitPretradePoliciesSpotFundsPnlBoundsAxis = 3;

fn import_spot_funds_pnl_bounds_axis(
    value: OpenPitPretradePoliciesSpotFundsPnlBoundsAxis,
) -> Result<SpotFundsPnlBoundsAxis, OpenPitConfigureError> {
    match value {
        OPENPIT_PRETRADE_POLICIES_SPOT_FUNDS_PNL_BOUNDS_AXIS_ACCOUNT_UNREALIZED => {
            Ok(SpotFundsPnlBoundsAxis::AccountUnrealized)
        }
        OPENPIT_PRETRADE_POLICIES_SPOT_FUNDS_PNL_BOUNDS_AXIS_ACCOUNT_TOTAL => {
            Ok(SpotFundsPnlBoundsAxis::AccountTotal)
        }
        OPENPIT_PRETRADE_POLICIES_SPOT_FUNDS_PNL_BOUNDS_AXIS_POSITION_UNREALIZED => {
            Ok(SpotFundsPnlBoundsAxis::PositionUnrealized)
        }
        OPENPIT_PRETRADE_POLICIES_SPOT_FUNDS_PNL_BOUNDS_AXIS_POSITION_TOTAL => {
            Ok(SpotFundsPnlBoundsAxis::PositionTotal)
        }
        other => Err(OpenPitConfigureError::validation(format!(
            concat!(
                "spot funds pnl_bounds axis must be 0 (AccountUnrealized), ",
                "1 (AccountTotal), 2 (PositionUnrealized) or 3 (PositionTotal), got {}"
            ),
            other
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
    value: OpenPitPretradePoliciesSpotFundsLimitMode,
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
/// Spot funds overrides use an explicit tagged hierarchy with the variants
/// `Instrument`, `InstrumentAccount`, and `InstrumentAccountGroup`.
pub type OpenPitPretradePoliciesSpotFundsOverrideTargetTag = u8;

/// Instrument-level override.
pub const OPENPIT_PRETRADE_POLICIES_SPOT_FUNDS_OVERRIDE_TARGET_TAG_INSTRUMENT:
    OpenPitPretradePoliciesSpotFundsOverrideTargetTag = 0;
/// Override for one instrument and account.
pub const OPENPIT_PRETRADE_POLICIES_SPOT_FUNDS_OVERRIDE_TARGET_TAG_INSTRUMENT_ACCOUNT:
    OpenPitPretradePoliciesSpotFundsOverrideTargetTag = 1;
/// Override for one instrument and account group.
pub const OPENPIT_PRETRADE_POLICIES_SPOT_FUNDS_OVERRIDE_TARGET_TAG_INSTRUMENT_ACCOUNT_GROUP:
    OpenPitPretradePoliciesSpotFundsOverrideTargetTag = 2;

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
    /// One of the `OPENPIT_PRETRADE_POLICIES_SPOT_FUNDS_OVERRIDE_TARGET_TAG_*`
    /// constants. Unknown values are rejected before the payload is read.
    pub tag: OpenPitPretradePoliciesSpotFundsOverrideTargetTag,
    /// Payload selected by `tag`.
    pub payload: OpenPitPretradePoliciesSpotFundsOverrideTargetPayload,
}

/// Slippage override entry for the spot funds policy.
///
/// `target` selects one of three tagged variants: `Instrument`,
/// `InstrumentAccount`, or `InstrumentAccountGroup`. When
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

/// Spot-funds account P&L bounds. With a known effective account currency, an
/// account-tier mismatch fails closed and blocks the account; account-group and
/// global mismatches are skipped. A known-currency account with no account
/// barrier and no matching fallback has no effective barrier, but its P&L keeps
/// accumulating and publishing. Without an effective currency, the first
/// in-scope barrier applies. Bounds are compared as stored and are never
/// FX-converted.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpenPitPretradePoliciesSpotFundsPnlBoundsBarrier {
    /// Rule mode: 0 or 1 Reject, 2 ReviewWithHold, 3 ReviewWithoutHold.
    /// Unknown values are invalid.
    pub mode: OpenPitPretradeRuleMode,
    /// Currency matched when the account has an effective currency.
    pub currency: OpenPitStringView,
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
    /// Bounds and rule mode for this group. A repeated (group, mode) is invalid.
    pub barrier: OpenPitPretradePoliciesSpotFundsPnlBoundsBarrier,
}

/// Account spot-funds P&L bounds refinement.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpenPitPretradePoliciesSpotFundsPnlBoundsAccountBarrier {
    /// Account the barrier applies to.
    pub account_id: OpenPitParamAccountId,
    /// Bounds and rule mode for this account. A repeated (account, mode) is invalid.
    pub barrier: OpenPitPretradePoliciesSpotFundsPnlBoundsBarrier,
}

fn parse_pnl_barrier_or_error(
    entry: &OpenPitPretradePoliciesSpotFundsPnlBoundsBarrier,
    label: &str,
    index: usize,
    out_error: OpenPitOutError,
) -> Option<SpotFundsPnlBoundsBarrier> {
    let mode = match import_rule_mode(entry.mode) {
        Some(mode) => mode,
        None => {
            write_error_format!(
                out_error,
                "{label}[{index}] mode is invalid: {}",
                entry.mode
            );
            return None;
        }
    };
    let currency = parse_asset_or_error(entry.currency, label, index, "currency", out_error)?;
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
        currency,
        lower_bound,
        upper_bound,
        mode,
    })
}

fn parse_configure_pnl_barrier(
    entry: &OpenPitPretradePoliciesSpotFundsPnlBoundsBarrier,
    label: &str,
    index: usize,
) -> Result<SpotFundsPnlBoundsBarrier, OpenPitConfigureError> {
    let mode = import_rule_mode(entry.mode).ok_or_else(|| {
        OpenPitConfigureError::validation(format!(
            "{label}[{index}] mode is invalid: {}",
            entry.mode
        ))
    })?;
    let currency = parse_configure_asset(entry.currency, label, index, "currency")?;
    let lower_bound = parse_configure_optional_pnl(entry.lower_bound, label, index, "lower_bound")?;
    let upper_bound = parse_configure_optional_pnl(entry.upper_bound, label, index, "upper_bound")?;
    Ok(SpotFundsPnlBoundsBarrier {
        currency,
        lower_bound,
        upper_bound,
        mode,
    })
}

type ParsedPnlBounds = (
    Vec<SpotFundsPnlBoundsBarrier>,
    Vec<SpotFundsPnlBoundsAccountGroupBarrier>,
    Vec<SpotFundsPnlBoundsAccountBarrier>,
);

unsafe fn parse_configure_pnl_bounds(
    global: Option<(
        *const OpenPitPretradePoliciesSpotFundsPnlBoundsBarrier,
        usize,
    )>,
    account_group: Option<(
        *const OpenPitPretradePoliciesSpotFundsPnlBoundsAccountGroupBarrier,
        usize,
    )>,
    account: Option<(
        *const OpenPitPretradePoliciesSpotFundsPnlBoundsAccountBarrier,
        usize,
    )>,
) -> Result<ParsedPnlBounds, OpenPitConfigureError> {
    let mut global_barriers = Vec::new();
    if let Some((global, global_len)) = global {
        let slice = unsafe {
            try_slice_arg(
                global,
                global_len,
                "spot_funds_pnl_bounds global",
                std::ptr::null_mut(),
            )
        }
        .ok_or_else(|| {
            OpenPitConfigureError::validation("spot_funds_pnl_bounds global is null".to_owned())
        })?;
        for (index, entry) in slice.iter().enumerate() {
            global_barriers.push(parse_configure_pnl_barrier(entry, "global", index)?);
        }
    }
    let account_group_barriers = if let Some((account_group, account_group_len)) = account_group {
        let slice = unsafe {
            try_slice_arg(
                account_group,
                account_group_len,
                "spot_funds_pnl_bounds account_group",
                std::ptr::null_mut(),
            )
        }
        .ok_or_else(|| {
            OpenPitConfigureError::validation(
                "spot_funds_pnl_bounds account_group is null".to_owned(),
            )
        })?;
        let mut out = Vec::with_capacity(slice.len());
        for (index, entry) in slice.iter().enumerate() {
            let account_group_id =
                AccountGroupId::from_u32(entry.account_group_id).map_err(|e| {
                    OpenPitConfigureError::validation(format!(
                        "account_group[{index}] account group id is invalid: {e}"
                    ))
                })?;
            let barrier = parse_configure_pnl_barrier(&entry.barrier, "account_group", index)?;
            out.push(SpotFundsPnlBoundsAccountGroupBarrier {
                barrier,
                account_group_id,
            });
        }
        out
    } else {
        Vec::new()
    };
    let account_barriers = if let Some((account, account_len)) = account {
        let slice = unsafe {
            try_slice_arg(
                account,
                account_len,
                "spot_funds_pnl_bounds account",
                std::ptr::null_mut(),
            )
        }
        .ok_or_else(|| {
            OpenPitConfigureError::validation("spot_funds_pnl_bounds account is null".to_owned())
        })?;
        let mut out = Vec::with_capacity(slice.len());
        for (index, entry) in slice.iter().enumerate() {
            let barrier = parse_configure_pnl_barrier(&entry.barrier, "account", index)?;
            out.push(SpotFundsPnlBoundsAccountBarrier {
                barrier,
                account_id: AccountId::from_u64(entry.account_id),
            });
        }
        out
    } else {
        Vec::new()
    };
    Ok((global_barriers, account_group_barriers, account_barriers))
}

fn override_target(
    entry: &OpenPitPretradePoliciesSpotFundsOverride,
) -> Result<SpotFundsOverrideTarget, String> {
    let tag = entry.target.tag;
    if tag == OPENPIT_PRETRADE_POLICIES_SPOT_FUNDS_OVERRIDE_TARGET_TAG_INSTRUMENT {
        let payload = unsafe { entry.target.payload.instrument };
        return Ok(SpotFundsOverrideTarget::Instrument(InstrumentId::new(
            payload.instrument_id,
        )));
    }
    if tag == OPENPIT_PRETRADE_POLICIES_SPOT_FUNDS_OVERRIDE_TARGET_TAG_INSTRUMENT_ACCOUNT {
        let payload = unsafe { entry.target.payload.instrument_account };
        return Ok(SpotFundsOverrideTarget::InstrumentAccount(
            InstrumentId::new(payload.instrument_id),
            AccountId::from_u64(payload.account_id),
        ));
    }
    if tag == OPENPIT_PRETRADE_POLICIES_SPOT_FUNDS_OVERRIDE_TARGET_TAG_INSTRUMENT_ACCOUNT_GROUP {
        let payload = unsafe { entry.target.payload.instrument_account_group };
        let account_group_id = AccountGroupId::from_u32(payload.account_group_id)
            .map_err(|e| format!("spot funds override account group id is invalid: {e}"))?;
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
/// - `instance` supplies the registration name and mode.
///   Register a policy that keeps its own state (Spot Funds, the rate limit,
///   the P&L bounds kill switch, a custom policy that holds funds or counts) in
///   `OPENPIT_PRETRADE_RULE_MODE_REJECT` mode; \ref OpenPitPolicyInstance::mode
///   says what a review mode discards.
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
    instance: *const crate::engine::OpenPitPolicyInstance,
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
    match crate::engine::add_pre_trade_policy_to_builder(builder_ref, policy, instance) {
        Ok(()) => true,
        Err(err) => {
            write_error(out_error, &err);
            false
        }
    }
}

#[no_mangle]
/// Adds the built-in spot-funds policy with account P&L bounds.
///
/// This entry point builds the regular `SpotFundsPolicy` and configures only
/// its realized P&L bounds. The policy keeps its stable built-in name
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
/// - `global` points to `global_len` barriers; zero length means none.
///   Every non-empty array must be non-null.
/// - Each barrier accepts mode 0 or 1 (Reject), 2 (ReviewWithHold), or
///   3 (ReviewWithoutHold); unknown values are errors.
/// - Repeated global modes, (group, mode), or (account, mode) fail the call
///   through `out_error`; the duplicate message names the mode.
/// - Each `currency` string view inside a barrier must be valid for the
///   duration of the call.
/// - Barrier configuration never seeds or resets live account P&L.
/// - `instance` supplies the registration name and mode.
///   Register a policy that keeps its own state (Spot Funds, the rate limit,
///   the P&L bounds kill switch, a custom policy that holds funds or counts) in
///   `OPENPIT_PRETRADE_RULE_MODE_REJECT` mode; \ref OpenPitPolicyInstance::mode
///   says what a review mode discards.
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
    instance: *const crate::engine::OpenPitPolicyInstance,
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
        Some(slice) => slice,
        None => return false,
    };
    let mut global_barriers = Vec::with_capacity(global_slice.len());
    for (index, entry) in global_slice.iter().enumerate() {
        match parse_pnl_barrier_or_error(entry, "global", index, out_error) {
            Some(barrier) => global_barriers.push(barrier),
            None => return false,
        }
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
                    "account_group[{index}] account group id is invalid: {e}",
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
        account_barriers.push(SpotFundsPnlBoundsAccountBarrier {
            barrier,
            account_id: AccountId::from_u64(entry.account_id),
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

    let builder_ref = unsafe { &mut *builder };
    let storage_builder = match policy_storage(builder_ref) {
        Some(s) => s,
        None => {
            write_error(out_error, "engine builder is no longer available");
            return false;
        }
    };
    let policy = match SpotFundsPolicy::<EngineLocking, EngineLocking>::pnl_bounds_kill_switch(
        global_barriers,
        account_group_barriers,
        account_barriers,
        market_orders,
        storage_builder,
    ) {
        Ok(policy) => policy.with_policy_group_id(PolicyGroupId::new(policy_group_id)),
        Err(e) => {
            write_error_format!(out_error, "spot funds pnl barriers invalid: {}", e);
            return false;
        }
    };
    match crate::engine::add_pre_trade_policy_to_builder(builder_ref, policy, instance) {
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
/// Concurrency: caller-guaranteed exclusive in every mode, including
/// `OpenPitSyncPolicy_Full`. No other engine call may be in flight, including
/// custom callbacks. Debug builds detect overlap; release builds do not wait.
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
    finish_configure_spot_funds(result, out_error)
}

#[no_mangle]
/// Retunes the realized P&L bounds of the built-in spot-funds policy registered
/// under `name`.
///
/// Concurrency: caller-guaranteed exclusive in every mode, including
/// `OpenPitSyncPolicy_Full`. No other engine call may be in flight, including
/// custom callbacks. Debug builds detect overlap; release builds do not wait.
///
/// Contract:
/// - `engine` must be a valid non-null engine pointer.
/// - `name` selects the policy; it is interpreted as UTF-8. A built-in policy
///   added via
///   openpit_engine_builder_add_builtin_spot_funds_pnl_bounds_killswitch_policy
///   registers under its fixed name `"SpotFundsPolicy"`, so pass that string
///   here.
/// - This is a partial update (PATCH): each of the three tiers (global,
///   `account_group`, `account`) is replaced only when its `has_*` flag is
///   `true`; a `has_*` flag set to `false` leaves that tier unchanged.
/// - `has_global == false` ignores `global` and `global_len`. When true,
///   `global_len == 0` clears all global modes; otherwise the array replaces
///   all global modes and must be non-null.
/// - Modes are 0 or 1 Reject, 2 ReviewWithHold, 3 ReviewWithoutHold.
///   Unknown modes return Validation. Repeated global modes, (group, mode),
///   or (account, mode) return Validation naming the mode, without changes.
/// - A Reject-mode breach blocks the account. A review-mode breach adds a
///   review mark on evaluation without blocking; evaluation failures block
///   in every mode.
/// - When `has_account_group` is `true`, the `account_group_len` entries at
///   `account_group` replace the whole per-account-group tier; an engaged
///   empty list (`has_account_group == true`, `account_group_len == 0`)
///   clears it.
/// - When `has_account` is `true`, the `account_len` entries at `account`
///   replace the whole per-account tier; an engaged empty list
///   (`has_account == true`, `account_len == 0`) clears it.
/// - Each `currency` string view inside a barrier must be valid for the
///   duration of the call.
/// - Barrier retuning never resets a live accumulated P&L value.
///
/// Success:
/// - returns a caller-owned account-block-outcome list, possibly empty; release
///   it with `openpit_destroy_pretrade_account_block_outcome_list`. Each
///   outcome pairs the engine-selected account with the block inserted for it.
///   Subsequent P&L-bound
///   evaluations use the new bounds, including pre-trade checks, execution
///   reports, account-P&L adjustments and account-P&L force-sets. An account
///   whose effective barrier changed is evaluated against its stored account
///   P&L here: an already halted account, or one beyond a new Reject-mode
///   barrier, is blocked before this call returns and appears in the list,
///   which reports the blocks the engine has already recorded. Removing the
///   last effective barrier reports no block and does not release an existing
///   block. Clearing an override can expose a fallback barrier; the fallback
///   is evaluated normally and may record and report a block.
///
/// Error:
/// - returns null; if `out_error` is non-null, writes a caller-owned
///   `OpenPitConfigureError` (release with `openpit_destroy_configure_error`).
/// - a null `engine` returns null and, when `out_error` is non-null, writes
///   a caller-owned `OpenPitConfigureError` (`Validation`) that must be
///   released with `openpit_destroy_configure_error`.
pub unsafe extern "C" fn openpit_engine_configure_spot_funds_pnl_bounds_killswitch(
    engine: *mut crate::engine::OpenPitEngine,
    name: OpenPitStringView,
    global: *const OpenPitPretradePoliciesSpotFundsPnlBoundsBarrier,
    global_len: usize,
    has_global: bool,
    account_group: *const OpenPitPretradePoliciesSpotFundsPnlBoundsAccountGroupBarrier,
    account_group_len: usize,
    has_account_group: bool,
    account: *const OpenPitPretradePoliciesSpotFundsPnlBoundsAccountBarrier,
    account_len: usize,
    has_account: bool,
    out_error: *mut *mut OpenPitConfigureError,
) -> *mut OpenPitPretradeAccountBlockOutcomeList {
    let name = match unsafe { configure_spot_funds_name(engine, name, out_error) } {
        Some(name) => name,
        None => return std::ptr::null_mut(),
    };

    let (global_barriers, account_group_barriers, account_barriers) = match unsafe {
        parse_configure_pnl_bounds(
            has_global.then_some((global, global_len)),
            has_account_group.then_some((account_group, account_group_len)),
            has_account.then_some((account, account_len)),
        )
    } {
        Ok(barriers) => barriers,
        Err(error) => {
            write_configure_error(out_error, error);
            return std::ptr::null_mut();
        }
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
    match result {
        Ok(result) => Box::into_raw(Box::new(block_outcomes_to_list_owned(
            result.account_blocks,
        ))),
        Err(error) => {
            write_configure_error(out_error, OpenPitConfigureError::new(error));
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
/// Retunes one P&L-bounds axis of the built-in spot-funds policy under `name`.
///
/// Concurrency: caller-guaranteed exclusive in every mode, including
/// `OpenPitSyncPolicy_Full`. No other engine call may be in flight, including
/// custom callbacks. Debug builds detect overlap; release builds do not wait.
///
/// Contract:
/// - `engine` must be a valid non-null engine pointer.
/// - `name` selects the policy and is interpreted as UTF-8. The built-in
///   spot-funds builder functions register under `"SpotFundsPolicy"`.
/// - `axis` selects `AccountUnrealized` (0), `AccountTotal` (1),
///   `PositionUnrealized` (2), or `PositionTotal` (3), as defined by
///   `OpenPitPretradePoliciesSpotFundsPnlBoundsAxis`.
/// - Unrealized P&L is what a full close of owned quantity at the current
///   price would add to realized P&L. Every position uses the same bounds.
/// - This is a partial update (PATCH): the global, account-group and account
///   tiers are each replaced only when their `has_*` flag is `true`. A false
///   flag leaves that tier unchanged and its arguments are ignored.
/// - `has_global == false` ignores `global` and `global_len`. When true,
///   `global_len == 0` clears all global modes; otherwise the array replaces
///   all global modes and must be non-null.
/// - Modes are 0 or 1 Reject, 2 ReviewWithHold, 3 ReviewWithoutHold.
///   Unknown modes return Validation. Repeated global modes, (group, mode),
///   or (account, mode) return Validation naming the mode, without changes.
/// - When `has_account_group` is `true`, the `account_group_len` entries at
///   `account_group` replace that whole tier; an engaged empty list clears it.
/// - When `has_account` is `true`, the `account_len` entries at `account`
///   replace that whole tier; an engaged empty list clears it.
/// - Engaged slices with non-zero lengths must be non-null. Each barrier's
///   `currency` string view must be valid for the duration of the call.
/// - Only the selected axis changes; realized bounds and P&L stay unchanged.
/// - With an effective barrier, a Reject-mode breach or a halt in any mode
///   blocks with `PnlKillSwitchTriggered` at pre-trade, dry run, a fill, the
///   account P&L force-set, and a retune that changes the effective barrier.
///   A fill checks axes for a trade or non-zero fee when the report gave no
///   block of its own. Moving an account to another account group does not
///   check the axes; the next order catches the breach.
///   A review-mode breach yields a review mark without blocking.
///
/// Success:
/// - Returns a caller-owned account-block-outcome list, possibly empty;
///   release it with `openpit_destroy_pretrade_account_block_outcome_list`.
///   Each outcome pairs the engine-selected account with its recorded block.
///   Accounts whose effective barrier changes are evaluated before return.
///   Removing the last effective barrier reports no block and does not
///   release an existing block. Clearing an override can expose a lower-tier
///   barrier that is evaluated and may record and report a block.
///
/// Error:
/// - Returns null; if `out_error` is non-null, writes a caller-owned
///   `OpenPitConfigureError` (release with `openpit_destroy_configure_error`).
/// - A null `engine`, null / invalid-UTF-8 `name`, invalid `axis`, or invalid
///   barrier argument returns `Validation`.
/// - An unregistered `name` returns `Unknown`; a name registered to a policy
///   other than spot-funds returns `TypeMismatch`.
/// - In every error case, the settings stay unchanged.
pub unsafe extern "C" fn openpit_engine_configure_spot_funds_pnl_bounds_axis(
    engine: *mut crate::engine::OpenPitEngine,
    name: OpenPitStringView,
    axis: OpenPitPretradePoliciesSpotFundsPnlBoundsAxis,
    global: *const OpenPitPretradePoliciesSpotFundsPnlBoundsBarrier,
    global_len: usize,
    has_global: bool,
    account_group: *const OpenPitPretradePoliciesSpotFundsPnlBoundsAccountGroupBarrier,
    account_group_len: usize,
    has_account_group: bool,
    account: *const OpenPitPretradePoliciesSpotFundsPnlBoundsAccountBarrier,
    account_len: usize,
    has_account: bool,
    out_error: *mut *mut OpenPitConfigureError,
) -> *mut OpenPitPretradeAccountBlockOutcomeList {
    let name = match unsafe { configure_spot_funds_name(engine, name, out_error) } {
        Some(name) => name,
        None => return std::ptr::null_mut(),
    };
    let axis = match import_spot_funds_pnl_bounds_axis(axis) {
        Ok(axis) => axis,
        Err(error) => {
            write_configure_error(out_error, error);
            return std::ptr::null_mut();
        }
    };
    let (global_barriers, account_group_barriers, account_barriers) = match unsafe {
        parse_configure_pnl_bounds(
            has_global.then_some((global, global_len)),
            has_account_group.then_some((account_group, account_group_len)),
            has_account.then_some((account, account_len)),
        )
    } {
        Ok(barriers) => barriers,
        Err(error) => {
            write_configure_error(out_error, error);
            return std::ptr::null_mut();
        }
    };
    let result = unsafe { &*engine }.configurator().spot_funds(
        &name,
        |settings| -> Result<(), SpotFundsConfigError> {
            if has_global {
                settings.set_pnl_axis_global_barriers(axis, global_barriers.iter().cloned())?;
            }
            if has_account_group {
                settings.set_pnl_axis_account_group_barriers(
                    axis,
                    account_group_barriers.iter().cloned(),
                )?;
            }
            if has_account {
                settings.set_pnl_axis_account_barriers(axis, account_barriers.iter().cloned())?;
            }
            Ok(())
        },
    );
    match result {
        Ok(result) => Box::into_raw(Box::new(block_outcomes_to_list_owned(
            result.account_blocks,
        ))),
        Err(error) => {
            write_configure_error(out_error, OpenPitConfigureError::new(error));
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
/// Force-sets the live accumulated account P&L state for the spot-funds policy
/// registered under `name`.
///
/// This is an absolute assignment and is separate from barrier retuning, which
/// never resets the accumulator. A numeric state re-arms this account
/// accumulator after a calculation halt. A halted state sets or keeps it
/// halted and replaces the stored halt reason. Neither form affects a
/// position-level accumulator. A numeric state beyond an effective Reject-mode
/// bound returns the policy-reported `PnlKillSwitchTriggered` block. A breach of
/// a review-mode bound returns no block and no review mark: configuration does
/// not issue review marks. A halted state with an effective barrier of any mode
/// returns the block. The engine processes that block request before returning.
/// An existing first-cause block remains unchanged but does not suppress the
/// returned block.
///
/// Contract:
/// - on success, returns a caller-owned account-block list, possibly empty;
///   release it with `openpit_destroy_pretrade_account_block_list`;
/// - on failure, returns null and, when `out_error` is non-null, writes a
///   caller-owned `OpenPitConfigureError` that must be released with
///   `openpit_destroy_configure_error`.
pub unsafe extern "C" fn openpit_engine_configure_spot_funds_set_account_pnl(
    engine: *mut crate::engine::OpenPitEngine,
    name: OpenPitStringView,
    account_id: OpenPitParamAccountId,
    state: OpenPitPnlState,
    out_error: *mut *mut OpenPitConfigureError,
) -> *mut OpenPitPretradeAccountBlockList {
    let name = match unsafe { configure_spot_funds_name(engine, name, out_error) } {
        Some(name) => name,
        None => return std::ptr::null_mut(),
    };
    let state = match import_pnl_state(state) {
        Ok(state) => state,
        Err(error) => {
            write_configure_error(
                out_error,
                OpenPitConfigureError::validation(format!("pnl state is invalid: {error}")),
            );
            return std::ptr::null_mut();
        }
    };

    let result = unsafe { &*engine }
        .configurator()
        .set_spot_funds_account_pnl(&name, AccountId::from_u64(account_id), state);
    match result {
        Ok(result) => Box::into_raw(Box::new(blocks_to_list_owned(result.account_blocks))),
        Err(error) => {
            write_configure_error(out_error, OpenPitConfigureError::new(error));
            std::ptr::null_mut()
        }
    }
}

/// Sets the global spot-funds limit mode for the policy registered under
/// `name`.
///
/// The global mode applies to every order that resolves to neither a
/// per-account nor a per-account-group override.
///
/// Concurrency: caller-guaranteed exclusive in every mode, including
/// `OpenPitSyncPolicy_Full`. No other engine call may be in flight, including
/// custom callbacks. Debug builds detect overlap; release builds do not wait.
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
    mode: OpenPitPretradePoliciesSpotFundsLimitMode,
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
/// Concurrency: caller-guaranteed exclusive in every mode, including
/// `OpenPitSyncPolicy_Full`. No other engine call may be in flight, including
/// custom callbacks. Debug builds detect overlap; release builds do not wait.
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
    mode: OpenPitPretradePoliciesSpotFundsLimitMode,
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

/// Pins or clears the spot-funds position limit for one account, asset and
/// rule mode on the policy registered under `name`.
///
/// Concurrency: caller-guaranteed exclusive in every mode, including
/// `OpenPitSyncPolicy_Full`. No other engine call may be in flight, including
/// custom callbacks. Debug builds detect overlap; release builds do not wait.
///
/// Contract:
/// - `engine` must be a valid non-null engine pointer.
/// - `name` selects the policy; see
///   `openpit_engine_configure_spot_funds_global_limit_mode`.
/// - `(account_id, asset, mode)` selects one limit; `asset` must be valid
///   UTF-8 and a valid asset. `mode` is 0 or 1 (Reject), 2 (ReviewWithHold), or
///   3 (ReviewWithoutHold). Unknown values are invalid.
/// - When `has_limit` is `true`, `limit` pins or replaces the inclusive
///   non-negative long and short magnitude for this position. The checked
///   position starts with recorded `available + held`, including a negative
///   `held` residual. A long projection adds open positive `incoming`; a short
///   projection subtracts open positive `held`. Only the side the order moves
///   toward is checked, so a position-reducing order passes while that
///   projection stays within the limit. `limit` must be a valid non-negative
///   quantity. When `has_limit` is `false`, only this mode's limit is cleared
///   and `limit` is ignored. Other modes' limits are unchanged by either call.
/// - A Reject breach rejects with `PositionLimitExceeded` at Order scope.
///   A review breach adds a `PositionLimitExceeded` review mark with the
///   limit's mode. A projection that cannot be computed exactly rejects with
///   `ArithmeticOverflow` at Order scope in every mode. A change applies from
///   the next order and does not re-evaluate open reservations.
///
/// Success / error: as
/// `openpit_engine_configure_spot_funds_global_limit_mode`. An invalid
/// `asset`, `mode` or supplied `limit` returns `false` with a `Validation` error
/// naming the invalid input.
#[no_mangle]
pub unsafe extern "C" fn openpit_engine_configure_spot_funds_position_limit(
    engine: *mut crate::engine::OpenPitEngine,
    name: OpenPitStringView,
    account_id: OpenPitParamAccountId,
    asset: OpenPitStringView,
    mode: OpenPitPretradeRuleMode,
    limit: OpenPitParamQuantity,
    has_limit: bool,
    out_error: *mut *mut OpenPitConfigureError,
) -> bool {
    let name = match unsafe { configure_spot_funds_name(engine, name, out_error) } {
        Some(name) => name,
        None => return false,
    };
    let asset = match parse_configure_asset(asset, "position_limit", 0, "asset") {
        Ok(asset) => asset,
        Err(err) => {
            write_configure_error(out_error, err);
            return false;
        }
    };
    let mode = match import_rule_mode(mode) {
        Some(mode) => mode,
        None => {
            write_configure_error(
                out_error,
                OpenPitConfigureError::validation(format!(
                    "position_limit mode is invalid: {mode}"
                )),
            );
            return false;
        }
    };
    let limit = if has_limit {
        match limit.to_param() {
            Ok(limit) => Some(limit),
            Err(err) => {
                write_configure_error(
                    out_error,
                    OpenPitConfigureError::validation(format!(
                        "position_limit limit is invalid: {err}"
                    )),
                );
                return false;
            }
        }
    } else {
        None
    };
    let account_id = AccountId::from_u64(account_id);
    let result = unsafe { &*engine }.configurator().spot_funds(
        &name,
        |settings| -> Result<(), SpotFundsConfigError> {
            settings.set_position_limit(account_id, asset, mode, limit);
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
/// Concurrency: caller-guaranteed exclusive in every mode, including
/// `OpenPitSyncPolicy_Full`. No other engine call may be in flight, including
/// custom callbacks. Debug builds detect overlap; release builds do not wait.
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
    mode: OpenPitPretradePoliciesSpotFundsLimitMode,
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
                    "spot funds account group id is invalid: {error}"
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
/// configure entry points, returning the decoded name or writing a
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

/// Maps the configurator result of a spot-funds update to the FFI
/// boolean convention, writing a caller-owned `OpenPitConfigureError` on
/// failure.
fn finish_configure_spot_funds(
    result: Result<openpit::pretrade::AccountBlockOutcomes, openpit::ConfigureError>,
    out_error: *mut *mut OpenPitConfigureError,
) -> bool {
    match result {
        // These retunes cannot change a P&L barrier, so the reported block
        // list is always empty.
        Ok(_) => true,
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
    use crate::param::OpenPitParamDecimal;
    use crate::string::openpit_destroy_shared_string;
    use openpit::pretrade::RuleMode;

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
                tag: OPENPIT_PRETRADE_POLICIES_SPOT_FUNDS_OVERRIDE_TARGET_TAG_INSTRUMENT,
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
                tag: OPENPIT_PRETRADE_POLICIES_SPOT_FUNDS_OVERRIDE_TARGET_TAG_INSTRUMENT_ACCOUNT,
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
                tag: OPENPIT_PRETRADE_POLICIES_SPOT_FUNDS_OVERRIDE_TARGET_TAG_INSTRUMENT_ACCOUNT_GROUP,
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
                std::ptr::null(),
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
                std::ptr::null(),
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
    fn add_builtin_spot_funds_pnl_bounds_delegates_to_policy_preset() {
        let builder = make_builder();
        let global = OpenPitPretradePoliciesSpotFundsPnlBoundsBarrier {
            mode: crate::reject::OPENPIT_PRETRADE_RULE_MODE_REJECT,
            currency: OpenPitStringView::from_utf8("USD"),
            lower_bound: OpenPitParamPnlOptional {
                value: crate::param::OpenPitParamPnl(
                    Pnl::from_str("-100").expect("pnl").to_decimal().into(),
                ),
                is_set: true,
            },
            upper_bound: OpenPitParamPnlOptional::default(),
        };

        assert!(unsafe {
            openpit_engine_builder_add_builtin_spot_funds_pnl_bounds_killswitch_policy(
                builder,
                std::ptr::null(),
                7,
                &global,
                1,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
                null_out_error(),
            )
        });

        let engine = crate::engine::openpit_engine_builder_build(
            builder,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
        assert!(!engine.is_null());
        crate::engine::openpit_destroy_engine(engine);
        openpit_destroy_engine_builder(builder);
    }

    fn pnl_bounds_barrier(lower: &str) -> OpenPitPretradePoliciesSpotFundsPnlBoundsBarrier {
        OpenPitPretradePoliciesSpotFundsPnlBoundsBarrier {
            mode: crate::reject::OPENPIT_PRETRADE_RULE_MODE_REJECT,
            currency: OpenPitStringView::from_utf8("USD"),
            lower_bound: OpenPitParamPnlOptional {
                value: crate::param::OpenPitParamPnl(
                    Pnl::from_str(lower).unwrap().to_decimal().into(),
                ),
                is_set: true,
            },
            upper_bound: OpenPitParamPnlOptional::default(),
        }
    }

    const PNL_BOUNDS_AXES: [(u8, SpotFundsPnlBoundsAxis); 4] = [
        (
            OPENPIT_PRETRADE_POLICIES_SPOT_FUNDS_PNL_BOUNDS_AXIS_ACCOUNT_UNREALIZED,
            SpotFundsPnlBoundsAxis::AccountUnrealized,
        ),
        (
            OPENPIT_PRETRADE_POLICIES_SPOT_FUNDS_PNL_BOUNDS_AXIS_ACCOUNT_TOTAL,
            SpotFundsPnlBoundsAxis::AccountTotal,
        ),
        (
            OPENPIT_PRETRADE_POLICIES_SPOT_FUNDS_PNL_BOUNDS_AXIS_POSITION_UNREALIZED,
            SpotFundsPnlBoundsAxis::PositionUnrealized,
        ),
        (
            OPENPIT_PRETRADE_POLICIES_SPOT_FUNDS_PNL_BOUNDS_AXIS_POSITION_TOTAL,
            SpotFundsPnlBoundsAxis::PositionTotal,
        ),
    ];

    fn add_pnl_bounds(
        builder: *mut crate::engine::OpenPitEngineBuilder,
        global: &[OpenPitPretradePoliciesSpotFundsPnlBoundsBarrier],
        groups: &[OpenPitPretradePoliciesSpotFundsPnlBoundsAccountGroupBarrier],
        accounts: &[OpenPitPretradePoliciesSpotFundsPnlBoundsAccountBarrier],
    ) -> Result<(), String> {
        let mut error = std::ptr::null_mut();
        let ok = unsafe {
            openpit_engine_builder_add_builtin_spot_funds_pnl_bounds_killswitch_policy(
                builder,
                std::ptr::null(),
                0,
                global.as_ptr(),
                global.len(),
                groups.as_ptr(),
                groups.len(),
                accounts.as_ptr(),
                accounts.len(),
                std::ptr::null(),
                &mut error,
            )
        };
        if ok {
            assert!(error.is_null());
            Ok(())
        } else {
            assert!(!error.is_null(), "builder failure returns out_error");
            Err(cstr_to_string(error))
        }
    }

    fn patch_pnl_bounds(
        engine: *mut crate::engine::OpenPitEngine,
        axis: Option<u8>,
        global: &[OpenPitPretradePoliciesSpotFundsPnlBoundsBarrier],
        has_global: bool,
        groups: Option<&[OpenPitPretradePoliciesSpotFundsPnlBoundsAccountGroupBarrier]>,
        accounts: Option<&[OpenPitPretradePoliciesSpotFundsPnlBoundsAccountBarrier]>,
    ) -> Result<(), String> {
        let mut error = std::ptr::null_mut();
        let group_entries = groups.unwrap_or(&[]);
        let account_entries = accounts.unwrap_or(&[]);
        let name = OpenPitStringView::from_utf8("SpotFundsPolicy");
        let result = unsafe {
            match axis {
                None => openpit_engine_configure_spot_funds_pnl_bounds_killswitch(
                    engine,
                    name,
                    global.as_ptr(),
                    global.len(),
                    has_global,
                    group_entries.as_ptr(),
                    group_entries.len(),
                    groups.is_some(),
                    account_entries.as_ptr(),
                    account_entries.len(),
                    accounts.is_some(),
                    &mut error,
                ),
                Some(axis) => openpit_engine_configure_spot_funds_pnl_bounds_axis(
                    engine,
                    name,
                    axis,
                    global.as_ptr(),
                    global.len(),
                    has_global,
                    group_entries.as_ptr(),
                    group_entries.len(),
                    groups.is_some(),
                    account_entries.as_ptr(),
                    account_entries.len(),
                    accounts.is_some(),
                    &mut error,
                ),
            }
        };
        if result.is_null() {
            assert!(
                !error.is_null(),
                "configure failure returns structured error"
            );
            assert_eq!(
                crate::engine::openpit_configure_error_get_kind(error),
                crate::engine::OpenPitConfigureErrorKind::Validation,
                "configure error is Validation"
            );
            let message =
                unsafe { cstr_arg(crate::engine::openpit_configure_error_get_message(error)) }
                    .unwrap()
                    .to_owned();
            crate::engine::openpit_destroy_configure_error(error);
            Err(message)
        } else {
            assert!(error.is_null());
            crate::reject::openpit_destroy_pretrade_account_block_outcome_list(result);
            Ok(())
        }
    }

    #[test]
    fn spot_funds_pnl_modes_round_trip_through_builder() {
        for (wire, mode) in [
            (0, RuleMode::Reject),
            (1, RuleMode::Reject),
            (2, RuleMode::ReviewWithHold),
            (3, RuleMode::ReviewWithoutHold),
        ] {
            let builder = make_local_engine_builder();
            let barrier = OpenPitPretradePoliciesSpotFundsPnlBoundsBarrier {
                mode: wire,
                ..pnl_bounds_barrier("-100")
            };
            let groups = [
                OpenPitPretradePoliciesSpotFundsPnlBoundsAccountGroupBarrier {
                    barrier,
                    account_group_id: 7,
                },
            ];
            let accounts = [OpenPitPretradePoliciesSpotFundsPnlBoundsAccountBarrier {
                barrier,
                account_id: 42,
            }];
            add_pnl_bounds(builder, &[barrier], &groups, &accounts).unwrap();
            let engine = crate::engine::openpit_engine_builder_build(
                builder,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            assert!(!engine.is_null());
            unsafe { &*engine }
                .configurator()
                .spot_funds("SpotFundsPolicy", |settings| {
                    let mut expected = settings.clone();
                    let barrier = SpotFundsPnlBoundsBarrier {
                        currency: openpit::param::Asset::new("USD").unwrap(),
                        lower_bound: Some(Pnl::from_str("-100").unwrap()),
                        upper_bound: None,
                        mode,
                    };
                    expected.set_pnl_global_barriers([barrier.clone()]).unwrap();
                    expected
                        .set_pnl_account_group_barriers([SpotFundsPnlBoundsAccountGroupBarrier {
                            barrier: barrier.clone(),
                            account_group_id: AccountGroupId::from_u32(7).unwrap(),
                        }])
                        .unwrap();
                    expected
                        .set_pnl_account_barriers([SpotFundsPnlBoundsAccountBarrier {
                            barrier,
                            account_id: AccountId::from_u64(42),
                        }])
                        .unwrap();
                    assert_eq!(
                        format!("{settings:?}"),
                        format!("{expected:?}"),
                        "every mode reaches every configured tier"
                    );
                    Ok::<(), SpotFundsConfigError>(())
                })
                .unwrap();
            crate::engine::openpit_destroy_engine(engine);
            openpit_destroy_engine_builder(builder);
        }
        let builder = make_local_engine_builder();
        let globals = [
            pnl_bounds_barrier("-100"),
            OpenPitPretradePoliciesSpotFundsPnlBoundsBarrier {
                mode: crate::reject::OPENPIT_PRETRADE_RULE_MODE_REVIEW_WITH_HOLD,
                ..pnl_bounds_barrier("-10")
            },
        ];
        add_pnl_bounds(builder, &globals, &[], &[]).unwrap();
        let engine = crate::engine::openpit_engine_builder_build(
            builder,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
        assert!(!engine.is_null());
        let snapshot = position_limit_settings_snapshot(engine);
        assert!(
            snapshot.contains("Reject: SpotFundsPnlBoundsBarrier"),
            "Reject global coexists"
        );
        assert!(
            snapshot.contains("ReviewWithHold: SpotFundsPnlBoundsBarrier"),
            "review global coexists"
        );
        crate::engine::openpit_destroy_engine(engine);
        openpit_destroy_engine_builder(builder);
    }

    #[test]
    fn spot_funds_pnl_invalid_modes_fail_without_change() {
        for mode in [4, 255] {
            let invalid = OpenPitPretradePoliciesSpotFundsPnlBoundsBarrier {
                mode,
                ..pnl_bounds_barrier("-10")
            };
            for tier in 0..3 {
                let groups = [
                    OpenPitPretradePoliciesSpotFundsPnlBoundsAccountGroupBarrier {
                        barrier: invalid,
                        account_group_id: 7,
                    },
                ];
                let accounts = [OpenPitPretradePoliciesSpotFundsPnlBoundsAccountBarrier {
                    barrier: invalid,
                    account_id: 42,
                }];
                let globals = if tier == 0 {
                    std::slice::from_ref(&invalid)
                } else {
                    &[]
                };
                let group_entries = if tier == 1 { &groups[..] } else { &[] };
                let account_entries = if tier == 2 { &accounts[..] } else { &[] };
                let builder = make_local_engine_builder();
                let error = add_pnl_bounds(builder, globals, group_entries, account_entries)
                    .expect_err("invalid mode must fail builder");
                assert!(
                    error.contains(&format!("mode is invalid: {mode}")),
                    "{error}"
                );
                add_pnl_bounds(builder, &[pnl_bounds_barrier("-100")], &[], &[]).unwrap();
                openpit_destroy_engine_builder(builder);
                for axis in [None, Some(0)] {
                    let engine = engine_with_spot_funds();
                    patch_pnl_bounds(
                        engine,
                        axis,
                        &[pnl_bounds_barrier("-100")],
                        true,
                        None,
                        None,
                    )
                    .unwrap();
                    let before = position_limit_settings_snapshot(engine);
                    let error = patch_pnl_bounds(
                        engine,
                        axis,
                        globals,
                        true,
                        Some(group_entries),
                        Some(account_entries),
                    )
                    .expect_err("invalid mode must fail configure");
                    assert!(
                        error.contains(&format!("mode is invalid: {mode}")),
                        "{error}"
                    );
                    assert_eq!(
                        position_limit_settings_snapshot(engine),
                        before,
                        "invalid mode leaves every tier unchanged"
                    );
                    crate::engine::openpit_destroy_engine(engine);
                }
            }
        }
    }

    fn assert_pnl_duplicate_tier_fails(tier: usize) {
        let barrier = OpenPitPretradePoliciesSpotFundsPnlBoundsBarrier {
            mode: crate::reject::OPENPIT_PRETRADE_RULE_MODE_REVIEW_WITH_HOLD,
            ..pnl_bounds_barrier("-10")
        };
        let groups = [OpenPitPretradePoliciesSpotFundsPnlBoundsAccountGroupBarrier {
            barrier,
            account_group_id: 7,
        }; 2];
        let accounts = [OpenPitPretradePoliciesSpotFundsPnlBoundsAccountBarrier {
            barrier,
            account_id: 42,
        }; 2];
        let globals = if tier == 0 { &[barrier; 2][..] } else { &[] };
        let group_entries = if tier == 1 { &groups[..] } else { &[] };
        let account_entries = if tier == 2 { &accounts[..] } else { &[] };
        let builder = make_local_engine_builder();
        let error = add_pnl_bounds(builder, globals, group_entries, account_entries)
            .expect_err("duplicate mode must fail builder");
        assert!(
            error.contains("duplicate") && error.contains("ReviewWithHold"),
            "{error}"
        );
        add_pnl_bounds(builder, &[pnl_bounds_barrier("-100")], &[], &[]).unwrap();
        openpit_destroy_engine_builder(builder);
        for axis in [None, Some(0)] {
            let engine = engine_with_spot_funds();
            patch_pnl_bounds(
                engine,
                axis,
                &[pnl_bounds_barrier("-100")],
                true,
                None,
                None,
            )
            .unwrap();
            let before = position_limit_settings_snapshot(engine);
            let error = patch_pnl_bounds(
                engine,
                axis,
                globals,
                true,
                Some(group_entries),
                Some(account_entries),
            )
            .expect_err("duplicate mode must fail configure");
            assert!(
                error.contains("duplicate") && error.contains("ReviewWithHold"),
                "{error}"
            );
            assert_eq!(
                position_limit_settings_snapshot(engine),
                before,
                "duplicate mode leaves every tier unchanged"
            );
            crate::engine::openpit_destroy_engine(engine);
        }
    }

    #[test]
    fn spot_funds_pnl_duplicate_global_modes_fail_without_change() {
        assert_pnl_duplicate_tier_fails(0);
    }

    #[test]
    fn spot_funds_pnl_duplicate_group_modes_fail_without_change() {
        assert_pnl_duplicate_tier_fails(1);
    }

    #[test]
    fn spot_funds_pnl_duplicate_account_modes_fail_without_change() {
        assert_pnl_duplicate_tier_fails(2);
    }

    #[test]
    fn spot_funds_pnl_global_lists_patch_all_modes() {
        for axis in [None, Some(0)] {
            let engine = engine_with_spot_funds();
            let before = position_limit_settings_snapshot(engine);
            let initial = [
                pnl_bounds_barrier("-100"),
                OpenPitPretradePoliciesSpotFundsPnlBoundsBarrier {
                    mode: crate::reject::OPENPIT_PRETRADE_RULE_MODE_REVIEW_WITH_HOLD,
                    ..pnl_bounds_barrier("-10")
                },
            ];
            patch_pnl_bounds(engine, axis, &initial, true, None, None).unwrap();
            let initial_snapshot = position_limit_settings_snapshot(engine);
            assert_ne!(initial_snapshot, before, "list installs global modes");
            let invalid = [OpenPitPretradePoliciesSpotFundsPnlBoundsBarrier {
                mode: 255,
                ..initial[0]
            }];
            patch_pnl_bounds(engine, axis, &invalid, false, None, None).unwrap();
            assert_eq!(
                position_limit_settings_snapshot(engine),
                initial_snapshot,
                "has_global false ignores its array and preserves every mode"
            );
            let replacement = [OpenPitPretradePoliciesSpotFundsPnlBoundsBarrier {
                mode: crate::reject::OPENPIT_PRETRADE_RULE_MODE_REVIEW_WITHOUT_HOLD,
                ..pnl_bounds_barrier("-50")
            }];
            patch_pnl_bounds(engine, axis, &replacement, true, None, None).unwrap();
            let replacement_snapshot = position_limit_settings_snapshot(engine);
            patch_pnl_bounds(engine, axis, &[], true, None, None).unwrap();
            assert_eq!(
                position_limit_settings_snapshot(engine),
                before,
                "empty list clears every mode"
            );
            patch_pnl_bounds(engine, axis, &replacement, true, None, None).unwrap();
            assert_eq!(
                position_limit_settings_snapshot(engine),
                replacement_snapshot,
                "list replaces all previous modes instead of upserting"
            );
            crate::engine::openpit_destroy_engine(engine);
        }
    }

    #[test]
    fn spot_funds_pnl_zero_mode_matches_reject_at_runtime() {
        for axis in [None, Some(0), Some(1), Some(2), Some(3)] {
            let engine = engine_with_spot_funds();
            let mut barrier = pnl_bounds_barrier("-100");
            let mut groups = [
                OpenPitPretradePoliciesSpotFundsPnlBoundsAccountGroupBarrier {
                    barrier,
                    account_group_id: 7,
                },
            ];
            let mut accounts = [OpenPitPretradePoliciesSpotFundsPnlBoundsAccountBarrier {
                barrier,
                account_id: 42,
            }];
            patch_pnl_bounds(
                engine,
                axis,
                &[barrier],
                true,
                Some(&groups),
                Some(&accounts),
            )
            .unwrap();
            let expected = position_limit_settings_snapshot(engine);
            barrier.mode = 0;
            groups[0].barrier.mode = 0;
            accounts[0].barrier.mode = 0;
            patch_pnl_bounds(
                engine,
                axis,
                &[barrier],
                true,
                Some(&groups),
                Some(&accounts),
            )
            .unwrap();
            assert_eq!(
                position_limit_settings_snapshot(engine),
                expected,
                "zero selects the Reject slot in every tier, axis {axis:?}"
            );
            crate::engine::openpit_destroy_engine(engine);
        }
    }

    #[test]
    fn configure_spot_funds_pnl_bounds_axis_selects_each_axis() {
        for (wire, (selector, axis)) in PNL_BOUNDS_AXES.into_iter().enumerate() {
            assert_eq!(usize::from(selector), wire, "stable axis wire value");
            let engine = engine_with_spot_funds();
            let barrier = pnl_bounds_barrier("-100");
            let mut error = std::ptr::null_mut();
            let result = unsafe {
                openpit_engine_configure_spot_funds_pnl_bounds_axis(
                    engine,
                    OpenPitStringView::from_utf8("SpotFundsPolicy"),
                    selector,
                    &barrier,
                    1,
                    true,
                    std::ptr::null(),
                    0,
                    false,
                    std::ptr::null(),
                    0,
                    false,
                    &mut error,
                )
            };
            assert!(!result.is_null(), "{axis:?}: global update succeeds");
            assert!(error.is_null());
            assert!(unsafe { &*result }.items.is_empty());
            let mut expected = SpotFundsSettings::new(0, SpotFundsPricingSource::Mark, []).unwrap();
            expected
                .set_pnl_axis_global_barriers(
                    axis,
                    [SpotFundsPnlBoundsBarrier {
                        currency: openpit::param::Asset::new("USD").unwrap(),
                        lower_bound: Some(Pnl::from_str("-100").unwrap()),
                        upper_bound: None,
                        mode: openpit::pretrade::RuleMode::Reject,
                    }],
                )
                .unwrap();
            assert_eq!(
                position_limit_settings_snapshot(engine),
                format!("{expected:?}"),
                "{axis:?}: global barrier reaches only the selected axis"
            );
            crate::reject::openpit_destroy_pretrade_account_block_outcome_list(result);
            crate::engine::openpit_destroy_engine(engine);
        }
    }

    #[test]
    fn configure_spot_funds_pnl_bounds_axis_patches_each_tier() {
        for (selector, axis) in PNL_BOUNDS_AXES {
            let engine = engine_with_spot_funds();
            let barrier = pnl_bounds_barrier("-100");
            let groups = [
                OpenPitPretradePoliciesSpotFundsPnlBoundsAccountGroupBarrier {
                    barrier,
                    account_group_id: 7,
                },
            ];
            let accounts = [OpenPitPretradePoliciesSpotFundsPnlBoundsAccountBarrier {
                barrier,
                account_id: 42,
            }];
            let update = |has_global, has_groups, has_accounts, clear| {
                let mut error = std::ptr::null_mut();
                let result = unsafe {
                    openpit_engine_configure_spot_funds_pnl_bounds_axis(
                        engine,
                        OpenPitStringView::from_utf8("SpotFundsPolicy"),
                        selector,
                        if clear { std::ptr::null() } else { &barrier },
                        usize::from(!clear),
                        has_global,
                        if clear {
                            std::ptr::null()
                        } else {
                            groups.as_ptr()
                        },
                        usize::from(!clear),
                        has_groups,
                        if clear {
                            std::ptr::null()
                        } else {
                            accounts.as_ptr()
                        },
                        usize::from(!clear),
                        has_accounts,
                        &mut error,
                    )
                };
                assert!(!result.is_null(), "{axis:?}: PATCH succeeds");
                assert!(error.is_null());
                crate::reject::openpit_destroy_pretrade_account_block_outcome_list(result);
            };
            let mut expected = SpotFundsSettings::new(0, SpotFundsPricingSource::Mark, []).unwrap();
            let core_barrier = SpotFundsPnlBoundsBarrier {
                currency: openpit::param::Asset::new("USD").unwrap(),
                lower_bound: Some(Pnl::from_str("-100").unwrap()),
                upper_bound: None,
                mode: openpit::pretrade::RuleMode::Reject,
            };
            expected
                .set_pnl_axis_global_barriers(axis, [core_barrier.clone()])
                .unwrap();
            expected
                .set_pnl_axis_account_group_barriers(
                    axis,
                    [SpotFundsPnlBoundsAccountGroupBarrier {
                        barrier: core_barrier.clone(),
                        account_group_id: AccountGroupId::from_u32(7).unwrap(),
                    }],
                )
                .unwrap();
            expected
                .set_pnl_axis_account_barriers(
                    axis,
                    [SpotFundsPnlBoundsAccountBarrier {
                        barrier: core_barrier,
                        account_id: AccountId::from_u64(42),
                    }],
                )
                .unwrap();
            update(true, true, true, false);
            assert_eq!(
                position_limit_settings_snapshot(engine),
                format!("{expected:?}"),
                "{axis:?}: all tiers are set"
            );
            update(false, false, false, true);
            assert_eq!(
                position_limit_settings_snapshot(engine),
                format!("{expected:?}"),
                "{axis:?}: false flags preserve every tier"
            );
            update(true, false, false, true);
            expected.set_pnl_axis_global_barriers(axis, []).unwrap();
            assert_eq!(
                position_limit_settings_snapshot(engine),
                format!("{expected:?}"),
                "{axis:?}: engaged null clears only global"
            );
            update(false, true, false, true);
            expected
                .set_pnl_axis_account_group_barriers(axis, [])
                .unwrap();
            assert_eq!(
                position_limit_settings_snapshot(engine),
                format!("{expected:?}"),
                "{axis:?}: engaged empty list clears only groups"
            );
            update(false, false, true, true);
            expected.set_pnl_axis_account_barriers(axis, []).unwrap();
            assert_eq!(
                position_limit_settings_snapshot(engine),
                format!("{expected:?}"),
                "{axis:?}: engaged empty list clears only accounts"
            );
            crate::engine::openpit_destroy_engine(engine);
        }
    }

    #[test]
    fn configure_spot_funds_pnl_bounds_axis_rejects_null_arguments() {
        let engine = engine_with_spot_funds();
        let before = position_limit_settings_snapshot(engine);
        let barrier = pnl_bounds_barrier("-100");
        for (null_engine, group_len, account_len, message) in [
            (true, 0, 0, "engine is null"),
            (false, 1, 0, "spot_funds_pnl_bounds account_group is null"),
            (false, 0, 1, "spot_funds_pnl_bounds account is null"),
        ] {
            let mut error = std::ptr::null_mut();
            let result = unsafe {
                openpit_engine_configure_spot_funds_pnl_bounds_axis(
                    if null_engine {
                        std::ptr::null_mut()
                    } else {
                        engine
                    },
                    OpenPitStringView::from_utf8("SpotFundsPolicy"),
                    0,
                    &barrier,
                    1,
                    true,
                    std::ptr::null(),
                    group_len,
                    true,
                    std::ptr::null(),
                    account_len,
                    true,
                    &mut error,
                )
            };
            assert!(result.is_null(), "{message}: null result");
            assert!(!error.is_null(), "{message}: validation error is returned");
            assert_eq!(
                crate::engine::openpit_configure_error_get_kind(error),
                crate::engine::OpenPitConfigureErrorKind::Validation,
                "{message}: error kind"
            );
            assert_eq!(
                unsafe { cstr_arg(crate::engine::openpit_configure_error_get_message(error)) }
                    .unwrap(),
                message,
                "null validation text is preserved"
            );
            assert_eq!(
                position_limit_settings_snapshot(engine),
                before,
                "{message}: invalid input leaves settings unchanged"
            );
            crate::engine::openpit_destroy_configure_error(error);
        }
        crate::engine::openpit_destroy_engine(engine);
    }

    #[test]
    fn configure_spot_funds_pnl_bounds_axis_rejects_unknown_axes_without_change() {
        let engine = engine_with_spot_funds();
        let before = position_limit_settings_snapshot(engine);
        let barrier = pnl_bounds_barrier("-100");
        for axis in [4, 255] {
            let mut error = std::ptr::null_mut();
            let result = unsafe {
                openpit_engine_configure_spot_funds_pnl_bounds_axis(
                    engine,
                    OpenPitStringView::from_utf8("SpotFundsPolicy"),
                    axis,
                    &barrier,
                    1,
                    true,
                    std::ptr::null(),
                    0,
                    false,
                    std::ptr::null(),
                    0,
                    false,
                    &mut error,
                )
            };
            assert!(result.is_null(), "axis {axis}: invalid axis returns null");
            assert!(!error.is_null());
            assert_eq!(
                crate::engine::openpit_configure_error_get_kind(error),
                crate::engine::OpenPitConfigureErrorKind::Validation
            );
            assert_eq!(
                unsafe { cstr_arg(crate::engine::openpit_configure_error_get_message(error)) }
                    .unwrap(),
                format!(
                    concat!(
                        "spot funds pnl_bounds axis must be 0 (AccountUnrealized), ",
                        "1 (AccountTotal), 2 (PositionUnrealized) or 3 (PositionTotal), got {}"
                    ),
                    axis
                ),
                "axis {axis}: error names every accepted value"
            );
            assert_eq!(
                position_limit_settings_snapshot(engine),
                before,
                "axis {axis}: invalid axis cannot change settings"
            );
            crate::engine::openpit_destroy_configure_error(error);
        }
        crate::engine::openpit_destroy_engine(engine);
    }

    #[test]
    fn configure_spot_funds_pnl_bounds_axis_retune_returns_position_block() {
        for (axis, label) in [(2, "position unrealized pnl"), (3, "position total pnl")] {
            let builder = make_local_engine_builder();
            let service = make_service();
            let slippage = 0;
            assert!(unsafe {
                openpit_engine_builder_add_builtin_spot_funds_policy(
                    builder,
                    service,
                    &slippage,
                    0,
                    std::ptr::null(),
                    0,
                    0,
                    std::ptr::null(),
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
            assert!(crate::engine::openpit_engine_set_account_currency(
                engine,
                42,
                OpenPitStringView::from_utf8("USD"),
                null_out_error(),
            ));
            let instrument = crate::instrument::OpenPitInstrument {
                underlying_asset: OpenPitStringView::from_utf8("AAPL"),
                settlement_asset: OpenPitStringView::from_utf8("USD"),
            };
            let mut quote = crate::marketdata::OpenPitMarketDataQuote::default();
            quote.mark.value = crate::param::OpenPitParamPrice(
                openpit::param::Price::from_str("90")
                    .unwrap()
                    .to_decimal()
                    .into(),
            );
            quote.mark.is_set = true;
            let mut instrument_id = 0;
            assert!(
                crate::marketdata::openpit_marketdata_service_push_by_instrument(
                    service,
                    &instrument,
                    quote,
                    0,
                    0,
                    &mut instrument_id,
                    null_out_error(),
                )
            );
            let mut report = crate::execution_report::OpenPitExecutionReport::default();
            report.operation.is_set = true;
            report.operation.value.instrument = instrument;
            report.operation.value.account_id.value = 42;
            report.operation.value.account_id.is_set = true;
            report.operation.value.side = crate::param::OPENPIT_PARAM_SIDE_BUY;
            report.fill.is_set = true;
            report.fill.value.last_trade.is_set = true;
            report.fill.value.last_trade.value.price = crate::param::OpenPitParamPrice(
                openpit::param::Price::from_str("100")
                    .unwrap()
                    .to_decimal()
                    .into(),
            );
            report.fill.value.last_trade.value.quantity = OpenPitParamQuantity(
                openpit::param::Quantity::from_str("2")
                    .unwrap()
                    .to_decimal()
                    .into(),
            );
            report.fill.value.remaining_reserved_quantity.is_set = true;
            report.fill.value.is_final.is_set = true;
            let lock = crate::pre_trade_lock::OpenPitPretradePreTradeLock::from_inner(
                openpit::pretrade::PreTradeLock::from_entries([(
                    openpit::DEFAULT_POLICY_GROUP_ID,
                    openpit::param::Price::from_str("100").unwrap(),
                )]),
            );
            report.fill.value.lock = lock;
            assert!(crate::engine::openpit_engine_apply_execution_report(
                engine,
                &report,
                std::ptr::null_mut(),
                null_out_error(),
            ));
            crate::pre_trade_lock::openpit_destroy_pretrade_pre_trade_lock(lock);
            let barrier = pnl_bounds_barrier("-10");
            let mut error = std::ptr::null_mut();
            let result = unsafe {
                openpit_engine_configure_spot_funds_pnl_bounds_axis(
                    engine,
                    OpenPitStringView::from_utf8("SpotFundsPolicy"),
                    axis,
                    &barrier,
                    1,
                    true,
                    std::ptr::null(),
                    0,
                    false,
                    std::ptr::null(),
                    0,
                    false,
                    &mut error,
                )
            };
            assert!(!result.is_null(), "{label}: retune succeeds");
            assert!(error.is_null());
            let outcomes = &unsafe { &*result }.items;
            assert_eq!(outcomes.len(), 1, "{label}: retune returns one block");
            assert_eq!(outcomes[0].account_id, AccountId::from_u64(42));
            assert_eq!(
                outcomes[0].block.code,
                openpit::pretrade::RejectCode::PnlKillSwitchTriggered
            );
            assert_eq!(
                outcomes[0].block.details,
                format!(
                    "lower bound breached: {label} -20, lower bound -10, currency USD, asset AAPL"
                ),
                "{label}: returned block names the breached axis and position"
            );
            crate::reject::openpit_destroy_pretrade_account_block_outcome_list(result);
            crate::engine::openpit_destroy_engine(engine);
            openpit_destroy_marketdata_service(service);
        }
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
                std::ptr::null(),
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
                std::ptr::null(),
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
                std::ptr::null(),
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
                std::ptr::null(),
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
                std::ptr::null(),
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
                std::ptr::null(),
                &mut err as *mut _ as OpenPitOutError,
            )
        };
        assert!(!result);
        assert!(!err.is_null());
        let msg = cstr_to_string(err);
        assert!(msg.contains("spot funds override account group id is invalid"));
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
                std::ptr::null(),
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
                std::ptr::null(),
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
                std::ptr::null(),
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
                std::ptr::null(),
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
                std::ptr::null(),
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
                std::ptr::null(),
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
                std::ptr::null(),
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
                std::ptr::null(),
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
                std::ptr::null(),
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
                std::ptr::null(),
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
                std::ptr::null(),
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

    fn position_limit_settings_snapshot(engine: *mut crate::engine::OpenPitEngine) -> String {
        let mut snapshot = String::new();
        unsafe { &*engine }
            .configurator()
            .spot_funds("SpotFundsPolicy", |settings| {
                snapshot = format!("{settings:?}");
                Ok::<(), SpotFundsConfigError>(())
            })
            .expect("spot-funds settings must be readable");
        snapshot
    }

    fn assert_position_limit_validation(
        engine: *mut crate::engine::OpenPitEngine,
        case: &str,
        asset: OpenPitStringView,
        limit: OpenPitParamDecimal,
    ) {
        let mut out_error = std::ptr::null_mut();
        let ok = unsafe {
            openpit_engine_configure_spot_funds_position_limit(
                engine,
                OpenPitStringView::from_utf8("SpotFundsPolicy"),
                42,
                asset,
                crate::reject::OPENPIT_PRETRADE_RULE_MODE_REJECT,
                OpenPitParamQuantity(limit),
                true,
                &mut out_error,
            )
        };
        assert!(!ok, "{case} was accepted");
        assert!(!out_error.is_null(), "{case} had no configure error");
        assert_eq!(
            crate::engine::openpit_configure_error_get_kind(out_error),
            crate::engine::OpenPitConfigureErrorKind::Validation,
            "{case} returned the wrong configure error"
        );
        crate::engine::openpit_destroy_configure_error(out_error);
    }

    #[test]
    fn configure_spot_funds_limit_modes_happy_path() {
        let engine = engine_with_spot_funds();
        let name = OpenPitStringView::from_utf8("SpotFundsPolicy");

        assert!(unsafe {
            openpit_engine_configure_spot_funds_global_limit_mode(
                engine,
                name,
                OPENPIT_PRETRADE_POLICIES_SPOT_FUNDS_LIMIT_MODE_TRACK_ONLY,
                std::ptr::null_mut(),
            )
        });
        assert!(unsafe {
            openpit_engine_configure_spot_funds_account_limit_mode(
                engine,
                name,
                42,
                OPENPIT_PRETRADE_POLICIES_SPOT_FUNDS_LIMIT_MODE_TRACK_ONLY,
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
                OPENPIT_PRETRADE_POLICIES_SPOT_FUNDS_LIMIT_MODE_ENFORCE,
                false,
                std::ptr::null_mut(),
            )
        });
        assert!(unsafe {
            openpit_engine_configure_spot_funds_account_group_limit_mode(
                engine,
                name,
                3,
                OPENPIT_PRETRADE_POLICIES_SPOT_FUNDS_LIMIT_MODE_TRACK_ONLY,
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
                OPENPIT_PRETRADE_POLICIES_SPOT_FUNDS_LIMIT_MODE_ENFORCE,
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
                OPENPIT_PRETRADE_POLICIES_SPOT_FUNDS_LIMIT_MODE_TRACK_ONLY,
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
    fn configure_spot_funds_position_limit_modes_round_trip_and_clear_independently() {
        let engine = engine_with_spot_funds();
        let mut expected = None;
        unsafe { &*engine }
            .configurator()
            .spot_funds("SpotFundsPolicy", |settings| {
                expected = Some(settings.clone());
                Ok::<(), SpotFundsConfigError>(())
            })
            .unwrap();
        let mut expected = expected.unwrap();
        let modes = [
            (0, RuleMode::Reject),
            (1, RuleMode::Reject),
            (2, RuleMode::ReviewWithHold),
            (3, RuleMode::ReviewWithoutHold),
        ];
        for has_limit in [true, false] {
            for (wire, mode) in modes {
                let quantity = openpit::param::Quantity::from_str(&wire.to_string()).unwrap();
                assert!(unsafe {
                    openpit_engine_configure_spot_funds_position_limit(
                        engine,
                        OpenPitStringView::from_utf8("SpotFundsPolicy"),
                        42,
                        OpenPitStringView::from_utf8("AAPL"),
                        wire,
                        OpenPitParamQuantity(quantity.to_decimal().into()),
                        has_limit,
                        std::ptr::null_mut(),
                    )
                });
                expected.set_position_limit(
                    AccountId::from_u64(42),
                    openpit::param::Asset::new("AAPL").unwrap(),
                    mode,
                    has_limit.then_some(quantity),
                );
                assert_eq!(
                    position_limit_settings_snapshot(engine),
                    format!("{expected:?}"),
                    "mode {mode:?}, has_limit {has_limit}: only the selected mode changes"
                );
            }
        }
        crate::engine::openpit_destroy_engine(engine);
    }

    #[test]
    fn configure_spot_funds_position_limit_invalid_modes_leave_settings_unchanged() {
        let engine = engine_with_spot_funds();
        let name = OpenPitStringView::from_utf8("SpotFundsPolicy");
        let asset = OpenPitStringView::from_utf8("AAPL");
        let limit = OpenPitParamQuantity(OpenPitParamDecimal::default());
        assert!(unsafe {
            openpit_engine_configure_spot_funds_position_limit(
                engine,
                name,
                42,
                asset,
                2,
                limit,
                true,
                std::ptr::null_mut(),
            )
        });
        let before = position_limit_settings_snapshot(engine);
        for mode in [4, 255] {
            for has_limit in [true, false] {
                let mut error = std::ptr::null_mut();
                assert!(
                    !unsafe {
                        openpit_engine_configure_spot_funds_position_limit(
                            engine, name, 42, asset, mode, limit, has_limit, &mut error,
                        )
                    },
                    "invalid mode {mode} was accepted, has_limit {has_limit}"
                );
                assert!(!error.is_null());
                assert_eq!(
                    crate::engine::openpit_configure_error_get_kind(error),
                    crate::engine::OpenPitConfigureErrorKind::Validation
                );
                let message = crate::engine::openpit_configure_error_get_message(error);
                let message_text = std::str::from_utf8(unsafe {
                    std::slice::from_raw_parts(message.ptr, message.len)
                })
                .unwrap();
                assert!(
                    message_text.contains("position_limit mode"),
                    "{message_text}"
                );
                assert!(message_text.contains(&mode.to_string()), "{message_text}");
                crate::engine::openpit_destroy_configure_error(error);
                assert_eq!(
                    position_limit_settings_snapshot(engine),
                    before,
                    "invalid mode {mode} changed settings, has_limit {has_limit}"
                );
            }
        }
        crate::engine::openpit_destroy_engine(engine);
    }

    #[test]
    fn configure_spot_funds_position_limit_review_and_reject_breaches() {
        for mode in [2, 3] {
            let engine = engine_with_spot_funds();
            let name = OpenPitStringView::from_utf8("SpotFundsPolicy");
            assert!(crate::engine::openpit_engine_set_account_currency(
                engine,
                42,
                OpenPitStringView::from_utf8("USD"),
                null_out_error(),
            ));
            assert!(unsafe {
                openpit_engine_configure_spot_funds_global_limit_mode(
                    engine,
                    name,
                    OPENPIT_PRETRADE_POLICIES_SPOT_FUNDS_LIMIT_MODE_TRACK_ONLY,
                    std::ptr::null_mut(),
                )
            });
            for (wire, limit) in [(1, "2"), (mode, "0")] {
                assert!(unsafe {
                    openpit_engine_configure_spot_funds_position_limit(
                        engine,
                        name,
                        42,
                        OpenPitStringView::from_utf8("AAPL"),
                        wire,
                        OpenPitParamQuantity(
                            openpit::param::Quantity::from_str(limit)
                                .unwrap()
                                .to_decimal()
                                .into(),
                        ),
                        true,
                        std::ptr::null_mut(),
                    )
                });
            }
            for (quantity, passed) in [("1", true), ("3", false)] {
                let mut order = crate::order::OpenPitOrder::default();
                order.operation.is_set = true;
                let operation = &mut order.operation.value;
                operation.account_id.value = 42;
                operation.account_id.is_set = true;
                operation.instrument = crate::instrument::OpenPitInstrument {
                    underlying_asset: OpenPitStringView::from_utf8("AAPL"),
                    settlement_asset: OpenPitStringView::from_utf8("USD"),
                };
                operation.side = crate::param::OPENPIT_PARAM_SIDE_BUY;
                operation.price.is_set = true;
                operation.price.value = crate::param::OpenPitParamPrice(
                    openpit::param::Price::from_str("100")
                        .unwrap()
                        .to_decimal()
                        .into(),
                );
                operation.trade_amount = crate::param::OpenPitParamTradeAmount {
                    kind: crate::param::OPENPIT_PARAM_TRADE_AMOUNT_KIND_QUANTITY,
                    value: openpit::param::Quantity::from_str(quantity)
                        .unwrap()
                        .to_decimal()
                        .into(),
                };
                let mut report = std::ptr::null_mut();
                assert!(crate::engine::openpit_engine_execute_pre_trade_dry_run(
                    engine,
                    &order,
                    &mut report,
                    null_out_error(),
                ));
                assert_eq!(
                    crate::engine::openpit_pretrade_pre_trade_dry_run_report_is_pass(report),
                    passed,
                    "mode {mode}, quantity {quantity}: review passes and Reject rejects"
                );
                let list = if passed {
                    crate::engine::openpit_pretrade_pre_trade_dry_run_report_get_reviews(report)
                } else {
                    crate::engine::openpit_pretrade_pre_trade_dry_run_report_get_rejects(report)
                };
                let items = &unsafe { &*list }.items;
                assert_eq!(items.len(), 1, "one position-limit outcome");
                assert_eq!(
                    items[0].code,
                    openpit::pretrade::RejectCode::PositionLimitExceeded
                );
                assert_eq!(items[0].scope, openpit::pretrade::RejectScope::Order);
                assert_eq!(
                    items[0].mode,
                    if passed {
                        import_rule_mode(mode).unwrap()
                    } else {
                        RuleMode::Reject
                    },
                    "position-limit outcome preserves its mode"
                );
                crate::reject::openpit_destroy_pretrade_reject_list(list);
                crate::engine::openpit_destroy_pretrade_pre_trade_dry_run_report(report);
            }
            crate::engine::openpit_destroy_engine(engine);
        }
    }

    #[test]
    fn configure_spot_funds_position_limit_ignores_limit_when_clearing() {
        let engine = engine_with_spot_funds();
        let name = OpenPitStringView::from_utf8("SpotFundsPolicy");
        let asset = OpenPitStringView::from_utf8("AAPL");
        let initial = position_limit_settings_snapshot(engine);
        for malformed in [
            OpenPitParamDecimal {
                mantissa_lo: -1,
                mantissa_hi: -1,
                scale: 0,
            },
            OpenPitParamDecimal {
                mantissa_lo: 1,
                mantissa_hi: 0,
                scale: 29,
            },
        ] {
            assert!(unsafe {
                openpit_engine_configure_spot_funds_position_limit(
                    engine,
                    name,
                    42,
                    asset,
                    crate::reject::OPENPIT_PRETRADE_RULE_MODE_REJECT,
                    OpenPitParamQuantity(OpenPitParamDecimal {
                        mantissa_lo: 1,
                        mantissa_hi: 0,
                        scale: 0,
                    }),
                    true,
                    std::ptr::null_mut(),
                )
            });
            assert_ne!(position_limit_settings_snapshot(engine), initial);
            let mut out_error = std::ptr::null_mut();
            let ok = unsafe {
                openpit_engine_configure_spot_funds_position_limit(
                    engine,
                    name,
                    42,
                    asset,
                    crate::reject::OPENPIT_PRETRADE_RULE_MODE_REJECT,
                    OpenPitParamQuantity(malformed),
                    false,
                    &mut out_error,
                )
            };
            assert!(ok, "malformed limit was read while clearing");
            assert!(out_error.is_null());
            assert_eq!(position_limit_settings_snapshot(engine), initial);
        }
        crate::engine::openpit_destroy_engine(engine);
    }

    #[test]
    fn configure_spot_funds_position_limit_rejects_invalid_limits_without_change() {
        let engine = engine_with_spot_funds();
        let asset = OpenPitStringView::from_utf8("AAPL");
        let cases = [
            (
                "negative quantity",
                OpenPitParamDecimal {
                    mantissa_lo: -1,
                    mantissa_hi: -1,
                    scale: 0,
                },
            ),
            (
                "scale 29",
                OpenPitParamDecimal {
                    mantissa_lo: 1,
                    mantissa_hi: 0,
                    scale: 29,
                },
            ),
            (
                "mantissa wider than 96 bits",
                OpenPitParamDecimal {
                    mantissa_lo: 0,
                    mantissa_hi: 1 << 32,
                    scale: 0,
                },
            ),
        ];
        assert!(unsafe {
            openpit_engine_configure_spot_funds_position_limit(
                engine,
                OpenPitStringView::from_utf8("SpotFundsPolicy"),
                42,
                asset,
                crate::reject::OPENPIT_PRETRADE_RULE_MODE_REJECT,
                OpenPitParamQuantity(OpenPitParamDecimal {
                    mantissa_lo: 1,
                    mantissa_hi: 0,
                    scale: 0,
                }),
                true,
                std::ptr::null_mut(),
            )
        });
        let before = position_limit_settings_snapshot(engine);
        for (case, limit) in cases {
            assert_position_limit_validation(engine, case, asset, limit);
            assert_eq!(
                position_limit_settings_snapshot(engine),
                before,
                "{case} changed the position limit"
            );
        }
        crate::engine::openpit_destroy_engine(engine);
    }

    #[test]
    fn configure_spot_funds_position_limit_rejects_invalid_assets() {
        let engine = engine_with_spot_funds();
        let invalid_utf8 = [0xff];
        let cases = [
            ("null asset", OpenPitStringView::default()),
            (
                "invalid UTF-8 asset",
                OpenPitStringView {
                    ptr: invalid_utf8.as_ptr(),
                    len: invalid_utf8.len(),
                },
            ),
            ("empty asset", OpenPitStringView::from_utf8("")),
        ];
        let valid_limit = OpenPitParamDecimal {
            mantissa_lo: 1,
            mantissa_hi: 0,
            scale: 0,
        };
        for (case, asset) in cases {
            assert_position_limit_validation(engine, case, asset, valid_limit);
        }
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
                OPENPIT_PRETRADE_POLICIES_SPOT_FUNDS_LIMIT_MODE_TRACK_ONLY,
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
                OPENPIT_PRETRADE_POLICIES_SPOT_FUNDS_LIMIT_MODE_TRACK_ONLY,
                &mut out_error,
            )
        };
        assert!(!ok);
        assert!(!out_error.is_null());
        crate::engine::openpit_destroy_configure_error(out_error);
        crate::engine::openpit_destroy_engine(engine);
    }
}
