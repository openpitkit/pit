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

// Type-level conformance checks for the custom-policy SDK surface. This file is
// intentionally NOT a `*.test.ts` (vitest ignores it) - it is exercised by
// `npm run typecheck`. Each `@ts-expect-error` asserts that a malformed input is
// a compile error; tsc fails the build if the expected error is ever absent, so
// this doubles as a regression guard that `preTrade`/`builtin` stay typed.

import { Engine } from "@openpit/engine";
import { type AccountId } from "@openpit/engine/param";
import {
  type AccountAdjustment,
  type ExecutionReport,
  type Order,
} from "@openpit/engine/model";
import {
  type Context,
  type Policy,
  type PolicyPreTradeResult,
  type PolicyReject,
  type PostTradeContext,
} from "@openpit/engine/pretrade";
import {
  buildOrderValidation,
  buildOrderSizeLimit,
  buildPnlBoundsKillswitch,
  buildRateLimit,
  buildSpotFunds,
  buildSpotFundsPnlBoundsKillswitch,
  SpotFundsPnlBoundsBarrier,
} from "@openpit/engine/pretrade/policies";
import { type AccountAdjustmentContext } from "@openpit/engine/accountadjustment";
import { type MarketDataService } from "@openpit/engine/marketdata";
import { RejectCode, RejectScope } from "@openpit/engine/reject";
import { Mutation } from "@openpit/engine/tx";

// A fully-typed custom policy implementing every hook. Each body uses its
// typed parameters through the public API so the argument types are exercised.
const complete: Policy = {
  name: "demo",
  policyGroupId: 7,
  checkPreTradeStart(ctx: Context, order: Order): Iterable<PolicyReject> {
    // `ctx.accountGroup` and `order.operation` are typed members.
    if (ctx.accountGroup === undefined && order.operation === undefined) {
      return [{ code: "Custom", reason: "no operation", details: "" }];
    }
    return [];
  },
  performPreTradeCheck(
    ctx: Context,
    order: Order,
  ): PolicyPreTradeResult | null {
    void ctx.accountGroup;
    void order.margin;
    return { lockPrices: ["100.50"] };
  },
  applyExecutionReport(ctx: PostTradeContext, report: ExecutionReport) {
    void ctx.accountGroup;
    void report.operation;
    return null;
  },
  applyAccountAdjustment(
    ctx: AccountAdjustmentContext,
    accountId: AccountId,
    adjustment: AccountAdjustment,
  ) {
    void ctx.accountControl;
    void accountId.value;
    void adjustment.amount;
    return {
      rejects: [{ code: "Custom", reason: "no", details: "" }],
      accountBlocks: [],
    };
  },
};

// A minimal policy: only the two required hooks.
const minimal: Policy = {
  name: "minimal",
  checkPreTradeStart(): Iterable<PolicyReject> {
    return [];
  },
  performPreTradeCheck(): PolicyPreTradeResult {
    return {};
  },
};

const typedReject: PolicyReject = {
  code: RejectCode.Custom,
  reason: "typed reject",
  details: "",
  scope: RejectScope.Account,
};
void typedReject;

declare const marketDataService: MarketDataService;

interface CustomOrderModel {
  readonly venueOrderId: string;
}

interface CustomExecutionReportModel {
  readonly venueExecutionId: string;
}

const genericModels: Policy<CustomOrderModel, CustomExecutionReportModel> = {
  name: "generic-models",
  checkPreTradeStart(_ctx, order): Iterable<PolicyReject> {
    void order.venueOrderId;
    return [];
  },
  performPreTradeCheck(_ctx, order): PolicyPreTradeResult {
    void order.venueOrderId;
    return {};
  },
  applyExecutionReport(_ctx, report): null {
    void report.venueExecutionId;
    return null;
  },
};
Engine.builder().preTrade(genericModels);

// `preTrade` accepts a typed `Policy` at both builder stages.
const ready = Engine.builder().preTrade(complete);
ready.preTrade(minimal);

// Always-ready builtin factories can be registered directly.
ready.builtin(buildOrderValidation());
ready.builtin(buildSpotFunds().withPolicyGroupId(3));

// Barrier-driven builders become ready only after a barrier-stage call.
const orderSizeReady = buildOrderSizeLimit().assetBarriers([]);
const rateLimitReady = buildRateLimit().assetBarriers([]);
const pnlReady = buildPnlBoundsKillswitch().brokerBarriers([]);
const spotFundsPnlReady = buildSpotFundsPnlBoundsKillswitch().globalBarrier(
  new SpotFundsPnlBoundsBarrier("-100", undefined),
);
ready.builtin(orderSizeReady);
ready.builtin(rateLimitReady);
ready.builtin(pnlReady);
ready.builtin(spotFundsPnlReady);

