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
    import_account_adjustment, AccountAdjustment, PitAccountAdjustment,
    PitAccountAdjustmentApplyStatus,
};
use crate::execution_report::{
    import_execution_report, ExecutionReport, PitExecutionReport, PitPretradePostTradeResult,
};
use crate::last_error::{write_error, PitOutError};
use crate::order::{import_order, Order, PitOrder};
use crate::param::{PitParamAccountId, PitParamPrice, PitParamPriceOptional};
use crate::reject::{rejects_to_list_owned, PitRejectList};
use openpit::param::AccountId;

//--------------------------------------------------------------------------------------------------

type Engine = openpit::Engine<Order, ExecutionReport, AccountAdjustment>;
type EngineBuilder = openpit::EngineBuilder<Order, ExecutionReport, AccountAdjustment>;

//--------------------------------------------------------------------------------------------------
// Threading:
// The SDK never spawns OS threads: each public call executes on the OS thread
// that invoked it. Concurrent entry into any public method on the same engine
// handle is undefined behavior and must be prevented by the caller. Sequential
// calls on the same handle from different OS threads are supported. In Go
// bindings, goroutine migration during one SDK call is supported, and callbacks
// into Go may run on a different OS thread than the goroutine that initiated
// the call; callback code must not rely on thread-local OS state.

/// Opaque builder pointer used to assemble an engine instance.
///
/// Ownership:
/// - returned by `pit_create_engine_builder`;
/// - owned by the caller until passed to `pit_destroy_engine_builder`;
/// - consumed by `pit_engine_builder_build`.
pub struct PitEngineBuilder {
    pub(crate) inner: Option<EngineBuilder>,
}

/// Opaque engine pointer.
///
/// The engine stores policies and mutable risk state. The caller owns the
/// pointer until `pit_destroy_engine`.
pub struct PitEngine {
    inner: Engine,
}

/// Opaque pointer for a deferred pre-trade request.
///
/// This is returned by `pit_engine_start_pre_trade`. It can be executed once
/// with `pit_pretrade_pre_trade_request_execute` or discarded with
/// `pit_destroy_pretrade_pre_trade_request`.
pub struct PitPretradePreTradeRequest {
    inner: Option<openpit::pretrade::PreTradeRequest<Order>>,
}

