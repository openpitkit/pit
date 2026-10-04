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
	"fmt"
	"testing"

	"go.openpit.dev/openpit/accountadjustment"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pretrade"
	"go.openpit.dev/openpit/reject"
	"go.openpit.dev/openpit/tx"
)

func TestHookArgumentNativeE2E_CheckPreTradeStartInHook(t *testing.T) {
	policy := newHookArgumentUsagePolicy(t)
	engine := newEngineWithPreTradePolicyForNativeE2E(t, policy)
	defer engine.Stop()

	executeHookArgumentPreTrade(t, engine)
	policy.startCalls.assert(t, "CheckPreTradeStart",
		"Context.IsDropCopy", "Context.AccountControl", "Context.AccountGroup",
	)
	if policy.startIsDropCopy {
		t.Fatal("Context.IsDropCopy() = true inside CheckPreTradeStart for an ordinary order")
	}
}

func TestHookArgumentNativeE2E_PerformPreTradeCheckInHook(
	t *testing.T,
) {
	policy := newHookArgumentUsagePolicy(t)
	engine := newEngineWithPreTradePolicyForNativeE2E(t, policy)
	defer engine.Stop()

	executeHookArgumentPreTrade(t, engine)
	policy.preTradeCalls.assert(t, "PerformPreTradeCheck",
		"Context.IsDropCopy", "Context.AccountControl", "Context.AccountGroup",
		"Mutations.Push", "Result.PushLockPrice", "Result.PushAccountAdjustment",
	)
}

func TestHookArgumentNativeE2E_ApplyExecutionReportInHook(
	t *testing.T,
) {
	policy := newHookArgumentUsagePolicy(t)
	engine := newEngineWithPreTradePolicyForNativeE2E(t, policy)
	defer engine.Stop()

	_, err := engine.ApplyExecutionReport(model.NewExecutionReport())
	if err != nil {
		t.Fatalf("ApplyExecutionReport() error = %v", err)
	}
	policy.postTradeCalls.assert(t, "ApplyExecutionReport",
		"PostTradeContext.AccountGroup", "PostTradeAdjustments.Push",
		"PostTradePnls.Push",
	)
}

func TestHookArgumentNativeE2E_ApplyAccountAdjustmentInHook(
	t *testing.T,
) {
	policy := newHookArgumentUsagePolicy(t)
	engine := newEngineWithPreTradePolicyForNativeE2E(t, policy)
	defer engine.Stop()

	result, err := engine.ApplyAccountAdjustment(
		param.NewAccountIDFromUint64(7001),
		[]model.AccountAdjustment{model.NewAccountAdjustment()},
	)
	if err != nil {
		t.Fatalf("ApplyAccountAdjustment() error = %v", err)
	}
	if result.BatchError.IsSet() {
		t.Fatalf("ApplyAccountAdjustment() batch error = %v", result.BatchError)
	}
	policy.adjustmentCalls.assert(t, "ApplyAccountAdjustment",
		"Context.AccountControl", "Context.AccountGroup",
		"Mutations.Push", "AccountOutcomes.Push",
	)
}

func TestHookArgumentNativeE2E_CheckPreTradeStartDryRunInHook(
	t *testing.T,
) {
	policy := newHookArgumentUsagePolicy(t)
	engine := newEngineWithPreTradePolicyForNativeE2E(t, policy)
	defer engine.Stop()

	executeHookArgumentDryRun(t, engine)
	policy.dryRunStartCalls.assert(t, "CheckPreTradeStartDryRun",
		"Context.IsDropCopy", "Context.AccountControl", "Context.AccountGroup",
	)
}

func TestHookArgumentNativeE2E_PerformPreTradeCheckDryRunInHook(
	t *testing.T,
) {
	policy := newHookArgumentUsagePolicy(t)
	engine := newEngineWithPreTradePolicyForNativeE2E(t, policy)
	defer engine.Stop()

	executeHookArgumentDryRun(t, engine)
	policy.dryRunPreTradeCalls.assert(t, "PerformPreTradeCheckDryRun",
		"Context.IsDropCopy", "Context.AccountControl", "Context.AccountGroup",
		"Mutations.Push", "Result.PushLockPrice", "Result.PushAccountAdjustment",
	)
}

