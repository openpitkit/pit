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

use std::time::Duration;

use openpit::param::AccountId;
use openpit::pretrade::policies::{
    RateLimit, RateLimitAccountAssetBarrier, RateLimitAccountBarrier, RateLimitAssetBarrier,
    RateLimitBrokerBarrier, RateLimitPolicy, RateLimitPolicyError, RateLimitSettings,
};

use crate::engine::{write_configure_error, OpenPitConfigureError};

use super::*;

/// Broker-wide rate-limit barrier for
/// `openpit_engine_builder_add_builtin_rate_limit_policy`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpenPitPretradePoliciesRateLimitBrokerBarrier {
    /// Maximum number of orders accepted within the window.
    pub max_orders: usize,
    /// Window duration in nanoseconds.
    pub window_nanoseconds: i64,
}

/// Per-settlement-asset rate-limit barrier for
/// `openpit_engine_builder_add_builtin_rate_limit_policy`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpenPitPretradePoliciesRateLimitAssetBarrier {
    /// Settlement asset this barrier applies to.
    pub settlement_asset: OpenPitStringView,
    /// Maximum number of orders accepted within the window.
    pub max_orders: usize,
    /// Window duration in nanoseconds.
    pub window_nanoseconds: i64,
}

/// Per-account rate-limit barrier for
/// `openpit_engine_builder_add_builtin_rate_limit_policy`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpenPitPretradePoliciesRateLimitAccountBarrier {
    /// Account this barrier applies to.
    pub account_id: OpenPitParamAccountId,
    /// Maximum number of orders accepted within the window.
    pub max_orders: usize,
    /// Window duration in nanoseconds.
    pub window_nanoseconds: i64,
}

/// Per-(account, settlement-asset) rate-limit barrier for
/// `openpit_engine_builder_add_builtin_rate_limit_policy`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpenPitPretradePoliciesRateLimitAccountAssetBarrier {
    /// Account this barrier applies to.
    pub account_id: OpenPitParamAccountId,
    /// Settlement asset this barrier applies to.
    pub settlement_asset: OpenPitStringView,
    /// Maximum number of orders accepted within the window.
    pub max_orders: usize,
    /// Window duration in nanoseconds.
    pub window_nanoseconds: i64,
}

fn rate_limit_window_duration(window_nanoseconds: i64) -> Result<Duration, String> {
    if window_nanoseconds <= 0 {
        return Err(format!(
            "rate limit window must be positive and fit in u64 nanoseconds, got {window_nanoseconds}ns"
        ));
    }
    Ok(Duration::from_nanos(window_nanoseconds as u64))
}

