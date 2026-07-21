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

//! Pre-trade flow result handles and the post-trade result.
//!
//! The two-stage flow is reproduced exactly: `startPreTrade` yields a
//! [`JsStartResult`] carrying a single-use [`JsRequest`]; `request.execute()`
//! yields a [`JsExecuteResult`] carrying a single-use [`JsReservation`]; the
//! reservation is committed or rolled back exactly once. Single-use handles
//! share their core value through an `Rc<RefCell<Option<..>>>` and throw
//! `LifecycleError` on reuse.

use std::{cell::RefCell, rc::Rc};

use js_sys::Array;
use openpit::pretrade::{PreTradeDryRunReport, PreTradeRequest, PreTradeReservation, Rejects};
use openpit::{AccountAdjustmentBatchError, AccountAdjustmentBatchResult, PostTradeResult};
use wasm_bindgen::convert::TryFromJsValue;
use wasm_bindgen::prelude::*;

use crate::context::LifecycleToken;
use crate::domain::extract_cloned_wrapper;
use crate::engine::Order;
use crate::error::{make_error, policy_callback_error, ErrorKind};
use crate::lock::JsLock;
use crate::outcome::{JsAccountAdjustmentOutcome, JsAccountPnlOutcome};
use crate::policy::CallbackErrorScope;
use crate::reject::{JsAccountBlock, JsReject};

type SharedRequest = Rc<RefCell<Option<(PreTradeRequest<Order>, LifecycleToken)>>>;
type SharedReservation = Rc<RefCell<Option<(PreTradeReservation, LifecycleToken)>>>;

/// Maps a core rejects list to an array of binding rejects.
fn convert_rejects(rejects: &Rejects) -> Vec<JsReject> {
    rejects.iter().map(JsReject::from_core).collect()
}

/// Maps a core outcome slice to an array of binding outcomes.
fn convert_outcomes(
    outcomes: &[openpit::AccountAdjustmentOutcome],
) -> Vec<JsAccountAdjustmentOutcome> {
    outcomes
        .iter()
        .map(JsAccountAdjustmentOutcome::from_core)
        .collect()
}

/// Builds the error raised when a single-use handle is reused.
fn lifecycle_error(message: &str) -> JsValue {
    make_error(ErrorKind::Lifecycle, message, None)
}

// ─── StartResult ─────────────────────────────────────────────────────────────

/// Outcome of `engine.startPreTrade`.
///
/// On success `ok` is `true` and `request` is a single-use `Request`; on
/// rejection `ok` is `false` and `rejects` is non-empty.
#[wasm_bindgen(js_name = StartResult)]
pub struct JsStartResult {
    request: Option<JsRequest>,
    rejects: Vec<JsReject>,
}

#[wasm_bindgen(js_class = StartResult)]
impl JsStartResult {
    /// Whether the start stage accepted the order.
    #[wasm_bindgen(getter, js_name = ok)]
    pub fn ok(&self) -> bool {
        self.rejects.is_empty()
    }

    /// The shared single-use request, or `undefined` on rejection.
    ///
    /// Repeated reads return handles sharing the same one-shot lifecycle.
    #[wasm_bindgen(getter, js_name = request)]
    pub fn request(&self) -> Option<JsRequest> {
        self.request.clone()
    }

    /// The rejects, empty on success.
    #[wasm_bindgen(getter, js_name = rejects)]
    pub fn rejects(&self) -> Vec<JsReject> {
        self.rejects.clone()
    }
}

impl JsStartResult {
    /// Builds an accepted start result wrapping the request handle.
    pub(crate) fn accepted(request: PreTradeRequest<Order>, lifecycle: LifecycleToken) -> Self {
        Self {
            request: Some(JsRequest::new(request, lifecycle)),
            rejects: Vec::new(),
        }
    }

    /// Builds a rejected start result from the core rejects.
    pub(crate) fn rejected(rejects: &Rejects) -> Self {
        Self {
            request: None,
            rejects: convert_rejects(rejects),
        }
    }
}

