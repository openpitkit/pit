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

// ExecutionReport represents a post-trade execution outcome.
type ExecutionReport struct {
	value native.ExecutionReport

	// retainOperationInstrument keeps the Instrument (and its two constituent
	// Assets) alive while the C struct's OpenPitStringView fields point to their
	// C-heap buffers.  See param/asset.go and internal/native/asset_buf.go
	// for the full explanation of the retain pattern.
	retainOperationInstrument param.Instrument
}

// NewExecutionReport creates a new zeroed ExecutionReport.
func NewExecutionReport() ExecutionReport {
	return NewExecutionReportFromHandle(native.NewExecutionReport())
}

// ExecutionReportValues holds the optional fields of an ExecutionReport.
type ExecutionReportValues struct {
	Operation       optional.Option[ExecutionReportOperation]
	FinancialImpact optional.Option[ExecutionReportFinancialImpact]
	Fill            optional.Option[ExecutionReportFill]
	PositionImpact  optional.Option[ExecutionReportPositionImpact]
}

// NewExecutionReportFromValues creates an ExecutionReport from the given values.
func NewExecutionReportFromValues(values ExecutionReportValues) ExecutionReport {
	report := NewExecutionReport()
	report.SetValues(values)
	return report
}

// NewExecutionReportFromHandle creates an ExecutionReport from a native handle.
func NewExecutionReportFromHandle(value native.ExecutionReport) ExecutionReport {
	return ExecutionReport{value: value}
}

// Reset zeroes out the execution report.
func (r *ExecutionReport) Reset() {
	native.ExecutionReportReset(&r.value)
	r.retainOperationInstrument = param.Instrument{}
}

// Values returns a copy of the current execution report fields.
func (r ExecutionReport) Values() ExecutionReportValues {
	return ExecutionReportValues{
		Operation:       r.Operation(),
		FinancialImpact: r.FinancialImpact(),
		Fill:            r.Fill(),
		PositionImpact:  r.PositionImpact(),
	}
}

// SetValues resets the report and applies the provided values.
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

// Operation returns the optional operation of the report.
func (r ExecutionReport) Operation() optional.Option[ExecutionReportOperation] {
	operation := native.ExecutionReportGetOperation(r.value)
	if !native.ExecutionReportOperationOptionalIsSet(operation) {
		return optional.None[ExecutionReportOperation]()
	}
	return optional.Some(
		newExecutionReportOperation(native.ExecutionReportOperationOptionalGet(operation)),
	)
}

// SetOperation sets the operation on the report.
func (r *ExecutionReport) SetOperation(operation ExecutionReportOperation) {
	native.ExecutionReportSetOperation(&r.value, operation.value)
	r.retainOperationInstrument = operation.retainInstrument
}

// EnsureOperationView ensures the operation exists and returns a mutable view.
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

// UnsetOperation clears the operation on the report.
func (r *ExecutionReport) UnsetOperation() {
	native.ExecutionReportUnsetOperation(&r.value)
	r.retainOperationInstrument = param.Instrument{}
}

// FinancialImpact returns the optional financial impact of the report.
func (r ExecutionReport) FinancialImpact() optional.Option[ExecutionReportFinancialImpact] {
	financialImpact := native.ExecutionReportGetFinancialImpact(r.value)
	if !native.FinancialImpactOptionalIsSet(financialImpact) {
		return optional.None[ExecutionReportFinancialImpact]()
	}
	return optional.Some(
		newExecutionReportFinancialImpact(native.FinancialImpactOptionalGet(financialImpact)),
	)
}

// SetFinancialImpact sets the financial impact on the report.
func (r *ExecutionReport) SetFinancialImpact(financialImpact ExecutionReportFinancialImpact) {
	native.ExecutionReportSetFinancialImpact(&r.value, financialImpact.value)
}

// EnsureFinancialImpactView ensures the financial impact exists and returns a mutable view.
func (r *ExecutionReport) EnsureFinancialImpactView() ExecutionReportFinancialImpactView {
	financialImpact := native.ExecutionReportGetFinancialImpactView(&r.value)
	if !native.FinancialImpactOptionalIsSet(*financialImpact) {
		native.FinancialImpactOptionalSet(financialImpact, native.NewFinancialImpact())
	}
	return newExecutionReportFinancialImpactView(native.FinancialImpactOptionalGetView(financialImpact))
}

