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

	"github.com/openpitkit/pit-go/internal/callback"
	"github.com/openpitkit/pit-go/internal/native"
	"github.com/openpitkit/pit-go/model"
	"github.com/openpitkit/pit-go/pretrade"
)

type CheckPreTradeStart struct {
	impl   pretrade.CheckPreTradeStartPolicy
	handle cgo.Handle
}

func StartCheckPreTradeStart(
	impl pretrade.CheckPreTradeStartPolicy,
) (native.PretradeCheckPreTradeStartPolicy, error) {
	implHandle := &CheckPreTradeStart{impl: impl}
	implHandle.handle = cgo.NewHandle(implHandle)

	policyHandle, err := native.CreatePretradeCustomCheckPreTradeStartPolicy(
		impl.Name(),
		CheckPreTradeStartPolicyCheckFnAddr(),
		CheckPreTradeStartPolicyApplyExecutionReportFnAddr(),
		CheckPreTradeStartPolicyFreeUserDataFnAddr(),
		callback.NewUserDataFromHandle(implHandle.handle),
	)
	if err != nil {
		implHandle.handle.Delete()
		return nil, err
	}

	return policyHandle, nil
}

func (c *CheckPreTradeStart) Close() {
	c.impl.Close()
	c.handle.Delete()
}

//export pitPretradeCheckPreTradeStartPolicyCheckPreTradeStart
func pitPretradeCheckPreTradeStartPolicyCheckPreTradeStart(
	ctx *C.PitPretradeContext,
	order *C.PitOrder,
	userData unsafe.Pointer,
) *C.PitRejectList {
	// Panics from the user implementation are deliberately allowed to propagate.
	// A panic unwinding across the FFI boundary may terminate the process;
	// containing it is the implementer's responsibility, as stated on the Policy
	// interface.
	return newNativeRejectListOrNil(
		getCheckPreTradeStart(userData).impl.CheckPreTradeStart(
			pretrade.NewContextFromHandle(
				native.PretradeContext(ctx),
			),
			model.NewOrderFromNative(*(*native.Order)(unsafe.Pointer(order))),
		),
	)
}

//export pitPretradeCheckPreTradeStartPolicyApplyExecutionReport
func pitPretradeCheckPreTradeStartPolicyApplyExecutionReport(
	report *C.PitExecutionReport,
	userData unsafe.Pointer,
) C.bool {
	// Panics from the user implementation are deliberately allowed to
	// propagate. A panic unwinding across the FFI boundary may terminate the
	// process; containing it is the implementer's responsibility, as stated
	// on the Policy interface.

	return C.bool(
		getCheckPreTradeStart(userData).impl.ApplyExecutionReport(
			model.NewExecutionReportFromNative(
				*(*native.ExecutionReport)(unsafe.Pointer(report)),
			),
		),
	)
}

//export pitPretradeCheckPreTradeStartPolicyClose
func pitPretradeCheckPreTradeStartPolicyClose(userData unsafe.Pointer) {
	getCheckPreTradeStart(userData).Close()
}

func getCheckPreTradeStart(userData unsafe.Pointer) *CheckPreTradeStart {
	return callback.NewHandleFromUserData(userData).Value().(*CheckPreTradeStart)
}
