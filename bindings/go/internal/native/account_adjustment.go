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
// AccountAdjustment

func NewAccountAdjustment() AccountAdjustment {
	return AccountAdjustment{}
}

func AccountAdjustmentReset(adjustment *AccountAdjustment) {
	*adjustment = NewAccountAdjustment()
}

func AccountAdjustmentGetBalanceOperation(
	adjustment AccountAdjustment,
) AccountAdjustmentBalanceOperationOptional {
	return adjustment.balance_operation
}

func AccountAdjustmentGetBalanceOperationView(
	adjustment *AccountAdjustment,
) *AccountAdjustmentBalanceOperationOptional {
	return &adjustment.balance_operation
}

func AccountAdjustmentSetBalanceOperationAndUnsetPositionOperation(
	adjustment *AccountAdjustment,
	operation AccountAdjustmentBalanceOperation,
) {
	AccountAdjustmentBalanceOperationOptionalSet(&adjustment.balance_operation, operation)
	AccountAdjustmentPositionOperationOptionalReset(&adjustment.position_operation)
}

func AccountAdjustmentUnsetBalanceOperation(adjustment *AccountAdjustment) {
	AccountAdjustmentBalanceOperationOptionalReset(&adjustment.balance_operation)
}

func AccountAdjustmentGetPositionOperation(
	adjustment AccountAdjustment,
) AccountAdjustmentPositionOperationOptional {
	return adjustment.position_operation
}

func AccountAdjustmentGetPositionOperationView(
	adjustment *AccountAdjustment,
) *AccountAdjustmentPositionOperationOptional {
	return &adjustment.position_operation
}

func AccountAdjustmentSetPositionOperationAndUnsetBalanceOperation(
	adjustment *AccountAdjustment,
	operation AccountAdjustmentPositionOperation,
) {
	AccountAdjustmentPositionOperationOptionalSet(&adjustment.position_operation, operation)
	AccountAdjustmentBalanceOperationOptionalReset(&adjustment.balance_operation)
}

