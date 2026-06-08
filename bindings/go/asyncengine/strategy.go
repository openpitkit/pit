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
// Please see https://github.com/openpitkit and the OWNERS file for details.

package asyncengine

import (
	"context"
	"sync"
	"sync/atomic"
	"time"

	"go.openpit.dev/openpit/param"
)

const (
	defaultQueueCapacity       = 1024
	defaultSlowSubmitThreshold = time.Minute
	defaultIdleCleanupAfter    = 5 * time.Minute
	defaultIdleCleanupPeriod   = time.Minute
)

// pendingTask is the unit of work queued by the strategy. Exactly one of
// run or abort runs over the task lifetime: run is invoked when the
// strategy decides to execute the task normally; abort is invoked when
// the strategy aborts a queued task (typically on hard stop) or when the
// submit itself fails on the caller goroutine.
//
// Each public operation implements pendingTask with a small concrete value
// carrying its future pointer and the handles it needs, so the hot submit
// path allocates the task once instead of capturing a pair of per-call
// run/abort closures. Implementations must resolve their future exactly
// once across run and abort combined.
type pendingTask interface {
	run()
	abort(err error)
}

// queuedTask wraps a pending task with the metadata needed by observer
// callbacks.
type queuedTask struct {
	task       pendingTask
	accountID  param.AccountID
	enqueuedAt time.Time
}

// keyQueue is a single per-account-or-shard channel-backed queue. The
// gate serializes producers against the cleanup path that retires idle
// queues: Dynamic producers hold an RLock while sending and cleanup holds
// the WLock while closing. Sharded queues are never retired, so the
// Sharded send path does not touch the gate.
//
// pending counts tasks that have been enqueued but not yet fully handled
// by the worker (still buffered in ch or currently running). Dynamic
// producers increment it under gate.RLock before the send and undo the
// bump if the send fails; the worker decrements it after the task
// completes. Bumping before the send means cleanup can never observe a
// false-idle queue mid-send. Idle cleanup reads pending under gate.WLock
// and refuses to retire a queue with pending != 0, which is what keeps a
// queue alive across a long-running task even after its channel has
// drained. Sharded queues do not track it.
type keyQueue struct {
	ch         chan queuedTask
	quit       chan struct{}
	gate       sync.RWMutex
	closed     atomic.Bool
	lastActive atomic.Int64
	pending    atomic.Int64
}

func newKeyQueue(capacity int) *keyQueue {
	q := &keyQueue{
		ch:   make(chan queuedTask, capacity),
		quit: make(chan struct{}),
	}
	q.touch()
	return q
}

func (q *keyQueue) touch() {
	q.lastActive.Store(time.Now().UnixNano())
}

func (q *keyQueue) lastActiveAt() time.Time {
	return time.Unix(0, q.lastActive.Load())
}

// strategy is the internal interface that dispatches a task to a worker
// goroutine bound to the given account. The chosen strategy is selected
// at build time.
type strategy interface {
	submit(ctx context.Context, accountID param.AccountID, task pendingTask) error

	stopGraceful(ctx context.Context) error
	stopHard(ctx context.Context) error
}

// baseConfig is the configuration shared by every concrete strategy.
type baseConfig struct {
	observer            Observer
	queueCapacity       int
	slowSubmitThreshold time.Duration
}

// base implements the producer/worker/stop logic common to every
// strategy. Concrete strategies own the routing of accountID -> *keyQueue.
type base struct {
	cfg             baseConfig
	inFlightSubmits sync.WaitGroup
	// submitMu orders inFlightSubmits.Add against stop so Add happens-before
	// Wait. beginSubmit takes the read side so concurrent submits proceed in
	// parallel; signalStopOnce takes the write side around close(stopCh).
	// RWMutex orders each RLock wholly before or wholly after the Lock: a
	// submit that runs after the close observes isStopped and skips Add; a
	// submit that runs before completes its Add before the close, so the
	// post-stop inFlightSubmits.Wait still observes the matching Done.
	submitMu sync.RWMutex
	// observerActive is false when cfg.observer is the shared no-op, letting
	// the hot path skip the per-task timestamps the observer would discard.
	observerActive bool
	// tracksIdle is true only for strategies whose queues can be retired by
	// idle cleanup (Dynamic). When set, the send and worker paths maintain
	// q.lastActive and q.pending; Sharded leaves both untouched.
	tracksIdle   bool
	workersWG    sync.WaitGroup
	stopCh       chan struct{}
	hardStopCh   chan struct{}
	stopOnce     sync.Once
	hardStopOnce sync.Once
}

func newBase(cfg baseConfig, tracksIdle bool) base {
	if cfg.observer == nil {
		cfg.observer = noopObserver
	}
	if cfg.queueCapacity <= 0 {
		cfg.queueCapacity = defaultQueueCapacity
	}
	if cfg.slowSubmitThreshold <= 0 {
		cfg.slowSubmitThreshold = defaultSlowSubmitThreshold
	}
	return base{
		cfg:            cfg,
		observerActive: cfg.observer != noopObserver,
		tracksIdle:     tracksIdle,
		stopCh:         make(chan struct{}),
		hardStopCh:     make(chan struct{}),
	}
}

