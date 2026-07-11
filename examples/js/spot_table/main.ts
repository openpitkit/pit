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

// Example spot_table.
//
// Runs a tabular spot-policy scenario against a single sequential engine
// - operation by operation, in the row order written in the table - and prints
// a summary report with operation counts, total wall-clock time, and
// order/report latency statistics. With --min-duration d it repeats the
// scenario until at least d of wall-clock time has elapsed (a repeat run),
// printing a periodic progress block with the engine's running order/report
// latency, then a final aggregate summary. The scenario tables live under
// examples/tables/spot/.

import { existsSync } from "node:fs";
import { basename, dirname, isAbsolute, resolve } from "node:path";
import { argv, cwd, exit, stderr } from "node:process";
import { fileURLToPath } from "node:url";

import {
  EngineAggregate,
  type Failure,
  LatencyStats,
  MODE_SYNC,
  Report,
  runSync,
} from "./runner.ts";
import { printPlatform } from "./platformInfo.ts";
import { parseFile, type Table } from "./table.ts";

const HERE = dirname(fileURLToPath(import.meta.url));

// defaultTimeout bounds a single pass of the scenario through the engine.
const DEFAULT_TIMEOUT = "30s";

// How often the repeat run prints a progress block, in seconds.
const REPEAT_LOG_INTERVAL_S = 10.0;

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

interface Args {
  table: string;
  timeout: string;
  minDuration: string;
}

function parseArgs(args: string[]): Args {
  const parsed: Args = { table: "", timeout: DEFAULT_TIMEOUT, minDuration: "0" };
  for (let i = 0; i < args.length; i += 1) {
    const arg = args[i]!;
    if (arg === "--table") {
      parsed.table = args[(i += 1)] ?? "";
    } else if (arg.startsWith("--table=")) {
      parsed.table = arg.slice("--table=".length);
    } else if (arg === "--timeout") {
      parsed.timeout = args[(i += 1)] ?? DEFAULT_TIMEOUT;
    } else if (arg.startsWith("--timeout=")) {
      parsed.timeout = arg.slice("--timeout=".length);
    } else if (arg === "--min-duration") {
      parsed.minDuration = args[(i += 1)] ?? "0";
    } else if (arg.startsWith("--min-duration=")) {
      parsed.minDuration = arg.slice("--min-duration=".length);
    } else {
      throw new Error(`unknown argument ${JSON.stringify(arg)}`);
    }
  }
  if (!parsed.table) {
    throw new Error("--table is required (path to a scenario table)");
  }
  return parsed;
}

/**
 * Parse a duration into seconds. Accepts a bare number (seconds) or a value
 * with an s/m/h suffix, so "--min-duration 3m" works. Throws on an invalid
 * value.
 */
export function parseDuration(s: string): number {
  const text = s.trim();
  if (!text) {
    throw new Error("duration is empty");
  }
  const unit = text[text.length - 1]!;
  const scale: Record<string, number> = { s: 1.0, m: 60.0, h: 3600.0 };
  if (unit in scale) {
    return Number(text.slice(0, -1)) * scale[unit]!;
  }
  const value = Number(text);
  if (Number.isNaN(value)) {
    throw new Error(`invalid duration ${JSON.stringify(s)}`);
  }
  return value;
}

/**
 * Resolve a table path: as-is when it exists, else alongside this script. A
 * relative path resolves whether the example is run from the repository root or
 * from its own directory.
 */
export function resolveTablePath(p: string): string {
  if (existsSync(p)) {
    return isAbsolute(p) ? p : resolve(cwd(), p);
  }
  const nearby = resolve(HERE, p);
  if (existsSync(nearby)) {
    return nearby;
  }
  throw new Error(`table ${JSON.stringify(p)} not found (cwd=${cwd()})`);
}

// ---------------------------------------------------------------------------
// Single pass
// ---------------------------------------------------------------------------

const NS_PER_SECOND = 1_000_000_000n;

/** Run the scenario once and print the summary report. */
function runOnce(tablePath: string, parsed: Table, timeoutS: number): number {
  printPlatform();
  const deadlineNs = process.hrtime.bigint() + BigInt(Math.round(timeoutS)) * NS_PER_SECOND;
  const report = runSync(parsed.fm, parsed.rows, deadlineNs);

  console.log(
    `Scenario: ${parsed.fm.name} (${basename(tablePath)}),` +
      ` slippage ${parsed.fm.slippageBps} bps\n`,
  );
  printLegend();
  printReport(report);
  return report.firstFail !== null ? 1 : 0;
}

/**
 * Re-run the scenario until at least `minDurS` of wall-clock has elapsed. Fails
 * fast on the first mismatch. Every ~10 s it prints a progress block with the
 * engine's running order/report latency; on completion it prints the platform
 * and an aggregate summary.
 */
function runRepeat(
  tablePath: string,
  parsed: Table,
  timeoutS: number,
  minDurS: number,
): number {
  console.log(
    `Repeat: ${parsed.fm.name} (${basename(tablePath)}),` +
      ` running for at least ${formatSeconds(minDurS)} ...\n`,
  );

  const agg = new EngineAggregate();
  const start = nowSeconds();
  let lastLog = start;
  let iterations = 0;
  for (;;) {
    const deadlineNs =
      process.hrtime.bigint() + BigInt(Math.round(timeoutS)) * NS_PER_SECOND;
    const report = runSync(parsed.fm, parsed.rows, deadlineNs);
    iterations += 1;

    if (report.firstFail !== null) {
      printReport(report);
      const elapsed = nowSeconds() - start;
      console.log(
        `repeat run failed on iteration ${iterations}` +
          ` after ${formatSeconds(elapsed)}`,
      );
      return 1;
    }
    agg.add(report);

    const now = nowSeconds();
    const elapsed = now - start;
    if (now - lastLog >= REPEAT_LOG_INTERVAL_S) {
      printHeartbeat(iterations, elapsed, minDurS, agg);
      lastLog = now;
    }
    if (elapsed >= minDurS) {
      // Platform info heads the final report, not the progress blocks.
      printPlatform();
      printRepeatSummary(iterations, elapsed, agg);
      return 0;
    }
  }
}

