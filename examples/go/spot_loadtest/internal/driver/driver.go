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
	"errors"
	"fmt"
	"sync"
	"time"

	openpit "go.openpit.dev/openpit"
	"go.openpit.dev/openpit/asyncengine"
	"go.openpit.dev/openpit/model"

	"openpit-loadtest-spot-funds-go/internal/config"
	"openpit-loadtest-spot-funds-go/internal/generator"
	"openpit-loadtest-spot-funds-go/internal/measurement"
)

// ErrBackpressureInvalidRun is returned by Run when the run drained cleanly
// (no context cancellation, oracle, transport, or invariant error) BUT one or
// more submits hit the dispatch capacity cap (asyncengine.ErrQueueLimit). Such
// a run is NOT a valid latency measurement: the engine refused those ops while
// the shadow oracle applied them offline, so later same-account ops diverge and
// the headline would be computed over an incomplete, skewed sample. The caller
// must suppress the headline and exit non-zero; Run still returns the Stats and
// Snapshot alongside this error so the caller can print non-latency diagnostics.
var ErrBackpressureInvalidRun = errors.New(
	"driver: run hit dispatch backpressure (ErrQueueLimit); not a valid latency measurement")

// ErrZeroChecksumInvalidRun is returned by Run when the run drained cleanly and
// resolved a non-empty set of order-class ops, settlements, or fundings BUT the
// anti-DCE checksum is zero. A zero checksum on a non-empty run means the
// per-decision fold was elided (or no decision was provably consumed), so the
// measurement cannot be trusted: it is treated as INVALID exactly like
// backpressure. The caller must suppress the headline and exit non-zero; Run
// still returns the Stats and Snapshot for the non-latency diagnostics.
var ErrZeroChecksumInvalidRun = errors.New(
	"driver: anti-DCE checksum is zero on a non-empty run; decisions were not provably consumed; not a valid latency measurement")

// stopTimeout bounds the graceful dispatcher drain and engine stop at the end of
// a run. By the time it is used every future has already resolved (the collectors
// drained the work channel) and every accepted reservation has been finalized
// (the finalizers drained the finalize channel), so this is only a safety bound
// on the dispatcher teardown.
const stopTimeout = 30 * time.Second

// defaultOverheadProbes is the number of empty-submit probes run before the
// workload to characterise harness self-overhead (see MeasureOverhead).
const defaultOverheadProbes = 200

// defaultCollectors / defaultFinalizers size the collector and finalizer pools
// when the config leaves them unset. The collector pool awaits resolved futures
// and must keep up with the open-loop in-flight depth so resolve instants stay
// accurate; the finalizer pool drains accepted reservations (CommitAndClose) off
// the measured path.
const (
	defaultCollectors = 16
	defaultFinalizers = 16
)

// finalizeBuffer is the capacity of the FAST channel feeding the finalizer pool.
// It is generous so the collector almost never overflows handing off an accepted
// reservation. The handoff is NON-BLOCKING: when this fast path is full the
// collector pushes to an unbounded overflow (finalizers also drain it) so the
// submit schedule is never throttled by the off-path finalize backlog, and the
// overflow use is counted as a HARNESS handoff stall DIAGNOSTIC (see
// handOffFinalize) — it does not contaminate the headline or invalidate the run.
const finalizeBuffer = 8192

// overflow is an unbounded FIFO used as the spill path behind a bounded fast
// channel so a momentarily-full buffer never blocks the producer. The submit
// schedule must be free to keep pacing to VirtualT0 regardless of how far behind
// the consumers are, so neither the submitter -> collector handoff nor the
// collector -> finalizer handoff may ever block; the overflow absorbs the spill
// and the consumers drain it. It is bounded in practice by the in-flight depth
// (each spilled item corresponds to one submitted-but-unconsumed op).
//
// It is safe for concurrent producers and consumers.
type overflow[T any] struct {
	mu    sync.Mutex
	items []T
}

// push appends one item and returns the new length. Never blocks (grows the
// slice as needed). The length is returned under the same mutex hold so the
// caller can track the peak depth without an extra lock round-trip.
func (o *overflow[T]) push(item T) int {
	o.mu.Lock()
	o.items = append(o.items, item)
	n := len(o.items)
	o.mu.Unlock()
	return n
}

