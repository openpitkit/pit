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
	"go.openpit.dev/openpit/internal/convert"
	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/optional"
)

//------------------------------------------------------------------------------
// Order

type Order struct{ value native.Order }

func NewOrder() Order {
	return NewOrderFromNative(native.NewOrder())
}

type OrderValues struct {
	Operation optional.Option[OrderOperation]
	Position  optional.Option[OrderPosition]
	Margin    optional.Option[OrderMargin]
}

func NewOrderFromValues(values OrderValues) Order {
	o := NewOrder()
	o.setValues(values)
	return o
}

func NewOrderFromNative(value native.Order) Order {
	return Order{value: value}
}

func (o *Order) Reset() {
	native.OrderReset(&o.value)
}

func (o Order) Values() OrderValues {
	return OrderValues{
		Operation: o.Operation(),
		Position:  o.Position(),
		Margin:    o.Margin(),
	}
}

func (o *Order) SetValues(values OrderValues) {
	o.Reset()
	o.setValues(values)
}

func (o *Order) setValues(values OrderValues) {
	if value, ok := values.Operation.Get(); ok {
		o.SetOperation(value)
	}
	if value, ok := values.Position.Get(); ok {
		o.SetPosition(value)
	}
	if value, ok := values.Margin.Get(); ok {
		o.SetMargin(value)
	}
}

func (o Order) Operation() optional.Option[OrderOperation] {
	operation := native.OrderGetOrderOperation(o.value)
	if !native.OrderOperationOptionalIsSet(operation) {
		return optional.None[OrderOperation]()
	}
	return optional.Some(newOrderOperation(native.OrderOperationOptionalGet(operation)))
}

func (o *Order) EnsureOperationView() OrderOperationView {
	operation := native.OrderGetOrderOperationView(&o.value)
	if !native.OrderOperationOptionalIsSet(*operation) {
		native.OrderOperationOptionalSet(operation, native.NewOrderOperation())
	}
	return newOrderOperationView(native.OrderOperationOptionalGetView(operation))
}

func (o *Order) SetOperation(operation OrderOperation) {
	native.OrderSetOrderOperation(&o.value, operation.value)
}

func (o *Order) UnsetOperation() {
	native.OrderUnsetOrderOperation(&o.value)
}

func (o Order) Position() optional.Option[OrderPosition] {
	position := native.OrderGetOrderPosition(o.value)
	if !native.OrderPositionOptionalIsSet(position) {
		return optional.None[OrderPosition]()
	}
	return optional.Some(newOrderPosition(native.OrderPositionOptionalGet(position)))
}

func (o *Order) EnsurePositionView() OrderPositionView {
	position := native.OrderGetOrderPositionView(&o.value)
	if !native.OrderPositionOptionalIsSet(*position) {
		native.OrderPositionOptionalSet(position, native.NewOrderPosition())
	}
	return newPositionView(native.OrderPositionOptionalGetView(position))
}

func (o *Order) SetPosition(position OrderPosition) {
	native.OrderSetOrderPosition(&o.value, position.value)
}

func (o *Order) UnsetPosition() {
	native.OrderUnsetOrderPosition(&o.value)
}

func (o Order) Margin() optional.Option[OrderMargin] {
	margin := native.OrderGetOrderMargin(o.value)
	if !native.OrderMarginOptionalIsSet(margin) {
		return optional.None[OrderMargin]()
	}
	return optional.Some(newOrderMargin(native.OrderMarginOptionalGet(margin)))
}

func (o *Order) EnsureMarginView() OrderMarginView {
	margin := native.OrderGetOrderMarginView(&o.value)
	if !native.OrderMarginOptionalIsSet(*margin) {
		native.OrderMarginOptionalSet(margin, native.NewOrderMargin())
	}
	return newMarginView(native.OrderMarginOptionalGetView(margin))
}

func (o *Order) SetMargin(margin OrderMargin) {
	native.OrderSetOrderMargin(&o.value, margin.value)
}

