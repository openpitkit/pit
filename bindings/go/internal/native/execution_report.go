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
// ExecutionReport

func NewExecutionReport() ExecutionReport {
	return ExecutionReport{}
}

func ExecutionReportReset(report *ExecutionReport) {
	*report = NewExecutionReport()
}

func ExecutionReportGetOperation(report ExecutionReport) ExecutionReportOperationOptional {
	return report.operation
}

func ExecutionReportGetOperationView(report *ExecutionReport) *ExecutionReportOperationOptional {
	return &report.operation
}

func ExecutionReportSetOperation(report *ExecutionReport, operation ExecutionReportOperation) {
	ExecutionReportOperationOptionalSet(&report.operation, operation)
}

func ExecutionReportUnsetOperation(report *ExecutionReport) {
	ExecutionReportOperationOptionalReset(&report.operation)
}

func ExecutionReportGetFinancialImpact(report ExecutionReport) FinancialImpactOptional {
	return report.financial_impact
}

func ExecutionReportGetFinancialImpactView(report *ExecutionReport) *FinancialImpactOptional {
	return &report.financial_impact
}

func ExecutionReportSetFinancialImpact(report *ExecutionReport, financialImpact FinancialImpact) {
	FinancialImpactOptionalSet(&report.financial_impact, financialImpact)
}

func ExecutionReportUnsetFinancialImpact(report *ExecutionReport) {
	FinancialImpactOptionalReset(&report.financial_impact)
}

func ExecutionReportGetFill(report ExecutionReport) ExecutionReportFillOptional {
	return report.fill
}

func ExecutionReportGetFillView(report *ExecutionReport) *ExecutionReportFillOptional {
	return &report.fill
}

func ExecutionReportSetFill(report *ExecutionReport, fill ExecutionReportFill) {
	ExecutionReportFillOptionalSet(&report.fill, fill)
}

func ExecutionReportUnsetFill(report *ExecutionReport) {
	ExecutionReportFillOptionalReset(&report.fill)
}

func ExecutionReportGetPositionImpact(
	report ExecutionReport,
) ExecutionReportPositionImpactOptional {
	return report.position_impact
}

func ExecutionReportGetPositionImpactView(
	report *ExecutionReport,
) *ExecutionReportPositionImpactOptional {
	return &report.position_impact
}

func ExecutionReportSetPositionImpact(
	report *ExecutionReport,
	positionImpact ExecutionReportPositionImpact,
) {
	ExecutionReportPositionImpactOptionalSet(&report.position_impact, positionImpact)
}

func ExecutionReportUnsetPositionImpact(report *ExecutionReport) {
	ExecutionReportPositionImpactOptionalReset(&report.position_impact)
}

func ExecutionReportGetUserData(report ExecutionReport) unsafe.Pointer {
	return report.user_data
}

func ExecutionReportSetUserData(report *ExecutionReport, userData unsafe.Pointer) {
	report.user_data = userData
}

//------------------------------------------------------------------------------
// ExecutionReportOperationOptional

func NewExecutionReportOperationOptional() ExecutionReportOperationOptional {
	return ExecutionReportOperationOptional{}
}

func ExecutionReportOperationOptionalReset(value *ExecutionReportOperationOptional) {
	ExecutionReportOperationReset(&value.value)
	value.is_set = false
}

func ExecutionReportOperationOptionalIsSet(value ExecutionReportOperationOptional) bool {
	return bool(value.is_set)
}

func ExecutionReportOperationOptionalGet(
	value ExecutionReportOperationOptional,
) ExecutionReportOperation {
	return value.value
}

func ExecutionReportOperationOptionalGetView(
	value *ExecutionReportOperationOptional,
) *ExecutionReportOperation {
	return &value.value
}

func ExecutionReportOperationOptionalSet(
	value *ExecutionReportOperationOptional,
	operation ExecutionReportOperation,
) {
	value.value = operation
	value.is_set = true
}

//------------------------------------------------------------------------------
// FinancialImpactOptional

