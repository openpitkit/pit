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

#include "openpit/account_id.hpp"
#include "openpit/asyncengine/future.hpp"
#include "openpit/asyncengine/observer.hpp"
#include "openpit/asyncengine/strategy.hpp"
#include "openpit/error.hpp"

#include <openpit.h>

#include <chrono>
#include <cstddef>
#include <cstdint>
#include <exception>
#include <functional>
#include <memory>
#include <mutex>
#include <optional>
#include <thread>
#include <type_traits>
#include <utility>
#include <variant>

// Concurrent facade over an AccountSync engine.
//
// `AsyncEngine` serializes account-scoped work for concurrent callers. It
// serializes every operation by account id behind a per-account dispatcher
// chosen at build time
// (`Sharded` or `Dynamic`) and returns a `Future` resolved once the worker has
// run the call. See `strategy.hpp` for the threading contract.
//
// generic over a `Driver`: the type that performs the actual engine calls. The
// async layer owns all of the concurrency (dispatch, worker threads, futures,
// graceful/hard stop); the driver owns the synchronous engine operations. This
// keeps the async surface free of any specific engine-method coupling, so it
// composes with whatever pre-trade pipeline wrapper the binding exposes and is
// trivially testable against a mock driver.
//
// ExecutePreTrade, ApplyExecutionReport, ApplyAccountAdjustment, the Accounts
// admin calls). Each is just "route this driver call through the account's
// queue and resolve a future with its result". Here that is expressed once,
// generically: `Call` for a single-value engine call, `Call2` for a two-value
// tuple call (request-or-rejects, reservation-or-rejects,
// batch-error-or-outcomes), and `Submit` for caller-owned work — each pinned
// to an account id. A typed binding can layer the named operations on top by
// forwarding to these (e.g.
// `StartPreTrade(order)` -> `Call2<Request, Rejects>(accountId, [order](auto&
// e) { return e.StartPreTrade(order); })`) once the synchronous pre-trade
// pipeline wrapper exists; the async machinery here does not change.
//
// The engine borrows the driver by reference; the driver must outlive the
// engine. Lifecycle ownership of the driver is the caller's: stopping the
// async facade does not stop the wrapped engine unless `WithStopUnderlying`
// was wired). Use `WithStopUnderlying` for the same atomic-release behavior.
//
// ERROR MODEL. Lifecycle/build failures throw `openpit::Error` (e.g. a
// non-positive shard count). Expected async outcomes — engine stopped, queue
// limit, submit deadline, a `Submit` closure that threw — are VALUES
// delivered through the future as `openpit::asyncengine::Error` and read at
// the boundary; they are never thrown across a worker thread in a way that
// loses them.

namespace openpit::asyncengine {

// A caller-supplied teardown callback invoked once after every worker exits on
// a successful stop. Used to release the wrapped engine atomically with the
using StopUnderlying = std::function<void()>;

namespace detail {

// Resolves a `Promise<void-like>`-style task. The engine builds one closure
// pair per operation: `run` performs the driver call and resolves the promise;
// `abort` fails the promise with the stop/limit error. Both are captured by
// value into a `ClosureTask`, keeping the strategy ignorant of the driver.
template <typename RunFn, typename AbortFn>
[[nodiscard]] TaskPtr MakeTask(RunFn run, AbortFn abort) {
  return std::make_unique<ClosureTask>(std::move(run), std::move(abort));
}

}  // namespace detail

// Builder stages (forward-declared so AsyncEngine can befriend the entry).
template <typename Driver>
class ShardedBuilder;
template <typename Driver>
class DynamicBuilder;

/// \brief Account-pinned async facade over a synchronous driver object.
//
// The concurrent engine facade. Construct via `Builder<Driver>(driver)` then a
// strategy stage. Move-only: it uniquely owns the dispatcher and its threads.
template <typename Driver>
class AsyncEngine {
 public:
  AsyncEngine(const AsyncEngine&) = delete;
  AsyncEngine& operator=(const AsyncEngine&) = delete;
  AsyncEngine(AsyncEngine&&) noexcept = default;
  AsyncEngine& operator=(AsyncEngine&&) noexcept = default;

  // RAII: the strategy's destructor hard-stops and joins every worker, so no
  // thread outlives the engine even if the caller never called a stop method.
  ~AsyncEngine() = default;

