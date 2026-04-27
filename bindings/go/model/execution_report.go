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
	"fmt"

	"github.com/openpitkit/pit-go/internal/native"
	"github.com/openpitkit/pit-go/param"
	"github.com/openpitkit/pit-go/pkg/optional"
)

//------------------------------------------------------------------------------
// ExecutionReport

type ExecutionReport struct{ value native.ExecutionReport }

func NewExecutionReport() ExecutionReport {
	return NewExecutionReportFromNative(native.NewExecutionReport())
}

type ExecutionReportValues struct {
	Operation       optional.Option[ExecutionReportOperation]
	FinancialImpact optional.Option[ExecutionReportFinancialImpact]
	Fill            optional.Option[ExecutionReportFill]
	PositionImpact  optional.Option[ExecutionReportPositionImpact]
}

func NewExecutionReportFromValues(values ExecutionReportValues) ExecutionReport {
	report := NewExecutionReport()
	report.SetValues(values)
	return report
}

func NewExecutionReportFromNative(value native.ExecutionReport) ExecutionReport {
	return ExecutionReport{value: value}
}

func (r *ExecutionReport) Reset() {
	native.ExecutionReportReset(&r.value)
}

func (r ExecutionReport) Values() ExecutionReportValues {
	return ExecutionReportValues{
		Operation:       r.Operation(),
		FinancialImpact: r.FinancialImpact(),
		Fill:            r.Fill(),
		PositionImpact:  r.PositionImpact(),
	}
}

func (r *ExecutionReport) SetValues(values ExecutionReportValues) {
	r.Reset()
	r.setValues(values)
}

func (r *ExecutionReport) setValues(values ExecutionReportValues) {
	if value, ok := values.Operation.Get(); ok {
		r.SetOperation(value)
	}
	if value, ok := values.FinancialImpact.Get(); ok {
		r.SetFinancialImpact(value)
	}
	if value, ok := values.Fill.Get(); ok {
		r.SetFill(value)
	}
	if value, ok := values.PositionImpact.Get(); ok {
		r.SetPositionImpact(value)
	}
}

func (r ExecutionReport) Operation() optional.Option[ExecutionReportOperation] {
	operation := native.ExecutionReportGetOperation(r.value)
	if !native.ExecutionReportOperationOptionalIsSet(operation) {
		return optional.None[ExecutionReportOperation]()
	}
	return optional.Some(
		newExecutionReportOperation(native.ExecutionReportOperationOptionalGet(operation)),
	)
}

func (r *ExecutionReport) SetOperation(operation ExecutionReportOperation) {
	native.ExecutionReportSetOperation(&r.value, operation.value)
}

func (r *ExecutionReport) UnsetOperation() {
	native.ExecutionReportUnsetOperation(&r.value)
}

func (r ExecutionReport) FinancialImpact() optional.Option[ExecutionReportFinancialImpact] {
	financialImpact := native.ExecutionReportGetFinancialImpact(r.value)
	if !native.FinancialImpactOptionalIsSet(financialImpact) {
		return optional.None[ExecutionReportFinancialImpact]()
	}
	return optional.Some(
		newExecutionReportFinancialImpact(native.FinancialImpactOptionalGet(financialImpact)),
	)
}

func (r *ExecutionReport) SetFinancialImpact(financialImpact ExecutionReportFinancialImpact) {
	native.ExecutionReportSetFinancialImpact(&r.value, financialImpact.value)
}

func (r *ExecutionReport) UnsetFinancialImpact() {
	native.ExecutionReportUnsetFinancialImpact(&r.value)
}

func (r ExecutionReport) Fill() optional.Option[ExecutionReportFill] {
	fill := native.ExecutionReportGetFill(r.value)
	if !native.ExecutionReportFillOptionalIsSet(fill) {
		return optional.None[ExecutionReportFill]()
	}
	return optional.Some(newExecutionReportFill(native.ExecutionReportFillOptionalGet(fill)))
}

