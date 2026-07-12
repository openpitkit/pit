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
 * Typed error hierarchy for failures crossing the wasm boundary.
 *
 * Invalid values use {@link RangeError} subclasses and malformed JS shapes use
 * the native {@link TypeError}; lifecycle and engine-state failures use named
 * {@link OpenpitError} subclasses. All errors constructed by this module are
 * branded, so `err instanceof OpenpitError` remains a convenient catch-all even
 * for the native built-in branches.
 */

import type {
  AccountAdjustmentBatchResult,
  AccountGroupId,
  AccountId,
  Instrument,
  InstrumentId,
  PostTradeResult,
  Quote,
} from "./wasm/openpit_js.js";
import type { ParamKind } from "./types.js";

/** Stable machine-readable numeric/value validation code. */
export const ErrorCode = {
  /** No more-specific code applies. */
  Unspecified: "Unspecified",
  /** Negative value supplied to a non-negative parameter. */
  Negative: "Negative",
  /** Division by zero. */
  DivisionByZero: "DivisionByZero",
  /** Arithmetic overflow. */
  Overflow: "Overflow",
  /** Arithmetic underflow. */
  Underflow: "Underflow",
  /** NaN or infinite floating-point input. */
  InvalidFloat: "InvalidFloat",
  /** Text that is not a valid decimal. */
  InvalidFormat: "InvalidFormat",
  /** Invalid price value. */
  InvalidPrice: "InvalidPrice",
  /** Invalid leverage value. */
  InvalidLeverage: "InvalidLeverage",
  /** Empty asset identifier. */
  AssetEmpty: "AssetEmpty",
  /** Empty account identifier. */
  AccountIdEmpty: "AccountIdEmpty",
  /** Validation failure outside the specific cases above. */
  Other: "Other",
} as const;

/** Stable machine-readable numeric/value validation code. */
export type ErrorCode = (typeof ErrorCode)[keyof typeof ErrorCode];

/** Explicit-id market-data registration conflict. */
export const RegistrationErrorKind = {
  /** Requested instrument id is already occupied. */
  DuplicateId: "DuplicateId",
  /** Requested instrument is already registered under another id. */
  DuplicateInstrument: "DuplicateInstrument",
} as const;

/** Explicit-id market-data registration conflict. */
export type RegistrationErrorKind =
  (typeof RegistrationErrorKind)[keyof typeof RegistrationErrorKind];

/** Reference-book registration conflict. */
export const ReferenceBookRegistrationErrorKind = {
  /** Requested instrument id is already occupied. */
  DuplicateId: "DuplicateId",
  /** Requested instrument is already registered under another id. */
  DuplicateInstrument: "DuplicateInstrument",
  /** A newer runtime reported a conflict unknown to this binding. */
  Unknown: "Unknown",
} as const;

/** Reference-book registration conflict. */
export type ReferenceBookRegistrationErrorKind =
  (typeof ReferenceBookRegistrationErrorKind)[keyof typeof ReferenceBookRegistrationErrorKind];

/** Account-group registration failure. */
export const AccountGroupRegistrationErrorKind = {
  /** The reserved default group cannot be explicitly registered. */
  ReservedGroup: "ReservedGroup",
  /** Account already belongs to a group. */
  AlreadyRegistered: "AlreadyRegistered",
  /** Account does not belong to the requested group. */
  NotInGroup: "NotInGroup",
} as const;

/** Account-group registration failure. */
export type AccountGroupRegistrationErrorKind =
  (typeof AccountGroupRegistrationErrorKind)[keyof typeof AccountGroupRegistrationErrorKind];

/** Account/group block administration failure. */
export const AccountBlockErrorKind = {
  /** The reserved default group cannot be blocked. */
  ReservedGroup: "ReservedGroup",
  /** Account block-reason replacement targeted an unblocked account. */
  AccountNotBlocked: "AccountNotBlocked",
  /** Group block-reason replacement targeted an unblocked group. */
  GroupNotBlocked: "GroupNotBlocked",
} as const;

/** Account/group block administration failure. */
export type AccountBlockErrorKind =
  (typeof AccountBlockErrorKind)[keyof typeof AccountBlockErrorKind];

