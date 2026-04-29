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

#![allow(unexpected_cfgs)]
#![allow(clippy::useless_conversion)]

use std::cell::{Cell, RefCell};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::thread_local;
use std::time::Duration;

use openpit::param::{
    AccountId, AdjustmentAmount, Asset, CashFlow, Fee, Leverage, Notional, Pnl, PositionEffect,
    PositionMode, PositionSide, PositionSize, Price, Quantity, RoundingStrategy, Side, Trade,
    TradeAmount, Volume,
};
use openpit::pretrade::policies::OrderValidationPolicy;
use openpit::pretrade::policies::PnlBoundsBarrier;
use openpit::pretrade::policies::PnlBoundsKillSwitchPolicy;
use openpit::pretrade::policies::RateLimitPolicy;
use openpit::pretrade::policies::{OrderSizeLimit, OrderSizeLimitPolicy};
use openpit::pretrade::{
    CheckPreTradeStartPolicy, PreTradeContext, PreTradeLock, PreTradePolicy, PreTradeRequest,
    PreTradeReservation, Reject, RejectCode, RejectScope, Rejects,
};
use openpit::{
    AccountAdjustmentBalanceOperation, AccountAdjustmentPositionOperation, Engine,
    EngineBuildError, HasAccountAdjustmentBalanceAverageEntryPrice, HasAccountAdjustmentPending,
    HasAccountAdjustmentPendingLowerBound, HasAccountAdjustmentPendingUpperBound,
    HasAccountAdjustmentPositionLeverage, HasAccountAdjustmentReserved,
    HasAccountAdjustmentReservedLowerBound, HasAccountAdjustmentReservedUpperBound,
    HasAccountAdjustmentTotal, HasAccountAdjustmentTotalLowerBound,
    HasAccountAdjustmentTotalUpperBound, HasAccountId, HasAutoBorrow, HasAverageEntryPrice,
    HasBalanceAsset, HasClosePosition, HasCollateralAsset, HasExecutionReportIsFinal,
    HasExecutionReportLastTrade, HasExecutionReportPositionEffect, HasExecutionReportPositionSide,
    HasFee, HasInstrument, HasOrderCollateralAsset, HasOrderLeverage, HasOrderPositionSide,
    HasOrderPrice, HasPnl, HasPositionInstrument, HasPositionMode, HasReduceOnly, HasSide,
    HasTradeAmount, Instrument, Mutation, Mutations, PostTradeResult, RequestFieldAccessError,
};
use openpit::{AccountAdjustmentContext, AccountAdjustmentPolicy};
use pit_interop::{
    AccountAdjustmentAmountAccess, AccountAdjustmentBoundsAccess, AccountAdjustmentOperationAccess,
    ExecutionReportFillAccess, ExecutionReportOperationAccess, ExecutionReportPositionImpactAccess,
    FinancialImpactAccess, OrderMarginAccess, OrderOperationAccess, OrderPositionAccess,
    PopulatedAccountAdjustmentOperation, PopulatedExecutionReportFill,
    PopulatedExecutionReportOperation, PopulatedExecutionReportPositionImpact,
    PopulatedFinancialImpact, PopulatedOrderMargin, PopulatedOrderOperation,
    PopulatedOrderPosition,
};
use pyo3::basic::CompareOp;
use pyo3::create_exception;
use pyo3::exceptions::{PyException, PyRuntimeError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyInt, PyModule};
use rust_decimal::prelude::ToPrimitive;

create_exception!(openpit, RejectError, PyException);
create_exception!(openpit, ParamError, PyValueError);

thread_local! {
    static PY_CALLBACK_ERROR: RefCell<Option<PyErr>> = const { RefCell::new(None) };
}

fn set_python_callback_error(error: PyErr) {
    PY_CALLBACK_ERROR.with(|slot| {
        slot.borrow_mut().replace(error);
    });
}

fn take_python_callback_error() -> Option<PyErr> {
    PY_CALLBACK_ERROR.with(|slot| slot.borrow_mut().take())
}

fn clear_python_callback_error() {
    PY_CALLBACK_ERROR.with(|slot| {
        slot.borrow_mut().take();
    });
}

fn create_param_error(message: impl Into<String>) -> PyErr {
    ParamError::new_err(message.into())
}

struct Order {
    operation: OrderOperationAccess,
    position: OrderPositionAccess,
    margin: OrderMarginAccess,
    original: Py<PyAny>,
}

impl Order {
    fn original(&self, py: Python<'_>) -> Py<PyAny> {
        self.original.clone_ref(py)
    }
}

impl HasInstrument for Order {
    fn instrument(&self) -> Result<&Instrument, RequestFieldAccessError> {
        self.operation.instrument()
    }
}

impl HasSide for Order {
    fn side(&self) -> Result<Side, RequestFieldAccessError> {
        self.operation.side()
    }
}

impl HasAccountId for Order {
    fn account_id(&self) -> Result<AccountId, RequestFieldAccessError> {
        self.operation.account_id()
    }
}

impl HasTradeAmount for Order {
    fn trade_amount(&self) -> Result<TradeAmount, RequestFieldAccessError> {
        self.operation.trade_amount()
    }
}

impl HasOrderPrice for Order {
    fn price(&self) -> Result<Option<Price>, RequestFieldAccessError> {
        self.operation.price()
    }
}

impl HasOrderPositionSide for Order {
    fn position_side(&self) -> Result<Option<PositionSide>, RequestFieldAccessError> {
        self.position.position_side()
    }
}

impl HasReduceOnly for Order {
    fn reduce_only(&self) -> Result<bool, RequestFieldAccessError> {
        self.position.reduce_only()
    }
}

impl HasClosePosition for Order {
    fn close_position(&self) -> Result<bool, RequestFieldAccessError> {
        self.position.close_position()
    }
}

impl HasOrderLeverage for Order {
    fn leverage(&self) -> Result<Option<Leverage>, RequestFieldAccessError> {
        self.margin.leverage()
    }
}

impl HasOrderCollateralAsset for Order {
    fn collateral_asset(&self) -> Result<Option<&Asset>, RequestFieldAccessError> {
        self.margin.collateral_asset()
    }
}

impl HasAutoBorrow for Order {
    fn auto_borrow(&self) -> Result<bool, RequestFieldAccessError> {
        self.margin.auto_borrow()
    }
}

struct ExecutionReport {
    operation: ExecutionReportOperationAccess,
    financial_impact: FinancialImpactAccess,
    fill: ExecutionReportFillAccess,
    position_impact: ExecutionReportPositionImpactAccess,
    original: Py<PyAny>,
}

impl ExecutionReport {
    fn original(&self, py: Python<'_>) -> Py<PyAny> {
        self.original.clone_ref(py)
    }
}

impl HasInstrument for ExecutionReport {
    fn instrument(&self) -> Result<&Instrument, RequestFieldAccessError> {
        self.operation.instrument()
    }
}

impl HasSide for ExecutionReport {
    fn side(&self) -> Result<Side, RequestFieldAccessError> {
        self.operation.side()
    }
}

impl HasAccountId for ExecutionReport {
    fn account_id(&self) -> Result<AccountId, RequestFieldAccessError> {
        self.operation.account_id()
    }
}

impl HasPnl for ExecutionReport {
    fn pnl(&self) -> Result<Pnl, RequestFieldAccessError> {
        self.financial_impact.pnl()
    }
}

impl HasFee for ExecutionReport {
    fn fee(&self) -> Result<Fee, RequestFieldAccessError> {
        self.financial_impact.fee()
    }
}

impl HasExecutionReportLastTrade for ExecutionReport {
    fn last_trade(&self) -> Result<Option<Trade>, RequestFieldAccessError> {
        self.fill.last_trade()
    }
}

impl HasExecutionReportIsFinal for ExecutionReport {
    fn is_final(&self) -> Result<bool, RequestFieldAccessError> {
        self.fill.is_final()
    }
}

impl HasExecutionReportPositionEffect for ExecutionReport {
    fn position_effect(&self) -> Result<Option<PositionEffect>, RequestFieldAccessError> {
        self.position_impact.position_effect()
    }
}

impl HasExecutionReportPositionSide for ExecutionReport {
    fn position_side(&self) -> Result<Option<PositionSide>, RequestFieldAccessError> {
        self.position_impact.position_side()
    }
}

struct AccountAdjustment {
    operation: AccountAdjustmentOperationAccess,
    amount: AccountAdjustmentAmountAccess,
    bounds: AccountAdjustmentBoundsAccess,
    original: Py<PyAny>,
}

impl AccountAdjustment {
    fn original(&self, py: Python<'_>) -> Py<PyAny> {
        self.original.clone_ref(py)
    }
}

impl HasBalanceAsset for AccountAdjustment {
    fn balance_asset(&self) -> Result<&Asset, RequestFieldAccessError> {
        self.operation.balance_asset()
    }
}

impl HasAccountAdjustmentBalanceAverageEntryPrice for AccountAdjustment {
    fn balance_average_entry_price(&self) -> Result<Option<Price>, RequestFieldAccessError> {
        self.operation.balance_average_entry_price()
    }
}

impl HasPositionInstrument for AccountAdjustment {
    fn position_instrument(&self) -> Result<&Instrument, RequestFieldAccessError> {
        self.operation.position_instrument()
    }
}

impl HasCollateralAsset for AccountAdjustment {
    fn collateral_asset(&self) -> Result<&Asset, RequestFieldAccessError> {
        self.operation.collateral_asset()
    }
}

impl HasAverageEntryPrice for AccountAdjustment {
    fn average_entry_price(&self) -> Result<Price, RequestFieldAccessError> {
        self.operation.average_entry_price()
    }
}

impl HasPositionMode for AccountAdjustment {
    fn position_mode(&self) -> Result<PositionMode, RequestFieldAccessError> {
        self.operation.position_mode()
    }
}

impl HasAccountAdjustmentPositionLeverage for AccountAdjustment {
    fn position_leverage(&self) -> Result<Option<Leverage>, RequestFieldAccessError> {
        self.operation.position_leverage()
    }
}

impl HasAccountAdjustmentTotal for AccountAdjustment {
    fn total(&self) -> Result<Option<AdjustmentAmount>, RequestFieldAccessError> {
        self.amount.total()
    }
}

impl HasAccountAdjustmentReserved for AccountAdjustment {
    fn reserved(&self) -> Result<Option<AdjustmentAmount>, RequestFieldAccessError> {
        self.amount.reserved()
    }
}

impl HasAccountAdjustmentPending for AccountAdjustment {
    fn pending(&self) -> Result<Option<AdjustmentAmount>, RequestFieldAccessError> {
        self.amount.pending()
    }
}

impl HasAccountAdjustmentTotalUpperBound for AccountAdjustment {
    fn total_upper(&self) -> Result<Option<PositionSize>, RequestFieldAccessError> {
        self.bounds.total_upper()
    }
}

impl HasAccountAdjustmentTotalLowerBound for AccountAdjustment {
    fn total_lower(&self) -> Result<Option<PositionSize>, RequestFieldAccessError> {
        self.bounds.total_lower()
    }
}

impl HasAccountAdjustmentReservedUpperBound for AccountAdjustment {
    fn reserved_upper(&self) -> Result<Option<PositionSize>, RequestFieldAccessError> {
        self.bounds.reserved_upper()
    }
}

impl HasAccountAdjustmentReservedLowerBound for AccountAdjustment {
    fn reserved_lower(&self) -> Result<Option<PositionSize>, RequestFieldAccessError> {
        self.bounds.reserved_lower()
    }
}

impl HasAccountAdjustmentPendingUpperBound for AccountAdjustment {
    fn pending_upper(&self) -> Result<Option<PositionSize>, RequestFieldAccessError> {
        self.bounds.pending_upper()
    }
}

impl HasAccountAdjustmentPendingLowerBound for AccountAdjustment {
    fn pending_lower(&self) -> Result<Option<PositionSize>, RequestFieldAccessError> {
        self.bounds.pending_lower()
    }
}

#[pyclass(name = "Engine", module = "openpit", unsendable)]
struct PyEngine {
    inner: Engine<Order, ExecutionReport, AccountAdjustment>,
}

#[pymethods]
impl PyEngine {
    #[staticmethod]
    fn builder() -> PyEngineBuilder {
        PyEngineBuilder {
            start_policies: RefCell::new(Vec::new()),
            main_policies: RefCell::new(Vec::new()),
            adjustment_policies: RefCell::new(Vec::new()),
        }
    }

    #[pyo3(signature = (order))]
    fn start_pre_trade(
        &self,
        py: Python<'_>,
        order: Bound<'_, PyAny>,
    ) -> PyResult<PyStartPreTradeResult> {
        clear_python_callback_error();
        let order = extract_python_order(&order)?;
        match self.inner.start_pre_trade(order) {
            Ok(request) => {
                if let Some(error) = take_python_callback_error() {
                    return Err(error);
                }

                Ok(PyStartPreTradeResult {
                    request: Some(Py::new(
                        py,
                        PyRequest {
                            inner: RefCell::new(Some(request)),
                        },
                    )?),
                    rejects: Vec::new(),
                })
            }
            Err(rejects) => {
                if let Some(error) = take_python_callback_error() {
                    return Err(error);
                }

                Ok(PyStartPreTradeResult {
                    request: None,
                    rejects: rejects.iter().map(convert_reject).collect(),
                })
            }
        }
    }

    #[pyo3(signature = (order))]
    fn execute_pre_trade(
        &self,
        py: Python<'_>,
        order: Bound<'_, PyAny>,
    ) -> PyResult<PyExecuteResult> {
        clear_python_callback_error();
        let order = extract_python_order(&order)?;
        match self.inner.execute_pre_trade(order) {
            Ok(reservation) => {
                if let Some(error) = take_python_callback_error() {
                    return Err(error);
                }
                Ok(PyExecuteResult {
                    reservation: Some(Py::new(
                        py,
                        PyReservation {
                            inner: RefCell::new(Some(reservation)),
                        },
                    )?),
                    rejects: Vec::new(),
                })
            }
            Err(rejects) => {
                if let Some(error) = take_python_callback_error() {
                    return Err(error);
                }
                Ok(PyExecuteResult {
                    reservation: None,
                    rejects: rejects.iter().map(convert_reject).collect(),
                })
            }
        }
    }

    #[pyo3(signature = (report))]
    fn apply_execution_report(&self, report: &Bound<'_, PyAny>) -> PyResult<PyPostTradeResult> {
        clear_python_callback_error();
        let report = extract_python_execution_report(report)?;
        let result = PyPostTradeResult {
            inner: self.inner.apply_execution_report(&report),
        };
        if let Some(error) = take_python_callback_error() {
            return Err(error);
        }
        Ok(result)
    }

    #[pyo3(signature = (account_id, adjustments))]
    fn apply_account_adjustment(
        &self,
        account_id: &Bound<'_, PyAny>,
        adjustments: &Bound<'_, PyAny>,
    ) -> PyResult<PyAccountAdjustmentBatchResult> {
        clear_python_callback_error();

        let account_id = parse_account_id_input(account_id)?;
        let batch = adjustments
            .iter()?
            .map(|item| extract_python_account_adjustment(&item?))
            .collect::<PyResult<Vec<_>>>()?;

        match self.inner.apply_account_adjustment(account_id, &batch) {
            Ok(()) => {
                if let Some(error) = take_python_callback_error() {
                    return Err(error);
                }
                Ok(PyAccountAdjustmentBatchResult {
                    failed_index: None,
                    rejects: Vec::new(),
                })
            }
            Err(error) => {
                if let Some(py_error) = take_python_callback_error() {
                    return Err(py_error);
                }
                let mut rejects = Vec::with_capacity(error.rejects.len());
                rejects.extend(error.rejects.iter().map(convert_reject));
                Ok(PyAccountAdjustmentBatchResult {
                    failed_index: Some(error.failed_adjustment_index),
                    rejects,
                })
            }
        }
    }
}

#[pyclass(name = "Reject", module = "openpit.pretrade")]
#[derive(Clone, Debug)]
struct PyReject {
    code: String,
    reason: String,
    details: String,
    policy: String,
    scope: String,
    user_data: u64,
}

#[pymethods]
impl PyReject {
    #[getter]
    fn code(&self) -> String {
        self.code.clone()
    }

    #[getter]
    fn reason(&self) -> String {
        self.reason.clone()
    }

    #[getter]
    fn details(&self) -> String {
        self.details.clone()
    }

    #[getter]
    fn policy(&self) -> String {
        self.policy.clone()
    }

    #[getter]
    fn scope(&self) -> String {
        self.scope.clone()
    }

    #[getter]
    fn user_data(&self) -> u64 {
        self.user_data
    }

    fn __repr__(&self) -> String {
        format!(
            "Reject(code={:?}, reason={:?}, details={:?}, policy={:?}, scope={:?}, user_data={})",
            self.code, self.reason, self.details, self.policy, self.scope, self.user_data
        )
    }
}

#[pyclass(name = "StartPreTradeResult", module = "openpit.pretrade", unsendable)]
struct PyStartPreTradeResult {
    request: Option<Py<PyRequest>>,
    rejects: Vec<PyReject>,
}

#[pymethods]
impl PyStartPreTradeResult {
    #[getter]
    fn ok(&self) -> bool {
        self.rejects.is_empty()
    }

    #[getter]
    fn request(&self, py: Python<'_>) -> Option<Py<PyRequest>> {
        self.request.as_ref().map(|request| request.clone_ref(py))
    }

    #[getter]
    fn rejects(&self) -> Vec<PyReject> {
        self.rejects.clone()
    }

    fn __bool__(&self) -> bool {
        self.ok()
    }

    fn __repr__(&self) -> String {
        if self.rejects.is_empty() {
            "StartPreTradeResult(ok=True)".to_owned()
        } else {
            format!(
                "StartPreTradeResult(ok=False, rejects={})",
                self.rejects.len()
            )
        }
    }
}

#[pyclass(name = "ExecuteResult", module = "openpit.pretrade", unsendable)]
struct PyExecuteResult {
    reservation: Option<Py<PyReservation>>,
    rejects: Vec<PyReject>,
}

#[pymethods]
impl PyExecuteResult {
    #[getter]
    fn ok(&self) -> bool {
        self.rejects.is_empty()
    }

    #[getter]
    fn reservation(&self, py: Python<'_>) -> Option<Py<PyReservation>> {
        self.reservation
            .as_ref()
            .map(|reservation| reservation.clone_ref(py))
    }

    #[getter]
    fn rejects(&self) -> Vec<PyReject> {
        self.rejects.clone()
    }

    fn __bool__(&self) -> bool {
        self.ok()
    }

    fn __repr__(&self) -> String {
        if self.ok() {
            "ExecuteResult(ok=True)".to_owned()
        } else {
            format!("ExecuteResult(ok=False, rejects={})", self.rejects.len())
        }
    }
}

#[pyclass(
    name = "AccountAdjustmentBatchResult",
    module = "openpit.pretrade",
    unsendable
)]
struct PyAccountAdjustmentBatchResult {
    failed_index: Option<usize>,
    rejects: Vec<PyReject>,
}

#[pymethods]
impl PyAccountAdjustmentBatchResult {
    #[getter]
    fn ok(&self) -> bool {
        self.rejects.is_empty()
    }

    #[getter]
    fn failed_index(&self) -> Option<usize> {
        self.failed_index
    }

    #[getter]
    fn rejects(&self) -> Vec<PyReject> {
        self.rejects.clone()
    }

    fn __bool__(&self) -> bool {
        self.ok()
    }

    fn __repr__(&self) -> String {
        match self.failed_index {
            Some(index) => format!(
                "AccountAdjustmentBatchResult(ok=False, failed_index={index}, rejects={})",
                self.rejects.len()
            ),
            _ => "AccountAdjustmentBatchResult(ok=True)".to_owned(),
        }
    }
}

#[derive(Clone)]
struct OrderSizeLimitConfig {
    settlement_asset: String,
    max_quantity: String,
    max_notional: String,
}

#[pyclass(name = "PreTradeContext", module = "openpit.pretrade", frozen)]
struct PyPreTradeContext;

#[pyclass(name = "AccountAdjustmentContext", module = "openpit", frozen)]
struct PyAccountAdjustmentContext;

impl From<&PreTradeContext> for PyPreTradeContext {
    fn from(_: &PreTradeContext) -> Self {
        Self
    }
}

impl From<&AccountAdjustmentContext> for PyAccountAdjustmentContext {
    fn from(_: &AccountAdjustmentContext) -> Self {
        Self
    }
}

struct SharedStartPolicy<P> {
    policy: Rc<P>,
}

struct BoxedStartPolicy {
    inner: Box<dyn CheckPreTradeStartPolicy<Order, ExecutionReport>>,
}

struct BoxedMainPolicy {
    inner: Box<dyn PreTradePolicy<Order, ExecutionReport>>,
}

struct BoxedAccountAdjustmentPolicy {
    inner: Box<dyn AccountAdjustmentPolicy<AccountAdjustment>>,
}

