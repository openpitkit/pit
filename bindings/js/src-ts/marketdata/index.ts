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
 * `@openpit/engine/marketdata` - the live market-data service.
 *
 * Exposes the service builder, the read-side service handle, the quote and TTL
 * value types, the instrument identifier, and the account-info read context.
 *
 * @packageDocumentation
 */

// Importing any subpath initializes its platform runtime; package wrappers keep
// one module graph when callers mix public entries.
import "#runtime";

export {
  MarketDataBuilder,
  MarketDataService,
  Quote,
  QuoteTtl,
  QuoteResolution,
  InstrumentId,
} from "../wasm/openpit_js.js";

// Plain-object inputs for the market-data constructors. `Instrument` and its
// `InstrumentInit` shape are value types under `@openpit/engine/param`.
export type { QuoteInit, AccountInfo } from "../wasm/openpit_js.js";

// Complete market-data and registration error family, sharing class identity
// with the package-root exports.
export {
  AlreadyRegistered,
  MarketDataError,
  QuoteExpired,
  QuoteUnavailable,
  RegistrationError,
  RegistrationErrorKind,
  UnknownInstrument,
  UnknownInstrumentId,
} from "../errors.js";
