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

// See engine.test.ts for the import-resolution scheme. Run `npm run build`
// first.
import {
  FillType,
  Leverage,
  ParamKind,
  Price,
  Quantity,
  RoundingStrategies,
  type RoundingStrategy,
} from "@openpit/engine/param";
import { RejectCode, RejectScope } from "@openpit/engine/reject";
import { SpotFundsPricingSource } from "@openpit/engine/pretrade/policies";

describe("decimal string round-trips", () => {
  it("round-trips a decimal string losslessly via fromString/toString", () => {
    expect(Price.fromString("100.50").toString()).toBe("100.50");
    expect(Quantity.fromString("0.00847000").toString()).toBe("0.00847000");
  });

  it("serializes via toJSON to the canonical decimal string", () => {
    expect(Price.fromString("1.23").toJSON()).toBe("1.23");
    // toJSON and toString agree on the canonical form.
    const price = Price.fromString("42.000");
    expect(price.toJSON()).toBe(price.toString());
  });

  it("is the lossless bridge for JSON.stringify", () => {
    // toJSON is honored by JSON.stringify, so a price serializes as a string.
    expect(JSON.stringify({ price: Price.fromString("185.25") })).toBe(
      '{"price":"185.25"}',
    );
  });
});

describe("fromInt and fromFloat factories", () => {
  it("constructs from an exact integer via fromInt", () => {
    expect(Price.fromInt(42n).toString()).toBe("42");
    expect(Quantity.fromInt(1000n).toString()).toBe("1000");
    expect(Price.fromInt(9_223_372_036_854_775_808n).toString()).toBe(
      "9223372036854775808",
    );
    expect(() => Price.fromInt(1n << 127n)).toThrow();
  });

  it("constructs from a JS number via fromFloat", () => {
    // Exact, representable floats round-trip cleanly.
    expect(Price.fromFloat(1.5).toString()).toBe("1.5");
    expect(Price.fromFloat(100).toString()).toBe("100");
  });

  it("reconstructs fractional leverage directly from fixed-point storage", () => {
    expect(Leverage.fromFloat(1.1).value).toBe(1.1);
    expect(Leverage.fromFloat(123.4).value).toBe(123.4);
    expect(Leverage.STEP()).toBe(0.1);
  });
});

describe("DecimalInput constructor", () => {
  it("accepts a string", () => {
    expect(new Price("185.25").toString()).toBe("185.25");
  });

  it("accepts a bigint (exact integer)", () => {
    expect(new Price(42n).toString()).toBe("42");
  });

  it("accepts a number (small exact integer)", () => {
    expect(new Price(7).toString()).toBe("7");
  });

  it("keeps a large safe-integer number exact", () => {
    // Number.MAX_SAFE_INTEGER (2^53 - 1) is an exact double; the boundary
    // routes integral safe integers through the lossless string path, so no
    // trailing digit is dropped the way the float path would.
    expect(new Price(9007199254740991).toString()).toBe("9007199254740991");
    expect(new Quantity(Number.MAX_SAFE_INTEGER).toString()).toBe(
      "9007199254740991",
    );
  });

  it("requires an explicit float factory for fractional numbers", () => {
    expect(() => new Price(1.5)).toThrow(RangeError);
    expect(() => new Price(0.5)).toThrow(RangeError);
    expect(Price.fromFloat(1.5).toString()).toBe("1.5");
  });

  it("rejects unsafe and non-finite generic number inputs", () => {
    expect(() => new Price(Number.MAX_SAFE_INTEGER + 1)).toThrow(RangeError);
    expect(() => new Price(Number.NaN)).toThrow(RangeError);
    expect(() => new Price(Number.POSITIVE_INFINITY)).toThrow(RangeError);
  });

  it("treats zero and small integers as exact", () => {
    // The integral boundary includes 0 and negative integers.
    expect(new Price(0).toString()).toBe("0");
    expect(new Quantity(1000000).toString()).toBe("1000000");
  });
});

