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
// Please see https://github.com/openpitkit and the OWNERS file for details.

#pragma once

#include <optional>
#include <string_view>
#include <typeinfo>
#include <utility>

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
struct Reject;
enum class RejectScope;
enum class RejectCode;

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
// - Returns `false` on report mismatch.
// - Risk profile: safe default at dynamic boundaries.
//
// `UnsafeFast`:
// - Uses direct `static_cast` without runtime verification.
// - Avoids runtime RTTI checks.
// - Wrong wiring is undefined behavior.
// - Risk profile: only for closed systems with compile-time pairing guarantees.
enum class CastMode {
  SafeSlow,
  UnsafeFast,
};

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
    }
    return m_policy.CheckPreTradeStart(
        static_cast<const ClientOrder&>(order));
  }

  // Adapts execution-report callback to client report type.
  //
  // SafeSlow:
  // - type mismatch -> `false`
  //
  // UnsafeFast:
  // - direct cast, mismatch is undefined behavior
  [[nodiscard]] bool ApplyExecutionReport(
      const openpit::ExecutionReport& report) const {
    if constexpr (mode == CastMode::SafeSlow) {
      const auto* concrete_report = dynamic_cast<const ClientReport*>(&report);
      if (concrete_report == nullptr) {
        return false;
      }
      return m_policy.ApplyExecutionReport(*concrete_report);
    }
    return m_policy.ApplyExecutionReport(
        static_cast<const ClientReport&>(report));
  }

 private:
  ClientPolicy m_policy;
};

// Main-stage adapter for client policy object.
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
 public:
  // Creates adapter around a client main-stage policy instance.
  explicit PolicyAdapter(ClientPolicy policy)
      : m_policy(std::move(policy)) {}

  // Returns stable policy name forwarded from client policy object.
  [[nodiscard]] std::string_view Name() const noexcept {
    return m_policy.Name();
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
                   MakeTypeMismatchReject(Name(), RejectScope::Order,
                                          RejectCode::Other,
                                          "order type mismatch",
                                          typeid(ClientOrder).name()));
        return;
      }
      m_policy.PerformPreTradeCheck(*concrete_order, context, decision);
      return;
    }
    m_policy.PerformPreTradeCheck(static_cast<const ClientOrder&>(order),
                                  context, decision);
  }

  // Adapts execution-report callback to client report type.
  //
  // SafeSlow:
  // - type mismatch -> `false`
  //
  // UnsafeFast:
  // - direct cast, mismatch is undefined behavior
  [[nodiscard]] bool ApplyExecutionReport(
      const openpit::ExecutionReport& report) const {
    if constexpr (mode == CastMode::SafeSlow) {
      const auto* concrete_report = dynamic_cast<const ClientReport*>(&report);
      if (concrete_report == nullptr) {
        return false;
      }
      return m_policy.ApplyExecutionReport(*concrete_report);
    }
    return m_policy.ApplyExecutionReport(
        static_cast<const ClientReport&>(report));
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
using PolicyAdapterWithUnsafeFastArgType = PolicyAdapter<
    ClientPolicy, ClientOrder, ClientReport, CastMode::UnsafeFast>;

}  // namespace openpit::pretrade