// UnsetFinancialImpact clears the financial impact on the report.
func (r *ExecutionReport) UnsetFinancialImpact() {
	native.ExecutionReportUnsetFinancialImpact(&r.value)
}

// Fill returns the optional fill of the report.
func (r ExecutionReport) Fill() optional.Option[ExecutionReportFill] {
	fill := native.ExecutionReportGetFill(r.value)
	if !native.ExecutionReportFillOptionalIsSet(fill) {
		return optional.None[ExecutionReportFill]()
	}
	return optional.Some(newExecutionReportFill(native.ExecutionReportFillOptionalGet(fill)))
}

// SetFill sets the fill on the report.
func (r *ExecutionReport) SetFill(fill ExecutionReportFill) {
	native.ExecutionReportSetFill(&r.value, fill.value)
}

// EnsureFillView ensures the fill exists and returns a mutable view.
func (r *ExecutionReport) EnsureFillView() ExecutionReportFillView {
	fill := native.ExecutionReportGetFillView(&r.value)
	if !native.ExecutionReportFillOptionalIsSet(*fill) {
		native.ExecutionReportFillOptionalSet(fill, native.NewExecutionReportFill())
	}
	return newExecutionReportFillView(native.ExecutionReportFillOptionalGetView(fill))
}

// UnsetFill clears the fill on the report.
func (r *ExecutionReport) UnsetFill() {
	native.ExecutionReportUnsetFill(&r.value)
}

// PositionImpact returns the optional position impact of the report.
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

// SetPositionImpact sets the position impact on the report.
func (r *ExecutionReport) SetPositionImpact(positionImpact ExecutionReportPositionImpact) {
	native.ExecutionReportSetPositionImpact(&r.value, positionImpact.value)
}

// EnsurePositionImpactView ensures the position impact exists and returns a mutable view.
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

// UnsetPositionImpact clears the position impact on the report.
func (r *ExecutionReport) UnsetPositionImpact() {
	native.ExecutionReportUnsetPositionImpact(&r.value)
}

// EngineExecutionReport returns this report as the standard engine report view.
func (r ExecutionReport) EngineExecutionReport() ExecutionReport {
	return r
}

// Handle returns the underlying native handle.
func (r ExecutionReport) Handle() native.ExecutionReport {
	return r.value
}

//------------------------------------------------------------------------------
// ExecutionReportOperation

// ExecutionReportOperation holds the operation fields of an execution report.
type ExecutionReportOperation struct {
	value native.ExecutionReportOperation

	// retainInstrument keeps the Instrument (and its two constituent Assets)
	// alive while the C struct's OpenPitStringView fields point to their C-heap
	// buffers.  See param/asset.go for the full explanation.
	retainInstrument param.Instrument
}

// ExecutionReportOperationValues holds the optional fields of an execution report operation.
type ExecutionReportOperationValues struct {
	Instrument optional.Option[param.Instrument]
	AccountID  optional.Option[param.AccountID]
	Side       optional.Option[param.Side]
}

// NewExecutionReportOperation creates a new zeroed ExecutionReportOperation.
func NewExecutionReportOperation() ExecutionReportOperation {
	return newExecutionReportOperation(native.NewExecutionReportOperation())
}

// NewExecutionReportOperationFromValues creates an ExecutionReportOperation from the given values.
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

// Reset zeroes out the operation.
func (o *ExecutionReportOperation) Reset() {
	native.ExecutionReportOperationReset(&o.value)
	o.retainInstrument = param.Instrument{}
}

// Values returns a copy of the current operation fields.
func (o ExecutionReportOperation) Values() ExecutionReportOperationValues {
	return ExecutionReportOperationValues{
		Instrument: o.Instrument(),
		AccountID:  o.AccountID(),
		Side:       o.Side(),
	}
}

// SetValues resets the operation and applies the provided values.
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

// Instrument returns the optional instrument of the operation.
func (o ExecutionReportOperation) Instrument() optional.Option[param.Instrument] {
	return param.NewInstrumentFromHandle(native.ExecutionReportOperationGetInstrument(o.value))
}

