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

use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::thread_local;
use std::time::Duration;

use openpit::marketdata::QuoteResolution;
use openpit::param::{AccountGroupId, AccountGroupIdError};
use openpit::param::{
    AccountId, AdjustmentAmount, Asset, CashFlow, Fee, Leverage, Notional, Pnl, PositionEffect,
    PositionMode, PositionSide, PositionSize, Price, Quantity, RoundingStrategy, Side, Trade,
    TradeAmount, Volume,
};
use openpit::pretrade::policies::OrderValidationPolicy;
use openpit::pretrade::policies::PnlBoundsAccountAssetBarrier;
use openpit::pretrade::policies::PnlBoundsAccountAssetBarrierUpdate;
use openpit::pretrade::policies::PnlBoundsBrokerBarrier;
use openpit::pretrade::policies::PnlBoundsKillSwitchPolicy;
use openpit::pretrade::policies::PnlBoundsKillSwitchSettings;
use openpit::pretrade::policies::SpotFundsPolicy;
use openpit::pretrade::policies::SpotFundsSettings;
use openpit::pretrade::policies::{
    OrderSizeAccountAssetBarrier, OrderSizeAssetBarrier, OrderSizeLimit, OrderSizeLimitPolicy,
    OrderSizeLimitSettings,
};
use openpit::pretrade::policies::{
    RateLimit, RateLimitAccountAssetBarrier, RateLimitAccountBarrier, RateLimitAssetBarrier,
    RateLimitBrokerBarrier, RateLimitPolicy, RateLimitSettings,
};
use openpit::pretrade::PostTradeContext;
use openpit::pretrade::{
    PolicyPreTradeResult, PreTradeContext, PreTradeLock, PreTradePolicy, PreTradeRequest,
    PreTradeReservation, Reject, RejectCode, RejectScope, Rejects,
};
use openpit::storage::StorageBuilder;
use openpit::AccountAdjustmentContext;
use openpit::{
    AccountAdjustmentOutcome, AccountOutcomeEntry, Engine, EngineBuildError, EngineBuilder,
    Instrument, Mutation, Mutations, OutcomeAmount, PostTradeResult,
};
use openpit::{AccountGroupError, Accounts, Configurator, PolicyGroupId, DEFAULT_POLICY_GROUP_ID};
use openpit::{
    ConfigureError, InstrumentId, MarketDataBuilder, MarketDataService, Quote, QuoteTtl,
    SpotFundsMarketData, SpotFundsOverride, SpotFundsOverrideTarget, SpotFundsPricingSource,
};
use openpit_interop::{
    AccountAdjustmentAmountAccess, AccountAdjustmentBoundsAccess, AccountAdjustmentOperationAccess,
    EngineHandle, EngineLocking, ExecutionReportFillAccess, ExecutionReportOperationAccess,
    ExecutionReportPositionImpactAccess, FinancialImpactAccess, OrderMarginAccess,
    OrderOperationAccess, OrderPositionAccess, PopulatedAccountAdjustmentOperation,
    PopulatedBalanceOperation, PopulatedExecutionReportFill, PopulatedExecutionReportOperation,
    PopulatedExecutionReportPositionImpact, PopulatedFinancialImpact, PopulatedOrderMargin,
    PopulatedOrderOperation, PopulatedOrderPosition, PopulatedPositionOperation, SyncMode,
};
use pyo3::basic::CompareOp;
use pyo3::create_exception;
use pyo3::exceptions::{PyException, PyRuntimeError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyInt, PyModule};
use rust_decimal::prelude::ToPrimitive;

create_exception!(openpit, RejectError, PyException);
create_exception!(openpit, ParamError, PyValueError);
create_exception!(openpit, MarketDataError, PyException);
create_exception!(openpit, UnknownInstrument, MarketDataError);
create_exception!(openpit, QuoteUnavailable, MarketDataError);
create_exception!(openpit, AlreadyRegistered, PyException);
create_exception!(openpit, RegistrationError, PyException);
create_exception!(openpit, UnknownInstrumentId, PyException);
create_exception!(openpit, AccountGroupRegistrationError, PyException);
create_exception!(openpit, AccountBlockError, PyException);
create_exception!(openpit, PolicyConfigureError, PyException);

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

struct DetachedFromGil<T>(T);

// SAFETY: `Python::allow_threads` runs the closure synchronously on the current
// OS thread and returns before Python-visible objects are constructed. This
// wrapper is used only for Rust-owned SDK result handles that do not borrow the
// Python token but are intentionally `!Send` as public SDK values.
unsafe impl<T> Send for DetachedFromGil<T> {}

fn allow_threads_detached<T, F>(py: Python<'_>, f: F) -> T
where
    F: FnOnce() -> T + Send,
{
    py.allow_threads(|| DetachedFromGil(f())).0
}

fn create_param_error(message: impl Into<String>) -> PyErr {
    ParamError::new_err(message.into())
}

fn create_already_registered_error(error: openpit::AlreadyRegistered) -> PyErr {
    AlreadyRegistered::new_err(error.to_string())
}

fn create_registration_error(error: openpit::RegistrationError) -> PyErr {
    RegistrationError::new_err(error.to_string())
}

fn create_unknown_instrument_id_error(error: openpit::UnknownInstrumentId) -> PyErr {
    UnknownInstrumentId::new_err(error.to_string())
}

fn create_market_data_error(error: openpit::MarketDataError) -> PyErr {
    match error {
        openpit::MarketDataError::UnknownInstrument => {
            UnknownInstrument::new_err(error.to_string())
        }
        openpit::MarketDataError::QuoteUnavailable => QuoteUnavailable::new_err(error.to_string()),
        _ => MarketDataError::new_err(error.to_string()),
    }
}

fn create_push_for_error(error: openpit::marketdata::PushForError) -> PyErr {
    match error {
        openpit::marketdata::PushForError::UnknownInstrument { instrument_id } => {
            create_unknown_instrument_id_error(openpit::UnknownInstrumentId { instrument_id })
        }
        openpit::marketdata::PushForError::NoTarget => {
            PyValueError::new_err("push_for requires at least one account or group target")
        }
        _ => PyRuntimeError::new_err(error.to_string()),
    }
}

fn convert_account_group_error(error: AccountGroupError) -> PyErr {
    AccountGroupRegistrationError::new_err(error.to_string())
}

fn convert_account_block_error(error: openpit::AccountBlockError) -> PyErr {
    AccountBlockError::new_err(error.to_string())
}

#[pyclass(name = "ConfigureErrorKind", module = "openpit", frozen, eq, eq_int)]
#[derive(Clone, Copy, PartialEq, Eq)]
// Integer values mirror the C `OpenPitConfigureErrorKind` so the Python,
// Go, and C bindings agree on the discriminants.
enum PyConfigureErrorKind {
    /// No registered policy carries the requested name.
    #[pyo3(name = "UNKNOWN")]
    Unknown = 0,
    /// The named policy exists but its settings type differs from the target.
    #[pyo3(name = "TYPE_MISMATCH")]
    TypeMismatch = 1,
    /// The update was rejected; the prior value still applies.
    #[pyo3(name = "VALIDATION")]
    Validation = 2,
}

impl PyConfigureErrorKind {
    fn of(error: &ConfigureError) -> Self {
        match error {
            ConfigureError::UnknownPolicy { .. } => Self::Unknown,
            ConfigureError::PolicyTypeMismatch { .. } => Self::TypeMismatch,
            ConfigureError::Validation { .. } => Self::Validation,
            // Unreachable via FFI (the configure closure runs no user callback,
            // so no same-thread re-entry), mapped to Validation for completeness.
            ConfigureError::NestedConfiguration => Self::Validation,
            // `ConfigureError` is `#[non_exhaustive]`; an unforeseen variant is
            // reported as a validation failure, matching the C binding default.
            _ => Self::Validation,
        }
    }
}

fn convert_configure_error(error: ConfigureError) -> PyErr {
    let kind = PyConfigureErrorKind::of(&error);
    let err = PolicyConfigureError::new_err(error.to_string());
    Python::with_gil(|py| {
        if let Err(set_err) = err.value(py).setattr("kind", kind) {
            set_err.restore(py);
        }
    });
    err
}

fn convert_account_group_id_error(error: AccountGroupIdError) -> PyErr {
    PyValueError::new_err(error.to_string())
}

type Order = openpit_interop::RequestWithPayload<openpit_interop::Order, Py<PyAny>>;
type ExecutionReport =
    openpit_interop::RequestWithPayload<openpit_interop::ExecutionReport, Py<PyAny>>;
type AccountAdjustment =
    openpit_interop::RequestWithPayload<openpit_interop::AccountAdjustment, Py<PyAny>>;

type PyEngineTrait = openpit_interop::InteropEngineTrait<Order, ExecutionReport, AccountAdjustment>;

#[pyclass(name = "InstrumentId", module = "openpit.marketdata", frozen)]
#[derive(Clone, Copy)]
struct PyInstrumentId {
    inner: InstrumentId,
}

#[pymethods]
impl PyInstrumentId {
    #[new]
    fn new(value: u64) -> Self {
        Self {
            inner: InstrumentId::new(value),
        }
    }

    #[getter]
    fn value(&self) -> u64 {
        self.inner.as_u64()
    }

    fn __richcmp__(&self, other: &Self, op: CompareOp) -> PyResult<bool> {
        match op {
            CompareOp::Eq => Ok(self.inner == other.inner),
            CompareOp::Ne => Ok(self.inner != other.inner),
            _ => Err(PyTypeError::new_err("InstrumentId supports only == and !=")),
        }
    }

    fn __hash__(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.inner.hash(&mut hasher);
        hasher.finish()
    }

    fn __repr__(&self) -> String {
        format!("InstrumentId({})", self.inner.as_u64())
    }
}

#[pyclass(name = "QuoteTtl", module = "openpit.marketdata", frozen)]
#[derive(Clone, Copy)]
struct PyQuoteTtl {
    inner: QuoteTtl,
}

#[pymethods]
impl PyQuoteTtl {
    #[staticmethod]
    fn infinite() -> Self {
        Self {
            inner: QuoteTtl::Infinite,
        }
    }

    #[staticmethod]
    fn within(duration: Duration) -> Self {
        Self {
            inner: QuoteTtl::Within(duration),
        }
    }

    fn __repr__(&self) -> String {
        match self.inner {
            QuoteTtl::Infinite => "QuoteTtl.infinite()".to_owned(),
            QuoteTtl::Within(duration) => {
                format!("QuoteTtl.within({:?})", duration)
            }
        }
    }
}

/// Resolution mode that controls which quote buckets a read may fall through to.
#[pyclass(
    name = "QuoteResolution",
    module = "openpit.marketdata",
    frozen,
    eq,
    eq_int
)]
#[derive(Clone, Copy, PartialEq, Eq)]
// Variant names mirror `openpit::marketdata::QuoteResolution` so the generated
// Python enum members match the core API; the shared `Account` prefix is
// intentional and cannot be dropped without diverging from the canonical names.
#[allow(clippy::enum_variant_names)]
enum PyQuoteResolution {
    /// Consult only the per-account bucket.
    #[pyo3(name = "ACCOUNT_ONLY")]
    AccountOnly = 0,
    /// Consult the per-account bucket, then the account's group bucket.
    #[pyo3(name = "ACCOUNT_THEN_GROUP")]
    AccountThenGroup = 1,
    /// Consult the per-account bucket, then the group bucket, then the
    /// default ("everyone-else") bucket.
    #[pyo3(name = "ACCOUNT_THEN_GROUP_THEN_DEFAULT")]
    AccountThenGroupThenDefault = 2,
}

impl From<PyQuoteResolution> for QuoteResolution {
    fn from(v: PyQuoteResolution) -> Self {
        match v {
            PyQuoteResolution::AccountOnly => QuoteResolution::AccountOnly,
            PyQuoteResolution::AccountThenGroup => QuoteResolution::AccountThenGroup,
            PyQuoteResolution::AccountThenGroupThenDefault => {
                QuoteResolution::AccountThenGroupThenDefault
            }
        }
    }
}

#[pyclass(name = "Quote", module = "openpit.marketdata", frozen)]
#[derive(Clone, Copy)]
struct PyQuote {
    inner: Quote,
}

#[pymethods]
impl PyQuote {
    #[new]
    #[pyo3(signature = (*, mark = None, bid = None, ask = None))]
    fn new(
        mark: Option<&Bound<'_, PyAny>>,
        bid: Option<&Bound<'_, PyAny>>,
        ask: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let mut quote = Quote::new();
        if let Some(mark) = mark.map(parse_price_input).transpose()? {
            quote = quote.with_mark(mark);
        }
        if let Some(bid) = bid.map(parse_price_input).transpose()? {
            quote = quote.with_bid(bid);
        }
        if let Some(ask) = ask.map(parse_price_input).transpose()? {
            quote = quote.with_ask(ask);
        }
        Ok(Self { inner: quote })
    }

    #[getter]
    fn mark(&self) -> Option<PyPrice> {
        self.inner.mark.map(|inner| PyPrice { inner })
    }

    #[getter]
    fn bid(&self) -> Option<PyPrice> {
        self.inner.bid.map(|inner| PyPrice { inner })
    }

    #[getter]
    fn ask(&self) -> Option<PyPrice> {
        self.inner.ask.map(|inner| PyPrice { inner })
    }

    fn __richcmp__(&self, other: &Self, op: CompareOp) -> PyResult<bool> {
        match op {
            CompareOp::Eq => Ok(self.inner == other.inner),
            CompareOp::Ne => Ok(self.inner != other.inner),
            _ => Err(PyTypeError::new_err("Quote supports only == and !=")),
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "Quote(mark={:?}, bid={:?}, ask={:?})",
            self.inner.mark, self.inner.bid, self.inner.ask
        )
    }
}

/// Adapter that satisfies `openpit::marketdata::AccountInfo` for an arbitrary
/// Python object that exposes an `account_group` attribute.
///
/// `group()` is called lazily by the core service only when the per-account
/// bucket misses and the resolution/TTL-cascade needs the group.  Because the
/// call reads back into Python, the GIL must be held for the entire duration of
/// `MarketDataService::get` / `get_or_err` — callers must NOT use
/// `py.allow_threads` around those calls.
///
/// Errors (attribute missing, wrong type, etc.) are stored in the thread-local
/// `PY_CALLBACK_ERROR` slot and `group()` returns `None` so the core can
/// complete; the caller drains the slot immediately after `get`/`get_or_err`
/// returns and surfaces the stored error as a `PyResult`.
struct PyAccountInfo<'py> {
    obj: &'py Bound<'py, PyAny>,
}

impl openpit::marketdata::AccountInfo for PyAccountInfo<'_> {
    fn group(&self) -> Option<openpit::param::AccountGroupId> {
        let attr = match self.obj.getattr("account_group") {
            Ok(v) => v,
            Err(err) => {
                set_python_callback_error(err);
                return None;
            }
        };
        if attr.is_none() {
            return None;
        }
        match attr.extract::<PyRef<'_, PyAccountGroupId>>() {
            Ok(g) => Some(g.inner),
            Err(err) => {
                set_python_callback_error(PyTypeError::new_err(format!(
                    "account_info.account_group must be AccountGroupId or None, got: {err}"
                )));
                None
            }
        }
    }
}

#[pyclass(name = "MarketDataService", module = "openpit.marketdata")]
#[derive(Clone)]
struct PyMarketDataService {
    inner: EngineHandle<MarketDataService<EngineLocking>>,
    mode: SyncMode,
}

#[pymethods]
impl PyMarketDataService {
    fn register(&self, py: Python<'_>, instrument: &Bound<'_, PyAny>) -> PyResult<PyInstrumentId> {
        let instrument = parse_instrument_input(instrument)?;
        py.allow_threads(|| self.inner.register(instrument))
            .map(|inner| PyInstrumentId { inner })
            .map_err(create_already_registered_error)
    }

    fn register_with_ttl(
        &self,
        py: Python<'_>,
        instrument: &Bound<'_, PyAny>,
        ttl: &PyQuoteTtl,
    ) -> PyResult<PyInstrumentId> {
        let instrument = parse_instrument_input(instrument)?;
        py.allow_threads(|| self.inner.register_with_ttl(instrument, ttl.inner))
            .map(|inner| PyInstrumentId { inner })
            .map_err(create_already_registered_error)
    }

    fn register_with_id(
        &self,
        py: Python<'_>,
        instrument: &Bound<'_, PyAny>,
        id: &PyInstrumentId,
    ) -> PyResult<PyInstrumentId> {
        let instrument = parse_instrument_input(instrument)?;
        py.allow_threads(|| self.inner.register_with_id(instrument, id.inner))
            .map(|inner| PyInstrumentId { inner })
            .map_err(create_registration_error)
    }

    fn register_with_id_and_ttl(
        &self,
        py: Python<'_>,
        instrument: &Bound<'_, PyAny>,
        id: &PyInstrumentId,
        ttl: &PyQuoteTtl,
    ) -> PyResult<PyInstrumentId> {
        let instrument = parse_instrument_input(instrument)?;
        py.allow_threads(|| {
            self.inner
                .register_with_id_and_ttl(instrument, id.inner, ttl.inner)
        })
        .map(|inner| PyInstrumentId { inner })
        .map_err(create_registration_error)
    }

    // ── TTL setters / clearers ────────────────────────────────────────────────

    fn set_account_ttl(
        &self,
        py: Python<'_>,
        account_id: &Bound<'_, PyAny>,
        ttl: &PyQuoteTtl,
    ) -> PyResult<()> {
        let account_id = parse_account_id_input(account_id)?;
        py.allow_threads(|| self.inner.set_account_ttl(account_id, ttl.inner));
        Ok(())
    }

    fn clear_account_ttl(&self, py: Python<'_>, account_id: &Bound<'_, PyAny>) -> PyResult<()> {
        let account_id = parse_account_id_input(account_id)?;
        py.allow_threads(|| self.inner.clear_account_ttl(account_id));
        Ok(())
    }

