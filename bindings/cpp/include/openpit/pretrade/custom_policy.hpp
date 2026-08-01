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
#include "openpit/pretrade/callbacks.hpp"
#include "openpit/pretrade/context.hpp"
#include "openpit/pretrade/decision.hpp"
#include "openpit/string.hpp"
#include "openpit/tx/tx.hpp"

#include <openpit.h>

#include <cstdint>
#include <memory>
#include <optional>
#include <stdexcept>
#include <string>
#include <type_traits>
#include <utility>
#include <vector>

// Custom-policy authoring support.
//
// `CustomPolicy<Handler>` lets a plain C++ object act as a pre-trade policy.
// It dispatches each supported stage to the handler and translates its
// `std::optional<Reject>` or `PolicyDecision` result into the engine outcome.
//
// `Handler` is any object exposing one or more of the methods below. Each hook
// is enabled only when the corresponding method is present (detected at compile
// time); an absent hook means "accept by default". This lets a
// `StartPolicyAdapter`
// (start-only), a unified `PolicyAdapter` (main plus any optional stages), or a
// direct handler be wrapped directly:
//   - std::optional<Reject> CheckPreTradeStart(const Context&,
//                                             const openpit::Order&) const
//   - std::optional<Reject> CheckPreTradeStartDryRun(
//         const Context&, const openpit::Order&) const
//   - void PerformPreTradeCheck(const Context&, tx::Mutations&, Result&,
//                              PolicyDecision&) const
//   - void PerformPreTradeCheckDryRun(const Context&, tx::Mutations&, Result&,
//                                    PolicyDecision&) const
//   - std::vector<accounts::AccountBlock> ApplyExecutionReport(
//         const PostTradeContext&, const openpit::ExecutionReport&,
//         PostTradeAdjustments&, PostTradePnls&) const
//   - PolicyAccountAdjustmentResult ApplyAccountAdjustment(
//         const accountadjustment::Context&, param::AccountId,
//         const accountadjustment::AccountAdjustment&, tx::Mutations&,
//         AccountOutcomes&) const
//
// The policy name is supplied to `CustomPolicy` itself. A direct handler does
// not need a `Name()` method. Adapter handlers do expose `Name()` through their
// client policy; when present, it must match the constructor name so
// registration and adapter-produced rejects use one stable identity.
//
// The legacy two-argument main-stage and one-argument report hooks remain
// accepted for source compatibility. They cannot use the added collectors.
//
// When a dry-run hook is absent, the engine delegates that stage to the normal
// hook.
//
// Handler exceptions are deferred until cleanup completes, then the owning
// Engine call rethrows the original exception directly. SafeSlow adapter
// payload mismatches remain value rejects rather than exceptions.
//
// The policy is a move-only owning RAII handle. Registration on the engine
// builder keeps its own reference; the caller still owns this handle and must
// keep it alive until at least registration completes.

namespace openpit::pretrade {

namespace detail {

struct PreTradePolicyDeleter {
  void operator()(OpenPitPretradePreTradePolicy* handle) const noexcept {
    openpit_destroy_pretrade_pre_trade_policy(handle);
  }
};

// Reports a refused `openpit_pretrade_reject_list_push` and throws,
// releasing `list` on the way out. `list` is always a fresh list created one
// line earlier by `openpit_create_pretrade_reject_list`, never the shared
// accept sentinel, so the only reachable cause is a reject carrying an
// unknown scope.
[[noreturn]] inline void ThrowRejectPushFailure(
    OpenPitPretradeRejectList* list) {
  openpit_destroy_pretrade_reject_list(list);
  throw std::invalid_argument("reject scope is invalid");
}

class CustomPolicyAccess final {
 private:
  [[nodiscard]] static OpenPitPretradeRejectList* AcceptList() {
    return openpit_pretrade_reject_list_get_accept_sentinel();
  }

  [[nodiscard]] static OpenPitPretradeRejectList* RejectToList(
      const Reject& reject) {
    OpenPitPretradeRejectList* list = openpit_create_pretrade_reject_list(1);
    if (!openpit_pretrade_reject_list_push(list,
                                           ::openpit::detail::Native(reject))) {
      ThrowRejectPushFailure(list);
    }
    return list;
  }

