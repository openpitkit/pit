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

use crate::account_adjustment::{
    import_account_adjustment, AccountAdjustment, OpenPitAccountAdjustment,
    OpenPitAccountAdjustmentApplyStatus,
};
use crate::execution_report::{import_execution_report, ExecutionReport, OpenPitExecutionReport};
use crate::last_error::{write_error, OpenPitOutError};
use crate::order::{import_order, OpenPitOrder, Order};
use crate::param::{OpenPitParamAccountId, OpenPitParamPrice, OpenPitParamPriceOptional};
use crate::reject::{
    blocks_to_list_owned, rejects_to_list_owned, OpenPitPretradeAccountBlockList,
    OpenPitPretradeRejectList,
};
use crate::write_error_format;
use openpit::param::AccountId;

//--------------------------------------------------------------------------------------------------

type EngineTrait = openpit_interop::InteropEngineTrait<Order, ExecutionReport, AccountAdjustment>;
type Engine = openpit::Engine<EngineTrait>;

pub(crate) enum BuilderState {
    Synced(
        openpit::SyncedEngineBuilder<
            Order,
            ExecutionReport,
            AccountAdjustment,
            openpit_interop::EngineLocking,
        >,
    ),
    Ready(
        openpit::ReadyEngineBuilder<
            Order,
            ExecutionReport,
            AccountAdjustment,
            openpit_interop::EngineLocking,
        >,
    ),
}

#[allow(dead_code, unused_imports)]
pub use openpit_interop::SyncMode as OpenPitSyncPolicy;

//--------------------------------------------------------------------------------------------------
// Threading:
// The SDK never spawns OS threads: each public call executes on the OS thread
// that invoked it. Full sync permits concurrent public calls on the same
// handle. Local sync keeps the handle on the OS thread that created it. Account
// sync permits sequential cross-thread access, but the caller must pin each
// account to a single processing chain and must not invoke public methods on
// the same handle concurrently. In Go bindings, goroutine migration during one
// SDK call is supported, and callbacks into Go may run on a different OS thread
// than the goroutine that initiated the call; callback code must not rely on
// thread-local OS state.

/// Opaque builder pointer used to assemble an engine instance.
///
/// Ownership:
/// - returned by `openpit_create_engine_builder`;
/// - owned by the caller until passed to `openpit_destroy_engine_builder`;
/// - consumed by `openpit_engine_builder_build`.
pub struct OpenPitEngineBuilder {
    pub(crate) inner: Option<BuilderState>,
}

/// Opaque engine pointer.
///
/// The engine stores policies and mutable risk state. The caller owns the
/// pointer until `openpit_destroy_engine`.
pub struct OpenPitEngine {
    inner: Engine,
}

/// Opaque pointer for a deferred pre-trade request.
///
/// This is returned by `openpit_engine_start_pre_trade`. It can be executed once
/// with `openpit_pretrade_pre_trade_request_execute` or discarded with
/// `openpit_destroy_pretrade_pre_trade_request`.
pub struct OpenPitPretradePreTradeRequest {
    inner: Option<openpit::pretrade::PreTradeRequest<Order>>,
}