func (b *base) isStopped() bool {
	select {
	case <-b.stopCh:
		return true
	default:
		return false
	}
}

func (b *base) hardStopped() bool {
	select {
	case <-b.hardStopCh:
		return true
	default:
		return false
	}
}

// beginSubmit registers an in-flight submit so that its channel send
// happens-before any closeQueueChannels. It returns false when the
// strategy has already been signalled to stop, in which case the caller
// must not send and must not call inFlightSubmits.Done.
func (b *base) beginSubmit() bool {
	b.submitMu.RLock()
	defer b.submitMu.RUnlock()
	if b.isStopped() {
		return false
	}
	b.inFlightSubmits.Add(1)
	return true
}

// submitToQueue enqueues task into a Dynamic per-account queue. It returns
// nil once the task is queued, ctx.Err() if the caller's context expires
// first, ErrStopped if the strategy has been stopped, or errQueueRetired
// if q was retired by idle cleanup (the caller is expected to retry with a
// freshly created queue).
//
// Producers hold q.gate.RLock for the duration of the send. Cleanup
// acquires q.gate.WLock to retire a queue; the WLock therefore waits until
// every in-flight send has either completed or returned.
func (b *base) submitToQueue(
	ctx context.Context,
	q *keyQueue,
	accountID param.AccountID,
	task pendingTask,
) error {
	if !b.beginSubmit() {
		return ErrStopped
	}
	defer b.inFlightSubmits.Done()

	q.gate.RLock()
	defer q.gate.RUnlock()
	if q.closed.Load() {
		return errQueueRetired
	}
	return b.sendToQueue(ctx, q, accountID, task, q.quit)
}

// submitToShard enqueues task into a Sharded queue. Sharded queues are
// never retired by cleanup, so there is no gate and no quit case: a closed
// channel is reachable only after stop, and closeQueueChannels closes a
// channel only once every in-flight submit registered by beginSubmit has
// drained, so no send-on-closed-channel is possible.
func (b *base) submitToShard(
	ctx context.Context,
	q *keyQueue,
	accountID param.AccountID,
	task pendingTask,
) error {
	if !b.beginSubmit() {
		return ErrStopped
	}
	defer b.inFlightSubmits.Done()
	return b.sendToQueue(ctx, q, accountID, task, nil)
}

// sendToQueue is the gate-free core send loop shared by both strategies.
// quit may be nil (Sharded); a nil channel disables the retired case. The
// caller is responsible for registering the in-flight submit and, for the
// Dynamic path, for holding q.gate.RLock and pre-checking q.closed.
func (b *base) sendToQueue(
	ctx context.Context,
	q *keyQueue,
	accountID param.AccountID,
	task pendingTask,
	quit <-chan struct{},
) error {
	// Honor an already-cancelled ctx before any enqueue: the fast-path send
	// below would otherwise sneak a task in when the queue has space, even
	// though the producer's ctx is already done.
	if err := ctx.Err(); err != nil {
		b.cfg.observer.OnSubmitCancelled(accountID, err)
		return err
	}

	qt := queuedTask{task: task, accountID: accountID}
	if b.observerActive {
		qt.enqueuedAt = time.Now()
	}

	// pending must be bumped before the send so cleanup never observes a
	// false-idle queue. A failed send undoes the bump on the way out.
	if b.tracksIdle {
		q.pending.Add(1)
	}

	// Fast path: try a non-blocking send first.
	select {
	case q.ch <- qt:
		b.markSent(q)
		b.cfg.observer.OnEnqueue(accountID, len(q.ch))
		return nil
	default:
	}

	// Slow path: wait with periodic slow-submit notifications. The ticker
	// only drives observer callbacks, so skip it when the observer is
	// inactive: a nil tick channel never fires in the select below.
	start := time.Now()
	attempt := 0
	var tick <-chan time.Time
	if b.observerActive {
		ticker := time.NewTicker(b.cfg.slowSubmitThreshold)
		defer ticker.Stop()
		tick = ticker.C
	}
	for {
		select {
		case q.ch <- qt:
			b.markSent(q)
			b.cfg.observer.OnEnqueue(accountID, len(q.ch))
			return nil
		case <-quit:
			b.unmarkSent(q)
			return errQueueRetired
		case <-ctx.Done():
			b.unmarkSent(q)
			b.cfg.observer.OnSubmitCancelled(accountID, ctx.Err())
			return ctx.Err()
		case <-b.stopCh:
			b.unmarkSent(q)
			return ErrStopped
		case <-tick:
			elapsed := time.Since(start)
			attempt++
			b.cfg.observer.OnQueueFullBlocked(accountID, elapsed)
			b.cfg.observer.OnSlowSubmit(accountID, elapsed, attempt)
		}
	}
}

