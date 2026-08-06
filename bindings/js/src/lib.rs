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

//! JavaScript/WASM bindings for the `openpit` pre-trade risk engine.
//!
//! This crate binds the `openpit` core directly and compiles to
//! `wasm32-unknown-unknown`.
//! Decimal values cross the boundary as canonical decimal strings; every
//! exported method carries an explicit camelCase `js_name` because
//! `wasm-bindgen` does not rename automatically.
//!
//! The full SDK surface is present: value types, identifiers, enums, the
//! decimal input contract, the JS error model, the order/report/adjustment
//! payloads, the pre-trade lock, market data, the staged engine builder and
//! account registry, the custom JS policy adapter, and the five builtin policy
//! builders (order validation, order-size limit, rate limit, P&L-bounds
//! kill-switch, and spot funds).

mod account_adjustment;
mod configure;
mod context;
mod decimal;
mod domain;
mod engine;
mod error;
mod execution_report;
mod lock;
mod marketdata;
mod order;
mod outcome;
mod param;
mod policy;
mod reference_book;
mod reject;
mod result;

pub use decimal::{parse_decimal_input, parse_rounding_strategy};
pub use error::{param_error_to_js, ErrorKind};
pub use param::enums::{JsPositionEffect, JsPositionMode, JsPositionSide, JsSide};
pub use param::ids::{JsAccountGroupId, JsAccountId, JsInstrumentId};
pub use param::leverage::JsLeverage;
pub use param::monetary_amount::JsMonetaryAmount;
pub use param::value_types::{
    JsCashFlow, JsFee, JsNotional, JsPnl, JsPositionSize, JsPrice, JsQuantity, JsVolume,
};

pub use account_adjustment::{
    JsAccountAdjustment, JsAccountAdjustmentAccountPnlOperation, JsAccountAdjustmentAmount,
    JsAccountAdjustmentBalanceOperation, JsAccountAdjustmentBounds,
    JsAccountAdjustmentPositionOperation, JsAdjustmentAmount,
};
pub use configure::JsConfigurator;
pub use execution_report::{
    JsExecutionReport, JsExecutionReportFillDetails, JsExecutionReportOperation,
    JsExecutionReportPositionImpact, JsFinancialImpact, JsTrade,
};
pub use lock::JsLock;
pub use marketdata::{
    JsInstrument, JsMarketDataBuilder, JsMarketDataService, JsQuote, JsQuoteResolution, JsQuoteTtl,
};
pub use order::{JsOrder, JsOrderMargin, JsOrderOperation, JsOrderPosition, JsTradeAmount};
pub use outcome::{
    JsAccountAdjustmentOutcome, JsAccountOutcomeEntry, JsAccountPnlOutcome, JsOutcomeAmount,
    JsPnlHaltReason, JsPnlOutcome, JsPnlOutcomeAmount,
};
pub use reference_book::{JsReferenceBook, JsSettlementLag, JsSettlementScheme, JsSettlementUnit};
pub use reject::{is_reject_code_evaluation_failure, JsAccountBlock, JsReject};

pub use context::{JsAccountAdjustmentContext, JsAccountControl, JsContext, JsPostTradeContext};
pub use engine::{JsAccounts, JsEngine, JsEngineBuilder, JsReadyEngineBuilder};
pub use result::{
    JsAccountAdjustmentBatchResult, JsAccountBlockOutcome, JsAccountBlockOutcomes,
    JsDropCopyOperation, JsDropCopyResult, JsDryRunReport, JsExecuteResult,
    JsPolicyConfigurationResult, JsPostTradeResult, JsRequest, JsReservation, JsStartResult,
};

pub use policy::order_size_limit::{
    build_order_size_limit, JsOrderSizeAccountAssetBarrier, JsOrderSizeAssetBarrier,
    JsOrderSizeBrokerBarrier, JsOrderSizeLimit, JsOrderSizeLimitBuilder,
};
pub use policy::order_validation::{build_order_validation, JsOrderValidationBuilder};
pub use policy::pnl_killswitch::{
    build_pnl_bounds_killswitch, JsPnlBoundsAccountAssetBarrier,
    JsPnlBoundsAccountAssetBarrierUpdate, JsPnlBoundsBrokerBarrier, JsPnlBoundsKillswitchBuilder,
};
pub use policy::rate_limit::{
    build_rate_limit, JsRateLimit, JsRateLimitAccountAssetBarrier, JsRateLimitAccountBarrier,
    JsRateLimitAssetBarrier, JsRateLimitBrokerBarrier, JsRateLimitBuilder,
};
pub use policy::spot_funds::{
    build_spot_funds, build_spot_funds_pnl_bounds_killswitch, JsSpotFundsBuilder,
    JsSpotFundsOverride, JsSpotFundsPnlBoundsAccountBarrier,
    JsSpotFundsPnlBoundsAccountGroupBarrier, JsSpotFundsPnlBoundsBarrier,
    JsSpotFundsPnlBoundsKillswitchBuilder,
};

use wasm_bindgen::prelude::*;

/// WASM module entry point invoked by the generated glue on instantiation.
///
/// Installs the panic boundary so a Rust panic reaches JavaScript as an
/// `InternalError` instead of a bare wasm trap.
#[wasm_bindgen(start)]
pub fn start() {
    install_panic_boundary();
}

/// Routes Rust panics out of WebAssembly as idiomatic JavaScript errors.
///
/// `wasm32-unknown-unknown` has no unwinding runtime - the shipped `std` is
/// built with `panic = "abort"` and `catch_unwind` can never catch - so the
/// panic hook is the only place where a panic is still observable from Rust.
/// Throwing from it hands control to the JavaScript engine, which discards the
/// wasm frames of the failed call and runs the `finally` blocks of the
/// generated glue. The module stays instantiated only to return the same
/// `InternalError` from every guarded entry point; callers must reload it.
///
/// Rust destructors of the aborted call do not run, so binding state that
/// outlives a single call is reset here. Everything the failed operation was
/// mutating is left unspecified: the error reports a defect, it does not make
/// the operation recoverable.
///
/// Only the first panic of an instance can be converted at all: the runtime
/// marks its hook as entered and clears that mark only when the hook returns,
/// which this one never does, so it would treat a second panic as a panic
/// inside the hook and abort before reaching it. That is why the report poisons
/// the module rather than merely failing one call - every entry point that
/// could reach core state, and therefore another panic, refuses to run
/// afterwards. Callers must reload the module after an `InternalError`.
fn install_panic_boundary() {
    std::panic::set_hook(Box::new(|info| {
        #[cfg(feature = "console_error_panic_hook")]
        console_error_panic_hook::hook(info);
        let report = info.to_string();
        policy::report_panic(&report);
        wasm_bindgen::throw_val(error::internal_error(&report));
    }));
}
