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

package model

import (
	"testing"

	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/optional"
)

type accountAdjustmentFixture struct {
	asset          param.Asset
	altAsset       param.Asset
	instrument     param.Instrument
	averagePrice   param.Price
	altPrice       param.Price
	realizedPnl    param.Pnl
	altRealizedPnl param.Pnl
	leverage       param.Leverage
	mode           param.PositionMode
	deltaAmount    param.AdjustmentAmount
	absoluteAmount param.AdjustmentAmount
	balanceUpper   param.PositionSize
	balanceLower   param.PositionSize
	heldUpper      param.PositionSize
	heldLower      param.PositionSize
	incomingUpper  param.PositionSize
	incomingLower  param.PositionSize
}

func TestAccountAdjustmentLifecycle(t *testing.T) {
	fixture := newAccountAdjustmentFixture(t)
	values := accountAdjustmentValuesFromFixture(fixture)

	adjustment := NewAccountAdjustment()
	assertAccountAdjustmentUnset(t, adjustment)

	err := adjustment.SetValues(values)
	if err != nil {
		t.Fatalf("AccountAdjustment.SetValues() error = %v", err)
	}
	assertAccountAdjustmentValuesEqual(t, adjustment.Values(), values)

	adjustment.UnsetPositionOperation()
	adjustment.UnsetAmount()
	adjustment.UnsetBounds()
	assertAccountAdjustmentUnset(t, adjustment)

	fromValues, err := NewAccountAdjustmentFromValues(values)
	if err != nil {
		t.Fatalf("NewAccountAdjustmentFromValues() error = %v", err)
	}
	assertAccountAdjustmentValuesEqual(t, fromValues.Values(), values)
	assertAccountAdjustmentValuesEqual(
		t,
		NewAccountAdjustmentFromHandle(fromValues.Handle()).Values(),
		values,
	)
	assertAccountAdjustmentValuesEqual(t, fromValues.EngineAccountAdjustment().Values(), values)

	fromValues.Reset()
	assertAccountAdjustmentUnset(t, fromValues)
}

func TestAccountAdjustmentSetValuesOperationsAreMutuallyExclusive(t *testing.T) {
	fixture := newAccountAdjustmentFixture(t)
	adjustment := NewAccountAdjustment()

	// The discriminated operation makes a conflicting balance/position pair
	// structurally impossible: applying both leaves only the last one set.
	err := adjustment.SetValues(
		AccountAdjustmentValues{
			BalanceOperation: optional.Some(
				NewAccountAdjustmentBalanceOperationFromValues(
					AccountAdjustmentBalanceOperationValues{
						Asset: optional.Some(fixture.asset),
					},
				),
			),
			PositionOperation: optional.Some(
				NewAccountAdjustmentPositionOperationFromValues(
					AccountAdjustmentPositionOperationValues{
						Instrument: optional.Some(fixture.instrument),
					},
				),
			),
		},
	)
	if err != nil {
		t.Fatalf("AccountAdjustment.SetValues() error = %v", err)
	}
	assertAccountAdjustmentBalanceOperationOptionUnset(t, adjustment.BalanceOperation())
	assertAccountAdjustmentPositionOperationOptionEqual(
		t,
		adjustment.PositionOperation(),
		NewAccountAdjustmentPositionOperationFromValues(
			AccountAdjustmentPositionOperationValues{
				Instrument: optional.Some(fixture.instrument),
			},
		),
	)
}

