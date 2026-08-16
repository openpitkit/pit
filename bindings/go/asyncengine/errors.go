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

// ErrFinalizationInProgress is returned when a snapshot read is attempted
// while an async handle is finalizing. This state is transient and not
// terminal: a later read may succeed, and the caller still owns the handle and
// still owes it a terminal Close, CommitAndClose, or RollbackAndClose call.
var ErrFinalizationInProgress = errors.New(
	"openpit/asyncengine: handle finalization is in progress",
)

// ErrMissingAccountID is returned when a call carries no account to route it
// by: from StartPreTrade, ExecutePreTrade, ApplyDropCopy, and
// ApplyExecutionReport when the supplied order or execution report has no
// account identifier, and from AsyncAccounts.RegisterGroup and
// AsyncAccounts.UnregisterGroup or their chain-step counterparts when the
// accounts slice is empty.
var ErrMissingAccountID = errors.New(
	"openpit/asyncengine: no account ID to route the call by",
)

// ErrUninitializedAccountGroupID is returned when a direct account-group
// operation or account-group-sourced chain receives an uninitialized
// AccountGroupID.
var ErrUninitializedAccountGroupID = errors.New(
	"openpit/asyncengine: uninitialized account group ID",
)

// ErrStopped is returned by Submit and engine methods after the
// AsyncEngine has been stopped, and via aborted futures for tasks not yet
// started when StopHard is invoked.
var ErrStopped = errors.New("openpit/asyncengine: engine is stopped")

// ErrQueueLimit is returned by submit on a Dynamic strategy configured with
// MaxQueues when the limit has been reached and the routing key is not already
// known.
var ErrQueueLimit = errors.New(
	"openpit/asyncengine: dynamic routing queue limit exceeded",
)

// ErrDriverResult is returned when a Driver result has an invalid shape,
// including a missing accepted lifetime object or a handle combined with
// rejects or an error. It applies to direct AsyncEngine operations and chains.
var ErrDriverResult = errors.New(
	"openpit/asyncengine: invalid driver result",
)

// ErrChainEngineMissing is returned when a chain is run with a nil
// *AsyncEngine, through either ChainBuilder.Run or ChainRunner.Run.
var ErrChainEngineMissing = errors.New("openpit/asyncengine: chain engine is nil")

// ErrChainIncomplete is returned when a required chain hook is absent.
var ErrChainIncomplete = errors.New("openpit/asyncengine: chain is incomplete")

// ErrChainOrderRequired is returned when an order-only operation is added to a
// chain whose source is not an order.
var ErrChainOrderRequired = errors.New(
	"openpit/asyncengine: chain operation requires an order source",
)

// ErrChainAccountRequired is returned when an account-only operation is added
// to a chain routed from an account-group ID.
var ErrChainAccountRequired = errors.New(
	"openpit/asyncengine: chain operation requires an account source",
)

// ErrChainAccountGroupRequired is returned when an account-group-only
// operation is added to a chain routed from an order or account ID.
var ErrChainAccountGroupRequired = errors.New(
	"openpit/asyncengine: chain operation requires an account-group source",
)

// ErrChainAccountMismatch is returned when a chain input would run on a
// different account from the lane selected for it.
var ErrChainAccountMismatch = errors.New(
	"openpit/asyncengine: chain account does not match lane",
)

// ErrChainInvalidDecision is returned after rolling a pending operation back
// when an accepted hook returns neither supported Decision value.
var ErrChainInvalidDecision = errors.New(
	"openpit/asyncengine: invalid chain decision",
)

// ErrChainRetryUnsafe is included in a failed chain error when an engine call
// that can change state was entered and the SDK cannot prove nothing changed.
// Chains are not atomic across steps, so callers track their progress in State.
// It means rerunning the chain is unsafe, not that a mutation happened. A
// completed or rejected outcome carries the same state without an error,
// through ChainOutcome.RetryUnsafe. ChainOutcomeUnknown is not a verdict.
var ErrChainRetryUnsafe = errors.New(
	"openpit/asyncengine: chain retry is unsafe",
)

// ErrChainResultUnavailable is returned when an operation result is read
// outside its accepted-operation hook.
var ErrChainResultUnavailable = errors.New(
	"openpit/asyncengine: chain operation result is unavailable",
)

// ErrReentrantLane is returned when an AsyncEngine operation is submitted with
// a context whose marker stack contains an active chain lane for the target
// engine, at any stack depth. It avoids waiting on that lane. An unrelated
// context carries no marker and bypasses this diagnostic.
var ErrReentrantLane = errors.New("openpit/asyncengine: reentrant chain lane")

// errQueueRetired is an internal signal that submit should retry against
// a freshly created queue. Never returned to callers.
var errQueueRetired = errors.New("openpit/asyncengine: queue retired")
