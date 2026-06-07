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

package reporter_test

import (
	"bytes"
	"strings"
	"testing"
	"time"

	"openpit-loadtest-spot-funds-go/internal/config"
	"openpit-loadtest-spot-funds-go/internal/env"
	"openpit-loadtest-spot-funds-go/internal/generator"
	"openpit-loadtest-spot-funds-go/internal/measurement"
	"openpit-loadtest-spot-funds-go/internal/reporter"
)

// syntheticSnapshot builds a non-trivial Snapshot for report rendering tests.
func syntheticSnapshot() measurement.Snapshot {
	windows := []measurement.WindowSnapshot{
		{
			Index: 0,
			OrderCheck: measurement.Percentiles{
				P50: 100 * time.Microsecond, P90: 200 * time.Microsecond,
				P99: 500 * time.Microsecond, P999: 1 * time.Millisecond,
				Max: 5 * time.Millisecond, Count: 100000,
			},
			Settlement: measurement.Percentiles{
				P50: 50 * time.Microsecond, P90: 100 * time.Microsecond,
				P99: 300 * time.Microsecond, P999: 500 * time.Microsecond,
				Max: 2 * time.Millisecond, Count: 50000,
			},
			WallStart: time.Now().Add(-10 * time.Second),
			WallEnd:   time.Now().Add(-5 * time.Second),
		},
		{
			Index: 1,
			OrderCheck: measurement.Percentiles{
				P50: 95 * time.Microsecond, P90: 180 * time.Microsecond,
				P99: 450 * time.Microsecond, P999: 900 * time.Microsecond,
				Max: 4 * time.Millisecond, Count: 100000,
			},
			Settlement: measurement.Percentiles{
				P50: 48 * time.Microsecond, P90: 95 * time.Microsecond,
				P99: 280 * time.Microsecond, P999: 480 * time.Microsecond,
				Max: 1800 * time.Microsecond, Count: 50000,
			},
			WallStart: time.Now().Add(-5 * time.Second),
			WallEnd:   time.Now(),
		},
	}
	// SteadyStateOrderCheck / SteadyStateSettlement mirror window[1] (warmup=1
	// for a 2-window snapshot). In production these come from Build via a
	// lossless HdrHistogram merge; here we supply representative values.
	return measurement.Snapshot{
		Windows: windows,
		OrderCheck: measurement.Percentiles{
			P50: 97 * time.Microsecond, P90: 190 * time.Microsecond,
			P99: 475 * time.Microsecond, P999: 950 * time.Microsecond,
			Max: 5 * time.Millisecond, Count: 200000,
		},
		Settlement: measurement.Percentiles{
			P50: 49 * time.Microsecond, P90: 98 * time.Microsecond,
			P99: 290 * time.Microsecond, P999: 490 * time.Microsecond,
			Max: 2 * time.Millisecond, Count: 100000,
		},
		SteadyStateOrderCheck: measurement.Percentiles{
			P50: 95 * time.Microsecond, P90: 180 * time.Microsecond,
			P99: 450 * time.Microsecond, P999: 900 * time.Microsecond,
			Max: 4 * time.Millisecond, Count: 100000,
		},
		SteadyStateSettlement: measurement.Percentiles{
			P50: 48 * time.Microsecond, P90: 95 * time.Microsecond,
			P99: 280 * time.Microsecond, P999: 480 * time.Microsecond,
			Max: 1800 * time.Microsecond, Count: 50000,
		},
		// Service-time is the diagnostic counterpart to the open-loop headline; in
		// a saturated run it is much lower than the open-loop tail (it discounts
		// pre-submit queue wait). These representative values exercise the render.
		ServiceTime: measurement.Percentiles{
			P50: 30 * time.Microsecond, P90: 45 * time.Microsecond,
			P99: 70 * time.Microsecond, P999: 120 * time.Microsecond,
			Max: 300 * time.Microsecond, Count: 200000,
		},
		WarmupWindows:      1,
		Throughput:         45000,
		TotalOrderChecks:   200000,
		TotalSettlements:   100000,
		TotalAccepts:       190000,
		TotalRejects:       10000,
		AchievedRejectRate: 0.050,
		MaxInFlight:        128,
		Checksum:           0xDEADBEEFCAFEBABE,
		Overhead: measurement.OverheadSummary{
			Probes: 200,
			Distribution: measurement.Percentiles{
				P50: 20 * time.Microsecond, P90: 35 * time.Microsecond,
				P99: 50 * time.Microsecond, P999: 80 * time.Microsecond,
				Max: 200 * time.Microsecond, Count: 200,
			},
		},
		InnerMetrics: measurement.InnerMetrics{
			QueueWait: measurement.Percentiles{
				P50: 10 * time.Microsecond, P99: 80 * time.Microsecond,
				P999: 200 * time.Microsecond, Max: 1 * time.Millisecond, Count: 200000,
			},
			EngineCompute: measurement.Percentiles{
				P50: 5 * time.Microsecond, P99: 20 * time.Microsecond,
				P999: 50 * time.Microsecond, Max: 200 * time.Microsecond, Count: 200000,
			},
			QueuesCreated: 10000,
			QueuesRemoved: 9950,
			Dequeues:      200000,
			Completes:     200000,
		},
		WallStart: time.Now().Add(-10 * time.Second),
		WallEnd:   time.Now(),
	}
}

