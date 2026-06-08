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

// Package accountadjustment provides types for account adjustment callbacks.
package accountadjustment

import (
	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/optional"
	"go.openpit.dev/openpit/reject"
)

// Context carries the native handle for an account adjustment callback invocation.
type Context struct {
	handle native.AccountAdjustmentContext
}

// NewContextFromHandle wraps a native handle into a Context.
func NewContextFromHandle(handle native.AccountAdjustmentContext) Context {
	return Context{handle: handle}
}

// AccountControl returns the account-control handle bound to this
// account-adjustment context.
//
// An account-adjustment context always carries account control, so this call
// always returns a usable handle. The returned handle is valid to use only
// within the account-adjustment processing of this request — through its commit
// or rollback (so it may be captured into a mutation commit/rollback callback
// for deferred blocking); using it afterwards is unspecified. Its memory is
// reclaimed by the garbage collector; callers do not manage its lifetime.
func (c Context) AccountControl() *reject.AccountControl {
	return reject.NewAccountControlFromHandle(
		native.AccountAdjustmentContextGetAccountControl(c.handle),
	)
}

// AccountGroup returns the account-group identifier for the account bound to
// this account-adjustment context. The option is empty when the account is not
// assigned to any group.
func (c Context) AccountGroup() optional.Option[param.AccountGroupID] {
	id, ok := native.AccountAdjustmentContextGetAccountGroup(c.handle)
	return optional.From(param.NewAccountGroupIDFromHandle(id), ok)
}
