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
	"bytes"
	"crypto/sha256"
	"testing"
	"time"

	"github.com/shopspring/decimal"

	"openpit-loadtest-spot-funds-go/internal/config"
)

// testConfig builds a representative, fully-valid config in code so the
// property tests are hermetic (no INI file dependency). totalOps sizes the
// stream; seed drives determinism.
func testConfig(seed, totalOps uint64) *config.Config {
	return &config.Config{
		Run: config.Run{
			Seed:       seed,
			TotalOps:   totalOps,
			Window:     1000,
			WindowUnit: config.WindowUnitOps,
		},
		Reject:      config.Reject{TargetRate: 0.05, Tolerance: 0.005},
		Accounts:    config.Accounts{Count: 500},
		Concurrency: config.Concurrency{ActiveAccounts: 128},
		Instruments: config.Instruments{
			Symbols:    []string{"AAPL", "SPX", "MSFT", "AMZN", "GOOG", "META", "TSLA", "NVDA", "JPM", "BAC"},
			Settlement: "USD",
		},
		Lifecycle: config.Lifecycle{POpen: 0.40, PAdd: 0.15, PPartialClose: 0.25, PFullClose: 0.20},
		Funding: config.Funding{
			Trigger:   config.FundingBalanceBelow,
			Threshold: decimal.RequireFromString("100000"),
			Seed:      decimal.RequireFromString("1000000"),
			TopUp:     decimal.RequireFromString("1000000"),
		},
		Cohorts: []config.Cohort{
			{
				Name: "chatty", Weight: 0.2, Activity: 0.9, RejectPropensity: 0.7,
				BurstLen:    4,
				SizeWeights: []config.SizeBucket{{Quantity: 1, Weight: 1}, {Quantity: 10, Weight: 4}, {Quantity: 100, Weight: 2}},
				SymbolSkew:  config.SymbolSkewZipf, ZipfS: 1.3,
			},
			{
				Name: "steady", Weight: 0.5, Activity: 0.5, RejectPropensity: 0.25,
				BurstLen:    2,
				SizeWeights: []config.SizeBucket{{Quantity: 1, Weight: 2}, {Quantity: 10, Weight: 3}, {Quantity: 100, Weight: 1}},
				SymbolSkew:  config.SymbolSkewUniform,
			},
			{
				Name: "dormant", Weight: 0.3, Activity: 0.1, RejectPropensity: 0.05,
				BurstLen:    1,
				SizeWeights: []config.SizeBucket{{Quantity: 1, Weight: 5}, {Quantity: 10, Weight: 1}},
				SymbolSkew:  config.SymbolSkewUniform,
			},
		},
	}
}

func mustGenerate(t *testing.T, cfg *config.Config) *Stream {
	t.Helper()
	s, err := Generate(cfg)
	if err != nil {
		t.Fatalf("Generate() error = %v", err)
	}
	return s
}

func serialize(t *testing.T, s *Stream) []byte {
	t.Helper()
	var buf bytes.Buffer
	if err := s.Serialize(&buf); err != nil {
		t.Fatalf("Serialize() error = %v", err)
	}
	return buf.Bytes()
}

// TestDeterminismByteIdentical is the headline determinism gate: same seed +
// config must yield a byte-identical serialised stream (and therefore identical
// predictions, which are part of the serialisation).
func TestDeterminismByteIdentical(t *testing.T) {
	t.Parallel()

	cfg := testConfig(0xC0FFEE, 20000)

	a := serialize(t, mustGenerate(t, cfg))
	b := serialize(t, mustGenerate(t, cfg))

	if !bytes.Equal(a, b) {
		t.Fatalf("serialised streams differ: %x != %x", sha256.Sum256(a), sha256.Sum256(b))
	}
	if len(a) == 0 {
		t.Fatal("serialised stream is empty")
	}
}

// TestDeterminismDistinctSeeds confirms different seeds produce different
// streams (so the determinism above is not a degenerate constant).
func TestDeterminismDistinctSeeds(t *testing.T) {
	t.Parallel()

	a := serialize(t, mustGenerate(t, testConfig(1, 20000)))
	b := serialize(t, mustGenerate(t, testConfig(2, 20000)))

	if bytes.Equal(a, b) {
		t.Fatal("distinct seeds produced identical streams")
	}
}

