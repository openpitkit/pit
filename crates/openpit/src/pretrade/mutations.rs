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

use crate::param::{Asset, Volume};

/// Closed set of state mutations available in pre-trade checks.
///
/// Policies register mutations via [`Mutations::push`]. The engine applies
/// commit mutations on reservation commit, and rollback mutations in reverse
/// order on reject or rollback.
///
/// The set is closed: user policies cannot introduce new variants.
///
/// # Examples
///
/// ```
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use openpit::param::{Asset, Volume};
/// use openpit::pretrade::RiskMutation;
///
/// let reserve = RiskMutation::ReserveNotional {
///     asset: Asset::new("USD")?,
///     amount: Volume::from_str("18500")?,
/// };
/// let toggle = RiskMutation::SetKillSwitch {
///     id: "daily_loss_limit",
///     enabled: true,
/// };
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RiskMutation {
    /// Reserve notional for a settlement asset.
    ///
    /// Applying this mutation overwrites any previously reserved value for the same
    /// asset (last-writer-wins). If multiple policies reserve notional for one asset,
    /// the last applied mutation takes effect.
    ReserveNotional { asset: Asset, amount: Volume },
    /// Set kill-switch state.
    SetKillSwitch { id: &'static str, enabled: bool },
}

/// Commit/rollback pair produced by a policy.
///
/// # Examples
///
/// ```
/// use openpit::pretrade::{Mutation, RiskMutation};
///
/// let mutation = Mutation {
///     commit: RiskMutation::SetKillSwitch { id: "guard", enabled: true },
///     rollback: RiskMutation::SetKillSwitch { id: "guard", enabled: false },
/// };
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mutation {
    /// Mutation applied on commit.
    pub commit: RiskMutation,
    /// Mutation applied on rollback.
    pub rollback: RiskMutation,
}

/// Collected mutations registered during pre-trade checks.
///
/// # Examples
///
/// ```
/// use openpit::pretrade::{Mutation, Mutations, RiskMutation};
///
/// let mut mutations = Mutations::new();
/// mutations.push(Mutation {
///     commit: RiskMutation::SetKillSwitch { id: "guard", enabled: true },
///     rollback: RiskMutation::SetKillSwitch { id: "guard", enabled: false },
/// });
/// ```
#[derive(Default)]
pub struct Mutations {
    mutations: Vec<Mutation>,
}

impl Mutations {
    /// Creates an empty collector.
    pub fn new() -> Self {
        Self {
            mutations: Vec::new(),
        }
    }

    /// Appends a mutation pair.
    pub fn push(&mut self, mutation: Mutation) {
        self.mutations.push(mutation);
    }

    pub(crate) fn as_slice(&self) -> &[Mutation] {
        &self.mutations
    }

    pub(crate) fn into_vec(self) -> Vec<Mutation> {
        self.mutations
    }
}

#[cfg(test)]
mod tests {
    use crate::param::{Asset, Volume};

    use super::{Mutation, Mutations, RiskMutation};

    #[test]
    fn default_creates_empty_mutation_collector() {
        let mutations = Mutations::default();

        assert!(mutations.as_slice().is_empty());
        assert!(mutations.into_vec().is_empty());
    }

    #[test]
    fn push_stores_mutation_pair() {
        let mut mutations = Mutations::new();
        mutations.push(Mutation {
            commit: RiskMutation::ReserveNotional {
                asset: Asset::new("USD").expect("asset code must be valid"),
                amount: Volume::from_str("10").expect("volume must be valid"),
            },
            rollback: RiskMutation::SetKillSwitch {
                id: "m1",
                enabled: false,
            },
        });

        assert_eq!(mutations.as_slice().len(), 1);
    }
}
