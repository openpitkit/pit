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
// The package mirrors the SDK module tree as subpath entries. Each entry is a
// thin barrel that imports the shared `#runtime` initializer for its wasm-init
// side effect and re-exports its slice of the generated classes; the `runtime`
// entry is the `#runtime` target itself.
//
// Both formats use shared chunks, but mixing their independent graphs would
// still duplicate wasm state and class identities. Node therefore emits one
// internal CJS namespace aggregator, and `finalize-module-format.mjs` rewrites
// every public CJS/ESM entry as a thin wrapper around that canonical graph.
//   - Node: reads the sibling .wasm from disk; copy-node-wasm.mjs places
//     openpit_js_bg.wasm next to the shared chunk.
//   - Browser: wasm inlined as base64; esbuild tree-shakes the node:fs path.
//
// Declarations are emitted separately by `tsc` (build:dts -> dist/types): the
// Node runtime uses `import.meta.url`, which tsup's declaration bundler cannot
// parse, while tsc handles it natively and the per-file `.d.ts` map cleanly onto
// the subpath `exports`.

import { defineConfig } from "tsup";

// The SDK module tree plus the platform runtime, as bundler entries. The keys
// are the emitted basenames (and the subpath segments in the `exports` map).
function entryFor(platform: "node" | "browser"): Record<string, string> {
  return {
    index: "src-ts/index.ts",
    param: "src-ts/param/index.ts",
    model: "src-ts/model/index.ts",
    pretrade: "src-ts/pretrade/index.ts",
    "pretrade/policies": "src-ts/pretrade/policies/index.ts",
    marketdata: "src-ts/marketdata/index.ts",
    reject: "src-ts/reject/index.ts",
    accountadjustment: "src-ts/accountadjustment/index.ts",
    accounts: "src-ts/accounts/index.ts",
    tx: "src-ts/tx/index.ts",
    runtime: `src-ts/runtime.${platform}.ts`,
    ...(platform === "node"
      ? { "node-cjs-shared": "src-ts/node-cjs-shared.ts" }
      : {}),
  };
}

export default defineConfig([
  {
    entry: entryFor("node"),
    outDir: "dist/node",
    format: ["esm", "cjs"],
    platform: "node",
    target: "node18",
    dts: false,
    sourcemap: true,
    clean: true,
    // Entries are canonicalized through the `node-cjs-shared` CJS graph by the
    // finalizer; now-dead ESM chunks are removed before publication.
    splitting: true,
    // Rewrite `import.meta.url` (used by the runtime to locate the sibling
    // .wasm) to a working value.
    shims: true,
    // The `runtime` entry resolves `#runtime` under the default condition.
    esbuildOptions(options, { format }) {
      options.conditions = [
        "node",
        format === "cjs" ? "require" : "import",
        "default",
      ];
    },
    onSuccess: "node scripts/finalize-module-format.mjs node",
  },
  {
    entry: entryFor("browser"),
    outDir: "dist/browser",
    format: ["esm", "cjs"],
    platform: "browser",
    target: "es2022",
    dts: false,
    sourcemap: true,
    clean: true,
    // One shared chunk for the inlined wasm glue across all entries.
    splitting: true,
    // The `runtime` entry resolves `#runtime` under the `browser` condition.
    esbuildOptions(options, { format }) {
      options.conditions = [
        "browser",
        format === "cjs" ? "require" : "import",
        "default",
      ];
    },
    onSuccess: "node scripts/finalize-module-format.mjs browser",
  },
]);
