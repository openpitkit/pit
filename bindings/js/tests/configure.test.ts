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
  OrderSizeAccountAssetBarrier,
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
              "AAPL",
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

    // The account+asset reject above carries account scope, and a start-stage
    // reject with account scope latches a block on that account, so the
    // override account is no longer admissible on this engine. The zero-cap
    // probes therefore need a fresh engine, retuned to zero the same way.
    const zeroEngine = Engine.builder()
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
    zeroEngine.configure().orderSizeLimit(OrderSizeLimitBuilder.NAME, {
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
    const zeroAsset = zeroEngine.startPreTrade(makeOrder("1", "1"));
    expect(zeroAsset.ok).toBe(false);
    expect(zeroAsset.rejects[0]?.code).toBe("OrderNotionalExceedsLimit");
    const zeroAccountAsset = zeroEngine.startPreTrade(
      makeOrder("1", "1", ORDER_SIZE_OVERRIDE_ACCOUNT),
    );
    expect(zeroAccountAsset.ok).toBe(false);
    expect(zeroAccountAsset.rejects[0]?.code).toBe("OrderNotionalExceedsLimit");
  });

  // These two look like tautologies - the order is under every visible cap - and
  // they are not. Quantity resolves by the instrument's underlying asset (AAPL)
  // and notional by its settlement asset (USD), so the quantity-only USD barrier
  // and the notional-only AAPL barrier each leave absent exactly the cap its own
  // key would make the engine consult. An absent cap turned into a zero cap
  // would land on that live chain and reject. Keep the asymmetry: one barrier
  // carrying both caps, or both keyed on one asset, silently removes the check.
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
    // Source: https://wiki.openpit.dev/Spot-Funds/ - Configuring Barriers
    const accountId = 99_224_416n;

    // The PnL kill switch is a distinct spot-funds builder entry point; it
    // produces the same SpotFundsPolicy, registered under the same name.
    const engine = Engine.builder()

      .builtin(
        buildSpotFundsPnlBoundsKillswitch()
          .globalBarrier(
            new SpotFundsPnlBoundsBarrier("USD", "-1000", undefined),
          )
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
        buildSpotFundsPnlBoundsKillswitch().globalBarrier(
          new SpotFundsPnlBoundsBarrier("EUR", "-100", undefined),
        ),
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
        buildSpotFundsPnlBoundsKillswitch().globalBarrier(
          new SpotFundsPnlBoundsBarrier("USD", "-100", undefined),
        ),
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
          remainingReservedQuantity: "0",
          lock,
          isFinal: true,
          fee: { amount: fee, currency: "USD" },
        },
      });
    };

    engine.configure().spotFundsPnlBoundsKillswitch(SpotFundsBuilder.NAME, {
      globalBarrier: new SpotFundsPnlBoundsBarrier("USD", "-10", undefined),
    });

    const firstSurvivorFill = fillWithFee(survivor, "9");
    expect(firstSurvivorFill.accountBlocks).toHaveLength(0);

    // An omitted globalBarrier retains both its threshold and live P&L.
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
    // Source: https://wiki.openpit.dev/Spot-Funds/ - Runtime Reconfiguration
    const retunedAccount = 99_224_416n;
    const forcedAccount = 99_224_417n;
    const engine = Engine.builder()
      .builtin(
        buildSpotFundsPnlBoundsKillswitch().globalBarrier(
          new SpotFundsPnlBoundsBarrier("USD", "-1000", undefined),
        ),
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

    // Tightening the barrier checks the known account and records the block now.
    const retune = engine
      .configure()
      .spotFundsPnlBoundsKillswitch(SpotFundsPnlBoundsKillswitchBuilder.NAME, {
        globalBarrier: new SpotFundsPnlBoundsBarrier("USD", "-500", undefined),
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
        globalBarrier: new SpotFundsPnlBoundsBarrier("USD", "-10", undefined),
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
