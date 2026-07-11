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
import { Engine } from "@openpit/engine";
import { AccountId, Price, TradeAmount } from "@openpit/engine/param";
import { Order, OrderOperation, type OrderInit } from "@openpit/engine/model";
import { buildOrderValidation } from "@openpit/engine/pretrade/policies";

// Builds the engine the whole module shares: a single order-validation policy,
// enough to exercise the start and main pre-trade stages.
function makeEngine(): Engine {
  return Engine.builder().builtin(buildOrderValidation()).build();
}

// Runs the full two-stage pre-trade flow and commits on accept. Returns whether
// the order was accepted plus the lock size observed on the reservation, so the
// wrapper and plain-object paths can be compared on identical observables.
function runFlow(
  engine: Engine,
  order: Order | OrderInit,
): {
  ok: boolean;
  lockSize: number;
} {
  const start = engine.startPreTrade(order);
  if (!start.ok) {
    return { ok: false, lockSize: 0 };
  }
  const execute = start.request!.execute();
  if (!execute.ok) {
    return { ok: false, lockSize: 0 };
  }
  const reservation = execute.reservation!;
  const lockSize = reservation.lock().size();
  reservation.commit();
  return { ok: true, lockSize };
}

describe("plain-object inputs", () => {
  it("runs the full pre-trade flow with an order given as object literals", () => {
    // The entire order is plain object literals plus primitive values: the
    // account id as a number, the side and price as strings. The only wrapper
    // is TradeAmount, which has no plain-object form.
    const engine = makeEngine();
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

    const result = runFlow(engine, order);
    expect(result.ok).toBe(true);
  });

  it("matches the wrapper path outcome for the same order", () => {
    // Build the same order twice - once entirely from wrapper classes, once
    // entirely from plain values - and assert the engine reaches an identical
    // accept verdict and lock for both.
    const wrapperEngine = makeEngine();
    const operation = new OrderOperation();
    operation.underlyingAsset = "AAPL";
    operation.settlementAsset = "USD";
    operation.accountId = AccountId.fromInt(99224416n);
    operation.side = "BUY";
    operation.tradeAmount = TradeAmount.quantity("100");
    operation.price = Price.fromString("185.00");
    const wrapped = new Order();
    wrapped.operation = operation;
    const wrapperResult = runFlow(wrapperEngine, wrapped);

    const pojoEngine = makeEngine();
    const pojoResult = runFlow(pojoEngine, {
      operation: {
        underlyingAsset: "AAPL",
        settlementAsset: "USD",
        accountId: 99224416,
        side: "BUY",
        tradeAmount: TradeAmount.quantity("100"),
        price: "185.00",
      },
    });

    expect(pojoResult).toEqual(wrapperResult);
    expect(pojoResult.ok).toBe(true);
  });

  it("accepts the account id as a number and as a string", () => {
    // Both primitive id forms denote the same account, so both orders pass.
    for (const accountId of [99224416, "99224416"]) {
      const engine = makeEngine();
      const result = runFlow(engine, {
        operation: {
          underlyingAsset: "AAPL",
          settlementAsset: "USD",
          accountId,
          side: "BUY",
          tradeAmount: TradeAmount.quantity("100"),
          price: "185.00",
        },
      });
      expect(result.ok, `accountId ${JSON.stringify(accountId)}`).toBe(true);
    }
  });

  it("accepts a decimal price as a string, number, and bigint", () => {
    // "185", 185, and 185n all denote the same price; each is a valid order.
    const decimals: ReadonlyArray<string | number | bigint> = [
      "185",
      185,
      185n,
    ];
    for (const price of decimals) {
      const engine = makeEngine();
      const result = runFlow(engine, {
        operation: {
          underlyingAsset: "AAPL",
          settlementAsset: "USD",
          accountId: 99224416,
          side: "BUY",
          tradeAmount: TradeAmount.quantity("100"),
          price,
        },
      });
      expect(result.ok, `price ${String(price)} (${typeof price})`).toBe(true);
    }
  });

  it("rejects a structurally invalid object-literal order at the start stage", () => {
    // An empty operation has no side/amount/instrument, so order-validation
    // refuses it - the plain-object path surfaces rejects exactly as the
    // wrapper path does.
    const engine = makeEngine();
    const start = engine.startPreTrade({ operation: {} });
    expect(start.ok).toBe(false);
    expect(start.request).toBeUndefined();
    expect(start.rejects.length).toBeGreaterThan(0);
  });

  it.each([new Date(0), new Map<string, string>(), Promise.resolve()])(
    "rejects a built-in object instead of treating it as an init record: %s",
    (value) => {
      expect(() => makeEngine().startPreTrade(value as never)).toThrow(
        TypeError,
      );
    },
  );

  it("rejects a wrapper of the wrong class without consuming it", () => {
    const price = Price.fromString("185");

    expect(() => makeEngine().startPreTrade(price as never)).toThrow(TypeError);
    expect(price.toString()).toBe("185");
  });

  it("accepts a structural application class as a custom order model", () => {
    class CustomOrder {
      readonly operation = {
        underlyingAsset: "AAPL",
        settlementAsset: "USD",
        accountId: 99224416,
        side: "BUY" as const,
        tradeAmount: TradeAmount.quantity("100"),
        price: "185.00",
      };

      strategyName(): string {
        return "class-backed";
      }
    }

    const result = runFlow(makeEngine(), new CustomOrder());
    expect(result.ok).toBe(true);
  });
});
