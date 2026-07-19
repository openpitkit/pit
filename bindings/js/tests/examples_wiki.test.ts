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
// Mirrors the public JS examples from the project wiki. Each test embeds the
// body of one wiki ```ts snippet verbatim (the first line of every test is a
// `// Source:` comment naming the page and section), wrapped with the imports,
// harness, and assertions that prove the documented outcome. Per the doc-mirror
// rule in doc/code_style.md, the snippet body here and the wiki snippet are one
// example: any forced edit to one must be mirrored in the other.
//
// Wiki pages mirrored here:
// - ../../../../pit.wiki/Domain-Types.md
// - ../../../../pit.wiki/Pre-trade-Pipeline.md
// - ../../../../pit.wiki/Getting-Started.md
// - ../../../../pit.wiki/Account-Adjustments.md
// - ../../../../pit.wiki/Account-Groups.md
// - ../../../../pit.wiki/Account-Blocking.md
// - ../../../../pit.wiki/Balance-Reconciliation.md
// - ../../../../pit.wiki/Pre-Trade-Lock.md
// - ../../../../pit.wiki/Policies.md
// - ../../../../pit.wiki/Spot-Funds.md
// - ../../../../pit.wiki/Policy-API.md
//
// See engine.test.ts for the import-resolution scheme. Run `npm run build`
// first. The import block of each snippet is hoisted to this file header (TS
// forbids in-body imports); everything after the imports is the verbatim body.

import { describe, expect, it } from "vitest";

import { Engine } from "@openpit/engine";
import {
  type AccountId,
  AdjustmentAmount,
  Instrument,
  Leverage,
  Pnl,
  PositionSize,
  PositionSide,
  Price,
  Quantity,
  Side,
  TradeAmount,
  Volume,
} from "@openpit/engine/param";
import {
  type AccountAdjustment,
  type AccountAdjustmentInit,
  ExecutionReport,
  type ExecutionReportInit,
  Order,
  type OrderInit,
} from "@openpit/engine/model";
import {
  type Context,
  Lock,
  type Policy,
  type PolicyAccountAdjustmentResult,
  type PolicyPreTradeResult,
  type PolicyReject,
} from "@openpit/engine/pretrade";
import {
  buildOrderSizeLimit,
  buildOrderValidation,
  buildPnlBoundsKillswitch,
  buildRateLimit,
  buildSpotFunds,
  OrderSizeAssetBarrier,
  OrderSizeBrokerBarrier,
  OrderSizeLimit,
  PnlBoundsBrokerBarrier,
  RateLimit,
  RateLimitBrokerBarrier,
} from "@openpit/engine/pretrade/policies";
import { Quote, QuoteTtl } from "@openpit/engine/marketdata";
import { AccountBlock } from "@openpit/engine/reject";
import { type AccountAdjustmentContext } from "@openpit/engine/accountadjustment";

// A structurally valid AAPL/USD buy used by the pipeline/getting-started
// snippets that take `order` from surrounding context in the wiki prose. Built
// fresh per call only to keep each test independent; runtime coercion is
// non-consuming, so callers may safely replay the same immutable init object.
function sharedOrder(): OrderInit {
  return {
    operation: {
      underlyingAsset: "AAPL",
      settlementAsset: "USD",
      accountId: 99224416,
      side: "BUY",
      tradeAmount: TradeAmount.quantity("100"),
      price: "185.00",
    },
  };
}

describe("Domain-Types.md wiki examples", () => {
  it("creates validated value objects", () => {
    // Source: Domain-Types.md - Create Validated Values
    // Build validated value objects at the integration boundary. Assets cross the
    // boundary as plain strings, so there is no asset wrapper to construct.
    const asset = "AAPL";
    const quantity = Quantity.fromString("10.5");
    const price = Price.fromString("185");
    const pnl = Pnl.fromString("-12.5");

    // The wrappers normalize formatting while preserving domain meaning, and
    // serialize back to a lossless decimal string - never a raw number.
    console.log(asset, quantity.toString(), price.toString(), pnl.toString());
    // => AAPL 10.5 185 -12.5

    expect(asset).toBe("AAPL");
    expect(quantity.toString()).toBe("10.5");
    expect(price.toString()).toBe("185");
    expect(pnl.toString()).toBe("-12.5");
  });

  it("works with directional types", () => {
    // Source: Domain-Types.md - Work With Directional Types
    // Directional helpers keep side logic explicit instead of comparing raw strings.
    const side = Side.buy();
    const positionSide = PositionSide.long();

    console.log(side.opposite().toString(), side.sign()); // => SELL 1
    console.log(positionSide.opposite().toString()); // => SHORT

    expect(side.opposite().toString()).toBe("SELL");
    expect(side.sign()).toBe(1);
    expect(positionSide.opposite().toString()).toBe("SHORT");
  });

  it("creates leverage from either representation", () => {
    // Source: Domain-Types.md - Create Leverage
    // Pick the constructor that matches the upstream representation you receive.
    const fromMultiplier = Leverage.fromInt(100);
    const fromFloat = Leverage.fromFloat(100.5);

    // Both constructors end up with the same strongly typed leverage wrapper.
    console.log(fromMultiplier.value, fromFloat.value); // => 100 100.5

    expect(fromMultiplier.value).toBe(100.0);
    expect(fromFloat.value).toBe(100.5);
  });
});

