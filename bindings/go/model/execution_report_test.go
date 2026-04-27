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

	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/optional"
)

type executionReportFixture struct {
	instrument     param.Instrument
	accountID      param.AccountID
	side           param.Side
	pnl            param.Pnl
	fee            param.Fee
	tradePrice     param.Price
	tradeQuantity  param.Quantity
	leavesQuantity param.Quantity
	lockPrice      param.Price
	positionEffect param.PositionEffect
	positionSide   param.PositionSide
}

func TestExecutionReportLifecycle(t *testing.T) {
	fixture := newExecutionReportFixture(t)
	report := NewExecutionReport()
	assertExecutionReportUnset(t, report)

	values := executionReportValuesFromFixture(fixture)
	report.SetValues(values)
	assertExecutionReportValuesEqual(t, report.Values(), values)

	report.UnsetOperation()
	report.UnsetFinancialImpact()
	report.UnsetFill()
	report.UnsetPositionImpact()
	assertExecutionReportUnset(t, report)

	report = NewExecutionReportFromValues(values)
	assertExecutionReportValuesEqual(t, report.Values(), values)
	assertExecutionReportValuesEqual(t, report.EngineExecutionReport().Values(), values)
	assertExecutionReportValuesEqual(
		t,
		NewExecutionReportFromNative(report.Native()).Values(),
		values,
	)

	report.Reset()
	assertExecutionReportUnset(t, report)
}

func TestExecutionReportOperationFieldRoundTrip(t *testing.T) {
	fixture := newExecutionReportFixture(t)
	operation := NewExecutionReportOperation()
	assertExecutionReportOperationUnset(t, operation)

	operation.SetInstrument(fixture.instrument)
	assertInstrumentOptionEqual(t, operation.Instrument(), fixture.instrument)
	operation.UnsetInstrument()
	assertInstrumentOptionUnset(t, operation.Instrument())

	operation.SetAccountID(fixture.accountID)
	assertAccountIDOptionEqual(t, operation.AccountID(), fixture.accountID)
	operation.UnsetAccountID()
	assertAccountIDOptionUnset(t, operation.AccountID())

	operation.SetSide(fixture.side)
	assertSideOptionEqual(t, operation.Side(), fixture.side)
	operation.UnsetSide()
	assertSideOptionUnset(t, operation.Side())

	values := ExecutionReportOperationValues{
		Instrument: optional.Some(fixture.instrument),
		AccountID:  optional.Some(fixture.accountID),
		Side:       optional.Some(fixture.side),
	}
	operation.SetValues(values)
	assertExecutionReportOperationValuesEqual(t, operation.Values(), values)

	operation.Reset()
	assertExecutionReportOperationUnset(t, operation)
}

func TestExecutionReportFinancialImpactFieldRoundTrip(t *testing.T) {
	fixture := newExecutionReportFixture(t)
	impact := NewExecutionReportFinancialImpact()
	assertExecutionReportFinancialImpactUnset(t, impact)

	impact.SetPnl(fixture.pnl)
	assertPnlOptionEqual(t, impact.Pnl(), fixture.pnl)
	impact.UnsetPnl()
	assertPnlOptionUnset(t, impact.Pnl())

	impact.SetFee(fixture.fee)
	assertFeeOptionEqual(t, impact.Fee(), fixture.fee)
	impact.UnsetFee()
	assertFeeOptionUnset(t, impact.Fee())

	values := ExecutionReportFinancialImpactValues{
		Pnl: optional.Some(fixture.pnl),
		Fee: optional.Some(fixture.fee),
	}
	impact.SetValues(values)
	assertExecutionReportFinancialImpactValuesEqual(t, impact.Values(), values)

	impact.Reset()
	assertExecutionReportFinancialImpactUnset(t, impact)
}

func TestExecutionReportTradeFieldRoundTrip(t *testing.T) {
	fixture := newExecutionReportFixture(t)
	trade := NewExecutionReportTrade(fixture.tradePrice, fixture.tradeQuantity)
	assertPriceEqual(t, trade.Price(), fixture.tradePrice)
	assertQuantityEqual(t, trade.Quantity(), fixture.tradeQuantity)

	trade.Reset()
	trade.SetPrice(fixture.lockPrice)
	trade.SetQuantity(fixture.leavesQuantity)

	assertPriceEqual(t, trade.Price(), fixture.lockPrice)
	assertQuantityEqual(t, trade.Quantity(), fixture.leavesQuantity)

	assertPriceEqual(
		t,
		NewExecutionReportTradeFromNative(trade.value).Price(),
		fixture.lockPrice,
	)
}

