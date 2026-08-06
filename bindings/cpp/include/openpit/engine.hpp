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
#include "openpit/detail/callback_error.hpp"
#include "openpit/detail/handle.hpp"
#include "openpit/error.hpp"
#include "openpit/model/model.hpp"
#include "openpit/param/account_id.hpp"
#include "openpit/pretrade/detail/lists.hpp"
#include "openpit/pretrade/drop_copy_operation.hpp"
#include "openpit/pretrade/dry_run_report.hpp"
#include "openpit/pretrade/start_result.hpp"
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

}  // namespace detail

// Storage synchronization policy selected at builder time. Mirrors
// `OpenPitSyncPolicy`.
enum class SyncPolicy : std::uint8_t {
  // Single-threaded: the engine must stay on its creating thread.
  None = 0,
  // Fully synchronized: concurrent calls on one handle are safe.
  Full = 1,
  // Account-sharded: sequential cross-thread access with per-account pinning.
  Account = 2,
};

// Machine-readable category of a domain engine-build failure. Mirrors
// `OpenPitEngineBuildErrorCode`.
enum class EngineBuildErrorCode : std::uint8_t {
  DuplicatePolicyName = 0,
  DuplicatePolicyGroupId = 1,
  Other = 2,
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

struct EngineBuildErrorDeleter {
  void operator()(OpenPitEngineBuildError* handle) const noexcept {
    openpit_destroy_engine_build_error(handle);
  }
};

}  // namespace detail

// Outcome of `ApplyExecutionReport`. Post-trade processing is non-atomic:
// account blocks do not invalidate successful account-level PnL or adjustment
// outcomes produced by other policies, so callers must consume every vector.
struct PostTradeResult {
  std::vector<::openpit::accounts::AccountBlock> accountBlocks;
  /// Account-level PnL outcomes. Each outcome identifies its account and policy
  /// group; `Amount()` and `HaltReason()` identify the result. A
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

// Newly inserted blocks for accounts selected by the engine.
struct AccountBlockOutcomes {
  std::vector<::openpit::accounts::AccountBlockOutcome> accountBlocks;
};

// Accepted per-account runtime policy configuration result. A SpotFunds
// account-P&L force-set exposes its policy-reported breach or halt block even
// when an existing first-cause block prevents insertion. The engine processes
// each exposed block before returning without replacing that first cause. The
// caller supplied the affected account, so the blocks do not repeat it.
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

  [[nodiscard]] explicit operator bool() const noexcept {
    return static_cast<bool>(m_handle);
  }

  // Starts a deferred pre-trade request from an explicitly owned polymorphic
  // order. This overload is the escape hatch for type-erased code: the shared
  // owner keeps the exact dynamic type alive through `Request::Execute()`.
  [[nodiscard]] ::openpit::pretrade::StartResult StartPreTrade(
      std::shared_ptr<const ::openpit::Order> order) const {
    if (order == nullptr) {
      throw ::openpit::Error("StartPreTrade requires a non-null order");
    }
    const OpenPitOrder raw = ::openpit::detail::Native(*order);
    OpenPitPretradePreTradeRequest* request = nullptr;
    OpenPitPretradeRejectList* rejects = nullptr;
    OpenPitSharedString* error = nullptr;
    const ::openpit::detail::CurrentOrderGuard orderGuard(*order);
    detail::CallbackExceptionScope callbackExceptions;
    const OpenPitPretradeStatus status = openpit_engine_start_pre_trade(
        m_handle.Get(), &raw, &request, &rejects, &error);
    if (callbackExceptions.HasPending()) {
      openpit_destroy_pretrade_pre_trade_request(request);
      openpit_destroy_pretrade_reject_list(rejects);
      openpit_destroy_shared_string(error);
    }
    callbackExceptions.ThrowIfPending();
    if (status == OpenPitPretradeStatus_Error) {
      detail::ThrowFromSharedString(error,
                                    "openpit_engine_start_pre_trade failed");
    }
    ::openpit::pretrade::StartResult out;
    if (status == OpenPitPretradeStatus_Rejected) {
      if (rejects != nullptr) {
        out.rejects = pretrade::detail::ListAccess::DrainRejects(rejects);
      }
      return out;
    }
    out.request = ::openpit::detail::FromNative<::openpit::pretrade::Request>(
        ::openpit::pretrade::detail::RequestInit{request, std::move(order)});
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
    const OpenPitOrder raw = ::openpit::detail::Native(order);
    OpenPitPretradePreTradeReservation* reservation = nullptr;
    OpenPitPretradeRejectList* rejects = nullptr;
    OpenPitSharedString* error = nullptr;
    const ::openpit::detail::CurrentOrderGuard orderGuard(order);
    detail::CallbackExceptionScope callbackExceptions;
    const OpenPitPretradeStatus status = openpit_engine_execute_pre_trade(
        m_handle.Get(), &raw, &reservation, &rejects, &error);
    if (callbackExceptions.HasPending()) {
      openpit_destroy_pretrade_pre_trade_reservation(reservation);
      openpit_destroy_pretrade_reject_list(rejects);
      openpit_destroy_shared_string(error);
    }
    callbackExceptions.ThrowIfPending();
    if (status == OpenPitPretradeStatus_Error) {
      detail::ThrowFromSharedString(error,
                                    "openpit_engine_execute_pre_trade failed");
    }
    ::openpit::pretrade::ExecuteResult out;
    if (status == OpenPitPretradeStatus_Rejected) {
      if (rejects != nullptr) {
        out.rejects = pretrade::detail::ListAccess::DrainRejects(rejects);
      }
      return out;
    }
    out.reservation =
        ::openpit::detail::FromNative<::openpit::pretrade::Reservation>(
            reservation);
    return out;
  }

