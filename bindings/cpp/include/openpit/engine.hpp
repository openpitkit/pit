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
#include "openpit/detail/callback_error.hpp"
#include "openpit/detail/handle.hpp"
#include "openpit/error.hpp"
#include "openpit/model.hpp"
#include "openpit/pretrade/pre_trade_lock.hpp"
#include "openpit/reject.hpp"
#include "openpit/string.hpp"

#include <openpit.h>

#include <cstddef>
#include <cstdint>
#include <memory>
#include <optional>
#include <string>
#include <string_view>
#include <type_traits>
#include <utility>
#include <vector>

// Engine surface.
//
// Covers the create-builder -> build -> destroy lifecycle, the pre-trade policy
// registration hook (`Add` for built-in configs and custom policy wrappers),
// and the runtime engine operations: the pre-trade
// pipeline (`StartPreTrade` / `ExecutePreTrade` and the deferred `Request` /
// `Reservation` it yields), `ApplyExecutionReport`, `ApplyAccountAdjustment`,
// and the `Accounts()` admin handle (account groups + account/group blocking).
//
// Error model: runtime boundary failures throw `openpit::Error`; expected
// pre-trade rejects and account-op outcomes are value types, never thrown.

namespace openpit {

class Configurator;
class EngineBuilder;

namespace detail {

template <typename>
struct DependentFalse : std::false_type {};

template <typename T, typename = void>
struct HasAddTo : std::false_type {};

template <typename T>
struct HasAddTo<T, std::void_t<decltype(std::declval<const T&>().AddTo(
                       std::declval<EngineBuilder&>()))>> : std::true_type {};

template <typename T, typename = void>
struct HasGet : std::false_type {};

template <typename T>
struct HasGet<T, std::void_t<decltype(std::declval<const T&>().Get())>>
    : std::true_type {};

// Drains a caller-owned reject list into owned `Reject` value types, then
// releases the list. The list pointer must be non-null.
[[nodiscard]] inline std::vector<::openpit::pretrade::Reject> DrainRejectList(
    OpenPitPretradeRejectList* list) {
  std::vector<::openpit::pretrade::Reject> rejects;
  const std::size_t count = openpit_pretrade_reject_list_len(list);
  rejects.reserve(count);
  for (std::size_t i = 0; i < count; ++i) {
    OpenPitPretradeReject raw{};
    if (openpit_pretrade_reject_list_get(list, i, &raw)) {
      rejects.push_back(::openpit::pretrade::Reject::FromRaw(raw));
    }
  }
  openpit_pretrade_destroy_reject_list(list);
  return rejects;
}

// Drains a caller-owned account-block list into owned `AccountBlock` values,
// then releases the list. The list pointer must be non-null.
[[nodiscard]] inline std::vector<::openpit::accounts::AccountBlock>
DrainAccountBlockList(OpenPitPretradeAccountBlockList* list) {
  std::vector<::openpit::accounts::AccountBlock> blocks;
  const std::size_t count = openpit_pretrade_account_block_list_len(list);
  blocks.reserve(count);
  for (std::size_t index = 0; index < count; ++index) {
    OpenPitPretradeAccountBlock raw{};
    if (openpit_pretrade_account_block_list_get(list, index, &raw)) {
      blocks.push_back(::openpit::accounts::AccountBlock::FromRaw(raw));
    }
  }
  openpit_pretrade_destroy_account_block_list(list);
  return blocks;
}

}  // namespace detail

// Storage synchronization policy selected at builder time. Mirrors
// `OpenPitSyncPolicy`.
enum class SyncPolicy : std::uint8_t {
  // Single-threaded: the engine must stay on its creating thread.
  None = OpenPitSyncPolicy_None,
  // Fully synchronized: concurrent calls on one handle are safe.
  Full = OpenPitSyncPolicy_Full,
  // Account-sharded: sequential cross-thread access with per-account pinning.
  Account = OpenPitSyncPolicy_Account,
};

// Machine-readable category of a domain engine-build failure. Mirrors
// `OpenPitEngineBuildErrorCode`.
enum class EngineBuildErrorCode : std::uint8_t {
  DuplicatePolicyName = OpenPitEngineBuildErrorCode_DuplicatePolicyName,
  DuplicatePolicyGroupId = OpenPitEngineBuildErrorCode_DuplicatePolicyGroupId,
  Other = OpenPitEngineBuildErrorCode_Other,
};

// Structured error thrown when engine construction fails its configuration
// validation (duplicate policy name / group id). Boundary failures (null
// builder, already consumed, no policies registered) throw a plain
// `openpit::Error` instead.
class EngineBuildError : public Error {
 public:
  EngineBuildError(std::string message, EngineBuildErrorCode code,
                   std::string policyName, std::uint16_t policyGroupId)
      : Error(std::move(message)),
        m_code(code),
        m_policyName(std::move(policyName)),
        m_policyGroupId(policyGroupId) {}

