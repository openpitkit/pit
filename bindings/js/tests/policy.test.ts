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
// first. This is the first RUNTIME coverage of the JS custom-policy adapter:
// policy.types.ts (a typecheck-only file) pins the compile-time surface, while
// the cases below build an engine from a TS `Policy` object and drive its hooks
// through real pre-trade calls.
import {
  Engine,
  LifecycleError,
  OpenpitError,
  PolicyCallbackError,
} from "@openpit/engine";
import {
  AdjustmentAmount,
  Pnl,
  Price,
  TradeAmount,
} from "@openpit/engine/param";
import {
  AccountAdjustment,
  AccountAdjustmentAccountPnlOperation,
  type ExecutionReport,
  type Order,
} from "@openpit/engine/model";
import {
  type Context,
  AccountAdjustmentBatchResult,
  type Policy,
  type PolicyAccountAdjustmentResult,
  type PolicyPreTradeResult,
  type PolicyReject,
  AccountPnlOutcome,
  PnlHaltReason,
  PnlOutcome,
  PostTradeResult,
} from "@openpit/engine/pretrade";
import {
  AccountAdjustmentOutcome,
  AccountOutcomeEntry,
  PnlOutcomeAmount,
} from "@openpit/engine/accountadjustment";
import { AccountBlock } from "@openpit/engine/reject";

const ACCOUNT = 99224416;
const REJECT_CODE = "InvalidFieldValue";

// Assembles a plain-object buy/sell order for the scenario instrument.
function order(side: "BUY" | "SELL") {
  return {
    operation: {
      underlyingAsset: "AAPL",
      settlementAsset: "USD",
      accountId: ACCOUNT,
      side,
      tradeAmount: TradeAmount.quantity("100"),
      price: "185.00",
    },
  };
}

// A custom policy that accepts BUY orders and rejects SELL orders with an
// order-scoped reject. Implemented against the public `Policy` interface so the
// adapter is exercised exactly as a consumer would write it.
const sellGate: Policy = {
  name: "sell-gate",
  policyGroupId: 0,
  checkPreTradeStart(): Iterable<PolicyReject> {
    return [];
  },
  performPreTradeCheck(
    ctx: Context,
    order: Order,
  ): PolicyPreTradeResult | null {
    // `ctx` is unused on this path; touch it so the typed parameter is exercised.
    void ctx;
    if (order.operation?.side === "SELL") {
      return {
        rejects: [
          {
            code: REJECT_CODE,
            reason: "sells are disabled for this desk",
            details: "sell-gate",
            scope: "order",
          },
        ],
      };
    }
    // Accept with no contribution.
    return null;
  },
};

