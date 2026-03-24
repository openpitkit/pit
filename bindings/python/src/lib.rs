#![allow(unexpected_cfgs)]
#![allow(clippy::useless_conversion)]

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

use std::cell::RefCell;
use std::rc::Rc;
use std::thread_local;
use std::time::Duration;

use openpit::param::{
    AccountId, Asset, CashFlow, Fee, Leverage, Pnl, PositionEffect, PositionSide, PositionSize,
    Price, Quantity, Side, Trade, TradeAmount, Volume,
};
use openpit::pretrade::policies::OrderValidationPolicy;
use openpit::pretrade::policies::PnlKillSwitchPolicy;
use openpit::pretrade::policies::RateLimitPolicy;
use openpit::pretrade::policies::{OrderSizeLimit, OrderSizeLimitPolicy};
use openpit::pretrade::{
    CheckPreTradeStartPolicy, Mutation, Mutations, Policy, Reject, RejectCode, RejectScope,
    Request, Reservation, RiskMutation,
};
use openpit::{
    Engine, EngineBuildError, ExecutionReportOperation, ExecutionReportPositionImpact,
    FinancialImpact, HasAccountId, HasAutoBorrow, HasClosePosition, HasExecutionReportIsTerminal,
    HasExecutionReportLastTrade, HasExecutionReportPositionEffect, HasExecutionReportPositionSide,
    HasFee, HasInstrument, HasOrderCollateralAsset, HasOrderLeverage, HasOrderPositionSide,
    HasOrderPrice, HasPnl, HasReduceOnly, HasSide, HasTradeAmount, Instrument, OrderMargin,
    OrderOperation, OrderPosition, PostTradeResult,
};
use pit_interop::{
    ExecutionReportGroupAccess, GuardedOrderSizeLimit, GuardedOrderValidation,
    GuardedPnlKillSwitch, GuardedRateLimit, OrderGroupAccess,
};
use pyo3::basic::CompareOp;
use pyo3::create_exception;
use pyo3::exceptions::{PyException, PyRuntimeError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyDict;

create_exception!(openpit, RejectError, PyException);

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

struct PythonOrder {
    operation: Option<OrderOperation>,
    position: Option<OrderPosition>,
    margin: Option<OrderMargin>,
    original: Py<PyAny>,
}

impl PythonOrder {
    fn original(&self, py: Python<'_>) -> Py<PyAny> {
        self.original.clone_ref(py)
    }
}

impl HasInstrument for PythonOrder {
    fn instrument(&self) -> &Instrument {
        &self
            .operation
            .as_ref()
            .expect("internal error: required order data not validated before policy dispatch")
            .instrument
    }
}

impl HasSide for PythonOrder {
    fn side(&self) -> Side {
        self.operation
            .as_ref()
            .expect("internal error: required order data not validated before policy dispatch")
            .side
    }
}

impl HasAccountId for PythonOrder {
    fn account_id(&self) -> AccountId {
        self.operation
            .as_ref()
            .map(|op| op.account_id)
            .unwrap_or_else(|| AccountId::from_u64(99224416))
    }
}

impl HasTradeAmount for PythonOrder {
    fn trade_amount(&self) -> TradeAmount {
        self.operation
            .as_ref()
            .expect("internal error: required order data not validated before policy dispatch")
            .trade_amount
    }
}

impl HasOrderPrice for PythonOrder {
    fn price(&self) -> Option<Price> {
        self.operation.as_ref().and_then(|op| op.price)
    }
}

impl HasOrderPositionSide for PythonOrder {
    fn position_side(&self) -> Option<PositionSide> {
        self.position.as_ref().and_then(|pos| pos.position_side)
    }
}

impl HasReduceOnly for PythonOrder {
    fn reduce_only(&self) -> bool {
        self.position.as_ref().is_some_and(|pos| pos.reduce_only)
    }
}

impl HasClosePosition for PythonOrder {
    fn close_position(&self) -> bool {
        self.position.as_ref().is_some_and(|pos| pos.close_position)
    }
}

impl HasOrderLeverage for PythonOrder {
    fn leverage(&self) -> Option<Leverage> {
        self.margin.as_ref().and_then(|m| m.leverage)
    }
}

impl HasOrderCollateralAsset for PythonOrder {
    fn collateral_asset(&self) -> Option<&Asset> {
        self.margin
            .as_ref()
            .and_then(|m| m.collateral_asset.as_ref())
    }
}

impl HasAutoBorrow for PythonOrder {
    fn auto_borrow(&self) -> bool {
        self.margin.as_ref().is_some_and(|m| m.auto_borrow)
    }
}

impl OrderGroupAccess for PythonOrder {
    fn has_operation(&self) -> bool {
        self.operation.is_some()
    }
}

struct PythonFillData {
    last_trade: Option<Trade>,
    is_terminal: bool,
}

struct PythonExecutionReport {
    operation: Option<ExecutionReportOperation>,
    financial_impact: Option<FinancialImpact>,
    fill: Option<PythonFillData>,
    position_impact: Option<ExecutionReportPositionImpact>,
    original: Py<PyAny>,
}

impl PythonExecutionReport {
    fn original(&self, py: Python<'_>) -> Py<PyAny> {
        self.original.clone_ref(py)
    }
}

impl HasInstrument for PythonExecutionReport {
    fn instrument(&self) -> &Instrument {
        &self.operation
            .as_ref()
            .expect("internal error: required execution report data not validated before policy dispatch")
            .instrument
    }
}

impl HasSide for PythonExecutionReport {
    fn side(&self) -> Side {
        self.operation
            .as_ref()
            .expect("internal error: required execution report data not validated before policy dispatch")
            .side
    }
}

impl HasAccountId for PythonExecutionReport {
    fn account_id(&self) -> AccountId {
        self.operation
            .as_ref()
            .map(|op| op.account_id)
            .unwrap_or_else(|| AccountId::from_u64(99224416))
    }
}

impl HasPnl for PythonExecutionReport {
    fn pnl(&self) -> Pnl {
        self.financial_impact
            .as_ref()
            .expect("internal error: required execution report data not validated before policy dispatch")
            .pnl
    }
}

impl HasFee for PythonExecutionReport {
    fn fee(&self) -> Fee {
        self.financial_impact
            .as_ref()
            .expect("internal error: required execution report data not validated before policy dispatch")
            .fee
    }
}

impl HasExecutionReportLastTrade for PythonExecutionReport {
    fn last_trade(&self) -> Option<Trade> {
        self.fill.as_ref().and_then(|f| f.last_trade)
    }
}

impl HasExecutionReportIsTerminal for PythonExecutionReport {
    fn is_terminal(&self) -> bool {
        self.fill.as_ref().is_some_and(|f| f.is_terminal)
    }
}

impl HasExecutionReportPositionEffect for PythonExecutionReport {
    fn position_effect(&self) -> Option<PositionEffect> {
        self.position_impact
            .as_ref()
            .and_then(|pi| pi.position_effect)
    }
}

impl HasExecutionReportPositionSide for PythonExecutionReport {
    fn position_side(&self) -> Option<PositionSide> {
        self.position_impact
            .as_ref()
            .and_then(|pi| pi.position_side)
    }
}

impl ExecutionReportGroupAccess for PythonExecutionReport {
    fn has_operation(&self) -> bool {
        self.operation.is_some()
    }
    fn has_financial_impact(&self) -> bool {
        self.financial_impact.is_some()
    }
}

#[pyclass(name = "Engine", module = "openpit", unsendable)]
struct PyEngine {
    inner: Engine<PythonOrder, PythonExecutionReport>,
}

#[pymethods]
impl PyEngine {
    #[staticmethod]
    fn builder() -> PyEngineBuilder {
        PyEngineBuilder {
            start_policies: RefCell::new(Vec::new()),
            main_policies: RefCell::new(Vec::new()),
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
                    reject: None,
                })
            }
            Err(reject) => {
                if let Some(error) = take_python_callback_error() {
                    return Err(error);
                }

                Ok(PyStartPreTradeResult {
                    request: None,
                    reject: Some(convert_reject(&reject)),
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
}

#[pyclass(name = "Reject", module = "openpit.pretrade")]
#[derive(Clone, Debug)]
struct PyReject {
    code: String,
    reason: String,
    details: String,
    policy: String,
    scope: String,
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

    fn __repr__(&self) -> String {
        format!(
            "Reject(code={:?}, reason={:?}, details={:?}, policy={:?}, scope={:?})",
            self.code, self.reason, self.details, self.policy, self.scope
        )
    }
}

#[pyclass(name = "RejectCode", module = "openpit.pretrade")]
struct PyRejectCode;

#[pymethods]
impl PyRejectCode {
    #[classattr]
    const MISSING_REQUIRED_FIELD: &'static str = "MissingRequiredField";
    #[classattr]
    const INVALID_FIELD_FORMAT: &'static str = "InvalidFieldFormat";
    #[classattr]
    const INVALID_FIELD_VALUE: &'static str = "InvalidFieldValue";
    #[classattr]
    const UNSUPPORTED_ORDER_TYPE: &'static str = "UnsupportedOrderType";
    #[classattr]
    const UNSUPPORTED_TIME_IN_FORCE: &'static str = "UnsupportedTimeInForce";
    #[classattr]
    const UNSUPPORTED_ORDER_ATTRIBUTE: &'static str = "UnsupportedOrderAttribute";
    #[classattr]
    const DUPLICATE_CLIENT_ORDER_ID: &'static str = "DuplicateClientOrderId";
    #[classattr]
    const TOO_LATE_TO_ENTER: &'static str = "TooLateToEnter";
    #[classattr]
    const EXCHANGE_CLOSED: &'static str = "ExchangeClosed";
    #[classattr]
    const UNKNOWN_INSTRUMENT: &'static str = "UnknownInstrument";
    #[classattr]
    const UNKNOWN_ACCOUNT: &'static str = "UnknownAccount";
    #[classattr]
    const UNKNOWN_VENUE: &'static str = "UnknownVenue";
    #[classattr]
    const UNKNOWN_CLEARING_ACCOUNT: &'static str = "UnknownClearingAccount";
    #[classattr]
    const UNKNOWN_COLLATERAL_ASSET: &'static str = "UnknownCollateralAsset";
    #[classattr]
    const INSUFFICIENT_FUNDS: &'static str = "InsufficientFunds";
    #[classattr]
    const INSUFFICIENT_MARGIN: &'static str = "InsufficientMargin";
    #[classattr]
    const INSUFFICIENT_POSITION: &'static str = "InsufficientPosition";
    #[classattr]
    const CREDIT_LIMIT_EXCEEDED: &'static str = "CreditLimitExceeded";
    #[classattr]
    const RISK_LIMIT_EXCEEDED: &'static str = "RiskLimitExceeded";
    #[classattr]
    const ORDER_EXCEEDS_LIMIT: &'static str = "OrderExceedsLimit";
    #[classattr]
    const ORDER_QTY_EXCEEDS_LIMIT: &'static str = "OrderQtyExceedsLimit";
    #[classattr]
    const ORDER_NOTIONAL_EXCEEDS_LIMIT: &'static str = "OrderNotionalExceedsLimit";
    #[classattr]
    const POSITION_LIMIT_EXCEEDED: &'static str = "PositionLimitExceeded";
    #[classattr]
    const CONCENTRATION_LIMIT_EXCEEDED: &'static str = "ConcentrationLimitExceeded";
    #[classattr]
    const LEVERAGE_LIMIT_EXCEEDED: &'static str = "LeverageLimitExceeded";
    #[classattr]
    const RATE_LIMIT_EXCEEDED: &'static str = "RateLimitExceeded";
    #[classattr]
    const PNL_KILL_SWITCH_TRIGGERED: &'static str = "PnlKillSwitchTriggered";
    #[classattr]
    const ACCOUNT_BLOCKED: &'static str = "AccountBlocked";
    #[classattr]
    const ACCOUNT_NOT_AUTHORIZED: &'static str = "AccountNotAuthorized";
    #[classattr]
    const COMPLIANCE_RESTRICTION: &'static str = "ComplianceRestriction";
    #[classattr]
    const INSTRUMENT_RESTRICTED: &'static str = "InstrumentRestricted";
    #[classattr]
    const JURISDICTION_RESTRICTION: &'static str = "JurisdictionRestriction";
    #[classattr]
    const WASH_TRADE_PREVENTION: &'static str = "WashTradePrevention";
    #[classattr]
    const SELF_MATCH_PREVENTION: &'static str = "SelfMatchPrevention";
    #[classattr]
    const SHORT_SALE_RESTRICTION: &'static str = "ShortSaleRestriction";
    #[classattr]
    const RISK_CONFIGURATION_MISSING: &'static str = "RiskConfigurationMissing";
    #[classattr]
    const REFERENCE_DATA_UNAVAILABLE: &'static str = "ReferenceDataUnavailable";
    #[classattr]
    const ORDER_VALUE_CALCULATION_FAILED: &'static str = "OrderValueCalculationFailed";
    #[classattr]
    const SYSTEM_UNAVAILABLE: &'static str = "SystemUnavailable";
    #[classattr]
    const OTHER: &'static str = "Other";
}

#[pyclass(name = "StartPreTradeResult", module = "openpit.pretrade", unsendable)]
struct PyStartPreTradeResult {
    request: Option<Py<PyRequest>>,
    reject: Option<PyReject>,
}

#[pymethods]
impl PyStartPreTradeResult {
    #[getter]
    fn ok(&self) -> bool {
        self.reject.is_none()
    }

    #[getter]
    fn request(&self, py: Python<'_>) -> Option<Py<PyRequest>> {
        self.request.as_ref().map(|request| request.clone_ref(py))
    }

    #[getter]
    fn reject(&self) -> Option<PyReject> {
        self.reject.clone()
    }

    fn __bool__(&self) -> bool {
        self.ok()
    }

    fn __repr__(&self) -> String {
        match &self.reject {
            Some(reject) => format!("StartPreTradeResult(ok=False, reject={reject:?})"),
            None => "StartPreTradeResult(ok=True)".to_owned(),
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

enum StartPolicyConfig {
    OrderValidation,
    PnlKillSwitchShared {
        policy: Rc<PnlKillSwitchPolicy>,
    },
    RateLimit {
        max_orders: usize,
        window_seconds: u64,
    },
    OrderSizeLimit {
        limits: Vec<OrderSizeLimitConfig>,
    },
    PythonCustom {
        name: &'static str,
        policy: Py<PyAny>,
    },
}

#[derive(Clone)]
struct OrderSizeLimitConfig {
    settlement_asset: String,
    max_quantity: String,
    max_notional: String,
}

enum MainPolicyConfig {
    PythonCustom {
        name: &'static str,
        policy: Py<PyAny>,
    },
}

struct PythonStartPolicyAdapter {
    name: &'static str,
    policy: Py<PyAny>,
}

struct PythonMainPolicyAdapter {
    name: &'static str,
    policy: Py<PyAny>,
}

impl CheckPreTradeStartPolicy<PythonOrder, PythonExecutionReport> for PythonStartPolicyAdapter {
    fn name(&self) -> &'static str {
        self.name
    }

    fn check_pre_trade_start(&self, order: &PythonOrder) -> Result<(), Reject> {
        Python::with_gil(|py| {
            let kwargs = PyDict::new_bound(py);
            kwargs
                .set_item("order", order.original(py))
                .map_err(|error| {
                    set_python_callback_error(error);
                    python_callback_reject(self.name)
                })?;
            let result = self
                .policy
                .bind(py)
                .call_method("check_pre_trade_start", (), Some(&kwargs))
                .map_err(|error| {
                    set_python_callback_error(error);
                    python_callback_reject(self.name)
                })?;

            if result.is_none() {
                Ok(())
            } else {
                let reject = parse_policy_reject(&result, self.name).map_err(|error| {
                    set_python_callback_error(error);
                    python_callback_reject(self.name)
                })?;
                Err(reject)
            }
        })
    }

    fn apply_execution_report(&self, report: &PythonExecutionReport) -> bool {
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

impl Policy<PythonOrder, PythonExecutionReport> for PythonMainPolicyAdapter {
    fn name(&self) -> &'static str {
        self.name
    }

    fn perform_pre_trade_check(
        &self,
        ctx: &openpit::pretrade::Context<'_, PythonOrder>,
        mutations: &mut Mutations,
        rejects: &mut Vec<Reject>,
    ) {
        Python::with_gil(|py| {
            let py_context = match build_python_policy_context(py, ctx.order()) {
                Ok(value) => value,
                Err(error) => {
                    set_python_callback_error(error);
                    rejects.push(python_callback_reject(self.name));
                    return;
                }
            };

            let kwargs = PyDict::new_bound(py);
            if let Err(error) = kwargs.set_item("context", py_context) {
                set_python_callback_error(error);
                rejects.push(python_callback_reject(self.name));
                return;
            }

            let decision =
                match self
                    .policy
                    .bind(py)
                    .call_method("perform_pre_trade_check", (), Some(&kwargs))
                {
                    Ok(value) => value,
                    Err(error) => {
                        set_python_callback_error(error);
                        rejects.push(python_callback_reject(self.name));
                        return;
                    }
                };

            if let Err(error) = apply_policy_decision(self.name, decision, mutations, rejects) {
                set_python_callback_error(error);
                rejects.push(python_callback_reject(self.name));
            }
        });
    }

    fn apply_execution_report(&self, report: &PythonExecutionReport) -> bool {
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

fn extract_python_policy_name(policy: &Bound<'_, PyAny>) -> PyResult<&'static str> {
    let name = policy
        .getattr("name")?
        .extract::<String>()
        .map_err(|_| PyValueError::new_err("policy.name must be a string"))?;
    if name.trim().is_empty() {
        return Err(PyValueError::new_err("policy.name must not be empty"));
    }
    Ok(leak_static_str(name))
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

fn leak_static_str(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
}

fn python_callback_reject(policy_name: &'static str) -> Reject {
    Reject::new(
        policy_name,
        RejectScope::Order,
        RejectCode::SystemUnavailable,
        "python policy callback failed",
        "python policy callback raised an exception",
    )
}

fn extract_python_order(obj: &Bound<'_, PyAny>) -> PyResult<PythonOrder> {
    let py = obj.py();
    let order = obj
        .extract::<PyRef<'_, PyOrder>>()
        .map_err(|_| PyTypeError::new_err("order must inherit from openpit.Order"))?;

    let operation = order
        .operation
        .as_ref()
        .map(|py_operation| {
            let op = py_operation.bind(py).borrow();
            let instrument = match (&op.underlying_asset, &op.settlement_asset) {
                (Some(underlying_asset), Some(settlement_asset)) => {
                    Instrument::new(underlying_asset.clone(), settlement_asset.clone())
                }
                _ => {
                    return Err(PyValueError::new_err(
                        "order.operation requires underlying_asset and settlement_asset",
                    ));
                }
            };
            let account_id = op
                .account_id
                .unwrap_or_else(|| AccountId::from_u64(99224416));
            let side = op
                .side
                .ok_or_else(|| PyValueError::new_err("order.operation requires side"))?;
            let trade_amount = op
                .trade_amount
                .ok_or_else(|| PyValueError::new_err("order.operation requires trade_amount"))?;
            Ok(OrderOperation {
                instrument,
                account_id,
                side,
                trade_amount,
                price: op.price,
            })
        })
        .transpose()?;

    let position = order.position.as_ref().map(|py_position| {
        let pos = py_position.bind(py).borrow();
        OrderPosition {
            position_side: pos.position_side,
            reduce_only: pos.reduce_only,
            close_position: pos.close_position,
        }
    });

    let margin = order.margin.as_ref().map(|py_margin| {
        let m = py_margin.bind(py).borrow();
        OrderMargin {
            leverage: m.leverage,
            collateral_asset: m.collateral_asset.clone(),
            auto_borrow: m.auto_borrow,
        }
    });

    Ok(PythonOrder {
        operation,
        position,
        margin,
        original: obj.clone().unbind(),
    })
}

fn extract_python_execution_report(obj: &Bound<'_, PyAny>) -> PyResult<PythonExecutionReport> {
    let py = obj.py();
    let report = obj
        .extract::<PyRef<'_, PyExecutionReport>>()
        .map_err(|_| PyTypeError::new_err("report must inherit from openpit.ExecutionReport"))?;

    let operation = report
        .operation
        .as_ref()
        .map(|py_operation| {
            let op = py_operation.bind(py).borrow();
            let instrument = match (&op.underlying_asset, &op.settlement_asset) {
                (Some(underlying_asset), Some(settlement_asset)) => {
                    Instrument::new(underlying_asset.clone(), settlement_asset.clone())
                }
                _ => {
                    return Err(PyValueError::new_err(
                        "execution report operation requires underlying_asset and settlement_asset",
                    ));
                }
            };
            let account_id = op
                .account_id
                .unwrap_or_else(|| AccountId::from_u64(99224416));
            let side = op
                .side
                .ok_or_else(|| PyValueError::new_err("execution report operation requires side"))?;
            Ok(ExecutionReportOperation {
                instrument,
                account_id,
                side,
            })
        })
        .transpose()?;

    let financial_impact = report
        .financial_impact
        .as_ref()
        .map(|py_fi| {
            let fi = py_fi.bind(py).borrow();
            let pnl = fi.pnl.ok_or_else(|| {
                PyValueError::new_err("execution report financial_impact requires pnl")
            })?;
            let fee = fi.fee.ok_or_else(|| {
                PyValueError::new_err("execution report financial_impact requires fee")
            })?;
            Ok::<_, PyErr>(FinancialImpact { pnl, fee })
        })
        .transpose()?;

    let fill = report.fill.as_ref().map(|py_fill| {
        let f = py_fill.bind(py).borrow();
        let last_trade = f
            .fill_price
            .zip(f.fill_quantity)
            .map(|(price, quantity)| Trade { price, quantity });
        PythonFillData {
            last_trade,
            is_terminal: f.is_terminal,
        }
    });

    let position_impact = report.position_impact.as_ref().map(|py_pi| {
        let pi = py_pi.bind(py).borrow();
        ExecutionReportPositionImpact {
            position_effect: pi.position_effect,
            position_side: pi.position_side,
        }
    });

    Ok(PythonExecutionReport {
        operation,
        financial_impact,
        fill,
        position_impact,
        original: obj.clone().unbind(),
    })
}

fn build_python_policy_context(py: Python<'_>, order: &PythonOrder) -> PyResult<Py<PyAny>> {
    let module = PyModule::import_bound(py, "openpit.pretrade")?;
    let cls = module.getattr("PolicyContext")?;
    let kwargs = PyDict::new_bound(py);
    kwargs.set_item("order", order.original(py))?;
    Ok(cls.call((), Some(&kwargs))?.unbind())
}

fn apply_policy_decision(
    policy_name: &'static str,
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

fn parse_policy_reject(value: &Bound<'_, PyAny>, policy_name: &'static str) -> PyResult<Reject> {
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
    Ok(Reject::new(policy_name, scope, code, reason, details))
}

fn parse_policy_mutation(value: &Bound<'_, PyAny>) -> PyResult<Mutation> {
    Ok(Mutation {
        commit: parse_risk_mutation(&value.getattr("commit")?)?,
        rollback: parse_risk_mutation(&value.getattr("rollback")?)?,
    })
}

fn parse_risk_mutation(value: &Bound<'_, PyAny>) -> PyResult<RiskMutation> {
    let kind = value
        .getattr("kind")?
        .extract::<String>()
        .map_err(|_| PyValueError::new_err("risk mutation kind must be a string"))?;

    match kind.as_str() {
        "reserve_notional" => {
            let settlement_asset_obj = value.getattr("settlement_asset")?;
            let settlement_asset_str = settlement_asset_obj
                .extract::<String>()
                .or_else(|_| {
                    settlement_asset_obj
                        .getattr("value")
                        .and_then(|v| v.extract::<String>())
                })
                .map_err(|_| {
                    PyValueError::new_err(
                        "reserve_notional.settlement_asset must be a string or openpit.param.Asset",
                    )
                })?;
            Ok(RiskMutation::ReserveNotional {
                asset: parse_asset(&settlement_asset_str)?,
                amount: parse_volume_input(&value.getattr("amount")?)?,
            })
        }
        "set_kill_switch" => {
            let id = value
                .getattr("kill_switch_id")?
                .extract::<String>()
                .map_err(|_| {
                    PyValueError::new_err("set_kill_switch.kill_switch_id must be a string")
                })?;
            if id.trim().is_empty() {
                return Err(PyValueError::new_err(
                    "set_kill_switch.kill_switch_id must not be empty",
                ));
            }
            let enabled = value
                .getattr("enabled")?
                .extract::<bool>()
                .map_err(|_| PyValueError::new_err("set_kill_switch.enabled must be a bool"))?;
            Ok(RiskMutation::SetKillSwitch {
                id: leak_static_str(id),
                enabled,
            })
        }
        _ => Err(PyValueError::new_err(format!(
            "unsupported risk mutation kind {kind:?}"
        ))),
    }
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
        "MissingRequiredField" => Ok(RejectCode::MissingRequiredField),
        "InvalidFieldFormat" => Ok(RejectCode::InvalidFieldFormat),
        "InvalidFieldValue" => Ok(RejectCode::InvalidFieldValue),
        "UnsupportedOrderType" => Ok(RejectCode::UnsupportedOrderType),
        "UnsupportedTimeInForce" => Ok(RejectCode::UnsupportedTimeInForce),
        "UnsupportedOrderAttribute" => Ok(RejectCode::UnsupportedOrderAttribute),
        "DuplicateClientOrderId" => Ok(RejectCode::DuplicateClientOrderId),
        "TooLateToEnter" => Ok(RejectCode::TooLateToEnter),
        "ExchangeClosed" => Ok(RejectCode::ExchangeClosed),
        "UnknownInstrument" => Ok(RejectCode::UnknownInstrument),
        "UnknownAccount" => Ok(RejectCode::UnknownAccount),
        "UnknownVenue" => Ok(RejectCode::UnknownVenue),
        "UnknownClearingAccount" => Ok(RejectCode::UnknownClearingAccount),
        "UnknownCollateralAsset" => Ok(RejectCode::UnknownCollateralAsset),
        "InsufficientFunds" => Ok(RejectCode::InsufficientFunds),
        "InsufficientMargin" => Ok(RejectCode::InsufficientMargin),
        "InsufficientPosition" => Ok(RejectCode::InsufficientPosition),
        "CreditLimitExceeded" => Ok(RejectCode::CreditLimitExceeded),
        "RiskLimitExceeded" => Ok(RejectCode::RiskLimitExceeded),
        "OrderExceedsLimit" => Ok(RejectCode::OrderExceedsLimit),
        "OrderQtyExceedsLimit" => Ok(RejectCode::OrderQtyExceedsLimit),
        "OrderNotionalExceedsLimit" => Ok(RejectCode::OrderNotionalExceedsLimit),
        "PositionLimitExceeded" => Ok(RejectCode::PositionLimitExceeded),
        "ConcentrationLimitExceeded" => Ok(RejectCode::ConcentrationLimitExceeded),
        "LeverageLimitExceeded" => Ok(RejectCode::LeverageLimitExceeded),
        "RateLimitExceeded" => Ok(RejectCode::RateLimitExceeded),
        "PnlKillSwitchTriggered" => Ok(RejectCode::PnlKillSwitchTriggered),
        "AccountBlocked" => Ok(RejectCode::AccountBlocked),
        "AccountNotAuthorized" => Ok(RejectCode::AccountNotAuthorized),
        "ComplianceRestriction" => Ok(RejectCode::ComplianceRestriction),
        "InstrumentRestricted" => Ok(RejectCode::InstrumentRestricted),
        "JurisdictionRestriction" => Ok(RejectCode::JurisdictionRestriction),
        "WashTradePrevention" => Ok(RejectCode::WashTradePrevention),
        "SelfMatchPrevention" => Ok(RejectCode::SelfMatchPrevention),
        "ShortSaleRestriction" => Ok(RejectCode::ShortSaleRestriction),
        "RiskConfigurationMissing" => Ok(RejectCode::RiskConfigurationMissing),
        "ReferenceDataUnavailable" => Ok(RejectCode::ReferenceDataUnavailable),
        "OrderValueCalculationFailed" => Ok(RejectCode::OrderValueCalculationFailed),
        "SystemUnavailable" => Ok(RejectCode::SystemUnavailable),
        "Other" => Ok(RejectCode::Other),
        _ => Err(PyValueError::new_err(format!(
            "unsupported reject code {value:?}"
        ))),
    }
}

impl PyEngineBuilder {
    fn push_start_policy(&self, policy: &Bound<'_, PyAny>) -> PyResult<()> {
        let config = if let Ok(policy) = policy.extract::<PyRef<'_, PyPnlKillSwitchPolicy>>() {
            StartPolicyConfig::PnlKillSwitchShared {
                policy: policy.get_or_create_runtime_policy()?,
            }
        } else if policy
            .extract::<PyRef<'_, PyOrderValidationPolicy>>()
            .is_ok()
        {
            StartPolicyConfig::OrderValidation
        } else if let Ok(policy) = policy.extract::<PyRef<'_, PyRateLimitPolicy>>() {
            StartPolicyConfig::RateLimit {
                max_orders: policy.max_orders,
                window_seconds: policy.window_seconds,
            }
        } else if let Ok(policy) = policy.extract::<PyRef<'_, PyOrderSizeLimitPolicy>>() {
            StartPolicyConfig::OrderSizeLimit {
                limits: policy.limits.borrow().clone(),
            }
        } else {
            let name = extract_python_policy_name(policy)?;
            ensure_callable_method(policy, "check_pre_trade_start")?;
            ensure_callable_method(policy, "apply_execution_report")?;
            StartPolicyConfig::PythonCustom {
                name,
                policy: policy.clone().unbind(),
            }
        };

        self.start_policies.borrow_mut().push(config);
        Ok(())
    }

    fn push_main_policy(&self, policy: &Bound<'_, PyAny>) -> PyResult<()> {
        let name = extract_python_policy_name(policy)?;
        ensure_callable_method(policy, "perform_pre_trade_check")?;
        ensure_callable_method(policy, "apply_execution_report")?;
        self.main_policies
            .borrow_mut()
            .push(MainPolicyConfig::PythonCustom {
                name,
                policy: policy.clone().unbind(),
            });
        Ok(())
    }
}

#[pyclass(name = "EngineBuilder", module = "openpit", unsendable)]
struct PyEngineBuilder {
    start_policies: RefCell<Vec<StartPolicyConfig>>,
    main_policies: RefCell<Vec<MainPolicyConfig>>,
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

    fn build(&self) -> PyResult<PyEngine> {
        let mut builder = Engine::<PythonOrder, PythonExecutionReport>::builder();

        for policy in self.start_policies.borrow().iter() {
            builder = match policy {
                StartPolicyConfig::OrderValidation => {
                    builder.check_pre_trade_start_policy(GuardedOrderValidation::new())
                }
                StartPolicyConfig::PnlKillSwitchShared { policy } => builder
                    .check_pre_trade_start_policy(GuardedPnlKillSwitch::new(Rc::clone(policy))),
                StartPolicyConfig::RateLimit {
                    max_orders,
                    window_seconds,
                } => builder.check_pre_trade_start_policy(GuardedRateLimit::new(
                    RateLimitPolicy::new(*max_orders, Duration::from_secs(*window_seconds)),
                )),
                StartPolicyConfig::OrderSizeLimit { limits } => {
                    let (first, rest) = limits.split_first().ok_or_else(|| {
                        PyValueError::new_err("OrderSizeLimitPolicy requires at least one limit")
                    })?;
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
                    let rust_policy = OrderSizeLimitPolicy::new(first_limit, rest_limits);
                    builder.check_pre_trade_start_policy(GuardedOrderSizeLimit::new(rust_policy))
                }
                StartPolicyConfig::PythonCustom { name, policy } => builder
                    .check_pre_trade_start_policy(PythonStartPolicyAdapter {
                        name,
                        policy: Python::with_gil(|py| policy.clone_ref(py)),
                    }),
            };
        }

        for policy in self.main_policies.borrow().iter() {
            builder = match policy {
                MainPolicyConfig::PythonCustom { name, policy } => {
                    builder.pre_trade_policy(PythonMainPolicyAdapter {
                        name,
                        policy: Python::with_gil(|py| policy.clone_ref(py)),
                    })
                }
            };
        }

        let engine = builder
            .build()
            .map_err(|error| PyValueError::new_err(format_engine_build_error(error)))?;

        Ok(PyEngine { inner: engine })
    }
}

#[pyclass(
    name = "PnlKillSwitchPolicy",
    module = "openpit.pretrade.policies",
    unsendable
)]
struct PyPnlKillSwitchPolicy {
    barriers: RefCell<Vec<(String, String)>>,
    runtime_policy: RefCell<Option<Rc<PnlKillSwitchPolicy>>>,
}

#[pymethods]
impl PyPnlKillSwitchPolicy {
    #[classattr]
    const NAME: &'static str = PnlKillSwitchPolicy::NAME;

    #[new]
    #[pyo3(signature = (settlement_asset, barrier))]
    fn new(settlement_asset: &Bound<'_, PyAny>, barrier: &Bound<'_, PyAny>) -> PyResult<Self> {
        let settlement_asset = if let Ok(asset) = settlement_asset.extract::<PyRef<'_, PyAsset>>() {
            asset.inner.to_string()
        } else {
            settlement_asset
                .extract::<String>()
                .map_err(|_| PyTypeError::new_err("settlement_asset must be openpit.param.Asset"))?
        };
        parse_asset(&settlement_asset)?;
        let barrier = parse_pnl_input(barrier)?.to_string();
        Ok(Self {
            barriers: RefCell::new(vec![(settlement_asset, barrier)]),
            runtime_policy: RefCell::new(None),
        })
    }

    #[pyo3(signature = (settlement_asset, barrier))]
    fn set_barrier(
        &self,
        settlement_asset: &Bound<'_, PyAny>,
        barrier: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        if self.runtime_policy.borrow().is_some() {
            return Err(PyRuntimeError::new_err(
                "pnl policy is already bound to an engine and cannot be reconfigured",
            ));
        }

        let settlement_asset = if let Ok(asset) = settlement_asset.extract::<PyRef<'_, PyAsset>>() {
            asset.inner.to_string()
        } else {
            settlement_asset
                .extract::<String>()
                .map_err(|_| PyTypeError::new_err("settlement_asset must be openpit.param.Asset"))?
        };
        parse_asset(&settlement_asset)?;
        let barrier = parse_pnl_input(barrier)?.to_string();

        let mut barriers = self.barriers.borrow_mut();
        if let Some(existing) = barriers
            .iter_mut()
            .find(|(asset, _)| asset == settlement_asset.as_str())
        {
            existing.1 = barrier;
        } else {
            barriers.push((settlement_asset, barrier));
        }
        Ok(())
    }

    #[pyo3(signature = (settlement_asset))]
    fn reset_pnl(&self, settlement_asset: &Bound<'_, PyAny>) -> PyResult<()> {
        let settlement_asset = if let Ok(asset) = settlement_asset.extract::<PyRef<'_, PyAsset>>() {
            asset.inner.clone()
        } else {
            return Err(PyTypeError::new_err(
                "settlement_asset must be openpit.param.Asset",
            ));
        };
        let policy = self.get_or_create_runtime_policy()?;
        policy.reset_pnl(&settlement_asset);
        Ok(())
    }
}

impl PyPnlKillSwitchPolicy {
    fn get_or_create_runtime_policy(&self) -> PyResult<Rc<PnlKillSwitchPolicy>> {
        if let Some(policy) = self.runtime_policy.borrow().as_ref() {
            return Ok(Rc::clone(policy));
        }

        let barriers = self.barriers.borrow();
        let (first, rest) = barriers.split_first().ok_or_else(|| {
            PyValueError::new_err("PnlKillSwitchPolicy requires at least one barrier")
        })?;
        let first_barrier = (parse_asset(first.0.as_str())?, parse_pnl(&first.1)?);
        let rest_barriers = rest
            .iter()
            .map(|(settlement_asset, barrier)| {
                Ok((parse_asset(settlement_asset.as_str())?, parse_pnl(barrier)?))
            })
            .collect::<PyResult<Vec<_>>>()?;
        let policy = Rc::new(
            PnlKillSwitchPolicy::new(first_barrier, rest_barriers)
                .map_err(|error| PyValueError::new_err(error.to_string()))?,
        );
        self.runtime_policy.borrow_mut().replace(Rc::clone(&policy));
        Ok(policy)
    }
}

#[pyclass(name = "RateLimitPolicy", module = "openpit.pretrade.policies")]
struct PyRateLimitPolicy {
    max_orders: usize,
    window_seconds: u64,
}

#[pymethods]
impl PyRateLimitPolicy {
    #[classattr]
    const NAME: &'static str = RateLimitPolicy::NAME;

    #[new]
    #[pyo3(signature = (max_orders, window_seconds))]
    fn new(max_orders: usize, window_seconds: u64) -> Self {
        Self {
            max_orders,
            window_seconds,
        }
    }
}

#[pyclass(name = "OrderValidationPolicy", module = "openpit.pretrade.policies")]
struct PyOrderValidationPolicy;

#[pymethods]
impl PyOrderValidationPolicy {
    #[classattr]
    const NAME: &'static str = OrderValidationPolicy::NAME;

    #[new]
    fn new() -> Self {
        Self
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
        let settlement_asset = if let Ok(asset) = settlement_asset.extract::<PyRef<'_, PyAsset>>() {
            asset.inner.to_string()
        } else {
            settlement_asset
                .extract::<String>()
                .map_err(|_| PyTypeError::new_err("settlement_asset must be openpit.param.Asset"))?
        };
        parse_asset(&settlement_asset)?;
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
}

#[pymethods]
impl PyOrderSizeLimitPolicy {
    #[classattr]
    const NAME: &'static str = OrderSizeLimitPolicy::NAME;

    #[new]
    #[pyo3(signature = (limit))]
    fn new(limit: &PyOrderSizeLimit) -> Self {
        Self {
            limits: RefCell::new(vec![limit.inner.clone()]),
        }
    }

    #[pyo3(signature = (limit))]
    fn set_limit(&self, limit: &PyOrderSizeLimit) {
        let mut limits = self.limits.borrow_mut();
        if let Some(existing) = limits.iter_mut().find(|existing| {
            existing.settlement_asset.as_str() == limit.inner.settlement_asset.as_str()
        }) {
            *existing = limit.inner.clone();
        } else {
            limits.push(limit.inner.clone());
        }
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
        underlying_asset: Option<String>,
        settlement_asset: Option<String>,
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
            underlying_asset: underlying_asset.as_deref().map(parse_asset).transpose()?,
            settlement_asset: settlement_asset.as_deref().map(parse_asset).transpose()?,
            account_id: account_id.map(parse_account_id_input).transpose()?,
            side: side.map(parse_side_input).transpose()?,
            trade_amount: trade_amount.map(parse_trade_amount_input).transpose()?,
            price: price.map(parse_price_input).transpose()?,
        })
    }

    #[getter]
    fn underlying_asset(&self) -> Option<String> {
        self.underlying_asset.as_ref().map(ToString::to_string)
    }

    #[setter]
    fn set_underlying_asset(&mut self, value: Option<String>) -> PyResult<()> {
        self.underlying_asset = value.as_deref().map(parse_asset).transpose()?;
        Ok(())
    }

    #[getter]
    fn settlement_asset(&self) -> Option<String> {
        self.settlement_asset.as_ref().map(ToString::to_string)
    }

    #[setter]
    fn set_settlement_asset(&mut self, value: Option<String>) -> PyResult<()> {
        self.settlement_asset = value.as_deref().map(parse_asset).transpose()?;
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
    fn side(&self) -> Option<&'static str> {
        self.side.map(side_name)
    }

    #[setter]
    fn set_side(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.side = value.map(parse_side_input).transpose()?;
        Ok(())
    }

    #[getter]
    fn trade_amount(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        self.trade_amount
            .map(|value| trade_amount_to_python(py, value))
            .transpose()
    }

    #[setter]
    fn set_trade_amount(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.trade_amount = value.map(parse_trade_amount_input).transpose()?;
        Ok(())
    }

    #[getter]
    fn price(&self) -> Option<String> {
        self.price.as_ref().map(ToString::to_string)
    }

    #[setter]
    fn set_price(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.price = value.map(parse_price_input).transpose()?;
        Ok(())
    }

    fn __repr__(&self) -> String {
        let trade_amount = self.trade_amount.as_ref().map(trade_amount_debug);
        format!(
            "OrderOperation(underlying_asset={:?}, settlement_asset={:?}, side={:?}, trade_amount={:?}, price={:?})",
            self.underlying_asset(),
            self.settlement_asset(),
            self.side(),
            trade_amount,
            self.price(),
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
    fn position_side(&self) -> Option<&'static str> {
        self.position_side.map(position_side_name)
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

    fn __repr__(&self) -> String {
        format!(
            "OrderPosition(position_side={:?}, reduce_only={:?}, close_position={:?})",
            self.position_side(),
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
        collateral_asset: Option<String>,
        auto_borrow: bool,
    ) -> PyResult<Self> {
        Ok(Self {
            leverage: leverage.map(parse_leverage_input).transpose()?,
            collateral_asset: collateral_asset.as_deref().map(parse_asset).transpose()?,
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
        self.collateral_asset.as_ref().map(ToString::to_string)
    }

    #[setter]
    fn set_collateral_asset(&mut self, value: Option<String>) -> PyResult<()> {
        self.collateral_asset = value.as_deref().map(parse_asset).transpose()?;
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

#[pyclass(name = "Asset", module = "openpit.param")]
#[derive(Clone)]
struct PyAsset {
    inner: Asset,
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

#[pymethods]
impl PyInstrument {
    #[new]
    #[pyo3(signature = (underlying_asset, settlement_asset))]
    fn new(underlying_asset: String, settlement_asset: String) -> PyResult<Self> {
        Ok(Self {
            inner: Instrument::new(
                parse_asset(&underlying_asset)?,
                parse_asset(&settlement_asset)?,
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
            .map(|v| v.bind(py).borrow().__repr__());
        let position = self
            .position
            .as_ref()
            .map(|v| v.bind(py).borrow().__repr__());
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
    fn new(multiplier: u16) -> PyResult<Self> {
        Ok(Self {
            inner: Leverage::from_u16(multiplier)
                .map_err(|error| PyValueError::new_err(error.to_string()))?,
        })
    }

    #[staticmethod]
    fn from_u16(multiplier: u16) -> PyResult<Self> {
        Ok(Self {
            inner: Leverage::from_u16(multiplier)
                .map_err(|error| PyValueError::new_err(error.to_string()))?,
        })
    }

    #[staticmethod]
    fn from_f64(multiplier: f64) -> PyResult<Self> {
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
    fn margin_required(&self, notional: f64) -> f64 {
        self.inner.margin_required(notional)
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
    fn from_str(value: &str) -> Self {
        Self {
            inner: AccountId::from_str(value),
        }
    }

    #[getter]
    fn value(&self) -> u64 {
        self.inner.as_u64()
    }

    fn __repr__(&self) -> String {
        format!("AccountId(value={:?})", self.value())
    }
}

#[pymethods]
impl PyAsset {
    #[new]
    fn new(value: String) -> PyResult<Self> {
        Ok(Self {
            inner: parse_asset(&value)?,
        })
    }

    #[getter]
    fn value(&self) -> String {
        self.inner.to_string()
    }

    fn __repr__(&self) -> String {
        format!("Asset(value={:?})", self.value())
    }
}

#[pymethods]
impl PyQuantity {
    #[new]
    fn new(value: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: parse_quantity_input(value)?,
        })
    }

    #[getter]
    fn value(&self) -> String {
        self.inner.to_string()
    }

    fn calculate_volume(&self, price: &PyPrice) -> PyResult<PyVolume> {
        Ok(PyVolume {
            inner: self
                .inner
                .calculate_volume(price.inner)
                .map_err(|error| PyValueError::new_err(error.to_string()))?,
        })
    }

    fn __repr__(&self) -> String {
        format!("Quantity(value={:?})", self.value())
    }
}

#[pymethods]
impl PyPrice {
    #[new]
    fn new(value: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: parse_price_input(value)?,
        })
    }

    #[getter]
    fn value(&self) -> String {
        self.inner.to_string()
    }

    fn calculate_volume(&self, quantity: &PyQuantity) -> PyResult<PyVolume> {
        Ok(PyVolume {
            inner: self
                .inner
                .calculate_volume(quantity.inner)
                .map_err(|error| PyValueError::new_err(error.to_string()))?,
        })
    }

    fn __repr__(&self) -> String {
        format!("Price(value={:?})", self.value())
    }
}

#[pymethods]
impl PyPnl {
    #[new]
    fn new(value: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: parse_pnl_input(value)?,
        })
    }

    #[getter]
    fn value(&self) -> String {
        self.inner.to_string()
    }

    fn __repr__(&self) -> String {
        format!("Pnl(value={:?})", self.value())
    }
}

#[pymethods]
impl PyFee {
    #[new]
    fn new(value: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: parse_fee_input(value)?,
        })
    }

    #[getter]
    fn value(&self) -> String {
        self.inner.to_string()
    }

    fn __repr__(&self) -> String {
        format!("Fee(value={:?})", self.value())
    }
}

#[pymethods]
impl PyVolume {
    #[new]
    fn new(value: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: parse_volume_input(value)?,
        })
    }

    #[getter]
    fn value(&self) -> String {
        self.inner.to_string()
    }

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

    fn __richcmp__(&self, other: &PyVolume, op: CompareOp) -> bool {
        match op {
            CompareOp::Lt => self.inner < other.inner,
            CompareOp::Le => self.inner <= other.inner,
            CompareOp::Eq => self.inner == other.inner,
            CompareOp::Ne => self.inner != other.inner,
            CompareOp::Gt => self.inner > other.inner,
            CompareOp::Ge => self.inner >= other.inner,
        }
    }

    fn __repr__(&self) -> String {
        format!("Volume(value={:?})", self.value())
    }
}

#[pymethods]
impl PyCashFlow {
    #[new]
    fn new(value: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: parse_cash_flow_input(value)?,
        })
    }

    #[getter]
    fn value(&self) -> String {
        self.inner.to_string()
    }

    fn __repr__(&self) -> String {
        format!("CashFlow(value={:?})", self.value())
    }
}

#[pymethods]
impl PyPositionSize {
    #[new]
    fn new(value: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: parse_position_size_input(value)?,
        })
    }

    #[getter]
    fn value(&self) -> String {
        self.inner.to_string()
    }

    fn __repr__(&self) -> String {
        format!("PositionSize(value={:?})", self.value())
    }
}

