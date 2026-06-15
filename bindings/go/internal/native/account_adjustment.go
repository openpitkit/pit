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
#include "openpit.h"
*/
import "C"
import "unsafe"

//------------------------------------------------------------------------------
// AccountAdjustment

func NewAccountAdjustment() AccountAdjustment {
	return AccountAdjustment{}
}

func AccountAdjustmentReset(adjustment *AccountAdjustment) {
	*adjustment = NewAccountAdjustment()
}

func AccountAdjustmentGetOperationKind(
	adjustment AccountAdjustment,
) AccountAdjustmentOperationKind {
	return adjustment.operation.kind
}

func AccountAdjustmentGetBalanceOperation(
	adjustment AccountAdjustment,
) AccountAdjustmentBalanceOperation {
	return adjustment.operation.balance
}

func AccountAdjustmentGetBalanceOperationView(
	adjustment *AccountAdjustment,
) *AccountAdjustmentBalanceOperation {
	return &adjustment.operation.balance
}

func AccountAdjustmentSetBalanceOperationAndUnsetPositionOperation(
	adjustment *AccountAdjustment,
	operation AccountAdjustmentBalanceOperation,
) {
	adjustment.operation.kind = AccountAdjustmentOperationKindBalance
	adjustment.operation.balance = operation
	AccountAdjustmentPositionOperationReset(&adjustment.operation.position)
}

func AccountAdjustmentSelectBalanceOperationAndUnsetPositionOperation(
	adjustment *AccountAdjustment,
) {
	if adjustment.operation.kind != AccountAdjustmentOperationKindBalance {
		AccountAdjustmentBalanceOperationReset(&adjustment.operation.balance)
	}
	adjustment.operation.kind = AccountAdjustmentOperationKindBalance
	AccountAdjustmentPositionOperationReset(&adjustment.operation.position)
}

func AccountAdjustmentUnsetBalanceOperation(adjustment *AccountAdjustment) {
	if adjustment.operation.kind == AccountAdjustmentOperationKindBalance {
		adjustment.operation.kind = AccountAdjustmentOperationKindAbsent
	}
	AccountAdjustmentBalanceOperationReset(&adjustment.operation.balance)
}

func AccountAdjustmentGetPositionOperation(
	adjustment AccountAdjustment,
) AccountAdjustmentPositionOperation {
	return adjustment.operation.position
}

func AccountAdjustmentGetPositionOperationView(
	adjustment *AccountAdjustment,
) *AccountAdjustmentPositionOperation {
	return &adjustment.operation.position
}

func AccountAdjustmentSetPositionOperationAndUnsetBalanceOperation(
	adjustment *AccountAdjustment,
	operation AccountAdjustmentPositionOperation,
) {
	adjustment.operation.kind = AccountAdjustmentOperationKindPosition
	adjustment.operation.position = operation
	AccountAdjustmentBalanceOperationReset(&adjustment.operation.balance)
}

func AccountAdjustmentSelectPositionOperationAndUnsetBalanceOperation(
	adjustment *AccountAdjustment,
) {
	if adjustment.operation.kind != AccountAdjustmentOperationKindPosition {
		AccountAdjustmentPositionOperationReset(&adjustment.operation.position)
	}
	adjustment.operation.kind = AccountAdjustmentOperationKindPosition
	AccountAdjustmentBalanceOperationReset(&adjustment.operation.balance)
}

func AccountAdjustmentUnsetPositionOperation(adjustment *AccountAdjustment) {
	if adjustment.operation.kind == AccountAdjustmentOperationKindPosition {
		adjustment.operation.kind = AccountAdjustmentOperationKindAbsent
	}
	AccountAdjustmentPositionOperationReset(&adjustment.operation.position)
}

func AccountAdjustmentGetAmount(adjustment AccountAdjustment) AccountAdjustmentAmountOptional {
	return adjustment.amount
}

func AccountAdjustmentGetAmountView(
	adjustment *AccountAdjustment,
) *AccountAdjustmentAmountOptional {
	return &adjustment.amount
}

func AccountAdjustmentSetAmount(adjustment *AccountAdjustment, amount AccountAdjustmentAmount) {
	AccountAdjustmentAmountOptionalSet(&adjustment.amount, amount)
}

func AccountAdjustmentUnsetAmount(adjustment *AccountAdjustment) {
	AccountAdjustmentAmountOptionalReset(&adjustment.amount)
}

func AccountAdjustmentGetBounds(adjustment AccountAdjustment) AccountAdjustmentBoundsOptional {
	return adjustment.bounds
}

func AccountAdjustmentGetBoundsView(
	adjustment *AccountAdjustment,
) *AccountAdjustmentBoundsOptional {
	return &adjustment.bounds
}

