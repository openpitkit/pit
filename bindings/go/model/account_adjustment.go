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
// Please see https://openpit.dev and the OWNERS file for details.

// Package model provides engine model types such as Order, ExecutionReport, and AccountAdjustment.
package model

import (
	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/optional"
)

//------------------------------------------------------------------------------
// AccountAdjustment

// AccountAdjustment represents a pending account state mutation.
type AccountAdjustment struct {
	value  native.AccountAdjustment
	retain retainAccountAdjustment
}

// retainAccountAdjustment keeps value objects alive while the C struct's like
// OpenPitStringView fields point to their C-heap buffers. For example, see
// param/asset.go and internal/native/string.go for the full explanation.
type retainAccountAdjustment struct {
	// Balance-operation asset.
	BalanceOperationAsset param.Asset
	// Position-operation instrument (holds two assets: underlying + settlement).
	PositionOperationInstrument param.Instrument
	// Position-operation collateral asset.
	PositionOperationCollateralAsset param.Asset
}

// NewAccountAdjustment creates a new zeroed AccountAdjustment.
func NewAccountAdjustment() AccountAdjustment {
	return NewAccountAdjustmentFromHandle(native.NewAccountAdjustment())
}

// AccountAdjustmentValues holds the optional fields of an AccountAdjustment.
//
// The operation is a faithful sum: BalanceOperation and PositionOperation are
// mutually exclusive — at most one is ever set, the other being Absent. The
// native discriminated operation enforces this structurally, so supplying both
// is not representable and the last one applied wins.
type AccountAdjustmentValues struct {
	BalanceOperation  optional.Option[AccountAdjustmentBalanceOperation]
	PositionOperation optional.Option[AccountAdjustmentPositionOperation]
	Amount            optional.Option[AccountAdjustmentAmount]
	Bounds            optional.Option[AccountAdjustmentBounds]
}

// NewAccountAdjustmentFromValues creates an AccountAdjustment from the given values.
//
// The error result is retained for API stability; it is always nil because the
// discriminated operation makes a conflicting balance/position pair structurally
// impossible.
func NewAccountAdjustmentFromValues(values AccountAdjustmentValues) (AccountAdjustment, error) {
	a := NewAccountAdjustment()
	a.setValues(values)
	return a, nil
}

// NewAccountAdjustmentFromHandle creates an AccountAdjustment from a native handle.
func NewAccountAdjustmentFromHandle(handle native.AccountAdjustment) AccountAdjustment {
	return AccountAdjustment{value: handle}
}

// Reset zeroes out the adjustment and clears all retained references.
func (a *AccountAdjustment) Reset() {
	native.AccountAdjustmentReset(&a.value)
	a.retain = retainAccountAdjustment{}
}

// Values returns a copy of the current adjustment fields.
func (a AccountAdjustment) Values() AccountAdjustmentValues {
	return AccountAdjustmentValues{
		BalanceOperation:  a.BalanceOperation(),
		PositionOperation: a.PositionOperation(),
		Amount:            a.Amount(),
		Bounds:            a.Bounds(),
	}
}

