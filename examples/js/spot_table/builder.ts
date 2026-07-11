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

// Domain-object builders for the spot_table example.

import {
  AccountGroupId,
  AccountId,
  AdjustmentAmount,
  Instrument,
  Price,
  TradeAmount,
} from "@openpit/engine/param";
import {
  type AccountAdjustmentInit,
  type ExecutionReportInit,
  type OrderInit,
} from "@openpit/engine/model";
import { DEFAULT_POLICY_GROUP_ID, Lock } from "@openpit/engine/pretrade";

import type { MarketFeed } from "./marketFeed.ts";
import type { Row } from "./table.ts";

/** Split "BASE/QUOTE" into [base, quote]. Throws when malformed. */
export function splitInstrument(s: string): [string, string] {
  const i = s.indexOf("/");
  if (i <= 0 || i === s.length - 1) {
    throw new Error(`instrument ${JSON.stringify(s)} must be BASE/QUOTE`);
  }
  return [s.slice(0, i), s.slice(i + 1)];
}

/** Turn "BASE/QUOTE" into an engine Instrument. */
export function parseInstrument(s: string): Instrument {
  const [base, quote] = splitInstrument(s);
  return new Instrument(base, quote);
}

/** Convert BUY/SELL to the wire side string. */
export function parseSide(s: string): "BUY" | "SELL" {
  if (s === "BUY") {
    return "BUY";
  }
  if (s === "SELL") {
    return "SELL";
  }
  throw new Error(`side must be BUY or SELL, got ${JSON.stringify(s)}`);
}

/**
 * Convert a free-form account label to a stable AccountId. The engine hashes
 * the string via FNV-1a; the runner keeps the source string for diagnostics.
 */
export function accountId(s: string): AccountId {
  if (!s) {
    throw new Error("account is required");
  }
  return AccountId.fromString(s);
}

/** Convert a free-form group label to a stable AccountGroupId. */
export function accountGroupId(s: string): AccountGroupId {
  if (!s) {
    throw new Error("group is required");
  }
  return AccountGroupId.fromString(s);
}

/** Turn a SEED row into an account adjustment seeding an absolute balance. */
export function buildSeedAdjustment(row: Row): AccountAdjustmentInit {
  return {
    operation: { asset: row.asset },
    amount: { balance: AdjustmentAmount.absolute(row.amount) },
  };
}

/**
 * Turn an ORDER row's qty or volume cell into a TradeAmount. Exactly one of the
 * two is set; the parser already enforced that.
 */
export function buildTradeAmount(row: Row): TradeAmount {
  if (row.volume) {
    return TradeAmount.volume(row.volume);
  }
  return TradeAmount.quantity(row.qty);
}

/**
 * Turn an ORDER row into an Order literal. The account label crosses as a
 * string (the engine hashes it via FNV-1a, the same as `AccountId.fromString`);
 * an empty price means a market order, so the field is left unset.
 */
export function buildOrder(row: Row, account: string): OrderInit {
  const [base, quote] = splitInstrument(row.instrument);
  return {
    operation: {
      underlyingAsset: base,
      settlementAsset: quote,
      accountId: account,
      side: parseSide(row.side),
      tradeAmount: buildTradeAmount(row),
      ...(row.price !== "" ? { price: row.price } : {}),
    },
  };
}

/**
 * Turn a FILL row into a final ExecutionReport.
 *
 * The price column on a FILL is the lock price (limit price for limit orders,
 * mark price for market orders). When it is empty the most recent quote pushed
 * for the instrument is reused.
 */
export function buildFillReport(
  row: Row,
  account: string,
  feed: MarketFeed,
): ExecutionReportInit {
  const priceStr = row.price || feed.latestPrice(row.instrument);
  if (!priceStr) {
    throw new Error(`FILL needs a price or a prior TICK for ${row.instrument}`);
  }

  const [base, quote] = splitInstrument(row.instrument);

  // The fill carries the pre-trade lock that ties it back to the reservation
  // the matching ORDER committed: one entry under the spot funds policy's
  // default group at the lock/reservation price. The Lock is the one wrapper
  // the report keeps; the prices and quantities cross as decimal strings.
  const lock = new Lock([
    [DEFAULT_POLICY_GROUP_ID(), Price.fromString(priceStr)],
  ]);

  return {
    operation: {
      underlyingAsset: base,
      settlementAsset: quote,
      accountId: account,
      side: parseSide(row.side),
    },
    financialImpact: { pnl: row.pnl || "0", fee: row.fee || "0" },
    fill: {
      lock,
      lastTrade: { price: priceStr, quantity: row.qty },
      leavesQuantity: "0",
      isFinal: true,
    },
  };
}
