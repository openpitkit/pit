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
// The scripted feed must trip the rate limit on the tail of the burst (a
// handful of "too frequent" rejects) and then trip the kill switch on the
// final execution report.

import type {
  ExecutionReportInit,
  OrderInit,
} from "@openpit/engine/model";
import type { PostTradeResult } from "@openpit/engine/pretrade";
import type { Reject } from "@openpit/engine/reject";
import { describe, expect, it } from "vitest";

import {
  buildEngine,
  type Reactor,
  run,
  scenarioLimits,
  scenarioStream,
  SCENARIO_ACCEPTED_REPORTS,
  SCENARIO_ATTEMPTS,
  SCENARIO_MAX_ORDERS_BURST,
} from "./main.ts";

// The CamelCase reject code produced by the RateLimit policy.
const RATE_LIMIT_EXCEEDED = "RateLimitExceeded";

/** Collects engine verdicts for assertion. */
class RecordingReactor implements Reactor {
  public accepted = 0;
  public readonly rejectCodes: string[] = [];
  public killSwitched = false;

  public onAccepted(_order: OrderInit): void {
    this.accepted += 1;
  }

  public onRejected(_order: OrderInit, rejects: Reject[]): void {
    for (const reject of rejects) {
      this.rejectCodes.push(reject.code);
    }
  }

  public onReport(_report: ExecutionReportInit, result: PostTradeResult): void {
    if (result.accountBlocks.length > 0) {
      this.killSwitched = true;
    }
  }
}

describe("rate_pnl_killswitch", () => {
  it("trips both kill switches", () => {
    const engine = buildEngine(scenarioLimits());

    const reactor = new RecordingReactor();
    const stream = scenarioStream();

    const stats = run(engine, stream, reactor);

    const wantAccepted = SCENARIO_MAX_ORDERS_BURST;
    const wantRejected = SCENARIO_ATTEMPTS - SCENARIO_MAX_ORDERS_BURST;
    const wantReports = SCENARIO_ACCEPTED_REPORTS;
    const wantPreTrade = SCENARIO_ATTEMPTS;

    expect(stats.accepted).toBe(wantAccepted);
    expect(stats.rejected).toBe(wantRejected);
    expect(stats.reports).toBe(wantReports);
    expect(stats.preTradeCalls).toBe(wantPreTrade);

    // Kill switch must trip on the final report.
    expect(stats.killSwitch).toBe(true);
    expect(reactor.killSwitched).toBe(true);
    expect(stats.killSwitchOnTrade).toBe(SCENARIO_ACCEPTED_REPORTS);

    // 99 * (-0.5) + (-460) = -509.5, tracked exactly in tenths (-5095).
    expect(stats.pnl).toBe(-5095n);

    // Every reject in the scenario must be a rate-limit reject: the burst
    // overshoots the ceiling within the same rate-limit window, so the tail
    // hits "too frequent".
    expect(reactor.rejectCodes).toHaveLength(wantRejected);
    for (const code of reactor.rejectCodes) {
      expect(code).toBe(RATE_LIMIT_EXCEEDED);
    }

    expect(stats.totalPreTradeNs > 0n).toBe(true);
    expect(stats.minPreTradeNs >= 0n).toBe(true);
    expect(stats.maxPreTradeNs >= stats.minPreTradeNs).toBe(true);
  });
});
