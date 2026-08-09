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

// Reporter rendering tests.
//

#include "spot_loadtest/reporter/reporter.hpp"

#include "spot_loadtest/config/config.hpp"
#include "spot_loadtest/env/env.hpp"
#include "spot_loadtest/generator/event.hpp"
#include "spot_loadtest/measurement/snapshot.hpp"

#include <gtest/gtest.h>

#include <chrono>
#include <cstddef>
#include <sstream>
#include <string>
#include <vector>

namespace {

namespace config = spot_loadtest::config;
namespace env = spot_loadtest::env;
namespace generator = spot_loadtest::generator;
namespace measurement = spot_loadtest::measurement;
namespace reporter = spot_loadtest::reporter;

using std::chrono::microseconds;
using std::chrono::milliseconds;
using std::chrono::nanoseconds;
using std::chrono::seconds;

constexpr std::size_t kSyntheticSubmitterThreads = 7;

[[nodiscard]] bool Contains(const std::string &haystack,
                            const std::string &needle) {
  return haystack.find(needle) != std::string::npos;
}

// syntheticSnapshot builds a non-trivial Snapshot for report rendering tests.
[[nodiscard]] measurement::Snapshot SyntheticSnapshot() {
  const measurement::Clock::time_point now = measurement::Clock::now();

  measurement::Snapshot snap;

  measurement::WindowSnapshot w0;
  w0.index = 0;
  w0.orderCheck = measurement::Percentiles{microseconds(100), microseconds(200),
                                           microseconds(500), milliseconds(1),
                                           milliseconds(5),   100000};
  w0.settlement = measurement::Percentiles{microseconds(50),  microseconds(100),
                                           microseconds(300), microseconds(500),
                                           milliseconds(2),   50000};
  w0.wallStart = now - seconds(10);
  w0.wallEnd = now - seconds(5);

  measurement::WindowSnapshot w1;
  w1.index = 1;
  w1.orderCheck = measurement::Percentiles{microseconds(95),  microseconds(180),
                                           microseconds(450), microseconds(900),
                                           milliseconds(4),   100000};
  w1.settlement = measurement::Percentiles{
      microseconds(48),  microseconds(95),   microseconds(280),
      microseconds(480), microseconds(1800), 50000};
  w1.wallStart = now - seconds(5);
  w1.wallEnd = now;

  snap.windows = {w0, w1};

  snap.orderCheck = measurement::Percentiles{
      microseconds(97),  microseconds(190), microseconds(475),
      microseconds(950), milliseconds(5),   200000};
  snap.settlement = measurement::Percentiles{
      microseconds(49),  microseconds(98), microseconds(290),
      microseconds(490), milliseconds(2),  100000};

  // SteadyStateOrderCheck / SteadyStateSettlement mirror window[1] (warmup=1
  // for a 2-window snapshot). In production these come from Build via a
  // lossless HdrHistogram merge; here we supply representative values.
  snap.steadyStateOrderCheck = measurement::Percentiles{
      microseconds(95),  microseconds(180), microseconds(450),
      microseconds(900), milliseconds(4),   100000};
  snap.steadyStateSettlement = measurement::Percentiles{
      microseconds(48),  microseconds(95),   microseconds(280),
      microseconds(480), microseconds(1800), 50000};

  // Service-time is the diagnostic counterpart to the open-loop headline; in a
  // saturated run it is much lower than the open-loop tail (it discounts
  // pre-submit queue wait). These representative values exercise the render.
  snap.serviceTime = measurement::Percentiles{
      microseconds(30),  microseconds(45),  microseconds(70),
      microseconds(120), microseconds(300), 200000};
  snap.submitLag = measurement::Percentiles{
      microseconds(10),  microseconds(20),  microseconds(80),
      microseconds(150), microseconds(400), 200000};

  snap.warmupWindows = 1;
  snap.throughput = 45000;
  snap.totalOrderChecks = 200000;
  snap.totalSettlements = 100000;
  snap.totalAccepts = 190000;
  snap.totalRejects = 10000;
  snap.achievedRejectRate = 0.050;
  snap.maxInFlight = 128;
  snap.checksum = 0xDEADBEEFCAFEBABEULL;

  snap.overhead.probes = 200;
  snap.overhead.distribution = measurement::Percentiles{
      microseconds(20), microseconds(35),  microseconds(50),
      microseconds(80), microseconds(200), 200};

  snap.innerMetrics.queueWait = measurement::Percentiles{
      microseconds(10),  microseconds(0), microseconds(80),
      microseconds(200), milliseconds(1), 200000};
  snap.innerMetrics.engineCompute = measurement::Percentiles{
      microseconds(5),  microseconds(0),   microseconds(20),
      microseconds(50), microseconds(200), 200000};
  snap.innerMetrics.queuesCreated = 10000;
  snap.innerMetrics.queuesRemoved = 9950;
  snap.innerMetrics.dequeues = 200000;
  snap.innerMetrics.completes = 200000;

  snap.wallStart = now - seconds(10);
  snap.wallEnd = now;
  snap.wallStartSet = true;

  return snap;
}

// syntheticConfig builds a minimal valid Config for rendering.
[[nodiscard]] config::Config SyntheticConfig() {
  config::Config cfg;
  cfg.path = "/tmp/test.ini";
  cfg.hash = "abc123";

  cfg.run.seed = 0xC0FFEE;
  cfg.run.totalOps = 200000;
  cfg.run.window = 100000;
  cfg.run.observer = true;

  cfg.reject.targetRate = 0.05;
  cfg.reject.tolerance = 0.005;

  cfg.accounts.count = 10000;
  cfg.concurrency.activeAccounts = 1024;
  cfg.concurrency.submitterWorkers = 8;
  cfg.concurrency.maxSubmitLag = milliseconds(1);

  cfg.asyncEngine.strategy = config::AsyncEngineStrategy::Dynamic;
  cfg.asyncEngine.maxQueues = 4096;
  cfg.asyncEngine.idleCleanup = seconds(2);

  cfg.cohorts = {
      config::Cohort{
          "chatty", 0.2, 0.9, 0.7, 4, {}, config::SymbolSkew::Uniform, 0.0},
      config::Cohort{
          "steady", 0.5, 0.5, 0.25, 2, {}, config::SymbolSkew::Uniform, 0.0},
      config::Cohort{
          "dormant", 0.3, 0.1, 0.05, 1, {}, config::SymbolSkew::Uniform, 0.0},
  };

  return cfg;
}

[[nodiscard]] env::Env SyntheticEnv() {
  env::Env e;
  e.host.cpuModel = "Test CPU 3.5GHz";
  e.host.cores = 8;
  e.host.ram = "16.0 GiB";
  e.host.os = "TestOS 1.0";
  e.host.kernel = "TestKernel 5.0";

  e.toolchain.compiler = "Clang 17.0.0";
  e.toolchain.cppStd = "C++17";
  e.toolchain.targetArch = "x86_64";

  e.pit.commit = "abc1234def";
  e.pit.dirty = false;
  e.pit.dirtyKnown = true;

  e.core.version = "0.1.0";
  e.core.profile = "release";
  e.core.optLevel = "3";
  e.core.debugAssertions = false;
  e.core.target = "x86_64-unknown-linux-gnu";
  e.core.targetCpu = "native";
  e.core.lto = "thin";
  e.core.raw = "profile=release;opt_level=3;debug_assertions=false";

  return e;
}

[[nodiscard]] generator::StreamStats SyntheticStreamStats() {
  generator::StreamStats s;
  s.orderChecks = 200000;
  s.accepts = 190000;
  s.rejects = 10000;
  s.settlements = 190000;
  s.fundings = 20000;
  s.forcedRejects = 9000;
  s.naturalRejects = 1000;
  return s;
}

[[nodiscard]] std::string Render() {
  std::ostringstream out;
  reporter::Write(out, SyntheticEnv(), SyntheticConfig(), "configs/test.ini",
                  SyntheticSnapshot(), kSyntheticSubmitterThreads,
                  SyntheticStreamStats());
  return out.str();
}

// Verifies that the report contains all expected block headers in the correct
// order.
TEST(Reporter, ReportContainsAllBlockHeaders) {
  const std::string out = Render();
  const std::vector<std::string> expectedHeaders = {
      "=== Headline:",      "=== Environment ===", "=== Workload ===",
      "=== Trajectory",     "=== Distribution",    "=== Diagnostics",
      "=== Disclaimer ===",
  };
  for (const auto &h : expectedHeaders) {
    EXPECT_TRUE(Contains(out, h)) << "report missing block header " << h;
  }

  // Headers must appear in order.
  std::size_t prev = 0;
  for (const auto &h : expectedHeaders) {
    const std::size_t idx = out.find(h);
    ASSERT_NE(idx, std::string::npos);
    EXPECT_GE(idx, prev) << "block header " << h << " is out of order";
    prev = idx;
  }
}

// Verifies the steady-state definition is clearly labeled and contains the
// warmup exclusion wording.
TEST(Reporter, HeadlineSteadyStateLabel) {
  const std::string out = Render();
  EXPECT_TRUE(Contains(out, "warmup"))
      << "headline must mention warmup exclusion";
  EXPECT_TRUE(Contains(out, "Steady-state definition"))
      << "headline must label the steady-state definition";
}

// Verifies that p99.9 and max are always shown in the headline (honesty
// guardrail: tail must be visible).
TEST(Reporter, HeadlineContainsTailPercentiles) {
  const std::string out = Render();
  EXPECT_TRUE(Contains(out, "p99.9"))
      << "headline must show p99.9 (tail must not be hidden)";
  EXPECT_TRUE(Contains(out, "max"))
      << "headline must show max (tail must not be hidden)";
}

// Verifies the headline is labelled as the OPEN-LOOP latency-under-load
// (intended arrival -> decision), with the coordinated-omission-defence
// wording.
TEST(Reporter, HeadlineIsOpenLoop) {
  const std::string out = Render();
  EXPECT_TRUE(Contains(out, "Open-Loop Order-Check Latency"))
      << "headline must be labelled as the open-loop order-check latency";
  EXPECT_TRUE(Contains(out, "intended arrival"))
      << "headline must explain t0 is the intended arrival (not the actual "
         "submit)";
  EXPECT_TRUE(Contains(out, "coordinated-omission"))
      << "headline must mention the coordinated-omission defence";
}

// Verifies the service-time figure is rendered in the diagnostics section, with
// the service-time percentiles, and is loudly labelled as NOT the headline.
TEST(Reporter, ServiceTimeIsDiagnosticOnly) {
  const std::string out = Render();
  const std::size_t diagIdx = out.find("=== Diagnostics");
  const std::size_t headIdx = out.find("=== Headline");
  const std::size_t stIdx = out.find("Service-time (resolve - ACTUAL submit)");
  ASSERT_NE(stIdx, std::string::npos)
      << "report must include the service-time diagnostic line";
  ASSERT_NE(diagIdx, std::string::npos);
  ASSERT_NE(headIdx, std::string::npos);
  // The service-time figure must live in the diagnostics block, after the
  // headline block (never in the headline).
  EXPECT_GT(stIdx, diagIdx)
      << "service-time must appear in the diagnostics block";
  EXPECT_GT(diagIdx, headIdx)
      << "diagnostics block must follow the headline block";
  EXPECT_TRUE(Contains(out, "DIAGNOSTIC, NOT the headline"))
      << "service-time must be loudly labelled as a diagnostic, not the "
         "headline";
}

// Verifies that submit scheduling lag is a separate harness-validity
// distribution and never masquerades as engine latency.
TEST(Reporter, SubmitLagIsHarnessValidityMetric) {
  const std::string out = Render();
  const std::size_t diagIdx = out.find("=== Diagnostics");
  const std::size_t lagIdx = out.find("Submit scheduling lag");
  ASSERT_NE(diagIdx, std::string::npos);
  ASSERT_NE(lagIdx, std::string::npos);
  EXPECT_GT(lagIdx, diagIdx);
  EXPECT_TRUE(Contains(out, "HARNESS VALIDITY metric"));
  EXPECT_TRUE(Contains(out, "breaches: 0"));
}

// Verifies the anti-DCE checksum is printed.
TEST(Reporter, ChecksumInReport) {
  std::ostringstream out;
  measurement::Snapshot snap = SyntheticSnapshot();
  snap.checksum = 0xDEADBEEFCAFEBABEULL;
  reporter::Write(out, SyntheticEnv(), SyntheticConfig(), "configs/test.ini",
                  snap, kSyntheticSubmitterThreads, SyntheticStreamStats());
  EXPECT_TRUE(Contains(out.str(), "DEADBEEFCAFEBABE"))
      << "report must print the anti-DCE checksum";
}

// Verifies that target and achieved reject rates appear.
TEST(Reporter, RejectRateInWorkload) {
  const std::string out = Render();
  EXPECT_TRUE(Contains(out, "Achieved reject rate"))
      << "workload block must show achieved reject rate";
  EXPECT_TRUE(Contains(out, "Target reject rate"))
      << "workload block must show target reject rate";
}

// Verifies the report discloses the bounded-concurrency model (population vs
// active working set), the engine dispatch sizing (strategy + active knobs),
// and the backpressure count.
TEST(Reporter, ConcurrencyDisclosure) {
  const std::string out = Render();
  for (const auto &want : {
           "Concurrency model",
           "population (total accounts)",
           "active working set",
           "configured submitter cap      : 8",
           "actual submitter threads      : 7",
           "maximum submit scheduling lag : 1ms",
           "submit-lag limit breaches     : 0",
           "Engine dispatch sizing",
           "strategy",
           "max_queues",
           "idle_cleanup",
           "queue_capacity",
           "slow_submit_threshold",
           "Backpressure",
       }) {
    EXPECT_TRUE(Contains(out, want))
        << "report missing concurrency disclosure " << want;
  }
  // A healthy synthetic snapshot (backpressure == 0) must say so.
  EXPECT_TRUE(Contains(out, "0 (healthy"))
      << "report must show backpressure = 0 as healthy when none occurred";
}

// Verifies a nonzero backpressure count is surfaced loudly (never hidden) when
// the dispatch capacity was exceeded.
TEST(Reporter, BackpressureDisclosed) {
  std::ostringstream out;
  measurement::Snapshot snap = SyntheticSnapshot();
  snap.backpressure = 4242;
  reporter::Write(out, SyntheticEnv(), SyntheticConfig(), "configs/test.ini",
                  snap, kSyntheticSubmitterThreads, SyntheticStreamStats());
  const std::string s = out.str();
  EXPECT_TRUE(Contains(s, "4242"))
      << "report must print a nonzero backpressure count";
  EXPECT_TRUE(Contains(s, "degraded"))
      << "report must flag a backpressured run as degraded";
}

// Verifies the diagnostics block correctly reports when the observer is off.
TEST(Reporter, ObserverDisabledMessage) {
  std::ostringstream out;
  config::Config cfg = SyntheticConfig();
  cfg.run.observer = false;
  reporter::Write(out, SyntheticEnv(), cfg, "configs/test.ini",
                  SyntheticSnapshot(), kSyntheticSubmitterThreads,
                  SyntheticStreamStats());
  EXPECT_TRUE(Contains(out.str(), "Observer disabled"))
      << "diagnostics block must say observer is disabled when observer=off";
}

// Verifies per-window rows appear in the trajectory.
TEST(Reporter, TrajectoryWindowsPresent) {
  const std::string out = Render();
  // Should have trajectory rows for windows 1 and 2.
  EXPECT_TRUE(Contains(out, "   1") || Contains(out, "1w"))
      << "trajectory block must show window 1 row";
}

// Verifies the config flag appears in the reproduction recipe.
TEST(Reporter, ReproductionRecipe) {
  std::ostringstream out;
  reporter::Write(out, SyntheticEnv(), SyntheticConfig(),
                  "configs/baseline.ini", SyntheticSnapshot(),
                  kSyntheticSubmitterThreads, SyntheticStreamStats());
  EXPECT_TRUE(Contains(out.str(), "configs/baseline.ini"))
      << "disclaimer must include the config flag in the reproduction recipe";
}

// Verifies key disclaimer language is included.
TEST(Reporter, DisclaimerPresent) {
  const std::string out = Render();
  EXPECT_TRUE(Contains(out, "What IS measured"))
      << "disclaimer must say what IS measured";
  EXPECT_TRUE(Contains(out, "What is NOT measured"))
      << "disclaimer must say what is NOT measured";
}

// Verifies that with a single window the report does not claim to exclude
// warmup (nothing to exclude).
TEST(Reporter, SingleWindowNoWarmupExclusion) {
  std::ostringstream out;
  measurement::Snapshot snap = SyntheticSnapshot();
  snap.windows.resize(1); // only one window
  snap.warmupWindows = 0; // no warmup when single window
  reporter::Write(out, SyntheticEnv(), SyntheticConfig(), "configs/test.ini",
                  snap, kSyntheticSubmitterThreads, SyntheticStreamStats());
  EXPECT_TRUE(Contains(out.str(), "single window"))
      << "with one window the report must note no warmup exclusion is possible";
}

// Verifies overhead summary appears when probes > 0.
TEST(Reporter, DistributionOverheadBlock) {
  const std::string out = Render();
  EXPECT_TRUE(Contains(out, "self-overhead"))
      << "distribution block must contain overhead section";
}

// Verifies the invalid-run report omits the headline and latency-distribution
// blocks and prints the loud invalid banner naming the actual reason(s).
TEST(Reporter, WriteInvalidSuppressesHeadlineAndNamesReason) {
  std::ostringstream out;
  measurement::Snapshot snap = SyntheticSnapshot();
  snap.backpressure = 7;
  reporter::WriteInvalid(out, SyntheticEnv(), SyntheticConfig(),
                         "configs/test.ini", snap, kSyntheticSubmitterThreads,
                         SyntheticStreamStats());
  const std::string s = out.str();
  EXPECT_TRUE(Contains(s, "RUN INVALID"))
      << "invalid report must print the invalid banner";
  EXPECT_TRUE(Contains(s, "dispatch backpressure"))
      << "invalid report must name the actual reason (backpressure)";
  // The headline and the latency-distribution percentile blocks are suppressed.
  EXPECT_FALSE(Contains(s, "=== Headline:"))
      << "invalid report must omit the headline block";
  EXPECT_FALSE(Contains(s, "=== Distribution"))
      << "invalid report must omit the distribution block";
  // The non-latency diagnostics are retained.
  EXPECT_TRUE(Contains(s, "=== Environment ==="))
      << "invalid report keeps the environment";
  EXPECT_TRUE(Contains(s, "=== Workload ==="))
      << "invalid report keeps the workload counts";
}

// Verifies a zero anti-DCE checksum on a non-empty run is flagged invalid.
TEST(Reporter, WriteInvalidNamesZeroChecksum) {
  std::ostringstream out;
  measurement::Snapshot snap = SyntheticSnapshot();
  snap.backpressure = 0;
  snap.checksum = 0; // non-empty run + zero checksum = invalid
  reporter::WriteInvalid(out, SyntheticEnv(), SyntheticConfig(),
                         "configs/test.ini", snap, kSyntheticSubmitterThreads,
                         SyntheticStreamStats());
  const std::string s = out.str();
  EXPECT_TRUE(Contains(s, "zero anti-DCE checksum on a non-empty run"))
      << "invalid report must name the zero-checksum reason";
}

// A submit-lag breach names the failed pacing invariant, retains the lag
// distribution for diagnosis, and suppresses the latency headline.
TEST(Reporter, WriteInvalidNamesSubmitLagAndKeepsDiagnostic) {
  std::ostringstream out;
  measurement::Snapshot snap = SyntheticSnapshot();
  snap.submitLagBreaches = 3;
  reporter::WriteInvalid(out, SyntheticEnv(), SyntheticConfig(),
                         "configs/test.ini", snap, kSyntheticSubmitterThreads,
                         SyntheticStreamStats());
  const std::string s = out.str();
  EXPECT_TRUE(Contains(s, "submit scheduling lag limit exceeded x 3"));
  EXPECT_TRUE(Contains(s, "Submit scheduling lag"));
  EXPECT_TRUE(Contains(s, "breaches: 3"));
  EXPECT_FALSE(Contains(s, "=== Headline:"));
}

// Pins compact duration-knob rendering. A fractional `idle_cleanup` keeps its
// fraction, while a sub-minute `slow_submit_threshold` renders as a duration
// string instead of a millisecond float.
TEST(Reporter, DurationKnobsUseCompactForm) {
  std::ostringstream out;
  config::Config cfg = SyntheticConfig();
  cfg.asyncEngine.idleCleanup = milliseconds(2500);
  cfg.asyncEngine.slowSubmitThreshold = milliseconds(90000); // 1m30s
  reporter::Write(out, SyntheticEnv(), cfg, "configs/test.ini",
                  SyntheticSnapshot(), kSyntheticSubmitterThreads,
                  SyntheticStreamStats());
  const std::string s = out.str();
  EXPECT_TRUE(Contains(s, "idle_cleanup (queue retire)   : 2.5s"))
      << "idle_cleanup must render 2500ms as 2.5s";
  EXPECT_FALSE(Contains(s, "idle_cleanup (queue retire)   : 2s"))
      << "idle_cleanup must not render 2500ms as 2s";
  EXPECT_TRUE(Contains(s, "slow_submit_threshold         : 1m30s"))
      << "slow_submit_threshold must render 90000ms as 1m30s";
}

} // namespace
