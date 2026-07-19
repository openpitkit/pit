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
	list := CreatePretradeRejectList(-3)
	t.Cleanup(func() { DestroyPretradeRejectList(list) })

	reject := CreatePretradeReject(
		RejectCodeOther,
		RejectScopeOrder,
		NewStringView("policy"),
		NewStringView("reason"),
		NewStringView("details"),
		nil,
	)
	if !PretradeRejectListPush(list, reject) {
		t.Fatal("PretradeRejectListPush() = false, want true")
	}

	if got := PretradeRejectListLen(list); got != 1 {
		t.Fatalf("PretradeRejectListLen() = %d, want 1", got)
	}
}

func TestRejectListGetReturnsZeroValueOutOfBounds(t *testing.T) {
	list := CreatePretradeRejectList(1)
	t.Cleanup(func() { DestroyPretradeRejectList(list) })

	if !PretradeRejectListPush(
		list,
		CreatePretradeReject(
			RejectCodeOther,
			RejectScopeOrder,
			NewStringView("policy"),
			NewStringView("reason"),
			NewStringView("details"),
			nil,
		),
	) {
		t.Fatal("PretradeRejectListPush() = false, want true")
	}

	outOfBounds := PretradeRejectListGet(list, 10)
	if PretradeRejectGetCode(outOfBounds) != 0 {
		t.Fatalf("PretradeRejectGetCode(outOfBounds) = %v, want 0", PretradeRejectGetCode(outOfBounds))
	}
	if PretradeRejectGetScope(outOfBounds) != 0 {
		t.Fatalf("PretradeRejectGetScope(outOfBounds) = %v, want 0", PretradeRejectGetScope(outOfBounds))
	}
	if PretradeRejectGetPolicy(outOfBounds).IsSet() {
		t.Fatal("PretradeRejectGetPolicy(outOfBounds).IsSet() = true, want false")
	}
	if PretradeRejectGetReason(outOfBounds).IsSet() {
		t.Fatal("PretradeRejectGetReason(outOfBounds).IsSet() = true, want false")
	}
	if PretradeRejectGetDetails(outOfBounds).IsSet() {
		t.Fatal("PretradeRejectGetDetails(outOfBounds).IsSet() = true, want false")
	}
	if PretradeRejectGetUserData(outOfBounds) != nil {
		t.Fatalf("PretradeRejectGetUserData(outOfBounds) = %v, want nil", PretradeRejectGetUserData(outOfBounds))
	}
}

func TestRejectListPushRejectsUnknownScope(t *testing.T) {
	list := CreatePretradeRejectList(1)
	t.Cleanup(func() { DestroyPretradeRejectList(list) })

	ok := PretradeRejectListPush(
		list,
		CreatePretradeReject(
			RejectCodeOther,
			PretradeRejectScope(255),
			NewStringView("policy"),
			NewStringView("reason"),
			NewStringView("details"),
			nil,
		),
	)
	if ok {
		t.Fatal("PretradeRejectListPush() = true, want false")
	}
	if got := PretradeRejectListLen(list); got != 0 {
		t.Fatalf("PretradeRejectListLen() = %d, want 0", got)
	}
}
