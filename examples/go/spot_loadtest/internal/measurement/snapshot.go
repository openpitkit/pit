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

// Snapshot is the complete post-run measurement picture exposed to the Phase-5
// reporter. It carries per-window trajectory data, merged percentiles for the
// headline streams, throughput, reject rate, harness overhead, inner metrics,
// and the anti-DCE checksum.
//
// Throughput is labeled "decided ops / wall time" and is a separate saturation
// measurement; the latency figures come from HdrHistogram windows with full CO
// correction and must never be taken from a saturation run.
type Snapshot struct {
	// Windows is the per-window trajectory (oldest first). Each window holds
	// order-check and settlement percentile distributions AND their raw
	// HdrHistogram copies (WindowSnapshot.OrderCheckHist / SettlementHist).
	Windows []WindowSnapshot

	// OrderCheck is the merged order-check (stage 1->2) percentiles across the
	// full run. This is the headline latency stream.
	OrderCheck Percentiles
	// Settlement is the merged settlement (stage 3->4) percentiles.
	Settlement Percentiles

	// SteadyStateOrderCheck is the order-check percentiles computed by a
	// lossless Merge of the raw HdrHistograms from the steady-state windows
	// (windows[WarmupWindows:]). When WarmupWindows == 0 this is identical to
	// OrderCheck (same histogram set). This MUST be used for the headline
	// number; percentile-of-percentiles over window Percentiles is invalid.
	SteadyStateOrderCheck Percentiles
	// SteadyStateSettlement is the settlement percentiles for the same
	// steady-state window range.
	SteadyStateSettlement Percentiles
	// WarmupWindows is the number of leading windows excluded from the
	// steady-state merge (0 when all windows are included).
	WarmupWindows int

	// ServiceTime is the order-check SERVICE-TIME diagnostic (resolve - actual
	// submit instant), merged over the whole run. It is explicitly NOT the
	// headline: the headline is the open-loop order-check latency (OrderCheck /
	// SteadyStateOrderCheck, measured resolve - VirtualT0). Service-time hides the
	// saturation tail by construction (it discounts queue wait that accrued before
	// the actual submit), so the reporter surfaces it only as a labelled
	// diagnostic, never as the headline.
	ServiceTime Percentiles

	// Throughput is the number of decided operations per second, computed from
	// the wall time between the first RecordSubmit and the last RecordResolve.
	// Labeled as a separate saturation metric, not the headline.
	Throughput float64

	// TotalOrderChecks / TotalSettlements are raw resolved-operation counts.
	TotalOrderChecks uint64
	TotalSettlements uint64
	// TotalAccepts is the number of ORDER-CHECK decisions that were accepted
	// (order-checks only; settlement and funding accepts are reported on their
	// own lines so a reader never sees accepts exceed order-checks).
	TotalAccepts uint64
	// TotalRejects is the number of order-check decisions that were rejected
	// (order-check rejects only; settlement/funding outcomes are not counted here).
	TotalRejects uint64
	// TotalSettlementAccepts / TotalSettlementBlocks are the settlement-class
	// outcomes, kept separate from the order-check accept/reject tally.
	TotalSettlementAccepts uint64
	TotalSettlementBlocks  uint64
	// TotalFundings / TotalFundingAccepts / TotalFundingRejects are the runtime
	// funding (top-up) adjustments resolved on the async path. Funding is NOT an
	// order-check: it is excluded from the headline histogram and the
	// achieved-reject-rate denominator and is disclosed only as its own line.
	TotalFundings       uint64
	TotalFundingAccepts uint64
	TotalFundingRejects uint64

	// AchievedRejectRate is order-check rejects / total order-check decisions
	// (accepts + rejects of order checks only). Settlements are NOT subject to
	// rejection by the reject controller and must not appear in the denominator;
	// including them would dilute the metric and make a run that hits the 5%
	// target exactly report ~2.56%. Guard: 0.0 when no order checks were recorded.
	AchievedRejectRate float64

	// MaxInFlight is the peak concurrent submitted-but-unresolved op count:
	// the open-loop witness (> 1 means submissions overlapped decisions).
	MaxInFlight int64

	// Backpressure is the number of submits the engine refused with a
	// dispatch-capacity backpressure signal (ErrQueueLimit). It is an explicit
	// measured outcome, never a silent drop; a healthy baseline reports zero.
	Backpressure uint64

	// HandoffStalls is the number of HARNESS-internal handoff stalls: the
	// collector -> finalizer fast path was full and the submit schedule would
	// otherwise have been throttled by the harness's own off-path backlog. It is a
	// HARNESS-side starvation witness, NOT engine latency: a collector blocked in
	// fut.Await because the engine is slow is REAL latency and stays in the
	// headline, never counted here. A non-zero count invalidates the run because
	// the headline could fold the harness backlog into the published engine
	// latency; a healthy run reports zero.
	HandoffStalls uint64

	// MaxWorkOverflow is the peak depth of the submitter -> collector spill
	// (workOverflow). DIAGNOSTIC only — NOT an INVALID signal, and NOT folded into
	// the anti-DCE checksum. A large value means collectors lagged submission:
	// usually because they were legitimately blocked in fut.Await (real engine
	// latency, correctly in the headline), but under host CPU starvation it can
	// include collector-dispatch delay that inflates — never flatters — the tail.
	// Cross-check throughput and engine-compute when this depth is large.
	MaxWorkOverflow int

	// ClampedSamples is the total number of latency samples (across the
	// order-check, settlement, service-time, observer, and overhead histograms)
	// whose value exceeded the histogram ceiling and was SATURATED to it rather
	// than dropped. A non-zero count means the upper tail saturated at the
	// ceiling; the count is reported so the saturation is never hidden.
	ClampedSamples int64

	// Checksum is the anti-DCE proof that every decision was consumed.
	Checksum uint64

	// Overhead is the harness self-overhead characterisation (empty-submit
	// round-trip with no workload). May be zero if not probed.
	Overhead OverheadSummary

	// InnerMetrics is the diagnostic observer snapshot (queue_wait,
	// engine_compute, queue lifecycle). Populated only when Observer = true.
	// Kept strictly separate from the headline streams.
	InnerMetrics InnerMetrics

	// WallStart / WallEnd are the wall-clock bounds of the run (first submit to
	// last resolve), used to compute Throughput.
	WallStart time.Time
	WallEnd   time.Time
}

