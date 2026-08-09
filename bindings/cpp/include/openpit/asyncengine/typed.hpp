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

#include "openpit/accountadjustment/account_adjustment.hpp"
#include "openpit/accounts/accounts.hpp"
#include "openpit/asyncengine/engine.hpp"
#include "openpit/asyncengine/future.hpp"
#include "openpit/engine.hpp"
#include "openpit/model/model.hpp"
#include "openpit/param/account_id.hpp"
#include "openpit/pretrade/decision.hpp"

#include <openpit.h>

#include <chrono>
#include <cstddef>
#include <exception>
#include <functional>
#include <memory>
#include <mutex>
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
// layer can supply any type exposing the same members. The borrowed engine
// object must remain alive at the same address and must not be moved while an
// adapter or async wrapper refers to it.
//
// ACCOUNT PINNING. Every named method routes through the per-account queue
// keyed by the order/report account id (read without mutating the caller's
// payload), or by the supplied id for adjustments and admin ops. An order
// whose account id cannot be read is refused up front, drop copy included.
// `StartPreTrade` and `ExecutePreTrade` yield an `AsyncRequest` /
// `AsyncReservation`, while `ApplyDropCopy` yields an
// `AsyncDropCopyOperation`. Their follow-up calls (`Execute`, `Commit`,
// `Rollback`, `Close`, ...) re-enter the same per-account queue, preserving the
// AccountSync invariant across the start->execute->finalize boundary.
//
// RESULT SHAPES. The futures carry the same values the synchronous engine
// returns. Because `pretrade::Request`, `pretrade::Reservation`,
// `pretrade::DropCopyOperation`, and `accountadjustment::BatchError` are
// move-only RAII handles while the follow-up wrappers are observed from a
// shared state, the accepted-channel value is held by `shared_ptr`: the worker
// that resolves the future and the consumer that drives the follow-up calls
// genuinely share ownership of one wrapper, so `shared_ptr` is the correct
// model rather than a copy or a raw move.
//
// ERROR MODEL. A missing account id resolves the future with the value
// `ErrorCode::MissingAccountId`. Stop, queue-limit, and dispatch errors,
// including `ErrorCode::SubmitCancelled`, are the same value-typed errors the
// generic layer uses. `SubmitCancelled` means the task was never enqueued. For
// a post-trade call, the already-occurred fact remains unregistered: a
// reservation is not released and spot funds are not settled, so the caller
// MUST retry. Pre-trade rejects and adjustment batch rejects are values in the
// accepted-or-rejected tuple, not errors. A runtime failure inside a driver
// call surfaces as `ErrorCode::TaskFailed` carrying the thrown message. An
// exception from the engine never crosses the worker boundary.
// A null payload is a precondition violation reported synchronously as
// `openpit::Error` before work is queued, distinct from value-typed dispatch
// errors.

namespace openpit::asyncengine {

//------------------------------------------------------------------------------
// Driver

/// \brief Adapter that exposes `openpit::Engine` methods to `TypedAsyncEngine`.
//
// Adapts a borrowed `openpit::Engine&` to the async driver seam. Non-owning:
// the same engine object must remain alive at its original address and must not
// be moved while any adapter or async engine built over it exists. Rvalue
// engines are rejected at compile time. Copyable and cheap (one pointer); the
// typed layer borrows it by reference like the generic `AsyncEngine`. Each
// member is just the corresponding synchronous engine call; the async layer
// supplies all concurrency.
class EngineAdapter {
 public:
  explicit EngineAdapter(const ::openpit::Engine& engine) noexcept
      : m_engine(&engine) {}
  EngineAdapter(::openpit::Engine&&) = delete;
  EngineAdapter(const ::openpit::Engine&&) = delete;

  [[nodiscard]] ::openpit::pretrade::StartResult StartPreTrade(
      std::unique_ptr<const ::openpit::Order> order) const {
    return m_engine->StartPreTrade(std::move(order));
  }

  [[nodiscard]] ::openpit::pretrade::ExecuteResult ExecutePreTrade(
      std::unique_ptr<const ::openpit::Order> order) const {
    if (!order) {
      throw ::openpit::Error("ExecutePreTrade requires a non-null order");
    }
    return m_engine->ExecutePreTrade(*order);
  }

  [[nodiscard]] ::openpit::pretrade::DropCopyResult ApplyDropCopy(
      std::unique_ptr<const ::openpit::Order> order) const {
    if (!order) {
      throw ::openpit::Error("ApplyDropCopy requires a non-null order");
    }
    return m_engine->ApplyDropCopy(*order);
  }