  /// Applies an already executed order without enforcing existing account and
  /// account-group blocks or ordinary policy rejects. On accept the result
  /// carries a `pretrade::DropCopyOperation` representing applied-but-not-
  /// finalized state; a fatal evaluation failure carries the rejects instead.
  /// Throws `openpit::Error` on a boundary failure.
  ///
  /// The returned operation owns finalization exactly like a reservation:
  /// commit applies the prepared state, rollback compensates it, and
  /// destruction rolls back an unresolved operation. Account-control operations
  /// and rate-limit attempts are applied before this call returns and stay
  /// outside that finalization boundary.
  ///
  /// Drop copy requires a readable account id: an order whose account id cannot
  /// be read is rejected with `RejectCode::MissingRequiredField` before any
  /// policy runs, so no policy observes it and no state is touched.
  ///
  /// A fully synchronized engine accepts concurrent calls for the same account,
  /// but individual storage accesses may interleave. Callers that require
  /// whole-pipeline isolation must serialize those calls externally.
  [[nodiscard]] ::openpit::pretrade::DropCopyResult ApplyDropCopy(
      const ::openpit::Order& order) const {
    const OpenPitOrder raw = ::openpit::detail::Native(order);
    OpenPitPretradeDropCopyOperation* operation = nullptr;
    OpenPitPretradeRejectList* rejects = nullptr;
    OpenPitSharedString* error = nullptr;
    const ::openpit::detail::CurrentOrderGuard orderGuard(order);
    detail::CallbackExceptionScope callbackExceptions;
    const OpenPitPretradeStatus status = openpit_engine_apply_drop_copy(
        m_handle.Get(), &raw, &operation, &rejects, &error);
    if (callbackExceptions.HasPending()) {
      openpit_destroy_pretrade_drop_copy_operation(operation);
      openpit_destroy_pretrade_reject_list(rejects);
      openpit_destroy_shared_string(error);
    }
    callbackExceptions.ThrowIfPending();
    if (status == OpenPitPretradeStatus_Error) {
      detail::ThrowFromSharedString(error,
                                    "openpit_engine_apply_drop_copy failed");
    }
    ::openpit::pretrade::DropCopyResult out;
    if (status == OpenPitPretradeStatus_Rejected) {
      if (rejects != nullptr) {
        out.rejects = pretrade::detail::ListAccess::DrainRejects(rejects);
      }
      return out;
    }
    out.operation =
        ::openpit::detail::FromNative<::openpit::pretrade::DropCopyOperation>(
            operation);
    return out;
  }

  // Runs the start stage as a non-mutating dry-run. The returned report carries
  // the would-be pass/reject verdict without applying policy side effects.
  [[nodiscard]] ::openpit::pretrade::DryRunReport StartPreTradeDryRun(
      const ::openpit::Order& order) const {
    const OpenPitOrder raw = ::openpit::detail::Native(order);
    OpenPitPretradePreTradeDryRunReport* report = nullptr;
    OpenPitSharedString* error = nullptr;
    const ::openpit::detail::CurrentOrderGuard orderGuard(order);
    detail::CallbackExceptionScope callbackExceptions;
    const bool ok = openpit_engine_start_pre_trade_dry_run(m_handle.Get(), &raw,
                                                           &report, &error);
    if (callbackExceptions.HasPending()) {
      openpit_destroy_pretrade_pre_trade_dry_run_report(report);
      openpit_destroy_shared_string(error);
    }
    callbackExceptions.ThrowIfPending();
    if (!ok) {
      detail::ThrowFromSharedString(
          error, "openpit_engine_start_pre_trade_dry_run failed");
    }
    return ::openpit::detail::FromNative<::openpit::pretrade::DryRunReport>(
        report);
  }

