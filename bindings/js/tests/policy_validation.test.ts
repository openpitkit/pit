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

import {
  AssetError,
  Engine,
  EngineBuildError,
  ParamError,
} from "@openpit/engine";
import { QuoteTtl } from "@openpit/engine/marketdata";
import { TradeAmount } from "@openpit/engine/param";
import type { OrderInit } from "@openpit/engine/model";
import type { Policy } from "@openpit/engine/pretrade";
import {
  buildOrderSizeLimit,
  buildRateLimit,
  buildSpotFunds,
  buildSpotFundsPnlBoundsKillswitch,
  OrderSizeAccountAssetBarrier,
  OrderSizeAssetBarrier,
  OrderSizeBrokerBarrier,
  OrderSizeLimit,
  RateLimit,
  RateLimitBrokerBarrier,
  SpotFundsOverride,
  SpotFundsPnlBoundsBarrier,
} from "@openpit/engine/pretrade/policies";

function buildBrokerRateLimit(windowMs: number): Engine {
  return Engine.builder()
    .builtin(
      buildRateLimit().brokerBarrier(
        new RateLimitBrokerBarrier(new RateLimit(1, windowMs)),
      ),
    )
    .build();
}

const ORDER_SIZE_ACCOUNT = 99_224_416n;
const ORDER_SIZE_OVERRIDE_ACCOUNT = 99_224_417n;

