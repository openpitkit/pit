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
	"errors"

	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/optional"
)

//------------------------------------------------------------------------------
// AccountAdjustment

type AccountAdjustment struct {
	value  native.AccountAdjustment
	retain retainAccountAdjustment
}

// retainAccountAdjustment keeps value objects alive while the C struct's like
// PitStringView fields point to their C-heap buffers. For example, see
// param/asset.go and internal/native/string.go for the full explanation.
type retainAccountAdjustment struct {
	// Balance-operation asset.
	BalanceOperationAsset param.Asset
	// Position-operation instrument (holds two assets: underlying + settlement).
	PositionOperationInstrument param.Instrument
	// Position-operation collateral asset.
	PositionOperationCollateralAsset param.Asset
}

func NewAccountAdjustment() AccountAdjustment {
	return NewAccountAdjustmentFromHandle(native.NewAccountAdjustment())
}

type AccountAdjustmentValues struct {
	BalanceOperation  optional.Option[AccountAdjustmentBalanceOperation]
	PositionOperation optional.Option[AccountAdjustmentPositionOperation]
	Amount            optional.Option[AccountAdjustmentAmount]
	Bounds            optional.Option[AccountAdjustmentBounds]
}

func (v AccountAdjustmentValues) Check() error {
	if v.BalanceOperation.IsSet() && v.PositionOperation.IsSet() {
		return errors.New("cannot set both BalanceOperation and PositionOperation")
	}
	return nil
}

func NewAccountAdjustmentFromValues(values AccountAdjustmentValues) (AccountAdjustment, error) {
	if err := values.Check(); err != nil {
		return AccountAdjustment{}, err
	}
	a := NewAccountAdjustment()
	a.setValues(values)
	return a, nil
}

func NewAccountAdjustmentFromHandle(handle native.AccountAdjustment) AccountAdjustment {
	return AccountAdjustment{value: handle}
}

func (a *AccountAdjustment) Reset() {
	native.AccountAdjustmentReset(&a.value)
	a.retain = retainAccountAdjustment{}
}

func (a AccountAdjustment) Values() AccountAdjustmentValues {
	return AccountAdjustmentValues{
		BalanceOperation:  a.BalanceOperation(),
		PositionOperation: a.PositionOperation(),
		Amount:            a.Amount(),
		Bounds:            a.Bounds(),
	}
}

func (a *AccountAdjustment) SetValues(values AccountAdjustmentValues) error {
	if err := values.Check(); err != nil {
		return err
	}
	a.Reset()
	a.setValues(values)
	return nil
}

func (a *AccountAdjustment) setValues(values AccountAdjustmentValues) {
	if value, ok := values.BalanceOperation.Get(); ok {
		a.SetBalanceOperationAndUnsetPositionOperation(value)
	}
	if value, ok := values.PositionOperation.Get(); ok {
		a.SetPositionOperationAndUnsetBalanceOperation(value)
	}
	if value, ok := values.Amount.Get(); ok {
		a.SetAmount(value)
	}
	if value, ok := values.Bounds.Get(); ok {
		a.SetBounds(value)
	}
}

func (a AccountAdjustment) BalanceOperation() optional.Option[AccountAdjustmentBalanceOperation] {
	operation := native.AccountAdjustmentGetBalanceOperation(a.value)
	if !native.AccountAdjustmentBalanceOperationOptionalIsSet(operation) {
		return optional.None[AccountAdjustmentBalanceOperation]()
	}
	return optional.Some(
		newAccountAdjustmentBalanceOperation(
			native.AccountAdjustmentBalanceOperationOptionalGet(operation),
		),
	)
}

// EnsureBalanceOperationView ensures the balance operation exists, unsets the
// position operation, and returns a mutable balance operation view.
func (a *AccountAdjustment) EnsureBalanceOperationView() AccountAdjustmentBalanceOperationView {
	operation := native.AccountAdjustmentGetBalanceOperationView(&a.value)
	if !native.AccountAdjustmentBalanceOperationOptionalIsSet(*operation) {
		native.AccountAdjustmentBalanceOperationOptionalSet(
			operation,
			native.NewAccountAdjustmentBalanceOperation(),
		)
	}
	result := newAccountAdjustmentBalanceOperationView(
		native.AccountAdjustmentBalanceOperationOptionalGetView(operation),
		&a.retain.BalanceOperationAsset,
	)
	native.AccountAdjustmentUnsetPositionOperation(&a.value)
	a.retain.PositionOperationInstrument = param.Instrument{}
	a.retain.PositionOperationCollateralAsset = param.Asset{}
	return result
}

func (a *AccountAdjustment) SetBalanceOperationAndUnsetPositionOperation(
	operation AccountAdjustmentBalanceOperation,
) {
	native.AccountAdjustmentSetBalanceOperationAndUnsetPositionOperation(&a.value, operation.value)
	// Propagate asset retention from the inner value struct so the C buffer
	// stays alive even after the caller's local `operation` is collected.
	a.retain.BalanceOperationAsset = operation.retainAsset
	a.retain.PositionOperationInstrument = param.Instrument{}
	a.retain.PositionOperationCollateralAsset = param.Asset{}
}

func (a *AccountAdjustment) UnsetBalanceOperation() {
	native.AccountAdjustmentUnsetBalanceOperation(&a.value)
	a.retain.BalanceOperationAsset = param.Asset{}
}

