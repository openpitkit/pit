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

type AccountAdjustment = C.PitAccountAdjustment
type AccountAdjustmentAmount = C.PitAccountAdjustmentAmount
type AccountAdjustmentAmountOptional = C.PitAccountAdjustmentAmountOptional
type AccountAdjustmentApplyStatus = C.PitAccountAdjustmentApplyStatus
type AccountAdjustmentBalanceOperation = C.PitAccountAdjustmentBalanceOperation
type AccountAdjustmentBalanceOperationOptional = C.PitAccountAdjustmentBalanceOperationOptional
type AccountAdjustmentBatchError = *C.PitAccountAdjustmentBatchError
type AccountAdjustmentBounds = C.PitAccountAdjustmentBounds
type AccountAdjustmentBoundsOptional = C.PitAccountAdjustmentBoundsOptional
type AccountAdjustmentContext = *C.PitAccountAdjustmentContext
type AccountAdjustmentPolicy = *C.PitAccountAdjustmentPolicy
type AccountAdjustmentPositionOperation = C.PitAccountAdjustmentPositionOperation
type AccountAdjustmentPositionOperationOptional = C.PitAccountAdjustmentPositionOperationOptional
type Engine = *C.PitEngine
type EngineBuilder = *C.PitEngineBuilder
type ExecutionReport = C.PitExecutionReport
type ExecutionReportFill = C.PitExecutionReportFill
type ExecutionReportFillOptional = C.PitExecutionReportFillOptional
type ExecutionReportIsFinalOptional = C.PitExecutionReportIsFinalOptional
type ExecutionReportOperation = C.PitExecutionReportOperation
type ExecutionReportOperationOptional = C.PitExecutionReportOperationOptional
type ExecutionReportPositionImpact = C.PitExecutionReportPositionImpact
type ExecutionReportPositionImpactOptional = C.PitExecutionReportPositionImpactOptional
type ExecutionReportTrade = C.PitExecutionReportTrade
type ExecutionReportTradeOptional = C.PitExecutionReportTradeOptional
type FinancialImpact = C.PitFinancialImpact
type FinancialImpactOptional = C.PitFinancialImpactOptional
type Instrument = C.PitInstrument
type Mutations = *C.PitMutations
type Order = C.PitOrder
type OrderMargin = C.PitOrderMargin
type OrderMarginOptional = C.PitOrderMarginOptional
type OrderOperation = C.PitOrderOperation
type OrderOperationOptional = C.PitOrderOperationOptional
type OrderPosition = C.PitOrderPosition
type OrderPositionOptional = C.PitOrderPositionOptional
type ParamAccountID = C.PitParamAccountId
type ParamAccountIDOptional = C.PitParamAccountIdOptional
type ParamAdjustmentAmount = C.PitParamAdjustmentAmount
type ParamAdjustmentAmountKind = C.PitParamAdjustmentAmountKind
type TriBool = C.PitTriBool
type ParamCashFlow = C.PitParamCashFlow
type ParamCashFlowOptional = C.PitParamCashFlowOptional
type ParamDecimal = C.PitParamDecimal
type ParamFee = C.PitParamFee
type ParamFeeOptional = C.PitParamFeeOptional
type ParamLeverage = C.PitParamLeverage
type ParamNotional = C.PitParamNotional
type ParamNotionalOptional = C.PitParamNotionalOptional
type ParamPnl = C.PitParamPnl
type ParamPnlOptional = C.PitParamPnlOptional
type ParamPositionEffect = C.PitParamPositionEffect
type ParamPositionMode = C.PitParamPositionMode
type ParamPositionSide = C.PitParamPositionSide
type ParamPositionSize = C.PitParamPositionSize
type ParamPositionSizeOptional = C.PitParamPositionSizeOptional
type ParamPrice = C.PitParamPrice
type ParamPriceOptional = C.PitParamPriceOptional
type ParamQuantity = C.PitParamQuantity
type ParamQuantityOptional = C.PitParamQuantityOptional
type ParamError = *C.PitParamError
type ParamErrorHandle = *C.PitParamError
type ParamErrorCode = C.PitParamErrorCode
type ParamRoundingStrategy = C.uint8_t
type ParamSide = C.PitParamSide
type ParamTradeAmount = C.PitParamTradeAmount
type ParamTradeAmountKind = C.PitParamTradeAmountKind
type ParamVolume = C.PitParamVolume
type ParamVolumeOptional = C.PitParamVolumeOptional
type PretradeCheckPreTradeStartPolicy = *C.PitPretradeCheckPreTradeStartPolicy
type PretradeContext = *C.PitPretradeContext
type PretradePoliciesOrderSizeLimitParam = C.PitPretradePoliciesOrderSizeLimitParam
type PretradePoliciesPnlBoundsBarrier = C.PitPretradePoliciesPnlBoundsBarrier
type PretradePreTradeLock = C.PitPretradePreTradeLock
type PretradePreTradePolicy = *C.PitPretradePreTradePolicy
type PretradePreTradeRequest = *C.PitPretradePreTradeRequest
type PretradePreTradeReservation = *C.PitPretradePreTradeReservation
type Reject = C.PitReject
type RejectCode = C.PitRejectCode
type RejectList = *C.PitRejectList
type RejectScope = C.PitRejectScope
type SharedString = *C.PitSharedString

