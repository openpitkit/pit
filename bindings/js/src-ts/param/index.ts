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
 * `@openpit/engine/param` - numeric value types, identifiers, and enums.
 *
 * Exposes the financial value types, the account and instrument-free
 * identifiers, the position-mode and side enums, and the rounding-strategy
 * vocabulary used by the `*Rounded` factories.
 *
 * @packageDocumentation
 */

// Importing any subpath initializes its platform runtime; package wrappers keep
// one module graph when callers mix public entries.
import "#runtime";

export {
  // Value types (numeric domain).
  Price,
  Quantity,
  Pnl,
  Fee,
  Volume,
  Notional,
  CashFlow,
  PositionSize,
  Leverage,
  MonetaryAmount,
  AdjustmentAmount,
  Trade,
  TradeAmount,
  // Instrument definition (underlying + settlement asset).
  Instrument,
  // Identifiers.
  AccountId,
  AccountGroupId,
  // Enums.
  Side,
  PositionSide,
  PositionEffect,
  PositionMode,
} from "../wasm/openpit_js.js";

// Plain-object forms for parameter-level records.
export type {
  InstrumentInit,
  MonetaryAmountInit,
  TradeInit,
} from "../wasm/openpit_js.js";

// Rounding-strategy contract used by the `*Rounded` value-type factories, and
// the string-discriminated enum value sets with their derived union types.
export { FillType, ParamKind, RoundingStrategies } from "../types.js";
export type {
  RoundingStrategy,
  SideValue,
  PositionSideValue,
  PositionEffectValue,
  PositionModeValue,
} from "../types.js";

// Value-validation errors are available from the domain subpath as well as the
// package root, preserving the same class identity.
export {
  AccountIdError,
  AssetError,
  ErrorCode,
  OpenpitValueError,
  ParamError,
} from "../errors.js";
export type { ValueErrorOptions } from "../errors.js";