  [[nodiscard]] EngineBuildErrorCode Code() const noexcept { return m_code; }

  // Offending policy name; set only for the duplicate-policy-name category.
  [[nodiscard]] const std::string& PolicyName() const noexcept {
    return m_policyName;
  }

  // Offending policy group id; set only for the duplicate-policy-group-id
  // category.
  [[nodiscard]] std::uint16_t PolicyGroupId() const noexcept {
    return m_policyGroupId;
  }

 private:
  EngineBuildErrorCode m_code;
  std::string m_policyName;
  std::uint16_t m_policyGroupId;
};

namespace detail {

struct EngineDeleter {
  void operator()(OpenPitEngine* handle) const noexcept {
    openpit_destroy_engine(handle);
  }
};

struct EngineBuilderDeleter {
  void operator()(OpenPitEngineBuilder* handle) const noexcept {
    openpit_destroy_engine_builder(handle);
  }
};

struct PostTradeResultDeleter {
  void operator()(OpenPitPostTradeResult* handle) const noexcept {
    openpit_destroy_post_trade_result(handle);
  }
};

}  // namespace detail

namespace pretrade {

namespace detail {

struct PreTradeRequestDeleter {
  void operator()(OpenPitPretradePreTradeRequest* handle) const noexcept {
    openpit_destroy_pretrade_pre_trade_request(handle);
  }
};

struct PreTradeReservationDeleter {
  void operator()(OpenPitPretradePreTradeReservation* handle) const noexcept {
    // An unresolved reservation rolls back during destruction. Destructors
    // cannot report user callback failures, so suppress only that rollback's
    // captured exception; explicit Rollback() reports it to the caller.
    ::openpit::detail::ClearPendingCallbackException();
    openpit_destroy_pretrade_pre_trade_reservation(handle);
    ::openpit::detail::ClearPendingCallbackException();
  }
};

struct PreTradeDryRunReportDeleter {
  void operator()(OpenPitPretradePreTradeDryRunReport* handle) const noexcept {
    openpit_destroy_pretrade_pre_trade_dry_run_report(handle);
  }
};

}  // namespace detail

// Outcome of running the full pre-trade pipeline (`ExecutePreTrade` or
// `Request::Execute`): exactly one channel is populated. A `reservation` means
// the order passed and reserved state awaits resolution; a non-empty `rejects`
// means it was rejected. runtime failures throw instead of producing this
// value.
struct ExecuteResult;

// Reserved-but-not-finalized pre-trade state. Move-only RAII: resolve it
// exactly once with `Commit()` or `Rollback()`; destruction rolls back any
// still-pending mutations. Both resolutions are idempotent at the pointer
// level.
class Reservation {
 public:
  Reservation() = default;

  explicit Reservation(OpenPitPretradePreTradeReservation* handle) noexcept
      : m_handle(handle) {}

  [[nodiscard]] explicit operator bool() const noexcept {
    return static_cast<bool>(m_handle);
  }

  // Finalizes the reservation, applying the reserved state permanently.
  void Commit() {
    ::openpit::detail::ClearPendingCallbackException();
    openpit_pretrade_pre_trade_reservation_commit(m_handle.Get());
    ::openpit::detail::ThrowIfPendingCallbackException(
        "pre-trade mutation commit callback failed");
  }

  // Cancels the reservation, releasing the reserved state.
  void Rollback() {
    ::openpit::detail::ClearPendingCallbackException();
    openpit_pretrade_pre_trade_reservation_rollback(m_handle.Get());
    ::openpit::detail::ThrowIfPendingCallbackException(
        "pre-trade mutation rollback callback failed");
  }

  // Returns an owned lock snapshot detached from the reservation state.
  [[nodiscard]] PreTradeLock Lock() const {
    return PreTradeLock(
        openpit_pretrade_pre_trade_reservation_get_lock(m_handle.Get()));
  }

  [[nodiscard]] OpenPitPretradePreTradeReservation* Get() const noexcept {
    return m_handle.Get();
  }

 private:
  ::openpit::detail::Handle<OpenPitPretradePreTradeReservation,
                            detail::PreTradeReservationDeleter>
      m_handle;
};

// Deferred pre-trade request returned by `StartPreTrade`. Move-only RAII:
// `Execute()` runs the remaining stages once; destruction abandons an
// unexecuted request without creating a reservation.
class Request {
 public:
  Request() = default;

  explicit Request(
      OpenPitPretradePreTradeRequest* handle,
      std::shared_ptr<const ::openpit::Order> order = nullptr) noexcept
      : m_handle(handle), m_order(std::move(order)) {}

