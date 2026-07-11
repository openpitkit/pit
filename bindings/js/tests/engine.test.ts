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

// Tests import the package by name (root and subpaths). At runtime vitest
// resolves each to the built Node entry (real disk-loaded wasm, single shared
// instance); for types it resolves to the source barrels. Run `npm run build`
// first.
import { Engine } from "@openpit/engine";
import { AccountId, Fee, Pnl, TradeAmount } from "@openpit/engine/param";
import {
  ExecutionReport,
  ExecutionReportOperation,
  FinancialImpact,
  Order,
  OrderOperation,
} from "@openpit/engine/model";
import { buildOrderValidation } from "@openpit/engine/pretrade/policies";

// Builds a structurally valid buy order that passes order-validation checks.
function makeOrder(): Order {
  const order = new Order();
  const operation = new OrderOperation();
  operation.underlyingAsset = "AAPL";
  operation.settlementAsset = "USD";
  operation.accountId = AccountId.fromInt(99224416n);
  operation.side = "BUY";
  operation.tradeAmount = TradeAmount.quantity("100");
  operation.price = "185.0";
  order.operation = operation;
  return order;
}

describe("Engine", () => {
  it("builds an engine with a builtin policy", () => {
    const engine = Engine.builder().builtin(buildOrderValidation()).build();
    expect(typeof engine.startPreTrade).toBe("function");
  });

  it("exposes the staged builder surface", () => {
    const builder = Engine.builder();
    expect(typeof builder.builtin).toBe("function");
    expect(typeof builder.preTrade).toBe("function");
    expect(typeof builder.marketData).toBe("function");
    expect("noSync" in builder).toBe(false);
    expect("fullSync" in builder).toBe(false);
  });

  it("runs the full pre-trade flow and commits the reservation", () => {
    const engine = Engine.builder()

      .builtin(buildOrderValidation())
      .build();

    const start = engine.startPreTrade(makeOrder());
    expect(start.ok).toBe(true);
    expect(start.rejects).toHaveLength(0);

    const request = start.request;
    expect(request).toBeDefined();

    const execute = request!.execute();
    expect(execute.ok).toBe(true);
    expect(execute.rejects).toHaveLength(0);

    const reservation = execute.reservation;
    expect(reservation).toBeDefined();

    // The lock payload is readable while the reservation is live.
    const lock = reservation!.lock();
    expect(typeof lock.size()).toBe("number");

    // Commit applies the reserved state. It must not throw the first time.
    expect(() => reservation!.commit()).not.toThrow();
  });

  it("rolls back a reservation instead of committing", () => {
    const engine = Engine.builder()

      .builtin(buildOrderValidation())
      .build();

    const execute = engine.executePreTrade(makeOrder());
    expect(execute.ok).toBe(true);

    const reservation = execute.reservation;
    expect(reservation).toBeDefined();
    expect(() => reservation!.rollback()).not.toThrow();
  });

  it("rejects a structurally invalid order at the start stage", () => {
    const engine = Engine.builder()

      .builtin(buildOrderValidation())
      .build();

    // An empty order fails order-validation structural checks.
    const start = engine.startPreTrade(new Order());
    expect(start.ok).toBe(false);
    expect(start.request).toBeUndefined();
    expect(start.rejects.length).toBeGreaterThan(0);
    expect(typeof start.rejects[0]!.code).toBe("string");
  });
});

describe("single-use lifecycle guards", () => {
  it("keeps repeated request property reads on one lifecycle", () => {
    const engine = Engine.builder()

      .builtin(buildOrderValidation())
      .build();

    const start = engine.startPreTrade(makeOrder());
    const first = start.request!;
    const second = start.request!;
    const execute = first.execute();
    expect(() => second.execute()).toThrowError(/already been executed/);
    execute.reservation!.rollback();
  });

  it("keeps repeated reservation property reads on one lifecycle", () => {
    const engine = Engine.builder()

      .builtin(buildOrderValidation())
      .build();

    const execute = engine.executePreTrade(makeOrder());
    const first = execute.reservation!;
    const second = execute.reservation!;
    first.commit();
    expect(() => second.rollback()).toThrowError(/already been finalized/);
  });

  it("throws when a request is executed twice", () => {
    const engine = Engine.builder()

      .builtin(buildOrderValidation())
      .build();

    const request = engine.startPreTrade(makeOrder()).request!;
    request.execute();
    expect(() => request.execute()).toThrowError(/already been executed/);
  });

  it("throws when a reservation is committed twice", () => {
    const engine = Engine.builder()

      .builtin(buildOrderValidation())
      .build();

    const reservation = engine.executePreTrade(makeOrder()).reservation!;
    reservation.commit();
    expect(() => reservation.commit()).toThrowError(/already been finalized/);
  });

  it("throws when a reservation is committed after rollback", () => {
    const engine = Engine.builder()

      .builtin(buildOrderValidation())
      .build();

    const reservation = engine.executePreTrade(makeOrder()).reservation!;
    reservation.rollback();
    expect(() => reservation.commit()).toThrowError(/already been finalized/);
  });
});

describe("applyExecutionReport", () => {
  it("applies a post-trade report and returns an aggregated result", () => {
    const engine = Engine.builder()

      .builtin(buildOrderValidation())
      .build();

    const report = new ExecutionReport();
    const operation = new ExecutionReportOperation();
    operation.underlyingAsset = "AAPL";
    operation.settlementAsset = "USD";
    operation.accountId = AccountId.fromInt(99224416n);
    operation.side = "BUY";
    report.operation = operation;
    report.financialImpact = new FinancialImpact(
      Pnl.fromString("-50"),
      Fee.fromString("3.4"),
    );

    const result = engine.applyExecutionReport(report);
    // No kill-switch policy is registered, so no account blocks fire.
    expect(result.accountBlocks).toHaveLength(0);
    expect(Array.isArray(result.accountAdjustments)).toBe(true);
  });
});