describe("Pre-trade-Pipeline.md wiki examples", () => {
  it("handles a start-stage reject", () => {
    // Source: Pre-trade-Pipeline.md - Handle a Start-Stage Reject
    const engine = Engine.builder().builtin(buildOrderValidation()).build();
    const order: OrderInit = {
      operation: {
        underlyingAsset: "AAPL",
        settlementAsset: "USD",
        accountId: 99224416,
        side: "BUY",
        tradeAmount: TradeAmount.quantity("100"),
        price: "185",
      },
    };

    // Start stage returns either rejects or a deferred request handle.
    const start = engine.startPreTrade(order);
    if (!start.ok) {
      for (const reject of start.rejects) {
        console.log(
          `rejected by ${reject.policy} [${reject.code}]: ${reject.reason}: ${reject.details}`,
        );
      }
    } else {
      // Keep the request object if later code wants to enter the main stage.
      const request = start.request;
      if (request === undefined) {
        throw new Error("accepted start result is missing its request");
      }
      void request;
    }

    // The documented order is structurally valid, so the start stage accepts it
    // and takes the else branch above.
    expect(start.ok).toBe(true);
  });

  it("executes the main stage and finalizes the reservation", () => {
    // Source: Pre-trade-Pipeline.md - Execute the Main Stage and Finalize the Reservation
    const engine = Engine.builder().builtin(buildOrderValidation()).build();
    const order: OrderInit = {
      operation: {
        underlyingAsset: "AAPL",
        settlementAsset: "USD",
        accountId: 99224416,
        side: "BUY",
        tradeAmount: TradeAmount.quantity("100"),
        price: "185",
      },
    };
    const start = engine.startPreTrade(order);
    const request = start.request;
    if (request === undefined) {
      throw new Error("start stage must accept the order");
    }

    // Main stage consumes the deferred request and returns a reservation or
    // rejects.
    const execute = request.execute();

    if (execute.ok) {
      // Commit only after the caller knows the reservation should become durable.
      const reservation = execute.reservation;
      if (reservation === undefined) {
        throw new Error("accepted execute result is missing its reservation");
      }
      reservation.commit();
    } else {
      for (const reject of execute.rejects) {
        console.log(
          `rejected by ${reject.policy} [${reject.code}]: ${reject.reason}: ${reject.details}`,
        );
      }
    }

    expect(execute.ok).toBe(true);
  });

  it("runs the start + main shortcut", () => {
    // Source: Pre-trade-Pipeline.md - Shortcut for Start + Main Stages
    const engine = Engine.builder().builtin(buildOrderValidation()).build();
    const order: OrderInit = {
      operation: {
        underlyingAsset: "AAPL",
        settlementAsset: "USD",
        accountId: 99224416,
        side: "BUY",
        tradeAmount: TradeAmount.quantity("100"),
        price: "185",
      },
    };

    // The shortcut runs start stage and main stage as one convenience call.
    const execute = engine.executePreTrade(order);
    if (execute.ok) {
      // Finalization is still explicit even when the two stages are composed.
      const reservation = execute.reservation;
      if (reservation === undefined) {
        throw new Error("accepted execute result is missing its reservation");
      }
      reservation.commit();
    } else {
      for (const reject of execute.rejects) {
        console.log(
          `rejected by ${reject.policy} [${reject.code}]: ${reject.reason}: ${reject.details}`,
        );
      }
    }

    expect(execute.ok).toBe(true);
  });

  it("applies post-trade feedback", () => {
    // Source: Pre-trade-Pipeline.md - Apply Post-Trade Feedback
    const engine = Engine.builder().builtin(buildOrderValidation()).build();
    const report: ExecutionReportInit = {
      operation: {
        underlyingAsset: "AAPL",
        settlementAsset: "USD",
        accountId: 99224416,
        side: "BUY",
      },
      financialImpact: { pnl: "-50", fee: "3.4" },
    };

    // Execution reports feed realized outcomes back into cumulative policy state.
    const result = engine.applyExecutionReport(report);
    for (const outcome of result.accountPnls) {
      console.log(`account P&L outcome for ${outcome.accountId.toString()}`);
    }
    for (const outcome of result.accountAdjustments) {
      console.log(`account adjustment from group ${outcome.policyGroupId}`);
    }
    if (result.accountBlocks.length > 0) {
      console.log("halt new orders until the blocked state is cleared");
    }

    expect(result.accountBlocks).toHaveLength(0);
  });
});

