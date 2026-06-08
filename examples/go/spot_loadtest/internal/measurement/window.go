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
	"sync"
	"time"

	hdrhistogram "github.com/HdrHistogram/hdrhistogram-go"
)

// WindowUnit mirrors config.WindowUnit to avoid a circular import.
type WindowUnit int

const (
	// WindowUnitOps sizes windows by resolved operation count (default).
	WindowUnitOps WindowUnit = iota
	// WindowUnitWall sizes windows by wall-clock duration.
	WindowUnitWall
)

// WindowSnapshot is the immutable record of one completed window.
//
// OrderCheckHist and SettlementHist are immutable copies of the raw
// HdrHistogram data for this window. They are exported so that callers can
// perform a lossless Merge across a window range (e.g. steady-state windows
// only) and read exact percentiles from the merged result. Aggregating the
// per-window Percentiles point-values instead (percentile-of-percentiles) is
// statistically invalid and must never be used for the headline number.
type WindowSnapshot struct {
	// Index is the zero-based window sequence number.
	Index int
	// OrderCheck is the order-check (stage 1->2) latency distribution.
	OrderCheck Percentiles
	// Settlement is the settlement (stage 3->4) latency distribution.
	Settlement Percentiles
	// WallStart / WallEnd bound the window's wall-clock interval.
	WallStart time.Time
	WallEnd   time.Time

	// OrderCheckHist / SettlementHist are immutable raw histogram copies for
	// this window. Use Histogram.Merge to combine across windows losslessly.
	OrderCheckHist *hdrhistogram.Histogram
	SettlementHist *hdrhistogram.Histogram
}

// windowState holds live (mutable) histograms for one active window.
type windowState struct {
	orderCheck *hdrhistogram.Histogram
	settlement *hdrhistogram.Histogram
	start      time.Time
	opCount    int64 // order-check ops resolved in this window
}

func newWindowState() *windowState {
	return &windowState{
		orderCheck: newHist(),
		settlement: newHist(),
		start:      time.Now(),
	}
}

func (w *windowState) snapshot(index int) WindowSnapshot {
	// Import(Export(...)) produces an independent histogram with identical
	// bucket data. This is a lossless deep-copy: the caller owns the result
	// and the original histogram can continue to be mutated safely.
	ocCopy := hdrhistogram.Import(w.orderCheck.Export())
	setCopy := hdrhistogram.Import(w.settlement.Export())
	return WindowSnapshot{
		Index:          index,
		OrderCheck:     extract(w.orderCheck),
		Settlement:     extract(w.settlement),
		WallStart:      w.start,
		WallEnd:        time.Now(),
		OrderCheckHist: ocCopy,
		SettlementHist: setCopy,
	}
}

// Windows manages the sliding-window histograms for order-check and settlement
// latencies. It is safe for concurrent use from multiple collector goroutines.
//
// Every sample is recorded into BOTH the current window histogram AND a
// run-level merged histogram. The two recording paths are independent:
// the window histogram is sealed and replaced at each boundary; the merged
// histogram accumulates across the whole run.
type Windows struct {
	mu sync.Mutex

	unit    WindowUnit
	opsSize int64         // window size in ops (unit = ops)
	wallDur time.Duration // window size in wall time (unit = wall)

	current   *windowState
	completed []WindowSnapshot
	index     int

	// mergedOrderCheck / mergedSettlement accumulate every sample across all
	// windows and all time for the headline merged percentiles.
	mergedOrderCheck *hdrhistogram.Histogram
	mergedSettlement *hdrhistogram.Histogram

	// mergedServiceTime accumulates the SERVICE-TIME diagnostic for every
	// order-check (resolve - actual submit instant). It is NOT windowed and is
	// NEVER the headline: the headline is the open-loop order-check latency
	// (resolve - VirtualT0). Service-time appears only in the diagnostic section.
	mergedServiceTime *hdrhistogram.Histogram

	// clamped counts samples whose value exceeded the histogram ceiling and was
	// saturated to the ceiling by recordClamped (across the order-check,
	// settlement, and service-time streams). It is reported, never hidden: a
	// non-zero count means the upper tail saturated rather than being dropped.
	clamped int64
}

