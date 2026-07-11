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

import {
  AlreadyRegistered,
  Engine,
  MarketDataError,
  OpenpitError,
  QuoteExpired,
  QuoteUnavailable,
  RegistrationError,
  UnknownInstrument,
  UnknownInstrumentId,
} from "@openpit/engine";
import {
  AlreadyRegistered as MarketDataAlreadyRegistered,
  MarketDataError as MarketDataBaseError,
  QuoteExpired as MarketDataQuoteExpired,
  QuoteUnavailable as MarketDataQuoteUnavailable,
  RegistrationError as MarketDataRegistrationError,
  RegistrationErrorKind,
  QuoteTtl,
  UnknownInstrument as MarketDataUnknownInstrument,
  UnknownInstrumentId as MarketDataUnknownInstrumentId,
} from "@openpit/engine/marketdata";
import { Price } from "@openpit/engine/param";

describe("QuoteExpired", () => {
  it("surfaces the stale quote through the typed market-data error", async () => {
    const service = Engine.builder().marketData(QuoteTtl.within(1)).build();
    const instrumentId = service.register({
      underlyingAsset: "AAPL",
      settlementAsset: "USD",
    }).value;
    service.push(instrumentId, { mark: "200", bid: "199", ask: "201" });

    await new Promise((resolve) => setTimeout(resolve, 20));

    let caught: unknown;
    try {
      service.getOrErr(
        instrumentId,
        1,
        { accountGroup: null },
        "ACCOUNT_THEN_GROUP_THEN_DEFAULT",
      );
    } catch (error) {
      caught = error;
    }

    expect(MarketDataQuoteExpired).toBe(QuoteExpired);
    expect(caught).toBeInstanceOf(OpenpitError);
    expect(caught).toBeInstanceOf(MarketDataError);
    expect(caught).toBeInstanceOf(QuoteExpired);
    const expired = caught as QuoteExpired;
    expect(expired.name).toBe("QuoteExpired");
    expect(expired.quote.mark?.equals(Price.fromString("200"))).toBe(true);
    expect(expired.quote.bid?.equals(Price.fromString("199"))).toBe(true);
    expect(expired.quote.ask?.equals(Price.fromString("201"))).toBe(true);
  });
});

describe("MarketDataService reads", () => {
  it("maps every ordinary read miss to undefined", () => {
    const unavailableService = Engine.builder()
      .marketData(QuoteTtl.infinite())
      .build();
    const unavailableId = unavailableService.register({
      underlyingAsset: "UNAVAILABLE",
      settlementAsset: "USD",
    }).value;

    expect(
      unavailableService.get(
        unavailableId,
        1,
        null,
        "ACCOUNT_THEN_GROUP_THEN_DEFAULT",
      ),
    ).toBeUndefined();
    let unknownReads = 0;
    const unknownAccountInfo = {
      get accountGroup(): never {
        unknownReads += 1;
        throw new Error("unknown-instrument lookup must stay lazy");
      },
    };
    expect(
      unavailableService.get(
        999_999,
        1,
        unknownAccountInfo,
        "ACCOUNT_THEN_GROUP_THEN_DEFAULT",
      ),
    ).toBeUndefined();
    expect(unknownReads).toBe(0);

    const expiredService = Engine.builder()
      .marketData(QuoteTtl.within(0))
      .build();
    const expiredId = expiredService.register({
      underlyingAsset: "EXPIRED",
      settlementAsset: "USD",
    }).value;
    expiredService.push(expiredId, { mark: "1" });
    expect(
      expiredService.get(expiredId, 1, null, "ACCOUNT_THEN_GROUP_THEN_DEFAULT"),
    ).toBeUndefined();
  });

  it("does not read accountGroup when the core lookup does not need it", () => {
    const service = Engine.builder().marketData(QuoteTtl.infinite()).build();
    const instrumentId = service.register({
      underlyingAsset: "LAZY",
      settlementAsset: "USD",
    }).value;
    const accountId = 7;
    service.pushFor(instrumentId, { mark: "10" }, [accountId], []);
    service.setInstrumentAccountTtl(
      instrumentId,
      accountId,
      QuoteTtl.infinite(),
    );

    let reads = 0;
    const accountInfo = {
      get accountGroup(): never {
        reads += 1;
        throw new Error("accountGroup must stay lazy");
      },
    };

    expect(
      service
        .getOrErr(instrumentId, accountId, accountInfo, "ACCOUNT_ONLY")
        .mark?.toString(),
    ).toBe("10");
    expect(reads).toBe(0);
  });

  it("reads accountGroup at most once when quote and TTL resolution need it", () => {
    const service = Engine.builder().marketData(QuoteTtl.infinite()).build();
    const instrumentId = service.register({
      underlyingAsset: "ONCE",
      settlementAsset: "USD",
    }).value;
    service.push(instrumentId, { mark: "20" });

    let reads = 0;
    const accountInfo = {
      get accountGroup(): null {
        reads += 1;
        return null;
      },
    };

    expect(
      service
        .getOrErr(
          instrumentId,
          1,
          accountInfo,
          "ACCOUNT_THEN_GROUP_THEN_DEFAULT",
        )
        .mark?.toString(),
    ).toBe("20");
    expect(reads).toBe(1);
  });

  it.each(["get", "getOrErr"] as const)(
    "allows accountGroup to re-enter %s without overlapping core guards",
    (method) => {
      const service = Engine.builder().marketData(QuoteTtl.infinite()).build();
      const instrumentId = service.register({
        underlyingAsset: `REENTRANT-${method}`,
        settlementAsset: "USD",
      }).value;
      service.push(instrumentId, { mark: "30" });

      let reads = 0;
      const accountInfo = {
        get accountGroup(): null {
          reads += 1;
          service.push(instrumentId, { mark: "40" });
          return null;
        },
      };

      const quote = service[method](
        instrumentId,
        1,
        accountInfo,
        "ACCOUNT_THEN_GROUP_THEN_DEFAULT",
      );
      expect(quote?.mark?.toString()).toBe("40");
      expect(reads).toBe(1);
    },
  );

  it.each(["get", "getOrErr"] as const)(
    "preserves the original accountGroup getter exception from %s",
    (method) => {
      const service = Engine.builder().marketData(QuoteTtl.infinite()).build();
      const instrumentId = service.register({
        underlyingAsset: `THROW-${method}`,
        settlementAsset: "USD",
      }).value;
      service.push(instrumentId, { mark: "30" });

      const getterError = new Error(`accountGroup failed in ${method}`);
      const accountInfo = {
        get accountGroup(): never {
          throw getterError;
        },
      };

      let caught: unknown;
      try {
        service[method](
          instrumentId,
          1,
          accountInfo,
          "ACCOUNT_THEN_GROUP_THEN_DEFAULT",
        );
      } catch (error) {
        caught = error;
      }
      expect(caught).toBe(getterError);
    },
  );
});