func (r *ExecutionReport) SetFill(fill ExecutionReportFill) {
	native.ExecutionReportSetFill(&r.value, fill.value)
}

func (r *ExecutionReport) UnsetFill() {
	native.ExecutionReportUnsetFill(&r.value)
}

func (r ExecutionReport) PositionImpact() optional.Option[ExecutionReportPositionImpact] {
	positionImpact := native.ExecutionReportGetPositionImpact(r.value)
	if !native.ExecutionReportPositionImpactOptionalIsSet(positionImpact) {
		return optional.None[ExecutionReportPositionImpact]()
	}
	return optional.Some(
		newExecutionReportPositionImpact(
			native.ExecutionReportPositionImpactOptionalGet(positionImpact),
		),
	)
}

func (r *ExecutionReport) SetPositionImpact(positionImpact ExecutionReportPositionImpact) {
	native.ExecutionReportSetPositionImpact(&r.value, positionImpact.value)
}

func (r *ExecutionReport) UnsetPositionImpact() {
	native.ExecutionReportUnsetPositionImpact(&r.value)
}

// EngineExecutionReport returns this report as the standard engine report view.
func (r ExecutionReport) EngineExecutionReport() ExecutionReport {
	return r
}

func (r ExecutionReport) Native() native.ExecutionReport {
	return r.value
}

//------------------------------------------------------------------------------
// ExecutionReportOperation

type ExecutionReportOperation struct {
	value native.ExecutionReportOperation
}

type ExecutionReportOperationValues struct {
	Instrument optional.Option[param.Instrument]
	AccountID  optional.Option[param.AccountID]
	Side       optional.Option[param.Side]
}

func NewExecutionReportOperation() ExecutionReportOperation {
	return newExecutionReportOperation(native.NewExecutionReportOperation())
}

func NewExecutionReportOperationFromValues(
	values ExecutionReportOperationValues,
) ExecutionReportOperation {
	operation := NewExecutionReportOperation()
	operation.setValues(values)
	return operation
}

func newExecutionReportOperation(value native.ExecutionReportOperation) ExecutionReportOperation {
	return ExecutionReportOperation{value: value}
}

func (o *ExecutionReportOperation) Reset() {
	native.ExecutionReportOperationReset(&o.value)
}

func (o ExecutionReportOperation) Values() ExecutionReportOperationValues {
	return ExecutionReportOperationValues{
		Instrument: o.Instrument(),
		AccountID:  o.AccountID(),
		Side:       o.Side(),
	}
}

func (o *ExecutionReportOperation) SetValues(values ExecutionReportOperationValues) {
	o.Reset()
	o.setValues(values)
}

func (o *ExecutionReportOperation) setValues(values ExecutionReportOperationValues) {
	if value, ok := values.Instrument.Get(); ok {
		o.SetInstrument(value)
	}
	if value, ok := values.AccountID.Get(); ok {
		o.SetAccountID(value)
	}
	if value, ok := values.Side.Get(); ok {
		o.SetSide(value)
	}
}

func (o ExecutionReportOperation) Instrument() optional.Option[param.Instrument] {
	return param.NewInstrumentFromNative(native.ExecutionReportOperationGetInstrument(o.value))
}

func (o *ExecutionReportOperation) SetInstrument(instrument param.Instrument) {
	native.ExecutionReportOperationSetInstrument(&o.value, instrument.Native())
}

func (o *ExecutionReportOperation) UnsetInstrument() {
	native.ExecutionReportOperationUnsetInstrument(&o.value)
}

func (o ExecutionReportOperation) AccountID() optional.Option[param.AccountID] {
	return param.NewAccountIDOptionFromNative(native.ExecutionReportOperationGetAccountID(o.value))
}

func (o *ExecutionReportOperation) SetAccountID(accountID param.AccountID) {
	native.ExecutionReportOperationSetAccountID(&o.value, accountID.Native())
}

func (o *ExecutionReportOperation) UnsetAccountID() {
	native.ExecutionReportOperationUnsetAccountID(&o.value)
}

