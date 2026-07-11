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

/**
 * `@openpit/engine/pretrade` - the pre-trade pipeline surface.
 *
 * Exposes the single-use request and reservation handles, the start / execute /
 * post-trade / batch results, the policy-callback contexts, the serializable
 * price lock, and the custom-policy contract a consumer implements and passes
 * to `preTrade`.
 *
 * The built-in policy builders live one level deeper under
 * `@openpit/engine/pretrade/policies`.
 *
 * @packageDocumentation
 */

// Importing any subpath initializes its platform runtime; package wrappers keep
// one module graph when callers mix public entries.
import "#runtime";

export {
  // Pipeline handles + results.
  Request,
  Reservation,
  StartResult,
  ExecuteResult,
  DryRunReport,
  AccountAdjustmentBatchResult,
  PostTradeResult,
  // Policy callback contexts.
  Context,
  PostTradeContext,
  // Serializable price-lock payload.
  Lock,
  // Module constant.
  DEFAULT_POLICY_GROUP_ID,
} from "../wasm/openpit_js.js";

export { PolicyCallbackError } from "../errors.js";
export type { PolicyCallbackResult } from "../errors.js";

// Custom-policy SDK contract: the `Policy` interface a consumer implements and
// the decision / result / reject shapes its hooks return.
export type {
  Policy,
  PolicyReject,
  PolicyDecision,
  PolicyPreTradeResult,
  PolicyMutation,
  PolicyAccountAdjustmentResult,
  PolicyOutcomeAmount,
  PolicyPnlOutcomeAmount,
  PolicyAccountOutcomeEntry,
  LockEntry,
  RateLimitConfigureOptions,
  PnlBoundsKillswitchConfigureOptions,
  SetAccountPnlOptions,
  OrderSizeLimitConfigureOptions,
  SpotFundsConfigureOptions,
  SpotFundsLimitModeAccountEntry,
  SpotFundsLimitModeAccountGroupEntry,
  SpotFundsPnlBoundsKillswitchConfigureOptions,
  SetSpotFundsAccountPnlOptions,
} from "../wasm/openpit_js.js";

import type { Lock, LockEntry } from "../wasm/openpit_js.js";

/**
 * Entries accepted by the `Lock` constructor and `extend`: an existing `Lock`
 * or an iterable of `[policyGroupId, price]` pairs. The TS layer surfaces this
 * spread-friendly shape over the generated `any` parameter.
 */
export type LockEntries = Lock | Iterable<LockEntry>;
