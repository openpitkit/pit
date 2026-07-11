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

use crate::account_adjustment::{
    import_account_adjustment, AccountAdjustment, OpenPitAccountAdjustment,
    OpenPitAccountAdjustmentApplyStatus,
};
use crate::account_outcome::{outcomes_to_list_owned, OpenPitAccountAdjustmentOutcomeList};
use crate::execution_report::{import_execution_report, ExecutionReport, OpenPitExecutionReport};
use crate::instrument::parse_asset_view;
use crate::last_error::{write_error, OpenPitOutError};
use crate::order::{import_order, OpenPitOrder, Order};
use crate::param::OpenPitParamAccountId;
use crate::reject::{
    blocks_to_list_owned, rejects_to_list_owned, OpenPitPretradeAccountBlockList,
    OpenPitPretradeRejectList,
};
use crate::write_error_format;
use crate::OpenPitStringView;
use openpit::param::{AccountGroupId, AccountId, DEFAULT_ACCOUNT_GROUP};
use openpit::pretrade::Rejects;

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
// handle. No-sync keeps the handle on the OS thread that created it. Account
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
    /// The synchronization mode chosen at creation time.
    ///
    /// The market-data builder reads this to inherit the correct MD mode
    /// without requiring the caller to pass it again.
    pub(crate) sync_mode: openpit_interop::SyncMode,
}

/// Opaque engine pointer.
///
/// The engine stores policies and mutable risk state. The caller owns the
/// pointer until `openpit_destroy_engine`.
pub struct OpenPitEngine {
    inner: Engine,
}

impl OpenPitEngine {
    /// Returns a handle for retuning configurable policies at runtime.
    ///
    /// Used by the `openpit_engine_configure_*` functions, which live in the
    /// policy submodules next to their corresponding builders so they can
    /// reuse those modules' barrier structs.
    pub(crate) fn configurator(&self) -> openpit::Configurator<openpit_interop::EngineLocking> {
        self.inner.configure()
    }
}

fn engine_ref<'a>(engine: *const OpenPitEngine) -> Option<&'a OpenPitEngine> {
    if engine.is_null() {
        None
    } else {
        Some(unsafe { &*engine })
    }
}

fn engine_ref_or_error<'a>(
    engine: *const OpenPitEngine,
    out_error: OpenPitOutError,
) -> Option<&'a OpenPitEngine> {
    match engine_ref(engine) {
        Some(engine) => Some(engine),
        None => {
            write_error(out_error, "engine is null");
            None
        }
    }
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

/// Opaque report pointer returned by a pre-trade dry-run.
///
/// A dry-run reports the verdict the equivalent real call would produce with
/// zero effect on engine state. Unlike `OpenPitPretradePreTradeReservation`,
/// this report is inert: it carries no commit/rollback capability. The caller
/// owns the pointer and MUST release it with
/// `openpit_destroy_pretrade_pre_trade_dry_run_report`.
pub struct OpenPitPretradePreTradeDryRunReport {
    inner: openpit::pretrade::PreTradeDryRunReport,
}

pub use crate::pre_trade_lock::OpenPitPretradePreTradeLock;

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

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Machine-readable discriminant describing why building an engine failed.
///
/// Each value identifies a distinct failure category. There is no success
/// value: a build-error object exists only when a build did not produce an
/// engine.
pub enum OpenPitEngineBuildErrorCode {
    /// Two or more registered policies declare the same name.
    DuplicatePolicyName = 0,
    /// Two or more registered policies declare the same non-default group id.
    DuplicatePolicyGroupId = 1,
    /// A failure category not covered by the above. Forward-compatible
    /// catch-all; no structured payload is available.
    Other = 2,
}

/// Structured build-failure details returned by engine construction.
///
/// Ownership:
/// - created by `openpit_engine_builder_build` when building does not produce
///   an engine;
/// - owned by the caller;
/// - released with `openpit_destroy_engine_build_error`.
pub struct OpenPitEngineBuildError {
    /// Machine-readable failure category.
    code: OpenPitEngineBuildErrorCode,
    /// Offending policy name for the duplicate-policy-name category; empty
    /// otherwise. Stored here so a view handed out by an accessor stays valid
    /// while this object is alive.
    policy_name: String,
    /// Offending policy group id for the duplicate-policy-group-id category;
    /// zero otherwise.
    policy_group_id: u16,
}

impl OpenPitEngineBuildError {
    fn new(err: openpit::EngineBuildError) -> Self {
        match err {
            openpit::EngineBuildError::DuplicatePolicyName { name } => Self {
                code: OpenPitEngineBuildErrorCode::DuplicatePolicyName,
                policy_name: name,
                policy_group_id: 0,
            },
            openpit::EngineBuildError::DuplicatePolicyGroupId { policy_group_id } => Self {
                code: OpenPitEngineBuildErrorCode::DuplicatePolicyGroupId,
                policy_name: String::new(),
                policy_group_id: policy_group_id.value(),
            },
            _ => Self {
                code: OpenPitEngineBuildErrorCode::Other,
                policy_name: String::new(),
                policy_group_id: 0,
            },
        }
    }
}

//--------------------------------------------------------------------------------------------------

