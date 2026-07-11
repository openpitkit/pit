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

/**
 * A decimal value crossing the engine boundary.
 *
 * RECOMMENDED: pass a decimal `string` (for example `"100.50"` or
 * `"0.00847000"`). Strings are lossless and the only safe form for full- or
 * variable-scale instruments.
 *
 * `bigint` is safe for exact integer values.
 *
 * WARNING: `number` is an IEEE-754 double - values such as `0.1`, `0.2`, or
 * magnitudes above `Number.MAX_SAFE_INTEGER` (9007199254740991) lose precision.
 * Prefer `string`. It is accepted only as a convenience for small, exact
 * integers.
 */
export type DecimalInput = string | number | bigint;

/**
 * Rounding strategy accepted by the `*Rounded` value-type factories.
 *
 * The four canonical core strategies are exposed alongside four ergonomic
 * aliases: `default`/`banker` map to midpoint nearest-even, while
 * `conservativeProfit`/`conservativeLoss` map to round-down.
 */
export type RoundingStrategy =
  | "midpointNearestEven"
  | "midpointAwayFromZero"
  | "up"
  | "down"
  | "default"
  | "banker"
  | "conservativeProfit"
  | "conservativeLoss";

/**
 * Named rounding strategies accepted by the `*Rounded` value-type factories.
 *
 * Each value is a {@link RoundingStrategy} wire string, so the object provides
 * autocomplete without forcing callers off the plain string form; the literals
 * remain assignable.
 */
export const RoundingStrategies = {
  /** Round midpoint values toward the nearest even last digit. */
  MidpointNearestEven: "midpointNearestEven",
  /** Round midpoint values away from zero. */
  MidpointAwayFromZero: "midpointAwayFromZero",
  /** Round toward positive infinity (ceiling). */
  Up: "up",
  /** Round toward negative infinity (floor). */
  Down: "down",
  /** Midpoint nearest-even (banker's rounding). */
  Default: "default",
  /** Alias of {@link RoundingStrategies.Default}. */
  Banker: "banker",
  /** Round toward negative infinity, conservative for profit. */
  ConservativeProfit: "conservativeProfit",
  /** Round toward negative infinity, conservative for loss. */
  ConservativeLoss: "conservativeLoss",
} as const;

/**
 * Type of execution-report fill event. Mirrors the core `FillType` enum and
 * its stable wire values.
 */
export const FillType = {
  /** Normal trade execution. */
  Trade: "TRADE",
  /** Forced liquidation by the venue. */
  Liquidation: "LIQUIDATION",
  /** Auto-deleveraging event. */
  AutoDeleverage: "AUTO_DELEVERAGE",
  /** Settlement at expiry or delivery. */
  Settlement: "SETTLEMENT",
  /** Funding payment. */
  Funding: "FUNDING",
} as const;

/** Stable execution-report fill-type wire value. */
export type FillType = (typeof FillType)[keyof typeof FillType];

/**
 * Parameter type attached to numeric validation and arithmetic errors.
 * Mirrors the core `ParamKind` discriminant.
 */
export const ParamKind = {
  /** Quantity parameter. */
  Quantity: "Quantity",
  /** Traded volume parameter. */
  Volume: "Volume",
  /** Order notional parameter. */
  Notional: "Notional",
  /** Price parameter. */
  Price: "Price",
  /** Profit-and-loss parameter. */
  Pnl: "Pnl",
  /** Cash-flow parameter. */
  CashFlow: "CashFlow",
  /** Signed position-size parameter. */
  PositionSize: "PositionSize",
  /** Fee parameter. */
  Fee: "Fee",
  /** Leverage parameter. */
  Leverage: "Leverage",
} as const;

/** Stable parameter-kind wire value carried by validation errors. */
export type ParamKind = (typeof ParamKind)[keyof typeof ParamKind];

/**
 * Scope of a business reject returned by a policy: `"order"` applies to the
 * current order only, while `"account"` applies at account scope. The string
 * literals stay assignable.
 */
export const RejectScope = {
  /** Reject that applies only to the current order. */
  Order: "order",
  /** Reject that applies at account scope. */
  Account: "account",
} as const;

/** Scope of a business reject (`"order"` or `"account"`). */
export type RejectScope = (typeof RejectScope)[keyof typeof RejectScope];

/**
 * Quote field a spot-funds policy uses to price market orders. These are the
 * stable wire values accepted by the JavaScript policy builder.
 */
export const SpotFundsPricingSource = {
  /** Use the mark price. */
  Mark: "Mark",
  /** Use the top of the book. */
  BookTop: "BookTop",
} as const;

/** Spot-funds market-order pricing source (`"Mark"` or `"BookTop"`). */
export type SpotFundsPricingSource =
  (typeof SpotFundsPricingSource)[keyof typeof SpotFundsPricingSource];

/**
 * Spot-funds insufficient-funds limit mode. The string literals stay
 * assignable.
 */
export const SpotFundsLimitMode = {
  /** Enforce available-funds checks and reject insufficient funds. */
  Enforce: "Enforce",
  /** Track funds and holds, but do not reject insufficient funds. */
  TrackOnly: "TrackOnly",
} as const;

/** Spot-funds insufficient-funds limit mode. */
export type SpotFundsLimitMode =
  (typeof SpotFundsLimitMode)[keyof typeof SpotFundsLimitMode];

