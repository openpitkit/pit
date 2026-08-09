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

// Example spot_table runs a tabular spot-policy scenario against the engine in
// two isolated runs - a sequential NoSync engine and a parallel AccountSync
// engine wrapped in the typed async engine - and prints a per-engine summary
// report with operation counts, total wall-clock time, and order/report latency
// statistics. With --min-duration d it repeats the scenario until at least d of
// wall-clock has elapsed (a repeat run), printing a periodic progress block
// with each engine's running order/report latency, then a final per-engine
// aggregate summary. The scenario tables live under examples/tables/spot/.

#include "duration.hpp"
#include "platform.hpp"
#include "runner.hpp"
#include "table.hpp"

#include <chrono>
#include <cstdlib>
#include <ctime>
#include <fstream>
#include <future>
#include <iostream>
#include <optional>
#include <string>
#include <utility>

#if defined(_WIN32)
#include <direct.h>
#define getcwd _getcwd
#else
#include <unistd.h>
#endif

namespace {

using namespace std::chrono_literals;
using spot_table::Mode;
using spot_table::Report;

// Default per-run deadline.
constexpr std::chrono::seconds kDefaultTimeout{30};

// Interval between repeat-run progress reports.
constexpr std::chrono::seconds kRepeatLogInterval{10};

//------------------------------------------------------------------------------

[[nodiscard]] bool FileExists(const std::string &path) {
  std::ifstream f(path);
  return f.good();
}

[[nodiscard]] std::string CurrentDir() {
  char buffer[4096];
  if (getcwd(buffer, sizeof(buffer)) != nullptr) {
    return std::string(buffer);
  }
  return "?";
}

// Resolves table paths directly or relative to the running binary's directory.
[[nodiscard]] std::optional<std::string>
ResolveTablePath(const std::string &argv0, const std::string &p) {
  if (FileExists(p)) {
    return p;
  }
  // Best-effort sibling-of-binary lookup using argv[0]'s directory.
  const std::string::size_type slash = argv0.find_last_of("/\\");
  if (slash != std::string::npos) {
    std::string next = argv0.substr(0, slash + 1) + p;
    if (FileExists(next)) {
      return next;
    }
  }
  return std::nullopt;
}

//------------------------------------------------------------------------------
// Report formatting and output.

[[nodiscard]] std::string EngineTitle(Mode m) {
  switch (m) {
  case Mode::Sync:
    return "sequential engine (sync)";
  case Mode::Async:
    return "parallel engine (async, one queue per account)";
  }
  return "unknown";
}

void PrintLegend() {
  std::cout << "Legend:\n";
  std::cout << "  operations  - table rows applied to the engine "
               "(SEED/GROUP/ORDER/FILL; market-data ticks excluded)\n";
  std::cout << "  accounts    - distinct accounts touched by the scenario\n";
  std::cout << "  total time  - wall-clock to run the whole scenario on this "
               "engine\n";
  std::cout << "  order check - time to decide one order (the pre-trade "
               "ACCEPT/REJECT check); n = orders checked\n";
  std::cout << "  reports     - time to apply one fill / execution report; n = "
               "reports applied\n";
  std::cout << "  parallel-engine times are the full submit-to-result "
               "round-trip: they include\n";
  std::cout << "  async dispatch (the per-account worker handoff) and any "
               "queue wait, while the\n";
  std::cout << "  sequential engine times only the direct call - so the two "
               "are not comparable.\n";
  std::cout << "\n";
}

void PrintLatency(const std::string &label, const spot_table::LatencyStats &s) {
  if (s.count == 0) {
    std::cout << label << ": none\n";
    return;
  }
  std::cout << label << ": n=" << s.count
            << "  min=" << spot_table::FormatDuration(s.min)
            << "  avg=" << spot_table::FormatDuration(s.Avg())
            << "  max=" << spot_table::FormatDuration(s.max) << "\n";
}

// Prints one engine report and its latency summary.
void PrintReport(const Report &r) {
  std::cout << "== " << EngineTitle(r.mode) << " ==\n";
  std::cout << "  operations  : " << r.total << "\n";
  std::cout << "  accounts    : " << r.AccountsCount() << "\n";
  std::cout << "  total time  : " << spot_table::FormatDuration(r.wallClock)
            << "\n";
  PrintLatency("  order check ", r.order);
  PrintLatency("  reports     ", r.fill);
  if (r.firstFail.has_value()) {
    const spot_table::Failure &f = *r.firstFail;
    std::cout << "  result      : FAILED at line " << f.row.line << " ("
              << f.row.account << ", " << f.row.action << "): " << f.message
              << "\n\n";
    return;
  }
  std::cout << "  result      : ALL PASS\n";
  std::cout << "\n";
}

//------------------------------------------------------------------------------

// Accumulates report totals and latencies across repeat iterations.
struct EngineAggregate {
  Mode mode = Mode::Sync;
  int accounts = 0;
  int ops = 0;
  spot_table::LatencyStats order;
  spot_table::LatencyStats fill;

