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
	"fmt"
	"sync"

	"go.openpit.dev/openpit/accountadjustment"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/future"
	"go.openpit.dev/openpit/pretrade"
)

// AsyncReservation wraps a *pretrade.Reservation so that Commit, Rollback,
// Close, CommitAndClose, and RollbackAndClose are routed through the same
// per-account queue as the call that produced the reservation. This preserves
// the AccountSync invariant up to and including reservation finalization.
// Lock and AccountAdjustments are synchronous. Snapshot reads distinguish
// three states. While open, an admitted read completes before native
// finalization starts. While finalizing, a read returns
// ErrFinalizationInProgress immediately. That signal is transient, the read
// may succeed later, and the caller still owns the handle and owes it a
// terminal call. Once closed, reads return pretrade.ErrReservationClosed
// permanently. Reads resume after Commit or Rollback; closing operations leave
// the wrapper closed.
//
// Lifecycle calls are serialized by the wrapper. Sequential calls after
// resolution or close, including Commit after Close and repeated Commit,
// inherit the synchronous reservation's no-op behavior.
type AsyncReservation struct {
	inner       *pretrade.Reservation
	engine      *AsyncEngine
	accountID   param.AccountID
	lifecycleMu sync.Mutex
	methodMu    sync.Mutex
	mu          sync.Mutex
	readersDone chan struct{}
	readers     int
	state       reservationState
	closed      bool
}

type reservationState uint8

const (
	reservationOpen reservationState = iota
	reservationFinalizing
	reservationClosed
)

func (r *AsyncReservation) runInner(op reservationOp) {
	r.lifecycleMu.Lock()
	r.mu.Lock()
	r.state = reservationFinalizing
	var readersDone <-chan struct{}
	if r.readers != 0 {
		r.readersDone = make(chan struct{})
		readersDone = r.readersDone
	}
	r.mu.Unlock()
	if readersDone != nil {
		<-readersDone
	}
	closes := op == reservationCommitAndClose ||
		op == reservationRollbackAndClose || op == reservationClose
	defer func() {
		r.mu.Lock()
		if closes {
			r.closed = true
		}
		if r.closed {
			r.state = reservationClosed
		} else {
			r.state = reservationOpen
		}
		r.mu.Unlock()
		r.lifecycleMu.Unlock()
	}()
	switch op {
	case reservationCommit:
		r.inner.Commit()
	case reservationCommitAndClose:
		r.inner.CommitAndClose()
	case reservationRollback:
		r.inner.Rollback()
	case reservationRollbackAndClose:
		r.inner.RollbackAndClose()
	case reservationClose:
		r.inner.Close()
	}
}

func (r *AsyncReservation) beginRead() error {
	r.mu.Lock()
	defer r.mu.Unlock()
	if r.closed {
		return pretrade.ErrReservationClosed
	}
	switch r.state {
	case reservationOpen:
		r.readers++
		return nil
	case reservationFinalizing:
		return ErrFinalizationInProgress
	case reservationClosed:
		return pretrade.ErrReservationClosed
	default:
		return fmt.Errorf(
			"openpit/asyncengine: invalid reservation lifecycle state %d",
			r.state,
		)
	}
}

func (r *AsyncReservation) endRead() {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.readers--
	if r.readers == 0 && r.readersDone != nil {
		close(r.readersDone)
		r.readersDone = nil
	}
}

func newAsyncReservation(
	inner *pretrade.Reservation,
	engine *AsyncEngine,
	accountID param.AccountID,
) *AsyncReservation {
	return &AsyncReservation{
		inner:     inner,
		engine:    engine,
		accountID: accountID,
	}
}

// AccountID returns the account identifier the reservation is pinned to.
func (r *AsyncReservation) AccountID() param.AccountID {
	return r.accountID
}

// Lock returns the reservation's pre-trade lock snapshot. While open, it
// returns the snapshot. While finalizing, it returns
// ErrFinalizationInProgress with a zero Lock; this signal is transient and a
// later read may succeed. Once closed, it returns
// pretrade.ErrReservationClosed with a zero Lock permanently.
func (r *AsyncReservation) Lock() (pretrade.Lock, error) {
	if err := r.beginRead(); err != nil {
		return pretrade.Lock{}, err
	}
	defer r.endRead()
	r.methodMu.Lock()
	defer r.methodMu.Unlock()
	return r.inner.Lock()
}

// AccountAdjustments returns the reservation's account-adjustment outcomes.
// While open, it returns the outcomes. While finalizing, it returns
// ErrFinalizationInProgress with a nil slice; this signal is transient and a
// later read may succeed. Once closed, it returns
// pretrade.ErrReservationClosed with a nil slice permanently.
func (r *AsyncReservation) AccountAdjustments() ([]accountadjustment.Outcome, error) {
	if err := r.beginRead(); err != nil {
		return nil, err
	}
	defer r.endRead()
	r.methodMu.Lock()
	defer r.methodMu.Unlock()
	return r.inner.AccountAdjustments()
}