// SetValues resets the adjustment and applies the provided values.
//
// The error result is retained for API stability; it is always nil because the
// discriminated operation makes a conflicting balance/position pair structurally
// impossible.
func (a *AccountAdjustment) SetValues(values AccountAdjustmentValues) error {
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

// BalanceOperation returns the optional balance operation. It is set only when
// the discriminated operation selects the balance kind.
func (a AccountAdjustment) BalanceOperation() optional.Option[AccountAdjustmentBalanceOperation] {
	if native.AccountAdjustmentGetOperationKind(a.value) != native.AccountAdjustmentOperationKindBalance {
		return optional.None[AccountAdjustmentBalanceOperation]()
	}
	return optional.Some(
		newAccountAdjustmentBalanceOperation(
			native.AccountAdjustmentGetBalanceOperation(a.value),
		),
	)
}

// EnsureBalanceOperationView ensures the balance operation exists, unsets the
// position operation, and returns a mutable balance operation view.
func (a *AccountAdjustment) EnsureBalanceOperationView() AccountAdjustmentBalanceOperationView {
	native.AccountAdjustmentSelectBalanceOperationAndUnsetPositionOperation(&a.value)
	result := newAccountAdjustmentBalanceOperationView(
		native.AccountAdjustmentGetBalanceOperationView(&a.value),
		&a.retain.BalanceOperationAsset,
	)
	a.retain.PositionOperationInstrument = param.Instrument{}
	a.retain.PositionOperationCollateralAsset = param.Asset{}
	return result
}

// SetBalanceOperationAndUnsetPositionOperation sets the balance operation and clears any position operation.
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

// UnsetBalanceOperation clears the balance operation.
func (a *AccountAdjustment) UnsetBalanceOperation() {
	native.AccountAdjustmentUnsetBalanceOperation(&a.value)
	a.retain.BalanceOperationAsset = param.Asset{}
}

// PositionOperation returns the optional position operation. It is set only when
// the discriminated operation selects the position kind.
func (a AccountAdjustment) PositionOperation() optional.Option[AccountAdjustmentPositionOperation] {
	if native.AccountAdjustmentGetOperationKind(a.value) != native.AccountAdjustmentOperationKindPosition {
		return optional.None[AccountAdjustmentPositionOperation]()
	}
	return optional.Some(
		newAccountAdjustmentPositionOperation(
			native.AccountAdjustmentGetPositionOperation(a.value),
		),
	)
}

// EnsurePositionOperationView ensures the position operation exists, unsets the
// balance operation, and returns a mutable position operation view.
func (a *AccountAdjustment) EnsurePositionOperationView() AccountAdjustmentPositionOperationView {
	native.AccountAdjustmentSelectPositionOperationAndUnsetBalanceOperation(&a.value)
	result := newAccountAdjustmentPositionOperationView(
		native.AccountAdjustmentGetPositionOperationView(&a.value),
		&a.retain.PositionOperationInstrument,
		&a.retain.PositionOperationCollateralAsset,
	)
	a.retain.BalanceOperationAsset = param.Asset{}
	return result
}

// SetPositionOperationAndUnsetBalanceOperation sets the position operation and clears any balance operation.
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

// UnsetPositionOperation clears the position operation.
func (a *AccountAdjustment) UnsetPositionOperation() {
	native.AccountAdjustmentUnsetPositionOperation(&a.value)
	a.retain.PositionOperationInstrument = param.Instrument{}
	a.retain.PositionOperationCollateralAsset = param.Asset{}
}

// Amount returns the optional adjustment amount.
func (a AccountAdjustment) Amount() optional.Option[AccountAdjustmentAmount] {
	amount := native.AccountAdjustmentGetAmount(a.value)
	if !native.AccountAdjustmentAmountOptionalIsSet(amount) {
		return optional.None[AccountAdjustmentAmount]()
	}
	return optional.Some(
		newAccountAdjustmentAmount(native.AccountAdjustmentAmountOptionalGet(amount)),
	)
}

// EnsureAmountView ensures the amount exists and returns a mutable amount view.
func (a *AccountAdjustment) EnsureAmountView() AccountAdjustmentAmountView {
	amount := native.AccountAdjustmentGetAmountView(&a.value)
	if !native.AccountAdjustmentAmountOptionalIsSet(*amount) {
		native.AccountAdjustmentAmountOptionalSet(amount, native.NewAccountAdjustmentAmount())
	}
	return newAccountAdjustmentAmountView(native.AccountAdjustmentAmountOptionalGetView(amount))
}

// SetAmount sets the adjustment amount.
func (a *AccountAdjustment) SetAmount(amount AccountAdjustmentAmount) {
	native.AccountAdjustmentSetAmount(&a.value, amount.value)
}

// UnsetAmount clears the adjustment amount.
func (a *AccountAdjustment) UnsetAmount() {
	native.AccountAdjustmentUnsetAmount(&a.value)
}

// Bounds returns the optional adjustment bounds.
func (a AccountAdjustment) Bounds() optional.Option[AccountAdjustmentBounds] {
	bounds := native.AccountAdjustmentGetBounds(a.value)
	if !native.AccountAdjustmentBoundsOptionalIsSet(bounds) {
		return optional.None[AccountAdjustmentBounds]()
	}
	return optional.Some(
		newAccountAdjustmentBounds(native.AccountAdjustmentBoundsOptionalGet(bounds)),
	)
}

// EnsureBoundsView ensures the bounds exist and returns a mutable bounds view.
func (a *AccountAdjustment) EnsureBoundsView() AccountAdjustmentBoundsView {
	bounds := native.AccountAdjustmentGetBoundsView(&a.value)
	if !native.AccountAdjustmentBoundsOptionalIsSet(*bounds) {
		native.AccountAdjustmentBoundsOptionalSet(bounds, native.NewAccountAdjustmentBounds())
	}
	return newAccountAdjustmentBoundsView(native.AccountAdjustmentBoundsOptionalGetView(bounds))
}

// SetBounds sets the adjustment bounds.
func (a *AccountAdjustment) SetBounds(bounds AccountAdjustmentBounds) {
	native.AccountAdjustmentSetBounds(&a.value, bounds.value)
}

// UnsetBounds clears the adjustment bounds.
func (a *AccountAdjustment) UnsetBounds() {
	native.AccountAdjustmentUnsetBounds(&a.value)
}

// EngineAccountAdjustment returns this adjustment as the standard engine
// adjustment view.
func (a AccountAdjustment) EngineAccountAdjustment() AccountAdjustment {
	return a
}

// Handle returns the underlying native handle.
func (a AccountAdjustment) Handle() native.AccountAdjustment {
	return a.value
}

//------------------------------------------------------------------------------
// AccountAdjustmentBalanceOperation

// AccountAdjustmentBalanceOperation holds the balance-operation fields of an adjustment.
type AccountAdjustmentBalanceOperation struct {
	value native.AccountAdjustmentBalanceOperation

	// retainAsset keeps the Asset alive while the C struct's OpenPitStringView
	// points to its C-heap buffer.  See AccountAdjustment for the full
	// explanation of the retain pattern.
	retainAsset param.Asset
}

// AccountAdjustmentBalanceOperationValues holds the optional fields of a balance operation.
type AccountAdjustmentBalanceOperationValues struct {
	Asset             optional.Option[param.Asset]
	AverageEntryPrice optional.Option[param.Price]
	RealizedPnl       optional.Option[param.Pnl]
}

// NewAccountAdjustmentBalanceOperation creates a new zeroed balance operation.
func NewAccountAdjustmentBalanceOperation() AccountAdjustmentBalanceOperation {
	return newAccountAdjustmentBalanceOperation(native.NewAccountAdjustmentBalanceOperation())
}

// NewAccountAdjustmentBalanceOperationFromValues creates a balance operation from the given values.
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

// Reset zeroes out the balance operation.
func (o *AccountAdjustmentBalanceOperation) Reset() {
	native.AccountAdjustmentBalanceOperationReset(&o.value)
	o.retainAsset = param.Asset{}
}

// Values returns a copy of the current balance operation fields.
func (o AccountAdjustmentBalanceOperation) Values() AccountAdjustmentBalanceOperationValues {
	return AccountAdjustmentBalanceOperationValues{
		Asset:             o.Asset(),
		AverageEntryPrice: o.AverageEntryPrice(),
		RealizedPnl:       o.RealizedPnl(),
	}
}

// SetValues resets the balance operation and applies the provided values.
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
	if value, ok := values.RealizedPnl.Get(); ok {
		o.SetRealizedPnl(value)
	}
}