/// Registers a built-in pre-trade policy on the builder via `pre_trade`. When
/// the policy is configurable, `pre_trade` records its settings cell for the
/// `openpit_engine_configure_*` entry points; non-configurable policies are
/// simply registered.
pub(crate) fn add_pre_trade_policy_to_builder(
    builder: &mut OpenPitEngineBuilder,
    policy: impl openpit::pretrade::PreTradePolicy<
            Order,
            ExecutionReport,
            AccountAdjustment,
            openpit_interop::EngineLocking,
        > + Send
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

fn export_pre_trade_lock(
    lock: &openpit::pretrade::PreTradeLock,
) -> *mut OpenPitPretradePreTradeLock {
    OpenPitPretradePreTradeLock::from_inner(lock.clone())
}

#[no_mangle]
/// Creates a new engine builder with the chosen synchronization policy.
///
/// Success:
/// - returns a non-null caller-owned builder object.
///
/// Error:
/// - returns null when `sync_policy` is not one of `OpenPitSyncPolicy_None` (0),
///   `OpenPitSyncPolicy_Full` (1), or `OpenPitSyncPolicy_Account` (2);
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
        0 => openpit_interop::SyncMode::None,
        1 => openpit_interop::SyncMode::Full,
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
    Box::into_raw(Box::new(OpenPitEngineBuilder {
        inner: Some(state),
        sync_mode: mode,
    }))
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
///   no policies were registered;
/// - for those non-domain failures, if `out_error` is not null, writes a
///   caller-owned `OpenPitSharedString` error handle that MUST be released with
///   `openpit_destroy_shared_string`; `out_build_error` is left untouched;
/// - returns null when the configuration is rejected during building (for
///   example, duplicate policy names or duplicate group ids); in that case, if
///   `out_build_error` is not null, writes a caller-owned
///   `OpenPitEngineBuildError` pointer that carries the machine-readable failure
///   code and the offending value, and MUST be released with
///   `openpit_destroy_engine_build_error`; `out_error` is left untouched for
///   this domain failure.
///
/// Ownership:
/// - on success the returned engine pointer is owned by the caller and must be
///   released with `openpit_destroy_engine`; `out_build_error` is left
///   untouched;
/// - the builder becomes consumed regardless of success and must not be reused.
pub extern "C" fn openpit_engine_builder_build(
    builder: *mut OpenPitEngineBuilder,
    out_build_error: *mut *mut OpenPitEngineBuildError,
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
            if !out_build_error.is_null() {
                unsafe {
                    *out_build_error = Box::into_raw(Box::new(OpenPitEngineBuildError::new(err)))
                };
            }
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
/// Releases a build-error object returned by engine construction.
///
/// Contract:
/// - passing null is allowed;
/// - this function always succeeds.
pub extern "C" fn openpit_destroy_engine_build_error(build_error: *mut OpenPitEngineBuildError) {
    if build_error.is_null() {
        return;
    }
    unsafe { drop(Box::from_raw(build_error)) };
}

#[no_mangle]
/// Returns the machine-readable failure category of a build error.
///
/// Contract:
/// - `build_error` must be a valid non-null pointer;
/// - this function never fails;
/// - violating the pointer contract aborts the call.
pub extern "C" fn openpit_engine_build_error_get_code(
    build_error: *const OpenPitEngineBuildError,
) -> OpenPitEngineBuildErrorCode {
    assert!(!build_error.is_null(), "build error pointer is null");
    unsafe { &*build_error }.code
}

#[no_mangle]
/// Returns a non-owning view of the offending policy name from a build error.
///
/// Contract:
/// - `build_error` must be a valid non-null pointer;
/// - the returned view points into memory owned by `build_error` and is valid
///   while `build_error` is alive; it must not be used after the build error is
///   destroyed;
/// - the view is empty unless the failure category is the duplicate-policy-name
///   category;
/// - this function never fails;
/// - violating the pointer contract aborts the call.
pub extern "C" fn openpit_engine_build_error_get_policy_name(
    build_error: *const OpenPitEngineBuildError,
) -> OpenPitStringView {
    assert!(!build_error.is_null(), "build error pointer is null");
    OpenPitStringView::from_utf8(unsafe { &*build_error }.policy_name.as_str())
}

#[no_mangle]
/// Returns the offending policy group id from a build error.
///
/// Contract:
/// - `build_error` must be a valid non-null pointer;
/// - the value is zero unless the failure category is the
///   duplicate-policy-group-id category;
/// - this function never fails;
/// - violating the pointer contract aborts the call.
pub extern "C" fn openpit_engine_build_error_get_policy_group_id(
    build_error: *const OpenPitEngineBuildError,
) -> u16 {
    assert!(!build_error.is_null(), "build error pointer is null");
    unsafe { &*build_error }.policy_group_id
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
/// - release a successful request with
///   `openpit_pretrade_pre_trade_request_execute` or
///   `openpit_destroy_pretrade_pre_trade_request`.
///
/// Output ownership contract:
/// - on `Passed`, a non-null request pointer is written to `out_request` if it
///   is not null;
/// - on `Rejected`, a non-null `OpenPitPretradeRejectList` pointer is written
///   to `out_rejects` if it is not null;
/// - the caller owns either returned object and MUST release it with the
///   corresponding destroy function;
/// - no thread-local state is involved, and returned pointers are safe to read
///   on any thread;
/// - on `Passed` and `Error`, `out_rejects` is left untouched;
/// - on `Rejected` and `Error`, `out_request` is left untouched.
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
/// Output ownership contract:
/// - on `Passed`, a non-null reservation pointer is written to
///   `out_reservation` if it is not null;
/// - on `Rejected`, a non-null `OpenPitPretradeRejectList` pointer is written to
///   `out_rejects` if it is not null;
/// - the caller owns either returned object and MUST release it with the
///   corresponding destroy function;
/// - no thread-local state is involved, and returned pointers are safe to read
///   on any thread;
/// - on `Passed` and `Error`, `out_rejects` is left untouched;
/// - on `Rejected` and `Error`, `out_reservation` is left untouched.
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
/// Runs the start stage as a non-mutating pre-trade dry-run.
///
/// A dry-run reports the verdict the start stage *would* return for `order` with
/// zero effect on engine state: no policy side effect is applied and no account
/// block is recorded, even when an account-scope reject *would* latch one. The
/// reported lock and account adjustments are always empty, because they are
/// produced by the main stage, which this call does not run; see
/// `openpit_engine_execute_pre_trade_dry_run` for a full-pipeline dry-run.
///
/// Success:
/// - returns `true` on valid input and writes a non-null caller-owned report to
///   `out_report` if it is not null; the pass/reject verdict lives inside the
///   report (a dry-run never rejects the *call*).
///
/// Error:
/// - returns `false` when input pointers are invalid or the order payload cannot
///   be decoded;
/// - on `false`, if `out_error` is not null, it is filled with a caller-owned
///   `OpenPitSharedString` that MUST be destroyed by the caller.
///
/// Cleanup:
/// - release a produced report with
///   `openpit_destroy_pretrade_pre_trade_dry_run_report`.
///
/// Output ownership contract:
/// - on success, a non-null report pointer is written to `out_report` if it is
///   not null;
/// - the caller owns the report and MUST release it with the corresponding
///   destroy function;
/// - no thread-local state is involved, and the returned pointer is safe to read
///   on any thread;
/// - on `false`, `out_report` is left untouched.
///
/// Order lifetime contract:
/// - `order` is read as a borrowed view during this call only;
/// - the operation does not retain any pointer into source memory after this
///   function returns.
pub extern "C" fn openpit_engine_start_pre_trade_dry_run(
    engine: *mut OpenPitEngine,
    order: *const OpenPitOrder,
    out_report: *mut *mut OpenPitPretradePreTradeDryRunReport,
    out_error: OpenPitOutError,
) -> bool {
    if engine.is_null() {
        write_error(out_error, "engine is null");
        return false;
    }
    if order.is_null() {
        write_error(out_error, "order is null");
        return false;
    }

    let order = match import_order(unsafe { &*order }) {
        Ok(v) => v,
        Err(e) => {
            write_error(out_error, &e);
            return false;
        }
    };

    let report = unsafe { &*engine }.inner.start_pre_trade_dry_run(order);
    if !out_report.is_null() {
        unsafe {
            *out_report = Box::into_raw(Box::new(OpenPitPretradePreTradeDryRunReport {
                inner: report,
            }))
        }
    }
    true
}

#[no_mangle]
/// Runs the full pre-trade pipeline as a non-mutating dry-run.
///
/// A dry-run reports the verdict, the lock, and the account adjustments both
/// stages *would* produce for `order` with zero effect on engine state: no
/// rate-limit budget is spent, no reservation or hold is applied, and no account
/// block is recorded. Any mutations a main-stage policy registers on the dry-run
/// path are dropped - neither committed nor rolled back - so a repeated dry-run
/// never moves engine state.
///
/// Success:
/// - returns `true` on valid input and writes a non-null caller-owned report to
///   `out_report` if it is not null; the pass/reject verdict lives inside the
///   report (a dry-run never rejects the *call*).
///
/// Error:
/// - returns `false` when input pointers are invalid or the order payload cannot
///   be decoded;
/// - on `false`, if `out_error` is not null, it is filled with a caller-owned
///   `OpenPitSharedString` that MUST be destroyed by the caller.
///
/// Cleanup:
/// - release a produced report with
///   `openpit_destroy_pretrade_pre_trade_dry_run_report`.
///
/// Output ownership contract:
/// - on success, a non-null report pointer is written to `out_report` if it is
///   not null;
/// - the caller owns the report and MUST release it with the corresponding
///   destroy function;
/// - no thread-local state is involved, and the returned pointer is safe to read
///   on any thread;
/// - on `false`, `out_report` is left untouched.
///
/// Order lifetime contract:
/// - `order` is read as a borrowed view during this call only;
/// - the operation does not retain any pointer into source memory after this
///   function returns.
pub extern "C" fn openpit_engine_execute_pre_trade_dry_run(
    engine: *mut OpenPitEngine,
    order: *const OpenPitOrder,
    out_report: *mut *mut OpenPitPretradePreTradeDryRunReport,
    out_error: OpenPitOutError,
) -> bool {
    if engine.is_null() {
        write_error(out_error, "engine is null");
        return false;
    }
    if order.is_null() {
        write_error(out_error, "order is null");
        return false;
    }

    let order = match import_order(unsafe { &*order }) {
        Ok(v) => v,
        Err(e) => {
            write_error(out_error, &e);
            return false;
        }
    };

    let report = unsafe { &*engine }.inner.execute_pre_trade_dry_run(order);
    if !out_report.is_null() {
        unsafe {
            *out_report = Box::into_raw(Box::new(OpenPitPretradePreTradeDryRunReport {
                inner: report,
            }))
        }
    }
    true
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
/// Output ownership contract:
/// - on `Passed`, a non-null reservation pointer is written to
///   `out_reservation` if it is not null;
/// - on `Rejected`, a non-null `OpenPitPretradeRejectList` pointer is written to
///   `out_rejects` if it is not null;
/// - the caller owns either returned object and MUST release it with the
///   corresponding destroy function;
/// - no thread-local state is involved, and returned pointers are safe to read
///   on any thread;
/// - on `Passed` and `Error`, `out_rejects` is left untouched;
/// - on `Rejected` and `Error`, `out_reservation` is left untouched.
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
) -> *mut OpenPitPretradePreTradeLock {
    assert!(!reservation.is_null());
    export_pre_trade_lock(unsafe { &*reservation }.inner.lock())
}

#[no_mangle]
/// Returns the account-adjustment outcomes collected by the reservation.
///
/// Contract:
/// - `reservation` must be a valid non-null pointer;
/// - violating the pointer contract aborts the call;
/// - this function never fails;
/// - always returns a caller-owned `OpenPitAccountAdjustmentOutcomeList`
///   (possibly empty); release it with
///   `openpit_destroy_account_adjustment_outcome_list`.
///
/// Lifetime contract:
/// - the returned list is detached from the reservation state.
pub extern "C" fn openpit_pretrade_pre_trade_reservation_get_account_adjustments(
    reservation: *const OpenPitPretradePreTradeReservation,
) -> *mut OpenPitAccountAdjustmentOutcomeList {
    assert!(!reservation.is_null());
    let outcomes = unsafe { &*reservation }
        .inner
        .account_adjustments()
        .to_vec();
    Box::into_raw(Box::new(outcomes_to_list_owned(outcomes)))
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
/// Returns whether the order would have passed every pre-trade stage.
///
/// Contract:
/// - `report` must be a valid non-null pointer;
/// - violating the pointer contract aborts the call;
/// - this function never fails;
/// - equivalent to "the rejects list returned by
///   `openpit_pretrade_pre_trade_dry_run_report_get_rejects` is empty".
pub extern "C" fn openpit_pretrade_pre_trade_dry_run_report_is_pass(
    report: *const OpenPitPretradePreTradeDryRunReport,
) -> bool {
    assert!(!report.is_null(), "dry-run report pointer is null");
    unsafe { &*report }.inner.is_pass()
}

#[no_mangle]
/// Returns the rejects the order would have collected.
///
/// Contract:
/// - `report` must be a valid non-null pointer;
/// - violating the pointer contract aborts the call;
/// - this function never fails;
/// - always returns a caller-owned `OpenPitPretradeRejectList` (empty when the
///   order would have passed); release it with
///   `openpit_pretrade_destroy_reject_list`.
///
/// Lifetime contract:
/// - the returned list is detached from the report state.
pub extern "C" fn openpit_pretrade_pre_trade_dry_run_report_get_rejects(
    report: *const OpenPitPretradePreTradeDryRunReport,
) -> *mut OpenPitPretradeRejectList {
    assert!(!report.is_null(), "dry-run report pointer is null");
    let rejects = match unsafe { &*report }.inner.rejects() {
        Some(rejects) => rejects.clone(),
        None => Rejects::new(Vec::new()),
    };
    Box::into_raw(Box::new(rejects_to_list_owned(rejects)))
}

#[no_mangle]
/// Returns a snapshot of the lock the main stage would have produced.
///
/// Contract:
/// - `report` must be a valid non-null pointer;
/// - violating the pointer contract aborts the call;
/// - this function never fails;
/// - the lock is empty when the start stage would have rejected (the main stage
///   never runs in that case) or when no policy locks a price; release the
///   returned handle with `openpit_destroy_pretrade_pre_trade_lock`.
///
/// Lifetime contract:
/// - the returned snapshot is detached from the report state.
pub extern "C" fn openpit_pretrade_pre_trade_dry_run_report_get_lock(
    report: *const OpenPitPretradePreTradeDryRunReport,
) -> *mut OpenPitPretradePreTradeLock {
    assert!(!report.is_null(), "dry-run report pointer is null");
    export_pre_trade_lock(unsafe { &*report }.inner.lock())
}

#[no_mangle]
/// Returns the account-adjustment outcomes the main stage would have produced.
///
/// Contract:
/// - `report` must be a valid non-null pointer;
/// - violating the pointer contract aborts the call;
/// - this function never fails;
/// - always returns a caller-owned `OpenPitAccountAdjustmentOutcomeList`
///   (possibly empty); release it with
///   `openpit_destroy_account_adjustment_outcome_list`. The numbers match what a
///   real reservation reports for the same order and engine state.
///
/// Lifetime contract:
/// - the returned list is detached from the report state.
pub extern "C" fn openpit_pretrade_pre_trade_dry_run_report_get_account_adjustments(
    report: *const OpenPitPretradePreTradeDryRunReport,
) -> *mut OpenPitAccountAdjustmentOutcomeList {
    assert!(!report.is_null(), "dry-run report pointer is null");
    let outcomes = unsafe { &*report }.inner.account_adjustments().to_vec();
    Box::into_raw(Box::new(outcomes_to_list_owned(outcomes)))
}

#[no_mangle]
/// Returns the account block an account-scope reject would have latched.
///
/// Contract:
/// - `report` must be a valid non-null pointer;
/// - violating the pointer contract aborts the call;
/// - this function never fails;
/// - always returns a caller-owned `OpenPitPretradeAccountBlockList` carrying the
///   single would-be block, or empty when no account-scope reject would have
///   latched one; release it with
///   `openpit_pretrade_destroy_account_block_list`. A real call records this
///   block in the engine's blocked-accounts registry; a dry-run reports it here
///   without recording it.
///
/// Lifetime contract:
/// - the returned list is detached from the report state.
pub extern "C" fn openpit_pretrade_pre_trade_dry_run_report_get_account_block(
    report: *const OpenPitPretradePreTradeDryRunReport,
) -> *mut OpenPitPretradeAccountBlockList {
    assert!(!report.is_null(), "dry-run report pointer is null");
    let blocks = match unsafe { &*report }.inner.account_block() {
        Some(block) => vec![block.clone()],
        None => Vec::new(),
    };
    Box::into_raw(Box::new(blocks_to_list_owned(blocks)))
}

#[no_mangle]
/// Releases a dry-run report pointer owned by the caller.
///
/// Contract:
/// - passing null is allowed;
/// - after this call the pointer is invalid;
/// - this function always succeeds.
pub extern "C" fn openpit_destroy_pretrade_pre_trade_dry_run_report(
    report: *mut OpenPitPretradePreTradeDryRunReport,
) {
    if report.is_null() {
        return;
    }
    unsafe { drop(Box::from_raw(report)) };
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
/// - when no policy blocked, `out_blocks` is left untouched;
/// - if `out_adjustments` is not null and at least one policy produced an
///   account-adjustment outcome, writes a caller-owned
///   `OpenPitAccountAdjustmentOutcomeList` pointer; release it with
///   `openpit_destroy_account_adjustment_outcome_list`;
/// - when no outcome was produced, `out_adjustments` is left untouched.
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
    out_adjustments: *mut *mut OpenPitAccountAdjustmentOutcomeList,
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

    if !out_blocks.is_null() && !result.account_blocks.is_empty() {
        let list = blocks_to_list_owned(result.account_blocks);
        unsafe { *out_blocks = Box::into_raw(Box::new(list)) };
    }

    if !out_adjustments.is_null() && !result.account_adjustments.is_empty() {
        let list = outcomes_to_list_owned(result.account_adjustments);
        unsafe { *out_adjustments = Box::into_raw(Box::new(list)) };
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
/// - on `Applied`, if `out_outcomes` is not null and at least one policy
///   produced an account-adjustment outcome, writes a caller-owned
///   `OpenPitAccountAdjustmentOutcomeList` pointer; release it with
///   `openpit_destroy_account_adjustment_outcome_list`; if no outcome was
///   produced, `out_outcomes` is left untouched;
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
    out_outcomes: *mut *mut OpenPitAccountAdjustmentOutcomeList,
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
        Ok(batch) => {
            if !out_outcomes.is_null() && !batch.outcomes.is_empty() {
                let list = outcomes_to_list_owned(batch.outcomes);
                unsafe { *out_outcomes = Box::into_raw(Box::new(list)) };
            }
            OpenPitAccountAdjustmentApplyStatus::Applied
        }
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

//--------------------------------------------------------------------------------------------------

/// Structured error returned by account-group registry operations.
///
/// Ownership:
/// - created by `openpit_engine_register_account_group` and
///   `openpit_engine_unregister_account_group` on failure;
/// - owned by the caller;
/// - released with `openpit_destroy_account_group_error`.
pub struct OpenPitAccountGroupError {
    /// Human-readable error message.
    message: String,
    /// Offending account identifier.
    account: openpit::param::AccountId,
    /// Existing group of the offending account, or `u32::MAX` when absent.
    current_group: u32,
    /// Whether `current_group` is present.
    current_group_is_set: bool,
}

impl OpenPitAccountGroupError {
    fn new(err: openpit::AccountGroupError) -> Self {
        match &err {
            openpit::AccountGroupError::AlreadyRegistered {
                account,
                current_group,
            } => Self {
                message: err.to_string(),
                account: *account,
                current_group: current_group.as_u32(),
                current_group_is_set: true,
            },
            openpit::AccountGroupError::NotInGroup {
                account,
                current_group,
                ..
            } => Self {
                message: err.to_string(),
                account: *account,
                current_group: current_group.map(|g| g.as_u32()).unwrap_or(0),
                current_group_is_set: current_group.is_some(),
            },
            _ => Self {
                message: err.to_string(),
                account: openpit::param::AccountId::from_u64(0),
                current_group: 0,
                current_group_is_set: false,
            },
        }
    }
}

#[no_mangle]
/// Releases a caller-owned account-group error.
///
/// Contract:
/// - call exactly once per pointer returned by a registry function;
/// - passing null is allowed and has no effect.
pub extern "C" fn openpit_destroy_account_group_error(err: *mut OpenPitAccountGroupError) {
    if err.is_null() {
        return;
    }
    unsafe { drop(Box::from_raw(err)) };
}

#[no_mangle]
/// Returns the human-readable error message from an account-group error.
///
/// Contract:
/// - `err` must be a valid non-null pointer;
/// - the returned view borrows from the error object and is valid while the
///   error is alive;
/// - violating the pointer contract aborts the call.
pub extern "C" fn openpit_account_group_error_get_message(
    err: *const OpenPitAccountGroupError,
) -> crate::OpenPitStringView {
    assert!(!err.is_null(), "account group error pointer is null");
    crate::OpenPitStringView::from_utf8(unsafe { &(*err).message })
}

#[no_mangle]
/// Returns the offending account identifier from an account-group error.
///
/// Contract:
/// - `err` must be a valid non-null pointer;
/// - this function never fails;
/// - violating the pointer contract aborts the call.
pub extern "C" fn openpit_account_group_error_get_account(
    err: *const OpenPitAccountGroupError,
) -> crate::param::OpenPitParamAccountId {
    assert!(!err.is_null(), "account group error pointer is null");
    unsafe { (*err).account.as_u64() }
}

#[no_mangle]
/// Returns the current group of the offending account from an account-group
/// error, or returns `false` and leaves `out_group` untouched when no group is set.
///
/// Contract:
/// - `err` must be a valid non-null pointer;
/// - `out_group` must be a valid non-null pointer;
/// - returns `true` when the account belongs to a group and writes that group
///   to `out_group`;
/// - returns `false` when the account belongs to no group; `out_group` is
///   written to only when the return value is `true`;
/// - violating the pointer contract aborts the call.
pub extern "C" fn openpit_account_group_error_get_current_group(
    err: *const OpenPitAccountGroupError,
    out_group: *mut crate::account_group_id::OpenPitParamAccountGroupId,
) -> bool {
    assert!(!err.is_null(), "account group error pointer is null");
    assert!(!out_group.is_null(), "out_group pointer is null");
    let err = unsafe { &*err };
    if err.current_group_is_set {
        unsafe { *out_group = err.current_group };
        true
    } else {
        false
    }
}

//--------------------------------------------------------------------------------------------------

#[no_mangle]
/// Atomically registers every account in `accounts` into `group`.
///
/// The operation is all-or-nothing: if any listed account is already a member
/// of any group (including `group`), no account is registered.
///
/// Contract:
/// - `engine` must be a valid non-null engine pointer;
/// - `accounts` must point to an array of at least `accounts_len` account
///   identifiers, or may be null when `accounts_len` is zero;
/// - `group` is the target group and must not be the reserved
///   `OPENPIT_DEFAULT_ACCOUNT_GROUP`.
///
/// Success:
/// - returns `true`; all listed accounts are now members of `group`.
///
/// Error:
/// - returns `false` when `engine` is null, `accounts` is null with non-zero
///   length, `group` is the reserved default group, or any listed account is
///   already registered;
/// - for pointer/argument errors, if `out_error` is not null, writes a
///   caller-owned `OpenPitSharedString` error handle that MUST be released
///   with `openpit_destroy_shared_string`;
/// - for domain errors (reserved target group, or account already
///   registered), if `out_group_error` is not null, writes a caller-owned
///   `OpenPitAccountGroupError` pointer that MUST be released with
///   `openpit_destroy_account_group_error`; `out_error` is left untouched for
///   domain failures.
pub extern "C" fn openpit_engine_register_account_group(
    engine: *mut OpenPitEngine,
    accounts: *const crate::param::OpenPitParamAccountId,
    accounts_len: usize,
    group: crate::account_group_id::OpenPitParamAccountGroupId,
    out_group_error: *mut *mut OpenPitAccountGroupError,
    out_error: OpenPitOutError,
) -> bool {
    if engine.is_null() {
        write_error(out_error, "engine is null");
        return false;
    }
    if accounts_len > 0 && accounts.is_null() {
        write_error(out_error, "accounts is null");
        return false;
    }
    let account_ids: Vec<openpit::param::AccountId> = if accounts_len == 0 {
        vec![]
    } else {
        unsafe { std::slice::from_raw_parts(accounts, accounts_len) }
            .iter()
            .map(|&id| openpit::param::AccountId::from_u64(id))
            .collect()
    };
    let group = match openpit::param::AccountGroupId::from_u32(group) {
        Ok(group) => group,
        Err(_) => {
            if !out_group_error.is_null() {
                unsafe {
                    *out_group_error = Box::into_raw(Box::new(OpenPitAccountGroupError::new(
                        openpit::AccountGroupError::ReservedGroup,
                    )))
                };
            }
            return false;
        }
    };
    match unsafe { &*engine }
        .inner
        .accounts()
        .register_group(&account_ids, group)
    {
        Ok(()) => true,
        Err(err) => {
            if !out_group_error.is_null() {
                unsafe {
                    *out_group_error = Box::into_raw(Box::new(OpenPitAccountGroupError::new(err)))
                };
            }
            false
        }
    }
}

#[no_mangle]
/// Atomically removes every account in `accounts` from `group`.
///
/// The operation is all-or-nothing: if any listed account is not currently a
/// member of `group`, no account is removed.
///
/// Contract:
/// - `engine` must be a valid non-null engine pointer;
/// - `accounts` must point to an array of at least `accounts_len` account
///   identifiers, or may be null when `accounts_len` is zero;
/// - `group` is the group to remove accounts from and must not be the reserved
///   `OPENPIT_DEFAULT_ACCOUNT_GROUP`.
///
/// Success:
/// - returns `true`; all listed accounts are now removed from `group`.
///
/// Error:
/// - returns `false` when `engine` is null, `accounts` is null with non-zero
///   length, `group` is the reserved default group, or any listed account is
///   not in `group`;
/// - for pointer/argument errors, if `out_error` is not null, writes a
///   caller-owned `OpenPitSharedString` error handle that MUST be released
///   with `openpit_destroy_shared_string`;
/// - for domain errors (reserved target group, or account not in group), if
///   `out_group_error` is not null, writes a caller-owned
///   `OpenPitAccountGroupError` pointer that MUST be released with
///   `openpit_destroy_account_group_error`; `out_error` is left untouched for
///   domain failures.
pub extern "C" fn openpit_engine_unregister_account_group(
    engine: *mut OpenPitEngine,
    accounts: *const crate::param::OpenPitParamAccountId,
    accounts_len: usize,
    group: crate::account_group_id::OpenPitParamAccountGroupId,
    out_group_error: *mut *mut OpenPitAccountGroupError,
    out_error: OpenPitOutError,
) -> bool {
    if engine.is_null() {
        write_error(out_error, "engine is null");
        return false;
    }
    if accounts_len > 0 && accounts.is_null() {
        write_error(out_error, "accounts is null");
        return false;
    }
    let account_ids: Vec<openpit::param::AccountId> = if accounts_len == 0 {
        vec![]
    } else {
        unsafe { std::slice::from_raw_parts(accounts, accounts_len) }
            .iter()
            .map(|&id| openpit::param::AccountId::from_u64(id))
            .collect()
    };
    let group = match openpit::param::AccountGroupId::from_u32(group) {
        Ok(group) => group,
        Err(_) => {
            if !out_group_error.is_null() {
                unsafe {
                    *out_group_error = Box::into_raw(Box::new(OpenPitAccountGroupError::new(
                        openpit::AccountGroupError::ReservedGroup,
                    )))
                };
            }
            return false;
        }
    };
    match unsafe { &*engine }
        .inner
        .accounts()
        .unregister_group(&account_ids, group)
    {
        Ok(()) => true,
        Err(err) => {
            if !out_group_error.is_null() {
                unsafe {
                    *out_group_error = Box::into_raw(Box::new(OpenPitAccountGroupError::new(err)))
                };
            }
            false
        }
    }
}

#[no_mangle]
/// Returns the account-group membership of a single account.
///
/// Contract:
/// - `engine` must be a valid non-null engine pointer;
/// - `account` is the account identifier to look up;
/// - `out_group` must be a valid non-null pointer.
///
/// Success:
/// - returns `true` when the account belongs to a group and writes that group
///   identifier to `out_group`;
/// - returns `false` when the account belongs to no group; `out_group` is not
///   written to when the return value is `false`.
///
/// Error:
/// - aborts the call when `engine` or `out_group` is null.
pub extern "C" fn openpit_engine_account_group(
    engine: *const OpenPitEngine,
    account: crate::param::OpenPitParamAccountId,
    out_group: *mut crate::account_group_id::OpenPitParamAccountGroupId,
) -> bool {
    assert!(!engine.is_null(), "engine is null");
    assert!(!out_group.is_null(), "out_group is null");
    let engine = unsafe { &*engine };
    let account = openpit::param::AccountId::from_u64(account);
    match engine.inner.accounts().group_of(account) {
        Some(group) => {
            unsafe { *out_group = group.as_u32() };
            true
        }
        None => false,
    }
}

//--------------------------------------------------------------------------------------------------

// Maps a raw u32 group id to `AccountGroupId`, allowing the reserved
// `DEFAULT_ACCOUNT_GROUP` (0). Any non-zero u32 is a valid group id, so
// `AccountGroupId::from_u32` cannot fail for non-zero values; the `Err`
// branch exists only for forward-compatibility should the reserved set widen.
fn import_account_group_id_allow_default(group: u32) -> Result<AccountGroupId, ()> {
    if group == DEFAULT_ACCOUNT_GROUP.as_u32() {
        return Ok(DEFAULT_ACCOUNT_GROUP);
    }
    AccountGroupId::from_u32(group).map_err(|_| ())
}

#[no_mangle]
/// Sets the explicit currency of `account`.
///
/// Setting or changing the account currency does not validate existing
/// holdings and does not recompute stored average entry price or realized PnL.
/// The caller owns the risk of changing currency on live state; a control or
/// recompute API may be added later.
///
/// Contract:
/// - passing null for `engine` returns `false` and writes `out_error`;
/// - `asset` is parsed as an asset code and must be present;
/// - on parse failure, if `out_error` is not null, writes a caller-owned
///   `OpenPitSharedString` error handle that MUST be released with
///   `openpit_destroy_shared_string`.
pub extern "C" fn openpit_engine_set_account_currency(
    engine: *mut OpenPitEngine,
    account_id: crate::param::OpenPitParamAccountId,
    asset: crate::OpenPitStringView,
    out_error: OpenPitOutError,
) -> bool {
    let Some(engine) = engine_ref_or_error(engine, out_error) else {
        return false;
    };
    let asset = match parse_asset_view(asset, "asset") {
        Ok(Some(asset)) => asset,
        Ok(None) => {
            write_error(out_error, "asset is required");
            return false;
        }
        Err(err) => {
            write_error(out_error, &err);
            return false;
        }
    };
    let account = openpit::param::AccountId::from_u64(account_id);
    engine.inner.accounts().set_currency(account, asset);
    true
}

#[no_mangle]
/// Clears the explicit currency of `account`.
///
/// Clearing the account currency does not validate existing holdings and does
/// not recompute stored average entry price or realized PnL. The caller owns
/// the risk of changing currency on live state; a control or recompute API may
/// be added later.
///
/// Contract:
/// - `engine` must be a valid non-null engine pointer.
pub extern "C" fn openpit_engine_clear_account_currency(
    engine: *mut OpenPitEngine,
    account_id: crate::param::OpenPitParamAccountId,
) {
    assert!(!engine.is_null(), "engine is null");
    let engine = unsafe { &*engine };
    let account = openpit::param::AccountId::from_u64(account_id);
    engine.inner.accounts().clear_currency(account);
}

#[no_mangle]
/// Sets the currency inherited by accounts in `group`.
///
/// `OPENPIT_DEFAULT_ACCOUNT_GROUP` (value `0`) is allowed and represents the
/// global default tier.
///
/// Setting or changing the group currency does not validate existing holdings
/// and does not recompute stored average entry price or realized PnL. The
/// caller owns the risk of changing currency on live state; a control or
/// recompute API may be added later.
///
/// Contract:
/// - passing null for `engine` returns `false` and writes `out_error`;
/// - `group` must be `OPENPIT_DEFAULT_ACCOUNT_GROUP` or a valid non-reserved
///   group id; an invalid (reserved) id returns `false` and writes
///   `out_error`;
/// - `asset` is parsed as an asset code and must be present;
/// - on any failure, if `out_error` is not null, writes a caller-owned
///   `OpenPitSharedString` error handle that MUST be released with
///   `openpit_destroy_shared_string`.
pub extern "C" fn openpit_engine_set_account_group_currency(
    engine: *mut OpenPitEngine,
    group: crate::account_group_id::OpenPitParamAccountGroupId,
    asset: crate::OpenPitStringView,
    out_error: OpenPitOutError,
) -> bool {
    let Some(engine) = engine_ref_or_error(engine, out_error) else {
        return false;
    };
    let group = match import_account_group_id_allow_default(group) {
        Ok(g) => g,
        Err(()) => {
            write_error(out_error, "group is reserved");
            return false;
        }
    };
    let asset = match parse_asset_view(asset, "asset") {
        Ok(Some(asset)) => asset,
        Ok(None) => {
            write_error(out_error, "asset is required");
            return false;
        }
        Err(err) => {
            write_error(out_error, &err);
            return false;
        }
    };
    engine.inner.accounts().set_group_currency(group, asset);
    true
}

#[no_mangle]
/// Clears the currency inherited by accounts in `group`.
///
/// `OPENPIT_DEFAULT_ACCOUNT_GROUP` (value `0`) is allowed and represents the
/// global default tier.
///
/// Clearing the group currency does not validate existing holdings and does
/// not recompute stored average entry price or realized PnL. The caller owns
/// the risk of changing currency on live state; a control or recompute API may
/// be added later.
///
/// Contract:
/// - `engine` must be a valid non-null engine pointer;
/// - `group` must be `OPENPIT_DEFAULT_ACCOUNT_GROUP` or a valid non-reserved
///   group id; an invalid (reserved) id is silently ignored (no-op) because
///   this function has no error-output channel; prefer
///   `openpit_engine_set_account_group_currency` when error reporting is
///   needed.
pub extern "C" fn openpit_engine_clear_account_group_currency(
    engine: *mut OpenPitEngine,
    group: crate::account_group_id::OpenPitParamAccountGroupId,
) {
    assert!(!engine.is_null(), "engine is null");
    let engine = unsafe { &*engine };
    let Ok(group) = import_account_group_id_allow_default(group) else {
        return;
    };
    engine.inner.accounts().clear_group_currency(group);
}

//--------------------------------------------------------------------------------------------------

/// Structured error returned by account block operations.
///
/// Ownership:
/// - created by `openpit_engine_replace_account_block_reason`,
///   `openpit_engine_block_account_group`,
///   `openpit_engine_unblock_account_group`, and
///   `openpit_engine_replace_account_group_block_reason` on failure;
/// - owned by the caller;
/// - released with `openpit_destroy_account_block_error`.
pub struct OpenPitAccountBlockError {
    /// Human-readable error message.
    message: String,
    /// Offending account identifier; meaningful only when `account_is_set` is
    /// true; stored as `0` when absent.
    account: u64,
    /// Whether `account` is present.
    account_is_set: bool,
    /// Offending account-group identifier; meaningful only when `group_is_set`
    /// is true; stored as `0` when absent.
    group: u32,
    /// Whether `group` is present.
    group_is_set: bool,
}

impl OpenPitAccountBlockError {
    fn new(err: openpit::AccountBlockError) -> Self {
        match &err {
            openpit::AccountBlockError::AccountNotBlocked { account } => Self {
                message: err.to_string(),
                account: account.as_u64(),
                account_is_set: true,
                group: 0,
                group_is_set: false,
            },
            openpit::AccountBlockError::GroupNotBlocked { group } => Self {
                message: err.to_string(),
                account: 0,
                account_is_set: false,
                group: group.as_u32(),
                group_is_set: true,
            },
            _ => Self {
                message: err.to_string(),
                account: 0,
                account_is_set: false,
                group: 0,
                group_is_set: false,
            },
        }
    }

    fn message(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            account: 0,
            account_is_set: false,
            group: 0,
            group_is_set: false,
        }
    }
}

fn write_account_block_error(
    out_error: *mut *mut OpenPitAccountBlockError,
    err: OpenPitAccountBlockError,
) {
    if !out_error.is_null() {
        unsafe { *out_error = Box::into_raw(Box::new(err)) };
    }
}

/// Discriminant for the variant carried by an [`OpenPitAccountBlockError`].
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenPitAccountBlockErrorKind {
    /// The target group is the reserved default account group.
    ReservedGroup = 0,
    /// The target account is not currently blocked.
    AccountNotBlocked = 1,
    /// The target account group is not currently blocked.
    GroupNotBlocked = 2,
}

#[no_mangle]
/// Releases a caller-owned account-block error.
///
/// Contract:
/// - call exactly once per pointer returned by a block function;
/// - passing null is allowed and has no effect.
pub extern "C" fn openpit_destroy_account_block_error(err: *mut OpenPitAccountBlockError) {
    if err.is_null() {
        return;
    }
    unsafe { drop(Box::from_raw(err)) };
}

#[no_mangle]
/// Returns the human-readable error message from an account-block error.
///
/// Contract:
/// - `err` must be a valid non-null pointer;
/// - the returned view borrows from the error object and is valid while the
///   error is alive;
/// - violating the pointer contract aborts the call.
pub extern "C" fn openpit_account_block_error_get_message(
    err: *const OpenPitAccountBlockError,
) -> crate::OpenPitStringView {
    assert!(!err.is_null(), "account block error pointer is null");
    crate::OpenPitStringView::from_utf8(unsafe { &(*err).message })
}

#[no_mangle]
/// Returns the variant kind of an account-block error.
///
/// Contract:
/// - `err` must be a valid non-null pointer;
/// - this function never fails;
/// - violating the pointer contract aborts the call.
pub extern "C" fn openpit_account_block_error_get_kind(
    err: *const OpenPitAccountBlockError,
) -> OpenPitAccountBlockErrorKind {
    assert!(!err.is_null(), "account block error pointer is null");
    let err = unsafe { &*err };
    if err.account_is_set {
        OpenPitAccountBlockErrorKind::AccountNotBlocked
    } else if err.group_is_set {
        OpenPitAccountBlockErrorKind::GroupNotBlocked
    } else {
        OpenPitAccountBlockErrorKind::ReservedGroup
    }
}

#[no_mangle]
/// Returns the offending account identifier from an account-block error.
///
/// Contract:
/// - `err` must be a valid non-null pointer;
/// - `out_account` must be a valid non-null pointer;
/// - returns `true` when the error variant carries an account and writes it to
///   `out_account`;
/// - returns `false` when no account is present; `out_account` is left
///   untouched when the return value is `false`;
/// - violating the pointer contract aborts the call.
pub extern "C" fn openpit_account_block_error_get_account(
    err: *const OpenPitAccountBlockError,
    out_account: *mut crate::param::OpenPitParamAccountId,
) -> bool {
    assert!(!err.is_null(), "account block error pointer is null");
    assert!(!out_account.is_null(), "out_account pointer is null");
    let err = unsafe { &*err };
    if err.account_is_set {
        unsafe { *out_account = err.account };
        true
    } else {
        false
    }
}

#[no_mangle]
/// Returns the offending account-group identifier from an account-block error.
///
/// Contract:
/// - `err` must be a valid non-null pointer;
/// - `out_group` must be a valid non-null pointer;
/// - returns `true` when the error variant carries a group and writes it to
///   `out_group`;
/// - returns `false` when no group is present; `out_group` is left untouched
///   when the return value is `false`;
/// - violating the pointer contract aborts the call.
pub extern "C" fn openpit_account_block_error_get_group(
    err: *const OpenPitAccountBlockError,
    out_group: *mut crate::account_group_id::OpenPitParamAccountGroupId,
) -> bool {
    assert!(!err.is_null(), "account block error pointer is null");
    assert!(!out_group.is_null(), "out_group pointer is null");
    let err = unsafe { &*err };
    if err.group_is_set {
        unsafe { *out_group = err.group };
        true
    } else {
        false
    }
}

//--------------------------------------------------------------------------------------------------

/// Discriminant for the variant carried by an [`OpenPitConfigureError`].
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenPitConfigureErrorKind {
    /// No configurable policy carries the requested name.
    Unknown = 0,
    /// A policy is registered under the name, but its settings type differs
    /// from the one the called configure function targets.
    TypeMismatch = 1,
    /// The applied update was rejected by the policy's settings validation; the
    /// prior configuration still applies.
    Validation = 2,
    /// A configuration call re-entered configuration on the same thread.
    NestedConfiguration = 3,
}

/// Structured error returned by runtime policy reconfiguration.
///
/// Ownership:
/// - created by the `openpit_engine_configure_*` functions on failure;
/// - owned by the caller;
/// - released with `openpit_destroy_configure_error`.
pub struct OpenPitConfigureError {
    /// Human-readable error message rendered from the core
    /// [`openpit::ConfigureError`].
    message: String,
    /// Machine-readable failure category.
    kind: OpenPitConfigureErrorKind,
}

impl OpenPitConfigureError {
    pub(crate) fn new(err: openpit::ConfigureError) -> Self {
        let kind = match &err {
            openpit::ConfigureError::UnknownPolicy { .. } => OpenPitConfigureErrorKind::Unknown,
            openpit::ConfigureError::PolicyTypeMismatch { .. } => {
                OpenPitConfigureErrorKind::TypeMismatch
            }
            openpit::ConfigureError::Validation { .. } => OpenPitConfigureErrorKind::Validation,
            openpit::ConfigureError::NestedConfiguration => {
                OpenPitConfigureErrorKind::NestedConfiguration
            }
            // `ConfigureError` is #[non_exhaustive]; treat any future variant as
            // a generic validation failure so the ABI stays forward-compatible.
            _ => OpenPitConfigureErrorKind::Validation,
        };
        Self {
            message: err.to_string(),
            kind,
        }
    }

    /// Builds an error for a caller-supplied argument that could not be parsed
    /// into the value the configure closure needs (for example an invalid
    /// asset code). These never reach the core configurator, so they are
    /// reported with the [`Validation`](OpenPitConfigureErrorKind::Validation)
    /// kind: nothing in the live configuration was changed.
    pub(crate) fn validation(message: String) -> Self {
        Self {
            message,
            kind: OpenPitConfigureErrorKind::Validation,
        }
    }
}

/// Writes a boxed [`OpenPitConfigureError`] through `out_error` iff
/// `out_error` is non-null. Shared by the `openpit_engine_configure_*`
/// functions in the policy submodules. Nothing is written on function entry
/// or on the success path: a successful call leaves `out_error` untouched.
pub(crate) fn write_configure_error(
    out_error: *mut *mut OpenPitConfigureError,
    err: OpenPitConfigureError,
) {
    if !out_error.is_null() {
        unsafe { *out_error = Box::into_raw(Box::new(err)) };
    }
}

#[no_mangle]
/// Releases a caller-owned configure error.
///
/// Contract:
/// - call exactly once per pointer returned by an `openpit_engine_configure_*`
///   function;
/// - passing null is allowed and has no effect.
pub extern "C" fn openpit_destroy_configure_error(err: *mut OpenPitConfigureError) {
    if err.is_null() {
        return;
    }
    unsafe { drop(Box::from_raw(err)) };
}

#[no_mangle]
/// Returns the human-readable error message from a configure error.
///
/// Contract:
/// - `err` must be a valid non-null pointer;
/// - the returned view borrows from the error object and is valid while the
///   error is alive;
/// - violating the pointer contract aborts the call.
pub extern "C" fn openpit_configure_error_get_message(
    err: *const OpenPitConfigureError,
) -> crate::OpenPitStringView {
    assert!(!err.is_null(), "configure error pointer is null");
    crate::OpenPitStringView::from_utf8(unsafe { &(*err).message })
}

#[no_mangle]
/// Returns the variant kind of a configure error.
///
/// Contract:
/// - `err` must be a valid non-null pointer;
/// - this function never fails;
/// - violating the pointer contract aborts the call.
pub extern "C" fn openpit_configure_error_get_kind(
    err: *const OpenPitConfigureError,
) -> OpenPitConfigureErrorKind {
    assert!(!err.is_null(), "configure error pointer is null");
    unsafe { &*err }.kind
}

//--------------------------------------------------------------------------------------------------

#[no_mangle]
/// Blocks `account` with `reason`.
///
/// The first cause for an account wins: if the account is already blocked (by
/// an admin call or a prior kill-switch), this call is a no-op and does not
/// overwrite the stored reason. Use
/// `openpit_engine_replace_account_block_reason` to change the stored reason.
///
/// Contract:
/// - `engine` must be a valid non-null engine pointer;
/// - `reason` is interpreted as UTF-8; an empty string is used when
///   `reason.ptr` is null OR `reason.len` is zero; passing a null `ptr` with
///   a non-zero `len` is caller misuse and is treated as empty (not read);
///   an empty reason is explicitly allowed.
pub extern "C" fn openpit_engine_block_account(
    engine: *mut OpenPitEngine,
    account_id: crate::param::OpenPitParamAccountId,
    reason: crate::OpenPitStringView,
) {
    assert!(!engine.is_null(), "engine is null");
    let engine = unsafe { &*engine };
    let account = openpit::param::AccountId::from_u64(account_id);
    let reason_bytes: &[u8] = if reason.ptr.is_null() || reason.len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(reason.ptr, reason.len) }
    };
    let reason_str = String::from_utf8_lossy(reason_bytes).into_owned();
    engine.inner.accounts().block(account, reason_str);
}

#[no_mangle]
/// Unblocks `account`, clearing any block on it.
///
/// Idempotent: a no-op when `account` is not blocked. Both admin blocks and
/// kill-switch blocks are cleared.
///
/// Contract:
/// - `engine` must be a valid non-null engine pointer.
pub extern "C" fn openpit_engine_unblock_account(
    engine: *mut OpenPitEngine,
    account_id: crate::param::OpenPitParamAccountId,
) {
    assert!(!engine.is_null(), "engine is null");
    let engine = unsafe { &*engine };
    let account = openpit::param::AccountId::from_u64(account_id);
    engine.inner.accounts().unblock(account);
}

#[no_mangle]
/// Replaces the stored reason of an already-blocked account.
///
/// Unlike `openpit_engine_block_account`, which preserves the first cause, this
/// overwrites the stored cause with `reason`, leaving the account blocked.
///
/// Contract:
/// - `engine` must be a valid non-null engine pointer;
/// - `reason` is interpreted as UTF-8; an empty string is used when
///   `reason.ptr` is null OR `reason.len` is zero; passing a null `ptr` with
///   a non-zero `len` is caller misuse and is treated as empty (not read);
///   an empty reason is explicitly allowed;
/// - on failure, if `out_error` is not null, writes a caller-owned
///   `OpenPitAccountBlockError` pointer that MUST be released with
///   `openpit_destroy_account_block_error`;
/// - returns `false` and writes `out_error` when `engine` is null.
///
/// Success:
/// - returns `true`; the stored reason has been replaced.
///
/// Error:
/// - returns `false` with `OpenPitAccountBlockErrorKind_AccountNotBlocked`
///   when `account` is not currently blocked.
pub extern "C" fn openpit_engine_replace_account_block_reason(
    engine: *mut OpenPitEngine,
    account_id: crate::param::OpenPitParamAccountId,
    reason: crate::OpenPitStringView,
    out_error: *mut *mut OpenPitAccountBlockError,
) -> bool {
    if engine.is_null() {
        write_account_block_error(
            out_error,
            OpenPitAccountBlockError::message("engine is null"),
        );
        return false;
    }
    let engine = unsafe { &*engine };
    let account = openpit::param::AccountId::from_u64(account_id);
    let reason_bytes: &[u8] = if reason.ptr.is_null() || reason.len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(reason.ptr, reason.len) }
    };
    let reason_str = String::from_utf8_lossy(reason_bytes).into_owned();
    match engine
        .inner
        .accounts()
        .replace_block_reason(account, reason_str)
    {
        Ok(()) => true,
        Err(err) => {
            write_account_block_error(out_error, OpenPitAccountBlockError::new(err));
            false
        }
    }
}

#[no_mangle]
/// Blocks the account group `group` with `reason`.
///
/// The first cause for a group wins: re-blocking an already-blocked group is a
/// no-op. Use `openpit_engine_replace_account_group_block_reason` to change the
/// stored reason.
///
/// Contract:
/// - `engine` must be a valid non-null engine pointer;
/// - `group` must not be `OPENPIT_DEFAULT_ACCOUNT_GROUP`;
/// - `reason` is interpreted as UTF-8; an empty string is used when
///   `reason.ptr` is null OR `reason.len` is zero; passing a null `ptr` with
///   a non-zero `len` is caller misuse and is treated as empty (not read);
///   an empty reason is explicitly allowed;
/// - on failure, if `out_error` is not null, writes a caller-owned
///   `OpenPitAccountBlockError` pointer that MUST be released with
///   `openpit_destroy_account_block_error`;
/// - returns `false` and writes `out_error` when `engine` is null.
///
/// Success:
/// - returns `true`; the group is now blocked.
///
/// Error:
/// - returns `false` with `OpenPitAccountBlockErrorKind_ReservedGroup` when
///   `group` is the reserved default group.
pub extern "C" fn openpit_engine_block_account_group(
    engine: *mut OpenPitEngine,
    group: crate::account_group_id::OpenPitParamAccountGroupId,
    reason: crate::OpenPitStringView,
    out_error: *mut *mut OpenPitAccountBlockError,
) -> bool {
    if engine.is_null() {
        write_account_block_error(
            out_error,
            OpenPitAccountBlockError::message("engine is null"),
        );
        return false;
    }
    let engine = unsafe { &*engine };
    let group = match openpit::param::AccountGroupId::from_u32(group) {
        Ok(group) => group,
        Err(_) => {
            write_account_block_error(
                out_error,
                OpenPitAccountBlockError::new(openpit::AccountBlockError::ReservedGroup),
            );
            return false;
        }
    };
    let reason_bytes: &[u8] = if reason.ptr.is_null() || reason.len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(reason.ptr, reason.len) }
    };
    let reason_str = String::from_utf8_lossy(reason_bytes).into_owned();
    match engine.inner.accounts().block_group(group, reason_str) {
        Ok(()) => true,
        Err(err) => {
            write_account_block_error(out_error, OpenPitAccountBlockError::new(err));
            false
        }
    }
}

