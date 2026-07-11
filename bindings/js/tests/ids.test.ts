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

import { Engine } from "@openpit/engine";
import { OrderOperation } from "@openpit/engine/model";
import { QuoteTtl } from "@openpit/engine/marketdata";
import { AccountId } from "@openpit/engine/param";

const MAX_U64 = 18_446_744_073_709_551_615n;
const UNSAFE_NUMBER = 2 ** 53 + 1;

describe("64-bit id primitive inputs", () => {
  it("accepts Number.MAX_SAFE_INTEGER for AccountId", () => {
    const operation = new OrderOperation();
    operation.accountId = Number.MAX_SAFE_INTEGER;

    expect(operation.accountId?.value).toBe(9_007_199_254_740_991n);
  });

  it("rejects an unsafe AccountId number with bigint/string guidance", () => {
    const operation = new OrderOperation();

    expect(() => {
      operation.accountId = UNSAFE_NUMBER;
    }).toThrow(/Number\.MAX_SAFE_INTEGER; use bigint.*string/);
  });

  it("accepts full-range AccountId bigint and semantic string inputs", () => {
    const operation = new OrderOperation();
    operation.accountId = MAX_U64;
    expect(operation.accountId?.value).toBe(MAX_U64);

    const stringId = MAX_U64.toString();
    const expected = AccountId.fromString(stringId).value;
    operation.accountId = stringId;
    expect(operation.accountId?.value).toBe(expected);
  });

  it("accepts Number.MAX_SAFE_INTEGER for InstrumentId", () => {
    const service = Engine.builder().marketData(QuoteTtl.infinite()).build();
    const id = service.registerWithId(
      { underlyingAsset: "SAFE", settlementAsset: "USD" },
      Number.MAX_SAFE_INTEGER,
    );

    expect(id.value).toBe(9_007_199_254_740_991n);
  });

  it("rejects an unsafe InstrumentId number with bigint/string guidance", () => {
    const service = Engine.builder().marketData(QuoteTtl.infinite()).build();

    expect(() =>
      service.registerWithId(
        { underlyingAsset: "UNSAFE", settlementAsset: "USD" },
        UNSAFE_NUMBER,
      ),
    ).toThrow(/Number\.MAX_SAFE_INTEGER; use bigint.*string/);
  });

  it("accepts the full InstrumentId range as bigint and decimal string", () => {
    const bigintService = Engine.builder()
      .marketData(QuoteTtl.infinite())
      .build();
    const bigintId = bigintService.registerWithId(
      { underlyingAsset: "BIGINT", settlementAsset: "USD" },
      MAX_U64,
    );
    expect(bigintId.value).toBe(MAX_U64);

    const stringService = Engine.builder()
      .marketData(QuoteTtl.infinite())
      .build();
    const stringId = stringService.registerWithId(
      { underlyingAsset: "STRING", settlementAsset: "USD" },
      MAX_U64.toString(),
    );
    expect(stringId.value).toBe(MAX_U64);
  });
});