  // The borrowed driver. Exposed so a typed layer built on top of this generic
  // engine can invoke named driver methods inside a `Submit` closure that has
  // already been routed onto the correct per-account worker (see
  // `asyncengine/typed.hpp`). The driver is borrowed; ownership is unchanged.
  [[nodiscard]] Driver& DriverRef() const noexcept { return *m_driver; }

  //----------------------------------------------------------------------------
  // Caller-owned work

  // Enqueues an arbitrary closure into the queue for `accountId`, running it
  // atomically with respect to engine calls on the same account. The future
  // resolves when the closure returns; if the closure throws, the future is
  // failed with `ErrorCode::TaskFailed` carrying the exception message (an
  // exception never crosses the worker boundary). An aborted task (hard stop)
  // fails the future with `ErrorCode::Stopped` and never runs the closure.
  //
  // `timeout` bounds only how long the producer waits for queue space; once
  // queued the closure runs to completion. Omit it (or pass a non-positive
  // duration) to wait indefinitely.
  [[nodiscard]] Future<std::monostate> Submit(
      ::openpit::param::AccountId accountId, std::function<void()> fn,
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    Promise<std::monostate> promise;
    Future<std::monostate> future = promise.GetFuture();
    auto run = [promise, fn = std::move(fn)] {
      try {
        fn();
        promise.Resolve(std::monostate{});
      } catch (const std::exception& ex) {
        promise.Fail(Error(ErrorCode::TaskFailed, ex.what()));
      } catch (...) {
        promise.Fail(Error(ErrorCode::TaskFailed,
                           "async submit closure threw a non-standard "
                           "exception"));
      }
    };
    auto abort = [promise](Error error) { promise.Fail(std::move(error)); };
    SubmitTask(accountId.Raw(),
               detail::MakeTask(std::move(run), std::move(abort)), promise,
               timeout);
    return future;
  }

  // Enqueues `op(driver)` for `accountId`, returning a `Future<R>` over its
  // result. `op` is invoked on the worker thread with a reference to the
  // borrowed driver; its return type `R` is deduced. Use this to express any
  // single-value engine call without the async layer needing to name it.
  template <typename Op, typename R = std::invoke_result_t<Op, Driver&>>
  [[nodiscard]] Future<R> Call(
      ::openpit::param::AccountId accountId, Op op,
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    Promise<R> promise;
    Future<R> future = promise.GetFuture();
    Driver* driver = m_driver;
    auto run = [promise, driver, op = std::move(op)] {
      try {
        promise.Resolve(op(*driver));
      } catch (const std::exception& ex) {
        promise.Fail(Error(ErrorCode::TaskFailed, ex.what()));
      } catch (...) {
        promise.Fail(Error(ErrorCode::TaskFailed,
                           "async engine call threw a non-standard exception"));
      }
    };
    auto abort = [promise](Error error) { promise.Fail(std::move(error)); };
    SubmitTask(accountId.Raw(),
               detail::MakeTask(std::move(run), std::move(abort)), promise,
               timeout);
    return future;
  }

  // Two-value variant of `Call`: `op` returns a `std::pair<A, B>` mirroring the
  // synchronous engine tuples (request-or-rejects, reservation-or-rejects).
  // Returns a `PairFuture<A, B>`.
  template <typename A, typename B, typename Op>
  [[nodiscard]] PairFuture<A, B> Call2(
      ::openpit::param::AccountId accountId, Op op,
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    PairPromise<A, B> promise;
    PairFuture<A, B> future = promise.GetFuture();
    Driver* driver = m_driver;
    auto run = [promise, driver, op = std::move(op)] {
      try {
        std::pair<A, B> result = op(*driver);
        promise.Resolve(std::move(result.first), std::move(result.second));
      } catch (const std::exception& ex) {
        promise.Fail(Error(ErrorCode::TaskFailed, ex.what()));
      } catch (...) {
        promise.Fail(Error(ErrorCode::TaskFailed,
                           "async engine call threw a non-standard exception"));
      }
    };
    auto abort = [promise](Error error) { promise.Fail(std::move(error)); };
    std::optional<Error> err = m_strategy->Submit(
        accountId.Raw(), detail::MakeTask(std::move(run), std::move(abort)),
        Deadline(timeout));
    if (err.has_value()) {
      // Submit failed before queueing: fail the future on the caller's thread,
      promise.Fail(std::move(*err));
    }
    return future;
  }

  //----------------------------------------------------------------------------
  // Lifecycle

