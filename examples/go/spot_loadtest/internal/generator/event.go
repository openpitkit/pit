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
	"bufio"
	"fmt"
	"io"
	"time"

	"github.com/shopspring/decimal"
)

// EventKind discriminates the abstract event types in the pre-materialised
// stream.
type EventKind uint8

const (
	// EventOrderCheck maps to ExecutePreTrade in Phase 3 (stage 1->2). Carries
	// the predicted accept/reject and the predicted post-op balance of the
	// charged asset.
	EventOrderCheck EventKind = iota
	// EventSettlement maps to ApplyExecutionReport in Phase 3 (stage 3->4) for a
	// previously accepted order. Carries the predicted post-settlement balances.
	EventSettlement
	// EventFunding maps to ApplyAccountAdjustment in Phase 3. Carries the
	// asset, kind, amount and predicted post-balance.
	EventFunding
)

// String renders the kind as a stable token used in serialisation.
func (k EventKind) String() string {
	switch k {
	case EventOrderCheck:
		return "ORDERCHECK"
	case EventSettlement:
		return "SETTLEMENT"
	case EventFunding:
		return "FUNDING"
	default:
		return "UNKNOWN"
	}
}

// Balance is a predicted (available, held) pair for one asset after an op. It
// is the oracle's expectation for the engine's post-op state.
type Balance struct {
	Asset     string
	Available decimal.Decimal
	Held      decimal.Decimal
}

// Event is one abstract, typed, serialisable load-test event. A single struct
// carries every kind; unused fields stay at their zero value. Every field is
// chosen so the event can be (a) mapped to a concrete engine call in Phase 3
// and (b) checked by the oracle against the engine's response.
//
// The predictions (Accept/Reason/Post) ARE the oracle: Phase 3 asserts the
// engine reproduces them exactly, per account, in account event order.
type Event struct {
	// Seq is the global emission index (0-based), the stream's total order.
	Seq  uint64
	Kind EventKind

	// VirtualT0 is the event's intended arrival time on the offline virtual
	// causal timeline, measured from run start. It is what the driver paces to
	// (open-loop) and stamps as the measured t0, so the headline latency is
	// resolve - VirtualT0 (honest open-loop, coordinated-omission-defended).
	//
	// Assignment (see assignVirtualTimes), all deterministic from (seed, config):
	//   - order-check arrivals follow the offered process ([arrival] offered_rate,
	//     poisson) across accounts;
	//   - a settlement's VirtualT0 = its order-check's VirtualT0 + a report-return
	//     delay sampled from [report_delay];
	//   - a causally-dependent event (a same-account order after the prior order's
	//     hold/fill, a top-up before the order it funds) has VirtualT0 >= the
	//     dependency's virtual completion + a small gap.
	// Seeds carry VirtualT0 = 0: they are applied synchronously before the run
	// (setup, not paced load), so their virtual time is never used for pacing.
	VirtualT0 time.Duration

	// Account is the engine account key string (FNV-hashed by the binding).
	Account string

	// --- order / settlement fields (zero for funding) ---

	Underlying string
	Settlement string
	Side       Side
	// Quantity is the integer lot count; Price is the limit price (always set
	// in v1). Both are exact decimals at the pinned scales.
	Quantity decimal.Decimal
	Price    decimal.Decimal

	// CorrelationID ties a settlement back to the order-check it settles, and
	// is set on the originating order-check too. Zero for funding.
	CorrelationID uint64

	// --- order-check prediction ---

	// Accept is the predicted order-check outcome.
	Accept bool
	// Reason is the predicted reject reason (empty when Accept).
	Reason RejectReason

	// --- funding fields (zero for order/settlement) ---

	FundingKind   fundingKind
	FundingAsset  string
	FundingAmount decimal.Decimal
	// FundingIsSeed marks the initial per-account balance seed (as opposed to a
	// runtime top-up). Seeds are SETUP, not measured load: the driver applies
	// them synchronously on the underlying engine before the async run, while
	// top-ups flow through the measured async pipeline. The shadow oracle
	// verifies both. Only meaningful when Kind == EventFunding.
	FundingIsSeed bool

	// Post holds the predicted post-op balances. For an order-check it is the
	// charged asset (1 entry); for a settlement it is the two affected assets;
	// for funding it is the funded asset (1 entry). Order within Post is stable
	// (charge/held leg first) for byte-identical serialisation.
	Post []Balance
}

