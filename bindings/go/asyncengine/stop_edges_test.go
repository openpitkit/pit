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
	"errors"
	"fmt"
	"runtime"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"go.openpit.dev/openpit/param"
)

type submitFailureCleanupTestTask struct {
	started chan<- struct{}
	release <-chan struct{}
}

func (t submitFailureCleanupTestTask) run() {
	if t.started != nil {
		t.started <- struct{}{}
	}
	if t.release != nil {
		<-t.release
	}
}

func (submitFailureCleanupTestTask) abort(error) {}

type submitFailureCleanupOrderTask struct {
	event  string
	events chan<- string
}

func (t submitFailureCleanupOrderTask) run() { t.events <- t.event }

func (t submitFailureCleanupOrderTask) abort(error) { t.events <- t.event }

type submitFailureCleanupRetryObserver struct {
	NoopObserver
	onQueueCreated func(param.AccountID)
}

// submitFailureHandoffGateStrategy exposes the submit-failure handoff window
// without changing production scheduling. submit blocks after the underlying
// strategy reports a synchronous error, and cleanup blocks after it starts.
type submitFailureHandoffGateStrategy struct {
	strategy
	errorKnown     chan struct{}
	releaseHandoff chan struct{}
	cleanupStarted chan struct{}
	releaseCleanup chan struct{}
	errorKnownOnce sync.Once
	cleanupOnce    sync.Once
}

func (s *submitFailureHandoffGateStrategy) submit(
	ctx context.Context,
	accountID param.AccountID,
	task pendingTask,
) error {
	err := s.strategy.submit(ctx, accountID, task)
	if err != nil {
		s.errorKnownOnce.Do(func() { close(s.errorKnown) })
		<-s.releaseHandoff
	}
	return err
}

func (s *submitFailureHandoffGateStrategy) submitWithFailureHandoff(
	ctx context.Context,
	accountID param.AccountID,
	task pendingTask,
	onFailure func(error),
) error {
	return submitWithFailureHandoff(
		ctx, s.strategy, accountID, task, func(err error) {
			s.errorKnownOnce.Do(func() { close(s.errorKnown) })
			<-s.releaseHandoff
			onFailure(err)
		},
	)
}

func (s *submitFailureHandoffGateStrategy) scheduleSubmitFailureCleanup(
	accountID param.AccountID,
	cleanup func(),
) {
	s.strategy.scheduleSubmitFailureCleanup(accountID, func() {
		s.cleanupOnce.Do(func() { close(s.cleanupStarted) })
		<-s.releaseCleanup
		cleanup()
	})
}

func (o *submitFailureCleanupRetryObserver) OnQueueCreated(
	accountID param.AccountID,
	_ int,
) {
	o.onQueueCreated(accountID)
}

func submitFailureCleanupTestQueue(
	t *testing.T,
	s strategy,
	accountID param.AccountID,
) *keyQueue {
	t.Helper()
	switch strategy := s.(type) {
	case *dynamicStrategy:
		strategy.mu.RLock()
		q := strategy.queues[accountID]
		strategy.mu.RUnlock()
		if q == nil {
			t.Fatal("Dynamic queue was not created")
		}
		return q
	case *shardedStrategy:
		return strategy.shardFor(accountID)
	default:
		t.Fatalf("strategy type = %T", s)
		return nil
	}
}

// TestWorkerRetiredLaneDoesNotAbandonQueuedCleanup pins the worker exit
// contract that makes a handle release exactly once: a lane that stops taking
// channel work still runs the cleanup it has already accepted, and once its
// worker is gone it accepts no more. The lane is sealed and retired directly
// because the whole point is that the contract must not depend on the order in
// which stop and idle cleanup happen to run.
func TestWorkerRetiredLaneDoesNotAbandonQueuedCleanup(t *testing.T) {
	b := newBase(baseConfig{queueCapacity: 1}, true)
	q := newKeyQueue(1)
	b.workersWG.Add(1)
	go b.worker(q)
	accountID := param.NewAccountIDFromUint64(1)

	cleanupRan := make(chan struct{})
	if !b.enqueueSubmitFailureCleanup(q, accountID, func() {
		close(cleanupRan)
	}, true) {
		t.Fatal("live lane refused mandatory cleanup")
	}
	// Seal the lane the way stop does, then retire it underneath the worker
	// while the release is still waiting for the producer fence.
	b.closeQueueChannels([]*keyQueue{q})
	close(q.quit)
	select {
	case <-cleanupRan:
		t.Fatal("cleanup ran before the producer fence closed")
	case <-time.After(20 * time.Millisecond):
	}

	b.signalStopOnce()
	select {
	case <-cleanupRan:
	case <-time.After(5 * time.Second):
		t.Fatal("retired lane abandoned the cleanup it had accepted")
	}
	select {
	case <-q.done:
	case <-time.After(5 * time.Second):
		t.Fatal("worker did not exit after draining its cleanup")
	}
	if b.enqueueSubmitFailureCleanup(q, accountID, func() {}, true) {
		t.Fatal("dead lane accepted cleanup that no worker can run")
	}
	b.workersWG.Wait()
}

// TestShardedMandatoryCleanupFencesOnlyItsAccount proves the ordering fence is
// scoped to the account that owes a handle release. A Sharded queue carries
// many accounts, so fencing the queue would stall accounts with nothing
// pending; the account that owes the release must still stay ordered behind
// it, and its producers must still honour their own context.
func TestShardedMandatoryCleanupFencesOnlyItsAccount(t *testing.T) {
	s := newShardedStrategy(baseConfig{queueCapacity: 4}, 1)
	fenced := param.NewAccountIDFromUint64(1)
	other := param.NewAccountIDFromUint64(2)
	releaseWorker := make(chan struct{})
	var releaseOnce sync.Once
	release := func() { releaseOnce.Do(func() { close(releaseWorker) }) }
	defer release()

	started := make(chan struct{}, 1)
	if err := s.submit(context.Background(), fenced,
		submitFailureCleanupTestTask{started: started, release: releaseWorker},
	); err != nil {
		t.Fatalf("blocking submit error = %v", err)
	}
	<-started
	cleanupRan := make(chan struct{})
	s.scheduleSubmitFailureCleanup(fenced, func() { close(cleanupRan) })

	otherSubmit := make(chan error, 1)
	go func() {
		otherSubmit <- s.submit(
			context.Background(), other, submitFailureCleanupTestTask{},
		)
	}()
	select {
	case err := <-otherSubmit:
		if err != nil {
			t.Fatalf("unrelated account submit error = %v", err)
		}
	case <-time.After(5 * time.Second):
		release()
		t.Fatal("one account's cleanup fenced an unrelated account")
	}

	fencedSubmit := make(chan error, 1)
	go func() {
		fencedSubmit <- s.submit(
			context.Background(), fenced, submitFailureCleanupTestTask{},
		)
	}()
	select {
	case err := <-fencedSubmit:
		release()
		t.Fatalf("submit overtook its own account's cleanup: %v", err)
	case <-time.After(20 * time.Millisecond):
	}

	cancelledCtx, cancel := context.WithCancel(context.Background())
	cancelledSubmit := make(chan error, 1)
	go func() {
		cancelledSubmit <- s.submit(
			cancelledCtx, fenced, submitFailureCleanupTestTask{},
		)
	}()
	cancel()
	select {
	case err := <-cancelledSubmit:
		if !errors.Is(err, context.Canceled) {
			release()
			t.Fatalf("fenced submit error = %v, want context.Canceled", err)
		}
	case <-time.After(5 * time.Second):
		release()
		t.Fatal("fenced producer ignored its cancelled context")
	}

	release()
	select {
	case <-cleanupRan:
	case <-time.After(5 * time.Second):
		t.Fatal("cleanup did not run after the account lane drained")
	}
	if err := <-fencedSubmit; err != nil {
		t.Fatalf("fenced submit error = %v", err)
	}
	if err := s.stopGraceful(context.Background()); err != nil {
		t.Fatalf("stopGraceful error = %v", err)
	}
}