func (o *Order) UnsetMargin() {
	native.OrderUnsetOrderMargin(&o.value)
}

// EngineOrder returns this order as the standard engine order view.
func (o Order) EngineOrder() Order {
	return o
}

func (o Order) Native() native.Order {
	return o.value
}

//------------------------------------------------------------------------------
// OrderOperation

type OrderOperation struct{ value native.OrderOperation }

func NewOrderOperation() OrderOperation {
	return newOrderOperation(native.NewOrderOperation())
}

type OrderOperationParams struct {
	TradeAmount optional.Option[param.TradeAmount]
	Instrument  optional.Option[param.Instrument]
	Price       optional.Option[param.Price]
	AccountID   optional.Option[param.AccountID]
	Side        optional.Option[param.Side]
}

func NewOrderOperationFromValues(values OrderOperationParams) OrderOperation {
	o := NewOrderOperation()
	o.setValues(values)
	return o
}

func newOrderOperation(v native.OrderOperation) OrderOperation {
	return OrderOperation{value: v}
}

func (o *OrderOperation) Reset() {
	native.OrderOperationReset(&o.value)
}

func (o OrderOperation) Values() OrderOperationParams {
	return OrderOperationParams{
		TradeAmount: o.TradeAmount(),
		Instrument:  o.Instrument(),
		Price:       o.Price(),
		AccountID:   o.AccountID(),
		Side:        o.Side(),
	}
}

func (o *OrderOperation) SetValues(values OrderOperationParams) {
	o.Reset()
	o.setValues(values)
}

func (o *OrderOperation) setValues(values OrderOperationParams) {
	if value, ok := values.TradeAmount.Get(); ok {
		o.SetTradeAmount(value)
	}
	if value, ok := values.Instrument.Get(); ok {
		o.SetInstrument(value)
	}
	if value, ok := values.Price.Get(); ok {
		o.SetPrice(value)
	}
	if value, ok := values.AccountID.Get(); ok {
		o.SetAccountID(value)
	}
	if value, ok := values.Side.Get(); ok {
		o.SetSide(value)
	}
}

func (o OrderOperation) TradeAmount() optional.Option[param.TradeAmount] {
	return param.NewTradeAmountFromNative(native.OrderOperationGetTradeAmount(o.value))
}

func (o *OrderOperation) SetTradeAmount(value param.TradeAmount) {
	native.OrderOperationSetTradeAmount(&o.value, value.Native())
}

func (o *OrderOperation) UnsetTradeAmount() {
	native.OrderOperationUnsetTradeAmount(&o.value)
}

func (o OrderOperation) Instrument() optional.Option[param.Instrument] {
	return param.NewInstrumentFromNative(native.OrderOperationGetInstrument(o.value))
}

func (o *OrderOperation) SetInstrument(instrument param.Instrument) {
	native.OrderOperationSetInstrument(&o.value, instrument.Native())
}

func (o *OrderOperation) UnsetInstrument() {
	native.OrderOperationUnsetInstrument(&o.value)
}

func (o OrderOperation) Price() optional.Option[param.Price] {
	return param.NewPriceOptionFromNative(native.OrderOperationGetPrice(o.value))
}

func (o *OrderOperation) SetPrice(price param.Price) {
	native.OrderOperationSetPrice(&o.value, price.Native())
}

func (o *OrderOperation) UnsetPrice() {
	native.OrderOperationUnsetPrice(&o.value)
}

func (o OrderOperation) AccountID() optional.Option[param.AccountID] {
	return param.NewAccountIDOptionFromNative(native.OrderOperationGetAccountID(o.value))
}

func (o *OrderOperation) SetAccountID(accountID param.AccountID) {
	native.OrderOperationSetAccountID(&o.value, accountID.Native())
}

func (o *OrderOperation) UnsetAccountID() {
	native.OrderOperationUnsetAccountID(&o.value)
}

func (o OrderOperation) Side() optional.Option[param.Side] {
	return param.NewSideFromNative(native.OrderOperationGetSide(o.value))
}

