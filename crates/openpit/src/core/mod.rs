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

pub(crate) mod account_adjustment;
pub(crate) mod account_adjustment_context;
pub(crate) mod account_adjustment_policy;
pub(crate) mod engine;
pub(crate) mod execution_report;
pub(crate) mod instrument;
pub(crate) mod mutation;
pub(crate) mod order;
pub(crate) mod request_trait;

mod macros;

pub use account_adjustment::{
    AccountAdjustmentAmount, AccountAdjustmentBalanceOperation, AccountAdjustmentBounds,
    AccountAdjustmentPositionOperation, WithAccountAdjustmentAmount,
    WithAccountAdjustmentBalanceOperation, WithAccountAdjustmentBounds,
    WithAccountAdjustmentPositionOperation,
};
pub use account_adjustment_context::AccountAdjustmentContext;
pub use account_adjustment_policy::AccountAdjustmentPolicy;
pub use execution_report::{
    ExecutionReportFillDetails, ExecutionReportOperation, ExecutionReportPositionImpact,
    FinancialImpact, WithExecutionReportFillDetails, WithExecutionReportOperation,
    WithExecutionReportPositionImpact, WithFinancialImpact,
};
pub use instrument::Instrument;
pub use mutation::{Mutation, Mutations};
pub use order::{
    OrderMargin, OrderOperation, OrderPosition, WithOrderMargin, WithOrderOperation,
    WithOrderPosition,
};
pub use request_trait::{
    HasAccountAdjustmentBalanceAverageEntryPrice, HasAccountAdjustmentPending,
    HasAccountAdjustmentPendingLowerBound, HasAccountAdjustmentPendingUpperBound,
    HasAccountAdjustmentPositionLeverage, HasAccountAdjustmentReserved,
    HasAccountAdjustmentReservedLowerBound, HasAccountAdjustmentReservedUpperBound,
    HasAccountAdjustmentTotal, HasAccountAdjustmentTotalLowerBound,
    HasAccountAdjustmentTotalUpperBound, HasAccountId, HasAutoBorrow, HasAverageEntryPrice,
    HasBalanceAsset, HasClosePosition, HasCollateralAsset, HasExecutionReportIsFinal,
    HasExecutionReportLastTrade, HasExecutionReportPositionEffect, HasExecutionReportPositionSide,
    HasFee, HasInstrument, HasLeavesQuantity, HasLock, HasOrderCollateralAsset, HasOrderLeverage,
    HasOrderPositionSide, HasOrderPrice, HasPnl, HasPositionInstrument, HasPositionMode,
    HasReduceOnly, HasSide, HasTradeAmount, RequestFieldAccessError,
};
