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

// Source: Non-Mutating-Dry-Run.md
//
// Compiling mirror of the C++ snippets published on the Non-Mutating Dry-Run
// wiki page. The snippets are wrapped in the minimal engine/order harness and
// assertions needed to execute them as tests.

#include <gtest/gtest.h>
#include <openpit/openpit.hpp>
#include <openpit/pretrade/policies.hpp>

#include <cstdint>
#include <iostream>
#include <optional>
#include <utility>

namespace {

namespace policies = openpit::pretrade::policies;
using openpit::param::Price;
using openpit::param::Quantity;

[[nodiscard]] openpit::model::Order WikiOrder() {
  openpit::model::Order order;
  openpit::model::OrderOperation op;
  op.instrument = openpit::model::Instrument(::openpit::param::Asset("AAPL"),
                                             ::openpit::param::Asset("USD"));
  op.accountId = openpit::param::AccountId::FromUint64(99224416);
  op.side = openpit::model::Side::Buy;
  op.tradeAmount =
      openpit::model::TradeAmount::OfQuantity(Quantity::FromString("10"));
  op.price = Price::FromString("100");
  order.operation = std::move(op);
  return order;
}

[[nodiscard]] openpit::Engine BuildValidationEngine() {
  openpit::EngineBuilder builder(openpit::SyncPolicy::None);
  builder.Add(policies::OrderValidationPolicy{});
  return builder.Build();
}

// Used in: pit.wiki/Non-Mutating-Dry-Run.md - Read the Dry-Run Verdict
TEST(NonMutatingDryRunWiki, ReadDryRunVerdict) {
  openpit::Engine engine = BuildValidationEngine();
  const openpit::model::Order order = WikiOrder();

  const openpit::pretrade::DryRunReport report =
      engine.ExecutePreTradeDryRun(order);
  if (report.Passed()) {
    std::cout << "order would be admitted\n";
  } else {
    for (const openpit::pretrade::Reject& reject : report.Rejects()) {
      std::cout << "would reject by " << reject.policy << " ["
                << static_cast<int>(reject.code) << "]: " << reject.reason
                << " (" << reject.details << ")\n";
    }
  }

  EXPECT_TRUE(report.Passed());
}

// Used in: pit.wiki/Non-Mutating-Dry-Run.md - Use the Dry-Run Before a Real
// Call
TEST(NonMutatingDryRunWiki, ProbeBeforeRealCall) {
  openpit::Engine engine = BuildValidationEngine();
  const openpit::model::Order order = WikiOrder();

  // Probe without spending any budget or creating any reservation.
  const openpit::pretrade::DryRunReport probe =
      engine.ExecutePreTradeDryRun(order);
  if (!probe.Passed()) {
    return;  // would have been rejected - skip the real call
  }

  // The real call now runs with fresh state; the probe had no effect.
  openpit::pretrade::ExecuteResult result = engine.ExecutePreTrade(order);
  if (result.Passed()) {
    result.reservation->Commit();
  }

  EXPECT_TRUE(probe.Passed());
  EXPECT_TRUE(result.Passed());
}

class MyCountingPolicy {
 public:
  explicit MyCountingPolicy(std::uint64_t* count) : m_count(count) {}

  [[nodiscard]] std::optional<openpit::pretrade::Reject> CheckPreTradeStart(
      const openpit::Order& order) const {
    static_cast<void>(order);
    ++*m_count;  // side effect: spends the real counter budget
    return std::nullopt;
  }

  void PerformPreTradeCheck(const openpit::pretrade::Context& context,
                            openpit::pretrade::PolicyDecision& decision) const {
    static_cast<void>(context);
    static_cast<void>(decision);
  }

  // CheckPreTradeStartDryRun is the read-only variant: no counter increment.
  [[nodiscard]] std::optional<openpit::pretrade::Reject>
  CheckPreTradeStartDryRun(const openpit::Order& order) const {
    static_cast<void>(order);
    return std::nullopt;  // read current state without spending the budget
  }

  // PerformPreTradeCheckDryRun is the read-only variant.
  void PerformPreTradeCheckDryRun(
      const openpit::pretrade::Context& context,
      openpit::pretrade::PolicyDecision& decision) const {
    static_cast<void>(context);
    static_cast<void>(decision);
  }

 private:
  std::uint64_t* m_count;
};

// Used in: pit.wiki/Non-Mutating-Dry-Run.md - Read-Only Custom Start-Stage Hook
TEST(NonMutatingDryRunWiki, CustomPolicyDryRunHook) {
  std::uint64_t count = 0;
  openpit::pretrade::CustomPolicy<MyCountingPolicy> policy(
      "MyCountingPolicy", MyCountingPolicy{&count});

  openpit::EngineBuilder builder(openpit::SyncPolicy::None);
  builder.Add(policy);
  openpit::Engine engine = builder.Build();
  const openpit::model::Order order = WikiOrder();

  const openpit::pretrade::DryRunReport probe =
      engine.ExecutePreTradeDryRun(order);
  EXPECT_TRUE(probe.Passed());
  EXPECT_EQ(count, 0u);

  openpit::pretrade::ExecuteResult result = engine.ExecutePreTrade(order);
  ASSERT_TRUE(result.Passed());
  result.reservation->Commit();
  EXPECT_EQ(count, 1u);
}

}  // namespace
