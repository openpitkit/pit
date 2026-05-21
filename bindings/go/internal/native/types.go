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

type AccountAdjustment = C.OpenPitAccountAdjustment
type AccountAdjustmentAmount = C.OpenPitAccountAdjustmentAmount
type AccountAdjustmentAmountOptional = C.OpenPitAccountAdjustmentAmountOptional
type AccountAdjustmentApplyStatus = C.OpenPitAccountAdjustmentApplyStatus
type AccountAdjustmentBalanceOperation = C.OpenPitAccountAdjustmentBalanceOperation
type AccountAdjustmentBalanceOperationOptional = C.OpenPitAccountAdjustmentBalanceOperationOptional
type AccountAdjustmentBatchError = *C.OpenPitAccountAdjustmentBatchError
type AccountAdjustmentBounds = C.OpenPitAccountAdjustmentBounds
type AccountAdjustmentBoundsOptional = C.OpenPitAccountAdjustmentBoundsOptional
type AccountAdjustmentContext = *C.OpenPitAccountAdjustmentContext
type AccountAdjustmentPositionOperation = C.OpenPitAccountAdjustmentPositionOperation
type AccountAdjustmentPositionOperationOptional = C.OpenPitAccountAdjustmentPositionOperationOptional
type Engine = *C.OpenPitEngine
type EngineBuilder = *C.OpenPitEngineBuilder
type ExecutionReport = C.OpenPitExecutionReport
type ExecutionReportFill = C.OpenPitExecutionReportFill
type ExecutionReportFillOptional = C.OpenPitExecutionReportFillOptional
type ExecutionReportIsFinalOptional = C.OpenPitExecutionReportIsFinalOptional
type ExecutionReportOperation = C.OpenPitExecutionReportOperation
type ExecutionReportOperationOptional = C.OpenPitExecutionReportOperationOptional
type ExecutionReportPositionImpact = C.OpenPitExecutionReportPositionImpact
type ExecutionReportPositionImpactOptional = C.OpenPitExecutionReportPositionImpactOptional
type ExecutionReportTrade = C.OpenPitExecutionReportTrade
type ExecutionReportTradeOptional = C.OpenPitExecutionReportTradeOptional
type FinancialImpact = C.OpenPitFinancialImpact
type FinancialImpactOptional = C.OpenPitFinancialImpactOptional
type Instrument = C.OpenPitInstrument
type Mutations = *C.OpenPitMutations
type Order = C.OpenPitOrder
type OrderMargin = C.OpenPitOrderMargin
type OrderMarginOptional = C.OpenPitOrderMarginOptional
type OrderOperation = C.OpenPitOrderOperation
type OrderOperationOptional = C.OpenPitOrderOperationOptional
type OrderPosition = C.OpenPitOrderPosition
type OrderPositionOptional = C.OpenPitOrderPositionOptional
type ParamAccountID = C.OpenPitParamAccountId
type ParamAccountIDOptional = C.OpenPitParamAccountIdOptional
type ParamAdjustmentAmount = C.OpenPitParamAdjustmentAmount
type ParamAdjustmentAmountKind = C.OpenPitParamAdjustmentAmountKind
type TriBool = C.OpenPitTriBool
type ParamCashFlow = C.OpenPitParamCashFlow
type ParamCashFlowOptional = C.OpenPitParamCashFlowOptional
type ParamDecimal = C.OpenPitParamDecimal
type ParamFee = C.OpenPitParamFee
type ParamFeeOptional = C.OpenPitParamFeeOptional
type ParamLeverage = C.OpenPitParamLeverage
type ParamNotional = C.OpenPitParamNotional
type ParamNotionalOptional = C.OpenPitParamNotionalOptional
type ParamPnl = C.OpenPitParamPnl
type ParamPnlOptional = C.OpenPitParamPnlOptional
type ParamPositionEffect = C.OpenPitParamPositionEffect
type ParamPositionMode = C.OpenPitParamPositionMode
type ParamPositionSide = C.OpenPitParamPositionSide
type ParamPositionSize = C.OpenPitParamPositionSize
type ParamPositionSizeOptional = C.OpenPitParamPositionSizeOptional
type ParamPrice = C.OpenPitParamPrice
type ParamPriceOptional = C.OpenPitParamPriceOptional
type ParamQuantity = C.OpenPitParamQuantity
type ParamQuantityOptional = C.OpenPitParamQuantityOptional
type ParamError = *C.OpenPitParamError
type ParamErrorHandle = *C.OpenPitParamError
type ParamErrorCode = C.OpenPitParamErrorCode
type ParamRoundingStrategy = C.uint8_t
type ParamSide = C.OpenPitParamSide
type ParamTradeAmount = C.OpenPitParamTradeAmount
type ParamTradeAmountKind = C.OpenPitParamTradeAmountKind
type ParamVolume = C.OpenPitParamVolume
type ParamVolumeOptional = C.OpenPitParamVolumeOptional
type PretradeContext = *C.OpenPitPretradeContext
type PretradePoliciesOrderSizeAccountAssetBarrier = C.OpenPitPretradePoliciesOrderSizeAccountAssetBarrier
type PretradePoliciesOrderSizeAssetBarrier = C.OpenPitPretradePoliciesOrderSizeAssetBarrier
type PretradePoliciesOrderSizeBrokerBarrier = C.OpenPitPretradePoliciesOrderSizeBrokerBarrier
type PretradePoliciesOrderSizeLimit = C.OpenPitPretradePoliciesOrderSizeLimit
type PretradePoliciesPnlBoundsAccountBarrier = C.OpenPitPretradePoliciesPnlBoundsAccountBarrier
type PretradePoliciesPnlBoundsBarrier = C.OpenPitPretradePoliciesPnlBoundsBarrier
type PretradePoliciesRateLimitAccountAssetBarrier = C.OpenPitPretradePoliciesRateLimitAccountAssetBarrier
type PretradePoliciesRateLimitAccountBarrier = C.OpenPitPretradePoliciesRateLimitAccountBarrier
type PretradePoliciesRateLimitAssetBarrier = C.OpenPitPretradePoliciesRateLimitAssetBarrier
type PretradePoliciesRateLimitBrokerBarrier = C.OpenPitPretradePoliciesRateLimitBrokerBarrier
type PretradePreTradeLock = C.OpenPitPretradePreTradeLock
type PretradePreTradePolicy = *C.OpenPitPretradePreTradePolicy
type PretradePreTradeRequest = *C.OpenPitPretradePreTradeRequest
type PretradePreTradeReservation = *C.OpenPitPretradePreTradeReservation
type PretradeAccountBlock = C.OpenPitPretradeAccountBlock
type PretradeAccountBlockList = *C.OpenPitPretradeAccountBlockList
type PretradeReject = C.OpenPitPretradeReject
type PretradeRejectCode = C.OpenPitPretradeRejectCode
type PretradeRejectList = *C.OpenPitPretradeRejectList
type PretradeRejectScope = C.OpenPitPretradeRejectScope
type SharedString = *C.OpenPitSharedString