func TestExecutionReportFillFieldRoundTrip(t *testing.T) {
	fixture := newExecutionReportFixture(t)
	fill := NewExecutionReportFill()
	assertExecutionReportFillUnset(t, fill)

	lastTrade := NewExecutionReportTrade(fixture.tradePrice, fixture.tradeQuantity)
	fill.SetLastTrade(lastTrade)
	assertExecutionReportTradeOptionEqual(t, fill.LastTrade(), lastTrade)
	fill.UnsetLastTrade()
	assertExecutionReportTradeOptionUnset(t, fill.LastTrade())

	fill.SetLeavesQuantity(fixture.leavesQuantity)
	assertQuantityOptionEqual(t, fill.LeavesQuantity(), fixture.leavesQuantity)
	fill.UnsetLeavesQuantity()
	assertQuantityOptionUnset(t, fill.LeavesQuantity())

	fill.SetLockPrice(fixture.lockPrice)
	assertPriceOptionEqual(t, fill.LockPrice(), fixture.lockPrice)
	fill.UnsetLockPrice()
	assertPriceOptionUnset(t, fill.LockPrice())

	fill.SetTerminal(true)
	if !fill.Terminal() {
		t.Fatal("Fill.Terminal() = false, want true")
	}
	fill.SetTerminal(false)
	if fill.Terminal() {
		t.Fatal("Fill.Terminal() = true, want false")
	}

	values := ExecutionReportFillValues{
		LastTrade:      optional.Some(lastTrade),
		LeavesQuantity: optional.Some(fixture.leavesQuantity),
		LockPrice:      optional.Some(fixture.lockPrice),
		Terminal:       true,
	}
	fill.SetValues(values)
	assertExecutionReportFillValuesEqual(t, fill.Values(), values)

	fill.Reset()
	assertExecutionReportFillUnset(t, fill)
}

func TestExecutionReportPositionImpactFieldRoundTrip(t *testing.T) {
	fixture := newExecutionReportFixture(t)
	impact := NewExecutionReportPositionImpact()
	assertExecutionReportPositionImpactUnset(t, impact)

	impact.SetPositionEffect(fixture.positionEffect)
	assertPositionEffectOptionEqual(t, impact.PositionEffect(), fixture.positionEffect)
	impact.UnsetPositionEffect()
	assertPositionEffectOptionUnset(t, impact.PositionEffect())

	impact.SetPositionSide(fixture.positionSide)
	assertPositionSideOptionEqual(t, impact.PositionSide(), fixture.positionSide)
	impact.UnsetPositionSide()
	assertPositionSideOptionUnset(t, impact.PositionSide())

	values := ExecutionReportPositionImpactValues{
		PositionEffect: optional.Some(fixture.positionEffect),
		PositionSide:   optional.Some(fixture.positionSide),
	}
	impact.SetValues(values)
	assertExecutionReportPositionImpactValuesEqual(t, impact.Values(), values)

	impact.Reset()
	assertExecutionReportPositionImpactUnset(t, impact)
}

func TestNewPositionEffectFromNative(t *testing.T) {
	if got, ok := newPositionEffectFromNative(native.ParamPositionEffectOpen).Get(); !ok || got != param.PositionEffectOpen {
		t.Fatalf("newPositionEffectFromNative(open) = (%v, %v), want (%v, true)", got, ok, param.PositionEffectOpen)
	}
	if got, ok := newPositionEffectFromNative(native.ParamPositionEffectClose).Get(); !ok || got != param.PositionEffectClose {
		t.Fatalf("newPositionEffectFromNative(close) = (%v, %v), want (%v, true)", got, ok, param.PositionEffectClose)
	}
	if newPositionEffectFromNative(native.ParamPositionEffectNotSet).IsSet() {
		t.Fatal("newPositionEffectFromNative(not-set).IsSet() = true, want false")
	}
}

func TestNewPositionEffectFromNativePanicsOnUnknownValue(t *testing.T) {
	didPanic := false
	func() {
		defer func() {
			if recover() != nil {
				didPanic = true
			}
		}()
		_ = newPositionEffectFromNative(native.ParamPositionEffect(255))
	}()
	if !didPanic {
		t.Fatal("newPositionEffectFromNative() panic = nil, want non-nil")
	}
}

