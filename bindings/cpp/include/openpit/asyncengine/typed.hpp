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
#include "openpit/accountadjustment/account_adjustment.hpp"
#include "openpit/accounts.hpp"
#include "openpit/asyncengine/engine.hpp"
#include "openpit/asyncengine/future.hpp"
#include "openpit/engine.hpp"
#include "openpit/model.hpp"
#include "openpit/reject.hpp"

#include <openpit.h>

#include <chrono>
#include <cstddef>
#include <functional>
#include <memory>
#include <optional>
#include <string>
#include <utility>
#include <variant>
#include <vector>

// Concrete typed async surface over `openpit::Engine`.
//
// `async_engine.hpp` provides the generic `AsyncEngine<Driver>` plus its
// `Call`/`Call2`/`Submit` driver seam. This header layers the named operations
// on top — `StartPreTrade`, `ExecutePreTrade`, `ApplyExecutionReport`,
// `ApplyAccountAdjustment`, and account administration — mapping the
// synchronous engine surface one-to-one. The generic seam is left untouched;
// this is purely additive.
//
// DRIVER. `EngineAdapter` adapts a borrowed `openpit::Engine&` to the driver
// seam and is the default driver of `TypedAsyncEngine`; a test or interposing
// layer can supply any type exposing the same members.
//
// ACCOUNT PINNING. Every named method routes through the per-account queue
// keyed by the order/report account id (read without mutating the caller's
// payload), or by the supplied id for adjustments and admin ops.
// `StartPreTrade` and `ExecutePreTrade` yield an `AsyncRequest` /
// `AsyncReservation` whose follow-up calls (`Execute`, `Commit`, `Rollback`,
// `Close`, ...) re-enter the same per-account queue, preserving the AccountSync
// invariant across the start->execute->finalize boundary.
//
// RESULT SHAPES. The futures carry the same values the synchronous engine
// returns. Because `pretrade::Request`, `pretrade::Reservation`, and
// `accountadjustment::BatchError` are move-only RAII handles while the
// follow-up wrappers are observed from a shared state, the accepted-channel
// value is held by `shared_ptr`: the worker that resolves the future and the
// consumer that drives the follow-up calls genuinely share ownership of one
// wrapper, so `shared_ptr` is the correct model rather than a copy or a raw
// move.
//
// ERROR MODEL. A missing account id resolves the future with the value
// `ErrorCode::MissingAccountId`. Stop/limit/cancel are the same value-typed
// dispatch errors the generic layer uses. Pre-trade rejects and adjustment
// batch rejects are values in the accepted-or-rejected tuple, not errors. A
// runtime failure inside a driver call surfaces as `ErrorCode::TaskFailed`
// carrying the thrown message. An exception from the engine never crosses the
// worker boundary.

namespace openpit::asyncengine {

//------------------------------------------------------------------------------
// Driver

/// \brief Adapter that exposes `openpit::Engine` methods to `TypedAsyncEngine`.
//
// Adapts a borrowed `openpit::Engine&` to the async driver seam. Non-owning:
// the engine must outlive every async engine built over this driver. Copyable
// and cheap (one pointer); the typed layer borrows it by reference like the
// generic `AsyncEngine`. Each member is just the corresponding synchronous
// engine call; the async layer supplies all concurrency.
class EngineAdapter {
 public:
  explicit EngineAdapter(const ::openpit::Engine& engine) noexcept
      : m_engine(&engine) {}

  [[nodiscard]] ::openpit::pretrade::StartResult StartPreTrade(
      const ::openpit::model::Order& order) const {
    return m_engine->StartPreTrade(order);
  }

  [[nodiscard]] ::openpit::pretrade::ExecuteResult ExecutePreTrade(
      const ::openpit::model::Order& order) const {
    return m_engine->ExecutePreTrade(order);
  }

  [[nodiscard]] ::openpit::pretrade::Reservation ExecutePreTradeDropCopy(
      const ::openpit::model::Order& order) const {
    return m_engine->ExecutePreTradeDropCopy(order);
  }

  [[nodiscard]] ::openpit::PostTradeResult ApplyExecutionReport(
      const ::openpit::model::ExecutionReport& report) const {
    return m_engine->ApplyExecutionReport(report);
  }

  [[nodiscard]] ::openpit::PostTradeResult ApplyExecutionReport(
      const ::openpit::model::ExecutionReport& report,
      const ::openpit::pretrade::PreTradeLock& lock) const {
    return m_engine->ApplyExecutionReport(report, lock);
  }

  // Applies a batch adjustment, returning the (batch-reject-or-none, outcomes)
  template <typename Adjustment>
  [[nodiscard]] ::openpit::AdjustmentResult ApplyAccountAdjustment(
      ::openpit::param::AccountId accountId,
      const std::vector<Adjustment>& adjustments) const {
    return m_engine->ApplyAccountAdjustment(accountId, adjustments);
  }

  [[nodiscard]] ::openpit::accounts::Accounts Accounts() const noexcept {
    return m_engine->Accounts();
  }