#[pyclass(name = "Request", module = "openpit.pretrade", unsendable)]
struct PyRequest {
    inner: RefCell<Option<Request<PythonOrder>>>,
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

#[pyclass(name = "Reservation", module = "openpit.pretrade", unsendable)]
struct PyReservation {
    inner: RefCell<Option<Reservation>>,
}

#[pymethods]
impl PyReservation {
    fn commit(&self) -> PyResult<()> {
        let reservation = self.take_reservation()?;
        reservation.commit();
        Ok(())
    }

    fn rollback(&self) -> PyResult<()> {
        let reservation = self.take_reservation()?;
        reservation.rollback();
        Ok(())
    }
}

impl PyReservation {
    fn take_reservation(&self) -> PyResult<Reservation> {
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
        underlying_asset: Option<String>,
        settlement_asset: Option<String>,
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
            underlying_asset: underlying_asset.as_deref().map(parse_asset).transpose()?,
            settlement_asset: settlement_asset.as_deref().map(parse_asset).transpose()?,
            account_id: account_id.map(parse_account_id_input).transpose()?,
            side: side.map(parse_side_input).transpose()?,
        })
    }

    #[getter]
    fn underlying_asset(&self) -> Option<String> {
        self.underlying_asset.as_ref().map(ToString::to_string)
    }

    #[setter]
    fn set_underlying_asset(&mut self, value: Option<String>) -> PyResult<()> {
        self.underlying_asset = value.as_deref().map(parse_asset).transpose()?;
        Ok(())
    }

    #[getter]
    fn settlement_asset(&self) -> Option<String> {
        self.settlement_asset.as_ref().map(ToString::to_string)
    }

    #[setter]
    fn set_settlement_asset(&mut self, value: Option<String>) -> PyResult<()> {
        self.settlement_asset = value.as_deref().map(parse_asset).transpose()?;
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
    fn side(&self) -> Option<&'static str> {
        self.side.map(side_name)
    }

    #[setter]
    fn set_side(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.side = value.map(parse_side_input).transpose()?;
        Ok(())
    }

    fn __repr__(&self) -> String {
        format!(
            "ExecutionReportOperation(underlying_asset={:?}, settlement_asset={:?}, account_id={:?}, side={:?})",
            self.underlying_asset(),
            self.settlement_asset(),
            self.account_id().map(|a| a.inner.as_u64()),
            self.side(),
        )
    }
}

