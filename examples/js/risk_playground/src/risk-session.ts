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

import { Engine } from "@openpit/engine";
import { AdjustmentAmount, Pnl, TradeAmount } from "@openpit/engine/param";
import {
  type ExecutionReportInit,
  type OrderInit,
} from "@openpit/engine/model";
import { type AccountAdjustmentOutcome } from "@openpit/engine/accountadjustment";
import {
  type AccountPnlOutcome,
  type Lock,
} from "@openpit/engine/pretrade";
import {
  buildOrderSizeLimit,
  buildRateLimit,
  buildSpotFunds,
  OrderSizeBrokerBarrier,
  OrderSizeLimit,
  OrderSizeLimitBuilder,
  RateLimit,
  RateLimitAccountBarrier,
  RateLimitBuilder,
  SpotFundsBuilder,
  SpotFundsPnlBoundsAccountBarrier,
  SpotFundsPnlBoundsBarrier,
} from "@openpit/engine/pretrade/policies";
import { RejectCode } from "@openpit/engine/reject";

const ACCOUNT = "risk-playground";
const FEE_CURRENCY = "USD";
const TRADED_ASSET = "SPCX";
const RATE_WINDOW_MS = 60_000;
const MONEY_SCALE = 4;
const MONEY_FACTOR = 10_000n;
const SIMULATED_VENUE_FEE = {
  amount: "0.25",
  currency: FEE_CURRENCY,
} as const;

export type CheckName = "pnl" | "funds" | "size" | "value" | "rate";

export interface DemoLimits {
  readonly maxShares: bigint;
  readonly maxValue: bigint;
  readonly maxRate: number;
  readonly lossLimit: bigint;
  readonly spotFundsLimit: bigint;
}

export interface DemoOrder {
  readonly side: "BUY" | "SELL";
  readonly quantity: bigint;
  readonly price: string;
}

export interface ExecutionLifecycle {
  readonly accountBlockCode: string | null;
  readonly feeAmount: string;
  readonly feeCurrency: string;
}

export interface GateDecision {
  readonly accepted: boolean;
  readonly execution: ExecutionLifecycle | null;
  readonly rejects: readonly GateReject[];
}

export interface GateReject {
  readonly checks: readonly CheckName[];
  readonly code: string;
  readonly details: string;
  readonly policy: string;
  readonly reason: string;
  readonly scope: string;
}

export interface SessionSnapshot {
  readonly availableFundsAtoms: bigint;
  readonly availableSharesAtoms: bigint;
  readonly halted: boolean;
  readonly lastOrderValueAtoms: bigint | null;
  readonly pnlAtoms: bigint;
}

export const DEFAULT_LIMITS: DemoLimits = {
  maxShares: 500n,
  maxValue: 5_000n,
  maxRate: 5,
  lossLimit: 50n,
  spotFundsLimit: 10_000n,
};

/** Runs the Playground's real OpenPit pre-trade and fill lifecycle. */
export class RiskSession {
  private limits: DemoLimits;
  private engine: Engine;
  private availableFundsAtoms = 0n;
  private availableSharesAtoms = 0n;
  private lastOrderValueAtoms: bigint | null = null;
  private pnlAtoms = 0n;
  private halted = false;

  public constructor(limits: DemoLimits = DEFAULT_LIMITS) {
    assertLimits(limits);
    this.limits = limits;
    this.engine = buildEngine(limits);
    this.applySpotFundsOutcomes(
      adjustFunds(
        this.engine,
        AdjustmentAmount.absolute(limits.spotFundsLimit.toString()),
      ),
    );
  }

  /** Returns the limits the running engine currently enforces. */
  public appliedLimits(): DemoLimits {
    return this.limits;
  }

  /**
   * Applies updated policy settings to the running browser engine.
   *
   * Every barrier is built before the first retune, so a value the binding
   * rejects leaves the engine on its previous limits. A changed cash allocation
   * moves the Funds balance by the difference, keeping what the session spent.
   */
  public updateLimits(limits: DemoLimits): void {
    assertLimits(limits);
    const sizeBarrier = orderSizeBarrier(limits);
    const accountRateBarrier = rateBarrier(limits);
    const pnlBarrier = pnlBoundsBarrier(limits);

    this.engine.configure().orderSizeLimit(OrderSizeLimitBuilder.NAME, {
      broker: sizeBarrier,
    });
    this.engine.configure().rateLimit(RateLimitBuilder.NAME, {
      accountBarriers: [accountRateBarrier],
    });
    // A tighter loss limit blocks an account already beyond it right here.
    const pnlBlocks = this.engine
      .configure()
      .spotFundsPnlBoundsKillswitch(SpotFundsBuilder.NAME, {
        accountBarriers: [pnlBarrier],
      });
    if (pnlBlocks.accountBlocks.length > 0) {
      this.halted = true;
    }

    if (limits.spotFundsLimit !== this.limits.spotFundsLimit) {
      const change = limits.spotFundsLimit - this.limits.spotFundsLimit;
      this.applySpotFundsOutcomes(
        adjustFunds(this.engine, AdjustmentAmount.delta(change.toString())),
      );
    }

    this.limits = limits;
  }