  [[nodiscard]] static OpenPitPretradeRejectList* DecisionToList(
      const PolicyDecision& decision) {
    if (!decision.IsRejected()) {
      return AcceptList();
    }
    OpenPitPretradeRejectList* list =
        openpit_create_pretrade_reject_list(decision.rejects.size());
    for (const Reject& reject : decision.rejects) {
      if (!openpit_pretrade_reject_list_push(
              list, ::openpit::detail::Native(reject))) {
        ThrowRejectPushFailure(list);
      }
    }
    return list;
  }

  [[nodiscard]] static OpenPitPretradeAccountBlockList* AccountBlocksToList(
      const std::vector<::openpit::accounts::AccountBlock>& blocks) {
    if (blocks.empty()) {
      return nullptr;
    }
    OpenPitPretradeAccountBlockList* list =
        openpit_create_pretrade_account_block_list(blocks.size());
    for (const auto& block : blocks) {
      openpit_pretrade_account_block_list_push(
          list, ::openpit::detail::Native(block));
    }
    return list;
  }

  [[nodiscard]] static OpenPitPretradeRejectList* CallbackErrorRejectList() {
    return RejectToList(Reject(
        "openpit.callback", RejectScope::Order, RejectCode::SystemUnavailable,
        "custom policy callback failed", "callback raised an exception"));
  }

  [[nodiscard]] static OpenPitPretradeRejectList*
  NoThrowCallbackErrorRejectList() noexcept {
    try {
      return CallbackErrorRejectList();
    } catch (...) {
      return nullptr;
    }
  }

  [[nodiscard]] static OpenPitPretradeAccountBlockList*
  CallbackErrorAccountBlockList() {
    std::string policy = "openpit.callback";
    std::string reason = "custom policy callback failed";
    std::string details = "callback raised an exception";
    OpenPitPretradeAccountBlock block{};
    block.policy = ::openpit::detail::MakeStringView(policy);
    block.reason = ::openpit::detail::MakeStringView(reason);
    block.details = ::openpit::detail::MakeStringView(details);
    block.code = static_cast<OpenPitPretradeRejectCode>(
        static_cast<std::uint16_t>(RejectCode::SystemUnavailable));
    OpenPitPretradeAccountBlockList* list =
        openpit_create_pretrade_account_block_list(1);
    openpit_pretrade_account_block_list_push(list, block);
    return list;
  }

  [[nodiscard]] static OpenPitPretradeAccountBlockList*
  NoThrowCallbackErrorAccountBlockList() noexcept {
    try {
      return CallbackErrorAccountBlockList();
    } catch (...) {
      return nullptr;
    }
  }