#[pyclass(name = "FinancialImpact", module = "openpit.core", subclass)]
#[derive(Clone)]
struct PyFinancialImpact {
    pnl: Option<Pnl>,
    fee: Option<Fee>,
}

#[pymethods]
impl PyFinancialImpact {
    #[new]
    #[pyo3(signature = (*, pnl = None, fee = None))]
    fn new(pnl: Option<&Bound<'_, PyAny>>, fee: Option<&Bound<'_, PyAny>>) -> PyResult<Self> {
        Ok(Self {
            pnl: pnl.map(parse_pnl_input).transpose()?,
            fee: fee.map(parse_fee_input).transpose()?,
        })
    }

    #[getter]
    fn pnl(&self) -> Option<String> {
        self.pnl.as_ref().map(ToString::to_string)
    }

    #[setter]
    fn set_pnl(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.pnl = value.map(parse_pnl_input).transpose()?;
        Ok(())
    }

    #[getter]
    fn fee(&self) -> Option<String> {
        self.fee.as_ref().map(ToString::to_string)
    }

    #[setter]
    fn set_fee(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.fee = value.map(parse_fee_input).transpose()?;
        Ok(())
    }

    fn __repr__(&self) -> String {
        format!(
            "FinancialImpact(pnl={:?}, fee={:?})",
            self.pnl(),
            self.fee(),
        )
    }
}