  // Refuses new submits and waits for every queued task to run to completion.
  // Returns true on a clean drain; false if `timeout` elapses first (the engine
  // is then partially stopped and `StopHard` may complete the shutdown). On a
  // clean stop the `StopUnderlying` callback (if wired) fires exactly once.
  [[nodiscard]] bool StopGraceful(
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    const bool ok = m_strategy->StopGraceful(Deadline(timeout));
    if (ok) {
      ReleaseUnderlying();
    }
    return ok;
  }

  // Refuses new submits, aborts every not-yet-started task with
  // `ErrorCode::Stopped`, and waits for the in-flight task per worker to
  // finish. Returns false if `timeout` elapses first. On a clean stop the
  // `StopUnderlying` callback (if wired) fires exactly once.
  [[nodiscard]] bool StopHard(
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    const bool ok = m_strategy->StopHard(Deadline(timeout));
    if (ok) {
      ReleaseUnderlying();
    }
    return ok;
  }

 private:
  template <typename D>
  friend class ShardedBuilder;
  template <typename D>
  friend class DynamicBuilder;

  AsyncEngine(Driver& driver, std::unique_ptr<detail::Strategy> strategy,
              StopUnderlying stopUnderlying)
      : m_driver(&driver),
        m_strategy(std::move(strategy)),
        m_stopUnderlying(std::move(stopUnderlying)),
        m_stopOnce(std::make_unique<std::once_flag>()) {}

  static std::chrono::steady_clock::time_point Deadline(
      std::chrono::nanoseconds timeout) {
    if (timeout <= std::chrono::nanoseconds(0)) {
      return std::chrono::steady_clock::time_point::max();
    }
    return std::chrono::steady_clock::now() + timeout;
  }

  // Submits a single-value task and, if the synchronous submit failed (the task
  // where a submit failure resolves the future synchronously on the submitter.
  template <typename T>
  void SubmitTask(OpenPitParamAccountId accountId, detail::TaskPtr task,
                  const Promise<T>& promise, std::chrono::nanoseconds timeout) {
    std::optional<Error> err =
        m_strategy->Submit(accountId, std::move(task), Deadline(timeout));
    if (err.has_value()) {
      promise.Fail(std::move(*err));
    }
  }

  void ReleaseUnderlying() {
    if (m_stopUnderlying) {
      std::call_once(*m_stopOnce, [this] { m_stopUnderlying(); });
    }
  }

  Driver* m_driver;
  std::unique_ptr<detail::Strategy> m_strategy;
  StopUnderlying m_stopUnderlying;
  std::unique_ptr<std::once_flag> m_stopOnce;
};

//------------------------------------------------------------------------------
// Builder

/// \brief Entry builder for selecting async strategy and common options.
//
// Entry point of the builder chain. Construct with the driver, optionally wire
// an observer / capacities / a teardown callback, then advance to a strategy
// stage via `Sharded` or `Dynamic`.
//
// The engine borrows the driver by reference; it must outlive the engine.
template <typename Driver>
class Builder {
 public:
  explicit Builder(Driver& driver) : m_driver(&driver) {}

  // Installs a teardown callback invoked after every worker exits on a clean
  // stop, at most once. Use it to release the wrapped engine atomically with
  // the async stop. Left unset, the caller owns the engine lifecycle and tears
  // it down after the async engine has stopped.
  Builder& WithStopUnderlying(StopUnderlying stop) {
    m_stopUnderlying = std::move(stop);
    return *this;
  }

  // Wires diagnostic callbacks. The observer must outlive the engine. Default
  // is a no-op; the observer applies to every queue regardless of strategy.
  Builder& WithObserver(Observer& observer) {
    m_observer = &observer;
    return *this;
  }

  // Sets the buffered capacity of each per-account or per-shard queue. Zero or
  // negative resets to the default (1024).
  Builder& WithQueueCapacity(std::size_t capacity) {
    m_queueCapacity = capacity;
    return *this;
  }

  // Controls how long a producer blocks on a full queue before the observer is
  // notified that the queue is slow. Zero or negative resets to the default
  // (1 minute).
  Builder& WithSlowSubmitThreshold(std::chrono::nanoseconds threshold) {
    m_slowSubmitThreshold = threshold;
    return *this;
  }

  // Selects the fixed N-shard strategy: cheapest hot path, O(1) memory, no
  // per-account observability; a hot account saturates one shard. Choose it for
  // a broad, roughly balanced account set.
  [[nodiscard]] ShardedBuilder<Driver> Sharded(std::size_t workers) {
    return ShardedBuilder<Driver>(*this, workers);
  }