func (a AccountAdjustment) PositionOperation() optional.Option[AccountAdjustmentPositionOperation] {
	operation := native.AccountAdjustmentGetPositionOperation(a.value)
	if !native.AccountAdjustmentPositionOperationOptionalIsSet(operation) {
		return optional.None[AccountAdjustmentPositionOperation]()
	}
	return optional.Some(
		newAccountAdjustmentPositionOperation(
			native.AccountAdjustmentPositionOperationOptionalGet(operation),
		),
	)
}

// EnsurePositionOperationView ensures the position operation exists, unsets the
// balance operation, and returns a mutable position operation view.
func (a *AccountAdjustment) EnsurePositionOperationView() AccountAdjustmentPositionOperationView {
	operation := native.AccountAdjustmentGetPositionOperationView(&a.value)
	if !native.AccountAdjustmentPositionOperationOptionalIsSet(*operation) {
		native.AccountAdjustmentPositionOperationOptionalSet(
			operation,
			native.NewAccountAdjustmentPositionOperation(),
		)
	}
	result := newAccountAdjustmentPositionOperationView(
		native.AccountAdjustmentPositionOperationOptionalGetView(operation),
		&a.retain.PositionOperationInstrument,
		&a.retain.PositionOperationCollateralAsset,
	)
	native.AccountAdjustmentUnsetBalanceOperation(&a.value)
	a.retain.BalanceOperationAsset = param.Asset{}
	return result
}

func (a *AccountAdjustment) SetPositionOperationAndUnsetBalanceOperation(
	operation AccountAdjustmentPositionOperation,
) {
	native.AccountAdjustmentSetPositionOperationAndUnsetBalanceOperation(&a.value, operation.value)
	// Propagate asset retention from the inner value struct so C buffers
	// stay alive even after the caller's local `operation` is collected.
	a.retain.PositionOperationInstrument = operation.retainInstrument
	a.retain.PositionOperationCollateralAsset = operation.retainCollateralAsset
	a.retain.BalanceOperationAsset = param.Asset{}
}

func (a *AccountAdjustment) UnsetPositionOperation() {
	native.AccountAdjustmentUnsetPositionOperation(&a.value)
	a.retain.PositionOperationInstrument = param.Instrument{}
	a.retain.PositionOperationCollateralAsset = param.Asset{}
}

func (a AccountAdjustment) Amount() optional.Option[AccountAdjustmentAmount] {
	amount := native.AccountAdjustmentGetAmount(a.value)
	if !native.AccountAdjustmentAmountOptionalIsSet(amount) {
		return optional.None[AccountAdjustmentAmount]()
	}
	return optional.Some(
		newAccountAdjustmentAmount(native.AccountAdjustmentAmountOptionalGet(amount)),
	)
}

func (a *AccountAdjustment) EnsureAmountView() AccountAdjustmentAmountView {
	amount := native.AccountAdjustmentGetAmountView(&a.value)
	if !native.AccountAdjustmentAmountOptionalIsSet(*amount) {
		native.AccountAdjustmentAmountOptionalSet(amount, native.NewAccountAdjustmentAmount())
	}
	return newAccountAdjustmentAmountView(native.AccountAdjustmentAmountOptionalGetView(amount))
}

func (a *AccountAdjustment) SetAmount(amount AccountAdjustmentAmount) {
	native.AccountAdjustmentSetAmount(&a.value, amount.value)
}

func (a *AccountAdjustment) UnsetAmount() {
	native.AccountAdjustmentUnsetAmount(&a.value)
}

func (a AccountAdjustment) Bounds() optional.Option[AccountAdjustmentBounds] {
	bounds := native.AccountAdjustmentGetBounds(a.value)
	if !native.AccountAdjustmentBoundsOptionalIsSet(bounds) {
		return optional.None[AccountAdjustmentBounds]()
	}
	return optional.Some(
		newAccountAdjustmentBounds(native.AccountAdjustmentBoundsOptionalGet(bounds)),
	)
}

func (a *AccountAdjustment) EnsureBoundsView() AccountAdjustmentBoundsView {
	bounds := native.AccountAdjustmentGetBoundsView(&a.value)
	if !native.AccountAdjustmentBoundsOptionalIsSet(*bounds) {
		native.AccountAdjustmentBoundsOptionalSet(bounds, native.NewAccountAdjustmentBounds())
	}
	return newAccountAdjustmentBoundsView(native.AccountAdjustmentBoundsOptionalGetView(bounds))
}

func (a *AccountAdjustment) SetBounds(bounds AccountAdjustmentBounds) {
	native.AccountAdjustmentSetBounds(&a.value, bounds.value)
}

func (a *AccountAdjustment) UnsetBounds() {
	native.AccountAdjustmentUnsetBounds(&a.value)
}

// EngineAccountAdjustment returns this adjustment as the standard engine
// adjustment view.
func (a AccountAdjustment) EngineAccountAdjustment() AccountAdjustment {
	return a
}

func (a AccountAdjustment) Handle() native.AccountAdjustment {
	return a.value
}

//------------------------------------------------------------------------------
// AccountAdjustmentBalanceOperation

