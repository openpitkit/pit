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

	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/optional"
)

//------------------------------------------------------------------------------
// ExecutionReport

type ExecutionReport struct {
	value native.ExecutionReport

	// retainOperationInstrument keeps the Instrument (and its two constituent
	// Assets) alive while the C struct's PitStringView fields point to their
	// C-heap buffers.  See param/asset.go and internal/native/asset_buf.go
	// for the full explanation of the retain pattern.
	retainOperationInstrument param.Instrument
}

func NewExecutionReport() ExecutionReport {
	return NewExecutionReportFromHandle(native.NewExecutionReport())
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

func NewExecutionReportFromHandle(value native.ExecutionReport) ExecutionReport {
	return ExecutionReport{value: value}
}

func (r *ExecutionReport) Reset() {
	native.ExecutionReportReset(&r.value)
	r.retainOperationInstrument = param.Instrument{}
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
	r.retainOperationInstrument = operation.retainInstrument
}

func (r *ExecutionReport) EnsureOperationView() ExecutionReportOperationView {
	operation := native.ExecutionReportGetOperationView(&r.value)
	if !native.ExecutionReportOperationOptionalIsSet(*operation) {
		native.ExecutionReportOperationOptionalSet(operation, native.NewExecutionReportOperation())
	}
	return newExecutionReportOperationView(
		native.ExecutionReportOperationOptionalGetView(operation),
		&r.retainOperationInstrument,
	)
}

func (r *ExecutionReport) UnsetOperation() {
	native.ExecutionReportUnsetOperation(&r.value)
	r.retainOperationInstrument = param.Instrument{}
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

func (r *ExecutionReport) EnsureFinancialImpactView() ExecutionReportFinancialImpactView {
	financialImpact := native.ExecutionReportGetFinancialImpactView(&r.value)
	if !native.FinancialImpactOptionalIsSet(*financialImpact) {
		native.FinancialImpactOptionalSet(financialImpact, native.NewFinancialImpact())
	}
	return newExecutionReportFinancialImpactView(native.FinancialImpactOptionalGetView(financialImpact))
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

func (r *ExecutionReport) EnsureFillView() ExecutionReportFillView {
	fill := native.ExecutionReportGetFillView(&r.value)
	if !native.ExecutionReportFillOptionalIsSet(*fill) {
		native.ExecutionReportFillOptionalSet(fill, native.NewExecutionReportFill())
	}
	return newExecutionReportFillView(native.ExecutionReportFillOptionalGetView(fill))
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

func (r *ExecutionReport) EnsurePositionImpactView() ExecutionReportPositionImpactView {
	positionImpact := native.ExecutionReportGetPositionImpactView(&r.value)
	if !native.ExecutionReportPositionImpactOptionalIsSet(*positionImpact) {
		native.ExecutionReportPositionImpactOptionalSet(
			positionImpact,
			native.NewExecutionReportPositionImpact(),
		)
	}
	return newExecutionReportPositionImpactView(
		native.ExecutionReportPositionImpactOptionalGetView(positionImpact),
	)
}

func (r *ExecutionReport) UnsetPositionImpact() {
	native.ExecutionReportUnsetPositionImpact(&r.value)
}

// EngineExecutionReport returns this report as the standard engine report view.
func (r ExecutionReport) EngineExecutionReport() ExecutionReport {
	return r
}

func (r ExecutionReport) Handle() native.ExecutionReport {
	return r.value
}

//------------------------------------------------------------------------------
// ExecutionReportOperation

type ExecutionReportOperation struct {
	value native.ExecutionReportOperation

	// retainInstrument keeps the Instrument (and its two constituent Assets)
	// alive while the C struct's PitStringView fields point to their C-heap
	// buffers.  See param/asset.go for the full explanation.
	retainInstrument param.Instrument
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
	o.retainInstrument = param.Instrument{}
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
	return param.NewInstrumentFromHandle(native.ExecutionReportOperationGetInstrument(o.value))
}

func (o *ExecutionReportOperation) SetInstrument(instrument param.Instrument) {
	native.ExecutionReportOperationSetInstrument(&o.value, instrument.Handle())
	o.retainInstrument = instrument
}

func (o *ExecutionReportOperation) UnsetInstrument() {
	native.ExecutionReportOperationUnsetInstrument(&o.value)
	o.retainInstrument = param.Instrument{}
}

func (o ExecutionReportOperation) AccountID() optional.Option[param.AccountID] {
	return param.NewAccountIDOptionFromHandle(native.ExecutionReportOperationGetAccountID(o.value))
}

func (o *ExecutionReportOperation) SetAccountID(accountID param.AccountID) {
	native.ExecutionReportOperationSetAccountID(&o.value, accountID.Handle())
}

func (o *ExecutionReportOperation) UnsetAccountID() {
	native.ExecutionReportOperationUnsetAccountID(&o.value)
}

func (o ExecutionReportOperation) Side() optional.Option[param.Side] {
	return param.NewSideFromHandle(native.ExecutionReportOperationGetSide(o.value))
}

func (o *ExecutionReportOperation) SetSide(side param.Side) {
	native.ExecutionReportOperationSetSide(&o.value, side.Handle())
}

func (o *ExecutionReportOperation) UnsetSide() {
	native.ExecutionReportOperationUnsetSide(&o.value)
}

type ExecutionReportOperationView struct {
	ref *native.ExecutionReportOperation
	// retainInstrument points to the owning ExecutionReport's
	// retainOperationInstrument field so that Set/Unset calls on this view
	// propagate retention to the parent automatically.
	retainInstrument *param.Instrument
}

func newExecutionReportOperationView(
	ref *native.ExecutionReportOperation,
	retainInstrument *param.Instrument,
) ExecutionReportOperationView {
	return ExecutionReportOperationView{ref: ref, retainInstrument: retainInstrument}
}

func (v *ExecutionReportOperationView) Reset() {
	native.ExecutionReportOperationReset(v.ref)
	*v.retainInstrument = param.Instrument{}
}

func (v ExecutionReportOperationView) Instrument() optional.Option[param.Instrument] {
	return param.NewInstrumentFromHandle(native.ExecutionReportOperationGetInstrument(*v.ref))
}

func (v *ExecutionReportOperationView) SetInstrument(instrument param.Instrument) {
	native.ExecutionReportOperationSetInstrument(v.ref, instrument.Handle())
	*v.retainInstrument = instrument
}

func (v *ExecutionReportOperationView) UnsetInstrument() {
	native.ExecutionReportOperationUnsetInstrument(v.ref)
	*v.retainInstrument = param.Instrument{}
}

func (v ExecutionReportOperationView) AccountID() optional.Option[param.AccountID] {
	return param.NewAccountIDOptionFromHandle(native.ExecutionReportOperationGetAccountID(*v.ref))
}

func (v *ExecutionReportOperationView) SetAccountID(accountID param.AccountID) {
	native.ExecutionReportOperationSetAccountID(v.ref, accountID.Handle())
}

func (v *ExecutionReportOperationView) UnsetAccountID() {
	native.ExecutionReportOperationUnsetAccountID(v.ref)
}

func (v ExecutionReportOperationView) Side() optional.Option[param.Side] {
	return param.NewSideFromHandle(native.ExecutionReportOperationGetSide(*v.ref))
}

func (v *ExecutionReportOperationView) SetSide(side param.Side) {
	native.ExecutionReportOperationSetSide(v.ref, side.Handle())
}

func (v *ExecutionReportOperationView) UnsetSide() {
	native.ExecutionReportOperationUnsetSide(v.ref)
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
	return param.NewPnlOptionFromHandle(native.FinancialImpactGetPnl(i.value))
}

func (i *ExecutionReportFinancialImpact) SetPnl(pnl param.Pnl) {
	native.FinancialImpactSetPnl(&i.value, pnl.Handle())
}

func (i *ExecutionReportFinancialImpact) UnsetPnl() {
	native.FinancialImpactUnsetPnl(&i.value)
}

func (i ExecutionReportFinancialImpact) Fee() optional.Option[param.Fee] {
	return param.NewFeeOptionFromHandle(native.FinancialImpactGetFee(i.value))
}

func (i *ExecutionReportFinancialImpact) SetFee(fee param.Fee) {
	native.FinancialImpactSetFee(&i.value, fee.Handle())
}

func (i *ExecutionReportFinancialImpact) UnsetFee() {
	native.FinancialImpactUnsetFee(&i.value)
}

type ExecutionReportFinancialImpactView struct{ ref *native.FinancialImpact }

func newExecutionReportFinancialImpactView(
	ref *native.FinancialImpact,
) ExecutionReportFinancialImpactView {
	return ExecutionReportFinancialImpactView{ref: ref}
}

func (v *ExecutionReportFinancialImpactView) Reset() {
	native.FinancialImpactReset(v.ref)
}

func (v ExecutionReportFinancialImpactView) Pnl() optional.Option[param.Pnl] {
	return param.NewPnlOptionFromHandle(native.FinancialImpactGetPnl(*v.ref))
}

func (v *ExecutionReportFinancialImpactView) SetPnl(pnl param.Pnl) {
	native.FinancialImpactSetPnl(v.ref, pnl.Handle())
}

func (v *ExecutionReportFinancialImpactView) UnsetPnl() {
	native.FinancialImpactUnsetPnl(v.ref)
}

func (v ExecutionReportFinancialImpactView) Fee() optional.Option[param.Fee] {
	return param.NewFeeOptionFromHandle(native.FinancialImpactGetFee(*v.ref))
}

func (v *ExecutionReportFinancialImpactView) SetFee(fee param.Fee) {
	native.FinancialImpactSetFee(v.ref, fee.Handle())
}

func (v *ExecutionReportFinancialImpactView) UnsetFee() {
	native.FinancialImpactUnsetFee(v.ref)
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

func NewExecutionReportTradeFromHandle(value native.ExecutionReportTrade) ExecutionReportTrade {
	return ExecutionReportTrade{value: value}
}

func (t *ExecutionReportTrade) Reset() {
	native.ExecutionReportTradeReset(&t.value)
}

func (t ExecutionReportTrade) Price() param.Price {
	return param.NewPriceFromHandle(native.ExecutionReportTradeGetPrice(t.value))
}

func (t *ExecutionReportTrade) SetPrice(price param.Price) {
	native.ExecutionReportTradeSetPrice(&t.value, price.Handle())
}

func (t ExecutionReportTrade) Quantity() param.Quantity {
	return param.NewQuantityFromHandle(native.ExecutionReportTradeGetQuantity(t.value))
}

func (t *ExecutionReportTrade) SetQuantity(quantity param.Quantity) {
	native.ExecutionReportTradeSetQuantity(&t.value, quantity.Handle())
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
		NewExecutionReportTradeFromHandle(native.ExecutionReportTradeOptionalGet(trade)),
	)
}

func (f *ExecutionReportFill) SetLastTrade(trade ExecutionReportTrade) {
	native.ExecutionReportFillSetLastTrade(&f.value, trade.value)
}

func (f *ExecutionReportFill) UnsetLastTrade() {
	native.ExecutionReportFillUnsetLastTrade(&f.value)
}

func (f ExecutionReportFill) LeavesQuantity() optional.Option[param.Quantity] {
	return param.NewQuantityOptionFromHandle(native.ExecutionReportFillGetLeavesQuantity(f.value))
}

func (f *ExecutionReportFill) SetLeavesQuantity(quantity param.Quantity) {
	native.ExecutionReportFillSetLeavesQuantity(&f.value, quantity.Handle())
}

func (f *ExecutionReportFill) UnsetLeavesQuantity() {
	native.ExecutionReportFillUnsetLeavesQuantity(&f.value)
}

func (f ExecutionReportFill) LockPrice() optional.Option[param.Price] {
	return param.NewPriceOptionFromHandle(native.ExecutionReportFillGetLockPrice(f.value))
}

func (f *ExecutionReportFill) SetLockPrice(price param.Price) {
	native.ExecutionReportFillSetLockPrice(&f.value, price.Handle())
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

type ExecutionReportFillView struct{ ref *native.ExecutionReportFill }

func newExecutionReportFillView(ref *native.ExecutionReportFill) ExecutionReportFillView {
	return ExecutionReportFillView{ref: ref}
}

func (v *ExecutionReportFillView) Reset() {
	native.ExecutionReportFillReset(v.ref)
}

func (v ExecutionReportFillView) LastTrade() optional.Option[ExecutionReportTrade] {
	trade := native.ExecutionReportFillGetLastTrade(*v.ref)
	if !native.ExecutionReportTradeOptionalIsSet(trade) {
		return optional.None[ExecutionReportTrade]()
	}
	return optional.Some(NewExecutionReportTradeFromHandle(native.ExecutionReportTradeOptionalGet(trade)))
}

func (v *ExecutionReportFillView) SetLastTrade(trade ExecutionReportTrade) {
	native.ExecutionReportFillSetLastTrade(v.ref, trade.value)
}

func (v *ExecutionReportFillView) UnsetLastTrade() {
	native.ExecutionReportFillUnsetLastTrade(v.ref)
}

func (v ExecutionReportFillView) LeavesQuantity() optional.Option[param.Quantity] {
	return param.NewQuantityOptionFromHandle(native.ExecutionReportFillGetLeavesQuantity(*v.ref))
}

func (v *ExecutionReportFillView) SetLeavesQuantity(quantity param.Quantity) {
	native.ExecutionReportFillSetLeavesQuantity(v.ref, quantity.Handle())
}

func (v *ExecutionReportFillView) UnsetLeavesQuantity() {
	native.ExecutionReportFillUnsetLeavesQuantity(v.ref)
}

func (v ExecutionReportFillView) LockPrice() optional.Option[param.Price] {
	return param.NewPriceOptionFromHandle(native.ExecutionReportFillGetLockPrice(*v.ref))
}

func (v *ExecutionReportFillView) SetLockPrice(price param.Price) {
	native.ExecutionReportFillSetLockPrice(v.ref, price.Handle())
}

func (v *ExecutionReportFillView) UnsetLockPrice() {
	native.ExecutionReportFillUnsetLockPrice(v.ref)
}

func (v ExecutionReportFillView) Terminal() bool {
	return native.ExecutionReportFillGetTerminal(*v.ref)
}

func (v *ExecutionReportFillView) SetTerminal(isTerminal bool) {
	native.ExecutionReportFillSetTerminal(v.ref, isTerminal)
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
	return newPositionEffectFromHandle(native.ExecutionReportPositionImpactGetPositionEffect(p.value))
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
	return param.NewPositionSideFromHandle(
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

type ExecutionReportPositionImpactView struct {
	ref *native.ExecutionReportPositionImpact
}

func newExecutionReportPositionImpactView(
	ref *native.ExecutionReportPositionImpact,
) ExecutionReportPositionImpactView {
	return ExecutionReportPositionImpactView{ref: ref}
}

func (v *ExecutionReportPositionImpactView) Reset() {
	native.ExecutionReportPositionImpactReset(v.ref)
}

func (v ExecutionReportPositionImpactView) PositionEffect() optional.Option[param.PositionEffect] {
	return newPositionEffectFromHandle(native.ExecutionReportPositionImpactGetPositionEffect(*v.ref))
}

func (v *ExecutionReportPositionImpactView) SetPositionEffect(effect param.PositionEffect) {
	native.ExecutionReportPositionImpactSetPositionEffect(v.ref, native.ParamPositionEffect(effect))
}

func (v *ExecutionReportPositionImpactView) UnsetPositionEffect() {
	native.ExecutionReportPositionImpactUnsetPositionEffect(v.ref)
}

func (v ExecutionReportPositionImpactView) PositionSide() optional.Option[param.PositionSide] {
	return param.NewPositionSideFromHandle(native.ExecutionReportPositionImpactGetPositionSide(*v.ref))
}

func (v *ExecutionReportPositionImpactView) SetPositionSide(side param.PositionSide) {
	native.ExecutionReportPositionImpactSetPositionSide(v.ref, native.ParamPositionSide(side))
}

func (v *ExecutionReportPositionImpactView) UnsetPositionSide() {
	native.ExecutionReportPositionImpactUnsetPositionSide(v.ref)
}

func newPositionEffectFromHandle(
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
