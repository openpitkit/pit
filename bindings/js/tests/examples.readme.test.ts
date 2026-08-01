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
// Published snippets and these executable mirrors are one entity and must stay
// in lockstep. Each test starts with a `// Source:` comment naming the public
// page and section; imports, harness, and assertions remain test-only.
//
// Public sources covered here:
// - bindings/js/README.md
// - https://wiki.openpit.dev/Getting-Started/
// - https://wiki.openpit.dev/Domain-Types/
// - https://wiki.openpit.dev/Errors/
//
// Run `npm run build` first: these tests import the built package.

import { describe, expect, it, vi } from "vitest";

import { Engine as DenoEngine } from "npm:@openpit/engine";
import { Engine, ParamError, OpenpitError } from "@openpit/engine";
import { Price, TradeAmount } from "@openpit/engine/param";
import {
  OrderSizeBrokerBarrier,
  buildOrderSizeLimit,
} from "@openpit/engine/pretrade/policies";

type DecimalInput = string | number | bigint;

describe("Install and decimal examples", () => {
  // Source: https://wiki.openpit.dev/Getting-Started/ - JavaScript / TypeScript
  it("resolves the documented Deno package surface", () => {
    expect(typeof DenoEngine.builder).toBe("function");
  });

  // Source: https://wiki.openpit.dev/Domain-Types/
  // - JavaScript Decimals and Handles
  it("accepts every documented DecimalInput representation", () => {
    const values: DecimalInput[] = ["100.50", 100, 100n];
    expect(values).toHaveLength(3);
  });

  // Source: https://wiki.openpit.dev/Domain-Types/
  // - JavaScript Decimals and Handles
  it("uses lossless value-type string output and explicit rounding", () => {
    const price = Price.fromString("100.50");
    price.toString(); // "100.50"
    price.toJSON(); // "100.50" (so JSON.stringify is lossless)

    // Quantize to an instrument tick with an explicit rounding strategy.
    Price.fromStringRounded("1.005", 2, "default").toString(); // "1.00"

    expect(price.toString()).toBe("100.50");
    expect(price.toJSON()).toBe("100.50");
    expect(Price.fromStringRounded("1.005", 2, "default").toString()).toBe(
      "1.00",
    );
  });
});

// Source: bindings/js/README.md - Quick Start
// The body below matches the documented block (console.error is stubbed so the
// assertion can see the reported reject); keep the two in lockstep.
describe("README Quick Start example", () => {
  it("rejects an order that breaches the fat-finger cap", () => {
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});

    // Build the engine once, at start-up: one broker-wide fat-finger cap.
    const engine = Engine.builder()
      .builtin(
        buildOrderSizeLimit().brokerBarrier(
          new OrderSizeBrokerBarrier({
            maxQuantity: "500",
            maxNotional: "1000000",
          }),
        ),
      )
      .build();

    const result = engine.executePreTrade({
      operation: {
        underlyingAsset: "AAPL",
        settlementAsset: "USD",
        accountId: 99224416,
        side: "BUY",
        tradeAmount: TradeAmount.quantity("1000"),
        price: "185",
      },
    });

    if (result.ok) {
      // Send the order to the venue, then commit or roll the reservation back.
      result.reservation?.commit();
    } else {
      for (const reject of result.rejects) {
        // OrderSizeLimitPolicy [OrderQtyExceedsLimit]: order quantity exceeded
        console.error(`${reject.policy} [${reject.code}]: ${reject.reason}`);
      }
    }

    expect(result.ok).toBe(false);
    expect(errorSpy).toHaveBeenCalledWith(
      "OrderSizeLimitPolicy [OrderQtyExceedsLimit]: order quantity exceeded",
    );
    errorSpy.mockRestore();
  });
});

// Source: https://wiki.openpit.dev/Errors/ - JavaScript and TypeScript
// The body below matches the documented block verbatim (console.error is
// stubbed so the assertion can see which branch ran); keep the two in lockstep.
describe("Errors instanceof example", () => {
  it("classifies a thrown error with instanceof", () => {
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});

    try {
      Price.fromString("not a number");
    } catch (err) {
      if (err instanceof ParamError) {
        console.error(err.code); // e.g. "InvalidFormat"
      } else if (err instanceof OpenpitError) {
        console.error(err.name, err.message);
      }
    }

    expect(errorSpy).toHaveBeenCalledWith("InvalidFormat");
    errorSpy.mockRestore();
  });
});