type AccountAdjustmentBalanceOperation struct {
	value native.AccountAdjustmentBalanceOperation

	// retainAsset keeps the Asset alive while the C struct's PitStringView
	// points to its C-heap buffer.  See AccountAdjustment for the full
	// explanation of the retain pattern.
	retainAsset param.Asset
}

type AccountAdjustmentBalanceOperationValues struct {
	Asset             optional.Option[param.Asset]
	AverageEntryPrice optional.Option[param.Price]
}

func NewAccountAdjustmentBalanceOperation() AccountAdjustmentBalanceOperation {
	return newAccountAdjustmentBalanceOperation(native.NewAccountAdjustmentBalanceOperation())
}

func NewAccountAdjustmentBalanceOperationFromValues(
	values AccountAdjustmentBalanceOperationValues,
) AccountAdjustmentBalanceOperation {
	o := NewAccountAdjustmentBalanceOperation()
	o.setValues(values)
	return o
}

func newAccountAdjustmentBalanceOperation(
	value native.AccountAdjustmentBalanceOperation,
) AccountAdjustmentBalanceOperation {
	return AccountAdjustmentBalanceOperation{value: value}
}

func (o *AccountAdjustmentBalanceOperation) Reset() {
	native.AccountAdjustmentBalanceOperationReset(&o.value)
	o.retainAsset = param.Asset{}
}

func (o AccountAdjustmentBalanceOperation) Values() AccountAdjustmentBalanceOperationValues {
	return AccountAdjustmentBalanceOperationValues{
		Asset:             o.Asset(),
		AverageEntryPrice: o.AverageEntryPrice(),
	}
}

func (o *AccountAdjustmentBalanceOperation) SetValues(
	values AccountAdjustmentBalanceOperationValues,
) {
	o.Reset()
	o.setValues(values)
}

func (o *AccountAdjustmentBalanceOperation) setValues(
	values AccountAdjustmentBalanceOperationValues,
) {
	if value, ok := values.Asset.Get(); ok {
		o.SetAsset(value)
	}
	if value, ok := values.AverageEntryPrice.Get(); ok {
		o.SetAverageEntryPrice(value)
	}
}

func (o AccountAdjustmentBalanceOperation) Asset() optional.Option[param.Asset] {
	return param.NewAssetFromHandle(native.AccountAdjustmentBalanceOperationGetAsset(o.value))
}

func (o *AccountAdjustmentBalanceOperation) SetAsset(asset param.Asset) {
	native.AccountAdjustmentBalanceOperationSetAsset(&o.value, asset.Handle())
	o.retainAsset = asset
}

func (o *AccountAdjustmentBalanceOperation) UnsetAsset() {
	native.AccountAdjustmentBalanceOperationUnsetAsset(&o.value)
	o.retainAsset = param.Asset{}
}

func (o AccountAdjustmentBalanceOperation) AverageEntryPrice() optional.Option[param.Price] {
	return param.NewPriceOptionFromHandle(
		native.AccountAdjustmentBalanceOperationGetAverageEntryPrice(o.value),
	)
}

func (o *AccountAdjustmentBalanceOperation) SetAverageEntryPrice(price param.Price) {
	native.AccountAdjustmentBalanceOperationSetAverageEntryPrice(&o.value, price.Handle())
}

func (o *AccountAdjustmentBalanceOperation) UnsetAverageEntryPrice() {
	native.AccountAdjustmentBalanceOperationUnsetAverageEntryPrice(&o.value)
}

type AccountAdjustmentBalanceOperationView struct {
	ref *native.AccountAdjustmentBalanceOperation
	// retainAsset points to the owning AccountAdjustment's
	// retainBalanceOperationAsset field so that SetAsset/UnsetAsset on the
	// view propagate retention to the parent without requiring the caller to
	// hold the Asset separately.
	retainAsset *param.Asset
}

func newAccountAdjustmentBalanceOperationView(
	ref *native.AccountAdjustmentBalanceOperation,
	retainAsset *param.Asset,
) AccountAdjustmentBalanceOperationView {
	return AccountAdjustmentBalanceOperationView{ref: ref, retainAsset: retainAsset}
}

func (o *AccountAdjustmentBalanceOperationView) Reset() {
	native.AccountAdjustmentBalanceOperationReset(o.ref)
	*o.retainAsset = param.Asset{}
}

func (o AccountAdjustmentBalanceOperationView) Asset() optional.Option[param.Asset] {
	return param.NewAssetFromHandle(native.AccountAdjustmentBalanceOperationGetAsset(*o.ref))
}

func (o *AccountAdjustmentBalanceOperationView) SetAsset(asset param.Asset) {
	native.AccountAdjustmentBalanceOperationSetAsset(o.ref, asset.Handle())
	*o.retainAsset = asset
}

func (o *AccountAdjustmentBalanceOperationView) UnsetAsset() {
	native.AccountAdjustmentBalanceOperationUnsetAsset(o.ref)
	*o.retainAsset = param.Asset{}
}

func (o AccountAdjustmentBalanceOperationView) AverageEntryPrice() optional.Option[param.Price] {
	return param.NewPriceOptionFromHandle(
		native.AccountAdjustmentBalanceOperationGetAverageEntryPrice(*o.ref),
	)
}

func (o *AccountAdjustmentBalanceOperationView) SetAverageEntryPrice(price param.Price) {
	native.AccountAdjustmentBalanceOperationSetAverageEntryPrice(o.ref, price.Handle())
}