// pop removes and returns the oldest item, or ok=false when empty.
func (o *overflow[T]) pop() (item T, ok bool) {
	o.mu.Lock()
	defer o.mu.Unlock()
	if len(o.items) == 0 {
		return item, false
	}
	item = o.items[0]
	// Shift the window forward rather than reslice-from-0 so the backing array is
	// reused; when fully drained, drop the array so memory is released.
	o.items = o.items[1:]
	if len(o.items) == 0 {
		o.items = nil
	}
	return item, true
}

// Config tunes one driver run. Zero values are filled with safe defaults so a
// test can pass an almost-empty Config.
type Config struct {
	// Collectors is the size of the goroutine pool draining resolved futures.
	// Zero defaults to defaultCollectors.
	Collectors int
	// Finalizers is the size of the goroutine pool finalizing accepted
	// reservations (CommitAndClose) off the measured path. Zero defaults to
	// defaultFinalizers.
	Finalizers int
	// Observer enables the asyncengine diagnostic observer wiring point.
	Observer bool

	// ActiveAccounts is the offered active working set (informational at the
	// driver level). In the open-loop driver the bound on how many accounts are
	// concurrently LIVE is governed by the virtual schedule itself: the generator
	// only ever has ~ActiveAccounts distinct accounts active within any short
	// virtual-time window, so the engine's live per-account dispatch queues track
	// the active set. It is retained for disclosure/reporting; the driver does not
	// gate submission on it (gating would defeat open-loop pacing).
	ActiveAccounts uint64

	// --- Async engine dispatch strategy ---

	// DispatchStrategy selects dynamic (default) or sharded dispatch. Zero value
	// (dispatchDynamic) is the locked design default.
	DispatchStrategy dispatchStrategy

	// --- Dynamic-only dispatch knobs ---

	// MaxQueues is the Dynamic dispatch capacity passed to the engine builder
	// (0 = unlimited). Sized to hold the active working set with margin; the
	// ErrQueueLimit backpressure path surfaces any breach.
	MaxQueues uint64
	// IdleCleanup is the per-account queue retire delay passed to the engine
	// builder. Zero disables cleanup.
	IdleCleanup time.Duration

	// --- Sharded-only dispatch knobs ---

	// ShardedWorkers is the number of fixed shards. Required > 0 when
	// DispatchStrategy = dispatchSharded.
	ShardedWorkers int

	// --- Shared dispatch knobs (both strategies) ---

	// QueueCapacity is the buffered channel size of each queue. 0 = engine default (1024).
	QueueCapacity int
	// SlowSubmitThreshold is the slow-submit observer threshold. 0 = engine default (1m).
	SlowSubmitThreshold time.Duration

	// WindowSize is the number of order-check ops per measurement window.
	// Zero uses a default of 10 000 ops.
	WindowSize int64
	// WindowUnit selects ops-count (default) or wall-clock windowing.
	WindowUnit measurement.WindowUnit
	// WallWindow is the wall-clock window duration when WindowUnit = wall.
	WallWindow time.Duration

	// OverheadProbes is the number of empty-submit probes to run before the
	// workload for the harness self-overhead characterisation. Zero disables.
	OverheadProbes int

	// Live, when non-nil, is populated by Run before any goroutine starts.
	// The progress reporter may call Live.Counters() concurrently at any point
	// after the sink is created (the first call before Run stores the accessor
	// returns zero counters, not a panic). Use a *LiveSource to share it
	// safely between the caller and the progress goroutine.
	Live *LiveSource
}

