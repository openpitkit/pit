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

// Scenario table parser for the spot_table example.

import { readFileSync } from "node:fs";

// ---------------------------------------------------------------------------
// Data model
// ---------------------------------------------------------------------------

/** Per-file configuration block. */
export interface Frontmatter {
  readonly name: string;
  readonly slippageBps: number;
}

/**
 * One parsed table row. Empty fields mean "not applicable to this action";
 * per-action validation enforces which cells each action requires or forbids.
 */
export interface Row {
  readonly line: number;
  readonly step: string;
  readonly account: string;
  readonly action: string;
  readonly instrument: string;
  readonly side: string;
  readonly qty: string;
  readonly volume: string;
  readonly price: string;
  readonly asset: string;
  readonly amount: string;
  readonly fee: string;
  readonly pnl: string;
  readonly group: string;
  readonly expect: string;
  readonly reject: string;
  readonly note: string;
}

/** Parsed scenario file. */
export interface Table {
  readonly fm: Frontmatter;
  readonly rows: Row[];
}

// ---------------------------------------------------------------------------
// Required headers
// ---------------------------------------------------------------------------

// Every other recognized column is optional and read by name when present;
// per-action validation then enforces the cells each action needs.
const REQUIRED_HEADERS = ["account", "action", "expect"];

// ---------------------------------------------------------------------------
// Public entry points
// ---------------------------------------------------------------------------

/** Read and parse a table file. */
export function parseFile(path: string): Table {
  return parse(readFileSync(path, "utf8"), path);
}

const STATE_START = 0;
const STATE_FM = 1;
const STATE_BODY = 2;
const STATE_AWAIT_DIVIDER = 3;
const STATE_ROWS = 4;
const STATE_DONE = 5;

/** Parse the table from `text`. `name` is used in error messages. */
export function parse(text: string, name: string): Table {
  const lines = text.split("\n");
  let fm: Frontmatter = { name: "", slippageBps: 0 };
  const rows: Row[] = [];
  let lineNo = 0;
  let state = STATE_START;
  let headers: string[] = [];

  for (const raw of lines) {
    lineNo += 1;
    const trimmed = raw.trim();

    if (state === STATE_START) {
      if (trimmed === "---") {
        state = STATE_FM;
        continue;
      }
      if (isTableRow(trimmed)) {
        headers = splitRow(trimmed);
        state = STATE_AWAIT_DIVIDER;
        continue;
      }
      // other text - skip
    } else if (state === STATE_FM) {
      if (trimmed === "---") {
        state = STATE_BODY;
        continue;
      }
      fm = parseFmLine(fm, trimmed, lineNo, name);
    } else if (state === STATE_BODY) {
      if (isTableRow(trimmed)) {
        headers = splitRow(trimmed);
        state = STATE_AWAIT_DIVIDER;
      }
    } else if (state === STATE_AWAIT_DIVIDER) {
      if (!isDividerRow(trimmed)) {
        throw new Error(
          `${name}:${lineNo}: expected table divider after header, got ${JSON.stringify(trimmed)}`,
        );
      }
      try {
        checkHeaders(headers);
      } catch (exc) {
        throw new Error(`${name}:${lineNo - 1}: ${(exc as Error).message}`);
      }
      state = STATE_ROWS;
    } else if (state === STATE_ROWS) {
      if (!isTableRow(trimmed)) {
        // table ended; v1 takes only the first table block.
        state = STATE_DONE;
        continue;
      }
      const fields = splitRow(trimmed);
      let row: Row;
      try {
        row = buildRow(fields, headers, lineNo);
      } catch (exc) {
        throw new Error(`${name}:${lineNo}: ${(exc as Error).message}`);
      }
      rows.push(row);
    }
    // STATE_DONE: ignore trailing prose
  }

  if (state !== STATE_ROWS && state !== STATE_DONE) {
    throw new Error(`${name}: no table found`);
  }
  if (rows.length === 0) {
    throw new Error(`${name}: table has no rows`);
  }
  return { fm, rows };
}

// ---------------------------------------------------------------------------
// Front-matter
// ---------------------------------------------------------------------------

function parseFmLine(
  fm: Frontmatter,
  line: string,
  lineNo: number,
  name: string,
): Frontmatter {
  if (line === "" || line.startsWith("#")) {
    return fm;
  }
  const i = line.indexOf(":");
  if (i < 0) {
    throw new Error(
      `${name}:${lineNo}: front-matter expects key: value, got ${JSON.stringify(line)}`,
    );
  }
  const key = line.slice(0, i).trim();
  const value = line.slice(i + 1).trim();
  if (key === "name") {
    return { name: value, slippageBps: fm.slippageBps };
  }
  if (key === "slippage_bps") {
    if (!/^-?\d+$/.test(value)) {
      throw new Error(
        `${name}:${lineNo}: slippage_bps: invalid literal for int: ${JSON.stringify(value)}`,
      );
    }
    const n = Number(value);
    if (n < 0 || n > 65535) {
      throw new Error(
        `${name}:${lineNo}: slippage_bps: value ${n} out of range 0..65535`,
      );
    }
    return { name: fm.name, slippageBps: n };
  }
  throw new Error(`${name}:${lineNo}: unknown front-matter key ${JSON.stringify(key)}`);
}

// ---------------------------------------------------------------------------
// Table row helpers
// ---------------------------------------------------------------------------

function isTableRow(s: string): boolean {
  return s.startsWith("|") && s.endsWith("|");
}

function isDividerRow(s: string): boolean {
  if (!isTableRow(s)) {
    return false;
  }
  return [...s].every((ch) => "|-: \t".includes(ch));
}