// TestSubmitFailureCleanupHandoffRetainsLifecycleObligation proves that an
// operation whose submit was already registered before stop cannot lose its
// lifecycle obligation between the synchronous submit error and mandatory
// cleanup registration.
func TestSubmitFailureCleanupHandoffRetainsLifecycleObligation(t *testing.T) {
	tests := []struct {
		name string
		stop func(*AsyncEngine, context.Context) error
	}{
		{name: "Graceful", stop: (*AsyncEngine).StopGraceful},
		{name: "Hard", stop: (*AsyncEngine).StopHard},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			errorKnown := make(chan struct{})
			releaseHandoff := make(chan struct{})
			cleanupStarted := make(chan struct{})
			releaseCleanup := make(chan struct{})
			var releaseHandoffOnce sync.Once
			var releaseCleanupOnce sync.Once
			releaseHandoffGate := func() {
				releaseHandoffOnce.Do(func() { close(releaseHandoff) })
			}
			releaseCleanupGate := func() {
				releaseCleanupOnce.Do(func() { close(releaseCleanup) })
			}
			t.Cleanup(releaseHandoffGate)
			t.Cleanup(releaseCleanupGate)

			wrapped := &submitFailureHandoffGateStrategy{
				strategy:       newDynamicStrategy(baseConfig{queueCapacity: 1}, dynamicConfig{}),
				errorKnown:     errorKnown,
				releaseHandoff: releaseHandoff,
				cleanupStarted: cleanupStarted,
				releaseCleanup: releaseCleanup,
			}
			underlyingStopped := make(chan struct{})
			async := newAsyncEngine(
				newFakeDriver(), func() { close(underlyingStopped) }, wrapped,
			)

			operation, rejects, err := async.ApplyDropCopy(
				context.Background(), buildTestOrder(t, 1),
			).Await(context.Background())
			if err != nil {
				t.Fatalf("ApplyDropCopy Await error = %v", err)
			}
			if rejects != nil {
				t.Fatalf("ApplyDropCopy rejects = %v, want nil", rejects)
			}

			cancelledCtx, cancel := context.WithCancel(context.Background())
			cancel()
			closeDone := make(chan error, 1)
			go func() {
				_, closeErr := operation.Close(cancelledCtx).Await(context.Background())
				closeDone <- closeErr
			}()
			select {
			case <-errorKnown:
			case <-time.After(5 * time.Second):
				t.Fatal("timeout waiting for synchronous submit error")
			}

			stopDone := make(chan error, 1)
			go func() { stopDone <- test.stop(async, context.Background()) }()
			var stopErr error
			var stopBeforeHandoff bool
			select {
			case stopErr = <-stopDone:
				stopBeforeHandoff = true
			case <-time.After(20 * time.Millisecond):
			}
			var underlyingBeforeHandoff bool
			select {
			case <-underlyingStopped:
				underlyingBeforeHandoff = true
			default:
			}

			releaseHandoffGate()
			select {
			case <-cleanupStarted:
			case <-time.After(5 * time.Second):
				t.Fatal("timeout waiting for mandatory cleanup to start")
			}

			var stopDuringCleanup bool
			if !stopBeforeHandoff {
				select {
				case stopErr = <-stopDone:
					stopDuringCleanup = true
				case <-time.After(20 * time.Millisecond):
				}
			}
			var underlyingDuringCleanup bool
			if !underlyingBeforeHandoff {
				select {
				case <-underlyingStopped:
					underlyingDuringCleanup = true
				default:
				}
			}

			releaseCleanupGate()
			select {
			case closeErr := <-closeDone:
				if !errors.Is(closeErr, context.Canceled) {
					t.Errorf("Close Await error = %v, want context.Canceled", closeErr)
				}
			case <-time.After(5 * time.Second):
				t.Fatal("timeout waiting for Close cleanup")
			}
			if !stopBeforeHandoff && !stopDuringCleanup {
				select {
				case stopErr = <-stopDone:
				case <-time.After(5 * time.Second):
					t.Fatal("timeout waiting for stop")
				}
			}
			if stopErr != nil {
				t.Errorf("stop error = %v", stopErr)
			}
			select {
			case <-underlyingStopped:
			case <-time.After(5 * time.Second):
				t.Fatal("timeout waiting for stopUnderlying")
			}

			if stopBeforeHandoff {
				t.Error("stop returned before cleanup handoff")
			}
			if underlyingBeforeHandoff {
				t.Error("stopUnderlying ran before cleanup handoff")
			}
			if stopDuringCleanup {
				t.Error("stop returned while mandatory cleanup was blocked")
			}
			if underlyingDuringCleanup {
				t.Error("stopUnderlying ran while mandatory cleanup was blocked")
			}
		})
	}
}

// TestDynamicQueueLimitFailureRetainsLifecycleObligation proves a terminal
// routing error cannot drop the producer obligation before mandatory cleanup
// takes ownership.
func TestDynamicQueueLimitFailureRetainsLifecycleObligation(t *testing.T) {
	tests := []struct {
		name string
		stop func(*AsyncEngine, context.Context) error
	}{
		{name: "Graceful", stop: (*AsyncEngine).StopGraceful},
		{name: "Hard", stop: (*AsyncEngine).StopHard},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			dynamic := newDynamicStrategy(
				baseConfig{queueCapacity: 1}, dynamicConfig{maxQueues: 1},
			)
			occupiedAccount := param.NewAccountIDFromUint64(1)
			if err := dynamic.submit(
				context.Background(), occupiedAccount, submitFailureCleanupTestTask{},
			); err != nil {
				t.Fatalf("setup submit error = %v", err)
			}

			errorKnown := make(chan struct{})
			releaseHandoff := make(chan struct{})
			cleanupStarted := make(chan struct{})
			releaseCleanup := make(chan struct{})
			var releaseHandoffOnce sync.Once
			var releaseCleanupOnce sync.Once
			releaseHandoffGate := func() {
				releaseHandoffOnce.Do(func() { close(releaseHandoff) })
			}
			releaseCleanupGate := func() {
				releaseCleanupOnce.Do(func() { close(releaseCleanup) })
			}
			t.Cleanup(releaseHandoffGate)
			t.Cleanup(releaseCleanupGate)

			wrapped := &submitFailureHandoffGateStrategy{
				strategy:       dynamic,
				errorKnown:     errorKnown,
				releaseHandoff: releaseHandoff,
				cleanupStarted: cleanupStarted,
				releaseCleanup: releaseCleanup,
			}
			underlyingStopped := make(chan struct{})
			async := newAsyncEngine(
				newFakeDriver(), func() { close(underlyingStopped) }, wrapped,
			)
			accountID := param.NewAccountIDFromUint64(2)
			submitDone := make(chan error, 1)
			go func() {
				submitDone <- submitWithFailureHandoff(
					context.Background(), wrapped, accountID,
					submitFailureCleanupTestTask{}, func(error) {
						wrapped.scheduleSubmitFailureCleanup(accountID, func() {})
					},
				)
			}()
			select {
			case <-errorKnown:
			case <-time.After(5 * time.Second):
				t.Fatal("timeout waiting for QueueLimit error")
			}

			stopDone := make(chan error, 1)
			go func() { stopDone <- test.stop(async, context.Background()) }()
			stopSignalDeadline := time.Now().Add(5 * time.Second)
			for !dynamic.isStopped() && time.Now().Before(stopSignalDeadline) {
				runtime.Gosched()
			}
			if !dynamic.isStopped() {
				t.Fatal("timeout waiting for QueueLimit stop signal")
			}
			var stopErr error
			var stopBeforeHandoff bool
			select {
			case stopErr = <-stopDone:
				stopBeforeHandoff = true
			case <-time.After(20 * time.Millisecond):
			}
			var underlyingBeforeHandoff bool
			select {
			case <-underlyingStopped:
				underlyingBeforeHandoff = true
			default:
			}

			releaseHandoffGate()
			select {
			case <-cleanupStarted:
			case <-time.After(5 * time.Second):
				t.Fatal("timeout waiting for QueueLimit cleanup")
			}
			var stopDuringCleanup bool
			if !stopBeforeHandoff {
				select {
				case stopErr = <-stopDone:
					stopDuringCleanup = true
				case <-time.After(20 * time.Millisecond):
				}
			}
			var underlyingDuringCleanup bool
			if !underlyingBeforeHandoff {
				select {
				case <-underlyingStopped:
					underlyingDuringCleanup = true
				default:
				}
			}

			releaseCleanupGate()
			select {
			case err := <-submitDone:
				if !errors.Is(err, ErrQueueLimit) {
					t.Errorf("submit error = %v, want ErrQueueLimit", err)
				}
			case <-time.After(5 * time.Second):
				t.Fatal("timeout waiting for QueueLimit submit")
			}
			if !stopBeforeHandoff && !stopDuringCleanup {
				select {
				case stopErr = <-stopDone:
				case <-time.After(5 * time.Second):
					t.Fatal("timeout waiting for stop")
				}
			}
			if stopErr != nil {
				t.Errorf("stop error = %v", stopErr)
			}
			select {
			case <-underlyingStopped:
			case <-time.After(5 * time.Second):
				t.Fatal("timeout waiting for stopUnderlying")
			}

			if stopBeforeHandoff {
				t.Error("stop returned before QueueLimit cleanup handoff")
			}
			if underlyingBeforeHandoff {
				t.Error("stopUnderlying ran before QueueLimit cleanup handoff")
			}
			if stopDuringCleanup {
				t.Error("stop returned while QueueLimit cleanup was blocked")
			}
			if underlyingDuringCleanup {
				t.Error("stopUnderlying ran while QueueLimit cleanup was blocked")
			}
		})
	}
}

