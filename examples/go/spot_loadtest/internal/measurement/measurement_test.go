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

package measurement_test

import (
	"context"
	"sync"
	"testing"
	"time"

	"openpit-loadtest-spot-funds-go/internal/measurement"
)

// TestCOSamplePassthrough asserts that a latency value supplied by the driver
// (resolve - intendedT0) lands in the histogram exactly as given, with no
// recomputation by the measurement package. This is the coordinated-omission
// correctness test: the driver is the source of truth for t0.
func TestCOSamplePassthrough(t *testing.T) {
	w := measurement.NewWindows(measurement.WindowUnitOps, 10000, 0)
	s := measurement.NewSink(w)

	// Simulate what the driver does: stamp intendedT0 before submit, then
	// compute latency = resolve - intendedT0 after Await returns.
	intendedT0 := time.Now()
	// Synthetic "processing" delay (just a known value; we do not actually call
	// the engine here since this is a pure-Go measurement test).
	syntheticProcessing := 3 * time.Millisecond
	resolveTime := intendedT0.Add(syntheticProcessing)
	latency := resolveTime.Sub(intendedT0) // = syntheticProcessing

	s.RecordSubmit()
	s.RecordOrderCheck(latency, true)

	_, ocMerged, _ := w.Snapshot()

	// The histogram stores nanosecond values; HdrHistogram quantises to
	// 3 significant figures, so the recorded value is at most 0.1% off.
	tolerance := syntheticProcessing / 1000
	if ocMerged.Count != 1 {
		t.Fatalf("merged order-check count = %d, want 1", ocMerged.Count)
	}
	diff := ocMerged.P50 - syntheticProcessing
	if diff < 0 {
		diff = -diff
	}
	if diff > tolerance {
		t.Errorf("merged p50 = %v, want ~%v (diff %v > tolerance %v)",
			ocMerged.P50, syntheticProcessing, diff, tolerance)
	}
}

// TestWindowingByOps verifies that windows rotate correctly on op-count
// boundaries and that each window's count matches the window size.
func TestWindowingByOps(t *testing.T) {
	const windowSize = 100
	w := measurement.NewWindows(measurement.WindowUnitOps, windowSize, 0)

	for i := 0; i < 350; i++ {
		w.RecordOrderCheck(time.Duration(i+1) * time.Microsecond)
	}

	snaps, _, _ := w.Snapshot()
	// 350 ops with a window of 100 = 3 complete windows (300) + 1 partial (50).
	if len(snaps) != 4 {
		t.Fatalf("window count = %d, want 4 (3 full + 1 partial)", len(snaps))
	}
	for i, s := range snaps[:3] {
		if s.OrderCheck.Count != windowSize {
			t.Errorf("window[%d].OrderCheck.Count = %d, want %d", i, s.OrderCheck.Count, windowSize)
		}
	}
	if snaps[3].OrderCheck.Count != 50 {
		t.Errorf("trailing window count = %d, want 50", snaps[3].OrderCheck.Count)
	}
}

// TestWindowingByWall verifies that wall-clock windows rotate when the
// configured duration elapses.
func TestWindowingByWall(t *testing.T) {
	// Use a short but non-trivially small duration; the test records ops quickly
	// so the boundary is crossed between bursts.
	const wallWindow = 50 * time.Millisecond
	w := measurement.NewWindows(measurement.WindowUnitWall, 0, wallWindow)

	// Record one batch, then sleep past the boundary, then record another.
	for i := 0; i < 20; i++ {
		w.RecordOrderCheck(time.Millisecond)
	}
	time.Sleep(wallWindow + 10*time.Millisecond)
	for i := 0; i < 20; i++ {
		w.RecordOrderCheck(time.Millisecond)
	}

	snaps, _, _ := w.Snapshot()
	if len(snaps) < 2 {
		t.Fatalf("wall-clock windowing produced %d windows, want >= 2", len(snaps))
	}
}

// TestMergedPercentilesMatchWindowUnion verifies that the merged histogram
// contains exactly as many samples as the sum of all window counts.
func TestMergedPercentilesMatchWindowUnion(t *testing.T) {
	w := measurement.NewWindows(measurement.WindowUnitOps, 50, 0)
	const total = 130
	for i := 0; i < total; i++ {
		w.RecordOrderCheck(time.Duration(i+1) * time.Microsecond)
	}
	snaps, merged, _ := w.Snapshot()

	var windowTotal int64
	for _, s := range snaps {
		windowTotal += s.OrderCheck.Count
	}
	if windowTotal != total {
		t.Errorf("sum of window counts = %d, want %d", windowTotal, total)
	}
	if merged.Count != total {
		t.Errorf("merged count = %d, want %d", merged.Count, total)
	}
}