/// Opaque reservation pointer returned by a successful pre-trade check.
///
/// A reservation represents resources that have been tentatively locked. The
/// caller must resolve it exactly once by calling `openpit_pretrade_pre_trade_reservation_commit`,
/// `openpit_pretrade_pre_trade_reservation_rollback`, or `openpit_destroy_pretrade_pre_trade_reservation`.
pub struct OpenPitPretradePreTradeReservation {
    inner: openpit::pretrade::PreTradeReservation,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
/// Price-lock snapshot returned from a reservation.
pub struct OpenPitPretradePreTradeLock {
    /// Optional reserved price.
    pub price: OpenPitParamPriceOptional,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Result status for pre-trade operations.
pub enum OpenPitPretradeStatus {
    /// Order/request passed this stage; read the success out-pointer.
    Passed = 0,
    /// Order/request was rejected; read the reject out-pointer.
    Rejected = 1,
    /// Call failed due to invalid input; read the error out-pointer.
    Error = 2,
}

/// Batch rejection details returned by account-adjustment apply API.
///
/// Ownership:
/// - created by `openpit_engine_apply_account_adjustment` on `Rejected`;
/// - owned by the caller;
/// - released with `openpit_destroy_account_adjustment_batch_error`.
pub struct OpenPitAccountAdjustmentBatchError {
    /// Rejects produced by the policy.
    rejects: OpenPitPretradeRejectList,
    /// Zero-based index of the failing adjustment.
    failed_adjustment_index: usize,
}

//--------------------------------------------------------------------------------------------------

pub(crate) fn add_pre_trade_policy_to_builder(
    builder: &mut OpenPitEngineBuilder,
    policy: impl openpit::pretrade::PreTradePolicy<Order, ExecutionReport, AccountAdjustment>
        + Send
        + 'static,
) -> Result<(), String> {
    let state = builder
        .inner
        .take()
        .ok_or_else(|| "engine builder is no longer available".to_string())?;
    builder.inner = Some(match state {
        BuilderState::Synced(b) => BuilderState::Ready(b.pre_trade(policy)),
        BuilderState::Ready(b) => BuilderState::Ready(b.pre_trade(policy)),
    });
    Ok(())
}

impl OpenPitAccountAdjustmentBatchError {
    fn new(err: openpit::AccountAdjustmentBatchError) -> Self {
        Self {
            failed_adjustment_index: err.failed_adjustment_index,
            rejects: rejects_to_list_owned(err.rejects),
        }
    }
}

fn export_pre_trade_lock(lock: &openpit::pretrade::PreTradeLock) -> OpenPitPretradePreTradeLock {
    OpenPitPretradePreTradeLock {
        price: match lock.price() {
            Some(v) => OpenPitParamPriceOptional {
                is_set: true,
                value: OpenPitParamPrice(v.to_decimal().into()),
            },
            None => OpenPitParamPriceOptional::default(),
        },
    }
}

#[no_mangle]
/// Creates a new engine builder with the chosen synchronization policy.
///
/// Success:
/// - returns a non-null caller-owned builder object.
///
/// Error:
/// - returns null when `sync_policy` is not one of `OpenPitSyncPolicy_Full` (0),
///   `OpenPitSyncPolicy_Local` (1), or `OpenPitSyncPolicy_Account` (2);
/// - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
///   error handle that MUST be released with `openpit_destroy_shared_string`.
///
/// Cleanup:
/// - release the pointer with `openpit_destroy_engine_builder` if you stop before
///   building;
/// - after a successful build the builder is consumed and must still be
///   released with `openpit_destroy_engine_builder`.
pub extern "C" fn openpit_create_engine_builder(
    sync_policy: u8,
    out_error: OpenPitOutError,
) -> *mut OpenPitEngineBuilder {
    // The argument is a raw `u8`, not `OpenPitSyncPolicy`, on purpose. `OpenPitSyncPolicy`
    // is a `#[repr(u8)] enum` with only 0, 1, 2 valid; passing any other byte in
    // a variable typed as that enum is undefined behavior at the FFI boundary,
    // before any Rust statement of this function runs. Validating after the
    // fact via `if x > 2` would already be too late. We accept the primitive
    // and translate via `match` here, where the input has no invariants yet.
    let mode = match sync_policy {
        0 => openpit_interop::SyncMode::Full,
        1 => openpit_interop::SyncMode::Local,
        2 => openpit_interop::SyncMode::Account,
        invalid => {
            write_error_format!(
                out_error,
                "openpit_create_engine_builder: invalid sync_policy byte {}, expected 0..=2",
                invalid
            );
            return std::ptr::null_mut();
        }
    };

    let state = BuilderState::Synced(
        openpit::EngineBuilder::<Order, ExecutionReport, AccountAdjustment>::new()
            .sync(openpit_interop::EngineLocking::new(mode)),
    );
    Box::into_raw(Box::new(OpenPitEngineBuilder { inner: Some(state) }))
}

#[no_mangle]
/// Releases a builder pointer owned by the caller.
///
/// Contract:
/// - passing null is allowed;
/// - after this call the pointer is invalid;
/// - this function always succeeds.
pub extern "C" fn openpit_destroy_engine_builder(builder: *mut OpenPitEngineBuilder) {
    if builder.is_null() {
        return;
    }
    unsafe { drop(Box::from_raw(builder)) };
}

#[no_mangle]
/// Finalizes a builder and creates an engine.
///
/// Success:
/// - returns a non-null engine pointer.
///
/// Error:
/// - returns null when `builder` is null, the builder was already consumed, or
///   configuration is invalid;
/// - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
///   error handle that MUST be released with `openpit_destroy_shared_string`.
///
/// Ownership:
/// - on success the returned engine pointer is owned by the caller and must be
///   released with `openpit_destroy_engine`;
/// - the builder becomes consumed regardless of success and must not be reused.
pub extern "C" fn openpit_engine_builder_build(
    builder: *mut OpenPitEngineBuilder,
    out_error: OpenPitOutError,
) -> *mut OpenPitEngine {
    if builder.is_null() {
        write_error(out_error, "engine builder is null");
        return std::ptr::null_mut();
    }

    let builder = unsafe { &mut *builder };
    let state = match builder.inner.take() {
        Some(v) => v,
        None => {
            write_error(out_error, "engine builder already consumed");
            return std::ptr::null_mut();
        }
    };
    let result = match state {
        BuilderState::Ready(b) => b.build(),
        BuilderState::Synced(_) => {
            write_error(out_error, "no policies registered");
            return std::ptr::null_mut();
        }
    };
    match result {
        Ok(engine) => Box::into_raw(Box::new(OpenPitEngine { inner: engine })),
        Err(err) => {
            write_error(out_error, &err.to_string());
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
/// Releases an engine pointer owned by the caller.
///
/// Contract:
/// - passing null is allowed;
/// - destroying the engine also releases any state and policies retained by
///   that engine instance;
/// - this function always succeeds.
pub extern "C" fn openpit_destroy_engine(engine: *mut OpenPitEngine) {
    if engine.is_null() {
        return;
    }
    unsafe { drop(Box::from_raw(engine)) };
}

#[no_mangle]
/// Starts pre-trade processing and returns a deferred request pointer.
///
/// This stage validates whether the order can enter the full pre-trade flow.
///
/// Success:
/// - returns `Passed` when the order passed this stage; read `out_request`;
/// - returns `Rejected` when the order was rejected; read `out_rejects` if not
///   null.
///
/// Error:
/// - returns `Error` when input pointers are invalid or the order payload
///   cannot be decoded;
/// - on `Error`, if `out_error` is not null, it is filled with a
///   caller-owned `OpenPitSharedString` that MUST be destroyed by the caller.
///
/// Cleanup:
/// - release a successful request with `openpit_pretrade_pre_trade_request_execute` or
///   `openpit_destroy_pretrade_pre_trade_request`.
///
/// Reject ownership contract:
/// - on `Rejected`, a non-null `OpenPitPretradeRejectList` pointer is written to `out_rejects`
///   if it is not null;
/// - the caller takes ownership and MUST release it with
///   `openpit_pretrade_destroy_reject_list`; failing to do so leaks the heap allocation made
///   inside this call;
/// - no thread-local state is involved, and the returned pointer is safe to
///   read on any thread;
/// - on `Passed` and `Error`, null is written to `out_rejects`, and the caller
///   must not call destroy in those cases.
///
/// Order lifetime contract:
/// - `order` is read as a borrowed view during this call;
/// - the operation snapshots that payload before returning, because the
///   deferred request may outlive the source buffers.
pub extern "C" fn openpit_engine_start_pre_trade(
    engine: *mut OpenPitEngine,
    order: *const OpenPitOrder,
    out_request: *mut *mut OpenPitPretradePreTradeRequest,
    out_rejects: *mut *mut OpenPitPretradeRejectList,
    out_error: OpenPitOutError,
) -> OpenPitPretradeStatus {
    if engine.is_null() {
        write_error(out_error, "engine is null");
        return OpenPitPretradeStatus::Error;
    }
    if order.is_null() {
        write_error(out_error, "order is null");
        return OpenPitPretradeStatus::Error;
    }

    // `start_pre_trade` stores the request for later execution, so the order
    // must become owned data before this function returns.
    let order = match import_order(unsafe { &*order }) {
        Ok(v) => v,
        Err(e) => {
            write_error(out_error, &e);
            return OpenPitPretradeStatus::Error;
        }
    };

    match unsafe { &*engine }.inner.start_pre_trade(order) {
        Ok(request) => {
            if !out_request.is_null() {
                unsafe {
                    *out_request = Box::into_raw(Box::new(OpenPitPretradePreTradeRequest {
                        inner: Some(request),
                    }))
                }
            }
            OpenPitPretradeStatus::Passed
        }
        Err(rejects) => {
            if !out_rejects.is_null() {
                let OpenPitPretradeRejectList { items } = rejects_to_list_owned(rejects);
                unsafe {
                    *out_rejects = Box::into_raw(Box::new(OpenPitPretradeRejectList { items }));
                }
            }
            OpenPitPretradeStatus::Rejected
        }
    }
}

#[no_mangle]
/// Runs the complete pre-trade check in one call.
///
/// Success:
/// - returns `Passed` when the order passed this stage; read `out_reservation`;
/// - returns `Rejected` when the order was rejected is not null; read
///   `out_rejects`.
///
/// Error:
/// - returns `Error` when input pointers are invalid or the order payload
///   cannot be decoded;
/// - on `Error`, if `out_error` is not null, it is filled with a
///   caller-owned `OpenPitSharedString` that MUST be destroyed by the caller.
///
/// Cleanup:
/// - release a successful reservation with `openpit_pretrade_pre_trade_reservation_commit`,
///   `openpit_pretrade_pre_trade_reservation_rollback`, or
///   `openpit_destroy_pretrade_pre_trade_reservation`.
///
/// Reject ownership contract:
/// - on `Rejected`, a non-null `OpenPitPretradeRejectList` pointer is written to
///   `out_rejects` if it is not null;
/// - the caller takes ownership and MUST release it with
///   `openpit_pretrade_destroy_reject_list`; failing to do so leaks the heap allocation made
///   inside this call;
/// - no thread-local state is involved, and the returned pointer is safe to
///   read on any thread;
/// - on `Passed` and `Error`, null is written to `out_rejects`, and the caller
///   must not call destroy in those cases.
///
/// Order lifetime contract:
/// - `order` is read as a borrowed view during this call only;
/// - the operation does not retain any pointer into source memory after this
///   function returns.
pub extern "C" fn openpit_engine_execute_pre_trade(
    engine: *mut OpenPitEngine,
    order: *const OpenPitOrder,
    out_reservation: *mut *mut OpenPitPretradePreTradeReservation,
    out_rejects: *mut *mut OpenPitPretradeRejectList,
    out_error: OpenPitOutError,
) -> OpenPitPretradeStatus {
    if engine.is_null() {
        write_error(out_error, "engine is null");
        return OpenPitPretradeStatus::Error;
    }
    if order.is_null() {
        write_error(out_error, "order is null");
        return OpenPitPretradeStatus::Error;
    }

    let order = match import_order(unsafe { &*order }) {
        Ok(v) => v,
        Err(e) => {
            write_error(out_error, &e);
            return OpenPitPretradeStatus::Error;
        }
    };

    match unsafe { &*engine }.inner.execute_pre_trade(order) {
        Ok(reservation) => {
            if !out_reservation.is_null() {
                unsafe {
                    *out_reservation = Box::into_raw(Box::new(OpenPitPretradePreTradeReservation {
                        inner: reservation,
                    }))
                }
            }
            OpenPitPretradeStatus::Passed
        }
        Err(rejects) => {
            if !out_rejects.is_null() {
                let OpenPitPretradeRejectList { items } = rejects_to_list_owned(rejects);
                unsafe {
                    *out_rejects = Box::into_raw(Box::new(OpenPitPretradeRejectList { items }));
                }
            }
            OpenPitPretradeStatus::Rejected
        }
    }
}

#[no_mangle]
/// Executes a deferred request returned by `openpit_engine_start_pre_trade`.
///
/// Success:
/// - returns `Passed` when the order passed this stage; read `out_reservation`;
/// - returns `Rejected` when the order was rejected and `out_rejects` is not
///   null; read `out_rejects`.
///
/// Error:
/// - returns `Error` when input pointers are invalid or the order payload
///   cannot be decoded;
/// - on `Error`, if `out_error` is not null, it is filled with a
///   caller-owned `OpenPitSharedString` that MUST be destroyed by the caller.
///
/// Ownership:
/// - this call consumes the request object's content exactly once;
/// - after a successful or failed execute, the object itself may still
///   be released with `openpit_destroy_pretrade_pre_trade_request`, but it cannot be executed again.
///
/// Reject ownership contract:
/// - on `Rejected`, a non-null `OpenPitPretradeRejectList` pointer is written to
///   `out_rejects` if it is not null;
/// - the caller takes ownership and MUST release it with
///   `openpit_pretrade_destroy_reject_list`; failing to do so leaks the heap allocation made
///   inside this call;
/// - no thread-local state is involved, and the returned pointer is safe to
///   read on any thread;
/// - on `Passed` and `Error`, null is written to `out_rejects`, and the caller
///   must not call destroy in those cases.
pub extern "C" fn openpit_pretrade_pre_trade_request_execute(
    request: *mut OpenPitPretradePreTradeRequest,
    out_reservation: *mut *mut OpenPitPretradePreTradeReservation,
    out_rejects: *mut *mut OpenPitPretradeRejectList,
    out_error: OpenPitOutError,
) -> OpenPitPretradeStatus {
    if request.is_null() {
        write_error(out_error, "request is null");
        return OpenPitPretradeStatus::Error;
    }

    let request = unsafe { &mut *request };
    let inner = match request.inner.take() {
        Some(v) => v,
        None => {
            write_error(out_error, "pre-trade request already consumed");
            return OpenPitPretradeStatus::Error;
        }
    };

    match inner.execute() {
        Ok(reservation) => {
            if !out_reservation.is_null() {
                unsafe {
                    *out_reservation = Box::into_raw(Box::new(OpenPitPretradePreTradeReservation {
                        inner: reservation,
                    }))
                };
            }
            OpenPitPretradeStatus::Passed
        }
        Err(rejects) => {
            if !out_rejects.is_null() {
                let OpenPitPretradeRejectList { items } = rejects_to_list_owned(rejects);
                unsafe {
                    *out_rejects = Box::into_raw(Box::new(OpenPitPretradeRejectList { items }));
                }
            }
            OpenPitPretradeStatus::Rejected
        }
    }
}

#[no_mangle]
/// Releases a deferred request pointer owned by the caller.
///
/// Contract:
/// - passing null is allowed;
/// - destroying an unexecuted request abandons it without creating a
///   reservation;
/// - this function always succeeds.
pub extern "C" fn openpit_destroy_pretrade_pre_trade_request(
    request: *mut OpenPitPretradePreTradeRequest,
) {
    if request.is_null() {
        return;
    }
    unsafe { drop(Box::from_raw(request)) };
}

#[no_mangle]
/// Finalizes a reservation and applies the reserved state permanently.
///
/// This call is idempotent at the pointer level: if the reservation was already
/// consumed, nothing happens. Passing null is allowed.
///
/// Contract:
/// - passing null is allowed;
/// - this function always succeeds.
pub extern "C" fn openpit_pretrade_pre_trade_reservation_commit(
    reservation: *mut OpenPitPretradePreTradeReservation,
) {
    if reservation.is_null() {
        return;
    }
    unsafe { &mut *reservation }.inner.commit();
}

#[no_mangle]
/// Cancels a reservation and releases the reserved state.
///
/// This call is idempotent at the pointer level: if the reservation was already
/// consumed, nothing happens. Passing null is allowed.
///
/// Contract:
/// - passing null is allowed;
/// - this function always succeeds.
pub extern "C" fn openpit_pretrade_pre_trade_reservation_rollback(
    reservation: *mut OpenPitPretradePreTradeReservation,
) {
    if reservation.is_null() {
        return;
    }
    unsafe { &mut *reservation }.inner.rollback();
}

#[no_mangle]
/// Returns a snapshot of the lock attached to a reservation.
///
/// Contract:
/// - `reservation` must be a valid non-null pointer;
/// - violating the pointer contract aborts the call;
/// - this function never fails.
///
/// Lifetime contract:
/// - the returned snapshot is detached from the reservation state.
pub extern "C" fn openpit_pretrade_pre_trade_reservation_get_lock(
    reservation: *const OpenPitPretradePreTradeReservation,
) -> OpenPitPretradePreTradeLock {
    assert!(!reservation.is_null());
    export_pre_trade_lock(unsafe { &*reservation }.inner.lock())
}

#[no_mangle]
/// Releases a reservation pointer owned by the caller.
///
/// Contract:
/// - passing null is allowed;
/// - destroying an unresolved reservation triggers rollback of any pending
///   mutations;
/// - callers that need explicit resolution should call commit or rollback
///   first;
/// - this function always succeeds.
pub extern "C" fn openpit_destroy_pretrade_pre_trade_reservation(
    reservation: *mut OpenPitPretradePreTradeReservation,
) {
    if reservation.is_null() {
        return;
    }
    unsafe { drop(Box::from_raw(reservation)) };
}

#[no_mangle]
/// Applies an execution report to engine state.
///
/// Returns `true` on success, `false` on error.
///
/// Success:
/// - returns `true`;
/// - if `out_blocks` is not null and at least one policy entered a blocked
///   state, writes a caller-owned `OpenPitPretradeAccountBlockList` pointer;
///   release it with `openpit_pretrade_destroy_account_block_list`;
/// - if `out_blocks` is not null and no policy blocked, writes null.
///
/// Error:
/// - returns `false` when input pointers are invalid or the report payload
///   cannot be decoded;
/// - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
///   error handle that MUST be released with `openpit_destroy_shared_string`.
///
/// Lifetime contract:
/// - `report` is read as a borrowed view during this call only;
/// - the operation does not retain any pointer into source memory after this
///   function returns.
pub extern "C" fn openpit_engine_apply_execution_report(
    engine: *mut OpenPitEngine,
    report: *const OpenPitExecutionReport,
    out_blocks: *mut *mut OpenPitPretradeAccountBlockList,
    out_error: OpenPitOutError,
) -> bool {
    if engine.is_null() {
        write_error(out_error, "engine is null");
        return false;
    }
    if report.is_null() {
        write_error(out_error, "report is null");
        return false;
    }

    let report = match import_execution_report(unsafe { &*report }) {
        Ok(v) => v,
        Err(e) => {
            write_error(out_error, &e);
            return false;
        }
    };

    let result = unsafe { &*engine }.inner.apply_execution_report(&report);

    if !out_blocks.is_null() {
        if !result.account_blocks.is_empty() {
            let list = blocks_to_list_owned(result.account_blocks);
            unsafe { *out_blocks = Box::into_raw(Box::new(list)) };
        } else {
            unsafe { *out_blocks = std::ptr::null_mut() };
        }
    }

    true
}

#[no_mangle]
/// Releases a batch-error object returned by account-adjustment apply.
///
/// Contract:
/// - passing null is allowed;
/// - this function always succeeds.
pub extern "C" fn openpit_destroy_account_adjustment_batch_error(
    batch_error: *mut OpenPitAccountAdjustmentBatchError,
) {
    if batch_error.is_null() {
        return;
    }
    unsafe { drop(Box::from_raw(batch_error)) };
}

#[no_mangle]
/// Returns the failing adjustment index from a batch error.
///
/// Contract:
/// - `batch_error` must be a valid non-null pointer;
/// - this function never fails;
/// - violating the pointer contract aborts the call.
pub extern "C" fn openpit_account_adjustment_batch_error_get_failed_adjustment_index(
    batch_error: *const OpenPitAccountAdjustmentBatchError,
) -> usize {
    assert!(!batch_error.is_null(), "batch error pointer is null");
    let batch_error = unsafe { &*batch_error };
    batch_error.failed_adjustment_index
}

#[no_mangle]
/// Returns a non-owning reject-list view from a batch error.
///
/// Contract:
/// - `batch_error` must be a valid non-null pointer;
/// - the returned pointer is valid while `batch_error` is alive;
/// - this function never fails;
/// - violating the pointer contract aborts the call.
pub extern "C" fn openpit_account_adjustment_batch_error_get_rejects(
    batch_error: *const OpenPitAccountAdjustmentBatchError,
) -> *const OpenPitPretradeRejectList {
    assert!(!batch_error.is_null(), "batch error pointer is null");
    let batch_error = unsafe { &*batch_error };
    &batch_error.rejects as *const OpenPitPretradeRejectList
}

#[no_mangle]
/// Applies a batch of account adjustments to one account.
///
/// Success:
/// - returns `OpenPitAccountAdjustmentApplyStatus::Applied` when the batch was
///   accepted and applied;
/// - returns `OpenPitAccountAdjustmentApplyStatus::Rejected` when the call itself
///   completed normally but a policy rejected the batch; read `out_reject`.
///
/// Error:
/// - returns `OpenPitAccountAdjustmentApplyStatus::Error` when input pointers are
///   invalid or some adjustment payload cannot be decoded;
/// - on `Error`, if `out_error` is not null, it is filled with a
///   caller-owned `OpenPitSharedString` that MUST be destroyed by the caller.
///
/// Result handling:
/// - `Applied` means there is no reject object to clean up;
/// - `Rejected` stores batch error details in `out_reject`, the caller must
///   release a returned object with `openpit_destroy_account_adjustment_batch_error`;
/// - rejects returned by `openpit_account_adjustment_batch_error_get_rejects`
///   contain string views borrowed from the batch error and must not be used
///   after the batch error is destroyed;
/// - when `Error` is returned, do not use any pointer from a previous
///   unrelated call as if it belonged to this failure.
///
/// Lifetime contract:
/// - every `adjustment` entry from the contiguous input array is read as a
///   borrowed view during this call only;
/// - release a returned batch error with
///   `openpit_destroy_account_adjustment_batch_error`.
pub extern "C" fn openpit_engine_apply_account_adjustment(
    engine: *mut OpenPitEngine,
    account_id: OpenPitParamAccountId,
    adjustments: *const OpenPitAccountAdjustment,
    adjustments_len: usize,
    out_reject: *mut *mut OpenPitAccountAdjustmentBatchError,
    out_error: OpenPitOutError,
) -> OpenPitAccountAdjustmentApplyStatus {
    if engine.is_null() {
        write_error(out_error, "engine is null");
        return OpenPitAccountAdjustmentApplyStatus::Error;
    }
    if adjustments_len > 0 && adjustments.is_null() {
        write_error(out_error, "adjustments is null");
        return OpenPitAccountAdjustmentApplyStatus::Error;
    }

    let adjustments = if adjustments_len == 0 {
        Vec::new()
    } else {
        let views = unsafe { std::slice::from_raw_parts(adjustments, adjustments_len) };
        let mut values = Vec::with_capacity(views.len());
        for view in views {
            let parsed = match import_account_adjustment(view) {
                Ok(v) => v,
                Err(e) => {
                    write_error(out_error, &e);
                    return OpenPitAccountAdjustmentApplyStatus::Error;
                }
            };
            values.push(parsed);
        }
        values
    };

    match unsafe { &*engine }
        .inner
        .apply_account_adjustment(AccountId::from_u64(account_id), &adjustments)
    {
        Ok(()) => OpenPitAccountAdjustmentApplyStatus::Applied,
        Err(err) => {
            if !out_reject.is_null() {
                unsafe {
                    *out_reject =
                        Box::into_raw(Box::new(OpenPitAccountAdjustmentBatchError::new(err)))
                }
            }
            OpenPitAccountAdjustmentApplyStatus::Rejected
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::c_void;

    use openpit::pretrade::{PreTradePolicy, Reject, RejectCode, RejectScope, Rejects};
    use openpit::EngineBuildError;

    use crate::account_adjustment::OpenPitAccountAdjustment;
    use crate::account_adjustment::OpenPitAccountAdjustmentApplyStatus;
    use crate::engine::{OpenPitPretradeStatus, OpenPitSyncPolicy};
    use crate::execution_report::{
        OpenPitExecutionReport, OpenPitExecutionReportOperation,
        OpenPitExecutionReportOperationOptional, OpenPitExecutionReportPositionImpactOptional,
        OpenPitFinancialImpactOptional,
    };
    use crate::order::OpenPitOrder;
    use crate::policy::{
        openpit_create_pretrade_custom_pre_trade_policy, openpit_destroy_pretrade_pre_trade_policy,
        openpit_engine_builder_add_pre_trade_policy, OpenPitPretradePreTradePolicy,
        OpenPitPretradePreTradePolicyApplyAccountAdjustmentFn,
        OpenPitPretradePreTradePolicyApplyExecutionReportFn,
        OpenPitPretradePreTradePolicyCheckPreTradeStartFn,
        OpenPitPretradePreTradePolicyFreeUserDataFn,
    };
    use crate::reject::{
        openpit_pretrade_create_reject_list, openpit_pretrade_destroy_reject_list,
        openpit_pretrade_reject_list_get, openpit_pretrade_reject_list_len, OpenPitPretradeReject,
        OpenPitPretradeRejectCode, OpenPitPretradeRejectList, OpenPitPretradeRejectScope,
    };
    use crate::OpenPitStringView;

    use super::{
        openpit_account_adjustment_batch_error_get_failed_adjustment_index,
        openpit_account_adjustment_batch_error_get_rejects, openpit_create_engine_builder,
        openpit_destroy_account_adjustment_batch_error, openpit_destroy_engine,
        openpit_destroy_engine_builder, openpit_destroy_pretrade_pre_trade_request,
        openpit_destroy_pretrade_pre_trade_reservation, openpit_engine_apply_account_adjustment,
        openpit_engine_apply_execution_report, openpit_engine_builder_build,
        openpit_engine_execute_pre_trade, openpit_engine_start_pre_trade,
        openpit_pretrade_pre_trade_request_execute, openpit_pretrade_pre_trade_reservation_commit,
        openpit_pretrade_pre_trade_reservation_get_lock,
        openpit_pretrade_pre_trade_reservation_rollback, OpenPitAccountAdjustmentBatchError,
        OpenPitPretradePreTradeLock,
    };

    struct AlwaysRejectStart;

    impl
        PreTradePolicy<
            crate::order::Order,
            crate::execution_report::ExecutionReport,
            crate::account_adjustment::AccountAdjustment,
        > for AlwaysRejectStart
    {
        fn name(&self) -> &str {
            "always.reject.start"
        }

        fn check_pre_trade_start(
            &self,
            _ctx: &openpit::pretrade::PreTradeContext,
            _order: &crate::order::Order,
        ) -> Result<(), Rejects> {
            Err(Rejects::from(Reject::new(
                self.name(),
                RejectScope::Order,
                RejectCode::OrderExceedsLimit,
                "rejected",
                "for coverage",
            )))
        }

        fn apply_execution_report(
            &self,
            _report: &crate::execution_report::ExecutionReport,
        ) -> Vec<openpit::pretrade::AccountBlock> {
            vec![]
        }
    }

    unsafe extern "C" fn always_reject_apply(
        _ctx: *const crate::policy::OpenPitAccountAdjustmentContext,
        _account_id: crate::param::OpenPitParamAccountId,
        _adjustment: *const OpenPitAccountAdjustment,
        _mutations: *mut crate::policy::OpenPitMutations,
        _user_data: *mut c_void,
    ) -> *mut OpenPitPretradeRejectList {
        let rejects = openpit_pretrade_create_reject_list(1);
        crate::reject::openpit_pretrade_reject_list_push(
            rejects,
            OpenPitPretradeReject {
                policy: OpenPitStringView::from_utf8("test_policy"),
                reason: OpenPitStringView::from_utf8("test_reason"),
                details: OpenPitStringView::from_utf8("test_details"),
                user_data: std::ptr::null_mut(),
                code: OpenPitPretradeRejectCode::AccountBlocked,
                scope: OpenPitPretradeRejectScope::Account,
            },
        );
        rejects
    }

    unsafe extern "C" fn always_pass_start_check(
        _ctx: *const crate::policy::OpenPitPretradeContext,
        _order: *const crate::order::OpenPitOrder,
        _user_data: *mut c_void,
    ) -> *mut OpenPitPretradeRejectList {
        std::ptr::null_mut()
    }

    unsafe extern "C" fn always_reject_start_check(
        _ctx: *const crate::policy::OpenPitPretradeContext,
        _order: *const crate::order::OpenPitOrder,
        _user_data: *mut c_void,
    ) -> *mut OpenPitPretradeRejectList {
        let rejects = openpit_pretrade_create_reject_list(1);
        crate::reject::openpit_pretrade_reject_list_push(
            rejects,
            OpenPitPretradeReject {
                policy: OpenPitStringView::from_utf8("start.reject"),
                reason: OpenPitStringView::from_utf8("blocked"),
                details: OpenPitStringView::from_utf8("by test"),
                user_data: std::ptr::null_mut(),
                code: OpenPitPretradeRejectCode::OrderExceedsLimit,
                scope: OpenPitPretradeRejectScope::Order,
            },
        );
        rejects
    }

    unsafe extern "C" fn always_reject_pre_trade(
        _ctx: *const crate::policy::OpenPitPretradeContext,
        _order: *const crate::order::OpenPitOrder,
        _mutations: *mut crate::policy::OpenPitMutations,
        _user_data: *mut c_void,
    ) -> *mut OpenPitPretradeRejectList {
        let rejects = openpit_pretrade_create_reject_list(1);
        let reject = OpenPitPretradeReject {
            policy: OpenPitStringView::from_utf8("pretrade.reject"),
            reason: OpenPitStringView::from_utf8("blocked"),
            details: OpenPitStringView::from_utf8("by test"),
            user_data: std::ptr::null_mut(),
            code: OpenPitPretradeRejectCode::RiskLimitExceeded,
            scope: OpenPitPretradeRejectScope::Order,
        };
        crate::reject::openpit_pretrade_reject_list_push(rejects, reject);
        rejects
    }

    unsafe extern "C" fn null_apply_report(
        _report: *const crate::execution_report::OpenPitExecutionReport,
        _user_data: *mut c_void,
    ) -> *mut crate::reject::OpenPitPretradeAccountBlockList {
        std::ptr::null_mut()
    }

    unsafe extern "C" fn noop_free_user_data(_user_data: *mut c_void) {}

    unsafe fn create_pre_trade_policy_with_start_hook(
        name: OpenPitStringView,
        check_fn: OpenPitPretradePreTradePolicyCheckPreTradeStartFn,
        apply_execution_report_fn: OpenPitPretradePreTradePolicyApplyExecutionReportFn,
        free_user_data_fn: OpenPitPretradePreTradePolicyFreeUserDataFn,
        user_data: *mut c_void,
        out_error: crate::last_error::OpenPitOutError,
    ) -> *mut OpenPitPretradePreTradePolicy {
        unsafe {
            openpit_create_pretrade_custom_pre_trade_policy(
                name,
                Some(check_fn),
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
        out_error: crate::last_error::OpenPitOutError,
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

    fn build_engine_with_reject_policy() -> *mut super::OpenPitEngine {
        let builder =
            openpit_create_engine_builder(OpenPitSyncPolicy::Full as u8, std::ptr::null_mut());
        let policy_name = OpenPitStringView::from_utf8("test_policy");
        let policy = unsafe {
            create_pre_trade_policy_with_account_adjustment_hook(
                policy_name,
                always_reject_apply,
                noop_free_user_data,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert!(!policy.is_null(), "failed to create policy");
        let ok = openpit_engine_builder_add_pre_trade_policy(builder, policy, std::ptr::null_mut());
        assert!(ok, "failed to add policy");
        openpit_destroy_pretrade_pre_trade_policy(policy);
        let engine = openpit_engine_builder_build(builder, std::ptr::null_mut());
        assert!(!engine.is_null(), "engine build failed");
        engine
    }

    fn build_engine_with_main_reject_policy() -> *mut super::OpenPitEngine {
        let builder =
            openpit_create_engine_builder(OpenPitSyncPolicy::Full as u8, std::ptr::null_mut());
        let name = OpenPitStringView::from_utf8("pretrade.reject");
        let policy = unsafe {
            crate::policy::openpit_create_pretrade_custom_pre_trade_policy(
                name,
                None,
                Some(always_reject_pre_trade),
                Some(null_apply_report),
                None,
                noop_free_user_data,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert!(!policy.is_null(), "failed to create policy");
        let ok = crate::policy::openpit_engine_builder_add_pre_trade_policy(
            builder,
            policy,
            std::ptr::null_mut(),
        );
        assert!(ok, "failed to add policy");
        crate::policy::openpit_destroy_pretrade_pre_trade_policy(policy);
        let engine = openpit_engine_builder_build(builder, std::ptr::null_mut());
        assert!(!engine.is_null(), "engine build failed");
        engine
    }

    fn build_engine_with_start_reject_policy() -> *mut super::OpenPitEngine {
        let builder =
            openpit_create_engine_builder(OpenPitSyncPolicy::Full as u8, std::ptr::null_mut());
        let name = OpenPitStringView::from_utf8("start.reject");
        let policy = unsafe {
            create_pre_trade_policy_with_start_hook(
                name,
                always_reject_start_check,
                null_apply_report,
                noop_free_user_data,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert!(!policy.is_null(), "failed to create policy");
        let ok = openpit_engine_builder_add_pre_trade_policy(builder, policy, std::ptr::null_mut());
        assert!(ok, "failed to add policy");
        openpit_destroy_pretrade_pre_trade_policy(policy);
        let engine = openpit_engine_builder_build(builder, std::ptr::null_mut());
        assert!(!engine.is_null(), "engine build failed");
        engine
    }

    fn build_engine_with_start_pass_policy() -> *mut super::OpenPitEngine {
        let builder =
            openpit_create_engine_builder(OpenPitSyncPolicy::Full as u8, std::ptr::null_mut());
        let name = OpenPitStringView::from_utf8("start.pass");
        let policy = unsafe {
            create_pre_trade_policy_with_start_hook(
                name,
                always_pass_start_check,
                null_apply_report,
                noop_free_user_data,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert!(!policy.is_null(), "failed to create policy");
        let ok = openpit_engine_builder_add_pre_trade_policy(builder, policy, std::ptr::null_mut());
        assert!(ok, "failed to add policy");
        openpit_destroy_pretrade_pre_trade_policy(policy);
        let engine = openpit_engine_builder_build(builder, std::ptr::null_mut());
        assert!(!engine.is_null(), "engine build failed");
        engine
    }

    fn build_passthrough_engine() -> *mut super::OpenPitEngine {
        let builder =
            openpit_create_engine_builder(OpenPitSyncPolicy::Full as u8, std::ptr::null_mut());
        let name = OpenPitStringView::from_utf8("passthrough");
        let policy = unsafe {
            create_pre_trade_policy_with_start_hook(
                name,
                always_pass_start_check,
                null_apply_report,
                noop_free_user_data,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert!(!policy.is_null(), "failed to create passthrough policy");
        assert!(
            openpit_engine_builder_add_pre_trade_policy(builder, policy, std::ptr::null_mut()),
            "failed to add passthrough policy"
        );
        openpit_destroy_pretrade_pre_trade_policy(policy);
        let engine = openpit_engine_builder_build(builder, std::ptr::null_mut());
        assert!(!engine.is_null(), "engine build failed");
        engine
    }

    fn string_view_to_string(view: OpenPitStringView) -> String {
        if view.ptr.is_null() {
            return String::new();
        }
        let bytes = unsafe { std::slice::from_raw_parts(view.ptr, view.len) };
        std::str::from_utf8(bytes).expect("utf8").to_string()
    }

    #[test]
    fn string_view_to_string_handles_null_pointer() {
        assert_eq!(string_view_to_string(OpenPitStringView::not_set()), "");
    }

    fn shared_string_to_owned(handle: *mut crate::string::OpenPitSharedString) -> String {
        let view = crate::string::openpit_shared_string_view(handle);
        string_view_to_string(view)
    }

    #[test]
    fn create_engine_builder_invalid_sync_policy_returns_null() {
        let mut error: *mut crate::string::OpenPitSharedString = std::ptr::null_mut();
        let builder = openpit_create_engine_builder(99, &mut error);
        assert!(builder.is_null());
        assert!(!error.is_null());
        let msg = shared_string_to_owned(error);
        assert!(
            msg.contains("invalid sync_policy byte 99"),
            "unexpected error message: {msg}"
        );
        crate::string::openpit_destroy_shared_string(error);
    }

    #[test]
    fn create_engine_builder_invalid_sync_policy_tolerates_null_out_error() {
        let builder = openpit_create_engine_builder(7, std::ptr::null_mut());
        assert!(builder.is_null());
    }

    #[test]
    fn create_engine_builder_accepts_valid_sync_policies() {
        for byte in [
            OpenPitSyncPolicy::Full as u8,
            OpenPitSyncPolicy::Local as u8,
            OpenPitSyncPolicy::Account as u8,
        ] {
            let mut error: *mut crate::string::OpenPitSharedString = std::ptr::null_mut();
            let builder = openpit_create_engine_builder(byte, &mut error);
            assert!(!builder.is_null(), "byte={byte} produced null builder");
            assert!(error.is_null(), "byte={byte} produced unexpected error");
            openpit_destroy_engine_builder(builder);
        }
    }

    #[test]
    fn engine_builder_build_reports_null_consumed_and_validation_errors() {
        let engine = openpit_engine_builder_build(std::ptr::null_mut(), std::ptr::null_mut());
        assert!(engine.is_null());

        let builder =
            openpit_create_engine_builder(OpenPitSyncPolicy::Full as u8, std::ptr::null_mut());
        let pass_name = OpenPitStringView::from_utf8("pass.build");
        let pass_policy = unsafe {
            create_pre_trade_policy_with_start_hook(
                pass_name,
                always_pass_start_check,
                null_apply_report,
                noop_free_user_data,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert!(openpit_engine_builder_add_pre_trade_policy(
            builder,
            pass_policy,
            std::ptr::null_mut()
        ));
        openpit_destroy_pretrade_pre_trade_policy(pass_policy);
        let built = openpit_engine_builder_build(builder, std::ptr::null_mut());
        assert!(!built.is_null());
        openpit_destroy_engine(built);
        let consumed = openpit_engine_builder_build(builder, std::ptr::null_mut());
        assert!(consumed.is_null());
        openpit_destroy_engine_builder(builder);

        let dup_builder =
            openpit_create_engine_builder(OpenPitSyncPolicy::Full as u8, std::ptr::null_mut());
        let dup_name = OpenPitStringView::from_utf8("dup.start");
        let first = unsafe {
            create_pre_trade_policy_with_start_hook(
                dup_name,
                always_pass_start_check,
                null_apply_report,
                noop_free_user_data,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        let second = unsafe {
            create_pre_trade_policy_with_start_hook(
                dup_name,
                always_pass_start_check,
                null_apply_report,
                noop_free_user_data,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert!(!first.is_null() && !second.is_null());
        assert!(openpit_engine_builder_add_pre_trade_policy(
            dup_builder,
            first,
            std::ptr::null_mut()
        ));
        assert!(openpit_engine_builder_add_pre_trade_policy(
            dup_builder,
            second,
            std::ptr::null_mut()
        ));
        openpit_destroy_pretrade_pre_trade_policy(first);
        openpit_destroy_pretrade_pre_trade_policy(second);

        let invalid = openpit_engine_builder_build(dup_builder, std::ptr::null_mut());
        assert!(invalid.is_null());
        openpit_destroy_engine_builder(dup_builder);
    }

    #[test]
    fn add_policy_on_consumed_builder_returns_error() {
        let builder =
            openpit_create_engine_builder(OpenPitSyncPolicy::Full as u8, std::ptr::null_mut());
        let pass_name = OpenPitStringView::from_utf8("pass.consumed");
        let pass_policy = unsafe {
            create_pre_trade_policy_with_start_hook(
                pass_name,
                always_pass_start_check,
                null_apply_report,
                noop_free_user_data,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert!(openpit_engine_builder_add_pre_trade_policy(
            builder,
            pass_policy,
            std::ptr::null_mut()
        ));
        openpit_destroy_pretrade_pre_trade_policy(pass_policy);
        let engine = openpit_engine_builder_build(builder, std::ptr::null_mut());
        assert!(!engine.is_null());
        openpit_destroy_engine(engine);

        let name = OpenPitStringView::from_utf8("consumed.builder");
        let policy = unsafe {
            create_pre_trade_policy_with_start_hook(
                name,
                always_pass_start_check,
                null_apply_report,
                noop_free_user_data,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert!(!policy.is_null());
        let ok = openpit_engine_builder_add_pre_trade_policy(builder, policy, std::ptr::null_mut());
        assert!(!ok);
        openpit_destroy_pretrade_pre_trade_policy(policy);
        openpit_destroy_engine_builder(builder);
    }

    #[test]
    fn start_pre_trade_does_not_touch_out_values_on_error() {
        let mut out_request = std::ptr::dangling_mut::<super::OpenPitPretradePreTradeRequest>();
        let mut out_rejects: *mut OpenPitPretradeRejectList = std::ptr::null_mut();

        let status = openpit_engine_start_pre_trade(
            std::ptr::null_mut(),
            std::ptr::null(),
            &mut out_request,
            &mut out_rejects,
            std::ptr::null_mut(),
        );

        assert_eq!(status, OpenPitPretradeStatus::Error);
        assert_eq!(
            out_request,
            std::ptr::dangling_mut::<super::OpenPitPretradePreTradeRequest>()
        );
        assert!(out_rejects.is_null());
    }

    #[test]
    fn start_pre_trade_covers_null_order_and_reject_outputs() {
        let engine = build_engine_with_start_reject_policy();
        let mut out_request = std::ptr::null_mut();
        let mut out_rejects: *mut OpenPitPretradeRejectList = std::ptr::null_mut();
        let order = OpenPitOrder::default();

        let status = openpit_engine_start_pre_trade(
            engine,
            std::ptr::null(),
            &mut out_request,
            &mut out_rejects,
            std::ptr::null_mut(),
        );
        assert_eq!(status, OpenPitPretradeStatus::Error);
        assert!(out_rejects.is_null());

        let status = openpit_engine_start_pre_trade(
            engine,
            &order,
            &mut out_request,
            &mut out_rejects,
            std::ptr::null_mut(),
        );
        assert_eq!(status, OpenPitPretradeStatus::Rejected);
        assert!(!out_rejects.is_null());
        openpit_pretrade_destroy_reject_list(out_rejects);

        let status = openpit_engine_start_pre_trade(
            engine,
            &order,
            &mut out_request,
            &mut out_rejects,
            std::ptr::null_mut(),
        );
        assert_eq!(status, OpenPitPretradeStatus::Rejected);
        assert!(!out_rejects.is_null());
        openpit_pretrade_destroy_reject_list(out_rejects);

        openpit_destroy_engine(engine);
    }

    #[test]
    fn start_pre_trade_pass_path_covers_null_out_request_pointer() {
        let engine = build_engine_with_start_pass_policy();
        let order = OpenPitOrder::default();
        let mut out_rejects: *mut OpenPitPretradeRejectList = std::ptr::null_mut();

        let status = openpit_engine_start_pre_trade(
            engine,
            &order,
            std::ptr::null_mut(),
            &mut out_rejects,
            std::ptr::null_mut(),
        );
        assert_eq!(status, OpenPitPretradeStatus::Passed);
        assert!(out_rejects.is_null());

        openpit_destroy_engine(engine);
    }

    #[test]
    fn execute_pre_trade_does_not_touch_out_values_on_error() {
        let mut out_reservation =
            std::ptr::dangling_mut::<super::OpenPitPretradePreTradeReservation>();
        let mut out_rejects: *mut OpenPitPretradeRejectList = std::ptr::null_mut();

        let status = openpit_engine_execute_pre_trade(
            std::ptr::null_mut(),
            std::ptr::null(),
            &mut out_reservation,
            &mut out_rejects,
            std::ptr::null_mut(),
        );

        assert_eq!(status, OpenPitPretradeStatus::Error);
        assert_eq!(
            out_reservation,
            std::ptr::dangling_mut::<super::OpenPitPretradePreTradeReservation>()
        );
        assert!(out_rejects.is_null());
    }

    #[test]
    fn execute_pre_trade_covers_null_order_and_optional_output_paths() {
        let order = OpenPitOrder::default();
        let mut out_rejects: *mut OpenPitPretradeRejectList = std::ptr::null_mut();

        let engine = build_passthrough_engine();
        let status = openpit_engine_execute_pre_trade(
            engine,
            std::ptr::null(),
            std::ptr::null_mut(),
            &mut out_rejects,
            std::ptr::null_mut(),
        );
        assert_eq!(status, OpenPitPretradeStatus::Error);
        assert!(out_rejects.is_null());
        let status = openpit_engine_execute_pre_trade(
            engine,
            &order,
            std::ptr::null_mut(),
            &mut out_rejects,
            std::ptr::null_mut(),
        );
        assert_eq!(status, OpenPitPretradeStatus::Passed);
        assert!(out_rejects.is_null());
        openpit_destroy_engine(engine);

        let reject_engine = build_engine_with_main_reject_policy();
        let status = openpit_engine_execute_pre_trade(
            reject_engine,
            &order,
            std::ptr::null_mut(),
            &mut out_rejects,
            std::ptr::null_mut(),
        );
        assert_eq!(status, OpenPitPretradeStatus::Rejected);
        assert!(!out_rejects.is_null());
        openpit_pretrade_destroy_reject_list(out_rejects);

        openpit_destroy_engine(reject_engine);
    }

    #[test]
    fn request_execute_does_not_touch_out_values_on_error() {
        let mut out_reservation =
            std::ptr::dangling_mut::<super::OpenPitPretradePreTradeReservation>();
        let mut out_rejects: *mut OpenPitPretradeRejectList = std::ptr::null_mut();

        let status = openpit_pretrade_pre_trade_request_execute(
            std::ptr::null_mut(),
            &mut out_reservation,
            &mut out_rejects,
            std::ptr::null_mut(),
        );

        assert_eq!(status, OpenPitPretradeStatus::Error);
        assert_eq!(
            out_reservation,
            std::ptr::dangling_mut::<super::OpenPitPretradePreTradeReservation>()
        );
        assert!(out_rejects.is_null());
    }

    #[test]
    fn request_execute_covers_success_reject_and_consumed_paths() {
        let order = OpenPitOrder::default();
        let mut out_rejects: *mut OpenPitPretradeRejectList = std::ptr::null_mut();

        let engine = build_passthrough_engine();
        let mut request = std::ptr::null_mut();
        let status = openpit_engine_start_pre_trade(
            engine,
            &order,
            &mut request,
            &mut out_rejects,
            std::ptr::null_mut(),
        );
        assert_eq!(status, OpenPitPretradeStatus::Passed);
        assert!(!request.is_null());
        assert!(out_rejects.is_null());
        let mut reservation = std::ptr::null_mut();
        let status = openpit_pretrade_pre_trade_request_execute(
            request,
            &mut reservation,
            &mut out_rejects,
            std::ptr::null_mut(),
        );
        assert_eq!(status, OpenPitPretradeStatus::Passed);
        assert!(!reservation.is_null());
        assert!(out_rejects.is_null());
        openpit_pretrade_pre_trade_reservation_rollback(reservation);
        openpit_destroy_pretrade_pre_trade_reservation(reservation);
        let status = openpit_pretrade_pre_trade_request_execute(
            request,
            std::ptr::null_mut(),
            &mut out_rejects,
            std::ptr::null_mut(),
        );
        assert_eq!(status, OpenPitPretradeStatus::Error);
        assert!(out_rejects.is_null());
        openpit_destroy_pretrade_pre_trade_request(request);
        openpit_destroy_engine(engine);

        let reject_engine = build_engine_with_main_reject_policy();
        let mut reject_request = std::ptr::null_mut();
        let status = openpit_engine_start_pre_trade(
            reject_engine,
            &order,
            &mut reject_request,
            &mut out_rejects,
            std::ptr::null_mut(),
        );
        assert_eq!(status, OpenPitPretradeStatus::Passed);
        assert!(!reject_request.is_null());
        assert!(out_rejects.is_null());
        let status = openpit_pretrade_pre_trade_request_execute(
            reject_request,
            std::ptr::null_mut(),
            &mut out_rejects,
            std::ptr::null_mut(),
        );
        assert_eq!(status, OpenPitPretradeStatus::Rejected);
        assert!(!out_rejects.is_null());
        openpit_pretrade_destroy_reject_list(out_rejects);
        openpit_destroy_pretrade_pre_trade_request(reject_request);
        openpit_destroy_engine(reject_engine);
    }

    #[test]
    fn apply_account_adjustment_accepts_null_when_batch_is_empty() {
        let engine = build_passthrough_engine();
        let mut out_reject = std::ptr::null_mut();

        let status = openpit_engine_apply_account_adjustment(
            engine,
            1,
            std::ptr::null(),
            0,
            &mut out_reject,
            std::ptr::null_mut(),
        );

        assert_eq!(status, OpenPitAccountAdjustmentApplyStatus::Applied);
        assert!(out_reject.is_null());
        openpit_destroy_engine(engine);
    }

    #[test]
    fn apply_account_adjustment_rejects_null_when_batch_is_non_empty() {
        let engine = build_passthrough_engine();
        let mut out_reject = std::ptr::null_mut::<OpenPitAccountAdjustmentBatchError>();

        let status = openpit_engine_apply_account_adjustment(
            engine,
            1,
            std::ptr::null(),
            1,
            &mut out_reject,
            std::ptr::null_mut(),
        );

        assert_eq!(status, OpenPitAccountAdjustmentApplyStatus::Error);
        assert!(out_reject.is_null());
        openpit_destroy_engine(engine);
    }

    #[test]
    fn apply_account_adjustment_reports_import_error_for_incomplete_payload() {
        let engine = build_passthrough_engine();

        let invalid = crate::account_adjustment::OpenPitAccountAdjustment {
            balance_operation:
                crate::account_adjustment::OpenPitAccountAdjustmentBalanceOperationOptional::default(
                ),
            position_operation:
                crate::account_adjustment::OpenPitAccountAdjustmentPositionOperationOptional {
                    value: crate::account_adjustment::OpenPitAccountAdjustmentPositionOperation {
                        mode: crate::param::OpenPitParamPositionMode::Hedged,
                        ..Default::default()
                    },
                    is_set: true,
                },
            amount: crate::account_adjustment::OpenPitAccountAdjustmentAmountOptional::default(),
            bounds: crate::account_adjustment::OpenPitAccountAdjustmentBoundsOptional::default(),
            user_data: std::ptr::null_mut(),
        };
        let batch = [invalid];
        let mut out_reject = std::ptr::null_mut::<OpenPitAccountAdjustmentBatchError>();
        let status = openpit_engine_apply_account_adjustment(
            engine,
            1,
            batch.as_ptr(),
            batch.len(),
            &mut out_reject,
            std::ptr::null_mut(),
        );
        assert_eq!(status, OpenPitAccountAdjustmentApplyStatus::Error);
        assert!(out_reject.is_null());
        openpit_destroy_engine(engine);
    }

    #[test]
    fn apply_account_adjustment_reports_error_for_null_engine() {
        let status = openpit_engine_apply_account_adjustment(
            std::ptr::null_mut(),
            1,
            std::ptr::null(),
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
        assert_eq!(status, OpenPitAccountAdjustmentApplyStatus::Error);
    }

    #[test]
    fn lock_snapshot_defaults_to_absent_price() {
        let detached = OpenPitPretradePreTradeLock::default();
        assert!(!detached.price.is_set);
    }

    #[test]
    fn reservation_get_lock_covers_success_and_committed_paths() {
        let engine = build_passthrough_engine();
        let order = OpenPitOrder::default();
        let mut out_reservation = std::ptr::null_mut();
        let mut out_rejects: *mut OpenPitPretradeRejectList = std::ptr::null_mut();
        let status = openpit_engine_execute_pre_trade(
            engine,
            &order,
            &mut out_reservation,
            &mut out_rejects,
            std::ptr::null_mut(),
        );
        assert_eq!(status, OpenPitPretradeStatus::Passed);
        assert!(!out_reservation.is_null());
        assert!(out_rejects.is_null());

        let lock = openpit_pretrade_pre_trade_reservation_get_lock(out_reservation);
        assert!(!lock.price.is_set);

        openpit_pretrade_pre_trade_reservation_commit(out_reservation);
        let committed_lock = openpit_pretrade_pre_trade_reservation_get_lock(out_reservation);
        assert!(!committed_lock.price.is_set);

        openpit_destroy_pretrade_pre_trade_reservation(out_reservation);
        openpit_destroy_engine(engine);
    }

    #[test]
    fn apply_account_adjustment_stores_index_on_reject() {
        let engine = build_engine_with_reject_policy();
        let adj = crate::account_adjustment::OpenPitAccountAdjustment::default();
        let mut out_reject = std::ptr::null_mut::<OpenPitAccountAdjustmentBatchError>();

        // First element (index 0) should be rejected.
        let batch = [adj];
        let status = openpit_engine_apply_account_adjustment(
            engine,
            1,
            batch.as_ptr(),
            1,
            &mut out_reject,
            std::ptr::null_mut(),
        );
        assert_eq!(status, OpenPitAccountAdjustmentApplyStatus::Rejected);
        assert!(!out_reject.is_null());

        let index = openpit_account_adjustment_batch_error_get_failed_adjustment_index(out_reject);
        assert_eq!(index, 0);
        let rejects = openpit_account_adjustment_batch_error_get_rejects(out_reject);
        assert!(!rejects.is_null());
        assert_eq!(openpit_pretrade_reject_list_len(rejects), 1);
        let mut reject = OpenPitPretradeReject {
            code: OpenPitPretradeRejectCode::Other,
            reason: OpenPitStringView::not_set(),
            details: OpenPitStringView::not_set(),
            policy: OpenPitStringView::not_set(),
            user_data: std::ptr::null_mut(),
            scope: OpenPitPretradeRejectScope::Order,
        };
        assert!(openpit_pretrade_reject_list_get(rejects, 0, &mut reject));
        assert_eq!(reject.code, OpenPitPretradeRejectCode::AccountBlocked);

        openpit_destroy_account_adjustment_batch_error(out_reject);
        openpit_destroy_engine(engine);
    }

    #[test]
    fn apply_account_adjustment_stores_all_reject_fields() {
        let engine = build_engine_with_reject_policy();
        let adj = crate::account_adjustment::OpenPitAccountAdjustment::default();
        let mut out_reject = std::ptr::null_mut::<OpenPitAccountAdjustmentBatchError>();

        let batch = [adj];
        let status = openpit_engine_apply_account_adjustment(
            engine,
            1,
            batch.as_ptr(),
            1,
            &mut out_reject,
            std::ptr::null_mut(),
        );
        assert_eq!(status, OpenPitAccountAdjustmentApplyStatus::Rejected);
        assert!(!out_reject.is_null());

        let rejects = openpit_account_adjustment_batch_error_get_rejects(out_reject);
        assert!(!rejects.is_null());
        assert_eq!(openpit_pretrade_reject_list_len(rejects), 1);
        let mut reject_ptr = OpenPitPretradeReject {
            code: OpenPitPretradeRejectCode::Other,
            reason: OpenPitStringView::not_set(),
            details: OpenPitStringView::not_set(),
            policy: OpenPitStringView::not_set(),
            user_data: std::ptr::null_mut(),
            scope: OpenPitPretradeRejectScope::Order,
        };
        assert!(openpit_pretrade_reject_list_get(
            rejects,
            0,
            &mut reject_ptr
        ));
        assert_eq!(reject_ptr.code, OpenPitPretradeRejectCode::AccountBlocked);
        assert_eq!(reject_ptr.scope, OpenPitPretradeRejectScope::Account);
        assert_eq!(reject_ptr.user_data, std::ptr::null_mut());

        let policy = string_view_to_string(reject_ptr.policy);
        assert_eq!(policy, "test_policy");
        let reason = string_view_to_string(reject_ptr.reason);
        assert_eq!(reason, "test_reason");
        let details = string_view_to_string(reject_ptr.details);
        assert_eq!(details, "test_details");

        openpit_destroy_account_adjustment_batch_error(out_reject);
        openpit_destroy_engine(engine);
    }

    #[test]
    fn apply_account_adjustment_out_reject_can_be_omitted() {
        let engine = build_engine_with_reject_policy();
        let adj = crate::account_adjustment::OpenPitAccountAdjustment::default();

        let batch = [adj];
        let status = openpit_engine_apply_account_adjustment(
            engine,
            1,
            batch.as_ptr(),
            1,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
        assert_eq!(status, OpenPitAccountAdjustmentApplyStatus::Rejected);

        openpit_destroy_engine(engine);
    }

    #[test]
    fn apply_account_adjustment_does_not_touch_out_reject_on_transport_error() {
        let engine = build_engine_with_reject_policy();
        let mut out_reject = std::ptr::dangling_mut::<OpenPitAccountAdjustmentBatchError>();
        let status = openpit_engine_apply_account_adjustment(
            engine,
            1,
            std::ptr::null(),
            1,
            &mut out_reject,
            std::ptr::null_mut(),
        );
        assert_eq!(status, OpenPitAccountAdjustmentApplyStatus::Error);
        assert_eq!(
            out_reject,
            std::ptr::dangling_mut::<OpenPitAccountAdjustmentBatchError>()
        );

        openpit_destroy_engine(engine);
    }

    #[test]
    fn account_adjustment_batch_error_destroy_is_null_safe() {
        openpit_destroy_account_adjustment_batch_error(std::ptr::null_mut());
    }

    #[test]
    fn destroy_functions_and_apply_report_api_are_callable_via_public_ffi() {
        openpit_destroy_engine_builder(std::ptr::null_mut());
        openpit_destroy_engine(std::ptr::null_mut());
        openpit_destroy_pretrade_pre_trade_request(std::ptr::null_mut());
        openpit_destroy_pretrade_pre_trade_reservation(std::ptr::null_mut());
        openpit_pretrade_pre_trade_reservation_commit(std::ptr::null_mut());
        openpit_pretrade_pre_trade_reservation_rollback(std::ptr::null_mut());
        let engine = build_passthrough_engine();

        let order = OpenPitOrder::default();
        let mut out_request = std::ptr::null_mut();
        let mut out_rejects: *mut OpenPitPretradeRejectList = std::ptr::null_mut();
        let status = openpit_engine_start_pre_trade(
            engine,
            &order,
            &mut out_request,
            &mut out_rejects,
            std::ptr::null_mut(),
        );
        assert_eq!(status, OpenPitPretradeStatus::Passed);
        assert!(!out_request.is_null());
        assert!(out_rejects.is_null());
        openpit_destroy_pretrade_pre_trade_request(out_request);

        let mut out_reservation = std::ptr::null_mut();
        let status = openpit_engine_execute_pre_trade(
            engine,
            &order,
            &mut out_reservation,
            &mut out_rejects,
            std::ptr::null_mut(),
        );
        assert_eq!(status, OpenPitPretradeStatus::Passed);
        assert!(!out_reservation.is_null());
        assert!(out_rejects.is_null());
        openpit_pretrade_pre_trade_reservation_commit(out_reservation);
        openpit_destroy_pretrade_pre_trade_reservation(out_reservation);

        let mut out_reservation2 = std::ptr::null_mut();
        let status = openpit_engine_execute_pre_trade(
            engine,
            &order,
            &mut out_reservation2,
            &mut out_rejects,
            std::ptr::null_mut(),
        );
        assert_eq!(status, OpenPitPretradeStatus::Passed);
        assert!(!out_reservation2.is_null());
        assert!(out_rejects.is_null());
        openpit_pretrade_pre_trade_reservation_rollback(out_reservation2);
        openpit_destroy_pretrade_pre_trade_reservation(out_reservation2);

        let report = OpenPitExecutionReport::default();
        let ok = openpit_engine_apply_execution_report(
            engine,
            &report,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
        assert!(ok);

        openpit_destroy_engine(engine);
    }

    #[test]
    fn apply_execution_report_covers_error_paths_and_custom_apply_callback() {
        assert!(!openpit_engine_apply_execution_report(
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        ));

        let engine = build_passthrough_engine();

        assert!(!openpit_engine_apply_execution_report(
            engine,
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        ));

        let invalid = OpenPitExecutionReport {
            operation: OpenPitExecutionReportOperationOptional {
                is_set: true,
                value: OpenPitExecutionReportOperation {
                    instrument: crate::instrument::OpenPitInstrument {
                        underlying_asset: OpenPitStringView::from_utf8("AAPL"),
                        settlement_asset: OpenPitStringView::default(),
                    },
                    ..OpenPitExecutionReportOperation::default()
                },
            },
            financial_impact: OpenPitFinancialImpactOptional::default(),
            fill: crate::execution_report::OpenPitExecutionReportFillOptional::default(),
            position_impact: OpenPitExecutionReportPositionImpactOptional::default(),
            user_data: std::ptr::null_mut(),
        };
        assert!(!openpit_engine_apply_execution_report(
            engine,
            &invalid,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        ));
        openpit_destroy_engine(engine);

        let callback_engine = build_engine_with_main_reject_policy();
        let report = OpenPitExecutionReport::default();
        assert!(openpit_engine_apply_execution_report(
            callback_engine,
            &report,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        ));
        openpit_destroy_engine(callback_engine);
    }

    #[test]
    fn execute_pre_trade_reject_path_returns_reject_list() {
        let engine = build_engine_with_main_reject_policy();
        let order = OpenPitOrder::default();
        let mut out_reservation = std::ptr::null_mut();
        let mut out_rejects: *mut OpenPitPretradeRejectList = std::ptr::null_mut();

        let status = openpit_engine_execute_pre_trade(
            engine,
            &order,
            &mut out_reservation,
            &mut out_rejects,
            std::ptr::null_mut(),
        );
        assert_eq!(status, OpenPitPretradeStatus::Rejected);
        assert!(!out_rejects.is_null());
        openpit_pretrade_destroy_reject_list(out_rejects);
        openpit_destroy_engine(engine);
    }

    #[test]
    fn direct_openpit_engine_paths_for_ffi_types_are_reachable() {
        let _ = EngineBuildError::DuplicatePolicyName {
            name: "dup".to_string(),
        }
        .to_string();

        let engine = openpit::EngineBuilder::<
            crate::order::Order,
            crate::execution_report::ExecutionReport,
            crate::account_adjustment::AccountAdjustment,
        >::new()
        .no_sync()
        .pre_trade(AlwaysRejectStart)
        .build()
        .expect("engine");

        let start = engine.start_pre_trade(crate::order::Order::default());
        assert!(start.is_err());

        let execute = engine.execute_pre_trade(crate::order::Order::default());
        assert!(execute.is_err());

        let report = openpit_interop::RequestWithPayload::new(
            openpit_interop::ExecutionReport {
                operation: openpit_interop::ExecutionReportOperationAccess::Absent,
                financial_impact: openpit_interop::FinancialImpactAccess::Absent,
                fill: openpit_interop::ExecutionReportFillAccess::Absent,
                position_impact: openpit_interop::ExecutionReportPositionImpactAccess::Absent,
            },
            std::ptr::null_mut(),
        );
        let post = engine.apply_execution_report(&report);
        assert!(post.account_blocks.is_empty());

        let apply = engine.apply_account_adjustment(
            openpit::param::AccountId::from_u64(1),
            &[crate::account_adjustment::AccountAdjustment::default()],
        );
        assert!(apply.is_ok());
    }
}
