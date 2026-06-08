/*
 * Copyright The Pit Project Owners. All rights reserved.
 * SPDX-License-Identifier: Apache-2.0
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing,
 * software distributed under the License is distributed on an
 * "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND,
 * either express or implied. See the License for the specific
 * language governing permissions and limitations under the
 * License.
 *
 * Please see https://openpit.dev and the OWNERS file for details.
 *
 * Generated file. Do not edit manually.
 */

#ifndef OPENPIT_H
#define OPENPIT_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct OpenPitAccountAdjustment OpenPitAccountAdjustment;
typedef struct OpenPitAccountAdjustmentAmount OpenPitAccountAdjustmentAmount;
typedef struct OpenPitAccountAdjustmentAmountOptional
    OpenPitAccountAdjustmentAmountOptional;
typedef struct OpenPitAccountAdjustmentBalanceOperation
    OpenPitAccountAdjustmentBalanceOperation;
typedef struct OpenPitAccountAdjustmentBatchError
    OpenPitAccountAdjustmentBatchError;
typedef struct OpenPitAccountAdjustmentBounds OpenPitAccountAdjustmentBounds;
typedef struct OpenPitAccountAdjustmentBoundsOptional
    OpenPitAccountAdjustmentBoundsOptional;
typedef struct OpenPitAccountAdjustmentContext OpenPitAccountAdjustmentContext;
typedef struct OpenPitAccountAdjustmentOperation
    OpenPitAccountAdjustmentOperation;
typedef struct OpenPitAccountAdjustmentOutcome OpenPitAccountAdjustmentOutcome;
typedef struct OpenPitAccountAdjustmentOutcomeList
    OpenPitAccountAdjustmentOutcomeList;
typedef struct OpenPitAccountAdjustmentPositionOperation
    OpenPitAccountAdjustmentPositionOperation;
typedef struct OpenPitAccountBlockError OpenPitAccountBlockError;
typedef struct OpenPitAccountControl OpenPitAccountControl;
typedef struct OpenPitAccountGroupError OpenPitAccountGroupError;
typedef struct OpenPitAccountOutcomeEntry OpenPitAccountOutcomeEntry;
typedef struct OpenPitAccountOutcomeEntryList OpenPitAccountOutcomeEntryList;
typedef struct OpenPitBytesView OpenPitBytesView;
typedef struct OpenPitEngine OpenPitEngine;
typedef struct OpenPitEngineBuildError OpenPitEngineBuildError;
typedef struct OpenPitEngineBuilder OpenPitEngineBuilder;
typedef struct OpenPitExecutionReport OpenPitExecutionReport;
typedef struct OpenPitExecutionReportFill OpenPitExecutionReportFill;
typedef struct OpenPitExecutionReportFillOptional
    OpenPitExecutionReportFillOptional;
typedef struct OpenPitExecutionReportIsFinalOptional
    OpenPitExecutionReportIsFinalOptional;
typedef struct OpenPitExecutionReportOperation OpenPitExecutionReportOperation;
typedef struct OpenPitExecutionReportOperationOptional
    OpenPitExecutionReportOperationOptional;
typedef struct OpenPitExecutionReportPositionImpact
    OpenPitExecutionReportPositionImpact;
typedef struct OpenPitExecutionReportPositionImpactOptional
    OpenPitExecutionReportPositionImpactOptional;
typedef struct OpenPitExecutionReportTrade OpenPitExecutionReportTrade;
typedef struct OpenPitExecutionReportTradeOptional
    OpenPitExecutionReportTradeOptional;
typedef struct OpenPitFinancialImpact OpenPitFinancialImpact;
typedef struct OpenPitFinancialImpactOptional OpenPitFinancialImpactOptional;
typedef struct OpenPitInstrument OpenPitInstrument;
typedef struct OpenPitMarketDataQuote OpenPitMarketDataQuote;
typedef struct OpenPitMarketDataQuoteTtl OpenPitMarketDataQuoteTtl;
typedef struct OpenPitMarketDataService OpenPitMarketDataService;
typedef struct OpenPitMutations OpenPitMutations;
typedef struct OpenPitOrder OpenPitOrder;
typedef struct OpenPitOrderMargin OpenPitOrderMargin;
typedef struct OpenPitOrderMarginOptional OpenPitOrderMarginOptional;
typedef struct OpenPitOrderOperation OpenPitOrderOperation;
typedef struct OpenPitOrderOperationOptional OpenPitOrderOperationOptional;
typedef struct OpenPitOrderPosition OpenPitOrderPosition;
typedef struct OpenPitOrderPositionOptional OpenPitOrderPositionOptional;
typedef struct OpenPitOutcomeAmount OpenPitOutcomeAmount;
typedef struct OpenPitOutcomeAmountOptional OpenPitOutcomeAmountOptional;
typedef struct OpenPitParamAccountGroupIdOptional
    OpenPitParamAccountGroupIdOptional;
typedef struct OpenPitParamAccountIdOptional OpenPitParamAccountIdOptional;
typedef struct OpenPitParamAdjustmentAmount OpenPitParamAdjustmentAmount;
typedef struct OpenPitParamCashFlow OpenPitParamCashFlow;
typedef struct OpenPitParamCashFlowOptional OpenPitParamCashFlowOptional;
typedef struct OpenPitParamDecimal OpenPitParamDecimal;
typedef struct OpenPitParamError OpenPitParamError;
typedef struct OpenPitParamFee OpenPitParamFee;
typedef struct OpenPitParamFeeOptional OpenPitParamFeeOptional;
typedef struct OpenPitParamNotional OpenPitParamNotional;
typedef struct OpenPitParamNotionalOptional OpenPitParamNotionalOptional;
typedef struct OpenPitParamPnl OpenPitParamPnl;
typedef struct OpenPitParamPnlOptional OpenPitParamPnlOptional;
typedef struct OpenPitParamPositionSize OpenPitParamPositionSize;
typedef struct OpenPitParamPositionSizeOptional
    OpenPitParamPositionSizeOptional;
typedef struct OpenPitParamPrice OpenPitParamPrice;
typedef struct OpenPitParamPriceOptional OpenPitParamPriceOptional;
typedef struct OpenPitParamQuantity OpenPitParamQuantity;
typedef struct OpenPitParamQuantityOptional OpenPitParamQuantityOptional;
typedef struct OpenPitParamTradeAmount OpenPitParamTradeAmount;
typedef struct OpenPitParamVolume OpenPitParamVolume;
typedef struct OpenPitParamVolumeOptional OpenPitParamVolumeOptional;
typedef struct OpenPitPostTradeAdjustmentList OpenPitPostTradeAdjustmentList;
typedef struct OpenPitPostTradeContext OpenPitPostTradeContext;
typedef struct OpenPitPretradeAccountBlock OpenPitPretradeAccountBlock;
typedef struct OpenPitPretradeAccountBlockList OpenPitPretradeAccountBlockList;
typedef struct OpenPitPretradeContext OpenPitPretradeContext;
typedef struct OpenPitPretradePoliciesOrderSizeAccountAssetBarrier
    OpenPitPretradePoliciesOrderSizeAccountAssetBarrier;
typedef struct OpenPitPretradePoliciesOrderSizeAssetBarrier
    OpenPitPretradePoliciesOrderSizeAssetBarrier;
typedef struct OpenPitPretradePoliciesOrderSizeBrokerBarrier
    OpenPitPretradePoliciesOrderSizeBrokerBarrier;
typedef struct OpenPitPretradePoliciesOrderSizeLimit
    OpenPitPretradePoliciesOrderSizeLimit;
typedef struct OpenPitPretradePoliciesPnlBoundsAccountBarrier
    OpenPitPretradePoliciesPnlBoundsAccountBarrier;
typedef struct OpenPitPretradePoliciesPnlBoundsBarrier
    OpenPitPretradePoliciesPnlBoundsBarrier;
typedef struct OpenPitPretradePoliciesRateLimitAccountAssetBarrier
    OpenPitPretradePoliciesRateLimitAccountAssetBarrier;
typedef struct OpenPitPretradePoliciesRateLimitAccountBarrier
    OpenPitPretradePoliciesRateLimitAccountBarrier;
typedef struct OpenPitPretradePoliciesRateLimitAssetBarrier
    OpenPitPretradePoliciesRateLimitAssetBarrier;
typedef struct OpenPitPretradePoliciesRateLimitBrokerBarrier
    OpenPitPretradePoliciesRateLimitBrokerBarrier;
typedef struct OpenPitPretradePoliciesSpotFundsOverride
    OpenPitPretradePoliciesSpotFundsOverride;
typedef struct OpenPitPretradePreTradeLock OpenPitPretradePreTradeLock;
typedef struct OpenPitPretradePreTradeLockEntries
    OpenPitPretradePreTradeLockEntries;
typedef struct OpenPitPretradePreTradeLockEntriesView
    OpenPitPretradePreTradeLockEntriesView;
typedef struct OpenPitPretradePreTradeLockEntry
    OpenPitPretradePreTradeLockEntry;
typedef struct OpenPitPretradePreTradeLockPrices
    OpenPitPretradePreTradeLockPrices;
typedef struct OpenPitPretradePreTradeLockPricesView
    OpenPitPretradePreTradeLockPricesView;
typedef struct OpenPitPretradePreTradePolicy OpenPitPretradePreTradePolicy;
typedef struct OpenPitPretradePreTradeRequest OpenPitPretradePreTradeRequest;
typedef struct OpenPitPretradePreTradeReservation
    OpenPitPretradePreTradeReservation;
typedef struct OpenPitPretradePreTradeResult OpenPitPretradePreTradeResult;
typedef struct OpenPitPretradeReject OpenPitPretradeReject;
typedef struct OpenPitPretradeRejectList OpenPitPretradeRejectList;
typedef struct OpenPitSharedBytes OpenPitSharedBytes;
typedef struct OpenPitSharedString OpenPitSharedString;
typedef struct OpenPitStringView OpenPitStringView;

/**
 * Leverage multiplier for FFI payloads.
 *
 * Uses fixed-point scale `10` in integer units:
 * - `10` means `1.0x`
 * - `11` means `1.1x`
 * - `1005` means `100.5x`
 *
 * Valid range: `10..=30000`.
 *
 * A value of `OPENPIT_PARAM_LEVERAGE_NOT_SET` (`0`) means leverage is not
 * specified.
 */
typedef uint16_t OpenPitParamLeverage;

/**
 * Stable account identifier type for FFI payloads.
 *
 * WARNING: Use exactly one account-id source model per runtime:
 * - either purely numeric IDs
 *   (`openpit_create_param_account_id_from_uint64`),
 * - or purely string-derived IDs
 *   (`openpit_create_param_account_id_from_string`).
 *
 * Do not mix both models in the same runtime state. A hashed string value can
 * coincide with a direct numeric ID, and then two distinct accounts become one
 * logical key in maps and engine state.
 */
typedef uint64_t OpenPitParamAccountId;

/**
 * Error out-pointer used by fallible FFI calls.
 */
typedef OpenPitSharedString ** OpenPitOutError;

/**
 * Parameter error out-pointer used by fallible param FFI calls.
 */
typedef OpenPitParamError ** OpenPitOutParamError;

/**
 * Stable account-group identifier type for FFI payloads.
 *
 * WARNING: Use exactly one account-group-id source model per runtime:
 * - either purely numeric IDs
 *   (`openpit_create_param_account_group_id_from_uint32`),
 * - or purely string-derived IDs
 *   (`openpit_create_param_account_group_id_from_string`).
 *
 * Do not mix both models in the same runtime state. A hashed string value can
 * coincide with a direct numeric ID, collapsing two distinct groups into one
 * key.
 */
typedef uint32_t OpenPitParamAccountGroupId;

/**
 * Stable instrument identifier for FFI payloads.
 */
typedef uint64_t OpenPitMarketDataInstrumentId;

/**
 * Sentinel value indicating leverage is not set.
 */
#define OPENPIT_PARAM_LEVERAGE_NOT_SET ((OpenPitParamLeverage) 0)

/**
 * Fixed-point scale used by leverage payloads.
 */
#define OPENPIT_PARAM_LEVERAGE_SCALE ((OpenPitParamLeverage) 10)

/**
 * Minimum leverage in whole units.
 */
#define OPENPIT_PARAM_LEVERAGE_MIN ((OpenPitParamLeverage) 1)

/**
 * Maximum leverage in whole units.
 */
#define OPENPIT_PARAM_LEVERAGE_MAX ((OpenPitParamLeverage) 3000)

/**
 * Supported leverage increment step.
 */
#define OPENPIT_PARAM_LEVERAGE_STEP ((float) 0.1)

/**
 * Default rounding strategy alias.
 */
#define OPENPIT_PARAM_ROUNDING_STRATEGY_DEFAULT \
    ((OpenPitParamRoundingStrategy) OpenPitParamRoundingStrategy_MidpointNearestEven)

/**
 * Banker's rounding alias.
 */
#define OPENPIT_PARAM_ROUNDING_STRATEGY_BANKER \
    ((OpenPitParamRoundingStrategy) OpenPitParamRoundingStrategy_MidpointNearestEven)

/**
 * Conservative profit rounding alias.
 */
#define OPENPIT_PARAM_ROUNDING_STRATEGY_CONSERVATIVE_PROFIT \
    ((OpenPitParamRoundingStrategy) OpenPitParamRoundingStrategy_Down)

/**
 * Conservative loss rounding alias.
 */
#define OPENPIT_PARAM_ROUNDING_STRATEGY_CONSERVATIVE_LOSS \
    ((OpenPitParamRoundingStrategy) OpenPitParamRoundingStrategy_Down)

/**
 * The reserved default account-group identifier. Every account belongs to this
 * group until it is registered into another one, so no constructor may produce
 * it. Mirrors `openpit::param::DEFAULT_ACCOUNT_GROUP`.
 */
#define OPENPIT_DEFAULT_ACCOUNT_GROUP ((OpenPitParamAccountGroupId) 0)

/**
 * The default policy-group identifier used when a caller does not assign a
 * policy to a specific group. Mirrors `openpit::DEFAULT_POLICY_GROUP_ID`.
 */
#define OPENPIT_DEFAULT_POLICY_GROUP_ID ((uint16_t) 0)

/**
 * Order side.
 */
typedef uint8_t OpenPitParamSide;
/**
 * Value is absent.
 */
#define OpenPitParamSide_NotSet ((OpenPitParamSide) 0)
/**
 * Buy side.
 */
#define OpenPitParamSide_Buy ((OpenPitParamSide) 1)
/**
 * Sell side.
 */
#define OpenPitParamSide_Sell ((OpenPitParamSide) 2)

/**
 * Position direction.
 */
typedef uint8_t OpenPitParamPositionSide;
/**
 * Value is absent.
 */
#define OpenPitParamPositionSide_NotSet ((OpenPitParamPositionSide) 0)
/**
 * Long exposure.
 */
#define OpenPitParamPositionSide_Long ((OpenPitParamPositionSide) 1)
/**
 * Short exposure.
 */
#define OpenPitParamPositionSide_Short ((OpenPitParamPositionSide) 2)

/**
 * Position accounting mode.
 */
typedef uint8_t OpenPitParamPositionMode;
/**
 * Value is absent.
 */
#define OpenPitParamPositionMode_NotSet ((OpenPitParamPositionMode) 0)
/**
 * Opposite trades net into one position.
 */
#define OpenPitParamPositionMode_Netting ((OpenPitParamPositionMode) 1)
/**
 * Long and short positions are tracked separately.
 */
#define OpenPitParamPositionMode_Hedged ((OpenPitParamPositionMode) 2)

/**
 * Whether a trade opens or closes exposure.
 */
typedef uint8_t OpenPitParamPositionEffect;
/**
 * Value is absent.
 */
#define OpenPitParamPositionEffect_NotSet ((OpenPitParamPositionEffect) 0)
/**
 * The trade opens or increases exposure.
 */
#define OpenPitParamPositionEffect_Open ((OpenPitParamPositionEffect) 1)
/**
 * The trade closes or reduces exposure.
 */
#define OpenPitParamPositionEffect_Close ((OpenPitParamPositionEffect) 2)

/**
 * Selects how one trade-amount numeric value should be interpreted.
 */
typedef uint8_t OpenPitParamTradeAmountKind;
/**
 * No amount field is selected.
 */
#define OpenPitParamTradeAmountKind_NotSet ((OpenPitParamTradeAmountKind) 0)
/**
 * The value is instrument quantity.
 */
#define OpenPitParamTradeAmountKind_Quantity ((OpenPitParamTradeAmountKind) 1)
/**
 * The value is settlement volume.
 */
#define OpenPitParamTradeAmountKind_Volume ((OpenPitParamTradeAmountKind) 2)

/**
 * Decimal rounding strategy for typed parameter constructors.
 */
typedef uint8_t OpenPitParamRoundingStrategy;
/**
 * Round half to nearest even number.
 */
#define OpenPitParamRoundingStrategy_MidpointNearestEven \
    ((OpenPitParamRoundingStrategy) 0)
/**
 * Round half away from zero.
 */
#define OpenPitParamRoundingStrategy_MidpointAwayFromZero \
    ((OpenPitParamRoundingStrategy) 1)
/**
 * Round towards positive infinity.
 */
#define OpenPitParamRoundingStrategy_Up ((OpenPitParamRoundingStrategy) 2)
/**
 * Round towards negative infinity.
 */
#define OpenPitParamRoundingStrategy_Down ((OpenPitParamRoundingStrategy) 3)

/**
 * Tri-state boolean value.
 */
typedef uint8_t OpenPitTriBool;
/**
 * Value is absent.
 */
#define OpenPitTriBool_NotSet ((OpenPitTriBool) 0)
/**
 * Boolean false.
 */
#define OpenPitTriBool_False ((OpenPitTriBool) 1)
/**
 * Boolean true.
 */
#define OpenPitTriBool_True ((OpenPitTriBool) 2)

/**
 * Selects how an account-adjustment amount should be interpreted.
 */
typedef uint8_t OpenPitParamAdjustmentAmountKind;
/**
 * No amount is specified.
 */
#define OpenPitParamAdjustmentAmountKind_NotSet \
    ((OpenPitParamAdjustmentAmountKind) 0)
/**
 * Change current state by the supplied signed amount.
 */
#define OpenPitParamAdjustmentAmountKind_Delta \
    ((OpenPitParamAdjustmentAmountKind) 1)
/**
 * Set current state to the supplied signed amount.
 */
#define OpenPitParamAdjustmentAmountKind_Absolute \
    ((OpenPitParamAdjustmentAmountKind) 2)

/**
 * Selects which account-adjustment operation payload is present.
 *
 * At most one operation payload can be selected at a time:
 * - `Absent` means no operation is supplied;
 * - `Balance` selects the balance-operation payload;
 * - `Position` selects the position-operation payload.
 */
typedef uint8_t OpenPitAccountAdjustmentOperationKind;
/**
 * No operation is supplied.
 */
#define OpenPitAccountAdjustmentOperationKind_Absent \
    ((OpenPitAccountAdjustmentOperationKind) 0)
/**
 * The balance-operation payload is selected.
 */
#define OpenPitAccountAdjustmentOperationKind_Balance \
    ((OpenPitAccountAdjustmentOperationKind) 1)
/**
 * The position-operation payload is selected.
 */
#define OpenPitAccountAdjustmentOperationKind_Position \
    ((OpenPitAccountAdjustmentOperationKind) 2)

/**
 * Result of `openpit_engine_apply_account_adjustment`.
 */
typedef uint8_t OpenPitAccountAdjustmentApplyStatus;
/**
 * The call failed before the batch could be evaluated.
 */
#define OpenPitAccountAdjustmentApplyStatus_Error \
    ((OpenPitAccountAdjustmentApplyStatus) 0)
/**
 * The batch was accepted and applied.
 */
#define OpenPitAccountAdjustmentApplyStatus_Applied \
    ((OpenPitAccountAdjustmentApplyStatus) 1)
/**
 * The batch was evaluated and rejected by policy or validation logic.
 */
#define OpenPitAccountAdjustmentApplyStatus_Rejected \
    ((OpenPitAccountAdjustmentApplyStatus) 2)

/**
 * Broad area to which a reject applies.
 *
 * Valid values: `Order` (1), `Account` (2). Zero is not a valid scope value;
 * the caller must always set this field explicitly.
 */
typedef uint8_t OpenPitPretradeRejectScope;
/**
 * The reject applies to one order or order-like request.
 */
#define OpenPitPretradeRejectScope_Order ((OpenPitPretradeRejectScope) 1)
/**
 * The reject applies to account state rather than to one order only.
 */
#define OpenPitPretradeRejectScope_Account ((OpenPitPretradeRejectScope) 2)

/**
 * Stable classification code for a reject.
 *
 * Read this first when you need machine-readable handling. The textual fields
 * in [`OpenPitPretradeReject`] provide operator-facing explanation and extra
 * context.
 *
 * Valid codes are `1..=42` and `255` (`Other`). Unknown incoming codes are
 * mapped to `Other` (`255`).
 */
typedef uint16_t OpenPitPretradeRejectCode;
/**
 * A required field is absent.
 */
#define OpenPitPretradeRejectCode_MissingRequiredField \
    ((OpenPitPretradeRejectCode) 1)
/**
 * A field cannot be parsed from the supplied wire value.
 */
#define OpenPitPretradeRejectCode_InvalidFieldFormat \
    ((OpenPitPretradeRejectCode) 2)
/**
 * A field is syntactically valid but semantically unacceptable.
 */
#define OpenPitPretradeRejectCode_InvalidFieldValue \
    ((OpenPitPretradeRejectCode) 3)
/**
 * The requested order type is not supported.
 */
#define OpenPitPretradeRejectCode_UnsupportedOrderType \
    ((OpenPitPretradeRejectCode) 4)
/**
 * The requested time-in-force is not supported.
 */
#define OpenPitPretradeRejectCode_UnsupportedTimeInForce \
    ((OpenPitPretradeRejectCode) 5)
/**
 * Another order attribute is unsupported.
 */
#define OpenPitPretradeRejectCode_UnsupportedOrderAttribute \
    ((OpenPitPretradeRejectCode) 6)
/**
 * The client order identifier duplicates an active order.
 */
#define OpenPitPretradeRejectCode_DuplicateClientOrderId \
    ((OpenPitPretradeRejectCode) 7)
/**
 * The order arrived after the allowed entry deadline.
 */
#define OpenPitPretradeRejectCode_TooLateToEnter ((OpenPitPretradeRejectCode) 8)
/**
 * Trading is closed for the relevant venue or session.
 */
#define OpenPitPretradeRejectCode_ExchangeClosed ((OpenPitPretradeRejectCode) 9)
/**
 * The instrument cannot be resolved.
 */
#define OpenPitPretradeRejectCode_UnknownInstrument \
    ((OpenPitPretradeRejectCode) 10)
/**
 * The account cannot be resolved.
 */
#define OpenPitPretradeRejectCode_UnknownAccount \
    ((OpenPitPretradeRejectCode) 11)
/**
 * The venue cannot be resolved.
 */
#define OpenPitPretradeRejectCode_UnknownVenue ((OpenPitPretradeRejectCode) 12)
/**
 * The clearing account cannot be resolved.
 */
#define OpenPitPretradeRejectCode_UnknownClearingAccount \
    ((OpenPitPretradeRejectCode) 13)
/**
 * The collateral asset cannot be resolved.
 */
#define OpenPitPretradeRejectCode_UnknownCollateralAsset \
    ((OpenPitPretradeRejectCode) 14)
/**
 * Available balance is insufficient.
 */
#define OpenPitPretradeRejectCode_InsufficientFunds \
    ((OpenPitPretradeRejectCode) 15)
/**
 * Available margin is insufficient.
 */
#define OpenPitPretradeRejectCode_InsufficientMargin \
    ((OpenPitPretradeRejectCode) 16)
/**
 * Available position is insufficient.
 */
#define OpenPitPretradeRejectCode_InsufficientPosition \
    ((OpenPitPretradeRejectCode) 17)
/**
 * A credit limit was exceeded.
 */
#define OpenPitPretradeRejectCode_CreditLimitExceeded \
    ((OpenPitPretradeRejectCode) 18)
/**
 * A risk limit was exceeded.
 */
#define OpenPitPretradeRejectCode_RiskLimitExceeded \
    ((OpenPitPretradeRejectCode) 19)
/**
 * The order exceeds a generic configured limit.
 */
#define OpenPitPretradeRejectCode_OrderExceedsLimit \
    ((OpenPitPretradeRejectCode) 20)
/**
 * The order quantity exceeds a configured limit.
 */
#define OpenPitPretradeRejectCode_OrderQtyExceedsLimit \
    ((OpenPitPretradeRejectCode) 21)
/**
 * The order notional exceeds a configured limit.
 */
#define OpenPitPretradeRejectCode_OrderNotionalExceedsLimit \
    ((OpenPitPretradeRejectCode) 22)
/**
 * The resulting position exceeds a configured limit.
 */
#define OpenPitPretradeRejectCode_PositionLimitExceeded \
    ((OpenPitPretradeRejectCode) 23)
/**
 * Concentration constraints were violated.
 */
#define OpenPitPretradeRejectCode_ConcentrationLimitExceeded \
    ((OpenPitPretradeRejectCode) 24)
/**
 * Leverage constraints were violated.
 */
#define OpenPitPretradeRejectCode_LeverageLimitExceeded \
    ((OpenPitPretradeRejectCode) 25)
/**
 * The request rate exceeded a configured limit.
 */
#define OpenPitPretradeRejectCode_RateLimitExceeded \
    ((OpenPitPretradeRejectCode) 26)
/**
 * A loss barrier has blocked further risk-taking.
 */
#define OpenPitPretradeRejectCode_PnlKillSwitchTriggered \
    ((OpenPitPretradeRejectCode) 27)
/**
 * The account is blocked.
 */
#define OpenPitPretradeRejectCode_AccountBlocked \
    ((OpenPitPretradeRejectCode) 28)
/**
 * The account is not authorized for this action.
 */
#define OpenPitPretradeRejectCode_AccountNotAuthorized \
    ((OpenPitPretradeRejectCode) 29)
/**
 * A compliance restriction blocked the action.
 */
#define OpenPitPretradeRejectCode_ComplianceRestriction \
    ((OpenPitPretradeRejectCode) 30)
/**
 * The instrument is restricted.
 */
