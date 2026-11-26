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
// Mirrors the public JS runtime-configuration examples from the project wiki.
// The tests marked with `// Source:` keep the snippet body in lockstep with:
// - https://wiki.openpit.dev/Dynamic-Policy-Reconfiguration/
// - https://wiki.openpit.dev/Spot-Funds/

import { describe, expect, it } from "vitest";

import {
  AccountIdError,
  AssetError,
  ConfigureErrorKind,
  Engine,
  ParamError,
  PolicyConfigureError,
} from "@openpit/engine";
import { QuoteTtl } from "@openpit/engine/marketdata";
import {
  AccountGroupId,
  AdjustmentAmount,
  Instrument,
  Price,
  TradeAmount,
} from "@openpit/engine/param";
import { type OrderInit } from "@openpit/engine/model";
import { Lock, PnlHaltReason } from "@openpit/engine/pretrade";
import { RuleMode } from "@openpit/engine/reject";
import {
  buildOrderSizeLimit,
  buildPnlBoundsKillswitch,
  buildRateLimit,
  buildReviewRule,
  buildSpotFunds,
  buildSpotFundsPnlBoundsKillswitch,
  OrderSizeAccountAssetBarrier,
  OrderSizeAssetBarrier,
  OrderSizeBrokerBarrier,
  OrderSizeLimit,
  OrderSizeLimitBuilder,
  PnlBoundsAccountAssetBarrier,
  PnlBoundsAccountAssetBarrierUpdate,
  PnlBoundsBrokerBarrier,
  PnlBoundsKillswitchBuilder,
  RateLimit,
  RateLimitAccountAssetBarrier,
  RateLimitAccountBarrier,
  RateLimitAssetBarrier,
  RateLimitBrokerBarrier,
  RateLimitBuilder,
  ReviewRule,
  ReviewRuleBuilder,
  type ReviewRuleConfigureOptions,
  SpotFundsBuilder,
  SpotFundsLimitMode,
  SpotFundsOverride,
  SpotFundsPnlBoundsAccountBarrier,
  SpotFundsPnlBoundsAccountGroupBarrier,
  SpotFundsPnlBoundsBarrier,
  SpotFundsPnlBoundsKillswitchBuilder,
} from "@openpit/engine/pretrade/policies";

import { holdings } from "./spot_funds_holdings";

const ACCOUNT = 99_224_416n;
const ORDER_SIZE_OVERRIDE_ACCOUNT = 99_224_417n;

function makeOrder(
  quantity: string = "1",
  price: string = "100",
  accountId: bigint = ACCOUNT,
): OrderInit {
  return {
    operation: {
      underlyingAsset: "AAPL",
      settlementAsset: "USD",
      accountId,
      side: "BUY",
      tradeAmount: TradeAmount.quantity(quantity),
      price,
    },
  };
}

// `fee` is denominated in the account currency, so a nonzero value engages the
// account line's need for that currency. A fee-less opening fill omits the
// account line instead.
function applySpotFundsFill(
  engine: Engine,
  accountId: bigint = ACCOUNT,
  side: "BUY" | "SELL" = "BUY",
  fee?: string,
) {
  const lock = new Lock(undefined);
  lock.push(0, Price.fromString("100"));
  return engine.applyExecutionReport({
    operation: {
      underlyingAsset: "AAPL",
      settlementAsset: "USD",
      accountId,
      side,
    },
    fill: {
      lastTrade: { price: "100", quantity: "1" },
      remainingReservedQuantity: "0",
      lock,
      isFinal: true,
      ...(fee === undefined ? {} : { fee: { amount: fee, currency: "USD" } }),
    },
  });
}

function forceSpotFundsBalancePnl(
  engine: Engine,
  accountId: bigint = ACCOUNT,
  state: string | PnlHaltReason = "0",
) {
  const result = engine.applyAccountAdjustment(accountId, [
    {
      operation: {
        asset: "AAPL",
        averageEntryPrice: "100",
        realizedPnl: state,
      },
    },
  ]);
  expect(result.ok).toBe(true);
}

function expectConfigureError(
  error: unknown,
  kind: (typeof ConfigureErrorKind)[keyof typeof ConfigureErrorKind],
): void {
  expect(error).toBeInstanceOf(PolicyConfigureError);
  expect((error as PolicyConfigureError).kind).toBe(kind);
}

function expectUnknownOption(
  configure: () => unknown,
  label: string,
  key: string,
): void {
  let caught: unknown;
  try {
    configure();
  } catch (error) {
    caught = error;
  }
  expect(caught).toBeInstanceOf(TypeError);
  expect(caught).toHaveProperty("message", `${label} has unknown key "${key}"`);
}

describe("review-rule configuration", () => {
  it("replaces, keeps, and clears each level atomically", () => {
    const name = ReviewRuleBuilder.NAME;
    const builder = Engine.builder().builtin(
      buildRateLimit().accountBarriers([]),
    );
    builder.builtin(buildReviewRule().accountRules([]));
    const engine = builder.build();
    engine.accounts().registerGroup([1001], 7);
    const configurator = engine.configure();
    const aapl = new Instrument("AAPL", "USD");
    const group = new ReviewRule({ accountGroupId: 7 });
    const account = new ReviewRule({ accountId: 1001, instruments: [aapl] });
    const global = new ReviewRule({ instruments: [aapl] });
    function check(accountId: bigint, symbol: string, rejected: boolean): void {
      const order = makeOrder("10", "25", accountId);
      if (!order.operation) {
        throw new Error("expected an order operation");
      }
      order.operation.underlyingAsset = symbol;
      const result = engine.executePreTradeDryRun(order);
      expect(result.rejects.map(({ code }) => code)).toEqual(
        rejected ? ["ComplianceRestriction"] : [],
      );
      expect(result.reviews).toEqual([]);
    }

    check(1001n, "AAPL", false);
    const misspelled = { accountGroupRules: [group], accountGroupsRules: [] };
    expectUnknownOption(
      () => configurator.reviewRule(name, misspelled),
      "reviewRule options",
      "accountGroupsRules",
    );
    check(1001n, "MSFT", false);
    configurator.reviewRule(name, { accountGroupRules: new Set([group]) });
    check(1001n, "MSFT", true);
    check(1002n, "MSFT", false);
    configurator.reviewRule(name, {
      globalRule: global,
      accountGroupRules: null,
    });
    configurator.reviewRule(name, { accountRules: [account] });
    check(1001n, "MSFT", true);
    check(1002n, "AAPL", true);
    check(1002n, "MSFT", false);

    const invalidRules: [ReviewRuleConfigureOptions, string][] = [
      [
        { globalRule: new ReviewRule({ instruments: [] }) },
        "an exact instrument list must not be empty",
      ],
      [
        { globalRule: new ReviewRule({ instruments: [aapl, aapl] }) },
        "duplicate review rule instrument AAPL/USD",
      ],
      [
        { clearGlobalRule: true, accountGroupRules: [group, group] },
        "duplicate account group review rule",
      ],
      [
        {
          clearGlobalRule: true,
          accountGroupRules: [],
          accountRules: [account, account],
        },
        "duplicate account review rule",
      ],
    ];
    for (const [options, message] of invalidRules) {
      try {
        configurator.reviewRule(name, options);
        throw new Error("expected PolicyConfigureError");
      } catch (error) {
        expectConfigureError(error, ConfigureErrorKind.Validation);
        expect((error as PolicyConfigureError).validationMessage).toBe(message);
      }
      check(1001n, "MSFT", true);
      check(1002n, "AAPL", true);
      check(1002n, "MSFT", false);
    }

    const badShapes: [ReviewRuleConfigureOptions, string][] = [
      [
        { globalRule: global, clearGlobalRule: true },
        "globalRule and clearGlobalRule cannot be used together",
      ],
      [{ globalRule: account }, "global level"],
      [
        { clearGlobalRule: true, accountGroupRules: [account] },
        "account group level",
      ],
      [
        { clearGlobalRule: true, accountGroupRules: [], accountRules: [group] },
        "account level",
      ],
      [
        {
          clearGlobalRule: true,
          accountRules: [
            new ReviewRule({ accountId: 1001, accountGroupId: 7 }),
          ],
        },
        "account level",
      ],
    ];
    for (const [options, message] of badShapes) {
      try {
        configurator.reviewRule(name, options);
        throw new Error("expected ParamError");
      } catch (error) {
        expect(error).toBeInstanceOf(ParamError);
        expect((error as ParamError).code).toBe("Other");
        expect((error as ParamError).message).toContain(message);
      }
      check(1001n, "MSFT", true);
      check(1002n, "AAPL", true);
    }

    configurator.reviewRule(name, {
      clearGlobalRule: true,
      accountGroupRules: [],
    });
    check(1001n, "AAPL", true);
    check(1001n, "MSFT", false);
    check(1002n, "AAPL", false);
    configurator.reviewRule(name, { globalRule: null, accountRules: null });
    check(1001n, "AAPL", true);
    configurator.reviewRule(name, { accountRules: [] });
    check(1001n, "AAPL", false);

    for (const [other, kind] of [
      ["missing", ConfigureErrorKind.Unknown],
      [RateLimitBuilder.NAME, ConfigureErrorKind.TypeMismatch],
    ] as const) {
      try {
        configurator.reviewRule(other, { globalRule: global });
        throw new Error("expected PolicyConfigureError");
      } catch (error) {
        expectConfigureError(error, kind);
      }
    }
    check(1002n, "AAPL", false);
  });
});

