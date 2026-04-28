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

package policies

import (
	"errors"
	"fmt"
	"strings"
	"testing"

	"go.openpit.dev/openpit"
	"go.openpit.dev/openpit/internal/custompolicy"
	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pretrade"
	"go.openpit.dev/openpit/reject"
	"go.openpit.dev/openpit/tx"
)

func TestCheckPreTradeStartPolicyConstructorAndBuiltinMethods(t *testing.T) {
	policy := newCheckStartPolicy(native.CreatePretradePoliciesOrderValidationPolicy)
	if got := policy.Name(); got != "OrderValidationPolicy" {
		t.Fatalf("Name() = %q, want %q", got, "OrderValidationPolicy")
	}

	policy.Close()
	policy.Close()
}

func TestCheckPreTradeStartPolicyWithErrorSuccessAndFailure(t *testing.T) {
	policy, err := newCheckPreTradeStartPolicyWithError(
		func() (native.PretradeCheckPreTradeStartPolicy, error) {
			params := []native.PretradePoliciesOrderSizeLimitParam{
				native.NewPretradePoliciesOrderSizeLimitParam(
					"USD",
					mustQuantity(t, "10").Handle(),
					mustVolume(t, "1000").Handle(),
				),
			}
			return native.CreatePretradePoliciesOrderSizeLimitPolicy(params)
		},
	)
	if err != nil {
		t.Fatalf("newCheckPreTradeStartPolicyWithError() error = %v, want nil", err)
	}
	if got := policy.Name(); got != "OrderSizeLimitPolicy" {
		t.Fatalf("Name() = %q, want %q", got, "OrderSizeLimitPolicy")
	}
	policy.Close()

	erroredPolicy, err := newCheckPreTradeStartPolicyWithError(
		func() (native.PretradeCheckPreTradeStartPolicy, error) {
			return native.CreatePretradePoliciesOrderSizeLimitPolicy(nil)
		},
	)
	if err == nil {
		t.Fatal("newCheckPreTradeStartPolicyWithError() error = nil, want non-nil")
	}
	if !strings.Contains(err.Error(), "parameter list is empty") {
		t.Fatalf("error = %q, want to contain %q", err.Error(), "parameter list is empty")
	}
	assertPanicContains(t, func() { _ = erroredPolicy.Name() })
}

func TestCheckPreTradeStartPolicyTakeNativeTransfersOwnership(t *testing.T) {
	policy := NewOrderValidation().(*checkStartPolicy)
	handle := policy.TakeHandle()
	if handle == nil {
		t.Fatal("TakeHandle() = nil, want non-nil")
	}

	policy.Close()
	native.DestroyPretradeCheckPreTradeStartPolicy(handle)

	assertPanicContains(t, func() { policy.TakeHandle() })
	assertPanicContains(t, func() { _ = policy.Name() })
}

func TestPreTradePolicyWithErrorSuccessAndFailure(t *testing.T) {
	policy, err := newPolicyWithError(
		func() (native.PretradePreTradePolicy, error) {
			return custompolicy.StartPreTrade(
				&policiesTestNoopMainPolicy{name: "main-policy-adapter"},
			)
		},
	)
	if err != nil {
		t.Fatalf("newPreTradePolicyWithError() error = %v, want nil", err)
	}
	if got := policy.Name(); got != "main-policy-adapter" {
		t.Fatalf("Name() = %q, want %q", got, "main-policy-adapter")
	}

	policy.Close()
	policy.Close()

	erroredPolicy, err := newPolicyWithError(
		func() (native.PretradePreTradePolicy, error) {
			return nil, errors.New("forced construction failure")
		},
	)
	if err == nil {
		t.Fatal("newPreTradePolicyWithError() error = nil, want non-nil")
	}
	if !strings.Contains(err.Error(), "forced construction failure") {
		t.Fatalf("error = %q, want to contain %q", err.Error(), "forced construction failure")
	}
	assertPanicContains(t, func() { _ = erroredPolicy.Name() })
}

