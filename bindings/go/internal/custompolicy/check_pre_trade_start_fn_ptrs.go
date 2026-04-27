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

extern PitRejectList* pitPretradeCheckPreTradeStartPolicyCheckPreTradeStart(
    const PitPretradeContext* ctx,
    const PitOrder* order,
    void* user_data);

extern bool pitPretradeCheckPreTradeStartPolicyApplyExecutionReport(
    const PitExecutionReport* report,
    void* user_data);

extern void pitPretradeCheckPreTradeStartPolicyClose(void* user_data);

static PitPretradeCheckPreTradeStartPolicyCheckPreTradeStartFn
    pit_pretrade_check_pre_trade_start_policy_check_pre_trade_start_fn = pitPretradeCheckPreTradeStartPolicyCheckPreTradeStart;

static PitPretradeCheckPreTradeStartPolicyApplyExecutionReportFn
    pit_pretrade_check_pre_trade_start_policy_apply_execution_report_fn = pitPretradeCheckPreTradeStartPolicyApplyExecutionReport;

static PitPretradeCheckPreTradeStartPolicyFreeUserDataFn
    pit_pretrade_check_pre_trade_start_policy_free_user_data_fn = pitPretradeCheckPreTradeStartPolicyClose;

static void* pitPretradeCheckPreTradeStartPolicyCheckFnAddr(void) {
    return &pit_pretrade_check_pre_trade_start_policy_check_pre_trade_start_fn;
}

static void* pitPretradeCheckPreTradeStartPolicyApplyReportFnAddr(void) {
    return &pit_pretrade_check_pre_trade_start_policy_apply_execution_report_fn;
}

static void* pitPretradeCheckPreTradeStartPolicyFreeUserDataFnAddr(void) {
    return &pit_pretrade_check_pre_trade_start_policy_free_user_data_fn;
}
*/
import "C"
import "unsafe"

// CheckPreTradeStartPolicyCheckFnAddr returns the address of a
// PitPretradeCheckPreTradeStartPolicyCheckPreTradeStartFn variable holding the check callback.
// Pass the result to native.CreatePretradeCustomCheckPreTradeStartPolicy.
func CheckPreTradeStartPolicyCheckFnAddr() unsafe.Pointer {
	return C.pitPretradeCheckPreTradeStartPolicyCheckFnAddr()
}

// CheckPreTradeStartPolicyApplyExecutionReportFnAddr returns the address of a
// PitPretradeCheckPreTradeStartPolicyApplyExecutionReportFn variable holding the apply callback.
// Pass the result to native.CreatePretradeCustomCheckPreTradeStartPolicy.
func CheckPreTradeStartPolicyApplyExecutionReportFnAddr() unsafe.Pointer {
	return C.pitPretradeCheckPreTradeStartPolicyApplyReportFnAddr()
}

// CheckPreTradeStartPolicyFreeUserDataFnAddr returns the address of a
// PitPretradeCheckPreTradeStartPolicyFreeUserDataFn variable holding the free callback.
// Pass the result to native.CreatePretradeCustomCheckPreTradeStartPolicy.
func CheckPreTradeStartPolicyFreeUserDataFnAddr() unsafe.Pointer {
	return C.pitPretradeCheckPreTradeStartPolicyFreeUserDataFnAddr()
}