describe("P&L-bounds kill switch", () => {
  const name = PnlBoundsKillswitchBuilder.NAME;

  function buildBroker(...mode: [mode?: RuleMode | null]): Engine {
    return Engine.builder()
      .builtin(
        buildPnlBoundsKillswitch().brokerBarriers([
          new PnlBoundsBrokerBarrier("USD", "-100", undefined, ...mode),
        ]),
      )
      .build();
  }

  function setPnl(engine: Engine, pnl: string): void {
    engine.configure().setAccountPnl(name, {
      account: ACCOUNT,
      settlementAsset: "USD",
      pnl,
    });
  }

  it("rejects a misspelled generic P&L accumulator option", () => {
    const engine = buildBroker();
    const configure = engine.configure();
    const pnl = { account: ACCOUNT, settlementAsset: "USD", pnl: "-150" };
    const misspelledPnl = { ...pnl, settlement: "USD" };
    expectUnknownOption(
      () => configure.setAccountPnl(name, misspelledPnl),
      "setAccountPnl options",
      "settlement",
    );
    expect(engine.startPreTrade(makeOrder()).ok).toBe(true);
    configure.setAccountPnl(name, pnl);
    expect(engine.startPreTrade(makeOrder()).rejects[0]?.code).toBe(
      "PnlKillSwitchTriggered",
    );
  });

  it("rejects a misspelled generic P&L barrier option", () => {
    const engine = buildBroker();
    const configure = engine.configure();
    const brokerBarriers = [
      new PnlBoundsBrokerBarrier("USD", "-50", undefined),
    ];
    const misspelledBarriers = { brokerBarriers, brokerBarrier: [] };
    expectUnknownOption(
      () => configure.pnlBoundsKillswitch(name, misspelledBarriers),
      "pnlBoundsKillswitch options",
      "brokerBarrier",
    );
    setPnl(engine, "-75");
    expect(engine.startPreTrade(makeOrder()).ok).toBe(true);
    configure.pnlBoundsKillswitch(name, { brokerBarriers });
    expect(engine.startPreTrade(makeOrder()).rejects[0]?.code).toBe(
      "PnlKillSwitchTriggered",
    );
  });

  it.each([
    { label: "omitted mode", build: () => buildBroker() },
    { label: "null mode", build: () => buildBroker(null) },
    { label: "Reject", build: () => buildBroker(RuleMode.Reject) },
  ])("rejects and latches an account block with $label", ({ build }) => {
    const engine = build();
    setPnl(engine, "-150");

    const rejected = engine.startPreTrade(makeOrder());
    expect(rejected.ok).toBe(false);
    expect(rejected.reviews).toEqual([]);
    expect(rejected.rejects.map((reject) => reject.code)).toEqual([
      "PnlKillSwitchTriggered",
    ]);

    setPnl(engine, "0");
    const blocked = engine.startPreTrade(makeOrder());
    expect(blocked.ok).toBe(false);
    expect(blocked.rejects.map((reject) => reject.code)).toEqual([
      "AccountBlocked",
    ]);
  });

  describe.each([RuleMode.ReviewWithHold, RuleMode.ReviewWithoutHold])(
    "%s",
    (mode) => {
      function expectReviewAndUnblockedAccount(engine: Engine): void {
        const started = engine.startPreTrade(makeOrder());
        expect(started.ok).toBe(true);
        expect(started.rejects).toEqual([]);
        expect(started.reviews).toMatchObject([
          { mode, code: "PnlKillSwitchTriggered" },
        ]);

        const following = engine.executePreTrade(makeOrder());
        expect(following.ok).toBe(true);
        expect(following.rejects).toEqual([]);
        expect(following.reviews).toMatchObject([
          { mode, code: "PnlKillSwitchTriggered" },
        ]);
        following.reservation!.rollback();
      }

      it("marks a broker breach and leaves the account open", () => {
        const engine = buildBroker(mode);
        setPnl(engine, "-150");
        expectReviewAndUnblockedAccount(engine);
      });

      it("marks an account breach and leaves the account open", () => {
        const engine = Engine.builder()
          .builtin(
            buildPnlBoundsKillswitch().accountBarriers([
              new PnlBoundsAccountAssetBarrier(
                ACCOUNT,
                "USD",
                "-150",
                "-100",
                undefined,
                mode,
              ),
            ]),
          )
          .build();
        expectReviewAndUnblockedAccount(engine);
      });

      it("carries the account update mode into the active barriers", () => {
        const engine = buildBroker();
        setPnl(engine, "-150");
        engine.configure().pnlBoundsKillswitch(name, {
          brokerBarriers: [
            new PnlBoundsBrokerBarrier("USD", "-1000", undefined),
          ],
          accountBarriers: [
            new PnlBoundsAccountAssetBarrierUpdate(
              ACCOUNT,
              "USD",
              "-100",
              undefined,
              mode,
            ),
          ],
        });
        expectReviewAndUnblockedAccount(engine);
      });

      it("rejects an order missing a required instrument field", () => {
        const engine = buildBroker(mode);
        const result = engine.startPreTrade({
          operation: { accountId: ACCOUNT, underlyingAsset: "AAPL" },
        });

        expect(result.ok).toBe(false);
        expect(result.reviews).toEqual([]);
        expect(result.rejects).toMatchObject([
          { code: "MissingRequiredField", mode: RuleMode.Reject, policy: name },
        ]);
      });
    },
  );

  it.each(
    (["broker", "account"] as const).flatMap((axis) =>
      [false, true].map((confirmed) => ({ axis, confirmed })),
    ),
  )(
    "enforces mixed P&L barriers on the $axis axis, confirmed=$confirmed",
    ({ axis, confirmed }) => {
      const reviewModes = [RuleMode.ReviewWithHold, RuleMode.ReviewWithoutHold];
      const bounds = [
        ...reviewModes.map((mode) => ({ mode, lower: "-100" })),
        { mode: RuleMode.Reject, lower: "-200" },
      ];
      const builder = buildPnlBoundsKillswitch();
      let policy;
      if (axis === "broker") {
        policy = builder.brokerBarriers(
          bounds.map(
            ({ mode, lower }) =>
              new PnlBoundsBrokerBarrier("USD", lower, undefined, mode),
          ),
        );
      } else {
        policy = builder.accountBarriers(
          bounds.map(
            ({ mode, lower }) =>
              new PnlBoundsAccountAssetBarrier(
                ACCOUNT,
                "USD",
                "0",
                lower,
                undefined,
                mode,
              ),
          ),
        );
      }
      const engine = Engine.builder().builtin(policy).build();
      setPnl(engine, "-150");
      const order = makeOrder();
      const expectedReviews = reviewModes.map((mode) => ({
        mode,
        code: "PnlKillSwitchTriggered",
        scope: "account",
      }));
      const first = engine.startPreTrade(order);
      expect(first.ok).toBe(true);
      expect(
        first.reviews.map(({ mode, code, scope }) => ({ mode, code, scope })),
      ).toEqual(expect.arrayContaining(expectedReviews));
      expect(first.reviews).toHaveLength(expectedReviews.length);
      const following = engine.startPreTrade(order);
      expect(following.ok).toBe(true);
      expect(
        following.reviews.map(({ mode, code, scope }) => ({
          mode,
          code,
          scope,
        })),
      ).toEqual(expect.arrayContaining(expectedReviews));
      expect(following.reviews).toHaveLength(expectedReviews.length);
      if (confirmed) {
        order.review = { confirmed: true };
        const rerun = engine.startPreTrade(order);
        expect(rerun.ok).toBe(true);
        expect(rerun.reviews).toEqual([]);
      }
      setPnl(engine, "-250");
      const effectiveReject = [
        {
          mode: RuleMode.Reject,
          code: "PnlKillSwitchTriggered",
          scope: "account",
        },
      ];
      if (!confirmed) {
        const dry = engine.startPreTradeDryRun(order);
        expect(dry.isPass).toBe(false);
        expect(dry.reviews).toEqual([]);
        expect(dry.rejects).toMatchObject(effectiveReject);
      }
      const rejected = engine.startPreTrade(order);
      expect(rejected.ok).toBe(false);
      expect(rejected.rejects).toMatchObject(effectiveReject);
      setPnl(engine, "0");
      const blocked = engine.startPreTrade(order);
      expect(blocked.ok).toBe(false);
      expect(blocked.rejects.map(({ code }) => code)).toEqual([
        "AccountBlocked",
      ]);
    },
  );

  it("preserves the broker barrier on a duplicate broker update", () => {
    const engine = buildBroker();
    setPnl(engine, "-150");
    const barrier = new PnlBoundsBrokerBarrier(
      "USD",
      "-1000",
      undefined,
      RuleMode.ReviewWithHold,
    );

    try {
      engine.configure().pnlBoundsKillswitch(name, {
        brokerBarriers: [barrier, barrier],
      });
      throw new Error("expected validation error");
    } catch (error) {
      expectConfigureError(error, ConfigureErrorKind.Validation);
      expect((error as PolicyConfigureError).validationMessage).toContain(
        "duplicate broker barrier",
      );
    }

    expect(engine.startPreTrade(makeOrder()).rejects[0]?.code).toBe(
      "PnlKillSwitchTriggered",
    );
  });

  it("preserves both barrier axes on a duplicate account update", () => {
    const accountWithBarrier = 99_224_417n;
    const engine = Engine.builder()
      .builtin(
        buildPnlBoundsKillswitch()
          .brokerBarriers([
            new PnlBoundsBrokerBarrier("USD", "-100", undefined),
          ])
          .accountBarriers([
            new PnlBoundsAccountAssetBarrier(
              accountWithBarrier,
              "USD",
              "-75",
              "-50",
              undefined,
            ),
          ]),
      )
      .build();
    setPnl(engine, "-150");
    const barrier = new PnlBoundsAccountAssetBarrierUpdate(
      accountWithBarrier,
      "USD",
      "-1000",
      undefined,
    );

    try {
      engine.configure().pnlBoundsKillswitch(name, {
        brokerBarriers: [new PnlBoundsBrokerBarrier("USD", "-1000", undefined)],
        accountBarriers: [barrier, barrier],
      });
      throw new Error("expected validation error");
    } catch (error) {
      expectConfigureError(error, ConfigureErrorKind.Validation);
      expect((error as PolicyConfigureError).validationMessage).toContain(
        "duplicate account barrier",
      );
    }

    const accountOrder = makeOrder("1", "100", accountWithBarrier);
    const accountReject = engine.startPreTrade(accountOrder).rejects[0];
    expect(accountReject?.reason).toBe(
      "pnl kill switch triggered: account + asset barrier",
    );
    expect(accountReject?.code).toBe("PnlKillSwitchTriggered");

    const brokerReject = engine.startPreTrade(makeOrder()).rejects[0];
    expect(brokerReject?.reason).toBe(
      "pnl kill switch triggered: broker barrier",
    );
    expect(brokerReject?.code).toBe("PnlKillSwitchTriggered");
  });
});

function spotFundsPositionLimitEngine(): Engine {
  const engine = Engine.builder().builtin(buildSpotFunds()).build();
  engine.accounts().setCurrency(ACCOUNT, "USD");
  engine.configure().spotFunds(SpotFundsBuilder.NAME, {
    globalLimitMode: SpotFundsLimitMode.TrackOnly,
  });
  return engine;
}

