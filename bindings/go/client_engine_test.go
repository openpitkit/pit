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
	"bytes"
	"errors"
	"runtime"
	"runtime/cgo"
	"strings"
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

type clientEngineFreshModelReport struct {
	lock         []byte
	fee          param.Fee
	feeCurrency  string
	nativeReport native.ExecutionReport
}

func (r *clientEngineFreshModelReport) EngineExecutionReport() model.ExecutionReport {
	currency, err := param.NewAsset(r.feeCurrency)
	if err != nil {
		panic(err)
	}
	fill := model.NewExecutionReportFill()
	fill.SetFee(param.NewMonetaryAmount(r.fee, currency))
	fill.SetLock(r.lock)
	report := model.NewExecutionReport()
	report.SetFill(fill)
	r.nativeReport = report.Handle()
	return report
}

type clientEngineFreshModelOrder struct {
	asset       string
	nativeOrder native.Order
}

func (o *clientEngineFreshModelOrder) EngineOrder() model.Order {
	asset, err := param.NewAsset(o.asset)
	if err != nil {
		panic(err)
	}
	margin := model.NewOrderMargin()
	margin.SetCollateralAsset(asset)
	order := model.NewOrder()
	order.SetMargin(margin)
	operation := order.EnsureOperationView()
	operation.SetAccountID(param.NewAccountIDFromUint64(1))
	o.nativeOrder = order.Handle()
	return order
}

type clientEngineFreshModelAdjustment struct {
	asset            string
	nativeAdjustment native.AccountAdjustment
}

func (a *clientEngineFreshModelAdjustment) EngineAccountAdjustment() model.AccountAdjustment {
	asset, err := param.NewAsset(a.asset)
	if err != nil {
		panic(err)
	}
	operation := model.NewAccountAdjustmentBalanceOperation()
	operation.SetAsset(asset)
	adjustment := model.NewAccountAdjustment()
	adjustment.SetBalanceOperationAndUnsetOtherOperations(operation)
	a.nativeAdjustment = adjustment.Handle()
	return adjustment
}

type clientEngineTestAdjustment struct {
	model.AccountAdjustment
	Source string
}

func TestClientRequestExecuteAfterCloseReturnsExportedError(t *testing.T) {
	request := &ClientRequest{}
	request.Close()
	reservation, rejects, err := request.Execute()
	if reservation != nil || rejects != nil {
		t.Fatalf("Execute() = (%v, %v, %v), want (nil, nil, ErrClientRequestClosed)", reservation, rejects, err)
	}
	if !errors.Is(err, ErrClientRequestClosed) {
		t.Fatalf("Execute() error = %v, want ErrClientRequestClosed", err)
	}
}

func TestClientEnginePassesClientOrderThroughDeferredRequest(t *testing.T) {
	startPolicy := &clientEngineTestStartPolicy{}
	mainPolicy := &clientEngineTestMainPolicy{}
	engine, err := NewClientEngineBuilder[
		clientEngineTestOrder,
		clientEngineTestReport,
		clientEngineTestAdjustment,
	]().
		FullSync().
		PreTrade(startPolicy).
		PreTrade(mainPolicy).
		Build()
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
	policy := &clientEngineTestStartPolicy{killSwitch: true}
	engine, err := NewClientEngineBuilder[
		clientEngineTestOrder,
		clientEngineTestReport,
		clientEngineTestAdjustment,
	]().
		FullSync().
		PreTrade(policy).
		Build()
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
	if len(result.AccountBlocks) == 0 || result.AccountBlocks[0].Code != reject.CodePnlKillSwitchTriggered {
		t.Fatalf("AccountBlocks = %v, want kill switch block", result.AccountBlocks)
	}
	if policy.report.VenueExecID != report.VenueExecID {
		t.Fatalf(
			"report venue exec id = %q, want %q",
			policy.report.VenueExecID,
			report.VenueExecID,
		)
	}
}

