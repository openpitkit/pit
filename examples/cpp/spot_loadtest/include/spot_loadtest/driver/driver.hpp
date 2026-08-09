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

#include "spot_loadtest/config/config.hpp"
#include "spot_loadtest/driver/live.hpp"
#include "spot_loadtest/generator/event.hpp"
#include "spot_loadtest/measurement/snapshot.hpp"

#include <chrono>
#include <cstddef>
#include <cstdint>
#include <stdexcept>
#include <string>

// Wires the offline generator stream into the real openpit asyncengine and
// measures the C++ FFI latency TRUE OPEN-LOOP against the generator's virtual
// causal timeline.
//
// Concurrency model (true open-loop): the driver uses
// `openpit::asyncengine::TypedAsyncEngine` with per-account queues and futures,
// plus bounded submitter, collector, and finalizer `std::thread` pools. Each
// submitter deterministically merges complete account chains assigned to its
// shard, paces every event to its VirtualT0, and submits non-blocking. A
// collector pool awaits futures and records the open-loop latency
// (resolve - VirtualT0, the headline); a finalizer pool CommitAndCloses
// accepted reservations off the measured path.

namespace spot_loadtest::driver {

// Returned by Run when the run drained cleanly but one or more submits hit the
// dispatch capacity cap (QueueLimit). Such a run is NOT a valid latency
// measurement; the caller suppresses the headline and exits non-zero.
class BackpressureInvalidRun : public std::runtime_error {
public:
  BackpressureInvalidRun()
      : std::runtime_error(
            "driver: run hit dispatch backpressure (QueueLimit); not a valid "
            "latency measurement") {}
};

class SubmitLagInvalidRun : public std::runtime_error {
public:
  SubmitLagInvalidRun()
      : std::runtime_error(
            "driver: submit scheduling lag exceeded the configured limit; "
            "not a valid latency measurement") {}
};

// Returned by Run when the run drained cleanly and resolved a non-empty set of
// ops but the anti-DCE checksum is zero (decisions not provably consumed).
class ZeroChecksumInvalidRun : public std::runtime_error {
public:
  ZeroChecksumInvalidRun()
      : std::runtime_error(
            "driver: anti-DCE checksum is zero on a non-empty run; decisions "
            "were not provably consumed; not a valid latency measurement") {}
};

// The dispatch strategy selected for the async engine.
enum class DispatchStrategy { Dynamic, Sharded };

// Tunes one driver run. Most zero values use the documented defaults.
// submitterWorkers is required for every non-empty stream.
struct Config {
  int collectors = 0; // pool draining resolved futures (0 -> default).
  int finalizers = 0; // pool finalizing accepted reservations (0 -> default).
  bool observer = false;

  std::uint64_t activeAccounts = 0; // informational (disclosure / reporting).
  std::size_t submitterWorkers = 0; // bounded account-chain scheduler pool.
  std::chrono::nanoseconds maxSubmitLag{0}; // required harness validity limit.

  DispatchStrategy dispatchStrategy = DispatchStrategy::Dynamic;
  std::uint64_t maxQueues = 0;             // Dynamic only (0 = unlimited).
  std::chrono::nanoseconds idleCleanup{0}; // Dynamic only (0 = disabled).
  int shardedWorkers = 0;                  // Sharded only (> 0 required).
  int queueCapacity = 0;                   // both (0 = engine default 1024).
  std::chrono::nanoseconds slowSubmitThreshold{0}; // both (0 = engine default).

  std::int64_t windowSize = 0; // order-check ops per window (> 0 required).

  int overheadProbes = 0; // self-overhead probes before the workload (0 = off).

  LiveSource *live = nullptr; // populated by Run before any thread starts.
};

// Derives the driver Config from the validated app config. Throws
// std::invalid_argument when a value cannot be represented by the driver or
// selects an unsupported mode.
[[nodiscard]] Config FromAppConfig(const config::Config &cfg);

// The immutable summary Run returns alongside the Snapshot.
struct Stats {
  std::uint64_t orderChecks = 0; // real order-checks + runtime fundings.
  std::uint64_t settlements = 0;
  std::uint64_t accepts = 0; // order-check decisions only.
  std::uint64_t rejects = 0;
  std::uint64_t fundings = 0;
  std::uint64_t fundingAccepts = 0;
  std::uint64_t fundingRejects = 0;
  std::uint64_t submitLagBreaches = 0;
  std::uint64_t backpressure = 0;
  std::uint64_t handoffStalls = 0;
  int maxWorkOverflow = 0;
  std::uint64_t checksum = 0;
  std::int64_t maxInFlight = 0;
  std::size_t submitterThreads = 0;
  int sampleCount = 0;
};

// The result of a driver run.
struct RunResult {
  Stats stats;
  measurement::Snapshot snapshot;
};

// Drives the whole generator stream through a freshly built engine, true
// open-loop, and returns measured stats and a full measurement Snapshot once
// every operation has resolved and the engine has stopped.
//
// Throws SubmitLagInvalidRun / BackpressureInvalidRun /
// ZeroChecksumInvalidRun for an invalid run. Use RunCollecting below when the
// caller must retain the result for diagnostics. Throws std::runtime_error on a
// hard error (oracle divergence, invariant break, engine build failure). Throws
// std::invalid_argument when a non-empty stream has no submitter worker.
[[nodiscard]] RunResult Run(const generator::Stream &stream, const Config &cfg);

// Variant that returns the Stats and Snapshot on an invalid run instead of
// throwing an invalid-run sentinel. `invalidReason` names the invalid
// condition.
[[nodiscard]] RunResult RunCollecting(const generator::Stream &stream,
                                      const Config &cfg,
                                      std::string &invalidReason);

} // namespace spot_loadtest::driver
