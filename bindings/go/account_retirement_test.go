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

package openpit

import (
	"errors"
	"testing"
	"time"

	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pretrade"
	"go.openpit.dev/openpit/pretrade/policies"
	"go.openpit.dev/openpit/reject"
	"go.openpit.dev/openpit/tx"
)

func requireRetirementRefusal(
	t *testing.T,
	err error,
	policy string,
	kind reject.AccountRetirementRefusalKind,
) {
	t.Helper()
	var retirementErr *reject.AccountRetirementError
	if !errors.As(err, &retirementErr) {
		t.Fatalf("RetireAccount() error = %v, want *AccountRetirementError", err)
	}
	if retirementErr.Kind != reject.AccountRetirementErrorKindRefused {
		t.Fatalf("retirement kind = %v, want Refused", retirementErr.Kind)
	}
	if len(retirementErr.Refusals) != 1 ||
		retirementErr.Refusals[0].Policy != policy ||
		retirementErr.Refusals[0].Kind != kind {
		t.Fatalf("refusals = %+v, want (%s, %v)", retirementErr.Refusals, policy, kind)
	}
}

func TestRetireAccountSuccessAndRepeatedCall(t *testing.T) {
	engine, err := NewEngineBuilder().FullSync().
		Builtin(policies.BuildOrderValidation()).Build()
	if err != nil {
		t.Fatal(err)
	}
	defer engine.Stop()
	accountID := param.NewAccountIDFromUint64(881)
	group, err := param.NewAccountGroupIDFromUint32(45)
	if err != nil {
		t.Fatal(err)
	}
	if err := engine.Accounts().RegisterGroup([]param.AccountID{accountID}, group); err != nil {
		t.Fatal(err)
	}
	for i := 0; i < 2; i++ {
		if err := engine.RetireAccount(accountID); err != nil {
			t.Fatalf("RetireAccount() call %d: %v", i+1, err)
		}
	}
	if engine.Accounts().GroupOf(accountID).IsSet() {
		t.Fatal("retired account retained group membership")
	}
}

func TestRetireAccountBuiltInConfigurationRefusal(t *testing.T) {
	accountID := param.NewAccountIDFromUint64(882)
	engine, err := NewEngineBuilder().FullSync().
		Builtin(policies.BuildRateLimit().AccountBarriers(
			policies.RateLimitAccountBarrier{
				AccountID: accountID,
				Limit:     policies.RateLimit{MaxOrders: 1, Window: time.Minute},
			},
		)).Build()
	if err != nil {
		t.Fatal(err)
	}
	defer engine.Stop()
	requireRetirementRefusal(
		t, engine.RetireAccount(accountID), "RateLimitPolicy",
		reject.AccountRetirementRefusalKindConfigurationReferencesAccount,
	)
}

type retirementProbePolicy struct {
	*postTradeResultPolicy
	name        string
	groupID     model.PolicyGroupID
	decision    pretrade.AccountRetirementDecision
	present     bool
	calls       int
	panic       bool
	commitPanic bool
}

func (p *retirementProbePolicy) Name() string { return p.name }

func (p *retirementProbePolicy) PolicyGroupID() model.PolicyGroupID { return p.groupID }

func (p *retirementProbePolicy) RetireAccount(
	_ param.AccountID,
	mutations tx.Mutations,
) pretrade.AccountRetirementDecision {
	p.calls++
	if p.panic {
		panic("retirement hook panic")
	}
	if p.decision == pretrade.AccountRetirementAccept && p.present {
		if err := mutations.Push(func() {
			if p.commitPanic {
				panic("retirement commit panic")
			}
			p.present = false
		}, func() {}); err != nil {
			return pretrade.AccountRetirementEvaluationFailed
		}
	}
	return p.decision
}

type retirementDryRunPolicy struct{ *retirementProbePolicy }

func (*retirementDryRunPolicy) CheckPreTradeStartDryRun(
	pretrade.Context, model.Order,
) []reject.Reject {
	return nil
}

func (*retirementDryRunPolicy) PerformPreTradeCheckDryRun(
	pretrade.Context, model.Order, tx.Mutations, pretrade.Result,
) []reject.Reject {
	return nil
}

func TestRetireAccountCustomPolicyWithDryRun(t *testing.T) {
	policy := &retirementDryRunPolicy{&retirementProbePolicy{
		postTradeResultPolicy: &postTradeResultPolicy{},
		name:                  "custom-dry-run",
		groupID:               17,
		decision:              pretrade.AccountRetirementNonZeroState,
	}}
	engine, err := NewEngineBuilder().FullSync().PreTrade(policy).Build()
	if err != nil {
		t.Fatal(err)
	}
	defer engine.Stop()
	requireRetirementRefusal(
		t, engine.RetireAccount(param.NewAccountIDFromUint64(886)),
		"custom-dry-run", reject.AccountRetirementRefusalKindNonZeroState,
	)
	if policy.calls != 1 {
		t.Fatalf("retirement calls = %d, want 1", policy.calls)
	}
}

func TestRetireAccountCustomPolicyWithoutHook(t *testing.T) {
	engine, err := NewEngineBuilder().FullSync().
		PreTrade(&postTradeResultPolicy{}).Build()
	if err != nil {
		t.Fatal(err)
	}
	defer engine.Stop()
	if err := engine.RetireAccount(param.NewAccountIDFromUint64(887)); err != nil {
		t.Fatal(err)
	}
}