  template <typename Handler>
  friend class ::openpit::pretrade::CustomPolicy;
};

// Compile-time detection of each optional handler hook.

template <typename Handler, typename = void>
struct HasCheckPreTradeStart : std::false_type {};
template <typename Handler>
struct HasCheckPreTradeStart<
    Handler,
    std::void_t<decltype(std::declval<const Handler&>().CheckPreTradeStart(
        std::declval<const ::openpit::Order&>()))>> : std::true_type {};

template <typename Handler, typename = void>
struct HasCheckPreTradeStartWithContext : std::false_type {};
template <typename Handler>
struct HasCheckPreTradeStartWithContext<
    Handler,
    std::void_t<decltype(std::declval<const Handler&>().CheckPreTradeStart(
        std::declval<const Context&>(),
        std::declval<const ::openpit::Order&>()))>> : std::true_type {};

template <typename Handler, typename = void>
struct HasCheckPreTradeStartDryRun : std::false_type {};
template <typename Handler>
struct HasCheckPreTradeStartDryRun<
    Handler,
    std::void_t<
        decltype(std::declval<const Handler&>().CheckPreTradeStartDryRun(
            std::declval<const ::openpit::Order&>()))>> : std::true_type {};

template <typename Handler, typename = void>
struct HasCheckPreTradeStartDryRunWithContext : std::false_type {};
template <typename Handler>
struct HasCheckPreTradeStartDryRunWithContext<
    Handler,
    std::void_t<
        decltype(std::declval<const Handler&>().CheckPreTradeStartDryRun(
            std::declval<const Context&>(),
            std::declval<const ::openpit::Order&>()))>> : std::true_type {};

template <typename Handler, typename = void>
struct HasPerformPreTradeCheck : std::false_type {};
template <typename Handler>
struct HasPerformPreTradeCheck<
    Handler,
    std::void_t<decltype(std::declval<const Handler&>().PerformPreTradeCheck(
        std::declval<const Context&>(), std::declval<PolicyDecision&>()))>>
    : std::true_type {};

template <typename Handler, typename = void>
struct HasPerformPreTradeCheckFull : std::false_type {};
template <typename Handler>
struct HasPerformPreTradeCheckFull<
    Handler,
    std::void_t<decltype(std::declval<const Handler&>().PerformPreTradeCheck(
        std::declval<const Context&>(),
        std::declval<::openpit::tx::Mutations&>(), std::declval<Result&>(),
        std::declval<PolicyDecision&>()))>> : std::true_type {};

template <typename Handler, typename = void>
struct HasPerformPreTradeCheckDryRun : std::false_type {};
template <typename Handler>
struct HasPerformPreTradeCheckDryRun<
    Handler,
    std::void_t<
        decltype(std::declval<const Handler&>().PerformPreTradeCheckDryRun(
            std::declval<const Context&>(), std::declval<PolicyDecision&>()))>>
    : std::true_type {};

template <typename Handler, typename = void>
struct HasPerformPreTradeCheckDryRunFull : std::false_type {};
template <typename Handler>
struct HasPerformPreTradeCheckDryRunFull<
    Handler,
    std::void_t<
        decltype(std::declval<const Handler&>().PerformPreTradeCheckDryRun(
            std::declval<const Context&>(),
            std::declval<::openpit::tx::Mutations&>(), std::declval<Result&>(),
            std::declval<PolicyDecision&>()))>> : std::true_type {};

template <typename Handler, typename = void>
struct HasApplyExecutionReport : std::false_type {};
template <typename Handler>
struct HasApplyExecutionReport<
    Handler,
    std::void_t<decltype(std::declval<const Handler&>().ApplyExecutionReport(
        std::declval<const ::openpit::ExecutionReport&>()))>> : std::true_type {
};

template <typename Handler, typename = void>
struct HasApplyExecutionReportFull : std::false_type {};
template <typename Handler>
struct HasApplyExecutionReportFull<
    Handler,
    std::void_t<decltype(std::declval<const Handler&>().ApplyExecutionReport(
        std::declval<const PostTradeContext&>(),
        std::declval<const ::openpit::ExecutionReport&>(),
        std::declval<PostTradeAdjustments&>(),
        std::declval<PostTradePnls&>()))>> : std::true_type {};

template <typename Handler, typename = void>
struct HasLegacyApplyExecutionReportFull : std::false_type {};
template <typename Handler>
struct HasLegacyApplyExecutionReportFull<
    Handler,
    std::void_t<decltype(std::declval<const Handler&>().ApplyExecutionReport(
        std::declval<const PostTradeContext&>(),
        std::declval<const ::openpit::ExecutionReport&>(),
        std::declval<PostTradeAdjustments&>()))>> : std::true_type {};

template <typename Handler, typename = void>
struct HasApplyAccountAdjustment : std::false_type {};
template <typename Handler>
struct HasApplyAccountAdjustment<
    Handler,
    std::void_t<decltype(std::declval<const Handler&>().ApplyAccountAdjustment(
        std::declval<const ::openpit::accountadjustment::Context&>(),
        std::declval<::openpit::param::AccountId>(),
        std::declval<const ::openpit::accountadjustment::AccountAdjustment&>(),
        std::declval<::openpit::tx::Mutations&>(),
        std::declval<AccountOutcomes&>()))>> : std::true_type {};

template <typename Handler, typename = void>
struct HasName : std::false_type {};
template <typename Handler>
struct HasName<Handler,
               std::void_t<decltype(std::declval<const Handler&>().Name())>>
    : std::true_type {};

}  // namespace detail

// Owning custom pre-trade policy backed by a C++ `Handler`.
template <typename Handler>
class CustomPolicy {
  static_assert(
      !detail::HasLegacyApplyExecutionReportFull<Handler>::value,
      "Handler::ApplyExecutionReport(context, report, adjustments) was "
      "removed; add PostTradePnls& as the fourth argument");
  static_assert(
      detail::HasCheckPreTradeStart<Handler>::value ||
          detail::HasCheckPreTradeStartWithContext<Handler>::value ||
          detail::HasCheckPreTradeStartDryRun<Handler>::value ||
          detail::HasCheckPreTradeStartDryRunWithContext<Handler>::value ||
          detail::HasPerformPreTradeCheck<Handler>::value ||
          detail::HasPerformPreTradeCheckFull<Handler>::value ||
          detail::HasPerformPreTradeCheckDryRun<Handler>::value ||
          detail::HasPerformPreTradeCheckDryRunFull<Handler>::value ||
          detail::HasApplyExecutionReport<Handler>::value ||
          detail::HasApplyExecutionReportFull<Handler>::value ||
          detail::HasLegacyApplyExecutionReportFull<Handler>::value ||
          detail::HasApplyAccountAdjustment<Handler>::value,
      "Handler must expose at least one policy hook");

