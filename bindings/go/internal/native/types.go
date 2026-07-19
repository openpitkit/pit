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
type AccountAdjustmentAccountPnlOperation = C.OpenPitAccountAdjustmentAccountPnlOperation
type AccountAdjustmentBatchError = *C.OpenPitAccountAdjustmentBatchError
type AccountAdjustmentBounds = C.OpenPitAccountAdjustmentBounds
type AccountAdjustmentBoundsOptional = C.OpenPitAccountAdjustmentBoundsOptional
type AccountAdjustmentContext = *C.OpenPitAccountAdjustmentContext
type AccountAdjustmentOperation = C.OpenPitAccountAdjustmentOperation
type AccountAdjustmentOperationKind = C.OpenPitAccountAdjustmentOperationKind
type AccountAdjustmentPositionOperation = C.OpenPitAccountAdjustmentPositionOperation
type AccountControl = *C.OpenPitAccountControl
type Engine = *C.OpenPitEngine
type EngineBuildError = *C.OpenPitEngineBuildError
type EngineBuildErrorCode = C.OpenPitEngineBuildErrorCode
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
type ParamMonetaryAmount = C.OpenPitParamMonetaryAmount
type ParamMonetaryAmountOptional = C.OpenPitParamMonetaryAmountOptional
type ParamNotional = C.OpenPitParamNotional
type ParamNotionalOptional = C.OpenPitParamNotionalOptional
type ParamPnl = C.OpenPitParamPnl
type ParamPnlOptional = C.OpenPitParamPnlOptional
type PnlState = C.OpenPitPnlState
type PnlStateOptional = C.OpenPitPnlStateOptional
type PnlStateKind = C.OpenPitPnlStateKind
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
type ParamFillType = C.OpenPitParamFillType
type ParamKind = C.OpenPitParamKind
type ParamRoundingStrategy = C.uint8_t
type ParamSide = C.OpenPitParamSide
type PretradePoliciesSpotFundsLimitMode = C.OpenPitPretradePoliciesSpotFundsLimitMode
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
type PretradePoliciesPnlBoundsAccountBarrierUpdate = C.OpenPitPretradePoliciesPnlBoundsAccountBarrierUpdate
type PretradePoliciesPnlBoundsBarrier = C.OpenPitPretradePoliciesPnlBoundsBarrier
type PretradePoliciesSpotFundsPnlBoundsAccountBarrier = C.OpenPitPretradePoliciesSpotFundsPnlBoundsAccountBarrier
type PretradePoliciesSpotFundsPnlBoundsAccountGroupBarrier = C.OpenPitPretradePoliciesSpotFundsPnlBoundsAccountGroupBarrier
type PretradePoliciesSpotFundsPnlBoundsBarrier = C.OpenPitPretradePoliciesSpotFundsPnlBoundsBarrier
type PretradePoliciesSpotFundsOverride = C.OpenPitPretradePoliciesSpotFundsOverride
type PretradePoliciesRateLimitAccountAssetBarrier = C.OpenPitPretradePoliciesRateLimitAccountAssetBarrier
type PretradePoliciesRateLimitAccountBarrier = C.OpenPitPretradePoliciesRateLimitAccountBarrier
type PretradePoliciesRateLimitAssetBarrier = C.OpenPitPretradePoliciesRateLimitAssetBarrier
type PretradePoliciesRateLimitBrokerBarrier = C.OpenPitPretradePoliciesRateLimitBrokerBarrier
type PretradePreTradeLock = *C.OpenPitPretradePreTradeLock
type PretradePreTradeLockPrices = *C.OpenPitPretradePreTradeLockPrices
type PretradePreTradeLockPricesStatus = C.OpenPitPretradePreTradeLockPricesStatus
type PretradePreTradeLockPricesView = C.OpenPitPretradePreTradeLockPricesView
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
type SharedBytes = *C.OpenPitSharedBytes
type BytesView = C.OpenPitBytesView
type PolicyGroupID = C.uint16_t
type ParamAccountGroupID = C.OpenPitParamAccountGroupId
type PostTradeContext = *C.OpenPitPostTradeContext
type AccountGroupError = *C.OpenPitAccountGroupError
type AccountBlockError = *C.OpenPitAccountBlockError
type AccountBlockErrorKind = C.OpenPitAccountBlockErrorKind

