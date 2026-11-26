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

#include <openpit.h>

#include <atomic>
#include <chrono>
#include <cstdint>
#include <exception>
#include <future>
#include <memory>
#include <optional>
#include <string>
#include <type_traits>
#include <utility>
#include <variant>

// Result delivery for the async engine.
//
// Typed future primitives for async results.
// Every queued operation hands the caller a `Future` resolved exactly once:
// from the worker thread in the normal case, or synchronously on the
// submitting thread if the submit fails before the task is queued.
//
// `Result<ResultValue>` carries a `ResultValue` on success or a status-coded
// `openpit::asyncengine::Error` on failure. Expected async
// outcomes (engine stopped, queue limit, submit cancelled, a `Submit` closure
// that threw) are encoded here and read at the boundary, never thrown across
// threads - losing them on a worker thread is not acceptable. `Await`
// rethrows the carried error on the *caller's* thread so the
// synchronous-looking call site can use ordinary try/catch.
//
// The future is intentionally a thin adapter over `std::shared_future`: a
// promise resolves the underlying state, and consumer accessors read it.
// `std::shared_future` is used deliberately - the producer (worker thread)
// and one or more consumers share ownership of the resolution state with no
// defined outlives-the-other ordering, which is exactly
// std::shared_future's contract.
// Copyable `T` may be observed by multiple consumers. Move-only `T` is
// supported for a single consuming `Await()`.
//
// `Payload<ResultValue>` stores a copyable result inline so the hot resolve
// path allocates nothing extra. A move-only result is uniquely boxed so a
// single consumer can move the value out of the shared state.

namespace openpit::asyncengine {

template <typename ResultValue>
class Future;

// Machine-readable category of an async-dispatch failure carried by a future
// or thrown by a lifecycle call. These are dispatcher conditions, not SDK
// parameter failures.
enum class ErrorCode : std::uint8_t {
  // A submit method was called after the engine was stopped, or the task was
  Stopped,
  // A Dynamic strategy with a positive queue cap rejected a new account
  QueueLimit,
  MissingAccountId,
  // `ctx` cancellation on the submit path, or an `Exclusive` timeout that
  // expired before the section's callback started.
  SubmitCancelled,
  // A caller-supplied `Submit` closure threw. The closure exception never
  // crosses a thread boundary; its `what()` is captured here instead.
  TaskFailed,
  // A move-only future value was already returned to another consumer.
  ValueConsumed,
  // `Exclusive` was called from a lane worker of the same engine, or from
  // inside one of its exclusive sections, where it could never stop every
  // lane.
  ReentrantLane,
};

namespace detail {

template <typename ResultValue,
          bool = std::is_copy_constructible_v<ResultValue>>
class ResultConsumption {
 protected:
  [[nodiscard]] bool TryConsumeValue() const noexcept { return true; }
};

template <typename ResultValue>
class ResultConsumption<ResultValue, false> {
 protected:
  ResultConsumption() = default;

  ResultConsumption(ResultConsumption&& other) noexcept {
    m_consumed.store(other.m_consumed.exchange(true, std::memory_order_relaxed),
                     std::memory_order_relaxed);
  }

  ResultConsumption& operator=(ResultConsumption&& other) noexcept {
    if (this != &other) {
      m_consumed.store(
          other.m_consumed.exchange(true, std::memory_order_relaxed),
          std::memory_order_relaxed);
    }
    return *this;
  }

  [[nodiscard]] bool TryConsumeValue() const noexcept {
    return !m_consumed.exchange(true, std::memory_order_acq_rel);
  }

 private:
  mutable std::atomic_bool m_consumed{false};
};

}  // namespace detail

/// \brief Async dispatch failure carried by a future or lifecycle API.
//
// Error carried by a resolved-with-failure future, or thrown by a lifecycle
// call. Derives from `std::exception` so `Future::Await` can rethrow it on the
// caller's thread for ordinary try/catch handling.
class Error : public std::exception {
 public:
  Error(ErrorCode code, std::string message)
      : m_code(code), m_message(std::move(message)) {}

  [[nodiscard]] const char* what() const noexcept override {
    return m_message.c_str();
  }

  [[nodiscard]] ErrorCode Code() const noexcept { return m_code; }

