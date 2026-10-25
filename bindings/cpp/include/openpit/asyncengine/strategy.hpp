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

#include "openpit/asyncengine/future.hpp"
#include "openpit/asyncengine/observer.hpp"

#include <openpit.h>

#include <algorithm>
#include <atomic>
#include <cassert>
#include <chrono>
#include <condition_variable>
#include <cstddef>
#include <cstdint>
#include <deque>
#include <exception>
#include <functional>
#include <memory>
#include <mutex>
#include <optional>
#include <thread>
#include <type_traits>
#include <unordered_map>
#include <unordered_set>
#include <utility>
#include <vector>

// Per-account dispatch strategies and the threading contract.
//
// THREADING CONTRACT (this is the domain-critical part; see the project
// "Threading Contract" and "Async Engine" wiki pages):
//
//   - The wrapped driver is assumed to be an AccountSync engine: concurrent
//     calls on one handle are safe IFF no two calls for the same account are
//     ever concurrent. This layer is exactly the per-account pinning scheme
//     that guarantee requires.
//   - Every task is routed by account id to one queue, and each queue is
//     drained by a single dedicated worker thread. Therefore no two tasks for
//     the same account ever run in the driver concurrently, and within one
//     account tasks run in submit order (FIFO).
//   - Parallelism across DIFFERENT accounts depends on the strategy. Dynamic
//     gives one worker per account: distinct accounts always run in parallel.
//     Sharded fans accounts onto a fixed worker pool: distinct accounts that
//     hash to the same shard are serialized through one worker. Neither relaxes
//     the per-account invariant.
//   - A task runs on a worker thread, never the submitting thread. The future
//     it produced is resolved from that worker thread (or synchronously on the
//     submitter if the submit fails before queueing). Result values cross the
//     thread boundary through the future; user code observing the future may
//     run on any thread, so it must not rely on thread-local OS state.
//
// Engine closures report callback exceptions through their promises. A failure
// to report, or any exception escaping the worker, terminates the process.
// Cleanup scheduling failures before publication propagate to the caller;
// reporting failures after publication terminate the process.
//
// STOP AND BLOCKED PRODUCERS. A graceful stop keeps every worker draining, so a
// producer blocked on a full queue still makes progress and is allowed to
// finish its send. A hard stop makes no such promise (workers abort instead of
// running, and the destructor's stop must terminate), so it releases blocked
// producers with `ErrorCode::Stopped`. Queues themselves are closed only after
// every registered producer has left, which is a separate concern from waking
// them: closing earlier would race a queue one of those producers is creating.
//
// EXCLUSIVE SECTIONS. A few engine operations change state that every account
// reads, and under AccountSync the engine takes no lock for them: they must run
// with nothing else in flight. `Strategy::Exclusive` provides that. It stops
// each lane with a barrier task queued behind the work the lane already
// accepted, keeps off-lane handle releases out of the way, runs the caller's
// callback on the calling thread, and then lets the lanes continue. Ordinary
// tasks pay nothing for it: a section touches only slow paths.

namespace openpit::asyncengine {

inline constexpr std::size_t kDefaultQueueCapacity = 1024;
inline constexpr std::chrono::nanoseconds kDefaultSlowSubmitThreshold =
    std::chrono::minutes(1);
inline constexpr std::chrono::nanoseconds kDefaultIdleCleanupAfter =
    std::chrono::minutes(5);
inline constexpr std::chrono::nanoseconds kDefaultIdleCleanupPeriod =
    std::chrono::minutes(1);

namespace detail {

// The unit of work a strategy dispatches. Exactly one of `Run`/`Abort` runs
// over the task's lifetime: `Run` when the worker executes it normally, `Abort`
// when a queued task is dropped (hard stop) or the submit itself failed on the
// caller thread. Both must resolve the task's future exactly once (the future
// machinery is itself idempotent as a backstop).
class Task {
 public:
  Task() = default;
  Task(const Task&) = delete;
  Task& operator=(const Task&) = delete;
  Task(Task&&) = delete;
  Task& operator=(Task&&) = delete;
  virtual ~Task() = default;

  // Executes the wrapped operation and resolves the future.
  virtual void Run() = 0;

  // Drops the operation and fails the future with `error`.
  virtual void Abort(Error error) = 0;
};

using TaskPtr = std::unique_ptr<Task>;
using SubmitFailureHandler = std::function<void(Error)>;

// Closure task: runs caller-supplied closures and resolves a void future. The
// closure is built by the engine layer to call the driver and resolve a typed
// promise; this keeps the strategy free of driver knowledge.
template <typename RunFn, typename AbortFn>
class ClosureTask final : public Task {
  static_assert(std::is_invocable_v<RunFn&>,
                "ClosureTask run closure must be invocable with no arguments");
  static_assert(
      std::is_invocable_v<AbortFn&, Error>,
      "ClosureTask abort closure must be invocable with openpit::Error");

 public:
  ClosureTask(RunFn run, AbortFn abort)
      : m_run(std::move(run)), m_abort(std::move(abort)) {}

  void Run() override {
    Contain([this] { m_run(); });
  }

  void Abort(Error error) override {
    Contain([this, &error] { m_abort(std::move(error)); });
  }

 private:
  // Future has no continuations. Contain closures that did not handle their
  // own exceptions; engine closures terminate if their error reporting fails.
  template <typename Body>
  static void Contain(Body&& body) noexcept {
    // Containment keeps the lane usable for closures without their own handler.
    // nosemgrep: async-cpp-dropped-exception
    try {
      body();
    } catch (...) {
      // Deliberately swallowed: see above.
    }
  }

  RunFn m_run;
  AbortFn m_abort;
};

inline void ResolveMandatoryCleanup(
    Promise<std::monostate> promise,
    const std::function<void()>& cleanup) noexcept {
  try {
    cleanup();
  } catch (const std::exception& ex) {
    promise.Fail(Error(ErrorCode::TaskFailed, ex.what()));
    return;
  } catch (...) {
    promise.Fail(Error(ErrorCode::TaskFailed,
                       "mandatory cleanup threw a non-standard exception"));
    return;
  }
  promise.Resolve(std::monostate{});
}

// Mandatory cleanup reports its own failure and never lets a caller exception
// escape the worker boundary. Run and Abort intentionally have the same
// behavior: cleanup must happen even when hard stop drops queued work.
class MandatoryCleanupTask final : public Task {
 public:
  MandatoryCleanupTask(Promise<std::monostate> promise,
                       std::function<void()> cleanup)
      : m_promise(std::move(promise)), m_cleanup(std::move(cleanup)) {}

  void Run() override { ResolveMandatoryCleanup(m_promise, m_cleanup); }

  void Abort(Error /*error*/) override {
    ResolveMandatoryCleanup(m_promise, m_cleanup);
  }

 private:
  Promise<std::monostate> m_promise;
  std::function<void()> m_cleanup;
};

// One queued task plus the metadata observer callbacks need.
struct QueuedTask {
  TaskPtr task;
  OpenPitParamAccountId accountId = 0;
  std::chrono::steady_clock::time_point enqueuedAt{};
};

// Stamped on an exclusive-section barrier in place of its enqueue time. No
// regular task carries it, so the worker tells a barrier apart without a field
// or a call of its own, and keeps barriers out of observer metrics.
inline constexpr std::chrono::steady_clock::time_point kSectionBarrierStamp =
    std::chrono::steady_clock::time_point::min();

class Base;
struct KeyQueue;

// Marks the threads that must not open an exclusive section on a strategy: its
// lane workers, inline account cleanup, and a section's own thread until its
// lanes continue. Marks form a per-thread stack, so a section opened on a lane
// of another engine, or inside another engine's section, keeps the outer marks
// visible.
enum class LaneMarkKind { Lane, InlineCleanup };

// Same layout in every build: the thread-local stack is shared by translation
// units that may differ in `NDEBUG`.
struct LaneMark {
  const Base* owner;
  bool section;
  const LaneMark* outer;
  const KeyQueue* queue = nullptr;
  LaneMarkKind kind = LaneMarkKind::Lane;
};

[[nodiscard]] inline const LaneMark*& CurrentLaneMark() noexcept {
  static thread_local const LaneMark* mark = nullptr;
  return mark;
}

#ifndef NDEBUG
[[nodiscard]] inline bool RunningInlineCleanup(const Base& owner) noexcept {
  for (const LaneMark* mark = CurrentLaneMark(); mark != nullptr;
       mark = mark->outer) {
    if (mark->owner == &owner && mark->kind == LaneMarkKind::InlineCleanup) {
      return true;
    }
  }
  return false;
}
#endif

// An off-lane handle release that arrived while an exclusive section was
// installed. Whether or not the callback ran, the section runs it on its own
// thread before it releases the lanes: its account had no lane when it
// arrived, and a lane created for it since starts with the section's barrier.
struct PostponedCleanup {
  Promise<std::monostate> promise;
  std::function<void()> cleanup;
};

// One `Exclusive` call in progress. A lane stops by running a barrier task
// queued behind the work it already accepted: the barrier counts itself in
// `parked` and then waits for `released`, so a stopped worker runs nothing else
// until the section ends. `expected` counts the barriers that must park before
// the callback runs: one per lane present when the section was installed, plus
// every Dynamic lane created while it is installed, which starts with its
// barrier. These fields are guarded by `Base::m_sectionMutex`; `postponed` is
// guarded by the owning strategy's routing mutex instead.
struct ExclusiveSection {
  std::size_t expected = 0;
  std::size_t parked = 0;
  bool aborted = false;
  bool released = false;
  std::vector<PostponedCleanup> postponed;
};

// A single per-account-or-shard FIFO queue drained by its own worker thread.
//
// `mutex`/`notEmpty` form a bounded blocking queue; `notFull` wakes a producer
// blocked on a full queue. `closed` (channel closed by stop) and `retired`
// (idle cleanup, Dynamic only) both unblock the worker and any waiting
// producer. `pending` counts tasks enqueued but not yet fully handled (buffered
// or running): idle cleanup refuses to retire a queue with `pending != 0`, so a
// queue running a long task is never retired even once its buffer drains.
//
// The queue owns the `worker` thread that drains it (started by the strategy
// right after construction). This keeps the thread's lifetime tied to the
// queue object, so neither a retired queue nor a stopped strategy can leave a
// thread un-joined (RAII).
struct KeyQueue {
  explicit KeyQueue(std::size_t capacity) : capacity(capacity) { Touch(); }

