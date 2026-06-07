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
	"time"

	hdrhistogram "github.com/HdrHistogram/hdrhistogram-go"
)

// histBounds are the HdrHistogram value range in nanoseconds (see package doc).
const (
	histMinNs  = int64(time.Microsecond) // 1 µs
	histMaxNs  = int64(60 * time.Second) // 60 s
	histSigFig = 3                       // 3 significant figures
)

// newHist allocates a fresh histogram with the standard [1 µs .. 60 s] bounds
// at 3 significant figures.
func newHist() *hdrhistogram.Histogram {
	return hdrhistogram.New(histMinNs, histMaxNs, histSigFig)
}

// recordClamped records ns into h, first clamping it to h's highest trackable
// value so an over-range sample SATURATES at the ceiling instead of being
// dropped. The vendored hdrhistogram-go records NOTHING (returns an error and
// leaves totalCount unchanged) when the value exceeds the histogram's range, so
// a bare RecordValue with the error discarded would silently lose exactly the
// coordinated-omission upper tail. Clamping preserves the COUNT (one sample is
// recorded at the ceiling) at the cost of the exact magnitude above the ceiling.
// It reports whether a clamp occurred so the caller can surface the clamped
// total in the report.
//
// Methodology invariant: latencies above the histogram ceiling are clamped
// (saturated), never dropped; the clamped count is reported.
func recordClamped(h *hdrhistogram.Histogram, ns int64) (clamped bool) {
	if hi := h.HighestTrackableValue(); ns > hi {
		ns = hi
		clamped = true
	}
	// After clamping, ns is within range, so RecordValue cannot fail; the error
	// is intentionally ignored (the clamp above is the range guard).
	_ = h.RecordValue(ns)
	return clamped
}

// Percentiles carries the p50/p90/p99/p99.9/max set derived from one histogram.
type Percentiles struct {
	P50   time.Duration
	P90   time.Duration
	P99   time.Duration
	P999  time.Duration
	Max   time.Duration
	Count int64
}

// Quantile constants for the five-point percentile set.
const (
	quantileP50  = 50.0
	quantileP90  = 90.0
	quantileP99  = 99.0
	quantileP999 = 99.9
)

// extract derives Percentiles from h. An empty histogram returns zero values.
func extract(h *hdrhistogram.Histogram) Percentiles {
	return Percentiles{
		P50:   time.Duration(h.ValueAtQuantile(quantileP50)),
		P90:   time.Duration(h.ValueAtQuantile(quantileP90)),
		P99:   time.Duration(h.ValueAtQuantile(quantileP99)),
		P999:  time.Duration(h.ValueAtQuantile(quantileP999)),
		Max:   time.Duration(h.Max()),
		Count: h.TotalCount(),
	}
}