const ParamLeverageNotSet = C.PIT_PARAM_LEVERAGE_NOT_SET

const (
	ParamLeverageScale = C.PIT_PARAM_LEVERAGE_SCALE
	ParamLeverageMin   = C.PIT_PARAM_LEVERAGE_MIN
	ParamLeverageMax   = C.PIT_PARAM_LEVERAGE_MAX
	ParamLeverageStep  = C.PIT_PARAM_LEVERAGE_STEP
)

const (
	ParamSideNotSet = C.PitParamSide_NotSet
	ParamSideBuy    = C.PitParamSide_Buy
	ParamSideSell   = C.PitParamSide_Sell
)

const (
	ParamPositionSideNotSet = C.PitParamPositionSide_NotSet
	ParamPositionSideLong   = C.PitParamPositionSide_Long
	ParamPositionSideShort  = C.PitParamPositionSide_Short
)

const (
	ParamPositionModeNotSet  = C.PitParamPositionMode_NotSet
	ParamPositionModeNetting = C.PitParamPositionMode_Netting
	ParamPositionModeHedged  = C.PitParamPositionMode_Hedged
)

const (
	ParamPositionEffectNotSet = C.PitParamPositionEffect_NotSet
	ParamPositionEffectOpen   = C.PitParamPositionEffect_Open
	ParamPositionEffectClose  = C.PitParamPositionEffect_Close
)

const (
	ParamTradeAmountKindNotSet   = C.PitParamTradeAmountKind_NotSet
	ParamTradeAmountKindQuantity = C.PitParamTradeAmountKind_Quantity
	ParamTradeAmountKindVolume   = C.PitParamTradeAmountKind_Volume
)

const (
	ParamRoundingStrategyMidpointNearestEven  = C.PitParamRoundingStrategy_MidpointNearestEven
	ParamRoundingStrategyMidpointAwayFromZero = C.PitParamRoundingStrategy_MidpointAwayFromZero
	ParamRoundingStrategyUp                   = C.PitParamRoundingStrategy_Up
	ParamRoundingStrategyDown                 = C.PitParamRoundingStrategy_Down
)

const (
	ParamRoundingStrategyDefault            = C.PIT_PARAM_ROUNDING_STRATEGY_DEFAULT
	ParamRoundingStrategyBanker             = C.PIT_PARAM_ROUNDING_STRATEGY_BANKER
	ParamRoundingStrategyConservativeProfit = C.PIT_PARAM_ROUNDING_STRATEGY_CONSERVATIVE_PROFIT
	ParamRoundingStrategyConservativeLoss   = C.PIT_PARAM_ROUNDING_STRATEGY_CONSERVATIVE_LOSS
)

const (
	ParamErrorCodeUnspecified     = C.PitParamErrorCode_Unspecified
	ParamErrorCodeNegative        = C.PitParamErrorCode_Negative
	ParamErrorCodeDivisionByZero  = C.PitParamErrorCode_DivisionByZero
	ParamErrorCodeOverflow        = C.PitParamErrorCode_Overflow
	ParamErrorCodeUnderflow       = C.PitParamErrorCode_Underflow
	ParamErrorCodeInvalidFloat    = C.PitParamErrorCode_InvalidFloat
	ParamErrorCodeInvalidFormat   = C.PitParamErrorCode_InvalidFormat
	ParamErrorCodeInvalidPrice    = C.PitParamErrorCode_InvalidPrice
	ParamErrorCodeInvalidLeverage = C.PitParamErrorCode_InvalidLeverage
	ParamErrorCodeAssetEmpty      = C.PitParamErrorCode_AssetEmpty
	ParamErrorCodeAccountIdEmpty  = C.PitParamErrorCode_AccountIdEmpty
	ParamErrorCodeOther           = C.PitParamErrorCode_Other
)

