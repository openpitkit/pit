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

// Sequential table runner for the spot_table example.
//
// Executes a parsed scenario against a single-threaded engine, operation by
// operation in row order. TICK rows are replayed live at their row position;
// the run stops at the first verdict mismatch and returns a partial report.

import { Engine } from "@openpit/engine";
import {
  type AccountGroupId,
  type AccountId,
} from "@openpit/engine/param";
import { type ExecuteResult } from "@openpit/engine/pretrade";
import { buildSpotFunds } from "@openpit/engine/pretrade/policies";
import { QuoteTtl } from "@openpit/engine/marketdata";
import { type Reject } from "@openpit/engine/reject";

import * as builder from "./builder.ts";
import { MarketFeed } from "./marketFeed.ts";
import type { Frontmatter, Row } from "./table.ts";

// Mode names the execution strategy of a runner. This example only ships the
// single-threaded sequential strategy.
export const MODE_SYNC = "sync";

// ---------------------------------------------------------------------------
// Reports and statistics
// ---------------------------------------------------------------------------

/** Latency samples for one operation kind. */
export class LatencyStats {
  public count = 0;
  public totalNs = 0n;
  public minNs = 0n;
  public maxNs = 0n;

  /** Fold one latency sample into the running statistics. */
  public observe(ns: bigint): void {
    this.count += 1;
    this.totalNs += ns;
    if (this.count === 1 || ns < this.minNs) {
      this.minNs = ns;
    }
    if (ns > this.maxNs) {
      this.maxNs = ns;
    }
  }

  /** Return the mean latency in nanoseconds. */
  public avgNs(): bigint {
    if (this.count === 0) {
      return 0n;
    }
    return this.totalNs / BigInt(this.count);
  }

  /** Fold another sample set into self, for aggregating repeat iterations. */
  public merge(other: LatencyStats): void {
    if (other.count === 0) {
      return;
    }
    if (this.count === 0 || other.minNs < this.minNs) {
      this.minNs = other.minNs;
    }
    if (other.maxNs > this.maxNs) {
      this.maxNs = other.maxNs;
    }
    this.count += other.count;
    this.totalNs += other.totalNs;
  }
}

/** The first mismatch or runtime error seen during a run. */
export interface Failure {
  readonly row: Row;
  readonly message: string;
}

/** The per-run outcome of executing a table. */
export class Report {
  public readonly accounts = new Map<string, number>();
  public total = 0; // executable rows (SEED/GROUP/ORDER/FILL; excludes TICK)
  public wallClockNs = 0n;
  public readonly order = new LatencyStats();
  public readonly fill = new LatencyStats();
  public firstFail: Failure | null = null;

  public constructor(public readonly mode: string) {}

  /** Return the number of distinct accounts touched. */
  public accountsCount(): number {
    return this.accounts.size;
  }

  public countAccount(label: string): void {
    this.total += 1;
    this.accounts.set(label, (this.accounts.get(label) ?? 0) + 1);
  }
}

/** Accumulates one engine's statistics across repeat iterations. */
export class EngineAggregate {
  public mode = MODE_SYNC;
  public accounts = 0;
  public ops = 0;
  public readonly order = new LatencyStats();
  public readonly fill = new LatencyStats();

  /** Fold one iteration's report into the aggregate. */
  public add(report: Report): void {
    this.mode = report.mode;
    this.accounts = report.accountsCount();
    this.ops += report.total;
    this.order.merge(report.order);
    this.fill.merge(report.fill);
  }
}

// ---------------------------------------------------------------------------
// Reject-code resolution
// ---------------------------------------------------------------------------

// Case-insensitive set of the CamelCase reject codes the runner recognizes.
const RECOGNIZED = new Set(
  [
    "MissingRequiredField",
    "InvalidFieldFormat",
    "InvalidFieldValue",
    "UnsupportedOrderType",
    "InsufficientFunds",
    "InsufficientMargin",
    "InsufficientPosition",
    "MarkPriceUnavailable",
    "OrderValueCalculationFailed",
    "AccountAdjustmentBoundsExceeded",
  ].map((c) => c.toLowerCase()),
);