// SetInstrument sets the instrument on the operation.
func (o *ExecutionReportOperation) SetInstrument(instrument param.Instrument) {
	native.ExecutionReportOperationSetInstrument(&o.value, instrument.Handle())
	o.retainInstrument = instrument
}

// UnsetInstrument clears the instrument on the operation.
func (o *ExecutionReportOperation) UnsetInstrument() {
	native.ExecutionReportOperationUnsetInstrument(&o.value)
	o.retainInstrument = param.Instrument{}
}

// AccountID returns the optional account ID of the operation.
func (o ExecutionReportOperation) AccountID() optional.Option[param.AccountID] {
	return param.NewAccountIDOptionFromHandle(native.ExecutionReportOperationGetAccountID(o.value))
}

// SetAccountID sets the account ID on the operation.
func (o *ExecutionReportOperation) SetAccountID(accountID param.AccountID) {
	native.ExecutionReportOperationSetAccountID(&o.value, accountID.Handle())
}

// UnsetAccountID clears the account ID on the operation.
func (o *ExecutionReportOperation) UnsetAccountID() {
	native.ExecutionReportOperationUnsetAccountID(&o.value)
}

// Side returns the optional trade side of the operation.
func (o ExecutionReportOperation) Side() optional.Option[param.Side] {
	return param.NewSideFromHandle(native.ExecutionReportOperationGetSide(o.value))
}

// SetSide sets the trade side on the operation.
func (o *ExecutionReportOperation) SetSide(side param.Side) {
	native.ExecutionReportOperationSetSide(&o.value, side.Handle())
}

// UnsetSide clears the trade side on the operation.
func (o *ExecutionReportOperation) UnsetSide() {
	native.ExecutionReportOperationUnsetSide(&o.value)
}

// ExecutionReportOperationView is a mutable view into an operation owned by an ExecutionReport.
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

// Reset zeroes out the operation view.
func (v *ExecutionReportOperationView) Reset() {
	native.ExecutionReportOperationReset(v.ref)
	*v.retainInstrument = param.Instrument{}
}

// Instrument returns the optional instrument from the view.
func (v ExecutionReportOperationView) Instrument() optional.Option[param.Instrument] {
	return param.NewInstrumentFromHandle(native.ExecutionReportOperationGetInstrument(*v.ref))
}

// SetInstrument sets the instrument on the view.
func (v *ExecutionReportOperationView) SetInstrument(instrument param.Instrument) {
	native.ExecutionReportOperationSetInstrument(v.ref, instrument.Handle())
	*v.retainInstrument = instrument
}

// UnsetInstrument clears the instrument on the view.
func (v *ExecutionReportOperationView) UnsetInstrument() {
	native.ExecutionReportOperationUnsetInstrument(v.ref)
	*v.retainInstrument = param.Instrument{}
}

// AccountID returns the optional account ID from the view.
func (v ExecutionReportOperationView) AccountID() optional.Option[param.AccountID] {
	return param.NewAccountIDOptionFromHandle(native.ExecutionReportOperationGetAccountID(*v.ref))
}

// SetAccountID sets the account ID on the view.
func (v *ExecutionReportOperationView) SetAccountID(accountID param.AccountID) {
	native.ExecutionReportOperationSetAccountID(v.ref, accountID.Handle())
}

// UnsetAccountID clears the account ID on the view.
func (v *ExecutionReportOperationView) UnsetAccountID() {
	native.ExecutionReportOperationUnsetAccountID(v.ref)
}

// Side returns the optional trade side from the view.
func (v ExecutionReportOperationView) Side() optional.Option[param.Side] {
	return param.NewSideFromHandle(native.ExecutionReportOperationGetSide(*v.ref))
}

// SetSide sets the trade side on the view.
func (v *ExecutionReportOperationView) SetSide(side param.Side) {
	native.ExecutionReportOperationSetSide(v.ref, side.Handle())
}

// UnsetSide clears the trade side on the view.
func (v *ExecutionReportOperationView) UnsetSide() {
	native.ExecutionReportOperationUnsetSide(v.ref)
}

//------------------------------------------------------------------------------
// ExecutionReportFinancialImpact

// ExecutionReportFinancialImpact holds the PnL and fee resulting from the execution.
type ExecutionReportFinancialImpact struct{ value native.FinancialImpact }

