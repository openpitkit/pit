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
	"fmt"
	"testing"

	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/optional"
	"go.openpit.dev/openpit/pretrade"
	"go.openpit.dev/openpit/pretrade/policies"
	"go.openpit.dev/openpit/reject"
	"go.openpit.dev/openpit/tx"
)

// --- Policy-API: Custom Order and Execution Report Models ---

type wikiStrategyOrder struct {
	model.Order
	StrategyTag string
}

type wikiStrategyReport struct {
	model.ExecutionReport
	VenueExecID string
}

type wikiStrategyTagPolicy struct{}

func (wikiStrategyTagPolicy) Close() {}

func (wikiStrategyTagPolicy) Name() string { return "StrategyTagPolicy" }

func (p *wikiStrategyTagPolicy) CheckPreTradeStart(
	_ pretrade.Context,
	order wikiStrategyOrder,
) []reject.Reject {
	if order.StrategyTag == "blocked" {
		return reject.NewSingleItemList(
			reject.CodeComplianceRestriction,
			p.Name(),
			"strategy blocked",
			fmt.Sprintf("strategy tag %q is not allowed", order.StrategyTag),
			reject.ScopeOrder,
		)
	}
	return nil
}

func (wikiStrategyTagPolicy) ApplyExecutionReport(wikiStrategyReport) bool {
	return false
}

// --- Shared helpers ---

// Used in: pit.wiki/Domain-Types.md — Create Validated Values.
// Keep this example synced with the wiki snippet when constructor behavior changes.
func TestExampleWikiDomainTypesCreateValidatedValues(t *testing.T) {
	asset, err := param.NewAsset("AAPL")
	if err != nil {
		t.Fatalf("NewAsset(AAPL) error = %v", err)
	}
	quantity, err := param.NewQuantityFromString("10.5")
	if err != nil {
		t.Fatalf("NewQuantityFromString(10.5) error = %v", err)
	}
	price, err := param.NewPriceFromString("185")
	if err != nil {
		t.Fatalf("NewPriceFromString(185) error = %v", err)
	}
	pnl, err := param.NewPnlFromString("-12.5")
	if err != nil {
		t.Fatalf("NewPnlFromString(-12.5) error = %v", err)
	}

	if got := asset.String(); got != "AAPL" {
		t.Fatalf("asset.String() = %q, want %q", got, "AAPL")
	}
	if got := quantity.String(); got != "10.5" {
		t.Fatalf("quantity.String() = %q, want %q", got, "10.5")
	}
	if got := price.String(); got != "185" {
		t.Fatalf("price.String() = %q, want %q", got, "185")
	}
	if got := pnl.String(); got != "-12.5" {
		t.Fatalf("pnl.String() = %q, want %q", got, "-12.5")
	}
}

// Used in: pit.wiki/Domain-Types.md — Create Validated Values.
// Keep this example synced with wiki snippets when constructor behavior changes.
func TestExampleWikiDomainTypesAssetValidationError(t *testing.T) {
	_, err := param.NewAsset("  ")
	if err == nil {
		t.Fatal("NewAsset(empty) error = nil, want ErrAssetEmpty")
	}
	if err != param.ErrAssetEmpty {
		t.Fatalf("NewAsset(empty) error = %v, want %v", err, param.ErrAssetEmpty)
	}
}

// Used in: pit.wiki/Domain-Types.md — Account Identifiers.
// Keep this example synced with wiki snippets when constructor behavior changes.
func TestExampleWikiDomainTypesAccountIDValidationError(t *testing.T) {
	_, err := param.NewAccountIDFromString("  ")
	if err == nil {
		t.Fatal("NewAccountIDFromString(empty) error = nil, want ErrAccountIDEmpty")
	}
	if err != param.ErrAccountIDEmpty {
		t.Fatalf("NewAccountIDFromString(empty) error = %v, want %v", err, param.ErrAccountIDEmpty)
	}
}