const CANONICAL = new Map(
  [
    "MissingRequiredField",
    "InvalidFieldFormat",
    "InvalidFieldValue",
    "UnsupportedOrderType",
    "InsufficientFunds",
    "InsufficientMargin",
    "InsufficientPosition",
    "MarkPriceUnavailable",
    "OrderValueCalculationFailed",
    "AccountAdjustmentBoundsExceeded",
  ].map((c) => [c.toLowerCase(), c] as const),
);

/** Resolve a table reject label to a recognized code, case-insensitively. */
export function resolveCode(name: string): string | null {
  const key = name.trim().toLowerCase();
  if (!RECOGNIZED.has(key)) {
    return null;
  }
  return CANONICAL.get(key) ?? null;
}

/** Report whether any reject carries the wanted code. */
export function containsCode(rejects: Reject[], want: string): boolean {
  return rejects.some((reject) => reject.code === want);
}

/** Render the rejects as a sorted, comma-separated list of code strings. */
export function describeRejects(rejects: Reject[]): string {
  return rejects
    .map((reject) => reject.code)
    .sort()
    .join(",");
}

// ---------------------------------------------------------------------------
// Engine build
// ---------------------------------------------------------------------------

/**
 * Build the sequential engine with the spot funds policy
 * reading a market-data service. The returned feed owns the instrument
 * registry; its instruments are registered up front so live TICK pushes
 * resolve.
 */
export function buildSpotEngineSync(
  fm: Frontmatter,
  rows: Row[],
): { engine: Engine; feed: MarketFeed } {
  const builder = Engine.builder();
  const service = builder.marketData(QuoteTtl.infinite()).build();
  const feed = new MarketFeed(service);
  feed.registerInstruments(rows);
  const engine = builder
    .builtin(buildSpotFunds().marketData(service, fm.slippageBps, "Mark", []))
    .build();
  return { engine, feed };
}

// ---------------------------------------------------------------------------
// Group membership
// ---------------------------------------------------------------------------

/**
 * Every GROUP row aggregated into the per-group account sets. Row order is
 * preserved so registration is deterministic; each GROUP row is retained both
 * for diagnostics and so the report's account counts are stable.
 */
class GroupMembership {
  public readonly order: string[] = [];
  public readonly members = new Map<string, AccountId[]>();
  public readonly rows: Row[] = [];

  /** Return the first GROUP row that named `label`, to anchor a failure. */
  public firstRow(label: string): Row | null {
    return this.rows.find((row) => row.group === label) ?? null;
  }

  /** Record every GROUP row toward the report's totals. */
  public countInReport(report: Report): void {
    for (const row of this.rows) {
      report.countAccount(row.account);
    }
  }
}

function collectGroups(rows: Row[]): { groups: GroupMembership | null; fail: Failure | null } {
  const groups = new GroupMembership();
  for (const row of rows) {
    if (row.action !== "GROUP") {
      continue;
    }
    let acc: AccountId;
    try {
      acc = builder.accountId(row.account);
    } catch (exc) {
      return { groups: null, fail: { row, message: (exc as Error).message } };
    }
    if (!groups.members.has(row.group)) {
      groups.order.push(row.group);
      groups.members.set(row.group, []);
    }
    groups.members.get(row.group)!.push(acc);
    groups.rows.push(row);
  }
  return { groups, fail: null };
}

function registerGroupsSync(
  engine: Engine,
  groups: GroupMembership,
  report: Report,
): Failure | null {
  groups.countInReport(report);
  const accountsView = engine.accounts();
  for (const label of groups.order) {
    let groupId: AccountGroupId;
    try {
      groupId = builder.accountGroupId(label);
    } catch (exc) {
      return failureFor(groups, label, (exc as Error).message);
    }
    try {
      accountsView.registerGroup(groups.members.get(label)!, groupId);
    } catch (exc) {
      // engine errors become verdict failures, not raises
      return failureFor(groups, label, `register group: ${(exc as Error).message}`);
    }
  }
  return null;
}