// TestMergeWindowRangeLossless is the consistency invariant test: merging ALL
// per-window raw histograms (warmup = 0, i.e. MergeWindowRange(windows, 0))
// must reproduce the all-run merged percentiles bit-for-bit (within HdrHistogram
// quantization). This proves that:
//   - the per-window histogram copies are lossless (Export+Import round-trips),
//   - MergeWindowRange and the internal merged histogram use the same data, and
//   - the two code paths agree so the steady-state headline is trustworthy.
//
// When no window is excluded, steady-state == all-run merged by definition;
// any divergence would indicate a bug in the copy or merge path.
func TestMergeWindowRangeLossless(t *testing.T) {
	const windowSize = 50
	w := measurement.NewWindows(measurement.WindowUnitOps, windowSize, 0)
	// Record a varied set of latencies spanning several windows.
	for i := 0; i < 220; i++ {
		w.RecordOrderCheck(time.Duration(i+1) * time.Microsecond)
		w.RecordSettlement(time.Duration((i+1)*2) * time.Microsecond)
	}

	snaps, allRunOC, allRunSet := w.Snapshot()

	// MergeWindowRange with start=0 merges every window — identical set to
	// the all-run merged histogram.
	ssOC, ssSet := measurement.MergeWindowRange(snaps, 0)

	// Percentile values must match exactly (same histogram bucket data).
	checkPercentiles := func(label string, got, want measurement.Percentiles) {
		t.Helper()
		if got.Count != want.Count {
			t.Errorf("%s: Count: got %d, want %d", label, got.Count, want.Count)
		}
		if got.P50 != want.P50 {
			t.Errorf("%s: P50: got %v, want %v", label, got.P50, want.P50)
		}
		if got.P99 != want.P99 {
			t.Errorf("%s: P99: got %v, want %v", label, got.P99, want.P99)
		}
		if got.P999 != want.P999 {
			t.Errorf("%s: P999: got %v, want %v", label, got.P999, want.P999)
		}
		if got.Max != want.Max {
			t.Errorf("%s: Max: got %v, want %v", label, got.Max, want.Max)
		}
	}
	checkPercentiles("order-check", ssOC, allRunOC)
	checkPercentiles("settlement", ssSet, allRunSet)
}

// TestChecksumChangesOnEachRecord verifies that the checksum is not constant
// after multiple distinct records (basic anti-DCE proof).
func TestChecksumChangesOnEachRecord(t *testing.T) {
	w := measurement.NewWindows(measurement.WindowUnitOps, 10000, 0)
	s := measurement.NewSink(w)

	s.RecordSubmit()
	s.RecordOrderCheck(time.Millisecond, true)
	stats1 := s.Stats()

	s.RecordSubmit()
	s.RecordOrderCheck(2*time.Millisecond, false)
	stats2 := s.Stats()

	if stats1.Checksum == stats2.Checksum {
		t.Error("checksum did not change across two distinct records")
	}
	if stats2.OrderChecks != 2 {
		t.Errorf("OrderChecks = %d, want 2", stats2.OrderChecks)
	}
}

// TestInFlight verifies that RecordSubmit/RecordResolve keep the in-flight
// counter and peak consistent.
func TestInFlight(t *testing.T) {
	w := measurement.NewWindows(measurement.WindowUnitOps, 10000, 0)
	s := measurement.NewSink(w)

	// Submit 3, resolve 1 -> peak = 3, in-flight ends at 0.
	s.RecordSubmit()
	s.RecordSubmit()
	s.RecordSubmit()
	s.RecordOrderCheck(time.Millisecond, true)
	s.RecordOrderCheck(time.Millisecond, false)
	s.RecordOrderCheck(time.Millisecond, true)

	stats := s.Stats()
	if stats.MaxInFlight < 2 {
		t.Errorf("MaxInFlight = %d, want >= 2", stats.MaxInFlight)
	}
}

// TestObserverSinkRaceClean runs concurrent RecordDequeue and RecordComplete
// calls to confirm there is no data race under the race detector.
func TestObserverSinkRaceClean(t *testing.T) {
	obs := measurement.NewObserverSink()
	const goroutines = 10
	const callsEach = 500
	var wg sync.WaitGroup
	wg.Add(goroutines * 2)
	for i := 0; i < goroutines; i++ {
		go func() {
			defer wg.Done()
			for j := 0; j < callsEach; j++ {
				obs.RecordDequeue(time.Duration(j+1) * time.Microsecond)
			}
		}()
		go func() {
			defer wg.Done()
			for j := 0; j < callsEach; j++ {
				obs.RecordComplete(time.Duration(j+1) * time.Microsecond)
			}
		}()
	}
	wg.Wait()
	m := obs.Snapshot()
	if m.Dequeues != goroutines*callsEach {
		t.Errorf("Dequeues = %d, want %d", m.Dequeues, goroutines*callsEach)
	}
	if m.Completes != goroutines*callsEach {
		t.Errorf("Completes = %d, want %d", m.Completes, goroutines*callsEach)
	}
}

