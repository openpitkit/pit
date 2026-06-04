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
	"runtime"
	"testing"
	"time"
)

// NOTE(T8 goroutine leak guard): A full no-dep goroutine-leak check at the
// TestMain level is deliberately omitted. Parallel tests share the goroutine
// namespace, so a baseline captured at TestMain startup includes goroutines
// from unrelated parallel test goroutines, making the post-test delta
// unreliable without a dependency like goleak. Instead, we apply a bounded
// per-test check inside two representative sequential tests below, where
// we control the engine lifecycle precisely.

// assertNoGoroutineLeak checks that the goroutine count returns to at most
// baseline+tolerance within a bounded retry window. It is applied to tests that
// create and fully stop an engine, where we can reason about which goroutines
// should have exited. The tolerance of 2 accounts for runtime/GC goroutines
// that may come and go.
//
// This helper intentionally uses no external dependency.
func assertNoGoroutineLeak(t *testing.T, baseline int) {
	t.Helper()
	const tolerance = 2
	const retryInterval = 10 * time.Millisecond
	const maxRetries = 50 // 500ms total

	for i := 0; i < maxRetries; i++ {
		current := runtime.NumGoroutine()
		if current <= baseline+tolerance {
			return
		}
		<-time.After(retryInterval)
	}
	// Final measurement after retries.
	current := runtime.NumGoroutine()
	if current > baseline+tolerance {
		t.Errorf(
			"goroutine leak: baseline=%d, current=%d (delta=%d > tolerance=%d)",
			baseline, current, current-baseline, tolerance,
		)
	}
}

// TestAsyncEngineShardedGoroutineCleanup verifies that all worker goroutines
// started by a Sharded engine exit after StopGraceful.
func TestAsyncEngineShardedGoroutineCleanup(t *testing.T) {
	// NOTE: Not parallel — the goroutine baseline must be stable. Running this
	// sequentially in isolation is what makes the check meaningful.

	baseline := runtime.NumGoroutine()
	driver := newFakeDriver()
	async, err := NewBuilder(driver).Sharded(4).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}

	// Run a few tasks.
	for i := 0; i < 8; i++ {
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

	assertNoGoroutineLeak(t, baseline)
}

// TestAsyncEngineDynamicGoroutineCleanup verifies that all worker goroutines
// and the cleanup goroutine started by a Dynamic engine exit after StopGraceful.
func TestAsyncEngineDynamicGoroutineCleanup(t *testing.T) {
	// NOTE: Not parallel — same reasoning as TestAsyncEngineShardedGoroutineCleanup.

	baseline := runtime.NumGoroutine()
	driver := newFakeDriver()
	async, err := NewBuilder(driver).
		Dynamic().
		MaxQueues(0).
		IdleCleanupAfter(time.Millisecond).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}

	// Create several per-account queues and run tasks.
	for i := 0; i < 6; i++ {
		f := async.StartPreTrade(
			context.Background(), buildTestOrder(t, uint64(i+1)),
		)
		if _, _, err := f.Await(context.Background()); err != nil {
			t.Fatalf("task %d Await error = %v", i, err)
		}
	}

	if err := async.StopGraceful(context.Background()); err != nil {
		t.Fatalf("StopGraceful() error = %v", err)
	}

	assertNoGoroutineLeak(t, baseline)
}
