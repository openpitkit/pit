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

type orderFixture struct {
	tradeAmount  param.TradeAmount
	instrument   param.Instrument
	price        param.Price
	accountID    param.AccountID
	side         param.Side
	positionSide param.PositionSide
	asset        param.Asset
	leverage     param.Leverage
}

func TestOrderLifecycle(t *testing.T) {
	fixture := newOrderFixture(t)

	o := NewOrder()
	assertOrderEmpty(t, o)

	opView := o.EnsureOperationView()
	if !o.Operation().IsSet() {
		t.Fatalf("Operation().IsSet() = false, want true after EnsureOperationView")
	}
	assertOrderOperationViewUnset(t, opView)

	opView.SetTradeAmount(fixture.tradeAmount)
	assertTradeAmountOptionEqual(t, opView.TradeAmount(), fixture.tradeAmount)
	opView.UnsetTradeAmount()
	assertTradeAmountOptionUnset(t, opView.TradeAmount())

	opView.SetInstrument(fixture.instrument)
	assertInstrumentOptionEqual(t, opView.Instrument(), fixture.instrument)
	opView.UnsetInstrument()
	assertInstrumentOptionUnset(t, opView.Instrument())

	opView.SetPrice(fixture.price)
	assertPriceOptionEqual(t, opView.Price(), fixture.price)
	opView.UnsetPrice()
	assertPriceOptionUnset(t, opView.Price())

	opView.SetAccountID(fixture.accountID)
	assertAccountIDOptionEqual(t, opView.AccountID(), fixture.accountID)
	opView.UnsetAccountID()
	assertAccountIDOptionUnset(t, opView.AccountID())

	opView.SetSide(fixture.side)
	assertSideOptionEqual(t, opView.Side(), fixture.side)
	opView.UnsetSide()
	assertSideOptionUnset(t, opView.Side())

	opView.Reset()
	assertOrderOperationViewUnset(t, opView)

	positionView := o.EnsurePositionView()
	if !o.Position().IsSet() {
		t.Fatalf("Position().IsSet() = false, want true after EnsurePositionView")
	}
	assertOrderPositionViewUnset(t, positionView)

	positionView.SetSide(fixture.positionSide)
	assertPositionSideOptionEqual(t, positionView.Side(), fixture.positionSide)
	positionView.UnsetSide()
	assertPositionSideOptionUnset(t, positionView.Side())

	positionView.SetReduceOnly(true)
	assertOptionalBoolEqual(t, positionView.ReduceOnly(), true)
	positionView.UnsetReduceOnly()
	assertOptionalBoolUnset(t, positionView.ReduceOnly())

	positionView.SetClosePosition(true)
	assertOptionalBoolEqual(t, positionView.ClosePosition(), true)
	positionView.UnsetClosePosition()
	assertOptionalBoolUnset(t, positionView.ClosePosition())

	positionView.Reset()
	assertOrderPositionViewUnset(t, positionView)

	marginView := o.EnsureMarginView()
	if !o.Margin().IsSet() {
		t.Fatalf("Margin().IsSet() = false, want true after EnsureMarginView")
	}
	assertOrderMarginViewUnset(t, marginView)

	marginView.SetCollateralAsset(fixture.asset)
	assertAssetOptionEqual(t, marginView.CollateralAsset(), fixture.asset)
	marginView.UnsetCollateralAsset()
	assertAssetOptionUnset(t, marginView.CollateralAsset())

	marginView.SetAutoBorrow(true)
	assertOptionalBoolEqual(t, marginView.AutoBorrow(), true)
	marginView.UnsetAutoBorrow()
	assertOptionalBoolUnset(t, marginView.AutoBorrow())

	marginView.SetLeverage(fixture.leverage)
	if got := marginView.Leverage(); !got.IsSet() || got.MustGet() != fixture.leverage {
		t.Fatalf("OrderMarginView.Leverage() = %v, want %v", got, fixture.leverage)
	}
	marginView.UnsetLeverage()
	if got := marginView.Leverage(); got.IsSet() {
		t.Fatalf("OrderMarginView.Leverage() = %v, want %v", got, native.ParamLeverageNotSet)
	}

	marginView.Reset()
	assertOrderMarginViewUnset(t, marginView)

	o.UnsetOperation()
	o.UnsetPosition()
	o.UnsetMargin()
	assertOrderEmpty(t, o)
}

