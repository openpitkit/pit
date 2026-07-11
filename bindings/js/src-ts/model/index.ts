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
 * `@openpit/engine/model` - request and feedback models.
 *
 * Exposes the order, execution-report, and account-adjustment models, each with
 * its plain-object `*Init` form for ergonomic construction from object
 * literals. `Trade` and `TradeAmount` remain compatibility exports here; new
 * code should import them from `@openpit/engine/param`.
 *
 * @packageDocumentation
 */

// Importing any subpath initializes its platform runtime; package wrappers keep
// one module graph when callers mix public entries.
import "#runtime";

export {
  // Order model.
  Order,
  OrderOperation,
  OrderPosition,
  OrderMargin,
  // Execution report model.
  ExecutionReport,
  ExecutionReportOperation,
  ExecutionReportFillDetails,
  ExecutionReportPositionImpact,
  FinancialImpact,
  // Account adjustment model.
  AccountAdjustment,
  AccountAdjustmentAmount,
  AccountAdjustmentBalanceOperation,
  AccountAdjustmentPositionOperation,
  AccountAdjustmentBounds,
} from "../wasm/openpit_js.js";

// Compatibility exports for callers that used the original model subpath.
export { Trade, TradeAmount } from "../param/index.js";

// Plain-object init interfaces behind the `T | TInit` constructor unions, so
// consumers can type their own object literals.
export type {
  OrderInit,
  OrderOperationInit,
  OrderPositionInit,
  OrderMarginInit,
  ExecutionReportInit,
  ExecutionReportOperationInit,
  ExecutionReportFillDetailsInit,
  ExecutionReportPositionImpactInit,
  FinancialImpactInit,
  AccountAdjustmentInit,
  AccountAdjustmentAmountInit,
  AccountAdjustmentBalanceOperationInit,
  AccountAdjustmentPositionOperationInit,
  AccountAdjustmentBoundsInit,
} from "../wasm/openpit_js.js";
export type { TradeInit } from "../param/index.js";
