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

package pretrade

import (
	"errors"

	"go.openpit.dev/openpit/accountadjustment"
	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/reject"
)

// ErrDropCopyOperationClosed is returned by DropCopyOperation accessors once
// the operation has been released.
var ErrDropCopyOperationClosed = errors.New("drop-copy operation already closed")

// DropCopyOperation holds the state a successful drop-copy evaluation prepared
// for a historical order.
//
// Lifecycle: the caller owns the operation and must resolve it exactly once
// with CommitAndClose, RollbackAndClose, or Close. Nothing releases it on the
// caller's behalf - there is no finalizer, because resolving an operation
// applies or unwinds engine state and that call must stay on a goroutine the
// caller serializes, never the garbage collector's. An operation that is
// dropped without Close leaks both the native handle and the state it holds
// prepared for the remainder of the process. Account-control operations and
// spent rate-limit attempts are applied before ApplyDropCopy returns and stay
// applied whichever way the operation is resolved.
//
// Concurrency: callers must serialize methods on the same operation. Commit
// and Rollback are mutually exclusive: whichever runs first resolves the
// operation, and any later Commit or Rollback on the same value has no effect.
// Every accessor reports ErrDropCopyOperationClosed once Close releases the
// handle, because the zero value it would otherwise return reads as a verdict.
//
// Finalization: a mutation commit or rollback callback has no right to fail. By
// the time it runs the decision is already made and the state it finalizes was
// applied eagerly, so there is nothing left to compensate. Commit and Rollback
// are void and stay void when a callback reports failure, and that failure is
// not discarded either: it arms the engine kill switch. Every mutation
// registered from Go belongs to a custom policy whose state reach the engine
// cannot bound, so the kill switch blocks EVERY account, not only this order's.
// Nothing reports it to the finalizing caller; it surfaces when the next
// pre-trade call is rejected with SystemUnavailable, and an operator lifts it
// with UnblockAll on the engine's Accounts accessor, which leaves accounts and
// groups blocked individually untouched. A panicking callback is recovered at
// the SDK boundary and reported to the core as exactly such a failure.
type DropCopyOperation struct {
	handle   native.PretradeDropCopyOperation
	resolved bool
}

// NewDropCopyOperationFromHandle creates a DropCopyOperation from a native
// handle.
func NewDropCopyOperationFromHandle(
	handle native.PretradeDropCopyOperation,
) *DropCopyOperation {
	return &DropCopyOperation{handle: handle}
}

// Close releases the operation.
//
// If neither Commit nor Rollback was called beforehand, Close rolls back
// any pending mutations implicitly; explicit resolution is only required
// when the caller needs to observe commit-time side effects. A rollback
// callback that fails during that implicit rollback arms the engine kill
// switch, exactly as in Rollback.
//
// Idempotency: safe to call more than once; subsequent calls are no-ops.
func (o *DropCopyOperation) Close() {
	o.close()
}

// Commit finalizes the operation and applies its prepared state permanently.
//
// Commit does not close the operation; call Close afterwards, or use
// CommitAndClose. A call after the operation was resolved or closed is a
// no-op. A mutation commit callback that reports failure does not fail this
// call; it arms the engine kill switch instead, as described on
// DropCopyOperation.
func (o *DropCopyOperation) Commit() {
	o.commit()
}

// CommitAndClose commits the operation and then releases it. A call after the
// operation was resolved or closed is a no-op.
//
// Callers must not interleave Commit, Rollback, or Close concurrently.
func (o *DropCopyOperation) CommitAndClose() {
	o.commit()
	o.close()
}

// Rollback cancels the operation and compensates its prepared state.
//
// Rollback does not close the operation; call Close afterwards, or use
// RollbackAndClose. A call after the operation was resolved or closed is a
// no-op, which permits unconditional cleanup in deferred error paths. A
// mutation rollback callback that reports failure does not fail this call; it
// arms the engine kill switch instead, as described on DropCopyOperation.
func (o *DropCopyOperation) Rollback() {
	o.rollback()
}

