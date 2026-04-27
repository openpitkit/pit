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
	"testing"

	"github.com/openpitkit/pit-go/accountadjustment"
	"github.com/openpitkit/pit-go/model"
	"github.com/openpitkit/pit-go/param"
	"github.com/openpitkit/pit-go/pkg/optional"
	"github.com/openpitkit/pit-go/reject"
	"github.com/openpitkit/pit-go/tx"
)

func TestAccountAdjustmentNativeE2E_BatchAppliesAndInvokesPolicyPerItem(t *testing.T) {
	policy := &accountAdjustmentCountingPolicy{name: "count-adjustments"}

	builder, err := NewEngineBuilder()
	if err != nil {
		t.Fatalf("NewEngineBuilder() error = %v", err)
	}
	builder.AccountAdjustmentPolicy(policy)

	engine, err := builder.Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	first, err := model.NewAccountAdjustmentFromValues(
		model.AccountAdjustmentValues{
			BalanceOperation: optional.Some(
				model.NewAccountAdjustmentBalanceOperationFromValues(
					model.AccountAdjustmentBalanceOperationParams{
						Asset:             optional.Some(param.NewAsset("USD")),
						AverageEntryPrice: optional.Some(mustAdjustmentNativePrice(t, "101.5")),
					},
				),
			),
			Amount: optional.Some(
				model.NewAccountAdjustmentAmountFromValues(
					model.AccountAdjustmentAmountValues{
						Total: optional.Some(
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
							param.NewInstrument(param.NewAsset("AAPL"), param.NewAsset("USD")),
						),
						CollateralAsset: optional.Some(param.NewAsset("USD")),
						AverageEntryPrice: optional.Some(
							mustAdjustmentNativePrice(t, "102.25"),
						),
						Leverage: optional.Some(param.NewLeverageFromInt(4)),
						Mode:     optional.Some(param.PositionModeHedged),
					},
				),
			),
			Bounds: optional.Some(
				model.NewAccountAdjustmentBoundsFromValues(
					model.AccountAdjustmentBoundsParams{
						TotalUpper:   optional.Some(mustAdjustmentNativePositionSize(t, "100")),
						TotalLower:   optional.Some(mustAdjustmentNativePositionSize(t, "20")),
						PendingUpper: optional.Some(mustAdjustmentNativePositionSize(t, "50")),
						PendingLower: optional.Some(mustAdjustmentNativePositionSize(t, "5")),
					},
				),
			),
		},
	)
	if err != nil {
		t.Fatalf("NewAccountAdjustmentFromValues(second) error = %v", err)
	}

	rejects, err := engine.ApplyAccountAdjustment(
		param.NewAccountIDFromInt(77),
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

type accountAdjustmentCountingPolicy struct {
	name  string
	calls int
}

func (p *accountAdjustmentCountingPolicy) Close() {}

func (p *accountAdjustmentCountingPolicy) Name() string {
	return p.name
}

func (p *accountAdjustmentCountingPolicy) ApplyAccountAdjustment(
	accountadjustment.Context,
	param.AccountID,
	model.AccountAdjustment,
	tx.Mutations,
) reject.List {
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

func mustAdjustmentNativePositionSize(t *testing.T, value string) param.PositionSize {
	t.Helper()
	v, err := param.NewPositionSizeFromString(value)
	if err != nil {
		t.Fatalf("NewPositionSizeFromString(%q) error = %v", value, err)
	}
	return v
}