    /// Sets the TTL override for the given account group.
    ///
    /// Passing `AccountGroupId.DEFAULT` targets the service-level default-group
    /// TTL (the "everyone-else" bucket that accounts without an explicit group
    /// assignment fall into).
    fn set_account_group_ttl(
        &self,
        py: Python<'_>,
        account_group_id: &Bound<'_, PyAny>,
        ttl: &PyQuoteTtl,
    ) -> PyResult<()> {
        let account_group_id = parse_account_group_id_input(account_group_id)?;
        py.allow_threads(|| {
            self.inner
                .set_account_group_ttl(account_group_id, ttl.inner)
        });
        Ok(())
    }

    /// Clears the TTL override for the given account group.
    ///
    /// Passing `AccountGroupId.DEFAULT` targets the service-level default-group
    /// TTL (the "everyone-else" bucket that accounts without an explicit group
    /// assignment fall into).
    fn clear_account_group_ttl(
        &self,
        py: Python<'_>,
        account_group_id: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let account_group_id = parse_account_group_id_input(account_group_id)?;
        py.allow_threads(|| self.inner.clear_account_group_ttl(account_group_id));
        Ok(())
    }

    fn set_instrument_ttl(
        &self,
        py: Python<'_>,
        instrument_id: &PyInstrumentId,
        ttl: &PyQuoteTtl,
    ) -> PyResult<()> {
        py.allow_threads(|| {
            self.inner
                .set_instrument_ttl(instrument_id.inner, ttl.inner)
        })
        .map_err(create_unknown_instrument_id_error)
    }

    fn clear_instrument_ttl(&self, py: Python<'_>, instrument_id: &PyInstrumentId) -> PyResult<()> {
        py.allow_threads(|| self.inner.clear_instrument_ttl(instrument_id.inner))
            .map_err(create_unknown_instrument_id_error)
    }

    fn set_instrument_account_ttl(
        &self,
        py: Python<'_>,
        instrument_id: &PyInstrumentId,
        account_id: &Bound<'_, PyAny>,
        ttl: &PyQuoteTtl,
    ) -> PyResult<()> {
        let account_id = parse_account_id_input(account_id)?;
        py.allow_threads(|| {
            self.inner
                .set_instrument_account_ttl(instrument_id.inner, account_id, ttl.inner)
        })
        .map_err(create_unknown_instrument_id_error)
    }

    fn clear_instrument_account_ttl(
        &self,
        py: Python<'_>,
        instrument_id: &PyInstrumentId,
        account_id: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let account_id = parse_account_id_input(account_id)?;
        py.allow_threads(|| {
            self.inner
                .clear_instrument_account_ttl(instrument_id.inner, account_id)
        })
        .map_err(create_unknown_instrument_id_error)
    }

    /// Sets the per-instrument TTL override for the given account group.
    ///
    /// Passing `AccountGroupId.DEFAULT` targets the instrument-level
    /// default-group TTL (the "everyone-else" bucket for this instrument).
    fn set_instrument_account_group_ttl(
        &self,
        py: Python<'_>,
        instrument_id: &PyInstrumentId,
        account_group_id: &Bound<'_, PyAny>,
        ttl: &PyQuoteTtl,
    ) -> PyResult<()> {
        let account_group_id = parse_account_group_id_input(account_group_id)?;
        py.allow_threads(|| {
            self.inner.set_instrument_account_group_ttl(
                instrument_id.inner,
                account_group_id,
                ttl.inner,
            )
        })
        .map_err(create_unknown_instrument_id_error)
    }

    /// Clears the per-instrument TTL override for the given account group.
    ///
    /// Passing `AccountGroupId.DEFAULT` targets the instrument-level
    /// default-group TTL (the "everyone-else" bucket for this instrument).
    fn clear_instrument_account_group_ttl(
        &self,
        py: Python<'_>,
        instrument_id: &PyInstrumentId,
        account_group_id: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let account_group_id = parse_account_group_id_input(account_group_id)?;
        py.allow_threads(|| {
            self.inner
                .clear_instrument_account_group_ttl(instrument_id.inner, account_group_id)
        })
        .map_err(create_unknown_instrument_id_error)
    }

    // ── Clear ─────────────────────────────────────────────────────────────────

    fn clear(&self, py: Python<'_>, instrument_id: &PyInstrumentId) {
        py.allow_threads(|| self.inner.clear(instrument_id.inner));
    }

    // ── Push (default bucket) ─────────────────────────────────────────────────

    fn push(
        &self,
        py: Python<'_>,
        instrument_id: &PyInstrumentId,
        quote: &PyQuote,
    ) -> PyResult<()> {
        py.allow_threads(|| self.inner.push(instrument_id.inner, quote.inner))
            .map_err(create_unknown_instrument_id_error)
    }

    fn push_patch(
        &self,
        py: Python<'_>,
        instrument_id: &PyInstrumentId,
        quote: &PyQuote,
    ) -> PyResult<()> {
        py.allow_threads(|| self.inner.push_patch(instrument_id.inner, quote.inner))
            .map_err(create_unknown_instrument_id_error)
    }

    fn push_by_instrument(
        &self,
        py: Python<'_>,
        instrument: &Bound<'_, PyAny>,
        quote: &PyQuote,
    ) -> PyResult<PyInstrumentId> {
        let instrument = parse_instrument_input(instrument)?;
        let inner = py.allow_threads(|| self.inner.push_by_instrument(&instrument, quote.inner));
        Ok(PyInstrumentId { inner })
    }

    fn push_by_instrument_patch(
        &self,
        py: Python<'_>,
        instrument: &Bound<'_, PyAny>,
        quote: &PyQuote,
    ) -> PyResult<PyInstrumentId> {
        let instrument = parse_instrument_input(instrument)?;
        let inner = py.allow_threads(|| {
            self.inner
                .push_by_instrument_patch(&instrument, quote.inner)
        });
        Ok(PyInstrumentId { inner })
    }

    // ── Targeted fan-out push ─────────────────────────────────────────────────

    /// Push a full quote snapshot to specific accounts and/or groups.
    ///
    /// To target the default ("everyone-else") bucket, include
    /// ``AccountGroupId.DEFAULT`` in ``account_group_ids``.
    fn push_for(
        &self,
        py: Python<'_>,
        instrument_id: &PyInstrumentId,
        quote: &PyQuote,
        account_ids: &Bound<'_, PyAny>,
        account_group_ids: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let account_ids = account_ids
            .try_iter()?
            .map(|item| parse_account_id_input(&item?))
            .collect::<PyResult<Vec<_>>>()?;
        let group_ids = account_group_ids
            .try_iter()?
            .map(|item| parse_account_group_id_input(&item?))
            .collect::<PyResult<Vec<_>>>()?;
        py.allow_threads(|| {
            self.inner
                .push_for(instrument_id.inner, quote.inner, &account_ids, &group_ids)
        })
        .map_err(create_push_for_error)
    }

    /// Push a partial quote patch to specific accounts and/or groups.
    ///
    /// To target the default ("everyone-else") bucket, include
    /// ``AccountGroupId.DEFAULT`` in ``account_group_ids``.
    fn push_for_patch(
        &self,
        py: Python<'_>,
        instrument_id: &PyInstrumentId,
        quote: &PyQuote,
        account_ids: &Bound<'_, PyAny>,
        account_group_ids: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let account_ids = account_ids
            .try_iter()?
            .map(|item| parse_account_id_input(&item?))
            .collect::<PyResult<Vec<_>>>()?;
        let group_ids = account_group_ids
            .try_iter()?
            .map(|item| parse_account_group_id_input(&item?))
            .collect::<PyResult<Vec<_>>>()?;
        py.allow_threads(|| {
            self.inner
                .push_for_patch(instrument_id.inner, quote.inner, &account_ids, &group_ids)
        })
        .map_err(create_push_for_error)
    }

    // ── Get ───────────────────────────────────────────────────────────────────

    #[pyo3(signature = (instrument_id, account_id, account_info, resolution))]
    fn get(
        &self,
        _py: Python<'_>,
        instrument_id: &PyInstrumentId,
        account_id: &Bound<'_, PyAny>,
        account_info: &Bound<'_, PyAny>,
        resolution: PyQuoteResolution,
    ) -> PyResult<Option<PyQuote>> {
        let account_id = parse_account_id_input(account_id)?;
        // GIL is held throughout so the adapter can read Python attributes lazily.
        clear_python_callback_error();
        let adapter = PyAccountInfo { obj: account_info };
        let result = self
            .inner
            .get(instrument_id.inner, account_id, &adapter, resolution.into());
        if let Some(err) = take_python_callback_error() {
            return Err(err);
        }
        Ok(result.map(|inner| PyQuote { inner }))
    }

    #[pyo3(signature = (instrument_id, account_id, account_info, resolution))]
    fn get_or_err(
        &self,
        _py: Python<'_>,
        instrument_id: &PyInstrumentId,
        account_id: &Bound<'_, PyAny>,
        account_info: &Bound<'_, PyAny>,
        resolution: PyQuoteResolution,
    ) -> PyResult<PyQuote> {
        let account_id = parse_account_id_input(account_id)?;
        // GIL is held throughout so the adapter can read Python attributes lazily.
        clear_python_callback_error();
        let adapter = PyAccountInfo { obj: account_info };
        let result =
            self.inner
                .get_or_err(instrument_id.inner, account_id, &adapter, resolution.into());
        if let Some(err) = take_python_callback_error() {
            return Err(err);
        }
        result
            .map(|inner| PyQuote { inner })
            .map_err(create_market_data_error)
    }

    // ── Resolve ───────────────────────────────────────────────────────────────

    fn resolve(
        &self,
        py: Python<'_>,
        instrument: &Bound<'_, PyAny>,
    ) -> PyResult<Option<PyInstrumentId>> {
        let instrument = parse_instrument_input(instrument)?;
        Ok(py
            .allow_threads(|| self.inner.resolve(&instrument))
            .map(|inner| PyInstrumentId { inner }))
    }
}

#[pyclass(name = "MarketDataBuilder", module = "openpit.marketdata")]
#[derive(Clone, Copy)]
struct PyMarketDataBuilder {
    default_ttl: QuoteTtl,
    mode: SyncMode,
}

