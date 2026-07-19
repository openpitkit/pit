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

// Result delivery for the async engine.
//
// Typed future primitives for async results.
// Every queued operation hands the caller a `Future` resolved exactly once:
// from the worker thread in the normal case, or synchronously on the
// submitting thread if the submit fails before the task is queued.
//
// `Result<T>` is the value the future carries: a `T` on success or an
// `openpit::asyncengine::Error` (status-coded) instead. Expected async
// outcomes (engine stopped, queue limit, submit cancelled, a `Submit` closure
// that threw) are encoded here and read at the boundary, never thrown across
// threads — losing them on a worker thread is not acceptable. `Await`
// rethrows the carried error on the *caller's* thread so the
// synchronous-looking call site can use ordinary try/catch.
//
// The future is intentionally a thin adapter over `std::shared_future`: a
// promise resolves the underlying state, and consumer accessors read it.
// `std::shared_future` is used deliberately — the producer (worker thread)
// and one or more consumers share ownership of the resolution state with no
// defined outlives-the-other ordering, which is exactly
// std::shared_future's contract.
// Copyable `T` may be observed by multiple consumers. Move-only `T` is
// supported for a single consuming `Await()`.
//
// The shared-future payload is conditional on `T` (see `Payload<T>`): a
// copyable `T` stores the `Result<T>` inline so the hot resolve path allocates
// nothing extra, while a move-only `T` is boxed in a `std::shared_ptr` so the
// single consuming `Await()` can move the value out of the shared state.

namespace openpit::asyncengine {

// Machine-readable category of an async-dispatch failure carried by a future
// or thrown by a lifecycle call. Distinct from `OpenPitParamErrorCode`: these
// are dispatcher conditions, not native runtime param failures.
enum class ErrorCode : std::uint8_t {
  // A submit method was called after the engine was stopped, or the task was
  Stopped,
  // A Dynamic strategy with a positive queue cap rejected a new account
  QueueLimit,
  MissingAccountId,
  // `ctx` cancellation on the submit path.
  SubmitCancelled,
  // A caller-supplied `Submit` closure threw. The closure exception never
  // crosses a thread boundary; its `what()` is captured here instead.
  TaskFailed,
};

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
// failure. A move-only `T` is supported by the consuming `Future::Await()`;
// `Get()` is available only when `T` is copyable.
template <typename T>
class Result {
 public:
  Result(T value) : m_value(std::move(value)) {}  // NOLINT: implicit by design.
  Result(Error error) : m_error(std::move(error)) {}  // NOLINT: implicit.

  [[nodiscard]] bool HasValue() const noexcept { return m_value.has_value(); }
  [[nodiscard]] bool HasError() const noexcept { return m_error.has_value(); }

  // The value; defined only when `HasValue()`.
  [[nodiscard]] const T& Value() const& { return *m_value; }
  [[nodiscard]] T&& Value() && { return std::move(*m_value); }

  // The error; defined only when `HasError()`.
  [[nodiscard]] const Error& GetError() const { return *m_error; }

 private:
  std::optional<T> m_value;
  std::optional<Error> m_error;
};

// Shared-state payload carried by the future. A copyable `T` stores the
// `Result<T>` inline (no per-resolve allocation); a move-only `T` is boxed in a
// `std::shared_ptr` so the single consuming `Await()` can move out of the
// shared state.
template <typename T>
using Payload = std::conditional_t<std::is_copy_constructible_v<T>, Result<T>,
                                   std::shared_ptr<Result<T>>>;

/// \brief Consumer side of an async operation returning one value.
//
// Future over a single value `T`. Resolved exactly once via the paired
// `Promise<T>`. Safe for concurrent observation by multiple threads when `T`
// is copyable; move-only `T` has a single consuming `Await()` contract.
template <typename T>
class Future {
 public:
  Future() = default;

  explicit Future(std::shared_future<Payload<T>> state)
      : m_state(std::move(state)) {}

  // Blocks until resolved, then returns the value or rethrows the carried
  // `Error` on the caller's thread.
  [[nodiscard]] T Await() const {
    if constexpr (std::is_copy_constructible_v<T>) {
      const Result<T>& result = m_state.get();
      if (result.HasError()) {
        throw result.GetError();
      }
      return result.Value();
    } else {
      const std::shared_ptr<Result<T>>& result = m_state.get();
      if (result->HasError()) {
        throw result->GetError();
      }
      return std::move(*result).Value();
    }
  }