func (o *AccountAdjustmentBalanceOperationView) UnsetAverageEntryPrice() {
	native.AccountAdjustmentBalanceOperationUnsetAverageEntryPrice(o.ref)
}

//------------------------------------------------------------------------------
// AccountAdjustmentPositionOperation

type AccountAdjustmentPositionOperation struct {
	value native.AccountAdjustmentPositionOperation

	// retainInstrument and retainCollateralAsset keep the Assets alive while
	// the C struct's PitStringView fields point to their C-heap buffers.
	// See AccountAdjustment for the full explanation of the retain pattern.
	retainInstrument      param.Instrument
	retainCollateralAsset param.Asset
}

type AccountAdjustmentPositionOperationValues struct {
	Instrument        optional.Option[param.Instrument]
	CollateralAsset   optional.Option[param.Asset]
	AverageEntryPrice optional.Option[param.Price]
	Leverage          optional.Option[param.Leverage]
	Mode              optional.Option[param.PositionMode]
}

func NewAccountAdjustmentPositionOperation() AccountAdjustmentPositionOperation {
	return newAccountAdjustmentPositionOperation(native.NewAccountAdjustmentPositionOperation())
}

func NewAccountAdjustmentPositionOperationFromValues(
	values AccountAdjustmentPositionOperationValues,
) AccountAdjustmentPositionOperation {
	o := NewAccountAdjustmentPositionOperation()
	o.setValues(values)
	return o
}

func newAccountAdjustmentPositionOperation(
	value native.AccountAdjustmentPositionOperation,
) AccountAdjustmentPositionOperation {
	return AccountAdjustmentPositionOperation{value: value}
}

func (o *AccountAdjustmentPositionOperation) Reset() {
	native.AccountAdjustmentPositionOperationReset(&o.value)
	o.retainInstrument = param.Instrument{}
	o.retainCollateralAsset = param.Asset{}
}

func (
	o AccountAdjustmentPositionOperation,
) Values() AccountAdjustmentPositionOperationValues {
	return AccountAdjustmentPositionOperationValues{
		Instrument:        o.Instrument(),
		CollateralAsset:   o.CollateralAsset(),
		AverageEntryPrice: o.AverageEntryPrice(),
		Leverage:          o.Leverage(),
		Mode:              o.Mode(),
	}
}

func (o *AccountAdjustmentPositionOperation) SetValues(
	values AccountAdjustmentPositionOperationValues,
) {
	o.Reset()
	o.setValues(values)
}

func (o *AccountAdjustmentPositionOperation) setValues(
	values AccountAdjustmentPositionOperationValues,
) {
	if value, ok := values.Instrument.Get(); ok {
		o.SetInstrument(value)
	}
	if value, ok := values.CollateralAsset.Get(); ok {
		o.SetCollateralAsset(value)
	}
	if value, ok := values.AverageEntryPrice.Get(); ok {
		o.SetAverageEntryPrice(value)
	}
	if value, ok := values.Leverage.Get(); ok {
		o.SetLeverage(value)
	}
	if value, ok := values.Mode.Get(); ok {
		o.SetMode(value)
	}
}

func (o AccountAdjustmentPositionOperation) Instrument() optional.Option[param.Instrument] {
	return param.NewInstrumentFromHandle(
		native.AccountAdjustmentPositionOperationGetInstrument(o.value),
	)
}

func (o *AccountAdjustmentPositionOperation) SetInstrument(instrument param.Instrument) {
	native.AccountAdjustmentPositionOperationSetInstrument(&o.value, instrument.Handle())
	o.retainInstrument = instrument
}

func (o *AccountAdjustmentPositionOperation) UnsetInstrument() {
	native.AccountAdjustmentPositionOperationUnsetInstrument(&o.value)
	o.retainInstrument = param.Instrument{}
}

func (o AccountAdjustmentPositionOperation) CollateralAsset() optional.Option[param.Asset] {
	return param.NewAssetFromHandle(
		native.AccountAdjustmentPositionOperationGetCollateralAsset(o.value),
	)
}

func (o *AccountAdjustmentPositionOperation) SetCollateralAsset(asset param.Asset) {
	native.AccountAdjustmentPositionOperationSetCollateralAsset(&o.value, asset.Handle())
	o.retainCollateralAsset = asset
}

func (o *AccountAdjustmentPositionOperation) UnsetCollateralAsset() {
	native.AccountAdjustmentPositionOperationUnsetCollateralAsset(&o.value)
	o.retainCollateralAsset = param.Asset{}
}

func (o AccountAdjustmentPositionOperation) AverageEntryPrice() optional.Option[param.Price] {
	return param.NewPriceOptionFromHandle(
		native.AccountAdjustmentPositionOperationGetAverageEntryPrice(o.value),
	)
}

func (o *AccountAdjustmentPositionOperation) SetAverageEntryPrice(price param.Price) {
	native.AccountAdjustmentPositionOperationSetAverageEntryPrice(&o.value, price.Handle())
}

func (o *AccountAdjustmentPositionOperation) UnsetAverageEntryPrice() {
	native.AccountAdjustmentPositionOperationUnsetAverageEntryPrice(&o.value)
}

