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

#include <atomic>
#include <chrono>
#include <cstdint>
#include <mutex>

// Observer inner-metrics accumulation (diagnostic, separate from the headline).
//
// `ObserverSink` accumulates queue_wait and engine_compute durations reported
// by the asyncengine observer callbacks into SEPARATE diagnostic histograms
// kept strictly out of the headline streams. These are per-account AGGREGATE
// distributions, not correlated to a specific order.

namespace spot_loadtest::measurement {

// The immutable diagnostic snapshot of the observer inner metrics.
struct InnerMetrics {
  Percentiles queueWait;
  Percentiles engineCompute;
  std::int64_t queuesCreated = 0;
  std::int64_t queuesRemoved = 0;
  std::int64_t dequeues = 0;
  std::int64_t completes = 0;
  std::int64_t clamped = 0;
};

class ObserverSink {
public:
  ObserverSink() = default;

  void RecordDequeue(std::chrono::nanoseconds waited) {
    const std::int64_t ns = ToNsClamp(waited);
    {
      std::lock_guard<std::mutex> lock(m_mutex);
      if (m_queueWait.RecordClamped(ns)) {
        ++m_clamped;
      }
    }
    m_dequeues.fetch_add(1, std::memory_order_relaxed);
  }

  void RecordComplete(std::chrono::nanoseconds ran) {
    const std::int64_t ns = ToNsClamp(ran);
    {
      std::lock_guard<std::mutex> lock(m_mutex);
      if (m_engineCompute.RecordClamped(ns)) {
        ++m_clamped;
      }
    }
    m_completes.fetch_add(1, std::memory_order_relaxed);
  }

  void RecordQueueCreated() {
    m_queuesCreated.fetch_add(1, std::memory_order_relaxed);
  }
  void RecordQueueRemoved() {
    m_queuesRemoved.fetch_add(1, std::memory_order_relaxed);
  }

  // An immutable copy of the inner metrics. Call after the run has drained.
  [[nodiscard]] InnerMetrics Snapshot() {
    InnerMetrics m;
    {
      std::lock_guard<std::mutex> lock(m_mutex);
      m.queueWait = m_queueWait.Extract();
      m.engineCompute = m_engineCompute.Extract();
      m.clamped = m_clamped;
    }
    m.queuesCreated = m_queuesCreated.load(std::memory_order_relaxed);
    m.queuesRemoved = m_queuesRemoved.load(std::memory_order_relaxed);
    m.dequeues = m_dequeues.load(std::memory_order_relaxed);
    m.completes = m_completes.load(std::memory_order_relaxed);
    return m;
  }

private:
  // Aborted tasks report ran = 0; recorded as 1 ns to stay in range.
  [[nodiscard]] static std::int64_t ToNsClamp(std::chrono::nanoseconds d) {
    const std::int64_t ns = d.count();
    return ns < 1 ? 1 : ns;
  }

  std::mutex m_mutex;
  Histogram m_queueWait;
  Histogram m_engineCompute;
  std::atomic<std::int64_t> m_queuesCreated{0};
  std::atomic<std::int64_t> m_queuesRemoved{0};
  std::atomic<std::int64_t> m_dequeues{0};
  std::atomic<std::int64_t> m_completes{0};
  std::int64_t m_clamped = 0;
};

} // namespace spot_loadtest::measurement
