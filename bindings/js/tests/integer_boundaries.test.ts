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

import { describe, expect, it } from "vitest";

import { Engine, ParamError } from "@openpit/engine";
import { InstrumentId, QuoteTtl } from "@openpit/engine/marketdata";
import {
  AccountGroupId,
  AccountId,
  Leverage,
  Price,
} from "@openpit/engine/param";
import { Lock } from "@openpit/engine/pretrade";
import { AccountBlock } from "@openpit/engine/reject";
import {
  buildOrderValidation,
  buildSpotFunds,
  RateLimit,
  SpotFundsOverride,
} from "@openpit/engine/pretrade/policies";

describe("integer boundary validation", () => {
  it("rejects out-of-range exact bigint factories before wasm narrowing", () => {
    expect(() => AccountId.fromInt(-1n)).toThrow(RangeError);
    expect(() => AccountId.fromInt(1n << 64n)).toThrow(RangeError);
    expect(() => new InstrumentId(-1n)).toThrow(RangeError);
    expect(() => new InstrumentId(1n << 64n)).toThrow(RangeError);
  });

  it("validates integer-number factories before narrowing", () => {
    expect(() => AccountGroupId.fromInt(0)).toThrow(ParamError);
    expect(() => AccountGroupId.fromInt(0x1_0000_0000)).toThrow(RangeError);
    expect(() => AccountGroupId.fromInt(1.5)).toThrow(RangeError);
    expect(() => Leverage.fromInt(65_536)).toThrow(RangeError);
  });

  it.each([-1, 65_536, 1.5, Number.NaN, Number.POSITIVE_INFINITY])(
    "rejects a non-u16 policy group before wasm narrowing: %s",
    (policyGroupId) => {
      expect(() =>
        buildOrderValidation().withPolicyGroupId(policyGroupId),
      ).toThrow(RangeError);
    },
  );

  it.each([-1, 65_536, 0x1_0000_0000, 1.5, Number.NaN])(
    "rejects a non-u16 lock group before wasm narrowing: %s",
    (policyGroupId) => {
      const lock = new Lock(undefined);
      expect(() => lock.push(policyGroupId, Price.fromString("1"))).toThrow(
        RangeError,
      );
    },
  );

  it.each([-1, 0x1_0000_0000, 1.5, Number.NaN])(
    "rejects a non-u32 rounding scale before wasm narrowing: %s",
    (scale) => {
      expect(() => Price.fromStringRounded("1.5", scale, "default")).toThrow(
        RangeError,
      );
    },
  );

  it.each([-1, 0x1_0000_0000, 1.5, Number.NaN])(
    "rejects a non-wasm32 rate limit count before narrowing: %s",
    (maxOrders) => {
      expect(() => new RateLimit(maxOrders, 1_000)).toThrow(RangeError);
    },
  );

  it("rejects slippage values before u32/u16 narrowing", () => {
    expect(
      () => new SpotFundsOverride(1n, undefined, undefined, 0x1_0000_0000),
    ).toThrow(RangeError);

    const marketData = Engine.builder().marketData(QuoteTtl.infinite()).build();
    expect(() =>
      buildSpotFunds().marketData(marketData, 0x1_0000_0000, "Mark", []),
    ).toThrow(RangeError);
  });

  it("validates opaque userData before the wasm32 usize boundary", () => {
    expect(
      () => new AccountBlock("policy", "Other", "reason", "details", 1n << 32n),
    ).toThrow(RangeError);
  });
});
