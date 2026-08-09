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

#include "spot_loadtest/measurement/histogram.hpp"
#include "spot_loadtest/measurement/sink.hpp"
#include "spot_loadtest/measurement/snapshot.hpp"
#include "spot_loadtest/measurement/window.hpp"

#include <chrono>
#include <cstdint>
#include <memory>
#include <mutex>
#include <vector>

namespace spot_loadtest::measurement {

//------------------------------------------------------------------------------
// Windows

Windows::Windows(std::int64_t opsSize) : m_opsSize(opsSize) {
  m_current.start = Clock::now();
}

WindowSnapshot Windows::SealCurrent(int index) {
  // Lossless deep-copies of the live histograms: the caller owns the result and
  // the originals can keep being mutated safely.
  WindowSnapshot snap;
  snap.index = index;
  snap.orderCheck = m_current.orderCheck.Extract();
  snap.settlement = m_current.settlement.Extract();
  snap.wallStart = m_current.start;
  snap.wallEnd = Clock::now();
  snap.orderCheckHist = std::make_shared<Histogram>(m_current.orderCheck);
  snap.settlementHist = std::make_shared<Histogram>(m_current.settlement);
  return snap;
}

void Windows::MaybeRotate() {
  if (m_opsSize <= 0 || m_current.opCount < m_opsSize) {
    return;
  }
  m_completed.push_back(SealCurrent(m_index));
  ++m_index;
  m_current = WindowState{};
  m_current.start = Clock::now();
}

void Windows::RecordOrderCheck(std::chrono::nanoseconds d) {
  const std::int64_t ns = ToNs(d);
  std::lock_guard<std::mutex> lock(m_mutex);
  m_current.orderCheck.RecordClamped(ns);
  if (m_mergedOrderCheck.RecordClamped(ns)) {
    ++m_clamped;
  }
  ++m_current.opCount;
  MaybeRotate();
}

void Windows::RecordSettlement(std::chrono::nanoseconds d) {
  const std::int64_t ns = ToNs(d);
  std::lock_guard<std::mutex> lock(m_mutex);
  m_current.settlement.RecordClamped(ns);
  if (m_mergedSettlement.RecordClamped(ns)) {
    ++m_clamped;
  }
}

void Windows::RecordServiceTime(std::chrono::nanoseconds d) {
  const std::int64_t ns = ToNs(d);
  std::lock_guard<std::mutex> lock(m_mutex);
  if (m_mergedServiceTime.RecordClamped(ns)) {
    ++m_clamped;
  }
}

void Windows::RecordSubmitLag(std::chrono::nanoseconds d) {
  const std::int64_t ns = ToNs(d);
  std::lock_guard<std::mutex> lock(m_mutex);
  if (m_mergedSubmitLag.RecordClamped(ns)) {
    ++m_clamped;
  }
}

void Windows::Snapshot(std::vector<WindowSnapshot> &outWindows,
                       Percentiles &outOrderCheck, Percentiles &outSettlement) {
  std::lock_guard<std::mutex> lock(m_mutex);
  if (m_current.orderCheck.TotalCount() > 0 ||
      m_current.settlement.TotalCount() > 0) {
    m_completed.push_back(SealCurrent(m_index));
  }
  outWindows = m_completed;
  outOrderCheck = m_mergedOrderCheck.Extract();
  outSettlement = m_mergedSettlement.Extract();
}

Percentiles Windows::ServiceTime() {
  std::lock_guard<std::mutex> lock(m_mutex);
  return m_mergedServiceTime.Extract();
}

Percentiles Windows::SubmitLag() {
  std::lock_guard<std::mutex> lock(m_mutex);
  return m_mergedSubmitLag.Extract();
}

std::int64_t Windows::ClampedSamples() {
  std::lock_guard<std::mutex> lock(m_mutex);
  return m_clamped;
}

//------------------------------------------------------------------------------
// Sink

namespace {
// 64-bit golden-ratio odd constant used to spread bits through the checksum.
constexpr std::uint64_t kChecksumMix = 0x9e3779b97f4a7c15ULL;

[[nodiscard]] std::int64_t ClampPositive(std::int64_t n) {
  return n < 0 ? 0 : n;
}
} // namespace

void Sink::RecordSubmit() {
  bool expected = false;
  if (m_wallStartSet.compare_exchange_strong(expected, true)) {
    std::lock_guard<std::mutex> lock(m_mutex);
    m_wallStart = Clock::now();
  }
  std::lock_guard<std::mutex> lock(m_mutex);
  ++m_inFlight;
  if (m_inFlight > m_maxInFlight) {
    m_maxInFlight = m_inFlight;
  }
}

void Sink::RecordOrderCheck(std::chrono::nanoseconds latency, bool accepted) {
  m_windows->RecordOrderCheck(latency);
  std::lock_guard<std::mutex> lock(m_mutex);
  --m_inFlight;
  ++m_orderChecks;
  if (accepted) {
    ++m_orderCheckAccepts;
  } else {
    ++m_orderCheckRejects;
  }
  const std::uint64_t bit = accepted ? 1U : 0U;
  m_checksum ^=
      (static_cast<std::uint64_t>(latency.count()) + kChecksumMix) ^ bit;
  m_wallEnd = Clock::now();
}

void Sink::RecordSettlement(std::chrono::nanoseconds latency, bool accepted) {
  m_windows->RecordSettlement(latency);
  std::lock_guard<std::mutex> lock(m_mutex);
  --m_inFlight;
  ++m_settlements;
  if (accepted) {
    ++m_settlementAccepts;
  } else {
    ++m_settlementBlocks;
  }
  constexpr std::uint64_t kSettlementSlot = 2;
  m_checksum ^= (static_cast<std::uint64_t>(latency.count()) + kChecksumMix) ^
                kSettlementSlot;
  m_wallEnd = Clock::now();
}

void Sink::RecordServiceTime(std::chrono::nanoseconds latency) {
  m_windows->RecordServiceTime(latency);
}

void Sink::RecordSubmitLag(std::chrono::nanoseconds lag,
                           std::chrono::nanoseconds maxLag) {
  m_windows->RecordSubmitLag(lag);
  if (lag <= maxLag) {
    return;
  }
  std::lock_guard<std::mutex> lock(m_mutex);
  ++m_submitLagBreaches;
}

void Sink::RecordFunding(bool accepted) {
  std::lock_guard<std::mutex> lock(m_mutex);
  --m_inFlight;
  ++m_fundings;
  if (accepted) {
    ++m_fundingAccepts;
  } else {
    ++m_fundingRejects;
  }
  constexpr std::uint64_t kFundingSlot = 4;
  const std::uint64_t bit = accepted ? 1U : 0U;
  m_checksum ^= (kChecksumMix ^ kFundingSlot) ^ bit;
  m_wallEnd = Clock::now();
}

void Sink::RecordBackpressure(std::chrono::nanoseconds latency) {
  std::lock_guard<std::mutex> lock(m_mutex);
  --m_inFlight;
  ++m_backpressure;
  constexpr std::uint64_t kBackpressureSlot = 3;
  m_checksum ^= (static_cast<std::uint64_t>(latency.count()) + kChecksumMix) ^
                kBackpressureSlot;
  m_wallEnd = Clock::now();
}

void Sink::RecordHandoffStall() {
  std::lock_guard<std::mutex> lock(m_mutex);
  ++m_handoffStalls;
}

void Sink::RecordWorkOverflowDepth(int depth) {
  std::lock_guard<std::mutex> lock(m_mutex);
  if (depth > m_maxWorkOverflow) {
    m_maxWorkOverflow = depth;
  }
}

SinkStats Sink::Stats() {
  std::lock_guard<std::mutex> lock(m_mutex);
  SinkStats s;
  s.orderChecks = m_orderChecks;
  s.orderCheckAccepts = m_orderCheckAccepts;
  s.orderCheckRejects = m_orderCheckRejects;
  s.settlements = m_settlements;
  s.settlementAccepts = m_settlementAccepts;
  s.settlementBlocks = m_settlementBlocks;
  s.fundings = m_fundings;
  s.fundingAccepts = m_fundingAccepts;
  s.fundingRejects = m_fundingRejects;
  s.submitLagBreaches = m_submitLagBreaches;
  s.backpressure = m_backpressure;
  s.handoffStalls = m_handoffStalls;
  s.maxWorkOverflow = m_maxWorkOverflow;
  s.checksum = m_checksum;
  s.maxInFlight = m_maxInFlight;
  s.wallStart = m_wallStart;
  s.wallEnd = m_wallEnd;
  s.wallStartSet = m_wallStartSet.load(std::memory_order_relaxed);
  return s;
}

LiveCounters Sink::Live() {
  std::lock_guard<std::mutex> lock(m_mutex);
  const std::uint64_t decided = m_orderChecks + m_settlements + m_fundings;
  LiveCounters c;
  c.decided = decided;
  c.submitted = decided + static_cast<std::uint64_t>(ClampPositive(m_inFlight));
  c.inFlight = m_inFlight;
  return c;
}

//------------------------------------------------------------------------------
// Snapshot

namespace {
// 1 warmup window when there is more than one window, 0 otherwise.
[[nodiscard]] int
WarmupWindowCount(const std::vector<WindowSnapshot> &windows) {
  return windows.size() <= 1 ? 0 : 1;
}
} // namespace

void MergeWindowRange(const std::vector<WindowSnapshot> &windows, int start,
                      Percentiles &outOrderCheck, Percentiles &outSettlement) {
  Histogram oc;
  Histogram set;
  for (std::size_t i = static_cast<std::size_t>(start); i < windows.size();
       ++i) {
    if (windows[i].orderCheckHist) {
      oc.Merge(*windows[i].orderCheckHist);
    }
    if (windows[i].settlementHist) {
      set.Merge(*windows[i].settlementHist);
    }
  }
  outOrderCheck = oc.Extract();
  outSettlement = set.Extract();
}

Snapshot Build(Windows &windows, Sink &sink, ObserverSink *observer,
               const OverheadSummary &overhead) {
  std::vector<WindowSnapshot> windowSnaps;
  Percentiles ocMerged;
  Percentiles setMerged;
  windows.Snapshot(windowSnaps, ocMerged, setMerged);
  const SinkStats stats = sink.Stats();

  double achieved = 0.0;
  if (stats.orderChecks > 0) {
    achieved = static_cast<double>(stats.orderCheckRejects) /
               static_cast<double>(stats.orderChecks);
  }

  double throughput = 0.0;
  const std::uint64_t total = stats.orderChecks + stats.settlements;
  if (stats.wallStartSet && stats.wallEnd > stats.wallStart) {
    const double elapsed =
        std::chrono::duration<double>(stats.wallEnd - stats.wallStart).count();
    if (elapsed > 0) {
      throughput = static_cast<double>(total) / elapsed;
    }
  }

  const int warmup = WarmupWindowCount(windowSnaps);
  Percentiles ssOC;
  Percentiles ssSet;
  MergeWindowRange(windowSnaps, warmup, ssOC, ssSet);

  std::int64_t clamped = windows.ClampedSamples() + overhead.clamped;

  Snapshot snap;
  snap.windows = std::move(windowSnaps);
  snap.orderCheck = ocMerged;
  snap.settlement = setMerged;
  snap.steadyStateOrderCheck = ssOC;
  snap.steadyStateSettlement = ssSet;
  snap.warmupWindows = warmup;
  snap.serviceTime = windows.ServiceTime();
  snap.submitLag = windows.SubmitLag();
  snap.throughput = throughput;
  snap.totalOrderChecks = stats.orderChecks;
  snap.totalSettlements = stats.settlements;
  snap.totalAccepts = stats.orderCheckAccepts;
  snap.totalRejects = stats.orderCheckRejects;
  snap.totalSettlementAccepts = stats.settlementAccepts;
  snap.totalSettlementBlocks = stats.settlementBlocks;
  snap.totalFundings = stats.fundings;
  snap.totalFundingAccepts = stats.fundingAccepts;
  snap.totalFundingRejects = stats.fundingRejects;
  snap.achievedRejectRate = achieved;
  snap.maxInFlight = stats.maxInFlight;
  snap.submitLagBreaches = stats.submitLagBreaches;
  snap.backpressure = stats.backpressure;
  snap.handoffStalls = stats.handoffStalls;
  snap.maxWorkOverflow = stats.maxWorkOverflow;
  snap.clampedSamples = clamped;
  snap.checksum = stats.checksum;
  snap.overhead = overhead;
  snap.wallStart = stats.wallStart;
  snap.wallEnd = stats.wallEnd;
  snap.wallStartSet = stats.wallStartSet;
  if (observer != nullptr) {
    snap.innerMetrics = observer->Snapshot();
    snap.clampedSamples += snap.innerMetrics.clamped;
  }
  return snap;
}

} // namespace spot_loadtest::measurement