// Non-stage configuration and cloning preserve an established ready stage.
ready.builtin(orderSizeReady.withPolicyGroupId(3));
ready.builtin(rateLimitReady.clone());
ready.builtin(spotFundsPnlReady.marketData(marketDataService));

// ─── Negative checks: each must be a tsc error ───────────────────────────────

// Missing the required `name` field.
// @ts-expect-error - `name` is required on Policy.
const missingName: Policy = {
  checkPreTradeStart(): Iterable<PolicyReject> {
    return [];
  },
  performPreTradeCheck(): PolicyPreTradeResult {
    return {};
  },
};
void missingName;

// `name` of the wrong type.
const wrongNameType: Policy = {
  // @ts-expect-error - `name` must be a string.
  name: 123,
  checkPreTradeStart(): Iterable<PolicyReject> {
    return [];
  },
  performPreTradeCheck(): PolicyPreTradeResult {
    return {};
  },
};
void wrongNameType;

// Missing a required hook (`performPreTradeCheck`).
// @ts-expect-error - `performPreTradeCheck` is required.
const missingHook: Policy = {
  name: "broken",
  checkPreTradeStart(): Iterable<PolicyReject> {
    return [];
  },
  applyExecutionReport(): null {
    return null;
  },
};
void missingHook;

// A plain object that is not a Policy at all.
// @ts-expect-error - a bare object literal is not assignable to Policy.
Engine.builder().preTrade({ foo: "bar" });

// A primitive is not a Policy.
// @ts-expect-error - a string is not a Policy.
Engine.builder().preTrade("not a policy");

// A non-token object is rejected by `builtin`.
// @ts-expect-error - a bare object is not a builtin ready-builder.
Engine.builder().builtin({ notA: "builder" });

// A custom policy object is not a builtin ready-builder token.
// @ts-expect-error - a Policy is not a BuiltinReadyBuilder.
Engine.builder().builtin(minimal);

// Barrier-driven factories are not ready before a barrier-stage call.
// @ts-expect-error - an order-size-limit builder requires a barrier stage.
Engine.builder().builtin(buildOrderSizeLimit());
// @ts-expect-error - a rate-limit builder requires a barrier stage.
Engine.builder().builtin(buildRateLimit());
// @ts-expect-error - a P&L kill-switch builder requires a barrier stage.
Engine.builder().builtin(buildPnlBoundsKillswitch());
// @ts-expect-error - a spot-funds P&L builder requires a barrier stage.
Engine.builder().builtin(buildSpotFundsPnlBoundsKillswitch());
const orderSizeStillConfiguring = buildOrderSizeLimit().withPolicyGroupId(1);
// @ts-expect-error - non-barrier configuration does not advance the stage.
Engine.builder().builtin(orderSizeStillConfiguring);
const rateLimitStillConfiguring = buildRateLimit().clone();
// @ts-expect-error - cloning does not advance the stage.
Engine.builder().builtin(rateLimitStillConfiguring);
// Market-data configuration alone does not satisfy the barrier stage.
Engine.builder().builtin(
  // @ts-expect-error - a spot-funds P&L builder still requires a barrier stage.
  buildSpotFundsPnlBoundsKillswitch().marketData(marketDataService),
);

const unknownRejectCode: PolicyReject = {
  // @ts-expect-error - reject codes are a closed wire vocabulary.
  code: "TypoRejectCode",
  reason: "invalid",
  details: "",
};
void unknownRejectCode;

const unknownRejectScope: PolicyReject = {
  code: RejectCode.Custom,
  reason: "invalid",
  details: "",
  // @ts-expect-error - reject scope is either order or account.
  scope: "venue",
};
void unknownRejectScope;

new Mutation(
  () => undefined,
  () => undefined,
);
const mutationEvents: string[] = [];
new Mutation(
  () => mutationEvents.push("commit"),
  () => ({ rolledBack: true }),
);
new Mutation(
  // @ts-expect-error - mutation callbacks must complete synchronously.
  () => Promise.resolve(undefined),
  () => undefined,
);
new Mutation(
  // @ts-expect-error - thenable mutation returns are asynchronous too.
  () => ({ then: () => undefined }),
  () => undefined,
);

const asyncMutationDecision: PolicyPreTradeResult = {
  // @ts-expect-error - policy mutations cannot return a Promise.
  mutations: [
    {
      commit: () => Promise.resolve(undefined),
      rollback: () => undefined,
    },
  ],
};
void asyncMutationDecision;

const synchronousMutationDecision: PolicyPreTradeResult = {
  mutations: [
    {
      commit: () => mutationEvents.push("commit"),
      rollback: () => ({ rolledBack: true }),
    },
  ],
};
void synchronousMutationDecision;
