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
	"sync/atomic"
	"time"
)

// checksumMix is the 64-bit golden-ratio odd constant used to spread bits
// through the anti-dead-code-elimination checksum.
const checksumMix = 0x9e3779b97f4a7c15

// Sink accumulates resolved operation measurements into HdrHistogram windows,
// maintains counters, and computes an anti-DCE checksum over every decision.
// It is thread-safe; all methods may be called concurrently from multiple
// collector goroutines.
type Sink struct {
	windows *Windows

	mu sync.Mutex
	// Operation-class counters are kept STRICTLY SEPARATE so the headline never
	// mixes classes: order-check accepts/rejects, settlement accepts/blocks, and
	// funding accepts/rejects each have their own fields. Only order-checks feed
	// the headline latency histogram and the achieved-reject-rate denominator.
	orderChecks       uint64
	orderCheckAccepts uint64
	orderCheckRejects uint64
	settlements       uint64
	settlementAccepts uint64
	settlementBlocks  uint64
	// fundings counts runtime funding (top-up) adjustments resolved on the async
	// path. Funding is NOT an order-check: it never touches the order-check
	// latency histogram, the order-check counters, or the achieved-reject-rate
	// denominator. It carries its own accept/reject tally for disclosure only.
	fundings       uint64
	fundingAccepts uint64
	fundingRejects uint64
	checksum       uint64
	// backpressure counts submits the engine refused with a dispatch-capacity
	// backpressure signal (ErrQueueLimit). It is an EXPLICIT MEASURED outcome,
	// never a silent drop; a healthy baseline run reports zero.
	backpressure uint64
	// handoffStalls counts the times a HARNESS-internal handoff fast path was full
	// and the submit schedule would otherwise have blocked (the collector ->
	// finalizer handoff backlog). It is a HARNESS-side starvation witness, NOT an
	// engine signal: a collector legitimately blocked in fut.Await because the
	// engine is slow is REAL latency that stays in the headline and is never
	// counted here. A non-zero count means the harness fell behind and the run is
	// invalid (the headline could fold the harness backlog into engine latency).
	// It is NOT folded into the anti-DCE checksum (that proves engine DECISIONS
	// were consumed; a stall is harness bookkeeping, not a decision) and it does
	// NOT release an in-flight slot (the op still resolves normally).
	handoffStalls uint64
	// maxWorkOverflow is the peak depth of the submitter -> collector overflow
	// spill (workOverflow). It is a HARNESS DIAGNOSTIC only; it is never an
	// INVALID trigger and is never folded into the anti-DCE checksum. A large
	// value means collectors lagged submission — usually because they were
	// legitimately blocked in fut.Await (real engine latency, correctly in the
	// headline), but under host CPU starvation it can include collector-dispatch
	// delay that inflates — never flatters — the tail.
	maxWorkOverflow int

	// inFlight / maxInFlight track the open-loop depth as the Phase-3 witness.
	inFlight    int64
	maxInFlight int64

	// wallStart is set on the first RecordSubmit so throughput can be computed.
	wallStart    time.Time
	wallStartSet atomic.Bool
	wallEnd      time.Time
}

// NewSink constructs a Sink that records into the provided Windows.
func NewSink(w *Windows) *Sink {
	return &Sink{windows: w}
}

// RecordSubmit notes one more submitted (in-flight) operation and tracks the peak.
func (s *Sink) RecordSubmit() {
	if s.wallStartSet.CompareAndSwap(false, true) {
		s.mu.Lock()
		s.wallStart = time.Now()
		s.mu.Unlock()
	}
	s.mu.Lock()
	s.inFlight++
	if s.inFlight > s.maxInFlight {
		s.maxInFlight = s.inFlight
	}
	s.mu.Unlock()
}

// RecordOrderCheck records one resolved order-check operation. latency is the
// CO-correct resolve - intendedT0 value supplied by the driver. accepted
// distinguishes an allow from a policy reject. The checksum folds both the
// latency and the decision so neither can be elided by the compiler.
func (s *Sink) RecordOrderCheck(latency time.Duration, accepted bool) {
	s.windows.RecordOrderCheck(latency)
	s.mu.Lock()
	s.inFlight--
	s.orderChecks++
	if accepted {
		s.orderCheckAccepts++
	} else {
		s.orderCheckRejects++
	}
	// Fold latency and accepted into the checksum. Wraparound is intentional.
	bit := uint64(0)
	if accepted {
		bit = 1
	}
	s.checksum ^= (uint64(latency.Nanoseconds()) + checksumMix) ^ bit //nolint:gosec // checksum mixing
	s.wallEnd = time.Now()
	s.mu.Unlock()
}

