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

use openpit::param::{Asset, Pnl};
use openpit::pretrade::{
    PolicyPreTradeResult, PostTradeContext, PostTradeResult, PreTradeContext, PreTradePolicy,
    Rejects,
};
use openpit::storage::StorageBuilder;
use openpit::{AccountAdjustmentContext, AccountOutcomeEntry, Mutation, Mutations};

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

#[allow(unused_imports)]
pub use custom::openpit_create_pretrade_custom_pre_trade_policy;
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

pub(super) fn parse_settlement_asset_or_error(
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
    ) -> Result<Vec<AccountOutcomeEntry>, Rejects> {
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

    unsafe extern "C" fn custom_apply_report_fn(
        _ctx: *const super::custom::OpenPitPostTradeContext,
        _report: *const crate::execution_report::OpenPitExecutionReport,
        _out_adjustments: *mut crate::account_outcome::OpenPitPostTradeAdjustmentList,
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
}