// syntheticConfig builds a minimal valid Config for rendering.
func syntheticConfig() *config.Config {
	return &config.Config{
		Path: "/tmp/test.ini",
		Hash: "abc123",
		Run: config.Run{
			Seed:       0xC0FFEE,
			TotalOps:   200000,
			Window:     100000,
			WindowUnit: config.WindowUnitOps,
			Observer:   true,
		},
		Reject: config.Reject{
			TargetRate: 0.05,
			Tolerance:  0.005,
		},
		Accounts:    config.Accounts{Count: 10000},
		Concurrency: config.Concurrency{ActiveAccounts: 1024},
		AsyncEngine: config.AsyncEngine{Strategy: config.AsyncEngineStrategyDynamic, MaxQueues: 4096, IdleCleanup: 2 * time.Second},
		Cohorts: []config.Cohort{
			{Name: "chatty", Weight: 0.2, Activity: 0.9, RejectPropensity: 0.7, BurstLen: 4},
			{Name: "steady", Weight: 0.5, Activity: 0.5, RejectPropensity: 0.25, BurstLen: 2},
			{Name: "dormant", Weight: 0.3, Activity: 0.1, RejectPropensity: 0.05, BurstLen: 1},
		},
	}
}

func syntheticEnv() env.Env {
	return env.Env{
		Host: env.Host{
			CPUModel: "Test CPU 3.5GHz",
			Cores:    8,
			RAM:      "16.0 GiB",
			OS:       "TestOS 1.0",
			Kernel:   "TestKernel 5.0",
		},
		Runtime: env.GoRuntime{
			Version:    "go1.22.0",
			GOOS:       "linux",
			GOARCH:     "amd64",
			CGOEnabled: true,
		},
		Pit: env.PitRepo{
			Commit: "abc1234def",
			Dirty:  false,
		},
		Core: env.CoreBuildProfile{
			Version:         "0.1.0",
			Profile:         "release",
			OptLevel:        "3",
			DebugAssertions: false,
			Target:          "x86_64-unknown-linux-gnu",
			TargetCPU:       "native",
			LTO:             "thin",
			Raw:             "profile=release;opt_level=3;debug_assertions=false",
		},
	}
}

func syntheticStreamStats() generator.StreamStats {
	return generator.StreamStats{
		OrderChecks:    200000,
		Accepts:        190000,
		Rejects:        10000,
		Settlements:    190000,
		Fundings:       20000,
		ForcedRejects:  9000,
		NaturalRejects: 1000,
	}
}

// TestReportContainsAllBlockHeaders verifies that the report contains all
// expected block headers in the correct order.
func TestReportContainsAllBlockHeaders(t *testing.T) {
	var buf bytes.Buffer
	reporter.Write(&buf, syntheticEnv(), syntheticConfig(), "configs/test.ini",
		syntheticSnapshot(), syntheticStreamStats())

	out := buf.String()
	expectedHeaders := []string{
		"=== Headline:",
		"=== Environment ===",
		"=== Workload ===",
		"=== Trajectory",
		"=== Distribution",
		"=== Diagnostics",
		"=== Disclaimer ===",
	}
	for _, h := range expectedHeaders {
		if !strings.Contains(out, h) {
			t.Errorf("report missing block header %q", h)
		}
	}

	// Headers must appear in order.
	prev := 0
	for _, h := range expectedHeaders {
		idx := strings.Index(out, h)
		if idx < prev {
			t.Errorf("block header %q is out of order", h)
		}
		prev = idx
	}
}

// TestHeadlineSteadyStateLabel verifies the steady-state definition is clearly
// labeled and contains the warmup exclusion wording.
func TestHeadlineSteadyStateLabel(t *testing.T) {
	var buf bytes.Buffer
	reporter.Write(&buf, syntheticEnv(), syntheticConfig(), "configs/test.ini",
		syntheticSnapshot(), syntheticStreamStats())

	out := buf.String()
	// With 2 windows, window 1 is excluded as warmup.
	if !strings.Contains(out, "warmup") {
		t.Error("headline must mention warmup exclusion")
	}
	if !strings.Contains(out, "Steady-state definition") {
		t.Error("headline must label the steady-state definition")
	}
}

