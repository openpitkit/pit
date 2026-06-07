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

package driver

import (
	"sync/atomic"
	"time"

	"openpit-loadtest-spot-funds-go/internal/measurement"
)

// opClass distinguishes the two measured latency streams.
type opClass uint8

const (
	// opOrderCheck is the stage 1->2 order-check latency (the headline).
	opOrderCheck opClass = iota
	// opSettlement is the stage 3->4 report-settlement latency.
	opSettlement
)

// resultSink is the driver-level facade over the measurement.Sink. It routes
// each resolved operation to the correct histogram stream and exposes the
// per-op oracle's SampleCount witness.
type resultSink struct {
	m *measurement.Sink
	// sampleCount tracks total resolved samples for the open-loop witness test.
	// Multiple collector goroutines call recordResolve concurrently, so this
	// must be accessed atomically.
	sampleCount atomic.Int64
}

func newResultSink(w *measurement.Windows) *resultSink {
	return &resultSink{m: measurement.NewSink(w)}
}

// recordSubmit notes one more in-flight operation.
func (s *resultSink) recordSubmit() {
	s.m.RecordSubmit()
}

// recordResolve records a resolved operation into the appropriate measurement
// stream. The latency value is the open-loop resolve - VirtualT0 computed by the
// collector (never recomputed here): for an order-check this is the HEADLINE
// open-loop latency-under-load, for a settlement the settlement open-loop
// latency.
func (s *resultSink) recordResolve(class opClass, latency time.Duration, accepted bool) {
	switch class {
	case opOrderCheck:
		s.m.RecordOrderCheck(latency, accepted)
	case opSettlement:
		s.m.RecordSettlement(latency, accepted)
	}
	s.sampleCount.Add(1)
}

// recordFunding records one resolved runtime funding (top-up) adjustment.
// Funding is NOT an order-check: it records into NO latency histogram and does
// not touch the order-check counters or the achieved-reject-rate denominator;
// it updates only the funding accept/reject tally (see
// measurement.Sink.RecordFunding). It still counts as one resolved sample for
// the open-loop witness.
func (s *resultSink) recordFunding(accepted bool) {
	s.m.RecordFunding(accepted)
	s.sampleCount.Add(1)
}

// recordServiceTime records one order-check SERVICE-TIME diagnostic sample
// (resolve - actual submit instant). It is a DIAGNOSTIC only and never the
// headline; see measurement.Sink.RecordServiceTime.
func (s *resultSink) recordServiceTime(serviceTime time.Duration) {
	s.m.RecordServiceTime(serviceTime)
}

// recordBackpressure records one submit the engine refused with a
// dispatch-capacity backpressure signal (ErrQueueLimit). It is an explicit
// measured outcome surfaced in the snapshot, never a panic or a silent drop.
func (s *resultSink) recordBackpressure(latency time.Duration) {
	s.m.RecordBackpressure(latency)
}

// recordHandoffStall records one HARNESS handoff stall (collector -> finalizer
// fast path full). It is a HARNESS-side starvation witness surfaced in the
// snapshot, never a panic or a silent absorb. Engine-await latency stays in the
// headline and is never recorded here.
func (s *resultSink) recordHandoffStall() {
	s.m.RecordHandoffStall()
}

// recordWorkOverflowDepth updates the running peak submitter -> collector spill
// depth. DIAGNOSTIC only — not a stall and not an INVALID trigger.
func (s *resultSink) recordWorkOverflowDepth(depth int) {
	s.m.RecordWorkOverflowDepth(depth)
}

// Stats is the immutable summary the run returns to its caller.
type Stats struct {
	// OrderChecks is the count of resolved order-CLASS async ops: real
	// order-checks plus runtime funding (top-up) adjustments. It is the
	// "every async order-class op was consumed" witness; seeds are applied
	// synchronously before the run and are not included. The HEADLINE
	// order-check count (excluding funding) lives in the measurement Snapshot
	// (Snapshot.TotalOrderChecks).
	OrderChecks uint64
	// Settlements is the count of resolved settlement ops.
	Settlements uint64
	// Accepts / Rejects are the ORDER-CHECK decisions only (funding and
	// settlement outcomes are not folded in, so accepts never exceed the real
	// order-check count). Funding outcomes are reported separately.
	Accepts uint64
	Rejects uint64
	// Fundings / FundingAccepts / FundingRejects are the runtime funding (top-up)
	// adjustments resolved on the async path, kept distinct from order-checks.
	Fundings       uint64
	FundingAccepts uint64
	FundingRejects uint64
	// Backpressure counts submits the engine refused with a dispatch-capacity
	// backpressure signal (ErrQueueLimit); 0 in a healthy run.
	Backpressure uint64
	// HandoffStalls counts HARNESS-internal handoff stalls (collector -> finalizer
	// fast path full); a HARNESS-side starvation witness, 0 in a healthy run. It
	// never counts engine-await latency, which stays in the headline.
	HandoffStalls uint64
	// MaxWorkOverflow is the peak depth of the submitter -> collector spill
	// (workOverflow). DIAGNOSTIC only — not a stall, not an INVALID trigger.
	MaxWorkOverflow int
	// Checksum proves every decision was consumed (anti-DCE).
	Checksum uint64
	// MaxInFlight is the peak concurrent submitted-but-unresolved op count: the
	// open-loop witness (> 1 means submissions overlapped decisions).
	MaxInFlight int64
	// SampleCount is the number of recorded latency samples.
	SampleCount int
}

func (s *resultSink) stats() Stats {
	ms := s.m.Stats()
	return Stats{
		// Order-class consumed witness = real order-checks + runtime fundings.
		OrderChecks:     ms.OrderChecks + ms.Fundings,
		Settlements:     ms.Settlements,
		Accepts:         ms.OrderCheckAccepts,
		Rejects:         ms.OrderCheckRejects,
		Fundings:        ms.Fundings,
		FundingAccepts:  ms.FundingAccepts,
		FundingRejects:  ms.FundingRejects,
		Backpressure:    ms.Backpressure,
		HandoffStalls:   ms.HandoffStalls,
		MaxWorkOverflow: ms.MaxWorkOverflow,
		Checksum:        ms.Checksum,
		MaxInFlight:     ms.MaxInFlight,
		SampleCount:     int(s.sampleCount.Load()),
	}
}

// live returns a race-safe snapshot of the in-progress counters for the
// progress reporter. Read-only; does not affect any measurement.
func (s *resultSink) live() measurement.LiveCounters {
	return s.m.Live()
}