// FromAppConfig derives the driver Config from the validated app config.
func FromAppConfig(cfg *config.Config) Config {
	unit := measurement.WindowUnitOps
	if cfg.Run.WindowUnit == config.WindowUnitWall {
		unit = measurement.WindowUnitWall
	}
	windowSize := int64(cfg.Run.Window) //nolint:gosec // window <= 2^31 in practice
	if windowSize <= 0 {
		windowSize = 10_000
	}

	strategy := dispatchDynamic
	if cfg.AsyncEngine.Strategy == config.AsyncEngineStrategySharded {
		strategy = dispatchSharded
	}

	return Config{
		Observer:            cfg.Run.Observer,
		ActiveAccounts:      cfg.Concurrency.ActiveAccounts,
		DispatchStrategy:    strategy,
		MaxQueues:           cfg.AsyncEngine.MaxQueues,
		IdleCleanup:         cfg.AsyncEngine.IdleCleanup,
		ShardedWorkers:      cfg.AsyncEngine.ShardedWorkers,
		QueueCapacity:       cfg.AsyncEngine.QueueCapacity,
		SlowSubmitThreshold: cfg.AsyncEngine.SlowSubmitThreshold,
		WindowSize:          windowSize,
		WindowUnit:          unit,
		OverheadProbes:      defaultOverheadProbes,
	}
}