func TestOrderFromValuesSetValuesAndReset(t *testing.T) {
	fixture := newOrderFixture(t)

	values := OrderValues{
		Operation: optional.Some(NewOrderOperationFromValues(OrderOperationValues{
			TradeAmount: optional.Some(fixture.tradeAmount),
			Instrument:  optional.Some(fixture.instrument),
			Price:       optional.Some(fixture.price),
			AccountID:   optional.Some(fixture.accountID),
			Side:        optional.Some(fixture.side),
		})),
		Position: optional.Some(NewOrderPositionFromValues(OrderPositionValues{
			Side:          optional.Some(fixture.positionSide),
			ReduceOnly:    optional.BoolSome(true),
			ClosePosition: optional.BoolSome(true),
		})),
		Margin: optional.Some(NewOrderMarginFromValues(OrderMarginValues{
			CollateralAsset: optional.Some(fixture.asset),
			AutoBorrow:      optional.BoolSome(true),
			Leverage:        optional.Some(fixture.leverage),
		})),
	}

	o := NewOrderFromValues(values)
	assertOrderMatchesFixture(t, o, fixture)
	assertOrderValuesMatchFixture(t, o.Values(), fixture)

	o.SetValues(OrderValues{})
	assertOrderEmpty(t, o)

	o.SetValues(values)
	assertOrderMatchesFixture(t, o, fixture)

	o.Reset()
	assertOrderEmpty(t, o)
}

func TestOrderFromHandleRoundTrip(t *testing.T) {
	fixture := newOrderFixture(t)

	source := NewOrderFromValues(OrderValues{
		Operation: optional.Some(NewOrderOperationFromValues(OrderOperationValues{
			TradeAmount: optional.Some(fixture.tradeAmount),
			Instrument:  optional.Some(fixture.instrument),
			Price:       optional.Some(fixture.price),
			AccountID:   optional.Some(fixture.accountID),
			Side:        optional.Some(fixture.side),
		})),
		Position: optional.Some(NewOrderPositionFromValues(OrderPositionValues{
			Side:          optional.Some(fixture.positionSide),
			ReduceOnly:    optional.BoolSome(true),
			ClosePosition: optional.BoolSome(true),
		})),
		Margin: optional.Some(NewOrderMarginFromValues(OrderMarginValues{
			CollateralAsset: optional.Some(fixture.asset),
			AutoBorrow:      optional.BoolSome(true),
			Leverage:        optional.Some(fixture.leverage),
		})),
	})

	roundTripped := NewOrderFromHandle(source.Handle())
	assertOrderEquivalent(t, source, roundTripped)

	engineOrder := source.EngineOrder()
	assertOrderEquivalent(t, source, engineOrder)
}

func TestOrderOperationFieldRoundTrip(t *testing.T) {
	fixture := newOrderFixture(t)
	op := NewOrderOperation()
	assertOrderOperationUnset(t, op)

	tests := []struct {
		name        string
		set         func(*OrderOperation)
		unset       func(*OrderOperation)
		assert      func(OrderOperation)
		assertUnset func(OrderOperation)
	}{
		{
			name: "TradeAmount",
			set: func(v *OrderOperation) {
				v.SetTradeAmount(fixture.tradeAmount)
			},
			unset: func(v *OrderOperation) {
				v.UnsetTradeAmount()
			},
			assert: func(v OrderOperation) {
				assertTradeAmountOptionEqual(t, v.TradeAmount(), fixture.tradeAmount)
			},
			assertUnset: func(v OrderOperation) {
				assertTradeAmountOptionUnset(t, v.TradeAmount())
			},
		},
		{
			name: "Instrument",
			set: func(v *OrderOperation) {
				v.SetInstrument(fixture.instrument)
			},
			unset: func(v *OrderOperation) {
				v.UnsetInstrument()
			},
			assert: func(v OrderOperation) {
				assertInstrumentOptionEqual(t, v.Instrument(), fixture.instrument)
			},
			assertUnset: func(v OrderOperation) {
				assertInstrumentOptionUnset(t, v.Instrument())
			},
		},
		{
			name: "Price",
			set: func(v *OrderOperation) {
				v.SetPrice(fixture.price)
			},
			unset: func(v *OrderOperation) {
				v.UnsetPrice()
			},
			assert: func(v OrderOperation) {
				assertPriceOptionEqual(t, v.Price(), fixture.price)
			},
			assertUnset: func(v OrderOperation) {
				assertPriceOptionUnset(t, v.Price())
			},
		},
		{
			name: "AccountID",
			set: func(v *OrderOperation) {
				v.SetAccountID(fixture.accountID)
			},
			unset: func(v *OrderOperation) {
				v.UnsetAccountID()
			},
			assert: func(v OrderOperation) {
				assertAccountIDOptionEqual(t, v.AccountID(), fixture.accountID)
			},
			assertUnset: func(v OrderOperation) {
				assertAccountIDOptionUnset(t, v.AccountID())
			},
		},
		{
			name: "Side",
			set: func(v *OrderOperation) {
				v.SetSide(fixture.side)
			},
			unset: func(v *OrderOperation) {
				v.UnsetSide()
			},
			assert: func(v OrderOperation) {
				assertSideOptionEqual(t, v.Side(), fixture.side)
			},
			assertUnset: func(v OrderOperation) {
				assertSideOptionUnset(t, v.Side())
			},
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(*testing.T) {
			tc.set(&op)
			tc.assert(op)
			tc.unset(&op)
			tc.assertUnset(op)
		})
	}

	op.SetValues(OrderOperationValues{
		TradeAmount: optional.Some(fixture.tradeAmount),
		Instrument:  optional.Some(fixture.instrument),
		Price:       optional.Some(fixture.price),
		AccountID:   optional.Some(fixture.accountID),
		Side:        optional.Some(fixture.side),
	})
	assertOrderOperationMatchesFixture(t, op, fixture)
	assertOrderOperationValuesMatchFixture(t, op.Values(), fixture)

	op.Reset()
	assertOrderOperationUnset(t, op)
}