#[pyclass(name = "ExecutionReportFillDetails", module = "openpit.core", subclass)]
#[derive(Clone)]
struct PyExecutionReportFillDetails {
    fill_price: Option<Price>,
    fill_quantity: Option<Quantity>,
    is_terminal: bool,
}

#[pymethods]
impl PyExecutionReportFillDetails {
    #[new]
    #[pyo3(signature = (*, fill_price = None, fill_quantity = None, is_terminal = false))]
    fn new(
        fill_price: Option<&Bound<'_, PyAny>>,
        fill_quantity: Option<&Bound<'_, PyAny>>,
        is_terminal: bool,
    ) -> PyResult<Self> {
        Ok(Self {
            fill_price: fill_price.map(parse_price_input).transpose()?,
            fill_quantity: fill_quantity.map(parse_quantity_input).transpose()?,
            is_terminal,
        })
    }

    #[getter]
    fn fill_price(&self) -> Option<String> {
        self.fill_price.as_ref().map(ToString::to_string)
    }

    #[setter]
    fn set_fill_price(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.fill_price = value.map(parse_price_input).transpose()?;
        Ok(())
    }

    #[getter]
    fn fill_quantity(&self) -> Option<String> {
        self.fill_quantity.as_ref().map(ToString::to_string)
    }

