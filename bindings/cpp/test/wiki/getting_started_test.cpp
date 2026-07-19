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

// Source: Getting-Started.md
//
// Compiling mirror of the C++ snippets published on the Getting-Started wiki
// page. Each TEST runs the same user code shown in the corresponding wiki block
// (modulo the minimal engine/order/report harness and the asserts); the
// published snippet body and the test body must stay in lock-step.

#include <gtest/gtest.h>
#include <openpit/openpit.hpp>
#include <openpit/pretrade/policies.hpp>

#include <cstdint>
#include <iostream>
#include <utility>

namespace {

namespace policies = openpit::pretrade::policies;
using openpit::param::Fee;
using openpit::param::Pnl;
using openpit::param::Price;
using openpit::param::Quantity;
using openpit::param::Volume;

// Harness: builds the engine configured exactly as the "Build an Engine" wiki
// snippet does, so the shortcut / two-stage / post-trade snippets have a real
// `engine` to run against.
[[nodiscard]] openpit::Engine BuildWikiEngine() {
  const Quantity maxQty = Quantity::FromString("500");
  const Volume maxNotional = Volume::FromString("100000");

  openpit::EngineBuilder builder(openpit::SyncPolicy::None);

  builder.Add(policies::OrderValidationPolicy{});

  policies::PnlBoundsBrokerBarrier pnlBarrier{"USD"};
  pnlBarrier.lowerBound = Pnl::FromString("-1000");
  builder.Add(policies::PnlBoundsKillSwitchPolicy{}.BrokerBarrier(pnlBarrier));

  builder.Add(policies::RateLimitPolicy{}.BrokerBarrier(
      policies::RateLimitBrokerBarrier(
          policies::RateLimit(/*maxOrders=*/100,
                              /*windowNanoseconds=*/1'000'000'000))));

  builder.Add(policies::OrderSizeLimitPolicy{}
                  .BrokerBarrier(policies::OrderSizeBrokerBarrier(
                      policies::OrderSizeLimit(maxQty, maxNotional)))
                  .AssetBarrier(policies::OrderSizeAssetBarrier(
                      policies::OrderSizeLimit(maxQty, maxNotional), "USD")));

  return builder.Build();
}

// Harness: the canonical AAPL buy order used across the wiki snippets.
[[nodiscard]] openpit::model::Order WikiOrder() {
  return openpit::model::Order::Limit(
      openpit::model::Instrument("AAPL", "USD"),
      ::openpit::param::AccountId::FromUint64(99224416),
      openpit::model::Side::Buy,
      openpit::model::TradeAmount::OfQuantity(Quantity::FromString("100")),
      Price::FromString("185"));
}

// Harness: the execution report used by the post-trade snippets.
[[nodiscard]] openpit::model::ExecutionReport WikiReport() {
  openpit::model::ExecutionReport report;
  openpit::model::ExecutionReportOperation reportOp;
  reportOp.instrument = openpit::model::Instrument("AAPL", "USD");
  reportOp.accountId = ::openpit::param::AccountId::FromUint64(99224416);
  reportOp.side = openpit::model::Side::Buy;
  report.operation = std::move(reportOp);

  openpit::model::FinancialImpact impact;
  impact.pnl = Pnl::FromString("-50");
  impact.fee = Fee::FromString("3.4");
  report.financialImpact = std::move(impact);
  return report;
}

//------------------------------------------------------------------------------
// Example: Build an Engine (the full end-to-end flow).

TEST(GettingStartedWiki, BuildAnEngine) {
  const Quantity maxQty = Quantity::FromString("500");
  const Volume maxNotional = Volume::FromString("100000");

  // 1. Build the engine (one time at the platform initialization).
  openpit::EngineBuilder builder(openpit::SyncPolicy::None);

  builder.Add(policies::OrderValidationPolicy{});

  policies::PnlBoundsBrokerBarrier pnlBarrier{"USD"};
  pnlBarrier.lowerBound = Pnl::FromString("-1000");
  builder.Add(policies::PnlBoundsKillSwitchPolicy{}.BrokerBarrier(pnlBarrier));

  builder.Add(policies::RateLimitPolicy{}.BrokerBarrier(
      policies::RateLimitBrokerBarrier(
          policies::RateLimit(/*maxOrders=*/100,
                              /*windowNanoseconds=*/1'000'000'000))));

  builder.Add(policies::OrderSizeLimitPolicy{}
                  .BrokerBarrier(policies::OrderSizeBrokerBarrier(
                      policies::OrderSizeLimit(maxQty, maxNotional)))
                  .AssetBarrier(policies::OrderSizeAssetBarrier(
                      policies::OrderSizeLimit(maxQty, maxNotional), "USD")));

  const openpit::Engine engine = builder.Build();

  // 2. Check an order.
  openpit::model::Order order = openpit::model::Order::Limit(
      openpit::model::Instrument("AAPL", "USD"),
      ::openpit::param::AccountId::FromUint64(99224416),
      openpit::model::Side::Buy,
      openpit::model::TradeAmount::OfQuantity(Quantity::FromString("100")),
      Price::FromString("185"));

  openpit::pretrade::StartResult start = engine.StartPreTrade(order);
  if (!start.Passed()) {
    for (const openpit::pretrade::Reject& r : start.rejects) {
      std::cout << "rejected by " << r.policy << " ["
                << static_cast<int>(r.code) << "]: " << r.reason << " ("
                << r.details << ")\n";
    }
    FAIL() << "start stage unexpectedly rejected";
  }
  openpit::pretrade::Request request = std::move(*start.request);

  // 3. Quick, lightweight checks were performed during the start stage. The
  // system state has not yet changed (except controls that must observe every
  // request). Before the heavy-duty checks, other work on the request can be
  // performed simply by holding the request object.

  // 4. Real pre-trade and risk control.
  openpit::pretrade::ExecuteResult executed = request.Execute();

  // Optional shortcut for the same two-stage flow:
  // openpit::pretrade::ExecuteResult executed = engine.ExecutePreTrade(order);

  if (!executed.Passed()) {
    for (const openpit::pretrade::Reject& r : executed.rejects) {
      std::cout << "rejected by " << r.policy << " ["
                << static_cast<int>(r.code) << "]: " << r.reason << " ("
                << r.details << ")\n";
    }
    FAIL() << "main stage unexpectedly rejected";
  }
  openpit::pretrade::Reservation reservation = std::move(*executed.reservation);

  // 5. If the request is successfully sent to the venue, it must be committed.
  // The rollback must be called otherwise to revert all performed reservations.
  reservation.Commit();

  // 6. The order goes to the venue and returns with an execution report.
  openpit::model::ExecutionReport report;
  openpit::model::ExecutionReportOperation reportOp;
  reportOp.instrument = openpit::model::Instrument("AAPL", "USD");
  reportOp.accountId = ::openpit::param::AccountId::FromUint64(99224416);
  reportOp.side = openpit::model::Side::Buy;
  report.operation = std::move(reportOp);

  openpit::model::FinancialImpact impact;
  impact.pnl = Pnl::FromString("-50");
  impact.fee = Fee::FromString("3.4");
  report.financialImpact = std::move(impact);

  const openpit::PostTradeResult result = engine.ApplyExecutionReport(report);

  for (const auto& outcome : result.accountPnls) {
    std::cout << "account P&L outcome for " << outcome.accountId.Raw() << '\n';
  }
  for (const auto& outcome : result.accountAdjustments) {
    std::cout << "account adjustment from group " << outcome.policyGroupId.Raw()
              << '\n';
  }

  // 7. After each execution report is applied, the system may report that it
  // has been determined in advance that all subsequent requests will be
  // rejected if the account status does not change.
  if (!result.accountBlocks.empty()) {
    std::cout << "halt new orders until the blocked state is cleared\n";
  }

  EXPECT_TRUE(result.accountBlocks.empty());
}

//------------------------------------------------------------------------------
// Example: Shortcut for Start + Main Stages.

TEST(GettingStartedWiki, ShortcutStartAndMainStages) {
  const openpit::Engine engine = BuildWikiEngine();
  const openpit::model::Order order = WikiOrder();

  // The shortcut runs start stage and main stage as one convenience call.
  openpit::pretrade::ExecuteResult executed = engine.ExecutePreTrade(order);
  if (executed.Passed()) {
    // Finalization is still explicit even when the two stages are composed.
    executed.reservation->Commit();
  } else {
    for (const openpit::pretrade::Reject& reject : executed.rejects) {
      std::cerr << "rejected by " << reject.policy << " ["
                << static_cast<int>(reject.code) << "]: " << reject.reason
                << " (" << reject.details << ")\n";
    }
  }

  EXPECT_TRUE(executed.Passed());
}

//------------------------------------------------------------------------------
// Example: Run an Order Through the Engine (explicit two-stage flow).

TEST(GettingStartedWiki, RunAnOrderThroughTheEngine) {
  const openpit::Engine engine = BuildWikiEngine();
  const openpit::model::Order order = WikiOrder();

  openpit::pretrade::StartResult start = engine.StartPreTrade(order);
  if (!start.Passed()) {
    for (const openpit::pretrade::Reject& reject : start.rejects) {
      std::cerr << "rejected by " << reject.policy << " ["
                << static_cast<int>(reject.code) << "]: " << reject.reason
                << " (" << reject.details << ")\n";
    }
    return;
  }

  openpit::pretrade::ExecuteResult executed = start.request->Execute();
  if (!executed.Passed()) {
    for (const openpit::pretrade::Reject& reject : executed.rejects) {
      std::cerr << "rejected by " << reject.policy << " ["
                << static_cast<int>(reject.code) << "]: " << reject.reason
                << " (" << reject.details << ")\n";
    }
    return;
  }

  executed.reservation->Commit();

  EXPECT_TRUE(executed.Passed());
}

//------------------------------------------------------------------------------
// Example: Apply Post-Trade Feedback.

TEST(GettingStartedWiki, ApplyPostTradeFeedback) {
  const openpit::Engine engine = BuildWikiEngine();
  const openpit::model::ExecutionReport report = WikiReport();

  // Execution reports feed realized outcomes back into cumulative policy state.
  const openpit::PostTradeResult result = engine.ApplyExecutionReport(report);
  for (const auto& outcome : result.accountPnls) {
    std::cerr << "account P&L outcome for " << outcome.accountId.Raw() << '\n';
  }
  for (const auto& outcome : result.accountAdjustments) {
    std::cerr << "account adjustment from group " << outcome.policyGroupId.Raw()
              << '\n';
  }
  if (!result.accountBlocks.empty()) {
    std::cerr << "halt new orders until the blocked state is cleared\n";
  }

  EXPECT_TRUE(result.accountBlocks.empty());
}

}  // namespace
