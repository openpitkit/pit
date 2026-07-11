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

// Tests for the spot_table example.
//
// testFast runs the shared coverage scenario once and asserts every verdict
// against the single sequential engine.

import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vitest";

import { runSync } from "./runner.ts";
import { parseFile } from "./table.ts";

const HERE = dirname(fileURLToPath(import.meta.url));

// The shared scenario lives outside the example so one file backs the CLI, the
// tests, and the just targets.
const COVERAGE_TABLE = resolve(HERE, "..", "..", "tables", "spot", "coverage.md");

describe("spot_table", () => {
  it("passes every verdict in the coverage scenario", () => {
    const table = parseFile(COVERAGE_TABLE);
    const report = runSync(table.fm, table.rows, null);

    if (report.firstFail !== null) {
      const fail = report.firstFail;
      expect.fail(
        `verdict mismatch at line ${fail.row.line}` +
          ` (${fail.row.account} ${fail.row.action}): ${fail.message}`,
      );
    }
    expect(report.total, `zero executable rows in ${COVERAGE_TABLE}`).toBeGreaterThan(0);
  });
});