describe("empty policy lifecycle", () => {
  it("cycles every rate-limit axis through empty settings", () => {
    const engine = Engine.builder()
      .builtin(buildRateLimit().assetBarriers([]))
      .build();
    const configure = engine.configure();
    const name = RateLimitBuilder.NAME;
    expect(engine.startPreTrade(makeOrder()).ok).toBe(true);

    const axes = [
      {
        update: (count: number) => ({
          brokerBarriers: [
            new RateLimitBrokerBarrier(new RateLimit(count, 60_000)),
          ],
        }),
        clear: { brokerBarriers: [] },
        account: ACCOUNT,
        settlement: "USD",
        resumes: false,
      },
      {
        update: (count: number) => ({
          assetBarriers: [
            new RateLimitAssetBarrier(new RateLimit(count, 60_000), "EUR"),
          ],
        }),
        clear: { assetBarriers: [] },
        account: ACCOUNT + 1n,
        settlement: "EUR",
        resumes: false,
      },
      {
        update: (count: number) => ({
          accountBarriers: [
            new RateLimitAccountBarrier(
              new RateLimit(count, 60_000),
              ACCOUNT + 2n,
            ),
          ],
        }),
        clear: { accountBarriers: [] },
        account: ACCOUNT + 2n,
        settlement: "GBP",
        resumes: true,
      },
      {
        update: (count: number) => ({
          accountAssetBarriers: [
            new RateLimitAccountAssetBarrier(
              new RateLimit(count, 60_000),
              ACCOUNT + 3n,
              "JPY",
            ),
          ],
        }),
        clear: { accountAssetBarriers: [] },
        account: ACCOUNT + 3n,
        settlement: "JPY",
        resumes: true,
      },
    ];
    for (const axis of axes) {
      const order = makeOrder("1", "100", axis.account);
      order.operation!.settlementAsset = axis.settlement;
      configure.rateLimit(name, axis.update(1));
      expect(engine.startPreTrade(order).ok).toBe(true);
      const rejected = engine.startPreTrade(order);
      expect(rejected.ok).toBe(false);
      expect(rejected.rejects[0]?.code).toBe("RateLimitExceeded");

      configure.rateLimit(name, {
        brokerBarriers: null,
        assetBarriers: null,
        accountBarriers: null,
        accountAssetBarriers: null,
      });
      const unchanged = engine.startPreTrade(order);
      expect(unchanged.ok).toBe(false);
      expect(unchanged.rejects[0]?.code).toBe("RateLimitExceeded");

      configure.rateLimit(name, axis.clear);
      for (let index = 0; index < 2; index += 1) {
        expect(engine.startPreTrade(order).ok).toBe(true);
      }

      // Account logs retain the accepted order and both rejected orders.
      configure.rateLimit(name, axis.update(axis.resumes ? 4 : 1));
      expect(engine.startPreTrade(order).ok).toBe(true);
      const reactivated = engine.startPreTrade(order);
      expect(reactivated.ok).toBe(false);
      expect(reactivated.rejects[0]?.code).toBe("RateLimitExceeded");
      configure.rateLimit(name, axis.clear);
      expect(engine.startPreTrade(order).ok).toBe(true);
    }

    configure.rateLimit(name, {
      brokerBarriers: [new RateLimitBrokerBarrier(new RateLimit(1, 60_000))],
    });
    expect(engine.startPreTrade(makeOrder()).ok).toBe(true);
    const reactivated = engine.startPreTrade(makeOrder());
    expect(reactivated.ok).toBe(false);
    expect(reactivated.rejects[0]?.code).toBe("RateLimitExceeded");
    configure.rateLimit(name, { brokerBarriers: [] });
    expect(engine.startPreTrade(makeOrder()).ok).toBe(true);
  });

  it("cycles every order-size-limit axis through empty settings", () => {
    const engine = Engine.builder()
      .builtin(buildOrderSizeLimit().assetBarriers([]))
      .build();
    const configure = engine.configure();
    const name = OrderSizeLimitBuilder.NAME;
    expect(engine.startPreTrade(makeOrder()).ok).toBe(true);

    const axes = [
      {
        update: {
          broker: new OrderSizeBrokerBarrier(
            new OrderSizeLimit("1", "1000000"),
          ),
        },
        clear: { clearBroker: true },
        account: ACCOUNT,
        underlying: "AAPL",
        settlement: "USD",
      },
      {
        update: {
          assetBarriers: [
            new OrderSizeAssetBarrier(
              new OrderSizeLimit("1", "1000000"),
              "MSFT",
            ),
          ],
        },
        clear: { assetBarriers: [] },
        account: ACCOUNT + 1n,
        underlying: "MSFT",
        settlement: "EUR",
      },
      {
        update: {
          accountAssetBarriers: [
            new OrderSizeAccountAssetBarrier(
              new OrderSizeLimit("1", "1000000"),
              ACCOUNT + 2n,
              "NVDA",
            ),
          ],
        },
        clear: { accountAssetBarriers: [] },
        account: ACCOUNT + 2n,
        underlying: "NVDA",
        settlement: "JPY",
      },
    ];
    for (const axis of axes) {
      const order = makeOrder("2", "100", axis.account);
      order.operation!.underlyingAsset = axis.underlying;
      order.operation!.settlementAsset = axis.settlement;
      configure.orderSizeLimit(name, axis.update);
      const rejected = engine.startPreTrade(order);
      expect(rejected.ok).toBe(false);
      expect(rejected.rejects[0]?.code).toBe("OrderQtyExceedsLimit");

      configure.orderSizeLimit(name, {
        broker: null,
        assetBarriers: null,
        accountAssetBarriers: null,
      });
      const unchanged = engine.startPreTrade(order);
      expect(unchanged.ok).toBe(false);
      expect(unchanged.rejects[0]?.code).toBe("OrderQtyExceedsLimit");

      configure.orderSizeLimit(name, axis.clear);
      for (let index = 0; index < 2; index += 1) {
        expect(engine.startPreTrade(order).ok).toBe(true);
      }
    }

    configure.orderSizeLimit(name, {
      broker: new OrderSizeBrokerBarrier(new OrderSizeLimit("1", "1000000")),
    });
    const reactivated = engine.startPreTrade(makeOrder("2"));
    expect(reactivated.ok).toBe(false);
    expect(reactivated.rejects[0]?.code).toBe("OrderQtyExceedsLimit");
    configure.orderSizeLimit(name, { clearBroker: true });
    expect(engine.startPreTrade(makeOrder("2")).ok).toBe(true);
  });
});

function commitSpotFundsHold(
  engine: Engine,
  operation: Pick<
    NonNullable<ReturnType<Engine["executePreTrade"]>["reservation"]>,
    "lock" | "accountAdjustments" | "commit"
  >,
): void {
  expect(operation.lock().pricesOf(0).map(String)).toEqual(["100"]);
  const entries = operation.accountAdjustments().map(({ entry }) => entry);
  expect(entries).toHaveLength(2);
  const usd = entries.find(({ asset }) => asset === "USD")!;
  expect(usd.balance!.absolute.toString()).toBe("-200");
  expect(usd.balance!.delta.toString()).toBe("-200");
  expect(usd.held!.absolute.toString()).toBe("200");
  expect(usd.held!.delta.toString()).toBe("200");
  expect(
    entries
      .find(({ asset }) => asset === "AAPL")!
      .incoming!.absolute.toString(),
  ).toBe("2");
  operation.commit();
  expect(holdings(engine, ACCOUNT, "USD")).toEqual({
    balance: "-200",
    held: "200",
    incoming: "0",
  });
  expect(holdings(engine, ACCOUNT, "AAPL")).toEqual({
    balance: "0",
    held: "0",
    incoming: "2",
  });
}

