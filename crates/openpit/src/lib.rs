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
//!    [`pretrade::PreTradeRequest`].
//! 2. [`pretrade::PreTradeRequest::execute`] runs main-stage policies and returns a
//!    [`pretrade::PreTradeReservation`].
//! 3. [`pretrade::PreTradeReservation::commit`] or
//!    [`pretrade::PreTradeReservation::rollback`] finalizes reserved state.
//! 4. [`Engine::apply_execution_report`] feeds realized outcomes back into policies.
//! 5. [`Engine::apply_account_adjustment`] validates non-trade adjustment batches.
//!
//! The current crate scope is deliberately narrow: in-memory admission control,
//! exact decimal value types, and a small set of built-in start-stage
//! policies. Persistence, market connectivity, and thread synchronization stay
//! with the caller.

mod core;
pub mod marketdata;
pub mod param;
pub mod pretrade;
pub mod storage;
mod time;

pub use core::engine::{
    AccountAdjustmentBatchError, AccountSyncEngine, Engine, FullSyncEngine, LocalEngine,
};
pub use core::engine_builder::{
    EngineBuildError, EngineBuilder, IntoPolicyObject, ReadyEngineBuilder, SyncedEngineBuilder,
};
pub use core::{
    AccountAdjustmentAmount, AccountAdjustmentBalanceOperation, AccountAdjustmentBatchResult,
    AccountAdjustmentBounds, AccountAdjustmentContext, AccountAdjustmentOutcome,
    AccountAdjustmentPositionOperation, AccountKey, AccountKeyConstraint, AccountOutcomeEntry,
    ExecutionReportFillDetails, ExecutionReportOperation, ExecutionReportPositionImpact,
    FinancialImpact, HasAccountAdjustmentBalance, HasAccountAdjustmentBalanceAverageEntryPrice,
    HasAccountAdjustmentBalanceLowerBound, HasAccountAdjustmentBalanceRealizedPnl,
    HasAccountAdjustmentBalanceUpperBound, HasAccountAdjustmentHeld,
    HasAccountAdjustmentHeldLowerBound, HasAccountAdjustmentHeldUpperBound,
    HasAccountAdjustmentIncoming, HasAccountAdjustmentIncomingLowerBound,
    HasAccountAdjustmentIncomingUpperBound, HasAccountAdjustmentPositionLeverage, HasAccountId,
    HasAutoBorrow, HasAverageEntryPrice, HasBalanceAsset, HasClosePosition, HasCollateralAsset,
    HasExecutionReportFillFee, HasExecutionReportIsFinal, HasExecutionReportLastTrade,
    HasExecutionReportPositionEffect, HasExecutionReportPositionSide, HasFee, HasInstrument,
    HasLeavesQuantity, HasOrderCollateralAsset, HasOrderLeverage, HasOrderPositionSide,
    HasOrderPrice, HasPnl, HasPositionInstrument, HasPositionMode, HasPreTradeLock, HasReduceOnly,
    HasSide, HasTradeAmount, Instrument, InstrumentId, Mutation, Mutations, OrderMargin,
    OrderOperation, OrderPosition, OutcomeAmount, PnlOutcomeAmount, ReferenceBook,
    ReferenceBookRegistrationError, RequestFieldAccessError, SettlementLag, SettlementScheme,
    SettlementUnit, UnknownReferenceBookInstrumentId, WithAccountAdjustmentAmount,
    WithAccountAdjustmentBalanceOperation, WithAccountAdjustmentBounds,
    WithAccountAdjustmentPositionOperation, WithExecutionReportFillDetails,
    WithExecutionReportOperation, WithExecutionReportPositionImpact, WithFinancialImpact,
    WithOrderMargin, WithOrderOperation, WithOrderPosition,
};
pub use core::{
    AccountBlockError, AccountBlockHandle, AccountControl, AccountGroupError, AccountSync,
    AccountSyncHandle, AccountSyncHandleWeak, Accounts, Configurator, ConfigureError, EngineTrait,
    EngineTraitOf, FullSync, LocalSync, SyncMode,
};
pub use core::{PolicyGroupId, DEFAULT_POLICY_GROUP_ID};
pub use marketdata::{
    AccountInfo, AlreadyRegistered, LocalTtlGate, MarketDataBuilder, MarketDataError,
    MarketDataLock, MarketDataService, MarketDataSync, NoopLock, PushForError, Quote,
    QuoteResolution, QuoteTtl, RegistrationError, ServiceTtlGate, UnknownInstrumentId,
};
#[cfg(feature = "derive")]
pub use openpit_derive::RequestFields;
pub use param::{AdjustmentAmount, PositionMode};
pub use pretrade::PostTradeResult;
pub use pretrade::{
    SpotFundsConfigError, SpotFundsLimitMode, SpotFundsMarketData, SpotFundsOverride,
    SpotFundsOverrideTarget, SpotFundsPricingSource,
};
pub use storage::IndexFlag;
pub use storage::StorageBuilder;
