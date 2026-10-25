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
	"go.openpit.dev/openpit/pkg/future"
)

// exclusiveTestBuilders builds one engine per strategy; a Sharded engine has
// fewer shards than the tests use accounts, so shards carry several keys.
func exclusiveTestBuilders() map[string]func(t *testing.T) *AsyncEngine {
	build := func(t *testing.T, builder func(*Builder) (*AsyncEngine, error)) *AsyncEngine {
		t.Helper()
		async, err := builder(NewBuilder(newFakeDriver()))
		if err != nil {
			t.Fatal(err)
		}
		t.Cleanup(func() {
			if err := async.StopGraceful(context.Background()); err != nil {
				t.Error(err)
			}
		})
		return async
	}
	return map[string]func(t *testing.T) *AsyncEngine{
		"dynamic": func(t *testing.T) *AsyncEngine {
			return build(t, func(b *Builder) (*AsyncEngine, error) { return b.Dynamic().Build() })
		},
		"sharded": func(t *testing.T) *AsyncEngine {
			return build(t, func(b *Builder) (*AsyncEngine, error) { return b.Sharded(3).Build() })
		},
	}
}

func exclusiveTestContext(t *testing.T) context.Context {
	t.Helper()
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	t.Cleanup(cancel)
	return ctx
}

// blockLane occupies accountID's lane with a task until the returned release
// is called.
func blockLane(
	t *testing.T,
	async *AsyncEngine,
	accountID param.AccountID,
) (release func(), done func()) {
	t.Helper()
	entered := make(chan struct{})
	releaseCh := make(chan struct{})
	release = sync.OnceFunc(func() { close(releaseCh) })
	t.Cleanup(release)
	blocked := async.Submit(context.Background(), accountID, func() error {
		close(entered)
		<-releaseCh
		return nil
	})
	<-entered
	return release, func() {
		if _, err := blocked.Await(context.Background()); err != nil {
			t.Error(err)
		}
	}
}

func TestExclusiveRunsWithEveryLaneStopped(t *testing.T) {
	for name, build := range exclusiveTestBuilders() {
		t.Run(name, func(t *testing.T) {
			async := build(t)
			var inFlight, overlaps atomic.Int64
			var sectionActive atomic.Bool
			stop := make(chan struct{})
			var workers sync.WaitGroup
			for worker := uint64(1); worker <= 8; worker++ {
				workers.Add(1)
				go func(accountID param.AccountID) {
					defer workers.Done()
					for {
						select {
						case <-stop:
							return
						default:
						}
						task := async.Submit(context.Background(), accountID, func() error {
							inFlight.Add(1)
							if sectionActive.Load() {
								overlaps.Add(1)
							}
							runtime.Gosched()
							inFlight.Add(-1)
							return nil
						})
						if _, err := task.Await(context.Background()); err != nil {
							t.Error(err)
							return
						}
					}
				}(param.NewAccountIDFromUint64(worker))
			}
			ctx := exclusiveTestContext(t)
			for range 50 {
				err := async.Exclusive(ctx, func(context.Context, Driver) error {
					sectionActive.Store(true)
					if inFlight.Load() != 0 {
						overlaps.Add(1)
					}
					runtime.Gosched()
					sectionActive.Store(false)
					return nil
				})
				if err != nil {
					t.Error(err)
					break
				}
			}
			close(stop)
			workers.Wait()
			if got := overlaps.Load(); got != 0 {
				t.Fatalf("lane tasks overlapped an exclusive section %d times", got)
			}
		})
	}
}

func TestExclusiveReturnsTheSectionError(t *testing.T) {
	async := exclusiveTestBuilders()["dynamic"](t)
	errSection := errors.New("section failed")
	err := async.Exclusive(exclusiveTestContext(t), func(context.Context, Driver) error {
		return errSection
	})
	if !errors.Is(err, errSection) {
		t.Fatalf("Exclusive() error = %v, want the section error", err)
	}
}

func TestExclusiveRefusesAContextThatCannotEnd(t *testing.T) {
	async := exclusiveTestBuilders()["dynamic"](t)
	called := false
	err := async.Exclusive(context.Background(), func(context.Context, Driver) error {
		called = true
		return nil
	})
	if !errors.Is(err, ErrExclusiveContext) || called {
		t.Fatalf("Exclusive(Background) = %v, called %v; want ErrExclusiveContext", err, called)
	}
}