// ─── ExecuteResult ───────────────────────────────────────────────────────────

/// Outcome of `request.execute()` or `engine.executePreTrade`.
///
/// On success `ok` is `true` and `reservation` is a single-use `Reservation`;
/// on rejection `ok` is `false` and `rejects` is non-empty.
#[wasm_bindgen(js_name = ExecuteResult)]
pub struct JsExecuteResult {
    reservation: Option<JsReservation>,
    rejects: Vec<JsReject>,
}

#[wasm_bindgen(js_class = ExecuteResult)]
impl JsExecuteResult {
    /// Whether the main stage accepted the order.
    #[wasm_bindgen(getter, js_name = ok)]
    pub fn ok(&self) -> bool {
        self.rejects.is_empty()
    }

    /// The shared single-use reservation, or `undefined` on rejection.
    ///
    /// Repeated reads return handles sharing the same one-shot lifecycle.
    #[wasm_bindgen(getter, js_name = reservation)]
    pub fn reservation(&self) -> Option<JsReservation> {
        self.reservation.clone()
    }

    /// The rejects, empty on success.
    #[wasm_bindgen(getter, js_name = rejects)]
    pub fn rejects(&self) -> Vec<JsReject> {
        self.rejects.clone()
    }
}

impl JsExecuteResult {
    /// Builds an accepted execute result wrapping the reservation handle.
    pub(crate) fn accepted(reservation: PreTradeReservation, lifecycle: LifecycleToken) -> Self {
        Self {
            reservation: Some(JsReservation::new(reservation, lifecycle)),
            rejects: Vec::new(),
        }
    }

    /// Builds a rejected execute result from the core rejects.
    pub(crate) fn rejected(rejects: &Rejects) -> Self {
        Self {
            reservation: None,
            rejects: convert_rejects(rejects),
        }
    }
}

// ─── Request ─────────────────────────────────────────────────────────────────

/// Single-use deferred handle returned by a successful start stage.
///
/// `execute()` runs the main stage and consumes the handle; a second call
/// throws `LifecycleError`.
#[wasm_bindgen(js_name = Request)]
#[derive(Clone)]
pub struct JsRequest {
    inner: SharedRequest,
}

#[wasm_bindgen(js_class = Request)]
impl JsRequest {
    /// Runs the main stage and returns a [`JsExecuteResult`].
    ///
    /// # Errors
    ///
    /// Throws `LifecycleError` when the request has already been executed.
    #[wasm_bindgen(js_name = execute)]
    pub fn execute(&self) -> Result<JsExecuteResult, JsValue> {
        let (request, lifecycle) = self
            .inner
            .borrow_mut()
            .take()
            .ok_or_else(|| lifecycle_error("request has already been executed"))?;
        let callback_scope = CallbackErrorScope::capture();
        let result = match request.execute() {
            Ok(reservation) => JsExecuteResult::accepted(reservation, lifecycle.clone()),
            Err(rejects) => {
                lifecycle.invalidate();
                JsExecuteResult::rejected(&rejects)
            }
        };
        if let Some(cause) = callback_scope.finish() {
            return Err(policy_callback_error(cause, JsValue::UNDEFINED));
        }
        Ok(result)
    }
}

impl JsRequest {
    /// Wraps a core pre-trade request handle.
    fn new(inner: PreTradeRequest<Order>, lifecycle: LifecycleToken) -> Self {
        Self {
            inner: Rc::new(RefCell::new(Some((inner, lifecycle)))),
        }
    }
}

impl Drop for JsRequest {
    fn drop(&mut self) {
        let Some(inner) = Rc::get_mut(&mut self.inner) else {
            return;
        };
        if let Some((request, lifecycle)) = inner.get_mut().take() {
            lifecycle.invalidate();
            drop(request);
        }
    }
}

// ─── Reservation ─────────────────────────────────────────────────────────────

