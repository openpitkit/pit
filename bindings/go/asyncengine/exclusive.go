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
	"sync"
	"sync/atomic"
)

// Exclusive runs fn on the calling goroutine while no lane of this engine runs
// a task and no off-lane handle release is in progress, then lets the lanes
// continue. It is the way to run the engine's exclusive operations - account
// group membership, group currency, the SpotFunds retune, and account
// retirement - through driver: under AccountSync the engine takes no lock for
// them, so nothing else may be in flight while they run.
//
// Ordinary tasks pay nothing for this method. Each lane is stopped by a
// barrier behind every task whose Submit returned before the section began;
// a Submit still in flight may land on either side. Work submitted during the
// section waits behind the barrier, with the usual back-pressure on a full queue.
//
// ctx bounds only the wait before fn starts and must be cancellable: a ctx
// whose Done is nil is refused with ErrExclusiveContext, because an unbounded
// wait could hang every lane. Once fn starts, the section holds until fn
// returns or panics. A cancelled ctx, a stop signal, or a hard stop before fn
// starts ends the section with ctx.Err() or ErrStopped without calling fn. A
// section started before a stop finishes before the stop releases the engine.
//
// An off-lane handle release that has no lane to queue behind waits for the
// section, which runs it on the calling goroutine before the lanes continue.
// A panic in fn or in such a release propagates after the lanes continue;
// when both panic, fn's panic is the one that propagates. A release must not
// call runtime.Goexit: that ends the drain, leaves the section installed, and
// later postponed releases are dropped.
//
// Hooks, Submit closures, observer callbacks, and fn must not synchronously
// call back into this AsyncEngine. fn receives the caller's ctx and must not
// wait for this engine's tasks, which queue behind the stopped lanes. A task
// waiting for another lane's task delays its own lane's stop until ctx ends.
//
// Requests, reservations, and drop-copy operations opened before the section
// stay open across it, and their next step sees the state fn left. Raw
// handles that fn opens through driver must be finished before fn returns.
//
// Exclusive returns the error fn returned, or the error that ended the
// section before fn ran.
func (e *AsyncEngine) Exclusive(
	ctx context.Context,
	fn func(ctx context.Context, driver Driver) error,
) error {
	if ctx.Done() == nil {
		return ErrExclusiveContext
	}
	var fnErr error
	err := e.strategy.exclusive(ctx, func() {
		fnErr = fn(ctx, e.driver)
	})
	if err != nil {
		return err
	}
	return fnErr
}

// exclusiveSection is one Exclusive call in progress.
//
// A lane stops by running a barrier task from the mandatory backlog of its
// queue. The barrier reports through parked and then waits for release, so a
// parked worker runs nothing else until the section ends. Those channel
// operations order every lane's earlier work before fn and fn before the
// lanes' later work.
type exclusiveSection struct {
	parked    chan struct{}
	release   chan struct{}
	aborted   chan struct{}
	abortOnce sync.Once
	// expected counts the barriers that must park before fn runs: one per
	// snapshotted queue that took a barrier, plus every Dynamic queue created
	// while the section is installed, which starts with its barrier.
	expected atomic.Int64
	mu       sync.Mutex
	// deferred holds the off-lane handle releases that arrived while the
	// section was installed. The section runs them on its own goroutine before
	// it releases the lanes.
	deferred []func()
}

func newExclusiveSection() *exclusiveSection {
	return &exclusiveSection{
		parked:  make(chan struct{}),
		release: make(chan struct{}),
		aborted: make(chan struct{}),
	}
}

// postpone records an off-lane cleanup. The caller holds the strategy lock
// under which the section was installed, so the cleanup either sees the
// section here or runs after the section was withdrawn.
func (s *exclusiveSection) postpone(cleanup func()) {
	s.mu.Lock()
	s.deferred = append(s.deferred, cleanup)
	s.mu.Unlock()
}

// takeDeferredLocked hands over the postponed cleanups; the caller holds s.mu.
func (s *exclusiveSection) takeDeferredLocked() []func() {
	deferred := s.deferred
	s.deferred = nil
	return deferred
}

// exclusiveBarrierTask stops one worker for a section.
type exclusiveBarrierTask struct {
	section *exclusiveSection
}

func (t exclusiveBarrierTask) run() {
	select {
	case t.section.parked <- struct{}{}:
	case <-t.section.release:
		return
	}
	<-t.section.release
}

// abort runs instead of run under a hard stop: the section cannot stop every
// lane and fails. The worker still holds its lane until the section ends, so
// the releases the section runs before it lets the lanes go cannot overlap
// this lane's aborted work.
func (t exclusiveBarrierTask) abort(error) {
	t.section.abortOnce.Do(func() { close(t.section.aborted) })
	<-t.section.release
}

