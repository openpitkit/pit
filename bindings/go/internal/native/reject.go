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
#include "pit.h"
*/
import "C"

import "unsafe"

//------------------------------------------------------------------------------
// Reject

func CreateReject(
	code RejectCode,
	scope RejectScope,
	policy StringView,
	reason StringView,
	details StringView,
	userData unsafe.Pointer,
) Reject {
	return Reject{
		policy:    policy.value,
		reason:    reason.value,
		details:   details.value,
		user_data: userData,
		code:      code,
		scope:     scope,
	}
}

func RejectGetCode(reject Reject) RejectCode {
	return reject.code
}

func RejectGetScope(reject Reject) RejectScope {
	return reject.scope
}

func RejectGetPolicy(reject Reject) StringView {
	return newStringView(reject.policy)
}

func RejectGetReason(reject Reject) StringView {
	return newStringView(reject.reason)
}

func RejectGetDetails(reject Reject) StringView {
	return newStringView(reject.details)
}

func RejectGetUserData(reject Reject) unsafe.Pointer {
	return reject.user_data
}

//------------------------------------------------------------------------------
// RejectList

func CreateRejectList(reserve int) RejectList {
	if reserve < 0 {
		reserve = 0
	}
	return C.pit_create_reject_list(C.size_t(reserve))
}

func DestroyRejectList(rejects RejectList) {
	C.pit_destroy_reject_list(rejects)
}

func RejectListPush(list RejectList, reject Reject) {
	C.pit_reject_list_push(list, reject)
}

func RejectListLen(list RejectList) int {
	return int(C.pit_reject_list_len(list))
}

func RejectListGet(list RejectList, index int) Reject {
	var out Reject
	if !C.pit_reject_list_get(list, C.size_t(index), &out) {
		return Reject{}
	}
	return out
}

//------------------------------------------------------------------------------
