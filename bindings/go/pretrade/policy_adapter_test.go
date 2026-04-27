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

package pretrade

import (
	"runtime/cgo"
	"testing"

	"github.com/openpitkit/pit-go/internal/callback"
	"github.com/openpitkit/pit-go/internal/native"
	"github.com/openpitkit/pit-go/model"
	"github.com/openpitkit/pit-go/reject"
	"github.com/openpitkit/pit-go/tx"
)

type clientPayloadTestOrder struct {
	model.Order
	Route string
}

type clientPayloadTestReport struct {
	model.ExecutionReport
	VenueExecID string
}

func TestSafeClientPolicyRejectsMissingOrderPayload(t *testing.T) {
	wrapped := NewSafeClientCheckPreTradeStartPolicy(&clientPayloadTestPolicy{})

	rejects := wrapped.CheckPreTradeStart(Context{}, model.NewOrder())
	if len(rejects) != 1 {
		t.Fatalf("CheckPreTradeStart() reject len = %d, want 1", len(rejects))
	}
	if rejects[0].Code != reject.CodeOther {
		t.Fatalf("reject code = %v, want %v", rejects[0].Code, reject.CodeOther)
	}
}

func TestSafeClientPolicyIgnoresMissingReportPayload(t *testing.T) {
	wrapped := NewSafeClientCheckPreTradeStartPolicy(&clientPayloadTestPolicy{killSwitch: true})

	if wrapped.ApplyExecutionReport(model.NewExecutionReport()) {
		t.Fatal("ApplyExecutionReport() = true, want false")
	}
}

func TestSafeClientPolicyCastsOrderPayload(t *testing.T) {
	policy := &clientPayloadTestPolicy{}
	wrapped := NewSafeClientCheckPreTradeStartPolicy(policy)
	order := clientPayloadTestOrder{Order: model.NewOrder(), Route: "dark-pool"}

	rejects := wrapped.CheckPreTradeStart(Context{}, orderWithPayload(t, order))
	if len(rejects) != 0 {
		t.Fatalf("CheckPreTradeStart() rejects = %v, want none", rejects)
	}
	if policy.order.Route != order.Route {
		t.Fatalf("order route = %q, want %q", policy.order.Route, order.Route)
	}
}

func TestSafeClientPolicyCastsReportPayload(t *testing.T) {
	policy := &clientPayloadTestPolicy{killSwitch: true}
	wrapped := NewSafeClientCheckPreTradeStartPolicy(policy)
	report := clientPayloadTestReport{
		ExecutionReport: model.NewExecutionReport(),
		VenueExecID:     "venue-fill-1",
	}

	if !wrapped.ApplyExecutionReport(reportWithPayload(t, report)) {
		t.Fatal("ApplyExecutionReport() = false, want true")
	}
	if policy.report.VenueExecID != report.VenueExecID {
		t.Fatalf("report venue id = %q, want %q", policy.report.VenueExecID, report.VenueExecID)
	}
}

func TestUnsafeFastClientPolicyCastsOrderPayload(t *testing.T) {
	policy := &clientPayloadTestPolicy{}
	wrapped := NewUnsafeFastClientCheckPreTradeStartPolicy(policy)
	order := clientPayloadTestOrder{Order: model.NewOrder(), Route: "fast-lane"}

	rejects := wrapped.CheckPreTradeStart(Context{}, orderWithPayload(t, order))
	if len(rejects) != 0 {
		t.Fatalf("CheckPreTradeStart() rejects = %v, want none", rejects)
	}
	if policy.order.Route != order.Route {
		t.Fatalf("order route = %q, want %q", policy.order.Route, order.Route)
	}
}

func TestSafeClientPreTradePolicyRejectsMissingOrderPayload(t *testing.T) {
	wrapped := NewSafeClientPreTradePolicy(&clientPayloadTestPolicy{})

	rejects := wrapped.PerformPreTradeCheck(Context{}, model.NewOrder(), tx.Mutations{})
	if len(rejects) != 1 {
		t.Fatalf("PerformPreTradeCheck() reject len = %d, want 1", len(rejects))
	}
	if rejects[0].Code != reject.CodeOther {
		t.Fatalf("reject code = %v, want %v", rejects[0].Code, reject.CodeOther)
	}
	if rejects[0].Scope != reject.ScopeOrder {
		t.Fatalf("reject scope = %v, want %v", rejects[0].Scope, reject.ScopeOrder)
	}
}

