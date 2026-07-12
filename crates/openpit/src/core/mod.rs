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
pub(crate) mod account_control;
pub(crate) mod account_groups;
pub(crate) mod account_key;
pub(crate) mod account_outcome;
pub(crate) mod accounts;
pub(crate) mod configure;
pub(crate) mod engine;
pub(crate) mod engine_builder;
pub(crate) mod engine_trait;
pub(crate) mod execution_report;
pub(crate) mod instrument;
pub(crate) mod instrument_id;
pub(crate) mod mutation;
pub(crate) mod order;
pub(crate) mod reference_book;
pub(crate) mod request_trait;
pub(crate) mod sync_mode;

mod macros;

pub use account_adjustment::{
    AccountAdjustmentAmount, AccountAdjustmentBalanceOperation, AccountAdjustmentBounds,
    AccountAdjustmentPositionOperation, WithAccountAdjustmentAmount,
    WithAccountAdjustmentBalanceOperation, WithAccountAdjustmentBounds,
    WithAccountAdjustmentPositionOperation,
};
pub use account_adjustment_context::AccountAdjustmentContext;
pub(crate) use account_control::BlockedAccounts;
pub use account_control::{AccountBlockError, AccountBlockHandle, AccountControl};
pub use account_groups::AccountGroupError;
pub(crate) use account_groups::{AccountGroups, AccountGroupsHandle, GroupLookup};
pub use account_key::{AccountKey, AccountKeyConstraint};
pub use account_outcome::{
    AccountAdjustmentBatchResult, AccountAdjustmentOutcome, AccountOutcomeEntry, OutcomeAmount,
    PnlOutcomeAmount,
};
pub(crate) use accounts::AccountCurrencies;
pub use accounts::Accounts;
pub(crate) use configure::{ConfigEntry, ConfigRegistry};
pub use configure::{Configurator, ConfigureError};
pub use engine_trait::{EngineTrait, EngineTraitOf};
pub use execution_report::{
    ExecutionReportFillDetails, ExecutionReportOperation, ExecutionReportPositionImpact,
    FinancialImpact, WithExecutionReportFillDetails, WithExecutionReportOperation,
    WithExecutionReportPositionImpact, WithFinancialImpact,
};
pub use instrument::Instrument;
pub use instrument_id::InstrumentId;
pub use mutation::{Mutation, Mutations};
pub use order::{
    OrderMargin, OrderOperation, OrderPosition, WithOrderMargin, WithOrderOperation,
    WithOrderPosition,
};
pub use reference_book::{
    ReferenceBook, ReferenceBookRegistrationError, SettlementLag, SettlementScheme, SettlementUnit,
    UnknownReferenceBookInstrumentId,
};
pub use request_trait::{
    HasAccountAdjustmentBalance, HasAccountAdjustmentBalanceAverageEntryPrice,
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
    HasSide, HasTradeAmount, RequestFieldAccessError,
};
pub use sync_mode::{
    AccountSync, AccountSyncHandle, AccountSyncHandleWeak, FullSync, LocalSync, SyncMode,
};

/// Policy-group tag used when the caller does not assign one.
pub const DEFAULT_POLICY_GROUP_ID: PolicyGroupId = PolicyGroupId::new(0);

/// Opaque policy-group tag attached to an abstract business entity.
///
/// A single value may be shared across multiple business entity instances so the caller
/// can associate outcomes with a logical group rather than a specific
/// implementation. The value is assigned at policy construction time and
/// carried unchanged through every operation.
///
/// Use [`DEFAULT_POLICY_GROUP_ID`] when grouping is not needed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct PolicyGroupId(u16);

impl PolicyGroupId {
    /// Creates a `PolicyGroupId` from a raw `u16` value.
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    /// Returns the raw `u16` value.
    pub const fn value(self) -> u16 {
        self.0
    }
}
