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
	"context"
	"testing"
	"time"

	"github.com/shopspring/decimal"

	"openpit-loadtest-spot-funds-go/internal/config"
	"openpit-loadtest-spot-funds-go/internal/generator"
	"openpit-loadtest-spot-funds-go/internal/measurement"
)

// testConfig builds a representative, fully-valid config in code (mirrors the
// generator package's test config) so the integration test is hermetic.
func testConfig(seed, totalOps uint64, target float64) *config.Config {
	return &config.Config{
		Run: config.Run{
			Seed:       seed,
			TotalOps:   totalOps,
			Window:     1000,
			WindowUnit: config.WindowUnitOps,
			Observer:   true,
		},
		// OfferedRate 0 = unpaced (saturated): every virtual arrival is at t0, so
		// the submitter issues as fast as it can. Inter-arrival spacing is always
		// exponential (Poisson); there is no "fixed" arrival mode.
		Arrival:     config.Arrival{OfferedRate: 0},
		Reject:      config.Reject{TargetRate: target, Tolerance: 0.01},
		Accounts:    config.Accounts{Count: 200},
		Concurrency: config.Concurrency{ActiveAccounts: 64},
		AsyncEngine: config.AsyncEngine{Strategy: config.AsyncEngineStrategyDynamic, MaxQueues: 256, IdleCleanup: 2 * time.Second},
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

// TestDriverOraclePipeline is the Phase-3 / Phase-4 integration gate. It runs
// a moderate generator stream through the REAL openpit asyncengine and asserts:
//
//   - the per-op oracle agrees with the engine over the whole stream;
//   - the aggregate fund-conservation and no-oversell invariants hold;
//   - submission was TRUE open-loop: max in-flight is well above 1 (genuine
//     pipelining), not merely overlapping by one;
//   - measurement windows are populated (non-zero counts, non-zero p99);
//   - inner metrics are populated when observer is enabled;
//   - the overhead probe ran and returned a positive distribution;
//   - the service-time diagnostic was populated;
//   - the checksum changed across the run (anti-DCE proof).
func TestDriverOraclePipeline(t *testing.T) {
	cfg := testConfig(0xC0FFEE, 30000, 0.05)
	stream, err := generator.Generate(cfg)
	if err != nil {
		t.Fatalf("Generate() error = %v", err)
	}
	if stream.Stats.Accepts == 0 || stream.Stats.Rejects == 0 {
		t.Fatalf("stream lacks a mix: accepts=%d rejects=%d", stream.Stats.Accepts, stream.Stats.Rejects)
	}

	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()

	stats, snap, err := Run(ctx, stream, Config{
		Observer:       true,
		Collectors:     16,
		WindowSize:     1000,
		WindowUnit:     measurement.WindowUnitOps,
		OverheadProbes: 50,
	})
	if err != nil {
		t.Fatalf("Run() oracle/invariant error = %v", err)
	}

	// Every order-check and runtime funding must have been consumed on the async
	// path. Seeds are applied synchronously before the run, so they are NOT among
	// the async order-class ops (consumed = order-checks + (fundings - seeds)).
	wantOrderEvents := stream.Stats.OrderChecks + (stream.Stats.Fundings - stream.Stats.Seeds)
	if stats.OrderChecks != wantOrderEvents {
		t.Errorf("consumed order-class ops = %d, want %d (order-checks %d + top-ups %d)",
			stats.OrderChecks, wantOrderEvents, stream.Stats.OrderChecks,
			stream.Stats.Fundings-stream.Stats.Seeds)
	}
	if stats.Settlements != stream.Stats.Settlements {
		t.Errorf("consumed settlements = %d, want %d", stats.Settlements, stream.Stats.Settlements)
	}
	if stats.SampleCount == 0 {
		t.Fatal("no latency samples recorded")
	}

	// Open-loop witness: with the submitter never blocking on a decision, the
	// peak in-flight must reflect GENUINE pipelining, well above the bounded
	// active-account count (an account fires its next event without awaiting the
	// previous one's decision). A value barely above 1 would mean the old
	// closed-loop-within-account behaviour leaked back in.
	wantInFlight := int64(cfg.Concurrency.ActiveAccounts) //nolint:gosec // active accounts is a small configured bound
	if stats.MaxInFlight <= wantInFlight {
		t.Fatalf("max in-flight = %d, want > active accounts %d (true open-loop must pipeline deeply)",
			stats.MaxInFlight, wantInFlight)
	}

	// Measurement snapshot: windows must exist.
	if len(snap.Windows) == 0 {
		t.Fatal("snapshot has no windows")
	}
	// Merged order-check must have samples.
	if snap.OrderCheck.Count == 0 {
		t.Fatal("merged order-check histogram is empty")
	}
	// p99 must be positive (the engine does real work).
	if snap.OrderCheck.P99 <= 0 {
		t.Errorf("merged order-check p99 = %v, want > 0", snap.OrderCheck.P99)
	}

	// Service-time diagnostic must be populated (one sample per order-check) and
	// must never be presented as the headline by the reporter.
	if snap.ServiceTime.Count == 0 {
		t.Error("service-time diagnostic histogram is empty")
	}

	// Inner metrics: observer was on, so dequeues and completes must be > 0.
	if snap.InnerMetrics.Dequeues == 0 {
		t.Error("inner metrics: Dequeues = 0 with observer enabled")
	}
	if snap.InnerMetrics.Completes == 0 {
		t.Error("inner metrics: Completes = 0 with observer enabled")
	}

	// Overhead probe ran.
	if snap.Overhead.Probes == 0 {
		t.Error("overhead probe did not run")
	}
	if snap.Overhead.Distribution.P50 <= 0 {
		t.Errorf("overhead p50 = %v, want > 0", snap.Overhead.Distribution.P50)
	}

	// Checksum must be non-zero.
	if stats.Checksum == 0 {
		t.Error("checksum is zero — anti-DCE proof failed")
	}

	// Handoff stalls are a DIAGNOSTIC, not a validity trigger. The collector ->
	// finalizer handoff is non-blocking (it spills to an unbounded off-path
	// overflow) and CommitAndClose is fully off the measured path, so under
	// saturation (unpaced: every arrival at t0) the finalizer pool may transiently
	// lag and the stall counter may be non-zero WITHOUT contaminating the headline
	// or invalidating the run. We log the count for visibility but do not assert it.
	t.Logf("handoff stalls = %d (diagnostic only; off-path, does not invalidate the run)",
		stats.HandoffStalls)

	t.Logf("ops=%d settlements=%d accepts=%d rejects=%d maxInFlight=%d checksum=%#x",
		stats.OrderChecks, stats.Settlements, stats.Accepts, stats.Rejects, stats.MaxInFlight, stats.Checksum)
	t.Logf("windows=%d openloop_p50=%v openloop_p99=%v servicetime_p50=%v overhead_p50=%v",
		len(snap.Windows), snap.OrderCheck.P50, snap.OrderCheck.P99, snap.ServiceTime.P50, snap.Overhead.Distribution.P50)
	t.Logf("inner: dequeues=%d completes=%d queueWait_p50=%v engineCompute_p50=%v",
		snap.InnerMetrics.Dequeues, snap.InnerMetrics.Completes,
		snap.InnerMetrics.QueueWait.P50, snap.InnerMetrics.EngineCompute.P50)
}

// TestDriverOpenLoopHighRejectRate raises the reject target so a large fraction
// of order-checks are rejected, exercising the reject-code mapping heavily, and
// confirms the oracle still agrees end to end.
func TestDriverOpenLoopHighRejectRate(t *testing.T) {
	cfg := testConfig(0xBEEF, 20000, 0.20)
	stream, err := generator.Generate(cfg)
	if err != nil {
		t.Fatalf("Generate() error = %v", err)
	}
	if stream.Stats.Rejects == 0 {
		t.Fatal("high-reject stream has no rejects")
	}

	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()

	stats, snap, err := Run(ctx, stream, Config{
		Observer:   false,
		Collectors: 16,
		WindowSize: 1000,
	})
	if err != nil {
		t.Fatalf("Run() oracle/invariant error = %v", err)
	}
	if stats.Rejects == 0 {
		t.Fatal("engine produced no rejects for a high-reject stream")
	}
	// True open-loop pipelining: deep in-flight, above the active-account count.
	wantInFlight := int64(cfg.Concurrency.ActiveAccounts) //nolint:gosec // active accounts is a small configured bound
	if stats.MaxInFlight <= wantInFlight {
		t.Fatalf("max in-flight = %d, want > active accounts %d (true open-loop)",
			stats.MaxInFlight, wantInFlight)
	}
	// Observer off: inner metrics should be zero.
	if snap.InnerMetrics.Dequeues != 0 {
		t.Errorf("inner metrics: Dequeues = %d, want 0 with observer disabled", snap.InnerMetrics.Dequeues)
	}
}

// TestDriverBoundedConcurrency drives the full bounded-concurrency path: the
// driver Config is derived from the app config via FromAppConfig (so the
// active-working-set chain gate and the Dynamic dispatch sizing are wired from
// [concurrency]/[engine]), and seeds are applied on the sync path. It asserts
// the oracle still agrees end to end, submission stays open-loop, and a healthy
// run reports ZERO backpressure (the dispatch held the offered active set).
func TestDriverBoundedConcurrency(t *testing.T) {
	cfg := testConfig(0xC0FFEE, 30000, 0.05)
	stream, err := generator.Generate(cfg)
	if err != nil {
		t.Fatalf("Generate() error = %v", err)
	}
	if stream.Stats.Seeds != cfg.Accounts.Count {
		t.Fatalf("expected %d seeds (one per account), got %d", cfg.Accounts.Count, stream.Stats.Seeds)
	}

	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()

	// FromAppConfig wires ActiveAccounts (chain gate), MaxQueues and IdleCleanup.
	dcfg := FromAppConfig(cfg)
	dcfg.Collectors = 16
	dcfg.OverheadProbes = 0

	stats, _, err := Run(ctx, stream, dcfg)
	// A healthy bounded run must NOT be flagged invalid for ANY reason: no
	// backpressure and a non-zero anti-DCE checksum. A nil error proves none of the
	// invalid sentinels fired (the precedence ladder in Run returns the first
	// applicable one). A handoff stall is NOT an invalidity trigger.
	if err != nil {
		t.Fatalf("Run() error = %v, want nil (healthy bounded run must not be flagged invalid)", err)
	}
	if stats.Backpressure != 0 {
		t.Fatalf("healthy bounded run reported backpressure = %d, want 0", stats.Backpressure)
	}
	// Handoff stalls are a DIAGNOSTIC only: the collector->finalizer handoff is
	// non-blocking (spills to an off-path overflow) and CommitAndClose is off the
	// measured path, so a non-zero count never throttles the submit schedule nor
	// invalidates the run. Logged for visibility, not asserted.
	t.Logf("handoff stalls = %d (diagnostic only; off-path, does not invalidate the run)",
		stats.HandoffStalls)
	// Non-empty run => the anti-DCE checksum must be non-zero (else Run would have
	// returned ErrZeroChecksumInvalidRun).
	if stats.Checksum == 0 {
		t.Fatal("healthy bounded run has a zero anti-DCE checksum on a non-empty run")
	}
	// Bounded concurrency must still pipeline deeply (true open-loop): the peak
	// in-flight reflects the virtual schedule's overlap, well above the active set.
	wantInFlight := int64(cfg.Concurrency.ActiveAccounts) //nolint:gosec // active accounts is a small configured bound
	if stats.MaxInFlight <= wantInFlight {
		t.Fatalf("max in-flight = %d, want > active accounts %d (bounded concurrency must still pipeline)",
			stats.MaxInFlight, wantInFlight)
	}
	// Seeds were applied on the sync path, so the async stream consumed only
	// order-checks + runtime top-ups (fundings minus seeds).
	wantOrderEvents := stream.Stats.OrderChecks + (stream.Stats.Fundings - stream.Stats.Seeds)
	if stats.OrderChecks != wantOrderEvents {
		t.Errorf("consumed order-class ops = %d, want %d (order-checks %d + top-ups %d)",
			stats.OrderChecks, wantOrderEvents, stream.Stats.OrderChecks,
			stream.Stats.Fundings-stream.Stats.Seeds)
	}
}

// TestDriverPacedSubmission exercises the PACED offered-rate path: the generator
// stamps virtual arrivals from a positive offered rate, and the driver paces each
// event to its virtual arrival open-loop. Under a finite offered rate the
// in-flight depth follows Little's law (rate * latency) and so is modest, but
// submission still overlaps decisions (open-loop), which the witness asserts.
func TestDriverPacedSubmission(t *testing.T) {
	cfg := testConfig(0x5EED, 8000, 0.05)
	// Pace the virtual timeline from a positive offered rate (the generator owns
	// pacing now; the driver follows the per-event virtual arrivals). Inter-arrival
	// spacing is always exponential (Poisson).
	cfg.Arrival = config.Arrival{OfferedRate: 100000}
	stream, err := generator.Generate(cfg)
	if err != nil {
		t.Fatalf("Generate() error = %v", err)
	}

	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()

	stats, snap, err := Run(ctx, stream, Config{
		Observer:   false,
		Collectors: 16,
		WindowSize: 1000,
	})
	if err != nil {
		t.Fatalf("Run() oracle/invariant error = %v", err)
	}
	if stats.SampleCount == 0 {
		t.Fatal("no samples recorded under paced submission")
	}
	// Open-loop overlap: even paced, the submitter never blocks on a decision, so
	// in-flight exceeds 1.
	if stats.MaxInFlight < 2 {
		t.Fatalf("max in-flight = %d, want > 1 under paced open-loop submission", stats.MaxInFlight)
	}
	if snap.OrderCheck.Count == 0 {
		t.Fatal("paced: merged order-check histogram is empty")
	}
}

// TestDriverShardedStrategy exercises the sharded dispatch path end-to-end.
// It builds and runs a short stream under strategy = sharded (3 workers) and
// asserts:
//   - the per-op oracle still agrees with the engine (sharded preserves per-account
//     FIFO within each shard, which is sufficient when one account always routes to
//     the same shard);
//   - aggregate invariants hold;
//   - submission stayed open-loop (max in-flight > 1);
//   - backpressure was zero (sharded has no queue cap so ErrQueueLimit never fires);
//   - measurement windows are populated.
//
// This is the primary wiring verification for the sharded path.
func TestDriverShardedStrategy(t *testing.T) {
	cfg := testConfig(0x5ADED, 15000, 0.05)
	// Override the async engine strategy to sharded with 3 workers.
	cfg.AsyncEngine = config.AsyncEngine{
		Strategy:       config.AsyncEngineStrategySharded,
		ShardedWorkers: 3,
	}
	stream, err := generator.Generate(cfg)
	if err != nil {
		t.Fatalf("Generate() error = %v", err)
	}
	if stream.Stats.Accepts == 0 || stream.Stats.Rejects == 0 {
		t.Fatalf("stream lacks a mix: accepts=%d rejects=%d", stream.Stats.Accepts, stream.Stats.Rejects)
	}

	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()

	dcfg := FromAppConfig(cfg)
	dcfg.Collectors = 16
	dcfg.OverheadProbes = 0

	stats, snap, err := Run(ctx, stream, dcfg)
	if err != nil {
		t.Fatalf("Run() oracle/invariant error (sharded) = %v", err)
	}

	// The oracle must agree over the whole stream.
	wantOrderEvents := stream.Stats.OrderChecks + (stream.Stats.Fundings - stream.Stats.Seeds)
	if stats.OrderChecks != wantOrderEvents {
		t.Errorf("sharded: consumed order-class ops = %d, want %d", stats.OrderChecks, wantOrderEvents)
	}
	if stats.Settlements != stream.Stats.Settlements {
		t.Errorf("sharded: consumed settlements = %d, want %d", stats.Settlements, stream.Stats.Settlements)
	}

	// Sharded has no queue cap, so backpressure must be zero.
	if stats.Backpressure != 0 {
		t.Fatalf("sharded: backpressure = %d, want 0", stats.Backpressure)
	}

	// True open-loop: in-flight must exceed 1.
	if stats.MaxInFlight < 2 {
		t.Fatalf("sharded: max in-flight = %d, want > 1 (true open-loop)", stats.MaxInFlight)
	}

	// Measurement windows must be populated.
	if len(snap.Windows) == 0 {
		t.Fatal("sharded: snapshot has no windows")
	}
	if snap.OrderCheck.Count == 0 {
		t.Fatal("sharded: merged order-check histogram is empty")
	}

	t.Logf("sharded(3 workers): ops=%d accepts=%d rejects=%d maxInFlight=%d windows=%d p99=%v",
		stats.OrderChecks, stats.Accepts, stats.Rejects, stats.MaxInFlight,
		len(snap.Windows), snap.OrderCheck.P99)
}