func exclusiveRoutingKey() routingKey {
	return routingKey{kind: routingKeyExclusive}
}

// exclusiveHost is the strategy-specific half of Exclusive.
type exclusiveHost interface {
	// installExclusive publishes section to the strategy's slow paths and
	// returns the queues that need a barrier and the done channels of the
	// off-lane cleanups already running.
	installExclusive(section *exclusiveSection) ([]*keyQueue, []<-chan struct{})
	// withdrawExclusive removes section from the slow paths unless it holds
	// postponed cleanups; then it keeps the section installed and hands them
	// over, so none can slip past the drain.
	withdrawExclusive(section *exclusiveSection) (deferred []func(), withdrawn bool)
}

// runExclusive implements Exclusive for both strategies.
func (b *base) runExclusive(ctx context.Context, host exclusiveHost, run func()) error {
	select {
	case b.exclusiveTurn <- struct{}{}:
	case <-ctx.Done():
		return ctx.Err()
	case <-b.stopCh:
		return ErrStopped
	}
	defer func() { <-b.exclusiveTurn }()

	if !b.beginSection() {
		return ErrStopped
	}
	defer b.sections.Done()

	section := newExclusiveSection()
	queues, running := host.installExclusive(section)
	// fnRunning stays true while fn panics, so a release panic below never
	// replaces fn's.
	fnRunning := false
	// Whether or not fn ran, the postponed releases run here while no lane can
	// start later work for their accounts, and new ones keep landing in the
	// section until it can be withdrawn empty. The lanes go only then, even
	// when a release panics.
	defer func() {
		defer close(section.release)
		var panicked any
		for {
			deferred, withdrawn := host.withdrawExclusive(section)
			if withdrawn {
				break
			}
			for _, cleanup := range deferred {
				if recovered := runRecovered(cleanup); recovered != nil && panicked == nil {
					panicked = recovered
				}
			}
		}
		if panicked != nil && !fnRunning {
			panic(panicked)
		}
	}()

	for _, done := range running {
		select {
		case <-done:
		case <-ctx.Done():
			return ctx.Err()
		case <-b.stopCh:
			return ErrStopped
		}
	}
	for _, q := range queues {
		if b.enqueueBarrier(q, section) {
			section.expected.Add(1)
		}
	}
	for parked := int64(0); parked < section.expected.Load(); {
		select {
		case <-section.parked:
			parked++
		case <-section.aborted:
			return ErrStopped
		case <-ctx.Done():
			return ctx.Err()
		case <-b.stopCh:
			return ErrStopped
		}
	}
	// The waits above may end without reading ctx or stopCh: with nothing to
	// wait for, or when another ready case won. A stop also closes lanes before
	// it waits for sections, and the snapshot skips closed lanes, so this
	// check is what keeps fn away from a lane that stop closed while busy.
	select {
	case <-ctx.Done():
		return ctx.Err()
	case <-b.stopCh:
		return ErrStopped
	default:
	}
	fnRunning = true
	run()
	fnRunning = false
	return nil
}

// runRecovered runs cleanup and returns what it panicked with, if anything.
func runRecovered(cleanup func()) (recovered any) {
	defer func() { recovered = recover() }()
	cleanup()
	return nil
}

// beginSection registers a section against stop under the same lock that
// signalStopOnce takes, so a stop either refuses the section or waits for it.
func (b *base) beginSection() bool {
	b.submitMu.RLock()
	defer b.submitMu.RUnlock()
	if b.isStopped() {
		return false
	}
	b.sections.Add(1)
	return true
}

// waitSections blocks until every section registered before stop has ended,
// or ctx fires. A Dynamic engine with no live queue has no worker to hold
// stop back, so this is what keeps the engine from being released under fn.
func (b *base) waitSections(ctx context.Context) error {
	done := make(chan struct{})
	go func() {
		b.sections.Wait()
		close(done)
	}()
	select {
	case <-done:
		return nil
	case <-ctx.Done():
		return ctx.Err()
	}
}

// enqueueBarrier places a section barrier in q's mandatory backlog. The
// backlog is unbounded, so a full channel never blocks it. Its settlement
// target keeps it behind accepted lane work and earlier backlog entries;
// in-flight submits may land on either side. Later abandoned admissions do
// not advance it. It reports false for a queue that takes no more work.
func (b *base) enqueueBarrier(q *keyQueue, section *exclusiveSection) bool {
	return b.enqueueMandatory(
		q,
		queuedTask{task: exclusiveBarrierTask{section: section}, key: exclusiveRoutingKey()},
		false,
		false,
	)
}
