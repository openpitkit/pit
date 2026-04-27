//! Runtime interoperability layer for Pit language bindings.
//!
//! Rust users verify policy-to-order compatibility at compile time through
//! `Has*` capability traits. Language bindings (Python, C++, WASM, Go, C#,
//! Java) represent orders and execution reports with all-optional groups
//! and cannot rely on compile-time checks.
//!
//! This crate provides `Populated*` / `Absent*` wrapper types and `*Access`
//! enums for each optional group in the domain model. Each `*Access` enum
//! implements the relevant `Has*` traits: `Populated` delegates to the
//! stored data, `Absent` returns `Err(RequestFieldAccessError)` for required
//! fields and `Ok(None)` for optional fields. Missing required fields surface
//! as `Reject(MissingRequiredField)` inside the policy that first needs them.

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

pub mod account_adjustment_amount;
pub mod account_adjustment_bounds;
pub mod account_adjustment_operation;
pub mod execution_report_fill;
pub mod execution_report_operation;
pub mod execution_report_position_impact;
pub mod financial_impact;
pub mod order_margin;
pub mod order_operation;
pub mod order_position;

pub use account_adjustment_amount::AccountAdjustmentAmountAccess;
pub use account_adjustment_bounds::AccountAdjustmentBoundsAccess;
pub use account_adjustment_operation::{
    AccountAdjustmentOperationAccess, PopulatedAccountAdjustmentOperation,
};
pub use execution_report_fill::{ExecutionReportFillAccess, PopulatedExecutionReportFill};
pub use execution_report_operation::{
    ExecutionReportOperationAccess, PopulatedExecutionReportOperation,
};
pub use execution_report_position_impact::{
    ExecutionReportPositionImpactAccess, PopulatedExecutionReportPositionImpact,
};
pub use financial_impact::{FinancialImpactAccess, PopulatedFinancialImpact};
pub use order_margin::{OrderMarginAccess, PopulatedOrderMargin};
pub use order_operation::{OrderOperationAccess, PopulatedOrderOperation};
pub use order_position::{OrderPositionAccess, PopulatedOrderPosition};