 private:
  const ::openpit::Engine* m_engine;
};

//------------------------------------------------------------------------------
// Result aliases

// Forward declarations: the wrappers re-enter the engine that produced them.
template <typename Driver>
class TypedAsyncEngine;
template <typename Driver>
class AsyncRequest;
template <typename Driver>
class AsyncReservation;

/// \brief Result value for an async start-stage call.
//
// Accepted-or-rejected start-stage outcome. On accept `request` is non-null and
// `rejects` is empty; on a policy reject `request` is null and `rejects` is
// populated.
template <typename Driver>
struct StartOutcome {
  std::shared_ptr<AsyncRequest<Driver>> request;
  std::vector<::openpit::pretrade::Reject> rejects;

  [[nodiscard]] bool Passed() const noexcept {
    return static_cast<bool>(request);
  }
};

/// \brief Result value for an async full pre-trade call.
//
// Accepted-or-rejected main-stage outcome. On accept `reservation` is non-null;
// on a policy reject `reservation` is null and `rejects` is populated.
template <typename Driver>
struct ExecuteOutcome {
  std::shared_ptr<AsyncReservation<Driver>> reservation;
  std::vector<::openpit::pretrade::Reject> rejects;

  [[nodiscard]] bool Passed() const noexcept {
    return static_cast<bool>(reservation);
  }
};

/// \brief Result value for an async account-adjustment batch.
//
// Accepted-or-rejected adjustment outcome. On accept `batchError` is null and
// `outcomes` and `accountBlocks` carry the per-adjustment outcomes and
// recorded account blocks; on reject `batchError` is set. This mirrors the
// synchronous `AdjustmentResult` value shape.
struct AdjustmentOutcome {
  std::shared_ptr<::openpit::accountadjustment::BatchError> batchError;
  std::vector<::openpit::accountadjustment::Outcome> outcomes;
  std::vector<::openpit::accounts::AccountBlock> accountBlocks;

  [[nodiscard]] bool Passed() const noexcept { return !batchError; }
};

namespace detail {

// Reads the account id off an order's operation view without mutating it.
[[nodiscard]] inline std::optional<::openpit::param::AccountId> OrderAccountId(
    const ::openpit::model::Order& order) {
  if (!order.operation.has_value()) {
    return std::nullopt;
  }
  if (!order.operation->accountId.has_value()) {
    return std::nullopt;
  }
  return order.operation->accountId;
}

// `extractReportAccountID`.
[[nodiscard]] inline std::optional<::openpit::param::AccountId> ReportAccountId(
    const ::openpit::model::ExecutionReport& report) {
  if (!report.operation.has_value()) {
    return std::nullopt;
  }
  if (!report.operation->accountId.has_value()) {
    return std::nullopt;
  }
  return report.operation->accountId;
}

// The shared "account id is not set on the order or report" failure, matching
[[nodiscard]] inline Error MissingAccountId() {
  return Error(ErrorCode::MissingAccountId,
               "openpit/asyncengine: account ID is not set on the order or "
               "report");
}

}  // namespace detail

//------------------------------------------------------------------------------
// AsyncReservation

// Wraps a `pretrade::Reservation` so that finalization re-enters the same
// per-account queue as the call that produced it, preserving AccountSync up to
/// \brief Async wrapper around an accepted pre-trade reservation.
//
// Wraps the reservation lifecycle after `ExecutePreTrade` or request
// execution. Obtained from an `ExecuteOutcome`; held
// by `shared_ptr`.
//
// Like the synchronous reservation, misuse (commit after close, double commit)
// is a programmer error. The async layer does not invent a failure mode the
// synchronous API lacks: `Commit` has no error channel, so a misuse is not
// turned into a resolved-with-error future. The void futures resolve with
// `std::monostate` on success.
template <typename Driver>
class AsyncReservation
    : public std::enable_shared_from_this<AsyncReservation<Driver>> {
 public:
  AsyncReservation(::openpit::pretrade::Reservation reservation,
                   TypedAsyncEngine<Driver>* engine,
                   ::openpit::param::AccountId accountId)
      : m_reservation(std::move(reservation)),
        m_engine(engine),
        m_accountId(accountId) {}

  [[nodiscard]] ::openpit::param::AccountId AccountId() const noexcept {
    return m_accountId;
  }

  // Enqueues Commit; the reservation is not closed. Pair with Close.
  [[nodiscard]] Future<std::monostate> Commit(
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    return Run([](::openpit::pretrade::Reservation& r) { r.Commit(); },
               timeout);
  }

  // Enqueues Rollback; the reservation is not closed. Pair with Close.
  [[nodiscard]] Future<std::monostate> Rollback(
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    return Run([](::openpit::pretrade::Reservation& r) { r.Rollback(); },
               timeout);
  }

  // Enqueues Commit followed by releasing the reservation handle.
  [[nodiscard]] Future<std::monostate> CommitAndClose(
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    return Run(
        [](::openpit::pretrade::Reservation& r) {
          r.Commit();
          r = ::openpit::pretrade::Reservation();
        },
        timeout);
  }

  // Enqueues Rollback followed by releasing the reservation handle.
  [[nodiscard]] Future<std::monostate> RollbackAndClose(
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    return Run(
        [](::openpit::pretrade::Reservation& r) {
          r.Rollback();
          r = ::openpit::pretrade::Reservation();
        },
        timeout);
  }

  // Enqueues a plain release: destroying the reservation rolls back any
  // still-pending mutations if Commit was not called first.
  [[nodiscard]] Future<std::monostate> Close(
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    return Run(
        [](::openpit::pretrade::Reservation& r) {
          r = ::openpit::pretrade::Reservation();
        },
        timeout);
  }

 private:
  // Routes `op(reservation)` through the account queue. The task pins this
  // wrapper alive via `shared_from_this`, so dropping the caller's handle while
  // the task is queued does not dangle. On an aborted task the generic `Submit`
  // resolves the future with `Stopped`; the reservation is then released by
  // this wrapper's destruction, so the native handle never leaks.
  template <typename Op>
  [[nodiscard]] Future<std::monostate> Run(Op op,
                                           std::chrono::nanoseconds timeout);

  ::openpit::pretrade::Reservation m_reservation;
  TypedAsyncEngine<Driver>* m_engine;
  ::openpit::param::AccountId m_accountId;
};

//------------------------------------------------------------------------------
// AsyncRequest

/// \brief Async wrapper around a start-stage request.
//
// Wraps a `pretrade::Request` so that `Execute`/`Close` re-enter the same
// per-account queue as the `StartPreTrade` call that produced it. Obtained from
// a `StartOutcome`; held by `shared_ptr`.
//
// As in the synchronous contract, a request may be executed at most once;
// reusing it after Execute/Close is a programmer error.
template <typename Driver>
class AsyncRequest : public std::enable_shared_from_this<AsyncRequest<Driver>> {
 public:
  AsyncRequest(::openpit::pretrade::Request request,
               TypedAsyncEngine<Driver>* engine,
               ::openpit::param::AccountId accountId)
      : m_request(std::move(request)),
        m_engine(engine),
        m_accountId(accountId) {}

