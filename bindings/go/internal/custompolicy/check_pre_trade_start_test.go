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

package custompolicy

import (
	"testing"

	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/pretrade"
	"go.openpit.dev/openpit/reject"
)

type fakeCheckPreTradeStartPolicy struct {
	name string
}

func (fakeCheckPreTradeStartPolicy) Close() {}

func (p fakeCheckPreTradeStartPolicy) Name() string { return p.name }

func (fakeCheckPreTradeStartPolicy) CheckPreTradeStart(
	_ pretrade.Context,
	_ model.Order,
) []reject.Reject {
	return nil
}

func (fakeCheckPreTradeStartPolicy) ApplyExecutionReport(_ model.ExecutionReport) bool {
	return false
}

func TestStartCheckPreTradeStartSuccess(t *testing.T) {
	policy := &fakeCheckPreTradeStartPolicy{name: "test-check-start-policy"}
	handle, err := StartCheckPreTradeStart(policy)
	if err != nil {
		t.Fatalf("StartCheckPreTradeStart() error = %v, want nil", err)
	}
	if handle == nil {
		t.Fatal("StartCheckPreTradeStart() = nil, want non-nil")
	}
	t.Cleanup(func() { native.DestroyPretradeCheckPreTradeStartPolicy(handle) })
}

func TestStartCheckPreTradeStartErrorOnInvalidName(t *testing.T) {
	policy := &fakeCheckPreTradeStartPolicy{name: ""}
	handle, err := StartCheckPreTradeStart(policy)
	if handle != nil {
		native.DestroyPretradeCheckPreTradeStartPolicy(handle)
		t.Fatal("StartCheckPreTradeStart() handle != nil, want nil on invalid name")
	}
	if err == nil {
		t.Fatal("StartCheckPreTradeStart() error = nil, want non-nil for invalid name")
	}
}
