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
 * Generated file. Do not edit manually.
 * Please see https://github.com/openpitkit and the OWNERS file for details.
 */

#ifndef OPENPIT_PIT_H
#define OPENPIT_PIT_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct PitAccountAdjustment PitAccountAdjustment;
typedef struct PitAccountAdjustmentAmount PitAccountAdjustmentAmount;
typedef struct PitAccountAdjustmentAmountOptional
    PitAccountAdjustmentAmountOptional;
typedef struct PitAccountAdjustmentBalanceOperation
    PitAccountAdjustmentBalanceOperation;
typedef struct PitAccountAdjustmentBalanceOperationOptional
    PitAccountAdjustmentBalanceOperationOptional;
typedef struct PitAccountAdjustmentBatchError PitAccountAdjustmentBatchError;
typedef struct PitAccountAdjustmentBounds PitAccountAdjustmentBounds;
typedef struct PitAccountAdjustmentBoundsOptional
    PitAccountAdjustmentBoundsOptional;
typedef struct PitAccountAdjustmentContext PitAccountAdjustmentContext;
typedef struct PitAccountAdjustmentPolicy PitAccountAdjustmentPolicy;
typedef struct PitAccountAdjustmentPositionOperation
    PitAccountAdjustmentPositionOperation;
typedef struct PitAccountAdjustmentPositionOperationOptional
    PitAccountAdjustmentPositionOperationOptional;
typedef struct PitEngine PitEngine;
typedef struct PitEngineApplyExecutionReportResult
    PitEngineApplyExecutionReportResult;
typedef struct PitEngineBuilder PitEngineBuilder;
typedef struct PitExecutionReport PitExecutionReport;
typedef struct PitExecutionReportFill PitExecutionReportFill;
typedef struct PitExecutionReportFillOptional PitExecutionReportFillOptional;
typedef struct PitExecutionReportOperation PitExecutionReportOperation;
typedef struct PitExecutionReportOperationOptional
    PitExecutionReportOperationOptional;
typedef struct PitExecutionReportPositionImpact
    PitExecutionReportPositionImpact;
typedef struct PitExecutionReportPositionImpactOptional
    PitExecutionReportPositionImpactOptional;
typedef struct PitExecutionReportTrade PitExecutionReportTrade;
typedef struct PitExecutionReportTradeOptional PitExecutionReportTradeOptional;
typedef struct PitFinancialImpact PitFinancialImpact;
typedef struct PitFinancialImpactOptional PitFinancialImpactOptional;
typedef struct PitInstrument PitInstrument;
typedef struct PitMutations PitMutations;
typedef struct PitOrder PitOrder;
typedef struct PitOrderMargin PitOrderMargin;
typedef struct PitOrderMarginOptional PitOrderMarginOptional;
typedef struct PitOrderOperation PitOrderOperation;
typedef struct PitOrderOperationOptional PitOrderOperationOptional;
typedef struct PitOrderPosition PitOrderPosition;
typedef struct PitOrderPositionOptional PitOrderPositionOptional;
typedef struct PitParamAccountIdOptional PitParamAccountIdOptional;
typedef struct PitParamAdjustmentAmount PitParamAdjustmentAmount;
typedef struct PitParamCashFlow PitParamCashFlow;
typedef struct PitParamCashFlowOptional PitParamCashFlowOptional;
typedef struct PitParamDecimal PitParamDecimal;
typedef struct PitParamError PitParamError;
typedef struct PitParamFee PitParamFee;
typedef struct PitParamFeeOptional PitParamFeeOptional;
typedef struct PitParamNotional PitParamNotional;
typedef struct PitParamNotionalOptional PitParamNotionalOptional;
typedef struct PitParamPnl PitParamPnl;
typedef struct PitParamPnlOptional PitParamPnlOptional;
typedef struct PitParamPositionSize PitParamPositionSize;
typedef struct PitParamPositionSizeOptional PitParamPositionSizeOptional;
typedef struct PitParamPrice PitParamPrice;
typedef struct PitParamPriceOptional PitParamPriceOptional;
typedef struct PitParamQuantity PitParamQuantity;
typedef struct PitParamQuantityOptional PitParamQuantityOptional;
typedef struct PitParamTradeAmount PitParamTradeAmount;
typedef struct PitParamVolume PitParamVolume;
typedef struct PitParamVolumeOptional PitParamVolumeOptional;
typedef struct PitPretradeCheckPreTradeStartPolicy
    PitPretradeCheckPreTradeStartPolicy;
typedef struct PitPretradeContext PitPretradeContext;
typedef struct PitPretradePoliciesOrderSizeLimitParam
    PitPretradePoliciesOrderSizeLimitParam;
typedef struct PitPretradePoliciesPnlBoundsBarrier
    PitPretradePoliciesPnlBoundsBarrier;
typedef struct PitPretradePostTradeResult PitPretradePostTradeResult;
typedef struct PitPretradePreTradeLock PitPretradePreTradeLock;
typedef struct PitPretradePreTradePolicy PitPretradePreTradePolicy;
typedef struct PitPretradePreTradeRequest PitPretradePreTradeRequest;
typedef struct PitPretradePreTradeReservation PitPretradePreTradeReservation;
typedef struct PitReject PitReject;
typedef struct PitRejectList PitRejectList;
typedef struct PitSharedString PitSharedString;
typedef struct PitStringView PitStringView;

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
 * A value of `PIT_PARAM_LEVERAGE_NOT_SET` (`0`) means leverage is not
 * specified.
 */
typedef uint16_t PitParamLeverage;

/**
 * Stable account identifier type for FFI payloads.
 *
 * WARNING:
 * Use exactly one account-id source model per runtime:
 * - either purely numeric IDs (`pit_create_param_account_id_from_u64`),
 * - or purely string-derived IDs (`pit_create_param_account_id_from_str`).
 *
 * Do not mix both models in the same runtime state. A hashed string value can
 * coincide with a direct numeric ID, and then two distinct accounts become one
 * logical key in maps and engine state.
 */
typedef uint64_t PitParamAccountId;

/**
 * Error out-pointer used by fallible FFI calls.
 */
typedef PitSharedString ** PitOutError;

/**
 * Parameter error out-pointer used by fallible param FFI calls.
 */
typedef PitParamError ** PitOutParamError;

/**
 * Sentinel value indicating leverage is not set.
 */
#define PIT_PARAM_LEVERAGE_NOT_SET ((PitParamLeverage) 0)

/**
 * Fixed-point scale used by leverage payloads.
 */
#define PIT_PARAM_LEVERAGE_SCALE ((PitParamLeverage) 10)

/**
 * Minimum leverage in whole units.
 */
#define PIT_PARAM_LEVERAGE_MIN ((PitParamLeverage) 1)

/**
 * Maximum leverage in whole units.
 */
#define PIT_PARAM_LEVERAGE_MAX ((PitParamLeverage) 3000)

/**
 * Supported leverage increment step.
 */
#define PIT_PARAM_LEVERAGE_STEP ((float) 0.1)

/**
 * Default rounding strategy alias.
 */
#define PIT_PARAM_ROUNDING_STRATEGY_DEFAULT \
    ((PitParamRoundingStrategy) PitParamRoundingStrategy_MidpointNearestEven)

/**
 * Banker's rounding alias.
 */
#define PIT_PARAM_ROUNDING_STRATEGY_BANKER \
    ((PitParamRoundingStrategy) PitParamRoundingStrategy_MidpointNearestEven)

/**
 * Conservative profit rounding alias.
 */
#define PIT_PARAM_ROUNDING_STRATEGY_CONSERVATIVE_PROFIT \
    ((PitParamRoundingStrategy) PitParamRoundingStrategy_Down)

/**
 * Conservative loss rounding alias.
 */
#define PIT_PARAM_ROUNDING_STRATEGY_CONSERVATIVE_LOSS \
    ((PitParamRoundingStrategy) PitParamRoundingStrategy_Down)

/**
 * Order side.
 */
typedef uint8_t PitParamSide;
/**
 * Value is absent.
 */
#define PitParamSide_NotSet ((PitParamSide) 0)
/**
 * Buy side.
 */
#define PitParamSide_Buy ((PitParamSide) 1)
/**
 * Sell side.
 */
#define PitParamSide_Sell ((PitParamSide) 2)

/**
 * Position direction.
 */
typedef uint8_t PitParamPositionSide;
/**
 * Value is absent.
 */
#define PitParamPositionSide_NotSet ((PitParamPositionSide) 0)
/**
 * Long exposure.
 */
#define PitParamPositionSide_Long ((PitParamPositionSide) 1)
/**
 * Short exposure.
 */
#define PitParamPositionSide_Short ((PitParamPositionSide) 2)

/**
 * Position accounting mode.
 */
typedef uint8_t PitParamPositionMode;
/**
 * Value is absent.
 */
#define PitParamPositionMode_NotSet ((PitParamPositionMode) 0)
/**
 * Opposite trades net into one position.
 */
#define PitParamPositionMode_Netting ((PitParamPositionMode) 1)
/**
 * Long and short positions are tracked separately.
 */
#define PitParamPositionMode_Hedged ((PitParamPositionMode) 2)

/**
 * Whether a trade opens or closes exposure.
 */
typedef uint8_t PitParamPositionEffect;
/**
 * Value is absent.
 */
#define PitParamPositionEffect_NotSet ((PitParamPositionEffect) 0)
/**
 * The trade opens or increases exposure.
 */
#define PitParamPositionEffect_Open ((PitParamPositionEffect) 1)
/**
 * The trade closes or reduces exposure.
 */
#define PitParamPositionEffect_Close ((PitParamPositionEffect) 2)

/**
 * Selects how one trade-amount numeric value should be interpreted.
 */
typedef uint8_t PitParamTradeAmountKind;
/**
 * No amount field is selected.
 */
#define PitParamTradeAmountKind_NotSet ((PitParamTradeAmountKind) 0)
/**
 * The value is instrument quantity.
 */
#define PitParamTradeAmountKind_Quantity ((PitParamTradeAmountKind) 1)
/**
 * The value is settlement volume.
 */
#define PitParamTradeAmountKind_Volume ((PitParamTradeAmountKind) 2)

/**
 * Decimal rounding strategy for typed parameter constructors.
 */
typedef uint8_t PitParamRoundingStrategy;
/**
 * Round half to nearest even number.
 */
#define PitParamRoundingStrategy_MidpointNearestEven \
    ((PitParamRoundingStrategy) 0)
/**
 * Round half away from zero.
 */
#define PitParamRoundingStrategy_MidpointAwayFromZero \
    ((PitParamRoundingStrategy) 1)
/**
 * Round towards positive infinity.
 */
#define PitParamRoundingStrategy_Up ((PitParamRoundingStrategy) 2)
/**
 * Round towards negative infinity.
 */
#define PitParamRoundingStrategy_Down ((PitParamRoundingStrategy) 3)

/**
 * Tri-state boolean value.
 */
typedef uint8_t PitTriBool;
/**
 * Value is absent.
 */
#define PitTriBool_NotSet ((PitTriBool) 0)
/**
 * Boolean false.
 */
#define PitTriBool_False ((PitTriBool) 1)
/**
 * Boolean true.
 */
#define PitTriBool_True ((PitTriBool) 2)

/**
 * Selects how an account-adjustment amount should be interpreted.
 */
typedef uint8_t PitParamAdjustmentAmountKind;
/**
 * No amount is specified.
 */
#define PitParamAdjustmentAmountKind_NotSet ((PitParamAdjustmentAmountKind) 0)
/**
 * Change current state by the supplied signed amount.
 */
#define PitParamAdjustmentAmountKind_Delta ((PitParamAdjustmentAmountKind) 1)
/**
 * Set current state to the supplied signed amount.
 */
#define PitParamAdjustmentAmountKind_Absolute ((PitParamAdjustmentAmountKind) 2)

/**
 * Result of `pit_engine_apply_account_adjustment`.
 */
typedef uint8_t PitAccountAdjustmentApplyStatus;
/**
 * The call failed before the batch could be evaluated.
 */
#define PitAccountAdjustmentApplyStatus_Error \
    ((PitAccountAdjustmentApplyStatus) 0)
/**
 * The batch was accepted and applied.
 */
#define PitAccountAdjustmentApplyStatus_Applied \
    ((PitAccountAdjustmentApplyStatus) 1)
/**
 * The batch was evaluated and rejected by policy or validation logic.
 */
#define PitAccountAdjustmentApplyStatus_Rejected \
    ((PitAccountAdjustmentApplyStatus) 2)

/**
 * Broad area to which a reject applies.
 *
 * Valid values: `Order` (1), `Account` (2). Zero is not a valid scope value;
 * the caller must always set this field explicitly.
 */
typedef uint8_t PitRejectScope;
/**
 * The reject applies to one order or order-like request.
 */
#define PitRejectScope_Order ((PitRejectScope) 1)
/**
 * The reject applies to account state rather than to one order only.
 */
#define PitRejectScope_Account ((PitRejectScope) 2)

/**
 * Stable classification code for a reject.
 *
 * Read this first when you need machine-readable handling. The textual fields
 * in [`PitReject`] provide operator-facing explanation and extra context.
 *
 * Valid codes are `1..=39` and `255` (`Other`). Unknown incoming codes are
 * mapped to `Other` (`255`).
 */
typedef uint16_t PitRejectCode;
/**
 * A required field is absent.
 */
#define PitRejectCode_MissingRequiredField ((PitRejectCode) 1)
/**
 * A field cannot be parsed from the supplied wire value.
 */
