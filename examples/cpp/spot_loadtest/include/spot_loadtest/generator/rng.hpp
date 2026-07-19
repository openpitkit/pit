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
#include <vector>

// Deterministic, reproducible RNG.
//
// Mirror of: examples/go/spot_loadtest/internal/generator/rng.go
//
// The Go harness uses math/rand/v2's PCG so the same seed yields the same
// stream (the basis of its determinism guarantee). This C++ port keeps the same
// property: a self-contained PCG64 (the canonical algorithm with a fixed bit
// layout) drives every draw, consumed in a fixed order, so the emitted stream
// and every prediction are reproducible for a given (seed, config). The exact
// numeric stream is not byte-identical to Go's — that is impossible across
// languages — but the determinism and convergence invariants the harness
// relies on hold within this binding.

namespace spot_loadtest::generator {

// A deterministic PCG64-backed generator.
class Rng {
public:
  // Seeds the content RNG. The 64-bit config seed is split across PCG's two
  // 64-bit seed words; the second word is a fixed mixing constant so a seed of
  // 0 still produces a well-distributed stream.
  [[nodiscard]] static Rng NewContent(std::uint64_t seed);

  // Seeds a SEPARATE generator for the virtual causal timeline, decorrelated
  // from the content RNG by a distinct second seed word so assigning virtual
  // times never perturbs the content draw order.
  [[nodiscard]] static Rng NewSchedule(std::uint64_t seed);

  // An exponentially distributed value with the given rate (mean = 1/rate),
  // used for Poisson inter-arrival sampling. A non-positive rate returns 0.
  [[nodiscard]] double ExpFloat(double rate);

  // A standard-normal N(0,1) draw, used for lognormal report-delay sampling.
  [[nodiscard]] double NormFloat();

  // True with probability p (clamped to [0, 1]).
  [[nodiscard]] bool Bernoulli(double p);

  // A uniform integer in [0, n). Requires n > 0.
  [[nodiscard]] int IntN(int n);

  // The index selected from cumulative weights `cum`, where cum[i] is the
  // running total up to and including i and cum.back() is the total weight.
  [[nodiscard]] int PickWeighted(const std::vector<double> &cum);

private:
  Rng(std::uint64_t state, std::uint64_t inc) : m_state(state), m_inc(inc) {}

  [[nodiscard]] std::uint64_t Next64();
  [[nodiscard]] double Float64();

  std::uint64_t m_state;
  std::uint64_t m_inc;
};

// Builds the running-sum slice used by PickWeighted from raw weights.
[[nodiscard]] std::vector<double>
Cumulative(const std::vector<double> &weights);

} // namespace spot_loadtest::generator
