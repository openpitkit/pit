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
	"runtime/cgo"
	"testing"

	"go.openpit.dev/openpit/accountadjustment"
	"go.openpit.dev/openpit/internal/callback"
	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pretrade"
	"go.openpit.dev/openpit/reject"
	"go.openpit.dev/openpit/tx"
)

type clientEngineTestOrder struct {
	model.Order
	Route string
}

type clientEngineTestReport struct {
	model.ExecutionReport
	VenueExecID string
}

type clientEngineTestAdjustment struct {
	model.AccountAdjustment
	Source string
}

func TestClientEnginePassesClientOrderThroughDeferredRequest(t *testing.T) {
	builder, err := NewClientEngineBuilder[
		clientEngineTestOrder,
		clientEngineTestReport,
		clientEngineTestAdjustment,
	]()
	if err != nil {
		t.Fatalf("NewClientEngineBuilder() error = %v", err)
	}

	startPolicy := &clientEngineTestStartPolicy{}
	mainPolicy := &clientEngineTestMainPolicy{}
	builder.CheckPreTradeStartPolicy(startPolicy)
	builder.PreTradePolicy(mainPolicy)

	engine, err := builder.Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	order := clientEngineTestOrder{Order: model.NewOrder(), Route: "alpha"}
	request, rejects, err := engine.StartPreTrade(order)
	if err != nil {
		t.Fatalf("StartPreTrade() error = %v", err)
	}
	if len(rejects) != 0 {
		t.Fatalf("StartPreTrade() rejects = %v, want none", rejects)
	}
	if startPolicy.order.Route != order.Route {
		t.Fatalf("start order route = %q, want %q", startPolicy.order.Route, order.Route)
	}

	reservation, rejects, err := request.Execute()
	if err != nil {
		t.Fatalf("Execute() error = %v", err)
	}
	if len(rejects) != 0 {
		t.Fatalf("Execute() rejects = %v, want none", rejects)
	}
	defer reservation.Close()
	if mainPolicy.order.Route != order.Route {
		t.Fatalf("main order route = %q, want %q", mainPolicy.order.Route, order.Route)
	}

	request.Close()
	request.Close()
}

func TestClientEnginePassesClientExecutionReport(t *testing.T) {
	builder, err := NewClientEngineBuilder[
		clientEngineTestOrder,
		clientEngineTestReport,
		clientEngineTestAdjustment,
	]()
	if err != nil {
		t.Fatalf("NewClientEngineBuilder() error = %v", err)
	}

	policy := &clientEngineTestStartPolicy{killSwitch: true}
	builder.CheckPreTradeStartPolicy(policy)

	engine, err := builder.Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	report := clientEngineTestReport{
		ExecutionReport: model.NewExecutionReport(),
		VenueExecID:     "exec-1",
	}
	result, err := engine.ApplyExecutionReport(report)
	if err != nil {
		t.Fatalf("ApplyExecutionReport() error = %v", err)
	}
	if !result.KillSwitchTriggered {
		t.Fatal("KillSwitchTriggered = false, want true")
	}
	if policy.report.VenueExecID != report.VenueExecID {
		t.Fatalf(
			"report venue exec id = %q, want %q",
			policy.report.VenueExecID,
			report.VenueExecID,
		)
	}
}

func TestClientEngineExecutePreTradeReleasesPayloadAfterCall(t *testing.T) {
	builder, err := NewClientEngineBuilder[
		clientEngineTestOrder,
		clientEngineTestReport,
		clientEngineTestAdjustment,
	]()
	if err != nil {
		t.Fatalf("NewClientEngineBuilder() error = %v", err)
	}
	builder.PreTradePolicy(&clientEngineTestMainPolicy{})

	engine, err := builder.Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	reservation, rejects, err := engine.ExecutePreTrade(
		clientEngineTestOrder{Order: model.NewOrder(), Route: "direct"},
	)
	if err != nil {
		t.Fatalf("ExecutePreTrade() error = %v", err)
	}
	if len(rejects) != 0 {
		t.Fatalf("ExecutePreTrade() rejects = %v, want none", rejects)
	}
	reservation.Close()
}