  [[nodiscard]] explicit operator bool() const noexcept {
    return static_cast<bool>(m_handle);
  }

  // Defined out-of-line below `ExecuteResult`.
  [[nodiscard]] ExecuteResult Execute();

  [[nodiscard]] OpenPitPretradePreTradeRequest* Get() const noexcept {
    return m_handle.Get();
  }

 private:
  ::openpit::detail::Handle<OpenPitPretradePreTradeRequest,
                            detail::PreTradeRequestDeleter>
      m_handle;
  // Owned polymorphic order this request was started from, so the deferred main
  // stage can recover the client order type without depending on caller
  // lifetime. Empty only for a default-constructed request.
  std::shared_ptr<const ::openpit::Order> m_order;
};

struct ExecuteResult {
  std::optional<Reservation> reservation;
  std::vector<::openpit::pretrade::Reject> rejects;

  // Whether the order passed and a reservation is available.
  [[nodiscard]] bool Passed() const noexcept { return reservation.has_value(); }
};

// Owning dry-run report. A dry-run never mutates engine state; the report is a
// detached snapshot of the verdict and any would-be lock, account adjustments,
// and account blocks.
class DryRunReport {
 public:
  DryRunReport() = default;

  explicit DryRunReport(OpenPitPretradePreTradeDryRunReport* handle) noexcept
      : m_handle(handle) {}

  [[nodiscard]] explicit operator bool() const noexcept {
    return static_cast<bool>(m_handle);
  }

  [[nodiscard]] bool Passed() const noexcept {
    return openpit_pretrade_pre_trade_dry_run_report_is_pass(m_handle.Get());
  }

  [[nodiscard]] std::vector<::openpit::pretrade::Reject> Rejects() const {
    return ::openpit::detail::DrainRejectList(
        openpit_pretrade_pre_trade_dry_run_report_get_rejects(m_handle.Get()));
  }

  [[nodiscard]] ::openpit::pretrade::PreTradeLock Lock() const {
    return ::openpit::pretrade::PreTradeLock(
        openpit_pretrade_pre_trade_dry_run_report_get_lock(m_handle.Get()));
  }

  [[nodiscard]] std::vector<::openpit::accountadjustment::Outcome>
  AccountAdjustments() const {
    const ::openpit::accountadjustment::OutcomeList outcomes(
        openpit_pretrade_pre_trade_dry_run_report_get_account_adjustments(
            m_handle.Get()));
    return outcomes.ToVector();
  }

  [[nodiscard]] std::vector<::openpit::accounts::AccountBlock> AccountBlocks()
      const {
    OpenPitPretradeAccountBlockList* blocks =
        openpit_pretrade_pre_trade_dry_run_report_get_account_block(
            m_handle.Get());
    std::vector<::openpit::accounts::AccountBlock> out;
    if (blocks == nullptr) {
      return out;
    }
    const std::size_t count = openpit_pretrade_account_block_list_len(blocks);
    out.reserve(count);
    for (std::size_t index = 0; index < count; ++index) {
      OpenPitPretradeAccountBlock block{};
      if (openpit_pretrade_account_block_list_get(blocks, index, &block)) {
        out.push_back(::openpit::accounts::AccountBlock::FromRaw(block));
      }
    }
    openpit_pretrade_destroy_account_block_list(blocks);
    return out;
  }

  [[nodiscard]] OpenPitPretradePreTradeDryRunReport* Get() const noexcept {
    return m_handle.Get();
  }

 private:
  ::openpit::detail::Handle<OpenPitPretradePreTradeDryRunReport,
                            detail::PreTradeDryRunReportDeleter>
      m_handle;
};

// Outcome of `StartPreTrade`: a `request` means the order passed the start
// stage and can proceed via `Request::Execute`; a non-empty `rejects` means it
// was rejected at the start stage.
struct StartResult {
  std::optional<Request> request;
  std::vector<::openpit::pretrade::Reject> rejects;

