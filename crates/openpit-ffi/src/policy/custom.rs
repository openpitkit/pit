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
use std::str;
use std::sync::Arc;

use openpit::pretrade::PostTradeContext;
use openpit::pretrade::{
    PolicyPreTradeResult, PostTradeResult, PreTradeContext, PreTradePolicy, Rejects,
};
use openpit::{AccountAdjustmentContext, AccountOutcomeEntry, Mutations, PolicyGroupId};

use crate::account_adjustment::{export_account_adjustment, OpenPitAccountAdjustment};
use crate::account_outcome::{
    OpenPitAccountOutcomeEntryList, OpenPitPostTradeAdjustmentList, OpenPitPretradePreTradeResult,
};
use crate::execution_report::{export_execution_report, OpenPitExecutionReport};
use crate::order::{export_order, OpenPitOrder};
use crate::reject::{OpenPitPretradeAccountBlockList, OpenPitPretradeRejectList};
use crate::{AccountAdjustment, ExecutionReport, Order};

use crate::param::OpenPitParamAccountId;

use super::*;

/// Opaque context passed to the `apply_execution_report` C policy callback.
///
/// Valid only for the duration of the callback. Cannot be constructed by
/// caller code.
pub struct OpenPitPostTradeContext;

//--------------------------------------------------------------------------------------------------

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
/// - `out_result` is a callback-scoped non-owning collector the callback may
///   fill with lock prices and account adjustments via
///   `openpit_pretrade_pre_trade_result_push_lock_price` and
///   `openpit_pretrade_pre_trade_result_push_account_adjustment`. Neither push
///   carries a `policy_group_id`; the engine assigns the policy group. The
///   callback must not store or use `out_result` after return.
/// - The reject channel and the `out_result` channel are independent: a
///   callback may both reject and fill `out_result`, but the engine only keeps
///   `out_result` when the callback accepts (returns null or an empty list).
/// - Return null or an empty list to accept the order.
/// - Return a non-empty reject list to reject the order.
/// - Every returned reject must contain explicit `code` and `scope` values.
/// - The returned list ownership is transferred to the engine; create it with
///   `openpit_pretrade_create_reject_list`.
/// - Every reject payload is copied into internal storage before this callback
///   returns.
/// - `user_data` is passed through unchanged from policy creation.
///
/// Parameter ordering convention: read-only inputs first (`ctx`, `order`),
/// then callback-scoped collectors in the order
/// (`mutations`, `out_result`), then the trailing opaque `user_data`.
pub type OpenPitPretradePreTradePolicyPerformPreTradeCheckFn =
    unsafe extern "C" fn(
        ctx: *const OpenPitPretradeContext,
        order: *const OpenPitOrder,
        mutations: *mut OpenPitMutations,
        out_result: *mut OpenPitPretradePreTradeResult,
        user_data: *mut c_void,
    ) -> *mut OpenPitPretradeRejectList;

