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

// Example rate_pnl_killswitch demonstrates how an algorithmic trading desk can
// wrap OpenPit's RateLimit and PnlBoundsKillswitch policies around a TypeScript
// strategy so that a runaway strategy is halted before it floods the venue with
// orders or burns through the loss budget.
//
// What is illustrated:
//
//   - building an engine with two killswitch policies side-by-side
//   - feeding the engine via a single Event stream (orders + fills)
//   - separating venue/strategy side-effects behind a Reactor interface
//   - aggregating accepted/rejected counts, pre-trade latency, and cumulative
//     P&L over the run
//
// Audience: an algo trader who wants an independent supervisor that prevents
// the strategy from "going crazy".
//
// What you typically change to adapt this example to your own application:
//
//   1. Engine policies and limits - see buildEngine() below.
//   2. The order/report stream - scenarioStream() in main() is a one-shot
//      replay; real systems plug in an async source driven by venue and
//      strategy events.
//   3. The reactor implementation - replace LoggingReactor with code that
//      actually submits orders to the venue, updates your strategy book, and
//      halts the strategy when accountBlocks is non-empty.

import { Engine } from "@openpit/engine";
import { TradeAmount } from "@openpit/engine/param";
import {
  type ExecutionReportInit,
  type OrderInit,
} from "@openpit/engine/model";
import { type PostTradeResult } from "@openpit/engine/pretrade";
import {
  buildOrderValidation,
  buildPnlBoundsKillswitch,
  buildRateLimit,
  PnlBoundsBrokerBarrier,
  RateLimit,
  RateLimitBrokerBarrier,
} from "@openpit/engine/pretrade/policies";
import { type Reject } from "@openpit/engine/reject";

// =============================================================================
// Section 1 - public extension points.
// The two event types, the Reactor interface, and the Stats type are the only
// things application code interacts with. run() below is policy-agnostic; it
// only knows these types.
// =============================================================================

/**
 * A strategy-emitted order intent waiting for pre-trade evaluation. The order
 * is a plain `OrderInit` literal - the engine accepts it directly, with no
 * wrapper objects to construct.
 */
interface OrderEvent {
  readonly kind: "order";
  readonly order: OrderInit;
}

/**
 * A venue-emitted execution report. `realizedPnl` mirrors the value stored in
 * the report's `financialImpact.pnl` so the example can track the running
 * balance outside the engine - production code would read it from its strategy
 * book. The decimal is carried as a string, the lossless cross-boundary form.
 */
interface ReportEvent {
  readonly kind: "report";
  readonly report: ExecutionReportInit;
  readonly realizedPnl: string;
}

type Event = OrderEvent | ReportEvent;

/** Engine-verdict consumer. Plug your venue client and strategy book here. */
interface Reactor {
  /** Pre-trade has reserved and committed the order. Send it to the venue. */
  onAccepted(order: OrderInit): void;

  /**
   * A policy refused the order. Inspect `rejects[i].code` to choose between
   * retry / throttle / escalate.
   */
  onRejected(order: OrderInit, rejects: Reject[]): void;

  /**
   * The engine consumed a venue execution report. When `result.accountBlocks`
   * is non-empty the strategy must stop sending orders for this account until
   * operators clear the state.
   */
  onReport(report: ExecutionReportInit, result: PostTradeResult): void;
}

/** Timing and trading outcomes over a run. */
interface Stats {
  accepted: number;
  rejected: number;
  preTradeCalls: number;
  reports: number;
  killSwitch: boolean;
  killSwitchOnTrade: number; // 1-based index of the tripping report, 0 if never
  pnl: bigint; // cumulative realized P&L in milli-units (see PNL_SCALE)
  totalPreTradeNs: bigint;
  minPreTradeNs: bigint;
  maxPreTradeNs: bigint;
}

// The realized P&L decimals in this scenario have at most one fractional digit
// (-0.5, -460), so tracking the running balance in tenths as an exact integer
// keeps the summation lossless without pulling in a decimal library.
const PNL_SCALE = 10n;

function newStats(): Stats {
  return {
    accepted: 0,
    rejected: 0,
    preTradeCalls: 0,
    reports: 0,
    killSwitch: false,
    killSwitchOnTrade: 0,
    pnl: 0n,
    totalPreTradeNs: 0n,
    minPreTradeNs: 0n,
    maxPreTradeNs: 0n,
  };
}