// TestDynamicRetiredRetryRetainsLifecycleObligation proves one producer
// obligation spans a retired queue attempt, the retry gap, and terminal
// failure cleanup.
//
// The retry gap is held open by a real cleanup barrier, installed and later
// closed by cleanupWithoutLiveLane itself: production only ever closes a
// barrier, so a barrier the test closed by hand would prove nothing about the
// contract the parked producer depends on.
func TestDynamicRetiredRetryRetainsLifecycleObligation(t *testing.T) {
	tests := []struct {
		name string
		stop func(*AsyncEngine, context.Context) error
	}{
		{name: "Graceful", stop: (*AsyncEngine).StopGraceful},
		{name: "Hard", stop: (*AsyncEngine).StopHard},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			accountID := param.NewAccountIDFromUint64(1)
			releaseBarrier := make(chan struct{})
			barrierHeld := make(chan struct{})
			barrierDone := make(chan struct{})
			retired := make(chan struct{})
			observerErr := make(chan error, 1)
			observer := &submitFailureCleanupRetryObserver{}
			var dynamic *dynamicStrategy
			observer.onQueueCreated = func(createdAccount param.AccountID) {
				defer close(retired)
				dynamic.mu.RLock()
				q := dynamic.queues[createdAccount]
				dynamic.mu.RUnlock()
				if q == nil {
					observerErr <- errors.New("created queue was not published")
					return
				}
				removed, _ := dynamic.retireIfIdle(
					idleCandidate{accountID: createdAccount, q: q},
					time.Now().Add(time.Second),
				)
				if !removed {
					observerErr <- errors.New("could not retire first queue")
					return
				}
				go func() {
					defer close(barrierDone)
					installed := dynamic.cleanupWithoutLiveLane(
						createdAccount,
						func() {
							close(barrierHeld)
							<-releaseBarrier
						},
					)
					if !installed {
						observerErr <- errors.New("retry barrier was not installed")
					}
				}()
				select {
				case <-barrierHeld:
				case <-time.After(5 * time.Second):
					observerErr <- errors.New("timeout installing retry barrier")
				}
			}
			dynamic = newDynamicStrategy(baseConfig{
				observer: observer, queueCapacity: 1,
			}, dynamicConfig{})

			errorKnown := make(chan struct{})
			releaseHandoff := make(chan struct{})
			cleanupStarted := make(chan struct{})
			releaseCleanup := make(chan struct{})
			var releaseHandoffOnce sync.Once
			var releaseCleanupOnce sync.Once
			var releaseRetryOnce sync.Once
			releaseHandoffGate := func() {
				releaseHandoffOnce.Do(func() { close(releaseHandoff) })
			}
			releaseCleanupGate := func() {
				releaseCleanupOnce.Do(func() { close(releaseCleanup) })
			}
			releaseRetryGate := func() {
				releaseRetryOnce.Do(func() { close(releaseBarrier) })
			}
			t.Cleanup(releaseHandoffGate)
			t.Cleanup(releaseCleanupGate)
			t.Cleanup(releaseRetryGate)

			wrapped := &submitFailureHandoffGateStrategy{
				strategy:       dynamic,
				errorKnown:     errorKnown,
				releaseHandoff: releaseHandoff,
				cleanupStarted: cleanupStarted,
				releaseCleanup: releaseCleanup,
			}
			underlyingStopped := make(chan struct{})
			async := newAsyncEngine(
				newFakeDriver(), func() { close(underlyingStopped) }, wrapped,
			)
			submitDone := make(chan error, 1)
			go func() {
				submitDone <- submitWithFailureHandoff(
					context.Background(), wrapped, accountID,
					submitFailureCleanupTestTask{}, func(error) {
						wrapped.scheduleSubmitFailureCleanup(accountID, func() {})
					},
				)
			}()
			select {
			case <-retired:
			case <-time.After(5 * time.Second):
				t.Fatal("timeout waiting for first queue retirement")
			}
			select {
			case err := <-observerErr:
				t.Fatal(err)
			default:
			}
			// The barrier is installed and held, so the retry can only be
			// parked on it or on its way there; nothing can carry the
			// producer past it until production closes it.

			stopDone := make(chan error, 1)
			go func() { stopDone <- test.stop(async, context.Background()) }()
			stopSignalDeadline := time.Now().Add(5 * time.Second)
			for !dynamic.isStopped() && time.Now().Before(stopSignalDeadline) {
				runtime.Gosched()
			}
			if !dynamic.isStopped() {
				t.Fatal("timeout waiting for retired-retry stop signal")
			}
			var stopErr error
			var stopInRetryGap bool
			select {
			case stopErr = <-stopDone:
				stopInRetryGap = true
			case <-time.After(20 * time.Millisecond):
			}
			var underlyingInRetryGap bool
			select {
			case <-underlyingStopped:
				underlyingInRetryGap = true
			default:
			}

			releaseRetryGate()
			// The producer can only resume once cleanupWithoutLiveLane has
			// closed the barrier it installed.
			select {
			case <-errorKnown:
			case <-time.After(5 * time.Second):
				t.Fatal("timeout waiting for terminal retry error")
			}
			select {
			case <-barrierDone:
			case <-time.After(5 * time.Second):
				t.Fatal("timeout waiting for the barrier holder to return")
			}
			select {
			case err := <-observerErr:
				t.Fatal(err)
			default:
			}
			releaseHandoffGate()
			select {
			case <-cleanupStarted:
			case <-time.After(5 * time.Second):
				t.Fatal("timeout waiting for retired-retry cleanup")
			}
			var stopDuringCleanup bool
			if !stopInRetryGap {
				select {
				case stopErr = <-stopDone:
					stopDuringCleanup = true
				case <-time.After(20 * time.Millisecond):
				}
			}
			var underlyingDuringCleanup bool
			if !underlyingInRetryGap {
				select {
				case <-underlyingStopped:
					underlyingDuringCleanup = true
				default:
				}
			}

			releaseCleanupGate()
			select {
			case err := <-submitDone:
				if !errors.Is(err, ErrStopped) {
					t.Errorf("submit error = %v, want ErrStopped", err)
				}
			case <-time.After(5 * time.Second):
				t.Fatal("timeout waiting for retired-retry submit")
			}
			if !stopInRetryGap && !stopDuringCleanup {
				select {
				case stopErr = <-stopDone:
				case <-time.After(5 * time.Second):
					t.Fatal("timeout waiting for stop")
				}
			}
			if stopErr != nil {
				t.Errorf("stop error = %v", stopErr)
			}
			select {
			case <-underlyingStopped:
			case <-time.After(5 * time.Second):
				t.Fatal("timeout waiting for stopUnderlying")
			}

			if stopInRetryGap {
				t.Error("stop returned in the retired-attempt retry gap")
			}
			if underlyingInRetryGap {
				t.Error("stopUnderlying ran in the retired-attempt retry gap")
			}
			if stopDuringCleanup {
				t.Error("stop returned while retired-retry cleanup was blocked")
			}
			if underlyingDuringCleanup {
				t.Error("stopUnderlying ran while retired-retry cleanup was blocked")
			}
		})
	}
}

