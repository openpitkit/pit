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
#include "openpit/accounts.hpp"
#include "openpit/pretrade/callbacks.hpp"
#include "openpit/reject.hpp"
#include "openpit/tx.hpp"

#include <cstdint>
#include <optional>
#include <string_view>
#include <type_traits>
#include <typeinfo>
#include <utility>
#include <vector>

namespace openpit {
class Order;
class ExecutionReport;
}  // namespace openpit

namespace openpit::pretrade {

// Adapter wrappers for client-defined policy types.
//
// This header demonstrates how to bridge client order/report payload types to
// openpit policy contracts with explicit cast strategy selection.

class Context;
struct PolicyDecision;

namespace detail {

template <typename Policy, typename Order, typename = void>
struct HasMainFull : std::false_type {};

template <typename Policy, typename Order>
struct HasMainFull<
    Policy, Order,
    std::void_t<decltype(std::declval<const Policy&>().PerformPreTradeCheck(
        std::declval<const Order&>(), std::declval<const Context&>(),
        std::declval<::openpit::tx::Mutations&>(), std::declval<Result&>(),
        std::declval<PolicyDecision&>()))>> : std::true_type {};

template <typename Policy, typename Order, typename = void>
struct HasMainDryRunFull : std::false_type {};

template <typename Policy, typename Order>
struct HasMainDryRunFull<
    Policy, Order,
    std::void_t<
        decltype(std::declval<const Policy&>().PerformPreTradeCheckDryRun(
            std::declval<const Order&>(), std::declval<const Context&>(),
            std::declval<::openpit::tx::Mutations&>(), std::declval<Result&>(),
            std::declval<PolicyDecision&>()))>> : std::true_type {};

template <typename Policy, typename Order, typename = void>
struct HasMainDryRunLegacy : std::false_type {};

template <typename Policy, typename Order>
struct HasMainDryRunLegacy<
    Policy, Order,
    std::void_t<
        decltype(std::declval<const Policy&>().PerformPreTradeCheckDryRun(
            std::declval<const Order&>(), std::declval<const Context&>(),
            std::declval<PolicyDecision&>()))>> : std::true_type {};

template <typename Policy, typename Report, typename = void>
struct HasReportFull : std::false_type {};

template <typename Policy, typename Report>
struct HasReportFull<
    Policy, Report,
    std::void_t<decltype(std::declval<const Policy&>().ApplyExecutionReport(
        std::declval<const PostTradeContext&>(), std::declval<const Report&>(),
        std::declval<PostTradeAdjustments&>(),
        std::declval<PostTradePnls&>()))>> : std::true_type {};

template <typename Policy, typename Report, typename = void>
struct HasReportLegacy : std::false_type {};

template <typename Policy, typename Report>
struct HasReportLegacy<
    Policy, Report,
    std::void_t<decltype(std::declval<const Policy&>().ApplyExecutionReport(
        std::declval<const Report&>()))>> : std::true_type {};

// Detects the removed three-argument post-trade form. Matching it is an error:
// without it the adapter would silently expose no report hook at all.
template <typename Policy, typename Report, typename = void>
struct HasLegacyReportFull : std::false_type {};

template <typename Policy, typename Report>
struct HasLegacyReportFull<
    Policy, Report,
    std::void_t<decltype(std::declval<const Policy&>().ApplyExecutionReport(
        std::declval<const PostTradeContext&>(), std::declval<const Report&>(),
        std::declval<PostTradeAdjustments&>()))>> : std::true_type {};

template <typename Policy, typename = void>
struct HasAdjustment : std::false_type {};

template <typename Policy>
struct HasAdjustment<
    Policy,
    std::void_t<decltype(std::declval<const Policy&>().ApplyAccountAdjustment(
        std::declval<const ::openpit::accountadjustment::Context&>(),
        std::declval<::openpit::param::AccountId>(),
        std::declval<const ::openpit::accountadjustment::AccountAdjustment&>(),
        std::declval<::openpit::tx::Mutations&>(),
        std::declval<AccountOutcomes&>()))>> : std::true_type {};

template <typename Policy, typename Order, typename = void>
struct HasStart : std::false_type {};

template <typename Policy, typename Order>
struct HasStart<
    Policy, Order,
    std::void_t<decltype(std::declval<const Policy&>().CheckPreTradeStart(
        std::declval<const Order&>()))>> : std::true_type {};

template <typename Policy, typename Order, typename = void>
struct HasStartDryRun : std::false_type {};

template <typename Policy, typename Order>
struct HasStartDryRun<
    Policy, Order,
    std::void_t<decltype(std::declval<const Policy&>().CheckPreTradeStartDryRun(
        std::declval<const Order&>()))>> : std::true_type {};

}  // namespace detail

// Implemented by binding layer.
[[nodiscard]] Reject MakeTypeMismatchReject(
    std::string_view policy_name, RejectScope scope, RejectCode code,
    std::string_view reason, std::string_view expected_type_name);

// Implemented by binding layer.
void PushReject(PolicyDecision& decision, Reject reject);

// Implemented by binding layer.
[[nodiscard]] const openpit::Order& ContextOrder(const Context& context);

// Cast strategy for adapter wrappers.
//
// `SafeSlow`:
// - Uses `dynamic_cast` to verify runtime type compatibility.
// - Produces deterministic reject on order mismatch.
// - Returns no account blocks on report mismatch.
// - Risk profile: safe default at dynamic boundaries.
//
// `UnsafeFast`:
// - Uses direct `static_cast` without runtime verification.
// - Avoids runtime RTTI checks.
// - Wrong wiring is undefined behavior.
// - Risk profile: only for closed systems with compile-time pairing guarantees.
enum class CastMode : std::uint8_t {
  SafeSlow,
  UnsafeFast,
};

/// \brief Adapts a client start-stage policy to the engine callback seam.
//
// Start-stage adapter for client policy object.
//
// Why this adapter exists:
// - Keeps client policy logic in client payload types.
// - Bridges to callback signatures expected by the engine.
// - Centralizes cast policy for order/report conversion.
//
// There is intentionally no default cast strategy.
// Policy author must choose `SafeSlow` or `UnsafeFast` explicitly.
template <typename ClientPolicy, typename ClientOrder, typename ClientReport,
          CastMode mode>
class StartPolicyAdapter {
  static_assert(
      !detail::HasLegacyReportFull<ClientPolicy, ClientReport>::value,
      "ClientPolicy::ApplyExecutionReport(context, report, adjustments) was "
      "removed; add PostTradePnls& as the fourth argument");

