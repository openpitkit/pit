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
	"strings"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"go.openpit.dev/openpit/param"
)

func mandatoryFenceEngine(t *testing.T, name string) (*AsyncEngine, *base) {
	t.Helper()
	builder := NewBuilder(newFakeDriver()).WithQueueCapacity(1)
	var async *AsyncEngine
	var err error
	switch name {
	case "dynamic":
		async, err = builder.Dynamic().Build()
	case "sharded":
		async, err = builder.Sharded(1).Build()
	default:
		t.Fatalf("unknown strategy %q", name)
	}
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		if err := async.StopGraceful(context.Background()); err != nil {
			t.Error(err)
		}
	})
	switch s := async.strategy.(type) {
	case *dynamicStrategy:
		return async, &s.base
	case *shardedStrategy:
		return async, &s.base
	default:
		t.Fatalf("unexpected strategy %T", s)
		return nil, nil
	}
}

func mandatoryFenceWait(t *testing.T, description string, ready func() bool) {
	t.Helper()
	deadline := time.After(5 * time.Second)
	for !ready() {
		select {
		case <-deadline:
			t.Fatalf("timeout waiting for %s", description)
		default:
			runtime.Gosched()
		}
	}
}

func mandatoryFenceWaitForEntry(t *testing.T, q *keyQueue) {
	t.Helper()
	mandatoryFenceWait(t, "mandatory backlog entry", func() bool {
		q.mandatoryMu.Lock()
		defer q.mandatoryMu.Unlock()
		return len(q.mandatory) == 1
	})
}

func mandatoryFenceCancelSubmit(
	t *testing.T,
	async *AsyncEngine,
	q *keyQueue,
	accountID param.AccountID,
) func() {
	t.Helper()
	ctx, cancel := context.WithCancel(context.Background())
	t.Cleanup(cancel)
	result := make(chan error, 1)
	admitted := q.admitted.Load()
	go func() {
		_, err := async.Submit(ctx, accountID, func() error {
			t.Error("cancelled task ran")
			return nil
		}).Await(context.Background())
		result <- err
	}()
	mandatoryFenceWait(t, "later admission", func() bool {
		return q.admitted.Load() == admitted+1
	})
	return func() {
		cancel()
		if err := <-result; !errors.Is(err, context.Canceled) {
			t.Fatalf("later Submit error = %v, want context.Canceled", err)
		}
	}
}

func TestMandatoryFenceExclusiveAfterLaterAbandonment(t *testing.T) {
	for _, name := range []string{"dynamic", "sharded"} {
		for _, later := range []int{1, 2} {
			t.Run(fmt.Sprintf("%s/later-%d", name, later), func(t *testing.T) {
				async, _ := mandatoryFenceEngine(t, name)
				accountID := param.NewAccountIDFromUint64(1)
				release, done := blockLane(t, async, accountID)
				var aDone atomic.Bool
				a := async.Submit(context.Background(), accountID, func() error {
					aDone.Store(true)
					return nil
				})
				q := submitFailureCleanupTestQueue(t, async.strategy, accountID)
				ctx := exclusiveTestContext(t)
				section := make(chan error, 1)
				go func() {
					section <- async.Exclusive(ctx, func(context.Context, Driver) error {
						if !aDone.Load() {
							return errors.New("Exclusive ran before accepted task A")
						}
						return nil
					})
				}()
				mandatoryFenceWaitForEntry(t, q)
				cancels := make([]func(), later)
				for i := range cancels {
					cancels[i] = mandatoryFenceCancelSubmit(t, async, q, accountID)
				}
				for i := len(cancels) - 1; i >= 0; i-- {
					cancels[i]()
				}
				release()
				done()
				if err := <-section; err != nil {
					t.Error(err)
				}
				if _, err := a.Await(ctx); err != nil {
					t.Fatal(err)
				}
			})
		}
	}
}

func TestMandatoryFenceShardedCleanupAfterLaterAbandonment(t *testing.T) {
	async, _ := mandatoryFenceEngine(t, "sharded")
	k1 := param.NewAccountIDFromUint64(1)
	k2 := param.NewAccountIDFromUint64(2)
	release, done := blockLane(t, async, k1)
	events := make(chan string, 2)
	a := async.Submit(context.Background(), k1, func() error {
		events <- "A"
		return nil
	})
	q := submitFailureCleanupTestQueue(t, async.strategy, k1)
	async.strategy.scheduleSubmitFailureCleanup(accountRoutingKey(k1), func() {
		events <- "E"
	})
	mandatoryFenceWaitForEntry(t, q)
	mandatoryFenceCancelSubmit(t, async, q, k2)()
	release()
	done()
	ctx := exclusiveTestContext(t)
	if _, err := a.Await(ctx); err != nil {
		t.Fatal(err)
	}
	var order []string
	for len(order) < 2 {
		select {
		case event := <-events:
			order = append(order, event)
		case <-ctx.Done():
			t.Fatal(ctx.Err())
		}
	}
	if strings.Join(order, ",") != "A,E" {
		t.Fatalf("cleanup order = %v, want [A E]", order)
	}
}

