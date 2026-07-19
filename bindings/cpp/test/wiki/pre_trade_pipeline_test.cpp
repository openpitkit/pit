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

// Source: Pre-trade-Pipeline.md
//
// Each TEST runs the exact C++ snippet published in the wiki page (modulo the
// minimal harness that builds the engine, order, and execution report and the
// asserts that pin the outcome). Keep the snippet bodies and the published code
// blocks in sync.

#include "openpit/engine.hpp"
#include "openpit/model.hpp"
#include "openpit/pretrade/pretrade.hpp"
#include "openpit/reject.hpp"

#include <gtest/gtest.h>

#include <cstdint>
#include <iostream>
#include <utility>

namespace {

using openpit::Engine;
using openpit::EngineBuilder;
using openpit::SyncPolicy;
using openpit::param::Price;
using openpit::param::Quantity;
using openpit::pretrade::RejectCode;

namespace policies = openpit::pretrade::policies;

// Builds the canonical single-leg test order for `accountId`: a buy of one AAPL
// settled in USD at price 100.
[[nodiscard]] openpit::model::Order TestOrder(std::uint64_t accountId) {
  openpit::model::Order order;
  openpit::model::OrderOperation op;
  op.instrument = openpit::model::Instrument("AAPL", "USD");
  op.accountId = ::openpit::param::AccountId::FromUint64(accountId);
  op.side = openpit::model::Side::Buy;
  op.tradeAmount =
      openpit::model::TradeAmount::OfQuantity(Quantity::FromString("1"));
  op.price = Price::FromString("100");
  order.operation = std::move(op);
  return order;
}

// Builds an execution report carrying the operation identity for `accountId`.
[[nodiscard]] openpit::model::ExecutionReport TestReport(
    std::uint64_t accountId) {
  openpit::model::ExecutionReport report;
  openpit::model::ExecutionReportOperation op;
  op.instrument = openpit::model::Instrument("AAPL", "USD");
  op.accountId = ::openpit::param::AccountId::FromUint64(accountId);
  op.side = openpit::model::Side::Buy;
  report.operation = std::move(op);
  return report;
}

// A rate-limit engine that admits a single order on the broker axis: the second
// pre-trade for any account is rejected with RateLimitExceeded.
[[nodiscard]] Engine SingleOrderEngine() {
  EngineBuilder builder(SyncPolicy::None);
  policies::RateLimitPolicy config;
  config.BrokerBarrier(policies::RateLimitBrokerBarrier(policies::RateLimit(
      /*maxOrders=*/1, /*windowNanoseconds=*/60'000'000'000)));
  config.AddTo(builder);
  return builder.Build();
}

// An order-validation engine that admits every well-formed order.
[[nodiscard]] Engine ValidationEngine() {
  EngineBuilder builder(SyncPolicy::None);
  builder.Add(policies::OrderValidationPolicy{});
  return builder.Build();
}

//------------------------------------------------------------------------------
// Example: Handle a Start-Stage Reject

TEST(PreTradePipeline, HandleAStartStageReject) {
  const Engine engine = ValidationEngine();
  const openpit::model::Order order = TestOrder(1);

  // --- begin wiki snippet ---
  // Start stage returns either a reject or a deferred request handle.
  openpit::pretrade::StartResult startResult = engine.StartPreTrade(order);
  if (!startResult.Passed()) {
    for (const openpit::pretrade::Reject& reject : startResult.rejects) {
      std::cout << "rejected by " << reject.policy << " ["
                << static_cast<int>(reject.code) << "]: " << reject.reason
                << ": " << reject.details << '\n';
    }
  } else {
    // Keep the request if later code wants to enter the main stage.
    openpit::pretrade::Request request = std::move(*startResult.request);
  }
  // --- end wiki snippet ---

  EXPECT_TRUE(startResult.Passed());
}

//------------------------------------------------------------------------------
// Example: Execute the Main Stage and Finalize the Reservation

TEST(PreTradePipeline, ExecuteTheMainStageAndFinalizeTheReservation) {
  const Engine engine = SingleOrderEngine();
  const openpit::model::Order order = TestOrder(1);

  // --- begin wiki snippet ---
  openpit::pretrade::StartResult startResult = engine.StartPreTrade(order);
  // Main stage consumes the deferred request and returns reservation or
  // rejects.
  openpit::pretrade::ExecuteResult executeResult =
      startResult.request->Execute();

  if (executeResult.Passed()) {
    // Commit only after the caller knows the reservation should become durable.
    executeResult.reservation->Commit();
  } else {
    for (const openpit::pretrade::Reject& reject : executeResult.rejects) {
      std::cout << "rejected by " << reject.policy << " ["
                << static_cast<int>(reject.code) << "]: " << reject.reason
                << ": " << reject.details << '\n';
    }
  }
  // --- end wiki snippet ---

  EXPECT_TRUE(executeResult.Passed());
}

//------------------------------------------------------------------------------
// Example: Shortcut for Start + Main Stages

TEST(PreTradePipeline, ShortcutForStartPlusMainStages) {
  const Engine engine = SingleOrderEngine();
  const openpit::model::Order order = TestOrder(1);

  // --- begin wiki snippet ---
  // The shortcut runs start stage and main stage as one convenience call.
  openpit::pretrade::ExecuteResult executeResult =
      engine.ExecutePreTrade(order);
  if (executeResult.Passed()) {
    // Finalization is still explicit even when the two stages are composed.
    executeResult.reservation->Commit();
  } else {
    for (const openpit::pretrade::Reject& reject : executeResult.rejects) {
      std::cout << "rejected by " << reject.policy << " ["
                << static_cast<int>(reject.code) << "]: " << reject.reason
                << ": " << reject.details << '\n';
    }
  }
  // --- end wiki snippet ---

  EXPECT_TRUE(executeResult.Passed());
}

//------------------------------------------------------------------------------
// Example: Apply Post-Trade Feedback

TEST(PreTradePipeline, ApplyPostTradeFeedback) {
  const Engine engine = ValidationEngine();
  const openpit::model::ExecutionReport report = TestReport(1);

  // --- begin wiki snippet ---
  // Execution reports feed realized outcomes back into cumulative policy state.
  const openpit::PostTradeResult result = engine.ApplyExecutionReport(report);
  for (const auto& outcome : result.accountPnls) {
    std::cout << "account P&L outcome for " << outcome.accountId.Raw() << '\n';
  }
  for (const auto& outcome : result.accountAdjustments) {
    std::cout << "account adjustment from group " << outcome.policyGroupId.Raw()
              << '\n';
  }
  if (!result.accountBlocks.empty()) {
    std::cout << "halt new orders until the blocked state is cleared" << '\n';
  }
  // --- end wiki snippet ---

  EXPECT_TRUE(result.accountBlocks.empty());
}

}  // namespace