// Run drives the whole generator stream through a freshly built engine, true
// open-loop, and returns measured stats and a full measurement Snapshot once
// every operation has resolved and the engine has stopped. It returns an error
// if the per-op oracle diverges, an aggregate invariant breaks, or the engine
// cannot be driven.
//
// Goroutine model (true open-loop, see package doc):
//
//   - one lightweight submitter goroutine per account owns that account's
//     ordered sub-stream. It paces each event by sleeping until the event's
//     VirtualT0 (relative to run start), stamps that virtual time as the measured
//     t0, submits non-blocking, hands the future to the collector via a
//     NON-BLOCKING handoff (fast buffered channel with an unbounded overflow the
//     collectors also drain), and IMMEDIATELY advances to the next event. It
//     NEVER awaits a future, never waits for a decision, and NEVER blocks on a
//     harness-internal handoff, so the open-loop submit schedule is never
//     throttled by the harness; many of one account's ops may be in flight at
//     once (true pipelining);
//   - a collector goroutine pool awaits every future, records the open-loop
//     latency = resolve - VirtualT0 (the headline) plus the service-time
//     diagnostic = resolve - actual submit, runs the oracle, and hands accepted
//     reservations to the finalizer pool;
//   - a finalizer goroutine pool drains accepted reservations and CommitAndCloses
//     them (commit = no-op, close releases the native handle, keeping the in-place
//     hold) strictly off the measured path; the collector hands them off
//     NON-BLOCKING (fast buffered channel with an unbounded overflow the
//     finalizers also drain) so it never stops draining the work handoff, and a
//     full finalize fast path is counted as a HARNESS handoff stall DIAGNOSTIC
//     (off the measured path; it does not throttle the schedule or invalidate
//     the run);
//   - per-account ordering (a top-up before its order, an order-check before its
//     settlement, a settlement before a dependent later order) is preserved by
//     submitting each account's chain from a single goroutine in causal order; the
//     engine's FIFO-per-account dispatch then replays the shadow's offline-ordered
//     decisions exactly, so the strict per-op oracle holds without any per-op
//     blocking.
func Run(ctx context.Context, stream *generator.Stream, cfg Config) (Stats, measurement.Snapshot, error) {
	if stream == nil {
		return Stats{}, measurement.Snapshot{}, fmt.Errorf("driver: nil stream")
	}
	obs, obsSink := newObserver(cfg.Observer)
	engine, syncEngine, stop, err := buildEngine(obs.asObserver(), engineDispatch{
		Strategy:            cfg.DispatchStrategy,
		MaxQueues:           int(cfg.MaxQueues), //nolint:gosec // MaxQueues is a configured dispatch cap well below int max
		IdleCleanup:         cfg.IdleCleanup,
		ShardedWorkers:      cfg.ShardedWorkers,
		QueueCapacity:       cfg.QueueCapacity,
		SlowSubmitThreshold: cfg.SlowSubmitThreshold,
	})
	if err != nil {
		return Stats{}, measurement.Snapshot{}, err
	}

	collectors := cfg.Collectors
	if collectors <= 0 {
		collectors = defaultCollectors
	}
	finalizers := cfg.Finalizers
	if finalizers <= 0 {
		finalizers = defaultFinalizers
	}
	windowSize := cfg.WindowSize
	if windowSize <= 0 {
		windowSize = 10_000
	}

	windows := measurement.NewWindows(cfg.WindowUnit, windowSize, cfg.WallWindow)

	sink := newResultSink(windows)
	// Publish the live-counter accessor to the LiveSource before any goroutine
	// starts. The caller may have already started a progress goroutine that
	// calls LiveSource.Counters(); the atomic store ensures it observes the real
	// accessor rather than the zero-value (which returns empty counters).
	if cfg.Live != nil {
		cfg.Live.store(sink.live)
	}

	r := &run{
		ctx:      ctx,
		cfg:      cfg,
		engine:   engine,
		oracle:   newOracle(),
		sink:     sink,
		work:     make(chan inflight, collectors*4),
		finalize: make(chan *asyncengine.AsyncReservation, finalizeBuffer),
		windows:  windows,
	}

	// Seeding is SETUP, not measured load: apply the initial per-account balance
	// seeds synchronously on the underlying engine BEFORE the async run, instead
	// of herding thousands of adjustments through the measured async pipeline.
	// The shadow oracle still verifies each seed's predicted post-balance.
	if err := r.applySeeds(syncEngine, stream.Events); err != nil {
		stop(ctx)
		return Stats{}, measurement.Snapshot{}, err
	}

	// Run harness self-overhead probe before the workload starts. The probe
	// submits trivial ops through the same async path so the measurement covers
	// the real FFI+queue overhead.
	var overhead measurement.OverheadSummary
	if cfg.OverheadProbes > 0 {
		overhead, err = measurement.MeasureOverhead(ctx, cfg.OverheadProbes, r.overheadProbe)
		if err != nil {
			stop(ctx)
			return Stats{}, measurement.Snapshot{}, fmt.Errorf("driver: overhead probe: %w", err)
		}
	}

	// Partition the stream per account, preserving each account's causal order.
	// Settlements are now part of the chains (the scheduler submits them at their
	// virtual arrival time); seeds are excluded (applied synchronously above).
	chains := partitionChains(stream.Events)

	// Start the collector pool.
	var collectorWG sync.WaitGroup
	collectorWG.Add(collectors)
	for i := 0; i < collectors; i++ {
		go func() {
			defer collectorWG.Done()
			r.collect()
		}()
	}

	// Start the finalizer pool (CommitAndClose accepted reservations off the
	// measured path).
	var finalizerWG sync.WaitGroup
	finalizerWG.Add(finalizers)
	for i := 0; i < finalizers; i++ {
		go func() {
			defer finalizerWG.Done()
			r.finalizeLoop()
		}()
	}

	// Start one submitter per account. Each paces its account's events by their
	// virtual arrival times, open-loop, from a shared run start.
	start := time.Now()
	var submitWG sync.WaitGroup
	for _, chain := range chains {
		submitWG.Add(1)
		go func(events []*generator.Event) {
			defer submitWG.Done()
			r.submitChain(start, events)
		}(chain)
	}

	submitWG.Wait()
	// All submitters done: no more futures will be handed to the collectors.
	close(r.work)
	collectorWG.Wait()
	// All futures awaited and recorded: no more reservations will be handed to
	// the finalizers.
	close(r.finalize)
	finalizerWG.Wait()

	// Shutdown: drain the dispatcher gracefully, then stop the engine.
	stopCtx, cancel := context.WithTimeout(context.Background(), stopTimeout)
	defer cancel()
	stop(stopCtx)

	if err := ctx.Err(); err != nil {
		snap := measurement.Build(windows, r.sink.m, obsSink, overhead)
		return r.sink.stats(), snap, fmt.Errorf("driver: run cancelled: %w", err)
	}
	if err := r.oracle.Err(); err != nil {
		snap := measurement.Build(windows, r.sink.m, obsSink, overhead)
		return r.sink.stats(), snap, err
	}
	if err := r.oracle.checkInvariants(stream.Events); err != nil {
		snap := measurement.Build(windows, r.sink.m, obsSink, overhead)
		return r.sink.stats(), snap, err
	}

	snap := measurement.Build(windows, r.sink.m, obsSink, overhead)
	stats := r.sink.stats()
	// Methodology invariants: a run is published ONLY when it is a valid latency
	// measurement. The checks below are ordered by precedence; real
	// ctx/oracle/transport/invariant errors above already took precedence.
	//
	//  1. Backpressure (ErrQueueLimit): the engine refused submits, so the sample
	//     is incomplete and skewed.
	//  2. Zero checksum on a non-empty run: decisions were not provably consumed
	//     (anti-DCE), so the measurement cannot be trusted.
	//
	// A harness handoff stall (collector -> finalizer fast-path overflow) is NOT
	// an invalidity trigger: the handoff is non-blocking, so a finalize-overflow
	// spill never throttles the open-loop submit schedule and never contaminates
	// the headline (CommitAndClose is fully off the measured path — the latency
	// was already recorded at resolve). HandoffStalls is reported as a diagnostic
	// only (see Snapshot / reporter), never folded into validity.
	//
	// In every invalid case the Stats and Snapshot are still returned so the
	// caller can print the relevant counts and the other non-latency diagnostics;
	// the caller suppresses the headline and exits non-zero.
	if stats.Backpressure > 0 {
		return stats, snap, ErrBackpressureInvalidRun
	}
	resolvedOps := stats.OrderChecks + stats.Settlements + stats.Fundings
	if resolvedOps > 0 && stats.Checksum == 0 {
		return stats, snap, ErrZeroChecksumInvalidRun
	}
	return stats, snap, nil
}