function failureFor(groups: GroupMembership, label: string, message: string): Failure {
  const row = groups.firstRow(label);
  if (row === null) {
    throw new Error(`internal: no GROUP row for label ${label}`);
  }
  return { row, message };
}

// ---------------------------------------------------------------------------
// Run loop
// ---------------------------------------------------------------------------

/**
 * Execute the table sequentially on a single-threaded engine.
 *
 * TICK rows are replayed live at their row position. Stops at the first verdict
 * mismatch and returns a partial report. `deadlineNs` is an optional
 * process.hrtime budget; when reached the loop breaks early.
 */
export function runSync(fm: Frontmatter, rows: Row[], deadlineNs: bigint | null): Report {
  const { engine, feed } = buildSpotEngineSync(fm, rows);
  const report = new Report(MODE_SYNC);

  const collected = collectGroups(rows);
  if (collected.fail !== null || collected.groups === null) {
    report.firstFail = collected.fail;
    return report;
  }
  const regFail = registerGroupsSync(engine, collected.groups, report);
  if (regFail !== null) {
    report.firstFail = regFail;
    return report;
  }

  const start = process.hrtime.bigint();
  for (const row of rows) {
    if (deadlineNs !== null && process.hrtime.bigint() >= deadlineNs) {
      break;
    }
    if (row.action === "GROUP") {
      // Registered up front in registerGroupsSync.
      continue;
    }
    if (row.action === "TICK") {
      const fail = runSyncTick(feed, row);
      if (fail !== null) {
        report.firstFail = fail;
        break;
      }
      continue;
    }
    // Validate the account label up front (non-empty, hashable); the engine
    // then hashes the same label string itself wherever an order, fill, or
    // adjustment carries it, so no AccountId wrapper is minted per row.
    try {
      builder.accountId(row.account);
    } catch (exc) {
      report.firstFail = { row, message: (exc as Error).message };
      break;
    }
    report.countAccount(row.account);

    if (row.action === "SEED") {
      const fail = runSyncSeed(engine, row.account, row);
      if (fail !== null) {
        report.firstFail = fail;
        break;
      }
    } else if (row.action === "ORDER") {
      const { fail, dur } = runSyncOrder(engine, row.account, row);
      report.order.observe(dur);
      if (fail !== null) {
        report.firstFail = fail;
        break;
      }
    } else if (row.action === "FILL") {
      const { fail, dur } = runSyncFill(engine, row.account, row, feed);
      report.fill.observe(dur);
      if (fail !== null) {
        report.firstFail = fail;
        break;
      }
    }
  }

  report.wallClockNs = process.hrtime.bigint() - start;
  return report;
}

// ---------------------------------------------------------------------------
// Per-row execution
// ---------------------------------------------------------------------------

function runSyncTick(feed: MarketFeed, row: Row): Failure | null {
  try {
    pushTick(feed, row);
  } catch (exc) {
    return { row, message: (exc as Error).message };
  }
  return null;
}

function pushTick(feed: MarketFeed, row: Row): void {
  if (row.account === "" && row.group === "") {
    feed.push(row.instrument, row.price);
    return;
  }
  const accounts: AccountId[] = [];
  if (row.account) {
    accounts.push(builder.accountId(row.account));
  }
  const groups: AccountGroupId[] = [];
  if (row.group) {
    groups.push(builder.accountGroupId(row.group));
  }
  feed.pushFor(row.instrument, row.price, accounts, groups);
}

function runSyncSeed(engine: Engine, account: string, row: Row): Failure | null {
  let adj;
  try {
    adj = builder.buildSeedAdjustment(row);
  } catch (exc) {
    return { row, message: (exc as Error).message };
  }
  let result;
  try {
    result = engine.applyAccountAdjustment(account, [adj]);
  } catch (exc) {
    // engine errors become verdict failures, not raises
    return { row, message: `engine: ${(exc as Error).message}` };
  }
  return checkSeedVerdict(row, !result.ok);
}