impl<P> CheckPreTradeStartPolicy<Order, ExecutionReport> for SharedStartPolicy<P>
where
    P: CheckPreTradeStartPolicy<Order, ExecutionReport>,
{
    fn name(&self) -> &str {
        self.policy.name()
    }

    fn check_pre_trade_start(&self, ctx: &PreTradeContext, order: &Order) -> Result<(), Rejects> {
        self.policy.check_pre_trade_start(ctx, order)
    }

    fn apply_execution_report(&self, report: &ExecutionReport) -> bool {
        self.policy.apply_execution_report(report)
    }
}

impl CheckPreTradeStartPolicy<Order, ExecutionReport> for BoxedStartPolicy {
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

impl PreTradePolicy<Order, ExecutionReport> for BoxedMainPolicy {
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

impl AccountAdjustmentPolicy<AccountAdjustment> for BoxedAccountAdjustmentPolicy {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn apply_account_adjustment(
        &self,
        ctx: &AccountAdjustmentContext,
        account_id: AccountId,
        adjustment: &AccountAdjustment,
        mutations: &mut Mutations,
    ) -> Result<(), Rejects> {
        self.inner
            .apply_account_adjustment(ctx, account_id, adjustment, mutations)
    }
}

struct PythonStartPolicyAdapter {
    name: String,
    policy: Py<PyAny>,
}

struct PythonMainPolicyAdapter {
    name: String,
    policy: Py<PyAny>,
}

struct PythonAccountAdjustmentPolicyAdapter {
    name: String,
    policy: Py<PyAny>,
}

impl CheckPreTradeStartPolicy<Order, ExecutionReport> for PythonStartPolicyAdapter {
    fn name(&self) -> &str {
        &self.name
    }

    fn check_pre_trade_start(&self, ctx: &PreTradeContext, order: &Order) -> Result<(), Rejects> {
        Python::with_gil(|py| {
            let policy_ctx = Py::new(py, PyPreTradeContext::from(ctx)).map_err(|error| {
                set_python_callback_error(error);
                python_callback_rejects(&self.name)
            })?;
            let result = self
                .policy
                .bind(py)
                .call_method1("check_pre_trade_start", (policy_ctx, order.original(py)))
                .map_err(|error| {
                    set_python_callback_error(error);
                    python_callback_rejects(&self.name)
                })?;

            let rejects = parse_policy_rejects(&result, &self.name).map_err(|error| {
                set_python_callback_error(error);
                python_callback_rejects(&self.name)
            })?;
            if rejects.is_empty() {
                Ok(())
            } else {
                Err(Rejects::from(rejects))
            }
        })
    }

    fn apply_execution_report(&self, report: &ExecutionReport) -> bool {
        Python::with_gil(|py| {
            let kwargs = PyDict::new_bound(py);
            if let Err(error) = kwargs.set_item("report", report.original(py)) {
                set_python_callback_error(error);
                return false;
            }

            let result =
                match self
                    .policy
                    .bind(py)
                    .call_method("apply_execution_report", (), Some(&kwargs))
                {
                    Ok(result) => result,
                    Err(error) => {
                        set_python_callback_error(error);
                        return false;
                    }
                };

            match result.extract::<bool>() {
                Ok(value) => value,
                Err(error) => {
                    set_python_callback_error(error);
                    false
                }
            }
        })
    }
}

impl PreTradePolicy<Order, ExecutionReport> for PythonMainPolicyAdapter {
    fn name(&self) -> &str {
        &self.name
    }

    fn perform_pre_trade_check(
        &self,
        ctx: &PreTradeContext,
        order: &Order,
        mutations: &mut Mutations,
    ) -> Result<(), Rejects> {
        Python::with_gil(|py| {
            let policy_ctx = Py::new(py, PyPreTradeContext::from(ctx)).map_err(|error| {
                set_python_callback_error(error);
                python_callback_rejects(&self.name)
            })?;

            let decision = self
                .policy
                .bind(py)
                .call_method1("perform_pre_trade_check", (policy_ctx, order.original(py)))
                .map_err(|error| {
                    set_python_callback_error(error);
                    python_callback_rejects(&self.name)
                })?;

            let mut rejects = Vec::new();
            if let Err(error) = apply_policy_decision(&self.name, decision, mutations, &mut rejects)
            {
                set_python_callback_error(error);
                return Err(python_callback_rejects(&self.name));
            }
            if rejects.is_empty() {
                Ok(())
            } else {
                Err(Rejects::from(rejects))
            }
        })
    }

    fn apply_execution_report(&self, report: &ExecutionReport) -> bool {
        Python::with_gil(|py| {
            let kwargs = PyDict::new_bound(py);
            if let Err(error) = kwargs.set_item("report", report.original(py)) {
                set_python_callback_error(error);
                return false;
            }

            let result =
                match self
                    .policy
                    .bind(py)
                    .call_method("apply_execution_report", (), Some(&kwargs))
                {
                    Ok(result) => result,
                    Err(error) => {
                        set_python_callback_error(error);
                        return false;
                    }
                };

            match result.extract::<bool>() {
                Ok(value) => value,
                Err(error) => {
                    set_python_callback_error(error);
                    false
                }
            }
        })
    }
}

impl AccountAdjustmentPolicy<AccountAdjustment> for PythonAccountAdjustmentPolicyAdapter {
    fn name(&self) -> &str {
        &self.name
    }

    fn apply_account_adjustment(
        &self,
        ctx: &AccountAdjustmentContext,
        account_id: AccountId,
        adjustment: &AccountAdjustment,
        mutations: &mut Mutations,
    ) -> Result<(), Rejects> {
        Python::with_gil(|py| {
            let adjustment_ctx =
                Py::new(py, PyAccountAdjustmentContext::from(ctx)).map_err(|error| {
                    set_python_callback_error(error);
                    python_callback_rejects(&self.name)
                })?;
            let py_account_id =
                Py::new(py, PyAccountId { inner: account_id }).map_err(|error| {
                    set_python_callback_error(error);
                    python_callback_rejects(&self.name)
                })?;
            let result = self
                .policy
                .bind(py)
                .call_method1(
                    "apply_account_adjustment",
                    (adjustment_ctx, py_account_id, adjustment.original(py)),
                )
                .map_err(|error| {
                    set_python_callback_error(error);
                    python_callback_rejects(&self.name)
                })?;

            // None -> pass without mutations.
            if result.is_none() {
                return Ok(());
            }

            if result.hasattr("rejects").map_err(|error| {
                set_python_callback_error(error);
                python_callback_rejects(&self.name)
            })? && result.hasattr("mutations").map_err(|error| {
                set_python_callback_error(error);
                python_callback_rejects(&self.name)
            })? {
                let mut rejects = Vec::new();
                if let Err(error) =
                    apply_policy_decision(&self.name, result, mutations, &mut rejects)
                {
                    set_python_callback_error(error);
                    return Err(python_callback_rejects(&self.name));
                }
                return if rejects.is_empty() {
                    Ok(())
                } else {
                    Err(Rejects::from(rejects))
                };
            }

            // Backward-compat mode: accept either a single reject, an iterable
            // of rejects, or an iterable of mutations.
            if result.hasattr("code").map_err(|error| {
                set_python_callback_error(error);
                python_callback_rejects(&self.name)
            })? {
                let reject = parse_policy_reject(&result, &self.name).map_err(|error| {
                    set_python_callback_error(error);
                    python_callback_rejects(&self.name)
                })?;
                return Err(Rejects::from(reject));
            }

            let mut rejects = Vec::new();
            let iter = result.iter().map_err(|error| {
                set_python_callback_error(error);
                python_callback_rejects(&self.name)
            })?;
            for item in iter {
                let item = item.map_err(|error| {
                    set_python_callback_error(error);
                    python_callback_rejects(&self.name)
                })?;
                if item.hasattr("code").map_err(|error| {
                    set_python_callback_error(error);
                    python_callback_rejects(&self.name)
                })? {
                    let reject = parse_policy_reject(&item, &self.name).map_err(|error| {
                        set_python_callback_error(error);
                        python_callback_rejects(&self.name)
                    })?;
                    rejects.push(reject);
                    continue;
                }
                let mutation = parse_policy_mutation(&item).map_err(|error| {
                    set_python_callback_error(error);
                    python_callback_rejects(&self.name)
                })?;
                mutations.push(mutation);
            }
            if rejects.is_empty() {
                Ok(())
            } else {
                Err(Rejects::from(rejects))
            }
        })
    }
}

fn ensure_callable_method(policy: &Bound<'_, PyAny>, method: &str) -> PyResult<()> {
    let callable = policy.getattr(method)?;
    if !callable.is_callable() {
        return Err(PyTypeError::new_err(format!(
            "policy.{method} must be callable"
        )));
    }
    Ok(())
}

fn python_callback_reject(policy_name: &str) -> Reject {
    Reject::new(
        policy_name,
        RejectScope::Order,
        RejectCode::SystemUnavailable,
        "python policy callback failed",
        "python policy callback raised an exception",
    )
}

fn python_callback_rejects(policy_name: &str) -> Rejects {
    Rejects::from(python_callback_reject(policy_name))
}

fn extract_python_order(obj: &Bound<'_, PyAny>) -> PyResult<Order> {
    let py = obj.py();
    // Aggregate wrapper/type contract is enforced on Python constructors.
    // Rust still validates entry-point object kind here because engine APIs can
    // receive arbitrary Python objects.
    let order = obj
        .extract::<PyRef<'_, PyOrder>>()
        .map_err(|_| PyTypeError::new_err("order must inherit from openpit.Order"))?;

    let operation = match order.operation.as_ref() {
        None => OrderOperationAccess::Absent,
        Some(py_operation) => {
            let op = py_operation.bind(py).borrow();
            let instrument = match (&op.underlying_asset, &op.settlement_asset) {
                (Some(underlying_asset), Some(settlement_asset)) => Some(Instrument::new(
                    underlying_asset.clone(),
                    settlement_asset.clone(),
                )),
                _ => None,
            };
            OrderOperationAccess::Populated(PopulatedOrderOperation {
                instrument,
                account_id: op.account_id,
                side: op.side,
                trade_amount: op.trade_amount,
                price: op.price,
            })
        }
    };

    let position = match order.position.as_ref() {
        None => OrderPositionAccess::Absent,
        Some(py_position) => {
            let pos = py_position.bind(py).borrow();
            OrderPositionAccess::Populated(PopulatedOrderPosition {
                position_side: pos.position_side,
                reduce_only: pos.reduce_only,
                close_position: pos.close_position,
            })
        }
    };

    let margin = match order.margin.as_ref() {
        None => OrderMarginAccess::Absent,
        Some(py_margin) => {
            let m = py_margin.bind(py).borrow();
            OrderMarginAccess::Populated(PopulatedOrderMargin {
                leverage: m.leverage,
                collateral_asset: m.collateral_asset.clone(),
                auto_borrow: m.auto_borrow,
            })
        }
    };

    Ok(Order {
        operation,
        position,
        margin,
        original: obj.clone().unbind(),
    })
}

fn extract_python_execution_report(obj: &Bound<'_, PyAny>) -> PyResult<ExecutionReport> {
    let py = obj.py();
    // Aggregate wrapper/type contract is enforced on Python constructors.
    // Rust still validates entry-point object kind here because engine APIs can
    // receive arbitrary Python objects.
    let report = obj
        .extract::<PyRef<'_, PyExecutionReport>>()
        .map_err(|_| PyTypeError::new_err("report must inherit from openpit.ExecutionReport"))?;

    let operation = match report.operation.as_ref() {
        None => ExecutionReportOperationAccess::Absent,
        Some(py_operation) => {
            let op = py_operation.bind(py).borrow();
            let instrument = match (&op.underlying_asset, &op.settlement_asset) {
                (Some(underlying_asset), Some(settlement_asset)) => Some(Instrument::new(
                    underlying_asset.clone(),
                    settlement_asset.clone(),
                )),
                _ => None,
            };
            ExecutionReportOperationAccess::Populated(PopulatedExecutionReportOperation {
                instrument,
                account_id: op.account_id,
                side: op.side,
            })
        }
    };

    let financial_impact = match report.financial_impact.as_ref() {
        None => FinancialImpactAccess::Absent,
        Some(py_fi) => {
            let fi = py_fi.bind(py).borrow();
            FinancialImpactAccess::Populated(PopulatedFinancialImpact {
                pnl: Some(fi.pnl),
                fee: Some(fi.fee),
            })
        }
    };

    let fill = match report.fill.as_ref() {
        None => ExecutionReportFillAccess::Absent,
        Some(py_fill) => {
            let f = py_fill.bind(py).borrow();
            ExecutionReportFillAccess::Populated(PopulatedExecutionReportFill {
                last_trade: f.last_trade,
                leaves_quantity: f.leaves_quantity,
                lock: f.lock,
                is_final: f.is_final,
            })
        }
    };

    let position_impact = match report.position_impact.as_ref() {
        None => ExecutionReportPositionImpactAccess::Absent,
        Some(py_pi) => {
            let pi = py_pi.bind(py).borrow();
            ExecutionReportPositionImpactAccess::Populated(PopulatedExecutionReportPositionImpact {
                position_effect: pi.position_effect,
                position_side: pi.position_side,
            })
        }
    };

    Ok(ExecutionReport {
        operation,
        financial_impact,
        fill,
        position_impact,
        original: obj.clone().unbind(),
    })
}

fn extract_python_account_adjustment(obj: &Bound<'_, PyAny>) -> PyResult<AccountAdjustment> {
    let py = obj.py();
    // Aggregate wrapper/type contract is enforced on Python constructors.
    // Rust still validates entry-point object kind here because engine APIs can
    // receive arbitrary Python objects.
    let adjustment = obj
        .extract::<PyRef<'_, PyAccountAdjustment>>()
        .map_err(|_| {
            PyTypeError::new_err("adjustment must inherit from openpit.AccountAdjustment")
        })?;

    let operation = match adjustment.operation.as_ref() {
        None => AccountAdjustmentOperationAccess::Absent,
        Some(py_operation) => {
            let populated = match py_operation {
                PyAccountAdjustmentOperation::Balance(py_balance_operation) => {
                    let operation = py_balance_operation.bind(py).borrow();
                    let asset = operation.asset.clone().ok_or_else(|| {
                        PyValueError::new_err("account adjustment balance operation requires asset")
                    })?;
                    PopulatedAccountAdjustmentOperation::Balance(
                        AccountAdjustmentBalanceOperation {
                            asset,
                            average_entry_price: operation.average_entry_price,
                        },
                    )
                }
                PyAccountAdjustmentOperation::Position(py_position_operation) => {
                    let operation = py_position_operation.bind(py).borrow();
                    let instrument = match (
                        &operation.underlying_asset,
                        &operation.settlement_asset,
                    ) {
                        (Some(underlying_asset), Some(settlement_asset)) => {
                            Instrument::new(underlying_asset.clone(), settlement_asset.clone())
                        }
                        _ => {
                            return Err(PyValueError::new_err(
                                    "account adjustment position operation requires underlying_asset and settlement_asset",
                                ));
                        }
                    };
                    let collateral_asset = operation.collateral_asset.clone().ok_or_else(|| {
                        PyValueError::new_err(
                            "account adjustment position operation requires collateral_asset",
                        )
                    })?;
                    let average_entry_price = operation.average_entry_price.ok_or_else(|| {
                        PyValueError::new_err(
                            "account adjustment position operation requires average_entry_price",
                        )
                    })?;
                    let mode = operation.mode.ok_or_else(|| {
                        PyValueError::new_err("account adjustment position operation requires mode")
                    })?;
                    PopulatedAccountAdjustmentOperation::Position(
                        AccountAdjustmentPositionOperation {
                            instrument,
                            collateral_asset,
                            average_entry_price,
                            mode,
                            leverage: operation.leverage,
                        },
                    )
                }
            };
            AccountAdjustmentOperationAccess::Populated(populated)
        }
    };

    let amount = match adjustment.amount.as_ref() {
        None => AccountAdjustmentAmountAccess::Absent,
        Some(py_amount) => {
            let value = py_amount.bind(py).borrow();
            AccountAdjustmentAmountAccess::Populated(openpit::AccountAdjustmentAmount {
                total: value.total,
                reserved: value.reserved,
                pending: value.pending,
            })
        }
    };

    let bounds = match adjustment.bounds.as_ref() {
        None => AccountAdjustmentBoundsAccess::Absent,
        Some(py_bounds) => {
            let value = py_bounds.bind(py).borrow();
            AccountAdjustmentBoundsAccess::Populated(openpit::AccountAdjustmentBounds {
                total_upper: value.total_upper,
                total_lower: value.total_lower,
                reserved_upper: value.reserved_upper,
                reserved_lower: value.reserved_lower,
                pending_upper: value.pending_upper,
                pending_lower: value.pending_lower,
            })
        }
    };

    Ok(AccountAdjustment {
        operation,
        amount,
        bounds,
        original: obj.clone().unbind(),
    })
}

fn apply_policy_decision(
    policy_name: &str,
    decision: Bound<'_, PyAny>,
    mutations: &mut Mutations,
    rejects: &mut Vec<Reject>,
) -> PyResult<()> {
    let reject_items = decision.getattr("rejects")?;
    for item in reject_items.iter()? {
        rejects.push(parse_policy_reject(&item?, policy_name)?);
    }

    let mutation_items = decision.getattr("mutations")?;
    for item in mutation_items.iter()? {
        mutations.push(parse_policy_mutation(&item?)?);
    }
    Ok(())
}

fn parse_policy_rejects(value: &Bound<'_, PyAny>, policy_name: &str) -> PyResult<Vec<Reject>> {
    if value.is_none() {
        return Ok(Vec::new());
    }

    let mut rejects = Vec::new();
    for item in value.iter()? {
        rejects.push(parse_policy_reject(&item?, policy_name)?);
    }
    Ok(rejects)
}

fn parse_policy_reject(value: &Bound<'_, PyAny>, policy_name: &str) -> PyResult<Reject> {
    let code = parse_reject_code(
        value
            .getattr("code")?
            .extract::<String>()
            .map_err(|_| PyValueError::new_err("reject.code must be a string"))?
            .as_str(),
    )?;
    let reason = value
        .getattr("reason")?
        .extract::<String>()
        .map_err(|_| PyValueError::new_err("reject.reason must be a string"))?;
    let details = value
        .getattr("details")?
        .extract::<String>()
        .map_err(|_| PyValueError::new_err("reject.details must be a string"))?;
    let scope = parse_reject_scope(
        value
            .getattr("scope")?
            .extract::<String>()
            .map_err(|_| PyValueError::new_err("reject.scope must be a string"))?
            .as_str(),
    )?;
    let user_data = if value.hasattr("user_data")? {
        value.getattr("user_data")?.extract::<u64>().map_err(|_| {
            PyValueError::new_err("reject.user_data must be an integer token (default 0)")
        })?
    } else {
        0
    };
    Ok(Reject::new(policy_name, scope, code, reason, details).with_user_data(user_data as usize))
}

fn parse_policy_mutation(value: &Bound<'_, PyAny>) -> PyResult<Mutation> {
    let commit_callable = value.getattr("commit")?.unbind();
    let rollback_callable = value.getattr("rollback")?.unbind();

    Ok(Mutation::new(
        move || {
            Python::with_gil(|py| {
                if let Err(error) = commit_callable.bind(py).call0() {
                    set_python_callback_error(error);
                }
            });
        },
        move || {
            Python::with_gil(|py| {
                if let Err(error) = rollback_callable.bind(py).call0() {
                    set_python_callback_error(error);
                }
            });
        },
    ))
}

fn parse_reject_scope(value: &str) -> PyResult<RejectScope> {
    match value.trim().to_ascii_lowercase().as_str() {
        "order" => Ok(RejectScope::Order),
        "account" => Ok(RejectScope::Account),
        _ => Err(PyValueError::new_err(
            "reject.scope must be either 'order' or 'account'",
        )),
    }
}

fn parse_reject_code(value: &str) -> PyResult<RejectCode> {
    match value {
        code if code == RejectCode::MissingRequiredField.as_str() => {
            Ok(RejectCode::MissingRequiredField)
        }
        code if code == RejectCode::InvalidFieldFormat.as_str() => {
            Ok(RejectCode::InvalidFieldFormat)
        }
        code if code == RejectCode::InvalidFieldValue.as_str() => Ok(RejectCode::InvalidFieldValue),
        code if code == RejectCode::UnsupportedOrderType.as_str() => {
            Ok(RejectCode::UnsupportedOrderType)
        }
        code if code == RejectCode::UnsupportedTimeInForce.as_str() => {
            Ok(RejectCode::UnsupportedTimeInForce)
        }
        code if code == RejectCode::UnsupportedOrderAttribute.as_str() => {
            Ok(RejectCode::UnsupportedOrderAttribute)
        }
        code if code == RejectCode::DuplicateClientOrderId.as_str() => {
            Ok(RejectCode::DuplicateClientOrderId)
        }
        code if code == RejectCode::TooLateToEnter.as_str() => Ok(RejectCode::TooLateToEnter),
        code if code == RejectCode::ExchangeClosed.as_str() => Ok(RejectCode::ExchangeClosed),
        code if code == RejectCode::UnknownInstrument.as_str() => Ok(RejectCode::UnknownInstrument),
        code if code == RejectCode::UnknownAccount.as_str() => Ok(RejectCode::UnknownAccount),
        code if code == RejectCode::UnknownVenue.as_str() => Ok(RejectCode::UnknownVenue),
        code if code == RejectCode::UnknownClearingAccount.as_str() => {
            Ok(RejectCode::UnknownClearingAccount)
        }
        code if code == RejectCode::UnknownCollateralAsset.as_str() => {
            Ok(RejectCode::UnknownCollateralAsset)
        }
        code if code == RejectCode::InsufficientFunds.as_str() => Ok(RejectCode::InsufficientFunds),
        code if code == RejectCode::InsufficientMargin.as_str() => {
            Ok(RejectCode::InsufficientMargin)
        }
        code if code == RejectCode::InsufficientPosition.as_str() => {
            Ok(RejectCode::InsufficientPosition)
        }
        code if code == RejectCode::CreditLimitExceeded.as_str() => {
            Ok(RejectCode::CreditLimitExceeded)
        }
        code if code == RejectCode::RiskLimitExceeded.as_str() => Ok(RejectCode::RiskLimitExceeded),
        code if code == RejectCode::OrderExceedsLimit.as_str() => Ok(RejectCode::OrderExceedsLimit),
        code if code == RejectCode::OrderQtyExceedsLimit.as_str() => {
            Ok(RejectCode::OrderQtyExceedsLimit)
        }
        code if code == RejectCode::OrderNotionalExceedsLimit.as_str() => {
            Ok(RejectCode::OrderNotionalExceedsLimit)
        }
        code if code == RejectCode::PositionLimitExceeded.as_str() => {
            Ok(RejectCode::PositionLimitExceeded)
        }
        code if code == RejectCode::ConcentrationLimitExceeded.as_str() => {
            Ok(RejectCode::ConcentrationLimitExceeded)
        }
        code if code == RejectCode::LeverageLimitExceeded.as_str() => {
            Ok(RejectCode::LeverageLimitExceeded)
        }
        code if code == RejectCode::RateLimitExceeded.as_str() => Ok(RejectCode::RateLimitExceeded),
        code if code == RejectCode::PnlKillSwitchTriggered.as_str() => {
            Ok(RejectCode::PnlKillSwitchTriggered)
        }
        code if code == RejectCode::AccountBlocked.as_str() => Ok(RejectCode::AccountBlocked),
        code if code == RejectCode::AccountNotAuthorized.as_str() => {
            Ok(RejectCode::AccountNotAuthorized)
        }
        code if code == RejectCode::ComplianceRestriction.as_str() => {
            Ok(RejectCode::ComplianceRestriction)
        }
        code if code == RejectCode::InstrumentRestricted.as_str() => {
            Ok(RejectCode::InstrumentRestricted)
        }
        code if code == RejectCode::JurisdictionRestriction.as_str() => {
            Ok(RejectCode::JurisdictionRestriction)
        }
        code if code == RejectCode::WashTradePrevention.as_str() => {
            Ok(RejectCode::WashTradePrevention)
        }
        code if code == RejectCode::SelfMatchPrevention.as_str() => {
            Ok(RejectCode::SelfMatchPrevention)
        }
        code if code == RejectCode::ShortSaleRestriction.as_str() => {
            Ok(RejectCode::ShortSaleRestriction)
        }
        code if code == RejectCode::RiskConfigurationMissing.as_str() => {
            Ok(RejectCode::RiskConfigurationMissing)
        }
        code if code == RejectCode::ReferenceDataUnavailable.as_str() => {
            Ok(RejectCode::ReferenceDataUnavailable)
        }
        code if code == RejectCode::OrderValueCalculationFailed.as_str() => {
            Ok(RejectCode::OrderValueCalculationFailed)
        }
        code if code == RejectCode::SystemUnavailable.as_str() => Ok(RejectCode::SystemUnavailable),
        code if code == RejectCode::Other.as_str() => Ok(RejectCode::Other),
        _ => Err(PyValueError::new_err(format!(
            "unsupported reject code {value:?}"
        ))),
    }
}

impl PyEngineBuilder {
    fn push_start_policy(&self, policy: &Bound<'_, PyAny>) -> PyResult<()> {
        let rust_policy: Box<dyn CheckPreTradeStartPolicy<Order, ExecutionReport>> =
            if let Ok(policy) = policy.extract::<PyRef<'_, PyPnlBoundsKillSwitchPolicy>>() {
                Box::new(SharedStartPolicy {
                    policy: policy.policy_for_engine(),
                })
            } else if let Ok(policy) = policy.extract::<PyRef<'_, PyOrderValidationPolicy>>() {
                Box::new(SharedStartPolicy {
                    policy: policy.policy_for_engine(),
                })
            } else if let Ok(policy) = policy.extract::<PyRef<'_, PyRateLimitPolicy>>() {
                Box::new(SharedStartPolicy {
                    policy: policy.policy_for_engine(),
                })
            } else if let Ok(policy) = policy.extract::<PyRef<'_, PyOrderSizeLimitPolicy>>() {
                Box::new(SharedStartPolicy {
                    policy: policy.policy_for_engine(),
                })
            } else {
                let name = policy
                    .getattr("name")?
                    .extract::<String>()
                    .map_err(|_| PyValueError::new_err("policy.name must be a string"))?;
                if name.trim().is_empty() {
                    return Err(PyValueError::new_err("policy.name must not be empty"));
                }
                ensure_callable_method(policy, "check_pre_trade_start")?;
                ensure_callable_method(policy, "apply_execution_report")?;
                Box::new(PythonStartPolicyAdapter {
                    name,
                    policy: policy.clone().unbind(),
                })
            };

        self.start_policies
            .borrow_mut()
            .push(BoxedStartPolicy { inner: rust_policy });
        Ok(())
    }

    fn push_main_policy(&self, policy: &Bound<'_, PyAny>) -> PyResult<()> {
        let name = policy
            .getattr("name")?
            .extract::<String>()
            .map_err(|_| PyValueError::new_err("policy.name must be a string"))?;
        if name.trim().is_empty() {
            return Err(PyValueError::new_err("policy.name must not be empty"));
        }
        ensure_callable_method(policy, "perform_pre_trade_check")?;
        ensure_callable_method(policy, "apply_execution_report")?;
        self.main_policies.borrow_mut().push(BoxedMainPolicy {
            inner: Box::new(PythonMainPolicyAdapter {
                name,
                policy: policy.clone().unbind(),
            }),
        });
        Ok(())
    }

    fn push_account_adjustment_policy(&self, policy: &Bound<'_, PyAny>) -> PyResult<()> {
        let name = policy
            .getattr("name")?
            .extract::<String>()
            .map_err(|_| PyValueError::new_err("policy.name must be a string"))?;
        if name.trim().is_empty() {
            return Err(PyValueError::new_err("policy.name must not be empty"));
        }
        ensure_callable_method(policy, "apply_account_adjustment")?;
        self.adjustment_policies
            .borrow_mut()
            .push(BoxedAccountAdjustmentPolicy {
                inner: Box::new(PythonAccountAdjustmentPolicyAdapter {
                    name,
                    policy: policy.clone().unbind(),
                }),
            });
        Ok(())
    }
}

