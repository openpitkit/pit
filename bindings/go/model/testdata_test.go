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

func newOrderFixture(t *testing.T) orderFixture {
	t.Helper()

	quantity, err := param.NewQuantityFromString("7")
	if err != nil {
		t.Fatalf("NewQuantityFromString() error = %v", err)
	}

	price, err := param.NewPriceFromString("123.45")
	if err != nil {
		t.Fatalf("NewPriceFromString() error = %v", err)
	}

	return orderFixture{
		tradeAmount:  param.NewQuantityTradeAmount(quantity),
		instrument:   param.NewInstrument(mustModelAsset(t, "USD"), mustModelAsset(t, "USD")),
		price:        price,
		accountID:    param.NewAccountIDFromInt(90210),
		side:         param.SideBuy,
		positionSide: param.PositionSideLong,
		asset:        mustModelAsset(t, "USD"),
		leverage:     param.NewLeverageFromInt(5),
	}
}

func assertOrderEquivalent(t *testing.T, want Order, got Order) {
	t.Helper()
	assertOrderFieldsEqual(t, got, want.Values())
}

func assertOrderMatchesFixture(t *testing.T, order Order, fixture orderFixture) {
	t.Helper()
	assertOrderFieldsEqual(t, order, expectedOrderValues(fixture))
}

func assertOrderValuesMatchFixture(t *testing.T, values OrderValues, fixture orderFixture) {
	t.Helper()
	expected := expectedOrderValues(fixture)
	assertOrderValuesEqual(t, values, expected)
}

func assertOrderEmpty(t *testing.T, o Order) {
	t.Helper()

	if o.Operation().IsSet() {
		t.Fatalf("Order.Operation().IsSet() = true, want false")
	}
	if o.Position().IsSet() {
		t.Fatalf("Order.Position().IsSet() = true, want false")
	}
	if o.Margin().IsSet() {
		t.Fatalf("Order.Margin().IsSet() = true, want false")
	}
}

func assertOrderFieldsEqual(t *testing.T, order Order, expected OrderValues) {
	t.Helper()
	assertOrderValuesEqual(t, order.Values(), expected)
}

func assertOrderValuesEqual(t *testing.T, got OrderValues, want OrderValues) {
	t.Helper()

	gotOperation, gotOperationIsSet := got.Operation.Get()
	wantOperation, wantOperationIsSet := want.Operation.Get()
	if gotOperationIsSet != wantOperationIsSet {
		t.Fatalf("OrderValues.Operation().IsSet() = %v, want %v", gotOperationIsSet, wantOperationIsSet)
	}
	if gotOperationIsSet {
		assertOrderOperationValuesEqual(t, gotOperation.Values(), wantOperation.Values())
	}

	gotPosition, gotPositionIsSet := got.Position.Get()
	wantPosition, wantPositionIsSet := want.Position.Get()
	if gotPositionIsSet != wantPositionIsSet {
		t.Fatalf("OrderValues.Position().IsSet() = %v, want %v", gotPositionIsSet, wantPositionIsSet)
	}
	if gotPositionIsSet {
		assertOrderPositionValuesEqual(t, gotPosition.Values(), wantPosition.Values())
	}

	gotMargin, gotMarginIsSet := got.Margin.Get()
	wantMargin, wantMarginIsSet := want.Margin.Get()
	if gotMarginIsSet != wantMarginIsSet {
		t.Fatalf("OrderValues.Margin().IsSet() = %v, want %v", gotMarginIsSet, wantMarginIsSet)
	}
	if gotMarginIsSet {
		assertOrderMarginValuesEqual(t, gotMargin.Values(), wantMargin.Values())
	}
}

func assertOrderOperationMatchesFixture(t *testing.T, operation OrderOperation, fixture orderFixture) {
	t.Helper()
	assertOrderOperationValuesEqual(t, operation.Values(), expectedOrderOperationValues(fixture))
}

func assertOrderOperationValuesMatchFixture(t *testing.T, values OrderOperationValues, fixture orderFixture) {
	t.Helper()
	assertOrderOperationValuesEqual(t, values, expectedOrderOperationValues(fixture))
}

func assertOrderOperationUnset(t *testing.T, operation OrderOperation) {
	t.Helper()
	assertTradeAmountOptionUnset(t, operation.TradeAmount())
	assertInstrumentOptionUnset(t, operation.Instrument())
	assertPriceOptionUnset(t, operation.Price())
	assertAccountIDOptionUnset(t, operation.AccountID())
	assertSideOptionUnset(t, operation.Side())
}