// Asset returns the optional balance-operation asset.
func (o AccountAdjustmentBalanceOperation) Asset() optional.Option[param.Asset] {
	return param.NewAssetFromHandle(native.AccountAdjustmentBalanceOperationGetAsset(o.value))
}

// SetAsset sets the balance-operation asset.
func (o *AccountAdjustmentBalanceOperation) SetAsset(asset param.Asset) {
	native.AccountAdjustmentBalanceOperationSetAsset(&o.value, asset.Handle())
	o.retainAsset = asset
}

// UnsetAsset clears the balance-operation asset.
func (o *AccountAdjustmentBalanceOperation) UnsetAsset() {
	native.AccountAdjustmentBalanceOperationUnsetAsset(&o.value)
	o.retainAsset = param.Asset{}
}

// AverageEntryPrice returns the optional account-currency average entry price
// force-set.
func (o AccountAdjustmentBalanceOperation) AverageEntryPrice() optional.Option[param.Price] {
	return param.NewPriceOptionFromHandle(
		native.AccountAdjustmentBalanceOperationGetAverageEntryPrice(o.value),
	)
}

// SetAverageEntryPrice force-sets the average entry price in account currency.
// The value is caller supplied; no FX is applied.
func (o *AccountAdjustmentBalanceOperation) SetAverageEntryPrice(price param.Price) {
	native.AccountAdjustmentBalanceOperationSetAverageEntryPrice(&o.value, price.Handle())
}

// UnsetAverageEntryPrice clears the average entry price.
func (o *AccountAdjustmentBalanceOperation) UnsetAverageEntryPrice() {
	native.AccountAdjustmentBalanceOperationUnsetAverageEntryPrice(&o.value)
}

// RealizedPnl returns the optional force-set of the slot's absolute realized
// PnL in account currency.
func (o AccountAdjustmentBalanceOperation) RealizedPnl() optional.Option[param.Pnl] {
	return param.NewPnlOptionFromHandle(
		native.AccountAdjustmentBalanceOperationGetRealizedPnl(o.value),
	)
}

// SetRealizedPnl force-sets the slot's absolute realized PnL in account
// currency. The value is caller supplied; no FX is applied.
func (o *AccountAdjustmentBalanceOperation) SetRealizedPnl(value param.Pnl) {
	native.AccountAdjustmentBalanceOperationSetRealizedPnl(&o.value, value.Handle())
}

// UnsetRealizedPnl clears the realized PnL force-set.
func (o *AccountAdjustmentBalanceOperation) UnsetRealizedPnl() {
	native.AccountAdjustmentBalanceOperationUnsetRealizedPnl(&o.value)
}

// AccountAdjustmentBalanceOperationView is a mutable view into a balance operation owned by an AccountAdjustment.
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

// Reset zeroes out the balance operation view.
func (o *AccountAdjustmentBalanceOperationView) Reset() {
	native.AccountAdjustmentBalanceOperationReset(o.ref)
	*o.retainAsset = param.Asset{}
}

// Asset returns the optional balance-operation asset from the view.
func (o AccountAdjustmentBalanceOperationView) Asset() optional.Option[param.Asset] {
	return param.NewAssetFromHandle(native.AccountAdjustmentBalanceOperationGetAsset(*o.ref))
}

// SetAsset sets the balance-operation asset on the view.
func (o *AccountAdjustmentBalanceOperationView) SetAsset(asset param.Asset) {
	native.AccountAdjustmentBalanceOperationSetAsset(o.ref, asset.Handle())
	*o.retainAsset = asset
}

// UnsetAsset clears the balance-operation asset on the view.
func (o *AccountAdjustmentBalanceOperationView) UnsetAsset() {
	native.AccountAdjustmentBalanceOperationUnsetAsset(o.ref)
	*o.retainAsset = param.Asset{}
}

// AverageEntryPrice returns the optional account-currency average entry price
// force-set from the view.
func (o AccountAdjustmentBalanceOperationView) AverageEntryPrice() optional.Option[param.Price] {
	return param.NewPriceOptionFromHandle(
		native.AccountAdjustmentBalanceOperationGetAverageEntryPrice(*o.ref),
	)
}

// SetAverageEntryPrice force-sets the average entry price in account currency
// on the view. The value is caller supplied; no FX is applied.
func (o *AccountAdjustmentBalanceOperationView) SetAverageEntryPrice(price param.Price) {
	native.AccountAdjustmentBalanceOperationSetAverageEntryPrice(o.ref, price.Handle())
}

// UnsetAverageEntryPrice clears the average entry price on the view.
func (o *AccountAdjustmentBalanceOperationView) UnsetAverageEntryPrice() {
	native.AccountAdjustmentBalanceOperationUnsetAverageEntryPrice(o.ref)
}

// RealizedPnl returns the optional account-currency realized PnL force-set from
// the view.
func (o AccountAdjustmentBalanceOperationView) RealizedPnl() optional.Option[param.Pnl] {
	return param.NewPnlOptionFromHandle(
		native.AccountAdjustmentBalanceOperationGetRealizedPnl(*o.ref),
	)
}

// SetRealizedPnl force-sets the slot's absolute realized PnL in account
// currency on the view. The value is caller supplied; no FX is applied.
func (o *AccountAdjustmentBalanceOperationView) SetRealizedPnl(value param.Pnl) {
	native.AccountAdjustmentBalanceOperationSetRealizedPnl(o.ref, value.Handle())
}

// UnsetRealizedPnl clears the realized PnL force-set on the view.
func (o *AccountAdjustmentBalanceOperationView) UnsetRealizedPnl() {
	native.AccountAdjustmentBalanceOperationUnsetRealizedPnl(o.ref)
}

//------------------------------------------------------------------------------
// AccountAdjustmentPositionOperation

