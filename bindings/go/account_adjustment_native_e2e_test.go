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
	"go.openpit.dev/openpit/pkg/optional"
	"go.openpit.dev/openpit/pretrade"
	"go.openpit.dev/openpit/reject"
	"go.openpit.dev/openpit/tx"
)

func TestAccountAdjustmentNativeE2E_BatchAppliesAndInvokesPolicyPerItem(t *testing.T) {
	policy := &accountAdjustmentCountingPolicy{name: "count-adjustments"}

	engine, err := NewEngineBuilder().FullSync().PreTrade(policy).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	first, err := model.NewAccountAdjustmentFromValues(
		model.AccountAdjustmentValues{
			BalanceOperation: optional.Some(
				model.NewAccountAdjustmentBalanceOperationFromValues(
					model.AccountAdjustmentBalanceOperationValues{
						Asset:             optional.Some(mustAdjustmentNativeAsset(t, "USD")),
						AverageEntryPrice: optional.Some(mustAdjustmentNativePrice(t, "101.5")),
					},
				),
			),
			Amount: optional.Some(
				model.NewAccountAdjustmentAmountFromValues(
					model.AccountAdjustmentAmountValues{
						Balance: optional.Some(
							param.NewDeltaAdjustmentAmount(
								mustAdjustmentNativePositionSize(t, "10"),
							),
						),
					},
				),
			),
		},
	)
	if err != nil {
		t.Fatalf("NewAccountAdjustmentFromValues(first) error = %v", err)
	}

	second, err := model.NewAccountAdjustmentFromValues(
		model.AccountAdjustmentValues{
			PositionOperation: optional.Some(
				model.NewAccountAdjustmentPositionOperationFromValues(
					model.AccountAdjustmentPositionOperationValues{
						Instrument: optional.Some(
							param.NewInstrument(
								mustAdjustmentNativeAsset(t, "AAPL"),
								mustAdjustmentNativeAsset(t, "USD"),
							),
						),
						CollateralAsset: optional.Some(mustAdjustmentNativeAsset(t, "USD")),
						AverageEntryPrice: optional.Some(
							mustAdjustmentNativePrice(t, "102.25"),
						),
						Leverage: optional.Some(param.NewLeverageFromUint16(4)),
						Mode:     optional.Some(param.PositionModeHedged),
					},
				),
			),
			Bounds: optional.Some(
				model.NewAccountAdjustmentBoundsFromValues(
					model.AccountAdjustmentBoundsValues{
						BalanceUpper:  optional.Some(mustAdjustmentNativePositionSize(t, "100")),
						BalanceLower:  optional.Some(mustAdjustmentNativePositionSize(t, "20")),
						IncomingUpper: optional.Some(mustAdjustmentNativePositionSize(t, "50")),
						IncomingLower: optional.Some(mustAdjustmentNativePositionSize(t, "5")),
					},
				),
			),
		},
	)
	if err != nil {
		t.Fatalf("NewAccountAdjustmentFromValues(second) error = %v", err)
	}

	result, err := engine.ApplyAccountAdjustment(
		param.NewAccountIDFromUint64(77),
		[]model.AccountAdjustment{first, second},
	)
	if err != nil {
		t.Fatalf("ApplyAccountAdjustment() error = %v", err)
	}
	if result.BatchError.IsSet() {
		t.Fatalf("ApplyAccountAdjustment() rejects = %v, want none", result.BatchError)
	}
	if policy.calls != 2 {
		t.Fatalf("policy calls = %d, want 2", policy.calls)
	}
}

