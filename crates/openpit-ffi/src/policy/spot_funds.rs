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

use openpit::param::{AccountGroupId, AccountId};
use openpit::pretrade::policies::SpotFundsPolicy;
use openpit::{
    InstrumentId, PolicyGroupId, SpotFundsMarketData, SpotFundsOverride, SpotFundsOverrideTarget,
    SpotFundsPricingSource,
};
use openpit_interop::{EngineLocking, SyncMode};

use crate::account_group_id::OpenPitParamAccountGroupIdOptional;
use crate::marketdata::{OpenPitMarketDataInstrumentId, OpenPitMarketDataService};
use crate::param::OpenPitParamAccountIdOptional;

use super::*;

/// Slippage override entry for the spot funds policy.
///
/// Mirrors [`SpotFundsOverride`](openpit::SpotFundsOverride) together with the
/// [`SpotFundsOverrideTarget`](openpit::SpotFundsOverrideTarget) it applies to.
/// `instrument_id` selects the registered instrument. The scope is chosen by
/// the `account_id` and `account_group_id` optionals, which are mutually
/// exclusive: when neither is set the entry is an instrument-level default; when
/// `account_id.is_set` it applies only to `account_id.value`; when
/// `account_group_id.is_set` it applies only to accounts in
/// `account_group_id.value`. When `has_slippage_bps` is `true`, `slippage_bps`
/// is the slippage for that scope; when `false`, the entry is ignored and the
/// cascade falls through to the next tier (ultimately the global
/// `market_slippage_bps`).
///
/// Slippage resolves account -> account group -> instrument -> global for each
/// order.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct OpenPitPretradePoliciesSpotFundsOverride {
    /// Registered market-data instrument id.
    pub instrument_id: OpenPitMarketDataInstrumentId,
    /// Account the override applies to. When `is_set`, the override is scoped to
    /// `value`; mutually exclusive with `account_group_id`. Both unset means an
    /// instrument-level default.
    pub account_id: OpenPitParamAccountIdOptional,
    /// Account group the override applies to. When `is_set`, the override is
    /// scoped to `value`; mutually exclusive with `account_id`. Both unset means
    /// an instrument-level default.
    pub account_group_id: OpenPitParamAccountGroupIdOptional,
    /// Slippage in basis points for the selected scope, used only when
    /// `has_slippage_bps` is `true`.
    pub slippage_bps: u16,
    /// Whether `slippage_bps` carries a value.
    pub has_slippage_bps: bool,
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
///   slippage overrides; pass null + 0 for none. Each entry selects an
///   instrument by `instrument_id` and a scope via its `account_id` /
///   `account_group_id` optionals: both unset is an instrument-level default,
///   a set `account_id` scopes the override to that account, a set
///   `account_group_id` scopes it to that account group. The two are mutually
///   exclusive; setting both fails the call. An entry with
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

    let market_orders: Option<SpotFundsMarketData<EngineLocking>> = if market_data.is_null() {
        None
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

        let pricing_source = match import_pricing_source(pricing_source, out_error) {
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
            let instrument_id = InstrumentId::new(entry.instrument_id);
            let target = match (entry.account_id.is_set, entry.account_group_id.is_set) {
                (true, true) => {
                    write_error(
                        out_error,
                        "spot funds override cannot target both an account and an account group",
                    );
                    return false;
                }
                (true, false) => SpotFundsOverrideTarget::InstrumentAccount(
                    instrument_id,
                    AccountId::from_u64(entry.account_id.value),
                ),
                (false, true) => match AccountGroupId::from_u32(entry.account_group_id.value) {
                    Ok(account_group_id) => SpotFundsOverrideTarget::InstrumentAccountGroup(
                        instrument_id,
                        account_group_id,
                    ),
                    Err(e) => {
                        write_error_format!(
                            out_error,
                            "spot funds override account group id {} is invalid: {}",
                            entry.account_group_id.value,
                            e
                        );
                        return false;
                    }
                },
                (false, false) => SpotFundsOverrideTarget::Instrument(instrument_id),
            };
            overrides.push((
                target,
                SpotFundsOverride {
                    slippage_bps: entry.has_slippage_bps.then_some(entry.slippage_bps),
                },
            ));
        }

        let handle = svc.handle_clone();
        match SpotFundsMarketData::<EngineLocking>::new(handle, bps, pricing_source, overrides) {
            Ok(bundle) => Some(bundle),
            Err(e) => {
                write_error_format!(out_error, "spot funds market data build failed: {}", e);
                return false;
            }
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
    let policy =
        SpotFundsPolicy::<EngineLocking, EngineLocking>::new(market_orders, storage_builder)
            .with_policy_group_id(PolicyGroupId::new(policy_group_id));
    match crate::engine::add_pre_trade_policy_to_builder(builder_ref, policy) {
        Ok(()) => true,
        Err(err) => {
            write_error(out_error, &err);
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

    /// Creates a Full-mode (byte 0) engine builder.
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
            OpenPitPretradePoliciesSpotFundsOverride {
                instrument_id: 1,
                account_id: OpenPitParamAccountIdOptional {
                    value: 0,
                    is_set: false,
                },
                account_group_id: OpenPitParamAccountGroupIdOptional {
                    value: 0,
                    is_set: false,
                },
                slippage_bps: 500,
                has_slippage_bps: true,
            },
            OpenPitPretradePoliciesSpotFundsOverride {
                instrument_id: 2,
                account_id: OpenPitParamAccountIdOptional {
                    value: 0,
                    is_set: false,
                },
                account_group_id: OpenPitParamAccountGroupIdOptional {
                    value: 0,
                    is_set: false,
                },
                slippage_bps: 0,
                has_slippage_bps: false,
            },
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
        let overrides = [OpenPitPretradePoliciesSpotFundsOverride {
            instrument_id: 1,
            account_id: OpenPitParamAccountIdOptional {
                value: 99224416,
                is_set: true,
            },
            account_group_id: OpenPitParamAccountGroupIdOptional {
                value: 0,
                is_set: false,
            },
            slippage_bps: 250,
            has_slippage_bps: true,
        }];
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
        let overrides = [OpenPitPretradePoliciesSpotFundsOverride {
            instrument_id: 1,
            account_id: OpenPitParamAccountIdOptional {
                value: 0,
                is_set: false,
            },
            account_group_id: OpenPitParamAccountGroupIdOptional {
                value: 3,
                is_set: true,
            },
            slippage_bps: 250,
            has_slippage_bps: true,
        }];
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
    fn add_builtin_spot_funds_policy_override_with_account_and_group_is_error() {
        let builder = make_builder();
        let service = make_service();
        let bps: u16 = 1000;
        let overrides = [OpenPitPretradePoliciesSpotFundsOverride {
            instrument_id: 1,
            account_id: OpenPitParamAccountIdOptional {
                value: 99224416,
                is_set: true,
            },
            account_group_id: OpenPitParamAccountGroupIdOptional {
                value: 3,
                is_set: true,
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
        assert!(msg.contains("both an account and an account group"));
        openpit_destroy_marketdata_service(service);
    }

    #[test]
    fn add_builtin_spot_funds_policy_override_with_invalid_group_is_error() {
        let builder = make_builder();
        let service = make_service();
        let bps: u16 = 1000;
        // Account group 0 is the reserved default and cannot be constructed.
        let overrides = [OpenPitPretradePoliciesSpotFundsOverride {
            instrument_id: 1,
            account_id: OpenPitParamAccountIdOptional {
                value: 0,
                is_set: false,
            },
            account_group_id: OpenPitParamAccountGroupIdOptional {
                value: 0,
                is_set: true,
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
}