  // Runs the full pre-trade pipeline as a non-mutating dry-run. The report
  // includes the would-be verdict, lock, account adjustments, and account
  // blocks, but engine state is unchanged.
  [[nodiscard]] ::openpit::pretrade::DryRunReport ExecutePreTradeDryRun(
      const ::openpit::Order& order) const {
    const OpenPitOrder raw = ::openpit::detail::Native(order);
    OpenPitPretradePreTradeDryRunReport* report = nullptr;
    OpenPitSharedString* error = nullptr;
    const ::openpit::detail::CurrentOrderGuard orderGuard(order);
    detail::CallbackExceptionScope callbackExceptions;
    const bool ok = openpit_engine_execute_pre_trade_dry_run(
        m_handle.Get(), &raw, &report, &error);
    if (callbackExceptions.HasPending()) {
      openpit_destroy_pretrade_pre_trade_dry_run_report(report);
      openpit_destroy_shared_string(error);
    }
    callbackExceptions.ThrowIfPending();
    if (!ok) {
      detail::ThrowFromSharedString(
          error, "openpit_engine_execute_pre_trade_dry_run failed");
    }
    return ::openpit::detail::FromNative<::openpit::pretrade::DryRunReport>(
        report);
  }

  // Updates engine state from a completed execution report. Returns one
  // aggregate containing the account blocks, account-level PnL outcomes, and
  // account-adjustment outcomes policies produced. Throws `openpit::Error` on
  // a boundary failure (invalid pointers, undecodable report payload).
  [[nodiscard]] PostTradeResult ApplyExecutionReport(
      const ::openpit::ExecutionReport& report) const {
    return ApplyExecutionReportRaw(report, ::openpit::detail::Native(report));
  }

  // Applies a batch of balance/position adjustments to one account. On accept
  // the result is `Passed()` and carries the outcomes and account blocks
  // policies produced; on reject `batchError` carries the failing index and
  // policy rejects. Throws `openpit::Error` on a boundary failure (invalid
  // pointers, undecodable adjustment payload).
  //
  // `Adjustment` is normally `accountadjustment::AccountAdjustment`. Custom
  // adjustment types can opt into the same zero-overhead bridge by granting
  // `detail::NativeAccess` access to a private `Native()` returning the
  // module's `accountadjustment::detail::RawAccountAdjustment` alias.
  template <typename Adjustment>
  [[nodiscard]] AdjustmentResult ApplyAccountAdjustment(
      ::openpit::param::AccountId accountId,
      const std::vector<Adjustment>& adjustments) const {
    std::vector<OpenPitAccountAdjustment> raw;
    raw.reserve(adjustments.size());
    for (const auto& adjustment : adjustments) {
      raw.push_back(::openpit::detail::Native(adjustment));
    }
    OpenPitAccountAdjustmentBatchError* reject = nullptr;
    OpenPitAccountAdjustmentOutcomeList* outcomes = nullptr;
    OpenPitPretradeAccountBlockList* blocks = nullptr;
    OpenPitSharedString* error = nullptr;
    detail::CallbackExceptionScope callbackExceptions;
    const OpenPitAccountAdjustmentApplyStatus status =
        openpit_engine_apply_account_adjustment(
            m_handle.Get(), ::openpit::detail::Native(accountId),
            raw.empty() ? nullptr : raw.data(), raw.size(), &reject, &outcomes,
            &blocks, &error);
    detail::Handle<OpenPitPretradeAccountBlockList,
                   ::openpit::pretrade::detail::AccountBlockListDeleter>
        blocksOwner(blocks);
    if (callbackExceptions.HasPending()) {
      openpit_destroy_account_adjustment_batch_error(reject);
      openpit_destroy_account_adjustment_outcome_list(outcomes);
      openpit_destroy_shared_string(error);
    }
    callbackExceptions.ThrowIfPending();
    if (status == OpenPitAccountAdjustmentApplyStatus_Error) {
      detail::ThrowFromSharedString(
          error, "openpit_engine_apply_account_adjustment failed");
    }
    if (status == OpenPitAccountAdjustmentApplyStatus_Rejected) {
      AdjustmentResult out;
      out.batchError = ::openpit::detail::FromNative<
          ::openpit::accountadjustment::BatchError>(reject);
      return out;
    }
    // Adopt the caller-owned outcome list into the canonical RAII wrapper,
    // which copies it out and releases it on scope exit. A null list yields
    // empty.
    const auto outcomeList = ::openpit::detail::FromNative<
        ::openpit::accountadjustment::OutcomeList>(outcomes);
    AdjustmentResult out;
    out.accountAdjustmentOutcomes = outcomeList.ToVector();
    if (blocksOwner) {
      out.accountBlocks =
          ::openpit::pretrade::detail::ListAccess::DrainAccountBlocks(
              blocksOwner.Release());
    }
    return out;
  }