 public:
  // Creates a policy named `name`, tagged with `policyGroupId`, dispatching to
  // a moved-in `handler`. Throws `openpit::Error` when registration fails.
  CustomPolicy(
      std::string_view name, Handler handler,
      std::uint16_t policyGroupId = ::openpit::param::DefaultPolicyGroupId)
      : m_handler(std::make_unique<Handler>(std::move(handler))) {
    if constexpr (detail::HasName<Handler>::value) {
      const std::string handlerName(m_handler->Name());
      if (handlerName != name) {
        throw ::openpit::Error("custom policy name \"" + std::string(name) +
                               "\" does not match handler name \"" +
                               handlerName + "\"");
      }
    }
    OpenPitSharedString* error = nullptr;
    OpenPitPretradePreTradePolicy* raw = nullptr;
    if constexpr (UsesDryRunHooks()) {
      raw = openpit_create_pretrade_custom_pre_trade_policy_with_dry_run(
          ::openpit::detail::MakeStringView(name), policyGroupId, StartHook(),
          StartDryRunHook(), MainHook(), MainDryRunHook(), ReportHook(),
          AdjustmentHook(), &FreeTrampoline, m_handler.get(), &error);
    } else {
      raw = openpit_create_pretrade_custom_pre_trade_policy(
          ::openpit::detail::MakeStringView(name), policyGroupId, StartHook(),
          MainHook(), ReportHook(), AdjustmentHook(), &FreeTrampoline,
          m_handler.get(), &error);
    }
    if (raw == nullptr) {
      ::openpit::detail::ThrowFromSharedString(error,
                                               "custom policy creation failed");
    }
    // The native runtime now owns the handler lifetime through the free
    // callback.
    static_cast<void>(m_handler.release());
    m_policy.Reset(raw);
  }

  [[nodiscard]] explicit operator bool() const noexcept {
    return static_cast<bool>(m_policy);
  }

  // The stable registered policy name.
  [[nodiscard]] std::string Name() const {
    return ::openpit::detail::FromNative<::openpit::StringView>(
               openpit_pretrade_pre_trade_policy_get_name(m_policy.Get()))
        .ToString();
  }

 private:
  friend class ::openpit::detail::NativeAccess;

  [[nodiscard]] OpenPitPretradePreTradePolicy* Native() const noexcept {
    return m_policy.Get();
  }

