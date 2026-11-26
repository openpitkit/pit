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

// Package openpit provides the Go binding for the OpenPit pre-trade risk engine.
package openpit

import (
	"testing"

	"go.openpit.dev/openpit/accountadjustment"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pretrade"
	"go.openpit.dev/openpit/reject"
	"go.openpit.dev/openpit/tx"
)

func TestCustomPolicyNativeE2E_ProducesPostTradeResult(t *testing.T) {
	account := param.NewAccountIDFromUint64(99224416)
	currency := mustAdjustmentNativeAsset(t, "USD")
	computed := accountadjustment.NewAccountPnlOutcome(
		17,
		account,
		accountadjustment.PnlOutcomeAmount{
			Delta:    mustAdjustmentNativePnl(t, "1.25"),
			Absolute: mustAdjustmentNativePnl(t, "7.5"),
		},
	)
	halted, err := accountadjustment.NewAccountPnlHaltedOutcome(
		17,
		account,
		model.PnlHaltReasonMissingAccountCurrency,
	)
	if err != nil {
		t.Fatalf("NewAccountPnlHaltedOutcome() error = %v", err)
	}
	policy := &postTradeResultPolicy{
		result: pretrade.PostTradeResult{
			AccountBlocks: []reject.AccountBlock{
				reject.NewAccountBlock(
					reject.CodePnlKillSwitchTriggered,
					"post-trade-result",
					"PnL limit exceeded",
					"account PnL crossed the configured limit",
				),
			},
			AccountPnls: []accountadjustment.AccountPnlOutcome{
				computed,
				halted,
			},
			AccountAdjustments: []accountadjustment.Outcome{
				{
					PolicyGroupID: 17,
					Entry: accountadjustment.AccountOutcomeEntry{
						Asset: currency,
					},
				},
			},
		},
	}
	engine, err := NewEngineBuilder().FullSync().PreTrade(policy).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	result, err := engine.ApplyExecutionReport(model.NewExecutionReport())
	if err != nil {
		t.Fatalf("ApplyExecutionReport() error = %v", err)
	}
	if len(result.AccountBlocks) != 1 {
		t.Fatalf("AccountBlocks len = %d, want 1", len(result.AccountBlocks))
	}
	if result.AccountBlocks[0].Code != reject.CodePnlKillSwitchTriggered {
		t.Fatalf(
			"AccountBlocks[0].Code = %v, want %v",
			result.AccountBlocks[0].Code,
			reject.CodePnlKillSwitchTriggered,
		)
	}
	if len(result.AccountAdjustments) != 1 {
		t.Fatalf(
			"AccountAdjustments len = %d, want 1",
			len(result.AccountAdjustments),
		)
	}
	gotAdjustment := result.AccountAdjustments[0]
	if gotAdjustment.PolicyGroupID != 17 || !gotAdjustment.Entry.Asset.Equal(currency) {
		t.Fatalf(
			"AccountAdjustments[0] = (%d, %v), want (17, USD)",
			gotAdjustment.PolicyGroupID,
			gotAdjustment.Entry.Asset,
		)
	}
	if len(result.AccountPnls) != 2 {
		t.Fatalf("AccountPnls len = %d, want 2", len(result.AccountPnls))
	}
	gotComputed := result.AccountPnls[0]
	if gotComputed.PolicyGroupID != 17 || gotComputed.AccountID != account {
		t.Fatalf(
			"computed identity = (%d, %v), want (17, %v)",
			gotComputed.PolicyGroupID,
			gotComputed.AccountID,
			account,
		)
	}
	gotAmount, ok := gotComputed.Amount()
	if !ok || !gotAmount.Absolute.Equal(mustAdjustmentNativePnl(t, "7.5")) {
		t.Fatalf("computed Amount() = (%v, %v)", gotAmount, ok)
	}
	gotHalted := result.AccountPnls[1]
	gotReason, ok := gotHalted.HaltReason()
	if !ok || gotReason != model.PnlHaltReasonMissingAccountCurrency {
		t.Fatalf("halted HaltReason() = (%v, %v)", gotReason, ok)
	}
}

func TestCustomPolicyNativeE2E_InvalidPostTradeResultReturnsCallbackBlock(t *testing.T) {
	policy := &postTradeResultPolicy{
		result: pretrade.PostTradeResult{
			AccountBlocks: []reject.AccountBlock{
				reject.NewAccountBlock(
					reject.CodeAccountBlocked,
					"post-trade-result",
					"existing block",
					"added before the invalid outcome",
				),
			},
			AccountAdjustments: []accountadjustment.Outcome{
				{
					PolicyGroupID: 17,
					Entry:         accountadjustment.AccountOutcomeEntry{},
				},
			},
		},
	}
	engine, err := NewEngineBuilder().FullSync().PreTrade(policy).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	result, err := engine.ApplyExecutionReport(model.NewExecutionReport())
	if err != nil {
		t.Fatalf("ApplyExecutionReport() error = %v", err)
	}
	if len(result.AccountBlocks) != 2 {
		t.Fatalf("AccountBlocks len = %d, want 2", len(result.AccountBlocks))
	}
	if result.AccountBlocks[0].Code != reject.CodeAccountBlocked {
		t.Fatalf(
			"AccountBlocks[0].Code = %v, want %v",
			result.AccountBlocks[0].Code,
			reject.CodeAccountBlocked,
		)
	}
	fallback := result.AccountBlocks[1]
	if fallback.Code != reject.CodeSystemUnavailable ||
		fallback.Policy != "openpit.callback" ||
		fallback.Reason != "custom policy callback failed" ||
		fallback.Details == "" {
		t.Fatalf("callback fallback block = %#v", fallback)
	}
	if len(result.AccountAdjustments) != 0 {
		t.Fatalf(
			"AccountAdjustments len = %d, want 0",
			len(result.AccountAdjustments),
		)
	}
}

