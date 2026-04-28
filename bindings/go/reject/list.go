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

package reject

import (
	"errors"

	"go.openpit.dev/openpit/internal/native"
)

func NewList(rejects ...Reject) []Reject {
	return rejects
}

func NewSingleItemList(
	code Code, // stable machine-readable reject code
	policy string, // policy name that produced the reject
	reason string, // human-readable reject reason
	details string, // case-specific reject details
	scope Scope, // reject scope
) []Reject {
	return NewList(New(code, policy, reason, details, scope))
}

func NewListFromHandle(handle native.RejectList) ([]Reject, error) {
	len := native.RejectListLen(handle)
	if len == 0 {
		return nil, errors.New("reject list is not provided")
	}
	result := make([]Reject, len)
	for i := 0; i < len; i++ {
		result[i] = NewFromHandle(native.RejectListGet(handle, i))
	}
	return result, nil
}
