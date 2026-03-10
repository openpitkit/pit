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
    Asset, CashFlow, Fee, Leverage, Pnl, PositionSide, PositionSize, Price, Quantity, Side, Volume,
};
use openpit::pretrade::policies::OrderValidationPolicy;
use openpit::pretrade::policies::PnlKillSwitchPolicy;
use openpit::pretrade::policies::RateLimitPolicy;
use openpit::pretrade::policies::{OrderSizeLimit, OrderSizeLimitPolicy};
use openpit::pretrade::{
    CheckPreTradeStartPolicy, ExecutionReport, Mutation, Mutations, Policy, PostTradeResult,
    Reject, RejectCode, RejectScope, Request, Reservation, RiskMutation,
};
use openpit::{Engine, EngineBuildError, Instrument, Order};
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

#[pyclass(name = "Engine", module = "openpit", unsendable)]
struct PyEngine {
    inner: Engine,
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

    #[pyo3(signature = (*, order))]
    fn start_pre_trade(&self, py: Python<'_>, order: &PyOrder) -> PyResult<PyStartPreTradeResult> {
        clear_python_callback_error();
        match self.inner.start_pre_trade(order.inner.clone()) {
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

    #[pyo3(signature = (*, report))]
    fn apply_execution_report(&self, report: &PyExecutionReport) -> PyResult<PyPostTradeResult> {
        clear_python_callback_error();
        let result = PyPostTradeResult {
            inner: self.inner.apply_execution_report(&report.inner),
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

struct SharedPnlStartPolicy {
    inner: Rc<PnlKillSwitchPolicy>,
}

struct PythonStartPolicyAdapter {
    name: &'static str,
    policy: Py<PyAny>,
}

struct PythonMainPolicyAdapter {
    name: &'static str,
    policy: Py<PyAny>,
}

impl CheckPreTradeStartPolicy for SharedPnlStartPolicy {
    fn name(&self) -> &'static str {
        self.inner.name()
    }

    fn check_pre_trade_start(&self, order: &Order) -> Result<(), Reject> {
        self.inner.check_pre_trade_start(order)
    }

    fn apply_execution_report(&self, report: &ExecutionReport) -> bool {
        self.inner.apply_execution_report(report)
    }
}

impl CheckPreTradeStartPolicy for PythonStartPolicyAdapter {
    fn name(&self) -> &'static str {
        self.name
    }

    fn check_pre_trade_start(&self, order: &Order) -> Result<(), Reject> {
        Python::with_gil(|py| {
            let py_order = Py::new(
                py,
                PyOrder {
                    inner: order.clone(),
                },
            )
            .map_err(|error| {
                set_python_callback_error(error);
                python_callback_reject(self.name)
            })?;

            let kwargs = PyDict::new_bound(py);
            kwargs.set_item("order", py_order).map_err(|error| {
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

    fn apply_execution_report(&self, report: &ExecutionReport) -> bool {
        Python::with_gil(|py| {
            let py_report = match Py::new(
                py,
                PyExecutionReport {
                    inner: report.clone(),
                },
            ) {
                Ok(report) => report,
                Err(error) => {
                    set_python_callback_error(error);
                    return false;
                }
            };

            let kwargs = PyDict::new_bound(py);
            if let Err(error) = kwargs.set_item("report", py_report) {
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

impl Policy for PythonMainPolicyAdapter {
    fn name(&self) -> &'static str {
        self.name
    }

    fn perform_pre_trade_check(
        &self,
        ctx: &openpit::pretrade::Context<'_>,
        mutations: &mut Mutations,
        rejects: &mut Vec<Reject>,
    ) {
        Python::with_gil(|py| {
            let py_context = match build_python_policy_context(py, ctx.order(), ctx.notional()) {
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

    fn apply_execution_report(&self, report: &ExecutionReport) -> bool {
        Python::with_gil(|py| {
            let py_report = match Py::new(
                py,
                PyExecutionReport {
                    inner: report.clone(),
                },
            ) {
                Ok(report) => report,
                Err(error) => {
                    set_python_callback_error(error);
                    return false;
                }
            };

            let kwargs = PyDict::new_bound(py);
            if let Err(error) = kwargs.set_item("report", py_report) {
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

fn build_python_policy_context(
    py: Python<'_>,
    order: &Order,
    notional: openpit::param::CashFlow,
) -> PyResult<Py<PyAny>> {
    let module = PyModule::import_bound(py, "openpit.pretrade")?;
    let cls = module.getattr("PolicyContext")?;
    let py_order = Py::new(
        py,
        PyOrder {
            inner: order.clone(),
        },
    )?;
    let kwargs = PyDict::new_bound(py);
    kwargs.set_item("order", py_order)?;
    kwargs.set_item("notional", notional.to_decimal().to_string())?;
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
        "reserve_notional" => Ok(RiskMutation::ReserveNotional {
            asset: parse_asset(
                value
                    .getattr("settlement_asset")?
                    .extract::<String>()
                    .map_err(|_| {
                        PyValueError::new_err("reserve_notional.settlement_asset must be a string")
                    })?
                    .as_str(),
            )?,
            amount: parse_volume_input(&value.getattr("amount")?)?,
        }),
        "set_kill_switch" => {
            let id = value
                .getattr("id")?
                .extract::<String>()
                .map_err(|_| PyValueError::new_err("set_kill_switch.id must be a string"))?;
            if id.trim().is_empty() {
                return Err(PyValueError::new_err(
                    "set_kill_switch.id must not be empty",
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
    #[pyo3(signature = (*, policy))]
    fn check_pre_trade_start_policy<'py>(
        slf: PyRef<'py, Self>,
        policy: &Bound<'_, PyAny>,
    ) -> PyResult<PyRef<'py, Self>> {
        slf.push_start_policy(policy)?;
        Ok(slf)
    }

    #[pyo3(signature = (*, policy))]
    fn pre_trade_policy<'py>(
        slf: PyRef<'py, Self>,
        policy: &Bound<'_, PyAny>,
    ) -> PyResult<PyRef<'py, Self>> {
        slf.push_main_policy(policy)?;
        Ok(slf)
    }

    fn build(&self) -> PyResult<PyEngine> {
        let mut builder = Engine::builder();

        for policy in self.start_policies.borrow().iter() {
            builder = match policy {
                StartPolicyConfig::OrderValidation => {
                    builder.check_pre_trade_start_policy(OrderValidationPolicy::new())
                }
                StartPolicyConfig::PnlKillSwitchShared { policy } => builder
                    .check_pre_trade_start_policy(SharedPnlStartPolicy {
                        inner: Rc::clone(policy),
                    }),
                StartPolicyConfig::RateLimit {
                    max_orders,
                    window_seconds,
                } => builder.check_pre_trade_start_policy(RateLimitPolicy::new(
                    *max_orders,
                    Duration::from_secs(*window_seconds),
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
                    builder.check_pre_trade_start_policy(rust_policy)
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
    #[pyo3(signature = (*, settlement_asset, barrier))]
    fn new(settlement_asset: String, barrier: &Bound<'_, PyAny>) -> PyResult<Self> {
        parse_asset(&settlement_asset)?;
        let barrier = parse_pnl_input(barrier)?.to_decimal().to_string();
        Ok(Self {
            barriers: RefCell::new(vec![(settlement_asset, barrier)]),
            runtime_policy: RefCell::new(None),
        })
    }

    #[pyo3(signature = (*, settlement_asset, barrier))]
    fn set_barrier(&self, settlement_asset: String, barrier: &Bound<'_, PyAny>) -> PyResult<()> {
        if self.runtime_policy.borrow().is_some() {
            return Err(PyRuntimeError::new_err(
                "pnl policy is already bound to an engine and cannot be reconfigured",
            ));
        }

        parse_asset(&settlement_asset)?;
        let barrier = parse_pnl_input(barrier)?.to_decimal().to_string();

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

    #[pyo3(signature = (*, settlement_asset))]
    fn reset_pnl(&self, settlement_asset: String) -> PyResult<()> {
        let settlement_asset = parse_asset(&settlement_asset)?;
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
    #[pyo3(signature = (*, max_orders, window_seconds))]
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
        settlement_asset: String,
        max_quantity: &Bound<'_, PyAny>,
        max_notional: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        parse_asset(&settlement_asset)?;
        let max_quantity = parse_quantity_input(max_quantity)?.to_decimal().to_string();
        let max_notional = parse_volume_input(max_notional)?.to_decimal().to_string();

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
    #[pyo3(signature = (*, limit))]
    fn new(limit: &PyOrderSizeLimit) -> Self {
        Self {
            limits: RefCell::new(vec![limit.inner.clone()]),
        }
    }

    #[pyo3(signature = (*, limit))]
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

#[pyclass(name = "Order", module = "openpit.core")]
#[derive(Clone)]
struct PyOrder {
    inner: Order,
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

#[pyclass(name = "Asset", module = "openpit.param")]
#[derive(Clone)]
struct PyAsset {
    inner: Asset,
}

#[pyclass(name = "Side", module = "openpit.param")]
#[derive(Clone, Copy)]
struct PySide {
    inner: Side,
}

#[pyclass(name = "PositionSide", module = "openpit.param")]
#[derive(Clone, Copy)]
struct PyPositionSide {
    inner: PositionSide,
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

#[pyclass(name = "ParamKind", module = "openpit.param")]
struct PyParamKind;

#[pyclass(name = "RoundingStrategy", module = "openpit.param")]
struct PyRoundingStrategy;

#[pymethods]
impl PyInstrument {
    #[new]
    #[pyo3(signature = (*, underlying_asset, settlement_asset))]
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

#[pymethods]
impl PyOrder {
    #[new]
    #[pyo3(signature = (*, underlying_asset, settlement_asset, side, quantity, price))]
    fn new(
        underlying_asset: String,
        settlement_asset: String,
        side: &str,
        quantity: &Bound<'_, PyAny>,
        price: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        let underlying_asset = parse_asset(&underlying_asset)?;
        let settlement_asset = parse_asset(&settlement_asset)?;
        Ok(Self {
            inner: Order {
                instrument: Instrument::new(underlying_asset, settlement_asset),
                side: parse_side(side)?,
                quantity: parse_quantity_input(quantity)?,
                price: parse_price_input(price)?,
            },
        })
    }

    #[getter]
    fn underlying_asset(&self) -> String {
        self.inner.instrument.underlying_asset().to_string()
    }

    #[getter]
    fn settlement_asset(&self) -> String {
        self.inner.instrument.settlement_asset().to_string()
    }

    #[getter]
    fn side(&self) -> &'static str {
        side_name(self.inner.side)
    }

    #[getter]
    fn quantity(&self) -> String {
        self.inner.quantity.to_decimal().to_string()
    }

    #[getter]
    fn price(&self) -> String {
        self.inner.price.to_decimal().to_string()
    }

    fn __repr__(&self) -> String {
        format!(
            "Order(underlying_asset={:?}, settlement_asset={:?}, side={:?}, quantity={:?}, price={:?})",
            self.underlying_asset(),
            self.settlement_asset(),
            self.side(),
            self.quantity(),
            self.price()
        )
    }
}

#[pymethods]
impl PyLeverage {
    #[new]
    fn new(multiplier: u16) -> PyResult<Self> {
        Ok(Self {
            inner: Leverage::new(multiplier)
                .map_err(|error| PyValueError::new_err(error.to_string()))?,
        })
    }

    #[staticmethod]
    fn from_multiplier(multiplier: u16) -> PyResult<Self> {
        Ok(Self {
            inner: Leverage::from_multiplier(multiplier)
                .map_err(|error| PyValueError::new_err(error.to_string()))?,
        })
    }

    #[staticmethod]
    fn from_stored(raw: u16) -> PyResult<Self> {
        Ok(Self {
            inner: Leverage::from_stored(raw)
                .map_err(|error| PyValueError::new_err(error.to_string()))?,
        })
    }

    #[getter]
    fn value(&self) -> f32 {
        self.inner.value()
    }

    #[pyo3(signature = (*, notional))]
    fn margin_required(&self, notional: f64) -> f64 {
        self.inner.margin_required(notional)
    }

    fn __repr__(&self) -> String {
        format!("Leverage(value={:?})", self.value())
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
impl PySide {
    #[new]
    fn new(value: &str) -> PyResult<Self> {
        Ok(Self {
            inner: parse_side(value)?,
        })
    }

    #[getter]
    fn value(&self) -> &'static str {
        side_name(self.inner)
    }

    fn is_buy(&self) -> bool {
        self.inner.is_buy()
    }

    fn is_sell(&self) -> bool {
        self.inner.is_sell()
    }

    fn opposite(&self) -> Self {
        Self {
            inner: self.inner.opposite(),
        }
    }

    fn sign(&self) -> i8 {
        self.inner.sign()
    }

    fn __repr__(&self) -> String {
        format!("Side(value={:?})", self.value())
    }
}

#[pymethods]
impl PyPositionSide {
    #[new]
    fn new(value: &str) -> PyResult<Self> {
        Ok(Self {
            inner: parse_position_side(value)?,
        })
    }

    #[getter]
    fn value(&self) -> &'static str {
        position_side_name(self.inner)
    }

    fn is_long(&self) -> bool {
        self.inner.is_long()
    }

    fn is_short(&self) -> bool {
        self.inner.is_short()
    }

    fn opposite(&self) -> Self {
        Self {
            inner: self.inner.opposite(),
        }
    }

    fn __repr__(&self) -> String {
        format!("PositionSide(value={:?})", self.value())
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
        self.inner.to_decimal().to_string()
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
        self.inner.to_decimal().to_string()
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
        self.inner.to_decimal().to_string()
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
        self.inner.to_decimal().to_string()
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
        self.inner.to_decimal().to_string()
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
        self.inner.to_decimal().to_string()
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
        self.inner.to_decimal().to_string()
    }

    fn __repr__(&self) -> String {
        format!("PositionSize(value={:?})", self.value())
    }
}

#[pymethods]
impl PyParamKind {
    #[classattr]
    const QUANTITY: &'static str = "Quantity";
    #[classattr]
    const VOLUME: &'static str = "Volume";
    #[classattr]
    const PRICE: &'static str = "Price";
    #[classattr]
    const PNL: &'static str = "Pnl";
    #[classattr]
    const CASH_FLOW: &'static str = "CashFlow";
    #[classattr]
    const POSITION_SIZE: &'static str = "PositionSize";
    #[classattr]
    const FEE: &'static str = "Fee";
    #[classattr]
    const LEVERAGE: &'static str = "Leverage";
}

#[pymethods]
impl PyRoundingStrategy {
    #[classattr]
    const MIDPOINT_NEAREST_EVEN: &'static str = "MidpointNearestEven";
    #[classattr]
    const MIDPOINT_AWAY_FROM_ZERO: &'static str = "MidpointAwayFromZero";
    #[classattr]
    const UP: &'static str = "Up";
    #[classattr]
    const DOWN: &'static str = "Down";
    #[classattr]
    const DEFAULT: &'static str = "MidpointNearestEven";
    #[classattr]
    const BANKER: &'static str = "MidpointNearestEven";
    #[classattr]
    const CONSERVATIVE_PROFIT: &'static str = "Down";
    #[classattr]
    const CONSERVATIVE_LOSS: &'static str = "Down";
}

#[pyclass(name = "Request", module = "openpit.pretrade", unsendable)]
struct PyRequest {
    inner: RefCell<Option<Request>>,
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

#[pyclass(name = "ExecutionReport", module = "openpit.pretrade")]
#[derive(Clone)]
struct PyExecutionReport {
    inner: ExecutionReport,
}

#[pymethods]
impl PyExecutionReport {
    #[new]
    #[pyo3(signature = (*, underlying_asset, settlement_asset, pnl, fee = None))]
    fn new(
        underlying_asset: String,
        settlement_asset: String,
        pnl: &Bound<'_, PyAny>,
        fee: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let underlying_asset = parse_asset(&underlying_asset)?;
        let settlement_asset = parse_asset(&settlement_asset)?;
        Ok(Self {
            inner: ExecutionReport {
                instrument: Instrument::new(underlying_asset, settlement_asset),
                pnl: parse_pnl_input(pnl)?,
                fee: match fee {
                    Some(fee) => parse_fee_input(fee)?,
                    None => Fee::ZERO,
                },
            },
        })
    }

    #[getter]
    fn underlying_asset(&self) -> String {
        self.inner.instrument.underlying_asset().to_string()
    }

    #[getter]
    fn settlement_asset(&self) -> String {
        self.inner.instrument.settlement_asset().to_string()
    }

    #[getter]
    fn pnl(&self) -> String {
        self.inner.pnl.to_decimal().to_string()
    }

    #[getter]
    fn fee(&self) -> String {
        self.inner.fee.to_decimal().to_string()
    }

    fn __repr__(&self) -> String {
        format!(
            "ExecutionReport(underlying_asset={:?}, settlement_asset={:?}, pnl={:?}, fee={:?})",
            self.underlying_asset(),
            self.settlement_asset(),
            self.pnl(),
            self.fee()
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

fn position_side_name(value: PositionSide) -> &'static str {
    match value {
        PositionSide::Long => "long",
        PositionSide::Short => "short",
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
    parse_decimal_input(value, "quantity", parse_quantity, |value| {
        Quantity::from_f64(value).map_err(|error| PyValueError::new_err(error.to_string()))
    })
}

fn parse_price_input(value: &Bound<'_, PyAny>) -> PyResult<Price> {
    parse_decimal_input(value, "price", parse_price, |value| {
        Price::from_f64(value).map_err(|error| PyValueError::new_err(error.to_string()))
    })
}

fn parse_pnl_input(value: &Bound<'_, PyAny>) -> PyResult<Pnl> {
    parse_decimal_input(value, "pnl", parse_pnl, |value| {
        Pnl::from_f64(value).map_err(|error| PyValueError::new_err(error.to_string()))
    })
}

fn parse_fee_input(value: &Bound<'_, PyAny>) -> PyResult<Fee> {
    parse_decimal_input(value, "fee", parse_fee, |value| {
        Fee::from_f64(value).map_err(|error| PyValueError::new_err(error.to_string()))
    })
}

fn parse_volume_input(value: &Bound<'_, PyAny>) -> PyResult<Volume> {
    parse_decimal_input(value, "volume", parse_volume, |value| {
        Volume::from_f64(value).map_err(|error| PyValueError::new_err(error.to_string()))
    })
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
    module.add_class::<PyOrder>()?;
    module.add_class::<PyAsset>()?;
    module.add_class::<PySide>()?;
    module.add_class::<PyPositionSide>()?;
    module.add_class::<PyQuantity>()?;
    module.add_class::<PyPrice>()?;
    module.add_class::<PyPnl>()?;
    module.add_class::<PyFee>()?;
    module.add_class::<PyVolume>()?;
    module.add_class::<PyCashFlow>()?;
    module.add_class::<PyPositionSize>()?;
    module.add_class::<PyLeverage>()?;
    module.add_class::<PyParamKind>()?;
    module.add_class::<PyRoundingStrategy>()?;
    module.add_class::<PyRequest>()?;
    module.add_class::<PyReservation>()?;
    module.add_class::<PyExecutionReport>()?;
    module.add_class::<PyPostTradeResult>()?;
    module.add_class::<PyPnlKillSwitchPolicy>()?;
    module.add_class::<PyRateLimitPolicy>()?;
    module.add_class::<PyOrderValidationPolicy>()?;
    module.add_class::<PyOrderSizeLimit>()?;
    module.add_class::<PyOrderSizeLimitPolicy>()?;
    Ok(())
}
