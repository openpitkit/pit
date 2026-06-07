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

package measurement

import (
	"context"
	"time"

	hdrhistogram "github.com/HdrHistogram/hdrhistogram-go"
)

// OverheadProber is a function that submits one trivial operation through the
// async engine and returns the submit->decision latency. The driver supplies
// this so the measurement package does not depend on engine internals.
//
// The probe must use the same async path as the real workload (ExecutePreTrade
// or ApplyAccountAdjustment through AsyncEngine) so the measured round-trip
// includes the real FFI and queue overhead. The driver uses a no-content
// adjustment on a dedicated synthetic account so the probe does not disturb the
// oracle or the shadow ledger.
type OverheadProber func(ctx context.Context) (time.Duration, error)

// OverheadSummary is the result of the harness self-overhead characterisation.
// The driver probes it through ApplyAccountAdjustment (NOT ExecutePreTrade), so
// it characterises the adjustment-path FFI+queue floor, not the order-check
// path; the reporter labels it accordingly.
type OverheadSummary struct {
	// Probes is the number of completed probe round-trips.
	Probes int
	// Distribution holds the latency percentiles across all probes.
	Distribution Percentiles
	// Clamped is the number of probe samples saturated to the histogram ceiling
	// rather than dropped.
	Clamped int64
}

// MeasureOverhead runs prober probeCount times sequentially (no concurrency,
// so the probe sees no queueing from the workload) and returns the summary.
// It is called before the workload begins, with a quiescent engine.
//
// The overhead characterisation is a best-effort estimate of the bare
// FFI+asyncengine round-trip with no policy work (or trivial policy work). The
// report discloses it as "harness self-overhead" so readers can subtract it
// from the headline if they wish. It is NOT subtracted from the headline itself.
func MeasureOverhead(ctx context.Context, probeCount int, prober OverheadProber) (OverheadSummary, error) {
	h := hdrhistogram.New(histMinNs, histMaxNs, histSigFig)
	var probeErr error
	var clamped int64
	for i := 0; i < probeCount; i++ {
		if ctx.Err() != nil {
			// Context cancelled: stop probing early; the completed probes are
			// still useful, so return what we have.
			break
		}
		d, err := prober(ctx)
		if err != nil {
			probeErr = err
			break
		}
		ns := toNs(d)
		if recordClamped(h, ns) {
			clamped++
		}
	}
	if probeErr != nil {
		return OverheadSummary{}, probeErr
	}
	return OverheadSummary{
		Probes:       int(h.TotalCount()),
		Distribution: extract(h),
		Clamped:      clamped,
	}, nil
}