func TestClientEngineSafeReportPayloadMismatchReturnsSystemUnavailable(t *testing.T) {
	for _, test := range []struct {
		name    string
		payload any
	}{
		{name: "missing"},
		{name: "mismatched", payload: 42},
	} {
		t.Run(test.name, func(t *testing.T) {
			policy := &clientEngineTestStartPolicy{killSwitch: true}
			engine, err := NewClientEngineBuilder[
				clientEngineTestOrder,
				clientEngineTestReport,
				clientEngineTestAdjustment,
			]().
				FullSync().
				PreTrade(policy).
				Build()
			if err != nil {
				t.Fatalf("Build() error = %v", err)
			}
			defer engine.Stop()

			report := model.NewExecutionReport()
			if test.payload != nil {
				nativeReport := report.Handle()
				handle := cgo.NewHandle(test.payload)
				t.Cleanup(handle.Delete)
				native.ExecutionReportSetUserData(&nativeReport, callback.NewUserDataFromHandle(handle))
				report = model.NewExecutionReportFromHandle(nativeReport)
			}

			result, err := engine.engine.ApplyExecutionReport(report)
			if err != nil {
				t.Fatalf("ApplyExecutionReport() error = %v", err)
			}
			want := reject.AccountBlock{
				Code:    reject.CodeSystemUnavailable,
				Policy:  policy.Name(),
				Reason:  "client execution report payload mismatch",
				Details: "expected client execution report payload type openpit.clientEngineTestReport",
			}
			if len(result.AccountBlocks) != 1 || result.AccountBlocks[0] != want {
				t.Fatalf("AccountBlocks = %v, want [%v]", result.AccountBlocks, want)
			}
			if policy.reportCalled {
				t.Fatal("client policy ApplyExecutionReport() called with invalid payload")
			}
		})
	}
}

func TestClientEngineRetainsFreshExecutionReportModel(t *testing.T) {
	price, err := param.NewPriceFromString("124")
	if err != nil {
		t.Fatalf("NewPriceFromString() error = %v", err)
	}
	lock, err := pretrade.NewLockFromEntries([]pretrade.Entry{
		{PolicyGroupID: model.DefaultPolicyGroupID, Price: price},
	})
	if err != nil {
		t.Fatalf("NewLockFromEntries() error = %v", err)
	}
	fee, err := param.NewFeeFromString("0.1")
	if err != nil {
		t.Fatalf("NewFeeFromString() error = %v", err)
	}
	report := &clientEngineFreshModelReport{
		lock:        lock.Bytes(),
		fee:         fee,
		feeCurrency: "USD",
	}
	policy := &clientEngineGCReportPolicy{t: t}
	engine, err := NewClientEngineBuilder[
		clientEngineTestOrder,
		*clientEngineFreshModelReport,
		clientEngineTestAdjustment,
	]().
		FullSync().
		PreTrade(policy).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	if _, err := engine.ApplyExecutionReport(report); err != nil {
		t.Fatalf("ApplyExecutionReport() error = %v", err)
	}
	if !policy.fillSet {
		t.Fatal("policy report Fill().IsSet() = false, want true")
	}
	if !bytes.Equal(policy.gotLock, report.lock) {
		t.Fatalf("policy report lock = %v, want %v", policy.gotLock, report.lock)
	}
}

