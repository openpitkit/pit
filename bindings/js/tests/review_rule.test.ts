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
import { Instrument, TradeAmount } from "@openpit/engine/param";
import { buildReviewRule, ReviewRule } from "@openpit/engine/pretrade/policies";
import { RejectCode, RuleMode } from "@openpit/engine/reject";

describe("review rules", () => {
  it.each([RuleMode.Reject, RuleMode.ReviewWithHold])(
    "applies the %s instance verdict to every instrument of its account",
    (mode) => {
      const engine = Engine.builder()
        .builtin(
          buildReviewRule()
            .withPolicyGroupId(3)
            .accountRules([
              new ReviewRule({ accountId: 1001, instruments: null }),
            ]),
          { name: "account-review", mode },
        )
        .build();
      engine.accounts().registerGroup([1001], 7);
      const run = (accountId: number, instrument: Instrument) =>
        engine.executePreTradeDryRun({
          operation: {
            accountId,
            underlyingAsset: instrument.underlyingAsset,
            settlementAsset: instrument.settlementAsset,
            side: "BUY",
            tradeAmount: TradeAmount.quantity("10"),
            price: "25",
          },
        });
      const aapl = new Instrument("AAPL", "USD");
      for (const instrument of [aapl, new Instrument("MSFT", "USD")]) {
        const report = run(1001, instrument);
        const expected = [{ code: RejectCode.ComplianceRestriction, mode }];
        expect(report.isPass).toBe(mode !== RuleMode.Reject);
        expect(report.rejects).toMatchObject(
          mode === RuleMode.Reject ? expected : [],
        );
        expect(report.reviews).toMatchObject(
          mode === RuleMode.Reject ? [] : expected,
        );
      }
      const other = run(1002, aapl);
      expect(other.isPass).toBe(true);
      expect(other.rejects).toEqual([]);
      expect(other.reviews).toEqual([]);
    },
  );

  it("matches exact instruments from a Set as from an array", () => {
    const exact = [
      new Instrument("AAPL", "USD"),
      { underlyingAsset: "MSFT", settlementAsset: "USD" },
    ];
    for (const instruments of [exact, new Set(exact)]) {
      const engine = Engine.builder()
        .builtin(buildReviewRule().globalRule(new ReviewRule({ instruments })))
        .build();
      for (const [underlyingAsset, settlementAsset, isPass] of [
        ["AAPL", "USD", false],
        ["MSFT", "USD", false],
        ["AAPL", "EUR", true],
        ["TSLA", "USD", true],
      ] as const) {
        const result = engine.executePreTradeDryRun({
          operation: {
            accountId: 1001,
            underlyingAsset,
            settlementAsset,
            side: "BUY",
            tradeAmount: TradeAmount.quantity("10"),
            price: "25",
          },
        });
        expect(result.isPass).toBe(isPass);
      }
    }
  });

  it("rejects a non-iterable instrument selection", () => {
    expect(() => {
      // @ts-expect-error An instrument selection must be iterable.
      new ReviewRule({ instruments: {} });
    }).toThrow(TypeError);
  });
});