  [[nodiscard]] bool Passed() const noexcept { return request.has_value(); }
};

[[nodiscard]] inline ExecuteResult Request::Execute() {
  OpenPitPretradePreTradeReservation* reservation = nullptr;
  OpenPitPretradeRejectList* rejects = nullptr;
  OpenPitSharedString* error = nullptr;
  // Re-establish the original order for the deferred main stage so a custom
  // policy still recovers the client order type; harmless when null.
  std::optional<::openpit::detail::CurrentOrderGuard> orderGuard;
  if (m_order != nullptr) {
    orderGuard.emplace(*m_order);
  }
  ::openpit::detail::ClearPendingCallbackException();
  const OpenPitPretradeStatus status =
      openpit_pretrade_pre_trade_request_execute(m_handle.Get(), &reservation,
                                                 &rejects, &error);
  if (::openpit::detail::HasPendingCallbackException()) {
    openpit_destroy_pretrade_pre_trade_reservation(reservation);
    openpit_pretrade_destroy_reject_list(rejects);
    openpit_destroy_shared_string(error);
  }
  ::openpit::detail::ThrowIfPendingCallbackException(
      "openpit_pretrade_pre_trade_request_execute callback failed");
  if (status == OpenPitPretradeStatus_Error) {
    ::openpit::detail::ThrowFromSharedString(
        error, "openpit_pretrade_pre_trade_request_execute failed");
  }
  ExecuteResult out;
  if (status == OpenPitPretradeStatus_Rejected) {
    if (rejects != nullptr) {
      out.rejects = ::openpit::detail::DrainRejectList(rejects);
    }
    return out;
  }
  out.reservation.emplace(reservation);
  return out;
}

}  // namespace pretrade

// Outcome of `ApplyExecutionReport`. Post-trade processing is non-atomic:
// account blocks do not invalidate successful account-level PnL or adjustment
// outcomes produced by other policies, so callers must consume every vector.
struct PostTradeResult {
  std::vector<::openpit::accounts::AccountBlock> accountBlocks;
  /// Account-level PnL outcomes. Each outcome identifies its account and policy
  /// group; `Get()` returns an account-currency amount or a halt reason. A
  /// newly halted calculation emits its reason once. Later checks can reject or
  /// block on the stored halt without emitting another account outcome, until a
  /// manager re-arms it with `Configurator::SetSpotFundsAccountPnl`. Re-arming
  /// an account accumulator does not affect any independently halted position
  /// accumulator.
  std::vector<::openpit::accountadjustment::AccountPnlOutcome> accountPnls;
  /// Policy-tagged adjustment outcomes. These remain meaningful even when
  /// `accountBlocks` is non-empty and must still be consumed by the caller.
  std::vector<::openpit::accountadjustment::Outcome> accountAdjustments;
};

// Accepted runtime policy configuration result. `accountBlocks` is non-empty
// when the accepted change immediately caused the engine to block an account.
struct PolicyConfigurationResult {
  std::vector<::openpit::accounts::AccountBlock> accountBlocks;
};

// Outcome of `ApplyAccountAdjustment`: either a rejected atomic batch or the
// per-adjustment outcomes produced by policies. This is a business result, not
// an exceptional failure; boundary failures still throw `openpit::Error`.
struct AdjustmentResult {
  std::optional<::openpit::accountadjustment::BatchError> batchError;
  std::vector<::openpit::accountadjustment::Outcome> accountAdjustmentOutcomes;
  std::vector<::openpit::accounts::AccountBlock> accountBlocks;

  [[nodiscard]] bool Passed() const noexcept { return !batchError.has_value(); }
};

// RAII engine handle. Move-only; destruction releases the engine and any state
// and policies it retained.
class Engine {
 public:
  Engine() = default;

  explicit Engine(OpenPitEngine* handle) noexcept : m_handle(handle) {}

  [[nodiscard]] explicit operator bool() const noexcept {
    return static_cast<bool>(m_handle);
  }

  // Runs the start stage of the pre-trade pipeline. On accept the result
  // carries a `pretrade::Request` to drive the remaining stages; on reject it
  // carries the rejects. Throws `openpit::Error` on a boundary failure (invalid
  // pointers, undecodable order payload).
  // Starts a deferred pre-trade request from an explicitly owned polymorphic
  // order. This overload is the escape hatch for type-erased code: the shared
  // owner keeps the exact dynamic type alive through `Request::Execute()`.
  [[nodiscard]] ::openpit::pretrade::StartResult StartPreTrade(
      std::shared_ptr<const ::openpit::Order> order) const {
    if (order == nullptr) {
      throw ::openpit::Error("StartPreTrade requires a non-null order");
    }
    const OpenPitOrder raw = order->EngineRaw();
    OpenPitPretradePreTradeRequest* request = nullptr;
    OpenPitPretradeRejectList* rejects = nullptr;
    OpenPitSharedString* error = nullptr;
    const ::openpit::detail::CurrentOrderGuard orderGuard(*order);
    detail::ClearPendingCallbackException();
    const OpenPitPretradeStatus status = openpit_engine_start_pre_trade(
        m_handle.Get(), &raw, &request, &rejects, &error);
    if (detail::HasPendingCallbackException()) {
      openpit_destroy_pretrade_pre_trade_request(request);
      openpit_pretrade_destroy_reject_list(rejects);
      openpit_destroy_shared_string(error);
    }
    detail::ThrowIfPendingCallbackException(
        "openpit_engine_start_pre_trade callback failed");
    if (status == OpenPitPretradeStatus_Error) {
      detail::ThrowFromSharedString(error,
                                    "openpit_engine_start_pre_trade failed");
    }
    ::openpit::pretrade::StartResult out;
    if (status == OpenPitPretradeStatus_Rejected) {
      if (rejects != nullptr) {
        out.rejects = detail::DrainRejectList(rejects);
      }
      return out;
    }
    out.request.emplace(request, std::move(order));
    return out;
  }