// Commit enqueues Commit on the underlying reservation. The reservation
// is not closed by this call; pair with Close or use CommitAndClose. If
// the task is aborted by a hard stop (the future resolves with
// ErrStopped) the underlying reservation is not released either, so the
// caller must still Close it (or use CommitAndClose) to avoid leaking the
// native handle.
// With a chain hook context for this engine, the future resolves with
// ErrReentrantLane; the caller retains the handle and must retry outside
// the hook.
func (r *AsyncReservation) Commit(ctx context.Context) *future.Future[struct{}] {
	return r.runVoid(ctx, reservationCommit, false)
}

// CommitAndClose enqueues Commit followed by Close. With a chain hook context
// for this engine, the future resolves with ErrReentrantLane; the caller
// retains the handle and must retry outside the hook.
func (r *AsyncReservation) CommitAndClose(ctx context.Context) *future.Future[struct{}] {
	return r.runVoid(ctx, reservationCommitAndClose, true)
}

// Rollback enqueues Rollback on the underlying reservation. Tolerates a
// closed reservation as a silent no-op. If the task is aborted by a hard
// stop (the future resolves with ErrStopped) the underlying reservation
// is not released either, so the caller must still Close it (or use
// RollbackAndClose) to avoid leaking the native handle.
// With a chain hook context for this engine, the future resolves with
// ErrReentrantLane; the caller retains the handle and must retry outside
// the hook.
func (r *AsyncReservation) Rollback(ctx context.Context) *future.Future[struct{}] {
	return r.runVoid(ctx, reservationRollback, false)
}

// RollbackAndClose enqueues Rollback followed by Close. With a chain hook
// context for this engine, the future resolves with ErrReentrantLane; the
// caller retains the handle and must retry outside the hook.
func (r *AsyncReservation) RollbackAndClose(ctx context.Context) *future.Future[struct{}] {
	return r.runVoid(ctx, reservationRollbackAndClose, true)
}

// Close enqueues Close on the underlying reservation. If Commit was not
// called first, the reserved state is rolled back implicitly.
// With a chain hook context for this engine, the future resolves with
// ErrReentrantLane; the caller retains the handle and must retry outside
// the hook.
func (r *AsyncReservation) Close(ctx context.Context) *future.Future[struct{}] {
	return r.runVoid(ctx, reservationClose, true)
}

// reservationOp identifies which underlying reservation call a
// reservationTask performs on its worker.
type reservationOp uint8

const (
	reservationCommit reservationOp = iota
	reservationCommitAndClose
	reservationRollback
	reservationRollbackAndClose
	reservationClose
)

// runVoid is the shared submit/resolve plumbing for the simple methods
// above. abortCloses controls whether an aborted task closes the
// underlying reservation as a safety net (true for Close-flavored
// methods so the reservation never leaks).
func (r *AsyncReservation) runVoid(
	ctx context.Context,
	op reservationOp,
	abortCloses bool,
) *future.Future[struct{}] {
	f := future.New[struct{}]()
	if r.engine.reentrantLane(ctx) {
		f.Resolve(struct{}{}, ErrReentrantLane)
		return f
	}
	task := &reservationTask{f: f, res: r, op: op, abortCloses: abortCloses}
	if abortCloses {
		_ = r.engine.strategy.submitWithFailureHandoff(
			ctx, accountRoutingKey(r.accountID), task, func(err error) {
				r.engine.strategy.scheduleSubmitFailureCleanup(
					accountRoutingKey(r.accountID), func() {
						r.runInner(reservationClose)
						f.Resolve(struct{}{}, err)
					},
				)
			},
		)
		return f
	}
	if err := r.engine.strategy.submit(ctx, accountRoutingKey(r.accountID), task); err != nil {
		f.Resolve(struct{}{}, err)
	}
	return f
}

// reservationTask carries one void reservation call (Commit, Rollback,
// Close, or their *AndClose variants) to its worker. abortCloses mirrors
// runVoid: when set, an aborted task closes the underlying reservation so
// the native handle is never leaked.
type reservationTask struct {
	f           *future.Future[struct{}]
	res         *AsyncReservation
	op          reservationOp
	abortCloses bool
}

func (t *reservationTask) run() {
	t.res.runInner(t.op)
	t.f.Resolve(struct{}{}, nil)
}

func (t *reservationTask) abort(err error) {
	if t.abortCloses {
		t.res.runInner(reservationClose)
	}
	t.f.Resolve(struct{}{}, err)
}
