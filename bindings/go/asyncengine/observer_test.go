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
	"errors"
	"sync"
	"testing"
	"time"

	"go.openpit.dev/openpit/param"
)

// recordingObserver is a thread-safe Observer implementation for tests.
// Function hook fields (e.g. OnQueueFullBlockedFunc) are called after the
// counter is updated; they are optional and may be nil.
type recordingObserver struct {
	mu sync.Mutex

	enqueueCount          int
	dequeueCount          int
	completeCount         int
	slowSubmitCount       int
	queueFullBlockedCount int
	queueCreatedCount     int
	queueRemovedCount     int
	submitCancelledCount  int

	queueCreatedAccounts []param.AccountID
	queueRemovedAccounts []param.AccountID
	submitCancelledErrs  []error

	// Optional hooks called (without lock held) after the matching counter
	// is updated.
	OnQueueFullBlockedFunc func()
}

func (o *recordingObserver) OnEnqueue(_ param.AccountID, _ int) {
	o.mu.Lock()
	o.enqueueCount++
	o.mu.Unlock()
}

func (o *recordingObserver) OnDequeue(_ param.AccountID, _ time.Duration) {
	o.mu.Lock()
	o.dequeueCount++
	o.mu.Unlock()
}

func (o *recordingObserver) OnComplete(_ param.AccountID, _ time.Duration) {
	o.mu.Lock()
	o.completeCount++
	o.mu.Unlock()
}

func (o *recordingObserver) OnSlowSubmit(_ param.AccountID, _ time.Duration, _ int) {
	o.mu.Lock()
	o.slowSubmitCount++
	o.mu.Unlock()
}

func (o *recordingObserver) OnQueueFullBlocked(_ param.AccountID, _ time.Duration) {
	o.mu.Lock()
	o.queueFullBlockedCount++
	fn := o.OnQueueFullBlockedFunc
	o.mu.Unlock()
	if fn != nil {
		fn()
	}
}

func (o *recordingObserver) OnQueueCreated(accountID param.AccountID, _ int) {
	o.mu.Lock()
	o.queueCreatedCount++
	o.queueCreatedAccounts = append(o.queueCreatedAccounts, accountID)
	o.mu.Unlock()
}

func (o *recordingObserver) OnQueueRemoved(accountID param.AccountID, _ int) {
	o.mu.Lock()
	o.queueRemovedCount++
	o.queueRemovedAccounts = append(o.queueRemovedAccounts, accountID)
	o.mu.Unlock()
}

func (o *recordingObserver) OnSubmitCancelled(_ param.AccountID, err error) {
	o.mu.Lock()
	o.submitCancelledCount++
	o.submitCancelledErrs = append(o.submitCancelledErrs, err)
	o.mu.Unlock()
}

func (o *recordingObserver) counts() (
	enqueue, dequeue, complete, slowSubmit, queueFullBlocked, queueCreated, queueRemoved, submitCancelled int,
) {
	o.mu.Lock()
	defer o.mu.Unlock()
	return o.enqueueCount, o.dequeueCount, o.completeCount,
		o.slowSubmitCount, o.queueFullBlockedCount,
		o.queueCreatedCount, o.queueRemovedCount, o.submitCancelledCount
}

// TestAsyncEngineObserverEnqueueDequeueCompletePerTask asserts that
// OnEnqueue, OnDequeue, and OnComplete each fire exactly once per task
// across a batch.
func TestAsyncEngineObserverEnqueueDequeueCompletePerTask(t *testing.T) {
	t.Parallel()
	obs := &recordingObserver{}
	driver := newFakeDriver()
	async, err := NewBuilder(driver).
		WithObserver(obs).
		Dynamic().
		MaxQueues(0).
		IdleCleanupAfter(0).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}

	const tasks = 20
	for i := 0; i < tasks; i++ {
		f := async.StartPreTrade(
			context.Background(), buildTestOrder(t, uint64(i%4)),
		)
		if _, _, err := f.Await(context.Background()); err != nil {
			t.Fatalf("task %d Await error = %v", i, err)
		}
	}
	if err := async.StopGraceful(context.Background()); err != nil {
		t.Fatalf("StopGraceful() error = %v", err)
	}

	enq, deq, comp, _, _, _, _, _ := obs.counts()
	if enq != tasks {
		t.Errorf("enqueueCount = %d, want %d", enq, tasks)
	}
	if deq != tasks {
		t.Errorf("dequeueCount = %d, want %d", deq, tasks)
	}
	if comp != tasks {
		t.Errorf("completeCount = %d, want %d", comp, tasks)
	}
}