/// Opaque reservation pointer returned by a successful pre-trade check.
///
/// A reservation represents resources that have been tentatively locked. The
/// caller must resolve it exactly once by calling `pit_pretrade_pre_trade_reservation_commit`,
/// `pit_pretrade_pre_trade_reservation_rollback`, or `pit_destroy_pretrade_pre_trade_reservation`.
pub struct PitPretradePreTradeReservation {
    inner: openpit::pretrade::PreTradeReservation,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
/// Price-lock snapshot returned from a reservation.
pub struct PitPretradePreTradeLock {
    /// Optional reserved price.
    pub price: PitParamPriceOptional,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Result status for pre-trade operations.
pub enum PitPretradeStatus {
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
/// - created by `pit_engine_apply_account_adjustment` on `Rejected`;
/// - owned by the caller;
/// - released with `pit_destroy_account_adjustment_batch_error`.
pub struct PitAccountAdjustmentBatchError {
    /// Rejects produced by the policy.
    rejects: PitRejectList,
    /// Zero-based index of the failing adjustment.
    failed_adjustment_index: usize,
}

//--------------------------------------------------------------------------------------------------

pub(crate) fn with_builder(
    builder: &mut PitEngineBuilder,
    mutate: Box<dyn FnOnce(EngineBuilder) -> EngineBuilder + '_>,
) -> Result<(), String> {
    let current = builder
        .inner
        .take()
        .ok_or_else(|| "engine builder is no longer available".to_string())?;
    builder.inner = Some(mutate(current));
    Ok(())
}

impl PitAccountAdjustmentBatchError {
    fn new(err: openpit::AccountAdjustmentBatchError) -> Self {
        Self {
            failed_adjustment_index: err.failed_adjustment_index,
            rejects: rejects_to_list_owned(err.rejects),
        }
    }
}

fn export_pre_trade_lock(lock: &openpit::pretrade::PreTradeLock) -> PitPretradePreTradeLock {
    PitPretradePreTradeLock {
        price: match lock.price() {
            Some(v) => PitParamPriceOptional {
                is_set: true,
                value: PitParamPrice(v.to_decimal().into()),
            },
            None => PitParamPriceOptional::default(),
        },
    }
}

#[no_mangle]
/// Creates a new engine builder.
///
/// Contract:
/// - returns a new caller-owned builder object;
/// - this function always succeeds.
///
/// Cleanup:
/// - release the pointer with `pit_destroy_engine_builder` if you stop before
///   building;
/// - after a successful build the builder is consumed and must still be
///   released with `pit_destroy_engine_builder`.
pub extern "C" fn pit_create_engine_builder() -> *mut PitEngineBuilder {
    Box::into_raw(Box::new(PitEngineBuilder {
        inner: Some(Engine::builder()),
    }))
}

#[no_mangle]
/// Releases a builder pointer owned by the caller.
///
/// Contract:
/// - passing null is allowed;
/// - after this call the pointer is invalid;
/// - this function always succeeds.
pub extern "C" fn pit_destroy_engine_builder(builder: *mut PitEngineBuilder) {
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
/// - if `out_error` is not null, writes a caller-owned `PitSharedString`
///   error handle that MUST be released with `pit_destroy_shared_string`.
///
/// Ownership:
/// - on success the returned engine pointer is owned by the caller and must be
///   released with `pit_destroy_engine`;
/// - the builder becomes consumed regardless of success and must not be reused.
pub extern "C" fn pit_engine_builder_build(
    builder: *mut PitEngineBuilder,
    out_error: PitOutError,
) -> *mut PitEngine {
    if builder.is_null() {
        write_error(out_error, "engine builder is null");
        return std::ptr::null_mut();
    }

    let builder = unsafe { &mut *builder };
    let inner = match builder.inner.take() {
        Some(v) => v,
        None => {
            write_error(out_error, "engine builder already consumed");
            return std::ptr::null_mut();
        }
    };
    match inner.build() {
        Ok(engine) => Box::into_raw(Box::new(PitEngine { inner: engine })),
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
pub extern "C" fn pit_destroy_engine(engine: *mut PitEngine) {
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
///   caller-owned `PitSharedString` that MUST be destroyed by the caller.
///
/// Cleanup:
/// - release a successful request with `pit_pretrade_pre_trade_request_execute` or
///   `pit_destroy_pretrade_pre_trade_request`.
///
/// Reject ownership contract:
/// - on `Rejected`, a non-null `PitRejectList` pointer is written to `out_rejects`
///   if it is not null;
/// - the caller takes ownership and MUST release it with
///   `pit_destroy_reject_list`; failing to do so leaks the heap allocation made
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
pub extern "C" fn pit_engine_start_pre_trade(
    engine: *mut PitEngine,
    order: *const PitOrder,
    out_request: *mut *mut PitPretradePreTradeRequest,
    out_rejects: *mut *mut PitRejectList,
    out_error: PitOutError,
) -> PitPretradeStatus {
    if engine.is_null() {
        write_error(out_error, "engine is null");
        return PitPretradeStatus::Error;
    }
    if order.is_null() {
        write_error(out_error, "order is null");
        return PitPretradeStatus::Error;
    }

    // `start_pre_trade` stores the request for later execution, so the order
    // must become owned data before this function returns.
    let order = match import_order(unsafe { &*order }) {
        Ok(v) => v,
        Err(e) => {
            write_error(out_error, &e);
            return PitPretradeStatus::Error;
        }
    };

    match unsafe { &mut *engine }.inner.start_pre_trade(order) {
        Ok(request) => {
            if !out_request.is_null() {
                unsafe {
                    *out_request = Box::into_raw(Box::new(PitPretradePreTradeRequest {
                        inner: Some(request),
                    }))
                }
            }
            PitPretradeStatus::Passed
        }
        Err(rejects) => {
            if !out_rejects.is_null() {
                let PitRejectList { items } = rejects_to_list_owned(rejects);
                unsafe {
                    *out_rejects = Box::into_raw(Box::new(PitRejectList { items }));
                }
            }
            PitPretradeStatus::Rejected
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
///   caller-owned `PitSharedString` that MUST be destroyed by the caller.
///
/// Cleanup:
/// - release a successful reservation with `pit_pretrade_pre_trade_reservation_commit`,
///   `pit_pretrade_pre_trade_reservation_rollback`, or
///   `pit_destroy_pretrade_pre_trade_reservation`.
///
/// Reject ownership contract:
/// - on `Rejected`, a non-null `PitRejectList` pointer is written to
///   `out_rejects` if it is not null;
/// - the caller takes ownership and MUST release it with
///   `pit_destroy_reject_list`; failing to do so leaks the heap allocation made
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
pub extern "C" fn pit_engine_execute_pre_trade(
    engine: *mut PitEngine,
    order: *const PitOrder,
    out_reservation: *mut *mut PitPretradePreTradeReservation,
    out_rejects: *mut *mut PitRejectList,
    out_error: PitOutError,
) -> PitPretradeStatus {
    if engine.is_null() {
        write_error(out_error, "engine is null");
        return PitPretradeStatus::Error;
    }
    if order.is_null() {
        write_error(out_error, "order is null");
        return PitPretradeStatus::Error;
    }

    let order = match import_order(unsafe { &*order }) {
        Ok(v) => v,
        Err(e) => {
            write_error(out_error, &e);
            return PitPretradeStatus::Error;
        }
    };

    match unsafe { &mut *engine }.inner.execute_pre_trade(order) {
        Ok(reservation) => {
            if !out_reservation.is_null() {
                unsafe {
                    *out_reservation = Box::into_raw(Box::new(PitPretradePreTradeReservation {
                        inner: reservation,
                    }))
                }
            }
            PitPretradeStatus::Passed
        }
        Err(rejects) => {
            if !out_rejects.is_null() {
                let PitRejectList { items } = rejects_to_list_owned(rejects);
                unsafe {
                    *out_rejects = Box::into_raw(Box::new(PitRejectList { items }));
                }
            }
            PitPretradeStatus::Rejected
        }
    }
}

#[no_mangle]
/// Executes a deferred request returned by `pit_engine_start_pre_trade`.
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
///   caller-owned `PitSharedString` that MUST be destroyed by the caller.
///
/// Ownership:
/// - this call consumes the request object's content exactly once;
/// - after a successful or failed execute, the object itself may still
///   be released with `pit_destroy_pretrade_pre_trade_request`, but it cannot be executed again.
///
/// Reject ownership contract:
/// - on `Rejected`, a non-null `PitRejectList` pointer is written to
///   `out_rejects` if it is not null;
/// - the caller takes ownership and MUST release it with
///   `pit_destroy_reject_list`; failing to do so leaks the heap allocation made
///   inside this call;
/// - no thread-local state is involved, and the returned pointer is safe to
///   read on any thread;
/// - on `Passed` and `Error`, null is written to `out_rejects`, and the caller
///   must not call destroy in those cases.
pub extern "C" fn pit_pretrade_pre_trade_request_execute(
    request: *mut PitPretradePreTradeRequest,
    out_reservation: *mut *mut PitPretradePreTradeReservation,
    out_rejects: *mut *mut PitRejectList,
    out_error: PitOutError,
) -> PitPretradeStatus {
    if request.is_null() {
        write_error(out_error, "request is null");
        return PitPretradeStatus::Error;
    }

    let request = unsafe { &mut *request };
    let inner = match request.inner.take() {
        Some(v) => v,
        None => {
            write_error(out_error, "pre-trade request already consumed");
            return PitPretradeStatus::Error;
        }
    };

    match inner.execute() {
        Ok(reservation) => {
            if !out_reservation.is_null() {
                unsafe {
                    *out_reservation = Box::into_raw(Box::new(PitPretradePreTradeReservation {
                        inner: reservation,
                    }))
                };
            }
            PitPretradeStatus::Passed
        }
        Err(rejects) => {
            if !out_rejects.is_null() {
                let PitRejectList { items } = rejects_to_list_owned(rejects);
                unsafe {
                    *out_rejects = Box::into_raw(Box::new(PitRejectList { items }));
                }
            }
            PitPretradeStatus::Rejected
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
pub extern "C" fn pit_destroy_pretrade_pre_trade_request(request: *mut PitPretradePreTradeRequest) {
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
pub extern "C" fn pit_pretrade_pre_trade_reservation_commit(
    reservation: *mut PitPretradePreTradeReservation,
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
pub extern "C" fn pit_pretrade_pre_trade_reservation_rollback(
    reservation: *mut PitPretradePreTradeReservation,
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
pub extern "C" fn pit_pretrade_pre_trade_reservation_get_lock(
    reservation: *const PitPretradePreTradeReservation,
) -> PitPretradePreTradeLock {
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
pub extern "C" fn pit_destroy_pretrade_pre_trade_reservation(
    reservation: *mut PitPretradePreTradeReservation,
) {
    if reservation.is_null() {
        return;
    }
    unsafe { drop(Box::from_raw(reservation)) };
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
/// Result of `pit_engine_apply_execution_report`.
pub struct PitEngineApplyExecutionReportResult {
    /// The result of the post-trade processing if no error occurred.
    pub post_trade_result: PitPretradePostTradeResult,
    /// Whether the call failed at the transport level.
    pub is_error: bool,
}

#[no_mangle]
/// Applies an execution report to engine state.
///
/// Success:
/// - returns `PitEngineApplyExecutionReportResult { is_error = false, ... }`.
///
/// Error:
/// - returns `PitEngineApplyExecutionReportResult { is_error = true, post_trade_result = { kill_switch_triggered = false } }`
///   when input pointers are invalid or the report payload cannot be decoded;
/// - if `out_error` is not null, writes a caller-owned `PitSharedString`
///   error handle that MUST be released with `pit_destroy_shared_string`;
/// - when `is_error` is `true`, do not trust any other fields beyond the fact
///   that the call failed.
///
/// Lifetime contract:
/// - `report` is read as a borrowed view during this call only;
/// - the operation does not retain any pointer into source memory after this
///   function returns.
pub extern "C" fn pit_engine_apply_execution_report(
    engine: *mut PitEngine,
    report: *const PitExecutionReport,
    out_error: PitOutError,
) -> PitEngineApplyExecutionReportResult {
    if engine.is_null() {
        write_error(out_error, "engine is null");
        return PitEngineApplyExecutionReportResult {
            is_error: true,
            post_trade_result: PitPretradePostTradeResult {
                kill_switch_triggered: false,
            },
        };
    }
    if report.is_null() {
        write_error(out_error, "report is null");
        return PitEngineApplyExecutionReportResult {
            is_error: true,
            post_trade_result: PitPretradePostTradeResult {
                kill_switch_triggered: false,
            },
        };
    }

    let report = match import_execution_report(unsafe { &*report }) {
        Ok(v) => v,
        Err(e) => {
            write_error(out_error, &e);
            return PitEngineApplyExecutionReportResult {
                is_error: true,
                post_trade_result: PitPretradePostTradeResult {
                    kill_switch_triggered: false,
                },
            };
        }
    };

    let report = unsafe { &mut *engine }
        .inner
        .apply_execution_report(&report);

    PitEngineApplyExecutionReportResult {
        is_error: false,
        post_trade_result: PitPretradePostTradeResult {
            kill_switch_triggered: report.kill_switch_triggered,
        },
    }
}

#[no_mangle]
/// Releases a batch-error object returned by account-adjustment apply.
///
/// Contract:
/// - passing null is allowed;
/// - this function always succeeds.
pub extern "C" fn pit_destroy_account_adjustment_batch_error(
    batch_error: *mut PitAccountAdjustmentBatchError,
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
pub extern "C" fn pit_account_adjustment_batch_error_get_failed_adjustment_index(
    batch_error: *const PitAccountAdjustmentBatchError,
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
pub extern "C" fn pit_account_adjustment_batch_error_get_rejects(
    batch_error: *const PitAccountAdjustmentBatchError,
) -> *const PitRejectList {
    assert!(!batch_error.is_null(), "batch error pointer is null");
    let batch_error = unsafe { &*batch_error };
    &batch_error.rejects as *const PitRejectList
}

#[no_mangle]
/// Applies a batch of account adjustments to one account.
///
/// Success:
/// - returns `PitAccountAdjustmentApplyStatus::Applied` when the batch was
///   accepted and applied;
/// - returns `PitAccountAdjustmentApplyStatus::Rejected` when the call itself
///   completed normally but a policy rejected the batch; read `out_reject`.
///
/// Error:
/// - returns `PitAccountAdjustmentApplyStatus::Error` when input pointers are
///   invalid or some adjustment payload cannot be decoded;
/// - on `Error`, if `out_error` is not null, it is filled with a
///   caller-owned `PitSharedString` that MUST be destroyed by the caller.
///
/// Result handling:
/// - `Applied` means there is no reject object to clean up;
/// - `Rejected` stores batch error details in `out_reject`, the caller must
///   release a returned object with `pit_destroy_account_adjustment_batch_error`;
/// - rejects returned by `pit_account_adjustment_batch_error_get_rejects`
///   contain string views borrowed from the batch error and must not be used
///   after the batch error is destroyed;
/// - when `Error` is returned, do not use any pointer from a previous
///   unrelated call as if it belonged to this failure.
///
/// Lifetime contract:
/// - every `adjustment` entry from the contiguous input array is read as a
///   borrowed view during this call only;
/// - release a returned batch error with
///   `pit_destroy_account_adjustment_batch_error`.
pub extern "C" fn pit_engine_apply_account_adjustment(
    engine: *mut PitEngine,
    account_id: PitParamAccountId,
    adjustments: *const PitAccountAdjustment,
    adjustments_len: usize,
    out_reject: *mut *mut PitAccountAdjustmentBatchError,
    out_error: PitOutError,
) -> PitAccountAdjustmentApplyStatus {
    if engine.is_null() {
        write_error(out_error, "engine is null");
        return PitAccountAdjustmentApplyStatus::Error;
    }
    if adjustments_len > 0 && adjustments.is_null() {
        write_error(out_error, "adjustments is null");
        return PitAccountAdjustmentApplyStatus::Error;
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
                    return PitAccountAdjustmentApplyStatus::Error;
                }
            };
            values.push(parsed);
        }
        values
    };

    match unsafe { &mut *engine }
        .inner
        .apply_account_adjustment(AccountId::from_u64(account_id), &adjustments)
    {
        Ok(()) => PitAccountAdjustmentApplyStatus::Applied,
        Err(err) => {
            if !out_reject.is_null() {
                unsafe {
                    *out_reject = Box::into_raw(Box::new(PitAccountAdjustmentBatchError::new(err)))
                }
            }
            PitAccountAdjustmentApplyStatus::Rejected
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::c_void;

    use openpit::pretrade::{CheckPreTradeStartPolicy, Reject, RejectCode, RejectScope, Rejects};
    use openpit::EngineBuildError;

    use crate::account_adjustment::PitAccountAdjustment;
    use crate::account_adjustment::PitAccountAdjustmentApplyStatus;
    use crate::engine::PitPretradeStatus;
    use crate::execution_report::{
        PitExecutionReport, PitExecutionReportOperation, PitExecutionReportOperationOptional,
        PitExecutionReportPositionImpactOptional, PitFinancialImpactOptional,
        PitPretradePostTradeResult,
    };
    use crate::order::PitOrder;
    use crate::policy::{
        pit_create_custom_account_adjustment_policy,
        pit_create_pretrade_custom_check_pre_trade_start_policy,
        pit_destroy_account_adjustment_policy, pit_destroy_pretrade_check_pre_trade_start_policy,
        pit_engine_builder_add_account_adjustment_policy,
        pit_engine_builder_add_check_pre_trade_start_policy,
    };
    use crate::reject::{
        pit_create_reject_list, pit_destroy_reject_list, pit_reject_list_get, pit_reject_list_len,
        PitReject, PitRejectCode, PitRejectList, PitRejectScope,
    };
    use crate::PitStringView;

    use super::{
        pit_account_adjustment_batch_error_get_failed_adjustment_index,
        pit_account_adjustment_batch_error_get_rejects, pit_create_engine_builder,
        pit_destroy_account_adjustment_batch_error, pit_destroy_engine, pit_destroy_engine_builder,
        pit_destroy_pretrade_pre_trade_request, pit_destroy_pretrade_pre_trade_reservation,
        pit_engine_apply_account_adjustment, pit_engine_apply_execution_report,
        pit_engine_builder_build, pit_engine_execute_pre_trade, pit_engine_start_pre_trade,
        pit_pretrade_pre_trade_request_execute, pit_pretrade_pre_trade_reservation_commit,
        pit_pretrade_pre_trade_reservation_get_lock, pit_pretrade_pre_trade_reservation_rollback,
        PitAccountAdjustmentBatchError, PitEngineApplyExecutionReportResult,
        PitPretradePreTradeLock,
    };

    struct AlwaysRejectStart;

    impl CheckPreTradeStartPolicy<crate::order::Order, crate::execution_report::ExecutionReport>
        for AlwaysRejectStart
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
        ) -> bool {
            false
        }
    }

    unsafe extern "C" fn always_reject_apply(
        _ctx: *const crate::policy::PitAccountAdjustmentContext,
        _account_id: crate::param::PitParamAccountId,
        _adjustment: *const PitAccountAdjustment,
        _mutations: *mut crate::policy::PitMutations,
        _user_data: *mut c_void,
    ) -> *mut PitRejectList {
        let rejects = pit_create_reject_list(1);
        crate::reject::pit_reject_list_push(
            rejects,
            PitReject {
                policy: PitStringView::from_utf8("test_policy"),
                reason: PitStringView::from_utf8("test_reason"),
                details: PitStringView::from_utf8("test_details"),
                user_data: std::ptr::null_mut(),
                code: PitRejectCode::AccountBlocked,
                scope: PitRejectScope::Account,
            },
        );
        rejects
    }

    unsafe extern "C" fn always_pass_start_check(
        _ctx: *const crate::policy::PitPretradeContext,
        _order: *const crate::order::PitOrder,
        _user_data: *mut c_void,
    ) -> *mut PitRejectList {
        std::ptr::null_mut()
    }

    unsafe extern "C" fn always_reject_start_check(
        _ctx: *const crate::policy::PitPretradeContext,
        _order: *const crate::order::PitOrder,
        _user_data: *mut c_void,
    ) -> *mut PitRejectList {
        let rejects = pit_create_reject_list(1);
        crate::reject::pit_reject_list_push(
            rejects,
            PitReject {
                policy: PitStringView::from_utf8("start.reject"),
                reason: PitStringView::from_utf8("blocked"),
                details: PitStringView::from_utf8("by test"),
                user_data: std::ptr::null_mut(),
                code: PitRejectCode::OrderExceedsLimit,
                scope: PitRejectScope::Order,
            },
        );
        rejects
    }

    unsafe extern "C" fn always_reject_pre_trade(
        _ctx: *const crate::policy::PitPretradeContext,
        _order: *const crate::order::PitOrder,
        _mutations: *mut crate::policy::PitMutations,
        _user_data: *mut c_void,
    ) -> *mut PitRejectList {
        let rejects = pit_create_reject_list(1);
        let reject = PitReject {
            policy: PitStringView::from_utf8("pretrade.reject"),
            reason: PitStringView::from_utf8("blocked"),
            details: PitStringView::from_utf8("by test"),
            user_data: std::ptr::null_mut(),
            code: PitRejectCode::RiskLimitExceeded,
            scope: PitRejectScope::Order,
        };
        crate::reject::pit_reject_list_push(rejects, reject);
        rejects
    }

    unsafe extern "C" fn always_false_apply_report(
        _report: *const crate::execution_report::PitExecutionReport,
        _user_data: *mut c_void,
    ) -> bool {
        false
    }

    unsafe extern "C" fn noop_free_user_data(_user_data: *mut c_void) {}

    fn build_engine_with_reject_policy() -> *mut super::PitEngine {
        let builder = pit_create_engine_builder();
        let policy_name = PitStringView::from_utf8("test_policy");
        let policy = unsafe {
            pit_create_custom_account_adjustment_policy(
                policy_name,
                always_reject_apply,
                noop_free_user_data,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert!(!policy.is_null(), "failed to create policy");
        let ok =
            pit_engine_builder_add_account_adjustment_policy(builder, policy, std::ptr::null_mut());
        assert!(ok, "failed to add policy");
        pit_destroy_account_adjustment_policy(policy);
        let engine = pit_engine_builder_build(builder, std::ptr::null_mut());
        assert!(!engine.is_null(), "engine build failed");
        engine
    }

    fn build_engine_with_main_reject_policy() -> *mut super::PitEngine {
        let builder = pit_create_engine_builder();
        let name = PitStringView::from_utf8("pretrade.reject");
        let policy = unsafe {
            crate::policy::pit_create_pretrade_custom_pre_trade_policy(
                name,
                always_reject_pre_trade,
                always_false_apply_report,
                noop_free_user_data,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert!(!policy.is_null(), "failed to create policy");
        let ok = crate::policy::pit_engine_builder_add_pre_trade_policy(
            builder,
            policy,
            std::ptr::null_mut(),
        );
        assert!(ok, "failed to add policy");
        crate::policy::pit_destroy_pretrade_pre_trade_policy(policy);
        let engine = pit_engine_builder_build(builder, std::ptr::null_mut());
        assert!(!engine.is_null(), "engine build failed");
        engine
    }

    fn build_engine_with_start_reject_policy() -> *mut super::PitEngine {
        let builder = pit_create_engine_builder();
        let name = PitStringView::from_utf8("start.reject");
        let policy = unsafe {
            pit_create_pretrade_custom_check_pre_trade_start_policy(
                name,
                always_reject_start_check,
                always_false_apply_report,
                noop_free_user_data,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert!(!policy.is_null(), "failed to create policy");
        let ok = pit_engine_builder_add_check_pre_trade_start_policy(
            builder,
            policy,
            std::ptr::null_mut(),
        );
        assert!(ok, "failed to add policy");
        pit_destroy_pretrade_check_pre_trade_start_policy(policy);
        let engine = pit_engine_builder_build(builder, std::ptr::null_mut());
        assert!(!engine.is_null(), "engine build failed");
        engine
    }

    fn build_engine_with_start_pass_policy() -> *mut super::PitEngine {
        let builder = pit_create_engine_builder();
        let name = PitStringView::from_utf8("start.pass");
        let policy = unsafe {
            pit_create_pretrade_custom_check_pre_trade_start_policy(
                name,
                always_pass_start_check,
                always_false_apply_report,
                noop_free_user_data,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert!(!policy.is_null(), "failed to create policy");
        let ok = pit_engine_builder_add_check_pre_trade_start_policy(
            builder,
            policy,
            std::ptr::null_mut(),
        );
        assert!(ok, "failed to add policy");
        pit_destroy_pretrade_check_pre_trade_start_policy(policy);
        let engine = pit_engine_builder_build(builder, std::ptr::null_mut());
        assert!(!engine.is_null(), "engine build failed");
        engine
    }

    fn string_view_to_string(view: PitStringView) -> String {
        if view.ptr.is_null() {
            return String::new();
        }
        let bytes = unsafe { std::slice::from_raw_parts(view.ptr, view.len) };
        std::str::from_utf8(bytes).expect("utf8").to_string()
    }

    #[test]
    fn string_view_to_string_handles_null_pointer() {
        assert_eq!(string_view_to_string(PitStringView::not_set()), "");
    }

    #[test]
    fn engine_builder_build_reports_null_consumed_and_validation_errors() {
        let engine = pit_engine_builder_build(std::ptr::null_mut(), std::ptr::null_mut());
        assert!(engine.is_null());

        let builder = pit_create_engine_builder();
        let built = pit_engine_builder_build(builder, std::ptr::null_mut());
        assert!(!built.is_null());
        pit_destroy_engine(built);
        let consumed = pit_engine_builder_build(builder, std::ptr::null_mut());
        assert!(consumed.is_null());
        pit_destroy_engine_builder(builder);

        let dup_builder = pit_create_engine_builder();
        let dup_name = PitStringView::from_utf8("dup.start");
        let first = unsafe {
            pit_create_pretrade_custom_check_pre_trade_start_policy(
                dup_name,
                always_pass_start_check,
                always_false_apply_report,
                noop_free_user_data,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        let second = unsafe {
            pit_create_pretrade_custom_check_pre_trade_start_policy(
                dup_name,
                always_pass_start_check,
                always_false_apply_report,
                noop_free_user_data,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert!(!first.is_null() && !second.is_null());
        assert!(pit_engine_builder_add_check_pre_trade_start_policy(
            dup_builder,
            first,
            std::ptr::null_mut()
        ));
        assert!(pit_engine_builder_add_check_pre_trade_start_policy(
            dup_builder,
            second,
            std::ptr::null_mut()
        ));
        pit_destroy_pretrade_check_pre_trade_start_policy(first);
        pit_destroy_pretrade_check_pre_trade_start_policy(second);

        let invalid = pit_engine_builder_build(dup_builder, std::ptr::null_mut());
        assert!(invalid.is_null());
        pit_destroy_engine_builder(dup_builder);
    }

    #[test]
    fn add_policy_on_consumed_builder_returns_error() {
        let builder = pit_create_engine_builder();
        let engine = pit_engine_builder_build(builder, std::ptr::null_mut());
        assert!(!engine.is_null());
        pit_destroy_engine(engine);

        let name = PitStringView::from_utf8("consumed.builder");
        let policy = unsafe {
            pit_create_pretrade_custom_check_pre_trade_start_policy(
                name,
                always_pass_start_check,
                always_false_apply_report,
                noop_free_user_data,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert!(!policy.is_null());
        let ok = pit_engine_builder_add_check_pre_trade_start_policy(
            builder,
            policy,
            std::ptr::null_mut(),
        );
        assert!(!ok);
        pit_destroy_pretrade_check_pre_trade_start_policy(policy);
        pit_destroy_engine_builder(builder);
    }

    #[test]
    fn start_pre_trade_does_not_touch_out_values_on_error() {
        let mut out_request = std::ptr::dangling_mut::<super::PitPretradePreTradeRequest>();
        let mut out_rejects = std::ptr::null_mut::<PitRejectList>();

        let status = pit_engine_start_pre_trade(
            std::ptr::null_mut(),
            std::ptr::null(),
            &mut out_request,
            &mut out_rejects,
            std::ptr::null_mut(),
        );

        assert_eq!(status, PitPretradeStatus::Error);
        assert_eq!(
            out_request,
            std::ptr::dangling_mut::<super::PitPretradePreTradeRequest>()
        );
        assert!(out_rejects.is_null());
    }

    #[test]
    fn start_pre_trade_covers_null_order_and_reject_outputs() {
        let engine = build_engine_with_start_reject_policy();
        let mut out_request = std::ptr::null_mut();
        let mut out_rejects = std::ptr::null_mut::<PitRejectList>();
        let order = PitOrder::default();

        let status = pit_engine_start_pre_trade(
            engine,
            std::ptr::null(),
            &mut out_request,
            &mut out_rejects,
            std::ptr::null_mut(),
        );
        assert_eq!(status, PitPretradeStatus::Error);
        assert!(out_rejects.is_null());

        let status = pit_engine_start_pre_trade(
            engine,
            &order,
            &mut out_request,
            &mut out_rejects,
            std::ptr::null_mut(),
        );
        assert_eq!(status, PitPretradeStatus::Rejected);
        assert!(!out_rejects.is_null());
        pit_destroy_reject_list(out_rejects);

        let status = pit_engine_start_pre_trade(
            engine,
            &order,
            &mut out_request,
            &mut out_rejects,
            std::ptr::null_mut(),
        );
        assert_eq!(status, PitPretradeStatus::Rejected);
        assert!(!out_rejects.is_null());
        pit_destroy_reject_list(out_rejects);

        pit_destroy_engine(engine);
    }

    #[test]
    fn start_pre_trade_pass_path_covers_null_out_request_pointer() {
        let engine = build_engine_with_start_pass_policy();
        let order = PitOrder::default();
        let mut out_rejects = std::ptr::null_mut::<PitRejectList>();

        let status = pit_engine_start_pre_trade(
            engine,
            &order,
            std::ptr::null_mut(),
            &mut out_rejects,
            std::ptr::null_mut(),
        );
        assert_eq!(status, PitPretradeStatus::Passed);
        assert!(out_rejects.is_null());

        pit_destroy_engine(engine);
    }

    #[test]
    fn execute_pre_trade_does_not_touch_out_values_on_error() {
        let mut out_reservation = std::ptr::dangling_mut::<super::PitPretradePreTradeReservation>();
        let mut out_rejects = std::ptr::null_mut::<PitRejectList>();

        let status = pit_engine_execute_pre_trade(
            std::ptr::null_mut(),
            std::ptr::null(),
            &mut out_reservation,
            &mut out_rejects,
            std::ptr::null_mut(),
        );

        assert_eq!(status, PitPretradeStatus::Error);
        assert_eq!(
            out_reservation,
            std::ptr::dangling_mut::<super::PitPretradePreTradeReservation>()
        );
        assert!(out_rejects.is_null());
    }

    #[test]
    fn execute_pre_trade_covers_null_order_and_optional_output_paths() {
        let order = PitOrder::default();
        let mut out_rejects = std::ptr::null_mut::<PitRejectList>();

        let builder = pit_create_engine_builder();
        let engine = pit_engine_builder_build(builder, std::ptr::null_mut());
        assert!(!engine.is_null());
        let status = pit_engine_execute_pre_trade(
            engine,
            std::ptr::null(),
            std::ptr::null_mut(),
            &mut out_rejects,
            std::ptr::null_mut(),
        );
        assert_eq!(status, PitPretradeStatus::Error);
        assert!(out_rejects.is_null());
        let status = pit_engine_execute_pre_trade(
            engine,
            &order,
            std::ptr::null_mut(),
            &mut out_rejects,
            std::ptr::null_mut(),
        );
        assert_eq!(status, PitPretradeStatus::Passed);
        assert!(out_rejects.is_null());
        pit_destroy_engine(engine);

        let reject_engine = build_engine_with_main_reject_policy();
        let status = pit_engine_execute_pre_trade(
            reject_engine,
            &order,
            std::ptr::null_mut(),
            &mut out_rejects,
            std::ptr::null_mut(),
        );
        assert_eq!(status, PitPretradeStatus::Rejected);
        assert!(!out_rejects.is_null());
        pit_destroy_reject_list(out_rejects);

        pit_destroy_engine(reject_engine);
    }

    #[test]
    fn request_execute_does_not_touch_out_values_on_error() {
        let mut out_reservation = std::ptr::dangling_mut::<super::PitPretradePreTradeReservation>();
        let mut out_rejects = std::ptr::null_mut::<PitRejectList>();

        let status = pit_pretrade_pre_trade_request_execute(
            std::ptr::null_mut(),
            &mut out_reservation,
            &mut out_rejects,
            std::ptr::null_mut(),
        );

        assert_eq!(status, PitPretradeStatus::Error);
        assert_eq!(
            out_reservation,
            std::ptr::dangling_mut::<super::PitPretradePreTradeReservation>()
        );
        assert!(out_rejects.is_null());
    }

    #[test]
    fn request_execute_covers_success_reject_and_consumed_paths() {
        let order = PitOrder::default();
        let mut out_rejects = std::ptr::null_mut::<PitRejectList>();

        let builder = pit_create_engine_builder();
        let engine = pit_engine_builder_build(builder, std::ptr::null_mut());
        assert!(!engine.is_null());
        let mut request = std::ptr::null_mut();
        let status = pit_engine_start_pre_trade(
            engine,
            &order,
            &mut request,
            &mut out_rejects,
            std::ptr::null_mut(),
        );
        assert_eq!(status, PitPretradeStatus::Passed);
        assert!(!request.is_null());
        assert!(out_rejects.is_null());
        let mut reservation = std::ptr::null_mut();
        let status = pit_pretrade_pre_trade_request_execute(
            request,
            &mut reservation,
            &mut out_rejects,
            std::ptr::null_mut(),
        );
        assert_eq!(status, PitPretradeStatus::Passed);
        assert!(!reservation.is_null());
        assert!(out_rejects.is_null());
        pit_pretrade_pre_trade_reservation_rollback(reservation);
        pit_destroy_pretrade_pre_trade_reservation(reservation);
        let status = pit_pretrade_pre_trade_request_execute(
            request,
            std::ptr::null_mut(),
            &mut out_rejects,
            std::ptr::null_mut(),
        );
        assert_eq!(status, PitPretradeStatus::Error);
        assert!(out_rejects.is_null());
        pit_destroy_pretrade_pre_trade_request(request);
        pit_destroy_engine(engine);

        let reject_engine = build_engine_with_main_reject_policy();
        let mut reject_request = std::ptr::null_mut();
        let status = pit_engine_start_pre_trade(
            reject_engine,
            &order,
            &mut reject_request,
            &mut out_rejects,
            std::ptr::null_mut(),
        );
        assert_eq!(status, PitPretradeStatus::Passed);
        assert!(!reject_request.is_null());
        assert!(out_rejects.is_null());
        let status = pit_pretrade_pre_trade_request_execute(
            reject_request,
            std::ptr::null_mut(),
            &mut out_rejects,
            std::ptr::null_mut(),
        );
        assert_eq!(status, PitPretradeStatus::Rejected);
        assert!(!out_rejects.is_null());
        pit_destroy_reject_list(out_rejects);
        pit_destroy_pretrade_pre_trade_request(reject_request);
        pit_destroy_engine(reject_engine);
    }

    #[test]
    fn apply_account_adjustment_accepts_null_when_batch_is_empty() {
        let builder = pit_create_engine_builder();
        let engine = pit_engine_builder_build(builder, std::ptr::null_mut());
        assert!(!engine.is_null(), "engine build failed");
        let mut out_reject = std::ptr::null_mut();

        let status = pit_engine_apply_account_adjustment(
            engine,
            1,
            std::ptr::null(),
            0,
            &mut out_reject,
            std::ptr::null_mut(),
        );

        assert_eq!(status, PitAccountAdjustmentApplyStatus::Applied);
        assert!(out_reject.is_null());
        pit_destroy_engine(engine);
    }

    #[test]
    fn apply_account_adjustment_rejects_null_when_batch_is_non_empty() {
        let builder = pit_create_engine_builder();
        let engine = pit_engine_builder_build(builder, std::ptr::null_mut());
        assert!(!engine.is_null(), "engine build failed");
        let mut out_reject = std::ptr::null_mut::<PitAccountAdjustmentBatchError>();

        let status = pit_engine_apply_account_adjustment(
            engine,
            1,
            std::ptr::null(),
            1,
            &mut out_reject,
            std::ptr::null_mut(),
        );

        assert_eq!(status, PitAccountAdjustmentApplyStatus::Error);
        assert!(out_reject.is_null());
        pit_destroy_engine(engine);
    }

    #[test]
    fn apply_account_adjustment_reports_import_error_for_incomplete_payload() {
        let builder = pit_create_engine_builder();
        let engine = pit_engine_builder_build(builder, std::ptr::null_mut());
        assert!(!engine.is_null(), "engine build failed");

        let invalid = crate::account_adjustment::PitAccountAdjustment {
            balance_operation:
                crate::account_adjustment::PitAccountAdjustmentBalanceOperationOptional::default(),
            position_operation:
                crate::account_adjustment::PitAccountAdjustmentPositionOperationOptional {
                    value: crate::account_adjustment::PitAccountAdjustmentPositionOperation {
                        mode: crate::param::PitParamPositionMode::Hedged,
                        ..Default::default()
                    },
                    is_set: true,
                },
            amount: crate::account_adjustment::PitAccountAdjustmentAmountOptional::default(),
            bounds: crate::account_adjustment::PitAccountAdjustmentBoundsOptional::default(),
            user_data: std::ptr::null_mut(),
        };
        let batch = [invalid];
        let mut out_reject = std::ptr::null_mut::<PitAccountAdjustmentBatchError>();
        let status = pit_engine_apply_account_adjustment(
            engine,
            1,
            batch.as_ptr(),
            batch.len(),
            &mut out_reject,
            std::ptr::null_mut(),
        );
        assert_eq!(status, PitAccountAdjustmentApplyStatus::Error);
        assert!(out_reject.is_null());
        pit_destroy_engine(engine);
    }

    #[test]
    fn apply_account_adjustment_reports_error_for_null_engine() {
        let status = pit_engine_apply_account_adjustment(
            std::ptr::null_mut(),
            1,
            std::ptr::null(),
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
        assert_eq!(status, PitAccountAdjustmentApplyStatus::Error);
    }

    #[test]
    fn lock_snapshot_defaults_to_absent_price() {
        let detached = PitPretradePreTradeLock::default();
        assert!(!detached.price.is_set);
    }

    #[test]
    fn reservation_get_lock_covers_success_and_committed_paths() {
        let builder = pit_create_engine_builder();
        let engine = pit_engine_builder_build(builder, std::ptr::null_mut());
        assert!(!engine.is_null());
        let order = PitOrder::default();
        let mut out_reservation = std::ptr::null_mut();
        let mut out_rejects = std::ptr::null_mut::<PitRejectList>();
        let status = pit_engine_execute_pre_trade(
            engine,
            &order,
            &mut out_reservation,
            &mut out_rejects,
            std::ptr::null_mut(),
        );
        assert_eq!(status, PitPretradeStatus::Passed);
        assert!(!out_reservation.is_null());
        assert!(out_rejects.is_null());

        let lock = pit_pretrade_pre_trade_reservation_get_lock(out_reservation);
        assert!(!lock.price.is_set);

        pit_pretrade_pre_trade_reservation_commit(out_reservation);
        let committed_lock = pit_pretrade_pre_trade_reservation_get_lock(out_reservation);
        assert!(!committed_lock.price.is_set);

        pit_destroy_pretrade_pre_trade_reservation(out_reservation);
        pit_destroy_engine(engine);
    }

    #[test]
    fn apply_account_adjustment_stores_index_on_reject() {
        let engine = build_engine_with_reject_policy();
        let adj = crate::account_adjustment::PitAccountAdjustment::default();
        let mut out_reject = std::ptr::null_mut::<PitAccountAdjustmentBatchError>();

        // First element (index 0) should be rejected.
        let batch = [adj];
        let status = pit_engine_apply_account_adjustment(
            engine,
            1,
            batch.as_ptr(),
            1,
            &mut out_reject,
            std::ptr::null_mut(),
        );
        assert_eq!(status, PitAccountAdjustmentApplyStatus::Rejected);
        assert!(!out_reject.is_null());

        let index = pit_account_adjustment_batch_error_get_failed_adjustment_index(out_reject);
        assert_eq!(index, 0);
        let rejects = pit_account_adjustment_batch_error_get_rejects(out_reject);
        assert!(!rejects.is_null());
        assert_eq!(pit_reject_list_len(rejects), 1);
        let mut reject = PitReject {
            code: PitRejectCode::Other,
            reason: PitStringView::not_set(),
            details: PitStringView::not_set(),
            policy: PitStringView::not_set(),
            user_data: std::ptr::null_mut(),
            scope: PitRejectScope::Order,
        };
        assert!(pit_reject_list_get(rejects, 0, &mut reject));
        assert_eq!(reject.code, PitRejectCode::AccountBlocked);

        pit_destroy_account_adjustment_batch_error(out_reject);
        pit_destroy_engine(engine);
    }

    #[test]
    fn apply_account_adjustment_stores_all_reject_fields() {
        let engine = build_engine_with_reject_policy();
        let adj = crate::account_adjustment::PitAccountAdjustment::default();
        let mut out_reject = std::ptr::null_mut::<PitAccountAdjustmentBatchError>();

        let batch = [adj];
        let status = pit_engine_apply_account_adjustment(
            engine,
            1,
            batch.as_ptr(),
            1,
            &mut out_reject,
            std::ptr::null_mut(),
        );
        assert_eq!(status, PitAccountAdjustmentApplyStatus::Rejected);
        assert!(!out_reject.is_null());

        let rejects = pit_account_adjustment_batch_error_get_rejects(out_reject);
        assert!(!rejects.is_null());
        assert_eq!(pit_reject_list_len(rejects), 1);
        let mut reject_ptr = PitReject {
            code: PitRejectCode::Other,
            reason: PitStringView::not_set(),
            details: PitStringView::not_set(),
            policy: PitStringView::not_set(),
            user_data: std::ptr::null_mut(),
            scope: PitRejectScope::Order,
        };
        assert!(pit_reject_list_get(rejects, 0, &mut reject_ptr));
        assert_eq!(reject_ptr.code, PitRejectCode::AccountBlocked);
        assert_eq!(reject_ptr.scope, PitRejectScope::Account);
        assert_eq!(reject_ptr.user_data, std::ptr::null_mut());

        let policy = string_view_to_string(reject_ptr.policy);
        assert_eq!(policy, "test_policy");
        let reason = string_view_to_string(reject_ptr.reason);
        assert_eq!(reason, "test_reason");
        let details = string_view_to_string(reject_ptr.details);
        assert_eq!(details, "test_details");

        pit_destroy_account_adjustment_batch_error(out_reject);
        pit_destroy_engine(engine);
    }

    #[test]
    fn apply_account_adjustment_out_reject_can_be_omitted() {
        let engine = build_engine_with_reject_policy();
        let adj = crate::account_adjustment::PitAccountAdjustment::default();

        let batch = [adj];
        let status = pit_engine_apply_account_adjustment(
            engine,
            1,
            batch.as_ptr(),
            1,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
        assert_eq!(status, PitAccountAdjustmentApplyStatus::Rejected);

        pit_destroy_engine(engine);
    }

    #[test]
    fn apply_account_adjustment_does_not_touch_out_reject_on_transport_error() {
        let engine = build_engine_with_reject_policy();
        let mut out_reject = std::ptr::dangling_mut::<PitAccountAdjustmentBatchError>();
        let status = pit_engine_apply_account_adjustment(
            engine,
            1,
            std::ptr::null(),
            1,
            &mut out_reject,
            std::ptr::null_mut(),
        );
        assert_eq!(status, PitAccountAdjustmentApplyStatus::Error);
        assert_eq!(
            out_reject,
            std::ptr::dangling_mut::<PitAccountAdjustmentBatchError>()
        );

        pit_destroy_engine(engine);
    }

    #[test]
    fn account_adjustment_batch_error_destroy_is_null_safe() {
        pit_destroy_account_adjustment_batch_error(std::ptr::null_mut());
    }

    #[test]
    fn destroy_functions_and_apply_report_api_are_callable_via_public_ffi() {
        pit_destroy_engine_builder(std::ptr::null_mut());
        pit_destroy_engine(std::ptr::null_mut());
        pit_destroy_pretrade_pre_trade_request(std::ptr::null_mut());
        pit_destroy_pretrade_pre_trade_reservation(std::ptr::null_mut());
        pit_pretrade_pre_trade_reservation_commit(std::ptr::null_mut());
        pit_pretrade_pre_trade_reservation_rollback(std::ptr::null_mut());
        let builder = pit_create_engine_builder();
        let engine = pit_engine_builder_build(builder, std::ptr::null_mut());
        assert!(!engine.is_null());

        let order = PitOrder::default();
        let mut out_request = std::ptr::null_mut();
        let mut out_rejects = std::ptr::null_mut::<PitRejectList>();
        let status = pit_engine_start_pre_trade(
            engine,
            &order,
            &mut out_request,
            &mut out_rejects,
            std::ptr::null_mut(),
        );
        assert_eq!(status, PitPretradeStatus::Passed);
        assert!(!out_request.is_null());
        assert!(out_rejects.is_null());
        pit_destroy_pretrade_pre_trade_request(out_request);

        let mut out_reservation = std::ptr::null_mut();
        let status = pit_engine_execute_pre_trade(
            engine,
            &order,
            &mut out_reservation,
            &mut out_rejects,
            std::ptr::null_mut(),
        );
        assert_eq!(status, PitPretradeStatus::Passed);
        assert!(!out_reservation.is_null());
        assert!(out_rejects.is_null());
        pit_pretrade_pre_trade_reservation_commit(out_reservation);
        pit_destroy_pretrade_pre_trade_reservation(out_reservation);

        let mut out_reservation2 = std::ptr::null_mut();
        let status = pit_engine_execute_pre_trade(
            engine,
            &order,
            &mut out_reservation2,
            &mut out_rejects,
            std::ptr::null_mut(),
        );
        assert_eq!(status, PitPretradeStatus::Passed);
        assert!(!out_reservation2.is_null());
        assert!(out_rejects.is_null());
        pit_pretrade_pre_trade_reservation_rollback(out_reservation2);
        pit_destroy_pretrade_pre_trade_reservation(out_reservation2);

        let report = PitExecutionReport::default();
        let post = pit_engine_apply_execution_report(engine, &report, std::ptr::null_mut());
        assert_eq!(
            post,
            PitEngineApplyExecutionReportResult {
                is_error: false,
                post_trade_result: PitPretradePostTradeResult {
                    kill_switch_triggered: false
                }
            }
        );

        pit_destroy_engine(engine);
    }

    #[test]
    fn apply_execution_report_covers_error_paths_and_custom_apply_callback() {
        let post = pit_engine_apply_execution_report(
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null_mut(),
        );
        assert!(post.is_error);
        assert!(!post.post_trade_result.kill_switch_triggered);

        let builder = pit_create_engine_builder();
        let engine = pit_engine_builder_build(builder, std::ptr::null_mut());
        assert!(!engine.is_null());

        let post =
            pit_engine_apply_execution_report(engine, std::ptr::null(), std::ptr::null_mut());
        assert!(post.is_error);
        assert!(!post.post_trade_result.kill_switch_triggered);

        let invalid = PitExecutionReport {
            operation: PitExecutionReportOperationOptional {
                is_set: true,
                value: PitExecutionReportOperation {
                    instrument: crate::instrument::PitInstrument {
                        underlying_asset: PitStringView::from_utf8("AAPL"),
                        settlement_asset: PitStringView::default(),
                    },
                    ..PitExecutionReportOperation::default()
                },
            },
            financial_impact: PitFinancialImpactOptional::default(),
            fill: crate::execution_report::PitExecutionReportFillOptional::default(),
            position_impact: PitExecutionReportPositionImpactOptional::default(),
            user_data: std::ptr::null_mut(),
        };
        let post = pit_engine_apply_execution_report(engine, &invalid, std::ptr::null_mut());
        assert!(post.is_error);
        assert!(!post.post_trade_result.kill_switch_triggered);
        pit_destroy_engine(engine);

        let callback_engine = build_engine_with_main_reject_policy();
        let report = PitExecutionReport::default();
        let post =
            pit_engine_apply_execution_report(callback_engine, &report, std::ptr::null_mut());
        assert!(!post.is_error);
        assert!(!post.post_trade_result.kill_switch_triggered);
        pit_destroy_engine(callback_engine);
    }

    #[test]
    fn execute_pre_trade_reject_path_returns_reject_list() {
        let engine = build_engine_with_main_reject_policy();
        let order = PitOrder::default();
        let mut out_reservation = std::ptr::null_mut();
        let mut out_rejects = std::ptr::null_mut::<PitRejectList>();

        let status = pit_engine_execute_pre_trade(
            engine,
            &order,
            &mut out_reservation,
            &mut out_rejects,
            std::ptr::null_mut(),
        );
        assert_eq!(status, PitPretradeStatus::Rejected);
        assert!(!out_rejects.is_null());
        pit_destroy_reject_list(out_rejects);
        pit_destroy_engine(engine);
    }

    #[test]
    fn direct_openpit_engine_paths_for_ffi_types_are_reachable() {
        let _ = EngineBuildError::DuplicatePolicyName {
            name: "dup".to_string(),
        }
        .to_string();

        let engine = openpit::Engine::<
            crate::order::Order,
            crate::execution_report::ExecutionReport,
            crate::account_adjustment::AccountAdjustment,
        >::builder()
        .check_pre_trade_start_policy(AlwaysRejectStart)
        .build()
        .expect("engine");

        let start = engine.start_pre_trade(crate::order::Order::default());
        assert!(start.is_err());

        let execute = engine.execute_pre_trade(crate::order::Order::default());
        assert!(execute.is_err());

        let report = crate::execution_report::ExecutionReport {
            operation: pit_interop::ExecutionReportOperationAccess::Absent,
            financial_impact: pit_interop::FinancialImpactAccess::Absent,
            fill: None,
            position_impact: None,
            user_data: std::ptr::null_mut(),
        };
        let post = engine.apply_execution_report(&report);
        assert!(!post.kill_switch_triggered);

        let apply = engine.apply_account_adjustment(
            openpit::param::AccountId::from_u64(1),
            &[crate::account_adjustment::AccountAdjustment {
                operation: None,
                amount: None,
                bounds: None,
                user_data: std::ptr::null_mut(),
            }],
        );
        assert!(apply.is_ok());
    }
}
