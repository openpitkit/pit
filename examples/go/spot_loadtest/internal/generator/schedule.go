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

import (
	"math"
	"time"

	"openpit-loadtest-spot-funds-go/internal/config"
)

// causalGap is the small fixed spacing inserted between an event and a
// causally-dependent successor on the same account, so a dependent order is
// always scheduled strictly after its dependency's virtual completion (hold or
// settlement). One microsecond keeps the virtual timeline strictly ordered
// per account without materially perturbing the offered cadence.
const causalGap = time.Microsecond

// assignVirtualTimes walks the emitted events in emission (Seq) order and
// stamps each with a VirtualT0 on the offline virtual causal timeline. It runs
// as a SEPARATE pass after the content has been generated, using a dedicated
// schedule RNG (newScheduleRNG) decorrelated from the content RNG, so the
// emitted content stays byte-identical and only the virtual times are added.
// Everything here is a pure function of (seed, config).
//
// Model (two clocks combined by max):
//
//   - globalClock advances ONLY on each order-check, by one inter-arrival of the
//     offered process ([arrival] offered_rate, Poisson/exponential). This makes
//     the AGGREGATE order-check arrival rate across all accounts equal
//     offered_rate. When offered_rate == 0 the process is unpaced (return 0).
//   - acctClock[account] is the earliest virtual instant the account's next event
//     may occupy, reflecting causal dependencies: the hold is visible at an
//     order-check's VirtualT0, the fill at its settlement's VirtualT0, and a
//     same-account successor must follow by at least causalGap.
//
// Each event's VirtualT0 = max(its offered-process instant, its account's causal
// floor). The max only ever pushes an event LATER than the bare offered tick,
// which is the honest intended arrival: a dependent order genuinely cannot be
// intended to arrive before its dependency completed.
func (gen *generator) assignVirtualTimes() {
	vg := newScheduleRNG(gen.cfg.Run.Seed)
	rate := float64(gen.cfg.Arrival.OfferedRate)
	reportMean, reportSigma, reportDist := reportDelayParams(gen.cfg.ReportDelay)

	var globalClock time.Duration
	acctClock := make(map[string]time.Duration, len(gen.pop.accounts))
	// ocVirtualByCorr maps an order-check's correlation id to its VirtualT0 so
	// the matching settlement can offset from it by the report-return delay.
	ocVirtualByCorr := make(map[uint64]time.Duration)

	for i := range gen.events {
		ev := &gen.events[i]
		switch ev.Kind {
		case EventFunding:
			if ev.FundingIsSeed {
				// Seeds are applied synchronously before the run (setup, not paced
				// load); their virtual time is never used. Pin to 0.
				ev.VirtualT0 = 0
				continue
			}
			// Runtime top-up: auxiliary to the offered order-check process, so it
			// does NOT consume an inter-arrival tick. It is emitted right before the
			// order-check it funds, so scheduling it at the account's causal floor
			// (and bumping the floor) guarantees that order-check arrives after it.
			arrival := maxDuration(globalClock, acctClock[ev.Account])
			ev.VirtualT0 = arrival
			acctClock[ev.Account] = arrival + causalGap

		case EventOrderCheck:
			// Advance the offered process by one inter-arrival.
			globalClock += interArrival(vg, rate)
			arrival := maxDuration(globalClock, acctClock[ev.Account])
			ev.VirtualT0 = arrival
			ocVirtualByCorr[ev.CorrelationID] = arrival
			// The hold is visible at the order-check's VirtualT0; a same-account
			// successor (e.g. an add against the hold) may follow after a gap. An
			// accepted order's settlement pushes this floor further when processed.
			acctClock[ev.Account] = arrival + causalGap

		case EventSettlement:
			oc := ocVirtualByCorr[ev.CorrelationID]
			delay := reportDelay(vg, reportDist, reportMean, reportSigma)
			settle := oc + delay
			ev.VirtualT0 = settle
			// A dependent same-account order (e.g. a sell funded by this fill) must
			// follow the settlement by at least a gap.
			acctClock[ev.Account] = maxDuration(acctClock[ev.Account], settle+causalGap)
		}
	}
}

// interArrival returns one inter-arrival of the offered order-check process.
// Arrivals are Poisson (exponential inter-arrivals). When offered_rate == 0 the
// process is unpaced: return 0 so order-check arrivals are driven purely by the
// per-account causal floors (the unpaced, nothing-throttled regime).
func interArrival(vg *rng, rate float64) time.Duration {
	if rate <= 0 {
		return 0
	}
	return time.Duration(vg.expFloat(rate) * float64(time.Second))
}

// reportDelayParams resolves the [report_delay] config into a parsed mean
// duration, a sigma, and the distribution. A missing/blank/zero mean yields a
// zero delay (settlement VirtualT0 == its order-check VirtualT0).
func reportDelayParams(rd config.ReportDelay) (mean time.Duration, sigma float64, dist config.ReportDelayDistribution) {
	if rd.Mean != "" {
		if d, err := time.ParseDuration(rd.Mean); err == nil && d > 0 {
			mean = d
		}
	}
	return mean, rd.Sigma, rd.Distribution
}

// reportDelay samples the simulated report-return (TS round-trip) delay added
// to a settlement's virtual time. With a zero mean it returns 0. Otherwise:
//   - fixed: the mean exactly;
//   - lognormal (default when a mean is set): a lognormal whose MEDIAN is the
//     mean, i.e. mean * exp(sigma * Z), Z ~ N(0,1). One schedule-RNG normal draw.
//
// All draws come from the schedule RNG in a fixed order, so the timeline stays
// deterministic for a given seed.
func reportDelay(vg *rng, dist config.ReportDelayDistribution, mean time.Duration, sigma float64) time.Duration {
	if mean <= 0 {
		return 0
	}
	if dist == config.ReportDelayFixed {
		return mean
	}
	// Lognormal (the configured default). median = mean; spread set by sigma.
	z := vg.normFloat()
	factor := math.Exp(sigma * z)
	d := time.Duration(float64(mean) * factor)
	if d < 0 {
		return 0
	}
	return d
}

// maxDuration returns the larger of a and b.
func maxDuration(a, b time.Duration) time.Duration {
	if a > b {
		return a
	}
	return b
}
