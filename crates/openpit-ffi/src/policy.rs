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

#![allow(
    clippy::arc_with_non_send_sync,
    clippy::missing_safety_doc,
    clippy::not_unsafe_ptr_arg_deref
)]

use std::ffi::c_void;
use std::rc::Rc;
use std::str;
use std::sync::Arc;
use std::time::Duration;

use openpit::param::{AccountId, Asset, Pnl};
use openpit::pretrade::policies::{
    OrderSizeAccountAssetBarrier, OrderSizeAssetBarrier, OrderSizeBrokerBarrier, OrderSizeLimit,
    OrderSizeLimitPolicy, OrderValidationPolicy, PnlBoundsAccountAssetBarrier,
    PnlBoundsBrokerBarrier, PnlBoundsKillSwitchPolicy, RateLimit, RateLimitAccountAssetBarrier,
    RateLimitAccountBarrier, RateLimitAssetBarrier, RateLimitBrokerBarrier, RateLimitPolicy,
};
use openpit::pretrade::{PreTradeContext, PreTradePolicy, Rejects};
use openpit::storage::StorageBuilder;
use openpit::{AccountAdjustmentContext, Mutation, Mutations};

use crate::account_adjustment::{export_account_adjustment, OpenPitAccountAdjustment};
use crate::execution_report::{export_execution_report, OpenPitExecutionReport};
use crate::order::{export_order, OpenPitOrder};
use crate::reject::{OpenPitPretradeAccountBlockList, OpenPitPretradeRejectList};
use crate::OpenPitStringView;
use crate::{AccountAdjustment, ExecutionReport, Order};

use crate::param::{
    OpenPitParamAccountId, OpenPitParamPnl, OpenPitParamPnlOptional, OpenPitParamQuantity,
    OpenPitParamVolume,
};

use crate::last_error::{write_error, OpenPitOutError};
use crate::write_error_format;

//--------------------------------------------------------------------------------------------------

/// Opaque pointer for a policy object.
///
/// What it is:
/// - A caller-owned reference to a policy instance.
///
/// Why it exists:
/// - It lets the caller create a policy once, pass it into the engine builder,
///   query its name, and destroy the caller-side pointer explicitly.
///
/// Lifetime contract:
/// - Each successful create function returns a new pointer owned by the caller.
/// - After the pointer is added to the engine builder, the engine keeps its own
///   reference to the same policy object.
/// - The caller must still destroy its own pointer when that local copy is no
///   longer needed. Destroying the caller pointer does not remove the policy from
///   the engine if the engine already retained it.
/// - Destroy the caller-owned pointer with
///   `openpit_destroy_pretrade_pre_trade_policy` exactly once.
pub struct PolicyHandle<P: ?Sized> {
    policy: Arc<P>,
}

impl<P: ?Sized + GeneralPreTradePolicy> PolicyHandle<P> {
    fn new(policy: Arc<P>) -> *mut Self {
        Box::into_raw(Box::new(Self { policy }))
    }

    fn get_name(&self) -> OpenPitStringView {
        OpenPitStringView::from_utf8(self.policy.name())
    }
}

//--------------------------------------------------------------------------------------------------

/// Unified trait object for all pre-trade hooks exposed through FFI.
///
/// It is backed by `PreTradePolicy<Order, ExecutionReport, AccountAdjustment>`
/// because the core engine now routes start-stage checks, main-stage checks,
/// execution-report updates, and account-adjustment validation through the same
/// policy list.
type UnifiedPreTradePolicy = dyn PreTradePolicy<Order, ExecutionReport, AccountAdjustment>;

/// Opaque pointer for a pre-trade policy.
///
/// Contract:
/// - Returned by custom policy create functions.
/// - May be passed to `openpit_engine_builder_add_pre_trade_policy`.
/// - Must be released by the caller with
///   `openpit_destroy_pretrade_pre_trade_policy` when no longer needed.
/// - A policy can implement any combination of start-stage, main-stage,
///   post-trade, and account-adjustment hooks.
pub type OpenPitPretradePreTradePolicy = PolicyHandle<UnifiedPreTradePolicy>;

//--------------------------------------------------------------------------------------------------

pub trait GeneralPreTradePolicy {
    fn name(&self) -> &str;
}

impl GeneralPreTradePolicy for UnifiedPreTradePolicy {
    fn name(&self) -> &str {
        self.name()
    }
}

//--------------------------------------------------------------------------------------------------

/// One broker barrier definition for
/// `openpit_engine_builder_add_builtin_pnl_bounds_killswitch_policy`.
///
/// What it describes:
/// - A settlement asset and its lower/upper P&L bounds applied as a broker
///   barrier across all accounts.
///
/// Contract:
/// - `settlement_asset` must point to a valid string for the duration of the
///   call.
/// - The array passed to the add function may contain multiple entries.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpenPitPretradePoliciesPnlBoundsBarrier {
    /// Settlement asset whose accumulated P&L is being monitored.
    pub settlement_asset: OpenPitStringView,
    /// Optional lower bound for accumulated P&L.
    pub lower_bound: OpenPitParamPnlOptional,
    /// Optional upper bound for accumulated P&L.
    pub upper_bound: OpenPitParamPnlOptional,
}

/// Per-(account, settlement-asset) P&L bounds barrier with an initial P&L seed.
///
/// What it describes:
/// - Refines P&L bounds for a specific account and settlement asset.
/// - `initial_pnl` is pre-loaded into storage at construction; accumulation
///   starts from this value.
/// - Both the broker barrier (if any) and this account+asset barrier are
///   evaluated on every check; the order passes only if neither is breached.
///
/// Passed to `openpit_engine_builder_add_builtin_pnl_bounds_killswitch_policy` in
/// the `account` array.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpenPitPretradePoliciesPnlBoundsAccountBarrier {
    /// Account this barrier applies to.
    pub account_id: OpenPitParamAccountId,
    /// Settlement asset whose accumulated P&L is being monitored.
    pub settlement_asset: OpenPitStringView,
    /// Optional lower bound for accumulated P&L for this account+asset pair.
    pub lower_bound: OpenPitParamPnlOptional,
    /// Optional upper bound for accumulated P&L for this account+asset pair.
    pub upper_bound: OpenPitParamPnlOptional,
    /// Starting accumulated P&L pre-loaded into storage at construction.
    pub initial_pnl: OpenPitParamPnl,
}

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

fn policy_storage(
    builder: &crate::engine::OpenPitEngineBuilder,
) -> Option<&StorageBuilder<openpit_interop::StorageLockingPolicyFactory>> {
    match builder.inner.as_ref()? {
        crate::engine::BuilderState::Synced(builder) => Some(builder.storage_builder()),
        crate::engine::BuilderState::Ready(builder) => Some(builder.storage_builder()),
    }
}

unsafe fn try_slice_arg<'a, T>(
    ptr: *const T,
    len: usize,
    label: &str,
    out_error: OpenPitOutError,
) -> Option<&'a [T]> {
    if len == 0 {
        return Some(&[]);
    }
    if ptr.is_null() {
        write_error_format!(out_error, "{} is null", label);
        return None;
    }
    Some(unsafe { std::slice::from_raw_parts(ptr, len) })
}

fn parse_settlement_asset_or_error(
    settlement: OpenPitStringView,
    label: &str,
    index: usize,
    out_error: OpenPitOutError,
) -> Option<Asset> {
    let settlement_raw = match unsafe { cstr_arg(settlement) } {
        Some(v) => v,
        None => {
            write_error_format!(
                out_error,
                "{}[{}] settlement_asset is not set",
                label,
                index
            );
            return None;
        }
    };
    match Asset::new(settlement_raw) {
        Ok(v) => Some(v),
        Err(e) => {
            write_error_format!(
                out_error,
                "{}[{}] settlement_asset is invalid: {}",
                label,
                index,
                e
            );
            None
        }
    }
}

fn parse_optional_pnl_or_error(
    bound: OpenPitParamPnlOptional,
    label: &str,
    index: usize,
    field: &str,
    out_error: OpenPitOutError,
) -> Result<Option<Pnl>, ()> {
    if !bound.is_set {
        return Ok(None);
    }
    match bound.value.to_param() {
        Ok(v) => Ok(Some(v)),
        Err(e) => {
            write_error_format!(
                out_error,
                "{}[{}] {} is invalid: {}",
                label,
                index,
                field,
                e
            );
            Err(())
        }
    }
}

