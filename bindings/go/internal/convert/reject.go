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

package convert

import (
	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/reject"
)

// NewNativeRejectList copies callback rejects into a native list.
//
// A shared immutable empty list represents a successful callback without a
// heap allocation. A nil list is reserved for callback failure at the C
// boundary.
//
// A rejected push invalidates the whole callback result. The native list is
// replaced with one SystemUnavailable reject naming the real cause, so the
// engine never applies a partial decision and the diagnosis is not misleading.
func NewNativeRejectList(source []reject.Reject) native.PretradeRejectList {
	if len(source) == 0 {
		return native.AcceptPretradeRejectList()
	}
	result := native.CreatePretradeRejectList(len(source))
	for _, r := range source {
		// Reject data will be copied here, source reject must be existing before.
		if native.PretradeRejectListPush(result, r.NewHandle()) {
			continue
		}
		native.DestroyPretradeRejectList(result)
		fallback := reject.New(
			reject.CodeSystemUnavailable,
			"openpit.callback",
			"custom policy callback failed",
			"reject scope is invalid",
			reject.ScopeOrder,
		)
		result = native.CreatePretradeRejectList(1)
		if native.PretradeRejectListPush(result, fallback.NewHandle()) {
			return result
		}
		native.DestroyPretradeRejectList(result)
		return nil
	}
	return result
}

func NewNativeAccountBlockListOrNil(source []reject.AccountBlock) native.PretradeAccountBlockList {
	if len(source) == 0 {
		return nil
	}
	result := native.CreatePretradeAccountBlockList(len(source))
	for _, b := range source {
		native.PretradeAccountBlockListPush(result, b.NewHandle())
	}
	return result
}
