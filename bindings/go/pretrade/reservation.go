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
)

// ErrReservationClosed is returned by Reservation accessors once the
// reservation has been released.
var ErrReservationClosed = errors.New("pre-trade reservation already closed")

// Reservation holds a successfully validated pre-trade state reservation.
//
// Lifecycle: the caller owns the reservation and must resolve it exactly once
// with CommitAndClose, RollbackAndClose, or Close. Nothing releases it on the
// caller's behalf - there is no finalizer, because resolving a reservation
// applies or unwinds engine state and that call must stay on a goroutine the
// caller serializes, never the garbage collector's. A reservation that is
// dropped without Close leaks both the native handle and the state it holds
// reserved for the remainder of the process.
//
// Concurrency: callers must serialize methods on the same reservation. Commit
// and Rollback are mutually exclusive: whichever runs first resolves the
// reservation, and any later Commit or Rollback on the same value has no
// effect. Every accessor reports ErrReservationClosed once Close releases the
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
type Reservation struct {
	handle   native.PretradePreTradeReservation
	resolved bool
}

// NewReservationFromHandle creates a Reservation from a native handle.
func NewReservationFromHandle(handle native.PretradePreTradeReservation) *Reservation {
	return &Reservation{handle: handle}
}

// Close releases the reservation.
//
// If neither Commit nor Rollback was called beforehand, Close rolls back
// any pending mutations implicitly; explicit resolution is only required
// when the caller needs to observe commit-time side effects. A rollback
// callback that fails during that implicit rollback arms the engine kill
// switch, exactly as in Rollback.
//
// Idempotency: safe to call more than once; subsequent calls are no-ops.
func (r *Reservation) Close() {
	r.close()
}

// Commit finalizes the reservation and applies its reserved state
// permanently.
//
// Commit does not close the reservation; call Close afterwards, or use
// CommitAndClose. A call after the reservation was resolved or closed is a
// no-op. A mutation commit callback that reports failure does not fail this
// call; it arms the engine kill switch instead, as described on Reservation.
func (r *Reservation) Commit() {
	r.commit()
}

// CommitAndClose commits the reservation and then releases it. A call after
// the reservation was resolved or closed is a no-op.
//
// Callers must not interleave Commit, Rollback, or Close concurrently.
func (r *Reservation) CommitAndClose() {
	r.commit()
	r.close()
}

// Rollback cancels the reservation and releases its reserved state.
//
// Rollback does not close the reservation; call Close afterwards, or use
// RollbackAndClose. A call after the reservation was resolved or closed is a
// no-op, which permits unconditional cleanup in deferred error paths. A
// mutation rollback callback that reports failure does not fail this call; it
// arms the engine kill switch instead, as described on Reservation.
func (r *Reservation) Rollback() {
	r.rollback()
}

// RollbackAndClose rolls back the reservation and then releases it.
// Safe to call after Close; both steps become no-ops.
//
// Callers must not interleave Commit, Rollback, or Close concurrently.
func (r *Reservation) RollbackAndClose() {
	r.rollback()
	r.close()
}

func (r *Reservation) commit() {
	if r.resolved || r.handle == nil {
		return
	}
	r.resolved = true
	native.PretradePreTradeReservationCommit(r.handle)
}

func (r *Reservation) rollback() {
	if r.handle == nil {
		// It's a normal scenario to set safe `defer Rollback()`, and it's cheaper
		// to check it here than call the API.
		return
	}
	if r.resolved {
		return
	}
	r.resolved = true
	native.PretradePreTradeReservationRollback(r.handle)
}

func (r *Reservation) close() {
	if r.handle == nil {
		// It's a normal scenario to set safe `defer Close()`, and it's cheaper
		// to check it here than call the API.
		return
	}
	native.DestroyPretradePreTradeReservation(r.handle)
	r.handle = nil
	r.resolved = true
}

// Lock returns a lock snapshot for the reservation.
//
// Returns ErrReservationClosed after Close: Bytes and Equal read the zero Lock
// without decoding it, so it would pass for a lock the reservation produced.
func (r *Reservation) Lock() (Lock, error) {
	if r.handle == nil {
		return Lock{}, ErrReservationClosed
	}
	handle := native.PretradePreTradeReservationGetLock(r.handle)
	result := newLockFromHandle(handle)
	native.DestroyPretradePreTradeLock(handle)
	return result, nil
}

// AccountAdjustments returns the account-adjustment outcomes produced by the
// reservation.
//
// Returns ErrReservationClosed after Close: an empty slice would claim the
// reservation moved no funds.
func (r *Reservation) AccountAdjustments() ([]accountadjustment.Outcome, error) {
	if r.handle == nil {
		return nil, ErrReservationClosed
	}
	handle := native.PretradePreTradeReservationGetAccountAdjustments(r.handle)
	result := accountadjustment.NewListFromHandle(handle)
	native.DestroyAccountAdjustmentOutcomeList(handle)
	return result, nil
}
