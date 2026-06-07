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

// Package reporter writes a plain-text load-test report to an io.Writer
// (typically os.Stdout). The report is structured in named blocks per the
// design doc (section 7/9).
//
// Block order:
//   - Headline   — steady-state OPEN-LOOP order-check p50/p99 (prominent, honest)
//   - Environment — host, runtime, pit commit, core build profile, run config
//   - Workload   — counts, reject rate, op mix, cohort summary
//   - Trajectory — per-window percentile evolution for order-check and settlement
//   - Distribution — final merged percentiles + harness self-overhead
//   - Diagnostics — service-time diagnostic + inner metrics (queue wait, compute)
//   - Disclaimer — what is/is not measured + one-line reproduction recipe
//
// A run is INVALID — not a valid latency measurement — when it hit dispatch
// backpressure (ErrQueueLimit) or produced a zero anti-DCE checksum on a
// non-empty run (decisions not provably consumed). A HARNESS handoff stall is
// NOT an invalidity trigger — the handoff is non-blocking and off the measured
// path, so it is a diagnostic only. For an invalid run the caller uses
// WriteInvalid, which prints a prominent invalid-run banner naming the ACTUAL
// reason(s) plus the non-latency disclosure (environment, workload counts,
// dispatch sizing, backpressure, handoff stalls) and OMITS the Headline,
// Trajectory, and Distribution percentile blocks entirely.
//
// Stdout/stderr separation: this package writes only to the w argument (stdout).
// Progress noise belongs on stderr and must never pass through this package.
package reporter

import (
	"fmt"
	"io"
	"strings"
	"time"

	"openpit-loadtest-spot-funds-go/internal/config"
	"openpit-loadtest-spot-funds-go/internal/env"
	"openpit-loadtest-spot-funds-go/internal/generator"
	"openpit-loadtest-spot-funds-go/internal/measurement"
)

// separatorWidth is the column width used for section dividers.
const separatorWidth = 72

// pctScale converts a fraction in [0,1] to a percentage.
const pctScale = 100.0

// trajectoryRuleWidthOC is the rule width under the order-check trajectory header.
const trajectoryRuleWidthOC = 67

// trajectoryRuleWidthSet is the rule width under the settlement trajectory header.
const trajectoryRuleWidthSet = 58

// Write prints the full post-run report to w. It must be the ONLY writer to
// w during the call; the caller is responsible for ensuring no progress noise
// reaches the same stream.
//
// configFlag is the raw -config flag value (used verbatim in the reproduction
// recipe). streamStats is the generator's pre-run prediction summary; it is
// used only for cohort/workload context.
//
// Use Write ONLY for a VALID run (no backpressure, non-zero checksum); a handoff
// stall is a diagnostic and does not make a run invalid. When the run is invalid
// (driver returns ErrBackpressureInvalidRun or ErrZeroChecksumInvalidRun), call
// WriteInvalid instead: the headline and latency percentiles must be suppressed.
func Write(
	w io.Writer,
	e env.Env,
	cfg *config.Config,
	configFlag string,
	snap measurement.Snapshot,
	streamStats generator.StreamStats,
) {
	writeHeadline(w, snap)
	writeEnvironment(w, e, cfg)
	writeWorkload(w, snap, streamStats, cfg)
	writeTrajectory(w, snap)
	writeDistribution(w, snap)
	writeDiagnostics(w, snap, cfg)
	writeDisclaimer(w, cfg, configFlag)
}

// WriteInvalid prints the INVALID-RUN report to w for a run that is not a valid
// latency measurement, for ANY of the invalid reasons: dispatch backpressure
// (asyncengine.ErrQueueLimit) or a zero anti-DCE checksum on a non-empty run
// (decisions not provably consumed). A HARNESS handoff stall is NOT an invalid
// reason — it is a diagnostic only and never suppresses the headline. It OMITS
// the Headline block
// and ALL latency-distribution percentile blocks (Trajectory, Distribution, the
// service-time/inner-metrics latency decomposition). It keeps the non-latency
// diagnostics — environment, workload counts, dispatch sizing, backpressure, and
// handoff stalls — and prints a prominent invalid-run banner at the top and
// bottom that names the ACTUAL reason(s) so the suppression cannot be missed.
//
// It must be the ONLY writer to w during the call.
func WriteInvalid(
	w io.Writer,
	e env.Env,
	cfg *config.Config,
	configFlag string,
	snap measurement.Snapshot,
	streamStats generator.StreamStats,
) {
	writeInvalidBanner(w, snap)
	writeEnvironment(w, e, cfg)
	writeWorkload(w, snap, streamStats, cfg)
	writeInvalidFooter(w, snap, configFlag)
}