#[pymethods]
impl PyMarketDataBuilder {
    /// Downgrades the builder to no-sync mode (no-op locks, zero overhead).
    ///
    /// Use only when the market-data service is written from a single thread
    /// and never read concurrently. No-op when the builder is already None.
    fn no_sync(mut slf: PyRefMut<'_, Self>) -> PyRefMut<'_, Self> {
        slf.mode = SyncMode::None;
        slf
    }

    /// Upgrades the builder to use full synchronization.
    ///
    /// If the builder was derived from a `no_sync()` engine (no-sync mode),
    /// this switches it to Full so the resulting service is safe for a
    /// concurrent quote feed. No-op when the builder is already Full.
    fn full_sync(mut slf: PyRefMut<'_, Self>) -> PyRefMut<'_, Self> {
        if slf.mode == SyncMode::None {
            slf.mode = SyncMode::Full;
        }
        slf
    }

    fn build(&self) -> PyMarketDataService {
        let handle =
            MarketDataBuilder::with_sync(EngineLocking::new(self.mode), self.default_ttl).build();
        PyMarketDataService {
            inner: handle,
            mode: self.mode,
        }
    }
}

#[pyclass(name = "Engine", module = "openpit")]
struct PyEngine {
    inner: Engine<PyEngineTrait>,
}

#[pymethods]
impl PyEngine {
    #[staticmethod]
    fn builder() -> PyEngineBuilder {
        PyEngineBuilder
    }

    #[pyo3(signature = (order))]
    fn start_pre_trade(
        &self,
        py: Python<'_>,
        order: Bound<'_, PyAny>,
    ) -> PyResult<PyStartPreTradeResult> {
        clear_python_callback_error();
        let order = extract_python_order(&order)?;
        match allow_threads_detached(py, || self.inner.start_pre_trade(order)) {
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
        match allow_threads_detached(py, || self.inner.execute_pre_trade(order)) {
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
    fn apply_execution_report(
        &self,
        py: Python<'_>,
        report: &Bound<'_, PyAny>,
    ) -> PyResult<PyPostTradeResult> {
        clear_python_callback_error();
        let report = extract_python_execution_report(report)?;
        let result = PyPostTradeResult {
            inner: py.allow_threads(|| self.inner.apply_execution_report(&report)),
        };
        if let Some(error) = take_python_callback_error() {
            return Err(error);
        }
        Ok(result)
    }

    #[pyo3(signature = (account_id, adjustments))]
    fn apply_account_adjustment(
        &self,
        py: Python<'_>,
        account_id: &Bound<'_, PyAny>,
        adjustments: &Bound<'_, PyAny>,
    ) -> PyResult<PyAccountAdjustmentBatchResult> {
        clear_python_callback_error();

        let account_id = parse_account_id_input(account_id)?;
        let batch = adjustments
            .try_iter()?
            .map(|item| extract_python_account_adjustment(&item?))
            .collect::<PyResult<Vec<_>>>()?;

        match py.allow_threads(|| self.inner.apply_account_adjustment(account_id, &batch)) {
            Ok(result) => {
                if let Some(error) = take_python_callback_error() {
                    return Err(error);
                }
                Ok(PyAccountAdjustmentBatchResult {
                    failed_index: None,
                    rejects: Vec::new(),
                    outcomes: result
                        .outcomes
                        .iter()
                        .map(convert_adjustment_outcome)
                        .collect(),
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
                    outcomes: Vec::new(),
                })
            }
        }
    }

    fn accounts(&self) -> PyAccounts {
        PyAccounts {
            inner: self.inner.accounts(),
        }
    }

    fn configure(&self) -> PyConfigurator {
        PyConfigurator {
            inner: self.inner.configure(),
        }
    }
}

/// Handle to the engine's runtime policy settings registry.
///
/// Obtained from ``Engine.configure()``. The handle shares the engine's single
/// settings registry, so changes made through it are visible to running
/// policies immediately. It inherits the engine's synchronization mode.
#[pyclass(name = "Configurator", module = "openpit")]
struct PyConfigurator {
    inner: Configurator<PyEngineSync>,
}

#[pymethods]
impl PyConfigurator {
    /// Retune a registered rate-limit policy at runtime.
    ///
    /// ``name`` must match the name given to the policy at registration time.
    /// Policy settings use the named entities from
    /// ``openpit.pretrade.policies``.
    ///
    /// Axes passed as ``None`` are left unchanged.  A supplied list REPLACES
    /// that axis wholesale: an empty list clears it (subject to the
    /// at-least-one-barrier rule enforced by the core).  Barriers may be added
    /// and removed at runtime; a barrier key that survives a replacement keeps
    /// its live counter (no reset).  ``broker`` replaces the broker barrier
    /// when provided and leaves it unchanged when ``None``.
    #[pyo3(signature = (name, *, broker = None, asset_barriers = None, account_barriers = None, account_asset_barriers = None))]
    fn rate_limit(
        &self,
        py: Python<'_>,
        name: &str,
        broker: Option<Bound<'_, PyAny>>,
        asset_barriers: Option<Vec<Bound<'_, PyAny>>>,
        account_barriers: Option<Vec<Bound<'_, PyAny>>>,
        account_asset_barriers: Option<Vec<Bound<'_, PyAny>>>,
    ) -> PyResult<()> {
        let broker_barrier = broker
            .as_ref()
            .map(parse_rate_limit_broker_barrier)
            .transpose()?;
        let assets: Option<Vec<RateLimitAssetBarrier>> = asset_barriers
            .map(|v| {
                v.iter()
                    .map(parse_rate_limit_asset_barrier)
                    .collect::<PyResult<Vec<_>>>()
            })
            .transpose()?;
        let accounts: Option<Vec<RateLimitAccountBarrier>> = account_barriers
            .map(|v| {
                v.iter()
                    .map(parse_rate_limit_account_barrier)
                    .collect::<PyResult<Vec<_>>>()
            })
            .transpose()?;
        let account_assets: Option<Vec<RateLimitAccountAssetBarrier>> = account_asset_barriers
            .map(|v| {
                v.iter()
                    .map(parse_rate_limit_account_asset_barrier)
                    .collect::<PyResult<Vec<_>>>()
            })
            .transpose()?;

        py.allow_threads(|| {
            self.inner.rate_limit(name, |s| {
                if let Some(b) = &broker_barrier {
                    s.set_broker(Some(b.clone()))?;
                }
                if let Some(v) = &assets {
                    s.set_asset_barriers(v.iter().cloned())?;
                }
                if let Some(v) = &accounts {
                    s.set_account_barriers(v.iter().cloned())?;
                }
                if let Some(v) = &account_assets {
                    s.set_account_asset_barriers(v.iter().cloned())?;
                }
                Ok::<_, openpit::pretrade::policies::RateLimitPolicyError>(())
            })
        })
        .map_err(convert_configure_error)
    }

    /// Retune a registered P&L bounds kill-switch policy at runtime.
    ///
    /// ``name`` must match the name given to the policy at registration time.
    /// ``account_barriers`` accepts only
    /// ``PnlBoundsAccountAssetBarrierUpdate`` entities. Runtime updates preserve
    /// live accumulated P&L and cannot provide construction-only ``initial_pnl``.
    ///
    /// An axis passed as ``None`` is left unchanged; an empty list replaces the
    /// axis with an empty set (subject to the policy's at-least-one-barrier
    /// rule).
    #[pyo3(signature = (name, *, broker_barriers = None, account_barriers = None))]
    fn pnl_bounds_killswitch(
        &self,
        py: Python<'_>,
        name: &str,
        broker_barriers: Option<Vec<Bound<'_, PyAny>>>,
        account_barriers: Option<Vec<Bound<'_, PyAny>>>,
    ) -> PyResult<()> {
        let brokers: Option<Vec<PnlBoundsBrokerBarrier>> = broker_barriers
            .map(|v| {
                v.iter()
                    .map(parse_pnl_broker_barrier)
                    .collect::<PyResult<_>>()
            })
            .transpose()?;
        let accounts: Option<Vec<PnlBoundsAccountAssetBarrierUpdate>> = account_barriers
            .map(|v| {
                v.iter()
                    .map(parse_pnl_account_barrier_update)
                    .collect::<PyResult<_>>()
            })
            .transpose()?;
        py.allow_threads(|| {
            self.inner.pnl_bounds_killswitch(name, |s| {
                if let Some(b) = &brokers {
                    s.set_broker_barriers(b.iter().cloned())?;
                }
                if let Some(a) = &accounts {
                    s.set_account_barriers(a.iter().cloned())?;
                }
                Ok::<_, openpit::pretrade::policies::PnlBoundsKillSwitchPolicyError>(())
            })
        })
        .map_err(convert_configure_error)
    }

    /// Force-set the live accumulated P&L for a ``(account, settlement_asset)``
    /// entry of a registered P&L bounds kill-switch policy.
    ///
    /// ``name`` must match the name given to the policy at registration time.
    /// Unlike :meth:`pnl_bounds_killswitch`, which retunes bounds and never
    /// touches accumulated P&L, this is an absolute assignment (upsert) of the
    /// live accumulator; the new value is evaluated against the live bounds on
    /// the next order. Forcing the accumulator past a bound trips the kill
    /// switch, which latches an engine-level account block that this call does
    /// not clear.
    #[pyo3(signature = (name, *, account, settlement_asset, pnl))]
    fn set_account_pnl(
        &self,
        py: Python<'_>,
        name: &str,
        account: &Bound<'_, PyAny>,
        settlement_asset: &Bound<'_, PyAny>,
        pnl: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let account = parse_account_id_input(account)?;
        let settlement_asset = parse_asset_input(settlement_asset)?;
        let pnl = parse_pnl_input(pnl)?;
        py.allow_threads(|| {
            self.inner
                .set_account_pnl(name, account, settlement_asset, pnl)
        })
        .map_err(convert_configure_error)
    }

    /// Retune a registered order-size-limit policy at runtime.
    ///
    /// ``name`` must match the name given to the policy at registration time.
    /// The barrier arguments mirror those of
    /// :meth:`~ReadyEngineBuilder._add_builtin_order_size_limit`.
    ///
    /// ``broker=None`` and axis arguments passed as ``None`` are left
    /// unchanged; an empty list replaces that axis with an empty set (subject
    /// to the at-least-one-barrier rule).
    #[pyo3(signature = (name, *, broker = None, asset_barriers = None, account_asset_barriers = None))]
    fn order_size_limit<'py>(
        &self,
        py: Python<'py>,
        name: &str,
        broker: Option<Bound<'py, PyAny>>,
        asset_barriers: Option<Vec<Bound<'py, PyAny>>>,
        account_asset_barriers: Option<Vec<Bound<'py, PyAny>>>,
    ) -> PyResult<()> {
        let broker_barrier = broker
            .as_ref()
            .map(parse_order_size_broker_barrier)
            .transpose()?;
        let asset: Option<Vec<OrderSizeAssetBarrier>> = asset_barriers
            .map(|v| {
                v.iter()
                    .map(parse_order_size_asset_barrier)
                    .collect::<PyResult<Vec<_>>>()
            })
            .transpose()?;
        let account_asset: Option<Vec<OrderSizeAccountAssetBarrier>> = account_asset_barriers
            .map(|v| {
                v.iter()
                    .map(parse_order_size_account_asset_barrier)
                    .collect::<PyResult<Vec<_>>>()
            })
            .transpose()?;

        py.allow_threads(|| {
            self.inner.order_size_limit(name, |s| {
                if let Some(b) = &broker_barrier {
                    s.set_broker(Some(b.clone()))?;
                }
                if let Some(a) = &asset {
                    s.set_asset_barriers(a.clone())?;
                }
                if let Some(aa) = &account_asset {
                    s.set_account_asset_barriers(aa.clone())?;
                }
                Ok::<_, openpit::pretrade::policies::OrderSizeLimitPolicyError>(())
            })
        })
        .map_err(convert_configure_error)
    }

    /// Retune a registered spot-funds policy at runtime.
    ///
    /// ``name`` must match the name given to the policy at registration time.
    /// ``global_slippage_bps`` replaces the global slippage when provided.
    /// ``pricing_source`` and ``overrides`` use the named entities from
    /// ``openpit.pretrade.policies``.
    #[pyo3(signature = (name, *, global_slippage_bps = None, pricing_source = None, overrides = vec![]))]
    fn spot_funds(
        &self,
        py: Python<'_>,
        name: &str,
        global_slippage_bps: Option<u16>,
        pricing_source: Option<Bound<'_, PyAny>>,
        overrides: Vec<Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let parsed_pricing_source = pricing_source
            .as_ref()
            .map(parse_spot_funds_pricing_source_entity)
            .transpose()?;
        let parsed_overrides: Vec<(SpotFundsOverrideTarget, SpotFundsOverride)> = overrides
            .iter()
            .map(parse_spot_funds_override_entity)
            .collect::<PyResult<_>>()?;

        py.allow_threads(|| {
            self.inner.spot_funds(name, |s| {
                if let Some(bps) = global_slippage_bps {
                    s.set_global_slippage_bps(bps)?;
                }
                if let Some(src) = parsed_pricing_source {
                    s.set_pricing_source(src);
                }
                for (target, ovr) in &parsed_overrides {
                    s.set_override(*target, *ovr)?;
                }
                Ok::<_, openpit::pretrade::policies::SpotFundsConfigError>(())
            })
        })
        .map_err(convert_configure_error)
    }
}

/// Handle to the engine's account-group registry and pre-trade block controls.
///
/// Obtained from ``Engine.accounts()``. The handle shares the engine's single
/// account control state, so changes made through it are visible to every other
/// handle and to running policies. It inherits the engine's synchronization
/// mode.
#[pyclass(name = "Accounts", module = "openpit")]
struct PyAccounts {
    inner: Accounts<PyStorageFactory>,
}

#[pymethods]
impl PyAccounts {
    #[pyo3(signature = (accounts, group))]
    fn register_group(
        &self,
        py: Python<'_>,
        accounts: &Bound<'_, PyAny>,
        group: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let group = parse_account_group_id_input(group)?;
        let account_ids = accounts
            .try_iter()?
            .map(|item| parse_account_id_input(&item?))
            .collect::<PyResult<Vec<_>>>()?;
        py.allow_threads(|| self.inner.register_group(&account_ids, group))
            .map_err(convert_account_group_error)
    }

    #[pyo3(signature = (accounts, group))]
    fn unregister_group(
        &self,
        py: Python<'_>,
        accounts: &Bound<'_, PyAny>,
        group: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let group = parse_account_group_id_input(group)?;
        let account_ids = accounts
            .try_iter()?
            .map(|item| parse_account_id_input(&item?))
            .collect::<PyResult<Vec<_>>>()?;
        py.allow_threads(|| self.inner.unregister_group(&account_ids, group))
            .map_err(convert_account_group_error)
    }

    #[pyo3(signature = (account))]
    fn group_of(
        &self,
        py: Python<'_>,
        account: &Bound<'_, PyAny>,
    ) -> PyResult<Option<Py<PyAccountGroupId>>> {
        let account_id = parse_account_id_input(account)?;
        let group = py.allow_threads(|| self.inner.group_of(account_id));
        match group {
            Some(g) => Py::new(py, PyAccountGroupId { inner: g }).map(Some),
            None => Ok(None),
        }
    }

    #[pyo3(signature = (account, reason))]
    fn block(&self, py: Python<'_>, account: &Bound<'_, PyAny>, reason: String) -> PyResult<()> {
        let account_id = parse_account_id_input(account)?;
        py.allow_threads(|| self.inner.block(account_id, reason));
        Ok(())
    }

    #[pyo3(signature = (account))]
    fn unblock(&self, py: Python<'_>, account: &Bound<'_, PyAny>) -> PyResult<()> {
        let account_id = parse_account_id_input(account)?;
        py.allow_threads(|| self.inner.unblock(account_id));
        Ok(())
    }

    #[pyo3(signature = (account, reason))]
    fn replace_block_reason(
        &self,
        py: Python<'_>,
        account: &Bound<'_, PyAny>,
        reason: String,
    ) -> PyResult<()> {
        let account_id = parse_account_id_input(account)?;
        py.allow_threads(|| self.inner.replace_block_reason(account_id, reason))
            .map_err(convert_account_block_error)
    }

    #[pyo3(signature = (group, reason))]
    fn block_group(
        &self,
        py: Python<'_>,
        group: &Bound<'_, PyAny>,
        reason: String,
    ) -> PyResult<()> {
        let group_id = parse_account_group_id_input(group)?;
        py.allow_threads(|| self.inner.block_group(group_id, reason))
            .map_err(convert_account_block_error)
    }

    #[pyo3(signature = (group))]
    fn unblock_group(&self, py: Python<'_>, group: &Bound<'_, PyAny>) -> PyResult<()> {
        let group_id = parse_account_group_id_input(group)?;
        py.allow_threads(|| self.inner.unblock_group(group_id))
            .map_err(convert_account_block_error)
    }

    #[pyo3(signature = (group, reason))]
    fn replace_group_block_reason(
        &self,
        py: Python<'_>,
        group: &Bound<'_, PyAny>,
        reason: String,
    ) -> PyResult<()> {
        let group_id = parse_account_group_id_input(group)?;
        py.allow_threads(|| self.inner.replace_group_block_reason(group_id, reason))
            .map_err(convert_account_block_error)
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

#[pyclass(name = "StartResult", module = "openpit.pretrade")]
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
            "StartResult(ok=True)".to_owned()
        } else {
            format!("StartResult(ok=False, rejects={})", self.rejects.len())
        }
    }
}

#[pyclass(name = "ExecuteResult", module = "openpit.pretrade")]
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

#[pyclass(name = "AccountAdjustmentBatchResult", module = "openpit.pretrade")]
struct PyAccountAdjustmentBatchResult {
    failed_index: Option<usize>,
    rejects: Vec<PyReject>,
    outcomes: Vec<PyAccountAdjustmentOutcome>,
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

    #[getter]
    fn outcomes(&self) -> Vec<PyAccountAdjustmentOutcome> {
        self.outcomes.clone()
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

#[pyclass(name = "OutcomeAmount", module = "openpit.pretrade")]
#[derive(Clone)]
struct PyOutcomeAmount {
    delta: PositionSize,
    absolute: PositionSize,
}

#[pymethods]
impl PyOutcomeAmount {
    #[new]
    #[pyo3(signature = (*, delta, absolute))]
    fn new(delta: &Bound<'_, PyAny>, absolute: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            delta: parse_position_size_input(delta)?,
            absolute: parse_position_size_input(absolute)?,
        })
    }

    #[getter]
    fn delta(&self) -> PyPositionSize {
        PyPositionSize { inner: self.delta }
    }

    #[getter]
    fn absolute(&self) -> PyPositionSize {
        PyPositionSize {
            inner: self.absolute,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "OutcomeAmount(delta={}, absolute={})",
            self.delta, self.absolute
        )
    }
}

#[pyclass(name = "AccountOutcomeEntry", module = "openpit.pretrade")]
#[derive(Clone)]
struct PyAccountOutcomeEntry {
    asset: Asset,
    balance: Option<PyOutcomeAmount>,
    held: Option<PyOutcomeAmount>,
    incoming: Option<PyOutcomeAmount>,
}

#[pymethods]
impl PyAccountOutcomeEntry {
    #[new]
    #[pyo3(signature = (*, asset, balance = None, held = None, incoming = None))]
    fn new(
        asset: &Bound<'_, PyAny>,
        balance: Option<PyRef<'_, PyOutcomeAmount>>,
        held: Option<PyRef<'_, PyOutcomeAmount>>,
        incoming: Option<PyRef<'_, PyOutcomeAmount>>,
    ) -> PyResult<Self> {
        Ok(Self {
            asset: parse_asset_input(asset)?,
            balance: balance.map(|value| value.clone()),
            held: held.map(|value| value.clone()),
            incoming: incoming.map(|value| value.clone()),
        })
    }

    #[getter]
    fn asset(&self) -> String {
        self.asset.to_string()
    }

    #[getter]
    fn balance(&self) -> Option<PyOutcomeAmount> {
        self.balance.clone()
    }

    #[getter]
    fn held(&self) -> Option<PyOutcomeAmount> {
        self.held.clone()
    }

    #[getter]
    fn incoming(&self) -> Option<PyOutcomeAmount> {
        self.incoming.clone()
    }

    fn __repr__(&self) -> String {
        format!(
            "AccountOutcomeEntry(asset={:?}, balance={:?}, held={:?}, incoming={:?})",
            self.asset.to_string(),
            self.balance.as_ref().map(|v| v.__repr__()),
            self.held.as_ref().map(|v| v.__repr__()),
            self.incoming.as_ref().map(|v| v.__repr__()),
        )
    }
}

#[pyclass(name = "AccountAdjustmentOutcome", module = "openpit.pretrade")]
#[derive(Clone)]
struct PyAccountAdjustmentOutcome {
    policy_group_id: PolicyGroupId,
    entry: PyAccountOutcomeEntry,
}

#[pymethods]
impl PyAccountAdjustmentOutcome {
    #[new]
    #[pyo3(signature = (*, policy_group_id, entry))]
    fn new(
        policy_group_id: &Bound<'_, PyAny>,
        entry: PyRef<'_, PyAccountOutcomeEntry>,
    ) -> PyResult<Self> {
        Ok(Self {
            policy_group_id: parse_policy_group_id_input(policy_group_id)?,
            entry: entry.clone(),
        })
    }

    #[getter]
    fn policy_group_id(&self) -> u16 {
        self.policy_group_id.value()
    }

    #[getter]
    fn entry(&self) -> PyAccountOutcomeEntry {
        self.entry.clone()
    }

    fn __repr__(&self) -> String {
        format!(
            "AccountAdjustmentOutcome(policy_group_id={}, entry={})",
            self.policy_group_id.value(),
            self.entry.__repr__()
        )
    }
}

/// Handle that records a kill-switch block against the account bound to a
/// callback context.
///
/// It is valid to use only within the pre-trade processing of the request it
/// belongs to — from the callback that produced it through the commit or
/// rollback of that request's reservation (so it may be retained and used from
/// a deferred mutation commit/rollback callback). Recording a block through it
/// after that pre-trade transaction has completed is unspecified and must not
/// be relied upon.
#[pyclass(name = "AccountControl", module = "openpit", frozen)]
struct PyAccountControl {
    inner: openpit::AccountControl<PyStorageFactory>,
}

#[pymethods]
impl PyAccountControl {
    fn block(&self, block: PyRef<'_, PyAccountBlock>) -> PyResult<()> {
        let native = openpit::pretrade::AccountBlock::new(
            block.policy.clone(),
            parse_reject_code(&block.code)?,
            block.reason.clone(),
            block.details.clone(),
        )
        .with_user_data(block.user_data as usize);
        self.inner.block(native);
        Ok(())
    }
}

#[pyclass(name = "Context", module = "openpit.pretrade", frozen)]
struct PyPreTradeContext {
    account_control: Option<openpit::AccountControl<PyStorageFactory>>,
    group: Option<AccountGroupId>,
}

#[pyclass(name = "AccountAdjustmentContext", module = "openpit", frozen)]
struct PyAccountAdjustmentContext {
    account_control: openpit::AccountControl<PyStorageFactory>,
    group: Option<AccountGroupId>,
}

#[pyclass(name = "PostTradeContext", module = "openpit.pretrade", frozen)]
struct PyPostTradeContext {
    group: Option<AccountGroupId>,
}

#[pymethods]
impl PyPreTradeContext {
    #[getter]
    fn account_control(&self, py: Python<'_>) -> PyResult<Option<Py<PyAccountControl>>> {
        self.account_control
            .as_ref()
            .map(|inner| {
                Py::new(
                    py,
                    PyAccountControl {
                        inner: inner.clone(),
                    },
                )
            })
            .transpose()
    }

    #[getter]
    fn account_group(&self, py: Python<'_>) -> PyResult<Option<Py<PyAccountGroupId>>> {
        match self.group {
            Some(g) => Py::new(py, PyAccountGroupId { inner: g }).map(Some),
            None => Ok(None),
        }
    }
}

#[pymethods]
impl PyAccountAdjustmentContext {
    #[getter]
    fn account_control(&self, py: Python<'_>) -> PyResult<Py<PyAccountControl>> {
        Py::new(
            py,
            PyAccountControl {
                inner: self.account_control.clone(),
            },
        )
    }

    #[getter]
    fn account_group(&self, py: Python<'_>) -> PyResult<Option<Py<PyAccountGroupId>>> {
        match self.group {
            Some(g) => Py::new(py, PyAccountGroupId { inner: g }).map(Some),
            None => Ok(None),
        }
    }
}

#[pymethods]
impl PyPostTradeContext {
    #[getter]
    fn account_group(&self, py: Python<'_>) -> PyResult<Option<Py<PyAccountGroupId>>> {
        match self.group {
            Some(g) => Py::new(py, PyAccountGroupId { inner: g }).map(Some),
            None => Ok(None),
        }
    }
}

type PyStorageFactory = openpit_interop::StorageLockingPolicyFactory;
type PyEngineSync = openpit_interop::EngineLocking;

impl From<&PreTradeContext<PyStorageFactory>> for PyPreTradeContext {
    fn from(context: &PreTradeContext<PyStorageFactory>) -> Self {
        Self {
            account_control: context.account_control.clone(),
            group: context.account_group(),
        }
    }
}

impl From<&AccountAdjustmentContext<PyStorageFactory>> for PyAccountAdjustmentContext {
    fn from(context: &AccountAdjustmentContext<PyStorageFactory>) -> Self {
        Self {
            account_control: context.account_control.clone(),
            group: context.account_group(),
        }
    }
}

impl From<&PostTradeContext<PyStorageFactory>> for PyPostTradeContext {
    fn from(context: &PostTradeContext<PyStorageFactory>) -> Self {
        Self {
            group: context.account_group(),
        }
    }
}

struct BoxedPreTradePolicy {
    inner: Box<dyn PreTradePolicy<Order, ExecutionReport, AccountAdjustment, PyEngineSync> + Send>,
}

impl PreTradePolicy<Order, ExecutionReport, AccountAdjustment, PyEngineSync>
    for BoxedPreTradePolicy
{
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn policy_group_id(&self) -> PolicyGroupId {
        self.inner.policy_group_id()
    }

    fn check_pre_trade_start(
        &self,
        ctx: &PreTradeContext<PyStorageFactory>,
        order: &Order,
    ) -> Result<(), Rejects> {
        self.inner.check_pre_trade_start(ctx, order)
    }

    fn perform_pre_trade_check(
        &self,
        ctx: &PreTradeContext<PyStorageFactory>,
        order: &Order,
        mutations: &mut Mutations,
    ) -> Result<Option<PolicyPreTradeResult>, Rejects> {
        self.inner.perform_pre_trade_check(ctx, order, mutations)
    }

    fn apply_execution_report(
        &self,
        ctx: &PostTradeContext<PyStorageFactory>,
        report: &ExecutionReport,
    ) -> Option<PostTradeResult> {
        self.inner.apply_execution_report(ctx, report)
    }

    fn apply_account_adjustment(
        &self,
        ctx: &AccountAdjustmentContext<PyStorageFactory>,
        account_id: AccountId,
        adjustment: &AccountAdjustment,
        mutations: &mut Mutations,
    ) -> Result<Vec<AccountOutcomeEntry>, Rejects> {
        self.inner
            .apply_account_adjustment(ctx, account_id, adjustment, mutations)
    }
}

struct PythonPreTradePolicyAdapter {
    name: String,
    policy_group_id: PolicyGroupId,
    policy: Py<PyAny>,
}

impl PreTradePolicy<Order, ExecutionReport, AccountAdjustment, PyEngineSync>
    for PythonPreTradePolicyAdapter
{
    fn name(&self) -> &str {
        &self.name
    }

    fn policy_group_id(&self) -> PolicyGroupId {
        self.policy_group_id
    }

    fn check_pre_trade_start(
        &self,
        ctx: &PreTradeContext<PyStorageFactory>,
        order: &Order,
    ) -> Result<(), Rejects> {
        Python::with_gil(|py| {
            let policy_ctx = Py::new(py, PyPreTradeContext::from(ctx)).map_err(|error| {
                set_python_callback_error(error);
                python_callback_rejects(&self.name)
            })?;
            let result = self
                .policy
                .bind(py)
                .call_method1(
                    "check_pre_trade_start",
                    (policy_ctx, order.payload.clone_ref(py)),
                )
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

    fn perform_pre_trade_check(
        &self,
        ctx: &PreTradeContext<PyStorageFactory>,
        order: &Order,
        mutations: &mut Mutations,
    ) -> Result<Option<PolicyPreTradeResult>, Rejects> {
        Python::with_gil(|py| {
            let policy_ctx = Py::new(py, PyPreTradeContext::from(ctx)).map_err(|error| {
                set_python_callback_error(error);
                python_callback_rejects(&self.name)
            })?;

            let decision = self
                .policy
                .bind(py)
                .call_method1(
                    "perform_pre_trade_check",
                    (policy_ctx, order.payload.clone_ref(py)),
                )
                .map_err(|error| {
                    set_python_callback_error(error);
                    python_callback_rejects(&self.name)
                })?;

            let mut rejects = Vec::new();
            let result = match apply_policy_pre_trade_result(
                &self.name,
                decision,
                mutations,
                &mut rejects,
            ) {
                Ok(result) => result,
                Err(error) => {
                    set_python_callback_error(error);
                    return Err(python_callback_rejects(&self.name));
                }
            };
            if rejects.is_empty() {
                Ok(result)
            } else {
                Err(Rejects::from(rejects))
            }
        })
    }

    fn apply_execution_report(
        &self,
        ctx: &PostTradeContext<PyStorageFactory>,
        report: &ExecutionReport,
    ) -> Option<PostTradeResult> {
        Python::with_gil(|py| {
            let post_trade_ctx = match Py::new(py, PyPostTradeContext::from(ctx)) {
                Ok(v) => v,
                Err(error) => {
                    set_python_callback_error(error);
                    return None;
                }
            };
            let kwargs = PyDict::new(py);
            if let Err(error) = kwargs.set_item("ctx", post_trade_ctx) {
                set_python_callback_error(error);
                return None;
            }
            if let Err(error) = kwargs.set_item("report", report.payload.clone_ref(py)) {
                set_python_callback_error(error);
                return None;
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
                        return None;
                    }
                };

            if result.is_none() {
                return None;
            }
            if matches!(result.extract::<bool>(), Ok(false)) {
                return None;
            }
            if matches!(result.extract::<bool>(), Ok(true)) {
                set_python_callback_error(PyTypeError::new_err(
                    "apply_execution_report must return PostTradeResult or None; bool True no longer maps to a synthetic account block",
                ));
                return None;
            }

            match parse_post_trade_result(&result) {
                Ok(result) => {
                    if result.is_empty() {
                        None
                    } else {
                        Some(result)
                    }
                }
                Err(error) => {
                    set_python_callback_error(error);
                    None
                }
            }
        })
    }

    fn apply_account_adjustment(
        &self,
        ctx: &AccountAdjustmentContext<PyStorageFactory>,
        account_id: AccountId,
        adjustment: &AccountAdjustment,
        mutations: &mut Mutations,
    ) -> Result<Vec<AccountOutcomeEntry>, Rejects> {
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
                    (
                        adjustment_ctx,
                        py_account_id,
                        adjustment.payload.clone_ref(py),
                    ),
                )
                .map_err(|error| {
                    set_python_callback_error(error);
                    python_callback_rejects(&self.name)
                })?;

            // None -> pass without mutations.
            if result.is_none() {
                return Ok(vec![]);
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
                    Ok(vec![])
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
            if result.hasattr("asset").map_err(|error| {
                set_python_callback_error(error);
                python_callback_rejects(&self.name)
            })? {
                let entry = parse_account_outcome_entry(&result).map_err(|error| {
                    set_python_callback_error(error);
                    python_callback_rejects(&self.name)
                })?;
                return Ok(vec![entry]);
            }

            let mut rejects = Vec::new();
            let mut outcomes = Vec::new();
            let iter = result.try_iter().map_err(|error| {
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
                if item.hasattr("asset").map_err(|error| {
                    set_python_callback_error(error);
                    python_callback_rejects(&self.name)
                })? {
                    let entry = parse_account_outcome_entry(&item).map_err(|error| {
                        set_python_callback_error(error);
                        python_callback_rejects(&self.name)
                    })?;
                    outcomes.push(entry);
                    continue;
                }
                let mutation = parse_policy_mutation(&item).map_err(|error| {
                    set_python_callback_error(error);
                    python_callback_rejects(&self.name)
                })?;
                mutations.push(mutation);
            }
            if rejects.is_empty() {
                Ok(outcomes)
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

    Ok(openpit_interop::RequestWithPayload::new(
        openpit_interop::Order {
            operation,
            position,
            margin,
        },
        obj.clone().unbind(),
    ))
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
                lock: f.lock.clone(),
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

    Ok(openpit_interop::RequestWithPayload::new(
        openpit_interop::ExecutionReport {
            operation,
            financial_impact,
            fill,
            position_impact,
        },
        obj.clone().unbind(),
    ))
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
                    PopulatedAccountAdjustmentOperation::Balance(PopulatedBalanceOperation {
                        asset: operation.asset.clone(),
                        average_entry_price: operation.average_entry_price,
                    })
                }
                PyAccountAdjustmentOperation::Position(py_position_operation) => {
                    let operation = py_position_operation.bind(py).borrow();
                    let instrument =
                        match (&operation.underlying_asset, &operation.settlement_asset) {
                            (Some(underlying_asset), Some(settlement_asset)) => Some(
                                Instrument::new(underlying_asset.clone(), settlement_asset.clone()),
                            ),
                            _ => None,
                        };
                    PopulatedAccountAdjustmentOperation::Position(PopulatedPositionOperation {
                        instrument,
                        collateral_asset: operation.collateral_asset.clone(),
                        average_entry_price: operation.average_entry_price,
                        mode: operation.mode,
                        leverage: operation.leverage,
                    })
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
                balance: value.balance,
                held: value.held,
                incoming: value.incoming,
            })
        }
    };

    let bounds = match adjustment.bounds.as_ref() {
        None => AccountAdjustmentBoundsAccess::Absent,
        Some(py_bounds) => {
            let value = py_bounds.bind(py).borrow();
            AccountAdjustmentBoundsAccess::Populated(openpit::AccountAdjustmentBounds {
                balance_upper: value.balance_upper,
                balance_lower: value.balance_lower,
                held_upper: value.held_upper,
                held_lower: value.held_lower,
                incoming_upper: value.incoming_upper,
                incoming_lower: value.incoming_lower,
            })
        }
    };

    Ok(openpit_interop::RequestWithPayload::new(
        openpit_interop::AccountAdjustment {
            operation,
            amount,
            bounds,
        },
        obj.clone().unbind(),
    ))
}

fn apply_policy_decision(
    policy_name: &str,
    decision: Bound<'_, PyAny>,
    mutations: &mut Mutations,
    rejects: &mut Vec<Reject>,
) -> PyResult<()> {
    let reject_items = decision.getattr("rejects")?;
    for item in reject_items.try_iter()? {
        rejects.push(parse_policy_reject(&item?, policy_name)?);
    }

    let mutation_items = decision.getattr("mutations")?;
    for item in mutation_items.try_iter()? {
        mutations.push(parse_policy_mutation(&item?)?);
    }
    Ok(())
}

fn apply_policy_pre_trade_result(
    policy_name: &str,
    value: Bound<'_, PyAny>,
    mutations: &mut Mutations,
    rejects: &mut Vec<Reject>,
) -> PyResult<Option<PolicyPreTradeResult>> {
    if value.is_none() {
        return Ok(None);
    }

    if value.hasattr("rejects")? || value.hasattr("mutations")? {
        apply_policy_decision(policy_name, value.clone(), mutations, rejects)?;
    }

    let has_account_adjustments = value.hasattr("account_adjustments")?;
    let has_lock_prices = value.hasattr("lock_prices")?;
    if !has_account_adjustments && !has_lock_prices {
        return Ok(None);
    }

    let mut result = PolicyPreTradeResult::empty();
    if has_account_adjustments {
        let items = value.getattr("account_adjustments")?;
        for item in items.try_iter()? {
            result
                .account_adjustments
                .push(parse_account_outcome_entry(&item?)?);
        }
    }
    if has_lock_prices {
        let items = value.getattr("lock_prices")?;
        for item in items.try_iter()? {
            result.lock_prices.push(parse_price_input(&item?)?);
        }
    }
    if result.account_adjustments.is_empty() && result.lock_prices.is_empty() {
        Ok(None)
    } else {
        Ok(Some(result))
    }
}

fn parse_policy_rejects(value: &Bound<'_, PyAny>, policy_name: &str) -> PyResult<Vec<Reject>> {
    if value.is_none() {
        return Ok(Vec::new());
    }

    let mut rejects = Vec::new();
    for item in value.try_iter()? {
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

fn parse_outcome_amount(value: &Bound<'_, PyAny>) -> PyResult<OutcomeAmount> {
    if let Ok(value) = value.extract::<PyRef<'_, PyOutcomeAmount>>() {
        return Ok(OutcomeAmount {
            delta: value.delta,
            absolute: value.absolute,
        });
    }

    let delta = value.getattr("delta")?;
    let absolute = value.getattr("absolute")?;
    Ok(OutcomeAmount {
        delta: parse_position_size_input(&delta)?,
        absolute: parse_position_size_input(&absolute)?,
    })
}

fn parse_optional_outcome_amount(value: &Bound<'_, PyAny>) -> PyResult<Option<OutcomeAmount>> {
    if value.is_none() {
        return Ok(None);
    }
    Ok(Some(parse_outcome_amount(value)?))
}

fn parse_account_outcome_entry(value: &Bound<'_, PyAny>) -> PyResult<AccountOutcomeEntry> {
    if let Ok(value) = value.extract::<PyRef<'_, PyAccountOutcomeEntry>>() {
        return Ok(AccountOutcomeEntry {
            asset: value.asset.clone(),
            balance: value.balance.as_ref().map(|amount| OutcomeAmount {
                delta: amount.delta,
                absolute: amount.absolute,
            }),
            held: value.held.as_ref().map(|amount| OutcomeAmount {
                delta: amount.delta,
                absolute: amount.absolute,
            }),
            incoming: value.incoming.as_ref().map(|amount| OutcomeAmount {
                delta: amount.delta,
                absolute: amount.absolute,
            }),
        });
    }

    Ok(AccountOutcomeEntry {
        asset: parse_asset_input(&value.getattr("asset")?)?,
        balance: parse_optional_outcome_amount(&value.getattr("balance")?)?,
        held: parse_optional_outcome_amount(&value.getattr("held")?)?,
        incoming: parse_optional_outcome_amount(&value.getattr("incoming")?)?,
    })
}

fn parse_account_block(value: &Bound<'_, PyAny>) -> PyResult<openpit::pretrade::AccountBlock> {
    if let Ok(value) = value.extract::<PyRef<'_, PyAccountBlock>>() {
        return Ok(openpit::pretrade::AccountBlock::new(
            value.policy.clone(),
            parse_reject_code(&value.code)?,
            value.reason.clone(),
            value.details.clone(),
        )
        .with_user_data(value.user_data as usize));
    }

    let code = value
        .getattr("code")?
        .extract::<String>()
        .map_err(|_| PyValueError::new_err("account_block.code must be a string"))?;
    let policy = value
        .getattr("policy")?
        .extract::<String>()
        .map_err(|_| PyValueError::new_err("account_block.policy must be a string"))?;
    let reason = value
        .getattr("reason")?
        .extract::<String>()
        .map_err(|_| PyValueError::new_err("account_block.reason must be a string"))?;
    let details = value
        .getattr("details")?
        .extract::<String>()
        .map_err(|_| PyValueError::new_err("account_block.details must be a string"))?;
    let user_data = if value.hasattr("user_data")? {
        value.getattr("user_data")?.extract::<u64>().map_err(|_| {
            PyValueError::new_err("account_block.user_data must be an integer token")
        })?
    } else {
        0
    };
    Ok(
        openpit::pretrade::AccountBlock::new(policy, parse_reject_code(&code)?, reason, details)
            .with_user_data(user_data as usize),
    )
}

fn parse_account_adjustment_outcome(
    value: &Bound<'_, PyAny>,
) -> PyResult<AccountAdjustmentOutcome> {
    if let Ok(value) = value.extract::<PyRef<'_, PyAccountAdjustmentOutcome>>() {
        return Ok(AccountAdjustmentOutcome {
            policy_group_id: value.policy_group_id,
            entry: AccountOutcomeEntry {
                asset: value.entry.asset.clone(),
                balance: value.entry.balance.as_ref().map(|amount| OutcomeAmount {
                    delta: amount.delta,
                    absolute: amount.absolute,
                }),
                held: value.entry.held.as_ref().map(|amount| OutcomeAmount {
                    delta: amount.delta,
                    absolute: amount.absolute,
                }),
                incoming: value.entry.incoming.as_ref().map(|amount| OutcomeAmount {
                    delta: amount.delta,
                    absolute: amount.absolute,
                }),
            },
        });
    }

    Ok(AccountAdjustmentOutcome {
        policy_group_id: parse_policy_group_id_input(&value.getattr("policy_group_id")?)?,
        entry: parse_account_outcome_entry(&value.getattr("entry")?)?,
    })
}

fn parse_account_block_list(
    value: &Bound<'_, PyAny>,
) -> PyResult<Vec<openpit::pretrade::AccountBlock>> {
    let mut blocks = Vec::new();
    for item in value.try_iter()? {
        blocks.push(parse_account_block(&item?)?);
    }
    Ok(blocks)
}

fn parse_account_adjustment_outcome_list(
    value: &Bound<'_, PyAny>,
) -> PyResult<Vec<AccountAdjustmentOutcome>> {
    let mut outcomes = Vec::new();
    for item in value.try_iter()? {
        outcomes.push(parse_account_adjustment_outcome(&item?)?);
    }
    Ok(outcomes)
}

fn parse_post_trade_result(value: &Bound<'_, PyAny>) -> PyResult<PostTradeResult> {
    if let Ok(value) = value.extract::<PyRef<'_, PyPostTradeResult>>() {
        return Ok(value.inner.clone());
    }

    Ok(PostTradeResult {
        account_blocks: parse_account_block_list(&value.getattr("account_blocks")?)?,
        account_adjustments: parse_account_adjustment_outcome_list(
            &value.getattr("account_adjustments")?,
        )?,
    })
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
        code if code == RejectCode::MarkPriceUnavailable.as_str() => {
            Ok(RejectCode::MarkPriceUnavailable)
        }
        code if code == RejectCode::AccountAdjustmentBoundsExceeded.as_str() => {
            Ok(RejectCode::AccountAdjustmentBoundsExceeded)
        }
        code if code == RejectCode::ArithmeticOverflow.as_str() => {
            Ok(RejectCode::ArithmeticOverflow)
        }
        code if code == RejectCode::Custom.as_str() => Ok(RejectCode::Custom),
        code if code == RejectCode::Other.as_str() => Ok(RejectCode::Other),
        _ => Err(PyValueError::new_err(format!(
            "unsupported reject code {value:?}"
        ))),
    }
}