func TestOrderPositionFieldRoundTrip(t *testing.T) {
	fixture := newOrderFixture(t)
	position := NewOrderPosition()
	assertOrderPositionUnset(t, position)

	position.SetSide(fixture.positionSide)
	assertPositionSideOptionEqual(t, position.Side(), fixture.positionSide)
	position.UnsetSide()
	assertPositionSideOptionUnset(t, position.Side())

	position.SetReduceOnly(true)
	assertOptionalBoolEqual(t, position.ReduceOnly(), true)
	position.UnsetReduceOnly()
	assertOptionalBoolUnset(t, position.ReduceOnly())

	position.SetClosePosition(true)
	assertOptionalBoolEqual(t, position.ClosePosition(), true)
	position.UnsetClosePosition()
	assertOptionalBoolUnset(t, position.ClosePosition())

	position.SetValues(OrderPositionValues{
		Side:          optional.Some(fixture.positionSide),
		ReduceOnly:    optional.BoolSome(true),
		ClosePosition: optional.BoolSome(true),
	})
	assertOrderPositionMatchesFixture(t, position, fixture)
	assertOrderPositionValuesMatchFixture(t, position.Values(), fixture)

	position.Reset()
	assertOrderPositionUnset(t, position)
}

func TestOrderMarginFieldRoundTrip(t *testing.T) {
	fixture := newOrderFixture(t)
	margin := NewOrderMargin()
	assertOrderMarginUnset(t, margin)

	margin.SetCollateralAsset(fixture.asset)
	assertAssetOptionEqual(t, margin.CollateralAsset(), fixture.asset)
	margin.UnsetCollateralAsset()
	assertAssetOptionUnset(t, margin.CollateralAsset())

	margin.SetAutoBorrow(true)
	assertOptionalBoolEqual(t, margin.AutoBorrow(), true)
	margin.UnsetAutoBorrow()
	assertOptionalBoolUnset(t, margin.AutoBorrow())

	margin.SetLeverage(fixture.leverage)
	if got := margin.Leverage(); !got.IsSet() || got.MustGet() != fixture.leverage {
		t.Fatalf("OrderMargin.Leverage() = %v, want %v", got, fixture.leverage)
	}
	margin.UnsetLeverage()
	if got := margin.Leverage(); got.IsSet() {
		t.Fatalf("OrderMargin.Leverage() = %v, want %v", got, native.ParamLeverageNotSet)
	}

	margin.SetValues(OrderMarginValues{
		CollateralAsset: optional.Some(fixture.asset),
		AutoBorrow:      optional.BoolSome(true),
		Leverage:        optional.Some(fixture.leverage),
	})
	assertOrderMarginMatchesFixture(t, margin, fixture)
	assertOrderMarginValuesMatchFixture(t, margin.Values(), fixture)

	margin.Reset()
	assertOrderMarginUnset(t, margin)
}
