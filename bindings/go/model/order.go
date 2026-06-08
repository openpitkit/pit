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

// Order represents an order submitted to the pre-trade pipeline.
type Order struct {
	value  native.Order
	retain orderFieldsRetain
}

// orderFieldsRetain keeps value objects alive while the C struct's like
// OpenPitStringView fields point to their C-heap buffers. For example, see
// param/asset.go and internal/native/string.go for the full explanation.
type orderFieldsRetain struct {
	// Instrument from the order operation (holds underlying + settlement assets).
	OperationInstrument param.Instrument
	// Collateral asset from the order margin group.
	MarginCollateralAsset param.Asset
}

// NewOrder creates a new zeroed Order.
func NewOrder() Order {
	return NewOrderFromHandle(native.NewOrder())
}

// OrderValues holds the optional fields of an Order.
type OrderValues struct {
	Operation optional.Option[OrderOperation]
	Position  optional.Option[OrderPosition]
	Margin    optional.Option[OrderMargin]
}

// NewOrderFromValues creates an Order from the given values.
func NewOrderFromValues(values OrderValues) Order {
	o := NewOrder()
	o.setValues(values)
	return o
}

// NewOrderFromHandle creates an Order from a native handle.
func NewOrderFromHandle(value native.Order) Order {
	return Order{value: value}
}

// Reset zeroes out the order and clears all retained references.
func (o *Order) Reset() {
	native.OrderReset(&o.value)
	o.retain = orderFieldsRetain{}
}

// Values returns a copy of the current order fields.
func (o Order) Values() OrderValues {
	return OrderValues{
		Operation: o.Operation(),
		Position:  o.Position(),
		Margin:    o.Margin(),
	}
}

// SetValues resets the order and applies the provided values.
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

// Operation returns the optional order operation.
func (o Order) Operation() optional.Option[OrderOperation] {
	operation := native.OrderGetOrderOperation(o.value)
	if !native.OrderOperationOptionalIsSet(operation) {
		return optional.None[OrderOperation]()
	}
	return optional.Some(newOrderOperation(native.OrderOperationOptionalGet(operation)))
}

// EnsureOperationView ensures the operation exists and returns a mutable view.
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

// SetOperation sets the order operation.
func (o *Order) SetOperation(operation OrderOperation) {
	native.OrderSetOrderOperation(&o.value, operation.value)
	o.retain.OperationInstrument = operation.retainInstrument
}

// UnsetOperation clears the order operation.
func (o *Order) UnsetOperation() {
	native.OrderUnsetOrderOperation(&o.value)
	o.retain.OperationInstrument = param.Instrument{}
}

// Position returns the optional order position.
func (o Order) Position() optional.Option[OrderPosition] {
	position := native.OrderGetOrderPosition(o.value)
	if !native.OrderPositionOptionalIsSet(position) {
		return optional.None[OrderPosition]()
	}
	return optional.Some(newOrderPosition(native.OrderPositionOptionalGet(position)))
}

// EnsurePositionView ensures the position exists and returns a mutable view.
func (o *Order) EnsurePositionView() OrderPositionView {
	position := native.OrderGetOrderPositionView(&o.value)
	if !native.OrderPositionOptionalIsSet(*position) {
		native.OrderPositionOptionalSet(position, native.NewOrderPosition())
	}
	return newPositionView(native.OrderPositionOptionalGetView(position))
}

// SetPosition sets the order position.
func (o *Order) SetPosition(position OrderPosition) {
	native.OrderSetOrderPosition(&o.value, position.value)
}

// UnsetPosition clears the order position.
func (o *Order) UnsetPosition() {
	native.OrderUnsetOrderPosition(&o.value)
}

// Margin returns the optional order margin.
func (o Order) Margin() optional.Option[OrderMargin] {
	margin := native.OrderGetOrderMargin(o.value)
	if !native.OrderMarginOptionalIsSet(margin) {
		return optional.None[OrderMargin]()
	}
	return optional.Some(newOrderMargin(native.OrderMarginOptionalGet(margin)))
}

