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
// Please see https://github.com/openpitkit and the OWNERS file for details.

package main

import (
	"context"
	"testing"
	"time"
)

// coverageTable is the scenario both tests run; it lives outside the module so
// the same file backs the CLI, the tests, and the just targets.
const coverageTable = "../../tables/spot/coverage.md"

// TestFast is the quick check: it runs the coverage scenario through both
// engines once and asserts every row's verdict. The scenario uses every feature
// of the runner, so a green run covers the whole tool end to end in well under a
// second.
func TestFast(t *testing.T) {
	table, err := ParseFile(coverageTable)
	if err != nil {
		t.Fatalf("parse %s: %v", coverageTable, err)
	}
	assertScenario(t, table, defaultTimeout)
}

// assertScenario runs the table on both engines, fails on any transport error
// or verdict mismatch, and returns both engines' reports so a caller (the
// repeat test) can aggregate statistics.
func assertScenario(t *testing.T, table *Table, timeout time.Duration) (syncReport, asyncReport *Report) {
	t.Helper()
	ctx, cancel := context.WithTimeout(context.Background(), timeout)
	defer cancel()
	syncReport = runAndAssert(ctx, t, "sync", table, RunSync)
	asyncReport = runAndAssert(ctx, t, "async", table, RunAsync)
	return syncReport, asyncReport
}

func runAndAssert(
	ctx context.Context,
	t *testing.T,
	name string,
	table *Table,
	run func(context.Context, Frontmatter, []Row) (*Report, error),
) *Report {
	t.Helper()
	report, err := run(ctx, table.FM, table.Rows)
	if err != nil {
		t.Fatalf("[%s] run: %v", name, err)
	}
	if report.FirstFail != nil {
		f := report.FirstFail
		t.Fatalf("[%s] verdict mismatch at line %d (%s %s): %s",
			name, f.Row.Line, f.Row.Account, f.Row.Action, f.Message)
	}
	if report.Total == 0 {
		t.Fatalf("[%s] zero executable rows in %s", name, coverageTable)
	}
	return report
}