// TestHeadlineContainsTailPercentiles verifies that p99.9 and max are always
// shown in the headline (honesty guardrail: tail must be visible).
func TestHeadlineContainsTailPercentiles(t *testing.T) {
	var buf bytes.Buffer
	reporter.Write(&buf, syntheticEnv(), syntheticConfig(), "configs/test.ini",
		syntheticSnapshot(), syntheticStreamStats())

	out := buf.String()
	if !strings.Contains(out, "p99.9") {
		t.Error("headline must show p99.9 (tail must not be hidden)")
	}
	if !strings.Contains(out, "max") {
		t.Error("headline must show max (tail must not be hidden)")
	}
}

// TestHeadlineIsOpenLoop verifies the headline is labelled as the OPEN-LOOP
// latency-under-load (intended arrival -> decision), with the coordinated-
// omission-defence wording.
func TestHeadlineIsOpenLoop(t *testing.T) {
	var buf bytes.Buffer
	reporter.Write(&buf, syntheticEnv(), syntheticConfig(), "configs/test.ini",
		syntheticSnapshot(), syntheticStreamStats())

	out := buf.String()
	if !strings.Contains(out, "Open-Loop Order-Check Latency") {
		t.Error("headline must be labelled as the open-loop order-check latency")
	}
	if !strings.Contains(out, "intended arrival") {
		t.Error("headline must explain t0 is the intended arrival (not the actual submit)")
	}
	if !strings.Contains(out, "coordinated-omission") {
		t.Error("headline must mention the coordinated-omission defence")
	}
}

// TestServiceTimeIsDiagnosticOnly verifies the service-time figure is rendered
// in the diagnostics section, with the service-time percentiles, and is loudly
// labelled as NOT the headline.
func TestServiceTimeIsDiagnosticOnly(t *testing.T) {
	var buf bytes.Buffer
	reporter.Write(&buf, syntheticEnv(), syntheticConfig(), "configs/test.ini",
		syntheticSnapshot(), syntheticStreamStats())

	out := buf.String()
	diagIdx := strings.Index(out, "=== Diagnostics")
	headIdx := strings.Index(out, "=== Headline")
	stIdx := strings.Index(out, "Service-time (resolve - ACTUAL submit)")
	if stIdx < 0 {
		t.Fatal("report must include the service-time diagnostic line")
	}
	// The service-time figure must live in the diagnostics block, after the
	// headline block (never in the headline).
	if stIdx <= diagIdx || diagIdx <= headIdx {
		t.Errorf("service-time must appear in the diagnostics block (st=%d diag=%d head=%d)", stIdx, diagIdx, headIdx)
	}
	if !strings.Contains(out, "DIAGNOSTIC, NOT the headline") {
		t.Error("service-time must be loudly labelled as a diagnostic, not the headline")
	}
}

// TestChecksumInReport verifies the anti-DCE checksum is printed.
func TestChecksumInReport(t *testing.T) {
	var buf bytes.Buffer
	snap := syntheticSnapshot()
	snap.Checksum = 0xDEADBEEFCAFEBABE
	reporter.Write(&buf, syntheticEnv(), syntheticConfig(), "configs/test.ini",
		snap, syntheticStreamStats())

	out := buf.String()
	if !strings.Contains(out, "DEADBEEFCAFEBABE") {
		t.Error("report must print the anti-DCE checksum")
	}
}

// TestRejectRateInWorkload verifies that target and achieved reject rates appear.
func TestRejectRateInWorkload(t *testing.T) {
	var buf bytes.Buffer
	reporter.Write(&buf, syntheticEnv(), syntheticConfig(), "configs/test.ini",
		syntheticSnapshot(), syntheticStreamStats())

	out := buf.String()
	if !strings.Contains(out, "Achieved reject rate") {
		t.Error("workload block must show achieved reject rate")
	}
	if !strings.Contains(out, "Target reject rate") {
		t.Error("workload block must show target reject rate")
	}
}