  void Add(const std::optional<Report> &r) {
    if (!r.has_value()) {
      return;
    }
    mode = r->mode;
    accounts = r->AccountsCount();
    ops += r->total;
    order.Merge(r->order);
    fill.Merge(r->fill);
  }
};

void PrintAggregate(const EngineAggregate &a,
                    std::chrono::nanoseconds elapsed) {
  std::cout << "== " << EngineTitle(a.mode) << " ==\n";
  std::cout << "  operations  : " << a.ops << " total across the repeat run\n";
  std::cout << "  accounts    : " << a.accounts << "\n";
  std::cout << "  total time  : "
            << spot_table::FormatDuration(
                   spot_table::RoundDuration(elapsed, 1ms))
            << " (whole repeat run)\n";
  PrintLatency("  order check ", a.order);
  PrintLatency("  reports     ", a.fill);
  std::cout << "\n";
}

void PrintHeartbeatEngine(const std::string &label, const EngineAggregate &a) {
  std::cout << "  " << label << " · ord "
            << spot_table::FormatDuration(a.order.min) << "/"
            << spot_table::FormatDuration(a.order.Avg()) << "/"
            << spot_table::FormatDuration(a.order.max) << " · rpt "
            << spot_table::FormatDuration(a.fill.min) << "/"
            << spot_table::FormatDuration(a.fill.Avg()) << "/"
            << spot_table::FormatDuration(a.fill.max) << "\n";
}

void PrintHeartbeat(std::chrono::system_clock::time_point now, int iterations,
                    std::chrono::nanoseconds elapsed,
                    std::chrono::nanoseconds minDuration,
                    const EngineAggregate &syncAgg,
                    const EngineAggregate &asyncAgg) {
  std::chrono::nanoseconds left = minDuration - elapsed;
  if (left < std::chrono::nanoseconds(0)) {
    left = std::chrono::nanoseconds(0);
  }
  const std::time_t t = std::chrono::system_clock::to_time_t(now);
  std::tm tm{};
#if defined(_WIN32)
  localtime_s(&tm, &t);
#else
  localtime_r(&t, &tm);
#endif
  char clock[16];
  std::strftime(clock, sizeof(clock), "%H:%M:%S", &tm);
  std::cout << "── " << clock << " · " << iterations << " iter · elapsed "
            << spot_table::FormatDuration(
                   spot_table::RoundDuration(elapsed, 1s))
            << " · left "
            << spot_table::FormatDuration(spot_table::RoundDuration(left, 1s))
            << " ──\n";
  PrintHeartbeatEngine("sync ", syncAgg);
  PrintHeartbeatEngine("async", asyncAgg);
}

//------------------------------------------------------------------------------

struct PairResult {
  std::optional<Report> syncReport;
  std::optional<Report> asyncReport;
  std::optional<std::string> syncErr;
  std::optional<std::string> asyncErr;
};

// Runs both engines concurrently and captures each report or exception.
[[nodiscard]] PairResult RunPair(spot_table::Deadline deadline,
                                 const spot_table::Table &table) {
  auto syncFuture = std::async(std::launch::async, [&]() {
    return spot_table::RunSync(deadline, table.fm, table.rows);
  });
  auto asyncFuture = std::async(std::launch::async, [&]() {
    return spot_table::RunAsync(deadline, table.fm, table.rows);
  });

  PairResult out;
  try {
    out.syncReport = syncFuture.get();
  } catch (const std::exception &err) {
    out.syncErr = err.what();
  }
  try {
    out.asyncReport = asyncFuture.get();
  } catch (const std::exception &err) {
    out.asyncErr = err.what();
  }
  return out;
}

// Returns a failure message when an engine errors or reports a verdict
// mismatch.
[[nodiscard]] std::optional<std::string> Verdict(const PairResult &pair) {
  if (pair.syncErr.has_value() || pair.asyncErr.has_value()) {
    return "one or more engines errored";
  }
  if ((pair.syncReport.has_value() && pair.syncReport->firstFail.has_value()) ||
      (pair.asyncReport.has_value() &&
       pair.asyncReport->firstFail.has_value())) {
    return "scenario failed";
  }
  return std::nullopt;
}

[[nodiscard]] std::string BaseName(const std::string &path) {
  const std::string::size_type slash = path.find_last_of("/\\");
  return slash == std::string::npos ? path : path.substr(slash + 1);
}

// Runs the scenario once on both engines and prints the per-engine summary.
[[nodiscard]] std::optional<std::string>
RunOnce(const std::string &tablePath, const spot_table::Table &table,
        std::chrono::nanoseconds timeout) {
  spot_table::PrintPlatform();
  const spot_table::Deadline deadline =
      std::chrono::steady_clock::now() + timeout;
  const PairResult pair = RunPair(deadline, table);

  std::cout << "Scenario: " << table.fm.name << " (" << BaseName(tablePath)
            << "), slippage " << table.fm.slippageBps << " bps\n\n";
  PrintLegend();
  if (pair.syncErr.has_value()) {
    std::cout << "sequential engine error: " << *pair.syncErr << "\n";
  }
  if (pair.asyncErr.has_value()) {
    std::cout << "parallel engine error: " << *pair.asyncErr << "\n";
  }
  if (pair.syncReport.has_value()) {
    PrintReport(*pair.syncReport);
  }
  if (pair.asyncReport.has_value()) {
    PrintReport(*pair.asyncReport);
  }
  return Verdict(pair);
}

void PrintRepeatSummary(int iterations, std::chrono::nanoseconds elapsed,
                        const EngineAggregate &syncAgg,
                        const EngineAggregate &asyncAgg) {
  std::cout << "Repeat summary: " << iterations << " iterations in "
            << spot_table::FormatDuration(
                   spot_table::RoundDuration(elapsed, 1s))
            << ", both engines agreed every time\n\n";
  PrintLegend();
  PrintAggregate(syncAgg, elapsed);
  PrintAggregate(asyncAgg, elapsed);
}

// Repeats the scenario until the minimum duration elapses, printing progress
// and a final summary.
[[nodiscard]] std::optional<std::string>
RunRepeat(const std::string &tablePath, const spot_table::Table &table,
          std::chrono::nanoseconds timeout,
          std::chrono::nanoseconds minDuration) {
  std::cout << "Repeat: " << table.fm.name << " (" << BaseName(tablePath)
            << "), running for at least "
            << spot_table::FormatDuration(minDuration) << " ...\n\n";

  EngineAggregate syncAgg;
  EngineAggregate asyncAgg;
  const auto start = std::chrono::steady_clock::now();
  auto lastLog = start;
  int iterations = 0;
  while (true) {
    const spot_table::Deadline deadline =
        std::chrono::steady_clock::now() + timeout;
    const PairResult pair = RunPair(deadline, table);
    iterations++;

    if (const std::optional<std::string> err = Verdict(pair); err.has_value()) {
      if (pair.syncReport.has_value()) {
        PrintReport(*pair.syncReport);
      }
      if (pair.asyncReport.has_value()) {
        PrintReport(*pair.asyncReport);
      }
      const auto since = std::chrono::steady_clock::now() - start;
      return "repeat run failed on iteration " + std::to_string(iterations) +
             " after " +
             spot_table::FormatDuration(spot_table::RoundDuration(since, 1ms)) +
             ": " + *err;
    }
    syncAgg.Add(pair.syncReport);
    asyncAgg.Add(pair.asyncReport);

    const auto now = std::chrono::steady_clock::now();
    if (now - lastLog >= kRepeatLogInterval) {
      PrintHeartbeat(std::chrono::system_clock::now(), iterations, now - start,
                     minDuration, syncAgg, asyncAgg);
      lastLog = now;
    }
    if (now - start >= minDuration) {
      // Platform info heads the final report, not the stream of progress
      // blocks.
      spot_table::PrintPlatform();
      PrintRepeatSummary(iterations, now - start, syncAgg, asyncAgg);
      return std::nullopt;
    }
  }
}

//------------------------------------------------------------------------------
// CLI flag parsing.

struct Args {
  std::string table;
  std::chrono::nanoseconds timeout = kDefaultTimeout;
  std::chrono::nanoseconds minDuration{0};
};

// Parses the supported flags in --name=value or --name value form.
[[nodiscard]] bool ParseArgs(int argc, char **argv, Args &args,
                             std::string &err) {
  const auto take = [&](int &i, const std::string &inlineValue,
                        std::string &out) -> bool {
    if (!inlineValue.empty()) {
      out = inlineValue;
      return true;
    }
    if (i + 1 >= argc) {
      err = "missing value for flag";
      return false;
    }
    out = argv[++i];
    return true;
  };

  for (int i = 1; i < argc; ++i) {
    std::string arg = argv[i];
    std::string name = arg;
    std::string inlineValue;
    const std::string::size_type eq = arg.find('=');
    if (eq != std::string::npos) {
      name = arg.substr(0, eq);
      inlineValue = arg.substr(eq + 1);
    }
    if (name == "-table" || name == "--table") {
      if (!take(i, inlineValue, args.table)) {
        return false;
      }
    } else if (name == "-timeout" || name == "--timeout") {
      std::string value;
      if (!take(i, inlineValue, value)) {
        return false;
      }
      if (!spot_table::ParseDuration(value, args.timeout, err)) {
        return false;
      }
    } else if (name == "-min-duration" || name == "--min-duration") {
      std::string value;
      if (!take(i, inlineValue, value)) {
        return false;
      }
      if (!spot_table::ParseDuration(value, args.minDuration, err)) {
        return false;
      }
    } else {
      err = "unknown flag \"" + name + "\"";
      return false;
    }
  }
  return true;
}

} // namespace