// zeroChecksumInvalid reports whether the run is invalid for a zero anti-DCE
// checksum: a non-empty run (some order-class op, settlement, or funding
// resolved) whose checksum is zero, meaning the per-decision fold was elided or
// no decision was provably consumed.
func zeroChecksumInvalid(snap measurement.Snapshot) bool {
	resolved := snap.TotalOrderChecks + snap.TotalSettlements + snap.TotalFundings
	return resolved > 0 && snap.Checksum == 0
}

// invalidReasons returns the human-readable invalid reason(s) present in the
// snapshot, in precedence order (backpressure, zero checksum). At least one is
// present whenever WriteInvalid is called. A handoff stall is NOT an invalid
// reason — it is a diagnostic only (off the measured path, non-blocking handoff).
func invalidReasons(snap measurement.Snapshot) []string {
	var reasons []string
	if snap.Backpressure > 0 {
		reasons = append(reasons, fmt.Sprintf(
			"dispatch backpressure (ErrQueueLimit x %d)", snap.Backpressure))
	}
	if zeroChecksumInvalid(snap) {
		reasons = append(reasons, "zero anti-DCE checksum on a non-empty run")
	}
	return reasons
}

// writeInvalidBanner prints the prominent invalid-run notice naming every reason
// the run is invalid. It is the loud, unmissable header that explains why no
// latency numbers follow.
func writeInvalidBanner(w io.Writer, snap measurement.Snapshot) {
	reasons := invalidReasons(snap)
	pf(w, "*** RUN INVALID: %s; latency numbers suppressed — this is not a valid "+
		"measurement. ***", strings.Join(reasons, "; "))
	p(w, "")

	if snap.Backpressure > 0 {
		p(w, "  Backpressure: the engine refused one or more submits because the")
		p(w, "  live-queue cap was reached. Those ops never produced a decision, so the")
		p(w, "  latency sample is incomplete and skewed. Fix the dispatch sizing (raise")
		p(w, "  max_queues or lower active_accounts) and re-run.")
		p(w, "")
	}
	if zeroChecksumInvalid(snap) {
		p(w, "  Zero checksum: the run resolved a non-empty set of decisions but the")
		p(w, "  anti-DCE checksum is zero, so the decisions were not provably consumed")
		p(w, "  (the measurement loop may have been optimized away). The numbers cannot be")
		p(w, "  trusted and the headline is suppressed.")
		p(w, "")
	}

	pf(w, "  Backpressure (ErrQueueLimit submits): %d", snap.Backpressure)
	pf(w, "  Handoff stalls (harness starvation)  : %d", snap.HandoffStalls)
	pf(w, "  Anti-DCE checksum                    : 0x%016X", snap.Checksum)
	p(w, "")
}

// writeInvalidFooter prints the closing banner and the reproduction recipe so
// the run can be re-attempted after the cause is fixed.
func writeInvalidFooter(w io.Writer, snap measurement.Snapshot, configFlag string) {
	p(w, "=== Reproduction (after fixing the cause above) ===")
	p(w, "")
	pf(w, "  cd <repo>")
	pf(w, "  cargo build --release")
	pf(w, "  export OPENPIT_RUNTIME_LIBRARY_PATH=$(pwd)/target/release/libopenpit_ffi.<dylib|so>")
	pf(w, "  cd examples/go/spot_loadtest")
	pf(w, "  go build ./...")
	pf(w, "  ./spot_loadtest -config %s", configFlag)
	p(w, "")
	pf(w, "*** RUN INVALID: latency numbers suppressed (%s). ***",
		strings.Join(invalidReasons(snap), "; "))
	p(w, strings.Repeat("-", separatorWidth))
}

// steadyStateLabel returns a human-readable label describing which windows
// are considered steady-state, given the warmup window count from the Snapshot.
func steadyStateLabel(snap measurement.Snapshot) string {
	if snap.WarmupWindows == 0 {
		return "all windows (single window — no warmup exclusion possible)"
	}
	return fmt.Sprintf(
		"windows 2-%d (window 1 excluded as warmup: JIT + cache + engine ramp-up)",
		len(snap.Windows),
	)
}