// TestConcurrencyDisclosure verifies the report discloses the bounded-
// concurrency model (population vs active working set), the engine dispatch
// sizing (strategy + active knobs), and the backpressure count.
func TestConcurrencyDisclosure(t *testing.T) {
	var buf bytes.Buffer
	reporter.Write(&buf, syntheticEnv(), syntheticConfig(), "configs/test.ini",
		syntheticSnapshot(), syntheticStreamStats())

	out := buf.String()
	for _, want := range []string{
		"Concurrency model",
		"population (total accounts)",
		"active working set",
		"Engine dispatch sizing",
		"strategy",
		"max_queues",
		"idle_cleanup",
		"queue_capacity",
		"slow_submit_threshold",
		"Backpressure",
	} {
		if !strings.Contains(out, want) {
			t.Errorf("report missing concurrency disclosure %q", want)
		}
	}
	// A healthy synthetic snapshot (Backpressure == 0) must say so.
	if !strings.Contains(out, "0 (healthy") {
		t.Error("report must show backpressure = 0 as healthy when none occurred")
	}
}

// TestBackpressureDisclosed verifies a nonzero backpressure count is surfaced
// loudly (never hidden) when the dispatch capacity was exceeded.
func TestBackpressureDisclosed(t *testing.T) {
	var buf bytes.Buffer
	snap := syntheticSnapshot()
	snap.Backpressure = 4242
	reporter.Write(&buf, syntheticEnv(), syntheticConfig(), "configs/test.ini",
		snap, syntheticStreamStats())

	out := buf.String()
	if !strings.Contains(out, "4242") {
		t.Error("report must print a nonzero backpressure count")
	}
	if !strings.Contains(out, "degraded") {
		t.Error("report must flag a backpressured run as degraded")
	}
}

// TestObserverDisabledMessage verifies the diagnostics block correctly reports
// when the observer is off.
func TestObserverDisabledMessage(t *testing.T) {
	var buf bytes.Buffer
	cfg := syntheticConfig()
	cfg.Run.Observer = false

	reporter.Write(&buf, syntheticEnv(), cfg, "configs/test.ini",
		syntheticSnapshot(), syntheticStreamStats())

	out := buf.String()
	if !strings.Contains(out, "Observer disabled") {
		t.Error("diagnostics block must say observer is disabled when observer=off")
	}
}

// TestTrajectoryWindowsPresent verifies per-window rows appear in the trajectory.
func TestTrajectoryWindowsPresent(t *testing.T) {
	var buf bytes.Buffer
	reporter.Write(&buf, syntheticEnv(), syntheticConfig(), "configs/test.ini",
		syntheticSnapshot(), syntheticStreamStats())

	out := buf.String()
	// Should have trajectory rows for windows 1 and 2.
	if !strings.Contains(out, "   1") && !strings.Contains(out, "1w") {
		t.Error("trajectory block must show window 1 row")
	}
}

// TestReproductionRecipe verifies the config flag appears in the reproduction recipe.
func TestReproductionRecipe(t *testing.T) {
	var buf bytes.Buffer
	reporter.Write(&buf, syntheticEnv(), syntheticConfig(), "configs/baseline.ini",
		syntheticSnapshot(), syntheticStreamStats())

	out := buf.String()
	if !strings.Contains(out, "configs/baseline.ini") {
		t.Error("disclaimer must include the config flag in the reproduction recipe")
	}
}

// TestDisclaimerPresent verifies key disclaimer language is included.
func TestDisclaimerPresent(t *testing.T) {
	var buf bytes.Buffer
	reporter.Write(&buf, syntheticEnv(), syntheticConfig(), "configs/test.ini",
		syntheticSnapshot(), syntheticStreamStats())

	out := buf.String()
	if !strings.Contains(out, "What IS measured") {
		t.Error("disclaimer must say what IS measured")
	}
	if !strings.Contains(out, "What is NOT measured") {
		t.Error("disclaimer must say what is NOT measured")
	}
}

// TestSingleWindowNoWarmupExclusion verifies that with a single window the
// report does not claim to exclude warmup (nothing to exclude).
func TestSingleWindowNoWarmupExclusion(t *testing.T) {
	var buf bytes.Buffer
	snap := syntheticSnapshot()
	snap.Windows = snap.Windows[:1] // only one window
	snap.WarmupWindows = 0          // no warmup when single window

	reporter.Write(&buf, syntheticEnv(), syntheticConfig(), "configs/test.ini",
		snap, syntheticStreamStats())

	out := buf.String()
	if !strings.Contains(out, "single window") {
		t.Error("with one window the report must note no warmup exclusion is possible")
	}
}

// TestDistributionOverheadBlock verifies overhead summary appears when probes > 0.
func TestDistributionOverheadBlock(t *testing.T) {
	var buf bytes.Buffer
	reporter.Write(&buf, syntheticEnv(), syntheticConfig(), "configs/test.ini",
		syntheticSnapshot(), syntheticStreamStats())

	out := buf.String()
	if !strings.Contains(out, "self-overhead") {
		t.Error("distribution block must contain overhead section")
	}
}