#define PitRejectCode_InvalidFieldFormat ((PitRejectCode) 2)
/**
 * A field is syntactically valid but semantically unacceptable.
 */
#define PitRejectCode_InvalidFieldValue ((PitRejectCode) 3)
/**
 * The requested order type is not supported.
 */
#define PitRejectCode_UnsupportedOrderType ((PitRejectCode) 4)
/**
 * The requested time-in-force is not supported.
 */
#define PitRejectCode_UnsupportedTimeInForce ((PitRejectCode) 5)
/**
 * Another order attribute is unsupported.
 */
#define PitRejectCode_UnsupportedOrderAttribute ((PitRejectCode) 6)
/**
 * The client order identifier duplicates an active order.
 */
#define PitRejectCode_DuplicateClientOrderId ((PitRejectCode) 7)
/**
 * The order arrived after the allowed entry deadline.
 */
#define PitRejectCode_TooLateToEnter ((PitRejectCode) 8)
/**
 * Trading is closed for the relevant venue or session.
 */
#define PitRejectCode_ExchangeClosed ((PitRejectCode) 9)
/**
 * The instrument cannot be resolved.
 */
#define PitRejectCode_UnknownInstrument ((PitRejectCode) 10)
/**
 * The account cannot be resolved.
 */
#define PitRejectCode_UnknownAccount ((PitRejectCode) 11)
/**
 * The venue cannot be resolved.
 */
#define PitRejectCode_UnknownVenue ((PitRejectCode) 12)
/**
 * The clearing account cannot be resolved.
 */
#define PitRejectCode_UnknownClearingAccount ((PitRejectCode) 13)
/**
 * The collateral asset cannot be resolved.
 */
#define PitRejectCode_UnknownCollateralAsset ((PitRejectCode) 14)
/**
 * Available balance is insufficient.
 */
#define PitRejectCode_InsufficientFunds ((PitRejectCode) 15)
/**
 * Available margin is insufficient.
 */
#define PitRejectCode_InsufficientMargin ((PitRejectCode) 16)
/**
 * Available position is insufficient.
 */
#define PitRejectCode_InsufficientPosition ((PitRejectCode) 17)
/**
 * A credit limit was exceeded.
 */
#define PitRejectCode_CreditLimitExceeded ((PitRejectCode) 18)
/**
 * A risk limit was exceeded.
 */
#define PitRejectCode_RiskLimitExceeded ((PitRejectCode) 19)
/**
 * The order exceeds a generic configured limit.
 */
#define PitRejectCode_OrderExceedsLimit ((PitRejectCode) 20)
/**
 * The order quantity exceeds a configured limit.
 */
#define PitRejectCode_OrderQtyExceedsLimit ((PitRejectCode) 21)
/**
 * The order notional exceeds a configured limit.
 */
#define PitRejectCode_OrderNotionalExceedsLimit ((PitRejectCode) 22)
/**
 * The resulting position exceeds a configured limit.
 */
#define PitRejectCode_PositionLimitExceeded ((PitRejectCode) 23)
/**
 * Concentration constraints were violated.
 */
#define PitRejectCode_ConcentrationLimitExceeded ((PitRejectCode) 24)
/**
 * Leverage constraints were violated.
 */
#define PitRejectCode_LeverageLimitExceeded ((PitRejectCode) 25)
/**
 * The request rate exceeded a configured limit.
 */
#define PitRejectCode_RateLimitExceeded ((PitRejectCode) 26)
/**
 * A loss barrier has blocked further risk-taking.
 */
#define PitRejectCode_PnlKillSwitchTriggered ((PitRejectCode) 27)
/**
 * The account is blocked.
 */
#define PitRejectCode_AccountBlocked ((PitRejectCode) 28)
/**
 * The account is not authorized for this action.
 */
#define PitRejectCode_AccountNotAuthorized ((PitRejectCode) 29)
/**
 * A compliance restriction blocked the action.
 */
#define PitRejectCode_ComplianceRestriction ((PitRejectCode) 30)
/**
 * The instrument is restricted.
 */
#define PitRejectCode_InstrumentRestricted ((PitRejectCode) 31)
/**
 * A jurisdiction restriction blocked the action.
 */
#define PitRejectCode_JurisdictionRestriction ((PitRejectCode) 32)
/**
 * The action would violate wash-trade prevention.
 */
#define PitRejectCode_WashTradePrevention ((PitRejectCode) 33)
/**
 * The action would violate self-match prevention.
 */
#define PitRejectCode_SelfMatchPrevention ((PitRejectCode) 34)
/**
 * Short-sale restriction blocked the action.
 */
#define PitRejectCode_ShortSaleRestriction ((PitRejectCode) 35)
/**
 * Required risk configuration is missing.
 */
#define PitRejectCode_RiskConfigurationMissing ((PitRejectCode) 36)
/**
 * Required reference data is unavailable.
 */
#define PitRejectCode_ReferenceDataUnavailable ((PitRejectCode) 37)
/**
 * The system could not compute an order value needed for validation.
 */
#define PitRejectCode_OrderValueCalculationFailed ((PitRejectCode) 38)
/**
 * A required service or subsystem is unavailable.
 */
#define PitRejectCode_SystemUnavailable ((PitRejectCode) 39)
/**
 * Reserved discriminant for caller-defined reject classes.
 *
 * Use together with `Reject::with_user_data` to attach a caller-defined
 * payload that the receiving code can decode. The SDK does not interpret
 * this code beyond mapping it to FFI value 254.
 */
#define PitRejectCode_Custom ((PitRejectCode) 254)
/**
 * A catch-all code for rejects that do not fit a more specific class.
 */
#define PitRejectCode_Other ((PitRejectCode) 255)

/**
 * Parameter error code transported through FFI.
 */
typedef uint32_t PitParamErrorCode;
/**
 * Error code is not specified.
 */
#define PitParamErrorCode_Unspecified ((PitParamErrorCode) 0)
/**
 * Value must be non-negative.
 */
#define PitParamErrorCode_Negative ((PitParamErrorCode) 1)
/**
 * Division by zero.
 */
#define PitParamErrorCode_DivisionByZero ((PitParamErrorCode) 2)
/**
 * Arithmetic overflow.
 */
#define PitParamErrorCode_Overflow ((PitParamErrorCode) 3)
/**
 * Arithmetic underflow.
 */
#define PitParamErrorCode_Underflow ((PitParamErrorCode) 4)
/**
 * Invalid float value.
 */
#define PitParamErrorCode_InvalidFloat ((PitParamErrorCode) 5)
/**
 * Invalid textual format.
 */
#define PitParamErrorCode_InvalidFormat ((PitParamErrorCode) 6)
/**
 * Invalid price value.
 */
#define PitParamErrorCode_InvalidPrice ((PitParamErrorCode) 7)
/**
 * Invalid leverage value.
 */
#define PitParamErrorCode_InvalidLeverage ((PitParamErrorCode) 8)
/**
 * Asset identifier is empty.
 */
#define PitParamErrorCode_AssetEmpty ((PitParamErrorCode) 9)
/**
 * Account identifier string is empty.
 */
#define PitParamErrorCode_AccountIdEmpty ((PitParamErrorCode) 10)
/**
 * Catch-all code for unknown cases.
 */
#define PitParamErrorCode_Other ((PitParamErrorCode) 4294967295)

/**
 * Result status for pre-trade operations.
 */
typedef uint8_t PitPretradeStatus;
/**
 * Order/request passed this stage; read the success out-pointer.
 */
#define PitPretradeStatus_Passed ((PitPretradeStatus) 0)
/**
 * Order/request was rejected; read the reject out-pointer.
 */
#define PitPretradeStatus_Rejected ((PitPretradeStatus) 1)
/**
 * Call failed due to invalid input; read the error out-pointer.
 */
#define PitPretradeStatus_Error ((PitPretradeStatus) 2)

/**
 * Decimal value represented as `mantissa * 10^-scale`.
 */