// writeHeadline prints the prominent C-level summary: the steady-state
// OPEN-LOOP order-check latency (measured resolve - intended virtual arrival),
// p50 and p99, plus the full tail (p99/p99.9/max) so the tail is always visible
// and cannot be hidden. This is an honest open-loop, coordinated-omission-
// defended number: t0 is each event's intended arrival on the virtual causal
// timeline, stamped independently of when the submit actually happened, so any
// engine queueing or stall under load is reflected in the tail.
//
// Steady-state percentiles come from Snapshot.SteadyStateOrderCheck, which is
// computed by a lossless HdrHistogram Merge over the raw per-window histograms
// of the steady-state windows. This avoids the statistical invalidity of
// aggregating per-window percentile point-values (percentile-of-percentiles).
func writeHeadline(w io.Writer, snap measurement.Snapshot) {
	p(w, "=== Headline: Open-Loop Order-Check Latency (intended arrival -> decision) ===")
	p(w, "")
	p(w, "  Open-loop latency-under-load: t0 is the event's intended arrival on the")
	p(w, "  virtual causal timeline (NOT the actual submit instant), so queueing and")
	p(w, "  stalls under load are counted, not omitted (coordinated-omission defence).")
	p(w, "")

	ocSteady := snap.SteadyStateOrderCheck
	pf(w, "  Steady-state definition : %s", steadyStateLabel(snap))
	p(w, "")
	pf(w, "  Order-check p50 (steady-state, open-loop): %s", fmtDur(ocSteady.P50))
	pf(w, "  Order-check p99 (steady-state, open-loop): %s", fmtDur(ocSteady.P99))
	p(w, "")
	p(w, "  Full tail (steady-state, so no warmup spike is hidden):")
	pf(w, "    p99   : %s", fmtDur(ocSteady.P99))
	pf(w, "    p99.9 : %s", fmtDur(ocSteady.P999))
	pf(w, "    max   : %s", fmtDur(ocSteady.Max))
	p(w, "")
	p(w, "  All-run merged (includes warmup window — full picture):")
	pf(w, "    p50   : %s", fmtDur(snap.OrderCheck.P50))
	pf(w, "    p99   : %s", fmtDur(snap.OrderCheck.P99))
	pf(w, "    p99.9 : %s", fmtDur(snap.OrderCheck.P999))
	pf(w, "    max   : %s", fmtDur(snap.OrderCheck.Max))
	p(w, "")
	pf(w, "  Throughput (decided ops/s, separate saturation metric): %.0f ops/s", snap.Throughput)
	pf(w, "  Max in-flight (open-loop depth witness)               : %d", snap.MaxInFlight)
	p(w, "")
}

func writeEnvironment(w io.Writer, e env.Env, cfg *config.Config) {
	p(w, "=== Environment ===")
	p(w, "")

	p(w, "Host:")
	pf(w, "  cpu model  : %s", e.Host.CPUModel)
	pf(w, "  cores      : %d", e.Host.Cores)
	pf(w, "  ram        : %s", e.Host.RAM)
	pf(w, "  os         : %s", e.Host.OS)
	pf(w, "  kernel     : %s", e.Host.Kernel)
	p(w, "")

	p(w, "Go runtime:")
	pf(w, "  version    : %s", e.Runtime.Version)
	pf(w, "  goos       : %s", e.Runtime.GOOS)
	pf(w, "  goarch     : %s", e.Runtime.GOARCH)
	pf(w, "  cgo        : %v", e.Runtime.CGOEnabled)
	p(w, "")

	p(w, "Pit repository:")
	// Tri-state working-tree status: clean | dirty | unknown. "unknown" (git
	// unavailable / not a repo / command error) is never collapsed to "clean", so
	// an unauditable build cannot print a false "clean".
	pf(w, "  commit     : %s (%s)", e.Pit.Commit, e.Pit.DirtyStatus())
	p(w, "")

	p(w, "Core (native runtime):")
	pf(w, "  version    : %s", e.Core.Version)
	pf(w, "  profile    : %s", e.Core.Profile)
	pf(w, "  opt_level  : %s", e.Core.OptLevel)
	pf(w, "  debug_assertions : %v", e.Core.DebugAssertions)
	pf(w, "  target     : %s", e.Core.Target)
	pf(w, "  target_cpu : %s", e.Core.TargetCPU)
	pf(w, "  lto        : %s", e.Core.LTO)
	pf(w, "  build_profile_raw : %s", e.Core.Raw)
	p(w, "")

	p(w, "Run config:")
	pf(w, "  config path: %s", cfg.Path)
	pf(w, "  config hash: %s (SHA-256)", cfg.Hash)
	pf(w, "  seed       : 0x%X", cfg.Run.Seed)
	if cfg.Run.TotalOps > 0 {
		pf(w, "  total_ops  : %d", cfg.Run.TotalOps)
	} else {
		pf(w, "  duration   : %s", cfg.Run.Duration)
	}
	pf(w, "  window     : %d %s", cfg.Run.Window, cfg.Run.WindowUnit)
	pf(w, "  observer   : %v", cfg.Run.Observer)
	p(w, "")
}

