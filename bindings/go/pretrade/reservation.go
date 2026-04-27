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

package pretrade

import (
	"go.openpit.dev/openpit/internal/native"
)

type Reservation struct {
	handle native.PretradePreTradeReservation
}

func NewReservationFromHandle(handle native.PretradePreTradeReservation) *Reservation {
	return &Reservation{handle: handle}
}

// Close releases the reservation.
//
// If neither Commit nor Rollback was called beforehand, Close rolls back
// any pending mutations implicitly; explicit resolution is only required
// when the caller needs to observe commit-time side effects.
//
// Idempotency: safe to call more than once; subsequent calls are no-ops.
func (r *Reservation) Close() {
	if r.handle == nil {
		return
	}
	native.DestroyPretradePreTradeReservation(r.handle)
	r.handle = nil
}

// Commit finalizes the reservation and applies its reserved state
// permanently.
//
// Precondition: the reservation must not be closed.
// Panics if called after Close, CommitAndClose, or RollbackAndClose.
//
// Commit does not close the reservation; call Close afterwards, or use
// CommitAndClose.
func (r *Reservation) Commit() {
	if r.handle == nil {
		panic("pre-trade reservation already closed")
	}
	native.PretradePreTradeReservationCommit(r.handle)
}

// CommitAndClose commits the reservation and then releases it.
// Panics on the Commit step if the reservation is already closed.
func (r *Reservation) CommitAndClose() {
	r.Commit()
	r.Close()
}

// Rollback cancels the reservation and releases its reserved state.
//
// Unlike Commit, Rollback tolerates a closed reservation: calling it
// after Close is a silent no-op. This allows RollbackAndClose cleanup in
// deferred error paths without extra guards.
//
// Rollback does not close the reservation; call Close afterwards, or use
// RollbackAndClose.
func (r *Reservation) Rollback() {
	if r.handle == nil {
		return
	}
	native.PretradePreTradeReservationRollback(r.handle)
}

// RollbackAndClose rolls back the reservation and then releases it.
// Safe to call after Close; both steps become no-ops.
func (r *Reservation) RollbackAndClose() {
	r.Rollback()
	r.Close()
}

// Lock returns a lock snapshot for the reservation.
//
// Panics if the reservation is already closed.
func (r Reservation) Lock() Lock {
	if r.handle == nil {
		panic("pre-trade reservation already closed")
	}
	return newLock(native.PretradePreTradeReservationGetLock(r.handle))
}