type ConfigureError = *C.OpenPitConfigureError
type ConfigureErrorKind = C.OpenPitConfigureErrorKind

type MarketDataService = *C.OpenPitMarketDataService
type MarketDataQuote = C.OpenPitMarketDataQuote
type MarketDataQuoteTTL = C.OpenPitMarketDataQuoteTtl
type InstrumentID = C.OpenPitInstrumentId
type MarketDataInstrumentID = InstrumentID
type MarketDataGetStatus = C.OpenPitMarketDataGetStatus
type MarketDataRegisterStatus = C.OpenPitMarketDataRegisterStatus
type MarketDataQuoteResolution = C.OpenPitMarketDataQuoteResolution

type ReferenceBook = *C.OpenPitReferenceBook
type ReferenceBookRegisterStatus = C.OpenPitReferenceBookRegisterStatus
type ReferenceBookStatus = C.OpenPitReferenceBookStatus
type SettlementUnit = C.OpenPitSettlementUnit
type SettlementLag = C.OpenPitSettlementLag
type SettlementScheme = C.OpenPitSettlementScheme

type PretradePreTradeResult = *C.OpenPitPretradePreTradeResult
type PostTradeAdjustmentList = *C.OpenPitPostTradeAdjustmentList
type PostTradeAccountPnlList = *C.OpenPitPostTradeAccountPnlList
type PostTradeResult = *C.OpenPitPostTradeResult
type PretradeAccountAdjustmentResult = *C.OpenPitPretradeAccountAdjustmentResult
type AccountOutcomeEntry = C.OpenPitAccountOutcomeEntry
type OutcomeAmount = C.OpenPitOutcomeAmount
type OutcomeAmountOptional = C.OpenPitOutcomeAmountOptional
type PnlOutcomeAmount = C.OpenPitPnlOutcomeAmount
type PnlOutcomeAmountOptional = C.OpenPitPnlOutcomeAmountOptional
type PnlOutcome = C.OpenPitPnlOutcome
type PnlOutcomeOptional = C.OpenPitPnlOutcomeOptional
type AccountAdjustmentOutcome = C.OpenPitAccountAdjustmentOutcome
type AccountAdjustmentOutcomeList = *C.OpenPitAccountAdjustmentOutcomeList
type AccountPnlOutcome = C.OpenPitAccountPnlOutcome
type AccountPnlOutcomeList = *C.OpenPitAccountPnlOutcomeList
type PnlHaltReason = C.OpenPitPnlHaltReason

type PretradePreTradeLockEntry = C.OpenPitPretradePreTradeLockEntry
type PretradePreTradeLockEntries = *C.OpenPitPretradePreTradeLockEntries
type PretradePreTradeLockEntriesView = C.OpenPitPretradePreTradeLockEntriesView

type PretradePreTradeDryRunReport = *C.OpenPitPretradePreTradeDryRunReport

const ParamLeverageNotSet = C.OPENPIT_PARAM_LEVERAGE_NOT_SET

const DefaultPolicyGroupID PolicyGroupID = C.OPENPIT_DEFAULT_POLICY_GROUP_ID

const DefaultAccountGroup ParamAccountGroupID = C.OPENPIT_DEFAULT_ACCOUNT_GROUP