func wikiExampleOrder(t *testing.T, quantity, price string) model.Order {
	t.Helper()

	order := model.NewOrder()
	op := order.EnsureOperationView()
	aapl, err := param.NewAsset("AAPL")
	if err != nil {
		t.Fatalf("NewAsset(AAPL) error = %v", err)
	}
	usd, err := param.NewAsset("USD")
	if err != nil {
		t.Fatalf("NewAsset(USD) error = %v", err)
	}
	op.SetInstrument(param.NewInstrument(aapl, usd))
	op.SetAccountID(param.NewAccountIDFromInt(99224416))
	op.SetSide(param.SideBuy)

	qty, err := param.NewQuantityFromString(quantity)
	if err != nil {
		t.Fatalf("NewQuantityFromString(%q) error = %v", quantity, err)
	}
	p, err := param.NewPriceFromString(price)
	if err != nil {
		t.Fatalf("NewPriceFromString(%q) error = %v", price, err)
	}
	op.SetTradeAmount(param.NewQuantityTradeAmount(qty))
	op.SetPrice(p)
	return order
}

func wikiExampleReport(t *testing.T, pnlStr, feeStr string) model.ExecutionReport {
	t.Helper()

	report := model.NewExecutionReport()
	op := model.NewExecutionReportOperation()
	aapl, err := param.NewAsset("AAPL")
	if err != nil {
		t.Fatalf("NewAsset(AAPL) error = %v", err)
	}
	usd, err := param.NewAsset("USD")
	if err != nil {
		t.Fatalf("NewAsset(USD) error = %v", err)
	}
	op.SetInstrument(param.NewInstrument(aapl, usd))
	op.SetAccountID(param.NewAccountIDFromInt(99224416))
	op.SetSide(param.SideBuy)
	report.SetOperation(op)

	pnl, err := param.NewPnlFromString(pnlStr)
	if err != nil {
		t.Fatalf("NewPnlFromString(%q) error = %v", pnlStr, err)
	}
	fee, err := param.NewFeeFromString(feeStr)
	if err != nil {
		t.Fatalf("NewFeeFromString(%q) error = %v", feeStr, err)
	}
	impact := model.NewExecutionReportFinancialImpact()
	impact.SetPnl(pnl)
	impact.SetFee(fee)
	report.SetFinancialImpact(impact)
	return report
}

func wikiExampleEngine(t *testing.T, startPolicies ...pretrade.BuiltinPolicy) *Engine {
	t.Helper()

	builder, err := NewEngineBuilder()
	if err != nil {
		t.Fatalf("NewEngineBuilder() error = %v", err)
	}
	builder.BuiltinCheckPreTradeStartPolicy(startPolicies...)
	engine, err := builder.Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	t.Cleanup(engine.Stop)
	return engine
}

// --- Policy-API: Custom Main-Stage Policy ---

type wikiNotionalCapPolicy struct {
	MaxAbsNotional param.Volume
}

func (wikiNotionalCapPolicy) Close() {}

func (wikiNotionalCapPolicy) Name() string { return "NotionalCapPolicy" }

func (p *wikiNotionalCapPolicy) PerformPreTradeCheck(
	_ pretrade.Context,
	order model.Order,
	_ tx.Mutations,
) []reject.Reject {
	operation, ok := order.Operation().Get()
	if !ok {
		return reject.NewSingleItemList(
			reject.CodeMissingRequiredField,
			p.Name(),
			"required order field missing",
			"operation is not set",
			reject.ScopeOrder,
		)
	}

	tradeAmount, ok := operation.TradeAmount().Get()
	if !ok {
		return reject.NewSingleItemList(
			reject.CodeMissingRequiredField,
			p.Name(),
			"required order field missing",
			"trade_amount is not set",
			reject.ScopeOrder,
		)
	}

	var requestedNotional param.Volume
	if tradeAmount.IsVolume() {
		requestedNotional = tradeAmount.MustVolume()
	} else {
		price, ok := operation.Price().Get()
		if !ok {
			return reject.NewSingleItemList(
				reject.CodeOrderValueCalculationFailed,
				p.Name(),
				"order value calculation failed",
				"price not provided for evaluating notional",
				reject.ScopeOrder,
			)
		}
		notional, err := price.CalculateVolume(tradeAmount.MustQuantity())
		if err != nil {
			return reject.NewSingleItemList(
				reject.CodeOrderValueCalculationFailed,
				p.Name(),
				"order value calculation failed",
				"price and quantity could not be used to evaluate notional",
				reject.ScopeOrder,
			)
		}
		requestedNotional = notional
	}

	if requestedNotional.Compare(p.MaxAbsNotional) > 0 {
		return reject.NewSingleItemList(
			reject.CodeRiskLimitExceeded,
			p.Name(),
			"strategy cap exceeded",
			fmt.Sprintf(
				"requested notional %v, max allowed: %v",
				requestedNotional, p.MaxAbsNotional,
			),
			reject.ScopeOrder,
		)
	}

	return nil
}

