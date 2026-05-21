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

package reject

import (
	"unsafe"

	"go.openpit.dev/openpit/internal/native"
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

type AccountAdjustmentBatchError struct {
	Rejects               []Reject
	FailedAdjustmentIndex int
}

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
