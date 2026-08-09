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
#include <memory>
#include <mutex>
#include <vector>

// Sliding-window HdrHistogram management.
//
// Every headline sample is recorded into BOTH the current window histogram AND
// a run-level merged histogram. Windows use reproducible operation-count
// boundaries. Run-level diagnostic histograms are kept separately.

namespace spot_loadtest::measurement {

using Clock = std::chrono::steady_clock;

// The immutable record of one completed window. OrderCheckHist / SettlementHist
// are independent raw-histogram copies so callers can perform a lossless Merge
// across a window range and read exact percentiles from the merged result.
struct WindowSnapshot {
  int index = 0;
  Percentiles orderCheck;
  Percentiles settlement;
  Clock::time_point wallStart{};
  Clock::time_point wallEnd{};
  std::shared_ptr<Histogram> orderCheckHist;
  std::shared_ptr<Histogram> settlementHist;
};

// Converts a duration to int64 nanoseconds, clamping to 1 if <= 0 so
// RecordValue never fails on a valid duration.
[[nodiscard]] inline std::int64_t ToNs(std::chrono::nanoseconds d) {
  const std::int64_t ns = d.count();
  return ns < 1 ? 1 : ns;
}

// Manages the sliding-window histograms for order-check and settlement
// latencies. Safe for concurrent use from multiple collector threads.
class Windows {
public:
  explicit Windows(std::int64_t opsSize);

  // Records one order-check latency, advancing the window on a boundary.
  void RecordOrderCheck(std::chrono::nanoseconds d);
  // Records one settlement latency (does not drive window rotation).
  void RecordSettlement(std::chrono::nanoseconds d);
  // Records one order-check SERVICE-TIME diagnostic (run-level merged only).
  void RecordServiceTime(std::chrono::nanoseconds d);
  // Records one submit scheduling-lag diagnostic (run-level merged only).
  void RecordSubmitLag(std::chrono::nanoseconds d);

  // Seals the trailing partial window and returns the full picture: per-window
  // history and the merged percentiles for each stream. Call once after drain.
  void Snapshot(std::vector<WindowSnapshot> &outWindows,
                Percentiles &outOrderCheck, Percentiles &outSettlement);

  // The merged order-check service-time diagnostic percentiles.
  [[nodiscard]] Percentiles ServiceTime();
  // The merged submit scheduling-lag diagnostic percentiles.
  [[nodiscard]] Percentiles SubmitLag();

  // Samples saturated to the ceiling rather than dropped, across all windowed
  // streams.
  [[nodiscard]] std::int64_t ClampedSamples();

private:
  struct WindowState {
    Histogram orderCheck;
    Histogram settlement;
    Clock::time_point start;
    std::int64_t opCount = 0;
  };

  [[nodiscard]] WindowSnapshot SealCurrent(int index);
  void MaybeRotate(); // called with m_mutex held.

  std::mutex m_mutex;
  std::int64_t m_opsSize;

  WindowState m_current;
  std::vector<WindowSnapshot> m_completed;
  int m_index = 0;

  Histogram m_mergedOrderCheck;
  Histogram m_mergedSettlement;
  Histogram m_mergedServiceTime;
  Histogram m_mergedSubmitLag;
  std::int64_t m_clamped = 0;
};

} // namespace spot_loadtest::measurement
