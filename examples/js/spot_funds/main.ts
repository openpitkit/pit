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

// Example spot_funds.
//
// The smallest end-to-end integration of OpenPit's built-in SpotFunds pre-trade
// policy: it shows how a buy order reserves settlement cash, how a second order
// is rejected because that cash is still held, and how a fill settles the held
// reservation.
//
// What is illustrated:
//
//   - building a limit-only engine with SpotFunds + OrderValidation
//   - seeding an account's available cash via applyAccountAdjustment
//   - the reservation mechanic: a committed BUY holds settlement funds, so a
//     follow-up BUY that needs the same cash is rejected with InsufficientFunds
//   - tying a fill back to its reservation by carrying the pre-trade lock on the
//     execution report, so SpotFunds settles the right held amount
//
// Audience: an integrator who wants to lift the SpotFunds call pattern into
// their own order/fill pipeline.
//
// What you typically change to adapt this example to your own application:
//
//   1. Engine policies - see buildEngine() below.
//   2. The seed balance and the orders - here they are hard-coded constants
//      chosen so the reservation mechanic is the lesson; your system feeds real
//      account state and strategy orders.
//   3. The print statements - replace them with your order-router and
//      fill-handler side effects.
//
// The example is deliberately flat: main() reads top-to-bottom as a story, and
// every engine call is factored into a small named helper that the smoke test
// reuses. For a table-driven harness around the same policy, see ../spot_table.

import { Engine } from "@openpit/engine";
import { AdjustmentAmount, TradeAmount } from "@openpit/engine/param";
import {
  type AccountAdjustmentInit,
  type ExecutionReportInit,
  type OrderInit,
} from "@openpit/engine/model";
import { type Lock, type PostTradeResult } from "@openpit/engine/pretrade";
import {
  buildOrderValidation,
  buildSpotFunds,
  SpotFundsBuilder,
  SpotFundsLimitMode,
  SpotFundsPnlBoundsAccountBarrierUpdate,
} from "@openpit/engine/pretrade/policies";
import { type Reject } from "@openpit/engine/reject";

// =============================================================================
// Scenario constants. The numbers are picked so the reservation is the whole
// point: two identical 60000-notional buys do not both fit inside a 100000
// balance, because the first one's funds stay held until it fills.
// =============================================================================

export const SCENARIO_ACCOUNT = 99_224_416n; // same account as rate_pnl_killswitch
export const SCENARIO_ASSET_TRADED = "AAPL"; // underlying
export const SCENARIO_ASSET_SETTLE = "USD"; // settlement asset reserved
export const SCENARIO_SEED_FUNDS = "100000"; // initial available USD
export const SCENARIO_ORDER_PRICE = "2000"; // limit price; also the lock price
export const SCENARIO_ORDER_QTY = "30"; // each buy is 30 * 2000 = 60000 USD

// Derived amounts used only in the narration below: one buy's notional
// (qty * price) and what stays available after the first buy's funds are held.
const ORDER_NOTIONAL = 60_000; // SCENARIO_ORDER_QTY * SCENARIO_ORDER_PRICE
const AVAILABLE_AFTER_BUY1 = 40_000; // SCENARIO_SEED_FUNDS - ORDER_NOTIONAL

// The CamelCase reject code SpotFunds returns when the order needs more
// settlement cash than the account has available.
export const INSUFFICIENT_FUNDS = "InsufficientFunds";

