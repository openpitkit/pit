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

#include <chrono>
#include <cstdint>
#include <vector>

// HdrHistogram-backed latency recording.
//
// A self-contained HdrHistogram uses log-linear buckets with configurable
// significant figures. Every latency histogram uses [1 µs .. 60 s] at three
// significant figures.
//
// # Out-of-range clamping (methodology invariant)
//
// A bare record of an over-range value would SILENTLY DROP exactly the
// coordinated-omission upper tail. Every record therefore goes through
// `RecordClamped`, which clamps the value to the histogram's highest trackable
// value BEFORE recording: an over-range sample SATURATES at the ceiling and its
// COUNT is preserved (only the exact magnitude above the ceiling is lost). The
// number of clamped samples is surfaced in the report; when non-zero, the tail
// percentiles at or above the ceiling are a lower bound on the true latency.

namespace spot_loadtest::measurement {

using std::chrono::nanoseconds;

// Histogram value range in nanoseconds (see file doc): [1 µs .. 60 s] at 3
// significant figures.
inline constexpr std::int64_t kHistMinNs = 1'000;                // 1 µs
inline constexpr std::int64_t kHistMaxNs = 60LL * 1'000'000'000; // 60 s
inline constexpr int kHistSigFig = 3;

// The five-point percentile set derived from one histogram.
struct Percentiles {
  nanoseconds p50{0};
  nanoseconds p90{0};
  nanoseconds p99{0};
  nanoseconds p999{0};
  nanoseconds max{0};
  std::int64_t count = 0;
};

// An HdrHistogram over [lowest, highest] at `sigFig` significant
// figures, recording int64 nanosecond values.
class Histogram {
public:
  Histogram(std::int64_t lowest, std::int64_t highest, int sigFig);

  // Convenience: the standard [1 µs .. 60 s] @ 3 sig-fig latency histogram.
  Histogram() : Histogram(kHistMinNs, kHistMaxNs, kHistSigFig) {}

  // Records `value`, clamping to the highest trackable value first so an
  // over-range sample saturates at the ceiling instead of being dropped.
  // Returns true when a clamp occurred so the caller can surface the count.
  bool RecordClamped(std::int64_t value);

  // Merges every recorded sample of `other` into this histogram. Both must
  // share identical bucketing (same lowest/highest/sigFig), which they do for
  // every latency histogram here.
  void Merge(const Histogram &other);

  [[nodiscard]] std::int64_t HighestTrackableValue() const noexcept {
    return m_highest;
  }
  [[nodiscard]] std::int64_t TotalCount() const noexcept { return m_total; }

  // The recorded value at the given quantile percentage (e.g. 99.9). Returns 0
  // for an empty histogram.
  [[nodiscard]] std::int64_t ValueAtQuantile(double quantile) const;

  // The highest recorded value (its bucket's upper-equivalent), 0 when empty.
  [[nodiscard]] std::int64_t Max() const;

  // Extracts the standard p50/p90/p99/p99.9/max percentile set.
  [[nodiscard]] Percentiles Extract() const;

private:
  [[nodiscard]] int CountsIndexFor(std::int64_t value) const;
  [[nodiscard]] std::int64_t ValueFromIndex(int index) const;
  [[nodiscard]] std::int64_t LowestEquivalentValue(std::int64_t value) const;
  [[nodiscard]] std::int64_t HighestEquivalentValue(std::int64_t value) const;
  [[nodiscard]] std::int64_t
  SizeOfEquivalentValueRange(std::int64_t value) const;
  [[nodiscard]] std::int64_t MedianEquivalentValue(std::int64_t value) const;

  std::int64_t m_highest;
  std::int32_t m_subBucketHalfCountMagnitude;
  std::int32_t m_subBucketHalfCount;
  std::int32_t m_subBucketCount;
  std::int32_t m_subBucketMask;
  std::int32_t m_unitMagnitude;
  std::int32_t m_bucketCount;
  std::int32_t m_countsLen;
  std::int64_t m_total = 0;
  std::vector<std::int64_t> m_counts;
};

} // namespace spot_loadtest::measurement
