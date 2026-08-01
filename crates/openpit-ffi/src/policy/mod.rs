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

#![allow(
    clippy::arc_with_non_send_sync,
    clippy::missing_safety_doc,
    clippy::not_unsafe_ptr_arg_deref
)]

use std::ffi::c_void;
use std::rc::Rc;
use std::str;
use std::sync::Arc;

use openpit::param::{Asset, Pnl};
use openpit::pretrade::{
    PolicyPreTradeResult, PostTradeContext, PostTradeResult, PreTradeContext, PreTradePolicy,
    Rejects,
};
use openpit::storage::StorageBuilder;
use openpit::{AccountAdjustmentContext, Mutation, Mutations, PolicyAccountAdjustmentResult};

use crate::OpenPitStringView;
use crate::{AccountAdjustment, ExecutionReport, Order};

use crate::param::{OpenPitParamAccountId, OpenPitParamPnlOptional};

use crate::last_error::{write_error, OpenPitOutError};
use crate::write_error_format;

pub mod custom;
mod order_size_limit;
mod order_validation;
mod pnl_bounds_killswitch;
mod rate_limit;
mod spot_funds;
pub use spot_funds::OpenPitPretradePoliciesSpotFundsLimitMode;

#[allow(unused_imports)]
pub use custom::{
    openpit_create_pretrade_custom_pre_trade_policy,
    openpit_create_pretrade_custom_pre_trade_policy_with_dry_run,
};
pub use custom::{
    OpenPitPretradePreTradePolicy, OpenPitPretradePreTradePolicyApplyAccountAdjustmentFn,
    OpenPitPretradePreTradePolicyApplyExecutionReportFn,
    OpenPitPretradePreTradePolicyCheckPreTradeStartFn, OpenPitPretradePreTradePolicyFreeUserDataFn,
    OpenPitPretradePreTradePolicyPerformPreTradeCheckFn,
};

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
pub struct PolicyHandle<Policy: ?Sized> {
    policy: Arc<Policy>,
}

impl<Policy: ?Sized + GeneralPreTradePolicy> PolicyHandle<Policy> {
    fn new(policy: Arc<Policy>) -> *mut Self {
        Box::into_raw(Box::new(Self { policy }))
    }

    fn get_name(&self) -> OpenPitStringView {
        OpenPitStringView::from_utf8(self.policy.name())
    }
}

//--------------------------------------------------------------------------------------------------

/// Unified trait object for all pre-trade hooks exposed through FFI.
///
/// It is backed by `PreTradePolicy<Order, ExecutionReport, AccountAdjustment, EngineLocking>`
/// because the core engine now routes start-stage checks, main-stage checks,
/// execution-report updates, and account-adjustment validation through the same
/// policy list.
type UnifiedPreTradePolicy =
    dyn PreTradePolicy<Order, ExecutionReport, AccountAdjustment, openpit_interop::EngineLocking>;

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
///
/// A finalizer has no right to fail: by the time it runs the decision is already
/// made and the state it finalizes was applied eagerly, so there is nothing left
/// to compensate and no caller left to answer.
///
/// Returns `true` on success. On `false`, the callback may write a caller-owned
/// `OpenPitSharedString` to `out_error`; the engine consumes and destroys it
/// before returning across the ABI. A null error handle still means failure and
/// receives generic diagnostics.
///
/// A reported failure never fails the void commit or rollback call that ran the
/// callback. It arms the engine kill switch instead, and a mutation registered
/// through this ABI is a custom-policy mutation whose state reach the engine
/// cannot bound, so EVERY account is blocked: policy `"Engine"`, code
/// `OPENPIT_PRETRADE_REJECT_CODE_SYSTEM_UNAVAILABLE`, reason
/// `"mutation finalizer failed"`. The owner of the reservation or the drop-copy
/// operation is not told directly; the next pre-trade call is rejected. An
/// operator clears the block with `openpit_engine_unblock_all_accounts`. A
/// failure reported while drop copy compensates a fatal evaluation reject
/// additionally surfaces `SystemUnavailable` rejects to that caller.
pub type OpenPitMutationFn =
    unsafe extern "C" fn(user_data: *mut c_void, out_error: OpenPitOutError) -> bool;

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