// EnsureMarginView ensures the margin exists and returns a mutable view.
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

// SetMargin sets the order margin.
func (o *Order) SetMargin(margin OrderMargin) {
	native.OrderSetOrderMargin(&o.value, margin.value)
	o.retain.MarginCollateralAsset = margin.retainCollateralAsset
}

// UnsetMargin clears the order margin.
func (o *Order) UnsetMargin() {
	native.OrderUnsetOrderMargin(&o.value)
	o.retain.MarginCollateralAsset = param.Asset{}
}

// EngineOrder returns this order as the standard engine order view.
func (o Order) EngineOrder() Order {
	return o
}

// Handle returns the underlying native handle.
func (o Order) Handle() native.Order {
	return o.value
}

//------------------------------------------------------------------------------
// OrderOperation

// OrderOperation holds the operation fields of an order.
type OrderOperation struct {
	value native.OrderOperation

	// retainInstrument keeps the Instrument (and its two constituent Assets)
	// alive while the C struct's OpenPitStringView fields point to their C-heap
	// buffers.  See param/asset.go for the full explanation of the retain
	// pattern.
	retainInstrument param.Instrument
}

// NewOrderOperation creates a new zeroed OrderOperation.
func NewOrderOperation() OrderOperation {
	return newOrderOperation(native.NewOrderOperation())
}

// OrderOperationValues holds the optional fields of an order operation.
type OrderOperationValues struct {
	TradeAmount optional.Option[param.TradeAmount]
	Instrument  optional.Option[param.Instrument]
	Price       optional.Option[param.Price]
	AccountID   optional.Option[param.AccountID]
	Side        optional.Option[param.Side]
}

// NewOrderOperationFromValues creates an OrderOperation from the given values.
func NewOrderOperationFromValues(values OrderOperationValues) OrderOperation {
	o := NewOrderOperation()
	o.setValues(values)
	return o
}

func newOrderOperation(v native.OrderOperation) OrderOperation {
	return OrderOperation{value: v}
}

// Reset zeroes out the operation.
func (o *OrderOperation) Reset() {
	native.OrderOperationReset(&o.value)
	o.retainInstrument = param.Instrument{}
}

// Values returns a copy of the current operation fields.
func (o OrderOperation) Values() OrderOperationValues {
	return OrderOperationValues{
		TradeAmount: o.TradeAmount(),
		Instrument:  o.Instrument(),
		Price:       o.Price(),
		AccountID:   o.AccountID(),
		Side:        o.Side(),
	}
}

// SetValues resets the operation and applies the provided values.
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

// TradeAmount returns the optional trade amount.
func (o OrderOperation) TradeAmount() optional.Option[param.TradeAmount] {
	return param.NewTradeAmountFromHandle(native.OrderOperationGetTradeAmount(o.value))
}

// SetTradeAmount sets the trade amount.
func (o *OrderOperation) SetTradeAmount(value param.TradeAmount) {
	native.OrderOperationSetTradeAmount(&o.value, value.Handle())
}

// UnsetTradeAmount clears the trade amount.
func (o *OrderOperation) UnsetTradeAmount() {
	native.OrderOperationUnsetTradeAmount(&o.value)
}

// Instrument returns the optional instrument.
func (o OrderOperation) Instrument() optional.Option[param.Instrument] {
	return param.NewInstrumentFromHandle(native.OrderOperationGetInstrument(o.value))
}

// SetInstrument sets the instrument.
func (o *OrderOperation) SetInstrument(instrument param.Instrument) {
	native.OrderOperationSetInstrument(&o.value, instrument.Handle())
	o.retainInstrument = instrument
}

// UnsetInstrument clears the instrument.
func (o *OrderOperation) UnsetInstrument() {
	native.OrderOperationUnsetInstrument(&o.value)
	o.retainInstrument = param.Instrument{}
}

// Price returns the optional order price.
func (o OrderOperation) Price() optional.Option[param.Price] {
	return param.NewPriceOptionFromHandle(native.OrderOperationGetPrice(o.value))
}