// TestFundInvariants replays the stream through an independent ledger and
// asserts available >= 0 and held >= 0 for every account/asset at every step,
// and that the predicted post-balances in each event match the replay exactly.
func TestFundInvariants(t *testing.T) {
	t.Parallel()

	cfg := testConfig(0xBEEF, 40000)
	s := mustGenerate(t, cfg)

	replay := newLedger()
	for i := range s.Events {
		e := &s.Events[i]
		applyEventToLedger(t, replay, e)
		// After applying, the affected slots must be non-negative.
		for _, b := range e.Post {
			h, _ := replay.get(e.Account, b.Asset)
			if h.available.IsNegative() {
				t.Fatalf("event %d: %s/%s available negative: %s", e.Seq, e.Account, b.Asset, h.available)
			}
			if h.held.IsNegative() {
				t.Fatalf("event %d: %s/%s held negative: %s", e.Seq, e.Account, b.Asset, h.held)
			}
			// The replayed balance must match the predicted balance exactly.
			if !h.available.Equal(b.Available) {
				t.Fatalf("event %d: %s/%s available predicted %s, replay %s", e.Seq, e.Account, b.Asset, b.Available, h.available)
			}
			if !h.held.Equal(b.Held) {
				t.Fatalf("event %d: %s/%s held predicted %s, replay %s", e.Seq, e.Account, b.Asset, b.Held, h.held)
			}
		}
	}
}

// applyEventToLedger re-derives the ledger mutation an event implies, mirroring
// the generator's own ledger calls. Rejected order-checks are no-ops (the
// engine rolls back), exactly as the generator models them.
func applyEventToLedger(t *testing.T, l *ledger, e *Event) {
	t.Helper()
	switch e.Kind {
	case EventFunding:
		l.applyFunding(e.Account, e.FundingAsset, e.FundingKind, e.FundingAmount)
	case EventOrderCheck:
		if e.Accept {
			l.preTrade(e.Account, e.Side, e.Underlying, e.Settlement, e.Quantity, e.Price)
		}
	case EventSettlement:
		if _, err := l.settleFullFill(e.Account, e.Side, e.Underlying, e.Settlement, e.Quantity, e.Price); err != nil {
			t.Fatalf("event %d: settlement replay error: %v", e.Seq, err)
		}
	}
}

// TestLifecycleValidity replays the stream tracking per-(account,instrument)
// positions and asserts no Sell ever exceeds the held position and the position
// never goes negative - i.e. transitions respect the state machine.
func TestLifecycleValidity(t *testing.T) {
	t.Parallel()

	cfg := testConfig(0xABCD, 40000)
	s := mustGenerate(t, cfg)

	// Only accepted, settled orders move the position; track from settlements.
	pos := make(map[posKey]uint64)
	for i := range s.Events {
		e := &s.Events[i]
		if e.Kind != EventSettlement {
			continue
		}
		lots := e.Quantity.BigInt().Uint64()
		key := posKey{e.Account, e.Underlying}
		switch e.Side {
		case SideBuy:
			pos[key] += lots
		case SideSell:
			have := pos[key]
			if lots > have {
				t.Fatalf("event %d: sell %d exceeds position %d for %s/%s", e.Seq, lots, have, e.Account, e.Underlying)
			}
			pos[key] -= lots
		}
	}
}

// TestRejectControllerConvergence asserts the predicted reject rate over a large
// stream lands within tolerance of the configured target.
func TestRejectControllerConvergence(t *testing.T) {
	t.Parallel()

	for _, target := range []float64{0.01, 0.05, 0.10, 0.20} {
		cfg := testConfig(0x5EED, 200000)
		cfg.Reject.TargetRate = target
		cfg.Reject.Tolerance = 0.005

		s := mustGenerate(t, cfg)
		got := s.Stats.PredictedRejectRate()
		if diff := abs(got - target); diff > cfg.Reject.Tolerance {
			t.Fatalf("target %.3f: predicted reject rate %.5f off by %.5f > tolerance %.3f (checks=%d rejects=%d)",
				target, got, diff, cfg.Reject.Tolerance, s.Stats.OrderChecks, s.Stats.Rejects)
		}
	}
}