pub(crate) fn mutation_from_ffi_callbacks(
    commit_fn: OpenPitMutationFn,
    rollback_fn: OpenPitMutationFn,
    user_data: *mut c_void,
    free_fn: Option<OpenPitMutationFreeFn>,
) -> Mutation {
    fn call(
        callback: OpenPitMutationFn,
        user_data: *mut c_void,
        fallback: &'static str,
    ) -> Result<(), String> {
        let mut error = std::ptr::null_mut();
        let succeeded = unsafe { callback(user_data, &mut error) };
        let details = unsafe { crate::string::take_shared_string(error) };
        if succeeded {
            Ok(())
        } else {
            Err(details.unwrap_or_else(|| fallback.to_owned()))
        }
    }

    let guard = Rc::new(FfiMutationGuard { user_data, free_fn });
    let commit_guard = Rc::clone(&guard);
    let rollback_guard = Rc::clone(&guard);
    let mutation = Mutation::new_fallible_with_error(
        move || {
            let result = call(
                commit_fn,
                user_data,
                "mutation commit callback failed without error details",
            );
            drop(commit_guard);
            result
        },
        move || {
            let result = call(
                rollback_fn,
                user_data,
                "mutation rollback callback failed without error details",
            );
            drop(rollback_guard);
            result
        },
    );
    drop(guard);
    mutation
}

//--------------------------------------------------------------------------------------------------

pub(super) fn policy_storage(
    builder: &crate::engine::OpenPitEngineBuilder,
) -> Option<&StorageBuilder<openpit_interop::StorageLockingPolicyFactory>> {
    match builder.inner.as_ref()? {
        crate::engine::BuilderState::Synced(builder) => Some(builder.storage_builder()),
        crate::engine::BuilderState::Ready(builder) => Some(builder.storage_builder()),
    }
}

pub(super) unsafe fn try_slice_arg<'a, T>(
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

/// Parses an asset string view named `field` (for example `"settlement_asset"`
/// or `"account_currency"`), reporting any failure through the shared-string
/// builder error channel. The `field` name is used verbatim in the error text so
/// each caller reports the field its API actually exposes.
pub(super) fn parse_asset_or_error(
    value: OpenPitStringView,
    label: &str,
    index: usize,
    field: &str,
    out_error: OpenPitOutError,
) -> Option<Asset> {
    let raw = match unsafe { cstr_arg(value) } {
        Some(v) => v,
        None => {
            write_error_format!(out_error, "{}[{}] {} is not set", label, index, field);
            return None;
        }
    };
    match Asset::new(raw) {
        Ok(v) => Some(v),
        Err(e) => {
            write_error_format!(
                out_error,
                "{}[{}] {} is invalid: {}",
                label,
                index,
                field,
                e
            );
            None
        }
    }
}

/// Parses an asset string view named `field` for a configure function, mapping
/// any failure to an [`OpenPitConfigureError`] (the only error channel those
/// functions expose). Mirrors [`parse_asset_or_error`], which instead targets
/// the shared-string builder error channel.
pub(super) fn parse_configure_asset(
    value: OpenPitStringView,
    label: &str,
    index: usize,
    field: &str,
) -> Result<Asset, crate::engine::OpenPitConfigureError> {
    let raw = unsafe { cstr_arg(value) }.ok_or_else(|| {
        crate::engine::OpenPitConfigureError::validation(format!(
            "{label}[{index}] {field} is not set"
        ))
    })?;
    Asset::new(&raw).map_err(|e| {
        crate::engine::OpenPitConfigureError::validation(format!(
            "{label}[{index}] {field} is invalid: {e}"
        ))
    })
}