 public:
  // Creates adapter around a client start-stage policy instance.
  explicit StartPolicyAdapter(ClientPolicy policy)
      : m_policy(std::move(policy)) {}

  // Returns stable policy name forwarded from client policy object.
  [[nodiscard]] std::string_view Name() const noexcept {
    return m_policy.Name();
  }

  // Adapts openpit order callback to client order type.
  //
  // SafeSlow:
  // - type mismatch -> deterministic reject
  //
  // UnsafeFast:
  // - direct cast, mismatch is undefined behavior
  [[nodiscard]] std::optional<Reject> CheckPreTradeStart(
      const openpit::Order& order) const {
    if constexpr (mode == CastMode::SafeSlow) {
      const auto* concrete_order = dynamic_cast<const ClientOrder*>(&order);
      if (concrete_order == nullptr) {
        return MakeTypeMismatchReject(Name(), RejectScope::Order,
                                      RejectCode::Other, "order type mismatch",
                                      typeid(ClientOrder).name());
      }
      return m_policy.CheckPreTradeStart(*concrete_order);
    } else {
      return m_policy.CheckPreTradeStart(
          static_cast<const ClientOrder&>(order));
    }
  }

  template <
      typename P = ClientPolicy,
      std::enable_if_t<detail::HasStartDryRun<P, ClientOrder>::value, int> = 0>
  [[nodiscard]] std::optional<Reject> CheckPreTradeStartDryRun(
      const openpit::Order& order) const {
    if constexpr (mode == CastMode::SafeSlow) {
      const auto* concrete_order = dynamic_cast<const ClientOrder*>(&order);
      if (concrete_order == nullptr) {
        return MakeTypeMismatchReject(Name(), RejectScope::Order,
                                      RejectCode::Other, "order type mismatch",
                                      typeid(ClientOrder).name());
      }
      return m_policy.CheckPreTradeStartDryRun(*concrete_order);
    } else {
      return m_policy.CheckPreTradeStartDryRun(
          static_cast<const ClientOrder&>(order));
    }
  }