function avgPreTradeNs(stats: Stats): bigint {
  if (stats.preTradeCalls === 0) {
    return 0n;
  }
  return stats.totalPreTradeNs / BigInt(stats.preTradeCalls);
}

// =============================================================================
// Section 2 - engine wiring.
// The two killswitch policies and the engine builder. Tune the limits to your
// risk tolerance.
// =============================================================================

/** Killswitch parameters; the call site reads like a risk-policy declaration. */
interface Limits {
  readonly settlementAsset: string;
  readonly pnlLowerBound: string; // loss floor as a signed decimal, e.g. "-500"
  readonly pnlUpperBound: string; // profit-taking ceiling, e.g. "500"
  readonly maxOrdersBurst: number; // orders allowed inside the rate window
  readonly rateWindowMs: number; // length of the rate-limit window
}

/**
 * Wire the engine with the two killswitch policies plus order validation. The
 * combination answers a single question: "is my strategy trading too fast or
 * losing too much?".
 */
function buildEngine(limits: Limits): Engine {
  // The WASM engine is single-threaded and always uses no-op locking.
  // OrderValidation must be present so the engine refuses malformed orders
  // before the killswitch policies see them. The first builtin() advances the
  // staged builder to the ready builder; the rest register in place.
  const ready = Engine.builder().builtin(buildOrderValidation());

  // PnL bounds halt the account permanently when realized P&L crosses either
  // edge of the corridor. Both bounds are optional - this example configures
  // both for completeness.
  ready.builtin(
    buildPnlBoundsKillswitch().brokerBarriers([
      new PnlBoundsBrokerBarrier(
        limits.settlementAsset,
        limits.pnlLowerBound,
        limits.pnlUpperBound,
      ),
    ]),
  );

  // Rate limit catches a strategy stuck in a tight loop. The example uses the
  // broker (global) axis; see the Policies wiki page for per-asset and
  // per-account axes.
  ready.builtin(
    buildRateLimit().brokerBarrier(
      new RateLimitBrokerBarrier(
        new RateLimit(limits.maxOrdersBurst, limits.rateWindowMs),
      ),
    ),
  );

  return ready.build();
}

// =============================================================================
// Section 3 - the engine loop.
// run() consumes the event stream, calls the engine, and notifies the reactor.
// This function is policy-agnostic - reuse it as-is in your code.
// =============================================================================

/**
 * Drive the engine until `stream` is exhausted and return aggregate stats. The
 * engine is owned by the caller. Exceptions raised here come from
 * infrastructure failures, not business rejects (those go to
 * `reactor.onRejected`).
 */
function run(engine: Engine, stream: Iterable<Event>, reactor: Reactor): Stats {
  const stats = newStats();
  for (const event of stream) {
    if (event.kind === "order") {
      runPreTrade(engine, event.order, stats, reactor);
    } else {
      runReport(engine, event, stats, reactor);
    }
  }
  return stats;
}

function runPreTrade(
  engine: Engine,
  order: OrderInit,
  stats: Stats,
  reactor: Reactor,
): void {
  const start = process.hrtime.bigint();
  const result = engine.executePreTrade(order);
  const elapsed = process.hrtime.bigint() - start;

  stats.preTradeCalls += 1;
  stats.totalPreTradeNs += elapsed;
  if (stats.preTradeCalls === 1 || elapsed < stats.minPreTradeNs) {
    stats.minPreTradeNs = elapsed;
  }
  if (elapsed > stats.maxPreTradeNs) {
    stats.maxPreTradeNs = elapsed;
  }

  if (!result.ok) {
    stats.rejected += 1;
    reactor.onRejected(order, result.rejects);
    return;
  }

  // On accept, persist the reservation. commit() finalizes the reserved state;
  // call rollback() instead to release it if you decide not to submit the
  // order to the venue.
  const reservation = result.reservation;
  if (reservation === undefined) {
    throw new Error("accepted result is missing its reservation");
  }
  reservation.commit();
  stats.accepted += 1;
  reactor.onAccepted(order);
}

function runReport(
  engine: Engine,
  event: ReportEvent,
  stats: Stats,
  reactor: Reactor,
): void {
  const result = engine.applyExecutionReport(event.report);
  stats.reports += 1;
  stats.pnl += scaledPnl(event.realizedPnl);
  if (result.accountBlocks.length > 0 && !stats.killSwitch) {
    stats.killSwitch = true;
    stats.killSwitchOnTrade = stats.reports;
  }
  reactor.onReport(event.report, result);
}