  void Touch() {
    lastActive.store(
        std::chrono::steady_clock::now().time_since_epoch().count(),
        std::memory_order_relaxed);
  }

  [[nodiscard]] std::chrono::steady_clock::time_point LastActiveAt() const {
    return std::chrono::steady_clock::time_point(
        std::chrono::steady_clock::duration(
            lastActive.load(std::memory_order_relaxed)));
  }

  void Join() {
    if (worker.joinable()) {
      worker.join();
    }
  }

  std::size_t capacity;
  std::mutex mutex;
  std::condition_variable notEmpty;
  std::condition_variable notFull;
  std::deque<QueuedTask> buffer;
  std::deque<QueuedTask> deferredMandatoryCleanups;
  bool closed = false;
  bool retired = false;
  bool workerExited = false;
  // Guarded by the owning strategy's deferred-registry mutex.
  bool deferredRegistered = false;
  std::atomic<std::int64_t> lastActive{0};
  std::atomic<std::int64_t> pending{0};
  std::thread worker;
};

using KeyQueuePtr = std::shared_ptr<KeyQueue>;

// Configuration shared by every concrete strategy.
struct BaseConfig {
  Observer* observer = nullptr;  // null -> the shared no-op.
  std::size_t queueCapacity = 0;
  std::chrono::nanoseconds slowSubmitThreshold{0};
};

// Deterministic rendezvous points for the binding's own concurrency tests. They
// are kept out of `BaseConfig` (and off every constructor the public builder
// calls) so no configuration a user can reach carries them. A strategy built
// without seams runs a worker loop compiled without the seam check at all.
struct WorkerSeams {
  // Runs under the lane mutex, immediately before the worker waits.
  std::function<void()> beforeWorkerWait;
  // Runs after deferred queue registration, before cleanup publication.
  std::function<void()> afterDeferredRegistrationBeforePublication;

  // Cold teardown seam on the joining thread, before a worker is joined.
  static std::function<void()>& BeforeJoin() {
    static thread_local std::function<void()> seam;
    return seam;
  }
};

// Outcome of a single `SendToQueue` attempt.
//   - success: `error` is empty.
//   - terminal failure (stop, deadline): `error` is set, `task` is null
//     (already consumed/dropped), and the caller fails the future with `error`.
//   - retired (Dynamic only): `retired` is true and `task` is handed back so
//     the caller can recreate the queue and retry without losing the task.
struct SendResult {
  std::optional<Error> error;
  TaskPtr task;
  bool retired = false;
};

// Producer/worker/stop logic shared by both strategies. Concrete strategies own
// the routing of account id -> KeyQueue and the worker-thread roster.
//
// `tracksIdle` is set only by Dynamic when its cleanup loop can actually retire
// a queue; it gates the `pending`/`lastActive` bookkeeping the Sharded path
// skips entirely.
class Base {
 public:
  // Marks a cleanup running inline under its account fence. `Exclusive`
  // refuses on it; debug builds also assert on calls back into this strategy.
  class InlineCleanupMark {
   public:
    explicit InlineCleanupMark(const Base& base) noexcept
        : m_mark{&base, false, CurrentLaneMark(), nullptr,
                 LaneMarkKind::InlineCleanup} {
      CurrentLaneMark() = &m_mark;
    }
    ~InlineCleanupMark() noexcept { CurrentLaneMark() = m_mark.outer; }
    InlineCleanupMark(const InlineCleanupMark&) = delete;
    InlineCleanupMark& operator=(const InlineCleanupMark&) = delete;

   private:
    LaneMark m_mark;
  };

  // True on this strategy's section thread, or on the worker of `queue`.
  [[nodiscard]] bool SerializesQueueOnThisThread(
      const KeyQueue* queue) const noexcept {
    for (const LaneMark* mark = CurrentLaneMark(); mark != nullptr;
         mark = mark->outer) {
      if (mark->owner == this &&
          (mark->section || (queue != nullptr && mark->queue == queue))) {
        return true;
      }
    }
    return false;
  }

  // The lane this thread works for this strategy, or null off its workers.
  [[nodiscard]] const KeyQueue* WorkerQueueOnThisThread() const noexcept {
    for (const LaneMark* mark = CurrentLaneMark(); mark != nullptr;
         mark = mark->outer) {
      if (mark->owner == this && mark->queue != nullptr) {
        return mark->queue;
      }
    }
    return nullptr;
  }

  Base(const BaseConfig& cfg, bool tracksIdle, WorkerSeams seams)
      : m_observer(cfg.observer != nullptr ? cfg.observer : SharedNoop()),
        m_queueCapacity(cfg.queueCapacity > 0 ? cfg.queueCapacity
                                              : kDefaultQueueCapacity),
        m_slowSubmitThreshold(cfg.slowSubmitThreshold >
                                      std::chrono::nanoseconds(0)
                                  ? cfg.slowSubmitThreshold
                                  : kDefaultSlowSubmitThreshold),
        m_seams(std::move(seams)),
        m_observerActive(m_observer != SharedNoop()),
        m_tracksIdle(tracksIdle) {}

  Base(const Base&) = delete;
  Base& operator=(const Base&) = delete;

 protected:
  // The account fence of every cleanup run on its caller's thread, in both
  // strategies; Dynamic also guards its lane routing with this mutex.
  std::mutex m_mutex;
  std::condition_variable m_routeCv;
  std::unordered_set<OpenPitParamAccountId> m_cleanupAccounts;

  struct CleanupAccountRelease {
    Base& strategy;
    OpenPitParamAccountId accountId;

    ~CleanupAccountRelease() noexcept {
      {
        std::lock_guard<std::mutex> lock(strategy.m_mutex);
        strategy.m_cleanupAccounts.erase(accountId);
      }
      strategy.m_routeCv.notify_all();
    }
  };

  void ResolveFencedCleanup(OpenPitParamAccountId accountId,
                            Promise<std::monostate> promise,
                            const std::function<void()>& cleanup) {
    const InlineCleanupMark mark{*this};
    const CleanupAccountRelease release{*this, accountId};
    ResolveMandatoryCleanup(std::move(promise), cleanup);
  }

  void CleanupAfterWorkerExit(OpenPitParamAccountId accountId,
                              Promise<std::monostate> promise,
                              const std::function<void()>& cleanup) {
    {
      std::unique_lock<std::mutex> lock(m_mutex);
      m_routeCv.wait(lock, [this, accountId] {
        return m_cleanupAccounts.find(accountId) == m_cleanupAccounts.end();
      });
      m_cleanupAccounts.insert(accountId);
    }
    ResolveFencedCleanup(accountId, std::move(promise), cleanup);
  }

  class ProducerGuard {
   public:
    explicit ProducerGuard(Base& base)
        : m_base(&base), m_registered(base.RegisterProducer()) {}

    ~ProducerGuard() {
      if (m_registered) {
        m_base->FinishProducer();
      }
    }

    ProducerGuard(const ProducerGuard&) = delete;
    ProducerGuard& operator=(const ProducerGuard&) = delete;

    [[nodiscard]] explicit operator bool() const noexcept {
      return m_registered;
    }

   private:
    Base* m_base;
    bool m_registered;
  };

  class MandatoryCleanupRegistration {
   public:
    explicit MandatoryCleanupRegistration(Base& base) : m_base(&base) {}
    ~MandatoryCleanupRegistration() { m_base->FinishMandatoryCleanup(); }

    MandatoryCleanupRegistration(const MandatoryCleanupRegistration&) = delete;
    MandatoryCleanupRegistration& operator=(
        const MandatoryCleanupRegistration&) = delete;

   private:
    Base* m_base;
  };

  [[nodiscard]] static ::openpit::param::AccountId PublicAccountId(
      OpenPitParamAccountId raw) noexcept {
    return ::openpit::detail::FromNative<::openpit::param::AccountId>(raw);
  }

  [[nodiscard]] std::size_t queueCapacity() const { return m_queueCapacity; }
  [[nodiscard]] bool observerActive() const { return m_observerActive; }
  [[nodiscard]] bool tracksIdle() const { return m_tracksIdle; }

  [[nodiscard]] std::function<void()> TrackMandatoryCleanup(
      std::function<void()> cleanup) {
    auto registration = RegisterMandatoryCleanup();
    return [registration = std::move(registration),
            cleanup = std::move(cleanup)] { cleanup(); };
  }

  template <typename Callback>
  void Notify(Callback&& callback) const noexcept {
    // Observer failures cannot change dispatch.
    // nosemgrep: async-cpp-dropped-exception
    try {
      callback(*m_observer);
    } catch (...) {
      // Observability must never change dispatcher behavior.
    }
  }

  [[nodiscard]] bool IsStopped() const {
    return m_stopRequested.load(std::memory_order_acquire);
  }

  [[nodiscard]] bool HardStopped() const {
    return (m_slowPaths.load(std::memory_order_acquire) & kHardStopBit) != 0;
  }

