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

use super::reject::AccountBlock;

/// Aggregated post-trade processing result.
///
/// A non-empty `account_blocks` list means at least one policy entered a
/// blocked state. The engine merges blocks from all policies in registration
/// order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostTradeResult {
    /// Account blocks reported by policies.
    ///
    /// Non-empty when at least one policy entered a blocked state. The engine
    /// merges blocks from all policies in registration order.
    pub account_blocks: Vec<AccountBlock>,
}
