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

//! Built-in pre-trade policy implementations.

pub mod order_size_limit;
pub mod order_validation;
pub mod pnl_bounds_killswitch;
pub mod rate_limit;

pub use order_size_limit::{OrderSizeLimit, OrderSizeLimitPolicy};
pub use order_validation::OrderValidationPolicy;
pub use pnl_bounds_killswitch::{
    PnlBoundsBarrier, PnlBoundsKillSwitchPolicy, PnlBoundsKillSwitchPolicyError,
};
pub use rate_limit::RateLimitPolicy;