  [[nodiscard]] ::openpit::param::AccountId AccountId() const noexcept {
    return m_accountId;
  }

  // Enqueues the main stage. The future mirrors `ExecutePreTrade`: a non-null
  // reservation on accept, populated rejects on a policy reject. The underlying
  // request is always released once the main stage has been issued (or the task
  // aborted), so the native handle never leaks.
  [[nodiscard]] PairFuture<std::shared_ptr<AsyncReservation<Driver>>,
                           std::vector<::openpit::pretrade::Reject>>
  Execute(std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0));

  // Enqueues a release of the request without running the main stage. Still
  // serializes through the account queue so concurrent same-account calls stay
  // disallowed.
  [[nodiscard]] Future<std::monostate> Close(
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0));

 private:
  ::openpit::pretrade::Request m_request;
  TypedAsyncEngine<Driver>* m_engine;
  ::openpit::param::AccountId m_accountId;
};

//------------------------------------------------------------------------------
// AsyncAccounts

/// \brief Async account-administration view.
//
// Account-group and account/group-block administration bound to a
// `TypedAsyncEngine`. Carries no state of its own: every call routes through
// the parent engine.
//
// Group-scoped ops carry no account, so they pin to a deterministic queue keyed
// ids share the numeric routing space with account ids, which is benign because
// admin ops are rare and the native layer is concurrency-safe regardless.
template <typename Driver>
class AsyncAccounts {
 public:
  explicit AsyncAccounts(TypedAsyncEngine<Driver>* engine) noexcept
      : m_engine(engine) {}

