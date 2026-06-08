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

// Package config loads and validates the INI configuration file for the
// spot-limit load test. Validation is up-front and explicit for the sections
// consumed by the generator: required keys must be present with valid values.
// The accepted conveniences are explicit: instruments.settlement defaults to
// USD, funding.seed and funding.top_up default to funding.amount, and the
// optional [arrival] and [report_delay] sections are parsed leniently.
package config

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"os"
	"sort"
	"strconv"
	"strings"
	"time"

	"github.com/shopspring/decimal"
	ini "gopkg.in/ini.v1"
)

// WindowUnit controls whether the sliding window is sized in operations or
// wall-clock time.
type WindowUnit string

const (
	WindowUnitOps  WindowUnit = "ops"
	WindowUnitWall WindowUnit = "wall"
)

// ReportDelayDistribution is the distribution of the simulated TS round-trip
// (report-return) delay.
type ReportDelayDistribution string

const (
	ReportDelayLognormal ReportDelayDistribution = "lognormal"
	ReportDelayFixed     ReportDelayDistribution = "fixed"
)

// Run holds the top-level knobs that govern a single load-test run.
type Run struct {
	// Seed is the RNG seed for the deterministic event stream.
	Seed uint64
	// TotalOps is the number of order-check operations to run (exclusive with Duration).
	TotalOps uint64
	// Duration is a human-readable wall-clock budget (exclusive with TotalOps; Phase 6).
	Duration string
	// Window is the sliding-window size.
	Window uint64
	// WindowUnit is ops or wall.
	WindowUnit WindowUnit
	// Observer controls whether the asyncengine observer is engaged.
	Observer bool
}

// Arrival holds the offered-rate model (Phase 2+). Inter-arrival spacing is
// always exponential.
type Arrival struct {
	OfferedRate uint64
}

// ReportDelay models the simulated TS round-trip before report settlement (Phase 2+).
type ReportDelay struct {
	Distribution ReportDelayDistribution
	Mean         string
	Sigma        float64
}

// Reject holds the target reject-rate controller knobs consumed by the
// generator's offline reject-rate controller (Phase 2).
type Reject struct {
	// TargetRate is the desired fraction of order-checks the shadow model
	// predicts will be rejected, in [0, 1).
	TargetRate float64
	// Tolerance is the allowed absolute deviation of the achieved predicted
	// reject rate from TargetRate, in (0, 1].
	Tolerance float64
}

// Accounts holds account-pool sizing consumed by the generator (Phase 2).
type Accounts struct {
	// Count is the number of distinct accounts in the population; must be > 0.
	Count uint64
}

// Concurrency holds the bounded-concurrency workload knob. Modelling every
// account as hot simultaneously is unrealistic; instead a large population has
// only a bounded ACTIVE WORKING SET hot at any moment (the rest act rarely, the
// dormant majority almost never). ActiveAccounts caps that working set.
type Concurrency struct {
	// ActiveAccounts is the maximum number of accounts concurrently "active"
	// (bursting) at any moment. It bounds the number of distinct accounts that
	// appear within any short window of the generated stream, which in turn
	// bounds the engine's live per-account dispatch queues. Must be > 0 and
	// <= Accounts.Count.
	ActiveAccounts uint64
}

// AsyncEngineStrategy is the dispatch strategy for the asyncengine.
type AsyncEngineStrategy string

const (
	// AsyncEngineStrategyDynamic uses a lazy per-account queue with idle
	// cleanup (the locked design default).
	AsyncEngineStrategyDynamic AsyncEngineStrategy = "dynamic"
	// AsyncEngineStrategySharded uses a fixed N-shard dispatch (lowest
	// per-call overhead, no per-account isolation).
	AsyncEngineStrategySharded AsyncEngineStrategy = "sharded"
)

