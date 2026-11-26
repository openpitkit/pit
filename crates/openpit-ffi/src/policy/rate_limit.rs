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
use openpit::pretrade::RuleMode;

use crate::engine::{write_configure_error, OpenPitConfigureError};
use crate::reject::{import_rule_mode, OpenPitPretradeRuleMode};

use super::*;

/// Broker-wide rate-limit barrier for
/// `openpit_engine_builder_add_builtin_rate_limit_policy`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpenPitPretradePoliciesRateLimitBrokerBarrier {
    /// Rule mode: 0 or 1 Reject, 2 ReviewWithHold, 3 ReviewWithoutHold.
    /// Values greater than 3 are an error.
    pub mode: OpenPitPretradeRuleMode,
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
    /// Rule mode: 0 or 1 Reject, 2 ReviewWithHold, 3 ReviewWithoutHold.
    /// Values greater than 3 are an error.
    pub mode: OpenPitPretradeRuleMode,
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
    /// Rule mode: 0 or 1 Reject, 2 ReviewWithHold, 3 ReviewWithoutHold.
    /// Values greater than 3 are an error.
    pub mode: OpenPitPretradeRuleMode,
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
    /// Rule mode: 0 or 1 Reject, 2 ReviewWithHold, 3 ReviewWithoutHold.
    /// Values greater than 3 are an error.
    pub mode: OpenPitPretradeRuleMode,
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

fn rate_limit_mode(
    value: OpenPitPretradeRuleMode,
    axis: &str,
    index: usize,
) -> Result<RuleMode, String> {
    import_rule_mode(value).ok_or_else(|| format!("{axis}[{index}] has invalid mode {value}"))
}