  // Registers every account into `group`. Resolves with the optional
  // `AccountGroupError` (set on a domain conflict / reserved group). Returns
  // `MissingAccountId` when `accounts` is empty.
  [[nodiscard]] Future<std::optional<::openpit::accounts::AccountGroupError>>
  RegisterGroup(std::vector<::openpit::param::AccountId> accounts,
                ::openpit::param::AccountGroupId group,
                std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0));

  // Removes every account from `group`. Mirrors `RegisterGroup`.
  [[nodiscard]] Future<std::optional<::openpit::accounts::AccountGroupError>>
  UnregisterGroup(
      std::vector<::openpit::param::AccountId> accounts,
      ::openpit::param::AccountGroupId group,
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0));

  // Looks up the account-group of `account`, empty when it belongs to none.
  [[nodiscard]] Future<std::optional<::openpit::param::AccountGroupId>> GroupOf(
      ::openpit::param::AccountId account,
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0));

  // Blocks `account`; infallible (resolves with `std::monostate`).
  [[nodiscard]] Future<std::monostate> Block(
      ::openpit::param::AccountId account, std::string reason,
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0));

  // Unblocks `account`; infallible (resolves with `std::monostate`).
  [[nodiscard]] Future<std::monostate> Unblock(
      ::openpit::param::AccountId account,
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0));

  // Replaces a blocked account's reason. Resolves with the optional
  // `AccountBlockError` (set with kind `AccountNotBlocked` when not blocked).
  [[nodiscard]] Future<std::optional<::openpit::accounts::AccountBlockError>>
  ReplaceBlockReason(
      ::openpit::param::AccountId account, std::string reason,
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0));

  // Blocks `group`. Resolves with the optional `AccountBlockError` (kind
  // `ReservedGroup` for the default group). Pinned to the group's queue.
  [[nodiscard]] Future<std::optional<::openpit::accounts::AccountBlockError>>
  BlockGroup(::openpit::param::AccountGroupId group, std::string reason,
             std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0));

  // Unblocks `group`. Mirrors `BlockGroup`. Pinned to the group's queue.
  [[nodiscard]] Future<std::optional<::openpit::accounts::AccountBlockError>>
  UnblockGroup(::openpit::param::AccountGroupId group,
               std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0));

  // Replaces a blocked group's reason. Resolves with the optional
  // `AccountBlockError` (kind `ReservedGroup` / `GroupNotBlocked`). Pinned to
  // the group's queue.
  [[nodiscard]] Future<std::optional<::openpit::accounts::AccountBlockError>>
  ReplaceGroupBlockReason(
      ::openpit::param::AccountGroupId group, std::string reason,
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0));

 private:
  // Stable per-group routing key. Group ids (uint32) share the numeric routing
  // space with account ids (uint64); benign for rare admin ops.
  [[nodiscard]] static ::openpit::param::AccountId GroupRoutingKey(
      ::openpit::param::AccountGroupId group) noexcept {
    return ::openpit::param::AccountId::FromRaw(group.Raw());
  }

  TypedAsyncEngine<Driver>* m_engine;
};

//------------------------------------------------------------------------------
// TypedAsyncEngine

/// \brief Typed async facade exposing named OpenPit engine operations.
//
// Wraps the generic `AsyncEngine<Driver>` (all dispatch/threading/lifecycle)
// and adds the typed methods; lifecycle (`StopGraceful`/`StopHard`) forwards
// straight to it.
//
// Move-only (it uniquely owns the dispatcher). The follow-up wrappers
// (`AsyncRequest`/`AsyncReservation`) hold a back-pointer to this engine, so it
// must outlive any wrapper produced from it.
template <typename Driver>
class TypedAsyncEngine {
 public:
  TypedAsyncEngine(const TypedAsyncEngine&) = delete;
  TypedAsyncEngine& operator=(const TypedAsyncEngine&) = delete;
  TypedAsyncEngine(TypedAsyncEngine&&) noexcept = default;
  TypedAsyncEngine& operator=(TypedAsyncEngine&&) noexcept = default;
  ~TypedAsyncEngine() = default;

  //----------------------------------------------------------------------------
  // Pre-trade pipeline

  // Enqueues a start-stage call for `order`, pinned to its account. The future
  // resolves with a `StartOutcome`: a non-null request on accept, populated
  // rejects on a policy reject. Resolves immediately with `MissingAccountId`
  // when the order carries no account id.
  [[nodiscard]] Future<StartOutcome<Driver>> StartPreTrade(
      ::openpit::model::Order order,
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    const std::optional<::openpit::param::AccountId> accountId =
        detail::OrderAccountId(order);
    if (!accountId.has_value()) {
      Promise<StartOutcome<Driver>> promise;
      Future<StartOutcome<Driver>> future = promise.GetFuture();
      promise.Fail(detail::MissingAccountId());
      return future;
    }
    const ::openpit::param::AccountId pinned = *accountId;
    TypedAsyncEngine* self = this;
    // Delegate to the generic `Call` seam: it owns abort (resolves with
    // `Stopped`) and synchronous submit-failure (resolves with the queue error)
    // so the returned future is always resolved exactly once.
    return m_engine.Call(
        pinned,
        [self, pinned, order = std::move(order)](Driver& driver) {
          ::openpit::pretrade::StartResult result = driver.StartPreTrade(order);
          if (!result.Passed()) {
            return StartOutcome<Driver>{nullptr, std::move(result.rejects)};
          }
          auto request = std::make_shared<AsyncRequest<Driver>>(
              std::move(*result.request), self, pinned);
          return StartOutcome<Driver>{std::move(request), {}};
        },
        timeout);
  }

