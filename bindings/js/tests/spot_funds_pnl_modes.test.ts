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
  Engine,
  EngineBuildError,
  PolicyConfigureError,
} from "@openpit/engine";
import { TradeAmount } from "@openpit/engine/param";
import { PnlHaltReason } from "@openpit/engine/pretrade";
import {
  buildSpotFundsPnlBoundsKillswitch,
  SpotFundsPnlBoundsAccountBarrier,
  SpotFundsPnlBoundsAccountGroupBarrier,
  SpotFundsPnlBoundsBarrier,
  SpotFundsPnlBoundsKillswitchBuilder,
} from "@openpit/engine/pretrade/policies";
import { RuleMode } from "@openpit/engine/reject";

const ACCOUNT = 83030n;
const GROUP = 83030;
const NAME = SpotFundsPnlBoundsKillswitchBuilder.NAME;
const TIERS = ["global", "group", "account"] as const;
type Tier = (typeof TIERS)[number];

function order() {
  return {
    operation: {
      underlyingAsset: "AAPL",
      settlementAsset: "USD",
      accountId: ACCOUNT,
      side: "BUY" as const,
      tradeAmount: TradeAmount.quantity("1"),
      price: "100",
    },
  };
}

function builderFor(tier: Tier, barriers: SpotFundsPnlBoundsBarrier[]) {
  const builder = buildSpotFundsPnlBoundsKillswitch();
  switch (tier) {
    case "global":
      return builder.globalBarriers(barriers);
    case "group":
      return builder.accountGroupBarriers(
        barriers.map(
          (barrier) =>
            new SpotFundsPnlBoundsAccountGroupBarrier(GROUP, barrier),
        ),
      );
    case "account":
      return builder.accountBarriers(
        barriers.map(
          (barrier) => new SpotFundsPnlBoundsAccountBarrier(ACCOUNT, barrier),
        ),
      );
  }
}

function setup(engine: Engine) {
  engine.accounts().setCurrency(ACCOUNT, "USD");
  engine.accounts().registerGroup([ACCOUNT], GROUP);
}

function setPnl(engine: Engine, state: string | PnlHaltReason) {
  return engine.configure().setSpotFundsAccountPnl(NAME, {
    account: ACCOUNT,
    state,
  });
}

