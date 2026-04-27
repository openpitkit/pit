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

package mutation

import "testing"

func TestNewCallbacksHandleIsNonNil(t *testing.T) {
	c := NewCallbacks(func() {}, func() {})
	t.Cleanup(c.Close)

	if c.Handle() == nil {
		t.Fatal("Handle() = nil, want non-nil")
	}
}

func TestCallbacksCloseReleasesHandle(t *testing.T) {
	c := NewCallbacks(func() {}, func() {})
	// Close must not panic; the cgo runtime detects double-delete.
	c.Close()
}

func TestCommitCallbackIsInvoked(t *testing.T) {
	called := false
	c := NewCallbacks(func() { called = true }, func() {})
	t.Cleanup(c.Close)

	pitMutationCommit(c.Handle())
	if !called {
		t.Fatal("commit callback was not called")
	}
}

func TestRollbackCallbackIsInvoked(t *testing.T) {
	called := false
	c := NewCallbacks(func() {}, func() { called = true })
	t.Cleanup(c.Close)

	pitMutationRollback(c.Handle())
	if !called {
		t.Fatal("rollback callback was not called")
	}
}

func TestFreeCallbackReleasesHandle(t *testing.T) {
	// pitMutationFree calls Close() internally; do NOT register t.Cleanup(c.Close)
	// to avoid a double-delete panic.
	c := NewCallbacks(func() {}, func() {})
	h := c.Handle()
	pitMutationFree(h)
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