int main(int argc, char **argv) {
  Args args;
  std::string parseErr;
  if (!ParseArgs(argc, argv, args, parseErr)) {
    std::cerr << "error: " << parseErr << "\n";
    return 1;
  }

  if (args.table.empty()) {
    std::cerr << "error: --table is required\n"
              << "usage: spot_table --table <path/to/table.md> [--timeout d] "
                 "[--min-duration d]\n"
              << "  scenario tables live under examples/tables/spot/ (e.g. "
                 "coverage.md);\n"
              << "  see examples/tables/spot/README.md for the table format\n";
    return 1;
  }

  const std::optional<std::string> resolved =
      ResolveTablePath(argv[0], args.table);
  if (!resolved.has_value()) {
    std::cerr << "error: table \"" << args.table
              << "\" not found (cwd=" << CurrentDir() << ")\n";
    return 1;
  }

  spot_table::Table table;
  try {
    table = spot_table::ParseFile(*resolved);
  } catch (const std::exception &err) {
    std::cerr << "parse: " << err.what() << "\n";
    return 1;
  }

  std::optional<std::string> runErr;
  try {
    if (args.minDuration > std::chrono::nanoseconds(0)) {
      runErr = RunRepeat(*resolved, table, args.timeout, args.minDuration);
    } else {
      runErr = RunOnce(*resolved, table, args.timeout);
    }
  } catch (const std::exception &err) {
    std::cerr << err.what() << "\n";
    return 1;
  }
  if (runErr.has_value()) {
    std::cerr << *runErr << "\n";
    return 1;
  }
  return 0;
}
