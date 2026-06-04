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

use openpit::pretrade::PreTradeContext;
use openpit::AccountAdjustmentContext;

use crate::policy::{OpenPitAccountAdjustmentContext, OpenPitPretradeContext};
use crate::reject::OpenPitPretradeAccountBlock;

type StorageFactory = openpit_interop::StorageLockingPolicyFactory;

/// Opaque handle to the per-account blocking facility bound to one account.
///
/// What it is:
/// - A caller-owned handle that records a block against a single, already-bound
///   account on the engine's shared blocked-accounts facility.
///
/// Why it exists:
/// - It lets a policy callback both block the bound account immediately and
///   retain the ability to block it later within the same pre-trade
///   transaction, for example from a deferred commit or rollback callback that
///   has no other channel to surface a block.
///
/// Lifetime contract:
/// - Every handle returned to the caller is owned by the caller and MUST be
///   released with `openpit_destroy_account_control` exactly once.
/// - A handle is valid to use ONLY within the pre-trade processing of the
///   request it belongs to — from the callback that produced it through the
///   commit or rollback of that request's reservation. Recording a block
///   through it after that pre-trade transaction has completed is undefined
///   behaviour.
pub struct OpenPitAccountControl {
    inner: openpit::AccountControl<StorageFactory>,
}

impl OpenPitAccountControl {
    fn into_raw(inner: openpit::AccountControl<StorageFactory>) -> *mut Self {
        Box::into_raw(Box::new(Self { inner }))
    }
}

//--------------------------------------------------------------------------------------------------

#[no_mangle]
/// Records a block against the account bound to an account-control handle.
///
/// Records `block` against the bound account on the engine's shared
/// blocked-accounts facility. The first cause recorded for an account wins;
/// later calls for the same account are no-ops.
///
/// Contract:
/// - `control` must be a valid non-null account-control handle, or null.
/// - `block` payload fields are copied into internal storage before this call
///   returns.
/// - Passing a null `control` records nothing and has no effect.
///
/// # Safety
///
/// `control` must be either null or a valid account-control handle provided by
/// this library.
pub unsafe extern "C" fn openpit_account_control_block(
    control: *const OpenPitAccountControl,
    block: OpenPitPretradeAccountBlock,
) {
    if control.is_null() {
        return;
    }
    let control = unsafe { &*control };
    control.inner.block(block.to_block());
}

#[no_mangle]
/// Returns a new handle referring to the same account-control facility.
///
/// Use this to retain the ability to block the bound account from a later
/// callback within the same pre-trade transaction. The returned handle records
/// blocks against the same account as the source handle and shares its validity
/// window: it is valid to use only within that pre-trade transaction, and is
/// undefined afterwards.
///
/// Success:
/// - returns a non-null caller-owned handle to the same facility.
///
/// Error:
/// - returns null when `control` is null.
///
/// Cleanup:
/// - the returned handle MUST be released with
///   `openpit_destroy_account_control` exactly once.
///
/// # Safety
///
/// `control` must be either null or a valid account-control handle provided by
/// this library.
pub unsafe extern "C" fn openpit_account_control_clone(
    control: *const OpenPitAccountControl,
) -> *mut OpenPitAccountControl {
    if control.is_null() {
        return std::ptr::null_mut();
    }
    let control = unsafe { &*control };
    OpenPitAccountControl::into_raw(control.inner.clone())
}

#[no_mangle]
/// Releases a caller-owned account-control handle.
///
/// Lifetime contract:
/// - Call this exactly once for each handle that was returned to the caller.
/// - After this call the handle is no longer valid.
/// - Passing a null pointer is allowed and has no effect.
/// - This function always succeeds.
pub extern "C" fn openpit_destroy_account_control(control: *mut OpenPitAccountControl) {
    if control.is_null() {
        return;
    }
    unsafe { drop(Box::from_raw(control)) };
}

//--------------------------------------------------------------------------------------------------

#[no_mangle]
/// Returns an account-control handle for a main-stage pre-trade context.
///
/// A main-stage pre-trade context carries account control only when an account
/// could be bound to the request.
///
/// Contract:
/// - `ctx` must be the callback-scoped context pointer passed to a custom
///   main-stage pre-trade callback; it is valid only for the duration of that
///   callback.
///
/// Success:
/// - returns a non-null caller-owned handle when the context carries account
///   control.
///
/// Error:
/// - returns null when `ctx` is null or the context carries no account control
///   (no account could be bound).
///
/// Cleanup:
/// - the returned handle MUST be released with
///   `openpit_destroy_account_control` exactly once. It may be retained for
///   deferred blocking, but it is valid to use only within the pre-trade
///   transaction of this request — through the commit or rollback of its
///   reservation; recording a block through it afterwards is undefined.
///
/// # Safety
///
/// `ctx` must be either null or a valid callback-scoped pre-trade context
/// pointer provided to this library.
pub unsafe extern "C" fn openpit_pretrade_context_get_account_control(
    ctx: *const OpenPitPretradeContext,
) -> *mut OpenPitAccountControl {
    if ctx.is_null() {
        return std::ptr::null_mut();
    }
    let ctx = unsafe { &*ctx.cast::<PreTradeContext<StorageFactory>>() };
    match ctx.account_control.as_ref() {
        Some(account_control) => OpenPitAccountControl::into_raw(account_control.clone()),
        None => std::ptr::null_mut(),
    }
}