// ExecutionReportFinancialImpactValues holds the optional financial impact fields.
type ExecutionReportFinancialImpactValues struct {
	Pnl optional.Option[param.Pnl]
	Fee optional.Option[param.Fee]
}

// NewExecutionReportFinancialImpact creates a new zeroed ExecutionReportFinancialImpact.
func NewExecutionReportFinancialImpact() ExecutionReportFinancialImpact {
	return newExecutionReportFinancialImpact(native.NewFinancialImpact())
}

// NewExecutionReportFinancialImpactFromValues creates an ExecutionReportFinancialImpact from values.
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

// Reset zeroes out the financial impact.
func (i *ExecutionReportFinancialImpact) Reset() {
	native.FinancialImpactReset(&i.value)
}

// Values returns a copy of the current financial impact fields.
func (i ExecutionReportFinancialImpact) Values() ExecutionReportFinancialImpactValues {
	return ExecutionReportFinancialImpactValues{
		Pnl: i.Pnl(),
		Fee: i.Fee(),
	}
}

// SetValues resets the financial impact and applies the provided values.
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

// Pnl returns the optional profit and loss.
func (i ExecutionReportFinancialImpact) Pnl() optional.Option[param.Pnl] {
	return param.NewPnlOptionFromHandle(native.FinancialImpactGetPnl(i.value))
}

// SetPnl sets the profit and loss.
func (i *ExecutionReportFinancialImpact) SetPnl(pnl param.Pnl) {
	native.FinancialImpactSetPnl(&i.value, pnl.Handle())
}

// UnsetPnl clears the profit and loss.
func (i *ExecutionReportFinancialImpact) UnsetPnl() {
	native.FinancialImpactUnsetPnl(&i.value)
}

// Fee returns the optional fee.
func (i ExecutionReportFinancialImpact) Fee() optional.Option[param.Fee] {
	return param.NewFeeOptionFromHandle(native.FinancialImpactGetFee(i.value))
}

// SetFee sets the fee.
func (i *ExecutionReportFinancialImpact) SetFee(fee param.Fee) {
	native.FinancialImpactSetFee(&i.value, fee.Handle())
}

// UnsetFee clears the fee.
func (i *ExecutionReportFinancialImpact) UnsetFee() {
	native.FinancialImpactUnsetFee(&i.value)
}

// ExecutionReportFinancialImpactView is a mutable view into financial impact owned by an ExecutionReport.
type ExecutionReportFinancialImpactView struct{ ref *native.FinancialImpact }

func newExecutionReportFinancialImpactView(
	ref *native.FinancialImpact,
) ExecutionReportFinancialImpactView {
	return ExecutionReportFinancialImpactView{ref: ref}
}

// Reset zeroes out the financial impact view.
func (v *ExecutionReportFinancialImpactView) Reset() {
	native.FinancialImpactReset(v.ref)
}

// Pnl returns the optional profit and loss from the view.
func (v ExecutionReportFinancialImpactView) Pnl() optional.Option[param.Pnl] {
	return param.NewPnlOptionFromHandle(native.FinancialImpactGetPnl(*v.ref))
}

// SetPnl sets the profit and loss on the view.
func (v *ExecutionReportFinancialImpactView) SetPnl(pnl param.Pnl) {
	native.FinancialImpactSetPnl(v.ref, pnl.Handle())
}

// UnsetPnl clears the profit and loss on the view.
func (v *ExecutionReportFinancialImpactView) UnsetPnl() {
	native.FinancialImpactUnsetPnl(v.ref)
}

// Fee returns the optional fee from the view.
func (v ExecutionReportFinancialImpactView) Fee() optional.Option[param.Fee] {
	return param.NewFeeOptionFromHandle(native.FinancialImpactGetFee(*v.ref))
}

// SetFee sets the fee on the view.
func (v *ExecutionReportFinancialImpactView) SetFee(fee param.Fee) {
	native.FinancialImpactSetFee(v.ref, fee.Handle())
}

// UnsetFee clears the fee on the view.
func (v *ExecutionReportFinancialImpactView) UnsetFee() {
	native.FinancialImpactUnsetFee(v.ref)
}

//------------------------------------------------------------------------------
// ExecutionReportTrade

// ExecutionReportTrade holds the price and quantity of a single trade execution.
type ExecutionReportTrade struct{ value native.ExecutionReportTrade }

