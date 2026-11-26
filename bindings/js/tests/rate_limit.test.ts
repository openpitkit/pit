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
  ConfigureErrorKind,
  Engine,
  EngineBuildError,
  PolicyCallbackError,
  PolicyConfigureError,
} from "@openpit/engine";
import type { OrderInit } from "@openpit/engine/model";
import { TradeAmount } from "@openpit/engine/param";
import {
  buildRateLimit,
  RateLimitAccountAssetBarrier,
  RateLimitAccountBarrier,
  RateLimitAssetBarrier,
  RateLimitBrokerBarrier,
  RateLimitBuilder,
  type RateLimitConfigureOptions,
} from "@openpit/engine/pretrade/policies";
import { RuleMode } from "@openpit/engine/reject";

const ACCOUNT = 99_224_416n;
const WINDOW_MS = 3_600_000;

function order(): OrderInit {
  return {
    operation: {
      underlyingAsset: "AAPL",
      settlementAsset: "USD",
      accountId: ACCOUNT,
      side: "BUY",
      tradeAmount: TradeAmount.quantity("1"),
      price: "100",
    },
  };
}

const axes = [
  {
    name: "broker",
    field: "RateLimitBrokerBarrier.mode",
    make: (maxOrders: number, mode?: RuleMode | null) =>
      new RateLimitBrokerBarrier({ maxOrders, windowMs: WINDOW_MS }, mode),
    builder: (maxOrders: number, mode: RuleMode, duplicate = false) => {
      const barrier = new RateLimitBrokerBarrier(
        { maxOrders, windowMs: WINDOW_MS },
        mode,
      );
      return buildRateLimit().brokerBarriers(
        duplicate ? [barrier, barrier] : [barrier],
      );
    },
    patch: (maxOrders: number, mode: RuleMode) => ({
      brokerBarriers: [
        new RateLimitBrokerBarrier({ maxOrders, windowMs: WINDOW_MS }, mode),
      ],
    }),
  },
  {
    name: "asset",
    field: "RateLimitAssetBarrier.mode",
    make: (maxOrders: number, mode?: RuleMode | null) =>
      new RateLimitAssetBarrier(
        { maxOrders, windowMs: WINDOW_MS },
        "USD",
        mode,
      ),
    builder: (maxOrders: number, mode: RuleMode, duplicate = false) => {
      const barrier = new RateLimitAssetBarrier(
        { maxOrders, windowMs: WINDOW_MS },
        "USD",
        mode,
      );
      return buildRateLimit().assetBarriers(
        duplicate ? [barrier, barrier] : [barrier],
      );
    },
    patch: (maxOrders: number, mode: RuleMode) => ({
      assetBarriers: [
        new RateLimitAssetBarrier(
          { maxOrders, windowMs: WINDOW_MS },
          "USD",
          mode,
        ),
      ],
    }),
  },
  {
    name: "account",
    field: "RateLimitAccountBarrier.mode",
    make: (maxOrders: number, mode?: RuleMode | null) =>
      new RateLimitAccountBarrier(
        { maxOrders, windowMs: WINDOW_MS },
        ACCOUNT,
        mode,
      ),
    builder: (maxOrders: number, mode: RuleMode, duplicate = false) => {
      const barrier = new RateLimitAccountBarrier(
        { maxOrders, windowMs: WINDOW_MS },
        ACCOUNT,
        mode,
      );
      return buildRateLimit().accountBarriers(
        duplicate ? [barrier, barrier] : [barrier],
      );
    },
    patch: (maxOrders: number, mode: RuleMode) => ({
      accountBarriers: [
        new RateLimitAccountBarrier(
          { maxOrders, windowMs: WINDOW_MS },
          ACCOUNT,
          mode,
        ),
      ],
    }),
  },
  {
    name: "account-asset",
    field: "RateLimitAccountAssetBarrier.mode",
    make: (maxOrders: number, mode?: RuleMode | null) =>
      new RateLimitAccountAssetBarrier(
        { maxOrders, windowMs: WINDOW_MS },
        ACCOUNT,
        "USD",
        mode,
      ),
    builder: (maxOrders: number, mode: RuleMode, duplicate = false) => {
      const barrier = new RateLimitAccountAssetBarrier(
        { maxOrders, windowMs: WINDOW_MS },
        ACCOUNT,
        "USD",
        mode,
      );
      return buildRateLimit().accountAssetBarriers(
        duplicate ? [barrier, barrier] : [barrier],
      );
    },
    patch: (maxOrders: number, mode: RuleMode) => ({
      accountAssetBarriers: [
        new RateLimitAccountAssetBarrier(
          { maxOrders, windowMs: WINDOW_MS },
          ACCOUNT,
          "USD",
          mode,
        ),
      ],
    }),
  },
] as const;

