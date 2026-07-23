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

#pragma once

//
// `RunSync` drives a single-threaded NoSync engine + NoSync market-data service
// operation by operation in row order. `RunAsync` drives an AccountSync engine
// wrapped in the typed async engine (Dynamic strategy, one serial queue per
// account) + FullSync market-data service: per-account operations serialize
// while different accounts run in parallel. Each stops at the first verdict
// mismatch and returns a partial report.
//
// `std::chrono::steady_clock` time point (`Deadline`); `RunSync` / `RunAsync`
// `ctx.Err()`.

#include "openpit/pretrade/decision.hpp"

#include <chrono>
#include <cstddef>
#include <map>
#include <optional>
#include <string>

#include "table.hpp"

namespace spot_table {

// `context.Context` timeout.
using Deadline = std::chrono::steady_clock::time_point;

enum class Mode {
  Sync,
  Async,
};

struct Failure {
  Row row;
  std::string message;
};

struct LatencyStats {
  std::size_t count = 0;
  std::chrono::nanoseconds total{0};
  std::chrono::nanoseconds min{0};
  std::chrono::nanoseconds max{0};

  void Observe(std::chrono::nanoseconds d);

  [[nodiscard]] std::chrono::nanoseconds Avg() const;

  // Folds another sample set in, for aggregating across repeat iterations.
  void Merge(const LatencyStats &o);
};

struct Report {
  Mode mode = Mode::Sync;
  // Executable rows (SEED/GROUP/ORDER/FILL; excludes TICK).
  int total = 0;
  // Account label -> row count.
  std::map<std::string, int> accounts;
  std::chrono::nanoseconds wallClock{0};
  LatencyStats order;
  LatencyStats fill;
  std::optional<Failure> firstFail;

  [[nodiscard]] int AccountsCount() const {
    return static_cast<int>(accounts.size());
  }
};

// Executes the table in Mode A: NoSync engine, strictly
// operation-by-operation. TICK rows are replayed live at their row position.
// Stops at the first verdict mismatch and returns a partial report. Throws on a
[[nodiscard]] Report RunSync(Deadline deadline, const Frontmatter &fm,
                             const std::vector<Row> &rows);

// Executes the table in Mode B: AccountSync engine wrapped in the typed async
// engine. GROUP rows are registered first; non-TICK rows are submitted in row
// order and run per-account-serially / cross-account-parallel; addressed TICKs
// fence their targets. Verdicts are awaited in row order, stopping on the first
[[nodiscard]] Report RunAsync(Deadline deadline, const Frontmatter &fm,
                              const std::vector<Row> &rows);

// The case-insensitive reject-code name from a reject Code, used in
[[nodiscard]] std::string CodeName(openpit::reject::RejectCode code);

} // namespace spot_table