// SetPrice sets the order price.
func (o *OrderOperation) SetPrice(price param.Price) {
	native.OrderOperationSetPrice(&o.value, price.Handle())
}

// UnsetPrice clears the order price.
func (o *OrderOperation) UnsetPrice() {
	native.OrderOperationUnsetPrice(&o.value)
}

// AccountID returns the optional account ID.
func (o OrderOperation) AccountID() optional.Option[param.AccountID] {
	return param.NewAccountIDOptionFromHandle(native.OrderOperationGetAccountID(o.value))
}

// SetAccountID sets the account ID.
func (o *OrderOperation) SetAccountID(accountID param.AccountID) {
	native.OrderOperationSetAccountID(&o.value, accountID.Handle())
}

// UnsetAccountID clears the account ID.
func (o *OrderOperation) UnsetAccountID() {
	native.OrderOperationUnsetAccountID(&o.value)
}

// Side returns the optional order side.
func (o OrderOperation) Side() optional.Option[param.Side] {
	return param.NewSideFromHandle(native.OrderOperationGetSide(o.value))
}

// SetSide sets the order side.
func (o *OrderOperation) SetSide(side param.Side) {
	native.OrderOperationSetSide(&o.value, side.Handle())
}

// UnsetSide clears the order side.
func (o *OrderOperation) UnsetSide() {
	native.OrderOperationUnsetSide(&o.value)
}

// OrderOperationView is a mutable view into an operation owned by an Order.
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

// Reset zeroes out the operation view.
func (v *OrderOperationView) Reset() {
	native.OrderOperationReset(v.ref)
	*v.retainInstrument = param.Instrument{}
}

// TradeAmount returns the optional trade amount from the view.
func (v OrderOperationView) TradeAmount() optional.Option[param.TradeAmount] {
	return param.NewTradeAmountFromHandle(native.OrderOperationGetTradeAmount(*v.ref))
}

// SetTradeAmount sets the trade amount on the view.
func (v *OrderOperationView) SetTradeAmount(value param.TradeAmount) {
	native.OrderOperationSetTradeAmount(v.ref, value.Handle())
}

// UnsetTradeAmount clears the trade amount on the view.
func (v *OrderOperationView) UnsetTradeAmount() {
	native.OrderOperationUnsetTradeAmount(v.ref)
}

// Instrument returns the optional instrument from the view.
func (v OrderOperationView) Instrument() optional.Option[param.Instrument] {
	return param.NewInstrumentFromHandle(native.OrderOperationGetInstrument(*v.ref))
}

// SetInstrument sets the instrument on the view.
func (v *OrderOperationView) SetInstrument(instrument param.Instrument) {
	native.OrderOperationSetInstrument(v.ref, instrument.Handle())
	*v.retainInstrument = instrument
}

// UnsetInstrument clears the instrument on the view.
func (v *OrderOperationView) UnsetInstrument() {
	native.OrderOperationUnsetInstrument(v.ref)
	*v.retainInstrument = param.Instrument{}
}

// Price returns the optional order price from the view.
func (v OrderOperationView) Price() optional.Option[param.Price] {
	return param.NewPriceOptionFromHandle(native.OrderOperationGetPrice(*v.ref))
}

// SetPrice sets the price on the view.
func (v *OrderOperationView) SetPrice(price param.Price) {
	native.OrderOperationSetPrice(v.ref, price.Handle())
}

// UnsetPrice clears the price on the view.
func (v *OrderOperationView) UnsetPrice() {
	native.OrderOperationUnsetPrice(v.ref)
}

// AccountID returns the optional account ID from the view.
func (v OrderOperationView) AccountID() optional.Option[param.AccountID] {
	return param.NewAccountIDOptionFromHandle(native.OrderOperationGetAccountID(*v.ref))
}

// SetAccountID sets the account ID on the view.
func (v *OrderOperationView) SetAccountID(accountID param.AccountID) {
	native.OrderOperationSetAccountID(v.ref, accountID.Handle())
}

// UnsetAccountID clears the account ID on the view.
func (v *OrderOperationView) UnsetAccountID() {
	native.OrderOperationUnsetAccountID(v.ref)
}