func TestExclusiveHoldsWorkSubmittedDuringTheSection(t *testing.T) {
	for name, build := range exclusiveTestBuilders() {
		t.Run(name, func(t *testing.T) {
			async := build(t)
			existing := param.NewAccountIDFromUint64(41)
			// A Dynamic lane created during the section starts stopped.
			created := param.NewAccountIDFromUint64(42)
			if _, err := async.Submit(context.Background(), existing, func() error { return nil }).
				Await(context.Background()); err != nil {
				t.Fatal(err)
			}
			var ran atomic.Int64
			var queued [2]*future.Future[struct{}]
			outside := exclusiveTestContext(t)
			err := async.Exclusive(exclusiveTestContext(t), func(context.Context, Driver) error {
				submitted := make(chan struct{})
				go func() {
					defer close(submitted)
					for i, accountID := range []param.AccountID{existing, created} {
						queued[i] = async.Submit(outside, accountID, func() error {
							ran.Add(1)
							return nil
						})
					}
				}()
				<-submitted
				time.Sleep(20 * time.Millisecond)
				if got := ran.Load(); got != 0 {
					t.Errorf("%d tasks ran during the section", got)
				}
				return nil
			})
			if err != nil {
				t.Fatal(err)
			}
			for _, task := range queued {
				if _, err := task.Await(context.Background()); err != nil {
					t.Fatal(err)
				}
			}
			if got := ran.Load(); got != 2 {
				t.Fatalf("tasks run after the section = %d, want 2", got)
			}
		})
	}
}

func TestExclusiveCancelledBeforeEveryLaneStopsReleasesTheLanes(t *testing.T) {
	for name, build := range exclusiveTestBuilders() {
		t.Run(name, func(t *testing.T) {
			async := build(t)
			busy := param.NewAccountIDFromUint64(51)
			release, done := blockLane(t, async, busy)
			ctx, cancel := context.WithTimeout(context.Background(), 50*time.Millisecond)
			defer cancel()
			err := async.Exclusive(ctx, func(context.Context, Driver) error {
				t.Error("the section ran beside a running lane task")
				return nil
			})
			if !errors.Is(err, context.DeadlineExceeded) {
				t.Fatalf("Exclusive() error = %v, want context.DeadlineExceeded", err)
			}
			if name == "dynamic" {
				// The cancelled section left no other lane stopped. A Sharded
				// shard may carry the busy account too, so only Dynamic gives
				// the other account a lane of its own.
				other := param.NewAccountIDFromUint64(52)
				awaitCtx, awaitCancel := context.WithTimeout(context.Background(), 10*time.Second)
				defer awaitCancel()
				if _, err := async.Submit(context.Background(), other, func() error { return nil }).
					Await(awaitCtx); err != nil {
					t.Fatalf("other lane after a cancelled section: %v", err)
				}
			}
			release()
			done()
			if err := async.Exclusive(exclusiveTestContext(t), func(context.Context, Driver) error {
				return nil
			}); err != nil {
				t.Fatalf("Exclusive() after the lane drained = %v", err)
			}
		})
	}
}

func TestExclusiveStopDuringPreparationEndsTheSection(t *testing.T) {
	for name, build := range exclusiveTestBuilders() {
		for _, hard := range []bool{false, true} {
			t.Run(fmt.Sprintf("%s/hard=%v", name, hard), func(t *testing.T) {
				async := build(t)
				blocked := param.NewAccountIDFromUint64(61)
				release, _ := blockLane(t, async, blocked)
				section := make(chan error, 1)
				called := atomic.Bool{}
				go func() {
					section <- async.Exclusive(exclusiveTestContext(t), func(context.Context, Driver) error {
						called.Store(true)
						return nil
					})
				}()
				time.Sleep(20 * time.Millisecond)
				stopped := make(chan error, 1)
				go func() {
					if hard {
						stopped <- async.StopHard(exclusiveTestContext(t))
						return
					}
					stopped <- async.StopGraceful(exclusiveTestContext(t))
				}()
				if err := <-section; !errors.Is(err, ErrStopped) || called.Load() {
					t.Fatalf("Exclusive() during stop = %v, called %v; want ErrStopped", err, called.Load())
				}
				release()
				if err := <-stopped; err != nil {
					t.Fatal(err)
				}
				if err := async.Exclusive(exclusiveTestContext(t), func(context.Context, Driver) error {
					return nil
				}); !errors.Is(err, ErrStopped) {
					t.Fatalf("Exclusive() after stop = %v, want ErrStopped", err)
				}
			})
		}
	}
}