use openpit_interop::SyncMode as PySyncPolicy;

enum PyBuilderState {
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

impl PyBuilderState {
    fn storage_builder(&self) -> &StorageBuilder<openpit_interop::StorageLockingPolicyFactory> {
        match self {
            Self::Synced(builder) => builder.storage_builder(),
            Self::Ready(builder) => builder.storage_builder(),
        }
    }

    fn add_configurable_rate_limit(
        self,
        policy: RateLimitPolicy<openpit_interop::StorageLockingPolicyFactory>,
    ) -> Self {
        PyBuilderState::Ready(match self {
            Self::Synced(builder) => builder.pre_trade(policy),
            Self::Ready(builder) => builder.pre_trade(policy),
        })
    }

    fn add_configurable_pnl_killswitch(
        self,
        policy: PnlBoundsKillSwitchPolicy<openpit_interop::StorageLockingPolicyFactory>,
    ) -> Self {
        PyBuilderState::Ready(match self {
            Self::Synced(builder) => builder.pre_trade(policy),
            Self::Ready(builder) => builder.pre_trade(policy),
        })
    }

    fn add_configurable_order_size_limit(
        self,
        policy: OrderSizeLimitPolicy<openpit_interop::StorageLockingPolicyFactory>,
    ) -> Self {
        PyBuilderState::Ready(match self {
            Self::Synced(builder) => builder.pre_trade(policy),
            Self::Ready(builder) => builder.pre_trade(policy),
        })
    }

