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
	"errors"
	"fmt"
	"runtime/cgo"
	"unsafe"

	"go.openpit.dev/openpit/accountadjustment"
	"go.openpit.dev/openpit/internal/callback"
	"go.openpit.dev/openpit/internal/diag"
	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pretrade"
	"go.openpit.dev/openpit/reject"
	"go.openpit.dev/openpit/tx"
)

type PreTrade struct {
	impl   pretrade.Policy
	dryRun pretrade.DryRunPolicy
	name   string
	handle cgo.Handle
}

// StartPreTrade registers impl as a native custom pre-trade policy.
//
// When impl also satisfies pretrade.DryRunPolicy, the engine is given
// explicit dry-run hooks; otherwise the normal hooks delegate for dry-runs.
func StartPreTrade(impl pretrade.Policy) (native.PretradePreTradePolicy, error) {
	implHandle := &PreTrade{impl: impl, name: impl.Name()}
	if dryRun, ok := impl.(pretrade.DryRunPolicy); ok {
		implHandle.dryRun = dryRun
	}
	implHandle.handle = cgo.NewHandle(implHandle)

	userData := callback.NewUserDataFromHandle(implHandle.handle)
	name := implHandle.name
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
	defer p.handle.Delete()
	p.impl.Close()
}

func callbackPanicRejects(policyName string, recovered any) []reject.Reject {
	return reject.NewSingleItemList(
		reject.CodeSystemUnavailable,
		policyName,
		"custom policy callback panicked",
		fmt.Sprintf("panic: %v", recovered),
		reject.ScopeOrder,
	)
}

func callbackPanicAccountBlocks(
	policyName string,
	recovered any,
) []reject.AccountBlock {
	return []reject.AccountBlock{reject.NewAccountBlock(
		reject.CodeSystemUnavailable,
		policyName,
		"custom policy callback panicked",
		fmt.Sprintf("panic: %v", recovered),
	)}
}

//export pitPretradePreTradePolicyCheckPreTradeStart
func pitPretradePreTradePolicyCheckPreTradeStart(
	ctx *C.OpenPitPretradeContext,
	order *C.OpenPitOrder,
	userData unsafe.Pointer,
) (result *C.OpenPitPretradeRejectList) {
	policyName := "openpit.callback"
	defer func() {
		if recovered := recover(); recovered != nil {
			result = newNativeRejectList(
				callbackPanicRejects(policyName, recovered),
			)
		}
	}()
	policy := getPreTrade(userData)
	policyName = policy.name
	return newNativeRejectList(
		policy.impl.CheckPreTradeStart(
			pretrade.NewContextFromHandle(native.PretradeContext(ctx)),
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
) (result *C.OpenPitPretradeRejectList) {
	policyName := "openpit.callback"
	defer func() {
		if recovered := recover(); recovered != nil {
			result = newNativeRejectList(
				callbackPanicRejects(policyName, recovered),
			)
		}
	}()
	policy := getPreTrade(userData)
	policyName = policy.name
	return newNativeRejectList(
		policy.impl.PerformPreTradeCheck(
			pretrade.NewContextFromHandle(native.PretradeContext(ctx)),
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
) (result *C.OpenPitPretradeAccountBlockList) {
	policyName := "openpit.callback"
	defer func() {
		if recovered := recover(); recovered != nil {
			result = newNativeAccountBlockListOrNil(
				callbackPanicAccountBlocks(policyName, recovered),
			)
		}
	}()
	policy := getPreTrade(userData)
	policyName = policy.name
	return newNativeAccountBlockListOrNil(
		policy.impl.ApplyExecutionReport(
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
) (rejectList *C.OpenPitPretradeRejectList) {
	policyName := "openpit.callback"
	defer func() {
		if recovered := recover(); recovered != nil {
			rejectList = newNativeRejectList(
				callbackPanicRejects(policyName, recovered),
			)
		}
	}()
	policy := getPreTrade(userData)
	policyName = policy.name
	result, rejects := policy.impl.ApplyAccountAdjustment(
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
	return newNativeRejectList(rejects)
}

//export pitPretradePreTradePolicyClose
func pitPretradePreTradePolicyClose(userData unsafe.Pointer) {
	// Close carries no result, so a panic here has no reject to travel on. Keep
	// it contained so one failing policy cannot unwind across the cgo frame, and
	// report it through the SDK diagnostic sink instead of the embedder's logger.
	handle := callback.NewHandleFromUserData(userData)
	defer func() {
		if recovered := recover(); recovered != nil {
			diag.Report(fmt.Errorf("custom pre-trade policy close callback panicked: %v", recovered))
		}
	}()
	policy, ok := handle.Value().(*PreTrade)
	if !ok {
		// The handle is still allocated, and no PreTrade.Close will ever reach it,
		// so release it here rather than leak it.
		handle.Delete()
		diag.Report(errors.New("custom pre-trade policy close callback got foreign user data"))
		return
	}
	policy.Close()
}

//export pitPretradePreTradePolicyCheckPreTradeStartDryRun
func pitPretradePreTradePolicyCheckPreTradeStartDryRun(
	ctx *C.OpenPitPretradeContext,
	order *C.OpenPitOrder,
	userData unsafe.Pointer,
) (result *C.OpenPitPretradeRejectList) {
	policyName := "openpit.callback"
	defer func() {
		if recovered := recover(); recovered != nil {
			result = newNativeRejectList(
				callbackPanicRejects(policyName, recovered),
			)
		}
	}()
	policy := getPreTrade(userData)
	policyName = policy.name
	return newNativeRejectList(
		policy.dryRun.CheckPreTradeStartDryRun(
			pretrade.NewContextFromHandle(native.PretradeContext(ctx)),
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
) (result *C.OpenPitPretradeRejectList) {
	policyName := "openpit.callback"
	defer func() {
		if recovered := recover(); recovered != nil {
			result = newNativeRejectList(
				callbackPanicRejects(policyName, recovered),
			)
		}
	}()
	policy := getPreTrade(userData)
	policyName = policy.name
	return newNativeRejectList(
		policy.dryRun.PerformPreTradeCheckDryRun(
			pretrade.NewContextFromHandle(native.PretradeContext(ctx)),
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