// TestForcedRejectsAreInsufficientFunds confirms every forced reject is the
// InsufficientFunds reason (the mechanism the controller uses) and the stream
// actually exercises both accepts and rejects.
func TestForcedRejectsReason(t *testing.T) {
	t.Parallel()

	cfg := testConfig(0x1234, 50000)
	cfg.Reject.TargetRate = 0.10
	s := mustGenerate(t, cfg)

	if s.Stats.Accepts == 0 {
		t.Fatal("stream has no accepted orders")
	}
	if s.Stats.Rejects == 0 {
		t.Fatal("stream has no rejected orders")
	}
	for i := range s.Events {
		e := &s.Events[i]
		if e.Kind == EventOrderCheck && !e.Accept && e.Reason != RejectInsufficientFunds {
			t.Fatalf("event %d: unexpected reject reason %q", e.Seq, e.Reason)
		}
	}
}

// TestOrderCheckBudgetRespected confirms the generator emits exactly TotalOps
// order-checks (deterministic budget).
func TestOrderCheckBudgetRespected(t *testing.T) {
	t.Parallel()

	const ops = 12345
	cfg := testConfig(7, ops)
	s := mustGenerate(t, cfg)
	if s.Stats.OrderChecks != ops {
		t.Fatalf("OrderChecks = %d, want %d", s.Stats.OrderChecks, ops)
	}
}

// TestSelfFundingPreventsStarvation drives a tiny, low-seed population so the
// funding trigger must fire repeatedly, and asserts (a) top-ups actually fire
// beyond the initial per-account seeds, (b) every funding event is accepted
// (never an erroneous missing+Delta reject), and (c) no account is left unable
// to trade - the predicted reject rate stays at the configured target, not
// inflated by starvation.
func TestSelfFundingPreventsStarvation(t *testing.T) {
	t.Parallel()

	cfg := testConfig(0x9, 60000)
	cfg.Accounts.Count = 3
	cfg.Funding.Seed = decimal.RequireFromString("2000")
	cfg.Funding.Threshold = decimal.RequireFromString("1000")
	cfg.Funding.TopUp = decimal.RequireFromString("2000")
	cfg.Reject.TargetRate = 0.0 // isolate funding: with no forced rejects, a
	// starving account would be the only source of rejects.

	s := mustGenerate(t, cfg)

	if s.Stats.Fundings <= cfg.Accounts.Count {
		t.Fatalf("expected top-ups beyond the %d seeds, got %d fundings", cfg.Accounts.Count, s.Stats.Fundings)
	}
	if s.Stats.Rejects != 0 {
		t.Fatalf("starvation: %d rejects with target_rate=0", s.Stats.Rejects)
	}
	for i := range s.Events {
		e := &s.Events[i]
		if e.Kind == EventFunding && !e.Accept {
			t.Fatalf("event %d: funding rejected (%s) - self-funding must always succeed", e.Seq, e.Reason)
		}
	}
}