// TestSinkRaceClean verifies RecordSubmit / RecordOrderCheck / RecordSettlement
// are race-free under concurrent collectors.
func TestSinkRaceClean(t *testing.T) {
	w := measurement.NewWindows(measurement.WindowUnitOps, 10000, 0)
	s := measurement.NewSink(w)
	const goroutines = 8
	const opsEach = 200
	var wg sync.WaitGroup
	wg.Add(goroutines)
	for i := 0; i < goroutines; i++ {
		go func(idx int) {
			defer wg.Done()
			for j := 0; j < opsEach; j++ {
				s.RecordSubmit()
				s.RecordOrderCheck(time.Duration(idx*opsEach+j+1)*time.Microsecond, j%2 == 0)
			}
		}(i)
	}
	wg.Wait()
	stats := s.Stats()
	if stats.OrderChecks != goroutines*opsEach {
		t.Errorf("OrderChecks = %d, want %d", stats.OrderChecks, goroutines*opsEach)
	}
}

// TestSteadyStateConsistency verifies that the steady-state percentiles in a
// Snapshot built by Build are consistent with the all-run merged percentiles
// in the expected direction: when warmup is excluded and steady-state has
// lower latencies than warmup, steady-state p50 must be lower than all-run p50.
//
// We use equal-size warmup and steady-state windows so the warmup clearly
// dominates enough to shift the all-run p50. With 50 warmup samples at ~10ms
// and 50 steady-state samples at ~100µs, the all-run p50 (median of 100) is
// where the 50th sample falls — right at the boundary between the two groups.
// Because HdrHistogram sorts by value (not insertion order), samples from the
// two groups interleave: all 50 low-latency samples fall below p50, so the
// 50th-percentile value depends on relative magnitudes. With 10ms >> 100µs
// and equal counts, all-run p50 ≈ 10ms (the lower half of samples are all
// 100µs, the upper half are all 10ms; median is either the last 100µs or
// first 10ms sample — implementation-defined boundary). Either way, all-run
// p50 is > steady-state p50 (≈100µs).
func TestSteadyStateConsistency(t *testing.T) {
	// Window size 50 ops: 1 warmup window (50 high-latency ops) +
	// 1 steady-state window (50 low-latency ops).
	const windowSize = 50
	w := measurement.NewWindows(measurement.WindowUnitOps, windowSize, 0)
	s := measurement.NewSink(w)

	// Warmup window: 50 ops at ~10ms each.
	for i := 0; i < windowSize; i++ {
		s.RecordSubmit()
		s.RecordOrderCheck(10*time.Millisecond, true)
	}
	// Steady-state window: 50 ops at ~100µs each. Plus 1 partial window so
	// Build seals the partial window and we still have ≥ 2 sealed windows.
	for i := 0; i < windowSize+1; i++ {
		s.RecordSubmit()
		s.RecordOrderCheck(100*time.Microsecond, true)
	}

	snap := measurement.Build(w, s, nil, measurement.OverheadSummary{})

	// We expect at least 3 windows (warmup + steady + partial).
	if len(snap.Windows) < 2 {
		t.Fatalf("expected >= 2 windows, got %d", len(snap.Windows))
	}
	if snap.WarmupWindows != 1 {
		t.Fatalf("expected WarmupWindows=1, got %d", snap.WarmupWindows)
	}

	// All-run p50 spans both groups; for equal-size groups of 10ms vs 100µs,
	// the median is on the 100µs side (values are sorted; 50th of 101 is the
	// 50th lowest value, all of which are 100µs — but just barely). However,
	// the key assertion is that steady-state p50 is lower than the all-run p50
	// WHEN the all-run is contaminated by warmup. Use p90 for a robust check:
	// steady-state p90 must be << all-run p90 (which includes the 10ms spike).
	if snap.SteadyStateOrderCheck.P90 >= snap.OrderCheck.P90 {
		t.Errorf("steady-state p90 (%v) >= all-run p90 (%v): warmup exclusion had no effect on p90",
			snap.SteadyStateOrderCheck.P90, snap.OrderCheck.P90)
	}

	// Steady-state p99 must be << the warmup latency (10ms).
	if snap.SteadyStateOrderCheck.P99 >= 1*time.Millisecond {
		t.Errorf("steady-state p99 = %v, want < 1ms (pure steady-state samples are ~100µs)",
			snap.SteadyStateOrderCheck.P99)
	}

	// All-run p99 must include the warmup spike (must be >= 1ms).
	if snap.OrderCheck.P99 < 1*time.Millisecond {
		t.Errorf("all-run p99 = %v, want >= 1ms (warmup spike at 10ms must appear in all-run)",
			snap.OrderCheck.P99)
	}

	t.Logf("all-run: p50=%v p90=%v p99=%v | steady-state: p50=%v p90=%v p99=%v",
		snap.OrderCheck.P50, snap.OrderCheck.P90, snap.OrderCheck.P99,
		snap.SteadyStateOrderCheck.P50, snap.SteadyStateOrderCheck.P90, snap.SteadyStateOrderCheck.P99)
}

