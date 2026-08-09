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

// Runs scenario tables through sequential and parallel engine configurations.
// `RunSync` uses a NoSync engine and NoSync market-data service in table order.
// `RunAsync` uses an AccountSync engine and FullSync market-data service; its
// typed async wrapper gives each account a serial queue, allowing different
// accounts to run in parallel while addressed TICKs fence their targets.
// Normal deadline and verdict-mismatch exits return partial reports.

#include "openpit/pretrade/decision.hpp"

#include <chrono>
#include <cstddef>
#include <map>
#include <optional>
#include <string>

#include "table.hpp"

namespace spot_table {

// A monotonic run deadline. RunSync checks it before each row; RunAsync checks
// it before submission and verdict waits, then releases submitted reservations.
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

// Runs table rows through NoSync components in order, replaying TICKs in place.
// Stops at the deadline or first verdict mismatch and returns a partial report.
// Throws `openpit::Error` if engine or market-data service setup fails and
// `FeedError` if a TICK instrument cannot be parsed or registered.
[[nodiscard]] Report RunSync(Deadline deadline, const Frontmatter &fm,
                             const std::vector<Row> &rows);

// Runs table rows through an AccountSync engine in the typed async wrapper and
// a FullSync market-data service. GROUP rows register first; non-TICK rows
// preserve per-account order while different accounts run in parallel.
// Addressed TICKs fence targets. Stops at the deadline or first verdict
// mismatch and returns a partial report. Throws `openpit::Error` on setup,
// `FeedError` if a TICK instrument cannot be parsed or registered, and
// `BuildError` if a SEED, ORDER, or FILL row cannot be translated. Unlike
// `RunSync`, the row-build `BuildError` escapes instead of becoming a
// `Failure`.
[[nodiscard]] Report RunAsync(Deadline deadline, const Frontmatter &fm,
                              const std::vector<Row> &rows);

// Returns the case-insensitive table name for `code`, or `Code(<value>)` when
// it is not in the table map.
[[nodiscard]] std::string CodeName(openpit::reject::RejectCode code);

} // namespace spot_table