// TestVirtualTimelineCausal asserts the virtual causal timeline invariants the
// open-loop driver depends on:
//   - seeds carry VirtualT0 == 0 (applied synchronously before the run);
//   - per account, VirtualT0 is non-decreasing in emission (Seq) order, so the
//     per-account submitter can pace its events with sequential sleeps and the
//     engine's FIFO-per-account replays the shadow's causal order;
//   - a settlement is scheduled at or after its originating order-check (a
//     report-return delay is non-negative);
//   - a same-account order-check that follows an accepted order's settlement is
//     scheduled strictly after that settlement (dependent-order causality);
//   - under a positive offered rate the timeline actually advances (not all zero).
func TestVirtualTimelineCausal(t *testing.T) {
	t.Parallel()

	cfg := testConfig(0xC0FFEE, 40000)
	cfg.Arrival = config.Arrival{OfferedRate: 50000}
	cfg.ReportDelay = config.ReportDelay{Distribution: config.ReportDelayLognormal, Mean: "2ms", Sigma: 0.5}
	s := mustGenerate(t, cfg)

	lastByAccount := make(map[string]time.Duration)
	ocVirtualByCorr := make(map[uint64]time.Duration)
	// lastSettleByAccount tracks the most recent settlement virtual time per
	// account so the next same-account order-check can be checked to follow it.
	lastSettleByAccount := make(map[string]time.Duration)
	var maxVirtual time.Duration

	for i := range s.Events {
		e := &s.Events[i]
		if e.VirtualT0 > maxVirtual {
			maxVirtual = e.VirtualT0
		}

		if e.Kind == EventFunding && e.FundingIsSeed {
			if e.VirtualT0 != 0 {
				t.Fatalf("event %d: seed VirtualT0 = %s, want 0", e.Seq, e.VirtualT0)
			}
			continue
		}

		// Per-account monotonicity in Seq order.
		if prev, ok := lastByAccount[e.Account]; ok && e.VirtualT0 < prev {
			t.Fatalf("event %d (%s): VirtualT0 %s < previous same-account %s (not monotone)",
				e.Seq, e.Kind, e.VirtualT0, prev)
		}

		switch e.Kind {
		case EventOrderCheck:
			ocVirtualByCorr[e.CorrelationID] = e.VirtualT0
			// A dependent order-check must follow the account's prior settlement.
			if settle, ok := lastSettleByAccount[e.Account]; ok && e.VirtualT0 < settle {
				t.Fatalf("event %d: order-check VirtualT0 %s precedes prior same-account settlement %s",
					e.Seq, e.VirtualT0, settle)
			}
		case EventSettlement:
			oc, ok := ocVirtualByCorr[e.CorrelationID]
			if !ok {
				t.Fatalf("event %d: settlement corr %d has no preceding order-check", e.Seq, e.CorrelationID)
			}
			if e.VirtualT0 < oc {
				t.Fatalf("event %d: settlement VirtualT0 %s precedes its order-check %s (negative report delay)",
					e.Seq, e.VirtualT0, oc)
			}
			lastSettleByAccount[e.Account] = e.VirtualT0
		}
		lastByAccount[e.Account] = e.VirtualT0
	}

	if maxVirtual <= 0 {
		t.Fatal("virtual timeline never advanced (max VirtualT0 == 0) under a positive offered rate")
	}
}

// TestVirtualTimelineDeterministic confirms the virtual times themselves are a
// pure function of (seed, config): two generations with the same seed produce
// identical VirtualT0 sequences, and a different seed perturbs them.
func TestVirtualTimelineDeterministic(t *testing.T) {
	t.Parallel()

	cfg := testConfig(0x5EED, 20000)
	cfg.Arrival = config.Arrival{OfferedRate: 50000}
	cfg.ReportDelay = config.ReportDelay{Distribution: config.ReportDelayLognormal, Mean: "2ms", Sigma: 0.5}

	a := mustGenerate(t, cfg)
	b := mustGenerate(t, cfg)
	if len(a.Events) != len(b.Events) {
		t.Fatalf("event counts differ: %d vs %d", len(a.Events), len(b.Events))
	}
	for i := range a.Events {
		if a.Events[i].VirtualT0 != b.Events[i].VirtualT0 {
			t.Fatalf("event %d VirtualT0 differs across runs: %s vs %s",
				i, a.Events[i].VirtualT0, b.Events[i].VirtualT0)
		}
	}

	cfg2 := testConfig(0x5EEE, 20000)
	cfg2.Arrival = cfg.Arrival
	cfg2.ReportDelay = cfg.ReportDelay
	c := mustGenerate(t, cfg2)
	same := len(c.Events) == len(a.Events)
	if same {
		for i := range a.Events {
			if a.Events[i].VirtualT0 != c.Events[i].VirtualT0 {
				same = false
				break
			}
		}
	}
	if same {
		t.Fatal("distinct seeds produced identical virtual timelines")
	}
}

func abs(x float64) float64 {
	if x < 0 {
		return -x
	}
	return x
}
