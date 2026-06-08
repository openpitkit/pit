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

package config

import (
	"os"
	"path/filepath"
	"testing"
	"time"
)

// writeINI writes content to a temp .ini file and returns its path.
func writeINI(t *testing.T, content string) string {
	t.Helper()
	dir := t.TempDir()
	path := filepath.Join(dir, "test.ini")
	if err := os.WriteFile(path, []byte(content), 0o600); err != nil {
		t.Fatalf("write temp ini: %v", err)
	}
	return path
}

// fullConfig is a minimal but complete, valid config covering every section the
// generator consumes plus the required [run] block.
const fullConfig = `
[run]
seed = 0xC0FFEE
total_ops = 1000
window = 100
window_unit = ops
observer = on

[arrival]
offered_rate = 50000
distribution = poisson

[reject]
target_rate = 0.05
tolerance = 0.005

[accounts]
count = 100

[concurrency]
active_accounts = 32

[async_engine]
strategy              = dynamic
max_queues            = 128
idle_cleanup          = 2s
sharded_workers       = 0
queue_capacity        = 0
slow_submit_threshold = 0

[instruments]
symbols = AAPL,SPX,MSFT
settlement = USD

[lifecycle]
p_open = 0.40
p_add = 0.15
p_partial_close = 0.25
p_full_close = 0.20

[funding]
trigger = balance_below
amount = 100000
seed = 1000000
top_up = 1000000

[cohort.chatty]
weight = 0.3
activity = 0.9
reject_propensity = 0.7
burst_len = 4
size_weights = 1:1,10:4,100:2
symbol_skew = zipf
zipf_s = 1.3

[cohort.steady]
weight = 0.7
activity = 0.5
reject_propensity = 0.25
burst_len = 2
size_weights = 1:2,10:3
symbol_skew = uniform
`

func TestLoadFullConfigValid(t *testing.T) {
	t.Parallel()

	cfg, err := Load(writeINI(t, fullConfig))
	if err != nil {
		t.Fatalf("Load() error = %v", err)
	}
	if cfg.Accounts.Count != 100 {
		t.Errorf("Accounts.Count = %d, want 100", cfg.Accounts.Count)
	}
	if cfg.Instruments.Settlement != "USD" {
		t.Errorf("Settlement = %q, want USD", cfg.Instruments.Settlement)
	}
	if got := len(cfg.Cohorts); got != 2 {
		t.Fatalf("len(Cohorts) = %d, want 2", got)
	}
	// Cohorts are sorted by name: chatty < steady.
	if cfg.Cohorts[0].Name != "chatty" || cfg.Cohorts[1].Name != "steady" {
		t.Errorf("cohort order = %q,%q, want chatty,steady", cfg.Cohorts[0].Name, cfg.Cohorts[1].Name)
	}
	if cfg.Cohorts[0].SymbolSkew != SymbolSkewZipf || cfg.Cohorts[0].ZipfS != 1.3 {
		t.Errorf("chatty skew = %q zipf_s = %v", cfg.Cohorts[0].SymbolSkew, cfg.Cohorts[0].ZipfS)
	}
	if len(cfg.Cohorts[0].SizeWeights) != 3 {
		t.Errorf("chatty size buckets = %d, want 3", len(cfg.Cohorts[0].SizeWeights))
	}
	if !cfg.Funding.Seed.Equal(cfg.Funding.TopUp) || cfg.Funding.Seed.String() != "1000000" {
		t.Errorf("funding seed/top_up = %s/%s", cfg.Funding.Seed, cfg.Funding.TopUp)
	}
	if cfg.Concurrency.ActiveAccounts != 32 {
		t.Errorf("Concurrency.ActiveAccounts = %d, want 32", cfg.Concurrency.ActiveAccounts)
	}
	if cfg.AsyncEngine.Strategy != AsyncEngineStrategyDynamic {
		t.Errorf("AsyncEngine.Strategy = %q, want dynamic", cfg.AsyncEngine.Strategy)
	}
	if cfg.AsyncEngine.MaxQueues != 128 {
		t.Errorf("AsyncEngine.MaxQueues = %d, want 128", cfg.AsyncEngine.MaxQueues)
	}
	if cfg.AsyncEngine.IdleCleanup != 2*time.Second {
		t.Errorf("AsyncEngine.IdleCleanup = %s, want 2s", cfg.AsyncEngine.IdleCleanup)
	}
	if cfg.AsyncEngine.ShardedWorkers != 0 {
		t.Errorf("AsyncEngine.ShardedWorkers = %d, want 0 (unused for dynamic)", cfg.AsyncEngine.ShardedWorkers)
	}
	if cfg.AsyncEngine.QueueCapacity != 0 {
		t.Errorf("AsyncEngine.QueueCapacity = %d, want 0 (engine default)", cfg.AsyncEngine.QueueCapacity)
	}
	if cfg.AsyncEngine.SlowSubmitThreshold != 0 {
		t.Errorf("AsyncEngine.SlowSubmitThreshold = %s, want 0 (engine default)", cfg.AsyncEngine.SlowSubmitThreshold)
	}
}