// AccountAdjustmentPositionOperation holds the position-operation fields of an adjustment.
type AccountAdjustmentPositionOperation struct {
	value native.AccountAdjustmentPositionOperation

	// retainInstrument and retainCollateralAsset keep the Assets alive while
	// the C struct's OpenPitStringView fields point to their C-heap buffers.
	// See AccountAdjustment for the full explanation of the retain pattern.
	retainInstrument      param.Instrument
	retainCollateralAsset param.Asset
}

// AccountAdjustmentPositionOperationValues holds the optional fields of a position operation.
type AccountAdjustmentPositionOperationValues struct {
	Instrument        optional.Option[param.Instrument]
	CollateralAsset   optional.Option[param.Asset]
	AverageEntryPrice optional.Option[param.Price]
	Leverage          optional.Option[param.Leverage]
	Mode              optional.Option[param.PositionMode]
}

// NewAccountAdjustmentPositionOperation creates a new zeroed position operation.
func NewAccountAdjustmentPositionOperation() AccountAdjustmentPositionOperation {
	return newAccountAdjustmentPositionOperation(native.NewAccountAdjustmentPositionOperation())
}

// NewAccountAdjustmentPositionOperationFromValues creates a position operation from the given values.
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

// Reset zeroes out the position operation.
func (o *AccountAdjustmentPositionOperation) Reset() {
	native.AccountAdjustmentPositionOperationReset(&o.value)
	o.retainInstrument = param.Instrument{}
	o.retainCollateralAsset = param.Asset{}
}

// Values returns a copy of the current position operation fields.
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

// SetValues resets the position operation and applies the provided values.
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

// Instrument returns the optional position-operation instrument.
func (o AccountAdjustmentPositionOperation) Instrument() optional.Option[param.Instrument] {
	return param.NewInstrumentFromHandle(
		native.AccountAdjustmentPositionOperationGetInstrument(o.value),
	)
}

// SetInstrument sets the position-operation instrument.
func (o *AccountAdjustmentPositionOperation) SetInstrument(instrument param.Instrument) {
	native.AccountAdjustmentPositionOperationSetInstrument(&o.value, instrument.Handle())
	o.retainInstrument = instrument
}

// UnsetInstrument clears the position-operation instrument.
func (o *AccountAdjustmentPositionOperation) UnsetInstrument() {
	native.AccountAdjustmentPositionOperationUnsetInstrument(&o.value)
	o.retainInstrument = param.Instrument{}
}

// CollateralAsset returns the optional collateral asset.
func (o AccountAdjustmentPositionOperation) CollateralAsset() optional.Option[param.Asset] {
	return param.NewAssetFromHandle(
		native.AccountAdjustmentPositionOperationGetCollateralAsset(o.value),
	)
}

// SetCollateralAsset sets the collateral asset.
func (o *AccountAdjustmentPositionOperation) SetCollateralAsset(asset param.Asset) {
	native.AccountAdjustmentPositionOperationSetCollateralAsset(&o.value, asset.Handle())
	o.retainCollateralAsset = asset
}

// UnsetCollateralAsset clears the collateral asset.
func (o *AccountAdjustmentPositionOperation) UnsetCollateralAsset() {
	native.AccountAdjustmentPositionOperationUnsetCollateralAsset(&o.value)
	o.retainCollateralAsset = param.Asset{}
}

// AverageEntryPrice returns the optional account-currency average entry price
// force-set for the position operation.
func (o AccountAdjustmentPositionOperation) AverageEntryPrice() optional.Option[param.Price] {
	return param.NewPriceOptionFromHandle(
		native.AccountAdjustmentPositionOperationGetAverageEntryPrice(o.value),
	)
}

// SetAverageEntryPrice force-sets the average entry price in account currency
// for the position operation. The value is caller supplied; no FX is applied.
func (o *AccountAdjustmentPositionOperation) SetAverageEntryPrice(price param.Price) {
	native.AccountAdjustmentPositionOperationSetAverageEntryPrice(&o.value, price.Handle())
}

// UnsetAverageEntryPrice clears the average entry price for the position operation.
func (o *AccountAdjustmentPositionOperation) UnsetAverageEntryPrice() {
	native.AccountAdjustmentPositionOperationUnsetAverageEntryPrice(&o.value)
}

// Leverage returns the optional leverage for the position operation.
func (o AccountAdjustmentPositionOperation) Leverage() optional.Option[param.Leverage] {
	return param.NewLeverageOptionFromHandle(
		native.AccountAdjustmentPositionOperationGetLeverage(o.value),
	)
}

// SetLeverage sets the leverage for the position operation.
func (o *AccountAdjustmentPositionOperation) SetLeverage(leverage param.Leverage) {
	native.AccountAdjustmentPositionOperationSetLeverage(&o.value, leverage.Handle())
}

// UnsetLeverage clears the leverage for the position operation.
func (o *AccountAdjustmentPositionOperation) UnsetLeverage() {
	native.AccountAdjustmentPositionOperationUnsetLeverage(&o.value)
}

// Mode returns the optional position mode for the position operation.
func (o AccountAdjustmentPositionOperation) Mode() optional.Option[param.PositionMode] {
	return param.NewPositionModeFromHandle(native.AccountAdjustmentPositionOperationGetMode(o.value))
}

// SetMode sets the position mode for the position operation.
func (o *AccountAdjustmentPositionOperation) SetMode(mode param.PositionMode) {
	native.AccountAdjustmentPositionOperationSetMode(&o.value, mode.Handle())
}

// UnsetMode clears the position mode for the position operation.
func (o *AccountAdjustmentPositionOperation) UnsetMode() {
	native.AccountAdjustmentPositionOperationUnsetMode(&o.value)
}

// AccountAdjustmentPositionOperationView is a mutable view into a position operation owned by an AccountAdjustment.
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

// Reset zeroes out the position operation view.
func (o *AccountAdjustmentPositionOperationView) Reset() {
	native.AccountAdjustmentPositionOperationReset(o.ref)
	*o.retainInstrument = param.Instrument{}
	*o.retainCollateralAsset = param.Asset{}
}

