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
//
// Published market-data snippets and these executable mirrors are one entity
// and must stay in lockstep. Each test starts with a `// Source:` comment naming
// the public wiki page and section; imports, harness, and assertions remain
// test-only.
//
// Public wiki pages covered here:
// - https://wiki.openpit.dev/Market-Data/
// - https://wiki.openpit.dev/Market-Data-TTL/
// - https://wiki.openpit.dev/Market-Data-Pricing/
//
// Run `npm run build` first: these tests import the built package.
// Imports are hoisted because TypeScript forbids in-body imports; everything
// after the imports is the verbatim snippet body. The Market Data TTL example
// uses a short asynchronous delay to advance past the 50 ms lifetime.

import { describe, expect, it } from "vitest";

import { Engine } from "@openpit/engine";
import {
  AdjustmentAmount,
  Instrument,
  Price,
  TradeAmount,
} from "@openpit/engine/param";
import { type OrderInit } from "@openpit/engine/model";
import {
  Quote,
  QuoteExpired,
  QuoteResolution,
  QuoteTtl,
} from "@openpit/engine/marketdata";
import {
  buildSpotFunds,
  SpotFundsOverride,
} from "@openpit/engine/pretrade/policies";

describe("Market-Data.md wiki examples", () => {
  it("registers, pushes, and reads a quote", () => {
    // Source: https://wiki.openpit.dev/Market-Data/
    // - Pushing and Reading Quotes
    // The engine spawns no threads; each call runs on the caller's thread.
    // See Threading-Contract for the full model.
    const service = Engine.builder().marketData(QuoteTtl.infinite()).build();

    // register returns an InstrumentId; read its numeric value so the same id can
    // be reused across calls (the id reads on every push and get below).
    const aaplId = service.register(new Instrument("AAPL", "USD")).value;

    // Publish a full snapshot into the default ("everyone-else") bucket.
    // Caller computes feed-observation-to-call age; the SDK cannot know it.
    const quoteSourceAgeMs = 20;
    service.push(
      aaplId,
      new Quote({ mark: "150", bid: "149.5", ask: "150.5" }),
      quoteSourceAgeMs,
    );

    // Read for an account with no group: the lookup falls through to the default
    // bucket. accountInfo is any object exposing an `accountGroup` getter; in
    // policy code this is usually the pre-trade context. Here we use a plain
    // stand-in. The resolution accepts a wire string.
    const accountId = 1;
    const accountInfo = { accountGroup: null };
    const quote = service.get(
      aaplId,
      accountId,
      accountInfo,
      "ACCOUNT_THEN_GROUP_THEN_DEFAULT",
    );
    if (quote === undefined) {
      throw new Error("quote must be present");
    }
    if (!quote.mark!.equals(Price.fromString("150"))) {
      throw new Error("unexpected mark");
    }
    if (!quote.bid!.equals(Price.fromString("149.5"))) {
      throw new Error("unexpected bid");
    }

    // resolve recovers the id from the instrument name.
    if (service.resolve(new Instrument("AAPL", "USD"))!.value !== aaplId) {
      throw new Error("unexpected resolve result");
    }

    expect(quote.mark!.equals(Price.fromString("150"))).toBe(true);
    expect(quote.bid!.equals(Price.fromString("149.5"))).toBe(true);
    expect(service.resolve(new Instrument("AAPL", "USD"))!.value).toBe(aaplId);
  });

  it("fans a quote out to specific accounts and a group", () => {
    // Source: https://wiki.openpit.dev/Market-Data/
    // - Targeted Fan-Out: push for
    const service = Engine.builder().marketData(QuoteTtl.infinite()).build();
    const aaplId = service.register(new Instrument("AAPL", "USD")).value;

    const groupId = 7;

    // Fan out to two accounts and one group simultaneously.
    service.pushFor(aaplId, new Quote({ mark: "150" }), 0, [10, 11], [groupId]);

    // Read back for account 10 under ACCOUNT_ONLY - hits the per-account bucket.
    const accountInfo = { accountGroup: null };
    const quote = service.get(aaplId, 10, accountInfo, "ACCOUNT_ONLY");
    if (quote === undefined) {
      throw new Error("quote must be present for account 10");
    }
    if (!quote.mark!.equals(Price.fromString("150"))) {
      throw new Error("unexpected mark for account 10");
    }

    expect(quote.mark!.equals(Price.fromString("150"))).toBe(true);
  });

  it("replaces the whole observation", () => {
    // Source: https://wiki.openpit.dev/Market-Data/ - Replace Versus Patch
    const service = Engine.builder().marketData(QuoteTtl.infinite()).build();
    const aaplId = service.register(new Instrument("AAPL", "USD")).value;

    service.push(aaplId, new Quote({ mark: "100", bid: "99", ask: "101" }), 0);

    // A mark-only observation clears bid and ask.
    service.push(aaplId, new Quote({ mark: "105" }), 0);

    const accountInfo = { accountGroup: null };
    const quote = service.get(
      aaplId,
      1,
      accountInfo,
      "ACCOUNT_THEN_GROUP_THEN_DEFAULT",
    )!;
    if (!quote.mark!.equals(Price.fromString("105"))) {
      throw new Error("unexpected mark after replacement");
    }
    if (quote.bid !== undefined) {
      throw new Error("bid must be cleared by replacement");
    }
    if (quote.ask !== undefined) {
      throw new Error("ask must be cleared by replacement");
    }

    expect(quote.mark!.equals(Price.fromString("105"))).toBe(true);
    expect(quote.bid).toBeUndefined();
    expect(quote.ask).toBeUndefined();
  });

  it("clears a quote and recovers it with a fresh push", () => {
    // Source: https://wiki.openpit.dev/Market-Data/ - Clearing a Quote
    const service = Engine.builder().marketData(QuoteTtl.infinite()).build();
    const aaplId = service.register(new Instrument("AAPL", "USD")).value;

    const accountInfo = { accountGroup: null };
    const read = () =>
      service.get(aaplId, 1, accountInfo, "ACCOUNT_THEN_GROUP_THEN_DEFAULT");

    service.push(aaplId, new Quote({ mark: "200" }), 0);

    // clear hides the quote but keeps the instrument registered.
    service.clear(aaplId);
    if (read() !== undefined) {
      throw new Error("quote must be absent after clear");
    }

    // Pushing again restores a quote for the same id.
    service.push(aaplId, new Quote({ mark: "210" }), 0);
    if (read() === undefined) {
      throw new Error("quote must be present after recovery push");
    }

    expect(read()).toBeDefined();
  });
});