/// Single-use handle for reserved pre-trade state.
///
/// `lock()` and `accountAdjustments()` may be read while the reservation is
/// live. `commit()` and `rollback()` each consume the handle; calling either a
/// second time (or after the other) throws `LifecycleError`.
#[wasm_bindgen(js_name = Reservation)]
#[derive(Clone)]
pub struct JsReservation {
    inner: SharedReservation,
}

#[wasm_bindgen(js_class = Reservation)]
impl JsReservation {
    /// Returns the lock payload accumulated for this reservation.
    ///
    /// # Errors
    ///
    /// Throws `LifecycleError` when the reservation has been finalized.
    #[wasm_bindgen(js_name = lock)]
    pub fn lock(&self) -> Result<JsLock, JsValue> {
        let reservation = self.inner.borrow();
        let (reservation, _) = reservation
            .as_ref()
            .ok_or_else(|| lifecycle_error("reservation has already been finalized"))?;
        Ok(JsLock::from_inner(reservation.lock().clone()))
    }

    /// Returns the account adjustments produced by this reservation.
    ///
    /// # Errors
    ///
    /// Throws `LifecycleError` when the reservation has been finalized.
    #[wasm_bindgen(js_name = accountAdjustments)]
    pub fn account_adjustments(&self) -> Result<Vec<JsAccountAdjustmentOutcome>, JsValue> {
        let reservation = self.inner.borrow();
        let (reservation, _) = reservation
            .as_ref()
            .ok_or_else(|| lifecycle_error("reservation has already been finalized"))?;
        Ok(convert_outcomes(reservation.account_adjustments()))
    }

    /// Returns the winning account block produced by this reservation's pipeline,
    /// or `undefined` when no account-scoped reject was produced.
    ///
    /// # Errors
    ///
    /// Throws `LifecycleError` when the reservation has been finalized.
    #[wasm_bindgen(js_name = accountBlock)]
    pub fn account_block(&self) -> Result<Option<JsAccountBlock>, JsValue> {
        let reservation = self.inner.borrow();
        let (reservation, _) = reservation
            .as_ref()
            .ok_or_else(|| lifecycle_error("reservation has already been finalized"))?;
        Ok(reservation.account_block().map(JsAccountBlock::from_core))
    }

    /// Commits the reserved state.
    ///
    /// # Errors
    ///
    /// Throws `LifecycleError` when the reservation has already been finalized.
    #[wasm_bindgen(js_name = commit)]
    pub fn commit(&self) -> Result<(), JsValue> {
        let (mut reservation, lifecycle) = self.take()?;
        let callback_scope = CallbackErrorScope::capture();
        reservation.commit();
        lifecycle.invalidate();
        match callback_scope.finish() {
            Some(cause) => Err(policy_callback_error(cause, JsValue::UNDEFINED)),
            None => Ok(()),
        }
    }

    /// Rolls back the reserved state.
    ///
    /// # Errors
    ///
    /// Throws `LifecycleError` when the reservation has already been finalized.
    #[wasm_bindgen(js_name = rollback)]
    pub fn rollback(&self) -> Result<(), JsValue> {
        let (mut reservation, lifecycle) = self.take()?;
        let callback_scope = CallbackErrorScope::capture();
        reservation.rollback();
        lifecycle.invalidate();
        match callback_scope.finish() {
            Some(cause) => Err(policy_callback_error(cause, JsValue::UNDEFINED)),
            None => Ok(()),
        }
    }
}

impl JsReservation {
    /// Wraps a core reservation handle.
    pub(crate) fn new(inner: PreTradeReservation, lifecycle: LifecycleToken) -> Self {
        Self {
            inner: Rc::new(RefCell::new(Some((inner, lifecycle)))),
        }
    }

    /// Removes and returns the reservation, leaving the handle finalized.
    ///
    /// # Errors
    ///
    /// Throws `LifecycleError` when the reservation has already been finalized.
    fn take(&self) -> Result<(PreTradeReservation, LifecycleToken), JsValue> {
        self.inner
            .borrow_mut()
            .take()
            .ok_or_else(|| lifecycle_error("reservation has already been finalized"))
    }
}