#[no_mangle]
/// Adds the built-in rate-limit policy to the engine builder.
///
/// Contract:
/// - `builder` must be a valid engine builder pointer.
/// - `policy_group_id` assigns the policy to a policy group (pass `0` for default).
/// - At least one barrier axis must be configured: `broker` non-null,
///   `asset_len > 0`, `account_len > 0`, or `account_asset_len > 0`.
/// - When a length is greater than zero the corresponding pointer must point
///   to that many readable entries.
/// - Each `settlement_asset` string view inside an array entry must be valid
///   for the duration of the call.
///
/// Success:
/// - returns `true`; the builder retains the policy.
///
/// Error:
/// - returns `false` when the builder is null or already consumed, when no
///   barrier axis is configured, or when argument parsing fails;
/// - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
///   error handle that MUST be released with `openpit_destroy_shared_string`.
pub unsafe extern "C" fn openpit_engine_builder_add_builtin_rate_limit_policy(
    builder: *mut crate::engine::OpenPitEngineBuilder,
    policy_group_id: u16,
    broker: *const OpenPitPretradePoliciesRateLimitBrokerBarrier,
    asset: *const OpenPitPretradePoliciesRateLimitAssetBarrier,
    asset_len: usize,
    account: *const OpenPitPretradePoliciesRateLimitAccountBarrier,
    account_len: usize,
    account_asset: *const OpenPitPretradePoliciesRateLimitAccountAssetBarrier,
    account_asset_len: usize,
    out_error: OpenPitOutError,
) -> bool {
    if builder.is_null() {
        write_error(out_error, "engine builder is null");
        return false;
    }
    let asset_slice =
        match unsafe { try_slice_arg(asset, asset_len, "rate_limit_policy asset", out_error) } {
            Some(v) => v,
            None => return false,
        };
    let account_slice = match unsafe {
        try_slice_arg(account, account_len, "rate_limit_policy account", out_error)
    } {
        Some(v) => v,
        None => return false,
    };
    let account_asset_slice = match unsafe {
        try_slice_arg(
            account_asset,
            account_asset_len,
            "rate_limit_policy account_asset",
            out_error,
        )
    } {
        Some(v) => v,
        None => return false,
    };

    let broker_opt = if !broker.is_null() {
        let b = unsafe { &*broker };
        Some(RateLimitBrokerBarrier {
            limit: RateLimit {
                max_orders: b.max_orders,
                window: match rate_limit_window_duration(b.window_nanoseconds) {
                    Ok(v) => v,
                    Err(e) => {
                        write_error(
                            out_error,
                            &format!("rate_limit_policy creation failed: {e}"),
                        );
                        return false;
                    }
                },
            },
        })
    } else {
        None
    };

    let mut asset_barriers = Vec::with_capacity(asset_slice.len());
    for (index, entry) in asset_slice.iter().enumerate() {
        let settlement = match parse_asset_or_error(
            entry.settlement_asset,
            "asset",
            index,
            "settlement_asset",
            out_error,
        ) {
            Some(v) => v,
            None => return false,
        };
        asset_barriers.push(RateLimitAssetBarrier {
            limit: RateLimit {
                max_orders: entry.max_orders,
                window: match rate_limit_window_duration(entry.window_nanoseconds) {
                    Ok(v) => v,
                    Err(e) => {
                        write_error(
                            out_error,
                            &format!("rate_limit_policy creation failed: {e}"),
                        );
                        return false;
                    }
                },
            },
            settlement_asset: settlement,
        });
    }

    let mut account_barriers = Vec::with_capacity(account_slice.len());
    for entry in account_slice {
        account_barriers.push(RateLimitAccountBarrier {
            limit: RateLimit {
                max_orders: entry.max_orders,
                window: match rate_limit_window_duration(entry.window_nanoseconds) {
                    Ok(v) => v,
                    Err(e) => {
                        write_error(
                            out_error,
                            &format!("rate_limit_policy creation failed: {e}"),
                        );
                        return false;
                    }
                },
            },
            account_id: AccountId::from_u64(entry.account_id),
        });
    }

    let mut account_asset_barriers = Vec::with_capacity(account_asset_slice.len());
    for (index, entry) in account_asset_slice.iter().enumerate() {
        let settlement = match parse_asset_or_error(
            entry.settlement_asset,
            "account_asset",
            index,
            "settlement_asset",
            out_error,
        ) {
            Some(v) => v,
            None => return false,
        };
        account_asset_barriers.push(RateLimitAccountAssetBarrier {
            limit: RateLimit {
                max_orders: entry.max_orders,
                window: match rate_limit_window_duration(entry.window_nanoseconds) {
                    Ok(v) => v,
                    Err(e) => {
                        write_error(
                            out_error,
                            &format!("rate_limit_policy creation failed: {e}"),
                        );
                        return false;
                    }
                },
            },
            account_id: AccountId::from_u64(entry.account_id),
            settlement_asset: settlement,
        });
    }

    let settings = match RateLimitSettings::new(
        broker_opt,
        asset_barriers,
        account_barriers,
        account_asset_barriers,
    ) {
        Ok(v) => v,
        Err(e) => {
            write_error_format!(out_error, "rate_limit_policy creation failed: {}", e);
            return false;
        }
    };

    let builder_ref = unsafe { &mut *builder };
    let storage = match policy_storage(builder_ref) {
        Some(storage) => storage,
        None => {
            write_error(out_error, "engine builder is no longer available");
            return false;
        }
    };
    let policy = RateLimitPolicy::new(settings, storage)
        .with_policy_group_id(openpit::PolicyGroupId::new(policy_group_id));
    match crate::engine::add_pre_trade_policy_to_builder(builder_ref, policy) {
        Ok(()) => true,
        Err(err) => {
            write_error(out_error, &err);
            false
        }
    }
}