// Side returns the optional order side from the view.
func (v OrderOperationView) Side() optional.Option[param.Side] {
	return param.NewSideFromHandle(native.OrderOperationGetSide(*v.ref))
}

// SetSide sets the order side on the view.
func (v *OrderOperationView) SetSide(side param.Side) {
	native.OrderOperationSetSide(v.ref, side.Handle())
}

// UnsetSide clears the order side on the view.
func (v *OrderOperationView) UnsetSide() {
	native.OrderOperationUnsetSide(v.ref)
}

//------------------------------------------------------------------------------
// OrderPosition

// OrderPosition holds position-specific fields of an order.
type OrderPosition struct{ value native.OrderPosition }

// NewOrderPosition creates a new zeroed OrderPosition.
func NewOrderPosition() OrderPosition {
	return newOrderPosition(native.NewOrderPosition())
}

// OrderPositionValues holds the optional position fields of an order.
type OrderPositionValues struct {
	Side          optional.Option[param.PositionSide]
	ReduceOnly    optional.Bool
	ClosePosition optional.Bool
}

// NewOrderPositionFromValues creates an OrderPosition from the given values.
func NewOrderPositionFromValues(values OrderPositionValues) OrderPosition {
	p := NewOrderPosition()
	p.setValues(values)
	return p
}

func newOrderPosition(v native.OrderPosition) OrderPosition {
	return OrderPosition{value: v}
}

// Reset zeroes out the position.
func (p *OrderPosition) Reset() {
	native.OrderPositionReset(&p.value)
}

// Values returns a copy of the current position fields.
func (p OrderPosition) Values() OrderPositionValues {
	return OrderPositionValues{
		Side:          p.Side(),
		ReduceOnly:    p.ReduceOnly(),
		ClosePosition: p.ClosePosition(),
	}
}

// SetValues resets the position and applies the provided values.
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

// Side returns the optional position side.
func (p OrderPosition) Side() optional.Option[param.PositionSide] {
	return param.NewPositionSideFromHandle(native.OrderPositionGetSide(p.value))
}

// SetSide sets the position side.
func (p *OrderPosition) SetSide(side param.PositionSide) {
	native.OrderPositionSetSide(&p.value, native.ParamPositionSide(side))
}

// UnsetSide clears the position side.
func (p *OrderPosition) UnsetSide() {
	native.OrderPositionUnsetSide(&p.value)
}

// ReduceOnly returns the optional reduce-only flag.
func (p OrderPosition) ReduceOnly() optional.Bool {
	return convert.NewBoolOptionFromNative(native.OrderPositionGetReduceOnly(p.value))
}

// SetReduceOnly sets the reduce-only flag.
func (p *OrderPosition) SetReduceOnly(reduceOnly bool) {
	native.OrderPositionSetReduceOnly(&p.value, convert.NewNativeTriBool(reduceOnly))
}

// UnsetReduceOnly clears the reduce-only flag.
func (p *OrderPosition) UnsetReduceOnly() {
	native.OrderPositionUnsetReduceOnly(&p.value)
}

// ClosePosition returns the optional close-position flag.
func (p OrderPosition) ClosePosition() optional.Bool {
	return convert.NewBoolOptionFromNative(native.OrderPositionGetClosePosition(p.value))
}

// SetClosePosition sets the close-position flag.
func (p *OrderPosition) SetClosePosition(closePosition bool) {
	native.OrderPositionSetClosePosition(&p.value, convert.NewNativeTriBool(closePosition))
}

// UnsetClosePosition clears the close-position flag.
func (p *OrderPosition) UnsetClosePosition() {
	native.OrderPositionSetClosePosition(&p.value, native.TriBoolNotSet)
}

// OrderPositionView is a mutable view into a position owned by an Order.
type OrderPositionView struct{ ref *native.OrderPosition }

func newPositionView(ref *native.OrderPosition) OrderPositionView {
	return OrderPositionView{ref: ref}
}