  /** Restores the demo with a fresh engine and the supplied funding limit. */
  public reset(limits: DemoLimits = DEFAULT_LIMITS): void {
    assertLimits(limits);
    this.limits = limits;
    this.engine = buildEngine(limits);
    this.availableFundsAtoms = 0n;
    this.availableSharesAtoms = 0n;
    this.lastOrderValueAtoms = null;
    this.pnlAtoms = 0n;
    this.halted = false;
    this.applySpotFundsOutcomes(
      adjustFunds(
        this.engine,
        AdjustmentAmount.absolute(limits.spotFundsLimit.toString()),
      ),
    );
  }

  /** Returns state derived from completed Funds execution reports. */
  public snapshot(): SessionSnapshot {
    return {
      availableFundsAtoms: this.availableFundsAtoms,
      availableSharesAtoms: this.availableSharesAtoms,
      halted: this.halted,
      lastOrderValueAtoms: this.lastOrderValueAtoms,
      pnlAtoms: this.pnlAtoms,
    };
  }

  /** Checks, reserves, commits, and immediately settles one simulated fill. */
  public send(order: DemoOrder): GateDecision {
    assertOrder(order);

    const engineOrder = toEngineOrder(order);
    const result = this.engine.executePreTrade(engineOrder);
    if (!result.ok) {
      return rejected(result.rejects.map(toGateReject));
    }

    const reservation = result.reservation;
    if (reservation === undefined) {
      throw new Error("accepted engine result is missing its reservation");
    }

    const lock = reservation.lock();
    const reservationOutcomes = reservation.accountAdjustments();
    reservation.commit();
    this.applySpotFundsOutcomes(reservationOutcomes);

    const postTrade = this.engine.applyExecutionReport(
      toExecutionReport(order, lock),
    );
    const accountBlockCode = postTrade.accountBlocks[0]?.code ?? null;
    this.applySpotFundsOutcomes(postTrade.accountAdjustments);
    this.applyReportedPnl(postTrade.accountPnls);
    this.lastOrderValueAtoms = reservationNotional(
      reservationOutcomes,
      order.side,
    );
    if (accountBlockCode !== null) {
      this.halted = true;
    }

    return {
      accepted: true,
      execution: {
        accountBlockCode,
        feeAmount: SIMULATED_VENUE_FEE.amount,
        feeCurrency: SIMULATED_VENUE_FEE.currency,
      },
      rejects: [],
    };
  }

  private applySpotFundsOutcomes(
    outcomes: readonly AccountAdjustmentOutcome[],
  ): void {
    for (const outcome of outcomes) {
      if (outcome.entry.asset === FEE_CURRENCY) {
        if (outcome.entry.balance !== undefined) {
          this.availableFundsAtoms = decimalToAtoms(
            outcome.entry.balance.absolute.toString(),
          );
        }
      }
      if (outcome.entry.asset === TRADED_ASSET) {
        if (outcome.entry.balance !== undefined) {
          this.availableSharesAtoms = decimalToAtoms(
            outcome.entry.balance.absolute.toString(),
          );
        }
      }
    }
  }

  private applyReportedPnl(outcomes: readonly AccountPnlOutcome[]): void {
    const outcome = outcomes.find((entry) => entry.pnl !== undefined);
    if (outcome?.pnl !== undefined) {
      // P&L follows an unrounded average entry price, so it can carry far more
      // decimals than the page shows; round it at the display scale.
      const shown = Pnl.fromStringRounded(
        outcome.pnl.absolute.toString(),
        MONEY_SCALE,
        "midpointawayfromzero",
      );
      this.pnlAtoms = decimalToAtoms(shown.toString());
    }
  }
}

function buildEngine(limits: DemoLimits): Engine {
  const builder = Engine.builder().builtin(buildSpotFunds());
  builder.builtin(
    buildOrderSizeLimit().brokerBarrier(orderSizeBarrier(limits)),
  );
  builder.builtin(buildRateLimit().accountBarriers([rateBarrier(limits)]));
  const engine = builder.build();
  engine.accounts().setCurrency(ACCOUNT, FEE_CURRENCY);
  // A fresh account has zero P&L, so this first barrier cannot block it.
  engine.configure().spotFundsPnlBoundsKillswitch(SpotFundsBuilder.NAME, {
    accountBarriers: [pnlBoundsBarrier(limits)],
  });
  return engine;
}

function pnlBoundsBarrier(
  limits: DemoLimits,
): SpotFundsPnlBoundsAccountBarrier {
  return new SpotFundsPnlBoundsAccountBarrier(
    ACCOUNT,
    new SpotFundsPnlBoundsBarrier(
      FEE_CURRENCY,
      `-${limits.lossLimit}`,
      undefined,
    ),
  );
}

