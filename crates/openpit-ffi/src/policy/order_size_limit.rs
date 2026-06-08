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

use openpit::param::AccountId;
use openpit::pretrade::policies::{
    OrderSizeAccountAssetBarrier, OrderSizeAssetBarrier, OrderSizeBrokerBarrier, OrderSizeLimit,
    OrderSizeLimitPolicy,
};

use crate::param::{OpenPitParamQuantity, OpenPitParamVolume};

use super::*;

/// Shared order-size limits for
/// `openpit_engine_builder_add_builtin_order_size_limit_policy`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpenPitPretradePoliciesOrderSizeLimit {
    /// Maximum allowed quantity for one order.
    pub max_quantity: OpenPitParamQuantity,
    /// Maximum allowed notional for one order.
    pub max_notional: OpenPitParamVolume,
}

/// Broker-wide order-size barrier for
/// `openpit_engine_builder_add_builtin_order_size_limit_policy`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpenPitPretradePoliciesOrderSizeBrokerBarrier {
    /// Size limits for this broker barrier.
    pub limit: OpenPitPretradePoliciesOrderSizeLimit,
}

/// Per-settlement-asset order-size barrier for
/// `openpit_engine_builder_add_builtin_order_size_limit_policy`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpenPitPretradePoliciesOrderSizeAssetBarrier {
    /// Size limits for this asset barrier.
    pub limit: OpenPitPretradePoliciesOrderSizeLimit,
    /// Settlement asset this barrier applies to.
    pub settlement_asset: OpenPitStringView,
}

/// Per-(account, settlement-asset) order-size barrier for
/// `openpit_engine_builder_add_builtin_order_size_limit_policy`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpenPitPretradePoliciesOrderSizeAccountAssetBarrier {
    /// Size limits for this account+asset barrier.
    pub limit: OpenPitPretradePoliciesOrderSizeLimit,
    /// Account this barrier applies to.
    pub account_id: OpenPitParamAccountId,
    /// Settlement asset this barrier applies to.
    pub settlement_asset: OpenPitStringView,
}