// TestAsyncEngineObserverQueueCreatedDynamic asserts that OnQueueCreated
// fires once per new Dynamic per-account queue.
func TestAsyncEngineObserverQueueCreatedDynamic(t *testing.T) {
	t.Parallel()
	obs := &recordingObserver{}
	driver := newFakeDriver()
	async, err := NewBuilder(driver).
		WithObserver(obs).
		Dynamic().
		MaxQueues(0).
		IdleCleanupAfter(0).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}

	// Submit to 3 distinct accounts — expect 3 creation events.
	for i := uint64(1); i <= 3; i++ {
		f := async.StartPreTrade(
			context.Background(), buildTestOrder(t, i),
		)
		if _, _, err := f.Await(context.Background()); err != nil {
			t.Fatalf("acc=%d Await error = %v", i, err)
		}
	}
	// Submit again to the same accounts — no new creation.
	for i := uint64(1); i <= 3; i++ {
		f := async.StartPreTrade(
			context.Background(), buildTestOrder(t, i),
		)
		if _, _, err := f.Await(context.Background()); err != nil {
			t.Fatalf("acc=%d second Await error = %v", i, err)
		}
	}

	if err := async.StopGraceful(context.Background()); err != nil {
		t.Fatalf("StopGraceful() error = %v", err)
	}

	_, _, _, _, _, created, _, _ := obs.counts()
	if created != 3 {
		t.Errorf("queueCreatedCount = %d, want 3", created)
	}
}

// TestAsyncEngineObserverQueueRemovedDynamic asserts that OnQueueRemoved
// fires after idle cleanup retires a Dynamic queue.
func TestAsyncEngineObserverQueueRemovedDynamic(t *testing.T) {
	t.Parallel()
	obs := &recordingObserver{}
	driver := newFakeDriver()

	const idleAfter = 5 * time.Millisecond
	async, err := NewBuilder(driver).
		WithObserver(obs).
		Dynamic().
		MaxQueues(0).
		IdleCleanupAfter(idleAfter).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}

	strategy, ok := async.strategy.(*dynamicStrategy)
	if !ok {
		t.Fatalf("unexpected strategy type %T", async.strategy)
	}

	// Submit to create two queues, then wait for them to become idle.
	f1 := async.StartPreTrade(context.Background(), buildTestOrder(t, 10))
	f2 := async.StartPreTrade(context.Background(), buildTestOrder(t, 20))
	if _, _, err := f1.Await(context.Background()); err != nil {
		t.Fatalf("f1 Await error = %v", err)
	}
	if _, _, err := f2.Await(context.Background()); err != nil {
		t.Fatalf("f2 Await error = %v", err)
	}

	// Wait until both queues are past the idle window then drive cleanup.
	deadline := time.Now().Add(5 * time.Second)
	for {
		<-time.After(idleAfter)
		strategy.cleanupIdle()
		_, _, _, _, _, _, removed, _ := obs.counts()
		if removed >= 2 {
			break
		}
		if time.Now().After(deadline) {
			_, _, _, _, _, _, removed, _ := obs.counts()
			t.Fatalf("queueRemovedCount = %d, want >= 2 (deadline exceeded)", removed)
		}
	}

	if err := async.StopGraceful(context.Background()); err != nil {
		t.Fatalf("StopGraceful() error = %v", err)
	}

	_, _, _, _, _, _, removed, _ := obs.counts()
	if removed < 2 {
		t.Errorf("queueRemovedCount = %d, want >= 2", removed)
	}
}

// TestAsyncEngineObserverShardedNoQueueCallbacks asserts that OnQueueCreated
// and OnQueueRemoved never fire for the Sharded strategy.
func TestAsyncEngineObserverShardedNoQueueCallbacks(t *testing.T) {
	t.Parallel()
	obs := &recordingObserver{}
	driver := newFakeDriver()
	async, err := NewBuilder(driver).
		WithObserver(obs).
		Sharded(2).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}

	for i := 0; i < 10; i++ {
		f := async.StartPreTrade(
			context.Background(), buildTestOrder(t, uint64(i)),
		)
		if _, _, err := f.Await(context.Background()); err != nil {
			t.Fatalf("task %d Await error = %v", i, err)
		}
	}
	if err := async.StopGraceful(context.Background()); err != nil {
		t.Fatalf("StopGraceful() error = %v", err)
	}

	_, _, _, _, _, created, removed, _ := obs.counts()
	if created != 0 {
		t.Errorf("Sharded: queueCreatedCount = %d, want 0", created)
	}
	if removed != 0 {
		t.Errorf("Sharded: queueRemovedCount = %d, want 0", removed)
	}
}

