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
// Emits the trimmed, publish-clean `dist/package.json` with the synced version
// (see sync-version.mjs) and `exports` paths relative to the `dist/` publish
// root. Dev-only fields (devDependencies, scripts) are dropped. Also stages the
// package metadata files the manifest's `files` list declares (README, LICENSE,
// OWNERS) into `dist/`, since the publish runs from there.

import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { resolveVersion } from "./sync-version.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const packageDir = resolve(here, "..");
const repoRoot = resolve(packageDir, "..", "..");
const sourceManifestPath = join(packageDir, "package.json");
const distDir = join(packageDir, "dist");
const distManifestPath = join(distDir, "package.json");

// Metadata files the published `files` list references; LICENSE and OWNERS live
// at the monorepo root, README beside this package. Each must exist - a missing
// one is a build error, not a silently incomplete package.
const metadataFiles = [
  { from: join(packageDir, "README.md"), to: join(distDir, "README.md") },
  { from: join(repoRoot, "LICENSE"), to: join(distDir, "LICENSE") },
  { from: join(repoRoot, "OWNERS"), to: join(distDir, "OWNERS") },
  {
    from: join(packageDir, "src-ts", "wasm", "openpit_js.d.ts"),
    to: join(distDir, "types", "wasm", "openpit_js.d.ts"),
  },
];

const source = JSON.parse(readFileSync(sourceManifestPath, "utf8"));
const version = resolveVersion();

// The published manifest lives at the dist/ root, so every relative path in
// `exports`/`module`/`types`/`browser` is already written without a `dist/`
// prefix in the source manifest. We trim dev-only fields and inject the synced
// version.
//
// `imports` is re-declared (not copied) for the published layout: the source
// manifest maps `#runtime` to the TS initializer for the build, but the barrel
// `.d.ts` files reference `#runtime`, so consumers need it mapped to the emitted
// `runtime` entry (Node or browser). `scripts`/`devDependencies` are dropped.
const publishedImports = {
  "#runtime": {
    browser: {
      types: "./types/runtime.browser.d.ts",
      import: "./browser/runtime.js",
      require: "./browser/runtime.cjs",
    },
    types: "./types/runtime.node.d.ts",
    import: "./node/runtime.js",
    require: "./node/runtime.cjs",
  },
};

const published = {
  name: source.name,
  version,
  description: source.description,
  license: source.license,
  author: source.author,
  homepage: source.homepage,
  repository: source.repository,
  bugs: source.bugs,
  keywords: source.keywords,
  type: source.type,
  sideEffects: source.sideEffects,
  engines: source.engines,
  publishConfig: source.publishConfig,
  imports: publishedImports,
  exports: source.exports,
  main: source.main,
  module: source.module,
  types: source.types,
  browser: source.browser,
  files: source.files,
};

mkdirSync(distDir, { recursive: true });
writeFileSync(distManifestPath, `${JSON.stringify(published, null, 2)}\n`);
console.log(`[make-package-json] wrote ${distManifestPath} (v${version})`);

for (const { from, to } of metadataFiles) {
  if (!existsSync(from)) {
    throw new Error(
      `[make-package-json] required metadata file missing: ${from}`,
    );
  }
  mkdirSync(dirname(to), { recursive: true });
  copyFileSync(from, to);
  console.log(`[make-package-json] copied ${from} -> ${to}`);
}