func assertOrderOperationViewUnset(t *testing.T, operation OrderOperationView) {
	t.Helper()
	assertTradeAmountOptionUnset(t, operation.TradeAmount())
	assertInstrumentOptionUnset(t, operation.Instrument())
	assertPriceOptionUnset(t, operation.Price())
	assertAccountIDOptionUnset(t, operation.AccountID())
	assertSideOptionUnset(t, operation.Side())
}

func assertOrderOperationValuesEqual(t *testing.T, got OrderOperationValues, want OrderOperationValues) {
	t.Helper()
	assertTradeAmountOptionValuesEqual(t, got.TradeAmount, want.TradeAmount)
	assertInstrumentOptionValuesEqual(t, got.Instrument, want.Instrument)
	assertPriceOptionValuesEqual(t, got.Price, want.Price)
	assertAccountIDOptionValuesEqual(t, got.AccountID, want.AccountID)
	assertSideOptionValuesEqual(t, got.Side, want.Side)
}

func assertOrderPositionMatchesFixture(t *testing.T, position OrderPosition, fixture orderFixture) {
	t.Helper()
	assertOrderPositionValuesEqual(t, position.Values(), expectedOrderPositionValues(fixture))
}

func assertOrderPositionValuesMatchFixture(t *testing.T, values OrderPositionValues, fixture orderFixture) {
	t.Helper()
	assertOrderPositionValuesEqual(t, values, expectedOrderPositionValues(fixture))
}

func assertOrderPositionUnset(t *testing.T, position OrderPosition) {
	t.Helper()
	assertPositionSideOptionUnset(t, position.Side())
	assertOptionalBoolUnset(t, position.ReduceOnly())
	assertOptionalBoolUnset(t, position.ClosePosition())
}

func assertOrderPositionViewUnset(t *testing.T, position OrderPositionView) {
	t.Helper()
	assertPositionSideOptionUnset(t, position.Side())
	assertOptionalBoolUnset(t, position.ReduceOnly())
	assertOptionalBoolUnset(t, position.ClosePosition())
}

func assertOrderPositionValuesEqual(t *testing.T, got OrderPositionValues, want OrderPositionValues) {
	t.Helper()
	assertPositionSideOptionValuesEqual(t, got.Side, want.Side)
	assertOptionalBoolValuesEqual(t, got.ReduceOnly, want.ReduceOnly)
	assertOptionalBoolValuesEqual(t, got.ClosePosition, want.ClosePosition)
}

func assertOrderMarginMatchesFixture(t *testing.T, margin OrderMargin, fixture orderFixture) {
	t.Helper()
	assertOrderMarginValuesEqual(t, margin.Values(), expectedOrderMarginValues(fixture))
}

func assertOrderMarginValuesMatchFixture(t *testing.T, values OrderMarginValues, fixture orderFixture) {
	t.Helper()
	assertOrderMarginValuesEqual(t, values, expectedOrderMarginValues(fixture))
}

func assertOrderMarginUnset(t *testing.T, margin OrderMargin) {
	t.Helper()
	assertAssetOptionUnset(t, margin.CollateralAsset())
	assertOptionalBoolUnset(t, margin.AutoBorrow())
	if got := margin.Leverage(); got.IsSet() {
		t.Fatalf("OrderMargin.Leverage() = %v, want %v", got, native.ParamLeverageNotSet)
	}
}

func assertOrderMarginViewUnset(t *testing.T, margin OrderMarginView) {
	t.Helper()
	assertAssetOptionUnset(t, margin.CollateralAsset())
	assertOptionalBoolUnset(t, margin.AutoBorrow())
	if got := margin.Leverage(); got.IsSet() {
		t.Fatalf("OrderMarginView.Leverage() = %v, want %v", got, native.ParamLeverageNotSet)
	}
}

func assertOrderMarginValuesEqual(t *testing.T, got OrderMarginValues, want OrderMarginValues) {
	t.Helper()
	assertAssetOptionValuesEqual(t, got.CollateralAsset, want.CollateralAsset)
	assertOptionalBoolValuesEqual(t, got.AutoBorrow, want.AutoBorrow)
	if got.Leverage != want.Leverage {
		t.Fatalf("OrderMarginValues.Leverage = %v, want %v", got.Leverage, want.Leverage)
	}
}

func assertTradeAmountOptionEqual(
	t *testing.T,
	got optional.Option[param.TradeAmount],
	want param.TradeAmount,
) {
	t.Helper()
	assertTradeAmountOptionValuesEqual(t, got, optional.Some(want))
}