pub(super) fn parse_optional_pnl_or_error(
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

pub(super) unsafe fn cstr_arg(ptr: OpenPitStringView) -> Option<String> {
    if ptr.ptr.is_null() {
        return None;
    }
    let bytes = unsafe { std::slice::from_raw_parts(ptr.ptr, ptr.len) };
    let value = str::from_utf8(bytes).ok()?.to_owned();
    Some(value)
}

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

type FfiStorageFactory = openpit_interop::StorageLockingPolicyFactory;

impl PreTradePolicy<Order, ExecutionReport, AccountAdjustment, openpit_interop::EngineLocking>
    for DynPreTradePolicy
{
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn policy_group_id(&self) -> openpit::PolicyGroupId {
        self.inner.policy_group_id()
    }

    fn check_pre_trade_start(
        &self,
        ctx: &PreTradeContext<FfiStorageFactory>,
        order: &Order,
    ) -> Result<(), Rejects> {
        self.inner.check_pre_trade_start(ctx, order)
    }

    fn perform_pre_trade_check(
        &self,
        ctx: &PreTradeContext<FfiStorageFactory>,
        order: &Order,
        mutations: &mut Mutations,
    ) -> Result<Option<PolicyPreTradeResult>, Rejects> {
        self.inner.perform_pre_trade_check(ctx, order, mutations)
    }

    fn check_pre_trade_start_dry_run(
        &self,
        ctx: &PreTradeContext<FfiStorageFactory>,
        order: &Order,
    ) -> Result<(), Rejects> {
        self.inner.check_pre_trade_start_dry_run(ctx, order)
    }

    fn perform_pre_trade_check_dry_run(
        &self,
        ctx: &PreTradeContext<FfiStorageFactory>,
        order: &Order,
        mutations: &mut Mutations,
    ) -> Result<Option<PolicyPreTradeResult>, Rejects> {
        self.inner
            .perform_pre_trade_check_dry_run(ctx, order, mutations)
    }

    fn apply_execution_report(
        &self,
        ctx: &PostTradeContext<FfiStorageFactory>,
        report: &ExecutionReport,
    ) -> Option<PostTradeResult> {
        self.inner.apply_execution_report(ctx, report)
    }

    fn apply_account_adjustment(
        &self,
        ctx: &AccountAdjustmentContext<FfiStorageFactory>,
        account_id: openpit::param::AccountId,
        adjustment: &AccountAdjustment,
        mutations: &mut Mutations,
    ) -> Result<PolicyAccountAdjustmentResult, Rejects> {
        self.inner
            .apply_account_adjustment(ctx, account_id, adjustment, mutations)
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

#[no_mangle]
/// Registers one commit/rollback mutation in the provided collector.
///
/// Contract:
/// - `mutations` must be a valid non-null callback-scoped pointer.
/// - `commit_fn` and `rollback_fn` must remain callable until one of them is
///   executed.
/// - `user_data` is passed to both callbacks.
/// - Apply tentative state before registration. Pre-trade and drop-copy
///   finalization each run exactly one callback per mutation, when the caller
///   commits or rolls back the returned handle. A fatal drop-copy evaluation
///   reject runs every collected `rollback_fn` instead, including mutations
///   whose `commit_fn` was not reached.
/// - Neither callback may fail. A failure reported by either one never fails the
///   void commit or rollback call; it arms the engine kill switch. A mutation
///   registered here is a custom-policy mutation, so that kill switch blocks
///   EVERY account until an operator calls
///   `openpit_engine_unblock_all_accounts`. See `OpenPitMutationFn`.
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

    unsafe {
        (*raw_mutations).push(mutation_from_ffi_callbacks(
            commit_fn,
            rollback_fn,
            user_data,
            free_fn,
        ));
    }
    true
}

//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::ffi::c_void;
    use std::rc::Rc;

    use super::*;

    use crate::order::OpenPitOrder;
    use crate::reject::OpenPitPretradeRejectList;

    use openpit::param::{AccountId, Asset, Quantity, Side, TradeAmount};
    use openpit::Instrument;
    use openpit_interop::{OrderOperationAccess, PopulatedOrderOperation};

    fn accepted_rejects() -> *mut OpenPitPretradeRejectList {
        crate::reject::openpit_create_pretrade_reject_list(0)
    }

    unsafe extern "C" fn custom_apply_report_fn(
        _ctx: *const super::custom::OpenPitPostTradeContext,
        _report: *const crate::execution_report::OpenPitExecutionReport,
        _out_adjustments: *mut crate::account_outcome::OpenPitPostTradeAdjustmentList,
        _out_account_pnls: *mut crate::account_outcome::OpenPitPostTradeAccountPnlList,
        _user_data: *mut c_void,
    ) -> *mut crate::reject::OpenPitPretradeAccountBlockList {
        std::ptr::null_mut()
    }

    unsafe extern "C" fn custom_free_user_data_fn(_user_data: *mut c_void) {}

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

    struct FailingMutationUserData {
        details: Option<&'static str>,
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
            .sync(openpit_interop::EngineLocking::new(
                openpit_interop::SyncMode::None,
            ))
            .pre_trade(custom::CustomPreTradePolicy {
                name: "ffi.custom".to_owned(),
                policy_group_id: openpit::PolicyGroupId::new(0),
                check_pre_trade_start_fn: None,
                perform_pre_trade_check_fn: Some(check_fn),
                check_pre_trade_start_dry_run_fn: None,
                perform_pre_trade_check_dry_run_fn: None,
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

    unsafe extern "C" fn tracked_mutation_commit(
        user_data: *mut c_void,
        _out_error: OpenPitOutError,
    ) -> bool {
        let data = unsafe { &*(user_data as *mut MutationUserData) };
        let mut state = data.state.borrow_mut();
        state.commit_calls += 1;
        state.sequence.push(data.marker);
        true
    }

    unsafe extern "C" fn tracked_mutation_rollback(
        user_data: *mut c_void,
        _out_error: OpenPitOutError,
    ) -> bool {
        let data = unsafe { &*(user_data as *mut MutationUserData) };
        let mut state = data.state.borrow_mut();
        state.rollback_calls += 1;
        state.sequence.push(data.marker);
        true
    }

    unsafe extern "C" fn tracked_mutation_free(user_data: *mut c_void) {
        let data = unsafe { Box::from_raw(user_data as *mut MutationUserData) };
        data.state.borrow_mut().free_calls += 1;
    }

    unsafe extern "C" fn failing_mutation_rollback(
        user_data: *mut c_void,
        out_error: OpenPitOutError,
    ) -> bool {
        let data = unsafe { &*(user_data as *const FailingMutationUserData) };
        if let Some(details) = data.details {
            unsafe {
                if let Some(out_error) = out_error.as_mut() {
                    *out_error = crate::string::OpenPitSharedString::new_handle(details);
                }
            }
        }
        false
    }

    unsafe extern "C" fn successful_mutation_commit(
        _user_data: *mut c_void,
        _out_error: OpenPitOutError,
    ) -> bool {
        true
    }

    unsafe extern "C" fn failing_mutation_free(user_data: *mut c_void) {
        unsafe { drop(Box::from_raw(user_data as *mut FailingMutationUserData)) };
    }

    // Pushes a mutation whose rollback callback fails, then aborts the pipeline
    // with a fatal reject so drop copy has to compensate that mutation.
    unsafe extern "C" fn push_failing_mutation_check_fn(
        _ctx: *const OpenPitPretradeContext,
        _order: *const OpenPitOrder,
        mutations: *mut OpenPitMutations,
        _out_result: *mut crate::account_outcome::OpenPitPretradePreTradeResult,
        user_data: *mut c_void,
    ) -> *mut OpenPitPretradeRejectList {
        let ok = unsafe {
            openpit_mutations_push(
                mutations,
                successful_mutation_commit,
                failing_mutation_rollback,
                user_data,
                Some(failing_mutation_free),
                std::ptr::null_mut(),
            )
        };
        assert!(ok);
        std::ptr::null_mut()
    }

    fn apply_drop_copy_with_failing_mutation(details: Option<&'static str>) -> Rejects {
        let user_data = Box::into_raw(Box::new(FailingMutationUserData { details })).cast();
        let engine = openpit::EngineBuilder::<Order, ExecutionReport, AccountAdjustment>::new()
            .sync(openpit_interop::EngineLocking::new(
                openpit_interop::SyncMode::None,
            ))
            .pre_trade(custom::CustomPreTradePolicy {
                name: "ffi.custom".to_owned(),
                policy_group_id: openpit::PolicyGroupId::new(0),
                check_pre_trade_start_fn: None,
                perform_pre_trade_check_fn: Some(push_failing_mutation_check_fn),
                check_pre_trade_start_dry_run_fn: None,
                perform_pre_trade_check_dry_run_fn: None,
                apply_execution_report_fn: Some(custom_apply_report_fn),
                apply_account_adjustment_fn: None,
                free_user_data_fn: custom_free_user_data_fn,
                user_data,
            })
            .build()
            .expect("engine build must succeed");
        engine
            .apply_drop_copy(sample_order())
            .expect_err("fatal reject must abort drop-copy")
    }

    unsafe extern "C" fn push_tracked_mutations_check_fn(
        _ctx: *const OpenPitPretradeContext,
        _order: *const OpenPitOrder,
        mutations: *mut OpenPitMutations,
        _out_result: *mut crate::account_outcome::OpenPitPretradePreTradeResult,
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
        accepted_rejects()
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
        unsafe extern "C" fn commit_without_free(
            user_data: *mut c_void,
            _out_error: OpenPitOutError,
        ) -> bool {
            let state = unsafe { &*(user_data as *const RefCell<MutationState>) };
            state.borrow_mut().commit_calls += 1;
            true
        }
        unsafe extern "C" fn rollback_without_free(
            _user_data: *mut c_void,
            _out_error: OpenPitOutError,
        ) -> bool {
            true
        }

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
            _out_result: *mut crate::account_outcome::OpenPitPretradePreTradeResult,
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
            accepted_rejects()
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
        unsafe extern "C" fn noop(_user_data: *mut c_void, _out_error: OpenPitOutError) -> bool {
            true
        }

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
    fn mutation_callback_failure_preserves_owned_error_details() {
        let rejects =
            apply_drop_copy_with_failing_mutation(Some("go mutation rollback panic: boom"));

        assert_eq!(rejects.len(), 2);
        assert_eq!(rejects[1].details, "go mutation rollback panic: boom");
    }

    #[test]
    fn mutation_callback_failure_without_payload_fails_closed() {
        let rejects = apply_drop_copy_with_failing_mutation(None);

        assert_eq!(rejects.len(), 2);
        assert_eq!(
            rejects[1].details,
            "mutation rollback callback failed without error details"
        );
    }

    #[test]
    fn mutation_callback_failure_preserves_empty_error_payload() {
        let rejects = apply_drop_copy_with_failing_mutation(Some(""));

        assert_eq!(rejects.len(), 2);
        assert_eq!(rejects[1].details, "");
    }

    #[test]
    fn drop_copy_start_recorder_surfaces_failing_ffi_rollback_and_blocks_account() {
        #[derive(Default)]
        struct State {
            rollback_calls: usize,
        }

        unsafe extern "C" fn commit(_user_data: *mut c_void, _out_error: OpenPitOutError) -> bool {
            true
        }

        unsafe extern "C" fn rollback(user_data: *mut c_void, out_error: OpenPitOutError) -> bool {
            let state = unsafe { &*(user_data as *const RefCell<State>) };
            state.borrow_mut().rollback_calls += 1;
            if let Some(out_error) = unsafe { out_error.as_mut() } {
                *out_error =
                    crate::string::OpenPitSharedString::new_handle("ffi start rollback failed");
            }
            false
        }

        unsafe extern "C" fn record_start_mutation(
            ctx: *const OpenPitPretradeContext,
            _order: *const OpenPitOrder,
            user_data: *mut c_void,
        ) -> *mut OpenPitPretradeRejectList {
            let mut error = std::ptr::null_mut();
            let recorded = unsafe {
                crate::account_control::openpit_pretrade_context_record_drop_copy_start_mutation(
                    ctx, commit, rollback, user_data, None, &mut error,
                )
            };
            assert!(recorded, "{}", cstr_to_string(error));
            accepted_rejects()
        }

        unsafe extern "C" fn fail_start(
            _ctx: *const OpenPitPretradeContext,
            _order: *const OpenPitOrder,
            _user_data: *mut c_void,
        ) -> *mut OpenPitPretradeRejectList {
            std::ptr::null_mut()
        }

        let state = RefCell::new(State::default());
        let user_data = (&state as *const RefCell<State>).cast_mut().cast();
        let policy = |name: &str, start_fn: OpenPitPretradePreTradePolicyCheckPreTradeStartFn| {
            custom::CustomPreTradePolicy {
                name: name.to_owned(),
                policy_group_id: openpit::PolicyGroupId::new(0),
                check_pre_trade_start_fn: Some(start_fn),
                perform_pre_trade_check_fn: None,
                check_pre_trade_start_dry_run_fn: None,
                perform_pre_trade_check_dry_run_fn: None,
                apply_execution_report_fn: Some(custom_apply_report_fn),
                apply_account_adjustment_fn: None,
                free_user_data_fn: custom_free_user_data_fn,
                user_data,
            }
        };
        let engine = openpit::EngineBuilder::<Order, ExecutionReport, AccountAdjustment>::new()
            .sync(openpit_interop::EngineLocking::new(
                openpit_interop::SyncMode::None,
            ))
            .pre_trade(policy("ffi.record-start", record_start_mutation))
            .pre_trade(policy("ffi.fail-start", fail_start))
            .build()
            .expect("engine build must succeed");
        let order = sample_order();

        let rejects = engine
            .apply_drop_copy(order.clone())
            .expect_err("fatal start callback must roll back the recorded mutation");

        assert_eq!(state.borrow().rollback_calls, 1);
        assert_eq!(rejects.len(), 2);
        assert_eq!(rejects[1].details, "ffi start rollback failed");
        assert_eq!(
            engine
                .start_pre_trade(order)
                .expect_err("failed FFI rollback must safety-block the account")[0]
                .code,
            openpit::pretrade::RejectCode::SystemUnavailable
        );
    }

    #[test]
    fn drop_copy_start_recorder_rejects_null_context_and_keeps_user_data() {
        let state = Rc::new(RefCell::new(MutationState::default()));
        let entry: *mut c_void = Box::into_raw(Box::new(MutationUserData {
            state: Rc::clone(&state),
            marker: 1,
        }))
        .cast();

        let mut error = std::ptr::null_mut();
        let recorded = unsafe {
            crate::account_control::openpit_pretrade_context_record_drop_copy_start_mutation(
                std::ptr::null(),
                tracked_mutation_commit,
                tracked_mutation_rollback,
                entry,
                Some(tracked_mutation_free),
                &mut error,
            )
        };

        assert!(!recorded);
        assert_eq!(
            cstr_to_string(error),
            "openpit_pretrade_context_record_drop_copy_start_mutation: context is null"
        );
        assert_eq!(state.borrow().free_calls, 0);

        // Ownership never transferred, so the caller runs its own cleanup.
        unsafe { tracked_mutation_free(entry) };
        assert_eq!(state.borrow().free_calls, 1);
    }

    #[test]
    fn drop_copy_start_recorder_rejects_ordinary_pre_trade_and_keeps_user_data() {
        struct RecorderProbe {
            error: String,
            entry: *mut c_void,
            recorded: bool,
        }

        unsafe extern "C" fn record_start_mutation_check_fn(
            ctx: *const OpenPitPretradeContext,
            _order: *const OpenPitOrder,
            _mutations: *mut OpenPitMutations,
            _out_result: *mut crate::account_outcome::OpenPitPretradePreTradeResult,
            user_data: *mut c_void,
        ) -> *mut OpenPitPretradeRejectList {
            let probe = unsafe { &mut *(user_data as *mut RecorderProbe) };
            let mut error = std::ptr::null_mut();
            probe.recorded = unsafe {
                crate::account_control::openpit_pretrade_context_record_drop_copy_start_mutation(
                    ctx,
                    tracked_mutation_commit,
                    tracked_mutation_rollback,
                    probe.entry,
                    Some(tracked_mutation_free),
                    &mut error,
                )
            };
            probe.error = cstr_to_string(error);
            accepted_rejects()
        }

        let state = Rc::new(RefCell::new(MutationState::default()));
        let entry: *mut c_void = Box::into_raw(Box::new(MutationUserData {
            state: Rc::clone(&state),
            marker: 1,
        }))
        .cast();
        let mut probe = RecorderProbe {
            error: String::new(),
            entry,
            recorded: true,
        };

        let mut reservation = execute_with_custom_pre_trade_policy(
            record_start_mutation_check_fn,
            (&mut probe as *mut RecorderProbe).cast(),
        );
        reservation.commit();

        assert!(!probe.recorded);
        assert_eq!(
            probe.error,
            "pre-trade context is not an active drop-copy operation"
        );
        assert_eq!(state.borrow().free_calls, 0);

        // Ownership never transferred, so the caller runs its own cleanup.
        unsafe { tracked_mutation_free(entry) };
        assert_eq!(state.borrow().free_calls, 1);
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
}