  // Implements `Strategy::Exclusive` for both strategies. `Host` is the
  // concrete strategy, which supplies the part that depends on its routing:
  //   - `InstallExclusive(section)` publishes the section to the strategy's
  //     slow paths and returns the lanes that need a barrier;
  //   - `AwaitRunningCleanups(deadline)` waits for the off-lane handle releases
  //     that started before the section was installed;
  //   - `WithdrawExclusive(section, postponed)` unpublishes the section unless
  //     it holds postponed releases; then it moves them into `postponed` and
  //     keeps the section published, so none can slip past the drain.
  template <typename Host>
  [[nodiscard]] std::optional<Error> RunExclusive(
      Host& host, std::chrono::steady_clock::time_point deadline,
      const std::function<void()>& run) {
    if (MarkedOnThisThread(/*sectionOnly=*/false)) {
      return Error(ErrorCode::ReentrantLane,
                   "async exclusive section requested from a lane or a section "
                   "of the same engine");
    }
    if (std::optional<Error> refused = TakeSectionTurn(deadline)) {
      return refused;
    }
    const SectionTurn turn(*this);
    // A section is a producer: stop refuses it once signalled, and otherwise
    // waits for it before closing any lane.
    const ProducerGuard producer(*this);
    if (!producer) {
      return Error(ErrorCode::Stopped, "async engine is stopped");
    }
    const auto section = std::make_shared<ExclusiveSection>();
    const std::vector<KeyQueuePtr> queues = host.InstallExclusive(section);
    // Declared before `mark`, so the mark is cleared before the lanes go.
    const SectionRelease release(*this, *section);
    std::optional<SectionMark> mark;
    std::optional<Error> error;
    std::exception_ptr failure;
    try {
      error = StopLanes(host, section, queues, deadline);
      if (!error.has_value()) {
        mark.emplace(*this);
        run();
      }
    } catch (...) {
      failure = std::current_exception();
    }
    // Whether or not the callback ran, the postponed releases run here while
    // no lane can start later work for their accounts, and new ones keep
    // landing in the section until it can be withdrawn empty. The lanes go
    // only then. Reporting or withdrawal failures terminate.
    std::vector<PostponedCleanup> postponed;
    while (!host.WithdrawExclusive(*section, postponed)) {
      for (PostponedCleanup& entry : postponed) {
        ResolveMandatoryCleanup(std::move(entry.promise), entry.cleanup);
      }
      postponed.clear();
    }
    if (failure) {
      std::rethrow_exception(failure);
    }
    return error;
  }

  // Gives a lane created during a section its barrier before its worker
  // starts, so the lane starts stopped and creation never waits for the
  // section. The count is raised before the worker can park, keeping the
  // section from running its callback while an older lane still works.
  // Returns whether the barrier was counted.
  [[nodiscard]] bool BarrierNewLane(
      const KeyQueuePtr& q, const std::shared_ptr<ExclusiveSection>& section) {
    if (!AppendBarrier(q, section)) {
      return false;
    }
    std::lock_guard<std::mutex> lock(m_sectionMutex);
    ++section->expected;
    return true;
  }

  // Undoes a counted `BarrierNewLane` for a lane whose worker failed to start;
  // the lane goes away with the barrier never handled.
  void ForgetNewLaneBarrier(ExclusiveSection& section) {
    m_slowPaths.fetch_sub(kBarrierUnit, std::memory_order_acq_rel);
    {
      std::lock_guard<std::mutex> lock(m_sectionMutex);
      --section.expected;
    }
    m_sectionCv.notify_all();
  }

  // Sends `task` into `q`, blocking with periodic slow-submit notifications
  // until queued, the deadline passes, the strategy hard-stops, or (Dynamic)
  // the queue is retired. See `SendResult`: on the retired path the task is
  // handed back for a retry; on every other failure it is dropped and `error`
  // is set. `deadline` of `time_point::max()` means "wait indefinitely", which
  // only a hard stop can cut short.
  [[nodiscard]] SendResult SendToQueue(
      const KeyQueuePtr& q, OpenPitParamAccountId accountId, TaskPtr task,
      std::chrono::steady_clock::time_point deadline) {
    if (deadline <= std::chrono::steady_clock::now()) {
      Notify([&](Observer& observer) {
        observer.OnSubmitCancelled(PublicAccountId(accountId));
      });
      return {
          Error(ErrorCode::SubmitCancelled, "async submit deadline expired"),
          nullptr, false};
    }

    if (m_tracksIdle) {
      q->pending.fetch_add(1, std::memory_order_relaxed);
    }

    QueuedTask qt;
    qt.accountId = accountId;
    qt.task = std::move(task);
    if (m_observerActive) {
      qt.enqueuedAt = std::chrono::steady_clock::now();
    }

    std::unique_lock<std::mutex> lock(q->mutex);
    const auto start = std::chrono::steady_clock::now();
    auto nextSlow = start + m_slowSubmitThreshold;
    int attempt = 0;
    while (true) {
      if (q->closed) {
        UndoPending(q);
        return {Error(ErrorCode::Stopped, "async engine is stopped"), nullptr,
                false};
      }
      if (q->retired) {
        UndoPending(q);
        // Hand the task back so the caller recreates the queue and retries.
        return {std::nullopt, std::move(qt.task), true};
      }
      if (q->buffer.size() < q->capacity) {
        q->buffer.push_back(std::move(qt));
        const std::size_t depth = q->buffer.size();
        if (m_tracksIdle) {
          q->Touch();
        }
        lock.unlock();
        q->notEmpty.notify_one();
        Notify([&](Observer& observer) {
          observer.OnEnqueue(PublicAccountId(accountId), depth);
        });
        return {std::nullopt, nullptr, false};
      }
      // Queue full. A hard stop stops promising that room will ever appear -
      // workers abort instead of running and the RAII stop joins them - so the
      // producer leaves with the outcome a stopped submit gets rather than
      // making stop wait for space. Checked while holding the lane mutex, the
      // same mutex the wake takes, so the decision to wait cannot be lost.
      if (HardStopped()) {
        UndoPending(q);
        lock.unlock();
        return {Error(ErrorCode::Stopped, "async engine is stopped"), nullptr,
                false};
      }
      // With an active observer, wake periodically to emit slow-submit signals;
      // otherwise wait straight to the deadline.
      const auto wakeAt =
          m_observerActive ? std::min(nextSlow, deadline) : deadline;
      std::cv_status status = std::cv_status::no_timeout;
      if (wakeAt == std::chrono::steady_clock::time_point::max()) {
        q->notFull.wait(lock);
      } else {
        status = q->notFull.wait_until(lock, wakeAt);
      }
      const auto now = std::chrono::steady_clock::now();
      if (now >= deadline) {
        UndoPending(q);
        lock.unlock();
        Notify([&](Observer& observer) {
          observer.OnSubmitCancelled(PublicAccountId(accountId));
        });
        return {
            Error(ErrorCode::SubmitCancelled, "async submit deadline expired"),
            nullptr, false};
      }
      if (status == std::cv_status::timeout && m_observerActive &&
          now >= nextSlow) {
        const auto elapsed = now - start;
        ++attempt;
        nextSlow = now + m_slowSubmitThreshold;
        // Drop the lock so user callbacks never run under the queue mutex.
        lock.unlock();
        Notify([&](Observer& observer) {
          observer.OnQueueFullBlocked(PublicAccountId(accountId), elapsed);
        });
        Notify([&](Observer& observer) {
          observer.OnSlowSubmit(PublicAccountId(accountId), elapsed, attempt);
        });
        lock.lock();
      }
    }
  }

  // Outcome of appending mandatory cleanup to one lane.
  enum class CleanupAppend {
    Queued,    // the lane worker runs it after the work it already accepted.
    Retired,   // Dynamic only: retry routing without creating a lane.
    NoWorker,  // the lane has no worker left; the caller must run it itself.
  };

  // Appends cleanup after a synchronous submit failure without ever blocking
  // its caller: that caller may already have spent its deadline, so waiting for
  // lane capacity here could hold it for an unbounded time. Cleanup is instead
  // appended past the lane capacity, still behind everything the lane has
  // already accepted, and a full or closed lane changes nothing. The whole
  // decision is taken under the lane mutex so retirement cannot slip between
  // the check and the append. Exactly one cleanup task is ever created, so the
  // caller-side fallback and the lane path cannot both run it.
  [[nodiscard]] CleanupAppend AppendMandatoryCleanup(
      const KeyQueuePtr& q, OpenPitParamAccountId accountId,
      const Promise<std::monostate>& promise,
      const std::function<void()>& cleanup) {
    QueuedTask queued;
    queued.accountId = accountId;
    queued.task = std::make_unique<MandatoryCleanupTask>(promise, cleanup);
    if (m_observerActive) {
      queued.enqueuedAt = std::chrono::steady_clock::now();
    }

    std::size_t depth = 0;
    {
      std::lock_guard<std::mutex> lock(q->mutex);
      if (q->retired) {
        return CleanupAppend::Retired;
      }
      if (q->workerExited) {
        return CleanupAppend::NoWorker;
      }
      q->buffer.push_back(std::move(queued));
      depth = q->buffer.size();
      if (m_tracksIdle) {
        q->pending.fetch_add(1, std::memory_order_relaxed);
        q->Touch();
      }
    }
    q->notEmpty.notify_one();
    Notify([&](Observer& observer) {
      observer.OnEnqueue(PublicAccountId(accountId), depth);
    });
    return CleanupAppend::Queued;
  }

  // Queues cleanup on `q`. Returns true when the lane retired and the caller
  // must retry routing. Without a worker, cleanup runs under the account fence.
  [[nodiscard]] bool ScheduleMandatoryCleanupOn(
      const KeyQueuePtr& q, OpenPitParamAccountId accountId,
      Promise<std::monostate> promise, const std::function<void()>& cleanup) {
    switch (AppendMandatoryCleanup(q, accountId, promise, cleanup)) {
      case CleanupAppend::Retired:
        return true;
      case CleanupAppend::NoWorker:
        CleanupAfterWorkerExit(accountId, std::move(promise), cleanup);
        break;
      case CleanupAppend::Queued:
        break;
    }
    return false;
  }

  // A stopped submit can still have registered predecessors that have not
  // acquired their lane mutex. Keep cleanup outside the bounded buffer until
  // they finish: the sole lane worker must remain free to drain tasks that
  // unblock those producers.
  void EnqueueMandatoryCleanupAfterProducerFenceOn(
      const KeyQueuePtr& q, OpenPitParamAccountId accountId,
      Promise<std::monostate> promise, const std::function<void()>& cleanup) {
    QueuedTask queued;
    queued.accountId = accountId;
    queued.task = std::make_unique<MandatoryCleanupTask>(promise, cleanup);
    if (m_observerActive) {
      queued.enqueuedAt = std::chrono::steady_clock::now();
    }

    // Register once before publication; the registry precedes the lane lock.
    RegisterDeferredQueue(q);
    if (m_seams.afterDeferredRegistrationBeforePublication) {
      m_seams.afterDeferredRegistrationBeforePublication();
    }
    std::size_t depth = 0;
    bool workerExited = false;
    {
      std::lock_guard<std::mutex> lock(q->mutex);
      if (q->workerExited) {
        workerExited = true;
      } else {
        q->deferredMandatoryCleanups.push_back(std::move(queued));
        depth = q->buffer.size() + q->deferredMandatoryCleanups.size();
        if (m_tracksIdle) {
          q->pending.fetch_add(1, std::memory_order_relaxed);
          q->Touch();
        }
      }
    }
    if (workerExited) {
      CleanupAfterWorkerExit(accountId, std::move(promise), cleanup);
      return;
    }
    [&]() noexcept {
      // Publication and producer-zero wakes share the lane mutex.
      {
        std::lock_guard<std::mutex> lock(q->mutex);
        q->notEmpty.notify_one();
      }
      Notify([&](Observer& observer) {
        observer.OnEnqueue(PublicAccountId(accountId), depth);
      });
    }();
  }

