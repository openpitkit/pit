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
	"strings"
	"sync"
	"testing"

	"go.openpit.dev/openpit/accountadjustment"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/optional"
	"go.openpit.dev/openpit/pretrade"
	"go.openpit.dev/openpit/reject"
	"go.openpit.dev/openpit/tx"
)

// Every exported cgo trampoline must contain a panicking user callback: an
// unwind across the C frame kills the process. Each hook has its own answer
// channel, so each is driven separately through the public engine surface.

// panickingHook names the policy callback that panics.
type panickingHook int

const (
	panicOnCheckPreTradeStart panickingHook = iota
	panicOnPerformPreTradeCheck
	panicOnApplyExecutionReport
	panicOnApplyAccountAdjustment
	panicOnCheckPreTradeStartDryRun
	panicOnPerformPreTradeCheckDryRun
	panicOnClose
)

const callbackPanicValue = "go policy hook exploded"

type panickingHookPolicy struct {
	hook panickingHook
}

var _ pretrade.DryRunPolicy = (*panickingHookPolicy)(nil)

func (p *panickingHookPolicy) Close() { p.panicIf(panicOnClose) }

func (*panickingHookPolicy) Name() string { return "panicking-hook" }

func (*panickingHookPolicy) PolicyGroupID() model.PolicyGroupID {
	return model.DefaultPolicyGroupID
}

func (p *panickingHookPolicy) panicIf(hook panickingHook) {
	if p.hook == hook {
		panic(callbackPanicValue)
	}
}

func (p *panickingHookPolicy) CheckPreTradeStart(
	pretrade.Context,
	model.Order,
) []reject.Reject {
	p.panicIf(panicOnCheckPreTradeStart)
	return nil
}

func (p *panickingHookPolicy) PerformPreTradeCheck(
	pretrade.Context,
	model.Order,
	tx.Mutations,
	pretrade.Result,
) []reject.Reject {
	p.panicIf(panicOnPerformPreTradeCheck)
	return nil
}

func (p *panickingHookPolicy) ApplyExecutionReport(
	pretrade.PostTradeContext,
	model.ExecutionReport,
	pretrade.PostTradeAdjustments,
	pretrade.PostTradePnls,
) []reject.AccountBlock {
	p.panicIf(panicOnApplyExecutionReport)
	return nil
}

func (p *panickingHookPolicy) ApplyAccountAdjustment(
	accountadjustment.Context,
	param.AccountID,
	model.AccountAdjustment,
	tx.Mutations,
	pretrade.AccountOutcomes,
) (pretrade.PolicyAccountAdjustmentResult, []reject.Reject) {
	p.panicIf(panicOnApplyAccountAdjustment)
	return pretrade.PolicyAccountAdjustmentResult{}, nil
}

func (p *panickingHookPolicy) CheckPreTradeStartDryRun(
	pretrade.Context,
	model.Order,
) []reject.Reject {
	p.panicIf(panicOnCheckPreTradeStartDryRun)
	return nil
}

func (p *panickingHookPolicy) PerformPreTradeCheckDryRun(
	pretrade.Context,
	model.Order,
	tx.Mutations,
	pretrade.Result,
) []reject.Reject {
	p.panicIf(panicOnPerformPreTradeCheckDryRun)
	return nil
}

func newPanickingHookEngine(t *testing.T, hook panickingHook) *Engine {
	t.Helper()
	engine, err := NewEngineBuilder().
		FullSync().
		PreTrade(&panickingHookPolicy{hook: hook}).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	t.Cleanup(engine.Stop)
	return engine
}

func assertCallbackPanicReject(t *testing.T, rejects []reject.Reject) {
	t.Helper()
	if len(rejects) != 1 {
		t.Fatalf("rejects len = %d, want 1", len(rejects))
	}
	got := rejects[0]
	if got.Code != reject.CodeSystemUnavailable {
		t.Fatalf("reject code = %v, want %v", got.Code, reject.CodeSystemUnavailable)
	}
	if !strings.Contains(got.Details, callbackPanicValue) {
		t.Fatalf("reject details = %q, want the panic value %q", got.Details, callbackPanicValue)
	}
}

func TestCustomPolicyPanicNativeE2E_CheckPreTradeStart(t *testing.T) {
	engine := newPanickingHookEngine(t, panicOnCheckPreTradeStart)

	request, rejects, err := engine.StartPreTrade(newValidOrderForNativeE2E(t))
	if err != nil {
		t.Fatalf("StartPreTrade() error = %v", err)
	}
	if request != nil {
		request.Close()
		t.Fatal("StartPreTrade() request != nil, want a callback-failure reject")
	}
	assertCallbackPanicReject(t, rejects)
}

func TestCustomPolicyPanicNativeE2E_PerformPreTradeCheck(t *testing.T) {
	engine := newPanickingHookEngine(t, panicOnPerformPreTradeCheck)

	reservation, rejects, err := engine.ExecutePreTrade(newValidOrderForNativeE2E(t))
	if err != nil {
		t.Fatalf("ExecutePreTrade() error = %v", err)
	}
	if reservation != nil {
		reservation.Close()
		t.Fatal("ExecutePreTrade() reservation != nil, want a callback-failure reject")
	}
	assertCallbackPanicReject(t, rejects)
}

