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

#include <cstdint>

namespace openpit::param {

/// Exact decimal represented as `mantissa * 10^-scale`.
///
/// The signed 128-bit mantissa is split into low and sign-extended high words
/// so the value remains lossless without requiring a compiler-specific integer
/// type in the public API.
struct Decimal {
  /// Low signed word of the two's-complement mantissa.
  std::int64_t mantissaLo = 0;
  /// Sign-extended high word of the two's-complement mantissa.
  std::int64_t mantissaHi = 0;
  /// Number of base-10 fractional digits.
  std::int32_t scale = 0;

  [[nodiscard]] constexpr bool operator==(const Decimal& other) const noexcept {
    return mantissaLo == other.mantissaLo && mantissaHi == other.mantissaHi &&
           scale == other.scale;
  }

  [[nodiscard]] constexpr bool operator!=(const Decimal& other) const noexcept {
    return !(*this == other);
  }
};

}  // namespace openpit::param