// AsyncEngine holds the asyncengine builder knobs exposed by the [async_engine]
// INI section. These are DISPATCH/RESOURCE limits (like a connection cap), NOT
// synchronization semantics: the measured semantics stay per-account AccountSync
// regardless of these values.
type AsyncEngine struct {
	// Strategy selects the dispatch strategy: dynamic (default) or sharded.
	Strategy AsyncEngineStrategy

	// --- Dynamic-only knobs ---

	// MaxQueues is the Dynamic dispatch capacity (max concurrent live
	// per-account queues). 0 means unlimited. When nonzero it must be
	// >= Concurrency.ActiveAccounts so the active set fits with margin.
	// Ignored when Strategy = sharded.
	MaxQueues uint64
	// IdleCleanup is the per-account queue retire delay. 0 disables cleanup.
	// Ignored when Strategy = sharded.
	IdleCleanup time.Duration

	// --- Sharded-only knobs ---

	// ShardedWorkers is the number of fixed shards. Required and > 0 when
	// Strategy = sharded; ignored when Strategy = dynamic.
	ShardedWorkers int

	// --- Both strategies ---

	// QueueCapacity is the buffered channel size of each queue. 0 lets the
	// engine use its default (1024).
	QueueCapacity int
	// SlowSubmitThreshold is how long a producer blocks before the slow-submit
	// observer fires. 0 lets the engine use its default (1 minute).
	SlowSubmitThreshold time.Duration
}

// Instruments holds the tradable universe consumed by the generator (Phase 2).
type Instruments struct {
	// Symbols is the list of classic equity/index underlyings; must be non-empty
	// with no duplicates and no blank entries.
	Symbols []string
	// Settlement is the cash/settlement asset every instrument settles in
	// (classic: USD). Defaults to "USD" when the key is absent.
	Settlement string
}

// SizeBucket is one discrete order-size weight: a quantity (in lots/shares) and
// its selection weight. The generator normalises weights per cohort.
type SizeBucket struct {
	// Quantity is the integer order quantity for this bucket (lots/shares).
	Quantity uint64
	// Weight is the unnormalised selection weight; must be > 0.
	Weight float64
}

// SymbolSkew selects how a cohort biases its symbol choice.
type SymbolSkew string

const (
	// SymbolSkewUniform picks symbols uniformly at random.
	SymbolSkewUniform SymbolSkew = "uniform"
	// SymbolSkewZipf picks symbols by a Zipf law over the symbol list order.
	SymbolSkewZipf SymbolSkew = "zipf"
)

// Cohort is one named population segment parsed from a [cohort.<name>] section.
// All probabilities are in [0, 1]; weights are unnormalised positives.
type Cohort struct {
	// Name is the cohort label (the part after "cohort." in the section name).
	Name string
	// Weight is the unnormalised share of accounts assigned to this cohort.
	Weight float64
	// Activity is the per-wake probability that an assigned account emits an
	// order rather than staying idle; models cohort "chattiness".
	Activity float64
	// RejectPropensity is the relative likelihood (0..1) that, when the global
	// reject controller decides to force a reject, it targets this cohort.
	RejectPropensity float64
	// BurstLen is the number of consecutive orders an account fires once awake
	// (a simple wake/burst model); must be >= 1.
	BurstLen uint64
	// SizeWeights is the discrete order-size distribution; must be non-empty.
	SizeWeights []SizeBucket
	// SymbolSkew controls symbol selection bias.
	SymbolSkew SymbolSkew
	// ZipfS is the Zipf exponent (s > 1) used when SymbolSkew == zipf.
	ZipfS float64
}

// Lifecycle holds position state-machine transition probabilities consumed by
// the generator (Phase 2). Each is an independent probability in [0, 1].
type Lifecycle struct {
	// POpen is the probability of opening a fresh position when flat.
	POpen float64
	// PAdd is the probability of adding to an existing open position.
	PAdd float64
	// PPartialClose is the probability of closing part of an open position.
	PPartialClose float64
	// PFullClose is the probability of fully closing an open position.
	PFullClose float64
}

// FundingTrigger selects when the generator injects a self-funding top-up.
type FundingTrigger string

const (
	// FundingBalanceBelow tops up when an account's settlement available drops
	// below the configured threshold amount.
	FundingBalanceBelow FundingTrigger = "balance_below"
)

