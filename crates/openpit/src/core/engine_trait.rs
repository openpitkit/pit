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

//! Engine type aggregate.

use std::marker::PhantomData;

use super::sync_mode::SyncMode;

/// Aggregates the four type-level choices of an [`Engine`](crate::Engine)
/// instance into a single trait, so that engine code carries one generic
/// parameter instead of four.
///
/// The trait is open. The engine builder chain composes it automatically via
/// [`EngineTraitOf`] - end users rarely implement `EngineTrait` directly.
/// Implement it manually only when wiring a non-standard engine type into
/// another layer.
pub trait EngineTrait: 'static {
    /// Order contract type used by `start_pre_trade`.
    type Order: 'static;
    /// Execution-report contract type used by `apply_execution_report`.
    type ExecutionReport: 'static;
    /// Account-adjustment contract type used by `apply_account_adjustment`.
    type AccountAdjustment: 'static;
    /// Synchronization mode (see [`SyncMode`]).
    type Sync: SyncMode;
}

/// Concrete [`EngineTrait`] implementation composed from the four individual
/// type choices.
///
/// The engine builder chain produces this type at `build()` time so that the
/// resulting `Engine` value has a single, fully-determined [`EngineTrait`]
/// parameter.
pub struct EngineTraitOf<Order, ExecutionReport, AccountAdjustment, Sync>(
    PhantomData<(Order, ExecutionReport, AccountAdjustment, Sync)>,
);

impl<Order, ExecutionReport, AccountAdjustment, Sync> EngineTrait
    for EngineTraitOf<Order, ExecutionReport, AccountAdjustment, Sync>
where
    Order: 'static,
    ExecutionReport: 'static,
    AccountAdjustment: 'static,
    Sync: SyncMode,
{
    type Order = Order;
    type ExecutionReport = ExecutionReport;
    type AccountAdjustment = AccountAdjustment;
    type Sync = Sync;
}