func (o ExecutionReportOperation) Side() optional.Option[param.Side] {
	return param.NewSideFromNative(native.ExecutionReportOperationGetSide(o.value))
}

func (o *ExecutionReportOperation) SetSide(side param.Side) {
	native.ExecutionReportOperationSetSide(&o.value, side.Native())
}

func (o *ExecutionReportOperation) UnsetSide() {
	native.ExecutionReportOperationUnsetSide(&o.value)
}

//------------------------------------------------------------------------------
// ExecutionReportFinancialImpact

type ExecutionReportFinancialImpact struct{ value native.FinancialImpact }

type ExecutionReportFinancialImpactValues struct {
	Pnl optional.Option[param.Pnl]
	Fee optional.Option[param.Fee]
}

func NewExecutionReportFinancialImpact() ExecutionReportFinancialImpact {
	return newExecutionReportFinancialImpact(native.NewFinancialImpact())
}

func NewExecutionReportFinancialImpactFromValues(
	values ExecutionReportFinancialImpactValues,
) ExecutionReportFinancialImpact {
	financialImpact := NewExecutionReportFinancialImpact()
	financialImpact.setValues(values)
	return financialImpact
}

func newExecutionReportFinancialImpact(
	value native.FinancialImpact,
) ExecutionReportFinancialImpact {
	return ExecutionReportFinancialImpact{value: value}
}

func (i *ExecutionReportFinancialImpact) Reset() {
	native.FinancialImpactReset(&i.value)
}

func (i ExecutionReportFinancialImpact) Values() ExecutionReportFinancialImpactValues {
	return ExecutionReportFinancialImpactValues{
		Pnl: i.Pnl(),
		Fee: i.Fee(),
	}
}

func (i *ExecutionReportFinancialImpact) SetValues(values ExecutionReportFinancialImpactValues) {
	i.Reset()
	i.setValues(values)
}

func (i *ExecutionReportFinancialImpact) setValues(values ExecutionReportFinancialImpactValues) {
	if value, ok := values.Pnl.Get(); ok {
		i.SetPnl(value)
	}
	if value, ok := values.Fee.Get(); ok {
		i.SetFee(value)
	}
}

func (i ExecutionReportFinancialImpact) Pnl() optional.Option[param.Pnl] {
	return param.NewPnlOptionFromNative(native.FinancialImpactGetPnl(i.value))
}

func (i *ExecutionReportFinancialImpact) SetPnl(pnl param.Pnl) {
	native.FinancialImpactSetPnl(&i.value, pnl.Native())
}

func (i *ExecutionReportFinancialImpact) UnsetPnl() {
	native.FinancialImpactUnsetPnl(&i.value)
}

func (i ExecutionReportFinancialImpact) Fee() optional.Option[param.Fee] {
	return param.NewFeeOptionFromNative(native.FinancialImpactGetFee(i.value))
}

func (i *ExecutionReportFinancialImpact) SetFee(fee param.Fee) {
	native.FinancialImpactSetFee(&i.value, fee.Native())
}

func (i *ExecutionReportFinancialImpact) UnsetFee() {
	native.FinancialImpactUnsetFee(&i.value)
}

//------------------------------------------------------------------------------
// ExecutionReportTrade

type ExecutionReportTrade struct{ value native.ExecutionReportTrade }

func NewExecutionReportTrade(price param.Price, quantity param.Quantity) ExecutionReportTrade {
	trade := ExecutionReportTrade{value: native.NewExecutionReportTrade()}
	trade.SetPrice(price)
	trade.SetQuantity(quantity)
	return trade
}

func NewExecutionReportTradeFromNative(value native.ExecutionReportTrade) ExecutionReportTrade {
	return ExecutionReportTrade{value: value}
}

func (t *ExecutionReportTrade) Reset() {
	native.ExecutionReportTradeReset(&t.value)
}

func (t ExecutionReportTrade) Price() param.Price {
	return param.NewPriceFromNative(native.ExecutionReportTradeGetPrice(t.value))
}