// NewExecutionReportTrade creates a new trade with the given price and quantity.
func NewExecutionReportTrade(price param.Price, quantity param.Quantity) ExecutionReportTrade {
	trade := ExecutionReportTrade{value: native.NewExecutionReportTrade()}
	trade.SetPrice(price)
	trade.SetQuantity(quantity)
	return trade
}

// NewExecutionReportTradeFromHandle creates an ExecutionReportTrade from a native handle.
func NewExecutionReportTradeFromHandle(value native.ExecutionReportTrade) ExecutionReportTrade {
	return ExecutionReportTrade{value: value}
}

// Reset zeroes out the trade.
func (t *ExecutionReportTrade) Reset() {
	native.ExecutionReportTradeReset(&t.value)
}

// Price returns the trade price.
func (t ExecutionReportTrade) Price() param.Price {
	return param.NewPriceFromHandle(native.ExecutionReportTradeGetPrice(t.value))
}

// SetPrice sets the trade price.
func (t *ExecutionReportTrade) SetPrice(price param.Price) {
	native.ExecutionReportTradeSetPrice(&t.value, price.Handle())
}

// Quantity returns the trade quantity.
func (t ExecutionReportTrade) Quantity() param.Quantity {
	return param.NewQuantityFromHandle(native.ExecutionReportTradeGetQuantity(t.value))
}

// SetQuantity sets the trade quantity.
func (t *ExecutionReportTrade) SetQuantity(quantity param.Quantity) {
	native.ExecutionReportTradeSetQuantity(&t.value, quantity.Handle())
}

//------------------------------------------------------------------------------
// ExecutionReportFill

// ExecutionReportFill holds fill details for an execution report.
type ExecutionReportFill struct{ value native.ExecutionReportFill }

// ExecutionReportFillValues holds the optional fill fields.
type ExecutionReportFillValues struct {
	LastTrade      optional.Option[ExecutionReportTrade]
	LeavesQuantity optional.Option[param.Quantity]
	// Lock carries the raw pre-trade lock representation, or nil when no lock is
	// set. These bytes round-trip with pretrade.Lock via pretrade.NewLockFromBytes
	// and Lock.Bytes.
	Lock    []byte
	IsFinal optional.Bool
}

// NewExecutionReportFill creates a new zeroed ExecutionReportFill.
func NewExecutionReportFill() ExecutionReportFill {
	return newExecutionReportFill(native.NewExecutionReportFill())
}

// NewExecutionReportFillFromValues creates an ExecutionReportFill from the given values.
func NewExecutionReportFillFromValues(values ExecutionReportFillValues) ExecutionReportFill {
	fill := NewExecutionReportFill()
	fill.setValues(values)
	return fill
}

func newExecutionReportFill(value native.ExecutionReportFill) ExecutionReportFill {
	return ExecutionReportFill{value: value}
}

// Reset zeroes out the fill.
func (f *ExecutionReportFill) Reset() {
	native.ExecutionReportFillReset(&f.value)
}

// Values returns a copy of the current fill fields.
func (f ExecutionReportFill) Values() ExecutionReportFillValues {
	return ExecutionReportFillValues{
		LastTrade:      f.LastTrade(),
		LeavesQuantity: f.LeavesQuantity(),
		Lock:           f.Lock(),
		IsFinal:        f.IsFinal(),
	}
}

// SetValues resets the fill and applies the provided values.
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
	if values.Lock != nil {
		f.SetLock(values.Lock)
	}
	if value, ok := values.IsFinal.Get(); ok {
		f.SetIsFinal(value)
	}
}

// LastTrade returns the optional last trade of this fill.
func (f ExecutionReportFill) LastTrade() optional.Option[ExecutionReportTrade] {
	trade := native.ExecutionReportFillGetLastTrade(f.value)
	if !native.ExecutionReportTradeOptionalIsSet(trade) {
		return optional.None[ExecutionReportTrade]()
	}
	return optional.Some(
		NewExecutionReportTradeFromHandle(native.ExecutionReportTradeOptionalGet(trade)),
	)
}

// SetLastTrade sets the last trade on the fill.
func (f *ExecutionReportFill) SetLastTrade(trade ExecutionReportTrade) {
	native.ExecutionReportFillSetLastTrade(&f.value, trade.value)
}

