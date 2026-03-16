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

//! Embeddable pre-trade risk engine for trading systems.
//!
//! `openpit` focuses on the moment before an order leaves the application.
//! The crate provides:
//!
//! - [`Engine`] to coordinate pre-trade checks and post-trade feedback;
//! - [`param`] for typed financial values such as [`param::Price`] and
//!   [`param::Pnl`];
//! - [`pretrade`] for policy traits, rejects, deferred requests, and
//!   reservations.
//!
//! The pipeline is intentionally explicit:
//!
//! 1. [`Engine::start_pre_trade`] runs start-stage policies and returns a
//!    [`pretrade::Request`].
//! 2. [`pretrade::Request::execute`] runs main-stage policies and returns a
//!    [`pretrade::Reservation`].
//! 3. [`pretrade::Reservation::commit`] or
//!    [`pretrade::Reservation::rollback`] finalizes reserved state.
//! 4. [`Engine::apply_execution_report`] feeds realized outcomes back into policies.
//!
//! The current crate scope is deliberately narrow: in-memory admission control,
//! exact decimal value types, and a small set of built-in start-stage
//! policies. Persistence, market connectivity, and thread synchronization stay
//! with the caller.

mod core;
pub mod param;
pub mod pretrade;

pub use core::engine::{Engine, EngineBuildError, EngineBuilder};
pub use core::{
    ExecutionReportFillDetails, ExecutionReportOperation, ExecutionReportPositionImpact,
    FinancialImpact, HasAutoBorrow, HasClosePosition, HasExecutionReportIsTerminal,
    HasExecutionReportLastTrade, HasExecutionReportPositionEffect, HasExecutionReportPositionSide,
    HasFee, HasInstrument, HasLeavesQuantity, HasLock, HasOrderCollateralAsset, HasOrderLeverage,
    HasOrderPositionSide, HasOrderPrice, HasPnl, HasReduceOnly, HasSide, HasTradeAmount,
    Instrument, OrderMargin, OrderOperation, OrderPosition, WithExecutionReportFillDetails,
    WithExecutionReportOperation, WithExecutionReportPositionImpact, WithFinancialImpact,
    WithOrderMargin, WithOrderOperation, WithOrderPosition,
};
pub use pretrade::PostTradeResult;