#[pyclass(name = "EngineBuilder", module = "openpit", unsendable)]
struct PyEngineBuilder {
    start_policies: RefCell<Vec<BoxedStartPolicy>>,
    main_policies: RefCell<Vec<BoxedMainPolicy>>,
    adjustment_policies: RefCell<Vec<BoxedAccountAdjustmentPolicy>>,
}

#[pymethods]
impl PyEngineBuilder {
    #[pyo3(signature = (policy))]
    fn check_pre_trade_start_policy<'py>(
        slf: PyRef<'py, Self>,
        policy: &Bound<'_, PyAny>,
    ) -> PyResult<PyRef<'py, Self>> {
        slf.push_start_policy(policy)?;
        Ok(slf)
    }

    #[pyo3(signature = (policy))]
    fn pre_trade_policy<'py>(
        slf: PyRef<'py, Self>,
        policy: &Bound<'_, PyAny>,
    ) -> PyResult<PyRef<'py, Self>> {
        slf.push_main_policy(policy)?;
        Ok(slf)
    }

    #[pyo3(signature = (policy))]
    fn account_adjustment_policy<'py>(
        slf: PyRef<'py, Self>,
        policy: &Bound<'_, PyAny>,
    ) -> PyResult<PyRef<'py, Self>> {
        slf.push_account_adjustment_policy(policy)?;
        Ok(slf)
    }

    fn build(&self) -> PyResult<PyEngine> {
        let mut builder = Engine::<Order, ExecutionReport, AccountAdjustment>::builder();

        for policy in self.start_policies.borrow_mut().drain(..) {
            builder = builder.check_pre_trade_start_policy(policy);
        }

        for policy in self.main_policies.borrow_mut().drain(..) {
            builder = builder.pre_trade_policy(policy);
        }

        for policy in self.adjustment_policies.borrow_mut().drain(..) {
            builder = builder.account_adjustment_policy(policy);
        }

        let engine = builder
            .build()
            .map_err(|error| PyValueError::new_err(format_engine_build_error(error)))?;

        Ok(PyEngine { inner: engine })
    }
}

fn parse_optional_pnl_input(value: Option<&Bound<'_, PyAny>>) -> PyResult<Option<Pnl>> {
    match value {
        Some(value) if value.is_none() => Ok(None),
        Some(value) => Ok(Some(parse_pnl_input(value)?)),
        None => Ok(None),
    }
}

fn create_pnl_bounds_barrier(
    settlement_asset: &Bound<'_, PyAny>,
    lower_bound: Option<&Bound<'_, PyAny>>,
    upper_bound: Option<&Bound<'_, PyAny>>,
    initial_pnl: &Bound<'_, PyAny>,
) -> PyResult<PnlBoundsBarrier> {
    let lower_bound = parse_optional_pnl_input(lower_bound)?;
    let upper_bound = parse_optional_pnl_input(upper_bound)?;
    if lower_bound.is_none() && upper_bound.is_none() {
        return Err(PyValueError::new_err(
            "at least one of lower_bound or upper_bound must be provided",
        ));
    }

    Ok(PnlBoundsBarrier {
        settlement_asset: parse_asset_input(settlement_asset)?,
        lower_bound,
        upper_bound,
        initial_pnl: parse_pnl_input(initial_pnl)?,
    })
}

fn build_pnl_kill_switch_policy(
    barriers: &[PnlBoundsBarrier],
) -> PyResult<PnlBoundsKillSwitchPolicy> {
    let (first, rest) = barriers.split_first().ok_or_else(|| {
        PyValueError::new_err("PnlBoundsKillSwitchPolicy requires at least one barrier")
    })?;
    let first_barrier = first.clone();
    let rest_barriers = rest.to_vec();
    PnlBoundsKillSwitchPolicy::new(first_barrier, rest_barriers)
        .map_err(|error| PyValueError::new_err(error.to_string()))
}

fn build_order_size_limit_policy(
    limits: &[OrderSizeLimitConfig],
) -> PyResult<OrderSizeLimitPolicy> {
    let (first, rest) = limits
        .split_first()
        .ok_or_else(|| PyValueError::new_err("OrderSizeLimitPolicy requires at least one limit"))?;
    let first_limit = OrderSizeLimit {
        settlement_asset: parse_asset(first.settlement_asset.as_str())?,
        max_quantity: parse_quantity(&first.max_quantity)?,
        max_notional: parse_volume(&first.max_notional)?,
    };
    let rest_limits = rest
        .iter()
        .map(|limit| {
            Ok(OrderSizeLimit {
                settlement_asset: parse_asset(limit.settlement_asset.as_str())?,
                max_quantity: parse_quantity(&limit.max_quantity)?,
                max_notional: parse_volume(&limit.max_notional)?,
            })
        })
        .collect::<PyResult<Vec<_>>>()?;
    Ok(OrderSizeLimitPolicy::new(first_limit, rest_limits))
}

#[pyclass(
    name = "PnlBoundsKillSwitchPolicy",
    module = "openpit.pretrade.policies",
    unsendable
)]
struct PyPnlBoundsKillSwitchPolicy {
    barriers: RefCell<Vec<PnlBoundsBarrier>>,
    inner: RefCell<Rc<PnlBoundsKillSwitchPolicy>>,
    bound_to_engine: Cell<bool>,
}

#[pymethods]
impl PyPnlBoundsKillSwitchPolicy {
    #[classattr]
    const NAME: &'static str = PnlBoundsKillSwitchPolicy::NAME;

    #[new]
    #[pyo3(signature = (*, settlement_asset, lower_bound = None, upper_bound = None, initial_pnl))]
    fn new(
        settlement_asset: &Bound<'_, PyAny>,
        lower_bound: Option<&Bound<'_, PyAny>>,
        upper_bound: Option<&Bound<'_, PyAny>>,
        initial_pnl: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        let barriers = vec![create_pnl_bounds_barrier(
            settlement_asset,
            lower_bound,
            upper_bound,
            initial_pnl,
        )?];
        let policy = build_pnl_kill_switch_policy(&barriers)?;

        Ok(Self {
            barriers: RefCell::new(barriers),
            inner: RefCell::new(Rc::new(policy)),
            bound_to_engine: Cell::new(false),
        })
    }

    #[pyo3(signature = (*, settlement_asset, lower_bound = None, upper_bound = None, initial_pnl))]
    fn set_barrier(
        &self,
        settlement_asset: &Bound<'_, PyAny>,
        lower_bound: Option<&Bound<'_, PyAny>>,
        upper_bound: Option<&Bound<'_, PyAny>>,
        initial_pnl: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        if self.bound_to_engine.get() {
            return Err(PyRuntimeError::new_err(
                "pnl policy is already bound to an engine and cannot be reconfigured",
            ));
        }

        let barrier =
            create_pnl_bounds_barrier(settlement_asset, lower_bound, upper_bound, initial_pnl)?;

        let mut barriers = self.barriers.borrow_mut();
        if let Some(existing) = barriers
            .iter_mut()
            .find(|existing| existing.settlement_asset == barrier.settlement_asset)
        {
            *existing = barrier.clone();
        } else {
            barriers.push(barrier);
        }
        let policy = build_pnl_kill_switch_policy(&barriers)?;
        self.inner.replace(Rc::new(policy));
        Ok(())
    }

    #[pyo3(signature = (settlement_asset))]
    fn reset_pnl(&self, settlement_asset: &Bound<'_, PyAny>) -> PyResult<()> {
        let settlement_asset = parse_asset_input(settlement_asset)?;
        let policy = self.inner.borrow();
        policy.reset_pnl(&settlement_asset);
        Ok(())
    }
}

impl PyPnlBoundsKillSwitchPolicy {
    fn policy_for_engine(&self) -> Rc<PnlBoundsKillSwitchPolicy> {
        self.bound_to_engine.set(true);
        Rc::clone(&self.inner.borrow())
    }
}

#[pyclass(
    name = "RateLimitPolicy",
    module = "openpit.pretrade.policies",
    unsendable
)]
struct PyRateLimitPolicy {
    inner: Rc<RateLimitPolicy>,
}

#[pymethods]
impl PyRateLimitPolicy {
    #[classattr]
    const NAME: &'static str = RateLimitPolicy::NAME;

    #[new]
    #[pyo3(signature = (max_orders, window_seconds))]
    fn new(max_orders: usize, window_seconds: u64) -> Self {
        Self {
            inner: Rc::new(RateLimitPolicy::new(
                max_orders,
                Duration::from_secs(window_seconds),
            )),
        }
    }
}

impl PyRateLimitPolicy {
    fn policy_for_engine(&self) -> Rc<RateLimitPolicy> {
        Rc::clone(&self.inner)
    }
}

#[pyclass(
    name = "OrderValidationPolicy",
    module = "openpit.pretrade.policies",
    unsendable
)]
struct PyOrderValidationPolicy {
    inner: Rc<OrderValidationPolicy>,
}

#[pymethods]
impl PyOrderValidationPolicy {
    #[classattr]
    const NAME: &'static str = OrderValidationPolicy::NAME;

    #[new]
    fn new() -> Self {
        Self {
            inner: Rc::new(OrderValidationPolicy::new()),
        }
    }
}

impl PyOrderValidationPolicy {
    fn policy_for_engine(&self) -> Rc<OrderValidationPolicy> {
        Rc::clone(&self.inner)
    }
}

#[pyclass(name = "OrderSizeLimit", module = "openpit.pretrade.policies")]
#[derive(Clone)]
struct PyOrderSizeLimit {
    inner: OrderSizeLimitConfig,
}

#[pymethods]
impl PyOrderSizeLimit {
    #[new]
    #[pyo3(signature = (*, settlement_asset, max_quantity, max_notional))]
    fn new(
        settlement_asset: &Bound<'_, PyAny>,
        max_quantity: &Bound<'_, PyAny>,
        max_notional: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        let settlement_asset = parse_asset_input(settlement_asset)?.to_string();
        let max_quantity = parse_quantity_input(max_quantity)?.to_string();
        let max_notional = parse_volume_input(max_notional)?.to_string();

        Ok(Self {
            inner: OrderSizeLimitConfig {
                settlement_asset,
                max_quantity,
                max_notional,
            },
        })
    }
}

#[pyclass(
    name = "OrderSizeLimitPolicy",
    module = "openpit.pretrade.policies",
    unsendable
)]
struct PyOrderSizeLimitPolicy {
    limits: RefCell<Vec<OrderSizeLimitConfig>>,
    inner: RefCell<Rc<OrderSizeLimitPolicy>>,
}

#[pymethods]
impl PyOrderSizeLimitPolicy {
    #[classattr]
    const NAME: &'static str = OrderSizeLimitPolicy::NAME;

    #[new]
    #[pyo3(signature = (limit))]
    fn new(limit: &PyOrderSizeLimit) -> PyResult<Self> {
        let limits = vec![limit.inner.clone()];
        let policy = build_order_size_limit_policy(&limits)?;
        Ok(Self {
            limits: RefCell::new(limits),
            inner: RefCell::new(Rc::new(policy)),
        })
    }

    #[pyo3(signature = (limit))]
    fn set_limit(&self, limit: &PyOrderSizeLimit) -> PyResult<()> {
        let mut limits = self.limits.borrow_mut();
        if let Some(existing) = limits.iter_mut().find(|existing| {
            existing.settlement_asset.as_str() == limit.inner.settlement_asset.as_str()
        }) {
            *existing = limit.inner.clone();
        } else {
            limits.push(limit.inner.clone());
        }
        let policy = build_order_size_limit_policy(&limits)?;
        self.inner.replace(Rc::new(policy));
        Ok(())
    }
}

impl PyOrderSizeLimitPolicy {
    fn policy_for_engine(&self) -> Rc<OrderSizeLimitPolicy> {
        Rc::clone(&self.inner.borrow())
    }
}

#[pyclass(name = "OrderOperation", module = "openpit.core", subclass)]
#[derive(Clone)]
struct PyOrderOperation {
    underlying_asset: Option<Asset>,
    settlement_asset: Option<Asset>,
    account_id: Option<AccountId>,
    side: Option<Side>,
    trade_amount: Option<TradeAmount>,
    price: Option<Price>,
}

#[pymethods]
impl PyOrderOperation {
    #[new]
    #[pyo3(signature = (*, underlying_asset = None, settlement_asset = None, account_id = None, side = None, trade_amount = None, price = None))]
    fn new(
        underlying_asset: Option<&Bound<'_, PyAny>>,
        settlement_asset: Option<&Bound<'_, PyAny>>,
        account_id: Option<&Bound<'_, PyAny>>,
        side: Option<&Bound<'_, PyAny>>,
        trade_amount: Option<&Bound<'_, PyAny>>,
        price: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let assets_are_partial = underlying_asset.is_some() ^ settlement_asset.is_some();
        if assets_are_partial {
            return Err(PyValueError::new_err(
                "underlying_asset and settlement_asset must be provided together",
            ));
        }
        Ok(Self {
            underlying_asset: underlying_asset.map(parse_asset_input).transpose()?,
            settlement_asset: settlement_asset.map(parse_asset_input).transpose()?,
            account_id: account_id.map(parse_account_id_input).transpose()?,
            side: side.map(parse_side_input).transpose()?,
            trade_amount: trade_amount.map(parse_trade_amount_input).transpose()?,
            price: price.map(parse_price_input).transpose()?,
        })
    }

    #[getter]
    fn underlying_asset(&self) -> Option<String> {
        self.underlying_asset.clone().map(|inner| inner.to_string())
    }

    #[setter]
    fn set_underlying_asset(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.underlying_asset = value.map(parse_asset_input).transpose()?;
        Ok(())
    }

    #[getter]
    fn settlement_asset(&self) -> Option<String> {
        self.settlement_asset.clone().map(|inner| inner.to_string())
    }