func TestRetireAccountCommitPanicIsFinalizerFailure(t *testing.T) {
	policy := &retirementProbePolicy{
		postTradeResultPolicy: &postTradeResultPolicy{},
		name:                  "custom-commit-panic",
		groupID:               17,
		decision:              pretrade.AccountRetirementAccept,
		present:               true,
		commitPanic:           true,
	}
	engine, err := NewEngineBuilder().FullSync().PreTrade(policy).Build()
	if err != nil {
		t.Fatal(err)
	}
	defer engine.Stop()
	var retirementErr *reject.AccountRetirementError
	err = engine.RetireAccount(param.NewAccountIDFromUint64(888))
	if !errors.As(err, &retirementErr) {
		t.Fatalf("RetireAccount() error = %v, want *AccountRetirementError", err)
	}
	if retirementErr.Kind != reject.AccountRetirementErrorKindFinalizerFailed ||
		len(retirementErr.Refusals) != 0 {
		t.Fatalf("retirement error = %+v, want FinalizerFailed without refusals", retirementErr)
	}
}

func TestRetireAccountCustomRefusalAndCommittedMutation(t *testing.T) {
	accepting := &retirementProbePolicy{
		postTradeResultPolicy: &postTradeResultPolicy{},
		name:                  "custom-accept",
		groupID:               17,
		decision:              pretrade.AccountRetirementAccept,
		present:               true,
	}
	refusing := &retirementProbePolicy{
		postTradeResultPolicy: &postTradeResultPolicy{},
		name:                  "custom-refuse",
		groupID:               18,
		decision:              pretrade.AccountRetirementNonZeroState,
	}
	engine, err := NewEngineBuilder().FullSync().
		PreTrade(accepting).PreTrade(refusing).Build()
	if err != nil {
		t.Fatal(err)
	}
	defer engine.Stop()
	accountID := param.NewAccountIDFromUint64(883)
	requireRetirementRefusal(
		t, engine.RetireAccount(accountID), "custom-refuse",
		reject.AccountRetirementRefusalKindNonZeroState,
	)
	if accepting.calls != 1 || refusing.calls != 1 || !accepting.present {
		t.Fatalf("refusal changed state or skipped callbacks: accept=%+v refuse=%+v", accepting, refusing)
	}
	refusing.decision = pretrade.AccountRetirementAccept
	if err := engine.RetireAccount(accountID); err != nil {
		t.Fatal(err)
	}
	if accepting.present || accepting.calls != 2 || refusing.calls != 2 {
		t.Fatalf("commit did not remove state: accept=%+v refuse=%+v", accepting, refusing)
	}
}

func TestRetireAccountCustomPanicBecomesEvaluationFailed(t *testing.T) {
	policy := &retirementProbePolicy{
		postTradeResultPolicy: &postTradeResultPolicy{},
		name:                  "custom-panic",
		groupID:               17,
		panic:                 true,
	}
	engine, err := NewEngineBuilder().FullSync().PreTrade(policy).Build()
	if err != nil {
		t.Fatal(err)
	}
	defer engine.Stop()
	requireRetirementRefusal(
		t, engine.RetireAccount(param.NewAccountIDFromUint64(884)),
		"custom-panic", reject.AccountRetirementRefusalKindEvaluationFailed,
	)
}

type retirementClientPolicy struct {
	*clientEngineTestAdjustmentPreTradePolicy
	calls int
}

func (p *retirementClientPolicy) RetireAccount(
	param.AccountID,
	tx.Mutations,
) pretrade.AccountRetirementDecision {
	p.calls++
	return pretrade.AccountRetirementOperationInProgress
}

func TestRetireAccountClientTypedAdapters(t *testing.T) {
	for _, test := range []struct {
		name    string
		options []ClientEngineOption
	}{
		{name: "safe"},
		{name: "unsafe", options: []ClientEngineOption{UnsafeFastClientPayloadCallbacks()}},
	} {
		t.Run(test.name, func(t *testing.T) {
			policy := &retirementClientPolicy{
				clientEngineTestAdjustmentPreTradePolicy: &clientEngineTestAdjustmentPreTradePolicy{},
			}
			engine, err := NewClientEngineBuilder[
				clientEngineTestOrder,
				clientEngineTestReport,
				clientEngineTestAdjustment,
			](test.options...).FullSync().PreTrade(policy).Build()
			if err != nil {
				t.Fatal(err)
			}
			defer engine.Stop()
			requireRetirementRefusal(
				t, engine.RetireAccount(param.NewAccountIDFromUint64(885)),
				"client-engine-test-adjustment",
				reject.AccountRetirementRefusalKindOperationInProgress,
			)
			if policy.calls != 1 {
				t.Fatalf("client policy retirement calls = %d, want 1", policy.calls)
			}
		})
	}
}

func TestRetireAccountClientTypedAdaptersWithoutHook(t *testing.T) {
	for _, test := range []struct {
		name    string
		options []ClientEngineOption
	}{
		{name: "safe"},
		{name: "unsafe", options: []ClientEngineOption{UnsafeFastClientPayloadCallbacks()}},
	} {
		t.Run(test.name, func(t *testing.T) {
			engine, err := NewClientEngineBuilder[
				clientEngineTestOrder,
				clientEngineTestReport,
				clientEngineTestAdjustment,
			](test.options...).FullSync().
				PreTrade(&clientEngineTestAdjustmentPreTradePolicy{}).Build()
			if err != nil {
				t.Fatal(err)
			}
			defer engine.Stop()
			if err := engine.RetireAccount(param.NewAccountIDFromUint64(889)); err != nil {
				t.Fatal(err)
			}
		})
	}
}