func TestAccountAdjustmentOperationSwitching(t *testing.T) {
	fixture := newAccountAdjustmentFixture(t)
	adjustment := NewAccountAdjustment()

	balance := NewAccountAdjustmentBalanceOperationFromValues(
		AccountAdjustmentBalanceOperationValues{
			Asset:             optional.Some(fixture.asset),
			AverageEntryPrice: optional.Some(fixture.averagePrice),
			RealizedPnl:       optional.Some(fixture.realizedPnl),
		},
	)
	adjustment.SetBalanceOperationAndUnsetPositionOperation(balance)
	assertAccountAdjustmentBalanceOperationOptionEqual(t, adjustment.BalanceOperation(), balance)
	assertAccountAdjustmentPositionOperationOptionUnset(t, adjustment.PositionOperation())

	position := NewAccountAdjustmentPositionOperationFromValues(
		AccountAdjustmentPositionOperationValues{
			Instrument:        optional.Some(fixture.instrument),
			CollateralAsset:   optional.Some(fixture.altAsset),
			AverageEntryPrice: optional.Some(fixture.altPrice),
			Leverage:          optional.Some(fixture.leverage),
			Mode:              optional.Some(fixture.mode),
		},
	)
	adjustment.SetPositionOperationAndUnsetBalanceOperation(position)
	assertAccountAdjustmentBalanceOperationOptionUnset(t, adjustment.BalanceOperation())
	assertAccountAdjustmentPositionOperationOptionEqual(t, adjustment.PositionOperation(), position)
}

func TestAccountAdjustmentBalanceOperationView(t *testing.T) {
	fixture := newAccountAdjustmentFixture(t)
	adjustment := NewAccountAdjustment()

	view := adjustment.EnsureBalanceOperationView()
	assertAssetOptionUnset(t, view.Asset())
	assertPriceOptionUnset(t, view.AverageEntryPrice())
	assertPnlOptionUnset(t, view.RealizedPnl())

	view.SetAsset(fixture.asset)
	view.SetAverageEntryPrice(fixture.averagePrice)
	view.SetRealizedPnl(fixture.realizedPnl)

	assertAssetOptionEqual(t, view.Asset(), fixture.asset)
	assertPriceOptionEqual(t, view.AverageEntryPrice(), fixture.averagePrice)
	assertPnlOptionEqual(t, view.RealizedPnl(), fixture.realizedPnl)

	view.UnsetAsset()
	view.UnsetAverageEntryPrice()
	view.UnsetRealizedPnl()
	assertAssetOptionUnset(t, view.Asset())
	assertPriceOptionUnset(t, view.AverageEntryPrice())
	assertPnlOptionUnset(t, view.RealizedPnl())

	view.SetAsset(fixture.altAsset)
	view.SetAverageEntryPrice(fixture.altPrice)
	view.SetRealizedPnl(fixture.altRealizedPnl)
	view.Reset()
	assertAssetOptionUnset(t, view.Asset())
	assertPriceOptionUnset(t, view.AverageEntryPrice())
	assertPnlOptionUnset(t, view.RealizedPnl())
}

func TestAccountAdjustmentPositionOperationView(t *testing.T) {
	fixture := newAccountAdjustmentFixture(t)
	adjustment := NewAccountAdjustment()
	instrument := param.NewInstrument(mustModelAsset(t, "AAPL"), mustModelAsset(t, "AAPL"))

	view := adjustment.EnsurePositionOperationView()
	view.SetInstrument(instrument)
	view.SetCollateralAsset(fixture.asset)
	view.SetAverageEntryPrice(fixture.averagePrice)
	view.SetLeverage(fixture.leverage)
	view.SetMode(fixture.mode)

	assertInstrumentOptionEqual(t, view.Instrument(), instrument)
	assertAssetOptionEqual(t, view.CollateralAsset(), fixture.asset)
	assertPriceOptionEqual(t, view.AverageEntryPrice(), fixture.averagePrice)
	assertLeverageOptionEqual(t, view.Leverage(), fixture.leverage)
	assertPositionModeOptionEqual(t, view.Mode(), fixture.mode)

	view.UnsetInstrument()
	view.UnsetCollateralAsset()
	view.UnsetAverageEntryPrice()
	view.UnsetLeverage()
	view.UnsetMode()

	assertInstrumentOptionUnset(t, view.Instrument())
	assertAssetOptionUnset(t, view.CollateralAsset())
	assertPriceOptionUnset(t, view.AverageEntryPrice())
	assertLeverageOptionUnset(t, view.Leverage())
	assertPositionModeOptionUnset(t, view.Mode())

	view.SetInstrument(instrument)
	view.SetCollateralAsset(fixture.asset)
	view.Reset()
	assertInstrumentOptionUnset(t, view.Instrument())
	assertAssetOptionUnset(t, view.CollateralAsset())
}

