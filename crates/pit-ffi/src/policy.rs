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

use openpit::param::{Asset, Quantity, Volume};
use openpit::pretrade::policies::{
    OrderSizeLimit, OrderSizeLimitPolicy, OrderValidationPolicy, PnlKillSwitchPolicy,
    RateLimitPolicy,
};
use openpit::pretrade::{CheckPreTradeStartPolicy, PreTradeContext, PreTradePolicy, Rejects};
use openpit::{AccountAdjustmentContext, AccountAdjustmentPolicy, Mutation, Mutations};

use crate::account_adjustment::{export_account_adjustment, PitAccountAdjustment};
use crate::execution_report::{export_execution_report, PitExecutionReport};
use crate::order::{export_order, PitOrder};
use crate::reject::PitRejectList;
use crate::PitStringView;
use crate::{AccountAdjustment, ExecutionReport, Order};

use crate::param::{PitParamAccountId, PitParamPnl, PitParamQuantity, PitParamVolume};

use crate::last_error::{write_error, PitOutError};
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
/// - Destroy the caller-owned pointer with the matching
///   `pit_destroy_pretrade_*_policy` function exactly once.
pub struct PolicyHandle<P: ?Sized> {
    policy: Arc<P>,
}

impl<P: ?Sized + GeneralPreTradePolicy> PolicyHandle<P> {
    fn new(policy: Arc<P>) -> *mut Self {
        Box::into_raw(Box::new(Self { policy }))
    }

    fn get_name(&self) -> PitStringView {
        PitStringView::from_utf8(self.policy.name())
    }
}

//--------------------------------------------------------------------------------------------------

/// Opaque pointer for a policy that runs at the start-stage pre-trade check.
///
/// Contract:
/// - Returned by start-stage policy create functions.
/// - May be passed to
///   `pit_engine_builder_add_check_pre_trade_start_policy`.
/// - Must be released by the caller with
///   `pit_destroy_pretrade_check_pre_trade_start_policy` when no longer needed.
pub type PitPretradeCheckPreTradeStartPolicy =
    PolicyHandle<dyn CheckPreTradeStartPolicy<Order, ExecutionReport>>;

/// Opaque pointer for a policy that runs during the main pre-trade check stage.
///
/// Contract:
/// - Returned by main-stage policy create functions.
/// - May be passed to `pit_engine_builder_add_pre_trade_policy`.
/// - Must be released by the caller with
///   `pit_destroy_pretrade_pre_trade_policy` when no longer needed.
pub type PitPretradePreTradePolicy = PolicyHandle<dyn PreTradePolicy<Order, ExecutionReport>>;

/// Opaque pointer for a policy that validates account adjustments.
///
/// Contract:
/// - Returned by account-adjustment policy create functions.
/// - May be passed to
///   `pit_engine_builder_add_account_adjustment_policy`.
/// - Must be released by the caller with
///   `pit_destroy_account_adjustment_policy` when no longer needed.
pub type PitAccountAdjustmentPolicy = PolicyHandle<dyn AccountAdjustmentPolicy<AccountAdjustment>>;

//--------------------------------------------------------------------------------------------------

pub trait GeneralPreTradePolicy {
    fn name(&self) -> &str;
}

impl GeneralPreTradePolicy for dyn CheckPreTradeStartPolicy<Order, ExecutionReport> {
    fn name(&self) -> &str {
        self.name()
    }
}

impl GeneralPreTradePolicy for dyn PreTradePolicy<Order, ExecutionReport> {
    fn name(&self) -> &str {
        self.name()
    }
}

impl GeneralPreTradePolicy for dyn AccountAdjustmentPolicy<AccountAdjustment> {
    fn name(&self) -> &str {
        self.name()
    }
}

//--------------------------------------------------------------------------------------------------

#[no_mangle]
/// Creates a built-in start-stage policy that validates order input shape.
///
/// Why it exists:
/// - Use it to reject structurally invalid orders before deeper checks run.
///
/// Success:
/// - returns a new caller-owned pointer.
/// - this function always succeeds.
///
/// Lifetime contract:
/// - The returned pointer belongs to the caller.
/// - If the pointer is added to the engine builder, the engine keeps its own
///   reference to the same policy object.
/// - The caller must still release its own pointer with
///   `pit_destroy_pretrade_check_pre_trade_start_policy` after the pointer is no
///   longer needed locally.
pub extern "C" fn pit_create_pretrade_policies_order_validation_policy(
) -> *mut PitPretradeCheckPreTradeStartPolicy {
    PitPretradeCheckPreTradeStartPolicy::new(Arc::new(OrderValidationPolicy::new()))
}

#[no_mangle]
/// Creates a built-in start-stage policy that limits how many orders may be
/// accepted within a time window.
///
/// Arguments:
/// - `max_orders`: maximum number of accepted orders allowed in one window.
/// - `window_seconds`: size of the rolling window in seconds.
///
/// Success:
/// - returns a new caller-owned pointer.
/// - this function always succeeds.
///
/// Lifetime contract:
/// - The returned pointer belongs to the caller.
/// - If the pointer is added to the engine builder, the engine keeps its own
///   reference to the same policy object.
/// - The caller must still release its own pointer with
///   `pit_destroy_pretrade_check_pre_trade_start_policy` after the pointer is no
///   longer needed locally.
pub extern "C" fn pit_create_pretrade_policies_rate_limit_policy(
    max_orders: usize,
    window_seconds: u64,
) -> *mut PitPretradeCheckPreTradeStartPolicy {
    PitPretradeCheckPreTradeStartPolicy::new(Arc::new(RateLimitPolicy::new(
        max_orders,
        Duration::from_secs(window_seconds),
    )))
}

/// One barrier definition for `pit_create_pretrade_policies_pnl_killswitch_policy`.
///
/// What it describes:
/// - A settlement asset and the loss threshold attached to it.
///
/// Contract:
/// - `settlement_asset` must point to a valid, null-terminated string for the
///   duration of the call.
/// - `barrier` must contain a valid PnL threshold value.
/// - The array passed to the create function may contain multiple entries.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PitPretradePoliciesPnlKillSwitchParam {
    /// Settlement asset whose accumulated loss is being monitored.
    pub settlement_asset: PitStringView,
    /// Loss barrier for that settlement asset.
    pub barrier: PitParamPnl,
}

#[no_mangle]
/// Creates a built-in start-stage policy that rejects new orders once a loss
/// threshold is reached.
///
/// Why it exists:
/// - Use it as a kill switch per settlement asset.
///
/// Arguments:
/// - `params`: pointer to an array of barrier definitions.
/// - `params_len`: number of elements in `params`.
///
/// Contract:
/// - `params` must point to `params_len` readable entries.
/// - `params_len` must be greater than zero.
/// - Each `settlement_asset` pointer inside `params` must be a valid
///   null-terminated string for the duration of the call.
///
/// Success:
/// - returns a new caller-owned policy object.
///
/// Error:
/// - returns null when arguments are invalid or the policy cannot be created;
/// - if `out_error` is not null, writes a caller-owned `PitSharedString`
///   error handle that MUST be released with `pit_destroy_shared_string`.
///
/// Lifetime contract:
/// - The returned pointer belongs to the caller.
/// - If the pointer is added to the engine builder, the engine keeps its own
///   reference to the same policy object.
/// - The caller must still release its own pointer with
///   `pit_destroy_pretrade_check_pre_trade_start_policy` after the pointer is no
///   longer needed locally.
pub unsafe extern "C" fn pit_create_pretrade_policies_pnl_killswitch_policy(
    params: *const PitPretradePoliciesPnlKillSwitchParam,
    params_len: usize,
    out_error: PitOutError,
) -> *mut PitPretradeCheckPreTradeStartPolicy {
    if params_len == 0 {
        write_error(
            out_error,
            "pnl_killswitch_policy requires at least one barrier",
        );
        return std::ptr::null_mut();
    }
    if params.is_null() {
        write_error(out_error, "pnl_killswitch_policy params is null");
        return std::ptr::null_mut();
    }

    let params = unsafe { std::slice::from_raw_parts(params, params_len) };
    let mut barriers = Vec::with_capacity(params.len());
    for (index, param) in params.iter().enumerate() {
        let settlement: Asset = match cstr_arg(param.settlement_asset) {
            Some(s) => match Asset::new(s) {
                Ok(v) => v,
                Err(e) => {
                    write_error_format!(
                        out_error,
                        "param[{index}] settlement asset is invalid: {}",
                        e
                    );
                    return std::ptr::null_mut();
                }
            },
            None => {
                write_error_format!(out_error, "param[{}] settlement asset is not set", index);
                return std::ptr::null_mut();
            }
        };

        let barrier = match param.barrier.to_param() {
            Ok(v) => v,
            Err(e) => {
                write_error_format!(out_error, "param[{index}] barrier is invalid: {}", e);
                return std::ptr::null_mut();
            }
        };
        barriers.push((settlement, barrier));
    }

    let (initial_barrier, additional_barriers) = match barriers.split_first() {
        Some(v) => v,
        None => {
            write_error(out_error, "required at least one barrier");
            return std::ptr::null_mut();
        }
    };

    let policy = match PnlKillSwitchPolicy::new(
        initial_barrier.clone(),
        additional_barriers.iter().cloned(),
    ) {
        Ok(v) => v,
        Err(e) => {
            write_error_format!(out_error, "policy creation failed: {}", e);
            return std::ptr::null_mut();
        }
    };

    PitPretradeCheckPreTradeStartPolicy::new(Arc::new(policy))
}