#define OpenPitPretradeRejectCode_InstrumentRestricted \
    ((OpenPitPretradeRejectCode) 31)
/**
 * A jurisdiction restriction blocked the action.
 */
#define OpenPitPretradeRejectCode_JurisdictionRestriction \
    ((OpenPitPretradeRejectCode) 32)
/**
 * The action would violate wash-trade prevention.
 */
#define OpenPitPretradeRejectCode_WashTradePrevention \
    ((OpenPitPretradeRejectCode) 33)
/**
 * The action would violate self-match prevention.
 */
#define OpenPitPretradeRejectCode_SelfMatchPrevention \
    ((OpenPitPretradeRejectCode) 34)
/**
 * Short-sale restriction blocked the action.
 */
#define OpenPitPretradeRejectCode_ShortSaleRestriction \
    ((OpenPitPretradeRejectCode) 35)
/**
 * Required risk configuration is missing.
 */
#define OpenPitPretradeRejectCode_RiskConfigurationMissing \
    ((OpenPitPretradeRejectCode) 36)
/**
 * Required reference data is unavailable.
 */
#define OpenPitPretradeRejectCode_ReferenceDataUnavailable \
    ((OpenPitPretradeRejectCode) 37)
/**
 * The system could not compute an order value needed for validation.
 */
#define OpenPitPretradeRejectCode_OrderValueCalculationFailed \
    ((OpenPitPretradeRejectCode) 38)
/**
 * A required service or subsystem is unavailable.
 */
#define OpenPitPretradeRejectCode_SystemUnavailable \
    ((OpenPitPretradeRejectCode) 39)
/**
 * Required mark price is unavailable.
 */
#define OpenPitPretradeRejectCode_MarkPriceUnavailable \
    ((OpenPitPretradeRejectCode) 40)
/**
 * Account adjustment would violate configured bounds.
 */
#define OpenPitPretradeRejectCode_AccountAdjustmentBoundsExceeded \
    ((OpenPitPretradeRejectCode) 41)
/**
 * Underlying decimal arithmetic overflowed during evaluation.
 */
#define OpenPitPretradeRejectCode_ArithmeticOverflow \
    ((OpenPitPretradeRejectCode) 42)
/**
 * Reserved discriminant for caller-defined reject classes.
 *
 * Use together with `Reject::with_user_data` to attach a caller-defined
 * payload that the receiving code can decode. The SDK does not interpret this
 * code beyond mapping it to FFI value 254.
 */
#define OpenPitPretradeRejectCode_Custom ((OpenPitPretradeRejectCode) 254)
/**
 * A catch-all code for rejects that do not fit a more specific class.
 */
#define OpenPitPretradeRejectCode_Other ((OpenPitPretradeRejectCode) 255)

/**
 * Parameter error code transported through FFI.
 */
typedef uint32_t OpenPitParamErrorCode;
/**
 * Error code is not specified.
 */
#define OpenPitParamErrorCode_Unspecified ((OpenPitParamErrorCode) 0)
/**
 * Value must be non-negative.
 */
#define OpenPitParamErrorCode_Negative ((OpenPitParamErrorCode) 1)
/**
 * Division by zero.
 */
#define OpenPitParamErrorCode_DivisionByZero ((OpenPitParamErrorCode) 2)
/**
 * Arithmetic overflow.
 */
#define OpenPitParamErrorCode_Overflow ((OpenPitParamErrorCode) 3)
/**
 * Arithmetic underflow.
 */
#define OpenPitParamErrorCode_Underflow ((OpenPitParamErrorCode) 4)
/**
 * Invalid float value.
 */
#define OpenPitParamErrorCode_InvalidFloat ((OpenPitParamErrorCode) 5)
/**
 * Invalid textual format.
 */
#define OpenPitParamErrorCode_InvalidFormat ((OpenPitParamErrorCode) 6)
/**
 * Invalid price value.
 */
#define OpenPitParamErrorCode_InvalidPrice ((OpenPitParamErrorCode) 7)
/**
 * Invalid leverage value.
 */
#define OpenPitParamErrorCode_InvalidLeverage ((OpenPitParamErrorCode) 8)
/**
 * Asset identifier is empty.
 */
#define OpenPitParamErrorCode_AssetEmpty ((OpenPitParamErrorCode) 9)
/**
 * Account identifier string is empty.
 */
#define OpenPitParamErrorCode_AccountIdEmpty ((OpenPitParamErrorCode) 10)
/**
 * Catch-all code for unknown cases.
 */
#define OpenPitParamErrorCode_Other ((OpenPitParamErrorCode) 4294967295)

/**
 * Runtime selector for the engine's storage synchronization policy.
 */
typedef uint8_t OpenPitSyncPolicy;
/**
 * The handle stays on the OS thread that created it. Use this for
 * single-threaded embeddings where synchronization overhead must be zero.
 */
#define OpenPitSyncPolicy_None ((OpenPitSyncPolicy) 0)
/**
 * Concurrent invocation of public methods on the same handle is safe.
 * Sequential cross-thread access is also safe. Use this when the engine is
 * shared across threads.
 */
#define OpenPitSyncPolicy_Full ((OpenPitSyncPolicy) 1)
/**
 * Sequential cross-thread access on the same handle is safe; the caller pins
 * each account to a single processing chain (one queue or one worker at a
 * time). Concurrent invocation on the same handle is not supported in this
 * mode.
 */
#define OpenPitSyncPolicy_Account ((OpenPitSyncPolicy) 2)

/**
 * Result status for pre-trade operations.
 */
typedef uint8_t OpenPitPretradeStatus;
/**
 * Order/request passed this stage; read the success out-pointer.
 */
#define OpenPitPretradeStatus_Passed ((OpenPitPretradeStatus) 0)
/**
 * Order/request was rejected; read the reject out-pointer.
 */
#define OpenPitPretradeStatus_Rejected ((OpenPitPretradeStatus) 1)
/**
 * Call failed due to invalid input; read the error out-pointer.
 */
#define OpenPitPretradeStatus_Error ((OpenPitPretradeStatus) 2)

/**
 * Machine-readable discriminant describing why building an engine failed.
 *
 * Each value identifies a distinct failure category. There is no success
 * value: a build-error object exists only when a build did not produce an
 * engine.
 */
typedef uint8_t OpenPitEngineBuildErrorCode;
/**
 * Two or more registered policies declare the same name.
 */
#define OpenPitEngineBuildErrorCode_DuplicatePolicyName \
    ((OpenPitEngineBuildErrorCode) 0)
/**
 * Two or more registered policies declare the same non-default group id.
 */
#define OpenPitEngineBuildErrorCode_DuplicatePolicyGroupId \
    ((OpenPitEngineBuildErrorCode) 1)
/**
 * A failure category not covered by the above. Forward-compatible catch-all;
 * no structured payload is available.
 */
#define OpenPitEngineBuildErrorCode_Other ((OpenPitEngineBuildErrorCode) 2)

/**
 * Discriminant for the variant carried by an [`OpenPitAccountBlockError`].
 */
typedef uint32_t OpenPitAccountBlockErrorKind;
/**
 * The target group is the reserved default account group.
 */
#define OpenPitAccountBlockErrorKind_ReservedGroup \
    ((OpenPitAccountBlockErrorKind) 0)
/**
 * The target account is not currently blocked.
 */
#define OpenPitAccountBlockErrorKind_AccountNotBlocked \
    ((OpenPitAccountBlockErrorKind) 1)
/**
 * The target account group is not currently blocked.
 */
#define OpenPitAccountBlockErrorKind_GroupNotBlocked \
    ((OpenPitAccountBlockErrorKind) 2)

/**
 * Selects which quote buckets a read will consult, in order.
 *
 * When the more-specific bucket has no quote, the read falls through to the
 * next bucket permitted by this value.
 */
typedef uint8_t OpenPitMarketDataQuoteResolution;
/**
 * Consult only the per-account bucket for the reading account.
 */
#define OpenPitMarketDataQuoteResolution_AccountOnly \
    ((OpenPitMarketDataQuoteResolution) 0)
/**
 * Consult the per-account bucket, then fall back to the account's group bucket
 * when the account bucket has no quote.
 */
#define OpenPitMarketDataQuoteResolution_AccountThenGroup \
    ((OpenPitMarketDataQuoteResolution) 1)
/**
 * Consult the per-account bucket, then the account's group bucket, then the
 * default-group ("everyone-else") bucket, in that order.
 */
#define OpenPitMarketDataQuoteResolution_AccountThenGroupThenDefault \
    ((OpenPitMarketDataQuoteResolution) 2)

/**
 * Result of a market-data read.
 */
typedef uint8_t OpenPitMarketDataGetStatus;
/**
 * A usable quote was found; `out_quote` was written.
 */
#define OpenPitMarketDataGetStatus_Found ((OpenPitMarketDataGetStatus) 0)
/**
 * The instrument is registered but no usable quote is available (never pushed,
 * cleared, or aged past its TTL).
 */
#define OpenPitMarketDataGetStatus_Unavailable ((OpenPitMarketDataGetStatus) 1)
/**
 * The instrument id is not registered with the service.
 */
#define OpenPitMarketDataGetStatus_UnknownInstrument \
    ((OpenPitMarketDataGetStatus) 2)

/**
 * Result of a market-data registration or update.
 *
 * Each operation returns only the subset of variants it can produce; see the
 * per-function contract for the variants it may report.
 */
typedef uint8_t OpenPitMarketDataRegisterStatus;
/**
 * The operation succeeded; any associated output was written.
 */
#define OpenPitMarketDataRegisterStatus_Ok ((OpenPitMarketDataRegisterStatus) 0)
/**
 * The instrument is already registered with the service.
 */
#define OpenPitMarketDataRegisterStatus_AlreadyRegistered \
    ((OpenPitMarketDataRegisterStatus) 1)
/**
 * The supplied id is already registered with the service.
 */
#define OpenPitMarketDataRegisterStatus_DuplicateId \
    ((OpenPitMarketDataRegisterStatus) 2)
/**
 * The supplied instrument is already registered under a different id.
 */
#define OpenPitMarketDataRegisterStatus_DuplicateInstrument \
    ((OpenPitMarketDataRegisterStatus) 3)
/**
 * The supplied instrument id is not registered with the service.
 */
#define OpenPitMarketDataRegisterStatus_UnknownInstrument \
    ((OpenPitMarketDataRegisterStatus) 4)
/**
 * A boundary failure occurred (null pointer or an invalid payload); when
 * `out_error` is not null, a caller-owned error string was written.
 */
#define OpenPitMarketDataRegisterStatus_Error \
    ((OpenPitMarketDataRegisterStatus) 5)
/**
 * A targeted push (`push_for` / `push_for_patch`) was called with both the
 * account list and the group list empty.
 */
#define OpenPitMarketDataRegisterStatus_NoTarget \
    ((OpenPitMarketDataRegisterStatus) 6)

typedef uint8_t OpenPitPretradePreTradeLockPricesStatus;
#define OpenPitPretradePreTradeLockPricesStatus_Error \
    ((OpenPitPretradePreTradeLockPricesStatus) 0)
#define OpenPitPretradePreTradeLockPricesStatus_Empty \
    ((OpenPitPretradePreTradeLockPricesStatus) 1)
#define OpenPitPretradePreTradeLockPricesStatus_One \
    ((OpenPitPretradePreTradeLockPricesStatus) 2)
#define OpenPitPretradePreTradeLockPricesStatus_List \
    ((OpenPitPretradePreTradeLockPricesStatus) 3)

/**
 * Decimal value represented as `mantissa * 10^-scale`.
 */
struct OpenPitParamDecimal {
    /**
     * Lower 64 bits of the i128 mantissa.
     */
    int64_t mantissa_lo;
    /**
     * Upper 64 bits of the i128 mantissa (sign-extended).
     */
    int64_t mantissa_hi;
    /**
     * Decimal scale.
     */
    int32_t scale;
};

/**
 * Validated `Pnl` value wrapper.
 */
struct OpenPitParamPnl {
    OpenPitParamDecimal _0;
};

/**
 * Validated `Price` value wrapper.
 */
struct OpenPitParamPrice {
    OpenPitParamDecimal _0;
};

/**
 * Validated `Quantity` value wrapper.
 */
struct OpenPitParamQuantity {
    OpenPitParamDecimal _0;
};

/**
 * Validated `Volume` value wrapper.
 */
struct OpenPitParamVolume {
    OpenPitParamDecimal _0;
};

/**
 * Validated `CashFlow` value wrapper.
 */
struct OpenPitParamCashFlow {
    OpenPitParamDecimal _0;
};

/**
 * Validated `PositionSize` value wrapper.
 */
struct OpenPitParamPositionSize {
    OpenPitParamDecimal _0;
};

/**
 * Validated `Fee` value wrapper.
 */
struct OpenPitParamFee {
    OpenPitParamDecimal _0;
};

/**
 * Validated `Notional` value wrapper.
 */
struct OpenPitParamNotional {
    OpenPitParamDecimal _0;
};

struct OpenPitParamNotionalOptional {
    OpenPitParamNotional value;
    bool is_set;
};

struct OpenPitParamPnlOptional {
    OpenPitParamPnl value;
    bool is_set;
};

struct OpenPitParamPriceOptional {
    OpenPitParamPrice value;
    bool is_set;
};

struct OpenPitParamQuantityOptional {
    OpenPitParamQuantity value;
    bool is_set;
};

struct OpenPitParamVolumeOptional {
    OpenPitParamVolume value;
    bool is_set;
};

struct OpenPitParamCashFlowOptional {
    OpenPitParamCashFlow value;
    bool is_set;
};

struct OpenPitParamPositionSizeOptional {
    OpenPitParamPositionSize value;
    bool is_set;
};

struct OpenPitParamFeeOptional {
    OpenPitParamFee value;
    bool is_set;
};

struct OpenPitParamAccountIdOptional {
    OpenPitParamAccountId value;
    bool is_set;
};

/**
 * Optional position-management group for an order.
 *
 * The group is absent when every field is `NotSet`.
 */
struct OpenPitOrderPosition {
    /**
     * Optional long/short side.
     */
    OpenPitParamPositionSide position_side;
    /**
     * Reduce-only flag.
     */
    OpenPitTriBool reduce_only;
    /**
     * Close-position flag.
     */
    OpenPitTriBool close_position;
};

struct OpenPitOrderPositionOptional {
    OpenPitOrderPosition value;
    bool is_set;
};

/**
 * Populated financial-impact group for an execution report.
 */
struct OpenPitFinancialImpact {
    /**
     * Profit-and-loss contribution carried by this report.
     */
    OpenPitParamPnlOptional pnl;
    /**
     * Fee or rebate contribution carried by this report.
     */
    OpenPitParamFeeOptional fee;
};

/**
 * Fill trade payload (`price + quantity`) for execution reports.
 */
struct OpenPitExecutionReportTrade {
    /**
     * Trade price.
     */
    OpenPitParamPrice price;
    /**
     * Trade quantity.
     */
    OpenPitParamQuantity quantity;
};

/**
 * Populated position-impact group for an execution report.
 */
struct OpenPitExecutionReportPositionImpact {
    /**
     * Whether exposure is opened or closed.
     */
    OpenPitParamPositionEffect position_effect;
    /**
     * Impacted side (long or short).
     */
    OpenPitParamPositionSide position_side;
};

struct OpenPitFinancialImpactOptional {
    OpenPitFinancialImpact value;
    bool is_set;
};

struct OpenPitExecutionReportTradeOptional {
    OpenPitExecutionReportTrade value;
    bool is_set;
};

struct OpenPitExecutionReportIsFinalOptional {
    bool value;
    bool is_set;
};

struct OpenPitExecutionReportPositionImpactOptional {
    OpenPitExecutionReportPositionImpact value;
    bool is_set;
};

/**
 * One amount component inside an account adjustment.
 *
 * The numeric value is interpreted according to `kind`:
 * - `Delta` means "change current state by this signed amount";
 * - `Absolute` means "set current state to this signed amount".
 */
struct OpenPitParamAdjustmentAmount {
    /**
     * Signed numeric value of the adjustment.
     */
    OpenPitParamPositionSize value;
    /**
     * Interpretation mode for `value`.
     */
    OpenPitParamAdjustmentAmountKind kind;
};

/**
 * Optional amount-change group for account adjustment.
 *
 * The group is absent when every field is absent.
 */
struct OpenPitAccountAdjustmentAmount {
    /**
     * Requested balance change.
     */
    OpenPitParamAdjustmentAmount balance;
    /**
     * Requested held-balance change.
     */
    OpenPitParamAdjustmentAmount held;
    /**
     * Requested incoming-balance change.
     */
    OpenPitParamAdjustmentAmount incoming;
};

/**
 * Optional bounds group for account adjustment.
 *
 * The group is absent when every bound is absent.
 */
struct OpenPitAccountAdjustmentBounds {
    /**
     * Optional upper bound for balance.
     */
    OpenPitParamPositionSizeOptional balance_upper;
    /**
     * Optional lower bound for balance.
     */
    OpenPitParamPositionSizeOptional balance_lower;
    /**
     * Optional upper bound for held balance.
     */
    OpenPitParamPositionSizeOptional held_upper;
    /**
     * Optional lower bound for held balance.
     */
    OpenPitParamPositionSizeOptional held_lower;
    /**
     * Optional upper bound for incoming balance.
     */
    OpenPitParamPositionSizeOptional incoming_upper;
    /**
     * Optional lower bound for incoming balance.
     */
    OpenPitParamPositionSizeOptional incoming_lower;
};

struct OpenPitAccountAdjustmentAmountOptional {
    OpenPitAccountAdjustmentAmount value;
    bool is_set;
};

struct OpenPitAccountAdjustmentBoundsOptional {
    OpenPitAccountAdjustmentBounds value;
    bool is_set;
};

/**
 * Caller-owned parameter error container.
 */
struct OpenPitParamError {
    /**
     * Stable machine-readable error code.
     */
    OpenPitParamErrorCode code;
    /**
     * Human-readable message allocated as shared string.
     */
    OpenPitSharedString * message;
};

/**
 * Shared order-size limits for
 * `openpit_engine_builder_add_builtin_order_size_limit_policy`.
 */
struct OpenPitPretradePoliciesOrderSizeLimit {
    /**
     * Maximum allowed quantity for one order.
     */
    OpenPitParamQuantity max_quantity;
    /**
     * Maximum allowed notional for one order.
     */
    OpenPitParamVolume max_notional;
};

/**
 * Broker-wide order-size barrier for
 * `openpit_engine_builder_add_builtin_order_size_limit_policy`.
 */
struct OpenPitPretradePoliciesOrderSizeBrokerBarrier {
    /**
     * Size limits for this broker barrier.
     */
    OpenPitPretradePoliciesOrderSizeLimit limit;
};

/**
 * Broker-wide rate-limit barrier for
 * `openpit_engine_builder_add_builtin_rate_limit_policy`.
 */
struct OpenPitPretradePoliciesRateLimitBrokerBarrier {
    /**
     * Maximum number of orders accepted within the window.
     */
    size_t max_orders;
    /**
     * Window duration in nanoseconds.
     */
    uint64_t window_nanoseconds;
};

/**
 * Per-account rate-limit barrier for
 * `openpit_engine_builder_add_builtin_rate_limit_policy`.
 */
struct OpenPitPretradePoliciesRateLimitAccountBarrier {
    /**
     * Account this barrier applies to.
     */
    OpenPitParamAccountId account_id;
    /**
     * Maximum number of orders accepted within the window.
     */
    size_t max_orders;
    /**
     * Window duration in nanoseconds.
     */
    uint64_t window_nanoseconds;
};

struct OpenPitParamAccountGroupIdOptional {
    OpenPitParamAccountGroupId value;
    bool is_set;
};

/**
 * A delta/absolute pair for one position field.
 */
struct OpenPitOutcomeAmount {
    /**
     * Signed change applied by this operation relative to the field value at
     * operation start. Authoritative for position bookkeeping.
     */
    OpenPitParamPositionSize delta;
    /**
     * Field value at the moment the policy returned, before deferred commit. Treat
     * as a convenience hint only; prefer `delta`.
     */
    OpenPitParamPositionSize absolute;
};

struct OpenPitOutcomeAmountOptional {
    OpenPitOutcomeAmount value;
    bool is_set;
};

/**
 * Non-owning byte slice view.
 *
 * Lifetime contract:
 * - `ptr` points to `len` readable bytes;
 * - the memory is valid while the original object is alive;
 * - the caller must not free or mutate memory behind `ptr`;
 * - if the caller needs to retain the bytes beyond that announced lifetime,
 *   the caller must copy them.
 */
struct OpenPitBytesView {
    const uint8_t * ptr;
    size_t len;
};

/**
 * Market snapshot value passed across the FFI boundary.
 *
 * Every field is optional (`is_set == false` means the producer did not
 * publish that field). Mirrors [`Quote`].
 */
struct OpenPitMarketDataQuote {
    /**
     * Mark price.
     */
    OpenPitParamPriceOptional mark;
    /**
     * Best-bid price.
     */
    OpenPitParamPriceOptional bid;
    /**
     * Best-ask price.
     */
    OpenPitParamPriceOptional ask;
};

/**
 * Service-wide / per-instrument quote lifetime for FFI payloads.
 *
 * `is_infinite == true` means quotes never expire on their own. Otherwise the
 * quote expires `secs` + `nanos` after the push that wrote it. Mirrors
 * [`QuoteTtl`].
 */
struct OpenPitMarketDataQuoteTtl {
    /**
     * Whole-seconds part of the finite lifetime (ignored when infinite).
     */
    uint64_t secs;
    /**
     * Sub-second nanoseconds part of the finite lifetime (ignored when infinite).
     */
    uint32_t nanos;
    /**
     * When `true`, quotes never expire on their own.
     */
    bool is_infinite;
};

struct OpenPitPretradePreTradeLockPricesView {
    const OpenPitParamPrice * ptr;
    size_t len;
};

/**
 * A single `(policy_group_id, price)` record exchanged across the C boundary.
 */
struct OpenPitPretradePreTradeLockEntry {
    uint16_t policy_group_id;
    OpenPitParamPrice price;
};

/**
 * Read-only view over a caller-owned lock entry snapshot.
 */
struct OpenPitPretradePreTradeLockEntriesView {
    const OpenPitPretradePreTradeLockEntry * ptr;
    size_t len;
};

/**
 * Non-owning UTF-8 string view.
 *
 * This type never owns memory. It borrows bytes from another object.
 *
 * Lifetime contract:
 * - `ptr` points to `len` readable bytes;
 * - the memory is valid while the original object is alive and the source
 *   string has not been modified;
 * - the caller must not free or mutate memory behind `ptr`.
 * - if the caller needs to retain the string beyond that announced lifetime,
 *   the caller must copy the bytes.
 */
struct OpenPitStringView {
    /**
     * Pointer to the first UTF-8 byte.
     */
    const uint8_t * ptr;
    /**
     * Number of bytes at `ptr`.
     */
    size_t len;
};

/**
 * One trade-amount value plus its interpretation mode.
 *
 * The numeric value is interpreted according to `kind`:
 * - `Quantity` means instrument quantity;
 * - `Volume` means settlement notional volume.
 */
struct OpenPitParamTradeAmount {
    /**
     * Non-negative numeric payload.
     */
    OpenPitParamDecimal value;
    /**
     * Interpretation mode for `value`.
     */
    OpenPitParamTradeAmountKind kind;
};

/**
 * Optional margin group for an order.
 *
 * The group is absent when every field is `NotSet`.
 */
struct OpenPitOrderMargin {
    /**
     * Optional collateral asset.
     */
    OpenPitStringView collateral_asset;
    /**
     * Auto-borrow flag.
     */
    OpenPitTriBool auto_borrow;
    /**
     * Optional leverage value.
     */
    OpenPitParamLeverage leverage;
};

struct OpenPitOrderMarginOptional {
    OpenPitOrderMargin value;
    bool is_set;
};

/**
 * Populated fill-details group for an execution report.
 */
struct OpenPitExecutionReportFill {
    /**
     * Optional latest trade payload.
     */
    OpenPitExecutionReportTradeOptional last_trade;
    /**
     * Remaining quantity after applying this report.
     */
    OpenPitParamQuantityOptional leaves_quantity;
    /**
     * Pre-trade lock attached to the order.
     *
     * Ownership contract:
     * - the caller owns the pointer when present (build it through
     *   `openpit_pretrade_lock_*` functions) and remains responsible for
     *   releasing it with `openpit_destroy_pretrade_pre_trade_lock`;
     * - null is equivalent to an empty lock; passing null does *not* mean
     *   "default group" - no record is created on import unless the caller
     *   supplied one through this pointer.
     */
    const OpenPitPretradePreTradeLock * lock;
    /**
     * Whether this report closes the order's report stream. The order is filled,
     * cancelled, or rejected.
     */
    OpenPitExecutionReportIsFinalOptional is_final;
};

struct OpenPitExecutionReportFillOptional {
    OpenPitExecutionReportFill value;
    bool is_set;
};

/**
 * Balance-operation payload for account adjustment.
 */
struct OpenPitAccountAdjustmentBalanceOperation {
    /**
     * Balance asset code.
     */
    OpenPitStringView asset;
    /**
     * Optional average entry price.
     */
    OpenPitParamPriceOptional average_entry_price;
};

/**
 * Single rejection record returned by checks.
 */
struct OpenPitPretradeReject {
    /**
     * Policy name that produced the reject.
     */
    OpenPitStringView policy;
    /**
     * Human-readable reject reason.
     */
    OpenPitStringView reason;
    /**
     * Case-specific reject details.
     */
    OpenPitStringView details;
    /**
     * Opaque caller-defined token.
     *
     * The SDK never inspects, dereferences, or frees this value. Its meaning,
     * lifetime, and thread-safety are the caller's responsibility. `0` / null
     * means "not set". See the project Threading Contract for the full lifetime
     * model.
     *
     * The token flows through every reject path the SDK exposes (start-stage,
     * main-stage, account-adjustment, batch results) and is preserved on `Clone`.
     */
    void * user_data;
    /**
     * Stable machine-readable reject code.
     */
    OpenPitPretradeRejectCode code;
    /**
     * Reject scope.
     */
    OpenPitPretradeRejectScope scope;
};

/**
 * Single account-block record returned by kill-switch checks.
 */