  // Enqueues a full pre-trade pipeline call for `order`, pinned to its account.
  // The future resolves with an `ExecuteOutcome`: a non-null reservation on
  // accept, populated rejects on a policy reject. Resolves immediately with
  // `MissingAccountId` when the order carries no account id.
  [[nodiscard]] Future<ExecuteOutcome<Driver>> ExecutePreTrade(
      ::openpit::model::Order order,
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    const std::optional<::openpit::param::AccountId> accountId =
        detail::OrderAccountId(order);
    if (!accountId.has_value()) {
      Promise<ExecuteOutcome<Driver>> promise;
      Future<ExecuteOutcome<Driver>> future = promise.GetFuture();
      promise.Fail(detail::MissingAccountId());
      return future;
    }
    const ::openpit::param::AccountId pinned = *accountId;
    TypedAsyncEngine* self = this;
    return m_engine.Call(
        pinned,
        [self, pinned, order = std::move(order)](Driver& driver) {
          ::openpit::pretrade::ExecuteResult result =
              driver.ExecutePreTrade(order);
          if (!result.Passed()) {
            return ExecuteOutcome<Driver>{nullptr, std::move(result.rejects)};
          }
          auto reservation = std::make_shared<AsyncReservation<Driver>>(
              std::move(*result.reservation), self, pinned);
          return ExecuteOutcome<Driver>{std::move(reservation), {}};
        },
        timeout);
  }

  // Enqueues a full drop-copy pre-trade call for `order`, pinned to its
  // account. Policy rejects and current account blocks do not prevent the
  // reservation from being returned; newly raised blocks are retained.
  [[nodiscard]] Future<ExecuteOutcome<Driver>> ExecutePreTradeDropCopy(
      ::openpit::model::Order order,
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    const std::optional<::openpit::param::AccountId> accountId =
        detail::OrderAccountId(order);
    if (!accountId.has_value()) {
      Promise<ExecuteOutcome<Driver>> promise;
      Future<ExecuteOutcome<Driver>> future = promise.GetFuture();
      promise.Fail(detail::MissingAccountId());
      return future;
    }
    const ::openpit::param::AccountId pinned = *accountId;
    TypedAsyncEngine* self = this;
    return m_engine.Call(
        pinned,
        [self, pinned, order = std::move(order)](Driver& driver) {
          ::openpit::pretrade::Reservation reservation =
              driver.ExecutePreTradeDropCopy(order);
          auto asyncReservation = std::make_shared<AsyncReservation<Driver>>(
              std::move(reservation), self, pinned);
          return ExecuteOutcome<Driver>{std::move(asyncReservation), {}};
        },
        timeout);
  }

  // Enqueues a post-trade call for `report`, pinned to its account. Resolves
  // with the `PostTradeResult`, or immediately with `MissingAccountId` when the
  // report carries no account id.
  [[nodiscard]] Future<::openpit::PostTradeResult> ApplyExecutionReport(
      ::openpit::model::ExecutionReport report,
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    const std::optional<::openpit::param::AccountId> accountId =
        detail::ReportAccountId(report);
    if (!accountId.has_value()) {
      Promise<::openpit::PostTradeResult> promise;
      Future<::openpit::PostTradeResult> future = promise.GetFuture();
      promise.Fail(detail::MissingAccountId());
      return future;
    }
    return m_engine.Call(
        *accountId,
        [report = std::move(report)](Driver& driver) {
          return driver.ApplyExecutionReport(report);
        },
        timeout);
  }

  // Enqueues a batch adjustment for `accountId` (supplied explicitly because
  // adjustments carry no account). Resolves with an `AdjustmentOutcome`: a
  // non-null batch error on reject, or outcomes and account blocks on accept.
  template <typename Adjustment>
  [[nodiscard]] Future<AdjustmentOutcome> ApplyAccountAdjustment(
      ::openpit::param::AccountId accountId,
      std::vector<Adjustment> adjustments,
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    return m_engine.Call(
        accountId,
        [accountId, adjustments = std::move(adjustments)](Driver& driver) {
          ::openpit::AdjustmentResult result =
              driver.template ApplyAccountAdjustment<Adjustment>(accountId,
                                                                 adjustments);
          AdjustmentOutcome out;
          if (result.batchError.has_value()) {
            out.batchError =
                std::make_shared<::openpit::accountadjustment::BatchError>(
                    std::move(*result.batchError));
          }
          out.outcomes = std::move(result.accountAdjustmentOutcomes);
          out.accountBlocks = std::move(result.accountBlocks);
          return out;
        },
        timeout);
  }

  // Returns the account-administration accessor bound to this engine.
  [[nodiscard]] AsyncAccounts<Driver> Accounts() noexcept {
    return AsyncAccounts<Driver>(this);
  }

  //----------------------------------------------------------------------------
  // Caller-owned work

  // Enqueues an arbitrary closure into the queue for `accountId`. Mirrors the
  // generic `AsyncEngine::Submit`.
  [[nodiscard]] Future<std::monostate> Submit(
      ::openpit::param::AccountId accountId, std::function<void()> fn,
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    return m_engine.Submit(accountId, std::move(fn), timeout);
  }

  //----------------------------------------------------------------------------
  // Lifecycle

  [[nodiscard]] bool StopGraceful(
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    return m_engine.StopGraceful(timeout);
  }

  [[nodiscard]] bool StopHard(
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    return m_engine.StopHard(timeout);
  }

  // The underlying generic engine, for the rare case a caller wants the generic
  // `Call`/`Call2` seam alongside the typed methods.
  [[nodiscard]] AsyncEngine<Driver>& Generic() noexcept { return m_engine; }