func (t *ExecutionReportTrade) SetPrice(price param.Price) {
	native.ExecutionReportTradeSetPrice(&t.value, price.Native())
}

func (t ExecutionReportTrade) Quantity() param.Quantity {
	return param.NewQuantityFromNative(native.ExecutionReportTradeGetQuantity(t.value))
}

func (t *ExecutionReportTrade) SetQuantity(quantity param.Quantity) {
	native.ExecutionReportTradeSetQuantity(&t.value, quantity.Native())
}

//------------------------------------------------------------------------------
// ExecutionReportFill

type ExecutionReportFill struct{ value native.ExecutionReportFill }

type ExecutionReportFillValues struct {
	LastTrade      optional.Option[ExecutionReportTrade]
	LeavesQuantity optional.Option[param.Quantity]
	LockPrice      optional.Option[param.Price]
	Terminal       bool
}

func NewExecutionReportFill() ExecutionReportFill {
	return newExecutionReportFill(native.NewExecutionReportFill())
}

func NewExecutionReportFillFromValues(values ExecutionReportFillValues) ExecutionReportFill {
	fill := NewExecutionReportFill()
	fill.setValues(values)
	return fill
}

func newExecutionReportFill(value native.ExecutionReportFill) ExecutionReportFill {
	return ExecutionReportFill{value: value}
}

func (f *ExecutionReportFill) Reset() {
	native.ExecutionReportFillReset(&f.value)
}

func (f ExecutionReportFill) Values() ExecutionReportFillValues {
	return ExecutionReportFillValues{
		LastTrade:      f.LastTrade(),
		LeavesQuantity: f.LeavesQuantity(),
		LockPrice:      f.LockPrice(),
		Terminal:       f.Terminal(),
	}
}

func (f *ExecutionReportFill) SetValues(values ExecutionReportFillValues) {
	f.Reset()
	f.setValues(values)
}

func (f *ExecutionReportFill) setValues(values ExecutionReportFillValues) {
	if value, ok := values.LastTrade.Get(); ok {
		f.SetLastTrade(value)
	}
	if value, ok := values.LeavesQuantity.Get(); ok {
		f.SetLeavesQuantity(value)
	}
	if value, ok := values.LockPrice.Get(); ok {
		f.SetLockPrice(value)
	}
	f.SetTerminal(values.Terminal)
}

func (f ExecutionReportFill) LastTrade() optional.Option[ExecutionReportTrade] {
	trade := native.ExecutionReportFillGetLastTrade(f.value)
	if !native.ExecutionReportTradeOptionalIsSet(trade) {
		return optional.None[ExecutionReportTrade]()
	}
	return optional.Some(
		NewExecutionReportTradeFromNative(native.ExecutionReportTradeOptionalGet(trade)),
	)
}

func (f *ExecutionReportFill) SetLastTrade(trade ExecutionReportTrade) {
	native.ExecutionReportFillSetLastTrade(&f.value, trade.value)
}

func (f *ExecutionReportFill) UnsetLastTrade() {
	native.ExecutionReportFillUnsetLastTrade(&f.value)
}

func (f ExecutionReportFill) LeavesQuantity() optional.Option[param.Quantity] {
	return param.NewQuantityOptionFromNative(native.ExecutionReportFillGetLeavesQuantity(f.value))
}

func (f *ExecutionReportFill) SetLeavesQuantity(quantity param.Quantity) {
	native.ExecutionReportFillSetLeavesQuantity(&f.value, quantity.Native())
}

func (f *ExecutionReportFill) UnsetLeavesQuantity() {
	native.ExecutionReportFillUnsetLeavesQuantity(&f.value)
}

func (f ExecutionReportFill) LockPrice() optional.Option[param.Price] {
	return param.NewPriceOptionFromNative(native.ExecutionReportFillGetLockPrice(f.value))
}

func (f *ExecutionReportFill) SetLockPrice(price param.Price) {
	native.ExecutionReportFillSetLockPrice(&f.value, price.Native())
}