func (wikiNotionalCapPolicy) ApplyExecutionReport(model.ExecutionReport) bool {
	return false
}

// --- Policy-API: Rollback Safety Pattern ---

type wikiReserveThenValidatePolicy struct {
	reserved param.Volume
	limit    param.Volume
}

func (wikiReserveThenValidatePolicy) Close() {}

func (wikiReserveThenValidatePolicy) Name() string { return "ReserveThenValidatePolicy" }

func (p *wikiReserveThenValidatePolicy) PerformPreTradeCheck(
	_ pretrade.Context,
	_ model.Order,
	mutations tx.Mutations,
) []reject.Reject {
	prevReserved := p.reserved
	nextReserved, _ := param.NewVolumeFromString("100")
	p.reserved = nextReserved

	_ = mutations.Push(
		func() {
			// Commit is empty: state was applied eagerly.
		},
		func() {
			p.reserved = prevReserved
		},
	)

	if p.reserved.Compare(p.limit) > 0 {
		return reject.NewSingleItemList(
			reject.CodeRiskLimitExceeded,
			p.Name(),
			"temporary reservation exceeds limit",
			fmt.Sprintf("reserved %v, limit: %v", nextReserved, p.limit),
			reject.ScopeOrder,
		)
	}

	return nil
}

func (wikiReserveThenValidatePolicy) ApplyExecutionReport(model.ExecutionReport) bool {
	return false
}

// --- Tests ---

// Used in: pit.wiki/Pre-trade-Pipeline.md — Handle a Start-Stage Reject
func TestExampleWikiPipelineStartStageReject(t *testing.T) {
	engine := wikiExampleEngine(t, policies.NewOrderValidation())
	order := wikiExampleOrder(t, "100", "185")

	request, rejects, err := engine.StartPreTrade(order)
	if err != nil {
		t.Fatalf("StartPreTrade() error = %v", err)
	}
	if rejects != nil {
		for _, r := range rejects {
			t.Logf(
				"rejected by %s [%d]: %s (%s)",
				r.Policy, r.Code, r.Reason, r.Details,
			)
		}
	} else {
		defer request.Close()
	}
}

// Used in: pit.wiki/Pre-trade-Pipeline.md — Execute the Main Stage and
// Finalize the Reservation
func TestExampleWikiPipelineMainStageFinalize(t *testing.T) {
	engine := wikiExampleEngine(t, policies.NewOrderValidation())
	order := wikiExampleOrder(t, "100", "185")

	request, rejects, err := engine.StartPreTrade(order)
	if err != nil {
		t.Fatalf("StartPreTrade() error = %v", err)
	}
	if rejects != nil {
		t.Fatalf("StartPreTrade() unexpected rejects: %v", rejects)
	}
	defer request.Close()

	reservation, rejects, err := request.Execute()
	if err != nil {
		t.Fatalf("Execute() error = %v", err)
	}
	if rejects != nil {
		for _, r := range rejects {
			t.Logf(
				"rejected by %s [%d]: %s (%s)",
				r.Policy, r.Code, r.Reason, r.Details,
			)
		}
		return
	}
	defer reservation.Close()
	reservation.Commit()
}