/** Render a signed one-decimal P&L string as an exact integer in tenths. */
function scaledPnl(value: string): bigint {
  const negative = value.startsWith("-");
  const digits = negative ? value.slice(1) : value;
  const [whole = "0", frac = ""] = digits.split(".");
  if (frac.length > 1) {
    // The scenario only emits at most one fractional digit; guard the contract.
    throw new Error(`realizedPnl ${value} exceeds one fractional digit`);
  }
  const tenths = BigInt(whole) * PNL_SCALE + BigInt((frac + "0").slice(0, 1));
  return negative ? -tenths : tenths;
}

// =============================================================================
// Section 4 - the scenario.
// A scripted feed that exercises the kill-switch policies. In your own
// application this is the place you delete entirely - your real strategy
// produces events.
// =============================================================================

// The burst overshoots the rate-limit ceiling by a few orders so the policy
// rejects the tail of the burst. The accepted orders then produce a stream of
// small-loss reports, and the final report contributes a large loss that pushes
// cumulative P&L past the lower bound and trips the kill switch on the last
// trade.
export const SCENARIO_ATTEMPTS = 105;
export const SCENARIO_MAX_ORDERS_BURST = 100;
export const SCENARIO_ACCEPTED_REPORTS = 100;
export const SCENARIO_ACCOUNT = 99_224_416n;

// 99 * (-0.5) + (-460) = -509.5 < -500 - the kill switch fires on the final
// report; every earlier report keeps cumulative P&L well inside the corridor
// (-49.5 at worst).
export const SCENARIO_REPORT_PNL = "-0.5";
export const SCENARIO_FINAL_REPORT_PNL = "-460";
export const SCENARIO_LOWER_BOUND = "-500";
export const SCENARIO_UPPER_BOUND = "500";
export const SCENARIO_RATE_WINDOW_MS = 10_000;
export const SCENARIO_ORDER_PRICE = "185";
export const SCENARIO_ORDER_QTY = "100";
export const SCENARIO_ASSET_TRADED = "AAPL";
export const SCENARIO_ASSET_SETTLE = "USD";

/**
 * Build a buy-AAPL order intent as a plain object. Scalars cross as plain values
 * (the account id as a bigint, the price as a decimal string); a real strategy
 * assembles this from a signal and current market data.
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

/**
 * Build a combined-mode execution report as a plain object. "Combined" means the
 * fee is embedded in pnl, so the fee field is set to zero; see the Policies wiki
 * page for the alternative "separate" convention.
 */
export function buildReport(pnl: string): ExecutionReportInit {
  return {
    operation: {
      underlyingAsset: SCENARIO_ASSET_TRADED,
      settlementAsset: SCENARIO_ASSET_SETTLE,
      accountId: SCENARIO_ACCOUNT,
      side: "BUY",
    },
    financialImpact: { pnl, fee: "0" },
  };
}

/**
 * Scripted feed: three counters walked in order - order attempts, then
 * small-loss reports, then one kill-switch report. Each event carries its own
 * freshly built Order / ExecutionReport, mirroring a live feed where every
 * order and report is a distinct event. Replace this generator with a source
 * that selects over your strategy and venue feeds.
 */
export function* scenarioStream(): Generator<Event> {
  for (let i = 0; i < SCENARIO_ATTEMPTS; i += 1) {
    yield { kind: "order", order: buildOrder() };
  }
  for (let i = 0; i < SCENARIO_ACCEPTED_REPORTS - 1; i += 1) {
    yield {
      kind: "report",
      report: buildReport(SCENARIO_REPORT_PNL),
      realizedPnl: SCENARIO_REPORT_PNL,
    };
  }
  yield {
    kind: "report",
    report: buildReport(SCENARIO_FINAL_REPORT_PNL),
    realizedPnl: SCENARIO_FINAL_REPORT_PNL,
  };
}

/** Assemble the limits used by both main() and the smoke test. */
export function scenarioLimits(): Limits {
  return {
    settlementAsset: SCENARIO_ASSET_SETTLE,
    pnlLowerBound: SCENARIO_LOWER_BOUND,
    pnlUpperBound: SCENARIO_UPPER_BOUND,
    maxOrdersBurst: SCENARIO_MAX_ORDERS_BURST,
    rateWindowMs: SCENARIO_RATE_WINDOW_MS,
  };
}