describe("spot-funds P&L modes", () => {
  describe.each(TIERS)("%s tier", (tier) => {
    it.each([RuleMode.ReviewWithHold, RuleMode.ReviewWithoutHold])(
      "passes %s to the engine alongside a reject barrier",
      (mode) => {
        const review = new SpotFundsPnlBoundsBarrier(
          "USD",
          "-10",
          undefined,
          mode,
        );
        const reject = new SpotFundsPnlBoundsBarrier("USD", "-100", undefined);
        // A second global call must append, including on the global tier.
        const ready = builderFor(tier, [review]).globalBarriers([reject]);
        const engine = Engine.builder().builtin(ready).build();
        setup(engine);
        expect(setPnl(engine, "-20").accountBlocks).toHaveLength(0);
        const start = engine.startPreTrade(order());
        expect(start.ok).toBe(true);
        expect(start.reviews).toEqual([]);
        const dryRun = engine.executePreTradeDryRun(order());
        const result = engine.executePreTrade(order());
        expect(dryRun.isPass).toBe(true);
        expect(result.ok).toBe(true);
        for (const output of [dryRun, result]) {
          expect(output.rejects).toEqual([]);
          expect(output.reviews.map((mark) => mark.mode)).toEqual([mode]);
          expect(output.reviews[0]!.code).toBe("PnlKillSwitchTriggered");
        }
        expect(result.reservation!.reviews().map((mark) => mark.mode)).toEqual([
          mode,
        ]);
        result.reservation!.rollback();
        const confirmed = { ...order(), review: { confirmed: true } };
        const confirmedDry = engine.executePreTradeDryRun(confirmed);
        expect(confirmedDry.isPass).toBe(true);
        expect(confirmedDry.reviews).toEqual([]);
        const approved = engine.executePreTrade(confirmed);
        expect(approved.ok).toBe(true);
        expect(approved.reviews).toEqual([]);
        approved.reservation!.rollback();
        expect(setPnl(engine, "-150").accountBlocks).toHaveLength(1);
        const blocked = engine.executePreTrade(order());
        expect(blocked.ok).toBe(false);
        expect(blocked.rejects[0]!.code).toBe("PnlKillSwitchTriggered");
        expect(blocked.rejects[0]!.mode).toBe(RuleMode.Reject);
        engine.accounts().unblock(ACCOUNT);
        const rejectedDry = engine.executePreTradeDryRun(confirmed);
        expect(rejectedDry.isPass).toBe(false);
        expect(rejectedDry.reviews).toEqual([]);
        expect(rejectedDry.rejects).toHaveLength(1);
        expect(rejectedDry.rejects[0]).toMatchObject({
          code: "PnlKillSwitchTriggered",
          mode: RuleMode.Reject,
        });
        const rejected = engine.executePreTrade(confirmed);
        expect(rejected.ok).toBe(false);
        expect(rejected.rejects).toHaveLength(1);
        expect(rejected.rejects[0]!.code).toBe("PnlKillSwitchTriggered");
        expect(rejected.rejects[0]!.mode).toBe(RuleMode.Reject);
      },
    );

    it.each(["omitted", "undefined", "null"] as const)(
      "defaults %s mode to Reject",
      (input) => {
        const barrier =
          input === "omitted"
            ? new SpotFundsPnlBoundsBarrier("USD", "-10", undefined)
            : new SpotFundsPnlBoundsBarrier(
                "USD",
                "-10",
                undefined,
                input === "null" ? null : undefined,
              );
        const engine = Engine.builder()
          .builtin(builderFor(tier, [barrier]))
          .build();
        setup(engine);
        expect(setPnl(engine, "-20").accountBlocks).toHaveLength(1);
        const result = engine.executePreTrade(order());
        expect(result.ok).toBe(false);
        expect(result.rejects[0]!.mode).toBe(RuleMode.Reject);
        expect(result.reviews).toEqual([]);
      },
    );

    it.each([RuleMode.ReviewWithHold, RuleMode.ReviewWithoutHold])(
      "blocks halted P&L even under %s only",
      (mode) => {
        const engine = Engine.builder()
          .builtin(
            builderFor(tier, [
              new SpotFundsPnlBoundsBarrier("USD", "-10", undefined, mode),
            ]),
          )
          .build();
        setup(engine);
        const halted = setPnl(engine, PnlHaltReason.fromMissingFx());
        expect(halted.accountBlocks).toHaveLength(1);
        expect(halted.accountBlocks[0]!.code).toBe("PnlKillSwitchTriggered");
        expect(halted.accountBlocks[0]!.reason).toBe(
          "account pnl calculation halted",
        );
        const result = engine.executePreTrade(order());
        expect(result.ok).toBe(false);
        expect(result.rejects[0]!.code).toBe("PnlKillSwitchTriggered");
        expect(result.rejects[0]!.mode).toBe(RuleMode.Reject);
        expect(result.reviews).toEqual([]);
      },
    );

    it("rejects a duplicate mode at engine build", () => {
      const barrier = new SpotFundsPnlBoundsBarrier(
        "USD",
        "-10",
        undefined,
        RuleMode.ReviewWithHold,
      );
      const ready = builderFor(tier, [barrier, barrier]);
      let failure: unknown;
      try {
        Engine.builder().builtin(ready).build();
      } catch (error) {
        failure = error;
      }
      expect(failure).toBeInstanceOf(EngineBuildError);
      expect(failure).toMatchObject({
        kind: "InvalidConfiguration",
      });
      expect((failure as Error).message).toMatch(/duplicate.*ReviewWithHold/);
    });

    it("keeps all previous settings after a duplicate runtime key", () => {
      const old = new SpotFundsPnlBoundsBarrier(
        "USD",
        "-10",
        undefined,
        RuleMode.ReviewWithHold,
      );
      const engine = Engine.builder()
        .builtin(builderFor("global", [old]))
        .build();
      setup(engine);
      setPnl(engine, "-20");
      const next = new SpotFundsPnlBoundsBarrier("USD", "-100", undefined);
      const patch = {
        globalBarriers: tier === "global" ? [next, next] : [next],
        ...(tier === "group"
          ? {
              accountGroupBarriers: [
                new SpotFundsPnlBoundsAccountGroupBarrier(GROUP, next),
                new SpotFundsPnlBoundsAccountGroupBarrier(GROUP, next),
              ],
            }
          : {}),
        ...(tier === "account"
          ? {
              accountBarriers: [
                new SpotFundsPnlBoundsAccountBarrier(ACCOUNT, next),
                new SpotFundsPnlBoundsAccountBarrier(ACCOUNT, next),
              ],
            }
          : {}),
      };
      expect(() =>
        engine.configure().spotFundsPnlBoundsKillswitch(NAME, patch),
      ).toThrow(PolicyConfigureError);
      expect(() =>
        engine.configure().spotFundsPnlBoundsKillswitch(NAME, patch),
      ).toThrow(/duplicate/);
      const result = engine.executePreTrade(order());
      expect(result.ok).toBe(true);
      expect(result.reviews.map((mark) => mark.mode)).toEqual([
        RuleMode.ReviewWithHold,
      ]);
      result.reservation!.rollback();
      expect(setPnl(engine, "-150").accountBlocks).toHaveLength(0);
      expect(engine.startPreTrade(order()).ok).toBe(true);
    });
  });

  it.each(["reject", "reviewwithhold", "Unknown", " Reject"])(
    "rejects invalid string mode %s in the constructor",
    (mode) => {
      let failure: unknown;
      try {
        new SpotFundsPnlBoundsBarrier("USD", "-10", undefined, mode as never);
      } catch (error) {
        failure = error;
      }
      expect(failure).toBeInstanceOf(RangeError);
      expect((failure as Error).message).toBe(
        'mode must be "Reject", "ReviewWithHold", or "ReviewWithoutHold"',
      );
    },
  );

  it.each([1, true, {}, []])(
    "rejects non-string mode %j in the constructor",
    (mode) => {
      let failure: unknown;
      try {
        new SpotFundsPnlBoundsBarrier("USD", "-10", undefined, mode as never);
      } catch (error) {
        failure = error;
      }
      expect(failure).toBeInstanceOf(TypeError);
      expect((failure as Error).message).toBe("mode must be a string");
    },
  );

  it.each([RuleMode.ReviewWithHold, RuleMode.ReviewWithoutHold])(
    "blocks an account currency mismatch under %s",
    (mode) => {
      const engine = Engine.builder()
        .builtin(
          builderFor("account", [
            new SpotFundsPnlBoundsBarrier("EUR", "-10", undefined, mode),
          ]),
        )
        .build();
      setup(engine);
      const mismatch = setPnl(engine, "0");
      expect(mismatch.accountBlocks).toHaveLength(1);
      expect(mismatch.accountBlocks[0]!.reason).toBe(
        "pnl barrier currency mismatch",
      );
      const result = engine.executePreTrade(order());
      expect(result.ok).toBe(false);
      expect(result.rejects[0]!.code).toBe("PnlKillSwitchTriggered");
      expect(result.rejects[0]!.reason).toBe("pnl barrier currency mismatch");
      expect(result.rejects[0]!.mode).toBe(RuleMode.Reject);
      expect(result.reviews).toEqual([]);
    },
  );

  it.each(["omitted", "null", "empty", "replace", "review"] as const)(
    "applies the %s global patch across every mode",
    (patch) => {
      const engine = Engine.builder()
        .builtin(
          builderFor("global", [
            new SpotFundsPnlBoundsBarrier("USD", "-100", undefined),
            new SpotFundsPnlBoundsBarrier(
              "USD",
              "-10",
              undefined,
              RuleMode.ReviewWithoutHold,
            ),
          ]),
        )
        .build();
      setup(engine);
      expect(setPnl(engine, "-20").accountBlocks).toHaveLength(0);
      const options =
        patch === "omitted"
          ? {}
          : {
              globalBarriers:
                patch === "null"
                  ? null
                  : patch === "empty"
                    ? []
                    : new Set([
                        new SpotFundsPnlBoundsBarrier(
                          "USD",
                          patch === "replace" ? "-30" : "-10",
                          undefined,
                          patch === "replace"
                            ? RuleMode.Reject
                            : RuleMode.ReviewWithHold,
                        ),
                      ]),
            };
      const retuned = engine
        .configure()
        .spotFundsPnlBoundsKillswitch(NAME, options);
      expect(retuned.accountBlocks).toHaveLength(0);
      const result = engine.executePreTrade(order());
      expect(result.ok).toBe(true);
      const expected =
        patch === "omitted" || patch === "null"
          ? [RuleMode.ReviewWithoutHold]
          : patch === "review"
            ? [RuleMode.ReviewWithHold]
            : [];
      expect(result.reviews.map((mark) => mark.mode)).toEqual(expected);
      result.reservation!.rollback();
      expect(setPnl(engine, "-40").accountBlocks.length > 0).toBe(
        patch === "replace",
      );
      if (patch !== "replace") {
        expect(setPnl(engine, "-150").accountBlocks.length > 0).toBe(
          patch === "omitted" || patch === "null",
        );
      }
    },
  );

  it("rejects scalar globalBarriers at build and runtime", () => {
    const barrier = new SpotFundsPnlBoundsBarrier("USD", "-10", undefined);
    expect(() =>
      buildSpotFundsPnlBoundsKillswitch().globalBarriers(barrier as never),
    ).toThrow(/globalBarriers/);
    const engine = Engine.builder()
      .builtin(builderFor("global", [barrier]))
      .build();
    expect(() =>
      engine.configure().spotFundsPnlBoundsKillswitch(NAME, {
        globalBarriers: barrier as never,
      }),
    ).toThrow(/globalBarriers/);
    expect(buildSpotFundsPnlBoundsKillswitch()).not.toHaveProperty(
      "globalBarrier",
    );
  });
});