func TestClientEnginePassesClientAccountAdjustment(t *testing.T) {
	builder, err := NewClientEngineBuilder[
		clientEngineTestOrder,
		clientEngineTestReport,
		clientEngineTestAdjustment,
	]()
	if err != nil {
		t.Fatalf("NewClientEngineBuilder() error = %v", err)
	}

	policy := &clientEngineTestAccountAdjustmentPolicy{}
	builder.AccountAdjustmentPolicy(policy)

	engine, err := builder.Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	adjustment := clientEngineTestAdjustment{
		AccountAdjustment: model.NewAccountAdjustment(),
		Source:            "ops-feed",
	}
	rejects, err := engine.ApplyAccountAdjustment(
		param.NewAccountIDFromInt(1),
		[]clientEngineTestAdjustment{adjustment},
	)
	if err != nil {
		t.Fatalf("ApplyAccountAdjustment() error = %v", err)
	}
	if rejects.IsSet() {
		t.Fatalf("ApplyAccountAdjustment() rejects = %v, want none", rejects)
	}
	if policy.adjustment.Source != adjustment.Source {
		t.Fatalf(
			"adjustment source = %q, want %q",
			policy.adjustment.Source,
			adjustment.Source,
		)
	}
}

func TestClientEngineStartPreTradeRejectReleasesPayload(t *testing.T) {
	builder, err := NewClientEngineBuilder[
		clientEngineTestOrder,
		clientEngineTestReport,
		clientEngineTestAdjustment,
	]()
	if err != nil {
		t.Fatalf("NewClientEngineBuilder() error = %v", err)
	}
	builder.CheckPreTradeStartPolicy(&clientEngineTestRejectingStartPolicy{})

	engine, err := builder.Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	order := clientEngineTestOrder{Order: model.NewOrder(), Route: "reject-start"}
	// Smoke-check the payload-release path by calling twice with the same flow.
	request, rejects, err := engine.StartPreTrade(order)
	if err != nil {
		t.Fatalf("StartPreTrade() error = %v", err)
	}
	if request != nil {
		t.Fatal("StartPreTrade() request != nil, want nil")
	}
	if len(rejects) != 1 {
		t.Fatalf("StartPreTrade() reject len = %d, want 1", len(rejects))
	}

	requestAgain, rejectsAgain, err := engine.StartPreTrade(order)
	if err != nil {
		t.Fatalf("second StartPreTrade() error = %v", err)
	}
	if requestAgain != nil {
		t.Fatal("second StartPreTrade() request != nil, want nil")
	}
	if len(rejectsAgain) != 1 {
		t.Fatalf("second StartPreTrade() reject len = %d, want 1", len(rejectsAgain))
	}
}

func TestClientEngineExecutePreTradeRejectReleasesPayload(t *testing.T) {
	builder, err := NewClientEngineBuilder[
		clientEngineTestOrder,
		clientEngineTestReport,
		clientEngineTestAdjustment,
	]()
	if err != nil {
		t.Fatalf("NewClientEngineBuilder() error = %v", err)
	}
	builder.PreTradePolicy(&clientEngineTestRejectingMainPolicy{})

	engine, err := builder.Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	reservation, rejects, err := engine.ExecutePreTrade(
		clientEngineTestOrder{Order: model.NewOrder(), Route: "reject-main"},
	)
	if err != nil {
		t.Fatalf("ExecutePreTrade() error = %v", err)
	}
	if reservation != nil {
		t.Fatal("ExecutePreTrade() reservation != nil, want nil")
	}
	if len(rejects) != 1 {
		t.Fatalf("ExecutePreTrade() reject len = %d, want 1", len(rejects))
	}
}

func TestClientEngineApplyAccountAdjustmentBatchHandlesMultipleAdjustments(t *testing.T) {
	builder, err := NewClientEngineBuilder[
		clientEngineTestOrder,
		clientEngineTestReport,
		clientEngineTestAdjustment,
	]()
	if err != nil {
		t.Fatalf("NewClientEngineBuilder() error = %v", err)
	}

	policy := &clientEngineTestAccountAdjustmentPolicy{}
	builder.AccountAdjustmentPolicy(policy)

	engine, err := builder.Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	adjustments := []clientEngineTestAdjustment{
		{AccountAdjustment: model.NewAccountAdjustment(), Source: "ops-feed-1"},
		{AccountAdjustment: model.NewAccountAdjustment(), Source: "ops-feed-2"},
	}
	rejects, err := engine.ApplyAccountAdjustment(param.NewAccountIDFromInt(1), adjustments)
	if err != nil {
		t.Fatalf("ApplyAccountAdjustment() error = %v", err)
	}
	if rejects.IsSet() {
		t.Fatalf("ApplyAccountAdjustment() rejects = %v, want none", rejects)
	}
	if len(policy.adjustments) != len(adjustments) {
		t.Fatalf(
			"received adjustments len = %d, want %d",
			len(policy.adjustments),
			len(adjustments),
		)
	}
	if policy.adjustments[0].Source != adjustments[0].Source {
		t.Fatalf(
			"first adjustment source = %q, want %q",
			policy.adjustments[0].Source,
			adjustments[0].Source,
		)
	}
	if policy.adjustments[1].Source != adjustments[1].Source {
		t.Fatalf(
			"second adjustment source = %q, want %q",
			policy.adjustments[1].Source,
			adjustments[1].Source,
		)
	}
}