func NewFinancialImpactOptional() FinancialImpactOptional {
	return FinancialImpactOptional{}
}

func FinancialImpactOptionalReset(value *FinancialImpactOptional) {
	FinancialImpactReset(&value.value)
	value.is_set = false
}

func FinancialImpactOptionalIsSet(value FinancialImpactOptional) bool {
	return bool(value.is_set)
}

func FinancialImpactOptionalGet(value FinancialImpactOptional) FinancialImpact {
	return value.value
}

func FinancialImpactOptionalGetView(value *FinancialImpactOptional) *FinancialImpact {
	return &value.value
}

func FinancialImpactOptionalSet(value *FinancialImpactOptional, financialImpact FinancialImpact) {
	value.value = financialImpact
	value.is_set = true
}

//------------------------------------------------------------------------------
// ExecutionReportTradeOptional

func NewExecutionReportTradeOptional() ExecutionReportTradeOptional {
	return ExecutionReportTradeOptional{}
}

func ExecutionReportTradeOptionalReset(value *ExecutionReportTradeOptional) {
	ExecutionReportTradeReset(&value.value)
	value.is_set = false
}

func ExecutionReportTradeOptionalIsSet(value ExecutionReportTradeOptional) bool {
	return bool(value.is_set)
}

func ExecutionReportTradeOptionalGet(value ExecutionReportTradeOptional) ExecutionReportTrade {
	return value.value
}

func ExecutionReportTradeOptionalGetView(
	value *ExecutionReportTradeOptional,
) *ExecutionReportTrade {
	return &value.value
}

func ExecutionReportTradeOptionalSet(
	value *ExecutionReportTradeOptional,
	trade ExecutionReportTrade,
) {
	value.value = trade
	value.is_set = true
}

//------------------------------------------------------------------------------
// ExecutionReportFillOptional

func NewExecutionReportFillOptional() ExecutionReportFillOptional {
	return ExecutionReportFillOptional{}
}

func ExecutionReportFillOptionalReset(value *ExecutionReportFillOptional) {
	ExecutionReportFillReset(&value.value)
	value.is_set = false
}

func ExecutionReportFillOptionalIsSet(value ExecutionReportFillOptional) bool {
	return bool(value.is_set)
}

func ExecutionReportFillOptionalGet(value ExecutionReportFillOptional) ExecutionReportFill {
	return value.value
}

func ExecutionReportFillOptionalGetView(value *ExecutionReportFillOptional) *ExecutionReportFill {
	return &value.value
}

func ExecutionReportFillOptionalSet(value *ExecutionReportFillOptional, fill ExecutionReportFill) {
	value.value = fill
	value.is_set = true
}

//------------------------------------------------------------------------------
// ExecutionReportPositionImpactOptional

func NewExecutionReportPositionImpactOptional() ExecutionReportPositionImpactOptional {
	return ExecutionReportPositionImpactOptional{}
}

func ExecutionReportPositionImpactOptionalReset(value *ExecutionReportPositionImpactOptional) {
	ExecutionReportPositionImpactReset(&value.value)
	value.is_set = false
}

func ExecutionReportPositionImpactOptionalIsSet(value ExecutionReportPositionImpactOptional) bool {
	return bool(value.is_set)
}

func ExecutionReportPositionImpactOptionalGet(
	value ExecutionReportPositionImpactOptional,
) ExecutionReportPositionImpact {
	return value.value
}

func ExecutionReportPositionImpactOptionalGetView(
	value *ExecutionReportPositionImpactOptional,
) *ExecutionReportPositionImpact {
	return &value.value
}

func ExecutionReportPositionImpactOptionalSet(
	value *ExecutionReportPositionImpactOptional,
	positionImpact ExecutionReportPositionImpact,
) {
	value.value = positionImpact
	value.is_set = true
}

//------------------------------------------------------------------------------
// ExecutionReportOperation

func NewExecutionReportOperation() ExecutionReportOperation {
	return ExecutionReportOperation{}
}