    #[setter]
    fn set_settlement_asset(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.settlement_asset = value.map(parse_asset_input).transpose()?;
        Ok(())
    }

    #[getter]
    fn account_id(&self) -> Option<PyAccountId> {
        self.account_id.map(|inner| PyAccountId { inner })
    }

    #[setter]
    fn set_account_id(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.account_id = value.map(parse_account_id_input).transpose()?;
        Ok(())
    }

    #[getter]
    fn side(&self, py: Python<'_>) -> PyResult<Option<PyObject>> {
        self.side
            .map(|side| {
                PyModule::import_bound(py, "openpit.param")?
                    .getattr("Side")?
                    .call1((side_name(side),))
                    .map(|obj| obj.into_py(py))
            })
            .transpose()
    }

    #[setter]
    fn set_side(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.side = value.map(parse_side_input).transpose()?;
        Ok(())
    }

    #[getter]
    fn trade_amount(&self) -> Option<PyTradeAmount> {
        self.trade_amount.map(trade_amount_to_python)
    }

    #[setter]
    fn set_trade_amount(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.trade_amount = value.map(parse_trade_amount_input).transpose()?;
        Ok(())
    }

    #[getter]
    fn price(&self) -> Option<PyPrice> {
        self.price.map(|inner| PyPrice { inner })
    }

    #[setter]
    fn set_price(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.price = value.map(parse_price_input).transpose()?;
        Ok(())
    }

    fn __repr__(&self, py: Python<'_>) -> String {
        let trade_amount = self.trade_amount().map(|v| v.__repr__());
        format!(
            "OrderOperation(underlying_asset={:?}, settlement_asset={:?}, side={:?}, trade_amount={:?}, price={:?})",
            self.underlying_asset(),
            self.settlement_asset(),
            self.side(py),
            trade_amount,
            self.price().map(|v| v.inner.to_string()),
        )
    }
}

#[pyclass(name = "OrderPosition", module = "openpit.core", subclass)]
#[derive(Clone)]
struct PyOrderPosition {
    position_side: Option<PositionSide>,
    reduce_only: bool,
    close_position: bool,
}

#[pymethods]
impl PyOrderPosition {
    #[new]
    #[pyo3(signature = (*, position_side = None, reduce_only = false, close_position = false))]
    fn new(
        position_side: Option<&Bound<'_, PyAny>>,
        reduce_only: bool,
        close_position: bool,
    ) -> PyResult<Self> {
        Ok(Self {
            position_side: position_side.map(parse_position_side_input).transpose()?,
            reduce_only,
            close_position,
        })
    }

    #[getter]
    fn position_side(&self, py: Python<'_>) -> PyResult<Option<PyObject>> {
        self.position_side
            .map(|side| {
                PyModule::import_bound(py, "openpit.param")?
                    .getattr("PositionSide")?
                    .call1((position_side_name(side),))
                    .map(|obj| obj.into_py(py))
            })
            .transpose()
    }

    #[setter]
    fn set_position_side(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.position_side = value.map(parse_position_side_input).transpose()?;
        Ok(())
    }

    #[getter]
    fn reduce_only(&self) -> bool {
        self.reduce_only
    }

    #[setter]
    fn set_reduce_only(&mut self, value: bool) {
        self.reduce_only = value;
    }

    #[getter]
    fn close_position(&self) -> bool {
        self.close_position
    }

    #[setter]
    fn set_close_position(&mut self, value: bool) {
        self.close_position = value;
    }

    fn __repr__(&self, py: Python<'_>) -> String {
        format!(
            "OrderPosition(position_side={:?}, reduce_only={:?}, close_position={:?})",
            self.position_side(py),
            self.reduce_only(),
            self.close_position(),
        )
    }
}

#[pyclass(name = "OrderMargin", module = "openpit.core", subclass)]
#[derive(Clone)]
struct PyOrderMargin {
    leverage: Option<Leverage>,
    collateral_asset: Option<Asset>,
    auto_borrow: bool,
}

#[pymethods]
impl PyOrderMargin {
    #[new]
    #[pyo3(signature = (*, leverage = None, collateral_asset = None, auto_borrow = false))]
    fn new(
        leverage: Option<&Bound<'_, PyAny>>,
        collateral_asset: Option<&Bound<'_, PyAny>>,
        auto_borrow: bool,
    ) -> PyResult<Self> {
        Ok(Self {
            leverage: leverage.map(parse_leverage_input).transpose()?,
            collateral_asset: collateral_asset.map(parse_asset_input).transpose()?,
            auto_borrow,
        })
    }

    #[getter]
    fn leverage(&self) -> Option<PyLeverage> {
        self.leverage.map(|inner| PyLeverage { inner })
    }

    #[setter]
    fn set_leverage(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.leverage = value.map(parse_leverage_input).transpose()?;
        Ok(())
    }

    #[getter]
    fn collateral_asset(&self) -> Option<String> {
        self.collateral_asset.clone().map(|inner| inner.to_string())
    }

    #[setter]
    fn set_collateral_asset(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.collateral_asset = value.map(parse_asset_input).transpose()?;
        Ok(())
    }

    #[getter]
    fn auto_borrow(&self) -> bool {
        self.auto_borrow
    }

    #[setter]
    fn set_auto_borrow(&mut self, value: bool) {
        self.auto_borrow = value;
    }

    fn __repr__(&self) -> String {
        format!(
            "OrderMargin(leverage={:?}, collateral_asset={:?}, auto_borrow={:?})",
            self.leverage().map(|v| v.value()),
            self.collateral_asset(),
            self.auto_borrow(),
        )
    }
}

#[pyclass(name = "Order", module = "openpit.core", subclass, unsendable)]
struct PyOrder {
    operation: Option<Py<PyOrderOperation>>,
    position: Option<Py<PyOrderPosition>>,
    margin: Option<Py<PyOrderMargin>>,
}

#[pyclass(name = "Instrument", module = "openpit.core")]
#[derive(Clone)]
struct PyInstrument {
    inner: Instrument,
}

#[pyclass(name = "Leverage", module = "openpit.param")]
#[derive(Clone, Copy)]
struct PyLeverage {
    inner: Leverage,
}

#[pyclass(name = "AccountId", module = "openpit.param")]
#[derive(Clone, Copy)]
struct PyAccountId {
    inner: AccountId,
}

#[pyclass(name = "Quantity", module = "openpit.param")]
#[derive(Clone)]
struct PyQuantity {
    inner: Quantity,
}

#[pyclass(name = "Price", module = "openpit.param")]
#[derive(Clone)]
struct PyPrice {
    inner: Price,
}

#[pyclass(name = "Trade", module = "openpit.param", subclass)]
#[derive(Clone)]
struct PyTrade {
    inner: Trade,
}

#[pyclass(name = "Pnl", module = "openpit.param")]
#[derive(Clone)]
struct PyPnl {
    inner: Pnl,
}

#[pyclass(name = "Fee", module = "openpit.param")]
#[derive(Clone)]
struct PyFee {
    inner: Fee,
}

#[pyclass(name = "Volume", module = "openpit.param")]
#[derive(Clone)]
struct PyVolume {
    inner: Volume,
}

#[pyclass(name = "Notional", module = "openpit.param")]
#[derive(Clone)]
struct PyNotional {
    inner: Notional,
}

#[pyclass(name = "CashFlow", module = "openpit.param")]
#[derive(Clone)]
struct PyCashFlow {
    inner: CashFlow,
}

#[pyclass(name = "PositionSize", module = "openpit.param")]
#[derive(Clone)]
struct PyPositionSize {
    inner: PositionSize,
}

#[pyclass(name = "AdjustmentAmount", module = "openpit.param", subclass)]
#[derive(Clone, Copy)]
struct PyAdjustmentAmount {
    inner: AdjustmentAmount,
}

#[pyclass(name = "TradeAmount", module = "openpit.param", subclass)]
#[derive(Clone, Copy)]
struct PyTradeAmount {
    inner: TradeAmount,
}

#[pyclass(name = "AccountAdjustmentAmount", module = "openpit.core", subclass)]
#[derive(Clone)]
struct PyAccountAdjustmentAmount {
    total: Option<AdjustmentAmount>,
    reserved: Option<AdjustmentAmount>,
    pending: Option<AdjustmentAmount>,
}

#[pyclass(
    name = "AccountAdjustmentBalanceOperation",
    module = "openpit.core",
    subclass
)]
#[derive(Clone)]
struct PyAccountAdjustmentBalanceOperation {
    asset: Option<Asset>,
    average_entry_price: Option<Price>,
}

#[pyclass(
    name = "AccountAdjustmentPositionOperation",
    module = "openpit.core",
    subclass
)]
#[derive(Clone)]
struct PyAccountAdjustmentPositionOperation {
    underlying_asset: Option<Asset>,
    settlement_asset: Option<Asset>,
    collateral_asset: Option<Asset>,
    average_entry_price: Option<Price>,
    mode: Option<PositionMode>,
    leverage: Option<Leverage>,
}

#[pyclass(name = "AccountAdjustmentBounds", module = "openpit.core", subclass)]
#[derive(Clone)]
struct PyAccountAdjustmentBounds {
    total_upper: Option<PositionSize>,
    total_lower: Option<PositionSize>,
    reserved_upper: Option<PositionSize>,
    reserved_lower: Option<PositionSize>,
    pending_upper: Option<PositionSize>,
    pending_lower: Option<PositionSize>,
}

enum PyAccountAdjustmentOperation {
    Balance(Py<PyAccountAdjustmentBalanceOperation>),
    Position(Py<PyAccountAdjustmentPositionOperation>),
}

#[pyclass(
    name = "AccountAdjustment",
    module = "openpit.core",
    subclass,
    unsendable
)]
struct PyAccountAdjustment {
    operation: Option<PyAccountAdjustmentOperation>,
    amount: Option<Py<PyAccountAdjustmentAmount>>,
    bounds: Option<Py<PyAccountAdjustmentBounds>>,
}

#[pymethods]
impl PyInstrument {
    #[new]
    #[pyo3(signature = (underlying_asset, settlement_asset))]
    fn new(
        underlying_asset: &Bound<'_, PyAny>,
        settlement_asset: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: Instrument::new(
                parse_asset_input(underlying_asset)?,
                parse_asset_input(settlement_asset)?,
            ),
        })
    }

    #[getter]
    fn underlying_asset(&self) -> String {
        self.inner.underlying_asset().to_string()
    }

    #[getter]
    fn settlement_asset(&self) -> String {
        self.inner.settlement_asset().to_string()
    }

    fn __repr__(&self) -> String {
        format!(
            "Instrument(underlying_asset={:?}, settlement_asset={:?})",
            self.underlying_asset(),
            self.settlement_asset()
        )
    }
}

// Capability traits and generic wrapper combinators stay Rust-only because
// they encode compile-time guarantees that do not map to Python runtime APIs.

#[pymethods]
impl PyOrder {
    #[new]
    #[pyo3(signature = (*, operation = None, position = None, margin = None))]
    fn new(
        py: Python<'_>,
        operation: Option<&Bound<'_, PyAny>>,
        position: Option<&Bound<'_, PyAny>>,
        margin: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let operation = operation
            .map(|v| {
                v.extract::<PyOrderOperation>()
                    .map(|op| Py::new(py, op))
                    .map_err(|_| {
                        PyTypeError::new_err("operation must be openpit.core.OrderOperation")
                    })
                    .and_then(|r| r)
            })
            .transpose()?;
        let position = position
            .map(|v| {
                v.extract::<PyOrderPosition>()
                    .map(|pos| Py::new(py, pos))
                    .map_err(|_| {
                        PyTypeError::new_err("position must be openpit.core.OrderPosition")
                    })
                    .and_then(|r| r)
            })
            .transpose()?;
        let margin = margin
            .map(|v| {
                v.extract::<PyOrderMargin>()
                    .map(|m| Py::new(py, m))
                    .map_err(|_| PyTypeError::new_err("margin must be openpit.core.OrderMargin"))
                    .and_then(|r| r)
            })
            .transpose()?;
        Ok(Self {
            operation,
            position,
            margin,
        })
    }

    #[getter]
    fn operation(&self, py: Python<'_>) -> Option<Py<PyOrderOperation>> {
        self.operation.as_ref().map(|v| v.clone_ref(py))
    }

    #[setter]
    fn set_operation(&mut self, py: Python<'_>, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.operation = value
            .map(|v| {
                v.extract::<PyOrderOperation>()
                    .map(|op| Py::new(py, op))
                    .map_err(|_| {
                        PyTypeError::new_err("operation must be openpit.core.OrderOperation")
                    })
                    .and_then(|r| r)
            })
            .transpose()?;
        Ok(())
    }

    #[getter]
    fn position(&self, py: Python<'_>) -> Option<Py<PyOrderPosition>> {
        self.position.as_ref().map(|v| v.clone_ref(py))
    }

    #[setter]
    fn set_position(&mut self, py: Python<'_>, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.position = value
            .map(|v| {
                v.extract::<PyOrderPosition>()
                    .map(|pos| Py::new(py, pos))
                    .map_err(|_| {
                        PyTypeError::new_err("position must be openpit.core.OrderPosition")
                    })
                    .and_then(|r| r)
            })
            .transpose()?;
        Ok(())
    }

    #[getter]
    fn margin(&self, py: Python<'_>) -> Option<Py<PyOrderMargin>> {
        self.margin.as_ref().map(|v| v.clone_ref(py))
    }

    #[setter]
    fn set_margin(&mut self, py: Python<'_>, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.margin = value
            .map(|v| {
                v.extract::<PyOrderMargin>()
                    .map(|m| Py::new(py, m))
                    .map_err(|_| PyTypeError::new_err("margin must be openpit.core.OrderMargin"))
                    .and_then(|r| r)
            })
            .transpose()?;
        Ok(())
    }

    fn __repr__(&self, py: Python<'_>) -> String {
        let operation = self
            .operation
            .as_ref()
            .map(|v| v.bind(py).borrow().__repr__(py));
        let position = self
            .position
            .as_ref()
            .map(|v| v.bind(py).borrow().__repr__(py));
        let margin = self.margin.as_ref().map(|v| v.bind(py).borrow().__repr__());
        format!(
            "Order(operation={:?}, position={:?}, margin={:?})",
            operation, position, margin,
        )
    }
}

#[pymethods]
impl PyLeverage {
    #[new]
    fn new(multiplier: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: parse_leverage_input(multiplier)?,
        })
    }

    #[staticmethod]
    fn from_int(multiplier: i64) -> PyResult<Self> {
        let multiplier = u16::try_from(multiplier)
            .map_err(|_| PyValueError::new_err("invalid leverage value"))?;
        Ok(Self {
            inner: Leverage::from_u16(multiplier)
                .map_err(|error| PyValueError::new_err(error.to_string()))?,
        })
    }

    #[staticmethod]
    fn from_float(multiplier: f64) -> PyResult<Self> {
        Ok(Self {
            inner: Leverage::from_f64(multiplier)
                .map_err(|error| PyValueError::new_err(error.to_string()))?,
        })
    }

    #[getter]
    fn value(&self) -> f32 {
        self.inner.value()
    }

    #[pyo3(signature = (notional))]
    fn calculate_margin_required(&self, notional: &Bound<'_, PyAny>) -> PyResult<PyNotional> {
        let notional = parse_notional_input(notional)?;
        Ok(PyNotional {
            inner: self
                .inner
                .calculate_margin_required(notional)
                .map_err(|error| create_param_error(error.to_string()))?,
        })
    }

    fn __repr__(&self) -> String {
        format!("Leverage(value={:?})", self.value())
    }
}

#[pymethods]
impl PyAccountId {
    /// Constructs an account identifier.
    ///
    /// No hashing. No collision risk.
    #[staticmethod]
    fn from_u64(value: u64) -> Self {
        Self {
            inner: AccountId::from_u64(value),
        }
    }

    /// Constructs an account identifier by hashing a string with FNV-1a 64-bit.
    ///
    /// Collisions are theoretically possible. For n distinct account strings
    /// the probability of at least one collision is approximately n^2 / 2^65.
    /// If collision risk is unacceptable, use ``from_u64`` with a collision-free
    /// integer mapping instead. See <http://www.isthe.com/chongo/tech/comp/fnv/> for the algorithm
    /// specification.
    #[staticmethod]
    fn from_str(value: &str) -> PyResult<Self> {
        Ok(Self {
            inner: AccountId::from_str(value)
                .map_err(|error| PyValueError::new_err(error.to_string()))?,
        })
    }

    #[getter]
    fn value(&self) -> u64 {
        self.inner.as_u64()
    }

    fn __repr__(&self) -> String {
        format!("AccountId(value={:?})", self.value())
    }

    fn __richcmp__(&self, other: &Bound<'_, PyAny>, op: CompareOp) -> PyResult<PyObject> {
        let py = other.py();
        if let Ok(other) = other.extract::<PyRef<'_, Self>>() {
            let result = match op {
                CompareOp::Eq => self.inner == other.inner,
                CompareOp::Ne => self.inner != other.inner,
                _ => return Ok(py.NotImplemented().into()),
            };
            return Ok(result.into_py(py));
        }
        Ok(py.NotImplemented().into())
    }

    fn __hash__(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.inner.as_u64().hash(&mut hasher);
        hasher.finish()
    }
}