// Used in: pit.wiki/Pre-trade-Pipeline.md — Shortcut for Start + Main Stages
// Used in: pit.wiki/Getting-Started.md — Shortcut for Start + Main Stages
func TestExampleWikiPipelineShortcutStartAndMain(t *testing.T) {
	engine := wikiExampleEngine(t, policies.NewOrderValidation())
	order := wikiExampleOrder(t, "100", "185")

	reservation, rejects, err := engine.ExecutePreTrade(order)
	if err != nil {
		t.Fatalf("ExecutePreTrade() error = %v", err)
	}
	if rejects != nil {
		for _, r := range rejects {
			t.Logf(
				"rejected by %s [%d]: %s (%s)",
				r.Policy, r.Code, r.Reason, r.Details,
			)
		}
		return
	}
	defer reservation.Close()
	reservation.Commit()
}

// Used in: pit.wiki/Pre-trade-Pipeline.md — Apply Post-Trade Feedback
func TestExampleWikiPipelineApplyPostTrade(t *testing.T) {
	engine := wikiExampleEngine(t, policies.NewOrderValidation())
	report := wikiExampleReport(t, "-50", "3.4")

	result, err := engine.ApplyExecutionReport(report)
	if err != nil {
		t.Fatalf("ApplyExecutionReport() error = %v", err)
	}
	if result.KillSwitchTriggered {
		t.Fatal("KillSwitchTriggered = true, want false")
	}
}

// Used in: pit.wiki/Policy-API.md — Example: Custom Main-Stage Policy
func TestExampleWikiPolicyNotionalCap(t *testing.T) {
	maxNotional, err := param.NewVolumeFromString("1000")
	if err != nil {
		t.Fatalf("NewVolumeFromString() error = %v", err)
	}

	policy := &wikiNotionalCapPolicy{MaxAbsNotional: maxNotional}
	builder, err := NewEngineBuilder()
	if err != nil {
		t.Fatalf("NewEngineBuilder() error = %v", err)
	}
	builder.PreTradePolicy(policy)
	engine, err := builder.Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	// Order below limit: price=25, qty=10 → notional=250 < 1000.
	order := wikiExampleOrder(t, "10", "25")
	startResult, rejects, err := engine.StartPreTrade(order)
	if err != nil {
		t.Fatalf("StartPreTrade() error = %v", err)
	}
	if rejects != nil {
		t.Fatalf("StartPreTrade() unexpected rejects: %v", rejects)
	}
	defer startResult.Close()

	reservation, rejects, err := startResult.Execute()
	if err != nil {
		t.Fatalf("Execute() error = %v", err)
	}
	if rejects != nil {
		t.Fatalf("Execute() unexpected rejects: %v", rejects)
	}
	reservation.CommitAndClose()

	// Order above limit: price=25, qty=100 → notional=2500 > 1000.
	bigOrder := wikiExampleOrder(t, "100", "25")
	bigStart, bigRejects, err := engine.StartPreTrade(bigOrder)
	if err != nil {
		t.Fatalf("StartPreTrade(big) error = %v", err)
	}
	if bigRejects != nil {
		t.Fatalf("StartPreTrade(big) unexpected rejects: %v", bigRejects)
	}
	defer bigStart.Close()

	_, executeRejects, err := bigStart.Execute()
	if err != nil {
		t.Fatalf("Execute(big) error = %v", err)
	}
	if executeRejects == nil {
		t.Fatal("Execute(big) rejects = nil, want non-nil")
	}
	if executeRejects[0].Code != reject.CodeRiskLimitExceeded {
		t.Fatalf(
			"reject code = %v, want %v",
			executeRejects[0].Code, reject.CodeRiskLimitExceeded,
		)
	}
}