// Funding holds the self-funding trigger/amount consumed by the generator
// (Phase 2). The generator seeds an Absolute starting balance and injects
// top-ups via the AccountAdjustment pipeline so accounts never starve.
type Funding struct {
	// Trigger selects the top-up condition.
	Trigger FundingTrigger
	// Threshold is the settlement-available level at or below which a top-up
	// fires (for balance_below); must be > 0.
	Threshold decimal.Decimal
	// Seed is the Absolute settlement balance every account starts with; must
	// be > 0.
	Seed decimal.Decimal
	// TopUp is the Delta amount added on each trigger; must be > 0.
	TopUp decimal.Decimal
}

// Config is the fully validated, strongly-typed representation of one INI file.
type Config struct {
	// Path is the absolute path of the INI file as read.
	Path string
	// Hash is the hex-encoded SHA-256 of the raw file bytes.
	Hash string

	Run         Run
	Arrival     Arrival
	ReportDelay ReportDelay
	Reject      Reject
	Accounts    Accounts
	Concurrency Concurrency
	AsyncEngine AsyncEngine
	Instruments Instruments
	Lifecycle   Lifecycle
	Funding     Funding
	// Cohorts are the parsed [cohort.<name>] sections, sorted by name for
	// deterministic iteration. Must contain at least one cohort.
	Cohorts []Cohort
}

// Load reads path, validates required fields, computes a content hash, and
// returns a fully populated Config. Any validation failure is an explicit error
// with context.
func Load(path string) (*Config, error) {
	raw, err := os.ReadFile(path) //nolint:gosec // G304: path comes from the -config flag, checked by caller
	if err != nil {
		return nil, fmt.Errorf("config: read %q: %w", path, err)
	}

	hash := sha256.Sum256(raw)

	f, err := ini.Load(raw)
	if err != nil {
		return nil, fmt.Errorf("config: parse %q: %w", path, err)
	}

	cfg := &Config{
		Path: path,
		Hash: hex.EncodeToString(hash[:]),
	}

	if err := loadRun(f, &cfg.Run); err != nil {
		return nil, fmt.Errorf("config %q [run]: %w", path, err)
	}
	// [arrival] and [report_delay] drive the Phase-3 driver timing, not the
	// generator; they stay leniently parsed until the phase that consumes them.
	loadArrival(f, &cfg.Arrival)
	loadReportDelay(f, &cfg.ReportDelay)

	// Sections the generator (Phase 2) consumes are strictly validated: a
	// present-but-invalid value is an error with context, and a key required by
	// an enabled feature that is missing is an error.
	if err := loadReject(f, &cfg.Reject); err != nil {
		return nil, fmt.Errorf("config %q [reject]: %w", path, err)
	}
	if err := loadAccounts(f, &cfg.Accounts); err != nil {
		return nil, fmt.Errorf("config %q [accounts]: %w", path, err)
	}
	// Concurrency depends on Accounts.Count (active set <= population) and the
	// engine dispatch sizing depends on the active set, so they are validated in
	// that order.
	if err := loadConcurrency(f, &cfg.Concurrency, cfg.Accounts.Count); err != nil {
		return nil, fmt.Errorf("config %q [concurrency]: %w", path, err)
	}
	if err := loadAsyncEngine(f, &cfg.AsyncEngine, cfg.Concurrency.ActiveAccounts); err != nil {
		return nil, fmt.Errorf("config %q [async_engine]: %w", path, err)
	}
	if err := loadInstruments(f, &cfg.Instruments); err != nil {
		return nil, fmt.Errorf("config %q [instruments]: %w", path, err)
	}
	if err := loadLifecycle(f, &cfg.Lifecycle); err != nil {
		return nil, fmt.Errorf("config %q [lifecycle]: %w", path, err)
	}
	if err := loadFunding(f, &cfg.Funding); err != nil {
		return nil, fmt.Errorf("config %q [funding]: %w", path, err)
	}
	if err := loadCohorts(f, cfg); err != nil {
		return nil, fmt.Errorf("config %q: %w", path, err)
	}

	return cfg, nil
}

