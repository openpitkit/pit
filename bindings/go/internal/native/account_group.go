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

package native

/*
#include "openpit.h"
*/
import "C"

import "unsafe"

//------------------------------------------------------------------------------
// ParamAccountGroupID

func CreateParamAccountGroupIDFromUint32(value uint32) (ParamAccountGroupID, error) {
	var out ParamAccountGroupID
	var outError SharedString
	if !C.openpit_create_param_account_group_id_from_uint32(
		C.uint32_t(value),
		&out,
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	) {
		return 0,
			consumeSharedStringAsError(
				outError,
				"openpit_create_param_account_group_id_from_uint32 failed",
			)
	}
	return out, nil
}

func CreateParamAccountGroupIDFromString(value string) (ParamAccountGroupID, error) {
	var out ParamAccountGroupID
	var outError SharedString
	if !C.openpit_create_param_account_group_id_from_string(
		importString(value),
		&out,
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	) {
		return 0,
			consumeSharedStringAsError(
				outError,
				"openpit_create_param_account_group_id_from_string failed",
			)
	}
	return out, nil
}

//------------------------------------------------------------------------------
// AccountGroupError

func DestroyAccountGroupError(err AccountGroupError) {
	C.openpit_destroy_account_group_error(err)
}

func AccountGroupErrorGetMessage(err AccountGroupError) string {
	return newStringView(C.openpit_account_group_error_get_message(err)).Safe()
}

func AccountGroupErrorGetAccount(err AccountGroupError) ParamAccountID {
	return C.openpit_account_group_error_get_account(err)
}

// AccountGroupErrorGetCurrentGroup returns the current group of the conflicting
// account, and false when the account is not in any group (unregister conflict).
func AccountGroupErrorGetCurrentGroup(err AccountGroupError) (ParamAccountGroupID, bool) {
	var out ParamAccountGroupID
	ok := bool(C.openpit_account_group_error_get_current_group(err, &out))
	return out, ok
}

//------------------------------------------------------------------------------
// Engine account-group operations

// EngineRegisterAccountGroup atomically registers accounts into group.
//
// On success returns nil, nil. On a domain conflict returns a non-nil
// AccountGroupError (caller must release with DestroyAccountGroupError) and
// nil error. On a transport failure returns nil, error.
func EngineRegisterAccountGroup(
	engine Engine,
	accounts []ParamAccountID,
	group ParamAccountGroupID,
) (AccountGroupError, error) {
	var accountsPtr *C.OpenPitParamAccountId
	if len(accounts) > 0 {
		accountsPtr = (*C.OpenPitParamAccountId)(unsafe.Pointer(&accounts[0]))
	}
	var outGroupError AccountGroupError
	var outError SharedString
	if !C.openpit_engine_register_account_group(
		engine,
		accountsPtr,
		C.size_t(len(accounts)),
		group,
		&outGroupError,
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	) {
		if outGroupError != nil {
			return outGroupError, nil
		}
		return nil, consumeSharedStringAsError(outError, "openpit_engine_register_account_group failed")
	}
	return nil, nil //nolint:nilnil // both nil signals success; the function's contract documents nil error as "not an error"
}

// EngineUnregisterAccountGroup atomically removes accounts from group.
//
// On success returns nil, nil. On a domain conflict returns a non-nil
// AccountGroupError (caller must release with DestroyAccountGroupError) and
// nil error. On a transport failure returns nil, error.
func EngineUnregisterAccountGroup(
	engine Engine,
	accounts []ParamAccountID,
	group ParamAccountGroupID,
) (AccountGroupError, error) {
	var accountsPtr *C.OpenPitParamAccountId
	if len(accounts) > 0 {
		accountsPtr = (*C.OpenPitParamAccountId)(unsafe.Pointer(&accounts[0]))
	}
	var outGroupError AccountGroupError
	var outError SharedString
	if !C.openpit_engine_unregister_account_group(
		engine,
		accountsPtr,
		C.size_t(len(accounts)),
		group,
		&outGroupError,
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	) {
		if outGroupError != nil {
			return outGroupError, nil
		}
		return nil,
			consumeSharedStringAsError(outError, "openpit_engine_unregister_account_group failed")
	}
	return nil, nil //nolint:nilnil // both nil signals success; the function's contract documents nil error as "not an error"
}

// EngineAccountGroup returns the group of account, and false when it is not
// in any group.
func EngineAccountGroup(engine Engine, account ParamAccountID) (ParamAccountGroupID, bool) {
	var out ParamAccountGroupID
	ok := bool(C.openpit_engine_account_group(engine, account, &out))
	return out, ok
}

//------------------------------------------------------------------------------
// Context group accessors

func PretradeContextGetAccountGroup(ctx PretradeContext) (ParamAccountGroupID, bool) {
	var out ParamAccountGroupID
	ok := bool(C.openpit_pretrade_context_get_account_group(ctx, &out))
	return out, ok
}

func AccountAdjustmentContextGetAccountGroup(
	ctx AccountAdjustmentContext,
) (ParamAccountGroupID, bool) {
	var out ParamAccountGroupID
	ok := bool(C.openpit_account_adjustment_context_get_account_group(ctx, &out))
	return out, ok
}

func PostTradeContextGetAccountGroup(ctx PostTradeContext) (ParamAccountGroupID, bool) {
	var out ParamAccountGroupID
	ok := bool(C.openpit_post_trade_context_get_account_group(ctx, &out))
	return out, ok
}