func TestClientEngineRetainsFreshOrderModel(t *testing.T) {
	const assetLength = 64 * 1024
	const samples = 64
	want := strings.Repeat("A", assetLength)
	for _, entryPoint := range []string{
		"StartPreTrade",
		"ExecutePreTrade",
		"ApplyDropCopy",
	} {
		t.Run(entryPoint, func(t *testing.T) {
			policy := &clientEngineGCAssetPolicy{
				t: t, checkStart: entryPoint == "StartPreTrade",
			}
			engine, err := NewClientEngineBuilder[
				*clientEngineFreshModelOrder,
				clientEngineTestReport,
				*clientEngineFreshModelAdjustment,
			]().
				FullSync().
				PreTrade(policy).
				Build()
			if err != nil {
				t.Fatalf("Build() error = %v", err)
			}
			defer engine.Stop()

			for i := 0; i < samples; i++ {
				order := &clientEngineFreshModelOrder{asset: want}
				var rejects []reject.Reject
				var err error
				switch entryPoint {
				case "StartPreTrade":
					var request *ClientRequest
					request, rejects, err = engine.StartPreTrade(order)
					if request != nil {
						request.Close()
					}
				case "ExecutePreTrade":
					var reservation *pretrade.Reservation
					reservation, rejects, err = engine.ExecutePreTrade(order)
					if reservation != nil {
						reservation.Close()
					}
				case "ApplyDropCopy":
					var operation *pretrade.DropCopyOperation
					operation, rejects, err = engine.ApplyDropCopy(order)
					if operation != nil {
						operation.Close()
					}
				}
				if err != nil {
					t.Fatalf("%s() sample %d error = %v", entryPoint, i, err)
				}
				if len(rejects) != 0 {
					t.Fatalf("%s() sample %d rejects = %v, want none", entryPoint, i, rejects)
				}
				if !policy.orderAssetSet {
					t.Fatalf("sample %d: policy order collateral asset is unset", i)
				}
				if policy.gotOrderAsset != want {
					prefix := policy.gotOrderAsset
					if len(prefix) > 64 {
						prefix = prefix[:64]
					}
					t.Fatalf(
						"sample %d: policy order collateral asset prefix = %q, length = %d, want A bytes",
						i, prefix, len(policy.gotOrderAsset),
					)
				}
			}
			if policy.orderChecks != samples {
				t.Fatalf("policy order checks = %d, want %d", policy.orderChecks, samples)
			}
		})
	}
}

func TestClientEngineRetainsFreshAccountAdjustmentModel(t *testing.T) {
	const assetLength = 64 * 1024
	const samples = 64
	want := strings.Repeat("A", assetLength)
	adjustments := make([]*clientEngineFreshModelAdjustment, samples)
	for i := range adjustments {
		adjustments[i] = &clientEngineFreshModelAdjustment{asset: want}
	}
	policy := &clientEngineGCAssetPolicy{t: t, adjustments: adjustments}
	engine, err := NewClientEngineBuilder[
		*clientEngineFreshModelOrder,
		clientEngineTestReport,
		*clientEngineFreshModelAdjustment,
	]().
		FullSync().
		PreTrade(policy).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	result, err := engine.ApplyAccountAdjustment(
		param.NewAccountIDFromUint64(1),
		adjustments,
	)
	if err != nil {
		t.Fatalf("ApplyAccountAdjustment() error = %v", err)
	}
	if result.BatchError.IsSet() {
		t.Fatalf("ApplyAccountAdjustment() rejects = %v, want none", result.BatchError)
	}
	if policy.adjustmentChecks != samples {
		t.Fatalf("policy adjustment checks = %d, want %d", policy.adjustmentChecks, samples)
	}
}