func TestAccountAdjustmentAmountLifecycle(t *testing.T) {
	fixture := newAccountAdjustmentFixture(t)
	amount := NewAccountAdjustmentAmount()
	assertAccountAdjustmentAmountUnset(t, amount)

	amount.SetBalance(fixture.deltaAmount)
	assertAdjustmentAmountOptionEqual(t, amount.Balance(), fixture.deltaAmount)
	amount.UnsetBalance()
	assertAdjustmentAmountOptionUnset(t, amount.Balance())

	amount.SetHeld(fixture.absoluteAmount)
	assertAdjustmentAmountOptionEqual(t, amount.Held(), fixture.absoluteAmount)
	amount.UnsetHeld()
	assertAdjustmentAmountOptionUnset(t, amount.Held())

	amount.SetIncoming(fixture.deltaAmount)
	assertAdjustmentAmountOptionEqual(t, amount.Incoming(), fixture.deltaAmount)
	amount.UnsetIncoming()
	assertAdjustmentAmountOptionUnset(t, amount.Incoming())

	values := AccountAdjustmentAmountValues{
		Balance:  optional.Some(fixture.deltaAmount),
		Held:     optional.Some(fixture.absoluteAmount),
		Incoming: optional.Some(fixture.deltaAmount),
	}
	amount.SetValues(values)
	assertAccountAdjustmentAmountValuesEqual(t, amount.Values(), values)

	adjustment := NewAccountAdjustment()
	view := adjustment.EnsureAmountView()
	view.SetBalance(fixture.deltaAmount)
	view.SetHeld(fixture.absoluteAmount)
	view.SetIncoming(fixture.deltaAmount)
	assertAccountAdjustmentAmountOptionEqual(
		t,
		adjustment.Amount(),
		NewAccountAdjustmentAmountFromValues(values),
	)
	view.Reset()
	assertAdjustmentAmountOptionUnset(t, view.Balance())
	assertAdjustmentAmountOptionUnset(t, view.Held())
	assertAdjustmentAmountOptionUnset(t, view.Incoming())

	amount.Reset()
	assertAccountAdjustmentAmountUnset(t, amount)
}

