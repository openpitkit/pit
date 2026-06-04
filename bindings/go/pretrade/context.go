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

// Package pretrade provides pre-trade risk checking types and interfaces.
package pretrade

import (
	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/optional"
	"go.openpit.dev/openpit/reject"
)

// Context carries engine-provided context passed to policy callbacks.
type Context struct{ handle native.PretradeContext }

// NewContextFromHandle creates a Context from a native handle.
func NewContextFromHandle(handle native.PretradeContext) Context {
	return Context{handle: handle}
}

// AccountControl returns the account-control handle bound to this main-stage
// pre-trade context.
//
// The present flag is false when the context carries no account control because
// no account could be bound to the request, in which case the returned handle is
// nil and must not be used. The returned handle is valid to use only within the
// pre-trade transaction of this request — through the commit or rollback of its
// reservation (so it may be captured into a mutation commit/rollback callback
// for deferred blocking); using it afterwards is unspecified. Its memory is
// reclaimed by the garbage collector; callers do not manage its lifetime.
func (c Context) AccountControl() (*reject.AccountControl, bool) {
	control := reject.NewAccountControlFromHandle(native.PretradeContextGetAccountControl(c.handle))
	return control, control != nil
}

// AccountGroup returns the account-group identifier for the account bound to
// this pre-trade context. The option is empty when the account is not assigned
// to any group.
func (c Context) AccountGroup() optional.Option[param.AccountGroupID] {
	id, ok := native.PretradeContextGetAccountGroup(c.handle)
	return optional.From(param.NewAccountGroupIDFromHandle(id), ok)
}