func writeWorkload(w io.Writer, snap measurement.Snapshot, streamStats generator.StreamStats, cfg *config.Config) {
	p(w, "=== Workload ===")
	p(w, "")

	total := snap.TotalOrderChecks + snap.TotalSettlements
	pf(w, "  Total resolved ops      : %d", total)
	// Order-check accepts/rejects are the ORDER-CHECK class ONLY; settlement
	// accepts/blocks are reported on their own lines below. A reader must never
	// see accepts exceed the order-check count.
	pf(w, "  Order-checks            : %d", snap.TotalOrderChecks)
	pf(w, "    accepts               : %d", snap.TotalAccepts)
	pf(w, "    rejects               : %d", snap.TotalRejects)
	pf(w, "  Settlements             : %d", snap.TotalSettlements)
	pf(w, "    accepts               : %d", snap.TotalSettlementAccepts)
	pf(w, "    blocked               : %d", snap.TotalSettlementBlocks)
	if snap.TotalFundings > 0 {
		pf(w, "  Funding adjustments     : %d (accepted %d / rejected %d)",
			snap.TotalFundings, snap.TotalFundingAccepts, snap.TotalFundingRejects)
	}
	p(w, "")

	writeConcurrency(w, snap, cfg)

	pf(w, "  Achieved reject rate    : %.4f (%.2f%%)",
		snap.AchievedRejectRate, snap.AchievedRejectRate*pctScale)
	pf(w, "  Target reject rate      : %.4f (%.2f%%)",
		cfg.Reject.TargetRate, cfg.Reject.TargetRate*pctScale)
	delta := snap.AchievedRejectRate - cfg.Reject.TargetRate
	if delta < 0 {
		delta = -delta
	}
	pf(w, "  Rate deviation          : %.4f (tolerance ±%.4f)", delta, cfg.Reject.Tolerance)
	p(w, "")

	p(w, "  Generator stream summary (pre-run predictions):")
	pf(w, "    order-checks  : %d", streamStats.OrderChecks)
	pf(w, "    accepts       : %d", streamStats.Accepts)
	pf(w, "    rejects       : %d", streamStats.Rejects)
	pf(w, "    settlements   : %d", streamStats.Settlements)
	pf(w, "    fundings      : %d", streamStats.Fundings)
	pf(w, "    forced rejects: %d", streamStats.ForcedRejects)
	pf(w, "    natural rej.  : %d", streamStats.NaturalRejects)
	if streamStats.OrderChecks > 0 {
		pf(w, "    predicted rej rate: %.4f", streamStats.PredictedRejectRate())
	}
	p(w, "")

	p(w, "  Cohorts:")
	totalWeight := 0.0
	for _, c := range cfg.Cohorts {
		totalWeight += c.Weight
	}
	for _, c := range cfg.Cohorts {
		share := 0.0
		if totalWeight > 0 {
			share = c.Weight / totalWeight * pctScale
		}
		pf(w, "    [%s] weight=%.2f (%.1f%%), activity=%.2f, reject_propensity=%.2f, burst_len=%d",
			c.Name, c.Weight, share, c.Activity, c.RejectPropensity, c.BurstLen)
	}
	p(w, "")

	if !snap.WallStart.IsZero() && snap.WallEnd.After(snap.WallStart) {
		elapsed := snap.WallEnd.Sub(snap.WallStart)
		pf(w, "  Wall time (first submit to last resolve): %s", elapsed.Round(time.Millisecond))
	}
	p(w, "")
}