function runSyncOrder(
  engine: Engine,
  account: string,
  row: Row,
): { fail: Failure | null; dur: bigint } {
  let order;
  try {
    order = builder.buildOrder(row, account);
  } catch (exc) {
    return { fail: { row, message: (exc as Error).message }, dur: 0n };
  }
  const start = process.hrtime.bigint();
  let result: ExecuteResult;
  try {
    result = engine.executePreTrade(order);
  } catch (exc) {
    // engine errors become verdict failures, not raises
    return {
      fail: { row, message: `engine: ${(exc as Error).message}` },
      dur: process.hrtime.bigint() - start,
    };
  }
  const dur = process.hrtime.bigint() - start;
  const fail = checkOrderVerdict(row, result);
  const reservation = result.reservation;
  if (reservation !== undefined) {
    if (fail === null) {
      reservation.commit();
    } else {
      reservation.rollback();
    }
  }
  return { fail, dur };
}

function runSyncFill(
  engine: Engine,
  account: string,
  row: Row,
  feed: MarketFeed,
): { fail: Failure | null; dur: bigint } {
  let report;
  try {
    report = builder.buildFillReport(row, account, feed);
  } catch (exc) {
    return { fail: { row, message: (exc as Error).message }, dur: 0n };
  }
  const start = process.hrtime.bigint();
  let result;
  try {
    result = engine.applyExecutionReport(report);
  } catch (exc) {
    // engine errors become verdict failures, not raises
    return {
      fail: { row, message: `engine: ${(exc as Error).message}` },
      dur: process.hrtime.bigint() - start,
    };
  }
  const dur = process.hrtime.bigint() - start;
  return { fail: checkFillVerdict(row, result.accountBlocks.length > 0), dur };
}

// ---------------------------------------------------------------------------
// Verdict checks
// ---------------------------------------------------------------------------

function checkOrderVerdict(row: Row, result: ExecuteResult): Failure | null {
  if (row.expect === "ACCEPT") {
    if (!result.ok) {
      return {
        row,
        message: `expected ACCEPT, got REJECT(${describeRejects(result.rejects)})`,
      };
    }
  } else if (row.expect === "REJECT") {
    if (result.ok) {
      return { row, message: "expected REJECT, got ACCEPT" };
    }
    if (row.reject) {
      const want = resolveCode(row.reject);
      if (want === null) {
        return { row, message: `unknown reject code ${JSON.stringify(row.reject)} in table` };
      }
      if (!containsCode(result.rejects, want)) {
        return {
          row,
          message:
            `expected REJECT(${row.reject}),` +
            ` got REJECT(${describeRejects(result.rejects)})`,
        };
      }
    }
  } else {
    return { row, message: `ORDER row must use ACCEPT/REJECT, got ${row.expect}` };
  }
  return null;
}

function checkSeedVerdict(row: Row, rejected: boolean): Failure | null {
  if (row.expect === "OK") {
    if (rejected) {
      return { row, message: "expected OK, SEED rejected" };
    }
  } else if (row.expect === "REJECT") {
    if (!rejected) {
      return { row, message: "expected REJECT, SEED accepted" };
    }
  } else {
    return seedFillVerdictError(row);
  }
  return null;
}

function checkFillVerdict(row: Row, blocked: boolean): Failure | null {
  if (row.expect === "OK") {
    if (blocked) {
      return { row, message: "expected OK, got account block" };
    }
  } else if (row.expect === "REJECT") {
    if (!blocked) {
      return { row, message: "expected REJECT, FILL produced no block" };
    }
  } else {
    return seedFillVerdictError(row);
  }
  return null;
}

function seedFillVerdictError(row: Row): Failure {
  if (row.expect === "ACCEPT") {
    return {
      row,
      message: `${row.action} row cannot use ACCEPT (ORDER-only); use OK or REJECT`,
    };
  }
  return { row, message: `${row.action} row must use OK/REJECT, got ${row.expect}` };
}