// TestSubmitFailureCleanupProducerHonorsStopDeadline proves a full queue does
// not block the cleanup caller or leave graceful or hard stop blocked on
// q.gate. Cleanup still runs once after work already accepted by the lane.
func TestSubmitFailureCleanupProducerHonorsStopDeadline(t *testing.T) {
	tests := []struct {
		name string
		new  func() strategy
		stop func(strategy, context.Context) error
	}{
		{
			name: "DynamicGraceful",
			new: func() strategy {
				return newDynamicStrategy(baseConfig{queueCapacity: 1}, dynamicConfig{})
			},
			stop: func(s strategy, ctx context.Context) error {
				return s.stopGraceful(ctx)
			},
		},
		{
			name: "DynamicHard",
			new: func() strategy {
				return newDynamicStrategy(baseConfig{queueCapacity: 1}, dynamicConfig{})
			},
			stop: func(s strategy, ctx context.Context) error {
				return s.stopHard(ctx)
			},
		},
		{
			name: "ShardedGraceful",
			new: func() strategy {
				return newShardedStrategy(baseConfig{queueCapacity: 1}, 1)
			},
			stop: func(s strategy, ctx context.Context) error {
				return s.stopGraceful(ctx)
			},
		},
		{
			name: "ShardedHard",
			new: func() strategy {
				return newShardedStrategy(baseConfig{queueCapacity: 1}, 1)
			},
			stop: func(s strategy, ctx context.Context) error {
				return s.stopHard(ctx)
			},
		},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			s := test.new()
			accountID := param.NewAccountIDFromUint64(1)
			release := make(chan struct{})
			var releaseOnce sync.Once
			releaseWorker := func() { releaseOnce.Do(func() { close(release) }) }
			t.Cleanup(func() {
				releaseWorker()
				ctx, cancel := context.WithTimeout(
					context.Background(), 5*time.Second,
				)
				defer cancel()
				_ = s.stopHard(ctx)
			})

			started := make(chan struct{}, 1)
			if err := s.submit(context.Background(), accountID,
				submitFailureCleanupTestTask{started: started, release: release},
			); err != nil {
				t.Fatalf("first submit error = %v", err)
			}
			<-started
			if err := s.submit(
				context.Background(), accountID, submitFailureCleanupTestTask{},
			); err != nil {
				t.Fatalf("second submit error = %v", err)
			}

			var cleanupCalls atomic.Int64
			cleanupQueued := make(chan struct{})
			cleanupRan := make(chan struct{})
			go func() {
				s.scheduleSubmitFailureCleanup(accountID, func() {
					cleanupCalls.Add(1)
					close(cleanupRan)
				})
				close(cleanupQueued)
			}()
			select {
			case <-cleanupQueued:
			case <-time.After(time.Second):
				releaseWorker()
				t.Fatal("full lane blocked the cleanup caller")
			}
			select {
			case <-cleanupRan:
				t.Fatal("cleanup overtook accepted account work")
			default:
			}

			stopCtx, stopCancel := context.WithTimeout(
				context.Background(), 50*time.Millisecond,
			)
			defer stopCancel()
			stopDone := make(chan error, 1)
			go func() { stopDone <- test.stop(s, stopCtx) }()
			select {
			case err := <-stopDone:
				if !errors.Is(err, context.DeadlineExceeded) {
					t.Fatalf(
						"first stop error = %v, want context.DeadlineExceeded", err,
					)
				}
			case <-time.After(time.Second):
				releaseWorker()
				t.Fatal("stop ignored its deadline while cleanup held the gate")
			}

			releaseWorker()
			stopCtx, stopCancel = context.WithTimeout(
				context.Background(), 5*time.Second,
			)
			defer stopCancel()
			if err := test.stop(s, stopCtx); err != nil {
				t.Fatalf("final stop error = %v", err)
			}
			select {
			case <-cleanupRan:
			case <-time.After(5 * time.Second):
				t.Fatal("timeout waiting for cleanup to run")
			}
			if got := cleanupCalls.Load(); got != 1 {
				t.Fatalf("cleanup calls = %d, want 1", got)
			}
		})
	}
}

func TestMandatoryCleanupIncludedInSuccessfulStop(t *testing.T) {
	tests := []struct {
		name string
		new  func() strategy
	}{
		{
			name: "Dynamic",
			new: func() strategy {
				return newDynamicStrategy(baseConfig{queueCapacity: 1}, dynamicConfig{})
			},
		},
		{
			name: "Sharded",
			new: func() strategy {
				return newShardedStrategy(baseConfig{queueCapacity: 1}, 1)
			},
		},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			s := test.new()
			accountID := param.NewAccountIDFromUint64(1)
			releaseWorker := make(chan struct{})
			releaseCleanup := make(chan struct{})
			var releaseWorkerOnce sync.Once
			var releaseCleanupOnce sync.Once
			releaseAll := func() {
				releaseWorkerOnce.Do(func() { close(releaseWorker) })
				releaseCleanupOnce.Do(func() { close(releaseCleanup) })
			}
			defer releaseAll()

			started := make(chan struct{}, 1)
			if err := s.submit(context.Background(), accountID,
				submitFailureCleanupTestTask{
					started: started,
					release: releaseWorker,
				},
			); err != nil {
				t.Fatalf("blocking submit error = %v", err)
			}
			<-started
			if err := s.submit(
				context.Background(), accountID, submitFailureCleanupTestTask{},
			); err != nil {
				t.Fatalf("buffered submit error = %v", err)
			}

			cleanupEntered := make(chan struct{})
			s.scheduleSubmitFailureCleanup(accountID, func() {
				close(cleanupEntered)
				<-releaseCleanup
			})
			stopDone := make(chan error, 1)
			go func() {
				stopDone <- s.stopGraceful(context.Background())
			}()
			releaseWorkerOnce.Do(func() { close(releaseWorker) })
			select {
			case <-cleanupEntered:
			case <-time.After(time.Second):
				t.Fatal("mandatory cleanup did not start")
			}
			select {
			case err := <-stopDone:
				releaseCleanupOnce.Do(func() { close(releaseCleanup) })
				t.Fatalf("stop returned before mandatory cleanup: %v", err)
			case <-time.After(20 * time.Millisecond):
			}
			releaseCleanupOnce.Do(func() { close(releaseCleanup) })
			select {
			case err := <-stopDone:
				if err != nil {
					t.Fatalf("stopGraceful error = %v", err)
				}
			case <-time.After(time.Second):
				t.Fatal("stop did not finish after mandatory cleanup")
			}
		})
	}
}

