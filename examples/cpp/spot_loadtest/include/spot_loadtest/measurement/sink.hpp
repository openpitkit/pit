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

#include "spot_loadtest/measurement/window.hpp"

#include <atomic>
#include <chrono>
#include <cstdint>
#include <mutex>

// Result-sink accumulation with an anti-DCE checksum.
//
// `Sink` accumulates resolved operation measurements into HdrHistogram windows,
// maintains operation-class counters kept STRICTLY SEPARATE so the headline
// never mixes classes, and computes an anti-DCE checksum over every decision.
// Thread-safe; all methods may be called concurrently from collector threads.

namespace spot_loadtest::measurement {

// A race-safe snapshot of in-progress counters for the progress reporter.
struct LiveCounters {
  std::uint64_t submitted = 0;
  std::uint64_t decided = 0;
  std::int64_t inFlight = 0;
};

// The immutable summary the driver exposes after a run. Accept and reject
// tallies are kept per operation class so the report never conflates them.
struct SinkStats {
  std::uint64_t orderChecks = 0;
  std::uint64_t orderCheckAccepts = 0;
  std::uint64_t orderCheckRejects = 0;
  std::uint64_t settlements = 0;
  std::uint64_t settlementAccepts = 0;
  std::uint64_t settlementBlocks = 0;
  std::uint64_t fundings = 0;
  std::uint64_t fundingAccepts = 0;
  std::uint64_t fundingRejects = 0;
  std::uint64_t submitLagBreaches = 0;
  std::uint64_t backpressure = 0;
  std::uint64_t handoffStalls = 0;
  int maxWorkOverflow = 0;
  std::uint64_t checksum = 0;
  std::int64_t maxInFlight = 0;
  Clock::time_point wallStart{};
  Clock::time_point wallEnd{};
  bool wallStartSet = false;
};

class Sink {
public:
  explicit Sink(Windows *windows) : m_windows(windows) {}

  // Notes one more submitted (in-flight) operation and tracks the peak.
  void RecordSubmit();
  // Records one resolved order-check; folds latency + decision into the
  // checksum so neither can be elided.
  void RecordOrderCheck(std::chrono::nanoseconds latency, bool accepted);
  // Records one resolved settlement (accepted == false on an account block).
  void RecordSettlement(std::chrono::nanoseconds latency, bool accepted);
  // Records one order-check SERVICE-TIME diagnostic (never the headline).
  void RecordServiceTime(std::chrono::nanoseconds latency);
  // Records scheduling lag and whether it exceeds the configured run limit.
  void RecordSubmitLag(std::chrono::nanoseconds lag,
                       std::chrono::nanoseconds maxLag);
  // Records one resolved funding adjustment (no histogram; own tally only).
  void RecordFunding(bool accepted);
  // Records one submit the engine refused with a backpressure signal.
  void RecordBackpressure(std::chrono::nanoseconds latency);
  // Records one HARNESS handoff stall (diagnostic; not folded into checksum).
  void RecordHandoffStall();
  // Updates the peak submitter -> collector spill depth (diagnostic).
  void RecordWorkOverflowDepth(int depth);

  // An immutable copy of the current counters. Call after the run drains.
  [[nodiscard]] SinkStats Stats();

  // A race-safe snapshot of the live counters. Safe at any time during the run.
  [[nodiscard]] LiveCounters Live();

private:
  Windows *m_windows;
  std::mutex m_mutex;

  std::uint64_t m_orderChecks = 0;
  std::uint64_t m_orderCheckAccepts = 0;
  std::uint64_t m_orderCheckRejects = 0;
  std::uint64_t m_settlements = 0;
  std::uint64_t m_settlementAccepts = 0;
  std::uint64_t m_settlementBlocks = 0;
  std::uint64_t m_fundings = 0;
  std::uint64_t m_fundingAccepts = 0;
  std::uint64_t m_fundingRejects = 0;
  std::uint64_t m_submitLagBreaches = 0;
  std::uint64_t m_checksum = 0;
  std::uint64_t m_backpressure = 0;
  std::uint64_t m_handoffStalls = 0;
  int m_maxWorkOverflow = 0;

  std::int64_t m_inFlight = 0;
  std::int64_t m_maxInFlight = 0;

  Clock::time_point m_wallStart{};
  std::atomic<bool> m_wallStartSet{false};
  Clock::time_point m_wallEnd{};
};

} // namespace spot_loadtest::measurement
