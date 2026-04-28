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

package custompolicy

/*
#cgo CFLAGS: -I${SRCDIR}/../native
#include "pit.h"
*/
import "C"

import (
	"runtime/cgo"
	"unsafe"

	"go.openpit.dev/openpit/accountadjustment"
	"go.openpit.dev/openpit/internal/callback"
	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/tx"
)

type AccountAdjustment struct {
	impl   accountadjustment.Policy
	handle cgo.Handle
}

func StartAccountAdjustment(
	impl accountadjustment.Policy,
) (native.AccountAdjustmentPolicy, error) {
	implHandle := &AccountAdjustment{impl: impl}
	implHandle.handle = cgo.NewHandle(implHandle)

	policyHandle, err := native.CreateCustomAccountAdjustmentPolicy(
		impl.Name(),
		AccountAdjustmentPolicyApplyFnAddr(),
		AccountAdjustmentPolicyFreeUserDataFnAddr(),
		callback.NewUserDataFromHandle(implHandle.handle),
	)
	if err != nil {
		implHandle.handle.Delete()
		return nil, err
	}

	return policyHandle, nil
}

func (a *AccountAdjustment) Close() {
	a.impl.Close()
	a.handle.Delete()
}

//export pitAccountAdjustmentPolicyApply
func pitAccountAdjustmentPolicyApply(
	ctx *C.PitAccountAdjustmentContext,
	accountID C.PitParamAccountId,
	adjustment *C.PitAccountAdjustment,
	mutations *C.PitMutations,
	userData unsafe.Pointer,
) *C.PitRejectList {
	// Panics from the user implementation are deliberately allowed to propagate.
	// A panic unwinding across the FFI boundary may terminate the process;
	// containing it is the implementer's responsibility, as stated on the Policy
	// interface.

	return newNativeRejectListOrNil(
		getAccountAdjustment(userData).impl.ApplyAccountAdjustment(
			accountadjustment.NewContextFromHandle(
				native.AccountAdjustmentContext(ctx),
			),
			param.NewAccountIDFromHandle(
				native.ParamAccountID(accountID),
			),
			model.NewAccountAdjustmentFromHandle(
				*(*native.AccountAdjustment)(unsafe.Pointer(adjustment)),
			),
			tx.NewMutationsFromHandle(
				native.Mutations(mutations),
			),
		),
	)
}

//export pitAccountAdjustmentPolicyClose
func pitAccountAdjustmentPolicyClose(userData unsafe.Pointer) {
	getAccountAdjustment(userData).Close()
}

func getAccountAdjustment(userData unsafe.Pointer) *AccountAdjustment {
	return callback.NewHandleFromUserData(userData).Value().(*AccountAdjustment)
}
