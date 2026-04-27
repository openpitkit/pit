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
	"testing"
	"unsafe"

	"go.openpit.dev/openpit/internal/native"
)

func TestRejectWithUserDataReturnsCopyWithToken(t *testing.T) {
	base := New(
		CodeInvalidFieldValue,
		"policy-a",
		"reason-a",
		"details-a",
		ScopeOrder,
	)
	var token byte
	userData := unsafe.Pointer(&token)
	copy := base.WithUserData(userData)

	if base.UserData != nil {
		t.Fatalf("base UserData = %v, want nil", base.UserData)
	}
	if copy.UserData != userData {
		t.Fatalf("copy UserData = %v, want %v", copy.UserData, userData)
	}
	if copy.Code != base.Code {
		t.Fatalf("copy Code = %v, want %v", copy.Code, base.Code)
	}
	if copy.Scope != base.Scope {
		t.Fatalf("copy Scope = %v, want %v", copy.Scope, base.Scope)
	}
	if copy.Policy != base.Policy {
		t.Fatalf("copy Policy = %q, want %q", copy.Policy, base.Policy)
	}
	if copy.Reason != base.Reason {
		t.Fatalf("copy Reason = %q, want %q", copy.Reason, base.Reason)
	}
	if copy.Details != base.Details {
		t.Fatalf("copy Details = %q, want %q", copy.Details, base.Details)
	}
}

func TestRejectNewWithUserDataInitialisesAllFields(t *testing.T) {
	var token byte
	userData := unsafe.Pointer(&token)
	rej := NewWithUserData(
		CodeRiskLimitExceeded,
		"policy-b",
		"reason-b",
		"details-b",
		ScopeAccount,
		userData,
	)

	if rej.Code != CodeRiskLimitExceeded {
		t.Fatalf("Code = %v, want %v", rej.Code, CodeRiskLimitExceeded)
	}
	if rej.Scope != ScopeAccount {
		t.Fatalf("Scope = %v, want %v", rej.Scope, ScopeAccount)
	}
	if rej.Policy != "policy-b" {
		t.Fatalf("Policy = %q, want %q", rej.Policy, "policy-b")
	}
	if rej.Reason != "reason-b" {
		t.Fatalf("Reason = %q, want %q", rej.Reason, "reason-b")
	}
	if rej.Details != "details-b" {
		t.Fatalf("Details = %q, want %q", rej.Details, "details-b")
	}
	if rej.UserData != userData {
		t.Fatalf("UserData = %v, want %v", rej.UserData, userData)
	}
}

func TestNewListFromHandleReturnsErrorForEmptyList(t *testing.T) {
	handle := native.CreateRejectList(0)
	t.Cleanup(func() { native.DestroyRejectList(handle) })

	list, err := NewListFromHandle(handle)
	if list != nil {
		t.Fatalf("NewListFromHandle() list = %v, want nil", list)
	}
	if err == nil {
		t.Fatal("NewListFromHandle() error = nil, want non-nil")
	}
}