    #[setter]
    fn set_fill_quantity(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.fill_quantity = value.map(parse_quantity_input).transpose()?;
        Ok(())
    }

    #[getter]
    fn is_terminal(&self) -> bool {
        self.is_terminal
    }

    #[setter]
    fn set_is_terminal(&mut self, value: bool) {
        self.is_terminal = value;
    }

    fn __repr__(&self) -> String {
        format!(
            "ExecutionReportFillDetails(fill_price={:?}, fill_quantity={:?}, is_terminal={:?})",
            self.fill_price(),
            self.fill_quantity(),
            self.is_terminal(),
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
    fn position_effect(&self) -> Option<&'static str> {
        self.position_effect.map(position_effect_name)
    }

    #[setter]
    fn set_position_effect(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.position_effect = value.map(parse_position_effect_input).transpose()?;
        Ok(())
    }

    #[getter]
    fn position_side(&self) -> Option<&'static str> {
        self.position_side.map(position_side_name)
    }

    #[setter]
    fn set_position_side(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.position_side = value.map(parse_position_side_input).transpose()?;
        Ok(())
    }

    fn __repr__(&self) -> String {
        format!(
            "ExecutionReportPositionImpact(position_effect={:?}, position_side={:?})",
            self.position_effect(),
            self.position_side(),
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
            .map(|v| v.bind(py).borrow().__repr__());
        let financial_impact = self
            .financial_impact
            .as_ref()
            .map(|v| v.bind(py).borrow().__repr__());
        let fill = self.fill.as_ref().map(|v| v.bind(py).borrow().__repr__());
        let position_impact = self
            .position_impact
            .as_ref()
            .map(|v| v.bind(py).borrow().__repr__());
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

fn parse_account_id_input(value: &Bound<'_, PyAny>) -> PyResult<AccountId> {
    if let Ok(v) = value.extract::<u64>() {
        return Ok(AccountId::from_u64(v));
    }
    if let Ok(v) = value.extract::<PyAccountId>() {
        return Ok(v.inner);
    }
    if let Ok(s) = value.extract::<String>() {
        return Ok(AccountId::from_str(s));
    }
    Err(PyTypeError::new_err(
        "account_id must be openpit.param.AccountId, int, or str",
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

fn parse_quantity(value: &str) -> PyResult<Quantity> {
    Quantity::from_str(value).map_err(|error| PyValueError::new_err(error.to_string()))
}

fn parse_asset(value: &str) -> PyResult<Asset> {
    Asset::new(value).map_err(|error| PyValueError::new_err(error.to_string()))
}

fn parse_price(value: &str) -> PyResult<Price> {
    Price::from_str(value).map_err(|error| PyValueError::new_err(error.to_string()))
}

fn parse_pnl(value: &str) -> PyResult<Pnl> {
    Pnl::from_str(value).map_err(|error| PyValueError::new_err(error.to_string()))
}

fn parse_fee(value: &str) -> PyResult<Fee> {
    Fee::from_str(value).map_err(|error| PyValueError::new_err(error.to_string()))
}

fn parse_volume(value: &str) -> PyResult<Volume> {
    Volume::from_str(value).map_err(|error| PyValueError::new_err(error.to_string()))
}

fn parse_cash_flow(value: &str) -> PyResult<CashFlow> {
    CashFlow::from_str(value).map_err(|error| PyValueError::new_err(error.to_string()))
}

fn parse_position_size(value: &str) -> PyResult<PositionSize> {
    PositionSize::from_str(value).map_err(|error| PyValueError::new_err(error.to_string()))
}

fn parse_leverage_input(value: &Bound<'_, PyAny>) -> PyResult<Leverage> {
    if value.extract::<bool>().is_ok() {
        return Err(PyValueError::new_err("leverage must be a Leverage or int"));
    }

    if let Ok(value) = value.extract::<PyRef<'_, PyLeverage>>() {
        return Ok(value.inner);
    }

    if let Ok(value) = value.extract::<u16>() {
        return Leverage::from_u16(value).map_err(|error| PyValueError::new_err(error.to_string()));
    }

    Err(PyValueError::new_err("leverage must be a Leverage or int"))
}

fn parse_decimal_input<T, ParseStr, ParseF64>(
    value: &Bound<'_, PyAny>,
    type_name: &str,
    parse_str_fn: ParseStr,
    parse_f64_fn: ParseF64,
) -> PyResult<T>
where
    ParseStr: Fn(&str) -> PyResult<T>,
    ParseF64: Fn(f64) -> PyResult<T>,
{
    if value.extract::<bool>().is_ok() {
        return Err(PyValueError::new_err(format!(
            "{type_name} must be a str, int, or float"
        )));
    }

    if let Ok(value) = value.extract::<String>() {
        return parse_str_fn(&value);
    }

    if let Ok(value) = value.extract::<i64>() {
        return parse_str_fn(&value.to_string());
    }

    if let Ok(value) = value.extract::<u64>() {
        return parse_str_fn(&value.to_string());
    }

    if let Ok(value) = value.extract::<f64>() {
        return parse_f64_fn(value);
    }

    Err(PyValueError::new_err(format!(
        "{type_name} must be a str, int, or float"
    )))
}

fn parse_quantity_input(value: &Bound<'_, PyAny>) -> PyResult<Quantity> {
    if let Ok(value) = value.extract::<PyRef<'_, PyQuantity>>() {
        return Ok(value.inner);
    }
    parse_decimal_input(value, "quantity", parse_quantity, |value| {
        Quantity::from_f64(value).map_err(|error| PyValueError::new_err(error.to_string()))
    })
}

fn parse_price_input(value: &Bound<'_, PyAny>) -> PyResult<Price> {
    if let Ok(value) = value.extract::<PyRef<'_, PyPrice>>() {
        return Ok(value.inner);
    }
    parse_decimal_input(value, "price", parse_price, |value| {
        Price::from_f64(value).map_err(|error| PyValueError::new_err(error.to_string()))
    })
}

fn parse_pnl_input(value: &Bound<'_, PyAny>) -> PyResult<Pnl> {
    if let Ok(value) = value.extract::<PyRef<'_, PyPnl>>() {
        return Ok(value.inner);
    }
    parse_decimal_input(value, "pnl", parse_pnl, |value| {
        Pnl::from_f64(value).map_err(|error| PyValueError::new_err(error.to_string()))
    })
}

fn parse_fee_input(value: &Bound<'_, PyAny>) -> PyResult<Fee> {
    if let Ok(value) = value.extract::<PyRef<'_, PyFee>>() {
        return Ok(value.inner);
    }
    parse_decimal_input(value, "fee", parse_fee, |value| {
        Fee::from_f64(value).map_err(|error| PyValueError::new_err(error.to_string()))
    })
}

fn parse_volume_input(value: &Bound<'_, PyAny>) -> PyResult<Volume> {
    if let Ok(value) = value.extract::<PyRef<'_, PyVolume>>() {
        return Ok(value.inner);
    }
    parse_decimal_input(value, "volume", parse_volume, |value| {
        Volume::from_f64(value).map_err(|error| PyValueError::new_err(error.to_string()))
    })
}

