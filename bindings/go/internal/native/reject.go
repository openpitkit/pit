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
// PretradeReject

func CreatePretradeReject(
	code PretradeRejectCode,
	scope PretradeRejectScope,
	policy StringView,
	reason StringView,
	details StringView,
	userData unsafe.Pointer,
) PretradeReject {
	return PretradeReject{
		policy:    policy.value,
		reason:    reason.value,
		details:   details.value,
		user_data: userData,
		code:      code,
		scope:     scope,
	}
}

func PretradeRejectGetCode(reject PretradeReject) PretradeRejectCode {
	return reject.code
}

func PretradeRejectGetScope(reject PretradeReject) PretradeRejectScope {
	return reject.scope
}

func PretradeRejectGetPolicy(reject PretradeReject) StringView {
	return newStringView(reject.policy)
}

func PretradeRejectGetReason(reject PretradeReject) StringView {
	return newStringView(reject.reason)
}

func PretradeRejectGetDetails(reject PretradeReject) StringView {
	return newStringView(reject.details)
}

func PretradeRejectGetUserData(reject PretradeReject) unsafe.Pointer {
	return reject.user_data
}

//------------------------------------------------------------------------------
// PretradeRejectList

func CreatePretradeRejectList(reserve int) PretradeRejectList {
	if reserve < 0 {
		reserve = 0
	}
	return C.openpit_pretrade_create_reject_list(C.size_t(reserve))
}

func DestroyPretradeRejectList(rejects PretradeRejectList) {
	C.openpit_pretrade_destroy_reject_list(rejects)
}

func PretradeRejectListPush(list PretradeRejectList, reject PretradeReject) {
	C.openpit_pretrade_reject_list_push(list, reject)
}

func PretradeRejectListLen(list PretradeRejectList) int {
	return int(C.openpit_pretrade_reject_list_len(list))
}

func PretradeRejectListGet(list PretradeRejectList, index int) PretradeReject {
	var out PretradeReject
	if !C.openpit_pretrade_reject_list_get(list, C.size_t(index), &out) { //nolint:gocritic
		return PretradeReject{}
	}
	return out
}

//------------------------------------------------------------------------------
// PretradeAccountBlock

func CreatePretradeAccountBlock(
	code PretradeRejectCode,
	policy StringView,
	reason StringView,
	details StringView,
	userData unsafe.Pointer,
) PretradeAccountBlock {
	return PretradeAccountBlock{
		policy:    policy.value,
		reason:    reason.value,
		details:   details.value,
		user_data: userData,
		code:      code,
	}
}

func PretradeAccountBlockGetCode(block PretradeAccountBlock) PretradeRejectCode {
	return block.code
}

func PretradeAccountBlockGetPolicy(block PretradeAccountBlock) StringView {
	return newStringView(block.policy)
}

func PretradeAccountBlockGetReason(block PretradeAccountBlock) StringView {
	return newStringView(block.reason)
}

func PretradeAccountBlockGetDetails(block PretradeAccountBlock) StringView {
	return newStringView(block.details)
}

func PretradeAccountBlockGetUserData(block PretradeAccountBlock) unsafe.Pointer {
	return block.user_data
}

//------------------------------------------------------------------------------
// PretradeAccountBlockList

func CreatePretradeAccountBlockList(reserve int) PretradeAccountBlockList {
	if reserve < 0 {
		reserve = 0
	}
	return C.openpit_pretrade_create_account_block_list(C.size_t(reserve))
}

func DestroyPretradeAccountBlockList(blocks PretradeAccountBlockList) {
	C.openpit_pretrade_destroy_account_block_list(blocks)
}

func PretradeAccountBlockListPush(list PretradeAccountBlockList, block PretradeAccountBlock) {
	C.openpit_pretrade_account_block_list_push(list, block)
}

func PretradeAccountBlockListLen(list PretradeAccountBlockList) int {
	return int(C.openpit_pretrade_account_block_list_len(list))
}

func PretradeAccountBlockListGet(list PretradeAccountBlockList, index int) PretradeAccountBlock {
	var out PretradeAccountBlock
	if !C.openpit_pretrade_account_block_list_get(list, C.size_t(index), &out) { //nolint:gocritic
		return PretradeAccountBlock{}
	}
	return out
}

//------------------------------------------------------------------------------