const (
	PnlHaltReasonNone                   PnlHaltReason = C.OPENPIT_PNL_HALT_REASON_NONE
	PnlHaltReasonMissingFx              PnlHaltReason = C.OPENPIT_PNL_HALT_REASON_MISSING_FX
	PnlHaltReasonMissingAccountCurrency PnlHaltReason = C.OPENPIT_PNL_HALT_REASON_MISSING_ACCOUNT_CURRENCY
	PnlHaltReasonMissingInitialPnl      PnlHaltReason = C.OPENPIT_PNL_HALT_REASON_MISSING_INITIAL_PNL
	PnlHaltReasonMissingCostBasis       PnlHaltReason = C.OPENPIT_PNL_HALT_REASON_MISSING_COST_BASIS
	PnlHaltReasonArithmeticOverflow     PnlHaltReason = C.OPENPIT_PNL_HALT_REASON_ARITHMETIC_OVERFLOW
)

const (
	PnlStateKindValue  PnlStateKind = C.OPENPIT_PNL_STATE_VALUE
	PnlStateKindHalted PnlStateKind = C.OPENPIT_PNL_STATE_HALTED
)

const (
	AccountBlockErrorKindReservedGroup     = C.OpenPitAccountBlockErrorKind_ReservedGroup
	AccountBlockErrorKindAccountNotBlocked = C.OpenPitAccountBlockErrorKind_AccountNotBlocked
	AccountBlockErrorKindGroupNotBlocked   = C.OpenPitAccountBlockErrorKind_GroupNotBlocked
)

const (
	SettlementUnitBusinessDays SettlementUnit = C.OPENPIT_SETTLEMENT_UNIT_BUSINESS_DAYS
	SettlementUnitCalendarDays SettlementUnit = C.OPENPIT_SETTLEMENT_UNIT_CALENDAR_DAYS
)

const (
	ReferenceBookRegisterStatusOK                  ReferenceBookRegisterStatus = C.OpenPitReferenceBookRegisterStatus_Ok
	ReferenceBookRegisterStatusDuplicateID         ReferenceBookRegisterStatus = C.OpenPitReferenceBookRegisterStatus_DuplicateId
	ReferenceBookRegisterStatusDuplicateInstrument ReferenceBookRegisterStatus = C.OpenPitReferenceBookRegisterStatus_DuplicateInstrument
	ReferenceBookRegisterStatusError               ReferenceBookRegisterStatus = C.OpenPitReferenceBookRegisterStatus_Error
)

const (
	ReferenceBookStatusOK                ReferenceBookStatus = C.OpenPitReferenceBookStatus_Ok
	ReferenceBookStatusUnknownInstrument ReferenceBookStatus = C.OpenPitReferenceBookStatus_UnknownInstrument
	ReferenceBookStatusError             ReferenceBookStatus = C.OpenPitReferenceBookStatus_Error
)

const (
	ConfigureErrorKindUnknown             ConfigureErrorKind = C.OpenPitConfigureErrorKind_Unknown
	ConfigureErrorKindTypeMismatch        ConfigureErrorKind = C.OpenPitConfigureErrorKind_TypeMismatch
	ConfigureErrorKindValidation          ConfigureErrorKind = C.OpenPitConfigureErrorKind_Validation
	ConfigureErrorKindNestedConfiguration ConfigureErrorKind = C.OpenPitConfigureErrorKind_NestedConfiguration
)

const (
	PretradePreTradeLockPricesStatusError PretradePreTradeLockPricesStatus = C.OpenPitPretradePreTradeLockPricesStatus_Error
	PretradePreTradeLockPricesStatusEmpty PretradePreTradeLockPricesStatus = C.OpenPitPretradePreTradeLockPricesStatus_Empty
	PretradePreTradeLockPricesStatusOne   PretradePreTradeLockPricesStatus = C.OpenPitPretradePreTradeLockPricesStatus_One
	PretradePreTradeLockPricesStatusList  PretradePreTradeLockPricesStatus = C.OpenPitPretradePreTradeLockPricesStatus_List
)

const (
	MarketDataGetStatusFound             MarketDataGetStatus = C.OpenPitMarketDataGetStatus_Found
	MarketDataGetStatusUnavailable       MarketDataGetStatus = C.OpenPitMarketDataGetStatus_Unavailable
	MarketDataGetStatusUnknownInstrument MarketDataGetStatus = C.OpenPitMarketDataGetStatus_UnknownInstrument
	MarketDataGetStatusQuoteExpired      MarketDataGetStatus = C.OpenPitMarketDataGetStatus_QuoteExpired
	MarketDataGetStatusError             MarketDataGetStatus = C.OpenPitMarketDataGetStatus_Error
)