 private:
  template <typename D>
  friend class TypedShardedBuilder;
  template <typename D>
  friend class TypedDynamicBuilder;
  template <typename D>
  friend class AsyncRequest;
  template <typename D>
  friend class AsyncReservation;
  template <typename D>
  friend class AsyncAccounts;

  explicit TypedAsyncEngine(AsyncEngine<Driver> engine)
      : m_engine(std::move(engine)) {}

  AsyncEngine<Driver> m_engine;
};

//------------------------------------------------------------------------------
// AsyncRequest / AsyncReservation / AsyncAccounts out-of-line definitions

template <typename Driver>
template <typename Op>
[[nodiscard]] Future<std::monostate> AsyncReservation<Driver>::Run(
    Op op, std::chrono::nanoseconds timeout) {
  auto self = this->shared_from_this();
  return m_engine->m_engine.Submit(
      m_accountId, [self, op = std::move(op)]() { op(self->m_reservation); },
      timeout);
}

template <typename Driver>
[[nodiscard]] PairFuture<std::shared_ptr<AsyncReservation<Driver>>,
                         std::vector<::openpit::pretrade::Reject>>
AsyncRequest<Driver>::Execute(std::chrono::nanoseconds timeout) {
  using ReservationPtr = std::shared_ptr<AsyncReservation<Driver>>;
  using Rejects = std::vector<::openpit::pretrade::Reject>;
  auto self = this->shared_from_this();
  // Route through `Call2`: the op ignores the driver and acts on the wrapped
  // request, but the generic seam still owns abort/sync-failure resolution. The
  // task pins this wrapper alive via `shared_from_this`. On an abort the future
  // resolves with `Stopped` and this request is released by the wrapper's
  // destruction, so the native handle never leaks.
  return m_engine->m_engine.template Call2<ReservationPtr, Rejects>(
      m_accountId,
      [self](Driver&) -> std::pair<ReservationPtr, Rejects> {
        ::openpit::pretrade::ExecuteResult result = self->m_request.Execute();
        // `Request::Execute` consumes the request; release our copy so the
        // native handle is freed regardless of outcome.
        self->m_request = ::openpit::pretrade::Request();
        if (!result.Passed()) {
          return {nullptr, std::move(result.rejects)};
        }
        auto reservation = std::make_shared<AsyncReservation<Driver>>(
            std::move(*result.reservation), self->m_engine, self->m_accountId);
        return {std::move(reservation), Rejects{}};
      },
      timeout);
}

template <typename Driver>
[[nodiscard]] Future<std::monostate> AsyncRequest<Driver>::Close(
    std::chrono::nanoseconds timeout) {
  auto self = this->shared_from_this();
  return m_engine->m_engine.Submit(
      m_accountId,
      [self]() { self->m_request = ::openpit::pretrade::Request(); }, timeout);
}

template <typename Driver>
[[nodiscard]] Future<std::optional<::openpit::accounts::AccountGroupError>>
AsyncAccounts<Driver>::RegisterGroup(
    std::vector<::openpit::param::AccountId> accounts,
    ::openpit::param::AccountGroupId group, std::chrono::nanoseconds timeout) {
  using Out = std::optional<::openpit::accounts::AccountGroupError>;
  if (accounts.empty()) {
    Promise<Out> promise;
    Future<Out> future = promise.GetFuture();
    promise.Fail(detail::MissingAccountId());
    return future;
  }
  const ::openpit::param::AccountId pinned = accounts.front();
  return m_engine->m_engine.Call(
      pinned,
      [accounts = std::move(accounts), group](Driver& driver) {
        return driver.Accounts().RegisterGroup(accounts, group);
      },
      timeout);
}

template <typename Driver>
[[nodiscard]] Future<std::optional<::openpit::accounts::AccountGroupError>>
AsyncAccounts<Driver>::UnregisterGroup(
    std::vector<::openpit::param::AccountId> accounts,
    ::openpit::param::AccountGroupId group, std::chrono::nanoseconds timeout) {
  using Out = std::optional<::openpit::accounts::AccountGroupError>;
  if (accounts.empty()) {
    Promise<Out> promise;
    Future<Out> future = promise.GetFuture();
    promise.Fail(detail::MissingAccountId());
    return future;
  }
  const ::openpit::param::AccountId pinned = accounts.front();
  return m_engine->m_engine.Call(
      pinned,
      [accounts = std::move(accounts), group](Driver& driver) {
        return driver.Accounts().UnregisterGroup(accounts, group);
      },
      timeout);
}

template <typename Driver>
[[nodiscard]] Future<std::optional<::openpit::param::AccountGroupId>>
AsyncAccounts<Driver>::GroupOf(::openpit::param::AccountId account,
                               std::chrono::nanoseconds timeout) {
  return m_engine->m_engine.Call(
      account.Raw(),
      [account](Driver& driver) { return driver.Accounts().GroupOf(account); },
      timeout);
}