function sizedOrder(
  quantity: string,
  price: string,
  accountId: bigint = ORDER_SIZE_ACCOUNT,
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

describe("rate-limit boundary conversion", () => {
  it("preserves a fractional sub-millisecond window", () => {
    const limit = new RateLimit(2, 0.0005);

    expect(limit.windowMs).toBeCloseTo(0.0005, 10);
    expect(() =>
      Engine.builder()
        .builtin(
          buildRateLimit().brokerBarrier(new RateLimitBrokerBarrier(limit)),
        )
        .build(),
    ).not.toThrow();
  });

  it("keeps ordinary whole-millisecond behavior unchanged", () => {
    const limit = new RateLimit(100, 1000);

    expect(limit.maxOrders).toBe(100);
    expect(limit.windowMs).toBe(1000);
    expect(() => buildBrokerRateLimit(1000)).not.toThrow();
  });

  it("surfaces the core error for a zero window", () => {
    let caught: unknown;
    try {
      buildBrokerRateLimit(0);
    } catch (error) {
      caught = error;
    }

    expect(caught).toBeInstanceOf(EngineBuildError);
    expect((caught as Error).message).toMatch(
      /rate limit window must be positive and fit in u64 nanoseconds/,
    );
  });

  it.each([-1, Number.NaN, Number.POSITIVE_INFINITY])(
    "rejects an unrepresentable window at the boundary: %s",
    (windowMs) => {
      expect(() => new RateLimit(1, windowMs)).toThrow(RangeError);
      expect(() => new RateLimit(1, windowMs)).toThrow(
        /windowMs must be finite, non-negative, and representable as a duration/,
      );
    },
  );

  it("propagates a plain-object getter exception unchanged", () => {
    const marker = new Error("maxOrders getter failed");
    const limit = Object.defineProperty({ windowMs: 1000 }, "maxOrders", {
      get() {
        throw marker;
      },
    }) as { maxOrders: number; windowMs: number };

    let caught: unknown;
    try {
      new RateLimitBrokerBarrier(limit);
    } catch (error) {
      caught = error;
    }

    expect(caught).toBe(marker);
  });

  it("propagates a wrapper clone exception unchanged", () => {
    const marker = new Error("RateLimit.clone failed");
    const limit = new RateLimit(1, 1000);
    Object.defineProperty(limit, "clone", {
      value() {
        throw marker;
      },
    });

    let caught: unknown;
    try {
      new RateLimitBrokerBarrier(limit);
    } catch (error) {
      caught = error;
    }

    expect(caught).toBe(marker);
  });

  it("accepts the largest whole millisecond below the core bound", () => {
    expect(() => buildBrokerRateLimit(18_446_744_073_709)).not.toThrow();
  });

  it("surfaces the core error above the maximum window", () => {
    let caught: unknown;
    try {
      buildBrokerRateLimit(18_446_744_073_710);
    } catch (error) {
      caught = error;
    }

    expect(caught).toBeInstanceOf(EngineBuildError);
    expect((caught as Error).message).toMatch(
      /rate limit window must be positive and fit in u64 nanoseconds/,
    );
  });
});

describe("order-size limits", () => {
  function brokerEngine(limit: OrderSizeLimit): Engine {
    return Engine.builder()
      .builtin(
        buildOrderSizeLimit().brokerBarrier(new OrderSizeBrokerBarrier(limit)),
      )
      .build();
  }

  function settlementAssetEngine(
    assetLimit: OrderSizeLimit,
    accountAssetLimit: OrderSizeLimit,
  ): Engine {
    return Engine.builder()
      .builtin(
        buildOrderSizeLimit()
          .assetBarriers([new OrderSizeAssetBarrier(assetLimit, "USD")])
          .accountAssetBarriers([
            new OrderSizeAccountAssetBarrier(
              accountAssetLimit,
              ORDER_SIZE_OVERRIDE_ACCOUNT,
              "USD",
            ),
          ]),
      )
      .build();
  }

  it("enforces a quantity-only boundary without inventing a notional cap", () => {
    const limit = new OrderSizeLimit("10", undefined);
    expect(limit.maxQuantity!.toString()).toBe("10");
    expect(limit.maxNotional).toBeUndefined();
    const engine = brokerEngine(limit);

    const above = engine.executePreTrade(sizedOrder("11", "1"));
    expect(above.ok).toBe(false);
    expect(above.rejects[0]?.code).toBe("OrderQtyExceedsLimit");

    const boundary = engine.executePreTrade(sizedOrder("10", "1"));
    expect(boundary.ok).toBe(true);
    boundary.reservation!.rollback();
  });

  it("enforces a notional-only boundary without inventing a quantity cap", () => {
    const limit = new OrderSizeLimit(undefined, "1000");
    expect(limit.maxQuantity).toBeUndefined();
    expect(limit.maxNotional!.toString()).toBe("1000");
    const engine = brokerEngine(limit);

    const above = engine.executePreTrade(sizedOrder("1", "1001"));
    expect(above.ok).toBe(false);
    expect(above.rejects[0]?.code).toBe("OrderNotionalExceedsLimit");

    const boundary = engine.executePreTrade(sizedOrder("1", "1000"));
    expect(boundary.ok).toBe(true);
    boundary.reservation!.rollback();
  });

  it("enforces notional-only settlement-asset boundaries without quantity caps", () => {
    const engine = settlementAssetEngine(
      new OrderSizeLimit(undefined, "1000"),
      new OrderSizeLimit(undefined, "2000"),
    );

    const assetBoundary = engine.executePreTrade(sizedOrder("1000", "1"));
    expect(assetBoundary.ok).toBe(true);
    assetBoundary.reservation!.rollback();
    const aboveAsset = engine.executePreTrade(sizedOrder("1001", "1"));
    expect(aboveAsset.ok).toBe(false);
    expect(aboveAsset.rejects[0]?.code).toBe("OrderNotionalExceedsLimit");

    const accountBoundary = engine.executePreTrade(
      sizedOrder("2000", "1", ORDER_SIZE_OVERRIDE_ACCOUNT),
    );
    expect(accountBoundary.ok).toBe(true);
    accountBoundary.reservation!.rollback();
    const aboveAccount = engine.executePreTrade(
      sizedOrder("2001", "1", ORDER_SIZE_OVERRIDE_ACCOUNT),
    );
    expect(aboveAccount.ok).toBe(false);
    expect(aboveAccount.rejects[0]?.code).toBe("OrderNotionalExceedsLimit");
  });

  // These two look like tautologies - the order is under every visible cap - and
  // they are not. Quantity resolves by the instrument's underlying asset (AAPL)
  // and notional by its settlement asset (USD), so the quantity-only USD barrier
  // and the notional-only AAPL barrier each leave absent exactly the cap its own
  // key would make the engine consult. An absent cap turned into a zero cap
  // would land on that live chain and reject. Keep the asymmetry: one barrier
  // carrying both caps, or both keyed on one asset, silently removes the check.
  it("preserves absent caps on the chains fed by asset keys", () => {
    const engine = Engine.builder()
      .builtin(
        buildOrderSizeLimit().assetBarriers([
          new OrderSizeAssetBarrier(new OrderSizeLimit("10", undefined), "USD"),
          new OrderSizeAssetBarrier(
            new OrderSizeLimit(undefined, "1000"),
            "AAPL",
          ),
        ]),
      )
      .build();

    const result = engine.executePreTrade(sizedOrder("5", "100"));
    expect(result.ok).toBe(true);
    result.reservation!.rollback();
  });

  it("preserves absent caps on the chains fed by account-asset keys", () => {
    const engine = Engine.builder()
      .builtin(
        buildOrderSizeLimit().accountAssetBarriers([
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
        ]),
      )
      .build();

    const result = engine.executePreTrade(
      sizedOrder("5", "100", ORDER_SIZE_OVERRIDE_ACCOUNT),
    );
    expect(result.ok).toBe(true);
    result.reservation!.rollback();
  });

  it("treats explicit zero settlement-asset caps as present", () => {
    const engine = settlementAssetEngine(
      new OrderSizeLimit(undefined, "0"),
      new OrderSizeLimit(undefined, "0"),
    );

    const asset = engine.executePreTrade(sizedOrder("1", "1"));
    expect(asset.ok).toBe(false);
    expect(asset.rejects[0]?.code).toBe("OrderNotionalExceedsLimit");
    const accountAsset = engine.executePreTrade(
      sizedOrder("1", "1", ORDER_SIZE_OVERRIDE_ACCOUNT),
    );
    expect(accountAsset.ok).toBe(false);
    expect(accountAsset.rejects[0]?.code).toBe("OrderNotionalExceedsLimit");
  });

  it("treats an explicit zero cap as present", () => {
    const engine = brokerEngine(new OrderSizeLimit("0", undefined));

    const result = engine.executePreTrade(sizedOrder("1", "1"));
    expect(result.ok).toBe(false);
    expect(result.rejects[0]?.code).toBe("OrderQtyExceedsLimit");
  });

  it("surfaces the core error when neither cap is configured", () => {
    let caught: unknown;
    try {
      Engine.builder().builtin(
        buildOrderSizeLimit().brokerBarrier(
          new OrderSizeBrokerBarrier(new OrderSizeLimit(undefined, undefined)),
        ),
      );
    } catch (error) {
      caught = error;
    }

    expect(caught).toBeInstanceOf(EngineBuildError);
    expect((caught as Error).message).toBe(
      "at least one of max_quantity or max_notional must be configured",
    );
  });

  it("surfaces the core error for a duplicate asset key", () => {
    let caught: unknown;
    try {
      Engine.builder().builtin(
        buildOrderSizeLimit().assetBarriers([
          new OrderSizeAssetBarrier(
            new OrderSizeLimit("10", undefined),
            "AAPL",
          ),
          new OrderSizeAssetBarrier({ maxNotional: "1000" }, "AAPL"),
        ]),
      );
    } catch (error) {
      caught = error;
    }

    expect(caught).toBeInstanceOf(EngineBuildError);
    expect((caught as Error).message).toBe(
      "duplicate asset barrier for asset AAPL",
    );
  });
});

describe("spot-funds pricing-source compatibility", () => {
  it.each(["MARK", "BOOK_TOP"] as const)(
    "accepts the legacy %s runtime alias without exposing it in the type",
    (pricingSource) => {
      const marketData = Engine.builder()
        .marketData(QuoteTtl.infinite())
        .build();
      expect(() =>
        buildSpotFunds().marketData(
          marketData,
          0,
          pricingSource as never,
          undefined,
        ),
      ).not.toThrow();
    },
  );
});

describe("native JS validation categories", () => {
  it("uses TypeError for a malformed policy object", () => {
    expect(() => Engine.builder().preTrade(null as unknown as Policy)).toThrow(
      TypeError,
    );
  });

  it("uses RangeError for a well-typed value outside its range", () => {
    expect(() => new RateLimit(-1, 1_000)).toThrow(RangeError);
  });
});

describe("spot-funds validation", () => {
  it("rejects an invalid spot-funds P&L barrier currency", () => {
    expect(() => new SpotFundsPnlBoundsBarrier("", "-100", undefined)).toThrow(
      AssetError,
    );
  });

  it("accepts account, group, and instrument override targets separately", () => {
    expect(() => new SpotFundsOverride(1n, 2n, undefined, 0)).not.toThrow();
    expect(() => new SpotFundsOverride(1n, undefined, 3, 0)).not.toThrow();
    expect(
      () => new SpotFundsOverride(1n, undefined, undefined, 0),
    ).not.toThrow();
  });

  it("rejects conflicting override scopes in the JS binding", () => {
    let caught: unknown;
    try {
      new SpotFundsOverride(1n, 2n, 3, 0);
    } catch (error) {
      caught = error;
    }

    expect(caught).toBeInstanceOf(ParamError);
    expect((caught as Error).message).toBe(
      "accountId and accountGroupId are mutually exclusive",
    );
  });

  it("surfaces the core error for an empty P&L barrier set", () => {
    let caught: unknown;
    try {
      // Deliberately bypass the TypeScript stage to exercise the runtime guard.
      Engine.builder()
        .builtin(buildSpotFundsPnlBoundsKillswitch() as never)
        .build();
    } catch (error) {
      caught = error;
    }

    expect(caught).toBeInstanceOf(EngineBuildError);
    expect((caught as Error).message).toBe(
      "spot funds P&L bounds require at least one barrier",
    );
  });

  it("requires at least one bound in each spot-funds P&L barrier", () => {
    expect(() =>
      Engine.builder()
        .builtin(
          buildSpotFundsPnlBoundsKillswitch().globalBarrier(
            new SpotFundsPnlBoundsBarrier("USD", undefined, undefined),
          ),
        )
        .build(),
    ).toThrow("spot-funds P&L bounds must configure at least one bound");
  });

  it("accepts a non-empty P&L barrier set", () => {
    expect(() =>
      Engine.builder()
        .builtin(
          buildSpotFundsPnlBoundsKillswitch().globalBarrier(
            new SpotFundsPnlBoundsBarrier("USD", "-100", undefined),
          ),
        )
        .build(),
    ).not.toThrow();
  });

  it("wires market data into the P&L-bounds builder", () => {
    const barrier = () =>
      new SpotFundsPnlBoundsBarrier("USD", "-100", undefined);
    const marketOrder = {
      operation: {
        underlyingAsset: "AAPL",
        settlementAsset: "USD",
        accountId: 99_224_416n,
        side: "BUY" as const,
        tradeAmount: TradeAmount.quantity("1"),
      },
    };

    const withoutMarketData = Engine.builder()
      .builtin(buildSpotFundsPnlBoundsKillswitch().globalBarrier(barrier()))
      .build();
    expect(
      withoutMarketData.executePreTrade(marketOrder).rejects[0]?.code,
    ).toBe("UnsupportedOrderType");

    const marketData = Engine.builder().marketData(QuoteTtl.infinite()).build();
    marketData.pushByInstrument(
      { underlyingAsset: "AAPL", settlementAsset: "USD" },
      { mark: "100" },
      0,
    );
    const withMarketData = Engine.builder()
      .builtin(
        buildSpotFundsPnlBoundsKillswitch()
          .globalBarrier(barrier())
          .marketData(marketData),
      )
      .build();
    const accepted = withMarketData.executePreTrade(marketOrder);
    expect(accepted.ok).toBe(true);
    accepted.reservation!.rollback();
  });
});