// UnsetLastTrade clears the last trade on the fill.
func (f *ExecutionReportFill) UnsetLastTrade() {
	native.ExecutionReportFillUnsetLastTrade(&f.value)
}

// LeavesQuantity returns the optional remaining unfilled quantity.
func (f ExecutionReportFill) LeavesQuantity() optional.Option[param.Quantity] {
	return param.NewQuantityOptionFromHandle(native.ExecutionReportFillGetLeavesQuantity(f.value))
}

// SetLeavesQuantity sets the remaining unfilled quantity on the fill.
func (f *ExecutionReportFill) SetLeavesQuantity(quantity param.Quantity) {
	native.ExecutionReportFillSetLeavesQuantity(&f.value, quantity.Handle())
}

// UnsetLeavesQuantity clears the remaining unfilled quantity on the fill.
func (f *ExecutionReportFill) UnsetLeavesQuantity() {
	native.ExecutionReportFillUnsetLeavesQuantity(&f.value)
}

// Lock returns the raw pre-trade lock attached to the fill, or nil when none is
// set. The bytes round-trip with pretrade.Lock via pretrade.NewLockFromBytes and
// Lock.Bytes.
func (f ExecutionReportFill) Lock() []byte {
	return executionReportFillLockBytes(native.ExecutionReportFillGetLock(f.value))
}

// SetLock attaches a pre-trade lock to the fill from its raw bytes. The bytes
// must come from pretrade.Lock.Bytes within the same library build; an empty or
// nil slice clears the lock. The fill owns the stored lock.
func (f *ExecutionReportFill) SetLock(lock []byte) {
	setExecutionReportFillLock(&f.value, lock)
}

// UnsetLock clears the pre-trade lock on the fill, releasing the stored lock.
func (f *ExecutionReportFill) UnsetLock() {
	unsetExecutionReportFillLock(&f.value)
}

// IsFinal reports whether the order is closed out by this fill.
func (f ExecutionReportFill) IsFinal() optional.Bool {
	return executionReportIsFinalOption(native.ExecutionReportFillGetFinal(f.value))
}

// SetIsFinal marks the fill as closing the order's report stream.
func (f *ExecutionReportFill) SetIsFinal(isFinal bool) {
	native.ExecutionReportFillSetFinal(&f.value, isFinal)
}

// UnsetIsFinal clears the "is final" flag.
func (f *ExecutionReportFill) UnsetIsFinal() {
	native.ExecutionReportFillUnsetFinal(&f.value)
}

// ExecutionReportFillView is a mutable view into a fill owned by an ExecutionReport.
type ExecutionReportFillView struct{ ref *native.ExecutionReportFill }

func newExecutionReportFillView(ref *native.ExecutionReportFill) ExecutionReportFillView {
	return ExecutionReportFillView{ref: ref}
}

// Reset zeroes out the fill view.
func (v *ExecutionReportFillView) Reset() {
	native.ExecutionReportFillReset(v.ref)
}

// LastTrade returns the optional last trade from the view.
func (v ExecutionReportFillView) LastTrade() optional.Option[ExecutionReportTrade] {
	trade := native.ExecutionReportFillGetLastTrade(*v.ref)
	if !native.ExecutionReportTradeOptionalIsSet(trade) {
		return optional.None[ExecutionReportTrade]()
	}
	return optional.Some(NewExecutionReportTradeFromHandle(native.ExecutionReportTradeOptionalGet(trade)))
}

// SetLastTrade sets the last trade on the view.
func (v *ExecutionReportFillView) SetLastTrade(trade ExecutionReportTrade) {
	native.ExecutionReportFillSetLastTrade(v.ref, trade.value)
}

// UnsetLastTrade clears the last trade on the view.
func (v *ExecutionReportFillView) UnsetLastTrade() {
	native.ExecutionReportFillUnsetLastTrade(v.ref)
}

// LeavesQuantity returns the optional remaining unfilled quantity from the view.
func (v ExecutionReportFillView) LeavesQuantity() optional.Option[param.Quantity] {
	return param.NewQuantityOptionFromHandle(native.ExecutionReportFillGetLeavesQuantity(*v.ref))
}