  [[nodiscard]] bool ProducersDrained() {
    std::lock_guard<std::mutex> lock(m_producerMutex);
    return m_liveProducers == 0;
  }

  void RegisterDeferredQueue(const KeyQueuePtr& q) {
    std::lock_guard<std::mutex> lock(m_deferredMutex);
    if (!q->deferredRegistered) {
      m_deferredQueues.emplace_back(q);
      q->deferredRegistered = true;
    }
  }

  // Wakes every lane that parked cleanup behind the producer fence. Runs on a
  // producer's unwind path (`ProducerGuard`), which cannot report a failure, so
  // it must not allocate: the registry is walked and pruned in place instead of
  // being copied into a temporary.
  //
  // WorkerReady reads the producer count while holding the lane mutex, so
  // notifying under that mutex closes the false-check/wait window. The lock
  // order is deferred registry -> lane mutex -> producer mutex; nothing takes
  // them in the opposite order (registration holds the registry alone, and
  // FinishProducer releases the producer mutex before calling this).
  void WakeDeferredQueues() {
    std::lock_guard<std::mutex> lock(m_deferredMutex);
    auto it = m_deferredQueues.begin();
    while (it != m_deferredQueues.end()) {
      const KeyQueuePtr q = it->lock();
      if (!q) {
        it = m_deferredQueues.erase(it);
        continue;
      }
      {
        std::lock_guard<std::mutex> queueLock(q->mutex);
        q->notEmpty.notify_all();
      }
      ++it;
    }
  }

  // Drains and dispatches `q` on its dedicated worker thread. Exits when the
  // queue is closed (stop) or retired (idle cleanup) and emptied.
  void Worker(const KeyQueuePtr& q) {
    // Resolved once per worker thread so the drain loop a user's build runs
    // carries no test-only branch at all.
    if (m_seams.beforeWorkerWait) {
      WorkerLoop</*WithSeams=*/true>(q);
      return;
    }
    WorkerLoop</*WithSeams=*/false>(q);
  }

  template <bool WithSeams>
  void WorkerLoop(const KeyQueuePtr& q) {
    while (true) {
      std::unique_lock<std::mutex> lock(q->mutex);
      while (!WorkerReady(q)) {
        if constexpr (WithSeams) {
          m_seams.beforeWorkerWait();
        }
        q->notEmpty.wait(lock);
      }

      QueuedTask qt;
      bool freedCapacity = false;
      if (!q->buffer.empty()) {
        qt = std::move(q->buffer.front());
        q->buffer.pop_front();
        freedCapacity = true;
      } else if (!q->deferredMandatoryCleanups.empty()) {
        qt = std::move(q->deferredMandatoryCleanups.front());
        q->deferredMandatoryCleanups.pop_front();
      } else {
        // Publish lane quiescence under the same mutex used by mandatory
        // cleanup enqueue, leaving no gap where work can be appended after the
        // worker has committed to exit.
        q->workerExited = true;
        return;
      }
      lock.unlock();
      if (freedCapacity) {
        q->notFull.notify_one();
      }
      HandleTask(q, std::move(qt));
    }
  }

  [[nodiscard]] bool WorkerReady(const KeyQueuePtr& q) {
    return !q->buffer.empty() ||
           (!q->deferredMandatoryCleanups.empty() && ProducersDrained()) ||
           ((q->closed || q->retired) && q->deferredMandatoryCleanups.empty());
  }

  // Runs (or, under hard stop, aborts) one dequeued task. For idle-tracking
  // strategies, decrements `pending` only after the task is fully handled so
  // cleanup cannot retire a queue mid-task even when its buffer is empty.
  void HandleTask(const KeyQueuePtr& q, QueuedTask qt) {
    struct PendingGuard {
      KeyQueue* q;
      bool tracks;
      ~PendingGuard() {
        if (tracks) {
          q->pending.fetch_sub(1, std::memory_order_relaxed);
        }
      }
    } guard{q.get(), m_tracksIdle};

    const std::uint64_t slowPaths = m_slowPaths.load(std::memory_order_acquire);
    if (slowPaths != 0 && HandleSlowTask(q, qt, slowPaths)) {
      return;
    }
    if (!m_observerActive) {
      qt.task->Run();
      if (m_tracksIdle) {
        q->Touch();
      }
      return;
    }
    Notify([&](Observer& observer) {
      observer.OnDequeue(PublicAccountId(qt.accountId),
                         std::chrono::steady_clock::now() - qt.enqueuedAt);
    });
    const auto started = std::chrono::steady_clock::now();
    qt.task->Run();
    Notify([&](Observer& observer) {
      observer.OnComplete(PublicAccountId(qt.accountId),
                          std::chrono::steady_clock::now() - started);
    });
    if (m_tracksIdle) {
      q->Touch();
    }
  }

  // Handles a task while a hard stop or a section barrier is outstanding. The
  // one load of `m_slowPaths` per task in `HandleTask` decides both, as the
  // hard-stop check alone did before sections existed, so ordinary tasks pay
  // nothing for sections. Returns false when `qt` takes the regular path.
  [[nodiscard]] bool HandleSlowTask(const KeyQueuePtr& q, QueuedTask& qt,
                                    std::uint64_t slowPaths) {
    const bool barrier = qt.enqueuedAt == kSectionBarrierStamp;
    if (barrier) {
      m_slowPaths.fetch_sub(kBarrierUnit, std::memory_order_acq_rel);
    }
    if ((slowPaths & kHardStopBit) != 0) {
      [&]() noexcept {
        qt.task->Abort(Error(ErrorCode::Stopped, "async engine is stopped"));
      }();
      if (!barrier) {
        Notify([&](Observer& observer) {
          observer.OnComplete(PublicAccountId(qt.accountId),
                              std::chrono::nanoseconds(0));
        });
      }
      return true;
    }
    if (!barrier) {
      return false;
    }
    // A barrier stays out of observer metrics.
    qt.task->Run();
    if (m_tracksIdle) {
      q->Touch();
    }
    return true;
  }

  // Marks the strategy stopped under the producer gate. A producer is either
  // fully registered before this point or observes stop and never enters. An
  // exclusive section still preparing is woken to end itself.
  void SignalStop() {
    {
      std::lock_guard<std::mutex> lock(m_producerMutex);
      m_stopRequested.store(true, std::memory_order_release);
    }
    {
      // A section checks for stop under this mutex, so passing through it
      // after the store means the check either sees the stop or is already
      // waiting for the notify below.
      std::lock_guard<std::mutex> lock(m_sectionMutex);
    }
    m_sectionCv.notify_all();
  }

  // Marks a hard stop so workers abort rather than run dequeued tasks.
  // Idempotent.
  void SignalHardStop() {
    m_slowPaths.fetch_or(kHardStopBit, std::memory_order_release);
  }

  // Starts the dedicated worker thread for `q`, wrapping `Worker` so the live
  // count is incremented before the thread runs and decremented (with a
  // notify) when it exits. The live count drives the deadline-honoring wait in
  // `WaitWorkersDrained` without needing a timed `std::thread::join`.
  void StartWorker(const KeyQueuePtr& q) {
    m_liveWorkers.fetch_add(1, std::memory_order_relaxed);
    try {
      q->worker = std::thread([this, q] {
        // Set once per worker thread, so tasks pay nothing for it.
        const LaneMark lane{this, /*section=*/false, nullptr, q.get()};
        CurrentLaneMark() = &lane;
        try {
          Worker(q);
        } catch (...) {
          std::terminate();
        }
        {
          std::lock_guard<std::mutex> lock(q->mutex);
          q->workerExited = true;
        }
        {
          std::lock_guard<std::mutex> lock(m_doneMutex);
          m_liveWorkers.fetch_sub(1, std::memory_order_relaxed);
        }
        m_doneCv.notify_all();
      });
    } catch (...) {
      m_liveWorkers.fetch_sub(1, std::memory_order_relaxed);
      throw;
    }
  }

  // Closes a queue so its worker drains and exits. Producers blocked on a full
  // queue are woken and observe `closed`.
  static void CloseQueue(const KeyQueuePtr& q) {
    {
      std::lock_guard<std::mutex> lock(q->mutex);
      q->closed = true;
    }
    q->notEmpty.notify_all();
    q->notFull.notify_all();
  }

  static void CloseQueues(const std::vector<KeyQueuePtr>& queues) {
    for (const KeyQueuePtr& q : queues) {
      CloseQueue(q);
    }
  }

  // Releases producers blocked on a full queue after a hard stop, without
  // closing the queue: closing is a separate step that may only happen once
  // every registered producer has left, or it would race a queue one of them is
  // still creating. Notifying under the lane mutex is what makes the wake
  // race-free - a producer decides to wait while holding that mutex, so it
  // either has not decided yet (and then observes the hard stop) or is already
  // waiting (and is woken here).
  static void WakeBlockedProducers(const KeyQueuePtr& q) {
    std::lock_guard<std::mutex> lock(q->mutex);
    q->notFull.notify_all();
  }

  static void WakeBlockedProducers(const std::vector<KeyQueuePtr>& queues) {
    for (const KeyQueuePtr& q : queues) {
      WakeBlockedProducers(q);
    }
  }