impl Drop for JsReservation {
    fn drop(&mut self) {
        let Some(inner) = Rc::get_mut(&mut self.inner) else {
            return;
        };
        if let Some((reservation, lifecycle)) = inner.get_mut().take() {
            lifecycle.invalidate();
            let callback_scope = CallbackErrorScope::suppress();
            drop(reservation);
            callback_scope.finish();
        }
    }
}

// ─── DryRunReport ────────────────────────────────────────────────────────────

/// Inert verdict returned by a pre-trade dry-run.
///
/// The report describes what a real pre-trade call would have produced without
/// spending rate-limit budget, applying holds, or latching account blocks.
#[wasm_bindgen(js_name = DryRunReport)]
#[derive(Clone)]
pub struct JsDryRunReport {
    inner: PreTradeDryRunReport,
}

#[wasm_bindgen(js_class = DryRunReport)]
impl JsDryRunReport {
    /// Whether the order would pass.
    #[wasm_bindgen(getter, js_name = isPass)]
    pub fn is_pass(&self) -> bool {
        self.inner.is_pass()
    }

    /// The rejects the order would collect, empty when it would pass.
    #[wasm_bindgen(getter, js_name = rejects)]
    pub fn rejects(&self) -> Vec<JsReject> {
        self.inner
            .rejects()
            .map(convert_rejects)
            .unwrap_or_default()
    }

    /// Returns the lock payload the main stage would produce.
    #[wasm_bindgen(js_name = lock)]
    pub fn lock(&self) -> JsLock {
        JsLock::from_inner(self.inner.lock().clone())
    }

    /// Returns the account adjustments the main stage would produce.
    #[wasm_bindgen(js_name = accountAdjustments)]
    pub fn account_adjustments(&self) -> Vec<JsAccountAdjustmentOutcome> {
        convert_outcomes(self.inner.account_adjustments())
    }

    /// The account block an account-scope reject would latch, or `undefined`.
    #[wasm_bindgen(getter, js_name = accountBlock)]
    pub fn account_block(&self) -> Option<JsAccountBlock> {
        self.inner.account_block().map(JsAccountBlock::from_core)
    }
}

impl JsDryRunReport {
    /// Wraps a core dry-run report.
    pub(crate) fn from_core(inner: PreTradeDryRunReport) -> Self {
        Self { inner }
    }
}

// ─── AccountAdjustmentBatchResult ────────────────────────────────────────────

/// Outcome of `engine.applyAccountAdjustment`.
///
/// On success `ok` is `true`, `failedIndex` is `undefined`, and `outcomes`
/// and `accountBlocks` hold the produced outcomes and blocks. On rejection
/// `ok` is `false`, `failedIndex` points at the failing adjustment, and
/// `rejects` is non-empty.
#[wasm_bindgen(js_name = AccountAdjustmentBatchResult)]
pub struct JsAccountAdjustmentBatchResult {
    failed_index: Option<usize>,
    rejects: Vec<JsReject>,
    outcomes: Vec<JsAccountAdjustmentOutcome>,
    account_blocks: Vec<JsAccountBlock>,
}

#[wasm_bindgen(js_class = AccountAdjustmentBatchResult)]
impl JsAccountAdjustmentBatchResult {
    /// Whether the batch was applied in full.
    #[wasm_bindgen(getter, js_name = ok)]
    pub fn ok(&self) -> bool {
        self.rejects.is_empty()
    }

    /// The index of the failing adjustment, or `undefined` on success.
    #[wasm_bindgen(getter, js_name = failedIndex)]
    pub fn failed_index(&self) -> Option<usize> {
        self.failed_index
    }

    /// The rejects, empty on success.
    #[wasm_bindgen(getter, js_name = rejects)]
    pub fn rejects(&self) -> Vec<JsReject> {
        self.rejects.clone()
    }