// Instrument returns the optional instrument from the view.
func (o AccountAdjustmentPositionOperationView) Instrument() optional.Option[param.Instrument] {
	return param.NewInstrumentFromHandle(
		native.AccountAdjustmentPositionOperationGetInstrument(*o.ref),
	)
}

// SetInstrument sets the instrument on the view.
func (o *AccountAdjustmentPositionOperationView) SetInstrument(instrument param.Instrument) {
	native.AccountAdjustmentPositionOperationSetInstrument(o.ref, instrument.Handle())
	*o.retainInstrument = instrument
}

// UnsetInstrument clears the instrument on the view.
func (o *AccountAdjustmentPositionOperationView) UnsetInstrument() {
	native.AccountAdjustmentPositionOperationUnsetInstrument(o.ref)
	*o.retainInstrument = param.Instrument{}
}

// CollateralAsset returns the optional collateral asset from the view.
func (o AccountAdjustmentPositionOperationView) CollateralAsset() optional.Option[param.Asset] {
	return param.NewAssetFromHandle(
		native.AccountAdjustmentPositionOperationGetCollateralAsset(*o.ref),
	)
}

// SetCollateralAsset sets the collateral asset on the view.
func (o *AccountAdjustmentPositionOperationView) SetCollateralAsset(asset param.Asset) {
	native.AccountAdjustmentPositionOperationSetCollateralAsset(o.ref, asset.Handle())
	*o.retainCollateralAsset = asset
}

// UnsetCollateralAsset clears the collateral asset on the view.
func (o *AccountAdjustmentPositionOperationView) UnsetCollateralAsset() {
	native.AccountAdjustmentPositionOperationUnsetCollateralAsset(o.ref)
	*o.retainCollateralAsset = param.Asset{}
}

// AverageEntryPrice returns the optional account-currency average entry price
// force-set from the view.
func (o AccountAdjustmentPositionOperationView) AverageEntryPrice() optional.Option[param.Price] {
	return param.NewPriceOptionFromHandle(
		native.AccountAdjustmentPositionOperationGetAverageEntryPrice(*o.ref),
	)
}

// SetAverageEntryPrice force-sets the average entry price in account currency
// on the view. The value is caller supplied; no FX is applied.
func (o *AccountAdjustmentPositionOperationView) SetAverageEntryPrice(price param.Price) {
	native.AccountAdjustmentPositionOperationSetAverageEntryPrice(o.ref, price.Handle())
}

// UnsetAverageEntryPrice clears the average entry price on the view.
func (o *AccountAdjustmentPositionOperationView) UnsetAverageEntryPrice() {
	native.AccountAdjustmentPositionOperationUnsetAverageEntryPrice(o.ref)
}

// Leverage returns the optional leverage from the view.
func (o AccountAdjustmentPositionOperationView) Leverage() optional.Option[param.Leverage] {
	return param.NewLeverageOptionFromHandle(
		native.AccountAdjustmentPositionOperationGetLeverage(*o.ref),
	)
}

// SetLeverage sets the leverage on the view.
func (o *AccountAdjustmentPositionOperationView) SetLeverage(leverage param.Leverage) {
	native.AccountAdjustmentPositionOperationSetLeverage(o.ref, leverage.Handle())
}

// UnsetLeverage clears the leverage on the view.
func (o *AccountAdjustmentPositionOperationView) UnsetLeverage() {
	native.AccountAdjustmentPositionOperationUnsetLeverage(o.ref)
}

// Mode returns the optional position mode from the view.
func (o AccountAdjustmentPositionOperationView) Mode() optional.Option[param.PositionMode] {
	return param.NewPositionModeFromHandle(native.AccountAdjustmentPositionOperationGetMode(*o.ref))
}

// SetMode sets the position mode on the view.
func (o *AccountAdjustmentPositionOperationView) SetMode(mode param.PositionMode) {
	native.AccountAdjustmentPositionOperationSetMode(o.ref, mode.Handle())
}

// UnsetMode clears the position mode on the view.
func (o *AccountAdjustmentPositionOperationView) UnsetMode() {
	native.AccountAdjustmentPositionOperationUnsetMode(o.ref)
}

//------------------------------------------------------------------------------
// AccountAdjustmentAmount

// AccountAdjustmentAmount holds the balance/held/incoming amount adjustments.
type AccountAdjustmentAmount struct {
	value native.AccountAdjustmentAmount
}

// NewAccountAdjustmentAmount creates a new zeroed AccountAdjustmentAmount.
func NewAccountAdjustmentAmount() AccountAdjustmentAmount {
	return newAccountAdjustmentAmount(native.NewAccountAdjustmentAmount())
}

// AccountAdjustmentAmountValues holds the optional amount fields of an adjustment.
type AccountAdjustmentAmountValues struct {
	Balance  optional.Option[param.AdjustmentAmount]
	Held     optional.Option[param.AdjustmentAmount]
	Incoming optional.Option[param.AdjustmentAmount]
}

// NewAccountAdjustmentAmountFromValues creates an AccountAdjustmentAmount from the given values.
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

// Reset zeroes out the amount.
func (a *AccountAdjustmentAmount) Reset() {
	native.AccountAdjustmentAmountReset(&a.value)
}

// SetValues resets the amount and applies the provided values.
func (a *AccountAdjustmentAmount) SetValues(values AccountAdjustmentAmountValues) {
	a.Reset()
	a.setValues(values)
}

func (a *AccountAdjustmentAmount) setValues(values AccountAdjustmentAmountValues) {
	if value, ok := values.Balance.Get(); ok {
		a.SetBalance(value)
	}
	if value, ok := values.Held.Get(); ok {
		a.SetHeld(value)
	}
	if value, ok := values.Incoming.Get(); ok {
		a.SetIncoming(value)
	}
}

