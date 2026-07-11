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

import { Buffer } from "node:buffer";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { gunzipSync } from "node:zlib";

const gzipHeaderLength = 10;
const gzipOsOffset = 9;
const unknownOs = 0xff;
const assets = [
  ["navigation.js", "navigationData"],
  ["search.js", "searchData"],
];

const here = dirname(fileURLToPath(import.meta.url));
const defaultAssetsDir = resolve(
  here,
  "..",
  "..",
  "..",
  "docs",
  "js-api",
  "assets",
);

function normalizeAsset(path, dataName) {
  const text = readFileSync(path, "utf8");
  const pattern = new RegExp(
    `^(window\\.${dataName} = "data:application/octet-stream;base64,)` +
      `([A-Za-z0-9+/=]+)(";?\\r?\\n?)$`,
  );
  const match = text.match(pattern);
  if (!match) {
    throw new Error(`unexpected TypeDoc asset format: ${path}`);
  }

  const gzip = Buffer.from(match[2], "base64");
  if (
    gzip.length < gzipHeaderLength ||
    gzip[0] !== 0x1f ||
    gzip[1] !== 0x8b ||
    gzip[2] !== 0x08
  ) {
    throw new Error(`invalid gzip payload: ${path}`);
  }
  if ((gzip[3] & 0x02) !== 0) {
    throw new Error(`gzip header CRC is not supported: ${path}`);
  }

  gunzipSync(gzip);
  if (gzip[gzipOsOffset] === unknownOs) {
    return false;
  }

  gzip[gzipOsOffset] = unknownOs;
  writeFileSync(path, `${match[1]}${gzip.toString("base64")}${match[3]}`);
  return true;
}

function normalizeAssets(assetsDir) {
  let normalized = 0;
  for (const [fileName, dataName] of assets) {
    const path = resolve(assetsDir, fileName);
    normalized += Number(normalizeAsset(path, dataName));
  }
  return normalized;
}

const assetsDir = process.argv[2] ?? defaultAssetsDir;
try {
  const normalized = normalizeAssets(assetsDir);
  console.log(
    `[normalize-typedoc-assets] normalized ${normalized} asset(s) in ${assetsDir}`,
  );
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  console.error(`[normalize-typedoc-assets] ${message}`);
  process.exitCode = 1;
}