func TestPreTradePolicyTakeNativeTransfersOwnership(t *testing.T) {
	policy, err := newPolicyWithError(
		func() (native.PretradePreTradePolicy, error) {
			return custompolicy.StartPreTrade(
				&policiesTestNoopMainPolicy{name: "main-policy-take"},
			)
		},
	)
	if err != nil {
		t.Fatalf("newPreTradePolicyWithError() error = %v, want nil", err)
	}

	handle := policy.TakeHandle()
	if handle == nil {
		t.Fatal("TakeHandle() = nil, want non-nil")
	}
	policy.Close()
	native.DestroyPretradePreTradePolicy(handle)

	assertPanicContains(t, func() { policy.TakeHandle() })
	assertPanicContains(t, func() { _ = policy.Name() })
}

func TestNewOrderValidationPolicyEngineFlow(t *testing.T) {
	policy := NewOrderValidation()
	if got := policy.Name(); got != "OrderValidationPolicy" {
		t.Fatalf("Name() = %q, want %q", got, "OrderValidationPolicy")
	}
	t.Cleanup(policy.Close)

	engine := newEngineWithStartPolicy(t, policy)

	passRequest, passRejects, err := engine.StartPreTrade(newValidOrder(t, "100", "2"))
	if err != nil {
		t.Fatalf("StartPreTrade(valid) error = %v", err)
	}
	if len(passRejects) != 0 {
		t.Fatalf("StartPreTrade(valid) rejects = %v, want none", passRejects)
	}
	if passRequest == nil {
		t.Fatal("StartPreTrade(valid) request = nil, want non-nil")
	}
	passRequest.Close()

	rejectRequest, rejects, err := engine.StartPreTrade(newValidOrder(t, "100", "0"))
	if err != nil {
		t.Fatalf("StartPreTrade(invalid) error = %v", err)
	}
	if rejectRequest != nil {
		t.Fatal("StartPreTrade(invalid) request != nil, want nil")
	}
	if len(rejects) != 1 {
		t.Fatalf("StartPreTrade(invalid) reject len = %d, want 1", len(rejects))
	}
	if rejects[0].Code != reject.CodeInvalidFieldValue {
		t.Fatalf("reject code = %v, want %v", rejects[0].Code, reject.CodeInvalidFieldValue)
	}
	if rejects[0].Policy != "OrderValidationPolicy" {
		t.Fatalf("reject policy = %q, want %q", rejects[0].Policy, "OrderValidationPolicy")
	}
	if rejects[0].Reason != "order quantity must be non-zero" {
		t.Fatalf(
			"reject reason = %q, want %q",
			rejects[0].Reason,
			"order quantity must be non-zero",
		)
	}
	if !strings.Contains(rejects[0].Details, "requested quantity 0") {
		t.Fatalf("reject details = %q, want to contain %q", rejects[0].Details, "requested quantity 0")
	}
}

func TestNewRateLimitPolicyEngineFlow(t *testing.T) {
	policy := NewRateLimitPolicy(1, 60)
	if got := policy.Name(); got != "RateLimitPolicy" {
		t.Fatalf("Name() = %q, want %q", got, "RateLimitPolicy")
	}
	t.Cleanup(policy.Close)

	engine := newEngineWithStartPolicy(t, policy)

	firstRequest, firstRejects, err := engine.StartPreTrade(newValidOrder(t, "100", "1"))
	if err != nil {
		t.Fatalf("first StartPreTrade() error = %v", err)
	}
	if len(firstRejects) != 0 {
		t.Fatalf("first StartPreTrade() rejects = %v, want none", firstRejects)
	}
	if firstRequest == nil {
		t.Fatal("first StartPreTrade() request = nil, want non-nil")
	}
	firstRequest.Close()

	secondRequest, secondRejects, err := engine.StartPreTrade(newValidOrder(t, "100", "1"))
	if err != nil {
		t.Fatalf("second StartPreTrade() error = %v", err)
	}
	if secondRequest != nil {
		t.Fatal("second StartPreTrade() request != nil, want nil")
	}
	if len(secondRejects) != 1 {
		t.Fatalf("second StartPreTrade() reject len = %d, want 1", len(secondRejects))
	}
	if secondRejects[0].Code != reject.CodeRateLimitExceeded {
		t.Fatalf("reject code = %v, want %v", secondRejects[0].Code, reject.CodeRateLimitExceeded)
	}
	if secondRejects[0].Reason != "rate limit exceeded" {
		t.Fatalf("reject reason = %q, want %q", secondRejects[0].Reason, "rate limit exceeded")
	}
	if !strings.Contains(secondRejects[0].Details, "max allowed: 1") {
		t.Fatalf("reject details = %q, want to contain %q", secondRejects[0].Details, "max allowed: 1")
	}
}