  static constexpr bool UsesDryRunHooks() noexcept {
    return detail::HasCheckPreTradeStartDryRun<Handler>::value ||
           detail::HasPerformPreTradeCheckDryRun<Handler>::value ||
           detail::HasPerformPreTradeCheckDryRunFull<Handler>::value;
  }

  static OpenPitPretradePreTradePolicyCheckPreTradeStartFn StartHook() {
    if constexpr (detail::HasCheckPreTradeStartWithContext<Handler>::value ||
                  detail::HasCheckPreTradeStart<Handler>::value) {
      return &CheckStartTrampoline;
    } else {
      return nullptr;
    }
  }

  static OpenPitPretradePreTradePolicyCheckPreTradeStartFn StartDryRunHook() {
    if constexpr (detail::HasCheckPreTradeStartDryRunWithContext<
                      Handler>::value ||
                  detail::HasCheckPreTradeStartDryRun<Handler>::value) {
      return &CheckStartDryRunTrampoline;
    } else {
      return nullptr;
    }
  }

  static OpenPitPretradePreTradePolicyPerformPreTradeCheckFn MainHook() {
    if constexpr (detail::HasPerformPreTradeCheckFull<Handler>::value ||
                  detail::HasPerformPreTradeCheck<Handler>::value) {
      return &PerformCheckTrampoline;
    } else {
      return nullptr;
    }
  }

  static OpenPitPretradePreTradePolicyPerformPreTradeCheckFn MainDryRunHook() {
    if constexpr (detail::HasPerformPreTradeCheckDryRunFull<Handler>::value ||
                  detail::HasPerformPreTradeCheckDryRun<Handler>::value) {
      return &PerformCheckDryRunTrampoline;
    } else {
      return nullptr;
    }
  }

  static OpenPitPretradePreTradePolicyApplyExecutionReportFn ReportHook() {
    if constexpr (detail::HasApplyExecutionReportFull<Handler>::value ||
                  detail::HasApplyExecutionReport<Handler>::value) {
      return &ApplyReportTrampoline;
    } else {
      return nullptr;
    }
  }

  static OpenPitPretradePreTradePolicyApplyAccountAdjustmentFn
  AdjustmentHook() {
    if constexpr (detail::HasApplyAccountAdjustment<Handler>::value) {
      return &ApplyAdjustmentTrampoline;
    } else {
      return nullptr;
    }
  }

  // ctx is the borrowed native context; order is a borrowed C order view. When
  // the pipeline runs synchronously from a polymorphic submission, the original
  // `openpit::Order` is recovered from the thread-local so the adapter can
  // `dynamic_cast` to the client order type; otherwise the order is rebuilt
  // from the C view.
  static OpenPitPretradeRejectList* CheckStartTrampoline(
      const OpenPitPretradeContext* ctx, const OpenPitOrder* order,
      void* userData) noexcept {
    try {
      const auto* handler = static_cast<const Handler*>(userData);
      const ::openpit::Order* original =
          ::openpit::detail::CurrentSubmittedOrder();
      std::optional<::openpit::model::Order> parsed;
      if (original == nullptr) {
        parsed = ::openpit::detail::FromNative<::openpit::model::Order>(*order);
        original = &*parsed;
      }
      std::optional<Reject> reject;
      if constexpr (detail::HasCheckPreTradeStartWithContext<Handler>::value) {
        const Context context = ::openpit::detail::FromNative<Context>(
            detail::ContextInit{*original, ctx});
        reject = handler->CheckPreTradeStart(context, *original);
      } else {
        reject = handler->CheckPreTradeStart(*original);
      }
      if (!reject) {
        return detail::CustomPolicyAccess::AcceptList();
      }
      return detail::CustomPolicyAccess::RejectToList(*reject);
    } catch (...) {
      ::openpit::detail::CaptureCurrentCallbackException();
      return detail::CustomPolicyAccess::NoThrowCallbackErrorRejectList();
    }
  }