func TestExclusiveRefusesACancelledContextOnAnIdleEngine(t *testing.T) {
	for name, build := range exclusiveTestBuilders() {
		t.Run(name, func(t *testing.T) {
			async := build(t)
			ctx, cancel := context.WithCancel(context.Background())
			cancel()
			// With nothing to wait for, a ready turn can win the select against
			// the cancelled context; the section must still refuse.
			for range 50 {
				called := false
				err := async.Exclusive(ctx, func(context.Context, Driver) error {
					called = true
					return nil
				})
				if !errors.Is(err, context.Canceled) || called {
					t.Fatalf("Exclusive(cancelled) = %v, called %v; want context.Canceled", err, called)
				}
			}
		})
	}
}

func TestExclusiveHoldsALaneCreatedWhileLanesStop(t *testing.T) {
	async := exclusiveTestBuilders()["dynamic"](t)
	release, done := blockLane(t, async, param.NewAccountIDFromUint64(64))
	var ran atomic.Int64
	ranAtSection := make(chan int64, 1)
	section := make(chan error, 1)
	go func() {
		section <- async.Exclusive(exclusiveTestContext(t), func(context.Context, Driver) error {
			ranAtSection <- ran.Load()
			return nil
		})
	}()
	time.Sleep(20 * time.Millisecond)
	// The section is waiting for the busy lane; this lane is born stopped.
	created := async.Submit(context.Background(), param.NewAccountIDFromUint64(65), func() error {
		ran.Add(1)
		return nil
	})
	time.Sleep(20 * time.Millisecond)
	if got := ran.Load(); got != 0 {
		t.Fatalf("a lane created during preparation ran %d tasks before the section", got)
	}
	release()
	done()
	if err := <-section; err != nil {
		t.Fatal(err)
	}
	if got := <-ranAtSection; got != 0 {
		t.Fatalf("a lane created during preparation ran %d tasks beside the section", got)
	}
	if _, err := created.Await(context.Background()); err != nil {
		t.Fatal(err)
	}
}

func TestExclusiveAbandonedSectionRunsPostponedReleasesBeforeTheLanes(t *testing.T) {
	async, err := NewBuilder(newFakeDriver()).Dynamic().MaxQueues(2).Build()
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		if err := async.StopGraceful(context.Background()); err != nil {
			t.Error(err)
		}
	})
	dynamic := async.strategy.(*dynamicStrategy)
	busy := param.NewAccountIDFromUint64(66)
	idle := param.NewAccountIDFromUint64(67)
	release, done := blockLane(t, async, busy)
	if _, err := async.Submit(context.Background(), idle, func() error { return nil }).
		Await(context.Background()); err != nil {
		t.Fatal(err)
	}
	var orderMu sync.Mutex
	var order []string
	record := func(event string) {
		orderMu.Lock()
		order = append(order, event)
		orderMu.Unlock()
	}
	ctx, cancel := context.WithTimeout(context.Background(), 300*time.Millisecond)
	defer cancel()
	section := make(chan error, 1)
	go func() {
		section <- async.Exclusive(ctx, func(context.Context, Driver) error {
			t.Error("the section ran beside a busy lane")
			return nil
		})
	}()
	time.Sleep(30 * time.Millisecond)
	later := async.Submit(context.Background(), idle, func() error {
		record("later work")
		return nil
	})
	releaseStarted := make(chan struct{})
	releaseGate := make(chan struct{})
	openGate := sync.OnceFunc(func() { close(releaseGate) })
	t.Cleanup(openGate)
	// Both queue slots are taken, so this release has no lane.
	dynamic.scheduleSubmitFailureCleanup(
		accountRoutingKey(param.NewAccountIDFromUint64(68)),
		func() {
			record("release")
			close(releaseStarted)
			<-releaseGate
		},
	)
	select {
	case <-releaseStarted:
	case <-time.After(10 * time.Second):
		t.Fatal("the abandoned section did not run its postponed release")
	}
	// The lanes stay stopped while the release runs.
	time.Sleep(50 * time.Millisecond)
	if later.Done() {
		t.Error("later lane work ran beside a postponed release")
	}
	openGate()
	if err := <-section; !errors.Is(err, context.DeadlineExceeded) {
		t.Fatalf("Exclusive() = %v, want context.DeadlineExceeded", err)
	}
	if _, err := later.Await(context.Background()); err != nil {
		t.Fatal(err)
	}
	release()
	done()
	orderMu.Lock()
	defer orderMu.Unlock()
	if len(order) != 2 || order[0] != "release" || order[1] != "later work" {
		t.Fatalf("order = %v, want the postponed release before later lane work", order)
	}
}