template <typename Driver>
[[nodiscard]] Future<std::monostate> AsyncAccounts<Driver>::Block(
    ::openpit::param::AccountId account, std::string reason,
    std::chrono::nanoseconds timeout) {
  TypedAsyncEngine<Driver>* engine = m_engine;
  return engine->m_engine.Submit(
      account,
      [engine, account, reason = std::move(reason)]() {
        engine->m_engine.DriverRef().Accounts().Block(account, reason);
      },
      timeout);
}

template <typename Driver>
[[nodiscard]] Future<std::monostate> AsyncAccounts<Driver>::Unblock(
    ::openpit::param::AccountId account, std::chrono::nanoseconds timeout) {
  TypedAsyncEngine<Driver>* engine = m_engine;
  return engine->m_engine.Submit(
      account,
      [engine, account]() {
        engine->m_engine.DriverRef().Accounts().Unblock(account);
      },
      timeout);
}

template <typename Driver>
[[nodiscard]] Future<std::optional<::openpit::accounts::AccountBlockError>>
AsyncAccounts<Driver>::ReplaceBlockReason(::openpit::param::AccountId account,
                                          std::string reason,
                                          std::chrono::nanoseconds timeout) {
  return m_engine->m_engine.Call(
      account.Raw(),
      [account, reason = std::move(reason)](Driver& driver) {
        return driver.Accounts().ReplaceBlockReason(account, reason);
      },
      timeout);
}

template <typename Driver>
[[nodiscard]] Future<std::optional<::openpit::accounts::AccountBlockError>>
AsyncAccounts<Driver>::BlockGroup(::openpit::param::AccountGroupId group,
                                  std::string reason,
                                  std::chrono::nanoseconds timeout) {
  return m_engine->m_engine.Call(
      GroupRoutingKey(group),
      [group, reason = std::move(reason)](Driver& driver) {
        return driver.Accounts().BlockGroup(group, reason);
      },
      timeout);
}

template <typename Driver>
[[nodiscard]] Future<std::optional<::openpit::accounts::AccountBlockError>>
AsyncAccounts<Driver>::UnblockGroup(::openpit::param::AccountGroupId group,
                                    std::chrono::nanoseconds timeout) {
  return m_engine->m_engine.Call(
      GroupRoutingKey(group),
      [group](Driver& driver) { return driver.Accounts().UnblockGroup(group); },
      timeout);
}

template <typename Driver>
[[nodiscard]] Future<std::optional<::openpit::accounts::AccountBlockError>>
AsyncAccounts<Driver>::ReplaceGroupBlockReason(
    ::openpit::param::AccountGroupId group, std::string reason,
    std::chrono::nanoseconds timeout) {
  return m_engine->m_engine.Call(
      GroupRoutingKey(group),
      [group, reason = std::move(reason)](Driver& driver) {
        return driver.Accounts().ReplaceGroupBlockReason(group, reason);
      },
      timeout);
}

//------------------------------------------------------------------------------
// TypedBuilder

/// \brief Typed builder stage for a fixed number of account shards.
//
// Second stage after `TypedShardedBuilder`/`TypedDynamicBuilder` advance from
// `TypedBuilder`. The builder reuses the generic `Builder<Driver>` chain
// verbatim and wraps the resulting `AsyncEngine<Driver>` into a
// `TypedAsyncEngine<Driver>`.
template <typename Driver>
class TypedShardedBuilder {
 public:
  [[nodiscard]] TypedAsyncEngine<Driver> Build() {
    return TypedAsyncEngine<Driver>(m_inner.Build());
  }

 private:
  template <typename D>
  friend class TypedBuilder;

  explicit TypedShardedBuilder(ShardedBuilder<Driver> inner)
      : m_inner(std::move(inner)) {}

  ShardedBuilder<Driver> m_inner;
};

/// \brief Typed builder stage for demand-created account queues.
template <typename Driver>
class TypedDynamicBuilder {
 public:
  TypedDynamicBuilder& MaxQueues(std::size_t maxQueues) {
    m_inner.MaxQueues(maxQueues);
    return *this;
  }

  TypedDynamicBuilder& IdleCleanupAfter(std::chrono::nanoseconds idle) {
    m_inner.IdleCleanupAfter(idle);
    return *this;
  }

  [[nodiscard]] TypedAsyncEngine<Driver> Build() {
    return TypedAsyncEngine<Driver>(m_inner.Build());
  }

 private:
  template <typename D>
  friend class TypedBuilder;

  explicit TypedDynamicBuilder(DynamicBuilder<Driver> inner)
      : m_inner(std::move(inner)) {}

  DynamicBuilder<Driver> m_inner;
};

/// \brief Entry builder for `TypedAsyncEngine`.
//
// Entry point of the typed builder chain. Wraps the generic `Builder<Driver>`,
// exposing the same configuration knobs, then advances to a strategy stage that
// produces a `TypedAsyncEngine`.
template <typename Driver>
class TypedBuilder {
 public:
  explicit TypedBuilder(Driver& driver) : m_inner(driver) {}

