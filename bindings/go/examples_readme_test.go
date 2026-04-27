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
	"go.openpit.dev/openpit/pretrade/policies"
)

// TestReadmeQuickstart mirrors the Usage example in bindings/go/README.md.
// Keep both in sync.
func TestReadmeQuickstart(t *testing.T) {
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

	// 1. Configure policies.
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

	// 2. Build the engine (one time at the platform initialization).
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

	// 3. Check an order.
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

	// 5. Real pre-trade and risk control.
	reservation, rejects, err := request.Execute()
	if err != nil {
		t.Fatalf("Execute() error = %v", err)
	}
	if rejects != nil {
		t.Fatalf("Execute() unexpected rejects: %v", rejects)
	}
	defer reservation.Close()

	// Optional shortcut for the same two-stage flow:
	// reservation, rejects, err := engine.ExecutePreTrade(order)

	// 6. Commit the reservation.
	reservation.Commit()

	// 7. Apply execution report.
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

	// 8. Kill switch must not be triggered after a small loss.
	if result.KillSwitchTriggered {
		t.Fatal("KillSwitchTriggered = true, want false")
	}
}