  // Adapts execution-report callback to client report type.
  //
  // SafeSlow:
  // - type mismatch -> empty account-block list
  //
  // UnsafeFast:
  // - direct cast, mismatch is undefined behavior
  template <
      typename P = ClientPolicy,
      std::enable_if_t<detail::HasReportFull<P, ClientReport>::value ||
                           detail::HasReportLegacy<P, ClientReport>::value,
                       int> = 0>
  [[nodiscard]] std::vector<::openpit::accounts::AccountBlock>
  ApplyExecutionReport(const PostTradeContext& context,
                       const openpit::ExecutionReport& report,
                       PostTradeAdjustments& adjustments,
                       PostTradePnls& pnls) const {
    if constexpr (mode == CastMode::SafeSlow) {
      const auto* concrete_report = dynamic_cast<const ClientReport*>(&report);
      if (concrete_report == nullptr) {
        return {};
      }
      if constexpr (detail::HasReportFull<P, ClientReport>::value) {
        return m_policy.ApplyExecutionReport(context, *concrete_report,
                                             adjustments, pnls);
      } else if constexpr (detail::HasReportLegacy<P, ClientReport>::value) {
        static_cast<void>(m_policy.ApplyExecutionReport(*concrete_report));
        return {};
      } else {
        return {};
      }
    } else {
      const auto& concrete_report = static_cast<const ClientReport&>(report);
      if constexpr (detail::HasReportFull<P, ClientReport>::value) {
        return m_policy.ApplyExecutionReport(context, concrete_report,
                                             adjustments, pnls);
      } else if constexpr (detail::HasReportLegacy<P, ClientReport>::value) {
        static_cast<void>(m_policy.ApplyExecutionReport(concrete_report));
        return {};
      } else {
        return {};
      }
    }
  }

  template <typename P = ClientPolicy,
            std::enable_if_t<detail::HasReportLegacy<P, ClientReport>::value,
                             int> = 0>
  [[nodiscard]] bool ApplyExecutionReport(
      const openpit::ExecutionReport& report) const {
    if constexpr (mode == CastMode::SafeSlow) {
      const auto* concrete_report = dynamic_cast<const ClientReport*>(&report);
      return concrete_report != nullptr &&
             m_policy.ApplyExecutionReport(*concrete_report);
    } else {
      return m_policy.ApplyExecutionReport(
          static_cast<const ClientReport&>(report));
    }
  }

  template <typename P = ClientPolicy,
            std::enable_if_t<detail::HasAdjustment<P>::value, int> = 0>
  [[nodiscard]] PolicyAccountAdjustmentResult ApplyAccountAdjustment(
      const ::openpit::accountadjustment::Context& context,
      ::openpit::param::AccountId accountId,
      const ::openpit::accountadjustment::AccountAdjustment& adjustment,
      ::openpit::tx::Mutations& mutations, AccountOutcomes& outcomes) const {
    return m_policy.ApplyAccountAdjustment(context, accountId, adjustment,
                                           mutations, outcomes);
  }

 private:
  ClientPolicy m_policy;
};

// Unified adapter for a client policy with a main-stage hook. Optional start,
// dry-run, post-trade, and account-adjustment hooks on the same object are
// forwarded through the same native policy registration.
//
// Why this adapter exists:
// - Keeps main-stage client policy API typed to client payloads.
// - Bridges to `Context` / `PolicyDecision` callbacks.
// - Encapsulates cast strategy in one place.
//
// There is intentionally no default cast strategy.
// Policy author must choose `SafeSlow` or `UnsafeFast` explicitly.
template <typename ClientPolicy, typename ClientOrder, typename ClientReport,
          CastMode mode>
class PolicyAdapter {
  static_assert(
      !detail::HasLegacyReportFull<ClientPolicy, ClientReport>::value,
      "ClientPolicy::ApplyExecutionReport(context, report, adjustments) was "
      "removed; add PostTradePnls& as the fourth argument");

 public:
  // Creates adapter around a client main-stage policy instance.
  explicit PolicyAdapter(ClientPolicy policy) : m_policy(std::move(policy)) {}

  // Returns stable policy name forwarded from client policy object.
  [[nodiscard]] std::string_view Name() const noexcept {
    return m_policy.Name();
  }

  // A main-stage adapter is also the unified adapter for any optional start
  // hooks exposed by the same client policy. This keeps all stages and their
  // policy state behind one native registration.
  template <typename P = ClientPolicy,
            std::enable_if_t<detail::HasStart<P, ClientOrder>::value, int> = 0>
  [[nodiscard]] std::optional<Reject> CheckPreTradeStart(
      const openpit::Order& order) const {
    if constexpr (mode == CastMode::SafeSlow) {
      const auto* concrete_order = dynamic_cast<const ClientOrder*>(&order);
      if (concrete_order == nullptr) {
        return MakeTypeMismatchReject(Name(), RejectScope::Order,
                                      RejectCode::Other, "order type mismatch",
                                      typeid(ClientOrder).name());
      }
      return m_policy.CheckPreTradeStart(*concrete_order);
    } else {
      return m_policy.CheckPreTradeStart(
          static_cast<const ClientOrder&>(order));
    }
  }