func TestNewOrderSizeLimitPolicyValidationAndEngineFlow(t *testing.T) {
	_, err := NewOrderSizeLimitPolicy()
	if err == nil {
		t.Fatal("NewOrderSizeLimitPolicy() error = nil, want non-nil")
	}
	if !strings.Contains(err.Error(), "parameter list is empty") {
		t.Fatalf("error = %q, want to contain %q", err.Error(), "parameter list is empty")
	}

	invalidPolicy, err := NewOrderSizeLimitPolicy(OrderSizeLimit{
		SettlementAsset: param.Asset{},
		MaxQuantity:     mustQuantity(t, "10"),
		MaxNotional:     mustVolume(t, "1000"),
	})
	if invalidPolicy != nil {
		invalidPolicy.Close()
	}
	if err == nil {
		t.Fatal("NewOrderSizeLimitPolicy(invalid) error = nil, want non-nil")
	}
	if !strings.Contains(err.Error(), "settlement asset is invalid") {
		t.Fatalf("error = %q, want to contain %q", err.Error(), "settlement asset is invalid")
	}

	policy, err := NewOrderSizeLimitPolicy(OrderSizeLimit{
		SettlementAsset: mustPolicyAsset(t, "USD"),
		MaxQuantity:     mustQuantity(t, "10"),
		MaxNotional:     mustVolume(t, "1000"),
	})
	if err != nil {
		t.Fatalf("NewOrderSizeLimitPolicy(valid) error = %v", err)
	}
	if got := policy.Name(); got != "OrderSizeLimitPolicy" {
		t.Fatalf("Name() = %q, want %q", got, "OrderSizeLimitPolicy")
	}
	t.Cleanup(policy.Close)

	engine := newEngineWithStartPolicy(t, policy)

	passRequest, passRejects, err := engine.StartPreTrade(newValidOrder(t, "100", "10"))
	if err != nil {
		t.Fatalf("StartPreTrade(valid) error = %v", err)
	}
	if len(passRejects) != 0 {
		t.Fatalf("StartPreTrade(valid) rejects = %v, want none", passRejects)
	}
	if passRequest == nil {
		t.Fatal("StartPreTrade(valid) request = nil, want non-nil")
	}
	passRequest.Close()

	rejectRequest, rejects, err := engine.StartPreTrade(newValidOrder(t, "90", "11"))
	if err != nil {
		t.Fatalf("StartPreTrade(reject) error = %v", err)
	}
	if rejectRequest != nil {
		t.Fatal("StartPreTrade(reject) request != nil, want nil")
	}
	if len(rejects) != 1 {
		t.Fatalf("StartPreTrade(reject) reject len = %d, want 1", len(rejects))
	}
	if rejects[0].Code != reject.CodeOrderQtyExceedsLimit {
		t.Fatalf("reject code = %v, want %v", rejects[0].Code, reject.CodeOrderQtyExceedsLimit)
	}
	if rejects[0].Reason != "order quantity exceeded" {
		t.Fatalf("reject reason = %q, want %q", rejects[0].Reason, "order quantity exceeded")
	}
	if !strings.Contains(rejects[0].Details, "max allowed: 10") {
		t.Fatalf("reject details = %q, want to contain %q", rejects[0].Details, "max allowed: 10")
	}
}

