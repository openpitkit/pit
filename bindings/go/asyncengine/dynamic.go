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
	"fmt"
	"sync"
	"time"

	"go.openpit.dev/openpit/param"
)

// dynamicStrategy lazily creates one keyQueue per active account. Idle
// queues are retired by a background cleanup goroutine. Total queue count
// is bounded by maxQueues (0 means unlimited).
//
// Strengths: every account is fully isolated, no hot-shard bottlenecks,
// memory scales with active set rather than total population.
// Weaknesses: a small read-mutex hit on the submit hot path; cleanup
// overhead in the background.
type dynamicStrategy struct {
	base
	mu sync.RWMutex
	// stopping is set under mu by stop before the queue snapshot is taken.
	// It fences getOrCreate so no worker is started after the snapshot,
	// guaranteeing every workersWG.Add happens-before workersWG.Wait.
	stopping         bool
	queues           map[param.AccountID]*keyQueue
	maxQueues        int
	idleCleanupAfter time.Duration
	cleanupPeriod    time.Duration
	cleanupStopCh    chan struct{}
	cleanupDoneCh    chan struct{}
	cleanupStopOnce  sync.Once
}

func newDynamicStrategy(
	cfg baseConfig,
	maxQueues int,
	idleCleanupAfter time.Duration,
) *dynamicStrategy {
	if idleCleanupAfter < 0 {
		idleCleanupAfter = 0
	}
	// Scan at a fraction of the idle window, but never tighter than the
	// default cadence.
	const idleCleanupPeriodDivisor = 5
	period := idleCleanupAfter / idleCleanupPeriodDivisor
	if period < time.Second {
		period = defaultIdleCleanupPeriod
	}
	// Idle tracking (q.lastActive/q.pending bookkeeping) is only needed when
	// the cleanup loop can actually retire a queue.
	tracksIdle := idleCleanupAfter > 0
	s := &dynamicStrategy{
		base:             newBase(cfg, tracksIdle),
		queues:           map[param.AccountID]*keyQueue{},
		maxQueues:        maxQueues,
		idleCleanupAfter: idleCleanupAfter,
		cleanupPeriod:    period,
		cleanupStopCh:    make(chan struct{}),
		cleanupDoneCh:    make(chan struct{}),
	}
	if tracksIdle {
		go s.cleanupLoop()
	} else {
		close(s.cleanupDoneCh)
	}
	return s
}

// getOrCreate returns the queue for accountID, creating it on first use.
// Returns ErrQueueLimit when maxQueues is positive and would be exceeded.
// When a queue is created, created is true and total is the live queue
// count snapshotted under s.mu; the caller fires OnQueueCreated after the
// lock is released to keep user callbacks off the critical section.
func (s *dynamicStrategy) getOrCreate(
	accountID param.AccountID,
) (q *keyQueue, created bool, total int, err error) {
	s.mu.RLock()
	q, ok := s.queues[accountID]
	s.mu.RUnlock()
	if ok && !q.closed.Load() {
		return q, false, 0, nil
	}

	s.mu.Lock()
	defer s.mu.Unlock()
	if s.stopping {
		return nil, false, 0, ErrStopped
	}
	if q, ok := s.queues[accountID]; ok && !q.closed.Load() {
		return q, false, 0, nil
	}
	if s.maxQueues > 0 && len(s.queues) >= s.maxQueues {
		return nil, false, 0, fmt.Errorf(
			"%w: max=%d", ErrQueueLimit, s.maxQueues,
		)
	}
	q = newKeyQueue(s.cfg.queueCapacity)
	s.queues[accountID] = q
	s.workersWG.Add(1)
	go s.worker(q)
	return q, true, len(s.queues), nil
}

func (s *dynamicStrategy) submit(
	ctx context.Context,
	accountID param.AccountID,
	task pendingTask,
) error {
	for {
		if s.isStopped() {
			return ErrStopped
		}
		q, created, total, err := s.getOrCreate(accountID)
		if err != nil {
			return err
		}
		if created {
			// Fire the callback after s.mu is released by getOrCreate to
			// avoid deadlock with a reentrant observer and tail latency.
			s.cfg.observer.OnQueueCreated(accountID, total)
		}
		err = s.submitToQueue(ctx, q, accountID, task)
		if err == nil {
			return nil
		}
		if errors.Is(err, errQueueRetired) {
			// Queue was retired between lookup and send; loop and create
			// a fresh one.
			continue
		}
		return err
	}
}