macro_rules! impl_decimal_pymethods {
    ($py_type:ident, $domain:ty, $parse_input:ident, $py_name:literal, signed, { $($extra:tt)* }) => {
        #[pymethods]
        impl $py_type {
            #[new]
            fn new(value: &Bound<'_, PyAny>) -> PyResult<Self> {
                Ok(Self {
                    inner: $parse_input(value)?,
                })
            }

            #[classattr]
            const ZERO: Self = Self {
                inner: <$domain>::ZERO,
            };

            #[staticmethod]
            fn from_decimal(value: &Bound<'_, PyAny>) -> PyResult<Self> {
                let decimal = parse_python_decimal(value, $py_name)?;
                Ok(Self {
                    inner: <$domain>::from_str(decimal.to_string().as_str())
                        .map_err(|error| create_param_error(error.to_string()))?,
                })
            }

            #[staticmethod]
            fn from_str(value: &str) -> PyResult<Self> {
                Ok(Self {
                    inner: <$domain>::from_str(value)
                        .map_err(|error| create_param_error(error.to_string()))?,
                })
            }

            #[staticmethod]
            fn from_int(value: i64) -> PyResult<Self> {
                Ok(Self {
                    inner: <$domain>::from_str(value.to_string().as_str())
                        .map_err(|error| create_param_error(error.to_string()))?,
                })
            }

            #[staticmethod]
            fn from_u64(value: u64) -> PyResult<Self> {
                Ok(Self {
                    inner: <$domain>::from_str(value.to_string().as_str())
                        .map_err(|error| create_param_error(error.to_string()))?,
                })
            }

            #[staticmethod]
            /// WARNING:
            /// float values are inherently imprecise.
            /// Use decimal/string inputs for external monetary data.
            fn from_float(value: f64) -> PyResult<Self> {
                Ok(Self {
                    inner: <$domain>::from_f64(value)
                        .map_err(|error| create_param_error(error.to_string()))?,
                })
            }

            #[staticmethod]
            fn from_str_rounded(value: &str, scale: u32, strategy: &str) -> PyResult<Self> {
                Ok(Self {
                    inner: <$domain>::from_str_rounded(
                        value,
                        scale,
                        parse_rounding_strategy(strategy)?,
                    )
                    .map_err(|error| create_param_error(error.to_string()))?,
                })
            }

            #[staticmethod]
            fn from_float_rounded(value: f64, scale: u32, strategy: &str) -> PyResult<Self> {
                Ok(Self {
                    inner: <$domain>::from_f64_rounded(
                        value,
                        scale,
                        parse_rounding_strategy(strategy)?,
                    )
                    .map_err(|error| create_param_error(error.to_string()))?,
                })
            }

            #[staticmethod]
            fn from_decimal_rounded(
                value: &Bound<'_, PyAny>,
                scale: u32,
                strategy: &str,
            ) -> PyResult<Self> {
                let decimal = parse_python_decimal(value, $py_name)?;
                Ok(Self {
                    inner: <$domain>::from_decimal_rounded(
                        decimal,
                        scale,
                        parse_rounding_strategy(strategy)?,
                    )
                    .map_err(|error| create_param_error(error.to_string()))?,
                })
            }

            #[getter]
            fn decimal(&self, py: Python<'_>) -> PyResult<PyObject> {
                rust_decimal_to_python_decimal(py, self.inner.to_decimal())
            }

            fn to_json_value(&self) -> String {
                self.inner.to_string()
            }

            fn __repr__(&self) -> String {
                format!("{}(Decimal('{}'))", $py_name, self.inner)
            }

            fn __str__(&self) -> String {
                self.inner.to_string()
            }

            fn __richcmp__(&self, other: &Bound<'_, PyAny>, op: CompareOp) -> PyResult<PyObject> {
                let py = other.py();
                if let Ok(other) = other.extract::<PyRef<'_, $py_type>>() {
                    let result = match op {
                        CompareOp::Lt => self.inner < other.inner,
                        CompareOp::Le => self.inner <= other.inner,
                        CompareOp::Eq => self.inner == other.inner,
                        CompareOp::Ne => self.inner != other.inner,
                        CompareOp::Gt => self.inner > other.inner,
                        CompareOp::Ge => self.inner >= other.inner,
                    };
                    return Ok(result.into_py(py));
                }
                Ok(py.NotImplemented().into())
            }

            fn __hash__(&self) -> u64 {
                let mut hasher = DefaultHasher::new();
                self.inner.to_decimal().hash(&mut hasher);
                hasher.finish()
            }

            fn __bool__(&self) -> bool {
                !self.inner.is_zero()
            }

            /// WARNING:
            /// float values are inherently imprecise.
            /// Use decimal/string inputs for external monetary data.
            fn __float__(&self) -> PyResult<f64> {
                self.inner.to_decimal().to_f64().ok_or_else(|| {
                    PyValueError::new_err(format!("{} cannot be represented as float", $py_name))
                })
            }

            fn __add__(&self, other: &Bound<'_, PyAny>) -> PyResult<PyObject> {
                let py = other.py();
                if let Ok(other) = other.extract::<PyRef<'_, $py_type>>() {
                    let result = self
                        .inner
                        .checked_add(other.inner)
                        .map_err(|error| create_param_error(error.to_string()))?;
                    return Ok(Py::new(py, Self { inner: result })?.into_py(py));
                }
                Ok(py.NotImplemented().into())
            }

            fn __radd__(&self, other: &Bound<'_, PyAny>) -> PyResult<PyObject> {
                self.__add__(other)
            }

            fn __sub__(&self, other: &Bound<'_, PyAny>) -> PyResult<PyObject> {
                let py = other.py();
                if let Ok(other) = other.extract::<PyRef<'_, $py_type>>() {
                    let result = self
                        .inner
                        .checked_sub(other.inner)
                        .map_err(|error| create_param_error(error.to_string()))?;
                    return Ok(Py::new(py, Self { inner: result })?.into_py(py));
                }
                Ok(py.NotImplemented().into())
            }

            fn __rsub__(&self, other: &Bound<'_, PyAny>) -> PyResult<PyObject> {
                let py = other.py();
                if let Ok(other) = other.extract::<PyRef<'_, $py_type>>() {
                    let result = other
                        .inner
                        .checked_sub(self.inner)
                        .map_err(|error| create_param_error(error.to_string()))?;
                    return Ok(Py::new(py, Self { inner: result })?.into_py(py));
                }
                Ok(py.NotImplemented().into())
            }

            fn __mul__(&self, other: &Bound<'_, PyAny>) -> PyResult<PyObject> {
                let py = other.py();
                if let Some(scalar) = extract_scalar_operand(other)? {
                    let result = match scalar {
                        ScalarOperand::I64(v) => self
                            .inner
                            .checked_mul_i64(v)
                            .map_err(|error| create_param_error(error.to_string()))?,
                        ScalarOperand::U64(v) => self
                            .inner
                            .checked_mul_u64(v)
                            .map_err(|error| create_param_error(error.to_string()))?,
                        ScalarOperand::F64(v) => self
                            .inner
                            .checked_mul_f64(v)
                            .map_err(|error| create_param_error(error.to_string()))?,
                    };
                    return Ok(Py::new(py, Self { inner: result })?.into_py(py));
                }
                Ok(py.NotImplemented().into())
            }

            fn __rmul__(&self, other: &Bound<'_, PyAny>) -> PyResult<PyObject> {
                self.__mul__(other)
            }

            fn __truediv__(&self, other: &Bound<'_, PyAny>) -> PyResult<PyObject> {
                let py = other.py();
                if let Some(divisor) = extract_scalar_operand(other)? {
                    let result = match divisor {
                        ScalarOperand::I64(v) => self
                            .inner
                            .checked_div_i64(v)
                            .map_err(|error| create_param_error(error.to_string()))?,
                        ScalarOperand::U64(v) => self
                            .inner
                            .checked_div_u64(v)
                            .map_err(|error| create_param_error(error.to_string()))?,
                        ScalarOperand::F64(v) => self
                            .inner
                            .checked_div_f64(v)
                            .map_err(|error| create_param_error(error.to_string()))?,
                    };
                    return Ok(Py::new(py, Self { inner: result })?.into_py(py));
                }
                Ok(py.NotImplemented().into())
            }

            fn __mod__(&self, other: &Bound<'_, PyAny>) -> PyResult<PyObject> {
                let py = other.py();
                if let Some(divisor) = extract_scalar_operand(other)? {
                    let result = match divisor {
                        ScalarOperand::I64(v) => self
                            .inner
                            .checked_rem_i64(v)
                            .map_err(|error| create_param_error(error.to_string()))?,
                        ScalarOperand::U64(v) => self
                            .inner
                            .checked_rem_u64(v)
                            .map_err(|error| create_param_error(error.to_string()))?,
                        ScalarOperand::F64(v) => self
                            .inner
                            .checked_rem_f64(v)
                            .map_err(|error| create_param_error(error.to_string()))?,
                    };
                    return Ok(Py::new(py, Self { inner: result })?.into_py(py));
                }
                Ok(py.NotImplemented().into())
            }

            fn __neg__(&self) -> PyResult<Self> {
                Ok(Self {
                    inner: self
                        .inner
                        .checked_neg()
                        .map_err(|error| create_param_error(error.to_string()))?,
                })
            }

            fn __abs__(&self) -> Self {
                Self {
                    inner: <$domain>::new(self.inner.to_decimal().abs()),
                }
            }

            $($extra)*
        }
    };
    ($py_type:ident, $domain:ty, $parse_input:ident, $py_name:literal, unsigned, { $($extra:tt)* }) => {
        #[pymethods]
        impl $py_type {
            #[new]
            fn new(value: &Bound<'_, PyAny>) -> PyResult<Self> {
                Ok(Self {
                    inner: $parse_input(value)?,
                })
            }

            #[classattr]
            const ZERO: Self = Self {
                inner: <$domain>::ZERO,
            };

            #[staticmethod]
            fn from_decimal(value: &Bound<'_, PyAny>) -> PyResult<Self> {
                let decimal = parse_python_decimal(value, $py_name)?;
                Ok(Self {
                    inner: <$domain>::from_str(decimal.to_string().as_str())
                        .map_err(|error| create_param_error(error.to_string()))?,
                })
            }

            #[staticmethod]
            fn from_str(value: &str) -> PyResult<Self> {
                Ok(Self {
                    inner: <$domain>::from_str(value)
                        .map_err(|error| create_param_error(error.to_string()))?,
                })
            }

            #[staticmethod]
            fn from_int(value: i64) -> PyResult<Self> {
                Ok(Self {
                    inner: <$domain>::from_str(value.to_string().as_str())
                        .map_err(|error| create_param_error(error.to_string()))?,
                })
            }

            #[staticmethod]
            fn from_u64(value: u64) -> PyResult<Self> {
                Ok(Self {
                    inner: <$domain>::from_str(value.to_string().as_str())
                        .map_err(|error| create_param_error(error.to_string()))?,
                })
            }

            #[staticmethod]
            /// WARNING:
            /// float values are inherently imprecise.
            /// Use decimal/string inputs for external monetary data.
            fn from_float(value: f64) -> PyResult<Self> {
                Ok(Self {
                    inner: <$domain>::from_f64(value)
                        .map_err(|error| create_param_error(error.to_string()))?,
                })
            }

            #[staticmethod]
            fn from_str_rounded(value: &str, scale: u32, strategy: &str) -> PyResult<Self> {
                Ok(Self {
                    inner: <$domain>::from_str_rounded(
                        value,
                        scale,
                        parse_rounding_strategy(strategy)?,
                    )
                    .map_err(|error| create_param_error(error.to_string()))?,
                })
            }

            #[staticmethod]
            fn from_float_rounded(value: f64, scale: u32, strategy: &str) -> PyResult<Self> {
                Ok(Self {
                    inner: <$domain>::from_f64_rounded(
                        value,
                        scale,
                        parse_rounding_strategy(strategy)?,
                    )
                    .map_err(|error| create_param_error(error.to_string()))?,
                })
            }

            #[staticmethod]
            fn from_decimal_rounded(
                value: &Bound<'_, PyAny>,
                scale: u32,
                strategy: &str,
            ) -> PyResult<Self> {
                let decimal = parse_python_decimal(value, $py_name)?;
                Ok(Self {
                    inner: <$domain>::from_decimal_rounded(
                        decimal,
                        scale,
                        parse_rounding_strategy(strategy)?,
                    )
                    .map_err(|error| create_param_error(error.to_string()))?,
                })
            }

            #[getter]
            fn decimal(&self, py: Python<'_>) -> PyResult<PyObject> {
                rust_decimal_to_python_decimal(py, self.inner.to_decimal())
            }

            fn to_json_value(&self) -> String {
                self.inner.to_string()
            }

            fn __repr__(&self) -> String {
                format!("{}(Decimal('{}'))", $py_name, self.inner)
            }

            fn __str__(&self) -> String {
                self.inner.to_string()
            }

            fn __richcmp__(&self, other: &Bound<'_, PyAny>, op: CompareOp) -> PyResult<PyObject> {
                let py = other.py();
                if let Ok(other) = other.extract::<PyRef<'_, $py_type>>() {
                    let result = match op {
                        CompareOp::Lt => self.inner < other.inner,
                        CompareOp::Le => self.inner <= other.inner,
                        CompareOp::Eq => self.inner == other.inner,
                        CompareOp::Ne => self.inner != other.inner,
                        CompareOp::Gt => self.inner > other.inner,
                        CompareOp::Ge => self.inner >= other.inner,
                    };
                    return Ok(result.into_py(py));
                }
                Ok(py.NotImplemented().into())
            }

            fn __hash__(&self) -> u64 {
                let mut hasher = DefaultHasher::new();
                self.inner.to_decimal().hash(&mut hasher);
                hasher.finish()
            }

            fn __bool__(&self) -> bool {
                !self.inner.is_zero()
            }

            /// WARNING:
            /// float values are inherently imprecise.
            /// Use decimal/string inputs for external monetary data.
            fn __float__(&self) -> PyResult<f64> {
                self.inner.to_decimal().to_f64().ok_or_else(|| {
                    PyValueError::new_err(format!("{} cannot be represented as float", $py_name))
                })
            }

            fn __add__(&self, other: &Bound<'_, PyAny>) -> PyResult<PyObject> {
                let py = other.py();
                if let Ok(other) = other.extract::<PyRef<'_, $py_type>>() {
                    let result = self
                        .inner
                        .checked_add(other.inner)
                        .map_err(|error| create_param_error(error.to_string()))?;
                    return Ok(Py::new(py, Self { inner: result })?.into_py(py));
                }
                Ok(py.NotImplemented().into())
            }

            fn __radd__(&self, other: &Bound<'_, PyAny>) -> PyResult<PyObject> {
                self.__add__(other)
            }

            fn __sub__(&self, other: &Bound<'_, PyAny>) -> PyResult<PyObject> {
                let py = other.py();
                if let Ok(other) = other.extract::<PyRef<'_, $py_type>>() {
                    let result = self
                        .inner
                        .checked_sub(other.inner)
                        .map_err(|error| create_param_error(error.to_string()))?;
                    return Ok(Py::new(py, Self { inner: result })?.into_py(py));
                }
                Ok(py.NotImplemented().into())
            }

            fn __rsub__(&self, other: &Bound<'_, PyAny>) -> PyResult<PyObject> {
                let py = other.py();
                if let Ok(other) = other.extract::<PyRef<'_, $py_type>>() {
                    let result = other
                        .inner
                        .checked_sub(self.inner)
                        .map_err(|error| create_param_error(error.to_string()))?;
                    return Ok(Py::new(py, Self { inner: result })?.into_py(py));
                }
                Ok(py.NotImplemented().into())
            }

            fn __mul__(&self, other: &Bound<'_, PyAny>) -> PyResult<PyObject> {
                let py = other.py();
                if let Some(scalar) = extract_scalar_operand(other)? {
                    let result = match scalar {
                        ScalarOperand::I64(v) => self
                            .inner
                            .checked_mul_i64(v)
                            .map_err(|error| create_param_error(error.to_string()))?,
                        ScalarOperand::U64(v) => self
                            .inner
                            .checked_mul_u64(v)
                            .map_err(|error| create_param_error(error.to_string()))?,
                        ScalarOperand::F64(v) => self
                            .inner
                            .checked_mul_f64(v)
                            .map_err(|error| create_param_error(error.to_string()))?,
                    };
                    return Ok(Py::new(py, Self { inner: result })?.into_py(py));
                }
                Ok(py.NotImplemented().into())
            }

            fn __rmul__(&self, other: &Bound<'_, PyAny>) -> PyResult<PyObject> {
                self.__mul__(other)
            }

            fn __truediv__(&self, other: &Bound<'_, PyAny>) -> PyResult<PyObject> {
                let py = other.py();
                if let Some(divisor) = extract_scalar_operand(other)? {
                    let result = match divisor {
                        ScalarOperand::I64(v) => self
                            .inner
                            .checked_div_i64(v)
                            .map_err(|error| create_param_error(error.to_string()))?,
                        ScalarOperand::U64(v) => self
                            .inner
                            .checked_div_u64(v)
                            .map_err(|error| create_param_error(error.to_string()))?,
                        ScalarOperand::F64(v) => self
                            .inner
                            .checked_div_f64(v)
                            .map_err(|error| create_param_error(error.to_string()))?,
                    };
                    return Ok(Py::new(py, Self { inner: result })?.into_py(py));
                }
                Ok(py.NotImplemented().into())
            }

            fn __mod__(&self, other: &Bound<'_, PyAny>) -> PyResult<PyObject> {
                let py = other.py();
                if let Some(divisor) = extract_scalar_operand(other)? {
                    let result = match divisor {
                        ScalarOperand::I64(v) => self
                            .inner
                            .checked_rem_i64(v)
                            .map_err(|error| create_param_error(error.to_string()))?,
                        ScalarOperand::U64(v) => self
                            .inner
                            .checked_rem_u64(v)
                            .map_err(|error| create_param_error(error.to_string()))?,
                        ScalarOperand::F64(v) => self
                            .inner
                            .checked_rem_f64(v)
                            .map_err(|error| create_param_error(error.to_string()))?,
                    };
                    return Ok(Py::new(py, Self { inner: result })?.into_py(py));
                }
                Ok(py.NotImplemented().into())
            }

            $($extra)*
        }
    };
}

impl_decimal_pymethods!(
    PyQuantity,
    Quantity,
    parse_quantity_input,
    "Quantity",
    unsigned,
    {
        fn calculate_volume(&self, price: &PyPrice) -> PyResult<PyVolume> {
            Ok(PyVolume {
                inner: self
                    .inner
                    .calculate_volume(price.inner)
                    .map_err(|error| create_param_error(error.to_string()))?,
            })
        }
    }
);
impl_decimal_pymethods!(PyPrice, Price, parse_price_input, "Price", signed, {
    fn calculate_volume(&self, quantity: &PyQuantity) -> PyResult<PyVolume> {
        Ok(PyVolume {
            inner: self
                .inner
                .calculate_volume(quantity.inner)
                .map_err(|error| create_param_error(error.to_string()))?,
        })
    }
});
impl_decimal_pymethods!(PyPnl, Pnl, parse_pnl_input, "Pnl", signed, {
    #[staticmethod]
    fn from_fee(fee: &PyFee) -> Self {
        Self {
            inner: Pnl::from_fee(fee.inner),
        }
    }

    fn to_cash_flow(&self) -> PyCashFlow {
        PyCashFlow {
            inner: self.inner.to_cash_flow(),
        }
    }

    fn to_position_size(&self) -> PyPositionSize {
        PyPositionSize {
            inner: self.inner.to_position_size(),
        }
    }
});
impl_decimal_pymethods!(PyFee, Fee, parse_fee_input, "Fee", signed, {
    fn to_pnl(&self) -> PyPnl {
        PyPnl {
            inner: self.inner.to_pnl(),
        }
    }

    fn to_position_size(&self) -> PyPositionSize {
        PyPositionSize {
            inner: self.inner.to_position_size(),
        }
    }

    fn to_cash_flow(&self) -> PyCashFlow {
        PyCashFlow {
            inner: self.inner.to_cash_flow(),
        }
    }
});
impl_decimal_pymethods!(PyVolume, Volume, parse_volume_input, "Volume", unsigned, {
    fn to_cash_flow_inflow(&self) -> PyCashFlow {
        PyCashFlow {
            inner: self.inner.to_cash_flow_inflow(),
        }
    }

    fn to_cash_flow_outflow(&self) -> PyCashFlow {
        PyCashFlow {
            inner: self.inner.to_cash_flow_outflow(),
        }
    }

    fn calculate_quantity(&self, price: &PyPrice) -> PyResult<PyQuantity> {
        Ok(PyQuantity {
            inner: self
                .inner
                .calculate_quantity(price.inner)
                .map_err(|error| create_param_error(error.to_string()))?,
        })
    }

    fn to_notional(&self) -> PyNotional {
        PyNotional {
            inner: Notional::from_volume(self.inner),
        }
    }
});
impl_decimal_pymethods!(
    PyNotional,
    Notional,
    parse_notional_input,
    "Notional",
    unsigned,
    {
        #[staticmethod]
        fn from_volume(volume: &PyVolume) -> Self {
            Self {
                inner: Notional::from_volume(volume.inner),
            }
        }

        #[staticmethod]
        fn from_price_quantity(price: &PyPrice, quantity: &PyQuantity) -> PyResult<Self> {
            Ok(Self {
                inner: Notional::from_price_quantity(price.inner, quantity.inner)
                    .map_err(|error| create_param_error(error.to_string()))?,
            })
        }

        fn to_volume(&self) -> PyVolume {
            PyVolume {
                inner: self.inner.to_volume(),
            }
        }

        fn calculate_margin_required(&self, leverage: &Bound<'_, PyAny>) -> PyResult<PyNotional> {
            let lev = parse_leverage_input(leverage)?;
            Ok(PyNotional {
                inner: self
                    .inner
                    .calculate_margin_required(lev)
                    .map_err(|error| create_param_error(error.to_string()))?,
            })
        }
    }
);
impl_decimal_pymethods!(
    PyCashFlow,
    CashFlow,
    parse_cash_flow_input,
    "CashFlow",
    signed,
    {
        #[staticmethod]
        fn from_pnl(pnl: &PyPnl) -> Self {
            Self {
                inner: CashFlow::from_pnl(pnl.inner),
            }
        }

        #[staticmethod]
        fn from_fee(fee: &PyFee) -> Self {
            Self {
                inner: CashFlow::from_fee(fee.inner),
            }
        }

        #[staticmethod]
        fn from_volume_inflow(volume: &PyVolume) -> Self {
            Self {
                inner: CashFlow::from_volume_inflow(volume.inner),
            }
        }

        #[staticmethod]
        fn from_volume_outflow(volume: &PyVolume) -> Self {
            Self {
                inner: CashFlow::from_volume_outflow(volume.inner),
            }
        }
    }
);
impl_decimal_pymethods!(
    PyPositionSize,
    PositionSize,
    parse_position_size_input,
    "PositionSize",
    signed,
    {
        #[staticmethod]
        fn from_quantity_and_side(quantity: &PyQuantity, side: &str) -> PyResult<Self> {
            Ok(Self {
                inner: PositionSize::from_quantity_and_side(quantity.inner, parse_side(side)?),
            })
        }

        #[staticmethod]
        fn from_pnl(pnl: &PyPnl) -> Self {
            Self {
                inner: PositionSize::from_pnl(pnl.inner),
            }
        }

        #[staticmethod]
        fn from_fee(fee: &PyFee) -> Self {
            Self {
                inner: PositionSize::from_fee(fee.inner),
            }
        }

        fn to_open_quantity(&self) -> (PyQuantity, String) {
            let (quantity, side) = self.inner.to_open_quantity();
            (PyQuantity { inner: quantity }, side_name(side).to_owned())
        }

        fn to_close_quantity(&self) -> (PyQuantity, Option<String>) {
            let (quantity, side) = self.inner.to_close_quantity();
            (
                PyQuantity { inner: quantity },
                side.map(|value| side_name(value).to_owned()),
            )
        }

        fn checked_add_quantity(&self, qty: &PyQuantity, side: &str) -> PyResult<Self> {
            Ok(Self {
                inner: self
                    .inner
                    .checked_add_quantity(qty.inner, parse_side(side)?)
                    .map_err(|error| create_param_error(error.to_string()))?,
            })
        }
    }
);

#[pymethods]
impl PyTrade {
    #[new]
    #[pyo3(signature = (*, price, quantity))]
    fn new(price: &Bound<'_, PyAny>, quantity: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: Trade {
                price: parse_price_input(price)?,
                quantity: parse_quantity_input(quantity)?,
            },
        })
    }

    #[getter]
    fn price(&self) -> PyPrice {
        PyPrice {
            inner: self.inner.price,
        }
    }

    #[getter]
    fn quantity(&self) -> PyQuantity {
        PyQuantity {
            inner: self.inner.quantity,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "Trade(price={:?}, quantity={:?})",
            self.inner.price.to_string(),
            self.inner.quantity.to_string()
        )
    }
}