func TestExclusiveHardStopKeepsAnAbortedLaneUntilTheSectionEnds(t *testing.T) {
	async, err := NewBuilder(newFakeDriver()).Dynamic().MaxQueues(2).Build()
	if err != nil {
		t.Fatal(err)
	}
	dynamic := async.strategy.(*dynamicStrategy)
	busy := param.NewAccountIDFromUint64(71)
	idle := param.NewAccountIDFromUint64(72)
	releaseBusy, _ := blockLane(t, async, busy)
	if _, err := async.Submit(context.Background(), idle, func() error { return nil }).
		Await(context.Background()); err != nil {
		t.Fatal(err)
	}
	section := make(chan error, 1)
	go func() {
		section <- async.Exclusive(exclusiveTestContext(t), func(context.Context, Driver) error {
			t.Error("the section ran beside a busy lane")
			return nil
		})
	}()
	time.Sleep(30 * time.Millisecond)
	// Queued behind the busy lane's barrier; the hard stop aborts it.
	later := async.Submit(context.Background(), busy, func() error { return nil })
	releaseStarted := make(chan struct{})
	releaseGate := make(chan struct{})
	openGate := sync.OnceFunc(func() { close(releaseGate) })
	t.Cleanup(openGate)
	// Both queue slots are taken, so this release has no lane.
	dynamic.scheduleSubmitFailureCleanup(
		accountRoutingKey(param.NewAccountIDFromUint64(73)),
		func() {
			close(releaseStarted)
			<-releaseGate
		},
	)
	stopped := make(chan error, 1)
	go func() { stopped <- async.StopHard(exclusiveTestContext(t)) }()
	select {
	case <-releaseStarted:
	case <-time.After(10 * time.Second):
		t.Fatal("the stopped section did not run its postponed release")
	}
	// The busy lane now reaches its barrier, which the hard stop aborts. The
	// lane must stay held while the section still runs the release.
	releaseBusy()
	time.Sleep(50 * time.Millisecond)
	if later.Done() {
		t.Error("an aborted lane went on beside a postponed release")
	}
	openGate()
	if err := <-section; !errors.Is(err, ErrStopped) {
		t.Fatalf("Exclusive() = %v, want ErrStopped", err)
	}
	if _, err := later.Await(context.Background()); !errors.Is(err, ErrStopped) {
		t.Fatalf("later work = %v, want ErrStopped", err)
	}
	if err := <-stopped; err != nil {
		t.Fatal(err)
	}
}