  TypedBuilder& WithStopUnderlying(StopUnderlying stop) {
    m_inner.WithStopUnderlying(std::move(stop));
    return *this;
  }

  TypedBuilder& WithObserver(Observer& observer) {
    m_inner.WithObserver(observer);
    return *this;
  }

  TypedBuilder& WithQueueCapacity(std::size_t capacity) {
    m_inner.WithQueueCapacity(capacity);
    return *this;
  }

  TypedBuilder& WithSlowSubmitThreshold(std::chrono::nanoseconds threshold) {
    m_inner.WithSlowSubmitThreshold(threshold);
    return *this;
  }

  [[nodiscard]] TypedShardedBuilder<Driver> Sharded(std::size_t workers) {
    return TypedShardedBuilder<Driver>(m_inner.Sharded(workers));
  }

  [[nodiscard]] TypedDynamicBuilder<Driver> Dynamic() {
    return TypedDynamicBuilder<Driver>(m_inner.Dynamic());
  }

 private:
  Builder<Driver> m_inner;
};

/// \brief Owning typed async engine built over the default `EngineAdapter`.
//
// Convenience wrapper returned by `MakeTypedAsyncEngine`. It owns the adapter
// object that the typed engine borrows, so callers do not need to manage a
// separate driver lifetime for the common `openpit::Engine` case.
class OwnedTypedAsyncEngine {
 public:
  using Driver = EngineAdapter;

  OwnedTypedAsyncEngine(const OwnedTypedAsyncEngine&) = delete;
  OwnedTypedAsyncEngine& operator=(const OwnedTypedAsyncEngine&) = delete;
  OwnedTypedAsyncEngine(OwnedTypedAsyncEngine&&) noexcept = default;
  OwnedTypedAsyncEngine& operator=(OwnedTypedAsyncEngine&&) noexcept = default;
  ~OwnedTypedAsyncEngine() = default;

  [[nodiscard]] Future<StartOutcome<Driver>> StartPreTrade(
      ::openpit::model::Order order,
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    return m_engine.StartPreTrade(std::move(order), timeout);
  }

  [[nodiscard]] Future<ExecuteOutcome<Driver>> ExecutePreTrade(
      ::openpit::model::Order order,
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    return m_engine.ExecutePreTrade(std::move(order), timeout);
  }

  [[nodiscard]] Future<ExecuteOutcome<Driver>> ExecutePreTradeDropCopy(
      ::openpit::model::Order order,
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    return m_engine.ExecutePreTradeDropCopy(std::move(order), timeout);
  }

  [[nodiscard]] Future<::openpit::PostTradeResult> ApplyExecutionReport(
      ::openpit::model::ExecutionReport report,
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    return m_engine.ApplyExecutionReport(std::move(report), timeout);
  }

  template <typename Adjustment>
  [[nodiscard]] Future<AdjustmentOutcome> ApplyAccountAdjustment(
      ::openpit::param::AccountId accountId,
      std::vector<Adjustment> adjustments,
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    return m_engine.ApplyAccountAdjustment(accountId, std::move(adjustments),
                                           timeout);
  }

  [[nodiscard]] AsyncAccounts<Driver> Accounts() noexcept {
    return m_engine.Accounts();
  }

  [[nodiscard]] Future<std::monostate> Submit(
      ::openpit::param::AccountId accountId, std::function<void()> fn,
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    return m_engine.Submit(accountId, std::move(fn), timeout);
  }

  [[nodiscard]] bool StopGraceful(
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    return m_engine.StopGraceful(timeout);
  }

  [[nodiscard]] bool StopHard(
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    return m_engine.StopHard(timeout);
  }

  [[nodiscard]] TypedAsyncEngine<Driver>& Typed() noexcept { return m_engine; }
  [[nodiscard]] const TypedAsyncEngine<Driver>& Typed() const noexcept {
    return m_engine;
  }

 private:
  friend OwnedTypedAsyncEngine MakeTypedAsyncEngine(
      const ::openpit::Engine& engine, std::size_t workers);

  OwnedTypedAsyncEngine(std::unique_ptr<Driver> driver,
                        TypedAsyncEngine<Driver> engine) noexcept
      : m_driver(std::move(driver)), m_engine(std::move(engine)) {}

  std::unique_ptr<Driver> m_driver;
  TypedAsyncEngine<Driver> m_engine;
};

/// \brief Builds a sharded typed async engine over `openpit::Engine`.
//
// Shortcut for the common production path. The returned wrapper owns the
// `EngineAdapter`; the source engine must still outlive the async wrapper.
[[nodiscard]] inline OwnedTypedAsyncEngine MakeTypedAsyncEngine(
    const ::openpit::Engine& engine, std::size_t workers) {
  auto driver = std::make_unique<EngineAdapter>(engine);
  TypedAsyncEngine<EngineAdapter> async =
      TypedBuilder<EngineAdapter>(*driver).Sharded(workers).Build();
  return OwnedTypedAsyncEngine(std::move(driver), std::move(async));
}

}  // namespace openpit::asyncengine