describe("rounding via fromStringRounded", () => {
  it("supports all four canonical core strategies", () => {
    expect(
      Price.fromStringRounded("1.005", 2, "midpointNearestEven").toString(),
    ).toBe("1.00");
    expect(
      Price.fromStringRounded("1.005", 2, "midpointAwayFromZero").toString(),
    ).toBe("1.01");
    expect(Price.fromStringRounded("1.001", 2, "up").toString()).toBe("1.01");
    expect(Price.fromStringRounded("1.999", 2, "down").toString()).toBe("1.99");
    expect(Price.fromStringRounded("-1.001", 2, "up").toString()).toBe("-1.00");
    expect(Price.fromStringRounded("-1.001", 2, "down").toString()).toBe(
      "-1.01",
    );
  });

  it("rounds half-to-even with the default strategy", () => {
    // 1.005 at scale 2 with banker's rounding lands on the even digit (1.00).
    expect(Price.fromStringRounded("1.005", 2, "default").toString()).toBe(
      "1.00",
    );
    expect(Price.fromStringRounded("1.015", 2, "banker").toString()).toBe(
      "1.02",
    );
  });

  it("rounds down with the conservative strategies", () => {
    expect(
      Price.fromStringRounded("1.999", 2, "conservativeLoss").toString(),
    ).toBe("1.99");
    expect(
      Price.fromStringRounded("1.999", 2, "conservativeProfit").toString(),
    ).toBe("1.99");
    expect(
      Price.fromStringRounded("-1.001", 2, "conservativeLoss").toString(),
    ).toBe("-1.01");
  });

  it("rounds floats with fromFloatRounded", () => {
    expect(Price.fromFloatRounded(1.005, 2, "default").toString()).toBe("1.00");
  });
});

describe("invalid input handling", () => {
  it("throws ParamError on a malformed decimal string", () => {
    expect(() => Price.fromString("not-a-number")).toThrowError(
      /invalid format/,
    );
  });
});

describe("runtime value sets", () => {
  it("exposes reject codes as their wire strings", () => {
    expect(RejectCode.InsufficientFunds).toBe("InsufficientFunds");
    expect(RejectCode.PnlKillSwitchTriggered).toBe("PnlKillSwitchTriggered");
  });

  it("exposes reject scope, rounding, and pricing-source constants", () => {
    expect(RejectScope.Order).toBe("order");
    expect(RejectScope.Account).toBe("account");
    expect(RoundingStrategies.Default).toBe("default");
    expect(RoundingStrategies.MidpointNearestEven).toBe("midpointNearestEven");
    expect(RoundingStrategies.MidpointAwayFromZero).toBe(
      "midpointAwayFromZero",
    );
    expect(RoundingStrategies.Up).toBe("up");
    expect(RoundingStrategies.Down).toBe("down");
    expect(RoundingStrategies.ConservativeLoss).toBe("conservativeLoss");
    expect(SpotFundsPricingSource.Mark).toBe("Mark");
    expect(SpotFundsPricingSource.BookTop).toBe("BookTop");
  });

  it("exposes FillType and ParamKind with the core wire values", () => {
    expect(FillType.Trade).toBe("TRADE");
    expect(FillType.Liquidation).toBe("LIQUIDATION");
    expect(FillType.AutoDeleverage).toBe("AUTO_DELEVERAGE");
    expect(FillType.Settlement).toBe("SETTLEMENT");
    expect(FillType.Funding).toBe("FUNDING");
    expect(Object.values(ParamKind)).toEqual([
      "Quantity",
      "Volume",
      "Notional",
      "Price",
      "Pnl",
      "CashFlow",
      "PositionSize",
      "Fee",
      "Leverage",
    ]);
  });

  it("keeps the plain string literals assignable to the union types", () => {
    // The const value and a hand-written literal are interchangeable, so the
    // additive constants never force callers off the string form.
    const fromConst: RoundingStrategy = RoundingStrategies.Banker;
    const fromLiteral: RoundingStrategy = "banker";
    expect(fromConst).toBe(fromLiteral);
    expect(
      Price.fromStringRounded("1.015", 2, RoundingStrategies.Banker).toString(),
    ).toBe("1.02");
  });
});
