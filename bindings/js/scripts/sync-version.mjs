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
// Reads the published package version from the crate manifest so the npm
// version is never hand-edited. `cargo release --shared-version` bumps
// `bindings/js/Cargo.toml`'s `version` field in lockstep with every other
// publishable workspace member, so a single `cargo release
// {patch|minor|major}` moves the npm package too.
//
// A staging/dry-run build may override the resolved value with the
// `OPENPIT_JS_VERSION` env var (the release workflow sets e.g.
// `0.4.0-alpha.<run>`).

import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const packageDir = resolve(here, "..");
const crateCargoToml = resolve(packageDir, "Cargo.toml");
const fallbackCargoToml = resolve(
  packageDir,
  "..",
  "..",
  "crates",
  "openpit",
  "Cargo.toml",
);

/**
 * Extracts the first `version = "..."` from a Cargo.toml `[package]` block.
 *
 * @param {string} path
 * @returns {string | null}
 */
function readCargoVersion(path) {
  let text;
  try {
    text = readFileSync(path, "utf8");
  } catch {
    return null;
  }
  // The crate version cargo-release bumps is the first top-level `version`
  // key; dependency `version` keys are always indented under a table entry.
  const match = text.match(/^\s*version\s*=\s*"([^"]+)"/m);
  return match ? match[1] : null;
}

/**
 * Resolves the package version: the `OPENPIT_JS_VERSION` override wins,
 * otherwise the crate manifest version (with a fallback to the core crate).
 *
 * @returns {string}
 */
export function resolveVersion() {
  const override = process.env.OPENPIT_JS_VERSION;
  if (override && override.trim() !== "") {
    return override.trim();
  }
  const crateVersion =
    readCargoVersion(crateCargoToml) ?? readCargoVersion(fallbackCargoToml);
  if (!crateVersion) {
    throw new Error(
      `could not determine version from ${crateCargoToml} or the core crate`,
    );
  }
  return crateVersion;
}

// When run directly (`npm run sync-version`) print the resolved version.
if (import.meta.url === `file://${process.argv[1]}`) {
  console.log(resolveVersion());
}