func AccountAdjustmentSetBounds(adjustment *AccountAdjustment, bounds AccountAdjustmentBounds) {
	AccountAdjustmentBoundsOptionalSet(&adjustment.bounds, bounds)
}

func AccountAdjustmentUnsetBounds(adjustment *AccountAdjustment) {
	AccountAdjustmentBoundsOptionalReset(&adjustment.bounds)
}

func AccountAdjustmentGetUserData(adjustment AccountAdjustment) unsafe.Pointer {
	return adjustment.user_data
}

func AccountAdjustmentSetUserData(adjustment *AccountAdjustment, userData unsafe.Pointer) {
	adjustment.user_data = userData
}

//------------------------------------------------------------------------------
// AccountAdjustmentAmountOptional

func NewAccountAdjustmentAmountOptional() AccountAdjustmentAmountOptional {
	return AccountAdjustmentAmountOptional{}
}

func AccountAdjustmentAmountOptionalReset(value *AccountAdjustmentAmountOptional) {
	AccountAdjustmentAmountReset(&value.value)
	value.is_set = false
}

func AccountAdjustmentAmountOptionalIsSet(value AccountAdjustmentAmountOptional) bool {
	return bool(value.is_set)
}

func AccountAdjustmentAmountOptionalGet(value AccountAdjustmentAmountOptional) AccountAdjustmentAmount {
	return value.value
}

func AccountAdjustmentAmountOptionalGetView(
	value *AccountAdjustmentAmountOptional,
) *AccountAdjustmentAmount {
	return &value.value
}

func AccountAdjustmentAmountOptionalSet(
	value *AccountAdjustmentAmountOptional,
	amount AccountAdjustmentAmount,
) {
	value.value = amount
	value.is_set = true
}

//------------------------------------------------------------------------------
// AccountAdjustmentBoundsOptional

func NewAccountAdjustmentBoundsOptional() AccountAdjustmentBoundsOptional {
	return AccountAdjustmentBoundsOptional{}
}

func AccountAdjustmentBoundsOptionalReset(value *AccountAdjustmentBoundsOptional) {
	AccountAdjustmentBoundsReset(&value.value)
	value.is_set = false
}

func AccountAdjustmentBoundsOptionalIsSet(value AccountAdjustmentBoundsOptional) bool {
	return bool(value.is_set)
}

func AccountAdjustmentBoundsOptionalGet(value AccountAdjustmentBoundsOptional) AccountAdjustmentBounds {
	return value.value
}

func AccountAdjustmentBoundsOptionalGetView(
	value *AccountAdjustmentBoundsOptional,
) *AccountAdjustmentBounds {
	return &value.value
}

func AccountAdjustmentBoundsOptionalSet(
	value *AccountAdjustmentBoundsOptional,
	bounds AccountAdjustmentBounds,
) {
	value.value = bounds
	value.is_set = true
}

//------------------------------------------------------------------------------
// AccountAdjustmentBalanceOperation

func NewAccountAdjustmentBalanceOperation() AccountAdjustmentBalanceOperation {
	return AccountAdjustmentBalanceOperation{}
}

func AccountAdjustmentBalanceOperationReset(operation *AccountAdjustmentBalanceOperation) {
	*operation = NewAccountAdjustmentBalanceOperation()
}

func AccountAdjustmentBalanceOperationGetAsset(
	operation AccountAdjustmentBalanceOperation,
) StringView {
	return newStringView(operation.asset)
}

func AccountAdjustmentBalanceOperationSetAsset(
	operation *AccountAdjustmentBalanceOperation,
	asset string,
) {
	operation.asset = importString(asset)
}

func AccountAdjustmentBalanceOperationUnsetAsset(operation *AccountAdjustmentBalanceOperation) {
	operation.asset = stringViewNone.value
}

func AccountAdjustmentBalanceOperationGetAverageEntryPrice(
	operation AccountAdjustmentBalanceOperation,
) ParamPriceOptional {
	return operation.average_entry_price
}

func AccountAdjustmentBalanceOperationSetAverageEntryPrice(
	operation *AccountAdjustmentBalanceOperation,
	price ParamPrice,
) {
	operation.average_entry_price.value = price
	operation.average_entry_price.is_set = true
}

func AccountAdjustmentBalanceOperationUnsetAverageEntryPrice(
	operation *AccountAdjustmentBalanceOperation,
) {
	operation.average_entry_price = ParamPriceOptional{}
}

func AccountAdjustmentBalanceOperationGetRealizedPnl(
	operation AccountAdjustmentBalanceOperation,
) ParamPnlOptional {
	return operation.realized_pnl
}