const ParamLeverageNotSet = C.OPENPIT_PARAM_LEVERAGE_NOT_SET

const (
	ParamLeverageScale = C.OPENPIT_PARAM_LEVERAGE_SCALE
	ParamLeverageMin   = C.OPENPIT_PARAM_LEVERAGE_MIN
	ParamLeverageMax   = C.OPENPIT_PARAM_LEVERAGE_MAX
	ParamLeverageStep  = C.OPENPIT_PARAM_LEVERAGE_STEP
)

const (
	ParamSideNotSet = C.OpenPitParamSide_NotSet
	ParamSideBuy    = C.OpenPitParamSide_Buy
	ParamSideSell   = C.OpenPitParamSide_Sell
)

const (
	ParamPositionSideNotSet = C.OpenPitParamPositionSide_NotSet
	ParamPositionSideLong   = C.OpenPitParamPositionSide_Long
	ParamPositionSideShort  = C.OpenPitParamPositionSide_Short
)

const (
	ParamPositionModeNotSet  = C.OpenPitParamPositionMode_NotSet
	ParamPositionModeNetting = C.OpenPitParamPositionMode_Netting
	ParamPositionModeHedged  = C.OpenPitParamPositionMode_Hedged
)

const (
	ParamPositionEffectNotSet = C.OpenPitParamPositionEffect_NotSet
	ParamPositionEffectOpen   = C.OpenPitParamPositionEffect_Open
	ParamPositionEffectClose  = C.OpenPitParamPositionEffect_Close
)

