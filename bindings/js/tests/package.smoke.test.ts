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

import { execFileSync, spawnSync } from "node:child_process";
import {
  existsSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";

import { build } from "esbuild";
import { describe, expect, it } from "vitest";

const distDir = fileURLToPath(new URL("../dist/", import.meta.url));
const jsCargoToml = fileURLToPath(new URL("../Cargo.toml", import.meta.url));
const coreCargoToml = fileURLToPath(
  new URL("../../../crates/openpit/Cargo.toml", import.meta.url),
);
const packageSubpaths = [
  ".",
  "./param",
  "./model",
  "./pretrade",
  "./pretrade/policies",
  "./marketdata",
  "./reject",
  "./accountadjustment",
  "./accounts",
  "./tx",
] as const;

interface ExportConditions {
  browser: {
    import: string;
    require: string;
  };
  import: string;
  require: string;
  types: string;
}

interface PublishedManifest {
  exports: Record<string, ExportConditions | string>;
}

const commonJsSmoke = String.raw`
const assert = require("node:assert/strict");

const { Engine } = require("@openpit/engine");
const param = require("@openpit/engine/param");
const model = require("@openpit/engine/model");
const pretrade = require("@openpit/engine/pretrade");
const policies = require("@openpit/engine/pretrade/policies");
const marketdata = require("@openpit/engine/marketdata");
const reject = require("@openpit/engine/reject");
const accountadjustment = require("@openpit/engine/accountadjustment");
const accounts = require("@openpit/engine/accounts");
const tx = require("@openpit/engine/tx");

for (const entry of [
  param,
  model,
  pretrade,
  policies,
  marketdata,
  reject,
  accountadjustment,
  accounts,
  tx,
]) {
  assert.ok(Object.keys(entry).length > 0);
}

assert.equal(model.Trade, param.Trade);
assert.equal(model.TradeAmount, param.TradeAmount);

const engine = Engine.builder().builtin(policies.buildOrderValidation()).build();
assert.equal(typeof engine.startPreTrade, "function");
`;

const esmSmoke = String.raw`
import assert from "node:assert/strict";

import { Engine } from "@openpit/engine";
import * as param from "@openpit/engine/param";
import * as model from "@openpit/engine/model";
import * as pretrade from "@openpit/engine/pretrade";
import * as policies from "@openpit/engine/pretrade/policies";
import * as marketdata from "@openpit/engine/marketdata";
import * as reject from "@openpit/engine/reject";
import * as accountadjustment from "@openpit/engine/accountadjustment";
import * as accounts from "@openpit/engine/accounts";
import * as tx from "@openpit/engine/tx";

for (const entry of [
  param,
  model,
  pretrade,
  policies,
  marketdata,
  reject,
  accountadjustment,
  accounts,
  tx,
]) {
  assert.ok(Object.keys(entry).length > 0);
}

assert.equal(model.Trade, param.Trade);
assert.equal(model.TradeAmount, param.TradeAmount);

const engine = Engine.builder().builtin(policies.buildOrderValidation()).build();
assert.equal(typeof engine.startPreTrade, "function");
`;

const mixedNodeSmoke = String.raw`
import assert from "node:assert/strict";
import { createRequire } from "node:module";

import { Engine, ParamError, QuoteExpired } from "@openpit/engine";
import * as esmParam from "@openpit/engine/param";
import * as esmPolicies from "@openpit/engine/pretrade/policies";
import * as esmMarketData from "@openpit/engine/marketdata";

const require = createRequire(import.meta.url);
const cjsRoot = require("@openpit/engine");
const cjsParam = require("@openpit/engine/param");
const cjsPolicies = require("@openpit/engine/pretrade/policies");
const cjsMarketData = require("@openpit/engine/marketdata");

assert.equal(Engine, cjsRoot.Engine);
assert.equal(esmParam.Price, cjsParam.Price);
assert.equal(ParamError, esmParam.ParamError);
assert.equal(ParamError, cjsParam.ParamError);
assert.equal(QuoteExpired, esmMarketData.QuoteExpired);
assert.equal(QuoteExpired, cjsMarketData.QuoteExpired);
assert.equal(esmPolicies.buildOrderValidation, cjsPolicies.buildOrderValidation);
Engine.builder().builtin(cjsPolicies.buildOrderValidation()).build();
cjsRoot.Engine.builder().builtin(esmPolicies.buildOrderValidation()).build();
`;

function runNode(source: string, inputType?: "module"): void {
  const args =
    inputType === undefined
      ? ["--eval", source]
      : ["--input-type=module", "--eval", source];
  execFileSync(process.execPath, args, {
    cwd: distDir,
    encoding: "utf8",
    stdio: "pipe",
  });
}

function runBundledNode(source: string, inputType?: "module"): void {
  const args = inputType === undefined ? [] : ["--input-type=module"];
  execFileSync(process.execPath, args, {
    cwd: distDir,
    encoding: "utf8",
    input: source,
    stdio: "pipe",
  });
}

const declarationConsumer = String.raw`
import {
  Engine,
  PolicyCallbackError,
  type OpenpitError,
} from "@openpit/engine";
import {
  FillType,
  ParamKind,
  Price,
  RoundingStrategies,
  type RoundingStrategy,
} from "@openpit/engine/param";
import type { OrderInit } from "@openpit/engine/model";
import type { Policy, PolicyReject } from "@openpit/engine/pretrade";
import { buildOrderValidation } from "@openpit/engine/pretrade/policies";
import {
  MarketDataError,
  QuoteExpired,
  QuoteTtl,
} from "@openpit/engine/marketdata";
import { RejectCode, RejectScope } from "@openpit/engine/reject";

const strategy: RoundingStrategy = RoundingStrategies.Up;
Price.fromStringRounded("1.001", 2, strategy);
const reject: PolicyReject = {
  code: RejectCode.Custom,
  reason: "consumer smoke",
  details: "",
  scope: RejectScope.Order,
};
const policy: Policy = {
  name: "consumer-smoke",
  checkPreTradeStart: () => [reject],
  performPreTradeCheck: () => ({}),
};
const order: OrderInit = {};
void order;
void policy;
void FillType.Trade;
void ParamKind.Price;
void QuoteTtl.infinite();
void MarketDataError;
void QuoteExpired;
void PolicyCallbackError;
void (undefined as OpenpitError | undefined);
Engine.builder().builtin(buildOrderValidation()).build();
`;

function runDeclarationConsumer(
  moduleResolution: "NodeNext" | "Bundler",
): void {
  const directory = mkdtempSync(join(distDir, ".types-smoke-"));
  try {
    writeFileSync(join(directory, "consumer.ts"), declarationConsumer);
    writeFileSync(
      join(directory, "tsconfig.json"),
      JSON.stringify({
        compilerOptions: {
          target: "ES2022",
          module: moduleResolution === "NodeNext" ? "NodeNext" : "ESNext",
          moduleResolution,
          strict: true,
          skipLibCheck: false,
          noEmit: true,
        },
        files: ["consumer.ts"],
      }),
    );
    const tsc = fileURLToPath(
      new URL("../node_modules/typescript/bin/tsc", import.meta.url),
    );
    const result = spawnSync(
      process.execPath,
      [tsc, "--project", "tsconfig.json"],
      {
        cwd: directory,
        encoding: "utf8",
        env: {
          ...process.env,
          NODE_PATH: fileURLToPath(new URL("../node_modules", import.meta.url)),
        },
      },
    );
    if (result.error !== undefined) {
      throw result.error;
    }
    if (result.status !== 0) {
      throw new Error(
        `declaration consumer failed under ${moduleResolution}:\n${result.stdout}${result.stderr}`,
      );
    }
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
}

async function buildBrowserConsumer(format: "esm" | "cjs"): Promise<string> {
  const source =
    format === "esm"
      ? String.raw`
import { Engine } from "@openpit/engine";
import { buildOrderValidation } from "@openpit/engine/pretrade/policies";
const engine = Engine.builder().builtin(buildOrderValidation()).build();
if (typeof engine.startPreTrade !== "function") throw new Error("browser ESM init failed");
`
      : String.raw`
const { Engine } = require("@openpit/engine");
const { buildOrderValidation } = require("@openpit/engine/pretrade/policies");
const engine = Engine.builder().builtin(buildOrderValidation()).build();
if (typeof engine.startPreTrade !== "function") throw new Error("browser CJS init failed");
`;
  const result = await build({
    absWorkingDir: distDir,
    bundle: true,
    conditions: ["browser", format === "esm" ? "import" : "require"],
    format,
    platform: "browser",
    target: "es2022",
    write: false,
    stdin: {
      contents: source,
      resolveDir: distDir,
      sourcefile: `browser-consumer.${format === "esm" ? "mjs" : "cjs"}`,
    },
  });
  const output = result.outputFiles[0]?.text;
  if (output === undefined) {
    throw new Error("esbuild produced no browser consumer output");
  }
  return output;
}

describe("published package module formats", () => {
  it("keeps the JS crate and published npm version in workspace lockstep", () => {
    const cargoVersion = (path: string): string => {
      const match = readFileSync(path, "utf8").match(
        /^version\s*=\s*"([^"]+)"/m,
      );
      if (match?.[1] === undefined) {
        throw new Error(`missing Cargo package version in ${path}`);
      }
      return match[1];
    };
    const jsVersion = cargoVersion(jsCargoToml);
    expect(jsVersion).toBe(cargoVersion(coreCargoToml));
    const manifest = JSON.parse(
      readFileSync(join(distDir, "package.json"), "utf8"),
    ) as { version: string };
    expect(manifest.version).toBe(jsVersion);
  });

  it("maps every code subpath to existing Node and browser builds", () => {
    const manifest = JSON.parse(
      readFileSync(join(distDir, "package.json"), "utf8"),
    ) as PublishedManifest;

    for (const subpath of packageSubpaths) {
      const conditions = manifest.exports[subpath];
      expect(conditions).not.toBeUndefined();
      expect(typeof conditions).toBe("object");
      if (typeof conditions === "string" || conditions === undefined) {
        continue;
      }
      for (const target of [
        conditions.types,
        conditions.import,
        conditions.require,
        conditions.browser.import,
        conditions.browser.require,
      ]) {
        expect(existsSync(join(distDir, target))).toBe(true);
      }
    }

    expect(existsSync(join(distDir, "types", "wasm", "openpit_js.d.ts"))).toBe(
      true,
    );
  });

  it("packs only the publishable runtime, declarations, and metadata", () => {
    const npmCache = mkdtempSync(join(tmpdir(), "openpit-npm-cache-"));
    let packed: Array<{ files: Array<{ path: string }>; version: string }>;
    try {
      packed = JSON.parse(
        execFileSync("npm", ["pack", "--dry-run", "--json"], {
          cwd: distDir,
          encoding: "utf8",
          env: { ...process.env, npm_config_cache: npmCache },
          stdio: "pipe",
        }),
      ) as Array<{ files: Array<{ path: string }>; version: string }>;
    } finally {
      rmSync(npmCache, { recursive: true, force: true });
    }
    const metadata = packed[0];
    if (metadata === undefined) {
      throw new Error("npm pack returned no package metadata");
    }
    const files = metadata.files.map(({ path }) => path);
    expect(files).toContain("node/index.cjs");
    expect(files).toContain("node/node-cjs-shared.cjs");
    expect(files).not.toContain("node/node-cjs-shared.js");
    expect(
      files.some((path) => /^node\/chunk-.*\.js(?:\.map)?$/.test(path)),
    ).toBe(false);
    expect(
      files.some((path) => /^browser\/chunk-.*\.cjs(?:\.map)?$/.test(path)),
    ).toBe(false);
    expect(files).toContain("browser/index.js");
    expect(files).toContain("types/index.d.ts");
    expect(files).toContain("README.md");
    for (const subpath of packageSubpaths) {
      const entry = subpath === "." ? "index" : subpath.slice(2);
      expect(files).not.toContain(`node/${entry}.js.map`);
      expect(files).not.toContain(`node/${entry}.cjs.map`);
      expect(files).not.toContain(`browser/${entry}.cjs.map`);
      expect(files).toContain(`browser/${entry}.js.map`);
    }
    expect(files).not.toContain("node/runtime.js.map");
    expect(files).not.toContain("node/runtime.cjs.map");
    expect(files).not.toContain("browser/runtime.cjs.map");
    expect(files).toContain("browser/runtime.js.map");
    expect(files.some((path) => path.startsWith("src-ts/"))).toBe(false);
    expect(files).not.toContain("Cargo.toml");

    const manifest = JSON.parse(
      readFileSync(join(distDir, "package.json"), "utf8"),
    ) as { version: string };
    expect(metadata.version).toBe(manifest.version);
  });

  it("loads the root and every subpath through CommonJS require", () => {
    runNode(commonJsSmoke);
  });

  it("loads the root and every subpath through ESM import", () => {
    runNode(esmSmoke, "module");
  });

  it("shares one Node wasm graph across import and require", () => {
    runNode(mixedNodeSmoke, "module");
  });

  it.each(["NodeNext", "Bundler"] as const)(
    "resolves published declarations under %s",
    (moduleResolution) => {
      runDeclarationConsumer(moduleResolution);
    },
  );

  it.each(["esm", "cjs"] as const)(
    "bundles and executes the browser %s condition",
    async (format) => {
      const output = await buildBrowserConsumer(format);
      expect(output).not.toContain("node:fs");
      runBundledNode(output, format === "esm" ? "module" : undefined);
    },
  );
});