// FundingIsDelta reports whether a Funding event applies a Delta (relative)
// balance adjustment rather than an Absolute (set) one. The driver needs this
// to build the matching AccountAdjustment, and the underlying funding-kind type
// is intentionally unexported, so this predicate is the stable accessor.
func (e *Event) FundingIsDelta() bool {
	return e.FundingKind == fundingDelta
}

// Stream is the pre-materialised, deterministic sequence of events plus summary
// metadata for the driver and reporter.
type Stream struct {
	Events []Event
	// Stats summarises the run for the reporter / convergence checks.
	Stats StreamStats
}

// StreamStats captures aggregate counts over the generated stream.
type StreamStats struct {
	OrderChecks    uint64
	Accepts        uint64
	Rejects        uint64
	Settlements    uint64
	Fundings       uint64
	ForcedRejects  uint64
	NaturalRejects uint64
	// Seeds is the number of initial per-account balance seeds (a subset of
	// Fundings). Seeds are applied synchronously by the driver before the async
	// run; the remaining Fundings-Seeds events are runtime top-ups in the async
	// pipeline.
	Seeds uint64
}

// PredictedRejectRate is the fraction of order-checks predicted to reject.
func (s StreamStats) PredictedRejectRate() float64 {
	if s.OrderChecks == 0 {
		return 0
	}
	return float64(s.Rejects) / float64(s.OrderChecks)
}

// Serialize writes a deterministic, line-oriented text encoding of the stream
// to w. The encoding is total and stable: every event is one line with a fixed
// field order and decimals rendered via String(), so the same seed + config
// yields byte-identical output. This is the artifact the determinism property
// test hashes and compares.
func (s *Stream) Serialize(w io.Writer) error {
	bw := bufio.NewWriter(w)
	for i := range s.Events {
		if err := s.Events[i].writeLine(bw); err != nil {
			return err
		}
	}
	return bw.Flush()
}

// writeLine emits one event as a single deterministic line.
func (e *Event) writeLine(w io.Writer) error {
	// Fixed field order; decimals via String() so the textual form is exact.
	// VirtualT0 is rendered in integer nanoseconds so the virtual causal timeline
	// is part of the byte-identical determinism artifact.
	if _, err := fmt.Fprintf(w, "%d|%s|vt0=%dns|%s|%s|%s|%s|q=%s|p=%s|corr=%d|acc=%t|rej=%s|fk=%s|fa=%s|fv=%s|seed=%t|",
		e.Seq, e.Kind, e.VirtualT0.Nanoseconds(), e.Account, e.Underlying, e.Settlement, e.Side,
		decStr(e.Quantity), decStr(e.Price), e.CorrelationID, e.Accept, e.Reason,
		e.FundingKind, e.FundingAsset, decStr(e.FundingAmount), e.FundingIsSeed,
	); err != nil {
		return err
	}
	for _, b := range e.Post {
		if _, err := fmt.Fprintf(w, "{%s:av=%s:hd=%s}", b.Asset, decStr(b.Available), decStr(b.Held)); err != nil {
			return err
		}
	}
	_, err := fmt.Fprintln(w)
	return err
}

// decStr renders a decimal deterministically. A zero-value (uninitialised)
// decimal.Decimal renders as "0" so empty fields are stable across runs.
func decStr(d decimal.Decimal) string {
	return d.String()
}