// loadRun parses and validates the mandatory [run] section.
func loadRun(f *ini.File, r *Run) error {
	sec, err := f.GetSection("run")
	if err != nil {
		return fmt.Errorf("section [run] is required")
	}

	// seed - required
	seedKey, err := sec.GetKey("seed")
	if err != nil {
		return fmt.Errorf("key seed is required")
	}
	seed, err := seedKey.Uint64()
	if err != nil {
		return fmt.Errorf("seed: must be a non-negative integer, got %q: %w", seedKey.String(), err)
	}
	r.Seed = seed

	// total_ops / duration - at least one is required
	if sec.HasKey("total_ops") && sec.HasKey("duration") {
		return fmt.Errorf("total_ops and duration are mutually exclusive; provide exactly one")
	}
	switch {
	case sec.HasKey("total_ops"):
		k := sec.Key("total_ops")
		n, err := k.Uint64()
		if err != nil || n == 0 {
			return fmt.Errorf("total_ops: must be a positive integer, got %q", k.String())
		}
		r.TotalOps = n
	case sec.HasKey("duration"):
		d := strings.TrimSpace(sec.Key("duration").String())
		if d == "" {
			return fmt.Errorf("duration: must be a non-empty duration string (e.g. 60s)")
		}
		r.Duration = d
	default:
		return fmt.Errorf("exactly one of total_ops or duration is required")
	}

	// window - required
	windowKey, err := sec.GetKey("window")
	if err != nil {
		return fmt.Errorf("key window is required")
	}
	window, err := windowKey.Uint64()
	if err != nil || window == 0 {
		return fmt.Errorf("window: must be a positive integer, got %q", windowKey.String())
	}
	r.Window = window

	// window_unit - required; must be ops or wall
	wuKey, err := sec.GetKey("window_unit")
	if err != nil {
		return fmt.Errorf("key window_unit is required")
	}
	switch WindowUnit(strings.TrimSpace(wuKey.String())) {
	case WindowUnitOps:
		r.WindowUnit = WindowUnitOps
	case WindowUnitWall:
		r.WindowUnit = WindowUnitWall
	default:
		return fmt.Errorf("window_unit: must be ops or wall, got %q", wuKey.String())
	}

	// observer - required; must be on or off
	obsKey, err := sec.GetKey("observer")
	if err != nil {
		return fmt.Errorf("key observer is required")
	}
	switch strings.TrimSpace(obsKey.String()) {
	case "on":
		r.Observer = true
	case "off":
		r.Observer = false
	default:
		return fmt.Errorf("observer: must be on or off, got %q", obsKey.String())
	}

	return nil
}

// loadArrival parses the optional [arrival] section; missing fields stay zero.
func loadArrival(f *ini.File, a *Arrival) {
	sec, err := f.GetSection("arrival")
	if err != nil {
		return
	}
	if k, err := sec.GetKey("offered_rate"); err == nil {
		if n, err := k.Uint64(); err == nil {
			a.OfferedRate = n
		}
	}
}

// loadReportDelay parses the optional [report_delay] section.
func loadReportDelay(f *ini.File, d *ReportDelay) {
	sec, err := f.GetSection("report_delay")
	if err != nil {
		return
	}
	if k, err := sec.GetKey("distribution"); err == nil {
		switch ReportDelayDistribution(strings.TrimSpace(k.String())) {
		case ReportDelayLognormal:
			d.Distribution = ReportDelayLognormal
		case ReportDelayFixed:
			d.Distribution = ReportDelayFixed
		}
	}
	if k, err := sec.GetKey("mean"); err == nil {
		d.Mean = strings.TrimSpace(k.String())
	}
	if k, err := sec.GetKey("sigma"); err == nil {
		if v, err := k.Float64(); err == nil {
			d.Sigma = v
		}
	}
}

// loadReject strictly validates the [reject] section the generator's
// reject-rate controller consumes.
func loadReject(f *ini.File, r *Reject) error {
	sec, err := f.GetSection("reject")
	if err != nil {
		return fmt.Errorf("section [reject] is required")
	}
	rate, err := requireUnitFloat(sec, "target_rate")
	if err != nil {
		return err
	}
	if rate >= 1 {
		return fmt.Errorf("target_rate: must be < 1, got %v", rate)
	}
	r.TargetRate = rate

	tol, err := requireFloat(sec, "tolerance")
	if err != nil {
		return err
	}
	if tol <= 0 || tol > 1 {
		return fmt.Errorf("tolerance: must be in (0, 1], got %v", tol)
	}
	r.Tolerance = tol
	return nil
}