func assertTradeAmountOptionUnset(t *testing.T, got optional.Option[param.TradeAmount]) {
	t.Helper()
	assertTradeAmountOptionValuesEqual(t, got, optional.None[param.TradeAmount]())
}

func assertTradeAmountOptionValuesEqual(
	t *testing.T,
	got optional.Option[param.TradeAmount],
	want optional.Option[param.TradeAmount],
) {
	t.Helper()
	assertOptionBy(t, "TradeAmount", got, want, func(gotValue param.TradeAmount, wantValue param.TradeAmount) {
		gotQuantity := gotValue.MustQuantity()
		wantQuantity := wantValue.MustQuantity()
		if !gotQuantity.Equal(wantQuantity) {
			t.Fatalf("TradeAmount quantity = %s, want %s", gotQuantity.String(), wantQuantity.String())
		}
	})
}

func assertInstrumentOptionEqual(
	t *testing.T,
	got optional.Option[param.Instrument],
	want param.Instrument,
) {
	t.Helper()
	assertInstrumentOptionValuesEqual(t, got, optional.Some(want))
}

func assertInstrumentOptionUnset(t *testing.T, got optional.Option[param.Instrument]) {
	t.Helper()
	assertInstrumentOptionValuesEqual(t, got, optional.None[param.Instrument]())
}

func assertInstrumentOptionValuesEqual(
	t *testing.T,
	got optional.Option[param.Instrument],
	want optional.Option[param.Instrument],
) {
	t.Helper()
	assertOptionBy(t, "Instrument", got, want, func(gotValue param.Instrument, wantValue param.Instrument) {
		if !gotValue.UnderlyingAsset.Equal(wantValue.UnderlyingAsset) {
			t.Fatalf(
				"Instrument.UnderlyingAsset = %s, want %s",
				gotValue.UnderlyingAsset,
				wantValue.UnderlyingAsset,
			)
		}
		if !gotValue.SettlementAsset.Equal(wantValue.SettlementAsset) {
			t.Fatalf(
				"Instrument.SettlementAsset = %s, want %s",
				gotValue.SettlementAsset,
				wantValue.SettlementAsset,
			)
		}
	})
}

func assertPriceOptionEqual(t *testing.T, got optional.Option[param.Price], want param.Price) {
	t.Helper()
	assertPriceOptionValuesEqual(t, got, optional.Some(want))
}

func assertPriceOptionUnset(t *testing.T, got optional.Option[param.Price]) {
	t.Helper()
	assertPriceOptionValuesEqual(t, got, optional.None[param.Price]())
}

func assertPriceOptionValuesEqual(
	t *testing.T,
	got optional.Option[param.Price],
	want optional.Option[param.Price],
) {
	t.Helper()
	assertOptionBy(t, "Price", got, want, func(gotValue param.Price, wantValue param.Price) {
		if !gotValue.Equal(wantValue) {
			t.Fatalf("Price = %s, want %s", gotValue.String(), wantValue.String())
		}
	})
}

func assertAccountIDOptionEqual(
	t *testing.T,
	got optional.Option[param.AccountID],
	want param.AccountID,
) {
	t.Helper()
	assertAccountIDOptionValuesEqual(t, got, optional.Some(want))
}

func assertAccountIDOptionUnset(t *testing.T, got optional.Option[param.AccountID]) {
	t.Helper()
	assertAccountIDOptionValuesEqual(t, got, optional.None[param.AccountID]())
}

func assertAccountIDOptionValuesEqual(
	t *testing.T,
	got optional.Option[param.AccountID],
	want optional.Option[param.AccountID],
) {
	t.Helper()
	assertOptionBy(t, "AccountID", got, want, func(gotValue param.AccountID, wantValue param.AccountID) {
		if gotValue.Handle() != wantValue.Handle() {
			t.Fatalf("AccountID.Handle() = %v, want %v", gotValue.Handle(), wantValue.Handle())
		}
	})
}

func assertSideOptionEqual(t *testing.T, got optional.Option[param.Side], want param.Side) {
	t.Helper()
	assertSideOptionValuesEqual(t, got, optional.Some(want))
}

func assertSideOptionUnset(t *testing.T, got optional.Option[param.Side]) {
	t.Helper()
	assertSideOptionValuesEqual(t, got, optional.None[param.Side]())
}

