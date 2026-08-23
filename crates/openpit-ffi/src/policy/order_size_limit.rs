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

use openpit::param::AccountId;
use openpit::pretrade::policies::{
    OrderSizeAccountAssetBarrier, OrderSizeAssetBarrier, OrderSizeBrokerBarrier, OrderSizeLimit,
    OrderSizeLimitPolicy, OrderSizeLimitPolicyError, OrderSizeLimitSettings,
};

use crate::engine::{write_configure_error, OpenPitConfigureError};
use crate::param::{OpenPitParamQuantityOptional, OpenPitParamVolumeOptional};

use super::*;

/// Converts a C order-size limit into the core type for a configure function,
/// mapping any parse failure to an [`OpenPitConfigureError`].
fn parse_configure_limit(
    limit: OpenPitPretradePoliciesOrderSizeLimit,
    label: &str,
    index: usize,
) -> Result<OrderSizeLimit, OpenPitConfigureError> {
    let max_quantity = if limit.max_quantity.is_set {
        Some(limit.max_quantity.value.to_param().map_err(|e| {
            OpenPitConfigureError::validation(format!(
                "{label}[{index}] max_quantity is invalid: {e}"
            ))
        })?)
    } else {
        None
    };
    let max_notional = if limit.max_notional.is_set {
        Some(limit.max_notional.value.to_param().map_err(|e| {
            OpenPitConfigureError::validation(format!(
                "{label}[{index}] max_notional is invalid: {e}"
            ))
        })?)
    } else {
        None
    };
    Ok(OrderSizeLimit {
        max_quantity,
        max_notional,
    })
}

/// Shared optional order-size limits for
/// `openpit_engine_builder_add_builtin_order_size_limit_policy`.
///
/// Each cap is present when its wrapper's `is_set` field is `true`. When
/// `is_set` is `false`, the wrapper's `value` field is ignored. An unset cap
/// does not constrain that metric, and lookup continues down that metric's
/// barrier chain. A cap rejects an order whose value on that metric is above it;
/// a cap of zero rejects positive metric values and admits a value of exactly
/// zero. At least one cap must be present in every limit.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpenPitPretradePoliciesOrderSizeLimit {
    /// Optional maximum allowed quantity for one order.
    pub max_quantity: OpenPitParamQuantityOptional,
    /// Optional maximum allowed notional for one order.
    pub max_notional: OpenPitParamVolumeOptional,
}

/// Broker-wide order-size barrier for
/// `openpit_engine_builder_add_builtin_order_size_limit_policy`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpenPitPretradePoliciesOrderSizeBrokerBarrier {
    /// Size limits for this broker barrier.
    pub limit: OpenPitPretradePoliciesOrderSizeLimit,
}

/// Per-asset order-size barrier for
/// `openpit_engine_builder_add_builtin_order_size_limit_policy`.
///
/// An asset key may appear at most once within one asset-barrier array.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpenPitPretradePoliciesOrderSizeAssetBarrier {
    /// Size limits for this asset barrier.
    pub limit: OpenPitPretradePoliciesOrderSizeLimit,
    /// Asset key: `max_quantity` matches the instrument's underlying asset,
    /// while `max_notional` matches its settlement asset.
    pub asset: OpenPitStringView,
}

/// Per-(account, asset) order-size barrier for
/// `openpit_engine_builder_add_builtin_order_size_limit_policy`.
///
/// An `(account_id, asset)` key may appear at most once within one
/// account+asset-barrier array.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpenPitPretradePoliciesOrderSizeAccountAssetBarrier {
    /// Size limits for this account+asset barrier.
    pub limit: OpenPitPretradePoliciesOrderSizeLimit,
    /// Account this barrier applies to.
    pub account_id: OpenPitParamAccountId,
    /// Asset key: `max_quantity` matches the instrument's underlying asset,
    /// while `max_notional` matches its settlement asset.
    pub asset: OpenPitStringView,
}