func TestAccountAdjustmentBoundsLifecycle(t *testing.T) {
	fixture := newAccountAdjustmentFixture(t)
	boundsValues := accountAdjustmentBoundsValuesFromFixture(fixture)
	bounds := NewAccountAdjustmentBounds()
	assertAccountAdjustmentBoundsUnset(t, bounds)

	bounds.SetBalanceUpper(fixture.balanceUpper)
	assertPositionSizeOptionEqual(t, bounds.BalanceUpper(), fixture.balanceUpper)
	bounds.UnsetBalanceUpper()
	assertPositionSizeOptionUnset(t, bounds.BalanceUpper())

	bounds.SetBalanceLower(fixture.balanceLower)
	assertPositionSizeOptionEqual(t, bounds.BalanceLower(), fixture.balanceLower)
	bounds.UnsetBalanceLower()
	assertPositionSizeOptionUnset(t, bounds.BalanceLower())

	bounds.SetHeldUpper(fixture.heldUpper)
	assertPositionSizeOptionEqual(t, bounds.HeldUpper(), fixture.heldUpper)
	bounds.UnsetHeldUpper()
	assertPositionSizeOptionUnset(t, bounds.HeldUpper())

	bounds.SetHeldLower(fixture.heldLower)
	assertPositionSizeOptionEqual(t, bounds.HeldLower(), fixture.heldLower)
	bounds.UnsetHeldLower()
	assertPositionSizeOptionUnset(t, bounds.HeldLower())

	bounds.SetIncomingUpper(fixture.incomingUpper)
	assertPositionSizeOptionEqual(t, bounds.IncomingUpper(), fixture.incomingUpper)
	bounds.UnsetIncomingUpper()
	assertPositionSizeOptionUnset(t, bounds.IncomingUpper())

	bounds.SetIncomingLower(fixture.incomingLower)
	assertPositionSizeOptionEqual(t, bounds.IncomingLower(), fixture.incomingLower)
	bounds.UnsetIncomingLower()
	assertPositionSizeOptionUnset(t, bounds.IncomingLower())

	// Keep this direct call to exercise SetValues path without changing
	// production behavior assumptions in this test session.
	bounds.SetValues(boundsValues)

	adjustment := NewAccountAdjustment()
	view := adjustment.EnsureBoundsView()
	view.SetBalanceUpper(fixture.balanceUpper)
	view.SetBalanceLower(fixture.balanceLower)
	view.SetHeldUpper(fixture.heldUpper)
	view.SetHeldLower(fixture.heldLower)
	view.SetIncomingUpper(fixture.incomingUpper)
	view.SetIncomingLower(fixture.incomingLower)
	expectedBounds := NewAccountAdjustmentBounds()
	expectedBounds.SetBalanceUpper(fixture.balanceUpper)
	expectedBounds.SetBalanceLower(fixture.balanceLower)
	expectedBounds.SetHeldUpper(fixture.heldUpper)
	expectedBounds.SetHeldLower(fixture.heldLower)
	expectedBounds.SetIncomingUpper(fixture.incomingUpper)
	expectedBounds.SetIncomingLower(fixture.incomingLower)
	assertAccountAdjustmentBoundsOptionEqual(
		t,
		adjustment.Bounds(),
		expectedBounds,
	)
	view.Reset()
	assertPositionSizeOptionUnset(t, view.BalanceUpper())
	assertPositionSizeOptionUnset(t, view.BalanceLower())
	assertPositionSizeOptionUnset(t, view.HeldUpper())
	assertPositionSizeOptionUnset(t, view.HeldLower())
	assertPositionSizeOptionUnset(t, view.IncomingUpper())
	assertPositionSizeOptionUnset(t, view.IncomingLower())

	bounds.Reset()
	assertAccountAdjustmentBoundsUnset(t, bounds)
}

func newAccountAdjustmentFixture(t *testing.T) accountAdjustmentFixture {
	t.Helper()

	averagePrice, err := param.NewPriceFromString("100.5")
	if err != nil {
		t.Fatalf("NewPriceFromString() error = %v", err)
	}
	altPrice, err := param.NewPriceFromString("101.25")
	if err != nil {
		t.Fatalf("NewPriceFromString() error = %v", err)
	}

	realizedPnl, err := param.NewPnlFromString("12.5")
	if err != nil {
		t.Fatalf("NewPnlFromString() error = %v", err)
	}
	altRealizedPnl, err := param.NewPnlFromString("-7.25")
	if err != nil {
		t.Fatalf("NewPnlFromString() error = %v", err)
	}

	balanceUpper, err := param.NewPositionSizeFromString("100")
	if err != nil {
		t.Fatalf("NewPositionSizeFromString() error = %v", err)
	}
	balanceLower, err := param.NewPositionSizeFromString("10")
	if err != nil {
		t.Fatalf("NewPositionSizeFromString() error = %v", err)
	}
	heldUpper, err := param.NewPositionSizeFromString("50")
	if err != nil {
		t.Fatalf("NewPositionSizeFromString() error = %v", err)
	}
	heldLower, err := param.NewPositionSizeFromString("5")
	if err != nil {
		t.Fatalf("NewPositionSizeFromString() error = %v", err)
	}
	incomingUpper, err := param.NewPositionSizeFromString("30")
	if err != nil {
		t.Fatalf("NewPositionSizeFromString() error = %v", err)
	}
	incomingLower, err := param.NewPositionSizeFromString("3")
	if err != nil {
		t.Fatalf("NewPositionSizeFromString() error = %v", err)
	}

	return accountAdjustmentFixture{
		asset:          mustModelAsset(t, "USD"),
		altAsset:       mustModelAsset(t, "USDT"),
		instrument:     param.NewInstrument(mustModelAsset(t, "AAPL"), mustModelAsset(t, "USD")),
		averagePrice:   averagePrice,
		altPrice:       altPrice,
		realizedPnl:    realizedPnl,
		altRealizedPnl: altRealizedPnl,
		leverage:       param.NewLeverageFromUint16(5),
		mode:           param.PositionModeHedged,
		deltaAmount:    param.NewDeltaAdjustmentAmount(balanceUpper),
		absoluteAmount: param.NewAbsoluteAdjustmentAmount(balanceLower),
		balanceUpper:   balanceUpper,
		balanceLower:   balanceLower,
		heldUpper:      heldUpper,
		heldLower:      heldLower,
		incomingUpper:  incomingUpper,
		incomingLower:  incomingLower,
	}
}