// Values returns a copy of the current amount fields.
func (a AccountAdjustmentAmount) Values() AccountAdjustmentAmountValues {
	return AccountAdjustmentAmountValues{
		Balance:  a.Balance(),
		Held:     a.Held(),
		Incoming: a.Incoming(),
	}
}

// Balance returns the optional balance adjustment amount.
func (a AccountAdjustmentAmount) Balance() optional.Option[param.AdjustmentAmount] {
	return param.NewAdjustmentAmountFromHandle(native.AccountAdjustmentAmountGetBalance(a.value))
}

// SetBalance sets the balance adjustment amount.
func (a *AccountAdjustmentAmount) SetBalance(value param.AdjustmentAmount) {
	native.AccountAdjustmentAmountSetBalance(&a.value, value.Handle())
}

// UnsetBalance clears the balance adjustment amount.
func (a *AccountAdjustmentAmount) UnsetBalance() {
	native.AccountAdjustmentAmountUnsetBalance(&a.value)
}

// Held returns the optional held adjustment amount.
func (a AccountAdjustmentAmount) Held() optional.Option[param.AdjustmentAmount] {
	return param.NewAdjustmentAmountFromHandle(native.AccountAdjustmentAmountGetHeld(a.value))
}

// SetHeld sets the held adjustment amount.
func (a *AccountAdjustmentAmount) SetHeld(value param.AdjustmentAmount) {
	native.AccountAdjustmentAmountSetHeld(&a.value, value.Handle())
}

// UnsetHeld clears the held adjustment amount.
func (a *AccountAdjustmentAmount) UnsetHeld() {
	native.AccountAdjustmentAmountUnsetHeld(&a.value)
}

// Incoming returns the optional incoming adjustment amount.
func (a AccountAdjustmentAmount) Incoming() optional.Option[param.AdjustmentAmount] {
	return param.NewAdjustmentAmountFromHandle(native.AccountAdjustmentAmountGetIncoming(a.value))
}

// SetIncoming sets the incoming adjustment amount.
func (a *AccountAdjustmentAmount) SetIncoming(value param.AdjustmentAmount) {
	native.AccountAdjustmentAmountSetIncoming(&a.value, value.Handle())
}

// UnsetIncoming clears the incoming adjustment amount.
func (a *AccountAdjustmentAmount) UnsetIncoming() {
	native.AccountAdjustmentAmountUnsetIncoming(&a.value)
}

// AccountAdjustmentAmountView is a mutable view into an amount owned by an AccountAdjustment.
type AccountAdjustmentAmountView struct {
	ref *native.AccountAdjustmentAmount
}

func newAccountAdjustmentAmountView(
	ref *native.AccountAdjustmentAmount,
) AccountAdjustmentAmountView {
	return AccountAdjustmentAmountView{ref: ref}
}

// Reset zeroes out the amount view.
func (a *AccountAdjustmentAmountView) Reset() {
	native.AccountAdjustmentAmountReset(a.ref)
}

// Balance returns the optional balance adjustment amount from the view.
func (a AccountAdjustmentAmountView) Balance() optional.Option[param.AdjustmentAmount] {
	return param.NewAdjustmentAmountFromHandle(native.AccountAdjustmentAmountGetBalance(*a.ref))
}

// SetBalance sets the balance adjustment amount on the view.
func (a *AccountAdjustmentAmountView) SetBalance(value param.AdjustmentAmount) {
	native.AccountAdjustmentAmountSetBalance(a.ref, value.Handle())
}

// UnsetBalance clears the balance adjustment amount on the view.
func (a *AccountAdjustmentAmountView) UnsetBalance() {
	native.AccountAdjustmentAmountUnsetBalance(a.ref)
}

// Held returns the optional held adjustment amount from the view.
func (a AccountAdjustmentAmountView) Held() optional.Option[param.AdjustmentAmount] {
	return param.NewAdjustmentAmountFromHandle(native.AccountAdjustmentAmountGetHeld(*a.ref))
}

// SetHeld sets the held adjustment amount on the view.
func (a *AccountAdjustmentAmountView) SetHeld(value param.AdjustmentAmount) {
	native.AccountAdjustmentAmountSetHeld(a.ref, value.Handle())
}

// UnsetHeld clears the held adjustment amount on the view.
func (a *AccountAdjustmentAmountView) UnsetHeld() {
	native.AccountAdjustmentAmountUnsetHeld(a.ref)
}

// Incoming returns the optional incoming adjustment amount from the view.
func (a AccountAdjustmentAmountView) Incoming() optional.Option[param.AdjustmentAmount] {
	return param.NewAdjustmentAmountFromHandle(native.AccountAdjustmentAmountGetIncoming(*a.ref))
}

// SetIncoming sets the incoming adjustment amount on the view.
func (a *AccountAdjustmentAmountView) SetIncoming(value param.AdjustmentAmount) {
	native.AccountAdjustmentAmountSetIncoming(a.ref, value.Handle())
}

// UnsetIncoming clears the incoming adjustment amount on the view.
func (a *AccountAdjustmentAmountView) UnsetIncoming() {
	native.AccountAdjustmentAmountUnsetIncoming(a.ref)
}

//------------------------------------------------------------------------------
// AccountAdjustmentBounds

// AccountAdjustmentBounds holds the upper and lower bounds for balance/held/incoming adjustments.
type AccountAdjustmentBounds struct {
	value native.AccountAdjustmentBounds
}

// NewAccountAdjustmentBounds creates a new zeroed AccountAdjustmentBounds.
func NewAccountAdjustmentBounds() AccountAdjustmentBounds {
	return newAccountAdjustmentBounds(native.NewAccountAdjustmentBounds())
}

// AccountAdjustmentBoundsValues holds the optional bound fields for an adjustment.
type AccountAdjustmentBoundsValues struct {
	BalanceUpper  optional.Option[param.PositionSize]
	BalanceLower  optional.Option[param.PositionSize]
	HeldUpper     optional.Option[param.PositionSize]
	HeldLower     optional.Option[param.PositionSize]
	IncomingUpper optional.Option[param.PositionSize]
	IncomingLower optional.Option[param.PositionSize]
}