// writeConcurrency discloses how concurrency was modelled and bounded: the full
// population vs the bounded active working set, the engine dispatch sizing (a
// resource knob, not sync semantics), and the backpressure count (0 in a healthy
// run). This makes the realism of the offered load auditable.
func writeConcurrency(w io.Writer, snap measurement.Snapshot, cfg *config.Config) {
	p(w, "  Concurrency model (bounded active working set):")
	pf(w, "    population (total accounts)   : %d", cfg.Accounts.Count)
	pf(w, "    active working set (max hot)  : %d", cfg.Concurrency.ActiveAccounts)
	if cfg.Accounts.Count > 0 {
		frac := float64(cfg.Concurrency.ActiveAccounts) / float64(cfg.Accounts.Count) * pctScale
		pf(w, "    active fraction of population : %.1f%%", frac)
	}
	p(w, "")

	ae := cfg.AsyncEngine
	p(w, "  Engine dispatch sizing (resource knob, NOT sync semantics):")
	pf(w, "    strategy                      : %s", ae.Strategy)
	switch ae.Strategy {
	case config.AsyncEngineStrategySharded:
		pf(w, "    sharded_workers               : %d", ae.ShardedWorkers)
	default: // dynamic
		if ae.MaxQueues == 0 {
			p(w, "    max_queues (Dynamic capacity) : unlimited (0)")
		} else {
			pf(w, "    max_queues (Dynamic capacity) : %d", ae.MaxQueues)
		}
		if ae.IdleCleanup == 0 {
			p(w, "    idle_cleanup (queue retire)   : disabled (0)")
		} else {
			pf(w, "    idle_cleanup (queue retire)   : %s", ae.IdleCleanup)
		}
	}
	if ae.QueueCapacity == 0 {
		p(w, "    queue_capacity (per-queue buf): default (1024)")
	} else {
		pf(w, "    queue_capacity (per-queue buf): %d", ae.QueueCapacity)
	}
	if ae.SlowSubmitThreshold == 0 {
		p(w, "    slow_submit_threshold         : default (1m)")
	} else {
		pf(w, "    slow_submit_threshold         : %s", ae.SlowSubmitThreshold)
	}
	p(w, "")

	// Backpressure is an explicit measured outcome: a submit the engine refused
	// because the live-queue cap was reached. A healthy baseline reports zero.
	if snap.Backpressure == 0 {
		p(w, "  Backpressure (ErrQueueLimit submits): 0 (healthy — dispatch held the load)")
	} else {
		pf(w, "  Backpressure (ErrQueueLimit submits): %d (dispatch capacity was exceeded; "+
			"the run is degraded — raise max_queues or lower active_accounts)", snap.Backpressure)
	}

	// Handoff stalls are a HARNESS-side DIAGNOSTIC: the collector -> finalizer fast
	// path filled and the collector spilled to the unbounded overflow. The handoff
	// is non-blocking and CommitAndClose is fully off the measured path (the latency
	// was already recorded at resolve), so a stall NEVER throttles the submit
	// schedule and does NOT invalidate the run — it only signals the finalizer pool
	// transiently lagged. A healthy baseline typically reports zero.
	if snap.HandoffStalls == 0 {
		p(w, "  Handoff stalls: 0 (finalizer pool kept up with the collector)")
	} else {
		pf(w, "  Handoff stalls: %d (DIAGNOSTIC — finalizer pool transiently lagged the "+
			"collector and spilled to the off-path overflow; does NOT throttle the submit "+
			"schedule or invalidate the run — raise the finalizer pool size if persistent)",
			snap.HandoffStalls)
	}

	// Submit->collector overflow depth is a DIAGNOSTIC only. A large peak means
	// collectors lagged submission — usually collectors legitimately blocked in
	// fut.Await (real engine latency, correctly in the headline), but under host
	// CPU starvation it can include collector-dispatch delay that inflates — never
	// flatters — the tail. It is NOT a stall and does NOT invalidate the run.
	if snap.MaxWorkOverflow == 0 {
		p(w, "  Submit->collector overflow (max depth): 0 (collectors kept up with submission)")
	} else {
		pf(w, "  Submit->collector overflow (max depth): %d (DIAGNOSTIC — collectors lagged "+
			"submission at peak; usually engine slowness correctly in the headline, but under "+
			"host CPU starvation can fold collector-dispatch delay into the tail — "+
			"cross-check throughput and engine-compute)",
			snap.MaxWorkOverflow)
	}
	p(w, "")
}