#[no_mangle]
/// Adds the built-in order-size limit policy to the engine builder.
///
/// Contract:
/// - `builder` must be a valid engine builder pointer.
/// - `policy_group_id` assigns the policy to a policy group (pass `0` for default).
/// - At least one barrier axis must be configured: `broker` non-null,
///   `asset_len > 0`, or `account_asset_len > 0`.
/// - When a length is greater than zero the corresponding pointer must point
///   to that many readable entries.
/// - Each `settlement_asset` string view inside an array entry must be valid
///   for the duration of the call.
/// - `max_quantity` and `max_notional` inside each limit must be valid.
///
/// Success:
/// - returns `true`; the builder retains the policy.
///
/// Error:
/// - returns `false` when the builder is null or already consumed, when no
///   barrier axis is configured, or when argument parsing fails;
/// - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
///   error handle that MUST be released with `openpit_destroy_shared_string`.
pub unsafe extern "C" fn openpit_engine_builder_add_builtin_order_size_limit_policy(
    builder: *mut crate::engine::OpenPitEngineBuilder,
    policy_group_id: u16,
    broker: *const OpenPitPretradePoliciesOrderSizeBrokerBarrier,
    asset: *const OpenPitPretradePoliciesOrderSizeAssetBarrier,
    asset_len: usize,
    account_asset: *const OpenPitPretradePoliciesOrderSizeAccountAssetBarrier,
    account_asset_len: usize,
    out_error: OpenPitOutError,
) -> bool {
    if builder.is_null() {
        write_error(out_error, "engine builder is null");
        return false;
    }
    let asset_slice = match unsafe {
        try_slice_arg(asset, asset_len, "order_size_limit_policy asset", out_error)
    } {
        Some(v) => v,
        None => return false,
    };
    let account_asset_slice = match unsafe {
        try_slice_arg(
            account_asset,
            account_asset_len,
            "order_size_limit_policy account_asset",
            out_error,
        )
    } {
        Some(v) => v,
        None => return false,
    };

    let broker_opt = if !broker.is_null() {
        let b = unsafe { &*broker };
        let max_quantity = match b.limit.max_quantity.to_param() {
            Ok(v) => v,
            Err(e) => {
                write_error_format!(out_error, "broker max_quantity is invalid: {}", e);
                return false;
            }
        };
        let max_notional = match b.limit.max_notional.to_param() {
            Ok(v) => v,
            Err(e) => {
                write_error_format!(out_error, "broker max_notional is invalid: {}", e);
                return false;
            }
        };
        Some(OrderSizeBrokerBarrier {
            limit: OrderSizeLimit {
                max_quantity,
                max_notional,
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
        let max_quantity = match entry.limit.max_quantity.to_param() {
            Ok(v) => v,
            Err(e) => {
                write_error_format!(out_error, "asset[{index}] max_quantity is invalid: {}", e);
                return false;
            }
        };
        let max_notional = match entry.limit.max_notional.to_param() {
            Ok(v) => v,
            Err(e) => {
                write_error_format!(out_error, "asset[{index}] max_notional is invalid: {}", e);
                return false;
            }
        };
        asset_barriers.push(OrderSizeAssetBarrier {
            limit: OrderSizeLimit {
                max_quantity,
                max_notional,
            },
            settlement_asset: settlement,
        });
    }

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
        let max_quantity = match entry.limit.max_quantity.to_param() {
            Ok(v) => v,
            Err(e) => {
                write_error_format!(
                    out_error,
                    "account_asset[{index}] max_quantity is invalid: {}",
                    e
                );
                return false;
            }
        };
        let max_notional = match entry.limit.max_notional.to_param() {
            Ok(v) => v,
            Err(e) => {
                write_error_format!(
                    out_error,
                    "account_asset[{index}] max_notional is invalid: {}",
                    e
                );
                return false;
            }
        };
        account_asset_barriers.push(OrderSizeAccountAssetBarrier {
            limit: OrderSizeLimit {
                max_quantity,
                max_notional,
            },
            account_id: AccountId::from_u64(entry.account_id),
            settlement_asset: settlement,
        });
    }

    let policy = match OrderSizeLimitPolicy::new(broker_opt, asset_barriers, account_asset_barriers)
    {
        Ok(v) => v,
        Err(e) => {
            write_error_format!(out_error, "order_size_limit_policy creation failed: {}", e);
            return false;
        }
    };
    let policy = policy.with_policy_group_id(openpit::PolicyGroupId::new(policy_group_id));
    match crate::engine::add_pre_trade_policy_to_builder(unsafe { &mut *builder }, policy) {
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
    use crate::param::{OpenPitParamDecimal, OpenPitParamQuantity, OpenPitParamVolume};

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

    fn volume_param(mantissa: i128, scale: i32) -> OpenPitParamVolume {
        OpenPitParamVolume(OpenPitParamDecimal {
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
    fn add_builtin_order_size_limit_policy_happy_path() {
        let usd = OpenPitStringView::from_utf8("USD");
        let asset = [OpenPitPretradePoliciesOrderSizeAssetBarrier {
            limit: OpenPitPretradePoliciesOrderSizeLimit {
                max_quantity: quantity_param(100, 0),
                max_notional: volume_param(10000, 0),
            },
            settlement_asset: usd,
        }];
        let engine = build_engine_with_builtin_start_policy(|builder| unsafe {
            openpit_engine_builder_add_builtin_order_size_limit_policy(
                builder,
                0,
                std::ptr::null(),
                asset.as_ptr(),
                asset.len(),
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
            )
        });
        run_start_pre_trade_passes(engine);
        crate::engine::openpit_destroy_engine(engine);
    }

    #[test]
    fn add_builtin_order_size_limit_policy_empty_config_reports_error() {
        let builder = crate::engine::openpit_create_engine_builder(
            crate::engine::OpenPitSyncPolicy::Full as u8,
            std::ptr::null_mut(),
        );
        let mut out_error = std::ptr::null_mut();
        let ok = unsafe {
            openpit_engine_builder_add_builtin_order_size_limit_policy(
                builder,
                0,
                std::ptr::null(),
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
            message.contains("order_size_limit_policy creation failed")
                && message.contains("must be configured"),
            "expected SDK no-barrier error wrapped by FFI, got: {message}"
        );
        crate::engine::openpit_destroy_engine_builder(builder);
    }
}
