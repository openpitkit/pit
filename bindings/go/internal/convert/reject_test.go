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
	"testing"

	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/reject"
)

func TestNewNativeRejectListOrNilReplacesInvalidScope(t *testing.T) {
	input := reject.New(
		reject.CodeRiskLimitExceeded,
		"custom-policy",
		"rejected",
		"original details",
		reject.Scope(255),
	)

	list := NewNativeRejectListOrNil([]reject.Reject{input})
	if list == nil {
		t.Fatal("NewNativeRejectListOrNil() = nil, want fallback reject")
	}
	defer native.DestroyPretradeRejectList(list)
	if got := native.PretradeRejectListLen(list); got != 1 {
		t.Fatalf("PretradeRejectListLen() = %d, want 1", got)
	}

	got := reject.NewFromHandle(native.PretradeRejectListGet(list, 0))
	if got.Code != reject.CodeSystemUnavailable {
		t.Errorf("fallback code = %v, want SystemUnavailable", got.Code)
	}
	if got.Scope != reject.ScopeOrder {
		t.Errorf("fallback scope = %v, want Order", got.Scope)
	}
	if got.Policy != "openpit.callback" {
		t.Errorf("fallback policy = %q, want openpit.callback", got.Policy)
	}
	if got.Reason != "custom policy callback failed" {
		t.Errorf("fallback reason = %q, want callback failure", got.Reason)
	}
	if got.Details != "reject scope is invalid" {
		t.Errorf("fallback details = %q, want invalid scope", got.Details)
	}
}

func TestNewNativeRejectListOrNilReplacesWholeBatchOnInvalidScope(t *testing.T) {
	valid := reject.New(
		reject.CodeRiskLimitExceeded,
		"custom-policy",
		"first reject",
		"must not survive the invalid callback result",
		reject.ScopeOrder,
	)
	invalid := reject.New(
		reject.CodeRiskLimitExceeded,
		"custom-policy",
		"invalid reject",
		"invalid scope invalidates the callback result",
		reject.Scope(255),
	)

	list := NewNativeRejectListOrNil([]reject.Reject{valid, invalid})
	if list == nil {
		t.Fatal("NewNativeRejectListOrNil() = nil, want fallback reject")
	}
	defer native.DestroyPretradeRejectList(list)
	if got := native.PretradeRejectListLen(list); got != 1 {
		t.Fatalf("PretradeRejectListLen() = %d, want one fallback reject", got)
	}

	got := reject.NewFromHandle(native.PretradeRejectListGet(list, 0))
	if got.Code != reject.CodeSystemUnavailable || got.Policy != "openpit.callback" {
		t.Fatalf("fallback reject = %#v", got)
	}
}
