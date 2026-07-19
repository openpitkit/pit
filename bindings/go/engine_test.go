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
	"strings"
	"testing"

	"go.openpit.dev/openpit/accountadjustment"
	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pretrade"
	"go.openpit.dev/openpit/reject"
	"go.openpit.dev/openpit/tx"
)

func TestEngineBuilderCloseIsIdempotent(*testing.T) {
	builder := NewEngineBuilder().
		FullSync().
		PreTrade(&engineTestStartPolicy{name: "p"})

	builder.Close()
	builder.Close()
}

func TestEngineBuilderBuildFailsAfterClose(t *testing.T) {
	builder := NewEngineBuilder().
		FullSync().
		PreTrade(&engineTestStartPolicy{name: "p"})

	builder.Close()
	engine, err := builder.Build()
	if engine != nil {
		engine.Stop()
		t.Fatal("Build() engine != nil, want nil")
	}
	if err == nil {
		t.Fatal("Build() error = nil, want non-nil")
	}
}

func TestEngineBuilderScheduleCloseAfterPolicyAddFailure(t *testing.T) {
	second := &engineTestStartPolicy{name: "second"}

	// A forced-failure applier triggers an error on the first Builtin call.
	// Subsequent policy adds must see the error and schedule policies for
	// cleanup.
	builder := NewEngineBuilder().FullSync().Builtin(&engineTestFailingBuilder{}).PreTrade(second)

	if second.closeCalls != 0 {
		t.Fatalf("second closeCalls before Build() = %d, want 0", second.closeCalls)
	}

	_, err := builder.Build()
	if err == nil {
		t.Fatal("Build() error = nil, want non-nil")
	}
	if !strings.Contains(err.Error(), "forced") {
		t.Fatalf(
			"Build() error = %q, want to contain %q",
			err.Error(), "forced",
		)
	}
	if second.closeCalls != 1 {
		t.Fatalf("second closeCalls after Build() = %d, want 1", second.closeCalls)
	}
}

func TestEngineBuilderPolicyAddErrorFormatsMessage(t *testing.T) {
	err := newEngineBuilderPolicyAddError(errors.New("forced"), "policy-a")
	if got, want := err.Error(), `failed to add policy "policy-a": forced`; got != want {
		t.Fatalf("Error() = %q, want %q", got, want)
	}
}

func TestEngineStartPreTradeReturnsErrorAfterStop(t *testing.T) {
	engine := newEngineForTests(t)
	engine.Stop()

	request, rejects, err := engine.StartPreTrade(model.NewOrder())
	if request != nil {
		request.Close()
		t.Fatal("StartPreTrade() request != nil, want nil")
	}
	if rejects != nil {
		t.Fatalf("StartPreTrade() rejects = %v, want nil", rejects)
	}
	if err == nil {
		t.Fatal("StartPreTrade() error = nil, want non-nil")
	}
}

func TestEngineExecutePreTradeReturnsErrorAfterStop(t *testing.T) {
	engine := newEngineForTests(t)
	engine.Stop()

	reservation, rejects, err := engine.ExecutePreTrade(model.NewOrder())
	if reservation != nil {
		reservation.Close()
		t.Fatal("ExecutePreTrade() reservation != nil, want nil")
	}
	if rejects != nil {
		t.Fatalf("ExecutePreTrade() rejects = %v, want nil", rejects)
	}
	if err == nil {
		t.Fatal("ExecutePreTrade() error = nil, want non-nil")
	}
}

func TestEngineApplyAccountAdjustmentEmptyBatchIsNoop(t *testing.T) {
	engine := newEngineForTests(t)
	defer engine.Stop()

	result, err := engine.ApplyAccountAdjustment(param.NewAccountIDFromUint64(1), nil)
	if err != nil {
		t.Fatalf("ApplyAccountAdjustment() error = %v, want nil", err)
	}
	if result.BatchError.IsSet() {
		t.Fatalf("ApplyAccountAdjustment() rejects = %v, want none", result.BatchError)
	}
}

// TestEngineNoAccountBlocksReturnsNilSlice pins the zero-value contract
// shared by ApplyExecutionReport and ApplyAccountAdjustment: when no account
// is blocked, AccountBlocks is a nil slice, not a non-nil empty one.
func TestEngineNoAccountBlocksReturnsNilSlice(t *testing.T) {
	engine := newEngineForTests(t)
	defer engine.Stop()

	adjustmentResult, err := engine.ApplyAccountAdjustment(
		param.NewAccountIDFromUint64(1), nil,
	)
	if err != nil {
		t.Fatalf("ApplyAccountAdjustment() error = %v, want nil", err)
	}
	if adjustmentResult.AccountBlocks != nil {
		t.Fatalf(
			"ApplyAccountAdjustment() AccountBlocks = %#v, want nil",
			adjustmentResult.AccountBlocks,
		)
	}

	reportResult, err := engine.ApplyExecutionReport(model.NewExecutionReport())
	if err != nil {
		t.Fatalf("ApplyExecutionReport() error = %v, want nil", err)
	}
	if reportResult.AccountBlocks != nil {
		t.Fatalf(
			"ApplyExecutionReport() AccountBlocks = %#v, want nil",
			reportResult.AccountBlocks,
		)
	}
}