  [[nodiscard]] const std::string& Message() const noexcept {
    return m_message;
  }

 private:
  ErrorCode m_code;
  std::string m_message;
};

/// \brief Resolved payload of a future: either a value or an async error.
//
// The resolved payload of a single-value future: either the value or the
// failure. The consuming `Future::Await()` supports move-only values.
// `Get()` requires a copyable `ResultValue`.
template <typename ResultValue>
class Result : private detail::ResultConsumption<ResultValue> {
 public:
  // NOLINTNEXTLINE: implicit by design.
  Result(ResultValue value) : m_value(std::move(value)) {}
  Result(Error error) : m_value(std::move(error)) {}  // NOLINT: implicit.
  Result(const Result&) = default;
  Result(Result&&) noexcept(
      std::is_nothrow_move_constructible_v<std::variant<ResultValue, Error>>) =
      default;
  Result& operator=(const Result&) = default;
  Result& operator=(Result&&) noexcept(
      std::is_nothrow_move_assignable_v<std::variant<ResultValue, Error>>) =
      default;

  [[nodiscard]] bool HasValue() const noexcept {
    return std::holds_alternative<ResultValue>(m_value);
  }
  [[nodiscard]] bool HasError() const noexcept {
    return std::holds_alternative<Error>(m_value);
  }

  // The value; defined only when `HasValue()`.
  [[nodiscard]] const ResultValue& Value() const& {
    return std::get<ResultValue>(m_value);
  }
  [[nodiscard]] ResultValue&& Value() && {
    return std::get<ResultValue>(std::move(m_value));
  }

  // The error; defined only when `HasError()`.
  [[nodiscard]] const Error& GetError() const {
    return std::get<Error>(m_value);
  }

 private:
  friend class Future<ResultValue>;

  [[nodiscard]] bool TryConsumeValue() const noexcept {
    return detail::ResultConsumption<ResultValue>::TryConsumeValue();
  }

  std::variant<ResultValue, Error> m_value;
};

// A copyable result is stored inline without a per-resolve allocation.
// A move-only result is boxed in a `std::unique_ptr` so the single
// consuming `Await()` can move it out of the shared state.
template <typename ResultValue>
using Payload = std::conditional_t<std::is_copy_constructible_v<ResultValue>,
                                   Result<ResultValue>,
                                   std::unique_ptr<Result<ResultValue>>>;

/// \brief Consumer side of an async operation returning one value.
//
// Resolved exactly once via the paired `Promise<ResultValue>`. Multiple
// threads can observe a copyable result concurrently. A move-only value can
// be consumed once; later `Await()` calls throw `ErrorCode::ValueConsumed`.
template <typename ResultValue>
class Future {
 public:
  Future() = default;

  explicit Future(std::shared_future<Payload<ResultValue>> state)
      : m_state(std::move(state)) {}

  // Blocks until resolved, then returns the value or rethrows the carried
  // `Error` on the caller's thread.
  [[nodiscard]] ResultValue Await() const { return ConsumeResolved(); }

  // Blocks up to `timeout`. Returns the value on resolution, rethrows the
  // carried `Error`, or returns `std::nullopt` if the deadline passes first
  // (the underlying task is unaffected; the caller merely stops waiting).
  template <typename Representation, typename Period>
  [[nodiscard]] std::optional<ResultValue> Await(
      std::chrono::duration<Representation, Period> timeout) const {
    if (m_state.wait_for(timeout) != std::future_status::ready) {
      return std::nullopt;
    }
    return ConsumeResolved();
  }

  [[nodiscard]] bool Done() const {
    return m_state.wait_for(std::chrono::seconds(0)) ==
           std::future_status::ready;
  }

  // Blocks for the resolution without consuming it, then returns the carried
  // `Result<ResultValue>` by copy. Requires a copyable result value.
  [[nodiscard]] Result<ResultValue> Get() const {
    if constexpr (std::is_copy_constructible_v<ResultValue>) {
      return m_state.get();
    } else {
      static_assert(std::is_copy_constructible_v<ResultValue>,
                    "Future<ResultValue>::Get() requires a copy-constructible "
                    "result value; use Await() for a move-only result value");
    }
  }