  // Blocks until every worker thread has exited, or `deadline` passes first
  // deadline timeout the workers are still running; the threads themselves are
  // joined later, unconditionally, in `JoinAll` (which the destructor calls
  // after a hard stop, so termination is guaranteed). Returns true if drained.
  [[nodiscard]] bool WaitWorkersDrained(
      std::chrono::steady_clock::time_point deadline) {
    std::unique_lock<std::mutex> lock(m_doneMutex);
    const auto drained = [this] {
      return m_liveWorkers.load(std::memory_order_relaxed) == 0;
    };
    if (deadline == std::chrono::steady_clock::time_point::max()) {
      m_doneCv.wait(lock, drained);
      return true;
    }
    return m_doneCv.wait_until(lock, deadline, drained);
  }

  [[nodiscard]] bool WaitLifecycleDrained(
      std::chrono::steady_clock::time_point deadline) {
    std::unique_lock<std::mutex> lock(m_producerMutex);
    const auto drained = [this] {
      return m_liveProducers == 0 && m_liveMandatoryCleanups == 0;
    };
    if (deadline == std::chrono::steady_clock::time_point::max()) {
      m_producerCv.wait(lock, drained);
      return true;
    }
    return m_producerCv.wait_until(lock, deadline, drained);
  }

  // The final stop handoff rechecks zero and seals registration under one
  // mutex. A cleanup racing worker drain is therefore either included in this
  // wait or linearized after the completed stop.
  [[nodiscard]] bool SealLifecycleDrained(
      std::chrono::steady_clock::time_point deadline) {
    std::unique_lock<std::mutex> lock(m_producerMutex);
    const auto drained = [this] {
      return m_liveProducers == 0 && m_liveMandatoryCleanups == 0;
    };
    if (deadline == std::chrono::steady_clock::time_point::max()) {
      m_producerCv.wait(lock, drained);
    } else if (!m_producerCv.wait_until(lock, deadline, drained)) {
      return false;
    }
    m_lifecycleSealed = true;
    return true;
  }

  // Joins every queue-owned worker thread unconditionally. Called only once the
  // workers are guaranteed to be exiting (queues closed and, in the destructor,
  // a hard stop signalled). Safe to call on already-exited threads.
  static void JoinAll(const std::vector<KeyQueuePtr>& queues) {
    for (const KeyQueuePtr& q : queues) {
      JoinLane(q);
    }
  }

  static void JoinLane(const KeyQueuePtr& q) {
    if (q->worker.joinable() && WorkerSeams::BeforeJoin()) {
      WorkerSeams::BeforeJoin()();
    }
    q->Join();
  }

  // The process-wide shared no-op observer; identity-compared to detect that
  // no real observer is wired (so the hot path can skip timestamps).
  [[nodiscard]] static Observer* SharedNoop() {
    static NoopObserver instance;
    return &instance;
  }

 private:
  [[nodiscard]] bool RegisterProducer() {
    std::lock_guard<std::mutex> lock(m_producerMutex);
    if (m_stopRequested.load(std::memory_order_relaxed)) {
      return false;
    }
    ++m_liveProducers;
    return true;
  }

  void FinishProducer() {
    {
      std::lock_guard<std::mutex> lock(m_producerMutex);
      --m_liveProducers;
    }
    m_producerCv.notify_all();
    WakeDeferredQueues();
  }

  [[nodiscard]] std::shared_ptr<MandatoryCleanupRegistration>
  RegisterMandatoryCleanup() {
    {
      std::lock_guard<std::mutex> lock(m_producerMutex);
      if (m_lifecycleSealed) {
        return nullptr;
      }
      ++m_liveMandatoryCleanups;
    }
    try {
      return std::make_shared<MandatoryCleanupRegistration>(*this);
    } catch (...) {
      FinishMandatoryCleanup();
      throw;
    }
  }

  void FinishMandatoryCleanup() {
    {
      std::lock_guard<std::mutex> lock(m_producerMutex);
      --m_liveMandatoryCleanups;
    }
    m_producerCv.notify_all();
  }

  void UndoPending(const KeyQueuePtr& q) const {
    if (m_tracksIdle) {
      q->pending.fetch_sub(1, std::memory_order_relaxed);
    }
  }

  // Stops one lane worker for a section; see `ExclusiveSection`.
  class SectionBarrierTask final : public Task {
   public:
    SectionBarrierTask(Base& base, std::shared_ptr<ExclusiveSection> section)
        : m_base(&base), m_section(std::move(section)) {}

    void Run() override { m_base->ParkLane(*m_section); }

    // Runs instead of `Run` under a hard stop; see `AbortBarrier`.
    void Abort(Error /*error*/) override { m_base->AbortBarrier(*m_section); }

   private:
    Base* m_base;
    std::shared_ptr<ExclusiveSection> m_section;
  };

  // Holds the one-at-a-time section turn taken by `TakeSectionTurn`.
  class SectionTurn {
   public:
    explicit SectionTurn(Base& base) noexcept : m_base(&base) {}

    ~SectionTurn() {
      {
        std::lock_guard<std::mutex> lock(m_base->m_sectionMutex);
        m_base->m_sectionTurnTaken = false;
      }
      m_base->m_sectionCv.notify_all();
    }

    SectionTurn(const SectionTurn&) = delete;
    SectionTurn& operator=(const SectionTurn&) = delete;

   private:
    Base* m_base;
  };

  // Lets a section's lanes continue when it goes out of scope.
  class SectionRelease {
   public:
    SectionRelease(Base& base, ExclusiveSection& section) noexcept
        : m_base(&base), m_section(&section) {}

    ~SectionRelease() { m_base->ReleaseSection(*m_section); }

    SectionRelease(const SectionRelease&) = delete;
    SectionRelease& operator=(const SectionRelease&) = delete;

   private:
    Base* m_base;
    ExclusiveSection* m_section;
  };

  // Marks the section thread until its lanes continue.
  class SectionMark {
   public:
    explicit SectionMark(Base& base) noexcept
        : m_mark{&base, /*section=*/true, CurrentLaneMark()} {
      CurrentLaneMark() = &m_mark;
    }
    ~SectionMark() { CurrentLaneMark() = m_mark.outer; }
    SectionMark(const SectionMark&) = delete;
    SectionMark& operator=(const SectionMark&) = delete;

   private:
    LaneMark m_mark;
  };

  [[nodiscard]] bool MarkedOnThisThread(bool sectionOnly) const noexcept {
    for (const LaneMark* mark = CurrentLaneMark(); mark != nullptr;
         mark = mark->outer) {
      if (mark->owner == this && (mark->section || !sectionOnly)) {
        return true;
      }
    }
    return false;
  }

  [[nodiscard]] static Error SectionDeadline() {
    return Error(ErrorCode::SubmitCancelled,
                 "async exclusive section deadline expired before its callback "
                 "started");
  }

  [[nodiscard]] std::optional<Error> TakeSectionTurn(
      std::chrono::steady_clock::time_point deadline) {
    std::unique_lock<std::mutex> lock(m_sectionMutex);
    (void)m_sectionCv.wait_until(
        lock, deadline, [this] { return !m_sectionTurnTaken || IsStopped(); });
    if (IsStopped()) {
      return Error(ErrorCode::Stopped, "async engine is stopped");
    }
    if (m_sectionTurnTaken) {
      return SectionDeadline();
    }
    m_sectionTurnTaken = true;
    return std::nullopt;
  }

  // Waits out the off-lane releases already running, then stops every lane
  // in `queues`, and every lane created meanwhile, behind its accepted work.
  // A stop signal, a hard stop that aborts a barrier, or the deadline ends
  // the section before the callback; each is checked once more after the
  // waits, which return at once when there is nothing to wait for.
  template <typename Host>
  [[nodiscard]] std::optional<Error> StopLanes(
      Host& host, const std::shared_ptr<ExclusiveSection>& section,
      const std::vector<KeyQueuePtr>& queues,
      std::chrono::steady_clock::time_point deadline) {
    if (!host.AwaitRunningCleanups(deadline)) {
      return SectionDeadline();
    }
    std::size_t barriers = 0;
    for (const KeyQueuePtr& q : queues) {
      if (AppendBarrier(q, section)) {
        ++barriers;
      }
    }
    std::unique_lock<std::mutex> lock(m_sectionMutex);
    section->expected += barriers;
    (void)m_sectionCv.wait_until(lock, deadline, [&] {
      return section->parked >= section->expected || section->aborted ||
             IsStopped();
    });
    if (section->aborted || IsStopped()) {
      return Error(ErrorCode::Stopped, "async engine is stopped");
    }
    if (section->parked < section->expected ||
        std::chrono::steady_clock::now() >= deadline) {
      return SectionDeadline();
    }
    return std::nullopt;
  }

  // Appends a section barrier to `q` past its capacity, behind everything the
  // lane has already accepted, without ever blocking. Observers never see it.
  // The barrier is counted in `m_slowPaths` once it is in the buffer, under
  // the lane mutex the worker takes to dequeue it, so its worker sends it down
  // the slow path. Returns false for a lane that takes no more work.
  [[nodiscard]] bool AppendBarrier(
      const KeyQueuePtr& q, const std::shared_ptr<ExclusiveSection>& section) {
    QueuedTask queued;
    queued.task = std::make_unique<SectionBarrierTask>(*this, section);
    queued.enqueuedAt = kSectionBarrierStamp;
    {
      std::lock_guard<std::mutex> lock(q->mutex);
      if (q->closed || q->retired || q->workerExited) {
        return false;
      }
      q->buffer.push_back(std::move(queued));
      m_slowPaths.fetch_add(kBarrierUnit, std::memory_order_acq_rel);
      if (m_tracksIdle) {
        q->pending.fetch_add(1, std::memory_order_relaxed);
        q->Touch();
      }
    }
    q->notEmpty.notify_one();
    return true;
  }

  // Runs on a lane worker: counts the lane as stopped and holds it until the
  // section ends. A barrier that reaches its worker after the section ended
  // returns at once.
  void ParkLane(ExclusiveSection& section) {
    std::unique_lock<std::mutex> lock(m_sectionMutex);
    if (section.released) {
      return;
    }
    ++section.parked;
    m_sectionCv.notify_all();
    m_sectionCv.wait(lock, [&section] { return section.released; });
  }