func (s *dynamicStrategy) cleanupLoop() {
	defer close(s.cleanupDoneCh)
	ticker := time.NewTicker(s.cleanupPeriod)
	defer ticker.Stop()
	for {
		select {
		case <-s.cleanupStopCh:
			return
		case <-ticker.C:
			s.cleanupIdle()
		}
	}
}

// idleCandidate pairs an account with the queue snapshotted as idle so the
// retirement phase can verify the map still maps to the same queue.
type idleCandidate struct {
	accountID param.AccountID
	q         *keyQueue
}

// cleanupIdle retires queues that have been empty and untouched longer
// than idleCleanupAfter. Producers signal "active" via q.touch() while
// holding q.gate.RLock; cleanup acquires q.gate.WLock under TryLock to
// avoid blocking on a busy queue.
//
// A queue is retired only when q.pending == 0, i.e. no task is buffered or
// running. Producers bump q.pending under q.gate.RLock and the worker
// clears it after the task completes, so a queue executing a long task is
// never retired even once its channel has drained.
//
// The scan that picks candidates runs under s.mu.RLock so it never stalls
// submits to existing queues; each retirement then takes s.mu.Lock only
// briefly, re-checking the map entry and the empty+idle condition under
// both the gate and the lock before deleting.
func (s *dynamicStrategy) cleanupIdle() {
	if s.isStopped() {
		return
	}
	cutoff := time.Now().Add(-s.idleCleanupAfter)

	s.mu.RLock()
	var candidates []idleCandidate
	for accountID, q := range s.queues {
		if q.pending.Load() == 0 && len(q.ch) == 0 &&
			!q.lastActiveAt().After(cutoff) {
			candidates = append(candidates, idleCandidate{accountID, q})
		}
	}
	s.mu.RUnlock()

	for _, c := range candidates {
		if removed, remaining := s.retireIfIdle(c, cutoff); removed {
			// Fire the callback after s.mu is released by retireIfIdle to
			// avoid deadlock with a reentrant observer and tail latency.
			s.cfg.observer.OnQueueRemoved(c.accountID, remaining)
		}
	}
}

// retireIfIdle retires c.q under s.mu.Lock if it is still the mapped queue
// and still empty and idle. Returns removed=false without effect otherwise.
// On retirement removed is true and remaining is the live queue count
// snapshotted under s.mu; the caller fires OnQueueRemoved after the lock is
// released to keep user callbacks off the critical section.
func (s *dynamicStrategy) retireIfIdle(
	c idleCandidate, cutoff time.Time,
) (removed bool, remaining int) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if cur, ok := s.queues[c.accountID]; !ok || cur != c.q {
		return false, 0
	}
	if c.q.pending.Load() != 0 || len(c.q.ch) > 0 ||
		c.q.lastActiveAt().After(cutoff) {
		return false, 0
	}
	if !c.q.gate.TryLock() {
		return false, 0
	}
	if c.q.pending.Load() != 0 || len(c.q.ch) > 0 ||
		c.q.lastActiveAt().After(cutoff) {
		c.q.gate.Unlock()
		return false, 0
	}
	c.q.closed.Store(true)
	close(c.q.quit)
	c.q.gate.Unlock()
	delete(s.queues, c.accountID)
	return true, len(s.queues)
}

// markStoppingAndSnapshot fences getOrCreate against starting new workers
// and returns every live queue, both under the same s.mu critical section.
// Any worker whose workersWG.Add has happened is therefore in the returned
// snapshot; any later getOrCreate observes stopping and returns ErrStopped
// without starting a worker. Idempotent across a repeated stop.
func (s *dynamicStrategy) markStoppingAndSnapshot() []*keyQueue {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.stopping = true
	queues := make([]*keyQueue, 0, len(s.queues))
	for _, q := range s.queues {
		queues = append(queues, q)
	}
	return queues
}

func (s *dynamicStrategy) stopGraceful(ctx context.Context) error {
	s.stopCleanup()
	s.signalStopOnce()
	if err := s.waitInFlightSubmits(ctx); err != nil {
		return err
	}
	s.closeQueueChannels(s.markStoppingAndSnapshot())
	return s.waitWorkers(ctx)
}

func (s *dynamicStrategy) stopHard(ctx context.Context) error {
	s.stopCleanup()
	s.signalHardStopOnce()
	s.signalStopOnce()
	if err := s.waitInFlightSubmits(ctx); err != nil {
		return err
	}
	s.closeQueueChannels(s.markStoppingAndSnapshot())
	return s.waitWorkers(ctx)
}

func (s *dynamicStrategy) stopCleanup() {
	s.cleanupStopOnce.Do(func() { close(s.cleanupStopCh) })
	<-s.cleanupDoneCh
}