#[no_mangle]
/// Unblocks the account group `group`, clearing the group block.
///
/// Idempotent: a no-op when `group` is not blocked. Accounts blocked
/// individually remain blocked.
///
/// Contract:
/// - `engine` must be a valid non-null engine pointer;
/// - `group` must not be `OPENPIT_DEFAULT_ACCOUNT_GROUP`;
/// - on failure, if `out_error` is not null, writes a caller-owned
///   `OpenPitAccountBlockError` pointer that MUST be released with
///   `openpit_destroy_account_block_error`;
/// - returns `false` and writes `out_error` when `engine` is null.
///
/// Success:
/// - returns `true`; the group is now unblocked.
///
/// Error:
/// - returns `false` with `OpenPitAccountBlockErrorKind_ReservedGroup` when
///   `group` is the reserved default group.
pub extern "C" fn openpit_engine_unblock_account_group(
    engine: *mut OpenPitEngine,
    group: crate::account_group_id::OpenPitParamAccountGroupId,
    out_error: *mut *mut OpenPitAccountBlockError,
) -> bool {
    if engine.is_null() {
        write_account_block_error(
            out_error,
            OpenPitAccountBlockError::message("engine is null"),
        );
        return false;
    }
    let engine = unsafe { &*engine };
    let group = match openpit::param::AccountGroupId::from_u32(group) {
        Ok(group) => group,
        Err(_) => {
            write_account_block_error(
                out_error,
                OpenPitAccountBlockError::new(openpit::AccountBlockError::ReservedGroup),
            );
            return false;
        }
    };
    match engine.inner.accounts().unblock_group(group) {
        Ok(()) => true,
        Err(err) => {
            write_account_block_error(out_error, OpenPitAccountBlockError::new(err));
            false
        }
    }
}