describe("Getting-Started.md wiki examples", () => {
  it("builds an engine and runs the end-to-end flow", () => {
    // Source: Getting-Started.md - Build an Engine
    // 1. Build the engine (one time at the platform initialization). The WASM
    // engine is single-threaded and has no user-selectable sync mode. The first
    // builtin() advances the staged builder to the ready builder; the rest register
    // in place.
    const ready = Engine.builder().builtin(buildOrderValidation());

    ready.builtin(
      buildPnlBoundsKillswitch().brokerBarriers([
        new PnlBoundsBrokerBarrier("USD", "-1000", undefined),
      ]),
    );

    ready.builtin(
      buildRateLimit().brokerBarrier(
        new RateLimitBrokerBarrier(new RateLimit(100, 1000)),
      ),
    );

    ready.builtin(
      buildOrderSizeLimit()
        .brokerBarrier(
          new OrderSizeBrokerBarrier(new OrderSizeLimit("500", "100000")),
        )
        .assetBarriers([
          new OrderSizeAssetBarrier(new OrderSizeLimit("500", "100000"), "USD"),
        ]),
    );

    const engine = ready.build();

    // 2. Check an order. Scalars accept plain values (the account id as a number,
    // the price as a decimal string); the order is an object literal.
    const order: OrderInit = {
      operation: {
        underlyingAsset: "AAPL",
        settlementAsset: "USD",
        accountId: 99224416,
        side: "BUY",
        tradeAmount: TradeAmount.quantity("100"),
        price: "185",
      },
    };

    const start = engine.startPreTrade(order);
    if (!start.ok) {
      const reasons = start.rejects
        .map((r) => `${r.policy} [${r.code}]: ${r.reason} (${r.details})`)
        .join(", ");
      throw new Error(reasons);
    }

    // 3. Quick, lightweight checks were performed during the start stage. The
    // system state has not yet changed. Before the heavy-duty checks, other work on
    // the request can be performed by holding the request object.

    // 4. Real pre-trade and risk control.
    const request = start.request;
    if (request === undefined) {
      throw new Error("accepted start result is missing its request");
    }
    const execute = request.execute();
    if (!execute.ok) {
      const reasons = execute.rejects
        .map((r) => `${r.policy} [${r.code}]: ${r.reason} (${r.details})`)
        .join(", ");
      throw new Error(reasons);
    }

    // Optional shortcut for the same two-stage flow:
    // const execute = engine.executePreTrade(order);

    // 5. If the request is successfully sent to the venue, commit; roll back
    // otherwise to revert all performed reservations.
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

    // 6. The order goes to the venue and returns with an execution report.
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
    for (const outcome of result.accountPnls) {
      console.log(`account P&L outcome for ${outcome.accountId.toString()}`);
    }
    for (const outcome of result.accountAdjustments) {
      console.log(`account adjustment from group ${outcome.policyGroupId}`);
    }

    // 7. A non-empty accountBlocks means a kill switch has fired for the account.
    if (result.accountBlocks.length > 0) {
      console.log("halt new orders until the blocked state is cleared");
    }

    expect(start.ok).toBe(true);
    expect(execute.ok).toBe(true);
    expect(result.accountBlocks).toHaveLength(0);
  });

  it("runs the start + main shortcut", () => {
    // Source: Getting-Started.md - Shortcut for Start + Main Stages
    const engine = Engine.builder().builtin(buildOrderValidation()).build();
    const order: OrderInit = {
      operation: {
        underlyingAsset: "AAPL",
        settlementAsset: "USD",
        accountId: 99224416,
        side: "BUY",
        tradeAmount: TradeAmount.quantity("100"),
        price: "185",
      },
    };

    // The shortcut runs start stage and main stage as one convenience call.
    const execute = engine.executePreTrade(order);
    if (execute.ok) {
      // Finalization is still explicit even when the two stages are composed.
      const reservation = execute.reservation;
      if (reservation === undefined) {
        throw new Error("accepted execute result is missing its reservation");
      }
      reservation.commit();
    } else {
      for (const reject of execute.rejects) {
        console.log(
          `rejected by ${reject.policy} [${reject.code}]: ${reject.reason}: ${reject.details}`,
        );
      }
    }

    expect(execute.ok).toBe(true);
  });

  it("runs an order through the engine", () => {
    // Source: Getting-Started.md - Run an Order Through the Engine
    const engine = Engine.builder().builtin(buildOrderValidation()).build();
    const order: OrderInit = {
      operation: {
        underlyingAsset: "AAPL",
        settlementAsset: "USD",
        accountId: 99224416,
        side: "BUY",
        tradeAmount: TradeAmount.quantity("100"),
        price: "185",
      },
    };

    const start = engine.startPreTrade(order);
    if (!start.ok) {
      const reasons = start.rejects
        .map((r) => `${r.policy} [${r.code}]: ${r.reason}: ${r.details}`)
        .join(", ");
      throw new Error(reasons);
    }

    const request = start.request;
    if (request === undefined) {
      throw new Error("accepted start result is missing its request");
    }
    const execute = request.execute();
    if (!execute.ok) {
      const reasons = execute.rejects
        .map((r) => `${r.policy} [${r.code}]: ${r.reason}: ${r.details}`)
        .join(", ");
      throw new Error(reasons);
    }

    const reservation = execute.reservation;
    if (reservation === undefined) {
      throw new Error("accepted execute result is missing its reservation");
    }
    reservation.commit();

    expect(start.ok).toBe(true);
    expect(execute.ok).toBe(true);
  });

  it("applies post-trade feedback", () => {
    // Source: Getting-Started.md - Apply Post-Trade Feedback
    const engine = Engine.builder().builtin(buildOrderValidation()).build();
    const report: ExecutionReportInit = {
      operation: {
        underlyingAsset: "AAPL",
        settlementAsset: "USD",
        accountId: 99224416,
        side: "BUY",
      },
      financialImpact: { pnl: "-50", fee: "3.4" },
    };

    // Execution reports feed realized outcomes back into cumulative policy state.
    const result = engine.applyExecutionReport(report);
    for (const outcome of result.accountPnls) {
      console.log(`account P&L outcome for ${outcome.accountId.toString()}`);
    }
    for (const outcome of result.accountAdjustments) {
      console.log(`account adjustment from group ${outcome.policyGroupId}`);
    }
    if (result.accountBlocks.length > 0) {
      console.log("halt new orders until the blocked state is cleared");
    }

    expect(result.accountBlocks).toHaveLength(0);
  });
});