function splitRow(s: string): string[] {
  const inner = s.slice(1, -1); // strip leading and trailing '|'
  return inner.split("|").map((part) => part.trim());
}

function checkHeaders(got: string[]): void {
  for (const want of REQUIRED_HEADERS) {
    if (!hasHeader(got, want)) {
      throw new Error(
        `missing required column ${JSON.stringify(want)}` +
          ` (required: ${REQUIRED_HEADERS.join(",")})`,
      );
    }
  }
}

function hasHeader(headers: string[], name: string): boolean {
  const lower = name.toLowerCase();
  return headers.some((h) => h.toLowerCase() === lower);
}

function buildRow(fields: string[], headers: string[], lineNo: number): Row {
  const cell = (col: string): string => {
    const lower = col.toLowerCase();
    for (let i = 0; i < headers.length; i += 1) {
      if (headers[i]!.toLowerCase() === lower) {
        return i < fields.length ? fields[i]! : "";
      }
    }
    return "";
  };

  const row: Row = {
    line: lineNo,
    step: cell("#"),
    account: cell("account"),
    action: cell("action").toUpperCase(),
    instrument: cell("instrument"),
    side: cell("side").toUpperCase(),
    qty: cell("qty"),
    volume: cell("volume"),
    price: cell("price"),
    asset: cell("asset"),
    amount: cell("amount"),
    fee: cell("fee"),
    pnl: cell("pnl"),
    group: cell("group"),
    expect: cell("expect").toUpperCase(),
    reject: cell("reject"),
    note: cell("note"),
  };
  validateRow(row);
  return row;
}

// ---------------------------------------------------------------------------
// Per-action validation
// ---------------------------------------------------------------------------

function validateRow(row: Row): void {
  switch (row.action) {
    case "SEED":
      validateSeed(row);
      break;
    case "TICK":
      validateTick(row);
      break;
    case "ORDER":
      validateOrder(row);
      break;
    case "FILL":
      validateFill(row);
      break;
    case "GROUP":
      validateGroup(row);
      break;
    default:
      throw new Error(`unknown action ${JSON.stringify(row.action)}`);
  }
}

function validateSeed(row: Row): void {
  requireExpect(row, "SEED", ["OK", "REJECT"]);
  if (!row.account) {
    throw new Error("SEED requires account");
  }
  if (!row.asset || !row.amount) {
    throw new Error("SEED requires asset and amount");
  }
  forbid("SEED", {
    instrument: row.instrument,
    side: row.side,
    qty: row.qty,
    volume: row.volume,
    price: row.price,
    group: row.group,
  });
}

function validateTick(row: Row): void {
  requireExpect(row, "TICK", ["OK"]);
  if (!row.instrument || !row.price) {
    throw new Error("TICK requires instrument and price");
  }
  // account and group are optional: empty = global push, set = addressed push.
  forbid("TICK", {
    side: row.side,
    qty: row.qty,
    volume: row.volume,
    asset: row.asset,
    amount: row.amount,
    fee: row.fee,
    pnl: row.pnl,
    reject: row.reject,
  });
}

function validateOrder(row: Row): void {
  requireExpect(row, "ORDER", ["ACCEPT", "REJECT"]);
  if (!row.account) {
    throw new Error("ORDER requires account");
  }
  if (!row.instrument || !row.side) {
    throw new Error("ORDER requires instrument and side");
  }
  const hasQty = Boolean(row.qty);
  const hasVolume = Boolean(row.volume);
  if (hasQty && hasVolume) {
    throw new Error("ORDER must set exactly one of qty or volume, not both");
  }
  if (!hasQty && !hasVolume) {
    throw new Error("ORDER must set exactly one of qty or volume");
  }
  if (row.expect !== "REJECT" && row.reject) {
    throw new Error("ORDER reject code is only valid with expect REJECT");
  }
  forbid("ORDER", {
    asset: row.asset,
    amount: row.amount,
    fee: row.fee,
    pnl: row.pnl,
    group: row.group,
  });
}

function validateFill(row: Row): void {
  requireExpect(row, "FILL", ["OK", "REJECT"]);
  if (!row.account) {
    throw new Error("FILL requires account");
  }
  if (!row.instrument || !row.side || !row.qty || !row.price) {
    throw new Error("FILL requires instrument, side, qty and price");
  }
  if (row.expect !== "REJECT" && row.reject) {
    throw new Error("FILL reject code is only valid with expect REJECT");
  }
  forbid("FILL", {
    volume: row.volume,
    asset: row.asset,
    amount: row.amount,
    group: row.group,
  });
}

function validateGroup(row: Row): void {
  requireExpect(row, "GROUP", ["OK"]);
  if (!row.account || !row.group) {
    throw new Error("GROUP requires account and group");
  }
  forbid("GROUP", {
    instrument: row.instrument,
    side: row.side,
    qty: row.qty,
    volume: row.volume,
    price: row.price,
    asset: row.asset,
    amount: row.amount,
    fee: row.fee,
    pnl: row.pnl,
    reject: row.reject,
  });
}

function forbid(action: string, cells: Record<string, string>): void {
  for (const [col, value] of Object.entries(cells)) {
    if (value) {
      throw new Error(`${action} does not use the ${JSON.stringify(col)} column`);
    }
  }
}

function requireExpect(row: Row, action: string, allowed: string[]): void {
  if (allowed.includes(row.expect)) {
    return;
  }
  throw new Error(
    `${action} expect must be one of ${allowed.join("/")}, got ${JSON.stringify(row.expect)}`,
  );
}