    fn add_configurable_spot_funds(
        self,
        policy: SpotFundsPolicy<openpit_interop::EngineLocking, openpit_interop::EngineLocking>,
    ) -> Self {
        PyBuilderState::Ready(match self {
            Self::Synced(builder) => builder.pre_trade(policy),
            Self::Ready(builder) => builder.pre_trade(policy),
        })
    }
}

#[pyclass(name = "EngineBuilder", module = "openpit")]
struct PyEngineBuilder;

#[pymethods]
impl PyEngineBuilder {
    fn full_sync(&self) -> PySyncedEngineBuilder {
        PySyncedEngineBuilder::synced(PySyncPolicy::Full)
    }

    fn no_sync(&self) -> PySyncedEngineBuilder {
        PySyncedEngineBuilder::synced(PySyncPolicy::None)
    }

    fn account_sync(&self) -> PySyncedEngineBuilder {
        PySyncedEngineBuilder::synced(PySyncPolicy::Account)
    }
}

#[pyclass(name = "SyncedEngineBuilder", module = "openpit")]
struct PySyncedEngineBuilder {
    sync_policy: PySyncPolicy,
}

impl PySyncedEngineBuilder {
    fn synced(sync_policy: PySyncPolicy) -> Self {
        PySyncedEngineBuilder { sync_policy }
    }
}

#[pymethods]
impl PySyncedEngineBuilder {
    #[pyo3(signature = (policy))]
    fn pre_trade(&self, policy: &Bound<'_, PyAny>) -> PyResult<PyReadyEngineBuilder> {
        let rb = PyReadyEngineBuilder::new(self.sync_policy);
        rb.push_policy(policy)?;
        Ok(rb)
    }

    fn builtin(
        &self,
        py: Python<'_>,
        builtin_ready_builder: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyReadyEngineBuilder>> {
        let rb = Py::new(py, PyReadyEngineBuilder::new(self.sync_policy))?;
        builtin_ready_builder.call_method1("_build", (rb.bind(py),))?;
        Ok(rb)
    }

    fn market_data(&self, default_ttl: &PyQuoteTtl) -> PyMarketDataBuilder {
        // Map engine sync policy to MD mode: no-sync stays no-sync (no-op
        // locks); Full and Account both get real locks so a concurrent feed
        // is safe.
        let md_mode = match self.sync_policy {
            SyncMode::None => SyncMode::None,
            SyncMode::Full | SyncMode::Account => SyncMode::Full,
        };
        PyMarketDataBuilder {
            default_ttl: default_ttl.inner,
            mode: md_mode,
        }
    }
}

#[pyclass(name = "ReadyEngineBuilder", module = "openpit", unsendable)]
struct PyReadyEngineBuilder {
    state: RefCell<Option<PyBuilderState>>,
    sync_policy: PySyncPolicy,
}

impl PyReadyEngineBuilder {
    fn new(sync_policy: PySyncPolicy) -> Self {
        PyReadyEngineBuilder {
            state: RefCell::new(Some(PyBuilderState::Synced(
                EngineBuilder::<Order, ExecutionReport, AccountAdjustment>::new()
                    .sync(openpit_interop::EngineLocking::new(sync_policy)),
            ))),
            sync_policy,
        }
    }

    fn with_state(&self, f: impl FnOnce(PyBuilderState) -> PyBuilderState) -> PyResult<()> {
        let state = self
            .state
            .borrow_mut()
            .take()
            .ok_or_else(|| PyValueError::new_err("engine builder is no longer available"))?;
        *self.state.borrow_mut() = Some(f(state));
        Ok(())
    }

    fn add_policy(&self, policy: BoxedPreTradePolicy) -> PyResult<()> {
        self.with_state(|state| {
            PyBuilderState::Ready(match state {
                PyBuilderState::Synced(builder) => builder.pre_trade(policy),
                PyBuilderState::Ready(builder) => builder.pre_trade(policy),
            })
        })
    }

    fn add_configurable_rate_limit(
        &self,
        policy: RateLimitPolicy<openpit_interop::StorageLockingPolicyFactory>,
    ) -> PyResult<()> {
        self.with_state(|state| state.add_configurable_rate_limit(policy))
    }

    fn add_configurable_pnl_killswitch(
        &self,
        policy: PnlBoundsKillSwitchPolicy<openpit_interop::StorageLockingPolicyFactory>,
    ) -> PyResult<()> {
        self.with_state(|state| state.add_configurable_pnl_killswitch(policy))
    }

    fn add_configurable_order_size_limit(
        &self,
        policy: OrderSizeLimitPolicy<openpit_interop::StorageLockingPolicyFactory>,
    ) -> PyResult<()> {
        self.with_state(|state| state.add_configurable_order_size_limit(policy))
    }

    fn add_configurable_spot_funds(
        &self,
        policy: SpotFundsPolicy<openpit_interop::EngineLocking, openpit_interop::EngineLocking>,
    ) -> PyResult<()> {
        self.with_state(|state| state.add_configurable_spot_funds(policy))
    }

    fn push_policy(&self, policy: &Bound<'_, PyAny>) -> PyResult<()> {
        let name = policy
            .getattr("name")?
            .extract::<String>()
            .map_err(|_| PyValueError::new_err("policy.name must be a string"))?;
        let policy_group_id = if policy.hasattr("policy_group_id")? {
            parse_policy_group_id_input(&policy.getattr("policy_group_id")?)?
        } else {
            DEFAULT_POLICY_GROUP_ID
        };
        ensure_callable_method(policy, "check_pre_trade_start")?;
        ensure_callable_method(policy, "perform_pre_trade_check")?;
        ensure_callable_method(policy, "apply_execution_report")?;
        ensure_callable_method(policy, "apply_account_adjustment")?;
        self.add_policy(BoxedPreTradePolicy {
            inner: Box::new(PythonPreTradePolicyAdapter {
                name,
                policy_group_id,
                policy: policy.clone().unbind(),
            }),
        })
    }
}

#[pymethods]
impl PyReadyEngineBuilder {
    #[pyo3(signature = (policy))]
    fn pre_trade<'py>(
        slf: PyRef<'py, Self>,
        policy: &Bound<'_, PyAny>,
    ) -> PyResult<PyRef<'py, Self>> {
        slf.push_policy(policy)?;
        Ok(slf)
    }

    fn market_data(&self, default_ttl: &PyQuoteTtl) -> PyMarketDataBuilder {
        let md_mode = match self.sync_policy {
            SyncMode::None => SyncMode::None,
            SyncMode::Full | SyncMode::Account => SyncMode::Full,
        };
        PyMarketDataBuilder {
            default_ttl: default_ttl.inner,
            mode: md_mode,
        }
    }

    // Underscore-prefixed Python name: called only by policy builders' _build hooks.
    #[pyo3(name = "_add_builtin_rate_limit", signature = (*, policy_group_id = 0, broker = None, asset_barriers = vec![], account_barriers = vec![], account_asset_barriers = vec![]))]
    fn add_builtin_rate_limit<'py>(
        slf: PyRef<'py, Self>,
        policy_group_id: u16,
        broker: Option<(usize, u64)>,
        asset_barriers: Vec<(String, usize, u64)>,
        account_barriers: Vec<(u64, usize, u64)>,
        account_asset_barriers: Vec<(u64, String, usize, u64)>,
    ) -> PyResult<PyRef<'py, Self>> {
        let policy = {
            let state = slf.state.borrow();
            let storage_builder = state
                .as_ref()
                .ok_or_else(|| PyValueError::new_err("engine builder is no longer available"))?
                .storage_builder();
            make_rate_limit_policy(
                storage_builder,
                PolicyGroupId::new(policy_group_id),
                broker,
                asset_barriers,
                account_barriers,
                account_asset_barriers,
            )?
        };
        slf.add_configurable_rate_limit(policy)?;
        Ok(slf)
    }

    #[pyo3(name = "_add_builtin_order_size_limit", signature = (*, policy_group_id = 0, broker = None, asset_barriers = vec![], account_asset_barriers = vec![]))]
    fn add_builtin_order_size_limit<'py>(
        slf: PyRef<'py, Self>,
        policy_group_id: u16,
        broker: Option<PyRef<'_, PyOrderSizeLimit>>,
        asset_barriers: Vec<(PyRef<'_, PyOrderSizeLimit>, String)>,
        account_asset_barriers: Vec<(PyRef<'_, PyOrderSizeLimit>, u64, String)>,
    ) -> PyResult<PyRef<'py, Self>> {
        let policy = make_order_size_limit_policy(
            PolicyGroupId::new(policy_group_id),
            broker,
            asset_barriers,
            account_asset_barriers,
        )?;
        slf.add_configurable_order_size_limit(policy)?;
        Ok(slf)
    }

    #[pyo3(name = "_add_builtin_pnl_bounds_killswitch", signature = (*, policy_group_id = 0, broker_barriers = vec![], account_barriers = vec![]))]
    fn add_builtin_pnl_bounds_killswitch<'py>(
        slf: PyRef<'py, Self>,
        policy_group_id: u16,
        broker_barriers: Vec<Bound<'_, PyAny>>,
        account_barriers: Vec<Bound<'_, PyAny>>,
    ) -> PyResult<PyRef<'py, Self>> {
        let policy = {
            let state = slf.state.borrow();
            let storage_builder = state
                .as_ref()
                .ok_or_else(|| PyValueError::new_err("engine builder is no longer available"))?
                .storage_builder();
            make_pnl_killswitch_policy(
                storage_builder,
                PolicyGroupId::new(policy_group_id),
                broker_barriers,
                account_barriers,
            )?
        };
        slf.add_configurable_pnl_killswitch(policy)?;
        Ok(slf)
    }

    #[pyo3(name = "_add_builtin_order_validation", signature = (*, policy_group_id = 0))]
    fn add_builtin_order_validation<'py>(
        slf: PyRef<'py, Self>,
        policy_group_id: u16,
    ) -> PyResult<PyRef<'py, Self>> {
        slf.add_policy(make_order_validation_start_check(PolicyGroupId::new(
            policy_group_id,
        )))?;
        Ok(slf)
    }

    #[pyo3(name = "_add_builtin_spot_funds", signature = (*, policy_group_id = 0, market_data = None, global_slippage_bps = None, pricing_source = None, overrides = vec![]))]
    fn add_builtin_spot_funds<'py>(
        slf: PyRef<'py, Self>,
        policy_group_id: u16,
        market_data: Option<PyRef<'_, PyMarketDataService>>,
        global_slippage_bps: Option<u16>,
        pricing_source: Option<&str>,
        overrides: Vec<Bound<'_, PyAny>>,
    ) -> PyResult<PyRef<'py, Self>> {
        let engine_sync = slf.sync_policy;
        let md_handle = market_data.map(|svc| (svc.inner.clone(), svc.mode));
        let overrides = overrides
            .iter()
            .map(parse_spot_funds_override_entity)
            .collect::<PyResult<Vec<_>>>()?;
        let policy = {
            let state = slf.state.borrow();
            let state_ref = state
                .as_ref()
                .ok_or_else(|| PyValueError::new_err("engine builder is no longer available"))?;
            let storage_builder = state_ref.storage_builder();
            make_spot_funds_policy(
                storage_builder,
                PolicyGroupId::new(policy_group_id),
                engine_sync,
                md_handle,
                global_slippage_bps,
                pricing_source,
                overrides,
            )?
        };
        slf.add_configurable_spot_funds(policy)?;
        Ok(slf)
    }

    fn builtin<'py>(
        slf: &Bound<'py, Self>,
        builtin_ready_builder: &Bound<'_, PyAny>,
    ) -> PyResult<Bound<'py, Self>> {
        builtin_ready_builder.call_method1("_build", (slf,))?;
        Ok(slf.clone())
    }

    fn build(&self) -> PyResult<PyEngine> {
        let state = self
            .state
            .borrow_mut()
            .take()
            .ok_or_else(|| PyValueError::new_err("engine builder is no longer available"))?;
        match state {
            PyBuilderState::Ready(builder) => builder
                .build()
                .map(|engine| PyEngine { inner: engine })
                .map_err(|e| PyValueError::new_err(format_engine_build_error(e))),
            PyBuilderState::Synced(_) => Err(PyValueError::new_err("no policies registered")),
        }
    }
}

#[pyclass(name = "OrderSizeLimit", module = "openpit.pretrade.policies")]
#[derive(Clone)]
struct PyOrderSizeLimit {
    max_quantity: String,
    max_notional: String,
}

#[pymethods]
impl PyOrderSizeLimit {
    #[new]
    #[pyo3(signature = (*, max_quantity, max_notional))]
    fn new(max_quantity: &Bound<'_, PyAny>, max_notional: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            max_quantity: parse_quantity_input(max_quantity)?.to_string(),
            max_notional: parse_volume_input(max_notional)?.to_string(),
        })
    }
}

fn ensure_policy_entity(obj: &Bound<'_, PyAny>, class_name: &str) -> PyResult<()> {
    let policies = obj.py().import("openpit.pretrade.policies")?;
    let expected = policies.getattr(class_name)?;
    if obj.is_instance(&expected)? {
        return Ok(());
    }
    Err(PyTypeError::new_err(format!(
        "expected openpit.pretrade.policies.{class_name}"
    )))
}

fn parse_rate_limit_window(value: &Bound<'_, PyAny>) -> PyResult<Duration> {
    let datetime = value.py().import("datetime")?;
    let timedelta = datetime.getattr("timedelta")?;
    if !value.is_instance(&timedelta)? {
        return Err(PyTypeError::new_err(
            "RateLimit.window must be datetime.timedelta",
        ));
    }

    let days = value.getattr("days")?.extract::<i64>()?;
    if days < 0 {
        return Err(PyValueError::new_err(
            "RateLimit.window must not be negative",
        ));
    }
    let days = u64::try_from(days)
        .map_err(|_| PyValueError::new_err("RateLimit.window must not be negative"))?;
    let seconds = value.getattr("seconds")?.extract::<u64>()?;
    let microseconds = value.getattr("microseconds")?.extract::<u32>()?;
    // `datetime.timedelta` normalises to 0 <= seconds < 86_400 and
    // 0 <= microseconds < 1_000_000, bounded by `timedelta.max` (~10^9 days),
    // so neither the day-to-second scaling nor the micro-to-nano scaling can
    // overflow their target types.
    Ok(Duration::new(days * 86_400 + seconds, microseconds * 1_000))
}

fn parse_rate_limit_entity(obj: &Bound<'_, PyAny>) -> PyResult<RateLimit> {
    ensure_policy_entity(obj, "RateLimit")?;
    Ok(RateLimit {
        max_orders: obj.getattr("max_orders")?.extract()?,
        window: parse_rate_limit_window(&obj.getattr("window")?)?,
    })
}

fn parse_rate_limit_broker_barrier(obj: &Bound<'_, PyAny>) -> PyResult<RateLimitBrokerBarrier> {
    ensure_policy_entity(obj, "RateLimitBrokerBarrier")?;
    Ok(RateLimitBrokerBarrier {
        limit: parse_rate_limit_entity(&obj.getattr("limit")?)?,
    })
}

fn parse_rate_limit_asset_barrier(obj: &Bound<'_, PyAny>) -> PyResult<RateLimitAssetBarrier> {
    ensure_policy_entity(obj, "RateLimitAssetBarrier")?;
    Ok(RateLimitAssetBarrier {
        limit: parse_rate_limit_entity(&obj.getattr("limit")?)?,
        settlement_asset: parse_asset_input(&obj.getattr("settlement_asset")?)?,
    })
}

fn parse_rate_limit_account_barrier(obj: &Bound<'_, PyAny>) -> PyResult<RateLimitAccountBarrier> {
    ensure_policy_entity(obj, "RateLimitAccountBarrier")?;
    Ok(RateLimitAccountBarrier {
        limit: parse_rate_limit_entity(&obj.getattr("limit")?)?,
        account_id: parse_account_id_input(&obj.getattr("account_id")?)?,
    })
}

fn parse_rate_limit_account_asset_barrier(
    obj: &Bound<'_, PyAny>,
) -> PyResult<RateLimitAccountAssetBarrier> {
    ensure_policy_entity(obj, "RateLimitAccountAssetBarrier")?;
    Ok(RateLimitAccountAssetBarrier {
        limit: parse_rate_limit_entity(&obj.getattr("limit")?)?,
        account_id: parse_account_id_input(&obj.getattr("account_id")?)?,
        settlement_asset: parse_asset_input(&obj.getattr("settlement_asset")?)?,
    })
}

fn parse_order_size_limit_entity(obj: &Bound<'_, PyAny>) -> PyResult<OrderSizeLimit> {
    let limit = obj
        .extract::<PyRef<'_, PyOrderSizeLimit>>()
        .map_err(|_| PyTypeError::new_err("limit must be OrderSizeLimit"))?;
    Ok(OrderSizeLimit {
        max_quantity: parse_quantity(&limit.max_quantity)?,
        max_notional: parse_volume(&limit.max_notional)?,
    })
}