const (
	ParamTradeAmountKindNotSet   = C.OpenPitParamTradeAmountKind_NotSet
	ParamTradeAmountKindQuantity = C.OpenPitParamTradeAmountKind_Quantity
	ParamTradeAmountKindVolume   = C.OpenPitParamTradeAmountKind_Volume
)

const (
	ParamRoundingStrategyMidpointNearestEven  = C.OpenPitParamRoundingStrategy_MidpointNearestEven
	ParamRoundingStrategyMidpointAwayFromZero = C.OpenPitParamRoundingStrategy_MidpointAwayFromZero
	ParamRoundingStrategyUp                   = C.OpenPitParamRoundingStrategy_Up
	ParamRoundingStrategyDown                 = C.OpenPitParamRoundingStrategy_Down
)

const (
	ParamRoundingStrategyDefault            = C.OPENPIT_PARAM_ROUNDING_STRATEGY_DEFAULT
	ParamRoundingStrategyBanker             = C.OPENPIT_PARAM_ROUNDING_STRATEGY_BANKER
	ParamRoundingStrategyConservativeProfit = C.OPENPIT_PARAM_ROUNDING_STRATEGY_CONSERVATIVE_PROFIT
	ParamRoundingStrategyConservativeLoss   = C.OPENPIT_PARAM_ROUNDING_STRATEGY_CONSERVATIVE_LOSS
)

const (
	ParamErrorCodeUnspecified     = C.OpenPitParamErrorCode_Unspecified
	ParamErrorCodeNegative        = C.OpenPitParamErrorCode_Negative
	ParamErrorCodeDivisionByZero  = C.OpenPitParamErrorCode_DivisionByZero
	ParamErrorCodeOverflow        = C.OpenPitParamErrorCode_Overflow
	ParamErrorCodeUnderflow       = C.OpenPitParamErrorCode_Underflow
	ParamErrorCodeInvalidFloat    = C.OpenPitParamErrorCode_InvalidFloat
	ParamErrorCodeInvalidFormat   = C.OpenPitParamErrorCode_InvalidFormat
	ParamErrorCodeInvalidPrice    = C.OpenPitParamErrorCode_InvalidPrice
	ParamErrorCodeInvalidLeverage = C.OpenPitParamErrorCode_InvalidLeverage
	ParamErrorCodeAssetEmpty      = C.OpenPitParamErrorCode_AssetEmpty
	ParamErrorCodeAccountIdEmpty  = C.OpenPitParamErrorCode_AccountIdEmpty
	ParamErrorCodeOther           = C.OpenPitParamErrorCode_Other
)

const (
	TriBoolNotSet = C.OpenPitTriBool_NotSet
	TriBoolFalse  = C.OpenPitTriBool_False
	TriBoolTrue   = C.OpenPitTriBool_True
)

const (
	ParamAdjustmentAmountKindNotSet   = C.OpenPitParamAdjustmentAmountKind_NotSet
	ParamAdjustmentAmountKindDelta    = C.OpenPitParamAdjustmentAmountKind_Delta
	ParamAdjustmentAmountKindAbsolute = C.OpenPitParamAdjustmentAmountKind_Absolute
)

const (
	RejectScopeOrder   = C.OpenPitPretradeRejectScope_Order
	RejectScopeAccount = C.OpenPitPretradeRejectScope_Account
)