  // Runs on a lane worker under a hard stop: the section cannot stop every
  // lane and fails. The worker still holds its lane until the section ends,
  // so the releases the section runs before it lets the lanes go cannot
  // overlap this lane's aborted work.
  void AbortBarrier(ExclusiveSection& section) {
    std::unique_lock<std::mutex> lock(m_sectionMutex);
    section.aborted = true;
    m_sectionCv.notify_all();
    m_sectionCv.wait(lock, [&section] { return section.released; });
  }

  void ReleaseSection(ExclusiveSection& section) {
    {
      std::lock_guard<std::mutex> lock(m_sectionMutex);
      section.released = true;
    }
    m_sectionCv.notify_all();
  }

  Observer* m_observer;
  std::size_t m_queueCapacity;
  std::chrono::nanoseconds m_slowSubmitThreshold;
  WorkerSeams m_seams;
  bool m_observerActive;
  bool m_tracksIdle;
  std::mutex m_deferredMutex;
  std::vector<std::weak_ptr<KeyQueue>> m_deferredQueues;
  std::mutex m_producerMutex;
  std::condition_variable m_producerCv;
  std::size_t m_liveProducers = 0;
  std::size_t m_liveMandatoryCleanups = 0;
  bool m_lifecycleSealed = false;
  std::mutex m_doneMutex;
  std::condition_variable m_doneCv;
  std::atomic<std::size_t> m_liveWorkers{0};
  std::atomic_bool m_stopRequested{false};
  // `kHardStopBit` marks a hard stop; the rest counts, in `kBarrierUnit`
  // steps, the section barriers queued but not yet handled. One word lets the
  // worker's per-task hard-stop check also tell it when a barrier may come
  // next, so ordinary tasks pay nothing for sections.
  static constexpr std::uint64_t kHardStopBit = 1;
  static constexpr std::uint64_t kBarrierUnit = 2;
  std::atomic<std::uint64_t> m_slowPaths{0};
  // Exclusive sections: the turn, every section's counters, and the wakes of
  // section threads and stopped lane workers share one mutex and condition.
  std::mutex m_sectionMutex;
  std::condition_variable m_sectionCv;
  bool m_sectionTurnTaken = false;
};

// The internal dispatch interface the engine drives. Owns its worker threads;
// the destructor must guarantee no thread outlives it (RAII).
class Strategy {
 public:
  Strategy() = default;
  Strategy(const Strategy&) = delete;
  Strategy& operator=(const Strategy&) = delete;
  virtual ~Strategy() = default;

  // Enqueues `task` for `accountId`, blocking up to `deadline` for queue space.
  // A synchronous failure invokes `onFailure` before the producer lifecycle
  // obligation is released; accepted tasks transfer that obligation to their
  // worker lane.
  virtual void Submit(OpenPitParamAccountId accountId, TaskPtr task,
                      std::chrono::steady_clock::time_point deadline,
                      SubmitFailureHandler onFailure) = 0;

  // Guarantees cleanup is ordered after the account lane's existing work and
  // reports cleanup exceptions without stopping the worker.
  [[nodiscard]] virtual Future<std::monostate> ScheduleMandatoryCleanup(
      OpenPitParamAccountId accountId, std::function<void()> cleanup) = 0;

  // Refuses new submits and waits for every queued task to run. Returns false
  // if `deadline` passes before workers drain (partial stop; a hard stop may
  // follow).
  [[nodiscard]] virtual bool StopGraceful(
      std::chrono::steady_clock::time_point deadline) = 0;

  // Refuses new submits, aborts not-yet-started tasks with `Stopped`, and waits
  // for the in-flight task per worker to finish. Returns false on deadline.
  [[nodiscard]] virtual bool StopHard(
      std::chrono::steady_clock::time_point deadline) = 0;

  // Runs `run` on the calling thread while every lane is stopped behind the
  // work it already accepted; see `AsyncEngine::Exclusive`. `deadline` bounds
  // only the wait before `run` starts. An exception from `run` propagates
  // after the lanes continue.
  [[nodiscard]] virtual std::optional<Error> Exclusive(
      std::chrono::steady_clock::time_point deadline,
      const std::function<void()>& run) = 0;

  // True on the section thread or the worker of the account's lane. Used only
  // by implicit handle release.
  [[nodiscard]] virtual bool SerializesAccount(
      OpenPitParamAccountId accountId) noexcept = 0;

  // Hard-stops and joins workers while the engine's cleanup route is alive.
  virtual void Terminate() noexcept = 0;
};

//------------------------------------------------------------------------------
// Sharded

// Fans accounts across a fixed worker pool chosen at build time. Routing is a
// single multiply-shift over a Fibonacci mix; the send path takes only that
// queue's mutex. Cheapest hot path, O(1) memory regardless of account
// population, no per-account observability. A hot account saturates one shard.
class ShardedStrategy final : public Strategy, private Base {
 public:
  ShardedStrategy(const BaseConfig& cfg, std::size_t shardCount)
      : ShardedStrategy(cfg, shardCount, WorkerSeams{}) {}

  // Seamed construction is internal to the binding: the builder chain never
  // exposes it, so a user-built strategy always takes the empty seams above.
  ShardedStrategy(const BaseConfig& cfg, std::size_t shardCount,
                  WorkerSeams seams)
      : Base(cfg, /*tracksIdle=*/false, std::move(seams)) {
    try {
      m_shards.reserve(shardCount);
      for (std::size_t i = 0; i < shardCount; ++i) {
        auto q = std::make_shared<KeyQueue>(queueCapacity());
        m_shards.push_back(q);
        StartWorker(q);
      }
    } catch (...) {
      SignalHardStop();
      SignalStop();
      CloseQueues(m_shards);
      (void)WaitWorkersDrained(std::chrono::steady_clock::time_point::max());
      JoinAll(m_shards);
      throw;
    }
  }

  ~ShardedStrategy() override { Terminate(); }

  void Submit(OpenPitParamAccountId accountId, TaskPtr task,
              std::chrono::steady_clock::time_point deadline,
              SubmitFailureHandler onFailure) override {
    assert(!RunningInlineCleanup(*this) &&
           "callbacks must not call or release handles of the same engine");
    ProducerGuard producer(*this);
    if (!producer) {
      onFailure(Error(ErrorCode::Stopped, "async engine is stopped"));
      return;
    }
    // Sharded queues are never retired, so `retired` cannot be set here.
    SendResult result =
        SendToQueue(ShardFor(accountId), accountId, std::move(task), deadline);
    if (result.error.has_value()) {
      onFailure(std::move(*result.error));
    }
  }

  [[nodiscard]] Future<std::monostate> ScheduleMandatoryCleanup(
      OpenPitParamAccountId accountId, std::function<void()> cleanup) override {
    assert(!RunningInlineCleanup(*this) &&
           "callbacks must not call or release handles of the same engine");
    cleanup = TrackMandatoryCleanup(std::move(cleanup));
    Promise<std::monostate> promise;
    Future<std::monostate> future = promise.GetFuture();
    RouteMandatoryCleanup(accountId, std::move(promise), cleanup);
    return future;
  }

  [[nodiscard]] bool StopGraceful(
      std::chrono::steady_clock::time_point deadline) override {
    SignalStop();
    if (!WaitLifecycleDrained(deadline)) {
      return false;
    }
    CloseQueues(m_shards);
    const bool drained = WaitWorkersDrained(deadline);
    if (!drained || !SealLifecycleDrained(deadline)) {
      return false;
    }
    JoinAll(m_shards);
    return true;
  }

  [[nodiscard]] bool StopHard(
      std::chrono::steady_clock::time_point deadline) override {
    SignalHardStop();
    SignalStop();
    WakeBlockedProducers(m_shards);
    if (!WaitLifecycleDrained(deadline)) {
      return false;
    }
    CloseQueues(m_shards);
    const bool drained = WaitWorkersDrained(deadline);
    if (!drained || !SealLifecycleDrained(deadline)) {
      return false;
    }
    JoinAll(m_shards);
    return true;
  }

  [[nodiscard]] std::optional<Error> Exclusive(
      std::chrono::steady_clock::time_point deadline,
      const std::function<void()>& run) override {
    return RunExclusive(*this, deadline, run);
  }

  [[nodiscard]] bool SerializesAccount(
      OpenPitParamAccountId accountId) noexcept override {
    return SerializesQueueOnThisThread(ShardFor(accountId).get());
  }

 private:
  friend class Base;

  void RouteMandatoryCleanup(OpenPitParamAccountId accountId,
                             Promise<std::monostate> promise,
                             const std::function<void()>& cleanup) {
    ProducerGuard producer(*this);
    if (!producer) {
      EnqueueMandatoryCleanupAfterProducerFenceOn(
          ShardFor(accountId), accountId, std::move(promise), cleanup);
      return;
    }
    (void)ScheduleMandatoryCleanupOn(ShardFor(accountId), accountId,
                                     std::move(promise), cleanup);
  }

  // Exclusive-section host; see `Base::RunExclusive`. Every shard takes a
  // barrier: a shard carries many accounts, and one barrier stops them all.
  // No Sharded release runs off-lane while every worker lives, so nothing is
  // running or postponed.
  [[nodiscard]] std::vector<KeyQueuePtr> InstallExclusive(
      const std::shared_ptr<ExclusiveSection>& /*section*/) const {
    return m_shards;
  }

  [[nodiscard]] static bool AwaitRunningCleanups(
      std::chrono::steady_clock::time_point /*deadline*/) noexcept {
    return true;
  }

  [[nodiscard]] static bool WithdrawExclusive(
      ExclusiveSection& /*section*/,
      std::vector<PostponedCleanup>& /*postponed*/) noexcept {
    return true;
  }

  // 2^64 / phi rounded to the nearest odd integer (Knuth, TAOCP vol. 3).
  static constexpr std::uint64_t kFibonacciMultiplier = 11400714819323198485ULL;

