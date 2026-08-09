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

#include <algorithm>
#include <cmath>
#include <cstdint>

// HdrHistogram bucketing uses a unit-magnitude floor plus
// log-linear sub-buckets sized for the requested significant figures, so two
// values within the resolution band map to the same bucket. The index math
// follows the reference implementation directly (countsIndexFor,
// valueFromIndex, the equivalent-value helpers) so percentiles and the array
// bound match.

namespace spot_loadtest::measurement {
namespace {

// Count of leading zeros of a 64-bit value (value > 0). Used by the
// bucket-index computation just as the reference uses numberOfLeadingZeros.
[[nodiscard]] int CountLeadingZeros64(std::uint64_t v) {
  int n = 0;
  for (int bit = 63; bit >= 0; --bit) {
    if ((v >> bit) & 1ULL) {
      break;
    }
    ++n;
  }
  return n;
}

} // namespace

Histogram::Histogram(std::int64_t lowest, std::int64_t highest, int sigFig)
    : m_highest(highest) {
  const std::int64_t largestValueWithSingleUnitResolution =
      2 * static_cast<std::int64_t>(std::pow(10.0, sigFig));

  const auto subBucketCountMagnitude = static_cast<std::int32_t>(std::ceil(
      std::log2(static_cast<double>(largestValueWithSingleUnitResolution))));
  m_subBucketHalfCountMagnitude =
      (subBucketCountMagnitude >= 1 ? subBucketCountMagnitude : 1) - 1;

  m_unitMagnitude = static_cast<std::int32_t>(
      std::floor(std::log2(static_cast<double>(lowest))));

  m_subBucketCount = static_cast<std::int32_t>(
      std::pow(2.0, m_subBucketHalfCountMagnitude + 1));
  m_subBucketHalfCount = m_subBucketCount / 2;
  m_subBucketMask = (m_subBucketCount - 1) << m_unitMagnitude;

  // bucketsNeededToCoverValue (reference algorithm): how many buckets of
  // doubling range are required to reach `highest`.
  std::int64_t smallestUntrackableValue =
      static_cast<std::int64_t>(m_subBucketCount) << m_unitMagnitude;
  std::int32_t bucketsNeeded = 1;
  while (smallestUntrackableValue <= highest) {
    if (smallestUntrackableValue > (INT64_MAX / 2)) {
      ++bucketsNeeded;
      break;
    }
    smallestUntrackableValue <<= 1;
    ++bucketsNeeded;
  }
  m_bucketCount = bucketsNeeded;

  m_countsLen = (m_bucketCount + 1) * m_subBucketHalfCount;
  m_counts.assign(static_cast<std::size_t>(m_countsLen), 0);
}

int Histogram::CountsIndexFor(std::int64_t value) const {
  const std::int32_t bucketIndex = std::max<std::int32_t>(
      0, 64 -
             CountLeadingZeros64(
                 static_cast<std::uint64_t>(value | m_subBucketMask)) -
             (m_unitMagnitude + m_subBucketHalfCountMagnitude + 1));
  const std::int32_t subBucketIndex =
      static_cast<std::int32_t>(value >> (bucketIndex + m_unitMagnitude));

  // countsIndex (reference): bucketBaseIndex + (subBucketIndex -
  // subBucketHalf).
  const std::int32_t bucketBaseIndex = (bucketIndex + 1)
                                       << m_subBucketHalfCountMagnitude;
  const std::int32_t offsetInBucket = subBucketIndex - m_subBucketHalfCount;
  return bucketBaseIndex + offsetInBucket;
}

std::int64_t Histogram::ValueFromIndex(int index) const {
  std::int32_t bucketIndex = (index >> m_subBucketHalfCountMagnitude) - 1;
  std::int32_t subBucketIndex =
      (index & (m_subBucketHalfCount - 1)) + m_subBucketHalfCount;
  if (bucketIndex < 0) {
    subBucketIndex -= m_subBucketHalfCount;
    bucketIndex = 0;
  }
  return static_cast<std::int64_t>(subBucketIndex)
         << (bucketIndex + m_unitMagnitude);
}

bool Histogram::RecordClamped(std::int64_t value) {
  bool clamped = false;
  if (value > m_highest) {
    value = m_highest;
    clamped = true;
  }
  if (value < 1) {
    value = 1;
  }
  int index = CountsIndexFor(value);
  if (index < 0) {
    index = 0;
  }
  if (index >= m_countsLen) {
    index = m_countsLen - 1;
    clamped = true;
  }
  m_counts[static_cast<std::size_t>(index)]++;
  m_total++;
  return clamped;
}

void Histogram::Merge(const Histogram &other) {
  const int n = std::min(other.m_countsLen, m_countsLen);
  for (int i = 0; i < n; ++i) {
    const std::int64_t c = other.m_counts[static_cast<std::size_t>(i)];
    if (c != 0) {
      m_counts[static_cast<std::size_t>(i)] += c;
      m_total += c;
    }
  }
}

std::int64_t Histogram::SizeOfEquivalentValueRange(std::int64_t value) const {
  const std::int32_t bucketIndex = std::max<std::int32_t>(
      0, 64 -
             CountLeadingZeros64(
                 static_cast<std::uint64_t>(value | m_subBucketMask)) -
             (m_unitMagnitude + m_subBucketHalfCountMagnitude + 1));
  const std::int32_t subBucketIndex =
      static_cast<std::int32_t>(value >> (bucketIndex + m_unitMagnitude));
  const std::int32_t adjustedBucket =
      (subBucketIndex >= m_subBucketCount) ? bucketIndex + 1 : bucketIndex;
  return static_cast<std::int64_t>(1) << (m_unitMagnitude + adjustedBucket);
}

std::int64_t Histogram::LowestEquivalentValue(std::int64_t value) const {
  const std::int32_t bucketIndex = std::max<std::int32_t>(
      0, 64 -
             CountLeadingZeros64(
                 static_cast<std::uint64_t>(value | m_subBucketMask)) -
             (m_unitMagnitude + m_subBucketHalfCountMagnitude + 1));
  const std::int32_t subBucketIndex =
      static_cast<std::int32_t>(value >> (bucketIndex + m_unitMagnitude));
  return static_cast<std::int64_t>(subBucketIndex)
         << (bucketIndex + m_unitMagnitude);
}

std::int64_t Histogram::HighestEquivalentValue(std::int64_t value) const {
  return LowestEquivalentValue(value) + SizeOfEquivalentValueRange(value) - 1;
}

std::int64_t Histogram::MedianEquivalentValue(std::int64_t value) const {
  return LowestEquivalentValue(value) +
         (SizeOfEquivalentValueRange(value) >> 1);
}

std::int64_t Histogram::ValueAtQuantile(double quantile) const {
  if (m_total == 0) {
    return 0;
  }
  const double q = std::min(quantile, 100.0) / 100.0;
  std::int64_t countAtPercentile =
      std::lround(q * static_cast<double>(m_total));
  if (countAtPercentile < 1) {
    countAtPercentile = 1;
  }
  std::int64_t total = 0;
  for (int i = 0; i < m_countsLen; ++i) {
    total += m_counts[static_cast<std::size_t>(i)];
    if (total >= countAtPercentile) {
      return MedianEquivalentValue(ValueFromIndex(i));
    }
  }
  return 0;
}

std::int64_t Histogram::Max() const {
  std::int64_t maxValue = 0;
  for (int i = m_countsLen - 1; i >= 0; --i) {
    if (m_counts[static_cast<std::size_t>(i)] != 0) {
      maxValue = ValueFromIndex(i);
      break;
    }
  }
  return HighestEquivalentValue(maxValue);
}

Percentiles Histogram::Extract() const {
  Percentiles p;
  p.p50 = nanoseconds(ValueAtQuantile(50.0));
  p.p90 = nanoseconds(ValueAtQuantile(90.0));
  p.p99 = nanoseconds(ValueAtQuantile(99.0));
  p.p999 = nanoseconds(ValueAtQuantile(99.9));
  p.max = nanoseconds(Max());
  p.count = m_total;
  return p;
}

} // namespace spot_loadtest::measurement
