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
	"fmt"
	"runtime"
	"time"
)

// defaultDynamicMaxQueuesMultiplier scales runtime.NumCPU() to derive the
// default MaxQueues cap for the Dynamic strategy. The multiplier is
// chosen large enough to make the cap effectively non-restrictive in
// typical hosts (16 cores -> 512 queues) while protecting against
// pathological growth from misuse such as ephemeral per-request accounts.
const defaultDynamicMaxQueuesMultiplier = 32

// Builder is the entry point of the AsyncEngine builder chain. Construct
// it via NewBuilder, then advance to a strategy stage via Sharded or
// Dynamic.
type Builder struct {
	driver              Driver
	stopUnderlying      func()
	observer            Observer
	queueCapacity       int
	slowSubmitThreshold time.Duration
}

// NewBuilder returns a Builder that wraps driver. *openpit.Engine
// satisfies Driver when the engine was built with AccountSync.
//
// AsyncEngine does not own the lifecycle of the supplied driver: stopping
// the AsyncEngine does not stop the driver. Use WithStopUnderlying to
// have AsyncEngine release the driver as part of its own stop sequence.
func NewBuilder(driver Driver) *Builder {
	return &Builder{driver: driver}
}

// WithStopUnderlying installs a teardown callback that AsyncEngine's
// StopGraceful or StopHard invokes after every worker has exited. The
// callback is invoked at most once.
//
// openpit.ReadyEngineBuilder.BuildAsync wires this to engine.Stop so the
// underlying engine is released atomically when the AsyncEngine stops.
// External callers usually leave this unset and call engine.Stop
// directly after the AsyncEngine has stopped.
func (b *Builder) WithStopUnderlying(stop func()) *Builder {
	b.stopUnderlying = stop
	return b
}

// WithObserver wires diagnostic callbacks. The default observer is a
// no-op. The observer applies to every queue regardless of strategy.
func (b *Builder) WithObserver(o Observer) *Builder {
	b.observer = o
	return b
}

// WithQueueCapacity sets the buffered channel size of each per-account or
// per-shard queue. Zero or negative resets to the default (1024). Larger
// capacities smooth bursts at the cost of memory and a longer tail
// during graceful stop.
func (b *Builder) WithQueueCapacity(capacity int) *Builder {
	b.queueCapacity = capacity
	return b
}

// WithSlowSubmitThreshold controls how long the submitter blocks before
// the observer is notified that the queue is slow. Zero or negative resets
// to the default (1 minute).
func (b *Builder) WithSlowSubmitThreshold(d time.Duration) *Builder {
	b.slowSubmitThreshold = d
	return b
}

func (b *Builder) baseConfig() baseConfig {
	return baseConfig{
		observer:            b.observer,
		queueCapacity:       b.queueCapacity,
		slowSubmitThreshold: b.slowSubmitThreshold,
	}
}

// Sharded selects the fixed N-shard strategy and advances to
// ShardedBuilder where Build is available.
//
// Pros: cheapest hot path, O(1) memory regardless of account population,
// lock-free routing (no per-queue RWMutex on the send path); one short
// shared read-lock per submit only to order against stop, so concurrent
// submits are not serialized against each other. Cons: one hot account
// saturates a single shard while others stay idle, no per-account
// observability.
//
// Choose this when the active account set is broad and roughly balanced
// and you want the lowest possible per-call overhead.
func (b *Builder) Sharded(workers int) *ShardedBuilder {
	return &ShardedBuilder{parent: b, workers: workers}
}

// Dynamic selects the lazy per-account strategy with idle cleanup and
// advances to DynamicBuilder where MaxQueues, IdleCleanupAfter, and
// Build are available.
//
// Pros: full per-account isolation, no hot-shard bottlenecks, queue-level
// observer events per account. Cons: an RWMutex hit on each submit
// lookup, background cleanup goroutine, slightly higher memory per
// active account.
//
// Choose this when account activity is skewed, when you want per-account
// dispatch metrics, or when the population is large enough that
// statically allocating shards would be wasteful.
func (b *Builder) Dynamic() *DynamicBuilder {
	return &DynamicBuilder{
		parent:           b,
		maxQueues:        runtime.NumCPU() * defaultDynamicMaxQueuesMultiplier,
		idleCleanupAfter: defaultIdleCleanupAfter,
	}
}

// ShardedBuilder is the second stage of the builder chain after Sharded.
type ShardedBuilder struct {
	parent  *Builder
	workers int
}

// Build constructs an AsyncEngine that dispatches via fixed shards.
func (b *ShardedBuilder) Build() (*AsyncEngine, error) {
	if b.workers <= 0 {
		return nil, fmt.Errorf(
			"openpit/asyncengine: sharded workers must be > 0, got %d",
			b.workers,
		)
	}
	strategy := newShardedStrategy(b.parent.baseConfig(), b.workers)
	return newAsyncEngine(
		b.parent.driver, b.parent.stopUnderlying, strategy,
	), nil
}

// DynamicBuilder is the second stage of the builder chain after Dynamic.
type DynamicBuilder struct {
	parent           *Builder
	maxQueues        int
	idleCleanupAfter time.Duration
}

// MaxQueues caps the number of concurrent live per-account queues. Zero
// removes the cap; submit never fails for new accounts. The default cap
// is runtime.NumCPU() * 32.
//
// When the cap is reached, submitting for an unknown account returns
// ErrQueueLimit; submits for known accounts continue normally.
func (b *DynamicBuilder) MaxQueues(n int) *DynamicBuilder {
	b.maxQueues = n
	return b
}

// IdleCleanupAfter sets the idle duration after which a queue with an
// empty channel is retired. Zero disables cleanup; queues live until
// stop. Default is 5 minutes.
//
// The background scan runs at a fifth of d but never faster than the
// default cadence (1 minute). So for d below ~5 seconds the effective
// retire delay is floored at the scan period (~1 minute), and queues can
// outlive the requested idle window by up to that period.
func (b *DynamicBuilder) IdleCleanupAfter(d time.Duration) *DynamicBuilder {
	b.idleCleanupAfter = d
	return b
}

// Build constructs an AsyncEngine that creates per-account queues on
// demand and retires idle ones in the background.
func (b *DynamicBuilder) Build() (*AsyncEngine, error) {
	if b.maxQueues < 0 {
		return nil, fmt.Errorf(
			"openpit/asyncengine: maxQueues must be >= 0, got %d",
			b.maxQueues,
		)
	}
	strategy := newDynamicStrategy(
		b.parent.baseConfig(), b.maxQueues, b.idleCleanupAfter,
	)
	return newAsyncEngine(
		b.parent.driver, b.parent.stopUnderlying, strategy,
	), nil
}