func TestMandatoryCleanupAfterSuccessfulStopRunsWithoutWorker(t *testing.T) {
	tests := []struct {
		name string
		new  func() strategy
	}{
		{
			name: "Dynamic",
			new: func() strategy {
				return newDynamicStrategy(baseConfig{queueCapacity: 1}, dynamicConfig{})
			},
		},
		{
			name: "Sharded",
			new: func() strategy {
				return newShardedStrategy(baseConfig{queueCapacity: 1}, 1)
			},
		},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			s := test.new()
			if err := s.stopGraceful(context.Background()); err != nil {
				t.Fatalf("stopGraceful error = %v", err)
			}
			cleanupRan := make(chan struct{})
			s.scheduleSubmitFailureCleanup(
				param.NewAccountIDFromUint64(1),
				func() { close(cleanupRan) },
			)
			select {
			case <-cleanupRan:
			default:
				t.Fatal("cleanup remained queued on a stopped worker")
			}
		})
	}
}

func TestDynamicQueueLimitCleanupIncludedInStopFence(t *testing.T) {
	s := newDynamicStrategy(
		baseConfig{queueCapacity: 1}, dynamicConfig{maxQueues: 1},
	)
	liveAccount := param.NewAccountIDFromUint64(1)
	cleanupAccount := param.NewAccountIDFromUint64(2)
	ran := make(chan struct{}, 1)
	if err := s.submit(context.Background(), liveAccount,
		submitFailureCleanupTestTask{started: ran},
	); err != nil {
		t.Fatalf("submit error = %v", err)
	}
	<-ran

	releaseCleanup := make(chan struct{})
	var releaseOnce sync.Once
	release := func() { releaseOnce.Do(func() { close(releaseCleanup) }) }
	defer release()
	cleanupEntered := make(chan struct{})
	cleanupReturned := make(chan struct{})
	go func() {
		s.scheduleSubmitFailureCleanup(cleanupAccount, func() {
			close(cleanupEntered)
			<-releaseCleanup
		})
		close(cleanupReturned)
	}()
	<-cleanupEntered

	stopDone := make(chan error, 1)
	go func() { stopDone <- s.stopGraceful(context.Background()) }()
	select {
	case err := <-stopDone:
		release()
		t.Fatalf("stop returned during QueueLimit cleanup: %v", err)
	case <-time.After(20 * time.Millisecond):
	}
	release()
	select {
	case <-cleanupReturned:
	case <-time.After(time.Second):
		t.Fatal("QueueLimit cleanup caller did not return")
	}
	select {
	case err := <-stopDone:
		if err != nil {
			t.Fatalf("stopGraceful error = %v", err)
		}
	case <-time.After(time.Second):
		t.Fatal("stop did not finish after QueueLimit cleanup")
	}
}

func TestMandatoryCleanupIsNotOvertakenByLaterNormalSubmit(t *testing.T) {
	previousProcs := runtime.GOMAXPROCS(1)
	defer runtime.GOMAXPROCS(previousProcs)
	const pairs = 64
	s := newDynamicStrategy(baseConfig{queueCapacity: pairs * 2}, dynamicConfig{})
	accountID := param.NewAccountIDFromUint64(1)
	releaseWorker := make(chan struct{})
	started := make(chan struct{}, 1)
	if err := s.submit(context.Background(), accountID,
		submitFailureCleanupTestTask{
			started: started,
			release: releaseWorker,
		},
	); err != nil {
		t.Fatalf("blocking submit error = %v", err)
	}
	<-started

	events := make(chan string, pairs*2)
	submitResults := make(chan error, pairs)
	for i := 0; i < pairs; i++ {
		cleanupEvent := fmt.Sprintf("cleanup-%d", i)
		s.scheduleSubmitFailureCleanup(accountID, func() {
			events <- cleanupEvent
		})
		go func(index int) {
			submitResults <- s.submit(
				context.Background(),
				accountID,
				submitFailureCleanupOrderTask{
					event:  fmt.Sprintf("normal-%d", index),
					events: events,
				},
			)
		}(i)
	}
	close(releaseWorker)
	for i := 0; i < pairs; i++ {
		if err := <-submitResults; err != nil {
			t.Fatalf("normal submit error = %v", err)
		}
	}
	positions := make(map[string]int, pairs*2)
	for i := 0; i < pairs*2; i++ {
		select {
		case event := <-events:
			positions[event] = i
		case <-time.After(time.Second):
			t.Fatal("timeout waiting for lane events")
		}
	}
	for i := 0; i < pairs; i++ {
		cleanupEvent := fmt.Sprintf("cleanup-%d", i)
		normalEvent := fmt.Sprintf("normal-%d", i)
		if positions[cleanupEvent] > positions[normalEvent] {
			t.Fatalf("%s was overtaken by %s", cleanupEvent, normalEvent)
		}
	}
	if err := s.stopGraceful(context.Background()); err != nil {
		t.Fatalf("stopGraceful error = %v", err)
	}
}

func TestDynamicCleanupRunsAfterPartialStopWithoutRetry(t *testing.T) {
	s := newDynamicStrategy(baseConfig{queueCapacity: 2}, dynamicConfig{})
	accountID := param.NewAccountIDFromUint64(1)
	releaseWorker := make(chan struct{})
	releaseProducer := make(chan struct{})
	var releaseWorkerOnce sync.Once
	var releaseProducerOnce sync.Once
	releaseAll := func() {
		releaseProducerOnce.Do(func() { close(releaseProducer) })
		releaseWorkerOnce.Do(func() { close(releaseWorker) })
	}
	defer releaseAll()

	started := make(chan struct{}, 1)
	if err := s.submit(context.Background(), accountID,
		submitFailureCleanupTestTask{
			started: started,
			release: releaseWorker,
		},
	); err != nil {
		t.Fatalf("blocking submit error = %v", err)
	}
	<-started
	q := submitFailureCleanupTestQueue(t, s, accountID)
	if !s.beginSubmit() {
		t.Fatal("beginSubmit refused producer before stop")
	}
	producerDone := make(chan struct{})
	go func() {
		<-releaseProducer
		q.gate.RLock()
		_ = s.sendToQueue(
			context.Background(),
			q,
			accountID,
			submitFailureCleanupTestTask{},
			q.quit,
		)
		q.gate.RUnlock()
		s.endSubmit()
		close(producerDone)
	}()

	stopCtx, stopCancel := context.WithTimeout(
		context.Background(), 20*time.Millisecond,
	)
	defer stopCancel()
	if err := s.stopGraceful(stopCtx); !errors.Is(err, context.DeadlineExceeded) {
		releaseAll()
		t.Fatalf("partial stop error = %v, want DeadlineExceeded", err)
	}
	cleanupRan := make(chan struct{})
	s.scheduleSubmitFailureCleanup(accountID, func() { close(cleanupRan) })
	releaseProducerOnce.Do(func() { close(releaseProducer) })
	select {
	case <-producerDone:
	case <-time.After(time.Second):
		releaseAll()
		t.Fatal("registered producer did not finish")
	}
	releaseWorkerOnce.Do(func() { close(releaseWorker) })
	select {
	case <-cleanupRan:
	case <-time.After(time.Second):
		ctx, cancel := context.WithTimeout(context.Background(), time.Second)
		defer cancel()
		_ = s.stopHard(ctx)
		t.Fatal("cleanup required a second stop to run")
	}
	ctx, cancel := context.WithTimeout(context.Background(), time.Second)
	defer cancel()
	if err := s.stopHard(ctx); err != nil {
		t.Fatalf("final stopHard error = %v", err)
	}
}

func TestDynamicNoLaneCleanupIncludedInRetryStop(t *testing.T) {
	tests := []struct {
		name      string
		retryStop func(*dynamicStrategy, context.Context) error
	}{
		{
			name: "Graceful",
			retryStop: func(s *dynamicStrategy, ctx context.Context) error {
				return s.stopGraceful(ctx)
			},
		},
		{
			name: "Hard",
			retryStop: func(s *dynamicStrategy, ctx context.Context) error {
				return s.stopHard(ctx)
			},
		},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			testDynamicNoLaneCleanupIncludedInRetryStop(t, test.retryStop)
		})
	}
}