func TestHookArgumentNativeE2E_DropCopyStartInHook(t *testing.T) {
	policy := newHookArgumentUsagePolicy(t)
	engine := newEngineWithPreTradePolicyForNativeE2E(t, policy)
	defer engine.Stop()

	requireAppliedDropCopy(t, engine, newValidOrderForNativeE2E(t)).CommitAndClose()
	policy.startCalls.assert(t, "CheckPreTradeStart",
		"Context.IsDropCopy", "Context.AccountControl", "Context.AccountGroup",
		"Context.RecordDropCopyStartMutation",
	)
}

type hookArgumentUsagePolicy struct {
	startIsDropCopy     bool
	startCalls          hookArgumentLiveCalls
	preTradeCalls       hookArgumentLiveCalls
	postTradeCalls      hookArgumentLiveCalls
	adjustmentCalls     hookArgumentLiveCalls
	dryRunStartCalls    hookArgumentLiveCalls
	dryRunPreTradeCalls hookArgumentLiveCalls
	lockPrice           param.Price
	entry               accountadjustment.AccountOutcomeEntry
	pnl                 accountadjustment.AccountPnlOutcome
}

func newHookArgumentUsagePolicy(t *testing.T) *hookArgumentUsagePolicy {
	t.Helper()
	return &hookArgumentUsagePolicy{
		lockPrice: mustOrderNativePrice(t, "100"),
		entry:     hookArgumentAccountOutcomeEntry(t),
		pnl:       hookArgumentPnlOutcome(t),
	}
}

func (*hookArgumentUsagePolicy) Close() {}

func (*hookArgumentUsagePolicy) Name() string {
	return "hook-argument-usage"
}

func (*hookArgumentUsagePolicy) PolicyGroupID() model.PolicyGroupID {
	return 0
}

func (p *hookArgumentUsagePolicy) CheckPreTradeStart(
	ctx pretrade.Context,
	_ model.Order,
) []reject.Reject {
	p.startCalls.ran = true
	isDropCopy := p.startCalls.usePreTradeContext(ctx)
	p.startIsDropCopy = isDropCopy
	if isDropCopy {
		p.startCalls.record("Context.RecordDropCopyStartMutation", func() error {
			return ctx.RecordDropCopyStartMutation(func() {}, func() {})
		})
	}
	return nil
}

func (p *hookArgumentUsagePolicy) PerformPreTradeCheck(
	ctx pretrade.Context,
	_ model.Order,
	mutations tx.Mutations,
	result pretrade.Result,
) []reject.Reject {
	p.preTradeCalls.ran = true
	p.preTradeCalls.usePreTradeContext(ctx)
	p.preTradeCalls.usePreTradeCollectors(mutations, result, p.lockPrice, p.entry)
	return nil
}

func (p *hookArgumentUsagePolicy) ApplyExecutionReport(
	ctx pretrade.PostTradeContext,
	_ model.ExecutionReport,
	adjustments pretrade.PostTradeAdjustments,
	pnls pretrade.PostTradePnls,
) []reject.AccountBlock {
	p.postTradeCalls.ran = true
	p.postTradeCalls.record("PostTradeContext.AccountGroup", func() error {
		ctx.AccountGroup()
		return nil
	})
	p.postTradeCalls.record("PostTradeAdjustments.Push", func() error {
		return adjustments.Push(1, p.entry)
	})
	p.postTradeCalls.record("PostTradePnls.Push", func() error {
		return pnls.Push(p.pnl)
	})
	return nil
}

func (p *hookArgumentUsagePolicy) ApplyAccountAdjustment(
	ctx accountadjustment.Context,
	_ param.AccountID,
	_ model.AccountAdjustment,
	mutations tx.Mutations,
	outcomes pretrade.AccountOutcomes,
) (pretrade.PolicyAccountAdjustmentResult, []reject.Reject) {
	p.adjustmentCalls.ran = true
	p.adjustmentCalls.record("Context.AccountControl", func() error {
		ctx.AccountControl()
		return nil
	})
	p.adjustmentCalls.record("Context.AccountGroup", func() error {
		ctx.AccountGroup()
		return nil
	})
	p.adjustmentCalls.record("Mutations.Push", func() error {
		return mutations.Push(func() {}, func() {})
	})
	p.adjustmentCalls.record("AccountOutcomes.Push", func() error {
		return outcomes.Push(p.entry)
	})
	return pretrade.PolicyAccountAdjustmentResult{}, nil
}