func newExecutionReportFixture(t *testing.T) executionReportFixture {
	t.Helper()

	pnl, err := param.NewPnlFromString("12.5")
	if err != nil {
		t.Fatalf("NewPnlFromString() error = %v", err)
	}

	fee, err := param.NewFeeFromString("0.15")
	if err != nil {
		t.Fatalf("NewFeeFromString() error = %v", err)
	}

	tradePrice, err := param.NewPriceFromString("123.45")
	if err != nil {
		t.Fatalf("NewPriceFromString() error = %v", err)
	}

	tradeQuantity, err := param.NewQuantityFromString("7")
	if err != nil {
		t.Fatalf("NewQuantityFromString() error = %v", err)
	}

	leavesQuantity, err := param.NewQuantityFromString("3")
	if err != nil {
		t.Fatalf("NewQuantityFromString() error = %v", err)
	}

	lockPrice, err := param.NewPriceFromString("124")
	if err != nil {
		t.Fatalf("NewPriceFromString() error = %v", err)
	}

	return executionReportFixture{
		// Keep same asset on both legs to avoid depending on current
		// NewInstrumentFromNative settlement-leg mapping behavior.
		instrument:     param.NewInstrument(param.NewAsset("USD"), param.NewAsset("USD")),
		accountID:      param.NewAccountIDFromInt(42),
		side:           param.SideBuy,
		pnl:            pnl,
		fee:            fee,
		tradePrice:     tradePrice,
		tradeQuantity:  tradeQuantity,
		leavesQuantity: leavesQuantity,
		lockPrice:      lockPrice,
		positionEffect: param.PositionEffectOpen,
		positionSide:   param.PositionSideLong,
	}
}

func executionReportValuesFromFixture(fixture executionReportFixture) ExecutionReportValues {
	operation := NewExecutionReportOperationFromValues(
		ExecutionReportOperationValues{
			Instrument: optional.Some(fixture.instrument),
			AccountID:  optional.Some(fixture.accountID),
			Side:       optional.Some(fixture.side),
		},
	)

	financialImpact := NewExecutionReportFinancialImpactFromValues(
		ExecutionReportFinancialImpactValues{
			Pnl: optional.Some(fixture.pnl),
			Fee: optional.Some(fixture.fee),
		},
	)

	fill := NewExecutionReportFillFromValues(
		ExecutionReportFillValues{
			LastTrade: optional.Some(
				NewExecutionReportTrade(fixture.tradePrice, fixture.tradeQuantity),
			),
			LeavesQuantity: optional.Some(fixture.leavesQuantity),
			LockPrice:      optional.Some(fixture.lockPrice),
			Terminal:       true,
		},
	)

	positionImpact := NewExecutionReportPositionImpactFromValues(
		ExecutionReportPositionImpactValues{
			PositionEffect: optional.Some(fixture.positionEffect),
			PositionSide:   optional.Some(fixture.positionSide),
		},
	)

	return ExecutionReportValues{
		Operation:       optional.Some(operation),
		FinancialImpact: optional.Some(financialImpact),
		Fill:            optional.Some(fill),
		PositionImpact:  optional.Some(positionImpact),
	}
}

func assertExecutionReportUnset(t *testing.T, report ExecutionReport) {
	t.Helper()
	if report.Operation().IsSet() {
		t.Fatal("ExecutionReport.Operation().IsSet() = true, want false")
	}
	if report.FinancialImpact().IsSet() {
		t.Fatal("ExecutionReport.FinancialImpact().IsSet() = true, want false")
	}
	if report.Fill().IsSet() {
		t.Fatal("ExecutionReport.Fill().IsSet() = true, want false")
	}
	if report.PositionImpact().IsSet() {
		t.Fatal("ExecutionReport.PositionImpact().IsSet() = true, want false")
	}
}

func assertExecutionReportValuesEqual(
	t *testing.T,
	got ExecutionReportValues,
	want ExecutionReportValues,
) {
	t.Helper()
	assertOptionBy(t, "ExecutionReport.Operation", got.Operation, want.Operation, func(gotValue ExecutionReportOperation, wantValue ExecutionReportOperation) {
		assertExecutionReportOperationValuesEqual(t, gotValue.Values(), wantValue.Values())
	})
	assertOptionBy(t, "ExecutionReport.FinancialImpact", got.FinancialImpact, want.FinancialImpact, func(gotValue ExecutionReportFinancialImpact, wantValue ExecutionReportFinancialImpact) {
		assertExecutionReportFinancialImpactValuesEqual(t, gotValue.Values(), wantValue.Values())
	})
	assertOptionBy(t, "ExecutionReport.Fill", got.Fill, want.Fill, func(gotValue ExecutionReportFill, wantValue ExecutionReportFill) {
		assertExecutionReportFillValuesEqual(t, gotValue.Values(), wantValue.Values())
	})
	assertOptionBy(t, "ExecutionReport.PositionImpact", got.PositionImpact, want.PositionImpact, func(gotValue ExecutionReportPositionImpact, wantValue ExecutionReportPositionImpact) {
		assertExecutionReportPositionImpactValuesEqual(t, gotValue.Values(), wantValue.Values())
	})
}

