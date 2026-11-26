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
import { MonetaryAmount, TradeAmount } from "@openpit/engine/param";
import { Lock } from "@openpit/engine/pretrade";
import {
  buildPnlBoundsKillswitch,
  buildSpotFunds,
  PnlBoundsBrokerBarrier,
  PnlBoundsKillswitchBuilder,
} from "@openpit/engine/pretrade/policies";
import { isEvaluationFailure, RuleMode } from "@openpit/engine/reject";

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
        remainingReservedQuantity: "0",
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

describe("P&L-bounds kill switch execution reports", () => {
  const operation = {
    underlyingAsset: "AAPL",
    settlementAsset: "USD",
    accountId: 99_224_416n,
    side: "BUY" as const,
  };
  const order = {
    operation: { ...operation, tradeAmount: TradeAmount.quantity("1") },
  };

  function buildEngine(mode: RuleMode): Engine {
    return Engine.builder()
      .builtin(
        buildPnlBoundsKillswitch().brokerBarriers([
          new PnlBoundsBrokerBarrier("USD", "-100", undefined, mode),
        ]),
      )
      .build();
  }

  const report = { operation, financialImpact: { pnl: "-150", fee: "0" } };

  it("latches a Reject breach with the report's original cause", () => {
    const engine = buildEngine(RuleMode.Reject);
    const result = engine.applyExecutionReport(report);
    expect(result.accountBlocks.map((block) => block.code)).toEqual([
      "PnlKillSwitchTriggered",
    ]);

    engine.configure().setAccountPnl(PnlBoundsKillswitchBuilder.NAME, {
      account: operation.accountId,
      settlementAsset: operation.settlementAsset,
      pnl: "0",
    });
    const blocked = engine.startPreTrade(order);
    expect(blocked.ok).toBe(false);
    expect(blocked.rejects.map((reject) => reject.code)).toEqual([
      "PnlKillSwitchTriggered",
    ]);
  });

  describe.each([RuleMode.ReviewWithHold, RuleMode.ReviewWithoutHold])(
    "%s",
    (mode) => {
      it("leaves the account open after a report breaches the barrier", () => {
        const engine = buildEngine(mode);
        expect(engine.applyExecutionReport(report).accountBlocks).toEqual([]);

        const next = engine.startPreTrade(order);
        expect(next.ok).toBe(true);
        expect(next.rejects).toEqual([]);
        expect(next.reviews.map((review) => review.mode)).toEqual([mode]);
      });

      it("blocks the account after a report with missing P&L", () => {
        const engine = buildEngine(mode);
        const result = engine.applyExecutionReport({
          operation,
          financialImpact: { fee: "0" },
        });
        expect(result.accountBlocks).toMatchObject([
          {
            code: "MissingRequiredField",
            details: "failed to access field 'financial_impact.pnl'",
          },
        ]);

        const blocked = engine.startPreTrade(order);
        expect(blocked.ok).toBe(false);
        expect(blocked.rejects.map((reject) => reject.code)).toEqual([
          "MissingRequiredField",
        ]);
      });

      it("blocks the account after realized P&L accumulation overflows", () => {
        const engine = buildEngine(mode);
        const maximum = engine.applyExecutionReport({
          ...report,
          financialImpact: {
            ...report.financialImpact,
            pnl: "79228162514264337593543950335",
          },
        });
        expect(maximum.accountBlocks).toEqual([]);
        const overflow = engine.applyExecutionReport({
          ...report,
          financialImpact: { ...report.financialImpact, pnl: "1" },
        });
        expect(overflow.accountBlocks).toHaveLength(1);
        const block = overflow.accountBlocks[0]!;
        expect(isEvaluationFailure(block.code)).toBe(true);

        engine.configure().setAccountPnl(PnlBoundsKillswitchBuilder.NAME, {
          account: operation.accountId,
          settlementAsset: operation.settlementAsset,
          pnl: "0",
        });
        const blocked = engine.startPreTrade(order);
        expect(blocked.ok).toBe(false);
        expect(blocked.rejects).toMatchObject([
          {
            code: block.code,
            reason: block.reason,
            details: block.details,
            mode: RuleMode.Reject,
          },
        ]);
      });
    },
  );
});