func writeTrajectory(w io.Writer, snap measurement.Snapshot) {
	p(w, "=== Trajectory (per-window percentiles) ===")
	p(w, "")

	if len(snap.Windows) == 0 {
		p(w, "  No windows recorded.")
		p(w, "")
		return
	}

	// Order-check trajectory.
	p(w, "  Order-check (open-loop: intended arrival -> decision, stage 1->2):")
	p(w, "  win  | ops   | p50        | p99        | p99.9      | wall")
	p(w, "  "+strings.Repeat("-", trajectoryRuleWidthOC))
	for i, win := range snap.Windows {
		label := fmt.Sprintf("%4d", i+1)
		if i == 0 && len(snap.Windows) > 1 {
			label += "w" // mark warmup window
		}
		wallRange := ""
		if !win.WallStart.IsZero() {
			wallRange = win.WallStart.Format("15:04:05") + "-" + win.WallEnd.Format("15:04:05")
		}
		pf(w, "  %-5s| %-6d| %-11s| %-11s| %-11s| %s",
			label,
			win.OrderCheck.Count,
			fmtDur(win.OrderCheck.P50),
			fmtDur(win.OrderCheck.P99),
			fmtDur(win.OrderCheck.P999),
			wallRange,
		)
	}
	if len(snap.Windows) > 1 {
		p(w, "  (w = warmup window, excluded from steady-state headline)")
	}
	p(w, "")

	// Settlement trajectory.
	p(w, "  Settlement (open-loop: intended arrival -> decision, stage 3->4):")
	p(w, "  win  | ops   | p50        | p99        | p99.9")
	p(w, "  "+strings.Repeat("-", trajectoryRuleWidthSet))
	for i, win := range snap.Windows {
		if win.Settlement.Count == 0 {
			continue
		}
		label := fmt.Sprintf("%4d", i+1)
		if i == 0 && len(snap.Windows) > 1 {
			label += "w"
		}
		pf(w, "  %-5s| %-6d| %-11s| %-11s| %s",
			label,
			win.Settlement.Count,
			fmtDur(win.Settlement.P50),
			fmtDur(win.Settlement.P99),
			fmtDur(win.Settlement.P999),
		)
	}
	p(w, "")
}

func writeDistribution(w io.Writer, snap measurement.Snapshot) {
	p(w, "=== Distribution (final merged, all windows) ===")
	p(w, "")

	p(w, "  Order-check latency (stage 1->2, open-loop intended arrival -> decision, incl. queue):")
	writePercentiles(w, snap.OrderCheck)
	p(w, "")

	p(w, "  Settlement latency (stage 3->4, open-loop intended arrival -> decision):")
	writePercentiles(w, snap.Settlement)
	p(w, "")

	p(w, "  Harness self-overhead (adjustment-path FFI+queue floor, quiescent engine):")
	p(w, "    NOTE: probed via ApplyAccountAdjustment, NOT ExecutePreTrade; read it")
	p(w, "    as the bare FFI+queue floor, not the order-check overhead.")
	if snap.Overhead.Probes == 0 {
		p(w, "    overhead probe disabled or not probed")
	} else {
		pf(w, "    probes : %d", snap.Overhead.Probes)
		pf(w, "    p50    : %s", fmtDur(snap.Overhead.Distribution.P50))
		pf(w, "    p99    : %s", fmtDur(snap.Overhead.Distribution.P99))
		pf(w, "    p99.9  : %s", fmtDur(snap.Overhead.Distribution.P999))
		pf(w, "    max    : %s", fmtDur(snap.Overhead.Distribution.Max))
	}
	p(w, "")

	// Clamped samples: latencies above the histogram ceiling are saturated to the
	// ceiling and counted, never dropped (coordinated-omission tail preservation).
	pf(w, "  Clamped samples (> hist max): %d", snap.ClampedSamples)
	if snap.ClampedSamples > 0 {
		p(w, "    NOTE: the upper tail saturated at the histogram ceiling; those")
		p(w, "    samples are counted at the ceiling, so tail percentiles at/above")
		p(w, "    the ceiling are a LOWER BOUND on the true latency.")
	}
	p(w, "")

	pf(w, "  Anti-DCE checksum: 0x%016X  (proof every decision was consumed)", snap.Checksum)
	p(w, "")
}

func writePercentiles(w io.Writer, p50 measurement.Percentiles) {
	pf(w, "    samples: %d", p50.Count)
	pf(w, "    p50    : %s", fmtDur(p50.P50))
	pf(w, "    p90    : %s", fmtDur(p50.P90))
	pf(w, "    p99    : %s", fmtDur(p50.P99))
	pf(w, "    p99.9  : %s", fmtDur(p50.P999))
	pf(w, "    max    : %s", fmtDur(p50.Max))
}

