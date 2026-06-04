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

package asyncengine

import "errors"

// ErrMissingAccountID is returned by StartPreTrade, ExecutePreTrade, and
// ApplyExecutionReport when the supplied order or execution report does
// not carry an account identifier.
var ErrMissingAccountID = errors.New(
	"openpit/asyncengine: account ID is not set on the order or report",
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
