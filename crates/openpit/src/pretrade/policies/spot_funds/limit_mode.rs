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
/// This enum is `Copy` and behaves like a small value type. It chooses between
/// gating a reservation on funds and merely accounting for it; it never encodes
/// the funds themselves, only the policy applied when a reservation would exceed
/// what is available.
///
/// The two modes differ only on the insufficiency path; the bookkeeping is
/// identical. In both modes the reservation is recorded against the account's
/// available balance, positions, average entry price and realized PnL evolve the
/// same way. The mode decides whether a shortfall is a hard reject
/// ([`Enforce`](Self::Enforce)) or a tracked overshoot
/// ([`TrackOnly`](Self::TrackOnly)).
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
}
