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
 * `@openpit/engine/accountadjustment` - adjustment outcome types.
 *
 * Exposes the policy-callback context for a non-trading account adjustment and
 * the per-account outcome it produces.
 *
 * @packageDocumentation
 */

// Importing any subpath initializes its platform runtime; package wrappers keep
// one module graph when callers mix public entries.
import "#runtime";

export {
  AccountAdjustmentContext,
  AccountAdjustmentOutcome,
  AccountOutcomeEntry,
  OutcomeAmount,
  PnlHaltReason,
  PnlOutcome,
  PnlOutcomeAmount,
} from "../wasm/openpit_js.js";

export type { PnlHaltReasonKind } from "../wasm/openpit_js.js";