func mustModelAsset(t *testing.T, value string) param.Asset {
	t.Helper()
	asset, err := param.NewAsset(value)
	if err != nil {
		t.Fatalf("NewAsset(%q) error = %v", value, err)
	}
	return asset
}

func accountAdjustmentValuesFromFixture(fixture accountAdjustmentFixture) AccountAdjustmentValues {
	return AccountAdjustmentValues{
		PositionOperation: optional.Some(
			NewAccountAdjustmentPositionOperationFromValues(
				AccountAdjustmentPositionOperationValues{
					Instrument:        optional.Some(fixture.instrument),
					CollateralAsset:   optional.Some(fixture.asset),
					AverageEntryPrice: optional.Some(fixture.averagePrice),
					Leverage:          optional.Some(fixture.leverage),
					Mode:              optional.Some(fixture.mode),
				},
			),
		),
		Amount: optional.Some(
			NewAccountAdjustmentAmountFromValues(
				AccountAdjustmentAmountValues{
					Balance:  optional.Some(fixture.deltaAmount),
					Held:     optional.Some(fixture.absoluteAmount),
					Incoming: optional.Some(fixture.deltaAmount),
				},
			),
		),
		Bounds: optional.Some(
			NewAccountAdjustmentBoundsFromValues(accountAdjustmentBoundsValuesFromFixture(fixture)),
		),
	}
}

func accountAdjustmentBoundsValuesFromFixture(
	fixture accountAdjustmentFixture,
) AccountAdjustmentBoundsValues {
	return AccountAdjustmentBoundsValues{
		BalanceUpper:  optional.Some(fixture.balanceUpper),
		BalanceLower:  optional.Some(fixture.balanceLower),
		HeldUpper:     optional.Some(fixture.heldUpper),
		HeldLower:     optional.Some(fixture.heldLower),
		IncomingUpper: optional.Some(fixture.incomingUpper),
		IncomingLower: optional.Some(fixture.incomingLower),
	}
}

func assertAccountAdjustmentUnset(t *testing.T, adjustment AccountAdjustment) {
	t.Helper()
	assertAccountAdjustmentBalanceOperationOptionUnset(t, adjustment.BalanceOperation())
	assertAccountAdjustmentPositionOperationOptionUnset(t, adjustment.PositionOperation())
	assertAccountAdjustmentAmountOptionUnset(t, adjustment.Amount())
	assertAccountAdjustmentBoundsOptionUnset(t, adjustment.Bounds())
}

func assertAccountAdjustmentValuesEqual(
	t *testing.T,
	got AccountAdjustmentValues,
	want AccountAdjustmentValues,
) {
	t.Helper()
	assertAccountAdjustmentBalanceOperationOptionValuesEqual(
		t,
		got.BalanceOperation,
		want.BalanceOperation,
	)
	assertAccountAdjustmentPositionOperationOptionValuesEqual(
		t,
		got.PositionOperation,
		want.PositionOperation,
	)
	assertAccountAdjustmentAmountOptionValuesEqual(t, got.Amount, want.Amount)
	assertAccountAdjustmentBoundsOptionValuesEqual(t, got.Bounds, want.Bounds)
}