  template <
      typename P = ClientPolicy,
      std::enable_if_t<detail::HasStartDryRun<P, ClientOrder>::value, int> = 0>
  [[nodiscard]] std::optional<Reject> CheckPreTradeStartDryRun(
      const openpit::Order& order) const {
    if constexpr (mode == CastMode::SafeSlow) {
      const auto* concrete_order = dynamic_cast<const ClientOrder*>(&order);
      if (concrete_order == nullptr) {
        return MakeTypeMismatchReject(Name(), RejectScope::Order,
                                      RejectCode::Other, "order type mismatch",
                                      typeid(ClientOrder).name());
      }
      return m_policy.CheckPreTradeStartDryRun(*concrete_order);
    } else {
      return m_policy.CheckPreTradeStartDryRun(
          static_cast<const ClientOrder&>(order));
    }
  }

  // Adapts main-stage callback to client order type and decision object.
  //
  // SafeSlow:
  // - order type mismatch -> deterministic reject pushed into decision
  //
  // UnsafeFast:
  // - direct cast, mismatch is undefined behavior
  void PerformPreTradeCheck(const Context& context,
                            PolicyDecision& decision) const {
    const openpit::Order& order = ContextOrder(context);
    if constexpr (mode == CastMode::SafeSlow) {
      const auto* concrete_order = dynamic_cast<const ClientOrder*>(&order);
      if (concrete_order == nullptr) {
        PushReject(decision,
                   MakeTypeMismatchReject(
                       Name(), RejectScope::Order, RejectCode::Other,
                       "order type mismatch", typeid(ClientOrder).name()));
        return;
      }
      m_policy.PerformPreTradeCheck(*concrete_order, context, decision);
    } else {
      m_policy.PerformPreTradeCheck(static_cast<const ClientOrder&>(order),
                                    context, decision);
    }
  }

  void PerformPreTradeCheck(const Context& context,
                            ::openpit::tx::Mutations& mutations, Result& result,
                            PolicyDecision& decision) const {
    const openpit::Order& order = ContextOrder(context);
    if constexpr (mode == CastMode::SafeSlow) {
      const auto* concrete_order = dynamic_cast<const ClientOrder*>(&order);
      if (concrete_order == nullptr) {
        PushReject(decision,
                   MakeTypeMismatchReject(
                       Name(), RejectScope::Order, RejectCode::Other,
                       "order type mismatch", typeid(ClientOrder).name()));
        return;
      }
      if constexpr (detail::HasMainFull<ClientPolicy, ClientOrder>::value) {
        m_policy.PerformPreTradeCheck(*concrete_order, context, mutations,
                                      result, decision);
      } else {
        m_policy.PerformPreTradeCheck(*concrete_order, context, decision);
      }
    } else {
      const auto& concrete_order = static_cast<const ClientOrder&>(order);
      if constexpr (detail::HasMainFull<ClientPolicy, ClientOrder>::value) {
        m_policy.PerformPreTradeCheck(concrete_order, context, mutations,
                                      result, decision);
      } else {
        m_policy.PerformPreTradeCheck(concrete_order, context, decision);
      }
    }
  }

  template <
      typename P = ClientPolicy,
      std::enable_if_t<detail::HasMainDryRunFull<P, ClientOrder>::value ||
                           detail::HasMainDryRunLegacy<P, ClientOrder>::value,
                       int> = 0>
  void PerformPreTradeCheckDryRun(const Context& context,
                                  ::openpit::tx::Mutations& mutations,
                                  Result& result,
                                  PolicyDecision& decision) const {
    const openpit::Order& order = ContextOrder(context);
    if constexpr (mode == CastMode::SafeSlow) {
      const auto* concrete_order = dynamic_cast<const ClientOrder*>(&order);
      if (concrete_order == nullptr) {
        PushReject(decision,
                   MakeTypeMismatchReject(
                       Name(), RejectScope::Order, RejectCode::Other,
                       "order type mismatch", typeid(ClientOrder).name()));
        return;
      }
      if constexpr (detail::HasMainDryRunFull<P, ClientOrder>::value) {
        m_policy.PerformPreTradeCheckDryRun(*concrete_order, context, mutations,
                                            result, decision);
      } else {
        m_policy.PerformPreTradeCheckDryRun(*concrete_order, context, decision);
      }
    } else {
      const auto& concrete_order = static_cast<const ClientOrder&>(order);
      if constexpr (detail::HasMainDryRunFull<P, ClientOrder>::value) {
        m_policy.PerformPreTradeCheckDryRun(concrete_order, context, mutations,
                                            result, decision);
      } else {
        m_policy.PerformPreTradeCheckDryRun(concrete_order, context, decision);
      }
    }
  }