describe("Account-Adjustments.md wiki examples", () => {
  it("applies a mixed balance + position batch atomically", () => {
    // Source: Account-Adjustments.md - Examples
    // Build one batch that mixes balance and position adjustments. Each adjustment
    // is a plain object literal; position sizes cross the boundary as decimal
    // strings.
    const accountId = 99224416;

    const adjustments: AccountAdjustmentInit[] = [
      {
        operation: { asset: "USD" },
        amount: { balance: AdjustmentAmount.absolute("10000") },
      },
      {
        operation: {
          underlyingAsset: "SPX",
          settlementAsset: "USD",
          collateralAsset: "USD",
          averageEntryPrice: "95000",
          mode: "hedged",
        },
        amount: { balance: AdjustmentAmount.absolute("-3") },
      },
    ];

    // The engine validates the whole batch atomically.
    const engine = Engine.builder()

      .builtin(buildOrderValidation())
      .build();
    const result = engine.applyAccountAdjustment(accountId, adjustments);
    // result.ok is true: the whole batch was accepted.

    expect(result.ok).toBe(true);
    expect(result.accountBlocks).toHaveLength(0);
  });

  it("drives a balance-limit policy from the adjustment path", () => {
    // Source: Account-Adjustments.md - Example: Balance Limit Policy
    // Tracks cumulative totals per asset, rejects the batch on a limit breach.
    class CumulativeLimitPolicy implements Policy {
      readonly name = "CumulativeLimitPolicy";
      private readonly totals = new Map<string, PositionSize>();

      constructor(private readonly maxCumulative: PositionSize) {}

      // The pre-trade hooks are required by the Policy interface; this policy only
      // acts on the account-adjustment path, so they accept with no contribution.
      checkPreTradeStart(): Iterable<PolicyReject> {
        return [];
      }

      performPreTradeCheck(): PolicyPreTradeResult {
        return {};
      }

      applyAccountAdjustment(
        _ctx: AccountAdjustmentContext,
        _accountId: AccountId,
        adjustment: AccountAdjustment,
      ): PolicyAccountAdjustmentResult {
        // Use the asset as the aggregation key for the cumulative limit.
        const operation = adjustment.operation;
        const assetId =
          operation !== undefined && "asset" in operation
            ? (operation.asset ?? "")
            : "";

        const previous = this.totals.get(assetId);
        const current = previous ?? PositionSize.fromInt(0n);
        const balance = adjustment.amount?.balance;
        if (balance === undefined) {
          return { accountBlocks: [] };
        }

        const absolute = balance.asAbsolute;
        let newTotal: PositionSize;
        if (absolute !== undefined) {
          newTotal = absolute;
        } else {
          const delta = balance.asDelta;
          if (delta === undefined) {
            return { accountBlocks: [] };
          }
          newTotal = current.add(delta);
        }

        // Reject if the limit is breached.
        if (newTotal.compare(this.maxCumulative) > 0) {
          return {
            rejects: [
              {
                code: "RiskLimitExceeded",
                reason: "cumulative limit exceeded",
                details: `${assetId}: ${newTotal.toString()} > ${this.maxCumulative.toString()}`,
                scope: "account",
              },
            ],
            accountBlocks: [],
          };
        }

        // Apply immediately so later adjustments in the same batch see the updated
        // total.
        this.totals.set(assetId, newTotal);

        // Rollback by absolute value - safe in the account-adjustment pipeline
        // because no external system sees intermediate batch state. Commit is empty:
        // the state was applied eagerly.
        return {
          mutations: [
            {
              commit: () => {},
              rollback: () => {
                if (previous === undefined) {
                  this.totals.delete(assetId);
                } else {
                  this.totals.set(assetId, previous);
                }
              },
            },
          ],
          accountBlocks: [],
        };
      }
    }

    // Seed an absolute value, then reject a batch after its first delta was
    // applied. A final delta reaches the limit exactly only if the failed batch
    // rolled its first mutation back.
    const engine = Engine.builder()
      .preTrade(new CumulativeLimitPolicy(PositionSize.fromString("100")))
      .build();
    const accountId = 99224416;
    const adjustment = (
      amount: ReturnType<typeof AdjustmentAmount.absolute>,
    ) => ({
      operation: { asset: "USD" },
      amount: { balance: amount },
    });

    const seed = engine.applyAccountAdjustment(accountId, [
      adjustment(AdjustmentAmount.absolute("40")),
    ]);

    const rejected = engine.applyAccountAdjustment(accountId, [
      adjustment(AdjustmentAmount.delta("30")),
      adjustment(AdjustmentAmount.delta("40")),
    ]);

    const afterRollback = engine.applyAccountAdjustment(accountId, [
      adjustment(AdjustmentAmount.delta("60")),
    ]);

    if (!seed.ok) {
      throw new Error("the absolute seed must be accepted");
    }
    if (
      rejected.ok ||
      rejected.failedIndex !== 1 ||
      rejected.rejects[0]?.code !== "RiskLimitExceeded"
    ) {
      throw new Error("the second delta must breach the cumulative limit");
    }
    if (!afterRollback.ok) {
      throw new Error("the failed batch must roll the first delta back");
    }
  });
});