#[no_mangle]
/// Adds the built-in order-validation policy to the engine builder.
///
/// Contract:
/// - `builder` must be a valid engine builder pointer.
///
/// Success:
/// - returns `true`; the builder retains the policy.
///
/// Error:
/// - returns `false` when the builder is null or already consumed;
/// - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
///   error handle that MUST be released with `openpit_destroy_shared_string`.
pub extern "C" fn openpit_engine_builder_add_builtin_order_validation_policy(
    builder: *mut crate::engine::OpenPitEngineBuilder,
    out_error: OpenPitOutError,
) -> bool {
    if builder.is_null() {
        write_error(out_error, "engine builder is null");
        return false;
    }
    match crate::engine::add_pre_trade_policy_to_builder(
        unsafe { &mut *builder },
        OrderValidationPolicy::new(),
    ) {
        Ok(()) => true,
        Err(err) => {
            write_error(out_error, &err);
            false
        }
    }
}

#[no_mangle]
/// Adds the built-in rate-limit policy to the engine builder.
///
/// Contract:
/// - `builder` must be a valid engine builder pointer.
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
    match crate::engine::add_pre_trade_policy_to_builder(builder_ref, policy) {
        Ok(()) => true,
        Err(err) => {
            write_error(out_error, &err);
            false
        }
    }
}

#[no_mangle]
/// Adds the built-in order-size limit policy to the engine builder.
///
/// Contract:
/// - `builder` must be a valid engine builder pointer.
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
    match crate::engine::add_pre_trade_policy_to_builder(unsafe { &mut *builder }, policy) {
        Ok(()) => true,
        Err(err) => {
            write_error(out_error, &err);
            false
        }
    }
}