/** Engine-construction failure. */
export const EngineBuildErrorKind = {
  /** No pre-trade policy was registered. */
  NoPolicies: "NoPolicies",
  /** Two registered policies use the same name. */
  DuplicatePolicyName: "DuplicatePolicyName",
  /** Two registered policies use the same non-default group id. */
  DuplicatePolicyGroupId: "DuplicatePolicyGroupId",
  /** A built-in policy configuration is invalid. */
  InvalidConfiguration: "InvalidConfiguration",
} as const;

/** Engine-construction failure. */
export type EngineBuildErrorKind =
  (typeof EngineBuildErrorKind)[keyof typeof EngineBuildErrorKind];

/** Runtime policy-configuration failure. */
export const ConfigureErrorKind = {
  /** No configurable policy has the requested name. */
  Unknown: "UNKNOWN",
  /** The policy exists but has another settings type. */
  TypeMismatch: "TYPE_MISMATCH",
  /** The policy rejected the proposed settings. */
  Validation: "VALIDATION",
  /** Configuration was re-entered from a configuration callback. */
  NestedConfiguration: "NESTED_CONFIGURATION",
} as const;

/** Runtime policy-configuration failure. */
export type ConfigureErrorKind =
  (typeof ConfigureErrorKind)[keyof typeof ConfigureErrorKind];

/** Completed reconciliation result attached to a policy callback failure. */
export type PolicyCallbackResult =
  | PostTradeResult
  | AccountAdjustmentBatchResult;

/** Structured payload accepted by the wasm error factory. */
export interface ErrorPayload {
  kind?: string;
  param?: ParamKind;
  input?: string;
  instrument?: Instrument;
  instrumentId?: InstrumentId;
  accountId?: AccountId;
  currentGroupId?: AccountGroupId;
  requestedGroupId?: AccountGroupId;
  accountGroupId?: AccountGroupId;
  name?: string;
  policyGroupId?: number;
  expected?: string;
  found?: string;
  validationMessage?: string;
  result?: PolicyCallbackResult;
}

/** Constructor options common to named OpenPit errors. */
export interface OpenpitErrorOptions {
  code?: string | undefined;
  cause?: unknown;
}

const openpitErrors = new WeakSet<object>();

function markOpenpitError<T extends Error>(error: T): T {
  openpitErrors.add(error);
  return error;
}

function errorOptions(cause: unknown): ErrorOptions | undefined {
  return cause === undefined ? undefined : { cause };
}

/** Base catch-all for every typed error constructed by the OpenPit boundary. */
export class OpenpitError extends Error {
  /** Optional stable code for generic/future errors. */
  readonly code?: string;

  static override [Symbol.hasInstance](value: unknown): boolean {
    if (this === OpenpitError) {
      return (
        (typeof value === "object" || typeof value === "function") &&
        value !== null &&
        openpitErrors.has(value)
      );
    }
    return Function.prototype[Symbol.hasInstance].call(this, value);
  }

  constructor(message: string, options?: OpenpitErrorOptions) {
    super(message, errorOptions(options?.cause));
    this.name = "OpenpitError";
    if (options?.code !== undefined) {
      this.code = options.code;
    }
    Object.setPrototypeOf(this, new.target.prototype);
    markOpenpitError(this);
  }
}

/** Constructor options for value/range validation errors. */
export interface ValueErrorOptions {
  code?: ErrorCode | undefined;
  param?: ParamKind | undefined;
  input?: string | undefined;
  cause?: unknown;
}

/** Base native RangeError branch for OpenPit value-validation failures. */
export abstract class OpenpitValueError extends RangeError {
  readonly code?: ErrorCode;
  readonly param?: ParamKind;
  readonly input?: string;

  protected constructor(message: string, options?: ValueErrorOptions) {
    super(message, errorOptions(options?.cause));
    if (options?.code !== undefined) {
      this.code = options.code;
    }
    if (options?.param !== undefined) {
      this.param = options.param;
    }
    if (options?.input !== undefined) {
      this.input = options.input;
    }
    Object.setPrototypeOf(this, new.target.prototype);
    markOpenpitError(this);
  }
}

