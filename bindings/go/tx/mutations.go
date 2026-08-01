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

// Mutations is a collection of commit/rollback callbacks registered during a pre-trade check.
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
// Neither callback has the right to fail: by the time a finalizer runs the
// decision is already made and the state it finalizes was applied eagerly, so
// there is nothing left to compensate. A callback panic is recovered at the SDK
// boundary and reported to the core as exactly such a failure. That failure
// never fails the void Commit or Rollback call that ran the callback, and is
// never discarded either: it arms the engine kill switch. A mutation registered
// from Go belongs to a custom policy whose state reach the engine cannot bound,
// so EVERY account is blocked, not only the order's own. Nothing reports the
// block to the finalizing caller; it surfaces when the next pre-trade call is
// rejected with SystemUnavailable, and an operator lifts it with UnblockAll on
// the engine's Accounts accessor, which leaves accounts and groups blocked
// individually untouched. A failure reported while the engine compensates a
// fatal drop-copy evaluation exit additionally appends SystemUnavailable to
// that call's rejects.
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