  static OpenPitPretradeRejectList* CheckStartDryRunTrampoline(
      const OpenPitPretradeContext* ctx, const OpenPitOrder* order,
      void* userData) noexcept {
    try {
      const auto* handler = static_cast<const Handler*>(userData);
      const ::openpit::Order* original =
          ::openpit::detail::CurrentSubmittedOrder();
      std::optional<::openpit::model::Order> parsed;
      if (original == nullptr) {
        parsed = ::openpit::detail::FromNative<::openpit::model::Order>(*order);
        original = &*parsed;
      }
      std::optional<Reject> reject;
      if constexpr (detail::HasCheckPreTradeStartDryRunWithContext<
                        Handler>::value) {
        const Context context = ::openpit::detail::FromNative<Context>(
            detail::ContextInit{*original, ctx});
        reject = handler->CheckPreTradeStartDryRun(context, *original);
      } else {
        reject = handler->CheckPreTradeStartDryRun(*original);
      }
      if (!reject) {
        return detail::CustomPolicyAccess::AcceptList();
      }
      return detail::CustomPolicyAccess::RejectToList(*reject);
    } catch (...) {
      ::openpit::detail::CaptureCurrentCallbackException();
      return detail::CustomPolicyAccess::NoThrowCallbackErrorRejectList();
    }
  }

  static OpenPitPretradeRejectList* PerformCheckTrampoline(
      const OpenPitPretradeContext* ctx, const OpenPitOrder* order,
      OpenPitMutations* mutations, OpenPitPretradePreTradeResult* outResult,
      void* userData) noexcept {
    try {
      const auto* handler = static_cast<const Handler*>(userData);
      const ::openpit::Order* original =
          ::openpit::detail::CurrentSubmittedOrder();
      std::optional<::openpit::model::Order> parsed;
      if (original == nullptr) {
        parsed = ::openpit::detail::FromNative<::openpit::model::Order>(*order);
        original = &*parsed;
      }
      const Context context = ::openpit::detail::FromNative<Context>(
          detail::ContextInit{*original, ctx});
      ::openpit::tx::Mutations mutationCollector =
          ::openpit::detail::FromNative<::openpit::tx::Mutations>(mutations);
      Result result = ::openpit::detail::FromNative<Result>(outResult);
      PolicyDecision decision;
      if constexpr (detail::HasPerformPreTradeCheckFull<Handler>::value) {
        handler->PerformPreTradeCheck(context, mutationCollector, result,
                                      decision);
      } else {
        handler->PerformPreTradeCheck(context, decision);
      }
      return detail::CustomPolicyAccess::DecisionToList(decision);
    } catch (...) {
      ::openpit::detail::CaptureCurrentCallbackException();
      return detail::CustomPolicyAccess::NoThrowCallbackErrorRejectList();
    }
  }

  static OpenPitPretradeRejectList* PerformCheckDryRunTrampoline(
      const OpenPitPretradeContext* ctx, const OpenPitOrder* order,
      OpenPitMutations* mutations, OpenPitPretradePreTradeResult* outResult,
      void* userData) noexcept {
    try {
      const auto* handler = static_cast<const Handler*>(userData);
      const ::openpit::Order* original =
          ::openpit::detail::CurrentSubmittedOrder();
      std::optional<::openpit::model::Order> parsed;
      if (original == nullptr) {
        parsed = ::openpit::detail::FromNative<::openpit::model::Order>(*order);
        original = &*parsed;
      }
      const Context context = ::openpit::detail::FromNative<Context>(
          detail::ContextInit{*original, ctx});
      ::openpit::tx::Mutations mutationCollector =
          ::openpit::detail::FromNative<::openpit::tx::Mutations>(mutations);
      Result result = ::openpit::detail::FromNative<Result>(outResult);
      PolicyDecision decision;
      if constexpr (detail::HasPerformPreTradeCheckDryRunFull<Handler>::value) {
        handler->PerformPreTradeCheckDryRun(context, mutationCollector, result,
                                            decision);
      } else {
        handler->PerformPreTradeCheckDryRun(context, decision);
      }
      return detail::CustomPolicyAccess::DecisionToList(decision);
    } catch (...) {
      ::openpit::detail::CaptureCurrentCallbackException();
      return detail::CustomPolicyAccess::NoThrowCallbackErrorRejectList();
    }
  }

