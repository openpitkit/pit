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

import "testing"

func TestCreateRejectListClampsNegativeReserve(t *testing.T) {
	list := CreateRejectList(-3)
	t.Cleanup(func() { DestroyRejectList(list) })

	reject := CreateReject(
		RejectCodeOther,
		RejectScopeOrder,
		NewStringView("policy"),
		NewStringView("reason"),
		NewStringView("details"),
		nil,
	)
	RejectListPush(list, reject)

	if got := RejectListLen(list); got != 1 {
		t.Fatalf("RejectListLen() = %d, want 1", got)
	}
}

func TestRejectListGetReturnsZeroValueOutOfBounds(t *testing.T) {
	list := CreateRejectList(1)
	t.Cleanup(func() { DestroyRejectList(list) })

	RejectListPush(
		list,
		CreateReject(
			RejectCodeOther,
			RejectScopeOrder,
			NewStringView("policy"),
			NewStringView("reason"),
			NewStringView("details"),
			nil,
		),
	)

	outOfBounds := RejectListGet(list, 10)
	if RejectGetCode(outOfBounds) != 0 {
		t.Fatalf("RejectGetCode(outOfBounds) = %v, want 0", RejectGetCode(outOfBounds))
	}
	if RejectGetScope(outOfBounds) != 0 {
		t.Fatalf("RejectGetScope(outOfBounds) = %v, want 0", RejectGetScope(outOfBounds))
	}
	if RejectGetPolicy(outOfBounds).IsSet() {
		t.Fatal("RejectGetPolicy(outOfBounds).IsSet() = true, want false")
	}
	if RejectGetReason(outOfBounds).IsSet() {
		t.Fatal("RejectGetReason(outOfBounds).IsSet() = true, want false")
	}
	if RejectGetDetails(outOfBounds).IsSet() {
		t.Fatal("RejectGetDetails(outOfBounds).IsSet() = true, want false")
	}
	if RejectGetUserData(outOfBounds) != nil {
		t.Fatalf("RejectGetUserData(outOfBounds) = %v, want nil", RejectGetUserData(outOfBounds))
	}
}