describe("runtime configurator", () => {
  it("retunes a built-in rate-limit policy", () => {
    // Source: https://wiki.openpit.dev/Dynamic-Policy-Reconfiguration/
    // - Retune a Built-in Policy
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

    // Register the rate-limit policy through builtin so the engine keeps a
    // handle to its settings; built-in policies are configurable by name.
    const engine = Engine.builder()

      .builtin(
        buildRateLimit().brokerBarriers([
          new RateLimitBrokerBarrier(new RateLimit(5, 60_000)),
        ]),
      )
      .build();

    // The generous limit of 5 admits the first three orders.
    for (let i = 0; i < 3; i += 1) {
      const result = engine.executePreTrade(order());
      if (!result.ok) {
        throw new Error("unexpected rejects");
      }
      const reservation = result.reservation;
      if (reservation === undefined) {
        throw new Error("accepted execute result is missing its reservation");
      }
      reservation.commit();
    }

    // Tighten the broker limit to 2 at runtime, without rebuilding the engine.
    // Built-in policies register under their type name (RateLimitBuilder.NAME).
    engine.configure().rateLimit(RateLimitBuilder.NAME, {
      brokerBarriers: [new RateLimitBrokerBarrier(new RateLimit(2, 60_000))],
    });

    // The next order would have passed under the old limit of 5; the new limit
    // of 2 rejects it, proving the live policy reads the retuned value.
    const rejected = engine.executePreTrade(order());
    console.log(rejected.rejects[0]!.reason); // "rate limit exceeded: broker barrier"

    expect(rejected.ok).toBe(false);
    expect(rejected.rejects[0]!.reason).toBe(
      "rate limit exceeded: broker barrier",
    );
  });

  it("reports stable configure-error kinds", () => {
    const engine = Engine.builder()

      .builtin(
        buildRateLimit().brokerBarriers([
          new RateLimitBrokerBarrier(new RateLimit(1, 60_000)),
        ]),
      )
      .build();

    try {
      engine.configure().rateLimit("NoSuchPolicy", {
        brokerBarriers: [new RateLimitBrokerBarrier(new RateLimit(2, 60_000))],
      });
      throw new Error("expected unknown policy error");
    } catch (error) {
      expectConfigureError(error, ConfigureErrorKind.Unknown);
      expect((error as PolicyConfigureError).policyName).toBe("NoSuchPolicy");
    }

    try {
      engine.configure().rateLimit(RateLimitBuilder.NAME, {
        brokerBarriers: [new RateLimitBrokerBarrier(new RateLimit(1, 0))],
      });
      throw new Error("expected validation error");
    } catch (error) {
      expectConfigureError(error, ConfigureErrorKind.Validation);
      expect((error as PolicyConfigureError).policyName).toBe(
        RateLimitBuilder.NAME,
      );
      expect((error as PolicyConfigureError).validationMessage).not.toBe("");
    }
  });

  it("rejects conflicting order-size broker set and clear inputs", () => {
    const sizeEngine = Engine.builder()
      .builtin(
        buildOrderSizeLimit().brokerBarrier(
          new OrderSizeBrokerBarrier(new OrderSizeLimit("1", "1000000")),
        ),
      )
      .build();
    try {
      sizeEngine.configure().orderSizeLimit(OrderSizeLimitBuilder.NAME, {
        broker: new OrderSizeBrokerBarrier(new OrderSizeLimit("2", "1000000")),
        clearBroker: true,
      });
      throw new Error("expected validation error");
    } catch (error) {
      expect(error).toBeInstanceOf(ParamError);
      expect((error as Error).message).toBe(
        "broker and clearBroker cannot be used together",
      );
    }
  });

  it("clears a rate-limit broker barrier at runtime", () => {
    const engine = Engine.builder()
      .builtin(
        buildRateLimit()
          .brokerBarriers([
            new RateLimitBrokerBarrier(new RateLimit(1, 60_000)),
          ])
          .assetBarriers([
            new RateLimitAssetBarrier(new RateLimit(10, 60_000), "USD"),
          ]),
      )
      .build();

    const configure = engine.configure();
    const name = RateLimitBuilder.NAME;
    const misspelled = { brokerBarriers: [], brokerBarrier: undefined };
    expectUnknownOption(
      () => configure.rateLimit(name, misspelled),
      "rateLimit options",
      "brokerBarrier",
    );

    const options = { brokerBarriers: [], [Symbol("metadata")]: true };
    Object.setPrototypeOf(options, { inherited: true });
    Object.defineProperty(options, "hidden", { value: true });
    configure.rateLimit(name, options);

    for (let index = 0; index < 2; index += 1) {
      const result = engine.executePreTrade(makeOrder());
      expect(result.ok).toBe(true);
      result.reservation!.commit();
    }
  });

  it("propagates a throwing ownKeys trap before reading a rate-limit field", () => {
    const failure = new Error("could not enumerate options");
    const options = new Proxy(
      {
        get brokerBarriers(): never {
          throw new Error("read broker barriers before enumerating keys");
        },
      },
      {
        ownKeys() {
          throw failure;
        },
      },
    );
    const engine = Engine.builder()
      .builtin(buildRateLimit().brokerBarriers([]))
      .build();

    let caught: unknown;
    try {
      engine.configure().rateLimit(RateLimitBuilder.NAME, options);
    } catch (error) {
      caught = error;
    }
    expect(caught).toBe(failure);
  });

  it("leaves a rate-limit broker barrier unchanged when omitted", () => {
    const engine = Engine.builder()
      .builtin(
        buildRateLimit().brokerBarriers([
          new RateLimitBrokerBarrier(new RateLimit(1, 60_000)),
        ]),
      )
      .build();

    engine.configure().rateLimit(RateLimitBuilder.NAME, {});

    const first = engine.executePreTrade(makeOrder());
    expect(first.ok).toBe(true);
    first.reservation!.commit();

    expect(engine.executePreTrade(makeOrder()).ok).toBe(false);
  });

  it("can clear an order-size broker barrier at runtime", () => {
    const engine = Engine.builder()

      .builtin(
        buildOrderSizeLimit()
          .brokerBarrier(
            new OrderSizeBrokerBarrier(new OrderSizeLimit("1", "1000000")),
          )
          .assetBarriers([
            new OrderSizeAssetBarrier(
              new OrderSizeLimit("10", "1000000"),
              "AAPL",
            ),
          ]),
      )
      .build();

    const oversized = engine.startPreTrade(makeOrder("2"));
    expect(oversized.ok).toBe(false);
    expect(oversized.rejects[0]!.code).toBe("OrderQtyExceedsLimit");

    const misspelled = { clearBroker: true, clearBrokers: true };
    expectUnknownOption(
      () =>
        engine
          .configure()
          .orderSizeLimit(OrderSizeLimitBuilder.NAME, misspelled),
      "orderSizeLimit options",
      "clearBrokers",
    );
    expect(engine.startPreTrade(makeOrder("2")).ok).toBe(false);

    engine.configure().orderSizeLimit(OrderSizeLimitBuilder.NAME, {
      clearBroker: true,
    });

    expect(engine.startPreTrade(makeOrder("2")).ok).toBe(true);
  });

  it("preserves absent and zero order-size caps at runtime", () => {
    const engine = Engine.builder()
      .builtin(
        buildOrderSizeLimit().brokerBarrier(
          new OrderSizeBrokerBarrier(new OrderSizeLimit("10", undefined)),
        ),
      )
      .build();

    engine.configure().orderSizeLimit(OrderSizeLimitBuilder.NAME, {
      broker: new OrderSizeBrokerBarrier(new OrderSizeLimit(undefined, "1000")),
    });
    expect(engine.startPreTrade(makeOrder("1", "1000")).ok).toBe(true);
    const aboveNotional = engine.startPreTrade(makeOrder("1", "1001"));
    expect(aboveNotional.ok).toBe(false);
    expect(aboveNotional.rejects[0]?.code).toBe("OrderNotionalExceedsLimit");

    engine.configure().orderSizeLimit(OrderSizeLimitBuilder.NAME, {
      broker: new OrderSizeBrokerBarrier(new OrderSizeLimit("0", undefined)),
    });
    const zeroQuantity = engine.startPreTrade(makeOrder("1", "1"));
    expect(zeroQuantity.ok).toBe(false);
    expect(zeroQuantity.rejects[0]?.code).toBe("OrderQtyExceedsLimit");
  });

  it("preserves absent and zero settlement-asset caps at runtime", () => {
    const engine = Engine.builder()
      .builtin(
        buildOrderSizeLimit()
          .assetBarriers([
            new OrderSizeAssetBarrier(
              new OrderSizeLimit(undefined, "5000"),
              "USD",
            ),
          ])
          .accountAssetBarriers([
            new OrderSizeAccountAssetBarrier(
              new OrderSizeLimit(undefined, "6000"),
              ORDER_SIZE_OVERRIDE_ACCOUNT,
              "USD",
            ),
          ]),
      )
      .build();

    engine.configure().orderSizeLimit(OrderSizeLimitBuilder.NAME, {
      assetBarriers: [
        new OrderSizeAssetBarrier(new OrderSizeLimit(undefined, "1000"), "USD"),
      ],
      accountAssetBarriers: [
        new OrderSizeAccountAssetBarrier(
          new OrderSizeLimit(undefined, "2000"),
          ORDER_SIZE_OVERRIDE_ACCOUNT,
          "USD",
        ),
      ],
    });

    expect(engine.startPreTrade(makeOrder("1000", "1")).ok).toBe(true);
    const aboveAsset = engine.startPreTrade(makeOrder("1001", "1"));
    expect(aboveAsset.ok).toBe(false);
    expect(aboveAsset.rejects[0]?.code).toBe("OrderNotionalExceedsLimit");
    expect(
      engine.startPreTrade(makeOrder("2000", "1", ORDER_SIZE_OVERRIDE_ACCOUNT))
        .ok,
    ).toBe(true);
    const aboveAccountAsset = engine.startPreTrade(
      makeOrder("2001", "1", ORDER_SIZE_OVERRIDE_ACCOUNT),
    );
    expect(aboveAccountAsset.ok).toBe(false);
    expect(aboveAccountAsset.rejects[0]?.code).toBe(
      "OrderNotionalExceedsLimit",
    );
    expect(
      engine.startPreTrade(makeOrder("2000", "1", ORDER_SIZE_OVERRIDE_ACCOUNT))
        .ok,
    ).toBe(true);

    engine.configure().orderSizeLimit(OrderSizeLimitBuilder.NAME, {
      assetBarriers: [
        new OrderSizeAssetBarrier(new OrderSizeLimit(undefined, "0"), "USD"),
      ],
      accountAssetBarriers: [
        new OrderSizeAccountAssetBarrier(
          new OrderSizeLimit(undefined, "0"),
          ORDER_SIZE_OVERRIDE_ACCOUNT,
          "USD",
        ),
      ],
    });
    const zeroAsset = engine.startPreTrade(makeOrder("1", "1"));
    expect(zeroAsset.ok).toBe(false);
    expect(zeroAsset.rejects[0]?.code).toBe("OrderNotionalExceedsLimit");
    const zeroAccountAsset = engine.startPreTrade(
      makeOrder("1", "1", ORDER_SIZE_OVERRIDE_ACCOUNT),
    );
    expect(zeroAccountAsset.ok).toBe(false);
    expect(zeroAccountAsset.rejects[0]?.code).toBe("OrderNotionalExceedsLimit");
  });

  // These two look like tautologies - the order is under every visible cap -
  // and they are not. Quantity resolves by the instrument's underlying asset
  // (AAPL) and notional by its settlement asset (USD), so the quantity-only USD
  // barrier and the notional-only AAPL barrier each leave absent exactly the
  // cap its own key would make the engine consult. An absent cap turned into a
  // zero cap would land on that live chain and reject. Keep the asymmetry: one
  // barrier carrying both caps, or both keyed on one asset, silently removes
  // the check.
  it("preserves absent caps on the chains fed by configured asset keys", () => {
    const engine = Engine.builder()
      .builtin(
        buildOrderSizeLimit().brokerBarrier(
          new OrderSizeBrokerBarrier(new OrderSizeLimit("100", "100000")),
        ),
      )
      .build();

    engine.configure().orderSizeLimit(OrderSizeLimitBuilder.NAME, {
      assetBarriers: [
        new OrderSizeAssetBarrier(new OrderSizeLimit("10", undefined), "USD"),
        new OrderSizeAssetBarrier(
          new OrderSizeLimit(undefined, "1000"),
          "AAPL",
        ),
      ],
    });

    expect(engine.startPreTrade(makeOrder("5", "100")).ok).toBe(true);
  });

  it("preserves absent caps on the chains fed by configured account-asset keys", () => {
    const engine = Engine.builder()
      .builtin(
        buildOrderSizeLimit().brokerBarrier(
          new OrderSizeBrokerBarrier(new OrderSizeLimit("100", "100000")),
        ),
      )
      .build();

    engine.configure().orderSizeLimit(OrderSizeLimitBuilder.NAME, {
      accountAssetBarriers: [
        new OrderSizeAccountAssetBarrier(
          new OrderSizeLimit("10", undefined),
          ORDER_SIZE_OVERRIDE_ACCOUNT,
          "USD",
        ),
        new OrderSizeAccountAssetBarrier(
          new OrderSizeLimit(undefined, "1000"),
          ORDER_SIZE_OVERRIDE_ACCOUNT,
          "AAPL",
        ),
      ],
    });

    expect(
      engine.startPreTrade(makeOrder("5", "100", ORDER_SIZE_OVERRIDE_ACCOUNT))
        .ok,
    ).toBe(true);
  });

  it("force-sets accumulated generic pnl", () => {
    // Source: https://wiki.openpit.dev/Dynamic-Policy-Reconfiguration/
    // - Force-set Accumulated P&L
    const accountId = 99_224_416n;
    const order = (): OrderInit => ({
      operation: {
        underlyingAsset: "AAPL",
        settlementAsset: "USD",
        accountId,
        side: "BUY",
        tradeAmount: TradeAmount.quantity("1"),
        price: "100",
      },
    });

    // Register the kill-switch policy through builtin so the engine keeps a
    // handle to its accumulator; built-in policies are configurable by name.
    const engine = Engine.builder()

      .builtin(
        buildPnlBoundsKillswitch().brokerBarriers([
          new PnlBoundsBrokerBarrier("USD", "-100", undefined),
        ]),
      )
      .build();

    // With no P&L history the order passes against the lower bound of -100.
    const first = engine.executePreTrade(order());
    if (!first.ok) {
      throw new Error("unexpected rejects");
    }
    const firstReservation = first.reservation;
    if (firstReservation === undefined) {
      throw new Error("accepted execute result is missing its reservation");
    }
    firstReservation.commit();

    // Force-set the account's accumulated P&L to -150 USD, below the bound.
    // Built-in policies register under their type name
    // (PnlBoundsKillswitchBuilder.NAME).
    engine.configure().setAccountPnl(PnlBoundsKillswitchBuilder.NAME, {
      account: accountId,
      settlementAsset: "USD",
      pnl: "-150",
    });

    // The next order for that account breaches the lower bound and is rejected;
    // the breach also latches an engine-level block on the account.
    const rejected = engine.startPreTrade(order());
    console.log(rejected.rejects[0]!.reason); // "pnl kill switch triggered: broker barrier"

    expect(rejected.ok).toBe(false);
    expect(rejected.rejects[0]!.reason).toBe(
      "pnl kill switch triggered: broker barrier",
    );
  });

  it.each([
    [SpotFundsLimitMode.ReviewWithHold, RuleMode.ReviewWithHold, 1],
    [SpotFundsLimitMode.ReviewWithoutHold, RuleMode.ReviewWithoutHold, 0],
  ] as const)(
    "marks a shortfall under funds mode %s",
    (mode, reviewMode, remainingReviews) => {
      const engine = Engine.builder().builtin(buildSpotFunds()).build();
      engine.accounts().setCurrency(ACCOUNT, "USD");
      expect(
        engine.applyAccountAdjustment(ACCOUNT, [
          {
            operation: { asset: "USD" },
            amount: { balance: AdjustmentAmount.absolute("1000") },
          },
        ]).ok,
      ).toBe(true);
      engine
        .configure()
        .spotFunds(SpotFundsBuilder.NAME, { globalLimitMode: mode });
      const checkOrder = (
        quantity: string,
        reviewCount: number,
        commit = false,
      ) => {
        const result = engine.executePreTrade(makeOrder(quantity));
        expect(result.ok, "funds review must pass").toBe(true);
        expect(result.rejects).toHaveLength(0);
        expect(result.reservation).toBeDefined();
        let committed = false;
        try {
          expect(
            result.reviews,
            "funds review count reflects the hold",
          ).toHaveLength(reviewCount);
          if (reviewCount > 0) {
            expect(result.reviews[0]?.code).toBe("InsufficientFunds");
            expect(result.reviews[0]?.mode).toBe(reviewMode);
          }
          if (commit) {
            result.reservation!.commit();
            committed = true;
          }
        } finally {
          if (!committed) {
            result.reservation?.rollback();
          }
        }
      };
      checkOrder("1", 0);
      checkOrder("11", 1, true);
      checkOrder("1", remainingReviews);
    },
  );

  it.each([
    SpotFundsLimitMode.ReviewWithHold,
    SpotFundsLimitMode.ReviewWithoutHold,
  ])(
    "records a confirmed hold without alreadyHeld in %s funds mode",
    (mode) => {
      const engine = spotFundsPositionLimitEngine();
      engine
        .configure()
        .spotFunds(SpotFundsBuilder.NAME, { globalLimitMode: mode });
      const input = makeOrder("2");
      const pending = engine.executePreTradeDryRun(input);
      expect(pending.reviews).toHaveLength(1);
      expect(pending.reviews[0]?.code).toBe("InsufficientFunds");
      const confirmed = { ...input, review: { confirmed: true } };
      const dry = engine.executePreTradeDryRun(confirmed);
      expect(dry.isPass).toBe(true);
      expect(dry.reviews).toEqual([]);
      const result = engine.executePreTrade(confirmed);
      expect(result.ok).toBe(true);
      expect(result.reviews).toEqual([]);
      expect(result.rejects).toEqual([]);
      const reservation = result.reservation!;
      commitSpotFundsHold(engine, reservation);
    },
  );

  it.each([
    SpotFundsLimitMode.ReviewWithHold,
    SpotFundsLimitMode.ReviewWithoutHold,
  ])(
    "tracks a drop-copy past funds and position limits in %s funds mode",
    (mode) => {
      const engine = spotFundsPositionLimitEngine();
      engine.configure().spotFunds(SpotFundsBuilder.NAME, {
        globalLimitMode: mode,
        positionLimits: [
          {
            accountId: ACCOUNT,
            asset: "AAPL",
            limit: "1",
            mode: RuleMode.Reject,
          },
        ],
      });
      const input = makeOrder("2");
      const ordinary = engine.executePreTradeDryRun(input);
      expect(ordinary.isPass).toBe(false);
      expect(ordinary.rejects).toHaveLength(1);
      expect(ordinary.rejects[0]).toMatchObject({
        code: "PositionLimitExceeded",
        mode: RuleMode.Reject,
      });
      const result = engine.applyDropCopy(input);
      expect(result.ok).toBe(true);
      expect(result.rejects).toEqual([]);
      const operation = result.operation!;
      expect(operation.accountBlock()).toBeUndefined();
      commitSpotFundsHold(engine, operation);
    },
  );

  it.each([
    [
      SpotFundsLimitMode.ReviewWithHold,
      SpotFundsLimitMode.ReviewWithoutHold,
      SpotFundsLimitMode.Enforce,
    ],
    [
      SpotFundsLimitMode.ReviewWithoutHold,
      SpotFundsLimitMode.Enforce,
      SpotFundsLimitMode.ReviewWithHold,
    ],
    [
      SpotFundsLimitMode.Enforce,
      SpotFundsLimitMode.ReviewWithHold,
      SpotFundsLimitMode.ReviewWithoutHold,
    ],
  ] as const)(
    "cascades account, group and global funds modes from %s",
    (globalMode, groupMode, accountMode) => {
      const engine = spotFundsPositionLimitEngine();
      engine.accounts().registerGroup([ACCOUNT], 42);
      engine.configure().spotFunds(SpotFundsBuilder.NAME, {
        globalLimitMode: globalMode,
        accountGroupLimitModes: [{ accountGroupId: 42, mode: groupMode }],
        accountLimitModes: [{ accountId: ACCOUNT, mode: accountMode }],
      });
      const check = (mode: SpotFundsLimitMode) => {
        const result = engine.executePreTrade(makeOrder("2"));
        if (mode === SpotFundsLimitMode.Enforce) {
          expect(result.ok).toBe(false);
          expect(result.reservation).toBeUndefined();
          expect(result.rejects).toHaveLength(1);
          expect(result.rejects[0]).toMatchObject({
            code: "InsufficientFunds",
            mode: RuleMode.Reject,
          });
          return;
        }
        expect(result.ok).toBe(true);
        expect(result.reviews).toHaveLength(1);
        expect(result.reviews[0]).toMatchObject({
          code: "InsufficientFunds",
          mode,
        });
        const reservation = result.reservation!;
        if (mode === SpotFundsLimitMode.ReviewWithHold) {
          expect(reservation.lock().pricesOf(0).map(String)).toEqual(["100"]);
          const usd = reservation
            .accountAdjustments()
            .find(({ entry }) => entry.asset === "USD")!.entry;
          expect(usd.balance!.absolute.toString()).toBe("-200");
          expect(usd.held!.absolute.toString()).toBe("200");
        } else {
          expect(reservation.lock().size()).toBe(0);
          expect(reservation.accountAdjustments()).toEqual([]);
        }
        reservation.rollback();
      };
      check(accountMode);
      engine.configure().spotFunds(SpotFundsBuilder.NAME, {
        accountLimitModes: [{ accountId: ACCOUNT, mode: null }],
      });
      check(groupMode);
      engine.configure().spotFunds(SpotFundsBuilder.NAME, {
        accountGroupLimitModes: [{ accountGroupId: 42, mode: null }],
      });
      check(globalMode);
    },
  );

  it.each([
    [
      "Unknown",
      RangeError,
      'limit mode must be "Enforce", "TrackOnly", "ReviewWithHold" or "ReviewWithoutHold"',
    ],
    [
      "reviewwithhold",
      RangeError,
      'limit mode must be "Enforce", "TrackOnly", "ReviewWithHold" or "ReviewWithoutHold"',
    ],
    [4, TypeError, "limit mode must be a string"],
  ] as const)("rejects invalid funds mode %s", (mode, error, message) => {
    const engine = Engine.builder().builtin(buildSpotFunds()).build();
    const configureMode = () =>
      engine.configure().spotFunds(SpotFundsBuilder.NAME, {
        globalLimitMode: mode as never,
      });
    expect(configureMode).toThrow(error);
    expect(configureMode).toThrow(message);
  });

  it("switches spot funds global limit mode at runtime", () => {
    // Source: https://wiki.openpit.dev/Dynamic-Policy-Reconfiguration/
    // - Spot Funds: Global Limit Mode
    const accountId = 99_224_416n;
    const engine = Engine.builder().builtin(buildSpotFunds()).build();

    // Seed 1 000 USD - not enough for 10 AAPL @ 200 (= 2 000 notional).
    engine.applyAccountAdjustment(accountId, [
      {
        operation: { asset: "USD" },
        amount: { balance: AdjustmentAmount.absolute("1000") },
      },
    ]);

    const order = (): OrderInit => ({
      operation: {
        underlyingAsset: "AAPL",
        settlementAsset: "USD",
        accountId,
        side: "BUY",
        tradeAmount: TradeAmount.quantity("10"),
        price: "200",
      },
    });

    // Default Enforce: 2 000 notional exceeds 1 000 available - rejected.
    let result = engine.executePreTrade(order());
    console.log(result.rejects[0]!.reason); // "spot funds insufficient"

    // Switch to TrackOnly: the same order now passes and reserves against
    // deficit.
    engine.configure().spotFunds(SpotFundsBuilder.NAME, {
      globalLimitMode: SpotFundsLimitMode.TrackOnly,
    });
    result = engine.executePreTrade(order());
    if (!result.ok) {
      throw new Error("unexpected rejects");
    }
    const reservation = result.reservation;
    if (reservation === undefined) {
      throw new Error("accepted execute result is missing its reservation");
    }
    reservation.commit(); // available: 1 000 - 2 000 = -1 000

    // Restore Enforce: available is negative - still rejected.
    engine.configure().spotFunds(SpotFundsBuilder.NAME, {
      globalLimitMode: SpotFundsLimitMode.Enforce,
    });
    result = engine.executePreTrade(order());
    console.log(result.rejects[0]!.reason); // "spot funds insufficient"

    expect(result.ok).toBe(false);
    expect(result.rejects[0]!.reason).toBe("spot funds insufficient");
  });

  it("switches spot funds per-account limit mode at runtime", () => {
    // Source: https://wiki.openpit.dev/Dynamic-Policy-Reconfiguration/
    // - Spot Funds: Per-Account Limit Mode
    const accountId = 99_224_416n;
    const engine = Engine.builder().builtin(buildSpotFunds()).build();

    // Seed 1 000 USD - not enough for 10 AAPL @ 200 (= 2 000 notional).
    engine.applyAccountAdjustment(accountId, [
      {
        operation: { asset: "USD" },
        amount: { balance: AdjustmentAmount.absolute("1000") },
      },
    ]);

    const order = (): OrderInit => ({
      operation: {
        underlyingAsset: "AAPL",
        settlementAsset: "USD",
        accountId,
        side: "BUY",
        tradeAmount: TradeAmount.quantity("10"),
        price: "200",
      },
    });

    // Global Enforce: under-funded buy is rejected.
    let result = engine.executePreTrade(order());
    console.log(result.rejects[0]!.reason); // "spot funds insufficient"

    // Pin this account to TrackOnly: per-account override wins over global
    // Enforce.
    engine.configure().spotFunds(SpotFundsBuilder.NAME, {
      accountLimitModes: [{ accountId, mode: SpotFundsLimitMode.TrackOnly }],
    });
    result = engine.executePreTrade(order());
    if (!result.ok) {
      throw new Error("unexpected rejects");
    }
    const reservation = result.reservation;
    if (reservation === undefined) {
      throw new Error("accepted execute result is missing its reservation");
    }
    reservation.commit(); // reservation recorded despite insufficient funds

    // Clear the per-account override: cascade falls back to global Enforce.
    engine.configure().spotFunds(SpotFundsBuilder.NAME, {
      accountLimitModes: [{ accountId, mode: null }],
    });
    result = engine.executePreTrade(order());
    console.log(result.rejects[0]!.reason); // "spot funds insufficient"

    expect(result.ok).toBe(false);
    expect(result.rejects[0]!.reason).toBe("spot funds insufficient");
  });

  it("rejects a misspelled spot-funds option before changing the policy", () => {
    const engine = Engine.builder().builtin(buildSpotFunds()).build();
    const configure = engine.configure();
    const name = SpotFundsBuilder.NAME;
    const options = { globalLimitMode: SpotFundsLimitMode.TrackOnly };
    const invalid = { ...options, globalLimitModes: undefined };

    expectUnknownOption(
      () => configure.spotFunds(name, invalid),
      "spotFunds options",
      "globalLimitModes",
    );
    expect(engine.executePreTradeDryRun(makeOrder()).isPass).toBe(false);
    configure.spotFunds(name, options);
    expect(engine.executePreTradeDryRun(makeOrder()).isPass).toBe(true);
  });

  it.each([
    ["globalSlippageBps", 65_536, RangeError],
    ["pricingSource", "Mid", RangeError],
    [
      "overrides",
      [{}],
      expect.objectContaining({
        constructor: TypeError,
        message: "overrides contains an invalid entry",
      }),
    ],
  ] as const)(
    "rejects an invalid spot-funds %s update",
    (key, value, error) => {
      const engine = Engine.builder().builtin(buildSpotFunds()).build();

      expect(() =>
        engine.configure().spotFunds(SpotFundsBuilder.NAME, { [key]: value }),
      ).toThrow(error);
    },
  );

  it("retunes slippage, overrides, and pricing for the next market order", () => {
    const builder = Engine.builder();
    const marketData = builder.marketData(QuoteTtl.infinite()).build();
    const instrumentId = marketData.register({
      underlyingAsset: "AAPL",
      settlementAsset: "USD",
    }).value;
    marketData.push(instrumentId, { mark: "100", bid: "90", ask: "110" }, 0);
    const engine = builder
      .builtin(buildSpotFunds().marketData(marketData, 0, "Mark", undefined))
      .build();
    const adjustment = engine.applyAccountAdjustment(ACCOUNT, [
      {
        operation: { asset: "USD" },
        amount: { balance: AdjustmentAmount.absolute("105") },
      },
    ]);
    expect(adjustment.ok).toBe(true);

    const order: OrderInit = {
      operation: {
        underlyingAsset: "AAPL",
        settlementAsset: "USD",
        accountId: ACCOUNT,
        side: "BUY",
        tradeAmount: TradeAmount.quantity("1"),
      },
    };
    const configure = engine.configure();
    const name = SpotFundsBuilder.NAME;

    expect(engine.executePreTradeDryRun(order).isPass).toBe(true);
    configure.spotFunds(name, { globalSlippageBps: 1_000 });
    expect(engine.executePreTradeDryRun(order).isPass).toBe(false);
    configure.spotFunds(name, {
      overrides: [new SpotFundsOverride(instrumentId, null, null, 0)],
    });
    expect(engine.executePreTradeDryRun(order).isPass).toBe(true);
    configure.spotFunds(name, { pricingSource: "BookTop" });
    expect(engine.executePreTradeDryRun(order).isPass).toBe(false);
  });

  it.each([
    {
      field: "accountLimitModes",
      identity: { accountId: ACCOUNT },
    },
    {
      field: "accountGroupLimitModes",
      identity: { accountGroupId: 7 },
    },
  ] as const)("rejects a misspelled mode in $field", ({ field, identity }) => {
    const engine = Engine.builder().builtin(buildSpotFunds()).build();
    engine.accounts().registerGroup([ACCOUNT], 7);
    const configure = engine.configure();
    const name = SpotFundsBuilder.NAME;
    const mode = SpotFundsLimitMode.TrackOnly;
    const invalid = { [field]: [{ ...identity, modes: mode }] };

    expect(() => configure.spotFunds(name, { [field]: [false] })).toThrow(
      expect.objectContaining({
        name: "TypeError",
        message: `${field} must be a plain object`,
      }),
    );
    expectUnknownOption(
      () => configure.spotFunds(name, invalid),
      `${field} entry`,
      "modes",
    );
    expect(engine.executePreTradeDryRun(makeOrder()).isPass).toBe(false);
    configure.spotFunds(name, { [field]: [{ ...identity, mode }] });
    expect(engine.executePreTradeDryRun(makeOrder()).isPass).toBe(true);
  });

  it("reconfigures a spot-funds position limit for the next order", () => {
    const engine = Engine.builder().builtin(buildSpotFunds()).build();
    const adjustment = engine.applyAccountAdjustment(ACCOUNT, [
      {
        operation: { asset: "USD" },
        amount: { balance: AdjustmentAmount.absolute("1000") },
      },
    ]);
    expect(adjustment.ok).toBe(true);

    const entry = { accountId: ACCOUNT, asset: "AAPL", limit: "1" };
    const invalid = { positionLimits: [{ ...entry, limits: "1" }] };
    expectUnknownOption(
      () => engine.configure().spotFunds(SpotFundsBuilder.NAME, invalid),
      "positionLimits entry",
      "limits",
    );
    expect(engine.executePreTradeDryRun(makeOrder("2")).isPass).toBe(true);

    engine.configure().spotFunds(SpotFundsBuilder.NAME, {
      positionLimits: [entry],
    });
    const rejected = engine.executePreTrade(makeOrder("2"));
    expect(rejected.ok).toBe(false);
    expect(rejected.rejects[0]?.code).toBe("PositionLimitExceeded");
    expect(rejected.rejects[0]?.scope).toBe("order");

    const within = engine.executePreTrade(makeOrder("1"));
    expect(within.ok).toBe(true);
    within.reservation?.rollback();

    engine.configure().spotFunds(SpotFundsBuilder.NAME, {
      positionLimits: [{ accountId: ACCOUNT, asset: "AAPL", limit: "3" }],
    });
    const withinReplacement = engine.executePreTrade(makeOrder("2"));
    expect(withinReplacement.ok).toBe(true);
    withinReplacement.reservation?.rollback();
    const beyond = engine.executePreTrade(makeOrder("4"));
    expect(beyond.ok).toBe(false);
    expect(beyond.rejects[0]?.code).toBe("PositionLimitExceeded");
    expect(beyond.rejects[0]?.scope).toBe("order");

    engine.configure().spotFunds(SpotFundsBuilder.NAME, {
      positionLimits: [{ accountId: ACCOUNT, asset: "AAPL", limit: null }],
    });
    const cleared = engine.executePreTrade(makeOrder("4"));
    expect(cleared.ok).toBe(true);
    cleared.reservation?.rollback();
  });

  it.each(["ReviewWithHold", "ReviewWithoutHold"] as const)(
    "keeps Reject and %s position limits independent",
    (mode) => {
      const engine = spotFundsPositionLimitEngine();
      const reviewing = {
        accountId: ACCOUNT,
        asset: "AAPL",
        mode,
        limit: "0",
      };
      const checkOrder = (
        quantity: string,
        passed: boolean,
        reviewCount: number,
      ) => {
        const result = engine.executePreTrade(makeOrder(quantity));
        try {
          expect(result.ok, "review passes and Reject rejects").toBe(passed);
          if (passed) {
            expect(result.rejects).toEqual([]);
            expect(result.reviews).toHaveLength(reviewCount);
            if (reviewCount > 0) {
              expect(result.reviews[0]?.mode).toBe(mode);
              expect(result.reviews[0]?.code).toBe("PositionLimitExceeded");
              expect(result.reviews[0]?.scope).toBe("order");
            }
          } else {
            expect(result.rejects).toHaveLength(1);
            expect(result.rejects[0]?.mode).toBe("Reject");
            expect(result.rejects[0]?.code).toBe("PositionLimitExceeded");
          }
        } finally {
          result.reservation?.rollback();
        }
      };
      const rejecting = { accountId: ACCOUNT, asset: "AAPL", limit: "2" };
      for (const entry of [
        rejecting,
        { ...rejecting, mode: null },
        { ...rejecting, mode: "Reject" as const },
      ]) {
        engine.configure().spotFunds(SpotFundsBuilder.NAME, {
          positionLimits: [entry, reviewing],
        });
        checkOrder("1", true, 1);
        checkOrder("3", false, 0);
      }
      const confirmed = { ...makeOrder("1"), review: { confirmed: true } };
      const approvedDry = engine.executePreTradeDryRun(confirmed);
      expect(approvedDry.isPass).toBe(true);
      expect(approvedDry.reviews).toEqual([]);
      const approved = engine.executePreTrade(confirmed);
      expect(approved.ok).toBe(true);
      expect(approved.reviews).toEqual([]);
      approved.reservation!.rollback();
      const beyond = { ...makeOrder("3"), review: { confirmed: true } };
      const rejectedDry = engine.executePreTradeDryRun(beyond);
      expect(rejectedDry.isPass).toBe(false);
      expect(rejectedDry.reviews).toEqual([]);
      expect(rejectedDry.rejects).toHaveLength(1);
      expect(rejectedDry.rejects[0]).toMatchObject({
        code: "PositionLimitExceeded",
        mode: RuleMode.Reject,
      });
      const rejected = engine.executePreTrade(beyond);
      expect(rejected.ok).toBe(false);
      expect(rejected.rejects).toHaveLength(1);
      expect(rejected.rejects[0]).toMatchObject({
        code: "PositionLimitExceeded",
        mode: RuleMode.Reject,
      });
      engine.configure().spotFunds(SpotFundsBuilder.NAME, {
        positionLimits: [{ ...reviewing, limit: null }],
      });
      checkOrder("1", true, 0);
      checkOrder("3", false, 0);
      engine.configure().spotFunds(SpotFundsBuilder.NAME, {
        positionLimits: [
          reviewing,
          { accountId: ACCOUNT, asset: "AAPL", mode: "Reject", limit: null },
        ],
      });
      checkOrder("3", true, 1);
    },
  );

  it.each([
    RuleMode.Reject,
    RuleMode.ReviewWithHold,
    RuleMode.ReviewWithoutHold,
  ])("rejects an uncomputable position projection in %s mode", (mode) => {
    for (const [balance, quantity] of [
      ["79228162514264337593543950335", "1"],
      ["100000000000", "0.000000000000000001"],
    ] as const) {
      const engine = spotFundsPositionLimitEngine();
      expect(
        engine.applyAccountAdjustment(ACCOUNT, [
          {
            operation: { asset: "AAPL" },
            amount: { balance: AdjustmentAmount.absolute(balance) },
          },
        ]).ok,
      ).toBe(true);
      engine.configure().spotFunds(SpotFundsBuilder.NAME, {
        positionLimits: [
          { accountId: ACCOUNT, asset: "AAPL", mode, limit: balance },
        ],
      });
      const input = makeOrder(quantity, "0");
      const dry = engine.executePreTradeDryRun(input);
      const live = engine.executePreTrade(input);
      expect(dry.isPass).toBe(false);
      expect(live.ok).toBe(false);
      for (const result of [dry, live]) {
        expect(result.rejects).toHaveLength(1);
        expect(result.rejects[0]).toMatchObject({
          code: "ArithmeticOverflow",
          scope: "order",
          mode: RuleMode.Reject,
        });
      }
      expect(dry.reviews).toEqual([]);
      expect(live.reservation).toBeUndefined();
    }
  });

  it.each(["Reject", "ReviewWithHold", "ReviewWithoutHold"] as const)(
    "rejects a duplicate %s position limit atomically",
    (mode) => {
      const engine = spotFundsPositionLimitEngine();
      engine.configure().spotFunds(SpotFundsBuilder.NAME, {
        positionLimits: [
          { accountId: ACCOUNT, asset: "AAPL", limit: "2" },
          {
            accountId: ACCOUNT,
            asset: "AAPL",
            mode: "ReviewWithHold",
            limit: "0",
          },
        ],
      });
      const patch = {
        positionLimits: [
          { accountId: ACCOUNT, asset: "AAPL", limit: "100" },
          { accountId: ACCOUNT, asset: "AAPL", mode, limit: "100" },
          { accountId: ACCOUNT, asset: "AAPL", mode, limit: null },
        ],
      };
      expect(() =>
        engine.configure().spotFunds(SpotFundsBuilder.NAME, patch),
      ).toThrow(PolicyConfigureError);
      expect(() =>
        engine.configure().spotFunds(SpotFundsBuilder.NAME, patch),
      ).toThrow(/duplicate/);
      const review = engine.executePreTradeDryRun(makeOrder("1"));
      expect(review.isPass).toBe(true);
      expect(review.reviews.map((mark) => mark.mode)).toEqual([
        "ReviewWithHold",
      ]);
      expect(review.reviews[0]?.code).toBe("PositionLimitExceeded");
      const rejecting = engine.executePreTradeDryRun(makeOrder("3"));
      expect(
        rejecting.isPass,
        "duplicate batch preserves the Reject bound",
      ).toBe(false);
      expect(rejecting.rejects[0]?.code).toBe("PositionLimitExceeded");
    },
  );

  it.each([
    ["Unknown", RangeError],
    ["reject", RangeError],
    ["reviewwithhold", RangeError],
    [1, TypeError],
    [{}, TypeError],
  ] as const)(
    "rejects invalid position-limit mode %s without changing settings",
    (mode, error) => {
      const engine = spotFundsPositionLimitEngine();
      engine.configure().spotFunds(SpotFundsBuilder.NAME, {
        positionLimits: [{ accountId: ACCOUNT, asset: "AAPL", limit: "2" }],
      });
      expect(() =>
        engine.configure().spotFunds(SpotFundsBuilder.NAME, {
          positionLimits: [
            {
              accountId: ACCOUNT,
              asset: "AAPL",
              limit: null,
              mode: mode as never,
            },
          ],
        }),
      ).toThrow(error);
      expect(() =>
        engine.configure().spotFunds(SpotFundsBuilder.NAME, {
          positionLimits: [
            {
              accountId: ACCOUNT,
              asset: "AAPL",
              limit: null,
              mode: mode as never,
            },
          ],
        }),
      ).toThrow(/mode must be/);
      const result = engine.executePreTradeDryRun(makeOrder("3"));
      expect(result.isPass, "invalid mode preserves the Reject bound").toBe(
        false,
      );
      expect(result.rejects[0]?.code).toBe("PositionLimitExceeded");
    },
  );

  it("requires an explicit spot-funds position limit to preserve an existing bound", () => {
    const engine = Engine.builder().builtin(buildSpotFunds()).build();
    const adjustment = engine.applyAccountAdjustment(ACCOUNT, [
      {
        operation: { asset: "USD" },
        amount: { balance: AdjustmentAmount.absolute("1000") },
      },
    ]);
    expect(adjustment.ok).toBe(true);

    engine.configure().spotFunds(SpotFundsBuilder.NAME, {
      positionLimits: [{ accountId: ACCOUNT, asset: "AAPL", limit: "1" }],
    });
    for (const entry of [
      { accountId: ACCOUNT, asset: "AAPL" },
      { accountId: ACCOUNT, asset: "AAPL", limit: undefined },
    ]) {
      expect(() =>
        engine.configure().spotFunds(SpotFundsBuilder.NAME, {
          positionLimits: [entry as never],
        }),
      ).toThrow("limit is required");
    }
    expectUnknownOption(
      () =>
        engine.configure().spotFunds(SpotFundsBuilder.NAME, {
          positionLimits: [
            { accountId: ACCOUNT, asset: "AAPL", limt: "3" } as never,
          ],
        }),
      "positionLimits entry",
      "limt",
    );

    const rejected = engine.executePreTrade(makeOrder("2"));
    expect(rejected.ok).toBe(false);
    expect(rejected.rejects[0]?.code).toBe("PositionLimitExceeded");
  });

  it("rejects invalid spot-funds position entries without applying the batch", () => {
    const engine = Engine.builder().builtin(buildSpotFunds()).build();
    const valid = { accountId: ACCOUNT, asset: "AAPL", limit: "1" };
    expect(() =>
      engine.configure().spotFunds(SpotFundsBuilder.NAME, {
        positionLimits: [valid, 7 as never],
      }),
    ).toThrow(TypeError);
    expect(() =>
      engine.configure().spotFunds(SpotFundsBuilder.NAME, {
        positionLimits: [{ accountId: ACCOUNT, asset: "", limit: "1" }],
      }),
    ).toThrow(AssetError);
    expect(() =>
      engine.configure().spotFunds(SpotFundsBuilder.NAME, {
        positionLimits: [{ accountId: ACCOUNT, asset: "AAPL", limit: "bad" }],
      }),
    ).toThrow(ParamError);

    const adjustment = engine.applyAccountAdjustment(ACCOUNT, [
      {
        operation: { asset: "USD" },
        amount: { balance: AdjustmentAmount.absolute("1000") },
      },
    ]);
    expect(adjustment.ok).toBe(true);
    const result = engine.executePreTrade(makeOrder("4"));
    expect(result.ok).toBe(true);
    result.reservation?.rollback();
  });

  it("builds spot-funds pnl barriers from the public builder", () => {
    // Source: https://wiki.openpit.dev/Spot-Funds/ - Configuring Barriers
    const accountId = 99_224_416n;

    // The PnL kill switch is a distinct spot-funds builder entry point; it
    // produces the same SpotFundsPolicy, registered under the same name.
    const engine = Engine.builder()

      .builtin(
        buildSpotFundsPnlBoundsKillswitch()
          .globalBarriers([
            new SpotFundsPnlBoundsBarrier("USD", "-1000", undefined),
          ])
          .accountBarriers([
            new SpotFundsPnlBoundsAccountBarrier(
              accountId,
              new SpotFundsPnlBoundsBarrier("USD", "-250", undefined),
            ),
          ]),
      )
      .build();

    void engine;
    expect(engine).toBeDefined();
  });

  it("ignores a spot-funds pnl barrier with a non-matching currency", () => {
    const engine = Engine.builder()
      .builtin(
        buildSpotFundsPnlBoundsKillswitch().globalBarriers([
          new SpotFundsPnlBoundsBarrier("EUR", "-100", undefined),
        ]),
      )
      .build();
    engine.accounts().setCurrency(ACCOUNT, "USD");

    const result = engine
      .configure()
      .setSpotFundsAccountPnl(SpotFundsPnlBoundsKillswitchBuilder.NAME, {
        account: ACCOUNT,
        state: "-150",
      });

    expect(result.accountBlocks).toHaveLength(0);
  });

  it("rejects a non-matching account-tier spot-funds pnl barrier", () => {
    const engine = Engine.builder()
      .builtin(
        buildSpotFundsPnlBoundsKillswitch().accountBarriers([
          new SpotFundsPnlBoundsAccountBarrier(
            ACCOUNT,
            new SpotFundsPnlBoundsBarrier("EUR", "-100", undefined),
          ),
        ]),
      )
      .build();
    engine.accounts().setCurrency(ACCOUNT, "USD");

    const result = engine
      .configure()
      .setSpotFundsAccountPnl(SpotFundsPnlBoundsKillswitchBuilder.NAME, {
        account: ACCOUNT,
        state: "0",
      });

    expect(result.accountBlocks).toHaveLength(1);
    expect(result.accountBlocks[0]!.code).toBe("PnlKillSwitchTriggered");
    expect(result.accountBlocks[0]!.reason).toBe(
      "pnl barrier currency mismatch",
    );
    expect(result.accountBlocks[0]!.details).toBe(
      "account currency USD, barrier currency EUR",
    );

    const blocked = engine.executePreTrade(makeOrder());
    expect(blocked.ok).toBe(false);
    expect(blocked.rejects).toHaveLength(1);
    expect(blocked.rejects[0]!.code).toBe("PnlKillSwitchTriggered");
    expect(blocked.rejects[0]!.reason).toBe("pnl barrier currency mismatch");
    expect(blocked.rejects[0]!.details).toBe(
      "account currency USD, barrier currency EUR",
    );
  });

  it("applies a spot-funds pnl barrier with a matching currency", () => {
    const engine = Engine.builder()
      .builtin(
        buildSpotFundsPnlBoundsKillswitch().globalBarriers([
          new SpotFundsPnlBoundsBarrier("USD", "-100", undefined),
        ]),
      )
      .build();
    engine.accounts().setCurrency(ACCOUNT, "USD");

    const result = engine
      .configure()
      .setSpotFundsAccountPnl(SpotFundsPnlBoundsKillswitchBuilder.NAME, {
        account: ACCOUNT,
        state: "-150",
      });

    expect(result.accountBlocks).toHaveLength(1);
    expect(result.accountBlocks[0]!.code).toBe("PnlKillSwitchTriggered");
  });

  it("checks an effective P&L barrier when group membership changes", () => {
    const group = 84;
    const engine = Engine.builder()
      .builtin(
        buildSpotFundsPnlBoundsKillswitch().accountGroupBarriers([
          new SpotFundsPnlBoundsAccountGroupBarrier(
            group,
            new SpotFundsPnlBoundsBarrier("USD", "1", undefined),
          ),
        ]),
      )
      .build();

    engine.accounts().registerGroup([ACCOUNT], group);
    const blocked = engine.executePreTrade(makeOrder());
    expect(blocked.ok).toBe(false);
    expect(blocked.rejects[0]!.code).toBe("PnlKillSwitchTriggered");
  });

  it("sets and clears account-currency fallbacks through the public API", () => {
    const accounts = Engine.builder()
      .builtin(buildSpotFunds())
      .build()
      .accounts();

    expect(accounts.setCurrency(ACCOUNT, "USD")).toBeUndefined();
    expect(accounts.clearCurrency(ACCOUNT)).toBeUndefined();
    expect(
      accounts.setGroupCurrency(AccountGroupId.DEFAULT(), "USD"),
    ).toBeUndefined();
    expect(
      accounts.clearGroupCurrency(AccountGroupId.DEFAULT()),
    ).toBeUndefined();

    expect(() => accounts.setCurrency(null as never, "USD")).toThrow(
      AccountIdError,
    );
    expect(() => accounts.setCurrency(ACCOUNT, "")).toThrow(AssetError);
    expect(() => accounts.setGroupCurrency(0, "USD")).toThrow(ParamError);
  });

  it("configures, replaces, and clears P&L axes on ordinary spot funds", () => {
    const survivor = ACCOUNT;
    const accountOverride = ACCOUNT + 1n;
    const cleared = ACCOUNT + 2n;
    const group = 7;
    const engine = Engine.builder().builtin(buildSpotFunds()).build();
    const accounts = engine.accounts();
    accounts.setGroupCurrency(AccountGroupId.DEFAULT(), "USD");
    accounts.registerGroup([accountOverride], group);
    accounts.setGroupCurrency(group, "USD");
    accounts.setCurrency(accountOverride, "USD");
    expect(accounts.groupOf(accountOverride)!.value).toBe(group);

    engine.applyAccountAdjustment(cleared, [
      {
        operation: { asset: "USD" },
        amount: { balance: AdjustmentAmount.absolute("1000") },
      },
    ]);

    // A plain Spot Funds policy has no P&L control, but funded orders still
    // run.
    const ordinary = engine.executePreTrade({
      operation: {
        underlyingAsset: "AAPL",
        settlementAsset: "USD",
        accountId: cleared,
        side: "BUY",
        tradeAmount: TradeAmount.quantity("1"),
        price: "100",
      },
    });
    expect(ordinary.ok).toBe(true);
    ordinary.reservation!.rollback();

    const fillWithFee = (accountId: bigint, fee: string) => {
      const lock = new Lock(undefined);
      lock.push(0, Price.fromString("100"));
      return engine.applyExecutionReport({
        operation: {
          underlyingAsset: "AAPL",
          settlementAsset: "USD",
          accountId,
          side: "BUY",
        },
        fill: {
          lastTrade: { price: "100", quantity: "1" },
          remainingReservedQuantity: "0",
          lock,
          isFinal: true,
          fee: { amount: fee, currency: "USD" },
        },
      });
    };

    engine.configure().spotFundsPnlBoundsKillswitch(SpotFundsBuilder.NAME, {
      globalBarriers: [new SpotFundsPnlBoundsBarrier("USD", "-10", undefined)],
    });

    const firstSurvivorFill = fillWithFee(survivor, "9");
    expect(firstSurvivorFill.accountBlocks).toHaveLength(0);

    // Omitted globalBarriers retain both their thresholds and live P&L.
    engine.configure().spotFundsPnlBoundsKillswitch(SpotFundsBuilder.NAME, {
      accountGroupBarriers: [
        new SpotFundsPnlBoundsAccountGroupBarrier(
          group,
          new SpotFundsPnlBoundsBarrier("USD", "-14", undefined),
        ),
      ],
      accountBarriers: [
        new SpotFundsPnlBoundsAccountBarrier(
          accountOverride,
          new SpotFundsPnlBoundsBarrier("USD", "-20", undefined),
        ),
      ],
    });

    const survivorBreach = fillWithFee(survivor, "2");
    expect(survivorBreach.accountBlocks[0]!.code).toBe(
      "PnlKillSwitchTriggered",
    );

    // The account axis wins over the registered account group axis.
    const overrideFill = fillWithFee(accountOverride, "15");
    expect(overrideFill.accountBlocks).toHaveLength(0);

    // Clearing the account axis reveals the group tier: the live -15 sits
    // inside the cleared account bound but past the group's -14, so the
    // retune itself blocks the account.
    const groupTier = engine
      .configure()
      .spotFundsPnlBoundsKillswitch(SpotFundsBuilder.NAME, {
        accountBarriers: [],
      });
    expect(groupTier.accountBlocks[0]!.block.code).toBe(
      "PnlKillSwitchTriggered",
    );
    engine.accounts().unblock(accountOverride);

    const groupRecheck = fillWithFee(accountOverride, "0.1");
    expect(groupRecheck.accountBlocks[0]!.code).toBe("PnlKillSwitchTriggered");
    engine.accounts().unblock(accountOverride);

    // Clearing the group axis reveals the unchanged global threshold.
    const globalTier = engine
      .configure()
      .spotFundsPnlBoundsKillswitch(SpotFundsBuilder.NAME, {
        accountGroupBarriers: [],
      });
    expect(globalTier.accountBlocks[0]!.block.code).toBe(
      "PnlKillSwitchTriggered",
    );
    engine.accounts().unblock(accountOverride);

    const globalRecheck = fillWithFee(accountOverride, "0.1");
    expect(globalRecheck.accountBlocks[0]!.code).toBe("PnlKillSwitchTriggered");

    engine.configure().spotFundsPnlBoundsKillswitch(SpotFundsBuilder.NAME, {
      globalBarriers: [],
      accountGroupBarriers: [],
      accountBarriers: [],
    });

    const clearedFill = fillWithFee(cleared, "15");
    expect(clearedFill.accountBlocks).toHaveLength(0);

    const afterClear = engine.executePreTrade({
      operation: {
        underlyingAsset: "AAPL",
        settlementAsset: "USD",
        accountId: cleared,
        side: "BUY",
        tradeAmount: TradeAmount.quantity("1"),
        price: "100",
      },
    });
    expect(afterClear.ok).toBe(true);
    afterClear.reservation!.rollback();
  });

  it("rejects the singular spot-funds P&L barrier before applying any key", () => {
    const engine = Engine.builder().builtin(buildSpotFunds()).build();
    const configure = engine.configure();
    const name = SpotFundsBuilder.NAME;
    const globalBarriers = [
      new SpotFundsPnlBoundsBarrier("USD", "-10", undefined),
    ];
    const pnl = { account: ACCOUNT, state: "-20" };
    const seeded = configure.setSpotFundsAccountPnl(name, pnl);
    expect(seeded.accountBlocks).toHaveLength(0);

    const removedOption = { globalBarrier: globalBarriers };
    expectUnknownOption(
      () =>
        configure.spotFundsPnlBoundsKillswitch(name, removedOption as never),
      "spotFundsPnlBoundsKillswitch options",
      "globalBarrier",
    );
    const mixedOptions = { ...removedOption, globalBarriers };
    expectUnknownOption(
      () => configure.spotFundsPnlBoundsKillswitch(name, mixedOptions),
      "spotFundsPnlBoundsKillswitch options",
      "globalBarrier",
    );
    expect(engine.startPreTrade(makeOrder()).ok).toBe(true);

    const patched = configure.spotFundsPnlBoundsKillswitch(name, {
      globalBarriers,
    });
    expect(patched.accountBlocks).toHaveLength(1);
    expect(patched.accountBlocks[0]?.accountId.value).toBe(ACCOUNT);
  });

  it("rejects a misspelled spot-funds P&L accumulator option", () => {
    const engine = Engine.builder()
      .builtin(
        buildSpotFundsPnlBoundsKillswitch().globalBarriers([
          new SpotFundsPnlBoundsBarrier("USD", "-10", undefined),
        ]),
      )
      .build();
    const configure = engine.configure();
    const name = SpotFundsPnlBoundsKillswitchBuilder.NAME;
    const pnl = { account: ACCOUNT, state: "-20" };
    const misspelled = { ...pnl, pnl: "-20" };
    expectUnknownOption(
      () => configure.setSpotFundsAccountPnl(name, misspelled),
      "setSpotFundsAccountPnl options",
      "pnl",
    );
    expect(engine.startPreTrade(makeOrder()).ok).toBe(true);
    const result = configure.setSpotFundsAccountPnl(name, pnl);
    expect(result.accountBlocks).toHaveLength(1);
    expect(result.accountBlocks[0]?.reason).toBe("pnl kill switch triggered");
    expect(engine.startPreTrade(makeOrder()).rejects[0]?.code).toBe(
      "PnlKillSwitchTriggered",
    );
  });

  it("retunes spot-funds pnl barriers and force-sets the live accumulator", () => {
    // Source: https://wiki.openpit.dev/Spot-Funds/ - Runtime Reconfiguration
    const retunedAccount = 99_224_416n;
    const forcedAccount = 99_224_417n;
    const engine = Engine.builder()
      .builtin(
        buildSpotFundsPnlBoundsKillswitch().globalBarriers([
          new SpotFundsPnlBoundsBarrier("USD", "-1000", undefined),
        ]),
      )
      .build();

    // Seed live PnL inside the current -1000 barrier.
    const seed = engine
      .configure()
      .setSpotFundsAccountPnl(SpotFundsPnlBoundsKillswitchBuilder.NAME, {
        account: retunedAccount,
        state: "-600",
      });
    expect(seed.accountBlocks).toHaveLength(0);

    // Tightening the barrier checks the known account and records the block
    // now.
    const retune = engine
      .configure()
      .spotFundsPnlBoundsKillswitch(SpotFundsPnlBoundsKillswitchBuilder.NAME, {
        globalBarriers: [
          new SpotFundsPnlBoundsBarrier("USD", "-500", undefined),
        ],
      });
    expect(retune.accountBlocks).toHaveLength(1);
    expect(retune.accountBlocks[0]!.accountId.value).toBe(retunedAccount);

    // A force-set beyond the current barrier also returns its recorded block.
    const forced = engine
      .configure()
      .setSpotFundsAccountPnl(SpotFundsPnlBoundsKillswitchBuilder.NAME, {
        account: forcedAccount,
        state: "-600",
      });
    expect(forced.accountBlocks).toHaveLength(1);
  });

  it("pairs barrier-sweep blocks with engine-selected accounts", () => {
    const blockedNumeric = ACCOUNT;
    const safe = ACCOUNT + 1n;
    const blockedHalted = ACCOUNT + 2n;
    const engine = Engine.builder().builtin(buildSpotFunds()).build();

    for (const [account, state] of [
      [blockedNumeric, "-20"],
      [safe, "-5"],
      [blockedHalted, PnlHaltReason.fromMissingFx()],
    ] as const) {
      const seeded = engine
        .configure()
        .setSpotFundsAccountPnl(SpotFundsBuilder.NAME, { account, state });
      expect(seeded.accountBlocks).toHaveLength(0);
    }

    const swept = engine
      .configure()
      .spotFundsPnlBoundsKillswitch(SpotFundsBuilder.NAME, {
        globalBarriers: [
          new SpotFundsPnlBoundsBarrier("USD", "-10", undefined),
        ],
      });

    expect(swept.accountBlocks).toHaveLength(2);
    expect(swept.accountBlocks[0]!.accountId.value).toBe(blockedNumeric);
    expect(swept.accountBlocks[0]!.block.reason).toBe(
      "pnl kill switch triggered",
    );
    expect(swept.accountBlocks[1]!.accountId.value).toBe(blockedHalted);
    expect(swept.accountBlocks[1]!.block.reason).toBe(
      "account pnl calculation halted",
    );

    const orderFor = (accountId: bigint): OrderInit => ({
      operation: {
        ...makeOrder().operation,
        accountId,
      },
    });
    expect(engine.startPreTrade(orderFor(blockedNumeric)).ok).toBe(false);
    expect(engine.startPreTrade(orderFor(blockedHalted)).ok).toBe(false);
    expect(engine.startPreTrade(orderFor(safe)).ok).toBe(true);
  });

  it("keeps an account P&L halt sticky until that accumulator is force-set", () => {
    const engine = Engine.builder().builtin(buildSpotFunds()).build();

    const first = applySpotFundsFill(engine, ACCOUNT, "BUY", "1");
    expect(first.accountPnls).toHaveLength(1);
    expect(first.accountPnls[0]?.haltReason?.isMissingAccountCurrency).toBe(
      true,
    );

    expect(applySpotFundsFill(engine).accountPnls).toHaveLength(0);

    engine.accounts().setCurrency(ACCOUNT, "USD");
    forceSpotFundsBalancePnl(engine);
    const accountStillHalted = applySpotFundsFill(engine);
    expect(accountStillHalted.accountPnls).toHaveLength(0);
    const aaplOutcome = accountStillHalted.accountAdjustments.find(
      (outcome) => outcome.entry.asset === "AAPL",
    );
    expect(aaplOutcome?.entry.realizedPnl).toBeUndefined();
    expect(aaplOutcome?.entry.averageEntryPrice).toBeDefined();

    engine.configure().setSpotFundsAccountPnl(SpotFundsBuilder.NAME, {
      account: ACCOUNT,
      state: PnlHaltReason.fromMissingFx(),
    });
    engine.configure().setSpotFundsAccountPnl(SpotFundsBuilder.NAME, {
      account: ACCOUNT,
      state: "10",
    });
    const rearmed = applySpotFundsFill(engine, ACCOUNT, "BUY", "1");
    expect(rearmed.accountPnls).toHaveLength(1);
    expect(rearmed.accountPnls[0]?.ok).toBe(true);
  });

  it("re-arms position and account P&L independently", () => {
    const engine = Engine.builder().builtin(buildSpotFunds()).build();
    engine.accounts().setCurrency(ACCOUNT, "EUR");

    const positionPnl = (result: ReturnType<typeof applySpotFundsFill>) =>
      result.accountAdjustments.find(
        (outcome) => outcome.entry.asset === "AAPL",
      )?.entry.realizedPnl;

    const opening = applySpotFundsFill(engine);
    expect(positionPnl(opening)?.haltReason?.isMissingFx).toBe(true);
    expect(opening.accountPnls).toHaveLength(0);

    const first = applySpotFundsFill(engine, ACCOUNT, "SELL");
    expect(positionPnl(first)).toBeUndefined();
    expect(first.accountPnls).toHaveLength(1);
    expect(first.accountPnls[0]?.haltReason?.isMissingFx).toBe(true);
    expect(first.accountBlocks).toHaveLength(0);

    expect(positionPnl(applySpotFundsFill(engine))).toBeUndefined();

    engine.configure().setSpotFundsAccountPnl(SpotFundsBuilder.NAME, {
      account: ACCOUNT,
      state: "10",
    });
    const accountRearmed = applySpotFundsFill(engine, ACCOUNT, "SELL");
    expect(positionPnl(accountRearmed)).toBeUndefined();
    expect(accountRearmed.accountPnls).toHaveLength(1);
    expect(accountRearmed.accountPnls[0]?.haltReason?.isMissingFx).toBe(true);

    forceSpotFundsBalancePnl(
      engine,
      ACCOUNT,
      PnlHaltReason.fromMissingCostBasis(),
    );
    forceSpotFundsBalancePnl(engine);

    const positionRearmed = applySpotFundsFill(engine);
    expect(positionPnl(positionRearmed)?.haltReason?.isMissingFx).toBe(true);
    const positionStickyAgain = applySpotFundsFill(engine, ACCOUNT, "SELL");
    expect(positionPnl(positionStickyAgain)).toBeUndefined();
    expect(positionStickyAgain.accountPnls).toHaveLength(0);
    expect(positionStickyAgain.accountBlocks).toHaveLength(0);
  });
});