// NewAccountAdjustmentBoundsFromValues creates AccountAdjustmentBounds from the given values.
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

// Reset zeroes out the bounds.
func (b *AccountAdjustmentBounds) Reset() {
	native.AccountAdjustmentBoundsReset(&b.value)
}

// Values returns a copy of the current bounds fields.
func (b AccountAdjustmentBounds) Values() AccountAdjustmentBoundsValues {
	return AccountAdjustmentBoundsValues{
		BalanceUpper:  b.BalanceUpper(),
		BalanceLower:  b.BalanceLower(),
		HeldUpper:     b.HeldUpper(),
		HeldLower:     b.HeldLower(),
		IncomingUpper: b.IncomingUpper(),
		IncomingLower: b.IncomingLower(),
	}
}

// SetValues resets the bounds and applies the provided values.
func (b *AccountAdjustmentBounds) SetValues(values AccountAdjustmentBoundsValues) {
	b.Reset()
	b.setValues(values)
}

func (b AccountAdjustmentBounds) setValues(values AccountAdjustmentBoundsValues) {
	if value, ok := values.BalanceUpper.Get(); ok {
		b.SetBalanceUpper(value)
	}
	if value, ok := values.BalanceLower.Get(); ok {
		b.SetBalanceLower(value)
	}
	if value, ok := values.HeldUpper.Get(); ok {
		b.SetHeldUpper(value)
	}
	if value, ok := values.HeldLower.Get(); ok {
		b.SetHeldLower(value)
	}
	if value, ok := values.IncomingUpper.Get(); ok {
		b.SetIncomingUpper(value)
	}
	if value, ok := values.IncomingLower.Get(); ok {
		b.SetIncomingLower(value)
	}
}

// BalanceUpper returns the optional balance upper bound.
func (b AccountAdjustmentBounds) BalanceUpper() optional.Option[param.PositionSize] {
	return param.NewPositionSizeOptionFromHandle(
		native.AccountAdjustmentBoundsGetBalanceUpper(b.value),
	)
}

// SetBalanceUpper sets the balance upper bound.
func (b *AccountAdjustmentBounds) SetBalanceUpper(bound param.PositionSize) {
	native.AccountAdjustmentBoundsSetBalanceUpper(&b.value, bound.Handle())
}

// UnsetBalanceUpper clears the balance upper bound.
func (b *AccountAdjustmentBounds) UnsetBalanceUpper() {
	native.AccountAdjustmentBoundsUnsetBalanceUpper(&b.value)
}

// BalanceLower returns the optional balance lower bound.
func (b AccountAdjustmentBounds) BalanceLower() optional.Option[param.PositionSize] {
	return param.NewPositionSizeOptionFromHandle(
		native.AccountAdjustmentBoundsGetBalanceLower(b.value),
	)
}

// SetBalanceLower sets the balance lower bound.
func (b *AccountAdjustmentBounds) SetBalanceLower(bound param.PositionSize) {
	native.AccountAdjustmentBoundsSetBalanceLower(&b.value, bound.Handle())
}

// UnsetBalanceLower clears the balance lower bound.
func (b *AccountAdjustmentBounds) UnsetBalanceLower() {
	native.AccountAdjustmentBoundsUnsetBalanceLower(&b.value)
}

// HeldUpper returns the optional held upper bound.
func (b AccountAdjustmentBounds) HeldUpper() optional.Option[param.PositionSize] {
	return param.NewPositionSizeOptionFromHandle(
		native.AccountAdjustmentBoundsGetHeldUpper(b.value),
	)
}

// SetHeldUpper sets the held upper bound.
func (b *AccountAdjustmentBounds) SetHeldUpper(bound param.PositionSize) {
	native.AccountAdjustmentBoundsSetHeldUpper(&b.value, bound.Handle())
}

// UnsetHeldUpper clears the held upper bound.
func (b *AccountAdjustmentBounds) UnsetHeldUpper() {
	native.AccountAdjustmentBoundsUnsetHeldUpper(&b.value)
}

// HeldLower returns the optional held lower bound.
func (b AccountAdjustmentBounds) HeldLower() optional.Option[param.PositionSize] {
	return param.NewPositionSizeOptionFromHandle(
		native.AccountAdjustmentBoundsGetHeldLower(b.value),
	)
}

// SetHeldLower sets the held lower bound.
func (b *AccountAdjustmentBounds) SetHeldLower(bound param.PositionSize) {
	native.AccountAdjustmentBoundsSetHeldLower(&b.value, bound.Handle())
}

// UnsetHeldLower clears the held lower bound.
func (b *AccountAdjustmentBounds) UnsetHeldLower() {
	native.AccountAdjustmentBoundsUnsetHeldLower(&b.value)
}

// IncomingUpper returns the optional incoming upper bound.
func (b AccountAdjustmentBounds) IncomingUpper() optional.Option[param.PositionSize] {
	return param.NewPositionSizeOptionFromHandle(
		native.AccountAdjustmentBoundsGetIncomingUpper(b.value),
	)
}

// SetIncomingUpper sets the incoming upper bound.
func (b *AccountAdjustmentBounds) SetIncomingUpper(bound param.PositionSize) {
	native.AccountAdjustmentBoundsSetIncomingUpper(&b.value, bound.Handle())
}

// UnsetIncomingUpper clears the incoming upper bound.
func (b *AccountAdjustmentBounds) UnsetIncomingUpper() {
	native.AccountAdjustmentBoundsUnsetIncomingUpper(&b.value)
}

// IncomingLower returns the optional incoming lower bound.
func (b AccountAdjustmentBounds) IncomingLower() optional.Option[param.PositionSize] {
	return param.NewPositionSizeOptionFromHandle(
		native.AccountAdjustmentBoundsGetIncomingLower(b.value),
	)
}

