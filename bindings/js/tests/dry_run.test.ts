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
// - https://wiki.openpit.dev/Non-Mutating-Dry-Run/

import { describe, expect, it } from "vitest";

import { Engine } from "@openpit/engine";
import { AdjustmentAmount, TradeAmount } from "@openpit/engine/param";
import { type OrderInit } from "@openpit/engine/model";
import { type Policy } from "@openpit/engine/pretrade";
import { RejectCode, RuleMode } from "@openpit/engine/reject";
import {
  buildOrderValidation,
  buildRateLimit,
  buildSpotFunds,
  RateLimit,
  RateLimitBrokerBarrier,
  RateLimitBuilder,
  SpotFundsBuilder,
  SpotFundsLimitMode,
  SpotFundsPnlBoundsBarrier,
} from "@openpit/engine/pretrade/policies";

import { holdings } from "./spot_funds_holdings";

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

function makeRateLimitEngine(maxOrders: number = 1, mode?: RuleMode): Engine {
  return Engine.builder()

    .builtin(
      buildRateLimit().brokerBarriers([
        new RateLimitBrokerBarrier(new RateLimit(maxOrders, 60_000), mode),
      ]),
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
    // Source: https://wiki.openpit.dev/Non-Mutating-Dry-Run/
    // - Read the Dry-Run Verdict
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
    if (report.reviews.length > 0) {
      console.log("order would require a human decision before sending");
    } else if (report.isPass) {
      console.log("order had no rejects or review marks in the simulation");
    } else {
      for (const reject of report.rejects) {
        console.log(
          `would reject by ${reject.policy} [${reject.code}]: ${reject.reason}: ${reject.details}`,
        );
      }
    }

    expect(report.isPass).toBe(true);
    expect(report.rejects).toEqual([]);
    expect(report.reviews).toEqual([]);
    expect(report.accountBlock).toBeUndefined();
  });

  it("reports a start-stage mark in both dry-runs", () => {
    const engine = Engine.builder()
      .preTrade({
        name: "dry-run-review",
        checkPreTradeStart: () => [],
        checkPreTradeStartDryRun: () => [
          {
            code: "Custom",
            reason: "start-stage review",
            details: "",
            mode: RuleMode.ReviewWithoutHold,
          },
        ],
        performPreTradeCheck: () => null,
        performPreTradeCheckDryRun: () => {
          throw new Error(
            "start-stage without-hold review skips the main stage",
          );
        },
      })
      .build();
    const input = makeOrder();
    for (const report of [
      engine.startPreTradeDryRun(input),
      engine.executePreTradeDryRun(input),
    ]) {
      expect(report.isPass).toBe(true);
      expect(report.rejects).toEqual([]);
      expect(report.reviews.map((review) => review.mode)).toEqual([
        RuleMode.ReviewWithoutHold,
      ]);
    }
  });

  it.each(
    [
      RuleMode.Reject,
      RuleMode.ReviewWithHold,
      RuleMode.ReviewWithoutHold,
    ].flatMap((mode) =>
      [false, true].map((startStage) => ({ mode, startStage })),
    ),
  )(
    "predicts no account block from review marks, mode=$mode, startStage=$startStage",
    ({ mode, startStage }) => {
      const entry = {
        code: "Custom" as const,
        reason: "account needs review",
        details: "dry run",
        scope: "account" as const,
      };
      const engine = Engine.builder()
        .preTrade(
          {
            name: "account-dry-review",
            checkPreTradeStart: () => (startStage ? [entry] : []),
            performPreTradeCheck: () => ({
              rejects: startStage ? [] : [entry],
            }),
          },
          { name: "review", mode },
        )
        .build();
      const reports = [engine.executePreTradeDryRun(makeOrder())];
      if (startStage) reports.push(engine.startPreTradeDryRun(makeOrder()));
      for (const report of reports) {
        expect(report.isPass).toBe(mode !== RuleMode.Reject);
        if (mode === RuleMode.Reject) {
          expect(report.reviews).toEqual([]);
          expect(report.accountBlock).toMatchObject({ code: "AccountBlocked" });
          expect(report.rejects).toHaveLength(1);
        } else {
          expect(report.rejects).toEqual([]);
          expect(report.accountBlock).toBeUndefined();
          expect(
            report.reviews.map(({ policy, scope, mode }) => ({
              policy,
              scope,
              mode,
            })),
          ).toEqual([{ policy: "account-dry-review", scope: "account", mode }]);
        }
      }
    },
  );

  it("passes and rejects without mutating the engine", () => {
    const engine = makeValidationEngine();

    const pass = engine.executePreTradeDryRun(makeOrder());
    expect(pass.isPass).toBe(true);

    const fail = engine.executePreTradeDryRun({ operation: {} });
    expect(fail.isPass).toBe(false);
    expect(fail.rejects.length).toBeGreaterThan(0);
  });

  it("uses the dry run before a real call", () => {
    // Source: https://wiki.openpit.dev/Non-Mutating-Dry-Run/
    // - Use the Dry-Run Before a Real Call
    const engine = Engine.builder()
      .builtin(
        buildRateLimit().brokerBarriers([
          new RateLimitBrokerBarrier(new RateLimit(1, 60_000)),
        ]),
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

  it.each([
    RuleMode.Reject,
    RuleMode.ReviewWithHold,
    RuleMode.ReviewWithoutHold,
  ])(
    "leaves the %s rate-limit budget untouched until a real call happens",
    (mode) => {
      const engine = makeRateLimitEngine(1, mode);

      for (let attempt = 0; attempt < 5; attempt += 1) {
        const dry = engine.executePreTradeDryRun(makeOrder());
        expect(dry.isPass).toBe(true);
        expect(dry.reviews).toEqual([]);
      }

      const real = engine.executePreTrade(makeOrder());
      expect(real.ok).toBe(true);
      expect(real.reviews).toEqual([]);
      real.reservation!.rollback();

      const probe = engine.executePreTradeDryRun(makeOrder());
      const exceeded = {
        mode,
        code: "RateLimitExceeded",
      };
      if (mode === RuleMode.Reject) {
        expect(probe.isPass).toBe(false);
        expect(probe.rejects).toMatchObject([exceeded]);
      } else {
        expect(probe.isPass).toBe(true);
        expect(probe.rejects).toEqual([]);
        expect(probe.reviews).toMatchObject([exceeded]);
        engine.configure().rateLimit(RateLimitBuilder.NAME, {
          brokerBarriers: [
            new RateLimitBrokerBarrier(new RateLimit(2, 60_000), mode),
          ],
        });
        const following = engine.startPreTrade(makeOrder());
        expect(following.ok).toBe(true);
        expect(following.reviews).toEqual([]);
        const breached = engine.startPreTrade(makeOrder());
        expect(breached.ok).toBe(true);
        expect(breached.reviews).toMatchObject([exceeded]);
      }
    },
  );

  it("uses a custom read-only start hook without running normal side effects", () => {
    // Source: https://wiki.openpit.dev/Non-Mutating-Dry-Run/
    // - Read-Only Custom Start-Stage Hook
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

describe("Spot Funds review with an existing hold", () => {
  it("passes without reserving funds again", () => {
    const engine = makeSpotFundsEngine();
    engine.configure().spotFunds(SpotFundsBuilder.NAME, {
      globalLimitMode: SpotFundsLimitMode.ReviewWithHold,
    });
    const order = makeOrder("12", "10000");
    const initial = engine.executePreTrade(order);
    expect(initial.ok).toBe(true);
    expect(initial.reviews).toHaveLength(1);
    expect(initial.reviews[0]).toMatchObject({
      code: RejectCode.InsufficientFunds,
      mode: RuleMode.ReviewWithHold,
    });
    const initialReservation = initial.reservation;
    if (initialReservation === undefined) {
      throw new Error("expected the initial reservation");
    }
    const lock = initialReservation.lock();
    expect(lock.pricesOf(0).map(String)).toEqual(["10000"]);
    initialReservation.commit();
    const before = holdings(engine, ACCOUNT, "USD");
    expect(before).toEqual({
      balance: "-20000",
      held: "120000",
      incoming: "0",
    });
    const incoming = holdings(engine, ACCOUNT, "AAPL");
    expect(incoming).toEqual({ balance: "0", held: "0", incoming: "12" });

    order.review = { confirmed: true, alreadyHeld: true };
    const held = engine.executePreTrade(order);
    expect(held.ok, "expected the already-held order to pass").toBe(true);
    const heldReservation = held.reservation;
    if (heldReservation === undefined) {
      throw new Error("expected the already-held order's reservation");
    }
    expect(held.reviews).toEqual([]);
    expect(heldReservation.lock().size()).toBe(0);
    expect(heldReservation.accountAdjustments()).toEqual([]);
    heldReservation.commit();
    expect(holdings(engine, ACCOUNT, "USD")).toEqual(before);
    expect(holdings(engine, ACCOUNT, "AAPL")).toEqual(incoming);

    const cancel = engine.applyExecutionReport({
      operation: order.operation!,
      fill: { remainingReservedQuantity: "12", isFinal: true, lock },
    });
    expect(cancel.accountBlocks).toEqual([]);
    expect(holdings(engine, ACCOUNT, "USD")).toEqual({
      balance: "100000",
      held: "0",
      incoming: "0",
    });
    expect(holdings(engine, ACCOUNT, "AAPL")).toEqual({
      balance: "0",
      held: "0",
      incoming: "0",
    });
  });

  it.each(["funds", "P&L"] as const)(
    "keeps the first hold when a confirmed rerun breaches Reject %s",
    (cause) => {
      const engine = makeSpotFundsEngine();
      engine.accounts().setCurrency(ACCOUNT, "USD");
      const order = makeOrder("6", "10000");
      const first = engine.executePreTrade(order);
      expect(first.ok).toBe(true);
      const lock = first.reservation!.lock();
      first.reservation!.commit();
      expect(holdings(engine, ACCOUNT, "USD")).toEqual({
        balance: "40000",
        held: "60000",
        incoming: "0",
      });
      if (cause === "funds") {
        order.review = { confirmed: true };
        const unheld = engine.executePreTrade(order);
        expect(unheld.reservation).toBeUndefined();
        expect(unheld.rejects).toHaveLength(1);
        expect(unheld.rejects[0]).toMatchObject({
          code: RejectCode.InsufficientFunds,
          mode: RuleMode.Reject,
        });
        order.review = { confirmed: true, alreadyHeld: true };
        const held = engine.executePreTrade(order);
        expect(held.ok).toBe(true);
        expect(held.reviews).toEqual([]);
        expect(held.reservation!.lock().size()).toBe(0);
        expect(held.reservation!.accountAdjustments()).toEqual([]);
        held.reservation!.commit();
        expect(holdings(engine, ACCOUNT, "USD")).toEqual({
          balance: "40000",
          held: "60000",
          incoming: "0",
        });
        const remaining = engine.executePreTrade(makeOrder("4", "10000"));
        expect(remaining.ok).toBe(true);
        remaining.reservation!.rollback();
        expect(
          engine.applyAccountAdjustment(ACCOUNT, [
            {
              operation: { asset: "USD" },
              amount: { balance: AdjustmentAmount.delta("-50000") },
            },
          ]).ok,
        ).toBe(true);
      } else {
        engine.configure().spotFundsPnlBoundsKillswitch(SpotFundsBuilder.NAME, {
          globalBarriers: [
            new SpotFundsPnlBoundsBarrier("USD", "-10", undefined),
          ],
        });
        const pnl = engine
          .configure()
          .setSpotFundsAccountPnl(SpotFundsBuilder.NAME, {
            account: ACCOUNT,
            state: "-20",
          });
        expect(pnl.accountBlocks).toHaveLength(1);
        expect(pnl.accountBlocks[0]!.code).toBe(
          RejectCode.PnlKillSwitchTriggered,
        );
        engine.accounts().unblock(ACCOUNT);
      }
      const before = holdings(engine, ACCOUNT, "USD");
      expect(before).toEqual({
        balance: cause === "funds" ? "-10000" : "40000",
        held: "60000",
        incoming: "0",
      });
      order.review = { confirmed: true, alreadyHeld: true };
      const result = engine.executePreTrade(order);
      expect(result.ok).toBe(false);
      expect(result.reservation).toBeUndefined();
      expect(result.rejects).toHaveLength(1);
      expect(result.rejects[0]).toMatchObject({
        code:
          cause === "funds"
            ? RejectCode.InsufficientFunds
            : RejectCode.PnlKillSwitchTriggered,
        mode: RuleMode.Reject,
      });
      expect(holdings(engine, ACCOUNT, "USD")).toEqual(before);
      engine.applyExecutionReport({
        operation: order.operation!,
        fill: { remainingReservedQuantity: "6", isFinal: true, lock },
      });
      expect(holdings(engine, ACCOUNT, "USD")).toEqual({
        balance: cause === "funds" ? "50000" : "100000",
        held: "0",
        incoming: "0",
      });
    },
  );

  it("blocks a cancel with the empty rerun lock without releasing USD", () => {
    const engine = makeSpotFundsEngine();
    engine.configure().spotFunds(SpotFundsBuilder.NAME, {
      globalLimitMode: SpotFundsLimitMode.ReviewWithHold,
    });
    const order = makeOrder("12", "10000");
    const first = engine.executePreTrade(order);
    expect(first.ok).toBe(true);
    first.reservation!.commit();
    order.review = { confirmed: true, alreadyHeld: true };
    const rerun = engine.executePreTrade(order);
    expect(rerun.ok).toBe(true);
    expect(rerun.reviews).toEqual([]);
    const lock = rerun.reservation!.lock();
    expect(lock.size()).toBe(0);
    expect(rerun.reservation!.accountAdjustments()).toEqual([]);
    rerun.reservation!.commit();
    const before = holdings(engine, ACCOUNT, "USD");
    expect(before).toEqual({
      balance: "-20000",
      held: "120000",
      incoming: "0",
    });
    const cancel = engine.applyExecutionReport({
      operation: order.operation!,
      fill: { remainingReservedQuantity: "12", isFinal: true, lock },
    });
    expect(cancel.accountBlocks).toHaveLength(1);
    expect(cancel.accountBlocks[0]!.code).toBe(RejectCode.MissingRequiredField);
    expect(holdings(engine, ACCOUNT, "USD")).toEqual(before);
  });

  it("omits Spot Funds results on an already-held rerun with breached review rules", () => {
    const engine = makeSpotFundsEngine();
    engine.accounts().setCurrency(ACCOUNT, "USD");
    const order = makeOrder("6", "10000");
    const first = engine.executePreTrade(order);
    expect(first.ok).toBe(true);
    first.reservation!.commit();
    engine.configure().spotFunds(SpotFundsBuilder.NAME, {
      globalLimitMode: SpotFundsLimitMode.TrackOnly,
      positionLimits: [
        {
          accountId: ACCOUNT,
          asset: "AAPL",
          limit: "0",
          mode: RuleMode.ReviewWithoutHold,
        },
      ],
    });
    engine.configure().spotFundsPnlBoundsKillswitch(SpotFundsBuilder.NAME, {
      globalBarriers: [
        new SpotFundsPnlBoundsBarrier(
          "USD",
          "-10",
          undefined,
          RuleMode.ReviewWithHold,
        ),
      ],
    });
    expect(
      engine.configure().setSpotFundsAccountPnl(SpotFundsBuilder.NAME, {
        account: ACCOUNT,
        state: "-20",
      }).accountBlocks,
    ).toEqual([]);
    const unflagged = engine.executePreTradeDryRun(order);
    expect(unflagged.isPass).toBe(true);
    expect(unflagged.reviews).toHaveLength(2);
    expect(unflagged.reviews).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          code: RejectCode.PnlKillSwitchTriggered,
          mode: RuleMode.ReviewWithHold,
        }),
        expect.objectContaining({
          code: RejectCode.PositionLimitExceeded,
          mode: RuleMode.ReviewWithoutHold,
        }),
      ]),
    );
    const before = [
      holdings(engine, ACCOUNT, "USD"),
      holdings(engine, ACCOUNT, "AAPL"),
    ];
    expect(before).toEqual([
      { balance: "40000", held: "60000", incoming: "0" },
      { balance: "0", held: "0", incoming: "6" },
    ]);
    order.review = { confirmed: true, alreadyHeld: true };
    const rerun = engine.executePreTrade(order);
    expect(rerun.ok).toBe(true);
    expect(rerun.reviews).toEqual([]);
    expect(rerun.reservation!.lock().size()).toBe(0);
    expect(rerun.reservation!.accountAdjustments()).toEqual([]);
    rerun.reservation!.commit();
    expect([
      holdings(engine, ACCOUNT, "USD"),
      holdings(engine, ACCOUNT, "AAPL"),
    ]).toEqual(before);
  });
});