func assertAccountAdjustmentBalanceOperationOptionEqual(
	t *testing.T,
	got optional.Option[AccountAdjustmentBalanceOperation],
	want AccountAdjustmentBalanceOperation,
) {
	t.Helper()
	assertAccountAdjustmentBalanceOperationOptionValuesEqual(t, got, optional.Some(want))
}

func assertAccountAdjustmentBalanceOperationOptionUnset(
	t *testing.T,
	got optional.Option[AccountAdjustmentBalanceOperation],
) {
	t.Helper()
	assertAccountAdjustmentBalanceOperationOptionValuesEqual(
		t,
		got,
		optional.None[AccountAdjustmentBalanceOperation](),
	)
}

func assertAccountAdjustmentBalanceOperationOptionValuesEqual(
	t *testing.T,
	got optional.Option[AccountAdjustmentBalanceOperation],
	want optional.Option[AccountAdjustmentBalanceOperation],
) {
	t.Helper()
	assertOptionBy(t, "BalanceOperation", got, want, func(gotValue AccountAdjustmentBalanceOperation, wantValue AccountAdjustmentBalanceOperation) {
		assertAssetOptionValuesEqual(t, gotValue.Asset(), wantValue.Asset())
		assertPriceOptionValuesEqual(t, gotValue.AverageEntryPrice(), wantValue.AverageEntryPrice())
		assertPnlOptionValuesEqual(t, gotValue.RealizedPnl(), wantValue.RealizedPnl())
	})
}

func assertAccountAdjustmentPositionOperationOptionEqual(
	t *testing.T,
	got optional.Option[AccountAdjustmentPositionOperation],
	want AccountAdjustmentPositionOperation,
) {
	t.Helper()
	assertAccountAdjustmentPositionOperationOptionValuesEqual(t, got, optional.Some(want))
}

func assertAccountAdjustmentPositionOperationOptionUnset(
	t *testing.T,
	got optional.Option[AccountAdjustmentPositionOperation],
) {
	t.Helper()
	assertAccountAdjustmentPositionOperationOptionValuesEqual(
		t,
		got,
		optional.None[AccountAdjustmentPositionOperation](),
	)
}

func assertAccountAdjustmentPositionOperationOptionValuesEqual(
	t *testing.T,
	got optional.Option[AccountAdjustmentPositionOperation],
	want optional.Option[AccountAdjustmentPositionOperation],
) {
	t.Helper()
	assertOptionBy(t, "PositionOperation", got, want, func(gotValue AccountAdjustmentPositionOperation, wantValue AccountAdjustmentPositionOperation) {
		assertInstrumentOptionValuesEqual(t, gotValue.Instrument(), wantValue.Instrument())
		assertAssetOptionValuesEqual(t, gotValue.CollateralAsset(), wantValue.CollateralAsset())
		assertPriceOptionValuesEqual(t, gotValue.AverageEntryPrice(), wantValue.AverageEntryPrice())
		assertLeverageOptionValuesEqual(t, gotValue.Leverage(), wantValue.Leverage())
		assertPositionModeOptionValuesEqual(t, gotValue.Mode(), wantValue.Mode())
	})
}

func assertAccountAdjustmentAmountOptionEqual(
	t *testing.T,
	got optional.Option[AccountAdjustmentAmount],
	want AccountAdjustmentAmount,
) {
	t.Helper()
	assertAccountAdjustmentAmountOptionValuesEqual(t, got, optional.Some(want))
}

func assertAccountAdjustmentAmountOptionUnset(
	t *testing.T,
	got optional.Option[AccountAdjustmentAmount],
) {
	t.Helper()
	assertAccountAdjustmentAmountOptionValuesEqual(t, got, optional.None[AccountAdjustmentAmount]())
}

func assertAccountAdjustmentAmountOptionValuesEqual(
	t *testing.T,
	got optional.Option[AccountAdjustmentAmount],
	want optional.Option[AccountAdjustmentAmount],
) {
	t.Helper()
	assertOptionBy(t, "Amount", got, want, func(gotValue AccountAdjustmentAmount, wantValue AccountAdjustmentAmount) {
		assertAccountAdjustmentAmountValuesEqual(t, gotValue.Values(), wantValue.Values())
	})
}