describe("runtime custom policy", () => {
  it("accepts an order its callback passes", () => {
    const engine = Engine.builder().preTrade(sellGate).build();

    const result = engine.executePreTrade(order("BUY"));
    expect(result.ok).toBe(true);
    expect(result.rejects).toHaveLength(0);
    // Finalize the reservation the accept produced.
    expect(() => result.reservation!.commit()).not.toThrow();
  });

  it("rejects an order its callback refuses, surfacing the code and scope", () => {
    const engine = Engine.builder().preTrade(sellGate).build();

    const result = engine.executePreTrade(order("SELL"));
    expect(result.ok).toBe(false);
    expect(result.reservation).toBeUndefined();
    expect(result.rejects).toHaveLength(1);

    const reject = result.rejects[0]!;
    expect(reject.code).toBe(REJECT_CODE);
    expect(reject.scope).toBe("order");
    expect(reject.policy).toBe("sell-gate");
    expect(reject.reason).toBe("sells are disabled for this desk");
  });

  it("validates policy reject userData before narrowing to wasm32", () => {
    const withUserData = (userData: number | bigint): Engine =>
      Engine.builder()
        .preTrade({
          name: "user-data",
          checkPreTradeStart: () => [
            {
              code: REJECT_CODE,
              reason: "token",
              details: "",
              userData,
            },
          ],
          performPreTradeCheck: () => ({}),
        })
        .build();

    const maximum = withUserData(0xffff_ffff).startPreTrade(order("BUY"));
    expect(maximum.rejects[0]?.userData).toBe(0xffff_ffffn);
    let caught: unknown;
    try {
      withUserData(0x1_0000_0000).startPreTrade(order("BUY"));
    } catch (error) {
      caught = error;
    }
    expect(caught).toBeInstanceOf(PolicyCallbackError);
    const cause = (caught as PolicyCallbackError).cause;
    expect(cause).toBeInstanceOf(RangeError);
    expect(cause).toBeInstanceOf(OpenpitError);
  });

  it("wraps a pre-trade callback failure with its original cause", () => {
    const thrown = new Error("policy callback blew up");
    const exploding: Policy = {
      name: "exploding",
      checkPreTradeStart(): Iterable<PolicyReject> {
        return [];
      },
      performPreTradeCheck(): PolicyPreTradeResult {
        throw thrown;
      },
    };

    const engine = Engine.builder().preTrade(exploding).build();

    let caught: unknown;
    try {
      engine.executePreTrade(order("BUY"));
    } catch (error) {
      caught = error;
    }
    expect(caught).toBeInstanceOf(PolicyCallbackError);
    expect((caught as PolicyCallbackError).cause).toBe(thrown);
    expect((caught as PolicyCallbackError).result).toBeUndefined();
    expect((caught as Error).message).toMatch(/policy callback blew up/);
  });

  it.each([
    ["Promise", () => Promise.resolve({})],
    ["thenable", () => ({ then: () => undefined })],
    ["malformed primitive", () => false],
    ["Date", () => new Date()],
    ["Map", () => new Map()],
  ])("rejects a %s main-stage return instead of passing", (_name, callback) => {
    const policy = {
      name: "invalid-main-return",
      checkPreTradeStart: () => [],
      performPreTradeCheck: callback,
    } as unknown as Policy;

    let caught: unknown;
    try {
      Engine.builder().preTrade(policy).build().executePreTrade(order("BUY"));
    } catch (error) {
      caught = error;
    }

    expect(caught).toBeInstanceOf(PolicyCallbackError);
    expect((caught as PolicyCallbackError).cause).toBeInstanceOf(TypeError);
  });

  it("observes a rejected native Promise after rejecting the async return", async () => {
    const unhandled: unknown[] = [];
    const onUnhandled = (reason: unknown): void => {
      unhandled.push(reason);
    };
    process.on("unhandledRejection", onUnhandled);

    try {
      const policy = {
        name: "rejected-promise-return",
        checkPreTradeStart: () => [],
        performPreTradeCheck: () => Promise.reject(new Error("async failure")),
      } as unknown as Policy;

      expect(() =>
        Engine.builder().preTrade(policy).build().executePreTrade(order("BUY")),
      ).toThrow(PolicyCallbackError);
      await new Promise((resolve) => setTimeout(resolve, 0));
      expect(unhandled).toEqual([]);
    } finally {
      process.off("unhandledRejection", onUnhandled);
    }
  });

  it("does not invoke a foreign thenable while rejecting it", () => {
    let thenCalls = 0;
    const policy = {
      name: "foreign-thenable-return",
      checkPreTradeStart: () => [],
      performPreTradeCheck: () => ({
        then: () => {
          thenCalls += 1;
        },
      }),
    } as unknown as Policy;

    expect(() =>
      Engine.builder().preTrade(policy).build().executePreTrade(order("BUY")),
    ).toThrow(PolicyCallbackError);
    expect(thenCalls).toBe(0);
  });

  it("rejects an async post-trade return instead of dropping it", () => {
    const policy = {
      name: "async-post-trade",
      checkPreTradeStart: () => [],
      performPreTradeCheck: () => ({}),
      applyExecutionReport: () =>
        Promise.resolve({
          accountBlocks: [],
          accountAdjustments: [],
        }),
    } as unknown as Policy;

    let caught: unknown;
    try {
      Engine.builder().preTrade(policy).build().applyExecutionReport({});
    } catch (error) {
      caught = error;
    }

    expect(caught).toBeInstanceOf(PolicyCallbackError);
    expect((caught as PolicyCallbackError).cause).toBeInstanceOf(TypeError);
    expect((caught as PolicyCallbackError).result).toBeInstanceOf(
      PostTradeResult,
    );
  });

  it("rejects a malformed post-trade object instead of dropping it", () => {
    const policy = {
      name: "malformed-post-trade",
      checkPreTradeStart: () => [],
      performPreTradeCheck: () => ({}),
      applyExecutionReport: () => ({ unexpected: true }),
    } as unknown as Policy;

    expect(() =>
      Engine.builder().preTrade(policy).build().applyExecutionReport({}),
    ).toThrow(PolicyCallbackError);
  });

  it("rejects a built-in object as a post-trade record", () => {
    const policy = {
      name: "built-in-post-trade",
      checkPreTradeStart: () => [],
      performPreTradeCheck: () => ({}),
      applyExecutionReport: () => new Map(),
    } as unknown as Policy;

    expect(() =>
      Engine.builder().preTrade(policy).build().applyExecutionReport({}),
    ).toThrow(PolicyCallbackError);
  });

  it("rejects an async account-adjustment return instead of passing", () => {
    const policy = {
      name: "async-account-adjustment",
      checkPreTradeStart: () => [],
      performPreTradeCheck: () => ({}),
      applyAccountAdjustment: () => Promise.resolve([]),
    } as unknown as Policy;

    let caught: unknown;
    try {
      Engine.builder()
        .preTrade(policy)
        .build()
        .applyAccountAdjustment(ACCOUNT, [
          {
            operation: { asset: "USD" },
            amount: { balance: AdjustmentAmount.absolute("1") },
          },
        ]);
    } catch (error) {
      caught = error;
    }

    expect(caught).toBeInstanceOf(PolicyCallbackError);
    expect((caught as PolicyCallbackError).cause).toBeInstanceOf(TypeError);
    expect((caught as PolicyCallbackError).result).toBeInstanceOf(
      AccountAdjustmentBatchResult,
    );
  });

  it.each([
    ["Promise", () => Promise.resolve(undefined)],
    ["thenable", () => ({ then: () => undefined })],
  ] as const)(
    "rejects a %s returned by mutation callbacks",
    (_kind, invalid) => {
      for (const action of ["commit", "rollback"] as const) {
        const policy = {
          name: `invalid-mutation-${action}`,
          checkPreTradeStart: () => [],
          performPreTradeCheck: () => ({
            mutations: [
              {
                commit: action === "commit" ? invalid : () => undefined,
                rollback: action === "rollback" ? invalid : () => undefined,
              },
            ],
          }),
        } as unknown as Policy;
        const reservation = Engine.builder()
          .preTrade(policy)
          .build()
          .executePreTrade(order("BUY")).reservation!;

        let caught: unknown;
        try {
          reservation[action]();
        } catch (error) {
          caught = error;
        }

        expect(caught).toBeInstanceOf(PolicyCallbackError);
        expect((caught as PolicyCallbackError).cause).toBeInstanceOf(TypeError);
      }
    },
  );

  it("attaches the reconciled post-trade result to callback failures", () => {
    const thrown = new Error("post-trade callback failed");
    const exploding: Policy = {
      name: "post-trade-exploding",
      checkPreTradeStart: () => [],
      performPreTradeCheck: () => ({}),
      applyExecutionReport: () => {
        throw thrown;
      },
    };
    const engine = Engine.builder().preTrade(exploding).build();

    let caught: unknown;
    try {
      engine.applyExecutionReport({
        operation: {
          underlyingAsset: "AAPL",
          settlementAsset: "USD",
          accountId: ACCOUNT,
          side: "BUY",
        },
      });
    } catch (error) {
      caught = error;
    }
    expect(caught).toBeInstanceOf(PolicyCallbackError);
    expect((caught as PolicyCallbackError).cause).toBe(thrown);
    expect((caught as PolicyCallbackError).result).toBeInstanceOf(
      PostTradeResult,
    );
  });

  it("attaches the reconciled account-adjustment result to callback failures", () => {
    const thrown = new Error("account-adjustment callback failed");
    const exploding: Policy = {
      name: "adjustment-exploding",
      checkPreTradeStart: () => [],
      performPreTradeCheck: () => ({}),
      applyAccountAdjustment: () => {
        throw thrown;
      },
    };
    const engine = Engine.builder().preTrade(exploding).build();

    let caught: unknown;
    try {
      engine.applyAccountAdjustment(ACCOUNT, [
        {
          operation: { asset: "USD" },
          amount: { balance: AdjustmentAmount.absolute("1") },
        },
      ]);
    } catch (error) {
      caught = error;
    }
    expect(caught).toBeInstanceOf(PolicyCallbackError);
    expect((caught as PolicyCallbackError).cause).toBe(thrown);
    expect((caught as PolicyCallbackError).result).toBeInstanceOf(
      AccountAdjustmentBatchResult,
    );
  });

  it("keeps PostTradeResult constructor inputs usable", () => {
    const block = new AccountBlock(
      "constructor-inputs",
      "Other",
      "blocked",
      "test block",
      undefined,
    );
    const outcome = new AccountAdjustmentOutcome(
      7,
      new AccountOutcomeEntry("USD"),
    );
    const pnl = new AccountPnlOutcome(
      7,
      ACCOUNT,
      new PnlOutcomeAmount("1.25", "7.5"),
      undefined,
    );

    const result = new PostTradeResult([block], [pnl], [outcome]);

    expect(block.clone().reason).toBe("blocked");
    expect(outcome.clone().entry.asset).toBe("USD");
    expect(result.accountBlocks[0]?.policy).toBe("constructor-inputs");
    expect(result.accountAdjustments[0]?.policyGroupId).toBe(7);
    expect(result.accountPnls[0]?.pnl?.delta.toString()).toBe("1.25");
  });

  it("round-trips account PnLs from custom post-trade policies", () => {
    const computed = new AccountPnlOutcome(
      7,
      ACCOUNT,
      new PnlOutcomeAmount("1.25", "7.5"),
      undefined,
    );
    const missingAccountCurrency = new AccountPnlOutcome(
      7,
      ACCOUNT,
      undefined,
      PnlHaltReason.fromMissingAccountCurrency(),
    );
    const policy: Policy = {
      name: "account-pnl-post-trade",
      checkPreTradeStart: () => [],
      performPreTradeCheck: () => ({}),
      applyExecutionReport: () =>
        new PostTradeResult(
          undefined,
          [computed, missingAccountCurrency],
          undefined,
        ),
    };

    const result = Engine.builder()
      .preTrade(policy)
      .build()
      .applyExecutionReport({});

    expect(result.accountPnls).toHaveLength(2);
    expect(result.accountPnls[0]?.accountId.value).toBe(BigInt(ACCOUNT));
    expect(result.accountPnls[0]?.policyGroupId).toBe(7);
    expect(result.accountPnls[0]?.ok).toBe(true);
    expect(result.accountPnls[0]?.isHalted).toBe(false);
    expect(result.accountPnls[0]?.haltReason).toBeUndefined();
    expect(result.accountPnls[0]?.pnl?.absolute.toString()).toBe("7.5");
    expect(result.accountPnls[1]?.policyGroupId).toBe(7);
    expect(result.accountPnls[1]?.ok).toBe(false);
    expect(result.accountPnls[1]?.isHalted).toBe(true);
    expect(result.accountPnls[1]?.haltReason?.isMissingAccountCurrency).toBe(
      true,
    );
    expect(result.accountPnls[1]?.pnl).toBeUndefined();
  });

  it("validates PnL outcome state and exposes every halt reason", () => {
    const amount = new PnlOutcomeAmount("1", "2");
    expect(() => new PnlOutcome(undefined, undefined)).toThrow(TypeError);
    expect(() => new PnlOutcome(amount, PnlHaltReason.fromMissingFx())).toThrow(
      TypeError,
    );
    expect(
      () => new AccountPnlOutcome(7, ACCOUNT, undefined, undefined),
    ).toThrow(TypeError);
    expect(
      () =>
        new AccountPnlOutcome(
          7,
          ACCOUNT,
          amount,
          PnlHaltReason.fromMissingFx(),
        ),
    ).toThrow(TypeError);
    expect(
      () => new AccountAdjustmentAccountPnlOperation(null as never),
    ).toThrow(TypeError);

    const computed = new PnlOutcome(amount, undefined);
    expect(computed.ok).toBe(true);
    expect(computed.isHalted).toBe(false);

    const reasons = [
      PnlHaltReason.fromMissingFx(),
      PnlHaltReason.fromMissingAccountCurrency(),
      PnlHaltReason.fromMissingInitialPnl(),
      PnlHaltReason.fromMissingCostBasis(),
      PnlHaltReason.fromArithmeticOverflow(),
    ];
    expect(reasons.map((reason) => reason.kind)).toEqual([
      "missing-fx",
      "missing-account-currency",
      "missing-initial-pnl",
      "missing-cost-basis",
      "arithmetic-overflow",
    ]);
    expect([
      reasons[0]?.isMissingFx,
      reasons[1]?.isMissingAccountCurrency,
      reasons[2]?.isMissingInitialPnl,
      reasons[3]?.isMissingCostBasis,
      reasons[4]?.isArithmeticOverflow,
    ]).toEqual([true, true, true, true, true]);

    const halted = new PnlOutcome(
      undefined,
      PnlHaltReason.fromArithmeticOverflow(),
    );
    expect(halted.ok).toBe(false);
    expect(halted.isHalted).toBe(true);
  });

  it("surfaces position PnL outcomes and passes account PnL adjustments", () => {
    const seenPnl: string[] = [];
    const parity: Policy = {
      name: "parity",
      checkPreTradeStart(): Iterable<PolicyReject> {
        return [];
      },
      performPreTradeCheck(): PolicyPreTradeResult {
        return {
          accountAdjustments: [
            new AccountOutcomeEntry(
              "USD",
              undefined,
              undefined,
              undefined,
              new PnlOutcome(new PnlOutcomeAmount("1.25", "7.5"), undefined),
              Price.fromString("11"),
            ),
          ],
          lockPrices: [Price.fromString("11")],
        };
      },
      applyAccountAdjustment(_ctx, _accountId, adjustment) {
        const operation = adjustment.operation;
        if (operation !== undefined && "state" in operation) {
          const state = operation.state;
          if (state instanceof Pnl) {
            seenPnl.push(state.toString());
          }
        }
        return { accountBlocks: [] };
      },
    };

    const engine = Engine.builder().preTrade(parity).build();

    const execute = engine.executePreTrade(order("BUY"));
    expect(execute.ok).toBe(true);
    const outcome = execute.reservation!.accountAdjustments()[0]!;
    expect(outcome.entry.realizedPnl!.pnl?.delta.toString()).toBe("1.25");
    expect(
      outcome.entry.realizedPnl!.pnl?.absolute.equals(Pnl.fromString("7.5")),
    ).toBe(true);
    expect(
      outcome.entry.averageEntryPrice!.equals(Price.fromString("11")),
    ).toBe(true);

    const batch = engine.applyAccountAdjustment(ACCOUNT, [
      {
        operation: { state: "42.5" },
      },
    ]);
    expect(batch.ok).toBe(true);
    expect(seenPnl).toEqual(["42.5"]);

    expect(() =>
      engine.applyAccountAdjustment(ACCOUNT, [
        {
          // @ts-expect-error - runtime rejects the removed `pnl` field.
          operation: { pnl: "42.5" },
        },
      ]),
    ).toThrow("account PnL operation uses state");

    expect(() =>
      engine.applyAccountAdjustment(ACCOUNT, [
        {
          operation: { state: "42.5", asset: "USD" },
        },
      ]),
    ).toThrow("cannot combine state with balance or position fields");
  });

  it("accepts PolicyAccountAdjustmentResult from applyAccountAdjustment", () => {
    const adjustment = {
      operation: { asset: "USD" },
      amount: { balance: AdjustmentAmount.absolute("1") },
    };
    const run = (result: PolicyAccountAdjustmentResult) => {
      const policy: Policy = {
        name: "adjustment-decision",
        checkPreTradeStart: () => [],
        performPreTradeCheck: () => ({}),
        applyAccountAdjustment: () => result,
      };
      return Engine.builder()
        .preTrade(policy)
        .build()
        .applyAccountAdjustment(ACCOUNT, [adjustment]);
    };

    expect(run({ accountBlocks: [] }).ok).toBe(true);

    const blocked = run({
      accountBlocks: [
        new AccountBlock(
          "adjustment-decision",
          "AccountBlocked",
          "accepted adjustment blocked account",
          "block is returned with the accepted batch",
          undefined,
        ),
      ],
    });
    expect(blocked.ok).toBe(true);
    expect(blocked.accountBlocks).toHaveLength(1);
    expect(blocked.accountBlocks[0]?.code).toBe("AccountBlocked");

    const rejected = run({
      accountBlocks: [],
      rejects: [
        {
          code: REJECT_CODE,
          reason: "adjustment rejected",
          details: "",
        },
      ],
    });
    expect(rejected.ok).toBe(false);
    expect(rejected.rejects[0]?.code).toBe(REJECT_CODE);

    let commits = 0;
    const mutated = run({
      accountBlocks: [],
      mutations: [
        {
          commit: () => {
            commits += 1;
          },
          rollback: () => {
            throw new Error("accepted adjustment must not roll back");
          },
        },
      ],
    });
    expect(mutated.ok).toBe(true);
    expect(commits).toBe(1);
  });

  it("fails closed on unrecognized account-adjustment result fields", () => {
    const adjustment = {
      operation: { asset: "USD" },
      amount: { balance: AdjustmentAmount.absolute("1") },
    };
    const run = (result: unknown) => {
      const policy: Policy = {
        name: "adjustment-result-shape",
        checkPreTradeStart: () => [],
        performPreTradeCheck: () => ({}),
        applyAccountAdjustment: () => result as PolicyAccountAdjustmentResult,
      };
      return Engine.builder()
        .preTrade(policy)
        .build()
        .applyAccountAdjustment(ACCOUNT, [adjustment]);
    };

    expect(run({}).ok).toBe(true);
    expect(run({ accountBlocks: [], accountOutcomes: [] }).ok).toBe(true);

    let caught: unknown;
    try {
      run({ accountOutcomes: [] });
    } catch (error) {
      caught = error;
    }
    expect(caught).toBeInstanceOf(PolicyCallbackError);
    expect((caught as PolicyCallbackError).cause).toBeInstanceOf(TypeError);
    expect(((caught as PolicyCallbackError).cause as Error).message).toContain(
      "no recognized result fields",
    );
  });

  it("normalizes callback models while preserving isolated custom metadata", () => {
    const marker = Symbol("strategy-marker");
    const prototype = {
      strategyName(): string {
        return "mean-reversion";
      },
    };
    const backing = new Uint8Array([9, 8, 7, 6]);
    let closePositionReads = 0;
    let autoBorrowReads = 0;
    const position = {
      get closePosition(): boolean {
        closePositionReads += 1;
        if (closePositionReads > 1) {
          throw new Error("closePosition getter must be normalized once");
        }
        return true;
      },
    };
    const margin = {
      collateralAsset: "USD",
      get autoBorrow(): boolean {
        autoBorrowReads += 1;
        if (autoBorrowReads > 1) {
          throw new Error("autoBorrow getter must be normalized once");
        }
        return true;
      },
    };
    const submitted = Object.assign(Object.create(prototype), order("BUY"), {
      position,
      margin,
      metadata: { revision: 7 },
      view: new DataView(backing.buffer, 1, 2),
      [marker]: "symbol-value",
    }) as ReturnType<typeof order> & {
      metadata: { revision: number };
      view: DataView;
      self?: unknown;
      operationAlias?: unknown;
      position: { readonly closePosition: boolean };
      margin: { readonly autoBorrow: boolean };
      [marker]: string;
    };
    Object.assign(submitted.operation, { venueTag: "XNYS" });
    submitted.self = submitted;
    submitted.operationAlias = submitted.operation;

    type CustomOrder = Order & {
      metadata: { revision: number };
      view: DataView;
      self: CustomOrder;
      [marker]: string;
      operation: NonNullable<Order["operation"]> & { venueTag: string };
      operationAlias: NonNullable<Order["operation"]> & { venueTag: string };
      strategyName(): string;
    };

    const first: Policy<CustomOrder> = {
      name: "metadata-mutator",
      checkPreTradeStart: () => [],
      performPreTradeCheck(_ctx, payload) {
        expect(payload.operation.price).toBeInstanceOf(Price);
        expect(payload.position?.closePosition).toBe(true);
        expect(payload.margin?.autoBorrow).toBe(true);
        expect(payload.strategyName()).toBe("mean-reversion");
        expect(Object.getPrototypeOf(payload)).toBe(prototype);
        expect(payload.self).toBe(payload);
        expect(payload.operationAlias).toBe(payload.operation);
        expect(payload[marker]).toBe("symbol-value");
        expect(payload.view.byteLength).toBe(2);
        expect(payload.view.getUint8(0)).toBe(8);
        payload.metadata.revision = 99;
        payload.operation.venueTag = "MUTATED";
        payload.view.setUint8(0, 1);
        return {};
      },
    };
    const second: Policy<CustomOrder> = {
      name: "metadata-observer",
      checkPreTradeStart: () => [],
      performPreTradeCheck(_ctx, payload) {
        expect(payload.metadata.revision).toBe(7);
        expect(payload.position?.closePosition).toBe(true);
        expect(payload.margin?.autoBorrow).toBe(true);
        expect(payload.operation.venueTag).toBe("XNYS");
        expect(payload.operationAlias).toBe(payload.operation);
        expect(payload.self).toBe(payload);
        expect(payload.view.getUint8(0)).toBe(8);
        return {};
      },
    };

    const ready = Engine.builder().preTrade(first);
    ready.preTrade(second);
    const result = ready.build().executePreTrade(submitted);
    expect(result.ok).toBe(true);
    result.reservation!.rollback();
    expect(submitted.metadata.revision).toBe(7);
    expect(submitted.view.getUint8(0)).toBe(8);
    expect((submitted.operation as { venueTag?: string }).venueTag).toBe(
      "XNYS",
    );
    expect(submitted.operationAlias).toBe(submitted.operation);
    expect(closePositionReads).toBe(1);
    expect(autoBorrowReads).toBe(1);
  });

  it("normalizes execution reports while preserving custom report metadata", () => {
    const marker = Symbol("execution-marker");
    const prototype = {
      source(): string {
        return "venue-adapter";
      },
    };
    const submitted = Object.assign(
      Object.create(prototype),
      {
        operation: {
          underlyingAsset: "AAPL",
          settlementAsset: "USD",
          accountId: ACCOUNT,
          side: "BUY" as const,
        },
      },
      { executionId: "exec-7", [marker]: true },
    ) as {
      operation: {
        underlyingAsset: string;
        settlementAsset: string;
        accountId: number;
        side: "BUY";
      };
      executionId: string;
      self?: unknown;
      [marker]: boolean;
    };
    submitted.self = submitted;

    type CustomReport = ExecutionReport & {
      executionId: string;
      self: CustomReport;
      [marker]: boolean;
      source(): string;
    };
    const policy: Policy<Order, CustomReport> = {
      name: "custom-report",
      checkPreTradeStart: () => [],
      performPreTradeCheck: () => ({}),
      applyExecutionReport(_ctx, report) {
        expect(report.operation?.accountId?.value).toBe(BigInt(ACCOUNT));
        expect(report.executionId).toBe("exec-7");
        expect(report.source()).toBe("venue-adapter");
        expect(report.self).toBe(report);
        expect(report[marker]).toBe(true);
        return null;
      },
    };

    expect(() =>
      Engine.builder().preTrade(policy).build().applyExecutionReport(submitted),
    ).not.toThrow();
  });

  it("keeps the first callback error while later policies still reconcile", () => {
    const firstError = new Error("first callback failure");
    const secondError = new Error("second callback failure");
    const makePolicy = (name: string, thrown: Error): Policy => ({
      name,
      checkPreTradeStart: () => [],
      performPreTradeCheck: () => ({}),
      applyExecutionReport: () => {
        throw thrown;
      },
    });
    const ready = Engine.builder().preTrade(makePolicy("first", firstError));
    ready.preTrade(makePolicy("second", secondError));
    const engine = ready.build();

    let caught: unknown;
    try {
      engine.applyExecutionReport({
        operation: {
          underlyingAsset: "AAPL",
          settlementAsset: "USD",
          accountId: ACCOUNT,
          side: "BUY",
        },
      });
    } catch (error) {
      caught = error;
    }
    expect(caught).toBeInstanceOf(PolicyCallbackError);
    expect((caught as PolicyCallbackError).cause).toBe(firstError);
    expect((caught as PolicyCallbackError).result).toBeInstanceOf(
      PostTradeResult,
    );
  });

  it("shares and invalidates retained account-control lifecycle handles", () => {
    let retained: Context["accountControl"];
    const policy: Policy = {
      name: "retained-control",
      checkPreTradeStart(ctx) {
        retained = ctx.accountControl;
        return [];
      },
      performPreTradeCheck: () => ({}),
    };
    const result = Engine.builder()
      .preTrade(policy)
      .build()
      .executePreTrade(order("BUY"));
    result.reservation!.rollback();

    expect(() =>
      retained!.block(
        new AccountBlock(
          "retained-control",
          "Other",
          "late block",
          "transaction is finalized",
          undefined,
        ),
      ),
    ).toThrow(LifecycleError);
  });

  it("passes a typed adjustment and accepts an outcome in its result", () => {
    let sawTypedAdjustment = false;
    const policy: Policy = {
      name: "plain-outcome",
      checkPreTradeStart: () => [],
      performPreTradeCheck: () => ({}),
      applyAccountAdjustment(_ctx, _accountId, adjustment) {
        sawTypedAdjustment = adjustment instanceof AccountAdjustment;
        return {
          accountAdjustments: [
            { asset: "USD", balance: { delta: "1", absolute: "1" } },
          ],
          accountBlocks: [],
        };
      },
    };
    const batch = Engine.builder()
      .preTrade(policy)
      .build()
      .applyAccountAdjustment(ACCOUNT, [
        {
          operation: { asset: "USD" },
          amount: { balance: AdjustmentAmount.absolute("1") },
        },
      ]);

    expect(sawTypedAdjustment).toBe(true);
    expect(batch.ok).toBe(true);
    expect(batch.outcomes[0]?.entry.balance?.delta.toString()).toBe("1");
  });

  it("preserves thrown getter and iterator errors as callback causes", () => {
    const getterError = new Error("accountAdjustments getter failed");
    const policy: Policy = {
      name: "throwing-getter",
      checkPreTradeStart: () => [],
      performPreTradeCheck: () => {
        const result = {};
        Object.defineProperty(result, "accountAdjustments", {
          get() {
            throw getterError;
          },
        });
        return result;
      },
    };

    let caught: unknown;
    try {
      Engine.builder().preTrade(policy).build().executePreTrade(order("BUY"));
    } catch (error) {
      caught = error;
    }
    expect(caught).toBeInstanceOf(PolicyCallbackError);
    expect((caught as PolicyCallbackError).cause).toBe(getterError);

    const iteratorError = new Error("iterator getter failed");
    const iteratorPolicy: Policy = {
      name: "throwing-iterator",
      checkPreTradeStart: () => {
        const rejects = {};
        Object.defineProperty(rejects, Symbol.iterator, {
          get() {
            throw iteratorError;
          },
        });
        return rejects as Iterable<PolicyReject>;
      },
      performPreTradeCheck: () => ({}),
    };
    caught = undefined;
    try {
      Engine.builder()
        .preTrade(iteratorPolicy)
        .build()
        .startPreTrade(order("BUY"));
    } catch (error) {
      caught = error;
    }
    expect(caught).toBeInstanceOf(PolicyCallbackError);
    expect((caught as PolicyCallbackError).cause).toBe(iteratorError);
  });

  it("does not leak destructor rollback errors into a later operation", () => {
    const policy: Policy = {
      name: "rollback-destructor",
      checkPreTradeStart: () => [],
      performPreTradeCheck: () => ({
        mutations: [
          {
            commit: () => undefined,
            rollback: () => {
              throw new Error("implicit rollback failure");
            },
          },
        ],
      }),
    };
    const engine = Engine.builder().preTrade(policy).build();
    const first = engine.executePreTrade(order("BUY"));
    const reservation = first.reservation!;
    (first as unknown as { free(): void }).free();
    (reservation as unknown as { free(): void }).free();

    const next = engine.startPreTrade(order("BUY"));
    const request = next.request!;
    (next as unknown as { free(): void }).free();
    (request as unknown as { free(): void }).free();
  });
});
