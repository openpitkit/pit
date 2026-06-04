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

import (
	"time"

	"go.openpit.dev/openpit/param"
)

// Observer receives diagnostic callbacks from the dispatcher.
//
// All callbacks are invoked synchronously from worker or submitter
// goroutines; implementations must be thread-safe and must not block for
// long. To export metrics into Prometheus, OpenTelemetry, or a logging
// pipeline, accumulate counters and dispatch the heavy work to a separate
// goroutine.
//
// Callback asymmetries to be aware of:
//   - OnComplete fires for aborted tasks (ran = 0), but OnDequeue is NOT
//     called for aborted tasks. Pairing dequeue and complete counts will
//     see completes without matching dequeues for every aborted task.
//   - A submit that fails synchronously with ErrStopped or ErrQueueLimit
//     emits no callback at all - OnSubmitCancelled fires only when ctx
//     expires while waiting for queue space.
//
// The interface is a no-op by default. Wire only the callbacks you need.
type Observer interface {
	// OnEnqueue is called immediately after a task has been queued.
	// queueDepth is the channel buffer length right after the send.
	OnEnqueue(accountID param.AccountID, queueDepth int)

	// OnDequeue is called right before a task starts running, with the
	// time the task spent waiting in the queue.
	OnDequeue(accountID param.AccountID, waited time.Duration)

	// OnComplete is called right after a task finished running, with the
	// wall-clock duration of the engine call. Aborted tasks report 0.
	OnComplete(accountID param.AccountID, ran time.Duration)

	// OnSlowSubmit is called when a producer has been blocked on Submit
	// for longer than the configured threshold. attempt grows by one
	// every threshold interval the producer keeps waiting.
	OnSlowSubmit(accountID param.AccountID, waiting time.Duration, attempt int)

	// OnQueueFullBlocked is called when a producer has not been able to
	// hand the task to a worker within the configured threshold.
	OnQueueFullBlocked(accountID param.AccountID, waiting time.Duration)

	// OnQueueCreated is reported by Dynamic strategies when a new
	// per-account queue is created. totalQueues is the number of live
	// queues after the creation.
	OnQueueCreated(accountID param.AccountID, totalQueues int)

	// OnQueueRemoved is reported by Dynamic strategies when an idle
	// per-account queue is retired. remainingQueues is the number of
	// live queues after the removal.
	OnQueueRemoved(accountID param.AccountID, remainingQueues int)

	// OnSubmitCancelled is called when ctx expires while a producer is
	// waiting for queue space. The task is never queued; the future is
	// resolved with err.
	OnSubmitCancelled(accountID param.AccountID, err error)
}

// NoopObserver implements Observer with empty methods. It is the default
// when no observer is configured on the builder.
type NoopObserver struct{}

// noopObserver is a shared NoopObserver instance.
var noopObserver Observer = NoopObserver{}

// OnEnqueue is a no-op.
func (NoopObserver) OnEnqueue(param.AccountID, int) {}

// OnDequeue is a no-op.
func (NoopObserver) OnDequeue(param.AccountID, time.Duration) {}

// OnComplete is a no-op.
func (NoopObserver) OnComplete(param.AccountID, time.Duration) {}

// OnSlowSubmit is a no-op.
func (NoopObserver) OnSlowSubmit(param.AccountID, time.Duration, int) {}

// OnQueueFullBlocked is a no-op.
func (NoopObserver) OnQueueFullBlocked(param.AccountID, time.Duration) {}

// OnQueueCreated is a no-op.
func (NoopObserver) OnQueueCreated(param.AccountID, int) {}

// OnQueueRemoved is a no-op.
func (NoopObserver) OnQueueRemoved(param.AccountID, int) {}

// OnSubmitCancelled is a no-op.
func (NoopObserver) OnSubmitCancelled(param.AccountID, error) {}
