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

// Market-data feed wrapper for the spot_table example.

import {
  type AccountGroupId,
  type AccountId,
} from "@openpit/engine/param";
import {
  type MarketDataService,
  Quote,
} from "@openpit/engine/marketdata";

import { parseInstrument } from "./builder.ts";
import type { Row } from "./table.ts";

/**
 * Wraps a live MarketDataService and replays TICK rows against it.
 *
 * The runner registers every instrument that any TICK row mentions up front,
 * then pushes quotes live at each TICK row's position. The feed also remembers
 * the last price pushed per instrument so a FILL row may omit its price and
 * reuse the latest quote as the lock price.
 */
export class MarketFeed {
  // The instrument ids are kept as their numeric values, not InstrumentId
  // wrappers: a wrapper is moved into the first push/pushFor that consumes it,
  // whereas a plain bigint can be reused across every tick for the instrument.
  private readonly ids = new Map<string, bigint>();
  private readonly latest = new Map<string, string>();

  public constructor(private readonly service: MarketDataService) {}

  /**
   * Register every instrument named by a TICK row. Registration only creates
   * the slot; quotes are published later by push/pushFor.
   */
  public registerInstruments(rows: Row[]): void {
    for (const row of rows) {
      if (row.action !== "TICK") {
        continue;
      }
      if (this.ids.has(row.instrument)) {
        continue;
      }
      let instrument;
      try {
        instrument = parseInstrument(row.instrument);
      } catch (exc) {
        throw new Error(`line ${row.line}: ${(exc as Error).message}`);
      }
      this.ids.set(row.instrument, this.service.register(instrument).value);
    }
  }

  /** Publish a global mark-price snapshot for instrument. */
  public push(instrument: string, price: string): void {
    const { id, quote } = this.quote(instrument, price);
    this.service.push(id, quote);
    this.latest.set(instrument, price);
  }

  /** Publish an addressed mark-price snapshot for specific targets. */
  public pushFor(
    instrument: string,
    price: string,
    accounts: AccountId[],
    groups: AccountGroupId[],
  ): void {
    const { id, quote } = this.quote(instrument, price);
    this.service.pushFor(id, quote, accounts, groups);
    this.latest.set(instrument, price);
  }

  /** Return the last price string pushed for instrument, or "" when none. */
  public latestPrice(instrument: string): string {
    return this.latest.get(instrument) ?? "";
  }

  private quote(instrument: string, price: string): { id: bigint; quote: Quote } {
    const id = this.ids.get(instrument);
    if (id === undefined) {
      throw new Error(
        `instrument ${instrument} is not registered` +
          " (every TICK instrument must appear in the table)",
      );
    }
    return { id, quote: new Quote({ mark: price }) };
  }
}