struct OpenPitPretradeAccountBlock {
    /**
     * Policy name that produced the block.
     */
    OpenPitStringView policy;
    /**
     * Human-readable reject reason.
     */
    OpenPitStringView reason;
    /**
     * Case-specific reject details.
     */
    OpenPitStringView details;
    /**
     * Opaque caller-defined token.
     *
     * The SDK never inspects, dereferences, or frees this value. Its meaning,
     * lifetime, and thread-safety are the caller's responsibility. `0` / null
     * means "not set". See the project Threading Contract for the full lifetime
     * model.
     */
    void * user_data;
    /**
     * Stable machine-readable reject code.
     */
    OpenPitPretradeRejectCode code;
};

/**
 * Per-settlement-asset order-size barrier for
 * `openpit_engine_builder_add_builtin_order_size_limit_policy`.
 */
struct OpenPitPretradePoliciesOrderSizeAssetBarrier {
    /**
     * Size limits for this asset barrier.
     */
    OpenPitPretradePoliciesOrderSizeLimit limit;
    /**
     * Settlement asset this barrier applies to.
     */
    OpenPitStringView settlement_asset;
};

/**
 * Per-(account, settlement-asset) order-size barrier for
 * `openpit_engine_builder_add_builtin_order_size_limit_policy`.
 */
struct OpenPitPretradePoliciesOrderSizeAccountAssetBarrier {
    /**
     * Size limits for this account+asset barrier.
     */
    OpenPitPretradePoliciesOrderSizeLimit limit;
    /**
     * Account this barrier applies to.
     */
    OpenPitParamAccountId account_id;
    /**
     * Settlement asset this barrier applies to.
     */
    OpenPitStringView settlement_asset;
};

/**
 * One broker barrier definition for
 * `openpit_engine_builder_add_builtin_pnl_bounds_killswitch_policy`.
 *
 * What it describes:
 * - A settlement asset and its lower/upper P&L bounds applied as a broker
 *   barrier across all accounts.
 *
 * Contract:
 * - `settlement_asset` must point to a valid string for the duration of the
 *   call.
 * - The array passed to the add function may contain multiple entries.
 */
struct OpenPitPretradePoliciesPnlBoundsBarrier {
    /**
     * Settlement asset whose accumulated P&L is being monitored.
     */
    OpenPitStringView settlement_asset;
    /**
     * Optional lower bound for accumulated P&L.
     */
    OpenPitParamPnlOptional lower_bound;
    /**
     * Optional upper bound for accumulated P&L.
     */
    OpenPitParamPnlOptional upper_bound;
};

/**
 * Per-(account, settlement-asset) P&L bounds barrier with an initial P&L seed.
 *
 * What it describes:
 * - Refines P&L bounds for a specific account and settlement asset.
 * - `initial_pnl` is pre-loaded into storage at construction; accumulation
 *   starts from this value.
 * - Both the broker barrier (if any) and this account+asset barrier are
 *   evaluated on every check; the order passes only if neither is breached.
 *
 * Passed to `openpit_engine_builder_add_builtin_pnl_bounds_killswitch_policy`
 * in the `account` array.
 */
struct OpenPitPretradePoliciesPnlBoundsAccountBarrier {
    /**
     * Account this barrier applies to.
     */
    OpenPitParamAccountId account_id;
    /**
     * Settlement asset whose accumulated P&L is being monitored.
     */
    OpenPitStringView settlement_asset;
    /**
     * Optional lower bound for accumulated P&L for this account+asset pair.
     */
    OpenPitParamPnlOptional lower_bound;
    /**
     * Optional upper bound for accumulated P&L for this account+asset pair.
     */
    OpenPitParamPnlOptional upper_bound;
    /**
     * Starting accumulated P&L pre-loaded into storage at construction.
     */
    OpenPitParamPnl initial_pnl;
};

/**
 * Per-settlement-asset rate-limit barrier for
 * `openpit_engine_builder_add_builtin_rate_limit_policy`.
 */
struct OpenPitPretradePoliciesRateLimitAssetBarrier {
    /**
     * Settlement asset this barrier applies to.
     */
    OpenPitStringView settlement_asset;
    /**
     * Maximum number of orders accepted within the window.
     */
    size_t max_orders;
    /**
     * Window duration in nanoseconds.
     */
    uint64_t window_nanoseconds;
};

/**
 * Per-(account, settlement-asset) rate-limit barrier for
 * `openpit_engine_builder_add_builtin_rate_limit_policy`.
 */
struct OpenPitPretradePoliciesRateLimitAccountAssetBarrier {
    /**
     * Account this barrier applies to.
     */
    OpenPitParamAccountId account_id;
    /**
     * Settlement asset this barrier applies to.
     */
    OpenPitStringView settlement_asset;
    /**
     * Maximum number of orders accepted within the window.
     */
    size_t max_orders;
    /**
     * Window duration in nanoseconds.
     */
    uint64_t window_nanoseconds;
};

/**
 * Slippage override entry for the spot funds policy.
 *
 * Mirrors [`SpotFundsOverride`](openpit::SpotFundsOverride) together with the
 * [`SpotFundsOverrideTarget`](openpit::SpotFundsOverrideTarget) it applies to.
 * `instrument_id` selects the registered instrument. The scope is chosen by
 * the `account_id` and `account_group_id` optionals, which are mutually
 * exclusive: when neither is set the entry is an instrument-level default;
 * when `account_id.is_set` it applies only to `account_id.value`; when
 * `account_group_id.is_set` it applies only to accounts in
 * `account_group_id.value`. When `has_slippage_bps` is `true`, `slippage_bps`
 * is the slippage for that scope; when `false`, the entry is ignored and the
 * cascade falls through to the next tier (ultimately the global
 * `market_slippage_bps`).
 *
 * Slippage resolves account -> account group -> instrument -> global for each
 * order.
 */
struct OpenPitPretradePoliciesSpotFundsOverride {
    /**
     * Registered market-data instrument id.
     */
    OpenPitMarketDataInstrumentId instrument_id;
    /**
     * Account the override applies to. When `is_set`, the override is scoped to
     * `value`; mutually exclusive with `account_group_id`. Both unset means an
     * instrument-level default.
     */
    OpenPitParamAccountIdOptional account_id;
    /**
     * Account group the override applies to. When `is_set`, the override is scoped
     * to `value`; mutually exclusive with `account_id`. Both unset means an
     * instrument-level default.
     */
    OpenPitParamAccountGroupIdOptional account_group_id;
    /**
     * Slippage in basis points for the selected scope, used only when
     * `has_slippage_bps` is `true`.
     */
    uint16_t slippage_bps;
    /**
     * Whether `slippage_bps` carries a value.
     */
    bool has_slippage_bps;
};

/**
 * Raw outcome data produced by a policy for one asset.
 */
struct OpenPitAccountOutcomeEntry {
    /**
     * Asset this outcome refers to.
     */
    OpenPitStringView asset;
    /**
     * Settled balance/position outcome.
     */
    OpenPitOutcomeAmountOptional balance;
    /**
     * Held (reserved) amount outcome.
     */
    OpenPitOutcomeAmountOptional held;
    /**
     * Incoming (pending inflow) amount outcome.
     */
    OpenPitOutcomeAmountOptional incoming;
};

/**
 * Account position outcome with the group tag of the business entity that
 * produced it.
 */
struct OpenPitAccountAdjustmentOutcome {
    /**
     * Policy-group tag of the policy that produced this outcome.
     */
    uint16_t policy_group_id;
    /**
     * Account adjustment outcome entry.
     */
    OpenPitAccountOutcomeEntry entry;
};

/**
 * Trading instrument view.
 *
 * The caller owns the memory referenced by both string views.
 *
 * Semantics:
 * - both string views set: instrument is present;
 * - both string views not set: instrument is absent;
 * - only one string view set: invalid payload.
 */
struct OpenPitInstrument {
    /**
     * Traded asset, for example `AAPL` or `BTC`.
     */
    OpenPitStringView underlying_asset;
    /**
     * Settlement asset, for example `USD`.
     */
    OpenPitStringView settlement_asset;
};

/**
 * Optional operation group for an order.
 *
 * The group is absent when all fields are absent.
 */
struct OpenPitOrderOperation {
    /**
     * Optional trade amount payload.
     */
    OpenPitParamTradeAmount trade_amount;
    /**
     * Trading instrument.
     */
    OpenPitInstrument instrument;
    /**
     * Optional limit price.
     */
    OpenPitParamPriceOptional price;
    /**
     * Optional account identifier.
     */
    OpenPitParamAccountIdOptional account_id;
    /**
     * Optional buy/sell direction.
     */
    OpenPitParamSide side;
};

struct OpenPitOrderOperationOptional {
    OpenPitOrderOperation value;
    bool is_set;
};

/**
 * Populated operation-identification group for an execution report.
 */
struct OpenPitExecutionReportOperation {
    /**
     * Trading instrument (`underlying + settlement` asset pair).
     */
    OpenPitInstrument instrument;
    /**
     * Account identifier associated with the report.
     */
    OpenPitParamAccountIdOptional account_id;
    /**
     * Buy or sell direction of the affected order or trade.
     */
    OpenPitParamSide side;
};

struct OpenPitExecutionReportOperationOptional {
    OpenPitExecutionReportOperation value;
    bool is_set;
};

/**
 * Full caller-owned execution-report payload.
 */
struct OpenPitExecutionReport {
    /**
     * Optional operation-identification group.
     */
    OpenPitExecutionReportOperationOptional operation;
    /**
     * Optional financial-impact group.
     */
    OpenPitFinancialImpactOptional financial_impact;
    /**
     * Optional fill-details group.
     */
    OpenPitExecutionReportFillOptional fill;
    /**
     * Optional position-impact group.
     */
    OpenPitExecutionReportPositionImpactOptional position_impact;
    /**
     * Opaque caller-defined token.
     *
     * The SDK never inspects, dereferences, or frees this value. Its meaning,
     * lifetime, and thread-safety are the caller's responsibility. `0` / null
     * means "not set". See the project Threading Contract for the full lifetime
     * model.
     *
     * The token is preserved unchanged across every engine callback that receives
     * the carrying value, including policy callbacks and adjustment callbacks.
     */
    void * user_data;
};

/**
 * Position-operation payload for account adjustment.
 */
struct OpenPitAccountAdjustmentPositionOperation {
    /**
     * Position instrument.
     */
    OpenPitInstrument instrument;
    /**
     * Position collateral asset.
     */
    OpenPitStringView collateral_asset;
    /**
     * Position average entry price.
     */
    OpenPitParamPriceOptional average_entry_price;
    /**
     * Optional leverage.
     */
    OpenPitParamLeverage leverage;
    /**
     * Position mode.
     */
    OpenPitParamPositionMode mode;
};

/**
 * Account-adjustment operation as a single discriminated value.
 *
 * `kind` selects which payload is meaningful; the payload not selected by
 * `kind` is ignored. Because a single discriminant chooses the payload,
 * supplying both a balance and a position operation at once is not
 * representable.
 */
struct OpenPitAccountAdjustmentOperation {
    /**
     * Selects which payload below is meaningful.
     */
    OpenPitAccountAdjustmentOperationKind kind;
    /**
     * Balance-operation payload, meaningful only when `kind` is `Balance`.
     */
    OpenPitAccountAdjustmentBalanceOperation balance;
    /**
     * Position-operation payload, meaningful only when `kind` is `Position`.
     */
    OpenPitAccountAdjustmentPositionOperation position;
};

/**
 * Full caller-owned account-adjustment payload.
 */
struct OpenPitAccountAdjustment {
    /**
     * Discriminated operation: at most one payload, selected by its kind.
     */
    OpenPitAccountAdjustmentOperation operation;
    /**
     * Optional amount-change group.
     */
    OpenPitAccountAdjustmentAmountOptional amount;
    /**
     * Optional bounds group.
     */
    OpenPitAccountAdjustmentBoundsOptional bounds;
    /**
     * Opaque caller-defined token.
     *
     * The SDK never inspects, dereferences, or frees this value. Its meaning,
     * lifetime, and thread-safety are the caller's responsibility. `0` / null
     * means "not set". See the project Threading Contract for the full lifetime
     * model.
     *
     * The token is preserved unchanged across every engine callback that receives
     * the carrying value, including policy callbacks and adjustment callbacks.
     */
    void * user_data;
};

/**
 * Full caller-owned order payload.
 */
struct OpenPitOrder {
    /**
     * Optional main operation group.
     */
    OpenPitOrderOperationOptional operation;
    /**
     * Optional margin group.
     */
    OpenPitOrderMarginOptional margin;
    /**
     * Optional position-management group.
     */
    OpenPitOrderPositionOptional position;
    /**
     * Opaque caller-defined token.
     *
     * The SDK never inspects, dereferences, or frees this value. Its meaning,
     * lifetime, and thread-safety are the caller's responsibility. `0` / null
     * means "not set". See the project Threading Contract for the full lifetime
     * model.
     *
     * The token is preserved unchanged across every engine callback that receives
     * the carrying value, including policy callbacks and adjustment callbacks.
     */
    void * user_data;
};

/**
 * Callback used by a custom pre-trade policy to validate one order before a
 * deferred pre-trade request is created.
 *
 * Contract:
 * - `ctx` is a read-only context valid only for the duration of the
 *   callback.
 * - `order` points to a read-only order view valid only for the duration of
 *   the callback.
 * - `order` is passed as a borrowed view and is not copied before the
 *   callback runs.
 * - If the callback wants to keep any data from `order`, it must copy that
 *   data before returning.
 * - Return null or an empty list to accept the order.
 * - Return a non-empty reject list to reject the order.
 * - A rejected order must set explicit `code` and `scope` values in every
 *   list item.
 * - The returned list ownership is transferred to the engine; create it with
 *   `openpit_pretrade_create_reject_list`.
 * - Every reject payload is copied into internal storage before the callback
 *   returns.
 * - `user_data` is passed through unchanged from policy creation.
 */
typedef OpenPitPretradeRejectList *
(*OpenPitPretradePreTradePolicyCheckPreTradeStartFn)(
    const OpenPitPretradeContext * ctx,
    const OpenPitOrder * order,
    void * user_data
);

/**
 * Callback used by a custom pre-trade policy to perform a main-stage check.
 *
 * Contract:
 * - `ctx` is a read-only context valid only for the duration of the
 *   callback.
 * - `order` points to a read-only order view valid only for the duration of
 *   the callback.
 * - `order` is passed as a borrowed view and is not copied before the
 *   callback runs.
 * - If the callback wants to keep any data from `order`, it must copy that
 *   data before returning.
 * - `mutations` is a callback-scoped non-owning pointer that allows the
 *   callback to register commit/rollback mutations.
 * - The callback must not store or use `mutations` after return.
 * - `out_result` is a callback-scoped non-owning collector the callback may
 *   fill with lock prices and account adjustments via
 *   `openpit_pretrade_pre_trade_result_push_lock_price` and
 *   `openpit_pretrade_pre_trade_result_push_account_adjustment`. Neither
 *   push carries a `policy_group_id`; the engine assigns the policy group.
 *   The callback must not store or use `out_result` after return.
 * - The reject channel and the `out_result` channel are independent: a
 *   callback may both reject and fill `out_result`, but the engine only
 *   keeps `out_result` when the callback accepts (returns null or an empty
 *   list).
 * - Return null or an empty list to accept the order.
 * - Return a non-empty reject list to reject the order.
 * - Every returned reject must contain explicit `code` and `scope` values.
 * - The returned list ownership is transferred to the engine; create it with
 *   `openpit_pretrade_create_reject_list`.
 * - Every reject payload is copied into internal storage before this
 *   callback returns.
 * - `user_data` is passed through unchanged from policy creation.
 *
 * Parameter ordering convention: read-only inputs first (`ctx`, `order`), then
 * callback-scoped collectors in the order (`mutations`, `out_result`), then
 * the trailing opaque `user_data`.
 */
typedef OpenPitPretradeRejectList *
(*OpenPitPretradePreTradePolicyPerformPreTradeCheckFn)(
    const OpenPitPretradeContext * ctx,
    const OpenPitOrder * order,
    OpenPitMutations * mutations,
    OpenPitPretradePreTradeResult * out_result,
    void * user_data
);

/**
 * Callback used by a custom pre-trade policy to observe an execution report.
 *
 * Contract:
 * - `ctx` is a read-only post-trade context valid only for the duration of
 *   the callback. Use `openpit_post_trade_context_get_account_group` to
 *   query the report account's group.
 * - `report` points to a read-only report view valid only for the duration
 *   of the callback.
 * - `report` is passed as a borrowed view and is not copied before the
 *   callback runs.
 * - If the callback wants to keep any data from `report`, it must copy that
 *   data before returning.
 * - `out_adjustments` is a callback-scoped non-owning collector the callback
 *   may fill with group-tagged account-adjustment outcomes via
 *   `openpit_pretrade_post_trade_adjustment_list_push`. This channel IS
 *   group-tagged. The callback must not store or use `out_adjustments` after
 *   return.
 * - The account-block return and the `out_adjustments` channel are
 *   independent: a callback may report blocks, adjustments, both, or
 *   neither.
 * - Return a non-null account-block list when this policy reports a
 *   kill-switch trigger. The returned list ownership is transferred to the
 *   engine; create it with `openpit_pretrade_create_account_block_list`.
 * - Return null to indicate no kill-switch condition.
 * - A null `apply_execution_report_fn` means that hook returns no blocks and
 *   no adjustments.
 * - `user_data` is passed through unchanged from policy creation.
 *
 * Parameter ordering convention: read-only context first (`ctx`), then
 * read-only input (`report`), then the callback-scoped collector
 * (`out_adjustments`), then the trailing opaque `user_data`.
 */
typedef OpenPitPretradeAccountBlockList *
(*OpenPitPretradePreTradePolicyApplyExecutionReportFn)(
    const OpenPitPostTradeContext * ctx,
    const OpenPitExecutionReport * report,
    OpenPitPostTradeAdjustmentList * out_adjustments,
    void * user_data
);

/**
 * Callback used by a custom pre-trade policy to validate one account
 * adjustment.
 *
 * Contract:
 * - `ctx` is a read-only context valid only for the duration of the
 *   callback.
 * - `adjustment` points to a read-only adjustment view valid only for the
 *   duration of the callback.
 * - `adjustment` is passed as a borrowed view and is not copied before the
 *   callback runs.
 * - If the callback wants to keep any data from `adjustment`, it must copy
 *   that data before returning.
 * - `account_id` must follow the same source model as the rest of the
 *   runtime state (numeric-only or string-derived-only).
 * - `mutations` is a callback-scoped non-owning pointer that allows the
 *   callback to register commit/rollback mutations.
 * - The callback must not store or use `mutations` after return.
 * - `out_outcomes` is a callback-scoped non-owning collector the callback
 *   may fill with account-outcome entries via
 *   `openpit_account_outcome_entry_list_push`. No `policy_group_id` is
 *   carried; the engine assigns the policy group. The callback must not
 *   store or use `out_outcomes` after return.
 * - The reject channel and the `out_outcomes` channel are independent: the
 *   engine only keeps `out_outcomes` when the callback accepts (returns null
 *   or an empty list).
 * - Return null to accept the adjustment.
 * - Return a non-empty reject list to reject the adjustment.
 * - Returned reject list ownership is transferred to the callee.
 * - `user_data` is passed through unchanged from policy creation.
 *
 * Parameter ordering convention: read-only inputs first (`ctx`, `account_id`,
 * `adjustment`), then callback-scoped collectors in the order (`mutations`,
 * `out_outcomes`), then the trailing opaque `user_data`.
 */
typedef OpenPitPretradeRejectList *
(*OpenPitPretradePreTradePolicyApplyAccountAdjustmentFn)(
    const OpenPitAccountAdjustmentContext * ctx,
    OpenPitParamAccountId account_id,
    const OpenPitAccountAdjustment * adjustment,
    OpenPitMutations * mutations,
    OpenPitAccountOutcomeEntryList * out_outcomes,
    void * user_data
);

/**
 * Callback invoked when the last reference to a custom pre-trade policy is
 * released and the policy object is about to be destroyed.
 *
 * Contract:
 * - Called exactly once, on the thread that drops the last policy reference.
 * - After this callback returns, no further callbacks will be invoked for
 *   this policy instance.
 * - `user_data` is the same value that was passed at policy creation.
 * - The callback must release any resources associated with `user_data`.
 */
typedef void (*OpenPitPretradePreTradePolicyFreeUserDataFn)(
    void * user_data
);

/**
 * Callback invoked for either commit or rollback of a registered mutation.
 */
typedef void (*OpenPitMutationFn)(
    void * user_data
);

/**
 * Optional callback to release mutation user_data after execution.
 *
 * Called exactly once per `openpit_mutations_push`:
 * - after `commit_fn` when commit runs;
 * - after `rollback_fn` when rollback runs;
 * - or on drop if neither action ran.
 */
typedef void (*OpenPitMutationFreeFn)(
    void * user_data
);

/**
 * Resolves the reading account's group on demand.
 *
 * Returns `true` and writes the group id to `out_account_group_id` when the
 * account belongs to a group; returns `false` when it has none. Invoked lazily
 * by `openpit_marketdata_service_get` — only when the resolution mode would
 * consult the group or default-group bucket and the per-account bucket has no
 * quote.
 *
 * The function pointer must not be null; see the contract on
 * `openpit_marketdata_service_get`.
 */
typedef bool (*OpenPitMarketDataAccountGroupResolver)(
    void * user_data,
    OpenPitParamAccountGroupId * out_account_group_id
);

/**
 * Validates a decimal and returns a `Pnl` wrapper.
 *
 * Meaning: Profit and loss value; positive means profit, negative means loss.
 *
 * Success:
 * - returns `true` and writes a validated wrapper to `out`.
 *
 * Error:
 * - returns `false` when `out` is null or when the decimal does not satisfy
 *   the rules of this type;
 * - on error read `out_error` for the message.
 */
bool openpit_create_param_pnl(
    OpenPitParamDecimal value,
    OpenPitParamPnl * out,
    OpenPitOutParamError out_error
);

/**
 * Returns the decimal stored in `Pnl`.
 */
OpenPitParamDecimal openpit_param_pnl_get_decimal(
    OpenPitParamPnl value
);

/**
 * Validates a decimal and returns a `Price` wrapper.
 *
 * Meaning: Price per one instrument unit; may be negative in some derivative
 * markets.
 *
 * Success:
 * - returns `true` and writes a validated wrapper to `out`.
 *
 * Error:
 * - returns `false` when `out` is null or when the decimal does not satisfy
 *   the rules of this type;
 * - on error read `out_error` for the message.
 */
bool openpit_create_param_price(
    OpenPitParamDecimal value,
    OpenPitParamPrice * out,
    OpenPitOutParamError out_error
);

/**
 * Returns the decimal stored in `Price`.
 */
OpenPitParamDecimal openpit_param_price_get_decimal(
    OpenPitParamPrice value
);

/**
 * Validates a decimal and returns a `Quantity` wrapper.
 *
 * Meaning: Instrument quantity; non-negative amount in instrument units.
 *
 * Success:
 * - returns `true` and writes a validated wrapper to `out`.
 *
 * Error:
 * - returns `false` when `out` is null or when the decimal does not satisfy
 *   the rules of this type;
 * - on error read `out_error` for the message.
 */
bool openpit_create_param_quantity(
    OpenPitParamDecimal value,
    OpenPitParamQuantity * out,
    OpenPitOutParamError out_error
);

/**
 * Returns the decimal stored in `Quantity`.
 */
OpenPitParamDecimal openpit_param_quantity_get_decimal(
    OpenPitParamQuantity value
);

/**
 * Validates a decimal and returns a `Volume` wrapper.
 *
 * Meaning: Settlement notional volume; non-negative amount in settlement
 * units.
 *
 * Success:
 * - returns `true` and writes a validated wrapper to `out`.
 *
 * Error:
 * - returns `false` when `out` is null or when the decimal does not satisfy
 *   the rules of this type;
 * - on error read `out_error` for the message.
 */
bool openpit_create_param_volume(
    OpenPitParamDecimal value,
    OpenPitParamVolume * out,
    OpenPitOutParamError out_error
);

/**
 * Returns the decimal stored in `Volume`.
 */
OpenPitParamDecimal openpit_param_volume_get_decimal(
    OpenPitParamVolume value
);

/**
 * Validates a decimal and returns a `CashFlow` wrapper.
 *
 * Meaning: Cash flow contribution; positive is inflow, negative is outflow.
 *
 * Success:
 * - returns `true` and writes a validated wrapper to `out`.
 *
 * Error:
 * - returns `false` when `out` is null or when the decimal does not satisfy
 *   the rules of this type;
 * - on error read `out_error` for the message.
 */
bool openpit_create_param_cash_flow(
    OpenPitParamDecimal value,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);

/**
 * Returns the decimal stored in `CashFlow`.
 */
OpenPitParamDecimal openpit_param_cash_flow_get_decimal(
    OpenPitParamCashFlow value
);

/**
 * Validates a decimal and returns a `PositionSize` wrapper.
 *
 * Meaning: Signed position size; long is positive, short is negative.
 *
 * Success:
 * - returns `true` and writes a validated wrapper to `out`.
 *
 * Error:
 * - returns `false` when `out` is null or when the decimal does not satisfy
 *   the rules of this type;
 * - on error read `out_error` for the message.
 */
bool openpit_create_param_position_size(
    OpenPitParamDecimal value,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);

/**
 * Returns the decimal stored in `PositionSize`.
 */
OpenPitParamDecimal openpit_param_position_size_get_decimal(
    OpenPitParamPositionSize value
);

/**
 * Validates a decimal and returns a `Fee` wrapper.
 *
 * Meaning: Fee amount; can be negative for rebates or reconciliation
 * adjustments.
 *
 * Success:
 * - returns `true` and writes a validated wrapper to `out`.
 *
 * Error:
 * - returns `false` when `out` is null or when the decimal does not satisfy
 *   the rules of this type;
 * - on error read `out_error` for the message.
 */
bool openpit_create_param_fee(
    OpenPitParamDecimal value,
    OpenPitParamFee * out,
    OpenPitOutParamError out_error
);

/**
 * Returns the decimal stored in `Fee`.
 */
OpenPitParamDecimal openpit_param_fee_get_decimal(
    OpenPitParamFee value
);