describe("rate-limit review modes", () => {
  it("counts Reject and ReviewWithoutHold broker rules separately", () => {
    const engine = Engine.builder()
      .builtin(
        buildRateLimit()
          .brokerBarriers([axes[0].make(2, RuleMode.Reject)])
          .brokerBarriers(
            new Set([axes[0].make(1, RuleMode.ReviewWithoutHold)]),
          ),
      )
      .build();
    const first = engine.startPreTrade(order());
    expect(first.ok).toBe(true);
    expect(first.reviews).toEqual([]);
    const second = engine.startPreTrade(order());
    expect(second.ok).toBe(true);
    expect(second.reviews).toHaveLength(1);
    expect(second.reviews[0]?.code).toBe("RateLimitExceeded");
    expect(second.reviews[0]?.mode).toBe(RuleMode.ReviewWithoutHold);
    const third = engine.startPreTrade(order());
    expect(third.ok).toBe(false);
    expect(third.rejects[0]?.code).toBe("RateLimitExceeded");
    expect(third.rejects[0]?.mode).toBe(RuleMode.Reject);
  });

  it.each([1, 3])(
    "counts only starts reaching the hook with a Reject budget of %i",
    (rejectLimit) => {
      for (const mode of [
        RuleMode.ReviewWithHold,
        RuleMode.ReviewWithoutHold,
      ]) {
        const engine = Engine.builder()
          .builtin(
            buildRateLimit().brokerBarriers([
              axes[0].make(rejectLimit, RuleMode.Reject),
              axes[0].make(rejectLimit === 1 ? 1 : 2, mode),
            ]),
          )
          .build();
        if (rejectLimit === 1) {
          engine.accounts().block(ACCOUNT, "operator block");
          const blocked = engine.startPreTrade(order());
          expect(blocked.ok).toBe(false);
          expect(blocked.rejects.map(({ code }) => code)).toEqual([
            "AccountBlocked",
          ]);
          engine.accounts().unblock(ACCOUNT);
          const first = engine.startPreTrade(order());
          expect(first.ok).toBe(true);
          expect(first.reviews).toEqual([]);
        } else {
          const first = engine.startPreTrade(order());
          expect(first.ok).toBe(true);
          expect(first.reviews).toEqual([]);
          const executed = first.request!.execute();
          expect(executed.ok).toBe(true);
          executed.reservation!.commit();

          const second = engine.startPreTrade(order());
          expect(second.ok).toBe(true);
          expect(second.reviews).toEqual([]);
          const third = engine.startPreTrade(order());
          expect(third.ok).toBe(true);
          expect(third.reviews).toMatchObject([
            {
              code: "RateLimitExceeded",
              mode,
            },
          ]);
        }
        const exhausted = engine.startPreTrade(order());
        expect(exhausted.ok).toBe(false);
        expect(exhausted.rejects).toMatchObject([
          {
            code: "RateLimitExceeded",
            mode: RuleMode.Reject,
          },
        ]);
      }
    },
  );

  it.each(axes)("forwards both review modes on the $name axis", (axis) => {
    for (const mode of [RuleMode.ReviewWithHold, RuleMode.ReviewWithoutHold]) {
      const engine = Engine.builder().builtin(axis.builder(1, mode)).build();
      expect(engine.startPreTrade(order()).reviews).toEqual([]);
      const reviewed = engine.startPreTrade(order());
      expect(reviewed.ok).toBe(true);
      expect(reviewed.reviews).toHaveLength(1);
      expect(reviewed.reviews[0]?.mode).toBe(mode);
      engine.configure().rateLimit(RateLimitBuilder.NAME, axis.patch(3, mode));
      expect(engine.startPreTrade(order()).reviews).toEqual([]);
      expect(engine.startPreTrade(order()).reviews).toHaveLength(1);
    }
  });

  it.each(
    [RuleMode.ReviewWithHold, RuleMode.ReviewWithoutHold].flatMap((mode) =>
      [false, true].map((alreadyHeld) => ({ mode, alreadyHeld })),
    ),
  )(
    "spends both thresholds and drops review marks under confirmed, mode=$mode, alreadyHeld=$alreadyHeld",
    ({ mode, alreadyHeld }) => {
      const engine = Engine.builder()
        .builtin(
          buildRateLimit().brokerBarriers([
            axes[0].make(7, RuleMode.Reject),
            axes[0].make(4, mode),
          ]),
        )
        .build();
      const input = order();
      const first = engine.startPreTrade(input);
      expect(first.ok).toBe(true);
      expect(first.reviews).toEqual([]);
      input.review = { confirmed: true };
      const second = engine.startPreTrade(input);
      expect(second.ok).toBe(true);
      expect(second.reviews).toEqual([]);
      input.review = { confirmed: true, alreadyHeld: true };
      const third = engine.startPreTrade(input);
      expect(third.ok).toBe(true);
      expect(third.reviews).toEqual([]);
      input.review = { confirmed: false };
      const fourth = engine.startPreTrade(input);
      expect(fourth.ok).toBe(true);
      expect(fourth.reviews).toEqual([]);
      const reviewed = engine.startPreTrade(input);
      expect(reviewed.ok).toBe(true);
      expect(
        reviewed.reviews.map(({ code, mode }) => ({ code, mode })),
      ).toEqual([{ code: "RateLimitExceeded", mode }]);
      input.review = { confirmed: true, alreadyHeld: true };
      const heldRerun = engine.startPreTrade(input);
      expect(heldRerun.ok).toBe(true);
      expect(heldRerun.reviews).toEqual([]);
      input.review = { confirmed: true };
      const rerun = engine.startPreTrade(input);
      expect(rerun.ok).toBe(true);
      expect(rerun.reviews).toEqual([]);
      input.review = { confirmed: true, alreadyHeld };
      const rejected = engine.startPreTrade(input);
      expect(rejected.ok).toBe(false);
      expect(
        rejected.rejects.map(({ code, mode }) => ({ code, mode })),
      ).toEqual([{ code: "RateLimitExceeded", mode: RuleMode.Reject }]);
    },
  );

  it.each(axes)("rejects duplicate $name keys atomically", (axis) => {
    const repeated = axis.patch(10, RuleMode.Reject);
    const field = Object.keys(repeated)[0] as keyof RateLimitConfigureOptions;
    const barriers = [...Object.values(repeated)[0]!];
    const duplicate = { [field]: [...barriers, ...barriers] };
    const build = () =>
      Engine.builder()
        .builtin(axis.builder(10, RuleMode.Reject, true))
        .build();
    let buildError: unknown;
    try {
      build();
    } catch (error) {
      buildError = error;
    }
    expect(buildError).toBeInstanceOf(EngineBuildError);
    expect((buildError as EngineBuildError).kind).toBe("InvalidConfiguration");
    expect((buildError as Error).message).toContain("Reject");
    expect((buildError as Error).message).toContain("duplicate");

    const engine = Engine.builder()
      .builtin(axis.builder(1, RuleMode.Reject))
      .build();
    expect(engine.startPreTrade(order()).ok).toBe(true);
    let configureError: unknown;
    try {
      engine.configure().rateLimit(RateLimitBuilder.NAME, duplicate);
    } catch (error) {
      configureError = error;
    }
    expect(configureError).toBeInstanceOf(PolicyConfigureError);
    expect((configureError as PolicyConfigureError).kind).toBe(
      ConfigureErrorKind.Validation,
    );
    expect((configureError as Error).message).toContain("Reject");
    expect((configureError as Error).message).toContain("duplicate");
    const unchanged = engine.startPreTrade(order());
    expect(unchanged.ok).toBe(false);
    expect(unchanged.rejects[0]?.code).toBe("RateLimitExceeded");
  });

  it("accepts one asset with two different modes", () => {
    const engine = Engine.builder()
      .builtin(
        buildRateLimit().assetBarriers([
          axes[1].make(2, RuleMode.Reject),
          axes[1].make(1, RuleMode.ReviewWithoutHold),
        ]),
      )
      .build();
    expect(engine.startPreTrade(order()).reviews).toEqual([]);
    const second = engine.startPreTrade(order());
    expect(second.ok).toBe(true);
    expect(second.reviews).toHaveLength(1);
    expect(second.reviews[0]?.mode).toBe(RuleMode.ReviewWithoutHold);
    expect(engine.startPreTrade(order()).ok).toBe(false);
  });

  it.each(axes)("validates and exposes the $name mode", (axis) => {
    const field = axis.field;
    expect(axis.make(1).mode).toBe(RuleMode.Reject);
    expect(axis.make(1, null).mode).toBe(RuleMode.Reject);
    for (const mode of Object.values(RuleMode)) {
      expect(axis.make(1, mode).clone().mode).toBe(mode);
    }
    for (const mode of ["reject", "Review"]) {
      expect(() => axis.make(1, mode as RuleMode)).toThrow(RangeError);
      expect(() => axis.make(1, mode as RuleMode)).toThrow(
        `${field} must be "Reject", "ReviewWithHold", or "ReviewWithoutHold"`,
      );
    }
    expect(() => axis.make(1, 1 as unknown as RuleMode)).toThrow(TypeError);
    expect(() => axis.make(1, 1 as unknown as RuleMode)).toThrow(
      `${field} must be a string`,
    );
  });

  it("preserves reject.mode error messages", () => {
    for (const [mode, causeType, message] of [
      [
        "reject",
        RangeError,
        'reject.mode must be "Reject", "ReviewWithHold", or "ReviewWithoutHold"',
      ],
      [1, TypeError, "reject.mode must be a string"],
    ] as const) {
      const engine = Engine.builder()
        .preTrade({
          name: "invalid-mode",
          checkPreTradeStart: () => [
            {
              code: "Custom",
              reason: "invalid",
              details: "",
              mode: mode as RuleMode,
            },
          ],
          performPreTradeCheck: () => null,
        })
        .build();
      let caught: unknown;
      try {
        engine.startPreTrade(order());
      } catch (error) {
        caught = error;
      }
      expect(caught).toBeInstanceOf(PolicyCallbackError);
      const cause = (caught as PolicyCallbackError).cause;
      expect(cause).toBeInstanceOf(causeType);
      expect((cause as Error).message).toBe(message);
    }
  });

  it("keeps omitted brokers, replaces all modes, and clears exhausted brokers", () => {
    const engine = Engine.builder()
      .builtin(
        buildRateLimit().brokerBarriers([
          axes[0].make(1, RuleMode.Reject),
          axes[0].make(1, RuleMode.ReviewWithHold),
          axes[0].make(1, RuleMode.ReviewWithoutHold),
        ]),
      )
      .build();
    expect(engine.startPreTrade(order()).ok).toBe(true);
    engine.configure().rateLimit(RateLimitBuilder.NAME, {});
    expect(engine.startPreTrade(order()).ok).toBe(false);
    engine
      .configure()
      .rateLimit(RateLimitBuilder.NAME, { brokerBarriers: null });
    expect(engine.startPreTrade(order()).ok).toBe(false);
    engine.configure().rateLimit(RateLimitBuilder.NAME, { brokerBarriers: [] });
    const cleared = engine.startPreTrade(order());
    expect(cleared.ok).toBe(true);
    expect(cleared.reviews).toEqual([]);
    engine
      .configure()
      .rateLimit(
        RateLimitBuilder.NAME,
        axes[0].patch(1, RuleMode.ReviewWithoutHold),
      );
    expect(engine.startPreTrade(order()).reviews).toEqual([]);
    engine.configure().rateLimit(RateLimitBuilder.NAME, {
      brokerBarriers: new Set([axes[0].make(10, RuleMode.Reject)]),
    });
    const replaced = engine.startPreTrade(order());
    expect(replaced.ok).toBe(true);
    expect(replaced.reviews).toEqual([]);
  });
});
