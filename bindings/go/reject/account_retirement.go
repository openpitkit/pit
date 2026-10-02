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

package reject

import (
	"fmt"

	"go.openpit.dev/openpit/internal/native"
)

// AccountRetirementErrorKind classifies a failed retirement.
type AccountRetirementErrorKind uint32

const (
	// AccountRetirementErrorKindRefused means no account state was removed.
	AccountRetirementErrorKindRefused AccountRetirementErrorKind = AccountRetirementErrorKind(native.AccountRetirementErrorKindRefused)
	// AccountRetirementErrorKindFinalizerFailed means policy state may be partly
	// removed; the account ID must never be reused.
	AccountRetirementErrorKindFinalizerFailed AccountRetirementErrorKind = AccountRetirementErrorKind(native.AccountRetirementErrorKindFinalizerFailed)
)

// AccountRetirementRefusalKind explains why one policy refused retirement.
type AccountRetirementRefusalKind uint32

const (
	// AccountRetirementRefusalKindConfigurationReferencesAccount means the
	// policy's configuration still names this account.
	AccountRetirementRefusalKindConfigurationReferencesAccount AccountRetirementRefusalKind = AccountRetirementRefusalKind(native.AccountRetirementRefusalKindConfigurationReferencesAccount)
	// AccountRetirementRefusalKindNonZeroState means account state is not zero.
	AccountRetirementRefusalKindNonZeroState AccountRetirementRefusalKind = AccountRetirementRefusalKind(native.AccountRetirementRefusalKindNonZeroState)
	// AccountRetirementRefusalKindOperationInProgress means an account operation
	// is still in flight.
	AccountRetirementRefusalKindOperationInProgress AccountRetirementRefusalKind = AccountRetirementRefusalKind(native.AccountRetirementRefusalKindOperationInProgress)
	// AccountRetirementRefusalKindEvaluationFailed means the policy could not
	// determine whether retirement is safe.
	AccountRetirementRefusalKindEvaluationFailed AccountRetirementRefusalKind = AccountRetirementRefusalKind(native.AccountRetirementRefusalKindEvaluationFailed)
)

// AccountRetirementPolicyRefusal identifies one refusing policy and its reason.
type AccountRetirementPolicyRefusal struct {
	Policy string
	Kind   AccountRetirementRefusalKind
}

// AccountRetirementError is a domain failure from Engine.RetireAccount.
// Refusals are in policy registration order when Kind is Refused.
type AccountRetirementError struct {
	Message  string
	Kind     AccountRetirementErrorKind
	Refusals []AccountRetirementPolicyRefusal
}

// Error implements error.
func (e *AccountRetirementError) Error() string { return e.Message }

// NewAccountRetirementErrorFromHandle copies and releases a native error.
func NewAccountRetirementErrorFromHandle(handle native.AccountRetirementError) (*AccountRetirementError, error) {
	defer native.DestroyAccountRetirementError(handle)
	result := &AccountRetirementError{
		Message: native.AccountRetirementErrorGetMessage(handle),
		Kind:    AccountRetirementErrorKind(native.AccountRetirementErrorGetKind(handle)),
	}
	count := native.AccountRetirementErrorGetRefusalCount(handle)
	result.Refusals = make([]AccountRetirementPolicyRefusal, 0, count)
	for index := 0; index < count; index++ {
		name, ok := native.AccountRetirementErrorGetRefusalPolicyName(handle, index)
		if !ok {
			return nil, fmt.Errorf("account retirement refusal %d has no policy name", index)
		}
		kind, ok := native.AccountRetirementErrorGetRefusalKind(handle, index)
		if !ok {
			return nil, fmt.Errorf("account retirement refusal %d has no kind", index)
		}
		result.Refusals = append(result.Refusals, AccountRetirementPolicyRefusal{
			Policy: name,
			Kind:   AccountRetirementRefusalKind(kind),
		})
	}
	return result, nil
}
