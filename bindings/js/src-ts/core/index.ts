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
 * `@openpit/engine/core` - global instrument reference data.
 *
 * Exposes stable instrument identifiers and the caller-owned reference book,
 * including typed delivery and payment settlement schemes. `InstrumentId`
 * remains re-exported from `@openpit/engine/marketdata` for compatibility.
 *
 * @packageDocumentation
 */

import "#runtime";

export {
  InstrumentId,
  ReferenceBook,
  SettlementLag,
  SettlementScheme,
  SettlementUnit,
} from "../wasm/openpit_js.js";

export type { InstrumentInit } from "../wasm/openpit_js.js";

export {
  ReferenceBookRegistrationError,
  ReferenceBookRegistrationErrorKind,
  UnknownReferenceBookInstrumentId,
} from "../errors.js";
