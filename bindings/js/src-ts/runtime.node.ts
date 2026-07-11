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
// Node platform runtime: the `#runtime` resolution target under the default
// condition. Reads the sibling `openpit_js_bg.wasm` from disk and instantiates
// the module SYNCHRONOUSLY at load via `initSync(bytes)`, so a consumer just
// writes `import { Engine } from "@openpit/engine"` (or any subpath) with NO
// await. The separate `.wasm` (rather than inlined base64) gives the smallest
// install footprint and the fastest cold start in Node.
//
// Barrels import `#runtime` for this init side effect. ESM entries share it via
// code splitting; CommonJS/ESM package wrappers point at one canonical CJS
// namespace bundle, so mixing subpaths and module formats never creates a
// second wasm-bindgen graph.

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { initSync } from "./wasm/openpit_js.js";

// `import.meta.url` points at the emitted entry; the copied `.wasm` sits next to
// it (see scripts/copy-node-wasm.mjs). tsup `shims: true` rewrites it to a
// working value where the runtime needs one.
const wasmPath = fileURLToPath(
  new URL("./openpit_js_bg.wasm", import.meta.url),
);
const wasmBytes = readFileSync(wasmPath);

// Zero-await default: compile and instantiate now. `initSync` runs the
// wasm-bindgen `start` hook and wires every exported class.
initSync({ module: wasmBytes });

/**
 * Resolves once the wasm module is initialized.
 *
 * The Node runtime initializes synchronously at import, so this is already
 * resolved by the time any consumer awaits it. It exists for symmetry with the
 * browser runtime and for code paths that prefer to gate on a promise.
 */
export const ready: Promise<void> = Promise.resolve();

/**
 * Async initialization hook.
 *
 * A no-op for the Node runtime (initialization already happened synchronously
 * at import). Provided so the same `await ensureInit()` shape works across
 * platforms.
 */
export async function ensureInit(): Promise<void> {
  await ready;
}
