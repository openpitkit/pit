//! Runtime interoperability layer for Pit language bindings.
//!
//! Rust users verify policy-to-order compatibility at compile time through
//! `Has*` capability traits. Language bindings (Python, C++, WASM, Go, C#,
//! Java) represent orders and execution reports with all-Optional groups
//! and cannot rely on compile-time checks.
//!
//! This crate provides:
//!
//! - [`OrderGroupAccess`] and [`ExecutionReportGroupAccess`] — traits
//!   that binding-level types implement to report which Optional groups
//!   are populated.
//! - `Guarded*` policy wrappers — each built-in policy has a guarded
//!   counterpart that validates required groups at runtime. Missing groups
//!   produce a standard `Reject` with `RejectCode::MissingRequiredField`
//!   instead of a panic. When all groups are present, the wrapper
//!   delegates to the inner policy unchanged.
//!
//! Every built-in policy has a guard, including those that currently
//! require no order data. Empty guards are kept for API uniformity and
//! forward-compatibility — new trait bounds may appear in future versions.

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

mod access;
pub mod order_size_limit;
pub mod order_validation;
pub mod pnl_killswitch;
pub mod rate_limit;

pub use access::{ExecutionReportGroupAccess, OrderGroupAccess};
pub use order_size_limit::GuardedOrderSizeLimit;
pub use order_validation::GuardedOrderValidation;
pub use pnl_killswitch::GuardedPnlKillSwitch;
pub use rate_limit::GuardedRateLimit;
