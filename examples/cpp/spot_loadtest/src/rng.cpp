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

#include "spot_loadtest/generator/rng.hpp"

#include <cmath>
#include <cstdint>
#include <vector>

namespace spot_loadtest::generator {
namespace {
// PCG64 multiplier (canonical constant).
constexpr std::uint64_t kPcgMultiplier = 6364136223846793005ULL;
} // namespace

Rng Rng::NewContent(std::uint64_t seed) {
  // Golden-ratio constant decorrelates seed 0; the increment must be odd.
  constexpr std::uint64_t kMix = 0x9E3779B97F4A7C15ULL;
  Rng g(seed, ((seed ^ kMix) << 1U) | 1U);
  // Advance once so distinct seeds diverge immediately.
  (void)g.Next64();
  return g;
}

Rng Rng::NewSchedule(std::uint64_t seed) {
  // Distinct odd constant decorrelates from the content RNG.
  constexpr std::uint64_t kMix = 0xD1B54A32D192ED03ULL;
  Rng g(seed ^ kMix, (seed << 1U) | 1U);
  (void)g.Next64();
  return g;
}

std::uint64_t Rng::Next64() {
  // PCG-XSH-RR style 64-bit output from a 128-bit-quality LCG state pair: here
  // we use a 64-bit state and an output permutation that mixes the high bits.
  const std::uint64_t oldState = m_state;
  m_state = oldState * kPcgMultiplier + m_inc;
  // Output function: xorshift then rotate, producing a well-distributed 64-bit
  // value from the 64-bit state.
  std::uint64_t xorshifted =
      ((oldState >> 29U) ^ oldState) * 0xAEF17502108EF2D9ULL;
  const unsigned rot = static_cast<unsigned>(oldState >> 59U);
  xorshifted = (xorshifted >> rot) | (xorshifted << ((64U - rot) & 63U));
  return xorshifted;
}

double Rng::Float64() {
  // 53-bit mantissa uniform in [0, 1), matching the standard construction.
  return static_cast<double>(Next64() >> 11U) * (1.0 / 9007199254740992.0);
}

double Rng::ExpFloat(double rate) {
  if (rate <= 0) {
    return 0.0;
  }
  // Inverse-CDF exponential with mean 1, then divide by rate to get mean
  // 1/rate. Float64() is in [0,1); 1 - u keeps the log argument in (0, 1].
  double u = Float64();
  if (u <= 0.0) {
    u = 1e-300; // guard against log(0) on the (extremely rare) exact-zero draw.
  }
  return -std::log(u) / rate;
}

double Rng::NormFloat() {
  // Box-Muller: two uniforms -> one standard normal. Deterministic per draw.
  double u1 = Float64();
  const double u2 = Float64();
  if (u1 <= 0.0) {
    u1 = 1e-300;
  }
  return std::sqrt(-2.0 * std::log(u1)) *
         std::cos(2.0 * 3.14159265358979323846 * u2);
}

bool Rng::Bernoulli(double p) {
  if (p <= 0) {
    return false;
  }
  if (p >= 1) {
    return true;
  }
  return Float64() < p;
}

int Rng::IntN(int n) {
  // Unbiased bounded integer via rejection on the 64-bit output.
  const std::uint64_t bound = static_cast<std::uint64_t>(n);
  const std::uint64_t threshold = (std::uint64_t{0} - bound) % bound;
  while (true) {
    const std::uint64_t r = Next64();
    if (r >= threshold) {
      return static_cast<int>(r % bound);
    }
  }
}

int Rng::PickWeighted(const std::vector<double> &cum) {
  const double total = cum.back();
  const double target = Float64() * total;
  for (std::size_t i = 0; i < cum.size(); ++i) {
    if (target < cum[i]) {
      return static_cast<int>(i);
    }
  }
  return static_cast<int>(cum.size()) - 1;
}

std::vector<double> Cumulative(const std::vector<double> &weights) {
  std::vector<double> cum(weights.size());
  double sum = 0.0;
  for (std::size_t i = 0; i < weights.size(); ++i) {
    sum += weights[i];
    cum[i] = sum;
  }
  return cum;
}

} // namespace spot_loadtest::generator