// Used in: pit.wiki/Policy-API.md — Example: Rollback Safety Pattern
func TestExampleWikiPolicyRollbackSafety(t *testing.T) {
	limit, err := param.NewVolumeFromString("50")
	if err != nil {
		t.Fatalf("NewVolumeFromString() error = %v", err)
	}

	policy := &wikiReserveThenValidatePolicy{
		reserved: param.VolumeZero,
		limit:    limit,
	}
	builder, err := NewEngineBuilder()
	if err != nil {
		t.Fatalf("NewEngineBuilder() error = %v", err)
	}
	builder.PreTradePolicy(policy)
	engine, err := builder.Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	order := wikiExampleOrder(t, "10", "25")
	startResult, rejects, err := engine.StartPreTrade(order)
	if err != nil {
		t.Fatalf("StartPreTrade() error = %v", err)
	}
	if rejects != nil {
		t.Fatalf("StartPreTrade() unexpected rejects: %v", rejects)
	}
	defer startResult.Close()

	_, executeRejects, err := startResult.Execute()
	if err != nil {
		t.Fatalf("Execute() error = %v", err)
	}
	if executeRejects == nil {
		t.Fatal("Execute() rejects = nil, want non-nil (reservation > limit)")
	}
	if executeRejects[0].Code != reject.CodeRiskLimitExceeded {
		t.Fatalf(
			"reject code = %v, want %v",
			executeRejects[0].Code, reject.CodeRiskLimitExceeded,
		)
	}

	// The rollback mutation must have restored reserved to zero.
	if !policy.reserved.Equal(param.VolumeZero) {
		t.Fatalf("reserved after rollback = %v, want zero", policy.reserved)
	}
}

// Used in: pit.wiki/Getting-Started.md — Build an Engine
func TestExampleWikiGettingStartedBuildEngine(t *testing.T) {
	usd, err := param.NewAsset("USD")
	if err != nil {
		t.Fatalf("NewAsset(USD) error = %v", err)
	}

	lowerBound, err := param.NewPnlFromString("-1000")
	if err != nil {
		t.Fatalf("NewPnlFromString(-1000) error = %v", err)
	}
	maxQty, err := param.NewQuantityFromString("500")
	if err != nil {
		t.Fatalf("NewQuantityFromString() error = %v", err)
	}
	maxNotional, err := param.NewVolumeFromString("100000")
	if err != nil {
		t.Fatalf("NewVolumeFromString() error = %v", err)
	}

	pnlPolicy, err := policies.NewPnlBoundsKillSwitchPolicy(policies.PnlBoundsBarrier{
		SettlementAsset: usd,
		LowerBound:      optional.Some(lowerBound),
		InitialPnl:      param.PnlZero,
	})
	if err != nil {
		t.Fatalf("NewPnlBoundsKillSwitchPolicy() error = %v", err)
	}
	defer pnlPolicy.Close()

	sizePolicy, err := policies.NewOrderSizeLimitPolicy(policies.OrderSizeLimit{
		SettlementAsset: usd,
		MaxQuantity:     maxQty,
		MaxNotional:     maxNotional,
	})
	if err != nil {
		t.Fatalf("NewOrderSizeLimitPolicy() error = %v", err)
	}
	defer sizePolicy.Close()

	builder, err := NewEngineBuilder()
	if err != nil {
		t.Fatalf("NewEngineBuilder() error = %v", err)
	}
	builder.BuiltinCheckPreTradeStartPolicy(
		policies.NewOrderValidation(),
		pnlPolicy,
		policies.NewRateLimitPolicy(100, 1),
		sizePolicy,
	)
	engine, err := builder.Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	order := model.NewOrder()
	op := order.EnsureOperationView()
	aapl, err := param.NewAsset("AAPL")
	if err != nil {
		t.Fatalf("NewAsset(AAPL) error = %v", err)
	}
	op.SetInstrument(param.NewInstrument(aapl, usd))
	op.SetAccountID(param.NewAccountIDFromInt(99224416))
	op.SetSide(param.SideBuy)
	price, _ := param.NewPriceFromString("185")
	qty, _ := param.NewQuantityFromString("100")
	op.SetTradeAmount(param.NewQuantityTradeAmount(qty))
	op.SetPrice(price)

	request, rejects, err := engine.StartPreTrade(order)
	if err != nil {
		t.Fatalf("StartPreTrade() error = %v", err)
	}
	if rejects != nil {
		t.Fatalf("StartPreTrade() unexpected rejects: %v", rejects)
	}
	defer request.Close()

	reservation, rejects, err := request.Execute()
	if err != nil {
		t.Fatalf("Execute() error = %v", err)
	}
	if rejects != nil {
		t.Fatalf("Execute() unexpected rejects: %v", rejects)
	}
	defer reservation.Close()

	reservation.Commit()

	report := model.NewExecutionReport()
	reportOp := model.NewExecutionReportOperation()
	reportOp.SetInstrument(param.NewInstrument(aapl, usd))
	reportOp.SetAccountID(param.NewAccountIDFromInt(99224416))
	reportOp.SetSide(param.SideBuy)
	report.SetOperation(reportOp)

	pnl, _ := param.NewPnlFromString("-50")
	fee, _ := param.NewFeeFromString("3.4")
	impact := model.NewExecutionReportFinancialImpact()
	impact.SetPnl(pnl)
	impact.SetFee(fee)
	report.SetFinancialImpact(impact)

	result, err := engine.ApplyExecutionReport(report)
	if err != nil {
		t.Fatalf("ApplyExecutionReport() error = %v", err)
	}
	if result.KillSwitchTriggered {
		t.Fatal("KillSwitchTriggered = true, want false")
	}
}