func TestClientEngineUnsafeFastPanicsOnMismatchedPayload(t *testing.T) {
	builder, err := NewClientEngineBuilder[
		clientEngineTestOrder,
		clientEngineTestReport,
		clientEngineTestAdjustment,
	](UnsafeFastClientPayloadCallbacks())
	if err != nil {
		t.Fatalf("NewClientEngineBuilder() error = %v", err)
	}
	builder.CheckPreTradeStartPolicy(&clientEngineTestStartPolicy{})

	engine, err := builder.Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	didPanic := false
	func() {
		defer func() {
			if recover() != nil {
				didPanic = true
			}
		}()
		_, _, _ = engine.engine.StartPreTrade(orderWithMismatchedPayload(t, 42))
	}()
	if !didPanic {
		t.Fatal("StartPreTrade() panic = nil, want non-nil")
	}
}

type clientEngineTestStartPolicy struct {
	order      clientEngineTestOrder
	report     clientEngineTestReport
	killSwitch bool
}

func (clientEngineTestStartPolicy) Close() {}

func (clientEngineTestStartPolicy) Name() string {
	return "client-engine-test-start"
}

func (p *clientEngineTestStartPolicy) CheckPreTradeStart(
	_ pretrade.Context,
	order clientEngineTestOrder,
) []reject.Reject {
	p.order = order
	return nil
}

func (p *clientEngineTestStartPolicy) ApplyExecutionReport(report clientEngineTestReport) bool {
	p.report = report
	return p.killSwitch
}

type clientEngineTestMainPolicy struct {
	order clientEngineTestOrder
}

func (clientEngineTestMainPolicy) Close() {}

func (clientEngineTestMainPolicy) Name() string {
	return "client-engine-test-main"
}

func (p *clientEngineTestMainPolicy) PerformPreTradeCheck(
	_ pretrade.Context,
	order clientEngineTestOrder,
	_ tx.Mutations,
) []reject.Reject {
	p.order = order
	return nil
}

func (clientEngineTestMainPolicy) ApplyExecutionReport(clientEngineTestReport) bool {
	return false
}

type clientEngineTestAccountAdjustmentPolicy struct {
	adjustment  clientEngineTestAdjustment
	adjustments []clientEngineTestAdjustment
}

func (clientEngineTestAccountAdjustmentPolicy) Close() {}

func (clientEngineTestAccountAdjustmentPolicy) Name() string {
	return "client-engine-test-adjustment"
}

func (p *clientEngineTestAccountAdjustmentPolicy) ApplyAccountAdjustment(
	_ accountadjustment.Context,
	_ param.AccountID,
	adjustment clientEngineTestAdjustment,
	_ tx.Mutations,
) []reject.Reject {
	p.adjustment = adjustment
	p.adjustments = append(p.adjustments, adjustment)
	return nil
}

type clientEngineTestRejectingStartPolicy struct{}

func (clientEngineTestRejectingStartPolicy) Close() {}

func (clientEngineTestRejectingStartPolicy) Name() string {
	return "client-engine-test-reject-start"
}

func (p *clientEngineTestRejectingStartPolicy) CheckPreTradeStart(
	_ pretrade.Context,
	_ clientEngineTestOrder,
) []reject.Reject {
	return reject.NewSingleItemList(
		reject.CodeOther,
		p.Name(),
		"start rejected",
		"reject start stage",
		reject.ScopeOrder,
	)
}

func (clientEngineTestRejectingStartPolicy) ApplyExecutionReport(clientEngineTestReport) bool {
	return false
}

type clientEngineTestRejectingMainPolicy struct{}

func (clientEngineTestRejectingMainPolicy) Close() {}

func (clientEngineTestRejectingMainPolicy) Name() string {
	return "client-engine-test-reject-main"
}

func (p *clientEngineTestRejectingMainPolicy) PerformPreTradeCheck(
	_ pretrade.Context,
	_ clientEngineTestOrder,
	_ tx.Mutations,
) []reject.Reject {
	return reject.NewSingleItemList(
		reject.CodeOther,
		p.Name(),
		"order rejected",
		"reject execute stage",
		reject.ScopeOrder,
	)
}

func (clientEngineTestRejectingMainPolicy) ApplyExecutionReport(clientEngineTestReport) bool {
	return false
}