func TestNewPnlKillSwitchPolicyValidationAndEngineFlow(t *testing.T) {
	_, err := NewPnlKillSwitchPolicy()
	if err == nil {
		t.Fatal("NewPnlKillSwitchPolicy() error = nil, want non-nil")
	}
	if !strings.Contains(err.Error(), "parameter list is empty") {
		t.Fatalf("error = %q, want to contain %q", err.Error(), "parameter list is empty")
	}

	nonPositivePolicy, err := NewPnlKillSwitchPolicy(PnlKillSwitchBarrier{
		SettlementAsset: mustPolicyAsset(t, "USD"),
		Barrier:         mustPnl(t, "0"),
	})
	if nonPositivePolicy != nil {
		nonPositivePolicy.Close()
	}
	if err == nil {
		t.Fatal("NewPnlKillSwitchPolicy(non-positive) error = nil, want non-nil")
	}
	if !strings.Contains(err.Error(), "barrier must be positive") {
		t.Fatalf("error = %q, want to contain %q", err.Error(), "barrier must be positive")
	}

	policy, err := NewPnlKillSwitchPolicy(PnlKillSwitchBarrier{
		SettlementAsset: mustPolicyAsset(t, "USD"),
		Barrier:         mustPnl(t, "500"),
	})
	if err != nil {
		t.Fatalf("NewPnlKillSwitchPolicy(valid) error = %v", err)
	}
	if got := policy.Name(); got != "PnlKillSwitchPolicy" {
		t.Fatalf("Name() = %q, want %q", got, "PnlKillSwitchPolicy")
	}
	t.Cleanup(policy.Close)

	engine := newEngineWithStartPolicy(t, policy)

	passRequest, passRejects, err := engine.StartPreTrade(newValidOrder(t, "100", "1"))
	if err != nil {
		t.Fatalf("StartPreTrade(valid) error = %v", err)
	}
	if len(passRejects) != 0 {
		t.Fatalf("StartPreTrade(valid) rejects = %v, want none", passRejects)
	}
	if passRequest == nil {
		t.Fatal("StartPreTrade(valid) request = nil, want non-nil")
	}
	passRequest.Close()

	result, err := engine.ApplyExecutionReport(newExecutionReportWithPnl(t, "-600"))
	if err != nil {
		t.Fatalf("ApplyExecutionReport() error = %v", err)
	}
	if !result.KillSwitchTriggered {
		t.Fatal("KillSwitchTriggered = false, want true")
	}

	rejectRequest, rejects, err := engine.StartPreTrade(newValidOrder(t, "100", "1"))
	if err != nil {
		t.Fatalf("StartPreTrade(kill-switch) error = %v", err)
	}
	if rejectRequest != nil {
		t.Fatal("StartPreTrade(kill-switch) request != nil, want nil")
	}
	if len(rejects) != 1 {
		t.Fatalf("StartPreTrade(kill-switch) reject len = %d, want 1", len(rejects))
	}
	if rejects[0].Code != reject.CodePnlKillSwitchTriggered {
		t.Fatalf(
			"reject code = %v, want %v",
			rejects[0].Code,
			reject.CodePnlKillSwitchTriggered,
		)
	}
	if rejects[0].Reason != "pnl kill switch triggered" {
		t.Fatalf("reject reason = %q, want %q", rejects[0].Reason, "pnl kill switch triggered")
	}
	if !strings.Contains(rejects[0].Details, "max allowed loss: 500") {
		t.Fatalf("reject details = %q, want to contain %q", rejects[0].Details, "max allowed loss: 500")
	}
}

type policiesTestNoopMainPolicy struct {
	name string
}

func (policiesTestNoopMainPolicy) Close() {}

func (p *policiesTestNoopMainPolicy) Name() string {
	return p.name
}

func (policiesTestNoopMainPolicy) PerformPreTradeCheck(
	pretrade.Context,
	model.Order,
	tx.Mutations,
) []reject.Reject {
	return nil
}

