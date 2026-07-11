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

#include "openpit/account_adjustment.hpp"
#include "openpit/accounts.hpp"
#include "openpit/detail/callback_error.hpp"
#include "openpit/detail/handle.hpp"
#include "openpit/error.hpp"
#include "openpit/model.hpp"
#include "openpit/pretrade/callbacks.hpp"
#include "openpit/pretrade/context.hpp"
#include "openpit/reject.hpp"
#include "openpit/string.hpp"
#include "openpit/tx.hpp"

#include <openpit.h>

#include <cstdint>
#include <memory>
#include <optional>
#include <string>
#include <type_traits>
#include <utility>
#include <vector>

// Custom-policy authoring glue.
//
// `CustomPolicy<Handler>` lets a plain C++ object act as a pre-trade policy
// through the native runtime custom-policy vtable
// (`openpit_create_pretrade_custom_pre_trade_policy`). The C callbacks receive
// borrowed C order/report POD views. During a normal C++ engine call the glue
// recovers the exact submitted polymorphic object from a thread-local guard;
// otherwise it copies the view into an owned `openpit::model::Order` /
// `openpit::model::ExecutionReport`. It wraps the order in a `Context`,
// dispatches to the handler, and translates the handler's
// `std::optional<Reject>` / `PolicyDecision` outcome back into the C
// reject-list the engine expects.
//
// `Handler` is any object exposing one or more of the methods below. Each hook
// is wired to the native runtime only when the corresponding method is present
// (detected at compile time); an absent hook is registered as null, which the
// engine treats as "accept by default". This lets a `StartPolicyAdapter`
// (start-only), a unified `PolicyAdapter` (main plus any optional stages), or a
// direct handler be wrapped directly:
//   - std::optional<Reject> CheckPreTradeStart(const openpit::Order&) const
//   - std::optional<Reject> CheckPreTradeStartDryRun(const openpit::Order&)
//   const
//   - void PerformPreTradeCheck(const Context&, tx::Mutations&, Result&,
//                              PolicyDecision&) const
//   - void PerformPreTradeCheckDryRun(const Context&, tx::Mutations&, Result&,
//                                    PolicyDecision&) const
//   - std::vector<accounts::AccountBlock> ApplyExecutionReport(
//         const PostTradeContext&, const openpit::ExecutionReport&,
//         PostTradeAdjustments&) const
//   - PolicyDecision ApplyAccountAdjustment(
//         const accountadjustment::Context&, param::AccountId,
//         const accountadjustment::AccountAdjustment&, tx::Mutations&,
//         AccountOutcomes&) const
//
// The policy name is supplied to `CustomPolicy` itself. A direct handler does
// not need a `Name()` method. Adapter handlers do expose `Name()` through their
// client policy; when present, it must match the constructor name so runtime
// registration and adapter-produced rejects use one stable identity.
//
// The legacy two-argument main-stage and one-argument report hooks remain
// accepted for source compatibility. They cannot use the added collectors.
//
// When a dry-run hook is present, the C++ binding registers the policy through
// `openpit_create_pretrade_custom_pre_trade_policy_with_dry_run`. Missing
// dry-run hooks are left null so the native runtime delegates them to the
// normal hook.
//
// Handler exceptions never cross the C boundary. The trampoline captures the
// first exception and the owning Engine call rethrows that exact exception
// after native cleanup. SafeSlow adapter payload mismatches remain value
// rejects rather than exceptions.
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

// Translates a value reject into a freshly allocated single-item C reject list.
[[nodiscard]] inline OpenPitPretradeRejectList* RejectToList(
    const Reject& reject) {
  OpenPitPretradeRejectList* list = openpit_pretrade_create_reject_list(1);
  openpit_pretrade_reject_list_push(list, reject.Raw());
  return list;
}

// Translates a decision into a C reject list, or null when the decision accepts
// (no rejects). The push copies the reject strings into the list.
[[nodiscard]] inline OpenPitPretradeRejectList* DecisionToList(
    const PolicyDecision& decision) {
  if (!decision.IsRejected()) {
    return nullptr;
  }
  OpenPitPretradeRejectList* list =
      openpit_pretrade_create_reject_list(decision.rejects.size());
  for (const Reject& reject : decision.rejects) {
    openpit_pretrade_reject_list_push(list, reject.Raw());
  }
  return list;
}

[[nodiscard]] inline OpenPitPretradeAccountBlockList* AccountBlocksToList(
    const std::vector<::openpit::accounts::AccountBlock>& blocks) {
  if (blocks.empty()) {
    return nullptr;
  }
  OpenPitPretradeAccountBlockList* list =
      openpit_pretrade_create_account_block_list(blocks.size());
  for (const auto& block : blocks) {
    openpit_pretrade_account_block_list_push(list, block.Raw());
  }
  return list;
}

[[nodiscard]] inline OpenPitPretradeRejectList* CallbackErrorRejectList() {
  return RejectToList(Reject(
      "openpit.callback", RejectScope::Order, RejectCode::SystemUnavailable,
      "custom policy callback failed", "callback raised an exception"));
}