func ExecutionReportOperationReset(operation *ExecutionReportOperation) {
	*operation = NewExecutionReportOperation()
}

func ExecutionReportOperationGetInstrument(operation ExecutionReportOperation) Instrument {
	return operation.instrument
}

func ExecutionReportOperationSetInstrument(
	operation *ExecutionReportOperation,
	instrument Instrument,
) {
	operation.instrument = instrument
}

func ExecutionReportOperationUnsetInstrument(operation *ExecutionReportOperation) {
	operation.instrument = Instrument{}
}

func ExecutionReportOperationGetAccountID(
	operation ExecutionReportOperation,
) ParamAccountIDOptional {
	return operation.account_id
}

func ExecutionReportOperationSetAccountID(
	operation *ExecutionReportOperation,
	accountID ParamAccountID,
) {
	operation.account_id.value = accountID
	operation.account_id.is_set = true
}

func ExecutionReportOperationUnsetAccountID(operation *ExecutionReportOperation) {
	operation.account_id = ParamAccountIDOptional{}
}

func ExecutionReportOperationGetSide(operation ExecutionReportOperation) ParamSide {
	return operation.side
}

func ExecutionReportOperationSetSide(operation *ExecutionReportOperation, side ParamSide) {
	operation.side = side
}

func ExecutionReportOperationUnsetSide(operation *ExecutionReportOperation) {
	operation.side = ParamSideNotSet
}

//------------------------------------------------------------------------------
// FinancialImpact

func NewFinancialImpact() FinancialImpact {
	return FinancialImpact{}
}

func FinancialImpactReset(financialImpact *FinancialImpact) {
	*financialImpact = NewFinancialImpact()
}

func FinancialImpactGetPnl(financialImpact FinancialImpact) ParamPnlOptional {
	return financialImpact.pnl
}

func FinancialImpactSetPnl(financialImpact *FinancialImpact, pnl ParamPnl) {
	financialImpact.pnl.value = pnl
	financialImpact.pnl.is_set = true
}

func FinancialImpactUnsetPnl(financialImpact *FinancialImpact) {
	financialImpact.pnl = ParamPnlOptional{}
}

func FinancialImpactGetFee(financialImpact FinancialImpact) ParamFeeOptional {
	return financialImpact.fee
}

func FinancialImpactSetFee(financialImpact *FinancialImpact, fee ParamFee) {
	financialImpact.fee.value = fee
	financialImpact.fee.is_set = true
}

func FinancialImpactUnsetFee(financialImpact *FinancialImpact) {
	financialImpact.fee = ParamFeeOptional{}
}

//------------------------------------------------------------------------------
// ExecutionReportTrade

func NewExecutionReportTrade() ExecutionReportTrade {
	return ExecutionReportTrade{}
}

func ExecutionReportTradeReset(trade *ExecutionReportTrade) {
	*trade = NewExecutionReportTrade()
}

func ExecutionReportTradeGetPrice(trade ExecutionReportTrade) ParamPrice {
	return trade.price
}

func ExecutionReportTradeSetPrice(trade *ExecutionReportTrade, price ParamPrice) {
	trade.price = price
}

func ExecutionReportTradeGetQuantity(trade ExecutionReportTrade) ParamQuantity {
	return trade.quantity
}

func ExecutionReportTradeSetQuantity(trade *ExecutionReportTrade, quantity ParamQuantity) {
	trade.quantity = quantity
}

//------------------------------------------------------------------------------
// ExecutionReportFill

func NewExecutionReportFill() ExecutionReportFill {
	return ExecutionReportFill{}
}

func ExecutionReportFillReset(fill *ExecutionReportFill) {
	*fill = NewExecutionReportFill()
}

func ExecutionReportFillGetLastTrade(fill ExecutionReportFill) ExecutionReportTradeOptional {
	return fill.last_trade
}

func ExecutionReportFillGetLastTradeView(fill *ExecutionReportFill) *ExecutionReportTradeOptional {
	return &fill.last_trade
}