// run holds the shared state for one driver invocation.
type run struct {
	ctx     context.Context
	cfg     Config
	engine  *asyncengine.AsyncEngine
	oracle  *oracle
	sink    *resultSink
	windows *measurement.Windows

	// work is the FAST submitter -> collector handoff. The submit path NEVER
	// blocks on it: when it is full the submitter spills to workOverflow (which
	// collectors also drain), so the open-loop submit schedule keeps pacing to
	// VirtualT0 regardless of how far behind the collectors are. A full work
	// channel is most often caused by collectors legitimately blocked in fut.Await
	// because the ENGINE is slow — that is REAL latency folded into the headline,
	// NOT a harness stall, so spilling here is deliberately NOT witnessed.
	work         chan inflight
	workOverflow overflow[inflight]

	// finalize is the FAST collector -> finalizer handoff; finalizers
	// CommitAndClose accepted reservations off the measured path. The collector
	// NEVER blocks on it: when it is full the collector spills to
	// finalizeOverflow (finalizers also drain it). Spilling here is counted as a
	// HARNESS handoff stall DIAGNOSTIC (RecordHandoffStall): the finalize backlog is
	// purely harness-internal (CommitAndClose throughput) and off the measured path,
	// so a full finalize fast path never throttles the submit schedule and does NOT
	// invalidate the run — it is reported as a diagnostic only.
	finalize         chan *asyncengine.AsyncReservation
	finalizeOverflow overflow[*asyncengine.AsyncReservation]
}

// inflight is one submitted operation handed from a submitter to the collector.
// The submitter never blocks on it: after submitting, it advances immediately to
// the next event (true open-loop). intendedT0 is the event's VirtualT0 mapped to
// an absolute instant (run start + VirtualT0); the collector subtracts it from
// the resolve instant to get the open-loop headline latency. actualSubmit is the
// real wall-clock instant of the submit call; the collector subtracts it from
// the resolve instant for the service-time diagnostic.
type inflight struct {
	event        *generator.Event
	intendedT0   time.Time
	actualSubmit time.Time

	// Exactly one of these future kinds is set, matching event.Kind.
	orderFut   *orderFuture
	settleFut  *settleFuture
	fundingFut *fundingFuture
}