func (o *OrderOperation) SetSide(side param.Side) {
	native.OrderOperationSetSide(&o.value, side.Native())
}

func (o *OrderOperation) UnsetSide() {
	native.OrderOperationUnsetSide(&o.value)
}

type OrderOperationView struct{ ref *native.OrderOperation }

func newOrderOperationView(ref *native.OrderOperation) OrderOperationView {
	return OrderOperationView{ref: ref}
}

func (v *OrderOperationView) Reset() {
	native.OrderOperationReset(v.ref)
}

func (o OrderOperationView) TradeAmount() optional.Option[param.TradeAmount] {
	return param.NewTradeAmountFromNative(native.OrderOperationGetTradeAmount(*o.ref))
}

func (o *OrderOperationView) SetTradeAmount(value param.TradeAmount) {
	native.OrderOperationSetTradeAmount(o.ref, value.Native())
}

func (o *OrderOperationView) UnsetTradeAmount() {
	native.OrderOperationUnsetTradeAmount(o.ref)
}

func (o OrderOperationView) Instrument() optional.Option[param.Instrument] {
	return param.NewInstrumentFromNative(native.OrderOperationGetInstrument(*o.ref))
}

func (o *OrderOperationView) SetInstrument(instrument param.Instrument) {
	native.OrderOperationSetInstrument(o.ref, instrument.Native())
}

func (o *OrderOperationView) UnsetInstrument() {
	native.OrderOperationUnsetInstrument(o.ref)
}

func (o OrderOperationView) Price() optional.Option[param.Price] {
	return param.NewPriceOptionFromNative(native.OrderOperationGetPrice(*o.ref))
}

func (o *OrderOperationView) SetPrice(price param.Price) {
	native.OrderOperationSetPrice(o.ref, price.Native())
}

func (o *OrderOperationView) UnsetPrice() {
	native.OrderOperationUnsetPrice(o.ref)
}

func (o OrderOperationView) AccountID() optional.Option[param.AccountID] {
	return param.NewAccountIDOptionFromNative(native.OrderOperationGetAccountID(*o.ref))
}

func (o *OrderOperationView) SetAccountID(accountID param.AccountID) {
	native.OrderOperationSetAccountID(o.ref, accountID.Native())
}

func (o *OrderOperationView) UnsetAccountID() {
	native.OrderOperationUnsetAccountID(o.ref)
}

func (o OrderOperationView) Side() optional.Option[param.Side] {
	return param.NewSideFromNative(native.OrderOperationGetSide(*o.ref))
}

func (o *OrderOperationView) SetSide(side param.Side) {
	native.OrderOperationSetSide(o.ref, side.Native())
}

func (o *OrderOperationView) UnsetSide() {
	native.OrderOperationUnsetSide(o.ref)
}

//------------------------------------------------------------------------------
// OrderPosition

type OrderPosition struct{ value native.OrderPosition }

func NewOrderPosition() OrderPosition {
	return newOrderPosition(native.NewOrderPosition())
}

type OrderPositionValues struct {
	Side          optional.Option[param.PositionSide]
	ReduceOnly    optional.Bool
	ClosePosition optional.Bool
}

func NewOrderPositionFromValues(values OrderPositionValues) OrderPosition {
	p := NewOrderPosition()
	p.setValues(values)
	return p
}

func newOrderPosition(v native.OrderPosition) OrderPosition {
	return OrderPosition{value: v}
}

func (p *OrderPosition) Reset() {
	native.OrderPositionReset(&p.value)
}

func (p OrderPosition) Values() OrderPositionValues {
	return OrderPositionValues{
		Side:          p.Side(),
		ReduceOnly:    p.ReduceOnly(),
		ClosePosition: p.ClosePosition(),
	}
}

func (p *OrderPosition) SetValues(values OrderPositionValues) {
	p.Reset()
	p.setValues(values)
}