const (
	AccountAdjustmentOperationKindAbsent     AccountAdjustmentOperationKind = C.OPENPIT_ACCOUNT_ADJUSTMENT_OPERATION_KIND_ABSENT
	AccountAdjustmentOperationKindBalance    AccountAdjustmentOperationKind = C.OPENPIT_ACCOUNT_ADJUSTMENT_OPERATION_KIND_BALANCE
	AccountAdjustmentOperationKindPosition   AccountAdjustmentOperationKind = C.OPENPIT_ACCOUNT_ADJUSTMENT_OPERATION_KIND_POSITION
	AccountAdjustmentOperationKindAccountPnl AccountAdjustmentOperationKind = C.OPENPIT_ACCOUNT_ADJUSTMENT_OPERATION_KIND_ACCOUNT_PNL
)

const (
	EngineBuildErrorCodeDuplicatePolicyName    EngineBuildErrorCode = C.OpenPitEngineBuildErrorCode_DuplicatePolicyName
	EngineBuildErrorCodeDuplicatePolicyGroupID EngineBuildErrorCode = C.OpenPitEngineBuildErrorCode_DuplicatePolicyGroupId
	EngineBuildErrorCodeOther                  EngineBuildErrorCode = C.OpenPitEngineBuildErrorCode_Other
)

const (
	MarketDataRegisterStatusOk                  MarketDataRegisterStatus = C.OpenPitMarketDataRegisterStatus_Ok
	MarketDataRegisterStatusAlreadyRegistered   MarketDataRegisterStatus = C.OpenPitMarketDataRegisterStatus_AlreadyRegistered
	MarketDataRegisterStatusDuplicateID         MarketDataRegisterStatus = C.OpenPitMarketDataRegisterStatus_DuplicateId
	MarketDataRegisterStatusDuplicateInstrument MarketDataRegisterStatus = C.OpenPitMarketDataRegisterStatus_DuplicateInstrument
	MarketDataRegisterStatusUnknownInstrument   MarketDataRegisterStatus = C.OpenPitMarketDataRegisterStatus_UnknownInstrument
	MarketDataRegisterStatusError               MarketDataRegisterStatus = C.OpenPitMarketDataRegisterStatus_Error
	MarketDataRegisterStatusNoTarget            MarketDataRegisterStatus = C.OpenPitMarketDataRegisterStatus_NoTarget
)

const (
	MarketDataQuoteResolutionAccountOnly                 MarketDataQuoteResolution = C.OPENPIT_MARKET_DATA_QUOTE_RESOLUTION_ACCOUNT_ONLY
	MarketDataQuoteResolutionAccountThenGroup            MarketDataQuoteResolution = C.OPENPIT_MARKET_DATA_QUOTE_RESOLUTION_ACCOUNT_THEN_GROUP
	MarketDataQuoteResolutionAccountThenGroupThenDefault MarketDataQuoteResolution = C.OPENPIT_MARKET_DATA_QUOTE_RESOLUTION_ACCOUNT_THEN_GROUP_THEN_DEFAULT
)

const (
	ParamLeverageScale = C.OPENPIT_PARAM_LEVERAGE_SCALE
	ParamLeverageMin   = C.OPENPIT_PARAM_LEVERAGE_MIN
	ParamLeverageMax   = C.OPENPIT_PARAM_LEVERAGE_MAX
	ParamLeverageStep  = C.OPENPIT_PARAM_LEVERAGE_STEP
)

const (
	ParamSideNotSet = C.OPENPIT_PARAM_SIDE_NOT_SET
	ParamSideBuy    = C.OPENPIT_PARAM_SIDE_BUY
	ParamSideSell   = C.OPENPIT_PARAM_SIDE_SELL
)

