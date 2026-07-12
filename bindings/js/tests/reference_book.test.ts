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
  InstrumentId,
  ReferenceBook,
  ReferenceBookRegistrationError,
  ReferenceBookRegistrationErrorKind,
  SettlementLag,
  SettlementScheme,
  SettlementUnit,
  UnknownReferenceBookInstrumentId,
} from "@openpit/engine/core";
import { Instrument } from "@openpit/engine/param";

describe("ReferenceBook", () => {
  it("stores independent delivery and payment settlement legs", () => {
    const book = new ReferenceBook();
    const instrument = new Instrument("AAPL", "USD");
    const id = book.registerWithId(instrument, 42);
    const scheme = new SettlementScheme(
      new SettlementLag(2n, SettlementUnit.BusinessDays),
      new SettlementLag(1n, SettlementUnit.CalendarDays),
    );

    expect(book.resolve(instrument)?.value).toBe(id.value);
    expect(book.settlementScheme(id)).toBeUndefined();

    book.setSettlementScheme(id, scheme);
    const stored = book.settlementScheme(id);
    expect(stored?.delivery.n).toBe(2n);
    expect(stored?.delivery.unit).toBe(SettlementUnit.BusinessDays);
    expect(stored?.payment.n).toBe(1n);
    expect(stored?.payment.unit).toBe(SettlementUnit.CalendarDays);
  });

  it("preserves typed registration and unknown-id errors", () => {
    const book = new ReferenceBook();
    const id = new InstrumentId(42n);
    book.registerWithId(
      { underlyingAsset: "AAPL", settlementAsset: "USD" },
      id,
    );

    let duplicateId: unknown;
    try {
      book.registerWithId(
        { underlyingAsset: "MSFT", settlementAsset: "USD" },
        id,
      );
    } catch (error) {
      duplicateId = error;
    }
    expect(duplicateId).toBeInstanceOf(ReferenceBookRegistrationError);
    expect((duplicateId as ReferenceBookRegistrationError).kind).toBe(
      ReferenceBookRegistrationErrorKind.DuplicateId,
    );
    expect(
      (duplicateId as ReferenceBookRegistrationError).instrumentId?.value,
    ).toBe(42n);

    let duplicateInstrument: unknown;
    try {
      book.registerWithId(
        { underlyingAsset: "AAPL", settlementAsset: "USD" },
        43n,
      );
    } catch (error) {
      duplicateInstrument = error;
    }
    expect(duplicateInstrument).toBeInstanceOf(ReferenceBookRegistrationError);
    expect((duplicateInstrument as ReferenceBookRegistrationError).kind).toBe(
      ReferenceBookRegistrationErrorKind.DuplicateInstrument,
    );
    expect(
      (duplicateInstrument as ReferenceBookRegistrationError).instrument
        ?.underlyingAsset,
    ).toBe("AAPL");
    expect(ReferenceBookRegistrationErrorKind.Unknown).toBe("Unknown");

    let caught: unknown;
    try {
      book.settlementScheme(99n);
    } catch (error) {
      caught = error;
    }
    expect(caught).toBeInstanceOf(UnknownReferenceBookInstrumentId);
    expect(
      (caught as UnknownReferenceBookInstrumentId).instrumentId.value,
    ).toBe(99n);
  });
});
