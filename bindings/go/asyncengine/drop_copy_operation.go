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
	"go.openpit.dev/openpit/reject"
)

// AsyncDropCopyOperation wraps a *pretrade.DropCopyOperation so that Commit,
// Rollback, Close, CommitAndClose, and RollbackAndClose are routed
// through the same per-account queue as the call that produced the
// operation. This preserves the AccountSync invariant up to and
// including drop-copy finalization. Lock, AccountAdjustments, AccountBlock,
// and IsAccountBlocked are synchronous. Finalization is idempotent. Snapshot
// reads distinguish three states. While open, an admitted read completes
// before native finalization starts. While finalizing, a read returns
// ErrFinalizationInProgress immediately. That signal is transient, the read
// may succeed later, and the caller still owns the handle and owes it a
// terminal call. Once closed, reads return
// pretrade.ErrDropCopyOperationClosed permanently. Reads resume after Commit
// or Rollback; closing operations leave the wrapper closed.
type AsyncDropCopyOperation struct {
	inner       *pretrade.DropCopyOperation
	engine      *AsyncEngine
	accountID   param.AccountID
	lifecycleMu sync.Mutex
	methodMu    sync.Mutex
	mu          sync.Mutex
	readersDone chan struct{}
	readers     int
	state       dropCopyOperationState
	closed      bool
}

type dropCopyOperationState uint8

const (
	dropCopyOperationOpen dropCopyOperationState = iota
	dropCopyOperationFinalizing
	dropCopyOperationClosed
)