const (
	ParamPositionSideNotSet = C.OPENPIT_PARAM_POSITION_SIDE_NOT_SET
	ParamPositionSideLong   = C.OPENPIT_PARAM_POSITION_SIDE_LONG
	ParamPositionSideShort  = C.OPENPIT_PARAM_POSITION_SIDE_SHORT
)

const (
	ParamPositionModeNotSet  = C.OPENPIT_PARAM_POSITION_MODE_NOT_SET
	ParamPositionModeNetting = C.OPENPIT_PARAM_POSITION_MODE_NETTING
	ParamPositionModeHedged  = C.OPENPIT_PARAM_POSITION_MODE_HEDGED
)

const (
	ParamPositionEffectNotSet = C.OPENPIT_PARAM_POSITION_EFFECT_NOT_SET
	ParamPositionEffectOpen   = C.OPENPIT_PARAM_POSITION_EFFECT_OPEN
	ParamPositionEffectClose  = C.OPENPIT_PARAM_POSITION_EFFECT_CLOSE
)

const (
	ParamTradeAmountKindNotSet   = C.OPENPIT_PARAM_TRADE_AMOUNT_KIND_NOT_SET
	ParamTradeAmountKindQuantity = C.OPENPIT_PARAM_TRADE_AMOUNT_KIND_QUANTITY
	ParamTradeAmountKindVolume   = C.OPENPIT_PARAM_TRADE_AMOUNT_KIND_VOLUME
)

const (
	ParamRoundingStrategyMidpointNearestEven  = C.OPENPIT_PARAM_ROUNDING_STRATEGY_MIDPOINT_NEAREST_EVEN
	ParamRoundingStrategyMidpointAwayFromZero = C.OPENPIT_PARAM_ROUNDING_STRATEGY_MIDPOINT_AWAY_FROM_ZERO
	ParamRoundingStrategyUp                   = C.OPENPIT_PARAM_ROUNDING_STRATEGY_UP
	ParamRoundingStrategyDown                 = C.OPENPIT_PARAM_ROUNDING_STRATEGY_DOWN
)

const (
	ParamFillTypeNotSet         = C.OPENPIT_PARAM_FILL_TYPE_NOT_SET
	ParamFillTypeTrade          = C.OPENPIT_PARAM_FILL_TYPE_TRADE
	ParamFillTypeLiquidation    = C.OPENPIT_PARAM_FILL_TYPE_LIQUIDATION
	ParamFillTypeAutoDeleverage = C.OPENPIT_PARAM_FILL_TYPE_AUTO_DELEVERAGE
	ParamFillTypeSettlement     = C.OPENPIT_PARAM_FILL_TYPE_SETTLEMENT
	ParamFillTypeFunding        = C.OPENPIT_PARAM_FILL_TYPE_FUNDING
)

const (
	ParamKindUnspecified  = C.OPENPIT_PARAM_KIND_UNSPECIFIED
	ParamKindQuantity     = C.OPENPIT_PARAM_KIND_QUANTITY
	ParamKindVolume       = C.OPENPIT_PARAM_KIND_VOLUME
	ParamKindNotional     = C.OPENPIT_PARAM_KIND_NOTIONAL
	ParamKindPrice        = C.OPENPIT_PARAM_KIND_PRICE
	ParamKindPnl          = C.OPENPIT_PARAM_KIND_PNL
	ParamKindCashFlow     = C.OPENPIT_PARAM_KIND_CASH_FLOW
	ParamKindPositionSize = C.OPENPIT_PARAM_KIND_POSITION_SIZE
	ParamKindFee          = C.OPENPIT_PARAM_KIND_FEE
	ParamKindLeverage     = C.OPENPIT_PARAM_KIND_LEVERAGE
)