#[no_mangle]
/// Replaces the stored reason of an already-blocked account group.
///
/// Unlike `openpit_engine_block_account_group`, which preserves the first
/// cause, this overwrites the stored cause with `reason`, leaving the group
/// blocked.
///
/// Contract:
/// - `engine` must be a valid non-null engine pointer;
/// - `group` must not be `OPENPIT_DEFAULT_ACCOUNT_GROUP`;
/// - `reason` is interpreted as UTF-8; an empty string is used when
///   `reason.ptr` is null OR `reason.len` is zero; passing a null `ptr` with
///   a non-zero `len` is caller misuse and is treated as empty (not read);
///   an empty reason is explicitly allowed;
/// - on failure, if `out_error` is not null, writes a caller-owned
///   `OpenPitAccountBlockError` pointer that MUST be released with
///   `openpit_destroy_account_block_error`;
/// - returns `false` and writes `out_error` when `engine` is null.
///
/// Success:
/// - returns `true`; the stored group-block reason has been replaced.
///
/// Error:
/// - returns `false` with `OpenPitAccountBlockErrorKind_ReservedGroup` when
///   `group` is the reserved default group;
/// - returns `false` with `OpenPitAccountBlockErrorKind_GroupNotBlocked` when
///   `group` is not currently blocked.
pub extern "C" fn openpit_engine_replace_account_group_block_reason(
    engine: *mut OpenPitEngine,
    group: crate::account_group_id::OpenPitParamAccountGroupId,
    reason: crate::OpenPitStringView,
    out_error: *mut *mut OpenPitAccountBlockError,
) -> bool {
    if engine.is_null() {
        write_account_block_error(
            out_error,
            OpenPitAccountBlockError::message("engine is null"),
        );
        return false;
    }
    let engine = unsafe { &*engine };
    let group = match openpit::param::AccountGroupId::from_u32(group) {
        Ok(group) => group,
        Err(_) => {
            write_account_block_error(
                out_error,
                OpenPitAccountBlockError::new(openpit::AccountBlockError::ReservedGroup),
            );
            return false;
        }
    };
    let reason_bytes: &[u8] = if reason.ptr.is_null() || reason.len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(reason.ptr, reason.len) }
    };
    let reason_str = String::from_utf8_lossy(reason_bytes).into_owned();
    match engine
        .inner
        .accounts()
        .replace_group_block_reason(group, reason_str)
    {
        Ok(()) => true,
        Err(err) => {
            write_account_block_error(out_error, OpenPitAccountBlockError::new(err));
            false
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
        openpit_destroy_engine_build_error, openpit_destroy_engine_builder,
        openpit_destroy_pretrade_pre_trade_dry_run_report,
        openpit_destroy_pretrade_pre_trade_request, openpit_destroy_pretrade_pre_trade_reservation,
        openpit_engine_apply_account_adjustment, openpit_engine_apply_execution_report,
        openpit_engine_block_account, openpit_engine_build_error_get_code,
        openpit_engine_build_error_get_policy_group_id, openpit_engine_build_error_get_policy_name,
        openpit_engine_builder_build, openpit_engine_execute_pre_trade,
        openpit_engine_execute_pre_trade_dry_run, openpit_engine_start_pre_trade,
        openpit_engine_start_pre_trade_dry_run,
        openpit_pretrade_pre_trade_dry_run_report_get_account_adjustments,
        openpit_pretrade_pre_trade_dry_run_report_get_account_block,
        openpit_pretrade_pre_trade_dry_run_report_get_lock,
        openpit_pretrade_pre_trade_dry_run_report_get_rejects,
        openpit_pretrade_pre_trade_dry_run_report_is_pass,
        openpit_pretrade_pre_trade_request_execute, openpit_pretrade_pre_trade_reservation_commit,
        openpit_pretrade_pre_trade_reservation_get_lock,
        openpit_pretrade_pre_trade_reservation_rollback, OpenPitAccountAdjustmentBatchError,
        OpenPitEngineBuildError, OpenPitEngineBuildErrorCode,
    };

    struct AlwaysRejectStart;

    impl
        PreTradePolicy<
            crate::order::Order,
            crate::execution_report::ExecutionReport,
            crate::account_adjustment::AccountAdjustment,
            openpit_interop::EngineLocking,
        > for AlwaysRejectStart
    {
        fn name(&self) -> &str {
            "always.reject.start"
        }

        fn check_pre_trade_start(
            &self,
            _ctx: &openpit::pretrade::PreTradeContext<openpit_interop::StorageLockingPolicyFactory>,
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
    }

    unsafe extern "C" fn always_reject_apply(
        _ctx: *const crate::policy::OpenPitAccountAdjustmentContext,
        _account_id: crate::param::OpenPitParamAccountId,
        _adjustment: *const OpenPitAccountAdjustment,
        _mutations: *mut crate::policy::OpenPitMutations,
        _out_outcomes: *mut crate::account_outcome::OpenPitAccountOutcomeEntryList,
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
        _out_result: *mut crate::account_outcome::OpenPitPretradePreTradeResult,
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
        _ctx: *const crate::policy::custom::OpenPitPostTradeContext,
        _report: *const crate::execution_report::OpenPitExecutionReport,
        _out_adjustments: *mut crate::account_outcome::OpenPitPostTradeAdjustmentList,
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
                0,
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
        let engine =
            openpit_engine_builder_build(builder, std::ptr::null_mut(), std::ptr::null_mut());
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
                0,
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
        let engine =
            openpit_engine_builder_build(builder, std::ptr::null_mut(), std::ptr::null_mut());
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
        let engine =
            openpit_engine_builder_build(builder, std::ptr::null_mut(), std::ptr::null_mut());
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
        let engine =
            openpit_engine_builder_build(builder, std::ptr::null_mut(), std::ptr::null_mut());
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
        let engine =
            openpit_engine_builder_build(builder, std::ptr::null_mut(), std::ptr::null_mut());
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

    fn order_for_account(account_id: crate::param::OpenPitParamAccountId) -> OpenPitOrder {
        OpenPitOrder {
            operation: crate::order::OpenPitOrderOperationOptional {
                is_set: true,
                value: crate::order::OpenPitOrderOperation {
                    account_id: crate::param::OpenPitParamAccountIdOptional {
                        is_set: true,
                        value: account_id,
                    },
                    ..Default::default()
                },
            },
            ..Default::default()
        }
    }

    fn reject_at(list: *mut OpenPitPretradeRejectList, index: usize) -> OpenPitPretradeReject {
        let mut reject = OpenPitPretradeReject {
            policy: OpenPitStringView::not_set(),
            reason: OpenPitStringView::not_set(),
            details: OpenPitStringView::not_set(),
            user_data: std::ptr::null_mut(),
            code: OpenPitPretradeRejectCode::Other,
            scope: OpenPitPretradeRejectScope::Order,
        };
        assert!(openpit_pretrade_reject_list_get(list, index, &mut reject));
        reject
    }

    fn assert_engine_account_block_reject(reject: OpenPitPretradeReject, reason: &str) {
        assert_eq!(reject.code, OpenPitPretradeRejectCode::AccountBlocked);
        assert_eq!(reject.scope, OpenPitPretradeRejectScope::Account);
        assert_eq!(string_view_to_string(reject.policy), "Engine");
        assert_eq!(string_view_to_string(reject.reason), reason);
        assert_eq!(string_view_to_string(reject.details), reason);
    }

    #[test]
    fn string_view_to_string_handles_null_pointer() {
        assert_eq!(string_view_to_string(OpenPitStringView::not_set()), "");
    }

    #[test]
    fn configure_error_preserves_nested_configuration_variant() {
        let error = super::OpenPitConfigureError::new(openpit::ConfigureError::NestedConfiguration);

        assert_eq!(
            error.kind,
            super::OpenPitConfigureErrorKind::NestedConfiguration
        );
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
            OpenPitSyncPolicy::None as u8,
            OpenPitSyncPolicy::Full as u8,
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
        let build_error_sentinel =
            core::ptr::NonNull::<OpenPitEngineBuildError>::dangling().as_ptr();
        let mut build_error = build_error_sentinel;
        let engine = openpit_engine_builder_build(
            std::ptr::null_mut(),
            &mut build_error,
            std::ptr::null_mut(),
        );
        assert!(engine.is_null());
        assert_eq!(build_error, build_error_sentinel);

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
        let built = openpit_engine_builder_build(builder, &mut build_error, std::ptr::null_mut());
        assert!(!built.is_null());
        assert_eq!(build_error, build_error_sentinel);
        openpit_destroy_engine(built);
        let consumed =
            openpit_engine_builder_build(builder, &mut build_error, std::ptr::null_mut());
        assert!(consumed.is_null());
        assert_eq!(build_error, build_error_sentinel);
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

        build_error = std::ptr::null_mut();
        let invalid =
            openpit_engine_builder_build(dup_builder, &mut build_error, std::ptr::null_mut());
        assert!(invalid.is_null());
        assert!(!build_error.is_null());
        assert_eq!(
            openpit_engine_build_error_get_code(build_error),
            OpenPitEngineBuildErrorCode::DuplicatePolicyName
        );
        assert_eq!(
            string_view_to_string(openpit_engine_build_error_get_policy_name(build_error)),
            "dup.start"
        );
        assert_eq!(
            openpit_engine_build_error_get_policy_group_id(build_error),
            0
        );
        openpit_destroy_engine_build_error(build_error);
        openpit_destroy_engine_build_error(std::ptr::null_mut());
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
        let engine =
            openpit_engine_builder_build(builder, std::ptr::null_mut(), std::ptr::null_mut());
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
        let mut out_request =
            core::ptr::NonNull::<super::OpenPitPretradePreTradeRequest>::dangling().as_ptr();
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
            core::ptr::NonNull::<super::OpenPitPretradePreTradeRequest>::dangling().as_ptr()
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
            core::ptr::NonNull::<super::OpenPitPretradePreTradeReservation>::dangling().as_ptr();
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
            core::ptr::NonNull::<super::OpenPitPretradePreTradeReservation>::dangling().as_ptr()
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
            core::ptr::NonNull::<super::OpenPitPretradePreTradeReservation>::dangling().as_ptr();
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
            core::ptr::NonNull::<super::OpenPitPretradePreTradeReservation>::dangling().as_ptr()
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
        let reject_sentinel =
            core::ptr::NonNull::<OpenPitAccountAdjustmentBatchError>::dangling().as_ptr();
        let outcomes_sentinel = core::ptr::NonNull::<
            crate::account_outcome::OpenPitAccountAdjustmentOutcomeList,
        >::dangling()
        .as_ptr();
        let error_sentinel =
            core::ptr::NonNull::<crate::string::OpenPitSharedString>::dangling().as_ptr();
        let mut out_reject = reject_sentinel;
        let mut out_outcomes = outcomes_sentinel;
        let mut out_error = error_sentinel;

        let status = openpit_engine_apply_account_adjustment(
            engine,
            1,
            std::ptr::null(),
            0,
            &mut out_reject,
            &mut out_outcomes,
            &mut out_error,
        );

        assert_eq!(status, OpenPitAccountAdjustmentApplyStatus::Applied);
        assert_eq!(out_reject, reject_sentinel);
        assert_eq!(out_outcomes, outcomes_sentinel);
        assert_eq!(out_error, error_sentinel);
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
            std::ptr::null_mut(),
        );

        assert_eq!(status, OpenPitAccountAdjustmentApplyStatus::Error);
        assert!(out_reject.is_null());
        openpit_destroy_engine(engine);
    }

    #[test]
    fn apply_account_adjustment_reports_import_error_for_incomplete_payload() {
        let engine = build_passthrough_engine();

        // A half-set instrument (one asset provided, the other absent) is
        // rejected at import time — "both or neither" is the rule.
        let invalid = crate::account_adjustment::OpenPitAccountAdjustment {
            operation: crate::account_adjustment::OpenPitAccountAdjustmentOperation {
                kind: crate::account_adjustment::OpenPitAccountAdjustmentOperationKind::Position,
                position: crate::account_adjustment::OpenPitAccountAdjustmentPositionOperation {
                    instrument: crate::instrument::OpenPitInstrument {
                        underlying_asset: OpenPitStringView::from_utf8("SPX"),
                        settlement_asset: OpenPitStringView::not_set(),
                    },
                    mode: crate::param::OpenPitParamPositionMode::Hedged,
                    ..Default::default()
                },
                ..Default::default()
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
            std::ptr::null_mut(),
        );
        assert_eq!(status, OpenPitAccountAdjustmentApplyStatus::Error);
    }

    #[test]
    fn lock_create_is_empty_handle() {
        let detached = crate::pre_trade_lock::openpit_create_pretrade_pre_trade_lock();
        assert!(!detached.is_null());
        assert_eq!(
            crate::pre_trade_lock::openpit_pretrade_pre_trade_lock_len(detached),
            0
        );
        crate::pre_trade_lock::openpit_destroy_pretrade_pre_trade_lock(detached);
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
        assert!(!lock.is_null());
        assert_eq!(
            crate::pre_trade_lock::openpit_pretrade_pre_trade_lock_len(lock),
            0
        );
        crate::pre_trade_lock::openpit_destroy_pretrade_pre_trade_lock(lock);

        openpit_pretrade_pre_trade_reservation_commit(out_reservation);
        let committed_lock = openpit_pretrade_pre_trade_reservation_get_lock(out_reservation);
        assert!(!committed_lock.is_null());
        assert_eq!(
            crate::pre_trade_lock::openpit_pretrade_pre_trade_lock_len(committed_lock),
            0
        );
        crate::pre_trade_lock::openpit_destroy_pretrade_pre_trade_lock(committed_lock);

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
            std::ptr::null_mut(),
        );
        assert_eq!(status, OpenPitAccountAdjustmentApplyStatus::Rejected);

        openpit_destroy_engine(engine);
    }

    #[test]
    fn apply_account_adjustment_does_not_touch_out_reject_on_transport_error() {
        let engine = build_engine_with_reject_policy();
        let mut out_reject =
            core::ptr::NonNull::<OpenPitAccountAdjustmentBatchError>::dangling().as_ptr();
        let status = openpit_engine_apply_account_adjustment(
            engine,
            1,
            std::ptr::null(),
            1,
            &mut out_reject,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
        assert_eq!(status, OpenPitAccountAdjustmentApplyStatus::Error);
        assert_eq!(
            out_reject,
            core::ptr::NonNull::<OpenPitAccountAdjustmentBatchError>::dangling().as_ptr()
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
            std::ptr::null_mut(),
        ));

        let engine = build_passthrough_engine();

        assert!(!openpit_engine_apply_execution_report(
            engine,
            std::ptr::null(),
            std::ptr::null_mut(),
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
            std::ptr::null_mut(),
        ));
        openpit_destroy_engine(engine);

        let callback_engine = build_engine_with_main_reject_policy();
        let report = OpenPitExecutionReport::default();
        let blocks_sentinel =
            core::ptr::NonNull::<crate::reject::OpenPitPretradeAccountBlockList>::dangling()
                .as_ptr();
        let adjustments_sentinel = core::ptr::NonNull::<
            crate::account_outcome::OpenPitAccountAdjustmentOutcomeList,
        >::dangling()
        .as_ptr();
        let error_sentinel =
            core::ptr::NonNull::<crate::string::OpenPitSharedString>::dangling().as_ptr();
        let mut out_blocks = blocks_sentinel;
        let mut out_adjustments = adjustments_sentinel;
        let mut out_error = error_sentinel;
        assert!(openpit_engine_apply_execution_report(
            callback_engine,
            &report,
            &mut out_blocks,
            &mut out_adjustments,
            &mut out_error,
        ));
        assert_eq!(out_blocks, blocks_sentinel);
        assert_eq!(out_adjustments, adjustments_sentinel);
        assert_eq!(out_error, error_sentinel);
        openpit_destroy_engine(callback_engine);
    }

    #[derive(Default)]
    struct ObservedFillLock {
        invoked: bool,
        lock_was_null: bool,
        lock_len: usize,
    }

    unsafe extern "C" fn record_fill_lock_apply(
        _ctx: *const crate::policy::custom::OpenPitPostTradeContext,
        report: *const OpenPitExecutionReport,
        _out_adjustments: *mut crate::account_outcome::OpenPitPostTradeAdjustmentList,
        user_data: *mut c_void,
    ) -> *mut crate::reject::OpenPitPretradeAccountBlockList {
        let observed = unsafe { &mut *(user_data as *mut ObservedFillLock) };
        observed.invoked = true;
        let lock = unsafe { &*report }.fill.value.lock;
        if lock.is_null() {
            observed.lock_was_null = true;
        } else {
            observed.lock_len = crate::pre_trade_lock::openpit_pretrade_pre_trade_lock_len(lock);
        }
        std::ptr::null_mut()
    }

    #[test]
    fn apply_execution_report_frees_exported_fill_lock() {
        use openpit::param::Price;
        use openpit::pretrade::PreTradeLock;
        use openpit::PolicyGroupId;

        let mut observed = ObservedFillLock::default();

        let builder =
            openpit_create_engine_builder(OpenPitSyncPolicy::Full as u8, std::ptr::null_mut());
        let policy = unsafe {
            openpit_create_pretrade_custom_pre_trade_policy(
                OpenPitStringView::from_utf8("record.fill.lock"),
                0,
                None,
                None,
                Some(record_fill_lock_apply),
                None,
                noop_free_user_data,
                (&mut observed as *mut ObservedFillLock).cast::<c_void>(),
                std::ptr::null_mut(),
            )
        };
        assert!(!policy.is_null(), "failed to create policy");
        assert!(openpit_engine_builder_add_pre_trade_policy(
            builder,
            policy,
            std::ptr::null_mut()
        ));
        openpit_destroy_pretrade_pre_trade_policy(policy);
        let engine =
            openpit_engine_builder_build(builder, std::ptr::null_mut(), std::ptr::null_mut());
        assert!(!engine.is_null(), "engine build failed");

        // Input report carrying a one-entry lock. The engine imports it
        // (lock -> Some(PreTradeLock)); the custom-policy trampoline then
        // re-exports it for the callback, allocating a fresh owned lock handle
        // that the trampoline must release exactly once after the callback.
        let input_lock = crate::pre_trade_lock::OpenPitPretradePreTradeLock::from_inner(
            PreTradeLock::from_entries([(
                PolicyGroupId::new(7),
                Price::from_str("101").expect("price must be valid"),
            )]),
        );
        let report = OpenPitExecutionReport {
            operation: OpenPitExecutionReportOperationOptional::default(),
            financial_impact: OpenPitFinancialImpactOptional::default(),
            fill: crate::execution_report::OpenPitExecutionReportFillOptional {
                is_set: true,
                value: crate::execution_report::OpenPitExecutionReportFill {
                    last_trade:
                        crate::execution_report::OpenPitExecutionReportTradeOptional::default(),
                    fee: crate::param::OpenPitParamMonetaryAmountOptional::default(),
                    leaves_quantity: crate::param::OpenPitParamQuantityOptional::default(),
                    lock: input_lock,
                    is_final:
                        crate::execution_report::OpenPitExecutionReportIsFinalOptional::default(),
                },
            },
            position_impact: OpenPitExecutionReportPositionImpactOptional::default(),
            user_data: std::ptr::null_mut(),
        };

        assert!(openpit_engine_apply_execution_report(
            engine,
            &report,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        ));

        // The callback observed the re-exported lock (non-null, one entry).
        // Completing without abort proves the trampoline freed the exported
        // lock exactly once - no double-free and no use-after-free.
        assert!(observed.invoked);
        assert!(!observed.lock_was_null);
        assert_eq!(observed.lock_len, 1);

        openpit_destroy_engine(engine);
        // Release the caller-owned input lock built for this test.
        crate::pre_trade_lock::openpit_destroy_pretrade_pre_trade_lock(input_lock);
    }

    #[test]
    fn account_group_outputs_change_only_for_their_error_channel() {
        let engine = build_passthrough_engine();
        let account = 7;
        let group_error_sentinel =
            core::ptr::NonNull::<super::OpenPitAccountGroupError>::dangling().as_ptr();
        let transport_error_sentinel =
            core::ptr::NonNull::<crate::string::OpenPitSharedString>::dangling().as_ptr();
        let mut group_error = group_error_sentinel;
        let mut transport_error = transport_error_sentinel;

        assert!(super::openpit_engine_register_account_group(
            engine,
            &account,
            1,
            1,
            &mut group_error,
            &mut transport_error,
        ));
        assert_eq!(group_error, group_error_sentinel);
        assert_eq!(transport_error, transport_error_sentinel);

        assert!(super::openpit_engine_unregister_account_group(
            engine,
            &account,
            1,
            1,
            &mut group_error,
            &mut transport_error,
        ));
        assert_eq!(group_error, group_error_sentinel);
        assert_eq!(transport_error, transport_error_sentinel);

        assert!(!super::openpit_engine_register_account_group(
            engine,
            std::ptr::null(),
            1,
            1,
            &mut group_error,
            &mut transport_error,
        ));
        assert_eq!(group_error, group_error_sentinel);
        assert_ne!(transport_error, transport_error_sentinel);
        crate::string::openpit_destroy_shared_string(transport_error);

        transport_error = transport_error_sentinel;
        assert!(!super::openpit_engine_register_account_group(
            engine,
            std::ptr::null(),
            0,
            0,
            &mut group_error,
            &mut transport_error,
        ));
        assert_ne!(group_error, group_error_sentinel);
        assert_eq!(transport_error, transport_error_sentinel);
        super::openpit_destroy_account_group_error(group_error);
        openpit_destroy_engine(engine);
    }

    #[test]
    fn account_block_error_outputs_are_untouched_on_success() {
        let engine = build_passthrough_engine();
        let account = 7;
        let reason = OpenPitStringView::from_utf8("reason");
        let error_sentinel =
            core::ptr::NonNull::<super::OpenPitAccountBlockError>::dangling().as_ptr();
        let mut out_error = error_sentinel;

        super::openpit_engine_block_account(engine, account, reason);
        assert!(super::openpit_engine_replace_account_block_reason(
            engine,
            account,
            reason,
            &mut out_error,
        ));
        assert_eq!(out_error, error_sentinel);

        assert!(super::openpit_engine_register_account_group(
            engine,
            &account,
            1,
            1,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        ));
        assert!(super::openpit_engine_block_account_group(
            engine,
            1,
            reason,
            &mut out_error,
        ));
        assert_eq!(out_error, error_sentinel);
        assert!(super::openpit_engine_replace_account_group_block_reason(
            engine,
            1,
            reason,
            &mut out_error,
        ));
        assert_eq!(out_error, error_sentinel);
        assert!(super::openpit_engine_unblock_account_group(
            engine,
            1,
            &mut out_error,
        ));
        assert_eq!(out_error, error_sentinel);

        assert!(!super::openpit_engine_block_account_group(
            engine,
            0,
            reason,
            &mut out_error,
        ));
        assert_ne!(out_error, error_sentinel);
        super::openpit_destroy_account_block_error(out_error);
        openpit_destroy_engine(engine);
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
    fn execute_pre_trade_and_dry_run_return_owned_account_block_reject_text() {
        let engine = build_passthrough_engine();
        let account_id = 1001;
        let reason = "blocked by operator";
        openpit_engine_block_account(engine, account_id, OpenPitStringView::from_utf8(reason));
        let order = order_for_account(account_id);

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
        assert!(out_reservation.is_null());
        assert!(!out_rejects.is_null());
        assert_eq!(openpit_pretrade_reject_list_len(out_rejects), 1);
        assert_engine_account_block_reject(reject_at(out_rejects, 0), reason);
        openpit_pretrade_destroy_reject_list(out_rejects);

        let mut out_report = std::ptr::null_mut();
        assert!(openpit_engine_execute_pre_trade_dry_run(
            engine,
            &order,
            &mut out_report,
            std::ptr::null_mut(),
        ));
        assert!(!out_report.is_null());
        assert!(!openpit_pretrade_pre_trade_dry_run_report_is_pass(
            out_report
        ));
        let dry_run_rejects = openpit_pretrade_pre_trade_dry_run_report_get_rejects(out_report);
        assert!(!dry_run_rejects.is_null());
        assert_eq!(openpit_pretrade_reject_list_len(dry_run_rejects), 1);
        assert_engine_account_block_reject(reject_at(dry_run_rejects, 0), reason);
        openpit_pretrade_destroy_reject_list(dry_run_rejects);
        openpit_destroy_pretrade_pre_trade_dry_run_report(out_report);
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
        .sync(openpit_interop::EngineLocking::new(
            openpit_interop::SyncMode::None,
        ))
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

    unsafe extern "C" fn always_reject_start_check_account_scope(
        _ctx: *const crate::policy::OpenPitPretradeContext,
        _order: *const crate::order::OpenPitOrder,
        _user_data: *mut c_void,
    ) -> *mut OpenPitPretradeRejectList {
        let rejects = openpit_pretrade_create_reject_list(1);
        crate::reject::openpit_pretrade_reject_list_push(
            rejects,
            OpenPitPretradeReject {
                policy: OpenPitStringView::from_utf8("start.account.reject"),
                reason: OpenPitStringView::from_utf8("blocked"),
                details: OpenPitStringView::from_utf8("by test"),
                user_data: std::ptr::null_mut(),
                code: OpenPitPretradeRejectCode::AccountBlocked,
                scope: OpenPitPretradeRejectScope::Account,
            },
        );
        rejects
    }

    fn build_engine_with_start_account_reject_policy() -> *mut super::OpenPitEngine {
        let builder =
            openpit_create_engine_builder(OpenPitSyncPolicy::Full as u8, std::ptr::null_mut());
        let name = OpenPitStringView::from_utf8("start.account.reject");
        let policy = unsafe {
            create_pre_trade_policy_with_start_hook(
                name,
                always_reject_start_check_account_scope,
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
        let engine =
            openpit_engine_builder_build(builder, std::ptr::null_mut(), std::ptr::null_mut());
        assert!(!engine.is_null(), "engine build failed");
        engine
    }

    #[test]
    fn start_pre_trade_dry_run_does_not_touch_out_report_on_error() {
        let report_sentinel =
            core::ptr::NonNull::<super::OpenPitPretradePreTradeDryRunReport>::dangling().as_ptr();
        let mut out_report = report_sentinel;

        // Null engine and null order both take the transport-error path and must
        // leave the out-pointer untouched.
        assert!(!openpit_engine_start_pre_trade_dry_run(
            std::ptr::null_mut(),
            std::ptr::null(),
            &mut out_report,
            std::ptr::null_mut(),
        ));
        assert_eq!(out_report, report_sentinel);

        let engine = build_passthrough_engine();
        assert!(!openpit_engine_start_pre_trade_dry_run(
            engine,
            std::ptr::null(),
            &mut out_report,
            std::ptr::null_mut(),
        ));
        assert_eq!(out_report, report_sentinel);
        openpit_destroy_engine(engine);
    }

    #[test]
    fn execute_pre_trade_dry_run_does_not_touch_out_report_on_error() {
        let report_sentinel =
            core::ptr::NonNull::<super::OpenPitPretradePreTradeDryRunReport>::dangling().as_ptr();
        let mut out_report = report_sentinel;

        assert!(!openpit_engine_execute_pre_trade_dry_run(
            std::ptr::null_mut(),
            std::ptr::null(),
            &mut out_report,
            std::ptr::null_mut(),
        ));
        assert_eq!(out_report, report_sentinel);

        let engine = build_passthrough_engine();
        assert!(!openpit_engine_execute_pre_trade_dry_run(
            engine,
            std::ptr::null(),
            &mut out_report,
            std::ptr::null_mut(),
        ));
        assert_eq!(out_report, report_sentinel);
        openpit_destroy_engine(engine);
    }

    #[test]
    fn execute_pre_trade_dry_run_pass_path_reports_empty_lock_and_adjustments() {
        let engine = build_passthrough_engine();
        let order = OpenPitOrder::default();
        let mut out_report = std::ptr::null_mut();

        assert!(openpit_engine_execute_pre_trade_dry_run(
            engine,
            &order,
            &mut out_report,
            std::ptr::null_mut(),
        ));
        assert!(!out_report.is_null());
        assert!(openpit_pretrade_pre_trade_dry_run_report_is_pass(
            out_report
        ));

        let rejects = openpit_pretrade_pre_trade_dry_run_report_get_rejects(out_report);
        assert!(!rejects.is_null());
        assert_eq!(openpit_pretrade_reject_list_len(rejects), 0);
        openpit_pretrade_destroy_reject_list(rejects);

        let lock = openpit_pretrade_pre_trade_dry_run_report_get_lock(out_report);
        assert!(!lock.is_null());
        assert_eq!(
            crate::pre_trade_lock::openpit_pretrade_pre_trade_lock_len(lock),
            0
        );
        crate::pre_trade_lock::openpit_destroy_pretrade_pre_trade_lock(lock);

        let adjustments =
            openpit_pretrade_pre_trade_dry_run_report_get_account_adjustments(out_report);
        assert!(!adjustments.is_null());
        assert_eq!(
            unsafe {
                crate::account_outcome::openpit_account_adjustment_outcome_list_len(adjustments)
            },
            0
        );
        unsafe {
            crate::account_outcome::openpit_destroy_account_adjustment_outcome_list(adjustments)
        };

        let blocks = openpit_pretrade_pre_trade_dry_run_report_get_account_block(out_report);
        assert!(!blocks.is_null());
        assert_eq!(
            crate::reject::openpit_pretrade_account_block_list_len(blocks),
            0
        );
        crate::reject::openpit_pretrade_destroy_account_block_list(blocks);

        openpit_destroy_pretrade_pre_trade_dry_run_report(out_report);
        openpit_destroy_engine(engine);
    }

    #[test]
    fn execute_pre_trade_dry_run_reject_path_reports_rejects_without_block() {
        // An order-scope main-stage reject is reported in the rejects list, but
        // latches no account block.
        let engine = build_engine_with_main_reject_policy();
        let order = OpenPitOrder::default();
        let mut out_report = std::ptr::null_mut();

        assert!(openpit_engine_execute_pre_trade_dry_run(
            engine,
            &order,
            &mut out_report,
            std::ptr::null_mut(),
        ));
        assert!(!out_report.is_null());
        assert!(!openpit_pretrade_pre_trade_dry_run_report_is_pass(
            out_report
        ));

        let rejects = openpit_pretrade_pre_trade_dry_run_report_get_rejects(out_report);
        assert!(!rejects.is_null());
        assert_eq!(openpit_pretrade_reject_list_len(rejects), 1);
        openpit_pretrade_destroy_reject_list(rejects);

        let blocks = openpit_pretrade_pre_trade_dry_run_report_get_account_block(out_report);
        assert!(!blocks.is_null());
        assert_eq!(
            crate::reject::openpit_pretrade_account_block_list_len(blocks),
            0
        );
        crate::reject::openpit_pretrade_destroy_account_block_list(blocks);

        openpit_destroy_pretrade_pre_trade_dry_run_report(out_report);
        openpit_destroy_engine(engine);
    }

    #[test]
    fn start_pre_trade_dry_run_reject_path_reports_account_block() {
        // A start-stage account-scope reject is reported with a would-be account
        // block - and a dry-run reports it without recording it in the engine.
        let engine = build_engine_with_start_account_reject_policy();
        let order = OpenPitOrder::default();
        let mut out_report = std::ptr::null_mut();

        assert!(openpit_engine_start_pre_trade_dry_run(
            engine,
            &order,
            &mut out_report,
            std::ptr::null_mut(),
        ));
        assert!(!out_report.is_null());
        assert!(!openpit_pretrade_pre_trade_dry_run_report_is_pass(
            out_report
        ));

        let rejects = openpit_pretrade_pre_trade_dry_run_report_get_rejects(out_report);
        assert!(!rejects.is_null());
        assert_eq!(openpit_pretrade_reject_list_len(rejects), 1);
        openpit_pretrade_destroy_reject_list(rejects);

        let blocks = openpit_pretrade_pre_trade_dry_run_report_get_account_block(out_report);
        assert!(!blocks.is_null());
        assert_eq!(
            crate::reject::openpit_pretrade_account_block_list_len(blocks),
            1
        );
        let mut block = crate::reject::OpenPitPretradeAccountBlock {
            policy: OpenPitStringView::not_set(),
            reason: OpenPitStringView::not_set(),
            details: OpenPitStringView::not_set(),
            user_data: std::ptr::null_mut(),
            code: OpenPitPretradeRejectCode::Other,
        };
        assert!(crate::reject::openpit_pretrade_account_block_list_get(
            blocks, 0, &mut block
        ));
        assert_eq!(string_view_to_string(block.policy), "start.account.reject");
        crate::reject::openpit_pretrade_destroy_account_block_list(blocks);

        openpit_destroy_pretrade_pre_trade_dry_run_report(out_report);

        // The dry-run did not record the block: a real check still reaches the
        // policy rather than being short-circuited by a recorded block.
        openpit_destroy_engine(engine);
    }

    #[test]
    fn destroy_dry_run_report_is_null_safe() {
        openpit_destroy_pretrade_pre_trade_dry_run_report(std::ptr::null_mut());
    }

    #[test]
    fn set_account_group_currency_accepts_default_and_nonzero_group() {
        // Group id 0 maps to DEFAULT_ACCOUNT_GROUP (allowed); any non-zero u32
        // is a valid group id. Both must succeed; out_error must be untouched.
        let engine = build_passthrough_engine();
        let asset = OpenPitStringView::from_utf8("USD");
        let error_sentinel =
            core::ptr::NonNull::<crate::string::OpenPitSharedString>::dangling().as_ptr();
        let mut out_error = error_sentinel;

        assert!(super::openpit_engine_set_account_group_currency(
            engine,
            0,
            asset,
            &mut out_error,
        ));
        assert_eq!(
            out_error, error_sentinel,
            "out_error must be untouched on success"
        );

        assert!(super::openpit_engine_set_account_group_currency(
            engine,
            42,
            asset,
            &mut out_error,
        ));
        assert_eq!(
            out_error, error_sentinel,
            "out_error must be untouched on success"
        );

        openpit_destroy_engine(engine);
    }

    #[test]
    fn clear_account_group_currency_accepts_default_and_nonzero_group() {
        // clear has no error channel; invalid ids are a documented no-op.
        // Verify that valid cases (0 and non-zero) do not panic.
        let engine = build_passthrough_engine();
        super::openpit_engine_clear_account_group_currency(engine, 0);
        super::openpit_engine_clear_account_group_currency(engine, 99);
        openpit_destroy_engine(engine);
    }
}
