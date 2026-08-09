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

#include "spot_loadtest/measurement/observer.hpp"
#include "spot_loadtest/measurement/overhead.hpp"
#include "spot_loadtest/measurement/sink.hpp"
#include "spot_loadtest/measurement/window.hpp"

#include <chrono>
#include <cstdint>
#include <vector>

// The complete post-run measurement picture.
//
// Carries per-window trajectory data, merged percentiles for the headline
// streams, throughput, reject rate, harness overhead, inner metrics, and the
// anti-DCE checksum. The steady-state headline is computed by a LOSSLESS Merge
// of the raw per-window histograms; aggregating per-window percentile
// point-values (percentile-of-percentiles) is statistically invalid and is
// never done.

namespace spot_loadtest::measurement {

struct Snapshot {
  std::vector<WindowSnapshot> windows;

  Percentiles orderCheck;
  Percentiles settlement;

  Percentiles steadyStateOrderCheck;
  Percentiles steadyStateSettlement;
  int warmupWindows = 0;

  Percentiles serviceTime;
  Percentiles submitLag;

  double throughput = 0.0;

  std::uint64_t totalOrderChecks = 0;
  std::uint64_t totalSettlements = 0;
  std::uint64_t totalAccepts = 0;
  std::uint64_t totalRejects = 0;
  std::uint64_t totalSettlementAccepts = 0;
  std::uint64_t totalSettlementBlocks = 0;
  std::uint64_t totalFundings = 0;
  std::uint64_t totalFundingAccepts = 0;
  std::uint64_t totalFundingRejects = 0;

  double achievedRejectRate = 0.0;

  std::int64_t maxInFlight = 0;
  std::uint64_t submitLagBreaches = 0;
  std::uint64_t backpressure = 0;
  std::uint64_t handoffStalls = 0;
  int maxWorkOverflow = 0;

  std::int64_t clampedSamples = 0;
  std::uint64_t checksum = 0;

  OverheadSummary overhead;
  InnerMetrics innerMetrics;

  Clock::time_point wallStart{};
  Clock::time_point wallEnd{};
  bool wallStartSet = false;
};

// Merges the raw histograms from windows[start:] and returns exact percentiles
// for order-check and settlement. The correct primitive for the steady-state
// headline (lossless Merge avoids percentile-of-percentiles invalidity).
void MergeWindowRange(const std::vector<WindowSnapshot> &windows, int start,
                      Percentiles &outOrderCheck, Percentiles &outSettlement);

// Assembles a Snapshot from the Windows, Sink, and optional ObserverSink after
// the run has fully drained. `observer` may be nullptr when Observer = false.
[[nodiscard]] Snapshot Build(Windows &windows, Sink &sink,
                             ObserverSink *observer,
                             const OverheadSummary &overhead);

} // namespace spot_loadtest::measurement