#[no_mangle]
/// Retunes the built-in rate-limit policy registered under `name`.
///
/// This is a partial update (PATCH): each axis is touched only when its
/// `has_*` flag is `true`. A touched axis is replaced wholesale — barriers
/// can be added and removed at runtime. A barrier key that survives the
/// replacement keeps its live counter (no reset). An empty axis (`len` 0
/// with `has_*` true) clears it, subject to the policy's at-least-one-
/// barrier rule. Setting `has_broker` to `true` with a null `broker` pointer
/// clears the broker barrier.
///
/// Contract:
/// - `engine` must be a valid non-null engine pointer.
/// - `name` selects the policy; it is interpreted as UTF-8. A built-in
///   policy added via `openpit_engine_builder_add_builtin_rate_limit_policy`
///   registers under its fixed name `"RateLimitPolicy"`, so pass that string
///   here.
/// - When `has_broker` is `true` and `broker` is non-null, it must point to
///   one readable entry whose `max_orders`/`window_nanoseconds` replace the
///   broker barrier; a null `broker` with `has_broker` true clears it.
/// - When `has_asset`/`has_account`/`has_account_asset` is `true`, the
///   matching pointer must point to `*_len` readable entries (a length of
///   zero clears that axis). Each `settlement_asset` view must be valid for
///   the duration of the call.
/// - A `has_*` flag set to `false` leaves that axis untouched regardless of
///   the pointer/length arguments.
///
/// Success:
/// - returns `true`; the new limits apply from the next order onward with no
///   counter reset.
///
/// Error:
/// - returns `false`; if `out_error` is non-null, writes a caller-owned
///   `OpenPitConfigureError` (release with `openpit_destroy_configure_error`)
///   describing the unknown policy, settings-type mismatch, or rejected
///   update.
/// - a null `engine` returns `false` and, when `out_error` is non-null, writes
///   a caller-owned `OpenPitConfigureError` (`Validation`) that must be released
///   with `openpit_destroy_configure_error`.
pub unsafe extern "C" fn openpit_engine_configure_rate_limit(
    engine: *mut crate::engine::OpenPitEngine,
    name: OpenPitStringView,
    broker: *const OpenPitPretradePoliciesRateLimitBrokerBarrier,
    has_broker: bool,
    asset: *const OpenPitPretradePoliciesRateLimitAssetBarrier,
    asset_len: usize,
    has_asset: bool,
    account: *const OpenPitPretradePoliciesRateLimitAccountBarrier,
    account_len: usize,
    has_account: bool,
    account_asset: *const OpenPitPretradePoliciesRateLimitAccountAssetBarrier,
    account_asset_len: usize,
    has_account_asset: bool,
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

    // Parse every C argument into owned Rust values up front, so the
    // configure closure only runs infallible-typed setters. An argument that
    // cannot be parsed (e.g. an invalid asset code) never reaches the core
    // configurator and is reported as a `Validation` error.
    let broker_barrier: Option<RateLimitBrokerBarrier> = if has_broker && !broker.is_null() {
        let b = unsafe { &*broker };
        Some(RateLimitBrokerBarrier {
            limit: RateLimit {
                max_orders: b.max_orders,
                window: match rate_limit_window_duration(b.window_nanoseconds) {
                    Ok(v) => v,
                    Err(e) => {
                        write_configure_error(out_error, OpenPitConfigureError::validation(e));
                        return false;
                    }
                },
            },
        })
    } else {
        None
    };

    let asset_barriers: Vec<RateLimitAssetBarrier> = if has_asset {
        let slice = match unsafe {
            try_slice_arg(asset, asset_len, "rate_limit asset", std::ptr::null_mut())
        } {
            Some(v) => v,
            None => {
                write_configure_error(
                    out_error,
                    OpenPitConfigureError::validation("rate_limit asset is null".to_owned()),
                );
                return false;
            }
        };
        let mut out = Vec::with_capacity(slice.len());
        for (index, entry) in slice.iter().enumerate() {
            let settlement = match parse_configure_asset(
                entry.settlement_asset,
                "asset",
                index,
                "settlement_asset",
            ) {
                Ok(v) => v,
                Err(e) => {
                    write_configure_error(out_error, e);
                    return false;
                }
            };
            out.push(RateLimitAssetBarrier {
                limit: RateLimit {
                    max_orders: entry.max_orders,
                    window: match rate_limit_window_duration(entry.window_nanoseconds) {
                        Ok(v) => v,
                        Err(e) => {
                            write_configure_error(out_error, OpenPitConfigureError::validation(e));
                            return false;
                        }
                    },
                },
                settlement_asset: settlement,
            });
        }
        out
    } else {
        Vec::new()
    };

    let account_barriers: Vec<RateLimitAccountBarrier> = if has_account {
        let slice = match unsafe {
            try_slice_arg(
                account,
                account_len,
                "rate_limit account",
                std::ptr::null_mut(),
            )
        } {
            Some(v) => v,
            None => {
                write_configure_error(
                    out_error,
                    OpenPitConfigureError::validation("rate_limit account is null".to_owned()),
                );
                return false;
            }
        };
        let mut out = Vec::with_capacity(slice.len());
        for entry in slice {
            out.push(RateLimitAccountBarrier {
                limit: RateLimit {
                    max_orders: entry.max_orders,
                    window: match rate_limit_window_duration(entry.window_nanoseconds) {
                        Ok(v) => v,
                        Err(e) => {
                            write_configure_error(out_error, OpenPitConfigureError::validation(e));
                            return false;
                        }
                    },
                },
                account_id: AccountId::from_u64(entry.account_id),
            });
        }
        out
    } else {
        Vec::new()
    };

    let account_asset_barriers: Vec<RateLimitAccountAssetBarrier> = if has_account_asset {
        let slice = match unsafe {
            try_slice_arg(
                account_asset,
                account_asset_len,
                "rate_limit account_asset",
                std::ptr::null_mut(),
            )
        } {
            Some(v) => v,
            None => {
                write_configure_error(
                    out_error,
                    OpenPitConfigureError::validation(
                        "rate_limit account_asset is null".to_owned(),
                    ),
                );
                return false;
            }
        };
        let mut out = Vec::with_capacity(slice.len());
        for (index, entry) in slice.iter().enumerate() {
            let settlement = match parse_configure_asset(
                entry.settlement_asset,
                "account_asset",
                index,
                "settlement_asset",
            ) {
                Ok(v) => v,
                Err(e) => {
                    write_configure_error(out_error, e);
                    return false;
                }
            };
            out.push(RateLimitAccountAssetBarrier {
                limit: RateLimit {
                    max_orders: entry.max_orders,
                    window: match rate_limit_window_duration(entry.window_nanoseconds) {
                        Ok(v) => v,
                        Err(e) => {
                            write_configure_error(out_error, OpenPitConfigureError::validation(e));
                            return false;
                        }
                    },
                },
                account_id: AccountId::from_u64(entry.account_id),
                settlement_asset: settlement,
            });
        }
        out
    } else {
        Vec::new()
    };

    let result = unsafe { &*engine }.configurator().rate_limit(
        &name,
        |settings| -> Result<(), RateLimitPolicyError> {
            if has_broker {
                settings.set_broker(broker_barrier.clone())?;
            }
            if has_asset {
                settings.set_asset_barriers(asset_barriers.iter().cloned())?;
            }
            if has_account {
                settings.set_account_barriers(account_barriers.iter().cloned())?;
            }
            if has_account_asset {
                settings.set_account_asset_barriers(account_asset_barriers.iter().cloned())?;
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

#[cfg(test)]
mod tests {
    use super::*;

    use crate::order::OpenPitOrder;
    use crate::param::{OpenPitParamDecimal, OpenPitParamQuantity};

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
        crate::string::openpit_destroy_shared_string(handle);
        result
    }

    fn quantity_param(mantissa: i128, scale: i32) -> OpenPitParamQuantity {
        OpenPitParamQuantity(OpenPitParamDecimal {
            mantissa_lo: mantissa as i64,
            mantissa_hi: (mantissa >> 64) as i64,
            scale,
        })
    }

    fn build_engine_with_builtin_start_policy(
        add_fn: impl FnOnce(*mut crate::engine::OpenPitEngineBuilder) -> bool,
    ) -> *mut crate::engine::OpenPitEngine {
        let builder = crate::engine::openpit_create_engine_builder(
            crate::engine::OpenPitSyncPolicy::Full as u8,
            std::ptr::null_mut(),
        );
        assert!(add_fn(builder), "failed to add policy");
        let engine = crate::engine::openpit_engine_builder_build(
            builder,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
        assert!(!engine.is_null(), "engine build failed");
        engine
    }

    fn valid_pit_order() -> OpenPitOrder {
        use crate::instrument::OpenPitInstrument;
        use crate::order::{OpenPitOrderOperation, OpenPitOrderOperationOptional};
        use crate::param::{
            OpenPitParamAccountIdOptional, OpenPitParamPrice, OpenPitParamPriceOptional,
            OpenPitParamTradeAmount,
        };
        OpenPitOrder {
            operation: OpenPitOrderOperationOptional {
                is_set: true,
                value: OpenPitOrderOperation {
                    instrument: OpenPitInstrument {
                        underlying_asset: OpenPitStringView::from_utf8("SPX"),
                        settlement_asset: OpenPitStringView::from_utf8("USD"),
                    },
                    trade_amount: OpenPitParamTradeAmount {
                        value: quantity_param(1, 0).0,
                        kind: crate::param::OPENPIT_PARAM_TRADE_AMOUNT_KIND_QUANTITY,
                    },
                    account_id: OpenPitParamAccountIdOptional {
                        value: 7,
                        is_set: true,
                    },
                    side: crate::param::OPENPIT_PARAM_SIDE_BUY,
                    price: OpenPitParamPriceOptional {
                        is_set: true,
                        value: OpenPitParamPrice(OpenPitParamDecimal {
                            mantissa_lo: 100,
                            mantissa_hi: 0,
                            scale: 0,
                        }),
                    },
                },
            },
            position: Default::default(),
            margin: Default::default(),
            user_data: std::ptr::null_mut(),
        }
    }

    fn run_start_pre_trade_passes(engine: *mut crate::engine::OpenPitEngine) {
        let order = valid_pit_order();
        let mut request = std::ptr::null_mut();
        let mut out_rejects = std::ptr::null_mut();
        let status = crate::engine::openpit_engine_start_pre_trade(
            engine,
            &order,
            &mut request,
            &mut out_rejects,
            std::ptr::null_mut(),
        );
        assert_eq!(
            status,
            crate::engine::OpenPitPretradeStatus::Passed,
            "start_pre_trade should pass"
        );
        crate::engine::openpit_destroy_pretrade_pre_trade_request(request);
    }

    #[test]
    fn add_builtin_rate_limit_policy_happy_path() {
        let broker = OpenPitPretradePoliciesRateLimitBrokerBarrier {
            max_orders: 100,
            window_nanoseconds: 1_000_000_000,
        };
        let engine = build_engine_with_builtin_start_policy(|builder| unsafe {
            openpit_engine_builder_add_builtin_rate_limit_policy(
                builder,
                0,
                &broker,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
            )
        });
        run_start_pre_trade_passes(engine);
        crate::engine::openpit_destroy_engine(engine);
    }

    #[test]
    fn add_builtin_rate_limit_policy_empty_config_reports_error() {
        let builder = crate::engine::openpit_create_engine_builder(
            crate::engine::OpenPitSyncPolicy::Full as u8,
            std::ptr::null_mut(),
        );
        let mut out_error = std::ptr::null_mut();
        let ok = unsafe {
            openpit_engine_builder_add_builtin_rate_limit_policy(
                builder,
                0,
                std::ptr::null(),
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                &mut out_error,
            )
        };
        assert!(!ok);
        let message = cstr_to_string(out_error);
        assert!(
            message.contains("rate_limit_policy creation failed")
                && message.contains("must be configured"),
            "expected SDK no-barrier error wrapped by FFI, got: {message}"
        );
        crate::engine::openpit_destroy_engine_builder(builder);
    }

    #[test]
    fn add_builtin_rate_limit_policy_local_sync_mode() {
        let broker = OpenPitPretradePoliciesRateLimitBrokerBarrier {
            max_orders: 50,
            window_nanoseconds: 10_000_000_000,
        };
        let builder = crate::engine::openpit_create_engine_builder(
            crate::engine::OpenPitSyncPolicy::None as u8,
            std::ptr::null_mut(),
        );
        let ok = unsafe {
            openpit_engine_builder_add_builtin_rate_limit_policy(
                builder,
                0,
                &broker,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
            )
        };
        assert!(ok, "add should succeed for no-sync mode");
        let engine = crate::engine::openpit_engine_builder_build(
            builder,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
        assert!(!engine.is_null());
        run_start_pre_trade_passes(engine);
        crate::engine::openpit_destroy_engine(engine);
    }

    #[test]
    fn add_builtin_rate_limit_policy_cross_axis_all_configured() {
        let usd = OpenPitStringView::from_utf8("USD");
        let broker = OpenPitPretradePoliciesRateLimitBrokerBarrier {
            max_orders: 1000,
            window_nanoseconds: 60_000_000_000,
        };
        let asset = [OpenPitPretradePoliciesRateLimitAssetBarrier {
            settlement_asset: usd,
            max_orders: 500,
            window_nanoseconds: 60_000_000_000,
        }];
        let account = [OpenPitPretradePoliciesRateLimitAccountBarrier {
            account_id: 42,
            max_orders: 200,
            window_nanoseconds: 60_000_000_000,
        }];
        let account_asset = [OpenPitPretradePoliciesRateLimitAccountAssetBarrier {
            account_id: 42,
            settlement_asset: usd,
            max_orders: 100,
            window_nanoseconds: 60_000_000_000,
        }];
        let engine = build_engine_with_builtin_start_policy(|builder| unsafe {
            openpit_engine_builder_add_builtin_rate_limit_policy(
                builder,
                0,
                &broker,
                asset.as_ptr(),
                asset.len(),
                account.as_ptr(),
                account.len(),
                account_asset.as_ptr(),
                account_asset.len(),
                std::ptr::null_mut(),
            )
        });
        run_start_pre_trade_passes(engine);
        crate::engine::openpit_destroy_engine(engine);
    }

    /// Runs `start_pre_trade` and asserts the order is rejected by a policy.
    fn run_start_pre_trade_rejected(engine: *mut crate::engine::OpenPitEngine) {
        let order = valid_pit_order();
        let mut request = std::ptr::null_mut();
        let mut out_rejects = std::ptr::null_mut();
        let status = crate::engine::openpit_engine_start_pre_trade(
            engine,
            &order,
            &mut request,
            &mut out_rejects,
            std::ptr::null_mut(),
        );
        assert_eq!(
            status,
            crate::engine::OpenPitPretradeStatus::Rejected,
            "start_pre_trade should be rejected after retune"
        );
        crate::reject::openpit_pretrade_destroy_reject_list(out_rejects);
    }

    fn configure_error_message(
        handle: *mut crate::engine::OpenPitConfigureError,
    ) -> (crate::engine::OpenPitConfigureErrorKind, String) {
        assert!(!handle.is_null(), "configure error must be populated");
        let kind = crate::engine::openpit_configure_error_get_kind(handle);
        let view = crate::engine::openpit_configure_error_get_message(handle);
        let bytes = unsafe { std::slice::from_raw_parts(view.ptr, view.len) };
        let message = std::str::from_utf8(bytes).expect("utf8").to_owned();
        crate::engine::openpit_destroy_configure_error(handle);
        (kind, message)
    }

    /// Round trip: build with a loose broker barrier, tighten it through the
    /// configure entry point, then observe the new limit on the hot path.
    #[test]
    fn configure_rate_limit_round_trip_tightens_broker_barrier() {
        let broker = OpenPitPretradePoliciesRateLimitBrokerBarrier {
            max_orders: 100,
            window_nanoseconds: 1_000_000_000,
        };
        let engine = build_engine_with_builtin_start_policy(|builder| unsafe {
            openpit_engine_builder_add_builtin_rate_limit_policy(
                builder,
                0,
                &broker,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
            )
        });

        // Retune the broker barrier down to a single order per minute before
        // any order is submitted, so the live counter starts clean.
        let tighter = OpenPitPretradePoliciesRateLimitBrokerBarrier {
            max_orders: 1,
            window_nanoseconds: 60_000_000_000,
        };
        let mut out_error = std::ptr::null_mut();
        let ok = unsafe {
            openpit_engine_configure_rate_limit(
                engine,
                OpenPitStringView::from_utf8("RateLimitPolicy"),
                &tighter,
                true,
                std::ptr::null(),
                0,
                false,
                std::ptr::null(),
                0,
                false,
                std::ptr::null(),
                0,
                false,
                &mut out_error,
            )
        };
        assert!(ok, "configure must succeed");
        assert!(out_error.is_null(), "no error on success");

        // First order consumes the single slot and passes; the second breaches
        // the retuned barrier and is rejected.
        run_start_pre_trade_passes(engine);
        run_start_pre_trade_rejected(engine);
        crate::engine::openpit_destroy_engine(engine);
    }

    /// Error path: configuring an unregistered policy name yields a populated
    /// `OpenPitConfigureError` with the `Unknown` kind.
    #[test]
    fn configure_rate_limit_unknown_name_reports_error() {
        let broker = OpenPitPretradePoliciesRateLimitBrokerBarrier {
            max_orders: 100,
            window_nanoseconds: 1_000_000_000,
        };
        let engine = build_engine_with_builtin_start_policy(|builder| unsafe {
            openpit_engine_builder_add_builtin_rate_limit_policy(
                builder,
                0,
                &broker,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
            )
        });

        let tighter = OpenPitPretradePoliciesRateLimitBrokerBarrier {
            max_orders: 1,
            window_nanoseconds: 60_000_000_000,
        };
        let mut out_error = std::ptr::null_mut();
        let ok = unsafe {
            openpit_engine_configure_rate_limit(
                engine,
                OpenPitStringView::from_utf8("NoSuchPolicy"),
                &tighter,
                true,
                std::ptr::null(),
                0,
                false,
                std::ptr::null(),
                0,
                false,
                std::ptr::null(),
                0,
                false,
                &mut out_error,
            )
        };
        assert!(!ok, "configure must fail for an unknown policy name");
        let (kind, message) = configure_error_message(out_error);
        assert_eq!(kind, crate::engine::OpenPitConfigureErrorKind::Unknown);
        assert!(
            message.contains("NoSuchPolicy"),
            "message should name the unknown policy, got: {message}"
        );
        crate::engine::openpit_destroy_engine(engine);
    }

    /// Adding a new asset barrier at runtime via configure succeeds even when
    /// no asset axis was configured at build time; the new limit is enforced
    /// immediately on the next order.
    #[test]
    fn configure_rate_limit_adds_asset_barrier_at_runtime() {
        let broker = OpenPitPretradePoliciesRateLimitBrokerBarrier {
            max_orders: 100,
            window_nanoseconds: 1_000_000_000,
        };
        // Build with only a broker barrier; no asset axis at build time.
        let engine = build_engine_with_builtin_start_policy(|builder| unsafe {
            openpit_engine_builder_add_builtin_rate_limit_policy(
                builder,
                0,
                &broker,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
            )
        });

        // Add a USD asset barrier (max 1 order per 60 s) at runtime.
        let usd = OpenPitStringView::from_utf8("USD");
        let asset = [OpenPitPretradePoliciesRateLimitAssetBarrier {
            settlement_asset: usd,
            max_orders: 1,
            window_nanoseconds: 60_000_000_000,
        }];
        let mut out_error = std::ptr::null_mut();
        let ok = unsafe {
            openpit_engine_configure_rate_limit(
                engine,
                OpenPitStringView::from_utf8("RateLimitPolicy"),
                std::ptr::null(),
                false,
                asset.as_ptr(),
                asset.len(),
                true,
                std::ptr::null(),
                0,
                false,
                std::ptr::null(),
                0,
                false,
                &mut out_error,
            )
        };
        assert!(ok, "adding an asset barrier at runtime must succeed");
        assert!(out_error.is_null(), "no error on success");

        // The test order has settlement USD (see valid_pit_order). The first
        // order consumes the single slot and passes; the second is rejected.
        run_start_pre_trade_passes(engine);
        run_start_pre_trade_rejected(engine);
        crate::engine::openpit_destroy_engine(engine);
    }

    /// Error path: retuning the broker barrier with window_nanoseconds 0
    /// yields a `Validation` error (invalid window).
    #[test]
    fn configure_rate_limit_invalid_window_reports_validation() {
        let broker = OpenPitPretradePoliciesRateLimitBrokerBarrier {
            max_orders: 100,
            window_nanoseconds: 1_000_000_000,
        };
        let engine = build_engine_with_builtin_start_policy(|builder| unsafe {
            openpit_engine_builder_add_builtin_rate_limit_policy(
                builder,
                0,
                &broker,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
            )
        });

        // Zero window is invalid.
        let zero_window = OpenPitPretradePoliciesRateLimitBrokerBarrier {
            max_orders: 1,
            window_nanoseconds: 0,
        };
        let mut out_error = std::ptr::null_mut();
        let ok = unsafe {
            openpit_engine_configure_rate_limit(
                engine,
                OpenPitStringView::from_utf8("RateLimitPolicy"),
                &zero_window,
                true,
                std::ptr::null(),
                0,
                false,
                std::ptr::null(),
                0,
                false,
                std::ptr::null(),
                0,
                false,
                &mut out_error,
            )
        };
        assert!(!ok, "zero-window broker retune must fail");
        let (kind, _message) = configure_error_message(out_error);
        assert_eq!(kind, crate::engine::OpenPitConfigureErrorKind::Validation);

        // Negative windows are rejected at the Rust FFI boundary before they
        // can be reinterpreted as huge unsigned durations.
        let negative_window = OpenPitPretradePoliciesRateLimitBrokerBarrier {
            max_orders: 1,
            window_nanoseconds: -1_000_000_000,
        };
        let mut out_error = std::ptr::null_mut();
        let ok = unsafe {
            openpit_engine_configure_rate_limit(
                engine,
                OpenPitStringView::from_utf8("RateLimitPolicy"),
                &negative_window,
                true,
                std::ptr::null(),
                0,
                false,
                std::ptr::null(),
                0,
                false,
                std::ptr::null(),
                0,
                false,
                &mut out_error,
            )
        };
        assert!(!ok, "negative-window broker retune must fail");
        let (kind, message) = configure_error_message(out_error);
        assert_eq!(kind, crate::engine::OpenPitConfigureErrorKind::Validation);
        assert!(
            message.contains("-1000000000ns"),
            "message must preserve the signed input, got: {message}"
        );
        crate::engine::openpit_destroy_engine(engine);
    }

    #[test]
    fn configure_rate_limit_rejects_null_and_invalid_utf8_names() {
        let broker = OpenPitPretradePoliciesRateLimitBrokerBarrier {
            max_orders: 100,
            window_nanoseconds: 1_000_000_000,
        };
        let engine = build_engine_with_builtin_start_policy(|builder| unsafe {
            openpit_engine_builder_add_builtin_rate_limit_policy(
                builder,
                0,
                &broker,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
            )
        });
        let invalid_utf8 = [0xff];
        let invalid_name = OpenPitStringView {
            ptr: invalid_utf8.as_ptr(),
            len: invalid_utf8.len(),
        };

        for name in [OpenPitStringView::default(), invalid_name] {
            let mut out_error = std::ptr::null_mut();
            let ok = unsafe {
                openpit_engine_configure_rate_limit(
                    engine,
                    name,
                    std::ptr::null(),
                    false,
                    std::ptr::null(),
                    0,
                    false,
                    std::ptr::null(),
                    0,
                    false,
                    std::ptr::null(),
                    0,
                    false,
                    &mut out_error,
                )
            };
            assert!(!ok);
            let (kind, _) = configure_error_message(out_error);
            assert_eq!(kind, crate::engine::OpenPitConfigureErrorKind::Validation);
        }

        crate::engine::openpit_destroy_engine(engine);
    }
}