describe("Account-Groups.md wiki examples", () => {
  it("registers a group and reads membership by id", () => {
    // Source: Account-Groups.md - Examples
    const engine = Engine.builder()

      .builtin(buildOrderValidation())
      .build();

    // Group two accounts under one compact identifier. Account and group ids cross
    // the boundary as plain numbers.
    const accounts = engine.accounts();
    const hedgeBook = 7;
    accounts.registerGroup([10, 11], hedgeBook);

    // Membership is readable by id, without enumerating the accounts.
    console.log(accounts.groupOf(10)?.value); // => 7 (hedgeBook)
    console.log(accounts.groupOf(99)); // => undefined (no group)

    // Removing the group is atomic too: every listed account must be a member.
    accounts.unregisterGroup([10, 11], hedgeBook);
    console.log(accounts.groupOf(10)); // => undefined

    expect(accounts.groupOf(10)).toBeUndefined();
  });
});

describe("Account-Blocking.md wiki examples", () => {
  it("blocks and unblocks accounts and groups", () => {
    // Source: Account-Blocking.md - Examples
    const engine = Engine.builder()

      .builtin(buildOrderValidation())
      .build();

    const accounts = engine.accounts();

    // Block account 99224416 - all subsequent pre-trade orders are rejected.
    accounts.block(99224416, "compliance hold");

    // Unblock account 99224416 - pre-trade orders are allowed again.
    accounts.unblock(99224416);

    // Block every current and future member of a group in one call.
    const desk = 7;
    accounts.blockGroup(desk, "desk suspended");
    accounts.unblockGroup(desk);

    // Harness: after the block/unblock round-trip the account is tradeable again.
    const result = engine.executePreTrade(sharedOrder());
    expect(result.ok).toBe(true);
  });
});

describe("Balance-Reconciliation.md wiki examples", () => {
  it("reports delta versus absolute across two seeds", () => {
    // Source: Balance-Reconciliation.md - Delta Versus Absolute
    const engine = Engine.builder().builtin(buildSpotFunds()).build();
    const accountId = 99224416;

    // An absolute adjustment sets the available USD to a target level. The amount
    // crosses the boundary as a decimal string.
    const seed = (amount: string): AccountAdjustmentInit => ({
      operation: { asset: "USD" },
      amount: { balance: AdjustmentAmount.absolute(amount) },
    });

    // First seed: available USD goes from 0 to 10000.
    const first = engine.applyAccountAdjustment(accountId, [seed("10000")]);
    let usd = first.outcomes[0]!.entry.balance!;
    console.log(usd.delta.toString(), usd.absolute.toString()); // => "10000" "10000"

    // Second seed: available USD goes from 10000 to 15000.
    const second = engine.applyAccountAdjustment(accountId, [seed("15000")]);
    usd = second.outcomes[0]!.entry.balance!;
    // delta is the change to add to your own ledger; absolute is just a snapshot.
    console.log(usd.delta.toString(), usd.absolute.toString()); // => "5000" "15000"

    expect(first.ok).toBe(true);
    expect(first.outcomes[0]!.entry.balance!.delta.toString()).toBe("10000");
    expect(first.outcomes[0]!.entry.balance!.absolute.toString()).toBe("10000");
    expect(second.ok).toBe(true);
    expect(usd.delta.toString()).toBe("5000");
    expect(usd.absolute.toString()).toBe("15000");
  });
});

describe("Pre-Trade-Lock.md wiki examples", () => {
  it("persists and restores a lock across a simulated restart", () => {
    // Source: Pre-Trade-Lock.md - Persisting and Restoring a Lock
    const engine = Engine.builder().builtin(buildSpotFunds()).build();

    const accountId = 99224416;

    // Seed 10000 USD so the buy can be reserved.
    engine.applyAccountAdjustment(accountId, [
      {
        operation: { asset: "USD" },
        amount: { balance: AdjustmentAmount.absolute("10000") },
      },
    ]);

    // Buy 10 AAPL @ 200 holds 2000 USD and records the lock price (200).
    const result = engine.executePreTrade({
      operation: {
        underlyingAsset: "AAPL",
        settlementAsset: "USD",
        accountId,
        side: "BUY",
        tradeAmount: TradeAmount.quantity("10"),
        price: "200",
      },
    });

    // Persist the lock with its built-in JSON serialization before committing.
    if (!result.ok) {
      throw new Error("unexpected rejects");
    }
    const reservation = result.reservation;
    if (reservation === undefined) {
      throw new Error("accepted execute result is missing its reservation");
    }
    const payload = reservation.lock().toJson();
    reservation.commit();

    // --- After a process restart, rebuild the lock from your store. ---
    const restored = Lock.fromJson(payload);

    // The final fill must carry the restored lock so the policy reconciles the
    // 2000 USD it held against the real fill instead of blocking the account.
    const post = engine.applyExecutionReport({
      operation: {
        underlyingAsset: "AAPL",
        settlementAsset: "USD",
        accountId,
        side: "BUY",
      },
      fill: {
        lastTrade: { price: "200", quantity: "10" },
        leavesQuantity: "0",
        lock: restored,
        isFinal: true,
      },
    });
    // post.accountBlocks is empty: the restored lock let the policy reconcile.

    expect(result.ok).toBe(true);
    expect(post.accountBlocks).toHaveLength(0);
  });
});

