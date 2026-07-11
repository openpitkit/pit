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
//
// Proves the typed error hierarchy crosses the wasm boundary as REAL class
// instances: `instanceof` holds for the base `OpenpitError` and the concrete
// subclass, the `name` matches, and the `code` / `kind` discriminants carry the
// right vocabulary. Runtime tests use the built Node entry; package.smoke.test
// separately bundles and executes both browser export conditions.

import { describe, expect, it } from "vitest";

import {
  Engine,
  OpenpitError,
  OpenpitValueError,
  ParamError,
  AccountIdError,
  AccountBlockError,
  AccountGroupRegistrationError,
  AccountGroupRegistrationErrorKind,
  ConfigureErrorKind,
  EngineBuildError,
  EngineBuildErrorKind,
  LifecycleError,
} from "@openpit/engine";
import { AccountGroupId, AccountId, Price } from "@openpit/engine/param";
import { buildOrderValidation } from "@openpit/engine/pretrade/policies";
import {
  makeError,
  OpenpitError as InternalOpenpitError,
  PolicyCallbackError as InternalPolicyCallbackError,
  PolicyConfigureError as InternalPolicyConfigureError,
} from "../src-ts/errors.js";
// The reject subpath re-exports the same AccountBlockError class identity.
import { AccountBlockError as RejectAccountBlockError } from "@openpit/engine/reject";

const ACCOUNT = 99224416;

// A minimal validation engine, used wherever a real account registry handle is
// needed to drive the admin-block failures.
function makeEngine(): Engine {
  return Engine.builder().builtin(buildOrderValidation()).build();
}