// TestAsyncEngineObserverSlowSubmitAndQueueFullBlocked asserts that
// OnSlowSubmit and OnQueueFullBlocked fire when the queue is full and the
// submitter blocks longer than the threshold.
func TestAsyncEngineObserverSlowSubmitAndQueueFullBlocked(t *testing.T) {
	t.Parallel()
	obs := &recordingObserver{}
	driver := newFakeDriver()
	gate := make(chan struct{})
	started := make(chan struct{}, 1)
	driver.startHook = func() {
		select {
		case started <- struct{}{}:
		default:
		}
		<-gate
	}

	const threshold = 5 * time.Millisecond
	async, err := NewBuilder(driver).
		WithObserver(obs).
		WithQueueCapacity(1).
		WithSlowSubmitThreshold(threshold).
		Sharded(1).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}

	// First task enters the worker and blocks.
	f1 := async.StartPreTrade(context.Background(), buildTestOrder(t, 1))
	<-started

	// Second task fills the queue buffer.
	f2 := async.StartPreTrade(context.Background(), buildTestOrder(t, 1))

	// Third task: submit blocks on a full queue long enough for slow callbacks.
	f3DoneCh := make(chan struct{})
	go func() {
		defer close(f3DoneCh)
		ctx, cancel := context.WithTimeout(context.Background(), 10*threshold)
		defer cancel()
		f := async.StartPreTrade(ctx, buildTestOrder(t, 1))
		// Drain future; the outcome (cancelled or not) is not asserted here.
		_, _, _ = f.Await(context.Background())
	}()

	// Wait for at least one slow callback with a bounded timeout.
	deadline := time.Now().Add(5 * time.Second)
	for {
		_, _, _, slow, blocked, _, _, _ := obs.counts()
		if slow >= 1 && blocked >= 1 {
			break
		}
		if time.Now().After(deadline) {
			t.Fatal("timeout waiting for OnSlowSubmit/OnQueueFullBlocked callbacks")
		}
		<-time.After(threshold / 2)
	}

	// Release the worker; this unblocks f1, which lets f2 run, which lets f3
	// either queue or time out.
	close(gate)
	<-f3DoneCh
	if _, _, err := f1.Await(context.Background()); err != nil {
		t.Fatalf("f1 Await error = %v", err)
	}
	if _, _, err := f2.Await(context.Background()); err != nil {
		t.Fatalf("f2 Await error = %v", err)
	}
	if err := async.StopGraceful(context.Background()); err != nil {
		t.Fatalf("StopGraceful() error = %v", err)
	}

	_, _, _, slow, blocked, _, _, _ := obs.counts()
	if slow < 1 {
		t.Errorf("slowSubmitCount = %d, want >= 1", slow)
	}
	if blocked < 1 {
		t.Errorf("queueFullBlockedCount = %d, want >= 1", blocked)
	}
}

// TestAsyncEngineObserverSubmitCancelledPreCancelled asserts that
// OnSubmitCancelled fires when ctx is already cancelled at submit time.
func TestAsyncEngineObserverSubmitCancelledPreCancelled(t *testing.T) {
	t.Parallel()
	obs := &recordingObserver{}
	driver := newFakeDriver()
	async, err := NewBuilder(driver).
		WithObserver(obs).
		Sharded(1).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer func() {
		if err := async.StopGraceful(context.Background()); err != nil {
			t.Fatalf("StopGraceful() error = %v", err)
		}
	}()

	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	f := async.StartPreTrade(ctx, buildTestOrder(t, 1))
	_, _, err = f.Await(context.Background())
	if !errors.Is(err, context.Canceled) {
		t.Fatalf("Await() err = %v, want context.Canceled", err)
	}

	_, _, _, _, _, _, _, cancelled := obs.counts()
	if cancelled != 1 {
		t.Errorf("submitCancelledCount = %d, want 1", cancelled)
	}
}

