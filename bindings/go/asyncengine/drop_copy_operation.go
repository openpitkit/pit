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

	"go.openpit.dev/openpit/accountadjustment"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/future"
	"go.openpit.dev/openpit/pretrade"
	"go.openpit.dev/openpit/reject"
)

// AsyncDropCopyOperation wraps a *pretrade.DropCopyOperation so that Commit,
// Rollback, Close, CommitAndClose, and RollbackAndClose are routed
// through the same per-account queue as the call that produced the
// operation. This preserves the AccountSync invariant up to and
// including drop-copy finalization.
//
// Finalization is idempotent. Accessors are serialized with finalization on
// this wrapper and report pretrade.ErrDropCopyOperationClosed after Close.
type AsyncDropCopyOperation struct {
	inner     *pretrade.DropCopyOperation
	engine    *AsyncEngine
	accountID param.AccountID
	mu        sync.Mutex
}

func (o *AsyncDropCopyOperation) runInner(op dropCopyOperationOp) {
	o.mu.Lock()
	defer o.mu.Unlock()
	switch op {
	case dropCopyOperationCommit:
		o.inner.Commit()
	case dropCopyOperationCommitAndClose:
		o.inner.CommitAndClose()
	case dropCopyOperationRollback:
		o.inner.Rollback()
	case dropCopyOperationRollbackAndClose:
		o.inner.RollbackAndClose()
	case dropCopyOperationClose:
		o.inner.Close()
	}
}

func newAsyncDropCopyOperation(
	inner *pretrade.DropCopyOperation,
	engine *AsyncEngine,
	accountID param.AccountID,
) *AsyncDropCopyOperation {
	return &AsyncDropCopyOperation{
		inner:     inner,
		engine:    engine,
		accountID: accountID,
	}
}

// AccountID returns the account identifier the operation is pinned to.
func (o *AsyncDropCopyOperation) AccountID() param.AccountID {
	return o.accountID
}

// Lock returns the operation's pre-trade lock snapshot.
func (o *AsyncDropCopyOperation) Lock() (pretrade.Lock, error) {
	o.mu.Lock()
	defer o.mu.Unlock()
	return o.inner.Lock()
}

// AccountAdjustments returns the operation's account-adjustment outcomes.
func (o *AsyncDropCopyOperation) AccountAdjustments() ([]accountadjustment.Outcome, error) {
	o.mu.Lock()
	defer o.mu.Unlock()
	return o.inner.AccountAdjustments()
}

// AccountBlock returns the first account block requested by the operation.
func (o *AsyncDropCopyOperation) AccountBlock() (*reject.AccountBlock, error) {
	o.mu.Lock()
	defer o.mu.Unlock()
	return o.inner.AccountBlock()
}

// IsAccountBlocked returns the apply-time blocked-state snapshot.
func (o *AsyncDropCopyOperation) IsAccountBlocked() (bool, error) {
	o.mu.Lock()
	defer o.mu.Unlock()
	return o.inner.IsAccountBlocked()
}

// Commit enqueues Commit on the underlying operation. The operation
// is not closed by this call; pair with Close or use CommitAndClose. If
// the task is aborted by a hard stop (the future resolves with
// ErrStopped) the underlying operation is not released either, so the
// caller must still Close it (or use CommitAndClose) to avoid leaking the
// native handle.
func (o *AsyncDropCopyOperation) Commit(ctx context.Context) *future.Future[struct{}] {
	return o.runVoid(ctx, dropCopyOperationCommit, false)
}

// CommitAndClose enqueues Commit followed by Close.
func (o *AsyncDropCopyOperation) CommitAndClose(ctx context.Context) *future.Future[struct{}] {
	return o.runVoid(ctx, dropCopyOperationCommitAndClose, true)
}

// Rollback enqueues Rollback on the underlying operation. Tolerates a
// closed operation as a silent no-op. If the task is aborted by a hard
// stop (the future resolves with ErrStopped) the underlying operation
// is not released either, so the caller must still Close it (or use
// RollbackAndClose) to avoid leaking the native handle.
func (o *AsyncDropCopyOperation) Rollback(ctx context.Context) *future.Future[struct{}] {
	return o.runVoid(ctx, dropCopyOperationRollback, false)
}

// RollbackAndClose enqueues Rollback followed by Close.
func (o *AsyncDropCopyOperation) RollbackAndClose(ctx context.Context) *future.Future[struct{}] {
	return o.runVoid(ctx, dropCopyOperationRollbackAndClose, true)
}

// Close enqueues Close on the underlying operation. If Commit was not
// called first, the prepared state is rolled back implicitly.
func (o *AsyncDropCopyOperation) Close(ctx context.Context) *future.Future[struct{}] {
	return o.runVoid(ctx, dropCopyOperationClose, true)
}

// dropCopyOperationOp identifies which underlying operation call a
// dropCopyOperationTask performs on its worker.
type dropCopyOperationOp uint8

const (
	dropCopyOperationCommit dropCopyOperationOp = iota
	dropCopyOperationCommitAndClose
	dropCopyOperationRollback
	dropCopyOperationRollbackAndClose
	dropCopyOperationClose
)

// runVoid is the shared submit/resolve plumbing for the simple methods
// above. abortCloses controls whether an aborted task closes the
// underlying operation as a safety net (true for Close-flavored
// methods so the operation never leaks).
func (o *AsyncDropCopyOperation) runVoid(
	ctx context.Context,
	op dropCopyOperationOp,
	abortCloses bool,
) *future.Future[struct{}] {
	f := future.New[struct{}]()
	task := &dropCopyOperationTask{f: f, operation: o, op: op, abortCloses: abortCloses}
	if abortCloses {
		_ = submitWithFailureHandoff(
			ctx, o.engine.strategy, o.accountID, task, func(err error) {
				o.engine.strategy.scheduleSubmitFailureCleanup(o.accountID, func() {
					o.runInner(dropCopyOperationClose)
					f.Resolve(struct{}{}, err)
				})
			},
		)
		return f
	}
	if err := o.engine.strategy.submit(ctx, o.accountID, task); err != nil {
		f.Resolve(struct{}{}, err)
	}
	return f
}

// dropCopyOperationTask carries one void drop-copy call (Commit, Rollback,
// Close, or their *AndClose variants) to its worker. abortCloses mirrors
// runVoid: when set, an aborted task closes the underlying operation so
// the native handle is never leaked.
type dropCopyOperationTask struct {
	f           *future.Future[struct{}]
	operation   *AsyncDropCopyOperation
	op          dropCopyOperationOp
	abortCloses bool
}

func (t *dropCopyOperationTask) run() {
	t.operation.runInner(t.op)
	t.f.Resolve(struct{}{}, nil)
}

func (t *dropCopyOperationTask) abort(err error) {
	if t.abortCloses {
		t.operation.runInner(dropCopyOperationClose)
	}
	t.f.Resolve(struct{}{}, err)
}