func assertExecutionReportOperationUnset(t *testing.T, operation ExecutionReportOperation) {
	t.Helper()
	assertInstrumentOptionUnset(t, operation.Instrument())
	assertAccountIDOptionUnset(t, operation.AccountID())
	assertSideOptionUnset(t, operation.Side())
}

func assertExecutionReportOperationValuesEqual(
	t *testing.T,
	got ExecutionReportOperationValues,
	want ExecutionReportOperationValues,
) {
	t.Helper()
	assertInstrumentOptionValuesEqual(t, got.Instrument, want.Instrument)
	assertAccountIDOptionValuesEqual(t, got.AccountID, want.AccountID)
	assertSideOptionValuesEqual(t, got.Side, want.Side)
}

func assertExecutionReportFinancialImpactUnset(
	t *testing.T,
	impact ExecutionReportFinancialImpact,
) {
	t.Helper()
	assertPnlOptionUnset(t, impact.Pnl())
	assertFeeOptionUnset(t, impact.Fee())
}

func assertExecutionReportFinancialImpactValuesEqual(
	t *testing.T,
	got ExecutionReportFinancialImpactValues,
	want ExecutionReportFinancialImpactValues,
) {
	t.Helper()
	assertPnlOptionValuesEqual(t, got.Pnl, want.Pnl)
	assertFeeOptionValuesEqual(t, got.Fee, want.Fee)
}

func assertExecutionReportFillUnset(t *testing.T, fill ExecutionReportFill) {
	t.Helper()
	assertExecutionReportTradeOptionUnset(t, fill.LastTrade())
	assertQuantityOptionUnset(t, fill.LeavesQuantity())
	assertPriceOptionUnset(t, fill.LockPrice())
	if fill.Terminal() {
		t.Fatal("ExecutionReportFill.Terminal() = true, want false")
	}
}

func assertExecutionReportFillValuesEqual(
	t *testing.T,
	got ExecutionReportFillValues,
	want ExecutionReportFillValues,
) {
	t.Helper()
	assertExecutionReportTradeOptionValuesEqual(t, got.LastTrade, want.LastTrade)
	assertQuantityOptionValuesEqual(t, got.LeavesQuantity, want.LeavesQuantity)
	assertPriceOptionValuesEqual(t, got.LockPrice, want.LockPrice)
	if got.Terminal != want.Terminal {
		t.Fatalf("ExecutionReportFillValues.Terminal = %v, want %v", got.Terminal, want.Terminal)
	}
}

func assertExecutionReportPositionImpactUnset(
	t *testing.T,
	impact ExecutionReportPositionImpact,
) {
	t.Helper()
	assertPositionEffectOptionUnset(t, impact.PositionEffect())
	assertPositionSideOptionUnset(t, impact.PositionSide())
}

func assertExecutionReportPositionImpactValuesEqual(
	t *testing.T,
	got ExecutionReportPositionImpactValues,
	want ExecutionReportPositionImpactValues,
) {
	t.Helper()
	assertPositionEffectOptionValuesEqual(t, got.PositionEffect, want.PositionEffect)
	assertPositionSideOptionValuesEqual(t, got.PositionSide, want.PositionSide)
}

func assertExecutionReportTradeOptionEqual(
	t *testing.T,
	got optional.Option[ExecutionReportTrade],
	want ExecutionReportTrade,
) {
	t.Helper()
	assertExecutionReportTradeOptionValuesEqual(t, got, optional.Some(want))
}

func assertExecutionReportTradeOptionUnset(t *testing.T, got optional.Option[ExecutionReportTrade]) {
	t.Helper()
	assertExecutionReportTradeOptionValuesEqual(t, got, optional.None[ExecutionReportTrade]())
}