func AccountAdjustmentUnsetPositionOperation(adjustment *AccountAdjustment) {
	AccountAdjustmentPositionOperationOptionalReset(&adjustment.position_operation)
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
// AccountAdjustmentBalanceOperationOptional

func NewAccountAdjustmentBalanceOperationOptional() AccountAdjustmentBalanceOperationOptional {
	return AccountAdjustmentBalanceOperationOptional{}
}

func AccountAdjustmentBalanceOperationOptionalReset(
	operation *AccountAdjustmentBalanceOperationOptional,
) {
	AccountAdjustmentBalanceOperationReset(&operation.value)
	operation.is_set = false
}

func AccountAdjustmentBalanceOperationOptionalIsSet(
	value AccountAdjustmentBalanceOperationOptional,
) bool {
	return bool(value.is_set)
}

func AccountAdjustmentBalanceOperationOptionalGet(
	value AccountAdjustmentBalanceOperationOptional,
) AccountAdjustmentBalanceOperation {
	return value.value
}

func AccountAdjustmentBalanceOperationOptionalGetView(
	value *AccountAdjustmentBalanceOperationOptional,
) *AccountAdjustmentBalanceOperation {
	return &value.value
}

func AccountAdjustmentBalanceOperationOptionalSet(
	value *AccountAdjustmentBalanceOperationOptional,
	operation AccountAdjustmentBalanceOperation,
) {
	value.value = operation
	value.is_set = true
}

//------------------------------------------------------------------------------
// AccountAdjustmentPositionOperationOptional

func NewAccountAdjustmentPositionOperationOptional() AccountAdjustmentPositionOperationOptional {
	return AccountAdjustmentPositionOperationOptional{}
}

func AccountAdjustmentPositionOperationOptionalReset(
	operation *AccountAdjustmentPositionOperationOptional,
) {
	AccountAdjustmentPositionOperationReset(&operation.value)
	operation.is_set = false
}

func AccountAdjustmentPositionOperationOptionalIsSet(
	value AccountAdjustmentPositionOperationOptional,
) bool {
	return bool(value.is_set)
}

func AccountAdjustmentPositionOperationOptionalGet(
	value AccountAdjustmentPositionOperationOptional,
) AccountAdjustmentPositionOperation {
	return value.value
}

func AccountAdjustmentPositionOperationOptionalGetView(
	value *AccountAdjustmentPositionOperationOptional,
) *AccountAdjustmentPositionOperation {
	return &value.value
}

func AccountAdjustmentPositionOperationOptionalSet(
	value *AccountAdjustmentPositionOperationOptional,
	operation AccountAdjustmentPositionOperation,
) {
	value.value = operation
	value.is_set = true
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

func AccountAdjustmentAmountGetTotal(amount AccountAdjustmentAmount) ParamAdjustmentAmount {
	return amount.total
}

func AccountAdjustmentAmountSetTotal(amount *AccountAdjustmentAmount, value ParamAdjustmentAmount) {
	amount.total = value
}

func AccountAdjustmentAmountUnsetTotal(amount *AccountAdjustmentAmount) {
	amount.total = ParamAdjustmentAmount{}
}

func AccountAdjustmentAmountGetReserved(amount AccountAdjustmentAmount) ParamAdjustmentAmount {
	return amount.reserved
}

func AccountAdjustmentAmountSetReserved(
	amount *AccountAdjustmentAmount,
	value ParamAdjustmentAmount,
) {
	amount.reserved = value
}

func AccountAdjustmentAmountUnsetReserved(amount *AccountAdjustmentAmount) {
	amount.reserved = ParamAdjustmentAmount{}
}

func AccountAdjustmentAmountGetPending(amount AccountAdjustmentAmount) ParamAdjustmentAmount {
	return amount.pending
}

func AccountAdjustmentAmountSetPending(
	amount *AccountAdjustmentAmount,
	value ParamAdjustmentAmount,
) {
	amount.pending = value
}

func AccountAdjustmentAmountUnsetPending(amount *AccountAdjustmentAmount) {
	amount.pending = ParamAdjustmentAmount{}
}

//------------------------------------------------------------------------------
// AccountAdjustmentBounds

func NewAccountAdjustmentBounds() AccountAdjustmentBounds {
	return AccountAdjustmentBounds{}
}

func AccountAdjustmentBoundsReset(bounds *AccountAdjustmentBounds) {
	*bounds = NewAccountAdjustmentBounds()
}

func AccountAdjustmentBoundsGetTotalUpper(
	bounds AccountAdjustmentBounds,
) ParamPositionSizeOptional {
	return bounds.total_upper
}

func AccountAdjustmentBoundsSetTotalUpper(
	bounds *AccountAdjustmentBounds,
	bound ParamPositionSize,
) {
	bounds.total_upper.value = bound
	bounds.total_upper.is_set = true
}

func AccountAdjustmentBoundsUnsetTotalUpper(bounds *AccountAdjustmentBounds) {
	bounds.total_upper = ParamPositionSizeOptional{}
}

func AccountAdjustmentBoundsGetTotalLower(
	bounds AccountAdjustmentBounds,
) ParamPositionSizeOptional {
	return bounds.total_lower
}

func AccountAdjustmentBoundsSetTotalLower(
	bounds *AccountAdjustmentBounds,
	bound ParamPositionSize,
) {
	bounds.total_lower.value = bound
	bounds.total_lower.is_set = true
}

func AccountAdjustmentBoundsUnsetTotalLower(bounds *AccountAdjustmentBounds) {
	bounds.total_lower = ParamPositionSizeOptional{}
}

func AccountAdjustmentBoundsGetReservedUpper(
	bounds AccountAdjustmentBounds,
) ParamPositionSizeOptional {
	return bounds.reserved_upper
}

func AccountAdjustmentBoundsSetReservedUpper(
	bounds *AccountAdjustmentBounds,
	bound ParamPositionSize,
) {
	bounds.reserved_upper.value = bound
	bounds.reserved_upper.is_set = true
}

func AccountAdjustmentBoundsUnsetReservedUpper(bounds *AccountAdjustmentBounds) {
	bounds.reserved_upper = ParamPositionSizeOptional{}
}

func AccountAdjustmentBoundsGetReservedLower(
	bounds AccountAdjustmentBounds,
) ParamPositionSizeOptional {
	return bounds.reserved_lower
}

func AccountAdjustmentBoundsSetReservedLower(
	bounds *AccountAdjustmentBounds,
	bound ParamPositionSize,
) {
	bounds.reserved_lower.value = bound
	bounds.reserved_lower.is_set = true
}

func AccountAdjustmentBoundsUnsetReservedLower(bounds *AccountAdjustmentBounds) {
	bounds.reserved_lower = ParamPositionSizeOptional{}
}

func AccountAdjustmentBoundsGetPendingUpper(
	bounds AccountAdjustmentBounds,
) ParamPositionSizeOptional {
	return bounds.pending_upper
}

func AccountAdjustmentBoundsSetPendingUpper(
	bounds *AccountAdjustmentBounds,
	bound ParamPositionSize,
) {
	bounds.pending_upper.value = bound
	bounds.pending_upper.is_set = true
}

func AccountAdjustmentBoundsUnsetPendingUpper(bounds *AccountAdjustmentBounds) {
	bounds.pending_upper = ParamPositionSizeOptional{}
}

func AccountAdjustmentBoundsGetPendingLower(
	bounds AccountAdjustmentBounds,
) ParamPositionSizeOptional {
	return bounds.pending_lower
}

func AccountAdjustmentBoundsSetPendingLower(
	bounds *AccountAdjustmentBounds,
	bound ParamPositionSize,
) {
	bounds.pending_lower.value = bound
	bounds.pending_lower.is_set = true
}

func AccountAdjustmentBoundsUnsetPendingLower(bounds *AccountAdjustmentBounds) {
	bounds.pending_lower = ParamPositionSizeOptional{}
}

//------------------------------------------------------------------------------