func (o *AsyncDropCopyOperation) runInner(op dropCopyOperationOp) {
	o.lifecycleMu.Lock()
	o.mu.Lock()
	o.state = dropCopyOperationFinalizing
	var readersDone <-chan struct{}
	if o.readers != 0 {
		o.readersDone = make(chan struct{})
		readersDone = o.readersDone
	}
	o.mu.Unlock()
	if readersDone != nil {
		<-readersDone
	}
	closes := op == dropCopyOperationCommitAndClose ||
		op == dropCopyOperationRollbackAndClose || op == dropCopyOperationClose
	defer func() {
		o.mu.Lock()
		if closes {
			o.closed = true
		}
		if o.closed {
			o.state = dropCopyOperationClosed
		} else {
			o.state = dropCopyOperationOpen
		}
		o.mu.Unlock()
		o.lifecycleMu.Unlock()
	}()
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

func (o *AsyncDropCopyOperation) beginRead() error {
	o.mu.Lock()
	defer o.mu.Unlock()
	if o.closed {
		return pretrade.ErrDropCopyOperationClosed
	}
	switch o.state {
	case dropCopyOperationOpen:
		o.readers++
		return nil
	case dropCopyOperationFinalizing:
		return ErrFinalizationInProgress
	case dropCopyOperationClosed:
		return pretrade.ErrDropCopyOperationClosed
	default:
		return fmt.Errorf(
			"openpit/asyncengine: invalid drop-copy operation lifecycle state %d",
			o.state,
		)
	}
}

func (o *AsyncDropCopyOperation) endRead() {
	o.mu.Lock()
	defer o.mu.Unlock()
	o.readers--
	if o.readers == 0 && o.readersDone != nil {
		close(o.readersDone)
		o.readersDone = nil
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

// Lock returns the operation's pre-trade lock snapshot. While open, it returns
// the snapshot. While finalizing, it returns ErrFinalizationInProgress with a
// zero Lock; this signal is transient and a later read may succeed. Once
// closed, it returns pretrade.ErrDropCopyOperationClosed with a zero Lock
// permanently.
func (o *AsyncDropCopyOperation) Lock() (pretrade.Lock, error) {
	if err := o.beginRead(); err != nil {
		return pretrade.Lock{}, err
	}
	defer o.endRead()
	o.methodMu.Lock()
	defer o.methodMu.Unlock()
	return o.inner.Lock()
}

// AccountAdjustments returns the operation's account-adjustment outcomes.
// While open, it returns the outcomes. While finalizing, it returns
// ErrFinalizationInProgress with a nil slice; this signal is transient and a
// later read may succeed. Once closed, it returns
// pretrade.ErrDropCopyOperationClosed with a nil slice permanently.
func (o *AsyncDropCopyOperation) AccountAdjustments() ([]accountadjustment.Outcome, error) {
	if err := o.beginRead(); err != nil {
		return nil, err
	}
	defer o.endRead()
	o.methodMu.Lock()
	defer o.methodMu.Unlock()
	return o.inner.AccountAdjustments()
}

// AccountBlock returns the first account block requested by the operation, or
// nil with a nil error when no block was requested. While open, it returns the
// snapshot. While finalizing, it returns ErrFinalizationInProgress with a nil
// block; this signal is transient and a later read may succeed. Once closed,
// it returns pretrade.ErrDropCopyOperationClosed with a nil block permanently.
func (o *AsyncDropCopyOperation) AccountBlock() (*reject.AccountBlock, error) {
	if err := o.beginRead(); err != nil {
		return nil, err
	}
	defer o.endRead()
	o.methodMu.Lock()
	defer o.methodMu.Unlock()
	return o.inner.AccountBlock()
}

// IsAccountBlocked returns the apply-time blocked-state snapshot. While open,
// it returns the snapshot. While finalizing, it returns
// ErrFinalizationInProgress with false; this signal is transient and a later
// read may succeed. Once closed, it returns
// pretrade.ErrDropCopyOperationClosed with false permanently.
func (o *AsyncDropCopyOperation) IsAccountBlocked() (bool, error) {
	if err := o.beginRead(); err != nil {
		return false, err
	}
	defer o.endRead()
	o.methodMu.Lock()
	defer o.methodMu.Unlock()
	return o.inner.IsAccountBlocked()
}

// Commit enqueues Commit on the underlying operation. The operation
// is not closed by this call; pair with Close or use CommitAndClose. If
// the task is aborted by a hard stop (the future resolves with
// ErrStopped) the underlying operation is not released either, so the
// caller must still Close it (or use CommitAndClose) to avoid leaking the
// native handle.
// With a chain hook context for this engine, the future resolves with
// ErrReentrantLane; the caller retains the handle and must retry outside
// the hook.
func (o *AsyncDropCopyOperation) Commit(ctx context.Context) *future.Future[struct{}] {
	return o.runVoid(ctx, dropCopyOperationCommit, false)
}

// CommitAndClose enqueues Commit followed by Close. With a chain hook context
// for this engine, the future resolves with ErrReentrantLane; the caller
// retains the handle and must retry outside the hook.
func (o *AsyncDropCopyOperation) CommitAndClose(ctx context.Context) *future.Future[struct{}] {
	return o.runVoid(ctx, dropCopyOperationCommitAndClose, true)
}

// Rollback enqueues Rollback on the underlying operation. Tolerates a
// closed operation as a silent no-op. If the task is aborted by a hard
// stop (the future resolves with ErrStopped) the underlying operation
// is not released either, so the caller must still Close it (or use
// RollbackAndClose) to avoid leaking the native handle.
// With a chain hook context for this engine, the future resolves with
// ErrReentrantLane; the caller retains the handle and must retry outside
// the hook.
func (o *AsyncDropCopyOperation) Rollback(ctx context.Context) *future.Future[struct{}] {
	return o.runVoid(ctx, dropCopyOperationRollback, false)
}

// RollbackAndClose enqueues Rollback followed by Close. With a chain hook
// context for this engine, the future resolves with ErrReentrantLane; the
// caller retains the handle and must retry outside the hook.
func (o *AsyncDropCopyOperation) RollbackAndClose(ctx context.Context) *future.Future[struct{}] {
	return o.runVoid(ctx, dropCopyOperationRollbackAndClose, true)
}

// Close enqueues Close on the underlying operation. If Commit was not
// called first, the prepared state is rolled back implicitly.
// With a chain hook context for this engine, the future resolves with
// ErrReentrantLane; the caller retains the handle and must retry outside
// the hook.
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
	if o.engine.reentrantLane(ctx) {
		f.Resolve(struct{}{}, ErrReentrantLane)
		return f
	}
	task := &dropCopyOperationTask{f: f, operation: o, op: op, abortCloses: abortCloses}
	if abortCloses {
		_ = o.engine.strategy.submitWithFailureHandoff(
			ctx, accountRoutingKey(o.accountID), task, func(err error) {
				o.engine.strategy.scheduleSubmitFailureCleanup(
					accountRoutingKey(o.accountID), func() {
						o.runInner(dropCopyOperationClose)
						f.Resolve(struct{}{}, err)
					},
				)
			},
		)
		return f
	}
	if err := o.engine.strategy.submit(ctx, accountRoutingKey(o.accountID), task); err != nil {
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