func ExecutionReportFillSetLastTrade(fill *ExecutionReportFill, trade ExecutionReportTrade) {
	ExecutionReportTradeOptionalSet(&fill.last_trade, trade)
}

func ExecutionReportFillUnsetLastTrade(fill *ExecutionReportFill) {
	ExecutionReportTradeOptionalReset(&fill.last_trade)
}

func ExecutionReportFillGetLeavesQuantity(fill ExecutionReportFill) ParamQuantityOptional {
	return fill.leaves_quantity
}

func ExecutionReportFillSetLeavesQuantity(fill *ExecutionReportFill, quantity ParamQuantity) {
	fill.leaves_quantity.value = quantity
	fill.leaves_quantity.is_set = true
}

func ExecutionReportFillUnsetLeavesQuantity(fill *ExecutionReportFill) {
	fill.leaves_quantity = ParamQuantityOptional{}
}

func ExecutionReportFillGetLockPrice(fill ExecutionReportFill) ParamPriceOptional {
	return fill.lock_price
}

func ExecutionReportFillSetLockPrice(fill *ExecutionReportFill, price ParamPrice) {
	fill.lock_price.value = price
	fill.lock_price.is_set = true
}

func ExecutionReportFillUnsetLockPrice(fill *ExecutionReportFill) {
	fill.lock_price = ParamPriceOptional{}
}

// ExecutionReportFillGetFinal returns the final flag when it is set.
func ExecutionReportFillGetFinal(fill ExecutionReportFill) ExecutionReportIsFinalOptional {
	return fill.is_final
}

// ExecutionReportFillSetFinal marks the fill as closing the report stream.
func ExecutionReportFillSetFinal(fill *ExecutionReportFill, isFinal bool) {
	fill.is_final.value = C.bool(isFinal)
	fill.is_final.is_set = true
}

// ExecutionReportFillUnsetFinal clears the final flag.
func ExecutionReportFillUnsetFinal(fill *ExecutionReportFill) {
	fill.is_final = ExecutionReportIsFinalOptional{}
}

// ExecutionReportIsFinalOptionalIsSet reports whether the final flag is set.
func ExecutionReportIsFinalOptionalIsSet(value ExecutionReportIsFinalOptional) bool {
	return bool(value.is_set)
}

// ExecutionReportIsFinalOptionalGet returns the stored final flag value.
func ExecutionReportIsFinalOptionalGet(value ExecutionReportIsFinalOptional) bool {
	return bool(value.value)
}

//------------------------------------------------------------------------------
// ExecutionReportPositionImpact

func NewExecutionReportPositionImpact() ExecutionReportPositionImpact {
	return ExecutionReportPositionImpact{}
}

func ExecutionReportPositionImpactReset(positionImpact *ExecutionReportPositionImpact) {
	*positionImpact = NewExecutionReportPositionImpact()
}

func ExecutionReportPositionImpactGetPositionEffect(
	positionImpact ExecutionReportPositionImpact,
) ParamPositionEffect {
	return positionImpact.position_effect
}

func ExecutionReportPositionImpactSetPositionEffect(
	positionImpact *ExecutionReportPositionImpact,
	positionEffect ParamPositionEffect,
) {
	positionImpact.position_effect = positionEffect
}

func ExecutionReportPositionImpactUnsetPositionEffect(
	positionImpact *ExecutionReportPositionImpact,
) {
	positionImpact.position_effect = ParamPositionEffectNotSet
}

func ExecutionReportPositionImpactGetPositionSide(
	positionImpact ExecutionReportPositionImpact,
) ParamPositionSide {
	return positionImpact.position_side
}

func ExecutionReportPositionImpactSetPositionSide(
	positionImpact *ExecutionReportPositionImpact,
	positionSide ParamPositionSide,
) {
	positionImpact.position_side = positionSide
}

func ExecutionReportPositionImpactUnsetPositionSide(
	positionImpact *ExecutionReportPositionImpact,
) {
	positionImpact.position_side = ParamPositionSideNotSet
}