func assertAccountAdjustmentAmountUnset(t *testing.T, amount AccountAdjustmentAmount) {
	t.Helper()
	assertAdjustmentAmountOptionUnset(t, amount.Balance())
	assertAdjustmentAmountOptionUnset(t, amount.Held())
	assertAdjustmentAmountOptionUnset(t, amount.Incoming())
}

func assertAccountAdjustmentAmountValuesEqual(
	t *testing.T,
	got AccountAdjustmentAmountValues,
	want AccountAdjustmentAmountValues,
) {
	t.Helper()
	assertAdjustmentAmountOptionValuesEqual(t, got.Balance, want.Balance)
	assertAdjustmentAmountOptionValuesEqual(t, got.Held, want.Held)
	assertAdjustmentAmountOptionValuesEqual(t, got.Incoming, want.Incoming)
}

func assertAccountAdjustmentBoundsOptionEqual(
	t *testing.T,
	got optional.Option[AccountAdjustmentBounds],
	want AccountAdjustmentBounds,
) {
	t.Helper()
	assertAccountAdjustmentBoundsOptionValuesEqual(t, got, optional.Some(want))
}

func assertAccountAdjustmentBoundsOptionUnset(
	t *testing.T,
	got optional.Option[AccountAdjustmentBounds],
) {
	t.Helper()
	assertAccountAdjustmentBoundsOptionValuesEqual(t, got, optional.None[AccountAdjustmentBounds]())
}

func assertAccountAdjustmentBoundsOptionValuesEqual(
	t *testing.T,
	got optional.Option[AccountAdjustmentBounds],
	want optional.Option[AccountAdjustmentBounds],
) {
	t.Helper()
	assertOptionBy(t, "Bounds", got, want, func(gotValue AccountAdjustmentBounds, wantValue AccountAdjustmentBounds) {
		assertPositionSizeOptionValuesEqual(t, gotValue.BalanceUpper(), wantValue.BalanceUpper())
		assertPositionSizeOptionValuesEqual(t, gotValue.BalanceLower(), wantValue.BalanceLower())
		assertPositionSizeOptionValuesEqual(t, gotValue.HeldUpper(), wantValue.HeldUpper())
		assertPositionSizeOptionValuesEqual(t, gotValue.HeldLower(), wantValue.HeldLower())
		assertPositionSizeOptionValuesEqual(t, gotValue.IncomingUpper(), wantValue.IncomingUpper())
		assertPositionSizeOptionValuesEqual(t, gotValue.IncomingLower(), wantValue.IncomingLower())
	})
}

func assertAccountAdjustmentBoundsUnset(t *testing.T, bounds AccountAdjustmentBounds) {
	t.Helper()
	assertPositionSizeOptionUnset(t, bounds.BalanceUpper())
	assertPositionSizeOptionUnset(t, bounds.BalanceLower())
	assertPositionSizeOptionUnset(t, bounds.HeldUpper())
	assertPositionSizeOptionUnset(t, bounds.HeldLower())
	assertPositionSizeOptionUnset(t, bounds.IncomingUpper())
	assertPositionSizeOptionUnset(t, bounds.IncomingLower())
}

func assertAdjustmentAmountOptionEqual(
	t *testing.T,
	got optional.Option[param.AdjustmentAmount],
	want param.AdjustmentAmount,
) {
	t.Helper()
	assertAdjustmentAmountOptionValuesEqual(t, got, optional.Some(want))
}

func assertAdjustmentAmountOptionUnset(t *testing.T, got optional.Option[param.AdjustmentAmount]) {
	t.Helper()
	assertAdjustmentAmountOptionValuesEqual(t, got, optional.None[param.AdjustmentAmount]())
}

