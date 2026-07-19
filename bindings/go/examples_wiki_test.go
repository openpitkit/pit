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
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"log"
	"sync"
	"testing"
	"time"

	"go.openpit.dev/openpit/accountadjustment"
	"go.openpit.dev/openpit/marketdata"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/optional"
	"go.openpit.dev/openpit/pretrade"
	"go.openpit.dev/openpit/pretrade/policies"
	"go.openpit.dev/openpit/reject"
	"go.openpit.dev/openpit/tx"
)

// Mirrors public Go examples from:
// - ../pit.wiki/Account-Adjustments.md
// - ../pit.wiki/Account-Blocking.md
// - ../pit.wiki/Account-Groups.md
// - ../pit.wiki/Async-Engine.md
// - ../pit.wiki/Balance-Reconciliation.md
// - ../pit.wiki/Domain-Types.md
// - ../pit.wiki/Dynamic-Policy-Reconfiguration.md
// - ../pit.wiki/Getting-Started.md
// - ../pit.wiki/Non-Mutating-Dry-Run.md
// - ../pit.wiki/Policies.md
// - ../pit.wiki/Policy-API.md
// - ../pit.wiki/Pre-trade-Pipeline.md
// - ../pit.wiki/Pre-Trade-Lock.md
// - ../pit.wiki/Spot-Funds.md
// If this file changes, update every linked documentation snippet.

// --- Policy-API: Custom Order and Execution Report Models ---

type StrategyOrder struct {
	model.Order
	StrategyTag string
}

type StrategyReport struct {
	model.ExecutionReport
	VenueExecID string
}

type StrategyTagPolicy struct{}

func (*StrategyTagPolicy) Close() {}

func (*StrategyTagPolicy) Name() string { return "StrategyTagPolicy" }

func (*StrategyTagPolicy) PolicyGroupID() model.PolicyGroupID {
	return model.DefaultPolicyGroupID
}

