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
// the strategy aborts a queued task, typically on hard stop. A submit that
// fails before enqueue is handled by the public operation that submitted it.
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

// submitFailureCleanupTask carries mandatory handle cleanup after the public
// operation's original submit failed. Both worker outcomes run the same
// cleanup: cancellation has already been reported by the original future, and
// this task exists only to preserve lane ordering before releasing the handle.
type submitFailureCleanupTask struct {
	cleanup func()
}

func (t submitFailureCleanupTask) run() { t.cleanup() }

func (t submitFailureCleanupTask) abort(error) { t.cleanup() }

// queuedTask wraps a pending task with the metadata needed by observer
// callbacks.
type queuedTask struct {
	task       pendingTask
	accountID  param.AccountID
	enqueuedAt time.Time
}

// submitFailureCleanupEntry is one queued handle release. admittedFence is
// the value of q.admitted when the entry was queued: the entry runs only once
// q.settled has caught up with it, so the release stays behind lane work that
// was already admitted and cannot be held back by work admitted later.
type submitFailureCleanupEntry struct {
	task            queuedTask
	admittedFence   int64
	waitSubmitFence bool
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
// drained. Mandatory cleanup uses an unbounded per-lane backlog consumed by
// the same worker, so a full channel never blocks its caller and successful
// stop cannot outlive cleanup. Sharded queues do not track idle state.
//
// admitted and settled are the monotonic pair that orders that backlog:
// admitted counts every task reserved for ch, settled counts every one of
// them that has been handled or abandoned. Both strategies maintain them,
// because a shard carries several accounts and a cleanup there must not wait
// for the whole shard to fall idle.
type keyQueue struct {
	ch   chan queuedTask
	quit chan struct{}
	// done is closed when this lane's worker has exited. Production joins
	// workers through workersWG; the per-lane signal exists so package tests
	// can observe the exit of one retired lane.
	done          chan struct{}
	mandatoryWake chan struct{}
	gate          sync.RWMutex
	mandatoryMu   sync.Mutex
	mandatory     []submitFailureCleanupEntry
	// mandatoryProgress is non-nil while mandatory is not empty and is closed
	// and replaced whenever an entry leaves it, so a producer fenced by an
	// entry re-checks instead of waiting for the whole backlog to drain.
	mandatoryProgress chan struct{}
	retired           bool
	workerExited      bool
	closed            atomic.Bool
	lastActive        atomic.Int64
	pending           atomic.Int64
	admitted          atomic.Int64
	settled           atomic.Int64
}

func newKeyQueue(capacity int) *keyQueue {
	q := &keyQueue{
		ch:            make(chan queuedTask, capacity),
		quit:          make(chan struct{}),
		done:          make(chan struct{}),
		mandatoryWake: make(chan struct{}, 1),
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

// settle records that a task reserved by admitTask has been handled, aborted,
// or given up on. It is what releases mandatory cleanup queued behind it.
func (q *keyQueue) settle() {
	q.settled.Add(1)
}

// markRetired seals the lane against new mandatory cleanup. It refuses while
// cleanup is still queued, so retirement can never strand a handle release on
// a lane whose worker is about to stop consuming it.
func (q *keyQueue) markRetired() bool {
	q.mandatoryMu.Lock()
	defer q.mandatoryMu.Unlock()
	if len(q.mandatory) != 0 {
		return false
	}
	q.retired = true
	return true
}

// pendingCleanupForLocked returns the first queued cleanup for accountID.
// Cleanup for one account must not fence producers of another, which matters
// for Sharded, where one queue carries many accounts.
func (q *keyQueue) pendingCleanupForLocked(
	accountID param.AccountID,
) (submitFailureCleanupEntry, bool) {
	for _, entry := range q.mandatory {
		if entry.task.accountID == accountID {
			return entry, true
		}
	}
	return submitFailureCleanupEntry{}, false
}

// noteMandatoryProgressLocked wakes producers fenced by the backlog after an
// entry left it.
func (q *keyQueue) noteMandatoryProgressLocked() {
	close(q.mandatoryProgress)
	if len(q.mandatory) == 0 {
		q.mandatoryProgress = nil
		return
	}
	q.mandatoryProgress = make(chan struct{})
}

// strategy is the internal interface that dispatches a task to a worker
// goroutine bound to the given account. The chosen strategy is selected
// at build time.
type strategy interface {
	submit(ctx context.Context, accountID param.AccountID, task pendingTask) error
	scheduleSubmitFailureCleanup(accountID param.AccountID, cleanup func())

	stopGraceful(ctx context.Context) error
	stopHard(ctx context.Context) error
}

// submitFailureHandoffStrategy keeps a failed pre-stop producer registered
// until its caller transfers handle ownership to mandatory cleanup. Concrete
// production strategies implement it; the fallback preserves compatibility
// with narrow test strategies that only implement strategy.
type submitFailureHandoffStrategy interface {
	submitWithFailureHandoff(
		ctx context.Context,
		accountID param.AccountID,
		task pendingTask,
		onFailure func(error),
	) error
}

func submitWithFailureHandoff(
	ctx context.Context,
	s strategy,
	accountID param.AccountID,
	task pendingTask,
	onFailure func(error),
) error {
	if handoffStrategy, ok := s.(submitFailureHandoffStrategy); ok {
		return handoffStrategy.submitWithFailureHandoff(
			ctx, accountID, task, onFailure,
		)
	}
	err := s.submit(ctx, accountID, task)
	if err != nil {
		onFailure(err)
	}
	return err
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
	cfg baseConfig
	// submitMu orders inFlightSubmits increments against stop. beginSubmit
	// takes the read side so concurrent submits proceed in parallel;
	// signalStopOnce takes the write side around close(stopCh).
	// RWMutex orders each RLock wholly before or wholly after the Lock: a
	// submit that runs after the close skips registration; a submit that runs
	// before increments the counter before stop observes it.
	submitMu        sync.RWMutex
	inFlightSubmits atomic.Int64
	submitFenceCh   chan struct{}
	submitFenceOnce sync.Once
	// lifecycleCleanupMu orders no-lane mandatory cleanup registration
	// against the final successful stop. Cleanup that registers first is
	// included in that stop; cleanup after sealing is already post-stop.
	lifecycleCleanupMu     sync.Mutex
	lifecycleCleanupDone   chan struct{}
	lifecycleCleanupCount  int
	lifecycleCleanupSealed bool
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
		submitFenceCh:  make(chan struct{}),
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

// beginSubmit registers an in-flight producer so that its channel send
// happens-before any closeQueueChannels. It returns false when the
// strategy has already been signalled to stop, in which case the caller
// must not send and must not call endSubmit.
func (b *base) beginSubmit() bool {
	b.submitMu.RLock()
	defer b.submitMu.RUnlock()
	if b.isStopped() {
		return false
	}
	b.inFlightSubmits.Add(1)
	return true
}

func (b *base) endSubmit() {
	if b.inFlightSubmits.Add(-1) == 0 && b.isStopped() {
		b.submitFenceOnce.Do(func() { close(b.submitFenceCh) })
	}
}

// submitToRegisteredQueue runs one Dynamic queue attempt under a producer
// obligation already owned by the caller. It releases q.gate before returning,
// so terminal failure handoff can safely register mandatory cleanup on the
// same lane.
func (b *base) submitToRegisteredQueue(
	ctx context.Context,
	q *keyQueue,
	accountID param.AccountID,
	task pendingTask,
) error {
	q.gate.RLock()
	defer q.gate.RUnlock()
	if q.closed.Load() {
		return errQueueRetired
	}
	return b.sendToQueue(ctx, q, accountID, task, q.quit)
}

// submitToShardWithFailureHandoff enqueues task into a Sharded queue. Sharded
// queues are never retired by cleanup, so there is no gate and no quit case: a
// closed channel is reachable only after stop, and closeQueueChannels closes a
// channel only once every in-flight submit registered by beginSubmit has
// drained, so no send-on-closed-channel is possible.
func (b *base) submitToShardWithFailureHandoff(
	ctx context.Context,
	q *keyQueue,
	accountID param.AccountID,
	task pendingTask,
	onFailure func(error),
) error {
	if !b.beginSubmit() {
		if onFailure != nil {
			onFailure(ErrStopped)
		}
		return ErrStopped
	}
	defer b.endSubmit()
	err := b.sendToQueue(ctx, q, accountID, task, nil)
	if err != nil && onFailure != nil {
		onFailure(err)
	}
	return err
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
	if err := b.waitForMandatoryCleanup(ctx, q, accountID, quit); err != nil {
		return err
	}
	// Honor an already-cancelled ctx before any enqueue: the fast-path send
	// below would otherwise sneak a task in when the queue has space, even
	// though the producer's ctx is already done.
	if err := ctx.Err(); err != nil {
		b.cfg.observer.OnSubmitCancelled(accountID, err)
		return err
	}

	qt := queuedTask{
		task:      task,
		accountID: accountID,
	}
	if b.observerActive {
		qt.enqueuedAt = time.Now()
	}

	// The task must be admitted before the send so cleanup never observes a
	// false-idle queue and never runs ahead of a task already on its way in.
	// A failed send settles the admission on the way out.
	b.admitTask(q)

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

// waitForMandatoryCleanup holds a producer back until this account has no
// queued handle release left, so nothing the account submits later overtakes
// it. The fence is per account, not per queue: a Sharded queue carries many
// accounts and one account's cleanup must not stall the others.
func (b *base) waitForMandatoryCleanup(
	ctx context.Context,
	q *keyQueue,
	accountID param.AccountID,
	quit <-chan struct{},
) error {
	for {
		q.mandatoryMu.Lock()
		entry, fenced := q.pendingCleanupForLocked(accountID)
		if !fenced {
			q.mandatoryMu.Unlock()
			return nil
		}
		progress := q.mandatoryProgress
		q.mandatoryMu.Unlock()

		// A stopped-lane cleanup waits for producers that registered before
		// stop, so those producers must be allowed to finish their sends.
		if entry.waitSubmitFence && b.isStopped() {
			return nil
		}
		select {
		case <-progress:
			continue
		case <-quit:
			return errQueueRetired
		case <-ctx.Done():
			b.cfg.observer.OnSubmitCancelled(accountID, ctx.Err())
			return ctx.Err()
		case <-b.stopCh:
			return ErrStopped
		}
	}
}

// worker drains a queue's channel and the mandatory-cleanup backlog that
// shares the lane. Channel work ends when q.ch is closed (via
// closeQueueChannels during stop) or when q.quit is closed (via idle cleanup
// retiring the queue); either way the worker returns only through
// finishWorker, which refuses to seal a lane that still owes a handle
// release. Both endings are latched into a local flag and drop their channel
// from the select, so a channel that stays ready once closed can never spin
// the loop.
//
// When hard stop is active, every task is aborted with ErrStopped rather
// than executed.
func (b *base) worker(q *keyQueue) {
	defer b.workersWG.Done()
	defer close(q.done)
	channelClosed := false
	laneRetired := false
	for {
		qt, mandatoryReady, waitSubmitFence := b.takeMandatoryCleanup(q)
		if mandatoryReady {
			b.handleTask(q, qt)
			continue
		}
		if channelClosed || laneRetired {
			if b.finishWorker(q) {
				return
			}
			// Cleanup is queued but not yet runnable, and no further channel
			// work can reach this lane. Wait for the producer fence that is
			// holding it rather than re-polling the sealed channel.
			b.waitForCleanupRelease(q, waitSubmitFence)
			continue
		}

		var submitFence <-chan struct{}
		if waitSubmitFence {
			submitFence = b.submitFenceCh
		}
		select {
		case qt, ok := <-q.ch:
			if !ok {
				channelClosed = true
				continue
			}
			b.handleTask(q, qt)
			q.settle()
		case <-q.mandatoryWake:
		case <-submitFence:
		case <-q.quit:
			laneRetired = true
			b.drainAndAbort(q)
		}
	}
}

// waitForCleanupRelease parks a worker whose lane is sealed but whose backlog
// is not runnable yet. waitSubmitFence is true only while b.submitFenceCh is
// still open, so neither arm of the select is already ready.
func (b *base) waitForCleanupRelease(q *keyQueue, waitSubmitFence bool) {
	var submitFence <-chan struct{}
	if waitSubmitFence {
		submitFence = b.submitFenceCh
	}
	select {
	case <-q.mandatoryWake:
	case <-submitFence:
	}
}

// enqueueSubmitFailureCleanup transfers cleanup to the lane-owned backlog and
// reports whether the lane took it. It refuses a lane that has been retired
// or whose worker has exited; that refusal is the caller's signal to retry on
// a live lane or to release the handle itself, and it is what keeps the
// release exactly once. waitSubmitFence marks cleanup registered after stop
// was signalled, which must not overtake producers registered before it.
func (b *base) enqueueSubmitFailureCleanup(
	q *keyQueue,
	accountID param.AccountID,
	cleanup func(),
	waitSubmitFence bool,
) bool {
	qt := queuedTask{
		task:      submitFailureCleanupTask{cleanup: cleanup},
		accountID: accountID,
	}
	if b.observerActive {
		qt.enqueuedAt = time.Now()
	}
	q.mandatoryMu.Lock()
	if q.retired || q.workerExited {
		q.mandatoryMu.Unlock()
		return false
	}
	if b.tracksIdle {
		q.pending.Add(1)
	}
	if q.mandatoryProgress == nil {
		q.mandatoryProgress = make(chan struct{})
	}
	q.mandatory = append(q.mandatory, submitFailureCleanupEntry{
		task:            qt,
		admittedFence:   q.admitted.Load(),
		waitSubmitFence: waitSubmitFence,
	})
	q.mandatoryMu.Unlock()
	b.markSent(q)
	b.cfg.observer.OnEnqueue(accountID, len(q.ch))
	select {
	case q.mandatoryWake <- struct{}{}:
	default:
	}
	return true
}

// finishWorker is the single exit gate of every worker. It seals the lane and
// reports true only when nothing is left in the backlog, so no exit path can
// abandon a queued handle release. Once it returns true,
// enqueueSubmitFailureCleanup refuses the dead lane and its caller releases
// the handle itself.
func (*base) finishWorker(q *keyQueue) bool {
	q.mandatoryMu.Lock()
	defer q.mandatoryMu.Unlock()
	if len(q.mandatory) != 0 {
		return false
	}
	q.workerExited = true
	return true
}

func (b *base) takeMandatoryCleanup(
	q *keyQueue,
) (task queuedTask, ready bool, waitingForSubmitFence bool) {
	q.mandatoryMu.Lock()
	defer q.mandatoryMu.Unlock()
	if len(q.mandatory) == 0 {
		return queuedTask{}, false, false
	}
	entry := q.mandatory[0]
	if entry.waitSubmitFence {
		select {
		case <-b.submitFenceCh:
			// Producers registered before stop have all admitted their work
			// by now, so the release owes that work an ordering too.
			entry.waitSubmitFence = false
			entry.admittedFence = q.admitted.Load()
			q.mandatory[0] = entry
		default:
			return queuedTask{}, false, true
		}
	}
	// Wait for the lane work admitted before the entry, and only for that:
	// tasks admitted afterwards, including those of unrelated accounts on a
	// shared shard, cannot delay the release.
	if q.settled.Load() < entry.admittedFence {
		return queuedTask{}, false, false
	}
	q.mandatory[0] = submitFailureCleanupEntry{}
	q.mandatory = q.mandatory[1:]
	q.noteMandatoryProgressLocked()
	return entry.task, true, false
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
			q.settle()
			b.cfg.observer.OnComplete(qt.accountID, 0)
		default:
			return
		}
	}
}

// waitInFlightSubmits blocks until every producer registered before stop has
// returned, or ctx fires.
func (b *base) waitInFlightSubmits(ctx context.Context) error {
	select {
	case <-b.submitFenceCh:
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

// runLifecycleCleanup accounts mandatory cleanup that has no worker lane.
// A successful stop cannot return while cleanup registered before its final
// sealing point is still running.
func (b *base) runLifecycleCleanup(cleanup func()) {
	if !b.beginLifecycleCleanup() {
		cleanup()
		return
	}
	defer b.endLifecycleCleanup()
	cleanup()
}

func (b *base) beginLifecycleCleanup() bool {
	b.lifecycleCleanupMu.Lock()
	defer b.lifecycleCleanupMu.Unlock()
	if b.lifecycleCleanupSealed {
		return false
	}
	if b.lifecycleCleanupCount == 0 {
		b.lifecycleCleanupDone = make(chan struct{})
	}
	b.lifecycleCleanupCount++
	return true
}

func (b *base) endLifecycleCleanup() {
	b.lifecycleCleanupMu.Lock()
	defer b.lifecycleCleanupMu.Unlock()
	if b.lifecycleCleanupCount == 0 {
		return
	}
	b.lifecycleCleanupCount--
	if b.lifecycleCleanupCount == 0 {
		close(b.lifecycleCleanupDone)
		b.lifecycleCleanupDone = nil
	}
}

// finishLifecycleCleanups seals registration once the current cleanup epoch
// is empty. A timed-out stop leaves registration open for a later retry.
func (b *base) finishLifecycleCleanups(ctx context.Context) error {
	for {
		b.lifecycleCleanupMu.Lock()
		if b.lifecycleCleanupCount == 0 {
			b.lifecycleCleanupSealed = true
			b.lifecycleCleanupMu.Unlock()
			return nil
		}
		done := b.lifecycleCleanupDone
		b.lifecycleCleanupMu.Unlock()

		select {
		case <-done:
		case <-ctx.Done():
			return ctx.Err()
		}
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

// signalStopOnce closes stopCh (idempotent). After this call beginSubmit
// refuses new producers, and any submitter already blocked in sendToQueue
// returns ErrStopped.
func (b *base) signalStopOnce() {
	b.stopOnce.Do(func() {
		b.submitMu.Lock()
		close(b.stopCh)
		if b.inFlightSubmits.Load() == 0 {
			b.submitFenceOnce.Do(func() { close(b.submitFenceCh) })
		}
		b.submitMu.Unlock()
	})
}

// admitTask reserves lane capacity for a task about to enter q.ch. It runs
// before the send so idle cleanup cannot see a false-idle queue and mandatory
// cleanup cannot run ahead of a task already on its way in.
func (b *base) admitTask(q *keyQueue) {
	q.admitted.Add(1)
	if b.tracksIdle {
		q.pending.Add(1)
	}
}

// markSent records that a task was accepted into q after a successful send.
// The task was already admitted before the send (see sendToQueue), so this
// only refreshes activity; Sharded leaves q untouched.
func (b *base) markSent(q *keyQueue) {
	if b.tracksIdle {
		q.touch()
	}
}

// unmarkSent settles the admission when the send did not place a task into
// q.ch (ctx expiry, stop, or queue retirement). Dynamic callers invoke it
// under q.gate.RLock so the q.pending decrement is ordered against cleanup's
// WLock read; Sharded does not track q.pending.
func (b *base) unmarkSent(q *keyQueue) {
	q.settle()
	if b.tracksIdle {
		q.pending.Add(-1)
	}
}

// signalHardStopOnce closes hardStopCh (idempotent). Workers picking up a
// task after this point invoke task.abort instead of task.run.
func (b *base) signalHardStopOnce() {
	b.hardStopOnce.Do(func() { close(b.hardStopCh) })
}
