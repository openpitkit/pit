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

	"github.com/openpitkit/pit-go/param"
	"github.com/openpitkit/pit-go/pkg/optional"
)

type accountAdjustmentFixture struct {
	asset          param.Asset
	altAsset       param.Asset
	instrument     param.Instrument
	averagePrice   param.Price
	altPrice       param.Price
	leverage       param.Leverage
	mode           param.PositionMode
	deltaAmount    param.AdjustmentAmount
	absoluteAmount param.AdjustmentAmount
	totalUpper     param.PositionSize
	totalLower     param.PositionSize
	reservedUpper  param.PositionSize
	reservedLower  param.PositionSize
	pendingUpper   param.PositionSize
	pendingLower   param.PositionSize
}

func TestAccountAdjustmentValuesCheck(t *testing.T) {
	fixture := newAccountAdjustmentFixture(t)

	balance := NewAccountAdjustmentBalanceOperationFromValues(
		AccountAdjustmentBalanceOperationParams{
			Asset:             optional.Some(fixture.asset),
			AverageEntryPrice: optional.Some(fixture.averagePrice),
		},
	)
	position := NewAccountAdjustmentPositionOperationFromValues(
		AccountAdjustmentPositionOperationValues{
			Instrument: optional.Some(fixture.instrument),
		},
	)

	err := (AccountAdjustmentValues{
		BalanceOperation:  optional.Some(balance),
		PositionOperation: optional.Some(position),
	}).Check()
	if err == nil {
		t.Fatal("AccountAdjustmentValues.Check() error = nil, want non-nil")
	}
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
		NewAccountAdjustmentFromNative(fromValues.Native()).Values(),
		values,
	)
	assertAccountAdjustmentValuesEqual(t, fromValues.EngineAccountAdjustment().Values(), values)

	fromValues.Reset()
	assertAccountAdjustmentUnset(t, fromValues)
}

