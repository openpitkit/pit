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
 * `@openpit/engine/pretrade/policies` - built-in policy builders.
 *
 * Exposes the staged builders and barrier types for order validation,
 * order-size limits, rate limits, PnL kill switches, and spot-funds checks,
 * plus their `build*` factories. Order validation and limit-only spot funds are
 * ready immediately; barrier-driven builders become tokens accepted by
 * `EngineBuilder.builtin` after a barrier configuration call.
 *
 * @packageDocumentation
 */

// Importing any subpath initializes its platform runtime; package wrappers keep
// one module graph when callers mix public entries.
import "#runtime";

export {
  // Builder + barrier types.
  OrderValidationBuilder,
  OrderSizeLimitBuilder,
  OrderSizeLimit,
  OrderSizeBrokerBarrier,
  OrderSizeAssetBarrier,
  OrderSizeAccountAssetBarrier,
  RateLimitBuilder,
  RateLimit,
  RateLimitBrokerBarrier,
  RateLimitAssetBarrier,
  RateLimitAccountBarrier,
  RateLimitAccountAssetBarrier,
  PnlBoundsKillswitchBuilder,
  PnlBoundsBrokerBarrier,
  PnlBoundsAccountAssetBarrier,
  PnlBoundsAccountAssetBarrierUpdate,
  SpotFundsBuilder,
  SpotFundsOverride,
  SpotFundsPnlBoundsBarrier,
  SpotFundsPnlBoundsAccountGroupBarrier,
  SpotFundsPnlBoundsAccountBarrier,
  SpotFundsPnlBoundsAccountBarrierUpdate,
  SpotFundsPnlBoundsKillswitchBuilder,
  // Factory functions.
  buildOrderValidation,
  buildOrderSizeLimit,
  buildRateLimit,
  buildPnlBoundsKillswitch,
  buildSpotFunds,
  buildSpotFundsPnlBoundsKillswitch,
} from "../../wasm/openpit_js.js";

// Plain-object configuration inputs for the limit builders.
export type {
  OrderValidationReadyBuilder,
  OrderSizeLimitReadyBuilder,
  RateLimitReadyBuilder,
  PnlBoundsKillswitchReadyBuilder,
  SpotFundsReadyBuilder,
  SpotFundsPnlBoundsKillswitchReadyBuilder,
  OrderSizeLimitInit,
  RateLimitInit,
  RateLimitConfigureOptions,
  PnlBoundsKillswitchConfigureOptions,
  SetAccountPnlOptions,
  OrderSizeLimitConfigureOptions,
  SpotFundsConfigureOptions,
  SpotFundsLimitModeAccountEntry,
  SpotFundsLimitModeAccountGroupEntry,
  SpotFundsPnlBoundsKillswitchConfigureOptions,
  SetSpotFundsAccountPnlOptions,
} from "../../wasm/openpit_js.js";

// Spot-funds market-order pricing source: a string-discriminated enum value set
// plus its derived union type. `SpotFundsLimitMode` follows the same pattern.
export { SpotFundsPricingSource, SpotFundsLimitMode } from "../../types.js";
