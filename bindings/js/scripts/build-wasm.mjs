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
// Drives the Rust wasm build for the `@openpit/engine` package:
//   1. cargo build -p openpit-js --release --target wasm32-unknown-unknown
//   2. wasm-bindgen --target web --typescript  (emits init()/initSync(bytes))
//   3. wasm-opt -Oz   (mandatory on the release path, optional for dev)
//
// `--target web` is required because it exports `initSync(bytes)` (synchronous,
// from controlled bytes) and a default async `init()`. That lets the Node and
// browser loaders instantiate from disk bytes or inlined bytes with no bundler
// dependency, in any environment.
//
// The cargo step pins a deterministic release configuration via
// CARGO_PROFILE_RELEASE_* so the artifact does not depend on local overrides:
// opt-level=z (smallest code), lto=fat, codegen-units=1. `panic = "abort"` is
// already the workspace release default and is left as-is - the boundary error
// model returns `Result<_, JsValue>` and never panics on reachable paths, so
// abort only fires on unrecoverable traps. wasm-opt is the second half of the
// size story; on the release path (OPENPIT_WASM_OPT=require) it is mandatory and
// a missing or failing pass is a hard error, while dev builds keep it optional.

import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, rmSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const packageDir = resolve(here, "..");
// The cargo workspace root is four levels up from bindings/js.
const workspaceRoot = resolve(packageDir, "..", "..");
const outDir = join(packageDir, "src-ts", "wasm");
const wasmInput = join(
  workspaceRoot,
  "target",
  "wasm32-unknown-unknown",
  "release",
  "openpit_js.wasm",
);

/**
 * Runs a command, inheriting stdio, and throws on a non-zero exit.
 *
 * @param {string} command
 * @param {string[]} args
 * @param {import("node:child_process").SpawnSyncOptions} [options]
 */
function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    stdio: "inherit",
    cwd: workspaceRoot,
    ...options,
  });
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw new Error(
      `${command} ${args.join(" ")} exited with code ${result.status}`,
    );
  }
}

/**
 * Returns true when `command` resolves on PATH (used to gate optional
 * `wasm-opt`).
 *
 * @param {string} command
 * @returns {boolean}
 */
function hasCommand(command) {
  const probe = spawnSync(command, ["--version"], { stdio: "ignore" });
  return !probe.error && probe.status === 0;
}

console.log("[build-wasm] cargo build (wasm32-unknown-unknown, release)");
run(
  "cargo",
  [
    "build",
    "-p",
    "openpit-js",
    "--release",
    "--target",
    "wasm32-unknown-unknown",
  ],
  {
    env: {
      ...process.env,
      // Deterministic release knobs (the rest - panic=abort, strip - come from
      // the workspace `[profile.release]`). Pinned here so a stray local
      // override cannot ship a larger or differently optimized artifact.
      CARGO_PROFILE_RELEASE_OPT_LEVEL: "z",
      CARGO_PROFILE_RELEASE_LTO: "fat",
      CARGO_PROFILE_RELEASE_CODEGEN_UNITS: "1",
    },
  },
);

if (!existsSync(wasmInput)) {
  throw new Error(`expected wasm artifact not found: ${wasmInput}`);
}

console.log("[build-wasm] wasm-bindgen (--target web --typescript)");
rmSync(outDir, { recursive: true, force: true });
mkdirSync(outDir, { recursive: true });
run("wasm-bindgen", [
  wasmInput,
  "--out-dir",
  outDir,
  "--target",
  "web",
  "--typescript",
  "--out-name",
  "openpit_js",
]);

// On the release/publish path wasm-opt is mandatory, so a missing or failing
// optimization fails the build instead of silently shipping a larger artifact.
const wasmOptRequired = process.env.OPENPIT_WASM_OPT === "require";

const bgWasm = join(outDir, "openpit_js_bg.wasm");
if (hasCommand("wasm-opt")) {
  console.log("[build-wasm] wasm-opt -Oz");
  // Current rustc/LLVM emits bulk-memory and reference-types ops for
  // wasm32-unknown-unknown by default; enable the matching wasm-opt features so
  // validation passes (these are baseline in every runtime we target - Node
  // 18+, modern browsers, Deno, Bun, Workers).
  const result = spawnSync(
    "wasm-opt",
    [
      "-Oz",
      "--enable-bulk-memory",
      "--enable-reference-types",
      "--enable-mutable-globals",
      "--enable-nontrapping-float-to-int",
      "--enable-sign-ext",
      bgWasm,
      "-o",
      bgWasm,
    ],
    { stdio: "inherit", cwd: packageDir },
  );
  if (result.status !== 0) {
    if (wasmOptRequired) {
      throw new Error(
        "[build-wasm] wasm-opt failed on the release path " +
          "(OPENPIT_WASM_OPT=require); refusing to ship an unoptimized artifact",
      );
    }
    console.warn("[build-wasm] wasm-opt failed; shipping unoptimized wasm");
  }
} else if (wasmOptRequired) {
  throw new Error(
    "[build-wasm] wasm-opt (binaryen) is required on the release path " +
      "(OPENPIT_WASM_OPT=require) but was not found on PATH; install binaryen " +
      "(brew install binaryen / apt-get install binaryen)",
  );
} else {
  console.log("[build-wasm] wasm-opt not found on PATH; skipping optimization");
}

console.log("[build-wasm] done");