  // Starts a deferred request from a concrete order value. Lvalues are copied
  // and rvalues are moved into the returned request, preserving the exact
  // client type and making execution independent of the caller's lifetime.
  // Code that has already erased the static type to `openpit::Order` must use
  // the shared_ptr overload above so slicing is impossible.
  template <typename OrderT,
            std::enable_if_t<
                std::is_base_of_v<::openpit::Order, std::decay_t<OrderT>> &&
                    !std::is_same_v<::openpit::Order, std::decay_t<OrderT>>,
                int> = 0>
  [[nodiscard]] ::openpit::pretrade::StartResult StartPreTrade(
      OrderT&& order) const {
    return StartPreTrade(
        std::make_shared<std::decay_t<OrderT>>(std::forward<OrderT>(order)));
  }

  // Runs the complete pre-trade pipeline. On accept the result carries a
  // `pretrade::Reservation` representing reserved-but-not-finalized state; on
  // reject it carries the rejects. Throws `openpit::Error` on a boundary
  // failure.
  [[nodiscard]] ::openpit::pretrade::ExecuteResult ExecutePreTrade(
      const ::openpit::Order& order) const {
    const OpenPitOrder raw = order.EngineRaw();
    OpenPitPretradePreTradeReservation* reservation = nullptr;
    OpenPitPretradeRejectList* rejects = nullptr;
    OpenPitSharedString* error = nullptr;
    const ::openpit::detail::CurrentOrderGuard orderGuard(order);
    detail::ClearPendingCallbackException();
    const OpenPitPretradeStatus status = openpit_engine_execute_pre_trade(
        m_handle.Get(), &raw, &reservation, &rejects, &error);
    if (detail::HasPendingCallbackException()) {
      openpit_destroy_pretrade_pre_trade_reservation(reservation);
      openpit_pretrade_destroy_reject_list(rejects);
      openpit_destroy_shared_string(error);
    }
    detail::ThrowIfPendingCallbackException(
        "openpit_engine_execute_pre_trade callback failed");
    if (status == OpenPitPretradeStatus_Error) {
      detail::ThrowFromSharedString(error,
                                    "openpit_engine_execute_pre_trade failed");
    }
    ::openpit::pretrade::ExecuteResult out;
    if (status == OpenPitPretradeStatus_Rejected) {
      if (rejects != nullptr) {
        out.rejects = detail::DrainRejectList(rejects);
      }
      return out;
    }
    out.reservation.emplace(reservation);
    return out;
  }

  // Runs the start stage as a non-mutating dry-run. The returned report carries
  // the would-be pass/reject verdict without applying policy side effects.
  [[nodiscard]] ::openpit::pretrade::DryRunReport StartPreTradeDryRun(
      const ::openpit::Order& order) const {
    const OpenPitOrder raw = order.EngineRaw();
    OpenPitPretradePreTradeDryRunReport* report = nullptr;
    OpenPitSharedString* error = nullptr;
    const ::openpit::detail::CurrentOrderGuard orderGuard(order);
    detail::ClearPendingCallbackException();
    const bool ok = openpit_engine_start_pre_trade_dry_run(m_handle.Get(), &raw,
                                                           &report, &error);
    if (detail::HasPendingCallbackException()) {
      openpit_destroy_pretrade_pre_trade_dry_run_report(report);
      openpit_destroy_shared_string(error);
    }
    detail::ThrowIfPendingCallbackException(
        "openpit_engine_start_pre_trade_dry_run callback failed");
    if (!ok) {
      detail::ThrowFromSharedString(
          error, "openpit_engine_start_pre_trade_dry_run failed");
    }
    return ::openpit::pretrade::DryRunReport(report);
  }

  // Runs the full pre-trade pipeline as a non-mutating dry-run. The report
  // includes the would-be verdict, lock, account adjustments, and account
  // blocks, but engine state is unchanged.
  [[nodiscard]] ::openpit::pretrade::DryRunReport ExecutePreTradeDryRun(
      const ::openpit::Order& order) const {
    const OpenPitOrder raw = order.EngineRaw();
    OpenPitPretradePreTradeDryRunReport* report = nullptr;
    OpenPitSharedString* error = nullptr;
    const ::openpit::detail::CurrentOrderGuard orderGuard(order);
    detail::ClearPendingCallbackException();
    const bool ok = openpit_engine_execute_pre_trade_dry_run(
        m_handle.Get(), &raw, &report, &error);
    if (detail::HasPendingCallbackException()) {
      openpit_destroy_pretrade_pre_trade_dry_run_report(report);
      openpit_destroy_shared_string(error);
    }
    detail::ThrowIfPendingCallbackException(
        "openpit_engine_execute_pre_trade_dry_run callback failed");
    if (!ok) {
      detail::ThrowFromSharedString(
          error, "openpit_engine_execute_pre_trade_dry_run failed");
    }
    return ::openpit::pretrade::DryRunReport(report);
  }