[[nodiscard]] inline OpenPitPretradeAccountBlockList*
CallbackErrorAccountBlockList() {
  std::string policy = "openpit.callback";
  std::string reason = "custom policy callback failed";
  std::string details = "callback raised an exception";
  OpenPitPretradeAccountBlock block{};
  block.policy = ::openpit::MakeStringView(policy);
  block.reason = ::openpit::MakeStringView(reason);
  block.details = ::openpit::MakeStringView(details);
  block.code = static_cast<OpenPitPretradeRejectCode>(
      static_cast<std::uint16_t>(RejectCode::SystemUnavailable));
  OpenPitPretradeAccountBlockList* list =
      openpit_pretrade_create_account_block_list(1);
  openpit_pretrade_account_block_list_push(list, block);
  return list;
}

// Compile-time detection of each optional handler hook.

template <typename Handler, typename = void>
struct HasCheckPreTradeStart : std::false_type {};
template <typename Handler>
struct HasCheckPreTradeStart<
    Handler,
    std::void_t<decltype(std::declval<const Handler&>().CheckPreTradeStart(
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

/// \brief Owning custom pre-trade policy backed by a C++ `Handler`.
//
// Owning custom pre-trade policy backed by a C++ `Handler`.
//
// The handler is heap-allocated and its address is passed verbatim to the
// native runtime as the opaque `user_data`; the free callback deletes it once
// the last reference (caller or engine) is released.
template <typename Handler>
class CustomPolicy {
  static_assert(detail::HasCheckPreTradeStart<Handler>::value ||
                    detail::HasCheckPreTradeStartDryRun<Handler>::value ||
                    detail::HasPerformPreTradeCheck<Handler>::value ||
                    detail::HasPerformPreTradeCheckFull<Handler>::value ||
                    detail::HasPerformPreTradeCheckDryRun<Handler>::value ||
                    detail::HasPerformPreTradeCheckDryRunFull<Handler>::value ||
                    detail::HasApplyExecutionReport<Handler>::value ||
                    detail::HasApplyExecutionReportFull<Handler>::value ||
                    detail::HasApplyAccountAdjustment<Handler>::value,
                "Handler must expose at least one pre-trade hook");

 public:
  // Creates a policy named `name`, tagged with `policyGroupId`, dispatching to
  // a moved-in `handler`. Throws `openpit::Error` when the native runtime
  // rejects the name.
  CustomPolicy(std::string_view name, Handler handler,
               std::uint16_t policyGroupId = OPENPIT_DEFAULT_POLICY_GROUP_ID)
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
          ::openpit::MakeStringView(name), policyGroupId, StartHook(),
          StartDryRunHook(), MainHook(), MainDryRunHook(), ReportHook(),
          AdjustmentHook(), &FreeTrampoline, m_handler.get(), &error);
    } else {
      raw = openpit_create_pretrade_custom_pre_trade_policy(
          ::openpit::MakeStringView(name), policyGroupId, StartHook(),
          MainHook(), ReportHook(), AdjustmentHook(), &FreeTrampoline,
          m_handler.get(), &error);
    }
    if (raw == nullptr) {
      ::openpit::detail::ThrowFromSharedString(
          error,
          UsesDryRunHooks()
              ? "openpit_create_pretrade_custom_pre_trade_policy_with_dry_run "
                "failed"
              : "openpit_create_pretrade_custom_pre_trade_policy failed");
    }
    // The native runtime now owns the handler lifetime through the free
    // callback.
    static_cast<void>(m_handler.release());
    m_policy.Reset(raw);
  }

  [[nodiscard]] explicit operator bool() const noexcept {
    return static_cast<bool>(m_policy);
  }

  // The stable policy name as the native runtime reports it.
  [[nodiscard]] std::string Name() const {
    return ::openpit::StringView(
               openpit_pretrade_pre_trade_policy_get_name(m_policy.Get()))
        .ToString();
  }

  // Borrows the native policy pointer for registration on the engine builder.
  [[nodiscard]] OpenPitPretradePreTradePolicy* Get() const noexcept {
    return m_policy.Get();
  }

 private:
  static constexpr bool UsesDryRunHooks() noexcept {
    return detail::HasCheckPreTradeStartDryRun<Handler>::value ||
           detail::HasPerformPreTradeCheckDryRun<Handler>::value ||
           detail::HasPerformPreTradeCheckDryRunFull<Handler>::value;
  }

  static OpenPitPretradePreTradePolicyCheckPreTradeStartFn StartHook() {
    if constexpr (detail::HasCheckPreTradeStart<Handler>::value) {
      return &CheckStartTrampoline;
    } else {
      return nullptr;
    }
  }

  static OpenPitPretradePreTradePolicyCheckPreTradeStartFn StartDryRunHook() {
    if constexpr (detail::HasCheckPreTradeStartDryRun<Handler>::value) {
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
      const OpenPitPretradeContext* /*ctx*/, const OpenPitOrder* order,
      void* userData) noexcept {
    try {
      const auto* handler = static_cast<const Handler*>(userData);
      const ::openpit::Order* original =
          ::openpit::detail::CurrentSubmittedOrder();
      std::optional<::openpit::model::Order> parsed;
      if (original == nullptr) {
        parsed = ::openpit::model::Order::FromRaw(*order);
        original = &*parsed;
      }
      const std::optional<Reject> reject =
          handler->CheckPreTradeStart(*original);
      if (!reject) {
        return nullptr;
      }
      return detail::RejectToList(*reject);
    } catch (...) {
      ::openpit::detail::CaptureCurrentCallbackException();
      return detail::CallbackErrorRejectList();
    }
  }

  static OpenPitPretradeRejectList* CheckStartDryRunTrampoline(
      const OpenPitPretradeContext* /*ctx*/, const OpenPitOrder* order,
      void* userData) noexcept {
    try {
      const auto* handler = static_cast<const Handler*>(userData);
      const ::openpit::Order* original =
          ::openpit::detail::CurrentSubmittedOrder();
      std::optional<::openpit::model::Order> parsed;
      if (original == nullptr) {
        parsed = ::openpit::model::Order::FromRaw(*order);
        original = &*parsed;
      }
      const std::optional<Reject> reject =
          handler->CheckPreTradeStartDryRun(*original);
      if (!reject) {
        return nullptr;
      }
      return detail::RejectToList(*reject);
    } catch (...) {
      ::openpit::detail::CaptureCurrentCallbackException();
      return detail::CallbackErrorRejectList();
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
        parsed = ::openpit::model::Order::FromRaw(*order);
        original = &*parsed;
      }
      const Context context(*original, ctx);
      ::openpit::tx::Mutations mutationCollector(mutations);
      Result result(outResult);
      PolicyDecision decision;
      if constexpr (detail::HasPerformPreTradeCheckFull<Handler>::value) {
        handler->PerformPreTradeCheck(context, mutationCollector, result,
                                      decision);
      } else {
        handler->PerformPreTradeCheck(context, decision);
      }
      return detail::DecisionToList(decision);
    } catch (...) {
      ::openpit::detail::CaptureCurrentCallbackException();
      return detail::CallbackErrorRejectList();
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
        parsed = ::openpit::model::Order::FromRaw(*order);
        original = &*parsed;
      }
      const Context context(*original, ctx);
      ::openpit::tx::Mutations mutationCollector(mutations);
      Result result(outResult);
      PolicyDecision decision;
      if constexpr (detail::HasPerformPreTradeCheckDryRunFull<Handler>::value) {
        handler->PerformPreTradeCheckDryRun(context, mutationCollector, result,
                                            decision);
      } else {
        handler->PerformPreTradeCheckDryRun(context, decision);
      }
      return detail::DecisionToList(decision);
    } catch (...) {
      ::openpit::detail::CaptureCurrentCallbackException();
      return detail::CallbackErrorRejectList();
    }
  }

  static OpenPitPretradeAccountBlockList* ApplyReportTrampoline(
      const OpenPitPostTradeContext* ctx, const OpenPitExecutionReport* report,
      OpenPitPostTradeAdjustmentList* outAdjustments, void* userData) noexcept {
    try {
      const auto* handler = static_cast<const Handler*>(userData);
      const ::openpit::ExecutionReport* original =
          ::openpit::detail::CurrentSubmittedReport();
      std::optional<::openpit::model::ExecutionReport> parsed;
      if (original == nullptr) {
        parsed = ::openpit::model::ExecutionReport::FromRaw(*report);
        original = &*parsed;
      }
      if constexpr (detail::HasApplyExecutionReportFull<Handler>::value) {
        const PostTradeContext context(ctx);
        PostTradeAdjustments adjustments(outAdjustments);
        return detail::AccountBlocksToList(
            handler->ApplyExecutionReport(context, *original, adjustments));
      } else {
        // Legacy report hooks were boolean notifications and could not return
        // a structured block. Preserve their notification behavior.
        static_cast<void>(handler->ApplyExecutionReport(*original));
        return nullptr;
      }
    } catch (...) {
      ::openpit::detail::CaptureCurrentCallbackException();
      return detail::CallbackErrorAccountBlockList();
    }
  }

  static OpenPitPretradeRejectList* ApplyAdjustmentTrampoline(
      const OpenPitAccountAdjustmentContext* ctx,
      OpenPitParamAccountId accountId,
      const OpenPitAccountAdjustment* adjustment, OpenPitMutations* mutations,
      OpenPitAccountOutcomeEntryList* outOutcomes, void* userData) noexcept {
    try {
      const auto* handler = static_cast<const Handler*>(userData);
      const ::openpit::accountadjustment::Context context(ctx);
      const ::openpit::accountadjustment::AccountAdjustment parsed =
          ::openpit::accountadjustment::AccountAdjustment::FromRaw(*adjustment);
      ::openpit::tx::Mutations mutationCollector(mutations);
      AccountOutcomes outcomes(outOutcomes);
      const PolicyDecision decision = handler->ApplyAccountAdjustment(
          context, ::openpit::param::AccountId::FromRaw(accountId), parsed,
          mutationCollector, outcomes);
      return detail::DecisionToList(decision);
    } catch (...) {
      ::openpit::detail::CaptureCurrentCallbackException();
      return detail::CallbackErrorRejectList();
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