func (p *hookArgumentUsagePolicy) CheckPreTradeStartDryRun(
	ctx pretrade.Context,
	_ model.Order,
) []reject.Reject {
	p.dryRunStartCalls.ran = true
	p.dryRunStartCalls.usePreTradeContext(ctx)
	return nil
}

func (p *hookArgumentUsagePolicy) PerformPreTradeCheckDryRun(
	ctx pretrade.Context,
	_ model.Order,
	mutations tx.Mutations,
	result pretrade.Result,
) []reject.Reject {
	p.dryRunPreTradeCalls.ran = true
	p.dryRunPreTradeCalls.usePreTradeContext(ctx)
	p.dryRunPreTradeCalls.usePreTradeCollectors(mutations, result, p.lockPrice, p.entry)
	return nil
}

type hookArgumentLiveCalls struct {
	ran        bool
	callErrors map[string]error
}

func (c *hookArgumentLiveCalls) record(name string, call func() error) {
	if c.callErrors == nil {
		c.callErrors = make(map[string]error)
	}
	defer func() {
		if recovered := recover(); recovered != nil {
			c.callErrors[name] = fmt.Errorf("panic: %v", recovered)
		}
	}()
	c.callErrors[name] = call()
}

func (c *hookArgumentLiveCalls) usePreTradeContext(ctx pretrade.Context) bool {
	var isDropCopy bool
	c.record("Context.IsDropCopy", func() error {
		isDropCopy = ctx.IsDropCopy()
		return nil
	})
	c.record("Context.AccountControl", func() error {
		ctx.AccountControl()
		return nil
	})
	c.record("Context.AccountGroup", func() error {
		ctx.AccountGroup()
		return nil
	})
	return isDropCopy
}

func (c *hookArgumentLiveCalls) usePreTradeCollectors(
	mutations tx.Mutations,
	result pretrade.Result,
	price param.Price,
	entry accountadjustment.AccountOutcomeEntry,
) {
	c.record("Mutations.Push", func() error {
		return mutations.Push(func() {}, func() {})
	})
	c.record("Result.PushLockPrice", func() error {
		return result.PushLockPrice(price)
	})
	c.record("Result.PushAccountAdjustment", func() error {
		return result.PushAccountAdjustment(entry)
	})
}

func (c *hookArgumentLiveCalls) assert(
	t *testing.T,
	hook string,
	wantCalls ...string,
) {
	t.Helper()
	if !c.ran {
		t.Fatalf("%s hook did not run", hook)
	}
	for _, name := range wantCalls {
		err, called := c.callErrors[name]
		if !called {
			t.Fatalf("%s: live %s was not called", hook, name)
		}
		if err != nil {
			t.Fatalf("%s: live %s failed: %v", hook, name, err)
		}
	}
}

func executeHookArgumentPreTrade(t *testing.T, engine *Engine) {
	t.Helper()

	reservation, rejects, err := engine.ExecutePreTrade(
		newValidOrderForNativeE2E(t),
	)
	if err != nil {
		t.Fatalf("ExecutePreTrade() error = %v", err)
	}
	if rejects != nil {
		t.Fatalf("ExecutePreTrade() rejects = %v, want nil", rejects)
	}
	if reservation == nil {
		t.Fatal("ExecutePreTrade() reservation = nil, want non-nil")
	}
	reservation.RollbackAndClose()
}

func executeHookArgumentDryRun(t *testing.T, engine *Engine) {
	t.Helper()

	report, err := engine.ExecutePreTradeDryRun(newValidOrderForNativeE2E(t))
	if err != nil {
		t.Fatalf("ExecutePreTradeDryRun() error = %v", err)
	}
	report.Close()
}

func hookArgumentAccountOutcomeEntry(
	t *testing.T,
) accountadjustment.AccountOutcomeEntry {
	t.Helper()
	return accountadjustment.AccountOutcomeEntry{
		Asset: mustAdjustmentNativeAsset(t, "USD"),
	}
}

func hookArgumentPnlOutcome(t *testing.T) accountadjustment.AccountPnlOutcome {
	t.Helper()
	return accountadjustment.NewAccountPnlOutcome(
		1,
		param.NewAccountIDFromUint64(7002),
		accountadjustment.PnlOutcomeAmount{
			Delta:    mustAdjustmentNativePnl(t, "0"),
			Absolute: mustAdjustmentNativePnl(t, "0"),
		},
	)
}