func (o AccountAdjustmentPositionOperation) Leverage() optional.Option[param.Leverage] {
	return param.NewLeverageOptionFromHandle(
		native.AccountAdjustmentPositionOperationGetLeverage(o.value),
	)
}

func (o *AccountAdjustmentPositionOperation) SetLeverage(leverage param.Leverage) {
	native.AccountAdjustmentPositionOperationSetLeverage(&o.value, leverage.Handle())
}

func (o *AccountAdjustmentPositionOperation) UnsetLeverage() {
	native.AccountAdjustmentPositionOperationUnsetLeverage(&o.value)
}

func (o AccountAdjustmentPositionOperation) Mode() optional.Option[param.PositionMode] {
	return param.NewPositionModeFromHandle(native.AccountAdjustmentPositionOperationGetMode(o.value))
}

func (o *AccountAdjustmentPositionOperation) SetMode(mode param.PositionMode) {
	native.AccountAdjustmentPositionOperationSetMode(&o.value, mode.Handle())
}

func (o *AccountAdjustmentPositionOperation) UnsetMode() {
	native.AccountAdjustmentPositionOperationUnsetMode(&o.value)
}

type AccountAdjustmentPositionOperationView struct {
	ref *native.AccountAdjustmentPositionOperation
	// retainInstrument and retainCollateralAsset point to the owning
	// AccountAdjustment's corresponding retain fields so that Set/Unset calls
	// on this view propagate retention to the parent automatically.
	retainInstrument      *param.Instrument
	retainCollateralAsset *param.Asset
}

func newAccountAdjustmentPositionOperationView(
	ref *native.AccountAdjustmentPositionOperation,
	retainInstrument *param.Instrument,
	retainCollateralAsset *param.Asset,
) AccountAdjustmentPositionOperationView {
	return AccountAdjustmentPositionOperationView{
		ref:                   ref,
		retainInstrument:      retainInstrument,
		retainCollateralAsset: retainCollateralAsset,
	}
}

func (o *AccountAdjustmentPositionOperationView) Reset() {
	native.AccountAdjustmentPositionOperationReset(o.ref)
	*o.retainInstrument = param.Instrument{}
	*o.retainCollateralAsset = param.Asset{}
}

func (o AccountAdjustmentPositionOperationView) Instrument() optional.Option[param.Instrument] {
	return param.NewInstrumentFromHandle(
		native.AccountAdjustmentPositionOperationGetInstrument(*o.ref),
	)
}

func (o *AccountAdjustmentPositionOperationView) SetInstrument(instrument param.Instrument) {
	native.AccountAdjustmentPositionOperationSetInstrument(o.ref, instrument.Handle())
	*o.retainInstrument = instrument
}

func (o *AccountAdjustmentPositionOperationView) UnsetInstrument() {
	native.AccountAdjustmentPositionOperationUnsetInstrument(o.ref)
	*o.retainInstrument = param.Instrument{}
}

func (o AccountAdjustmentPositionOperationView) CollateralAsset() optional.Option[param.Asset] {
	return param.NewAssetFromHandle(
		native.AccountAdjustmentPositionOperationGetCollateralAsset(*o.ref),
	)
}

func (o *AccountAdjustmentPositionOperationView) SetCollateralAsset(asset param.Asset) {
	native.AccountAdjustmentPositionOperationSetCollateralAsset(o.ref, asset.Handle())
	*o.retainCollateralAsset = asset
}

func (o *AccountAdjustmentPositionOperationView) UnsetCollateralAsset() {
	native.AccountAdjustmentPositionOperationUnsetCollateralAsset(o.ref)
	*o.retainCollateralAsset = param.Asset{}
}

func (o AccountAdjustmentPositionOperationView) AverageEntryPrice() optional.Option[param.Price] {
	return param.NewPriceOptionFromHandle(
		native.AccountAdjustmentPositionOperationGetAverageEntryPrice(*o.ref),
	)
}

func (o *AccountAdjustmentPositionOperationView) SetAverageEntryPrice(price param.Price) {
	native.AccountAdjustmentPositionOperationSetAverageEntryPrice(o.ref, price.Handle())
}

func (o *AccountAdjustmentPositionOperationView) UnsetAverageEntryPrice() {
	native.AccountAdjustmentPositionOperationUnsetAverageEntryPrice(o.ref)
}

func (o AccountAdjustmentPositionOperationView) Leverage() optional.Option[param.Leverage] {
	return param.NewLeverageOptionFromHandle(
		native.AccountAdjustmentPositionOperationGetLeverage(*o.ref),
	)
}

func (o *AccountAdjustmentPositionOperationView) SetLeverage(leverage param.Leverage) {
	native.AccountAdjustmentPositionOperationSetLeverage(o.ref, leverage.Handle())
}

func (o *AccountAdjustmentPositionOperationView) UnsetLeverage() {
	native.AccountAdjustmentPositionOperationUnsetLeverage(o.ref)
}

func (o AccountAdjustmentPositionOperationView) Mode() optional.Option[param.PositionMode] {
	return param.NewPositionModeFromHandle(native.AccountAdjustmentPositionOperationGetMode(*o.ref))
}

func (o *AccountAdjustmentPositionOperationView) SetMode(mode param.PositionMode) {
	native.AccountAdjustmentPositionOperationSetMode(o.ref, mode.Handle())
}

func (o *AccountAdjustmentPositionOperationView) UnsetMode() {
	native.AccountAdjustmentPositionOperationUnsetMode(o.ref)
}

