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

#include "spot_loadtest/driver/live.hpp"
#include "spot_loadtest/measurement/sink.hpp"

#include <atomic>
#include <chrono>
#include <condition_variable>
#include <cstdint>
#include <mutex>
#include <ostream>
#include <string>
#include <thread>

// Writes live run progress to an std::ostream (typically std::cerr) at a
// configurable tick interval.
//
// It reads counters from the driver's LiveSource via a race-safe accessor and
// NEVER writes to the same stream as the final report (stdout). Each tick
// overwrites the previous line using a carriage-return prefix; the final stop
// clears the line so no progress noise remains after the report prints.

namespace spot_loadtest::progress {

// The default tick interval used when the caller passes a non-positive value.
inline constexpr std::chrono::milliseconds kDefaultInterval{500};

// Builds one progress line from the provided counters, total, and elapsed time.
// Exported so tests can verify formatting without spawning a thread.
[[nodiscard]] std::string FormatLine(const measurement::LiveCounters &c,
                                     std::uint64_t total,
                                     std::chrono::nanoseconds elapsed);

// Writes periodic live progress to an output stream.
class Reporter {
public:
  // Creates a Reporter that writes to `out` every `interval`. `total` is the
  // target number of decided ops; pass 0 when unknown. interval <= 0 uses
  // 500ms.
  Reporter(std::ostream &out, const driver::LiveSource &source,
           std::uint64_t total, std::chrono::milliseconds interval);

  ~Reporter();

  Reporter(const Reporter &) = delete;
  Reporter &operator=(const Reporter &) = delete;

  // Begins the progress loop in a background thread.
  void Start(std::chrono::steady_clock::time_point start);
  // Signals the loop to exit and erases the current progress line. Blocks until
  // the thread has stopped.
  void Stop();

private:
  void Loop(std::chrono::steady_clock::time_point start);
  void Render(std::chrono::steady_clock::time_point now,
              std::chrono::steady_clock::time_point start);

  std::ostream &m_out;
  const driver::LiveSource &m_source;
  std::uint64_t m_total;
  std::chrono::milliseconds m_interval;

  std::thread m_thread;
  std::mutex m_mutex;
  std::condition_variable m_cv;
  bool m_done = false;
  bool m_started = false;
};

} // namespace spot_loadtest::progress