  // Returns the account-administration handle (account groups + account/group
  // pre-trade blocking) bound to this engine. The handle is non-owning and
  // valid for as long as this engine is.
  [[nodiscard]] ::openpit::accounts::Accounts Accounts() const noexcept {
    return ::openpit::detail::FromNative<::openpit::accounts::Accounts>(
        m_handle.Get());
  }

  // Returns a runtime policy-settings updater bound to this engine. Include
  // `openpit/pretrade/policies.hpp` (or the aggregate `openpit/openpit.hpp`) to
  // get the inline definition and configurator methods.
  [[nodiscard]] ::openpit::Configurator Configure() const noexcept;

 private:
  friend class detail::NativeAccess;

  explicit Engine(OpenPitEngine* handle) noexcept : m_handle(handle) {}

  [[nodiscard]] OpenPitEngine* Native() const noexcept {
    return m_handle.Get();
  }

  [[nodiscard]] PostTradeResult ApplyExecutionReportRaw(
      const ::openpit::ExecutionReport& report,
      const OpenPitExecutionReport& raw) const {
    OpenPitPostTradeResult* result = nullptr;
    OpenPitSharedString* error = nullptr;
    const ::openpit::detail::CurrentReportGuard reportGuard(report);
    detail::CallbackExceptionScope callbackExceptions;
    const bool ok = openpit_engine_apply_execution_report(m_handle.Get(), &raw,
                                                          &result, &error);
    detail::Handle<OpenPitPostTradeResult, detail::PostTradeResultDeleter>
        resultHandle(result);
    if (callbackExceptions.HasPending()) {
      openpit_destroy_shared_string(error);
    }
    callbackExceptions.ThrowIfPending();
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
            ::openpit::detail::FromNative<::openpit::accounts::AccountBlock>(
                block));
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
            ::openpit::detail::FromNative<
                ::openpit::accountadjustment::AccountPnlOutcome>(outcome));
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
            ::openpit::detail::FromNative<
                ::openpit::accountadjustment::Outcome>(adjustment));
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

  // Registers either a built-in policy configuration or a custom pre-trade
  // policy wrapper. Registration throws `openpit::Error` on a boundary or
  // configuration failure. Returns `*this` for chaining.
  template <typename Policy>
  EngineBuilder& Add(const Policy& policy) {
    if constexpr (detail::HasAddTo<Policy>::value) {
      policy.AddTo(*this);
    } else {
      AddPreTradePolicy(policy);
    }
    return *this;
  }

  // Registers a custom pre-trade policy on this builder. The builder retains
  // its own reference; the caller keeps ownership of the policy object.
  // Throws `openpit::Error` on a boundary failure. Returns `*this` for
  // chaining.
  template <typename Policy>
  EngineBuilder& AddPreTradePolicy(const Policy& policy) {
    OpenPitSharedString* error = nullptr;
    if (!openpit_engine_builder_add_pre_trade_policy(
            m_handle.Get(), detail::Native(policy), &error)) {
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
      return detail::FromNative<Engine>(engine);
    }
    if (buildError != nullptr) {
      ThrowBuildError(buildError);
    }
    detail::ThrowFromSharedString(error, "openpit_engine_builder_build failed");
  }

 private:
  friend class detail::NativeAccess;

  [[nodiscard]] OpenPitEngineBuilder* Native() const noexcept {
    return m_handle.Get();
  }
  [[noreturn]] static void ThrowBuildError(
      OpenPitEngineBuildError* buildError) {
    detail::Handle<OpenPitEngineBuildError, detail::EngineBuildErrorDeleter>
        owner(buildError);
    EngineBuildErrorCode code = static_cast<EngineBuildErrorCode>(
        openpit_engine_build_error_get_code(owner.Get()));
    std::string policyName =
        detail::FromNative<StringView>(
            openpit_engine_build_error_get_policy_name(owner.Get()))
            .ToString();
    std::uint16_t policyGroupId =
        openpit_engine_build_error_get_policy_group_id(owner.Get());
    throw EngineBuildError("engine build failed", code, std::move(policyName),
                           policyGroupId);
  }

  detail::Handle<OpenPitEngineBuilder, detail::EngineBuilderDeleter> m_handle;
};

}  // namespace openpit