// RecordSettlement records one resolved settlement operation. accepted is false
// when the report produced an account block. Settlement accepts are counted in
// their OWN field (never the order-check accept field) so the report can show
// them distinctly and a reader never sees accepts exceed order-checks.
func (s *Sink) RecordSettlement(latency time.Duration, accepted bool) {
	s.windows.RecordSettlement(latency)
	s.mu.Lock()
	s.inFlight--
	s.settlements++
	if accepted {
		s.settlementAccepts++
	} else {
		s.settlementBlocks++
	}
	// Distinguish settlement records from order-check records in the checksum
	// by XORing with a small constant. The value (2) is a deliberate slot
	// assignment, not a measurement quantity.
	const settlementSlot = uint64(2)
	s.checksum ^= (uint64(latency.Nanoseconds()) + checksumMix) ^ settlementSlot //nolint:gosec // checksum mixing
	s.wallEnd = time.Now()
	s.mu.Unlock()
}

// RecordServiceTime records one order-check SERVICE-TIME diagnostic sample
// (resolve - actual submit instant). It feeds a run-level merged histogram only
// and is NEVER part of the headline (the headline is the open-loop order-check
// latency, resolve - VirtualT0). It touches no counters and is not folded into
// the anti-DCE checksum (service-time is a derived diagnostic, not a decision);
// recording it into the reported histogram is what keeps the path from being
// elided. It is safe to call concurrently from the collector pool.
func (s *Sink) RecordServiceTime(latency time.Duration) {
	s.windows.RecordServiceTime(latency)
}

// RecordFunding records one resolved runtime funding (top-up) adjustment.
// Funding is NOT one of the measured latency classes: it records into NO
// histogram (not the order-check headline, not any other), and it does NOT
// touch the order-check counters or the achieved-reject-rate denominator. It
// updates only its own funding accept/reject tally (for disclosure), releases
// the in-flight slot the submit took, and folds the decision into the anti-DCE
// checksum so the work cannot be elided. Safe to call from the collector pool.
func (s *Sink) RecordFunding(accepted bool) {
	s.mu.Lock()
	s.inFlight--
	s.fundings++
	if accepted {
		s.fundingAccepts++
	} else {
		s.fundingRejects++
	}
	// Distinguish funding records from order-check/settlement records in the
	// checksum by XORing with a distinct slot (4). Funding carries no measured
	// latency, so the decision bit alone is folded.
	const fundingSlot = uint64(4)
	bit := uint64(0)
	if accepted {
		bit = 1
	}
	s.checksum ^= (checksumMix ^ fundingSlot) ^ bit
	s.wallEnd = time.Now()
	s.mu.Unlock()
}

// RecordBackpressure records one submit the engine refused with a
// dispatch-capacity backpressure signal (e.g. ErrQueueLimit). The submit was
// already counted in-flight by RecordSubmit, so this releases that slot and
// folds the event into the checksum (the backpressure decision is real work
// that must not be elided). latency is resolve - intendedT0, kept for the
// checksum only; backpressured submits are not added to the latency histograms
// because they never reached a decision.
func (s *Sink) RecordBackpressure(latency time.Duration) {
	s.mu.Lock()
	s.inFlight--
	s.backpressure++
	const backpressureSlot = uint64(3)
	s.checksum ^= (uint64(latency.Nanoseconds()) + checksumMix) ^ backpressureSlot //nolint:gosec // checksum mixing
	s.wallEnd = time.Now()
	s.mu.Unlock()
}

// RecordHandoffStall records one HARNESS handoff stall: the collector ->
// finalizer fast path was full and the submit schedule would otherwise have
// been throttled by the harness's own off-path backlog. It is a HARNESS-side
// starvation witness only; engine-await latency (a collector legitimately
// blocked in fut.Await because the engine is slow) is REAL latency, stays in
// the headline, and is NEVER recorded here. It only bumps the counter: unlike
// backpressure there is no in-flight slot to release (the op still resolves) and
// it is deliberately NOT folded into the anti-DCE checksum, which proves engine
// DECISIONS were consumed, not that a harness queue overflowed.
func (s *Sink) RecordHandoffStall() {
	s.mu.Lock()
	s.handoffStalls++
	s.mu.Unlock()
}