fn parse_order_size_broker_barrier(
    obj: &Bound<'_, PyAny>,
) -> PyResult<openpit::pretrade::policies::OrderSizeBrokerBarrier> {
    ensure_policy_entity(obj, "OrderSizeBrokerBarrier")?;
    Ok(openpit::pretrade::policies::OrderSizeBrokerBarrier {
        limit: parse_order_size_limit_entity(&obj.getattr("limit")?)?,
    })
}

fn parse_order_size_asset_barrier(obj: &Bound<'_, PyAny>) -> PyResult<OrderSizeAssetBarrier> {
    ensure_policy_entity(obj, "OrderSizeAssetBarrier")?;
    Ok(OrderSizeAssetBarrier {
        limit: parse_order_size_limit_entity(&obj.getattr("limit")?)?,
        settlement_asset: parse_asset_input(&obj.getattr("settlement_asset")?)?,
    })
}

fn parse_order_size_account_asset_barrier(
    obj: &Bound<'_, PyAny>,
) -> PyResult<OrderSizeAccountAssetBarrier> {
    ensure_policy_entity(obj, "OrderSizeAccountAssetBarrier")?;
    Ok(OrderSizeAccountAssetBarrier {
        limit: parse_order_size_limit_entity(&obj.getattr("limit")?)?,
        account_id: parse_account_id_input(&obj.getattr("account_id")?)?,
        settlement_asset: parse_asset_input(&obj.getattr("settlement_asset")?)?,
    })
}

type ParsedRateLimitBarriers = (
    Option<RateLimitBrokerBarrier>,
    Vec<RateLimitAssetBarrier>,
    Vec<RateLimitAccountBarrier>,
    Vec<RateLimitAccountAssetBarrier>,
);

fn parse_rate_limit_barriers(
    broker: Option<(usize, u64)>,
    asset_barriers: Vec<(String, usize, u64)>,
    account_barriers: Vec<(u64, usize, u64)>,
    account_asset_barriers: Vec<(u64, String, usize, u64)>,
) -> PyResult<ParsedRateLimitBarriers> {
    let broker_barrier = broker.map(|(max_orders, window_nanoseconds)| RateLimitBrokerBarrier {
        limit: RateLimit {
            max_orders,
            window: Duration::from_nanos(window_nanoseconds),
        },
    });

    let asset: Vec<RateLimitAssetBarrier> = asset_barriers
        .into_iter()
        .map(|(settlement, max_orders, window_nanoseconds)| {
            Ok(RateLimitAssetBarrier {
                limit: RateLimit {
                    max_orders,
                    window: Duration::from_nanos(window_nanoseconds),
                },
                settlement_asset: openpit::param::Asset::new(&settlement)
                    .map_err(|e| PyValueError::new_err(e.to_string()))?,
            })
        })
        .collect::<PyResult<Vec<_>>>()?;

    let account: Vec<RateLimitAccountBarrier> = account_barriers
        .into_iter()
        .map(
            |(account_id, max_orders, window_nanoseconds)| RateLimitAccountBarrier {
                limit: RateLimit {
                    max_orders,
                    window: Duration::from_nanos(window_nanoseconds),
                },
                account_id: AccountId::from_u64(account_id),
            },
        )
        .collect();

    let account_asset: Vec<RateLimitAccountAssetBarrier> = account_asset_barriers
        .into_iter()
        .map(|(account_id, settlement, max_orders, window_nanoseconds)| {
            Ok(RateLimitAccountAssetBarrier {
                limit: RateLimit {
                    max_orders,
                    window: Duration::from_nanos(window_nanoseconds),
                },
                account_id: AccountId::from_u64(account_id),
                settlement_asset: openpit::param::Asset::new(&settlement)
                    .map_err(|e| PyValueError::new_err(e.to_string()))?,
            })
        })
        .collect::<PyResult<Vec<_>>>()?;

    Ok((broker_barrier, asset, account, account_asset))
}

fn make_rate_limit_policy(
    storage_builder: &StorageBuilder<openpit_interop::StorageLockingPolicyFactory>,
    policy_group_id: PolicyGroupId,
    broker: Option<(usize, u64)>,
    asset_barriers: Vec<(String, usize, u64)>,
    account_barriers: Vec<(u64, usize, u64)>,
    account_asset_barriers: Vec<(u64, String, usize, u64)>,
) -> PyResult<RateLimitPolicy<openpit_interop::StorageLockingPolicyFactory>> {
    let (broker_barrier, asset, account, account_asset) = parse_rate_limit_barriers(
        broker,
        asset_barriers,
        account_barriers,
        account_asset_barriers,
    )?;
    let settings = RateLimitSettings::new(broker_barrier, asset, account, account_asset)
        .map_err(|e| PyValueError::new_err(e.to_string()))?;
    Ok(RateLimitPolicy::new(settings, storage_builder).with_policy_group_id(policy_group_id))
}

fn make_order_size_limit_policy(
    policy_group_id: PolicyGroupId,
    broker: Option<PyRef<'_, PyOrderSizeLimit>>,
    asset_barriers: Vec<(PyRef<'_, PyOrderSizeLimit>, String)>,
    account_asset_barriers: Vec<(PyRef<'_, PyOrderSizeLimit>, u64, String)>,
) -> PyResult<OrderSizeLimitPolicy<openpit_interop::StorageLockingPolicyFactory>> {
    use openpit::param::AccountId;

    let broker_barrier = broker
        .map(|l| {
            Ok::<_, pyo3::PyErr>(openpit::pretrade::policies::OrderSizeBrokerBarrier {
                limit: OrderSizeLimit {
                    max_quantity: parse_quantity(&l.max_quantity)?,
                    max_notional: parse_volume(&l.max_notional)?,
                },
            })
        })
        .transpose()?;

    let asset: Vec<OrderSizeAssetBarrier> = asset_barriers
        .into_iter()
        .map(|(l, settlement)| {
            Ok(OrderSizeAssetBarrier {
                limit: OrderSizeLimit {
                    max_quantity: parse_quantity(&l.max_quantity)?,
                    max_notional: parse_volume(&l.max_notional)?,
                },
                settlement_asset: parse_asset(&settlement)?,
            })
        })
        .collect::<PyResult<Vec<_>>>()?;

    let account_asset: Vec<OrderSizeAccountAssetBarrier> = account_asset_barriers
        .into_iter()
        .map(|(l, account_id, settlement)| {
            Ok(OrderSizeAccountAssetBarrier {
                limit: OrderSizeLimit {
                    max_quantity: parse_quantity(&l.max_quantity)?,
                    max_notional: parse_volume(&l.max_notional)?,
                },
                account_id: AccountId::from_u64(account_id),
                settlement_asset: parse_asset(&settlement)?,
            })
        })
        .collect::<PyResult<Vec<_>>>()?;

    let settings = OrderSizeLimitSettings::new(broker_barrier, asset, account_asset)
        .map_err(|e| PyValueError::new_err(e.to_string()))?;
    Ok(OrderSizeLimitPolicy::new(settings).with_policy_group_id(policy_group_id))
}

fn parse_pnl_bounds_barrier(obj: &Bound<'_, PyAny>) -> PyResult<PnlBoundsBrokerBarrier> {
    let settlement_asset = obj.getattr("settlement_asset")?;
    let lower_bound_value = obj.getattr("lower_bound")?;
    let lower_bound = if lower_bound_value.is_none() {
        None
    } else {
        Some(parse_pnl_input(&lower_bound_value)?)
    };
    let upper_bound_value = obj.getattr("upper_bound")?;
    let upper_bound = if upper_bound_value.is_none() {
        None
    } else {
        Some(parse_pnl_input(&upper_bound_value)?)
    };
    Ok(PnlBoundsBrokerBarrier {
        settlement_asset: parse_asset_input(&settlement_asset)?,
        lower_bound,
        upper_bound,
    })
}

fn parse_pnl_broker_barrier(obj: &Bound<'_, PyAny>) -> PyResult<PnlBoundsBrokerBarrier> {
    ensure_policy_entity(obj, "PnlBoundsBrokerBarrier")?;
    parse_pnl_bounds_barrier(obj)
}

fn parse_pnl_account_barrier(obj: &Bound<'_, PyAny>) -> PyResult<PnlBoundsAccountAssetBarrier> {
    ensure_policy_entity(obj, "PnlBoundsAccountAssetBarrier")?;
    let barrier_obj = obj.getattr("barrier")?;
    let account_id_obj = obj.getattr("account_id")?;
    let initial_pnl_obj = obj.getattr("initial_pnl")?;
    Ok(PnlBoundsAccountAssetBarrier {
        barrier: parse_pnl_broker_barrier(&barrier_obj)?,
        account_id: parse_account_id_input(&account_id_obj)?,
        initial_pnl: parse_pnl_input(&initial_pnl_obj)?,
    })
}

fn parse_pnl_account_barrier_update(
    obj: &Bound<'_, PyAny>,
) -> PyResult<PnlBoundsAccountAssetBarrierUpdate> {
    ensure_policy_entity(obj, "PnlBoundsAccountAssetBarrierUpdate")?;
    let barrier_obj = obj.getattr("barrier")?;
    let account_id_obj = obj.getattr("account_id")?;
    Ok(PnlBoundsAccountAssetBarrierUpdate {
        barrier: parse_pnl_broker_barrier(&barrier_obj)?,
        account_id: parse_account_id_input(&account_id_obj)?,
    })
}

fn parse_pnl_killswitch_barriers<'py>(
    broker_barriers: &[Bound<'py, PyAny>],
    account_barriers: &[Bound<'py, PyAny>],
) -> PyResult<(
    Vec<PnlBoundsBrokerBarrier>,
    Vec<PnlBoundsAccountAssetBarrier>,
)> {
    let brokers: Vec<PnlBoundsBrokerBarrier> = broker_barriers
        .iter()
        .map(parse_pnl_broker_barrier)
        .collect::<PyResult<Vec<_>>>()?;
    let accounts: Vec<PnlBoundsAccountAssetBarrier> = account_barriers
        .iter()
        .map(parse_pnl_account_barrier)
        .collect::<PyResult<Vec<_>>>()?;
    Ok((brokers, accounts))
}

fn make_pnl_killswitch_policy(
    storage_builder: &StorageBuilder<openpit_interop::StorageLockingPolicyFactory>,
    policy_group_id: PolicyGroupId,
    broker_barriers: Vec<Bound<'_, PyAny>>,
    account_barriers: Vec<Bound<'_, PyAny>>,
) -> PyResult<PnlBoundsKillSwitchPolicy<openpit_interop::StorageLockingPolicyFactory>> {
    let (brokers, accounts) = parse_pnl_killswitch_barriers(&broker_barriers, &account_barriers)?;
    let settings = PnlBoundsKillSwitchSettings::new(brokers, accounts)
        .map_err(|e| PyValueError::new_err(e.to_string()))?;
    Ok(PnlBoundsKillSwitchPolicy::new(settings, storage_builder)
        .with_policy_group_id(policy_group_id))
}

fn make_order_validation_start_check(policy_group_id: PolicyGroupId) -> BoxedPreTradePolicy {
    BoxedPreTradePolicy {
        inner: Box::new(OrderValidationPolicy::new().with_policy_group_id(policy_group_id)),
    }
}

fn parse_spot_funds_pricing_source_str(value: &str) -> PyResult<SpotFundsPricingSource> {
    match value {
        "Mark" => Ok(SpotFundsPricingSource::Mark),
        "BookTop" => Ok(SpotFundsPricingSource::BookTop),
        other => Err(PyValueError::new_err(format!(
            "pricing_source must be 'Mark' or 'BookTop', got {other:?}"
        ))),
    }
}

fn parse_spot_funds_pricing_source_entity(
    value: &Bound<'_, PyAny>,
) -> PyResult<SpotFundsPricingSource> {
    ensure_policy_entity(value, "SpotFundsPricingSource")?;
    parse_spot_funds_pricing_source_str(&value.getattr("value")?.extract::<String>()?)
}

fn parse_spot_funds_override_entity(
    value: &Bound<'_, PyAny>,
) -> PyResult<(SpotFundsOverrideTarget, SpotFundsOverride)> {
    ensure_policy_entity(value, "SpotFundsOverrideEntry")?;
    let target_value = value.getattr("target")?;
    let target = parse_spot_funds_override_target_entity(&target_value)?;

    let override_value = value.getattr("override")?;
    ensure_policy_entity(&override_value, "SpotFundsOverride")?;
    let slippage_bps_value = override_value.getattr("slippage_bps")?;
    let slippage_bps = if slippage_bps_value.is_none() {
        None
    } else {
        Some(slippage_bps_value.extract::<u16>()?)
    };

    Ok((target, SpotFundsOverride { slippage_bps }))
}

fn parse_spot_funds_override_target_entity(
    value: &Bound<'_, PyAny>,
) -> PyResult<SpotFundsOverrideTarget> {
    let policies = value.py().import("openpit.pretrade.policies")?;
    let instrument_target = policies.getattr("SpotFundsOverrideTargetInstrument")?;
    let account_target = policies.getattr("SpotFundsOverrideTargetInstrumentAccount")?;
    let account_group_target = policies.getattr("SpotFundsOverrideTargetInstrumentAccountGroup")?;

    let instrument = || {
        value
            .getattr("instrument")?
            .extract::<PyRef<'_, PyInstrumentId>>()
            .map(|instrument| instrument.inner)
            .map_err(|_| {
                PyTypeError::new_err(
                    "spot funds override target instrument must be \
                     openpit.marketdata.InstrumentId",
                )
            })
    };

    if value.is_instance(&instrument_target)? {
        return Ok(SpotFundsOverrideTarget::Instrument(instrument()?));
    }
    if value.is_instance(&account_target)? {
        let account_id = value
            .getattr("account_id")?
            .extract::<PyRef<'_, PyAccountId>>()
            .map_err(|_| {
                PyTypeError::new_err(
                    "SpotFundsOverrideTargetInstrumentAccount.account_id must be \
                     openpit.param.AccountId",
                )
            })?
            .inner;
        return Ok(SpotFundsOverrideTarget::InstrumentAccount(
            instrument()?,
            account_id,
        ));
    }
    if value.is_instance(&account_group_target)? {
        let account_group_id = value
            .getattr("account_group_id")?
            .extract::<PyRef<'_, PyAccountGroupId>>()
            .map_err(|_| {
                PyTypeError::new_err(
                    "SpotFundsOverrideTargetInstrumentAccountGroup.account_group_id \
                     must be openpit.param.AccountGroupId",
                )
            })?
            .inner;
        return Ok(SpotFundsOverrideTarget::InstrumentAccountGroup(
            instrument()?,
            account_group_id,
        ));
    }

    Err(PyTypeError::new_err(
        "SpotFundsOverrideEntry.target must be one of \
         openpit.pretrade.policies.SpotFundsOverrideTargetInstrument, \
         openpit.pretrade.policies.SpotFundsOverrideTargetInstrumentAccount, or \
         openpit.pretrade.policies.SpotFundsOverrideTargetInstrumentAccountGroup",
    ))
}

fn parse_spot_funds_pricing_source(value: Option<&str>) -> PyResult<SpotFundsPricingSource> {
    parse_spot_funds_pricing_source_str(value.unwrap_or("Mark"))
}

fn make_spot_funds_policy(
    storage_builder: &StorageBuilder<openpit_interop::StorageLockingPolicyFactory>,
    policy_group_id: PolicyGroupId,
    engine_sync: SyncMode,
    market_data: Option<(EngineHandle<MarketDataService<EngineLocking>>, SyncMode)>,
    global_slippage_bps: Option<u16>,
    pricing_source: Option<&str>,
    instrument_overrides: Vec<(SpotFundsOverrideTarget, SpotFundsOverride)>,
) -> PyResult<SpotFundsPolicy<EngineLocking, EngineLocking>> {
    let market_orders: Option<SpotFundsMarketData<EngineLocking>> = match market_data {
        Some((handle, md_mode)) => {
            // Mismatch guard: a multi-threaded engine requires a fully-locked MD
            // service. A no-sync MD service has no-op internal locks and is
            // unsafe under concurrent access from a Full/Account engine.
            if matches!(engine_sync, SyncMode::Full | SyncMode::Account)
                && md_mode == SyncMode::None
            {
                return Err(PyValueError::new_err(
                    "market data service is in no-sync mode but the engine is multi-threaded; \
                     call .full_sync() on the market-data builder before .build()",
                ));
            }
            if global_slippage_bps.is_none() {
                return Err(PyValueError::new_err(
                    "global_slippage_bps is required when market_data is provided",
                ));
            }
            Some(SpotFundsMarketData::<EngineLocking>::new(handle))
        }
        None => None,
    };
    // Build the settings for the slippage cascade.
    let slippage_bps = global_slippage_bps.unwrap_or(0);
    let settings = SpotFundsSettings::new(
        slippage_bps,
        parse_spot_funds_pricing_source(pricing_source)?,
        instrument_overrides,
    )
    .map_err(|e| PyValueError::new_err(e.to_string()))?;
    Ok(SpotFundsPolicy::<EngineLocking, EngineLocking>::new(
        settings,
        market_orders,
        storage_builder,
    )
    .with_policy_group_id(policy_group_id))
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
                PyModule::import(py, "openpit.param")?
                    .getattr("Side")?
                    .call1((side_name(side),))
                    .map(|obj| obj.unbind())
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
                PyModule::import(py, "openpit.param")?
                    .getattr("PositionSide")?
                    .call1((position_side_name(side),))
                    .map(|obj| obj.unbind())
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

#[pyclass(name = "Order", module = "openpit.core", subclass)]
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
    balance: Option<AdjustmentAmount>,
    held: Option<AdjustmentAmount>,
    incoming: Option<AdjustmentAmount>,
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
    balance_upper: Option<PositionSize>,
    balance_lower: Option<PositionSize>,
    held_upper: Option<PositionSize>,
    held_lower: Option<PositionSize>,
    incoming_upper: Option<PositionSize>,
    incoming_lower: Option<PositionSize>,
}

enum PyAccountAdjustmentOperation {
    Balance(Py<PyAccountAdjustmentBalanceOperation>),
    Position(Py<PyAccountAdjustmentPositionOperation>),
}

#[pyclass(name = "AccountAdjustment", module = "openpit.core", subclass)]
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
    ///
    /// Raises ``OverflowError`` for negative values.
    #[staticmethod]
    fn from_int(value: &Bound<'_, PyAny>) -> PyResult<Self> {
        let raw = value.extract::<u64>().map_err(|_| {
            pyo3::exceptions::PyOverflowError::new_err(
                "account id must be a non-negative integer in range 0..=18446744073709551615",
            )
        })?;
        Ok(Self {
            inner: AccountId::from_u64(raw),
        })
    }

    /// Constructs an account identifier by hashing a string with FNV-1a 64-bit.
    ///
    /// Collisions are theoretically possible. For n distinct account strings
    /// the probability of at least one collision is approximately n^2 / 2^65.
    /// If collision risk is unacceptable, use ``from_int`` with a collision-free
    /// integer mapping instead. See <http://www.isthe.com/chongo/tech/comp/fnv/> for the algorithm
    /// specification.
    #[staticmethod]
    fn from_string(value: &str) -> PyResult<Self> {
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
            return Ok(pyo3::types::PyBool::new(py, result)
                .as_any()
                .clone()
                .unbind());
        }
        Ok(py.NotImplemented().into())
    }

    fn __hash__(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.inner.as_u64().hash(&mut hasher);
        hasher.finish()
    }
}

#[pyclass(name = "AccountGroupId", module = "openpit.param")]
#[derive(Clone, Copy)]
struct PyAccountGroupId {
    inner: AccountGroupId,
}

#[pymethods]
impl PyAccountGroupId {
    /// The account group an account belongs to until assigned to another.
    ///
    /// Mirrors the module-level ``openpit.param.DEFAULT_ACCOUNT_GROUP``. No
    /// constructor produces this value: ``from_int(0)`` raises and ``from_string``
    /// never hashes to it.
    #[classattr]
    #[allow(non_snake_case)]
    fn DEFAULT() -> Self {
        Self {
            inner: AccountGroupId::DEFAULT,
        }
    }

    /// Constructs an account-group identifier from an integer.
    ///
    /// No hashing, no collision risk.
    ///
    /// Raises ``ValueError`` when ``value`` equals the reserved
    /// ``DEFAULT_ACCOUNT_GROUP`` (``0``); that group cannot be named by a
    /// constructor. Raises ``OverflowError`` for values outside
    /// ``1..=4294967295``.
    #[staticmethod]
    fn from_int(value: &Bound<'_, PyAny>) -> PyResult<Self> {
        let raw = value.extract::<u32>().map_err(|_| {
            pyo3::exceptions::PyOverflowError::new_err(
                "account_group_id must be an integer in range 1..=4294967295",
            )
        })?;
        Ok(Self {
            inner: AccountGroupId::from_u32(raw).map_err(convert_account_group_id_error)?,
        })
    }

    /// Constructs an account-group identifier by hashing a string with FNV-1a 32-bit.
    ///
    /// Hash collisions are possible. For n distinct group strings the probability of at
    /// least one collision is approximately n^2 / (2 * 2^32). If collision risk is
    /// unacceptable, use ``from_int`` with a collision-free integer mapping instead.
    /// See <http://www.isthe.com/chongo/tech/comp/fnv/> for the algorithm specification.
    #[staticmethod]
    fn from_string(value: &str) -> PyResult<Self> {
        Ok(Self {
            inner: AccountGroupId::from_str(value)
                .map_err(|error| PyValueError::new_err(error.to_string()))?,
        })
    }

    #[getter]
    fn value(&self) -> u32 {
        self.inner.as_u32()
    }

    fn __repr__(&self) -> String {
        format!("AccountGroupId(value={:?})", self.value())
    }

    fn __richcmp__(&self, other: &Bound<'_, PyAny>, op: CompareOp) -> PyResult<PyObject> {
        let py = other.py();
        if let Ok(other) = other.extract::<PyRef<'_, Self>>() {
            let result = match op {
                CompareOp::Eq => self.inner == other.inner,
                CompareOp::Ne => self.inner != other.inner,
                _ => return Ok(py.NotImplemented().into()),
            };
            return Ok(pyo3::types::PyBool::new(py, result)
                .as_any()
                .clone()
                .unbind());
        }
        Ok(py.NotImplemented().into())
    }

    fn __hash__(&self) -> u32 {
        self.inner.as_u32()
    }
}