// TestStrictValidationRejectsBadValues drives the tightened validation: a
// present-but-invalid value (or a missing required key for a consumed feature)
// must be an explicit error. Each case mutates one field of fullConfig.
func TestStrictValidationRejectsBadValues(t *testing.T) {
	t.Parallel()

	cases := []struct {
		name    string
		ini     string
		wantErr string
	}{
		{
			name:    "reject target_rate non-numeric",
			ini:     replaceLine("target_rate = 0.05", "target_rate = high"),
			wantErr: "target_rate",
		},
		{
			name:    "reject target_rate >= 1",
			ini:     replaceLine("target_rate = 0.05", "target_rate = 1.0"),
			wantErr: "target_rate",
		},
		{
			name:    "reject tolerance zero",
			ini:     replaceLine("tolerance = 0.005", "tolerance = 0"),
			wantErr: "tolerance",
		},
		{
			name:    "accounts count zero",
			ini:     replaceLine("count = 100", "count = 0"),
			wantErr: "count",
		},
		{
			name:    "concurrency active_accounts zero",
			ini:     replaceLine("active_accounts = 32", "active_accounts = 0"),
			wantErr: "active_accounts",
		},
		{
			name:    "concurrency active_accounts exceeds population",
			ini:     replaceLine("active_accounts = 32", "active_accounts = 101"),
			wantErr: "exceeds accounts.count",
		},
		{
			name:    "engine bad strategy",
			ini:     replaceLine("strategy              = dynamic", "strategy = turbocharged"),
			wantErr: "strategy",
		},
		{
			name:    "engine sharded without workers",
			ini:     replaceLine("strategy              = dynamic", "strategy = sharded"),
			wantErr: "sharded_workers",
		},
		{
			name:    "engine sharded workers zero",
			ini:     replaceLines("strategy              = dynamic", "strategy = sharded", "sharded_workers       = 0", "sharded_workers = 0"),
			wantErr: "sharded_workers",
		},
		{
			name:    "engine max_queues below active set",
			ini:     replaceLine("max_queues            = 128", "max_queues = 16"),
			wantErr: "max_queues",
		},
		{
			name:    "engine max_queues non-numeric",
			ini:     replaceLine("max_queues            = 128", "max_queues = lots"),
			wantErr: "max_queues",
		},
		{
			name:    "engine idle_cleanup not a duration",
			ini:     replaceLine("idle_cleanup          = 2s", "idle_cleanup = soon"),
			wantErr: "idle_cleanup",
		},
		{
			name:    "engine idle_cleanup negative",
			ini:     replaceLine("idle_cleanup          = 2s", "idle_cleanup = -1s"),
			wantErr: "idle_cleanup",
		},
		{
			name:    "engine queue_capacity negative",
			ini:     replaceLine("queue_capacity        = 0", "queue_capacity = -1"),
			wantErr: "queue_capacity",
		},
		{
			name:    "engine slow_submit_threshold not a duration",
			ini:     replaceLine("slow_submit_threshold = 0", "slow_submit_threshold = soon"),
			wantErr: "slow_submit_threshold",
		},
		{
			name:    "engine slow_submit_threshold negative",
			ini:     replaceLine("slow_submit_threshold = 0", "slow_submit_threshold = -1s"),
			wantErr: "slow_submit_threshold",
		},
		{
			name:    "accounts count non-numeric",
			ini:     replaceLine("count = 100", "count = many"),
			wantErr: "count",
		},
		{
			name:    "instruments symbols blank entry",
			ini:     replaceLine("symbols = AAPL,SPX,MSFT", "symbols = AAPL,,MSFT"),
			wantErr: "blank",
		},
		{
			name:    "instruments duplicate symbol",
			ini:     replaceLine("symbols = AAPL,SPX,MSFT", "symbols = AAPL,AAPL"),
			wantErr: "duplicate",
		},
		{
			name:    "settlement collides with underlying",
			ini:     replaceLine("settlement = USD", "settlement = AAPL"),
			wantErr: "must not also be an underlying",
		},
		{
			name:    "lifecycle probability out of range",
			ini:     replaceLine("p_open = 0.40", "p_open = 1.5"),
			wantErr: "p_open",
		},
		{
			name:    "funding unknown trigger",
			ini:     replaceLine("trigger = balance_below", "trigger = on_tuesday"),
			wantErr: "trigger",
		},
		{
			name:    "funding amount non-positive",
			ini:     replaceLine("amount = 100000", "amount = 0"),
			wantErr: "amount",
		},
		{
			name:    "cohort weight non-positive",
			ini:     replaceLine("weight = 0.3", "weight = 0"),
			wantErr: "weight",
		},
		{
			name:    "cohort missing burst_len",
			ini:     removeLine("burst_len = 4"),
			wantErr: "burst_len",
		},
		{
			name:    "cohort bad size_weights",
			ini:     replaceLine("size_weights = 1:1,10:4,100:2", "size_weights = 1:1,bad"),
			wantErr: "size_weights",
		},
		{
			name:    "cohort zipf without zipf_s",
			ini:     removeLine("zipf_s = 1.3"),
			wantErr: "zipf_s",
		},
		{
			name:    "cohort zipf_s <= 1",
			ini:     replaceLine("zipf_s = 1.3", "zipf_s = 0.9"),
			wantErr: "zipf_s",
		},
	}

	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()
			_, err := Load(writeINI(t, tc.ini))
			if err == nil {
				t.Fatalf("Load() error = nil, want error containing %q", tc.wantErr)
			}
			if !contains(err.Error(), tc.wantErr) {
				t.Fatalf("Load() error = %q, want it to contain %q", err.Error(), tc.wantErr)
			}
		})
	}
}

