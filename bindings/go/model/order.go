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

type Order struct {
	value  native.Order
	retain orderFieldsRetain
}

// orderFieldsRetain keeps value objects alive while the C struct's like
// PitStringView fields point to their C-heap buffers. For example, see
// param/asset.go and internal/native/string.go for the full explanation.
type orderFieldsRetain struct {
	// Instrument from the order operation (holds underlying + settlement assets).
	OperationInstrument param.Instrument
	// Collateral asset from the order margin group.
	MarginCollateralAsset param.Asset
}

func NewOrder() Order {
	return NewOrderFromHandle(native.NewOrder())
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

func NewOrderFromHandle(value native.Order) Order {
	return Order{value: value}
}

func (o *Order) Reset() {
	native.OrderReset(&o.value)
	o.retain = orderFieldsRetain{}
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
	return newOrderOperationView(
		native.OrderOperationOptionalGetView(operation),
		&o.retain.OperationInstrument,
	)
}

func (o *Order) SetOperation(operation OrderOperation) {
	native.OrderSetOrderOperation(&o.value, operation.value)
	o.retain.OperationInstrument = operation.retainInstrument
}

func (o *Order) UnsetOperation() {
	native.OrderUnsetOrderOperation(&o.value)
	o.retain.OperationInstrument = param.Instrument{}
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
	return newMarginView(
		native.OrderMarginOptionalGetView(margin),
		&o.retain.MarginCollateralAsset,
	)
}

func (o *Order) SetMargin(margin OrderMargin) {
	native.OrderSetOrderMargin(&o.value, margin.value)
	o.retain.MarginCollateralAsset = margin.retainCollateralAsset
}

func (o *Order) UnsetMargin() {
	native.OrderUnsetOrderMargin(&o.value)
	o.retain.MarginCollateralAsset = param.Asset{}
}

// EngineOrder returns this order as the standard engine order view.
func (o Order) EngineOrder() Order {
	return o
}

func (o Order) Handle() native.Order {
	return o.value
}

//------------------------------------------------------------------------------
// OrderOperation

type OrderOperation struct {
	value native.OrderOperation

	// retainInstrument keeps the Instrument (and its two constituent Assets)
	// alive while the C struct's PitStringView fields point to their C-heap
	// buffers.  See param/asset.go for the full explanation of the retain
	// pattern.
	retainInstrument param.Instrument
}

func NewOrderOperation() OrderOperation {
	return newOrderOperation(native.NewOrderOperation())
}

type OrderOperationValues struct {
	TradeAmount optional.Option[param.TradeAmount]
	Instrument  optional.Option[param.Instrument]
	Price       optional.Option[param.Price]
	AccountID   optional.Option[param.AccountID]
	Side        optional.Option[param.Side]
}

func NewOrderOperationFromValues(values OrderOperationValues) OrderOperation {
	o := NewOrderOperation()
	o.setValues(values)
	return o
}

func newOrderOperation(v native.OrderOperation) OrderOperation {
	return OrderOperation{value: v}
}

func (o *OrderOperation) Reset() {
	native.OrderOperationReset(&o.value)
	o.retainInstrument = param.Instrument{}
}

func (o OrderOperation) Values() OrderOperationValues {
	return OrderOperationValues{
		TradeAmount: o.TradeAmount(),
		Instrument:  o.Instrument(),
		Price:       o.Price(),
		AccountID:   o.AccountID(),
		Side:        o.Side(),
	}
}

func (o *OrderOperation) SetValues(values OrderOperationValues) {
	o.Reset()
	o.setValues(values)
}

func (o *OrderOperation) setValues(values OrderOperationValues) {
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
	return param.NewTradeAmountFromHandle(native.OrderOperationGetTradeAmount(o.value))
}

func (o *OrderOperation) SetTradeAmount(value param.TradeAmount) {
	native.OrderOperationSetTradeAmount(&o.value, value.Handle())
}

func (o *OrderOperation) UnsetTradeAmount() {
	native.OrderOperationUnsetTradeAmount(&o.value)
}

func (o OrderOperation) Instrument() optional.Option[param.Instrument] {
	return param.NewInstrumentFromHandle(native.OrderOperationGetInstrument(o.value))
}

func (o *OrderOperation) SetInstrument(instrument param.Instrument) {
	native.OrderOperationSetInstrument(&o.value, instrument.Handle())
	o.retainInstrument = instrument
}

func (o *OrderOperation) UnsetInstrument() {
	native.OrderOperationUnsetInstrument(&o.value)
	o.retainInstrument = param.Instrument{}
}

func (o OrderOperation) Price() optional.Option[param.Price] {
	return param.NewPriceOptionFromHandle(native.OrderOperationGetPrice(o.value))
}

func (o *OrderOperation) SetPrice(price param.Price) {
	native.OrderOperationSetPrice(&o.value, price.Handle())
}

func (o *OrderOperation) UnsetPrice() {
	native.OrderOperationUnsetPrice(&o.value)
}

func (o OrderOperation) AccountID() optional.Option[param.AccountID] {
	return param.NewAccountIDOptionFromHandle(native.OrderOperationGetAccountID(o.value))
}

func (o *OrderOperation) SetAccountID(accountID param.AccountID) {
	native.OrderOperationSetAccountID(&o.value, accountID.Handle())
}

func (o *OrderOperation) UnsetAccountID() {
	native.OrderOperationUnsetAccountID(&o.value)
}

func (o OrderOperation) Side() optional.Option[param.Side] {
	return param.NewSideFromHandle(native.OrderOperationGetSide(o.value))
}

func (o *OrderOperation) SetSide(side param.Side) {
	native.OrderOperationSetSide(&o.value, side.Handle())
}

func (o *OrderOperation) UnsetSide() {
	native.OrderOperationUnsetSide(&o.value)
}

type OrderOperationView struct {
	ref *native.OrderOperation
	// retainInstrument points to the owning Order's retainOperationInstrument
	// so that SetInstrument/UnsetInstrument on this view propagate retention
	// to the parent automatically.
	retainInstrument *param.Instrument
}

func newOrderOperationView(
	ref *native.OrderOperation,
	retainInstrument *param.Instrument,
) OrderOperationView {
	return OrderOperationView{ref: ref, retainInstrument: retainInstrument}
}

func (v *OrderOperationView) Reset() {
	native.OrderOperationReset(v.ref)
	*v.retainInstrument = param.Instrument{}
}

func (v OrderOperationView) TradeAmount() optional.Option[param.TradeAmount] {
	return param.NewTradeAmountFromHandle(native.OrderOperationGetTradeAmount(*v.ref))
}

func (v *OrderOperationView) SetTradeAmount(value param.TradeAmount) {
	native.OrderOperationSetTradeAmount(v.ref, value.Handle())
}

func (v *OrderOperationView) UnsetTradeAmount() {
	native.OrderOperationUnsetTradeAmount(v.ref)
}

func (v OrderOperationView) Instrument() optional.Option[param.Instrument] {
	return param.NewInstrumentFromHandle(native.OrderOperationGetInstrument(*v.ref))
}

func (v *OrderOperationView) SetInstrument(instrument param.Instrument) {
	native.OrderOperationSetInstrument(v.ref, instrument.Handle())
	*v.retainInstrument = instrument
}

func (v *OrderOperationView) UnsetInstrument() {
	native.OrderOperationUnsetInstrument(v.ref)
	*v.retainInstrument = param.Instrument{}
}

func (v OrderOperationView) Price() optional.Option[param.Price] {
	return param.NewPriceOptionFromHandle(native.OrderOperationGetPrice(*v.ref))
}

func (v *OrderOperationView) SetPrice(price param.Price) {
	native.OrderOperationSetPrice(v.ref, price.Handle())
}

func (v *OrderOperationView) UnsetPrice() {
	native.OrderOperationUnsetPrice(v.ref)
}

func (v OrderOperationView) AccountID() optional.Option[param.AccountID] {
	return param.NewAccountIDOptionFromHandle(native.OrderOperationGetAccountID(*v.ref))
}

func (v *OrderOperationView) SetAccountID(accountID param.AccountID) {
	native.OrderOperationSetAccountID(v.ref, accountID.Handle())
}

func (v *OrderOperationView) UnsetAccountID() {
	native.OrderOperationUnsetAccountID(v.ref)
}

func (v OrderOperationView) Side() optional.Option[param.Side] {
	return param.NewSideFromHandle(native.OrderOperationGetSide(*v.ref))
}

func (v *OrderOperationView) SetSide(side param.Side) {
	native.OrderOperationSetSide(v.ref, side.Handle())
}

func (v *OrderOperationView) UnsetSide() {
	native.OrderOperationUnsetSide(v.ref)
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
	return param.NewPositionSideFromHandle(native.OrderPositionGetSide(p.value))
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

func (v OrderPositionView) Side() optional.Option[param.PositionSide] {
	return param.NewPositionSideFromHandle(native.OrderPositionGetSide(*v.ref))
}

func (v *OrderPositionView) SetSide(side param.PositionSide) {
	native.OrderPositionSetSide(v.ref, native.ParamPositionSide(side))
}

func (v *OrderPositionView) UnsetSide() {
	native.OrderPositionUnsetSide(v.ref)
}

func (v OrderPositionView) ReduceOnly() optional.Bool {
	return convert.NewBoolOptionFromNative(native.OrderPositionGetReduceOnly(*v.ref))
}

func (v *OrderPositionView) SetReduceOnly(reduceOnly bool) {
	native.OrderPositionSetReduceOnly(v.ref, convert.NewNativeTriBool(reduceOnly))
}

func (v *OrderPositionView) UnsetReduceOnly() {
	native.OrderPositionUnsetReduceOnly(v.ref)
}

func (v OrderPositionView) ClosePosition() optional.Bool {
	return convert.NewBoolOptionFromNative(native.OrderPositionGetClosePosition(*v.ref))
}

func (v *OrderPositionView) SetClosePosition(closePosition bool) {
	native.OrderPositionSetClosePosition(v.ref, convert.NewNativeTriBool(closePosition))
}

func (v *OrderPositionView) UnsetClosePosition() {
	native.OrderPositionUnsetClosePosition(v.ref)
}

//------------------------------------------------------------------------------
// OrderMargin

type OrderMargin struct {
	value native.OrderMargin

	// retainCollateralAsset keeps the Asset alive while the C struct's
	// PitStringView points to its C-heap buffer.  See param/asset.go for the
	// full explanation of the retain pattern.
	retainCollateralAsset param.Asset
}

func NewOrderMargin() OrderMargin {
	return newOrderMargin(native.NewOrderMargin())
}

type OrderMarginValues struct {
	CollateralAsset optional.Option[param.Asset]
	AutoBorrow      optional.Bool
	Leverage        optional.Option[param.Leverage]
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
	m.retainCollateralAsset = param.Asset{}
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
	if value, ok := values.Leverage.Get(); ok {
		m.SetLeverage(value)
	}
}

func (m OrderMargin) CollateralAsset() optional.Option[param.Asset] {
	return param.NewAssetFromHandle(native.OrderMarginGetCollateralAsset(m.value))
}

func (m *OrderMargin) SetCollateralAsset(asset param.Asset) {
	native.OrderMarginSetCollateralAsset(&m.value, asset.Handle())
	m.retainCollateralAsset = asset
}

func (m *OrderMargin) UnsetCollateralAsset() {
	native.OrderMarginUnsetCollateralAsset(&m.value)
	m.retainCollateralAsset = param.Asset{}
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

func (m OrderMargin) Leverage() optional.Option[param.Leverage] {
	return param.NewLeverageOptionFromHandle(native.OrderMarginGetLeverage(m.value))
}

func (m *OrderMargin) SetLeverage(leverage param.Leverage) {
	native.OrderMarginSetLeverage(&m.value, leverage.Handle())
}

func (m *OrderMargin) UnsetLeverage() {
	native.OrderMarginSetLeverage(&m.value, native.ParamLeverageNotSet)
}

type OrderMarginView struct {
	ref *native.OrderMargin
	// retainCollateralAsset points to the owning Order's
	// retainMarginCollateralAsset field so that Set/Unset calls on this view
	// propagate retention to the parent automatically.
	retainCollateralAsset *param.Asset
}

func newMarginView(ref *native.OrderMargin, retainCollateralAsset *param.Asset) OrderMarginView {
	return OrderMarginView{ref: ref, retainCollateralAsset: retainCollateralAsset}
}

func (v *OrderMarginView) Reset() {
	native.OrderMarginReset(v.ref)
	*v.retainCollateralAsset = param.Asset{}
}

func (v OrderMarginView) CollateralAsset() optional.Option[param.Asset] {
	return param.NewAssetFromHandle(native.OrderMarginGetCollateralAsset(*v.ref))
}

func (v *OrderMarginView) SetCollateralAsset(asset param.Asset) {
	native.OrderMarginSetCollateralAsset(v.ref, asset.Handle())
	*v.retainCollateralAsset = asset
}

func (v *OrderMarginView) UnsetCollateralAsset() {
	native.OrderMarginUnsetCollateralAsset(v.ref)
	*v.retainCollateralAsset = param.Asset{}
}

func (v OrderMarginView) AutoBorrow() optional.Bool {
	return convert.NewBoolOptionFromNative(native.OrderMarginGetAutoBorrow(*v.ref))
}

func (v *OrderMarginView) SetAutoBorrow(autoBorrow bool) {
	native.OrderMarginSetAutoBorrow(v.ref, convert.NewNativeTriBool(autoBorrow))
}

func (v *OrderMarginView) UnsetAutoBorrow() {
	native.OrderMarginUnsetAutoBorrow(v.ref)
}

func (v OrderMarginView) Leverage() optional.Option[param.Leverage] {
	return param.NewLeverageOptionFromHandle(native.OrderMarginGetLeverage(*v.ref))
}

func (v *OrderMarginView) SetLeverage(leverage param.Leverage) {
	native.OrderMarginSetLeverage(v.ref, leverage.Handle())
}

func (v *OrderMarginView) UnsetLeverage() {
	native.OrderMarginUnsetLeverage(v.ref)
}

//------------------------------------------------------------------------------