/// One limit definition for `pit_create_pretrade_policies_order_size_limit_policy`.
///
/// What it describes:
/// - Per-settlement maximum quantity and maximum notional allowed for one order.
///
/// Contract:
/// - `settlement_asset` must point to a valid, null-terminated string for the
///   duration of the call.
/// - `max_quantity` and `max_notional` must contain valid limit values.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PitPretradePoliciesOrderSizeLimitParam {
    /// Settlement asset to which the limits apply.
    pub settlement_asset: PitStringView,
    /// Maximum allowed quantity for one order.
    pub max_quantity: PitParamQuantity,
    /// Maximum allowed notional for one order.
    pub max_notional: PitParamVolume,
}

#[no_mangle]
/// Creates a built-in start-stage policy that rejects orders above configured
/// size limits.
///
/// Why it exists:
/// - Use it to cap order quantity and notional per settlement asset.
///
/// Arguments:
/// - `params`: pointer to an array of size-limit definitions.
/// - `params_len`: number of elements in `params`.
///
/// Contract:
/// - `params` must point to `params_len` readable entries.
/// - `params_len` must be greater than zero.
/// - Each `settlement_asset` pointer inside `params` must be a valid
///   null-terminated string for the duration of the call.
///
/// Success:
/// - returns a new caller-owned policy object.
///
/// Error:
/// - returns null when arguments are invalid;
/// - if `out_error` is not null, writes a caller-owned `PitSharedString`
///   error handle that MUST be released with `pit_destroy_shared_string`.
///
/// Lifetime contract:
/// - The returned pointer belongs to the caller.
/// - If the pointer is added to the engine builder, the engine keeps its own
///   reference to the same policy object.
/// - The caller must still release its own pointer with
///   `pit_destroy_pretrade_check_pre_trade_start_policy` after the pointer is no
///   longer needed locally.
pub unsafe extern "C" fn pit_create_pretrade_policies_order_size_limit_policy(
    params: *const PitPretradePoliciesOrderSizeLimitParam,
    params_len: usize,
    out_error: PitOutError,
) -> *mut PitPretradeCheckPreTradeStartPolicy {
    if params_len == 0 {
        write_error(
            out_error,
            "order_size_limit_policy requires at least one limit",
        );
        return std::ptr::null_mut();
    }
    if params.is_null() {
        write_error(out_error, "order_size_limit_policy params is null");
        return std::ptr::null_mut();
    }

    let params = unsafe { std::slice::from_raw_parts(params, params_len) };
    let mut limits = Vec::with_capacity(params.len());
    for (index, param) in params.iter().enumerate() {
        let settlement_asset: Asset = match cstr_arg(param.settlement_asset) {
            Some(s) => match Asset::new(s) {
                Ok(v) => v,
                Err(e) => {
                    write_error_format!(
                        out_error,
                        "param[{index}] settlement asset is invalid: {}",
                        e
                    );
                    return std::ptr::null_mut();
                }
            },
            None => {
                write_error_format!(out_error, "param[{}] settlement asset is not set", index);
                return std::ptr::null_mut();
            }
        };

        let max_quantity: Quantity = match param.max_quantity.to_param() {
            Ok(v) => v,
            Err(e) => {
                write_error_format!(out_error, "param[{index}] max quantity is invalid: {}", e);
                return std::ptr::null_mut();
            }
        };

        let max_notional: Volume = match param.max_notional.to_param() {
            Ok(v) => v,
            Err(e) => {
                write_error_format!(out_error, "param[{index}] max notional is invalid: {}", e);
                return std::ptr::null_mut();
            }
        };

        limits.push(OrderSizeLimit {
            settlement_asset,
            max_quantity,
            max_notional,
        });
    }

    let (initial_limit, additional_limits) = match limits.split_first() {
        Some(v) => v,
        None => {
            write_error(out_error, "required at least one limit");
            return std::ptr::null_mut();
        }
    };
    let policy =
        OrderSizeLimitPolicy::new(initial_limit.clone(), additional_limits.iter().cloned());

    PitPretradeCheckPreTradeStartPolicy::new(Arc::new(policy))
}

//--------------------------------------------------------------------------------------------------