  [[nodiscard]] const KeyQueuePtr& ShardFor(
      OpenPitParamAccountId accountId) const {
    // Lemire multiply-shift over the HIGH 64 bits of the 128-bit product: the
    // low bits of an odd-constant multiply mix poorly, so index via the high
    // half rather than a modulo. Result is in [0, size).
    const std::uint64_t mixed =
        static_cast<std::uint64_t>(accountId) * kFibonacciMultiplier;
    const std::size_t index =
        static_cast<std::size_t>(MulHigh64(mixed, m_shards.size()));
    return m_shards[index];
  }

  // High 64 bits of the 128-bit product a*b, computed from 32-bit limbs so the
  // binding needs no 128-bit integer extension (portable C++17).
  [[nodiscard]] static std::uint64_t MulHigh64(std::uint64_t a,
                                               std::uint64_t b) {
    const std::uint64_t aLo = a & 0xFFFFFFFFULL;
    const std::uint64_t aHi = a >> 32U;
    const std::uint64_t bLo = b & 0xFFFFFFFFULL;
    const std::uint64_t bHi = b >> 32U;
    const std::uint64_t loLo = aLo * bLo;
    const std::uint64_t hiLo = aHi * bLo;
    const std::uint64_t loHi = aLo * bHi;
    const std::uint64_t hiHi = aHi * bHi;
    const std::uint64_t cross =
        (loLo >> 32U) + (hiLo & 0xFFFFFFFFULL) + (loHi & 0xFFFFFFFFULL);
    return hiHi + (hiLo >> 32U) + (loHi >> 32U) + (cross >> 32U);
  }

  void Terminate() noexcept override {
    // RAII backstop: if the owner never stopped us, hard-stop now so no worker
    // thread outlives this object. After a hard stop every worker exits and
    // every blocked producer is released, so both waits below terminate.
    SignalHardStop();
    SignalStop();
    WakeBlockedProducers(m_shards);
    (void)WaitLifecycleDrained(std::chrono::steady_clock::time_point::max());
    CloseQueues(m_shards);
    JoinAll(m_shards);
    (void)SealLifecycleDrained(std::chrono::steady_clock::time_point::max());
  }

  std::vector<KeyQueuePtr> m_shards;
};

//------------------------------------------------------------------------------
// Dynamic

// Lazily creates one queue (and worker) per active account. Idle queues are
// retired by a background cleanup thread. The live per-account queue count is
// bounded by `maxQueues` (0 = unbounded), so a cap of n means n usable account
// queues. Full per-account isolation and per-account observer signals come at
// the cost of a map lookup per submit and a cleanup thread.
class DynamicStrategy final : public Strategy, private Base {
 public:
  DynamicStrategy(const BaseConfig& cfg, std::size_t maxQueues,
                  std::chrono::nanoseconds idleCleanupAfter, bool capEnabled)
      : DynamicStrategy(cfg, maxQueues, idleCleanupAfter, capEnabled,
                        WorkerSeams{}) {}

  // Seamed construction is internal to the binding: the builder chain never
  // exposes it, so a user-built strategy always takes the empty seams above.
  DynamicStrategy(const BaseConfig& cfg, std::size_t maxQueues,
                  std::chrono::nanoseconds idleCleanupAfter, bool capEnabled,
                  WorkerSeams seams)
      : Base(cfg,
             /*tracksIdle=*/idleCleanupAfter > std::chrono::nanoseconds(0),
             std::move(seams)),
        m_maxQueues(maxQueues),
        m_capEnabled(capEnabled),
        m_idleCleanupAfter(idleCleanupAfter) {
    try {
      if (tracksIdle()) {
        // Scan at a fifth of the idle window, never tighter than the default.
        auto period = idleCleanupAfter / 5;
        if (period < std::chrono::seconds(1)) {
          period = kDefaultIdleCleanupPeriod;
        }
        m_cleanupPeriod = period;
        m_cleanup = std::thread([this] { CleanupLoop(); });
      }
    } catch (...) {
      SignalHardStop();
      SignalStop();
      const std::vector<KeyQueuePtr> queues = MarkStoppingAndSnapshot();
      CloseQueues(queues);
      (void)WaitWorkersDrained(std::chrono::steady_clock::time_point::max());
      JoinAll(queues);
      throw;
    }
  }

  ~DynamicStrategy() override { Terminate(); }

  void Submit(OpenPitParamAccountId accountId, TaskPtr task,
              std::chrono::steady_clock::time_point deadline,
              SubmitFailureHandler onFailure) override {
    assert(!RunningInlineCleanup(*this) &&
           "callbacks must not call or release handles of the same engine");
    // A queue can be retired by idle cleanup between lookup and send; the send
    // hands the task back and we loop to recreate a fresh queue. The loop is
    // bounded in practice: retirement requires an idle window, so a live
    // producer re-creates faster than cleanup can retire.
    ProducerGuard producer(*this);
    if (!producer) {
      onFailure(Error(ErrorCode::Stopped, "async engine is stopped"));
      return;
    }
    while (true) {
      bool created = false;
      std::size_t total = 0;
      std::optional<Error> err;
      KeyQueuePtr q = GetOrCreate(accountId, created, total, err);
      if (err.has_value()) {
        onFailure(std::move(*err));
        return;
      }
      if (created) {
        Notify([&](Observer& observer) {
          observer.OnQueueCreated(PublicAccountId(accountId), total);
        });
      }
      SendResult result = SendToQueue(q, accountId, std::move(task), deadline);
      if (result.retired) {
        // Recover the task and retry against a freshly created queue.
        task = std::move(result.task);
        continue;
      }
      if (result.error.has_value()) {
        onFailure(std::move(*result.error));
      }
      return;
    }
  }

  [[nodiscard]] Future<std::monostate> ScheduleMandatoryCleanup(
      OpenPitParamAccountId accountId, std::function<void()> cleanup) override {
    assert(!RunningInlineCleanup(*this) &&
           "callbacks must not call or release handles of the same engine");
    cleanup = TrackMandatoryCleanup(std::move(cleanup));
    Promise<std::monostate> promise;
    Future<std::monostate> future = promise.GetFuture();
    RouteMandatoryCleanup(accountId, std::move(promise), cleanup);
    return future;
  }

  [[nodiscard]] bool StopGraceful(
      std::chrono::steady_clock::time_point deadline) override {
    StopCleanup();
    SignalStop();
    if (!WaitLifecycleDrained(deadline)) {
      return false;
    }
    std::vector<KeyQueuePtr> queues = MarkStoppingAndSnapshot();
    CloseQueues(queues);
    const bool drained = WaitWorkersDrained(deadline);
    if (!drained || !SealLifecycleDrained(deadline)) {
      return false;
    }
    JoinAll(queues);
    return true;
  }

  [[nodiscard]] bool StopHard(
      std::chrono::steady_clock::time_point deadline) override {
    StopCleanup();
    SignalHardStop();
    SignalStop();
    WakeBlockedLaneProducers();
    if (!WaitLifecycleDrained(deadline)) {
      return false;
    }
    std::vector<KeyQueuePtr> queues = MarkStoppingAndSnapshot();
    CloseQueues(queues);
    const bool drained = WaitWorkersDrained(deadline);
    if (!drained || !SealLifecycleDrained(deadline)) {
      return false;
    }
    JoinAll(queues);
    return true;
  }

  [[nodiscard]] std::optional<Error> Exclusive(
      std::chrono::steady_clock::time_point deadline,
      const std::function<void()>& run) override {
    return RunExclusive(*this, deadline, run);
  }

  [[nodiscard]] bool SerializesAccount(
      OpenPitParamAccountId accountId) noexcept override {
    if (SerializesQueueOnThisThread(nullptr)) {
      return true;
    }
    // Only a worker of this strategy can own the account's lane: other
    // threads answer without the routing lock.
    const KeyQueue* own = WorkerQueueOnThisThread();
    if (own == nullptr) {
      return false;
    }
    std::lock_guard<std::mutex> lock(m_mutex);
    const auto it = m_queues.find(accountId);
    return it != m_queues.end() && it->second.get() == own;
  }

 private:
  friend class Base;

  void RouteMandatoryCleanup(OpenPitParamAccountId accountId,
                             Promise<std::monostate> promise,
                             const std::function<void()>& cleanup) {
    ProducerGuard producer(*this);
    if (!producer) {
      CleanupAfterStoppedLane(accountId, std::move(promise), cleanup);
      return;
    }
    while (true) {
      KeyQueuePtr q;
      bool stopping;
      {
        std::lock_guard<std::mutex> lock(m_mutex);
        stopping = m_stopping;
        const auto it = m_queues.find(accountId);
        if (it != m_queues.end()) {
          q = it->second;
        }
      }
      if (stopping) {
        CleanupAfterStoppedLane(accountId, std::move(promise), cleanup);
        return;
      }
      if (!q) {
        if (CleanupWithoutLiveLane(accountId, promise, cleanup)) {
          return;
        }
      } else if (!ScheduleMandatoryCleanupOn(q, accountId, promise, cleanup)) {
        return;
      }
      // A retired lane or a changed routing fence requires a fresh lookup.
    }
  }

  // Exclusive-section host; see `Base::RunExclusive`. The section is
  // published under `m_mutex`, the mutex every Dynamic slow path that reads it
  // holds: lane creation and the two off-lane release paths. Idle retirement
  // needs no check: a lane that holds a barrier is not idle, and a lane
  // retired first refuses its barrier and runs nothing more.
  [[nodiscard]] std::vector<KeyQueuePtr> InstallExclusive(
      const std::shared_ptr<ExclusiveSection>& section) {
    std::vector<KeyQueuePtr> queues;
    std::lock_guard<std::mutex> lock(m_mutex);
    queues.reserve(m_queues.size());
    for (const auto& entry : m_queues) {
      queues.push_back(entry.second);
    }
    m_activeSection = section;
    return queues;
  }

  // An off-lane release that passed its section check before the section was
  // installed holds its account in `m_cleanupAccounts` while it runs.
  [[nodiscard]] bool AwaitRunningCleanups(
      std::chrono::steady_clock::time_point deadline) {
    std::unique_lock<std::mutex> lock(m_mutex);
    return m_routeCv.wait_until(lock, deadline,
                                [this] { return m_cleanupAccounts.empty(); });
  }