/** Numeric validation or arithmetic failure. */
export class ParamError extends OpenpitValueError {
  constructor(message: string, options?: ValueErrorOptions) {
    super(message, options);
    this.name = "ParamError";
  }
}

/** Empty or invalid asset identifier. */
export class AssetError extends OpenpitValueError {
  constructor(message: string, options?: ValueErrorOptions) {
    super(message, options);
    this.name = "AssetError";
  }
}

/** Empty or invalid account identifier. */
export class AccountIdError extends OpenpitValueError {
  constructor(message: string, options?: ValueErrorOptions) {
    super(message, options);
    this.name = "AccountIdError";
  }
}

/** Base market-data read failure. */
export class MarketDataError extends OpenpitError {
  constructor(message: string, options?: OpenpitErrorOptions) {
    super(message, options);
    this.name = "MarketDataError";
  }
}

/** Market-data read against an unregistered instrument. */
export class UnknownInstrument extends MarketDataError {
  constructor(message: string, options?: OpenpitErrorOptions) {
    super(message, options);
    this.name = "UnknownInstrument";
  }
}

/** Market-data read for which no quote is available. */
export class QuoteUnavailable extends MarketDataError {
  constructor(message: string, options?: OpenpitErrorOptions) {
    super(message, options);
    this.name = "QuoteUnavailable";
  }
}

/** Market-data read whose selected quote aged past its TTL. */
export class QuoteExpired extends MarketDataError {
  readonly quote: Quote;

  constructor(message: string, quote: Quote, options?: { cause?: unknown }) {
    super(message, options);
    this.name = "QuoteExpired";
    this.quote = quote;
  }
}

/** Registration of an instrument that is already registered. */
export class AlreadyRegistered extends OpenpitError {
  readonly instrument: Instrument;

  constructor(
    message: string,
    options: { instrument: Instrument; cause?: unknown },
  ) {
    super(message, options);
    this.name = "AlreadyRegistered";
    this.instrument = options.instrument;
  }
}

/** Explicit-id market-data registration conflict. */
export class RegistrationError extends OpenpitError {
  readonly kind: RegistrationErrorKind;
  readonly instrumentId?: InstrumentId;
  readonly instrument?: Instrument;

  constructor(
    message: string,
    options: {
      kind: RegistrationErrorKind;
      instrumentId?: InstrumentId | undefined;
      instrument?: Instrument | undefined;
      cause?: unknown;
    },
  ) {
    super(message, options);
    this.name = "RegistrationError";
    this.kind = options.kind;
    if (options.instrumentId !== undefined) {
      this.instrumentId = options.instrumentId;
    }
    if (options.instrument !== undefined) {
      this.instrument = options.instrument;
    }
  }
}

/** Operation referencing an instrument id that is not registered. */
export class UnknownInstrumentId extends OpenpitError {
  readonly instrumentId: InstrumentId;

  constructor(
    message: string,
    options: { instrumentId: InstrumentId; cause?: unknown },
  ) {
    super(message, options);
    this.name = "UnknownInstrumentId";
    this.instrumentId = options.instrumentId;
  }
}

/** Reference-book registration conflict. */
export class ReferenceBookRegistrationError extends OpenpitError {
  readonly kind: ReferenceBookRegistrationErrorKind;
  readonly instrumentId?: InstrumentId;
  readonly instrument?: Instrument;

  constructor(
    message: string,
    options: {
      kind: ReferenceBookRegistrationErrorKind;
      instrumentId?: InstrumentId | undefined;
      instrument?: Instrument | undefined;
      cause?: unknown;
    },
  ) {
    super(message, options);
    this.name = "ReferenceBookRegistrationError";
    this.kind = options.kind;
    if (options.instrumentId !== undefined) {
      this.instrumentId = options.instrumentId;
    }
    if (options.instrument !== undefined) {
      this.instrument = options.instrument;
    }
  }
}

/** Reference-book operation targeting an unregistered instrument id. */
export class UnknownReferenceBookInstrumentId extends OpenpitError {
  readonly instrumentId: InstrumentId;

  constructor(
    message: string,
    options: { instrumentId: InstrumentId; cause?: unknown },
  ) {
    super(message, options);
    this.name = "UnknownReferenceBookInstrumentId";
    this.instrumentId = options.instrumentId;
  }
}