// Used in: pit.wiki/Getting-Started.md — Shortcut for Start + Main Stages
func TestExampleWikiGettingStartedShortcut(t *testing.T) {
	engine := wikiExampleEngine(t, policies.NewOrderValidation())
	order := wikiExampleOrder(t, "100", "185")

	reservation, rejects, err := engine.ExecutePreTrade(order)
	if err != nil {
		t.Fatalf("ExecutePreTrade() error = %v", err)
	}
	if rejects != nil {
		for _, r := range rejects {
			t.Logf(
				"rejected by %s [%d]: %s (%s)",
				r.Policy, r.Code, r.Reason, r.Details,
			)
		}
		return
	}
	defer reservation.Close()
	reservation.Commit()
}

// Used in: pit.wiki/Getting-Started.md — Run an Order Through the Engine
func TestExampleWikiGettingStartedRunOrder(t *testing.T) {
	engine := wikiExampleEngine(t, policies.NewOrderValidation())
	order := wikiExampleOrder(t, "100", "185")

	request, rejects, err := engine.StartPreTrade(order)
	if err != nil {
		t.Fatalf("StartPreTrade() error = %v", err)
	}
	if rejects != nil {
		for _, r := range rejects {
			t.Logf(
				"rejected by %s [%d]: %s (%s)",
				r.Policy, r.Code, r.Reason, r.Details,
			)
		}
		return
	}
	defer request.Close()

	reservation, rejects, err := request.Execute()
	if err != nil {
		t.Fatalf("Execute() error = %v", err)
	}
	if rejects != nil {
		for _, r := range rejects {
			t.Logf(
				"rejected by %s [%d]: %s (%s)",
				r.Policy, r.Code, r.Reason, r.Details,
			)
		}
		return
	}
	defer reservation.Close()
	reservation.Commit()
}

// Used in: pit.wiki/Getting-Started.md — Apply Post-Trade Feedback
func TestExampleWikiGettingStartedApplyPostTrade(t *testing.T) {
	engine := wikiExampleEngine(t, policies.NewOrderValidation())
	report := wikiExampleReport(t, "-50", "3.4")

	result, err := engine.ApplyExecutionReport(report)
	if err != nil {
		t.Fatalf("ApplyExecutionReport() error = %v", err)
	}
	if result.KillSwitchTriggered {
		t.Fatal("KillSwitchTriggered = true, want false")
	}
}