func TestSafeClientPreTradePolicyCastsOrderPayload(t *testing.T) {
	policy := &clientPayloadTestPolicy{}
	wrapped := NewSafeClientPreTradePolicy(policy)
	order := clientPayloadTestOrder{Order: model.NewOrder(), Route: "safe-main"}

	rejects := wrapped.PerformPreTradeCheck(Context{}, orderWithPayload(t, order), tx.Mutations{})
	if len(rejects) != 0 {
		t.Fatalf("PerformPreTradeCheck() rejects = %v, want none", rejects)
	}
	if policy.order.Route != order.Route {
		t.Fatalf("order route = %q, want %q", policy.order.Route, order.Route)
	}
}

func TestSafeClientPreTradePolicyApplyExecutionReportIgnoresMissingPayload(t *testing.T) {
	wrapped := NewSafeClientPreTradePolicy(&clientPayloadTestPolicy{killSwitch: true})

	if wrapped.ApplyExecutionReport(model.NewExecutionReport()) {
		t.Fatal("ApplyExecutionReport() = true, want false")
	}
}

func TestUnsafeFastClientPreTradePolicyCastsOrderPayload(t *testing.T) {
	policy := &clientPayloadTestPolicy{}
	wrapped := NewUnsafeFastClientPreTradePolicy(policy)
	order := clientPayloadTestOrder{Order: model.NewOrder(), Route: "unsafe-main"}

	rejects := wrapped.PerformPreTradeCheck(Context{}, orderWithPayload(t, order), tx.Mutations{})
	if len(rejects) != 0 {
		t.Fatalf("PerformPreTradeCheck() rejects = %v, want none", rejects)
	}
	if policy.order.Route != order.Route {
		t.Fatalf("order route = %q, want %q", policy.order.Route, order.Route)
	}
}

func TestUnsafeFastClientPreTradePolicyApplyExecutionReportCastsReport(t *testing.T) {
	policy := &clientPayloadTestPolicy{killSwitch: true}
	wrapped := NewUnsafeFastClientPreTradePolicy(policy)
	report := clientPayloadTestReport{
		ExecutionReport: model.NewExecutionReport(),
		VenueExecID:     "unsafe-main-report",
	}

	if !wrapped.ApplyExecutionReport(reportWithPayload(t, report)) {
		t.Fatal("ApplyExecutionReport() = false, want true")
	}
	if policy.report.VenueExecID != report.VenueExecID {
		t.Fatalf("report venue id = %q, want %q", policy.report.VenueExecID, report.VenueExecID)
	}
}

func TestUnsafeFastClientCheckPreTradeStartPolicyApplyExecutionReportCastsReport(t *testing.T) {
	policy := &clientPayloadTestPolicy{killSwitch: true}
	wrapped := NewUnsafeFastClientCheckPreTradeStartPolicy(policy)
	report := clientPayloadTestReport{
		ExecutionReport: model.NewExecutionReport(),
		VenueExecID:     "unsafe-start-report",
	}

	if !wrapped.ApplyExecutionReport(reportWithPayload(t, report)) {
		t.Fatal("ApplyExecutionReport() = false, want true")
	}
	if policy.report.VenueExecID != report.VenueExecID {
		t.Fatalf("report venue id = %q, want %q", policy.report.VenueExecID, report.VenueExecID)
	}
}

func TestSafeClientCheckPreTradeStartPolicyNameAndClose(t *testing.T) {
	policy := &clientPayloadTestPolicy{}
	wrapped := NewSafeClientCheckPreTradeStartPolicy(policy)

	if got := wrapped.Name(); got != policy.Name() {
		t.Fatalf("Name() = %q, want %q", got, policy.Name())
	}
	wrapped.Close()
	if policy.closeCalls != 1 {
		t.Fatalf("closeCalls = %d, want 1", policy.closeCalls)
	}
}

func TestUnsafeFastClientCheckPreTradeStartPolicyNameAndClose(t *testing.T) {
	policy := &clientPayloadTestPolicy{}
	wrapped := NewUnsafeFastClientCheckPreTradeStartPolicy(policy)

	if got := wrapped.Name(); got != policy.Name() {
		t.Fatalf("Name() = %q, want %q", got, policy.Name())
	}
	wrapped.Close()
	if policy.closeCalls != 1 {
		t.Fatalf("closeCalls = %d, want 1", policy.closeCalls)
	}
}

