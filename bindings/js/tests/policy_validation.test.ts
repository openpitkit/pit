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

import { Engine, EngineBuildError, ParamError } from "@openpit/engine";
import { QuoteTtl } from "@openpit/engine/marketdata";
import { TradeAmount } from "@openpit/engine/param";
import type { Policy } from "@openpit/engine/pretrade";
import {
  buildRateLimit,
  buildSpotFunds,
  buildSpotFundsPnlBoundsKillswitch,
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
            new SpotFundsPnlBoundsBarrier(undefined, undefined),
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
            new SpotFundsPnlBoundsBarrier("-100", undefined),
          ),
        )
        .build(),
    ).not.toThrow();
  });

  it("wires market data into the P&L-bounds builder", () => {
    const barrier = () => new SpotFundsPnlBoundsBarrier("-100", undefined);
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