/**
 * Validates a decimal and returns a `Notional` wrapper.
 *
 * Meaning: Monetary position exposure for margin and risk calculation; always
 * non-negative.
 *
 * Success:
 * - returns `true` and writes a validated wrapper to `out`.
 *
 * Error:
 * - returns `false` when `out` is null or when the decimal does not satisfy
 *   the rules of this type;
 * - on error read `out_error` for the message.
 */
bool openpit_create_param_notional(
    OpenPitParamDecimal value,
    OpenPitParamNotional * out,
    OpenPitOutParamError out_error
);

/**
 * Returns the decimal stored in `Notional`.
 */
OpenPitParamDecimal openpit_param_notional_get_decimal(
    OpenPitParamNotional value
);

bool openpit_create_param_pnl_from_string(
    OpenPitStringView value,
    OpenPitParamPnl * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_pnl_from_f64(
    double value,
    OpenPitParamPnl * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_pnl_from_int64(
    int64_t value,
    OpenPitParamPnl * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_pnl_from_uint64(
    uint64_t value,
    OpenPitParamPnl * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_pnl_from_string_rounded(
    OpenPitStringView value,
    uint32_t scale,
    OpenPitParamRoundingStrategy rounding,
    OpenPitParamPnl * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_pnl_from_f64_rounded(
    double value,
    uint32_t scale,
    OpenPitParamRoundingStrategy rounding,
    OpenPitParamPnl * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_pnl_from_decimal_rounded(
    OpenPitParamDecimal value,
    uint32_t scale,
    OpenPitParamRoundingStrategy rounding,
    OpenPitParamPnl * out,
    OpenPitOutParamError out_error
);

bool openpit_param_pnl_to_f64(
    OpenPitParamPnl value,
    double * out,
    OpenPitOutParamError out_error
);

bool openpit_param_pnl_is_zero(
    OpenPitParamPnl value,
    bool * out,
    OpenPitOutParamError out_error
);

bool openpit_param_pnl_compare(
    OpenPitParamPnl lhs,
    OpenPitParamPnl rhs,
    int8_t * out,
    OpenPitOutParamError out_error
);

OpenPitSharedString * openpit_param_pnl_to_string(
    OpenPitParamPnl value,
    OpenPitOutParamError out_error
);

bool openpit_param_pnl_checked_add(
    OpenPitParamPnl lhs,
    OpenPitParamPnl rhs,
    OpenPitParamPnl * out,
    OpenPitOutParamError out_error
);

bool openpit_param_pnl_checked_sub(
    OpenPitParamPnl lhs,
    OpenPitParamPnl rhs,
    OpenPitParamPnl * out,
    OpenPitOutParamError out_error
);

bool openpit_param_pnl_checked_mul_i64(
    OpenPitParamPnl value,
    int64_t multiplier,
    OpenPitParamPnl * out,
    OpenPitOutParamError out_error
);

bool openpit_param_pnl_checked_mul_u64(
    OpenPitParamPnl value,
    uint64_t multiplier,
    OpenPitParamPnl * out,
    OpenPitOutParamError out_error
);

bool openpit_param_pnl_checked_mul_f64(
    OpenPitParamPnl value,
    double multiplier,
    OpenPitParamPnl * out,
    OpenPitOutParamError out_error
);

bool openpit_param_pnl_checked_div_i64(
    OpenPitParamPnl value,
    int64_t divisor,
    OpenPitParamPnl * out,
    OpenPitOutParamError out_error
);

bool openpit_param_pnl_checked_div_u64(
    OpenPitParamPnl value,
    uint64_t divisor,
    OpenPitParamPnl * out,
    OpenPitOutParamError out_error
);

bool openpit_param_pnl_checked_div_f64(
    OpenPitParamPnl value,
    double divisor,
    OpenPitParamPnl * out,
    OpenPitOutParamError out_error
);

bool openpit_param_pnl_checked_rem_i64(
    OpenPitParamPnl value,
    int64_t divisor,
    OpenPitParamPnl * out,
    OpenPitOutParamError out_error
);

bool openpit_param_pnl_checked_rem_u64(
    OpenPitParamPnl value,
    uint64_t divisor,
    OpenPitParamPnl * out,
    OpenPitOutParamError out_error
);

bool openpit_param_pnl_checked_rem_f64(
    OpenPitParamPnl value,
    double divisor,
    OpenPitParamPnl * out,
    OpenPitOutParamError out_error
);

bool openpit_param_pnl_checked_neg(
    OpenPitParamPnl value,
    OpenPitParamPnl * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_price_from_string(
    OpenPitStringView value,
    OpenPitParamPrice * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_price_from_f64(
    double value,
    OpenPitParamPrice * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_price_from_int64(
    int64_t value,
    OpenPitParamPrice * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_price_from_uint64(
    uint64_t value,
    OpenPitParamPrice * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_price_from_string_rounded(
    OpenPitStringView value,
    uint32_t scale,
    OpenPitParamRoundingStrategy rounding,
    OpenPitParamPrice * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_price_from_f64_rounded(
    double value,
    uint32_t scale,
    OpenPitParamRoundingStrategy rounding,
    OpenPitParamPrice * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_price_from_decimal_rounded(
    OpenPitParamDecimal value,
    uint32_t scale,
    OpenPitParamRoundingStrategy rounding,
    OpenPitParamPrice * out,
    OpenPitOutParamError out_error
);

bool openpit_param_price_to_f64(
    OpenPitParamPrice value,
    double * out,
    OpenPitOutParamError out_error
);

bool openpit_param_price_is_zero(
    OpenPitParamPrice value,
    bool * out,
    OpenPitOutParamError out_error
);

bool openpit_param_price_compare(
    OpenPitParamPrice lhs,
    OpenPitParamPrice rhs,
    int8_t * out,
    OpenPitOutParamError out_error
);

OpenPitSharedString * openpit_param_price_to_string(
    OpenPitParamPrice value,
    OpenPitOutParamError out_error
);

bool openpit_param_price_checked_add(
    OpenPitParamPrice lhs,
    OpenPitParamPrice rhs,
    OpenPitParamPrice * out,
    OpenPitOutParamError out_error
);

bool openpit_param_price_checked_sub(
    OpenPitParamPrice lhs,
    OpenPitParamPrice rhs,
    OpenPitParamPrice * out,
    OpenPitOutParamError out_error
);

bool openpit_param_price_checked_mul_i64(
    OpenPitParamPrice value,
    int64_t multiplier,
    OpenPitParamPrice * out,
    OpenPitOutParamError out_error
);

bool openpit_param_price_checked_mul_u64(
    OpenPitParamPrice value,
    uint64_t multiplier,
    OpenPitParamPrice * out,
    OpenPitOutParamError out_error
);

bool openpit_param_price_checked_mul_f64(
    OpenPitParamPrice value,
    double multiplier,
    OpenPitParamPrice * out,
    OpenPitOutParamError out_error
);

bool openpit_param_price_checked_div_i64(
    OpenPitParamPrice value,
    int64_t divisor,
    OpenPitParamPrice * out,
    OpenPitOutParamError out_error
);

bool openpit_param_price_checked_div_u64(
    OpenPitParamPrice value,
    uint64_t divisor,
    OpenPitParamPrice * out,
    OpenPitOutParamError out_error
);

bool openpit_param_price_checked_div_f64(
    OpenPitParamPrice value,
    double divisor,
    OpenPitParamPrice * out,
    OpenPitOutParamError out_error
);

bool openpit_param_price_checked_rem_i64(
    OpenPitParamPrice value,
    int64_t divisor,
    OpenPitParamPrice * out,
    OpenPitOutParamError out_error
);

bool openpit_param_price_checked_rem_u64(
    OpenPitParamPrice value,
    uint64_t divisor,
    OpenPitParamPrice * out,
    OpenPitOutParamError out_error
);

bool openpit_param_price_checked_rem_f64(
    OpenPitParamPrice value,
    double divisor,
    OpenPitParamPrice * out,
    OpenPitOutParamError out_error
);

bool openpit_param_price_checked_neg(
    OpenPitParamPrice value,
    OpenPitParamPrice * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_quantity_from_string(
    OpenPitStringView value,
    OpenPitParamQuantity * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_quantity_from_f64(
    double value,
    OpenPitParamQuantity * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_quantity_from_int64(
    int64_t value,
    OpenPitParamQuantity * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_quantity_from_uint64(
    uint64_t value,
    OpenPitParamQuantity * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_quantity_from_string_rounded(
    OpenPitStringView value,
    uint32_t scale,
    OpenPitParamRoundingStrategy rounding,
    OpenPitParamQuantity * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_quantity_from_f64_rounded(
    double value,
    uint32_t scale,
    OpenPitParamRoundingStrategy rounding,
    OpenPitParamQuantity * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_quantity_from_decimal_rounded(
    OpenPitParamDecimal value,
    uint32_t scale,
    OpenPitParamRoundingStrategy rounding,
    OpenPitParamQuantity * out,
    OpenPitOutParamError out_error
);

bool openpit_param_quantity_to_f64(
    OpenPitParamQuantity value,
    double * out,
    OpenPitOutParamError out_error
);

bool openpit_param_quantity_is_zero(
    OpenPitParamQuantity value,
    bool * out,
    OpenPitOutParamError out_error
);

bool openpit_param_quantity_compare(
    OpenPitParamQuantity lhs,
    OpenPitParamQuantity rhs,
    int8_t * out,
    OpenPitOutParamError out_error
);

OpenPitSharedString * openpit_param_quantity_to_string(
    OpenPitParamQuantity value,
    OpenPitOutParamError out_error
);

bool openpit_param_quantity_checked_add(
    OpenPitParamQuantity lhs,
    OpenPitParamQuantity rhs,
    OpenPitParamQuantity * out,
    OpenPitOutParamError out_error
);

bool openpit_param_quantity_checked_sub(
    OpenPitParamQuantity lhs,
    OpenPitParamQuantity rhs,
    OpenPitParamQuantity * out,
    OpenPitOutParamError out_error
);

bool openpit_param_quantity_checked_mul_i64(
    OpenPitParamQuantity value,
    int64_t multiplier,
    OpenPitParamQuantity * out,
    OpenPitOutParamError out_error
);

bool openpit_param_quantity_checked_mul_u64(
    OpenPitParamQuantity value,
    uint64_t multiplier,
    OpenPitParamQuantity * out,
    OpenPitOutParamError out_error
);

bool openpit_param_quantity_checked_mul_f64(
    OpenPitParamQuantity value,
    double multiplier,
    OpenPitParamQuantity * out,
    OpenPitOutParamError out_error
);

bool openpit_param_quantity_checked_div_i64(
    OpenPitParamQuantity value,
    int64_t divisor,
    OpenPitParamQuantity * out,
    OpenPitOutParamError out_error
);

bool openpit_param_quantity_checked_div_u64(
    OpenPitParamQuantity value,
    uint64_t divisor,
    OpenPitParamQuantity * out,
    OpenPitOutParamError out_error
);

bool openpit_param_quantity_checked_div_f64(
    OpenPitParamQuantity value,
    double divisor,
    OpenPitParamQuantity * out,
    OpenPitOutParamError out_error
);

bool openpit_param_quantity_checked_rem_i64(
    OpenPitParamQuantity value,
    int64_t divisor,
    OpenPitParamQuantity * out,
    OpenPitOutParamError out_error
);

bool openpit_param_quantity_checked_rem_u64(
    OpenPitParamQuantity value,
    uint64_t divisor,
    OpenPitParamQuantity * out,
    OpenPitOutParamError out_error
);

bool openpit_param_quantity_checked_rem_f64(
    OpenPitParamQuantity value,
    double divisor,
    OpenPitParamQuantity * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_volume_from_string(
    OpenPitStringView value,
    OpenPitParamVolume * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_volume_from_f64(
    double value,
    OpenPitParamVolume * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_volume_from_int64(
    int64_t value,
    OpenPitParamVolume * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_volume_from_uint64(
    uint64_t value,
    OpenPitParamVolume * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_volume_from_string_rounded(
    OpenPitStringView value,
    uint32_t scale,
    OpenPitParamRoundingStrategy rounding,
    OpenPitParamVolume * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_volume_from_f64_rounded(
    double value,
    uint32_t scale,
    OpenPitParamRoundingStrategy rounding,
    OpenPitParamVolume * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_volume_from_decimal_rounded(
    OpenPitParamDecimal value,
    uint32_t scale,
    OpenPitParamRoundingStrategy rounding,
    OpenPitParamVolume * out,
    OpenPitOutParamError out_error
);

bool openpit_param_volume_to_f64(
    OpenPitParamVolume value,
    double * out,
    OpenPitOutParamError out_error
);

bool openpit_param_volume_is_zero(
    OpenPitParamVolume value,
    bool * out,
    OpenPitOutParamError out_error
);

bool openpit_param_volume_compare(
    OpenPitParamVolume lhs,
    OpenPitParamVolume rhs,
    int8_t * out,
    OpenPitOutParamError out_error
);

OpenPitSharedString * openpit_param_volume_to_string(
    OpenPitParamVolume value,
    OpenPitOutParamError out_error
);

bool openpit_param_volume_checked_add(
    OpenPitParamVolume lhs,
    OpenPitParamVolume rhs,
    OpenPitParamVolume * out,
    OpenPitOutParamError out_error
);

bool openpit_param_volume_checked_sub(
    OpenPitParamVolume lhs,
    OpenPitParamVolume rhs,
    OpenPitParamVolume * out,
    OpenPitOutParamError out_error
);

bool openpit_param_volume_checked_mul_i64(
    OpenPitParamVolume value,
    int64_t multiplier,
    OpenPitParamVolume * out,
    OpenPitOutParamError out_error
);

bool openpit_param_volume_checked_mul_u64(
    OpenPitParamVolume value,
    uint64_t multiplier,
    OpenPitParamVolume * out,
    OpenPitOutParamError out_error
);

bool openpit_param_volume_checked_mul_f64(
    OpenPitParamVolume value,
    double multiplier,
    OpenPitParamVolume * out,
    OpenPitOutParamError out_error
);

bool openpit_param_volume_checked_div_i64(
    OpenPitParamVolume value,
    int64_t divisor,
    OpenPitParamVolume * out,
    OpenPitOutParamError out_error
);

bool openpit_param_volume_checked_div_u64(
    OpenPitParamVolume value,
    uint64_t divisor,
    OpenPitParamVolume * out,
    OpenPitOutParamError out_error
);

bool openpit_param_volume_checked_div_f64(
    OpenPitParamVolume value,
    double divisor,
    OpenPitParamVolume * out,
    OpenPitOutParamError out_error
);

bool openpit_param_volume_checked_rem_i64(
    OpenPitParamVolume value,
    int64_t divisor,
    OpenPitParamVolume * out,
    OpenPitOutParamError out_error
);

bool openpit_param_volume_checked_rem_u64(
    OpenPitParamVolume value,
    uint64_t divisor,
    OpenPitParamVolume * out,
    OpenPitOutParamError out_error
);

bool openpit_param_volume_checked_rem_f64(
    OpenPitParamVolume value,
    double divisor,
    OpenPitParamVolume * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_cash_flow_from_string(
    OpenPitStringView value,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_cash_flow_from_f64(
    double value,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_cash_flow_from_int64(
    int64_t value,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_cash_flow_from_uint64(
    uint64_t value,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_cash_flow_from_string_rounded(
    OpenPitStringView value,
    uint32_t scale,
    OpenPitParamRoundingStrategy rounding,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_cash_flow_from_f64_rounded(
    double value,
    uint32_t scale,
    OpenPitParamRoundingStrategy rounding,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_cash_flow_from_decimal_rounded(
    OpenPitParamDecimal value,
    uint32_t scale,
    OpenPitParamRoundingStrategy rounding,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);

bool openpit_param_cash_flow_to_f64(
    OpenPitParamCashFlow value,
    double * out,
    OpenPitOutParamError out_error
);

bool openpit_param_cash_flow_is_zero(
    OpenPitParamCashFlow value,
    bool * out,
    OpenPitOutParamError out_error
);

bool openpit_param_cash_flow_compare(
    OpenPitParamCashFlow lhs,
    OpenPitParamCashFlow rhs,
    int8_t * out,
    OpenPitOutParamError out_error
);

OpenPitSharedString * openpit_param_cash_flow_to_string(
    OpenPitParamCashFlow value,
    OpenPitOutParamError out_error
);

bool openpit_param_cash_flow_checked_add(
    OpenPitParamCashFlow lhs,
    OpenPitParamCashFlow rhs,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);

bool openpit_param_cash_flow_checked_sub(
    OpenPitParamCashFlow lhs,
    OpenPitParamCashFlow rhs,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);

bool openpit_param_cash_flow_checked_mul_i64(
    OpenPitParamCashFlow value,
    int64_t multiplier,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);

bool openpit_param_cash_flow_checked_mul_u64(
    OpenPitParamCashFlow value,
    uint64_t multiplier,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);

bool openpit_param_cash_flow_checked_mul_f64(
    OpenPitParamCashFlow value,
    double multiplier,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);

bool openpit_param_cash_flow_checked_div_i64(
    OpenPitParamCashFlow value,
    int64_t divisor,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);

bool openpit_param_cash_flow_checked_div_u64(
    OpenPitParamCashFlow value,
    uint64_t divisor,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);

bool openpit_param_cash_flow_checked_div_f64(
    OpenPitParamCashFlow value,
    double divisor,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);

bool openpit_param_cash_flow_checked_rem_i64(
    OpenPitParamCashFlow value,
    int64_t divisor,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);

bool openpit_param_cash_flow_checked_rem_u64(
    OpenPitParamCashFlow value,
    uint64_t divisor,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);

bool openpit_param_cash_flow_checked_rem_f64(
    OpenPitParamCashFlow value,
    double divisor,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);

bool openpit_param_cash_flow_checked_neg(
    OpenPitParamCashFlow value,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_position_size_from_string(
    OpenPitStringView value,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_position_size_from_f64(
    double value,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_position_size_from_int64(
    int64_t value,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_position_size_from_uint64(
    uint64_t value,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_position_size_from_string_rounded(
    OpenPitStringView value,
    uint32_t scale,
    OpenPitParamRoundingStrategy rounding,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_position_size_from_f64_rounded(
    double value,
    uint32_t scale,
    OpenPitParamRoundingStrategy rounding,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_position_size_from_decimal_rounded(
    OpenPitParamDecimal value,
    uint32_t scale,
    OpenPitParamRoundingStrategy rounding,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);

bool openpit_param_position_size_to_f64(
    OpenPitParamPositionSize value,
    double * out,
    OpenPitOutParamError out_error
);

bool openpit_param_position_size_is_zero(
    OpenPitParamPositionSize value,
    bool * out,
    OpenPitOutParamError out_error
);

bool openpit_param_position_size_compare(
    OpenPitParamPositionSize lhs,
    OpenPitParamPositionSize rhs,
    int8_t * out,
    OpenPitOutParamError out_error
);

OpenPitSharedString * openpit_param_position_size_to_string(
    OpenPitParamPositionSize value,
    OpenPitOutParamError out_error
);

bool openpit_param_position_size_checked_add(
    OpenPitParamPositionSize lhs,
    OpenPitParamPositionSize rhs,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);

bool openpit_param_position_size_checked_sub(
    OpenPitParamPositionSize lhs,
    OpenPitParamPositionSize rhs,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);

bool openpit_param_position_size_checked_mul_i64(
    OpenPitParamPositionSize value,
    int64_t multiplier,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);

bool openpit_param_position_size_checked_mul_u64(
    OpenPitParamPositionSize value,
    uint64_t multiplier,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);

bool openpit_param_position_size_checked_mul_f64(
    OpenPitParamPositionSize value,
    double multiplier,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);

bool openpit_param_position_size_checked_div_i64(
    OpenPitParamPositionSize value,
    int64_t divisor,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);

bool openpit_param_position_size_checked_div_u64(
    OpenPitParamPositionSize value,
    uint64_t divisor,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);

bool openpit_param_position_size_checked_div_f64(
    OpenPitParamPositionSize value,
    double divisor,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);

bool openpit_param_position_size_checked_rem_i64(
    OpenPitParamPositionSize value,
    int64_t divisor,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);

bool openpit_param_position_size_checked_rem_u64(
    OpenPitParamPositionSize value,
    uint64_t divisor,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);

bool openpit_param_position_size_checked_rem_f64(
    OpenPitParamPositionSize value,
    double divisor,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);

bool openpit_param_position_size_checked_neg(
    OpenPitParamPositionSize value,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_fee_from_string(
    OpenPitStringView value,
    OpenPitParamFee * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_fee_from_f64(
    double value,
    OpenPitParamFee * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_fee_from_int64(
    int64_t value,
    OpenPitParamFee * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_fee_from_uint64(
    uint64_t value,
    OpenPitParamFee * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_fee_from_string_rounded(
    OpenPitStringView value,
    uint32_t scale,
    OpenPitParamRoundingStrategy rounding,
    OpenPitParamFee * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_fee_from_f64_rounded(
    double value,
    uint32_t scale,
    OpenPitParamRoundingStrategy rounding,
    OpenPitParamFee * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_fee_from_decimal_rounded(
    OpenPitParamDecimal value,
    uint32_t scale,
    OpenPitParamRoundingStrategy rounding,
    OpenPitParamFee * out,
    OpenPitOutParamError out_error
);

bool openpit_param_fee_to_f64(
    OpenPitParamFee value,
    double * out,
    OpenPitOutParamError out_error
);

bool openpit_param_fee_is_zero(
    OpenPitParamFee value,
    bool * out,
    OpenPitOutParamError out_error
);

bool openpit_param_fee_compare(
    OpenPitParamFee lhs,
    OpenPitParamFee rhs,
    int8_t * out,
    OpenPitOutParamError out_error
);

OpenPitSharedString * openpit_param_fee_to_string(
    OpenPitParamFee value,
    OpenPitOutParamError out_error
);

bool openpit_param_fee_checked_add(
    OpenPitParamFee lhs,
    OpenPitParamFee rhs,
    OpenPitParamFee * out,
    OpenPitOutParamError out_error
);

bool openpit_param_fee_checked_sub(
    OpenPitParamFee lhs,
    OpenPitParamFee rhs,
    OpenPitParamFee * out,
    OpenPitOutParamError out_error
);

bool openpit_param_fee_checked_mul_i64(
    OpenPitParamFee value,
    int64_t multiplier,
    OpenPitParamFee * out,
    OpenPitOutParamError out_error
);

bool openpit_param_fee_checked_mul_u64(
    OpenPitParamFee value,
    uint64_t multiplier,
    OpenPitParamFee * out,
    OpenPitOutParamError out_error
);

bool openpit_param_fee_checked_mul_f64(
    OpenPitParamFee value,
    double multiplier,
    OpenPitParamFee * out,
    OpenPitOutParamError out_error
);

bool openpit_param_fee_checked_div_i64(
    OpenPitParamFee value,
    int64_t divisor,
    OpenPitParamFee * out,
    OpenPitOutParamError out_error
);

bool openpit_param_fee_checked_div_u64(
    OpenPitParamFee value,
    uint64_t divisor,
    OpenPitParamFee * out,
    OpenPitOutParamError out_error
);

bool openpit_param_fee_checked_div_f64(
    OpenPitParamFee value,
    double divisor,
    OpenPitParamFee * out,
    OpenPitOutParamError out_error
);

bool openpit_param_fee_checked_rem_i64(
    OpenPitParamFee value,
    int64_t divisor,
    OpenPitParamFee * out,
    OpenPitOutParamError out_error
);

bool openpit_param_fee_checked_rem_u64(
    OpenPitParamFee value,
    uint64_t divisor,
    OpenPitParamFee * out,
    OpenPitOutParamError out_error
);

bool openpit_param_fee_checked_rem_f64(
    OpenPitParamFee value,
    double divisor,
    OpenPitParamFee * out,
    OpenPitOutParamError out_error
);

bool openpit_param_fee_checked_neg(
    OpenPitParamFee value,
    OpenPitParamFee * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_notional_from_string(
    OpenPitStringView value,
    OpenPitParamNotional * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_notional_from_f64(
    double value,
    OpenPitParamNotional * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_notional_from_int64(
    int64_t value,
    OpenPitParamNotional * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_notional_from_uint64(
    uint64_t value,
    OpenPitParamNotional * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_notional_from_string_rounded(
    OpenPitStringView value,
    uint32_t scale,
    OpenPitParamRoundingStrategy rounding,
    OpenPitParamNotional * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_notional_from_f64_rounded(
    double value,
    uint32_t scale,
    OpenPitParamRoundingStrategy rounding,
    OpenPitParamNotional * out,
    OpenPitOutParamError out_error
);

bool openpit_create_param_notional_from_decimal_rounded(
    OpenPitParamDecimal value,
    uint32_t scale,
    OpenPitParamRoundingStrategy rounding,
    OpenPitParamNotional * out,
    OpenPitOutParamError out_error
);

bool openpit_param_notional_to_f64(
    OpenPitParamNotional value,
    double * out,
    OpenPitOutParamError out_error
);

bool openpit_param_notional_is_zero(
    OpenPitParamNotional value,
    bool * out,
    OpenPitOutParamError out_error
);

bool openpit_param_notional_compare(
    OpenPitParamNotional lhs,
    OpenPitParamNotional rhs,
    int8_t * out,
    OpenPitOutParamError out_error
);

OpenPitSharedString * openpit_param_notional_to_string(
    OpenPitParamNotional value,
    OpenPitOutParamError out_error
);

bool openpit_param_notional_checked_add(
    OpenPitParamNotional lhs,
    OpenPitParamNotional rhs,
    OpenPitParamNotional * out,
    OpenPitOutParamError out_error
);

bool openpit_param_notional_checked_sub(
    OpenPitParamNotional lhs,
    OpenPitParamNotional rhs,
    OpenPitParamNotional * out,
    OpenPitOutParamError out_error
);

bool openpit_param_notional_checked_mul_i64(
    OpenPitParamNotional value,
    int64_t multiplier,
    OpenPitParamNotional * out,
    OpenPitOutParamError out_error
);

bool openpit_param_notional_checked_mul_u64(
    OpenPitParamNotional value,
    uint64_t multiplier,
    OpenPitParamNotional * out,
    OpenPitOutParamError out_error
);

bool openpit_param_notional_checked_mul_f64(
    OpenPitParamNotional value,
    double multiplier,
    OpenPitParamNotional * out,
    OpenPitOutParamError out_error
);

bool openpit_param_notional_checked_div_i64(
    OpenPitParamNotional value,
    int64_t divisor,
    OpenPitParamNotional * out,
    OpenPitOutParamError out_error
);

bool openpit_param_notional_checked_div_u64(
    OpenPitParamNotional value,
    uint64_t divisor,
    OpenPitParamNotional * out,
    OpenPitOutParamError out_error
);

bool openpit_param_notional_checked_div_f64(
    OpenPitParamNotional value,
    double divisor,
    OpenPitParamNotional * out,
    OpenPitOutParamError out_error
);

bool openpit_param_notional_checked_rem_i64(
    OpenPitParamNotional value,
    int64_t divisor,
    OpenPitParamNotional * out,
    OpenPitOutParamError out_error
);

bool openpit_param_notional_checked_rem_u64(
    OpenPitParamNotional value,
    uint64_t divisor,
    OpenPitParamNotional * out,
    OpenPitOutParamError out_error
);

bool openpit_param_notional_checked_rem_f64(
    OpenPitParamNotional value,
    double divisor,
    OpenPitParamNotional * out,
    OpenPitOutParamError out_error
);

bool openpit_param_leverage_calculate_margin_required(
    OpenPitParamLeverage leverage,
    OpenPitParamNotional notional,
    OpenPitParamNotional * out,
    OpenPitOutParamError out_error
);

bool openpit_param_price_calculate_volume(
    OpenPitParamPrice price,
    OpenPitParamQuantity quantity,
    OpenPitParamVolume * out,
    OpenPitOutParamError out_error
);

bool openpit_param_price_calculate_position_size(
    OpenPitParamPrice price,
    OpenPitParamQuantity quantity,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);

bool openpit_param_quantity_calculate_volume(
    OpenPitParamQuantity quantity,
    OpenPitParamPrice price,
    OpenPitParamVolume * out,
    OpenPitOutParamError out_error
);

bool openpit_param_volume_calculate_quantity(
    OpenPitParamVolume volume,
    OpenPitParamPrice price,
    OpenPitParamQuantity * out,
    OpenPitOutParamError out_error
);

bool openpit_param_pnl_to_cash_flow(
    OpenPitParamPnl value,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);

bool openpit_param_pnl_to_position_size(
    OpenPitParamPnl value,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);

bool openpit_param_quantity_to_position_size(
    OpenPitParamQuantity value,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);

bool openpit_param_volume_to_position_size(
    OpenPitParamVolume value,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);

bool openpit_param_pnl_from_fee(
    OpenPitParamFee fee,
    OpenPitParamPnl * out,
    OpenPitOutParamError out_error
);

bool openpit_param_cash_flow_from_pnl(
    OpenPitParamPnl pnl,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);

bool openpit_param_cash_flow_from_fee(
    OpenPitParamFee fee,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);

bool openpit_param_cash_flow_from_volume_inflow(
    OpenPitParamVolume volume,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);

bool openpit_param_cash_flow_from_volume_outflow(
    OpenPitParamVolume volume,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);

bool openpit_param_fee_to_pnl(
    OpenPitParamFee fee,
    OpenPitParamPnl * out,
    OpenPitOutParamError out_error
);

bool openpit_param_fee_to_position_size(
    OpenPitParamFee fee,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);

bool openpit_param_fee_to_cash_flow(
    OpenPitParamFee fee,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);

bool openpit_param_volume_to_cash_flow_inflow(
    OpenPitParamVolume volume,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);

bool openpit_param_volume_to_cash_flow_outflow(
    OpenPitParamVolume volume,
    OpenPitParamCashFlow * out,
    OpenPitOutParamError out_error
);

bool openpit_param_position_size_from_pnl(
    OpenPitParamPnl pnl,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);

bool openpit_param_position_size_from_fee(
    OpenPitParamFee fee,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);

bool openpit_param_position_size_from_quantity_and_side(
    OpenPitParamQuantity quantity,
    OpenPitParamSide side,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);

bool openpit_param_position_size_to_open_quantity(
    OpenPitParamPositionSize value,
    OpenPitParamQuantity * out_quantity,
    OpenPitParamSide * out_side,
    OpenPitOutParamError out_error
);

bool openpit_param_position_size_to_close_quantity(
    OpenPitParamPositionSize value,
    OpenPitParamQuantity * out_quantity,
    OpenPitParamSide * out_side,
    OpenPitOutParamError out_error
);

bool openpit_param_position_size_checked_add_quantity(
    OpenPitParamPositionSize value,
    OpenPitParamQuantity quantity,
    OpenPitParamSide side,
    OpenPitParamPositionSize * out,
    OpenPitOutParamError out_error
);

bool openpit_param_price_calculate_notional(
    OpenPitParamPrice price,
    OpenPitParamQuantity quantity,
    OpenPitParamNotional * out,
    OpenPitOutParamError out_error
);

bool openpit_param_quantity_calculate_notional(
    OpenPitParamQuantity quantity,
    OpenPitParamPrice price,
    OpenPitParamNotional * out,
    OpenPitOutParamError out_error
);

bool openpit_param_notional_from_volume(
    OpenPitParamVolume volume,
    OpenPitParamNotional * out,
    OpenPitOutParamError out_error
);

bool openpit_param_notional_to_volume(
    OpenPitParamNotional notional,
    OpenPitParamVolume * out,
    OpenPitOutParamError out_error
);

bool openpit_param_notional_calculate_margin_required(
    OpenPitParamNotional notional,
    OpenPitParamLeverage leverage,
    OpenPitParamNotional * out,
    OpenPitOutParamError out_error
);

bool openpit_param_volume_from_notional(
    OpenPitParamNotional notional,
    OpenPitParamVolume * out,
    OpenPitOutParamError out_error
);

/**
 * Constructs an account identifier from a 64-bit integer.
 *
 * This is a direct numeric mapping with no collision risk.
 *
 * WARNING: Do not mix IDs produced by this function with IDs produced by
 * `openpit_create_param_account_id_from_string` in the same runtime state.
 *
 * Contract:
 * - returns a stable account identifier value;
 * - this function always succeeds.
 */
OpenPitParamAccountId openpit_create_param_account_id_from_uint64(
    uint64_t value
);

/**
 * Constructs an account identifier from a UTF-8 byte sequence.
 *
 * The bytes are read only for the duration of the call. No trailing zero byte
 * is required.
 *
 * Collision note:
 * - different account strings can map to the same identifier;
 * - for `n` distinct account strings the probability of at least one
 *   collision is approximately `n^2 / 2^65`.
 * - if collision risk is unacceptable, keep your own collision-free
 *   string-to-integer mapping and use
 *   `openpit_create_param_account_id_from_uint64`.
 *
 * The previous sentence is why this helper is suitable for stable adapter-side
 * mapping, but not for workflows that require guaranteed uniqueness.
 *
 * WARNING: Do not mix IDs produced by this function with IDs produced by
 * `openpit_create_param_account_id_from_uint64` in the same runtime state.
 *
 * Contract:
 * - returns `true` and writes a stable account identifier to `out` on
 *   success;
 * - returns `false` on invalid input and optionally writes
 *   `OpenPitParamError`.
 *
 * # Safety
 *
 * `value.ptr` must be non-null and point to at least `value.len` readable
 * UTF-8 bytes.
 */
bool openpit_create_param_account_id_from_string(
    OpenPitStringView value,
    OpenPitParamAccountId * out,
    OpenPitOutParamError out_error
);

/**
 * Validates and copies an asset identifier into a caller-owned shared-string
 * handle.
 *
 * The returned handle must be destroyed with `openpit_destroy_param_asset`.
 */
OpenPitSharedString * openpit_create_param_asset_from_string(
    OpenPitStringView value,
    OpenPitOutParamError out_error
);

/**
 * Destroys a caller-owned asset handle created by
 * `openpit_create_param_asset_from_string`.
 */
void openpit_destroy_param_asset(
    OpenPitSharedString * handle
);

/**
 * Renders an order side into a caller-owned shared string.
 *
 * Returns null and writes `out_error` when the side is not set.
 */
OpenPitSharedString * openpit_param_side_to_string(
    OpenPitParamSide value,
    OpenPitOutParamError out_error
);

/**
 * Renders a position side into a caller-owned shared string.
 *
 * Returns null and writes `out_error` when the position side is not set.
 */
OpenPitSharedString * openpit_param_position_side_to_string(
    OpenPitParamPositionSide value,
    OpenPitOutParamError out_error
);

/**
 * Renders a position effect into a caller-owned shared string.
 *
 * Returns null and writes `out_error` when the position effect is not set.
 */
OpenPitSharedString * openpit_param_position_effect_to_string(
    OpenPitParamPositionEffect value,
    OpenPitOutParamError out_error
);

/**
 * Renders a position mode into a caller-owned shared string.
 *
 * Returns null and writes `out_error` when the position mode is not set.
 */
OpenPitSharedString * openpit_param_position_mode_to_string(
    OpenPitParamPositionMode value,
    OpenPitOutParamError out_error
);

/**
 * Renders an account identifier into a caller-owned shared string.
 *
 * This conversion always succeeds.
 */
OpenPitSharedString * openpit_param_account_id_to_string(
    OpenPitParamAccountId value
);

/**
 * Renders a trade amount into a caller-owned shared string.
 *
 * Returns null and writes `out_error` when the trade amount is not set or its
 * numeric value cannot be decoded.
 */
OpenPitSharedString * openpit_param_trade_amount_to_string(
    OpenPitParamTradeAmount value,
    OpenPitOutParamError out_error
);

/**
 * Renders an adjustment amount into a caller-owned shared string.
 *
 * Returns null and writes `out_error` when the amount is not set or its
 * numeric value cannot be decoded.
 */
OpenPitSharedString * openpit_param_adjustment_amount_to_string(
    OpenPitParamAdjustmentAmount value,
    OpenPitOutParamError out_error
);

/**
 * Creates a caller-owned reject list with preallocated capacity.
 *
 * `reserve` is the requested number of elements to preallocate.
 *
 * Contract:
 * - returns a new caller-owned list;
 * - release it with `openpit_pretrade_destroy_reject_list`;
 * - this function always succeeds.
 */
OpenPitPretradeRejectList * openpit_pretrade_create_reject_list(
    size_t reserve
);

/**
 * Releases a caller-owned reject list.
 *
 * Contract:
 * - passing null is allowed;
 * - this function always succeeds.
 */
void openpit_pretrade_destroy_reject_list(
    OpenPitPretradeRejectList * rejects
);

/**
 * Appends one reject to the list by copying its payload.
 *
 * Contract:
 * - `list` must be a valid non-null pointer;
 * - string views in `reject` are copied before this function returns;
 * - this function never fails;
 * - violating the pointer contract aborts the call.
 */
void openpit_pretrade_reject_list_push(
    OpenPitPretradeRejectList * list,
    OpenPitPretradeReject reject
);

/**
 * Returns the number of rejects in the list.
 *
 * Contract:
 * - `list` must be a valid non-null pointer;
 * - this function never fails;
 * - violating the pointer contract aborts the call.
 */
size_t openpit_pretrade_reject_list_len(
    const OpenPitPretradeRejectList * list
);

/**
 * Copies a non-owning reject view at `index` into `out_reject`.
 *
 * The copied view borrows string memory from `list`.
 *
 * Contract:
 * - `list` must be a valid non-null pointer;
 * - `out_reject` must be a valid non-null pointer;
 * - returns `true` when a value exists and was copied;
 * - returns `false` when `index` is out of bounds and does not write
 *   `out_reject`;
 * - the copied view remains valid while `list` is alive and unchanged;
 * - this function never fails;
 * - violating the pointer contract aborts the call.
 */
bool openpit_pretrade_reject_list_get(
    const OpenPitPretradeRejectList * list,
    size_t index,
    OpenPitPretradeReject * out_reject
);

/**
 * Creates a caller-owned account-block list with preallocated capacity.
 *
 * `reserve` is the requested number of elements to preallocate.
 *
 * Contract:
 * - returns a new caller-owned list;
 * - release it with `openpit_pretrade_destroy_account_block_list`;
 * - this function always succeeds.
 */
OpenPitPretradeAccountBlockList * openpit_pretrade_create_account_block_list(
    size_t reserve
);

/**
 * Releases a caller-owned account-block list.
 *
 * Contract:
 * - passing null is allowed;
 * - this function always succeeds.
 */
void openpit_pretrade_destroy_account_block_list(
    OpenPitPretradeAccountBlockList * blocks
);

/**
 * Appends one account block to the list by copying its payload.
 *
 * Contract:
 * - `list` must be a valid non-null pointer;
 * - string views in `block` are copied before this function returns;
 * - this function never fails;
 * - violating the pointer contract aborts the call.
 */
void openpit_pretrade_account_block_list_push(
    OpenPitPretradeAccountBlockList * list,
    OpenPitPretradeAccountBlock block
);

/**
 * Returns the number of account blocks in the list.
 *
 * Contract:
 * - `list` must be a valid non-null pointer;
 * - this function never fails;
 * - violating the pointer contract aborts the call.
 */
size_t openpit_pretrade_account_block_list_len(
    const OpenPitPretradeAccountBlockList * list
);

/**
 * Copies a non-owning account-block view at `index` into `out_block`.
 *
 * The copied view borrows string memory from `list`.
 *
 * Contract:
 * - `list` must be a valid non-null pointer;
 * - `out_block` must be a valid non-null pointer;
 * - returns `true` when a value exists and was copied;
 * - returns `false` when `index` is out of bounds and does not write
 *   `out_block`;
 * - the copied view remains valid while `list` is alive and unchanged;
 * - this function never fails;
 * - violating the pointer contract aborts the call.
 */
bool openpit_pretrade_account_block_list_get(
    const OpenPitPretradeAccountBlockList * list,
    size_t index,
    OpenPitPretradeAccountBlock * out_block
);

/**
 * Releases a caller-owned parameter error container.
 *
 * # Safety
 *
 * `handle` must be either null or a pointer returned by this library through
 * `OpenPitOutParamError`. The handle must be destroyed at most once.
 */
void openpit_destroy_param_error(
    OpenPitParamError * handle
);

/**
 * Creates a new engine builder with the chosen synchronization policy.
 *
 * Success:
 * - returns a non-null caller-owned builder object.
 *
 * Error:
 * - returns null when `sync_policy` is not one of `OpenPitSyncPolicy_None`
 *   (0), `OpenPitSyncPolicy_Full` (1), or `OpenPitSyncPolicy_Account` (2);
 * - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
 *   error handle that MUST be released with `openpit_destroy_shared_string`.
 *
 * Cleanup:
 * - release the pointer with `openpit_destroy_engine_builder` if you stop
 *   before building;
 * - after a successful build the builder is consumed and must still be
 *   released with `openpit_destroy_engine_builder`.
 */
OpenPitEngineBuilder * openpit_create_engine_builder(
    uint8_t sync_policy,
    OpenPitOutError out_error
);

/**
 * Releases a builder pointer owned by the caller.
 *
 * Contract:
 * - passing null is allowed;
 * - after this call the pointer is invalid;
 * - this function always succeeds.
 */
void openpit_destroy_engine_builder(
    OpenPitEngineBuilder * builder
);

/**
 * Finalizes a builder and creates an engine.
 *
 * Success:
 * - returns a non-null engine pointer.
 *
 * Error:
 * - returns null when `builder` is null, the builder was already consumed,
 *   or no policies were registered;
 * - for those non-domain failures, if `out_error` is not null, writes a
 *   caller-owned `OpenPitSharedString` error handle that MUST be released
 *   with `openpit_destroy_shared_string`, and writes null to
 *   `out_build_error` if it is not null;
 * - returns null when the configuration is rejected during building (for
 *   example, duplicate policy names or duplicate group ids); in that case,
 *   if `out_build_error` is not null, writes a caller-owned
 *   `OpenPitEngineBuildError` pointer that carries the machine-readable
 *   failure code and the offending value, and MUST be released with
 *   `openpit_destroy_engine_build_error`; `out_error` is left untouched for
 *   this domain failure.
 *
 * Ownership:
 * - on success the returned engine pointer is owned by the caller and must
 *   be released with `openpit_destroy_engine`; on success, null is written
 *   to `out_build_error` if it is not null;
 * - the builder becomes consumed regardless of success and must not be
 *   reused.
 */
OpenPitEngine * openpit_engine_builder_build(
    OpenPitEngineBuilder * builder,
    OpenPitEngineBuildError ** out_build_error,
    OpenPitOutError out_error
);

/**
 * Releases a build-error object returned by engine construction.
 *
 * Contract:
 * - passing null is allowed;
 * - this function always succeeds.
 */
void openpit_destroy_engine_build_error(
    OpenPitEngineBuildError * build_error
);

/**
 * Returns the machine-readable failure category of a build error.
 *
 * Contract:
 * - `build_error` must be a valid non-null pointer;
 * - this function never fails;
 * - violating the pointer contract aborts the call.
 */
OpenPitEngineBuildErrorCode openpit_engine_build_error_get_code(
    const OpenPitEngineBuildError * build_error
);

/**
 * Returns a non-owning view of the offending policy name from a build error.
 *
 * Contract:
 * - `build_error` must be a valid non-null pointer;
 * - the returned view points into memory owned by `build_error` and is valid
 *   while `build_error` is alive; it must not be used after the build error
 *   is destroyed;
 * - the view is empty unless the failure category is the
 *   duplicate-policy-name category;
 * - this function never fails;
 * - violating the pointer contract aborts the call.
 */
OpenPitStringView openpit_engine_build_error_get_policy_name(
    const OpenPitEngineBuildError * build_error
);

/**
 * Returns the offending policy group id from a build error.
 *
 * Contract:
 * - `build_error` must be a valid non-null pointer;
 * - the value is zero unless the failure category is the
 *   duplicate-policy-group-id category;
 * - this function never fails;
 * - violating the pointer contract aborts the call.
 */
uint16_t openpit_engine_build_error_get_policy_group_id(
    const OpenPitEngineBuildError * build_error
);

/**
 * Releases an engine pointer owned by the caller.
 *
 * Contract:
 * - passing null is allowed;
 * - destroying the engine also releases any state and policies retained by
 *   that engine instance;
 * - this function always succeeds.
 */
void openpit_destroy_engine(
    OpenPitEngine * engine
);

/**
 * Starts pre-trade processing and returns a deferred request pointer.
 *
 * This stage validates whether the order can enter the full pre-trade flow.
 *
 * Success:
 * - returns `Passed` when the order passed this stage; read `out_request`;
 * - returns `Rejected` when the order was rejected; read `out_rejects` if
 *   not null.
 *
 * Error:
 * - returns `Error` when input pointers are invalid or the order payload
 *   cannot be decoded;
 * - on `Error`, if `out_error` is not null, it is filled with a caller-owned
 *   `OpenPitSharedString` that MUST be destroyed by the caller.
 *
 * Cleanup:
 * - release a successful request with
 *   `openpit_pretrade_pre_trade_request_execute` or
 *   `openpit_destroy_pretrade_pre_trade_request`.
 *
 * Reject ownership contract:
 * - on `Rejected`, a non-null `OpenPitPretradeRejectList` pointer is written
 *   to `out_rejects` if it is not null;
 * - the caller takes ownership and MUST release it with
 *   `openpit_pretrade_destroy_reject_list`; failing to do so leaks the
 *   memory allocated inside this call;
 * - no thread-local state is involved, and the returned pointer is safe to
 *   read on any thread;
 * - on `Passed` and `Error`, null is written to `out_rejects`, and the
 *   caller must not call destroy in those cases.
 *
 * Order lifetime contract:
 * - `order` is read as a borrowed view during this call;
 * - the operation snapshots that payload before returning, because the
 *   deferred request may outlive the source buffers.
 */
OpenPitPretradeStatus openpit_engine_start_pre_trade(
    OpenPitEngine * engine,
    const OpenPitOrder * order,
    OpenPitPretradePreTradeRequest ** out_request,
    OpenPitPretradeRejectList ** out_rejects,
    OpenPitOutError out_error
);

/**
 * Runs the complete pre-trade check in one call.
 *
 * Success:
 * - returns `Passed` when the order passed this stage; read
 *   `out_reservation`;
 * - returns `Rejected` when the order was rejected is not null; read
 *   `out_rejects`.
 *
 * Error:
 * - returns `Error` when input pointers are invalid or the order payload
 *   cannot be decoded;
 * - on `Error`, if `out_error` is not null, it is filled with a caller-owned
 *   `OpenPitSharedString` that MUST be destroyed by the caller.
 *
 * Cleanup:
 * - release a successful reservation with
 *   `openpit_pretrade_pre_trade_reservation_commit`,
 *   `openpit_pretrade_pre_trade_reservation_rollback`, or
 *   `openpit_destroy_pretrade_pre_trade_reservation`.
 *
 * Reject ownership contract:
 * - on `Rejected`, a non-null `OpenPitPretradeRejectList` pointer is written
 *   to `out_rejects` if it is not null;
 * - the caller takes ownership and MUST release it with
 *   `openpit_pretrade_destroy_reject_list`; failing to do so leaks the
 *   memory allocated inside this call;
 * - no thread-local state is involved, and the returned pointer is safe to
 *   read on any thread;
 * - on `Passed` and `Error`, null is written to `out_rejects`, and the
 *   caller must not call destroy in those cases.
 *
 * Order lifetime contract:
 * - `order` is read as a borrowed view during this call only;
 * - the operation does not retain any pointer into source memory after this
 *   function returns.
 */
OpenPitPretradeStatus openpit_engine_execute_pre_trade(
    OpenPitEngine * engine,
    const OpenPitOrder * order,
    OpenPitPretradePreTradeReservation ** out_reservation,
    OpenPitPretradeRejectList ** out_rejects,
    OpenPitOutError out_error
);

/**
 * Executes a deferred request returned by `openpit_engine_start_pre_trade`.
 *
 * Success:
 * - returns `Passed` when the order passed this stage; read
 *   `out_reservation`;
 * - returns `Rejected` when the order was rejected and `out_rejects` is not
 *   null; read `out_rejects`.
 *
 * Error:
 * - returns `Error` when input pointers are invalid or the order payload
 *   cannot be decoded;
 * - on `Error`, if `out_error` is not null, it is filled with a caller-owned
 *   `OpenPitSharedString` that MUST be destroyed by the caller.
 *
 * Ownership:
 * - this call consumes the request object's content exactly once;
 * - after a successful or failed execute, the object itself may still be
 *   released with `openpit_destroy_pretrade_pre_trade_request`, but it
 *   cannot be executed again.
 *
 * Reject ownership contract:
 * - on `Rejected`, a non-null `OpenPitPretradeRejectList` pointer is written
 *   to `out_rejects` if it is not null;
 * - the caller takes ownership and MUST release it with
 *   `openpit_pretrade_destroy_reject_list`; failing to do so leaks the
 *   memory allocated inside this call;
 * - no thread-local state is involved, and the returned pointer is safe to
 *   read on any thread;
 * - on `Passed` and `Error`, null is written to `out_rejects`, and the
 *   caller must not call destroy in those cases.
 */
OpenPitPretradeStatus openpit_pretrade_pre_trade_request_execute(
    OpenPitPretradePreTradeRequest * request,
    OpenPitPretradePreTradeReservation ** out_reservation,
    OpenPitPretradeRejectList ** out_rejects,
    OpenPitOutError out_error
);

/**
 * Releases a deferred request pointer owned by the caller.
 *
 * Contract:
 * - passing null is allowed;
 * - destroying an unexecuted request abandons it without creating a
 *   reservation;
 * - this function always succeeds.
 */
void openpit_destroy_pretrade_pre_trade_request(
    OpenPitPretradePreTradeRequest * request
);

/**
 * Finalizes a reservation and applies the reserved state permanently.
 *
 * This call is idempotent at the pointer level: if the reservation was already
 * consumed, nothing happens. Passing null is allowed.
 *
 * Contract:
 * - passing null is allowed;
 * - this function always succeeds.
 */
void openpit_pretrade_pre_trade_reservation_commit(
    OpenPitPretradePreTradeReservation * reservation
);

/**
 * Cancels a reservation and releases the reserved state.
 *
 * This call is idempotent at the pointer level: if the reservation was already
 * consumed, nothing happens. Passing null is allowed.
 *
 * Contract:
 * - passing null is allowed;
 * - this function always succeeds.
 */
void openpit_pretrade_pre_trade_reservation_rollback(
    OpenPitPretradePreTradeReservation * reservation
);

/**
 * Returns a snapshot of the lock attached to a reservation.
 *
 * Contract:
 * - `reservation` must be a valid non-null pointer;
 * - violating the pointer contract aborts the call;
 * - this function never fails.
 *
 * Lifetime contract:
 * - the returned snapshot is detached from the reservation state.
 */
OpenPitPretradePreTradeLock * openpit_pretrade_pre_trade_reservation_get_lock(
    const OpenPitPretradePreTradeReservation * reservation
);

/**
 * Returns the account-adjustment outcomes collected by the reservation.
 *
 * Contract:
 * - `reservation` must be a valid non-null pointer;
 * - violating the pointer contract aborts the call;
 * - this function never fails;
 * - always returns a caller-owned `OpenPitAccountAdjustmentOutcomeList`
 *   (possibly empty); release it with
 *   `openpit_destroy_account_adjustment_outcome_list`.
 *
 * Lifetime contract:
 * - the returned list is detached from the reservation state.
 */
OpenPitAccountAdjustmentOutcomeList *
openpit_pretrade_pre_trade_reservation_get_account_adjustments(
    const OpenPitPretradePreTradeReservation * reservation
);

/**
 * Releases a reservation pointer owned by the caller.
 *
 * Contract:
 * - passing null is allowed;
 * - destroying an unresolved reservation triggers rollback of any pending
 *   mutations;
 * - callers that need explicit resolution should call commit or rollback
 *   first;
 * - this function always succeeds.
 */
void openpit_destroy_pretrade_pre_trade_reservation(
    OpenPitPretradePreTradeReservation * reservation
);

/**
 * Applies an execution report to engine state.
 *
 * Returns `true` on success, `false` on error.
 *
 * Success:
 * - returns `true`;
 * - if `out_blocks` is not null and at least one policy entered a blocked
 *   state, writes a caller-owned `OpenPitPretradeAccountBlockList` pointer;
 *   release it with `openpit_pretrade_destroy_account_block_list`;
 * - if `out_blocks` is not null and no policy blocked, writes null.
 * - if `out_adjustments` is not null and at least one policy produced an
 *   account-adjustment outcome, writes a caller-owned
 *   `OpenPitAccountAdjustmentOutcomeList` pointer; release it with
 *   `openpit_destroy_account_adjustment_outcome_list`;
 * - if `out_adjustments` is not null and no outcome was produced, writes
 *   null.
 *
 * Error:
 * - returns `false` when input pointers are invalid or the report payload
 *   cannot be decoded;
 * - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
 *   error handle that MUST be released with `openpit_destroy_shared_string`.
 *
 * Lifetime contract:
 * - `report` is read as a borrowed view during this call only;
 * - the operation does not retain any pointer into source memory after this
 *   function returns.
 */
bool openpit_engine_apply_execution_report(
    OpenPitEngine * engine,
    const OpenPitExecutionReport * report,
    OpenPitPretradeAccountBlockList ** out_blocks,
    OpenPitAccountAdjustmentOutcomeList ** out_adjustments,
    OpenPitOutError out_error
);

/**
 * Releases a batch-error object returned by account-adjustment apply.
 *
 * Contract:
 * - passing null is allowed;
 * - this function always succeeds.
 */
void openpit_destroy_account_adjustment_batch_error(
    OpenPitAccountAdjustmentBatchError * batch_error
);

/**
 * Returns the failing adjustment index from a batch error.
 *
 * Contract:
 * - `batch_error` must be a valid non-null pointer;
 * - this function never fails;
 * - violating the pointer contract aborts the call.
 */
size_t openpit_account_adjustment_batch_error_get_failed_adjustment_index(
    const OpenPitAccountAdjustmentBatchError * batch_error
);

/**
 * Returns a non-owning reject-list view from a batch error.
 *
 * Contract:
 * - `batch_error` must be a valid non-null pointer;
 * - the returned pointer is valid while `batch_error` is alive;
 * - this function never fails;
 * - violating the pointer contract aborts the call.
 */
const OpenPitPretradeRejectList *
openpit_account_adjustment_batch_error_get_rejects(
    const OpenPitAccountAdjustmentBatchError * batch_error
);

/**
 * Applies a batch of account adjustments to one account.
 *
 * Success:
 * - returns `OpenPitAccountAdjustmentApplyStatus::Applied` when the batch
 *   was accepted and applied;
 * - returns `OpenPitAccountAdjustmentApplyStatus::Rejected` when the call
 *   itself completed normally but a policy rejected the batch; read
 *   `out_reject`.
 *
 * Error:
 * - returns `OpenPitAccountAdjustmentApplyStatus::Error` when input pointers
 *   are invalid or some adjustment payload cannot be decoded;
 * - on `Error`, if `out_error` is not null, it is filled with a caller-owned
 *   `OpenPitSharedString` that MUST be destroyed by the caller.
 *
 * Result handling:
 * - `Applied` means there is no reject object to clean up;
 * - on `Applied`, if `out_outcomes` is not null and at least one policy
 *   produced an account-adjustment outcome, writes a caller-owned
 *   `OpenPitAccountAdjustmentOutcomeList` pointer; release it with
 *   `openpit_destroy_account_adjustment_outcome_list`; if no outcome was
 *   produced, writes null;
 * - `Rejected` stores batch error details in `out_reject`, the caller must
 *   release a returned object with
 *   `openpit_destroy_account_adjustment_batch_error`;
 * - rejects returned by `openpit_account_adjustment_batch_error_get_rejects`
 *   contain string views borrowed from the batch error and must not be used
 *   after the batch error is destroyed;
 * - when `Error` is returned, do not use any pointer from a previous
 *   unrelated call as if it belonged to this failure.
 *
 * Lifetime contract:
 * - every `adjustment` entry from the contiguous input array is read as a
 *   borrowed view during this call only;
 * - release a returned batch error with
 *   `openpit_destroy_account_adjustment_batch_error`.
 */
OpenPitAccountAdjustmentApplyStatus openpit_engine_apply_account_adjustment(
    OpenPitEngine * engine,
    OpenPitParamAccountId account_id,
    const OpenPitAccountAdjustment * adjustments,
    size_t adjustments_len,
    OpenPitAccountAdjustmentBatchError ** out_reject,
    OpenPitAccountAdjustmentOutcomeList ** out_outcomes,
    OpenPitOutError out_error
);

/**
 * Releases a caller-owned account-group error.
 *
 * Contract:
 * - call exactly once per pointer returned by a registry function;
 * - passing null is allowed and has no effect.
 */
void openpit_destroy_account_group_error(
    OpenPitAccountGroupError * err
);

/**
 * Returns the human-readable error message from an account-group error.
 *
 * Contract:
 * - `err` must be a valid non-null pointer;
 * - the returned view borrows from the error object and is valid while the
 *   error is alive;
 * - violating the pointer contract aborts the call.
 */
OpenPitStringView openpit_account_group_error_get_message(
    const OpenPitAccountGroupError * err
);

/**
 * Returns the offending account identifier from an account-group error.
 *
 * Contract:
 * - `err` must be a valid non-null pointer;
 * - this function never fails;
 * - violating the pointer contract aborts the call.
 */
OpenPitParamAccountId openpit_account_group_error_get_account(
    const OpenPitAccountGroupError * err
);

/**
 * Returns the current group of the offending account from an account-group
 * error, or writes zero and returns `false` when no group is present.
 *
 * Contract:
 * - `err` must be a valid non-null pointer;
 * - `out_group` must be a valid non-null pointer;
 * - returns `true` when the account belongs to a group and writes that group
 *   to `out_group`;
 * - returns `false` when the account belongs to no group; `out_group` is
 *   written to only when the return value is `true`;
 * - violating the pointer contract aborts the call.
 */
bool openpit_account_group_error_get_current_group(
    const OpenPitAccountGroupError * err,
    OpenPitParamAccountGroupId * out_group
);

/**
 * Atomically registers every account in `accounts` into `group`.
 *
 * The operation is all-or-nothing: if any listed account is already a member
 * of any group (including `group`), no account is registered.
 *
 * Contract:
 * - `engine` must be a valid non-null engine pointer;
 * - `accounts` must point to an array of at least `accounts_len` account
 *   identifiers, or may be null when `accounts_len` is zero;
 * - `group` is the target group and must not be the reserved
 *   `OPENPIT_DEFAULT_ACCOUNT_GROUP`.
 *
 * Success:
 * - returns `true`; all listed accounts are now members of `group`.
 *
 * Error:
 * - returns `false` when `engine` is null, `accounts` is null with non-zero
 *   length, `group` is the reserved default group, or any listed account is
 *   already registered;
 * - for pointer/argument errors, if `out_error` is not null, writes a
 *   caller-owned `OpenPitSharedString` error handle that MUST be released
 *   with `openpit_destroy_shared_string`;
 * - for domain errors (reserved target group, or account already
 *   registered), if `out_group_error` is not null, writes a caller-owned
 *   `OpenPitAccountGroupError` pointer that MUST be released with
 *   `openpit_destroy_account_group_error`; `out_error` is left untouched for
 *   domain failures.
 */
bool openpit_engine_register_account_group(
    OpenPitEngine * engine,
    const OpenPitParamAccountId * accounts,
    size_t accounts_len,
    OpenPitParamAccountGroupId group,
    OpenPitAccountGroupError ** out_group_error,
    OpenPitOutError out_error
);

/**
 * Atomically removes every account in `accounts` from `group`.
 *
 * The operation is all-or-nothing: if any listed account is not currently a
 * member of `group`, no account is removed.
 *
 * Contract:
 * - `engine` must be a valid non-null engine pointer;
 * - `accounts` must point to an array of at least `accounts_len` account
 *   identifiers, or may be null when `accounts_len` is zero;
 * - `group` is the group to remove accounts from and must not be the
 *   reserved `OPENPIT_DEFAULT_ACCOUNT_GROUP`.
 *
 * Success:
 * - returns `true`; all listed accounts are now removed from `group`.
 *
 * Error:
 * - returns `false` when `engine` is null, `accounts` is null with non-zero
 *   length, `group` is the reserved default group, or any listed account is
 *   not in `group`;
 * - for pointer/argument errors, if `out_error` is not null, writes a
 *   caller-owned `OpenPitSharedString` error handle that MUST be released
 *   with `openpit_destroy_shared_string`;
 * - for domain errors (reserved target group, or account not in group), if
 *   `out_group_error` is not null, writes a caller-owned
 *   `OpenPitAccountGroupError` pointer that MUST be released with
 *   `openpit_destroy_account_group_error`; `out_error` is left untouched for
 *   domain failures.
 */
bool openpit_engine_unregister_account_group(
    OpenPitEngine * engine,
    const OpenPitParamAccountId * accounts,
    size_t accounts_len,
    OpenPitParamAccountGroupId group,
    OpenPitAccountGroupError ** out_group_error,
    OpenPitOutError out_error
);

/**
 * Returns the account-group membership of a single account.
 *
 * Contract:
 * - `engine` must be a valid non-null engine pointer;
 * - `account` is the account identifier to look up;
 * - `out_group` must be a valid non-null pointer.
 *
 * Success:
 * - returns `true` when the account belongs to a group and writes that group
 *   identifier to `out_group`;
 * - returns `false` when the account belongs to no group; `out_group` is not
 *   written to when the return value is `false`.
 *
 * Error:
 * - aborts the call when `engine` or `out_group` is null.
 */
bool openpit_engine_account_group(
    const OpenPitEngine * engine,
    OpenPitParamAccountId account,
    OpenPitParamAccountGroupId * out_group
);

/**
 * Releases a caller-owned account-block error.
 *
 * Contract:
 * - call exactly once per pointer returned by a block function;
 * - passing null is allowed and has no effect.
 */
void openpit_destroy_account_block_error(
    OpenPitAccountBlockError * err
);

/**
 * Returns the human-readable error message from an account-block error.
 *
 * Contract:
 * - `err` must be a valid non-null pointer;
 * - the returned view borrows from the error object and is valid while the
 *   error is alive;
 * - violating the pointer contract aborts the call.
 */
OpenPitStringView openpit_account_block_error_get_message(
    const OpenPitAccountBlockError * err
);

/**
 * Returns the variant kind of an account-block error.
 *
 * Contract:
 * - `err` must be a valid non-null pointer;
 * - this function never fails;
 * - violating the pointer contract aborts the call.
 */
OpenPitAccountBlockErrorKind openpit_account_block_error_get_kind(
    const OpenPitAccountBlockError * err
);

/**
 * Returns the offending account identifier from an account-block error.
 *
 * Contract:
 * - `err` must be a valid non-null pointer;
 * - `out_account` must be a valid non-null pointer;
 * - returns `true` when the error variant carries an account and writes it
 *   to `out_account`;
 * - returns `false` when no account is present; `out_account` is left
 *   untouched when the return value is `false`;
 * - violating the pointer contract aborts the call.
 */
bool openpit_account_block_error_get_account(
    const OpenPitAccountBlockError * err,
    OpenPitParamAccountId * out_account
);

/**
 * Returns the offending account-group identifier from an account-block error.
 *
 * Contract:
 * - `err` must be a valid non-null pointer;
 * - `out_group` must be a valid non-null pointer;
 * - returns `true` when the error variant carries a group and writes it to
 *   `out_group`;
 * - returns `false` when no group is present; `out_group` is left untouched
 *   when the return value is `false`;
 * - violating the pointer contract aborts the call.
 */
bool openpit_account_block_error_get_group(
    const OpenPitAccountBlockError * err,
    OpenPitParamAccountGroupId * out_group
);

/**
 * Blocks `account` with `reason`.
 *
 * The first cause for an account wins: if the account is already blocked (by
 * an admin call or a prior kill-switch), this call is a no-op and does not
 * overwrite the stored reason. Use
 * `openpit_engine_replace_account_block_reason` to change the stored reason.
 *
 * Contract:
 * - `engine` must be a valid non-null engine pointer;
 * - `reason` is interpreted as UTF-8; an empty string is used when
 *   `reason.ptr` is null OR `reason.len` is zero; passing a null `ptr` with
 *   a non-zero `len` is caller misuse and is treated as empty (not read); an
 *   empty reason is explicitly allowed;
 * - violating the `engine` pointer contract aborts the call.
 */
void openpit_engine_block_account(
    OpenPitEngine * engine,
    OpenPitParamAccountId account_id,
    OpenPitStringView reason
);

/**
 * Unblocks `account`, clearing any block on it.
 *
 * Idempotent: a no-op when `account` is not blocked. Both admin blocks and
 * kill-switch blocks are cleared.
 *
 * Contract:
 * - `engine` must be a valid non-null engine pointer;
 * - violating the pointer contract aborts the call.
 */
void openpit_engine_unblock_account(
    OpenPitEngine * engine,
    OpenPitParamAccountId account_id
);

/**
 * Replaces the stored reason of an already-blocked account.
 *
 * Unlike `openpit_engine_block_account`, which preserves the first cause, this
 * overwrites the stored cause with `reason`, leaving the account blocked.
 *
 * Contract:
 * - `engine` must be a valid non-null engine pointer;
 * - `reason` is interpreted as UTF-8; an empty string is used when
 *   `reason.ptr` is null OR `reason.len` is zero; passing a null `ptr` with
 *   a non-zero `len` is caller misuse and is treated as empty (not read); an
 *   empty reason is explicitly allowed;
 * - on failure, if `out_error` is not null, writes a caller-owned
 *   `OpenPitAccountBlockError` pointer that MUST be released with
 *   `openpit_destroy_account_block_error`;
 * - aborts the call when `engine` is null.
 *
 * Success:
 * - returns `true`; the stored reason has been replaced.
 *
 * Error:
 * - returns `false` with `OpenPitAccountBlockErrorKind_AccountNotBlocked`
 *   when `account` is not currently blocked.
 */
bool openpit_engine_replace_account_block_reason(
    OpenPitEngine * engine,
    OpenPitParamAccountId account_id,
    OpenPitStringView reason,
    OpenPitAccountBlockError ** out_error
);

/**
 * Blocks the account group `group` with `reason`.
 *
 * The first cause for a group wins: re-blocking an already-blocked group is a
 * no-op. Use `openpit_engine_replace_account_group_block_reason` to change the
 * stored reason.
 *
 * Contract:
 * - `engine` must be a valid non-null engine pointer;
 * - `group` must not be `OPENPIT_DEFAULT_ACCOUNT_GROUP`;
 * - `reason` is interpreted as UTF-8; an empty string is used when
 *   `reason.ptr` is null OR `reason.len` is zero; passing a null `ptr` with
 *   a non-zero `len` is caller misuse and is treated as empty (not read); an
 *   empty reason is explicitly allowed;
 * - on failure, if `out_error` is not null, writes a caller-owned
 *   `OpenPitAccountBlockError` pointer that MUST be released with
 *   `openpit_destroy_account_block_error`;
 * - aborts the call when `engine` is null.
 *
 * Success:
 * - returns `true`; the group is now blocked.
 *
 * Error:
 * - returns `false` with `OpenPitAccountBlockErrorKind_ReservedGroup` when
 *   `group` is the reserved default group.
 */
bool openpit_engine_block_account_group(
    OpenPitEngine * engine,
    OpenPitParamAccountGroupId group,
    OpenPitStringView reason,
    OpenPitAccountBlockError ** out_error
);

/**
 * Unblocks the account group `group`, clearing the group block.
 *
 * Idempotent: a no-op when `group` is not blocked. Accounts blocked
 * individually remain blocked.
 *
 * Contract:
 * - `engine` must be a valid non-null engine pointer;
 * - `group` must not be `OPENPIT_DEFAULT_ACCOUNT_GROUP`;
 * - on failure, if `out_error` is not null, writes a caller-owned
 *   `OpenPitAccountBlockError` pointer that MUST be released with
 *   `openpit_destroy_account_block_error`;
 * - aborts the call when `engine` is null.
 *
 * Success:
 * - returns `true`; the group is now unblocked.
 *
 * Error:
 * - returns `false` with `OpenPitAccountBlockErrorKind_ReservedGroup` when
 *   `group` is the reserved default group.
 */
bool openpit_engine_unblock_account_group(
    OpenPitEngine * engine,
    OpenPitParamAccountGroupId group,
    OpenPitAccountBlockError ** out_error
);

/**
 * Replaces the stored reason of an already-blocked account group.
 *
 * Unlike `openpit_engine_block_account_group`, which preserves the first
 * cause, this overwrites the stored cause with `reason`, leaving the group
 * blocked.
 *
 * Contract:
 * - `engine` must be a valid non-null engine pointer;
 * - `group` must not be `OPENPIT_DEFAULT_ACCOUNT_GROUP`;
 * - `reason` is interpreted as UTF-8; an empty string is used when
 *   `reason.ptr` is null OR `reason.len` is zero; passing a null `ptr` with
 *   a non-zero `len` is caller misuse and is treated as empty (not read); an
 *   empty reason is explicitly allowed;
 * - on failure, if `out_error` is not null, writes a caller-owned
 *   `OpenPitAccountBlockError` pointer that MUST be released with
 *   `openpit_destroy_account_block_error`;
 * - aborts the call when `engine` is null.
 *
 * Success:
 * - returns `true`; the stored group-block reason has been replaced.
 *
 * Error:
 * - returns `false` with `OpenPitAccountBlockErrorKind_ReservedGroup` when
 *   `group` is the reserved default group;
 * - returns `false` with `OpenPitAccountBlockErrorKind_GroupNotBlocked` when
 *   `group` is not currently blocked.
 */
bool openpit_engine_replace_account_group_block_reason(
    OpenPitEngine * engine,
    OpenPitParamAccountGroupId group,
    OpenPitStringView reason,
    OpenPitAccountBlockError ** out_error
);

/**
 * Creates a custom pre-trade policy from caller-provided callbacks.
 *
 * Contract:
 * - `name` must point to a valid, null-terminated string for the duration of
 *   the call.
 * - `policy_group_id` is the policy-group tag the engine embeds in every
 *   account adjustment outcome this policy produces. Use `0` for the default
 *   group.
 * - `check_pre_trade_start_fn`, `perform_pre_trade_check_fn`,
 *   `apply_execution_report_fn`, and `apply_account_adjustment_fn` may be
 *   null.
 * - A null `check_pre_trade_start_fn`, `perform_pre_trade_check_fn`, or
 *   `apply_account_adjustment_fn` means that hook accepts by default.
 * - A null `apply_execution_report_fn` means that hook returns an empty list
 *   (no kill switch).
 * - Non-null callbacks and `free_user_data_fn` must remain callable for as
 *   long as the policy may still be used by either the caller pointer or the
 *   engine.
 * - Custom main-stage and account-adjustment callbacks can register
 *   commit/rollback mutations through their `mutations` pointer.
 * - `free_user_data_fn` will be called exactly once, when the last reference
 *   to the policy is released.
 * - `user_data` is opaque to the SDK: the engine never inspects,
 *   dereferences, or frees it; it is forwarded verbatim to the registered
 *   callbacks. Lifetime, thread-safety, and meaning of the pointed-at state
 *   are entirely the caller's responsibility. Under `OpenPitSyncPolicy_None`
 *   or `OpenPitSyncPolicy_Account`, the caller serialises per-handle
 *   invocation per the SDK threading contract; under
 *   `OpenPitSyncPolicy_Full`, the caller is responsible for making any state
 *   reachable through `user_data` safe under concurrent invocation.
 *
 * Success:
 * - returns a new caller-owned policy object.
 *
 * Error:
 * - returns null when `name` is invalid;
 * - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
 *   error handle that MUST be released with `openpit_destroy_shared_string`.
 *
 * Lifetime contract:
 * - The policy stores its own copy of `name`; the caller may release the
 *   input string after this function returns.
 * - The returned pointer is owned by the caller and must be released with
 *   `openpit_destroy_pretrade_pre_trade_policy` when no longer needed.
 * - If the policy is added to the engine builder, the engine keeps its own
 *   reference, but the caller must still release the caller-owned pointer.
 * - `free_user_data_fn` runs once the last reference to the policy is
 *   released; when the engine is the final holder, it runs as part of engine
 *   destruction.
 */
OpenPitPretradePreTradePolicy * openpit_create_pretrade_custom_pre_trade_policy(
    OpenPitStringView name,
    uint16_t policy_group_id,
    OpenPitPretradePreTradePolicyCheckPreTradeStartFn check_pre_trade_start_fn,
    OpenPitPretradePreTradePolicyPerformPreTradeCheckFn perform_pre_trade_check_fn,
    OpenPitPretradePreTradePolicyApplyExecutionReportFn apply_execution_report_fn,
    OpenPitPretradePreTradePolicyApplyAccountAdjustmentFn apply_account_adjustment_fn,
    OpenPitPretradePreTradePolicyFreeUserDataFn free_user_data_fn,
    void * user_data,
    OpenPitOutError out_error
);

/**
 * Destroys the caller-owned pointer for a pre-trade policy.
 *
 * Lifetime contract:
 * - Call this exactly once for each pointer that was returned to the caller
 *   by a custom policy create function.
 * - After this call the pointer is no longer valid.
 * - Passing a null pointer is allowed and has no effect.
 * - This function always succeeds.
 * - If the policy was previously added to the engine builder, the engine
 *   keeps its own reference and may continue using the policy.
 * - Destroying this caller-owned pointer does not remove the policy from the
 *   engine.
 */
void openpit_destroy_pretrade_pre_trade_policy(
    OpenPitPretradePreTradePolicy * policy
);

/**
 * Returns the stable policy name for a pre-trade policy pointer.
 *
 * Contract:
 * - This function never fails.
 * - `policy` must be a valid non-null pointer.
 * - The returned view does not own memory.
 * - The view remains valid while the policy object is alive and its name is
 *   not changed.
 * - Passing an invalid pointer aborts the call.
 */
OpenPitStringView openpit_pretrade_pre_trade_policy_get_name(
    const OpenPitPretradePreTradePolicy * policy
);

/**
 * Adds a pre-trade policy to the engine builder.
 *
 * Contract:
 * - `builder` must be a valid engine builder pointer.
 * - `policy` must be a valid non-null pre-trade policy pointer.
 *
 * Success:
 * - returns `true` and the builder retains its own reference to the policy.
 *
 * Error:
 * - returns `false` when the builder or policy cannot be used;
 * - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
 *   error handle that MUST be released with `openpit_destroy_shared_string`.
 *
 * Lifetime contract:
 * - The engine builder retains its own reference to the policy object.
 * - The caller still owns the passed pointer and must release that local
 *   pointer separately with `openpit_destroy_pretrade_pre_trade_policy` when
 *   it is no longer needed.
 */
bool openpit_engine_builder_add_pre_trade_policy(
    OpenPitEngineBuilder * builder,
    OpenPitPretradePreTradePolicy * policy,
    OpenPitOutError out_error
);

/**
 * Registers one commit/rollback mutation in the provided collector.
 *
 * Contract:
 * - `mutations` must be a valid non-null callback-scoped pointer.
 * - `commit_fn` and `rollback_fn` must remain callable until one of them is
 *   executed.
 * - `user_data` is passed to both callbacks.
 * - Exactly one of `commit_fn` or `rollback_fn` runs for each successful
 *   push.
 * - After the executed callback returns, `free_fn` is called exactly once
 *   when provided.
 * - If neither callback runs (for example collector drop), only `free_fn`
 *   runs exactly once when provided.
 *
 * Error:
 * - returns `false` when `mutations` is null or invalid;
 * - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
 *   error handle that MUST be released with `openpit_destroy_shared_string`.
 */
bool openpit_mutations_push(
    OpenPitMutations * mutations,
    OpenPitMutationFn commit_fn,
    OpenPitMutationFn rollback_fn,
    void * user_data,
    OpenPitMutationFreeFn free_fn,
    OpenPitOutError out_error
);

/**
 * Adds the built-in order-size limit policy to the engine builder.
 *
 * Contract:
 * - `builder` must be a valid engine builder pointer.
 * - `policy_group_id` assigns the policy to a policy group (pass `0` for
 *   default).
 * - At least one barrier axis must be configured: `broker` non-null,
 *   `asset_len > 0`, or `account_asset_len > 0`.
 * - When a length is greater than zero the corresponding pointer must point
 *   to that many readable entries.
 * - Each `settlement_asset` string view inside an array entry must be valid
 *   for the duration of the call.
 * - `max_quantity` and `max_notional` inside each limit must be valid.
 *
 * Success:
 * - returns `true`; the builder retains the policy.
 *
 * Error:
 * - returns `false` when the builder is null or already consumed, when no
 *   barrier axis is configured, or when argument parsing fails;
 * - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
 *   error handle that MUST be released with `openpit_destroy_shared_string`.
 */
bool openpit_engine_builder_add_builtin_order_size_limit_policy(
    OpenPitEngineBuilder * builder,
    uint16_t policy_group_id,
    const OpenPitPretradePoliciesOrderSizeBrokerBarrier * broker,
    const OpenPitPretradePoliciesOrderSizeAssetBarrier * asset,
    size_t asset_len,
    const OpenPitPretradePoliciesOrderSizeAccountAssetBarrier * account_asset,
    size_t account_asset_len,
    OpenPitOutError out_error
);

/**
 * Adds the built-in order-validation policy to the engine builder.
 *
 * Contract:
 * - `builder` must be a valid engine builder pointer.
 * - `policy_group_id` assigns the policy to a policy group (pass `0` for
 *   default).
 *
 * Success:
 * - returns `true`; the builder retains the policy.
 *
 * Error:
 * - returns `false` when the builder is null or already consumed;
 * - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
 *   error handle that MUST be released with `openpit_destroy_shared_string`.
 */
bool openpit_engine_builder_add_builtin_order_validation_policy(
    OpenPitEngineBuilder * builder,
    uint16_t policy_group_id,
    OpenPitOutError out_error
);

/**
 * Adds the built-in P&L bounds kill-switch policy to the engine builder.
 *
 * Contract:
 * - `builder` must be a valid engine builder pointer.
 * - `policy_group_id` assigns the policy to a policy group (pass `0` for
 *   default).
 * - At least one barrier must be provided: `broker_len > 0` or `account_len
 *   > 0`.
 * - When a length is greater than zero the corresponding pointer must point
 *   to that many readable entries.
 * - Each `settlement_asset` string view inside an array entry must be valid
 *   for the duration of the call.
 *
 * Success:
 * - returns `true`; the builder retains the policy.
 *
 * Error:
 * - returns `false` when the builder is null or already consumed, when no
 *   barrier is configured, or when argument parsing fails;
 * - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
 *   error handle that MUST be released with `openpit_destroy_shared_string`.
 */
bool openpit_engine_builder_add_builtin_pnl_bounds_killswitch_policy(
    OpenPitEngineBuilder * builder,
    uint16_t policy_group_id,
    const OpenPitPretradePoliciesPnlBoundsBarrier * broker,
    size_t broker_len,
    const OpenPitPretradePoliciesPnlBoundsAccountBarrier * account,
    size_t account_len,
    OpenPitOutError out_error
);

/**
 * Adds the built-in rate-limit policy to the engine builder.
 *
 * Contract:
 * - `builder` must be a valid engine builder pointer.
 * - `policy_group_id` assigns the policy to a policy group (pass `0` for
 *   default).
 * - At least one barrier axis must be configured: `broker` non-null,
 *   `asset_len > 0`, `account_len > 0`, or `account_asset_len > 0`.
 * - When a length is greater than zero the corresponding pointer must point
 *   to that many readable entries.
 * - Each `settlement_asset` string view inside an array entry must be valid
 *   for the duration of the call.
 *
 * Success:
 * - returns `true`; the builder retains the policy.
 *
 * Error:
 * - returns `false` when the builder is null or already consumed, when no
 *   barrier axis is configured, or when argument parsing fails;
 * - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
 *   error handle that MUST be released with `openpit_destroy_shared_string`.
 */
bool openpit_engine_builder_add_builtin_rate_limit_policy(
    OpenPitEngineBuilder * builder,
    uint16_t policy_group_id,
    const OpenPitPretradePoliciesRateLimitBrokerBarrier * broker,
    const OpenPitPretradePoliciesRateLimitAssetBarrier * asset,
    size_t asset_len,
    const OpenPitPretradePoliciesRateLimitAccountBarrier * account,
    size_t account_len,
    const OpenPitPretradePoliciesRateLimitAccountAssetBarrier * account_asset,
    size_t account_asset_len,
    OpenPitOutError out_error
);

/**
 * Adds the built-in spot funds policy to the engine builder.
 *
 * Contract:
 * - `builder` must be a valid engine builder pointer.
 * - `market_data` is a borrowed market-data service handle or null. Null
 *   disables market orders entirely (limit-only mode): they are rejected
 *   with `UnsupportedOrderType`. A non-null handle enables market orders;
 *   the policy reads live quotes from the supplied market-data service.
 * - `market_slippage_bps` is a pointer to a `u16` or null. When
 *   `market_data` is non-null it MUST be non-null too (otherwise this is a
 *   configuration error and the call fails). The value is the worst-case
 *   global slippage in basis points (1 bps = 0.01%). Range validation is
 *   performed by the core engine.
 * - `pricing_source` selects the base price: `0` = Mark, `1` = BookTop.
 * - `instrument_overrides` / `overrides_len` describe a contiguous array of
 *   slippage overrides; pass null + 0 for none. Each entry selects an
 *   instrument by `instrument_id` and a scope via its `account_id` /
 *   `account_group_id` optionals: both unset is an instrument-level default,
 *   a set `account_id` scopes the override to that account, a set
 *   `account_group_id` scopes it to that account group. The two are mutually
 *   exclusive; setting both fails the call. An entry with `has_slippage_bps
 *   == false` is ignored. Slippage resolves account -> account group ->
 *   instrument -> global per order.
 * - `policy_group_id` tags the policy instance.
 *
 * Mismatch guard: when `market_data` is non-null and the engine is
 * multi-threaded (`Full` or `Account` sync mode) but the market-data service
 * was built in no-sync (`None`, no-op locks) mode, this call fails with a
 * descriptive error. A no-sync engine accepts both no-sync and full-sync MD
 * services.
 *
 * Success: returns `true`; the builder retains the policy.
 *
 * Error: returns `false`. If `out_error` is non-null, writes a caller-owned
 * `OpenPitSharedString` error handle (release with
 * `openpit_destroy_shared_string`).
 */
bool openpit_engine_builder_add_builtin_spot_funds_policy(
    OpenPitEngineBuilder * builder,
    const OpenPitMarketDataService * market_data,
    const uint16_t * market_slippage_bps,
    uint8_t pricing_source,
    const OpenPitPretradePoliciesSpotFundsOverride * instrument_overrides,
    size_t overrides_len,
    uint16_t policy_group_id,
    OpenPitOutError out_error
);

/**
 * Returns the OpenPit runtime version string.
 *
 * This function never fails.
 *
 * The returned view is read-only, never null, and remains valid for the entire
 * process lifetime. The caller must not release it.
 */
OpenPitStringView openpit_get_runtime_version(void);

/**
 * Returns the build profile of the linked OpenPit runtime.
 *
 * This function never fails.
 *
 * The value is a stable, machine-parseable `key=value;`-delimited string (keys
 * `version`, `profile`, `opt_level`, `debug_assertions`, `target`,
 * `target_cpu`, `lto`). It lets a consumer reliably distinguish a debug core
 * from a release core, for example to refuse latency-sensitive work on a debug
 * build. The `target_cpu` and `lto` fields report the literal `unknown` when
 * they cannot be determined at build time.
 *
 * The returned view is read-only, never null, and remains valid for the entire
 * process lifetime. The caller must not release it.
 */
OpenPitStringView openpit_get_runtime_build_profile(void);

/**
 * Records a block against the account bound to an account-control handle.
 *
 * Records `block` against the bound account on the engine's shared
 * blocked-accounts facility. The first cause recorded for an account wins;
 * later calls for the same account are no-ops.
 *
 * Contract:
 * - `control` must be a valid non-null account-control handle, or null.
 * - `block` payload fields are copied into internal storage before this call
 *   returns.
 * - Passing a null `control` records nothing and has no effect.
 *
 * # Safety
 *
 * `control` must be either null or a valid account-control handle provided by
 * this library.
 */
void openpit_account_control_block(
    const OpenPitAccountControl * control,
    OpenPitPretradeAccountBlock block
);

/**
 * Returns a new handle referring to the same account-control facility.
 *
 * Use this to retain the ability to block the bound account from a later
 * callback within the same pre-trade transaction. The returned handle records
 * blocks against the same account as the source handle and shares its validity
 * window: it is valid to use only within that pre-trade transaction, and is
 * undefined afterwards.
 *
 * Success:
 * - returns a non-null caller-owned handle to the same facility.
 *
 * Error:
 * - returns null when `control` is null.
 *
 * Cleanup:
 * - the returned handle MUST be released with
 *   `openpit_destroy_account_control` exactly once.
 *
 * # Safety
 *
 * `control` must be either null or a valid account-control handle provided by
 * this library.
 */
OpenPitAccountControl * openpit_account_control_clone(
    const OpenPitAccountControl * control
);

/**
 * Releases a caller-owned account-control handle.
 *
 * Lifetime contract:
 * - Call this exactly once for each handle that was returned to the caller.
 * - After this call the handle is no longer valid.
 * - Passing a null pointer is allowed and has no effect.
 * - This function always succeeds.
 */
void openpit_destroy_account_control(
    OpenPitAccountControl * control
);

/**
 * Returns an account-control handle for a main-stage pre-trade context.
 *
 * A main-stage pre-trade context carries account control only when an account
 * could be bound to the request.
 *
 * Contract:
 * - `ctx` must be the callback-scoped context pointer passed to a custom
 *   main-stage pre-trade callback; it is valid only for the duration of that
 *   callback.
 *
 * Success:
 * - returns a non-null caller-owned handle when the context carries account
 *   control.
 *
 * Error:
 * - returns null when `ctx` is null or the context carries no account
 *   control (no account could be bound).
 *
 * Cleanup:
 * - the returned handle MUST be released with
 *   `openpit_destroy_account_control` exactly once. It may be retained for
 *   deferred blocking, but it is valid to use only within the pre-trade
 *   transaction of this request — through the commit or rollback of its
 *   reservation; recording a block through it afterwards is undefined.
 *
 * # Safety
 *
 * `ctx` must be either null or a valid callback-scoped pre-trade context
 * pointer provided to this library.
 */
OpenPitAccountControl * openpit_pretrade_context_get_account_control(
    const OpenPitPretradeContext * ctx
);

/**
 * Returns an account-control handle for an account-adjustment context.
 *
 * An account-adjustment context always carries account control, so this call
 * returns a non-null handle for any valid context.
 *
 * Contract:
 * - `ctx` must be the callback-scoped context pointer passed to a custom
 *   account-adjustment callback; it is valid only for the duration of that
 *   callback.
 *
 * Success:
 * - returns a non-null caller-owned handle.
 *
 * Error:
 * - returns null when `ctx` is null.
 *
 * Cleanup:
 * - the returned handle MUST be released with
 *   `openpit_destroy_account_control` exactly once. It may be retained for
 *   deferred blocking, but it is valid to use only within the account
 *   adjustment processing of this request — through the commit or rollback
 *   of that request; recording a block through it afterwards is undefined.
 *
 * # Safety
 *
 * `ctx` must be either null or a valid callback-scoped account-adjustment
 * context pointer provided to this library.
 */
OpenPitAccountControl * openpit_account_adjustment_context_get_account_control(
    const OpenPitAccountAdjustmentContext * ctx
);

/**
 * Returns the account-group for a main-stage pre-trade context.
 *
 * Looks up the group registered for the bound order account. The result is
 * cached on first call and reused for subsequent calls within the same context
 * lifetime.
 *
 * Contract:
 * - `ctx` must be the callback-scoped context pointer passed to a custom
 *   main-stage pre-trade callback; it is valid only for the duration of that
 *   callback.
 * - `out_group` must be a valid non-null pointer.
 *
 * Success:
 * - returns `true` and writes the group to `out_group` when the account is
 *   registered in a group;
 * - returns `false` when `ctx` is null, no account was bound to the request,
 *   or the account belongs to no group; `out_group` is not written to.
 *
 * # Safety
 *
 * `ctx` must be either null or a valid callback-scoped pre-trade context
 * pointer provided to this library.
 */
bool openpit_pretrade_context_get_account_group(
    const OpenPitPretradeContext * ctx,
    OpenPitParamAccountGroupId * out_group
);

/**
 * Returns the account-group for an account-adjustment context.
 *
 * Looks up the group registered for the adjusted account. The result is cached
 * on first call and reused for subsequent calls within the same context
 * lifetime.
 *
 * Contract:
 * - `ctx` must be the callback-scoped context pointer passed to a custom
 *   account-adjustment callback; it is valid only for the duration of that
 *   callback.
 * - `out_group` must be a valid non-null pointer.
 *
 * Success:
 * - returns `true` and writes the group to `out_group` when the account is
 *   registered in a group;
 * - returns `false` when `ctx` is null or the account belongs to no group;
 *   `out_group` is not written to.
 *
 * # Safety
 *
 * `ctx` must be either null or a valid callback-scoped account-adjustment
 * context pointer provided to this library.
 */
bool openpit_account_adjustment_context_get_account_group(
    const OpenPitAccountAdjustmentContext * ctx,
    OpenPitParamAccountGroupId * out_group
);

/**
 * Returns the account-group for a post-trade context.
 *
 * Looks up the group registered for the report's account. The result is cached
 * on first call and reused for subsequent calls within the same context
 * lifetime.
 *
 * Contract:
 * - `ctx` must be the callback-scoped context pointer passed to a custom
 *   `apply_execution_report` callback; it is valid only for the duration of
 *   that callback.
 * - `out_group` must be a valid non-null pointer.
 *
 * Success:
 * - returns `true` and writes the group to `out_group` when the account is
 *   registered in a group;
 * - returns `false` when `ctx` is null or the account belongs to no group;
 *   `out_group` is not written to.
 *
 * # Safety
 *
 * `ctx` must be either null or a valid callback-scoped post-trade context
 * pointer provided to this library.
 */
bool openpit_post_trade_context_get_account_group(
    const OpenPitPostTradeContext * ctx,
    OpenPitParamAccountGroupId * out_group
);

/**
 * Constructs an account-group identifier from a 32-bit integer.
 *
 * This is a direct numeric mapping with no collision risk.
 *
 * The value `0` is reserved for the default account group
 * (`OPENPIT_DEFAULT_ACCOUNT_GROUP`) and is rejected: every account already
 * belongs to that group implicitly, so no external input may name it.
 *
 * WARNING: Do not mix IDs produced by this function with IDs produced by
 * `openpit_create_param_account_group_id_from_string` in the same runtime
 * state.
 *
 * Contract:
 * - returns `true` and writes a stable account-group identifier to `out` on
 *   success;
 * - returns `false` on the reserved value (`0`) and optionally writes an
 *   error message to `out_error`.
 *
 * # Safety
 *
 * `out` must be either null or a valid writable pointer.
 */
bool openpit_create_param_account_group_id_from_uint32(
    uint32_t value,
    OpenPitParamAccountGroupId * out,
    OpenPitOutError out_error
);

/**
 * Constructs an account-group identifier from a UTF-8 byte sequence using
 * FNV-1a 32-bit hashing.
 *
 * The bytes are read only for the duration of the call. No trailing zero byte
 * is required.
 *
 * Collision note:
 * - different group strings can map to the same identifier;
 * - for `n` distinct group strings the probability of at least one collision
 *   is approximately `n^2 / (2 * 2^32)`.
 * - if collision risk is unacceptable, keep your own collision-free
 *   string-to-integer mapping and use
 *   `openpit_create_param_account_group_id_from_uint32`.
 *
 * WARNING: Do not mix IDs produced by this function with IDs produced by
 * `openpit_create_param_account_group_id_from_uint32` in the same runtime
 * state.
 *
 * Contract:
 * - returns `true` and writes a stable account-group identifier to `out` on
 *   success;
 * - returns `false` on invalid input (empty string) and optionally writes an
 *   error message to `out_error`.
 *
 * # Safety
 *
 * `value.ptr` must be non-null and point to at least `value.len` readable
 * UTF-8 bytes when `value.len > 0`.
 */
bool openpit_create_param_account_group_id_from_string(
    OpenPitStringView value,
    OpenPitParamAccountGroupId * out,
    OpenPitOutError out_error
);

/**
 * Releases a caller-owned account-adjustment outcome list.
 *
 * Contract:
 * - passing null is allowed;
 * - this function always succeeds.
 *
 * # Safety
 *
 * `outcomes` must be either null or a pointer returned by this library. The
 * list must be destroyed at most once.
 */
void openpit_destroy_account_adjustment_outcome_list(
    OpenPitAccountAdjustmentOutcomeList * outcomes
);

/**
 * Returns the number of outcomes in the list.
 *
 * Contract:
 * - `list` must be a valid non-null pointer;
 * - this function never fails;
 * - violating the pointer contract aborts the call.
 *
 * # Safety
 *
 * `list` must be a valid non-null pointer returned by this library and must
 * remain alive for the duration of this call.
 */
size_t openpit_account_adjustment_outcome_list_len(
    const OpenPitAccountAdjustmentOutcomeList * list
);

/**
 * Copies a non-owning outcome view at `index` into `out_outcome`.
 *
 * The copied view borrows string memory from `list`.
 *
 * Contract:
 * - `list` must be a valid non-null pointer;
 * - `out_outcome` must be a valid non-null pointer;
 * - returns `true` when a value exists and was copied;
 * - returns `false` when `index` is out of bounds and does not write
 *   `out_outcome`;
 * - the copied view remains valid while `list` is alive and unchanged;
 * - this function never fails;
 * - violating the pointer contract aborts the call.
 *
 * # Safety
 *
 * `list` and `out_outcome` must be valid non-null pointers returned by or
 * provided to this library and must remain alive for the duration of this
 * call.
 */
bool openpit_account_adjustment_outcome_list_get(
    const OpenPitAccountAdjustmentOutcomeList * list,
    size_t index,
    OpenPitAccountAdjustmentOutcome * out_outcome
);

/**
 * Appends one lock price to the main-stage pre-trade result.
 *
 * # Safety
 *
 * If `result` is non-null it must be a valid, properly aligned pointer to an
 * `OpenPitPretradePreTradeResult` that is exclusively accessible for the
 * duration of this call.
 *
 * Contract:
 * - `result` must be a valid non-null callback-scoped pointer;
 * - `price` is validated with the same domain rules as
 *   `openpit_create_param_price`;
 * - no `policy_group_id` is accepted: the engine assigns the policy's group.
 *
 * Success:
 * - returns `true`; the result now carries one extra lock price.
 *
 * Error:
 * - returns `false` when `result` is null or `price` fails domain
 *   validation;
 * - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
 *   error handle that MUST be released with `openpit_destroy_shared_string`.
 */
bool openpit_pretrade_pre_trade_result_push_lock_price(
    OpenPitPretradePreTradeResult * result,
    OpenPitParamPrice price,
    OpenPitOutError out_error
);

/**
 * Appends one account-adjustment outcome to the main-stage pre-trade result.
 *
 * # Safety
 *
 * If `result` is non-null it must be a valid, properly aligned pointer to an
 * `OpenPitPretradePreTradeResult` that is exclusively accessible for the
 * duration of this call.
 *
 * Contract:
 * - `result` must be a valid non-null callback-scoped pointer;
 * - `entry` is validated with `OpenPitAccountOutcomeEntry::to_entry`;
 * - no `policy_group_id` is accepted: the engine assigns the policy's group.
 *
 * Success:
 * - returns `true`; the result now carries one extra account-adjustment
 *   entry.
 *
 * Error:
 * - returns `false` when `result` is null or `entry` fails validation;
 * - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
 *   error handle that MUST be released with `openpit_destroy_shared_string`.
 */
bool openpit_pretrade_pre_trade_result_push_account_adjustment(
    OpenPitPretradePreTradeResult * result,
    OpenPitAccountOutcomeEntry entry,
    OpenPitOutError out_error
);

/**
 * Appends one group-tagged account-adjustment outcome to the post-trade list.
 *
 * # Safety
 *
 * If `list` is non-null it must be a valid, properly aligned pointer to an
 * `OpenPitPostTradeAdjustmentList` that is exclusively accessible for the
 * duration of this call.
 *
 * Contract:
 * - `list` must be a valid non-null callback-scoped pointer;
 * - `policy_group_id` tags the produced outcome;
 * - `entry` is validated with `OpenPitAccountOutcomeEntry::to_entry`.
 *
 * Success:
 * - returns `true`; the list now carries one extra outcome.
 *
 * Error:
 * - returns `false` when `list` is null or `entry` fails validation;
 * - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
 *   error handle that MUST be released with `openpit_destroy_shared_string`.
 */
bool openpit_pretrade_post_trade_adjustment_list_push(
    OpenPitPostTradeAdjustmentList * list,
    uint16_t policy_group_id,
    OpenPitAccountOutcomeEntry entry,
    OpenPitOutError out_error
);

/**
 * Appends one account-outcome entry to the account-adjustment outcome list.
 *
 * # Safety
 *
 * If `list` is non-null it must be a valid, properly aligned pointer to an
 * `OpenPitAccountOutcomeEntryList` that is exclusively accessible for the
 * duration of this call.
 *
 * Contract:
 * - `list` must be a valid non-null callback-scoped pointer;
 * - `entry` is validated with `OpenPitAccountOutcomeEntry::to_entry`;
 * - no `policy_group_id` is accepted: the engine assigns the policy's group.
 *
 * Success:
 * - returns `true`; the list now carries one extra entry.
 *
 * Error:
 * - returns `false` when `list` is null or `entry` fails validation;
 * - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
 *   error handle that MUST be released with `openpit_destroy_shared_string`.
 */
bool openpit_account_outcome_entry_list_push(
    OpenPitAccountOutcomeEntryList * list,
    OpenPitAccountOutcomeEntry entry,
    OpenPitOutError out_error
);

/**
 * Releases a main-stage pre-trade result collector. Passing null is allowed.
 *
 * # Safety
 *
 * `result` must be either null or a pointer returned by this library, and must
 * be destroyed at most once.
 */
void openpit_destroy_pretrade_pre_trade_result(
    OpenPitPretradePreTradeResult * result
);

/**
 * Releases a post-trade adjustment list collector. Passing null is allowed.
 *
 * # Safety
 *
 * `list` must be either null or a pointer returned by this library, and must
 * be destroyed at most once.
 */
void openpit_destroy_post_trade_adjustment_list(
    OpenPitPostTradeAdjustmentList * list
);

/**
 * Releases an account-outcome entry list collector. Passing null is allowed.
 *
 * # Safety
 *
 * `list` must be either null or a pointer returned by this library, and must
 * be destroyed at most once.
 */
void openpit_destroy_account_outcome_entry_list(
    OpenPitAccountOutcomeEntryList * list
);

/**
 * Releases a `OpenPitSharedBytes` handle.
 *
 * Null input is a no-op.
 */
void openpit_destroy_shared_bytes(
    OpenPitSharedBytes * handle
);

/**
 * Borrows a read-only view of the bytes stored in the handle.
 *
 * Returns an unset view (`ptr == null`, `len == 0`) when `handle` is null.
 */
OpenPitBytesView openpit_shared_bytes_view(
    const OpenPitSharedBytes * handle
);

/**
 * Returns an empty quote with every field unset.
 *
 * This function never fails.
 */
OpenPitMarketDataQuote openpit_create_marketdata_quote(void);

/**
 * Builds an infinite quote lifetime.
 *
 * This function never fails.
 */
OpenPitMarketDataQuoteTtl openpit_create_marketdata_quote_ttl_infinite(void);

/**
 * Builds a finite quote lifetime of `secs` seconds plus `nanos` nanoseconds.
 *
 * This function never fails.
 */
OpenPitMarketDataQuoteTtl openpit_create_marketdata_quote_ttl_within(
    uint64_t secs,
    uint32_t nanos
);

/**
 * Creates a market-data service with the chosen synchronization mode.
 *
 * `mode` uses the same byte convention as `openpit_create_engine_builder`:
 * - `0` = `None` (no internal synchronization: no-op locks, zero overhead,
 *   single-threaded use only);
 * - `1` = `Full` (full synchronization: real `RwLock`, safe for a concurrent
 *   quote feed).
 *
 * Only `None` (0) and `Full` (1) are valid for a market-data service. Passing
 * `2` (`Account`) or any other byte is an error.
 *
 * Success:
 * - returns a non-null caller-owned `OpenPitMarketDataService` handle.
 *
 * Error:
 * - returns null when `mode` is not `0` or `1`; if `out_error` is not null,
 *   writes a caller-owned `OpenPitSharedString` error handle that MUST be
 *   released with `openpit_destroy_shared_string`.
 *
 * Cleanup:
 * - the returned service handle MUST be released with
 *   `openpit_destroy_marketdata_service` exactly once.
 */
OpenPitMarketDataService * openpit_create_marketdata_service(
    uint8_t mode,
    OpenPitMarketDataQuoteTtl default_ttl,
    OpenPitOutError out_error
);

/**
 * Releases a market-data service handle.
 *
 * Contract:
 * - passing null is allowed;
 * - releases this handle; the underlying service stays alive while other
 *   handles to it exist;
 * - after this call the pointer is invalid;
 * - this function always succeeds.
 */
void openpit_destroy_marketdata_service(
    OpenPitMarketDataService * service
);

/**
 * Returns a new handle referring to the same market-data service.
 *
 * Use this to hand the same service to a policy and a feed.
 *
 * Success:
 * - returns a non-null caller-owned handle to the same service.
 *
 * Error:
 * - returns null when `service` is null.
 *
 * Cleanup:
 * - the returned handle MUST be released with
 *   `openpit_destroy_marketdata_service` exactly once.
 */
OpenPitMarketDataService * openpit_marketdata_service_clone(
    const OpenPitMarketDataService * service
);

/**
 * Registers `instrument` with the service-wide default TTL.
 *
 * Status:
 * - `Ok`: registered; the auto-assigned id was written to `out_id`;
 * - `AlreadyRegistered`: the instrument is already registered;
 * - `Error`: `service`/`out_id` is null or the instrument payload is
 *   invalid; if `out_error` is not null, a caller-owned
 *   `OpenPitSharedString` error handle was written that MUST be released
 *   with `openpit_destroy_shared_string`.
 */
OpenPitMarketDataRegisterStatus openpit_marketdata_service_register(
    const OpenPitMarketDataService * service,
    const OpenPitInstrument * instrument,
    OpenPitMarketDataInstrumentId * out_id,
    OpenPitOutError out_error
);

/**
 * Registers `instrument` with a per-instrument TTL override.
 *
 * Behaves like `openpit_marketdata_service_register` otherwise.
 */
OpenPitMarketDataRegisterStatus openpit_marketdata_service_register_with_ttl(
    const OpenPitMarketDataService * service,
    const OpenPitInstrument * instrument,
    OpenPitMarketDataQuoteTtl ttl,
    OpenPitMarketDataInstrumentId * out_id,
    OpenPitOutError out_error
);

/**
 * Registers `instrument` under the caller-supplied `instrument_id` with the
 * service-wide default TTL.
 *
 * Status:
 * - `Ok`: registered; `instrument_id` was written to `out_id`;
 * - `DuplicateInstrument`: the instrument name is already registered under a
 *   different id;
 * - `DuplicateId`: `instrument_id` is already registered;
 * - `Error`: `service`/`out_id` is null or the instrument payload is
 *   invalid; if `out_error` is not null, a caller-owned
 *   `OpenPitSharedString` error handle was written.
 */
OpenPitMarketDataRegisterStatus openpit_marketdata_service_register_with_id(
    const OpenPitMarketDataService * service,
    const OpenPitInstrument * instrument,
    OpenPitMarketDataInstrumentId instrument_id,
    OpenPitMarketDataInstrumentId * out_id,
    OpenPitOutError out_error
);

/**
 * Registers `instrument` under the caller-supplied `instrument_id` with a
 * per-instrument TTL override.
 *
 * Behaves like `openpit_marketdata_service_register_with_id` otherwise.
 */
OpenPitMarketDataRegisterStatus
openpit_marketdata_service_register_with_id_and_ttl(
    const OpenPitMarketDataService * service,
    const OpenPitInstrument * instrument,
    OpenPitMarketDataInstrumentId instrument_id,
    OpenPitMarketDataQuoteTtl ttl,
    OpenPitMarketDataInstrumentId * out_id,
    OpenPitOutError out_error
);

/**
 * Pins the service-level TTL for `account_id`.
 *
 * Applies to every instrument for `account_id` that does not have a more
 * specific instrument × account TTL cell.
 *
 * Contract:
 * - `service` must be a valid non-null handle; passing null aborts the call;
 * - this function never fails.
 */
void openpit_marketdata_service_set_account_ttl(
    const OpenPitMarketDataService * service,
    OpenPitParamAccountId account_id,
    OpenPitMarketDataQuoteTtl ttl
);

/**
 * Reverts the service-level TTL for `account_id` back to "inherit".
 *
 * Contract:
 * - `service` must be a valid non-null handle; passing null aborts the call;
 * - this function never fails.
 */
void openpit_marketdata_service_clear_account_ttl(
    const OpenPitMarketDataService * service,
    OpenPitParamAccountId account_id
);

/**
 * Pins the service-level TTL for `account_group_id`.
 *
 * Pass `OPENPIT_DEFAULT_ACCOUNT_GROUP` (`0`) to set the service-level
 * default-group TTL.
 *
 * Contract:
 * - `service` must be a valid non-null handle; passing null aborts the call;
 * - this function never fails.
 */
void openpit_marketdata_service_set_account_group_ttl(
    const OpenPitMarketDataService * service,
    OpenPitParamAccountGroupId account_group_id,
    OpenPitMarketDataQuoteTtl ttl
);

/**
 * Reverts the service-level TTL for `account_group_id` back to "inherit".
 *
 * Pass `OPENPIT_DEFAULT_ACCOUNT_GROUP` (`0`) to clear the default-group TTL.
 *
 * Contract:
 * - `service` must be a valid non-null handle; passing null aborts the call;
 * - this function never fails.
 */
void openpit_marketdata_service_clear_account_group_ttl(
    const OpenPitMarketDataService * service,
    OpenPitParamAccountGroupId account_group_id
);

/**
 * Updates the instrument-level TTL for an already-registered instrument.
 *
 * This replaces the removed `openpit_marketdata_service_set_ttl`.
 *
 * Status:
 * - `Ok`: updated; the new TTL takes effect on the next read;
 * - `UnknownInstrument`: `instrument_id` is not registered.
 *
 * Contract:
 * - `service` must be a valid non-null handle; passing null aborts the call.
 */
OpenPitMarketDataRegisterStatus openpit_marketdata_service_set_instrument_ttl(
    const OpenPitMarketDataService * service,
    OpenPitMarketDataInstrumentId instrument_id,
    OpenPitMarketDataQuoteTtl ttl
);

/**
 * Reverts the instrument-level TTL for `instrument_id` back to "inherit".
 *
 * Status:
 * - `Ok`: cleared;
 * - `UnknownInstrument`: `instrument_id` is not registered.
 *
 * Contract:
 * - `service` must be a valid non-null handle; passing null aborts the call.
 */
OpenPitMarketDataRegisterStatus openpit_marketdata_service_clear_instrument_ttl(
    const OpenPitMarketDataService * service,
    OpenPitMarketDataInstrumentId instrument_id
);

/**
 * Pins the instrument × account TTL cell for `(instrument_id, account_id)`.
 *
 * This is the highest-priority TTL tier (overrides all group and
 * instrument-level cells for this account).
 *
 * Status:
 * - `Ok`: pinned;
 * - `UnknownInstrument`: `instrument_id` is not registered.
 *
 * Contract:
 * - `service` must be a valid non-null handle; passing null aborts the call.
 */
OpenPitMarketDataRegisterStatus
openpit_marketdata_service_set_instrument_account_ttl(
    const OpenPitMarketDataService * service,
    OpenPitMarketDataInstrumentId instrument_id,
    OpenPitParamAccountId account_id,
    OpenPitMarketDataQuoteTtl ttl
);

/**
 * Reverts the instrument × account TTL cell for `(instrument_id, account_id)`
 * back to "inherit".
 *
 * Status:
 * - `Ok`: cleared;
 * - `UnknownInstrument`: `instrument_id` is not registered.
 *
 * Contract:
 * - `service` must be a valid non-null handle; passing null aborts the call.
 */
OpenPitMarketDataRegisterStatus
openpit_marketdata_service_clear_instrument_account_ttl(
    const OpenPitMarketDataService * service,
    OpenPitMarketDataInstrumentId instrument_id,
    OpenPitParamAccountId account_id
);

/**
 * Pins the instrument × group TTL cell for `(instrument_id,
 * account_group_id)`.
 *
 * Pass `OPENPIT_DEFAULT_ACCOUNT_GROUP` (`0`) for `account_group_id` to target
 * the instrument's default-group TTL cell.
 *
 * Status:
 * - `Ok`: pinned;
 * - `UnknownInstrument`: `instrument_id` is not registered.
 *
 * Contract:
 * - `service` must be a valid non-null handle; passing null aborts the call.
 */
OpenPitMarketDataRegisterStatus
openpit_marketdata_service_set_instrument_account_group_ttl(
    const OpenPitMarketDataService * service,
    OpenPitMarketDataInstrumentId instrument_id,
    OpenPitParamAccountGroupId account_group_id,
    OpenPitMarketDataQuoteTtl ttl
);

/**
 * Reverts the instrument × group TTL cell for `(instrument_id,
 * account_group_id)` back to "inherit".
 *
 * Pass `OPENPIT_DEFAULT_ACCOUNT_GROUP` (`0`) for `account_group_id` to clear
 * the instrument's default-group TTL cell.
 *
 * Status:
 * - `Ok`: cleared;
 * - `UnknownInstrument`: `instrument_id` is not registered.
 *
 * Contract:
 * - `service` must be a valid non-null handle; passing null aborts the call.
 */
OpenPitMarketDataRegisterStatus
openpit_marketdata_service_clear_instrument_account_group_ttl(
    const OpenPitMarketDataService * service,
    OpenPitMarketDataInstrumentId instrument_id,
    OpenPitParamAccountGroupId account_group_id
);

/**
 * Clears the stored quote for `instrument_id`.
 *
 * Contract:
 * - `service` must be a valid non-null handle; passing null aborts the call;
 * - a no-op if `instrument_id` is not registered;
 * - this function never fails.
 */
void openpit_marketdata_service_clear(
    const OpenPitMarketDataService * service,
    OpenPitMarketDataInstrumentId instrument_id
);

/**
 * Publishes a quote for `instrument_id`, replacing the entire stored snapshot.
 *
 * Status:
 * - `Ok`: the snapshot was replaced;
 * - `UnknownInstrument`: `instrument_id` is not registered;
 * - `Error`: `service` is null or `quote` carries an invalid price; if
 *   `out_error` is not null, a caller-owned `OpenPitSharedString` error
 *   handle was written.
 */
OpenPitMarketDataRegisterStatus openpit_marketdata_service_push(
    const OpenPitMarketDataService * service,
    OpenPitMarketDataInstrumentId instrument_id,
    OpenPitMarketDataQuote quote,
    OpenPitOutError out_error
);

/**
 * Publishes a partial update for `instrument_id`, merging it into the stored
 * snapshot.
 *
 * Behaves like `openpit_marketdata_service_push` otherwise.
 */
OpenPitMarketDataRegisterStatus openpit_marketdata_service_push_patch(
    const OpenPitMarketDataService * service,
    OpenPitMarketDataInstrumentId instrument_id,
    OpenPitMarketDataQuote quote,
    OpenPitOutError out_error
);

/**
 * Publishes a quote for `instrument_id` into the per-account bucket of every
 * account in `account_ids` and the per-group bucket of every group in
 * `account_group_ids`, replacing each target's snapshot.
 *
 * A null pointer with a matching length of `0` is a valid empty list.
 *
 * Status:
 * - `Ok`: all targets were written;
 * - `UnknownInstrument`: `instrument_id` is not registered;
 * - `NoTarget`: both `account_ids` and `account_group_ids` are empty; use
 *   `openpit_marketdata_service_push` to write the default bucket;
 * - `Error`: `service` is null or `quote` carries an invalid price; if
 *   `out_error` is not null, a caller-owned `OpenPitSharedString` error
 *   handle was written.
 */
OpenPitMarketDataRegisterStatus openpit_marketdata_service_push_for(
    const OpenPitMarketDataService * service,
    OpenPitMarketDataInstrumentId instrument_id,
    OpenPitMarketDataQuote quote,
    const OpenPitParamAccountId * account_ids,
    size_t account_ids_len,
    const OpenPitParamAccountGroupId * account_group_ids,
    size_t account_group_ids_len,
    OpenPitOutError out_error
);

/**
 * Publishes a partial update for `instrument_id` into the per-account bucket
 * of every account in `account_ids` and the per-group bucket of every group in
 * `account_group_ids`, merging independently into each target's existing
 * snapshot.
 *
 * Behaves like `openpit_marketdata_service_push_for` otherwise.
 */
OpenPitMarketDataRegisterStatus openpit_marketdata_service_push_for_patch(
    const OpenPitMarketDataService * service,
    OpenPitMarketDataInstrumentId instrument_id,
    OpenPitMarketDataQuote quote,
    const OpenPitParamAccountId * account_ids,
    size_t account_ids_len,
    const OpenPitParamAccountGroupId * account_group_ids,
    size_t account_group_ids_len,
    OpenPitOutError out_error
);

/**
 * Publishes a quote for `instrument`, replacing the stored snapshot.
 *
 * If `instrument` is unregistered, a named slot is created with the
 * service-default TTL.
 *
 * Success:
 * - returns `true` and writes the instrument's id to `out_id`.
 *
 * Error:
 * - returns `false` when `service`/`out_id` is null, the instrument payload
 *   is invalid, or `quote` carries an invalid price;
 * - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
 *   error handle.
 */
bool openpit_marketdata_service_push_by_instrument(
    const OpenPitMarketDataService * service,
    const OpenPitInstrument * instrument,
    OpenPitMarketDataQuote quote,
    OpenPitMarketDataInstrumentId * out_id,
    OpenPitOutError out_error
);

/**
 * Publishes a partial update for `instrument`, merging it into the stored
 * snapshot.
 *
 * Behaves like `openpit_marketdata_service_push_by_instrument` otherwise.
 */
bool openpit_marketdata_service_push_by_instrument_patch(
    const OpenPitMarketDataService * service,
    const OpenPitInstrument * instrument,
    OpenPitMarketDataQuote quote,
    OpenPitMarketDataInstrumentId * out_id,
    OpenPitOutError out_error
);

/**
 * Reads the latest quote for `(instrument_id, account_id)` under the given
 * resolution.
 *
 * `resolve_account_group` is a **required** callback that supplies the reading
 * account's group **lazily** — it is invoked only when the resolution mode
 * would consult a group or default-group bucket and the per-account bucket has
 * no quote. The callback receives the caller-supplied `user_data` context
 * pointer and, when the account belongs to a group, writes the group id to
 * `out_account_group_id` and returns `true`; when the account has no group it
 * returns `false`. Pass `OPENPIT_DEFAULT_ACCOUNT_GROUP` (`0`) to target the
 * default group bucket.
 *
 * `resolution` controls which buckets are consulted, in order, when the
 * more-specific bucket has no quote.
 *
 * Status:
 * - `Found`: a usable quote was written to `out_quote`;
 * - `Unavailable`: registered but no usable quote (never pushed, cleared, or
 *   aged past TTL);
 * - `UnknownInstrument`: `instrument_id` is not registered.
 *
 * Contract:
 * - `service`, `resolve_account_group`, and `out_quote` must be valid
 *   non-null pointers; passing null for any of them aborts the call.
 */
OpenPitMarketDataGetStatus openpit_marketdata_service_get(
    const OpenPitMarketDataService * service,
    OpenPitMarketDataInstrumentId instrument_id,
    OpenPitParamAccountId account_id,
    OpenPitMarketDataAccountGroupResolver resolve_account_group,
    void * user_data,
    OpenPitMarketDataQuoteResolution resolution,
    OpenPitMarketDataQuote * out_quote
);

/**
 * Resolves `instrument` to its registered id.
 *
 * Success:
 * - returns `true` and writes the id to `out_id` when `instrument` is
 *   registered by name;
 * - returns `false` (without writing `out_id`) when the instrument is not
 *   registered, the instrument payload is invalid, or `service`/`out_id` is
 *   null.
 *
 * This call does not use `out_error`: a `false` result simply means "not
 * resolved".
 */
bool openpit_marketdata_service_resolve(
    const OpenPitMarketDataService * service,
    const OpenPitInstrument * instrument,
    OpenPitMarketDataInstrumentId * out_id
);

/**
 * Allocates an empty lock.
 *
 * Success:
 * - always returns a non-null caller-owned handle.
 *
 * Cleanup:
 * - the caller MUST release the returned handle with
 *   `openpit_destroy_pretrade_pre_trade_lock` exactly once.
 */
OpenPitPretradePreTradeLock * openpit_create_pretrade_pre_trade_lock(void);

/**
 * Releases a lock handle.
 *
 * Contract:
 * - passing null is allowed;
 * - after this call the pointer is invalid;
 * - this function always succeeds.
 */
void openpit_destroy_pretrade_pre_trade_lock(
    OpenPitPretradePreTradeLock * handle
);

/**
 * Returns a deep copy of `lock`.
 *
 * Success:
 * - returns a non-null caller-owned handle independent of `lock`.
 *
 * Error:
 * - returns null when `lock` is null.
 *
 * Cleanup:
 * - the caller MUST release the returned handle with
 *   `openpit_destroy_pretrade_pre_trade_lock` exactly once.
 */
OpenPitPretradePreTradeLock * openpit_pretrade_pre_trade_lock_clone(
    const OpenPitPretradePreTradeLock * lock
);

/**
 * Total number of stored prices across all groups.
 *
 * `lock` must be a valid non-null handle. Passing null aborts the process.
 */
size_t openpit_pretrade_pre_trade_lock_len(
    const OpenPitPretradePreTradeLock * lock
);

/**
 * Returns `true` when the lock carries no price records.
 *
 * `lock` must be a valid non-null handle. Passing null aborts the process.
 */
bool openpit_pretrade_pre_trade_lock_is_empty(
    const OpenPitPretradePreTradeLock * lock
);

/**
 * Appends `price` under `policy_group_id`.
 *
 * Success:
 * - returns `true`; the lock now carries one extra record for
 *   `policy_group_id`.
 *
 * Error:
 * - returns `false` when `lock` is null or when `price` fails domain
 *   validation;
 * - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
 *   error handle that MUST be released with `openpit_destroy_shared_string`.
 */
bool openpit_pretrade_pre_trade_lock_push(
    OpenPitPretradePreTradeLock * lock,
    uint16_t policy_group_id,
    OpenPitParamPrice price,
    OpenPitOutError out_error
);

/**
 * Appends every `(policy_group_id, price)` record from `entries` into `lock`.
 *
 * `entries_ptr`/`entries_len` describe an array of
 * `OpenPitPretradePreTradeLockEntry`. A zero length is allowed and leaves the
 * lock unchanged regardless of `entries_ptr`.
 *
 * Success:
 * - returns `true`; every record has been appended in input order.
 *
 * Error:
 * - returns `false` when `lock` is null, when `entries_ptr` is null while
 *   `entries_len` is non-zero, or when any price fails domain validation; on
 *   the first invalid price no record is appended;
 * - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
 *   error handle that MUST be released with `openpit_destroy_shared_string`.
 */
bool openpit_pretrade_pre_trade_lock_push_many(
    OpenPitPretradePreTradeLock * lock,
    const OpenPitPretradePreTradeLockEntry * entries_ptr,
    size_t entries_len,
    OpenPitOutError out_error
);

/**
 * Builds a new lock populated from the given `(policy_group_id, price)`
 * records.
 *
 * `entries_ptr`/`entries_len` describe an array of
 * `OpenPitPretradePreTradeLockEntry`. A zero length is allowed and yields an
 * empty lock regardless of `entries_ptr`.
 *
 * Success:
 * - returns a non-null caller-owned lock handle.
 *
 * Error:
 * - returns null when `entries_ptr` is null while `entries_len` is non-zero
 *   or when any price fails domain validation;
 * - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
 *   error handle that MUST be released with `openpit_destroy_shared_string`.
 *
 * Cleanup:
 * - on success the caller MUST release the returned handle with
 *   `openpit_destroy_pretrade_pre_trade_lock` exactly once.
 */
OpenPitPretradePreTradeLock *
openpit_create_pretrade_pre_trade_lock_from_entries(
    const OpenPitPretradePreTradeLockEntry * entries_ptr,
    size_t entries_len,
    OpenPitOutError out_error
);

/**
 * Appends every record from `src` into `dst`, leaving `src` unchanged.
 *
 * Success:
 * - returns `true`; `dst` now also carries every record from `src`.
 *
 * Error:
 * - returns `false` when `dst` or `src` is null;
 * - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
 *   error handle that MUST be released with `openpit_destroy_shared_string`.
 */
bool openpit_pretrade_pre_trade_lock_merge(
    OpenPitPretradePreTradeLock * dst,
    const OpenPitPretradePreTradeLock * src,
    OpenPitOutError out_error
);

/**
 * Releases a caller-owned lock price list.
 *
 * Contract:
 * - `handle` must be a valid non-null pointer;
 * - this function always succeeds.
 */
void openpit_destroy_pretrade_pre_trade_lock_prices(
    OpenPitPretradePreTradeLockPrices * handle
);

/**
 * Borrows a read-only view of a lock price list.
 *
 * `handle` must be a valid non-null pointer; violating this triggers a panic.
 *
 * Returns an unset view (`ptr == null`, `len == 0`) when the list is empty.
 * The view remains valid only while `handle` is alive.
 */
OpenPitPretradePreTradeLockPricesView
openpit_pretrade_pre_trade_lock_prices_view(
    const OpenPitPretradePreTradeLockPrices * handle
);

/**
 * Returns the prices stored under `policy_group_id`.
 *
 * Single-price case:
 * - when the group holds exactly one price, it is written directly to
 *   `out_price`.
 *
 * Status:
 * - `Error`: `lock`, `out_price`, or `out_prices` is null; `out_error`
 *   receives an error handle when provided.
 * - `Empty`: the call succeeded and the group has no prices; `out_prices` is
 *   set to null.
 * - `One`: the call succeeded and `out_price` contains the only stored
 *   price; `out_prices` is set to null.
 * - `List`: the call succeeded and `out_prices` contains a caller-owned
 *   list.
 *
 * Cleanup:
 * - when status is `List`, the caller MUST release `*out_prices` with
 *   `openpit_destroy_pretrade_pre_trade_lock_prices` exactly once.
 */
OpenPitPretradePreTradeLockPricesStatus
openpit_pretrade_pre_trade_lock_prices_of(
    const OpenPitPretradePreTradeLock * lock,
    uint16_t policy_group_id,
    OpenPitParamPrice * out_price,
    OpenPitPretradePreTradeLockPrices ** out_prices,
    OpenPitOutError out_error
);

/**
 * Returns a caller-owned snapshot of every `(policy_group_id, price)` record
 * stored in `lock`, in iteration order (default-group records first, then each
 * non-default group in insertion order).
 *
 * `lock` must be a valid non-null handle. Passing null aborts the process.
 *
 * Cleanup:
 * - the caller MUST release the returned handle with
 *   `openpit_destroy_pretrade_pre_trade_lock_entries` exactly once.
 */
OpenPitPretradePreTradeLockEntries * openpit_pretrade_pre_trade_lock_entries(
    const OpenPitPretradePreTradeLock * lock
);

/**
 * Releases a caller-owned lock entry snapshot.
 *
 * Contract:
 * - `handle` must be a valid non-null pointer;
 * - this function always succeeds.
 */
void openpit_destroy_pretrade_pre_trade_lock_entries(
    OpenPitPretradePreTradeLockEntries * handle
);

/**
 * Borrows a read-only view of a lock entry snapshot.
 *
 * `handle` must be a valid non-null pointer; violating this triggers a panic.
 *
 * Returns an unset view (`ptr == null`, `len == 0`) when the snapshot is
 * empty. The view remains valid only while `handle` is alive.
 */
OpenPitPretradePreTradeLockEntriesView
openpit_pretrade_pre_trade_lock_entries_view(
    const OpenPitPretradePreTradeLockEntries * handle
);

/**
 * Serializes the lock as MessagePack.
 *
 * Success:
 * - returns a non-null caller-owned `OpenPitSharedBytes` carrying the
 *   MessagePack payload.
 *
 * Error:
 * - returns null when `lock` is null or when the encoder fails;
 * - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
 *   error handle that MUST be released with `openpit_destroy_shared_string`.
 *
 * Cleanup:
 * - on success the caller MUST release the returned handle with
 *   `openpit_destroy_shared_bytes` exactly once.
 */
OpenPitSharedBytes * openpit_pretrade_pre_trade_lock_to_msgpack(
    const OpenPitPretradePreTradeLock * lock,
    OpenPitOutError out_error
);

/**
 * Builds a new lock from a MessagePack payload.
 *
 * Success:
 * - returns a non-null caller-owned lock handle.
 *
 * Error:
 * - returns null when `data_ptr` is null or when the payload cannot be
 *   decoded;
 * - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
 *   error handle that MUST be released with `openpit_destroy_shared_string`.
 *
 * Cleanup:
 * - on success the caller MUST release the returned handle with
 *   `openpit_destroy_pretrade_pre_trade_lock` exactly once.
 */
OpenPitPretradePreTradeLock *
openpit_create_pretrade_pre_trade_lock_from_msgpack(
    const uint8_t * data_ptr,
    size_t data_len,
    OpenPitOutError out_error
);

/**
 * Serializes the lock as compact JSON.
 *
 * Success:
 * - returns a non-null caller-owned `OpenPitSharedString` carrying the JSON
 *   payload.
 *
 * Error:
 * - returns null when `lock` is null or when the encoder fails;
 * - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
 *   error handle that MUST be released with `openpit_destroy_shared_string`.
 *
 * Cleanup:
 * - on success the caller MUST release the returned handle with
 *   `openpit_destroy_shared_string` exactly once.
 */
OpenPitSharedString * openpit_pretrade_pre_trade_lock_to_json(
    const OpenPitPretradePreTradeLock * lock,
    OpenPitOutError out_error
);

/**
 * Builds a new lock from a JSON payload produced by
 * `openpit_pretrade_pre_trade_lock_to_json` (or any compatible serializer).
 *
 * `text_ptr`/`text_len` describe a UTF-8 byte sequence.
 *
 * Success:
 * - returns a non-null caller-owned lock handle.
 *
 * Error:
 * - returns null when `text_ptr` is null or when the payload cannot be
 *   decoded (invalid UTF-8 or invalid lock JSON);
 * - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
 *   error handle that MUST be released with `openpit_destroy_shared_string`.
 *
 * Cleanup:
 * - on success the caller MUST release the returned handle with
 *   `openpit_destroy_pretrade_pre_trade_lock` exactly once.
 */
OpenPitPretradePreTradeLock * openpit_create_pretrade_pre_trade_lock_from_json(
    const uint8_t * text_ptr,
    size_t text_len,
    OpenPitOutError out_error
);

/**
 * Serializes the lock as CBOR.
 *
 * Success:
 * - returns a non-null caller-owned `OpenPitSharedBytes` carrying the CBOR
 *   payload.
 *
 * Error:
 * - returns null when `lock` is null or when the encoder fails;
 * - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
 *   error handle that MUST be released with `openpit_destroy_shared_string`.
 *
 * Cleanup:
 * - on success the caller MUST release the returned handle with
 *   `openpit_destroy_shared_bytes` exactly once.
 */
OpenPitSharedBytes * openpit_pretrade_pre_trade_lock_to_cbor(
    const OpenPitPretradePreTradeLock * lock,
    OpenPitOutError out_error
);

/**
 * Builds a new lock from a CBOR payload.
 *
 * Success:
 * - returns a non-null caller-owned lock handle.
 *
 * Error:
 * - returns null when `data_ptr` is null or when the payload cannot be
 *   decoded;
 * - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
 *   error handle that MUST be released with `openpit_destroy_shared_string`.
 *
 * Cleanup:
 * - on success the caller MUST release the returned handle with
 *   `openpit_destroy_pretrade_pre_trade_lock` exactly once.
 */
OpenPitPretradePreTradeLock * openpit_create_pretrade_pre_trade_lock_from_cbor(
    const uint8_t * data_ptr,
    size_t data_len,
    OpenPitOutError out_error
);

/**
 * Serializes the lock using the in-process binary-stable raw layout.
 *
 * `lock` must be a valid non-null handle; violating this triggers a panic.
 *
 * Success:
 * - always returns a non-null caller-owned `OpenPitSharedBytes` carrying the
 *   raw payload.
 *
 * Cleanup:
 * - the caller MUST release the returned handle with
 *   `openpit_destroy_shared_bytes` exactly once.
 */
OpenPitSharedBytes * openpit_pretrade_pre_trade_lock_to_raw(
    const OpenPitPretradePreTradeLock * lock
);

/**
 * Builds a new lock from a raw payload produced by
 * `openpit_pretrade_pre_trade_lock_to_raw`.
 *
 * Success:
 * - returns a non-null caller-owned lock handle.
 *
 * Error:
 * - returns null when `data_ptr` is null or when the payload cannot be
 *   decoded;
 * - if `out_error` is not null, writes a caller-owned `OpenPitSharedString`
 *   error handle that MUST be released with `openpit_destroy_shared_string`.
 *
 * Cleanup:
 * - on success the caller MUST release the returned handle with
 *   `openpit_destroy_pretrade_pre_trade_lock` exactly once.
 */
OpenPitPretradePreTradeLock * openpit_create_pretrade_pre_trade_lock_from_raw(
    const uint8_t * data_ptr,
    size_t data_len,
    OpenPitOutError out_error
);

/**
 * Releases a `OpenPitSharedString` handle.
 *
 * Null input is a no-op.
 *
 * After this call, the handle and any `OpenPitStringView` previously obtained
 * from it are invalid and must not be used.
 */
void openpit_destroy_shared_string(
    OpenPitSharedString * handle
);

/**
 * Borrows a read-only view of the bytes stored in the handle.
 *
 * Returns an unset view (`ptr == null`, `len == 0`) when `handle` is null.
 *
 * The returned view is valid only while `handle` remains alive. The caller
 * must copy the bytes if they must outlive the handle.
 */
OpenPitStringView openpit_shared_string_view(
    const OpenPitSharedString * handle
);

#ifdef __cplusplus
}
#endif

#endif
