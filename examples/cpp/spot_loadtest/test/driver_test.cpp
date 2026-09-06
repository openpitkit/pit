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

// Driver integration tests (require the native core).
//
// Run with the native runtime resolvable at load time, e.g.:
//   OPENPIT_RUNTIME_LIBRARY=$(pwd)/target/release/libopenpit_ffi.dylib

#include "spot_loadtest/config/config.hpp"
#include "spot_loadtest/decimal.hpp"
#include "spot_loadtest/driver/driver.hpp"
#include "spot_loadtest/env/env.hpp"
#include "spot_loadtest/generator/generator.hpp"
#include "spot_loadtest/reporter/reporter.hpp"

#include <gtest/gtest.h>

#include <algorithm>
#include <chrono>
#include <cstdint>
#include <limits>
#include <map>
#include <memory>
#include <optional>
#include <sstream>
#include <string>
#include <vector>

namespace {

namespace config = spot_loadtest::config;
namespace driver = spot_loadtest::driver;
namespace gen = spot_loadtest::generator;
using spot_loadtest::Decimal;

constexpr std::chrono::seconds kFunctionalTestMaxSubmitLag{30};

[[nodiscard]] Decimal Dec(const std::string &s) {
  return Decimal::FromString(s);
}

[[nodiscard]] config::Config TestConfig(std::uint64_t seed,
                                        std::uint64_t totalOps, double target) {
  config::Config c;
  c.run.seed = seed;
  c.run.totalOps = totalOps;
  c.run.window = 1000;
  c.run.observer = true;
  c.arrival.offeredRate = 0; // unpaced (saturated).
  c.reject = config::Reject{target, 0.01};
  c.accounts = config::Accounts{200};
  c.concurrency = config::Concurrency{64, 8, kFunctionalTestMaxSubmitLag};
  c.asyncEngine.strategy = config::AsyncEngineStrategy::Dynamic;
  c.asyncEngine.maxQueues = 256;
  c.asyncEngine.idleCleanup = std::chrono::seconds(2);
  c.instruments.symbols = {"AAPL", "SPX",  "MSFT", "AMZN", "GOOG",
                           "META", "TSLA", "NVDA", "JPM",  "BAC"};
  c.instruments.settlement = "USD";
  c.lifecycle = config::Lifecycle{0.40, 0.15, 0.25, 0.20};
  c.funding.trigger = config::FundingTrigger::BalanceBelow;
  c.funding.threshold = Dec("100000");
  c.funding.seed = Dec("1000000");
  c.funding.topUp = Dec("1000000");
  c.cohorts = {
      config::Cohort{"chatty",
                     0.2,
                     0.9,
                     0.7,
                     4,
                     {{1, 1}, {10, 4}, {100, 2}},
                     config::SymbolSkew::Zipf,
                     1.3},
      config::Cohort{"steady",
                     0.5,
                     0.5,
                     0.25,
                     2,
                     {{1, 2}, {10, 3}, {100, 1}},
                     config::SymbolSkew::Uniform,
                     0.0},
      config::Cohort{"dormant",
                     0.3,
                     0.1,
                     0.05,
                     1,
                     {{1, 5}, {10, 1}},
                     config::SymbolSkew::Uniform,
                     0.0},
  };
  return c;
}

struct SingleEventRun {
  std::unique_ptr<gen::Stream> stream;
  driver::Config config;
};

[[nodiscard]] SingleEventRun MakeSingleEventRun(std::uint64_t seed) {
  config::Config cfg = TestConfig(seed, 1, 0.05);
  cfg.accounts.count = 1;
  cfg.concurrency = config::Concurrency{1, 1, kFunctionalTestMaxSubmitLag};
  std::unique_ptr<gen::Stream> stream = gen::Generate(cfg);

  driver::Config dcfg = driver::FromAppConfig(cfg);
  dcfg.collectors = 1;
  dcfg.finalizers = 1;
  dcfg.overheadProbes = 0;
  return SingleEventRun{std::move(stream), dcfg};
}

void SetWorkEventTimes(gen::Stream &stream,
                       std::chrono::nanoseconds virtualT0) {
  std::size_t workEvents = 0;
  for (gen::Event &event : stream.events) {
    if (event.kind == gen::EventKind::Funding && event.fundingIsSeed) {
      continue;
    }
    event.virtualT0 = virtualT0;
    ++workEvents;
  }
  if (workEvents == 0) {
    throw std::logic_error("test setup: generated stream has no work event");
  }
}

// The Phase-3 / Phase-4 integration gate: a moderate stream through the REAL
// asyncengine, asserting the per-op oracle agrees, submission is open-loop, the
// windows are populated, inner metrics fire, the overhead probe ran, and the
// checksum changed.
TEST(Driver, OraclePipeline) {
  const config::Config cfg = TestConfig(0xC0FFEE, 30000, 0.05);
  const std::unique_ptr<gen::Stream> stream = gen::Generate(cfg);
  ASSERT_GT(stream->stats.accepts, 0u);
  ASSERT_GT(stream->stats.rejects, 0u);

  driver::Config dcfg;
  dcfg.observer = true;
  dcfg.submitterWorkers = 8;
  dcfg.collectors = 16;
  dcfg.windowSize = 1000;
  dcfg.maxSubmitLag = kFunctionalTestMaxSubmitLag;
  dcfg.overheadProbes = 50;

  const driver::RunResult result = driver::Run(*stream, dcfg);
  const driver::Stats &stats = result.stats;
  const auto &snap = result.snapshot;

  const std::uint64_t wantOrderEvents =
      stream->stats.orderChecks +
      (stream->stats.fundings - stream->stats.seeds);
  EXPECT_EQ(stats.orderChecks, wantOrderEvents);
  EXPECT_EQ(stats.settlements, stream->stats.settlements);
  EXPECT_GT(stats.sampleCount, 0);

  // Open-loop witness: peak in-flight well above the active set.
  EXPECT_GT(stats.maxInFlight,
            static_cast<std::int64_t>(cfg.concurrency.activeAccounts));
  EXPECT_EQ(stats.submitterThreads, dcfg.submitterWorkers);

  EXPECT_FALSE(snap.windows.empty());
  EXPECT_GT(snap.orderCheck.count, 0);
  EXPECT_GT(snap.orderCheck.p99.count(), 0);
  EXPECT_GT(snap.serviceTime.count, 0);
  EXPECT_GT(snap.innerMetrics.dequeues, 0);
  EXPECT_GT(snap.innerMetrics.completes, 0);
  EXPECT_GT(snap.overhead.probes, 0);
  EXPECT_GT(snap.overhead.distribution.p50.count(), 0);
  EXPECT_NE(stats.checksum, 0u);
}

// Raises the reject target so a large fraction rejects, exercising the
// reject-code mapping heavily; the oracle must still agree.
TEST(Driver, OpenLoopHighRejectRate) {
  const config::Config cfg = TestConfig(0xBEEF, 20000, 0.20);
  const std::unique_ptr<gen::Stream> stream = gen::Generate(cfg);
  ASSERT_GT(stream->stats.rejects, 0u);

  driver::Config dcfg;
  dcfg.observer = false;
  dcfg.submitterWorkers = 8;
  dcfg.collectors = 16;
  dcfg.windowSize = 1000;
  dcfg.maxSubmitLag = kFunctionalTestMaxSubmitLag;
  dcfg.overheadProbes = 0;

  const driver::RunResult result = driver::Run(*stream, dcfg);
  EXPECT_EQ(result.stats.submitterThreads, dcfg.submitterWorkers);
  EXPECT_GT(result.stats.rejects, 0u);
  EXPECT_GT(result.stats.maxInFlight,
            static_cast<std::int64_t>(cfg.concurrency.activeAccounts));
  EXPECT_EQ(result.snapshot.innerMetrics.dequeues, 0);
}

// Drives the full bounded-concurrency path via FromAppConfig and asserts the
// oracle agrees, submission stays open-loop, and a healthy run reports ZERO
// backpressure.
TEST(Driver, BoundedConcurrency) {
  const config::Config cfg = TestConfig(0xC0FFEE, 30000, 0.05);
  const std::unique_ptr<gen::Stream> stream = gen::Generate(cfg);
  ASSERT_EQ(stream->stats.seeds, cfg.accounts.count);

  driver::Config dcfg = driver::FromAppConfig(cfg);
  dcfg.collectors = 16;
  dcfg.overheadProbes = 0;

  const driver::RunResult result = driver::Run(*stream, dcfg);
  EXPECT_EQ(result.stats.backpressure, 0u);
  EXPECT_NE(result.stats.checksum, 0u);
  EXPECT_EQ(result.stats.submitterThreads,
            static_cast<std::size_t>(cfg.concurrency.submitterWorkers));
  EXPECT_GT(result.stats.maxInFlight,
            static_cast<std::int64_t>(cfg.concurrency.activeAccounts));
  const std::uint64_t wantOrderEvents =
      stream->stats.orderChecks +
      (stream->stats.fundings - stream->stats.seeds);
  EXPECT_EQ(result.stats.orderChecks, wantOrderEvents);
}

TEST(Driver, NonEmptyStreamRequiresSubmitterWorkers) {
  const config::Config cfg = TestConfig(0xBAD, 100, 0.05);
  const std::unique_ptr<gen::Stream> stream = gen::Generate(cfg);
  ASSERT_FALSE(stream->events.empty());

  driver::Config dcfg;
  dcfg.collectors = 2;
  dcfg.finalizers = 2;
  dcfg.windowSize = 100;

  EXPECT_THROW((void)driver::Run(*stream, dcfg), std::invalid_argument);
}

TEST(Driver, FromAppConfigChecksWindowRepresentationBounds) {
  config::Config cfg = TestConfig(0xA12, 100, 0.05);

  cfg.run.window = 0;
  EXPECT_THROW((void)driver::FromAppConfig(cfg), std::invalid_argument);

  cfg.run.window =
      static_cast<std::uint64_t>(std::numeric_limits<std::int64_t>::max());
  EXPECT_EQ(driver::FromAppConfig(cfg).windowSize,
            std::numeric_limits<std::int64_t>::max());

  ++cfg.run.window;
  EXPECT_THROW((void)driver::FromAppConfig(cfg), std::invalid_argument);
}

TEST(Driver, RunRequiresPositiveWindowSize) {
  gen::Stream stream;
  driver::Config cfg;
  cfg.collectors = 1;
  cfg.finalizers = 1;
  cfg.maxSubmitLag = kFunctionalTestMaxSubmitLag;
  cfg.overheadProbes = 0;

  cfg.windowSize = 0;
  EXPECT_THROW((void)driver::Run(stream, cfg), std::invalid_argument);
  cfg.windowSize = -1;
  EXPECT_THROW((void)driver::Run(stream, cfg), std::invalid_argument);

  cfg.windowSize = 1;
  EXPECT_NO_THROW((void)driver::Run(stream, cfg));
  cfg.windowSize = std::numeric_limits<std::int64_t>::max();
  EXPECT_NO_THROW((void)driver::Run(stream, cfg));
}

TEST(Driver, FromAppConfigRequiresPositiveSubmitLagLimit) {
  config::Config cfg = TestConfig(0xA13, 100, 0.05);
  cfg.concurrency.maxSubmitLag = std::chrono::nanoseconds(0);

  EXPECT_THROW((void)driver::FromAppConfig(cfg), std::invalid_argument);
}

TEST(Driver, SmallVirtualDeadlineRuns) {
  SingleEventRun run = MakeSingleEventRun(0xD1);
  SetWorkEventTimes(*run.stream, std::chrono::nanoseconds(1));

  EXPECT_NO_THROW((void)driver::Run(*run.stream, run.config));
}

TEST(Driver, SubmitLagBreachInvalidatesRun) {
  SingleEventRun run = MakeSingleEventRun(0xD10);
  SetWorkEventTimes(*run.stream, std::chrono::nanoseconds(0));
  run.config.maxSubmitLag = std::chrono::nanoseconds(1);

  std::string invalidReason;
  const driver::RunResult result =
      driver::RunCollecting(*run.stream, run.config, invalidReason);
  EXPECT_EQ(invalidReason, "submit-lag");
  EXPECT_GT(result.snapshot.submitLag.count, 0);
  EXPECT_GT(result.snapshot.submitLagBreaches, 0u);
  EXPECT_EQ(result.stats.submitLagBreaches, result.snapshot.submitLagBreaches);
  EXPECT_THROW((void)driver::Run(*run.stream, run.config),
               driver::SubmitLagInvalidRun);
}

TEST(Driver, NegativeVirtualDeadlineIsRejected) {
  SingleEventRun run = MakeSingleEventRun(0xD2);
  SetWorkEventTimes(*run.stream, std::chrono::nanoseconds(-1));

  try {
    (void)driver::Run(*run.stream, run.config);
    FAIL() << "expected negative virtual deadline failure";
  } catch (const std::runtime_error &error) {
    EXPECT_NE(std::string(error.what()).find("negative virtualT0"),
              std::string::npos);
  }
}

TEST(Driver, UnrepresentableVirtualDeadlineIsRejectedWithoutSleeping) {
  SingleEventRun run = MakeSingleEventRun(0xD3);
  for (const std::chrono::nanoseconds offset :
       {std::chrono::nanoseconds::max() - std::chrono::nanoseconds(1),
        std::chrono::nanoseconds::max()}) {
    SCOPED_TRACE(offset.count());
    SetWorkEventTimes(*run.stream, offset);
    try {
      (void)driver::Run(*run.stream, run.config);
      FAIL() << "expected virtual deadline overflow";
    } catch (const std::runtime_error &error) {
      EXPECT_NE(std::string(error.what()).find("clock"), std::string::npos);
    }
  }
}

TEST(Driver, SubmitterWorkerFailurePropagatesCleanly) {
  const config::Config cfg = TestConfig(0xFA17, 1000, 0.05);
  const std::unique_ptr<gen::Stream> stream = gen::Generate(cfg);
  auto event = std::find_if(stream->events.begin(), stream->events.end(),
                            [](const gen::Event &item) {
                              return item.kind == gen::EventKind::OrderCheck;
                            });
  ASSERT_NE(event, stream->events.end());
  event->price = Dec("-1");
  event->quantity = Dec("-1");

  driver::Config dcfg = driver::FromAppConfig(cfg);
  dcfg.collectors = 4;
  dcfg.finalizers = 4;
  dcfg.overheadProbes = 0;

  try {
    (void)driver::Run(*stream, dcfg);
    FAIL() << "expected submitter worker failure";
  } catch (const std::runtime_error &error) {
    EXPECT_NE(std::string(error.what()).find("driver: submitter worker:"),
              std::string::npos);
  }
}

TEST(Driver, WorkerFailureDoesNotLoseSleepingSubmitterWakeup) {
  constexpr int kSleepers = 32;
  constexpr int kIterations = 16;
  config::Config cfg = TestConfig(0xFA18, 1, 0.05);
  cfg.accounts.count = kSleepers + 1;
  cfg.concurrency = config::Concurrency{kSleepers + 1, kSleepers + 1,
                                        kFunctionalTestMaxSubmitLag};
  std::unique_ptr<gen::Stream> stream = gen::Generate(cfg);

  std::vector<std::string> accounts;
  for (const gen::Event &event : stream->events) {
    if (event.kind == gen::EventKind::Funding && event.fundingIsSeed) {
      accounts.push_back(event.account);
    }
  }
  ASSERT_EQ(accounts.size(), static_cast<std::size_t>(kSleepers + 1));
  const auto work = std::find_if(
      stream->events.begin(), stream->events.end(),
      [](const gen::Event &event) {
        return !(event.kind == gen::EventKind::Funding && event.fundingIsSeed);
      });
  ASSERT_NE(work, stream->events.end());

  std::vector<gen::Event> events;
  for (const gen::Event &event : stream->events) {
    if (event.kind == gen::EventKind::Funding && event.fundingIsSeed) {
      events.push_back(event);
    }
  }
  for (int index = 0; index < kSleepers; ++index) {
    gen::Event sleeper = *work;
    sleeper.seq = static_cast<std::uint64_t>(100 + index);
    sleeper.account = accounts[static_cast<std::size_t>(index)];
    sleeper.virtualT0 = std::chrono::seconds(1);
    events.push_back(std::move(sleeper));
  }

  gen::Event failing = *work;
  failing.seq = 1000;
  failing.account = accounts.back();
  failing.virtualT0 = std::chrono::nanoseconds(0);
  failing.price = Dec("-1");
  failing.quantity = Dec("-1");
  events.push_back(failing);
  stream->events = std::move(events);

  driver::Config dcfg = driver::FromAppConfig(cfg);
  dcfg.collectors = 4;
  dcfg.finalizers = 4;
  dcfg.overheadProbes = 0;

  for (int iteration = 0; iteration < kIterations; ++iteration) {
    SCOPED_TRACE(iteration);
    const auto started = std::chrono::steady_clock::now();
    EXPECT_THROW((void)driver::Run(*stream, dcfg), std::runtime_error);
    const auto elapsed = std::chrono::steady_clock::now() - started;
    EXPECT_LT(elapsed, std::chrono::milliseconds(500));
  }
}

TEST(Driver, SubmitterShardMergesChainsByVirtualTime) {
  config::Config cfg = TestConfig(0x5A4D, 80, 0.05);
  cfg.accounts.count = 4;
  cfg.concurrency = config::Concurrency{4, 1, kFunctionalTestMaxSubmitLag};
  cfg.arrival.offeredRate = 40;
  const std::unique_ptr<gen::Stream> stream = gen::Generate(cfg);

  std::map<std::string, std::chrono::nanoseconds> lastArrival;
  for (const gen::Event &event : stream->events) {
    auto [position, inserted] =
        lastArrival.try_emplace(event.account, event.virtualT0);
    if (!inserted && position->second < event.virtualT0) {
      position->second = event.virtualT0;
    }
  }
  ASSERT_GE(lastArrival.size(), 2u);

  std::vector<std::string> accounts;
  accounts.reserve(lastArrival.size());
  for (const auto &[account, _] : lastArrival) {
    accounts.push_back(account);
  }
  std::sort(accounts.begin(), accounts.end(),
            [&](const std::string &lhs, const std::string &rhs) {
              if (lastArrival.at(lhs) != lastArrival.at(rhs)) {
                return lastArrival.at(lhs) > lastArrival.at(rhs);
              }
              return lhs < rhs;
            });
  std::map<std::string, std::size_t> rank;
  for (std::size_t index = 0; index < accounts.size(); ++index) {
    rank.emplace(accounts[index], index);
  }

  std::optional<std::chrono::nanoseconds> laterFirstArrival;
  for (const gen::Event &event : stream->events) {
    if (rank.at(event.account) == 0 ||
        (event.kind == gen::EventKind::Funding && event.fundingIsSeed)) {
      continue;
    }
    if (!laterFirstArrival || event.virtualT0 < *laterFirstArrival) {
      laterFirstArrival = event.virtualT0;
    }
  }
  ASSERT_TRUE(laterFirstArrival.has_value());
  // A whole-chain scheduler would issue the first chain's late event before
  // this earlier event from another chain. The driver rejects that regression.
  ASSERT_GT(lastArrival.at(accounts.front()), *laterFirstArrival);

  std::stable_sort(stream->events.begin(), stream->events.end(),
                   [&](const gen::Event &lhs, const gen::Event &rhs) {
                     return rank.at(lhs.account) < rank.at(rhs.account);
                   });

  driver::Config dcfg = driver::FromAppConfig(cfg);
  dcfg.collectors = 4;
  dcfg.finalizers = 4;
  dcfg.overheadProbes = 0;
  const driver::RunResult result = driver::Run(*stream, dcfg);

  EXPECT_EQ(result.stats.submitterThreads, 1u);
  EXPECT_EQ(result.stats.backpressure, 0u);
}

// Exercises the PACED offered-rate path; submission still overlaps decisions.
TEST(Driver, PacedSubmission) {
  config::Config cfg = TestConfig(0x5EED, 8000, 0.05);
  cfg.arrival.offeredRate = 100000;
  const std::unique_ptr<gen::Stream> stream = gen::Generate(cfg);

  driver::Config dcfg;
  dcfg.observer = false;
  dcfg.submitterWorkers = 8;
  dcfg.collectors = 16;
  dcfg.windowSize = 1000;
  dcfg.maxSubmitLag = kFunctionalTestMaxSubmitLag;
  dcfg.overheadProbes = 0;

  const driver::RunResult result = driver::Run(*stream, dcfg);
  EXPECT_GT(result.stats.sampleCount, 0);
  EXPECT_GE(result.stats.maxInFlight, 2);
  EXPECT_GT(result.snapshot.orderCheck.count, 0);
}

// Exercises the sharded dispatch path end-to-end.
TEST(Driver, ShardedStrategy) {
  config::Config cfg = TestConfig(0x5ADED, 15000, 0.05);
  cfg.asyncEngine.strategy = config::AsyncEngineStrategy::Sharded;
  cfg.asyncEngine.shardedWorkers = 3;
  const std::unique_ptr<gen::Stream> stream = gen::Generate(cfg);
  ASSERT_GT(stream->stats.accepts, 0u);
  ASSERT_GT(stream->stats.rejects, 0u);

  driver::Config dcfg = driver::FromAppConfig(cfg);
  dcfg.collectors = 16;
  dcfg.overheadProbes = 0;

  const driver::RunResult result = driver::Run(*stream, dcfg);
  const std::uint64_t wantOrderEvents =
      stream->stats.orderChecks +
      (stream->stats.fundings - stream->stats.seeds);
  EXPECT_EQ(result.stats.orderChecks, wantOrderEvents);
  EXPECT_EQ(result.stats.settlements, stream->stats.settlements);
  EXPECT_EQ(result.stats.backpressure, 0u);
  EXPECT_GE(result.stats.maxInFlight, 2);
  EXPECT_FALSE(result.snapshot.windows.empty());
  EXPECT_GT(result.snapshot.orderCheck.count, 0);
}

// Source: examples/cpp/spot_loadtest/README.md - Build and run
//
// The doc-backing test: loads the committed baseline.ini, runs a reduced
// end-to-end through the REAL engine, and renders the report asserting every
// named block is present, the run is oracle-clean, backpressure is zero, and
// the anti-DCE checksum is non-zero.
TEST(Driver, DocBackingBaselineRecipe) {
  config::Config baseCfg = config::Load(SPOT_LOADTEST_BASELINE_INI);
  ASSERT_FALSE(baseCfg.cohorts.empty());
  ASSERT_FALSE(baseCfg.instruments.symbols.empty());
  EXPECT_EQ(baseCfg.concurrency.maxSubmitLag, std::chrono::milliseconds(1));

  // Reduced run: same seed + cohort structure as the baseline, smaller scale.
  config::Config reduced = baseCfg;
  reduced.run.totalOps = 30000;
  reduced.run.window = 5000;
  reduced.accounts.count = 500;
  reduced.concurrency.activeAccounts = 64;
  reduced.concurrency.maxSubmitLag = kFunctionalTestMaxSubmitLag;
  reduced.asyncEngine.maxQueues = 0;
  reduced.asyncEngine.idleCleanup = std::chrono::seconds(2);

  const std::unique_ptr<gen::Stream> stream = gen::Generate(reduced);
  ASSERT_GT(stream->stats.orderChecks, 0u);

  driver::Config dcfg = driver::FromAppConfig(reduced);
  dcfg.collectors = 16;
  dcfg.overheadProbes = 50;

  const driver::RunResult result = driver::Run(*stream, dcfg);
  EXPECT_GT(result.stats.orderChecks, 0u);
  EXPECT_GT(result.stats.accepts, 0u);
  EXPECT_GT(result.stats.rejects, 0u);
  EXPECT_EQ(result.snapshot.backpressure, 0u);
  EXPECT_EQ(result.snapshot.submitLagBreaches, 0u);
  EXPECT_NE(result.snapshot.checksum, 0u);
  EXPECT_GE(result.snapshot.maxInFlight, 2);
  EXPECT_FALSE(result.snapshot.windows.empty());
  EXPECT_GT(result.snapshot.orderCheck.count, 0);

  // Render and assert all named blocks (use a synthetic env so the test is
  // hermetic).
  spot_loadtest::env::Env e;
  e.host.cpuModel = "doc-backing-test (synthetic)";
  e.core.version = "0.0.0";
  e.core.profile = "release";
  std::ostringstream buf;
  spot_loadtest::reporter::Write(buf, e, reduced, "configs/baseline.ini",
                                 result.snapshot, result.stats.submitterThreads,
                                 stream->stats);
  const std::string out = buf.str();
  for (const char *block :
       {"=== Headline:", "=== Environment ===", "=== Workload ===",
        "=== Trajectory", "=== Distribution", "=== Diagnostics",
        "=== Disclaimer ==="}) {
    EXPECT_NE(out.find(block), std::string::npos) << "missing block " << block;
  }
  EXPECT_NE(out.find("0 (healthy"), std::string::npos);
  EXPECT_NE(out.find("Open-Loop Order-Check Latency"), std::string::npos);
  EXPECT_NE(out.find("DIAGNOSTIC, NOT the headline"), std::string::npos);
  EXPECT_NE(out.find("Submit scheduling lag"), std::string::npos);
  EXPECT_NE(out.find("What IS measured"), std::string::npos);
  EXPECT_NE(out.find("What is NOT measured"), std::string::npos);
  EXPECT_NE(out.find("configs/baseline.ini"), std::string::npos);
  EXPECT_NE(out.find("Anti-DCE checksum"), std::string::npos);
}

} // namespace
