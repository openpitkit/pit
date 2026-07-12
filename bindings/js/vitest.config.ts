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

import { fileURLToPath } from "node:url";

import { defineConfig } from "vitest/config";

export default defineConfig({
  resolve: {
    // Tests import the package by name (root and subpaths); each alias resolves
    // to the built Node entry so they exercise the real disk-loaded wasm and the
    // single shared instance. Longer specifiers come first so `@openpit/engine`
    // does not shadow `@openpit/engine/param` etc. Types come from the source
    // barrels via the tsconfig `paths` mapping (see tsconfig.json).
    alias: [
      {
        find: "npm:@openpit/engine",
        replacement: fileURLToPath(
          new URL("./dist/node/index.js", import.meta.url),
        ),
      },
      {
        find: "@openpit/engine/pretrade/policies",
        replacement: fileURLToPath(
          new URL("./dist/node/pretrade/policies.js", import.meta.url),
        ),
      },
      {
        find: "@openpit/engine/pretrade",
        replacement: fileURLToPath(
          new URL("./dist/node/pretrade.js", import.meta.url),
        ),
      },
      {
        find: "@openpit/engine/param",
        replacement: fileURLToPath(
          new URL("./dist/node/param.js", import.meta.url),
        ),
      },
      {
        find: "@openpit/engine/core",
        replacement: fileURLToPath(
          new URL("./dist/node/core.js", import.meta.url),
        ),
      },
      {
        find: "@openpit/engine/model",
        replacement: fileURLToPath(
          new URL("./dist/node/model.js", import.meta.url),
        ),
      },
      {
        find: "@openpit/engine/marketdata",
        replacement: fileURLToPath(
          new URL("./dist/node/marketdata.js", import.meta.url),
        ),
      },
      {
        find: "@openpit/engine/accountadjustment",
        replacement: fileURLToPath(
          new URL("./dist/node/accountadjustment.js", import.meta.url),
        ),
      },
      {
        find: "@openpit/engine/accounts",
        replacement: fileURLToPath(
          new URL("./dist/node/accounts.js", import.meta.url),
        ),
      },
      {
        find: "@openpit/engine/reject",
        replacement: fileURLToPath(
          new URL("./dist/node/reject.js", import.meta.url),
        ),
      },
      {
        find: "@openpit/engine/tx",
        replacement: fileURLToPath(
          new URL("./dist/node/tx.js", import.meta.url),
        ),
      },
      {
        find: "@openpit/engine",
        replacement: fileURLToPath(
          new URL("./dist/node/index.js", import.meta.url),
        ),
      },
    ],
  },
  test: {
    // Tests run the Node entry, which loads the disk-backed wasm.
    environment: "node",
    include: ["tests/**/*.test.ts"],
    coverage: {
      provider: "v8",
      reporter: ["text", "lcov"],
      include: ["src-ts/**"],
      exclude: ["src-ts/wasm/**"],
    },
  },
});