func mandatoryFenceGoroutine() string {
	var buf [256]byte
	header := string(buf[:runtime.Stack(buf[:], false)])
	return header[:strings.IndexByte(header, '[')]
}

func mandatoryFenceParked(stacks, goroutine, function string) bool {
	for _, stack := range strings.Split(stacks, "\n\n") {
		if strings.HasPrefix(stack, goroutine+"[select]") &&
			strings.Contains(stack, function) {
			return true
		}
	}
	return false
}

func TestMandatoryFenceAbandonmentWakesParkedBarrier(t *testing.T) {
	for _, name := range []string{"dynamic", "sharded"} {
		t.Run(name, func(t *testing.T) {
			async, b := mandatoryFenceEngine(t, name)
			accountID := param.NewAccountIDFromUint64(1)
			var worker string
			warmup := async.Submit(context.Background(), accountID, func() error {
				worker = mandatoryFenceGoroutine()
				return nil
			})
			if _, err := warmup.Await(context.Background()); err != nil {
				t.Fatal(err)
			}
			q := submitFailureCleanupTestQueue(t, async.strategy, accountID)
			mandatoryFenceWait(t, "warmup settlement", func() bool {
				return q.settled.Load() == 1
			})
			if !b.beginSubmit() {
				t.Fatal("synthetic producer refused")
			}
			q.gate.RLock()
			ticket := b.admitTask(q)
			abandon := sync.OnceFunc(func() {
				b.unmarkSent(q, ticket)
				q.gate.RUnlock()
				b.endSubmit()
			})
			t.Cleanup(abandon)
			ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
			defer cancel()
			section := make(chan error, 1)
			sectionGoroutine := make(chan string, 1)
			go func() {
				sectionGoroutine <- mandatoryFenceGoroutine()
				section <- async.Exclusive(ctx, func(context.Context, Driver) error {
					return nil
				})
			}()
			caller := <-sectionGoroutine
			mandatoryFenceWaitForEntry(t, q)
			buf := make([]byte, 1<<20)
			mandatoryFenceWait(t, "worker and Exclusive parked after the wake", func() bool {
				stacks := string(buf[:runtime.Stack(buf, true)])
				return mandatoryFenceParked(stacks, worker, ".(*base).worker(") &&
					mandatoryFenceParked(stacks, caller, ".(*base).runExclusive(") &&
					!strings.Contains(stacks, ".(*base).enqueueMandatory(") &&
					len(q.mandatoryWake) == 0
			})
			abandon()
			if err := <-section; err != nil {
				t.Fatalf("Exclusive after abandonment = %v, want nil", err)
			}
		})
	}
}

// TestMandatoryQueuedMirrorsBacklogAcrossPops pins the mutex-free empty-backlog
// check: while the first of two queued cleanups runs, the second is still in
// the backlog, so producers and the worker must still see it queued.
func TestMandatoryQueuedMirrorsBacklogAcrossPops(t *testing.T) {
	b := newBase(baseConfig{queueCapacity: 1}, false)
	q := newKeyQueue(1)
	key := accountRoutingKey(param.NewAccountIDFromUint64(1))
	firstRunning := make(chan struct{})
	releaseFirst := make(chan struct{})
	secondRan := make(chan struct{})
	if !b.enqueueSubmitFailureCleanup(q, key, func() {
		close(firstRunning)
		<-releaseFirst
	}, false) {
		t.Fatal("live lane refused the first cleanup")
	}
	if !b.enqueueSubmitFailureCleanup(q, key, func() {
		close(secondRan)
	}, false) {
		t.Fatal("live lane refused the second cleanup")
	}
	b.workersWG.Add(1)
	go b.worker(q)

	select {
	case <-firstRunning:
	case <-time.After(5 * time.Second):
		t.Fatal("first cleanup did not run")
	}
	if !q.mandatoryQueued.Load() {
		t.Error("backlog reads empty while the second cleanup is queued")
	}
	close(releaseFirst)
	select {
	case <-secondRan:
	case <-time.After(5 * time.Second):
		t.Fatal("second cleanup did not run")
	}
	mandatoryFenceWait(t, "empty backlog", func() bool {
		return !q.mandatoryQueued.Load()
	})

	b.closeQueueChannels([]*keyQueue{q})
	select {
	case <-q.done:
	case <-time.After(5 * time.Second):
		t.Fatal("worker did not exit")
	}
	b.workersWG.Wait()
}
