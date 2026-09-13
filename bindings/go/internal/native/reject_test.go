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
		0,
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
			0,
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
	if PretradeRejectGetUserData(outOfBounds) != 0 {
		t.Fatalf("PretradeRejectGetUserData(outOfBounds) = %v, want 0", PretradeRejectGetUserData(outOfBounds))
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
			0,
		),
	)
	if ok {
		t.Fatal("PretradeRejectListPush() = true, want false")
	}
	if got := PretradeRejectListLen(list); got != 0 {
		t.Fatalf("PretradeRejectListLen() = %d, want 0", got)
	}
}

// A full-width token has its top bit set, so any narrowing conversion on the
// way through the native lists would lose it.
func TestUserDataTokenRoundTripsThroughNativeLists(t *testing.T) {
	const token = ^uintptr(0)

	rejects := CreatePretradeRejectList(1)
	t.Cleanup(func() { DestroyPretradeRejectList(rejects) })
	if !PretradeRejectListPush(
		rejects,
		CreatePretradeReject(
			RejectCodeOther,
			RejectScopeOrder,
			NewStringView("policy"),
			NewStringView("reason"),
			NewStringView("details"),
			token,
		),
	) {
		t.Fatal("PretradeRejectListPush() = false, want true")
	}
	if got := PretradeRejectGetUserData(PretradeRejectListGet(rejects, 0)); got != token {
		t.Fatalf("reject user data = %#x, want %#x", got, token)
	}

	blocks := CreatePretradeAccountBlockList(1)
	t.Cleanup(func() { DestroyPretradeAccountBlockList(blocks) })
	PretradeAccountBlockListPush(
		blocks,
		CreatePretradeAccountBlock(
			RejectCodeOther,
			NewStringView("policy"),
			NewStringView("reason"),
			NewStringView("details"),
			token,
		),
	)
	if got := PretradeAccountBlockGetUserData(PretradeAccountBlockListGet(blocks, 0)); got != token {
		t.Fatalf("account block user data = %#x, want %#x", got, token)
	}
}

const (
	// stackGrowthDepth frames of at least stackFrameBytes each outgrow any
	// initial goroutine stack, so growStack forces the stack to be copied.
	stackGrowthDepth = 2000
	stackFrameBytes  = 256
)

//go:noinline
func growStack(depth int) int {
	var frame [stackFrameBytes]byte
	frame[depth%len(frame)] = 1
	if depth == 0 {
		return int(frame[0])
	}
	return growStack(depth-1) + int(frame[(depth+1)%len(frame)])
}

// A stack copy aborts the process when a pointer-typed slot of a live frame
// holds a value below 4096, so a small token must never travel as a pointer.
func TestSmallUserDataTokenSurvivesStackCopy(t *testing.T) {
	const token uintptr = 1

	reject := CreatePretradeReject(
		RejectCodeOther,
		RejectScopeOrder,
		NewStringView("policy"),
		NewStringView("reason"),
		NewStringView("details"),
		token,
	)
	block := CreatePretradeAccountBlock(
		RejectCodeOther,
		NewStringView("policy"),
		NewStringView("reason"),
		NewStringView("details"),
		token,
	)

	growStack(stackGrowthDepth)

	if got := PretradeRejectGetUserData(reject); got != token {
		t.Fatalf("reject user data = %#x, want %#x", got, token)
	}
	if got := PretradeAccountBlockGetUserData(block); got != token {
		t.Fatalf("account block user data = %#x, want %#x", got, token)
	}
}