/// Callback used by a custom pre-trade policy to observe an execution report.
///
/// Contract:
/// - `ctx` is a read-only post-trade context valid only for the duration of
///   the callback. Use `openpit_post_trade_context_get_account_group` to query
///   the report account's group.
/// - `report` points to a read-only report view valid only for the duration of
///   the callback.
/// - `report` is passed as a borrowed view and is not copied before the
///   callback runs.
/// - If the callback wants to keep any data from `report`, it must copy that
///   data before returning.
/// - `out_adjustments` is a callback-scoped non-owning collector the callback
///   may fill with group-tagged account-adjustment outcomes via
///   `openpit_pretrade_post_trade_adjustment_list_push`. This channel IS
///   group-tagged. The callback must not store or use `out_adjustments` after
///   return.
/// - The account-block return and the `out_adjustments` channel are
///   independent: a callback may report blocks, adjustments, both, or neither.
/// - Return a non-null account-block list when this policy reports a
///   kill-switch trigger. The returned list ownership is transferred to the
///   engine; create it with `openpit_pretrade_create_account_block_list`.
/// - Return null to indicate no kill-switch condition.
/// - A null `apply_execution_report_fn` means that hook returns no blocks and
///   no adjustments.
/// - `user_data` is passed through unchanged from policy creation.
///
/// Parameter ordering convention: read-only context first (`ctx`), then
/// read-only input (`report`), then the callback-scoped collector
/// (`out_adjustments`), then the trailing opaque `user_data`.
pub type OpenPitPretradePreTradePolicyApplyExecutionReportFn =
    unsafe extern "C" fn(
        ctx: *const OpenPitPostTradeContext,
        report: *const OpenPitExecutionReport,
        out_adjustments: *mut OpenPitPostTradeAdjustmentList,
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
/// - `out_outcomes` is a callback-scoped non-owning collector the callback may
///   fill with account-outcome entries via
///   `openpit_account_outcome_entry_list_push`. No `policy_group_id` is carried;
///   the engine assigns the policy group. The callback must not store or use
///   `out_outcomes` after return.
/// - The reject channel and the `out_outcomes` channel are independent: the
///   engine only keeps `out_outcomes` when the callback accepts (returns null
///   or an empty list).
/// - Return null to accept the adjustment.
/// - Return a non-empty reject list to reject the adjustment.
/// - Returned reject list ownership is transferred to the callee.
/// - `user_data` is passed through unchanged from policy creation.
///
/// Parameter ordering convention: read-only inputs first (`ctx`, `account_id`,
/// `adjustment`), then callback-scoped collectors in the order
/// (`mutations`, `out_outcomes`), then the trailing opaque `user_data`.
pub type OpenPitPretradePreTradePolicyApplyAccountAdjustmentFn =
    unsafe extern "C" fn(
        ctx: *const OpenPitAccountAdjustmentContext,
        account_id: OpenPitParamAccountId,
        adjustment: *const OpenPitAccountAdjustment,
        mutations: *mut OpenPitMutations,
        out_outcomes: *mut OpenPitAccountOutcomeEntryList,
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

pub(super) struct CustomPreTradePolicy {
    pub(super) name: String,
    pub(super) policy_group_id: PolicyGroupId,
    pub(super) check_pre_trade_start_fn: Option<OpenPitPretradePreTradePolicyCheckPreTradeStartFn>,
    pub(super) perform_pre_trade_check_fn:
        Option<OpenPitPretradePreTradePolicyPerformPreTradeCheckFn>,
    pub(super) apply_execution_report_fn:
        Option<OpenPitPretradePreTradePolicyApplyExecutionReportFn>,
    pub(super) apply_account_adjustment_fn:
        Option<OpenPitPretradePreTradePolicyApplyAccountAdjustmentFn>,
    pub(super) free_user_data_fn: OpenPitPretradePreTradePolicyFreeUserDataFn,
    pub(super) user_data: *mut c_void,
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
// - Under `SyncMode::None` or `SyncMode::Account` the binding caller
//   serialises per-handle invocation; under `SyncMode::Full` the caller is
//   responsible for making any state reachable through user_data safe under
//   concurrent invocation.
//
// Violating the user_data contract is undefined behavior at the contract level.
unsafe impl Send for CustomPreTradePolicy {}
unsafe impl Sync for CustomPreTradePolicy {}

type StorageFactory = openpit_interop::StorageLockingPolicyFactory;

impl PreTradePolicy<Order, ExecutionReport, AccountAdjustment, openpit_interop::EngineLocking>
    for CustomPreTradePolicy
{
    fn name(&self) -> &str {
        &self.name
    }

    fn policy_group_id(&self) -> PolicyGroupId {
        self.policy_group_id
    }

    fn check_pre_trade_start(
        &self,
        ctx: &PreTradeContext<StorageFactory>,
        order: &Order,
    ) -> Result<(), Rejects> {
        let Some(check_fn) = self.check_pre_trade_start_fn else {
            return Ok(());
        };
        let input = export_order(order);
        let c_ctx =
            (ctx as *const PreTradeContext<StorageFactory>).cast::<OpenPitPretradeContext>();
        let rejects = unsafe { check_fn(c_ctx, &input, self.user_data) };
        import_reject_list_result(rejects)
    }

    fn perform_pre_trade_check(
        &self,
        ctx: &PreTradeContext<StorageFactory>,
        order: &Order,
        mutations: &mut Mutations,
    ) -> Result<Option<PolicyPreTradeResult>, Rejects> {
        let Some(check_fn) = self.perform_pre_trade_check_fn else {
            return Ok(None);
        };
        let mut mutations_handle = OpenPitMutations {
            mutations: mutations as *mut Mutations,
        };
        let mut out_result = OpenPitPretradePreTradeResult {
            lock_prices: Vec::new(),
            account_adjustments: Vec::new(),
        };
        let input = export_order(order);
        let c_ctx =
            (ctx as *const PreTradeContext<StorageFactory>).cast::<OpenPitPretradeContext>();
        let rejects = unsafe {
            check_fn(
                c_ctx,
                &input,
                &mut mutations_handle,
                &mut out_result,
                self.user_data,
            )
        };
        import_reject_list_result(rejects)?;
        if out_result.lock_prices.is_empty() && out_result.account_adjustments.is_empty() {
            Ok(None)
        } else {
            Ok(Some(PolicyPreTradeResult {
                account_adjustments: out_result.account_adjustments.into(),
                lock_prices: out_result.lock_prices.into(),
            }))
        }
    }

    fn apply_execution_report(
        &self,
        ctx: &PostTradeContext<StorageFactory>,
        report: &ExecutionReport,
    ) -> Option<PostTradeResult> {
        let apply_fn = self.apply_execution_report_fn?;
        let input = export_execution_report(report);
        let mut out_adjustments = OpenPitPostTradeAdjustmentList { items: Vec::new() };
        let c_ctx =
            (ctx as *const PostTradeContext<StorageFactory>).cast::<OpenPitPostTradeContext>();
        let raw = unsafe { apply_fn(c_ctx, &input, &mut out_adjustments, self.user_data) };
        let account_blocks = if raw.is_null() {
            Vec::new()
        } else {
            unsafe { Box::from_raw(raw) }.items
        };
        let account_adjustments = out_adjustments.items;
        if account_blocks.is_empty() && account_adjustments.is_empty() {
            None
        } else {
            Some(PostTradeResult {
                account_blocks,
                account_adjustments,
            })
        }
    }

    fn apply_account_adjustment(
        &self,
        ctx: &AccountAdjustmentContext<StorageFactory>,
        account_id: openpit::param::AccountId,
        adjustment: &AccountAdjustment,
        mutations: &mut Mutations,
    ) -> Result<Vec<AccountOutcomeEntry>, Rejects> {
        let Some(apply_fn) = self.apply_account_adjustment_fn else {
            return Ok(vec![]);
        };
        let mut mutations_handle = OpenPitMutations {
            mutations: mutations as *mut Mutations,
        };
        let mut out_outcomes = OpenPitAccountOutcomeEntryList { items: Vec::new() };
        let input = export_account_adjustment(adjustment);
        let c_ctx = (ctx as *const AccountAdjustmentContext<StorageFactory>)
            .cast::<OpenPitAccountAdjustmentContext>();
        let rejects = unsafe {
            apply_fn(
                c_ctx,
                account_id.as_u64(),
                &input,
                &mut mutations_handle,
                &mut out_outcomes,
                self.user_data,
            )
        };
        import_reject_list_result(rejects)?;
        Ok(out_outcomes.items)
    }
}

impl Drop for CustomPreTradePolicy {
    fn drop(&mut self) {
        unsafe { (self.free_user_data_fn)(self.user_data) };
    }
}

//--------------------------------------------------------------------------------------------------

pub(super) unsafe fn parse_policy_name(
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

pub(super) fn import_reject_list_result(
    rejects: *mut OpenPitPretradeRejectList,
) -> Result<(), Rejects> {
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
/// - `policy_group_id` is the policy-group tag the engine embeds in every
///   account adjustment outcome this policy produces. Use `0` for the default
///   group.
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
///   the caller's responsibility. Under `OpenPitSyncPolicy_None` or
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
    policy_group_id: u16,
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
        policy_group_id: PolicyGroupId::new(policy_group_id),
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

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::ffi::c_void;
    use std::rc::Rc;

    use openpit::param::{AccountId, Asset, Quantity, Side, TradeAmount};
    use openpit::Instrument;
    use openpit_interop::{OrderOperationAccess, PopulatedOrderOperation};

    use super::*;

    use crate::reject::{OpenPitPretradeAccountBlockList, OpenPitPretradeRejectList};

    unsafe extern "C" fn custom_check_fn(
        _ctx: *const OpenPitPretradeContext,
        _order: *const OpenPitOrder,
        _user_data: *mut c_void,
    ) -> *mut OpenPitPretradeRejectList {
        std::ptr::null_mut()
    }

    unsafe extern "C" fn custom_apply_report_fn(
        _ctx: *const OpenPitPostTradeContext,
        _report: *const OpenPitExecutionReport,
        _out_adjustments: *mut OpenPitPostTradeAdjustmentList,
        _user_data: *mut c_void,
    ) -> *mut OpenPitPretradeAccountBlockList {
        std::ptr::null_mut()
    }

    unsafe extern "C" fn custom_free_user_data_fn(_user_data: *mut c_void) {}
    unsafe extern "C" fn custom_pre_trade_check_fn(
        _ctx: *const OpenPitPretradeContext,
        _order: *const OpenPitOrder,
        _mutations: *mut OpenPitMutations,
        _out_result: *mut OpenPitPretradePreTradeResult,
        _user_data: *mut c_void,
    ) -> *mut OpenPitPretradeRejectList {
        std::ptr::null_mut()
    }

    unsafe extern "C" fn custom_account_adjustment_apply_fn(
        _ctx: *const OpenPitAccountAdjustmentContext,
        _account_id: OpenPitParamAccountId,
        _adjustment: *const OpenPitAccountAdjustment,
        _mutations: *mut OpenPitMutations,
        _out_outcomes: *mut OpenPitAccountOutcomeEntryList,
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
                0,
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
                0,
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

    #[derive(Default)]
    struct MutationState {
        commit_calls: usize,
        free_calls: usize,
    }

    struct MutationUserData {
        state: Rc<RefCell<MutationState>>,
        #[allow(dead_code)]
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
            .pre_trade(CustomPreTradePolicy {
                name: "ffi.custom".to_owned(),
                policy_group_id: PolicyGroupId::new(0),
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
        let data = unsafe { &*(user_data as *const MutationUserData) };
        data.state.borrow_mut().commit_calls += 1;
    }

    unsafe extern "C" fn tracked_mutation_rollback(_user_data: *mut c_void) {}

    unsafe extern "C" fn tracked_mutation_free(user_data: *mut c_void) {
        let data = unsafe { Box::from_raw(user_data as *mut MutationUserData) };
        data.state.borrow_mut().free_calls += 1;
    }

    unsafe extern "C" fn push_tracked_mutations_check_fn(
        _ctx: *const OpenPitPretradeContext,
        _order: *const OpenPitOrder,
        mutations: *mut OpenPitMutations,
        _out_result: *mut OpenPitPretradePreTradeResult,
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
                0,
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
                0,
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

        let engine = crate::engine::openpit_engine_builder_build(
            builder,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
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

        let engine = crate::engine::openpit_engine_builder_build(
            builder,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
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
                0,
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

        let engine = crate::engine::openpit_engine_builder_build(
            builder,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
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
}
