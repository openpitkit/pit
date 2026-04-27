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

package native

/*
#include "pit.h"
*/
import "C"
import "unsafe"

//------------------------------------------------------------------------------
// Order

func NewOrder() Order {
	return Order{}
}

func OrderReset(o *Order) {
	*o = NewOrder()
}

func OrderGetOrderOperation(o Order) OrderOperationOptional {
	return o.operation
}

func OrderGetOrderOperationView(o *Order) *OrderOperationOptional {
	return &o.operation
}

func OrderSetOrderOperation(o *Order, operation OrderOperation) {
	OrderOperationOptionalSet(&o.operation, operation)
}

func OrderUnsetOrderOperation(o *Order) {
	OrderOperationOptionalReset(&o.operation)
}

func OrderGetOrderPosition(o Order) OrderPositionOptional {
	return o.position
}

func OrderGetOrderPositionView(o *Order) *OrderPositionOptional {
	return &o.position
}

func OrderSetOrderPosition(o *Order, position OrderPosition) {
	OrderPositionOptionalSet(&o.position, position)
}

func OrderUnsetOrderPosition(o *Order) {
	OrderPositionOptionalReset(&o.position)
}

func OrderGetOrderMargin(o Order) OrderMarginOptional {
	return o.margin
}

func OrderGetOrderMarginView(o *Order) *OrderMarginOptional {
	return &o.margin
}

func OrderSetOrderMargin(o *Order, margin OrderMargin) {
	OrderMarginOptionalSet(&o.margin, margin)
}

func OrderUnsetOrderMargin(o *Order) {
	OrderMarginOptionalReset(&o.margin)
}

func OrderGetUserData(o Order) unsafe.Pointer {
	return o.user_data
}

func OrderSetUserData(o *Order, userData unsafe.Pointer) {
	o.user_data = userData
}

//------------------------------------------------------------------------------
// OrderOperationOptional

func NewOrderOperationOptional() OrderOperationOptional {
	return OrderOperationOptional{}
}

func OrderOperationOptionalReset(value *OrderOperationOptional) {
	OrderOperationReset(&value.value)
	value.is_set = false
}

func OrderOperationOptionalIsSet(value OrderOperationOptional) bool {
	return bool(value.is_set)
}

func OrderOperationOptionalGet(value OrderOperationOptional) OrderOperation {
	return value.value
}

func OrderOperationOptionalGetView(value *OrderOperationOptional) *OrderOperation {
	return &value.value
}

func OrderOperationOptionalSet(value *OrderOperationOptional, operation OrderOperation) {
	value.value = operation
	value.is_set = true
}

//------------------------------------------------------------------------------
// OrderPositionOptional

func NewOrderPositionOptional() OrderPositionOptional {
	return OrderPositionOptional{}
}

func OrderPositionOptionalReset(value *OrderPositionOptional) {
	OrderPositionReset(&value.value)
	value.is_set = false
}

func OrderPositionOptionalIsSet(value OrderPositionOptional) bool {
	return bool(value.is_set)
}

func OrderPositionOptionalGet(value OrderPositionOptional) OrderPosition {
	return value.value
}

func OrderPositionOptionalGetView(value *OrderPositionOptional) *OrderPosition {
	return &value.value
}

func OrderPositionOptionalSet(value *OrderPositionOptional, position OrderPosition) {
	value.value = position
	value.is_set = true
}

//------------------------------------------------------------------------------
// OrderMarginOptional

func NewOrderMarginOptional() OrderMarginOptional {
	return OrderMarginOptional{}
}

func OrderMarginOptionalReset(value *OrderMarginOptional) {
	OrderMarginReset(&value.value)
	value.is_set = false
}

func OrderMarginOptionalIsSet(value OrderMarginOptional) bool {
	return bool(value.is_set)
}

func OrderMarginOptionalGet(value OrderMarginOptional) OrderMargin {
	return value.value
}

func OrderMarginOptionalGetView(value *OrderMarginOptional) *OrderMargin {
	return &value.value
}

func OrderMarginOptionalSet(value *OrderMarginOptional, margin OrderMargin) {
	value.value = margin
	value.is_set = true
}

//------------------------------------------------------------------------------
// OrderOperation

func NewOrderOperation() OrderOperation {
	return OrderOperation{}
}

func OrderOperationReset(o *OrderOperation) {
	*o = NewOrderOperation()
}