#[pymethods]
impl PyAdjustmentAmount {
    /// Copy / subclass constructor — accepts another AdjustmentAmount instance.
    #[new]
    fn new(other: PyRef<'_, PyAdjustmentAmount>) -> Self {
        Self { inner: other.inner }
    }

    /// Create a delta-type adjustment amount.
    #[staticmethod]
    fn delta(value: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: AdjustmentAmount::Delta(parse_position_size_input(value)?),
        })
    }

    /// Create an absolute-type adjustment amount.
    #[staticmethod]
    fn absolute(value: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: AdjustmentAmount::Absolute(parse_position_size_input(value)?),
        })
    }

    #[getter]
    fn is_delta(&self) -> bool {
        matches!(self.inner, AdjustmentAmount::Delta(_))
    }

    #[getter]
    fn is_absolute(&self) -> bool {
        matches!(self.inner, AdjustmentAmount::Absolute(_))
    }

    #[getter]
    fn as_delta(&self) -> Option<PyPositionSize> {
        match self.inner {
            AdjustmentAmount::Delta(size) => Some(PyPositionSize { inner: size }),
            _ => None,
        }
    }

    #[getter]
    fn as_absolute(&self) -> Option<PyPositionSize> {
        match self.inner {
            AdjustmentAmount::Absolute(size) => Some(PyPositionSize { inner: size }),
            _ => None,
        }
    }

    fn __repr__(&self) -> String {
        match self.inner {
            AdjustmentAmount::Delta(size) => format!("AdjustmentAmount.delta(value={size:?})"),
            AdjustmentAmount::Absolute(size) => {
                format!("AdjustmentAmount.absolute(value={size:?})")
            }
            _ => "AdjustmentAmount(<unsupported>)".to_string(),
        }
    }
}

#[pymethods]
impl PyTradeAmount {
    /// Copy / subclass constructor — accepts another TradeAmount instance.
    #[new]
    fn new(other: PyRef<'_, PyTradeAmount>) -> Self {
        Self { inner: other.inner }
    }

    /// Create a quantity-based trade amount.
    #[staticmethod]
    fn quantity(value: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: TradeAmount::Quantity(parse_quantity_input(value)?),
        })
    }

    /// Create a volume-based trade amount.
    #[staticmethod]
    fn volume(value: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: TradeAmount::Volume(parse_volume_input(value)?),
        })
    }

    #[getter]
    fn is_quantity(&self) -> bool {
        matches!(self.inner, TradeAmount::Quantity(_))
    }

    #[getter]
    fn is_volume(&self) -> bool {
        matches!(self.inner, TradeAmount::Volume(_))
    }

    #[getter]
    fn as_quantity(&self) -> Option<PyQuantity> {
        match self.inner {
            TradeAmount::Quantity(qty) => Some(PyQuantity { inner: qty }),
            _ => None,
        }
    }

    #[getter]
    fn as_volume(&self) -> Option<PyVolume> {
        match self.inner {
            TradeAmount::Volume(vol) => Some(PyVolume { inner: vol }),
            _ => None,
        }
    }

    fn __repr__(&self) -> String {
        match self.inner {
            TradeAmount::Quantity(qty) => format!("TradeAmount.quantity(value={qty:?})"),
            TradeAmount::Volume(vol) => format!("TradeAmount.volume(value={vol:?})"),
            _ => "TradeAmount(<unsupported>)".to_string(),
        }
    }
}

#[pymethods]
impl PyAccountAdjustmentAmount {
    #[new]
    #[pyo3(signature = (*, total = None, reserved = None, pending = None))]
    fn new(
        total: Option<&Bound<'_, PyAny>>,
        reserved: Option<&Bound<'_, PyAny>>,
        pending: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        Ok(Self {
            total: total.map(parse_adjustment_amount_input).transpose()?,
            reserved: reserved.map(parse_adjustment_amount_input).transpose()?,
            pending: pending.map(parse_adjustment_amount_input).transpose()?,
        })
    }

    #[getter]
    fn total(&self) -> Option<PyAdjustmentAmount> {
        self.total.map(|inner| PyAdjustmentAmount { inner })
    }

    #[setter]
    fn set_total(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.total = value.map(parse_adjustment_amount_input).transpose()?;
        Ok(())
    }

    #[getter]
    fn reserved(&self) -> Option<PyAdjustmentAmount> {
        self.reserved.map(|inner| PyAdjustmentAmount { inner })
    }

    #[setter]
    fn set_reserved(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.reserved = value.map(parse_adjustment_amount_input).transpose()?;
        Ok(())
    }

    #[getter]
    fn pending(&self) -> Option<PyAdjustmentAmount> {
        self.pending.map(|inner| PyAdjustmentAmount { inner })
    }

    #[setter]
    fn set_pending(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.pending = value.map(parse_adjustment_amount_input).transpose()?;
        Ok(())
    }

    fn __repr__(&self) -> String {
        format!(
            "AccountAdjustmentAmount(total={:?}, reserved={:?}, pending={:?})",
            self.total().map(|v| v.__repr__()),
            self.reserved().map(|v| v.__repr__()),
            self.pending().map(|v| v.__repr__()),
        )
    }
}

#[pymethods]
impl PyAccountAdjustmentBalanceOperation {
    #[new]
    #[pyo3(signature = (*, asset = None, average_entry_price = None))]
    fn new(
        asset: Option<&Bound<'_, PyAny>>,
        average_entry_price: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        Ok(Self {
            asset: asset.map(parse_asset_input).transpose()?,
            average_entry_price: average_entry_price.map(parse_price_input).transpose()?,
        })
    }

    #[getter]
    fn asset(&self) -> Option<String> {
        self.asset.clone().map(|inner| inner.to_string())
    }

    #[setter]
    fn set_asset(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.asset = value.map(parse_asset_input).transpose()?;
        Ok(())
    }

    #[getter]
    fn average_entry_price(&self) -> Option<PyPrice> {
        self.average_entry_price.map(|inner| PyPrice { inner })
    }

    #[setter]
    fn set_average_entry_price(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.average_entry_price = value.map(parse_price_input).transpose()?;
        Ok(())
    }

    fn __repr__(&self) -> String {
        format!(
            "AccountAdjustmentBalanceOperation(asset={:?}, average_entry_price={:?})",
            self.asset(),
            self.average_entry_price().map(|v| v.inner.to_string()),
        )
    }
}

#[pymethods]
impl PyAccountAdjustmentPositionOperation {
    #[new]
    #[pyo3(signature = (*, underlying_asset = None, settlement_asset = None, collateral_asset = None, average_entry_price = None, mode = None, leverage = None))]
    fn new(
        underlying_asset: Option<&Bound<'_, PyAny>>,
        settlement_asset: Option<&Bound<'_, PyAny>>,
        collateral_asset: Option<&Bound<'_, PyAny>>,
        average_entry_price: Option<&Bound<'_, PyAny>>,
        mode: Option<&Bound<'_, PyAny>>,
        leverage: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let assets_are_partial = underlying_asset.is_some() ^ settlement_asset.is_some();
        if assets_are_partial {
            return Err(PyValueError::new_err(
                "underlying_asset and settlement_asset must be provided together",
            ));
        }

        Ok(Self {
            underlying_asset: underlying_asset.map(parse_asset_input).transpose()?,
            settlement_asset: settlement_asset.map(parse_asset_input).transpose()?,
            collateral_asset: collateral_asset.map(parse_asset_input).transpose()?,
            average_entry_price: average_entry_price.map(parse_price_input).transpose()?,
            mode: mode.map(parse_position_mode_input).transpose()?,
            leverage: leverage.map(parse_leverage_input).transpose()?,
        })
    }

    #[getter]
    fn underlying_asset(&self) -> Option<String> {
        self.underlying_asset.clone().map(|inner| inner.to_string())
    }

    #[setter]
    fn set_underlying_asset(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.underlying_asset = value.map(parse_asset_input).transpose()?;
        Ok(())
    }

    #[getter]
    fn settlement_asset(&self) -> Option<String> {
        self.settlement_asset.clone().map(|inner| inner.to_string())
    }

    #[setter]
    fn set_settlement_asset(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.settlement_asset = value.map(parse_asset_input).transpose()?;
        Ok(())
    }

    #[getter]
    fn collateral_asset(&self) -> Option<String> {
        self.collateral_asset.clone().map(|inner| inner.to_string())
    }

    #[setter]
    fn set_collateral_asset(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.collateral_asset = value.map(parse_asset_input).transpose()?;
        Ok(())
    }

    #[getter]
    fn average_entry_price(&self) -> Option<PyPrice> {
        self.average_entry_price.map(|inner| PyPrice { inner })
    }

    #[setter]
    fn set_average_entry_price(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.average_entry_price = value.map(parse_price_input).transpose()?;
        Ok(())
    }

    #[getter]
    fn mode(&self, py: Python<'_>) -> PyResult<Option<PyObject>> {
        self.mode
            .map(|mode| {
                PyModule::import_bound(py, "openpit.param")?
                    .getattr("PositionMode")?
                    .call1((position_mode_name(mode),))
                    .map(|obj| obj.into_py(py))
            })
            .transpose()
    }

    #[setter]
    fn set_mode(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.mode = value.map(parse_position_mode_input).transpose()?;
        Ok(())
    }

    #[getter]
    fn leverage(&self) -> Option<PyLeverage> {
        self.leverage.map(|inner| PyLeverage { inner })
    }

    #[setter]
    fn set_leverage(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.leverage = value.map(parse_leverage_input).transpose()?;
        Ok(())
    }

    fn __repr__(&self, py: Python<'_>) -> String {
        format!(
            "AccountAdjustmentPositionOperation(underlying_asset={:?}, settlement_asset={:?}, collateral_asset={:?}, average_entry_price={:?}, mode={:?}, leverage={:?})",
            self.underlying_asset(),
            self.settlement_asset(),
            self.collateral_asset(),
            self.average_entry_price().map(|v| v.inner.to_string()),
            self.mode(py),
            self.leverage().map(|v| v.value()),
        )
    }
}

#[pymethods]
impl PyAccountAdjustmentBounds {
    #[new]
    #[pyo3(signature = (*, total_upper = None, total_lower = None, reserved_upper = None, reserved_lower = None, pending_upper = None, pending_lower = None))]
    fn new(
        total_upper: Option<&Bound<'_, PyAny>>,
        total_lower: Option<&Bound<'_, PyAny>>,
        reserved_upper: Option<&Bound<'_, PyAny>>,
        reserved_lower: Option<&Bound<'_, PyAny>>,
        pending_upper: Option<&Bound<'_, PyAny>>,
        pending_lower: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        Ok(Self {
            total_upper: total_upper.map(parse_position_size_input).transpose()?,
            total_lower: total_lower.map(parse_position_size_input).transpose()?,
            reserved_upper: reserved_upper.map(parse_position_size_input).transpose()?,
            reserved_lower: reserved_lower.map(parse_position_size_input).transpose()?,
            pending_upper: pending_upper.map(parse_position_size_input).transpose()?,
            pending_lower: pending_lower.map(parse_position_size_input).transpose()?,
        })
    }

    #[getter]
    fn total_upper(&self) -> Option<PyPositionSize> {
        self.total_upper.map(|inner| PyPositionSize { inner })
    }
    #[setter]
    fn set_total_upper(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.total_upper = value.map(parse_position_size_input).transpose()?;
        Ok(())
    }
    #[getter]
    fn total_lower(&self) -> Option<PyPositionSize> {
        self.total_lower.map(|inner| PyPositionSize { inner })
    }
    #[setter]
    fn set_total_lower(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.total_lower = value.map(parse_position_size_input).transpose()?;
        Ok(())
    }
    #[getter]
    fn reserved_upper(&self) -> Option<PyPositionSize> {
        self.reserved_upper.map(|inner| PyPositionSize { inner })
    }
    #[setter]
    fn set_reserved_upper(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.reserved_upper = value.map(parse_position_size_input).transpose()?;
        Ok(())
    }
    #[getter]
    fn reserved_lower(&self) -> Option<PyPositionSize> {
        self.reserved_lower.map(|inner| PyPositionSize { inner })
    }
    #[setter]
    fn set_reserved_lower(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.reserved_lower = value.map(parse_position_size_input).transpose()?;
        Ok(())
    }
    #[getter]
    fn pending_upper(&self) -> Option<PyPositionSize> {
        self.pending_upper.map(|inner| PyPositionSize { inner })
    }
    #[setter]
    fn set_pending_upper(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.pending_upper = value.map(parse_position_size_input).transpose()?;
        Ok(())
    }
    #[getter]
    fn pending_lower(&self) -> Option<PyPositionSize> {
        self.pending_lower.map(|inner| PyPositionSize { inner })
    }
    #[setter]
    fn set_pending_lower(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.pending_lower = value.map(parse_position_size_input).transpose()?;
        Ok(())
    }

    fn __repr__(&self) -> String {
        format!(
            "AccountAdjustmentBounds(total_upper={:?}, total_lower={:?}, reserved_upper={:?}, reserved_lower={:?}, pending_upper={:?}, pending_lower={:?})",
            self.total_upper().map(|v| v.inner.to_string()),
            self.total_lower().map(|v| v.inner.to_string()),
            self.reserved_upper().map(|v| v.inner.to_string()),
            self.reserved_lower().map(|v| v.inner.to_string()),
            self.pending_upper().map(|v| v.inner.to_string()),
            self.pending_lower().map(|v| v.inner.to_string()),
        )
    }
}

#[pymethods]
impl PyAccountAdjustment {
    #[new]
    #[pyo3(signature = (*, operation = None, amount = None, bounds = None))]
    fn new(
        py: Python<'_>,
        operation: Option<&Bound<'_, PyAny>>,
        amount: Option<&Bound<'_, PyAny>>,
        bounds: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        Ok(Self {
            operation: operation
                .map(|value| parse_account_adjustment_operation(py, value))
                .transpose()?,
            amount: amount
                .map(|v| {
                    v.extract::<PyAccountAdjustmentAmount>()
                        .map(|obj| Py::new(py, obj))
                        .map_err(|_| {
                            PyTypeError::new_err(
                                "amount must be openpit.core.AccountAdjustmentAmount",
                            )
                        })
                        .and_then(|r| r)
                })
                .transpose()?,
            bounds: bounds
                .map(|v| {
                    v.extract::<PyAccountAdjustmentBounds>()
                        .map(|obj| Py::new(py, obj))
                        .map_err(|_| {
                            PyTypeError::new_err(
                                "bounds must be openpit.core.AccountAdjustmentBounds",
                            )
                        })
                        .and_then(|r| r)
                })
                .transpose()?,
        })
    }

    #[getter]
    fn operation(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        self.operation.as_ref().map(|op| match op {
            PyAccountAdjustmentOperation::Balance(value) => value.clone_ref(py).into_any(),
            PyAccountAdjustmentOperation::Position(value) => value.clone_ref(py).into_any(),
        })
    }

    #[setter]
    fn set_operation(&mut self, py: Python<'_>, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.operation = value
            .map(|v| parse_account_adjustment_operation(py, v))
            .transpose()?;
        Ok(())
    }

    #[getter]
    fn amount(&self, py: Python<'_>) -> Option<Py<PyAccountAdjustmentAmount>> {
        self.amount.as_ref().map(|v| v.clone_ref(py))
    }

    #[setter]
    fn set_amount(&mut self, py: Python<'_>, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.amount = value
            .map(|v| {
                v.extract::<PyAccountAdjustmentAmount>()
                    .map(|obj| Py::new(py, obj))
                    .map_err(|_| {
                        PyTypeError::new_err("amount must be openpit.core.AccountAdjustmentAmount")
                    })
                    .and_then(|r| r)
            })
            .transpose()?;
        Ok(())
    }

    #[getter]
    fn bounds(&self, py: Python<'_>) -> Option<Py<PyAccountAdjustmentBounds>> {
        self.bounds.as_ref().map(|v| v.clone_ref(py))
    }

    #[setter]
    fn set_bounds(&mut self, py: Python<'_>, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.bounds = value
            .map(|v| {
                v.extract::<PyAccountAdjustmentBounds>()
                    .map(|obj| Py::new(py, obj))
                    .map_err(|_| {
                        PyTypeError::new_err("bounds must be openpit.core.AccountAdjustmentBounds")
                    })
                    .and_then(|r| r)
            })
            .transpose()?;
        Ok(())
    }

    fn __repr__(&self, py: Python<'_>) -> String {
        let operation = self.operation.as_ref().map(|op| match op {
            PyAccountAdjustmentOperation::Balance(value) => value.bind(py).borrow().__repr__(),
            PyAccountAdjustmentOperation::Position(value) => value.bind(py).borrow().__repr__(py),
        });
        let amount = self
            .amount
            .as_ref()
            .map(|value| value.bind(py).borrow().__repr__());
        let bounds = self
            .bounds
            .as_ref()
            .map(|value| value.bind(py).borrow().__repr__());
        format!(
            "AccountAdjustment(operation={:?}, amount={:?}, bounds={:?})",
            operation, amount, bounds
        )
    }
}

#[pyclass(name = "PreTradeRequest", module = "openpit.pretrade", unsendable)]
struct PyRequest {
    inner: RefCell<Option<PreTradeRequest<Order>>>,
}

#[pymethods]
impl PyRequest {
    fn execute(&self, py: Python<'_>) -> PyResult<PyExecuteResult> {
        let request = self
            .inner
            .borrow_mut()
            .take()
            .ok_or_else(|| PyRuntimeError::new_err("request has already been executed"))?;
        clear_python_callback_error();

        match request.execute() {
            Ok(reservation) => {
                if let Some(error) = take_python_callback_error() {
                    return Err(error);
                }
                Ok(PyExecuteResult {
                    reservation: Some(Py::new(
                        py,
                        PyReservation {
                            inner: RefCell::new(Some(reservation)),
                        },
                    )?),
                    rejects: Vec::new(),
                })
            }
            Err(rejects) => {
                if let Some(error) = take_python_callback_error() {
                    return Err(error);
                }
                Ok(PyExecuteResult {
                    reservation: None,
                    rejects: rejects.iter().map(convert_reject).collect(),
                })
            }
        }
    }
}

#[pyclass(name = "PreTradeReservation", module = "openpit.pretrade", unsendable)]
struct PyReservation {
    inner: RefCell<Option<PreTradeReservation>>,
}

#[pyclass(name = "PreTradeLock", module = "openpit.pretrade", subclass)]
#[derive(Clone)]
struct PyPreTradeLock {
    inner: PreTradeLock,
}

#[pymethods]
impl PyReservation {
    fn lock(&self) -> PyResult<PyPreTradeLock> {
        let reservation_ref = self.inner.borrow();
        let reservation = reservation_ref
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err("reservation has already been finalized"))?;
        Ok(PyPreTradeLock {
            inner: *reservation.lock(),
        })
    }

    fn commit(&self) -> PyResult<()> {
        let mut reservation = self.take_reservation()?;
        reservation.commit();
        Ok(())
    }

    fn rollback(&self) -> PyResult<()> {
        let mut reservation = self.take_reservation()?;
        reservation.rollback();
        Ok(())
    }
}

#[pymethods]
impl PyPreTradeLock {
    #[new]
    #[pyo3(signature = (price = None))]
    fn new(price: Option<&Bound<'_, PyAny>>) -> PyResult<Self> {
        Ok(Self {
            inner: PreTradeLock::new(price.map(parse_price_input).transpose()?),
        })
    }

    #[getter]
    fn price(&self) -> Option<PyPrice> {
        self.inner.price().map(|inner| PyPrice { inner })
    }

    fn __repr__(&self) -> String {
        format!(
            "PreTradeLock(price={:?})",
            self.price().map(|price| price.inner.to_string())
        )
    }
}

impl PyReservation {
    fn take_reservation(&self) -> PyResult<PreTradeReservation> {
        self.inner
            .borrow_mut()
            .take()
            .ok_or_else(|| PyRuntimeError::new_err("reservation has already been finalized"))
    }
}

#[pyclass(name = "ExecutionReportOperation", module = "openpit.core", subclass)]
#[derive(Clone)]
struct PyExecutionReportOperation {
    underlying_asset: Option<Asset>,
    settlement_asset: Option<Asset>,
    account_id: Option<AccountId>,
    side: Option<Side>,
}

#[pymethods]
impl PyExecutionReportOperation {
    #[new]
    #[pyo3(signature = (*, underlying_asset = None, settlement_asset = None, account_id = None, side = None))]
    fn new(
        underlying_asset: Option<&Bound<'_, PyAny>>,
        settlement_asset: Option<&Bound<'_, PyAny>>,
        account_id: Option<&Bound<'_, PyAny>>,
        side: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let assets_are_partial = underlying_asset.is_some() ^ settlement_asset.is_some();
        if assets_are_partial {
            return Err(PyValueError::new_err(
                "underlying_asset and settlement_asset must be provided together",
            ));
        }
        Ok(Self {
            underlying_asset: underlying_asset.map(parse_asset_input).transpose()?,
            settlement_asset: settlement_asset.map(parse_asset_input).transpose()?,
            account_id: account_id.map(parse_account_id_input).transpose()?,
            side: side.map(parse_side_input).transpose()?,
        })
    }