//------------------------------------------------------------------------------
// AccountAdjustmentAmount

type AccountAdjustmentAmount struct {
	value native.AccountAdjustmentAmount
}

func NewAccountAdjustmentAmount() AccountAdjustmentAmount {
	return newAccountAdjustmentAmount(native.NewAccountAdjustmentAmount())
}

type AccountAdjustmentAmountValues struct {
	Total    optional.Option[param.AdjustmentAmount]
	Reserved optional.Option[param.AdjustmentAmount]
	Pending  optional.Option[param.AdjustmentAmount]
}

func NewAccountAdjustmentAmountFromValues(
	values AccountAdjustmentAmountValues,
) AccountAdjustmentAmount {
	a := NewAccountAdjustmentAmount()
	a.setValues(values)
	return a
}

func newAccountAdjustmentAmount(value native.AccountAdjustmentAmount) AccountAdjustmentAmount {
	return AccountAdjustmentAmount{value: value}
}

func (a *AccountAdjustmentAmount) Reset() {
	native.AccountAdjustmentAmountReset(&a.value)
}

func (a *AccountAdjustmentAmount) SetValues(values AccountAdjustmentAmountValues) {
	a.Reset()
	a.setValues(values)
}

func (a *AccountAdjustmentAmount) setValues(values AccountAdjustmentAmountValues) {
	if value, ok := values.Total.Get(); ok {
		a.SetTotal(value)
	}
	if value, ok := values.Reserved.Get(); ok {
		a.SetReserved(value)
	}
	if value, ok := values.Pending.Get(); ok {
		a.SetPending(value)
	}
}

func (a AccountAdjustmentAmount) Values() AccountAdjustmentAmountValues {
	return AccountAdjustmentAmountValues{
		Total:    a.Total(),
		Reserved: a.Reserved(),
		Pending:  a.Pending(),
	}
}

func (a AccountAdjustmentAmount) Total() optional.Option[param.AdjustmentAmount] {
	return param.NewAdjustmentAmountFromHandle(native.AccountAdjustmentAmountGetTotal(a.value))
}

func (a *AccountAdjustmentAmount) SetTotal(value param.AdjustmentAmount) {
	native.AccountAdjustmentAmountSetTotal(&a.value, value.Handle())
}

func (a *AccountAdjustmentAmount) UnsetTotal() {
	native.AccountAdjustmentAmountUnsetTotal(&a.value)
}

func (a AccountAdjustmentAmount) Reserved() optional.Option[param.AdjustmentAmount] {
	return param.NewAdjustmentAmountFromHandle(native.AccountAdjustmentAmountGetReserved(a.value))
}

func (a *AccountAdjustmentAmount) SetReserved(value param.AdjustmentAmount) {
	native.AccountAdjustmentAmountSetReserved(&a.value, value.Handle())
}

func (a *AccountAdjustmentAmount) UnsetReserved() {
	native.AccountAdjustmentAmountUnsetReserved(&a.value)
}

func (a AccountAdjustmentAmount) Pending() optional.Option[param.AdjustmentAmount] {
	return param.NewAdjustmentAmountFromHandle(native.AccountAdjustmentAmountGetPending(a.value))
}

func (a *AccountAdjustmentAmount) SetPending(value param.AdjustmentAmount) {
	native.AccountAdjustmentAmountSetPending(&a.value, value.Handle())
}

func (a *AccountAdjustmentAmount) UnsetPending() {
	native.AccountAdjustmentAmountUnsetPending(&a.value)
}

type AccountAdjustmentAmountView struct {
	ref *native.AccountAdjustmentAmount
}

func newAccountAdjustmentAmountView(
	ref *native.AccountAdjustmentAmount,
) AccountAdjustmentAmountView {
	return AccountAdjustmentAmountView{ref: ref}
}

func (a *AccountAdjustmentAmountView) Reset() {
	native.AccountAdjustmentAmountReset(a.ref)
}

func (a AccountAdjustmentAmountView) Total() optional.Option[param.AdjustmentAmount] {
	return param.NewAdjustmentAmountFromHandle(native.AccountAdjustmentAmountGetTotal(*a.ref))
}

func (a *AccountAdjustmentAmountView) SetTotal(value param.AdjustmentAmount) {
	native.AccountAdjustmentAmountSetTotal(a.ref, value.Handle())
}

func (a *AccountAdjustmentAmountView) UnsetTotal() {
	native.AccountAdjustmentAmountUnsetTotal(a.ref)
}

func (a AccountAdjustmentAmountView) Reserved() optional.Option[param.AdjustmentAmount] {
	return param.NewAdjustmentAmountFromHandle(native.AccountAdjustmentAmountGetReserved(*a.ref))
}

func (a *AccountAdjustmentAmountView) SetReserved(value param.AdjustmentAmount) {
	native.AccountAdjustmentAmountSetReserved(a.ref, value.Handle())
}

func (a *AccountAdjustmentAmountView) UnsetReserved() {
	native.AccountAdjustmentAmountUnsetReserved(a.ref)
}

func (a AccountAdjustmentAmountView) Pending() optional.Option[param.AdjustmentAmount] {
	return param.NewAdjustmentAmountFromHandle(native.AccountAdjustmentAmountGetPending(*a.ref))
}

