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

package reject

import (
	"runtime"

	"go.openpit.dev/openpit/internal/native"
)

// AccountControl is an engine-provided handle that records kill-switch blocks
// against the account bound to a callback context.
//
// It is valid to use only within the pre-trade processing of the request it
// belongs to — from the callback that produced it through the commit or
// rollback of that request's reservation (so it may be captured into a mutation
// commit/rollback callback for deferred blocking). Recording a block through it
// after that pre-trade transaction has completed is unspecified and must not be
// relied upon.
//
// Its memory is reclaimed automatically by the garbage collector once it is no
// longer referenced; callers do not manage its lifetime.
type AccountControl struct {
	handle native.AccountControl
}

// NewAccountControlFromHandle wraps a native handle into an AccountControl whose
// release is managed by the garbage collector. Returns nil for a nil handle.
func NewAccountControlFromHandle(handle native.AccountControl) *AccountControl {
	if handle == nil {
		return nil
	}
	control := &AccountControl{handle: handle}
	runtime.SetFinalizer(control, func(c *AccountControl) {
		native.DestroyAccountControl(c.handle)
	})
	return control
}

// Block records block against the account bound to this control.
//
// The first cause recorded for an account wins; later calls for the same
// account are no-ops.
func (c *AccountControl) Block(block AccountBlock) {
	native.AccountControlBlock(c.handle, block.NewHandle())
	runtime.KeepAlive(c)
}

// Clone returns a new handle referring to the same account-control facility.
// The returned control records blocks against the same account; its lifetime is
// likewise managed by the garbage collector.
func (c *AccountControl) Clone() *AccountControl {
	clone := NewAccountControlFromHandle(native.AccountControlClone(c.handle))
	runtime.KeepAlive(c)
	return clone
}