describe("Policies.md wiki examples", () => {
  it("builds a limit-only SpotFunds engine", () => {
    // Source: Policies.md - SpotFundsPolicy
    // Limit-only spot funds, registered first in the policy list.
    const engine = Engine.builder().builtin(buildSpotFunds()).build();

    expect(engine).toBeDefined();
  });

  it("builds an OrderValidation engine", () => {
    // Source: Policies.md - OrderValidationPolicy
    const engine = Engine.builder()

      .builtin(buildOrderValidation())
      .build();

    const result = engine.executePreTrade(sharedOrder());
    expect(result.ok).toBe(true);
  });

  it("builds a RateLimit engine", () => {
    // Source: Policies.md - RateLimitPolicy
    // windowMs is the rolling-window length in milliseconds (1 second here).
    const engine = Engine.builder()

      .builtin(
        buildRateLimit().brokerBarrier(
          new RateLimitBrokerBarrier(new RateLimit(100, 1000)),
        ),
      )
      .build();

    const result = engine.executePreTrade(sharedOrder());
    expect(result.ok).toBe(true);
  });

  it("builds an OrderSizeLimit engine", () => {
    // Source: Policies.md - OrderSizeLimitPolicy
    // Quantities and notionals cross as decimal strings.
    const engine = Engine.builder()

      .builtin(
        buildOrderSizeLimit()
          .assetBarriers([
            new OrderSizeAssetBarrier(
              new OrderSizeLimit("100", "50000"),
              "USD",
            ),
          ])
          .brokerBarrier(
            new OrderSizeBrokerBarrier(new OrderSizeLimit("100", "50000")),
          ),
      )
      .build();

    const result = engine.executePreTrade(sharedOrder());
    expect(result.ok).toBe(true);
  });

  it("builds a PnlBoundsKillSwitch engine", () => {
    // Source: Policies.md - PnlBoundsKillSwitchPolicy
    // Bounds cross as signed decimal strings; at least one bound must be set.
    const engine = Engine.builder()

      .builtin(
        buildPnlBoundsKillswitch().brokerBarriers([
          new PnlBoundsBrokerBarrier("USD", "-1000", "500"),
        ]),
      )
      .build();

    const result = engine.executePreTrade(sharedOrder());
    expect(result.ok).toBe(true);
  });
});

describe("Spot-Funds.md wiki examples", () => {
  it("reserves against limit-only available funds", () => {
    // Source: Spot-Funds.md - Limit-Only Mode (Default)
    // Limit-only spot funds: register first in the policy list.
    const engine = Engine.builder().builtin(buildSpotFunds()).build();

    const accountId = 99224416;

    // Seed 10000 USD of available funds through the account-adjustment pipeline.
    const seed: AccountAdjustmentInit = {
      operation: { asset: "USD" },
      amount: { balance: AdjustmentAmount.absolute("10000") },
    };
    const seedResult = engine.applyAccountAdjustment(accountId, [seed]);
    if (!seedResult.ok) {
      throw new Error("unexpected rejects");
    }

    // Buy 10 AAPL @ 200 holds 2000 USD; available drops to 8000.
    const order: OrderInit = {
      operation: {
        underlyingAsset: "AAPL",
        settlementAsset: "USD",
        accountId,
        side: "BUY",
        tradeAmount: TradeAmount.quantity("10"),
        price: "200",
      },
    };
    const result = engine.executePreTrade(order);
    if (!result.ok) {
      throw new Error("unexpected post-trade rejects");
    }
    const reservation = result.reservation;
    if (reservation === undefined) {
      throw new Error("accepted execute result is missing its reservation");
    }
    reservation.commit();

    expect(seedResult.ok).toBe(true);
    expect(result.ok).toBe(true);
  });

  it("prices a market buy from the quote mark", () => {
    // Source: Spot-Funds.md - Market Orders
    const builder = Engine.builder();

    // A shared market-data service feeds the policy's market-order pricing.
    const marketData = builder.marketData(QuoteTtl.infinite()).build();
    const aapl = new Instrument("AAPL", "USD");
    const aaplId = marketData.register(aapl);
    marketData.push(aaplId, new Quote({ mark: "200" }));

    // Spot funds with market orders enabled at 1500 bps worst-case slippage,
    // priced from the quote mark.
    const engine = builder
      .builtin(buildSpotFunds().marketData(marketData, 1500, "Mark", undefined))
      .build();

    const accountId = 99224416;
    const seed: AccountAdjustmentInit = {
      operation: { asset: "USD" },
      amount: { balance: AdjustmentAmount.absolute("10000") },
    };
    engine.applyAccountAdjustment(accountId, [seed]);

    // Market buy (no price): priced at mark 200 + 15% = 230 per unit worst case.
    const order: OrderInit = {
      operation: {
        underlyingAsset: "AAPL",
        settlementAsset: "USD",
        accountId,
        side: "BUY",
        tradeAmount: TradeAmount.quantity("5"),
      },
    };
    const result = engine.executePreTrade(order);
    if (!result.ok) {
      throw new Error("unexpected post-trade rejects");
    }
    const reservation = result.reservation;
    if (reservation === undefined) {
      throw new Error("accepted execute result is missing its reservation");
    }
    reservation.commit();

    expect(result.ok).toBe(true);
  });
});

