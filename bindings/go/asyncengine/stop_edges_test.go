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
	"sync/atomic"
	"testing"
	"time"
)

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
