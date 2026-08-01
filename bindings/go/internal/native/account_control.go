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

package native

/*
#include "openpit.h"
*/
import "C"
import "unsafe"

//------------------------------------------------------------------------------
// AccountControl

func AccountControlBlock(control AccountControl, block PretradeAccountBlock) {
	C.openpit_account_control_block(control, block)
}

func AccountControlClone(control AccountControl) AccountControl {
	return C.openpit_account_control_clone(control)
}

func DestroyAccountControl(control AccountControl) {
	C.openpit_destroy_account_control(control)
}

func PretradeContextGetAccountControl(ctx PretradeContext) AccountControl {
	return C.openpit_pretrade_context_get_account_control(ctx)
}

func PretradeContextIsDropCopy(ctx PretradeContext) bool {
	return bool(C.openpit_pretrade_context_is_drop_copy(ctx))
}

func PretradeContextRecordDropCopyStartMutation(
	ctx PretradeContext,
	commitFnAddr unsafe.Pointer,
	rollbackFnAddr unsafe.Pointer,
	userData unsafe.Pointer,
	freeFnAddr unsafe.Pointer,
) error {
	var outError SharedString
	if !C.openpit_pretrade_context_record_drop_copy_start_mutation(
		ctx,
		*(*C.OpenPitMutationFn)(commitFnAddr),
		*(*C.OpenPitMutationFn)(rollbackFnAddr),
		userData,
		*(*C.OpenPitMutationFreeFn)(freeFnAddr),
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	) {
		return consumeSharedStringAsError(
			outError,
			"openpit_pretrade_context_record_drop_copy_start_mutation failed",
		)
	}
	return nil
}

func AccountAdjustmentContextGetAccountControl(ctx AccountAdjustmentContext) AccountControl {
	return C.openpit_account_adjustment_context_get_account_control(ctx)
}

//------------------------------------------------------------------------------