func TestEngineApplyAccountAdjustmentReturnsBatchReject(t *testing.T) {
	engine, err := NewEngineBuilder().
		FullSync().
		PreTrade(&engineTestRejectingAdjustmentPolicy{name: "adjustment-reject"}).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	result, err := engine.ApplyAccountAdjustment(
		param.NewAccountIDFromUint64(1),
		[]model.AccountAdjustment{model.NewAccountAdjustment()},
	)
	if err != nil {
		t.Fatalf("ApplyAccountAdjustment() error = %v", err)
	}
	if !result.BatchError.IsSet() {
		t.Fatal("ApplyAccountAdjustment() rejects.IsSet() = false, want true")
	}
	batchReject, ok := result.BatchError.Get()
	if !ok {
		t.Fatal("ApplyAccountAdjustment() rejects.Get() ok = false, want true")
	}
	if batchReject.FailedAdjustmentIndex != 0 {
		t.Fatalf("FailedAdjustmentIndex = %d, want 0", batchReject.FailedAdjustmentIndex)
	}
	if len(batchReject.Rejects) != 1 {
		t.Fatalf("batch reject len = %d, want 1", len(batchReject.Rejects))
	}
	if batchReject.Rejects[0].Policy != "adjustment-reject" {
		t.Fatalf("reject policy = %q, want %q", batchReject.Rejects[0].Policy, "adjustment-reject")
	}
}

func newEngineForTests(t *testing.T) *Engine {
	t.Helper()

	engine, err := NewEngineBuilder().
		FullSync().
		PreTrade(&engineTestNoopStartPolicy{}).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	return engine
}

type engineTestStartPolicy struct {
	name       string
	closeCalls int
}

func (p *engineTestStartPolicy) Close() {
	p.closeCalls++
}

func (p engineTestStartPolicy) Name() string {
	return p.name
}

func (engineTestStartPolicy) PolicyGroupID() model.PolicyGroupID { return model.DefaultPolicyGroupID }

func (engineTestStartPolicy) CheckPreTradeStart(pretrade.Context, model.Order) []reject.Reject {
	return nil
}

func (engineTestStartPolicy) PerformPreTradeCheck(
	pretrade.Context, model.Order, tx.Mutations, pretrade.Result,
) []reject.Reject {
	return nil
}

func (engineTestStartPolicy) ApplyExecutionReport(
	pretrade.PostTradeContext,
	model.ExecutionReport,
	pretrade.PostTradeAdjustments,
	pretrade.PostTradePnls,
) []reject.AccountBlock {
	return nil
}

func (engineTestStartPolicy) ApplyAccountAdjustment(
	accountadjustment.Context, param.AccountID, model.AccountAdjustment, tx.Mutations,
	pretrade.AccountOutcomes,
) (pretrade.PolicyAccountAdjustmentResult, []reject.Reject) {
	return pretrade.PolicyAccountAdjustmentResult{}, nil
}

type engineTestNoopStartPolicy struct{}

func (engineTestNoopStartPolicy) Close() {}

func (engineTestNoopStartPolicy) Name() string { return "noop" }

func (engineTestNoopStartPolicy) PolicyGroupID() model.PolicyGroupID {
	return model.DefaultPolicyGroupID
}

func (engineTestNoopStartPolicy) CheckPreTradeStart(pretrade.Context, model.Order) []reject.Reject {
	return nil
}

func (engineTestNoopStartPolicy) PerformPreTradeCheck(
	pretrade.Context, model.Order, tx.Mutations, pretrade.Result,
) []reject.Reject {
	return nil
}

func (engineTestNoopStartPolicy) ApplyExecutionReport(
	pretrade.PostTradeContext,
	model.ExecutionReport,
	pretrade.PostTradeAdjustments,
	pretrade.PostTradePnls,
) []reject.AccountBlock {
	return nil
}

func (engineTestNoopStartPolicy) ApplyAccountAdjustment(
	accountadjustment.Context, param.AccountID, model.AccountAdjustment, tx.Mutations,
	pretrade.AccountOutcomes,
) (pretrade.PolicyAccountAdjustmentResult, []reject.Reject) {
	return pretrade.PolicyAccountAdjustmentResult{}, nil
}

type engineTestRejectingAdjustmentPolicy struct {
	name string
}

func (engineTestRejectingAdjustmentPolicy) Close() {}

func (p engineTestRejectingAdjustmentPolicy) Name() string {
	return p.name
}

func (engineTestRejectingAdjustmentPolicy) PolicyGroupID() model.PolicyGroupID {
	return model.DefaultPolicyGroupID
}

func (engineTestRejectingAdjustmentPolicy) CheckPreTradeStart(
	pretrade.Context, model.Order,
) []reject.Reject {
	return nil
}

func (engineTestRejectingAdjustmentPolicy) PerformPreTradeCheck(
	pretrade.Context, model.Order, tx.Mutations, pretrade.Result,
) []reject.Reject {
	return nil
}

func (engineTestRejectingAdjustmentPolicy) ApplyExecutionReport(
	pretrade.PostTradeContext,
	model.ExecutionReport,
	pretrade.PostTradeAdjustments,
	pretrade.PostTradePnls,
) []reject.AccountBlock {
	return nil
}

func (p *engineTestRejectingAdjustmentPolicy) ApplyAccountAdjustment(
	accountadjustment.Context,
	param.AccountID,
	model.AccountAdjustment,
	tx.Mutations,
	pretrade.AccountOutcomes,
) (pretrade.PolicyAccountAdjustmentResult, []reject.Reject) {
	return pretrade.PolicyAccountAdjustmentResult{}, reject.NewSingleItemList(
		reject.CodeOther,
		p.name,
		"adjustment rejected",
		"rejected in test policy",
		reject.ScopeAccount,
	)
}

type engineTestFailingBuilder struct{}

func (*engineTestFailingBuilder) Build(_ native.EngineBuilder) error {
	return errors.New("forced")
}