// loadAccounts strictly validates the [accounts] section.
func loadAccounts(f *ini.File, a *Accounts) error {
	sec, err := f.GetSection("accounts")
	if err != nil {
		return fmt.Errorf("section [accounts] is required")
	}
	k, err := sec.GetKey("count")
	if err != nil {
		return fmt.Errorf("key count is required")
	}
	n, err := k.Uint64()
	if err != nil || n == 0 {
		return fmt.Errorf("count: must be a positive integer, got %q", k.String())
	}
	a.Count = n
	return nil
}

// loadConcurrency strictly validates the [concurrency] section. The active
// working set must be positive and must not exceed the total population.
func loadConcurrency(f *ini.File, c *Concurrency, population uint64) error {
	sec, err := f.GetSection("concurrency")
	if err != nil {
		return fmt.Errorf("section [concurrency] is required")
	}
	k, err := sec.GetKey("active_accounts")
	if err != nil {
		return fmt.Errorf("key active_accounts is required")
	}
	n, err := k.Uint64()
	if err != nil || n == 0 {
		return fmt.Errorf("active_accounts: must be a positive integer, got %q", k.String())
	}
	if n > population {
		return fmt.Errorf("active_accounts: %d exceeds accounts.count %d (active set cannot exceed the population)", n, population)
	}
	c.ActiveAccounts = n
	return nil
}

// loadAsyncEngine strictly validates the [async_engine] dispatch-sizing section.
// It exposes the complete asyncengine builder surface: strategy (dynamic|sharded),
// per-strategy knobs, and shared knobs (queue_capacity, slow_submit_threshold).
func loadAsyncEngine(f *ini.File, e *AsyncEngine, activeAccounts uint64) error {
	sec, err := f.GetSection("async_engine")
	if err != nil {
		return fmt.Errorf("section [async_engine] is required")
	}

	// strategy — required; must be "dynamic" or "sharded"
	stratKey, err := sec.GetKey("strategy")
	if err != nil {
		return fmt.Errorf("key strategy is required")
	}
	switch AsyncEngineStrategy(strings.TrimSpace(stratKey.String())) {
	case AsyncEngineStrategyDynamic:
		e.Strategy = AsyncEngineStrategyDynamic
	case AsyncEngineStrategySharded:
		e.Strategy = AsyncEngineStrategySharded
	default:
		return fmt.Errorf("strategy: must be %q or %q, got %q",
			AsyncEngineStrategyDynamic, AsyncEngineStrategySharded, stratKey.String())
	}

	// max_queues — required key; Dynamic only: active-set constraint; ignored for sharded
	mqKey, err := sec.GetKey("max_queues")
	if err != nil {
		return fmt.Errorf("key max_queues is required")
	}
	mq, err := mqKey.Uint64()
	if err != nil {
		return fmt.Errorf("max_queues: must be a non-negative integer (0 = unlimited), got %q", mqKey.String())
	}
	if e.Strategy == AsyncEngineStrategyDynamic {
		// 0 means unlimited; any nonzero cap must hold the active working set so a
		// healthy run never trips the dispatch limit on its steady-state load.
		if mq != 0 && mq < activeAccounts {
			return fmt.Errorf("max_queues: %d must be 0 (unlimited) or >= concurrency.active_accounts %d (dynamic strategy)", mq, activeAccounts)
		}
	}
	e.MaxQueues = mq

	// idle_cleanup — required key; Dynamic only: queue retire delay; ignored for sharded
	icKey, err := sec.GetKey("idle_cleanup")
	if err != nil {
		return fmt.Errorf("key idle_cleanup is required")
	}
	idleDur, err := time.ParseDuration(strings.TrimSpace(icKey.String()))
	if err != nil {
		return fmt.Errorf("idle_cleanup: must be a duration (e.g. 5s), got %q: %w", icKey.String(), err)
	}
	if idleDur < 0 {
		return fmt.Errorf("idle_cleanup: must be >= 0 (0 = disabled), got %q", icKey.String())
	}
	e.IdleCleanup = idleDur

	// sharded_workers — required key; must be > 0 when strategy = sharded
	swKey, err := sec.GetKey("sharded_workers")
	if err != nil {
		return fmt.Errorf("key sharded_workers is required")
	}
	swRaw, err := swKey.Int()
	if err != nil {
		return fmt.Errorf("sharded_workers: must be a non-negative integer, got %q", swKey.String())
	}
	if e.Strategy == AsyncEngineStrategySharded {
		if swRaw <= 0 {
			return fmt.Errorf("sharded_workers: must be > 0 when strategy = sharded, got %d", swRaw)
		}
	}
	e.ShardedWorkers = swRaw

	// queue_capacity — required key; both strategies; 0 = engine default
	qcKey, err := sec.GetKey("queue_capacity")
	if err != nil {
		return fmt.Errorf("key queue_capacity is required")
	}
	qc, err := qcKey.Int()
	if err != nil {
		return fmt.Errorf("queue_capacity: must be a non-negative integer (0 = engine default 1024), got %q", qcKey.String())
	}
	if qc < 0 {
		return fmt.Errorf("queue_capacity: must be >= 0 (0 = engine default), got %d", qc)
	}
	e.QueueCapacity = qc

	// slow_submit_threshold — required key; both strategies; 0 = engine default
	sstKey, err := sec.GetKey("slow_submit_threshold")
	if err != nil {
		return fmt.Errorf("key slow_submit_threshold is required")
	}
	sstStr := strings.TrimSpace(sstKey.String())
	// Allow bare "0" as a valid alias for "0s" (engine default).
	if sstStr == "0" {
		sstStr = "0s"
	}
	sst, err := time.ParseDuration(sstStr)
	if err != nil {
		return fmt.Errorf("slow_submit_threshold: must be a duration (e.g. 1m) or 0 (engine default), got %q: %w", sstKey.String(), err)
	}
	if sst < 0 {
		return fmt.Errorf("slow_submit_threshold: must be >= 0 (0 = engine default 1m), got %q", sstKey.String())
	}
	e.SlowSubmitThreshold = sst

	return nil
}