// warmupWindowCount returns the number of leading windows to exclude as
// warmup, applying the same policy as the reporter: 1 warmup window when
// there are more than one window, 0 otherwise (single-window runs cannot
// exclude warmup).
func warmupWindowCount(windows []WindowSnapshot) int {
	if len(windows) <= 1 {
		return 0
	}
	return 1
}

// MergeWindowRange merges the raw HdrHistograms from windows[start:] and
// returns exact Percentiles for order-check and settlement. This is the
// correct primitive for the steady-state headline: lossless Merge of raw
// per-window histograms avoids the statistical invalidity of aggregating
// per-window percentile point-values (percentile-of-percentiles).
func MergeWindowRange(windows []WindowSnapshot, start int) (oc, set Percentiles) {
	hOC := hdrhistogram.New(histMinNs, histMaxNs, histSigFig)
	hSet := hdrhistogram.New(histMinNs, histMaxNs, histSigFig)
	for _, w := range windows[start:] {
		if w.OrderCheckHist != nil {
			hOC.Merge(w.OrderCheckHist)
		}
		if w.SettlementHist != nil {
			hSet.Merge(w.SettlementHist)
		}
	}
	return extract(hOC), extract(hSet)
}

// Build assembles a Snapshot from the Windows, Sink, and optional ObserverSink
// after the run has fully drained. obs may be nil when Observer = false.
func Build(w *Windows, s *Sink, obs *ObserverSink, overhead OverheadSummary) Snapshot {
	windows, ocMerged, setMerged := w.Snapshot()
	stats := s.Stats()

	var achieved float64
	if stats.OrderChecks > 0 {
		achieved = float64(stats.OrderCheckRejects) / float64(stats.OrderChecks)
	}

	var throughput float64
	total := stats.OrderChecks + stats.Settlements
	if !stats.WallStart.IsZero() && stats.WallEnd.After(stats.WallStart) {
		elapsed := stats.WallEnd.Sub(stats.WallStart).Seconds()
		if elapsed > 0 {
			throughput = float64(total) / elapsed
		}
	}

	warmup := warmupWindowCount(windows)
	ssOC, ssSet := MergeWindowRange(windows, warmup)

	// Total clamped samples across every histogram: the windowed streams
	// (order-check, settlement, service-time), plus the observer and overhead
	// histograms when present. Surfacing the sum keeps any saturated tail visible.
	clamped := w.ClampedSamples() + overhead.Clamped

	snap := Snapshot{
		Windows:                windows,
		OrderCheck:             ocMerged,
		Settlement:             setMerged,
		SteadyStateOrderCheck:  ssOC,
		SteadyStateSettlement:  ssSet,
		WarmupWindows:          warmup,
		ServiceTime:            w.ServiceTime(),
		Throughput:             throughput,
		TotalOrderChecks:       stats.OrderChecks,
		TotalSettlements:       stats.Settlements,
		TotalAccepts:           stats.OrderCheckAccepts,
		TotalRejects:           stats.OrderCheckRejects,
		TotalSettlementAccepts: stats.SettlementAccepts,
		TotalSettlementBlocks:  stats.SettlementBlocks,
		TotalFundings:          stats.Fundings,
		TotalFundingAccepts:    stats.FundingAccepts,
		TotalFundingRejects:    stats.FundingRejects,
		AchievedRejectRate:     achieved,
		MaxInFlight:            stats.MaxInFlight,
		Backpressure:           stats.Backpressure,
		HandoffStalls:          stats.HandoffStalls,
		MaxWorkOverflow:        stats.MaxWorkOverflow,
		ClampedSamples:         clamped,
		Checksum:               stats.Checksum,
		Overhead:               overhead,
		WallStart:              stats.WallStart,
		WallEnd:                stats.WallEnd,
	}
	if obs != nil {
		snap.InnerMetrics = obs.Snapshot()
		// Observer histograms have their own clamp tally; fold it into the total.
		snap.ClampedSamples += snap.InnerMetrics.Clamped
	}
	return snap
}