fn parse_trade_amount_input(value: &Bound<'_, PyAny>) -> PyResult<TradeAmount> {
    if let Ok(value) = value.extract::<PyRef<'_, PyQuantity>>() {
        return Ok(TradeAmount::Quantity(value.inner));
    }
    if let Ok(value) = value.extract::<PyRef<'_, PyVolume>>() {
        return Ok(TradeAmount::Volume(value.inner));
    }
    Err(PyTypeError::new_err(
        "trade_amount must be openpit.param.Quantity or openpit.param.Volume",
    ))
}

fn trade_amount_to_python(py: Python<'_>, value: TradeAmount) -> PyResult<Py<PyAny>> {
    match value {
        TradeAmount::Quantity(quantity) => {
            Ok(Py::new(py, PyQuantity { inner: quantity })?.into_any())
        }
        TradeAmount::Volume(volume) => Ok(Py::new(py, PyVolume { inner: volume })?.into_any()),
        _ => Err(PyValueError::new_err("unrecognized trade amount type")),
    }
}

fn trade_amount_debug(value: &TradeAmount) -> String {
    match value {
        TradeAmount::Quantity(quantity) => format!("Quantity(value={:?})", quantity),
        TradeAmount::Volume(volume) => format!("Volume(value={:?})", volume),
        _ => "TradeAmount(<unsupported>)".to_string(),
    }
}

