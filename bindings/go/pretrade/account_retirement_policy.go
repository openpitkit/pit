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

package pretrade

import (
	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/tx"
)

// AccountRetirementDecision accepts retirement or gives one refusal reason.
type AccountRetirementDecision uint8

const (
	// AccountRetirementAccept permits the account's state to be removed.
	AccountRetirementAccept AccountRetirementDecision = AccountRetirementDecision(native.PretradeRetireAccountAccept)
	// AccountRetirementConfigurationReferencesAccount keeps configuration intact.
	AccountRetirementConfigurationReferencesAccount AccountRetirementDecision = AccountRetirementDecision(native.PretradeRetireAccountConfigurationReferencesAccount)
	// AccountRetirementNonZeroState refuses non-zero account state.
	AccountRetirementNonZeroState AccountRetirementDecision = AccountRetirementDecision(native.PretradeRetireAccountNonZeroState)
	// AccountRetirementOperationInProgress refuses in-flight state.
	AccountRetirementOperationInProgress AccountRetirementDecision = AccountRetirementDecision(native.PretradeRetireAccountOperationInProgress)
	// AccountRetirementEvaluationFailed refuses an indeterminate check.
	AccountRetirementEvaluationFailed AccountRetirementDecision = AccountRetirementDecision(native.PretradeRetireAccountEvaluationFailed)
)

// AccountRetirementPolicy is an optional interface for account-scoped state.
// A policy without it is treated as holding no account-scoped state. A policy
// that keeps such state MUST implement it; otherwise the state silently
// survives retirement and a reused account ID inherits it.
//
// RetireAccount only verifies zero, unused state and registers removals as
// commit mutations. It removes nothing before commit; rollback must leave
// the state intact. The hook and its mutations run under the engine's
// exclusive account-state lease and must not call back into the engine.
// A panic in RetireAccount is recovered at the SDK boundary and reported as
// AccountRetirementEvaluationFailed for this policy. The panic value is not
// propagated; the retirement result carries only the policy name and refusal
// kind.
// The mutations value is valid only during this hook call. Do not retain it or
// use it after return; doing so is undefined.
type AccountRetirementPolicy interface {
	RetireAccount(accountID param.AccountID, mutations tx.Mutations) AccountRetirementDecision
}
