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
// Copies the raw `openpit_js_bg.wasm` next to the Node entry (`dist/node/`).
// The Node loader reads this sibling file from disk and instantiates
// synchronously, giving a smaller footprint and faster cold start than
// decoding inlined base64 (which the browser bundle uses instead).

import { copyFileSync, existsSync, mkdirSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptPath = fileURLToPath(import.meta.url);
const here = dirname(scriptPath);

export function copyNodeWasm() {
  const packageDir = resolve(here, "..");
  const source = join(packageDir, "src-ts", "wasm", "openpit_js_bg.wasm");
  const destDir = join(packageDir, "dist", "node");
  const dest = join(destDir, "openpit_js_bg.wasm");

  if (!existsSync(source)) {
    throw new Error(
      `generated wasm not found: ${source} (run build:wasm first)`,
    );
  }

  mkdirSync(destDir, { recursive: true });
  copyFileSync(source, dest);
  console.log(`[copy-node-wasm] copied ${dest}`);
}

if (process.argv[1] !== undefined && resolve(process.argv[1]) === scriptPath) {
  copyNodeWasm();
}