func TestExclusiveReleasesTheLanesWhenAPostponedReleasePanics(t *testing.T) {
	async, err := NewBuilder(newFakeDriver()).Dynamic().MaxQueues(1).Build()
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		if err := async.StopGraceful(context.Background()); err != nil {
			t.Error(err)
		}
	})
	dynamic := async.strategy.(*dynamicStrategy)
	occupied := param.NewAccountIDFromUint64(69)
	if _, err := async.Submit(context.Background(), occupied, func() error { return nil }).
		Await(context.Background()); err != nil {
		t.Fatal(err)
	}
	recovered := func() (recovered any) {
		defer func() { recovered = recover() }()
		_ = async.Exclusive(exclusiveTestContext(t), func(context.Context, Driver) error {
			scheduled := make(chan struct{})
			go func() {
				defer close(scheduled)
				dynamic.scheduleSubmitFailureCleanup(
					accountRoutingKey(param.NewAccountIDFromUint64(70)),
					func() { panic("release failed") },
				)
			}()
			<-scheduled
			return nil
		})
		return nil
	}()
	if recovered != "release failed" {
		t.Fatalf("Exclusive() panic = %v, want the release's panic", recovered)
	}
	// When fn panics too, fn's panic is the one that propagates.
	recovered = func() (recovered any) {
		defer func() { recovered = recover() }()
		_ = async.Exclusive(exclusiveTestContext(t), func(context.Context, Driver) error {
			scheduled := make(chan struct{})
			go func() {
				defer close(scheduled)
				dynamic.scheduleSubmitFailureCleanup(
					accountRoutingKey(param.NewAccountIDFromUint64(74)),
					func() { panic("release failed") },
				)
			}()
			<-scheduled
			panic("fn failed")
		})
		return nil
	}()
	if recovered != "fn failed" {
		t.Fatalf("Exclusive() panic = %v, want fn's panic", recovered)
	}
	awaitCtx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()
	if _, err := async.Submit(context.Background(), occupied, func() error { return nil }).
		Await(awaitCtx); err != nil {
		t.Fatalf("lane after a panicking release: %v", err)
	}
}

func TestStopWaitsForARunningSectionBeforeReleasingTheEngine(t *testing.T) {
	var released atomic.Bool
	async, err := NewBuilder(newFakeDriver()).
		WithStopUnderlying(func() { released.Store(true) }).
		Dynamic().
		Build()
	if err != nil {
		t.Fatal(err)
	}
	inSection := make(chan struct{})
	leave := make(chan struct{})
	section := make(chan error, 1)
	go func() {
		section <- async.Exclusive(exclusiveTestContext(t), func(context.Context, Driver) error {
			close(inSection)
			<-leave
			if released.Load() {
				t.Error("the engine was released under a running section")
			}
			return nil
		})
	}()
	<-inSection
	stopped := make(chan error, 1)
	go func() { stopped <- async.StopGraceful(exclusiveTestContext(t)) }()
	select {
	case err := <-stopped:
		t.Fatalf("StopGraceful returned %v while a section was running", err)
	case <-time.After(50 * time.Millisecond):
	}
	close(leave)
	if err := <-section; err != nil {
		t.Fatal(err)
	}
	if err := <-stopped; err != nil {
		t.Fatal(err)
	}
	if !released.Load() {
		t.Fatal("stop did not release the engine")
	}
}

func TestExclusiveSectionsRunOneAtATime(t *testing.T) {
	async := exclusiveTestBuilders()["dynamic"](t)
	var active, overlaps atomic.Int64
	var sections sync.WaitGroup
	for range 8 {
		sections.Add(1)
		go func() {
			defer sections.Done()
			for range 20 {
				err := async.Exclusive(exclusiveTestContext(t), func(context.Context, Driver) error {
					if active.Add(1) != 1 {
						overlaps.Add(1)
					}
					runtime.Gosched()
					active.Add(-1)
					return nil
				})
				if err != nil {
					t.Error(err)
					return
				}
			}
		}()
	}
	sections.Wait()
	if got := overlaps.Load(); got != 0 {
		t.Fatalf("exclusive sections overlapped %d times", got)
	}
}

func TestExclusiveStopsALaneWithAFullQueue(t *testing.T) {
	async, err := NewBuilder(newFakeDriver()).WithQueueCapacity(1).Dynamic().Build()
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		if err := async.StopGraceful(context.Background()); err != nil {
			t.Error(err)
		}
	})
	accountID := param.NewAccountIDFromUint64(71)
	release, done := blockLane(t, async, accountID)
	var ran atomic.Int64
	queued := async.Submit(context.Background(), accountID, func() error {
		ran.Add(1)
		return nil
	})
	// The queue is full, so this producer waits for room.
	blockedProducer := make(chan error, 1)
	go func() {
		_, err := async.Submit(context.Background(), accountID, func() error {
			ran.Add(1)
			return nil
		}).Await(context.Background())
		blockedProducer <- err
	}()
	section := make(chan error, 1)
	go func() {
		section <- async.Exclusive(exclusiveTestContext(t), func(context.Context, Driver) error {
			if got := ran.Load(); got < 1 {
				t.Errorf("tasks admitted before the section ran %d times, want at least 1", got)
			}
			return nil
		})
	}()
	time.Sleep(20 * time.Millisecond)
	release()
	done()
	if err := <-section; err != nil {
		t.Fatal(err)
	}
	if _, err := queued.Await(context.Background()); err != nil {
		t.Fatal(err)
	}
	if err := <-blockedProducer; err != nil {
		t.Fatal(err)
	}
}