func OrderOperationGetTradeAmount(o OrderOperation) ParamTradeAmount {
	return o.trade_amount
}

func OrderOperationSetTradeAmount(o *OrderOperation, value ParamTradeAmount) {
	o.trade_amount = value
}

func OrderOperationUnsetTradeAmount(o *OrderOperation) {
	o.trade_amount = ParamTradeAmount{}
}

func OrderOperationGetInstrument(o OrderOperation) Instrument {
	return o.instrument
}

func OrderOperationSetInstrument(o *OrderOperation, instrument Instrument) {
	o.instrument = instrument
}

func OrderOperationUnsetInstrument(o *OrderOperation) {
	o.instrument = Instrument{}
}

func OrderOperationGetPrice(o OrderOperation) ParamPriceOptional {
	return o.price
}

func OrderOperationSetPrice(o *OrderOperation, price ParamPrice) {
	o.price.value = price
	o.price.is_set = true
}

func OrderOperationUnsetPrice(o *OrderOperation) {
	o.price = ParamPriceOptional{}
}

func OrderOperationGetAccountID(o OrderOperation) ParamAccountIDOptional {
	return o.account_id
}

func OrderOperationSetAccountID(o *OrderOperation, accountID ParamAccountID) {
	o.account_id.value = accountID
	o.account_id.is_set = true
}

func OrderOperationUnsetAccountID(o *OrderOperation) {
	o.account_id = ParamAccountIDOptional{}
}

func OrderOperationGetSide(o OrderOperation) ParamSide {
	return o.side
}

func OrderOperationSetSide(o *OrderOperation, side ParamSide) {
	o.side = side
}

func OrderOperationUnsetSide(o *OrderOperation) {
	o.side = ParamSideNotSet
}

//------------------------------------------------------------------------------
// OrderPosition

func NewOrderPosition() OrderPosition {
	return OrderPosition{}
}

func OrderPositionReset(p *OrderPosition) {
	*p = NewOrderPosition()
}

func OrderPositionGetSide(p OrderPosition) ParamPositionSide {
	return p.position_side
}

func OrderPositionSetSide(p *OrderPosition, side ParamPositionSide) {
	p.position_side = side
}

func OrderPositionUnsetSide(p *OrderPosition) {
	p.position_side = ParamPositionSideNotSet
}

func OrderPositionGetReduceOnly(p OrderPosition) TriBool {
	return p.reduce_only
}

func OrderPositionSetReduceOnly(p *OrderPosition, reduceOnly TriBool) {
	p.reduce_only = reduceOnly
}

func OrderPositionUnsetReduceOnly(p *OrderPosition) {
	p.reduce_only = TriBoolNotSet
}

func OrderPositionGetClosePosition(p OrderPosition) TriBool {
	return p.close_position
}

func OrderPositionSetClosePosition(p *OrderPosition, closePosition TriBool) {
	p.close_position = closePosition
}

func OrderPositionUnsetClosePosition(p *OrderPosition) {
	p.close_position = TriBoolNotSet
}

//------------------------------------------------------------------------------
// OrderMargin

func NewOrderMargin() OrderMargin {
	return OrderMargin{}
}

func OrderMarginReset(m *OrderMargin) {
	*m = NewOrderMargin()
}

func OrderMarginGetCollateralAsset(m OrderMargin) StringView {
	return newStringView(m.collateral_asset)
}

func OrderMarginSetCollateralAsset(m *OrderMargin, asset string) {
	m.collateral_asset = importString(asset)
}

func OrderMarginUnsetCollateralAsset(m *OrderMargin) {
	m.collateral_asset = stringViewNone.value
}

func OrderMarginGetAutoBorrow(m OrderMargin) TriBool {
	return m.auto_borrow
}

func OrderMarginSetAutoBorrow(m *OrderMargin, autoBorrow TriBool) {
	m.auto_borrow = autoBorrow
}

func OrderMarginUnsetAutoBorrow(m *OrderMargin) {
	m.auto_borrow = TriBoolNotSet
}

func OrderMarginGetLeverage(m OrderMargin) ParamLeverage {
	return m.leverage
}

func OrderMarginSetLeverage(m *OrderMargin, leverage ParamLeverage) {
	m.leverage = leverage
}

func OrderMarginUnsetLeverage(m *OrderMargin) {
	m.leverage = ParamLeverageNotSet
}

//------------------------------------------------------------------------------
