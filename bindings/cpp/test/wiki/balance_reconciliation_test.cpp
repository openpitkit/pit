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

// Source: Balance-Reconciliation.md

#include "openpit/accountadjustment/account_adjustment.hpp"
#include "openpit/engine.hpp"
#include "openpit/param.hpp"
#include "openpit/pretrade/pretrade.hpp"

#include <gtest/gtest.h>

#include <cassert>
#include <vector>

namespace {

namespace aa = openpit::accountadjustment;
namespace param = openpit::param;
namespace policies = openpit::pretrade::policies;

TEST(BalanceReconciliation, DeltaVersusAbsolute) {
  openpit::EngineBuilder builder(openpit::SyncPolicy::None);
  builder.Add(policies::SpotFundsPolicy{});
  openpit::Engine engine = builder.Build();

  const param::AccountId accountId = param::AccountId::FromUint64(99224416);

  auto seed = [](const char* amount) {
    aa::AccountAdjustment adj;
    aa::BalanceOperation op;
    op.asset = "USD";
    adj.operation = aa::Operation::OfBalance(op);
    aa::Amount amountGroup;
    amountGroup.balance = param::AdjustmentAmount::OfAbsolute(
        param::PositionSize::FromString(amount));
    adj.amount = amountGroup;
    return adj;
  };

  // First seed: available USD goes from 0 to 10000.
  const openpit::AdjustmentResult firstResult = engine.ApplyAccountAdjustment(
      accountId, std::vector<aa::AccountAdjustment>{seed("10000")});
  assert(firstResult.Passed());
  ASSERT_TRUE(firstResult.Passed());
  assert(firstResult.accountAdjustmentOutcomes.size() == 1);
  ASSERT_EQ(firstResult.accountAdjustmentOutcomes.size(), 1u);
  assert(firstResult.accountAdjustmentOutcomes[0].entry.balance);
  ASSERT_TRUE(firstResult.accountAdjustmentOutcomes[0].entry.balance);
  const aa::OutcomeAmount& firstUSD =
      *firstResult.accountAdjustmentOutcomes[0].entry.balance;
  assert(firstUSD.delta == param::PositionSize::FromString("10000"));
  EXPECT_EQ(firstUSD.delta, param::PositionSize::FromString("10000"));
  assert(firstUSD.absolute == param::PositionSize::FromString("10000"));
  EXPECT_EQ(firstUSD.absolute, param::PositionSize::FromString("10000"));

  // Second seed: available USD goes from 10000 to 15000.
  const openpit::AdjustmentResult secondResult = engine.ApplyAccountAdjustment(
      accountId, std::vector<aa::AccountAdjustment>{seed("15000")});
  assert(secondResult.Passed());
  ASSERT_TRUE(secondResult.Passed());
  assert(secondResult.accountAdjustmentOutcomes.size() == 1);
  ASSERT_EQ(secondResult.accountAdjustmentOutcomes.size(), 1u);
  assert(secondResult.accountAdjustmentOutcomes[0].entry.balance);
  ASSERT_TRUE(secondResult.accountAdjustmentOutcomes[0].entry.balance);
  const aa::OutcomeAmount& secondUSD =
      *secondResult.accountAdjustmentOutcomes[0].entry.balance;
  // delta is the change to add to your own ledger; absolute is just a snapshot.
  assert(secondUSD.delta == param::PositionSize::FromString("5000"));
  EXPECT_EQ(secondUSD.delta, param::PositionSize::FromString("5000"));
  assert(secondUSD.absolute == param::PositionSize::FromString("15000"));
  EXPECT_EQ(secondUSD.absolute, param::PositionSize::FromString("15000"));
}

}  // namespace
