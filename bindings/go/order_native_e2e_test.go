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
	"testing"

	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/optional"
)

func TestOrderNativeE2E_ExecuteAndApplyExecutionReport(t *testing.T) {
	engine := newEngineForOrderNativeE2ETest(t)
	defer engine.Stop()

	order := model.NewOrderFromValues(
		model.OrderValues{
			Operation: optional.Some(
				model.NewOrderOperationFromValues(
					model.OrderOperationParams{
						TradeAmount: optional.Some(
							param.NewQuantityTradeAmount(mustOrderNativeQuantity(t, "2")),
						),
						Instrument: optional.Some(
							param.NewInstrument(param.NewAsset("AAPL"), param.NewAsset("USD")),
						),
						Price:     optional.Some(mustOrderNativePrice(t, "182.50")),
						AccountID: optional.Some(param.NewAccountIDFromInt(9001)),
						Side:      optional.Some(param.SideBuy),
					},
				),
			),
			Position: optional.Some(
				model.NewOrderPositionFromValues(
					model.OrderPositionValues{
						Side:          optional.Some(param.PositionSideLong),
						ReduceOnly:    optional.BoolSome(false),
						ClosePosition: optional.BoolSome(false),
					},
				),
			),
			Margin: optional.Some(
				model.NewOrderMarginFromValues(
					model.OrderMarginValues{
						CollateralAsset: optional.Some(param.NewAsset("USD")),
						AutoBorrow:      optional.BoolSome(true),
						Leverage:        param.NewLeverageFromInt(3).Native(),
					},
				),
			),
		},
	)

	reservation, rejects, err := engine.ExecutePreTrade(order)
	if err != nil {
		t.Fatalf("ExecutePreTrade() error = %v", err)
	}
	if rejects != nil {
		t.Fatalf("ExecutePreTrade() rejects = %v, want nil", rejects)
	}
	if reservation == nil {
		t.Fatal("ExecutePreTrade() reservation = nil, want non-nil")
	}
	reservation.CommitAndClose()

	report := model.NewExecutionReportFromValues(
		model.ExecutionReportValues{
			Operation: optional.Some(
				model.NewExecutionReportOperationFromValues(
					model.ExecutionReportOperationValues{
						Instrument: optional.Some(
							param.NewInstrument(param.NewAsset("AAPL"), param.NewAsset("USD")),
						),
						AccountID: optional.Some(param.NewAccountIDFromInt(9001)),
						Side:      optional.Some(param.SideBuy),
					},
				),
			),
			FinancialImpact: optional.Some(
				model.NewExecutionReportFinancialImpactFromValues(
					model.ExecutionReportFinancialImpactValues{
						Pnl: optional.Some(mustOrderNativePnl(t, "1.25")),
						Fee: optional.Some(mustOrderNativeFee(t, "0.05")),
					},
				),
			),
			Fill: optional.Some(
				model.NewExecutionReportFillFromValues(
					model.ExecutionReportFillValues{
						LastTrade: optional.Some(
							model.NewExecutionReportTrade(
								mustOrderNativePrice(t, "182.50"),
								mustOrderNativeQuantity(t, "2"),
							),
						),
						LeavesQuantity: optional.Some(mustOrderNativeQuantity(t, "0")),
						LockPrice:      optional.Some(mustOrderNativePrice(t, "182.40")),
						Terminal:       true,
					},
				),
			),
			PositionImpact: optional.Some(
				model.NewExecutionReportPositionImpactFromValues(
					model.ExecutionReportPositionImpactValues{
						PositionEffect: optional.Some(param.PositionEffectOpen),
						PositionSide:   optional.Some(param.PositionSideLong),
					},
				),
			),
		},
	)

	result, err := engine.ApplyExecutionReport(report)
	if err != nil {
		t.Fatalf("ApplyExecutionReport() error = %v", err)
	}
	if result.KillSwitchTriggered {
		t.Fatal("ApplyExecutionReport().KillSwitchTriggered = true, want false")
	}
}

func mustOrderNativePrice(t *testing.T, value string) param.Price {
	t.Helper()
	v, err := param.NewPriceFromString(value)
	if err != nil {
		t.Fatalf("NewPriceFromString(%q) error = %v", value, err)
	}
	return v
}

func mustOrderNativeQuantity(t *testing.T, value string) param.Quantity {
	t.Helper()
	v, err := param.NewQuantityFromString(value)
	if err != nil {
		t.Fatalf("NewQuantityFromString(%q) error = %v", value, err)
	}
	return v
}

func mustOrderNativePnl(t *testing.T, value string) param.Pnl {
	t.Helper()
	v, err := param.NewPnlFromString(value)
	if err != nil {
		t.Fatalf("NewPnlFromString(%q) error = %v", value, err)
	}
	return v
}

func mustOrderNativeFee(t *testing.T, value string) param.Fee {
	t.Helper()
	v, err := param.NewFeeFromString(value)
	if err != nil {
		t.Fatalf("NewFeeFromString(%q) error = %v", value, err)
	}
	return v
}

func newEngineForOrderNativeE2ETest(t *testing.T) *Engine {
	t.Helper()
	builder, err := NewEngineBuilder()
	if err != nil {
		t.Fatalf("NewEngineBuilder() error = %v", err)
	}
	engine, err := builder.Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	return engine
}