#[no_mangle]
/// Adds the built-in order-size limit policy to the engine builder.
///
/// Contract:
/// - A null or already-consumed `builder` is a handled error.
/// - `policy_group_id` assigns the policy to a policy group (pass `0` for the
///   default group).
/// - At least one barrier axis must be configured: `broker` non-null,
///   `asset_len > 0`, or `account_asset_len > 0`.
/// - A pointer may be null when its array length is zero.
/// - Each non-null `asset` string view must contain UTF-8 and a valid asset for
///   the call to succeed.
/// - Each optional cap with `is_set == true` must contain a valid value. When
///   `is_set == false`, its `value` field is ignored. Every limit must set at
///   least one cap.
///
/// # Safety
///
/// - A non-null `builder` must be properly aligned and point to a live,
///   initialized engine builder for the duration of the call.
/// - A non-null `broker` must be properly aligned and point to one initialized,
///   readable barrier for the duration of the call.
/// - When an array length is greater than zero, its pointer must be non-null,
///   properly aligned, and point to that many initialized, readable entries for
///   the duration of the call.
/// - Every optional wrapper's `is_set` field in each supplied barrier must hold
///   a valid `bool` value.
/// - Each non-null `asset` string-view pointer must point to `len` initialized,
///   readable bytes for the duration of the call.
/// - `out_error` may be null; otherwise it must be properly aligned and point
///   to writable storage for an `OpenPitSharedString` handle.
///
/// Success:
/// - returns `true`; the builder retains the policy.
///
/// Error:
/// - returns `false` when the builder is null or already consumed, when no
///   barrier axis is configured, when any limit has no cap, when an asset or
///   `(account_id, asset)` key is duplicated within its axis, or when argument
///   parsing fails;
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
        let max_quantity = match parse_optional_quantity_without_index_or_error(
            b.limit.max_quantity,
            "broker",
            "max_quantity",
            out_error,
        ) {
            Ok(v) => v,
            Err(()) => return false,
        };
        let max_notional = match parse_optional_volume_without_index_or_error(
            b.limit.max_notional,
            "broker",
            "max_notional",
            out_error,
        ) {
            Ok(v) => v,
            Err(()) => return false,
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
        let asset = match parse_asset_or_error(entry.asset, "asset", index, "asset", out_error) {
            Some(v) => v,
            None => return false,
        };
        let max_quantity = match parse_optional_quantity_or_error(
            entry.limit.max_quantity,
            "asset",
            index,
            "max_quantity",
            out_error,
        ) {
            Ok(v) => v,
            Err(()) => return false,
        };
        let max_notional = match parse_optional_volume_or_error(
            entry.limit.max_notional,
            "asset",
            index,
            "max_notional",
            out_error,
        ) {
            Ok(v) => v,
            Err(()) => return false,
        };
        asset_barriers.push(OrderSizeAssetBarrier {
            limit: OrderSizeLimit {
                max_quantity,
                max_notional,
            },
            asset,
        });
    }

    let mut account_asset_barriers = Vec::with_capacity(account_asset_slice.len());
    for (index, entry) in account_asset_slice.iter().enumerate() {
        let asset =
            match parse_asset_or_error(entry.asset, "account_asset", index, "asset", out_error) {
                Some(v) => v,
                None => return false,
            };
        let max_quantity = match parse_optional_quantity_or_error(
            entry.limit.max_quantity,
            "account_asset",
            index,
            "max_quantity",
            out_error,
        ) {
            Ok(v) => v,
            Err(()) => return false,
        };
        let max_notional = match parse_optional_volume_or_error(
            entry.limit.max_notional,
            "account_asset",
            index,
            "max_notional",
            out_error,
        ) {
            Ok(v) => v,
            Err(()) => return false,
        };
        account_asset_barriers.push(OrderSizeAccountAssetBarrier {
            limit: OrderSizeLimit {
                max_quantity,
                max_notional,
            },
            account_id: AccountId::from_u64(entry.account_id),
            asset,
        });
    }

    let settings =
        match OrderSizeLimitSettings::new(broker_opt, asset_barriers, account_asset_barriers) {
            Ok(v) => v,
            Err(e) => {
                write_error_format!(out_error, "order_size_limit_policy creation failed: {}", e);
                return false;
            }
        };
    // The policy is generic over its locking factory; instantiate it with the
    // interop factory used by every built-in in this crate.
    let policy = OrderSizeLimitPolicy::<FfiStorageFactory>::new(settings)
        .with_policy_group_id(openpit::PolicyGroupId::new(policy_group_id));
    match crate::engine::add_pre_trade_policy_to_builder(unsafe { &mut *builder }, policy) {
        Ok(()) => true,
        Err(err) => {
            write_error(out_error, &err);
            false
        }
    }
}