    #[getter]
    fn underlying_asset(&self) -> Option<String> {
        self.underlying_asset.clone().map(|inner| inner.to_string())
    }

    #[setter]
    fn set_underlying_asset(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.underlying_asset = value.map(parse_asset_input).transpose()?;
        Ok(())
    }

    #[getter]
    fn settlement_asset(&self) -> Option<String> {
        self.settlement_asset.clone().map(|inner| inner.to_string())
    }

    #[setter]
    fn set_settlement_asset(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.settlement_asset = value.map(parse_asset_input).transpose()?;
        Ok(())
    }

    #[getter]
    fn account_id(&self) -> Option<PyAccountId> {
        self.account_id.map(|inner| PyAccountId { inner })
    }

    #[setter]
    fn set_account_id(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.account_id = value.map(parse_account_id_input).transpose()?;
        Ok(())
    }

    #[getter]
    fn side(&self, py: Python<'_>) -> PyResult<Option<PyObject>> {
        self.side
            .map(|side| {
                PyModule::import_bound(py, "openpit.param")?
                    .getattr("Side")?
                    .call1((side_name(side),))
                    .map(|obj| obj.into_py(py))
            })
            .transpose()
    }

    #[setter]
    fn set_side(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.side = value.map(parse_side_input).transpose()?;
        Ok(())
    }

    fn __repr__(&self, py: Python<'_>) -> String {
        format!(
            "ExecutionReportOperation(underlying_asset={:?}, settlement_asset={:?}, account_id={:?}, side={:?})",
            self.underlying_asset(),
            self.settlement_asset(),
            self.account_id().map(|a| a.inner.as_u64()),
            self.side(py),
        )
    }
}

#[pyclass(name = "FinancialImpact", module = "openpit.core", subclass)]
#[derive(Clone)]
struct PyFinancialImpact {
    pnl: Pnl,
    fee: Fee,
}

#[pymethods]
impl PyFinancialImpact {
    #[new]
    #[pyo3(signature = (*, pnl, fee))]
    fn new(pnl: &Bound<'_, PyAny>, fee: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            pnl: parse_pnl_input(pnl)?,
            fee: parse_fee_input(fee)?,
        })
    }

    #[getter]
    fn pnl(&self) -> PyPnl {
        PyPnl { inner: self.pnl }
    }

    #[setter]
    fn set_pnl(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.pnl = parse_pnl_input(value)?;
        Ok(())
    }

    #[getter]
    fn fee(&self) -> PyFee {
        PyFee { inner: self.fee }
    }

    #[setter]
    fn set_fee(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.fee = parse_fee_input(value)?;
        Ok(())
    }

    fn __repr__(&self) -> String {
        format!(
            "FinancialImpact(pnl={:?}, fee={:?})",
            self.pnl().inner.to_string(),
            self.fee().inner.to_string(),
        )
    }
}

#[pyclass(name = "ExecutionReportFillDetails", module = "openpit.core", subclass)]
#[derive(Clone)]
struct PyExecutionReportFillDetails {
    last_trade: Option<Trade>,
    leaves_quantity: Option<Quantity>,
    lock: PreTradeLock,
    is_final: Option<bool>,
}

#[pymethods]
impl PyExecutionReportFillDetails {
    #[new]
    #[pyo3(signature = (*, last_trade = None, leaves_quantity = None, lock, is_final = None))]
    fn new(
        last_trade: Option<&Bound<'_, PyAny>>,
        leaves_quantity: Option<&Bound<'_, PyAny>>,
        lock: &Bound<'_, PyAny>,
        is_final: Option<bool>,
    ) -> PyResult<Self> {
        Ok(Self {
            last_trade: last_trade
                .map(|value| {
                    value
                        .extract::<PyRef<'_, PyTrade>>()
                        .map(|value| value.inner)
                })
                .transpose()?,
            leaves_quantity: leaves_quantity.map(parse_quantity_input).transpose()?,
            lock: lock.extract::<PyRef<'_, PyPreTradeLock>>()?.inner,
            is_final,
        })
    }

    #[getter]
    fn last_trade(&self) -> Option<PyTrade> {
        self.last_trade.map(|inner| PyTrade { inner })
    }

    #[setter]
    fn set_last_trade(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.last_trade = value
            .map(|value| {
                value
                    .extract::<PyRef<'_, PyTrade>>()
                    .map(|value| value.inner)
            })
            .transpose()?;
        Ok(())
    }

    #[getter]
    fn leaves_quantity(&self) -> Option<PyQuantity> {
        self.leaves_quantity.map(|inner| PyQuantity { inner })
    }

    #[setter]
    fn set_leaves_quantity(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.leaves_quantity = value.map(parse_quantity_input).transpose()?;
        Ok(())
    }

    #[getter]
    fn lock(&self) -> PyPreTradeLock {
        PyPreTradeLock { inner: self.lock }
    }

    #[setter]
    fn set_lock(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.lock = value.extract::<PyRef<'_, PyPreTradeLock>>()?.inner;
        Ok(())
    }

    #[getter]
    /// Whether this report closes the order's report stream.
    /// The order is filled, cancelled, or rejected.
    fn is_final(&self) -> Option<bool> {
        self.is_final
    }

    #[setter]
    fn set_is_final(&mut self, value: Option<bool>) {
        self.is_final = value;
    }

    fn __repr__(&self) -> String {
        format!(
            "ExecutionReportFillDetails(last_trade={:?}, leaves_quantity={:?}, lock={:?}, is_final={:?})",
            self.last_trade().map(|trade| trade.__repr__()),
            self.leaves_quantity().map(|quantity| quantity.inner.to_string()),
            self.lock().__repr__(),
            self.is_final(),
        )
    }
}

#[pyclass(
    name = "ExecutionReportPositionImpact",
    module = "openpit.core",
    subclass
)]
#[derive(Clone)]
struct PyExecutionReportPositionImpact {
    position_effect: Option<PositionEffect>,
    position_side: Option<PositionSide>,
}

#[pymethods]
impl PyExecutionReportPositionImpact {
    #[new]
    #[pyo3(signature = (*, position_effect = None, position_side = None))]
    fn new(
        position_effect: Option<&Bound<'_, PyAny>>,
        position_side: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        Ok(Self {
            position_effect: position_effect
                .map(parse_position_effect_input)
                .transpose()?,
            position_side: position_side.map(parse_position_side_input).transpose()?,
        })
    }

    #[getter]
    fn position_effect(&self, py: Python<'_>) -> PyResult<Option<PyObject>> {
        self.position_effect
            .map(|effect| {
                PyModule::import_bound(py, "openpit.param")?
                    .getattr("PositionEffect")?
                    .call1((position_effect_name(effect),))
                    .map(|obj| obj.into_py(py))
            })
            .transpose()
    }

    #[setter]
    fn set_position_effect(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.position_effect = value.map(parse_position_effect_input).transpose()?;
        Ok(())
    }

    #[getter]
    fn position_side(&self, py: Python<'_>) -> PyResult<Option<PyObject>> {
        self.position_side
            .map(|side| {
                PyModule::import_bound(py, "openpit.param")?
                    .getattr("PositionSide")?
                    .call1((position_side_name(side),))
                    .map(|obj| obj.into_py(py))
            })
            .transpose()
    }

    #[setter]
    fn set_position_side(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.position_side = value.map(parse_position_side_input).transpose()?;
        Ok(())
    }

    fn __repr__(&self, py: Python<'_>) -> String {
        format!(
            "ExecutionReportPositionImpact(position_effect={:?}, position_side={:?})",
            self.position_effect(py),
            self.position_side(py),
        )
    }
}

#[pyclass(
    name = "ExecutionReport",
    module = "openpit.core",
    subclass,
    unsendable
)]
struct PyExecutionReport {
    operation: Option<Py<PyExecutionReportOperation>>,
    financial_impact: Option<Py<PyFinancialImpact>>,
    fill: Option<Py<PyExecutionReportFillDetails>>,
    position_impact: Option<Py<PyExecutionReportPositionImpact>>,
}

#[pymethods]
impl PyExecutionReport {
    #[new]
    #[pyo3(signature = (*, operation = None, financial_impact = None, fill = None, position_impact = None))]
    fn new(
        py: Python<'_>,
        operation: Option<&Bound<'_, PyAny>>,
        financial_impact: Option<&Bound<'_, PyAny>>,
        fill: Option<&Bound<'_, PyAny>>,
        position_impact: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let operation = operation
            .map(|v| {
                v.extract::<PyExecutionReportOperation>()
                    .map(|op| Py::new(py, op))
                    .map_err(|_| {
                        PyTypeError::new_err(
                            "operation must be openpit.core.ExecutionReportOperation",
                        )
                    })
                    .and_then(|r| r)
            })
            .transpose()?;
        let financial_impact = financial_impact
            .map(|v| {
                v.extract::<PyFinancialImpact>()
                    .map(|fi| Py::new(py, fi))
                    .map_err(|_| {
                        PyTypeError::new_err(
                            "financial_impact must be openpit.core.FinancialImpact",
                        )
                    })
                    .and_then(|r| r)
            })
            .transpose()?;
        let fill = fill
            .map(|v| {
                v.extract::<PyExecutionReportFillDetails>()
                    .map(|f| Py::new(py, f))
                    .map_err(|_| {
                        PyTypeError::new_err("fill must be openpit.core.ExecutionReportFillDetails")
                    })
                    .and_then(|r| r)
            })
            .transpose()?;
        let position_impact = position_impact
            .map(|v| {
                v.extract::<PyExecutionReportPositionImpact>()
                    .map(|pi| Py::new(py, pi))
                    .map_err(|_| {
                        PyTypeError::new_err(
                            "position_impact must be openpit.core.ExecutionReportPositionImpact",
                        )
                    })
                    .and_then(|r| r)
            })
            .transpose()?;
        Ok(Self {
            operation,
            financial_impact,
            fill,
            position_impact,
        })
    }

    #[getter]
    fn operation(&self, py: Python<'_>) -> Option<Py<PyExecutionReportOperation>> {
        self.operation.as_ref().map(|v| v.clone_ref(py))
    }

    #[setter]
    fn set_operation(&mut self, py: Python<'_>, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.operation = value
            .map(|v| {
                v.extract::<PyExecutionReportOperation>()
                    .map(|op| Py::new(py, op))
                    .map_err(|_| {
                        PyTypeError::new_err(
                            "operation must be openpit.core.ExecutionReportOperation",
                        )
                    })
                    .and_then(|r| r)
            })
            .transpose()?;
        Ok(())
    }

    #[getter]
    fn financial_impact(&self, py: Python<'_>) -> Option<Py<PyFinancialImpact>> {
        self.financial_impact.as_ref().map(|v| v.clone_ref(py))
    }

    #[setter]
    fn set_financial_impact(
        &mut self,
        py: Python<'_>,
        value: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        self.financial_impact = value
            .map(|v| {
                v.extract::<PyFinancialImpact>()
                    .map(|fi| Py::new(py, fi))
                    .map_err(|_| {
                        PyTypeError::new_err(
                            "financial_impact must be openpit.core.FinancialImpact",
                        )
                    })
                    .and_then(|r| r)
            })
            .transpose()?;
        Ok(())
    }

    #[getter]
    fn fill(&self, py: Python<'_>) -> Option<Py<PyExecutionReportFillDetails>> {
        self.fill.as_ref().map(|v| v.clone_ref(py))
    }

    #[setter]
    fn set_fill(&mut self, py: Python<'_>, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.fill = value
            .map(|v| {
                v.extract::<PyExecutionReportFillDetails>()
                    .map(|f| Py::new(py, f))
                    .map_err(|_| {
                        PyTypeError::new_err("fill must be openpit.core.ExecutionReportFillDetails")
                    })
                    .and_then(|r| r)
            })
            .transpose()?;
        Ok(())
    }

    #[getter]
    fn position_impact(&self, py: Python<'_>) -> Option<Py<PyExecutionReportPositionImpact>> {
        self.position_impact.as_ref().map(|v| v.clone_ref(py))
    }

    #[setter]
    fn set_position_impact(
        &mut self,
        py: Python<'_>,
        value: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        self.position_impact = value
            .map(|v| {
                v.extract::<PyExecutionReportPositionImpact>()
                    .map(|pi| Py::new(py, pi))
                    .map_err(|_| {
                        PyTypeError::new_err(
                            "position_impact must be openpit.core.ExecutionReportPositionImpact",
                        )
                    })
                    .and_then(|r| r)
            })
            .transpose()?;
        Ok(())
    }

    fn __repr__(&self, py: Python<'_>) -> String {
        let operation = self
            .operation
            .as_ref()
            .map(|v| v.bind(py).borrow().__repr__(py));
        let financial_impact = self
            .financial_impact
            .as_ref()
            .map(|v| v.bind(py).borrow().__repr__());
        let fill = self.fill.as_ref().map(|v| v.bind(py).borrow().__repr__());
        let position_impact = self
            .position_impact
            .as_ref()
            .map(|v| v.bind(py).borrow().__repr__(py));
        format!(
            "ExecutionReport(operation={:?}, financial_impact={:?}, fill={:?}, position_impact={:?})",
            operation, financial_impact, fill, position_impact,
        )
    }
}

#[pyclass(name = "PostTradeResult", module = "openpit.pretrade")]
#[derive(Clone, Copy)]
struct PyPostTradeResult {
    inner: PostTradeResult,
}

#[pymethods]
impl PyPostTradeResult {
    #[getter]
    fn kill_switch_triggered(&self) -> bool {
        self.inner.kill_switch_triggered
    }

    fn __repr__(&self) -> String {
        format!(
            "PostTradeResult(kill_switch_triggered={})",
            self.kill_switch_triggered()
        )
    }
}

fn parse_side(value: &str) -> PyResult<Side> {
    match value.trim().to_ascii_lowercase().as_str() {
        "buy" => Ok(Side::Buy),
        "sell" => Ok(Side::Sell),
        other => Err(PyValueError::new_err(format!(
            "invalid side {other:?}; expected 'buy' or 'sell'"
        ))),
    }
}

// Leaf/value parsing and validation source of truth for Python bindings.
// Python layer keeps only structural aggregate checks and delegates value
// semantics to these native parsers/setters.
fn parse_account_id_input(value: &Bound<'_, PyAny>) -> PyResult<AccountId> {
    if let Ok(v) = value.extract::<PyRef<'_, PyAccountId>>() {
        return Ok(v.inner);
    }
    Err(PyTypeError::new_err(
        "account_id must be openpit.param.AccountId",
    ))
}

fn parse_side_input(value: &Bound<'_, PyAny>) -> PyResult<Side> {
    let side = value
        .extract::<String>()
        .map_err(|_| PyTypeError::new_err("side must be a str or openpit.Side"))?;
    parse_side(&side).map_err(|error| PyTypeError::new_err(error.to_string()))
}

fn side_name(value: Side) -> &'static str {
    match value {
        Side::Buy => "buy",
        Side::Sell => "sell",
    }
}

fn parse_position_side(value: &str) -> PyResult<PositionSide> {
    match value.trim().to_ascii_lowercase().as_str() {
        "long" => Ok(PositionSide::Long),
        "short" => Ok(PositionSide::Short),
        other => Err(PyValueError::new_err(format!(
            "invalid position side {other:?}; expected 'long' or 'short'"
        ))),
    }
}

fn parse_position_side_input(value: &Bound<'_, PyAny>) -> PyResult<PositionSide> {
    let position_side = value
        .extract::<String>()
        .map_err(|_| PyTypeError::new_err("position_side must be a str or openpit.PositionSide"))?;
    parse_position_side(&position_side).map_err(|error| PyTypeError::new_err(error.to_string()))
}

fn position_side_name(value: PositionSide) -> &'static str {
    match value {
        PositionSide::Long => "long",
        PositionSide::Short => "short",
    }
}

fn parse_position_effect(value: &str) -> PyResult<PositionEffect> {
    match value.trim().to_ascii_lowercase().as_str() {
        "open" => Ok(PositionEffect::Open),
        "close" => Ok(PositionEffect::Close),
        other => Err(PyValueError::new_err(format!(
            "invalid position effect {other:?}; expected 'open' or 'close'"
        ))),
    }
}

fn parse_position_effect_input(value: &Bound<'_, PyAny>) -> PyResult<PositionEffect> {
    let position_effect = value.extract::<String>().map_err(|_| {
        PyTypeError::new_err("position_effect must be a str or openpit.PositionEffect")
    })?;
    parse_position_effect(&position_effect).map_err(|error| PyTypeError::new_err(error.to_string()))
}

fn position_effect_name(value: PositionEffect) -> &'static str {
    match value {
        PositionEffect::Open => "open",
        PositionEffect::Close => "close",
    }
}

fn parse_position_mode(value: &str) -> PyResult<PositionMode> {
    match value.trim().to_ascii_lowercase().as_str() {
        "netting" => Ok(PositionMode::Netting),
        "hedged" => Ok(PositionMode::Hedged),
        other => Err(PyValueError::new_err(format!(
            "invalid position mode {other:?}; expected 'netting' or 'hedged'"
        ))),
    }
}

fn parse_rounding_strategy(value: &str) -> PyResult<RoundingStrategy> {
    match value.trim().to_ascii_lowercase().as_str() {
        "midpointnearesteven" => Ok(RoundingStrategy::MidpointNearestEven),
        "midpointawayfromzero" => Ok(RoundingStrategy::MidpointAwayFromZero),
        "up" => Ok(RoundingStrategy::Up),
        "down" => Ok(RoundingStrategy::Down),
        other => Err(PyValueError::new_err(format!(
            "invalid rounding strategy {other:?}; expected 'MidpointNearestEven', 'MidpointAwayFromZero', 'Up', or 'Down'"
        ))),
    }
}

fn rounding_strategy_name(value: RoundingStrategy) -> &'static str {
    match value {
        RoundingStrategy::MidpointNearestEven => "MidpointNearestEven",
        RoundingStrategy::MidpointAwayFromZero => "MidpointAwayFromZero",
        RoundingStrategy::Up => "Up",
        RoundingStrategy::Down => "Down",
    }
}

fn parse_position_mode_input(value: &Bound<'_, PyAny>) -> PyResult<PositionMode> {
    let mode = value
        .extract::<String>()
        .map_err(|_| PyTypeError::new_err("mode must be a str or openpit.param.PositionMode"))?;
    parse_position_mode(&mode).map_err(|error| PyTypeError::new_err(error.to_string()))
}

fn position_mode_name(value: PositionMode) -> &'static str {
    match value {
        PositionMode::Netting => "netting",
        PositionMode::Hedged => "hedged",
    }
}

fn parse_adjustment_amount_input(value: &Bound<'_, PyAny>) -> PyResult<AdjustmentAmount> {
    if let Ok(value) = value.extract::<PyRef<'_, PyAdjustmentAmount>>() {
        return Ok(value.inner);
    }
    Err(PyTypeError::new_err(
        "value must be openpit.param.AdjustmentAmount",
    ))
}

fn parse_account_adjustment_operation(
    py: Python<'_>,
    value: &Bound<'_, PyAny>,
) -> PyResult<PyAccountAdjustmentOperation> {
    if let Ok(op) = value.extract::<PyAccountAdjustmentBalanceOperation>() {
        return Ok(PyAccountAdjustmentOperation::Balance(Py::new(py, op)?));
    }
    if let Ok(op) = value.extract::<PyAccountAdjustmentPositionOperation>() {
        return Ok(PyAccountAdjustmentOperation::Position(Py::new(py, op)?));
    }
    Err(PyTypeError::new_err(
        "operation must be openpit.core.AccountAdjustmentBalanceOperation or openpit.core.AccountAdjustmentPositionOperation",
    ))
}

fn parse_quantity(value: &str) -> PyResult<Quantity> {
    Quantity::from_str(value).map_err(|error| create_param_error(error.to_string()))
}

fn parse_asset(value: &str) -> PyResult<Asset> {
    Asset::new(value).map_err(|error| PyValueError::new_err(error.to_string()))
}

fn parse_asset_input(value: &Bound<'_, PyAny>) -> PyResult<Asset> {
    if let Ok(value) = value.extract::<String>() {
        return parse_asset(&value);
    }
    Err(PyTypeError::new_err("asset must be a str"))
}

#[pyfunction]
fn _validate_asset(value: &str) -> PyResult<()> {
    parse_asset(value).map(|_| ())
}

fn parse_price(value: &str) -> PyResult<Price> {
    Price::from_str(value).map_err(|error| create_param_error(error.to_string()))
}

fn parse_pnl(value: &str) -> PyResult<Pnl> {
    Pnl::from_str(value).map_err(|error| create_param_error(error.to_string()))
}