// loadInstruments strictly validates the [instruments] section.
func loadInstruments(f *ini.File, inst *Instruments) error {
	sec, err := f.GetSection("instruments")
	if err != nil {
		return fmt.Errorf("section [instruments] is required")
	}
	k, err := sec.GetKey("symbols")
	if err != nil {
		return fmt.Errorf("key symbols is required")
	}
	parts := strings.Split(k.String(), ",")
	symbols := make([]string, 0, len(parts))
	seen := make(map[string]struct{}, len(parts))
	for i, p := range parts {
		s := strings.TrimSpace(p)
		if s == "" {
			return fmt.Errorf("symbols: entry %d is blank", i+1)
		}
		if _, dup := seen[s]; dup {
			return fmt.Errorf("symbols: duplicate entry %q", s)
		}
		seen[s] = struct{}{}
		symbols = append(symbols, s)
	}
	if len(symbols) == 0 {
		return fmt.Errorf("symbols: must list at least one instrument")
	}
	inst.Symbols = symbols

	// settlement is optional; classic default is USD.
	settlement := "USD"
	if sk, err := sec.GetKey("settlement"); err == nil {
		settlement = strings.TrimSpace(sk.String())
		if settlement == "" {
			return fmt.Errorf("settlement: must be a non-empty asset code")
		}
	}
	if _, clash := seen[settlement]; clash {
		return fmt.Errorf("settlement %q must not also be an underlying symbol", settlement)
	}
	inst.Settlement = settlement
	return nil
}

// loadLifecycle strictly validates the [lifecycle] transition probabilities.
func loadLifecycle(f *ini.File, lc *Lifecycle) error {
	sec, err := f.GetSection("lifecycle")
	if err != nil {
		return fmt.Errorf("section [lifecycle] is required")
	}
	open, err := requireUnitFloat(sec, "p_open")
	if err != nil {
		return err
	}
	add, err := requireUnitFloat(sec, "p_add")
	if err != nil {
		return err
	}
	partial, err := requireUnitFloat(sec, "p_partial_close")
	if err != nil {
		return err
	}
	full, err := requireUnitFloat(sec, "p_full_close")
	if err != nil {
		return err
	}
	lc.POpen, lc.PAdd, lc.PPartialClose, lc.PFullClose = open, add, partial, full
	return nil
}

