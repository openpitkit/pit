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

	"go.openpit.dev/openpit/internal/callback"
	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/pretrade"
	"go.openpit.dev/openpit/tx"
)

type PreTrade struct {
	impl   pretrade.Policy
	handle cgo.Handle
}

func StartPreTrade(impl pretrade.Policy) (native.PretradePreTradePolicy, error) {
	implHandle := &PreTrade{impl: impl}
	implHandle.handle = cgo.NewHandle(implHandle)

	policyHandle, err := native.CreatePretradeCustomPreTradePolicy(
		impl.Name(),
		PreTradePolicyCheckFnAddr(),
		PreTradePolicyApplyReportFnAddr(),
		PreTradePolicyFreeUserDataFnAddr(),
		callback.NewUserDataFromHandle(implHandle.handle),
	)
	if err != nil {
		implHandle.handle.Delete()
		return nil, err
	}

	return policyHandle, nil
}

func (p *PreTrade) Close() {
	p.impl.Close()
	p.handle.Delete()
}

//export pitPretradePreTradePolicyCheckPreTrade
func pitPretradePreTradePolicyCheckPreTrade(
	ctx *C.PitPretradeContext,
	order *C.PitOrder,
	mutations *C.PitMutations,
	userData unsafe.Pointer,
) *C.PitRejectList {
	// Panics from the user implementation are deliberately allowed to propagate.
	// A panic unwinding across the FFI boundary may terminate the process;
	// containing it is the implementer's responsibility, as stated on the Policy
	// interface.

	return newNativeRejectListOrNil(
		getPreTrade(userData).impl.PerformPreTradeCheck(
			pretrade.NewContextFromHandle(
				native.PretradeContext(ctx),
			),
			model.NewOrderFromHandle(*(*native.Order)(unsafe.Pointer(order))),
			tx.NewMutationsFromHandle(
				native.Mutations(mutations),
			),
		),
	)
}

//export pitPretradePreTradePolicyApplyExecutionReport
func pitPretradePreTradePolicyApplyExecutionReport(
	report *C.PitExecutionReport,
	userData unsafe.Pointer,
) C.bool {
	// Panics from the user implementation are deliberately allowed to
	// propagate. A panic unwinding across the FFI boundary may terminate the
	// process; containing it is the implementer's responsibility, as stated
	// on the Policy interface.

	return C.bool(
		getPreTrade(userData).impl.ApplyExecutionReport(
			model.NewExecutionReportFromHandle(
				*(*native.ExecutionReport)(unsafe.Pointer(report)),
			),
		),
	)
}

//export pitPretradePreTradePolicyClose
func pitPretradePreTradePolicyClose(userData unsafe.Pointer) {
	getPreTrade(userData).Close()
}

func getPreTrade(userData unsafe.Pointer) *PreTrade {
	return callback.NewHandleFromUserData(userData).Value().(*PreTrade)
}
