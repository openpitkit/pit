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

package asyncengine

import "errors"

// ErrMissingAccountID is returned when a call carries no account to route it
// by: from StartPreTrade, ExecutePreTrade, ApplyDropCopy, and
// ApplyExecutionReport when the supplied order or execution report has no
// account identifier, and from AsyncAccounts.RegisterGroup and
// AsyncAccounts.UnregisterGroup when the accounts slice is empty.
var ErrMissingAccountID = errors.New(
	"openpit/asyncengine: no account ID to route the call by",
)

// ErrStopped is returned by Submit and engine methods after the
// AsyncEngine has been stopped, and via aborted futures for tasks not yet
// started when StopHard is invoked.
var ErrStopped = errors.New("openpit/asyncengine: engine is stopped")

// ErrQueueLimit is returned by submit on a Dynamic strategy configured
// with MaxQueues when the limit has been reached and the account ID is
// not already known.
var ErrQueueLimit = errors.New(
	"openpit/asyncengine: dynamic per-account queue limit exceeded",
)

// errQueueRetired is an internal signal that submit should retry against
// a freshly created queue. Never returned to callers.
var errQueueRetired = errors.New("openpit/asyncengine: queue retired")
