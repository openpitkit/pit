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

extern OpenPitPretradeRejectList* pitPretradePreTradePolicyCheckPreTradeStart(
    const OpenPitPretradeContext* ctx,
    const OpenPitOrder* order,
    void* user_data);

extern OpenPitPretradeRejectList* pitPretradePreTradePolicyPerformPreTradeCheck(
    const OpenPitPretradeContext* ctx,
    const OpenPitOrder* order,
    OpenPitMutations* mutations,
    OpenPitPretradePreTradeResult* out_result,
    void* user_data);

extern OpenPitPretradeAccountBlockList* pitPretradePreTradePolicyApplyExecutionReport(
    const OpenPitPostTradeContext* ctx,
    const OpenPitExecutionReport* report,
    OpenPitPostTradeAdjustmentList* out_adjustments,
    OpenPitPostTradeAccountPnlList* out_account_pnls,
    void* user_data);

extern OpenPitPretradeRejectList* pitPretradePreTradePolicyApplyAccountAdjustment(
    const OpenPitAccountAdjustmentContext* ctx,
    OpenPitParamAccountId account_id,
    const OpenPitAccountAdjustment* adjustment,
    OpenPitMutations* mutations,
    OpenPitPretradeAccountAdjustmentResult* out_result,
    void* user_data);

extern void pitPretradePreTradePolicyClose(void* user_data);

extern OpenPitPretradeRejectList* pitPretradePreTradePolicyCheckPreTradeStartDryRun(
    const OpenPitPretradeContext* ctx,
    const OpenPitOrder* order,
    void* user_data);

extern OpenPitPretradeRejectList* pitPretradePreTradePolicyPerformPreTradeCheckDryRun(
    const OpenPitPretradeContext* ctx,
    const OpenPitOrder* order,
    OpenPitMutations* mutations,
    OpenPitPretradePreTradeResult* out_result,
    void* user_data);

static OpenPitPretradePreTradePolicyCheckPreTradeStartFn
    openpit_pretrade_pre_trade_policy_check_pre_trade_start_fn = pitPretradePreTradePolicyCheckPreTradeStart;

static OpenPitPretradePreTradePolicyPerformPreTradeCheckFn
    openpit_pretrade_pre_trade_policy_perform_pre_trade_check_fn = pitPretradePreTradePolicyPerformPreTradeCheck;

static OpenPitPretradePreTradePolicyApplyExecutionReportFn
    openpit_pretrade_pre_trade_policy_apply_execution_report_fn = pitPretradePreTradePolicyApplyExecutionReport;

static OpenPitPretradePreTradePolicyApplyAccountAdjustmentFn
    openpit_pretrade_pre_trade_policy_apply_account_adjustment_fn = pitPretradePreTradePolicyApplyAccountAdjustment;

static OpenPitPretradePreTradePolicyFreeUserDataFn
    openpit_pretrade_pre_trade_policy_free_user_data_fn = pitPretradePreTradePolicyClose;

static OpenPitPretradePreTradePolicyCheckPreTradeStartFn
    openpit_pretrade_pre_trade_policy_check_pre_trade_start_dry_run_fn =
        pitPretradePreTradePolicyCheckPreTradeStartDryRun;

static OpenPitPretradePreTradePolicyPerformPreTradeCheckFn
    openpit_pretrade_pre_trade_policy_perform_pre_trade_check_dry_run_fn =
        pitPretradePreTradePolicyPerformPreTradeCheckDryRun;

static void* pitPretradePreTradePolicyCheckPreTradeStartFnAddr(void) {
    return &openpit_pretrade_pre_trade_policy_check_pre_trade_start_fn;
}

static void* pitPretradePreTradePolicyPerformPreTradeCheckFnAddr(void) {
    return &openpit_pretrade_pre_trade_policy_perform_pre_trade_check_fn;
}

static void* pitPretradePreTradePolicyApplyReportFnAddr(void) {
    return &openpit_pretrade_pre_trade_policy_apply_execution_report_fn;
}