// RollbackAndClose rolls back the operation and then releases it.
// Safe to call after Close; both steps become no-ops.
//
// Callers must not interleave Commit, Rollback, or Close concurrently.
func (o *DropCopyOperation) RollbackAndClose() {
	o.rollback()
	o.close()
}

func (o *DropCopyOperation) commit() {
	if o.resolved || o.handle == nil {
		return
	}
	o.resolved = true
	native.PretradeDropCopyOperationCommit(o.handle)
}

func (o *DropCopyOperation) rollback() {
	if o.handle == nil {
		// It's a normal scenario to set safe `defer Rollback()`, and it's cheaper
		// to check it here than call the API.
		return
	}
	if o.resolved {
		return
	}
	o.resolved = true
	native.PretradeDropCopyOperationRollback(o.handle)
}

func (o *DropCopyOperation) close() {
	if o.handle == nil {
		// It's a normal scenario to set safe `defer Close()`, and it's cheaper
		// to check it here than call the API.
		return
	}
	native.DestroyPretradeDropCopyOperation(o.handle)
	o.handle = nil
	o.resolved = true
}

// Lock returns a lock snapshot for the operation.
//
// Returns ErrDropCopyOperationClosed after Close: Bytes and Equal read the zero
// Lock without decoding it, so it would pass for a lock the operation produced.
func (o *DropCopyOperation) Lock() (Lock, error) {
	if o.handle == nil {
		return Lock{}, ErrDropCopyOperationClosed
	}
	handle := native.PretradeDropCopyOperationGetLock(o.handle)
	result := newLockFromHandle(handle)
	native.DestroyPretradePreTradeLock(handle)
	return result, nil
}

// AccountAdjustments returns the account-adjustment outcomes produced by the
// operation.
//
// Returns ErrDropCopyOperationClosed after Close: an empty slice would claim
// the operation moved no funds.
func (o *DropCopyOperation) AccountAdjustments() ([]accountadjustment.Outcome, error) {
	if o.handle == nil {
		return nil, ErrDropCopyOperationClosed
	}
	handle := native.PretradeDropCopyOperationGetAccountAdjustments(o.handle)
	result := accountadjustment.NewListFromHandle(handle)
	native.DestroyAccountAdjustmentOutcomeList(handle)
	return result, nil
}

// AccountBlock returns the first account block requested by this operation, or
// nil when none was produced. This request-local history can differ from the
// apply-time registry snapshot: an earlier block may remain the stored cause,
// or a deferred unblock may remove this block. Use IsAccountBlocked for the
// snapshot captured before ApplyDropCopy returned.
//
// Returns ErrDropCopyOperationClosed after Close: a nil block would claim the
// operation requested none.
func (o *DropCopyOperation) AccountBlock() (*reject.AccountBlock, error) {
	if o.handle == nil {
		return nil, ErrDropCopyOperationClosed
	}
	list := native.PretradeDropCopyOperationGetAccountBlock(o.handle)
	defer native.DestroyPretradeAccountBlockList(list)
	if native.PretradeAccountBlockListLen(list) == 0 {
		return nil, nil //nolint:nilnil // a nil block is the documented "none requested" answer, not a missing value
	}
	block := reject.NewAccountBlockFromHandle(native.PretradeAccountBlockListGet(list, 0))
	return &block, nil
}

// IsAccountBlocked returns the apply-time blocked-state snapshot for the order
// account, including a block that existed before drop-copy ran. The snapshot
// was captured before ApplyDropCopy returned and does not track later registry
// changes.
//
// Returns ErrDropCopyOperationClosed after Close: false would clear an account
// the engine may well have blocked.
func (o *DropCopyOperation) IsAccountBlocked() (bool, error) {
	if o.handle == nil {
		return false, ErrDropCopyOperationClosed
	}
	return native.PretradeDropCopyOperationIsAccountBlocked(o.handle), nil
}