func TestCustomPolicyPanicNativeE2E_DropCopyPerformPreTradeCheck(t *testing.T) {
	engine := newPanickingHookEngine(t, panicOnPerformPreTradeCheck)

	assertCallbackPanicReject(
		t,
		requireRejectedDropCopy(t, engine, newValidOrderForNativeE2E(t)),
	)
}

func TestCustomPolicyPanicNativeE2E_ApplyExecutionReport(t *testing.T) {
	engine := newPanickingHookEngine(t, panicOnApplyExecutionReport)

	result, err := engine.ApplyExecutionReport(model.NewExecutionReport())
	if err != nil {
		t.Fatalf("ApplyExecutionReport() error = %v", err)
	}
	// This hook answers with account blocks, not rejects, so the recovered
	// panic has to travel on that channel instead.
	if len(result.AccountBlocks) != 1 {
		t.Fatalf("AccountBlocks len = %d, want 1", len(result.AccountBlocks))
	}
	block := result.AccountBlocks[0]
	if block.Code != reject.CodeSystemUnavailable {
		t.Fatalf("block code = %v, want %v", block.Code, reject.CodeSystemUnavailable)
	}
	if !strings.Contains(block.Details, callbackPanicValue) {
		t.Fatalf("block details = %q, want the panic value %q", block.Details, callbackPanicValue)
	}
}

func TestCustomPolicyPanicNativeE2E_ApplyAccountAdjustment(t *testing.T) {
	engine := newPanickingHookEngine(t, panicOnApplyAccountAdjustment)

	adjustment, err := model.NewAccountAdjustmentFromValues(
		model.AccountAdjustmentValues{
			AccountPnlOperation: optional.Some(
				model.NewAccountAdjustmentAccountPnlOperation(
					model.NewPnlState(mustAdjustmentNativePnl(t, "1")),
				),
			),
		},
	)
	if err != nil {
		t.Fatalf("NewAccountAdjustmentFromValues() error = %v", err)
	}

	result, err := engine.ApplyAccountAdjustment(
		param.NewAccountIDFromUint64(55),
		[]model.AccountAdjustment{adjustment},
	)
	if err != nil {
		t.Fatalf("ApplyAccountAdjustment() error = %v", err)
	}
	batchError, ok := result.BatchError.Get()
	if !ok {
		t.Fatal("BatchError not set, want a callback-failure reject")
	}
	assertCallbackPanicReject(t, batchError.Rejects)
}

// TestCustomPolicyPanicNativeE2E_CloseReachesTeardownHandler pins the only
// channel a teardown panic has: Close returns nothing to the engine, so without
// the handler the failure and the cgo handle it strands leave no trace.
func TestCustomPolicyPanicNativeE2E_CloseReachesTeardownHandler(t *testing.T) {
	var mu sync.Mutex
	var reported []error
	SetTeardownFailureHandler(func(err error) {
		mu.Lock()
		defer mu.Unlock()
		reported = append(reported, err)
	})
	t.Cleanup(func() { SetTeardownFailureHandler(nil) })

	engine, err := NewEngineBuilder().
		FullSync().
		PreTrade(&panickingHookPolicy{hook: panicOnClose}).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	engine.Stop()

	mu.Lock()
	defer mu.Unlock()
	if len(reported) != 1 {
		t.Fatalf("reported teardown failures = %d, want 1", len(reported))
	}
	if !strings.Contains(reported[0].Error(), callbackPanicValue) {
		t.Fatalf(
			"teardown failure = %q, want the panic value %q",
			reported[0],
			callbackPanicValue,
		)
	}
}

func TestCustomPolicyPanicNativeE2E_CheckPreTradeStartDryRun(t *testing.T) {
	engine := newPanickingHookEngine(t, panicOnCheckPreTradeStartDryRun)

	report, err := engine.StartPreTradeDryRun(newValidOrderForNativeE2E(t))
	if err != nil {
		t.Fatalf("StartPreTradeDryRun() error = %v", err)
	}
	defer report.Close()
	pass, err := report.IsPass()
	if err != nil {
		t.Fatalf("IsPass() error = %v", err)
	}
	if pass {
		t.Fatal("StartPreTradeDryRun() passed, want a callback-failure reject")
	}
	rejects, err := report.Rejects()
	if err != nil {
		t.Fatalf("Rejects() error = %v", err)
	}
	assertCallbackPanicReject(t, rejects)
}

func TestCustomPolicyPanicNativeE2E_PerformPreTradeCheckDryRun(t *testing.T) {
	engine := newPanickingHookEngine(t, panicOnPerformPreTradeCheckDryRun)

	report, err := engine.ExecutePreTradeDryRun(newValidOrderForNativeE2E(t))
	if err != nil {
		t.Fatalf("ExecutePreTradeDryRun() error = %v", err)
	}
	defer report.Close()
	pass, err := report.IsPass()
	if err != nil {
		t.Fatalf("IsPass() error = %v", err)
	}
	if pass {
		t.Fatal("ExecutePreTradeDryRun() passed, want a callback-failure reject")
	}
	rejects, err := report.Rejects()
	if err != nil {
		t.Fatalf("Rejects() error = %v", err)
	}
	assertCallbackPanicReject(t, rejects)
}