function adjustFunds(
  engine: Engine,
  amount: AdjustmentAmount,
): readonly AccountAdjustmentOutcome[] {
  const result = engine.applyAccountAdjustment(ACCOUNT, [
    {
      operation: { asset: FEE_CURRENCY },
      amount: { balance: amount },
    },
  ]);
  if (!result.ok) {
    throw new Error("Funds rejected the demo funding adjustment");
  }
  return result.outcomes;
}

function orderSizeBarrier(limits: DemoLimits): OrderSizeBrokerBarrier {
  return new OrderSizeBrokerBarrier(
    new OrderSizeLimit(limits.maxShares.toString(), limits.maxValue.toString()),
  );
}

function rateBarrier(limits: DemoLimits): RateLimitAccountBarrier {
  return new RateLimitAccountBarrier(
    new RateLimit(limits.maxRate, RATE_WINDOW_MS),
    ACCOUNT,
  );
}

function toEngineOrder(order: DemoOrder): OrderInit {
  return {
    operation: {
      underlyingAsset: TRADED_ASSET,
      settlementAsset: FEE_CURRENCY,
      accountId: ACCOUNT,
      side: order.side,
      tradeAmount: TradeAmount.quantity(order.quantity.toString()),
      price: order.price,
    },
  };
}

function toExecutionReport(order: DemoOrder, lock: Lock): ExecutionReportInit {
  return {
    operation: {
      underlyingAsset: TRADED_ASSET,
      settlementAsset: FEE_CURRENCY,
      accountId: ACCOUNT,
      side: order.side,
    },
    fill: {
      fee: {
        amount: SIMULATED_VENUE_FEE.amount,
        currency: SIMULATED_VENUE_FEE.currency,
      },
      isFinal: true,
      lastTrade: {
        price: order.price,
        quantity: order.quantity.toString(),
      },
      remainingReservedQuantity: "0",
      lock,
    },
  };
}

function reservationNotional(
  outcomes: readonly AccountAdjustmentOutcome[],
  side: DemoOrder["side"],
): bigint | null {
  const settlement = outcomes.find(
    (outcome) => outcome.entry.asset === FEE_CURRENCY,
  );
  const amount =
    side === "BUY"
      ? settlement?.entry.held?.delta
      : settlement?.entry.incoming?.delta;
  return amount === undefined ? null : decimalToAtoms(amount.toString());
}

function rejected(rejects: readonly GateReject[]): GateDecision {
  return {
    accepted: false,
    execution: null,
    rejects,
  };
}

function toGateReject(reject: {
  readonly code: string;
  readonly details: string;
  readonly policy: string;
  readonly reason: string;
  readonly scope: string;
}): GateReject {
  return {
    checks: checksForReject(reject.code, reject.policy),
    code: reject.code,
    details: reject.details,
    policy: reject.policy,
    reason: reject.reason,
    scope: reject.scope,
  };
}

function checksForReject(code: string, policy: string): readonly CheckName[] {
  // The P&L axis is the only source of account blocks in the playground.
  if (
    code === RejectCode.PnlKillSwitchTriggered ||
    code === RejectCode.AccountBlocked
  ) {
    return ["pnl"];
  }
  if (policy === RateLimitBuilder.NAME) {
    return ["rate"];
  }
  if (policy === OrderSizeLimitBuilder.NAME) {
    if (code === RejectCode.OrderQtyExceedsLimit) {
      return ["size"];
    }
    if (code === RejectCode.OrderNotionalExceedsLimit) {
      return ["value"];
    }
    return ["size", "value"];
  }
  if (policy === SpotFundsBuilder.NAME) {
    return ["funds"];
  }
  throw new Error(`reject ${code} came from unexpected policy ${policy}`);
}

function assertLimits(limits: DemoLimits): void {
  if (
    limits.maxShares <= 0n ||
    limits.maxValue <= 0n ||
    limits.lossLimit <= 0n ||
    limits.spotFundsLimit <= 0n
  ) {
    throw new RangeError("limits must be positive");
  }
  if (!Number.isSafeInteger(limits.maxRate) || limits.maxRate <= 0) {
    throw new RangeError("maxRate must be a positive safe integer");
  }
}

function assertOrder(order: DemoOrder): void {
  if (order.quantity <= 0n || !isPositiveDecimal(order.price)) {
    throw new RangeError("quantity and price must be positive");
  }
}

function isPositiveDecimal(value: string): boolean {
  return /^\d+(?:\.\d{1,4})?$/.test(value) && /[1-9]/.test(value);
}

export function decimalToAtoms(value: string): bigint {
  const match = /^(-?)(\d+)(?:\.(\d+))?$/.exec(value);
  if (match === null) {
    throw new RangeError(`expected an exact decimal amount, received ${value}`);
  }
  const fraction = (match[3] ?? "").padEnd(MONEY_SCALE, "0");
  if (fraction.length > MONEY_SCALE) {
    throw new RangeError(`amount has more than ${MONEY_SCALE} decimal places`);
  }
  const atoms = BigInt(match[2]!) * MONEY_FACTOR + BigInt(fraction || "0");
  return match[1] === "-" ? -atoms : atoms;
}
