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

// Companion of `panic.test.ts` for a panic raised by a nested call made from a
// policy callback. See `panic.test.ts` for how the panic is triggered.
import { Engine, InternalError } from "@openpit/engine";
import { TradeAmount } from "@openpit/engine/param";
import type { Policy, PolicyReject } from "@openpit/engine/pretrade";

const ACCOUNT = 99224416;

function order() {
  return {
    operation: {
      underlyingAsset: "AAPL",
      settlementAsset: "USD",
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

describe("panic boundary, nested calls", () => {
  it("reports a panic raised by a nested call from a policy callback", () => {
    const inner = Engine.builder().preTrade(accepting).build();
    let hookCalls = 0;
    const outer = Engine.builder()
      .preTrade({
        name: "nested-caller",
        checkPreTradeStart(): Iterable<PolicyReject> {
          return [];
        },
        performPreTradeCheck() {
          return null;
        },
        applyExecutionReport() {
          hookCalls += 1;
          inner.startPreTrade(order());
          return null;
        },
      })
      .build();

    let caught: unknown;
    withoutPerformance(() => {
      try {
        outer.applyExecutionReport({});
      } catch (error) {
        caught = error;
      }
    });

    expect(hookCalls).toBe(1);
    expect(caught).toBeInstanceOf(InternalError);
    expect(String((caught as Error).message)).toContain("Performance");
    expect(() => outer.applyExecutionReport({})).toThrow(InternalError);
  });
});