func (a *AccountAdjustmentAmountView) SetPending(value param.AdjustmentAmount) {
	native.AccountAdjustmentAmountSetPending(a.ref, value.Handle())
}

func (a *AccountAdjustmentAmountView) UnsetPending() {
	native.AccountAdjustmentAmountUnsetPending(a.ref)
}

//------------------------------------------------------------------------------
// AccountAdjustmentBounds

type AccountAdjustmentBounds struct {
	value native.AccountAdjustmentBounds
}

func NewAccountAdjustmentBounds() AccountAdjustmentBounds {
	return newAccountAdjustmentBounds(native.NewAccountAdjustmentBounds())
}

type AccountAdjustmentBoundsValues struct {
	TotalUpper    optional.Option[param.PositionSize]
	TotalLower    optional.Option[param.PositionSize]
	ReservedUpper optional.Option[param.PositionSize]
	ReservedLower optional.Option[param.PositionSize]
	PendingUpper  optional.Option[param.PositionSize]
	PendingLower  optional.Option[param.PositionSize]
}

func NewAccountAdjustmentBoundsFromValues(
	values AccountAdjustmentBoundsValues,
) AccountAdjustmentBounds {
	b := NewAccountAdjustmentBounds()
	b.setValues(values)
	return b
}

func newAccountAdjustmentBounds(value native.AccountAdjustmentBounds) AccountAdjustmentBounds {
	return AccountAdjustmentBounds{value: value}
}

func (b *AccountAdjustmentBounds) Reset() {
	native.AccountAdjustmentBoundsReset(&b.value)
}

func (b AccountAdjustmentBounds) Values() AccountAdjustmentBoundsValues {
	return AccountAdjustmentBoundsValues{
		TotalUpper:    b.TotalUpper(),
		TotalLower:    b.TotalLower(),
		ReservedUpper: b.ReservedUpper(),
		ReservedLower: b.ReservedLower(),
		PendingUpper:  b.PendingUpper(),
		PendingLower:  b.PendingLower(),
	}
}

func (b *AccountAdjustmentBounds) SetValues(values AccountAdjustmentBoundsValues) {
	b.Reset()
	b.setValues(values)
}

func (b AccountAdjustmentBounds) setValues(values AccountAdjustmentBoundsValues) {
	if value, ok := values.TotalUpper.Get(); ok {
		b.SetTotalUpper(value)
	}
	if value, ok := values.TotalLower.Get(); ok {
		b.SetTotalLower(value)
	}
	if value, ok := values.ReservedUpper.Get(); ok {
		b.SetReservedUpper(value)
	}
	if value, ok := values.ReservedLower.Get(); ok {
		b.SetReservedLower(value)
	}
	if value, ok := values.PendingUpper.Get(); ok {
		b.SetPendingUpper(value)
	}
	if value, ok := values.PendingLower.Get(); ok {
		b.SetPendingLower(value)
	}
}

func (b AccountAdjustmentBounds) TotalUpper() optional.Option[param.PositionSize] {
	return param.NewPositionSizeOptionFromHandle(
		native.AccountAdjustmentBoundsGetTotalUpper(b.value),
	)
}

func (b *AccountAdjustmentBounds) SetTotalUpper(bound param.PositionSize) {
	native.AccountAdjustmentBoundsSetTotalUpper(&b.value, bound.Handle())
}

func (b *AccountAdjustmentBounds) UnsetTotalUpper() {
	native.AccountAdjustmentBoundsUnsetTotalUpper(&b.value)
}

func (b AccountAdjustmentBounds) TotalLower() optional.Option[param.PositionSize] {
	return param.NewPositionSizeOptionFromHandle(
		native.AccountAdjustmentBoundsGetTotalLower(b.value),
	)
}

func (b *AccountAdjustmentBounds) SetTotalLower(bound param.PositionSize) {
	native.AccountAdjustmentBoundsSetTotalLower(&b.value, bound.Handle())
}

func (b *AccountAdjustmentBounds) UnsetTotalLower() {
	native.AccountAdjustmentBoundsUnsetTotalLower(&b.value)
}

func (b AccountAdjustmentBounds) ReservedUpper() optional.Option[param.PositionSize] {
	return param.NewPositionSizeOptionFromHandle(
		native.AccountAdjustmentBoundsGetReservedUpper(b.value),
	)
}

func (b *AccountAdjustmentBounds) SetReservedUpper(bound param.PositionSize) {
	native.AccountAdjustmentBoundsSetReservedUpper(&b.value, bound.Handle())
}

func (b *AccountAdjustmentBounds) UnsetReservedUpper() {
	native.AccountAdjustmentBoundsUnsetReservedUpper(&b.value)
}

func (b AccountAdjustmentBounds) ReservedLower() optional.Option[param.PositionSize] {
	return param.NewPositionSizeOptionFromHandle(
		native.AccountAdjustmentBoundsGetReservedLower(b.value),
	)
}

func (b *AccountAdjustmentBounds) SetReservedLower(bound param.PositionSize) {
	native.AccountAdjustmentBoundsSetReservedLower(&b.value, bound.Handle())
}

func (b *AccountAdjustmentBounds) UnsetReservedLower() {
	native.AccountAdjustmentBoundsUnsetReservedLower(&b.value)
}