// Reset zeroes out the position view.
func (v *OrderPositionView) Reset() {
	native.OrderPositionReset(v.ref)
}

// Side returns the optional position side from the view.
func (v OrderPositionView) Side() optional.Option[param.PositionSide] {
	return param.NewPositionSideFromHandle(native.OrderPositionGetSide(*v.ref))
}

// SetSide sets the position side on the view.
func (v *OrderPositionView) SetSide(side param.PositionSide) {
	native.OrderPositionSetSide(v.ref, native.ParamPositionSide(side))
}

// UnsetSide clears the position side on the view.
func (v *OrderPositionView) UnsetSide() {
	native.OrderPositionUnsetSide(v.ref)
}

// ReduceOnly returns the optional reduce-only flag from the view.
func (v OrderPositionView) ReduceOnly() optional.Bool {
	return convert.NewBoolOptionFromNative(native.OrderPositionGetReduceOnly(*v.ref))
}

// SetReduceOnly sets the reduce-only flag on the view.
func (v *OrderPositionView) SetReduceOnly(reduceOnly bool) {
	native.OrderPositionSetReduceOnly(v.ref, convert.NewNativeTriBool(reduceOnly))
}

// UnsetReduceOnly clears the reduce-only flag on the view.
func (v *OrderPositionView) UnsetReduceOnly() {
	native.OrderPositionUnsetReduceOnly(v.ref)
}

// ClosePosition returns the optional close-position flag from the view.
func (v OrderPositionView) ClosePosition() optional.Bool {
	return convert.NewBoolOptionFromNative(native.OrderPositionGetClosePosition(*v.ref))
}

// SetClosePosition sets the close-position flag on the view.
func (v *OrderPositionView) SetClosePosition(closePosition bool) {
	native.OrderPositionSetClosePosition(v.ref, convert.NewNativeTriBool(closePosition))
}

// UnsetClosePosition clears the close-position flag on the view.
func (v *OrderPositionView) UnsetClosePosition() {
	native.OrderPositionUnsetClosePosition(v.ref)
}

//------------------------------------------------------------------------------
// OrderMargin

// OrderMargin holds margin-specific fields of an order.
type OrderMargin struct {
	value native.OrderMargin

	// retainCollateralAsset keeps the Asset alive while the C struct's
	// OpenPitStringView points to its C-heap buffer.  See param/asset.go for the
	// full explanation of the retain pattern.
	retainCollateralAsset param.Asset
}

// NewOrderMargin creates a new zeroed OrderMargin.
func NewOrderMargin() OrderMargin {
	return newOrderMargin(native.NewOrderMargin())
}

// OrderMarginValues holds the optional margin fields of an order.
type OrderMarginValues struct {
	CollateralAsset optional.Option[param.Asset]
	AutoBorrow      optional.Bool
	Leverage        optional.Option[param.Leverage]
}

// NewOrderMarginFromValues creates an OrderMargin from the given values.
func NewOrderMarginFromValues(values OrderMarginValues) OrderMargin {
	m := NewOrderMargin()
	m.setValues(values)
	return m
}

func newOrderMargin(v native.OrderMargin) OrderMargin {
	return OrderMargin{value: v}
}

// Reset zeroes out the margin.
func (m *OrderMargin) Reset() {
	native.OrderMarginReset(&m.value)
	m.retainCollateralAsset = param.Asset{}
}

// Values returns a copy of the current margin fields.
func (m OrderMargin) Values() OrderMarginValues {
	return OrderMarginValues{
		CollateralAsset: m.CollateralAsset(),
		AutoBorrow:      m.AutoBorrow(),
		Leverage:        m.Leverage(),
	}
}

// SetValues resets the margin and applies the provided values.
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

// CollateralAsset returns the optional collateral asset.
func (m OrderMargin) CollateralAsset() optional.Option[param.Asset] {
	return param.NewAssetFromHandle(native.OrderMarginGetCollateralAsset(m.value))
}

// SetCollateralAsset sets the collateral asset.
func (m *OrderMargin) SetCollateralAsset(asset param.Asset) {
	native.OrderMarginSetCollateralAsset(&m.value, asset.Handle())
	m.retainCollateralAsset = asset
}

