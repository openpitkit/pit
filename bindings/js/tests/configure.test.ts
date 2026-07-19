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
// - ../../../../pit.wiki/Dynamic-Policy-Reconfiguration.md
// - ../../../../pit.wiki/Spot-Funds.md

import { describe, expect, it } from "vitest";

import {
  AccountIdError,
  AssetError,
  ConfigureErrorKind,
  Engine,
  ParamError,
  PolicyConfigureError,
} from "@openpit/engine";
import {
  AccountGroupId,
  AdjustmentAmount,
  Price,
  TradeAmount,
} from "@openpit/engine/param";
import { type OrderInit } from "@openpit/engine/model";
import { Lock, PnlHaltReason } from "@openpit/engine/pretrade";
import {
  buildOrderSizeLimit,
  buildPnlBoundsKillswitch,
  buildRateLimit,
  buildSpotFunds,
  buildSpotFundsPnlBoundsKillswitch,
  OrderSizeAssetBarrier,
  OrderSizeBrokerBarrier,
  OrderSizeLimit,
  OrderSizeLimitBuilder,
  PnlBoundsBrokerBarrier,
  PnlBoundsKillswitchBuilder,
  RateLimit,
  RateLimitAssetBarrier,
  RateLimitBrokerBarrier,
  RateLimitBuilder,
  SpotFundsBuilder,
  SpotFundsLimitMode,
  SpotFundsPnlBoundsAccountBarrier,
  SpotFundsPnlBoundsAccountGroupBarrier,
  SpotFundsPnlBoundsBarrier,
  SpotFundsPnlBoundsKillswitchBuilder,
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

// `fee` is denominated in the account currency, so passing one engages the
// account line's need for that currency. A fee-less fill contributes a
// computable zero to it instead.
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
      leavesQuantity: "0",
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

describe("runtime configurator", () => {
  it("retunes a built-in rate-limit policy", () => {
    // Source: Dynamic-Policy-Reconfiguration.md - Retune a Built-in Policy
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
        buildRateLimit().brokerBarrier(
          new RateLimitBrokerBarrier(new RateLimit(5, 60_000)),
        ),
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
      broker: new RateLimitBrokerBarrier(new RateLimit(2, 60_000)),
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
        buildRateLimit().brokerBarrier(
          new RateLimitBrokerBarrier(new RateLimit(1, 60_000)),
        ),
      )
      .build();

    try {
      engine.configure().rateLimit("NoSuchPolicy", {
        broker: new RateLimitBrokerBarrier(new RateLimit(2, 60_000)),
      });
      throw new Error("expected unknown policy error");
    } catch (error) {
      expectConfigureError(error, ConfigureErrorKind.Unknown);
      expect((error as PolicyConfigureError).policyName).toBe("NoSuchPolicy");
    }

    try {
      engine.configure().rateLimit(RateLimitBuilder.NAME, {
        broker: new RateLimitBrokerBarrier(new RateLimit(1, 0)),
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

  it("rejects conflicting broker set and clear inputs in the JS binding", () => {
    const rateEngine = Engine.builder()
      .builtin(
        buildRateLimit().brokerBarrier(
          new RateLimitBrokerBarrier(new RateLimit(1, 60_000)),
        ),
      )
      .build();

    expect(() =>
      rateEngine.configure().rateLimit(RateLimitBuilder.NAME, {
        broker: new RateLimitBrokerBarrier(new RateLimit(2, 60_000)),
        clearBroker: true,
      }),
    ).toThrow(ParamError);
    try {
      rateEngine.configure().rateLimit(RateLimitBuilder.NAME, {
        broker: new RateLimitBrokerBarrier(new RateLimit(2, 60_000)),
        clearBroker: true,
      });
    } catch (error) {
      expect(error).toBeInstanceOf(ParamError);
      expect((error as Error).message).toBe(
        "broker and clearBroker cannot be used together",
      );
    }

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
          .brokerBarrier(new RateLimitBrokerBarrier(new RateLimit(1, 60_000)))
          .assetBarriers([
            new RateLimitAssetBarrier(new RateLimit(10, 60_000), "USD"),
          ]),
      )
      .build();

    engine.configure().rateLimit(RateLimitBuilder.NAME, { clearBroker: true });

    for (let index = 0; index < 2; index += 1) {
      const result = engine.executePreTrade(makeOrder());
      expect(result.ok).toBe(true);
      result.reservation!.commit();
    }
  });

  it("leaves a rate-limit broker barrier unchanged when clearBroker is false", () => {
    const engine = Engine.builder()
      .builtin(
        buildRateLimit().brokerBarrier(
          new RateLimitBrokerBarrier(new RateLimit(1, 60_000)),
        ),
      )
      .build();

    engine.configure().rateLimit(RateLimitBuilder.NAME, { clearBroker: false });

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
              "USD",
            ),
          ]),
      )
      .build();

    const oversized = engine.startPreTrade(makeOrder("2"));
    expect(oversized.ok).toBe(false);
    expect(oversized.rejects[0]!.code).toBe("OrderQtyExceedsLimit");

    engine.configure().orderSizeLimit(OrderSizeLimitBuilder.NAME, {
      clearBroker: true,
    });

    expect(engine.startPreTrade(makeOrder("2")).ok).toBe(true);
  });

  it("force-sets accumulated generic pnl", () => {
    // Source: Dynamic-Policy-Reconfiguration.md - Force-set Accumulated P&L
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
    // Built-in policies register under their type name (PnlBoundsKillswitchBuilder.NAME).
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

  it("switches spot funds global limit mode at runtime", () => {
    // Source: Dynamic-Policy-Reconfiguration.md - Spot Funds: Global Limit Mode
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

    // Switch to TrackOnly: the same order now passes and reserves against deficit.
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
    // Source: Dynamic-Policy-Reconfiguration.md - Spot Funds: Per-Account Limit Mode
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

    // Pin this account to TrackOnly: per-account override wins over global Enforce.
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

  it("builds spot-funds pnl barriers from the public builder", () => {
    // Source: Spot-Funds.md - Configuring Barriers
    const accountId = 99_224_416n;

    // The PnL kill switch is a distinct spot-funds builder entry point; it
    // produces the same SpotFundsPolicy, registered under the same name.
    const engine = Engine.builder()

      .builtin(
        buildSpotFundsPnlBoundsKillswitch()
          .globalBarrier(new SpotFundsPnlBoundsBarrier("-1000", undefined))
          .accountBarriers([
            new SpotFundsPnlBoundsAccountBarrier(
              accountId,
              new SpotFundsPnlBoundsBarrier("-250", undefined),
            ),
          ]),
      )
      .build();

    void engine;
    expect(engine).toBeDefined();
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

    // A plain Spot Funds policy has no P&L control, but funded orders still run.
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
          leavesQuantity: "0",
          lock,
          isFinal: true,
          fee: { amount: fee, currency: "USD" },
        },
      });
    };

    engine.configure().spotFundsPnlBoundsKillswitch(SpotFundsBuilder.NAME, {
      globalBarrier: new SpotFundsPnlBoundsBarrier("-10", undefined),
    });

    const firstSurvivorFill = fillWithFee(survivor, "9");
    expect(firstSurvivorFill.accountBlocks).toHaveLength(0);

    // An omitted globalBarrier retains both its threshold and live P&L.
    engine.configure().spotFundsPnlBoundsKillswitch(SpotFundsBuilder.NAME, {
      accountGroupBarriers: [
        new SpotFundsPnlBoundsAccountGroupBarrier(
          group,
          new SpotFundsPnlBoundsBarrier("-14", undefined),
        ),
      ],
      accountBarriers: [
        new SpotFundsPnlBoundsAccountBarrier(
          accountOverride,
          new SpotFundsPnlBoundsBarrier("-20", undefined),
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

    // Clearing the direct currency and the account axis reveals the group tier.
    accounts.clearCurrency(accountOverride);
    engine.configure().spotFundsPnlBoundsKillswitch(SpotFundsBuilder.NAME, {
      accountBarriers: [],
    });
    const groupRecheck = fillWithFee(accountOverride, "0");
    expect(groupRecheck.accountBlocks[0]!.code).toBe("PnlKillSwitchTriggered");

    // Clearing the group axis reveals the unchanged global threshold.
    engine.configure().spotFundsPnlBoundsKillswitch(SpotFundsBuilder.NAME, {
      accountGroupBarriers: [],
    });
    const globalRecheck = fillWithFee(accountOverride, "0");
    expect(globalRecheck.accountBlocks[0]!.code).toBe("PnlKillSwitchTriggered");

    engine.configure().spotFundsPnlBoundsKillswitch(SpotFundsBuilder.NAME, {
      globalBarrier: null,
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

  it("retunes spot-funds pnl barriers and force-sets the live accumulator", () => {
    // Source: Spot-Funds.md - Runtime Reconfiguration
    const accountId = 99_224_416n;
    const engine = Engine.builder()

      .builtin(
        buildSpotFundsPnlBoundsKillswitch().globalBarrier(
          new SpotFundsPnlBoundsBarrier("-1000", undefined),
        ),
      )
      .build();

    // Retune the account PnL barriers; live accumulated PnL is untouched.
    engine
      .configure()
      .spotFundsPnlBoundsKillswitch(SpotFundsPnlBoundsKillswitchBuilder.NAME, {
        globalBarrier: new SpotFundsPnlBoundsBarrier("-500", undefined),
        accountBarriers: [
          new SpotFundsPnlBoundsAccountBarrier(
            accountId,
            new SpotFundsPnlBoundsBarrier("-250", "250"),
          ),
        ],
      });

    // Force-set the live accumulated PnL for one account.
    const result = engine
      .configure()
      .setSpotFundsAccountPnl(SpotFundsPnlBoundsKillswitchBuilder.NAME, {
        account: accountId,
        state: "-600",
      });
    expect(result.accountBlocks).toHaveLength(1);
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
    const rearmed = applySpotFundsFill(engine);
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
    expect(positionPnl(opening)).toBeUndefined();
    expect(opening.accountPnls).toHaveLength(1);
    expect(opening.accountPnls[0]?.ok).toBe(true);
    expect(opening.accountPnls[0]?.pnl?.delta.toString()).toBe("0");

    const first = applySpotFundsFill(engine, ACCOUNT, "SELL");
    expect(positionPnl(first)?.haltReason?.isMissingFx).toBe(true);
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

    expect(positionPnl(applySpotFundsFill(engine))).toBeUndefined();
    const rearmed = applySpotFundsFill(engine, ACCOUNT, "SELL");
    expect(positionPnl(rearmed)?.haltReason?.isMissingFx).toBe(true);
    expect(rearmed.accountPnls).toHaveLength(0);
    expect(rearmed.accountBlocks).toHaveLength(0);
  });
});
