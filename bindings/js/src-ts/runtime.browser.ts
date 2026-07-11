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
//
// Browser / edge wasm initializer. Imports the base64-inlined wasm bytes and
// instantiates the module SYNCHRONOUSLY at load via `initSync(bytes)`, so a
// consumer just writes `import { Engine } from "@openpit/engine"` (or any
// subpath) with NO await. The inlined wasm means no `fetch`, no `fs`, and no
// separate asset to host - it works on any CDN and in edge isolates (Cloudflare
// Workers) with zero configuration.
//
// This module is resolved through the package `#runtime` internal import under
// the `browser` condition: the barrels import it for its init side effect, so
// importing ANY entry (root or subpath) instantiates the engine exactly once.
// Bundlers hoist this module into a single shared chunk, so two subpath imports
// never double-instantiate.
//
// Some environments forbid synchronous wasm compilation (very large modules in
// the main thread, or a strict CSP without `unsafe-eval`-style allowances). For
// those, an async `ready` promise / `ensureInit()` is exported; await it before
// touching any class. When `initSync` already succeeded, awaiting resolves
// immediately.

import init, { initSync } from "./wasm/openpit_js.js";
import { wasmBytes } from "./wasm/openpit_js_inline.js";

const bytes = wasmBytes();

// Zero-await default: compile and instantiate now. If synchronous compilation
// is disallowed the throw is caught and callers must await `ready`/`ensureInit`
// instead.
let syncInitialized = false;
try {
  initSync({ module: bytes });
  syncInitialized = true;
} catch {
  syncInitialized = false;
}

/**
 * Resolves once the wasm module is initialized.
 *
 * Already resolved when synchronous initialization succeeded at import.
 * Otherwise it performs asynchronous instantiation from the inlined bytes.
 */
export const ready: Promise<void> = syncInitialized
  ? Promise.resolve()
  : init({ module_or_path: bytes }).then(() => undefined);

/**
 * Async initialization hook.
 *
 * Awaits asynchronous instantiation in environments that forbid synchronous
 * wasm compilation. A no-op once the module is initialized.
 */
export async function ensureInit(): Promise<void> {
  await ready;
}