func TestClientEngineExecutePreTradeReleasesPayloadAfterCall(t *testing.T) {
	engine, err := NewClientEngineBuilder[
		clientEngineTestOrder,
		clientEngineTestReport,
		clientEngineTestAdjustment,
	]().
		FullSync().
		PreTrade(&clientEngineTestMainPolicy{}).
		Build()
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

func TestClientEnginePassesAccountAdjustmentThroughPreTradePolicy(t *testing.T) {
	policy := &clientEngineTestAdjustmentPreTradePolicy{}
	engine, err := NewClientEngineBuilder[
		clientEngineTestOrder,
		clientEngineTestReport,
		clientEngineTestAdjustment,
	]().
		FullSync().
		PreTrade(policy).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	result, err := engine.ApplyAccountAdjustment(
		param.NewAccountIDFromUint64(1),
		[]clientEngineTestAdjustment{{AccountAdjustment: model.NewAccountAdjustment(), Source: "ops-feed"}},
	)
	if err != nil {
		t.Fatalf("ApplyAccountAdjustment() error = %v", err)
	}
	if result.BatchError.IsSet() {
		t.Fatalf("ApplyAccountAdjustment() rejects = %v, want none", result.BatchError)
	}
	if policy.adjustmentCallCount != 1 {
		t.Fatalf("adjustmentCallCount = %d, want 1", policy.adjustmentCallCount)
	}
}

func TestClientEngineStartPreTradeRejectReleasesPayload(t *testing.T) {
	engine, err := NewClientEngineBuilder[
		clientEngineTestOrder,
		clientEngineTestReport,
		clientEngineTestAdjustment,
	]().
		FullSync().
		PreTrade(&clientEngineTestRejectingStartPolicy{}).
		Build()
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
	engine, err := NewClientEngineBuilder[
		clientEngineTestOrder,
		clientEngineTestReport,
		clientEngineTestAdjustment,
	]().
		FullSync().
		PreTrade(&clientEngineTestRejectingMainPolicy{}).
		Build()
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
	policy := &clientEngineTestAdjustmentPreTradePolicy{}
	engine, err := NewClientEngineBuilder[
		clientEngineTestOrder,
		clientEngineTestReport,
		clientEngineTestAdjustment,
	]().
		FullSync().
		PreTrade(policy).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	adjustments := []clientEngineTestAdjustment{
		{AccountAdjustment: model.NewAccountAdjustment(), Source: "ops-feed-1"},
		{AccountAdjustment: model.NewAccountAdjustment(), Source: "ops-feed-2"},
	}
	result, err := engine.ApplyAccountAdjustment(param.NewAccountIDFromUint64(1), adjustments)
	if err != nil {
		t.Fatalf("ApplyAccountAdjustment() error = %v", err)
	}
	if result.BatchError.IsSet() {
		t.Fatalf("ApplyAccountAdjustment() rejects = %v, want none", result.BatchError)
	}
	if policy.adjustmentCallCount != len(adjustments) {
		t.Fatalf(
			"adjustmentCallCount = %d, want %d",
			policy.adjustmentCallCount,
			len(adjustments),
		)
	}
}

func TestClientEngineUnsafeFastPayloadMismatchReturnsSystemUnavailable(t *testing.T) {
	engine, err := NewClientEngineBuilder[
		clientEngineTestOrder,
		clientEngineTestReport,
		clientEngineTestAdjustment,
	](UnsafeFastClientPayloadCallbacks()).
		FullSync().
		PreTrade(&clientEngineTestStartPolicy{}).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	request, rejects, err := engine.engine.StartPreTrade(
		orderWithMismatchedPayload(t, 42),
	)
	if err != nil {
		t.Fatalf("StartPreTrade() error = %v", err)
	}
	if request != nil {
		request.Close()
		t.Fatal("StartPreTrade() request != nil, want nil")
	}
	if len(rejects) != 1 || rejects[0].Code != reject.CodeSystemUnavailable {
		t.Fatalf("StartPreTrade() rejects = %v, want SystemUnavailable", rejects)
	}
}

func TestClientEngineDropCopyPayloadMismatchIsEvaluationFailure(t *testing.T) {
	engine, err := NewClientEngineBuilder[
		clientEngineTestOrder,
		clientEngineTestReport,
		clientEngineTestAdjustment,
	]().
		FullSync().
		PreTrade(&clientEngineTestMainPolicy{}).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	rejects := requireRejectedDropCopy(
		t,
		engine.engine,
		orderWithMismatchedPayload(t, 42),
	)
	if len(rejects) == 0 {
		t.Fatal("ApplyDropCopy() rejects is empty, want evaluation failure")
	}
	for _, item := range rejects {
		if item.Code != reject.CodeSystemUnavailable {
			t.Fatalf("reject code = %v, want SystemUnavailable", item.Code)
		}
		if !item.Code.IsEvaluationFailure() {
			t.Fatalf("reject code %v is not an evaluation failure", item.Code)
		}
	}
}

type clientEngineTestStartPolicy struct {
	order        clientEngineTestOrder
	report       clientEngineTestReport
	reportCalled bool
	killSwitch   bool
}

type clientEngineGCReportPolicy struct {
	t       *testing.T
	fillSet bool
	gotLock []byte
}

func (clientEngineGCReportPolicy) Close() {}

func (clientEngineGCReportPolicy) Name() string {
	return "client-engine-gc-report"
}

func (clientEngineGCReportPolicy) PolicyGroupID() model.PolicyGroupID {
	return model.DefaultPolicyGroupID
}

func (clientEngineGCReportPolicy) CheckPreTradeStart(
	pretrade.Context,
	clientEngineTestOrder,
) []reject.Reject {
	return nil
}

func (clientEngineGCReportPolicy) PerformPreTradeCheck(
	pretrade.Context,
	clientEngineTestOrder,
	tx.Mutations,
	pretrade.Result,
) []reject.Reject {
	return nil
}

func (p *clientEngineGCReportPolicy) ApplyExecutionReport(
	_ pretrade.PostTradeContext,
	report *clientEngineFreshModelReport,
	_ pretrade.PostTradeAdjustments,
	_ pretrade.PostTradePnls,
) []reject.AccountBlock {
	forceHandleRetentionFinalizers(p.t)
	overwriteHandleRetentionLocks(p.t)
	engineReport := model.NewExecutionReportFromHandle(report.nativeReport)
	fill, ok := engineReport.Fill().Get()
	p.fillSet = ok
	if ok {
		p.gotLock = fill.Lock()
	}
	return nil
}

func (clientEngineGCReportPolicy) ApplyAccountAdjustment(
	accountadjustment.Context,
	param.AccountID,
	model.AccountAdjustment,
	tx.Mutations,
	pretrade.AccountOutcomes,
) (pretrade.PolicyAccountAdjustmentResult, []reject.Reject) {
	return pretrade.PolicyAccountAdjustmentResult{}, nil
}

type clientEngineGCAssetPolicy struct {
	t                *testing.T
	checkStart       bool
	orderChecks      int
	orderAssetSet    bool
	gotOrderAsset    string
	adjustments      []*clientEngineFreshModelAdjustment
	adjustmentChecks int
}

func (clientEngineGCAssetPolicy) Close() {}

func (clientEngineGCAssetPolicy) Name() string {
	return "client-engine-gc-asset"
}

func (clientEngineGCAssetPolicy) PolicyGroupID() model.PolicyGroupID {
	return model.DefaultPolicyGroupID
}

func (p *clientEngineGCAssetPolicy) CheckPreTradeStart(
	_ pretrade.Context,
	order *clientEngineFreshModelOrder,
) []reject.Reject {
	if p.checkStart {
		p.checkOrderAsset(order)
	}
	return nil
}

func (p *clientEngineGCAssetPolicy) PerformPreTradeCheck(
	_ pretrade.Context,
	order *clientEngineFreshModelOrder,
	_ tx.Mutations,
	_ pretrade.Result,
) []reject.Reject {
	if !p.checkStart {
		p.checkOrderAsset(order)
	}
	return nil
}

func (p *clientEngineGCAssetPolicy) checkOrderAsset(order *clientEngineFreshModelOrder) {
	p.orderChecks++
	p.orderAssetSet = false
	p.gotOrderAsset = ""
	forceHandleRetentionFinalizers(p.t)
	replacements := newClientEngineAssetReplacementBuffers(len(order.asset))
	engineOrder := model.NewOrderFromHandle(order.nativeOrder)
	margin, marginSet := engineOrder.Margin().Get()
	if marginSet {
		asset, assetSet := margin.CollateralAsset().Get()
		p.orderAssetSet = assetSet
		if assetSet {
			p.gotOrderAsset = asset.Safe()
		}
	}
	runtime.KeepAlive(replacements)
}

func (clientEngineGCAssetPolicy) ApplyExecutionReport(
	pretrade.PostTradeContext,
	clientEngineTestReport,
	pretrade.PostTradeAdjustments,
	pretrade.PostTradePnls,
) []reject.AccountBlock {
	return nil
}

func (p *clientEngineGCAssetPolicy) ApplyAccountAdjustment(
	_ accountadjustment.Context,
	_ param.AccountID,
	_ model.AccountAdjustment,
	_ tx.Mutations,
	_ pretrade.AccountOutcomes,
) (pretrade.PolicyAccountAdjustmentResult, []reject.Reject) {
	adjustment := p.adjustments[p.adjustmentChecks]
	p.adjustmentChecks++
	forceHandleRetentionFinalizers(p.t)
	replacements := newClientEngineAssetReplacementBuffers(len(adjustment.asset))
	engineAdjustment := model.NewAccountAdjustmentFromHandle(adjustment.nativeAdjustment)
	var gotAsset string
	var assetSet bool
	operation, operationSet := engineAdjustment.BalanceOperation().Get()
	if operationSet {
		var asset param.Asset
		asset, assetSet = operation.Asset().Get()
		if assetSet {
			gotAsset = asset.Safe()
		}
	}
	runtime.KeepAlive(replacements)
	if !assetSet {
		p.t.Errorf("sample %d: policy adjustment asset is unset", p.adjustmentChecks-1)
	} else if gotAsset != adjustment.asset {
		prefix := gotAsset
		if len(prefix) > 64 {
			prefix = prefix[:64]
		}
		p.t.Errorf(
			"sample %d: policy adjustment asset prefix = %q, length = %d, want A bytes",
			p.adjustmentChecks-1, prefix, len(gotAsset),
		)
	}
	return pretrade.PolicyAccountAdjustmentResult{}, nil
}

func newClientEngineAssetReplacementBuffers(length int) []*native.String {
	overwrite := strings.Repeat("Z", length)
	replacements := make([]*native.String, 1024)
	for i := range replacements {
		replacements[i] = native.NewString(overwrite)
	}
	return replacements
}

func (clientEngineTestStartPolicy) Close() {}

func (clientEngineTestStartPolicy) Name() string {
	return "client-engine-test-start"
}

func (clientEngineTestStartPolicy) PolicyGroupID() model.PolicyGroupID {
	return model.DefaultPolicyGroupID
}

func (p *clientEngineTestStartPolicy) CheckPreTradeStart(
	_ pretrade.Context,
	order clientEngineTestOrder,
) []reject.Reject {
	p.order = order
	return nil
}

func (clientEngineTestStartPolicy) PerformPreTradeCheck(
	_ pretrade.Context,
	_ clientEngineTestOrder,
	_ tx.Mutations,
	_ pretrade.Result,
) []reject.Reject {
	return nil
}

func (p *clientEngineTestStartPolicy) ApplyExecutionReport(
	_ pretrade.PostTradeContext,
	report clientEngineTestReport,
	_ pretrade.PostTradeAdjustments,
	_ pretrade.PostTradePnls,
) []reject.AccountBlock {
	p.reportCalled = true
	p.report = report
	if p.killSwitch {
		return []reject.AccountBlock{
			reject.NewAccountBlock(
				reject.CodePnlKillSwitchTriggered,
				"test",
				"kill switch",
				"",
			),
		}
	}
	return nil
}

func (clientEngineTestStartPolicy) ApplyAccountAdjustment(
	_ accountadjustment.Context,
	_ param.AccountID,
	_ model.AccountAdjustment,
	_ tx.Mutations,
	_ pretrade.AccountOutcomes,
) (pretrade.PolicyAccountAdjustmentResult, []reject.Reject) {
	return pretrade.PolicyAccountAdjustmentResult{}, nil
}

type clientEngineTestMainPolicy struct {
	order clientEngineTestOrder
}

func (clientEngineTestMainPolicy) Close() {}

func (clientEngineTestMainPolicy) Name() string {
	return "client-engine-test-main"
}

func (clientEngineTestMainPolicy) PolicyGroupID() model.PolicyGroupID {
	return model.DefaultPolicyGroupID
}

func (clientEngineTestMainPolicy) CheckPreTradeStart(
	_ pretrade.Context,
	_ clientEngineTestOrder,
) []reject.Reject {
	return nil
}

func (p *clientEngineTestMainPolicy) PerformPreTradeCheck(
	_ pretrade.Context,
	order clientEngineTestOrder,
	_ tx.Mutations,
	_ pretrade.Result,
) []reject.Reject {
	p.order = order
	return nil
}

func (clientEngineTestMainPolicy) ApplyExecutionReport(
	pretrade.PostTradeContext,
	clientEngineTestReport,
	pretrade.PostTradeAdjustments,
	pretrade.PostTradePnls,
) []reject.AccountBlock {
	return nil
}

func (clientEngineTestMainPolicy) ApplyAccountAdjustment(
	_ accountadjustment.Context,
	_ param.AccountID,
	_ model.AccountAdjustment,
	_ tx.Mutations,
	_ pretrade.AccountOutcomes,
) (pretrade.PolicyAccountAdjustmentResult, []reject.Reject) {
	return pretrade.PolicyAccountAdjustmentResult{}, nil
}

type clientEngineTestAdjustmentPreTradePolicy struct {
	adjustmentCallCount int
}

func (clientEngineTestAdjustmentPreTradePolicy) Close() {}

func (clientEngineTestAdjustmentPreTradePolicy) Name() string {
	return "client-engine-test-adjustment"
}

func (clientEngineTestAdjustmentPreTradePolicy) PolicyGroupID() model.PolicyGroupID {
	return model.DefaultPolicyGroupID
}

func (clientEngineTestAdjustmentPreTradePolicy) CheckPreTradeStart(
	_ pretrade.Context,
	_ clientEngineTestOrder,
) []reject.Reject {
	return nil
}

func (clientEngineTestAdjustmentPreTradePolicy) PerformPreTradeCheck(
	_ pretrade.Context,
	_ clientEngineTestOrder,
	_ tx.Mutations,
	_ pretrade.Result,
) []reject.Reject {
	return nil
}

func (clientEngineTestAdjustmentPreTradePolicy) ApplyExecutionReport(
	pretrade.PostTradeContext,
	clientEngineTestReport,
	pretrade.PostTradeAdjustments,
	pretrade.PostTradePnls,
) []reject.AccountBlock {
	return nil
}

func (p *clientEngineTestAdjustmentPreTradePolicy) ApplyAccountAdjustment(
	_ accountadjustment.Context,
	_ param.AccountID,
	_ model.AccountAdjustment,
	_ tx.Mutations,
	_ pretrade.AccountOutcomes,
) (pretrade.PolicyAccountAdjustmentResult, []reject.Reject) {
	p.adjustmentCallCount++
	return pretrade.PolicyAccountAdjustmentResult{}, nil
}

type clientEngineTestRejectingStartPolicy struct{}

func (clientEngineTestRejectingStartPolicy) Close() {}

func (clientEngineTestRejectingStartPolicy) Name() string {
	return "client-engine-test-reject-start"
}

func (clientEngineTestRejectingStartPolicy) PolicyGroupID() model.PolicyGroupID {
	return model.DefaultPolicyGroupID
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

func (clientEngineTestRejectingStartPolicy) PerformPreTradeCheck(
	_ pretrade.Context,
	_ clientEngineTestOrder,
	_ tx.Mutations,
	_ pretrade.Result,
) []reject.Reject {
	return nil
}

func (clientEngineTestRejectingStartPolicy) ApplyExecutionReport(
	pretrade.PostTradeContext,
	clientEngineTestReport,
	pretrade.PostTradeAdjustments,
	pretrade.PostTradePnls,
) []reject.AccountBlock {
	return nil
}

func (clientEngineTestRejectingStartPolicy) ApplyAccountAdjustment(
	_ accountadjustment.Context,
	_ param.AccountID,
	_ model.AccountAdjustment,
	_ tx.Mutations,
	_ pretrade.AccountOutcomes,
) (pretrade.PolicyAccountAdjustmentResult, []reject.Reject) {
	return pretrade.PolicyAccountAdjustmentResult{}, nil
}

type clientEngineTestRejectingMainPolicy struct{}

func (clientEngineTestRejectingMainPolicy) Close() {}

func (clientEngineTestRejectingMainPolicy) Name() string {
	return "client-engine-test-reject-main"
}

func (clientEngineTestRejectingMainPolicy) PolicyGroupID() model.PolicyGroupID {
	return model.DefaultPolicyGroupID
}

func (clientEngineTestRejectingMainPolicy) CheckPreTradeStart(
	_ pretrade.Context,
	_ clientEngineTestOrder,
) []reject.Reject {
	return nil
}

func (p *clientEngineTestRejectingMainPolicy) PerformPreTradeCheck(
	_ pretrade.Context,
	_ clientEngineTestOrder,
	_ tx.Mutations,
	_ pretrade.Result,
) []reject.Reject {
	return reject.NewSingleItemList(
		reject.CodeOther,
		p.Name(),
		"order rejected",
		"reject execute stage",
		reject.ScopeOrder,
	)
}

func (clientEngineTestRejectingMainPolicy) ApplyExecutionReport(
	pretrade.PostTradeContext,
	clientEngineTestReport,
	pretrade.PostTradeAdjustments,
	pretrade.PostTradePnls,
) []reject.AccountBlock {
	return nil
}

func (clientEngineTestRejectingMainPolicy) ApplyAccountAdjustment(
	_ accountadjustment.Context,
	_ param.AccountID,
	_ model.AccountAdjustment,
	_ tx.Mutations,
	_ pretrade.AccountOutcomes,
) (pretrade.PolicyAccountAdjustmentResult, []reject.Reject) {
	return pretrade.PolicyAccountAdjustmentResult{}, nil
}

func orderWithMismatchedPayload(t *testing.T, payload any) model.Order {
	t.Helper()

	order := model.NewOrder()
	operation := order.EnsureOperationView()
	operation.SetAccountID(param.NewAccountIDFromUint64(1))
	nativeOrder := order.Handle()
	handle := cgo.NewHandle(payload)
	t.Cleanup(handle.Delete)
	native.OrderSetUserData(&nativeOrder, callback.NewUserDataFromHandle(handle))
	return model.NewOrderFromHandle(nativeOrder)
}

func TestNewClientPreTradeEngineBuilder(*testing.T) {
	_ = NewClientPreTradeEngineBuilder[clientEngineTestOrder, clientEngineTestReport]()
}

func TestNewClientAccountAdjustmentEngineBuilder(*testing.T) {
	_ = NewClientAccountAdjustmentEngineBuilder[clientEngineTestAdjustment]()
}

func TestClientEngineBuilderCloseIsIdempotent(t *testing.T) {
	builder := NewClientEngineBuilder[
		clientEngineTestOrder,
		clientEngineTestReport,
		clientEngineTestAdjustment,
	]().
		FullSync().
		PreTrade(&clientEngineTestStartPolicy{})

	builder.Close()
	builder.Close()

	builtBuilder := NewClientEngineBuilder[
		clientEngineTestOrder,
		clientEngineTestReport,
		clientEngineTestAdjustment,
	]().
		FullSync().
		PreTrade(&clientEngineTestStartPolicy{})
	engine, err := builtBuilder.Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	builtBuilder.Close()
	builtBuilder.Close()
}

func TestClientEngineBuilderBuildReturnsErrorAfterClose(t *testing.T) {
	builder := NewClientEngineBuilder[
		clientEngineTestOrder,
		clientEngineTestReport,
		clientEngineTestAdjustment,
	]().
		FullSync().
		PreTrade(&clientEngineTestStartPolicy{})

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
	policy := &clientEngineTestMainPolicy{}
	engine, err := NewClientEngineBuilder[
		clientEngineTestOrder,
		clientEngineTestReport,
		clientEngineTestAdjustment,
	](UnsafeFastClientPayloadCallbacks()).
		FullSync().
		PreTrade(policy).
		Build()
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

func TestClientEngineUnsafeFastPreTradePolicyAppliesAccountAdjustment(t *testing.T) {
	policy := &clientEngineTestAdjustmentPreTradePolicy{}
	engine, err := NewClientEngineBuilder[
		clientEngineTestOrder,
		clientEngineTestReport,
		clientEngineTestAdjustment,
	](UnsafeFastClientPayloadCallbacks()).
		FullSync().
		PreTrade(policy).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	result, err := engine.ApplyAccountAdjustment(
		param.NewAccountIDFromUint64(1),
		[]clientEngineTestAdjustment{{AccountAdjustment: model.NewAccountAdjustment(), Source: "unsafe-fast-adjustment"}},
	)
	if err != nil {
		t.Fatalf("ApplyAccountAdjustment() error = %v", err)
	}
	if result.BatchError.IsSet() {
		t.Fatalf("ApplyAccountAdjustment() rejects = %v, want none", result.BatchError)
	}
	if policy.adjustmentCallCount != 1 {
		t.Fatalf("adjustmentCallCount = %d, want 1", policy.adjustmentCallCount)
	}
}

func TestClientPayloadHandleReleaseNilIsNoop(*testing.T) {
	var payload *clientPayloadHandle
	payload.release()
}