static void* pitPretradePreTradePolicyApplyAccountAdjustmentFnAddr(void) {
    return &openpit_pretrade_pre_trade_policy_apply_account_adjustment_fn;
}

static void* pitPretradePreTradePolicyFreeUserDataFnAddr(void) {
    return &openpit_pretrade_pre_trade_policy_free_user_data_fn;
}

static void* pitPretradePreTradePolicyCheckPreTradeStartDryRunFnAddr(void) {
    return &openpit_pretrade_pre_trade_policy_check_pre_trade_start_dry_run_fn;
}

static void* pitPretradePreTradePolicyPerformPreTradeCheckDryRunFnAddr(void) {
    return &openpit_pretrade_pre_trade_policy_perform_pre_trade_check_dry_run_fn;
}
*/
import "C"
import "unsafe"

// PreTradePolicyCheckPreTradeStartFnAddr returns the address of a
// OpenPitPretradePreTradePolicyCheckPreTradeStartFn variable holding the
// check-pre-trade-start callback.
// Pass the result to native.CreatePretradeCustomPreTradePolicy.
func PreTradePolicyCheckPreTradeStartFnAddr() unsafe.Pointer {
	return C.pitPretradePreTradePolicyCheckPreTradeStartFnAddr()
}

// PreTradePolicyPerformPreTradeCheckFnAddr returns the address of a
// OpenPitPretradePreTradePolicyPerformPreTradeCheckFn variable holding the
// perform-pre-trade-check callback.
// Pass the result to native.CreatePretradeCustomPreTradePolicy.
func PreTradePolicyPerformPreTradeCheckFnAddr() unsafe.Pointer {
	return C.pitPretradePreTradePolicyPerformPreTradeCheckFnAddr()
}

// PreTradePolicyApplyReportFnAddr returns the address of a
// OpenPitPretradePreTradePolicyApplyExecutionReportFn variable holding the
// apply callback.
// Pass the result to native.CreatePretradeCustomPreTradePolicy.
func PreTradePolicyApplyReportFnAddr() unsafe.Pointer {
	return C.pitPretradePreTradePolicyApplyReportFnAddr()
}

// PreTradePolicyApplyAccountAdjustmentFnAddr returns the address of a
// OpenPitPretradePreTradePolicyApplyAccountAdjustmentFn variable holding the
// apply-account-adjustment callback.
// Pass the result to native.CreatePretradeCustomPreTradePolicy.
func PreTradePolicyApplyAccountAdjustmentFnAddr() unsafe.Pointer {
	return C.pitPretradePreTradePolicyApplyAccountAdjustmentFnAddr()
}

// PreTradePolicyFreeUserDataFnAddr returns the address of a
// OpenPitPretradePreTradePolicyFreeUserDataFn variable holding the free
// callback.
// Pass the result to native.CreatePretradeCustomPreTradePolicy.
func PreTradePolicyFreeUserDataFnAddr() unsafe.Pointer {
	return C.pitPretradePreTradePolicyFreeUserDataFnAddr()
}

// PreTradePolicyCheckPreTradeStartDryRunFnAddr returns the address of a
// OpenPitPretradePreTradePolicyCheckPreTradeStartFn variable holding the
// dry-run check-pre-trade-start callback.
// Pass the result to native.CreatePretradeCustomPreTradePolicyWithDryRun.
func PreTradePolicyCheckPreTradeStartDryRunFnAddr() unsafe.Pointer {
	return C.pitPretradePreTradePolicyCheckPreTradeStartDryRunFnAddr()
}

// PreTradePolicyPerformPreTradeCheckDryRunFnAddr returns the address of a
// OpenPitPretradePreTradePolicyPerformPreTradeCheckFn variable holding the
// dry-run perform-pre-trade-check callback.
// Pass the result to native.CreatePretradeCustomPreTradePolicyWithDryRun.
func PreTradePolicyPerformPreTradeCheckDryRunFnAddr() unsafe.Pointer {
	return C.pitPretradePreTradePolicyPerformPreTradeCheckDryRunFnAddr()
}