fn parse_fee(value: &str) -> PyResult<Fee> {
    Fee::from_str(value).map_err(|error| create_param_error(error.to_string()))
}

fn parse_volume(value: &str) -> PyResult<Volume> {
    Volume::from_str(value).map_err(|error| create_param_error(error.to_string()))
}

fn parse_notional(value: &str) -> PyResult<Notional> {
    Notional::from_str(value).map_err(|error| create_param_error(error.to_string()))
}

fn parse_cash_flow(value: &str) -> PyResult<CashFlow> {
    CashFlow::from_str(value).map_err(|error| create_param_error(error.to_string()))
}

fn parse_position_size(value: &str) -> PyResult<PositionSize> {
    PositionSize::from_str(value).map_err(|error| create_param_error(error.to_string()))
}

fn rust_decimal_to_python_decimal(
    py: Python<'_>,
    value: rust_decimal::Decimal,
) -> PyResult<PyObject> {
    let decimal_mod = PyModule::import_bound(py, "decimal")?;
    let decimal_cls = decimal_mod.getattr("Decimal")?;
    let text = value.to_string();
    Ok(decimal_cls.call1((text,))?.unbind())
}

fn extract_python_decimal_string(value: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    let py = value.py();
    let decimal_mod = PyModule::import_bound(py, "decimal")?;
    let decimal_cls = decimal_mod.getattr("Decimal")?;
    if value.is_instance(&decimal_cls)? {
        return Ok(Some(value.str()?.extract::<String>()?));
    }
    Ok(None)
}

fn parse_python_decimal(
    value: &Bound<'_, PyAny>,
    type_name: &str,
) -> PyResult<rust_decimal::Decimal> {
    let text = extract_python_decimal_string(value)?.ok_or_else(|| {
        PyTypeError::new_err(format!("{type_name}.from_decimal expects decimal.Decimal"))
    })?;
    text.parse::<rust_decimal::Decimal>()
        .map_err(|error| create_param_error(error.to_string()))
}

fn is_other_decimal_param_type(value: &Bound<'_, PyAny>, expected_type: &str) -> PyResult<bool> {
    let py_type = value.get_type();
    let module_name = py_type.getattr("__module__")?.extract::<String>()?;
    if module_name != "openpit.param" {
        return Ok(false);
    }

    let type_name = py_type.getattr("__name__")?.extract::<String>()?;
    if type_name == expected_type {
        return Ok(false);
    }

    Ok(matches!(
        type_name.as_str(),
        "Quantity" | "Volume" | "Notional" | "Price" | "Pnl" | "Fee" | "CashFlow" | "PositionSize"
    ))
}

fn is_decimal_param_type(value: &Bound<'_, PyAny>) -> PyResult<bool> {
    let py_type = value.get_type();
    let module_name = py_type.getattr("__module__")?.extract::<String>()?;
    if module_name != "openpit.param" {
        return Ok(false);
    }

    let type_name = py_type.getattr("__name__")?.extract::<String>()?;
    Ok(matches!(
        type_name.as_str(),
        "Quantity" | "Volume" | "Notional" | "Price" | "Pnl" | "Fee" | "CashFlow" | "PositionSize"
    ))
}

enum ScalarOperand {
    I64(i64),
    U64(u64),
    F64(f64),
}

fn extract_scalar_operand(value: &Bound<'_, PyAny>) -> PyResult<Option<ScalarOperand>> {
    if value.extract::<bool>().is_ok() {
        return Ok(None);
    }

    if is_decimal_param_type(value)? {
        return Ok(None);
    }

    if value.is_instance_of::<PyInt>() {
        if let Ok(number) = value.extract::<i64>() {
            return Ok(Some(ScalarOperand::I64(number)));
        }
        if let Ok(number) = value.extract::<u64>() {
            if let Ok(number_i64) = i64::try_from(number) {
                return Ok(Some(ScalarOperand::I64(number_i64)));
            }
            return Ok(Some(ScalarOperand::U64(number)));
        }
        return Err(create_param_error(
            "integer operand is out of range for i64/u64",
        ));
    }

    if let Ok(number) = value.extract::<f64>() {
        return Ok(Some(ScalarOperand::F64(number)));
    }

    Ok(None)
}

fn parse_leverage_input(value: &Bound<'_, PyAny>) -> PyResult<Leverage> {
    // Python bool is a subclass of int, so True/False would pass as 1/0 here.
    // We explicitly reject bool to avoid silently accepting it as a numeric value.
    if value.extract::<bool>().is_ok() {
        return Err(PyValueError::new_err(
            "leverage must be openpit.param.Leverage, int, or float",
        ));
    }

    if let Ok(value) = value.extract::<PyRef<'_, PyLeverage>>() {
        return Ok(value.inner);
    }

    if let Ok(value) = value.extract::<u16>() {
        return Leverage::from_u16(value).map_err(|error| PyValueError::new_err(error.to_string()));
    }

    if let Ok(value) = value.extract::<f64>() {
        return Leverage::from_f64(value).map_err(|error| PyValueError::new_err(error.to_string()));
    }

    Err(PyValueError::new_err(
        "leverage must be openpit.param.Leverage, int, or float",
    ))
}

macro_rules! define_typed_decimal_input_parser {
    ($fn_name:ident, $py_wrapper:ident, $result_type:ty, $parse_fn:ident, $type_name:literal) => {
        fn $fn_name(value: &Bound<'_, PyAny>) -> PyResult<$result_type> {
            if let Ok(value) = value.extract::<PyRef<'_, $py_wrapper>>() {
                return Ok(value.inner);
            }

            if let Some(text) = extract_python_decimal_string(value)? {
                return $parse_fn(&text);
            }

            let error_message = format!("{0} must be a Decimal, str, int, or float", $type_name);

            if is_other_decimal_param_type(value, $type_name)? {
                return Err(PyTypeError::new_err(error_message));
            }

            if value.extract::<bool>().is_ok() {
                return Err(PyValueError::new_err(error_message));
            }

            if let Ok(text) = value.extract::<String>() {
                return $parse_fn(&text);
            }

            if let Ok(number) = value.extract::<i64>() {
                return $parse_fn(&number.to_string());
            }

            if let Ok(number) = value.extract::<u64>() {
                return $parse_fn(&number.to_string());
            }

            if let Ok(number) = value.extract::<f64>() {
                return $parse_fn(&format!("{number}"));
            }

            Err(PyValueError::new_err(error_message))
        }
    };
}

define_typed_decimal_input_parser!(
    parse_quantity_input,
    PyQuantity,
    Quantity,
    parse_quantity,
    "Quantity"
);
define_typed_decimal_input_parser!(parse_price_input, PyPrice, Price, parse_price, "Price");
define_typed_decimal_input_parser!(parse_pnl_input, PyPnl, Pnl, parse_pnl, "Pnl");
define_typed_decimal_input_parser!(parse_fee_input, PyFee, Fee, parse_fee, "Fee");
define_typed_decimal_input_parser!(parse_volume_input, PyVolume, Volume, parse_volume, "Volume");
define_typed_decimal_input_parser!(
    parse_notional_input,
    PyNotional,
    Notional,
    parse_notional,
    "Notional"
);
define_typed_decimal_input_parser!(
    parse_cash_flow_input,
    PyCashFlow,
    CashFlow,
    parse_cash_flow,
    "CashFlow"
);
define_typed_decimal_input_parser!(
    parse_position_size_input,
    PyPositionSize,
    PositionSize,
    parse_position_size,
    "PositionSize"
);

fn parse_trade_amount_input(value: &Bound<'_, PyAny>) -> PyResult<TradeAmount> {
    if let Ok(value) = value.extract::<PyRef<'_, PyTradeAmount>>() {
        return Ok(value.inner);
    }
    Err(PyTypeError::new_err(
        "trade_amount must be openpit.param.TradeAmount",
    ))
}

fn trade_amount_to_python(value: TradeAmount) -> PyTradeAmount {
    PyTradeAmount { inner: value }
}

fn convert_reject(reject: &Reject) -> PyReject {
    PyReject {
        code: reject.code.as_str().to_owned(),
        reason: reject.reason.clone(),
        details: reject.details.clone(),
        policy: reject.policy.to_owned(),
        scope: match reject.scope {
            RejectScope::Order => "order",
            RejectScope::Account => "account",
        }
        .to_owned(),
        user_data: reject.user_data as u64,
    }
}

fn format_engine_build_error(error: EngineBuildError) -> String {
    match error {
        EngineBuildError::DuplicatePolicyName { name } => {
            format!("duplicate policy name in engine configuration: {name}")
        }
        _ => error.to_string(),
    }
}

#[pymodule]
fn _openpit(py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add("RejectError", py.get_type_bound::<RejectError>())?;
    module.add("ParamError", py.get_type_bound::<ParamError>())?;
    module.add(
        "_ROUNDING_STRATEGY_DEFAULT",
        rounding_strategy_name(RoundingStrategy::DEFAULT),
    )?;
    module.add(
        "_ROUNDING_STRATEGY_BANKER",
        rounding_strategy_name(RoundingStrategy::BANKER),
    )?;
    module.add(
        "_ROUNDING_STRATEGY_CONSERVATIVE_PROFIT",
        rounding_strategy_name(RoundingStrategy::CONSERVATIVE_PROFIT),
    )?;
    module.add(
        "_ROUNDING_STRATEGY_CONSERVATIVE_LOSS",
        rounding_strategy_name(RoundingStrategy::CONSERVATIVE_LOSS),
    )?;
    module.add("_LEVERAGE_SCALE", Leverage::SCALE)?;
    module.add("_LEVERAGE_MIN", Leverage::MIN)?;
    module.add("_LEVERAGE_MAX", Leverage::MAX)?;
    module.add("_LEVERAGE_STEP", Leverage::STEP)?;
    module.add_function(wrap_pyfunction!(_validate_asset, module)?)?;
    module.add_class::<PyAccountId>()?;
    module.add_class::<PyQuantity>()?;
    module.add_class::<PyPrice>()?;
    module.add_class::<PyTrade>()?;
    module.add_class::<PyPnl>()?;
    module.add_class::<PyFee>()?;
    module.add_class::<PyVolume>()?;
    module.add_class::<PyNotional>()?;
    module.add_class::<PyCashFlow>()?;
    module.add_class::<PyPositionSize>()?;
    module.add_class::<PyTradeAmount>()?;
    module.add_class::<PyAdjustmentAmount>()?;
    module.add_class::<PyLeverage>()?;
    module.add_class::<PyEngine>()?;
    module.add_class::<PyReject>()?;
    module.add_class::<PyStartPreTradeResult>()?;
    module.add_class::<PyExecuteResult>()?;
    module.add_class::<PyAccountAdjustmentBatchResult>()?;
    module.add_class::<PyEngineBuilder>()?;
    module.add_class::<PyInstrument>()?;
    module.add_class::<PyOrderOperation>()?;
    module.add_class::<PyOrderPosition>()?;
    module.add_class::<PyOrderMargin>()?;
    module.add_class::<PyOrder>()?;
    module.add_class::<PyRequest>()?;
    module.add_class::<PyReservation>()?;
    module.add_class::<PyExecutionReportOperation>()?;
    module.add_class::<PyFinancialImpact>()?;
    module.add_class::<PyExecutionReportFillDetails>()?;
    module.add_class::<PyExecutionReportPositionImpact>()?;
    module.add_class::<PyExecutionReport>()?;
    module.add_class::<PyAccountAdjustmentAmount>()?;
    module.add_class::<PyAccountAdjustmentBalanceOperation>()?;
    module.add_class::<PyAccountAdjustmentPositionOperation>()?;
    module.add_class::<PyAccountAdjustmentBounds>()?;
    module.add_class::<PyAccountAdjustment>()?;
    module.add_class::<PyPostTradeResult>()?;
    module.add_class::<PyPreTradeLock>()?;
    module.add_class::<PyPnlBoundsKillSwitchPolicy>()?;
    module.add_class::<PyRateLimitPolicy>()?;
    module.add_class::<PyOrderValidationPolicy>()?;
    module.add_class::<PyOrderSizeLimit>()?;
    module.add_class::<PyOrderSizeLimitPolicy>()?;
    module.add_class::<PyPreTradeContext>()?;
    module.add_class::<PyAccountAdjustmentContext>()?;
    Ok(())
}

#[cfg(test)]
mod field_access_tests {
    use super::*;
    use openpit::RequestFieldAccessError;
    use pyo3::types::{PyList, PyString};
    use std::sync::Once;

    fn ensure_python_initialized() {
        static INIT: Once = Once::new();
        INIT.call_once(pyo3::prepare_freethreaded_python);
    }

    fn order_without_operation() -> Order {
        ensure_python_initialized();
        Python::with_gil(|py| Order {
            operation: OrderOperationAccess::Absent,
            position: OrderPositionAccess::Absent,
            margin: OrderMarginAccess::Absent,
            original: py.None(),
        })
    }

    fn report_without_groups() -> ExecutionReport {
        ensure_python_initialized();
        Python::with_gil(|py| ExecutionReport {
            operation: ExecutionReportOperationAccess::Absent,
            financial_impact: FinancialImpactAccess::Absent,
            fill: ExecutionReportFillAccess::Absent,
            position_impact: ExecutionReportPositionImpactAccess::Absent,
            original: py.None(),
        })
    }

    #[test]
    fn python_order_instrument_returns_err_when_operation_absent() {
        let order = order_without_operation();
        assert_eq!(
            order.instrument(),
            Err(RequestFieldAccessError::new("operation.instrument"))
        );
    }

    #[test]
    fn python_order_side_returns_err_when_operation_absent() {
        let order = order_without_operation();
        assert_eq!(
            order.side(),
            Err(RequestFieldAccessError::new("operation.side"))
        );
    }

    #[test]
    fn python_order_account_id_returns_err_when_operation_absent() {
        let order = order_without_operation();
        assert_eq!(
            order.account_id(),
            Err(RequestFieldAccessError::new("operation.account_id"))
        );
    }

    #[test]
    fn python_order_trade_amount_returns_err_when_operation_absent() {
        let order = order_without_operation();
        assert_eq!(
            order.trade_amount(),
            Err(RequestFieldAccessError::new("operation.trade_amount"))
        );
    }

    #[test]
    fn python_report_instrument_returns_err_when_operation_absent() {
        let report = report_without_groups();
        assert_eq!(
            report.instrument(),
            Err(RequestFieldAccessError::new("operation.instrument"))
        );
    }

    #[test]
    fn python_report_pnl_returns_err_when_financial_impact_absent() {
        let report = report_without_groups();
        assert_eq!(
            report.pnl(),
            Err(RequestFieldAccessError::new("financial_impact.pnl"))
        );
    }

    #[test]
    fn python_report_fee_returns_err_when_financial_impact_absent() {
        let report = report_without_groups();
        assert_eq!(
            report.fee(),
            Err(RequestFieldAccessError::new("financial_impact.fee"))
        );
    }

    #[test]
    fn python_engine_end_to_end_covers_python_adapter_paths() {
        ensure_python_initialized();
        Python::with_gil(|py| -> PyResult<()> {
            let policy_module = PyModule::from_code_bound(
                py,
                r#"
from types import SimpleNamespace

class StartPolicy:
    def __init__(self):
        self.name = "PythonStartPolicy"

    def check_pre_trade_start(self, ctx, order):
        return None

    def apply_execution_report(self, *, report):
        return False

class MainPolicy:
    def __init__(self):
        self.name = "PythonMainPolicy"

    def perform_pre_trade_check(self, ctx, order):
        mutation = SimpleNamespace(commit=lambda: None, rollback=lambda: None)
        return SimpleNamespace(rejects=[], mutations=[mutation])

    def apply_execution_report(self, *, report):
        return False

class AdjustmentPolicy:
    def __init__(self):
        self.name = "PythonAdjustmentPolicy"

    def apply_account_adjustment(self, ctx, account_id, adjustment):
        return None
"#,
                "test_python_policies.py",
                "test_python_policies",
            )?;

            let start_policy = policy_module.getattr("StartPolicy")?.call0()?;
            let main_policy = policy_module.getattr("MainPolicy")?.call0()?;
            let adjustment_policy = policy_module.getattr("AdjustmentPolicy")?.call0()?;

            let builder = PyEngineBuilder {
                start_policies: RefCell::new(Vec::new()),
                main_policies: RefCell::new(Vec::new()),
                adjustment_policies: RefCell::new(Vec::new()),
            };

            let order_validation = Py::new(py, PyOrderValidationPolicy::new())?;
            builder.push_start_policy(&order_validation.bind(py).clone().into_any())?;

            let pnl_policy = Py::new(
                py,
                PyPnlBoundsKillSwitchPolicy::new(
                    &PyString::new_bound(py, "USD").into_any(),
                    Some(
                        &Py::new(
                            py,
                            PyPnl {
                                inner: Pnl::from_str("-500").expect("pnl must be valid"),
                            },
                        )?
                        .bind(py)
                        .clone()
                        .into_any(),
                    ),
                    None,
                    &Py::new(py, PyPnl { inner: Pnl::ZERO })?
                        .bind(py)
                        .clone()
                        .into_any(),
                )?,
            )?;
            builder.push_start_policy(&pnl_policy.bind(py).clone().into_any())?;

            let rate_limit = Py::new(py, PyRateLimitPolicy::new(100, 1))?;
            builder.push_start_policy(&rate_limit.bind(py).clone().into_any())?;

            let size_limit = Py::new(
                py,
                PyOrderSizeLimit {
                    inner: OrderSizeLimitConfig {
                        settlement_asset: "USD".to_owned(),
                        max_quantity: "1000".to_owned(),
                        max_notional: "1000000".to_owned(),
                    },
                },
            )?;
            let size_limit_policy = Py::new(
                py,
                PyOrderSizeLimitPolicy::new(&size_limit.bind(py).borrow())?,
            )?;
            builder.push_start_policy(&size_limit_policy.bind(py).clone().into_any())?;
            builder.push_start_policy(&start_policy)?;
            builder.push_main_policy(&main_policy)?;
            builder.push_account_adjustment_policy(&adjustment_policy)?;

            let engine = builder.build()?;

            let operation = Py::new(
                py,
                PyOrderOperation {
                    underlying_asset: Some(Asset::new("AAPL").expect("asset code must be valid")),
                    settlement_asset: Some(Asset::new("USD").expect("asset code must be valid")),
                    account_id: Some(AccountId::from_u64(99224416)),
                    side: Some(Side::Buy),
                    trade_amount: Some(TradeAmount::Quantity(
                        Quantity::from_str("1").expect("quantity must be valid"),
                    )),
                    price: Some(Price::from_str("100").expect("price must be valid")),
                },
            )?;
            let order = Py::new(
                py,
                PyOrder {
                    operation: Some(operation),
                    position: None,
                    margin: None,
                },
            )?;

            let start_result = engine.start_pre_trade(py, order.bind(py).clone().into_any())?;
            assert!(start_result.ok());

            let request = start_result.request(py).expect("request must be present");
            let execute_result = request.bind(py).borrow().execute(py)?;
            assert!(execute_result.ok());

            let reservation = execute_result
                .reservation(py)
                .expect("reservation must be present");
            {
                let reservation_ref = reservation.bind(py).borrow();
                let lock_price = reservation_ref
                    .inner
                    .borrow()
                    .as_ref()
                    .expect("reservation must exist")
                    .lock()
                    .price();
                assert_eq!(lock_price, None);
            }
            reservation.bind(py).borrow().commit()?;

            let report_operation = Py::new(
                py,
                PyExecutionReportOperation {
                    underlying_asset: Some(Asset::new("AAPL").expect("asset code must be valid")),
                    settlement_asset: Some(Asset::new("USD").expect("asset code must be valid")),
                    account_id: Some(AccountId::from_u64(99224416)),
                    side: Some(Side::Buy),
                },
            )?;
            let report_impact = Py::new(
                py,
                PyFinancialImpact {
                    pnl: Pnl::from_str("1").expect("pnl must be valid"),
                    fee: Fee::from_str("0").expect("fee must be valid"),
                },
            )?;
            let report = Py::new(
                py,
                PyExecutionReport {
                    operation: Some(report_operation),
                    financial_impact: Some(report_impact),
                    fill: None,
                    position_impact: None,
                },
            )?;
            let _ = engine.apply_execution_report(&report.bind(py).clone().into_any())?;

            let adjustment = Py::new(
                py,
                PyAccountAdjustment {
                    operation: None,
                    amount: None,
                    bounds: None,
                },
            )?;
            let account_id = Py::new(
                py,
                PyAccountId {
                    inner: AccountId::from_u64(99224416),
                },
            )?;
            let adjustments = PyList::new_bound(py, [adjustment.bind(py).clone().into_any()]);
            let batch = engine.apply_account_adjustment(
                &account_id.bind(py).clone().into_any(),
                &adjustments.into_any(),
            )?;
            assert_eq!(batch.failed_index(), None);
            assert!(batch.rejects().is_empty());

            Ok(())
        })
        .expect("python adapter flow must succeed");
    }
}
