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

extern PitRejectList* pitAccountAdjustmentPolicyApply(
    const PitAccountAdjustmentContext* ctx,
    PitParamAccountId account_id,
    const PitAccountAdjustment* adjustment,
    PitMutations* mutations,
    void* user_data);

extern void pitAccountAdjustmentPolicyClose(void* user_data);

static void* pit_account_adjustment_policy_apply_fn =
    (void*)pitAccountAdjustmentPolicyApply;

static PitAccountAdjustmentPolicyFreeUserDataFn
    pit_account_adjustment_policy_free_user_data_fn = pitAccountAdjustmentPolicyClose;

static void* pitAccountAdjustmentPolicyApplyFnAddr(void) {
    return &pit_account_adjustment_policy_apply_fn;
}

static void* pitAccountAdjustmentPolicyFreeUserDataFnAddr(void) {
    return &pit_account_adjustment_policy_free_user_data_fn;
}
*/
import "C"
import "unsafe"

// AccountAdjustmentPolicyApplyFnAddr returns the address of a
// PitAccountAdjustmentPolicyApplyFn variable holding the apply callback.
// Pass the result to native.CreateCustomAccountAdjustmentPolicy.
func AccountAdjustmentPolicyApplyFnAddr() unsafe.Pointer {
	return C.pitAccountAdjustmentPolicyApplyFnAddr()
}

// AccountAdjustmentPolicyFreeUserDataFnAddr returns the address of a
// PitAccountAdjustmentPolicyFreeUserDataFn variable holding the free callback.
// Pass the result to native.CreateCustomAccountAdjustmentPolicy.
func AccountAdjustmentPolicyFreeUserDataFnAddr() unsafe.Pointer {
	return C.pitAccountAdjustmentPolicyFreeUserDataFnAddr()
}