  // Adapts execution-report callback to client report type.
  //
  // SafeSlow:
  // - type mismatch -> empty account-block list
  //
  // UnsafeFast:
  // - direct cast, mismatch is undefined behavior
  template <
      typename P = ClientPolicy,
      std::enable_if_t<detail::HasReportFull<P, ClientReport>::value ||
                           detail::HasReportLegacy<P, ClientReport>::value,
                       int> = 0>
  [[nodiscard]] std::vector<::openpit::accounts::AccountBlock>
  ApplyExecutionReport(const PostTradeContext& context,
                       const openpit::ExecutionReport& report,
                       PostTradeAdjustments& adjustments,
                       PostTradePnls& pnls) const {
    if constexpr (mode == CastMode::SafeSlow) {
      const auto* concrete_report = dynamic_cast<const ClientReport*>(&report);
      if (concrete_report == nullptr) {
        return {};
      }
      if constexpr (detail::HasReportFull<P, ClientReport>::value) {
        return m_policy.ApplyExecutionReport(context, *concrete_report,
                                             adjustments, pnls);
      } else {
        static_cast<void>(m_policy.ApplyExecutionReport(*concrete_report));
        return {};
      }
    } else {
      const auto& concrete_report = static_cast<const ClientReport&>(report);
      if constexpr (detail::HasReportFull<P, ClientReport>::value) {
        return m_policy.ApplyExecutionReport(context, concrete_report,
                                             adjustments, pnls);
      } else {
        static_cast<void>(m_policy.ApplyExecutionReport(concrete_report));
        return {};
      }
    }
  }

  template <typename P = ClientPolicy,
            std::enable_if_t<detail::HasReportLegacy<P, ClientReport>::value,
                             int> = 0>
  [[nodiscard]] bool ApplyExecutionReport(
      const openpit::ExecutionReport& report) const {
    if constexpr (mode == CastMode::SafeSlow) {
      const auto* concrete_report = dynamic_cast<const ClientReport*>(&report);
      return concrete_report != nullptr &&
             m_policy.ApplyExecutionReport(*concrete_report);
    } else {
      return m_policy.ApplyExecutionReport(
          static_cast<const ClientReport&>(report));
    }
  }

  template <typename P = ClientPolicy,
            std::enable_if_t<detail::HasAdjustment<P>::value, int> = 0>
  [[nodiscard]] PolicyAccountAdjustmentResult ApplyAccountAdjustment(
      const ::openpit::accountadjustment::Context& context,
      ::openpit::param::AccountId accountId,
      const ::openpit::accountadjustment::AccountAdjustment& adjustment,
      ::openpit::tx::Mutations& mutations, AccountOutcomes& outcomes) const {
    return m_policy.ApplyAccountAdjustment(context, accountId, adjustment,
                                           mutations, outcomes);
  }

 private:
  ClientPolicy m_policy;
};

template <typename ClientPolicy, typename ClientOrder, typename ClientReport>
using StartPolicyAdapterWithSafeSlowArgType =
    StartPolicyAdapter<ClientPolicy, ClientOrder, ClientReport,
                       CastMode::SafeSlow>;

template <typename ClientPolicy, typename ClientOrder, typename ClientReport>
using StartPolicyAdapterWithUnsafeFastArgType =
    StartPolicyAdapter<ClientPolicy, ClientOrder, ClientReport,
                       CastMode::UnsafeFast>;

template <typename ClientPolicy, typename ClientOrder, typename ClientReport>
using PolicyAdapterWithSafeSlowArgType =
    PolicyAdapter<ClientPolicy, ClientOrder, ClientReport, CastMode::SafeSlow>;

template <typename ClientPolicy, typename ClientOrder, typename ClientReport>
using PolicyAdapterWithUnsafeFastArgType =
    PolicyAdapter<ClientPolicy, ClientOrder, ClientReport,
                  CastMode::UnsafeFast>;

}  // namespace openpit::pretrade