fn parse_account_group_id_input(value: &Bound<'_, PyAny>) -> PyResult<AccountGroupId> {
    if let Ok(group) = value.extract::<PyRef<'_, PyAccountGroupId>>() {
        return Ok(group.inner);
    }
    let raw = value.extract::<u32>().map_err(|_| {
        PyValueError::new_err(
            "account_group_id must be an AccountGroupId or integer in range 1..=4294967295",
        )
    })?;
    AccountGroupId::from_u32(raw).map_err(convert_account_group_id_error)
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
            fn from_string(value: &str) -> PyResult<Self> {
                Ok(Self {
                    inner: <$domain>::from_str(value)
                        .map_err(|error| create_param_error(error.to_string()))?,
                })
            }

            #[staticmethod]
            fn from_int(value: i128) -> PyResult<Self> {
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
            fn from_string_rounded(value: &str, scale: u32, strategy: &str) -> PyResult<Self> {
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
                    return Ok(pyo3::types::PyBool::new(py, result).as_any().clone().unbind());
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
                    return Ok(Py::new(py, Self { inner: result })?.into_any());
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
                    return Ok(Py::new(py, Self { inner: result })?.into_any());
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
                    return Ok(Py::new(py, Self { inner: result })?.into_any());
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
                    return Ok(Py::new(py, Self { inner: result })?.into_any());
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
                    return Ok(Py::new(py, Self { inner: result })?.into_any());
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
                    return Ok(Py::new(py, Self { inner: result })?.into_any());
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
            fn from_string(value: &str) -> PyResult<Self> {
                Ok(Self {
                    inner: <$domain>::from_str(value)
                        .map_err(|error| create_param_error(error.to_string()))?,
                })
            }

            #[staticmethod]
            fn from_int(value: i128) -> PyResult<Self> {
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
            fn from_string_rounded(value: &str, scale: u32, strategy: &str) -> PyResult<Self> {
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
                    return Ok(pyo3::types::PyBool::new(py, result).as_any().clone().unbind());
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
                    return Ok(Py::new(py, Self { inner: result })?.into_any());
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
                    return Ok(Py::new(py, Self { inner: result })?.into_any());
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
                    return Ok(Py::new(py, Self { inner: result })?.into_any());
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
                    return Ok(Py::new(py, Self { inner: result })?.into_any());
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
                    return Ok(Py::new(py, Self { inner: result })?.into_any());
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
                    return Ok(Py::new(py, Self { inner: result })?.into_any());
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

        fn to_position_size(&self) -> PyPositionSize {
            PyPositionSize {
                inner: self.inner.to_position_size(),
            }
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

    fn calculate_position_size(&self, quantity: &PyQuantity) -> PyResult<PyPositionSize> {
        Ok(PyPositionSize {
            inner: self
                .inner
                .calculate_position_size(quantity.inner)
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

    fn to_position_size(&self) -> PyPositionSize {
        PyPositionSize {
            inner: self.inner.to_position_size(),
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
    #[pyo3(signature = (*, balance = None, held = None, incoming = None))]
    fn new(
        balance: Option<&Bound<'_, PyAny>>,
        held: Option<&Bound<'_, PyAny>>,
        incoming: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        Ok(Self {
            balance: balance.map(parse_adjustment_amount_input).transpose()?,
            held: held.map(parse_adjustment_amount_input).transpose()?,
            incoming: incoming.map(parse_adjustment_amount_input).transpose()?,
        })
    }

    #[getter]
    fn balance(&self) -> Option<PyAdjustmentAmount> {
        self.balance.map(|inner| PyAdjustmentAmount { inner })
    }

    #[setter]
    fn set_balance(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.balance = value.map(parse_adjustment_amount_input).transpose()?;
        Ok(())
    }

    #[getter]
    fn held(&self) -> Option<PyAdjustmentAmount> {
        self.held.map(|inner| PyAdjustmentAmount { inner })
    }

    #[setter]
    fn set_held(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.held = value.map(parse_adjustment_amount_input).transpose()?;
        Ok(())
    }

    #[getter]
    fn incoming(&self) -> Option<PyAdjustmentAmount> {
        self.incoming.map(|inner| PyAdjustmentAmount { inner })
    }

    #[setter]
    fn set_incoming(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.incoming = value.map(parse_adjustment_amount_input).transpose()?;
        Ok(())
    }

    fn __repr__(&self) -> String {
        format!(
            "AccountAdjustmentAmount(balance={:?}, held={:?}, incoming={:?})",
            self.balance().map(|v| v.__repr__()),
            self.held().map(|v| v.__repr__()),
            self.incoming().map(|v| v.__repr__()),
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
                PyModule::import(py, "openpit.param")?
                    .getattr("PositionMode")?
                    .call1((position_mode_name(mode),))
                    .map(|obj| obj.unbind())
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
    #[pyo3(signature = (*, balance_upper = None, balance_lower = None, held_upper = None, held_lower = None, incoming_upper = None, incoming_lower = None))]
    fn new(
        balance_upper: Option<&Bound<'_, PyAny>>,
        balance_lower: Option<&Bound<'_, PyAny>>,
        held_upper: Option<&Bound<'_, PyAny>>,
        held_lower: Option<&Bound<'_, PyAny>>,
        incoming_upper: Option<&Bound<'_, PyAny>>,
        incoming_lower: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        Ok(Self {
            balance_upper: balance_upper.map(parse_position_size_input).transpose()?,
            balance_lower: balance_lower.map(parse_position_size_input).transpose()?,
            held_upper: held_upper.map(parse_position_size_input).transpose()?,
            held_lower: held_lower.map(parse_position_size_input).transpose()?,
            incoming_upper: incoming_upper.map(parse_position_size_input).transpose()?,
            incoming_lower: incoming_lower.map(parse_position_size_input).transpose()?,
        })
    }

    #[getter]
    fn balance_upper(&self) -> Option<PyPositionSize> {
        self.balance_upper.map(|inner| PyPositionSize { inner })
    }
    #[setter]
    fn set_balance_upper(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.balance_upper = value.map(parse_position_size_input).transpose()?;
        Ok(())
    }
    #[getter]
    fn balance_lower(&self) -> Option<PyPositionSize> {
        self.balance_lower.map(|inner| PyPositionSize { inner })
    }
    #[setter]
    fn set_balance_lower(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.balance_lower = value.map(parse_position_size_input).transpose()?;
        Ok(())
    }
    #[getter]
    fn held_upper(&self) -> Option<PyPositionSize> {
        self.held_upper.map(|inner| PyPositionSize { inner })
    }
    #[setter]
    fn set_held_upper(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.held_upper = value.map(parse_position_size_input).transpose()?;
        Ok(())
    }
    #[getter]
    fn held_lower(&self) -> Option<PyPositionSize> {
        self.held_lower.map(|inner| PyPositionSize { inner })
    }
    #[setter]
    fn set_held_lower(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.held_lower = value.map(parse_position_size_input).transpose()?;
        Ok(())
    }
    #[getter]
    fn incoming_upper(&self) -> Option<PyPositionSize> {
        self.incoming_upper.map(|inner| PyPositionSize { inner })
    }
    #[setter]
    fn set_incoming_upper(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.incoming_upper = value.map(parse_position_size_input).transpose()?;
        Ok(())
    }
    #[getter]
    fn incoming_lower(&self) -> Option<PyPositionSize> {
        self.incoming_lower.map(|inner| PyPositionSize { inner })
    }
    #[setter]
    fn set_incoming_lower(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.incoming_lower = value.map(parse_position_size_input).transpose()?;
        Ok(())
    }

    fn __repr__(&self) -> String {
        format!(
            "AccountAdjustmentBounds(balance_upper={:?}, balance_lower={:?}, held_upper={:?}, held_lower={:?}, incoming_upper={:?}, incoming_lower={:?})",
            self.balance_upper().map(|v| v.inner.to_string()),
            self.balance_lower().map(|v| v.inner.to_string()),
            self.held_upper().map(|v| v.inner.to_string()),
            self.held_lower().map(|v| v.inner.to_string()),
            self.incoming_upper().map(|v| v.inner.to_string()),
            self.incoming_lower().map(|v| v.inner.to_string()),
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

#[pyclass(name = "Request", module = "openpit.pretrade", unsendable)]
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

#[pyclass(name = "Reservation", module = "openpit.pretrade", unsendable)]
struct PyReservation {
    inner: RefCell<Option<PreTradeReservation>>,
}

#[pyclass(name = "Lock", module = "openpit.pretrade", subclass)]
#[derive(Clone, Default)]
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
            inner: reservation.lock().clone(),
        })
    }

    fn account_adjustments(&self) -> PyResult<Vec<PyAccountAdjustmentOutcome>> {
        let reservation_ref = self.inner.borrow();
        let reservation = reservation_ref
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err("reservation has already been finalized"))?;
        Ok(reservation
            .account_adjustments()
            .iter()
            .map(convert_adjustment_outcome)
            .collect())
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
    #[pyo3(signature = (entries = None))]
    fn new(entries: Option<&Bound<'_, PyAny>>) -> PyResult<Self> {
        let Some(source) = entries else {
            return Ok(Self::default());
        };
        if let Ok(other) = source.extract::<PyRef<'_, PyPreTradeLock>>() {
            return Ok(Self {
                inner: other.inner.clone(),
            });
        }
        let mut lock = PreTradeLock::new();
        push_pairs_into_lock(&mut lock, source)?;
        Ok(Self { inner: lock })
    }

    fn push(
        &mut self,
        policy_group_id: &Bound<'_, PyAny>,
        price: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        self.inner.push(
            parse_policy_group_id_input(policy_group_id)?,
            parse_price_input(price)?,
        );
        Ok(())
    }

    fn push_many(
        &mut self,
        policy_group_id: &Bound<'_, PyAny>,
        prices: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let policy_group_id = parse_policy_group_id_input(policy_group_id)?;
        let iterator = prices
            .try_iter()
            .map_err(|_| PyTypeError::new_err("prices must be an iterable of Price values"))?;
        let mut parsed = Vec::new();
        for item in iterator {
            parsed.push(parse_price_input(&item?)?);
        }
        self.inner.push_many(policy_group_id, parsed);
        Ok(())
    }

    fn extend(&mut self, entries: &Bound<'_, PyAny>) -> PyResult<()> {
        push_pairs_into_lock(&mut self.inner, entries)
    }

    fn merge(&mut self, other: &PyPreTradeLock) {
        self.inner.merge(&other.inner);
    }

    fn __len__(&self) -> usize {
        self.inner.len()
    }

    fn prices_of(&self, policy_group_id: &Bound<'_, PyAny>) -> PyResult<Vec<PyPrice>> {
        let policy_group_id = parse_policy_group_id_input(policy_group_id)?;
        Ok(self
            .inner
            .prices_of(policy_group_id)
            .map(|inner| PyPrice { inner })
            .collect())
    }

    fn entries(&self) -> Vec<(u16, PyPrice)> {
        self.inner
            .entries()
            .map(|(policy_group_id, price)| (policy_group_id.value(), PyPrice { inner: price }))
            .collect()
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|error| PyValueError::new_err(format!("lock json encode failed: {error}")))
    }

    #[staticmethod]
    fn from_json(text: &str) -> PyResult<Self> {
        let inner: PreTradeLock = serde_json::from_str(text)
            .map_err(|error| PyValueError::new_err(format!("lock json decode failed: {error}")))?;
        Ok(Self { inner })
    }

    fn to_msgpack<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let bytes = rmp_serde::to_vec(&self.inner).map_err(|error| {
            PyValueError::new_err(format!("lock msgpack encode failed: {error}"))
        })?;
        Ok(PyBytes::new(py, &bytes))
    }

    #[staticmethod]
    fn from_msgpack(data: &[u8]) -> PyResult<Self> {
        let inner: PreTradeLock = rmp_serde::from_slice(data).map_err(|error| {
            PyValueError::new_err(format!("lock msgpack decode failed: {error}"))
        })?;
        Ok(Self { inner })
    }

    fn to_cbor<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let mut buffer = Vec::new();
        ciborium::ser::into_writer(&self.inner, &mut buffer)
            .map_err(|error| PyValueError::new_err(format!("lock cbor encode failed: {error}")))?;
        Ok(PyBytes::new(py, &buffer))
    }

    #[staticmethod]
    fn from_cbor(data: &[u8]) -> PyResult<Self> {
        let inner: PreTradeLock = ciborium::de::from_reader(data)
            .map_err(|error| PyValueError::new_err(format!("lock cbor decode failed: {error}")))?;
        Ok(Self { inner })
    }

    fn __richcmp__(&self, other: &Self, op: CompareOp) -> PyResult<bool> {
        match op {
            CompareOp::Eq => Ok(self.inner == other.inner),
            CompareOp::Ne => Ok(self.inner != other.inner),
            _ => Err(PyTypeError::new_err("Lock supports only == and !=")),
        }
    }

    fn __repr__(&self) -> String {
        let mut out = String::from("Lock([");
        let mut first = true;
        for (policy_group_id, price) in self.inner.entries() {
            if !first {
                out.push_str(", ");
            }
            first = false;
            out.push_str(&format!("({}, {})", policy_group_id.value(), price));
        }
        out.push_str("])");
        out
    }
}

fn parse_policy_group_id_input(value: &Bound<'_, PyAny>) -> PyResult<PolicyGroupId> {
    let raw: u16 = value.extract().map_err(|_| {
        PyValueError::new_err("policy_group_id must be an integer in range 0..=65535")
    })?;
    Ok(PolicyGroupId::new(raw))
}

fn push_pairs_into_lock(lock: &mut PreTradeLock, entries: &Bound<'_, PyAny>) -> PyResult<()> {
    let iterator = entries.try_iter().map_err(|_| {
        PyTypeError::new_err("entries must be an iterable of (policy_group_id, price) pairs")
    })?;
    for item in iterator {
        let item = item?;
        let (policy_group_id, price): (Bound<'_, PyAny>, Bound<'_, PyAny>) =
            item.extract().map_err(|_| {
                PyTypeError::new_err(
                    "each entry must be a (policy_group_id, price) two-element tuple",
                )
            })?;
        lock.push(
            parse_policy_group_id_input(&policy_group_id)?,
            parse_price_input(&price)?,
        );
    }
    Ok(())
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
                PyModule::import(py, "openpit.param")?
                    .getattr("Side")?
                    .call1((side_name(side),))
                    .map(|obj| obj.unbind())
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
            lock: lock.extract::<PyRef<'_, PyPreTradeLock>>()?.inner.clone(),
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
        PyPreTradeLock {
            inner: self.lock.clone(),
        }
    }

    #[setter]
    fn set_lock(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.lock = value.extract::<PyRef<'_, PyPreTradeLock>>()?.inner.clone();
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
                PyModule::import(py, "openpit.param")?
                    .getattr("PositionEffect")?
                    .call1((position_effect_name(effect),))
                    .map(|obj| obj.unbind())
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
                PyModule::import(py, "openpit.param")?
                    .getattr("PositionSide")?
                    .call1((position_side_name(side),))
                    .map(|obj| obj.unbind())
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

#[pyclass(name = "ExecutionReport", module = "openpit.core", subclass)]
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

#[pyclass(name = "AccountBlock", module = "openpit.pretrade")]
#[derive(Clone)]
struct PyAccountBlock {
    code: String,
    policy: String,
    reason: String,
    details: String,
    user_data: u64,
}

#[pymethods]
impl PyAccountBlock {
    #[new]
    #[pyo3(signature = (*, policy, code, reason, details, user_data = 0))]
    fn new(
        policy: &str,
        code: &str,
        reason: &str,
        details: &str,
        user_data: u64,
    ) -> PyResult<Self> {
        parse_reject_code(code)?;
        Ok(Self {
            code: code.to_owned(),
            policy: policy.to_owned(),
            reason: reason.to_owned(),
            details: details.to_owned(),
            user_data,
        })
    }

    #[getter]
    fn code(&self) -> String {
        self.code.clone()
    }

    #[getter]
    fn policy(&self) -> String {
        self.policy.clone()
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
    fn user_data(&self) -> u64 {
        self.user_data
    }

    fn __repr__(&self) -> String {
        format!(
            "AccountBlock(code={:?}, policy={:?}, reason={:?}, details={:?}, user_data={})",
            self.code, self.policy, self.reason, self.details, self.user_data
        )
    }
}

fn convert_account_block(block: &openpit::pretrade::AccountBlock) -> PyAccountBlock {
    PyAccountBlock {
        code: block.code.as_str().to_owned(),
        policy: block.policy.clone(),
        reason: block.reason.clone(),
        details: block.details.clone(),
        user_data: block.user_data as u64,
    }
}

#[pyclass(name = "PostTradeResult", module = "openpit.pretrade")]
#[derive(Clone)]
struct PyPostTradeResult {
    inner: PostTradeResult,
}

#[pymethods]
impl PyPostTradeResult {
    #[new]
    #[pyo3(signature = (*, account_blocks = None, account_adjustments = None))]
    fn new(
        account_blocks: Option<&Bound<'_, PyAny>>,
        account_adjustments: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let blocks = match account_blocks {
            Some(value) => parse_account_block_list(value)?,
            None => Vec::new(),
        };
        let adjustments = match account_adjustments {
            Some(value) => parse_account_adjustment_outcome_list(value)?,
            None => Vec::new(),
        };
        Ok(Self {
            inner: PostTradeResult {
                account_blocks: blocks,
                account_adjustments: adjustments,
            },
        })
    }

    fn __repr__(&self) -> String {
        format!(
            "PostTradeResult(account_blocks={}, account_adjustments={})",
            self.inner.account_blocks.len(),
            self.inner.account_adjustments.len()
        )
    }

    #[getter]
    fn account_blocks(&self) -> Vec<PyAccountBlock> {
        self.inner
            .account_blocks
            .iter()
            .map(convert_account_block)
            .collect()
    }

    #[getter]
    fn account_adjustments(&self) -> Vec<PyAccountAdjustmentOutcome> {
        self.inner
            .account_adjustments
            .iter()
            .map(convert_adjustment_outcome)
            .collect()
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

fn parse_instrument_input(value: &Bound<'_, PyAny>) -> PyResult<Instrument> {
    if let Ok(v) = value.extract::<PyRef<'_, PyInstrument>>() {
        return Ok(v.inner.clone());
    }
    if let Ok(inner) = value.getattr("_inner") {
        if let Ok(v) = inner.extract::<PyRef<'_, PyInstrument>>() {
            return Ok(v.inner.clone());
        }
    }
    let underlying_asset = value
        .getattr("underlying_asset")
        .map_err(|_| PyTypeError::new_err("instrument must be openpit.core.Instrument"))?;
    let settlement_asset = value
        .getattr("settlement_asset")
        .map_err(|_| PyTypeError::new_err("instrument must be openpit.core.Instrument"))?;
    Ok(Instrument::new(
        parse_asset_input(&underlying_asset)?,
        parse_asset_input(&settlement_asset)?,
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
        Side::Buy => "BUY",
        Side::Sell => "SELL",
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
        PositionSide::Long => "LONG",
        PositionSide::Short => "SHORT",
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
        PositionEffect::Open => "OPEN",
        PositionEffect::Close => "CLOSE",
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
    let decimal_mod = PyModule::import(py, "decimal")?;
    let decimal_cls = decimal_mod.getattr("Decimal")?;
    let text = value.to_string();
    Ok(decimal_cls.call1((text,))?.unbind())
}

fn extract_python_decimal_string(value: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    let py = value.py();
    let decimal_mod = PyModule::import(py, "decimal")?;
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

fn convert_outcome_amount(value: &OutcomeAmount) -> PyOutcomeAmount {
    PyOutcomeAmount {
        delta: value.delta,
        absolute: value.absolute,
    }
}

fn convert_outcome_entry(value: &AccountOutcomeEntry) -> PyAccountOutcomeEntry {
    PyAccountOutcomeEntry {
        asset: value.asset.clone(),
        balance: value.balance.as_ref().map(convert_outcome_amount),
        held: value.held.as_ref().map(convert_outcome_amount),
        incoming: value.incoming.as_ref().map(convert_outcome_amount),
    }
}

fn convert_adjustment_outcome(value: &AccountAdjustmentOutcome) -> PyAccountAdjustmentOutcome {
    PyAccountAdjustmentOutcome {
        policy_group_id: value.policy_group_id,
        entry: convert_outcome_entry(&value.entry),
    }
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
    module.add("RejectError", py.get_type::<RejectError>())?;
    module.add("ParamError", py.get_type::<ParamError>())?;
    module.add("MarketDataError", py.get_type::<MarketDataError>())?;
    module.add("UnknownInstrument", py.get_type::<UnknownInstrument>())?;
    module.add("QuoteUnavailable", py.get_type::<QuoteUnavailable>())?;
    module.add("AlreadyRegistered", py.get_type::<AlreadyRegistered>())?;
    module.add("RegistrationError", py.get_type::<RegistrationError>())?;
    module.add("UnknownInstrumentId", py.get_type::<UnknownInstrumentId>())?;
    module.add(
        "AccountGroupRegistrationError",
        py.get_type::<AccountGroupRegistrationError>(),
    )?;
    module.add("AccountBlockError", py.get_type::<AccountBlockError>())?;
    module.add(
        "PolicyConfigureError",
        py.get_type::<PolicyConfigureError>(),
    )?;
    module.add_class::<PyConfigureErrorKind>()?;
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
    module.add("_DEFAULT_POLICY_GROUP_ID", DEFAULT_POLICY_GROUP_ID.value())?;
    module.add("_LEVERAGE_SCALE", Leverage::SCALE)?;
    module.add("_LEVERAGE_MIN", Leverage::MIN)?;
    module.add("_LEVERAGE_MAX", Leverage::MAX)?;
    module.add("_LEVERAGE_STEP", Leverage::STEP)?;
    module.add_function(wrap_pyfunction!(_validate_asset, module)?)?;
    module.add_class::<PyAccountId>()?;
    module.add_class::<PyAccountGroupId>()?;
    module.add_class::<PyQuantity>()?;
    module.add_class::<PyPrice>()?;
    module.add_class::<PyTrade>()?;
    module.add_class::<PyPnl>()?;
    module.add_class::<PyFee>()?;
    module.add_class::<PyVolume>()?;
    module.add_class::<PyNotional>()?;
    module.add_class::<PyCashFlow>()?;
    module.add_class::<PyPositionSize>()?;
    module.add_class::<PyInstrumentId>()?;
    module.add_class::<PyQuoteTtl>()?;
    module.add_class::<PyQuote>()?;
    module.add_class::<PyQuoteResolution>()?;
    module.add_class::<PyMarketDataService>()?;
    module.add_class::<PyMarketDataBuilder>()?;
    module.add_class::<PyTradeAmount>()?;
    module.add_class::<PyAdjustmentAmount>()?;
    module.add_class::<PyLeverage>()?;
    module.add_class::<PyEngine>()?; // "Engine"
    module.add_class::<PyAccounts>()?;
    module.add_class::<PyConfigurator>()?;
    module.add_class::<PyReject>()?;
    module.add_class::<PyAccountBlock>()?;
    module.add_class::<PyStartPreTradeResult>()?;
    module.add_class::<PyExecuteResult>()?;
    module.add_class::<PyAccountAdjustmentBatchResult>()?;
    module.add_class::<PyOutcomeAmount>()?;
    module.add_class::<PyAccountOutcomeEntry>()?;
    module.add_class::<PyAccountAdjustmentOutcome>()?;
    module.add_class::<PyEngineBuilder>()?;
    module.add_class::<PySyncedEngineBuilder>()?;
    module.add_class::<PyReadyEngineBuilder>()?;
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
    module.add_class::<PyOrderSizeLimit>()?;
    module.add_class::<PyPreTradeContext>()?;
    module.add_class::<PyAccountAdjustmentContext>()?;
    module.add_class::<PyPostTradeContext>()?;
    module.add_class::<PyAccountControl>()?;
    Ok(())
}

#[cfg(test)]
mod field_access_tests {
    use super::*;
    use openpit::{
        HasAccountId, HasFee, HasInstrument, HasPnl, HasSide, HasTradeAmount,
        RequestFieldAccessError,
    };
    use pyo3::types::PyList;
    use std::sync::Once;

    fn ensure_python_initialized() {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            pyo3::prepare_freethreaded_python();
            // PyO3 embeds the interpreter against the base CPython install,
            // not the virtualenv `PYO3_PYTHON` points at, and the embedded
            // binary has no `pyvenv.cfg`, so `site` never resolves the venv
            // prefix. As a result the venv `site-packages` (and the
            // `maturin develop` `.pth` that puts the editable `openpit`
            // package on the path) are absent from the embedded `sys.path`.
            // Query the venv interpreter for its effective `sys.path` and
            // prepend the missing entries so `import openpit` resolves
            // regardless of how `cargo test` was invoked.
            if let Err(error) = Python::with_gil(prepend_venv_sys_path) {
                panic!("failed to configure venv sys.path for embedded python: {error}");
            }
        });
    }

    fn prepend_venv_sys_path(py: Python<'_>) -> PyResult<()> {
        let interpreter = option_env!("PYO3_PYTHON").unwrap_or("python3");
        let output = std::process::Command::new(interpreter)
            .args(["-c", "import sys; print('\\n'.join(sys.path))"])
            .output()
            .map_err(|error| {
                PyRuntimeError::new_err(format!(
                    "failed to run venv interpreter {interpreter:?}: {error}"
                ))
            })?;
        if !output.status.success() {
            return Err(PyRuntimeError::new_err(format!(
                "venv interpreter {interpreter:?} exited with {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr),
            )));
        }
        let venv_path = String::from_utf8_lossy(&output.stdout);

        let sys = py.import("sys")?;
        let path = sys.getattr("path")?.downcast_into::<PyList>()?;
        let existing: Vec<String> = path.extract()?;
        // Preserve the venv ordering while prepending only entries that are
        // not already present, so the editable package directory wins without
        // duplicating the embedded interpreter's own search paths.
        for entry in venv_path.lines().rev() {
            let entry = entry.trim();
            if entry.is_empty() || existing.iter().any(|present| present == entry) {
                continue;
            }
            path.insert(0, entry)?;
        }
        Ok(())
    }

    fn order_without_operation() -> Order {
        ensure_python_initialized();
        Python::with_gil(|py| {
            openpit_interop::RequestWithPayload::new(
                openpit_interop::Order {
                    operation: OrderOperationAccess::Absent,
                    position: OrderPositionAccess::Absent,
                    margin: OrderMarginAccess::Absent,
                },
                py.None(),
            )
        })
    }

    fn report_without_groups() -> ExecutionReport {
        ensure_python_initialized();
        Python::with_gil(|py| {
            openpit_interop::RequestWithPayload::new(
                openpit_interop::ExecutionReport {
                    operation: ExecutionReportOperationAccess::Absent,
                    financial_impact: FinancialImpactAccess::Absent,
                    fill: ExecutionReportFillAccess::Absent,
                    position_impact: ExecutionReportPositionImpactAccess::Absent,
                },
                py.None(),
            )
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
            let policy_module = PyModule::from_code(
                py,
                cr#"
from types import SimpleNamespace

class StartCheck:
    def __init__(self):
        self.name = "PythonAdmissionCheck"

    def check_pre_trade_start(self, ctx, order):
        return None

    def perform_pre_trade_check(self, ctx, order):
        return SimpleNamespace(rejects=[], mutations=[])

    def apply_execution_report(self, ctx, report):
        return False

    def apply_account_adjustment(self, ctx, account_id, adjustment):
        return None

class ExecutionCheck:
    def __init__(self):
        self.name = "PythonExecutionCheck"

    def check_pre_trade_start(self, ctx, order):
        return None

    def perform_pre_trade_check(self, ctx, order):
        mutation = SimpleNamespace(commit=lambda: None, rollback=lambda: None)
        return SimpleNamespace(rejects=[], mutations=[mutation])

    def apply_execution_report(self, ctx, report):
        return False

    def apply_account_adjustment(self, ctx, account_id, adjustment):
        return None

class AdjustmentCheck:
    def __init__(self):
        self.name = "PythonAccountCheck"

    def check_pre_trade_start(self, ctx, order):
        return None

    def perform_pre_trade_check(self, ctx, order):
        return SimpleNamespace(rejects=[], mutations=[])

    def apply_execution_report(self, ctx, report):
        return False

    def apply_account_adjustment(self, ctx, account_id, adjustment):
        return None
"#,
                c"test_python_policies.py",
                c"test_python_policies",
            )?;

            let start_check = policy_module.getattr("StartCheck")?.call0()?;
            let execution_check = policy_module.getattr("ExecutionCheck")?.call0()?;
            let adjustment_check = policy_module.getattr("AdjustmentCheck")?.call0()?;

            let builder = PyReadyEngineBuilder::new(PySyncPolicy::None);

            let ov_policy = make_order_validation_start_check(DEFAULT_POLICY_GROUP_ID);
            builder.add_policy(ov_policy)?;

            let policies_module = py.import("openpit.pretrade.policies")?;
            let pnl_lower_bound = Py::new(
                py,
                PyPnl {
                    inner: Pnl::from_str("-500").expect("pnl must be valid"),
                },
            )?;
            let pnl_barrier_kwargs = PyDict::new(py);
            pnl_barrier_kwargs.set_item("settlement_asset", "USD")?;
            pnl_barrier_kwargs.set_item("lower_bound", pnl_lower_bound)?;
            let pnl_barrier_obj = policies_module
                .getattr("PnlBoundsBrokerBarrier")?
                .call((), Some(&pnl_barrier_kwargs))?;
            let pnl_policy = {
                let state = builder.state.borrow();
                let storage_builder = state
                    .as_ref()
                    .expect("builder must be available")
                    .storage_builder();
                make_pnl_killswitch_policy(
                    storage_builder,
                    DEFAULT_POLICY_GROUP_ID,
                    vec![pnl_barrier_obj],
                    vec![],
                )?
            };
            builder.add_configurable_pnl_killswitch(pnl_policy)?;

            let rl_policy = {
                let state = builder.state.borrow();
                let storage_builder = state
                    .as_ref()
                    .expect("builder must be available")
                    .storage_builder();
                make_rate_limit_policy(
                    storage_builder,
                    DEFAULT_POLICY_GROUP_ID,
                    Some((100, 1_000)),
                    vec![],
                    vec![],
                    vec![],
                )?
            };
            builder.add_configurable_rate_limit(rl_policy)?;

            let size_limit = PyOrderSizeLimit {
                max_quantity: "1000".to_owned(),
                max_notional: "1000000".to_owned(),
            };
            let size_limit_py = Py::new(py, size_limit)?;
            let sl_policy = make_order_size_limit_policy(
                DEFAULT_POLICY_GROUP_ID,
                None,
                vec![(size_limit_py.bind(py).borrow(), "USD".to_owned())],
                vec![],
            )?;
            builder.add_configurable_order_size_limit(sl_policy)?;
            builder.push_policy(&start_check)?;
            builder.push_policy(&execution_check)?;
            builder.push_policy(&adjustment_check)?;

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
                let reservation_borrow = reservation_ref.inner.borrow();
                let lock = reservation_borrow
                    .as_ref()
                    .expect("reservation must exist")
                    .lock();
                assert!(lock.entries().next().is_none());
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
            let _ = engine.apply_execution_report(py, &report.bind(py).clone().into_any())?;

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
            let adjustments = PyList::new(py, [adjustment.bind(py).clone().into_any()])?;
            let batch = engine.apply_account_adjustment(
                py,
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