// loadFunding strictly validates the [funding] section.
func loadFunding(f *ini.File, fd *Funding) error {
	sec, err := f.GetSection("funding")
	if err != nil {
		return fmt.Errorf("section [funding] is required")
	}
	tk, err := sec.GetKey("trigger")
	if err != nil {
		return fmt.Errorf("key trigger is required")
	}
	switch FundingTrigger(strings.TrimSpace(tk.String())) {
	case FundingBalanceBelow:
		fd.Trigger = FundingBalanceBelow
	default:
		return fmt.Errorf("trigger: must be %q, got %q", FundingBalanceBelow, tk.String())
	}

	threshold, err := requirePositiveDecimal(sec, "amount")
	if err != nil {
		return err
	}
	fd.Threshold = threshold

	// seed/top_up default to the trigger amount when omitted so the smallest
	// valid config (just trigger + amount) still funds and tops up.
	fd.Seed = threshold
	if sec.HasKey("seed") {
		v, err := requirePositiveDecimal(sec, "seed")
		if err != nil {
			return err
		}
		fd.Seed = v
	}
	fd.TopUp = threshold
	if sec.HasKey("top_up") {
		v, err := requirePositiveDecimal(sec, "top_up")
		if err != nil {
			return err
		}
		fd.TopUp = v
	}
	return nil
}

// loadCohorts parses and validates every repeatable [cohort.<name>] section.
// At least one cohort is required; weights need not sum to one (the generator
// normalises them), but each must be a positive number.
func loadCohorts(f *ini.File, cfg *Config) error {
	var cohorts []Cohort
	for _, sec := range f.Sections() {
		name, ok := strings.CutPrefix(sec.Name(), "cohort.")
		if !ok {
			continue
		}
		name = strings.TrimSpace(name)
		if name == "" {
			return fmt.Errorf("[cohort.]: cohort name must not be empty")
		}
		c, err := parseCohort(sec, name)
		if err != nil {
			return fmt.Errorf("[cohort.%s]: %w", name, err)
		}
		cohorts = append(cohorts, c)
	}
	if len(cohorts) == 0 {
		return fmt.Errorf("at least one [cohort.<name>] section is required")
	}
	// Deterministic order regardless of INI section order.
	sort.Slice(cohorts, func(i, j int) bool { return cohorts[i].Name < cohorts[j].Name })
	cfg.Cohorts = cohorts
	return nil
}

// parseCohort strictly validates one [cohort.<name>] section.
func parseCohort(sec *ini.Section, name string) (Cohort, error) {
	c := Cohort{Name: name}

	weight, err := requirePositiveFloat(sec, "weight")
	if err != nil {
		return Cohort{}, err
	}
	c.Weight = weight

	if c.Activity, err = requireUnitFloat(sec, "activity"); err != nil {
		return Cohort{}, err
	}
	if c.RejectPropensity, err = requireUnitFloat(sec, "reject_propensity"); err != nil {
		return Cohort{}, err
	}

	burstKey, err := sec.GetKey("burst_len")
	if err != nil {
		return Cohort{}, fmt.Errorf("key burst_len is required")
	}
	burst, err := burstKey.Uint64()
	if err != nil || burst == 0 {
		return Cohort{}, fmt.Errorf("burst_len: must be a positive integer, got %q", burstKey.String())
	}
	c.BurstLen = burst

	buckets, err := parseSizeWeights(sec)
	if err != nil {
		return Cohort{}, err
	}
	c.SizeWeights = buckets

	skewKey, err := sec.GetKey("symbol_skew")
	if err != nil {
		return Cohort{}, fmt.Errorf("key symbol_skew is required")
	}
	switch SymbolSkew(strings.TrimSpace(skewKey.String())) {
	case SymbolSkewUniform:
		c.SymbolSkew = SymbolSkewUniform
	case SymbolSkewZipf:
		c.SymbolSkew = SymbolSkewZipf
		s, err := requireFloat(sec, "zipf_s")
		if err != nil {
			return Cohort{}, err
		}
		if s <= 1 {
			return Cohort{}, fmt.Errorf("zipf_s: must be > 1, got %v", s)
		}
		c.ZipfS = s
	default:
		return Cohort{}, fmt.Errorf("symbol_skew: must be %q or %q, got %q",
			SymbolSkewUniform, SymbolSkewZipf, skewKey.String())
	}

	return c, nil
}