// SetIncomingLower sets the incoming lower bound.
func (b *AccountAdjustmentBounds) SetIncomingLower(bound param.PositionSize) {
	native.AccountAdjustmentBoundsSetIncomingLower(&b.value, bound.Handle())
}

// UnsetIncomingLower clears the incoming lower bound.
func (b *AccountAdjustmentBounds) UnsetIncomingLower() {
	native.AccountAdjustmentBoundsUnsetIncomingLower(&b.value)
}

// AccountAdjustmentBoundsView is a mutable view into bounds owned by an AccountAdjustment.
type AccountAdjustmentBoundsView struct {
	ref *native.AccountAdjustmentBounds
}

func newAccountAdjustmentBoundsView(
	ref *native.AccountAdjustmentBounds,
) AccountAdjustmentBoundsView {
	return AccountAdjustmentBoundsView{ref: ref}
}

// Reset zeroes out the bounds view.
func (b *AccountAdjustmentBoundsView) Reset() {
	native.AccountAdjustmentBoundsReset(b.ref)
}

// BalanceUpper returns the optional balance upper bound from the view.
func (b AccountAdjustmentBoundsView) BalanceUpper() optional.Option[param.PositionSize] {
	return param.NewPositionSizeOptionFromHandle(
		native.AccountAdjustmentBoundsGetBalanceUpper(*b.ref),
	)
}

// SetBalanceUpper sets the balance upper bound on the view.
func (b *AccountAdjustmentBoundsView) SetBalanceUpper(bound param.PositionSize) {
	native.AccountAdjustmentBoundsSetBalanceUpper(b.ref, bound.Handle())
}

// UnsetBalanceUpper clears the balance upper bound on the view.
func (b *AccountAdjustmentBoundsView) UnsetBalanceUpper() {
	native.AccountAdjustmentBoundsUnsetBalanceUpper(b.ref)
}

// BalanceLower returns the optional balance lower bound from the view.
func (b AccountAdjustmentBoundsView) BalanceLower() optional.Option[param.PositionSize] {
	return param.NewPositionSizeOptionFromHandle(
		native.AccountAdjustmentBoundsGetBalanceLower(*b.ref),
	)
}

// SetBalanceLower sets the balance lower bound on the view.
func (b *AccountAdjustmentBoundsView) SetBalanceLower(bound param.PositionSize) {
	native.AccountAdjustmentBoundsSetBalanceLower(b.ref, bound.Handle())
}

// UnsetBalanceLower clears the balance lower bound on the view.
func (b *AccountAdjustmentBoundsView) UnsetBalanceLower() {
	native.AccountAdjustmentBoundsUnsetBalanceLower(b.ref)
}

// HeldUpper returns the optional held upper bound from the view.
func (b AccountAdjustmentBoundsView) HeldUpper() optional.Option[param.PositionSize] {
	return param.NewPositionSizeOptionFromHandle(
		native.AccountAdjustmentBoundsGetHeldUpper(*b.ref),
	)
}

// SetHeldUpper sets the held upper bound on the view.
func (b *AccountAdjustmentBoundsView) SetHeldUpper(bound param.PositionSize) {
	native.AccountAdjustmentBoundsSetHeldUpper(b.ref, bound.Handle())
}

// UnsetHeldUpper clears the held upper bound on the view.
func (b *AccountAdjustmentBoundsView) UnsetHeldUpper() {
	native.AccountAdjustmentBoundsUnsetHeldUpper(b.ref)
}

// HeldLower returns the optional held lower bound from the view.
func (b AccountAdjustmentBoundsView) HeldLower() optional.Option[param.PositionSize] {
	return param.NewPositionSizeOptionFromHandle(
		native.AccountAdjustmentBoundsGetHeldLower(*b.ref),
	)
}

// SetHeldLower sets the held lower bound on the view.
func (b *AccountAdjustmentBoundsView) SetHeldLower(bound param.PositionSize) {
	native.AccountAdjustmentBoundsSetHeldLower(b.ref, bound.Handle())
}

// UnsetHeldLower clears the held lower bound on the view.
func (b *AccountAdjustmentBoundsView) UnsetHeldLower() {
	native.AccountAdjustmentBoundsUnsetHeldLower(b.ref)
}

// IncomingUpper returns the optional incoming upper bound from the view.
func (b AccountAdjustmentBoundsView) IncomingUpper() optional.Option[param.PositionSize] {
	return param.NewPositionSizeOptionFromHandle(
		native.AccountAdjustmentBoundsGetIncomingUpper(*b.ref),
	)
}

// SetIncomingUpper sets the incoming upper bound on the view.
func (b *AccountAdjustmentBoundsView) SetIncomingUpper(bound param.PositionSize) {
	native.AccountAdjustmentBoundsSetIncomingUpper(b.ref, bound.Handle())
}

// UnsetIncomingUpper clears the incoming upper bound on the view.
func (b *AccountAdjustmentBoundsView) UnsetIncomingUpper() {
	native.AccountAdjustmentBoundsUnsetIncomingUpper(b.ref)
}

// IncomingLower returns the optional incoming lower bound from the view.
func (b AccountAdjustmentBoundsView) IncomingLower() optional.Option[param.PositionSize] {
	return param.NewPositionSizeOptionFromHandle(
		native.AccountAdjustmentBoundsGetIncomingLower(*b.ref),
	)
}

// SetIncomingLower sets the incoming lower bound on the view.
func (b *AccountAdjustmentBoundsView) SetIncomingLower(bound param.PositionSize) {
	native.AccountAdjustmentBoundsSetIncomingLower(b.ref, bound.Handle())
}

// UnsetIncomingLower clears the incoming lower bound on the view.
func (b *AccountAdjustmentBoundsView) UnsetIncomingLower() {
	native.AccountAdjustmentBoundsUnsetIncomingLower(b.ref)
}

//------------------------------------------------------------------------------
