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

#include <cstdint>
#include <optional>
#include <string>
#include <string_view>
#include <utility>

#include "openpit/adapters.hpp"

// This example demonstrates adapter-only wiring:
// - client payload types (`BrokerOrderData`, `BrokerReportData`)
// - thin payload adapters (`BrokerOrder`, `BrokerExecutionReport`)
// - policy adapters in SafeSlow cast mode.
//
// Engine construction/execution is intentionally omitted.

// Client order payload from broker API.
struct BrokerOrderData {
  std::string m_symbol;
  std::string m_settlement;
  std::string m_side;
  std::string m_quantity;
  std::string m_price;
  std::uint32_t m_clientTag;
};

// Client report payload from broker API.
struct BrokerReportData {
  std::string m_symbol;
  std::string m_settlement;
  std::string m_pnl;
  std::string m_fee;
};

// Thin adapter from broker order payload to `openpit::Order`.
class BrokerOrder : public openpit::Order {
 public:
  explicit BrokerOrder(BrokerOrderData source) : m_source(std::move(source)) {}

  [[nodiscard]] openpit::param::Quantity Quantity() const {
    return openpit::param::Quantity(m_source.m_quantity);
  }

  [[nodiscard]] openpit::param::Price Price() const {
    return openpit::param::Price(m_source.m_price);
  }

  [[nodiscard]] std::uint32_t ClientTag() const noexcept {
    return m_source.m_clientTag;
  }

 private:
  BrokerOrderData m_source;
};

// Thin adapter from broker report payload to `openpit::ExecutionReport`.
class BrokerExecutionReport : public openpit::ExecutionReport {
 public:
  explicit BrokerExecutionReport(BrokerReportData source)
      : m_source(std::move(source)) {}

  [[nodiscard]] std::optional<openpit::param::Pnl> Pnl() const {
    return openpit::param::Pnl(m_source.m_pnl);
  }

 private:
  BrokerReportData m_source;
};

// SafeSlow start-stage policy adapter.
class TagPolicy {
 public:
  [[nodiscard]] std::string_view Name() const noexcept { return "TagPolicy"; }

  [[nodiscard]] std::optional<openpit::pretrade::Reject> CheckPreTradeStart(
      const BrokerOrder& order) const {
    if (order.ClientTag() == 0) {
      return openpit::pretrade::MakeTypeMismatchReject(
          Name(), openpit::pretrade::RejectScope::Order,
          openpit::pretrade::RejectCode::InvalidFieldValue,
          "client_tag must be non-zero", "expected non-zero broker tag");
    }
    return std::nullopt;
  }

  [[nodiscard]] bool ApplyExecutionReport(
      const BrokerExecutionReport& report) const {
    static_cast<void>(report);
    return false;
  }
};

// SafeSlow main-stage policy adapter.
class LossGuardPolicy {
 public:
  [[nodiscard]] std::string_view Name() const noexcept {
    return "LossGuardPolicy";
  }

  void PerformPreTradeCheck(const BrokerOrder& order,
                            const openpit::pretrade::Context& context,
                            openpit::pretrade::PolicyDecision& decision) const {
    static_cast<void>(context);
    if (order.ClientTag() == 0) {
      openpit::pretrade::PushReject(
          decision,
          openpit::pretrade::MakeTypeMismatchReject(
              Name(), openpit::pretrade::RejectScope::Order,
              openpit::pretrade::RejectCode::InvalidFieldValue,
              "client_tag must be non-zero", "expected non-zero broker tag"));
    }
  }

  [[nodiscard]] bool ApplyExecutionReport(
      const BrokerExecutionReport& report) const {
    static_cast<void>(report);
    return false;
  }
};

int main() {
  // Demo file intentionally focuses on adapter declarations and policy wiring.
  // Engine wiring is added in real integration.
  auto start_adapter = openpit::pretrade::StartPolicyAdapterWithSafeSlowArgType<
      TagPolicy, BrokerOrder, BrokerExecutionReport>(TagPolicy{});
  auto main_adapter = openpit::pretrade::PolicyAdapterWithSafeSlowArgType<
      LossGuardPolicy, BrokerOrder, BrokerExecutionReport>(LossGuardPolicy{});
  static_cast<void>(start_adapter);
  static_cast<void>(main_adapter);
  return 0;
}