  // Selects the lazy per-account strategy with idle cleanup: full per-account
  // isolation and per-account observer signals at the cost of a map lookup per
  // submit and a cleanup thread. Choose it for skewed activity or per-account
  // metrics.
  [[nodiscard]] DynamicBuilder<Driver> Dynamic() {
    return DynamicBuilder<Driver>(*this);
  }

 private:
  template <typename D>
  friend class ShardedBuilder;
  template <typename D>
  friend class DynamicBuilder;

  [[nodiscard]] detail::BaseConfig MakeBaseConfig() const {
    detail::BaseConfig cfg;
    cfg.observer = m_observer;
    cfg.queueCapacity = m_queueCapacity;
    cfg.slowSubmitThreshold = m_slowSubmitThreshold;
    return cfg;
  }

  Driver* m_driver;
  StopUnderlying m_stopUnderlying;
  Observer* m_observer = nullptr;
  std::size_t m_queueCapacity = 0;
  std::chrono::nanoseconds m_slowSubmitThreshold{0};
};

/// \brief Builder stage for a fixed number of account shards.
//
// Second stage after `Sharded`. `Build()` constructs the engine.
template <typename Driver>
class ShardedBuilder {
 public:
  // Constructs the async engine with a fixed shard pool. Throws
  // `openpit::Error` if the shard count is zero.
  [[nodiscard]] AsyncEngine<Driver> Build() {
    if (m_workers == 0) {
      throw ::openpit::Error(
          "openpit::asyncengine: sharded workers must be > 0");
    }
    auto strategy = std::make_unique<detail::ShardedStrategy>(
        m_parent->MakeBaseConfig(), m_workers);
    return AsyncEngine<Driver>(*m_parent->m_driver, std::move(strategy),
                               m_parent->m_stopUnderlying);
  }

 private:
  friend class Builder<Driver>;

  ShardedBuilder(Builder<Driver>& parent, std::size_t workers)
      : m_parent(&parent), m_workers(workers) {}

  Builder<Driver>* m_parent;
  std::size_t m_workers;
};

/// \brief Builder stage for demand-created per-account queues.
//
// Second stage after `Dynamic`. Configure `MaxQueues` / `IdleCleanupAfter`,
// then `Build()`.
template <typename Driver>
class DynamicBuilder {
 public:
  // Caps the number of concurrent live per-account queues. Zero removes the cap
  // (submit never fails for new accounts). When the cap is reached, submitting
  // for an unknown account fails the future with `ErrorCode::QueueLimit`. The
  // default cap is `hardware_concurrency() * 32` (a non-restrictive bound that
  // still guards against pathological growth).
  DynamicBuilder& MaxQueues(std::size_t maxQueues) {
    m_maxQueues = maxQueues;
    m_maxQueuesSet = true;
    return *this;
  }

  // Sets the idle duration after which an empty, untouched queue is retired.
  // Zero disables cleanup; queues then live until stop. Default is 5 minutes.
  DynamicBuilder& IdleCleanupAfter(std::chrono::nanoseconds idle) {
    m_idleCleanupAfter = idle;
    return *this;
  }

  // Constructs the async engine with on-demand per-account queues and idle
  // retirement.
  [[nodiscard]] AsyncEngine<Driver> Build() {
    const std::size_t cap = m_maxQueuesSet ? m_maxQueues : DefaultMaxQueues();
    const bool capEnabled = cap > 0;
    auto idle = m_idleCleanupAfter < std::chrono::nanoseconds(0)
                    ? std::chrono::nanoseconds(0)
                    : m_idleCleanupAfter;
    auto strategy = std::make_unique<detail::DynamicStrategy>(
        m_parent->MakeBaseConfig(), cap, idle, capEnabled);
    return AsyncEngine<Driver>(*m_parent->m_driver, std::move(strategy),
                               m_parent->m_stopUnderlying);
  }

 private:
  friend class Builder<Driver>;

  explicit DynamicBuilder(Builder<Driver>& parent) : m_parent(&parent) {}

  [[nodiscard]] static std::size_t DefaultMaxQueues() {
    const unsigned hw = std::thread::hardware_concurrency();
    const std::size_t cores = hw == 0 ? 1 : hw;
    return cores * 32;
  }

  Builder<Driver>* m_parent;
  std::size_t m_maxQueues = 0;
  bool m_maxQueuesSet = false;
  std::chrono::nanoseconds m_idleCleanupAfter = kDefaultIdleCleanupAfter;
};

}  // namespace openpit::asyncengine
