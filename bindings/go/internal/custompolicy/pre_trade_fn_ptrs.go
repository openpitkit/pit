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

extern PitRejectList* pitPretradePreTradePolicyCheckPreTrade(
    const PitPretradeContext* ctx,
    const PitOrder* order,
    PitMutations* mutations,
    void* user_data);

extern bool pitPretradePreTradePolicyApplyExecutionReport(
    const PitExecutionReport* report,
    void* user_data);

extern void pitPretradePreTradePolicyClose(void* user_data);

static PitPretradePreTradePolicyCheckFn
    pit_pretrade_pre_trade_policy_check_fn = pitPretradePreTradePolicyCheckPreTrade;

static PitPretradePreTradePolicyApplyExecutionReportFn
    pit_pretrade_pre_trade_policy_apply_execution_report_fn = pitPretradePreTradePolicyApplyExecutionReport;

static PitPretradePreTradePolicyFreeUserDataFn
    pit_pretrade_pre_trade_policy_free_user_data_fn = pitPretradePreTradePolicyClose;

static void* pitPretradePreTradePolicyCheckFnAddr(void) {
    return &pit_pretrade_pre_trade_policy_check_fn;
}

static void* pitPretradePreTradePolicyApplyReportFnAddr(void) {
    return &pit_pretrade_pre_trade_policy_apply_execution_report_fn;
}

static void* pitPretradePreTradePolicyFreeUserDataFnAddr(void) {
    return &pit_pretrade_pre_trade_policy_free_user_data_fn;
}
*/
import "C"
import "unsafe"

// PreTradePolicyCheckFnAddr returns the address of a
// PitPretradePreTradePolicyCheckFn variable holding the check callback.
// Pass the result to native.CreatePretradeCustomPreTradePolicy.
func PreTradePolicyCheckFnAddr() unsafe.Pointer {
	return C.pitPretradePreTradePolicyCheckFnAddr()
}

// PreTradePolicyApplyReportFnAddr returns the address of a
// PitPretradePreTradePolicyApplyExecutionReportFn variable holding the apply callback.
// Pass the result to native.CreatePretradeCustomPreTradePolicy.
func PreTradePolicyApplyReportFnAddr() unsafe.Pointer {
	return C.pitPretradePreTradePolicyApplyReportFnAddr()
}

// PreTradePolicyFreeUserDataFnAddr returns the address of a
// PitPretradePreTradePolicyFreeUserDataFn variable holding the free callback.
// Pass the result to native.CreatePretradeCustomPreTradePolicy.
func PreTradePolicyFreeUserDataFnAddr() unsafe.Pointer {
	return C.pitPretradePreTradePolicyFreeUserDataFnAddr()
}