fn parse_cash_flow_input(value: &Bound<'_, PyAny>) -> PyResult<CashFlow> {
    parse_decimal_input(value, "cash flow", parse_cash_flow, |value| {
        CashFlow::from_f64(value).map_err(|error| PyValueError::new_err(error.to_string()))
    })
}

fn parse_position_size_input(value: &Bound<'_, PyAny>) -> PyResult<PositionSize> {
    parse_decimal_input(value, "position size", parse_position_size, |value| {
        PositionSize::from_f64(value).map_err(|error| PyValueError::new_err(error.to_string()))
    })
}

fn convert_reject(reject: &Reject) -> PyReject {
    PyReject {
        code: reject_code_name(reject.code).to_owned(),
        reason: reject.reason.clone(),
        details: reject.details.clone(),
        policy: reject.policy.to_owned(),
        scope: reject_scope_name(&reject.scope).to_owned(),
    }
}

fn reject_scope_name(scope: &RejectScope) -> &'static str {
    match scope {
        RejectScope::Order => "order",
        RejectScope::Account => "account",
    }
}

fn reject_code_name(code: RejectCode) -> &'static str {
    code.as_str()
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
    module.add_class::<PyEngine>()?;
    module.add_class::<PyRejectCode>()?;
    module.add_class::<PyReject>()?;
    module.add_class::<PyStartPreTradeResult>()?;
    module.add_class::<PyExecuteResult>()?;
    module.add_class::<PyEngineBuilder>()?;
    module.add_class::<PyInstrument>()?;
    module.add_class::<PyOrderOperation>()?;
    module.add_class::<PyOrderPosition>()?;
    module.add_class::<PyOrderMargin>()?;
    module.add_class::<PyOrder>()?;
    module.add_class::<PyAccountId>()?;
    module.add_class::<PyAsset>()?;
    module.add_class::<PyQuantity>()?;
    module.add_class::<PyPrice>()?;
    module.add_class::<PyPnl>()?;
    module.add_class::<PyFee>()?;
    module.add_class::<PyVolume>()?;
    module.add_class::<PyCashFlow>()?;
    module.add_class::<PyPositionSize>()?;
    module.add_class::<PyLeverage>()?;
    module.add_class::<PyRequest>()?;
    module.add_class::<PyReservation>()?;
    module.add_class::<PyExecutionReportOperation>()?;
    module.add_class::<PyFinancialImpact>()?;
    module.add_class::<PyExecutionReportFillDetails>()?;
    module.add_class::<PyExecutionReportPositionImpact>()?;
    module.add_class::<PyExecutionReport>()?;
    module.add_class::<PyPostTradeResult>()?;
    module.add_class::<PyPnlKillSwitchPolicy>()?;
    module.add_class::<PyRateLimitPolicy>()?;
    module.add_class::<PyOrderValidationPolicy>()?;
    module.add_class::<PyOrderSizeLimit>()?;
    module.add_class::<PyOrderSizeLimitPolicy>()?;
    Ok(())
}
