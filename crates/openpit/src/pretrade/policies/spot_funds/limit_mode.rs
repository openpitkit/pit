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

//! Spot-funds limit mode value type.

/// Selects how the spot-funds control reacts to insufficient available funds.
///
/// This `Copy` value selects the policy applied when a reservation exceeds
/// available funds. The four modes have identical bookkeeping when funds are
/// sufficient. On a shortfall they reject, track the overshoot, or request
/// review with or without a hold. Positions, average entry price and realized
/// PnL evolve identically through execution reports in every mode.
///
/// # Examples
///
/// ```rust
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use openpit::pretrade::SpotFundsLimitMode;
///
/// assert_eq!(SpotFundsLimitMode::default(), SpotFundsLimitMode::Enforce);
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SpotFundsLimitMode {
    /// Reject a reservation when available funds are insufficient.
    ///
    /// This is the conventional pre-trade behavior: a reservation that would
    /// drive `available` below zero is refused with
    /// [`RejectCode::InsufficientFunds`](crate::pretrade::RejectCode::InsufficientFunds)
    /// and never recorded.
    #[default]
    Enforce,

    /// Account for the reservation without rejecting on insufficiency.
    ///
    /// The reservation is always recorded; `available` is allowed to go
    /// negative. Use this when the funds limit is observational - the engine
    /// keeps an accurate running balance but does not gate orders on it.
    /// Arithmetic overflow is still surfaced as an integrity guard and is not
    /// suppressed by this mode.
    TrackOnly,

    /// Request review on insufficient funds while recording the full hold.
    ///
    /// A shortfall drives `available` negative as in [`Self::TrackOnly`] and
    /// emits an `InsufficientFunds` mark with `RuleMode::ReviewWithHold`.
    /// Sufficient funds produce no mark. A confirmed order behaves as
    /// [`Self::TrackOnly`], without a mark; drop-copy always uses that mode.
    /// Arithmetic overflow remains a reject. A final cancel report releases
    /// the hold through the normal execution path.
    ReviewWithHold,

    /// Request review on insufficient funds without writing that leg or later
    /// reservation legs.
    ///
    /// A shortfall emits an `InsufficientFunds` mark with
    /// `RuleMode::ReviewWithoutHold`. Remaining checks use temporary holdings
    /// as in [`Self::TrackOnly`]; the engine rolls back earlier order mutations
    /// and returns a reservation with empty lock and outcomes. Sufficient funds
    /// produce a normal hold without a mark. Confirmed orders and drop-copy
    /// behave as [`Self::TrackOnly`], without a mark.
    /// Overflow remains a reject.
    ReviewWithoutHold,
}