// parseSizeWeights parses the required size_weights key, a comma-separated list
// of "qty:weight" pairs (e.g. "1:5,10:3,100:1").
func parseSizeWeights(sec *ini.Section) ([]SizeBucket, error) {
	k, err := sec.GetKey("size_weights")
	if err != nil {
		return nil, fmt.Errorf("key size_weights is required")
	}
	parts := strings.Split(k.String(), ",")
	buckets := make([]SizeBucket, 0, len(parts))
	for i, p := range parts {
		p = strings.TrimSpace(p)
		if p == "" {
			return nil, fmt.Errorf("size_weights: entry %d is blank", i+1)
		}
		qStr, wStr, ok := strings.Cut(p, ":")
		if !ok {
			return nil, fmt.Errorf("size_weights: entry %q must be qty:weight", p)
		}
		qty, err := parseUint64(strings.TrimSpace(qStr))
		if err != nil || qty == 0 {
			return nil, fmt.Errorf("size_weights: quantity in %q must be a positive integer", p)
		}
		w, err := parseFloat64(strings.TrimSpace(wStr))
		if err != nil || w <= 0 {
			return nil, fmt.Errorf("size_weights: weight in %q must be a positive number", p)
		}
		buckets = append(buckets, SizeBucket{Quantity: qty, Weight: w})
	}
	if len(buckets) == 0 {
		return nil, fmt.Errorf("size_weights: must list at least one qty:weight bucket")
	}
	return buckets, nil
}

// requireFloat reads a required key as a float64, error on missing or invalid.
func requireFloat(sec *ini.Section, key string) (float64, error) {
	k, err := sec.GetKey(key)
	if err != nil {
		return 0, fmt.Errorf("key %s is required", key)
	}
	v, err := k.Float64()
	if err != nil {
		return 0, fmt.Errorf("%s: must be a number, got %q", key, k.String())
	}
	return v, nil
}

// requireUnitFloat reads a required key as a probability in [0, 1].
func requireUnitFloat(sec *ini.Section, key string) (float64, error) {
	v, err := requireFloat(sec, key)
	if err != nil {
		return 0, err
	}
	if v < 0 || v > 1 {
		return 0, fmt.Errorf("%s: must be in [0, 1], got %v", key, v)
	}
	return v, nil
}

// requirePositiveFloat reads a required key as a strictly positive float64.
func requirePositiveFloat(sec *ini.Section, key string) (float64, error) {
	v, err := requireFloat(sec, key)
	if err != nil {
		return 0, err
	}
	if v <= 0 {
		return 0, fmt.Errorf("%s: must be > 0, got %v", key, v)
	}
	return v, nil
}

// requirePositiveDecimal reads a required key as a strictly positive decimal,
// preserving exact precision (never via float64).
func requirePositiveDecimal(sec *ini.Section, key string) (decimal.Decimal, error) {
	k, err := sec.GetKey(key)
	if err != nil {
		return decimal.Decimal{}, fmt.Errorf("key %s is required", key)
	}
	d, err := decimal.NewFromString(strings.TrimSpace(k.String()))
	if err != nil {
		return decimal.Decimal{}, fmt.Errorf("%s: must be a decimal, got %q", key, k.String())
	}
	if !d.IsPositive() {
		return decimal.Decimal{}, fmt.Errorf("%s: must be > 0, got %s", key, d.String())
	}
	return d, nil
}

// parseUint64 parses a complete base-10 unsigned integer string (no trailing
// garbage, unlike Sscanf).
func parseUint64(s string) (uint64, error) {
	return strconv.ParseUint(s, 10, 64)
}

// parseFloat64 parses a complete float string (no trailing garbage).
func parseFloat64(s string) (float64, error) {
	return strconv.ParseFloat(s, 64)
}