const (
	PretradePoliciesSpotFundsLimitModeEnforce   = C.OPENPIT_PRETRADE_POLICIES_SPOT_FUNDS_LIMIT_MODE_ENFORCE
	PretradePoliciesSpotFundsLimitModeTrackOnly = C.OPENPIT_PRETRADE_POLICIES_SPOT_FUNDS_LIMIT_MODE_TRACK_ONLY
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
	TriBoolNotSet = C.OPENPIT_TRI_BOOL_NOT_SET
	TriBoolFalse  = C.OPENPIT_TRI_BOOL_FALSE
	TriBoolTrue   = C.OPENPIT_TRI_BOOL_TRUE
)

const (
	ParamAdjustmentAmountKindNotSet   = C.OPENPIT_PARAM_ADJUSTMENT_AMOUNT_KIND_NOT_SET
	ParamAdjustmentAmountKindDelta    = C.OPENPIT_PARAM_ADJUSTMENT_AMOUNT_KIND_DELTA
	ParamAdjustmentAmountKindAbsolute = C.OPENPIT_PARAM_ADJUSTMENT_AMOUNT_KIND_ABSOLUTE
)

const (
	RejectScopeOrder   = C.OPENPIT_PRETRADE_REJECT_SCOPE_ORDER
	RejectScopeAccount = C.OPENPIT_PRETRADE_REJECT_SCOPE_ACCOUNT
)