/** Account-group register/unregister conflict. */
export class AccountGroupRegistrationError extends OpenpitError {
  readonly kind: AccountGroupRegistrationErrorKind;
  readonly accountId?: AccountId;
  readonly currentGroupId?: AccountGroupId;
  readonly requestedGroupId?: AccountGroupId;

  constructor(
    message: string,
    options: {
      kind: AccountGroupRegistrationErrorKind;
      accountId?: AccountId | undefined;
      currentGroupId?: AccountGroupId | undefined;
      requestedGroupId?: AccountGroupId | undefined;
      cause?: unknown;
    },
  ) {
    super(message, options);
    this.name = "AccountGroupRegistrationError";
    this.kind = options.kind;
    if (options.accountId !== undefined) {
      this.accountId = options.accountId;
    }
    if (options.currentGroupId !== undefined) {
      this.currentGroupId = options.currentGroupId;
    }
    if (options.requestedGroupId !== undefined) {
      this.requestedGroupId = options.requestedGroupId;
    }
  }
}

/** Account/group block administration failure. */
export class AccountBlockError extends OpenpitError {
  readonly kind: AccountBlockErrorKind;
  readonly accountId?: AccountId;
  readonly accountGroupId?: AccountGroupId;

  constructor(
    message: string,
    options: {
      kind: AccountBlockErrorKind;
      accountId?: AccountId | undefined;
      accountGroupId?: AccountGroupId | undefined;
      cause?: unknown;
    },
  ) {
    super(message, options);
    this.name = "AccountBlockError";
    this.kind = options.kind;
    if (options.accountId !== undefined) {
      this.accountId = options.accountId;
    }
    if (options.accountGroupId !== undefined) {
      this.accountGroupId = options.accountGroupId;
    }
  }
}

/** Misuse of a single-use lifecycle handle. */
export class LifecycleError extends OpenpitError {
  constructor(message: string, options?: { cause?: unknown }) {
    super(message, options);
    this.name = "LifecycleError";
  }
}

/** Engine construction failure. */
export class EngineBuildError extends OpenpitError {
  readonly kind: EngineBuildErrorKind;
  readonly policyName?: string;
  readonly policyGroupId?: number;

  constructor(
    message: string,
    options: {
      kind: EngineBuildErrorKind;
      name?: string | undefined;
      policyGroupId?: number | undefined;
      cause?: unknown;
    },
  ) {
    super(message, options);
    this.name = "EngineBuildError";
    this.kind = options.kind;
    if (options.name !== undefined) {
      this.policyName = options.name;
    }
    if (options.policyGroupId !== undefined) {
      this.policyGroupId = options.policyGroupId;
    }
  }
}

/** Runtime policy-reconfiguration failure. */
export class PolicyConfigureError extends OpenpitError {
  readonly kind: ConfigureErrorKind;
  readonly policyName?: string;
  readonly expected?: string;
  readonly found?: string;
  readonly validationMessage?: string;

  constructor(
    message: string,
    options: {
      kind: ConfigureErrorKind;
      name?: string | undefined;
      expected?: string | undefined;
      found?: string | undefined;
      validationMessage?: string | undefined;
      cause?: unknown;
    },
  ) {
    super(message, options);
    this.name = "PolicyConfigureError";
    this.kind = options.kind;
    if (options.name !== undefined) {
      this.policyName = options.name;
    }
    if (options.expected !== undefined) {
      this.expected = options.expected;
    }
    if (options.found !== undefined) {
      this.found = options.found;
    }
    if (options.validationMessage !== undefined) {
      this.validationMessage = options.validationMessage;
    }
  }
}

/**
 * A JavaScript policy callback failed after the engine reconciled all callbacks
 * that could still run. `cause` is the original thrown value; `result` carries
 * the completed post-trade/account-adjustment result when that operation has
 * one, and is undefined for pre-trade or mutation callbacks.
 */
export class PolicyCallbackError extends OpenpitError {
  override readonly cause: unknown;
  readonly result: PolicyCallbackResult | undefined;