export { buildEngine, run };
export type { Event, Limits, Reactor, Stats };

// =============================================================================
// Section 5 - the reactor.
// Plug your venue client and strategy book here.
// =============================================================================

/**
 * Prints rejects and kill-switch events to stdout. Production code routes these
 * to your monitoring channel and to a strategy-halt signal.
 */
class LoggingReactor implements Reactor {
  private rejectsPrinted = 0;

  public constructor(private readonly rejectCap = 4) {}

  public onAccepted(_order: OrderInit): void {
    // In production: venue.sendOrder(order).
  }

  public onRejected(_order: OrderInit, rejects: Reject[]): void {
    // Cap noisy outputs in case a real run produces a long burst of
    // rate-limit rejects.
    if (this.rejectsPrinted >= this.rejectCap) {
      return;
    }
    for (const reject of rejects) {
      console.log(
        `rejected by ${reject.policy} [${reject.code}]: ${reject.reason} (${reject.details})`,
      );
      this.rejectsPrinted += 1;
      if (this.rejectsPrinted >= this.rejectCap) {
        console.log("... further rejects suppressed");
        return;
      }
    }
  }

  public onReport(_report: ExecutionReportInit, result: PostTradeResult): void {
    if (result.accountBlocks.length > 0) {
      console.log("kill switch triggered - halt new orders until cleared");
    }
  }
}

// =============================================================================
// Section 6 - main().
// The application entry point. Read top-to-bottom for the integration flow.
// =============================================================================

function formatNs(valueNs: bigint): string {
  const ns = Number(valueNs);
  if (ns >= 1_000_000_000) {
    return `${(ns / 1_000_000_000).toFixed(3)}s`;
  }
  if (ns >= 1_000_000) {
    return `${(ns / 1_000_000).toFixed(3)}ms`;
  }
  if (ns >= 1_000) {
    return `${(ns / 1_000).toFixed(3)}us`;
  }
  return `${ns}ns`;
}

/** Render an exact-tenths P&L back as a two-decimal signed string. */
function formatPnl(tenths: bigint): string {
  const negative = tenths < 0n;
  const magnitude = negative ? -tenths : tenths;
  const whole = magnitude / PNL_SCALE;
  const frac = magnitude % PNL_SCALE;
  const sign = negative ? "-" : "";
  return `${sign}${whole}.${frac.toString().padStart(1, "0")}0`;
}

function main(): number {
  // Step 1 - declare the risk limits.
  const limits = scenarioLimits();

  // Step 2 - build the engine. Do this once at platform start-up.
  const engine = buildEngine(limits);

  // Step 3 - assemble the event stream. In production this is your strategy +
  // venue listener; here it is a generator driven by the scenario constants
  // above.
  const stream = scenarioStream();

  // Step 4 - run the loop. Replace LoggingReactor with your venue client.
  const stats = run(engine, stream, new LoggingReactor(4));

  // Step 5 - report the outcome. In production you would push these to your
  // metrics backend.
  console.log();
  console.log("--- run summary ---");
  console.log(
    `pnl result   : ${formatPnl(stats.pnl)} ${limits.settlementAsset}`,
  );
  console.log(`total trades : ${stats.reports}`);
  console.log(`pre-trade avg: ${formatNs(avgPreTradeNs(stats))}`);
  console.log(`pre-trade min: ${formatNs(stats.minPreTradeNs)}`);
  console.log(`pre-trade max: ${formatNs(stats.maxPreTradeNs)}`);
  console.log(`pre-trade tot: ${formatNs(stats.totalPreTradeNs)}`);
  console.log(`accepted     : ${stats.accepted}`);
  console.log(`rejected     : ${stats.rejected}`);
  if (stats.killSwitch) {
    console.log(
      `kill switch  : TRIPPED on trade ${stats.killSwitchOnTrade} of ${stats.reports}`,
    );
  } else {
    console.log("kill switch  : not triggered");
  }
  return 0;
}

// Run main() only when executed directly, so the test module can import the
// helpers above without launching the scenario.
if (import.meta.url === `file://${process.argv[1]}`) {
  process.exit(main());
}
