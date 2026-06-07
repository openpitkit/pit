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

package generator

import "math/rand/v2"

// rng wraps a deterministic, cross-platform generator. math/rand/v2's PCG
// source is a fixed algorithm with a stable bit layout across platforms and Go
// versions, so the same seed yields the same stream everywhere - the basis of
// the determinism guarantee (same seed + config -> byte-identical event
// stream and identical predictions).
//
// All randomness in the generator flows through a single rng instance consumed
// in a fixed order, so the stream and every prediction are reproducible.
type rng struct {
	r *rand.Rand
}

// newRNG seeds a PCG generator. The 64-bit config seed is split across PCG's
// two 64-bit seed words; the second word is a fixed mixing constant so a seed
// of 0 still produces a well-distributed stream.
func newRNG(seed uint64) *rng {
	const mix = 0x9E3779B97F4A7C15                        // golden-ratio constant, decorrelates seed 0
	return &rng{r: rand.New(rand.NewPCG(seed, seed^mix))} //nolint:gosec // G404: math/rand PCG is intentional — deterministic reproducibility is a hard requirement
}

// newScheduleRNG seeds a SEPARATE deterministic PCG generator for the virtual
// causal timeline (see assignVirtualTimes). It is decorrelated from the content
// RNG (newRNG) by a distinct second seed word, so assigning virtual times never
// perturbs the content draw order: the emitted events (which accounts wake,
// sizes, rejects, the reject-controller convergence) stay byte-identical, and
// only the new VirtualT0 field is added. Same seed + config still yields a
// byte-identical serialised stream including the virtual times.
func newScheduleRNG(seed uint64) *rng {
	const mix = 0xD1B54A32D192ED03                        // distinct odd constant, decorrelates from newRNG
	return &rng{r: rand.New(rand.NewPCG(seed^mix, seed))} //nolint:gosec // G404: math/rand PCG is intentional — deterministic reproducibility is a hard requirement
}

// expFloat returns an exponentially distributed value with the given rate
// (mean = 1/rate), used for Poisson inter-arrival sampling. One source draw per
// call, consumed in stream order, so determinism holds. A non-positive rate
// returns 0 (the caller treats that as "no spacing").
func (g *rng) expFloat(rate float64) float64 {
	if rate <= 0 {
		return 0
	}
	// rand.ExpFloat64 returns an exponential with mean 1; divide by rate to get
	// mean 1/rate. It is a fixed, stable algorithm over the PCG source.
	return g.r.ExpFloat64() / rate
}

// normFloat returns a standard-normal N(0,1) draw. Used for lognormal
// report-delay sampling. math/rand/v2's NormFloat64 is a fixed ziggurat
// algorithm over the PCG source, so the value is reproducible for a given seed.
func (g *rng) normFloat() float64 {
	return g.r.NormFloat64()
}

// bernoulli returns true with probability p (clamped to [0, 1]).
func (g *rng) bernoulli(p float64) bool {
	if p <= 0 {
		return false
	}
	if p >= 1 {
		return true
	}
	return g.r.Float64() < p
}

// intn returns a uniform integer in [0, n). Panics if n <= 0, matching
// rand.IntN.
func (g *rng) intn(n int) int { return g.r.IntN(n) }

// pickWeighted returns the index selected from cumulative weights cum, where
// cum[i] is the running total up to and including i and cum[len-1] is the
// total weight. The caller precomputes cum once; selection is O(log n) via a
// uniform draw scaled to the total. Determinism: one Float64 draw per call.
func (g *rng) pickWeighted(cum []float64) int {
	total := cum[len(cum)-1]
	target := g.r.Float64() * total
	// Linear scan is fine for the small slices here (cohorts, size buckets);
	// it avoids any binary-search edge-case ambiguity at boundaries.
	for i, c := range cum {
		if target < c {
			return i
		}
	}
	return len(cum) - 1
}

// cumulative builds the running-sum slice used by pickWeighted from raw
// weights. Weights must be non-negative with a positive total.
func cumulative(weights []float64) []float64 {
	cum := make([]float64, len(weights))
	var sum float64
	for i, w := range weights {
		sum += w
		cum[i] = sum
	}
	return cum
}