struct PitParamDecimal {
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
struct PitParamPnl {
    PitParamDecimal _0;
};

/**
 * Validated `Price` value wrapper.
 */
struct PitParamPrice {
    PitParamDecimal _0;
};

/**
 * Validated `Quantity` value wrapper.
 */
struct PitParamQuantity {
    PitParamDecimal _0;
};

/**
 * Validated `Volume` value wrapper.
 */
struct PitParamVolume {
    PitParamDecimal _0;
};

/**
 * Validated `CashFlow` value wrapper.
 */
struct PitParamCashFlow {
    PitParamDecimal _0;
};

/**
 * Validated `PositionSize` value wrapper.
 */
struct PitParamPositionSize {
    PitParamDecimal _0;
};

/**
 * Validated `Fee` value wrapper.
 */
struct PitParamFee {
    PitParamDecimal _0;
};

/**
 * Validated `Notional` value wrapper.
 */
struct PitParamNotional {
    PitParamDecimal _0;
};

struct PitParamNotionalOptional {
    PitParamNotional value;
    bool is_set;
};

struct PitParamPnlOptional {
    PitParamPnl value;
    bool is_set;
};

struct PitParamPriceOptional {
    PitParamPrice value;
    bool is_set;
};

struct PitParamQuantityOptional {
    PitParamQuantity value;
    bool is_set;
};

struct PitParamVolumeOptional {
    PitParamVolume value;
    bool is_set;
};

struct PitParamCashFlowOptional {
    PitParamCashFlow value;
    bool is_set;
};

struct PitParamPositionSizeOptional {
    PitParamPositionSize value;
    bool is_set;
};

struct PitParamFeeOptional {
    PitParamFee value;
    bool is_set;
};

struct PitParamAccountIdOptional {
    PitParamAccountId value;
    bool is_set;
};

/**
 * Optional position-management group for an order.
 *
 * The group is absent when every field is `NotSet`.
 */
struct PitOrderPosition {
    /**
     * Optional long/short side.
     */
    PitParamPositionSide position_side;
    /**
     * Reduce-only flag.
     */
    PitTriBool reduce_only;
    /**
     * Close-position flag.
     */
    PitTriBool close_position;
};

struct PitOrderPositionOptional {
    PitOrderPosition value;
    bool is_set;
};

/**
 * Populated financial-impact group for an execution report.
 */
struct PitFinancialImpact {
    /**
     * Profit-and-loss contribution carried by this report.
     */
    PitParamPnlOptional pnl;
    /**
     * Fee or rebate contribution carried by this report.
     */
    PitParamFeeOptional fee;
};

/**
 * Fill trade payload (`price + quantity`) for execution reports.
 */
struct PitExecutionReportTrade {
    /**
     * Trade price.
     */
    PitParamPrice price;
    /**
     * Trade quantity.
     */
    PitParamQuantity quantity;
};

/**
 * Populated position-impact group for an execution report.
 */
struct PitExecutionReportPositionImpact {
    /**
     * Whether exposure is opened or closed.
     */
    PitParamPositionEffect position_effect;
    /**
     * Impacted side (long or short).
     */
    PitParamPositionSide position_side;
};

struct PitFinancialImpactOptional {
    PitFinancialImpact value;
    bool is_set;
};

struct PitExecutionReportTradeOptional {
    PitExecutionReportTrade value;
    bool is_set;
};

struct PitExecutionReportPositionImpactOptional {
    PitExecutionReportPositionImpact value;
    bool is_set;
};

/**
 * Aggregated post-trade processing result.
 */
struct PitPretradePostTradeResult {
    /**
     * Whether the report triggered some kill-switch policy.
     */
    bool kill_switch_triggered;
};

/**
 * One amount component inside an account adjustment.
 *
 * The numeric value is interpreted according to `kind`:
 * - `Delta` means "change current state by this signed amount";
 * - `Absolute` means "set current state to this signed amount".
 */
struct PitParamAdjustmentAmount {
    /**
     * Signed numeric value of the adjustment.
     */
    PitParamPositionSize value;
    /**
     * Interpretation mode for `value`.
     */
    PitParamAdjustmentAmountKind kind;
};

/**
 * Optional amount-change group for account adjustment.
 *
 * The group is absent when every field is absent.
 */
struct PitAccountAdjustmentAmount {
    /**
     * Requested total-balance change.
     */
    PitParamAdjustmentAmount total;
    /**
     * Requested reserved-balance change.
     */
    PitParamAdjustmentAmount reserved;
    /**
     * Requested pending-balance change.
     */
    PitParamAdjustmentAmount pending;
};

/**
 * Optional bounds group for account adjustment.
 *
 * The group is absent when every bound is absent.
 */
struct PitAccountAdjustmentBounds {
    /**
     * Optional upper bound for total balance.
     */
    PitParamPositionSizeOptional total_upper;
    /**
     * Optional lower bound for total balance.
     */
    PitParamPositionSizeOptional total_lower;
    /**
     * Optional upper bound for reserved balance.
     */
    PitParamPositionSizeOptional reserved_upper;
    /**
     * Optional lower bound for reserved balance.
     */
    PitParamPositionSizeOptional reserved_lower;
    /**
     * Optional upper bound for pending balance.
     */
    PitParamPositionSizeOptional pending_upper;
    /**
     * Optional lower bound for pending balance.
     */
    PitParamPositionSizeOptional pending_lower;
};

struct PitAccountAdjustmentAmountOptional {
    PitAccountAdjustmentAmount value;
    bool is_set;
};

struct PitAccountAdjustmentBoundsOptional {
    PitAccountAdjustmentBounds value;
    bool is_set;
};

/**
 * Caller-owned parameter error container.
 */
struct PitParamError {
    /**
     * Stable machine-readable error code.
     */
    PitParamErrorCode code;
    /**
     * Human-readable message allocated as shared string.
     */
    PitSharedString * message;
};

/**
 * Price-lock snapshot returned from a reservation.
 */
struct PitPretradePreTradeLock {
    /**
     * Optional reserved price.
     */
    PitParamPriceOptional price;
};

/**
 * Result of `pit_engine_apply_execution_report`.
 */
struct PitEngineApplyExecutionReportResult {
    /**
     * The result of the post-trade processing if no error occurred.
     */
    PitPretradePostTradeResult post_trade_result;
    /**
     * Whether the call failed at the transport level.
     */
    bool is_error;
};

/**
 * Non-owning UTF-8 string view.
 *
 * This type never owns memory. It borrows bytes from another object.
 *
 * Lifetime contract:
 * - `ptr` points to `len` readable bytes;
 * - the memory is valid while the original object is alive and the source
 * string has not been modified;
 * - the caller must not free or mutate memory behind `ptr`.
 * - if the caller needs to retain the string beyond that announced lifetime,
 * the caller must copy the bytes.
 */
struct PitStringView {
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
struct PitParamTradeAmount {
    /**
     * Non-negative numeric payload.
     */
    PitParamDecimal value;
    /**
     * Interpretation mode for `value`.
     */
    PitParamTradeAmountKind kind;
};

/**
 * Optional margin group for an order.
 *
 * The group is absent when every field is `NotSet`.
 */
struct PitOrderMargin {
    /**
     * Optional collateral asset.
     */
    PitStringView collateral_asset;
    /**
     * Auto-borrow flag.
     */
    PitTriBool auto_borrow;
    /**
     * Optional leverage value.
     */
    PitParamLeverage leverage;
};

struct PitOrderMarginOptional {
    PitOrderMargin value;
    bool is_set;
};

/**
 * Populated fill-details group for an execution report.
 */
struct PitExecutionReportFill {
    /**
     * Optional latest trade payload.
     */
    PitExecutionReportTradeOptional last_trade;
    /**
     * Remaining quantity after applying this report.
     */
    PitParamQuantityOptional leaves_quantity;
    /**
     * Optional lock price associated with the report.
     */
    PitParamPriceOptional lock_price;
    /**
     * Whether this report ends the report stream for the order.
     */
    bool is_terminal;
};

struct PitExecutionReportFillOptional {
    PitExecutionReportFill value;
    bool is_set;
};

/**
 * Balance-operation payload for account adjustment.
 */
struct PitAccountAdjustmentBalanceOperation {
    /**
     * Balance asset code.
     */
    PitStringView asset;
    /**
     * Optional average entry price.
     */
    PitParamPriceOptional average_entry_price;
};

struct PitAccountAdjustmentBalanceOperationOptional {
    PitAccountAdjustmentBalanceOperation value;
    bool is_set;
};

/**
 * Single rejection record returned by checks.
 */
struct PitReject {
    /**
     * Policy name that produced the reject.
     */
    PitStringView policy;
    /**
     * Human-readable reject reason.
     */
    PitStringView reason;
    /**
     * Case-specific reject details.
     */
    PitStringView details;
    /**
     * Opaque caller-defined token.
     *
     * The SDK never inspects, dereferences, or frees this value. Its meaning,
     * lifetime, and thread-safety are the caller's responsibility. `0` / null
     * means "not set". See the project Threading Contract for the full lifetime
     * model.
     *
     * The token flows through every reject path the SDK exposes (start-stage,
     * main-stage, account-adjustment, batch results) and is preserved on
     * `Clone`.
     */
    void * user_data;
    /**
     * Stable machine-readable reject code.
     */
    PitRejectCode code;
    /**
     * Reject scope.
     */
    PitRejectScope scope;
};

/**
 * One barrier definition for
 * `pit_create_pretrade_policies_pnl_bounds_killswitch_policy`.
 *
 * What it describes:
 * - A settlement asset and its lower/upper P&L bounds.
 *
 * Contract:
 * - `settlement_asset` must point to a valid, null-terminated string for the
 * duration of the call.
 * - `initial_pnl` must contain a valid PnL value.
 * - The array passed to the create function may contain multiple entries.
 */
struct PitPretradePoliciesPnlBoundsBarrier {
    /**
     * Settlement asset whose accumulated P&L is being monitored.
     */
    PitStringView settlement_asset;
    /**
     * Optional lower bound for accumulated P&L.
     */
    PitParamPnlOptional lower_bound;
    /**
     * Optional upper bound for accumulated P&L.
     */
    PitParamPnlOptional upper_bound;
    /**
     * Initial accumulated P&L value.
     */
    PitParamPnl initial_pnl;
};

/**
 * One limit definition for
 * `pit_create_pretrade_policies_order_size_limit_policy`.
 *
 * What it describes:
 * - Per-settlement maximum quantity and maximum notional allowed for one
 *   order.
 *
 * Contract:
 * - `settlement_asset` must point to a valid, null-terminated string for the
 * duration of the call.
 * - `max_quantity` and `max_notional` must contain valid limit values.
 */
struct PitPretradePoliciesOrderSizeLimitParam {
    /**
     * Settlement asset to which the limits apply.
     */
    PitStringView settlement_asset;
    /**
     * Maximum allowed quantity for one order.
     */
    PitParamQuantity max_quantity;
    /**
     * Maximum allowed notional for one order.
     */
    PitParamVolume max_notional;
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
struct PitInstrument {
    /**
     * Traded asset, for example `AAPL` or `BTC`.
     */
    PitStringView underlying_asset;
    /**
     * Settlement asset, for example `USD`.
     */
    PitStringView settlement_asset;
};

/**
 * Optional operation group for an order.
 *
 * The group is absent when all fields are absent.
 */
struct PitOrderOperation {
    /**
     * Optional trade amount payload.
     */
    PitParamTradeAmount trade_amount;
    /**
     * Trading instrument.
     */
    PitInstrument instrument;
    /**
     * Optional limit price.
     */
    PitParamPriceOptional price;
    /**
     * Optional account identifier.
     */
    PitParamAccountIdOptional account_id;
    /**
     * Optional buy/sell direction.
     */
    PitParamSide side;
};

struct PitOrderOperationOptional {
    PitOrderOperation value;
    bool is_set;
};

/**
 * Populated operation-identification group for an execution report.
 */
struct PitExecutionReportOperation {
    /**
     * Trading instrument (`underlying + settlement` asset pair).
     */
    PitInstrument instrument;
    /**
     * Account identifier associated with the report.
     */
    PitParamAccountIdOptional account_id;
    /**
     * Buy or sell direction of the affected order or trade.
     */
    PitParamSide side;
};

struct PitExecutionReportOperationOptional {
    PitExecutionReportOperation value;
    bool is_set;
};

/**
 * Full caller-owned execution-report payload.
 */
struct PitExecutionReport {
    /**
     * Optional operation-identification group.
     */
    PitExecutionReportOperationOptional operation;
    /**
     * Optional financial-impact group.
     */
    PitFinancialImpactOptional financial_impact;
    /**
     * Optional fill-details group.
     */
    PitExecutionReportFillOptional fill;
    /**
     * Optional position-impact group.
     */
    PitExecutionReportPositionImpactOptional position_impact;
    /**
     * Opaque caller-defined token.
     *
     * The SDK never inspects, dereferences, or frees this value. Its meaning,
     * lifetime, and thread-safety are the caller's responsibility. `0` / null
     * means "not set". See the project Threading Contract for the full lifetime
     * model.
     *
     * The token is preserved unchanged across every engine callback that
     * receives the carrying value, including policy callbacks and adjustment
     * callbacks.
     */
    void * user_data;
};

/**
 * Position-operation payload for account adjustment.
 */
struct PitAccountAdjustmentPositionOperation {
    /**
     * Position instrument.
     */
    PitInstrument instrument;
    /**
     * Position collateral asset.
     */
    PitStringView collateral_asset;
    /**
     * Position average entry price.
     */
    PitParamPriceOptional average_entry_price;
    /**
     * Optional leverage.
     */
    PitParamLeverage leverage;
    /**
     * Position mode.
     */
    PitParamPositionMode mode;
};

struct PitAccountAdjustmentPositionOperationOptional {
    PitAccountAdjustmentPositionOperation value;
    bool is_set;
};

/**
 * Full caller-owned account-adjustment payload.
 */
struct PitAccountAdjustment {
    /**
     * Optional balance-operation group.
     */
    PitAccountAdjustmentBalanceOperationOptional balance_operation;
    /**
     * Optional position-operation group.
     */
    PitAccountAdjustmentPositionOperationOptional position_operation;
    /**
     * Optional amount-change group.
     */
    PitAccountAdjustmentAmountOptional amount;
    /**
     * Optional bounds group.
     */
    PitAccountAdjustmentBoundsOptional bounds;
    /**
     * Opaque caller-defined token.
     *
     * The SDK never inspects, dereferences, or frees this value. Its meaning,
     * lifetime, and thread-safety are the caller's responsibility. `0` / null
     * means "not set". See the project Threading Contract for the full lifetime
     * model.
     *
     * The token is preserved unchanged across every engine callback that
     * receives the carrying value, including policy callbacks and adjustment
     * callbacks.
     */
    void * user_data;
};

/**
 * Full caller-owned order payload.
 */
struct PitOrder {
    /**
     * Optional main operation group.
     */
    PitOrderOperationOptional operation;
    /**
     * Optional margin group.
     */
    PitOrderMarginOptional margin;
    /**
     * Optional position-management group.
     */
    PitOrderPositionOptional position;
    /**
     * Opaque caller-defined token.
     *
     * The SDK never inspects, dereferences, or frees this value. Its meaning,
     * lifetime, and thread-safety are the caller's responsibility. `0` / null
     * means "not set". See the project Threading Contract for the full lifetime
     * model.
     *
     * The token is preserved unchanged across every engine callback that
     * receives the carrying value, including policy callbacks and adjustment
     * callbacks.
     */
    void * user_data;
};

/**
 * Callback invoked for either commit or rollback of a registered mutation.
 */
typedef void (*PitMutationFn)(
    void * user_data
);

/**
 * Optional callback to release mutation user_data after execution.
 *
 * Called exactly once per `pit_mutations_push`:
 * - after `commit_fn` when commit runs;
 * - after `rollback_fn` when rollback runs;
 * - or on drop if neither action ran.
 */
typedef void (*PitMutationFreeFn)(
    void * user_data
);

/**
 * Callback used by a custom start-stage policy to validate one order.
 *
 * Contract:
 * - `ctx` is a read-only context valid only for the duration of the
 *   callback.
 * - `order` points to a read-only order view valid only for the duration of
 * the callback.
 * - `order` is passed as a borrowed view and is not copied before the
 * callback runs.
 * - If the callback wants to keep any data from `order`, it must copy that
 * data before returning.
 * - Return null or an empty list to accept the order.
 * - Return a non-empty reject list to reject the order.
 * - A rejected order must set explicit `code` and `scope` values in every
 * list item.
 * - The returned list ownership is transferred to the engine; create it with
 * `pit_create_reject_list`.
 * - Every reject payload is copied into internal storage before the callback
 * returns.
 * - `user_data` is passed through unchanged from policy creation.
 */
typedef PitRejectList *
(*PitPretradeCheckPreTradeStartPolicyCheckPreTradeStartFn)(
    const PitPretradeContext * ctx,
    const PitOrder * order,
    void * user_data
);

/**
 * Callback used by a custom start-stage policy to observe an execution report.
 *
 * Contract:
 * - `report` points to a read-only report view valid only for the duration
 *   of
 * the callback.
 * - `report` is passed as a borrowed view and is not copied before the
 * callback runs.
 * - If the callback wants to keep any data from `report`, it must copy that
 * data before returning.
 * - Return `true` if the policy state changed and the engine should keep the
 * update.
 * - Return `false` when nothing changed.
 * - `user_data` is passed through unchanged from policy creation.
 */
typedef bool (*PitPretradeCheckPreTradeStartPolicyApplyExecutionReportFn)(
    const PitExecutionReport * report,
    void * user_data
);

/**
 * Callback invoked when the last reference to a custom start-stage policy is
 * released and the policy object is about to be destroyed.
 *
 * Contract:
 * - Called exactly once, on the thread that drops the last policy reference.
 * - After this callback returns, no further callbacks will be invoked for
 * this policy instance.
 * - `user_data` is the same value that was passed at policy creation.
 * - The callback must release any resources associated with `user_data`.
 */
typedef void (*PitPretradeCheckPreTradeStartPolicyFreeUserDataFn)(
    void * user_data
);

/**
 * Callback used by a custom main-stage policy to perform a pre-trade check.
 *
 * Contract:
 * - `ctx` is a read-only context valid only for the duration of the
 *   callback.
 * - `order` points to a read-only order view valid only for the duration of
 * the callback.
 * - `order` is passed as a borrowed view and is not copied before the
 * callback runs.
 * - If the callback wants to keep any data from `order`, it must copy that
 * data before returning.
 * - `mutations` is a callback-scoped non-owning pointer that allows the
 * callback to register commit/rollback mutations.
 * - The callback must not store or use `mutations` after return.
 * - Return null or an empty list to accept the order.
 * - Return a non-empty reject list to reject the order.
 * - Every returned reject must contain explicit `code` and `scope` values.
 * - The returned list ownership is transferred to the engine; create it with
 * `pit_create_reject_list`.
 * - Every reject payload is copied into internal storage before this
 *   callback
 * returns.
 * - `user_data` is passed through unchanged from policy creation.
 */
typedef PitRejectList * (*PitPretradePreTradePolicyCheckFn)(
    const PitPretradeContext * ctx,
    const PitOrder * order,
    PitMutations * mutations,
    void * user_data
);

/**
 * Callback used by a custom main-stage policy to observe an execution report.
 *
 * Contract:
 * - `report` points to a read-only report view valid only for the duration
 *   of
 * the callback.
 * - `report` is passed as a borrowed view and is not copied before the
 * callback runs.
 * - If the callback wants to keep any data from `report`, it must copy that
 * data before returning.
 * - Return `true` if the policy state changed and the engine should keep the
 * update.
 * - Return `false` when nothing changed.
 * - `user_data` is passed through unchanged from policy creation.
 */
typedef bool (*PitPretradePreTradePolicyApplyExecutionReportFn)(
    const PitExecutionReport * report,
    void * user_data
);

/**
 * Callback invoked when the last reference to a custom main-stage policy is
 * released and the policy object is about to be destroyed.
 *
 * Contract:
 * - Called exactly once, on the thread that drops the last policy reference.
 * - After this callback returns, no further callbacks will be invoked for
 * this policy instance.
 * - `user_data` is the same value that was passed at policy creation.
 * - The callback must release any resources associated with `user_data`.
 */
typedef void (*PitPretradePreTradePolicyFreeUserDataFn)(
    void * user_data
);

/**
 * Callback used by a custom account-adjustment policy to validate one
 * adjustment.
 *
 * Contract:
 * - `ctx` is a read-only context valid only for the duration of the
 *   callback.
 * - `adjustment` points to a read-only adjustment view valid only for the
 * duration of the callback.
 * - `adjustment` is passed as a borrowed view and is not copied before the
 * callback runs.
 * - If the callback wants to keep any data from `adjustment`, it must copy
 * that data before returning.
 * - `account_id` must follow the same source model as the rest of the
 * runtime state (numeric-only or string-derived-only).
 * - `mutations` is a callback-scoped non-owning pointer that allows the
 * callback to register commit/rollback mutations.
 * - The callback must not store or use `mutations` after return.
 * - Return null to accept the adjustment.
 * - Return a non-empty reject list to reject the adjustment.
 * - Returned reject list ownership is transferred to the callee.
 * - `user_data` is passed through unchanged from policy creation.
 */
typedef PitRejectList * (*PitAccountAdjustmentPolicyApplyFn)(
    const PitAccountAdjustmentContext * ctx,
    PitParamAccountId account_id,
    const PitAccountAdjustment * adjustment,
    PitMutations * mutations,
    void * user_data
);

/**
 * Callback invoked when the last reference to a custom account-adjustment
 * policy is released and the policy object is about to be destroyed.
 *
 * Contract:
 * - Called exactly once, on the thread that drops the last policy reference.
 * - After this callback returns, no further callbacks will be invoked for
 * this policy instance.
 * - `user_data` is the same value that was passed at policy creation.
 * - The callback must release any resources associated with `user_data`.
 */
typedef void (*PitAccountAdjustmentPolicyFreeUserDataFn)(
    void * user_data
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
bool pit_create_param_pnl(
    PitParamDecimal value,
    PitParamPnl * out,
    PitOutParamError out_error
);

/**
 * Returns the decimal stored in `Pnl`.
 */
PitParamDecimal pit_param_pnl_get_decimal(
    PitParamPnl value
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
bool pit_create_param_price(
    PitParamDecimal value,
    PitParamPrice * out,
    PitOutParamError out_error
);

/**
 * Returns the decimal stored in `Price`.
 */
PitParamDecimal pit_param_price_get_decimal(
    PitParamPrice value
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
bool pit_create_param_quantity(
    PitParamDecimal value,
    PitParamQuantity * out,
    PitOutParamError out_error
);

/**
 * Returns the decimal stored in `Quantity`.
 */
PitParamDecimal pit_param_quantity_get_decimal(
    PitParamQuantity value
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
bool pit_create_param_volume(
    PitParamDecimal value,
    PitParamVolume * out,
    PitOutParamError out_error
);

/**
 * Returns the decimal stored in `Volume`.
 */
PitParamDecimal pit_param_volume_get_decimal(
    PitParamVolume value
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
bool pit_create_param_cash_flow(
    PitParamDecimal value,
    PitParamCashFlow * out,
    PitOutParamError out_error
);

/**
 * Returns the decimal stored in `CashFlow`.
 */
PitParamDecimal pit_param_cash_flow_get_decimal(
    PitParamCashFlow value
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
bool pit_create_param_position_size(
    PitParamDecimal value,
    PitParamPositionSize * out,
    PitOutParamError out_error
);

/**
 * Returns the decimal stored in `PositionSize`.
 */
PitParamDecimal pit_param_position_size_get_decimal(
    PitParamPositionSize value
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
bool pit_create_param_fee(
    PitParamDecimal value,
    PitParamFee * out,
    PitOutParamError out_error
);

/**
 * Returns the decimal stored in `Fee`.
 */
PitParamDecimal pit_param_fee_get_decimal(
    PitParamFee value
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
bool pit_create_param_notional(
    PitParamDecimal value,
    PitParamNotional * out,
    PitOutParamError out_error
);

/**
 * Returns the decimal stored in `Notional`.
 */
PitParamDecimal pit_param_notional_get_decimal(
    PitParamNotional value
);

bool pit_create_param_pnl_from_str(
    PitStringView value,
    PitParamPnl * out,
    PitOutParamError out_error
);

bool pit_create_param_pnl_from_f64(
    double value,
    PitParamPnl * out,
    PitOutParamError out_error
);

bool pit_create_param_pnl_from_i64(
    int64_t value,
    PitParamPnl * out,
    PitOutParamError out_error
);

bool pit_create_param_pnl_from_u64(
    uint64_t value,
    PitParamPnl * out,
    PitOutParamError out_error
);

bool pit_create_param_pnl_from_str_rounded(
    PitStringView value,
    uint32_t scale,
    PitParamRoundingStrategy rounding,
    PitParamPnl * out,
    PitOutParamError out_error
);

bool pit_create_param_pnl_from_f64_rounded(
    double value,
    uint32_t scale,
    PitParamRoundingStrategy rounding,
    PitParamPnl * out,
    PitOutParamError out_error
);

bool pit_create_param_pnl_from_decimal_rounded(
    PitParamDecimal value,
    uint32_t scale,
    PitParamRoundingStrategy rounding,
    PitParamPnl * out,
    PitOutParamError out_error
);

bool pit_param_pnl_to_f64(
    PitParamPnl value,
    double * out,
    PitOutParamError out_error
);

bool pit_param_pnl_is_zero(
    PitParamPnl value,
    bool * out,
    PitOutParamError out_error
);

bool pit_param_pnl_compare(
    PitParamPnl lhs,
    PitParamPnl rhs,
    int8_t * out,
    PitOutParamError out_error
);

PitSharedString * pit_param_pnl_to_string(
    PitParamPnl value,
    PitOutParamError out_error
);

bool pit_param_pnl_checked_add(
    PitParamPnl lhs,
    PitParamPnl rhs,
    PitParamPnl * out,
    PitOutParamError out_error
);

bool pit_param_pnl_checked_sub(
    PitParamPnl lhs,
    PitParamPnl rhs,
    PitParamPnl * out,
    PitOutParamError out_error
);

bool pit_param_pnl_checked_mul_i64(
    PitParamPnl value,
    int64_t multiplier,
    PitParamPnl * out,
    PitOutParamError out_error
);

bool pit_param_pnl_checked_mul_u64(
    PitParamPnl value,
    uint64_t multiplier,
    PitParamPnl * out,
    PitOutParamError out_error
);

bool pit_param_pnl_checked_mul_f64(
    PitParamPnl value,
    double multiplier,
    PitParamPnl * out,
    PitOutParamError out_error
);

bool pit_param_pnl_checked_div_i64(
    PitParamPnl value,
    int64_t divisor,
    PitParamPnl * out,
    PitOutParamError out_error
);

bool pit_param_pnl_checked_div_u64(
    PitParamPnl value,
    uint64_t divisor,
    PitParamPnl * out,
    PitOutParamError out_error
);

bool pit_param_pnl_checked_div_f64(
    PitParamPnl value,
    double divisor,
    PitParamPnl * out,
    PitOutParamError out_error
);

bool pit_param_pnl_checked_rem_i64(
    PitParamPnl value,
    int64_t divisor,
    PitParamPnl * out,
    PitOutParamError out_error
);

bool pit_param_pnl_checked_rem_u64(
    PitParamPnl value,
    uint64_t divisor,
    PitParamPnl * out,
    PitOutParamError out_error
);

bool pit_param_pnl_checked_rem_f64(
    PitParamPnl value,
    double divisor,
    PitParamPnl * out,
    PitOutParamError out_error
);

bool pit_param_pnl_checked_neg(
    PitParamPnl value,
    PitParamPnl * out,
    PitOutParamError out_error
);

bool pit_create_param_price_from_str(
    PitStringView value,
    PitParamPrice * out,
    PitOutParamError out_error
);

bool pit_create_param_price_from_f64(
    double value,
    PitParamPrice * out,
    PitOutParamError out_error
);

bool pit_create_param_price_from_i64(
    int64_t value,
    PitParamPrice * out,
    PitOutParamError out_error
);

bool pit_create_param_price_from_u64(
    uint64_t value,
    PitParamPrice * out,
    PitOutParamError out_error
);

bool pit_create_param_price_from_str_rounded(
    PitStringView value,
    uint32_t scale,
    PitParamRoundingStrategy rounding,
    PitParamPrice * out,
    PitOutParamError out_error
);

bool pit_create_param_price_from_f64_rounded(
    double value,
    uint32_t scale,
    PitParamRoundingStrategy rounding,
    PitParamPrice * out,
    PitOutParamError out_error
);

bool pit_create_param_price_from_decimal_rounded(
    PitParamDecimal value,
    uint32_t scale,
    PitParamRoundingStrategy rounding,
    PitParamPrice * out,
    PitOutParamError out_error
);

bool pit_param_price_to_f64(
    PitParamPrice value,
    double * out,
    PitOutParamError out_error
);

bool pit_param_price_is_zero(
    PitParamPrice value,
    bool * out,
    PitOutParamError out_error
);

bool pit_param_price_compare(
    PitParamPrice lhs,
    PitParamPrice rhs,
    int8_t * out,
    PitOutParamError out_error
);

PitSharedString * pit_param_price_to_string(
    PitParamPrice value,
    PitOutParamError out_error
);

bool pit_param_price_checked_add(
    PitParamPrice lhs,
    PitParamPrice rhs,
    PitParamPrice * out,
    PitOutParamError out_error
);

bool pit_param_price_checked_sub(
    PitParamPrice lhs,
    PitParamPrice rhs,
    PitParamPrice * out,
    PitOutParamError out_error
);

bool pit_param_price_checked_mul_i64(
    PitParamPrice value,
    int64_t multiplier,
    PitParamPrice * out,
    PitOutParamError out_error
);

bool pit_param_price_checked_mul_u64(
    PitParamPrice value,
    uint64_t multiplier,
    PitParamPrice * out,
    PitOutParamError out_error
);

bool pit_param_price_checked_mul_f64(
    PitParamPrice value,
    double multiplier,
    PitParamPrice * out,
    PitOutParamError out_error
);

bool pit_param_price_checked_div_i64(
    PitParamPrice value,
    int64_t divisor,
    PitParamPrice * out,
    PitOutParamError out_error
);

bool pit_param_price_checked_div_u64(
    PitParamPrice value,
    uint64_t divisor,
    PitParamPrice * out,
    PitOutParamError out_error
);

bool pit_param_price_checked_div_f64(
    PitParamPrice value,
    double divisor,
    PitParamPrice * out,
    PitOutParamError out_error
);

bool pit_param_price_checked_rem_i64(
    PitParamPrice value,
    int64_t divisor,
    PitParamPrice * out,
    PitOutParamError out_error
);

bool pit_param_price_checked_rem_u64(
    PitParamPrice value,
    uint64_t divisor,
    PitParamPrice * out,
    PitOutParamError out_error
);

bool pit_param_price_checked_rem_f64(
    PitParamPrice value,
    double divisor,
    PitParamPrice * out,
    PitOutParamError out_error
);

bool pit_param_price_checked_neg(
    PitParamPrice value,
    PitParamPrice * out,
    PitOutParamError out_error
);

bool pit_create_param_quantity_from_str(
    PitStringView value,
    PitParamQuantity * out,
    PitOutParamError out_error
);

bool pit_create_param_quantity_from_f64(
    double value,
    PitParamQuantity * out,
    PitOutParamError out_error
);

bool pit_create_param_quantity_from_i64(
    int64_t value,
    PitParamQuantity * out,
    PitOutParamError out_error
);

bool pit_create_param_quantity_from_u64(
    uint64_t value,
    PitParamQuantity * out,
    PitOutParamError out_error
);

bool pit_create_param_quantity_from_str_rounded(
    PitStringView value,
    uint32_t scale,
    PitParamRoundingStrategy rounding,
    PitParamQuantity * out,
    PitOutParamError out_error
);

bool pit_create_param_quantity_from_f64_rounded(
    double value,
    uint32_t scale,
    PitParamRoundingStrategy rounding,
    PitParamQuantity * out,
    PitOutParamError out_error
);

bool pit_create_param_quantity_from_decimal_rounded(
    PitParamDecimal value,
    uint32_t scale,
    PitParamRoundingStrategy rounding,
    PitParamQuantity * out,
    PitOutParamError out_error
);

bool pit_param_quantity_to_f64(
    PitParamQuantity value,
    double * out,
    PitOutParamError out_error
);

bool pit_param_quantity_is_zero(
    PitParamQuantity value,
    bool * out,
    PitOutParamError out_error
);

bool pit_param_quantity_compare(
    PitParamQuantity lhs,
    PitParamQuantity rhs,
    int8_t * out,
    PitOutParamError out_error
);

PitSharedString * pit_param_quantity_to_string(
    PitParamQuantity value,
    PitOutParamError out_error
);

bool pit_param_quantity_checked_add(
    PitParamQuantity lhs,
    PitParamQuantity rhs,
    PitParamQuantity * out,
    PitOutParamError out_error
);

bool pit_param_quantity_checked_sub(
    PitParamQuantity lhs,
    PitParamQuantity rhs,
    PitParamQuantity * out,
    PitOutParamError out_error
);

bool pit_param_quantity_checked_mul_i64(
    PitParamQuantity value,
    int64_t multiplier,
    PitParamQuantity * out,
    PitOutParamError out_error
);

bool pit_param_quantity_checked_mul_u64(
    PitParamQuantity value,
    uint64_t multiplier,
    PitParamQuantity * out,
    PitOutParamError out_error
);

bool pit_param_quantity_checked_mul_f64(
    PitParamQuantity value,
    double multiplier,
    PitParamQuantity * out,
    PitOutParamError out_error
);

bool pit_param_quantity_checked_div_i64(
    PitParamQuantity value,
    int64_t divisor,
    PitParamQuantity * out,
    PitOutParamError out_error
);

bool pit_param_quantity_checked_div_u64(
    PitParamQuantity value,
    uint64_t divisor,
    PitParamQuantity * out,
    PitOutParamError out_error
);

bool pit_param_quantity_checked_div_f64(
    PitParamQuantity value,
    double divisor,
    PitParamQuantity * out,
    PitOutParamError out_error
);

bool pit_param_quantity_checked_rem_i64(
    PitParamQuantity value,
    int64_t divisor,
    PitParamQuantity * out,
    PitOutParamError out_error
);

bool pit_param_quantity_checked_rem_u64(
    PitParamQuantity value,
    uint64_t divisor,
    PitParamQuantity * out,
    PitOutParamError out_error
);

bool pit_param_quantity_checked_rem_f64(
    PitParamQuantity value,
    double divisor,
    PitParamQuantity * out,
    PitOutParamError out_error
);

bool pit_create_param_volume_from_str(
    PitStringView value,
    PitParamVolume * out,
    PitOutParamError out_error
);

bool pit_create_param_volume_from_f64(
    double value,
    PitParamVolume * out,
    PitOutParamError out_error
);

bool pit_create_param_volume_from_i64(
    int64_t value,
    PitParamVolume * out,
    PitOutParamError out_error
);

bool pit_create_param_volume_from_u64(
    uint64_t value,
    PitParamVolume * out,
    PitOutParamError out_error
);

bool pit_create_param_volume_from_str_rounded(
    PitStringView value,
    uint32_t scale,
    PitParamRoundingStrategy rounding,
    PitParamVolume * out,
    PitOutParamError out_error
);

bool pit_create_param_volume_from_f64_rounded(
    double value,
    uint32_t scale,
    PitParamRoundingStrategy rounding,
    PitParamVolume * out,
    PitOutParamError out_error
);

bool pit_create_param_volume_from_decimal_rounded(
    PitParamDecimal value,
    uint32_t scale,
    PitParamRoundingStrategy rounding,
    PitParamVolume * out,
    PitOutParamError out_error
);

bool pit_param_volume_to_f64(
    PitParamVolume value,
    double * out,
    PitOutParamError out_error
);

bool pit_param_volume_is_zero(
    PitParamVolume value,
    bool * out,
    PitOutParamError out_error
);

bool pit_param_volume_compare(
    PitParamVolume lhs,
    PitParamVolume rhs,
    int8_t * out,
    PitOutParamError out_error
);

PitSharedString * pit_param_volume_to_string(
    PitParamVolume value,
    PitOutParamError out_error
);

bool pit_param_volume_checked_add(
    PitParamVolume lhs,
    PitParamVolume rhs,
    PitParamVolume * out,
    PitOutParamError out_error
);

bool pit_param_volume_checked_sub(
    PitParamVolume lhs,
    PitParamVolume rhs,
    PitParamVolume * out,
    PitOutParamError out_error
);

bool pit_param_volume_checked_mul_i64(
    PitParamVolume value,
    int64_t multiplier,
    PitParamVolume * out,
    PitOutParamError out_error
);

bool pit_param_volume_checked_mul_u64(
    PitParamVolume value,
    uint64_t multiplier,
    PitParamVolume * out,
    PitOutParamError out_error
);

bool pit_param_volume_checked_mul_f64(
    PitParamVolume value,
    double multiplier,
    PitParamVolume * out,
    PitOutParamError out_error
);

bool pit_param_volume_checked_div_i64(
    PitParamVolume value,
    int64_t divisor,
    PitParamVolume * out,
    PitOutParamError out_error
);

bool pit_param_volume_checked_div_u64(
    PitParamVolume value,
    uint64_t divisor,
    PitParamVolume * out,
    PitOutParamError out_error
);

bool pit_param_volume_checked_div_f64(
    PitParamVolume value,
    double divisor,
    PitParamVolume * out,
    PitOutParamError out_error
);

bool pit_param_volume_checked_rem_i64(
    PitParamVolume value,
    int64_t divisor,
    PitParamVolume * out,
    PitOutParamError out_error
);

bool pit_param_volume_checked_rem_u64(
    PitParamVolume value,
    uint64_t divisor,
    PitParamVolume * out,
    PitOutParamError out_error
);

bool pit_param_volume_checked_rem_f64(
    PitParamVolume value,
    double divisor,
    PitParamVolume * out,
    PitOutParamError out_error
);

bool pit_create_param_cash_flow_from_str(
    PitStringView value,
    PitParamCashFlow * out,
    PitOutParamError out_error
);

bool pit_create_param_cash_flow_from_f64(
    double value,
    PitParamCashFlow * out,
    PitOutParamError out_error
);

bool pit_create_param_cash_flow_from_i64(
    int64_t value,
    PitParamCashFlow * out,
    PitOutParamError out_error
);

bool pit_create_param_cash_flow_from_u64(
    uint64_t value,
    PitParamCashFlow * out,
    PitOutParamError out_error
);

bool pit_create_param_cash_flow_from_str_rounded(
    PitStringView value,
    uint32_t scale,
    PitParamRoundingStrategy rounding,
    PitParamCashFlow * out,
    PitOutParamError out_error
);

bool pit_create_param_cash_flow_from_f64_rounded(
    double value,
    uint32_t scale,
    PitParamRoundingStrategy rounding,
    PitParamCashFlow * out,
    PitOutParamError out_error
);

bool pit_create_param_cash_flow_from_decimal_rounded(
    PitParamDecimal value,
    uint32_t scale,
    PitParamRoundingStrategy rounding,
    PitParamCashFlow * out,
    PitOutParamError out_error
);

bool pit_param_cash_flow_to_f64(
    PitParamCashFlow value,
    double * out,
    PitOutParamError out_error
);

bool pit_param_cash_flow_is_zero(
    PitParamCashFlow value,
    bool * out,
    PitOutParamError out_error
);

bool pit_param_cash_flow_compare(
    PitParamCashFlow lhs,
    PitParamCashFlow rhs,
    int8_t * out,
    PitOutParamError out_error
);

PitSharedString * pit_param_cash_flow_to_string(
    PitParamCashFlow value,
    PitOutParamError out_error
);

bool pit_param_cash_flow_checked_add(
    PitParamCashFlow lhs,
    PitParamCashFlow rhs,
    PitParamCashFlow * out,
    PitOutParamError out_error
);

bool pit_param_cash_flow_checked_sub(
    PitParamCashFlow lhs,
    PitParamCashFlow rhs,
    PitParamCashFlow * out,
    PitOutParamError out_error
);

bool pit_param_cash_flow_checked_mul_i64(
    PitParamCashFlow value,
    int64_t multiplier,
    PitParamCashFlow * out,
    PitOutParamError out_error
);

bool pit_param_cash_flow_checked_mul_u64(
    PitParamCashFlow value,
    uint64_t multiplier,
    PitParamCashFlow * out,
    PitOutParamError out_error
);

bool pit_param_cash_flow_checked_mul_f64(
    PitParamCashFlow value,
    double multiplier,
    PitParamCashFlow * out,
    PitOutParamError out_error
);

bool pit_param_cash_flow_checked_div_i64(
    PitParamCashFlow value,
    int64_t divisor,
    PitParamCashFlow * out,
    PitOutParamError out_error
);

bool pit_param_cash_flow_checked_div_u64(
    PitParamCashFlow value,
    uint64_t divisor,
    PitParamCashFlow * out,
    PitOutParamError out_error
);

bool pit_param_cash_flow_checked_div_f64(
    PitParamCashFlow value,
    double divisor,
    PitParamCashFlow * out,
    PitOutParamError out_error
);

bool pit_param_cash_flow_checked_rem_i64(
    PitParamCashFlow value,
    int64_t divisor,
    PitParamCashFlow * out,
    PitOutParamError out_error
);

bool pit_param_cash_flow_checked_rem_u64(
    PitParamCashFlow value,
    uint64_t divisor,
    PitParamCashFlow * out,
    PitOutParamError out_error
);

bool pit_param_cash_flow_checked_rem_f64(
    PitParamCashFlow value,
    double divisor,
    PitParamCashFlow * out,
    PitOutParamError out_error
);

bool pit_param_cash_flow_checked_neg(
    PitParamCashFlow value,
    PitParamCashFlow * out,
    PitOutParamError out_error
);

bool pit_create_param_position_size_from_str(
    PitStringView value,
    PitParamPositionSize * out,
    PitOutParamError out_error
);

bool pit_create_param_position_size_from_f64(
    double value,
    PitParamPositionSize * out,
    PitOutParamError out_error
);

bool pit_create_param_position_size_from_i64(
    int64_t value,
    PitParamPositionSize * out,
    PitOutParamError out_error
);

bool pit_create_param_position_size_from_u64(
    uint64_t value,
    PitParamPositionSize * out,
    PitOutParamError out_error
);

bool pit_create_param_position_size_from_str_rounded(
    PitStringView value,
    uint32_t scale,
    PitParamRoundingStrategy rounding,
    PitParamPositionSize * out,
    PitOutParamError out_error
);

bool pit_create_param_position_size_from_f64_rounded(
    double value,
    uint32_t scale,
    PitParamRoundingStrategy rounding,
    PitParamPositionSize * out,
    PitOutParamError out_error
);

bool pit_create_param_position_size_from_decimal_rounded(
    PitParamDecimal value,
    uint32_t scale,
    PitParamRoundingStrategy rounding,
    PitParamPositionSize * out,
    PitOutParamError out_error
);

bool pit_param_position_size_to_f64(
    PitParamPositionSize value,
    double * out,
    PitOutParamError out_error
);

bool pit_param_position_size_is_zero(
    PitParamPositionSize value,
    bool * out,
    PitOutParamError out_error
);

bool pit_param_position_size_compare(
    PitParamPositionSize lhs,
    PitParamPositionSize rhs,
    int8_t * out,
    PitOutParamError out_error
);

PitSharedString * pit_param_position_size_to_string(
    PitParamPositionSize value,
    PitOutParamError out_error
);

bool pit_param_position_size_checked_add(
    PitParamPositionSize lhs,
    PitParamPositionSize rhs,
    PitParamPositionSize * out,
    PitOutParamError out_error
);

bool pit_param_position_size_checked_sub(
    PitParamPositionSize lhs,
    PitParamPositionSize rhs,
    PitParamPositionSize * out,
    PitOutParamError out_error
);

bool pit_param_position_size_checked_mul_i64(
    PitParamPositionSize value,
    int64_t multiplier,
    PitParamPositionSize * out,
    PitOutParamError out_error
);

bool pit_param_position_size_checked_mul_u64(
    PitParamPositionSize value,
    uint64_t multiplier,
    PitParamPositionSize * out,
    PitOutParamError out_error
);

bool pit_param_position_size_checked_mul_f64(
    PitParamPositionSize value,
    double multiplier,
    PitParamPositionSize * out,
    PitOutParamError out_error
);

bool pit_param_position_size_checked_div_i64(
    PitParamPositionSize value,
    int64_t divisor,
    PitParamPositionSize * out,
    PitOutParamError out_error
);

bool pit_param_position_size_checked_div_u64(
    PitParamPositionSize value,
    uint64_t divisor,
    PitParamPositionSize * out,
    PitOutParamError out_error
);

bool pit_param_position_size_checked_div_f64(
    PitParamPositionSize value,
    double divisor,
    PitParamPositionSize * out,
    PitOutParamError out_error
);

bool pit_param_position_size_checked_rem_i64(
    PitParamPositionSize value,
    int64_t divisor,
    PitParamPositionSize * out,
    PitOutParamError out_error
);

bool pit_param_position_size_checked_rem_u64(
    PitParamPositionSize value,
    uint64_t divisor,
    PitParamPositionSize * out,
    PitOutParamError out_error
);

bool pit_param_position_size_checked_rem_f64(
    PitParamPositionSize value,
    double divisor,
    PitParamPositionSize * out,
    PitOutParamError out_error
);

bool pit_param_position_size_checked_neg(
    PitParamPositionSize value,
    PitParamPositionSize * out,
    PitOutParamError out_error
);

bool pit_create_param_fee_from_str(
    PitStringView value,
    PitParamFee * out,
    PitOutParamError out_error
);

bool pit_create_param_fee_from_f64(
    double value,
    PitParamFee * out,
    PitOutParamError out_error
);

bool pit_create_param_fee_from_i64(
    int64_t value,
    PitParamFee * out,
    PitOutParamError out_error
);

bool pit_create_param_fee_from_u64(
    uint64_t value,
    PitParamFee * out,
    PitOutParamError out_error
);

bool pit_create_param_fee_from_str_rounded(
    PitStringView value,
    uint32_t scale,
    PitParamRoundingStrategy rounding,
    PitParamFee * out,
    PitOutParamError out_error
);

bool pit_create_param_fee_from_f64_rounded(
    double value,
    uint32_t scale,
    PitParamRoundingStrategy rounding,
    PitParamFee * out,
    PitOutParamError out_error
);

bool pit_create_param_fee_from_decimal_rounded(
    PitParamDecimal value,
    uint32_t scale,
    PitParamRoundingStrategy rounding,
    PitParamFee * out,
    PitOutParamError out_error
);

bool pit_param_fee_to_f64(
    PitParamFee value,
    double * out,
    PitOutParamError out_error
);

bool pit_param_fee_is_zero(
    PitParamFee value,
    bool * out,
    PitOutParamError out_error
);

bool pit_param_fee_compare(
    PitParamFee lhs,
    PitParamFee rhs,
    int8_t * out,
    PitOutParamError out_error
);

PitSharedString * pit_param_fee_to_string(
    PitParamFee value,
    PitOutParamError out_error
);

bool pit_param_fee_checked_add(
    PitParamFee lhs,
    PitParamFee rhs,
    PitParamFee * out,
    PitOutParamError out_error
);

bool pit_param_fee_checked_sub(
    PitParamFee lhs,
    PitParamFee rhs,
    PitParamFee * out,
    PitOutParamError out_error
);

bool pit_param_fee_checked_mul_i64(
    PitParamFee value,
    int64_t multiplier,
    PitParamFee * out,
    PitOutParamError out_error
);

bool pit_param_fee_checked_mul_u64(
    PitParamFee value,
    uint64_t multiplier,
    PitParamFee * out,
    PitOutParamError out_error
);

bool pit_param_fee_checked_mul_f64(
    PitParamFee value,
    double multiplier,
    PitParamFee * out,
    PitOutParamError out_error
);

bool pit_param_fee_checked_div_i64(
    PitParamFee value,
    int64_t divisor,
    PitParamFee * out,
    PitOutParamError out_error
);

bool pit_param_fee_checked_div_u64(
    PitParamFee value,
    uint64_t divisor,
    PitParamFee * out,
    PitOutParamError out_error
);

bool pit_param_fee_checked_div_f64(
    PitParamFee value,
    double divisor,
    PitParamFee * out,
    PitOutParamError out_error
);

bool pit_param_fee_checked_rem_i64(
    PitParamFee value,
    int64_t divisor,
    PitParamFee * out,
    PitOutParamError out_error
);

bool pit_param_fee_checked_rem_u64(
    PitParamFee value,
    uint64_t divisor,
    PitParamFee * out,
    PitOutParamError out_error
);

bool pit_param_fee_checked_rem_f64(
    PitParamFee value,
    double divisor,
    PitParamFee * out,
    PitOutParamError out_error
);

bool pit_param_fee_checked_neg(
    PitParamFee value,
    PitParamFee * out,
    PitOutParamError out_error
);

bool pit_create_param_notional_from_str(
    PitStringView value,
    PitParamNotional * out,
    PitOutParamError out_error
);

bool pit_create_param_notional_from_f64(
    double value,
    PitParamNotional * out,
    PitOutParamError out_error
);

bool pit_create_param_notional_from_i64(
    int64_t value,
    PitParamNotional * out,
    PitOutParamError out_error
);

bool pit_create_param_notional_from_u64(
    uint64_t value,
    PitParamNotional * out,
    PitOutParamError out_error
);

bool pit_create_param_notional_from_str_rounded(
    PitStringView value,
    uint32_t scale,
    PitParamRoundingStrategy rounding,
    PitParamNotional * out,
    PitOutParamError out_error
);

bool pit_create_param_notional_from_f64_rounded(
    double value,
    uint32_t scale,
    PitParamRoundingStrategy rounding,
    PitParamNotional * out,
    PitOutParamError out_error
);

bool pit_create_param_notional_from_decimal_rounded(
    PitParamDecimal value,
    uint32_t scale,
    PitParamRoundingStrategy rounding,
    PitParamNotional * out,
    PitOutParamError out_error
);

bool pit_param_notional_to_f64(
    PitParamNotional value,
    double * out,
    PitOutParamError out_error
);

bool pit_param_notional_is_zero(
    PitParamNotional value,
    bool * out,
    PitOutParamError out_error
);

bool pit_param_notional_compare(
    PitParamNotional lhs,
    PitParamNotional rhs,
    int8_t * out,
    PitOutParamError out_error
);

PitSharedString * pit_param_notional_to_string(
    PitParamNotional value,
    PitOutParamError out_error
);

bool pit_param_notional_checked_add(
    PitParamNotional lhs,
    PitParamNotional rhs,
    PitParamNotional * out,
    PitOutParamError out_error
);

bool pit_param_notional_checked_sub(
    PitParamNotional lhs,
    PitParamNotional rhs,
    PitParamNotional * out,
    PitOutParamError out_error
);

bool pit_param_notional_checked_mul_i64(
    PitParamNotional value,
    int64_t multiplier,
    PitParamNotional * out,
    PitOutParamError out_error
);

bool pit_param_notional_checked_mul_u64(
    PitParamNotional value,
    uint64_t multiplier,
    PitParamNotional * out,
    PitOutParamError out_error
);

bool pit_param_notional_checked_mul_f64(
    PitParamNotional value,
    double multiplier,
    PitParamNotional * out,
    PitOutParamError out_error
);

bool pit_param_notional_checked_div_i64(
    PitParamNotional value,
    int64_t divisor,
    PitParamNotional * out,
    PitOutParamError out_error
);

bool pit_param_notional_checked_div_u64(
    PitParamNotional value,
    uint64_t divisor,
    PitParamNotional * out,
    PitOutParamError out_error
);

bool pit_param_notional_checked_div_f64(
    PitParamNotional value,
    double divisor,
    PitParamNotional * out,
    PitOutParamError out_error
);

bool pit_param_notional_checked_rem_i64(
    PitParamNotional value,
    int64_t divisor,
    PitParamNotional * out,
    PitOutParamError out_error
);

bool pit_param_notional_checked_rem_u64(
    PitParamNotional value,
    uint64_t divisor,
    PitParamNotional * out,
    PitOutParamError out_error
);

bool pit_param_notional_checked_rem_f64(
    PitParamNotional value,
    double divisor,
    PitParamNotional * out,
    PitOutParamError out_error
);

bool pit_param_leverage_calculate_margin_required(
    PitParamLeverage leverage,
    PitParamNotional notional,
    PitParamNotional * out,
    PitOutParamError out_error
);

bool pit_param_price_calculate_volume(
    PitParamPrice price,
    PitParamQuantity quantity,
    PitParamVolume * out,
    PitOutParamError out_error
);

bool pit_param_quantity_calculate_volume(
    PitParamQuantity quantity,
    PitParamPrice price,
    PitParamVolume * out,
    PitOutParamError out_error
);

bool pit_param_volume_calculate_quantity(
    PitParamVolume volume,
    PitParamPrice price,
    PitParamQuantity * out,
    PitOutParamError out_error
);

bool pit_param_pnl_to_cash_flow(
    PitParamPnl value,
    PitParamCashFlow * out,
    PitOutParamError out_error
);

bool pit_param_pnl_to_position_size(
    PitParamPnl value,
    PitParamPositionSize * out,
    PitOutParamError out_error
);

bool pit_param_pnl_from_fee(
    PitParamFee fee,
    PitParamPnl * out,
    PitOutParamError out_error
);

bool pit_param_cash_flow_from_pnl(
    PitParamPnl pnl,
    PitParamCashFlow * out,
    PitOutParamError out_error
);

bool pit_param_cash_flow_from_fee(
    PitParamFee fee,
    PitParamCashFlow * out,
    PitOutParamError out_error
);

bool pit_param_cash_flow_from_volume_inflow(
    PitParamVolume volume,
    PitParamCashFlow * out,
    PitOutParamError out_error
);

bool pit_param_cash_flow_from_volume_outflow(
    PitParamVolume volume,
    PitParamCashFlow * out,
    PitOutParamError out_error
);

bool pit_param_fee_to_pnl(
    PitParamFee fee,
    PitParamPnl * out,
    PitOutParamError out_error
);

bool pit_param_fee_to_position_size(
    PitParamFee fee,
    PitParamPositionSize * out,
    PitOutParamError out_error
);

bool pit_param_fee_to_cash_flow(
    PitParamFee fee,
    PitParamCashFlow * out,
    PitOutParamError out_error
);

bool pit_param_volume_to_cash_flow_inflow(
    PitParamVolume volume,
    PitParamCashFlow * out,
    PitOutParamError out_error
);

bool pit_param_volume_to_cash_flow_outflow(
    PitParamVolume volume,
    PitParamCashFlow * out,
    PitOutParamError out_error
);

bool pit_param_position_size_from_pnl(
    PitParamPnl pnl,
    PitParamPositionSize * out,
    PitOutParamError out_error
);

bool pit_param_position_size_from_fee(
    PitParamFee fee,
    PitParamPositionSize * out,
    PitOutParamError out_error
);

bool pit_param_position_size_from_quantity_and_side(
    PitParamQuantity quantity,
    PitParamSide side,
    PitParamPositionSize * out,
    PitOutParamError out_error
);

bool pit_param_position_size_to_open_quantity(
    PitParamPositionSize value,
    PitParamQuantity * out_quantity,
    PitParamSide * out_side,
    PitOutParamError out_error
);

bool pit_param_position_size_to_close_quantity(
    PitParamPositionSize value,
    PitParamQuantity * out_quantity,
    PitParamSide * out_side,
    PitOutParamError out_error
);

bool pit_param_position_size_checked_add_quantity(
    PitParamPositionSize value,
    PitParamQuantity quantity,
    PitParamSide side,
    PitParamPositionSize * out,
    PitOutParamError out_error
);

bool pit_param_price_calculate_notional(
    PitParamPrice price,
    PitParamQuantity quantity,
    PitParamNotional * out,
    PitOutParamError out_error
);

bool pit_param_quantity_calculate_notional(
    PitParamQuantity quantity,
    PitParamPrice price,
    PitParamNotional * out,
    PitOutParamError out_error
);

bool pit_param_notional_from_volume(
    PitParamVolume volume,
    PitParamNotional * out,
    PitOutParamError out_error
);

bool pit_param_notional_to_volume(
    PitParamNotional notional,
    PitParamVolume * out,
    PitOutParamError out_error
);

bool pit_param_notional_calculate_margin_required(
    PitParamNotional notional,
    PitParamLeverage leverage,
    PitParamNotional * out,
    PitOutParamError out_error
);

bool pit_param_volume_from_notional(
    PitParamNotional notional,
    PitParamVolume * out,
    PitOutParamError out_error
);

/**
 * Constructs an account identifier from a 64-bit integer.
 *
 * This is a direct numeric mapping with no collision risk.
 *
 * WARNING:
 * Do not mix IDs produced by this function with IDs produced by
 * `pit_create_param_account_id_from_str` in the same runtime state.
 *
 * Contract:
 * - returns a stable account identifier value;
 * - this function always succeeds.
 */
PitParamAccountId pit_create_param_account_id_from_u64(
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
 *   collision
 * is approximately `n^2 / 2^65`.
 * - if collision risk is unacceptable, keep your own collision-free
 * string-to-integer mapping and use `pit_create_param_account_id_from_u64`.
 *
 * The previous sentence is why this helper is suitable for stable adapter-side
 * mapping, but not for workflows that require guaranteed uniqueness.
 *
 * WARNING:
 * Do not mix IDs produced by this function with IDs produced by
 * `pit_create_param_account_id_from_u64` in the same runtime state.
 *
 * Contract:
 * - returns `true` and writes a stable account identifier to `out` on
 *   success;
 * - returns `false` on invalid input and optionally writes `PitParamError`.
 *
 * # Safety
 *
 * `value.ptr` must be non-null and point to at least `value.len` readable
 * UTF-8 bytes.
 */
bool pit_create_param_account_id_from_str(
    PitStringView value,
    PitParamAccountId * out,
    PitOutParamError out_error
);

/**
 * Validates and copies an asset identifier into a caller-owned shared-string
 * handle.
 *
 * The returned handle must be destroyed with `pit_destroy_param_asset`.
 */
PitSharedString * pit_create_param_asset_from_str(
    PitStringView value,
    PitOutParamError out_error
);

/**
 * Destroys a caller-owned asset handle created by
 * `pit_create_param_asset_from_str`.
 */
void pit_destroy_param_asset(
    PitSharedString * handle
);

/**
 * Creates a caller-owned reject list with preallocated capacity.
 *
 * `reserve` is the requested number of elements to preallocate.
 *
 * Contract:
 * - returns a new caller-owned list;
 * - release it with `pit_destroy_reject_list`;
 * - this function always succeeds.
 */
PitRejectList * pit_create_reject_list(
    size_t reserve
);

/**
 * Releases a caller-owned reject list.
 *
 * Contract:
 * - passing null is allowed;
 * - this function always succeeds.
 */
void pit_destroy_reject_list(
    PitRejectList * rejects
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
void pit_reject_list_push(
    PitRejectList * list,
    PitReject reject
);

/**
 * Returns the number of rejects in the list.
 *
 * Contract:
 * - `list` must be a valid non-null pointer;
 * - this function never fails;
 * - violating the pointer contract aborts the call.
 */
size_t pit_reject_list_len(
    const PitRejectList * list
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
 * `out_reject`;
 * - the copied view remains valid while `list` is alive and unchanged;
 * - this function never fails;
 * - violating the pointer contract aborts the call.
 */
bool pit_reject_list_get(
    const PitRejectList * list,
    size_t index,
    PitReject * out_reject
);

/**
 * Releases a caller-owned parameter error container.
 *
 * # Safety
 *
 * `handle` must be either null or a pointer returned by this library through
 * `PitOutParamError`. The handle must be destroyed at most once.
 */
void pit_destroy_param_error(
    PitParamError * handle
);

/**
 * Creates a new engine builder.
 *
 * Contract:
 * - returns a new caller-owned builder object;
 * - this function always succeeds.
 *
 * Cleanup:
 * - release the pointer with `pit_destroy_engine_builder` if you stop before
 * building;
 * - after a successful build the builder is consumed and must still be
 * released with `pit_destroy_engine_builder`.
 */
PitEngineBuilder * pit_create_engine_builder(void);

/**
 * Releases a builder pointer owned by the caller.
 *
 * Contract:
 * - passing null is allowed;
 * - after this call the pointer is invalid;
 * - this function always succeeds.
 */
void pit_destroy_engine_builder(
    PitEngineBuilder * builder
);

/**
 * Finalizes a builder and creates an engine.
 *
 * Success:
 * - returns a non-null engine pointer.
 *
 * Error:
 * - returns null when `builder` is null, the builder was already consumed,
 *   or
 * configuration is invalid;
 * - if `out_error` is not null, writes a caller-owned `PitSharedString`
 * error handle that MUST be released with `pit_destroy_shared_string`.
 *
 * Ownership:
 * - on success the returned engine pointer is owned by the caller and must
 *   be
 * released with `pit_destroy_engine`;
 * - the builder becomes consumed regardless of success and must not be
 *   reused.
 */
PitEngine * pit_engine_builder_build(
    PitEngineBuilder * builder,
    PitOutError out_error
);

/**
 * Releases an engine pointer owned by the caller.
 *
 * Contract:
 * - passing null is allowed;
 * - destroying the engine also releases any state and policies retained by
 * that engine instance;
 * - this function always succeeds.
 */
void pit_destroy_engine(
    PitEngine * engine
);

/**
 * Starts pre-trade processing and returns a deferred request pointer.
 *
 * This stage validates whether the order can enter the full pre-trade flow.
 *
 * Success:
 * - returns `Passed` when the order passed this stage; read `out_request`;
 * - returns `Rejected` when the order was rejected; read `out_rejects` if
 *   not
 * null.
 *
 * Error:
 * - returns `Error` when input pointers are invalid or the order payload
 * cannot be decoded;
 * - on `Error`, if `out_error` is not null, it is filled with a
 * caller-owned `PitSharedString` that MUST be destroyed by the caller.
 *
 * Cleanup:
 * - release a successful request with
 *   `pit_pretrade_pre_trade_request_execute` or
 * `pit_destroy_pretrade_pre_trade_request`.
 *
 * Reject ownership contract:
 * - on `Rejected`, a non-null `PitRejectList` pointer is written to
 *   `out_rejects`
 * if it is not null;
 * - the caller takes ownership and MUST release it with
 * `pit_destroy_reject_list`; failing to do so leaks the heap allocation made
 * inside this call;
 * - no thread-local state is involved, and the returned pointer is safe to
 * read on any thread;
 * - on `Passed` and `Error`, null is written to `out_rejects`, and the
 *   caller
 * must not call destroy in those cases.
 *
 * Order lifetime contract:
 * - `order` is read as a borrowed view during this call;
 * - the operation snapshots that payload before returning, because the
 * deferred request may outlive the source buffers.
 */
PitPretradeStatus pit_engine_start_pre_trade(
    PitEngine * engine,
    const PitOrder * order,
    PitPretradePreTradeRequest ** out_request,
    PitRejectList ** out_rejects,
    PitOutError out_error
);

/**
 * Runs the complete pre-trade check in one call.
 *
 * Success:
 * - returns `Passed` when the order passed this stage; read
 *   `out_reservation`;
 * - returns `Rejected` when the order was rejected is not null; read
 * `out_rejects`.
 *
 * Error:
 * - returns `Error` when input pointers are invalid or the order payload
 * cannot be decoded;
 * - on `Error`, if `out_error` is not null, it is filled with a
 * caller-owned `PitSharedString` that MUST be destroyed by the caller.
 *
 * Cleanup:
 * - release a successful reservation with
 *   `pit_pretrade_pre_trade_reservation_commit`,
 * `pit_pretrade_pre_trade_reservation_rollback`, or
 * `pit_destroy_pretrade_pre_trade_reservation`.
 *
 * Reject ownership contract:
 * - on `Rejected`, a non-null `PitRejectList` pointer is written to
 * `out_rejects` if it is not null;
 * - the caller takes ownership and MUST release it with
 * `pit_destroy_reject_list`; failing to do so leaks the heap allocation made
 * inside this call;
 * - no thread-local state is involved, and the returned pointer is safe to
 * read on any thread;
 * - on `Passed` and `Error`, null is written to `out_rejects`, and the
 *   caller
 * must not call destroy in those cases.
 *
 * Order lifetime contract:
 * - `order` is read as a borrowed view during this call only;
 * - the operation does not retain any pointer into source memory after this
 * function returns.
 */
PitPretradeStatus pit_engine_execute_pre_trade(
    PitEngine * engine,
    const PitOrder * order,
    PitPretradePreTradeReservation ** out_reservation,
    PitRejectList ** out_rejects,
    PitOutError out_error
);

/**
 * Executes a deferred request returned by `pit_engine_start_pre_trade`.
 *
 * Success:
 * - returns `Passed` when the order passed this stage; read
 *   `out_reservation`;
 * - returns `Rejected` when the order was rejected and `out_rejects` is not
 * null; read `out_rejects`.
 *
 * Error:
 * - returns `Error` when input pointers are invalid or the order payload
 * cannot be decoded;
 * - on `Error`, if `out_error` is not null, it is filled with a
 * caller-owned `PitSharedString` that MUST be destroyed by the caller.
 *
 * Ownership:
 * - this call consumes the request object's content exactly once;
 * - after a successful or failed execute, the object itself may still
 * be released with `pit_destroy_pretrade_pre_trade_request`, but it cannot be
 * executed again.
 *
 * Reject ownership contract:
 * - on `Rejected`, a non-null `PitRejectList` pointer is written to
 * `out_rejects` if it is not null;
 * - the caller takes ownership and MUST release it with
 * `pit_destroy_reject_list`; failing to do so leaks the heap allocation made
 * inside this call;
 * - no thread-local state is involved, and the returned pointer is safe to
 * read on any thread;
 * - on `Passed` and `Error`, null is written to `out_rejects`, and the
 *   caller
 * must not call destroy in those cases.
 */
PitPretradeStatus pit_pretrade_pre_trade_request_execute(
    PitPretradePreTradeRequest * request,
    PitPretradePreTradeReservation ** out_reservation,
    PitRejectList ** out_rejects,
    PitOutError out_error
);

/**
 * Releases a deferred request pointer owned by the caller.
 *
 * Contract:
 * - passing null is allowed;
 * - destroying an unexecuted request abandons it without creating a
 * reservation;
 * - this function always succeeds.
 */
void pit_destroy_pretrade_pre_trade_request(
    PitPretradePreTradeRequest * request
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
void pit_pretrade_pre_trade_reservation_commit(
    PitPretradePreTradeReservation * reservation
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
void pit_pretrade_pre_trade_reservation_rollback(
    PitPretradePreTradeReservation * reservation
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
PitPretradePreTradeLock pit_pretrade_pre_trade_reservation_get_lock(
    const PitPretradePreTradeReservation * reservation
);

/**
 * Releases a reservation pointer owned by the caller.
 *
 * Contract:
 * - passing null is allowed;
 * - destroying an unresolved reservation triggers rollback of any pending
 * mutations;
 * - callers that need explicit resolution should call commit or rollback
 * first;
 * - this function always succeeds.
 */
void pit_destroy_pretrade_pre_trade_reservation(
    PitPretradePreTradeReservation * reservation
);

/**
 * Applies an execution report to engine state.
 *
 * Success:
 * - returns `PitEngineApplyExecutionReportResult { is_error = false, ... }`.
 *
 * Error:
 * - returns `PitEngineApplyExecutionReportResult { is_error = true,
 *   post_trade_result = { kill_switch_triggered = false } }`
 * when input pointers are invalid or the report payload cannot be decoded;
 * - if `out_error` is not null, writes a caller-owned `PitSharedString`
 * error handle that MUST be released with `pit_destroy_shared_string`;
 * - when `is_error` is `true`, do not trust any other fields beyond the fact
 * that the call failed.
 *
 * Lifetime contract:
 * - `report` is read as a borrowed view during this call only;
 * - the operation does not retain any pointer into source memory after this
 * function returns.
 */
PitEngineApplyExecutionReportResult pit_engine_apply_execution_report(
    PitEngine * engine,
    const PitExecutionReport * report,
    PitOutError out_error
);

/**
 * Releases a batch-error object returned by account-adjustment apply.
 *
 * Contract:
 * - passing null is allowed;
 * - this function always succeeds.
 */
void pit_destroy_account_adjustment_batch_error(
    PitAccountAdjustmentBatchError * batch_error
);

/**
 * Returns the failing adjustment index from a batch error.
 *
 * Contract:
 * - `batch_error` must be a valid non-null pointer;
 * - this function never fails;
 * - violating the pointer contract aborts the call.
 */
size_t pit_account_adjustment_batch_error_get_failed_adjustment_index(
    const PitAccountAdjustmentBatchError * batch_error
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
const PitRejectList * pit_account_adjustment_batch_error_get_rejects(
    const PitAccountAdjustmentBatchError * batch_error
);

/**
 * Applies a batch of account adjustments to one account.
 *
 * Success:
 * - returns `PitAccountAdjustmentApplyStatus::Applied` when the batch was
 * accepted and applied;
 * - returns `PitAccountAdjustmentApplyStatus::Rejected` when the call itself
 * completed normally but a policy rejected the batch; read `out_reject`.
 *
 * Error:
 * - returns `PitAccountAdjustmentApplyStatus::Error` when input pointers are
 * invalid or some adjustment payload cannot be decoded;
 * - on `Error`, if `out_error` is not null, it is filled with a
 * caller-owned `PitSharedString` that MUST be destroyed by the caller.
 *
 * Result handling:
 * - `Applied` means there is no reject object to clean up;
 * - `Rejected` stores batch error details in `out_reject`, the caller must
 * release a returned object with `pit_destroy_account_adjustment_batch_error`;
 * - rejects returned by `pit_account_adjustment_batch_error_get_rejects`
 * contain string views borrowed from the batch error and must not be used
 * after the batch error is destroyed;
 * - when `Error` is returned, do not use any pointer from a previous
 * unrelated call as if it belonged to this failure.
 *
 * Lifetime contract:
 * - every `adjustment` entry from the contiguous input array is read as a
 * borrowed view during this call only;
 * - release a returned batch error with
 * `pit_destroy_account_adjustment_batch_error`.
 */
PitAccountAdjustmentApplyStatus pit_engine_apply_account_adjustment(
    PitEngine * engine,
    PitParamAccountId account_id,
    const PitAccountAdjustment * adjustments,
    size_t adjustments_len,
    PitAccountAdjustmentBatchError ** out_reject,
    PitOutError out_error
);

/**
 * Creates a built-in start-stage policy that validates order input shape.
 *
 * Why it exists:
 * - Use it to reject structurally invalid orders before deeper checks run.
 *
 * Success:
 * - returns a new caller-owned pointer.
 * - this function always succeeds.
 *
 * Lifetime contract:
 * - The returned pointer belongs to the caller.
 * - If the pointer is added to the engine builder, the engine keeps its own
 * reference to the same policy object.
 * - The caller must still release its own pointer with
 * `pit_destroy_pretrade_check_pre_trade_start_policy` after the pointer is no
 * longer needed locally.
 */
PitPretradeCheckPreTradeStartPolicy *
pit_create_pretrade_policies_order_validation_policy(
    void
);

/**
 * Creates a built-in start-stage policy that limits how many orders may be
 * accepted within a time window.
 *
 * Arguments:
 * - `max_orders`: maximum number of accepted orders allowed in one window.
 * - `window_seconds`: size of the rolling window in seconds.
 *
 * Success:
 * - returns a new caller-owned pointer.
 * - this function always succeeds.
 *
 * Lifetime contract:
 * - The returned pointer belongs to the caller.
 * - If the pointer is added to the engine builder, the engine keeps its own
 * reference to the same policy object.
 * - The caller must still release its own pointer with
 * `pit_destroy_pretrade_check_pre_trade_start_policy` after the pointer is no
 * longer needed locally.
 */
PitPretradeCheckPreTradeStartPolicy *
pit_create_pretrade_policies_rate_limit_policy(
    size_t max_orders,
    uint64_t window_seconds
);

/**
 * Creates a built-in start-stage policy that rejects new orders when
 * accumulated P&L is outside configured bounds.
 *
 * Why it exists:
 * - Use it as a P&L bounds kill switch per settlement asset.
 *
 * Arguments:
 * - `params`: pointer to an array of barrier definitions.
 * - `params_len`: number of elements in `params`.
 *
 * Contract:
 * - `params` must point to `params_len` readable entries.
 * - `params_len` must be greater than zero.
 * - Each `settlement_asset` pointer inside `params` must be a valid
 * null-terminated string for the duration of the call.
 *
 * Success:
 * - returns a new caller-owned policy object.
 *
 * Error:
 * - returns null when arguments are invalid or the policy cannot be created;
 * - if `out_error` is not null, writes a caller-owned `PitSharedString`
 * error handle that MUST be released with `pit_destroy_shared_string`.
 *
 * Lifetime contract:
 * - The returned pointer belongs to the caller.
 * - If the pointer is added to the engine builder, the engine keeps its own
 * reference to the same policy object.
 * - The caller must still release its own pointer with
 * `pit_destroy_pretrade_check_pre_trade_start_policy` after the pointer is no
 * longer needed locally.
 */
PitPretradeCheckPreTradeStartPolicy *
pit_create_pretrade_policies_pnl_bounds_killswitch_policy(
    const PitPretradePoliciesPnlBoundsBarrier * params,
    size_t params_len,
    PitOutError out_error
);

/**
 * Creates a built-in start-stage policy that rejects orders above configured
 * size limits.
 *
 * Why it exists:
 * - Use it to cap order quantity and notional per settlement asset.
 *
 * Arguments:
 * - `params`: pointer to an array of size-limit definitions.
 * - `params_len`: number of elements in `params`.
 *
 * Contract:
 * - `params` must point to `params_len` readable entries.
 * - `params_len` must be greater than zero.
 * - Each `settlement_asset` pointer inside `params` must be a valid
 * null-terminated string for the duration of the call.
 *
 * Success:
 * - returns a new caller-owned policy object.
 *
 * Error:
 * - returns null when arguments are invalid;
 * - if `out_error` is not null, writes a caller-owned `PitSharedString`
 * error handle that MUST be released with `pit_destroy_shared_string`.
 *
 * Lifetime contract:
 * - The returned pointer belongs to the caller.
 * - If the pointer is added to the engine builder, the engine keeps its own
 * reference to the same policy object.
 * - The caller must still release its own pointer with
 * `pit_destroy_pretrade_check_pre_trade_start_policy` after the pointer is no
 * longer needed locally.
 */
PitPretradeCheckPreTradeStartPolicy *
pit_create_pretrade_policies_order_size_limit_policy(
    const PitPretradePoliciesOrderSizeLimitParam * params,
    size_t params_len,
    PitOutError out_error
);

void pit_destroy_pretrade_check_pre_trade_start_policy(
    PitPretradeCheckPreTradeStartPolicy * policy
);

void pit_destroy_pretrade_pre_trade_policy(
    PitPretradePreTradePolicy * policy
);

void pit_destroy_account_adjustment_policy(
    PitAccountAdjustmentPolicy * policy
);

PitStringView pit_pretrade_check_pre_trade_start_policy_get_name(
    const PitPretradeCheckPreTradeStartPolicy * policy
);

PitStringView pit_pretrade_pre_trade_policy_get_name(
    const PitPretradePreTradePolicy * policy
);

PitStringView pit_account_adjustment_policy_get_name(
    const PitAccountAdjustmentPolicy * policy
);

/**
 * Adds a start-stage policy to the engine builder.
 *
 * Why it exists:
 * - Registers a policy that runs before the main pre-trade stage.
 *
 * Contract:
 * - `builder` must be a valid engine builder pointer.
 * - `policy` must be a valid non-null start-stage policy pointer.
 *
 * Success:
 * - returns `true` and the builder retains its own reference to the policy.
 *
 * Error:
 * - returns `false` when the builder or policy cannot be used;
 * - if `out_error` is not null, writes a caller-owned `PitSharedString`
 * error handle that MUST be released with `pit_destroy_shared_string`.
 *
 * Lifetime contract:
 * - The engine builder retains its own reference to the policy object.
 * - The caller still owns the passed pointer and must release that local
 *   pointer
 * separately with `pit_destroy_pretrade_check_pre_trade_start_policy` when
 * it is no longer needed.
 */
bool pit_engine_builder_add_check_pre_trade_start_policy(
    PitEngineBuilder * builder,
    PitPretradeCheckPreTradeStartPolicy * policy,
    PitOutError out_error
);

/**
 * Adds a main-stage pre-trade policy to the engine builder.
 *
 * Contract:
 * - `builder` must be a valid engine builder pointer.
 * - `policy` must be a valid non-null main-stage policy pointer.
 *
 * Success:
 * - returns `true` and the builder retains its own reference to the policy.
 *
 * Error:
 * - returns `false` when the builder or policy cannot be used;
 * - if `out_error` is not null, writes a caller-owned `PitSharedString`
 * error handle that MUST be released with `pit_destroy_shared_string`.
 *
 * Lifetime contract:
 * - The engine builder retains its own reference to the policy object.
 * - The caller still owns the passed pointer and must release that local
 *   pointer
 * separately with `pit_destroy_pretrade_pre_trade_policy` when it is no
 * longer needed.
 */
bool pit_engine_builder_add_pre_trade_policy(
    PitEngineBuilder * builder,
    PitPretradePreTradePolicy * policy,
    PitOutError out_error
);

/**
 * Adds an account-adjustment policy to the engine builder.
 *
 * Contract:
 * - `builder` must be a valid engine builder pointer.
 * - `policy` must be a valid non-null account-adjustment policy pointer.
 *
 * Success:
 * - returns `true` and the builder retains its own reference to the policy.
 *
 * Error:
 * - returns `false` when the builder or policy cannot be used;
 * - if `out_error` is not null, writes a caller-owned `PitSharedString`
 * error handle that MUST be released with `pit_destroy_shared_string`.
 *
 * Lifetime contract:
 * - The engine builder retains its own reference to the policy object.
 * - The caller still owns the passed pointer and must release that local
 *   pointer
 * separately with `pit_destroy_account_adjustment_policy` when it
 * is no longer needed.
 */
bool pit_engine_builder_add_account_adjustment_policy(
    PitEngineBuilder * builder,
    PitAccountAdjustmentPolicy * policy,
    PitOutError out_error
);

/**
 * Registers one commit/rollback mutation in the provided collector.
 *
 * Contract:
 * - `mutations` must be a valid non-null callback-scoped pointer.
 * - `commit_fn` and `rollback_fn` must remain callable until one of them is
 * executed.
 * - `user_data` is passed to both callbacks.
 * - Exactly one of `commit_fn` or `rollback_fn` runs for each successful
 *   push.
 * - After the executed callback returns, `free_fn` is called exactly once
 *   when
 * provided.
 * - If neither callback runs (for example collector drop), only `free_fn`
 * runs exactly once when provided.
 *
 * Error:
 * - returns `false` when `mutations` is null or invalid;
 * - if `out_error` is not null, writes a caller-owned `PitSharedString`
 * error handle that MUST be released with `pit_destroy_shared_string`.
 */
bool pit_mutations_push(
    PitMutations * mutations,
    PitMutationFn commit_fn,
    PitMutationFn rollback_fn,
    void * user_data,
    PitMutationFreeFn free_fn,
    PitOutError out_error
);

/**
 * Creates a custom start-stage policy from caller-provided callbacks.
 *
 * Why it exists:
 * - Lets the caller implement policy logic outside the engine and plug it
 *   into
 * the same builder flow as built-in policies.
 *
 * Contract:
 * - `name` must point to a valid, null-terminated string for the duration of
 * the call.
 * - `check_fn`, `apply_fn`, and `free_user_data_fn` must remain callable for
 * as long as the policy may still be used by either the caller pointer or
 * the engine.
 * - `free_user_data_fn` will be called exactly once, when the last reference
 * to the policy is released.
 * - `user_data` is stored as-is and passed back to every callback
 *   invocation.
 *
 * Success:
 * - returns a new caller-owned policy object.
 *
 * Error:
 * - returns null when `name` is invalid;
 * - if `out_error` is not null, writes a caller-owned `PitSharedString`
 * error handle that MUST be released with `pit_destroy_shared_string`.
 *
 * Lifetime contract:
 * - The policy stores its own copy of `name`; the caller may release the
 *   input
 * string after this function returns.
 * - The returned pointer is owned by the caller and must be released with
 * `pit_destroy_pretrade_check_pre_trade_start_policy` when no longer needed.
 * - If the policy is added to the engine builder, the engine keeps its own
 * reference, but the caller must still release the caller-owned pointer.
 * - `free_user_data_fn` runs once the last reference to the policy is
 * released; when the engine is the final holder, it runs as part of engine
 * destruction.
 */
PitPretradeCheckPreTradeStartPolicy *
pit_create_pretrade_custom_check_pre_trade_start_policy(
    PitStringView name,
    PitPretradeCheckPreTradeStartPolicyCheckPreTradeStartFn check_fn,
    PitPretradeCheckPreTradeStartPolicyApplyExecutionReportFn apply_execution_report_fn,
    PitPretradeCheckPreTradeStartPolicyFreeUserDataFn free_user_data_fn,
    void * user_data,
    PitOutError out_error
);

/**
 * Creates a custom main-stage pre-trade policy from caller-provided callbacks.
 *
 * Contract:
 * - `name` must point to a valid, null-terminated string for the duration of
 * the call.
 * - `check_fn`, `apply_fn`, and `free_user_data_fn` must
 * remain callable for as long as the policy may still be used by either the
 * caller pointer or the engine.
 * - Custom policy callbacks can register commit/rollback mutations through
 *   the
 * mutations pointer passed to `check_fn`.
 * - `free_user_data_fn` will be called exactly once, when the last reference
 * to the policy is released.
 * - `user_data` is stored as-is and passed back to every callback
 *   invocation.
 *
 * Success:
 * - returns a new caller-owned policy object.
 *
 * Error:
 * - returns null when `name` is invalid;
 * - if `out_error` is not null, writes a caller-owned `PitSharedString`
 * error handle that MUST be released with `pit_destroy_shared_string`.
 *
 * Lifetime contract:
 * - The policy stores its own copy of `name`; the caller may release the
 *   input
 * string after this function returns.
 * - The returned pointer is owned by the caller and must be released with
 * `pit_destroy_pretrade_pre_trade_policy` when no longer needed.
 * - If the policy is added to the engine builder, the engine keeps its own
 * reference, but the caller must still release the caller-owned pointer.
 * - `free_user_data_fn` runs once the last reference to the policy is
 * released; when the engine is the final holder, it runs as part of engine
 * destruction.
 */
PitPretradePreTradePolicy * pit_create_pretrade_custom_pre_trade_policy(
    PitStringView name,
    PitPretradePreTradePolicyCheckFn check_fn,
    PitPretradePreTradePolicyApplyExecutionReportFn apply_fn,
    PitPretradePreTradePolicyFreeUserDataFn free_user_data_fn,
    void * user_data,
    PitOutError out_error
);

/**
 * Creates a custom account-adjustment policy from caller-provided callbacks.
 *
 * Contract:
 * - `name` must point to a valid, null-terminated string for the duration of
 * the call.
 * - `apply_fn` and `free_user_data_fn` must remain callable for as long as
 * the policy may still be used by either the caller pointer or the engine.
 * - Custom policy callbacks can register commit/rollback mutations through
 *   the
 * mutations pointer passed to `apply_fn`.
 * - `free_user_data_fn` will be called exactly once, when the last reference
 * to the policy is released.
 * - `user_data` is stored as-is and passed back to every callback
 *   invocation.
 *
 * Success:
 * - returns a new caller-owned policy object.
 *
 * Error:
 * - returns null when `name` is invalid;
 * - if `out_error` is not null, writes a caller-owned `PitSharedString`
 * error handle that MUST be released with `pit_destroy_shared_string`.
 *
 * Lifetime contract:
 * - The policy stores its own copy of `name`; the caller may release the
 *   input
 * string after this function returns.
 * - The returned pointer is owned by the caller and must be released with
 * `pit_destroy_account_adjustment_policy` when no longer needed.
 * - If the policy is added to the engine builder, the engine keeps its own
 * reference, but the caller must still release the caller-owned pointer.
 * - `free_user_data_fn` runs once the last reference to the policy is
 * released; when the engine is the final holder, it runs as part of engine
 * destruction.
 */
PitAccountAdjustmentPolicy * pit_create_custom_account_adjustment_policy(
    PitStringView name,
    PitAccountAdjustmentPolicyApplyFn apply_fn,
    PitAccountAdjustmentPolicyFreeUserDataFn free_user_data_fn,
    void * user_data,
    PitOutError out_error
);

/**
 * Returns the Pit runtime version string.
 *
 * This function never fails.
 *
 * The returned view is read-only, never null, and remains valid for the
 * entire process lifetime. The caller must not release it.
 */
PitStringView pit_get_runtime_version(void);

/**
 * Releases a `PitSharedString` handle.
 *
 * Null input is a no-op.
 *
 * After this call, the handle and any `PitStringView` previously obtained
 * from it are invalid and must not be used.
 */
void pit_destroy_shared_string(
    PitSharedString * handle
);

/**
 * Borrows a read-only view of the bytes stored in the handle.
 *
 * Returns an unset view (`ptr == null`, `len == 0`) when `handle` is null.
 *
 * The returned view is valid only while `handle` remains alive. The caller
 * must copy the bytes if they must outlive the handle.
 */
PitStringView pit_shared_string_view(
    const PitSharedString * handle
);

#ifdef __cplusplus
}
#endif

#endif