// Used in: pit.wiki/Policy-API.md — Example: Go Custom Models
func TestExampleWikiCustomGoModels(t *testing.T) {
	builder, err := NewClientPreTradeEngineBuilder[wikiStrategyOrder, wikiStrategyReport]()
	if err != nil {
		t.Fatalf("NewClientPreTradeEngineBuilder() error = %v", err)
	}
	builder.CheckPreTradeStartPolicy(&wikiStrategyTagPolicy{})
	engine, err := builder.Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	// Allowed order must pass.
	allowed := wikiStrategyOrder{Order: model.NewOrder(), StrategyTag: "alpha"}
	request, rejects, err := engine.StartPreTrade(allowed)
	if err != nil {
		t.Fatalf("StartPreTrade(allowed) error = %v", err)
	}
	if rejects != nil {
		t.Fatalf("StartPreTrade(allowed) unexpected rejects: %v", rejects)
	}
	reservation, rejects, err := request.Execute()
	if err != nil {
		t.Fatalf("Execute(allowed) error = %v", err)
	}
	request.Close()
	if rejects != nil {
		t.Fatalf("Execute(allowed) unexpected rejects: %v", rejects)
	}
	reservation.CommitAndClose()

	// Blocked order must be rejected by the start stage.
	blocked := wikiStrategyOrder{Order: model.NewOrder(), StrategyTag: "blocked"}
	blockedRequest, blockedRejects, err := engine.StartPreTrade(blocked)
	if err != nil {
		t.Fatalf("StartPreTrade(blocked) error = %v", err)
	}
	if blockedRequest != nil {
		blockedRequest.Close()
	}
	if blockedRejects == nil {
		t.Fatal("StartPreTrade(blocked) rejects = nil, want non-nil")
	}
	if blockedRejects[0].Code != reject.CodeComplianceRestriction {
		t.Fatalf(
			"reject code = %v, want %v",
			blockedRejects[0].Code, reject.CodeComplianceRestriction,
		)
	}
}

// Used in: pit.wiki/Custom-Go-Types.md — Example
// Keep this example synced with the wiki snippet when the ClientEngine API changes.
func TestExampleWikiCustomGoTypes(t *testing.T) {
	builder, err := NewClientPreTradeEngineBuilder[wikiStrategyOrder, wikiStrategyReport]()
	if err != nil {
		t.Fatalf("NewClientPreTradeEngineBuilder() error = %v", err)
	}
	builder.CheckPreTradeStartPolicy(&wikiStrategyTagPolicy{})
	engine, err := builder.Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	// Allowed order must pass both stages.
	allowed := wikiStrategyOrder{Order: model.NewOrder(), StrategyTag: "alpha"}
	request, rejects, err := engine.StartPreTrade(allowed)
	if err != nil {
		t.Fatalf("StartPreTrade(allowed) error = %v", err)
	}
	if rejects != nil {
		t.Fatalf("StartPreTrade(allowed) unexpected rejects: %v", rejects)
	}
	defer request.Close()

	reservation, rejects, err := request.Execute()
	if err != nil {
		t.Fatalf("Execute(allowed) error = %v", err)
	}
	if rejects != nil {
		t.Fatalf("Execute(allowed) unexpected rejects: %v", rejects)
	}
	reservation.CommitAndClose()

	// Blocked order must be rejected by the start stage.
	blocked := wikiStrategyOrder{Order: model.NewOrder(), StrategyTag: "blocked"}
	blockedRequest, blockedRejects, err := engine.StartPreTrade(blocked)
	if err != nil {
		t.Fatalf("StartPreTrade(blocked) error = %v", err)
	}
	if blockedRequest != nil {
		blockedRequest.Close()
	}
	if blockedRejects == nil {
		t.Fatal("StartPreTrade(blocked) rejects = nil, want non-nil")
	}
	if blockedRejects[0].Code != reject.CodeComplianceRestriction {
		t.Fatalf(
			"reject code = %v, want %v",
			blockedRejects[0].Code, reject.CodeComplianceRestriction,
		)
	}
}
