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
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { gunzipSync, gzipSync } from "node:zlib";

import { afterEach, describe, expect, it } from "vitest";

const normalizer = fileURLToPath(
  new URL("../scripts/normalize-typedoc-assets.mjs", import.meta.url),
);
const temporaryDirectories: string[] = [];

function makeAssetsDirectory(): string {
  const directory = mkdtempSync(join(tmpdir(), "openpit-typedoc-assets-"));
  temporaryDirectories.push(directory);
  return directory;
}

function writeAsset(
  directory: string,
  fileName: string,
  dataName: string,
  osCode: number,
  payload: string,
): void {
  const gzip = gzipSync(payload);
  gzip[9] = osCode;
  const semicolon = fileName === "search.js" ? ";" : "";
  writeFileSync(
    join(directory, fileName),
    `window.${dataName} = "data:application/octet-stream;base64,` +
      `${gzip.toString("base64")}"${semicolon}`,
  );
}

function readGzip(directory: string, fileName: string): Buffer {
  const text = readFileSync(join(directory, fileName), "utf8");
  const match = text.match(/base64,([A-Za-z0-9+/=]+)"/);
  const base64 = match?.[1];
  if (!base64) {
    throw new Error(`missing base64 payload in ${fileName}`);
  }
  return Buffer.from(base64, "base64");
}

function runNormalizer(directory: string): string {
  return execFileSync(process.execPath, [normalizer, directory], {
    encoding: "utf8",
  });
}

afterEach(() => {
  for (const directory of temporaryDirectories.splice(0)) {
    rmSync(directory, { force: true, recursive: true });
  }
});

describe("TypeDoc asset normalizer", () => {
  it("normalizes macOS and Unix gzip headers without changing content", () => {
    const directory = makeAssetsDirectory();
    writeAsset(
      directory,
      "navigation.js",
      "navigationData",
      19,
      '{"navigation":true}',
    );
    writeAsset(directory, "search.js", "searchData", 3, '{"search":true}');

    expect(runNormalizer(directory)).toContain("normalized 2 asset(s)");

    const navigation = readGzip(directory, "navigation.js");
    const search = readGzip(directory, "search.js");
    expect(navigation[9]).toBe(0xff);
    expect(search[9]).toBe(0xff);
    expect(gunzipSync(navigation).toString()).toBe('{"navigation":true}');
    expect(gunzipSync(search).toString()).toBe('{"search":true}');
  });

  it("is idempotent", () => {
    const directory = makeAssetsDirectory();
    writeAsset(directory, "navigation.js", "navigationData", 19, "nav");
    writeAsset(directory, "search.js", "searchData", 3, "search");
    runNormalizer(directory);
    const navigation = readFileSync(join(directory, "navigation.js"));
    const search = readFileSync(join(directory, "search.js"));

    expect(runNormalizer(directory)).toContain("normalized 0 asset(s)");
    expect(readFileSync(join(directory, "navigation.js"))).toEqual(navigation);
    expect(readFileSync(join(directory, "search.js"))).toEqual(search);
  });

  it("rejects an invalid gzip payload", () => {
    const directory = makeAssetsDirectory();
    writeFileSync(
      join(directory, "navigation.js"),
      'window.navigationData = "data:application/octet-stream;base64,bm90LWd6aXA="',
    );
    writeAsset(directory, "search.js", "searchData", 3, "search");

    const result = spawnSync(process.execPath, [normalizer, directory], {
      encoding: "utf8",
    });

    expect(result.status).toBe(1);
    expect(result.stderr).toContain("invalid gzip payload");
  });
});
