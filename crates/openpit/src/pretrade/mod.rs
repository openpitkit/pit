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

//! Pre-trade pipeline types and extension points.
//!
//! This module contains the request lifecycle used by Pit before an order is
//! sent to an external execution system:
//!
//! - [`PreTradePolicy`] models start-stage, main-stage, post-trade, and
//!   account-adjustment policy hooks;
//! - [`PreTradeRequest`] is the single-use handle returned after start-stage success;
//! - [`PreTradeReservation`] is the finalizable handle for reserved state;
//! - [`PreTradeDryRunReport`] is the inert verdict of a non-mutating dry-run.
//!
//! Custom controls typically start from the policy traits plus [`PreTradeContext`].

mod configurable;
mod context;
mod dry_run_report;
pub(crate) mod handle;
pub mod holdings;
mod lock;
pub mod policies;
mod policy;
mod policy_result;
mod post_trade_context;
mod post_trade_result;
mod reject;
mod request;
mod reservation;
pub(crate) mod start_pre_trade_time;

pub(crate) use configurable::ConfigurablePolicy;
pub use context::PreTradeContext;
pub use dry_run_report::PreTradeDryRunReport;
pub use lock::PreTradeLock;
pub use policies::{
    SpotFundsConfigError, SpotFundsLimitMode, SpotFundsMarketData, SpotFundsOverride,
    SpotFundsOverrideTarget, SpotFundsPricingSource,
};
pub use policy::{PolicyGroupId, PreTradePolicy, DEFAULT_POLICY_GROUP_ID};
pub use policy_result::PolicyPreTradeResult;
pub use post_trade_context::PostTradeContext;
pub use post_trade_result::PostTradeResult;
pub use reject::{AccountBlock, Reject, RejectCode, RejectScope, Rejects};
pub use request::PreTradeRequest;
pub use reservation::PreTradeReservation;
