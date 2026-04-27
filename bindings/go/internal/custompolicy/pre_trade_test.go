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
	"go.openpit.dev/openpit/tx"
)

type fakePreTradePolicy struct {
	name string
}

func (p *fakePreTradePolicy) Close() {}

func (p *fakePreTradePolicy) Name() string { return p.name }

func (p *fakePreTradePolicy) PerformPreTradeCheck(
	_ pretrade.Context,
	_ model.Order,
	_ tx.Mutations,
) reject.List {
	return nil
}

func (p *fakePreTradePolicy) ApplyExecutionReport(_ model.ExecutionReport) bool {
	return false
}

func TestStartPreTradeSuccess(t *testing.T) {
	policy := &fakePreTradePolicy{name: "test-pre-trade-policy"}
	handle, err := StartPreTrade(policy)
	if err != nil {
		t.Fatalf("StartPreTrade() error = %v, want nil", err)
	}
	if handle == nil {
		t.Fatal("StartPreTrade() = nil, want non-nil")
	}
	t.Cleanup(func() { native.DestroyPretradePreTradePolicy(handle) })
}

func TestStartPreTradeErrorOnInvalidName(t *testing.T) {
	policy := &fakePreTradePolicy{name: ""}
	handle, err := StartPreTrade(policy)
	if handle != nil {
		native.DestroyPretradePreTradePolicy(handle)
		t.Fatal("StartPreTrade() handle != nil, want nil on invalid name")
	}
	if err == nil {
		t.Fatal("StartPreTrade() error = nil, want non-nil for invalid name")
	}
}
