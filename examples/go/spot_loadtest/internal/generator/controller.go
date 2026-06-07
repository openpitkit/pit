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

// rejectController is the offline reject-rate controller. Without any engine
// feedback it calibrates how often the generator emits an order the shadow
// model predicts will be rejected, converging the predicted reject rate to the
// configured target within tolerance.
//
// Mechanism - error-driven (integral) control:
//
//	rate = rejects / checks (running, over the whole stream so far)
//	force the next eligible order iff rate < target
//
// This bang-bang law steers the running rate toward target: whenever the rate
// dips below target the controller forces rejects (pushing it up); once at/above
// target it stops forcing (letting accepts pull it down). Over a large stream
// the rate settles within one event of target, i.e. well inside any practical
// tolerance, provided enough orders are eligible to force. Eligibility is gated
// by the cohort's reject propensity so forced rejects land where configured.
//
// Determinism: the only randomness is the eligibility draw (one Float64 per
// candidate), consumed in stream order, so the forced-reject pattern is
// reproducible for a given seed + config.
type rejectController struct {
	target  float64
	checks  uint64
	rejects uint64
}

func newRejectController(target float64) *rejectController {
	return &rejectController{target: target}
}

// shouldForce reports whether the next eligible order should be forced into a
// predicted reject. propensity gates eligibility (the cohort's share of forced
// rejects); the integral law then decides based on the running rate.
func (c *rejectController) shouldForce(g *rng, propensity float64) bool {
	if c.target <= 0 {
		return false
	}
	// Eligibility: only force within cohorts that opted in via propensity.
	if !g.bernoulli(propensity) {
		return false
	}
	return c.rate() < c.target
}

// observe records the realised order-check outcome so the running rate tracks
// what the shadow model actually predicted (forced and natural rejects alike).
func (c *rejectController) observe(accepted bool) {
	c.checks++
	if !accepted {
		c.rejects++
	}
}

// rate is the running predicted reject rate; 0 before any observation.
func (c *rejectController) rate() float64 {
	if c.checks == 0 {
		return 0
	}
	return float64(c.rejects) / float64(c.checks)
}