// TestNoCohortsIsError confirms at least one cohort is required.
func TestNoCohortsIsError(t *testing.T) {
	t.Parallel()

	ini := removeLine("[cohort.chatty]")
	ini = removeSectionBlock(ini, "chatty")
	ini = removeSectionBlock(ini, "steady")
	_, err := Load(writeINI(t, ini))
	if err == nil || !contains(err.Error(), "cohort") {
		t.Fatalf("Load() error = %v, want a missing-cohort error", err)
	}
}

// TestBaselineConfigLoads loads the committed configs/baseline.ini through the
// strict parser, so the shipped reference config can never drift out of sync
// with the tightened validation. The path is relative to this package.
func TestBaselineConfigLoads(t *testing.T) {
	t.Parallel()

	cfg, err := Load(filepath.Join("..", "..", "configs", "baseline.ini"))
	if err != nil {
		t.Fatalf("baseline.ini failed strict validation: %v", err)
	}
	if len(cfg.Cohorts) == 0 {
		t.Fatal("baseline.ini parsed with no cohorts")
	}
	if cfg.Instruments.Settlement == "" || len(cfg.Instruments.Symbols) == 0 {
		t.Fatal("baseline.ini missing instruments")
	}
}

// TestRunValidationNotRegressed re-checks a couple of [run] rules survive the
// Phase-2 tightening (missing window_unit, bad seed).
func TestRunValidationNotRegressed(t *testing.T) {
	t.Parallel()

	bad := removeLine("window_unit = ops")
	if _, err := Load(writeINI(t, bad)); err == nil || !contains(err.Error(), "window_unit") {
		t.Fatalf("missing window_unit: err = %v", err)
	}

	badSeed := replaceLine("seed = 0xC0FFEE", "seed = notanumber")
	if _, err := Load(writeINI(t, badSeed)); err == nil || !contains(err.Error(), "seed") {
		t.Fatalf("bad seed: err = %v", err)
	}
}

