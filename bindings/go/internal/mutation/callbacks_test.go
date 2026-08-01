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

package mutation

import (
	"runtime/cgo"
	"testing"

	"go.openpit.dev/openpit/internal/callback"
	"go.openpit.dev/openpit/internal/diag"
)

func TestNewCallbacksHandleIsNonNil(t *testing.T) {
	c := NewCallbacks(func() {}, func() {})
	t.Cleanup(c.Close)

	if c.Handle() == nil {
		t.Fatal("Handle() = nil, want non-nil")
	}
}

func TestCallbacksCloseReleasesHandle(*testing.T) {
	c := NewCallbacks(func() {}, func() {})
	// Close must not panic; the cgo runtime detects double-delete.
	c.Close()
}

func TestCommitCallbackIsInvoked(t *testing.T) {
	called := false
	c := NewCallbacks(func() { called = true }, func() {})
	t.Cleanup(c.Close)

	pitMutationCommit(c.Handle(), nil)
	if !called {
		t.Fatal("commit callback was not called")
	}
}

func TestRollbackCallbackIsInvoked(t *testing.T) {
	called := false
	c := NewCallbacks(func() {}, func() { called = true })
	t.Cleanup(c.Close)

	pitMutationRollback(c.Handle(), nil)
	if !called {
		t.Fatal("rollback callback was not called")
	}
}

func TestCommitCallbackPanicReturnsFailure(t *testing.T) {
	c := NewCallbacks(func() { panic("commit failed") }, func() {})
	t.Cleanup(c.Close)

	if succeeded := pitMutationCommit(c.Handle(), nil); bool(succeeded) {
		t.Fatalf("pitMutationCommit() = %v, want false", succeeded)
	}
}

func TestRollbackCallbackPanicReturnsFailure(t *testing.T) {
	c := NewCallbacks(func() {}, func() { panic("rollback failed") })
	t.Cleanup(c.Close)

	if succeeded := pitMutationRollback(c.Handle(), nil); bool(succeeded) {
		t.Fatalf("pitMutationRollback() = %v, want false", succeeded)
	}
}

func TestFreeCallbackReleasesHandle(*testing.T) {
	// pitMutationFree calls Close() internally; do NOT register t.Cleanup(c.Close)
	// to avoid a double-delete panic.
	c := NewCallbacks(func() {}, func() {})
	h := c.Handle()
	pitMutationFree(h)
}

// pitMutationFree is an exported cgo trampoline invoked by the native runtime
// at teardown. It carries no result, so an unwind out of it crosses the C frame
// and kills the process. A stale or doubled free must be contained instead.

func TestFreeCallbackContainsDoubledFree(t *testing.T) {
	reported := recordTeardownFailures(t)

	c := NewCallbacks(func() {}, func() {})
	h := c.Handle()

	pitMutationFree(h)
	// The second free hits an already-deleted cgo handle.
	pitMutationFree(h)

	if len(*reported) != 1 {
		t.Fatalf("reported teardown failures = %d, want 1 for the doubled free", len(*reported))
	}
}

// TestFreeCallbackReleasesUnexpectedHandleType pins that free owns whatever
// handle the native runtime passes it: no Callbacks.Close will ever reach a
// foreign value, so the handle has to be released here or it is stranded for
// the lifetime of the process.
func TestFreeCallbackReleasesUnexpectedHandleType(t *testing.T) {
	reported := recordTeardownFailures(t)

	handle := cgo.NewHandle(struct{}{})
	pitMutationFree(callback.NewUserDataFromHandle(handle))

	if len(*reported) != 1 {
		t.Fatalf("reported teardown failures = %d, want 1", len(*reported))
	}
	// The handle is gone: reaching it again is contained, not fatal.
	pitMutationFree(callback.NewUserDataFromHandle(handle))
}

// recordTeardownFailures installs a diagnostic sink for the duration of the
// test and returns the slice it appends to.
func recordTeardownFailures(t *testing.T) *[]error {
	t.Helper()

	var reported []error
	diag.SetHandler(func(err error) { reported = append(reported, err) })
	t.Cleanup(func() { diag.SetHandler(nil) })
	return &reported
}

func TestGetCommitFnAddrIsNonNil(t *testing.T) {
	if GetCommitFnAddr() == nil {
		t.Fatal("GetCommitFnAddr() = nil, want non-nil")
	}
}

func TestGetRollbackFnAddrIsNonNil(t *testing.T) {
	if GetRollbackFnAddr() == nil {
		t.Fatal("GetRollbackFnAddr() = nil, want non-nil")
	}
}

func TestGetFreeFnAddrIsNonNil(t *testing.T) {
	if GetFreeFnAddr() == nil {
		t.Fatal("GetFreeFnAddr() = nil, want non-nil")
	}
}
