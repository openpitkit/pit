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
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/optional"
)

// PostTradeContext carries engine-provided context passed to the
// ApplyExecutionReport policy callback.
type PostTradeContext struct{ handle native.PostTradeContext }

// NewPostTradeContextFromHandle creates a PostTradeContext from a native handle.
func NewPostTradeContextFromHandle(handle native.PostTradeContext) PostTradeContext {
	return PostTradeContext{handle: handle}
}

// AccountGroup returns the account-group identifier for the account that
// triggered the execution report. The option is empty when the account is not
// assigned to any group.
func (c PostTradeContext) AccountGroup() optional.Option[param.AccountGroupID] {
	id, ok := native.PostTradeContextGetAccountGroup(c.handle)
	return optional.From(param.NewAccountGroupIDFromHandle(id), ok)
}
