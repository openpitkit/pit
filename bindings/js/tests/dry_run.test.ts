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
// Mirrors the public JS dry-run examples from:
// - ../../../../pit.wiki/Non-Mutating-Dry-Run.md

import { describe, expect, it } from "vitest";

import { Engine } from "@openpit/engine";
import { AdjustmentAmount, TradeAmount } from "@openpit/engine/param";
import { type OrderInit } from "@openpit/engine/model";
import { type Policy } from "@openpit/engine/pretrade";
import {
  buildOrderValidation,
  buildRateLimit,
  buildSpotFunds,
  RateLimit,
  RateLimitBrokerBarrier,
} from "@openpit/engine/pretrade/policies";

const ACCOUNT = 99_224_416n;

function makeOrder(quantity: string = "1", price: string = "100"): OrderInit {
  return {
    operation: {
      underlyingAsset: "AAPL",
      settlementAsset: "USD",
      accountId: ACCOUNT,
      side: "BUY",
      tradeAmount: TradeAmount.quantity(quantity),
      price,
    },
  };
}

function makeValidationEngine(): Engine {
  return Engine.builder().builtin(buildOrderValidation()).build();
}

function makeRateLimitEngine(maxOrders: number = 1): Engine {
  return Engine.builder()

    .builtin(
      buildRateLimit().brokerBarrier(
        new RateLimitBrokerBarrier(new RateLimit(maxOrders, 60_000)),
      ),
    )
    .build();
}

function makeSpotFundsEngine(): Engine {
  const engine = Engine.builder().builtin(buildSpotFunds()).build();
  const seed = engine.applyAccountAdjustment(ACCOUNT, [
    {
      operation: { asset: "USD" },
      amount: { balance: AdjustmentAmount.absolute("100000") },
    },
  ]);
  expect(seed.ok).toBe(true);
  return engine;
}

describe("dry-run surface", () => {
  it("reads the dry-run verdict", () => {
    // Source: Non-Mutating-Dry-Run.md - Read the Dry-Run Verdict
    const engine = Engine.builder().builtin(buildOrderValidation()).build();
    const order: OrderInit = {
      operation: {
        underlyingAsset: "AAPL",
        settlementAsset: "USD",
        accountId: 99_224_416n,
        side: "BUY",
        tradeAmount: TradeAmount.quantity("1"),
        price: "100",
      },
    };

    const report = engine.executePreTradeDryRun(order);
    if (report.isPass) {
      console.log("order would be admitted");
    } else {
      for (const reject of report.rejects) {
        console.log(
          `would reject by ${reject.policy} [${reject.code}]: ${reject.reason}: ${reject.details}`,
        );
      }
    }

    expect(report.isPass).toBe(true);
    expect(report.rejects).toEqual([]);
    expect(report.accountBlock).toBeUndefined();
  });

  it("passes and rejects without mutating the engine", () => {
    const engine = makeValidationEngine();

    const pass = engine.executePreTradeDryRun(makeOrder());
    expect(pass.isPass).toBe(true);

    const fail = engine.executePreTradeDryRun({ operation: {} });
    expect(fail.isPass).toBe(false);
    expect(fail.rejects.length).toBeGreaterThan(0);
  });

  it("uses the dry run before a real call", () => {
    // Source: Non-Mutating-Dry-Run.md - Use the Dry-Run Before a Real Call
    const engine = Engine.builder()
      .builtin(
        buildRateLimit().brokerBarrier(
          new RateLimitBrokerBarrier(new RateLimit(1, 60_000)),
        ),
      )
      .build();
    const order = (): OrderInit => ({
      operation: {
        underlyingAsset: "AAPL",
        settlementAsset: "USD",
        accountId: 99_224_416n,
        side: "BUY",
        tradeAmount: TradeAmount.quantity("1"),
        price: "100",
      },
    });

    // Probe without spending any budget or creating any reservation.
    const probe = engine.executePreTradeDryRun(order());
    if (!probe.isPass) {
      // would have been rejected - skip the real call
    } else {
      // The real call now runs with fresh state; the probe had no effect.
      const result = engine.executePreTrade(order());
      if (result.ok) {
        const reservation = result.reservation;
        if (reservation === undefined) {
          throw new Error("accepted execute result is missing its reservation");
        }
        reservation.commit();
      }
    }

    expect(probe.isPass).toBe(true);
  });

  it("leaves rate-limit budget untouched until a real call happens", () => {
    const engine = makeRateLimitEngine(1);

    for (let attempt = 0; attempt < 5; attempt += 1) {
      const dry = engine.executePreTradeDryRun(makeOrder());
      expect(dry.isPass).toBe(true);
    }

    const real = engine.executePreTrade(makeOrder());
    expect(real.ok).toBe(true);
    real.reservation!.rollback();

    const blocked = engine.executePreTradeDryRun(makeOrder());
    expect(blocked.isPass).toBe(false);
    expect(blocked.rejects[0]!.code).toBe("RateLimitExceeded");
  });

  it("uses a custom read-only start hook without running normal side effects", () => {
    // Source: Non-Mutating-Dry-Run.md - Read-Only Custom Start-Stage Hook
    const counterState: { acceptedOrders: number; limit: number } = {
      acceptedOrders: 0,
      limit: 10,
    };

    const readOnlyStartPolicy: Policy = {
      name: "ReadOnlyStartPolicy",

      checkPreTradeStart() {
        counterState.acceptedOrders += 1;
        return [];
      },

      checkPreTradeStartDryRun() {
        if (counterState.acceptedOrders >= counterState.limit) {
          return [
            {
              code: "RateLimitExceeded",
              reason: "counter budget exhausted",
              details: "the read-only start check observed a full counter",
              scope: "order",
            },
          ];
        }
        return [];
      },

      performPreTradeCheck() {
        return null;
      },
    };

    const engine = Engine.builder().preTrade(readOnlyStartPolicy).build();
    const order: OrderInit = {
      operation: {
        underlyingAsset: "AAPL",
        settlementAsset: "USD",
        accountId: 99_224_416n,
        side: "BUY",
        tradeAmount: TradeAmount.quantity("1"),
        price: "100",
      },
    };

    const probe = engine.startPreTradeDryRun(order);
    if (!probe.isPass || counterState.acceptedOrders !== 0) {
      throw new Error("the dry-run start hook must remain read-only");
    }

    const real = engine.executePreTrade(order);
    if (!real.ok || Number(counterState.acceptedOrders) !== 1) {
      throw new Error("the real start hook must apply its side effect");
    }
    const reservation = real.reservation;
    if (reservation === undefined) {
      throw new Error("accepted execute result is missing its reservation");
    }
    reservation.rollback();

    expect(probe.isPass).toBe(true);
    expect(counterState.acceptedOrders).toBe(1);
  });

  it("keeps start-stage dry runs lock-free and surfaces main-stage spot-funds effects", () => {
    const engine = makeSpotFundsEngine();

    const start = engine.startPreTradeDryRun(makeOrder("30", "2000"));
    expect(start.isPass).toBe(true);
    expect(start.lock().size()).toBe(0);
    expect(start.accountAdjustments()).toEqual([]);

    const execute = engine.executePreTradeDryRun(makeOrder("30", "2000"));
    expect(execute.isPass).toBe(true);
    expect(execute.lock().size()).toBeGreaterThan(0);
    expect(execute.accountAdjustments().length).toBeGreaterThan(0);

    const real = engine.executePreTrade(makeOrder("30", "2000"));
    expect(real.ok).toBe(true);
    real.reservation!.rollback();
  });
});