const (
	RejectCodeMissingRequiredField            = C.OPENPIT_PRETRADE_REJECT_CODE_MISSING_REQUIRED_FIELD
	RejectCodeInvalidFieldFormat              = C.OPENPIT_PRETRADE_REJECT_CODE_INVALID_FIELD_FORMAT
	RejectCodeInvalidFieldValue               = C.OPENPIT_PRETRADE_REJECT_CODE_INVALID_FIELD_VALUE
	RejectCodeUnsupportedOrderType            = C.OPENPIT_PRETRADE_REJECT_CODE_UNSUPPORTED_ORDER_TYPE
	RejectCodeUnsupportedTimeInForce          = C.OPENPIT_PRETRADE_REJECT_CODE_UNSUPPORTED_TIME_IN_FORCE
	RejectCodeUnsupportedOrderAttribute       = C.OPENPIT_PRETRADE_REJECT_CODE_UNSUPPORTED_ORDER_ATTRIBUTE
	RejectCodeDuplicateClientOrderID          = C.OPENPIT_PRETRADE_REJECT_CODE_DUPLICATE_CLIENT_ORDER_ID
	RejectCodeTooLateToEnter                  = C.OPENPIT_PRETRADE_REJECT_CODE_TOO_LATE_TO_ENTER
	RejectCodeExchangeClosed                  = C.OPENPIT_PRETRADE_REJECT_CODE_EXCHANGE_CLOSED
	RejectCodeUnknownInstrument               = C.OPENPIT_PRETRADE_REJECT_CODE_UNKNOWN_INSTRUMENT
	RejectCodeUnknownAccount                  = C.OPENPIT_PRETRADE_REJECT_CODE_UNKNOWN_ACCOUNT
	RejectCodeUnknownVenue                    = C.OPENPIT_PRETRADE_REJECT_CODE_UNKNOWN_VENUE
	RejectCodeUnknownClearingAccount          = C.OPENPIT_PRETRADE_REJECT_CODE_UNKNOWN_CLEARING_ACCOUNT
	RejectCodeUnknownCollateralAsset          = C.OPENPIT_PRETRADE_REJECT_CODE_UNKNOWN_COLLATERAL_ASSET
	RejectCodeInsufficientFunds               = C.OPENPIT_PRETRADE_REJECT_CODE_INSUFFICIENT_FUNDS
	RejectCodeInsufficientMargin              = C.OPENPIT_PRETRADE_REJECT_CODE_INSUFFICIENT_MARGIN
	RejectCodeInsufficientPosition            = C.OPENPIT_PRETRADE_REJECT_CODE_INSUFFICIENT_POSITION
	RejectCodeCreditLimitExceeded             = C.OPENPIT_PRETRADE_REJECT_CODE_CREDIT_LIMIT_EXCEEDED
	RejectCodeRiskLimitExceeded               = C.OPENPIT_PRETRADE_REJECT_CODE_RISK_LIMIT_EXCEEDED
	RejectCodeOrderExceedsLimit               = C.OPENPIT_PRETRADE_REJECT_CODE_ORDER_EXCEEDS_LIMIT
	RejectCodeOrderQtyExceedsLimit            = C.OPENPIT_PRETRADE_REJECT_CODE_ORDER_QTY_EXCEEDS_LIMIT
	RejectCodeOrderNotionalExceedsLimit       = C.OPENPIT_PRETRADE_REJECT_CODE_ORDER_NOTIONAL_EXCEEDS_LIMIT
	RejectCodePositionLimitExceeded           = C.OPENPIT_PRETRADE_REJECT_CODE_POSITION_LIMIT_EXCEEDED
	RejectCodeConcentrationLimitExceeded      = C.OPENPIT_PRETRADE_REJECT_CODE_CONCENTRATION_LIMIT_EXCEEDED
	RejectCodeLeverageLimitExceeded           = C.OPENPIT_PRETRADE_REJECT_CODE_LEVERAGE_LIMIT_EXCEEDED
	RejectCodeRateLimitExceeded               = C.OPENPIT_PRETRADE_REJECT_CODE_RATE_LIMIT_EXCEEDED
	RejectCodePnlKillSwitchTriggered          = C.OPENPIT_PRETRADE_REJECT_CODE_PNL_KILL_SWITCH_TRIGGERED
	RejectCodeAccountBlocked                  = C.OPENPIT_PRETRADE_REJECT_CODE_ACCOUNT_BLOCKED
	RejectCodeAccountNotAuthorized            = C.OPENPIT_PRETRADE_REJECT_CODE_ACCOUNT_NOT_AUTHORIZED
	RejectCodeComplianceRestriction           = C.OPENPIT_PRETRADE_REJECT_CODE_COMPLIANCE_RESTRICTION
	RejectCodeInstrumentRestricted            = C.OPENPIT_PRETRADE_REJECT_CODE_INSTRUMENT_RESTRICTED
	RejectCodeJurisdictionRestriction         = C.OPENPIT_PRETRADE_REJECT_CODE_JURISDICTION_RESTRICTION
	RejectCodeWashTradePrevention             = C.OPENPIT_PRETRADE_REJECT_CODE_WASH_TRADE_PREVENTION
	RejectCodeSelfMatchPrevention             = C.OPENPIT_PRETRADE_REJECT_CODE_SELF_MATCH_PREVENTION
	RejectCodeShortSaleRestriction            = C.OPENPIT_PRETRADE_REJECT_CODE_SHORT_SALE_RESTRICTION
	RejectCodeRiskConfigurationMissing        = C.OPENPIT_PRETRADE_REJECT_CODE_RISK_CONFIGURATION_MISSING
	RejectCodeReferenceDataUnavailable        = C.OPENPIT_PRETRADE_REJECT_CODE_REFERENCE_DATA_UNAVAILABLE
	RejectCodeOrderValueCalculationFailed     = C.OPENPIT_PRETRADE_REJECT_CODE_ORDER_VALUE_CALCULATION_FAILED
	RejectCodeSystemUnavailable               = C.OPENPIT_PRETRADE_REJECT_CODE_SYSTEM_UNAVAILABLE
	RejectCodeMarkPriceUnavailable            = C.OPENPIT_PRETRADE_REJECT_CODE_MARK_PRICE_UNAVAILABLE
	RejectCodeAccountAdjustmentBoundsExceeded = C.OPENPIT_PRETRADE_REJECT_CODE_ACCOUNT_ADJUSTMENT_BOUNDS_EXCEEDED
	RejectCodeArithmeticOverflow              = C.OPENPIT_PRETRADE_REJECT_CODE_ARITHMETIC_OVERFLOW
	RejectCodeCustom                          = C.OPENPIT_PRETRADE_REJECT_CODE_CUSTOM
	RejectCodeOther                           = C.OPENPIT_PRETRADE_REJECT_CODE_OTHER
)
