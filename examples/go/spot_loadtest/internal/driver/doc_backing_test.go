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

// Doc-backing test for the README recipe.
//
// Mirror of: examples/go/spot_loadtest/README.md
// (the "Build and run" section and the report structure walkthrough).
//
// This test loads configs/baseline.ini through the real config parser (guarding
// that the committed reference config stays in sync with the tightened validator),
// then runs a reduced end-to-end — 30 000 order-checks instead of the 2 000 000
// in the baseline — through the REAL openpit asyncengine, and finally renders the
// report, asserting every named block is present and the run is oracle-clean with
// zero backpressure.
//
// The test requires the native core:
//
//	OPENPIT_RUNTIME_LIBRARY_PATH=$(pwd)/target/release/libopenpit_ffi.dylib \
//	  go test -race ./internal/...

import (
	"bytes"
	"context"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"openpit-loadtest-spot-funds-go/internal/config"
	"openpit-loadtest-spot-funds-go/internal/env"
	"openpit-loadtest-spot-funds-go/internal/generator"
	"openpit-loadtest-spot-funds-go/internal/reporter"
)

// TestDocBackingBaselineRecipe is the doc-backing integration test for the
// README "Build and run" recipe (examples/go/spot_loadtest/README.md).
//
// It guards two invariants:
//  1. configs/baseline.ini parses and passes strict validation without error —
//     the committed reference config must always stay in sync with the parser.
//  2. A reduced run (30 000 ops, same seed + cohort structure as the baseline)
//     through the REAL openpit asyncengine produces an oracle-clean report that
//     contains every named block, reports zero backpressure, and has a non-zero
//     anti-DCE checksum.
func TestDocBackingBaselineRecipe(t *testing.T) {
	// --- Step 1: load and validate configs/baseline.ini ---
	//
	// Path is relative from the driver package to the configs dir at the
	// example root (two levels up from internal/driver/).
	baselinePath := filepath.Join("..", "..", "configs", "baseline.ini")
	baseCfg, err := config.Load(baselinePath)
	if err != nil {
		t.Fatalf("configs/baseline.ini failed strict validation: %v", err)
	}
	if len(baseCfg.Cohorts) == 0 {
		t.Fatal("configs/baseline.ini: no cohorts parsed")
	}
	if len(baseCfg.Instruments.Symbols) == 0 {
		t.Fatal("configs/baseline.ini: no instrument symbols parsed")
	}
	if baseCfg.Instruments.Settlement == "" {
		t.Fatal("configs/baseline.ini: settlement asset is empty")
	}
	if baseCfg.Accounts.Count == 0 {
		t.Fatal("configs/baseline.ini: accounts.count is zero")
	}
	if baseCfg.Concurrency.ActiveAccounts == 0 {
		t.Fatal("configs/baseline.ini: concurrency.active_accounts is zero")
	}

	// --- Step 2: build a reduced config from the baseline ---
	//
	// Keep the same seed and cohort structure as configs/baseline.ini so the
	// generator exercises the same code paths. Shrink total_ops so the test
	// completes in seconds rather than ~40s.
	reducedCfg := *baseCfg
	reducedCfg.Run.TotalOps = 30_000
	reducedCfg.Run.Window = 5_000

	// Use a small but representative population so the bounded-concurrency
	// path exercises the chain gate without needing all 10 000 accounts.
	// max_queues=0 (unlimited) matches the baseline intent: within a short run
	// the idle-cleanup scan does not fire, so capping queues would cause false
	// backpressure as the submitter rotates through distinct accounts.
	reducedCfg.Accounts.Count = 500
	reducedCfg.Concurrency.ActiveAccounts = 64
	reducedCfg.AsyncEngine.MaxQueues = 0
	reducedCfg.AsyncEngine.IdleCleanup = 2 * time.Second

	// --- Step 3: generate the event stream ---
	stream, err := generator.Generate(&reducedCfg)
	if err != nil {
		t.Fatalf("generator.Generate() error = %v", err)
	}
	if stream.Stats.OrderChecks == 0 {
		t.Fatal("generated stream has no order-checks")
	}

	// --- Step 4: run the driver through the REAL engine ---
	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()

	dCfg := FromAppConfig(&reducedCfg)
	dCfg.Collectors = 16
	dCfg.OverheadProbes = 50

	stats, snap, err := Run(ctx, stream, dCfg)
	if err != nil {
		t.Fatalf("driver.Run() oracle/invariant error: %v", err)
	}

	// --- Step 5: assert the oracle-clean / healthy-run invariants ---

	// Oracle agrees: if Run returns nil, every per-op oracle check passed.
	// Verify the counts are non-trivial.
	if stats.OrderChecks == 0 {
		t.Fatal("no order-checks resolved")
	}
	if stats.Accepts == 0 {
		t.Fatal("no accepts recorded (expected a mix of accepts and rejects)")
	}
	if stats.Rejects == 0 {
		t.Fatal("no rejects recorded (target reject rate > 0 but got none)")
	}

	// A healthy baseline run with a supported config must never trigger
	// dispatch-capacity backpressure (ErrQueueLimit).
	if snap.Backpressure != 0 {
		t.Fatalf("backpressure = %d, want 0 (a healthy baseline run must be backpressure-free)",
			snap.Backpressure)
	}

	// Anti-DCE proof: the checksum must be non-zero.
	if snap.Checksum == 0 {
		t.Error("anti-DCE checksum is zero — not all decisions were consumed")
	}

	// Open-loop witness: the submitter never blocks on a decision, so submissions
	// overlap decisions and the peak in-flight exceeds 1. (This reduced run is
	// paced below saturation at the baseline offered rate, so the depth follows
	// Little's law and is modest; the unpaced driver tests assert the deeper
	// pipelining that saturation produces.)
	if snap.MaxInFlight < 2 {
		t.Fatalf("max in-flight = %d, want > 1 (submission was not open-loop)", snap.MaxInFlight)
	}

	// Measurement windows must exist and carry real data.
	if len(snap.Windows) == 0 {
		t.Fatal("snapshot has no measurement windows")
	}
	if snap.OrderCheck.Count == 0 {
		t.Fatal("merged order-check histogram is empty")
	}
	if snap.OrderCheck.P99 <= 0 {
		t.Errorf("order-check p99 = %v, want > 0", snap.OrderCheck.P99)
	}

	// --- Step 6: render the report and assert all named blocks are present ---
	//
	// Use a synthetic env so the test is hermetic (no real gopsutil calls that
	// depend on the host). The env block content is not under test here; the
	// structure is.
	e := syntheticDocEnv()
	var buf bytes.Buffer
	reporter.Write(&buf, e, &reducedCfg, "configs/baseline.ini", snap, stream.Stats)
	out := buf.String()

	// Every named section header that the README documents must appear.
	requiredBlocks := []string{
		"=== Headline:",
		"=== Environment ===",
		"=== Workload ===",
		"=== Trajectory",
		"=== Distribution",
		"=== Diagnostics",
		"=== Disclaimer ===",
	}
	for _, block := range requiredBlocks {
		if !strings.Contains(out, block) {
			t.Errorf("report missing expected block header: %q", block)
		}
	}

	// Backpressure == 0 must be rendered as "healthy".
	if !strings.Contains(out, "0 (healthy") {
		t.Error("report must label backpressure=0 as healthy")
	}

	// The headline must be the OPEN-LOOP latency-under-load, and the service-time
	// figure must appear ONLY as a labelled diagnostic, never as the headline.
	if !strings.Contains(out, "Open-Loop Order-Check Latency") {
		t.Error("headline must be the open-loop order-check latency")
	}
	if !strings.Contains(out, "Service-time") {
		t.Error("diagnostics must include the service-time figure")
	}
	if !strings.Contains(out, "DIAGNOSTIC, NOT the headline") {
		t.Error("service-time must be explicitly labelled as a diagnostic, not the headline")
	}

	// The disclaimer must state what is and is not measured.
	if !strings.Contains(out, "What IS measured") {
		t.Error("disclaimer block must say what IS measured")
	}
	if !strings.Contains(out, "What is NOT measured") {
		t.Error("disclaimer block must say what is NOT measured")
	}

	// The reproduction recipe must echo the config flag.
	if !strings.Contains(out, "configs/baseline.ini") {
		t.Error("disclaimer must include the config flag in the reproduction recipe")
	}

	// The anti-DCE checksum must be printed.
	if !strings.Contains(out, "Anti-DCE checksum") {
		t.Error("report must print the anti-DCE checksum")
	}

	t.Logf("doc-backing test passed: ops=%d accepts=%d rejects=%d maxInFlight=%d backpressure=%d checksum=%#x",
		stats.OrderChecks, stats.Accepts, stats.Rejects, stats.MaxInFlight, stats.Backpressure, stats.Checksum)
	t.Logf("windows=%d orderCheck.p50=%v orderCheck.p99=%v settlement.p50=%v",
		len(snap.Windows), snap.OrderCheck.P50, snap.OrderCheck.P99, snap.Settlement.P50)
}