func (policiesTestNoopMainPolicy) ApplyExecutionReport(model.ExecutionReport) bool {
	return false
}

func newEngineWithStartPolicy(
	t *testing.T,
	policy ...pretrade.BuiltinPolicy,
) *openpit.Engine {
	t.Helper()

	builder, err := openpit.NewEngineBuilder()
	if err != nil {
		t.Fatalf("NewEngineBuilder() error = %v", err)
	}
	builder.BuiltinCheckPreTradeStartPolicy(policy...)

	engine, err := builder.Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	t.Cleanup(engine.Stop)
	return engine
}

func newValidOrder(t *testing.T, price string, quantity string) model.Order {
	t.Helper()

	result := model.NewOrder()
	operation := result.EnsureOperationView()
	operation.SetInstrument(
		param.NewInstrument(mustPolicyAsset(t, "AAPL"), mustPolicyAsset(t, "USD")),
	)
	operation.SetAccountID(param.NewAccountIDFromInt(99224416))
	operation.SetSide(param.SideBuy)
	operation.SetTradeAmount(param.NewQuantityTradeAmount(mustQuantity(t, quantity)))
	operation.SetPrice(mustPrice(t, price))
	return result
}

func newExecutionReportWithPnl(t *testing.T, pnlValue string) model.ExecutionReport {
	t.Helper()

	result := model.NewExecutionReport()
	operation := model.NewExecutionReportOperation()
	operation.SetInstrument(
		param.NewInstrument(mustPolicyAsset(t, "AAPL"), mustPolicyAsset(t, "USD")),
	)
	operation.SetAccountID(param.NewAccountIDFromInt(99224416))
	operation.SetSide(param.SideBuy)
	result.SetOperation(operation)

	financialImpact := model.NewExecutionReportFinancialImpact()
	financialImpact.SetPnl(mustPnl(t, pnlValue))
	financialImpact.SetFee(mustFee(t, 0))
	result.SetFinancialImpact(financialImpact)
	return result
}

func assertPanicContains(t *testing.T, fn func()) {
	t.Helper()

	const want = "built-in policy is already closed"

	defer func() {
		recovered := recover()
		if recovered == nil {
			t.Fatalf("panic = nil, want non-nil panic containing %q", want)
		}
		if !strings.Contains(fmt.Sprint(recovered), want) {
			t.Fatalf("panic = %q, want to contain %q", fmt.Sprint(recovered), want)
		}
	}()

	fn()
}

func mustPrice(t *testing.T, source string) param.Price {
	t.Helper()

	value, err := param.NewPriceFromString(source)
	if err != nil {
		t.Fatalf("NewPriceFromString(%q) error = %v", source, err)
	}
	return value
}

func mustPolicyAsset(t *testing.T, value string) param.Asset {
	t.Helper()
	asset, err := param.NewAsset(value)
	if err != nil {
		t.Fatalf("NewAsset(%q) error = %v", value, err)
	}
	return asset
}

func mustQuantity(t *testing.T, source string) param.Quantity {
	t.Helper()

	value, err := param.NewQuantityFromString(source)
	if err != nil {
		t.Fatalf("NewQuantityFromString(%q) error = %v", source, err)
	}
	return value
}

func mustVolume(t *testing.T, source string) param.Volume {
	t.Helper()

	value, err := param.NewVolumeFromString(source)
	if err != nil {
		t.Fatalf("NewVolumeFromString(%q) error = %v", source, err)
	}
	return value
}

func mustPnl(t *testing.T, source string) param.Pnl {
	t.Helper()

	value, err := param.NewPnlFromString(source)
	if err != nil {
		t.Fatalf("NewPnlFromString(%q) error = %v", source, err)
	}
	return value
}

func mustFee(t *testing.T, source int64) param.Fee {
	t.Helper()

	value, err := param.NewFeeFromInt(source)
	if err != nil {
		t.Fatalf("NewFeeFromInt(%d) error = %v", source, err)
	}
	return value
}