// NewWindows creates a Windows with the given unit and size. opsSize is the
// number of order-check operations per window (used when unit = ops); wallDur
// is the wall-clock window length (used when unit = wall).
func NewWindows(unit WindowUnit, opsSize int64, wallDur time.Duration) *Windows {
	return &Windows{
		unit:              unit,
		opsSize:           opsSize,
		wallDur:           wallDur,
		current:           newWindowState(),
		mergedOrderCheck:  newHist(),
		mergedSettlement:  newHist(),
		mergedServiceTime: newHist(),
	}
}

// RecordOrderCheck records one order-check latency. It advances the window
// when the boundary is reached.
func (w *Windows) RecordOrderCheck(d time.Duration) {
	ns := toNs(d)
	w.mu.Lock()
	defer w.mu.Unlock()
	// Clamp to the ceiling so an over-range sample saturates rather than being
	// dropped. The window and merged histograms share bounds, so a clamp on the
	// merged record is counted once for the logical sample.
	recordClamped(w.current.orderCheck, ns)
	if recordClamped(w.mergedOrderCheck, ns) {
		w.clamped++
	}
	w.current.opCount++
	w.maybeRotate()
}

// RecordSettlement records one settlement latency. Settlement is not counted
// for window rotation (windows rotate on order-check ops, which drive the
// primary window boundary).
func (w *Windows) RecordSettlement(d time.Duration) {
	ns := toNs(d)
	w.mu.Lock()
	defer w.mu.Unlock()
	recordClamped(w.current.settlement, ns)
	if recordClamped(w.mergedSettlement, ns) {
		w.clamped++
	}
}

// RecordServiceTime records one order-check SERVICE-TIME diagnostic sample
// (resolve - actual submit instant). It is a run-level merged histogram only
// (no windowing) and is kept strictly out of the headline streams: the headline
// is the open-loop order-check latency (resolve - VirtualT0). Service-time does
// not drive window rotation.
func (w *Windows) RecordServiceTime(d time.Duration) {
	ns := toNs(d)
	w.mu.Lock()
	defer w.mu.Unlock()
	if recordClamped(w.mergedServiceTime, ns) {
		w.clamped++
	}
}

// maybeRotate seals the current window and starts a fresh one if the boundary
// has been crossed. Called with w.mu held.
func (w *Windows) maybeRotate() {
	var rotate bool
	switch w.unit {
	case WindowUnitOps:
		rotate = w.opsSize > 0 && w.current.opCount >= w.opsSize
	case WindowUnitWall:
		rotate = w.wallDur > 0 && time.Since(w.current.start) >= w.wallDur
	}
	if !rotate {
		return
	}
	w.completed = append(w.completed, w.current.snapshot(w.index))
	w.index++
	w.current = newWindowState()
}

// Snapshot seals the current (possibly partial) window and returns the full
// picture: per-window history and the merged percentiles for each stream.
// Called exactly once after the run completes.
func (w *Windows) Snapshot() (windows []WindowSnapshot, orderCheck Percentiles, settlement Percentiles) {
	w.mu.Lock()
	defer w.mu.Unlock()

	// Seal the trailing partial window if it has any samples.
	if w.current.orderCheck.TotalCount() > 0 || w.current.settlement.TotalCount() > 0 {
		w.completed = append(w.completed, w.current.snapshot(w.index))
	}

	return w.completed, extract(w.mergedOrderCheck), extract(w.mergedSettlement)
}

// ServiceTime returns the merged order-check service-time diagnostic
// percentiles (resolve - actual submit). Safe to call after the run drains.
// This is a DIAGNOSTIC only and must never be presented as the headline.
func (w *Windows) ServiceTime() Percentiles {
	w.mu.Lock()
	defer w.mu.Unlock()
	return extract(w.mergedServiceTime)
}

// ClampedSamples returns the number of samples across all windowed streams
// (order-check, settlement, service-time) whose value exceeded the histogram
// ceiling and was saturated to it rather than dropped. Safe to call after the
// run drains.
func (w *Windows) ClampedSamples() int64 {
	w.mu.Lock()
	defer w.mu.Unlock()
	return w.clamped
}

// toNs converts d to nanoseconds, clamping to 1 if d <= 0 so RecordValue
// never fails on valid durations.
func toNs(d time.Duration) int64 {
	ns := d.Nanoseconds()
	if ns < 1 {
		return 1
	}
	return ns
}