func TestAccountAdjustmentNativeE2E_AccountPnlStateReachesPolicy(t *testing.T) {
	policy := &accountAdjustmentPnlStatePolicy{name: "observe-account-pnl"}

	engine, err := NewEngineBuilder().FullSync().PreTrade(policy).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	want := mustAdjustmentNativePnl(t, "42.5")
	adjustment, err := model.NewAccountAdjustmentFromValues(
		model.AccountAdjustmentValues{
			AccountPnlOperation: optional.Some(
				model.NewAccountAdjustmentAccountPnlOperation(model.NewPnlState(want)),
			),
		},
	)
	if err != nil {
		t.Fatalf("NewAccountAdjustmentFromValues() error = %v", err)
	}

	result, err := engine.ApplyAccountAdjustment(
		param.NewAccountIDFromUint64(91),
		[]model.AccountAdjustment{adjustment},
	)
	if err != nil {
		t.Fatalf("ApplyAccountAdjustment() error = %v", err)
	}
	if result.BatchError.IsSet() {
		t.Fatalf("ApplyAccountAdjustment() rejects = %v, want none", result.BatchError)
	}
	if !policy.sawPnlState {
		t.Fatal("policy did not observe the account-PnL state input")
	}
	if len(result.Outcomes) != 0 {
		t.Fatalf("outcomes len = %d, want 0", len(result.Outcomes))
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
}

// accountAdjustmentPnlStatePolicy observes an account-wide PnL state from the
// incoming adjustment, proving the typed input reaches custom policies.
type accountAdjustmentPnlStatePolicy struct {
	name        string
	sawPnlState bool
}

func (accountAdjustmentPnlStatePolicy) Close() {}

func (p accountAdjustmentPnlStatePolicy) Name() string {
	return p.name
}

func (accountAdjustmentPnlStatePolicy) PolicyGroupID() model.PolicyGroupID {
	return model.DefaultPolicyGroupID
}

func (accountAdjustmentPnlStatePolicy) CheckPreTradeStart(
	pretrade.Context,
	model.Order,
) []reject.Reject {
	return nil
}

func (accountAdjustmentPnlStatePolicy) PerformPreTradeCheck(
	pretrade.Context,
	model.Order,
	tx.Mutations,
	pretrade.Result,
) []reject.Reject {
	return nil
}

func (accountAdjustmentPnlStatePolicy) ApplyExecutionReport(
	_ pretrade.PostTradeContext,
	_ model.ExecutionReport,
	_ pretrade.PostTradeAdjustments,
	_ pretrade.PostTradePnls,
) []reject.AccountBlock {
	return nil
}

func (p *accountAdjustmentPnlStatePolicy) ApplyAccountAdjustment(
	_ accountadjustment.Context,
	_ param.AccountID,
	adjustment model.AccountAdjustment,
	_ tx.Mutations,
	_ pretrade.AccountOutcomes,
) (pretrade.PolicyAccountAdjustmentResult, []reject.Reject) {
	operation, ok := adjustment.AccountPnlOperation().Get()
	if !ok {
		return pretrade.PolicyAccountAdjustmentResult{}, nil
	}
	if _, ok := operation.State().Value(); !ok {
		return pretrade.PolicyAccountAdjustmentResult{}, nil
	}
	p.sawPnlState = true
	return pretrade.PolicyAccountAdjustmentResult{
		AccountBlocks: []reject.AccountBlock{
			reject.NewAccountBlock(
				reject.CodePnlKillSwitchTriggered,
				p.name,
				"account PnL halted",
				"custom policy accepted the adjustment and blocked the account",
			),
		},
	}, nil
}

func mustAdjustmentNativePnl(t *testing.T, source string) param.Pnl {
	t.Helper()
	value, err := param.NewPnlFromString(source)
	if err != nil {
		t.Fatalf("NewPnlFromString(%q) error = %v", source, err)
	}
	return value
}

type accountAdjustmentCountingPolicy struct {
	name  string
	calls int
}

func (accountAdjustmentCountingPolicy) Close() {}

func (p accountAdjustmentCountingPolicy) Name() string {
	return p.name
}

func (accountAdjustmentCountingPolicy) PolicyGroupID() model.PolicyGroupID {
	return model.DefaultPolicyGroupID
}

func (accountAdjustmentCountingPolicy) CheckPreTradeStart(
	pretrade.Context,
	model.Order,
) []reject.Reject {
	return nil
}

func (accountAdjustmentCountingPolicy) PerformPreTradeCheck(
	pretrade.Context,
	model.Order,
	tx.Mutations,
	pretrade.Result,
) []reject.Reject {
	return nil
}

func (accountAdjustmentCountingPolicy) ApplyExecutionReport(
	_ pretrade.PostTradeContext,
	_ model.ExecutionReport,
	_ pretrade.PostTradeAdjustments,
	_ pretrade.PostTradePnls,
) []reject.AccountBlock {
	return nil
}

func (p *accountAdjustmentCountingPolicy) ApplyAccountAdjustment(
	accountadjustment.Context,
	param.AccountID,
	model.AccountAdjustment,
	tx.Mutations,
	pretrade.AccountOutcomes,
) (pretrade.PolicyAccountAdjustmentResult, []reject.Reject) {
	p.calls++
	return pretrade.PolicyAccountAdjustmentResult{}, nil
}

func mustAdjustmentNativePrice(t *testing.T, value string) param.Price {
	t.Helper()
	v, err := param.NewPriceFromString(value)
	if err != nil {
		t.Fatalf("NewPriceFromString(%q) error = %v", value, err)
	}
	return v
}

func mustAdjustmentNativeAsset(t *testing.T, value string) param.Asset {
	t.Helper()
	asset, err := param.NewAsset(value)
	if err != nil {
		t.Fatalf("NewAsset(%q) error = %v", value, err)
	}
	return asset
}

func mustAdjustmentNativePositionSize(t *testing.T, value string) param.PositionSize {
	t.Helper()
	v, err := param.NewPositionSizeFromString(value)
	if err != nil {
		t.Fatalf("NewPositionSizeFromString(%q) error = %v", value, err)
	}
	return v
}
