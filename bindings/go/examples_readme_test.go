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
	"testing"

	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pretrade/policies"
)

// Mirrors public examples from bindings/go/README.md.
// If this test changes, update every linked documentation snippet.

// Source: bindings/go/README.md - Quick Start
func TestReadmeCheckOrder(t *testing.T) {
	// Build the engine once, at platform initialization.
	engine, err := NewEngineBuilder().
		FullSync().
		Builtin(policies.BuildOrderValidation()).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	aapl, err := param.NewAsset("AAPL")
	if err != nil {
		t.Fatalf("NewAsset(AAPL) error = %v", err)
	}
	usd, err := param.NewAsset("USD")
	if err != nil {
		t.Fatalf("NewAsset(USD) error = %v", err)
	}
	qty, err := param.NewQuantityFromString("100")
	if err != nil {
		t.Fatalf("NewQuantityFromString() error = %v", err)
	}
	price, err := param.NewPriceFromString("185")
	if err != nil {
		t.Fatalf("NewPriceFromString() error = %v", err)
	}

	// Describe the order: buy 100 AAPL at 185 USD.
	order := model.NewOrder()
	operation := order.EnsureOperationView()
	operation.SetInstrument(param.NewInstrument(aapl, usd))
	operation.SetAccountID(param.NewAccountIDFromUint64(99224416))
	operation.SetSide(param.SideBuy)
	operation.SetTradeAmount(param.NewQuantityTradeAmount(qty))
	operation.SetPrice(price)

	// Run the pre-trade pipeline and read the verdict.
	reservation, rejects, err := engine.ExecutePreTrade(order)
	if err != nil {
		t.Fatalf("ExecutePreTrade() error = %v", err)
	}
	if rejects != nil {
		t.Fatalf("ExecutePreTrade() unexpected rejects: %v", rejects)
	}
	// Close rolls the reservation back unless it was committed.
	defer reservation.Close()

	// The venue accepted the order, so the reserved state stays.
	reservation.Commit()
}
