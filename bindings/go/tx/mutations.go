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

// Package tx provides transaction mutation types for the pre-trade pipeline.
package tx

import (
	"errors"

	"go.openpit.dev/openpit/internal/mutation"
	"go.openpit.dev/openpit/internal/native"
)

// Mutations is a collection of commit/rollback callbacks registered during a
// PerformPreTradeCheck, ApplyAccountAdjustment, RetireAccount, or
// PerformPreTradeCheckDryRun hook. The engine discards mutations pushed by
// PerformPreTradeCheckDryRun.
//
// The collection is non-owning and callback-scoped: it borrows engine state
// that is valid only for the duration of the hook that received it. Retaining
// the collection or using it after that hook has returned is undefined.
type Mutations struct{ handle native.Mutations }

// NewMutationsFromHandle creates a Mutations from a native handle.
func NewMutationsFromHandle(handle native.Mutations) Mutations {
	return Mutations{handle: handle}
}

// Push registers one mutation with commit and rollback callbacks.
//
// Apply tentative state before Push. Ordinary reservation finalization calls
// commit to keep that state or rollback to undo it. A drop-copy operation
// registers the same pair and finalizes it the same way, from Commit or
// Rollback on the returned operation, or implicitly from an unresolved Close.
// A rollback also runs for mutations whose commit was never reached, because
// their tentative state was already applied.
//
// In RetireAccount, the hook only verifies the account state and registers
// removals. Commit applies each removal for the first time; rollback has
// nothing to undo. The engine finalizes these mutations inside RetireAccount,
// which returns no mutation handle.
//
// Neither callback has the right to fail: by the time a finalizer runs the
// decision is already made and there is nothing left to compensate. A callback
// panic is recovered at the SDK boundary and reported to the core as exactly
// such a failure. It arms the engine kill switch. A mutation registered from
// Go belongs to a custom policy whose state reach the engine cannot bound,
// so EVERY account is blocked, not only the order's own.
//
// Ordinary void Commit and Rollback calls do not report this failure; the
// next pre-trade call is rejected with SystemUnavailable. RetireAccount instead
// returns a FinalizerFailed domain error after a commit callback failure, with
// no policy refusals. An operator lifts the kill switch with UnblockAll on the
// engine's Accounts accessor, which leaves individual account and group blocks
// untouched. A failure reported while the engine compensates a fatal drop-copy
// evaluation exit additionally appends SystemUnavailable to that call's
// rejects.
func (m Mutations) Push(commit, rollback func()) error {
	if commit == nil {
		return errors.New("mutation commit callback is nil")
	}
	if rollback == nil {
		return errors.New("mutation rollback callback is nil")
	}

	callbacks := mutation.NewCallbacks(commit, rollback)
	if err := native.MutationsPush(
		m.handle,
		mutation.GetCommitFnAddr(),
		mutation.GetRollbackFnAddr(),
		callbacks.Handle(),
		mutation.GetFreeFnAddr(),
	); err != nil {
		callbacks.Close()
		return err
	}

	return nil
}
