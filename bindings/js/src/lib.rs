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
    JsAccountAdjustment, JsAccountAdjustmentAmount, JsAccountAdjustmentBalanceOperation,
    JsAccountAdjustmentBounds, JsAccountAdjustmentPositionOperation, JsAdjustmentAmount,
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
    JsAccountAdjustmentOutcome, JsAccountOutcomeEntry, JsOutcomeAmount, JsPnlOutcomeAmount,
};
pub use reject::{JsAccountBlock, JsReject};

pub use context::{JsAccountAdjustmentContext, JsAccountControl, JsContext, JsPostTradeContext};
pub use engine::{JsAccounts, JsEngine, JsEngineBuilder, JsReadyEngineBuilder};
pub use result::{
    JsAccountAdjustmentBatchResult, JsDryRunReport, JsExecuteResult, JsPostTradeResult, JsRequest,
    JsReservation, JsStartResult,
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
    JsSpotFundsPnlBoundsAccountBarrierUpdate, JsSpotFundsPnlBoundsAccountGroupBarrier,
    JsSpotFundsPnlBoundsBarrier, JsSpotFundsPnlBoundsKillswitchBuilder,
};

use wasm_bindgen::prelude::*;

/// WASM module entry point invoked by the generated glue on instantiation.
///
/// Installs the panic hook only when the `console_error_panic_hook` feature is
/// enabled. In release builds (feature off) this is a no-op, keeping the panic
/// machinery and the extra dependency out of the shipped artifact.
#[wasm_bindgen(start)]
pub fn start() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}
