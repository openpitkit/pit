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
// Source: bindings/js/README.md - Install, Decimals, Usage, and Errors
//
// The tests below mirror the README snippets, per the doc-mirror rule in
// doc/code_style.md. Keep both sides in lockstep.

import { describe, expect, it, vi } from "vitest";

import { Engine as DenoEngine } from "npm:@openpit/engine";
import { Engine, ParamError, OpenpitError } from "@openpit/engine";
import { Price, TradeAmount } from "@openpit/engine/param";
import {
  type OrderInit,
  type ExecutionReportInit,
} from "@openpit/engine/model";
import { buildOrderValidation } from "@openpit/engine/pretrade/policies";

type DecimalInput = string | number | bigint;

describe("README install and decimal examples", () => {
  it("resolves the documented Deno package surface", () => {
    expect(typeof DenoEngine.builder).toBe("function");
  });

  it("accepts every documented DecimalInput representation", () => {
    const values: DecimalInput[] = ["100.50", 100, 100n];
    expect(values).toHaveLength(3);
  });

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

describe("README Usage example", () => {
  it("runs the documented end-to-end flow", () => {
    // 1. Build the engine once, at platform initialization.
    const engine = Engine.builder().builtin(buildOrderValidation()).build();

    // 2. Assemble an order as a plain object. Scalars accept plain values (the
    //    account id as a number, the price as a decimal string); the order itself
    //    is an object literal - no wrapper classes to construct. The OrderInit
    //    annotation is optional; it just lets the literal sit in its own variable.
    const order: OrderInit = {
      operation: {
        underlyingAsset: "AAPL",
        settlementAsset: "USD",
        accountId: 99224416,
        side: "BUY",
        tradeAmount: TradeAmount.quantity("100"),
        price: "185.00",
      },
    };

    // 3. Start stage: lightweight checks, no state change yet.
    const start = engine.startPreTrade(order);
    if (!start.ok) {
      const reasons = start.rejects
        .map((r) => `${r.policy} [${r.code}]: ${r.reason}`)
        .join(", ");
      throw new Error(reasons);
    }

    // 4. Main stage: full pre-trade and risk control.
    const request = start.request;
    if (request === undefined) {
      throw new Error("accepted start result is missing its request");
    }
    const execute = request.execute();
    if (!execute.ok) {
      const reasons = execute.rejects
        .map((r) => `${r.policy} [${r.code}]: ${r.reason}`)
        .join(", ");
      throw new Error(reasons);
    }

    // 5. Commit once the venue accepts the order; roll back otherwise.
    const reservation = execute.reservation;
    if (reservation === undefined) {
      throw new Error("accepted execute result is missing its reservation");
    }
    try {
      // sendOrderToVenue(order);
      reservation.commit();
    } catch (err) {
      reservation.rollback();
      throw err;
    }

    // 6. Feed the venue's execution report back into post-trade policy state, again
    //    as a plain object literal. P&L and fee cross as decimal strings.
    const report: ExecutionReportInit = {
      operation: {
        underlyingAsset: "AAPL",
        settlementAsset: "USD",
        accountId: 99224416,
        side: "BUY",
      },
      financialImpact: { pnl: "-50", fee: "3.4" },
    };

    const result = engine.applyExecutionReport(report);
    // A non-empty `accountBlocks` means a kill switch has fired for the account.
    if (result.accountBlocks.length > 0) {
      // Halt routing for the blocked account.
    }

    expect(result.accountBlocks).toHaveLength(0);
  });
});

// Mirrors the README "Errors" instanceof snippet. The body below matches the
// documented block verbatim (console.error is stubbed so the assertion can see
// which branch ran); keep the two in lockstep.
describe("README Errors example", () => {
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
