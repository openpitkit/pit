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

use std::fmt::{Display, Formatter};

/// Why one policy refuses to retire an account.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum AccountRetirementRefusal {
    /// The policy's configuration still names the account.
    ConfigurationReferencesAccount,
    /// The policy holds non-zero state for the account.
    NonZeroState,
    /// The policy holds state of an operation that is still in flight.
    OperationInProgress,
    /// The policy could not determine whether its account state may be
    /// retired - for example a foreign callback failed or returned an invalid
    /// decision; retirement refuses and nothing is removed.
    EvaluationFailed,
}

impl Display for AccountRetirementRefusal {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConfigurationReferencesAccount => {
                formatter.write_str("policy configuration references the account")
            }
            Self::NonZeroState => formatter.write_str("policy holds non-zero account state"),
            Self::OperationInProgress => {
                formatter.write_str("policy has an account operation in progress")
            }
            Self::EvaluationFailed => {
                formatter.write_str("policy could not evaluate account retirement")
            }
        }
    }
}

impl std::error::Error for AccountRetirementRefusal {}

/// One policy's refusal to retire an account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountRetirementPolicyRefusal {
    /// Name returned by the refusing policy's `name()` method.
    pub policy: String,
    /// Reason the policy refused retirement.
    pub refusal: AccountRetirementRefusal,
}

/// Error returned by [`Engine::retire_account`](crate::Engine::retire_account).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AccountRetirementError {
    /// One or more policies refused; nothing was removed. A failing rollback
    /// finalizer still arms the kill switch, per the finalizer contract on
    /// [`Mutation`](crate::Mutation).
    Refused {
        /// Every refusal, in policy registration order; never empty.
        refusals: Vec<AccountRetirementPolicyRefusal>,
    },
    /// A commit finalizer failed after every policy accepted. Some policy
    /// state may already be removed. The engine raised its kill switch per
    /// the finalizer contract on `Mutation`: the account is blocked, or every
    /// account is blocked for a custom-policy finalizer. The account's
    /// currency, membership, and block remain. Do not reuse the `AccountId`,
    /// even if a later retirement attempt succeeds.
    FinalizerFailed,
}

impl Display for AccountRetirementError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Refused { .. } => {
                formatter.write_str("account retirement refused by one or more policies")
            }
            Self::FinalizerFailed => formatter.write_str("account retirement finalizer failed"),
        }
    }
}

impl std::error::Error for AccountRetirementError {}

#[cfg(test)]
mod tests {
    use super::{AccountRetirementError, AccountRetirementRefusal};

    #[test]
    fn retire_refusal_display_is_stable_and_omits_account_id() {
        let cases = [
            (
                AccountRetirementRefusal::ConfigurationReferencesAccount,
                "policy configuration references the account",
            ),
            (
                AccountRetirementRefusal::NonZeroState,
                "policy holds non-zero account state",
            ),
            (
                AccountRetirementRefusal::OperationInProgress,
                "policy has an account operation in progress",
            ),
            (
                AccountRetirementRefusal::EvaluationFailed,
                "policy could not evaluate account retirement",
            ),
        ];
        for (refusal, expected) in cases {
            let display = refusal.to_string();
            assert_eq!(display, expected);
            assert!(!display.contains("99224416"));
        }
    }

    #[test]
    fn retire_error_display_is_stable_and_omits_account_id() {
        let errors = [
            (
                AccountRetirementError::Refused { refusals: vec![] },
                "account retirement refused by one or more policies",
            ),
            (
                AccountRetirementError::FinalizerFailed,
                "account retirement finalizer failed",
            ),
        ];
        for (error, expected) in errors {
            let display = error.to_string();
            assert_eq!(display, expected);
            assert!(!display.contains("99224416"));
        }
    }
}