func (p *StrategyTagPolicy) CheckPreTradeStart(
	_ pretrade.Context,
	order StrategyOrder,
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

func (*StrategyTagPolicy) PerformPreTradeCheck(
	pretrade.Context,
	StrategyOrder,
	tx.Mutations,
	pretrade.Result,
) []reject.Reject {
	return nil
}

func (*StrategyTagPolicy) ApplyExecutionReport(
	pretrade.PostTradeContext,
	StrategyReport,
	pretrade.PostTradeAdjustments,
	pretrade.PostTradePnls,
) []reject.AccountBlock {
	return nil
}

func (*StrategyTagPolicy) ApplyAccountAdjustment(
	accountadjustment.Context,
	param.AccountID,
	model.AccountAdjustment,
	tx.Mutations,
	pretrade.AccountOutcomes,
) (pretrade.PolicyAccountAdjustmentResult, []reject.Reject) {
	return pretrade.PolicyAccountAdjustmentResult{}, nil
}

// Used in: pit.wiki/Policy-API.md - Block an Account from an Adjustment Callback.
type BlockOnAdjustmentPolicy struct{}

func (*BlockOnAdjustmentPolicy) Close() {}

func (*BlockOnAdjustmentPolicy) Name() string {
	return "BlockOnAdjustmentPolicy"
}

func (*BlockOnAdjustmentPolicy) PolicyGroupID() model.PolicyGroupID {
	return model.DefaultPolicyGroupID
}

func (*BlockOnAdjustmentPolicy) CheckPreTradeStart(
	pretrade.Context,
	model.Order,
) []reject.Reject {
	return nil
}

func (*BlockOnAdjustmentPolicy) PerformPreTradeCheck(
	pretrade.Context,
	model.Order,
	tx.Mutations,
	pretrade.Result,
) []reject.Reject {
	return nil
}

func (*BlockOnAdjustmentPolicy) ApplyExecutionReport(
	pretrade.PostTradeContext,
	model.ExecutionReport,
	pretrade.PostTradeAdjustments,
	pretrade.PostTradePnls,
) []reject.AccountBlock {
	return nil
}

func (p *BlockOnAdjustmentPolicy) ApplyAccountAdjustment(
	_ accountadjustment.Context,
	_ param.AccountID,
	_ model.AccountAdjustment,
	_ tx.Mutations,
	_ pretrade.AccountOutcomes,
) (pretrade.PolicyAccountAdjustmentResult, []reject.Reject) {
	return pretrade.PolicyAccountAdjustmentResult{
		AccountBlocks: []reject.AccountBlock{reject.NewAccountBlock(
			reject.CodeAccountBlocked,
			p.Name(),
			"blocked by account-adjustment policy",
			"custom policy reported an account block from a callback",
		)},
	}, nil
}

// --- Shared helpers ---

// Used in: pit.wiki/Domain-Types.md - Create Validated Values.
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

// Used in: pit.wiki/Domain-Types.md - Create Validated Values.
// Keep this example synced with wiki snippets when constructor behavior changes.
func TestExampleWikiDomainTypesAssetValidationError(t *testing.T) {
	_, err := param.NewAsset("  ")
	if err == nil {
		t.Fatal("NewAsset(empty) error = nil, want ErrAssetEmpty")
	}
	if !errors.Is(err, param.ErrAssetEmpty) {
		t.Fatalf("NewAsset(empty) error = %v, want %v", err, param.ErrAssetEmpty)
	}
}

// Used in: pit.wiki/Domain-Types.md - Account Identifiers.
// Keep this example synced with wiki snippets when constructor behavior changes.
func TestExampleWikiDomainTypesAccountIDValidationError(t *testing.T) {
	_, err := param.NewAccountIDFromString("  ")
	if err == nil {
		t.Fatal("NewAccountIDFromString(empty) error = nil, want ErrAccountIDEmpty")
	}
	if !errors.Is(err, param.ErrAccountIDEmpty) {
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
	op.SetAccountID(param.NewAccountIDFromUint64(99224416))
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
	op.SetAccountID(param.NewAccountIDFromUint64(99224416))
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

func wikiExampleEngine(t *testing.T) *Engine {
	t.Helper()

	engine, err := NewEngineBuilder().
		FullSync().
		Builtin(policies.BuildOrderValidation()).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	t.Cleanup(engine.Stop)
	return engine
}

// --- Policy-API: Custom Main-Stage Policy ---

type NotionalCapPolicy struct {
	MaxAbsNotional param.Volume
}

func (*NotionalCapPolicy) Close() {}

func (*NotionalCapPolicy) Name() string { return "NotionalCapPolicy" }

func (*NotionalCapPolicy) PolicyGroupID() model.PolicyGroupID { return model.DefaultPolicyGroupID }

func (*NotionalCapPolicy) CheckPreTradeStart(pretrade.Context, model.Order) []reject.Reject {
	return nil
}

func (p *NotionalCapPolicy) PerformPreTradeCheck(
	_ pretrade.Context,
	order model.Order,
	_ tx.Mutations,
	_ pretrade.Result,
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

func (*NotionalCapPolicy) ApplyExecutionReport(
	pretrade.PostTradeContext,
	model.ExecutionReport,
	pretrade.PostTradeAdjustments,
	pretrade.PostTradePnls,
) []reject.AccountBlock {
	return nil
}

func (*NotionalCapPolicy) ApplyAccountAdjustment(
	accountadjustment.Context,
	param.AccountID,
	model.AccountAdjustment,
	tx.Mutations,
	pretrade.AccountOutcomes,
) (pretrade.PolicyAccountAdjustmentResult, []reject.Reject) {
	return pretrade.PolicyAccountAdjustmentResult{}, nil
}

// --- Policy-API: Rollback Safety Pattern ---

type ReserveThenValidatePolicy struct {
	reserved param.Volume
	limit    param.Volume
}

func (*ReserveThenValidatePolicy) Close() {}

func (*ReserveThenValidatePolicy) Name() string { return "ReserveThenValidatePolicy" }

func (*ReserveThenValidatePolicy) PolicyGroupID() model.PolicyGroupID {
	return model.DefaultPolicyGroupID
}

func (*ReserveThenValidatePolicy) CheckPreTradeStart(pretrade.Context, model.Order) []reject.Reject {
	return nil
}

func (p *ReserveThenValidatePolicy) PerformPreTradeCheck(
	_ pretrade.Context,
	_ model.Order,
	mutations tx.Mutations,
	_ pretrade.Result,
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

func (*ReserveThenValidatePolicy) ApplyExecutionReport(
	pretrade.PostTradeContext,
	model.ExecutionReport,
	pretrade.PostTradeAdjustments,
	pretrade.PostTradePnls,
) []reject.AccountBlock {
	return nil
}

func (*ReserveThenValidatePolicy) ApplyAccountAdjustment(
	accountadjustment.Context,
	param.AccountID,
	model.AccountAdjustment,
	tx.Mutations,
	pretrade.AccountOutcomes,
) (pretrade.PolicyAccountAdjustmentResult, []reject.Reject) {
	return pretrade.PolicyAccountAdjustmentResult{}, nil
}

// --- Tests ---

// Used in: pit.wiki/Pre-trade-Pipeline.md - Handle a Start-Stage Reject
func TestExampleWikiPipelineStartStageReject(t *testing.T) {
	engine := wikiExampleEngine(t)
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

// Used in: pit.wiki/Pre-trade-Pipeline.md - Execute the Main Stage and
// Finalize the Reservation
func TestExampleWikiPipelineMainStageFinalize(t *testing.T) {
	engine := wikiExampleEngine(t)
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

// Used in: pit.wiki/Pre-trade-Pipeline.md - Shortcut for Start + Main Stages
// Used in: pit.wiki/Getting-Started.md - Shortcut for Start + Main Stages
func TestExampleWikiPipelineShortcutStartAndMain(t *testing.T) {
	engine := wikiExampleEngine(t)
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

// Used in: pit.wiki/Pre-trade-Pipeline.md - Apply Post-Trade Feedback
func TestExampleWikiPipelineApplyPostTrade(t *testing.T) {
	engine := wikiExampleEngine(t)
	report := wikiExampleReport(t, "-50", "3.4")

	result, err := engine.ApplyExecutionReport(report)
	if err != nil {
		t.Fatalf("ApplyExecutionReport() error = %v", err)
	}
	for _, outcome := range result.AccountPnls {
		fmt.Printf("account P&L outcome for %v\n", outcome.AccountID)
	}
	for _, outcome := range result.AccountAdjustments {
		fmt.Printf("account adjustment from group %d\n", outcome.PolicyGroupID)
	}
	if len(result.AccountBlocks) > 0 {
		t.Fatalf("AccountBlocks = %v, want none", result.AccountBlocks)
	}
}

// Used in: pit.wiki/Policy-API.md - Example: Custom Main-Stage Policy
func TestExampleWikiPolicyNotionalCap(t *testing.T) {
	maxNotional, err := param.NewVolumeFromString("1000")
	if err != nil {
		t.Fatalf("NewVolumeFromString() error = %v", err)
	}

	policy := &NotionalCapPolicy{MaxAbsNotional: maxNotional}

	engine, err := NewEngineBuilder().FullSync().PreTrade(policy).Build()
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

// Used in: pit.wiki/Policy-API.md - Example: Rollback Safety Pattern
func TestExampleWikiPolicyRollbackSafety(t *testing.T) {
	limit, err := param.NewVolumeFromString("50")
	if err != nil {
		t.Fatalf("NewVolumeFromString() error = %v", err)
	}

	policy := &ReserveThenValidatePolicy{
		reserved: param.NewVolumeZero(),
		limit:    limit,
	}

	engine, err := NewEngineBuilder().FullSync().PreTrade(policy).Build()
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
	if !policy.reserved.Equal(param.NewVolumeZero()) {
		t.Fatalf("reserved after rollback = %v, want zero", policy.reserved)
	}
}

// Used in: pit.wiki/Getting-Started.md - Build an Engine
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

	engine, err := NewEngineBuilder().
		FullSync().
		Builtin(policies.BuildOrderValidation()).
		Builtin(
			policies.BuildPnlBoundsKillSwitch().BrokerBarriers(
				policies.PnlBoundsBrokerBarrier{
					SettlementAsset: usd,
					LowerBound:      optional.Some(lowerBound),
				},
			),
		).
		Builtin(
			policies.BuildRateLimit().BrokerBarrier(
				policies.RateLimitBrokerBarrier{
					Limit: policies.RateLimit{MaxOrders: 100, Window: time.Second},
				},
			),
		).
		Builtin(
			policies.BuildOrderSizeLimit().
				AssetBarriers(
					policies.OrderSizeAssetBarrier{
						SettlementAsset: usd,
						Limit: policies.OrderSizeLimit{
							MaxQuantity: maxQty,
							MaxNotional: maxNotional,
						},
					},
				).
				BrokerBarrier(
					policies.OrderSizeBrokerBarrier{
						Limit: policies.OrderSizeLimit{
							MaxQuantity: maxQty,
							MaxNotional: maxNotional,
						},
					},
				),
		).
		Build()
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
	op.SetAccountID(param.NewAccountIDFromUint64(99224416))
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
	reportOp.SetAccountID(param.NewAccountIDFromUint64(99224416))
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
	for _, outcome := range result.AccountPnls {
		fmt.Printf("account P&L outcome for %v\n", outcome.AccountID)
	}
	for _, outcome := range result.AccountAdjustments {
		fmt.Printf("account adjustment from group %d\n", outcome.PolicyGroupID)
	}
	if len(result.AccountBlocks) > 0 {
		t.Fatalf("AccountBlocks = %v, want none", result.AccountBlocks)
	}
}

// Used in: pit.wiki/Getting-Started.md - Shortcut for Start + Main Stages
func TestExampleWikiGettingStartedShortcut(t *testing.T) {
	engine := wikiExampleEngine(t)
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

// Used in: pit.wiki/Getting-Started.md - Run an Order Through the Engine
func TestExampleWikiGettingStartedRunOrder(t *testing.T) {
	engine := wikiExampleEngine(t)
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

// Used in: pit.wiki/Getting-Started.md - Apply Post-Trade Feedback
func TestExampleWikiGettingStartedApplyPostTrade(t *testing.T) {
	engine := wikiExampleEngine(t)
	report := wikiExampleReport(t, "-50", "3.4")

	result, err := engine.ApplyExecutionReport(report)
	if err != nil {
		t.Fatalf("ApplyExecutionReport() error = %v", err)
	}
	for _, outcome := range result.AccountPnls {
		log.Printf("account P&L outcome for %v", outcome.AccountID)
	}
	for _, outcome := range result.AccountAdjustments {
		log.Printf("account adjustment from group %d", outcome.PolicyGroupID)
	}
	if len(result.AccountBlocks) > 0 {
		t.Fatalf("AccountBlocks = %v, want none", result.AccountBlocks)
	}
}

// Used in: pit.wiki/Policy-API.md - Go Custom Models
func TestExampleWikiCustomGoModels(t *testing.T) {
	engine, err := NewClientPreTradeEngineBuilder[StrategyOrder, StrategyReport]().
		FullSync().
		PreTrade(&StrategyTagPolicy{}).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	// Allowed order must pass.
	allowed := StrategyOrder{Order: model.NewOrder(), StrategyTag: "alpha"}
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
	blocked := StrategyOrder{Order: model.NewOrder(), StrategyTag: "blocked"}
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

func TestExampleWikiPolicyBlocksAccountFromAdjustment(t *testing.T) {
	engine, err := NewEngineBuilder().
		NoSync().
		PreTrade(&BlockOnAdjustmentPolicy{}).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	accountID := param.NewAccountIDFromUint64(99224416)
	adjustmentResult, err := engine.ApplyAccountAdjustment(
		accountID,
		[]model.AccountAdjustment{model.NewAccountAdjustment()},
	)
	if err != nil {
		t.Fatalf("ApplyAccountAdjustment() error = %v", err)
	}
	if adjustmentResult.BatchError.IsSet() {
		t.Fatalf("ApplyAccountAdjustment() unexpected rejects: %v", adjustmentResult.BatchError)
	}
	if len(adjustmentResult.AccountBlocks) != 1 {
		t.Fatalf("ApplyAccountAdjustment() blocks = %v, want one", adjustmentResult.AccountBlocks)
	}

	request, blocked, err := engine.StartPreTrade(wikiExampleOrder(t, "10", "25"))
	if err != nil {
		t.Fatalf("StartPreTrade() error = %v", err)
	}
	if request != nil {
		request.Close()
		t.Fatal("StartPreTrade() passed for a blocked account")
	}
	if len(blocked) == 0 || blocked[0].Code != reject.CodeAccountBlocked {
		t.Fatalf("StartPreTrade() rejects = %v, want AccountBlocked", blocked)
	}
}

// Used in: pit.wiki/Policies.md - SpotFundsPolicy
func TestExampleWikiPoliciesSpotFunds(t *testing.T) {
	engine, err := NewEngineBuilder().
		NoSync().
		Builtin(policies.BuildSpotFunds()).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()
}

// Used in: pit.wiki/Policies.md - OrderValidationPolicy
func TestExampleWikiPoliciesOrderValidation(t *testing.T) {
	engine, err := NewEngineBuilder().
		NoSync().
		Builtin(
			policies.BuildOrderValidation(),
		).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	order := wikiExampleOrder(t, "100", "185")
	request, rejects, err := engine.StartPreTrade(order)
	if err != nil {
		t.Fatalf("StartPreTrade() error = %v", err)
	}
	if rejects != nil {
		t.Fatalf("StartPreTrade() unexpected rejects: %v", rejects)
	}
	reservation, rejects, err := request.Execute()
	if err != nil {
		t.Fatalf("Execute() error = %v", err)
	}
	if rejects != nil {
		t.Fatalf("Execute() unexpected rejects: %v", rejects)
	}
	reservation.CommitAndClose()
}

// Used in: pit.wiki/Policies.md - RateLimitPolicy
func TestExampleWikiPoliciesRateLimit(t *testing.T) {
	engine, err := NewEngineBuilder().
		NoSync().
		Builtin(
			policies.BuildRateLimit().
				BrokerBarrier(
					policies.RateLimitBrokerBarrier{
						Limit: policies.RateLimit{
							MaxOrders: 100,
							Window:    time.Second,
						},
					},
				),
		).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	order := wikiExampleOrder(t, "1", "100")
	request, rejects, err := engine.StartPreTrade(order)
	if err != nil {
		t.Fatalf("StartPreTrade() error = %v", err)
	}
	if rejects != nil {
		t.Fatalf("StartPreTrade() unexpected rejects: %v", rejects)
	}
	reservation, rejects, err := request.Execute()
	if err != nil {
		t.Fatalf("Execute() error = %v", err)
	}
	if rejects != nil {
		t.Fatalf("Execute() unexpected rejects: %v", rejects)
	}
	reservation.CommitAndClose()
}

// Used in: pit.wiki/Policies.md - OrderSizeLimitPolicy
func TestExampleWikiPoliciesOrderSizeLimit(t *testing.T) {
	usd, err := param.NewAsset("USD")
	if err != nil {
		t.Fatalf("NewAsset() error = %v", err)
	}
	maxQty, err := param.NewQuantityFromString("100")
	if err != nil {
		t.Fatalf("NewQuantityFromString() error = %v", err)
	}
	maxNotional, err := param.NewVolumeFromString("50000")
	if err != nil {
		t.Fatalf("NewVolumeFromString() error = %v", err)
	}

	engine, err := NewEngineBuilder().
		NoSync().
		Builtin(
			policies.BuildOrderSizeLimit().
				AssetBarriers(
					policies.OrderSizeAssetBarrier{
						SettlementAsset: usd,
						Limit: policies.OrderSizeLimit{
							MaxQuantity: maxQty,
							MaxNotional: maxNotional,
						},
					},
				).
				BrokerBarrier(
					policies.OrderSizeBrokerBarrier{
						Limit: policies.OrderSizeLimit{
							MaxQuantity: maxQty,
							MaxNotional: maxNotional,
						},
					},
				),
		).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	order := wikiExampleOrder(t, "10", "100")
	request, rejects, err := engine.StartPreTrade(order)
	if err != nil {
		t.Fatalf("StartPreTrade() error = %v", err)
	}
	if rejects != nil {
		t.Fatalf("StartPreTrade() unexpected rejects: %v", rejects)
	}
	reservation, rejects, err := request.Execute()
	if err != nil {
		t.Fatalf("Execute() error = %v", err)
	}
	if rejects != nil {
		t.Fatalf("Execute() unexpected rejects: %v", rejects)
	}
	reservation.CommitAndClose()
}

// Used in: pit.wiki/Policies.md - PnlBoundsKillSwitchPolicy
func TestExampleWikiPoliciesPnlBoundsKillSwitch(t *testing.T) {
	usd, err := param.NewAsset("USD")
	if err != nil {
		t.Fatalf("NewAsset() error = %v", err)
	}
	lowerBound, err := param.NewPnlFromString("-1000")
	if err != nil {
		t.Fatalf("NewPnlFromString() error = %v", err)
	}
	upperBound, err := param.NewPnlFromString("500")
	if err != nil {
		t.Fatalf("NewPnlFromString() error = %v", err)
	}

	engine, err := NewEngineBuilder().
		NoSync().
		Builtin(
			policies.BuildPnlBoundsKillSwitch().
				BrokerBarriers(
					policies.PnlBoundsBrokerBarrier{
						SettlementAsset: usd,
						LowerBound:      optional.Some(lowerBound),
						UpperBound:      optional.Some(upperBound),
					},
				),
		).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	order := wikiExampleOrder(t, "1", "100")
	request, rejects, err := engine.StartPreTrade(order)
	if err != nil {
		t.Fatalf("StartPreTrade() error = %v", err)
	}
	if rejects != nil {
		t.Fatalf("StartPreTrade() unexpected rejects: %v", rejects)
	}
	reservation, rejects, err := request.Execute()
	if err != nil {
		t.Fatalf("Execute() error = %v", err)
	}
	if rejects != nil {
		t.Fatalf("Execute() unexpected rejects: %v", rejects)
	}
	reservation.CommitAndClose()
}

// Used in: pit.wiki/Policies.md - Synchronization
//
// The const/type/worker-pool/dispatch user code mirrors the wiki snippet
// verbatim. The rest is test harness: the AccountSync engine, the fnv32
// helper the reader supplies, the distinct per-account tasks, a WaitGroup
// to drain the workers before stopping the engine, and assertions. The only
// lines woven into the worker loop are the WaitGroup Add/Done bookkeeping
// that lets the test join the workers deterministically.
func TestExampleWikiPoliciesAccountSyncShardedDispatch(t *testing.T) {
	engine, err := NewEngineBuilder().
		AccountSync().
		Builtin(policies.BuildOrderValidation()).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}

	// fnv32 is a given the reader supplies; the standard FNV-1a hash routes
	// each account ID to a shard.
	fnv32 := func(s string) uint32 {
		const (
			offset = 2166136261
			prime  = 16777619
		)
		hash := uint32(offset)
		for i := 0; i < len(s); i++ {
			hash ^= uint32(s[i])
			hash *= prime
		}
		return hash
	}

	// --- Begin wiki snippet (pit.wiki/Policies.md - Synchronization). ---
	const shards = 256

	type task struct {
		accountID string
		order     model.Order
	}

	// One channel per shard, one goroutine draining each channel: the same
	// account always lands on the same worker, so AccountSync holds.
	var workersWG sync.WaitGroup
	workers := make([]chan task, shards)
	for i := range workers {
		ch := make(chan task, 1024)
		workers[i] = ch
		workersWG.Add(1)
		go func(ch <-chan task) {
			defer workersWG.Done()
			for t := range ch {
				req, _, _ := engine.ExecutePreTrade(t.order)
				if req != nil {
					req.Close()
				}
			}
		}(ch)
	}

	// fnv32 hashes the account ID; the modulo pins each account to one shard.
	dispatch := func(t task) {
		workers[fnv32(t.accountID)%shards] <- t
	}
	// --- End wiki snippet. ---

	// Harness: dispatch tasks for several distinct accounts. Each routing key
	// is bound to its own engine account, so no engine account is ever touched
	// by more than one shard - the AccountSync invariant.
	const accounts = 16
	seen := make(map[uint32]string, accounts)
	for i := 0; i < accounts; i++ {
		accountID := fmt.Sprintf("acct-%d", i)
		// Every distinct account must resolve to a stable single shard, and no
		// two distinct accounts used here may collide onto the same shard, or
		// the test would touch one engine account from two routing keys.
		shard := fnv32(accountID) % shards
		if prev, ok := seen[shard]; ok {
			t.Fatalf("accounts %q and %q collide on shard %d", prev, accountID, shard)
		}
		seen[shard] = accountID
		dispatch(task{
			accountID: accountID,
			order:     wikiAccountSyncOrder(t, uint64(i+1)),
		})
	}

	// Drain: closing every channel lets each worker finish its range loop; the
	// WaitGroup join guarantees no ExecutePreTrade is in flight before Stop.
	for _, ch := range workers {
		close(ch)
	}
	workersWG.Wait()
	engine.Stop()
}

// wikiAccountSyncOrder builds a minimal valid AAPL/USD buy order bound to a
// distinct account, used to feed the sharded-dispatch example harness.
func wikiAccountSyncOrder(t *testing.T, accountID uint64) model.Order {
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
	op.SetAccountID(param.NewAccountIDFromUint64(accountID))
	op.SetSide(param.SideBuy)
	qty, err := param.NewQuantityFromString("1")
	if err != nil {
		t.Fatalf("NewQuantityFromString() error = %v", err)
	}
	price, err := param.NewPriceFromString("185")
	if err != nil {
		t.Fatalf("NewPriceFromString() error = %v", err)
	}
	op.SetTradeAmount(param.NewQuantityTradeAmount(qty))
	op.SetPrice(price)
	return order
}

// Used in: pit.wiki/Domain-Types.md - Work With Directional Types
func TestExampleWikiDomainTypesDirectionalTypes(t *testing.T) {
	//nolint:staticcheck // explicit types mirror the published snippet verbatim
	var side param.Side = param.SideBuy
	//nolint:staticcheck // explicit types mirror the published snippet verbatim
	var positionSide param.PositionSide = param.PositionSideLong

	if side != param.SideBuy {
		t.Fatalf("side = %v, want %v", side, param.SideBuy)
	}
	if positionSide != param.PositionSideLong {
		t.Fatalf("positionSide = %v, want %v", positionSide, param.PositionSideLong)
	}
}

// Used in: pit.wiki/Domain-Types.md - Create Leverage
func TestExampleWikiDomainTypesLeverage(t *testing.T) {
	fromMultiplier := param.NewLeverageFromUint16(100)
	fromFloat := param.NewLeverageFromFloat32(100.5)

	if got := fromMultiplier.Value(); got != 100.0 {
		t.Fatalf("fromMultiplier.Value() = %v, want %v", got, 100.0)
	}
	if got := fromFloat.Value(); got != 100.5 {
		t.Fatalf("fromFloat.Value() = %v, want %v", got, 100.5)
	}
}

// Used in: pit.wiki/Account-Adjustments.md - Examples → Go
func TestExampleWikiAccountAdjustments(t *testing.T) {
	maxCumulative, err := param.NewPositionSizeFromString("1000000")
	if err != nil {
		t.Fatalf("NewPositionSizeFromString() error = %v", err)
	}
	acceptAll := &CumulativeLimitPolicy{
		maxCumulative: maxCumulative,
		totals:        make(map[string]param.PositionSize),
	}
	engine, err := NewEngineBuilder().FullSync().PreTrade(acceptAll).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	accountID := param.NewAccountIDFromUint64(99224416)

	usd, err := param.NewAsset("USD")
	if err != nil {
		t.Fatalf("NewAsset(USD) error = %v", err)
	}
	spx, err := param.NewAsset("SPX")
	if err != nil {
		t.Fatalf("NewAsset(SPX) error = %v", err)
	}
	entryPrice, err := param.NewPriceFromString("95000")
	if err != nil {
		t.Fatalf("NewPriceFromString() error = %v", err)
	}
	totalCash, err := param.NewPositionSizeFromString("10000")
	if err != nil {
		t.Fatalf("NewPositionSizeFromString(10000) error = %v", err)
	}
	totalPosition, err := param.NewPositionSizeFromString("-3")
	if err != nil {
		t.Fatalf("NewPositionSizeFromString(-3) error = %v", err)
	}

	cashAdj, err := model.NewAccountAdjustmentFromValues(model.AccountAdjustmentValues{
		BalanceOperation: optional.Some(
			model.NewAccountAdjustmentBalanceOperationFromValues(
				model.AccountAdjustmentBalanceOperationValues{
					Asset: optional.Some(usd),
				},
			),
		),
		Amount: optional.Some(
			model.NewAccountAdjustmentAmountFromValues(model.AccountAdjustmentAmountValues{
				Balance: optional.Some(param.NewAbsoluteAdjustmentAmount(totalCash)),
			}),
		),
	})
	if err != nil {
		t.Fatalf("NewAccountAdjustmentFromValues(cash) error = %v", err)
	}

	posAdj, err := model.NewAccountAdjustmentFromValues(model.AccountAdjustmentValues{
		PositionOperation: optional.Some(
			model.NewAccountAdjustmentPositionOperationFromValues(
				model.AccountAdjustmentPositionOperationValues{
					Instrument:        optional.Some(param.NewInstrument(spx, usd)),
					CollateralAsset:   optional.Some(usd),
					AverageEntryPrice: optional.Some(entryPrice),
					Mode:              optional.Some(param.PositionModeHedged),
				},
			),
		),
		Amount: optional.Some(
			model.NewAccountAdjustmentAmountFromValues(model.AccountAdjustmentAmountValues{
				Balance: optional.Some(param.NewAbsoluteAdjustmentAmount(totalPosition)),
			}),
		),
	})
	if err != nil {
		t.Fatalf("NewAccountAdjustmentFromValues(position) error = %v", err)
	}

	result, err := engine.ApplyAccountAdjustment(
		accountID,
		[]model.AccountAdjustment{cashAdj, posAdj},
	)
	if err != nil {
		t.Fatalf("ApplyAccountAdjustment() error = %v", err)
	}
	if result.BatchError.IsSet() {
		t.Fatalf("ApplyAccountAdjustment() rejects = %v, want none", result.BatchError)
	}
	if len(result.AccountBlocks) != 0 {
		t.Fatalf("ApplyAccountAdjustment() account blocks = %v, want none", result.AccountBlocks)
	}
}

// Used in: pit.wiki/Account-Adjustments.md - Example: Balance Limit Policy → Go
func TestExampleWikiAccountAdjustmentsBalanceLimitPolicy(t *testing.T) {
	maxCumulative, err := param.NewPositionSizeFromString("100")
	if err != nil {
		t.Fatalf("NewPositionSizeFromString() error = %v", err)
	}
	policy := &CumulativeLimitPolicy{
		maxCumulative: maxCumulative,
		totals:        make(map[string]param.PositionSize),
	}

	engine, err := NewEngineBuilder().FullSync().PreTrade(policy).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	usd, err := param.NewAsset("USD")
	if err != nil {
		t.Fatalf("NewAsset(USD) error = %v", err)
	}
	totalCash, err := param.NewPositionSizeFromString("100")
	if err != nil {
		t.Fatalf("NewPositionSizeFromString() error = %v", err)
	}
	adjustment, err := model.NewAccountAdjustmentFromValues(model.AccountAdjustmentValues{
		BalanceOperation: optional.Some(
			model.NewAccountAdjustmentBalanceOperationFromValues(
				model.AccountAdjustmentBalanceOperationValues{
					Asset: optional.Some(usd),
				},
			),
		),
		Amount: optional.Some(
			model.NewAccountAdjustmentAmountFromValues(model.AccountAdjustmentAmountValues{
				Balance: optional.Some(param.NewAbsoluteAdjustmentAmount(totalCash)),
			}),
		),
	})
	if err != nil {
		t.Fatalf("NewAccountAdjustmentFromValues() error = %v", err)
	}

	result, err := engine.ApplyAccountAdjustment(
		param.NewAccountIDFromUint64(99224416),
		[]model.AccountAdjustment{adjustment},
	)
	if err != nil {
		t.Fatalf("ApplyAccountAdjustment() error = %v", err)
	}
	if result.BatchError.IsSet() {
		t.Fatalf("ApplyAccountAdjustment() rejects = %v, want none", result.BatchError)
	}
	if len(result.AccountBlocks) != 0 {
		t.Fatalf("ApplyAccountAdjustment() account blocks = %v, want none", result.AccountBlocks)
	}
}

type CumulativeLimitPolicy struct {
	maxCumulative param.PositionSize
	totals        map[string]param.PositionSize
}

func (*CumulativeLimitPolicy) Close() {}

func (*CumulativeLimitPolicy) Name() string { return "CumulativeLimitPolicy" }

func (*CumulativeLimitPolicy) PolicyGroupID() model.PolicyGroupID {
	return model.DefaultPolicyGroupID
}

func (*CumulativeLimitPolicy) CheckPreTradeStart(
	pretrade.Context,
	model.Order,
) []reject.Reject {
	return nil
}

func (*CumulativeLimitPolicy) PerformPreTradeCheck(
	pretrade.Context,
	model.Order,
	tx.Mutations,
	pretrade.Result,
) []reject.Reject {
	return nil
}

func (*CumulativeLimitPolicy) ApplyExecutionReport(
	pretrade.PostTradeContext,
	model.ExecutionReport,
	pretrade.PostTradeAdjustments,
	pretrade.PostTradePnls,
) []reject.AccountBlock {
	return nil
}

func (v *CumulativeLimitPolicy) ApplyAccountAdjustment(
	_ accountadjustment.Context,
	_ param.AccountID,
	adjustment model.AccountAdjustment,
	mutations tx.Mutations,
	_ pretrade.AccountOutcomes,
) (pretrade.PolicyAccountAdjustmentResult, []reject.Reject) {
	operation, ok := adjustment.BalanceOperation().Get()
	if !ok {
		return pretrade.PolicyAccountAdjustmentResult{}, nil
	}
	asset, ok := operation.Asset().Get()
	if !ok {
		return pretrade.PolicyAccountAdjustmentResult{}, nil
	}
	amounts, ok := adjustment.Amount().Get()
	if !ok {
		return pretrade.PolicyAccountAdjustmentResult{}, nil
	}
	balance, ok := amounts.Balance().Get()
	if !ok {
		return pretrade.PolicyAccountAdjustmentResult{}, nil
	}

	if v.totals == nil {
		v.totals = make(map[string]param.PositionSize)
	}

	assetID := asset.String()
	previous, existed := v.totals[assetID]
	current := previous
	if !existed {
		current = param.NewPositionSizeZero()
	}

	var next param.PositionSize
	if balance.IsAbsolute() {
		next = balance.MustAbsolute()
	} else {
		var err error
		next, err = current.CheckedAdd(balance.MustDelta())
		if err != nil {
			return pretrade.PolicyAccountAdjustmentResult{}, reject.NewSingleItemList(
				reject.CodeArithmeticOverflow,
				v.Name(),
				"invalid adjustment total",
				err.Error(),
				reject.ScopeAccount,
			)
		}
	}

	if next.Compare(v.maxCumulative) > 0 {
		return pretrade.PolicyAccountAdjustmentResult{}, reject.NewSingleItemList(
			reject.CodeRiskLimitExceeded,
			v.Name(),
			"cumulative limit exceeded",
			fmt.Sprintf("%s: %s > %s", assetID, next, v.maxCumulative),
			reject.ScopeAccount,
		)
	}

	v.totals[assetID] = next
	rollback := func() {
		if existed {
			v.totals[assetID] = previous
		} else {
			delete(v.totals, assetID)
		}
	}
	if err := mutations.Push(func() {}, rollback); err != nil {
		rollback()
		return pretrade.PolicyAccountAdjustmentResult{}, reject.NewSingleItemList(
			reject.CodeSystemUnavailable,
			v.Name(),
			"failed to register adjustment rollback",
			err.Error(),
			reject.ScopeAccount,
		)
	}
	return pretrade.PolicyAccountAdjustmentResult{}, nil
}

// wikiSeedBalanceAdjustment builds an absolute USD balance adjustment used to
// seed spot-funds available funds through the account-adjustment pipeline.
func wikiSeedBalanceAdjustment(t *testing.T, amount string) model.AccountAdjustment {
	t.Helper()

	usd, err := param.NewAsset("USD")
	if err != nil {
		t.Fatalf("NewAsset(USD) error = %v", err)
	}
	total, err := param.NewPositionSizeFromString(amount)
	if err != nil {
		t.Fatalf("NewPositionSizeFromString(%q) error = %v", amount, err)
	}

	adj, err := model.NewAccountAdjustmentFromValues(model.AccountAdjustmentValues{
		BalanceOperation: optional.Some(
			model.NewAccountAdjustmentBalanceOperationFromValues(
				model.AccountAdjustmentBalanceOperationValues{
					Asset: optional.Some(usd),
				},
			),
		),
		Amount: optional.Some(
			model.NewAccountAdjustmentAmountFromValues(model.AccountAdjustmentAmountValues{
				Balance: optional.Some(param.NewAbsoluteAdjustmentAmount(total)),
			}),
		),
	})
	if err != nil {
		t.Fatalf("NewAccountAdjustmentFromValues() error = %v", err)
	}
	return adj
}

// Used in: pit.wiki/Spot-Funds.md - Limit-Only Mode (Default)
func TestExampleWikiSpotFundsLimitOnly(t *testing.T) {
	// Limit-only spot funds: register first in the policy list.
	engine, err := NewEngineBuilder().
		FullSync().
		Builtin(policies.BuildSpotFunds()).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	accountID := param.NewAccountIDFromUint64(99224416)

	// Seed 10000 USD of available funds through the account-adjustment pipeline.
	seed := wikiSeedBalanceAdjustment(t, "10000")
	seedResult, err := engine.ApplyAccountAdjustment(
		accountID,
		[]model.AccountAdjustment{seed},
	)
	if err != nil {
		t.Fatalf("ApplyAccountAdjustment() error = %v", err)
	}
	if seedResult.BatchError.IsSet() {
		t.Fatalf("ApplyAccountAdjustment() rejects = %v, want none", seedResult.BatchError)
	}

	// Buy 10 AAPL @ 200 holds 2000 USD; available drops to 8000.
	order := wikiExampleOrder(t, "10", "200")
	reservation, execRejects, err := engine.ExecutePreTrade(order)
	if err != nil {
		t.Fatalf("ExecutePreTrade() error = %v", err)
	}
	if execRejects != nil {
		t.Fatalf("ExecutePreTrade() unexpected rejects: %v", execRejects)
	}
	reservation.CommitAndClose()
}

// Used in: pit.wiki/Spot-Funds.md - Market Orders
func TestExampleWikiSpotFundsMarketOrders(t *testing.T) {
	// Obtain the market-data builder from the engine builder so the sync mode
	// is derived automatically.
	eb := NewEngineBuilder().FullSync()
	// A shared market-data service feeds the policy's market-order pricing.
	marketData, err := eb.MarketData(marketdata.InfiniteTTL()).Build()
	if err != nil {
		t.Fatalf("marketdata Build() error = %v", err)
	}
	defer marketData.Close()

	aapl, err := param.NewAsset("AAPL")
	if err != nil {
		t.Fatalf("NewAsset(AAPL) error = %v", err)
	}
	usd, err := param.NewAsset("USD")
	if err != nil {
		t.Fatalf("NewAsset(USD) error = %v", err)
	}
	instrument := param.NewInstrument(aapl, usd)

	aaplID, err := marketData.Register(instrument)
	if err != nil {
		t.Fatalf("Register() error = %v", err)
	}
	mark, _ := param.NewPriceFromString("200")
	if err := marketData.Push(aaplID, marketdata.NewQuote().WithMark(mark)); err != nil {
		t.Fatalf("Push() error = %v", err)
	}

	// Spot funds with market orders enabled at 1500 bps worst-case slippage,
	// priced from the quote mark.
	engine, err := eb.
		Builtin(
			policies.BuildSpotFunds().
				WithMarketOrders(marketData, 1500).
				PricingSource(policies.SpotFundsPricingSourceMark),
		).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	accountID := param.NewAccountIDFromUint64(99224416)
	seed := wikiSeedBalanceAdjustment(t, "10000")
	seedResult, err := engine.ApplyAccountAdjustment(
		accountID,
		[]model.AccountAdjustment{seed},
	)
	if err != nil {
		t.Fatalf("ApplyAccountAdjustment() error = %v", err)
	}
	if seedResult.BatchError.IsSet() {
		t.Fatalf("ApplyAccountAdjustment() rejects = %v, want none", seedResult.BatchError)
	}

	// Market buy (no price): priced at mark 200 + 15% = 230 per unit worst case.
	order := model.NewOrder()
	op := order.EnsureOperationView()
	op.SetInstrument(instrument)
	op.SetAccountID(accountID)
	op.SetSide(param.SideBuy)
	qty, err := param.NewQuantityFromString("5")
	if err != nil {
		t.Fatalf("NewQuantityFromString() error = %v", err)
	}
	op.SetTradeAmount(param.NewQuantityTradeAmount(qty))

	reservation, execRejects, err := engine.ExecutePreTrade(order)
	if err != nil {
		t.Fatalf("ExecutePreTrade() error = %v", err)
	}
	if execRejects != nil {
		t.Fatalf("ExecutePreTrade() unexpected rejects: %v", execRejects)
	}
	reservation.CommitAndClose()
}

// Used in: pit.wiki/Spot-Funds.md - Self-Computed PnL Kill Switch / Configuring Barriers
func TestExampleWikiSpotFundsPnlKillSwitchBuilder(t *testing.T) {
	account := param.NewAccountIDFromUint64(99224416)
	lower, _ := param.NewPnlFromString("-1000")
	accountLower, _ := param.NewPnlFromString("-250")

	// The PnL kill switch is a distinct spot-funds builder entry point; it
	// produces the same SpotFundsPolicy, registered under the same name.
	engine, err := NewEngineBuilder().
		NoSync().
		Builtin(
			policies.BuildSpotFundsPnlBoundsKillSwitch().
				GlobalBarrier(policies.SpotFundsPnlBoundsBarrier{
					LowerBound: optional.Some(lower),
				}).
				AccountBarriers(policies.SpotFundsPnlBoundsAccountBarrier{
					AccountID: account,
					Barrier: policies.SpotFundsPnlBoundsBarrier{
						LowerBound: optional.Some(accountLower),
					},
				}),
		).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()
}

// Used in: pit.wiki/Spot-Funds.md - Self-Computed PnL Kill Switch / Runtime Reconfiguration
func TestExampleWikiSpotFundsPnlKillSwitchReconfigure(t *testing.T) {
	// Harness scaffolding: a spot-funds engine with a per-account barrier the
	// snippet then retunes and force-sets.
	seedAccount := param.NewAccountIDFromUint64(99224416)
	seedLower, _ := param.NewPnlFromString("-250")
	engine, err := NewEngineBuilder().
		NoSync().
		Builtin(
			policies.BuildSpotFundsPnlBoundsKillSwitch().
				AccountBarriers(policies.SpotFundsPnlBoundsAccountBarrier{
					AccountID: seedAccount,
					Barrier: policies.SpotFundsPnlBoundsBarrier{
						LowerBound: optional.Some(seedLower),
					},
				}),
		).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	account := param.NewAccountIDFromUint64(99224416)
	newLower, _ := param.NewPnlFromString("-500")
	forced, _ := param.NewPnlFromString("-600")
	globalBarrier := policies.SpotFundsPnlBoundsBarrier{
		LowerBound: optional.Some(newLower),
	}

	// Retune the account PnL barrier; live accumulated PnL is untouched.
	if err := engine.Configure().SpotFundsPnlBoundsKillSwitch(
		policies.SpotFundsPolicyName,
		optional.Some(&globalBarrier),
		nil,
		nil,
	); err != nil {
		t.Fatalf("SpotFundsPnlBoundsKillSwitch() error = %v", err)
	}

	// Force-set the live accumulated PnL for one account.
	result, err := engine.Configure().SetSpotFundsAccountPnl(
		policies.SpotFundsPolicyName,
		account,
		model.NewPnlState(forced),
	)
	if err != nil {
		t.Fatalf("SetSpotFundsAccountPnl() error = %v", err)
	}
	if len(result.AccountBlocks) != 1 {
		t.Fatalf("AccountBlocks = %v, want one PnL block", result.AccountBlocks)
	}
}

// Used in: pit.wiki/Pre-Trade-Lock.md - Persisting and Restoring a Lock
func TestExampleWikiPreTradeLockPersistence(t *testing.T) {
	engine, err := NewEngineBuilder().
		FullSync().
		Builtin(policies.BuildSpotFunds()).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	accountID := param.NewAccountIDFromUint64(99224416)
	usd, _ := param.NewAsset("USD")
	aapl, _ := param.NewAsset("AAPL")

	// Seed 10000 USD so the buy can be reserved.
	total, _ := param.NewPositionSizeFromString("10000")
	seed, _ := model.NewAccountAdjustmentFromValues(model.AccountAdjustmentValues{
		BalanceOperation: optional.Some(
			model.NewAccountAdjustmentBalanceOperationFromValues(
				model.AccountAdjustmentBalanceOperationValues{Asset: optional.Some(usd)},
			),
		),
		Amount: optional.Some(
			model.NewAccountAdjustmentAmountFromValues(model.AccountAdjustmentAmountValues{
				Balance: optional.Some(param.NewAbsoluteAdjustmentAmount(total)),
			}),
		),
	})
	if _, err := engine.ApplyAccountAdjustment(
		accountID,
		[]model.AccountAdjustment{seed},
	); err != nil {
		t.Fatalf("ApplyAccountAdjustment() error = %v", err)
	}

	// Buy 10 AAPL @ 200 holds 2000 USD and records the lock price (200).
	order := model.NewOrder()
	op := order.EnsureOperationView()
	op.SetInstrument(param.NewInstrument(aapl, usd))
	op.SetAccountID(accountID)
	op.SetSide(param.SideBuy)
	qty, _ := param.NewQuantityFromString("10")
	price, _ := param.NewPriceFromString("200")
	op.SetTradeAmount(param.NewQuantityTradeAmount(qty))
	op.SetPrice(price)

	reservation, execRejects, err := engine.ExecutePreTrade(order)
	if err != nil {
		t.Fatalf("ExecutePreTrade() error = %v", err)
	}
	if execRejects != nil {
		t.Fatalf("ExecutePreTrade() unexpected rejects: %v", execRejects)
	}

	// Persist the lock with its built-in JSON serialization before committing.
	lock := reservation.Lock()
	payload, err := json.Marshal(lock)
	if err != nil {
		t.Fatalf("json.Marshal(lock) error = %v", err)
	}
	reservation.CommitAndClose()

	// --- After a process restart, rebuild the lock from your store. ---
	var restored pretrade.Lock
	if err := json.Unmarshal(payload, &restored); err != nil {
		t.Fatalf("json.Unmarshal(lock) error = %v", err)
	}

	// The final fill must carry the restored lock so the policy reconciles the
	// 2000 USD it held against the real fill instead of blocking the account.
	report := model.NewExecutionReport()
	reportOp := model.NewExecutionReportOperation()
	reportOp.SetInstrument(param.NewInstrument(aapl, usd))
	reportOp.SetAccountID(accountID)
	reportOp.SetSide(param.SideBuy)
	report.SetOperation(reportOp)

	filledQty, _ := param.NewQuantityFromString("10")
	leaves, _ := param.NewQuantityFromString("0")
	fill := report.EnsureFillView()
	fill.SetLastTrade(model.NewExecutionReportTrade(price, filledQty))
	fill.SetLeavesQuantity(leaves)
	fill.SetLock(restored.Bytes())
	fill.SetIsFinal(true)

	result, err := engine.ApplyExecutionReport(report)
	if err != nil {
		t.Fatalf("ApplyExecutionReport() error = %v", err)
	}
	if len(result.AccountBlocks) > 0 {
		t.Fatalf("AccountBlocks = %v, want none", result.AccountBlocks)
	}
}

// Used in: pit.wiki/Balance-Reconciliation.md - Delta Versus Absolute
func TestExampleWikiBalanceReconciliationDeltaVersusAbsolute(t *testing.T) {
	engine, err := NewEngineBuilder().
		FullSync().
		Builtin(policies.BuildSpotFunds()).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	accountID := param.NewAccountIDFromUint64(99224416)
	usd, _ := param.NewAsset("USD")

	seed := func(amount string) model.AccountAdjustment {
		total, _ := param.NewPositionSizeFromString(amount)
		adj, _ := model.NewAccountAdjustmentFromValues(model.AccountAdjustmentValues{
			BalanceOperation: optional.Some(
				model.NewAccountAdjustmentBalanceOperationFromValues(
					model.AccountAdjustmentBalanceOperationValues{Asset: optional.Some(usd)},
				),
			),
			Amount: optional.Some(
				model.NewAccountAdjustmentAmountFromValues(model.AccountAdjustmentAmountValues{
					Balance: optional.Some(param.NewAbsoluteAdjustmentAmount(total)),
				}),
			),
		})
		return adj
	}

	// First seed: available USD goes from 0 to 10000.
	firstResult, err := engine.ApplyAccountAdjustment(
		accountID,
		[]model.AccountAdjustment{seed("10000")},
	)
	if err != nil {
		t.Fatalf("ApplyAccountAdjustment(first) error = %v", err)
	}
	if firstResult.BatchError.IsSet() {
		t.Fatalf("ApplyAccountAdjustment(first) rejects = %v, want none", firstResult.BatchError)
	}
	firstUSD, ok := firstResult.Outcomes[0].Entry.Balance.Get()
	if !ok {
		t.Fatal("first balance outcome unset, want set")
	}
	want10000, _ := param.NewPositionSizeFromString("10000")
	if !firstUSD.Delta.Equal(want10000) {
		t.Fatalf("first delta = %v, want %v", firstUSD.Delta, want10000)
	}
	if !firstUSD.Absolute.Equal(want10000) {
		t.Fatalf("first absolute = %v, want %v", firstUSD.Absolute, want10000)
	}

	// Second seed: available USD goes from 10000 to 15000.
	secondResult, err := engine.ApplyAccountAdjustment(
		accountID,
		[]model.AccountAdjustment{seed("15000")},
	)
	if err != nil {
		t.Fatalf("ApplyAccountAdjustment(second) error = %v", err)
	}
	if secondResult.BatchError.IsSet() {
		t.Fatalf("ApplyAccountAdjustment(second) rejects = %v, want none", secondResult.BatchError)
	}
	secondUSD, ok := secondResult.Outcomes[0].Entry.Balance.Get()
	if !ok {
		t.Fatal("second balance outcome unset, want set")
	}
	// Delta is the change to add to your own ledger; absolute is just a snapshot.
	want5000, _ := param.NewPositionSizeFromString("5000")
	want15000, _ := param.NewPositionSizeFromString("15000")
	if !secondUSD.Delta.Equal(want5000) {
		t.Fatalf("second delta = %v, want %v", secondUSD.Delta, want5000)
	}
	if !secondUSD.Absolute.Equal(want15000) {
		t.Fatalf("second absolute = %v, want %v", secondUSD.Absolute, want15000)
	}
}

// --- Async-Engine.md mirror ---

// TestExampleWikiAsyncEngine mirrors the public example in
// ../pit.wiki/Async-Engine.md ("Example" section). If this test changes,
// update the wiki snippet to match (and vice versa).
func TestExampleWikiAsyncEngine(t *testing.T) {
	// Build an AccountSync engine and wrap it into an async facade in one
	// chain. BuildAsync is only available on the AccountSync builder; for
	// FullSync or NoSync engines the bundled async helper is not used.
	asyncBuilder, err := NewEngineBuilder().
		AccountSync().
		Builtin(policies.BuildOrderValidation()).
		Builtin(
			policies.BuildRateLimit().BrokerBarrier(
				policies.RateLimitBrokerBarrier{
					Limit: policies.RateLimit{
						MaxOrders: 100,
						Window:    time.Second,
					},
				},
			),
		).
		BuildAsync()
	if err != nil {
		t.Fatalf("BuildAsync() error = %v", err)
	}

	// Pick a dispatch strategy. Use Sharded for cheap routing across a
	// balanced account population; use Dynamic for per-account isolation
	// and per-account metrics.
	async, err := asyncBuilder.Dynamic().
		MaxQueues(0).
		IdleCleanupAfter(5 * time.Minute).
		Build()
	if err != nil {
		t.Fatalf("Dynamic Build() error = %v", err)
	}
	defer func() {
		if err := async.StopGraceful(context.Background()); err != nil {
			t.Fatalf("StopGraceful() error = %v", err)
		}
	}()

	// Submit a start-stage call. The future resolves once the worker has
	// executed the call. AsyncRequest.Execute and Close are queued in the
	// same per-account chain so AccountSync is never violated.
	usd, err := param.NewAsset("USD")
	if err != nil {
		t.Fatalf("NewAsset(USD) error = %v", err)
	}
	aapl, err := param.NewAsset("AAPL")
	if err != nil {
		t.Fatalf("NewAsset(AAPL) error = %v", err)
	}
	order := model.NewOrder()
	op := order.EnsureOperationView()
	op.SetInstrument(param.NewInstrument(aapl, usd))
	op.SetAccountID(param.NewAccountIDFromUint64(99224416))
	op.SetSide(param.SideBuy)
	price, err := param.NewPriceFromString("185")
	if err != nil {
		t.Fatalf("NewPriceFromString(185) error = %v", err)
	}
	qty, err := param.NewQuantityFromString("100")
	if err != nil {
		t.Fatalf("NewQuantityFromString(100) error = %v", err)
	}
	op.SetTradeAmount(param.NewQuantityTradeAmount(qty))
	op.SetPrice(price)

	request, rejects, err := async.StartPreTrade(
		context.Background(), order,
	).Await(context.Background())
	if err != nil {
		t.Fatalf("StartPreTrade Await error = %v", err)
	}
	if request == nil {
		t.Fatalf("StartPreTrade rejected: %v", rejects)
	}

	// The async request preserves AccountSync across the Start - Execute
	// boundary by routing Execute through the same per-account queue. The
	// future yields the same (reservation, rejects, error) tuple the
	// synchronous main stage returns.
	reservation, rejects, err := request.Execute(
		context.Background(),
	).Await(context.Background())
	if err != nil {
		t.Fatalf("Execute Await error = %v", err)
	}
	if reservation == nil {
		t.Fatalf("Execute rejected: %v", rejects)
	}

	if _, err := reservation.CommitAndClose(
		context.Background(),
	).Await(context.Background()); err != nil {
		t.Fatalf("CommitAndClose Await error = %v", err)
	}
}

// Used in: pit.wiki/Account-Blocking.md - Examples → Go
func TestExampleWikiAccountBlockUnblock(t *testing.T) {
	engine, err := NewEngineBuilder().
		FullSync().
		Builtin(policies.BuildOrderValidation()).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	accounts := engine.Accounts()

	// Block account 99224416 - all subsequent pre-trade orders are rejected.
	accounts.Block(param.NewAccountIDFromUint64(99224416), "compliance hold")

	// Unblock account 99224416 - pre-trade orders are allowed again.
	accounts.Unblock(param.NewAccountIDFromUint64(99224416))

	// Block every current and future member of a group in one call.
	desk, err := param.NewAccountGroupIDFromUint32(7)
	if err != nil {
		t.Fatalf("NewAccountGroupIDFromUint32() error = %v", err)
	}
	if err := accounts.BlockGroup(desk, "desk suspended"); err != nil {
		t.Fatalf("BlockGroup() error = %v", err)
	}
	if err := accounts.UnblockGroup(desk); err != nil {
		t.Fatalf("UnblockGroup() error = %v", err)
	}
}

// Used in: pit.wiki/Account-Groups.md - Examples → Go
func TestExampleWikiAccountGroupsRegisterAndRead(t *testing.T) {
	engine, err := NewEngineBuilder().
		FullSync().
		Builtin(policies.BuildOrderValidation()).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	// Group two accounts under one compact identifier.
	accounts := engine.Accounts()
	hedgeBook, err := param.NewAccountGroupIDFromUint32(7)
	if err != nil {
		t.Fatalf("NewAccountGroupIDFromUint32() error = %v", err)
	}
	members := []param.AccountID{
		param.NewAccountIDFromUint64(10),
		param.NewAccountIDFromUint64(11),
	}
	if err := accounts.RegisterGroup(members, hedgeBook); err != nil {
		t.Fatalf("RegisterGroup() error = %v", err)
	}

	// Membership is readable by id, without enumerating the accounts.
	group, ok := accounts.GroupOf(param.NewAccountIDFromUint64(10)).Get()
	if !ok || group.String() != hedgeBook.String() {
		t.Fatalf("GroupOf(10) = (%v, %v), want (%v, true)", group, ok, hedgeBook)
	}
	if _, ok := accounts.GroupOf(param.NewAccountIDFromUint64(99)).Get(); ok {
		t.Fatalf("GroupOf(99) ok = true, want false")
	}

	// Removing the group is atomic too: every listed account must be a member.
	if err := accounts.UnregisterGroup(members, hedgeBook); err != nil {
		t.Fatalf("UnregisterGroup() error = %v", err)
	}
}

// Used in: pit.wiki/Dynamic-Policy-Reconfiguration.md - Retune a Built-in Policy
// This mirror is intentionally wider than the wiki snippet: it adds the test
// harness (t.Fatalf assertions, wikiExampleOrder) so the example runs. The
// snippet shows idiomatic return-error style; the mirror uses t.Fatalf for the
// same error paths. Keep the shared user-code flow in sync with the wiki.
func TestExampleWikiDynamicPolicyReconfigurationRateLimit(t *testing.T) {
	order := wikiExampleOrder(t, "1", "100")

	// Register the rate-limit policy through Builtin so the engine keeps a
	// handle to its settings; built-in policies are configurable by name.
	engine, err := NewEngineBuilder().
		NoSync().
		Builtin(
			policies.BuildRateLimit().BrokerBarrier(
				policies.RateLimitBrokerBarrier{
					Limit: policies.RateLimit{
						MaxOrders: 5,
						Window:    60 * time.Second,
					},
				},
			),
		).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	// The generous limit of 5 admits the first three orders.
	for i := 0; i < 3; i++ {
		reservation, rejects, err := engine.ExecutePreTrade(order)
		if err != nil {
			t.Fatalf("ExecutePreTrade() error = %v", err)
		}
		if rejects != nil {
			t.Fatalf("ExecutePreTrade() unexpected rejects: %v", rejects)
		}
		reservation.CommitAndClose()
	}

	// Tighten the broker limit to 2 at runtime, without rebuilding the engine.
	// Built-in policies register under their type name (policies.RateLimitPolicyName).
	err = engine.Configure().RateLimit(
		policies.RateLimitPolicyName,
		&policies.RateLimitBrokerBarrier{
			Limit: policies.RateLimit{MaxOrders: 2, Window: 60 * time.Second},
		},
		nil,
		nil,
		nil,
	)
	if err != nil {
		t.Fatalf("Configure().RateLimit() error = %v", err)
	}

	// The next order would have passed under the old limit of 5; the new limit
	// of 2 rejects it, proving the live policy reads the retuned value.
	_, rejects, err := engine.ExecutePreTrade(order)
	if err != nil {
		t.Fatalf("ExecutePreTrade() error = %v", err)
	}
	if len(rejects) != 1 {
		t.Fatalf("reject len = %d, want 1", len(rejects))
	}
	if rejects[0].Reason != "rate limit exceeded: broker barrier" {
		t.Fatalf("reject reason = %q, want %q",
			rejects[0].Reason, "rate limit exceeded: broker barrier")
	}
}

// --- Non-Mutating-Dry-Run: Custom Policy with Dry-Run Hooks ---

// MyCountingPolicy is a start-stage policy that counts real calls but
// provides a read-only dry-run variant that does not increment the counter.
type MyCountingPolicy struct {
	count int
}

func (*MyCountingPolicy) Close()                             {}
func (*MyCountingPolicy) Name() string                       { return "MyCountingPolicy" }
func (*MyCountingPolicy) PolicyGroupID() model.PolicyGroupID { return model.DefaultPolicyGroupID }

func (p *MyCountingPolicy) CheckPreTradeStart(
	_ pretrade.Context,
	_ model.Order,
) []reject.Reject {
	p.count++ // side effect: spends the real counter budget
	return nil
}

func (*MyCountingPolicy) PerformPreTradeCheck(
	_ pretrade.Context,
	_ model.Order,
	_ tx.Mutations,
	_ pretrade.Result,
) []reject.Reject {
	return nil
}

func (*MyCountingPolicy) ApplyExecutionReport(
	_ pretrade.PostTradeContext,
	_ model.ExecutionReport,
	_ pretrade.PostTradeAdjustments,
	_ pretrade.PostTradePnls,
) []reject.AccountBlock {
	return nil
}

func (*MyCountingPolicy) ApplyAccountAdjustment(
	_ accountadjustment.Context,
	_ param.AccountID,
	_ model.AccountAdjustment,
	_ tx.Mutations,
	_ pretrade.AccountOutcomes,
) (pretrade.PolicyAccountAdjustmentResult, []reject.Reject) {
	return pretrade.PolicyAccountAdjustmentResult{}, nil
}

// CheckPreTradeStartDryRun is the read-only variant: no counter increment.
func (*MyCountingPolicy) CheckPreTradeStartDryRun(
	_ pretrade.Context,
	_ model.Order,
) []reject.Reject {
	return nil // read current state without spending the budget
}

// PerformPreTradeCheckDryRun is the read-only variant.
func (*MyCountingPolicy) PerformPreTradeCheckDryRun(
	_ pretrade.Context,
	_ model.Order,
	_ tx.Mutations,
	_ pretrade.Result,
) []reject.Reject {
	return nil
}

// Used in: pit.wiki/Non-Mutating-Dry-Run.md - Read the Dry-Run Verdict
// This mirror is intentionally wider than the wiki snippet: it adds the test
// harness (t.Fatalf assertions, wikiExampleOrder) so the example runs. The
// snippet shows idiomatic return-error style; the mirror uses t.Fatalf for the
// same error paths. Keep the shared user-code flow in sync with the wiki.
func TestExampleWikiDryRunVerdict(t *testing.T) {
	order := wikiExampleOrder(t, "100", "185")

	engine, err := NewEngineBuilder().
		NoSync().
		Builtin(policies.BuildOrderValidation()).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	report, err := engine.ExecutePreTradeDryRun(order)
	if err != nil {
		t.Fatalf("ExecutePreTradeDryRun() error = %v", err)
	}
	defer report.Close()

	if report.IsPass() {
		fmt.Println("order would be admitted")
	} else {
		for _, r := range report.Rejects() {
			t.Logf(
				"would reject by %s [%d]: %s (%s)",
				r.Policy,
				r.Code,
				r.Reason,
				r.Details,
			)
		}
	}
	if !report.IsPass() {
		t.Fatalf("expected dry-run to pass, got rejects: %v", report.Rejects())
	}
}

// Used in: pit.wiki/Non-Mutating-Dry-Run.md - Use the Dry-Run Before a Real Call
// This mirror is intentionally wider than the wiki snippet: it adds the test
// harness (t.Fatalf assertions, wikiExampleOrder) so the example runs. The
// snippet shows idiomatic return-error style; the mirror uses t.Fatalf for the
// same error paths. Keep the shared user-code flow in sync with the wiki.
func TestExampleWikiDryRunBeforeRealCall(t *testing.T) {
	order := wikiExampleOrder(t, "100", "185")

	engine, err := NewEngineBuilder().
		NoSync().
		Builtin(policies.BuildOrderValidation()).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	// Probe without spending any budget or creating any reservation.
	probe, err := engine.ExecutePreTradeDryRun(order)
	if err != nil {
		t.Fatalf("ExecutePreTradeDryRun() error = %v", err)
	}
	defer probe.Close()

	if !probe.IsPass() {
		return // would have been rejected - skip the real call
	}

	// The real call now runs with fresh state; the probe had no effect.
	reservation, rejects, err := engine.ExecutePreTrade(order)
	if err != nil {
		t.Fatalf("ExecutePreTrade() error = %v", err)
	}
	if rejects != nil {
		t.Fatalf("ExecutePreTrade() unexpected rejects: %v", rejects)
	}
	defer reservation.Close()
	reservation.Commit()
}

// Used in: pit.wiki/Non-Mutating-Dry-Run.md - Read-Only Custom Start-Stage Hook
// This mirror is intentionally wider than the wiki snippet: it defines the
// full MyCountingPolicy type above (the snippet shows only the hook methods)
// and adds assertions proving the dry-run does not increment the counter.
// Keep the shared user-code flow in sync with the wiki.
func TestExampleWikiDryRunCustomPolicyHook(t *testing.T) {
	order := wikiExampleOrder(t, "10", "100")

	policy := &MyCountingPolicy{}
	engine, err := NewEngineBuilder().
		NoSync().
		PreTrade(policy).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	// Dry-run must not increment the counter.
	probe, err := engine.ExecutePreTradeDryRun(order)
	if err != nil {
		t.Fatalf("ExecutePreTradeDryRun() error = %v", err)
	}
	defer probe.Close()

	if !probe.IsPass() {
		t.Fatalf("dry-run unexpected reject")
	}
	if policy.count != 0 {
		t.Fatalf("dry-run incremented counter: want 0, got %d", policy.count)
	}

	// Real call increments.
	reservation, rejects, err := engine.ExecutePreTrade(order)
	if err != nil {
		t.Fatalf("ExecutePreTrade() error = %v", err)
	}
	if rejects != nil {
		t.Fatalf("ExecutePreTrade() unexpected rejects: %v", rejects)
	}
	reservation.CommitAndClose()
	if policy.count != 1 {
		t.Fatalf("real call counter: want 1, got %d", policy.count)
	}
}

// Used in: pit.wiki/Dynamic-Policy-Reconfiguration.md - Spot Funds: Global Limit Mode
// This mirror is intentionally wider than the wiki snippet: it adds the test
// harness (t.Fatalf assertions, wikiExampleOrder and wikiSeedBalanceAdjustment)
// so the example runs. Keep the shared user-code flow in sync with the wiki.
func TestExampleWikiSpotFundsGlobalLimitMode(t *testing.T) {
	engine, err := NewEngineBuilder().
		NoSync().
		Builtin(policies.BuildSpotFunds()).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	accountID := param.NewAccountIDFromUint64(99224416)
	// Seed 1 000 USD - not enough for 10 AAPL @ 200 (= 2 000 notional).
	seed := wikiSeedBalanceAdjustment(t, "1000")
	if result, err := engine.ApplyAccountAdjustment(
		accountID, []model.AccountAdjustment{seed},
	); err != nil {
		t.Fatalf("ApplyAccountAdjustment() error = %v", err)
	} else if result.BatchError.IsSet() {
		t.Fatalf("ApplyAccountAdjustment() unexpected rejects: %v", result.BatchError)
	}

	order := wikiExampleOrder(t, "10", "200")

	// Default Enforce: 2 000 notional exceeds 1 000 available - rejected.
	_, rejects, err := engine.ExecutePreTrade(order)
	if err != nil {
		t.Fatalf("ExecutePreTrade() error = %v", err)
	}
	if len(rejects) == 0 {
		t.Fatalf("ExecutePreTrade(): expected InsufficientFunds rejection")
	}
	if rejects[0].Reason != "spot funds insufficient" {
		t.Fatalf("Reason = %q, want %q", rejects[0].Reason, "spot funds insufficient")
	}

	// Switch to TrackOnly: the same order now passes and reserves against deficit.
	if err := engine.Configure().SpotFundsGlobalLimitMode(
		policies.SpotFundsPolicyName,
		policies.SpotFundsLimitModeTrackOnly,
	); err != nil {
		t.Fatalf("SpotFundsGlobalLimitMode(TrackOnly) error = %v", err)
	}
	reservation, rejects, err := engine.ExecutePreTrade(order)
	if err != nil {
		t.Fatalf("ExecutePreTrade() error = %v", err)
	}
	if rejects != nil {
		t.Fatalf("ExecutePreTrade() unexpected rejects after TrackOnly: %v", rejects)
	}
	reservation.CommitAndClose() // available: 1 000 - 2 000 = -1 000

	// Restore Enforce: available is negative - still rejected.
	if err := engine.Configure().SpotFundsGlobalLimitMode(
		policies.SpotFundsPolicyName,
		policies.SpotFundsLimitModeEnforce,
	); err != nil {
		t.Fatalf("SpotFundsGlobalLimitMode(Enforce) error = %v", err)
	}
	_, rejects, err = engine.ExecutePreTrade(order)
	if err != nil {
		t.Fatalf("ExecutePreTrade() error = %v", err)
	}
	if len(rejects) == 0 {
		t.Fatalf("ExecutePreTrade(): expected InsufficientFunds rejection after Enforce restore")
	}
	if rejects[0].Reason != "spot funds insufficient" {
		t.Fatalf("Reason = %q, want %q", rejects[0].Reason, "spot funds insufficient")
	}
}

// Used in: pit.wiki/Dynamic-Policy-Reconfiguration.md - Spot Funds: Per-Account Limit Mode
// This mirror is intentionally wider than the wiki snippet: it adds the test
// harness (t.Fatalf assertions, wikiExampleOrder and wikiSeedBalanceAdjustment)
// so the example runs. Keep the shared user-code flow in sync with the wiki.
func TestExampleWikiSpotFundsPerAccountLimitMode(t *testing.T) {
	engine, err := NewEngineBuilder().
		NoSync().
		Builtin(policies.BuildSpotFunds()).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	accountID := param.NewAccountIDFromUint64(99224416)
	// Seed 1 000 USD - not enough for 10 AAPL @ 200 (= 2 000 notional).
	seed := wikiSeedBalanceAdjustment(t, "1000")
	if result, err := engine.ApplyAccountAdjustment(
		accountID, []model.AccountAdjustment{seed},
	); err != nil {
		t.Fatalf("ApplyAccountAdjustment() error = %v", err)
	} else if result.BatchError.IsSet() {
		t.Fatalf("ApplyAccountAdjustment() unexpected rejects: %v", result.BatchError)
	}

	order := wikiExampleOrder(t, "10", "200")

	// Global Enforce: under-funded buy is rejected.
	_, rejects, err := engine.ExecutePreTrade(order)
	if err != nil {
		t.Fatalf("ExecutePreTrade() error = %v", err)
	}
	if len(rejects) == 0 {
		t.Fatalf("ExecutePreTrade(): expected InsufficientFunds rejection")
	}
	if rejects[0].Reason != "spot funds insufficient" {
		t.Fatalf("Reason = %q, want %q", rejects[0].Reason, "spot funds insufficient")
	}

	// Pin this account to TrackOnly: per-account override wins over global Enforce.
	if err := engine.Configure().SpotFundsAccountLimitMode(
		policies.SpotFundsPolicyName,
		accountID,
		optional.Some(policies.SpotFundsLimitModeTrackOnly),
	); err != nil {
		t.Fatalf("SpotFundsAccountLimitMode(Some(TrackOnly)) error = %v", err)
	}
	reservation, rejects, err := engine.ExecutePreTrade(order)
	if err != nil {
		t.Fatalf("ExecutePreTrade() error = %v", err)
	}
	if rejects != nil {
		t.Fatalf("ExecutePreTrade() unexpected rejects after per-account TrackOnly: %v", rejects)
	}
	reservation.CommitAndClose() // reservation recorded despite insufficient funds

	// Clear the per-account override: cascade falls back to global Enforce.
	if err := engine.Configure().SpotFundsAccountLimitMode(
		policies.SpotFundsPolicyName,
		accountID,
		optional.None[policies.SpotFundsLimitMode](),
	); err != nil {
		t.Fatalf("SpotFundsAccountLimitMode(None) error = %v", err)
	}
	_, rejects, err = engine.ExecutePreTrade(order)
	if err != nil {
		t.Fatalf("ExecutePreTrade() error = %v", err)
	}
	if len(rejects) == 0 {
		t.Fatalf("ExecutePreTrade(): expected InsufficientFunds rejection after pin cleared")
	}
	if rejects[0].Reason != "spot funds insufficient" {
		t.Fatalf("Reason = %q, want %q", rejects[0].Reason, "spot funds insufficient")
	}
}

// Used in: pit.wiki/Dynamic-Policy-Reconfiguration.md - Force-set Accumulated P&L
// This mirror is intentionally wider than the wiki snippet: it adds the test
// harness (t.Fatalf assertions, wikiExampleOrder and the account it carries) so
// the example runs. The snippet shows idiomatic return-error style; the mirror
// uses t.Fatalf for the same error paths. Keep the shared user-code flow in
// sync with the wiki.
func TestExampleWikiDynamicPolicyReconfigurationSetAccountPnl(t *testing.T) {
	order := wikiExampleOrder(t, "1", "100")
	account := param.NewAccountIDFromUint64(99224416)

	usd, err := param.NewAsset("USD")
	if err != nil {
		t.Fatalf("NewAsset(USD) error = %v", err)
	}
	lowerBound, err := param.NewPnlFromString("-100")
	if err != nil {
		t.Fatalf("NewPnlFromString(-100) error = %v", err)
	}

	// Register the kill-switch policy through Builtin so the engine keeps a
	// handle to its accumulator; built-in policies are configurable by name.
	engine, err := NewEngineBuilder().
		NoSync().
		Builtin(
			policies.BuildPnlBoundsKillSwitch().BrokerBarriers(
				policies.PnlBoundsBrokerBarrier{
					SettlementAsset: usd,
					LowerBound:      optional.Some(lowerBound),
				},
			),
		).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	// With no P&L history the order passes against the lower bound of -100.
	reservation, rejects, err := engine.ExecutePreTrade(order)
	if err != nil {
		t.Fatalf("ExecutePreTrade() error = %v", err)
	}
	if rejects != nil {
		t.Fatalf("ExecutePreTrade() unexpected rejects: %v", rejects)
	}
	reservation.CommitAndClose()

	// Force-set the account's accumulated P&L to -150 USD, below the bound.
	// Built-in policies register under their type name (policies.PnlBoundsKillSwitchPolicyName).
	forced, err := param.NewPnlFromString("-150")
	if err != nil {
		t.Fatalf("NewPnlFromString(-150) error = %v", err)
	}
	err = engine.Configure().SetAccountPnl(
		policies.PnlBoundsKillSwitchPolicyName,
		account,
		usd,
		forced,
	)
	if err != nil {
		t.Fatalf("Configure().SetAccountPnl() error = %v", err)
	}

	// The next order for that account breaches the lower bound and is rejected;
	// the breach also latches an engine-level block on the account.
	_, rejects, err = engine.ExecutePreTrade(order)
	if err != nil {
		t.Fatalf("ExecutePreTrade() error = %v", err)
	}
	if len(rejects) != 1 {
		t.Fatalf("reject len = %d, want 1", len(rejects))
	}
	if rejects[0].Reason != "pnl kill switch triggered: broker barrier" {
		t.Fatalf("reject reason = %q, want %q",
			rejects[0].Reason, "pnl kill switch triggered: broker barrier")
	}
}