func testDynamicNoLaneCleanupIncludedInRetryStop(
	t *testing.T,
	retryStop func(*dynamicStrategy, context.Context) error,
) {
	const idleAfter = time.Nanosecond
	s := newDynamicStrategy(
		baseConfig{queueCapacity: 1},
		dynamicConfig{idleCleanupAfter: idleAfter},
	)
	cleanupAccount := param.NewAccountIDFromUint64(1)
	producerAccount := param.NewAccountIDFromUint64(2)

	cleanupLaneRan := make(chan struct{}, 1)
	if err := s.submit(
		context.Background(),
		cleanupAccount,
		submitFailureCleanupTestTask{started: cleanupLaneRan},
	); err != nil {
		t.Fatalf("cleanup-account submit error = %v", err)
	}
	<-cleanupLaneRan
	cleanupQueue := submitFailureCleanupTestQueue(t, s, cleanupAccount)
	time.Sleep(time.Millisecond)
	s.cleanupIdle()
	select {
	case <-cleanupQueue.done:
	case <-time.After(time.Second):
		t.Fatal("cleanup account lane was not retired")
	}
	s.mu.RLock()
	_, cleanupLaneLive := s.queues[cleanupAccount]
	s.mu.RUnlock()
	if cleanupLaneLive {
		t.Fatal("cleanup account still has a live lane")
	}

	producerLaneRan := make(chan struct{}, 1)
	if err := s.submit(
		context.Background(),
		producerAccount,
		submitFailureCleanupTestTask{started: producerLaneRan},
	); err != nil {
		t.Fatalf("producer-account submit error = %v", err)
	}
	<-producerLaneRan
	producerQueue := submitFailureCleanupTestQueue(t, s, producerAccount)
	if !s.beginSubmit() {
		t.Fatal("beginSubmit refused producer before stop")
	}
	releaseProducer := make(chan struct{})
	var releaseProducerOnce sync.Once
	producerDone := make(chan error, 1)
	go func() {
		<-releaseProducer
		producerQueue.gate.RLock()
		err := s.sendToQueue(
			context.Background(),
			producerQueue,
			producerAccount,
			submitFailureCleanupTestTask{},
			producerQueue.quit,
		)
		producerQueue.gate.RUnlock()
		s.endSubmit()
		producerDone <- err
	}()

	stopCtx, stopCancel := context.WithTimeout(
		context.Background(), 20*time.Millisecond,
	)
	if err := s.stopGraceful(stopCtx); !errors.Is(
		err, context.DeadlineExceeded,
	) {
		stopCancel()
		releaseProducerOnce.Do(func() { close(releaseProducer) })
		t.Fatalf("partial stop error = %v, want DeadlineExceeded", err)
	}
	stopCancel()

	releaseCleanup := make(chan struct{})
	var releaseCleanupOnce sync.Once
	cleanupEntered := make(chan struct{})
	cleanupReturned := make(chan struct{})
	go func() {
		s.scheduleSubmitFailureCleanup(cleanupAccount, func() {
			close(cleanupEntered)
			<-releaseCleanup
		})
		close(cleanupReturned)
	}()
	<-cleanupEntered

	releaseProducerOnce.Do(func() { close(releaseProducer) })
	if err := <-producerDone; err != nil {
		releaseCleanupOnce.Do(func() { close(releaseCleanup) })
		t.Fatalf("registered producer send error = %v", err)
	}
	retryStopDone := make(chan error, 1)
	go func() {
		retryStopDone <- retryStop(s, context.Background())
	}()
	select {
	case err := <-retryStopDone:
		releaseCleanupOnce.Do(func() { close(releaseCleanup) })
		<-cleanupReturned
		t.Fatalf("retry stop returned before no-lane cleanup: %v", err)
	case <-time.After(20 * time.Millisecond):
	}

	releaseCleanupOnce.Do(func() { close(releaseCleanup) })
	select {
	case <-cleanupReturned:
	case <-time.After(time.Second):
		t.Fatal("no-lane cleanup did not return")
	}
	select {
	case err := <-retryStopDone:
		if err != nil {
			t.Fatalf("retry stop error = %v", err)
		}
	case <-time.After(time.Second):
		t.Fatal("retry stop did not finish after no-lane cleanup")
	}
}

func TestDynamicCleanupWithoutLiveLaneDoesNotSerializeAccounts(t *testing.T) {
	const idleAfter = time.Nanosecond
	s := newDynamicStrategy(
		baseConfig{queueCapacity: 1},
		dynamicConfig{maxQueues: 1, idleCleanupAfter: idleAfter},
	)
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	defer func() { _ = s.stopHard(ctx) }()

	liveAccount := param.NewAccountIDFromUint64(1)
	cleanupAccount := param.NewAccountIDFromUint64(2)
	otherCleanupAccount := param.NewAccountIDFromUint64(3)
	ran := make(chan struct{}, 1)
	if err := s.submit(ctx, liveAccount,
		submitFailureCleanupTestTask{started: ran},
	); err != nil {
		t.Fatalf("submit error = %v", err)
	}
	<-ran

	cleanupStarted := make(chan struct{})
	releaseCleanup := make(chan struct{})
	cleanupReturned := make(chan struct{})
	go func() {
		s.scheduleSubmitFailureCleanup(cleanupAccount, func() {
			close(cleanupStarted)
			<-releaseCleanup
		})
		close(cleanupReturned)
	}()
	<-cleanupStarted

	retired := make(chan struct{})
	go func() {
		s.cleanupIdle()
		close(retired)
	}()
	select {
	case <-retired:
	case <-time.After(time.Second):
		close(releaseCleanup)
		t.Fatal("cleanup held the queue-map lock across native release")
	}

	otherCleanupRan := make(chan struct{})
	go s.scheduleSubmitFailureCleanup(otherCleanupAccount, func() {
		close(otherCleanupRan)
	})
	select {
	case <-otherCleanupRan:
	case <-time.After(time.Second):
		close(releaseCleanup)
		t.Fatal("one account cleanup serialized an unrelated account")
	}

	sameAccountSubmit := make(chan error, 1)
	go func() {
		sameAccountSubmit <- s.submit(
			ctx, cleanupAccount, submitFailureCleanupTestTask{},
		)
	}()
	select {
	case err := <-sameAccountSubmit:
		close(releaseCleanup)
		t.Fatalf("same-account submit crossed cleanup barrier: %v", err)
	case <-time.After(20 * time.Millisecond):
	}

	close(releaseCleanup)
	select {
	case <-cleanupReturned:
	case <-time.After(time.Second):
		t.Fatal("cleanup caller did not return")
	}
	select {
	case err := <-sameAccountSubmit:
		if !errors.Is(err, ErrQueueLimit) {
			t.Fatalf("same-account submit error = %v, want ErrQueueLimit", err)
		}
	case <-time.After(time.Second):
		t.Fatal("same-account submit remained behind a released barrier")
	}
}

func TestDynamicCleanupAfterStoppedLaneReturnsBeforeBlockedWorker(t *testing.T) {
	const idleAfter = time.Nanosecond
	s := newDynamicStrategy(
		baseConfig{queueCapacity: 1},
		dynamicConfig{idleCleanupAfter: idleAfter},
	)
	accountID := param.NewAccountIDFromUint64(1)
	releaseWorker := make(chan struct{})
	var releaseOnce sync.Once
	release := func() { releaseOnce.Do(func() { close(releaseWorker) }) }
	defer release()

	started := make(chan struct{}, 1)
	if err := s.submit(context.Background(), accountID,
		submitFailureCleanupTestTask{
			started: started,
			release: releaseWorker,
		},
	); err != nil {
		t.Fatalf("submit error = %v", err)
	}
	<-started

	stopCtx, stopCancel := context.WithTimeout(
		context.Background(), 20*time.Millisecond,
	)
	defer stopCancel()
	if err := s.stopHard(stopCtx); !errors.Is(err, context.DeadlineExceeded) {
		t.Fatalf("stopHard error = %v, want context.DeadlineExceeded", err)
	}

	cleanupRan := make(chan struct{})
	cleanupReturned := make(chan struct{})
	go func() {
		s.scheduleSubmitFailureCleanup(accountID, func() {
			close(cleanupRan)
		})
		close(cleanupReturned)
	}()
	select {
	case <-cleanupReturned:
	case <-time.After(time.Second):
		release()
		t.Fatal("stopped-lane cleanup caller waited for q.done")
	}
	select {
	case <-cleanupRan:
		t.Fatal("cleanup overlapped the still-running account task")
	default:
	}

	release()
	select {
	case <-cleanupRan:
	case <-time.After(5 * time.Second):
		t.Fatal("cleanup did not run after the account lane drained")
	}
	finalCtx, finalCancel := context.WithTimeout(
		context.Background(), 5*time.Second,
	)
	defer finalCancel()
	if err := s.stopHard(finalCtx); err != nil {
		t.Fatalf("final stopHard error = %v", err)
	}
}