// TestEngineMaxQueuesUnlimited confirms max_queues = 0 is accepted and means
// unlimited (it bypasses the >= active_accounts check).
func TestEngineMaxQueuesUnlimited(t *testing.T) {
	t.Parallel()

	ini := replaceLine("max_queues            = 128", "max_queues = 0")
	cfg, err := Load(writeINI(t, ini))
	if err != nil {
		t.Fatalf("max_queues = 0 (unlimited) should be valid, got %v", err)
	}
	if cfg.AsyncEngine.MaxQueues != 0 {
		t.Errorf("AsyncEngine.MaxQueues = %d, want 0 (unlimited)", cfg.AsyncEngine.MaxQueues)
	}
}

// TestEngineShardedStrategy confirms that strategy = sharded with a positive
// sharded_workers parses correctly and that max_queues / idle_cleanup are
// accepted (they are ignored but not rejected) while sharded_workers = 0
// produces an error.
func TestEngineShardedStrategy(t *testing.T) {
	t.Parallel()

	// Valid sharded config: replace dynamic knobs with sharded knobs.
	ini := replaceLines(
		"strategy              = dynamic", "strategy = sharded",
		"sharded_workers       = 0", "sharded_workers = 4",
	)
	cfg, err := Load(writeINI(t, ini))
	if err != nil {
		t.Fatalf("valid sharded config failed: %v", err)
	}
	if cfg.AsyncEngine.Strategy != AsyncEngineStrategySharded {
		t.Errorf("Strategy = %q, want sharded", cfg.AsyncEngine.Strategy)
	}
	if cfg.AsyncEngine.ShardedWorkers != 4 {
		t.Errorf("ShardedWorkers = %d, want 4", cfg.AsyncEngine.ShardedWorkers)
	}

	// sharded_workers = 0 with strategy = sharded is an error.
	badIni := replaceLine("strategy              = dynamic", "strategy = sharded")
	if _, err := Load(writeINI(t, badIni)); err == nil || !contains(err.Error(), "sharded_workers") {
		t.Fatalf("sharded with workers=0: err = %v, want sharded_workers error", err)
	}
}

// TestEngineSharedKnobs confirms queue_capacity and slow_submit_threshold parse
// for both strategies.
func TestEngineSharedKnobs(t *testing.T) {
	t.Parallel()

	ini := replaceLines(
		"queue_capacity        = 0", "queue_capacity = 512",
		"slow_submit_threshold = 0", "slow_submit_threshold = 250ms",
	)
	cfg, err := Load(writeINI(t, ini))
	if err != nil {
		t.Fatalf("shared knobs parse failed: %v", err)
	}
	if cfg.AsyncEngine.QueueCapacity != 512 {
		t.Errorf("QueueCapacity = %d, want 512", cfg.AsyncEngine.QueueCapacity)
	}
	if cfg.AsyncEngine.SlowSubmitThreshold != 250*time.Millisecond {
		t.Errorf("SlowSubmitThreshold = %s, want 250ms", cfg.AsyncEngine.SlowSubmitThreshold)
	}
}

// TestConcurrencyAndEngineSectionsRequired confirms both new sections are
// mandatory (no silent defaults for the bounded-concurrency knobs). Dropping a
// whole section's lines from fullConfig must produce a section-required error.
func TestConcurrencyAndEngineSectionsRequired(t *testing.T) {
	t.Parallel()

	withoutConcurrency := dropLines(fullConfig, "[concurrency]", "active_accounts = 32")
	if _, err := Load(writeINI(t, withoutConcurrency)); err == nil || !contains(err.Error(), "concurrency") {
		t.Fatalf("missing [concurrency]: err = %v", err)
	}

	withoutEngine := dropLines(fullConfig, "[async_engine]",
		"strategy              = dynamic",
		"max_queues            = 128",
		"idle_cleanup          = 2s",
		"sharded_workers       = 0",
		"queue_capacity        = 0",
		"slow_submit_threshold = 0",
	)
	if _, err := Load(writeINI(t, withoutEngine)); err == nil || !contains(err.Error(), "async_engine") {
		t.Fatalf("missing [async_engine]: err = %v", err)
	}
}