const (
	RejectCodeMissingRequiredField        = C.OpenPitPretradeRejectCode_MissingRequiredField
	RejectCodeInvalidFieldFormat          = C.OpenPitPretradeRejectCode_InvalidFieldFormat
	RejectCodeInvalidFieldValue           = C.OpenPitPretradeRejectCode_InvalidFieldValue
	RejectCodeUnsupportedOrderType        = C.OpenPitPretradeRejectCode_UnsupportedOrderType
	RejectCodeUnsupportedTimeInForce      = C.OpenPitPretradeRejectCode_UnsupportedTimeInForce
	RejectCodeUnsupportedOrderAttribute   = C.OpenPitPretradeRejectCode_UnsupportedOrderAttribute
	RejectCodeDuplicateClientOrderID      = C.OpenPitPretradeRejectCode_DuplicateClientOrderId
	RejectCodeTooLateToEnter              = C.OpenPitPretradeRejectCode_TooLateToEnter
	RejectCodeExchangeClosed              = C.OpenPitPretradeRejectCode_ExchangeClosed
	RejectCodeUnknownInstrument           = C.OpenPitPretradeRejectCode_UnknownInstrument
	RejectCodeUnknownAccount              = C.OpenPitPretradeRejectCode_UnknownAccount
	RejectCodeUnknownVenue                = C.OpenPitPretradeRejectCode_UnknownVenue
	RejectCodeUnknownClearingAccount      = C.OpenPitPretradeRejectCode_UnknownClearingAccount
	RejectCodeUnknownCollateralAsset      = C.OpenPitPretradeRejectCode_UnknownCollateralAsset
	RejectCodeInsufficientFunds           = C.OpenPitPretradeRejectCode_InsufficientFunds
	RejectCodeInsufficientMargin          = C.OpenPitPretradeRejectCode_InsufficientMargin
	RejectCodeInsufficientPosition        = C.OpenPitPretradeRejectCode_InsufficientPosition
	RejectCodeCreditLimitExceeded         = C.OpenPitPretradeRejectCode_CreditLimitExceeded
	RejectCodeRiskLimitExceeded           = C.OpenPitPretradeRejectCode_RiskLimitExceeded
	RejectCodeOrderExceedsLimit           = C.OpenPitPretradeRejectCode_OrderExceedsLimit
	RejectCodeOrderQtyExceedsLimit        = C.OpenPitPretradeRejectCode_OrderQtyExceedsLimit
	RejectCodeOrderNotionalExceedsLimit   = C.OpenPitPretradeRejectCode_OrderNotionalExceedsLimit
	RejectCodePositionLimitExceeded       = C.OpenPitPretradeRejectCode_PositionLimitExceeded
	RejectCodeConcentrationLimitExceeded  = C.OpenPitPretradeRejectCode_ConcentrationLimitExceeded
	RejectCodeLeverageLimitExceeded       = C.OpenPitPretradeRejectCode_LeverageLimitExceeded
	RejectCodeRateLimitExceeded           = C.OpenPitPretradeRejectCode_RateLimitExceeded
	RejectCodePnlKillSwitchTriggered      = C.OpenPitPretradeRejectCode_PnlKillSwitchTriggered
	RejectCodeAccountBlocked              = C.OpenPitPretradeRejectCode_AccountBlocked
	RejectCodeAccountNotAuthorized        = C.OpenPitPretradeRejectCode_AccountNotAuthorized
	RejectCodeComplianceRestriction       = C.OpenPitPretradeRejectCode_ComplianceRestriction
	RejectCodeInstrumentRestricted        = C.OpenPitPretradeRejectCode_InstrumentRestricted
	RejectCodeJurisdictionRestriction     = C.OpenPitPretradeRejectCode_JurisdictionRestriction
	RejectCodeWashTradePrevention         = C.OpenPitPretradeRejectCode_WashTradePrevention
	RejectCodeSelfMatchPrevention         = C.OpenPitPretradeRejectCode_SelfMatchPrevention
	RejectCodeShortSaleRestriction        = C.OpenPitPretradeRejectCode_ShortSaleRestriction
	RejectCodeRiskConfigurationMissing    = C.OpenPitPretradeRejectCode_RiskConfigurationMissing
	RejectCodeReferenceDataUnavailable    = C.OpenPitPretradeRejectCode_ReferenceDataUnavailable
	RejectCodeOrderValueCalculationFailed = C.OpenPitPretradeRejectCode_OrderValueCalculationFailed
	RejectCodeSystemUnavailable           = C.OpenPitPretradeRejectCode_SystemUnavailable
	RejectCodeCustom                      = C.OpenPitPretradeRejectCode_Custom
	RejectCodeOther                       = C.OpenPitPretradeRejectCode_Other
)