// UnsetCollateralAsset clears the collateral asset.
func (m *OrderMargin) UnsetCollateralAsset() {
	native.OrderMarginUnsetCollateralAsset(&m.value)
	m.retainCollateralAsset = param.Asset{}
}

// AutoBorrow returns the optional auto-borrow flag.
func (m OrderMargin) AutoBorrow() optional.Bool {
	return convert.NewBoolOptionFromNative(native.OrderMarginGetAutoBorrow(m.value))
}

// SetAutoBorrow sets the auto-borrow flag.
func (m *OrderMargin) SetAutoBorrow(autoBorrow bool) {
	native.OrderMarginSetAutoBorrow(&m.value, convert.NewNativeTriBool(autoBorrow))
}

// UnsetAutoBorrow clears the auto-borrow flag.
func (m *OrderMargin) UnsetAutoBorrow() {
	native.OrderMarginSetAutoBorrow(&m.value, native.TriBoolNotSet)
}

// Leverage returns the optional leverage.
func (m OrderMargin) Leverage() optional.Option[param.Leverage] {
	return param.NewLeverageOptionFromHandle(native.OrderMarginGetLeverage(m.value))
}

// SetLeverage sets the leverage.
func (m *OrderMargin) SetLeverage(leverage param.Leverage) {
	native.OrderMarginSetLeverage(&m.value, leverage.Handle())
}

// UnsetLeverage clears the leverage.
func (m *OrderMargin) UnsetLeverage() {
	native.OrderMarginSetLeverage(&m.value, native.ParamLeverageNotSet)
}

// OrderMarginView is a mutable view into a margin owned by an Order.
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

// Reset zeroes out the margin view.
func (v *OrderMarginView) Reset() {
	native.OrderMarginReset(v.ref)
	*v.retainCollateralAsset = param.Asset{}
}

// CollateralAsset returns the optional collateral asset from the view.
func (v OrderMarginView) CollateralAsset() optional.Option[param.Asset] {
	return param.NewAssetFromHandle(native.OrderMarginGetCollateralAsset(*v.ref))
}

// SetCollateralAsset sets the collateral asset on the view.
func (v *OrderMarginView) SetCollateralAsset(asset param.Asset) {
	native.OrderMarginSetCollateralAsset(v.ref, asset.Handle())
	*v.retainCollateralAsset = asset
}

// UnsetCollateralAsset clears the collateral asset on the view.
func (v *OrderMarginView) UnsetCollateralAsset() {
	native.OrderMarginUnsetCollateralAsset(v.ref)
	*v.retainCollateralAsset = param.Asset{}
}

// AutoBorrow returns the optional auto-borrow flag from the view.
func (v OrderMarginView) AutoBorrow() optional.Bool {
	return convert.NewBoolOptionFromNative(native.OrderMarginGetAutoBorrow(*v.ref))
}

// SetAutoBorrow sets the auto-borrow flag on the view.
func (v *OrderMarginView) SetAutoBorrow(autoBorrow bool) {
	native.OrderMarginSetAutoBorrow(v.ref, convert.NewNativeTriBool(autoBorrow))
}

// UnsetAutoBorrow clears the auto-borrow flag on the view.
func (v *OrderMarginView) UnsetAutoBorrow() {
	native.OrderMarginUnsetAutoBorrow(v.ref)
}

// Leverage returns the optional leverage from the view.
func (v OrderMarginView) Leverage() optional.Option[param.Leverage] {
	return param.NewLeverageOptionFromHandle(native.OrderMarginGetLeverage(*v.ref))
}

// SetLeverage sets the leverage on the view.
func (v *OrderMarginView) SetLeverage(leverage param.Leverage) {
	native.OrderMarginSetLeverage(v.ref, leverage.Handle())
}

// UnsetLeverage clears the leverage on the view.
func (v *OrderMarginView) UnsetLeverage() {
	native.OrderMarginUnsetLeverage(v.ref)
}

//------------------------------------------------------------------------------