func TestDynamicStoppedLaneCleanupWaitsForRegisteredProducer(t *testing.T) {
	const idleAfter = time.Nanosecond
	s := newDynamicStrategy(
		baseConfig{queueCapacity: 2},
		dynamicConfig{idleCleanupAfter: idleAfter},
	)
	accountID := param.NewAccountIDFromUint64(1)
	releaseWorker := make(chan struct{})
	releaseProducer := make(chan struct{})
	var releaseWorkerOnce sync.Once
	var releaseProducerOnce sync.Once
	releaseAll := func() {
		releaseProducerOnce.Do(func() { close(releaseProducer) })
		releaseWorkerOnce.Do(func() { close(releaseWorker) })
	}
	defer releaseAll()

	workerStarted := make(chan struct{}, 1)
	if err := s.submit(context.Background(), accountID,
		submitFailureCleanupTestTask{
			started: workerStarted,
			release: releaseWorker,
		},
	); err != nil {
		t.Fatalf("blocking submit error = %v", err)
	}
	<-workerStarted
	q := submitFailureCleanupTestQueue(t, s, accountID)

	if !s.beginSubmit() {
		t.Fatal("beginSubmit refused producer before stop")
	}
	producerRan := make(chan struct{}, 1)
	producerSent := make(chan error, 1)
	go func() {
		<-releaseProducer
		q.gate.RLock()
		err := s.sendToQueue(
			context.Background(),
			q,
			accountID,
			submitFailureCleanupTestTask{started: producerRan},
			q.quit,
		)
		q.gate.RUnlock()
		s.endSubmit()
		producerSent <- err
	}()

	stopDone := make(chan error, 1)
	go func() {
		ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
		defer cancel()
		stopDone <- s.stopGraceful(ctx)
	}()
	deadline := time.Now().Add(time.Second)
	for !s.isStopped() && time.Now().Before(deadline) {
		time.Sleep(time.Millisecond)
	}
	if !s.isStopped() {
		releaseAll()
		t.Fatal("strategy did not enter stopped state")
	}
	cleanupRan := make(chan struct{})
	cleanupReturned := make(chan struct{})
	go func() {
		s.scheduleSubmitFailureCleanup(accountID, func() {
			close(cleanupRan)
		})
		close(cleanupReturned)
	}()
	select {
	case <-cleanupReturned:
	case <-time.After(time.Second):
		releaseAll()
		t.Fatal("cleanup caller waited for the registered producer")
	}

	releaseProducerOnce.Do(func() { close(releaseProducer) })
	select {
	case err := <-producerSent:
		if err != nil {
			releaseAll()
			t.Fatalf("registered producer send error = %v", err)
		}
	case <-time.After(time.Second):
		releaseAll()
		t.Fatal("registered producer did not finish")
	}
	releaseWorkerOnce.Do(func() { close(releaseWorker) })
	select {
	case <-cleanupRan:
		t.Fatal("cleanup overtook the registered producer")
	case <-producerRan:
	}
	select {
	case <-cleanupRan:
	case <-time.After(time.Second):
		t.Fatal("cleanup did not run after registered producer")
	}
	if err := <-stopDone; err != nil {
		t.Fatalf("stopGraceful error = %v", err)
	}
}

// TestDynamicIdleCleanupPeriodFloor covers the derivation of the background
// scan cadence, which package tests bypass by injecting a period of their own.
func TestDynamicIdleCleanupPeriodFloor(t *testing.T) {
	tests := []struct {
		name      string
		idleAfter time.Duration
		want      time.Duration
	}{
		{
			name:      "CleanupDisabled",
			idleAfter: 0,
			want:      defaultIdleCleanupPeriod,
		},
		{
			name:      "BelowFloor",
			idleAfter: 4 * time.Second,
			want:      defaultIdleCleanupPeriod,
		},
		{
			name:      "AtFloor",
			idleAfter: 5 * time.Second,
			want:      time.Second,
		},
		{
			name:      "AboveFloor",
			idleAfter: time.Minute,
			want:      12 * time.Second,
		},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			if got := idleCleanupPeriod(test.idleAfter); got != test.want {
				t.Fatalf(
					"idleCleanupPeriod(%v) = %v, want %v",
					test.idleAfter, got, test.want,
				)
			}
		})
	}
}

// TestDynamicCleanupLoopRetiresIdleLane drives retirement through the
// background loop instead of calling the scan directly, so the ticker, the
// scan, and retirement are exercised as a whole.
func TestDynamicCleanupLoopRetiresIdleLane(t *testing.T) {
	s := newDynamicStrategy(baseConfig{queueCapacity: 1}, dynamicConfig{
		idleCleanupAfter: time.Millisecond,
		cleanupPeriod:    time.Millisecond,
	})
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	defer func() { _ = s.stopHard(ctx) }()

	accountID := param.NewAccountIDFromUint64(1)
	ran := make(chan struct{}, 1)
	if err := s.submit(
		ctx, accountID, submitFailureCleanupTestTask{started: ran},
	); err != nil {
		t.Fatalf("submit error = %v", err)
	}
	<-ran
	q := submitFailureCleanupTestQueue(t, s, accountID)
	select {
	case <-q.done:
	case <-time.After(5 * time.Second):
		t.Fatal("the background scan never retired the idle lane")
	}
	s.mu.RLock()
	_, live := s.queues[accountID]
	s.mu.RUnlock()
	if live {
		t.Fatal("retired lane is still mapped to its account")
	}
}

// TestDynamicSubmitFailureCleanupRetriesLaneRetiredByCleanupLoop is the
// retired-lane retry as it happens in production: the background scan, not the
// test, takes the lane away between lookup and handoff, and the release still
// has to land on the live account lane.
func TestDynamicSubmitFailureCleanupRetriesLaneRetiredByCleanupLoop(
	t *testing.T,
) {
	observer := &submitFailureCleanupRetryObserver{}
	s := newDynamicStrategy(baseConfig{
		observer:      observer,
		queueCapacity: 1,
	}, dynamicConfig{
		idleCleanupAfter: 20 * time.Millisecond,
		cleanupPeriod:    time.Millisecond,
	})
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	releaseWorker := make(chan struct{})
	var releaseOnce sync.Once
	release := func() { releaseOnce.Do(func() { close(releaseWorker) }) }
	defer func() {
		release()
		_ = s.stopHard(ctx)
	}()

	accountID := param.NewAccountIDFromUint64(1)
	started := make(chan struct{}, 1)
	observerErr := make(chan error, 1)
	var created atomic.Int64
	observer.onQueueCreated = func(createdAccount param.AccountID) {
		switch created.Add(1) {
		case 1:
			s.mu.RLock()
			retired := s.queues[createdAccount]
			s.mu.RUnlock()
			if retired == nil {
				observerErr <- errors.New("created queue was not published")
				return
			}
			select {
			case <-retired.done:
			case <-time.After(5 * time.Second):
				observerErr <- errors.New("background scan kept the idle lane")
				return
			}
			// Take the scan out of the picture so the replacement lane, which
			// is idle until the probe task lands on it, survives the retry.
			s.stopCleanup()
		case 2:
			if err := s.submit(ctx, createdAccount,
				submitFailureCleanupTestTask{
					started: started,
					release: releaseWorker,
				},
			); err != nil {
				observerErr <- fmt.Errorf("replacement-lane submit: %w", err)
				return
			}
			<-started
		}
	}

	cleanupRan := make(chan struct{})
	s.scheduleSubmitFailureCleanup(accountID, func() { close(cleanupRan) })
	select {
	case err := <-observerErr:
		t.Fatal(err)
	default:
	}
	if got := created.Load(); got != 2 {
		t.Fatalf("queue creations = %d, want retired plus replacement", got)
	}
	select {
	case <-cleanupRan:
		t.Fatal("cleanup did not stay on the live account lane")
	default:
	}
	release()
	select {
	case <-cleanupRan:
	case <-time.After(5 * time.Second):
		t.Fatal("cleanup did not run on the live account lane")
	}
	if err := s.stopGraceful(ctx); err != nil {
		t.Fatalf("stopGraceful error = %v", err)
	}
}