func TestAccountAdjustmentSetValuesRejectsConflictingOperations(t *testing.T) {
	fixture := newAccountAdjustmentFixture(t)
	adjustment := NewAccountAdjustment()

	err := adjustment.SetValues(
		AccountAdjustmentValues{
			BalanceOperation: optional.Some(
				NewAccountAdjustmentBalanceOperationFromValues(
					AccountAdjustmentBalanceOperationParams{
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
	if err == nil {
		t.Fatal("AccountAdjustment.SetValues() error = nil, want non-nil")
	}
}

func TestAccountAdjustmentOperationSwitching(t *testing.T) {
	fixture := newAccountAdjustmentFixture(t)
	adjustment := NewAccountAdjustment()

	balance := NewAccountAdjustmentBalanceOperationFromValues(
		AccountAdjustmentBalanceOperationParams{
			Asset:             optional.Some(fixture.asset),
			AverageEntryPrice: optional.Some(fixture.averagePrice),
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

	view := adjustment.SetBalanceOperationByViewAndUnsetPositionOperation()
	assertAssetOptionUnset(t, view.Asset())
	assertPriceOptionUnset(t, view.AverageEntryPrice())

	view.SetAsset(fixture.asset)
	view.SetAverageEntryPrice(fixture.averagePrice)

	assertAssetOptionEqual(t, view.Asset(), fixture.asset)
	assertPriceOptionEqual(t, view.AverageEntryPrice(), fixture.averagePrice)

	view.UnsetAsset()
	view.UnsetAverageEntryPrice()
	assertAssetOptionUnset(t, view.Asset())
	assertPriceOptionUnset(t, view.AverageEntryPrice())

	view.SetAsset(fixture.altAsset)
	view.SetAverageEntryPrice(fixture.altPrice)
	view.Reset()
	assertAssetOptionUnset(t, view.Asset())
	assertPriceOptionUnset(t, view.AverageEntryPrice())
}

func TestAccountAdjustmentPositionOperationView(t *testing.T) {
	fixture := newAccountAdjustmentFixture(t)
	adjustment := NewAccountAdjustment()
	instrument := param.NewInstrument(param.NewAsset("AAPL"), param.NewAsset("AAPL"))

	view := adjustment.SetPositionOperationByViewAndUnsetBalanceOperation()
	view.SetInstrument(instrument)
	view.SetCollateralAsset(fixture.asset)
	view.SetAverageEntryPrice(fixture.averagePrice)
	view.SetLeverage(fixture.leverage.Native())
	view.SetMode(fixture.mode.Native())

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

	amount.SetTotal(fixture.deltaAmount)
	assertAdjustmentAmountOptionEqual(t, amount.Total(), fixture.deltaAmount)
	amount.UnsetTotal()
	assertAdjustmentAmountOptionUnset(t, amount.Total())

	amount.SetReserved(fixture.absoluteAmount)
	assertAdjustmentAmountOptionEqual(t, amount.Reserved(), fixture.absoluteAmount)
	amount.UnsetReserved()
	assertAdjustmentAmountOptionUnset(t, amount.Reserved())

	amount.SetPending(fixture.deltaAmount)
	assertAdjustmentAmountOptionEqual(t, amount.Pending(), fixture.deltaAmount)
	amount.UnsetPending()
	assertAdjustmentAmountOptionUnset(t, amount.Pending())

	values := AccountAdjustmentAmountValues{
		Total:    optional.Some(fixture.deltaAmount),
		Reserved: optional.Some(fixture.absoluteAmount),
		Pending:  optional.Some(fixture.deltaAmount),
	}
	amount.SetValues(values)
	assertAccountAdjustmentAmountValuesEqual(t, amount.Values(), values)

	adjustment := NewAccountAdjustment()
	view := adjustment.EnsureAmountView()
	view.SetTotal(fixture.deltaAmount)
	view.SetReserved(fixture.absoluteAmount)
	view.SetPending(fixture.deltaAmount)
	assertAccountAdjustmentAmountOptionEqual(
		t,
		adjustment.Amount(),
		NewAccountAdjustmentAmountFromValues(values),
	)
	view.Reset()
	assertAdjustmentAmountOptionUnset(t, view.Total())
	assertAdjustmentAmountOptionUnset(t, view.Reserved())
	assertAdjustmentAmountOptionUnset(t, view.Pending())

	amount.Reset()
	assertAccountAdjustmentAmountUnset(t, amount)
}

func TestAccountAdjustmentBoundsLifecycle(t *testing.T) {
	fixture := newAccountAdjustmentFixture(t)
	boundsValues := accountAdjustmentBoundsValuesFromFixture(fixture)
	bounds := NewAccountAdjustmentBounds()
	assertAccountAdjustmentBoundsUnset(t, bounds)

	bounds.SetTotalUpper(fixture.totalUpper)
	assertPositionSizeOptionEqual(t, bounds.TotalUpper(), fixture.totalUpper)
	bounds.UnsetTotalUpper()
	assertPositionSizeOptionUnset(t, bounds.TotalUpper())

	bounds.SetTotalLower(fixture.totalLower)
	assertPositionSizeOptionEqual(t, bounds.TotalLower(), fixture.totalLower)
	bounds.UnsetTotalLower()
	assertPositionSizeOptionUnset(t, bounds.TotalLower())

	bounds.SetReservedUpper(fixture.reservedUpper)
	assertPositionSizeOptionEqual(t, bounds.ReservedUpper(), fixture.reservedUpper)
	bounds.UnsetReservedUpper()
	assertPositionSizeOptionUnset(t, bounds.ReservedUpper())

	bounds.SetReservedLower(fixture.reservedLower)
	assertPositionSizeOptionEqual(t, bounds.ReservedLower(), fixture.reservedLower)
	bounds.UnsetReservedLower()
	assertPositionSizeOptionUnset(t, bounds.ReservedLower())

	bounds.SetPendingUpper(fixture.pendingUpper)
	assertPositionSizeOptionEqual(t, bounds.PendingUpper(), fixture.pendingUpper)
	bounds.UnsetPendingUpper()
	assertPositionSizeOptionUnset(t, bounds.PendingUpper())

	bounds.SetPendingLower(fixture.pendingLower)
	assertPositionSizeOptionEqual(t, bounds.PendingLower(), fixture.pendingLower)
	bounds.UnsetPendingLower()
	assertPositionSizeOptionUnset(t, bounds.PendingLower())

	// Keep this direct call to exercise SetValues path without changing
	// production behavior assumptions in this test session.
	bounds.SetValues(boundsValues)

	adjustment := NewAccountAdjustment()
	view := adjustment.EnsureBoundsView()
	view.SetTotalUpper(fixture.totalUpper)
	view.SetTotalLower(fixture.totalLower)
	view.SetReservedUpper(fixture.reservedUpper)
	view.SetReservedLower(fixture.reservedLower)
	view.SetPendingUpper(fixture.pendingUpper)
	view.SetPendingLower(fixture.pendingLower)
	expectedBounds := NewAccountAdjustmentBounds()
	expectedBounds.SetTotalUpper(fixture.totalUpper)
	expectedBounds.SetTotalLower(fixture.totalLower)
	expectedBounds.SetReservedUpper(fixture.reservedUpper)
	expectedBounds.SetReservedLower(fixture.reservedLower)
	expectedBounds.SetPendingUpper(fixture.pendingUpper)
	expectedBounds.SetPendingLower(fixture.pendingLower)
	assertAccountAdjustmentBoundsOptionEqual(
		t,
		adjustment.Bounds(),
		expectedBounds,
	)
	view.Reset()
	assertPositionSizeOptionUnset(t, view.TotalUpper())
	assertPositionSizeOptionUnset(t, view.TotalLower())
	assertPositionSizeOptionUnset(t, view.ReservedUpper())
	assertPositionSizeOptionUnset(t, view.ReservedLower())
	assertPositionSizeOptionUnset(t, view.PendingUpper())
	assertPositionSizeOptionUnset(t, view.PendingLower())

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

	totalUpper, err := param.NewPositionSizeFromString("100")
	if err != nil {
		t.Fatalf("NewPositionSizeFromString() error = %v", err)
	}
	totalLower, err := param.NewPositionSizeFromString("10")
	if err != nil {
		t.Fatalf("NewPositionSizeFromString() error = %v", err)
	}
	reservedUpper, err := param.NewPositionSizeFromString("50")
	if err != nil {
		t.Fatalf("NewPositionSizeFromString() error = %v", err)
	}
	reservedLower, err := param.NewPositionSizeFromString("5")
	if err != nil {
		t.Fatalf("NewPositionSizeFromString() error = %v", err)
	}
	pendingUpper, err := param.NewPositionSizeFromString("30")
	if err != nil {
		t.Fatalf("NewPositionSizeFromString() error = %v", err)
	}
	pendingLower, err := param.NewPositionSizeFromString("3")
	if err != nil {
		t.Fatalf("NewPositionSizeFromString() error = %v", err)
	}

	return accountAdjustmentFixture{
		asset:          param.NewAsset("USD"),
		altAsset:       param.NewAsset("USDT"),
		instrument:     param.NewInstrument(param.NewAsset("AAPL"), param.NewAsset("USD")),
		averagePrice:   averagePrice,
		altPrice:       altPrice,
		leverage:       param.NewLeverageFromInt(5),
		mode:           param.PositionModeHedged,
		deltaAmount:    param.NewDeltaAdjustmentAmount(totalUpper),
		absoluteAmount: param.NewAbsoluteAdjustmentAmount(totalLower),
		totalUpper:     totalUpper,
		totalLower:     totalLower,
		reservedUpper:  reservedUpper,
		reservedLower:  reservedLower,
		pendingUpper:   pendingUpper,
		pendingLower:   pendingLower,
	}
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
					Total:    optional.Some(fixture.deltaAmount),
					Reserved: optional.Some(fixture.absoluteAmount),
					Pending:  optional.Some(fixture.deltaAmount),
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
) AccountAdjustmentBoundsParams {
	return AccountAdjustmentBoundsParams{
		TotalUpper:    optional.Some(fixture.totalUpper),
		TotalLower:    optional.Some(fixture.totalLower),
		ReservedUpper: optional.Some(fixture.reservedUpper),
		ReservedLower: optional.Some(fixture.reservedLower),
		PendingUpper:  optional.Some(fixture.pendingUpper),
		PendingLower:  optional.Some(fixture.pendingLower),
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
	assertAdjustmentAmountOptionUnset(t, amount.Total())
	assertAdjustmentAmountOptionUnset(t, amount.Reserved())
	assertAdjustmentAmountOptionUnset(t, amount.Pending())
}

func assertAccountAdjustmentAmountValuesEqual(
	t *testing.T,
	got AccountAdjustmentAmountValues,
	want AccountAdjustmentAmountValues,
) {
	t.Helper()
	assertAdjustmentAmountOptionValuesEqual(t, got.Total, want.Total)
	assertAdjustmentAmountOptionValuesEqual(t, got.Reserved, want.Reserved)
	assertAdjustmentAmountOptionValuesEqual(t, got.Pending, want.Pending)
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
		assertPositionSizeOptionValuesEqual(t, gotValue.TotalUpper(), wantValue.TotalUpper())
		assertPositionSizeOptionValuesEqual(t, gotValue.TotalLower(), wantValue.TotalLower())
		assertPositionSizeOptionValuesEqual(t, gotValue.ReservedUpper(), wantValue.ReservedUpper())
		assertPositionSizeOptionValuesEqual(t, gotValue.ReservedLower(), wantValue.ReservedLower())
		assertPositionSizeOptionValuesEqual(t, gotValue.PendingUpper(), wantValue.PendingUpper())
		assertPositionSizeOptionValuesEqual(t, gotValue.PendingLower(), wantValue.PendingLower())
	})
}

func assertAccountAdjustmentBoundsUnset(t *testing.T, bounds AccountAdjustmentBounds) {
	t.Helper()
	assertPositionSizeOptionUnset(t, bounds.TotalUpper())
	assertPositionSizeOptionUnset(t, bounds.TotalLower())
	assertPositionSizeOptionUnset(t, bounds.ReservedUpper())
	assertPositionSizeOptionUnset(t, bounds.ReservedLower())
	assertPositionSizeOptionUnset(t, bounds.PendingUpper())
	assertPositionSizeOptionUnset(t, bounds.PendingLower())
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
		if gotValue.Native() != wantValue.Native() {
			t.Fatalf("Leverage.Native() = %v, want %v", gotValue.Native(), wantValue.Native())
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
