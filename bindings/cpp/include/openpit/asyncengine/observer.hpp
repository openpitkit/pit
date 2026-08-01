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

#pragma once

#include "openpit/param/account_id.hpp"

#include <chrono>
#include <cstddef>
#include <cstdint>

// Optional diagnostic callbacks from the dispatcher.
//
// the callbacks you need; the layer pulls in no observability dependency, so
// the destination of the signals is the caller's choice.
//
// All callbacks fire synchronously from worker or submitter threads;
// implementations must be thread-safe and must not block for long. Accumulate
// counters and hand heavy work to a separate thread. Exceptions are ignored so
// diagnostics cannot change dispatcher behavior.
//
// Every callback carries the account id the task was routed by, so a reported
// account is always a real account: the dispatcher never queues work it cannot
// key, and never substitutes a sentinel such as AccountId(0).
//
//   - OnComplete fires for aborted tasks (ran == 0), but OnDequeue does NOT.
//     Pairing dequeue/complete counts will see unmatched completes per abort.
//   - A submit that fails synchronously with Stopped or QueueLimit emits no
//     callback; OnSubmitCancelled fires only when the deadline expires while
//     waiting for queue space.

namespace openpit::asyncengine {

// Diagnostic sink. Default-implemented as no-ops so subclasses override only
// the signals they care about.
class Observer {
 public:
  Observer() = default;
  Observer(const Observer&) = default;
  Observer& operator=(const Observer&) = default;
  Observer(Observer&&) = default;
  Observer& operator=(Observer&&) = default;
  virtual ~Observer() = default;

  // Called immediately after a task is queued; `queueDepth` is the buffered
  // task count right after the send.
  virtual void OnEnqueue(::openpit::param::AccountId /*accountId*/,
                         std::size_t /*queueDepth*/) {}

  // Called right before a task starts running, with the time it waited.
  virtual void OnDequeue(::openpit::param::AccountId /*accountId*/,
                         std::chrono::nanoseconds /*waited*/) {}

  // Called right after a task finishes, with the wall-clock run duration.
  // Aborted tasks report zero.
  virtual void OnComplete(::openpit::param::AccountId /*accountId*/,
                          std::chrono::nanoseconds /*ran*/) {}

  // Called when a producer has blocked on submit longer than the configured
  // threshold; `attempt` grows by one each threshold interval it keeps waiting.
  virtual void OnSlowSubmit(::openpit::param::AccountId /*accountId*/,
                            std::chrono::nanoseconds /*waiting*/,
                            int /*attempt*/) {}

  // Called when a producer could not hand a task to a worker within the
  // configured threshold (the queue is full).
  virtual void OnQueueFullBlocked(::openpit::param::AccountId /*accountId*/,
                                  std::chrono::nanoseconds /*waiting*/) {}

  // Reported by Dynamic strategies when a per-account queue is created;
  // `totalQueues` is the live count after creation.
  virtual void OnQueueCreated(::openpit::param::AccountId /*accountId*/,
                              std::size_t /*totalQueues*/) {}

  // Reported by Dynamic strategies when an idle per-account queue is retired;
  // `remainingQueues` is the live count after removal.
  virtual void OnQueueRemoved(::openpit::param::AccountId /*accountId*/,
                              std::size_t /*remainingQueues*/) {}

  // Called when the deadline expires while a producer waits for queue space.
  // The task is never queued; its future is failed with `SubmitCancelled`.
  virtual void OnSubmitCancelled(::openpit::param::AccountId /*accountId*/) {}
};

// The default observer: every method is a no-op (inherited from `Observer`).
// A shared instance is used when the builder is left without an observer, and
// the hot path skips per-task timestamps when the active observer is this one.
class NoopObserver final : public Observer {};

}  // namespace openpit::asyncengine