describe("QuoteTtl.within", () => {
  it.each([
    -1,
    Number.NaN,
    Number.POSITIVE_INFINITY,
    Number.NEGATIVE_INFINITY,
    Number.MAX_VALUE,
  ])("rejects an invalid duration: %s", (durationMs) => {
    expect(() => QuoteTtl.within(durationMs)).toThrow(RangeError);
    expect(() => QuoteTtl.within(durationMs)).toThrow(
      /durationMs must be finite, non-negative, and representable as a duration/,
    );
  });

  it("preserves fractional millisecond precision", () => {
    expect(QuoteTtl.within(0.125).durationMs).toBeCloseTo(0.125, 12);
  });
});

describe("market-data error surface", () => {
  it("re-exports the complete error family with one class identity", () => {
    expect(MarketDataBaseError).toBe(MarketDataError);
    expect(MarketDataUnknownInstrument).toBe(UnknownInstrument);
    expect(MarketDataQuoteUnavailable).toBe(QuoteUnavailable);
    expect(MarketDataQuoteExpired).toBe(QuoteExpired);
    expect(MarketDataAlreadyRegistered).toBe(AlreadyRegistered);
    expect(MarketDataRegistrationError).toBe(RegistrationError);
    expect(MarketDataUnknownInstrumentId).toBe(UnknownInstrumentId);
  });

  it("keeps every read variant under MarketDataError", () => {
    const service = Engine.builder().marketData(QuoteTtl.infinite()).build();
    const instrumentId = service.register({
      underlyingAsset: "NO_QUOTE",
      settlementAsset: "USD",
    }).value;

    expect(() =>
      service.getOrErr(999_999n, 1, null, "ACCOUNT_THEN_GROUP_THEN_DEFAULT"),
    ).toThrowError(UnknownInstrument);

    let unavailable: unknown;
    try {
      service.getOrErr(
        instrumentId,
        1,
        null,
        "ACCOUNT_THEN_GROUP_THEN_DEFAULT",
      );
    } catch (error) {
      unavailable = error;
    }
    expect(unavailable).toBeInstanceOf(MarketDataError);
    expect(unavailable).toBeInstanceOf(QuoteUnavailable);
  });

  it("preserves structured registration conflict payloads", () => {
    const service = Engine.builder().marketData(QuoteTtl.infinite()).build();
    const instrument = {
      underlyingAsset: "STRUCTURED",
      settlementAsset: "USD",
    };
    service.registerWithId(instrument, 41n);

    let duplicateId: unknown;
    try {
      service.registerWithId(
        { underlyingAsset: "OTHER", settlementAsset: "USD" },
        41n,
      );
    } catch (error) {
      duplicateId = error;
    }
    expect(duplicateId).toBeInstanceOf(RegistrationError);
    expect((duplicateId as RegistrationError).kind).toBe(
      RegistrationErrorKind.DuplicateId,
    );
    expect((duplicateId as RegistrationError).instrumentId?.value).toBe(41n);

    let duplicateInstrument: unknown;
    try {
      service.registerWithId(instrument, 42n);
    } catch (error) {
      duplicateInstrument = error;
    }
    expect(duplicateInstrument).toBeInstanceOf(RegistrationError);
    expect((duplicateInstrument as RegistrationError).kind).toBe(
      RegistrationErrorKind.DuplicateInstrument,
    );
    expect(
      (duplicateInstrument as RegistrationError).instrument?.underlyingAsset,
    ).toBe("STRUCTURED");

    let alreadyRegistered: unknown;
    try {
      service.register(instrument);
    } catch (error) {
      alreadyRegistered = error;
    }
    expect(alreadyRegistered).toBeInstanceOf(AlreadyRegistered);
    expect(
      (alreadyRegistered as AlreadyRegistered).instrument.underlyingAsset,
    ).toBe("STRUCTURED");

    let unknownId: unknown;
    try {
      service.push(99n, { mark: "1" });
    } catch (error) {
      unknownId = error;
    }
    expect(unknownId).toBeInstanceOf(UnknownInstrumentId);
    expect((unknownId as UnknownInstrumentId).instrumentId.value).toBe(99n);
  });
});