// submitChain runs one account's ordered events, true open-loop. For each event
// it sleeps until the event's virtual arrival (start + VirtualT0), stamps that
// virtual time as the measured t0, submits non-blocking, hands the future to the
// collector, and IMMEDIATELY advances - it never awaits a future or waits for a
// decision before the next submit. Settlements and runtime funding are submitted
// here at their own virtual times; seeds are applied synchronously before the run
// (partitionChains already excludes them).
func (r *run) submitChain(start time.Time, events []*generator.Event) {
	for _, ev := range events {
		if r.ctx.Err() != nil {
			return
		}
		deadline := start.Add(ev.VirtualT0)
		if !sleepUntil(r.ctx, deadline) {
			return // context cancelled while pacing to the virtual arrival
		}
		switch ev.Kind {
		case generator.EventOrderCheck:
			if !r.submitOrder(ev, deadline) {
				return
			}
		case generator.EventSettlement:
			if !r.submitSettlement(ev, deadline) {
				return
			}
		case generator.EventFunding:
			if ev.FundingIsSeed {
				// Seeds are applied synchronously before the run; never re-apply on
				// the async path. partitionChains already excludes them, so this is a
				// defensive guard against a future stream change.
				continue
			}
			if !r.submitFunding(ev, deadline) {
				return
			}
		}
	}
}

// applySeeds applies the initial per-account balance seeds synchronously on the
// underlying engine before the async run, and checks each against the shadow
// oracle's prediction. Seeding is setup, not measured load, so it must never go
// through the measured async pipeline (a 10k-account herd would create thousands
// of live queues at once). The seeds are the first events in the stream, so
// checking them here (single-threaded, before the collectors start) keeps the
// oracle's per-account sequence exact: seed first, then the account's async ops.
func (r *run) applySeeds(syncEngine *openpit.Engine, events []generator.Event) error {
	for i := range events {
		ev := &events[i]
		if ev.Kind != generator.EventFunding || !ev.FundingIsSeed {
			continue
		}
		adj, acc, err := buildAdjustment(ev)
		if err != nil {
			return fmt.Errorf("driver: build seed adjustment (account %s asset %s): %w",
				ev.Account, ev.FundingAsset, err)
		}
		result, err := syncEngine.ApplyAccountAdjustment(acc, []model.AccountAdjustment{adj})
		if err != nil {
			return fmt.Errorf("driver: apply seed (account %s asset %s): %w",
				ev.Account, ev.FundingAsset, err)
		}
		// Verify the seed outcome against the shadow's predicted post-seed balance,
		// exactly as a runtime funding adjustment is checked on the async path.
		r.oracle.checkFunding(ev, fundingObservation{
			rejected: result.BatchError.IsSet(),
			outcomes: result.Outcomes,
		})
		// A rejected seed is a setup failure: surface it loudly rather than
		// starting a run whose accounts cannot trade.
		if result.BatchError.IsSet() {
			return fmt.Errorf("driver: seed rejected for account %s asset %s (setup must succeed)",
				ev.Account, ev.FundingAsset)
		}
	}
	return r.oracle.Err()
}

// submitOrder stamps the virtual arrival as the measured t0, submits
// ExecutePreTrade non-blocking, and hands the future to the collector. It never
// awaits. Returns false only on a build error (a programmer/stream bug surfaced
// loudly). intendedT0 is the absolute virtual arrival instant; actualSubmit is
// the real instant of the submit, captured for the service-time diagnostic.
func (r *run) submitOrder(ev *generator.Event, intendedT0 time.Time) bool {
	order, _, err := buildOrder(ev)
	if err != nil {
		r.oracle.failExternal(fmt.Errorf("driver: build order (account %s corr %d): %w", ev.Account, ev.CorrelationID, err))
		return false
	}
	r.sink.recordSubmit()
	actualSubmit := time.Now()
	fut := r.engine.ExecutePreTrade(r.ctx, order)
	r.handOffWork(inflight{event: ev, intendedT0: intendedT0, actualSubmit: actualSubmit, orderFut: &orderFuture{fut: fut}})
	return true
}

// submitSettlement stamps the virtual arrival as the measured t0, submits
// ApplyExecutionReport non-blocking, and hands the future to the collector. The
// settlement's virtual arrival already includes the report-return delay (baked
// into VirtualT0 by the generator), so no driver-side delay is applied.
func (r *run) submitSettlement(ev *generator.Event, intendedT0 time.Time) bool {
	report, _, err := buildReport(ev)
	if err != nil {
		r.oracle.failExternal(fmt.Errorf("driver: build report (account %s corr %d): %w", ev.Account, ev.CorrelationID, err))
		return false
	}
	r.sink.recordSubmit()
	actualSubmit := time.Now()
	fut := r.engine.ApplyExecutionReport(r.ctx, report)
	r.handOffWork(inflight{event: ev, intendedT0: intendedT0, actualSubmit: actualSubmit, settleFut: &settleFuture{fut: fut}})
	return true
}