/**
 * Stable machine-readable reject codes used across built-in and custom
 * policies. Each value is the wire string carried on a `Reject`; the object
 * keeps string literals assignable so code that hard-codes a code continues to
 * type-check.
 */
export const RejectCode = {
  /** A required field is absent. */
  MissingRequiredField: "MissingRequiredField",
  /** A field exists but has an invalid format. */
  InvalidFieldFormat: "InvalidFieldFormat",
  /** A field exists but its value is not allowed. */
  InvalidFieldValue: "InvalidFieldValue",
  /** The order type is not supported by the current policy set. */
  UnsupportedOrderType: "UnsupportedOrderType",
  /** The time-in-force value is not supported. */
  UnsupportedTimeInForce: "UnsupportedTimeInForce",
  /** The request contains an unsupported order attribute. */
  UnsupportedOrderAttribute: "UnsupportedOrderAttribute",
  /** The client order ID is already in use. */
  DuplicateClientOrderId: "DuplicateClientOrderId",
  /** The request arrived after the allowed entry window. */
  TooLateToEnter: "TooLateToEnter",
  /** The venue or market is closed for the requested action. */
  ExchangeClosed: "ExchangeClosed",
  /** The instrument is not recognized. */
  UnknownInstrument: "UnknownInstrument",
  /** The account is not recognized. */
  UnknownAccount: "UnknownAccount",
  /** The venue is not recognized. */
  UnknownVenue: "UnknownVenue",
  /** The clearing account is not recognized. */
  UnknownClearingAccount: "UnknownClearingAccount",
  /** The collateral asset is not recognized. */
  UnknownCollateralAsset: "UnknownCollateralAsset",
  /** Available cash is not enough. */
  InsufficientFunds: "InsufficientFunds",
  /** Available margin is not enough. */
  InsufficientMargin: "InsufficientMargin",
  /** Available position is not enough. */
  InsufficientPosition: "InsufficientPosition",
  /** A credit limit would be exceeded. */
  CreditLimitExceeded: "CreditLimitExceeded",
  /** A general risk limit would be exceeded. */
  RiskLimitExceeded: "RiskLimitExceeded",
  /** More than one order-size limit would be exceeded. */
  OrderExceedsLimit: "OrderExceedsLimit",
  /** The requested quantity exceeds its limit. */
  OrderQtyExceedsLimit: "OrderQtyExceedsLimit",
  /** The requested notional exceeds its limit. */
  OrderNotionalExceedsLimit: "OrderNotionalExceedsLimit",
  /** A position limit would be exceeded. */
  PositionLimitExceeded: "PositionLimitExceeded",
  /** A concentration limit would be exceeded. */
  ConcentrationLimitExceeded: "ConcentrationLimitExceeded",
  /** A leverage limit would be exceeded. */
  LeverageLimitExceeded: "LeverageLimitExceeded",
  /** Too many requests were submitted inside the configured window. */
  RateLimitExceeded: "RateLimitExceeded",
  /** The configured PnL kill switch is active. */
  PnlKillSwitchTriggered: "PnlKillSwitchTriggered",
  /** The account is blocked from new requests. */
  AccountBlocked: "AccountBlocked",
  /** The account is not authorized for the requested action. */
  AccountNotAuthorized: "AccountNotAuthorized",
  /** A compliance rule blocks the request. */
  ComplianceRestriction: "ComplianceRestriction",
  /** The instrument is restricted. */
  InstrumentRestricted: "InstrumentRestricted",
  /** The request is blocked by jurisdiction rules. */
  JurisdictionRestriction: "JurisdictionRestriction",
  /** The request would violate wash-trade prevention. */
  WashTradePrevention: "WashTradePrevention",
  /** The request would self-match. */
  SelfMatchPrevention: "SelfMatchPrevention",
  /** The request violates a short-sale restriction. */
  ShortSaleRestriction: "ShortSaleRestriction",
  /** Required risk configuration is missing. */
  RiskConfigurationMissing: "RiskConfigurationMissing",
  /** Required reference data is unavailable. */
  ReferenceDataUnavailable: "ReferenceDataUnavailable",
  /** The requested order value could not be computed safely. */
  OrderValueCalculationFailed: "OrderValueCalculationFailed",
  /** The system cannot process the request right now. */
  SystemUnavailable: "SystemUnavailable",
  /** A mark price required for order evaluation is unavailable. */
  MarkPriceUnavailable: "MarkPriceUnavailable",
  /** An account adjustment would violate its configured bounds. */
  AccountAdjustmentBoundsExceeded: "AccountAdjustmentBoundsExceeded",
  /** Underlying decimal arithmetic overflowed during evaluation. */
  ArithmeticOverflow: "ArithmeticOverflow",
  /** Custom reject code; meaning depends on the policy implementation. */
  Custom: "Custom",
  /** A standard code does not describe the case precisely enough. */
  Other: "Other",
} as const;

/** A stable machine-readable reject code carried on a `Reject`. */
export type RejectCode = (typeof RejectCode)[keyof typeof RejectCode];

/** Order / position side wire value. */
export type SideValue = "BUY" | "SELL";

/** Position side wire value. */
export type PositionSideValue = "LONG" | "SHORT";

/** Position effect wire value. */
export type PositionEffectValue = "OPEN" | "CLOSE";

/** Position bookkeeping mode wire value (lowercase). */
export type PositionModeValue = "netting" | "hedged";