// syntheticDocEnv returns a hermetic env.Env for the doc-backing test so the
// report render does not depend on gopsutil or git access.
func syntheticDocEnv() env.Env {
	return env.Env{
		Host: env.Host{
			CPUModel: "doc-backing-test (synthetic)",
			Cores:    8,
			RAM:      "16.0 GiB",
			OS:       "TestOS",
			Kernel:   "TestKernel",
		},
		Runtime: env.GoRuntime{
			Version:    "go1.26.0",
			GOOS:       "linux",
			GOARCH:     "amd64",
			CGOEnabled: true,
		},
		Pit: env.PitRepo{
			Commit: "doc-backing-test",
			Dirty:  false,
		},
		Core: env.CoreBuildProfile{
			Version:         "0.3.0",
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

// TestBaselineIniParsesAndValidates is the unit layer of the doc-backing test:
// it asserts configs/baseline.ini passes strict validation in isolation, without
// requiring the native core. This catches config-drift early even in environments
// where OPENPIT_RUNTIME_LIBRARY_PATH is not set.
//
// Mirror of: examples/go/spot_loadtest/README.md (configuration section).
func TestBaselineIniParsesAndValidates(t *testing.T) {
	t.Parallel()

	baselinePath := filepath.Join("..", "..", "configs", "baseline.ini")
	cfg, err := config.Load(baselinePath)
	if err != nil {
		t.Fatalf("configs/baseline.ini failed strict validation: %v", err)
	}

	// Spot-check expected baseline values so a committed config mutation is caught.
	if cfg.Run.Seed != 0xC0FFEE {
		t.Errorf("Run.Seed = %#x, want 0xC0FFEE", cfg.Run.Seed)
	}
	if cfg.Run.TotalOps != 2_000_000 {
		t.Errorf("Run.TotalOps = %d, want 2000000", cfg.Run.TotalOps)
	}
	if cfg.Run.Window != 100_000 {
		t.Errorf("Run.Window = %d, want 100000", cfg.Run.Window)
	}
	if cfg.Accounts.Count != 10_000 {
		t.Errorf("Accounts.Count = %d, want 10000", cfg.Accounts.Count)
	}
	if cfg.Concurrency.ActiveAccounts != 1024 {
		t.Errorf("Concurrency.ActiveAccounts = %d, want 1024", cfg.Concurrency.ActiveAccounts)
	}
	if cfg.AsyncEngine.Strategy != config.AsyncEngineStrategyDynamic {
		t.Errorf("AsyncEngine.Strategy = %q, want dynamic", cfg.AsyncEngine.Strategy)
	}
	if cfg.AsyncEngine.MaxQueues != 0 {
		t.Errorf("AsyncEngine.MaxQueues = %d, want 0 (unlimited)", cfg.AsyncEngine.MaxQueues)
	}
	if cfg.AsyncEngine.IdleCleanup != 5*time.Second {
		t.Errorf("AsyncEngine.IdleCleanup = %s, want 5s", cfg.AsyncEngine.IdleCleanup)
	}
	if cfg.AsyncEngine.ShardedWorkers != 0 {
		t.Errorf("AsyncEngine.ShardedWorkers = %d, want 0 (baseline: dynamic)", cfg.AsyncEngine.ShardedWorkers)
	}
	if cfg.AsyncEngine.QueueCapacity != 0 {
		t.Errorf("AsyncEngine.QueueCapacity = %d, want 0 (engine default)", cfg.AsyncEngine.QueueCapacity)
	}
	if cfg.AsyncEngine.SlowSubmitThreshold != 0 {
		t.Errorf("AsyncEngine.SlowSubmitThreshold = %s, want 0 (engine default)", cfg.AsyncEngine.SlowSubmitThreshold)
	}
	if cfg.Reject.TargetRate != 0.05 {
		t.Errorf("Reject.TargetRate = %v, want 0.05", cfg.Reject.TargetRate)
	}
	if len(cfg.Cohorts) != 3 {
		t.Errorf("len(Cohorts) = %d, want 3 (chatty, dormant, steady)", len(cfg.Cohorts))
	}
	if len(cfg.Instruments.Symbols) != 10 {
		t.Errorf("len(Instruments.Symbols) = %d, want 10", len(cfg.Instruments.Symbols))
	}
	if cfg.Instruments.Settlement != "USD" {
		t.Errorf("Instruments.Settlement = %q, want USD", cfg.Instruments.Settlement)
	}
	if cfg.Run.Observer != true {
		t.Error("Run.Observer = false, want true (observer is on in the baseline)")
	}
	// Hash must be non-empty (proves the file was read and hashed).
	if cfg.Hash == "" {
		t.Error("Config.Hash is empty")
	}
}

// TestBaselineWindowUnit asserts the baseline uses ops-based windowing.
func TestBaselineWindowUnit(t *testing.T) {
	t.Parallel()

	baselinePath := filepath.Join("..", "..", "configs", "baseline.ini")
	cfg, err := config.Load(baselinePath)
	if err != nil {
		t.Fatalf("configs/baseline.ini failed validation: %v", err)
	}
	if cfg.Run.WindowUnit != config.WindowUnitOps {
		t.Errorf("Run.WindowUnit = %q, want %q", cfg.Run.WindowUnit, config.WindowUnitOps)
	}
}

// TestBaselineLifecycleProbabilities asserts the baseline transition
// probabilities sum to a value consistent with the generator's expectations
// (each is an independent probability in [0,1]; the generator uses them
// independently, not as a simplex).
func TestBaselineLifecycleProbabilities(t *testing.T) {
	t.Parallel()

	baselinePath := filepath.Join("..", "..", "configs", "baseline.ini")
	cfg, err := config.Load(baselinePath)
	if err != nil {
		t.Fatalf("configs/baseline.ini failed validation: %v", err)
	}
	for name, p := range map[string]float64{
		"p_open":          cfg.Lifecycle.POpen,
		"p_add":           cfg.Lifecycle.PAdd,
		"p_partial_close": cfg.Lifecycle.PPartialClose,
		"p_full_close":    cfg.Lifecycle.PFullClose,
	} {
		if p < 0 || p > 1 {
			t.Errorf("baseline lifecycle %s = %v, must be in [0, 1]", name, p)
		}
	}
}

// TestBaselineCohortWeightsPositive asserts all cohort weights are positive.
func TestBaselineCohortWeightsPositive(t *testing.T) {
	t.Parallel()

	baselinePath := filepath.Join("..", "..", "configs", "baseline.ini")
	cfg, err := config.Load(baselinePath)
	if err != nil {
		t.Fatalf("configs/baseline.ini failed validation: %v", err)
	}
	for _, c := range cfg.Cohorts {
		if c.Weight <= 0 {
			t.Errorf("cohort %q: weight = %v, must be > 0", c.Name, c.Weight)
		}
	}
}

// TestBaselineFundingConsistent asserts the baseline seed balance is strictly
// greater than the top-up threshold, so accounts start well-funded and the
// generator does not immediately trigger a top-up on the very first check.
func TestBaselineFundingConsistent(t *testing.T) {
	t.Parallel()

	baselinePath := filepath.Join("..", "..", "configs", "baseline.ini")
	cfg, err := config.Load(baselinePath)
	if err != nil {
		t.Fatalf("configs/baseline.ini failed validation: %v", err)
	}
	// Seed (starting balance) must exceed the trigger threshold so accounts do
	// not immediately fire a top-up before any orders are placed.
	if cfg.Funding.Seed.LessThanOrEqual(cfg.Funding.Threshold) {
		t.Errorf("funding.seed (%s) must be > funding.amount threshold (%s)",
			cfg.Funding.Seed, cfg.Funding.Threshold)
	}
	if !cfg.Funding.TopUp.IsPositive() {
		t.Errorf("funding.top_up = %s, must be > 0", cfg.Funding.TopUp)
	}
}
