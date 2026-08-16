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

// Package asyncengine wraps the AccountSync engine into an asynchronous
// facade that serializes every call by its routing key.
//
// The package is a convenience layer on top of openpit.Engine configured
// with AccountSync. It is one possible integration pattern. Callers may
// implement their own dispatch (sharded channels, ring buffer,
// actor model, third-party libraries, and so on); nothing in the SDK
// requires the use of this package.
//
// # Threading
//
// The wrapped engine must be built with AccountSync. AsyncEngine maintains
// per-key queues internally and guarantees that operations routed by the same
// key never overlap. Account operations use an account routing key.
// Account-group membership changes use the first supplied account's key. Group
// blocking and group currency operations use an account-group routing key, so
// they serialize with that group's other administrative operations rather than
// with member-account work. UnblockAll uses the engine-wide routing key.
// Parallelism across different keys depends on the strategy: Dynamic processes
// distinct keys in parallel, while Sharded serializes keys that hash to the
// same shard.
//
// All public methods accept a context. The context controls how long the
// caller is willing to wait for the task to be queued. Once a task is
// queued, it runs to completion unless a hard stop is requested.
//
// # Result Delivery
//
// Every queued operation returns a Future. The Future is resolved either
// from the worker goroutine that ran the task or, if the strategy aborted
// the task, from the strategy itself. Callers may block on the future via
// Await, poll via Done/TryGet, or compose via the channel returned by
// Wait.
//
// # Wrapped Lifetime Objects
//
// StartPreTrade returns an AsyncRequest. Its Execute and Close methods
// also go through the same per-account queue, so the AccountSync invariant
// is preserved across the Start - Execute boundary. ExecutePreTrade and
// AsyncRequest.Execute both return an AsyncReservation; its Commit,
// Rollback, Close, CommitAndClose, and RollbackAndClose methods are
// likewise queued. ApplyDropCopy returns an AsyncDropCopyOperation with the
// same set of queued finalizers.
//
// # Chains
//
// Chain(source, begin) builds one lane chain and infers State from begin. An
// order or account source selects an account routing key; an AccountGroupID
// selects that group's account-group routing key, so distinct groups are not
// collapsed onto one global queue. The engine-wide routing key is reserved for
// operations such as UnblockAll. Operations that handle engine outcomes take
// named-field hook structs. CheckOrder and Then take a function directly.
// Chain.CheckOrder is the asynchronous full-pipeline dry-run path. The async
// facade intentionally has no direct dry-run methods; start-stage dry run
// remains available only through the synchronous engine.
//
// Every hook receives the same lane-marked context plus caller State and any
// capability-limited result. Hooks never receive an engine, driver, queue,
// future, or lifecycle object. Operation-backed reads are invalid outside
// their hook. An OrderCheckResult's Passed and Rejects remain readable after
// its hook returns, but its operation-backed reads do not.
//
// The hook context retains the submitted context's values but not its
// cancellation or deadline. The submitted context bounds queue admission only:
// chains are not atomic across steps, so interrupting one midway would create
// the uncertainty ErrChainRetryUnsafe reports.
//
// Run resolves a ChainOutcome even without a terminal hook, and does so only
// after every engine call, hook, and required handle cleanup: a pending
// reservation or drop-copy operation is already rolled back and closed by the
// time the future resolves. Its Err and the error returned by Await agree
// whenever Await returned an outcome; when Await returns ctx.Err() instead, it
// yields the zero ChainOutcome (Status ChainOutcomeUnknown, Err nil,
// RetryUnsafe false). That reports only that the wait ended: waiting never
// cancels the chain, which may still be running or may already have finished -
// call Await again, or use TryGet, to read the real outcome. The outcome passed
// to Finally describes the chain before that hook runs; the outcome resolved
// into the future describes the whole run, including Finally.
//
// Callers track their own progress in State. RetryUnsafe is conservative: it
// reports that the chain entered an engine call that can change state, not
// that a mutation is known to have happened. It is meaningful on
// ChainOutcomeCompleted, ChainOutcomeRejected and ChainOutcomeFailed. It is
// not meaningful on ChainOutcomeUnknown: that status belongs to the zero
// outcome above, whose RetryUnsafe: false is never permission to rerun a chain
// that may still be running and may already have changed state.
// On a failed outcome, Err wraps ErrChainRetryUnsafe when the SDK cannot prove
// nothing changed. Rerunning is unsafe; the sentinel does not claim a mutation
// happened.
//
// A CheckOrder step reports its complete non-mutating verdict to its hook and
// does not end the chain merely because the order failed the check. Finally
// returns a ChainRunner, whose Run is the only way to run a chain with a
// terminal hook. Finally runs after native cleanup and before future resolution
// after a successful begin and a started chain, on completion, rejection,
// failure, or panic. It does not run if begin fails or panics. Validation and
// queue-submission failures, plus any pre-start abort including hard stop and
// idle-queue retirement or drain, invoke no chain callbacks: Begin, step hooks,
// and Finally do not run. AsyncEngine observers remain independent and may
// still be notified.
//
// Submitting another operation to an AsyncEngine with a context containing any
// active chain-lane marker for that engine, at any marker-stack depth, resolves
// its future with ErrReentrantLane instead of waiting on that lane. This is a
// diagnostic only: caller code that supplies an unrelated context bypasses the
// marker, so hooks must still not synchronously re-enter the engine.
//
// # Stopping
//
// StopGraceful refuses new submissions and waits for every queued task to
// run to completion. StopHard refuses new submissions, aborts every task
// not yet started with ErrStopped, and waits for the in-flight task to
// finish. Both methods accept a context for deadline control.
package asyncengine
