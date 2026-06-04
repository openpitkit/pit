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

// Package asyncengine wraps the AccountSync engine into an asynchronous
// facade that serializes every call by account identifier.
//
// The package is a convenience layer on top of openpit.Engine configured
// with AccountSync. It is one possible integration pattern. Callers may
// implement their own per-account dispatch (sharded channels, ring buffer,
// actor model, third-party libraries, and so on); nothing in the SDK
// requires the use of this package.
//
// # Threading
//
// The wrapped engine must be built with AccountSync. AsyncEngine maintains
// per-account queues internally and guarantees, in both strategies, that no
// two operations for the same account ever run concurrently inside the
// engine. Parallelism across different accounts depends on the strategy:
// Dynamic gives full per-account isolation, so distinct accounts are always
// processed in parallel, while Sharded serializes distinct accounts that
// hash to the same shard through a single worker.
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
// likewise queued.
//
// # Stopping
//
// StopGraceful refuses new submissions and waits for every queued task to
// run to completion. StopHard refuses new submissions, aborts every task
// not yet started with ErrStopped, and waits for the in-flight task to
// finish. Both methods accept a context for deadline control.
package asyncengine