func AccountAdjustmentBalanceOperationSetRealizedPnl(
	operation *AccountAdjustmentBalanceOperation,
	value ParamPnl,
) {
	operation.realized_pnl.value = value
	operation.realized_pnl.is_set = true
}

func AccountAdjustmentBalanceOperationUnsetRealizedPnl(
	operation *AccountAdjustmentBalanceOperation,
) {
	operation.realized_pnl = ParamPnlOptional{}
}

//------------------------------------------------------------------------------
// AccountAdjustmentPositionOperation

func NewAccountAdjustmentPositionOperation() AccountAdjustmentPositionOperation {
	return AccountAdjustmentPositionOperation{}
}

func AccountAdjustmentPositionOperationReset(operation *AccountAdjustmentPositionOperation) {
	*operation = NewAccountAdjustmentPositionOperation()
}

func AccountAdjustmentPositionOperationGetInstrument(
	operation AccountAdjustmentPositionOperation,
) Instrument {
	return operation.instrument
}

func AccountAdjustmentPositionOperationSetInstrument(
	operation *AccountAdjustmentPositionOperation,
	instrument Instrument,
) {
	operation.instrument = instrument
}

func AccountAdjustmentPositionOperationUnsetInstrument(
	operation *AccountAdjustmentPositionOperation,
) {
	operation.instrument = Instrument{}
}

func AccountAdjustmentPositionOperationGetCollateralAsset(
	operation AccountAdjustmentPositionOperation,
) StringView {
	return newStringView(operation.collateral_asset)
}

func AccountAdjustmentPositionOperationSetCollateralAsset(
	operation *AccountAdjustmentPositionOperation,
	asset string,
) {
	operation.collateral_asset = importString(asset)
}

func AccountAdjustmentPositionOperationUnsetCollateralAsset(
	operation *AccountAdjustmentPositionOperation,
) {
	operation.collateral_asset = stringViewNone.value
}

func AccountAdjustmentPositionOperationGetAverageEntryPrice(
	operation AccountAdjustmentPositionOperation,
) ParamPriceOptional {
	return operation.average_entry_price
}

func AccountAdjustmentPositionOperationSetAverageEntryPrice(
	operation *AccountAdjustmentPositionOperation,
	price ParamPrice,
) {
	operation.average_entry_price.value = price
	operation.average_entry_price.is_set = true
}

func AccountAdjustmentPositionOperationUnsetAverageEntryPrice(
	operation *AccountAdjustmentPositionOperation,
) {
	operation.average_entry_price = ParamPriceOptional{}
}

func AccountAdjustmentPositionOperationGetLeverage(
	operation AccountAdjustmentPositionOperation,
) ParamLeverage {
	return operation.leverage
}

func AccountAdjustmentPositionOperationSetLeverage(
	operation *AccountAdjustmentPositionOperation,
	leverage ParamLeverage,
) {
	operation.leverage = leverage
}

func AccountAdjustmentPositionOperationUnsetLeverage(
	operation *AccountAdjustmentPositionOperation,
) {
	operation.leverage = ParamLeverageNotSet
}

func AccountAdjustmentPositionOperationGetMode(
	operation AccountAdjustmentPositionOperation,
) ParamPositionMode {
	return operation.mode
}

func AccountAdjustmentPositionOperationSetMode(
	operation *AccountAdjustmentPositionOperation,
	mode ParamPositionMode,
) {
	operation.mode = mode
}

func AccountAdjustmentPositionOperationUnsetMode(operation *AccountAdjustmentPositionOperation) {
	operation.mode = ParamPositionModeNotSet
}

//------------------------------------------------------------------------------
// AccountAdjustmentAmount

func NewAccountAdjustmentAmount() AccountAdjustmentAmount {
	return AccountAdjustmentAmount{}
}

func AccountAdjustmentAmountReset(amount *AccountAdjustmentAmount) {
	*amount = NewAccountAdjustmentAmount()
}

func AccountAdjustmentAmountGetBalance(amount AccountAdjustmentAmount) ParamAdjustmentAmount {
	return amount.balance
}

func AccountAdjustmentAmountSetBalance(amount *AccountAdjustmentAmount, value ParamAdjustmentAmount) {
	amount.balance = value
}

func AccountAdjustmentAmountUnsetBalance(amount *AccountAdjustmentAmount) {
	amount.balance = ParamAdjustmentAmount{}
}

func AccountAdjustmentAmountGetHeld(amount AccountAdjustmentAmount) ParamAdjustmentAmount {
	return amount.held
}

func AccountAdjustmentAmountSetHeld(
	amount *AccountAdjustmentAmount,
	value ParamAdjustmentAmount,
) {
	amount.held = value
}

