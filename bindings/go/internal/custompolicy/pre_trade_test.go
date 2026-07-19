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

package custompolicy

import (
	"testing"

	"go.openpit.dev/openpit/accountadjustment"
	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pretrade"
	"go.openpit.dev/openpit/reject"
	"go.openpit.dev/openpit/tx"
)

type fakePreTradePolicy struct {
	name string
}

func (fakePreTradePolicy) Close() {}

func (p fakePreTradePolicy) Name() string { return p.name }

func (fakePreTradePolicy) PolicyGroupID() model.PolicyGroupID { return model.DefaultPolicyGroupID }

func (fakePreTradePolicy) CheckPreTradeStart(
	_ pretrade.Context,
	_ model.Order,
) []reject.Reject {
	return nil
}

func (fakePreTradePolicy) PerformPreTradeCheck(
	_ pretrade.Context,
	_ model.Order,
	_ tx.Mutations,
	_ pretrade.Result,
) []reject.Reject {
	return nil
}

func (fakePreTradePolicy) ApplyExecutionReport(
	_ pretrade.PostTradeContext,
	_ model.ExecutionReport,
	_ pretrade.PostTradeAdjustments,
	_ pretrade.PostTradePnls,
) []reject.AccountBlock {
	return nil
}

func (fakePreTradePolicy) ApplyAccountAdjustment(
	_ accountadjustment.Context,
	_ param.AccountID,
	_ model.AccountAdjustment,
	_ tx.Mutations,
	_ pretrade.AccountOutcomes,
) (pretrade.PolicyAccountAdjustmentResult, []reject.Reject) {
	return pretrade.PolicyAccountAdjustmentResult{}, nil
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

// ---------------------------------------------------------------------------
// DryRunPolicy optional interface detection

type fakeDryRunPreTradePolicy struct {
	fakePreTradePolicy
	checkDryRunCalled   bool
	performDryRunCalled bool
}

var _ pretrade.DryRunPolicy = (*fakeDryRunPreTradePolicy)(nil)

func (p *fakeDryRunPreTradePolicy) CheckPreTradeStartDryRun(
	_ pretrade.Context,
	_ model.Order,
) []reject.Reject {
	p.checkDryRunCalled = true
	return nil
}

func (p *fakeDryRunPreTradePolicy) PerformPreTradeCheckDryRun(
	_ pretrade.Context,
	_ model.Order,
	_ tx.Mutations,
	_ pretrade.Result,
) []reject.Reject {
	p.performDryRunCalled = true
	return nil
}

func TestStartPreTradeWithDryRunSucceeds(t *testing.T) {
	policy := &fakeDryRunPreTradePolicy{fakePreTradePolicy: fakePreTradePolicy{name: "dry-run-policy"}}
	handle, err := StartPreTrade(policy)
	if err != nil {
		t.Fatalf("StartPreTrade() error = %v, want nil for policy with DryRunPolicy", err)
	}
	if handle == nil {
		t.Fatal("StartPreTrade() = nil, want non-nil")
	}
	t.Cleanup(func() { native.DestroyPretradePreTradePolicy(handle) })
}

func TestStartPreTradeWithDryRunSetsField(t *testing.T) {
	policy := &fakeDryRunPreTradePolicy{fakePreTradePolicy: fakePreTradePolicy{name: "dry-run-field-policy"}}
	impl := &PreTrade{impl: policy}
	if dryRun, ok := pretrade.Policy(policy).(pretrade.DryRunPolicy); ok {
		impl.dryRun = dryRun
	}
	if impl.dryRun == nil {
		t.Fatal("dryRun field = nil, want non-nil when policy implements DryRunPolicy")
	}
}

func TestStartPreTradeWithoutDryRunLeavesFieldNil(t *testing.T) {
	policy := &fakePreTradePolicy{name: "non-dry-run-policy"}
	impl := &PreTrade{impl: policy}
	if _, ok := pretrade.Policy(policy).(pretrade.DryRunPolicy); ok {
		t.Fatal("fakePreTradePolicy unexpectedly satisfies DryRunPolicy")
	}
	if impl.dryRun != nil {
		t.Fatal("dryRun field != nil, want nil when policy does not implement DryRunPolicy")
	}
}
