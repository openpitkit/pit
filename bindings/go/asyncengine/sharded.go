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
	"math/bits"

	"go.openpit.dev/openpit/param"
)

// shardedStrategy fans accounts across a fixed pool of worker channels
// chosen at build time. Routing is a single multiply-shift operation per
// submit and the send path takes no per-queue lock; a short shared
// read-lock is taken only to order against stop and does not serialize
// concurrent submits against each other. This makes it the cheapest
// option when the active account set is large and roughly evenly
// distributed.
//
// Caveat: a single account that produces a hot stream of orders saturates
// one shard while the others stay idle. For workloads with skewed
// account activity, prefer the Dynamic strategy.
type shardedStrategy struct {
	base
	shards []*keyQueue
}

// fibonacciHashMultiplier mixes uint64 inputs into a uniform shard index.
// 2^64 / phi rounded to the nearest odd integer, the classic Fibonacci
// hash constant used by Knuth in TAOCP, vol. 3.
const fibonacciHashMultiplier uint64 = 11400714819323198485

func newShardedStrategy(cfg baseConfig, shardCount int) *shardedStrategy {
	s := &shardedStrategy{
		base:   newBase(cfg, false),
		shards: make([]*keyQueue, shardCount),
	}
	for i := range s.shards {
		s.shards[i] = newKeyQueue(s.cfg.queueCapacity)
		s.workersWG.Add(1)
		go s.worker(s.shards[i])
	}
	return s
}

func (s *shardedStrategy) shardFor(accountID param.AccountID) *keyQueue {
	// Lemire multiply-shift over the HIGH bits of the Fibonacci mix: the low
	// bits of an odd-constant multiply are poorly mixed, so map via the high
	// half of the 128-bit product instead of a modulo. hi is in [0, len).
	h := uint64(accountID.Handle()) * fibonacciHashMultiplier
	hi, _ := bits.Mul64(h, uint64(len(s.shards)))
	return s.shards[hi]
}

func (s *shardedStrategy) submit(
	ctx context.Context,
	accountID param.AccountID,
	task pendingTask,
) error {
	q := s.shardFor(accountID)
	// Sharded queues are never retired, so the send never reports
	// errQueueRetired; a stopped strategy short-circuits with ErrStopped.
	return s.submitToShard(ctx, q, accountID, task)
}

func (s *shardedStrategy) stopGraceful(ctx context.Context) error {
	s.signalStopOnce()
	if err := s.waitInFlightSubmits(ctx); err != nil {
		return err
	}
	s.closeQueueChannels(s.shards)
	return s.waitWorkers(ctx)
}

func (s *shardedStrategy) stopHard(ctx context.Context) error {
	s.signalHardStopOnce()
	s.signalStopOnce()
	if err := s.waitInFlightSubmits(ctx); err != nil {
		return err
	}
	s.closeQueueChannels(s.shards)
	return s.waitWorkers(ctx)
}
