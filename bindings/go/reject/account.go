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

// Package reject provides reject codes, scopes, and reject/block value types.
package reject

import (
	"unsafe"

	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/param"
)

// AccountBlock is a kill-switch block record returned by policy callbacks.
type AccountBlock struct {
	// Human-readable reject reason.
	Reason string
	// Case-specific reject details.
	Details string
	// Policy name that produced the block.
	Policy string
	// Opaque caller-defined payload. Nil means "not set".
	UserData unsafe.Pointer
	// Stable machine-readable reject code.
	Code Code
}

// NewAccountBlock creates an account block.
func NewAccountBlock(code Code, policy, reason, details string) AccountBlock {
	return AccountBlock{
		Code:    code,
		Policy:  policy,
		Reason:  reason,
		Details: details,
	}
}

// NewAccountBlockFromHandle creates an AccountBlock from a native handle.
func NewAccountBlockFromHandle(handle native.PretradeAccountBlock) AccountBlock {
	return AccountBlock{
		Code:     Code(native.PretradeAccountBlockGetCode(handle)),
		Policy:   native.PretradeAccountBlockGetPolicy(handle).Safe(),
		Reason:   native.PretradeAccountBlockGetReason(handle).Safe(),
		Details:  native.PretradeAccountBlockGetDetails(handle).Safe(),
		UserData: native.PretradeAccountBlockGetUserData(handle),
	}
}

// NewHandle returns a native handle for this account block.
func (b AccountBlock) NewHandle() native.PretradeAccountBlock {
	return native.CreatePretradeAccountBlock(
		native.PretradeRejectCode(b.Code),
		native.NewStringView(b.Policy),
		native.NewStringView(b.Reason),
		native.NewStringView(b.Details),
		b.UserData,
	)
}

// WithUserData returns a copy of this block with updated UserData.
func (b AccountBlock) WithUserData(userData unsafe.Pointer) AccountBlock {
	b.UserData = userData
	return b
}

// AccountAdjustmentBatchError is returned when a batch adjustment is rejected.
type AccountAdjustmentBatchError struct {
	Rejects               []Reject
	FailedAdjustmentIndex int
}

// NewAccountAdjustmentBatchErrorFromHandle creates an AccountAdjustmentBatchError from a native handle.
func NewAccountAdjustmentBatchErrorFromHandle(
	reject native.AccountAdjustmentBatchError,
) (AccountAdjustmentBatchError, error) {
	rejectList, err := NewListFromHandle(native.AccountAdjustmentBatchErrorGetRejects(reject))
	if err != nil {
		return AccountAdjustmentBatchError{}, err
	}

	return AccountAdjustmentBatchError{
			Rejects:               rejectList,
			FailedAdjustmentIndex: native.AccountAdjustmentBatchErrorGetFailedAdjustmentIndex(reject),
		},
		nil
}

// AccountGroupError is returned when an Accounts.RegisterGroup or
// Accounts.UnregisterGroup call fails due to a group conflict or a reserved
// default-group target.
type AccountGroupError struct {
	// Message is the human-readable error description.
	Message string
	// Account is the identifier of the conflicting account.
	Account param.AccountID
	// CurrentGroup is set to the group the account currently belongs to
	// when the conflict is a duplicate-registration error. It is nil when
	// the conflict is an unregister error (account not in the given group).
	CurrentGroup *param.AccountGroupID
}

// Error implements the error interface.
func (e *AccountGroupError) Error() string {
	return e.Message
}

// NewAccountGroupErrorFromHandle creates an AccountGroupError from a native
// handle and releases it.
func NewAccountGroupErrorFromHandle(handle native.AccountGroupError) *AccountGroupError {
	msg := native.AccountGroupErrorGetMessage(handle)
	account := param.NewAccountIDFromHandle(native.AccountGroupErrorGetAccount(handle))
	group, hasGroup := native.AccountGroupErrorGetCurrentGroup(handle)
	native.DestroyAccountGroupError(handle)
	err := &AccountGroupError{
		Message: msg,
		Account: account,
	}
	if hasGroup {
		g := param.NewAccountGroupIDFromHandle(group)
		err.CurrentGroup = &g
	}
	return err
}
