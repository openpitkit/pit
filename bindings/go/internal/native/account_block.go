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

//------------------------------------------------------------------------------
// AccountBlockError

func DestroyAccountBlockError(err AccountBlockError) {
	C.openpit_destroy_account_block_error(err)
}

func AccountBlockErrorGetMessage(err AccountBlockError) string {
	return newStringView(C.openpit_account_block_error_get_message(err)).Safe()
}

func AccountBlockErrorGetKind(err AccountBlockError) AccountBlockErrorKind {
	return C.openpit_account_block_error_get_kind(err)
}

// AccountBlockErrorGetAccount returns the account of the error, and false unless
// the error kind is AccountNotBlocked.
func AccountBlockErrorGetAccount(err AccountBlockError) (ParamAccountID, bool) {
	var out ParamAccountID
	ok := bool(C.openpit_account_block_error_get_account(err, &out))
	return out, ok
}

// AccountBlockErrorGetGroup returns the group of the error, and false unless the
// error kind is GroupNotBlocked.
func AccountBlockErrorGetGroup(err AccountBlockError) (ParamAccountGroupID, bool) {
	var out ParamAccountGroupID
	ok := bool(C.openpit_account_block_error_get_group(err, &out))
	return out, ok
}

//------------------------------------------------------------------------------
// Engine account-block operations

// EngineBlockAccount blocks account with reason. Re-blocking an already-blocked
// account keeps the first reason. It is infallible.
func EngineBlockAccount(engine Engine, account ParamAccountID, reason string) {
	C.openpit_engine_block_account(engine, account, importString(reason))
}

// EngineUnblockAccount unblocks account. Unblocking an account that is not
// blocked is a no-op. It is infallible.
func EngineUnblockAccount(engine Engine, account ParamAccountID) {
	C.openpit_engine_unblock_account(engine, account)
}

// EngineReplaceAccountBlockReason replaces the block reason of account.
//
// On success returns nil. On a domain error returns a non-nil AccountBlockError
// (caller must release with DestroyAccountBlockError) carrying kind
// AccountNotBlocked.
func EngineReplaceAccountBlockReason(
	engine Engine,
	account ParamAccountID,
	reason string,
) AccountBlockError {
	var outError AccountBlockError
	if !C.openpit_engine_replace_account_block_reason(
		engine,
		account,
		importString(reason),
		&outError, //nolint:gocritic // CGo out-parameter requires address-of operator
	) {
		return outError
	}
	return nil
}

// EngineBlockAccountGroup blocks group with reason. Re-blocking an
// already-blocked group keeps the first reason.
//
// On success returns nil. On a domain error returns a non-nil AccountBlockError
// (caller must release with DestroyAccountBlockError) carrying kind
// ReservedGroup.
func EngineBlockAccountGroup(
	engine Engine,
	group ParamAccountGroupID,
	reason string,
) AccountBlockError {
	var outError AccountBlockError
	if !C.openpit_engine_block_account_group(
		engine,
		group,
		importString(reason),
		&outError, //nolint:gocritic // CGo out-parameter requires address-of operator
	) {
		return outError
	}
	return nil
}

// EngineUnblockAccountGroup unblocks group. Unblocking a group that is not
// blocked is a no-op.
//
// On success returns nil. On a domain error returns a non-nil AccountBlockError
// (caller must release with DestroyAccountBlockError) carrying kind
// ReservedGroup.
func EngineUnblockAccountGroup(
	engine Engine,
	group ParamAccountGroupID,
) AccountBlockError {
	var outError AccountBlockError
	if !C.openpit_engine_unblock_account_group(
		engine,
		group,
		&outError, //nolint:gocritic // CGo out-parameter requires address-of operator
	) {
		return outError
	}
	return nil
}

// EngineReplaceAccountGroupBlockReason replaces the block reason of group.
//
// On success returns nil. On a domain error returns a non-nil AccountBlockError
// (caller must release with DestroyAccountBlockError) carrying kind
// ReservedGroup or GroupNotBlocked.
func EngineReplaceAccountGroupBlockReason(
	engine Engine,
	group ParamAccountGroupID,
	reason string,
) AccountBlockError {
	var outError AccountBlockError
	if !C.openpit_engine_replace_account_group_block_reason(
		engine,
		group,
		importString(reason),
		&outError, //nolint:gocritic // CGo out-parameter requires address-of operator
	) {
		return outError
	}
	return nil
}

//------------------------------------------------------------------------------
// Engine account-currency operations

func EngineSetAccountCurrency(
	engine Engine,
	account ParamAccountID,
	asset string,
) error {
	var outError SharedString
	if !C.openpit_engine_set_account_currency(
		engine,
		account,
		importString(asset),
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	) {
		return consumeSharedStringAsError(
			outError,
			"openpit_engine_set_account_currency failed",
		)
	}
	return nil
}

func EngineClearAccountCurrency(engine Engine, account ParamAccountID) {
	C.openpit_engine_clear_account_currency(engine, account)
}

func EngineSetAccountGroupCurrency(
	engine Engine,
	group ParamAccountGroupID,
	asset string,
) error {
	var outError SharedString
	if !C.openpit_engine_set_account_group_currency(
		engine,
		group,
		importString(asset),
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	) {
		return consumeSharedStringAsError(
			outError,
			"openpit_engine_set_account_group_currency failed",
		)
	}
	return nil
}

func EngineClearAccountGroupCurrency(engine Engine, group ParamAccountGroupID) {
	C.openpit_engine_clear_account_group_currency(engine, group)
}
