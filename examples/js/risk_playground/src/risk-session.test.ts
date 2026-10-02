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

import { DEFAULT_LIMITS, RiskSession } from "./risk-session.ts";

const normalOrder = {
  side: "BUY" as const,
  quantity: 30n,
  price: "162.45",
};

describe("RiskSession", () => {
  it("reserves Funds, commits, and settles an immediate fill report", () => {
    const session = new RiskSession();
    const decision = session.send(normalOrder);

    expect(decision).toMatchObject({
      accepted: true,
      execution: {
        accountBlockCode: null,
        feeAmount: "0.25",
        feeCurrency: "USD",
      },
      rejects: [],
    });
    expect(session.snapshot()).toMatchObject({
      availableFundsAtoms: 51_262_500n,
      availableSharesAtoms: 300_000n,
      halted: false,
      lastOrderValueAtoms: 48_735_000n,
      pnlAtoms: -2_500n,
    });
  });

  it("uses the filled SPCX balance for a following sell", () => {
    const session = new RiskSession({
      ...DEFAULT_LIMITS,
      spotFundsLimit: 61_000n,
    });

    expect(session.send(normalOrder)).toMatchObject({ accepted: true });
    expect(session.snapshot().availableSharesAtoms).toBe(300_000n);

    expect(session.send({ ...normalOrder, side: "SELL" })).toMatchObject({
      accepted: true,
    });
    expect(session.snapshot()).toMatchObject({
      availableFundsAtoms: 609_995_000n,
      availableSharesAtoms: 0n,
      pnlAtoms: -5_000n,
    });
  });

  it("shows an average-cost P&L rounded to the display scale", () => {
    const session = new RiskSession();
    const buy = { side: "BUY" as const, quantity: 1n, price: "100" };

    expect(session.send(buy)).toMatchObject({ accepted: true });
    expect(session.send({ ...buy, quantity: 2n, price: "101" })).toMatchObject(
      { accepted: true },
    );
    // Average entry 302/3; selling one share at 100 realizes -0.666...,
    // and three fees add -0.75.
    expect(session.send({ ...buy, side: "SELL" })).toMatchObject({
      accepted: true,
    });
    expect(session.snapshot().pnlAtoms).toBe(-14_167n);
  });

  it("stops at the quantity portion of the real order-size policy", () => {
    const decision = new RiskSession().send({
      ...normalOrder,
      quantity: DEFAULT_LIMITS.maxShares + 1n,
      price: "1",
    });

    expect(decision.accepted).toBe(false);
    expect(decision.rejects).toMatchObject([
      { checks: ["size"], code: "OrderQtyExceedsLimit" },
    ]);
  });

  it("reaches the notional portion after quantity passes", () => {
    const decision = new RiskSession({
      ...DEFAULT_LIMITS,
      spotFundsLimit: 2_000_000n,
    }).send({
      ...normalOrder,
      quantity: DEFAULT_LIMITS.maxShares,
    });

    expect(decision.accepted).toBe(false);
    expect(decision.rejects).toMatchObject([
      { checks: ["value"], code: "OrderNotionalExceedsLimit" },
    ]);
  });

  it("blocks an order that does not fit inside the configured Funds balance", () => {
    const decision = new RiskSession({
      ...DEFAULT_LIMITS,
      spotFundsLimit: 4_000n,
    }).send(normalOrder);

    expect(decision).toMatchObject({
      accepted: false,
      rejects: [{ checks: ["funds"], code: "InsufficientFunds" }],
    });
  });

  it("leaves policy stage ordering to the unified engine", () => {
    const decision = new RiskSession({
      ...DEFAULT_LIMITS,
      spotFundsLimit: 1n,
    }).send({
      ...normalOrder,
      quantity: DEFAULT_LIMITS.maxShares + 1n,
    });

    expect(decision.accepted).toBe(false);
    expect(decision.rejects.map((reject) => reject.code)).toEqual([
      "OrderExceedsLimit",
    ]);
    expect(decision.rejects[0]?.checks).toEqual(["size", "value"]);
  });

  it("blocks the maxRate + 1 order inside the 60-second window", () => {
    const session = new RiskSession();
    const smallOrder = {
      side: "BUY" as const,
      quantity: 1n,
      price: "100",
    };
    for (let index = 0; index < DEFAULT_LIMITS.maxRate; index += 1) {
      expect(session.send(smallOrder)).toMatchObject({
        accepted: true,
      });
    }

    expect(session.send(smallOrder)).toMatchObject({
      accepted: false,
      rejects: [{ checks: ["rate"], code: "RateLimitExceeded" }],
    });
  });

  it("uses the Funds kill switch reported by a loss-making execution", () => {
    const session = new RiskSession();
    expect(session.send(normalOrder)).toMatchObject({ accepted: true });

    const loss = session.send({
      ...normalOrder,
      price: "1",
      side: "SELL",
    });

    expect(loss).toMatchObject({
      accepted: true,
      execution: { accountBlockCode: "PnlKillSwitchTriggered" },
    });
    expect(session.snapshot()).toMatchObject({
      halted: true,
      pnlAtoms: -48_440_000n,
    });
    expect(session.send(normalOrder)).toMatchObject({
      accepted: false,
      rejects: [{ checks: ["pnl"] }],
    });
  });

  it("resets the live policies with the visible default limits", () => {
    const session = new RiskSession();
    session.updateLimits({ ...DEFAULT_LIMITS, maxShares: 1n });

    expect(session.send(normalOrder)).toMatchObject({
      accepted: false,
      rejects: [{ checks: ["size"] }],
    });

    session.reset();

    expect(session.send(normalOrder)).toMatchObject({ accepted: true });
  });

  it("rejects non-positive orders before crossing the binding boundary", () => {
    expect(() =>
      new RiskSession().send({ ...normalOrder, quantity: 0n }),
    ).toThrow(RangeError);
  });

  it("halts when a tightened loss limit is already breached", () => {
    const session = new RiskSession();
    expect(session.send(normalOrder)).toMatchObject({ accepted: true });
    expect(
      session.send({ ...normalOrder, side: "SELL", price: "161.50" }),
    ).toMatchObject({ accepted: true });
    expect(session.snapshot()).toMatchObject({
      halted: false,
      pnlAtoms: -290_000n,
    });

    session.updateLimits({ ...DEFAULT_LIMITS, lossLimit: 20n });

    expect(session.snapshot().halted).toBe(true);
    expect(session.send(normalOrder)).toMatchObject({
      accepted: false,
      rejects: [{ checks: ["pnl"] }],
    });
  });

  it("moves the Funds balance by a changed cash allocation", () => {
    const session = new RiskSession();
    expect(session.send(normalOrder)).toMatchObject({ accepted: true });

    session.updateLimits({ ...DEFAULT_LIMITS, spotFundsLimit: 10_001n });

    expect(session.snapshot()).toMatchObject({
      availableFundsAtoms: 51_272_500n,
      availableSharesAtoms: 300_000n,
    });
  });

  it("keeps every previous limit when the binding rejects one", () => {
    const session = new RiskSession();

    expect(() =>
      session.updateLimits({
        ...DEFAULT_LIMITS,
        maxShares: 10n,
        lossLimit: 79_228_162_514_264_337_593_543_950_336n,
      }),
    ).toThrow();

    expect(session.appliedLimits()).toBe(DEFAULT_LIMITS);
    expect(session.send(normalOrder)).toMatchObject({ accepted: true });
  });
});