const (
	TriBoolNotSet = C.PitTriBool_NotSet
	TriBoolFalse  = C.PitTriBool_False
	TriBoolTrue   = C.PitTriBool_True
)

const (
	ParamAdjustmentAmountKindNotSet   = C.PitParamAdjustmentAmountKind_NotSet
	ParamAdjustmentAmountKindDelta    = C.PitParamAdjustmentAmountKind_Delta
	ParamAdjustmentAmountKindAbsolute = C.PitParamAdjustmentAmountKind_Absolute
)

const (
	RejectScopeOrder   = C.PitRejectScope_Order
	RejectScopeAccount = C.PitRejectScope_Account
)

const (
	RejectCodeMissingRequiredField        = C.PitRejectCode_MissingRequiredField
	RejectCodeInvalidFieldFormat          = C.PitRejectCode_InvalidFieldFormat
	RejectCodeInvalidFieldValue           = C.PitRejectCode_InvalidFieldValue
	RejectCodeUnsupportedOrderType        = C.PitRejectCode_UnsupportedOrderType
	RejectCodeUnsupportedTimeInForce      = C.PitRejectCode_UnsupportedTimeInForce
	RejectCodeUnsupportedOrderAttribute   = C.PitRejectCode_UnsupportedOrderAttribute
	RejectCodeDuplicateClientOrderID      = C.PitRejectCode_DuplicateClientOrderId
	RejectCodeTooLateToEnter              = C.PitRejectCode_TooLateToEnter
	RejectCodeExchangeClosed              = C.PitRejectCode_ExchangeClosed
	RejectCodeUnknownInstrument           = C.PitRejectCode_UnknownInstrument
	RejectCodeUnknownAccount              = C.PitRejectCode_UnknownAccount
	RejectCodeUnknownVenue                = C.PitRejectCode_UnknownVenue
	RejectCodeUnknownClearingAccount      = C.PitRejectCode_UnknownClearingAccount
	RejectCodeUnknownCollateralAsset      = C.PitRejectCode_UnknownCollateralAsset
	RejectCodeInsufficientFunds           = C.PitRejectCode_InsufficientFunds
	RejectCodeInsufficientMargin          = C.PitRejectCode_InsufficientMargin
	RejectCodeInsufficientPosition        = C.PitRejectCode_InsufficientPosition
	RejectCodeCreditLimitExceeded         = C.PitRejectCode_CreditLimitExceeded
	RejectCodeRiskLimitExceeded           = C.PitRejectCode_RiskLimitExceeded
	RejectCodeOrderExceedsLimit           = C.PitRejectCode_OrderExceedsLimit
	RejectCodeOrderQtyExceedsLimit        = C.PitRejectCode_OrderQtyExceedsLimit
	RejectCodeOrderNotionalExceedsLimit   = C.PitRejectCode_OrderNotionalExceedsLimit
	RejectCodePositionLimitExceeded       = C.PitRejectCode_PositionLimitExceeded
	RejectCodeConcentrationLimitExceeded  = C.PitRejectCode_ConcentrationLimitExceeded
	RejectCodeLeverageLimitExceeded       = C.PitRejectCode_LeverageLimitExceeded
	RejectCodeRateLimitExceeded           = C.PitRejectCode_RateLimitExceeded
	RejectCodePnlKillSwitchTriggered      = C.PitRejectCode_PnlKillSwitchTriggered
	RejectCodeAccountBlocked              = C.PitRejectCode_AccountBlocked
	RejectCodeAccountNotAuthorized        = C.PitRejectCode_AccountNotAuthorized
	RejectCodeComplianceRestriction       = C.PitRejectCode_ComplianceRestriction
	RejectCodeInstrumentRestricted        = C.PitRejectCode_InstrumentRestricted
	RejectCodeJurisdictionRestriction     = C.PitRejectCode_JurisdictionRestriction
	RejectCodeWashTradePrevention         = C.PitRejectCode_WashTradePrevention
	RejectCodeSelfMatchPrevention         = C.PitRejectCode_SelfMatchPrevention
	RejectCodeShortSaleRestriction        = C.PitRejectCode_ShortSaleRestriction
	RejectCodeRiskConfigurationMissing    = C.PitRejectCode_RiskConfigurationMissing
	RejectCodeReferenceDataUnavailable    = C.PitRejectCode_ReferenceDataUnavailable
	RejectCodeOrderValueCalculationFailed = C.PitRejectCode_OrderValueCalculationFailed
	RejectCodeSystemUnavailable           = C.PitRejectCode_SystemUnavailable
	RejectCodeCustom                      = C.PitRejectCode_Custom
	RejectCodeOther                       = C.PitRejectCode_Other
)