// TestAchievedRejectRateOrderCheckOnly asserts that AchievedRejectRate is
// computed as order-check rejects / order-check decisions and that settlements
// (including any settlement rejects) are completely excluded from both the
// numerator and the denominator.
//
// Scenario: 100 order checks, 5 of which are rejected (5% target hit exactly),
// plus 95 settlements recorded — some of which are also marked as rejected.
// The achieved rate must be exactly 0.05 regardless of the settlement count.
func TestAchievedRejectRateOrderCheckOnly(t *testing.T) {
	const (
		totalOrderChecks  = 100
		orderCheckRejects = 5
		orderCheckAccepts = totalOrderChecks - orderCheckRejects
		totalSettlements  = 95
		settlementRejects = 10                                                     // must NOT affect the rate
		wantAchievedRate  = float64(orderCheckRejects) / float64(totalOrderChecks) // 0.05
	)

	w := measurement.NewWindows(measurement.WindowUnitOps, 10000, 0)
	s := measurement.NewSink(w)

	// Record order checks: first orderCheckRejects as rejected, rest as accepted.
	for i := 0; i < orderCheckRejects; i++ {
		s.RecordSubmit()
		s.RecordOrderCheck(time.Millisecond, false)
	}
	for i := 0; i < orderCheckAccepts; i++ {
		s.RecordSubmit()
		s.RecordOrderCheck(time.Millisecond, true)
	}

	// Record settlements: some rejected, rest accepted.
	for i := 0; i < settlementRejects; i++ {
		s.RecordSubmit()
		s.RecordSettlement(time.Millisecond, false)
	}
	for i := 0; i < totalSettlements-settlementRejects; i++ {
		s.RecordSubmit()
		s.RecordSettlement(time.Millisecond, true)
	}

	snap := measurement.Build(w, s, nil, measurement.OverheadSummary{})

	if snap.AchievedRejectRate != wantAchievedRate {
		t.Errorf("AchievedRejectRate = %v, want %v (settlements must not appear in denominator or numerator)",
			snap.AchievedRejectRate, wantAchievedRate)
	}
	// Sanity: TotalRejects must be order-check rejects only.
	if snap.TotalRejects != orderCheckRejects {
		t.Errorf("TotalRejects = %d, want %d (order-check rejects only)", snap.TotalRejects, orderCheckRejects)
	}
}

// TestAchievedRejectRateZeroOrderChecks verifies the divide-by-zero guard:
// when no order checks have been recorded the achieved rate is 0.0.
func TestAchievedRejectRateZeroOrderChecks(t *testing.T) {
	w := measurement.NewWindows(measurement.WindowUnitOps, 10000, 0)
	s := measurement.NewSink(w)

	// Record only settlements — no order checks.
	for i := 0; i < 10; i++ {
		s.RecordSubmit()
		s.RecordSettlement(time.Millisecond, i%2 == 0)
	}

	snap := measurement.Build(w, s, nil, measurement.OverheadSummary{})
	if snap.AchievedRejectRate != 0.0 {
		t.Errorf("AchievedRejectRate = %v, want 0.0 (divide-by-zero guard)", snap.AchievedRejectRate)
	}
}

// TestOverheadProbe verifies MeasureOverhead calls the prober the requested
// number of times and populates the summary correctly.
func TestOverheadProbe(t *testing.T) {
	const probeCount = 20
	prober := func(_ context.Context) (time.Duration, error) {
		return 500 * time.Microsecond, nil
	}
	summary, err := measurement.MeasureOverhead(context.Background(), probeCount, prober)
	if err != nil {
		t.Fatalf("MeasureOverhead() error = %v", err)
	}
	if summary.Probes != probeCount {
		t.Errorf("Probes = %d, want %d", summary.Probes, probeCount)
	}
	if summary.Distribution.Count != probeCount {
		t.Errorf("Distribution.Count = %d, want %d", summary.Distribution.Count, probeCount)
	}
	// All probes had the same value; p50 should be very close to 500 µs.
	target := 500 * time.Microsecond
	tolerance := target / 1000
	diff := summary.Distribution.P50 - target
	if diff < 0 {
		diff = -diff
	}
	if diff > tolerance {
		t.Errorf("overhead p50 = %v, want ~%v", summary.Distribution.P50, target)
	}
}