#[no_mangle]
/// Adds the built-in P&L bounds kill-switch policy to the engine builder.
///
/// Contract:
/// - `builder` must be a valid engine builder pointer.
/// - At least one barrier must be provided: `broker_len > 0` or
///   `account_len > 0`.
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
///   barrier is configured, or when argument parsing fails;
/// - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
///   error handle that MUST be released with `openpit_destroy_shared_string`.
pub unsafe extern "C" fn openpit_engine_builder_add_builtin_pnl_bounds_killswitch_policy(
    builder: *mut crate::engine::OpenPitEngineBuilder,
    broker: *const OpenPitPretradePoliciesPnlBoundsBarrier,
    broker_len: usize,
    account: *const OpenPitPretradePoliciesPnlBoundsAccountBarrier,
    account_len: usize,
    out_error: OpenPitOutError,
) -> bool {
    if builder.is_null() {
        write_error(out_error, "engine builder is null");
        return false;
    }
    let broker_slice = match unsafe {
        try_slice_arg(
            broker,
            broker_len,
            "pnl_bounds_killswitch_policy broker",
            out_error,
        )
    } {
        Some(v) => v,
        None => return false,
    };
    let mut barriers = Vec::with_capacity(broker_slice.len());
    for (index, param) in broker_slice.iter().enumerate() {
        let settlement = match parse_settlement_asset_or_error(
            param.settlement_asset,
            "broker",
            index,
            out_error,
        ) {
            Some(v) => v,
            None => return false,
        };
        let lower_bound = match parse_optional_pnl_or_error(
            param.lower_bound,
            "broker",
            index,
            "lower_bound",
            out_error,
        ) {
            Ok(v) => v,
            Err(()) => return false,
        };
        let upper_bound = match parse_optional_pnl_or_error(
            param.upper_bound,
            "broker",
            index,
            "upper_bound",
            out_error,
        ) {
            Ok(v) => v,
            Err(()) => return false,
        };
        barriers.push(PnlBoundsBrokerBarrier {
            settlement_asset: settlement,
            lower_bound,
            upper_bound,
        });
    }

    let account_slice = match unsafe {
        try_slice_arg(
            account,
            account_len,
            "pnl_bounds_killswitch_policy account",
            out_error,
        )
    } {
        Some(v) => v,
        None => return false,
    };
    let mut account_barriers = Vec::with_capacity(account_slice.len());
    for (index, param) in account_slice.iter().enumerate() {
        let account_id = AccountId::from_u64(param.account_id);
        let settlement = match parse_settlement_asset_or_error(
            param.settlement_asset,
            "account",
            index,
            out_error,
        ) {
            Some(v) => v,
            None => return false,
        };
        let lower_bound = match parse_optional_pnl_or_error(
            param.lower_bound,
            "account",
            index,
            "lower_bound",
            out_error,
        ) {
            Ok(v) => v,
            Err(()) => return false,
        };
        let upper_bound = match parse_optional_pnl_or_error(
            param.upper_bound,
            "account",
            index,
            "upper_bound",
            out_error,
        ) {
            Ok(v) => v,
            Err(()) => return false,
        };
        let initial_pnl = match param.initial_pnl.to_param() {
            Ok(v) => v,
            Err(e) => {
                write_error_format!(out_error, "account[{index}] initial_pnl is invalid: {}", e);
                return false;
            }
        };
        account_barriers.push(PnlBoundsAccountAssetBarrier {
            barrier: PnlBoundsBrokerBarrier {
                settlement_asset: settlement,
                lower_bound,
                upper_bound,
            },
            account_id,
            initial_pnl,
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
    let policy = match PnlBoundsKillSwitchPolicy::new(barriers, account_barriers, storage) {
        Ok(v) => v,
        Err(e) => {
            write_error_format!(
                out_error,
                "pnl_bounds_killswitch_policy creation failed: {}",
                e
            );
            return false;
        }
    };
    match crate::engine::add_pre_trade_policy_to_builder(builder_ref, policy) {
        Ok(()) => true,
        Err(err) => {
            write_error(out_error, &err);
            false
        }
    }
}

//--------------------------------------------------------------------------------------------------

#[no_mangle]
/// Destroys the caller-owned pointer for a pre-trade policy.
///
/// Lifetime contract:
/// - Call this exactly once for each pointer that was returned to the caller
///   by a custom policy create function.
/// - After this call the pointer is no longer valid.
/// - Passing a null pointer is allowed and has no effect.
/// - This function always succeeds.
/// - If the policy was previously added to the engine builder, the engine
///   keeps its own reference and may continue using the policy.
/// - Destroying this caller-owned pointer does not remove the policy from
///   the engine.
pub extern "C" fn openpit_destroy_pretrade_pre_trade_policy(
    policy: *mut OpenPitPretradePreTradePolicy,
) {
    if policy.is_null() {
        return;
    }
    unsafe { drop(Box::from_raw(policy)) };
}

//--------------------------------------------------------------------------------------------------

#[no_mangle]
/// Returns the stable policy name for a pre-trade policy pointer.
///
/// Contract:
/// - This function never fails.
/// - `policy` must be a valid non-null pointer.
/// - The returned view does not own memory.
/// - The view remains valid while the policy object is alive and its name
///   is not changed.
/// - Passing an invalid pointer aborts the call.
pub extern "C" fn openpit_pretrade_pre_trade_policy_get_name(
    policy: *const OpenPitPretradePreTradePolicy,
) -> OpenPitStringView {
    assert!(!policy.is_null());
    unsafe { (&*policy).get_name() }
}

//--------------------------------------------------------------------------------------------------

fn get_policy_arc<P: ?Sized>(
    builder: *mut crate::engine::OpenPitEngineBuilder,
    policy: *mut PolicyHandle<P>,
) -> Result<(*mut crate::engine::OpenPitEngineBuilder, Arc<P>), String> {
    if builder.is_null() {
        return Err("engine builder is null".to_string());
    }
    if policy.is_null() {
        return Err("policy is null".to_string());
    }
    let arc = Arc::clone(unsafe { &(*policy).policy });
    Ok((builder, arc))
}

#[no_mangle]
/// Adds a pre-trade policy to the engine builder.
///
/// Contract:
/// - `builder` must be a valid engine builder pointer.
/// - `policy` must be a valid non-null pre-trade policy pointer.
///
/// Success:
/// - returns `true` and the builder retains its own reference to the policy.
///
/// Error:
/// - returns `false` when the builder or policy cannot be used;
/// - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
///   error handle that MUST be released with `openpit_destroy_shared_string`.
///
/// Lifetime contract:
/// - The engine builder retains its own reference to the policy object.
/// - The caller still owns the passed pointer and must release that local pointer
///   separately with `openpit_destroy_pretrade_pre_trade_policy` when it is no
///   longer needed.
pub extern "C" fn openpit_engine_builder_add_pre_trade_policy(
    builder: *mut crate::engine::OpenPitEngineBuilder,
    policy: *mut OpenPitPretradePreTradePolicy,
    out_error: OpenPitOutError,
) -> bool {
    let result = get_policy_arc(builder, policy).and_then(|(b, policy)| {
        crate::engine::add_pre_trade_policy_to_builder(
            unsafe { &mut *b },
            DynPreTradePolicy { inner: policy },
        )
    });
    match result {
        Ok(()) => true,
        Err(err) => {
            write_error(out_error, &err);
            false
        }
    }
}

//--------------------------------------------------------------------------------------------------

/// Opaque context passed to main-stage C policy callbacks.
///
/// Valid only for the duration of the callback. Cannot be constructed by
/// caller code.
///
/// Future extension: this type is the designated seam for engine
/// storage-cell access. A read accessor will be added here when the engine
/// store is introduced.
pub struct OpenPitPretradeContext;

/// Opaque context passed to account-adjustment C policy callbacks.
///
/// Valid only for the duration of the callback. Cannot be constructed by
/// caller code.
///
/// Future extension: this type is the designated seam for engine
/// storage-cell access. A read accessor will be added here when the engine
/// store is introduced.
pub struct OpenPitAccountAdjustmentContext;

/// Opaque, non-owning pointer to the mutation collector.
///
/// Valid only during the policy callback that received it.
/// The caller must not store or use this pointer after the callback returns.
pub struct OpenPitMutations {
    mutations: *mut Mutations,
}

/// Callback invoked for either commit or rollback of a registered mutation.
pub type OpenPitMutationFn = unsafe extern "C" fn(user_data: *mut c_void);

/// Optional callback to release mutation user_data after execution.
///
/// Called exactly once per `openpit_mutations_push`:
/// - after `commit_fn` when commit runs;
/// - after `rollback_fn` when rollback runs;
/// - or on drop if neither action ran.
pub type OpenPitMutationFreeFn = unsafe extern "C" fn(user_data: *mut c_void);

struct FfiMutationGuard {
    user_data: *mut c_void,
    free_fn: Option<OpenPitMutationFreeFn>,
}

impl Drop for FfiMutationGuard {
    fn drop(&mut self) {
        if let Some(free) = self.free_fn {
            unsafe { free(self.user_data) };
        }
    }
}

#[no_mangle]
/// Registers one commit/rollback mutation in the provided collector.
///
/// Contract:
/// - `mutations` must be a valid non-null callback-scoped pointer.
/// - `commit_fn` and `rollback_fn` must remain callable until one of them is
///   executed.
/// - `user_data` is passed to both callbacks.
/// - Exactly one of `commit_fn` or `rollback_fn` runs for each successful push.
/// - After the executed callback returns, `free_fn` is called exactly once when
///   provided.
/// - If neither callback runs (for example collector drop), only `free_fn`
///   runs exactly once when provided.
///
/// Error:
/// - returns `false` when `mutations` is null or invalid;
/// - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
///   error handle that MUST be released with `openpit_destroy_shared_string`.
pub unsafe extern "C" fn openpit_mutations_push(
    mutations: *mut OpenPitMutations,
    commit_fn: OpenPitMutationFn,
    rollback_fn: OpenPitMutationFn,
    user_data: *mut c_void,
    free_fn: Option<OpenPitMutationFreeFn>,
    out_error: OpenPitOutError,
) -> bool {
    if mutations.is_null() {
        write_error(out_error, "openpit_mutations_push: mutations is null");
        return false;
    }

    let raw_mutations = unsafe { (*mutations).mutations };
    if raw_mutations.is_null() {
        write_error(out_error, "openpit_mutations_push: inner mutations is null");
        return false;
    }

    let guard = Rc::new(FfiMutationGuard { user_data, free_fn });
    let commit_guard = Rc::clone(&guard);
    let rollback_guard = Rc::clone(&guard);

    unsafe {
        (*raw_mutations).push(Mutation::new(
            move || {
                commit_fn(user_data);
                drop(commit_guard);
            },
            move || {
                rollback_fn(user_data);
                drop(rollback_guard);
            },
        ));
    }
    drop(guard);
    true
}

//--------------------------------------------------------------------------------------------------

/// Callback used by a custom pre-trade policy to validate one order before a
/// deferred pre-trade request is created.
///
/// Contract:
/// - `ctx` is a read-only context valid only for the duration of the callback.
/// - `order` points to a read-only order view valid only for the duration of
///   the callback.
/// - `order` is passed as a borrowed view and is not copied before the
///   callback runs.
/// - If the callback wants to keep any data from `order`, it must copy that
///   data before returning.
/// - Return null or an empty list to accept the order.
/// - Return a non-empty reject list to reject the order.
/// - A rejected order must set explicit `code` and `scope` values in every
///   list item.
/// - The returned list ownership is transferred to the engine; create it with
///   `openpit_pretrade_create_reject_list`.
/// - Every reject payload is copied into internal storage before the callback
///   returns.
/// - `user_data` is passed through unchanged from policy creation.
pub type OpenPitPretradePreTradePolicyCheckPreTradeStartFn =
    unsafe extern "C" fn(
        ctx: *const OpenPitPretradeContext,
        order: *const OpenPitOrder,
        user_data: *mut c_void,
    ) -> *mut OpenPitPretradeRejectList;

/// Callback used by a custom pre-trade policy to perform a main-stage check.
///
/// Contract:
/// - `ctx` is a read-only context valid only for the duration of the callback.
/// - `order` points to a read-only order view valid only for the duration of
///   the callback.
/// - `order` is passed as a borrowed view and is not copied before the
///   callback runs.
/// - If the callback wants to keep any data from `order`, it must copy that
///   data before returning.
/// - `mutations` is a callback-scoped non-owning pointer that allows the
///   callback to register commit/rollback mutations.
/// - The callback must not store or use `mutations` after return.
/// - Return null or an empty list to accept the order.
/// - Return a non-empty reject list to reject the order.
/// - Every returned reject must contain explicit `code` and `scope` values.
/// - The returned list ownership is transferred to the engine; create it with
///   `openpit_pretrade_create_reject_list`.
/// - Every reject payload is copied into internal storage before this callback
///   returns.
/// - `user_data` is passed through unchanged from policy creation.
pub type OpenPitPretradePreTradePolicyPerformPreTradeCheckFn =
    unsafe extern "C" fn(
        ctx: *const OpenPitPretradeContext,
        order: *const OpenPitOrder,
        mutations: *mut OpenPitMutations,
        user_data: *mut c_void,
    ) -> *mut OpenPitPretradeRejectList;

/// Callback used by a custom pre-trade policy to observe an execution report.
///
/// Contract:
/// - `report` points to a read-only report view valid only for the duration of
///   the callback.
/// - `report` is passed as a borrowed view and is not copied before the
///   callback runs.
/// - If the callback wants to keep any data from `report`, it must copy that
///   data before returning.
/// - Return a non-null account-block list when this policy reports a
///   kill-switch trigger. The returned list ownership is transferred to the
///   engine; create it with `openpit_pretrade_create_account_block_list`.
/// - Return null to indicate no kill-switch condition.
/// - A null `apply_execution_report_fn` means that hook returns an empty list
///   (no kill switch).
/// - `user_data` is passed through unchanged from policy creation.
pub type OpenPitPretradePreTradePolicyApplyExecutionReportFn =
    unsafe extern "C" fn(
        report: *const OpenPitExecutionReport,
        user_data: *mut c_void,
    ) -> *mut OpenPitPretradeAccountBlockList;

/// Callback used by a custom pre-trade policy to validate one account
/// adjustment.
///
/// Contract:
/// - `ctx` is a read-only context valid only for the duration of the callback.
/// - `adjustment` points to a read-only adjustment view valid only for the
///   duration of the callback.
/// - `adjustment` is passed as a borrowed view and is not copied before the
///   callback runs.
/// - If the callback wants to keep any data from `adjustment`, it must copy
///   that data before returning.
/// - `account_id` must follow the same source model as the rest of the
///   runtime state (numeric-only or string-derived-only).
/// - `mutations` is a callback-scoped non-owning pointer that allows the
///   callback to register commit/rollback mutations.
/// - The callback must not store or use `mutations` after return.
/// - Return null to accept the adjustment.
/// - Return a non-empty reject list to reject the adjustment.
/// - Returned reject list ownership is transferred to the callee.
/// - `user_data` is passed through unchanged from policy creation.
pub type OpenPitPretradePreTradePolicyApplyAccountAdjustmentFn =
    unsafe extern "C" fn(
        ctx: *const OpenPitAccountAdjustmentContext,
        account_id: OpenPitParamAccountId,
        adjustment: *const OpenPitAccountAdjustment,
        mutations: *mut OpenPitMutations,
        user_data: *mut c_void,
    ) -> *mut OpenPitPretradeRejectList;

/// Callback invoked when the last reference to a custom pre-trade policy is
/// released and the policy object is about to be destroyed.
///
/// Contract:
/// - Called exactly once, on the thread that drops the last policy reference.
/// - After this callback returns, no further callbacks will be invoked for
///   this policy instance.
/// - `user_data` is the same value that was passed at policy creation.
/// - The callback must release any resources associated with `user_data`.
pub type OpenPitPretradePreTradePolicyFreeUserDataFn = unsafe extern "C" fn(user_data: *mut c_void);

//--------------------------------------------------------------------------------------------------

struct DynPreTradePolicy {
    inner: Arc<UnifiedPreTradePolicy>,
}

// SAFETY: The binding threading contract (engine.rs module comment) describes
// when concurrent calls are allowed. The inner Arc's concrete type is a custom
// callback struct whose user_data is accessed under that contract. The Arc
// refcount is atomically maintained. Sequential transfer across OS threads is
// permitted by the contract.
unsafe impl Send for DynPreTradePolicy {}
// SAFETY: the concrete type behind the dyn object is `CustomPreTradePolicy`,
// which implements `Send + Sync` (see its unsafe impls). The Arc refcount is
// thread-safe. Concurrent access to `&self` methods is safe under
// `SyncMode::Full`; under other modes the binding caller serialises per-handle
// invocation per the SDK threading contract.
unsafe impl Sync for DynPreTradePolicy {}

impl PreTradePolicy<Order, ExecutionReport, AccountAdjustment> for DynPreTradePolicy {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn check_pre_trade_start(&self, ctx: &PreTradeContext, order: &Order) -> Result<(), Rejects> {
        self.inner.check_pre_trade_start(ctx, order)
    }

    fn perform_pre_trade_check(
        &self,
        ctx: &PreTradeContext,
        order: &Order,
        mutations: &mut Mutations,
    ) -> Result<(), Rejects> {
        self.inner.perform_pre_trade_check(ctx, order, mutations)
    }

    fn apply_execution_report(
        &self,
        report: &ExecutionReport,
    ) -> Vec<openpit::pretrade::AccountBlock> {
        self.inner.apply_execution_report(report)
    }

    fn apply_account_adjustment(
        &self,
        ctx: &AccountAdjustmentContext,
        account_id: openpit::param::AccountId,
        adjustment: &AccountAdjustment,
        mutations: &mut Mutations,
    ) -> Result<(), Rejects> {
        self.inner
            .apply_account_adjustment(ctx, account_id, adjustment, mutations)
    }
}

//--------------------------------------------------------------------------------------------------

struct CustomPreTradePolicy {
    name: String,
    check_pre_trade_start_fn: Option<OpenPitPretradePreTradePolicyCheckPreTradeStartFn>,
    perform_pre_trade_check_fn: Option<OpenPitPretradePreTradePolicyPerformPreTradeCheckFn>,
    apply_execution_report_fn: Option<OpenPitPretradePreTradePolicyApplyExecutionReportFn>,
    apply_account_adjustment_fn: Option<OpenPitPretradePreTradePolicyApplyAccountAdjustmentFn>,
    free_user_data_fn: OpenPitPretradePreTradePolicyFreeUserDataFn,
    user_data: *mut c_void,
}

// SAFETY:
// `CustomPreTradePolicy` holds `extern "C" fn` pointers (inherently `Send +
// Sync`) and a `*mut c_void` user_data slot. Raw pointers are `!Send + !Sync`
// by default; Send and Sync are asserted manually under the following contract:
//
// - The public Pit threading contract documents that user_data slots on custom
//   policy structs are opaque caller tokens. Their lifetime, thread-safety, and
//   meaning are entirely the caller's responsibility.
// - The SDK never inspects or dereferences user_data; it forwards it to the
//   registered C callbacks verbatim. Whatever synchronization the caller
//   attaches to user_data is the caller's contract to uphold.
// - Under `SyncMode::Local` or `SyncMode::Account` the binding caller
//   serialises per-handle invocation; under `SyncMode::Full` the caller is
//   responsible for making any state reachable through user_data safe under
//   concurrent invocation.
//
// Violating the user_data contract is undefined behavior at the contract level.
unsafe impl Send for CustomPreTradePolicy {}
unsafe impl Sync for CustomPreTradePolicy {}

impl PreTradePolicy<Order, ExecutionReport, AccountAdjustment> for CustomPreTradePolicy {
    fn name(&self) -> &str {
        &self.name
    }

    fn check_pre_trade_start(&self, ctx: &PreTradeContext, order: &Order) -> Result<(), Rejects> {
        let Some(check_fn) = self.check_pre_trade_start_fn else {
            return Ok(());
        };
        let input = export_order(order);
        let c_ctx = (ctx as *const PreTradeContext).cast::<OpenPitPretradeContext>();
        let rejects = unsafe { check_fn(c_ctx, &input, self.user_data) };
        import_reject_list_result(rejects)
    }

    fn perform_pre_trade_check(
        &self,
        ctx: &PreTradeContext,
        order: &Order,
        mutations: &mut Mutations,
    ) -> Result<(), Rejects> {
        let Some(check_fn) = self.perform_pre_trade_check_fn else {
            return Ok(());
        };
        let mut mutations_handle = OpenPitMutations {
            mutations: mutations as *mut Mutations,
        };
        let input = export_order(order);
        let c_ctx = (ctx as *const PreTradeContext).cast::<OpenPitPretradeContext>();
        let rejects = unsafe { check_fn(c_ctx, &input, &mut mutations_handle, self.user_data) };
        import_reject_list_result(rejects)
    }

    fn apply_execution_report(
        &self,
        report: &ExecutionReport,
    ) -> Vec<openpit::pretrade::AccountBlock> {
        let Some(apply_fn) = self.apply_execution_report_fn else {
            return vec![];
        };
        let input = export_execution_report(report);
        let raw = unsafe { apply_fn(&input, self.user_data) };
        if raw.is_null() {
            return vec![];
        }
        let list = unsafe { Box::from_raw(raw) };
        list.items
    }

    fn apply_account_adjustment(
        &self,
        ctx: &AccountAdjustmentContext,
        account_id: openpit::param::AccountId,
        adjustment: &AccountAdjustment,
        mutations: &mut Mutations,
    ) -> Result<(), Rejects> {
        let Some(apply_fn) = self.apply_account_adjustment_fn else {
            return Ok(());
        };
        let mut mutations_handle = OpenPitMutations {
            mutations: mutations as *mut Mutations,
        };
        let input = export_account_adjustment(adjustment);
        let c_ctx =
            (ctx as *const AccountAdjustmentContext).cast::<OpenPitAccountAdjustmentContext>();
        let rejects = unsafe {
            apply_fn(
                c_ctx,
                account_id.as_u64(),
                &input,
                &mut mutations_handle,
                self.user_data,
            )
        };
        import_reject_list_result(rejects)
    }
}

impl Drop for CustomPreTradePolicy {
    fn drop(&mut self) {
        unsafe { (self.free_user_data_fn)(self.user_data) };
    }
}

//--------------------------------------------------------------------------------------------------

unsafe fn parse_policy_name(
    name_ptr: OpenPitStringView,
    out_error: OpenPitOutError,
) -> Option<String> {
    if name_ptr.ptr.is_null() {
        write_error(out_error, "policy name is null");
        return None;
    }
    let bytes = unsafe { std::slice::from_raw_parts(name_ptr.ptr, name_ptr.len) };
    let value = match str::from_utf8(bytes) {
        Ok(v) => v,
        Err(_) => {
            write_error(out_error, "policy name is not valid string");
            return None;
        }
    };
    if value.is_empty() {
        write_error(out_error, "policy name is empty");
        return None;
    }
    Some(value.to_owned())
}

fn import_reject_list_result(rejects: *mut OpenPitPretradeRejectList) -> Result<(), Rejects> {
    if rejects.is_null() {
        return Ok(());
    }
    let rejects = unsafe { Box::from_raw(rejects) };
    if rejects.items.is_empty() {
        return Ok(());
    }
    Err(Rejects::from(rejects.items))
}

#[no_mangle]
/// Creates a custom pre-trade policy from caller-provided callbacks.
///
/// Contract:
/// - `name` must point to a valid, null-terminated string for the duration of
///   the call.
/// - `check_pre_trade_start_fn`, `perform_pre_trade_check_fn`,
///   `apply_execution_report_fn`, and `apply_account_adjustment_fn` may be null.
/// - A null `check_pre_trade_start_fn`, `perform_pre_trade_check_fn`, or
///   `apply_account_adjustment_fn` means that hook accepts by default.
/// - A null `apply_execution_report_fn` means that hook returns an empty list (no kill switch).
/// - Non-null callbacks and `free_user_data_fn` must remain callable for as long
///   as the policy may still be used by either the caller pointer or the engine.
/// - Custom main-stage and account-adjustment callbacks can register
///   commit/rollback mutations through their `mutations` pointer.
/// - `free_user_data_fn` will be called exactly once, when the last reference
///   to the policy is released.
/// - `user_data` is opaque to the SDK: the engine never inspects, dereferences,
///   or frees it; it is forwarded verbatim to the registered callbacks.
///   Lifetime, thread-safety, and meaning of the pointed-at state are entirely
///   the caller's responsibility. Under `OpenPitSyncPolicy_Local` or
///   `OpenPitSyncPolicy_Account`, the caller serialises per-handle invocation per
///   the SDK threading contract; under `OpenPitSyncPolicy_Full`, the caller is
///   responsible for making any state reachable through `user_data` safe under
///   concurrent invocation.
///
/// Success:
/// - returns a new caller-owned policy object.
///
/// Error:
/// - returns null when `name` is invalid;
/// - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
///   error handle that MUST be released with `openpit_destroy_shared_string`.
///
/// Lifetime contract:
/// - The policy stores its own copy of `name`; the caller may release the input
///   string after this function returns.
/// - The returned pointer is owned by the caller and must be released with
///   `openpit_destroy_pretrade_pre_trade_policy` when no longer needed.
/// - If the policy is added to the engine builder, the engine keeps its own
///   reference, but the caller must still release the caller-owned pointer.
/// - `free_user_data_fn` runs once the last reference to the policy is
///   released; when the engine is the final holder, it runs as part of engine
///   destruction.
pub unsafe extern "C" fn openpit_create_pretrade_custom_pre_trade_policy(
    name: OpenPitStringView,
    check_pre_trade_start_fn: Option<OpenPitPretradePreTradePolicyCheckPreTradeStartFn>,
    perform_pre_trade_check_fn: Option<OpenPitPretradePreTradePolicyPerformPreTradeCheckFn>,
    apply_execution_report_fn: Option<OpenPitPretradePreTradePolicyApplyExecutionReportFn>,
    apply_account_adjustment_fn: Option<OpenPitPretradePreTradePolicyApplyAccountAdjustmentFn>,
    free_user_data_fn: OpenPitPretradePreTradePolicyFreeUserDataFn,
    user_data: *mut c_void,
    out_error: OpenPitOutError,
) -> *mut OpenPitPretradePreTradePolicy {
    let name = match unsafe { parse_policy_name(name, out_error) } {
        Some(v) => v,
        None => return std::ptr::null_mut(),
    };

    let policy = CustomPreTradePolicy {
        name,
        check_pre_trade_start_fn,
        perform_pre_trade_check_fn,
        apply_execution_report_fn,
        apply_account_adjustment_fn,
        free_user_data_fn,
        user_data,
    };

    OpenPitPretradePreTradePolicy::new(Arc::new(policy))
}

//--------------------------------------------------------------------------------------------------

unsafe fn cstr_arg(ptr: OpenPitStringView) -> Option<String> {
    if ptr.ptr.is_null() {
        return None;
    }
    let bytes = unsafe { std::slice::from_raw_parts(ptr.ptr, ptr.len) };
    let value = str::from_utf8(bytes).ok()?.to_owned();
    Some(value)
}

//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use openpit::param::{AccountId, Asset, Quantity, Side, TradeAmount};
    use openpit::Instrument;
    use openpit_interop::{OrderOperationAccess, PopulatedOrderOperation};

    use super::*;

    use crate::param::{OpenPitParamDecimal, OpenPitParamPnl};
    use crate::reject::{OpenPitPretradeAccountBlockList, OpenPitPretradeRejectList};

    unsafe extern "C" fn custom_check_fn(
        _ctx: *const OpenPitPretradeContext,
        _order: *const OpenPitOrder,
        _user_data: *mut c_void,
    ) -> *mut OpenPitPretradeRejectList {
        std::ptr::null_mut()
    }

    unsafe extern "C" fn custom_apply_report_fn(
        _report: *const OpenPitExecutionReport,
        _user_data: *mut c_void,
    ) -> *mut OpenPitPretradeAccountBlockList {
        std::ptr::null_mut()
    }

    unsafe extern "C" fn custom_free_user_data_fn(_user_data: *mut c_void) {}
    unsafe extern "C" fn custom_pre_trade_check_fn(
        _ctx: *const OpenPitPretradeContext,
        _order: *const OpenPitOrder,
        _mutations: *mut OpenPitMutations,
        _user_data: *mut c_void,
    ) -> *mut OpenPitPretradeRejectList {
        std::ptr::null_mut()
    }

    unsafe extern "C" fn custom_account_adjustment_apply_fn(
        _ctx: *const OpenPitAccountAdjustmentContext,
        _account_id: OpenPitParamAccountId,
        _adjustment: *const OpenPitAccountAdjustment,
        _mutations: *mut OpenPitMutations,
        _user_data: *mut c_void,
    ) -> *mut OpenPitPretradeRejectList {
        std::ptr::null_mut()
    }

    unsafe fn create_pre_trade_policy_with_start_hook(
        name: OpenPitStringView,
        check_pte_trade_start_fn: OpenPitPretradePreTradePolicyCheckPreTradeStartFn,
        apply_execution_report_fn: OpenPitPretradePreTradePolicyApplyExecutionReportFn,
        free_user_data_fn: OpenPitPretradePreTradePolicyFreeUserDataFn,
        user_data: *mut c_void,
        out_error: OpenPitOutError,
    ) -> *mut OpenPitPretradePreTradePolicy {
        unsafe {
            openpit_create_pretrade_custom_pre_trade_policy(
                name,
                Some(check_pte_trade_start_fn),
                None,
                Some(apply_execution_report_fn),
                None,
                free_user_data_fn,
                user_data,
                out_error,
            )
        }
    }

    unsafe fn create_pre_trade_policy_with_account_adjustment_hook(
        name: OpenPitStringView,
        apply_fn: OpenPitPretradePreTradePolicyApplyAccountAdjustmentFn,
        free_user_data_fn: OpenPitPretradePreTradePolicyFreeUserDataFn,
        user_data: *mut c_void,
        out_error: OpenPitOutError,
    ) -> *mut OpenPitPretradePreTradePolicy {
        unsafe {
            openpit_create_pretrade_custom_pre_trade_policy(
                name,
                None,
                None,
                None,
                Some(apply_fn),
                free_user_data_fn,
                user_data,
                out_error,
            )
        }
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
        crate::string::openpit_destroy_shared_string(handle);
        result
    }

    fn string_view_to_string(view: OpenPitStringView) -> String {
        if view.ptr.is_null() {
            return String::new();
        }
        let bytes = unsafe { std::slice::from_raw_parts(view.ptr, view.len) };
        std::str::from_utf8(bytes).expect("utf8").to_string()
    }

    fn pnl_param(mantissa: i128, scale: i32) -> OpenPitParamPnl {
        OpenPitParamPnl(OpenPitParamDecimal {
            mantissa_lo: mantissa as i64,
            mantissa_hi: (mantissa >> 64) as i64,
            scale,
        })
    }

    fn pnl_optional(value: Option<OpenPitParamPnl>) -> OpenPitParamPnlOptional {
        match value {
            Some(v) => OpenPitParamPnlOptional {
                is_set: true,
                value: v,
            },
            None => OpenPitParamPnlOptional::default(),
        }
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

    #[derive(Default)]
    struct MutationState {
        commit_calls: usize,
        rollback_calls: usize,
        free_calls: usize,
        sequence: Vec<u8>,
    }

    struct MutationUserData {
        state: Rc<RefCell<MutationState>>,
        marker: u8,
    }

    struct MutationPushContext {
        entries: Vec<*mut c_void>,
        free_fn: Option<OpenPitMutationFreeFn>,
    }

    fn sample_order() -> Order {
        openpit_interop::RequestWithPayload::new(
            openpit_interop::Order {
                operation: OrderOperationAccess::Populated(PopulatedOrderOperation {
                    instrument: Some(Instrument::new(
                        Asset::new("AAPL").expect("asset code must be valid"),
                        Asset::new("USD").expect("asset code must be valid"),
                    )),
                    account_id: Some(AccountId::from_u64(99224416)),
                    side: Some(Side::Buy),
                    trade_amount: Some(TradeAmount::Quantity(
                        Quantity::from_str("1").expect("quantity must be valid"),
                    )),
                    price: None,
                }),
                position: openpit_interop::OrderPositionAccess::Absent,
                margin: openpit_interop::OrderMarginAccess::Absent,
            },
            std::ptr::null_mut(),
        )
    }

    fn execute_with_custom_pre_trade_policy(
        check_fn: OpenPitPretradePreTradePolicyPerformPreTradeCheckFn,
        user_data: *mut c_void,
    ) -> openpit::pretrade::PreTradeReservation {
        let engine = openpit::EngineBuilder::<Order, ExecutionReport, AccountAdjustment>::new()
            .sync(openpit::LocalSync)
            .pre_trade(CustomPreTradePolicy {
                name: "ffi.custom".to_owned(),
                check_pre_trade_start_fn: None,
                perform_pre_trade_check_fn: Some(check_fn),
                apply_execution_report_fn: Some(custom_apply_report_fn),
                apply_account_adjustment_fn: None,
                free_user_data_fn: custom_free_user_data_fn,
                user_data,
            })
            .build()
            .expect("engine build must succeed");
        engine
            .start_pre_trade(sample_order())
            .expect("start pre-trade must succeed")
            .execute()
            .expect("main pre-trade must succeed")
    }

    unsafe extern "C" fn tracked_mutation_commit(user_data: *mut c_void) {
        let data = unsafe { &*(user_data as *mut MutationUserData) };
        let mut state = data.state.borrow_mut();
        state.commit_calls += 1;
        state.sequence.push(data.marker);
    }

    unsafe extern "C" fn tracked_mutation_rollback(user_data: *mut c_void) {
        let data = unsafe { &*(user_data as *mut MutationUserData) };
        let mut state = data.state.borrow_mut();
        state.rollback_calls += 1;
        state.sequence.push(data.marker);
    }

    unsafe extern "C" fn tracked_mutation_free(user_data: *mut c_void) {
        let data = unsafe { Box::from_raw(user_data as *mut MutationUserData) };
        data.state.borrow_mut().free_calls += 1;
    }

    unsafe extern "C" fn push_tracked_mutations_check_fn(
        _ctx: *const OpenPitPretradeContext,
        _order: *const OpenPitOrder,
        mutations: *mut OpenPitMutations,
        user_data: *mut c_void,
    ) -> *mut OpenPitPretradeRejectList {
        let ctx = unsafe { &*(user_data as *const MutationPushContext) };
        for entry in &ctx.entries {
            let ok = unsafe {
                openpit_mutations_push(
                    mutations,
                    tracked_mutation_commit,
                    tracked_mutation_rollback,
                    *entry,
                    ctx.free_fn,
                    std::ptr::null_mut(),
                )
            };
            assert!(ok, "{}", cstr_to_string(std::ptr::null_mut()));
        }
        std::ptr::null_mut()
    }

    #[test]
    fn mutations_push_commit_calls_commit_fn_and_free() {
        let state = Rc::new(RefCell::new(MutationState::default()));
        let entry = Box::into_raw(Box::new(MutationUserData {
            state: Rc::clone(&state),
            marker: 1,
        }))
        .cast();
        let mut ctx = MutationPushContext {
            entries: vec![entry],
            free_fn: Some(tracked_mutation_free),
        };

        let mut reservation = execute_with_custom_pre_trade_policy(
            push_tracked_mutations_check_fn,
            (&mut ctx as *mut MutationPushContext).cast(),
        );
        reservation.commit();

        let state = state.borrow();
        assert_eq!(state.commit_calls, 1);
        assert_eq!(state.rollback_calls, 0);
        assert_eq!(state.free_calls, 1);
    }

    #[test]
    fn mutations_push_rollback_calls_rollback_fn_and_free() {
        let state = Rc::new(RefCell::new(MutationState::default()));
        let entry = Box::into_raw(Box::new(MutationUserData {
            state: Rc::clone(&state),
            marker: 1,
        }))
        .cast();
        let mut ctx = MutationPushContext {
            entries: vec![entry],
            free_fn: Some(tracked_mutation_free),
        };

        let mut reservation = execute_with_custom_pre_trade_policy(
            push_tracked_mutations_check_fn,
            (&mut ctx as *mut MutationPushContext).cast(),
        );
        reservation.rollback();

        let state = state.borrow();
        assert_eq!(state.commit_calls, 0);
        assert_eq!(state.rollback_calls, 1);
        assert_eq!(state.free_calls, 1);
    }

    #[test]
    fn mutations_push_drop_calls_free_without_action() {
        let state = Rc::new(RefCell::new(MutationState::default()));
        let entry = Box::into_raw(Box::new(MutationUserData {
            state: Rc::clone(&state),
            marker: 7,
        }))
        .cast();

        let mut mutations = Mutations::new();
        let mut pointer = OpenPitMutations {
            mutations: &mut mutations as *mut Mutations,
        };
        let ok = unsafe {
            openpit_mutations_push(
                &mut pointer,
                tracked_mutation_commit,
                tracked_mutation_rollback,
                entry,
                Some(tracked_mutation_free),
                std::ptr::null_mut(),
            )
        };
        assert!(ok, "{}", cstr_to_string(std::ptr::null_mut()));

        drop(mutations);

        let state = state.borrow();
        assert_eq!(state.commit_calls, 0);
        assert_eq!(state.rollback_calls, 0);
        assert_eq!(state.free_calls, 1);
    }

    #[test]
    fn mutations_push_null_free_fn_no_crash() {
        unsafe extern "C" fn commit_without_free(user_data: *mut c_void) {
            let state = unsafe { &*(user_data as *const RefCell<MutationState>) };
            state.borrow_mut().commit_calls += 1;
        }
        unsafe extern "C" fn rollback_without_free(_user_data: *mut c_void) {}

        let state = RefCell::new(MutationState::default());
        let entry = (&state as *const RefCell<MutationState>).cast_mut().cast();
        let mut ctx = MutationPushContext {
            entries: vec![entry],
            free_fn: None,
        };

        unsafe extern "C" fn push_without_free_check_fn(
            _ctx: *const OpenPitPretradeContext,
            _order: *const OpenPitOrder,
            mutations: *mut OpenPitMutations,
            user_data: *mut c_void,
        ) -> *mut OpenPitPretradeRejectList {
            let ctx = unsafe { &*(user_data as *const MutationPushContext) };
            let ok = unsafe {
                openpit_mutations_push(
                    mutations,
                    commit_without_free,
                    rollback_without_free,
                    ctx.entries[0],
                    None,
                    std::ptr::null_mut(),
                )
            };
            assert!(ok, "{}", cstr_to_string(std::ptr::null_mut()));
            std::ptr::null_mut()
        }

        let mut reservation = execute_with_custom_pre_trade_policy(
            push_without_free_check_fn,
            (&mut ctx as *mut MutationPushContext).cast(),
        );
        reservation.commit();

        assert_eq!(state.borrow().commit_calls, 1);
    }

    #[test]
    fn mutations_push_null_handle_returns_false() {
        unsafe extern "C" fn noop(_user_data: *mut c_void) {}

        let ok = unsafe {
            openpit_mutations_push(
                std::ptr::null_mut(),
                noop,
                noop,
                std::ptr::null_mut(),
                None,
                std::ptr::null_mut(),
            )
        };
        assert!(!ok);
    }

    #[test]
    fn mutations_push_ordering() {
        let state = Rc::new(RefCell::new(MutationState::default()));
        let mut commit_entries = Vec::new();
        for marker in [1_u8, 2, 3] {
            commit_entries.push(
                Box::into_raw(Box::new(MutationUserData {
                    state: Rc::clone(&state),
                    marker,
                }))
                .cast(),
            );
        }
        let mut commit_ctx = MutationPushContext {
            entries: commit_entries,
            free_fn: Some(tracked_mutation_free),
        };

        let mut reservation = execute_with_custom_pre_trade_policy(
            push_tracked_mutations_check_fn,
            (&mut commit_ctx as *mut MutationPushContext).cast(),
        );
        reservation.commit();

        {
            let state = state.borrow();
            assert_eq!(state.sequence, vec![1, 2, 3]);
            assert_eq!(state.free_calls, 3);
        }

        state.borrow_mut().sequence.clear();
        state.borrow_mut().free_calls = 0;

        let mut rollback_entries = Vec::new();
        for marker in [1_u8, 2, 3] {
            rollback_entries.push(
                Box::into_raw(Box::new(MutationUserData {
                    state: Rc::clone(&state),
                    marker,
                }))
                .cast(),
            );
        }
        let mut rollback_ctx = MutationPushContext {
            entries: rollback_entries,
            free_fn: Some(tracked_mutation_free),
        };

        let mut reservation = execute_with_custom_pre_trade_policy(
            push_tracked_mutations_check_fn,
            (&mut rollback_ctx as *mut MutationPushContext).cast(),
        );
        reservation.rollback();

        let state = state.borrow();
        assert_eq!(state.sequence, vec![3, 2, 1]);
        assert_eq!(state.free_calls, 3);
    }

    #[test]
    fn custom_pre_trade_policy_callback_can_push_mutations() {
        let state = Rc::new(RefCell::new(MutationState::default()));
        let entry = Box::into_raw(Box::new(MutationUserData {
            state: Rc::clone(&state),
            marker: 42,
        }))
        .cast();
        let mut ctx = MutationPushContext {
            entries: vec![entry],
            free_fn: Some(tracked_mutation_free),
        };

        let mut reservation = execute_with_custom_pre_trade_policy(
            push_tracked_mutations_check_fn,
            (&mut ctx as *mut MutationPushContext).cast(),
        );
        reservation.commit();

        let state = state.borrow();
        assert_eq!(state.commit_calls, 1);
        assert_eq!(state.free_calls, 1);
    }

    #[test]
    fn add_policy_reports_null_builder() {
        let name = OpenPitStringView::from_utf8("null.builder.check");
        let policy = unsafe {
            create_pre_trade_policy_with_start_hook(
                name,
                custom_check_fn,
                custom_apply_report_fn,
                custom_free_user_data_fn,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert!(!policy.is_null());
        let mut out_error = std::ptr::null_mut();
        let ok = openpit_engine_builder_add_pre_trade_policy(
            std::ptr::null_mut(),
            policy,
            &mut out_error,
        );
        assert!(!ok);
        assert_eq!(cstr_to_string(out_error), "engine builder is null");
        openpit_destroy_pretrade_pre_trade_policy(policy);
    }

    #[test]
    fn add_policy_reports_null_policy() {
        let builder = crate::engine::openpit_create_engine_builder(
            crate::engine::OpenPitSyncPolicy::Full as u8,
            std::ptr::null_mut(),
        );
        let mut out_error = std::ptr::null_mut();
        let ok = openpit_engine_builder_add_pre_trade_policy(
            builder,
            std::ptr::null_mut(),
            &mut out_error,
        );
        assert!(!ok);
        assert_eq!(cstr_to_string(out_error), "policy is null");
        crate::engine::openpit_destroy_engine_builder(builder);
    }

    #[test]
    fn custom_check_policy_keeps_caller_name() {
        let name = OpenPitStringView::from_utf8("caller.check.start");
        let pointer = unsafe {
            create_pre_trade_policy_with_start_hook(
                name,
                custom_check_fn,
                custom_apply_report_fn,
                custom_free_user_data_fn,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert!(
            !pointer.is_null(),
            "{}",
            cstr_to_string(std::ptr::null_mut())
        );

        let got = openpit_pretrade_pre_trade_policy_get_name(pointer);
        assert_eq!(string_view_to_string(got), "caller.check.start");
        openpit_destroy_pretrade_pre_trade_policy(pointer);
    }

    #[test]
    fn custom_pre_trade_policy_keeps_caller_name() {
        let name = OpenPitStringView::from_utf8("caller.pretrade");
        let pointer = unsafe {
            openpit_create_pretrade_custom_pre_trade_policy(
                name,
                None,
                Some(custom_pre_trade_check_fn),
                Some(custom_apply_report_fn),
                None,
                custom_free_user_data_fn,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert!(
            !pointer.is_null(),
            "{}",
            cstr_to_string(std::ptr::null_mut())
        );

        let got = openpit_pretrade_pre_trade_policy_get_name(pointer);
        assert_eq!(string_view_to_string(got), "caller.pretrade");
        openpit_destroy_pretrade_pre_trade_policy(pointer);
    }

    #[test]
    fn custom_pre_trade_policy_with_account_adjustment_hook_keeps_caller_name() {
        let name = OpenPitStringView::from_utf8("caller.account.adjustment");
        let pointer = unsafe {
            create_pre_trade_policy_with_account_adjustment_hook(
                name,
                custom_account_adjustment_apply_fn,
                custom_free_user_data_fn,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert!(
            !pointer.is_null(),
            "{}",
            cstr_to_string(std::ptr::null_mut())
        );

        let got = openpit_pretrade_pre_trade_policy_get_name(pointer);
        assert_eq!(string_view_to_string(got), "caller.account.adjustment");
        openpit_destroy_pretrade_pre_trade_policy(pointer);
    }

    #[test]
    fn custom_policy_create_rejects_null_empty_and_invalid_name() {
        let mut out_error = std::ptr::null_mut();
        let null_name = unsafe {
            create_pre_trade_policy_with_start_hook(
                OpenPitStringView::not_set(),
                custom_check_fn,
                custom_apply_report_fn,
                custom_free_user_data_fn,
                std::ptr::null_mut(),
                &mut out_error,
            )
        };
        assert!(null_name.is_null());
        assert!(cstr_to_string(out_error).contains("policy name is null"));

        let empty = OpenPitStringView::from_utf8("");
        let mut out_error = std::ptr::null_mut();
        let empty_name = unsafe {
            create_pre_trade_policy_with_start_hook(
                empty,
                custom_check_fn,
                custom_apply_report_fn,
                custom_free_user_data_fn,
                std::ptr::null_mut(),
                &mut out_error,
            )
        };
        assert!(empty_name.is_null());
        assert!(cstr_to_string(out_error).contains("policy name is empty"));

        let invalid_utf8 = [0xff_u8, 0x00];
        let mut out_error = std::ptr::null_mut();
        let invalid_name = unsafe {
            create_pre_trade_policy_with_start_hook(
                OpenPitStringView {
                    ptr: invalid_utf8.as_ptr(),
                    len: invalid_utf8.len(),
                },
                custom_check_fn,
                custom_apply_report_fn,
                custom_free_user_data_fn,
                std::ptr::null_mut(),
                &mut out_error,
            )
        };
        assert!(invalid_name.is_null());
        assert!(cstr_to_string(out_error).contains("policy name is not valid string"));
    }

    #[test]
    fn different_custom_names_do_not_collapse() {
        let name_a = OpenPitStringView::from_utf8("custom.a");
        let name_b = OpenPitStringView::from_utf8("custom.b");
        let handle_a = unsafe {
            create_pre_trade_policy_with_start_hook(
                name_a,
                custom_check_fn,
                custom_apply_report_fn,
                custom_free_user_data_fn,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        let handle_b = unsafe {
            create_pre_trade_policy_with_start_hook(
                name_b,
                custom_check_fn,
                custom_apply_report_fn,
                custom_free_user_data_fn,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert!(!handle_a.is_null());
        assert!(!handle_b.is_null());

        let got_a = openpit_pretrade_pre_trade_policy_get_name(handle_a);
        let got_b = openpit_pretrade_pre_trade_policy_get_name(handle_b);
        assert_eq!(string_view_to_string(got_a), "custom.a");
        assert_eq!(string_view_to_string(got_b), "custom.b");
        openpit_destroy_pretrade_pre_trade_policy(handle_a);
        openpit_destroy_pretrade_pre_trade_policy(handle_b);
    }

    #[test]
    fn add_pre_trade_policies_with_main_and_account_adjustment_hooks_to_builder() {
        let builder = crate::engine::openpit_create_engine_builder(
            crate::engine::OpenPitSyncPolicy::Full as u8,
            std::ptr::null_mut(),
        );

        let pre_trade_name = OpenPitStringView::from_utf8("caller.pretrade.add");
        let pre_trade_policy = unsafe {
            openpit_create_pretrade_custom_pre_trade_policy(
                pre_trade_name,
                None,
                Some(custom_pre_trade_check_fn),
                Some(custom_apply_report_fn),
                None,
                custom_free_user_data_fn,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert!(
            !pre_trade_policy.is_null(),
            "{}",
            cstr_to_string(std::ptr::null_mut())
        );
        let ok = openpit_engine_builder_add_pre_trade_policy(
            builder,
            pre_trade_policy,
            std::ptr::null_mut(),
        );
        assert!(ok, "{}", cstr_to_string(std::ptr::null_mut()));
        openpit_destroy_pretrade_pre_trade_policy(pre_trade_policy);

        let account_name = OpenPitStringView::from_utf8("caller.adjustment.add");
        let account_policy = unsafe {
            create_pre_trade_policy_with_account_adjustment_hook(
                account_name,
                custom_account_adjustment_apply_fn,
                custom_free_user_data_fn,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert!(
            !account_policy.is_null(),
            "{}",
            cstr_to_string(std::ptr::null_mut())
        );
        let ok = openpit_engine_builder_add_pre_trade_policy(
            builder,
            account_policy,
            std::ptr::null_mut(),
        );
        assert!(ok, "{}", cstr_to_string(std::ptr::null_mut()));
        openpit_destroy_pretrade_pre_trade_policy(account_policy);

        let engine = crate::engine::openpit_engine_builder_build(builder, std::ptr::null_mut());
        assert!(
            !engine.is_null(),
            "{}",
            cstr_to_string(std::ptr::null_mut())
        );
        crate::engine::openpit_destroy_engine(engine);
        crate::engine::openpit_destroy_engine_builder(builder);
    }

    #[test]
    fn add_pre_trade_policy_with_start_hook_to_builder_and_execute_paths() {
        let builder = crate::engine::openpit_create_engine_builder(
            crate::engine::OpenPitSyncPolicy::Full as u8,
            std::ptr::null_mut(),
        );

        let check_name = OpenPitStringView::from_utf8("caller.check.start.add");
        let check_policy = unsafe {
            create_pre_trade_policy_with_start_hook(
                check_name,
                custom_check_fn,
                custom_apply_report_fn,
                custom_free_user_data_fn,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert!(
            !check_policy.is_null(),
            "{}",
            cstr_to_string(std::ptr::null_mut())
        );
        let ok = openpit_engine_builder_add_pre_trade_policy(
            builder,
            check_policy,
            std::ptr::null_mut(),
        );
        assert!(ok, "{}", cstr_to_string(std::ptr::null_mut()));
        openpit_destroy_pretrade_pre_trade_policy(check_policy);

        let engine = crate::engine::openpit_engine_builder_build(builder, std::ptr::null_mut());
        assert!(
            !engine.is_null(),
            "{}",
            cstr_to_string(std::ptr::null_mut())
        );

        let order = OpenPitOrder::default();
        let mut request = std::ptr::null_mut();
        let mut out_rejects = std::ptr::null_mut();
        let status = crate::engine::openpit_engine_start_pre_trade(
            engine,
            &order,
            &mut request,
            &mut out_rejects,
            std::ptr::null_mut(),
        );
        assert_eq!(status, crate::engine::OpenPitPretradeStatus::Passed);
        assert!(out_rejects.is_null());
        crate::engine::openpit_destroy_pretrade_pre_trade_request(request);

        let report = crate::execution_report::OpenPitExecutionReport::default();
        assert!(crate::engine::openpit_engine_apply_execution_report(
            engine,
            &report,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        ));
        crate::engine::openpit_destroy_engine(engine);
        crate::engine::openpit_destroy_engine_builder(builder);
    }

    #[test]
    fn custom_pre_trade_and_account_adjustment_callbacks_are_invoked_via_engine() {
        let builder = crate::engine::openpit_create_engine_builder(
            crate::engine::OpenPitSyncPolicy::Full as u8,
            std::ptr::null_mut(),
        );

        let pre_trade_name = OpenPitStringView::from_utf8("pretrade.invoke");
        let pre_trade_policy = unsafe {
            openpit_create_pretrade_custom_pre_trade_policy(
                pre_trade_name,
                None,
                Some(custom_pre_trade_check_fn),
                Some(custom_apply_report_fn),
                None,
                custom_free_user_data_fn,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert!(
            !pre_trade_policy.is_null(),
            "{}",
            cstr_to_string(std::ptr::null_mut())
        );
        assert!(openpit_engine_builder_add_pre_trade_policy(
            builder,
            pre_trade_policy,
            std::ptr::null_mut()
        ));
        openpit_destroy_pretrade_pre_trade_policy(pre_trade_policy);

        let account_name = OpenPitStringView::from_utf8("account.invoke");
        let account_policy = unsafe {
            create_pre_trade_policy_with_account_adjustment_hook(
                account_name,
                custom_account_adjustment_apply_fn,
                custom_free_user_data_fn,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert!(
            !account_policy.is_null(),
            "{}",
            cstr_to_string(std::ptr::null_mut())
        );
        assert!(openpit_engine_builder_add_pre_trade_policy(
            builder,
            account_policy,
            std::ptr::null_mut()
        ));
        openpit_destroy_pretrade_pre_trade_policy(account_policy);

        let engine = crate::engine::openpit_engine_builder_build(builder, std::ptr::null_mut());
        assert!(
            !engine.is_null(),
            "{}",
            cstr_to_string(std::ptr::null_mut())
        );

        let order = OpenPitOrder::default();
        let mut out_reservation = std::ptr::null_mut();
        let mut out_rejects = std::ptr::null_mut();
        let status = crate::engine::openpit_engine_execute_pre_trade(
            engine,
            &order,
            &mut out_reservation,
            &mut out_rejects,
            std::ptr::null_mut(),
        );
        assert_eq!(status, crate::engine::OpenPitPretradeStatus::Passed);
        assert!(out_rejects.is_null());
        crate::engine::openpit_destroy_pretrade_pre_trade_reservation(out_reservation);

        let report = crate::execution_report::OpenPitExecutionReport::default();
        assert!(crate::engine::openpit_engine_apply_execution_report(
            engine,
            &report,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        ));

        let adjustment = crate::account_adjustment::OpenPitAccountAdjustment::default();
        let batch = [adjustment];
        let mut out_reject = std::ptr::null_mut();
        let status = crate::engine::openpit_engine_apply_account_adjustment(
            engine,
            1,
            batch.as_ptr(),
            batch.len(),
            &mut out_reject,
            std::ptr::null_mut(),
        );
        assert_eq!(
            status,
            crate::account_adjustment::OpenPitAccountAdjustmentApplyStatus::Applied
        );
        assert!(out_reject.is_null());

        crate::engine::openpit_destroy_engine(engine);
        crate::engine::openpit_destroy_engine_builder(builder);
    }

    fn build_engine_with_builtin_start_policy(
        add_fn: impl FnOnce(*mut crate::engine::OpenPitEngineBuilder) -> bool,
    ) -> *mut crate::engine::OpenPitEngine {
        let builder = crate::engine::openpit_create_engine_builder(
            crate::engine::OpenPitSyncPolicy::Full as u8,
            std::ptr::null_mut(),
        );
        assert!(add_fn(builder), "failed to add policy");
        let engine = crate::engine::openpit_engine_builder_build(builder, std::ptr::null_mut());
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
    fn add_builtin_order_validation_policy_happy_path() {
        let engine = build_engine_with_builtin_start_policy(|builder| {
            openpit_engine_builder_add_builtin_order_validation_policy(
                builder,
                std::ptr::null_mut(),
            )
        });
        run_start_pre_trade_passes(engine);
        crate::engine::openpit_destroy_engine(engine);
    }

    #[test]
    fn add_builtin_order_validation_policy_null_builder_reports_error() {
        let mut out_error = std::ptr::null_mut();
        let ok = openpit_engine_builder_add_builtin_order_validation_policy(
            std::ptr::null_mut(),
            &mut out_error,
        );
        assert!(!ok);
        assert_eq!(cstr_to_string(out_error), "engine builder is null");
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
            crate::engine::OpenPitSyncPolicy::Local as u8,
            std::ptr::null_mut(),
        );
        let ok = unsafe {
            openpit_engine_builder_add_builtin_rate_limit_policy(
                builder,
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
        assert!(ok, "add should succeed for Local sync mode");
        let engine = crate::engine::openpit_engine_builder_build(builder, std::ptr::null_mut());
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

    #[test]
    fn add_builtin_pnl_bounds_killswitch_policy_happy_path() {
        let usd = OpenPitStringView::from_utf8("USD");
        let broker = [OpenPitPretradePoliciesPnlBoundsBarrier {
            settlement_asset: usd,
            lower_bound: pnl_optional(Some(pnl_param(-10000, 0))),
            upper_bound: pnl_optional(None),
        }];
        let engine = build_engine_with_builtin_start_policy(|builder| unsafe {
            openpit_engine_builder_add_builtin_pnl_bounds_killswitch_policy(
                builder,
                broker.as_ptr(),
                broker.len(),
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
            )
        });
        run_start_pre_trade_passes(engine);
        crate::engine::openpit_destroy_engine(engine);
    }

    #[test]
    fn add_builtin_pnl_bounds_killswitch_policy_empty_config_reports_error() {
        let builder = crate::engine::openpit_create_engine_builder(
            crate::engine::OpenPitSyncPolicy::Full as u8,
            std::ptr::null_mut(),
        );
        let mut out_error = std::ptr::null_mut();
        let ok = unsafe {
            openpit_engine_builder_add_builtin_pnl_bounds_killswitch_policy(
                builder,
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
            message.contains("pnl_bounds_killswitch_policy creation failed")
                && message.contains("must be configured"),
            "expected SDK no-barrier error wrapped by FFI, got: {message}"
        );
        crate::engine::openpit_destroy_engine_builder(builder);
    }

    #[test]
    fn add_builtin_pnl_bounds_killswitch_null_broker_with_positive_len_reports_error() {
        let builder = crate::engine::openpit_create_engine_builder(
            crate::engine::OpenPitSyncPolicy::Full as u8,
            std::ptr::null_mut(),
        );
        let mut out_error = std::ptr::null_mut();
        let ok = unsafe {
            openpit_engine_builder_add_builtin_pnl_bounds_killswitch_policy(
                builder,
                std::ptr::null(),
                1,
                std::ptr::null(),
                0,
                &mut out_error,
            )
        };
        assert!(!ok);
        assert_eq!(
            cstr_to_string(out_error),
            "pnl_bounds_killswitch_policy broker is null"
        );
        crate::engine::openpit_destroy_engine_builder(builder);
    }

    #[test]
    fn add_builtin_pnl_bounds_killswitch_null_account_with_positive_len_reports_error() {
        let builder = crate::engine::openpit_create_engine_builder(
            crate::engine::OpenPitSyncPolicy::Full as u8,
            std::ptr::null_mut(),
        );
        let mut out_error = std::ptr::null_mut();
        let ok = unsafe {
            openpit_engine_builder_add_builtin_pnl_bounds_killswitch_policy(
                builder,
                std::ptr::null(),
                0,
                std::ptr::null(),
                1,
                &mut out_error,
            )
        };
        assert!(!ok);
        assert_eq!(
            cstr_to_string(out_error),
            "pnl_bounds_killswitch_policy account is null"
        );
        crate::engine::openpit_destroy_engine_builder(builder);
    }
}
