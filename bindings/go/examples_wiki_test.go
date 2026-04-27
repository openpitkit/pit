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

package pit

import (
	"fmt"
	"testing"

	"github.com/openpitkit/pit-go/model"
	"github.com/openpitkit/pit-go/param"
	"github.com/openpitkit/pit-go/pretrade"
	"github.com/openpitkit/pit-go/pretrade/policies"
	"github.com/openpitkit/pit-go/reject"
	"github.com/openpitkit/pit-go/tx"
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

func (p *wikiStrategyTagPolicy) Close() {}

func (p *wikiStrategyTagPolicy) Name() string { return "StrategyTagPolicy" }

func (p *wikiStrategyTagPolicy) CheckPreTradeStart(
	_ pretrade.Context,
	order wikiStrategyOrder,
) reject.List {
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

func (p *wikiStrategyTagPolicy) ApplyExecutionReport(wikiStrategyReport) bool {
	return false
}

// --- Shared helpers ---

func wikiExampleOrder(t *testing.T, quantity, price string) model.Order {
	t.Helper()

	order := model.NewOrder()
	op := order.EnsureOperationView()
	op.SetInstrument(param.NewInstrument(param.NewAsset("AAPL"), param.NewAsset("USD")))
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
	op.SetInstrument(param.NewInstrument(param.NewAsset("AAPL"), param.NewAsset("USD")))
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

func wikiExampleEngine(t *testing.T, startPolicies ...pretrade.CheckPreTradeStartPolicy) *Engine {
	t.Helper()

	builder, err := NewEngineBuilder()
	if err != nil {
		t.Fatalf("NewEngineBuilder() error = %v", err)
	}
	builder.CheckPreTradeStartPolicy(startPolicies...)
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

func (p *wikiNotionalCapPolicy) Close() {}

func (p *wikiNotionalCapPolicy) Name() string { return "NotionalCapPolicy" }

func (p *wikiNotionalCapPolicy) PerformPreTradeCheck(
	_ pretrade.Context,
	order model.Order,
	_ tx.Mutations,
) reject.List {
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

func (p *wikiNotionalCapPolicy) ApplyExecutionReport(model.ExecutionReport) bool {
	return false
}

// --- Policy-API: Rollback Safety Pattern ---

type wikiReserveThenValidatePolicy struct {
	reserved param.Volume
	limit    param.Volume
}

func (p *wikiReserveThenValidatePolicy) Close() {}

func (p *wikiReserveThenValidatePolicy) Name() string { return "ReserveThenValidatePolicy" }

func (p *wikiReserveThenValidatePolicy) PerformPreTradeCheck(
	_ pretrade.Context,
	_ model.Order,
	mutations tx.Mutations,
) reject.List {
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

func (p *wikiReserveThenValidatePolicy) ApplyExecutionReport(model.ExecutionReport) bool {
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
	usd := param.NewAsset("USD")

	barrier, err := param.NewPnlFromString("1000")
	if err != nil {
		t.Fatalf("NewPnlFromString() error = %v", err)
	}
	maxQty, err := param.NewQuantityFromString("500")
	if err != nil {
		t.Fatalf("NewQuantityFromString() error = %v", err)
	}
	maxNotional, err := param.NewVolumeFromString("100000")
	if err != nil {
		t.Fatalf("NewVolumeFromString() error = %v", err)
	}

	pnlPolicy, err := policies.NewPnlKillSwitchPolicy(policies.PnlKillSwitchBarrier{
		SettlementAsset: usd,
		Barrier:         barrier,
	})
	if err != nil {
		t.Fatalf("NewPnlKillSwitchPolicy() error = %v", err)
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
	builder.CheckPreTradeStartPolicy(
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
	op.SetInstrument(param.NewInstrument(param.NewAsset("AAPL"), usd))
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
	reportOp.SetInstrument(param.NewInstrument(param.NewAsset("AAPL"), usd))
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