type postTradeResultPolicy struct {
	result      pretrade.PostTradeResult
	preAccount  func(param.AccountID, error)
	postAccount func(param.AccountID, error)
}

func (*postTradeResultPolicy) Close() {}

func (*postTradeResultPolicy) Name() string { return "post-trade-result" }

func (*postTradeResultPolicy) PolicyGroupID() model.PolicyGroupID { return 17 }

func (p *postTradeResultPolicy) CheckPreTradeStart(
	ctx pretrade.Context,
	_ model.Order,
) []reject.Reject {
	if p.preAccount != nil {
		p.preAccount(ctx.AccountID())
	}
	return nil
}

func (*postTradeResultPolicy) PerformPreTradeCheck(
	pretrade.Context,
	model.Order,
	tx.Mutations,
	pretrade.Result,
) []reject.Reject {
	return nil
}

func (p *postTradeResultPolicy) ApplyExecutionReport(
	ctx pretrade.PostTradeContext,
	_ model.ExecutionReport,
	adjustments pretrade.PostTradeAdjustments,
	pnls pretrade.PostTradePnls,
) []reject.AccountBlock {
	if p.postAccount != nil {
		p.postAccount(ctx.AccountID())
	}
	blocks := append([]reject.AccountBlock(nil), p.result.AccountBlocks...)
	for _, pnl := range p.result.AccountPnls {
		if err := pnls.Push(pnl); err != nil {
			return append(blocks, reject.NewAccountBlock(
				reject.CodeSystemUnavailable,
				"openpit.callback",
				"custom policy callback failed",
				err.Error(),
			))
		}
	}
	for _, adjustment := range p.result.AccountAdjustments {
		if err := adjustments.Push(adjustment.PolicyGroupID, adjustment.Entry); err != nil {
			return append(blocks, reject.NewAccountBlock(
				reject.CodeSystemUnavailable,
				"openpit.callback",
				"custom policy callback failed",
				err.Error(),
			))
		}
	}
	return blocks
}

func TestCustomPolicyNativeE2E_OperationAccountContexts(t *testing.T) {
	account := param.NewAccountIDFromUint64(42)
	preReads, postReads := 0, 0
	missing := false
	check := func(got param.AccountID, err error) {
		if missing {
			if err == nil {
				t.Error("unreadable operation account must return an explicit error")
			}
			return
		}
		if err != nil || got != account {
			t.Errorf("operation account = %v, %v; want %v", got, err, account)
		}
	}
	policy := &postTradeResultPolicy{
		preAccount:  func(got param.AccountID, err error) { preReads++; check(got, err) },
		postAccount: func(got param.AccountID, err error) { postReads++; check(got, err) },
	}
	engine, err := NewEngineBuilder().FullSync().PreTrade(policy).Build()
	if err != nil {
		t.Fatal(err)
	}
	defer engine.Stop()
	for _, absent := range []bool{false, true} {
		missing = absent
		order := model.NewOrder()
		report := model.NewExecutionReport()
		if !absent {
			op := model.NewOrderOperation()
			op.SetAccountID(account)
			order.SetOperation(op)
			reportOp := model.NewExecutionReportOperation()
			reportOp.SetAccountID(account)
			report.SetOperation(reportOp)
		}
		request, rejects, err := engine.StartPreTrade(order)
		if err != nil || len(rejects) != 0 {
			t.Fatalf("start = %v, %v", rejects, err)
		}
		if request != nil {
			request.Close()
		}
		if _, err := engine.ApplyExecutionReport(report); err != nil {
			t.Fatal(err)
		}
	}
	if preReads != 2 || postReads != 2 {
		t.Fatalf("callback reads = %d, %d; want 2, 2", preReads, postReads)
	}
}

func (*postTradeResultPolicy) ApplyAccountAdjustment(
	accountadjustment.Context,
	param.AccountID,
	model.AccountAdjustment,
	tx.Mutations,
	pretrade.AccountOutcomes,
) (pretrade.PolicyAccountAdjustmentResult, []reject.Reject) {
	return pretrade.PolicyAccountAdjustmentResult{}, nil
}
