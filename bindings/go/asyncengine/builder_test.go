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
	"runtime"
	"testing"
)

// TestAsyncEngineBuilderShardedZeroWorkers asserts that Sharded(0).Build()
// returns a non-nil error.
func TestAsyncEngineBuilderShardedZeroWorkers(t *testing.T) {
	t.Parallel()
	driver := newFakeDriver()
	_, err := NewBuilder(driver).Sharded(0).Build()
	if err == nil {
		t.Fatal("Sharded(0).Build() returned nil error, want non-nil")
	}
}

// TestAsyncEngineBuilderDynamicNegativeMaxQueues asserts that
// Dynamic().MaxQueues(-1).Build() returns a non-nil error.
func TestAsyncEngineBuilderDynamicNegativeMaxQueues(t *testing.T) {
	t.Parallel()
	driver := newFakeDriver()
	_, err := NewBuilder(driver).Dynamic().MaxQueues(-1).Build()
	if err == nil {
		t.Fatal("Dynamic().MaxQueues(-1).Build() returned nil error, want non-nil")
	}
}

// TestAsyncEngineBuilderDynamicZeroMaxQueuesUnbounded asserts that
// MaxQueues(0) removes the cap: creating more than the default cap worth of
// accounts does not return ErrQueueLimit.
func TestAsyncEngineBuilderDynamicZeroMaxQueuesUnbounded(t *testing.T) {
	t.Parallel()
	driver := newFakeDriver()
	async, err := NewBuilder(driver).
		Dynamic().
		MaxQueues(0).
		IdleCleanupAfter(0).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer func() {
		if err := async.StopGraceful(context.Background()); err != nil {
			t.Fatalf("StopGraceful() error = %v", err)
		}
	}()

	// Exceed the old default cap (runtime.NumCPU() * 32) by 1.
	accountCount := runtime.NumCPU()*defaultDynamicMaxQueuesMultiplier + 1
	for i := 0; i < accountCount; i++ {
		f := async.StartPreTrade(
			context.Background(), buildTestOrder(t, uint64(i+1000)),
		)
		_, _, err := f.Await(context.Background())
		if errors.Is(err, ErrQueueLimit) {
			t.Fatalf(
				"account %d got ErrQueueLimit with MaxQueues(0)", i,
			)
		}
		if err != nil {
			t.Fatalf("account %d Await error = %v", i, err)
		}
	}
}