// SetLeavesQuantity sets the remaining unfilled quantity on the view.
func (v *ExecutionReportFillView) SetLeavesQuantity(quantity param.Quantity) {
	native.ExecutionReportFillSetLeavesQuantity(v.ref, quantity.Handle())
}

// UnsetLeavesQuantity clears the remaining unfilled quantity on the view.
func (v *ExecutionReportFillView) UnsetLeavesQuantity() {
	native.ExecutionReportFillUnsetLeavesQuantity(v.ref)
}

// Lock returns the raw pre-trade lock attached to the view, or nil when none is
// set. The bytes round-trip with pretrade.Lock via pretrade.NewLockFromBytes and
// Lock.Bytes.
func (v ExecutionReportFillView) Lock() []byte {
	return executionReportFillLockBytes(native.ExecutionReportFillGetLock(*v.ref))
}

// SetLock attaches a pre-trade lock to the view from its raw bytes. The bytes
// must come from pretrade.Lock.Bytes within the same library build; an empty or
// nil slice clears the lock. The fill owns the stored lock.
func (v *ExecutionReportFillView) SetLock(lock []byte) {
	setExecutionReportFillLock(v.ref, lock)
}

// UnsetLock clears the pre-trade lock on the view, releasing the stored lock.
func (v *ExecutionReportFillView) UnsetLock() {
	unsetExecutionReportFillLock(v.ref)
}

// IsFinal reports whether the order is closed out by this fill.
func (v ExecutionReportFillView) IsFinal() optional.Bool {
	return executionReportIsFinalOption(native.ExecutionReportFillGetFinal(*v.ref))
}

// SetIsFinal marks the fill as closing the order's report stream.
func (v *ExecutionReportFillView) SetIsFinal(isFinal bool) {
	native.ExecutionReportFillSetFinal(v.ref, isFinal)
}

// UnsetIsFinal clears the "is final" flag.
func (v *ExecutionReportFillView) UnsetIsFinal() {
	native.ExecutionReportFillUnsetFinal(v.ref)
}

func executionReportIsFinalOption(value native.ExecutionReportIsFinalOptional) optional.Bool {
	if !native.ExecutionReportIsFinalOptionalIsSet(value) {
		return optional.BoolNone
	}
	return optional.BoolSome(native.ExecutionReportIsFinalOptionalGet(value))
}

// executionReportFillLockBytes converts the fill's stored lock handle to its raw
// byte representation, returning nil when no lock is attached.
func executionReportFillLockBytes(lock native.PretradePreTradeLock) []byte {
	if lock == nil {
		return nil
	}
	raw := native.PretradePreTradeLockToRaw(lock)
	value := native.CloneBytes(native.SharedBytesView(raw))
	native.DestroySharedBytes(raw)
	return value
}

// setExecutionReportFillLock replaces the fill's stored lock with one decoded
// from the raw bytes. Empty bytes clear the lock. The fill owns the stored
// handle (the C struct's lock field; null means no lock).
func setExecutionReportFillLock(fill *native.ExecutionReportFill, lock []byte) {
	unsetExecutionReportFillLock(fill)
	if len(lock) == 0 {
		return
	}
	handle, err := native.CreatePretradePreTradeLockFromRaw(lock)
	if err != nil {
		panic(fmt.Sprintf("failed to decode pre-trade lock bytes: %v", err))
	}
	native.ExecutionReportFillSetLock(fill, handle)
}

// unsetExecutionReportFillLock clears and frees the fill's stored lock handle.
func unsetExecutionReportFillLock(fill *native.ExecutionReportFill) {
	if existing := native.ExecutionReportFillGetLock(*fill); existing != nil {
		native.DestroyPretradePreTradeLock(existing)
	}
	native.ExecutionReportFillUnsetLock(fill)
}

//------------------------------------------------------------------------------
// ExecutionReportPositionImpact

// ExecutionReportPositionImpact holds the position effect and side resulting from the execution.
type ExecutionReportPositionImpact struct {
	value native.ExecutionReportPositionImpact
}

// ExecutionReportPositionImpactValues holds the optional position impact fields.
type ExecutionReportPositionImpactValues struct {
	PositionEffect optional.Option[param.PositionEffect]
	PositionSide   optional.Option[param.PositionSide]
}

