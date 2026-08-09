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

#include "spot_loadtest/measurement/histogram.hpp"

#include <chrono>
#include <cstdint>
#include <functional>

// Harness self-overhead characterisation.
//
// `MeasureOverhead` runs a prober a fixed number of times sequentially (no
// concurrency, so the probe sees no workload queueing) against a quiescent
// engine, and returns the latency distribution. The driver probes it through
// ApplyAccountAdjustment (NOT ExecutePreTrade), so this characterises the
// adjustment-path FFI+queue floor, not the order-check path.

namespace spot_loadtest::measurement {

// Submits one trivial operation through the async engine and returns the
// submit->decision latency. Returns a negative duration to signal an error.
using OverheadProber = std::function<std::chrono::nanoseconds()>;

// The result of the harness self-overhead characterisation.
struct OverheadSummary {
  int probes = 0;
  Percentiles distribution;
  std::int64_t clamped = 0;
};

// Runs `prober` `probeCount` times sequentially and returns the summary.
[[nodiscard]] inline OverheadSummary
MeasureOverhead(int probeCount, const OverheadProber &prober) {
  Histogram h;
  std::int64_t clamped = 0;
  for (int i = 0; i < probeCount; ++i) {
    const std::chrono::nanoseconds d = prober();
    if (d.count() < 0) {
      // Prober error: return what we have (none).
      break;
    }
    const std::int64_t ns = d.count() < 1 ? 1 : d.count();
    if (h.RecordClamped(ns)) {
      ++clamped;
    }
  }
  OverheadSummary out;
  out.probes = static_cast<int>(h.TotalCount());
  out.distribution = h.Extract();
  out.clamped = clamped;
  return out;
}

} // namespace spot_loadtest::measurement
