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

	rejects, _, err := engine.ApplyAccountAdjustment(
		param.NewAccountIDFromUint64(77),
		[]model.AccountAdjustment{first, second},
	)
	if err != nil {
		t.Fatalf("ApplyAccountAdjustment() error = %v", err)
	}
	if rejects.IsSet() {
		t.Fatalf("ApplyAccountAdjustment() rejects = %v, want none", rejects)
	}
	if policy.calls != 2 {
		t.Fatalf("policy calls = %d, want 2", policy.calls)
	}
}

func TestAccountAdjustmentNativeE2E_BalanceRealizedPnlReachesPolicyAndSurfacesInOutcome(t *testing.T) {
	policy := &accountAdjustmentRealizedPnlEchoPolicy{name: "echo-realized-pnl"}

	engine, err := NewEngineBuilder().FullSync().PreTrade(policy).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	want := mustAdjustmentNativePnl(t, "42.5")
	adjustment, err := model.NewAccountAdjustmentFromValues(
		model.AccountAdjustmentValues{
			BalanceOperation: optional.Some(
				model.NewAccountAdjustmentBalanceOperationFromValues(
					model.AccountAdjustmentBalanceOperationValues{
						Asset:       optional.Some(mustAdjustmentNativeAsset(t, "USD")),
						RealizedPnl: optional.Some(want),
					},
				),
			),
		},
	)
	if err != nil {
		t.Fatalf("NewAccountAdjustmentFromValues() error = %v", err)
	}

	rejects, outcomes, err := engine.ApplyAccountAdjustment(
		param.NewAccountIDFromUint64(91),
		[]model.AccountAdjustment{adjustment},
	)
	if err != nil {
		t.Fatalf("ApplyAccountAdjustment() error = %v", err)
	}
	if rejects.IsSet() {
		t.Fatalf("ApplyAccountAdjustment() rejects = %v, want none", rejects)
	}
	if !policy.sawRealizedPnl {
		t.Fatal("policy did not observe the balance-operation realized PnL input")
	}
	if policy.pushErr != nil {
		t.Fatalf("outcomes.Push() error = %v", policy.pushErr)
	}
	if len(outcomes) != 1 {
		t.Fatalf("outcomes len = %d, want 1", len(outcomes))
	}

	got, ok := outcomes[0].Entry.RealizedPnl.Get()
	if !ok {
		t.Fatal("outcome RealizedPnl is unset, want force-set value")
	}
	assertNativePnlEqual(t, got.Absolute, want)
	assertNativePnlEqual(t, got.Delta, want)
}

// accountAdjustmentRealizedPnlEchoPolicy reads the balance-operation realized
// PnL force-set from the incoming adjustment and re-emits it as an outcome,
// proving the input reaches the engine and surfaces on the outcome side.
type accountAdjustmentRealizedPnlEchoPolicy struct {
	name           string
	sawRealizedPnl bool
	pushErr        error
}

func (accountAdjustmentRealizedPnlEchoPolicy) Close() {}

func (p accountAdjustmentRealizedPnlEchoPolicy) Name() string {
	return p.name
}

func (accountAdjustmentRealizedPnlEchoPolicy) PolicyGroupID() model.PolicyGroupID {
	return model.DefaultPolicyGroupID
}

func (accountAdjustmentRealizedPnlEchoPolicy) CheckPreTradeStart(
	pretrade.Context,
	model.Order,
) []reject.Reject {
	return nil
}

func (accountAdjustmentRealizedPnlEchoPolicy) PerformPreTradeCheck(
	pretrade.Context,
	model.Order,
	tx.Mutations,
	pretrade.Result,
) []reject.Reject {
	return nil
}

func (accountAdjustmentRealizedPnlEchoPolicy) ApplyExecutionReport(
	_ pretrade.PostTradeContext,
	_ model.ExecutionReport,
	_ pretrade.PostTradeAdjustments,
) []reject.AccountBlock {
	return nil
}

func (p *accountAdjustmentRealizedPnlEchoPolicy) ApplyAccountAdjustment(
	_ accountadjustment.Context,
	_ param.AccountID,
	adjustment model.AccountAdjustment,
	_ tx.Mutations,
	outcomes pretrade.AccountOutcomes,
) []reject.Reject {
	operation, ok := adjustment.BalanceOperation().Get()
	if !ok {
		return nil
	}
	realizedPnl, ok := operation.RealizedPnl().Get()
	if !ok {
		return nil
	}
	asset, ok := operation.Asset().Get()
	if !ok {
		return nil
	}
	p.sawRealizedPnl = true
	p.pushErr = outcomes.Push(accountadjustment.AccountOutcomeEntry{
		Asset: asset,
		RealizedPnl: optional.Some(accountadjustment.PnlOutcomeAmount{
			Delta:    realizedPnl,
			Absolute: realizedPnl,
		}),
	})
	return nil
}

func mustAdjustmentNativePnl(t *testing.T, source string) param.Pnl {
	t.Helper()
	value, err := param.NewPnlFromString(source)
	if err != nil {
		t.Fatalf("NewPnlFromString(%q) error = %v", source, err)
	}
	return value
}

func assertNativePnlEqual(t *testing.T, got, want param.Pnl) {
	t.Helper()
	if !got.Equal(want) {
		t.Fatalf("Pnl = %s, want %s", got.String(), want.String())
	}
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
) []reject.AccountBlock {
	return nil
}

func (p *accountAdjustmentCountingPolicy) ApplyAccountAdjustment(
	accountadjustment.Context,
	param.AccountID,
	model.AccountAdjustment,
	tx.Mutations,
	pretrade.AccountOutcomes,
) []reject.Reject {
	p.calls++
	return nil
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