func assertSideOptionValuesEqual(
	t *testing.T,
	got optional.Option[param.Side],
	want optional.Option[param.Side],
) {
	t.Helper()
	assertOptionBy(t, "Side", got, want, func(gotValue param.Side, wantValue param.Side) {
		if gotValue != wantValue {
			t.Fatalf("Side = %v, want %v", gotValue, wantValue)
		}
	})
}

func assertPositionSideOptionEqual(
	t *testing.T,
	got optional.Option[param.PositionSide],
	want param.PositionSide,
) {
	t.Helper()
	assertPositionSideOptionValuesEqual(t, got, optional.Some(want))
}

func assertPositionSideOptionUnset(t *testing.T, got optional.Option[param.PositionSide]) {
	t.Helper()
	assertPositionSideOptionValuesEqual(t, got, optional.None[param.PositionSide]())
}

func assertPositionSideOptionValuesEqual(
	t *testing.T,
	got optional.Option[param.PositionSide],
	want optional.Option[param.PositionSide],
) {
	t.Helper()
	assertOptionBy(t, "PositionSide", got, want, func(gotValue param.PositionSide, wantValue param.PositionSide) {
		if gotValue != wantValue {
			t.Fatalf("PositionSide = %v, want %v", gotValue, wantValue)
		}
	})
}

func assertAssetOptionEqual(t *testing.T, got optional.Option[param.Asset], want param.Asset) {
	t.Helper()
	assertAssetOptionValuesEqual(t, got, optional.Some(want))
}

func assertAssetOptionUnset(t *testing.T, got optional.Option[param.Asset]) {
	t.Helper()
	assertAssetOptionValuesEqual(t, got, optional.None[param.Asset]())
}

func assertAssetOptionValuesEqual(
	t *testing.T,
	got optional.Option[param.Asset],
	want optional.Option[param.Asset],
) {
	t.Helper()
	assertOptionBy(t, "Asset", got, want, func(gotValue param.Asset, wantValue param.Asset) {
		if !gotValue.Equal(wantValue) {
			t.Fatalf("Asset = %v, want %v", gotValue, wantValue)
		}
	})
}

func assertOptionalBoolEqual(t *testing.T, got optional.Bool, want bool) { // nolint
	t.Helper()
	assertOptionalBoolValuesEqual(t, got, optional.BoolSome(want))
}

func assertOptionalBoolUnset(t *testing.T, got optional.Bool) {
	t.Helper()
	assertOptionalBoolValuesEqual(t, got, optional.BoolNone)
}

func assertOptionalBoolValuesEqual(t *testing.T, got optional.Bool, want optional.Bool) {
	t.Helper()
	if got != want {
		t.Fatalf("optional.Bool = %v, want %v", got, want)
	}
}

func assertOptionBy[T any](
	t *testing.T,
	name string,
	got optional.Option[T],
	want optional.Option[T],
	assertValue func(got T, want T),
) {
	t.Helper()
	gotValue, gotIsSet := got.Get()
	wantValue, wantIsSet := want.Get()
	if gotIsSet != wantIsSet {
		t.Fatalf("%s option IsSet = %v, want %v", name, gotIsSet, wantIsSet)
	}
	if gotIsSet {
		assertValue(gotValue, wantValue)
	}
}

func expectedOrderValues(fixture orderFixture) OrderValues {
	return OrderValues{
		Operation: optional.Some(NewOrderOperationFromValues(expectedOrderOperationValues(fixture))),
		Position:  optional.Some(NewOrderPositionFromValues(expectedOrderPositionValues(fixture))),
		Margin:    optional.Some(NewOrderMarginFromValues(expectedOrderMarginValues(fixture))),
	}
}

func expectedOrderOperationValues(fixture orderFixture) OrderOperationValues {
	return OrderOperationValues{
		TradeAmount: optional.Some(fixture.tradeAmount),
		Instrument:  optional.Some(fixture.instrument),
		Price:       optional.Some(fixture.price),
		AccountID:   optional.Some(fixture.accountID),
		Side:        optional.Some(fixture.side),
	}
}

func expectedOrderPositionValues(fixture orderFixture) OrderPositionValues {
	return OrderPositionValues{
		Side:          optional.Some(fixture.positionSide),
		ReduceOnly:    optional.BoolSome(true),
		ClosePosition: optional.BoolSome(true),
	}
}

func expectedOrderMarginValues(fixture orderFixture) OrderMarginValues {
	return OrderMarginValues{
		CollateralAsset: optional.Some(fixture.asset),
		AutoBorrow:      optional.BoolSome(true),
		Leverage:        optional.Some(fixture.leverage),
	}
}