#[no_mangle]
/// Retunes the built-in order-size limit policy registered under `name`.
///
/// This is a partial update (PATCH) at the axis level: each axis is replaced
/// wholesale only when its `has_*` flag is `true`, mirroring the
/// replace-shaped settings setters.
///
/// Contract:
/// - A null `engine` is a handled error.
/// - `name` selects the policy and is interpreted as UTF-8. A built-in policy
///   added via `openpit_engine_builder_add_builtin_order_size_limit_policy`
///   registers under its fixed name `"OrderSizeLimitPolicy"`, so pass that
///   string here.
/// - When `has_broker` is `true`, the broker barrier is set to `*broker` when
///   `broker` is non-null, or cleared when `broker` is null.
/// - When `has_asset` is `true`, the per-asset axis is replaced by the
///   `asset_len` entries at `asset`.
/// - When `has_account_asset` is `true`, the per-(account, asset) axis is
///   replaced by the `account_asset_len` entries at `account_asset`.
/// - A `has_*` flag set to `false` leaves that axis untouched and ignores the
///   corresponding pointer and length. The policy's "at least one barrier"
///   rule still applies to the resulting configuration.
/// - Each non-null `asset` view must contain UTF-8 and a valid asset for the
///   call to succeed.
/// - Each optional cap with `is_set == true` must contain a valid value. When
///   `is_set == false`, its `value` field is ignored. Every supplied limit must
///   set at least one cap.
///
/// # Safety
///
/// - A non-null `engine` must be properly aligned and point to a live,
///   initialized engine for the duration of the call.
/// - Every `has_*` argument must hold a valid `bool` value.
/// - When `name.ptr` is non-null, it must point to `name.len` initialized,
///   readable bytes for the duration of the call.
/// - When `has_broker` is `true`, a non-null `broker` must be properly aligned
///   and point to one initialized, readable barrier for the duration of the
///   call.
/// - When `has_asset` is `true` and `asset_len` is greater than zero, `asset`
///   must be non-null, properly aligned, and point to that many initialized,
///   readable entries for the duration of the call.
/// - When `has_account_asset` is `true` and `account_asset_len` is greater than
///   zero, `account_asset` must be non-null, properly aligned, and point to that
///   many initialized, readable entries for the duration of the call.
/// - Every optional wrapper's `is_set` field in each supplied barrier must hold
///   a valid `bool` value.
/// - Each non-null `asset` string-view pointer must point to `len` initialized,
///   readable bytes for the duration of the call.
/// - `out_error` may be null; otherwise it must be properly aligned and point
///   to writable storage for an `OpenPitConfigureError` pointer.
///
/// Success:
/// - returns `true`; the new limits apply from the next order onward.
///
/// Error:
/// - returns `false`; if `out_error` is non-null, writes a caller-owned
///   `OpenPitConfigureError` (release with `openpit_destroy_configure_error`).
/// - A supplied limit with neither cap set, a duplicate asset key, or a
///   duplicate `(account_id, asset)` key is rejected.
/// - a null `engine` returns `false` and, when `out_error` is non-null, writes
///   a caller-owned `OpenPitConfigureError` (`Validation`) that must be
///   released with `openpit_destroy_configure_error`.
pub unsafe extern "C" fn openpit_engine_configure_order_size_limit(
    engine: *mut crate::engine::OpenPitEngine,
    name: OpenPitStringView,
    broker: *const OpenPitPretradePoliciesOrderSizeBrokerBarrier,
    has_broker: bool,
    asset: *const OpenPitPretradePoliciesOrderSizeAssetBarrier,
    asset_len: usize,
    has_asset: bool,
    account_asset: *const OpenPitPretradePoliciesOrderSizeAccountAssetBarrier,
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

    let broker_barrier: Option<OrderSizeBrokerBarrier> = if has_broker && !broker.is_null() {
        let limit = match parse_configure_limit(unsafe { &*broker }.limit, "broker", 0) {
            Ok(v) => v,
            Err(e) => {
                write_configure_error(out_error, e);
                return false;
            }
        };
        Some(OrderSizeBrokerBarrier { limit })
    } else {
        None
    };

    let asset_barriers: Vec<OrderSizeAssetBarrier> = if has_asset {
        let slice = match unsafe {
            try_slice_arg(
                asset,
                asset_len,
                "order_size_limit asset",
                std::ptr::null_mut(),
            )
        } {
            Some(v) => v,
            None => {
                write_configure_error(
                    out_error,
                    OpenPitConfigureError::validation("order_size_limit asset is null".to_owned()),
                );
                return false;
            }
        };
        let mut out = Vec::with_capacity(slice.len());
        for (index, entry) in slice.iter().enumerate() {
            let asset = match parse_configure_asset(entry.asset, "asset", index, "asset") {
                Ok(v) => v,
                Err(e) => {
                    write_configure_error(out_error, e);
                    return false;
                }
            };
            let limit = match parse_configure_limit(entry.limit, "asset", index) {
                Ok(v) => v,
                Err(e) => {
                    write_configure_error(out_error, e);
                    return false;
                }
            };
            out.push(OrderSizeAssetBarrier { limit, asset });
        }
        out
    } else {
        Vec::new()
    };

    let account_asset_barriers: Vec<OrderSizeAccountAssetBarrier> = if has_account_asset {
        let slice = match unsafe {
            try_slice_arg(
                account_asset,
                account_asset_len,
                "order_size_limit account_asset",
                std::ptr::null_mut(),
            )
        } {
            Some(v) => v,
            None => {
                write_configure_error(
                    out_error,
                    OpenPitConfigureError::validation(
                        "order_size_limit account_asset is null".to_owned(),
                    ),
                );
                return false;
            }
        };
        let mut out = Vec::with_capacity(slice.len());
        for (index, entry) in slice.iter().enumerate() {
            let asset = match parse_configure_asset(entry.asset, "account_asset", index, "asset") {
                Ok(v) => v,
                Err(e) => {
                    write_configure_error(out_error, e);
                    return false;
                }
            };
            let limit = match parse_configure_limit(entry.limit, "account_asset", index) {
                Ok(v) => v,
                Err(e) => {
                    write_configure_error(out_error, e);
                    return false;
                }
            };
            out.push(OrderSizeAccountAssetBarrier {
                limit,
                account_id: AccountId::from_u64(entry.account_id),
                asset,
            });
        }
        out
    } else {
        Vec::new()
    };

    let result = unsafe { &*engine }.configurator().order_size_limit(
        &name,
        |settings| -> Result<(), OrderSizeLimitPolicyError> {
            if has_broker {
                settings.set_broker(broker_barrier.clone())?;
            }
            if has_asset {
                settings.set_asset_barriers(asset_barriers.iter().cloned())?;
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
    use crate::param::{
        OpenPitParamDecimal, OpenPitParamQuantity, OpenPitParamQuantityOptional,
        OpenPitParamVolume, OpenPitParamVolumeOptional,
    };

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

    fn quantity_cap(mantissa: i128, scale: i32) -> OpenPitParamQuantityOptional {
        OpenPitParamQuantityOptional {
            value: quantity_param(mantissa, scale),
            is_set: true,
        }
    }

    fn volume_cap(mantissa: i128, scale: i32) -> OpenPitParamVolumeOptional {
        OpenPitParamVolumeOptional {
            value: volume_param(mantissa, scale),
            is_set: true,
        }
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

    fn pit_order_with_quantity(mantissa: i128) -> OpenPitOrder {
        let mut order = valid_pit_order();
        order.operation.value.trade_amount.value = quantity_param(mantissa, 0).0;
        order
    }

    fn pit_order_with_price(mantissa: i128) -> OpenPitOrder {
        let mut order = valid_pit_order();
        order.operation.value.price.value.0 = OpenPitParamDecimal {
            mantissa_lo: mantissa as i64,
            mantissa_hi: (mantissa >> 64) as i64,
            scale: 0,
        };
        order
    }

    fn run_start_pre_trade(
        engine: *mut crate::engine::OpenPitEngine,
        order: OpenPitOrder,
    ) -> crate::engine::OpenPitPretradeStatus {
        let mut request = std::ptr::null_mut();
        let mut out_rejects = std::ptr::null_mut();
        let status = crate::engine::openpit_engine_start_pre_trade(
            engine,
            &order,
            &mut request,
            &mut out_rejects,
            std::ptr::null_mut(),
        );
        crate::engine::openpit_destroy_pretrade_pre_trade_request(request);
        crate::reject::openpit_destroy_pretrade_reject_list(out_rejects);
        status
    }

    #[test]
    fn add_builtin_order_size_limit_policy_accepts_quantity_only() {
        let broker = OpenPitPretradePoliciesOrderSizeBrokerBarrier {
            limit: OpenPitPretradePoliciesOrderSizeLimit {
                max_quantity: quantity_cap(100, 0),
                max_notional: OpenPitParamVolumeOptional::default(),
            },
        };
        let engine = build_engine_with_builtin_start_policy(|builder| unsafe {
            openpit_engine_builder_add_builtin_order_size_limit_policy(
                builder,
                0,
                &broker,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
            )
        });
        assert_eq!(
            run_start_pre_trade(engine, pit_order_with_quantity(101)),
            crate::engine::OpenPitPretradeStatus::Rejected
        );
        assert_eq!(
            run_start_pre_trade(engine, pit_order_with_quantity(100)),
            crate::engine::OpenPitPretradeStatus::Passed
        );
        assert_eq!(
            run_start_pre_trade(engine, valid_pit_order()),
            crate::engine::OpenPitPretradeStatus::Passed,
            "an unset max_notional must not act like an explicit zero cap"
        );
        crate::engine::openpit_destroy_engine(engine);

        let zero_broker = OpenPitPretradePoliciesOrderSizeBrokerBarrier {
            limit: OpenPitPretradePoliciesOrderSizeLimit {
                max_quantity: quantity_cap(0, 0),
                max_notional: OpenPitParamVolumeOptional::default(),
            },
        };
        let zero_engine = build_engine_with_builtin_start_policy(|builder| unsafe {
            openpit_engine_builder_add_builtin_order_size_limit_policy(
                builder,
                0,
                &zero_broker,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
            )
        });
        assert_eq!(
            run_start_pre_trade(zero_engine, valid_pit_order()),
            crate::engine::OpenPitPretradeStatus::Rejected,
            "an explicit zero max_quantity must reject a positive quantity"
        );
        crate::engine::openpit_destroy_engine(zero_engine);
    }

    #[test]
    fn add_builtin_order_size_limit_policy_accepts_notional_only() {
        let broker = OpenPitPretradePoliciesOrderSizeBrokerBarrier {
            limit: OpenPitPretradePoliciesOrderSizeLimit {
                max_quantity: OpenPitParamQuantityOptional::default(),
                max_notional: volume_cap(100, 0),
            },
        };
        let engine = build_engine_with_builtin_start_policy(|builder| unsafe {
            openpit_engine_builder_add_builtin_order_size_limit_policy(
                builder,
                0,
                &broker,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
            )
        });
        assert_eq!(
            run_start_pre_trade(engine, pit_order_with_price(101)),
            crate::engine::OpenPitPretradeStatus::Rejected
        );
        assert_eq!(
            run_start_pre_trade(engine, pit_order_with_price(100)),
            crate::engine::OpenPitPretradeStatus::Passed
        );
        assert_eq!(
            run_start_pre_trade(engine, pit_order_with_price(1)),
            crate::engine::OpenPitPretradeStatus::Passed,
            "an unset max_quantity must not act like an explicit zero cap"
        );
        crate::engine::openpit_destroy_engine(engine);

        let zero_broker = OpenPitPretradePoliciesOrderSizeBrokerBarrier {
            limit: OpenPitPretradePoliciesOrderSizeLimit {
                max_quantity: OpenPitParamQuantityOptional::default(),
                max_notional: volume_cap(0, 0),
            },
        };
        let zero_engine = build_engine_with_builtin_start_policy(|builder| unsafe {
            openpit_engine_builder_add_builtin_order_size_limit_policy(
                builder,
                0,
                &zero_broker,
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
            )
        });
        assert_eq!(
            run_start_pre_trade(zero_engine, valid_pit_order()),
            crate::engine::OpenPitPretradeStatus::Rejected,
            "an explicit zero max_notional must reject a positive notional"
        );
        crate::engine::openpit_destroy_engine(zero_engine);
    }

    #[test]
    fn add_builtin_order_size_limit_policy_rejects_limit_without_caps() {
        let asset = [OpenPitPretradePoliciesOrderSizeAssetBarrier {
            limit: OpenPitPretradePoliciesOrderSizeLimit {
                max_quantity: OpenPitParamQuantityOptional::default(),
                max_notional: OpenPitParamVolumeOptional::default(),
            },
            asset: OpenPitStringView::from_utf8("USD"),
        }];
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
                asset.as_ptr(),
                asset.len(),
                std::ptr::null(),
                0,
                &mut out_error,
            )
        };
        assert!(!ok);
        assert!(!out_error.is_null());
        let error = cstr_to_string(out_error);
        assert!(
            error.contains("at least one of max_quantity or max_notional"),
            "unexpected error: {error}"
        );
        crate::engine::openpit_destroy_engine_builder(builder);
    }

    #[test]
    fn add_builtin_order_size_limit_policy_rejects_duplicate_asset_key() {
        let barrier = OpenPitPretradePoliciesOrderSizeAssetBarrier {
            limit: OpenPitPretradePoliciesOrderSizeLimit {
                max_quantity: quantity_cap(100, 0),
                max_notional: OpenPitParamVolumeOptional::default(),
            },
            asset: OpenPitStringView::from_utf8("USD"),
        };
        let asset = [barrier, barrier];
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
                asset.as_ptr(),
                asset.len(),
                std::ptr::null(),
                0,
                &mut out_error,
            )
        };
        assert!(!ok);
        assert!(!out_error.is_null());
        let error = cstr_to_string(out_error);
        assert!(
            error.contains("duplicate asset barrier for asset USD"),
            "unexpected error: {error}"
        );
        crate::engine::openpit_destroy_engine_builder(builder);
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
        assert!(!out_error.is_null());
        let error = cstr_to_string(out_error);
        assert!(
            error.contains("order_size_limit_policy creation failed"),
            "unexpected error: {error}"
        );
        assert!(
            error.contains("must be configured"),
            "unexpected error: {error}"
        );
        crate::engine::openpit_destroy_engine_builder(builder);
    }

    #[test]
    fn configure_order_size_limit_rejects_null_and_invalid_utf8_names() {
        let asset = [OpenPitPretradePoliciesOrderSizeAssetBarrier {
            limit: OpenPitPretradePoliciesOrderSizeLimit {
                max_quantity: quantity_cap(100, 0),
                max_notional: volume_cap(10000, 0),
            },
            asset: OpenPitStringView::from_utf8("USD"),
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
        let invalid_utf8 = [0xff];
        let invalid_name = OpenPitStringView {
            ptr: invalid_utf8.as_ptr(),
            len: invalid_utf8.len(),
        };

        for name in [OpenPitStringView::default(), invalid_name] {
            let mut out_error = std::ptr::null_mut();
            let ok = unsafe {
                openpit_engine_configure_order_size_limit(
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
}