#[no_mangle]
/// Returns an account-control handle for an account-adjustment context.
///
/// An account-adjustment context always carries account control, so this call
/// returns a non-null handle for any valid context.
///
/// Contract:
/// - `ctx` must be the callback-scoped context pointer passed to a custom
///   account-adjustment callback; it is valid only for the duration of that
///   callback.
///
/// Success:
/// - returns a non-null caller-owned handle.
///
/// Error:
/// - returns null when `ctx` is null.
///
/// Cleanup:
/// - the returned handle MUST be released with
///   `openpit_destroy_account_control` exactly once. It may be retained for
///   deferred blocking, but it is valid to use only within the account
///   adjustment processing of this request — through the commit or rollback of
///   that request; recording a block through it afterwards is undefined.
///
/// # Safety
///
/// `ctx` must be either null or a valid callback-scoped account-adjustment
/// context pointer provided to this library.
pub unsafe extern "C" fn openpit_account_adjustment_context_get_account_control(
    ctx: *const OpenPitAccountAdjustmentContext,
) -> *mut OpenPitAccountControl {
    if ctx.is_null() {
        return std::ptr::null_mut();
    }
    let ctx = unsafe { &*ctx.cast::<AccountAdjustmentContext<StorageFactory>>() };
    OpenPitAccountControl::into_raw(ctx.account_control.clone())
}

//--------------------------------------------------------------------------------------------------

#[no_mangle]
/// Returns the account-group for a main-stage pre-trade context.
///
/// Looks up the group registered for the bound order account. The result is
/// cached on first call and reused for subsequent calls within the same
/// context lifetime.
///
/// Contract:
/// - `ctx` must be the callback-scoped context pointer passed to a custom
///   main-stage pre-trade callback; it is valid only for the duration of that
///   callback.
/// - `out_group` must be a valid non-null pointer.
///
/// Success:
/// - returns `true` and writes the group to `out_group` when the account is
///   registered in a group;
/// - returns `false` when `ctx` is null, no account was bound to the request,
///   or the account belongs to no group; `out_group` is not written to.
///
/// # Safety
///
/// `ctx` must be either null or a valid callback-scoped pre-trade context
/// pointer provided to this library.
pub unsafe extern "C" fn openpit_pretrade_context_get_account_group(
    ctx: *const crate::policy::OpenPitPretradeContext,
    out_group: *mut crate::account_group_id::OpenPitParamAccountGroupId,
) -> bool {
    if ctx.is_null() {
        return false;
    }
    assert!(!out_group.is_null(), "out_group is null");
    let ctx = unsafe { &*ctx.cast::<PreTradeContext<StorageFactory>>() };
    match ctx.account_group() {
        Some(group) => {
            unsafe { *out_group = group.as_u32() };
            true
        }
        None => false,
    }
}

#[no_mangle]
/// Returns the account-group for an account-adjustment context.
///
/// Looks up the group registered for the adjusted account. The result is
/// cached on first call and reused for subsequent calls within the same
/// context lifetime.
///
/// Contract:
/// - `ctx` must be the callback-scoped context pointer passed to a custom
///   account-adjustment callback; it is valid only for the duration of that
///   callback.
/// - `out_group` must be a valid non-null pointer.
///
/// Success:
/// - returns `true` and writes the group to `out_group` when the account is
///   registered in a group;
/// - returns `false` when `ctx` is null or the account belongs to no group;
///   `out_group` is not written to.
///
/// # Safety
///
/// `ctx` must be either null or a valid callback-scoped account-adjustment
/// context pointer provided to this library.
pub unsafe extern "C" fn openpit_account_adjustment_context_get_account_group(
    ctx: *const crate::policy::OpenPitAccountAdjustmentContext,
    out_group: *mut crate::account_group_id::OpenPitParamAccountGroupId,
) -> bool {
    if ctx.is_null() {
        return false;
    }
    assert!(!out_group.is_null(), "out_group is null");
    let ctx = unsafe { &*ctx.cast::<openpit::AccountAdjustmentContext<StorageFactory>>() };
    match ctx.account_group() {
        Some(group) => {
            unsafe { *out_group = group.as_u32() };
            true
        }
        None => false,
    }
}

#[no_mangle]
/// Returns the account-group for a post-trade context.
///
/// Looks up the group registered for the report's account. The result is
/// cached on first call and reused for subsequent calls within the same
/// context lifetime.
///
/// Contract:
/// - `ctx` must be the callback-scoped context pointer passed to a custom
///   `apply_execution_report` callback; it is valid only for the duration of
///   that callback.
/// - `out_group` must be a valid non-null pointer.
///
/// Success:
/// - returns `true` and writes the group to `out_group` when the account is
///   registered in a group;
/// - returns `false` when `ctx` is null or the account belongs to no group;
///   `out_group` is not written to.
///
/// # Safety
///
/// `ctx` must be either null or a valid callback-scoped post-trade context
/// pointer provided to this library.
pub unsafe extern "C" fn openpit_post_trade_context_get_account_group(
    ctx: *const crate::policy::custom::OpenPitPostTradeContext,
    out_group: *mut crate::account_group_id::OpenPitParamAccountGroupId,
) -> bool {
    if ctx.is_null() {
        return false;
    }
    assert!(!out_group.is_null(), "out_group is null");
    let ctx = unsafe { &*ctx.cast::<openpit::pretrade::PostTradeContext<StorageFactory>>() };
    match ctx.account_group() {
        Some(group) => {
            unsafe { *out_group = group.as_u32() };
            true
        }
        None => false,
    }
}