  [[nodiscard]] ::openpit::PostTradeResult ApplyExecutionReport(
      std::unique_ptr<const ::openpit::ExecutionReport> report) const {
    if (!report) {
      throw ::openpit::Error(
          "ApplyExecutionReport requires a non-null execution report");
    }
    return m_engine->ApplyExecutionReport(*report);
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
template <typename Driver>
class AsyncDropCopyOperation;

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

/// \brief Result value for an async drop-copy call.
//
// Applied-or-rejected drop-copy outcome. On accept `operation` is non-null; on
// a fatal evaluation failure `operation` is null and `rejects` is populated.
template <typename Driver>
struct DropCopyOutcome {
  std::shared_ptr<AsyncDropCopyOperation<Driver>> operation;
  std::vector<::openpit::pretrade::Reject> rejects;

  [[nodiscard]] bool Passed() const noexcept {
    return static_cast<bool>(operation);
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
    const ::openpit::Order& order) {
  // Every client payload derives from `model::Order`; the base constructor is
  // private. `nullopt` therefore means only that the account id is absent.
  const auto& modelOrder = static_cast<const ::openpit::model::Order&>(order);
  if (!modelOrder.operation.has_value()) {
    return std::nullopt;
  }
  if (!modelOrder.operation->accountId.has_value()) {
    return std::nullopt;
  }
  return modelOrder.operation->accountId;
}

// `extractReportAccountID`.
[[nodiscard]] inline std::optional<::openpit::param::AccountId> ReportAccountId(
    const ::openpit::ExecutionReport& report) {
  // Every client payload derives from `model::ExecutionReport`; the base
  // constructor is private. `nullopt` therefore means only no account id.
  const auto& modelReport =
      static_cast<const ::openpit::model::ExecutionReport&>(report);
  if (!modelReport.operation.has_value()) {
    return std::nullopt;
  }
  if (!modelReport.operation->accountId.has_value()) {
    return std::nullopt;
  }
  return modelReport.operation->accountId;
}

// The shared "account id is not set on the order or report" failure, matching
[[nodiscard]] inline Error MissingAccountId() {
  return Error(ErrorCode::MissingAccountId,
               "openpit/asyncengine: account ID is not set on the order or "
               "report");
}

// Hands a failed submit's handle release to the account lane and deliberately
// drops the future that tracks it. The error the caller is already waiting for
// is the actionable outcome, and a cleanup failure must never replace or mask
// it; cleanup itself reports through the future the caller does observe (see
// `CompleteMandatoryCleanup`), so nothing is lost by not observing this one.
// Naming the discard keeps that decision explicit at every call site.
template <typename Driver>
void DetachMandatoryCleanup(AsyncEngine<Driver>& engine,
                            ::openpit::param::AccountId accountId,
                            std::function<void()> cleanup) {
  (void)engine.ScheduleMandatoryCleanup(accountId, std::move(cleanup));
}

template <typename PromiseType, typename Cleanup>
void CompleteMandatoryCleanup(PromiseType promise, Error originalError,
                              Cleanup cleanup) {
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
  promise.Fail(std::move(originalError));
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
                   AsyncEngine<Driver>* engine,
                   ::openpit::param::AccountId accountId)
      : m_reservation(std::move(reservation)),
        m_engine(engine),
        m_accountId(accountId) {}

  [[nodiscard]] ::openpit::param::AccountId AccountId() const noexcept {
    return m_accountId;
  }

  // Enqueues Commit; the reservation is not closed. Pair with Close. A throwing
  // mutation commit callback fails this future with `ErrorCode::TaskFailed` and
  // arms the engine kill switch, blocking every account; see
  // `pretrade::Reservation` for that contract.
  //
  // A hard stop aborts this task and fails the future with
  // `ErrorCode::Stopped` without releasing the underlying reservation, so the
  // caller must still `Close` it (or use `CommitAndClose`) to avoid leaking the
  // native handle.
  [[nodiscard]] Future<std::monostate> Commit(
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    return Run([](::openpit::pretrade::Reservation& r) { r.Commit(); }, timeout,
               /*abortCloses=*/false);
  }

  // Enqueues Rollback; the reservation is not closed. Pair with Close. A
  // throwing mutation rollback callback fails this future with
  // `ErrorCode::TaskFailed` and arms the engine kill switch, blocking every
  // account; see `pretrade::Reservation` for that contract.
  //
  // A hard stop aborts this task and fails the future with
  // `ErrorCode::Stopped` without releasing the underlying reservation, so the
  // caller must still `Close` it (or use `RollbackAndClose`) to avoid leaking
  // the native handle.
  [[nodiscard]] Future<std::monostate> Rollback(
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    return Run([](::openpit::pretrade::Reservation& r) { r.Rollback(); },
               timeout, /*abortCloses=*/false);
  }

  // Enqueues Commit followed by releasing the reservation handle.
  [[nodiscard]] Future<std::monostate> CommitAndClose(
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    return Run(
        [](::openpit::pretrade::Reservation& r) {
          r.Commit();
          r = ::openpit::pretrade::Reservation();
        },
        timeout, /*abortCloses=*/true);
  }

  // Enqueues Rollback followed by releasing the reservation handle.
  [[nodiscard]] Future<std::monostate> RollbackAndClose(
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    return Run(
        [](::openpit::pretrade::Reservation& r) {
          r.Rollback();
          r = ::openpit::pretrade::Reservation();
        },
        timeout, /*abortCloses=*/true);
  }

  // Enqueues a plain release: destroying the reservation rolls back any
  // still-pending mutations if Commit was not called first.
  [[nodiscard]] Future<std::monostate> Close(
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    return Run(
        [](::openpit::pretrade::Reservation& r) {
          r = ::openpit::pretrade::Reservation();
        },
        timeout, /*abortCloses=*/true);
  }

 private:
  // Routes `op(reservation)` through the account queue. The task pins this
  // wrapper alive via `shared_from_this`, so dropping the caller's handle while
  // the task is queued does not dangle. Closing tasks transfer submit failures
  // and hard-stop aborts to the account lane before releasing the reservation.
  template <typename Op>
  [[nodiscard]] Future<std::monostate> Run(Op op,
                                           std::chrono::nanoseconds timeout,
                                           bool abortCloses);

  ::openpit::pretrade::Reservation m_reservation;
  AsyncEngine<Driver>* m_engine;
  ::openpit::param::AccountId m_accountId;
  mutable std::mutex m_mutex;
};

//------------------------------------------------------------------------------
// AsyncDropCopyOperation

/// \brief Async wrapper around an applied drop-copy operation.
//
// Wraps the operation lifecycle after `ApplyDropCopy`, so that finalization
// re-enters the same per-account queue as the call that produced it. Obtained
// from a `DropCopyOutcome`; held by `shared_ptr`.
//
// Finalization is idempotent. Snapshot accessors serialize with queued
// finalization on this wrapper. The void futures resolve with `std::monostate`
// on success.
template <typename Driver>
class AsyncDropCopyOperation
    : public std::enable_shared_from_this<AsyncDropCopyOperation<Driver>> {
 public:
  AsyncDropCopyOperation(::openpit::pretrade::DropCopyOperation operation,
                         AsyncEngine<Driver>* engine,
                         ::openpit::param::AccountId accountId)
      : m_operation(std::move(operation)),
        m_engine(engine),
        m_accountId(accountId) {}

  [[nodiscard]] ::openpit::param::AccountId AccountId() const noexcept {
    return m_accountId;
  }

  [[nodiscard]] ::openpit::pretrade::PreTradeLock Lock() const {
    std::lock_guard<std::mutex> lock(m_mutex);
    return m_operation.Lock();
  }

  [[nodiscard]] std::vector<::openpit::accountadjustment::Outcome>
  AccountAdjustments() const {
    std::lock_guard<std::mutex> lock(m_mutex);
    return m_operation.AccountAdjustments();
  }

  [[nodiscard]] std::optional<::openpit::accounts::AccountBlock> AccountBlock()
      const {
    std::lock_guard<std::mutex> lock(m_mutex);
    return m_operation.AccountBlock();
  }

  [[nodiscard]] bool IsAccountBlocked() const {
    std::lock_guard<std::mutex> lock(m_mutex);
    return m_operation.IsAccountBlocked();
  }

  // Enqueues Commit; the operation is not closed. Pair with Close. A throwing
  // mutation commit callback fails this future with `ErrorCode::TaskFailed` and
  // arms the engine kill switch, blocking every account; see
  // `pretrade::DropCopyOperation` for that contract.
  //
  // A hard stop aborts this task and fails the future with
  // `ErrorCode::Stopped` without releasing the underlying operation, so the
  // caller must still `Close` it (or use `CommitAndClose`) to avoid leaking the
  // native handle.
  [[nodiscard]] Future<std::monostate> Commit(
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    return Run(
        [](::openpit::pretrade::DropCopyOperation& operation) {
          operation.Commit();
        },
        timeout, /*abortCloses=*/false);
  }

  // Enqueues Rollback; the operation is not closed. Pair with Close. A throwing
  // mutation rollback callback fails this future with `ErrorCode::TaskFailed`
  // and arms the engine kill switch, blocking every account; see
  // `pretrade::DropCopyOperation` for that contract.
  //
  // A hard stop aborts this task and fails the future with
  // `ErrorCode::Stopped` without releasing the underlying operation, so the
  // caller must still `Close` it (or use `RollbackAndClose`) to avoid leaking
  // the native handle.
  [[nodiscard]] Future<std::monostate> Rollback(
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    return Run(
        [](::openpit::pretrade::DropCopyOperation& operation) {
          operation.Rollback();
        },
        timeout, /*abortCloses=*/false);
  }

  // Enqueues Commit followed by releasing the operation handle.
  [[nodiscard]] Future<std::monostate> CommitAndClose(
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    return Run(
        [](::openpit::pretrade::DropCopyOperation& operation) {
          operation.Commit();
          operation = ::openpit::pretrade::DropCopyOperation();
        },
        timeout, /*abortCloses=*/true);
  }

  // Enqueues Rollback followed by releasing the operation handle.
  [[nodiscard]] Future<std::monostate> RollbackAndClose(
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    return Run(
        [](::openpit::pretrade::DropCopyOperation& operation) {
          operation.Rollback();
          operation = ::openpit::pretrade::DropCopyOperation();
        },
        timeout, /*abortCloses=*/true);
  }

  // Enqueues a plain release: destroying the operation rolls back any
  // still-pending mutations if Commit was not called first.
  [[nodiscard]] Future<std::monostate> Close(
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    return Run(
        [](::openpit::pretrade::DropCopyOperation& operation) {
          operation = ::openpit::pretrade::DropCopyOperation();
        },
        timeout, /*abortCloses=*/true);
  }

 private:
  // Routes `op(operation)` through the account queue. The task pins this
  // wrapper alive via `shared_from_this`, so dropping the caller's handle while
  // the task is queued does not dangle. A closing task transfers a submit
  // failure or hard-stop abort to the same lane before releasing the operation.
  template <typename Op>
  [[nodiscard]] Future<std::monostate> Run(Op op,
                                           std::chrono::nanoseconds timeout,
                                           bool abortCloses);

  ::openpit::pretrade::DropCopyOperation m_operation;
  AsyncEngine<Driver>* m_engine;
  ::openpit::param::AccountId m_accountId;
  mutable std::mutex m_mutex;
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
               AsyncEngine<Driver>* engine,
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
  AsyncEngine<Driver>* m_engine;
  ::openpit::param::AccountId m_accountId;
  mutable std::mutex m_mutex;
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
// by the group id. Group ids share the numeric routing space with account ids,
// which is benign because admin ops are rare and the native layer is
// concurrency-safe regardless.
//
// The engine-wide unblock names neither an account nor a group, so it pins to
// its own deterministic queue for the same reason.
template <typename Driver>
class AsyncAccounts {
 public:
  explicit AsyncAccounts(AsyncEngine<Driver>* engine) noexcept
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

  // Clears the engine-wide block; infallible (resolves with `std::monostate`).
  // Lifting an inactive engine-wide block is a no-op, and accounts and account
  // groups blocked individually stay blocked. Pinned to the engine-wide queue,
  // so engine-wide clears serialize against each other.
  //
  // An engine-wide block never comes from `Block` or `BlockGroup`: the engine
  // raises it itself, on a kill switch reported for an execution report with no
  // readable account, or on a failed mutation finalizer of a custom policy -
  // which every mutation registered from C++ is.
  [[nodiscard]] Future<std::monostate> UnblockAll(
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
    return ::openpit::detail::FromNative<::openpit::param::AccountId>(
        ::openpit::detail::Native(group));
  }

  // Stable routing key for admin ops that name neither an account nor a group,
  // so they pin to one deterministic queue instead of an arbitrary one. The
  // collision with account and group keys is benign for the same reason it is
  // in `GroupRoutingKey`.
  [[nodiscard]] static ::openpit::param::AccountId
  EngineWideRoutingKey() noexcept {
    return ::openpit::param::AccountId::FromUint64(0);
  }

  AsyncEngine<Driver>* m_engine;
};

//------------------------------------------------------------------------------
// TypedAsyncEngine

/// \brief Typed async facade exposing named OpenPit engine operations.
//
// Wraps the generic `AsyncEngine<Driver>` (all dispatch/threading/lifecycle)
// and adds the typed methods; lifecycle (`StopGraceful`/`StopHard`) forwards
// straight to it.
//
// Non-copyable and move-constructible. The dispatch state has a stable address,
// so moving this facade does not invalidate follow-up `AsyncRequest` or
// `AsyncReservation` wrappers. Move assignment is disabled because replacing a
// live target could invalidate wrappers produced by that target. The engine
// must still outlive every wrapper it produced.
//
// SERIALIZATION. This facade adds whole-pipeline isolation beyond a fully
// synchronized direct engine. Every operation is routed by account id to one
// queue drained by one worker, so two complete pipelines for the same account
// never overlap, and follow-up calls on `AsyncRequest` and `AsyncReservation`
// re-enter that same queue. Callers need no locking of their own on top.
template <typename Driver>
class TypedAsyncEngine {
 public:
  TypedAsyncEngine(const TypedAsyncEngine&) = delete;
  TypedAsyncEngine& operator=(const TypedAsyncEngine&) = delete;
  TypedAsyncEngine(TypedAsyncEngine&&) noexcept = default;
  TypedAsyncEngine& operator=(TypedAsyncEngine&&) = delete;
  ~TypedAsyncEngine() = default;

  //----------------------------------------------------------------------------
  // Pre-trade pipeline

  // Enqueues a start-stage call for `order`, pinned to its account. The future
  // resolves with a `StartOutcome`: a non-null request on accept, populated
  // rejects on a policy reject. Resolves immediately with `MissingAccountId`
  // when the order carries no account id.
  [[nodiscard]] Future<StartOutcome<Driver>> StartPreTrade(
      std::unique_ptr<const ::openpit::Order> order,
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    if (!order) {
      throw ::openpit::Error("StartPreTrade requires a non-null order");
    }
    const std::optional<::openpit::param::AccountId> accountId =
        detail::OrderAccountId(*order);
    if (!accountId.has_value()) {
      Promise<StartOutcome<Driver>> promise;
      Future<StartOutcome<Driver>> future = promise.GetFuture();
      promise.Fail(detail::MissingAccountId());
      return future;
    }
    const ::openpit::param::AccountId pinned = *accountId;
    AsyncEngine<Driver>* engine = m_engine.get();
    // Delegate to the generic `Call` seam: it owns abort (resolves with
    // `Stopped`) and synchronous submit-failure (resolves with the queue error)
    // so the returned future is always resolved exactly once.
    return m_engine->Call(
        pinned,
        [engine, pinned, order = std::move(order)](Driver& driver) mutable {
          ::openpit::pretrade::StartResult result =
              driver.StartPreTrade(std::move(order));
          if (!result.Passed()) {
            return StartOutcome<Driver>{nullptr, std::move(result.rejects)};
          }
          auto request = std::make_shared<AsyncRequest<Driver>>(
              std::move(*result.request), engine, pinned);
          return StartOutcome<Driver>{std::move(request), {}};
        },
        timeout);
  }

  template <
      typename OrderT,
      std::enable_if_t<
          std::is_base_of_v<::openpit::Order, std::decay_t<OrderT>>, int> = 0>
  [[nodiscard]] Future<StartOutcome<Driver>> StartPreTrade(
      OrderT&& order,
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    return StartPreTrade(
        ::openpit::detail::OwnExactPolymorphic<::openpit::Order>(
            std::forward<OrderT>(order),
            "StartPreTrade cannot own a base-typed reference to a derived "
            "order; pass std::unique_ptr<const openpit::Order> instead"),
        timeout);
  }

  // Enqueues a full pre-trade pipeline call for `order`, pinned to its account.
  // The future resolves with an `ExecuteOutcome`: a non-null reservation on
  // accept, populated rejects on a policy reject. Resolves immediately with
  // `MissingAccountId` when the order carries no account id.
  [[nodiscard]] Future<ExecuteOutcome<Driver>> ExecutePreTrade(
      std::unique_ptr<const ::openpit::Order> order,
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    if (!order) {
      throw ::openpit::Error("ExecutePreTrade requires a non-null order");
    }
    const std::optional<::openpit::param::AccountId> accountId =
        detail::OrderAccountId(*order);
    if (!accountId.has_value()) {
      Promise<ExecuteOutcome<Driver>> promise;
      Future<ExecuteOutcome<Driver>> future = promise.GetFuture();
      promise.Fail(detail::MissingAccountId());
      return future;
    }
    const ::openpit::param::AccountId pinned = *accountId;
    AsyncEngine<Driver>* engine = m_engine.get();
    return m_engine->Call(
        pinned,
        [engine, pinned, order = std::move(order)](Driver& driver) mutable {
          ::openpit::pretrade::ExecuteResult result =
              driver.ExecutePreTrade(std::move(order));
          if (!result.Passed()) {
            return ExecuteOutcome<Driver>{nullptr, std::move(result.rejects)};
          }
          auto reservation = std::make_shared<AsyncReservation<Driver>>(
              std::move(*result.reservation), engine, pinned);
          return ExecuteOutcome<Driver>{std::move(reservation), {}};
        },
        timeout);
  }

  template <
      typename OrderT,
      std::enable_if_t<
          std::is_base_of_v<::openpit::Order, std::decay_t<OrderT>>, int> = 0>
  [[nodiscard]] Future<ExecuteOutcome<Driver>> ExecutePreTrade(
      OrderT&& order,
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    return ExecutePreTrade(
        ::openpit::detail::OwnExactPolymorphic<::openpit::Order>(
            std::forward<OrderT>(order),
            "ExecutePreTrade cannot own a base-typed reference to a derived "
            "order; pass std::unique_ptr<const openpit::Order> instead"),
        timeout);
  }

  // Enqueues a drop-copy call for `order`, pinned to its account. The future
  // resolves with a `DropCopyOutcome`: a non-null operation on accept,
  // populated rejects on a fatal evaluation failure. Resolves immediately with
  // `MissingAccountId` when the order carries no readable account id: drop copy
  // requires one, and the engine rejects an unreadable one with
  // `MissingRequiredField` before any policy runs, so there is nothing to gain
  // by queueing such an order.
  //
  // `timeout` bounds only the wait for queue space. Before enqueue, a positive
  // timeout can fail with `ErrorCode::SubmitCancelled`, and a dynamic strategy
  // can fail with `ErrorCode::QueueLimit`. A hard stop can fail an enqueued or
  // waiting call with `ErrorCode::Stopped`. The drop-copy fact is then
  // unregistered, so the caller MUST retry.
  //
  // This overload always transfers ownership. Whenever a retry may be required,
  // retain the upstream payload or an independent copy of the exact concrete
  // order before submission; otherwise a failure consumes the only payload.
  [[nodiscard]] Future<DropCopyOutcome<Driver>> ApplyDropCopy(
      std::unique_ptr<const ::openpit::Order> order,
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    if (!order) {
      throw ::openpit::Error("ApplyDropCopy requires a non-null order");
    }
    const std::optional<::openpit::param::AccountId> accountId =
        detail::OrderAccountId(*order);
    if (!accountId.has_value()) {
      Promise<DropCopyOutcome<Driver>> promise;
      Future<DropCopyOutcome<Driver>> future = promise.GetFuture();
      promise.Fail(detail::MissingAccountId());
      return future;
    }
    const ::openpit::param::AccountId pinned = *accountId;
    AsyncEngine<Driver>* engine = m_engine.get();
    return m_engine->Call(
        pinned,
        [engine, pinned, order = std::move(order)](Driver& driver) mutable {
          ::openpit::pretrade::DropCopyResult result =
              driver.ApplyDropCopy(std::move(order));
          if (!result.Passed()) {
            return DropCopyOutcome<Driver>{nullptr, std::move(result.rejects)};
          }
          auto operation = std::make_shared<AsyncDropCopyOperation<Driver>>(
              std::move(*result.operation), engine, pinned);
          return DropCopyOutcome<Driver>{std::move(operation), {}};
        },
        timeout);
  }

  // A non-const rvalue order is moved; an lvalue or const rvalue is copied.
  // Pass an lvalue to retain a retry payload. A named const object cast to an
  // rvalue is copied, but only the named original survives for retry; an
  // unnamed const rvalue temporary does not leave a caller-owned payload.
  //
  // The decayed static type of `order` must be a concrete subclass of
  // `openpit::Order` and match the dynamic type. A payload whose decayed static
  // type is exactly `openpit::Order` is rejected at compile time. A reference
  // whose static type is a concrete intermediate base such as
  // `openpit::model::Order`, but whose dynamic type is more derived, throws
  // `openpit::Error` synchronously before ownership transfer. Type-erased
  // callers must use the
  // `std::unique_ptr<const openpit::Order>` overload and retain an upstream
  // payload or independent concrete copy before transferring ownership.
  //
  // This overload has the same mandatory retry contract for
  // `SubmitCancelled`, `QueueLimit`, and `Stopped` as the pointer overload.
  template <
      typename OrderT,
      std::enable_if_t<
          std::is_base_of_v<::openpit::Order, std::decay_t<OrderT>>, int> = 0>
  [[nodiscard]] Future<DropCopyOutcome<Driver>> ApplyDropCopy(
      OrderT&& order,
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    return ApplyDropCopy(
        ::openpit::detail::OwnExactPolymorphic<::openpit::Order>(
            std::forward<OrderT>(order),
            "ApplyDropCopy cannot own a base-typed reference to a derived "
            "order; pass std::unique_ptr<const openpit::Order> instead"),
        timeout);
  }

  // Enqueues a post-trade call for `report`, pinned to its account. Resolves
  // with the `PostTradeResult`, or immediately with `MissingAccountId` when the
  // report carries no account id.
  //
  // `timeout` bounds only the wait for queue space. Before enqueue, a positive
  // timeout can fail with `ErrorCode::SubmitCancelled`, and a dynamic strategy
  // can fail with `ErrorCode::QueueLimit`. A hard stop can fail an enqueued or
  // waiting call with `ErrorCode::Stopped`. An execution that already occurred
  // then remains unregistered: its reservation is not released and its spot
  // funds are not settled. The caller MUST retry.
  //
  // This overload always transfers ownership. Whenever a retry may be required,
  // retain the upstream message or an independent copy of the exact concrete
  // report before submission; otherwise a failure consumes the only payload.
  [[nodiscard]] Future<::openpit::PostTradeResult> ApplyExecutionReport(
      std::unique_ptr<const ::openpit::ExecutionReport> report,
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    if (!report) {
      throw ::openpit::Error(
          "ApplyExecutionReport requires a non-null execution report");
    }
    const std::optional<::openpit::param::AccountId> accountId =
        detail::ReportAccountId(*report);
    if (!accountId.has_value()) {
      Promise<::openpit::PostTradeResult> promise;
      Future<::openpit::PostTradeResult> future = promise.GetFuture();
      promise.Fail(detail::MissingAccountId());
      return future;
    }
    return m_engine->Call(
        *accountId,
        [report = std::move(report)](Driver& driver) mutable {
          return driver.ApplyExecutionReport(std::move(report));
        },
        timeout);
  }

  // A non-const rvalue report is moved; an lvalue or const rvalue is copied.
  // Copying deep-clones the pre-trade lock through the C ABI when the fill
  // carries one, so it can throw. Pass an lvalue to retain a retry payload. A
  // named const object cast to an rvalue is copied, but only the named original
  // survives for retry; an unnamed const rvalue temporary does not leave a
  // caller-owned payload.
  //
  // The decayed static type of `report` must be a concrete subclass of
  // `openpit::ExecutionReport` and match the dynamic type. A payload whose
  // decayed static type is exactly `openpit::ExecutionReport` is rejected at
  // compile time. A reference whose static type is the concrete intermediate
  // base `openpit::model::ExecutionReport`, but whose dynamic type is more
  // derived, throws `openpit::Error` synchronously before this call creates a
  // `Future`. The check precedes the copy or move, leaving the caller's report
  // unchanged. Type-erased callers must use the
  // `std::unique_ptr<const openpit::ExecutionReport>` overload and retain the
  // upstream message or an independent concrete copy before ownership transfer.
  //
  // This overload has the same mandatory retry contract for
  // `SubmitCancelled`, `QueueLimit`, and `Stopped` as the pointer overload.
  template <typename ExecutionReportT,
            std::enable_if_t<std::is_base_of_v<::openpit::ExecutionReport,
                                               std::decay_t<ExecutionReportT>>,
                             int> = 0>
  [[nodiscard]] Future<::openpit::PostTradeResult> ApplyExecutionReport(
      ExecutionReportT&& report,
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    return ApplyExecutionReport(
        ::openpit::detail::OwnExactPolymorphic<::openpit::ExecutionReport>(
            std::forward<ExecutionReportT>(report),
            "ApplyExecutionReport cannot own a base-typed reference to a "
            "derived execution report; pass std::unique_ptr<const "
            "openpit::ExecutionReport> instead"),
        timeout);
  }

  // Enqueues a batch adjustment for `accountId` (supplied explicitly because
  // adjustments carry no account). Resolves with an `AdjustmentOutcome`: a
  // non-null batch error on reject, or outcomes and account blocks on accept.
  //
  // `timeout` bounds only the wait for queue space. A positive timeout can
  // return `ErrorCode::SubmitCancelled` before enqueue, leaving an adjustment
  // for an already-occurred fact unregistered. The caller MUST retry.
  // `adjustments` is by value: passing an lvalue copies it and preserves the
  // caller's batch for retry. Passing `std::move(...)` consumes it, so it
  // cannot be resubmitted after `SubmitCancelled`, `QueueLimit`, or `Stopped`.
  // The default non-positive timeout waits indefinitely; a hard stop returns
  // `ErrorCode::Stopped`. A hard stop can abort an enqueued task before it
  // starts, so a non-positive timeout does not remove the need to retain the
  // batch.
  template <typename Adjustment>
  [[nodiscard]] Future<AdjustmentOutcome> ApplyAccountAdjustment(
      ::openpit::param::AccountId accountId,
      std::vector<Adjustment> adjustments,
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    return m_engine->Call(
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
    return AsyncAccounts<Driver>(m_engine.get());
  }

  //----------------------------------------------------------------------------
  // Caller-owned work

  // Enqueues an arbitrary closure into the queue for `accountId`. Mirrors the
  // generic `AsyncEngine::Submit`.
  [[nodiscard]] Future<std::monostate> Submit(
      ::openpit::param::AccountId accountId, std::function<void()> fn,
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    return m_engine->Submit(accountId, std::move(fn), timeout);
  }

  //----------------------------------------------------------------------------
  // Lifecycle

  [[nodiscard]] bool StopGraceful(
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    return m_engine->StopGraceful(timeout);
  }

  [[nodiscard]] bool StopHard(
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    return m_engine->StopHard(timeout);
  }

  // The underlying generic engine, for the rare case a caller wants the generic
  // `Call`/`Call2` seam alongside the typed methods.
  [[nodiscard]] AsyncEngine<Driver>& Generic() noexcept { return *m_engine; }

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
  friend class AsyncDropCopyOperation;
  template <typename D>
  friend class AsyncAccounts;

  explicit TypedAsyncEngine(AsyncEngine<Driver> engine)
      : m_engine(std::make_unique<AsyncEngine<Driver>>(std::move(engine))) {}

  std::unique_ptr<AsyncEngine<Driver>> m_engine;
};

//------------------------------------------------------------------------------
// AsyncRequest / AsyncReservation / AsyncDropCopyOperation / AsyncAccounts
// out-of-line definitions

template <typename Driver>
template <typename Op>
[[nodiscard]] Future<std::monostate> AsyncReservation<Driver>::Run(
    Op op, std::chrono::nanoseconds timeout, bool abortCloses) {
  auto self = this->shared_from_this();
  if (!abortCloses) {
    return m_engine->Submit(
        m_accountId,
        [self, op = std::move(op)]() {
          std::lock_guard<std::mutex> lock(self->m_mutex);
          op(self->m_reservation);
        },
        timeout);
  }

  AsyncEngine<Driver>* engine = m_engine;
  const ::openpit::param::AccountId accountId = m_accountId;
  return m_engine->Submit(
      accountId,
      [self, op = std::move(op)]() {
        std::lock_guard<std::mutex> lock(self->m_mutex);
        op(self->m_reservation);
      },
      [self, engine, accountId](Promise<std::monostate> promise, Error error,
                                bool inAccountLane) mutable {
        auto cleanup = [self, promise, error = std::move(error)]() mutable {
          detail::CompleteMandatoryCleanup(promise, std::move(error), [self] {
            std::lock_guard<std::mutex> lock(self->m_mutex);
            self->m_reservation = ::openpit::pretrade::Reservation();
          });
        };
        if (inAccountLane) {
          cleanup();
          return;
        }
        detail::DetachMandatoryCleanup(*engine, accountId, std::move(cleanup));
      },
      timeout);
}

template <typename Driver>
template <typename Op>
[[nodiscard]] Future<std::monostate> AsyncDropCopyOperation<Driver>::Run(
    Op op, std::chrono::nanoseconds timeout, bool abortCloses) {
  auto self = this->shared_from_this();
  if (!abortCloses) {
    return m_engine->Submit(
        m_accountId,
        [self, op = std::move(op)]() {
          std::lock_guard<std::mutex> lock(self->m_mutex);
          op(self->m_operation);
        },
        timeout);
  }

  AsyncEngine<Driver>* engine = m_engine;
  const ::openpit::param::AccountId accountId = m_accountId;
  return m_engine->Submit(
      accountId,
      [self, op = std::move(op)]() {
        std::lock_guard<std::mutex> lock(self->m_mutex);
        op(self->m_operation);
      },
      [self, engine, accountId](Promise<std::monostate> promise, Error error,
                                bool inAccountLane) mutable {
        auto cleanup = [self, promise, error = std::move(error)]() mutable {
          detail::CompleteMandatoryCleanup(promise, std::move(error), [self] {
            std::lock_guard<std::mutex> lock(self->m_mutex);
            self->m_operation = ::openpit::pretrade::DropCopyOperation();
          });
        };
        if (inAccountLane) {
          cleanup();
          return;
        }
        detail::DetachMandatoryCleanup(*engine, accountId, std::move(cleanup));
      },
      timeout);
}

template <typename Driver>
[[nodiscard]] PairFuture<std::shared_ptr<AsyncReservation<Driver>>,
                         std::vector<::openpit::pretrade::Reject>>
AsyncRequest<Driver>::Execute(std::chrono::nanoseconds timeout) {
  using ReservationPtr = std::shared_ptr<AsyncReservation<Driver>>;
  using Rejects = std::vector<::openpit::pretrade::Reject>;
  auto self = this->shared_from_this();
  AsyncEngine<Driver>* engine = m_engine;
  const ::openpit::param::AccountId accountId = m_accountId;
  return m_engine->template Call2<ReservationPtr, Rejects>(
      accountId,
      [self](Driver&) -> std::pair<ReservationPtr, Rejects> {
        std::lock_guard<std::mutex> lock(self->m_mutex);
        ::openpit::pretrade::ExecuteResult result;
        try {
          result = self->m_request.Execute();
        } catch (...) {
          self->m_request = ::openpit::pretrade::Request();
          throw;
        }
        self->m_request = ::openpit::pretrade::Request();
        if (!result.Passed()) {
          return {nullptr, std::move(result.rejects)};
        }
        auto reservation = std::make_shared<AsyncReservation<Driver>>(
            std::move(*result.reservation), self->m_engine, self->m_accountId);
        return {std::move(reservation), Rejects{}};
      },
      [self, engine, accountId](PairPromise<ReservationPtr, Rejects> promise,
                                Error error, bool inAccountLane) mutable {
        auto cleanup = [self, promise, error = std::move(error)]() mutable {
          detail::CompleteMandatoryCleanup(promise, std::move(error), [self] {
            std::lock_guard<std::mutex> lock(self->m_mutex);
            self->m_request = ::openpit::pretrade::Request();
          });
        };
        if (inAccountLane) {
          cleanup();
          return;
        }
        detail::DetachMandatoryCleanup(*engine, accountId, std::move(cleanup));
      },
      timeout);
}

template <typename Driver>
[[nodiscard]] Future<std::monostate> AsyncRequest<Driver>::Close(
    std::chrono::nanoseconds timeout) {
  auto self = this->shared_from_this();
  AsyncEngine<Driver>* engine = m_engine;
  const ::openpit::param::AccountId accountId = m_accountId;
  return m_engine->Submit(
      accountId,
      [self]() {
        std::lock_guard<std::mutex> lock(self->m_mutex);
        self->m_request = ::openpit::pretrade::Request();
      },
      [self, engine, accountId](Promise<std::monostate> promise, Error error,
                                bool inAccountLane) mutable {
        auto cleanup = [self, promise, error = std::move(error)]() mutable {
          detail::CompleteMandatoryCleanup(promise, std::move(error), [self] {
            std::lock_guard<std::mutex> lock(self->m_mutex);
            self->m_request = ::openpit::pretrade::Request();
          });
        };
        if (inAccountLane) {
          cleanup();
          return;
        }
        detail::DetachMandatoryCleanup(*engine, accountId, std::move(cleanup));
      },
      timeout);
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
  return m_engine->Call(
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
  return m_engine->Call(
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
  return m_engine->Call(
      account,
      [account](Driver& driver) { return driver.Accounts().GroupOf(account); },
      timeout);
}

template <typename Driver>
[[nodiscard]] Future<std::monostate> AsyncAccounts<Driver>::Block(
    ::openpit::param::AccountId account, std::string reason,
    std::chrono::nanoseconds timeout) {
  AsyncEngine<Driver>* engine = m_engine;
  return engine->Submit(
      account,
      [engine, account, reason = std::move(reason)]() {
        engine->DriverRef().Accounts().Block(account, reason);
      },
      timeout);
}

template <typename Driver>
[[nodiscard]] Future<std::monostate> AsyncAccounts<Driver>::Unblock(
    ::openpit::param::AccountId account, std::chrono::nanoseconds timeout) {
  AsyncEngine<Driver>* engine = m_engine;
  return engine->Submit(
      account,
      [engine, account]() { engine->DriverRef().Accounts().Unblock(account); },
      timeout);
}

template <typename Driver>
[[nodiscard]] Future<std::monostate> AsyncAccounts<Driver>::UnblockAll(
    std::chrono::nanoseconds timeout) {
  AsyncEngine<Driver>* engine = m_engine;
  return engine->Submit(
      EngineWideRoutingKey(),
      [engine]() { engine->DriverRef().Accounts().UnblockAll(); }, timeout);
}

template <typename Driver>
[[nodiscard]] Future<std::optional<::openpit::accounts::AccountBlockError>>
AsyncAccounts<Driver>::ReplaceBlockReason(::openpit::param::AccountId account,
                                          std::string reason,
                                          std::chrono::nanoseconds timeout) {
  return m_engine->Call(
      account,
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
  return m_engine->Call(
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
  return m_engine->Call(
      GroupRoutingKey(group),
      [group](Driver& driver) { return driver.Accounts().UnblockGroup(group); },
      timeout);
}

template <typename Driver>
[[nodiscard]] Future<std::optional<::openpit::accounts::AccountBlockError>>
AsyncAccounts<Driver>::ReplaceGroupBlockReason(
    ::openpit::param::AccountGroupId group, std::string reason,
    std::chrono::nanoseconds timeout) {
  return m_engine->Call(
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
// separate driver lifetime for the common `openpit::Engine` case. It does not
// own the source engine, whose stable-address requirement still applies.
class OwnedTypedAsyncEngine {
 public:
  using Driver = EngineAdapter;

  OwnedTypedAsyncEngine(const OwnedTypedAsyncEngine&) = delete;
  OwnedTypedAsyncEngine& operator=(const OwnedTypedAsyncEngine&) = delete;
  OwnedTypedAsyncEngine(OwnedTypedAsyncEngine&&) noexcept = default;
  OwnedTypedAsyncEngine& operator=(OwnedTypedAsyncEngine&&) = delete;
  ~OwnedTypedAsyncEngine() = default;

  [[nodiscard]] Future<StartOutcome<Driver>> StartPreTrade(
      std::unique_ptr<const ::openpit::Order> order,
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    return m_engine.StartPreTrade(std::move(order), timeout);
  }

  template <
      typename OrderT,
      std::enable_if_t<
          std::is_base_of_v<::openpit::Order, std::decay_t<OrderT>>, int> = 0>
  [[nodiscard]] Future<StartOutcome<Driver>> StartPreTrade(
      OrderT&& order,
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    return m_engine.StartPreTrade(std::forward<OrderT>(order), timeout);
  }

  [[nodiscard]] Future<ExecuteOutcome<Driver>> ExecutePreTrade(
      std::unique_ptr<const ::openpit::Order> order,
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    return m_engine.ExecutePreTrade(std::move(order), timeout);
  }

  template <
      typename OrderT,
      std::enable_if_t<
          std::is_base_of_v<::openpit::Order, std::decay_t<OrderT>>, int> = 0>
  [[nodiscard]] Future<ExecuteOutcome<Driver>> ExecutePreTrade(
      OrderT&& order,
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    return m_engine.ExecutePreTrade(std::forward<OrderT>(order), timeout);
  }

  // Transfers ownership of `order`. On `SubmitCancelled`, `QueueLimit`, or
  // `Stopped`, the caller MUST retry from a retained upstream payload or an
  // independent copy of the exact concrete order.
  [[nodiscard]] Future<DropCopyOutcome<Driver>> ApplyDropCopy(
      std::unique_ptr<const ::openpit::Order> order,
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    return m_engine.ApplyDropCopy(std::move(order), timeout);
  }

  // A non-const rvalue order is moved; an lvalue or const rvalue is copied.
  // Pass an lvalue to retain a retry payload. An unnamed const rvalue temporary
  // leaves no caller-owned payload. A payload whose decayed static type is
  // exactly `openpit::Order` is rejected at compile time. A reference whose
  // static type is a concrete intermediate base such as
  // `openpit::model::Order`, but whose dynamic type is more derived, throws
  // `openpit::Error` synchronously. Type-erased callers use the pointer
  // overload and retain an independent payload before transferring ownership.
  // On `SubmitCancelled`, `QueueLimit`, or `Stopped`, the caller MUST retry.
  template <
      typename OrderT,
      std::enable_if_t<
          std::is_base_of_v<::openpit::Order, std::decay_t<OrderT>>, int> = 0>
  [[nodiscard]] Future<DropCopyOutcome<Driver>> ApplyDropCopy(
      OrderT&& order,
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    return m_engine.ApplyDropCopy(std::forward<OrderT>(order), timeout);
  }

  // Transfers ownership of `report`. On `SubmitCancelled`, `QueueLimit`, or
  // `Stopped`, the caller MUST retry from a retained upstream message or an
  // independent copy of the exact concrete report.
  [[nodiscard]] Future<::openpit::PostTradeResult> ApplyExecutionReport(
      std::unique_ptr<const ::openpit::ExecutionReport> report,
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    return m_engine.ApplyExecutionReport(std::move(report), timeout);
  }

  // A non-const rvalue report is moved; an lvalue or const rvalue is copied.
  // Copying deep-clones a fill's pre-trade lock and can throw. Pass an lvalue
  // to retain a retry payload. A named const object cast to an rvalue is
  // copied, but only the named original survives for retry; an unnamed const
  // rvalue temporary leaves no caller-owned payload.
  //
  // A payload whose decayed static type is exactly `openpit::ExecutionReport`
  // is rejected at compile time. A reference whose static type is the concrete
  // intermediate base `openpit::model::ExecutionReport`, but whose dynamic type
  // is more derived, throws `openpit::Error` synchronously before ownership
  // transfer. Type-erased callers must use the pointer overload and retain an
  // upstream message or independent concrete copy before transfer.
  //
  // On `SubmitCancelled`, `QueueLimit`, or `Stopped`, the caller MUST retry.
  template <typename ExecutionReportT,
            std::enable_if_t<std::is_base_of_v<::openpit::ExecutionReport,
                                               std::decay_t<ExecutionReportT>>,
                             int> = 0>
  [[nodiscard]] Future<::openpit::PostTradeResult> ApplyExecutionReport(
      ExecutionReportT&& report,
      std::chrono::nanoseconds timeout = std::chrono::nanoseconds(0)) {
    return m_engine.ApplyExecutionReport(std::forward<ExecutionReportT>(report),
                                         timeout);
  }

  // `adjustments` is by value: passing an lvalue copies it and keeps the
  // caller's batch available, while passing an rvalue consumes it. The local
  // batch is moved into the queued task. On `SubmitCancelled`, `QueueLimit`,
  // or `Stopped`, the caller MUST retry from a retained batch.
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

  OwnedTypedAsyncEngine(std::unique_ptr<Driver> driver, std::size_t workers)
      : m_driver(std::move(driver)),
        m_engine(TypedBuilder<Driver>(*m_driver).Sharded(workers).Build()) {}

  std::unique_ptr<Driver> m_driver;
  TypedAsyncEngine<Driver> m_engine;
};

/// \brief Builds a sharded typed async engine over `openpit::Engine`.
//
// Shortcut for the common production path. The returned wrapper owns the
// `EngineAdapter`; the same source engine object must outlive the async wrapper
// at its original address and must not be moved. Rvalue engines are rejected at
// compile time.
[[nodiscard]] inline OwnedTypedAsyncEngine MakeTypedAsyncEngine(
    const ::openpit::Engine& engine, std::size_t workers) {
  auto driver = std::make_unique<EngineAdapter>(engine);
  return OwnedTypedAsyncEngine(std::move(driver), workers);
}

OwnedTypedAsyncEngine MakeTypedAsyncEngine(::openpit::Engine&&,
                                           std::size_t) = delete;
OwnedTypedAsyncEngine MakeTypedAsyncEngine(const ::openpit::Engine&&,
                                           std::size_t) = delete;

}  // namespace openpit::asyncengine
