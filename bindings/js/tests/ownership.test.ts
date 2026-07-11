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

import { Engine } from "@openpit/engine";
import {
  AccountOutcomeEntry,
  OutcomeAmount,
  PnlOutcomeAmount,
} from "@openpit/engine/accountadjustment";
import { Price } from "@openpit/engine/param";
import {
  buildOrderSizeLimit,
  buildOrderValidation,
  buildPnlBoundsKillswitch,
  buildRateLimit,
  buildSpotFundsPnlBoundsKillswitch,
  OrderSizeAccountAssetBarrier,
  OrderSizeAssetBarrier,
  OrderSizeLimit,
  PnlBoundsAccountAssetBarrier,
  PnlBoundsBrokerBarrier,
  RateLimit,
  RateLimitAccountAssetBarrier,
  RateLimitAccountBarrier,
  RateLimitAssetBarrier,
  SpotFundsPnlBoundsAccountBarrier,
  SpotFundsPnlBoundsAccountGroupBarrier,
  SpotFundsPnlBoundsBarrier,
} from "@openpit/engine/pretrade/policies";

describe("wasm wrapper ownership", () => {
  it("keeps ready builders persistent and reusable", () => {
    const token = buildOrderValidation();
    const groupOne = token.withPolicyGroupId(1);
    const groupTwo = token.withPolicyGroupId(2);

    expect(() => Engine.builder().builtin(groupOne).build()).not.toThrow();
    expect(() => Engine.builder().builtin(groupOne).build()).not.toThrow();
    expect(() => Engine.builder().builtin(groupTwo).build()).not.toThrow();
    expect(() => token.clone()).not.toThrow();
  });

  it("keeps the staged builder and rejected token reusable after validation", () => {
    const ready = Engine.builder().builtin(buildOrderValidation());
    const incomplete = buildSpotFundsPnlBoundsKillswitch();

    // Deliberately bypass the TypeScript stage to exercise the runtime guard.
    expect(() => ready.builtin(incomplete as never)).toThrow(
      /require at least one barrier/,
    );
    const corrected = incomplete.globalBarriers([
      new SpotFundsPnlBoundsBarrier("USD", "-100", undefined),
    ]);
    expect(() => ready.builtin(corrected)).not.toThrow();
    expect(() => ready.build()).not.toThrow();
  });

  it("does not consume exported barriers passed through iterables", () => {
    const orderSize = new OrderSizeLimit("10", "1000");
    const orderSizeAsset = new OrderSizeAssetBarrier(orderSize, "USD");
    const orderSizeAccount = new OrderSizeAccountAssetBarrier(
      orderSize,
      7,
      "USD",
    );
    const orderSizeBuilder = buildOrderSizeLimit();
    orderSizeBuilder.assetBarriers([orderSizeAsset]);
    orderSizeBuilder.accountAssetBarriers([orderSizeAccount]);
    expect(() =>
      orderSizeBuilder.assetBarriers([orderSizeAsset]),
    ).not.toThrow();
    expect(() =>
      orderSizeBuilder.accountAssetBarriers([orderSizeAccount]),
    ).not.toThrow();

    const rate = new RateLimit(10, 1000);
    const rateAsset = new RateLimitAssetBarrier(rate, "USD");
    const rateAccount = new RateLimitAccountBarrier(rate, 7);
    const rateAccountAsset = new RateLimitAccountAssetBarrier(rate, 7, "USD");
    const rateBuilder = buildRateLimit();
    rateBuilder.assetBarriers([rateAsset]);
    rateBuilder.accountBarriers([rateAccount]);
    rateBuilder.accountAssetBarriers([rateAccountAsset]);
    expect(() => rateBuilder.assetBarriers([rateAsset])).not.toThrow();
    expect(() => rateBuilder.accountBarriers([rateAccount])).not.toThrow();
    expect(() =>
      rateBuilder.accountAssetBarriers([rateAccountAsset]),
    ).not.toThrow();

    const brokerPnl = new PnlBoundsBrokerBarrier("USD", "-100", undefined);
    const accountPnl = new PnlBoundsAccountAssetBarrier(
      7,
      "USD",
      "0",
      "-100",
      undefined,
    );
    const pnlBuilder = buildPnlBoundsKillswitch();
    pnlBuilder.brokerBarriers([brokerPnl]);
    pnlBuilder.accountBarriers([accountPnl]);
    expect(() => pnlBuilder.brokerBarriers([brokerPnl])).not.toThrow();
    expect(() => pnlBuilder.accountBarriers([accountPnl])).not.toThrow();

    const globalSpot = new SpotFundsPnlBoundsBarrier("USD", "-100", undefined);
    const groupSpot = new SpotFundsPnlBoundsAccountGroupBarrier(
      8,
      "USD",
      "-100",
      undefined,
    );
    const accountSpot = new SpotFundsPnlBoundsAccountBarrier(
      7,
      "USD",
      "0",
      "-100",
      undefined,
    );
    const spotBuilder = buildSpotFundsPnlBoundsKillswitch();
    spotBuilder.globalBarriers([globalSpot]);
    spotBuilder.accountGroupBarriers([groupSpot]);
    spotBuilder.accountBarriers([accountSpot]);
    expect(() => spotBuilder.globalBarriers([globalSpot])).not.toThrow();
    expect(() => spotBuilder.accountGroupBarriers([groupSpot])).not.toThrow();
    expect(() => spotBuilder.accountBarriers([accountSpot])).not.toThrow();
  });

  it("does not consume exported values passed to outcome constructors", () => {
    const balance = new OutcomeAmount("1", "10");
    const pnl = new PnlOutcomeAmount("2", "20");
    const averageEntryPrice = Price.fromString("100");

    const entry = new AccountOutcomeEntry(
      "USD",
      balance,
      undefined,
      undefined,
      pnl,
      averageEntryPrice,
    );

    expect(entry.balance?.delta.toString()).toBe("1");
    expect(balance.absolute.toString()).toBe("10");
    expect(pnl.absolute.toString()).toBe("20");
    expect(averageEntryPrice.toString()).toBe("100");
  });
});