func TestDynamicSubmitFailureCleanupRetriesRetiredLane(t *testing.T) {
	const idleAfter = time.Nanosecond
	observer := &submitFailureCleanupRetryObserver{}
	s := newDynamicStrategy(baseConfig{
		observer:      observer,
		queueCapacity: 1,
	}, dynamicConfig{idleCleanupAfter: idleAfter})
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	defer func() { _ = s.stopHard(ctx) }()
	accountID := param.NewAccountIDFromUint64(1)
	releaseWorker := make(chan struct{})
	started := make(chan struct{}, 1)
	var retiredQueue *keyQueue
	var created atomic.Int64
	observer.onQueueCreated = func(createdAccount param.AccountID) {
		switch created.Add(1) {
		case 1:
			s.mu.RLock()
			retiredQueue = s.queues[createdAccount]
			s.mu.RUnlock()
			if retiredQueue == nil {
				t.Fatal("created queue was not published")
			}
			removed, _ := s.retireIfIdle(
				idleCandidate{accountID: createdAccount, q: retiredQueue},
				time.Now().Add(time.Second),
			)
			if !removed {
				t.Fatal("probe could not retire queue before cleanup enqueue")
			}
		case 2:
			if err := s.submit(ctx, createdAccount,
				submitFailureCleanupTestTask{
					started: started,
					release: releaseWorker,
				},
			); err != nil {
				t.Fatalf("replacement-lane submit error = %v", err)
			}
			<-started
		}
	}

	cleanupRan := make(chan struct{})
	s.scheduleSubmitFailureCleanup(accountID, func() { close(cleanupRan) })
	if got := created.Load(); got != 2 {
		close(releaseWorker)
		t.Fatalf("queue creations = %d, want retired plus replacement", got)
	}
	select {
	case <-retiredQueue.done:
	case <-time.After(time.Second):
		close(releaseWorker)
		t.Fatal("retired queue worker did not exit")
	}
	select {
	case <-cleanupRan:
		close(releaseWorker)
		t.Fatal("cleanup did not stay on the live account lane")
	default:
	}
	close(releaseWorker)
	select {
	case <-cleanupRan:
	case <-time.After(time.Second):
		t.Fatal("cleanup did not run on the live account lane")
	}
}

// TestAsyncEngineStopGracefulDeadlineExceededThenHard asserts that:
//   - StopGraceful returns ctx.Err() (DeadlineExceeded) when ctx fires while
//     a worker is blocked;
//   - a subsequent StopHard completes the shutdown.
func TestAsyncEngineStopGracefulDeadlineExceededThenHard(t *testing.T) {
	t.Parallel()
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
		WithQueueCapacity(8).
		Dynamic().
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}

	// Occupy the single worker for account 1.
	first := async.StartPreTrade(context.Background(), buildTestOrder(t, 1))
	<-started

	// StopGraceful with a short deadline — should return DeadlineExceeded
	// while the worker is still blocked.
	gracefulCtx, gracefulCancel := context.WithTimeout(
		context.Background(), 20*time.Millisecond,
	)
	defer gracefulCancel()
	gracefulErr := async.StopGraceful(gracefulCtx)
	if !errors.Is(gracefulErr, context.DeadlineExceeded) {
		t.Fatalf(
			"StopGraceful() err = %v, want context.DeadlineExceeded", gracefulErr,
		)
	}

	// StopHard should complete the shutdown once the worker is released.
	hardDone := make(chan error, 1)
	go func() {
		hardDone <- async.StopHard(context.Background())
	}()
	hardStopDeadline := time.Now().Add(5 * time.Second)
	for !isStrategyHardStopped(async.strategy) {
		if time.Now().After(hardStopDeadline) {
			t.Fatal("timeout waiting for strategy to reach hard-stopped state")
		}
		<-time.After(time.Microsecond)
	}
	close(gate)
	if hardErr := <-hardDone; hardErr != nil {
		t.Fatalf("StopHard() error = %v", hardErr)
	}

	// The blocked first task should complete (it ran before the hard signal).
	if _, _, err := first.Await(context.Background()); err != nil {
		t.Fatalf("first Await error = %v", err)
	}
}

// TestAsyncEngineDoubleStopNoPanic asserts that calling stop methods more
// than once does not panic or hang. Each call uses a bounded context so the
// test cannot stall if something regresses.
func TestAsyncEngineDoubleStopNoPanic(t *testing.T) {
	t.Parallel()
	newEngine := func() *AsyncEngine {
		driver := newFakeDriver()
		async, err := NewBuilder(driver).Sharded(1).Build()
		if err != nil {
			t.Fatalf("Build() error = %v", err)
		}
		return async
	}

	shortCtx := func() context.Context {
		ctx, cancel := context.WithTimeout(
			context.Background(), 5*time.Second,
		)
		t.Cleanup(cancel)
		return ctx
	}

	// Graceful × 2.
	t.Run("GracefulTwice", func(t *testing.T) {
		t.Parallel()
		async := newEngine()
		if err := async.StopGraceful(shortCtx()); err != nil {
			t.Fatalf("first StopGraceful() error = %v", err)
		}
		if err := async.StopGraceful(shortCtx()); err != nil {
			t.Fatalf("second StopGraceful() error = %v", err)
		}
	})

	// Hard × 2.
	t.Run("HardTwice", func(t *testing.T) {
		t.Parallel()
		async := newEngine()
		if err := async.StopHard(shortCtx()); err != nil {
			t.Fatalf("first StopHard() error = %v", err)
		}
		if err := async.StopHard(shortCtx()); err != nil {
			t.Fatalf("second StopHard() error = %v", err)
		}
	})

	// Graceful then Hard.
	t.Run("GracefulThenHard", func(t *testing.T) {
		t.Parallel()
		async := newEngine()
		if err := async.StopGraceful(shortCtx()); err != nil {
			t.Fatalf("StopGraceful() error = %v", err)
		}
		if err := async.StopHard(shortCtx()); err != nil {
			t.Fatalf("StopHard() error = %v", err)
		}
	})
}

// TestAsyncEngineWithStopUnderlyingFiresOnce asserts that the callback
// installed via WithStopUnderlying is called exactly once on a successful stop,
// regardless of how many times stop is called.
func TestAsyncEngineWithStopUnderlyingFiresOnce(t *testing.T) {
	t.Parallel()
	driver := newFakeDriver()

	var callCount atomic.Int64
	stopFn := func() { callCount.Add(1) }

	async, err := NewBuilder(driver).
		WithStopUnderlying(stopFn).
		Sharded(1).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	if err := async.StopGraceful(ctx); err != nil {
		t.Fatalf("StopGraceful() error = %v", err)
	}
	// Second stop — callback must NOT fire again.
	if err := async.StopGraceful(ctx); err != nil {
		t.Fatalf("second StopGraceful() error = %v", err)
	}
	if err := async.StopHard(ctx); err != nil {
		t.Fatalf("StopHard() after graceful error = %v", err)
	}

	if got := callCount.Load(); got != 1 {
		t.Errorf("stopUnderlying called %d times, want 1", got)
	}
}