function main(): number {
  // Step 1 - build the engine. Limit-only SpotFunds plus OrderValidation; do
  // this once at platform start-up.
  const engine = buildEngine();

  // Step 2 - seed the account's available settlement cash. SpotFunds has no
  // initial-balance builder option; the balance is established through the
  // account-adjustment pipeline, exactly as a deposit would be.
  seedFunds(engine, SCENARIO_SEED_FUNDS);
  console.log(
    `seeded account with ${SCENARIO_SEED_FUNDS} ${SCENARIO_ASSET_SETTLE} available`,
  );

  // Step 3 - Buy #1: BUY 30 AAPL @ 2000 (60000 USD notional). It fits inside
  // the 100000 balance, so the pre-trade check accepts it. Committing the
  // reservation moves 60000 from available to held. We capture the
  // reservation's pre-trade lock first - the fill in Step 5 must carry it back
  // so SpotFunds settles this exact reservation.
  const buy1 = buildOrder();
  const placed1 = placeOrder(engine, buy1);
  if (placed1.lock === null) {
    throw new Error(
      `buy #1 unexpectedly rejected: ${describe(placed1.rejects)}`,
    );
  }
  console.log(
    `buy #1 accepted: held ${ORDER_NOTIONAL} ${SCENARIO_ASSET_SETTLE},` +
      ` ${AVAILABLE_AFTER_BUY1} ${SCENARIO_ASSET_SETTLE} now available`,
  );

  // Step 4 - Buy #2: an identical BUY 30 AAPL @ 2000. This is the teaching
  // point. Only 40000 USD is available now (60000 is held by Buy #1), but the
  // order needs 60000, so SpotFunds rejects it with InsufficientFunds. A
  // rejected order produces no reservation - there is nothing to commit.
  const buy2 = buildOrder();
  const placed2 = placeOrder(engine, buy2);
  if (placed2.lock !== null) {
    throw new Error("buy #2 unexpectedly accepted");
  }
  if (!containsCode(placed2.rejects, INSUFFICIENT_FUNDS)) {
    throw new Error(
      `buy #2 rejected for the wrong reason: ${describe(placed2.rejects)}`,
    );
  }
  console.log(
    `buy #2 rejected: ${describe(placed2.rejects)}` +
      " (held funds reduce what is available)",
  );

  // Step 5 - fill Buy #1 in full. The execution report carries the lock we
  // captured at commit time, so SpotFunds matches the fill to Buy #1's
  // reservation and settles the 60000 it was holding. No account block means
  // the settlement succeeded.
  const fill = buildFillReport(placed1.lock);
  const result = applyFill(engine, fill);
  if (result.accountBlocks.length > 0) {
    throw new Error("fill produced an unexpected account block");
  }
  console.log(
    `buy #1 filled: ${ORDER_NOTIONAL} ${SCENARIO_ASSET_SETTLE} reservation settled,` +
      " no account block",
  );

  // Step 6 - switch the policy to track-only at runtime. In TrackOnly the
  // insufficient-funds gate is dropped: reservations are still recorded, but a
  // lack of available cash no longer rejects the order. The same 60000-notional
  // buy that failed in Step 4 is now accepted.
  enableTrackOnly(engine);
  const buy3 = buildOrder();
  const placed3 = placeOrder(engine, buy3);
  if (placed3.lock === null) {
    throw new Error(
      `buy #3 unexpectedly rejected: ${describe(placed3.rejects)}`,
    );
  }
  console.log(
    `buy #3 accepted in track-only: ${ORDER_NOTIONAL} ${SCENARIO_ASSET_SETTLE}` +
      " reserved, available may go negative",
  );

  // Step 7 - arm the spot-funds account-currency P&L axis and force-set the
  // live accumulator. This is how an application can retune bounds and then
  // synchronize its own realized-P&L snapshot into the running policy without
  // rebuilding the engine.
  configureSpotFundsPnlAxis(engine);
  forceSpotFundsPnl(engine, "-120");
  console.log(
    `spot-funds pnl axis configured for ${SCENARIO_ASSET_SETTLE}; live pnl forced to -120`,
  );

  return 0;
}

// =============================================================================
// Shared helpers. main() and the smoke test both call these; each wraps one
// engine interaction so the flow above stays readable.
// =============================================================================

/**
 * Wire a limit-only engine with the SpotFunds policy.
 *
 * OrderValidation is registered first so the engine refuses malformed orders
 * before SpotFunds sees them. SpotFunds is not given `.marketData(...)`, so
 * market orders (no limit price) are rejected with UnsupportedOrderType - this
 * example only sends limit orders.
 */
export function buildEngine(): Engine {
  // The first builtin() advances the staged builder to the ready builder;
  // further builtin() calls register additional policies in place, so they are
  // statements rather than a fluent chain.
  const ready = Engine.builder().builtin(buildOrderValidation());
  ready.builtin(buildSpotFunds());
  return ready.build();
}

/**
 * Set the account's available settlement balance to an absolute amount. An
 * absolute adjustment overwrites the balance (unlike a relative delta), so it
 * reads as "set available USD to funds".
 *
 * The adjustment is a plain object literal: the account id passes as a bigint
 * and the balance amount as a decimal string wrapped only by the
 * delta/absolute tag the pipeline requires.
 */
export function seedFunds(engine: Engine, funds: string): void {
  const adjustment: AccountAdjustmentInit = {
    operation: { asset: SCENARIO_ASSET_SETTLE },
    amount: { balance: AdjustmentAmount.absolute(funds) },
  };

  const result = engine.applyAccountAdjustment(SCENARIO_ACCOUNT, [adjustment]);
  if (!result.ok) {
    throw new Error(`seed adjustment rejected: ${describe(result.rejects)}`);
  }
}

/**
 * Assemble a BUY limit order for the scenario instrument as a plain object.
 * Scalars cross as plain values (the account id as a bigint, the price as a
 * decimal string); a real strategy builds this from a signal and market data.
 */
export function buildOrder(): OrderInit {
  return {
    operation: {
      underlyingAsset: SCENARIO_ASSET_TRADED,
      settlementAsset: SCENARIO_ASSET_SETTLE,
      accountId: SCENARIO_ACCOUNT,
      side: "BUY",
      tradeAmount: TradeAmount.quantity(SCENARIO_ORDER_QTY),
      price: SCENARIO_ORDER_PRICE,
    },
  };
}