func TestSafeClientPreTradePolicyNameAndClose(t *testing.T) {
	policy := &clientPayloadTestPolicy{}
	wrapped := NewSafeClientPreTradePolicy(policy)

	if got := wrapped.Name(); got != policy.Name() {
		t.Fatalf("Name() = %q, want %q", got, policy.Name())
	}
	wrapped.Close()
	if policy.closeCalls != 1 {
		t.Fatalf("closeCalls = %d, want 1", policy.closeCalls)
	}
}

func TestUnsafeFastClientPreTradePolicyNameAndClose(t *testing.T) {
	policy := &clientPayloadTestPolicy{}
	wrapped := NewUnsafeFastClientPreTradePolicy(policy)

	if got := wrapped.Name(); got != policy.Name() {
		t.Fatalf("Name() = %q, want %q", got, policy.Name())
	}
	wrapped.Close()
	if policy.closeCalls != 1 {
		t.Fatalf("closeCalls = %d, want 1", policy.closeCalls)
	}
}

func TestSafePayloadReturnsFalseForInvalidHandlePointer(t *testing.T) {
	handle := cgo.NewHandle(42)
	userData := callback.NewUserDataFromHandle(handle)
	handle.Delete()

	payload, ok := safePayload[clientPayloadTestOrder](userData)
	if ok {
		t.Fatal("safePayload() ok = true, want false")
	}
	if payload.Route != "" {
		t.Fatalf("safePayload() route = %q, want empty", payload.Route)
	}
}

func TestSafeClientPreTradePolicyApplyExecutionReportReturnsFalseOnMismatchedPayload(t *testing.T) {
	wrapped := NewSafeClientPreTradePolicy(&clientPayloadTestPolicy{killSwitch: true})

	if wrapped.ApplyExecutionReport(reportWithAnyPayload(t, 42)) {
		t.Fatal("ApplyExecutionReport() = true, want false")
	}
}

type clientPayloadTestPolicy struct {
	order      clientPayloadTestOrder
	report     clientPayloadTestReport
	killSwitch bool
	closeCalls int
}

func (p *clientPayloadTestPolicy) Close() { p.closeCalls++ }

func (p *clientPayloadTestPolicy) Name() string {
	return "client-payload-test"
}

func (p *clientPayloadTestPolicy) CheckPreTradeStart(
	_ Context,
	order clientPayloadTestOrder,
) reject.List {
	p.order = order
	return nil
}

func (p *clientPayloadTestPolicy) PerformPreTradeCheck(
	_ Context,
	order clientPayloadTestOrder,
	_ tx.Mutations,
) reject.List {
	p.order = order
	return nil
}

func (p *clientPayloadTestPolicy) ApplyExecutionReport(report clientPayloadTestReport) bool {
	p.report = report
	return p.killSwitch
}

func orderWithPayload(t *testing.T, order clientPayloadTestOrder) model.Order {
	t.Helper()

	nativeOrder := order.EngineOrder().Native()
	handle := cgo.NewHandle(order)
	t.Cleanup(handle.Delete)
	native.OrderSetUserData(&nativeOrder, callback.NewUserDataFromHandle(handle))
	return model.NewOrderFromNative(nativeOrder)
}

func reportWithPayload(t *testing.T, report clientPayloadTestReport) model.ExecutionReport {
	t.Helper()

	nativeReport := report.EngineExecutionReport().Native()
	handle := cgo.NewHandle(report)
	t.Cleanup(handle.Delete)
	native.ExecutionReportSetUserData(&nativeReport, callback.NewUserDataFromHandle(handle))
	return model.NewExecutionReportFromNative(nativeReport)
}

func reportWithAnyPayload(t *testing.T, payload any) model.ExecutionReport {
	t.Helper()

	nativeReport := model.NewExecutionReport().Native()
	handle := cgo.NewHandle(payload)
	t.Cleanup(handle.Delete)
	native.ExecutionReportSetUserData(&nativeReport, callback.NewUserDataFromHandle(handle))
	return model.NewExecutionReportFromNative(nativeReport)
}