// TestAsyncEngineObserverSubmitCancelledQueueFullTimeout asserts that
// OnSubmitCancelled fires when a queue-full submit times out via ctx
// deadline.
func TestAsyncEngineObserverSubmitCancelledQueueFullTimeout(t *testing.T) {
	t.Parallel()
	obs := &recordingObserver{}
	driver := newFakeDriver()
	gate := make(chan struct{})
	started := make(chan struct{}, 1)
	driver.startHook = func() {
		select {
		case started <- struct{}{}:
		default:
		}
		<-gate
	}

	async, err := NewBuilder(driver).
		WithObserver(obs).
		WithQueueCapacity(1).
		WithSlowSubmitThreshold(time.Millisecond).
		Sharded(1).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}

	f1 := async.StartPreTrade(context.Background(), buildTestOrder(t, 1))
	<-started
	f2 := async.StartPreTrade(context.Background(), buildTestOrder(t, 1))

	// f3 submit times out because the queue is full and the worker is blocked.
	ctx, cancel := context.WithTimeout(context.Background(), 20*time.Millisecond)
	defer cancel()
	f3 := async.StartPreTrade(ctx, buildTestOrder(t, 1))
	_, _, err = f3.Await(context.Background())
	if !errors.Is(err, context.DeadlineExceeded) {
		t.Fatalf("f3 Await err = %v, want DeadlineExceeded", err)
	}

	_, _, _, _, _, _, _, cancelled := obs.counts()
	if cancelled < 1 {
		t.Errorf("submitCancelledCount after timeout = %d, want >= 1", cancelled)
	}

	close(gate)
	if _, _, err := f1.Await(context.Background()); err != nil {
		t.Fatalf("f1 Await error = %v", err)
	}
	if _, _, err := f2.Await(context.Background()); err != nil {
		t.Fatalf("f2 Await error = %v", err)
	}
	if err := async.StopGraceful(context.Background()); err != nil {
		t.Fatalf("StopGraceful() error = %v", err)
	}
}

// TestAsyncEngineObserverCompleteFiresForAbortedTasks asserts that
// OnComplete fires even for tasks aborted by hard stop (with ran=0 duration).
func TestAsyncEngineObserverCompleteFiresForAbortedTasks(t *testing.T) {
	t.Parallel()
	obs := &recordingObserver{}
	driver := newFakeDriver()
	gate := make(chan struct{})
	started := make(chan struct{}, 1)
	driver.startHook = func() {
		select {
		case started <- struct{}{}:
		default:
		}
		<-gate
	}

	async, err := NewBuilder(driver).
		WithObserver(obs).
		WithQueueCapacity(16).
		Sharded(1).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}

	// First task occupies the worker.
	first := async.StartPreTrade(context.Background(), buildTestOrder(t, 1))
	<-started

	const pendingCount = 3
	type f2type = interface{ Done() bool }
	pending := make([]f2type, pendingCount)
	for i := 0; i < pendingCount; i++ {
		pending[i] = async.StartPreTrade(context.Background(), buildTestOrder(t, 1))
	}

	hardStopDone := make(chan error, 1)
	go func() {
		hardStopDone <- async.StopHard(context.Background())
	}()
	hardStopDeadline := time.Now().Add(5 * time.Second)
	for !isStrategyHardStopped(async.strategy) {
		if time.Now().After(hardStopDeadline) {
			t.Fatal("timeout waiting for strategy to reach hard-stopped state")
		}
		<-time.After(time.Microsecond)
	}
	close(gate)
	if err := <-hardStopDone; err != nil {
		t.Fatalf("StopHard() error = %v", err)
	}
	if _, _, err := first.Await(context.Background()); err != nil {
		t.Fatalf("first Await error = %v", err)
	}

	deadline := time.Now().Add(5 * time.Second)
	for _, p := range pending {
		for !p.Done() {
			if time.Now().After(deadline) {
				t.Fatal("pending future never resolved")
			}
			<-time.After(time.Millisecond)
		}
	}

	// All tasks (run + aborted) should have fired OnComplete. The aborted
	// ones fire with ran=0; the first ran normally.
	_, _, comp, _, _, _, _, _ := obs.counts()
	// 1 (first ran) + pendingCount (aborted).
	wantMin := 1 + pendingCount
	if comp < wantMin {
		t.Errorf("completeCount = %d, want >= %d", comp, wantMin)
	}
}