func (p *OrderPosition) setValues(values OrderPositionValues) {
	if value, ok := values.Side.Get(); ok {
		p.SetSide(value)
	}
	if value, ok := values.ReduceOnly.Get(); ok {
		p.SetReduceOnly(value)
	}
	if value, ok := values.ClosePosition.Get(); ok {
		p.SetClosePosition(value)
	}
}

func (p OrderPosition) Side() optional.Option[param.PositionSide] {
	return param.NewPositionSideFromNative(native.OrderPositionGetSide(p.value))
}

func (p *OrderPosition) SetSide(side param.PositionSide) {
	native.OrderPositionSetSide(&p.value, native.ParamPositionSide(side))
}

func (p *OrderPosition) UnsetSide() {
	native.OrderPositionUnsetSide(&p.value)
}

func (p OrderPosition) ReduceOnly() optional.Bool {
	return convert.NewBoolOptionFromNative(native.OrderPositionGetReduceOnly(p.value))
}

func (p *OrderPosition) SetReduceOnly(reduceOnly bool) {
	native.OrderPositionSetReduceOnly(&p.value, convert.NewNativeTriBool(reduceOnly))
}

func (p *OrderPosition) UnsetReduceOnly() {
	native.OrderPositionUnsetReduceOnly(&p.value)
}

func (p OrderPosition) ClosePosition() optional.Bool {
	return convert.NewBoolOptionFromNative(native.OrderPositionGetClosePosition(p.value))
}

func (p *OrderPosition) SetClosePosition(closePosition bool) {
	native.OrderPositionSetClosePosition(&p.value, convert.NewNativeTriBool(closePosition))
}

func (p *OrderPosition) UnsetClosePosition() {
	native.OrderPositionSetClosePosition(&p.value, native.TriBoolNotSet)
}

type OrderPositionView struct{ ref *native.OrderPosition }

func newPositionView(ref *native.OrderPosition) OrderPositionView {
	return OrderPositionView{ref: ref}
}

func (v *OrderPositionView) Reset() {
	native.OrderPositionReset(v.ref)
}

func (p OrderPositionView) Side() optional.Option[param.PositionSide] {
	return param.NewPositionSideFromNative(native.OrderPositionGetSide(*p.ref))
}

func (p *OrderPositionView) SetSide(side param.PositionSide) {
	native.OrderPositionSetSide(p.ref, native.ParamPositionSide(side))
}

func (p *OrderPositionView) UnsetSide() {
	native.OrderPositionUnsetSide(p.ref)
}

func (p OrderPositionView) ReduceOnly() optional.Bool {
	return convert.NewBoolOptionFromNative(native.OrderPositionGetReduceOnly(*p.ref))
}

func (p *OrderPositionView) SetReduceOnly(reduceOnly bool) {
	native.OrderPositionSetReduceOnly(p.ref, convert.NewNativeTriBool(reduceOnly))
}

func (p *OrderPositionView) UnsetReduceOnly() {
	native.OrderPositionUnsetReduceOnly(p.ref)
}

func (p OrderPositionView) ClosePosition() optional.Bool {
	return convert.NewBoolOptionFromNative(native.OrderPositionGetClosePosition(*p.ref))
}

func (p *OrderPositionView) SetClosePosition(closePosition bool) {
	native.OrderPositionSetClosePosition(p.ref, convert.NewNativeTriBool(closePosition))
}

func (p *OrderPositionView) UnsetClosePosition() {
	native.OrderPositionUnsetClosePosition(p.ref)
}

//------------------------------------------------------------------------------
// OrderMargin

type OrderMargin struct{ value native.OrderMargin }

func NewOrderMargin() OrderMargin {
	return newOrderMargin(native.NewOrderMargin())
}

type OrderMarginValues struct {
	CollateralAsset optional.Option[param.Asset]
	AutoBorrow      optional.Bool
	Leverage        native.ParamLeverage
}

func NewOrderMarginFromValues(values OrderMarginValues) OrderMargin {
	m := NewOrderMargin()
	m.setValues(values)
	return m
}

func newOrderMargin(v native.OrderMargin) OrderMargin {
	return OrderMargin{value: v}
}

