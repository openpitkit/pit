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

// Covers the panic boundary: a Rust panic must reach JavaScript as an
// `InternalError`, and the poisoned module must reject every later core call.
//
// The trigger is a real reachable panic, not a synthetic one. The engine reads
// a monotonic clock on every pre-trade call, and on wasm that clock is the
// host's `Performance` object; a host that does not expose one (a worklet, a
// stripped edge runtime) makes the very first clock read panic. Removing
// `globalThis.performance` reproduces exactly that host.
//
// The clock handle is cached on first successful read, and the Rust runtime
// converts one panic per instance (see `panic_nested.test.ts`), so this file
// raises exactly one panic and vitest gives it its own wasm instance.
import { Engine, InternalError, OpenpitError } from "@openpit/engine";
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

describe("panic boundary", () => {
  it("reports a panic and poisons every engine in the module", () => {
    const engine = Engine.builder().preTrade(accepting).build();
    const untouched = Engine.builder().preTrade(accepting).build();

    let caught: unknown;
    withoutPerformance(() => {
      try {
        engine.startPreTrade(order());
      } catch (error) {
        caught = error;
      }
    });

    expect(caught).toBeInstanceOf(InternalError);
    expect(caught).toBeInstanceOf(OpenpitError);
    expect((caught as InternalError).name).toBe("InternalError");
    // A usable report: the panic message plus its source location.
    expect(String((caught as InternalError).message)).toContain("panicked at");
    expect(String((caught as InternalError).message)).toContain("Performance");

    expect(() => engine.startPreTrade(order())).toThrow(InternalError);
    expect(() => untouched.startPreTrade(order())).toThrow(InternalError);
  });
});