func TestExclusivePostponesALaneFreeCleanupUntilAfterTheSection(t *testing.T) {
	async, err := NewBuilder(newFakeDriver()).Dynamic().MaxQueues(1).Build()
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		if err := async.StopGraceful(context.Background()); err != nil {
			t.Error(err)
		}
	})
	dynamic := async.strategy.(*dynamicStrategy)
	occupied := param.NewAccountIDFromUint64(81)
	if _, err := async.Submit(context.Background(), occupied, func() error { return nil }).
		Await(context.Background()); err != nil {
		t.Fatal(err)
	}
	var cleaned atomic.Bool
	err = async.Exclusive(exclusiveTestContext(t), func(context.Context, Driver) error {
		scheduled := make(chan struct{})
		go func() {
			defer close(scheduled)
			// The only queue slot is taken, so this release has no lane.
			dynamic.scheduleSubmitFailureCleanup(
				accountRoutingKey(param.NewAccountIDFromUint64(82)),
				func() { cleaned.Store(true) },
			)
		}()
		<-scheduled
		if cleaned.Load() {
			t.Error("a lane-free cleanup ran during the section")
		}
		return nil
	})
	if err != nil {
		t.Fatal(err)
	}
	if !cleaned.Load() {
		t.Fatal("the postponed cleanup did not run before the section ended")
	}
}

func TestExclusiveKeepsSnapshottedLanesFromIdleRetirement(t *testing.T) {
	strategy := newDynamicStrategy(baseConfig{}, dynamicConfig{
		idleCleanupAfter: time.Millisecond,
		cleanupPeriod:    time.Hour,
	})
	t.Cleanup(func() {
		if err := strategy.stopGraceful(context.Background()); err != nil {
			t.Error(err)
		}
	})
	key := accountRoutingKey(param.NewAccountIDFromUint64(91))
	q, _, _, state := strategy.getOrCreate(key)
	if state != dynamicQueueReady {
		t.Fatalf("getOrCreate state = %v", state)
	}
	time.Sleep(5 * time.Millisecond)
	section := newExclusiveSection()
	strategy.installExclusive(section)
	if removed, _ := strategy.retireIfIdle(idleCandidate{key: key, q: q}, time.Now()); removed {
		t.Fatal("an idle lane was retired while a section was installed")
	}
	if _, withdrawn := strategy.withdrawExclusive(section); !withdrawn {
		t.Fatal("the section could not be withdrawn")
	}
	if removed, _ := strategy.retireIfIdle(idleCandidate{key: key, q: q}, time.Now()); !removed {
		t.Fatal("the idle lane stayed after the section was withdrawn")
	}
}

func TestExclusiveBarriersStayOutOfObserverMetrics(t *testing.T) {
	observer := &recordingObserver{}
	async, err := NewBuilder(newFakeDriver()).WithObserver(observer).Dynamic().Build()
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		if err := async.StopGraceful(context.Background()); err != nil {
			t.Error(err)
		}
	})
	if _, err := async.Submit(context.Background(), param.NewAccountIDFromUint64(101),
		func() error { return nil }).Await(context.Background()); err != nil {
		t.Fatal(err)
	}
	enqueued, dequeued, completed, _, _, _, _, _ := observer.counts()
	err = async.Exclusive(exclusiveTestContext(t), func(context.Context, Driver) error { return nil })
	if err != nil {
		t.Fatal(err)
	}
	afterEnqueued, afterDequeued, afterCompleted, _, _, _, _, _ := observer.counts()
	if afterEnqueued != enqueued || afterDequeued != dequeued || afterCompleted != completed {
		t.Fatalf(
			"observer saw a section: enqueue %d->%d, dequeue %d->%d, complete %d->%d",
			enqueued, afterEnqueued, dequeued, afterDequeued, completed, afterCompleted,
		)
	}
}