  // Updates engine state from a completed execution report. Returns one
  // aggregate containing the account blocks, account-level PnL outcomes, and
  // account-adjustment outcomes policies produced. Throws `openpit::Error` on
  // a boundary failure (invalid pointers, undecodable report payload).
  [[nodiscard]] PostTradeResult ApplyExecutionReport(
      const ::openpit::ExecutionReport& report) const {
    return ApplyExecutionReportRaw(report, report.EngineRaw());
  }

  // Updates engine state from a completed fill while attaching the matching
  // pre-trade lock. Throws `openpit::Error` when `report` has no fill.
  [[nodiscard]] PostTradeResult ApplyExecutionReport(
      const ::openpit::ExecutionReport& report,
      const ::openpit::pretrade::PreTradeLock& lock) const {
    OpenPitExecutionReport raw = report.EngineRaw();
    if (!raw.fill.is_set) {
      throw ::openpit::Error(
          "ApplyExecutionReport with a pre-trade lock requires a fill");
    }
    raw.fill.value.lock = lock.Get();
    return ApplyExecutionReportRaw(report, raw);
  }

  // Applies a batch of balance/position adjustments to one account. On accept
  // the result is `Passed()` and carries the outcomes and account blocks
  // policies produced; on reject `batchError` carries the failing index and
  // policy rejects. Throws `openpit::Error` on a boundary failure (invalid
  // pointers, undecodable adjustment payload).
  //
  // `Adjustment` is the adjustment value type authored in
  // `openpit/accountadjustment/account_adjustment.hpp`
  // (`accountadjustment::AccountAdjustment`);
  // any type exposing `Raw()` returning a borrowed `OpenPitAccountAdjustment`
  // works. Each `Raw()` view must stay valid until this call returns.
  template <typename Adjustment>
  [[nodiscard]] AdjustmentResult ApplyAccountAdjustment(
      ::openpit::param::AccountId accountId,
      const std::vector<Adjustment>& adjustments) const {
    std::vector<OpenPitAccountAdjustment> raw;
    raw.reserve(adjustments.size());
    for (const auto& adjustment : adjustments) {
      raw.push_back(adjustment.Raw());
    }
    OpenPitAccountAdjustmentBatchError* reject = nullptr;
    OpenPitAccountAdjustmentOutcomeList* outcomes = nullptr;
    OpenPitPretradeAccountBlockList* blocks = nullptr;
    OpenPitSharedString* error = nullptr;
    detail::ClearPendingCallbackException();
    const OpenPitAccountAdjustmentApplyStatus status =
        openpit_engine_apply_account_adjustment(
            m_handle.Get(), accountId.Raw(), raw.empty() ? nullptr : raw.data(),
            raw.size(), &reject, &outcomes, &blocks, &error);
    if (detail::HasPendingCallbackException()) {
      openpit_destroy_account_adjustment_batch_error(reject);
      openpit_destroy_account_adjustment_outcome_list(outcomes);
      openpit_pretrade_destroy_account_block_list(blocks);
      openpit_destroy_shared_string(error);
    }
    detail::ThrowIfPendingCallbackException(
        "openpit_engine_apply_account_adjustment callback failed");
    if (status == OpenPitAccountAdjustmentApplyStatus_Error) {
      detail::ThrowFromSharedString(
          error, "openpit_engine_apply_account_adjustment failed");
    }
    if (status == OpenPitAccountAdjustmentApplyStatus_Rejected) {
      AdjustmentResult out;
      out.batchError = ::openpit::accountadjustment::BatchError(reject);
      return out;
    }
    // Adopt the caller-owned outcome list into the canonical RAII wrapper,
    // which copies it out and releases it on scope exit. A null list yields
    // empty.
    const ::openpit::accountadjustment::OutcomeList outcomeList(outcomes);
    AdjustmentResult out;
    out.accountAdjustmentOutcomes = outcomeList.ToVector();
    if (blocks != nullptr) {
      out.accountBlocks = ::openpit::detail::DrainAccountBlockList(blocks);
    }
    return out;
  }