#[no_mangle]
/// Adds the built-in rate-limit policy to the engine builder.
///
/// Contract:
/// - `builder` must be a valid engine builder pointer.
/// - `policy_group_id` assigns the policy to a policy group (pass `0` for
///   default).
/// - `instance` supplies the registration name and mode.
///   Register a policy that keeps its own state (Spot Funds, the rate limit,
///   the P&L bounds kill switch, a custom policy that holds funds or counts) in
///   `OPENPIT_PRETRADE_RULE_MODE_REJECT` mode; \ref OpenPitPolicyInstance::mode
///   says what a review mode discards.
/// - Every axis may be empty (zero lengths, pointers may be null). The
///   policy then admits every order and can be configured later through
///   `openpit_engine_configure_rate_limit`.
/// - When a length is greater than zero the corresponding pointer must point
///   to that many readable entries.
/// - Each `settlement_asset` string view inside an array entry must be valid
///   for the duration of the call.
/// - Each barrier's `mode` is 0 or 1 for Reject, 2 for ReviewWithHold, or
///   3 for ReviewWithoutHold; values greater than 3 are an error.
/// - Broker barriers are an array keyed by mode. Other axes are keyed by
///   their target and mode. A repeated (key, mode) within one call fails the
///   whole call and leaves the settings unchanged.
///
/// Success:
/// - returns `true`; the builder retains the policy.
///
/// Error:
/// - returns `false` when the builder is null or already consumed, or when
///   argument parsing fails;
/// - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
///   error handle that MUST be released with `openpit_destroy_shared_string`.
pub unsafe extern "C" fn openpit_engine_builder_add_builtin_rate_limit_policy(
    builder: *mut crate::engine::OpenPitEngineBuilder,
    policy_group_id: u16,
    broker: *const OpenPitPretradePoliciesRateLimitBrokerBarrier,
    broker_len: usize,
    asset: *const OpenPitPretradePoliciesRateLimitAssetBarrier,
    asset_len: usize,
    account: *const OpenPitPretradePoliciesRateLimitAccountBarrier,
    account_len: usize,
    account_asset: *const OpenPitPretradePoliciesRateLimitAccountAssetBarrier,
    account_asset_len: usize,
    instance: *const crate::engine::OpenPitPolicyInstance,
    out_error: OpenPitOutError,
) -> bool {
    if builder.is_null() {
        write_error(out_error, "engine builder is null");
        return false;
    }
    let broker_slice =
        match unsafe { try_slice_arg(broker, broker_len, "rate_limit_policy broker", out_error) } {
            Some(v) => v,
            None => return false,
        };
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

    let mut broker_barriers = Vec::with_capacity(broker_slice.len());
    for (index, entry) in broker_slice.iter().enumerate() {
        let mode = match rate_limit_mode(entry.mode, "broker", index) {
            Ok(mode) => mode,
            Err(e) => {
                write_error(out_error, &format!("rate_limit_policy {e}"));
                return false;
            }
        };
        broker_barriers.push(RateLimitBrokerBarrier {
            mode,
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
        });
    }

    let mut asset_barriers = Vec::with_capacity(asset_slice.len());
    for (index, entry) in asset_slice.iter().enumerate() {
        let mode = match rate_limit_mode(entry.mode, "asset", index) {
            Ok(mode) => mode,
            Err(e) => {
                write_error(out_error, &format!("rate_limit_policy {e}"));
                return false;
            }
        };
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
            mode,
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
    for (index, entry) in account_slice.iter().enumerate() {
        let mode = match rate_limit_mode(entry.mode, "account", index) {
            Ok(mode) => mode,
            Err(e) => {
                write_error(out_error, &format!("rate_limit_policy {e}"));
                return false;
            }
        };
        account_barriers.push(RateLimitAccountBarrier {
            mode,
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
        let mode = match rate_limit_mode(entry.mode, "account_asset", index) {
            Ok(mode) => mode,
            Err(e) => {
                write_error(out_error, &format!("rate_limit_policy {e}"));
                return false;
            }
        };
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
            mode,
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
        broker_barriers,
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
    match crate::engine::add_pre_trade_policy_to_builder(builder_ref, policy, instance) {
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
/// `has_*` flag is `true`. A touched axis is replaced wholesale - barriers
/// can be added and removed at runtime. A (key, mode) that survives the
/// replacement keeps its live counter (no reset). An empty axis (`len` 0
/// with `has_*` true) clears every mode on that axis. Clearing every axis
/// is valid; the policy then admits every order and can be configured again
/// through this function.
///
/// Clearing an account or account+asset key retains its stored sliding log,
/// even when every axis becomes empty; re-adding that key counts orders still
/// within its window. Re-added broker and asset barriers start fresh windows.
///
/// Contract:
/// - `engine` must be a valid non-null engine pointer.
/// - `name` selects the policy; it is interpreted as UTF-8. A built-in
///   policy added via `openpit_engine_builder_add_builtin_rate_limit_policy`
///   registers under its fixed name `"RateLimitPolicy"`, so pass that string
///   here.
/// - When `has_broker`/`has_asset`/`has_account`/`has_account_asset` is `true`,
///   the matching pointer must point to `*_len` readable entries (a length of
///   zero clears that axis and allows a null pointer). Each `settlement_asset`
///   view must be valid for the duration of the call.
/// - A `has_*` flag set to `false` leaves that axis untouched regardless of
///   the pointer/length arguments.
/// - Each barrier's `mode` is 0 or 1 for Reject, 2 for ReviewWithHold, or
///   3 for ReviewWithoutHold; values greater than 3 are a Validation error.
/// - Broker barriers are an array keyed by mode. Other axes are keyed by
///   their target and mode. A repeated (key, mode) within one call fails the
///   whole call with Validation and leaves the settings unchanged.
///
/// Success:
/// - returns `true`; the new limits apply from the next order onward, retaining
///   counters for surviving keys.
///
/// Error:
/// - returns `false`; if `out_error` is non-null, writes a caller-owned
///   `OpenPitConfigureError` (release with `openpit_destroy_configure_error`)
///   describing the unknown policy, settings-type mismatch, or rejected
///   update.
/// - a null `engine` returns `false` and, when `out_error` is non-null, writes
///   a caller-owned `OpenPitConfigureError` (`Validation`) that must be
///   released with `openpit_destroy_configure_error`.
pub unsafe extern "C" fn openpit_engine_configure_rate_limit(
    engine: *mut crate::engine::OpenPitEngine,
    name: OpenPitStringView,
    broker: *const OpenPitPretradePoliciesRateLimitBrokerBarrier,
    broker_len: usize,
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
    // configure closure only receives parsed values. An argument that
    // cannot be parsed (e.g. an invalid asset code) never reaches the core
    // configurator and is reported as a `Validation` error.
    let broker_barriers: Vec<RateLimitBrokerBarrier> = if has_broker {
        let slice = match unsafe {
            try_slice_arg(
                broker,
                broker_len,
                "rate_limit broker",
                std::ptr::null_mut(),
            )
        } {
            Some(v) => v,
            None => {
                write_configure_error(
                    out_error,
                    OpenPitConfigureError::validation("rate_limit broker is null".to_owned()),
                );
                return false;
            }
        };
        let mut out = Vec::with_capacity(slice.len());
        for (index, entry) in slice.iter().enumerate() {
            let mode = match rate_limit_mode(entry.mode, "broker", index) {
                Ok(mode) => mode,
                Err(e) => {
                    write_configure_error(
                        out_error,
                        OpenPitConfigureError::validation(format!("rate_limit {e}")),
                    );
                    return false;
                }
            };
            out.push(RateLimitBrokerBarrier {
                mode,
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
            });
        }
        out
    } else {
        Vec::new()
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
            let mode = match rate_limit_mode(entry.mode, "asset", index) {
                Ok(mode) => mode,
                Err(e) => {
                    write_configure_error(
                        out_error,
                        OpenPitConfigureError::validation(format!("rate_limit {e}")),
                    );
                    return false;
                }
            };
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
                mode,
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
        for (index, entry) in slice.iter().enumerate() {
            let mode = match rate_limit_mode(entry.mode, "account", index) {
                Ok(mode) => mode,
                Err(e) => {
                    write_configure_error(
                        out_error,
                        OpenPitConfigureError::validation(format!("rate_limit {e}")),
                    );
                    return false;
                }
            };
            out.push(RateLimitAccountBarrier {
                mode,
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
            let mode = match rate_limit_mode(entry.mode, "account_asset", index) {
                Ok(mode) => mode,
                Err(e) => {
                    write_configure_error(
                        out_error,
                        OpenPitConfigureError::validation(format!("rate_limit {e}")),
                    );
                    return false;
                }
            };
            out.push(RateLimitAccountAssetBarrier {
                mode,
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
                settings.set_broker_barriers(broker_barriers.iter().cloned())?;
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
    use crate::reject::{
        OPENPIT_PRETRADE_RULE_MODE_REJECT, OPENPIT_PRETRADE_RULE_MODE_REVIEW_WITHOUT_HOLD,
        OPENPIT_PRETRADE_RULE_MODE_REVIEW_WITH_HOLD,
    };

    fn broker_barrier(
        mode: OpenPitPretradeRuleMode,
        max_orders: usize,
    ) -> OpenPitPretradePoliciesRateLimitBrokerBarrier {
        OpenPitPretradePoliciesRateLimitBrokerBarrier {
            mode,
            max_orders,
            window_nanoseconds: 3_600_000_000_000,
        }
    }

    fn add_rate_limit_arrays(
        builder: *mut crate::engine::OpenPitEngineBuilder,
        broker: &[OpenPitPretradePoliciesRateLimitBrokerBarrier],
        asset: &[OpenPitPretradePoliciesRateLimitAssetBarrier],
        account: &[OpenPitPretradePoliciesRateLimitAccountBarrier],
        account_asset: &[OpenPitPretradePoliciesRateLimitAccountAssetBarrier],
        out_error: OpenPitOutError,
    ) -> bool {
        unsafe {
            openpit_engine_builder_add_builtin_rate_limit_policy(
                builder,
                0,
                broker.as_ptr(),
                broker.len(),
                asset.as_ptr(),
                asset.len(),
                account.as_ptr(),
                account.len(),
                account_asset.as_ptr(),
                account_asset.len(),
                std::ptr::null(),
                out_error,
            )
        }
    }

    fn configure_rate_limit_arrays(
        engine: *mut crate::engine::OpenPitEngine,
        broker: &[OpenPitPretradePoliciesRateLimitBrokerBarrier],
        asset: &[OpenPitPretradePoliciesRateLimitAssetBarrier],
        account: &[OpenPitPretradePoliciesRateLimitAccountBarrier],
        account_asset: &[OpenPitPretradePoliciesRateLimitAccountAssetBarrier],
        has_axes: [bool; 4],
        out_error: *mut *mut OpenPitConfigureError,
    ) -> bool {
        unsafe {
            openpit_engine_configure_rate_limit(
                engine,
                OpenPitStringView::from_utf8("RateLimitPolicy"),
                broker.as_ptr(),
                broker.len(),
                has_axes[0],
                asset.as_ptr(),
                asset.len(),
                has_axes[1],
                account.as_ptr(),
                account.len(),
                has_axes[2],
                account_asset.as_ptr(),
                account_asset.len(),
                has_axes[3],
                out_error,
            )
        }
    }

    fn build_rate_limit_engine(
        broker: &[OpenPitPretradePoliciesRateLimitBrokerBarrier],
    ) -> *mut crate::engine::OpenPitEngine {
        build_engine_with_builtin_start_policy(|builder| {
            add_rate_limit_arrays(builder, broker, &[], &[], &[], std::ptr::null_mut())
        })
    }

    fn builder_error_message(error: *mut crate::string::OpenPitSharedString) -> String {
        assert!(!error.is_null(), "builder error must be populated");
        let view = crate::string::openpit_shared_string_view(error);
        let bytes = unsafe { std::slice::from_raw_parts(view.ptr, view.len) };
        let message = std::str::from_utf8(bytes).expect("utf8").to_owned();
        crate::string::openpit_destroy_shared_string(error);
        message
    }

    fn assert_rate_limit_mark(
        list: *mut crate::reject::OpenPitPretradeRejectList,
        mode: OpenPitPretradeRuleMode,
    ) {
        assert!(!list.is_null(), "mark list must be populated");
        assert_eq!(crate::reject::openpit_pretrade_reject_list_len(list), 1);
        let mut mark = std::mem::MaybeUninit::uninit();
        assert!(crate::reject::openpit_pretrade_reject_list_get(
            list,
            0,
            mark.as_mut_ptr(),
        ));
        let mark = unsafe { mark.assume_init() };
        assert_eq!(mark.mode, mode, "mark must preserve the barrier mode");
        assert_eq!(
            mark.code,
            crate::reject::OPENPIT_PRETRADE_REJECT_CODE_RATE_LIMIT_EXCEEDED
        );
        crate::reject::openpit_destroy_pretrade_reject_list(list);
    }

    fn assert_no_rate_limit_mark(list: *mut crate::reject::OpenPitPretradeRejectList) {
        assert!(!list.is_null(), "mark list must be populated");
        assert_eq!(crate::reject::openpit_pretrade_reject_list_len(list), 0);
        crate::reject::openpit_destroy_pretrade_reject_list(list);
    }

    #[test]
    fn rate_limit_broker_reject_wins_over_review() {
        let broker = [
            broker_barrier(OPENPIT_PRETRADE_RULE_MODE_REJECT, 1),
            broker_barrier(OPENPIT_PRETRADE_RULE_MODE_REVIEW_WITHOUT_HOLD, 1),
        ];
        let engine = build_rate_limit_engine(&broker);
        run_start_pre_trade_passes(engine);
        let mut request = std::ptr::null_mut();
        let mut rejects = std::ptr::null_mut();
        let status = crate::engine::openpit_engine_start_pre_trade(
            engine,
            &valid_pit_order(),
            &mut request,
            &mut rejects,
            std::ptr::null_mut(),
        );
        assert_eq!(status, crate::engine::OpenPitPretradeStatus::Rejected);
        assert!(request.is_null());
        assert_rate_limit_mark(rejects, OPENPIT_PRETRADE_RULE_MODE_REJECT);
        crate::engine::openpit_destroy_engine(engine);
    }

    #[test]
    fn rate_limit_broker_review_getter_preserves_mode() {
        for mode in [
            OPENPIT_PRETRADE_RULE_MODE_REVIEW_WITH_HOLD,
            OPENPIT_PRETRADE_RULE_MODE_REVIEW_WITHOUT_HOLD,
        ] {
            let engine = build_rate_limit_engine(&[
                broker_barrier(mode, 1),
                broker_barrier(OPENPIT_PRETRADE_RULE_MODE_REJECT, 3),
            ]);
            run_start_pre_trade_passes(engine);
            let mut request = std::ptr::null_mut();
            let mut rejects = std::ptr::null_mut();
            let status = crate::engine::openpit_engine_start_pre_trade(
                engine,
                &valid_pit_order(),
                &mut request,
                &mut rejects,
                std::ptr::null_mut(),
            );
            assert_eq!(status, crate::engine::OpenPitPretradeStatus::Passed);
            assert!(rejects.is_null());
            let mut error = std::ptr::null_mut();
            let reviews =
                crate::engine::openpit_pretrade_pre_trade_request_get_reviews(request, &mut error);
            assert!(error.is_null());
            assert_rate_limit_mark(reviews, mode);
            crate::engine::openpit_destroy_pretrade_pre_trade_request(request);
            crate::engine::openpit_destroy_engine(engine);
        }
    }

    #[test]
    fn rate_limit_confirmed_rerun_counts_both_thresholds_with_already_held() {
        for mode in [
            OPENPIT_PRETRADE_RULE_MODE_REVIEW_WITH_HOLD,
            OPENPIT_PRETRADE_RULE_MODE_REVIEW_WITHOUT_HOLD,
        ] {
            for already_held in [false, true] {
                let engine = build_rate_limit_engine(&[
                    broker_barrier(OPENPIT_PRETRADE_RULE_MODE_REJECT, 5),
                    broker_barrier(mode, 3),
                ]);
                run_start_pre_trade_passes(engine);
                for attempt in 2..=6 {
                    let mut order = valid_pit_order();
                    if attempt != 3 && attempt != 4 {
                        order.review.is_set = true;
                        order.review.value.confirmed = crate::param::OPENPIT_TRI_BOOL_TRUE;
                        if already_held {
                            order.review.value.already_held = crate::param::OPENPIT_TRI_BOOL_TRUE;
                        }
                    }
                    let mut request = std::ptr::null_mut();
                    let mut rejects = std::ptr::null_mut();
                    let mut error = std::ptr::null_mut();
                    let status = crate::engine::openpit_engine_start_pre_trade(
                        engine,
                        &order,
                        &mut request,
                        &mut rejects,
                        &mut error,
                    );
                    assert!(error.is_null());
                    if attempt == 6 {
                        assert_eq!(status, crate::engine::OpenPitPretradeStatus::Rejected);
                        assert!(request.is_null());
                        assert_rate_limit_mark(rejects, OPENPIT_PRETRADE_RULE_MODE_REJECT);
                    } else {
                        assert_eq!(status, crate::engine::OpenPitPretradeStatus::Passed);
                        assert!(!request.is_null());
                        assert!(rejects.is_null());
                        let reviews = crate::engine::openpit_pretrade_pre_trade_request_get_reviews(
                            request, &mut error,
                        );
                        assert!(error.is_null());
                        if attempt != 4 {
                            assert_no_rate_limit_mark(reviews);
                        } else {
                            assert_rate_limit_mark(reviews, mode);
                        }
                        crate::engine::openpit_destroy_pretrade_pre_trade_request(request);
                    }
                }
                crate::engine::openpit_destroy_engine(engine);
            }
        }
    }

    #[test]
    fn rate_limit_broker_replacement_preserves_surviving_mode_counter() {
        for mode in [
            OPENPIT_PRETRADE_RULE_MODE_REJECT,
            OPENPIT_PRETRADE_RULE_MODE_REVIEW_WITH_HOLD,
            OPENPIT_PRETRADE_RULE_MODE_REVIEW_WITHOUT_HOLD,
        ] {
            let engine = build_rate_limit_engine(&[broker_barrier(mode, 2)]);
            run_start_pre_trade_passes(engine);
            let added_mode = if mode == OPENPIT_PRETRADE_RULE_MODE_REJECT {
                OPENPIT_PRETRADE_RULE_MODE_REVIEW_WITH_HOLD
            } else {
                OPENPIT_PRETRADE_RULE_MODE_REJECT
            };
            let replacement = [broker_barrier(added_mode, 100), broker_barrier(mode, 4)];
            let mut error = std::ptr::null_mut();
            assert!(configure_rate_limit_arrays(
                engine,
                &replacement,
                &[],
                &[],
                &[],
                [true, false, false, false],
                &mut error
            ));
            assert!(error.is_null());
            for count in 2..=5 {
                let mut error = std::ptr::null_mut();
                let mut request = std::ptr::null_mut();
                let mut rejects = std::ptr::null_mut();
                let status = crate::engine::openpit_engine_start_pre_trade(
                    engine,
                    &valid_pit_order(),
                    &mut request,
                    &mut rejects,
                    &mut error,
                );
                assert!(error.is_null());
                if count == 5 && mode == OPENPIT_PRETRADE_RULE_MODE_REJECT {
                    assert_eq!(status, crate::engine::OpenPitPretradeStatus::Rejected);
                    assert!(request.is_null());
                    assert_rate_limit_mark(rejects, mode);
                } else {
                    assert_eq!(status, crate::engine::OpenPitPretradeStatus::Passed);
                    assert!(!request.is_null());
                    assert!(rejects.is_null());
                    let reviews = crate::engine::openpit_pretrade_pre_trade_request_get_reviews(
                        request, &mut error,
                    );
                    assert!(error.is_null());
                    if count <= 4 {
                        assert_no_rate_limit_mark(reviews);
                    } else {
                        assert_rate_limit_mark(reviews, mode);
                    }
                    crate::engine::openpit_destroy_pretrade_pre_trade_request(request);
                }
            }
            crate::engine::openpit_destroy_engine(engine);
        }
    }

    #[test]
    fn rate_limit_review_dry_run_spends_no_budget() {
        for mode in [
            OPENPIT_PRETRADE_RULE_MODE_REVIEW_WITH_HOLD,
            OPENPIT_PRETRADE_RULE_MODE_REVIEW_WITHOUT_HOLD,
        ] {
            let engine = build_rate_limit_engine(&[broker_barrier(mode, 1)]);
            for attempt in 0..3 {
                let mut error = std::ptr::null_mut();
                let reviews;
                if attempt % 2 == 0 {
                    let mut report = std::ptr::null_mut();
                    assert!(crate::engine::openpit_engine_start_pre_trade_dry_run(
                        engine,
                        &valid_pit_order(),
                        &mut report,
                        &mut error
                    ));
                    assert!(error.is_null());
                    assert!(!report.is_null());
                    assert!(
                        crate::engine::openpit_pretrade_pre_trade_dry_run_report_is_pass(report)
                    );
                    reviews = crate::engine::openpit_pretrade_pre_trade_dry_run_report_get_reviews(
                        report,
                    );
                    crate::engine::openpit_destroy_pretrade_pre_trade_dry_run_report(report);
                } else {
                    let mut request = std::ptr::null_mut();
                    let mut rejects = std::ptr::null_mut();
                    assert_eq!(
                        crate::engine::openpit_engine_start_pre_trade(
                            engine,
                            &valid_pit_order(),
                            &mut request,
                            &mut rejects,
                            &mut error
                        ),
                        crate::engine::OpenPitPretradeStatus::Passed
                    );
                    assert!(error.is_null());
                    assert!(!request.is_null());
                    assert!(rejects.is_null());
                    reviews = crate::engine::openpit_pretrade_pre_trade_request_get_reviews(
                        request, &mut error,
                    );
                    assert!(error.is_null());
                    crate::engine::openpit_destroy_pretrade_pre_trade_request(request);
                }
                if attempt < 2 {
                    assert_no_rate_limit_mark(reviews);
                } else {
                    assert_rate_limit_mark(reviews, mode);
                }
            }
            let mut error = std::ptr::null_mut();
            assert!(configure_rate_limit_arrays(
                engine,
                &[broker_barrier(mode, 2)],
                &[],
                &[],
                &[],
                [true, false, false, false],
                &mut error
            ));
            assert!(error.is_null());
            for count in 2..=3 {
                let mut error = std::ptr::null_mut();
                let mut request = std::ptr::null_mut();
                let mut rejects = std::ptr::null_mut();
                assert_eq!(
                    crate::engine::openpit_engine_start_pre_trade(
                        engine,
                        &valid_pit_order(),
                        &mut request,
                        &mut rejects,
                        &mut error
                    ),
                    crate::engine::OpenPitPretradeStatus::Passed
                );
                assert!(error.is_null());
                assert!(!request.is_null());
                assert!(rejects.is_null());
                let reviews = crate::engine::openpit_pretrade_pre_trade_request_get_reviews(
                    request, &mut error,
                );
                assert!(error.is_null());
                if count == 2 {
                    assert_no_rate_limit_mark(reviews);
                } else {
                    assert_rate_limit_mark(reviews, mode);
                }
                crate::engine::openpit_destroy_pretrade_pre_trade_request(request);
            }
            crate::engine::openpit_destroy_engine(engine);
        }
    }

    #[test]
    fn rate_limit_blocked_account_spends_no_budget() {
        for mode in [
            OPENPIT_PRETRADE_RULE_MODE_REVIEW_WITH_HOLD,
            OPENPIT_PRETRADE_RULE_MODE_REVIEW_WITHOUT_HOLD,
        ] {
            let engine = build_rate_limit_engine(&[
                broker_barrier(OPENPIT_PRETRADE_RULE_MODE_REJECT, 1),
                broker_barrier(mode, 1),
            ]);
            crate::engine::openpit_engine_block_account(
                engine,
                7,
                OpenPitStringView::from_utf8("administrative block"),
            );
            let mut request = std::ptr::null_mut();
            let mut rejects = std::ptr::null_mut();
            let mut error = std::ptr::null_mut();
            assert_eq!(
                crate::engine::openpit_engine_start_pre_trade(
                    engine,
                    &valid_pit_order(),
                    &mut request,
                    &mut rejects,
                    &mut error
                ),
                crate::engine::OpenPitPretradeStatus::Rejected
            );
            assert!(error.is_null());
            assert!(request.is_null());
            let mut reject = std::mem::MaybeUninit::uninit();
            assert_eq!(crate::reject::openpit_pretrade_reject_list_len(rejects), 1);
            assert!(crate::reject::openpit_pretrade_reject_list_get(
                rejects,
                0,
                reject.as_mut_ptr()
            ));
            assert_eq!(
                unsafe { reject.assume_init() }.code,
                crate::reject::OPENPIT_PRETRADE_REJECT_CODE_ACCOUNT_BLOCKED
            );
            crate::reject::openpit_destroy_pretrade_reject_list(rejects);
            crate::engine::openpit_engine_unblock_account(engine, 7);
            request = std::ptr::null_mut();
            rejects = std::ptr::null_mut();
            assert_eq!(
                crate::engine::openpit_engine_start_pre_trade(
                    engine,
                    &valid_pit_order(),
                    &mut request,
                    &mut rejects,
                    &mut error
                ),
                crate::engine::OpenPitPretradeStatus::Passed,
                "blocked-account rejection spends neither threshold's budget"
            );
            assert!(error.is_null());
            assert!(!request.is_null());
            assert!(rejects.is_null());
            let reviews =
                crate::engine::openpit_pretrade_pre_trade_request_get_reviews(request, &mut error);
            assert!(error.is_null());
            assert_no_rate_limit_mark(reviews);
            crate::engine::openpit_destroy_pretrade_pre_trade_request(request);
            request = std::ptr::null_mut();
            assert_eq!(
                crate::engine::openpit_engine_start_pre_trade(
                    engine,
                    &valid_pit_order(),
                    &mut request,
                    &mut rejects,
                    &mut error
                ),
                crate::engine::OpenPitPretradeStatus::Rejected
            );
            assert!(error.is_null());
            assert!(request.is_null());
            assert_rate_limit_mark(rejects, OPENPIT_PRETRADE_RULE_MODE_REJECT);
            crate::engine::openpit_destroy_engine(engine);
        }
    }

    #[test]
    fn rate_limit_execute_spends_no_second_unit() {
        for mode in [
            OPENPIT_PRETRADE_RULE_MODE_REVIEW_WITH_HOLD,
            OPENPIT_PRETRADE_RULE_MODE_REVIEW_WITHOUT_HOLD,
        ] {
            let engine = build_rate_limit_engine(&[
                broker_barrier(OPENPIT_PRETRADE_RULE_MODE_REJECT, 3),
                broker_barrier(mode, 2),
            ]);
            let mut request = std::ptr::null_mut();
            let mut rejects = std::ptr::null_mut();
            let mut error = std::ptr::null_mut();
            assert_eq!(
                crate::engine::openpit_engine_start_pre_trade(
                    engine,
                    &valid_pit_order(),
                    &mut request,
                    &mut rejects,
                    &mut error
                ),
                crate::engine::OpenPitPretradeStatus::Passed
            );
            assert!(error.is_null());
            assert!(!request.is_null());
            assert!(rejects.is_null());
            let mut reservation = std::ptr::null_mut();
            assert_eq!(
                crate::engine::openpit_pretrade_pre_trade_request_execute(
                    request,
                    &mut reservation,
                    &mut rejects,
                    &mut error
                ),
                crate::engine::OpenPitPretradeStatus::Passed
            );
            assert!(error.is_null());
            assert!(!reservation.is_null());
            assert!(rejects.is_null());
            crate::engine::openpit_pretrade_pre_trade_reservation_commit(reservation);
            crate::engine::openpit_destroy_pretrade_pre_trade_reservation(reservation);
            crate::engine::openpit_destroy_pretrade_pre_trade_request(request);
            for attempt in 2..=3 {
                request = std::ptr::null_mut();
                assert_eq!(
                    crate::engine::openpit_engine_start_pre_trade(
                        engine,
                        &valid_pit_order(),
                        &mut request,
                        &mut rejects,
                        &mut error
                    ),
                    crate::engine::OpenPitPretradeStatus::Passed,
                    "execute must not spend a second Reject unit"
                );
                assert!(error.is_null());
                assert!(!request.is_null());
                assert!(rejects.is_null());
                let reviews = crate::engine::openpit_pretrade_pre_trade_request_get_reviews(
                    request, &mut error,
                );
                assert!(error.is_null());
                if attempt == 2 {
                    assert_no_rate_limit_mark(reviews);
                } else {
                    assert_rate_limit_mark(reviews, mode);
                }
                crate::engine::openpit_destroy_pretrade_pre_trade_request(request);
            }
            request = std::ptr::null_mut();
            assert_eq!(
                crate::engine::openpit_engine_start_pre_trade(
                    engine,
                    &valid_pit_order(),
                    &mut request,
                    &mut rejects,
                    &mut error
                ),
                crate::engine::OpenPitPretradeStatus::Rejected,
                "the fourth start must exhaust the Reject budget"
            );
            assert!(error.is_null());
            assert!(request.is_null());
            assert_rate_limit_mark(rejects, OPENPIT_PRETRADE_RULE_MODE_REJECT);
            crate::engine::openpit_destroy_engine(engine);
        }
    }

    #[test]
    fn rate_limit_zero_mode_is_reject_and_duplicate_modes_fail() {
        let mut broker = [broker_barrier(OPENPIT_PRETRADE_RULE_MODE_REJECT, 1); 2];
        let mut asset = [OpenPitPretradePoliciesRateLimitAssetBarrier {
            settlement_asset: OpenPitStringView::from_utf8("USD"),
            mode: OPENPIT_PRETRADE_RULE_MODE_REJECT,
            max_orders: 1,
            window_nanoseconds: 3_600_000_000_000,
        }; 2];
        let mut account = [OpenPitPretradePoliciesRateLimitAccountBarrier {
            account_id: 7,
            mode: OPENPIT_PRETRADE_RULE_MODE_REJECT,
            max_orders: 1,
            window_nanoseconds: 3_600_000_000_000,
        }; 2];
        let mut account_asset = [OpenPitPretradePoliciesRateLimitAccountAssetBarrier {
            account_id: 7,
            settlement_asset: OpenPitStringView::from_utf8("USD"),
            mode: OPENPIT_PRETRADE_RULE_MODE_REJECT,
            max_orders: 1,
            window_nanoseconds: 3_600_000_000_000,
        }; 2];
        for first_mode in [0, OPENPIT_PRETRADE_RULE_MODE_REJECT] {
            broker[0].mode = first_mode;
            asset[0].mode = first_mode;
            account[0].mode = first_mode;
            account_asset[0].mode = first_mode;
            for axis in 0..4 {
                let builder = crate::engine::openpit_create_engine_builder(
                    crate::engine::OpenPitSyncPolicy::Full as u8,
                    std::ptr::null_mut(),
                );
                let mut error = std::ptr::null_mut();
                let ok = add_rate_limit_arrays(
                    builder,
                    if axis == 0 { &broker } else { &[] },
                    if axis == 1 { &asset } else { &[] },
                    if axis == 2 { &account } else { &[] },
                    if axis == 3 { &account_asset } else { &[] },
                    &mut error,
                );
                assert!(!ok, "duplicate axis {axis} must fail");
                let message = builder_error_message(error);
                assert!(message.contains("duplicate"), "{message}");
                assert!(message.contains("Reject"), "{message}");
                assert!(
                    add_rate_limit_arrays(builder, &[], &[], &[], &[], std::ptr::null_mut(),),
                    "failed registration must leave the builder unchanged"
                );
                let engine = crate::engine::openpit_engine_builder_build(
                    builder,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                );
                assert!(!engine.is_null());
                run_start_pre_trade_passes(engine);
                crate::engine::openpit_destroy_engine(engine);
                crate::engine::openpit_destroy_engine_builder(builder);
            }
        }
    }

    #[test]
    fn rate_limit_same_key_different_modes_builds() {
        let broker = [
            broker_barrier(OPENPIT_PRETRADE_RULE_MODE_REJECT, 1),
            broker_barrier(OPENPIT_PRETRADE_RULE_MODE_REVIEW_WITHOUT_HOLD, 1),
        ];
        let asset = broker.map(|barrier| OpenPitPretradePoliciesRateLimitAssetBarrier {
            settlement_asset: OpenPitStringView::from_utf8("USD"),
            mode: barrier.mode,
            max_orders: barrier.max_orders,
            window_nanoseconds: barrier.window_nanoseconds,
        });
        let account = broker.map(|barrier| OpenPitPretradePoliciesRateLimitAccountBarrier {
            account_id: 7,
            mode: barrier.mode,
            max_orders: barrier.max_orders,
            window_nanoseconds: barrier.window_nanoseconds,
        });
        let account_asset =
            broker.map(
                |barrier| OpenPitPretradePoliciesRateLimitAccountAssetBarrier {
                    account_id: 7,
                    settlement_asset: OpenPitStringView::from_utf8("USD"),
                    mode: barrier.mode,
                    max_orders: barrier.max_orders,
                    window_nanoseconds: barrier.window_nanoseconds,
                },
            );
        let engine = build_engine_with_builtin_start_policy(|builder| {
            add_rate_limit_arrays(
                builder,
                &broker,
                &asset,
                &account,
                &account_asset,
                std::ptr::null_mut(),
            )
        });
        run_start_pre_trade_passes(engine);
        run_start_pre_trade_rejected(engine);
        crate::engine::openpit_destroy_engine(engine);
    }

    #[test]
    fn rate_limit_configure_duplicate_is_atomic() {
        let asset = [OpenPitPretradePoliciesRateLimitAssetBarrier {
            settlement_asset: OpenPitStringView::from_utf8("USD"),
            mode: OPENPIT_PRETRADE_RULE_MODE_REJECT,
            max_orders: 100,
            window_nanoseconds: 3_600_000_000_000,
        }; 2];
        for duplicate_broker in [true, false] {
            let engine =
                build_rate_limit_engine(&[broker_barrier(OPENPIT_PRETRADE_RULE_MODE_REJECT, 1)]);
            run_start_pre_trade_passes(engine);
            let broker = [broker_barrier(OPENPIT_PRETRADE_RULE_MODE_REJECT, 100); 2];
            let mut error = std::ptr::null_mut();
            let ok = configure_rate_limit_arrays(
                engine,
                if duplicate_broker {
                    &broker
                } else {
                    &broker[..1]
                },
                if duplicate_broker { &[] } else { &asset },
                &[],
                &[],
                [true, true, false, false],
                &mut error,
            );
            assert!(!ok, "duplicate configure must fail");
            let (kind, message) = configure_error_message(error);
            assert_eq!(kind, crate::engine::OpenPitConfigureErrorKind::Validation);
            assert!(message.contains("duplicate"), "{message}");
            assert!(message.contains("Reject"), "{message}");
            run_start_pre_trade_rejected(engine);
            crate::engine::openpit_destroy_engine(engine);
        }
    }

    #[test]
    fn rate_limit_configure_zero_mode_is_reject_and_duplicate_is_atomic() {
        let broker = [
            broker_barrier(0, 100),
            broker_barrier(OPENPIT_PRETRADE_RULE_MODE_REJECT, 100),
        ];
        let mut asset = [OpenPitPretradePoliciesRateLimitAssetBarrier {
            settlement_asset: OpenPitStringView::from_utf8("USD"),
            mode: OPENPIT_PRETRADE_RULE_MODE_REJECT,
            max_orders: 100,
            window_nanoseconds: 3_600_000_000_000,
        }; 2];
        let mut account = [OpenPitPretradePoliciesRateLimitAccountBarrier {
            account_id: 7,
            mode: OPENPIT_PRETRADE_RULE_MODE_REJECT,
            max_orders: 100,
            window_nanoseconds: 3_600_000_000_000,
        }; 2];
        let mut account_asset = [OpenPitPretradePoliciesRateLimitAccountAssetBarrier {
            account_id: 7,
            settlement_asset: OpenPitStringView::from_utf8("USD"),
            mode: OPENPIT_PRETRADE_RULE_MODE_REJECT,
            max_orders: 100,
            window_nanoseconds: 3_600_000_000_000,
        }; 2];
        asset[0].mode = 0;
        account[0].mode = 0;
        account_asset[0].mode = 0;
        for axis in 0..4 {
            let engine =
                build_rate_limit_engine(&[broker_barrier(OPENPIT_PRETRADE_RULE_MODE_REJECT, 1)]);
            run_start_pre_trade_passes(engine);
            let mut error = std::ptr::null_mut();
            let ok = configure_rate_limit_arrays(
                engine,
                if axis == 0 { &broker } else { &broker[1..] },
                if axis == 1 { &asset } else { &[] },
                if axis == 2 { &account } else { &[] },
                if axis == 3 { &account_asset } else { &[] },
                [true; 4],
                &mut error,
            );
            assert!(!ok, "duplicate axis {axis} must fail");
            let (kind, message) = configure_error_message(error);
            assert_eq!(kind, crate::engine::OpenPitConfigureErrorKind::Validation);
            assert!(message.contains("duplicate"), "{message}");
            assert!(message.contains("Reject"), "{message}");
            run_start_pre_trade_rejected(engine);
            crate::engine::openpit_destroy_engine(engine);
        }
    }

    #[test]
    fn rate_limit_invalid_modes_fail_atomically() {
        for bad_mode in [4] {
            for (axis, axis_name) in ["broker", "asset", "account", "account_asset"]
                .iter()
                .enumerate()
            {
                let mut broker = [
                    broker_barrier(OPENPIT_PRETRADE_RULE_MODE_REJECT, 100),
                    broker_barrier(OPENPIT_PRETRADE_RULE_MODE_REVIEW_WITHOUT_HOLD, 100),
                ];
                let mut asset =
                    ["USD", "EUR"].map(|asset| OpenPitPretradePoliciesRateLimitAssetBarrier {
                        settlement_asset: OpenPitStringView::from_utf8(asset),
                        mode: OPENPIT_PRETRADE_RULE_MODE_REJECT,
                        max_orders: 100,
                        window_nanoseconds: 3_600_000_000_000,
                    });
                let mut account =
                    [7, 8].map(
                        |account_id| OpenPitPretradePoliciesRateLimitAccountBarrier {
                            account_id,
                            mode: OPENPIT_PRETRADE_RULE_MODE_REJECT,
                            max_orders: 100,
                            window_nanoseconds: 3_600_000_000_000,
                        },
                    );
                let mut account_asset =
                    [7, 8].map(
                        |account_id| OpenPitPretradePoliciesRateLimitAccountAssetBarrier {
                            account_id,
                            settlement_asset: OpenPitStringView::from_utf8("USD"),
                            mode: OPENPIT_PRETRADE_RULE_MODE_REJECT,
                            max_orders: 100,
                            window_nanoseconds: 3_600_000_000_000,
                        },
                    );
                match axis {
                    0 => broker[1].mode = bad_mode,
                    1 => asset[1].mode = bad_mode,
                    2 => account[1].mode = bad_mode,
                    3 => account_asset[1].mode = bad_mode,
                    _ => unreachable!(),
                }
                let builder = crate::engine::openpit_create_engine_builder(
                    crate::engine::OpenPitSyncPolicy::Full as u8,
                    std::ptr::null_mut(),
                );
                let mut error = std::ptr::null_mut();
                assert!(
                    !add_rate_limit_arrays(
                        builder,
                        &broker,
                        &asset,
                        &account,
                        &account_asset,
                        &mut error,
                    ),
                    "builder must reject {axis_name} mode {bad_mode}"
                );
                let message = builder_error_message(error);
                let expected = format!("{axis_name}[1] has invalid mode {bad_mode}");
                assert!(message.contains(&expected), "{message}");
                crate::engine::openpit_destroy_engine_builder(builder);

                let engine = build_rate_limit_engine(&[broker_barrier(
                    OPENPIT_PRETRADE_RULE_MODE_REJECT,
                    1,
                )]);
                run_start_pre_trade_passes(engine);
                let mut error = std::ptr::null_mut();
                assert!(
                    !configure_rate_limit_arrays(
                        engine,
                        &broker,
                        &asset,
                        &account,
                        &account_asset,
                        [true; 4],
                        &mut error,
                    ),
                    "configure must reject {axis_name} mode {bad_mode}"
                );
                let (kind, message) = configure_error_message(error);
                assert_eq!(kind, crate::engine::OpenPitConfigureErrorKind::Validation);
                assert!(message.contains(&expected), "{message}");
                run_start_pre_trade_rejected(engine);
                crate::engine::openpit_destroy_engine(engine);
            }
        }
    }

    #[test]
    fn rate_limit_configure_broker_clear_and_untouched() {
        let broker = [
            broker_barrier(OPENPIT_PRETRADE_RULE_MODE_REJECT, 1),
            broker_barrier(OPENPIT_PRETRADE_RULE_MODE_REVIEW_WITH_HOLD, 1),
            broker_barrier(OPENPIT_PRETRADE_RULE_MODE_REVIEW_WITHOUT_HOLD, 1),
        ];
        let engine = build_rate_limit_engine(&broker);
        run_start_pre_trade_passes(engine);
        let mut error = std::ptr::null_mut();
        let invalid = [broker_barrier(
            OPENPIT_PRETRADE_RULE_MODE_REVIEW_WITHOUT_HOLD + 1,
            100,
        )];
        assert!(
            configure_rate_limit_arrays(engine, &invalid, &[], &[], &[], [false; 4], &mut error,),
            "untouched broker must ignore the pointer and length"
        );
        assert!(error.is_null());
        run_start_pre_trade_rejected(engine);
        assert!(configure_rate_limit_arrays(
            engine,
            &[],
            &[],
            &[],
            &[],
            [true, false, false, false],
            &mut error,
        ));
        assert!(error.is_null());
        for _ in 0..2 {
            let mut request = std::ptr::null_mut();
            let mut rejects = std::ptr::null_mut();
            let status = crate::engine::openpit_engine_start_pre_trade(
                engine,
                &valid_pit_order(),
                &mut request,
                &mut rejects,
                std::ptr::null_mut(),
            );
            assert_eq!(status, crate::engine::OpenPitPretradeStatus::Passed);
            assert!(rejects.is_null());
            let mut error = std::ptr::null_mut();
            let reviews =
                crate::engine::openpit_pretrade_pre_trade_request_get_reviews(request, &mut error);
            assert!(error.is_null());
            assert!(!reviews.is_null());
            assert_eq!(
                crate::reject::openpit_pretrade_reject_list_len(reviews),
                0,
                "clearing broker must remove every review mode"
            );
            crate::reject::openpit_destroy_pretrade_reject_list(reviews);
            crate::engine::openpit_destroy_pretrade_pre_trade_request(request);
        }
        crate::engine::openpit_destroy_engine(engine);
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
        crate::engine::openpit_destroy_engine_builder(builder);
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
            review: Default::default(),
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
            mode: crate::reject::OPENPIT_PRETRADE_RULE_MODE_REJECT,
            max_orders: 100,
            window_nanoseconds: 1_000_000_000,
        };
        let engine = build_engine_with_builtin_start_policy(|builder| unsafe {
            openpit_engine_builder_add_builtin_rate_limit_policy(
                builder,
                0,
                &broker,
                1,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
                std::ptr::null_mut(),
            )
        });
        run_start_pre_trade_passes(engine);
        crate::engine::openpit_destroy_engine(engine);
    }

    #[test]
    fn add_builtin_rate_limit_policy_empty_config_builds() {
        let mut out_error = std::ptr::null_mut();
        let engine = build_engine_with_builtin_start_policy(|builder| unsafe {
            openpit_engine_builder_add_builtin_rate_limit_policy(
                builder,
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
                &mut out_error,
            )
        });
        assert!(out_error.is_null(), "empty registration must succeed");
        run_start_pre_trade_passes(engine);
        crate::engine::openpit_destroy_engine(engine);
    }

    #[test]
    fn configure_rate_limit_empty_policy_lifecycle() {
        let engine = build_engine_with_builtin_start_policy(|builder| unsafe {
            openpit_engine_builder_add_builtin_rate_limit_policy(
                builder,
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
                std::ptr::null_mut(),
            )
        });
        run_start_pre_trade_passes(engine);
        let broker = OpenPitPretradePoliciesRateLimitBrokerBarrier {
            mode: crate::reject::OPENPIT_PRETRADE_RULE_MODE_REJECT,
            max_orders: 1,
            window_nanoseconds: 3_600_000_000_000,
        };
        let asset = [OpenPitPretradePoliciesRateLimitAssetBarrier {
            settlement_asset: OpenPitStringView::from_utf8("USD"),
            mode: crate::reject::OPENPIT_PRETRADE_RULE_MODE_REJECT,
            max_orders: 1,
            window_nanoseconds: 3_600_000_000_000,
        }];
        let account = [OpenPitPretradePoliciesRateLimitAccountBarrier {
            account_id: 7,
            mode: crate::reject::OPENPIT_PRETRADE_RULE_MODE_REJECT,
            max_orders: 1,
            window_nanoseconds: 3_600_000_000_000,
        }];
        let account_asset = [OpenPitPretradePoliciesRateLimitAccountAssetBarrier {
            account_id: 7,
            settlement_asset: OpenPitStringView::from_utf8("USD"),
            mode: crate::reject::OPENPIT_PRETRADE_RULE_MODE_REJECT,
            max_orders: 1,
            window_nanoseconds: 3_600_000_000_000,
        }];
        for axis in 0..4 {
            let mut out_error = std::ptr::null_mut();
            assert!(
                unsafe {
                    openpit_engine_configure_rate_limit(
                        engine,
                        OpenPitStringView::from_utf8("RateLimitPolicy"),
                        &broker,
                        1,
                        axis == 0,
                        asset.as_ptr(),
                        asset.len(),
                        axis == 1,
                        account.as_ptr(),
                        account.len(),
                        axis == 2,
                        account_asset.as_ptr(),
                        account_asset.len(),
                        axis == 3,
                        &mut out_error,
                    )
                },
                "adding one axis must succeed"
            );
            assert!(out_error.is_null());
            run_start_pre_trade_passes(engine);
            run_start_pre_trade_rejected(engine);
            assert!(
                unsafe {
                    openpit_engine_configure_rate_limit(
                        engine,
                        OpenPitStringView::from_utf8("RateLimitPolicy"),
                        std::ptr::null(),
                        0,
                        axis == 0,
                        std::ptr::null(),
                        0,
                        axis == 1,
                        std::ptr::null(),
                        0,
                        axis == 2,
                        std::ptr::null(),
                        0,
                        axis == 3,
                        &mut out_error,
                    )
                },
                "clearing the last axis must succeed"
            );
            assert!(out_error.is_null());
            run_start_pre_trade_passes(engine);
            run_start_pre_trade_passes(engine);
        }
        for _ in 0..2 {
            let mut out_error = std::ptr::null_mut();
            assert!(
                unsafe {
                    openpit_engine_configure_rate_limit(
                        engine,
                        OpenPitStringView::from_utf8("RateLimitPolicy"),
                        &broker,
                        1,
                        true,
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
                },
                "re-activation must succeed"
            );
            assert!(out_error.is_null());
            run_start_pre_trade_passes(engine);
            assert!(
                unsafe {
                    openpit_engine_configure_rate_limit(
                        engine,
                        OpenPitStringView::from_utf8("RateLimitPolicy"),
                        std::ptr::null(),
                        0,
                        false,
                        std::ptr::null(),
                        0,
                        true,
                        std::ptr::null(),
                        0,
                        false,
                        std::ptr::null(),
                        0,
                        false,
                        &mut out_error,
                    )
                },
                "asset clear must succeed"
            );
            assert!(out_error.is_null());
            run_start_pre_trade_rejected(engine);
            assert!(
                unsafe {
                    openpit_engine_configure_rate_limit(
                        engine,
                        OpenPitStringView::from_utf8("RateLimitPolicy"),
                        std::ptr::null(),
                        0,
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
                },
                "last-to-empty must succeed"
            );
            assert!(out_error.is_null());
            run_start_pre_trade_passes(engine);
        }
        crate::engine::openpit_destroy_engine(engine);
    }

    #[test]
    fn add_builtin_rate_limit_policy_local_sync_mode() {
        let broker = OpenPitPretradePoliciesRateLimitBrokerBarrier {
            mode: crate::reject::OPENPIT_PRETRADE_RULE_MODE_REJECT,
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
                1,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
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
            mode: crate::reject::OPENPIT_PRETRADE_RULE_MODE_REJECT,
            max_orders: 1000,
            window_nanoseconds: 60_000_000_000,
        };
        let asset = [OpenPitPretradePoliciesRateLimitAssetBarrier {
            settlement_asset: usd,
            mode: crate::reject::OPENPIT_PRETRADE_RULE_MODE_REJECT,
            max_orders: 500,
            window_nanoseconds: 60_000_000_000,
        }];
        let account = [OpenPitPretradePoliciesRateLimitAccountBarrier {
            account_id: 42,
            mode: crate::reject::OPENPIT_PRETRADE_RULE_MODE_REJECT,
            max_orders: 200,
            window_nanoseconds: 60_000_000_000,
        }];
        let account_asset = [OpenPitPretradePoliciesRateLimitAccountAssetBarrier {
            account_id: 42,
            settlement_asset: usd,
            mode: crate::reject::OPENPIT_PRETRADE_RULE_MODE_REJECT,
            max_orders: 100,
            window_nanoseconds: 60_000_000_000,
        }];
        let engine = build_engine_with_builtin_start_policy(|builder| unsafe {
            openpit_engine_builder_add_builtin_rate_limit_policy(
                builder,
                0,
                &broker,
                1,
                asset.as_ptr(),
                asset.len(),
                account.as_ptr(),
                account.len(),
                account_asset.as_ptr(),
                account_asset.len(),
                std::ptr::null(),
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
        crate::reject::openpit_destroy_pretrade_reject_list(out_rejects);
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
            mode: crate::reject::OPENPIT_PRETRADE_RULE_MODE_REJECT,
            max_orders: 100,
            window_nanoseconds: 1_000_000_000,
        };
        let engine = build_engine_with_builtin_start_policy(|builder| unsafe {
            openpit_engine_builder_add_builtin_rate_limit_policy(
                builder,
                0,
                &broker,
                1,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
                std::ptr::null_mut(),
            )
        });

        // Retune the broker barrier down to a single order per minute before
        // any order is submitted, so the live counter starts clean.
        let tighter = OpenPitPretradePoliciesRateLimitBrokerBarrier {
            mode: crate::reject::OPENPIT_PRETRADE_RULE_MODE_REJECT,
            max_orders: 1,
            window_nanoseconds: 60_000_000_000,
        };
        let mut out_error = std::ptr::null_mut();
        let ok = unsafe {
            openpit_engine_configure_rate_limit(
                engine,
                OpenPitStringView::from_utf8("RateLimitPolicy"),
                &tighter,
                1,
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
            mode: crate::reject::OPENPIT_PRETRADE_RULE_MODE_REJECT,
            max_orders: 100,
            window_nanoseconds: 1_000_000_000,
        };
        let engine = build_engine_with_builtin_start_policy(|builder| unsafe {
            openpit_engine_builder_add_builtin_rate_limit_policy(
                builder,
                0,
                &broker,
                1,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
                std::ptr::null_mut(),
            )
        });

        let tighter = OpenPitPretradePoliciesRateLimitBrokerBarrier {
            mode: crate::reject::OPENPIT_PRETRADE_RULE_MODE_REJECT,
            max_orders: 1,
            window_nanoseconds: 60_000_000_000,
        };
        let mut out_error = std::ptr::null_mut();
        let ok = unsafe {
            openpit_engine_configure_rate_limit(
                engine,
                OpenPitStringView::from_utf8("NoSuchPolicy"),
                &tighter,
                1,
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
            mode: crate::reject::OPENPIT_PRETRADE_RULE_MODE_REJECT,
            max_orders: 100,
            window_nanoseconds: 1_000_000_000,
        };
        // Build with only a broker barrier; no asset axis at build time.
        let engine = build_engine_with_builtin_start_policy(|builder| unsafe {
            openpit_engine_builder_add_builtin_rate_limit_policy(
                builder,
                0,
                &broker,
                1,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
                std::ptr::null_mut(),
            )
        });

        // Add a USD asset barrier (max 1 order per 60 s) at runtime.
        let usd = OpenPitStringView::from_utf8("USD");
        let asset = [OpenPitPretradePoliciesRateLimitAssetBarrier {
            settlement_asset: usd,
            mode: crate::reject::OPENPIT_PRETRADE_RULE_MODE_REJECT,
            max_orders: 1,
            window_nanoseconds: 60_000_000_000,
        }];
        let mut out_error = std::ptr::null_mut();
        let ok = unsafe {
            openpit_engine_configure_rate_limit(
                engine,
                OpenPitStringView::from_utf8("RateLimitPolicy"),
                std::ptr::null(),
                0,
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
            mode: crate::reject::OPENPIT_PRETRADE_RULE_MODE_REJECT,
            max_orders: 100,
            window_nanoseconds: 1_000_000_000,
        };
        let engine = build_engine_with_builtin_start_policy(|builder| unsafe {
            openpit_engine_builder_add_builtin_rate_limit_policy(
                builder,
                0,
                &broker,
                1,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
                std::ptr::null_mut(),
            )
        });

        // Zero window is invalid.
        let zero_window = OpenPitPretradePoliciesRateLimitBrokerBarrier {
            mode: crate::reject::OPENPIT_PRETRADE_RULE_MODE_REJECT,
            max_orders: 1,
            window_nanoseconds: 0,
        };
        let mut out_error = std::ptr::null_mut();
        let ok = unsafe {
            openpit_engine_configure_rate_limit(
                engine,
                OpenPitStringView::from_utf8("RateLimitPolicy"),
                &zero_window,
                1,
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
            mode: crate::reject::OPENPIT_PRETRADE_RULE_MODE_REJECT,
            max_orders: 1,
            window_nanoseconds: -1_000_000_000,
        };
        let mut out_error = std::ptr::null_mut();
        let ok = unsafe {
            openpit_engine_configure_rate_limit(
                engine,
                OpenPitStringView::from_utf8("RateLimitPolicy"),
                &negative_window,
                1,
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
            mode: crate::reject::OPENPIT_PRETRADE_RULE_MODE_REJECT,
            max_orders: 100,
            window_nanoseconds: 1_000_000_000,
        };
        let engine = build_engine_with_builtin_start_policy(|builder| unsafe {
            openpit_engine_builder_add_builtin_rate_limit_policy(
                builder,
                0,
                &broker,
                1,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
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
                    0,
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