func (b AccountAdjustmentBounds) PendingUpper() optional.Option[param.PositionSize] {
	return param.NewPositionSizeOptionFromHandle(
		native.AccountAdjustmentBoundsGetPendingUpper(b.value),
	)
}

func (b *AccountAdjustmentBounds) SetPendingUpper(bound param.PositionSize) {
	native.AccountAdjustmentBoundsSetPendingUpper(&b.value, bound.Handle())
}

func (b *AccountAdjustmentBounds) UnsetPendingUpper() {
	native.AccountAdjustmentBoundsUnsetPendingUpper(&b.value)
}

func (b AccountAdjustmentBounds) PendingLower() optional.Option[param.PositionSize] {
	return param.NewPositionSizeOptionFromHandle(
		native.AccountAdjustmentBoundsGetPendingLower(b.value),
	)
}

func (b *AccountAdjustmentBounds) SetPendingLower(bound param.PositionSize) {
	native.AccountAdjustmentBoundsSetPendingLower(&b.value, bound.Handle())
}

func (b *AccountAdjustmentBounds) UnsetPendingLower() {
	native.AccountAdjustmentBoundsUnsetPendingLower(&b.value)
}

type AccountAdjustmentBoundsView struct {
	ref *native.AccountAdjustmentBounds
}

func newAccountAdjustmentBoundsView(
	ref *native.AccountAdjustmentBounds,
) AccountAdjustmentBoundsView {
	return AccountAdjustmentBoundsView{ref: ref}
}

func (b *AccountAdjustmentBoundsView) Reset() {
	native.AccountAdjustmentBoundsReset(b.ref)
}

func (b AccountAdjustmentBoundsView) TotalUpper() optional.Option[param.PositionSize] {
	return param.NewPositionSizeOptionFromHandle(
		native.AccountAdjustmentBoundsGetTotalUpper(*b.ref),
	)
}

func (b *AccountAdjustmentBoundsView) SetTotalUpper(bound param.PositionSize) {
	native.AccountAdjustmentBoundsSetTotalUpper(b.ref, bound.Handle())
}

func (b *AccountAdjustmentBoundsView) UnsetTotalUpper() {
	native.AccountAdjustmentBoundsUnsetTotalUpper(b.ref)
}

func (b AccountAdjustmentBoundsView) TotalLower() optional.Option[param.PositionSize] {
	return param.NewPositionSizeOptionFromHandle(
		native.AccountAdjustmentBoundsGetTotalLower(*b.ref),
	)
}

func (b *AccountAdjustmentBoundsView) SetTotalLower(bound param.PositionSize) {
	native.AccountAdjustmentBoundsSetTotalLower(b.ref, bound.Handle())
}

func (b *AccountAdjustmentBoundsView) UnsetTotalLower() {
	native.AccountAdjustmentBoundsUnsetTotalLower(b.ref)
}

func (b AccountAdjustmentBoundsView) ReservedUpper() optional.Option[param.PositionSize] {
	return param.NewPositionSizeOptionFromHandle(
		native.AccountAdjustmentBoundsGetReservedUpper(*b.ref),
	)
}

func (b *AccountAdjustmentBoundsView) SetReservedUpper(bound param.PositionSize) {
	native.AccountAdjustmentBoundsSetReservedUpper(b.ref, bound.Handle())
}

func (b *AccountAdjustmentBoundsView) UnsetReservedUpper() {
	native.AccountAdjustmentBoundsUnsetReservedUpper(b.ref)
}

func (b AccountAdjustmentBoundsView) ReservedLower() optional.Option[param.PositionSize] {
	return param.NewPositionSizeOptionFromHandle(
		native.AccountAdjustmentBoundsGetReservedLower(*b.ref),
	)
}

func (b *AccountAdjustmentBoundsView) SetReservedLower(bound param.PositionSize) {
	native.AccountAdjustmentBoundsSetReservedLower(b.ref, bound.Handle())
}

func (b *AccountAdjustmentBoundsView) UnsetReservedLower() {
	native.AccountAdjustmentBoundsUnsetReservedLower(b.ref)
}

func (b AccountAdjustmentBoundsView) PendingUpper() optional.Option[param.PositionSize] {
	return param.NewPositionSizeOptionFromHandle(
		native.AccountAdjustmentBoundsGetPendingUpper(*b.ref),
	)
}

func (b *AccountAdjustmentBoundsView) SetPendingUpper(bound param.PositionSize) {
	native.AccountAdjustmentBoundsSetPendingUpper(b.ref, bound.Handle())
}

func (b *AccountAdjustmentBoundsView) UnsetPendingUpper() {
	native.AccountAdjustmentBoundsUnsetPendingUpper(b.ref)
}

func (b AccountAdjustmentBoundsView) PendingLower() optional.Option[param.PositionSize] {
	return param.NewPositionSizeOptionFromHandle(
		native.AccountAdjustmentBoundsGetPendingLower(*b.ref),
	)
}

func (b *AccountAdjustmentBoundsView) SetPendingLower(bound param.PositionSize) {
	native.AccountAdjustmentBoundsSetPendingLower(b.ref, bound.Handle())
}

func (b *AccountAdjustmentBoundsView) UnsetPendingLower() {
	native.AccountAdjustmentBoundsUnsetPendingLower(b.ref)
}

//------------------------------------------------------------------------------