    /// The produced outcomes, empty on rejection.
    #[wasm_bindgen(getter, js_name = outcomes)]
    pub fn outcomes(&self) -> Vec<JsAccountAdjustmentOutcome> {
        self.outcomes.clone()
    }

    /// The account blocks reported after the accepted batch commits.
    #[wasm_bindgen(getter, js_name = accountBlocks)]
    pub fn account_blocks(&self) -> Vec<JsAccountBlock> {
        self.account_blocks.clone()
    }
}

impl JsAccountAdjustmentBatchResult {
    /// Builds a successful batch result from the core outcomes.
    pub(crate) fn accepted(result: &AccountAdjustmentBatchResult) -> Self {
        Self {
            failed_index: None,
            rejects: Vec::new(),
            outcomes: convert_outcomes(&result.outcomes),
            account_blocks: result
                .account_blocks
                .iter()
                .map(JsAccountBlock::from_core)
                .collect(),
        }
    }

    /// Builds a rejected batch result from the core batch error.
    pub(crate) fn rejected(error: &AccountAdjustmentBatchError) -> Self {
        Self {
            failed_index: Some(error.failed_adjustment_index),
            rejects: convert_rejects(&error.rejects),
            outcomes: Vec::new(),
            account_blocks: Vec::new(),
        }
    }
}

// ─── PolicyConfigurationResult ───────────────────────────────────────────────

/// Result of an accepted runtime policy configuration operation.
#[wasm_bindgen(js_name = PolicyConfigurationResult)]
#[derive(Clone)]
pub struct JsPolicyConfigurationResult {
    account_blocks: Vec<JsAccountBlock>,
}

#[wasm_bindgen(js_class = PolicyConfigurationResult)]
impl JsPolicyConfigurationResult {
    /// Blocks recorded by the engine before configuration returned.
    #[wasm_bindgen(getter, js_name = accountBlocks)]
    pub fn account_blocks(&self) -> Vec<JsAccountBlock> {
        self.account_blocks.clone()
    }

    /// Returns a deep copy of this result.
    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsPolicyConfigurationResult {
        self.clone()
    }
}

impl JsPolicyConfigurationResult {
    pub(crate) fn from_core(result: &openpit::PolicyConfigurationResult) -> Self {
        Self {
            account_blocks: result
                .account_blocks
                .iter()
                .map(JsAccountBlock::from_core)
                .collect(),
        }
    }
}

// ─── PostTradeResult ─────────────────────────────────────────────────────────

/// Result of `engine.applyExecutionReport`, also constructible for a custom
/// policy's post-trade return.
///
/// A non-empty `accountBlocks` means a kill switch fired. `accountPnls`
/// and `accountAdjustments` are already applied and must be consumed even when
/// a block is present.
#[wasm_bindgen(js_name = PostTradeResult)]
#[derive(Clone)]
pub struct JsPostTradeResult {
    account_blocks: Vec<JsAccountBlock>,
    account_pnls: Vec<JsAccountPnlOutcome>,
    account_adjustments: Vec<JsAccountAdjustmentOutcome>,
}

#[wasm_bindgen(js_class = PostTradeResult)]
impl JsPostTradeResult {
    /// Constructs a post-trade result from optional block, PnL, and adjustment
    /// outcome lists.
    ///
    /// All three default to empty. Used by a custom policy's
    /// `applyExecutionReport` return path.
    ///
    /// # Errors
    ///
    /// Throws `TypeError` when an array contains a value of the wrong type.
    #[wasm_bindgen(constructor)]
    pub fn new(
        #[wasm_bindgen(unchecked_optional_param_type = "readonly AccountBlock[]")]
        account_blocks: Option<Array>,
        #[wasm_bindgen(unchecked_optional_param_type = "readonly AccountPnlOutcome[]")]
        account_pnls: Option<Array>,
        #[wasm_bindgen(unchecked_optional_param_type = "readonly AccountAdjustmentOutcome[]")]
        account_adjustments: Option<Array>,
    ) -> Result<JsPostTradeResult, JsValue> {
        Ok(Self {
            account_blocks: clone_array(account_blocks, "AccountBlock")?,
            account_pnls: clone_array(account_pnls, "AccountPnlOutcome")?,
            account_adjustments: clone_array(account_adjustments, "AccountAdjustmentOutcome")?,
        })
    }