describe("typed error hierarchy", () => {
  it("ParamError is a real OpenpitError with an ErrorCode", () => {
    let caught: unknown;
    try {
      Price.fromString("not-a-number");
    } catch (err) {
      caught = err;
    }
    expect(caught).toBeInstanceOf(Error);
    expect(caught).toBeInstanceOf(OpenpitError);
    expect(caught).toBeInstanceOf(OpenpitValueError);
    expect(caught).toBeInstanceOf(ParamError);
    expect(caught).toBeInstanceOf(RangeError);
    expect((caught as ParamError).name).toBe("ParamError");
    // The code is an ErrorCode (here the decimal-parse failure), never a
    // RejectCode like "InvalidFieldValue".
    expect((caught as ParamError).code).toBe("InvalidFormat");
    expect((caught as ParamError).param).toBe("Price");
    expect((caught as ParamError).input).toBe("not-a-number");
  });

  it("AccountIdError carries the AccountIdEmpty code", () => {
    let caught: unknown;
    try {
      AccountId.fromString("");
    } catch (err) {
      caught = err;
    }
    expect(caught).toBeInstanceOf(OpenpitError);
    expect(caught).toBeInstanceOf(AccountIdError);
    expect(caught).toBeInstanceOf(RangeError);
    expect((caught as AccountIdError).name).toBe("AccountIdError");
    expect((caught as AccountIdError).code).toBe("AccountIdEmpty");
  });

  it("EngineBuildError fires on a duplicate policy name", () => {
    let caught: unknown;
    try {
      // Two OrderValidation built-ins share the default policy name. The first
      // builtin() advances to the ready builder; the second registers in place.
      const ready = Engine.builder().builtin(buildOrderValidation());
      ready.builtin(buildOrderValidation());
      ready.build();
    } catch (err) {
      caught = err;
    }
    expect(caught).toBeInstanceOf(OpenpitError);
    expect(caught).toBeInstanceOf(EngineBuildError);
    expect((caught as EngineBuildError).name).toBe("EngineBuildError");
    expect((caught as EngineBuildError).kind).toBe(
      EngineBuildErrorKind.DuplicatePolicyName,
    );
    expect((caught as EngineBuildError).policyName).toBe(
      "OrderValidationPolicy",
    );
  });

  it("LifecycleError fires on reusing a consumed builder", () => {
    const builder = Engine.builder().builtin(buildOrderValidation());
    builder.build();
    let caught: unknown;
    try {
      // The ready builder was consumed by build(); a second build() is misuse.
      builder.build();
    } catch (err) {
      caught = err;
    }
    expect(caught).toBeInstanceOf(OpenpitError);
    expect(caught).toBeInstanceOf(LifecycleError);
    expect((caught as LifecycleError).name).toBe("LifecycleError");
  });

  it("AccountBlockError carries the AccountNotBlocked kind", () => {
    const accounts = makeEngine().accounts();
    let caught: unknown;
    try {
      // Replacing the reason of an account that is not blocked.
      accounts.replaceBlockReason(ACCOUNT, "still not blocked");
    } catch (err) {
      caught = err;
    }
    expect(caught).toBeInstanceOf(OpenpitError);
    expect(caught).toBeInstanceOf(AccountBlockError);
    expect((caught as AccountBlockError).name).toBe("AccountBlockError");
    expect((caught as AccountBlockError).kind).toBe("AccountNotBlocked");
    expect((caught as AccountBlockError).accountId?.value).toBe(
      BigInt(ACCOUNT),
    );
  });

  it("AccountBlockError carries the ReservedGroup kind", () => {
    const accounts = makeEngine().accounts();
    let caught: unknown;
    try {
      // The reserved default account group cannot be a block target. Passing
      // the DEFAULT group object reaches the block layer (a numeric 0 would be
      // rejected earlier as an invalid group id).
      accounts.blockGroup(AccountGroupId.DEFAULT(), "reserved");
    } catch (err) {
      caught = err;
    }
    expect(caught).toBeInstanceOf(AccountBlockError);
    expect((caught as AccountBlockError).kind).toBe("ReservedGroup");
  });

  it("AccountBlockError carries a typed group payload", () => {
    const accounts = makeEngine().accounts();
    const group = AccountGroupId.fromInt(73);
    let caught: unknown;
    try {
      accounts.replaceGroupBlockReason(group, "not blocked");
    } catch (err) {
      caught = err;
    }
    expect(caught).toBeInstanceOf(AccountBlockError);
    expect((caught as AccountBlockError).kind).toBe("GroupNotBlocked");
    expect((caught as AccountBlockError).accountGroupId?.value).toBe(73);
  });

  it("AccountGroupRegistrationError carries membership context", () => {
    const accounts = makeEngine().accounts();
    const current = AccountGroupId.fromInt(81);
    const requested = AccountGroupId.fromInt(82);
    accounts.registerGroup([ACCOUNT], current);

    let duplicate: unknown;
    try {
      accounts.registerGroup([ACCOUNT], requested);
    } catch (err) {
      duplicate = err;
    }
    expect(duplicate).toBeInstanceOf(AccountGroupRegistrationError);
    expect((duplicate as AccountGroupRegistrationError).kind).toBe(
      AccountGroupRegistrationErrorKind.AlreadyRegistered,
    );
    expect((duplicate as AccountGroupRegistrationError).accountId?.value).toBe(
      BigInt(ACCOUNT),
    );
    expect(
      (duplicate as AccountGroupRegistrationError).currentGroupId?.value,
    ).toBe(81);

    let wrongGroup: unknown;
    try {
      accounts.unregisterGroup([ACCOUNT], requested);
    } catch (err) {
      wrongGroup = err;
    }
    expect((wrongGroup as AccountGroupRegistrationError).kind).toBe(
      AccountGroupRegistrationErrorKind.NotInGroup,
    );
    expect(
      (wrongGroup as AccountGroupRegistrationError).requestedGroupId?.value,
    ).toBe(82);
    expect(
      (wrongGroup as AccountGroupRegistrationError).currentGroupId?.value,
    ).toBe(81);
  });

  it("the reject subpath re-exports the same AccountBlockError identity", () => {
    expect(RejectAccountBlockError).toBe(AccountBlockError);
    const accounts = makeEngine().accounts();
    let caught: unknown;
    try {
      accounts.blockGroup(AccountGroupId.DEFAULT(), "reserved");
    } catch (err) {
      caught = err;
    }
    // The class from the reject subpath recognizes the same instance.
    expect(caught).toBeInstanceOf(RejectAccountBlockError);
  });

  it("uses branded native TypeError and RangeError branches", () => {
    const typeError = makeError("TypeError", "wrong shape");
    const rangeError = makeError("RangeError", "outside range");

    expect(typeError).toBeInstanceOf(TypeError);
    expect(typeError).toBeInstanceOf(InternalOpenpitError);
    expect(rangeError).toBeInstanceOf(RangeError);
    expect(rangeError).toBeInstanceOf(InternalOpenpitError);
  });

  it("preserves an unknown future error name", () => {
    const error = makeError("FutureEngineError", "future failure", "FUTURE");

    expect(error).toBeInstanceOf(InternalOpenpitError);
    expect(error.name).toBe("FutureEngineError");
    expect(error.code).toBe("FUTURE");
  });

  it("preserves a callback cause when no reconciliation result exists", () => {
    const cause = new Error("callback failed");
    const error = makeError(
      "PolicyCallbackError",
      "policy callback failed",
      undefined,
      {},
      cause,
    );

    expect(error).toBeInstanceOf(InternalPolicyCallbackError);
    expect((error as InternalPolicyCallbackError).cause).toBe(cause);
    expect((error as InternalPolicyCallbackError).result).toBeUndefined();
  });

  it("keeps nested configuration distinct from validation", () => {
    const error = makeError(
      "PolicyConfigureError",
      "configuration is not reentrant",
      ConfigureErrorKind.NestedConfiguration,
      { kind: ConfigureErrorKind.NestedConfiguration },
    );

    expect(error).toBeInstanceOf(InternalPolicyConfigureError);
    expect((error as InternalPolicyConfigureError).kind).toBe(
      ConfigureErrorKind.NestedConfiguration,
    );
  });
});
