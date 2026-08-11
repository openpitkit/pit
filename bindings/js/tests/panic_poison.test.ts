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

// Companion of `panic.test.ts` for the reach of the module poison: the caller-
// owned registries and the handles a policy callback retains must refuse to
// touch core state too, otherwise a caller following the published contract
// could still mutate a module the docs declare dead - and reach a second panic,
// which the runtime can no longer convert.
//
// See `panic.test.ts` for how the panic is triggered and why each panicking
// case needs its own wasm instance. The panic is raised from a nested call
// inside an account-adjustment callback: that is the only flow that both hands
// its callback an `AccountControl` and reaches that callback before the engine
// reads the clock, which the first successful read would cache for good.
import { Engine, InternalError } from "@openpit/engine";
import {
  ReferenceBook,
  SettlementLag,
  SettlementScheme,
  SettlementUnit,
} from "@openpit/engine/core";
import { AdjustmentAmount, TradeAmount } from "@openpit/engine/param";
import { QuoteTtl } from "@openpit/engine/marketdata";
import { type AccountAdjustmentContext } from "@openpit/engine/accountadjustment";
import { AccountBlock } from "@openpit/engine/reject";
import type { Policy, PolicyReject } from "@openpit/engine/pretrade";

const ACCOUNT = 99224416;
const INSTRUMENT = { underlyingAsset: "AAPL", settlementAsset: "USD" };

function order() {
  return {
    operation: {
      ...INSTRUMENT,
      accountId: ACCOUNT,
      side: "BUY" as const,
      tradeAmount: TradeAmount.quantity("100"),
      price: "185.00",
    },
  };
}

const accepting: Policy = {
  name: "accepting",
  checkPreTradeStart(): Iterable<PolicyReject> {
    return [];
  },
  performPreTradeCheck() {
    return null;
  },
};

// Runs `body` on a host with no `Performance` object, restoring it afterwards.
function withoutPerformance<T>(body: () => T): T {
  const descriptor = Object.getOwnPropertyDescriptor(globalThis, "performance");
  if (descriptor === undefined) {
    throw new Error("this host exposes no globalThis.performance to remove");
  }
  Reflect.deleteProperty(globalThis, "performance");
  try {
    return body();
  } finally {
    Object.defineProperty(globalThis, "performance", descriptor);
  }
}

describe("panic boundary, poisoned surfaces", () => {
  it("rejects every surface that would reach core state", () => {
    const inner = Engine.builder().preTrade(accepting).build();
    let retained: AccountAdjustmentContext["accountControl"] | undefined;
    const outer = Engine.builder()
      .preTrade({
        ...accepting,
        name: "nested-caller",
        applyAccountAdjustment(ctx) {
          retained = ctx.accountControl;
          inner.startPreTrade(order());
          return null;
        },
      })
      .build();

    // Registration reads no clock, so this setup leaves the panic ahead of us.
    const service = Engine.builder().marketData(QuoteTtl.infinite()).build();
    const instrumentId = service.register(INSTRUMENT);
    const book = new ReferenceBook();
    const bookId = book.register(INSTRUMENT);
    const scheme = new SettlementScheme(
      new SettlementLag(2n, SettlementUnit.BusinessDays),
      new SettlementLag(1n, SettlementUnit.CalendarDays),
    );

    let caught: unknown;
    withoutPerformance(() => {
      try {
        outer.applyAccountAdjustment(ACCOUNT, [
          {
            operation: { asset: "USD" },
            amount: { balance: AdjustmentAmount.absolute("1") },
          },
        ]);
      } catch (error) {
        caught = error;
      }
    });

    expect(caught).toBeInstanceOf(InternalError);
    expect(String((caught as Error).message)).toContain("Performance");

    // A handle the callback retained reports the module defect rather than its
    // own lifecycle state: the poison check runs before the validity check, so
    // a dead module never looks like an ordinary stale handle.
    expect(retained).toBeDefined();
    const block = new AccountBlock(
      "nested-caller",
      "Other",
      "late block",
      "module is poisoned",
      undefined,
    );
    expect(() => retained!.block(block)).toThrow(InternalError);

    // `push` is the market-data clock reader, so leaving it unguarded is what
    // would raise the unconvertible second panic.
    expect(() => service.push(instrumentId, { mark: "200" }, 0)).toThrow(
      InternalError,
    );
    expect(() =>
      service.getOrErr(instrumentId, ACCOUNT, null, "ACCOUNT_ONLY"),
    ).toThrow(InternalError);
    expect(() => service.register(INSTRUMENT)).toThrow(InternalError);
    expect(() => service.clear(instrumentId)).toThrow(InternalError);

    expect(() => book.register(INSTRUMENT)).toThrow(InternalError);
    expect(() => book.registerWithId(INSTRUMENT, 77)).toThrow(InternalError);
    expect(() => book.setSettlementScheme(bookId, scheme)).toThrow(
      InternalError,
    );
    expect(() => book.clearSettlementScheme(bookId)).toThrow(InternalError);
    expect(() => book.settlementScheme(bookId)).toThrow(InternalError);
  });
});