// worker drains a queue's channel. It exits when q.ch is closed (via
// closeQueueChannels during stop) or when q.quit is closed (via idle
// cleanup retiring the queue).
//
// When hard stop is active, every task is aborted with ErrStopped rather
// than executed.
func (b *base) worker(q *keyQueue) {
	defer b.workersWG.Done()
	for {
		select {
		case qt, ok := <-q.ch:
			if !ok {
				return
			}
			b.handleTask(q, qt)
		case <-q.quit:
			b.drainAndAbort(q)
			return
		}
	}
}

// handleTask runs (or, under hard stop, aborts) a single dequeued task. For
// idle-tracking strategies it decrements q.pending only after the task has
// been fully handled, so cleanup observing q.pending != 0 cannot retire the
// queue while the task is still running - even though its channel is empty.
func (b *base) handleTask(q *keyQueue, qt queuedTask) {
	if b.tracksIdle {
		defer q.pending.Add(-1)
	}
	if b.hardStopped() {
		qt.task.abort(ErrStopped)
		b.cfg.observer.OnComplete(qt.accountID, 0)
		return
	}
	if !b.observerActive {
		qt.task.run()
		if b.tracksIdle {
			q.touch()
		}
		return
	}
	b.cfg.observer.OnDequeue(qt.accountID, time.Since(qt.enqueuedAt))
	started := time.Now()
	qt.task.run()
	b.cfg.observer.OnComplete(qt.accountID, time.Since(started))
	if b.tracksIdle {
		q.touch()
	}
}

// drainAndAbort empties any tasks left in q.ch and aborts them. Used when
// a queue is retired by cleanup: cleanup verified empty under WLock, but
// a producer that grabbed q.gate.RLock before WLock could have raced in
// a send. We drain defensively to avoid losing tasks.
func (b *base) drainAndAbort(q *keyQueue) {
	for {
		select {
		case qt, ok := <-q.ch:
			if !ok {
				return
			}
			if b.tracksIdle {
				q.pending.Add(-1)
			}
			qt.task.abort(ErrStopped)
			b.cfg.observer.OnComplete(qt.accountID, 0)
		default:
			return
		}
	}
}

// waitInFlightSubmits blocks until every in-flight submitToQueue has
// returned, or ctx fires.
func (b *base) waitInFlightSubmits(ctx context.Context) error {
	done := make(chan struct{})
	go func() {
		b.inFlightSubmits.Wait()
		close(done)
	}()
	select {
	case <-done:
		return nil
	case <-ctx.Done():
		return ctx.Err()
	}
}

// waitWorkers blocks until every worker goroutine has exited, or ctx
// fires.
func (b *base) waitWorkers(ctx context.Context) error {
	done := make(chan struct{})
	go func() {
		b.workersWG.Wait()
		close(done)
	}()
	select {
	case <-done:
		return nil
	case <-ctx.Done():
		return ctx.Err()
	}
}

// closeQueueChannels closes q.ch on every queue in queues so that workers
// drain and exit naturally. The caller must guarantee no producer is
// holding q.gate.RLock at the time of the call (typically by first
// awaiting inFlightSubmits).
func (*base) closeQueueChannels(queues []*keyQueue) {
	for _, q := range queues {
		q.gate.Lock()
		if q.closed.CompareAndSwap(false, true) {
			close(q.ch)
		}
		q.gate.Unlock()
	}
}

// signalStopOnce closes stopCh (idempotent). After this call new
// submitToQueue invocations short-circuit with ErrStopped, and any
// in-flight submitter waiting on q.gate.RLock returns ErrStopped.
func (b *base) signalStopOnce() {
	b.stopOnce.Do(func() {
		b.submitMu.Lock()
		close(b.stopCh)
		b.submitMu.Unlock()
	})
}

// markSent records that a task was accepted into q after a successful send.
// q.pending was already bumped before the send (see sendToQueue), so this
// only refreshes activity; Sharded leaves q untouched.
func (b *base) markSent(q *keyQueue) {
	if b.tracksIdle {
		q.touch()
	}
}

// unmarkSent undoes the pre-send q.pending bump when the send did not place
// a task into q.ch (ctx expiry, stop, or queue retirement). Dynamic callers
// invoke it under q.gate.RLock so the decrement is ordered against cleanup's
// WLock read; Sharded leaves q.pending untouched.
func (b *base) unmarkSent(q *keyQueue) {
	if b.tracksIdle {
		q.pending.Add(-1)
	}
}

// signalHardStopOnce closes hardStopCh (idempotent). Workers picking up a
// task after this point invoke task.abort instead of task.run.
func (b *base) signalHardStopOnce() {
	b.hardStopOnce.Do(func() { close(b.hardStopCh) })
}