func orderWithMismatchedPayload(t *testing.T, payload any) model.Order {
	t.Helper()

	order := model.NewOrder()
	nativeOrder := order.Handle()
	handle := cgo.NewHandle(payload)
	t.Cleanup(handle.Delete)
	native.OrderSetUserData(&nativeOrder, callback.NewUserDataFromHandle(handle))
	return model.NewOrderFromHandle(nativeOrder)
}

func TestNewClientPreTradeEngineBuilder(t *testing.T) {
	builder, err := NewClientPreTradeEngineBuilder[clientEngineTestOrder, clientEngineTestReport]()
	if err != nil {
		t.Fatalf("NewClientPreTradeEngineBuilder() error = %v", err)
	}
	builder.Close()
}

func TestNewClientAccountAdjustmentEngineBuilder(t *testing.T) {
	builder, err := NewClientAccountAdjustmentEngineBuilder[clientEngineTestAdjustment]()
	if err != nil {
		t.Fatalf("NewClientAccountAdjustmentEngineBuilder() error = %v", err)
	}
	builder.Close()
}

func TestClientEngineBuilderCloseIsIdempotent(t *testing.T) {
	builder, err := NewClientEngineBuilder[
		clientEngineTestOrder,
		clientEngineTestReport,
		clientEngineTestAdjustment,
	]()
	if err != nil {
		t.Fatalf("NewClientEngineBuilder() error = %v", err)
	}

	builder.Close()
	builder.Close()

	builtBuilder, err := NewClientEngineBuilder[
		clientEngineTestOrder,
		clientEngineTestReport,
		clientEngineTestAdjustment,
	]()
	if err != nil {
		t.Fatalf("NewClientEngineBuilder() error = %v", err)
	}
	engine, err := builtBuilder.Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	builtBuilder.Close()
	builtBuilder.Close()
}

func TestClientEngineBuilderBuildReturnsErrorAfterClose(t *testing.T) {
	builder, err := NewClientEngineBuilder[
		clientEngineTestOrder,
		clientEngineTestReport,
		clientEngineTestAdjustment,
	]()
	if err != nil {
		t.Fatalf("NewClientEngineBuilder() error = %v", err)
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

func TestClientEngineUnsafeFastPreTradePolicyUsesFastAdapter(t *testing.T) {
	builder, err := NewClientEngineBuilder[
		clientEngineTestOrder,
		clientEngineTestReport,
		clientEngineTestAdjustment,
	](UnsafeFastClientPayloadCallbacks())
	if err != nil {
		t.Fatalf("NewClientEngineBuilder() error = %v", err)
	}

	policy := &clientEngineTestMainPolicy{}
	builder.PreTradePolicy(policy)

	engine, err := builder.Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	order := clientEngineTestOrder{Order: model.NewOrder(), Route: "unsafe-main-route"}
	reservation, rejects, err := engine.ExecutePreTrade(order)
	if err != nil {
		t.Fatalf("ExecutePreTrade() error = %v", err)
	}
	if len(rejects) != 0 {
		t.Fatalf("ExecutePreTrade() rejects = %v, want none", rejects)
	}
	defer reservation.Close()
	if policy.order.Route != order.Route {
		t.Fatalf("policy order route = %q, want %q", policy.order.Route, order.Route)
	}
}

func TestClientEngineUnsafeFastAccountAdjustmentPolicyUsesFastAdapter(t *testing.T) {
	builder, err := NewClientEngineBuilder[
		clientEngineTestOrder,
		clientEngineTestReport,
		clientEngineTestAdjustment,
	](UnsafeFastClientPayloadCallbacks())
	if err != nil {
		t.Fatalf("NewClientEngineBuilder() error = %v", err)
	}

	policy := &clientEngineTestAccountAdjustmentPolicy{}
	builder.AccountAdjustmentPolicy(policy)

	engine, err := builder.Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	adjustment := clientEngineTestAdjustment{
		AccountAdjustment: model.NewAccountAdjustment(),
		Source:            "unsafe-fast-adjustment",
	}
	rejects, err := engine.ApplyAccountAdjustment(
		param.NewAccountIDFromInt(1),
		[]clientEngineTestAdjustment{adjustment},
	)
	if err != nil {
		t.Fatalf("ApplyAccountAdjustment() error = %v", err)
	}
	if rejects.IsSet() {
		t.Fatalf("ApplyAccountAdjustment() rejects = %v, want none", rejects)
	}
	if policy.adjustment.Source != adjustment.Source {
		t.Fatalf("adjustment source = %q, want %q", policy.adjustment.Source, adjustment.Source)
	}
}

func TestClientPayloadHandleReleaseNilIsNoop(*testing.T) {
	var payload *clientPayloadHandle
	payload.release()
}
