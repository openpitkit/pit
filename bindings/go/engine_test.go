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

package openpit

import (
	"errors"
	"strings"
	"testing"

	"go.openpit.dev/openpit/accountadjustment"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pretrade"
	"go.openpit.dev/openpit/reject"
	"go.openpit.dev/openpit/tx"
)

func TestEngineBuilderCloseIsIdempotent(t *testing.T) {
	builder, err := NewEngineBuilder()
	if err != nil {
		t.Fatalf("NewEngineBuilder() error = %v", err)
	}

	builder.Close()
	builder.Close()
}

func TestEngineBuilderBuildFailsAfterClose(t *testing.T) {
	builder, err := NewEngineBuilder()
	if err != nil {
		t.Fatalf("NewEngineBuilder() error = %v", err)
	}

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
	builder, err := NewEngineBuilder()
	if err != nil {
		t.Fatalf("NewEngineBuilder() error = %v", err)
	}

	builder.Close()

	first := &engineTestStartPolicy{name: "first"}
	second := &engineTestStartPolicy{name: "second"}
	builder.CheckPreTradeStartPolicy(first)
	builder.CheckPreTradeStartPolicy(second)

	if second.closeCalls != 0 {
		t.Fatalf("second closeCalls before Build() = %d, want 0", second.closeCalls)
	}

	_, err = builder.Build()
	if err == nil {
		t.Fatal("Build() error = nil, want non-nil")
	}
	if !strings.Contains(err.Error(), `failed to add policy "first"`) {
		t.Fatalf("Build() error = %q, want to contain %q", err.Error(), `failed to add policy "first"`)
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

func TestEngineApplyAccountAdjustmentReturnsErrorForEmptyBatch(t *testing.T) {
	engine := newEngineForTests(t)
	defer engine.Stop()

	rejects, err := engine.ApplyAccountAdjustment(param.NewAccountIDFromInt(1), nil)
	if rejects.IsSet() {
		t.Fatalf("ApplyAccountAdjustment() rejects = %v, want none", rejects)
	}
	if err == nil {
		t.Fatal("ApplyAccountAdjustment() error = nil, want non-nil")
	}
	if !strings.Contains(err.Error(), "adjustment list is empty") {
		t.Fatalf("ApplyAccountAdjustment() error = %q, want to contain %q", err.Error(), "adjustment list is empty")
	}
}

func TestEngineApplyAccountAdjustmentReturnsBatchReject(t *testing.T) {
	builder, err := NewEngineBuilder()
	if err != nil {
		t.Fatalf("NewEngineBuilder() error = %v", err)
	}
	builder.AccountAdjustmentPolicy(&engineTestRejectingAdjustmentPolicy{name: "adjustment-reject"})

	engine, err := builder.Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	rejects, err := engine.ApplyAccountAdjustment(
		param.NewAccountIDFromInt(1),
		[]model.AccountAdjustment{model.NewAccountAdjustment()},
	)
	if err != nil {
		t.Fatalf("ApplyAccountAdjustment() error = %v", err)
	}
	if !rejects.IsSet() {
		t.Fatal("ApplyAccountAdjustment() rejects.IsSet() = false, want true")
	}
	batchReject, ok := rejects.Get()
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

	builder, err := NewEngineBuilder()
	if err != nil {
		t.Fatalf("NewEngineBuilder() error = %v", err)
	}

	engine, err := builder.Build()
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

func (engineTestStartPolicy) CheckPreTradeStart(pretrade.Context, model.Order) []reject.Reject {
	return nil
}

func (engineTestStartPolicy) ApplyExecutionReport(model.ExecutionReport) bool {
	return false
}

type engineTestRejectingAdjustmentPolicy struct {
	name string
}

func (engineTestRejectingAdjustmentPolicy) Close() {}

func (p engineTestRejectingAdjustmentPolicy) Name() string {
	return p.name
}

func (p *engineTestRejectingAdjustmentPolicy) ApplyAccountAdjustment(
	accountadjustment.Context,
	param.AccountID,
	model.AccountAdjustment,
	tx.Mutations,
) []reject.Reject {
	return reject.NewSingleItemList(
		reject.CodeOther,
		p.name,
		"adjustment rejected",
		"rejected in test policy",
		reject.ScopeAccount,
	)
}
