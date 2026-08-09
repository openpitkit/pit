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

// Runs the coverage scenario through both engines and asserts every row's
// verdict.
// The scenario uses every feature of the runner, so a green run covers the
// whole tool end to end in well under a second.

#include "runner.hpp"
#include "table.hpp"

#include <gtest/gtest.h>

#include <chrono>
#include <functional>
#include <string>
#include <vector>

namespace {

using spot_table::Frontmatter;
using spot_table::Report;
using spot_table::Row;
using spot_table::Table;

// Bounds a scenario pass with the CLI's default timeout.
constexpr std::chrono::seconds kDefaultTimeout{30};

// Returns the CMake-injected absolute path to the coverage scenario.
const char *CoverageTable() { return SPOT_TABLE_COVERAGE_PATH; }

// Runs one engine and records test failures for exceptions or verdict
// mismatches.
Report RunAndAssert(
    spot_table::Deadline deadline, const std::string &name, const Table &table,
    const std::function<Report(spot_table::Deadline, const Frontmatter &,
                               const std::vector<Row> &)> &run) {
  Report report;
  try {
    report = run(deadline, table.fm, table.rows);
  } catch (const std::exception &err) {
    ADD_FAILURE() << "[" << name << "] run: " << err.what();
    return report;
  }
  if (report.firstFail.has_value()) {
    const spot_table::Failure &f = *report.firstFail;
    ADD_FAILURE() << "[" << name << "] verdict mismatch at line " << f.row.line
                  << " (" << f.row.account << " " << f.row.action
                  << "): " << f.message;
  }
  EXPECT_GT(report.total, 0)
      << "[" << name << "] zero executable rows in " << CoverageTable();
  return report;
}

// Runs the coverage table through both engine modes with a shared deadline.
void AssertScenario(const Table &table, std::chrono::nanoseconds timeout) {
  const spot_table::Deadline deadline =
      std::chrono::steady_clock::now() + timeout;
  RunAndAssert(deadline, "sync", table, spot_table::RunSync);
  RunAndAssert(deadline, "async", table, spot_table::RunAsync);
}

// Runs the coverage scenario through both engines once.
TEST(SpotTable, Fast) {
  Table table;
  ASSERT_NO_THROW({ table = spot_table::ParseFile(CoverageTable()); })
      << "parse " << CoverageTable();
  AssertScenario(table, kDefaultTimeout);
}

} // namespace