  // Returns the account-administration handle (account groups + account/group
  // pre-trade blocking) bound to this engine. The handle is non-owning and
  // valid for as long as this engine is.
  [[nodiscard]] ::openpit::accounts::Accounts Accounts() const noexcept {
    return ::openpit::accounts::Accounts(m_handle.Get());
  }

  // Trivial read query: returns the account-group id for `accountId`, or
  // `std::nullopt` when the account belongs to no group.
  [[nodiscard]] std::optional<::openpit::param::AccountGroupId> AccountGroup(
      ::openpit::param::AccountId accountId) const {
    OpenPitParamAccountGroupId group = 0;
    if (openpit_engine_account_group(m_handle.Get(), accountId.Raw(), &group)) {
      return ::openpit::param::AccountGroupId::FromRaw(group);
    }
    return std::nullopt;
  }

  // Sets or clears the explicit account currency used by account-aware
  // policies. These calls do not recompute existing holdings; callers own any
  // live-state migration.
  void SetAccountCurrency(::openpit::param::AccountId accountId,
                          std::string_view asset) const {
    OpenPitSharedString* error = nullptr;
    if (!openpit_engine_set_account_currency(m_handle.Get(), accountId.Raw(),
                                             ::openpit::MakeStringView(asset),
                                             &error)) {
      detail::ThrowFromSharedString(
          error, "openpit_engine_set_account_currency failed");
    }
  }

  void ClearAccountCurrency(
      ::openpit::param::AccountId accountId) const noexcept {
    openpit_engine_clear_account_currency(m_handle.Get(), accountId.Raw());
  }

  void SetAccountGroupCurrency(::openpit::param::AccountGroupId groupId,
                               std::string_view asset) const {
    OpenPitSharedString* error = nullptr;
    if (!openpit_engine_set_account_group_currency(
            m_handle.Get(), groupId.Raw(), ::openpit::MakeStringView(asset),
            &error)) {
      detail::ThrowFromSharedString(
          error, "openpit_engine_set_account_group_currency failed");
    }
  }

  void ClearAccountGroupCurrency(
      ::openpit::param::AccountGroupId groupId) const noexcept {
    openpit_engine_clear_account_group_currency(m_handle.Get(), groupId.Raw());
  }

  // Returns a runtime policy-settings updater bound to this engine. Include
  // `openpit/pretrade/policies.hpp` (or the aggregate `openpit/openpit.hpp`) to
  // get the inline definition and configurator methods.
  [[nodiscard]] ::openpit::Configurator Configure() const noexcept;

  [[nodiscard]] OpenPitEngine* Get() const noexcept { return m_handle.Get(); }

 private:
  [[nodiscard]] PostTradeResult ApplyExecutionReportRaw(
      const ::openpit::ExecutionReport& report,
      const OpenPitExecutionReport& raw) const {
    OpenPitPostTradeResult* result = nullptr;
    OpenPitSharedString* error = nullptr;
    const ::openpit::detail::CurrentReportGuard reportGuard(report);
    detail::ClearPendingCallbackException();
    const bool ok = openpit_engine_apply_execution_report(m_handle.Get(), &raw,
                                                          &result, &error);
    detail::Handle<OpenPitPostTradeResult, detail::PostTradeResultDeleter>
        resultHandle(result);
    if (detail::HasPendingCallbackException()) {
      openpit_destroy_shared_string(error);
    }
    detail::ThrowIfPendingCallbackException(
        "openpit_engine_apply_execution_report callback failed");
    if (!ok) {
      detail::ThrowFromSharedString(
          error, "openpit_engine_apply_execution_report failed");
    }

    PostTradeResult out;
    const auto* blocks =
        openpit_post_trade_result_get_account_blocks(resultHandle.Get());
    const std::size_t blockCount =
        openpit_pretrade_account_block_list_len(blocks);
    out.accountBlocks.reserve(blockCount);
    for (std::size_t i = 0; i < blockCount; ++i) {
      OpenPitPretradeAccountBlock block{};
      if (openpit_pretrade_account_block_list_get(blocks, i, &block)) {
        out.accountBlocks.push_back(
            ::openpit::accounts::AccountBlock::FromRaw(block));
      }
    }

    const auto* accountPnls =
        openpit_post_trade_result_get_account_pnls(resultHandle.Get());
    const std::size_t accountPnlCount =
        openpit_account_pnl_outcome_list_len(accountPnls);
    out.accountPnls.reserve(accountPnlCount);
    for (std::size_t i = 0; i < accountPnlCount; ++i) {
      OpenPitAccountPnlOutcome outcome{};
      if (openpit_account_pnl_outcome_list_get(accountPnls, i, &outcome)) {
        out.accountPnls.push_back(
            ::openpit::accountadjustment::AccountPnlOutcome::FromRaw(outcome));
      }
    }

    const auto* adjustments =
        openpit_post_trade_result_get_account_adjustments(resultHandle.Get());
    const std::size_t adjustmentCount =
        openpit_account_adjustment_outcome_list_len(adjustments);
    out.accountAdjustments.reserve(adjustmentCount);
    for (std::size_t i = 0; i < adjustmentCount; ++i) {
      OpenPitAccountAdjustmentOutcome adjustment{};
      if (openpit_account_adjustment_outcome_list_get(adjustments, i,
                                                      &adjustment)) {
        out.accountAdjustments.push_back(
            ::openpit::accountadjustment::Outcome::FromRaw(adjustment));
      }
    }
    return out;
  }

