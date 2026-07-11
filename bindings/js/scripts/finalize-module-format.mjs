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
// Keeps one wasm module graph when consumers mix ESM and CommonJS subpaths.
// Node uses one CJS aggregation graph as canonical because ESM can re-export
// CJS synchronously. Browser bundlers use the split ESM graph as canonical
// because they can statically bundle a CJS require of ESM, while native
// browsers cannot load CommonJS directly. Dead bundles/chunks from the other
// graph are removed before publication.

import { createRequire } from "node:module";
import {
  existsSync,
  readdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { dirname, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { copyNodeWasm } from "./copy-node-wasm.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const packageDir = resolve(here, "..");
const distDir = resolve(packageDir, "dist");
const manifest = JSON.parse(
  readFileSync(resolve(packageDir, "package.json"), "utf8"),
);

function relativeSpecifier(from, to) {
  const specifier = relative(dirname(from), to);
  return specifier.startsWith(".") ? specifier : `./${specifier}`;
}

function removeStaleSourceMap(output) {
  // tsup generated the map for the bundle that occupied this path before the
  // public entry was rewritten as a wrapper. Keeping it in the published
  // directory would expose misleading source mappings for code that no longer
  // exists, even though the wrapper itself has no sourceMappingURL comment.
  rmSync(`${output}.map`, { force: true });
}

function removeDeadNodeEsmArtifacts(publicEntries) {
  const nodeDir = resolve(distDir, "node");
  const retained = new Set(
    publicEntries.map((entry) => resolve(distDir, entry)),
  );

  function visit(directory) {
    for (const item of readdirSync(directory, { withFileTypes: true })) {
      const path = resolve(directory, item.name);
      if (item.isDirectory()) {
        visit(path);
      } else if (path.endsWith(".js.map")) {
        // Every retained Node ESM file is now a source-less wrapper; all other
        // ESM maps belong to bundles/chunks that are no longer reachable.
        rmSync(path, { force: true });
      } else if (path.endsWith(".js") && !retained.has(path)) {
        // The public ESM wrappers import only the canonical CJS graph, so the
        // original ESM aggregator and split chunks would otherwise be dead
        // duplicate wasm code in the tarball.
        rmSync(path, { force: true });
      }
    }
  }

  visit(nodeDir);
}

function removeDeadBrowserCjsArtifacts(publicEntries) {
  const browserDir = resolve(distDir, "browser");
  const retained = new Set(
    publicEntries.map((entry) => resolve(distDir, entry)),
  );

  function visit(directory) {
    for (const item of readdirSync(directory, { withFileTypes: true })) {
      const path = resolve(directory, item.name);
      if (item.isDirectory()) {
        visit(path);
      } else if (path.endsWith(".cjs.map")) {
        rmSync(path, { force: true });
      } else if (path.endsWith(".cjs") && !retained.has(path)) {
        // Browser CommonJS entries now delegate to the ESM graph, so their
        // original split chunks are unreachable duplicate wasm code.
        rmSync(path, { force: true });
      }
    }
  }

  visit(browserDir);
}

function writeBrowserCjsWrapper(outputTarget, canonicalTarget) {
  const output = resolve(distDir, outputTarget);
  const canonical = resolve(distDir, canonicalTarget);
  if (!existsSync(output) || !existsSync(canonical)) {
    throw new Error(`module-format output missing: ${output} or ${canonical}`);
  }

  const specifier = relativeSpecifier(output, canonical);
  writeFileSync(
    output,
    `"use strict";\nmodule.exports = require(${JSON.stringify(specifier)});\n`,
  );
  removeStaleSourceMap(output);
}

function namespaceName(outputTarget) {
  const entry = outputTarget.replace(/^\.\/node\//, "").replace(/\.cjs$/, "");
  if (entry === "index") {
    return "root";
  }
  if (entry === "pretrade/policies") {
    return "pretradePolicies";
  }
  return entry;
}

function writeNodeCjsWrapper(outputTarget, canonicalTarget, namespace) {
  const output = resolve(distDir, outputTarget);
  const canonical = resolve(distDir, canonicalTarget);
  if (!existsSync(output) || !existsSync(canonical)) {
    throw new Error(`module-format output missing: ${output} or ${canonical}`);
  }
  const specifier = relativeSpecifier(output, canonical);
  writeFileSync(
    output,
    `"use strict";\nmodule.exports = require(${JSON.stringify(specifier)})[${JSON.stringify(namespace)}];\n`,
  );
  removeStaleSourceMap(output);
}

function writeNodeEsmWrapper(
  outputTarget,
  canonicalTarget,
  namespace,
  exportNames,
) {
  const output = resolve(distDir, outputTarget);
  const canonical = resolve(distDir, canonicalTarget);
  if (!existsSync(output) || !existsSync(canonical)) {
    throw new Error(`module-format output missing: ${output} or ${canonical}`);
  }
  const specifier = relativeSpecifier(output, canonical);
  const lines = [
    `import shared from ${JSON.stringify(specifier)};`,
    `const namespace = shared[${JSON.stringify(namespace)}];`,
  ];
  for (const name of exportNames) {
    if (name === "default") {
      lines.push("export default namespace.default;");
      continue;
    }
    if (!/^[$A-Z_a-z][$\w]*$/.test(name)) {
      throw new Error(
        `unsupported ESM export name in ${outputTarget}: ${name}`,
      );
    }
    lines.push(`export const ${name} = namespace[${JSON.stringify(name)}];`);
  }
  writeFileSync(output, `${lines.join("\n")}\n`);
  removeStaleSourceMap(output);
}

function finalizeNode() {
  copyNodeWasm();
  const sharedTarget = "./node/node-cjs-shared.cjs";
  const sharedPath = resolve(distDir, sharedTarget);
  if (!existsSync(sharedPath)) {
    throw new Error(`module-format output missing: ${sharedPath}`);
  }
  const require = createRequire(import.meta.url);
  const shared = require(sharedPath);
  const publicEsmEntries = [];
  for (const conditions of Object.values(manifest.exports)) {
    if (typeof conditions === "string") {
      continue;
    }
    const namespace = namespaceName(conditions.require);
    const namespaceExports = shared[namespace];
    if (namespaceExports === undefined) {
      throw new Error(`shared CJS namespace missing: ${namespace}`);
    }
    const exportNames = Object.keys(namespaceExports).filter(
      (name) => name !== "__esModule",
    );
    publicEsmEntries.push(conditions.import);
    writeNodeCjsWrapper(conditions.require, sharedTarget, namespace);
    writeNodeEsmWrapper(
      conditions.import,
      sharedTarget,
      namespace,
      exportNames,
    );
  }
  const runtimeExports = shared.runtime;
  if (runtimeExports === undefined) {
    throw new Error("shared CJS namespace missing: runtime");
  }
  writeNodeCjsWrapper("./node/runtime.cjs", sharedTarget, "runtime");
  writeNodeEsmWrapper(
    "./node/runtime.js",
    sharedTarget,
    "runtime",
    Object.keys(runtimeExports).filter((name) => name !== "__esModule"),
  );
  publicEsmEntries.push("./node/runtime.js");
  removeDeadNodeEsmArtifacts(publicEsmEntries);
}

function finalizeBrowser() {
  const publicCjsEntries = [];
  for (const conditions of Object.values(manifest.exports)) {
    if (typeof conditions === "string") {
      continue;
    }
    publicCjsEntries.push(conditions.browser.require);
    writeBrowserCjsWrapper(
      conditions.browser.require,
      conditions.browser.import,
    );
  }
  writeBrowserCjsWrapper("./browser/runtime.cjs", "./browser/runtime.js");
  publicCjsEntries.push("./browser/runtime.cjs");
  removeDeadBrowserCjsArtifacts(publicCjsEntries);
}

const platform = process.argv[2];
if (platform === "node") {
  finalizeNode();
} else if (platform === "browser") {
  finalizeBrowser();
} else {
  throw new Error("expected module-format platform: node or browser");
}

console.log(`[finalize-module-format] finalized ${platform} entries`);