func (f *ExecutionReportFill) UnsetLockPrice() {
	native.ExecutionReportFillUnsetLockPrice(&f.value)
}

func (f ExecutionReportFill) Terminal() bool {
	return native.ExecutionReportFillGetTerminal(f.value)
}

func (f *ExecutionReportFill) SetTerminal(isTerminal bool) {
	native.ExecutionReportFillSetTerminal(&f.value, isTerminal)
}

//------------------------------------------------------------------------------
// ExecutionReportPositionImpact

type ExecutionReportPositionImpact struct {
	value native.ExecutionReportPositionImpact
}

type ExecutionReportPositionImpactValues struct {
	PositionEffect optional.Option[param.PositionEffect]
	PositionSide   optional.Option[param.PositionSide]
}

func NewExecutionReportPositionImpact() ExecutionReportPositionImpact {
	return newExecutionReportPositionImpact(native.NewExecutionReportPositionImpact())
}

func NewExecutionReportPositionImpactFromValues(
	values ExecutionReportPositionImpactValues,
) ExecutionReportPositionImpact {
	positionImpact := NewExecutionReportPositionImpact()
	positionImpact.setValues(values)
	return positionImpact
}

func newExecutionReportPositionImpact(
	value native.ExecutionReportPositionImpact,
) ExecutionReportPositionImpact {
	return ExecutionReportPositionImpact{value: value}
}

func (p *ExecutionReportPositionImpact) Reset() {
	native.ExecutionReportPositionImpactReset(&p.value)
}

func (p ExecutionReportPositionImpact) Values() ExecutionReportPositionImpactValues {
	return ExecutionReportPositionImpactValues{
		PositionEffect: p.PositionEffect(),
		PositionSide:   p.PositionSide(),
	}
}

func (p *ExecutionReportPositionImpact) SetValues(values ExecutionReportPositionImpactValues) {
	p.Reset()
	p.setValues(values)
}

func (p *ExecutionReportPositionImpact) setValues(values ExecutionReportPositionImpactValues) {
	if value, ok := values.PositionEffect.Get(); ok {
		p.SetPositionEffect(value)
	}
	if value, ok := values.PositionSide.Get(); ok {
		p.SetPositionSide(value)
	}
}

func (p ExecutionReportPositionImpact) PositionEffect() optional.Option[param.PositionEffect] {
	return newPositionEffectFromNative(native.ExecutionReportPositionImpactGetPositionEffect(p.value))
}

func (p *ExecutionReportPositionImpact) SetPositionEffect(effect param.PositionEffect) {
	native.ExecutionReportPositionImpactSetPositionEffect(
		&p.value,
		native.ParamPositionEffect(effect),
	)
}

func (p *ExecutionReportPositionImpact) UnsetPositionEffect() {
	native.ExecutionReportPositionImpactUnsetPositionEffect(&p.value)
}

func (p ExecutionReportPositionImpact) PositionSide() optional.Option[param.PositionSide] {
	return param.NewPositionSideFromNative(
		native.ExecutionReportPositionImpactGetPositionSide(p.value),
	)
}

func (p *ExecutionReportPositionImpact) SetPositionSide(side param.PositionSide) {
	native.ExecutionReportPositionImpactSetPositionSide(
		&p.value,
		native.ParamPositionSide(side),
	)
}

func (p *ExecutionReportPositionImpact) UnsetPositionSide() {
	native.ExecutionReportPositionImpactUnsetPositionSide(&p.value)
}

func newPositionEffectFromNative(
	value native.ParamPositionEffect,
) optional.Option[param.PositionEffect] {
	switch value {
	case native.ParamPositionEffectOpen:
		return optional.Some(param.PositionEffectOpen)
	case native.ParamPositionEffectClose:
		return optional.Some(param.PositionEffectClose)
	case native.ParamPositionEffectNotSet:
		return optional.None[param.PositionEffect]()
	default:
		panic(fmt.Sprintf("unknown native ParamPositionEffect value %d", value))
	}
}