// NewExecutionReportPositionImpact creates a new zeroed ExecutionReportPositionImpact.
func NewExecutionReportPositionImpact() ExecutionReportPositionImpact {
	return newExecutionReportPositionImpact(native.NewExecutionReportPositionImpact())
}

// NewExecutionReportPositionImpactFromValues creates an ExecutionReportPositionImpact from values.
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

// Reset zeroes out the position impact.
func (p *ExecutionReportPositionImpact) Reset() {
	native.ExecutionReportPositionImpactReset(&p.value)
}

// Values returns a copy of the current position impact fields.
func (p ExecutionReportPositionImpact) Values() ExecutionReportPositionImpactValues {
	return ExecutionReportPositionImpactValues{
		PositionEffect: p.PositionEffect(),
		PositionSide:   p.PositionSide(),
	}
}

// SetValues resets the position impact and applies the provided values.
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

// PositionEffect returns the optional position effect.
func (p ExecutionReportPositionImpact) PositionEffect() optional.Option[param.PositionEffect] {
	return newPositionEffectFromHandle(native.ExecutionReportPositionImpactGetPositionEffect(p.value))
}

// SetPositionEffect sets the position effect.
func (p *ExecutionReportPositionImpact) SetPositionEffect(effect param.PositionEffect) {
	native.ExecutionReportPositionImpactSetPositionEffect(
		&p.value,
		native.ParamPositionEffect(effect),
	)
}

// UnsetPositionEffect clears the position effect.
func (p *ExecutionReportPositionImpact) UnsetPositionEffect() {
	native.ExecutionReportPositionImpactUnsetPositionEffect(&p.value)
}

// PositionSide returns the optional position side.
func (p ExecutionReportPositionImpact) PositionSide() optional.Option[param.PositionSide] {
	return param.NewPositionSideFromHandle(
		native.ExecutionReportPositionImpactGetPositionSide(p.value),
	)
}

// SetPositionSide sets the position side.
func (p *ExecutionReportPositionImpact) SetPositionSide(side param.PositionSide) {
	native.ExecutionReportPositionImpactSetPositionSide(
		&p.value,
		native.ParamPositionSide(side),
	)
}

// UnsetPositionSide clears the position side.
func (p *ExecutionReportPositionImpact) UnsetPositionSide() {
	native.ExecutionReportPositionImpactUnsetPositionSide(&p.value)
}

// ExecutionReportPositionImpactView is a mutable view into position impact owned by an ExecutionReport.
type ExecutionReportPositionImpactView struct {
	ref *native.ExecutionReportPositionImpact
}

func newExecutionReportPositionImpactView(
	ref *native.ExecutionReportPositionImpact,
) ExecutionReportPositionImpactView {
	return ExecutionReportPositionImpactView{ref: ref}
}

// Reset zeroes out the position impact view.
func (v *ExecutionReportPositionImpactView) Reset() {
	native.ExecutionReportPositionImpactReset(v.ref)
}

// PositionEffect returns the optional position effect from the view.
func (v ExecutionReportPositionImpactView) PositionEffect() optional.Option[param.PositionEffect] {
	return newPositionEffectFromHandle(native.ExecutionReportPositionImpactGetPositionEffect(*v.ref))
}

// SetPositionEffect sets the position effect on the view.
func (v *ExecutionReportPositionImpactView) SetPositionEffect(effect param.PositionEffect) {
	native.ExecutionReportPositionImpactSetPositionEffect(v.ref, native.ParamPositionEffect(effect))
}

// UnsetPositionEffect clears the position effect on the view.
func (v *ExecutionReportPositionImpactView) UnsetPositionEffect() {
	native.ExecutionReportPositionImpactUnsetPositionEffect(v.ref)
}

// PositionSide returns the optional position side from the view.
func (v ExecutionReportPositionImpactView) PositionSide() optional.Option[param.PositionSide] {
	return param.NewPositionSideFromHandle(native.ExecutionReportPositionImpactGetPositionSide(*v.ref))
}

// SetPositionSide sets the position side on the view.
func (v *ExecutionReportPositionImpactView) SetPositionSide(side param.PositionSide) {
	native.ExecutionReportPositionImpactSetPositionSide(v.ref, native.ParamPositionSide(side))
}

// UnsetPositionSide clears the position side on the view.
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