// submitFunding stamps the virtual arrival as the measured t0, submits
// ApplyAccountAdjustment non-blocking, and hands the future to the collector.
func (r *run) submitFunding(ev *generator.Event, intendedT0 time.Time) bool {
	adj, acc, err := buildAdjustment(ev)
	if err != nil {
		r.oracle.failExternal(fmt.Errorf("driver: build adjustment (account %s asset %s): %w", ev.Account, ev.FundingAsset, err))
		return false
	}
	r.sink.recordSubmit()
	actualSubmit := time.Now()
	fut := r.engine.ApplyAccountAdjustment(r.ctx, acc, []model.AccountAdjustment{adj})
	r.handOffWork(inflight{event: ev, intendedT0: intendedT0, actualSubmit: actualSubmit, fundingFut: &fundingFuture{fut: fut}})
	return true
}

// handOffWork hands one submitted op to the collector pool WITHOUT EVER BLOCKING
// the submit schedule. It tries the fast buffered channel first; if that buffer
// is momentarily full it spills to the unbounded workOverflow, which collectors
// also drain. The submit loop therefore keeps pacing to each event's VirtualT0
// no matter how far behind the collectors are.
//
// A full work buffer is almost always caused by collectors legitimately blocked
// in fut.Await because the ENGINE is slow under load. That is REAL engine
// latency and is correctly folded into the open-loop headline (resolve -
// VirtualT0) once each future resolves, so spilling here is NOT a harness stall
// and is deliberately NOT witnessed (witnessing it would flag honest engine
// latency as a harness defect). The harness-starvation witness lives on the
// collector -> finalizer handoff instead (see handOffFinalize).
//
// DIAGNOSTIC: when we spill, we track the running peak spill depth via
// recordWorkOverflowDepth. A large peak means collectors lagged submission —
// usually because they were legitimately blocked in fut.Await (real engine
// latency, correctly in the headline), but under host CPU starvation it can
// include collector-dispatch delay that inflates the tail. This is NOT a stall
// and does NOT invalidate the run; it is surfaced as a diagnostic only.
func (r *run) handOffWork(item inflight) {
	select {
	case r.work <- item:
	default:
		depth := r.workOverflow.push(item)
		r.sink.recordWorkOverflowDepth(depth)
	}
}

// collect is one collector goroutine: it awaits each future, records the
// open-loop latency (and the service-time diagnostic for order-checks), runs the
// oracle, and hands accepted order-check reservations to the finalizer pool.
//
// It drains BOTH the fast work channel AND the unbounded workOverflow that the
// submit path spills into when the fast buffer is momentarily full. The overflow
// is drained first on every iteration so its items cannot be stranded and memory
// stays bounded by the in-flight depth. When the submitters close the work
// channel, the closed branch drains any remaining overflow before returning, so
// no submitted op is ever lost and the goroutine always terminates.
func (r *run) collect() {
	for {
		// Drain the spill first so overflowed items cannot be stranded behind the
		// fast channel and the backing array is released as it empties.
		if item, ok := r.workOverflow.pop(); ok {
			r.collectOne(item)
			continue
		}
		item, ok := <-r.work
		if !ok {
			// Submitters are done and the fast channel is closed. Drain any items
			// that spilled to the overflow after our last check, then exit.
			for {
				item, ok := r.workOverflow.pop()
				if !ok {
					return
				}
				r.collectOne(item)
			}
		}
		r.collectOne(item)
	}
}

// collectOne routes one in-flight op to its per-kind collector.
func (r *run) collectOne(item inflight) {
	switch item.event.Kind {
	case generator.EventOrderCheck:
		r.collectOrder(item)
	case generator.EventSettlement:
		r.collectSettlement(item)
	case generator.EventFunding:
		r.collectFunding(item)
	}
}