  constructor(
    message: string,
    result: PolicyCallbackResult | undefined,
    options: { cause: unknown },
  ) {
    super(message, options);
    this.name = "PolicyCallbackError";
    this.cause = options.cause;
    this.result = result;
  }
}

/** Constructs a {@link QuoteExpired} for the wasm boundary. */
export function makeQuoteExpiredError(
  message: string,
  quote: Quote,
  cause?: unknown,
): QuoteExpired {
  return new QuoteExpired(message, quote, { cause });
}

/**
 * Constructs a concrete error for the stable wasm-boundary discriminator.
 * Unknown names degrade to a branded {@link OpenpitError} while retaining the
 * original name for forward-compatible branching.
 */
export function makeError(
  name: string,
  message: string,
  code?: string,
  payload?: ErrorPayload,
  cause?: unknown,
): OpenpitError {
  switch (name) {
    case "TypeError":
      return markOpenpitError(new TypeError(message, errorOptions(cause)));
    case "RangeError":
      return markOpenpitError(new RangeError(message, errorOptions(cause)));
    case "ParamError":
      return new ParamError(message, {
        code: code as ErrorCode,
        param: payload?.param,
        input: payload?.input,
        cause,
      });
    case "AssetError":
      return new AssetError(message, {
        code: code as ErrorCode,
        param: payload?.param,
        input: payload?.input,
        cause,
      });
    case "AccountIdError":
      return new AccountIdError(message, {
        code: code as ErrorCode,
        param: payload?.param,
        input: payload?.input,
        cause,
      });
    case "MarketDataError":
      return new MarketDataError(message, { code, cause });
    case "UnknownInstrument":
      return new UnknownInstrument(message, { code, cause });
    case "QuoteUnavailable":
      return new QuoteUnavailable(message, { code, cause });
    case "AlreadyRegistered":
      return new AlreadyRegistered(message, {
        instrument: payload?.instrument as Instrument,
        cause,
      });
    case "RegistrationError":
      return new RegistrationError(message, {
        kind: (payload?.kind ?? code) as RegistrationErrorKind,
        instrumentId: payload?.instrumentId,
        instrument: payload?.instrument,
        cause,
      });
    case "UnknownInstrumentId":
      return new UnknownInstrumentId(message, {
        instrumentId: payload?.instrumentId as InstrumentId,
        cause,
      });
    case "ReferenceBookRegistrationError":
      return new ReferenceBookRegistrationError(message, {
        kind: (payload?.kind ?? code) as ReferenceBookRegistrationErrorKind,
        instrumentId: payload?.instrumentId,
        instrument: payload?.instrument,
        cause,
      });
    case "UnknownReferenceBookInstrumentId":
      return new UnknownReferenceBookInstrumentId(message, {
        instrumentId: payload?.instrumentId as InstrumentId,
        cause,
      });
    case "AccountGroupRegistrationError":
      return new AccountGroupRegistrationError(message, {
        kind: (payload?.kind ?? code) as AccountGroupRegistrationErrorKind,
        accountId: payload?.accountId,
        currentGroupId: payload?.currentGroupId,
        requestedGroupId: payload?.requestedGroupId,
        cause,
      });
    case "AccountBlockError":
      return new AccountBlockError(message, {
        kind: (payload?.kind ?? code) as AccountBlockErrorKind,
        accountId: payload?.accountId,
        accountGroupId: payload?.accountGroupId,
        cause,
      });
    case "LifecycleError":
      return new LifecycleError(message, { cause });
    case "EngineBuildError":
      return new EngineBuildError(message, {
        kind: (payload?.kind ?? code) as EngineBuildErrorKind,
        name: payload?.name,
        policyGroupId: payload?.policyGroupId,
        cause,
      });
    case "PolicyConfigureError":
      return new PolicyConfigureError(message, {
        kind: (payload?.kind ?? code) as ConfigureErrorKind,
        name: payload?.name,
        expected: payload?.expected,
        found: payload?.found,
        validationMessage: payload?.validationMessage,
        cause,
      });
    case "PolicyCallbackError":
      return new PolicyCallbackError(message, payload?.result, { cause });
    default: {
      const error = new OpenpitError(message, { code, cause });
      error.name = name;
      return error;
    }
  }
}
