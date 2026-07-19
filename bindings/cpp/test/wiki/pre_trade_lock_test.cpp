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

// Source: Pre-Trade-Lock.md
//
// Compiling mirror of the C++ snippet published on the Pre-Trade-Lock wiki
// page. Each TEST runs the same user code shown in the corresponding C++
// subsection, wrapped only in the minimal engine / harness (setup + asserts)
// the snippet elides for readability. The published snippet body and the test
// body must stay in lock-step.

#include "openpit/accountadjustment/account_adjustment.hpp"
#include "openpit/engine.hpp"
#include "openpit/model.hpp"
#include "openpit/param.hpp"
#include "openpit/pretrade/pretrade.hpp"

#include <gtest/gtest.h>

#include <cassert>
#include <string>
#include <utility>
#include <vector>

namespace {

//------------------------------------------------------------------------------
// Persisting and Restoring a Lock
//
// Mirrors the "Persisting and Restoring a Lock" example block: reserves a buy,
// serializes its lock to JSON, then restores the lock and feeds it back on the
// final fill so the held funds reconcile cleanly.

TEST(PreTradeLockWiki, PersistAndRestoreLockRoundTrip) {
  // Limit-only spot funds: the lock price is required to reconcile fills.
  openpit::EngineBuilder builder(openpit::SyncPolicy::None);
  openpit::pretrade::policies::SpotFundsPolicy{}.AddTo(builder);
  openpit::Engine engine = builder.Build();

  const openpit::param::AccountId accountId =
      openpit::param::AccountId::FromUint64(99224416);

  // Seed 10000 USD so the buy can be reserved.
  openpit::accountadjustment::AccountAdjustment seed;
  openpit::accountadjustment::BalanceOperation balanceOp;
  balanceOp.asset = "USD";
  seed.operation =
      openpit::accountadjustment::Operation::OfBalance(std::move(balanceOp));
  openpit::accountadjustment::Amount seedAmount;
  seedAmount.balance = openpit::param::AdjustmentAmount::OfAbsolute(
      openpit::param::PositionSize::FromString("10000"));
  seed.amount = std::move(seedAmount);

  const openpit::AdjustmentResult seedResult = engine.ApplyAccountAdjustment(
      accountId,
      std::vector<openpit::accountadjustment::AccountAdjustment>{seed});
  assert(seedResult.Passed());
  ASSERT_TRUE(seedResult.Passed());

  // Buy 10 AAPL @ 200 holds 2000 USD and records the lock price (200).
  openpit::model::Order order = openpit::model::Order::Limit(
      openpit::model::Instrument("AAPL", "USD"), accountId,
      openpit::model::Side::Buy,
      openpit::model::TradeAmount::OfQuantity(
          openpit::param::Quantity::FromString("10")),
      openpit::param::Price::FromString("200"));

  openpit::pretrade::ExecuteResult result = engine.ExecutePreTrade(order);
  assert(result.Passed());
  ASSERT_TRUE(result.Passed());

  // Persist the lock with its built-in JSON serialization before committing.
  const openpit::pretrade::PreTradeLock lock = result.reservation->Lock();
  const std::string payload = lock.ToJson();
  assert(!payload.empty());
  ASSERT_FALSE(payload.empty());
  result.reservation->Commit();

  // --- After a process restart, rebuild the lock from your store. ---
  openpit::pretrade::PreTradeLock restored =
      openpit::pretrade::PreTradeLock::FromJson(payload);
  assert(!restored.IsEmpty());
  ASSERT_FALSE(restored.IsEmpty());

  // The final fill must carry the restored lock so the policy reconciles the
  // 2000 USD it held against the real fill instead of blocking the account.
  openpit::model::ExecutionReportOperation operation;
  operation.instrument = openpit::model::Instrument("AAPL", "USD");
  operation.accountId = accountId;
  operation.side = openpit::model::Side::Buy;

  openpit::model::Fill fill;
  fill.lastTrade =
      openpit::model::Trade(openpit::param::Price::FromString("200"),
                            openpit::param::Quantity::FromString("10"));
  fill.leavesQuantity = openpit::param::Quantity::FromString("0");
  fill.isFinal = true;

  openpit::model::ExecutionReport report;
  report.operation = std::move(operation);
  report.fill = std::move(fill);
  const openpit::PostTradeResult postTradeResult =
      engine.ApplyExecutionReport(report, restored);
  assert(postTradeResult.accountBlocks.empty());
  EXPECT_TRUE(postTradeResult.accountBlocks.empty());
}

}  // namespace