macro_rules! policy_destroy_fn {
    ($(#[$meta:meta])* $fn_name:ident, $handle_ty:ty) => {
        $(#[$meta])*
        #[no_mangle]
        pub extern "C" fn $fn_name(policy: *mut $handle_ty) {
            if policy.is_null() {
                return;
            }
            unsafe { drop(Box::from_raw(policy)) };
        }
    };
}

policy_destroy_fn!(
    /// Destroys the caller-owned pointer for a start-stage policy.
    ///
    /// Lifetime contract:
    /// - Call this exactly once for each pointer that was returned to the caller
    ///   by a start-stage policy create function.
    /// - After this call the pointer is no longer valid.
    /// - Passing a null pointer is allowed and has no effect.
    /// - This function always succeeds.
    /// - If the policy was previously added to the engine builder, the engine
    ///   keeps its own reference and may continue using the policy.
    /// - Destroying this caller-owned pointer does not remove the policy from
    ///   the engine.
    pit_destroy_pretrade_check_pre_trade_start_policy,
    PitPretradeCheckPreTradeStartPolicy
);

policy_destroy_fn!(
    /// Destroys the caller-owned pointer for a main-stage policy.
    ///
    /// Lifetime contract:
    /// - Call this exactly once for each pointer that was returned to the caller
    ///   by a main-stage policy create function.
    /// - After this call the pointer is no longer valid.
    /// - Passing a null pointer is allowed and has no effect.
    /// - This function always succeeds.
    /// - If the policy was previously added to the engine builder, the engine
    ///   keeps its own reference and may continue using the policy.
    /// - Destroying this caller-owned pointer does not remove the policy from
    ///   the engine.
    pit_destroy_pretrade_pre_trade_policy,
    PitPretradePreTradePolicy
);

policy_destroy_fn!(
    /// Destroys the caller-owned pointer for an account-adjustment policy.
    ///
    /// Lifetime contract:
    /// - Call this exactly once for each pointer that was returned to the caller
    ///   by an account-adjustment policy create function.
    /// - After this call the pointer is no longer valid.
    /// - Passing a null pointer is allowed and has no effect.
    /// - This function always succeeds.
    /// - If the policy was previously added to the engine builder, the engine
    ///   keeps its own reference and may continue using the policy.
    /// - Destroying this caller-owned pointer does not remove the policy from
    ///   the engine.
    pit_destroy_account_adjustment_policy,
    PitAccountAdjustmentPolicy
);

//--------------------------------------------------------------------------------------------------

macro_rules! policy_get_name_fn {
    ($(#[$meta:meta])* $fn_name:ident, $handle_ty:ty) => {
        $(#[$meta])*
        #[no_mangle]
        pub extern "C" fn $fn_name(policy: *const $handle_ty) -> PitStringView {
            assert!(!policy.is_null());
            unsafe { (&*policy).get_name() }
        }
    };
}

policy_get_name_fn!(
    /// Returns the stable policy name for a start-stage policy pointer.
    ///
    /// Contract:
    /// - This function never fails.
    /// - `policy` must be a valid non-null pointer.
    /// - The returned view does not own memory.
    /// - The view remains valid while the policy object is alive and its name
    ///   is not changed.
    /// - Passing an invalid pointer aborts the call.
    pit_pretrade_check_pre_trade_start_policy_get_name,
    PitPretradeCheckPreTradeStartPolicy
);

policy_get_name_fn!(
    /// Returns the stable policy name for a main-stage policy pointer.
    ///
    /// Contract:
    /// - This function never fails.
    /// - `policy` must be a valid non-null pointer.
    /// - The returned view does not own memory.
    /// - The view remains valid while the policy object is alive and its name
    ///   is not changed.
    /// - Passing an invalid pointer aborts the call.
    pit_pretrade_pre_trade_policy_get_name,
    PitPretradePreTradePolicy
);

policy_get_name_fn!(
    /// Returns the stable policy name for an account-adjustment policy pointer.
    ///
    /// Contract:
    /// - This function never fails.
    /// - `policy` must be a valid non-null pointer.
    /// - The returned view does not own memory.
    /// - The view remains valid while the policy object is alive and its name
    ///   is not changed.
    /// - Passing an invalid pointer aborts the call.
    pit_account_adjustment_policy_get_name,
    PitAccountAdjustmentPolicy
);

//--------------------------------------------------------------------------------------------------

fn add_policy_to_builder<P: ?Sized, F>(
    builder: *mut crate::engine::PitEngineBuilder,
    policy: *mut PolicyHandle<P>,
    add_fn: F,
) -> Result<(), String>
where
    F: FnOnce(
        openpit::EngineBuilder<Order, ExecutionReport, AccountAdjustment>,
        Arc<P>,
    ) -> openpit::EngineBuilder<Order, ExecutionReport, AccountAdjustment>,
{
    if builder.is_null() {
        return Err("engine builder is null".to_string());
    }
    if policy.is_null() {
        return Err("policy is null".to_string());
    }
    let pointer = unsafe { &*policy };
    let policy = Arc::clone(&pointer.policy);
    crate::engine::with_builder(
        unsafe { &mut *builder },
        Box::new(move |b| add_fn(b, policy)),
    )?;
    Ok(())
}

#[no_mangle]
/// Adds a start-stage policy to the engine builder.
///
/// Why it exists:
/// - Registers a policy that runs before the main pre-trade stage.
///
/// Contract:
/// - `builder` must be a valid engine builder pointer.
/// - `policy` must be a valid non-null start-stage policy pointer.
///
/// Success:
/// - returns `true` and the builder retains its own reference to the policy.
///
/// Error:
/// - returns `false` when the builder or policy cannot be used;
/// - if `out_error` is not null, writes a caller-owned `PitSharedString`
///   error handle that MUST be released with `pit_destroy_shared_string`.
///
/// Lifetime contract:
/// - The engine builder retains its own reference to the policy object.
/// - The caller still owns the passed pointer and must release that local pointer
///   separately with `pit_destroy_pretrade_check_pre_trade_start_policy` when
///   it is no longer needed.
pub extern "C" fn pit_engine_builder_add_check_pre_trade_start_policy(
    builder: *mut crate::engine::PitEngineBuilder,
    policy: *mut PitPretradeCheckPreTradeStartPolicy,
    out_error: PitOutError,
) -> bool {
    match add_policy_to_builder(builder, policy, |b, policy| {
        b.check_pre_trade_start_policy(DynCheckPreTradeStartPolicy { inner: policy })
    }) {
        Ok(()) => true,
        Err(err) => {
            write_error(out_error, &err);
            false
        }
    }
}

#[no_mangle]
/// Adds a main-stage pre-trade policy to the engine builder.
///
/// Contract:
/// - `builder` must be a valid engine builder pointer.
/// - `policy` must be a valid non-null main-stage policy pointer.
///
/// Success:
/// - returns `true` and the builder retains its own reference to the policy.
///
/// Error:
/// - returns `false` when the builder or policy cannot be used;
/// - if `out_error` is not null, writes a caller-owned `PitSharedString`
///   error handle that MUST be released with `pit_destroy_shared_string`.
///
/// Lifetime contract:
/// - The engine builder retains its own reference to the policy object.
/// - The caller still owns the passed pointer and must release that local pointer
///   separately with `pit_destroy_pretrade_pre_trade_policy` when it is no
///   longer needed.
pub extern "C" fn pit_engine_builder_add_pre_trade_policy(
    builder: *mut crate::engine::PitEngineBuilder,
    policy: *mut PitPretradePreTradePolicy,
    out_error: PitOutError,
) -> bool {
    match add_policy_to_builder(builder, policy, |b, policy| {
        b.pre_trade_policy(DynPreTradePolicy { inner: policy })
    }) {
        Ok(()) => true,
        Err(err) => {
            write_error(out_error, &err);
            false
        }
    }
}

#[no_mangle]
/// Adds an account-adjustment policy to the engine builder.
///
/// Contract:
/// - `builder` must be a valid engine builder pointer.
/// - `policy` must be a valid non-null account-adjustment policy pointer.
///
/// Success:
/// - returns `true` and the builder retains its own reference to the policy.
///
/// Error:
/// - returns `false` when the builder or policy cannot be used;
/// - if `out_error` is not null, writes a caller-owned `PitSharedString`
///   error handle that MUST be released with `pit_destroy_shared_string`.
///
/// Lifetime contract:
/// - The engine builder retains its own reference to the policy object.
/// - The caller still owns the passed pointer and must release that local pointer
///   separately with `pit_destroy_account_adjustment_policy` when it
///   is no longer needed.
pub extern "C" fn pit_engine_builder_add_account_adjustment_policy(
    builder: *mut crate::engine::PitEngineBuilder,
    policy: *mut PitAccountAdjustmentPolicy,
    out_error: PitOutError,
) -> bool {
    match add_policy_to_builder(builder, policy, |b, policy| {
        b.account_adjustment_policy(DynAccountAdjustmentPolicy { inner: policy })
    }) {
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
pub struct PitPretradeContext;

/// Opaque context passed to account-adjustment C policy callbacks.
///
/// Valid only for the duration of the callback. Cannot be constructed by
/// caller code.
///
/// Future extension: this type is the designated seam for engine
/// storage-cell access. A read accessor will be added here when the engine
/// store is introduced.
pub struct PitAccountAdjustmentContext;

/// Opaque, non-owning pointer to the mutation collector.
///
/// Valid only during the policy callback that received it.
/// The caller must not store or use this pointer after the callback returns.
pub struct PitMutations {
    mutations: *mut Mutations,
}

/// Callback invoked for either commit or rollback of a registered mutation.
pub type PitMutationFn = unsafe extern "C" fn(user_data: *mut c_void);

/// Optional callback to release mutation user_data after execution.
///
/// Called exactly once per `pit_mutations_push`:
/// - after `commit_fn` when commit runs;
/// - after `rollback_fn` when rollback runs;
/// - or on drop if neither action ran.
pub type PitMutationFreeFn = unsafe extern "C" fn(user_data: *mut c_void);

struct FfiMutationGuard {
    user_data: *mut c_void,
    free_fn: Option<PitMutationFreeFn>,
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
/// - if `out_error` is not null, writes a caller-owned `PitSharedString`
///   error handle that MUST be released with `pit_destroy_shared_string`.
pub unsafe extern "C" fn pit_mutations_push(
    mutations: *mut PitMutations,
    commit_fn: PitMutationFn,
    rollback_fn: PitMutationFn,
    user_data: *mut c_void,
    free_fn: Option<PitMutationFreeFn>,
    out_error: PitOutError,
) -> bool {
    if mutations.is_null() {
        write_error(out_error, "pit_mutations_push: mutations is null");
        return false;
    }

    let raw_mutations = unsafe { (*mutations).mutations };
    if raw_mutations.is_null() {
        write_error(out_error, "pit_mutations_push: inner mutations is null");
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

/// Callback used by a custom start-stage policy to validate one order.
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
///   `pit_create_reject_list`.
/// - Every reject payload is copied into internal storage before the callback
///   returns.
/// - `user_data` is passed through unchanged from policy creation.
pub type PitPretradeCheckPreTradeStartPolicyCheckPreTradeStartFn =
    unsafe extern "C" fn(
        ctx: *const PitPretradeContext,
        order: *const PitOrder,
        user_data: *mut c_void,
    ) -> *mut PitRejectList;

/// Callback used by a custom start-stage policy to observe an execution report.
///
/// Contract:
/// - `report` points to a read-only report view valid only for the duration of
///   the callback.
/// - `report` is passed as a borrowed view and is not copied before the
///   callback runs.
/// - If the callback wants to keep any data from `report`, it must copy that
///   data before returning.
/// - Return `true` if the policy state changed and the engine should keep the
///   update.
/// - Return `false` when nothing changed.
/// - `user_data` is passed through unchanged from policy creation.
pub type PitPretradeCheckPreTradeStartPolicyApplyExecutionReportFn =
    unsafe extern "C" fn(report: *const PitExecutionReport, user_data: *mut c_void) -> bool;

/// Callback invoked when the last reference to a custom start-stage policy is
/// released and the policy object is about to be destroyed.
///
/// Contract:
/// - Called exactly once, on the thread that drops the last policy reference.
/// - After this callback returns, no further callbacks will be invoked for
///   this policy instance.
/// - `user_data` is the same value that was passed at policy creation.
/// - The callback must release any resources associated with `user_data`.
pub type PitPretradeCheckPreTradeStartPolicyFreeUserDataFn =
    unsafe extern "C" fn(user_data: *mut c_void);

/// Callback used by a custom main-stage policy to perform a pre-trade check.
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
///   `pit_create_reject_list`.
/// - Every reject payload is copied into internal storage before this callback
///   returns.
/// - `user_data` is passed through unchanged from policy creation.
pub type PitPretradePreTradePolicyCheckFn = unsafe extern "C" fn(
    ctx: *const PitPretradeContext,
    order: *const PitOrder,
    mutations: *mut PitMutations,
    user_data: *mut c_void,
) -> *mut PitRejectList;

/// Callback used by a custom main-stage policy to observe an execution report.
///
/// Contract:
/// - `report` points to a read-only report view valid only for the duration of
///   the callback.
/// - `report` is passed as a borrowed view and is not copied before the
///   callback runs.
/// - If the callback wants to keep any data from `report`, it must copy that
///   data before returning.
/// - Return `true` if the policy state changed and the engine should keep the
///   update.
/// - Return `false` when nothing changed.
/// - `user_data` is passed through unchanged from policy creation.
pub type PitPretradePreTradePolicyApplyExecutionReportFn =
    unsafe extern "C" fn(report: *const PitExecutionReport, user_data: *mut c_void) -> bool;

/// Callback invoked when the last reference to a custom main-stage policy is
/// released and the policy object is about to be destroyed.
///
/// Contract:
/// - Called exactly once, on the thread that drops the last policy reference.
/// - After this callback returns, no further callbacks will be invoked for
///   this policy instance.
/// - `user_data` is the same value that was passed at policy creation.
/// - The callback must release any resources associated with `user_data`.
pub type PitPretradePreTradePolicyFreeUserDataFn = unsafe extern "C" fn(user_data: *mut c_void);

/// Callback used by a custom account-adjustment policy to validate one
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
pub type PitAccountAdjustmentPolicyApplyFn = unsafe extern "C" fn(
    ctx: *const PitAccountAdjustmentContext,
    account_id: PitParamAccountId,
    adjustment: *const PitAccountAdjustment,
    mutations: *mut PitMutations,
    user_data: *mut c_void,
) -> *mut PitRejectList;

/// Callback invoked when the last reference to a custom account-adjustment
/// policy is released and the policy object is about to be destroyed.
///
/// Contract:
/// - Called exactly once, on the thread that drops the last policy reference.
/// - After this callback returns, no further callbacks will be invoked for
///   this policy instance.
/// - `user_data` is the same value that was passed at policy creation.
/// - The callback must release any resources associated with `user_data`.
pub type PitAccountAdjustmentPolicyFreeUserDataFn = unsafe extern "C" fn(user_data: *mut c_void);

//--------------------------------------------------------------------------------------------------

struct DynCheckPreTradeStartPolicy {
    inner: Arc<dyn CheckPreTradeStartPolicy<Order, ExecutionReport>>,
}

impl CheckPreTradeStartPolicy<Order, ExecutionReport> for DynCheckPreTradeStartPolicy {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn check_pre_trade_start(&self, ctx: &PreTradeContext, order: &Order) -> Result<(), Rejects> {
        self.inner.check_pre_trade_start(ctx, order)
    }

    fn apply_execution_report(&self, report: &ExecutionReport) -> bool {
        self.inner.apply_execution_report(report)
    }
}

struct DynPreTradePolicy {
    inner: Arc<dyn PreTradePolicy<Order, ExecutionReport>>,
}

impl PreTradePolicy<Order, ExecutionReport> for DynPreTradePolicy {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn perform_pre_trade_check(
        &self,
        ctx: &PreTradeContext,
        order: &Order,
        mutations: &mut Mutations,
    ) -> Result<(), Rejects> {
        self.inner.perform_pre_trade_check(ctx, order, mutations)
    }

    fn apply_execution_report(&self, report: &ExecutionReport) -> bool {
        self.inner.apply_execution_report(report)
    }
}

struct DynAccountAdjustmentPolicy {
    inner: Arc<dyn AccountAdjustmentPolicy<AccountAdjustment>>,
}

impl AccountAdjustmentPolicy<AccountAdjustment> for DynAccountAdjustmentPolicy {
    fn name(&self) -> &str {
        self.inner.name()
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

struct CustomCheckPreTradeStartPolicy {
    name: String,
    check_fn: PitPretradeCheckPreTradeStartPolicyCheckPreTradeStartFn,
    apply_execution_report_fn: PitPretradeCheckPreTradeStartPolicyApplyExecutionReportFn,
    free_user_data_fn: PitPretradeCheckPreTradeStartPolicyFreeUserDataFn,
    user_data: *mut c_void,
}

impl CheckPreTradeStartPolicy<Order, ExecutionReport> for CustomCheckPreTradeStartPolicy {
    fn name(&self) -> &str {
        &self.name
    }

    fn check_pre_trade_start(&self, ctx: &PreTradeContext, order: &Order) -> Result<(), Rejects> {
        let input = export_order(order);
        let c_ctx = (ctx as *const PreTradeContext).cast::<PitPretradeContext>();
        let rejects = unsafe { (self.check_fn)(c_ctx, &input, self.user_data) };
        import_reject_list_result(rejects)
    }

    fn apply_execution_report(&self, report: &ExecutionReport) -> bool {
        let input = export_execution_report(report);
        unsafe { (self.apply_execution_report_fn)(&input, self.user_data) }
    }
}

impl Drop for CustomCheckPreTradeStartPolicy {
    fn drop(&mut self) {
        unsafe { (self.free_user_data_fn)(self.user_data) };
    }
}

struct CustomPreTradePolicy {
    name: String,
    check_fn: PitPretradePreTradePolicyCheckFn,
    apply_execution_report_fn: PitPretradePreTradePolicyApplyExecutionReportFn,
    free_user_data_fn: PitPretradePreTradePolicyFreeUserDataFn,
    user_data: *mut c_void,
}

impl PreTradePolicy<Order, ExecutionReport> for CustomPreTradePolicy {
    fn name(&self) -> &str {
        &self.name
    }

    fn perform_pre_trade_check(
        &self,
        ctx: &PreTradeContext,
        order: &Order,
        mutations: &mut Mutations,
    ) -> Result<(), Rejects> {
        let mut mutations_handle = PitMutations {
            mutations: mutations as *mut Mutations,
        };
        let input = export_order(order);
        let c_ctx = (ctx as *const PreTradeContext).cast::<PitPretradeContext>();
        let rejects =
            unsafe { (self.check_fn)(c_ctx, &input, &mut mutations_handle, self.user_data) };
        import_reject_list_result(rejects)
    }

    fn apply_execution_report(&self, report: &ExecutionReport) -> bool {
        let input = export_execution_report(report);
        unsafe { (self.apply_execution_report_fn)(&input, self.user_data) }
    }
}

impl Drop for CustomPreTradePolicy {
    fn drop(&mut self) {
        unsafe { (self.free_user_data_fn)(self.user_data) };
    }
}

struct CustomAccountAdjustmentPolicy {
    name: String,
    apply_fn: PitAccountAdjustmentPolicyApplyFn,
    free_user_data_fn: PitAccountAdjustmentPolicyFreeUserDataFn,
    user_data: *mut c_void,
}

impl AccountAdjustmentPolicy<AccountAdjustment> for CustomAccountAdjustmentPolicy {
    fn name(&self) -> &str {
        &self.name
    }

    fn apply_account_adjustment(
        &self,
        _ctx: &AccountAdjustmentContext,
        account_id: openpit::param::AccountId,
        adjustment: &AccountAdjustment,
        mutations: &mut Mutations,
    ) -> Result<(), Rejects> {
        let mut mutations_handle = PitMutations {
            mutations: mutations as *mut Mutations,
        };
        let input = export_account_adjustment(adjustment);
        let c_ctx = (_ctx as *const AccountAdjustmentContext).cast::<PitAccountAdjustmentContext>();
        let rejects = unsafe {
            (self.apply_fn)(
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

impl Drop for CustomAccountAdjustmentPolicy {
    fn drop(&mut self) {
        unsafe { (self.free_user_data_fn)(self.user_data) };
    }
}

//--------------------------------------------------------------------------------------------------

unsafe fn parse_policy_name(name_ptr: PitStringView, out_error: PitOutError) -> Option<String> {
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

fn import_reject_list_result(rejects: *mut PitRejectList) -> Result<(), Rejects> {
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
/// Creates a custom start-stage policy from caller-provided callbacks.
///
/// Why it exists:
/// - Lets the caller implement policy logic outside the engine and plug it into
///   the same builder flow as built-in policies.
///
/// Contract:
/// - `name` must point to a valid, null-terminated string for the duration of
///   the call.
/// - `check_fn`, `apply_fn`, and `free_user_data_fn` must remain callable for
///   as long as the policy may still be used by either the caller pointer or
///   the engine.
/// - `free_user_data_fn` will be called exactly once, when the last reference
///   to the policy is released.
/// - `user_data` is stored as-is and passed back to every callback invocation.
///
/// Success:
/// - returns a new caller-owned policy object.
///
/// Error:
/// - returns null when `name` is invalid;
/// - if `out_error` is not null, writes a caller-owned `PitSharedString`
///   error handle that MUST be released with `pit_destroy_shared_string`.
///
/// Lifetime contract:
/// - The policy stores its own copy of `name`; the caller may release the input
///   string after this function returns.
/// - The returned pointer is owned by the caller and must be released with
///   `pit_destroy_pretrade_check_pre_trade_start_policy` when no longer needed.
/// - If the policy is added to the engine builder, the engine keeps its own
///   reference, but the caller must still release the caller-owned pointer.
/// - `free_user_data_fn` runs once the last reference to the policy is
///   released; when the engine is the final holder, it runs as part of engine
///   destruction.
pub unsafe extern "C" fn pit_create_pretrade_custom_check_pre_trade_start_policy(
    name: PitStringView,
    check_fn: PitPretradeCheckPreTradeStartPolicyCheckPreTradeStartFn,
    apply_execution_report_fn: PitPretradeCheckPreTradeStartPolicyApplyExecutionReportFn,
    free_user_data_fn: PitPretradeCheckPreTradeStartPolicyFreeUserDataFn,
    user_data: *mut c_void,
    out_error: PitOutError,
) -> *mut PitPretradeCheckPreTradeStartPolicy {
    let name = match unsafe { parse_policy_name(name, out_error) } {
        Some(v) => v,
        None => return std::ptr::null_mut(),
    };

    let policy = CustomCheckPreTradeStartPolicy {
        name,
        check_fn,
        apply_execution_report_fn,
        free_user_data_fn,
        user_data,
    };

    PitPretradeCheckPreTradeStartPolicy::new(Arc::new(policy))
}

#[no_mangle]
/// Creates a custom main-stage pre-trade policy from caller-provided callbacks.
///
/// Contract:
/// - `name` must point to a valid, null-terminated string for the duration of
///   the call.
/// - `check_fn`, `apply_fn`, and `free_user_data_fn` must
///   remain callable for as long as the policy may still be used by either the
///   caller pointer or the engine.
/// - Custom policy callbacks can register commit/rollback mutations through the
///   mutations pointer passed to `check_fn`.
/// - `free_user_data_fn` will be called exactly once, when the last reference
///   to the policy is released.
/// - `user_data` is stored as-is and passed back to every callback invocation.
///
/// Success:
/// - returns a new caller-owned policy object.
///
/// Error:
/// - returns null when `name` is invalid;
/// - if `out_error` is not null, writes a caller-owned `PitSharedString`
///   error handle that MUST be released with `pit_destroy_shared_string`.
///
/// Lifetime contract:
/// - The policy stores its own copy of `name`; the caller may release the input
///   string after this function returns.
/// - The returned pointer is owned by the caller and must be released with
///   `pit_destroy_pretrade_pre_trade_policy` when no longer needed.
/// - If the policy is added to the engine builder, the engine keeps its own
///   reference, but the caller must still release the caller-owned pointer.
/// - `free_user_data_fn` runs once the last reference to the policy is
///   released; when the engine is the final holder, it runs as part of engine
///   destruction.
pub unsafe extern "C" fn pit_create_pretrade_custom_pre_trade_policy(
    name: PitStringView,
    check_fn: PitPretradePreTradePolicyCheckFn,
    apply_fn: PitPretradePreTradePolicyApplyExecutionReportFn,
    free_user_data_fn: PitPretradePreTradePolicyFreeUserDataFn,
    user_data: *mut c_void,
    out_error: PitOutError,
) -> *mut PitPretradePreTradePolicy {
    let name = match unsafe { parse_policy_name(name, out_error) } {
        Some(v) => v,
        None => return std::ptr::null_mut(),
    };

    let policy = CustomPreTradePolicy {
        name,
        check_fn,
        apply_execution_report_fn: apply_fn,
        free_user_data_fn,
        user_data,
    };

    PitPretradePreTradePolicy::new(Arc::new(policy))
}

#[no_mangle]
/// Creates a custom account-adjustment policy from caller-provided callbacks.
///
/// Contract:
/// - `name` must point to a valid, null-terminated string for the duration of
///   the call.
/// - `apply_fn` and `free_user_data_fn` must remain callable for as long as
///   the policy may still be used by either the caller pointer or the engine.
/// - Custom policy callbacks can register commit/rollback mutations through the
///   mutations pointer passed to `apply_fn`.
/// - `free_user_data_fn` will be called exactly once, when the last reference
///   to the policy is released.
/// - `user_data` is stored as-is and passed back to every callback invocation.
///
/// Success:
/// - returns a new caller-owned policy object.
///
/// Error:
/// - returns null when `name` is invalid;
/// - if `out_error` is not null, writes a caller-owned `PitSharedString`
///   error handle that MUST be released with `pit_destroy_shared_string`.
///
/// Lifetime contract:
/// - The policy stores its own copy of `name`; the caller may release the input
///   string after this function returns.
/// - The returned pointer is owned by the caller and must be released with
///   `pit_destroy_account_adjustment_policy` when no longer needed.
/// - If the policy is added to the engine builder, the engine keeps its own
///   reference, but the caller must still release the caller-owned pointer.
/// - `free_user_data_fn` runs once the last reference to the policy is
///   released; when the engine is the final holder, it runs as part of engine
///   destruction.
pub unsafe extern "C" fn pit_create_custom_account_adjustment_policy(
    name: PitStringView,
    apply_fn: PitAccountAdjustmentPolicyApplyFn,
    free_user_data_fn: PitAccountAdjustmentPolicyFreeUserDataFn,
    user_data: *mut c_void,
    out_error: PitOutError,
) -> *mut PitAccountAdjustmentPolicy {
    let name = match unsafe { parse_policy_name(name, out_error) } {
        Some(v) => v,
        None => return std::ptr::null_mut(),
    };

    let policy = CustomAccountAdjustmentPolicy {
        name,
        apply_fn,
        free_user_data_fn,
        user_data,
    };

    PitAccountAdjustmentPolicy::new(Arc::new(policy))
}

//--------------------------------------------------------------------------------------------------

unsafe fn cstr_arg(ptr: PitStringView) -> Option<String> {
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
    use pit_interop::{OrderOperationAccess, PopulatedOrderOperation};

    use super::*;

    use crate::param::PitParamDecimal;
    use crate::reject::PitRejectList;

    unsafe extern "C" fn custom_check_fn(
        _ctx: *const PitPretradeContext,
        _order: *const PitOrder,
        _user_data: *mut c_void,
    ) -> *mut PitRejectList {
        std::ptr::null_mut()
    }

    unsafe extern "C" fn custom_apply_report_fn(
        _report: *const PitExecutionReport,
        _user_data: *mut c_void,
    ) -> bool {
        false
    }

    unsafe extern "C" fn custom_free_user_data_fn(_user_data: *mut c_void) {}
    unsafe extern "C" fn custom_pre_trade_check_fn(
        _ctx: *const PitPretradeContext,
        _order: *const PitOrder,
        _mutations: *mut PitMutations,
        _user_data: *mut c_void,
    ) -> *mut PitRejectList {
        std::ptr::null_mut()
    }

    unsafe extern "C" fn custom_account_adjustment_apply_fn(
        _ctx: *const PitAccountAdjustmentContext,
        _account_id: PitParamAccountId,
        _adjustment: *const PitAccountAdjustment,
        _mutations: *mut PitMutations,
        _user_data: *mut c_void,
    ) -> *mut PitRejectList {
        std::ptr::null_mut()
    }

    fn cstr_to_string(handle: *mut crate::string::PitSharedString) -> String {
        if handle.is_null() {
            return String::new();
        }
        let view = crate::string::pit_shared_string_view(handle);
        let result = if view.ptr.is_null() {
            String::new()
        } else {
            let bytes = unsafe { std::slice::from_raw_parts(view.ptr, view.len) };
            std::str::from_utf8(bytes).expect("utf8").to_string()
        };
        crate::string::pit_destroy_shared_string(handle);
        result
    }

    fn string_view_to_string(view: PitStringView) -> String {
        if view.ptr.is_null() {
            return String::new();
        }
        let bytes = unsafe { std::slice::from_raw_parts(view.ptr, view.len) };
        std::str::from_utf8(bytes).expect("utf8").to_string()
    }

    fn pnl_param(mantissa: i128, scale: i32) -> PitParamPnl {
        PitParamPnl(PitParamDecimal {
            mantissa_lo: mantissa as i64,
            mantissa_hi: (mantissa >> 64) as i64,
            scale,
        })
    }

    fn quantity_param(mantissa: i128, scale: i32) -> PitParamQuantity {
        PitParamQuantity(PitParamDecimal {
            mantissa_lo: mantissa as i64,
            mantissa_hi: (mantissa >> 64) as i64,
            scale,
        })
    }

    fn volume_param(mantissa: i128, scale: i32) -> PitParamVolume {
        PitParamVolume(PitParamDecimal {
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
        free_fn: Option<PitMutationFreeFn>,
    }

    fn sample_order() -> Order {
        Order {
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
            position: None,
            margin: None,
            user_data: std::ptr::null_mut(),
        }
    }

    fn execute_with_custom_pre_trade_policy(
        check_fn: PitPretradePreTradePolicyCheckFn,
        user_data: *mut c_void,
    ) -> openpit::pretrade::PreTradeReservation {
        let engine = openpit::Engine::<Order, ExecutionReport, AccountAdjustment>::builder()
            .pre_trade_policy(CustomPreTradePolicy {
                name: "ffi.custom".to_owned(),
                check_fn,
                apply_execution_report_fn: custom_apply_report_fn,
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
        _ctx: *const PitPretradeContext,
        _order: *const PitOrder,
        mutations: *mut PitMutations,
        user_data: *mut c_void,
    ) -> *mut PitRejectList {
        let ctx = unsafe { &*(user_data as *const MutationPushContext) };
        for entry in &ctx.entries {
            let ok = unsafe {
                pit_mutations_push(
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
        let mut pointer = PitMutations {
            mutations: &mut mutations as *mut Mutations,
        };
        let ok = unsafe {
            pit_mutations_push(
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
            _ctx: *const PitPretradeContext,
            _order: *const PitOrder,
            mutations: *mut PitMutations,
            user_data: *mut c_void,
        ) -> *mut PitRejectList {
            let ctx = unsafe { &*(user_data as *const MutationPushContext) };
            let ok = unsafe {
                pit_mutations_push(
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
            pit_mutations_push(
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
    fn pnl_killswitch_create_accepts_multiple_params() {
        let usd = PitStringView::from_utf8("USD");
        let eur = PitStringView::from_utf8("EUR");
        let params = [
            PitPretradePoliciesPnlKillSwitchParam {
                settlement_asset: usd,
                barrier: pnl_param(1000, 0),
            },
            PitPretradePoliciesPnlKillSwitchParam {
                settlement_asset: eur,
                barrier: pnl_param(500, 0),
            },
        ];
        let pointer = unsafe {
            pit_create_pretrade_policies_pnl_killswitch_policy(
                params.as_ptr(),
                params.len(),
                std::ptr::null_mut(),
            )
        };
        assert!(!pointer.is_null());
        pit_destroy_pretrade_check_pre_trade_start_policy(pointer);
    }

    #[test]
    fn order_size_limit_create_accepts_multiple_params() {
        let usd = PitStringView::from_utf8("USD");
        let eur = PitStringView::from_utf8("EUR");
        let params = [
            PitPretradePoliciesOrderSizeLimitParam {
                settlement_asset: usd,
                max_quantity: quantity_param(10, 0),
                max_notional: volume_param(1000, 0),
            },
            PitPretradePoliciesOrderSizeLimitParam {
                settlement_asset: eur,
                max_quantity: quantity_param(5, 0),
                max_notional: volume_param(500, 0),
            },
        ];
        let pointer = unsafe {
            pit_create_pretrade_policies_order_size_limit_policy(
                params.as_ptr(),
                params.len(),
                std::ptr::null_mut(),
            )
        };
        assert!(!pointer.is_null());
        pit_destroy_pretrade_check_pre_trade_start_policy(pointer);
    }

    #[test]
    fn pnl_killswitch_create_rejects_zero_len_params() {
        let mut out_error = std::ptr::null_mut();
        let pointer = unsafe {
            pit_create_pretrade_policies_pnl_killswitch_policy(std::ptr::null(), 0, &mut out_error)
        };
        assert!(pointer.is_null());
        assert_eq!(
            cstr_to_string(out_error),
            "pnl_killswitch_policy requires at least one barrier"
        );
    }

    #[test]
    fn order_size_limit_create_rejects_zero_len_params() {
        let mut out_error = std::ptr::null_mut();
        let pointer = unsafe {
            pit_create_pretrade_policies_order_size_limit_policy(
                std::ptr::null(),
                0,
                &mut out_error,
            )
        };
        assert!(pointer.is_null());
        assert_eq!(
            cstr_to_string(out_error),
            "order_size_limit_policy requires at least one limit"
        );
    }

    #[test]
    fn pnl_killswitch_create_rejects_null_params_with_positive_len() {
        let mut out_error = std::ptr::null_mut();
        let pointer = unsafe {
            pit_create_pretrade_policies_pnl_killswitch_policy(std::ptr::null(), 1, &mut out_error)
        };
        assert!(pointer.is_null());
        assert_eq!(
            cstr_to_string(out_error),
            "pnl_killswitch_policy params is null"
        );
    }

    #[test]
    fn order_size_limit_create_rejects_null_params_with_positive_len() {
        let mut out_error = std::ptr::null_mut();
        let pointer = unsafe {
            pit_create_pretrade_policies_order_size_limit_policy(
                std::ptr::null(),
                1,
                &mut out_error,
            )
        };
        assert!(pointer.is_null());
        assert_eq!(
            cstr_to_string(out_error),
            "order_size_limit_policy params is null"
        );
    }

    #[test]
    fn pnl_killswitch_create_reports_indexed_settlement_error() {
        let usd = PitStringView::from_utf8("USD");
        let invalid = PitStringView::from_utf8("");
        let params = [
            PitPretradePoliciesPnlKillSwitchParam {
                settlement_asset: usd,
                barrier: pnl_param(1000, 0),
            },
            PitPretradePoliciesPnlKillSwitchParam {
                settlement_asset: invalid,
                barrier: pnl_param(100, 0),
            },
        ];
        let mut out_error = std::ptr::null_mut();
        let pointer = unsafe {
            pit_create_pretrade_policies_pnl_killswitch_policy(
                params.as_ptr(),
                params.len(),
                &mut out_error,
            )
        };
        assert!(pointer.is_null());
        let error = cstr_to_string(out_error);
        assert!(
            error.contains("param[1] settlement asset is invalid"),
            "actual error: {}",
            error
        );
    }

    #[test]
    fn order_size_limit_create_reports_indexed_settlement_error() {
        let usd = PitStringView::from_utf8("USD");
        let invalid = PitStringView::from_utf8("");
        let params = [
            PitPretradePoliciesOrderSizeLimitParam {
                settlement_asset: usd,
                max_quantity: quantity_param(10, 0),
                max_notional: volume_param(1000, 0),
            },
            PitPretradePoliciesOrderSizeLimitParam {
                settlement_asset: invalid,
                max_quantity: quantity_param(5, 0),
                max_notional: volume_param(500, 0),
            },
        ];
        let mut out_error = std::ptr::null_mut();
        let pointer = unsafe {
            pit_create_pretrade_policies_order_size_limit_policy(
                params.as_ptr(),
                params.len(),
                &mut out_error,
            )
        };
        assert!(pointer.is_null());
        let error = cstr_to_string(out_error);
        assert!(
            error.contains("param[1] settlement asset is invalid"),
            "actual error: {}",
            error
        );
    }

    #[test]
    fn pnl_killswitch_create_reports_indexed_barrier_error() {
        let usd = PitStringView::from_utf8("USD");
        let params = [PitPretradePoliciesPnlKillSwitchParam {
            settlement_asset: usd,
            barrier: pnl_param(1000, -1),
        }];
        let mut out_error = std::ptr::null_mut();
        let pointer = unsafe {
            pit_create_pretrade_policies_pnl_killswitch_policy(
                params.as_ptr(),
                params.len(),
                &mut out_error,
            )
        };
        assert!(pointer.is_null());
        let error = cstr_to_string(out_error);
        assert!(
            error.contains("param[0] barrier is invalid"),
            "actual error: {}",
            error
        );
    }

    #[test]
    fn order_size_limit_create_reports_indexed_max_quantity_error() {
        let usd = PitStringView::from_utf8("USD");
        let params = [PitPretradePoliciesOrderSizeLimitParam {
            settlement_asset: usd,
            max_quantity: quantity_param(10, -1),
            max_notional: volume_param(1000, 0),
        }];
        let mut out_error = std::ptr::null_mut();
        let pointer = unsafe {
            pit_create_pretrade_policies_order_size_limit_policy(
                params.as_ptr(),
                params.len(),
                &mut out_error,
            )
        };
        assert!(pointer.is_null());
        let error = cstr_to_string(out_error);
        assert!(
            error.contains("param[0] max quantity is invalid"),
            "actual error: {}",
            error
        );
    }

    #[test]
    fn order_size_limit_create_reports_indexed_max_notional_error() {
        let usd = PitStringView::from_utf8("USD");
        let params = [PitPretradePoliciesOrderSizeLimitParam {
            settlement_asset: usd,
            max_quantity: quantity_param(10, 0),
            max_notional: volume_param(1000, -1),
        }];
        let mut out_error = std::ptr::null_mut();
        let pointer = unsafe {
            pit_create_pretrade_policies_order_size_limit_policy(
                params.as_ptr(),
                params.len(),
                &mut out_error,
            )
        };
        assert!(pointer.is_null());
        let error = cstr_to_string(out_error);
        assert!(
            error.contains("param[0] max notional is invalid"),
            "actual error: {}",
            error
        );
    }

    #[test]
    fn create_functions_accept_duplicate_settlement_entries() {
        let usd = PitStringView::from_utf8("USD");
        let pnl_params = [
            PitPretradePoliciesPnlKillSwitchParam {
                settlement_asset: usd,
                barrier: pnl_param(1000, 0),
            },
            PitPretradePoliciesPnlKillSwitchParam {
                settlement_asset: usd,
                barrier: pnl_param(900, 0),
            },
        ];
        let pnl_handle = unsafe {
            pit_create_pretrade_policies_pnl_killswitch_policy(
                pnl_params.as_ptr(),
                pnl_params.len(),
                std::ptr::null_mut(),
            )
        };
        assert!(
            !pnl_handle.is_null(),
            "unexpected error: {}",
            cstr_to_string(std::ptr::null_mut())
        );
        pit_destroy_pretrade_check_pre_trade_start_policy(pnl_handle);

        let size_params = [
            PitPretradePoliciesOrderSizeLimitParam {
                settlement_asset: usd,
                max_quantity: quantity_param(10, 0),
                max_notional: volume_param(1000, 0),
            },
            PitPretradePoliciesOrderSizeLimitParam {
                settlement_asset: usd,
                max_quantity: quantity_param(9, 0),
                max_notional: volume_param(900, 0),
            },
        ];
        let size_handle = unsafe {
            pit_create_pretrade_policies_order_size_limit_policy(
                size_params.as_ptr(),
                size_params.len(),
                std::ptr::null_mut(),
            )
        };
        assert!(
            !size_handle.is_null(),
            "unexpected error: {}",
            cstr_to_string(std::ptr::null_mut())
        );
        pit_destroy_pretrade_check_pre_trade_start_policy(size_handle);
    }

    #[test]
    fn add_policy_reports_null_builder() {
        let policy = pit_create_pretrade_policies_order_validation_policy();
        let mut out_error = std::ptr::null_mut();
        let ok = pit_engine_builder_add_check_pre_trade_start_policy(
            std::ptr::null_mut(),
            policy,
            &mut out_error,
        );
        assert!(!ok);
        assert_eq!(cstr_to_string(out_error), "engine builder is null");
        pit_destroy_pretrade_check_pre_trade_start_policy(policy);
    }

    #[test]
    fn add_policy_reports_null_policy() {
        let builder = crate::engine::pit_create_engine_builder();
        let mut out_error = std::ptr::null_mut();
        let ok = pit_engine_builder_add_check_pre_trade_start_policy(
            builder,
            std::ptr::null_mut(),
            &mut out_error,
        );
        assert!(!ok);
        assert_eq!(cstr_to_string(out_error), "policy is null");
        crate::engine::pit_destroy_engine_builder(builder);
    }

    #[test]
    fn custom_check_policy_keeps_caller_name() {
        let name = PitStringView::from_utf8("caller.check.start");
        let pointer = unsafe {
            pit_create_pretrade_custom_check_pre_trade_start_policy(
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

        let got = pit_pretrade_check_pre_trade_start_policy_get_name(pointer);
        assert_eq!(string_view_to_string(got), "caller.check.start");
        pit_destroy_pretrade_check_pre_trade_start_policy(pointer);
    }

    #[test]
    fn custom_pre_trade_policy_keeps_caller_name() {
        let name = PitStringView::from_utf8("caller.pretrade");
        let pointer = unsafe {
            pit_create_pretrade_custom_pre_trade_policy(
                name,
                custom_pre_trade_check_fn,
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

        let got = pit_pretrade_pre_trade_policy_get_name(pointer);
        assert_eq!(string_view_to_string(got), "caller.pretrade");
        pit_destroy_pretrade_pre_trade_policy(pointer);
    }

    #[test]
    fn custom_account_adjustment_policy_keeps_caller_name() {
        let name = PitStringView::from_utf8("caller.account.adjustment");
        let pointer = unsafe {
            pit_create_custom_account_adjustment_policy(
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

        let got = pit_account_adjustment_policy_get_name(pointer);
        assert_eq!(string_view_to_string(got), "caller.account.adjustment");
        pit_destroy_account_adjustment_policy(pointer);
    }

    #[test]
    fn custom_policy_create_rejects_null_empty_and_invalid_name() {
        let mut out_error = std::ptr::null_mut();
        let null_name = unsafe {
            pit_create_pretrade_custom_check_pre_trade_start_policy(
                PitStringView::not_set(),
                custom_check_fn,
                custom_apply_report_fn,
                custom_free_user_data_fn,
                std::ptr::null_mut(),
                &mut out_error,
            )
        };
        assert!(null_name.is_null());
        assert!(cstr_to_string(out_error).contains("policy name is null"));

        let empty = PitStringView::from_utf8("");
        let mut out_error = std::ptr::null_mut();
        let empty_name = unsafe {
            pit_create_pretrade_custom_check_pre_trade_start_policy(
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
            pit_create_pretrade_custom_check_pre_trade_start_policy(
                PitStringView {
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
        let name_a = PitStringView::from_utf8("custom.a");
        let name_b = PitStringView::from_utf8("custom.b");
        let handle_a = unsafe {
            pit_create_pretrade_custom_check_pre_trade_start_policy(
                name_a,
                custom_check_fn,
                custom_apply_report_fn,
                custom_free_user_data_fn,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        let handle_b = unsafe {
            pit_create_pretrade_custom_check_pre_trade_start_policy(
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

        let got_a = pit_pretrade_check_pre_trade_start_policy_get_name(handle_a);
        let got_b = pit_pretrade_check_pre_trade_start_policy_get_name(handle_b);
        assert_eq!(string_view_to_string(got_a), "custom.a");
        assert_eq!(string_view_to_string(got_b), "custom.b");
        pit_destroy_pretrade_check_pre_trade_start_policy(handle_a);
        pit_destroy_pretrade_check_pre_trade_start_policy(handle_b);
    }

    #[test]
    fn rate_limit_policy_create_and_destroy_are_reachable() {
        let pointer = pit_create_pretrade_policies_rate_limit_policy(10, 1);
        assert!(!pointer.is_null());
        pit_destroy_pretrade_check_pre_trade_start_policy(pointer);
    }

    #[test]
    fn add_main_and_account_adjustment_policy_to_builder() {
        let builder = crate::engine::pit_create_engine_builder();

        let pre_trade_name = PitStringView::from_utf8("caller.pretrade.add");
        let pre_trade_policy = unsafe {
            pit_create_pretrade_custom_pre_trade_policy(
                pre_trade_name,
                custom_pre_trade_check_fn,
                custom_apply_report_fn,
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
        let ok = pit_engine_builder_add_pre_trade_policy(
            builder,
            pre_trade_policy,
            std::ptr::null_mut(),
        );
        assert!(ok, "{}", cstr_to_string(std::ptr::null_mut()));
        pit_destroy_pretrade_pre_trade_policy(pre_trade_policy);

        let account_name = PitStringView::from_utf8("caller.adjustment.add");
        let account_policy = unsafe {
            pit_create_custom_account_adjustment_policy(
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
        let ok = pit_engine_builder_add_account_adjustment_policy(
            builder,
            account_policy,
            std::ptr::null_mut(),
        );
        assert!(ok, "{}", cstr_to_string(std::ptr::null_mut()));
        pit_destroy_account_adjustment_policy(account_policy);

        let engine = crate::engine::pit_engine_builder_build(builder, std::ptr::null_mut());
        assert!(
            !engine.is_null(),
            "{}",
            cstr_to_string(std::ptr::null_mut())
        );
        crate::engine::pit_destroy_engine(engine);
        crate::engine::pit_destroy_engine_builder(builder);
    }

    #[test]
    fn add_check_start_policy_to_builder_and_execute_paths() {
        let builder = crate::engine::pit_create_engine_builder();

        let check_name = PitStringView::from_utf8("caller.check.start.add");
        let check_policy = unsafe {
            pit_create_pretrade_custom_check_pre_trade_start_policy(
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
        let ok = pit_engine_builder_add_check_pre_trade_start_policy(
            builder,
            check_policy,
            std::ptr::null_mut(),
        );
        assert!(ok, "{}", cstr_to_string(std::ptr::null_mut()));
        pit_destroy_pretrade_check_pre_trade_start_policy(check_policy);

        let engine = crate::engine::pit_engine_builder_build(builder, std::ptr::null_mut());
        assert!(
            !engine.is_null(),
            "{}",
            cstr_to_string(std::ptr::null_mut())
        );

        let order = PitOrder::default();
        let mut request = std::ptr::null_mut();
        let mut out_rejects = std::ptr::null_mut();
        let status = crate::engine::pit_engine_start_pre_trade(
            engine,
            &order,
            &mut request,
            &mut out_rejects,
            std::ptr::null_mut(),
        );
        assert_eq!(status, crate::engine::PitPretradeStatus::Passed);
        assert!(out_rejects.is_null());
        crate::engine::pit_destroy_pretrade_pre_trade_request(request);

        let report = crate::execution_report::PitExecutionReport::default();
        let post =
            crate::engine::pit_engine_apply_execution_report(engine, &report, std::ptr::null_mut());
        assert!(!post.is_error);
        crate::engine::pit_destroy_engine(engine);
        crate::engine::pit_destroy_engine_builder(builder);
    }

    #[test]
    fn custom_pre_trade_and_account_adjustment_callbacks_are_invoked_via_engine() {
        let builder = crate::engine::pit_create_engine_builder();

        let pre_trade_name = PitStringView::from_utf8("pretrade.invoke");
        let pre_trade_policy = unsafe {
            pit_create_pretrade_custom_pre_trade_policy(
                pre_trade_name,
                custom_pre_trade_check_fn,
                custom_apply_report_fn,
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
        assert!(pit_engine_builder_add_pre_trade_policy(
            builder,
            pre_trade_policy,
            std::ptr::null_mut()
        ));
        pit_destroy_pretrade_pre_trade_policy(pre_trade_policy);

        let account_name = PitStringView::from_utf8("account.invoke");
        let account_policy = unsafe {
            pit_create_custom_account_adjustment_policy(
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
        assert!(pit_engine_builder_add_account_adjustment_policy(
            builder,
            account_policy,
            std::ptr::null_mut()
        ));
        pit_destroy_account_adjustment_policy(account_policy);

        let engine = crate::engine::pit_engine_builder_build(builder, std::ptr::null_mut());
        assert!(
            !engine.is_null(),
            "{}",
            cstr_to_string(std::ptr::null_mut())
        );

        let order = PitOrder::default();
        let mut out_reservation = std::ptr::null_mut();
        let mut out_rejects = std::ptr::null_mut();
        let status = crate::engine::pit_engine_execute_pre_trade(
            engine,
            &order,
            &mut out_reservation,
            &mut out_rejects,
            std::ptr::null_mut(),
        );
        assert_eq!(status, crate::engine::PitPretradeStatus::Passed);
        assert!(out_rejects.is_null());
        crate::engine::pit_destroy_pretrade_pre_trade_reservation(out_reservation);

        let report = crate::execution_report::PitExecutionReport::default();
        let post =
            crate::engine::pit_engine_apply_execution_report(engine, &report, std::ptr::null_mut());
        assert!(!post.is_error);

        let adjustment = crate::account_adjustment::PitAccountAdjustment::default();
        let batch = [adjustment];
        let mut out_reject = std::ptr::null_mut();
        let status = crate::engine::pit_engine_apply_account_adjustment(
            engine,
            1,
            batch.as_ptr(),
            batch.len(),
            &mut out_reject,
            std::ptr::null_mut(),
        );
        assert_eq!(
            status,
            crate::account_adjustment::PitAccountAdjustmentApplyStatus::Applied
        );
        assert!(out_reject.is_null());

        crate::engine::pit_destroy_engine(engine);
        crate::engine::pit_destroy_engine_builder(builder);
    }
}
