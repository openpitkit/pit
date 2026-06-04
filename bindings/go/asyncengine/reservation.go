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

	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/future"
	"go.openpit.dev/openpit/pretrade"
)

// AsyncReservation wraps a *pretrade.Reservation so that Commit,
// Rollback, Close, CommitAndClose, and RollbackAndClose are routed
// through the same per-account queue as the call that produced the
// reservation. This preserves the AccountSync invariant up to and
// including reservation finalization.
//
// Misusing the underlying reservation (for example Commit after Close,
// or a double Commit) panics on the worker goroutine, exactly as the
// synchronous pretrade.Reservation does; such misuse is a programmer
// error and is intentionally fatal. Commit has no error return by
// contract - a commit either applies or the call was a programmer error -
// so the async layer deliberately does not recover these panics: turning
// them into a resolved-with-error future would invent a failure mode the
// synchronous API does not have. This is by design, not an oversight.
type AsyncReservation struct {
	inner     *pretrade.Reservation
	engine    *AsyncEngine
	accountID param.AccountID
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

// Commit enqueues Commit on the underlying reservation. The reservation
// is not closed by this call; pair with Close or use CommitAndClose. If
// the task is aborted by a hard stop (the future resolves with
// ErrStopped) the underlying reservation is not released either, so the
// caller must still Close it (or use CommitAndClose) to avoid leaking the
// native handle.
func (r *AsyncReservation) Commit(ctx context.Context) *future.Future[struct{}] {
	return r.runVoid(ctx, reservationCommit, false)
}

// CommitAndClose enqueues Commit followed by Close.
func (r *AsyncReservation) CommitAndClose(ctx context.Context) *future.Future[struct{}] {
	return r.runVoid(ctx, reservationCommitAndClose, true)
}

// Rollback enqueues Rollback on the underlying reservation. Tolerates a
// closed reservation as a silent no-op. If the task is aborted by a hard
// stop (the future resolves with ErrStopped) the underlying reservation
// is not released either, so the caller must still Close it (or use
// RollbackAndClose) to avoid leaking the native handle.
func (r *AsyncReservation) Rollback(ctx context.Context) *future.Future[struct{}] {
	return r.runVoid(ctx, reservationRollback, false)
}

// RollbackAndClose enqueues Rollback followed by Close.
func (r *AsyncReservation) RollbackAndClose(ctx context.Context) *future.Future[struct{}] {
	return r.runVoid(ctx, reservationRollbackAndClose, true)
}

// Close enqueues Close on the underlying reservation. If Commit was not
// called first, the reserved state is rolled back implicitly.
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
	task := &reservationTask{f: f, res: r, op: op, abortCloses: abortCloses}
	if err := r.engine.strategy.submit(ctx, r.accountID, task); err != nil {
		if abortCloses {
			r.inner.Close()
		}
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
	switch t.op {
	case reservationCommit:
		t.res.inner.Commit()
	case reservationCommitAndClose:
		t.res.inner.CommitAndClose()
	case reservationRollback:
		t.res.inner.Rollback()
	case reservationRollbackAndClose:
		t.res.inner.RollbackAndClose()
	case reservationClose:
		t.res.inner.Close()
	}
	t.f.Resolve(struct{}{}, nil)
}

func (t *reservationTask) abort(err error) {
	if t.abortCloses {
		t.res.inner.Close()
	}
	t.f.Resolve(struct{}{}, err)
}