func (m *OrderMargin) Reset() {
	native.OrderMarginReset(&m.value)
}

func (m OrderMargin) Values() OrderMarginValues {
	return OrderMarginValues{
		CollateralAsset: m.CollateralAsset(),
		AutoBorrow:      m.AutoBorrow(),
		Leverage:        m.Leverage(),
	}
}

func (m *OrderMargin) SetValues(values OrderMarginValues) {
	m.Reset()
	m.setValues(values)
}

func (m *OrderMargin) setValues(values OrderMarginValues) {
	if value, ok := values.CollateralAsset.Get(); ok {
		m.SetCollateralAsset(value)
	}
	if value, ok := values.AutoBorrow.Get(); ok {
		m.SetAutoBorrow(value)
	}
	m.SetLeverage(values.Leverage)
}

func (m OrderMargin) CollateralAsset() optional.Option[param.Asset] {
	return param.NewAssetFromNative(native.OrderMarginGetCollateralAsset(m.value))
}

func (m *OrderMargin) SetCollateralAsset(asset param.Asset) {
	native.OrderMarginSetCollateralAsset(&m.value, asset.Native())
}

func (m *OrderMargin) UnsetCollateralAsset() {
	native.OrderMarginUnsetCollateralAsset(&m.value)
}

func (m OrderMargin) AutoBorrow() optional.Bool {
	return convert.NewBoolOptionFromNative(native.OrderMarginGetAutoBorrow(m.value))
}

func (m *OrderMargin) SetAutoBorrow(autoBorrow bool) {
	native.OrderMarginSetAutoBorrow(&m.value, convert.NewNativeTriBool(autoBorrow))
}

func (m *OrderMargin) UnsetAutoBorrow() {
	native.OrderMarginSetAutoBorrow(&m.value, native.TriBoolNotSet)
}

func (m OrderMargin) Leverage() native.ParamLeverage {
	return native.OrderMarginGetLeverage(m.value)
}

func (m *OrderMargin) SetLeverage(leverage native.ParamLeverage) {
	native.OrderMarginSetLeverage(&m.value, leverage)
}

func (m *OrderMargin) UnsetLeverage() {
	native.OrderMarginSetLeverage(&m.value, native.ParamLeverageNotSet)
}

type OrderMarginView struct{ ref *native.OrderMargin }

func newMarginView(ref *native.OrderMargin) OrderMarginView {
	return OrderMarginView{ref: ref}
}

func (v *OrderMarginView) Reset() {
	native.OrderMarginReset(v.ref)
}

func (m OrderMarginView) CollateralAsset() optional.Option[param.Asset] {
	return param.NewAssetFromNative(native.OrderMarginGetCollateralAsset(*m.ref))
}

func (m *OrderMarginView) SetCollateralAsset(asset param.Asset) {
	native.OrderMarginSetCollateralAsset(m.ref, asset.Native())
}

func (m *OrderMarginView) UnsetCollateralAsset() {
	native.OrderMarginUnsetCollateralAsset(m.ref)
}

func (m OrderMarginView) AutoBorrow() optional.Bool {
	return convert.NewBoolOptionFromNative(native.OrderMarginGetAutoBorrow(*m.ref))
}

func (m *OrderMarginView) SetAutoBorrow(autoBorrow bool) {
	native.OrderMarginSetAutoBorrow(m.ref, convert.NewNativeTriBool(autoBorrow))
}

func (m *OrderMarginView) UnsetAutoBorrow() {
	native.OrderMarginUnsetAutoBorrow(m.ref)
}

func (m OrderMarginView) Leverage() native.ParamLeverage {
	return native.OrderMarginGetLeverage(*m.ref)
}

func (m *OrderMarginView) SetLeverage(leverage native.ParamLeverage) {
	native.OrderMarginSetLeverage(m.ref, leverage)
}

func (m *OrderMarginView) UnsetLeverage() {
	native.OrderMarginUnsetLeverage(m.ref)
}

//------------------------------------------------------------------------------