// writeServiceTime prints the service-time diagnostic (resolve - ACTUAL submit
// instant) for order-checks, with a loud, explicit label that it is NOT the
// headline. Service-time discounts the queue wait that accrued between an
// event's intended arrival and its actual submit, so it hides the saturation
// tail by construction; the headline is the open-loop latency-under-load above.
func writeServiceTime(w io.Writer, snap measurement.Snapshot) {
	st := snap.ServiceTime
	p(w, "  Service-time (resolve - ACTUAL submit), order-check:")
	p(w, "    DIAGNOSTIC, NOT the headline. It discounts queue wait before the actual")
	p(w, "    submit, so it hides the saturation tail; the headline is the open-loop")
	p(w, "    latency-under-load (intended arrival -> decision).")
	if st.Count == 0 {
		p(w, "    no samples")
	} else {
		pf(w, "    samples: %d", st.Count)
		pf(w, "    p50    : %s", fmtDur(st.P50))
		pf(w, "    p99    : %s", fmtDur(st.P99))
		pf(w, "    p99.9  : %s", fmtDur(st.P999))
		pf(w, "    max    : %s", fmtDur(st.Max))
	}
	p(w, "")
}

func writeDiagnostics(w io.Writer, snap measurement.Snapshot, cfg *config.Config) {
	p(w, "=== Diagnostics (decomposition, NOT the headline) ===")
	p(w, "")

	writeServiceTime(w, snap)

	if !cfg.Run.Observer {
		p(w, "  Observer disabled (observer = off in config). No inner metrics.")
		p(w, "  To enable: set observer = on in [run] section of the config.")
		p(w, "")
		return
	}

	im := snap.InnerMetrics

	p(w, "  NOTE: these are per-account AGGREGATE distributions, not per-order.")
	p(w, "  The residual is an approximation (aggregate, not per-op subtraction).")
	p(w, "")

	p(w, "  Queue wait (time a task spent in the async engine queue):")
	if im.QueueWait.Count == 0 {
		p(w, "    no samples (observer may not have fired)")
	} else {
		pf(w, "    callbacks: %d", im.Dequeues)
		pf(w, "    p50      : %s", fmtDur(im.QueueWait.P50))
		pf(w, "    p99      : %s", fmtDur(im.QueueWait.P99))
		pf(w, "    p99.9    : %s", fmtDur(im.QueueWait.P999))
		pf(w, "    max      : %s", fmtDur(im.QueueWait.Max))
	}
	p(w, "")

	p(w, "  Engine compute (wall time of the engine call inside the queue):")
	if im.EngineCompute.Count == 0 {
		p(w, "    no samples")
	} else {
		pf(w, "    callbacks: %d", im.Completes)
		pf(w, "    p50      : %s", fmtDur(im.EngineCompute.P50))
		pf(w, "    p99      : %s", fmtDur(im.EngineCompute.P99))
		pf(w, "    p99.9    : %s", fmtDur(im.EngineCompute.P999))
		pf(w, "    max      : %s", fmtDur(im.EngineCompute.Max))
	}
	p(w, "")

	// Aggregate FFI/handoff residual = order-check p50 minus (queue_wait p50 + engine_compute p50).
	// This is an APPROXIMATE aggregate estimate only; NOT a per-op subtraction.
	// Labeled explicitly so readers do not mistake it for a precise measurement.
	if im.QueueWait.Count > 0 && im.EngineCompute.Count > 0 {
		residualP50 := snap.OrderCheck.P50 - (im.QueueWait.P50 + im.EngineCompute.P50)
		residualP99 := snap.OrderCheck.P99 - (im.QueueWait.P99 + im.EngineCompute.P99)
		p(w, "  Aggregate FFI+handoff residual (APPROXIMATE — aggregate arithmetic,")
		p(w, "  NOT per-op subtraction; interpret with care):")
		pf(w, "    residual p50 = order_check.p50 - (queue_wait.p50 + engine_compute.p50)")
		pf(w, "               = %s - (%s + %s) = %s",
			fmtDur(snap.OrderCheck.P50), fmtDur(im.QueueWait.P50),
			fmtDur(im.EngineCompute.P50), fmtDurSigned(residualP50))
		pf(w, "    residual p99 = %s - (%s + %s) = %s",
			fmtDur(snap.OrderCheck.P99), fmtDur(im.QueueWait.P99),
			fmtDur(im.EngineCompute.P99), fmtDurSigned(residualP99))
	}
	p(w, "")

	p(w, "  Queue lifecycle:")
	pf(w, "    queues created: %d", im.QueuesCreated)
	pf(w, "    queues removed: %d", im.QueuesRemoved)
	p(w, "")
}

