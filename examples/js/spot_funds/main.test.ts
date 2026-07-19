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

// Assertion-driven counterpart of main.ts.
//
// Drives the same shared helpers main() uses and asserts the three outcomes
// that make the example a lesson: the first buy is accepted (reserving funds),
// the second identical buy is rejected with InsufficientFunds (those funds are
// held), and the fill - carrying the first reservation's lock - settles without
// an account block.

import { describe, expect, it } from "vitest";

import {
  applyFill,
  buildEngine,
  buildFillReport,
  buildOrder,
  containsCode,
  configureSpotFundsPnlAxis,
  describe as describeRejects,
  enableTrackOnly,
  forceSpotFundsPnl,
  INSUFFICIENT_FUNDS,
  placeOrder,
  SCENARIO_SEED_FUNDS,
  seedFunds,
} from "./main.ts";

describe("spot_funds", () => {
  it("reserves, rejects the duplicate, and settles the fill", () => {
    const engine = buildEngine();
    seedFunds(engine, SCENARIO_SEED_FUNDS);

    // Buy #1 must be accepted and yield a non-null lock to carry to the fill.
    const buy1 = buildOrder();
    const placed1 = placeOrder(engine, buy1);
    expect(placed1.lock, `buy #1 rejected: ${describeRejects(placed1.rejects)}`).not.toBeNull();

    // Buy #2 must be rejected with InsufficientFunds: 60000 is held by buy #1,
    // only 40000 is available, and the order needs 60000.
    const buy2 = buildOrder();
    const placed2 = placeOrder(engine, buy2);
    expect(placed2.lock).toBeNull();
    expect(
      containsCode(placed2.rejects, INSUFFICIENT_FUNDS),
      `buy #2 reject codes = ${placed2.rejects.map((r) => r.code).join(",")}, want InsufficientFunds`,
    ).toBe(true);

    // The fill carries buy #1's lock, so SpotFunds settles that reservation; a
    // successful settlement produces no account block.
    const fill = buildFillReport(placed1.lock!);
    const result = applyFill(engine, fill);
    expect(
      result.accountBlocks.length,
      `fill produced ${result.accountBlocks.length} account block(s), want 0`,
    ).toBe(0);

    // Step 6 mirrors the example's runtime configurator call: TrackOnly drops
    // the insufficient-funds reject, so the same order is accepted.
    enableTrackOnly(engine);
    const buy3 = buildOrder();
    const placed3 = placeOrder(engine, buy3);
    expect(
      placed3.lock,
      `buy #3 rejected after TrackOnly: ${describeRejects(placed3.rejects)}`,
    ).not.toBeNull();

    // Step 7 exercises the account-wide P&L surface: retune the account barrier
    // and force-set the single live account P&L state. -120 is inside the
    // [-250, 250] barrier just configured, so the returned
    // PolicyConfigurationResult carries no account block.
    expect(() => configureSpotFundsPnlAxis(engine)).not.toThrow();
    const pnlResult = forceSpotFundsPnl(engine, "-120");
    expect(pnlResult.accountBlocks).toEqual([]);
  });
});