func AccountAdjustmentAmountUnsetHeld(amount *AccountAdjustmentAmount) {
	amount.held = ParamAdjustmentAmount{}
}

func AccountAdjustmentAmountGetIncoming(amount AccountAdjustmentAmount) ParamAdjustmentAmount {
	return amount.incoming
}

func AccountAdjustmentAmountSetIncoming(
	amount *AccountAdjustmentAmount,
	value ParamAdjustmentAmount,
) {
	amount.incoming = value
}

func AccountAdjustmentAmountUnsetIncoming(amount *AccountAdjustmentAmount) {
	amount.incoming = ParamAdjustmentAmount{}
}

//------------------------------------------------------------------------------
// AccountAdjustmentBounds

func NewAccountAdjustmentBounds() AccountAdjustmentBounds {
	return AccountAdjustmentBounds{}
}

func AccountAdjustmentBoundsReset(bounds *AccountAdjustmentBounds) {
	*bounds = NewAccountAdjustmentBounds()
}

func AccountAdjustmentBoundsGetBalanceUpper(
	bounds AccountAdjustmentBounds,
) ParamPositionSizeOptional {
	return bounds.balance_upper
}

func AccountAdjustmentBoundsSetBalanceUpper(
	bounds *AccountAdjustmentBounds,
	bound ParamPositionSize,
) {
	bounds.balance_upper.value = bound
	bounds.balance_upper.is_set = true
}

func AccountAdjustmentBoundsUnsetBalanceUpper(bounds *AccountAdjustmentBounds) {
	bounds.balance_upper = ParamPositionSizeOptional{}
}

func AccountAdjustmentBoundsGetBalanceLower(
	bounds AccountAdjustmentBounds,
) ParamPositionSizeOptional {
	return bounds.balance_lower
}

func AccountAdjustmentBoundsSetBalanceLower(
	bounds *AccountAdjustmentBounds,
	bound ParamPositionSize,
) {
	bounds.balance_lower.value = bound
	bounds.balance_lower.is_set = true
}

func AccountAdjustmentBoundsUnsetBalanceLower(bounds *AccountAdjustmentBounds) {
	bounds.balance_lower = ParamPositionSizeOptional{}
}

func AccountAdjustmentBoundsGetHeldUpper(
	bounds AccountAdjustmentBounds,
) ParamPositionSizeOptional {
	return bounds.held_upper
}

func AccountAdjustmentBoundsSetHeldUpper(
	bounds *AccountAdjustmentBounds,
	bound ParamPositionSize,
) {
	bounds.held_upper.value = bound
	bounds.held_upper.is_set = true
}

func AccountAdjustmentBoundsUnsetHeldUpper(bounds *AccountAdjustmentBounds) {
	bounds.held_upper = ParamPositionSizeOptional{}
}

func AccountAdjustmentBoundsGetHeldLower(
	bounds AccountAdjustmentBounds,
) ParamPositionSizeOptional {
	return bounds.held_lower
}

func AccountAdjustmentBoundsSetHeldLower(
	bounds *AccountAdjustmentBounds,
	bound ParamPositionSize,
) {
	bounds.held_lower.value = bound
	bounds.held_lower.is_set = true
}

func AccountAdjustmentBoundsUnsetHeldLower(bounds *AccountAdjustmentBounds) {
	bounds.held_lower = ParamPositionSizeOptional{}
}

func AccountAdjustmentBoundsGetIncomingUpper(
	bounds AccountAdjustmentBounds,
) ParamPositionSizeOptional {
	return bounds.incoming_upper
}

func AccountAdjustmentBoundsSetIncomingUpper(
	bounds *AccountAdjustmentBounds,
	bound ParamPositionSize,
) {
	bounds.incoming_upper.value = bound
	bounds.incoming_upper.is_set = true
}

func AccountAdjustmentBoundsUnsetIncomingUpper(bounds *AccountAdjustmentBounds) {
	bounds.incoming_upper = ParamPositionSizeOptional{}
}

func AccountAdjustmentBoundsGetIncomingLower(
	bounds AccountAdjustmentBounds,
) ParamPositionSizeOptional {
	return bounds.incoming_lower
}

func AccountAdjustmentBoundsSetIncomingLower(
	bounds *AccountAdjustmentBounds,
	bound ParamPositionSize,
) {
	bounds.incoming_lower.value = bound
	bounds.incoming_lower.is_set = true
}

func AccountAdjustmentBoundsUnsetIncomingLower(bounds *AccountAdjustmentBounds) {
	bounds.incoming_lower = ParamPositionSizeOptional{}
}

//------------------------------------------------------------------------------