/** The outcome of placing one order: a committed lock or the rejects. */
export interface Placed {
  readonly lock: Lock | null;
  readonly rejects: Reject[];
}

/**
 * Run the pre-trade check and, on accept, commit the reservation. Returns the
 * committed reservation's pre-trade lock so the caller can later attach it to
 * the matching fill; on reject it returns a `null` lock and the rejects. The
 * lock MUST be read before commit(), because reservation.lock() throws once the
 * reservation is finalized.
 */
export function placeOrder(engine: Engine, order: OrderInit): Placed {
  const result = engine.executePreTrade(order);
  if (!result.ok) {
    // A rejected order reserves nothing; there is no lock and nothing to
    // commit.
    return { lock: null, rejects: result.rejects };
  }
  // Snapshot the lock the engine assigned to this reservation, then commit.
  // commit() moves the reserved settlement funds from available to held;
  // rollback() would release them instead.
  const reservation = result.reservation;
  if (reservation === undefined) {
    throw new Error("accepted result is missing its reservation");
  }
  const lock = reservation.lock();
  reservation.commit();
  return { lock, rejects: [] };
}

/**
 * Assemble a full, final execution report for a buy order as a plain object.
 *
 * The pre-trade lock captured when the reservation was committed is attached to
 * the fill. Carrying that lock is what ties the fill back to the reservation:
 * SpotFunds reads the lock to find which held funds to settle. Reusing the
 * stored Lock object is more faithful than rebuilding the lock - it is exactly
 * what the engine produced - but an equivalent lock can be reconstructed with
 * `new Lock([[DEFAULT_POLICY_GROUP_ID(), price]])` when the caller did not keep
 * the reservation's lock (see ../spot_table). The trade price and quantities
 * cross as decimal strings.
 */
export function buildFillReport(lock: Lock): ExecutionReportInit {
  return {
    operation: {
      underlyingAsset: SCENARIO_ASSET_TRADED,
      settlementAsset: SCENARIO_ASSET_SETTLE,
      accountId: SCENARIO_ACCOUNT,
      side: "BUY",
    },
    // Combined-mode impact: the fee is embedded in pnl, so both are zero for a
    // plain settlement. See the SpotFunds wiki page for the "separate" fee
    // convention.
    financialImpact: { pnl: "0", fee: "0" },
    fill: {
      lock,
      lastTrade: { price: SCENARIO_ORDER_PRICE, quantity: SCENARIO_ORDER_QTY },
      // A full fill of a 30-lot order leaves nothing outstanding.
      leavesQuantity: "0",
      isFinal: true,
    },
  };
}

/**
 * Feed a completed execution report to the engine. The returned
 * `accountBlocks` is empty when settlement succeeds; a non-empty list would
 * mean a policy permanently blocked the account.
 */
export function applyFill(
  engine: Engine,
  report: ExecutionReportInit,
): PostTradeResult {
  return engine.applyExecutionReport(report);
}

/** Switch the SpotFunds policy to global track-only mode at runtime. */
export function enableTrackOnly(engine: Engine): void {
  engine.configure().spotFunds(SpotFundsBuilder.NAME, {
    globalLimitMode: SpotFundsLimitMode.TrackOnly,
  });
}

/** Retune the SpotFunds account-currency P&L axis for the example account. */
export function configureSpotFundsPnlAxis(engine: Engine): void {
  engine.configure().spotFundsPnlBoundsKillswitch(SpotFundsBuilder.NAME, {
    accountBarriers: [
      new SpotFundsPnlBoundsAccountBarrierUpdate(
        SCENARIO_ACCOUNT,
        SCENARIO_ASSET_SETTLE,
        "-250",
        "250",
      ),
    ],
  });
}

/** Force-set the SpotFunds live accumulated P&L for the example account. */
export function forceSpotFundsPnl(engine: Engine, pnl: string): void {
  engine.configure().setSpotFundsAccountPnl(SpotFundsBuilder.NAME, {
    account: SCENARIO_ACCOUNT,
    accountCurrency: SCENARIO_ASSET_SETTLE,
    pnl,
  });
}

/** Report whether the rejects include the given business code. */
export function containsCode(rejects: Reject[], want: string): boolean {
  return rejects.some((reject) => reject.code === want);
}

/** Render rejects as "reason (details)" pairs for a one-line message. */
export function describe(rejects: Reject[]): string {
  if (rejects.length === 0) {
    return "no rejects";
  }
  return rejects
    .map((reject) => `${reject.reason} (${reject.details})`)
    .join("; ");
}

// Run main() only when executed directly, so the test module can import the
// helpers above without launching the scenario.
if (import.meta.url === `file://${process.argv[1]}`) {
  process.exit(main());
}