  [[nodiscard]] bool WithdrawExclusive(
      ExclusiveSection& section,
      std::vector<PostponedCleanup>& postponed) noexcept {
    std::lock_guard<std::mutex> lock(m_mutex);
    postponed.swap(section.postponed);
    if (!postponed.empty()) {
      return false;
    }
    m_activeSection.reset();
    return true;
  }

  void CleanupAfterStoppedLane(OpenPitParamAccountId accountId,
                               Promise<std::monostate> promise,
                               const std::function<void()>& cleanup) {
    KeyQueuePtr q;
    {
      std::unique_lock<std::mutex> lock(m_mutex);
      m_routeCv.wait(lock, [this, accountId] {
        return m_cleanupAccounts.find(accountId) == m_cleanupAccounts.end();
      });
      auto it = m_queues.find(accountId);
      if (it != m_queues.end()) {
        q = it->second;
      } else if (m_activeSection) {
        // Stop was signalled while a section still runs: the release must
        // not run beside it, so the section runs it before it lets the lanes
        // go.
        m_activeSection->postponed.push_back(
            PostponedCleanup{std::move(promise), cleanup});
        return;
      } else {
        m_cleanupAccounts.insert(accountId);
      }
    }
    if (q) {
      EnqueueMandatoryCleanupAfterProducerFenceOn(q, accountId,
                                                  std::move(promise), cleanup);
      return;
    }
    ResolveFencedCleanup(accountId, std::move(promise), cleanup);
  }

  [[nodiscard]] bool CleanupWithoutLiveLane(
      OpenPitParamAccountId accountId, Promise<std::monostate> promise,
      const std::function<void()>& cleanup) {
    {
      std::unique_lock<std::mutex> lock(m_mutex);
      if (m_stopping || m_queues.find(accountId) != m_queues.end()) {
        return false;
      }
      if (m_cleanupAccounts.find(accountId) != m_cleanupAccounts.end()) {
        m_routeCv.wait(lock, [this, accountId] {
          return m_stopping ||
                 m_cleanupAccounts.find(accountId) == m_cleanupAccounts.end();
        });
        return false;
      }
      if (m_activeSection) {
        // The release must not run beside the section. The section runs it
        // before it releases the lanes, so a lane created for this account in
        // the meantime starts with the barrier and runs its work after it.
        m_activeSection->postponed.push_back(
            PostponedCleanup{std::move(promise), cleanup});
        return true;
      }
      m_cleanupAccounts.insert(accountId);
    }
    ResolveFencedCleanup(accountId, std::move(promise), cleanup);
    return true;
  }

  [[nodiscard]] KeyQueuePtr GetOrCreate(OpenPitParamAccountId accountId,
                                        bool& created, std::size_t& total,
                                        std::optional<Error>& err) {
    std::unique_lock<std::mutex> lock(m_mutex);
    m_routeCv.wait(lock, [this, accountId] {
      return m_stopping ||
             m_cleanupAccounts.find(accountId) == m_cleanupAccounts.end();
    });
    if (m_stopping) {
      err = Error(ErrorCode::Stopped, "async engine is stopped");
      return nullptr;
    }
    auto it = m_queues.find(accountId);
    if (it != m_queues.end()) {
      return it->second;
    }
    if (m_capEnabled && m_queues.size() >= m_maxQueues) {
      err = Error(ErrorCode::QueueLimit,
                  "async dynamic per-account queue limit exceeded");
      return nullptr;
    }
    auto q = std::make_shared<KeyQueue>(queueCapacity());
    m_queues.emplace(accountId, q);
    bool barrier = false;
    try {
      // A lane created during an exclusive section starts stopped.
      if (m_activeSection) {
        barrier = BarrierNewLane(q, m_activeSection);
      }
      StartWorker(q);
    } catch (...) {
      if (barrier) {
        ForgetNewLaneBarrier(*m_activeSection);
      }
      m_queues.erase(accountId);
      throw;
    }
    created = true;
    total = m_queues.size();
    return q;
  }

  // Releases producers blocked on a full lane after a hard stop. A lane created
  // after this pass cannot strand a producer: a producer that reaches a full
  // lane later observes the hard stop before deciding to wait.
  void WakeBlockedLaneProducers() {
    std::lock_guard<std::mutex> lock(m_mutex);
    for (const auto& entry : m_queues) {
      WakeBlockedProducers(entry.second);
    }
  }

  // Closes and joins every live lane without allocating, for the destructor
  // path where a failed allocation could not be reported. Closing all lanes
  // before joining any keeps the teardown order of the stop methods: a worker
  // never waits for a lane that has not been closed yet.
  void CloseAndJoinLanes() {
    {
      std::lock_guard<std::mutex> lock(m_mutex);
      m_stopping = true;
      for (const auto& entry : m_queues) {
        CloseQueue(entry.second);
      }
    }
    m_routeCv.notify_all();
    while (true) {
      KeyQueuePtr q;
      OpenPitParamAccountId accountId;
      {
        std::lock_guard<std::mutex> lock(m_mutex);
        const auto it = m_queues.begin();
        if (it == m_queues.end()) {
          return;
        }
        accountId = it->first;
        q = it->second;
      }
      // Releases during join must still find this lane behind its work.
      JoinLane(q);
      {
        std::lock_guard<std::mutex> lock(m_mutex);
        m_queues.erase(accountId);
      }
    }
  }

  // Snapshots every live queue and sets `m_stopping` so `GetOrCreate` starts no
  // new worker after the snapshot. Retired queues are already joined inline by
  // the cleanup thread, so they need not appear here.
  [[nodiscard]] std::vector<KeyQueuePtr> MarkStoppingAndSnapshot() {
    std::lock_guard<std::mutex> lock(m_mutex);
    m_stopping = true;
    std::vector<KeyQueuePtr> queues;
    queues.reserve(m_queues.size());
    for (const auto& entry : m_queues) {
      queues.push_back(entry.second);
    }
    return queues;
  }

  void CleanupLoop() {
    std::unique_lock<std::mutex> lock(m_cleanupMutex);
    while (!m_cleanupStop) {
      if (m_cleanupCv.wait_for(lock, m_cleanupPeriod,
                               [this] { return m_cleanupStop; })) {
        return;
      }
      lock.unlock();
      CleanupIdle();
      lock.lock();
    }
  }

  void CleanupIdle() {
    if (IsStopped()) {
      return;
    }
    const auto cutoff = std::chrono::steady_clock::now() - m_idleCleanupAfter;
    std::vector<std::pair<OpenPitParamAccountId, KeyQueuePtr>> candidates;
    {
      std::lock_guard<std::mutex> lock(m_mutex);
      for (const auto& entry : m_queues) {
        const KeyQueuePtr& q = entry.second;
        if (q->pending.load(std::memory_order_relaxed) == 0 &&
            q->LastActiveAt() <= cutoff) {
          std::lock_guard<std::mutex> qlock(q->mutex);
          if (q->buffer.empty()) {
            candidates.emplace_back(entry.first, q);
          }
        }
      }
    }
    for (const auto& candidate : candidates) {
      std::size_t remaining = 0;
      if (RetireIfIdle(candidate.first, candidate.second, cutoff, remaining)) {
        Notify([&](Observer& observer) {
          observer.OnQueueRemoved(PublicAccountId(candidate.first), remaining);
        });
      }
    }
  }

  [[nodiscard]] bool RetireIfIdle(OpenPitParamAccountId accountId,
                                  const KeyQueuePtr& q,
                                  std::chrono::steady_clock::time_point cutoff,
                                  std::size_t& remaining) {
    {
      std::lock_guard<std::mutex> lock(m_mutex);
      auto it = m_queues.find(accountId);
      if (it == m_queues.end() || it->second != q) {
        return false;
      }
      {
        std::lock_guard<std::mutex> qlock(q->mutex);
        if (q->pending.load(std::memory_order_relaxed) != 0 ||
            !q->buffer.empty() || q->LastActiveAt() > cutoff) {
          return false;
        }
        // Retire: the worker observes `retired`, drains (already empty), and
        // exits.
        q->retired = true;
      }
      q->notEmpty.notify_all();
      q->notFull.notify_all();
      m_queues.erase(it);
      remaining = m_queues.size();
    }
    // The retired queue owns a still-joinable worker thread. A retired queue is
    // empty by construction, so its worker's wait predicate fires immediately
    // and the worker returns promptly; join it OUTSIDE m_mutex so a concurrent
    // submit is never blocked, and so its std::thread is destroyed cleanly with
    // no dead-thread accumulation. The `q` argument keeps the KeyQueue alive
    // until the join completes.
    q->Join();
    return true;
  }

  void StopCleanup() {
    if (!tracksIdle()) {
      return;
    }
    {
      std::lock_guard<std::mutex> lock(m_cleanupMutex);
      m_cleanupStop = true;
    }
    m_cleanupCv.notify_all();
    if (m_cleanup.joinable()) {
      m_cleanup.join();
    }
  }

  void Terminate() noexcept override {
    // RAII backstop: stop the cleanup thread first (it joins retired workers
    // inline), then hard-stop so every live worker exits and every blocked
    // producer is released, then close and join the lanes. No worker thread
    // outlives this object. A failed handle-release hand-off terminates.
    StopCleanup();
    SignalHardStop();
    SignalStop();
    WakeBlockedLaneProducers();
    (void)WaitLifecycleDrained(std::chrono::steady_clock::time_point::max());
    CloseAndJoinLanes();
    (void)SealLifecycleDrained(std::chrono::steady_clock::time_point::max());
  }

  bool m_stopping = false;
  std::unordered_map<OpenPitParamAccountId, KeyQueuePtr> m_queues;
  std::shared_ptr<ExclusiveSection> m_activeSection;
  std::size_t m_maxQueues;
  bool m_capEnabled;
  std::chrono::nanoseconds m_idleCleanupAfter;
  std::chrono::nanoseconds m_cleanupPeriod{kDefaultIdleCleanupPeriod};
  std::thread m_cleanup;
  std::mutex m_cleanupMutex;
  std::condition_variable m_cleanupCv;
  bool m_cleanupStop = false;
};

}  // namespace detail
}  // namespace openpit::asyncengine