func assertExecutionReportTradeOptionValuesEqual(
	t *testing.T,
	got optional.Option[ExecutionReportTrade],
	want optional.Option[ExecutionReportTrade],
) {
	t.Helper()
	assertOptionBy(t, "ExecutionReportTrade", got, want, func(gotValue ExecutionReportTrade, wantValue ExecutionReportTrade) {
		assertPriceEqual(t, gotValue.Price(), wantValue.Price())
		assertQuantityEqual(t, gotValue.Quantity(), wantValue.Quantity())
	})
}

func assertPnlOptionEqual(t *testing.T, got optional.Option[param.Pnl], want param.Pnl) {
	t.Helper()
	assertPnlOptionValuesEqual(t, got, optional.Some(want))
}

func assertPnlOptionUnset(t *testing.T, got optional.Option[param.Pnl]) {
	t.Helper()
	assertPnlOptionValuesEqual(t, got, optional.None[param.Pnl]())
}

func assertPnlOptionValuesEqual(
	t *testing.T,
	got optional.Option[param.Pnl],
	want optional.Option[param.Pnl],
) {
	t.Helper()
	assertOptionBy(t, "Pnl", got, want, func(gotValue param.Pnl, wantValue param.Pnl) {
		if !gotValue.Equal(wantValue) {
			t.Fatalf("Pnl = %s, want %s", gotValue.String(), wantValue.String())
		}
	})
}

func assertFeeOptionEqual(t *testing.T, got optional.Option[param.Fee], want param.Fee) {
	t.Helper()
	assertFeeOptionValuesEqual(t, got, optional.Some(want))
}

func assertFeeOptionUnset(t *testing.T, got optional.Option[param.Fee]) {
	t.Helper()
	assertFeeOptionValuesEqual(t, got, optional.None[param.Fee]())
}

func assertFeeOptionValuesEqual(
	t *testing.T,
	got optional.Option[param.Fee],
	want optional.Option[param.Fee],
) {
	t.Helper()
	assertOptionBy(t, "Fee", got, want, func(gotValue param.Fee, wantValue param.Fee) {
		if !gotValue.Equal(wantValue) {
			t.Fatalf("Fee = %s, want %s", gotValue.String(), wantValue.String())
		}
	})
}

func assertQuantityOptionEqual(
	t *testing.T,
	got optional.Option[param.Quantity],
	want param.Quantity,
) {
	t.Helper()
	assertQuantityOptionValuesEqual(t, got, optional.Some(want))
}

func assertQuantityOptionUnset(t *testing.T, got optional.Option[param.Quantity]) {
	t.Helper()
	assertQuantityOptionValuesEqual(t, got, optional.None[param.Quantity]())
}

func assertQuantityOptionValuesEqual(
	t *testing.T,
	got optional.Option[param.Quantity],
	want optional.Option[param.Quantity],
) {
	t.Helper()
	assertOptionBy(t, "Quantity", got, want, func(gotValue param.Quantity, wantValue param.Quantity) {
		if !gotValue.Equal(wantValue) {
			t.Fatalf("Quantity = %s, want %s", gotValue.String(), wantValue.String())
		}
	})
}

func assertPositionEffectOptionEqual(
	t *testing.T,
	got optional.Option[param.PositionEffect],
	want param.PositionEffect,
) {
	t.Helper()
	assertPositionEffectOptionValuesEqual(t, got, optional.Some(want))
}

func assertPositionEffectOptionUnset(t *testing.T, got optional.Option[param.PositionEffect]) {
	t.Helper()
	assertPositionEffectOptionValuesEqual(t, got, optional.None[param.PositionEffect]())
}

func assertPositionEffectOptionValuesEqual(
	t *testing.T,
	got optional.Option[param.PositionEffect],
	want optional.Option[param.PositionEffect],
) {
	t.Helper()
	assertOptionBy(t, "PositionEffect", got, want, func(gotValue param.PositionEffect, wantValue param.PositionEffect) {
		if gotValue != wantValue {
			t.Fatalf("PositionEffect = %v, want %v", gotValue, wantValue)
		}
	})
}

func assertPriceEqual(t *testing.T, got param.Price, want param.Price) {
	t.Helper()
	if !got.Equal(want) {
		t.Fatalf("Price = %s, want %s", got.String(), want.String())
	}
}

func assertQuantityEqual(t *testing.T, got param.Quantity, want param.Quantity) {
	t.Helper()
	if !got.Equal(want) {
		t.Fatalf("Quantity = %s, want %s", got.String(), want.String())
	}
}