    /// The account blocks; non-empty when a kill switch fired.
    #[wasm_bindgen(getter, js_name = accountBlocks)]
    pub fn account_blocks(&self) -> Vec<JsAccountBlock> {
        self.account_blocks.clone()
    }

    /// Account-level PnL outcomes reported by policies.
    ///
    /// A computed outcome with a nonzero delta changed the ledger; a zero delta
    /// recomputed it unchanged. A halt-reason outcome has no authoritative PnL
    /// value and must not be interpreted as zero. SpotFunds emits a halt reason
    /// only when the current report transitions the account accumulator to
    /// halted; later reports omit the unchanged halt. Position force-sets do not
    /// re-arm it, and unchanged halted position PnL is likewise omitted from
    /// `accountAdjustments`.
    #[wasm_bindgen(getter, js_name = accountPnls)]
    pub fn account_pnls(&self) -> Vec<JsAccountPnlOutcome> {
        self.account_pnls.clone()
    }

    /// The already-applied account adjustments.
    #[wasm_bindgen(getter, js_name = accountAdjustments)]
    pub fn account_adjustments(&self) -> Vec<JsAccountAdjustmentOutcome> {
        self.account_adjustments.clone()
    }

    /// Returns a deep copy of this result.
    #[wasm_bindgen(js_name = clone)]
    pub fn js_clone(&self) -> JsPostTradeResult {
        self.clone()
    }
}

/// Clones typed wasm wrappers from an optional JavaScript array.
fn clone_array<T>(values: Option<Array>, expected: &str) -> Result<Vec<T>, JsValue>
where
    T: TryFromJsValue,
{
    let Some(values) = values else {
        return Ok(Vec::new());
    };
    if !Array::is_array(values.as_ref()) {
        return Err(make_error(
            ErrorKind::Type,
            "post-trade result inputs must be arrays",
            None,
        ));
    }
    let mut result = Vec::with_capacity(values.length() as usize);
    for value in values.iter() {
        let value = extract_cloned_wrapper::<T>(&value)?.ok_or_else(|| {
            make_error(
                ErrorKind::Type,
                &format!("post-trade result array must contain only {expected} values"),
                None,
            )
        })?;
        result.push(value);
    }
    Ok(result)
}

impl JsPostTradeResult {
    /// Builds a post-trade result from the core result.
    pub(crate) fn from_core(result: &PostTradeResult) -> Self {
        Self {
            account_blocks: result
                .account_blocks
                .iter()
                .map(JsAccountBlock::from_core)
                .collect(),
            account_pnls: result
                .account_pnls
                .iter()
                .map(JsAccountPnlOutcome::from_core)
                .collect(),
            account_adjustments: convert_outcomes(&result.account_adjustments),
        }
    }

    /// Builds the core post-trade result from this wrapper.
    ///
    /// # Errors
    ///
    /// Throws `ParamError` when a stored block code is no longer recognized.
    pub(crate) fn to_core(&self) -> Result<PostTradeResult, JsValue> {
        let account_blocks = self
            .account_blocks
            .iter()
            .map(JsAccountBlock::to_core)
            .collect::<Result<Vec<_>, _>>()?;
        let account_adjustments = self
            .account_adjustments
            .iter()
            .map(JsAccountAdjustmentOutcome::to_core)
            .collect::<Result<Vec<_>, _>>()?;
        let account_pnls = self
            .account_pnls
            .iter()
            .map(JsAccountPnlOutcome::to_core)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(PostTradeResult {
            account_blocks,
            account_pnls,
            account_adjustments,
        })
    }
}
