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

use super::{Mutations, Reject};
use crate::param::AccountId;

/// Policy contract for account-adjustment batch validation.
///
/// Account-adjustment policies run in [`crate::Engine::apply_account_adjustment`]
/// and validate each adjustment atomically before the caller applies any
/// external effects.
///
/// # Rollback safety
///
/// Account-adjustment policies run within a single engine borrow.
/// Intermediate state is never visible to external systems (venues, risk
/// aggregators), so rollback by absolute value is always safe.
///
/// This is different from pre-trade pipeline policies, where reserved
/// state may be observed externally between reservation creation and
/// finalization. Pre-trade policies should prefer delta-based rollback.
///
/// `A` is the account-adjustment contract type visible to policies.
///
/// # Examples
///
/// ```rust
/// use openpit::param::AccountId;
/// use openpit::pretrade::{AccountAdjustmentPolicy, Mutations, Reject, RejectCode, RejectScope};
///
/// struct PositiveAmountOnly;
///
/// impl AccountAdjustmentPolicy<i64> for PositiveAmountOnly {
///     fn name(&self) -> &'static str {
///         "PositiveAmountOnly"
///     }
///
///     fn apply_account_adjustment(&self, _account_id: AccountId, adjustment: &i64, _mutations: &mut Mutations) -> Result<(), Reject> {
///         if *adjustment < 0 {
///             return Err(Reject::new(
///                 self.name(),
///                 RejectScope::Order,
///                 RejectCode::Other,
///                 "negative adjustment is not allowed",
///                 "adjustment amount must be non-negative",
///             ));
///         }
///         Ok(())
///     }
/// }
/// ```
pub trait AccountAdjustmentPolicy<A> {
    /// Stable policy name.
    ///
    /// Policy names must be unique across all policies registered in the same
    /// engine instance.
    fn name(&self) -> &'static str;

    /// Validates a single account adjustment.
    ///
    /// `account_id` is the identifier passed to
    /// [`crate::Engine::apply_account_adjustment`].
    ///
    /// # Rollback safety
    ///
    /// In this account-adjustment pipeline, rollback by absolute value is
    /// safe because validation and mutation execution happen within a single
    /// engine borrow and no external system observes intermediate state.
    ///
    /// # Errors
    ///
    /// Returns [`Reject`] when the adjustment violates policy constraints.
    fn apply_account_adjustment(
        &self,
        account_id: AccountId,
        adjustment: &A,
        mutations: &mut Mutations,
    ) -> Result<(), Reject>;
}
