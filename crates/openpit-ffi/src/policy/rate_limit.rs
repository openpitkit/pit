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

use std::time::Duration;

use openpit::param::AccountId;
use openpit::pretrade::policies::{
    RateLimit, RateLimitAccountAssetBarrier, RateLimitAccountBarrier, RateLimitAssetBarrier,
    RateLimitBrokerBarrier, RateLimitPolicy,
};

use super::*;

/// Broker-wide rate-limit barrier for
/// `openpit_engine_builder_add_builtin_rate_limit_policy`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpenPitPretradePoliciesRateLimitBrokerBarrier {
    /// Maximum number of orders accepted within the window.
    pub max_orders: usize,
    /// Window duration in nanoseconds.
    pub window_nanoseconds: u64,
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
    pub window_nanoseconds: u64,
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
    pub window_nanoseconds: u64,
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
    pub window_nanoseconds: u64,
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
                window: Duration::from_nanos(b.window_nanoseconds),
            },
        })
    } else {
        None
    };

    let mut asset_barriers = Vec::with_capacity(asset_slice.len());
    for (index, entry) in asset_slice.iter().enumerate() {
        let settlement = match parse_settlement_asset_or_error(
            entry.settlement_asset,
            "asset",
            index,
            out_error,
        ) {
            Some(v) => v,
            None => return false,
        };
        asset_barriers.push(RateLimitAssetBarrier {
            limit: RateLimit {
                max_orders: entry.max_orders,
                window: Duration::from_nanos(entry.window_nanoseconds),
            },
            settlement_asset: settlement,
        });
    }

    let account_barriers: Vec<RateLimitAccountBarrier> = account_slice
        .iter()
        .map(|entry| RateLimitAccountBarrier {
            limit: RateLimit {
                max_orders: entry.max_orders,
                window: Duration::from_nanos(entry.window_nanoseconds),
            },
            account_id: AccountId::from_u64(entry.account_id),
        })
        .collect();

    let mut account_asset_barriers = Vec::with_capacity(account_asset_slice.len());
    for (index, entry) in account_asset_slice.iter().enumerate() {
        let settlement = match parse_settlement_asset_or_error(
            entry.settlement_asset,
            "account_asset",
            index,
            out_error,
        ) {
            Some(v) => v,
            None => return false,
        };
        account_asset_barriers.push(RateLimitAccountAssetBarrier {
            limit: RateLimit {
                max_orders: entry.max_orders,
                window: Duration::from_nanos(entry.window_nanoseconds),
            },
            account_id: AccountId::from_u64(entry.account_id),
            settlement_asset: settlement,
        });
    }

    let builder_ref = unsafe { &mut *builder };
    let storage = match policy_storage(builder_ref) {
        Some(storage) => storage,
        None => {
            write_error(out_error, "engine builder is no longer available");
            return false;
        }
    };
    let policy = match RateLimitPolicy::new(
        broker_opt,
        asset_barriers,
        account_barriers,
        account_asset_barriers,
        storage,
    ) {
        Ok(v) => v,
        Err(e) => {
            write_error_format!(out_error, "rate_limit_policy creation failed: {}", e);
            return false;
        }
    };
    let policy = policy.with_policy_group_id(openpit::PolicyGroupId::new(policy_group_id));
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
            OpenPitParamSide, OpenPitParamTradeAmount, OpenPitParamTradeAmountKind,
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
                        kind: OpenPitParamTradeAmountKind::Quantity,
                    },
                    account_id: OpenPitParamAccountIdOptional {
                        value: 7,
                        is_set: true,
                    },
                    side: OpenPitParamSide::Buy,
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
}