 private:
  [[nodiscard]] ResultValue ConsumeResolved() const {
    if constexpr (std::is_copy_constructible_v<ResultValue>) {
      const Result<ResultValue>& result = m_state.get();
      if (result.HasError()) {
        throw result.GetError();
      }
      return result.Value();
    } else {
      const std::unique_ptr<Result<ResultValue>>& result = m_state.get();
      if (result->HasError()) {
        throw result->GetError();
      }
      if (!result->TryConsumeValue()) {
        throw Error(ErrorCode::ValueConsumed,
                    "async future value was already consumed");
      }
      return std::move(*result).Value();
    }
  }
  std::shared_future<Payload<ResultValue>> m_state;
};

/// \brief Producer side used by the dispatcher to resolve a
/// `Future<ResultValue>`.
//
// The dispatcher creates one promise per task and resolves it exactly
// once. A second resolve is ignored.
template <typename ResultValue>
class Promise {
 public:
  Promise()
      : m_state(std::make_shared<State>()),
        m_future(m_state->promise.get_future().share()) {}

  [[nodiscard]] Future<ResultValue> GetFuture() const {
    return Future<ResultValue>(m_future);
  }

  // Resolves with a value. Idempotent: only the first resolve takes effect.
  void Resolve(ResultValue value) const {
    ResolveResult(Result<ResultValue>(std::move(value)));
  }

  // Resolves with a failure. Idempotent.
  void Fail(Error error) const {
    ResolveResult(Result<ResultValue>(std::move(error)));
  }

 private:
  // Past the latch the result can no longer be reported any other way, so a
  // delivery failure terminates instead of leaving the future unresolved.
  void ResolveResult(Result<ResultValue> result) const noexcept {
    if (m_state->done.exchange(true)) {
      return;
    }
    if constexpr (std::is_copy_constructible_v<ResultValue>) {
      m_state->promise.set_value(std::move(result));
    } else {
      m_state->promise.set_value(
          std::make_unique<Result<ResultValue>>(std::move(result)));
    }
  }

  struct State {
    std::promise<Payload<ResultValue>> promise;
    // Guards single resolution: std::promise::set_value throws on a second
    // call, and a task's run/abort halves could otherwise both fire under a
    // stop race.
    std::atomic_bool done{false};
  };

  std::shared_ptr<State> m_state;
  std::shared_future<Payload<ResultValue>> m_future;
};

/// \brief Consumer side of an async operation returning two values.
//
// Future over a pair of values. Used by operations whose synchronous
// counterpart returns two values plus an error, so the async shape lines up
// with the sync tuple instead of a result struct.
template <typename FirstValue, typename SecondValue>
class PairFuture {
 public:
  PairFuture() = default;

  explicit PairFuture(Future<std::pair<FirstValue, SecondValue>> inner)
      : m_inner(std::move(inner)) {}

  // Blocks until resolved, then returns both values or rethrows the error.
  [[nodiscard]] std::pair<FirstValue, SecondValue> Await() const {
    return m_inner.Await();
  }

  template <typename Representation, typename Period>
  [[nodiscard]] std::optional<std::pair<FirstValue, SecondValue>> Await(
      std::chrono::duration<Representation, Period> timeout) const {
    return m_inner.Await(timeout);
  }

  [[nodiscard]] bool Done() const { return m_inner.Done(); }

 private:
  Future<std::pair<FirstValue, SecondValue>> m_inner;
};

/// \brief Producer side for `PairFuture<FirstValue, SecondValue>`.
//
// Producer side of a `PairFuture<FirstValue, SecondValue>`; a thin adapter over
// `Promise<pair>`.
template <typename FirstValue, typename SecondValue>
class PairPromise {
 public:
  [[nodiscard]] PairFuture<FirstValue, SecondValue> GetFuture() const {
    return PairFuture<FirstValue, SecondValue>(m_inner.GetFuture());
  }

  void Resolve(FirstValue first, SecondValue second) const {
    m_inner.Resolve(std::make_pair(std::move(first), std::move(second)));
  }

  void Fail(Error error) const { m_inner.Fail(std::move(error)); }

 private:
  Promise<std::pair<FirstValue, SecondValue>> m_inner;
};

}  // namespace openpit::asyncengine
