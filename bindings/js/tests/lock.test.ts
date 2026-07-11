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

// See engine.test.ts for the import-resolution scheme. Run `npm run build`
// first.
import { Price } from "@openpit/engine/param";
import { Lock } from "@openpit/engine/pretrade";

// Builds a lock with records under the default and a non-default group.
function makeLock(): Lock {
  // `undefined` yields an empty lock; records are added via `push`.
  const lock = new Lock(undefined);
  lock.push(0, Price.fromString("100.50"));
  lock.push(0, Price.fromString("100.75"));
  lock.push(3, Price.fromString("200.25"));
  return lock;
}

// Normalizes a lock's entries to comparable `[group, decimalString]` tuples.
// `Lock.entries()` is typed as `[number, Price][]`, so the pair destructures
// without a cast.
function entriesOf(lock: Lock): Array<[number, string]> {
  return lock.entries().map(([group, price]) => [group, price.toString()]);
}

describe("Lock serialization round-trips", () => {
  it("round-trips through JSON to equal entries", () => {
    const lock = makeLock();
    const restored = Lock.fromJson(lock.toJson());
    expect(lock.equals(restored)).toBe(true);
    expect(entriesOf(restored)).toEqual(entriesOf(lock));
  });

  it("round-trips through MessagePack to equal entries", () => {
    const lock = makeLock();
    const bytes = lock.toMsgpack();
    expect(bytes).toBeInstanceOf(Uint8Array);
    const restored = Lock.fromMsgpack(bytes);
    expect(lock.equals(restored)).toBe(true);
    expect(entriesOf(restored)).toEqual(entriesOf(lock));
  });

  it("round-trips through CBOR to equal entries", () => {
    const lock = makeLock();
    const bytes = lock.toCbor();
    expect(bytes).toBeInstanceOf(Uint8Array);
    const restored = Lock.fromCbor(bytes);
    expect(lock.equals(restored)).toBe(true);
    expect(entriesOf(restored)).toEqual(entriesOf(lock));
  });

  it("produces equal entries across all three wire formats", () => {
    const lock = makeLock();
    const fromJson = Lock.fromJson(lock.toJson());
    const fromMsgpack = Lock.fromMsgpack(lock.toMsgpack());
    const fromCbor = Lock.fromCbor(lock.toCbor());

    expect(entriesOf(fromJson)).toEqual(entriesOf(fromMsgpack));
    expect(entriesOf(fromMsgpack)).toEqual(entriesOf(fromCbor));
  });
});

describe("Lock accessors", () => {
  it("reports its record count and per-group prices", () => {
    const lock = makeLock();
    expect(lock.size()).toBe(3);
    expect(lock.length).toBe(3);
    expect(lock.pricesOf(0).map((p) => p.toString())).toEqual([
      "100.50",
      "100.75",
    ]);
    expect(lock.pricesOf(3).map((p) => p.toString())).toEqual(["200.25"]);
  });

  it("reports emptiness via isEmpty", () => {
    expect(new Lock(undefined).isEmpty).toBe(true);
    expect(makeLock().isEmpty).toBe(false);
  });

  it("flattens every price in iteration order via prices", () => {
    // Default-group records first, then each non-default group in insertion
    // order - the same order as entries().
    expect(
      makeLock()
        .prices()
        .map((p) => p.toString()),
    ).toEqual(["100.50", "100.75", "200.25"]);
    expect(new Lock(undefined).prices()).toEqual([]);
  });
});