describe("Market-Data-TTL.md wiki examples", () => {
  it("surfaces an expired quote with its stale snapshot", async () => {
    // Source: https://wiki.openpit.dev/Market-Data-TTL/ - Quote Freshness
    // A 50 ms service-wide lifetime: getOrErr distinguishes expired quotes.
    const service = Engine.builder().marketData(QuoteTtl.within(50)).build();
    const aaplId = service.register({
      underlyingAsset: "AAPL",
      settlementAsset: "USD",
    }).value;

    const accountId = 1;
    // A reading account with no group: any object exposing an `accountGroup` getter.
    const accountInfo = { accountGroup: null };

    const read = () =>
      service.getOrErr(
        aaplId,
        accountId,
        accountInfo,
        QuoteResolution.ACCOUNT_THEN_GROUP_THEN_DEFAULT(),
      );

    service.push(aaplId, { mark: "200" }, 0);
    const fresh = read();

    // After the lifetime elapses, QuoteExpired preserves the stale snapshot.
    await new Promise<void>((resolve) => setTimeout(resolve, 80));

    let expiredMark: string | undefined;
    try {
      read();
      throw new Error("the stale quote must expire");
    } catch (error) {
      if (!(error instanceof QuoteExpired)) {
        throw error;
      }
      expiredMark = error.quote.mark?.toString();
    }

    // A fresh push restores visibility.
    service.push(aaplId, { mark: "205" }, 0);
    const restored = read();

    if (
      fresh.mark?.toString() !== "200" ||
      expiredMark !== "200" ||
      restored.mark?.toString() !== "205"
    ) {
      throw new Error("unexpected quote freshness result");
    }

    expect(expiredMark).toBe("200");
  });
});

describe("Market-Data-Pricing.md wiki examples", () => {
  it("prices market orders from the book top with an instrument override", () => {
    // Source: https://wiki.openpit.dev/Market-Data-Pricing/
    // - Pricing Market Orders
    const builder = Engine.builder();

    // A shared market-data service feeds the policy's market-order pricing.
    const marketData = builder.marketData(QuoteTtl.infinite()).build();
    const aaplId = marketData.register({
      underlyingAsset: "AAPL",
      settlementAsset: "USD",
    }).value;
    marketData.push(aaplId, { mark: "200", bid: "199.5", ask: "200.5" }, 0);

    // Price from the top of book; AAPL overrides the global 100 bps slippage to
    // zero, so a buy is priced exactly at the ask. An instrument-level override
    // leaves the account and group ids unset.
    const engine = builder
      .builtin(
        buildSpotFunds().marketData(marketData, 100, "BookTop", [
          new SpotFundsOverride(aaplId, null, null, 0),
        ]),
      )
      .build();

    const accountId = 99224416;
    engine.applyAccountAdjustment(accountId, [
      {
        operation: { asset: "USD" },
        amount: { balance: AdjustmentAmount.absolute("1000") },
      },
    ]);

    // A market buy carries no price; the policy prices it from the book.
    const marketBuy = (): OrderInit => ({
      operation: {
        underlyingAsset: "AAPL",
        settlementAsset: "USD",
        accountId,
        side: "BUY",
        tradeAmount: TradeAmount.quantity("1"),
      },
    });

    // Market buy (no price): priced at the ask 200.5, which the balance covers.
    const passed = engine.executePreTrade(marketBuy());
    if (!passed.ok) {
      throw new Error("market buy must pass with a complete book quote");
    }
    const reservation = passed.reservation;
    if (reservation === undefined) {
      throw new Error("accepted execute result is missing its reservation");
    }
    reservation.commit();

    // Replace with a mark-only quote: bid and ask are gone, so BookTop can no
    // longer price a buy and the next market order is rejected.
    marketData.push(aaplId, { mark: "215" }, 0);
    const rejected = engine.executePreTrade(marketBuy());
    if (rejected.ok || rejected.rejects[0]?.code !== "MarkPriceUnavailable") {
      throw new Error("BookTop must reject a quote without bid and ask");
    }

    expect(passed.ok).toBe(true);
    expect(rejected.ok).toBe(false);
    expect(rejected.rejects[0].code).toBe("MarkPriceUnavailable");
  });
});