  static OpenPitPretradeAccountBlockList* ApplyReportTrampoline(
      const OpenPitPostTradeContext* ctx, const OpenPitExecutionReport* report,
      OpenPitPostTradeAdjustmentList* outAdjustments,
      OpenPitPostTradeAccountPnlList* outAccountPnls, void* userData) noexcept {
    try {
      const auto* handler = static_cast<const Handler*>(userData);
      const ::openpit::ExecutionReport* original =
          ::openpit::detail::CurrentSubmittedReport();
      std::optional<::openpit::model::ExecutionReport> parsed;
      if (original == nullptr) {
        parsed =
            ::openpit::detail::FromNative<::openpit::model::ExecutionReport>(
                *report);
        original = &*parsed;
      }
      if constexpr (detail::HasApplyExecutionReportFull<Handler>::value) {
        const PostTradeContext context =
            ::openpit::detail::FromNative<PostTradeContext>(ctx);
        PostTradeAdjustments adjustments =
            ::openpit::detail::FromNative<PostTradeAdjustments>(outAdjustments);
        PostTradePnls pnls =
            ::openpit::detail::FromNative<PostTradePnls>(outAccountPnls);
        return detail::CustomPolicyAccess::AccountBlocksToList(
            handler->ApplyExecutionReport(context, *original, adjustments,
                                          pnls));
      } else {
        // Legacy report hooks were boolean notifications and could not return
        // a structured block. Preserve their notification behavior.
        static_cast<void>(handler->ApplyExecutionReport(*original));
        return nullptr;
      }
    } catch (...) {
      ::openpit::detail::CaptureCurrentCallbackException();
      return detail::CustomPolicyAccess::NoThrowCallbackErrorAccountBlockList();
    }
  }

  static OpenPitPretradeRejectList* ApplyAdjustmentTrampoline(
      const OpenPitAccountAdjustmentContext* ctx,
      OpenPitParamAccountId accountId,
      const OpenPitAccountAdjustment* adjustment, OpenPitMutations* mutations,
      OpenPitPretradeAccountAdjustmentResult* outResult,
      void* userData) noexcept {
    try {
      const auto* handler = static_cast<const Handler*>(userData);
      const ::openpit::accountadjustment::Context context =
          ::openpit::detail::FromNative<::openpit::accountadjustment::Context>(
              ctx);
      const ::openpit::accountadjustment::AccountAdjustment parsed =
          ::openpit::detail::FromNative<
              ::openpit::accountadjustment::AccountAdjustment>(*adjustment);
      ::openpit::tx::Mutations mutationCollector =
          ::openpit::detail::FromNative<::openpit::tx::Mutations>(mutations);
      AccountOutcomes outcomes =
          ::openpit::detail::FromNative<AccountOutcomes>(outResult);
      const PolicyAccountAdjustmentResult result =
          handler->ApplyAccountAdjustment(
              context,
              ::openpit::detail::FromNative<::openpit::param::AccountId>(
                  accountId),
              parsed, mutationCollector, outcomes);
      for (const auto& block : result.accountBlocks) {
        openpit_pretrade_account_adjustment_result_push_account_block(
            outResult, ::openpit::detail::Native(block));
      }
      return detail::CustomPolicyAccess::DecisionToList(result.decision);
    } catch (...) {
      ::openpit::detail::CaptureCurrentCallbackException();
      return detail::CustomPolicyAccess::NoThrowCallbackErrorRejectList();
    }
  }

  static void FreeTrampoline(void* userData) noexcept {
    delete static_cast<Handler*>(userData);
  }

  // Owns the handler only until the native runtime adopts it in the
  // constructor; after a successful create it is released (the free callback
  // owns it thereafter).
  std::unique_ptr<Handler> m_handler;
  ::openpit::detail::Handle<OpenPitPretradePreTradePolicy,
                            detail::PreTradePolicyDeleter>
      m_policy;
};

}  // namespace openpit::pretrade