// RecordWorkOverflowDepth updates the running peak submitter -> collector spill
// depth (maxWorkOverflow). It is a HARNESS DIAGNOSTIC only: a large peak means
// collectors lagged submission — usually because they were legitimately blocked
// in fut.Await (real engine latency, correctly in the headline), but under host
// CPU starvation it can include collector-dispatch delay that inflates — never
// flatters — the tail. It is NOT an INVALID trigger and is NOT folded into the
// anti-DCE checksum.
func (s *Sink) RecordWorkOverflowDepth(depth int) {
	s.mu.Lock()
	if depth > s.maxWorkOverflow {
		s.maxWorkOverflow = depth
	}
	s.mu.Unlock()
}

// SinkStats is the immutable summary the driver exposes after a run. The accept
// and reject tallies are kept per operation class so the report never conflates
// them: OrderCheckAccepts/OrderCheckRejects are the headline class, settlement
// and funding tallies are separate and never inflate the order-check numbers.
type SinkStats struct {
	OrderChecks       uint64
	OrderCheckAccepts uint64 // order-check accepts only
	OrderCheckRejects uint64 // order-check rejects only; excludes settlement/funding
	Settlements       uint64
	SettlementAccepts uint64 // settlements that did not produce an account block
	SettlementBlocks  uint64 // settlements that produced an account block
	Fundings          uint64 // runtime funding (top-up) adjustments; not order-checks
	FundingAccepts    uint64
	FundingRejects    uint64
	Backpressure      uint64
	// HandoffStalls is the number of HARNESS-internal handoff stalls (collector ->
	// finalizer fast path full); a HARNESS-side starvation witness, 0 in a healthy
	// run. It excludes engine-await latency, which stays in the headline.
	HandoffStalls uint64
	// MaxWorkOverflow is the peak depth of the submitter -> collector spill
	// (workOverflow). DIAGNOSTIC only — not an INVALID trigger.
	MaxWorkOverflow int
	Checksum        uint64
	MaxInFlight     int64
	WallStart       time.Time
	WallEnd         time.Time
}

// Stats returns an immutable copy of the current counters. Call after the run
// has fully drained (all futures resolved).
func (s *Sink) Stats() SinkStats {
	s.mu.Lock()
	defer s.mu.Unlock()
	return SinkStats{
		OrderChecks:       s.orderChecks,
		OrderCheckAccepts: s.orderCheckAccepts,
		OrderCheckRejects: s.orderCheckRejects,
		Settlements:       s.settlements,
		SettlementAccepts: s.settlementAccepts,
		SettlementBlocks:  s.settlementBlocks,
		Fundings:          s.fundings,
		FundingAccepts:    s.fundingAccepts,
		FundingRejects:    s.fundingRejects,
		Backpressure:      s.backpressure,
		HandoffStalls:     s.handoffStalls,
		MaxWorkOverflow:   s.maxWorkOverflow,
		Checksum:          s.checksum,
		MaxInFlight:       s.maxInFlight,
		WallStart:         s.wallStart,
		WallEnd:           s.wallEnd,
	}
}

// LiveCounters is a race-safe snapshot of in-progress counters for the
// progress reporter. It may be called concurrently with recording methods.
type LiveCounters struct {
	// Submitted is the total number of submit calls recorded so far.
	Submitted uint64
	// Decided is the total number of resolved operations (order-checks +
	// settlements + fundings). Decided = Submitted - InFlight (approximately).
	Decided uint64
	// InFlight is the current count of submitted-but-unresolved operations.
	InFlight int64
}

// Live returns a race-safe snapshot of the live counters. Safe to call at any
// time during the run; it acquires the same mutex as the recording methods.
func (s *Sink) Live() LiveCounters {
	s.mu.Lock()
	defer s.mu.Unlock()
	decided := s.orderChecks + s.settlements + s.fundings
	return LiveCounters{
		Submitted: decided + uint64(clampPositive(s.inFlight)), //nolint:gosec // inFlight is always >= 0 in normal operation
		Decided:   decided,
		InFlight:  s.inFlight,
	}
}

func clampPositive(n int64) int64 {
	if n < 0 {
		return 0
	}
	return n
}
