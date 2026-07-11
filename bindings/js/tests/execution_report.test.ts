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

import { AssetError, Engine } from "@openpit/engine";
import {
  ExecutionReport,
  ExecutionReportFillDetails,
} from "@openpit/engine/model";
import { MonetaryAmount } from "@openpit/engine/param";
import { Lock } from "@openpit/engine/pretrade";
import {
  buildPnlBoundsKillswitch,
  buildSpotFunds,
  PnlBoundsBrokerBarrier,
} from "@openpit/engine/pretrade/policies";

describe("MonetaryAmount", () => {
  it("preserves an exact fee and validated currency", () => {
    const amount = new MonetaryAmount("0.2500", "USD");

    expect(amount.amount.toString()).toBe("0.2500");
    expect(amount.currency).toBe("USD");
    expect(amount.equals(amount.clone())).toBe(true);
  });

  it("preserves a negative fractional fee without floating-point loss", () => {
    const amount = new MonetaryAmount("-0.000000000000000001", "EUR");

    expect(amount.amount.toString()).toBe("-0.000000000000000001");
    expect(amount.currency).toBe("EUR");
  });

  it("delegates invalid currency rejection to the core asset type", () => {
    expect(() => new MonetaryAmount("0.25", "")).toThrow(AssetError);
  });
});

describe("ExecutionReportFillDetails fee", () => {
  it("sets, reads, clones, and clears a typed monetary amount", () => {
    const fill = new ExecutionReportFillDetails(new Lock([]));
    fill.fee = new MonetaryAmount("0.25", "USD");

    expect(fill.fee?.amount.toString()).toBe("0.25");
    expect(fill.fee?.currency).toBe("USD");

    const cloned = fill.clone();
    expect(cloned.fee?.amount.toString()).toBe("0.25");
    expect(cloned.fee?.currency).toBe("USD");

    fill.fee = undefined;
    expect(fill.fee).toBeUndefined();
  });

  it("extracts the fee from a plain execution-report object", () => {
    const report = new ExecutionReport();
    report.fill = {
      lock: new Lock([]),
      fee: { amount: "1.75", currency: "GBP" },
    };

    expect(report.fill?.fee?.amount.toString()).toBe("1.75");
    expect(report.fill?.fee?.currency).toBe("GBP");
  });

  it.each([undefined, 123, false])(
    "rejects a plain monetary amount with a non-string currency: %s",
    (currency) => {
      const report = new ExecutionReport();

      expect(() => {
        report.fill = {
          lock: new Lock([]),
          fee: { amount: "0.25", currency },
        } as never;
      }).toThrow(TypeError);
    },
  );
});

describe("execution-report required fields", () => {
  const operation = {
    underlyingAsset: "AAPL",
    settlementAsset: "USD",
    accountId: 99_224_416n,
    side: "BUY" as const,
  };

  it.each([
    {
      field: "P&L",
      financialImpact: { fee: "0" },
      details: "failed to access field 'financial_impact.pnl'",
    },
    {
      field: "fee",
      financialImpact: { pnl: "0" },
      details: "failed to access field 'financial_impact.fee'",
    },
  ])(
    "lets the core report a missing $field instead of throwing ParamError",
    ({ financialImpact, details }) => {
      const engine = Engine.builder()
        .builtin(
          buildPnlBoundsKillswitch().brokerBarriers([
            new PnlBoundsBrokerBarrier("USD", "-100", undefined),
          ]),
        )
        .build();

      const result = engine.applyExecutionReport({
        operation,
        financialImpact,
      });

      expect(result.accountBlocks).toHaveLength(1);
      expect(result.accountBlocks[0]!.code).toBe("MissingRequiredField");
      expect(result.accountBlocks[0]!.details).toBe(details);
    },
  );

  it("lets the core report a missing fill lock instead of throwing ParamError", () => {
    const engine = Engine.builder().builtin(buildSpotFunds()).build();

    const result = engine.applyExecutionReport({
      operation,
      fill: {
        leavesQuantity: "0",
        isFinal: true,
      },
    });

    expect(result.accountBlocks).toHaveLength(1);
    expect(result.accountBlocks[0]!.code).toBe("MissingRequiredField");
    expect(result.accountBlocks[0]!.details).toBe(
      "failed to access field 'fill.lock'",
    );
  });
});
