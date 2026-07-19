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

package custompolicy

/*
#cgo CFLAGS: -I${SRCDIR}/../native
#include "openpit.h"
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
	"go.openpit.dev/openpit/pretrade"
	"go.openpit.dev/openpit/tx"
)

type PreTrade struct {
	impl   pretrade.Policy
	dryRun pretrade.DryRunPolicy
	handle cgo.Handle
}

// StartPreTrade registers impl as a native custom pre-trade policy.
//
// When impl also satisfies pretrade.DryRunPolicy, the engine is given
// explicit dry-run hooks; otherwise the normal hooks delegate for dry-runs.
func StartPreTrade(impl pretrade.Policy) (native.PretradePreTradePolicy, error) {
	implHandle := &PreTrade{impl: impl}
	if dryRun, ok := impl.(pretrade.DryRunPolicy); ok {
		implHandle.dryRun = dryRun
	}
	implHandle.handle = cgo.NewHandle(implHandle)

	userData := callback.NewUserDataFromHandle(implHandle.handle)
	name := impl.Name()
	groupID := native.PolicyGroupID(impl.PolicyGroupID())

	var policyHandle native.PretradePreTradePolicy
	var err error

	if implHandle.dryRun != nil {
		policyHandle, err = native.CreatePretradeCustomPreTradePolicyWithDryRun(
			name,
			groupID,
			PreTradePolicyCheckPreTradeStartFnAddr(),
			PreTradePolicyCheckPreTradeStartDryRunFnAddr(),
			PreTradePolicyPerformPreTradeCheckFnAddr(),
			PreTradePolicyPerformPreTradeCheckDryRunFnAddr(),
			PreTradePolicyApplyReportFnAddr(),
			PreTradePolicyApplyAccountAdjustmentFnAddr(),
			PreTradePolicyFreeUserDataFnAddr(),
			userData,
		)
	} else {
		policyHandle, err = native.CreatePretradeCustomPreTradePolicy(
			name,
			groupID,
			PreTradePolicyCheckPreTradeStartFnAddr(),
			PreTradePolicyPerformPreTradeCheckFnAddr(),
			PreTradePolicyApplyReportFnAddr(),
			PreTradePolicyApplyAccountAdjustmentFnAddr(),
			PreTradePolicyFreeUserDataFnAddr(),
			userData,
		)
	}
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

//export pitPretradePreTradePolicyCheckPreTradeStart
func pitPretradePreTradePolicyCheckPreTradeStart(
	ctx *C.OpenPitPretradeContext,
	order *C.OpenPitOrder,
	userData unsafe.Pointer,
) *C.OpenPitPretradeRejectList {
	// Panics from the user implementation are deliberately allowed to propagate.
	// A panic unwinding across the FFI boundary may terminate the process;
	// containing it is the implementer's responsibility, as stated on the Policy
	// interface.

	return newNativeRejectListOrNil(
		getPreTrade(userData).impl.CheckPreTradeStart(
			pretrade.NewContextFromHandle(
				native.PretradeContext(ctx),
			),
			model.NewOrderFromHandle(*(*native.Order)(unsafe.Pointer(order))),
		),
	)
}

//export pitPretradePreTradePolicyPerformPreTradeCheck
func pitPretradePreTradePolicyPerformPreTradeCheck(
	ctx *C.OpenPitPretradeContext,
	order *C.OpenPitOrder,
	mutations *C.OpenPitMutations,
	outResult *C.OpenPitPretradePreTradeResult,
	userData unsafe.Pointer,
) *C.OpenPitPretradeRejectList {
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
			pretrade.NewPreTradeResultFromHandle(
				native.PretradePreTradeResult(outResult),
			),
		),
	)
}

//export pitPretradePreTradePolicyApplyExecutionReport
func pitPretradePreTradePolicyApplyExecutionReport(
	ctx *C.OpenPitPostTradeContext,
	report *C.OpenPitExecutionReport,
	outAdjustments *C.OpenPitPostTradeAdjustmentList,
	outAccountPnls *C.OpenPitPostTradeAccountPnlList,
	userData unsafe.Pointer,
) *C.OpenPitPretradeAccountBlockList {
	// Panics from the user implementation are deliberately allowed to
	// propagate. A panic unwinding across the FFI boundary may terminate the
	// process; containing it is the implementer's responsibility, as stated
	// on the Policy interface.

	return newNativeAccountBlockListOrNil(
		getPreTrade(userData).impl.ApplyExecutionReport(
			pretrade.NewPostTradeContextFromHandle(
				native.PostTradeContext(ctx),
			),
			model.NewExecutionReportFromHandle(
				*(*native.ExecutionReport)(unsafe.Pointer(report)),
			),
			pretrade.NewPostTradeAdjustmentsFromHandle(
				native.PostTradeAdjustmentList(outAdjustments),
			),
			pretrade.NewPostTradePnlsFromHandle(
				native.PostTradeAccountPnlList(outAccountPnls),
			),
		),
	)
}

//export pitPretradePreTradePolicyApplyAccountAdjustment
func pitPretradePreTradePolicyApplyAccountAdjustment(
	ctx *C.OpenPitAccountAdjustmentContext,
	accountID C.OpenPitParamAccountId,
	adjustment *C.OpenPitAccountAdjustment,
	mutations *C.OpenPitMutations,
	outResult *C.OpenPitPretradeAccountAdjustmentResult,
	userData unsafe.Pointer,
) *C.OpenPitPretradeRejectList {
	// Panics from the user implementation are deliberately allowed to propagate.
	// A panic unwinding across the FFI boundary may terminate the process;
	// containing it is the implementer's responsibility, as stated on the Policy
	// interface.

	result, rejects := getPreTrade(userData).impl.ApplyAccountAdjustment(
		accountadjustment.NewContextFromHandle(native.AccountAdjustmentContext(ctx)),
		param.NewAccountIDFromHandle(native.ParamAccountID(accountID)),
		model.NewAccountAdjustmentFromHandle(
			*(*native.AccountAdjustment)(unsafe.Pointer(adjustment)),
		),
		tx.NewMutationsFromHandle(native.Mutations(mutations)),
		pretrade.NewAccountOutcomesFromHandle(
			native.PretradeAccountAdjustmentResult(outResult),
		),
	)
	for _, block := range result.AccountBlocks {
		native.PretradeAccountAdjustmentResultPushAccountBlock(
			native.PretradeAccountAdjustmentResult(outResult), block.NewHandle(),
		)
	}
	return newNativeRejectListOrNil(rejects)
}

//export pitPretradePreTradePolicyClose
func pitPretradePreTradePolicyClose(userData unsafe.Pointer) {
	getPreTrade(userData).Close()
}

//export pitPretradePreTradePolicyCheckPreTradeStartDryRun
func pitPretradePreTradePolicyCheckPreTradeStartDryRun(
	ctx *C.OpenPitPretradeContext,
	order *C.OpenPitOrder,
	userData unsafe.Pointer,
) *C.OpenPitPretradeRejectList {
	// Panics from the user implementation are deliberately allowed to propagate.
	// A panic unwinding across the FFI boundary may terminate the process;
	// containing it is the implementer's responsibility, as stated on the Policy
	// interface.

	return newNativeRejectListOrNil(
		getPreTrade(userData).dryRun.CheckPreTradeStartDryRun(
			pretrade.NewContextFromHandle(
				native.PretradeContext(ctx),
			),
			model.NewOrderFromHandle(*(*native.Order)(unsafe.Pointer(order))),
		),
	)
}

//export pitPretradePreTradePolicyPerformPreTradeCheckDryRun
func pitPretradePreTradePolicyPerformPreTradeCheckDryRun(
	ctx *C.OpenPitPretradeContext,
	order *C.OpenPitOrder,
	mutations *C.OpenPitMutations,
	outResult *C.OpenPitPretradePreTradeResult,
	userData unsafe.Pointer,
) *C.OpenPitPretradeRejectList {
	// Panics from the user implementation are deliberately allowed to propagate.
	// A panic unwinding across the FFI boundary may terminate the process;
	// containing it is the implementer's responsibility, as stated on the Policy
	// interface.

	return newNativeRejectListOrNil(
		getPreTrade(userData).dryRun.PerformPreTradeCheckDryRun(
			pretrade.NewContextFromHandle(
				native.PretradeContext(ctx),
			),
			model.NewOrderFromHandle(*(*native.Order)(unsafe.Pointer(order))),
			tx.NewMutationsFromHandle(
				native.Mutations(mutations),
			),
			pretrade.NewPreTradeResultFromHandle(
				native.PretradePreTradeResult(outResult),
			),
		),
	)
}

func getPreTrade(userData unsafe.Pointer) *PreTrade {
	return callback.NewHandleFromUserData(userData).Value().(*PreTrade)
}