  detail::Handle<OpenPitEngine, detail::EngineDeleter> m_handle;
};

// RAII engine builder. Construction selects the sync policy; register one or
// more pre-trade policies (`Add` accepts built-in configs and custom policy
// wrappers), then `Build()` consumes the configuration and yields an
// `Engine`.
//
// Move-only. Building with no policies registered is a boundary failure and
// throws `openpit::Error`.
class EngineBuilder {
 public:
  explicit EngineBuilder(SyncPolicy syncPolicy) {
    OpenPitSharedString* error = nullptr;
    OpenPitEngineBuilder* raw = openpit_create_engine_builder(
        static_cast<std::uint8_t>(syncPolicy), &error);
    if (raw == nullptr) {
      detail::ThrowFromSharedString(error,
                                    "openpit_create_engine_builder failed");
    }
    m_handle.Reset(raw);
  }

  // Registers either a built-in policy configuration (types exposing
  // `AddTo(EngineBuilder&)`) or a custom pre-trade policy wrapper (types
  // exposing `Get()`). Registration throws `openpit::Error` on a
  // boundary/configuration failure. Returns `*this` for chaining.
  template <typename Policy>
  EngineBuilder& Add(const Policy& policy) {
    if constexpr (detail::HasAddTo<Policy>::value) {
      policy.AddTo(*this);
    } else if constexpr (detail::HasGet<Policy>::value) {
      AddPreTradePolicy(policy);
    } else {
      static_assert(detail::DependentFalse<Policy>::value,
                    "openpit::EngineBuilder::Add expects a policy config with "
                    "AddTo(EngineBuilder&) or a policy wrapper with Get()");
    }
    return *this;
  }

  // Registers a custom pre-trade policy on this builder. `Policy` is any type
  // exposing `Get()` that returns the borrowed `OpenPitPretradePreTradePolicy*`
  // (e.g. `openpit::pretrade::CustomPolicy<Handler>`). The builder retains its
  // own reference; the caller keeps ownership of the policy object. Throws
  // `openpit::Error` on a boundary failure. Returns `*this` for chaining.
  template <typename Policy>
  EngineBuilder& AddPreTradePolicy(const Policy& policy) {
    OpenPitSharedString* error = nullptr;
    if (!openpit_engine_builder_add_pre_trade_policy(m_handle.Get(),
                                                     policy.Get(), &error)) {
      detail::ThrowFromSharedString(
          error, "openpit_engine_builder_add_pre_trade_policy failed");
    }
    return *this;
  }

  // Finalizes the builder into an engine.
  //
  // Throws `EngineBuildError` for a domain configuration failure, or
  // `openpit::Error` for a boundary failure. The builder is consumed regardless
  // of outcome; this object must not be built again.
  [[nodiscard]] Engine Build() {
    OpenPitEngineBuildError* buildError = nullptr;
    OpenPitSharedString* error = nullptr;
    OpenPitEngine* engine =
        openpit_engine_builder_build(m_handle.Get(), &buildError, &error);
    if (engine != nullptr) {
      return Engine(engine);
    }
    if (buildError != nullptr) {
      ThrowBuildError(buildError);
    }
    detail::ThrowFromSharedString(error, "openpit_engine_builder_build failed");
  }

  [[nodiscard]] OpenPitEngineBuilder* Get() const noexcept {
    return m_handle.Get();
  }

 private:
  [[noreturn]] static void ThrowBuildError(
      OpenPitEngineBuildError* buildError) {
    EngineBuildErrorCode code = static_cast<EngineBuildErrorCode>(
        openpit_engine_build_error_get_code(buildError));
    std::string policyName =
        StringView(openpit_engine_build_error_get_policy_name(buildError))
            .ToString();
    std::uint16_t policyGroupId =
        openpit_engine_build_error_get_policy_group_id(buildError);
    openpit_destroy_engine_build_error(buildError);
    throw EngineBuildError("engine build failed", code, std::move(policyName),
                           policyGroupId);
  }

  detail::Handle<OpenPitEngineBuilder, detail::EngineBuilderDeleter> m_handle;
};

}  // namespace openpit