  // Blocks up to `timeout`. Returns the value on resolution, rethrows the
  // carried `Error`, or returns `std::nullopt` if the deadline passes first
  // (the underlying task is unaffected; the caller merely stops waiting).
  template <typename Rep, typename Period>
  [[nodiscard]] std::optional<T> Await(
      std::chrono::duration<Rep, Period> timeout) const {
    if (m_state.wait_for(timeout) != std::future_status::ready) {
      return std::nullopt;
    }
    if constexpr (std::is_copy_constructible_v<T>) {
      const Result<T>& result = m_state.get();
      if (result.HasError()) {
        throw result.GetError();
      }
      return result.Value();
    } else {
      const std::shared_ptr<Result<T>>& result = m_state.get();
      if (result->HasError()) {
        throw result->GetError();
      }
      return std::move(*result).Value();
    }
  }

  [[nodiscard]] bool Done() const {
    return m_state.wait_for(std::chrono::seconds(0)) ==
           std::future_status::ready;
  }

  // Blocks for the resolution without consuming it, then returns the carried
  // `Result<T>` by copy. Requires a copyable `T`.
  [[nodiscard]] Result<T> Get() const { return m_state.get(); }

 private:
  std::shared_future<Payload<T>> m_state;
};

/// \brief Producer side used by the dispatcher to resolve a `Future<T>`.
//
// Producer side of a `Future<T>`. The dispatcher creates one per task and
// resolves it exactly once; a second resolve is ignored.
template <typename T>
class Promise {
 public:
  Promise()
      : m_state(std::make_shared<State>()),
        m_future(m_state->promise.get_future().share()) {}

  [[nodiscard]] Future<T> GetFuture() const { return Future<T>(m_future); }

  // Resolves with a value. Idempotent: only the first resolve takes effect.
  void Resolve(T value) const { ResolveResult(Result<T>(std::move(value))); }

  // Resolves with a failure. Idempotent.
  void Fail(Error error) const { ResolveResult(Result<T>(std::move(error))); }

 private:
  void ResolveResult(Result<T> result) const {
    if (m_state->done.exchange(true)) {
      return;
    }
    if constexpr (std::is_copy_constructible_v<T>) {
      m_state->promise.set_value(std::move(result));
    } else {
      m_state->promise.set_value(
          std::make_shared<Result<T>>(std::move(result)));
    }
  }

  struct State {
    std::promise<Payload<T>> promise;
    // Guards single resolution: std::promise::set_value throws on a second
    // call, and a task's run/abort halves could otherwise both fire under a
    // stop race.
    std::atomic_bool done{false};
  };

  std::shared_ptr<State> m_state;
  std::shared_future<Payload<T>> m_future;
};

/// \brief Consumer side of an async operation returning two values.
//
// Future over a pair of values. Used by operations whose synchronous
// counterpart returns two values plus an error, so the async shape lines up
// with the sync tuple instead of a result struct.
template <typename A, typename B>
class PairFuture {
 public:
  PairFuture() = default;

  explicit PairFuture(Future<std::pair<A, B>> inner)
      : m_inner(std::move(inner)) {}

  // Blocks until resolved, then returns both values or rethrows the error.
  [[nodiscard]] std::pair<A, B> Await() const { return m_inner.Await(); }

  template <typename Rep, typename Period>
  [[nodiscard]] std::optional<std::pair<A, B>> Await(
      std::chrono::duration<Rep, Period> timeout) const {
    return m_inner.Await(timeout);
  }

  [[nodiscard]] bool Done() const { return m_inner.Done(); }

 private:
  Future<std::pair<A, B>> m_inner;
};

/// \brief Producer side for `PairFuture<A, B>`.
//
// Producer side of a `PairFuture<A, B>`; a thin adapter over `Promise<pair>`.
template <typename A, typename B>
class PairPromise {
 public:
  [[nodiscard]] PairFuture<A, B> GetFuture() const {
    return PairFuture<A, B>(m_inner.GetFuture());
  }

  void Resolve(A first, B second) const {
    m_inner.Resolve(std::make_pair(std::move(first), std::move(second)));
  }

  void Fail(Error error) const { m_inner.Fail(std::move(error)); }

 private:
  Promise<std::pair<A, B>> m_inner;
};

}  // namespace openpit::asyncengine