func writeDisclaimer(w io.Writer, _ *config.Config, configFlag string) {
	p(w, "=== Disclaimer ===")
	p(w, "")
	p(w, "What IS measured (HEADLINE = open-loop latency-under-load):")
	p(w, "  intended-arrival -> decision latency for the pre-trade order-check")
	p(w, "  (ExecutePreTrade), including the per-account async queue wait, through the")
	p(w, "  Go FFI boundary. Both order-check (stage 1->2) and report-settlement")
	p(w, "  (stage 3->4) are measured this way.")
	p(w, "")
	p(w, "  The harness is TRUE OPEN-LOOP. The generator assigns every event a virtual")
	p(w, "  arrival time on an offline CAUSAL timeline: order-check arrivals follow the")
	p(w, "  offered process; a settlement is its order's arrival plus a report-return")
	p(w, "  delay; a causally-dependent order follows its dependency's hold/fill. The")
	p(w, "  driver paces each event to its virtual arrival and submits without ever")
	p(w, "  blocking on a decision, so submissions pipeline (many ops in flight per")
	p(w, "  account). t0 is that virtual arrival, stamped independently of the actual")
	p(w, "  submit, so any queueing or stall under load is COUNTED, not omitted")
	p(w, "  (coordinated-omission defence). The per-op oracle stays strict: the engine")
	p(w, "  is FIFO-per-account with in-place holds, so the single live run reproduces")
	p(w, "  the shadow's offline-ordered decisions exactly.")
	p(w, "")
	p(w, "  The Diagnostics section also reports a SERVICE-TIME figure (resolve -")
	p(w, "  ACTUAL submit). That is a diagnostic only and is NEVER the headline: it")
	p(w, "  discounts the pre-submit queue wait and so hides the saturation tail.")
	p(w, "")
	p(w, "  Tail inflation caveat: the headline is resolve - VirtualT0 captured when")
	p(w, "  a collector awaits the future, so a deep submit->collector overflow can")
	p(w, "  fold collector-dispatch delay into the tail under host CPU starvation (it")
	p(w, "  can only inflate, never flatter); the Workload section's")
	p(w, "  'Submit->collector overflow (max depth)' diagnostic surfaces that depth.")
	p(w, "")
	p(w, "What is NOT measured:")
	p(w, "  client or TS network latency, serialization beyond the Go binding boundary,")
	p(w, "  OS scheduling jitter beyond what time.Now() already captures,")
	p(w, "  and any TS-side processing other than the pit core.")
	p(w, "")
	p(w, "Reproduction recipe:")
	pf(w, "  cd <repo>")
	pf(w, "  cargo build --release")
	pf(w, "  export OPENPIT_RUNTIME_LIBRARY_PATH=$(pwd)/target/release/libopenpit_ffi.<dylib|so>")
	pf(w, "  cd examples/go/spot_loadtest")
	pf(w, "  go build ./...")
	pf(w, "  ./spot_loadtest -config %s", configFlag)
	p(w, "")

	p(w, strings.Repeat("-", separatorWidth))
}

// fmtDur renders a time.Duration compactly for the report table.
func fmtDur(d time.Duration) string {
	if d <= 0 {
		return "0"
	}
	switch {
	case d >= time.Second:
		return fmt.Sprintf("%.3fs", d.Seconds())
	case d >= time.Millisecond:
		return fmt.Sprintf("%.3fms", float64(d)/float64(time.Millisecond))
	case d >= time.Microsecond:
		return fmt.Sprintf("%.1fµs", float64(d)/float64(time.Microsecond))
	default:
		return fmt.Sprintf("%dns", d.Nanoseconds())
	}
}

// fmtDurSigned renders a signed duration (may be negative residual).
func fmtDurSigned(d time.Duration) string {
	if d < 0 {
		return "-" + fmtDur(-d)
	}
	return fmtDur(d)
}

func p(w io.Writer, s string) {
	_, _ = fmt.Fprintln(w, s)
}

func pf(w io.Writer, format string, args ...any) {
	_, _ = fmt.Fprintf(w, format+"\n", args...)
}
