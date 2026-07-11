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
 * `@openpit/engine` - embeddable pre-trade risk SDK as a WebAssembly engine for
 * Node, browsers, Deno, Bun, and edge runtimes.
 *
 * The root entry mirrors the SDK facade: the engine, its staged builder, the
 * wasm initialization hooks, and the typed {@link OpenpitError} hierarchy.
 * Everything else lives under a subpath that mirrors the SDK module tree:
 *
 * - `@openpit/engine/param` - value types, identifiers, and enums.
 * - `@openpit/engine/model` - order, execution-report, and adjustment models.
 * - `@openpit/engine/pretrade` - pipeline handles, results, contexts, and the
 *   custom-policy contract.
 * - `@openpit/engine/pretrade/policies` - built-in policy builders.
 * - `@openpit/engine/marketdata` - the live market-data service.
 * - `@openpit/engine/reject` - rejects, reject codes, and account-block types.
 * - `@openpit/engine/accountadjustment` - adjustment outcome types.
 * - `@openpit/engine/accounts` - the account registry handle.
 * - `@openpit/engine/tx` - the commit/rollback mutation pair.
 *
 * The published package ships two platform builds that share this surface:
 *
 * - Node (`node/`): reads the sibling `.wasm` from disk and instantiates
 *   synchronously at import.
 * - Browser / edge (`browser/`): instantiates from base64-inlined wasm
 *   synchronously at import (no `fetch`, no `fs`).
 *
 * Both default to zero-await usage:
 *
 * ```ts
 * import { Engine } from "@openpit/engine";
 * import { buildOrderValidation } from "@openpit/engine/pretrade/policies";
 *
 * const engine = Engine.builder()
 *   .builtin(buildOrderValidation())
 *   .build();
 * ```
 *
 * For environments that forbid synchronous wasm compilation, `await ready`
 * (or `await ensureInit()`) before touching any class.
 *
 * @packageDocumentation
 */

// Instantiate the wasm engine as a side effect of importing the root entry.
import "#runtime";

// Surface the platform initializer's lifecycle hooks at the root.
export { ready, ensureInit } from "#runtime";

export {
  // Engine + staged builder.
  Engine,
  Configurator,
  EngineBuilder,
  ReadyEngineBuilder,
} from "./wasm/openpit_js.js";

// Typed error hierarchy: the base class, every concrete subclass, the
// validation `ErrorCode` set, and the `AccountBlockErrorKind` discriminant.
export {
  OpenpitError,
  OpenpitValueError,
  ParamError,
  AssetError,
  AccountIdError,
  MarketDataError,
  UnknownInstrument,
  QuoteUnavailable,
  QuoteExpired,
  AlreadyRegistered,
  RegistrationError,
  UnknownInstrumentId,
  AccountGroupRegistrationError,
  AccountBlockError,
  LifecycleError,
  EngineBuildError,
  PolicyConfigureError,
  PolicyCallbackError,
  ErrorCode,
  RegistrationErrorKind,
  AccountGroupRegistrationErrorKind,
  AccountBlockErrorKind,
  EngineBuildErrorKind,
  ConfigureErrorKind,
} from "./errors.js";
export type {
  OpenpitErrorOptions,
  PolicyCallbackResult,
  ValueErrorOptions,
} from "./errors.js";

// The decimal input contract is a cross-cutting boundary type used by nearly
// every value-type and model constructor, so it stays at the root.
export type { DecimalInput } from "./types.js";