// ---------------------------------------------------------------------------
// Printing
// ---------------------------------------------------------------------------

/** Explain every field of the report once, so the output stands alone. */
function printLegend(): void {
  console.log("Legend:");
  console.log(
    "  operations  - table rows applied to the engine" +
      " (SEED/GROUP/ORDER/FILL; market-data ticks excluded)",
  );
  console.log("  accounts    - distinct accounts touched by the scenario");
  console.log("  total time  - wall-clock to run the whole scenario on this engine");
  console.log(
    "  order check - time to decide one order" +
      " (the pre-trade ACCEPT/REJECT check); n = orders checked",
  );
  console.log(
    "  reports     - time to apply one fill / execution report;" +
      " n = reports applied",
  );
  console.log();
}

/** Render the engine's outcome with the legend's field names. */
function printReport(report: Report): void {
  console.log(`== ${engineTitle(report.mode)} ==`);
  console.log(`  operations  : ${report.total}`);
  console.log(`  accounts    : ${report.accountsCount()}`);
  console.log(`  total time  : ${formatNs(report.wallClockNs)}`);
  printLatency("  order check ", report.order);
  printLatency("  reports     ", report.fill);
  if (report.firstFail !== null) {
    const fail: Failure = report.firstFail;
    console.log(
      `  result      : FAILED at line ${fail.row.line}` +
        ` (${fail.row.account}, ${fail.row.action}): ${fail.message}\n`,
    );
    return;
  }
  console.log("  result      : ALL PASS");
  console.log();
}

function printLatency(label: string, stats: LatencyStats): void {
  if (stats.count === 0) {
    console.log(`${label}: none`);
    return;
  }
  console.log(
    `${label}: n=${stats.count}  min=${formatNs(stats.minNs)}` +
      `  avg=${formatNs(stats.avgNs())}  max=${formatNs(stats.maxNs)}`,
  );
}

function printAggregate(agg: EngineAggregate, elapsedS: number): void {
  console.log(`== ${engineTitle(agg.mode)} ==`);
  console.log(`  operations  : ${agg.ops} total across the repeat run`);
  console.log(`  accounts    : ${agg.accounts}`);
  console.log(`  total time  : ${formatSeconds(elapsedS)} (whole repeat run)`);
  printLatency("  order check ", agg.order);
  printLatency("  reports     ", agg.fill);
  console.log();
}

function printRepeatSummary(
  iterations: number,
  elapsedS: number,
  agg: EngineAggregate,
): void {
  console.log(
    `Repeat summary: ${iterations} iterations in ${formatSeconds(elapsedS)},` +
      " the engine passed every time\n",
  );
  printLegend();
  printAggregate(agg, elapsedS);
}

function printHeartbeat(
  iterations: number,
  elapsedS: number,
  minDurS: number,
  agg: EngineAggregate,
): void {
  const left = Math.max(0, minDurS - elapsedS);
  const clock = new Date().toTimeString().slice(0, 8);
  console.log(
    `-- ${clock} . ${iterations} iter . elapsed ${formatSeconds(elapsedS)}` +
      ` . left ${formatSeconds(left)} --`,
  );
  const o = agg.order;
  const r = agg.fill;
  console.log(
    `  sync . ord ${formatNs(o.minNs)}/${formatNs(o.avgNs())}` +
      `/${formatNs(o.maxNs)} . rpt ${formatNs(r.minNs)}` +
      `/${formatNs(r.avgNs())}/${formatNs(r.maxNs)}`,
  );
}

function engineTitle(mode: string): string {
  if (mode === MODE_SYNC) {
    return "sequential engine (sync)";
  }
  return mode;
}

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

function formatSeconds(valueS: number): string {
  if (valueS >= 1.0) {
    return `${valueS.toFixed(3)}s`;
  }
  return `${(valueS * 1000).toFixed(3)}ms`;
}

function nowSeconds(): number {
  return Number(process.hrtime.bigint()) / 1e9;
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

function main(): number {
  let args: Args;
  try {
    args = parseArgs(argv.slice(2));
  } catch (exc) {
    stderr.write(`${(exc as Error).message}\n`);
    return 2;
  }

  let timeoutS: number;
  let minDurS: number;
  try {
    timeoutS = parseDuration(args.timeout);
    minDurS = parseDuration(args.minDuration);
  } catch (exc) {
    stderr.write(`${(exc as Error).message}\n`);
    return 2;
  }

  let resolved: string;
  try {
    resolved = resolveTablePath(args.table);
  } catch (exc) {
    stderr.write(`resolve table: ${(exc as Error).message}\n`);
    return 1;
  }

  let parsed: Table;
  try {
    parsed = parseFile(resolved);
  } catch (exc) {
    stderr.write(`parse: ${(exc as Error).message}\n`);
    return 1;
  }

  if (minDurS > 0) {
    return runRepeat(resolved, parsed, timeoutS, minDurS);
  }
  return runOnce(resolved, parsed, timeoutS);
}

if (import.meta.url === `file://${argv[1]}`) {
  exit(main());
}