describe("Policy-API.md wiki examples", () => {
  it("preserves application fields in custom-policy callbacks", () => {
    // Source: Policy-API.md - JS Custom Models
    type StrategyOrder = Order & {
      strategyTag: string;
    };

    type StrategyReport = ExecutionReport & {
      venueExecId: string;
    };

    let appliedVenueExecId: string | undefined;
    const strategyTagPolicy: Policy<StrategyOrder, StrategyReport> = {
      name: "StrategyTagPolicy",

      checkPreTradeStart(_ctx, order) {
        if (order.strategyTag === "blocked") {
          return [
            {
              code: "ComplianceRestriction",
              reason: "strategy blocked",
              details: `strategy tag ${order.strategyTag} is not allowed`,
              scope: "order",
            },
          ];
        }
        return [];
      },

      performPreTradeCheck() {
        return null;
      },

      applyExecutionReport(_ctx, report) {
        appliedVenueExecId = report.venueExecId;
        return null;
      },
    };

    const strategyOrder: StrategyOrder = Object.assign(new Order(), {
      strategyTag: "alpha",
    });
    strategyOrder.operation = {
      underlyingAsset: "AAPL",
      settlementAsset: "USD",
      accountId: 99_224_416n,
      side: "BUY",
      tradeAmount: TradeAmount.quantity("10"),
      price: "25",
    };

    const strategyReport: StrategyReport = Object.assign(
      new ExecutionReport(),
      {
        venueExecId: "venue-42",
      },
    );
    strategyReport.operation = {
      underlyingAsset: "AAPL",
      settlementAsset: "USD",
      accountId: 99_224_416n,
      side: "BUY",
    };
    strategyReport.financialImpact = { pnl: "5", fee: "0.25" };

    const engine = Engine.builder().preTrade(strategyTagPolicy).build();
    const start = engine.startPreTrade(strategyOrder);
    if (!start.ok) {
      throw new Error("strategy order must pass the start stage");
    }

    const request = start.request;
    if (request === undefined) {
      throw new Error("accepted start result is missing its request");
    }
    const execute = request.execute();
    if (!execute.ok) {
      throw new Error("strategy order must pass the main stage");
    }
    const reservation = execute.reservation;
    if (reservation === undefined) {
      throw new Error("accepted execute result is missing its reservation");
    }
    reservation.commit();
    engine.applyExecutionReport(strategyReport);

    console.log(strategyOrder.strategyTag, appliedVenueExecId);

    expect(strategyOrder.strategyTag).toBe("alpha");
    expect(appliedVenueExecId).toBe("venue-42");
  });

  it("rejects orders above a notional cap", () => {
    // Source: Policy-API.md - Example: Custom Main-Stage Policy
    // Reject any order above this absolute notional. Implemented against the
    // public `Policy` interface; the callbacks read the typed `Order` view.
    function notionalCapPolicy(maxAbsNotional: Volume): Policy {
      const name = "NotionalCapPolicy";
      return {
        name,

        checkPreTradeStart(ctx: Context, order: Order): Iterable<PolicyReject> {
          void ctx;
          void order;
          return [];
        },

        performPreTradeCheck(
          ctx: Context,
          order: Order,
        ): PolicyPreTradeResult | null {
          void ctx;
          const operation = order.operation;
          if (operation === undefined) {
            return {
              rejects: [
                {
                  code: "MissingRequiredField",
                  reason: "required order field missing",
                  details: "operation is not set",
                  scope: "order",
                },
              ],
            };
          }

          // Translate the public order surface into one number this policy can
          // reason about: requested notional.
          const tradeAmount = operation.tradeAmount;
          if (tradeAmount === undefined) {
            return {
              rejects: [
                {
                  code: "MissingRequiredField",
                  reason: "required order field missing",
                  details: "tradeAmount is not set",
                  scope: "order",
                },
              ],
            };
          }

          let requestedNotional: Volume;
          if (tradeAmount.isVolume) {
            requestedNotional = tradeAmount.asVolume!;
          } else {
            const price = operation.price;
            if (price === undefined) {
              return {
                rejects: [
                  {
                    code: "OrderValueCalculationFailed",
                    reason: "order value calculation failed",
                    details: "price not provided for evaluating notional",
                    scope: "order",
                  },
                ],
              };
            }
            requestedNotional = price.calculateVolume(tradeAmount.asQuantity!);
          }

          if (requestedNotional.compare(maxAbsNotional) > 0) {
            // Business validation failures should become explicit rejects.
            return {
              rejects: [
                {
                  code: "RiskLimitExceeded",
                  reason: "strategy cap exceeded",
                  details: `requested notional ${requestedNotional.toString()}, max allowed: ${maxAbsNotional.toString()}`,
                  scope: "order",
                },
              ],
            };
          }

          // This policy only validates. It does not reserve mutable state.
          return null;
        },
      };
    }

    // Harness: cap at 1000. price=25, qty=10 => notional 250 passes; qty=100 =>
    // notional 2500 is rejected with RiskLimitExceeded.
    const engine = Engine.builder()
      .preTrade(notionalCapPolicy(Volume.fromString("1000")))
      .build();

    const below = engine.executePreTrade({
      operation: {
        underlyingAsset: "AAPL",
        settlementAsset: "USD",
        accountId: 99224416,
        side: "BUY",
        tradeAmount: TradeAmount.quantity("10"),
        price: "25",
      },
    });
    expect(below.ok).toBe(true);
    const reservation = below.reservation;
    if (reservation === undefined) {
      throw new Error("accepted execute result is missing its reservation");
    }
    reservation.commit();

    const above = engine.executePreTrade({
      operation: {
        underlyingAsset: "AAPL",
        settlementAsset: "USD",
        accountId: 99224416,
        side: "BUY",
        tradeAmount: TradeAmount.quantity("100"),
        price: "25",
      },
    });
    expect(above.ok).toBe(false);
    expect(above.rejects[0]!.code).toBe("RiskLimitExceeded");
  });

  it("rolls back eager state when the same hook rejects", () => {
    // Source: Policy-API.md - Example: Rollback Safety Pattern
    // Updates intermediate in-memory state and may then reject the same request.
    function reserveThenValidatePolicy(): Policy {
      // Policy-local state, captured by the hook closure.
      let reserved = Volume.fromString("0");
      const limit = Volume.fromString("50");

      return {
        name: "ReserveThenValidatePolicy",

        checkPreTradeStart() {
          return [];
        },

        performPreTradeCheck(
          ctx: Context,
          order: Order,
        ): PolicyPreTradeResult | null {
          void ctx;
          void order;

          // Pretend that this request needs a temporary reservation of 100. We
          // apply it eagerly because downstream logic wants to observe the
          // tentative state immediately.
          const prevReserved = reserved;
          const nextReserved = Volume.fromString("100");
          reserved = nextReserved;

          // Commit is empty: state was applied eagerly. Rollback restores the
          // previous value if any policy rejects; the engine runs it
          // automatically in reverse registration order.
          const rollback = {
            commit: () => {},
            rollback: () => {
              reserved = prevReserved;
            },
          };

          if (nextReserved.compare(limit) > 0) {
            // Return the reject together with the rollback mutation.
            return {
              rejects: [
                {
                  code: "RiskLimitExceeded",
                  reason: "temporary reservation exceeds limit",
                  details: `reserved ${nextReserved.toString()}, limit: ${limit.toString()}`,
                  scope: "order",
                },
              ],
              mutations: [rollback],
            };
          }

          return { mutations: [rollback] };
        },
      };
    }

    // Harness: the reservation (100) exceeds the limit (50), so the main stage
    // rejects and the rollback mutation must run.
    const engine = Engine.builder()

      .preTrade(reserveThenValidatePolicy())
      .build();

    const start = engine.startPreTrade({
      operation: {
        underlyingAsset: "AAPL",
        settlementAsset: "USD",
        accountId: 99224416,
        side: "BUY",
        tradeAmount: TradeAmount.quantity("10"),
        price: "25",
      },
    });
    expect(start.ok).toBe(true);

    const request = start.request;
    if (request === undefined) {
      throw new Error("accepted start result is missing its request");
    }
    const execute = request.execute();
    expect(execute.ok).toBe(false);
    expect(execute.rejects[0]!.code).toBe("RiskLimitExceeded");
  });

  it("blocks an account from an adjustment callback", () => {
    // Source: Policy-API.md - Example: Block an Account from an Adjustment Callback
    const blockOnAdjustmentPolicy: Policy = {
      name: "BlockOnAdjustmentPolicy",

      checkPreTradeStart() {
        return [];
      },

      performPreTradeCheck() {
        return null;
      },

      applyAccountAdjustment(
        _ctx: AccountAdjustmentContext,
        _accountId: AccountId,
        _adjustment: AccountAdjustment,
      ): PolicyAccountAdjustmentResult {
        void _ctx;
        void _accountId;
        void _adjustment;
        return {
          accountBlocks: [
            new AccountBlock(
              "BlockOnAdjustmentPolicy",
              "AccountBlocked",
              "blocked by account-adjustment policy",
              "custom policy reported an account block from a callback",
            ),
          ],
        };
      },
    };

    const engine = Engine.builder()

      .preTrade(blockOnAdjustmentPolicy)
      .build();

    // The accepted adjustment reports a block that the engine has already recorded.
    const adjustmentResult = engine.applyAccountAdjustment(99224416, [
      { operation: { asset: "USD" } },
    ]);
    if (!adjustmentResult.ok || adjustmentResult.accountBlocks.length !== 1) {
      throw new Error("accepted adjustment must report one account block");
    }

    // A later order on the same account is rejected with AccountBlocked, without
    // any start-check involvement.
    const blocked = engine.startPreTrade({
      operation: {
        underlyingAsset: "AAPL",
        settlementAsset: "USD",
        accountId: 99224416,
        side: "BUY",
        tradeAmount: TradeAmount.quantity("10"),
        price: "25",
      },
    });
    if (blocked.ok) {
      throw new Error("order must be blocked");
    }
    if (blocked.rejects[0]!.code !== "AccountBlocked") {
      throw new Error("expected AccountBlocked");
    }

    expect(blocked.ok).toBe(false);
    expect(blocked.rejects[0]!.code).toBe("AccountBlocked");
  });
});
