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

// Measurement layer tests.
//

#include "spot_loadtest/measurement/overhead.hpp"
#include "spot_loadtest/measurement/sink.hpp"
#include "spot_loadtest/measurement/snapshot.hpp"
#include "spot_loadtest/measurement/window.hpp"

#include <gtest/gtest.h>

#include <chrono>
#include <thread>
#include <vector>

namespace {

namespace m = spot_loadtest::measurement;
using std::chrono::microseconds;
using std::chrono::milliseconds;
using std::chrono::nanoseconds;

// A latency value supplied by the driver lands in the histogram exactly as
// given, with no recomputation (the coordinated-omission correctness test).
TEST(Measurement, COSamplePassthrough) {
  m::Windows w(10000);
  m::Sink s(&w);

  const auto syntheticProcessing = milliseconds(3);
  const auto latency =
      std::chrono::duration_cast<nanoseconds>(syntheticProcessing);

  s.RecordSubmit();
  s.RecordOrderCheck(latency, true);

  std::vector<m::WindowSnapshot> windows;
  m::Percentiles ocMerged;
  m::Percentiles setMerged;
  w.Snapshot(windows, ocMerged, setMerged);

  ASSERT_EQ(ocMerged.count, 1);
  const auto tol =
      std::chrono::duration_cast<nanoseconds>(syntheticProcessing) / 1000;
  const auto diff =
      ocMerged.p50 > latency ? ocMerged.p50 - latency : latency - ocMerged.p50;
  EXPECT_LE(diff, tol);
}

// Windows rotate correctly on op-count boundaries.
TEST(Measurement, WindowingByOps) {
  constexpr int kWindowSize = 100;
  m::Windows w(kWindowSize);
  for (int i = 0; i < 350; ++i) {
    w.RecordOrderCheck(microseconds(i + 1));
  }
  std::vector<m::WindowSnapshot> snaps;
  m::Percentiles oc;
  m::Percentiles set;
  w.Snapshot(snaps, oc, set);
  // 350 ops with a window of 100 = 3 complete + 1 partial.
  ASSERT_EQ(snaps.size(), 4u);
  for (int i = 0; i < 3; ++i) {
    EXPECT_EQ(snaps[static_cast<std::size_t>(i)].orderCheck.count, kWindowSize);
  }
  EXPECT_EQ(snaps[3].orderCheck.count, 50);
}

// The merged histogram contains exactly as many samples as the sum of all
// window counts.
TEST(Measurement, MergedPercentilesMatchWindowUnion) {
  m::Windows w(50);
  constexpr int kTotal = 130;
  for (int i = 0; i < kTotal; ++i) {
    w.RecordOrderCheck(microseconds(i + 1));
  }
  std::vector<m::WindowSnapshot> snaps;
  m::Percentiles merged;
  m::Percentiles set;
  w.Snapshot(snaps, merged, set);

  long long windowTotal = 0;
  for (const auto &s : snaps) {
    windowTotal += s.orderCheck.count;
  }
  EXPECT_EQ(windowTotal, kTotal);
  EXPECT_EQ(merged.count, kTotal);
}

// Merging ALL per-window raw histograms reproduces the all-run merged
// percentiles bit-for-bit (the steady-state lossless-merge invariant).
TEST(Measurement, MergeWindowRangeLossless) {
  constexpr int kWindowSize = 50;
  m::Windows w(kWindowSize);
  for (int i = 0; i < 220; ++i) {
    w.RecordOrderCheck(microseconds(i + 1));
    w.RecordSettlement(microseconds((i + 1) * 2));
  }
  std::vector<m::WindowSnapshot> snaps;
  m::Percentiles allRunOC;
  m::Percentiles allRunSet;
  w.Snapshot(snaps, allRunOC, allRunSet);

  m::Percentiles ssOC;
  m::Percentiles ssSet;
  m::MergeWindowRange(snaps, 0, ssOC, ssSet);

  EXPECT_EQ(ssOC.count, allRunOC.count);
  EXPECT_EQ(ssOC.p50, allRunOC.p50);
  EXPECT_EQ(ssOC.p99, allRunOC.p99);
  EXPECT_EQ(ssOC.p999, allRunOC.p999);
  EXPECT_EQ(ssOC.max, allRunOC.max);
  EXPECT_EQ(ssSet.count, allRunSet.count);
  EXPECT_EQ(ssSet.p50, allRunSet.p50);
  EXPECT_EQ(ssSet.max, allRunSet.max);
}

// The checksum is not constant after multiple distinct records (anti-DCE).
TEST(Measurement, ChecksumChangesOnEachRecord) {
  m::Windows w(10000);
  m::Sink s(&w);

  s.RecordSubmit();
  s.RecordOrderCheck(milliseconds(1), true);
  const m::SinkStats stats1 = s.Stats();

  s.RecordSubmit();
  s.RecordOrderCheck(milliseconds(2), false);
  const m::SinkStats stats2 = s.Stats();

  EXPECT_NE(stats1.checksum, stats2.checksum);
  EXPECT_EQ(stats2.orderChecks, 2u);
}

// RecordSubmit / RecordOrderCheck keep the in-flight counter and peak
// consistent.
TEST(Measurement, InFlight) {
  m::Windows w(10000);
  m::Sink s(&w);
  s.RecordSubmit();
  s.RecordSubmit();
  s.RecordSubmit();
  s.RecordOrderCheck(milliseconds(1), true);
  s.RecordOrderCheck(milliseconds(1), false);
  s.RecordOrderCheck(milliseconds(1), true);
  EXPECT_GE(s.Stats().maxInFlight, 2);
}

// Submit lag is measured independently from engine latency, and every sample
// beyond the configured limit invalidates the run.
TEST(Measurement, SubmitLagHistogramAndBreaches) {
  m::Windows w(10000);
  m::Sink s(&w);
  const auto maxLag = milliseconds(1);

  s.RecordSubmitLag(microseconds(400), maxLag);
  s.RecordSubmitLag(maxLag, maxLag);
  s.RecordSubmitLag(milliseconds(2), maxLag);

  const m::Snapshot snap = m::Build(w, s, nullptr, m::OverheadSummary{});
  EXPECT_EQ(snap.submitLag.count, 3);
  EXPECT_EQ(snap.submitLagBreaches, 1u);
  EXPECT_GE(snap.submitLag.max, milliseconds(2));
}

// Concurrent RecordDequeue / RecordComplete are race-free and counted.
TEST(Measurement, ObserverSinkRaceClean) {
  m::ObserverSink obs;
  // `static` keeps the constants usable inside the lambdas below without an
  // explicit capture: MSVC rejects the implicit use otherwise (C3493).
  static constexpr int kWorkerThreads = 10;
  static constexpr int kCallsEach = 500;
  std::vector<std::thread> ts;
  for (int i = 0; i < kWorkerThreads; ++i) {
    ts.emplace_back([&obs] {
      for (int j = 0; j < kCallsEach; ++j) {
        obs.RecordDequeue(microseconds(j + 1));
      }
    });
    ts.emplace_back([&obs] {
      for (int j = 0; j < kCallsEach; ++j) {
        obs.RecordComplete(microseconds(j + 1));
      }
    });
  }
  for (auto &t : ts) {
    t.join();
  }
  const m::InnerMetrics im = obs.Snapshot();
  EXPECT_EQ(im.dequeues, kWorkerThreads * kCallsEach);
  EXPECT_EQ(im.completes, kWorkerThreads * kCallsEach);
}

// RecordSubmit / RecordOrderCheck are race-free under concurrent collectors.
TEST(Measurement, SinkRaceClean) {
  m::Windows w(10000);
  m::Sink s(&w);
  // See `ObserverSinkRaceClean`: `static` avoids the MSVC C3493 capture rule.
  static constexpr int kWorkerThreads = 8;
  static constexpr int kOpsEach = 200;
  std::vector<std::thread> ts;
  for (int i = 0; i < kWorkerThreads; ++i) {
    ts.emplace_back([&s, i] {
      for (int j = 0; j < kOpsEach; ++j) {
        s.RecordSubmit();
        s.RecordOrderCheck(microseconds(i * kOpsEach + j + 1), j % 2 == 0);
      }
    });
  }
  for (auto &t : ts) {
    t.join();
  }
  EXPECT_EQ(s.Stats().orderChecks,
            static_cast<unsigned>(kWorkerThreads * kOpsEach));
}

// Steady-state percentiles exclude warmup: p90/p99 are lower than the all-run
// figures that include the warmup spike.
TEST(Measurement, SteadyStateConsistency) {
  constexpr int kWindowSize = 50;
  m::Windows w(kWindowSize);
  m::Sink s(&w);
  for (int i = 0; i < kWindowSize; ++i) {
    s.RecordSubmit();
    s.RecordOrderCheck(milliseconds(10), true);
  }
  for (int i = 0; i < kWindowSize + 1; ++i) {
    s.RecordSubmit();
    s.RecordOrderCheck(microseconds(100), true);
  }
  const m::Snapshot snap = m::Build(w, s, nullptr, m::OverheadSummary{});
  ASSERT_GE(snap.windows.size(), 2u);
  ASSERT_EQ(snap.warmupWindows, 1);
  EXPECT_LT(snap.steadyStateOrderCheck.p90, snap.orderCheck.p90);
  EXPECT_LT(snap.steadyStateOrderCheck.p99, milliseconds(1));
  EXPECT_GE(snap.orderCheck.p99, milliseconds(1));
}

// AchievedRejectRate is order-check rejects / order-check decisions;
// settlements are excluded from both numerator and denominator.
TEST(Measurement, AchievedRejectRateOrderCheckOnly) {
  constexpr int kTotalOrderChecks = 100;
  constexpr int kOrderCheckRejects = 5;
  constexpr int kTotalSettlements = 95;
  constexpr int kSettlementRejects = 10;

  m::Windows w(10000);
  m::Sink s(&w);
  for (int i = 0; i < kOrderCheckRejects; ++i) {
    s.RecordSubmit();
    s.RecordOrderCheck(milliseconds(1), false);
  }
  for (int i = 0; i < kTotalOrderChecks - kOrderCheckRejects; ++i) {
    s.RecordSubmit();
    s.RecordOrderCheck(milliseconds(1), true);
  }
  for (int i = 0; i < kSettlementRejects; ++i) {
    s.RecordSubmit();
    s.RecordSettlement(milliseconds(1), false);
  }
  for (int i = 0; i < kTotalSettlements - kSettlementRejects; ++i) {
    s.RecordSubmit();
    s.RecordSettlement(milliseconds(1), true);
  }
  const m::Snapshot snap = m::Build(w, s, nullptr, m::OverheadSummary{});
  EXPECT_DOUBLE_EQ(snap.achievedRejectRate, 0.05);
  EXPECT_EQ(snap.totalRejects, static_cast<unsigned>(kOrderCheckRejects));
}

// The divide-by-zero guard: no order checks => achieved rate 0.
TEST(Measurement, AchievedRejectRateZeroOrderChecks) {
  m::Windows w(10000);
  m::Sink s(&w);
  for (int i = 0; i < 10; ++i) {
    s.RecordSubmit();
    s.RecordSettlement(milliseconds(1), i % 2 == 0);
  }
  const m::Snapshot snap = m::Build(w, s, nullptr, m::OverheadSummary{});
  EXPECT_DOUBLE_EQ(snap.achievedRejectRate, 0.0);
}

// MeasureOverhead calls the prober the requested number of times and populates
// the summary.
TEST(Measurement, OverheadProbe) {
  constexpr int kProbeCount = 20;
  const m::OverheadSummary summary =
      m::MeasureOverhead(kProbeCount, [] { return microseconds(500); });
  EXPECT_EQ(summary.probes, kProbeCount);
  EXPECT_EQ(summary.distribution.count, kProbeCount);
  const auto target =
      std::chrono::duration_cast<nanoseconds>(microseconds(500));
  const auto tol = target / 1000;
  const auto diff = summary.distribution.p50 > target
                        ? summary.distribution.p50 - target
                        : target - summary.distribution.p50;
  EXPECT_LE(diff, tol);
}

} // namespace