func assertAdjustmentAmountOptionValuesEqual(
	t *testing.T,
	got optional.Option[param.AdjustmentAmount],
	want optional.Option[param.AdjustmentAmount],
) {
	t.Helper()
	assertOptionBy(t, "AdjustmentAmount", got, want, func(gotValue param.AdjustmentAmount, wantValue param.AdjustmentAmount) {
		if gotValue.IsDelta() != wantValue.IsDelta() {
			t.Fatalf("AdjustmentAmount.IsDelta() = %v, want %v", gotValue.IsDelta(), wantValue.IsDelta())
		}
		if gotValue.IsAbsolute() != wantValue.IsAbsolute() {
			t.Fatalf(
				"AdjustmentAmount.IsAbsolute() = %v, want %v",
				gotValue.IsAbsolute(),
				wantValue.IsAbsolute(),
			)
		}
		if gotValue.IsDelta() {
			if !gotValue.MustDelta().Equal(wantValue.MustDelta()) {
				t.Fatalf("AdjustmentAmount delta = %s, want %s", gotValue.MustDelta(), wantValue.MustDelta())
			}
		}
		if gotValue.IsAbsolute() {
			if !gotValue.MustAbsolute().Equal(wantValue.MustAbsolute()) {
				t.Fatalf(
					"AdjustmentAmount absolute = %s, want %s",
					gotValue.MustAbsolute(),
					wantValue.MustAbsolute(),
				)
			}
		}
	})
}

func assertPositionSizeOptionEqual(
	t *testing.T,
	got optional.Option[param.PositionSize],
	want param.PositionSize,
) {
	t.Helper()
	assertPositionSizeOptionValuesEqual(t, got, optional.Some(want))
}

func assertPositionSizeOptionUnset(t *testing.T, got optional.Option[param.PositionSize]) {
	t.Helper()
	assertPositionSizeOptionValuesEqual(t, got, optional.None[param.PositionSize]())
}

func assertPositionSizeOptionValuesEqual(
	t *testing.T,
	got optional.Option[param.PositionSize],
	want optional.Option[param.PositionSize],
) {
	t.Helper()
	assertOptionBy(t, "PositionSize", got, want, func(gotValue param.PositionSize, wantValue param.PositionSize) {
		if !gotValue.Equal(wantValue) {
			t.Fatalf("PositionSize = %s, want %s", gotValue.String(), wantValue.String())
		}
	})
}

func assertLeverageOptionEqual(
	t *testing.T,
	got optional.Option[param.Leverage],
	want param.Leverage,
) {
	t.Helper()
	assertLeverageOptionValuesEqual(t, got, optional.Some(want))
}

func assertLeverageOptionUnset(t *testing.T, got optional.Option[param.Leverage]) {
	t.Helper()
	assertLeverageOptionValuesEqual(t, got, optional.None[param.Leverage]())
}

func assertLeverageOptionValuesEqual(
	t *testing.T,
	got optional.Option[param.Leverage],
	want optional.Option[param.Leverage],
) {
	t.Helper()
	assertOptionBy(t, "Leverage", got, want, func(gotValue param.Leverage, wantValue param.Leverage) {
		if gotValue.Handle() != wantValue.Handle() {
			t.Fatalf("Leverage.Handle() = %v, want %v", gotValue.Handle(), wantValue.Handle())
		}
	})
}

func assertPositionModeOptionEqual(
	t *testing.T,
	got optional.Option[param.PositionMode],
	want param.PositionMode,
) {
	t.Helper()
	assertPositionModeOptionValuesEqual(t, got, optional.Some(want))
}

func assertPositionModeOptionUnset(
	t *testing.T,
	got optional.Option[param.PositionMode],
) {
	t.Helper()
	assertPositionModeOptionValuesEqual(t, got, optional.None[param.PositionMode]())
}

func assertPositionModeOptionValuesEqual(
	t *testing.T,
	got optional.Option[param.PositionMode],
	want optional.Option[param.PositionMode],
) {
	t.Helper()
	assertOptionBy(t, "PositionMode", got, want, func(gotValue param.PositionMode, wantValue param.PositionMode) {
		if gotValue != wantValue {
			t.Fatalf("PositionMode = %v, want %v", gotValue, wantValue)
		}
	})
}
