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

#include "openpit/openpit.hpp"

#include <gtest/gtest.h>

#include <array>
#include <vector>

namespace {

namespace aa = openpit::accountadjustment;
namespace param = openpit::param;

// The public umbrella header exposes the PnL outcome types used by both
// post-trade account PnL and per-asset account-adjustment outcomes.
TEST(AccountPnlOutcome, AvailablePreservesZeroDelta) {
  const aa::AccountPnlOutcome outcome(
      aa::PnlOutcomeAmount(param::Pnl::FromString("0"),
                           param::Pnl::FromString("25.5")),
      param::AccountId::FromUint64(42), param::GroupId(7));

  EXPECT_EQ(outcome.accountId, param::AccountId::FromUint64(42));
  EXPECT_EQ(outcome.policyGroupId, param::GroupId(7));
  ASSERT_NE(outcome.Amount(), nullptr);
  EXPECT_FALSE(outcome.HaltReason().has_value());
  const auto& pnl = *outcome.Amount();
  EXPECT_EQ(pnl.delta.ToString(), "0");
  EXPECT_EQ(pnl.absolute.ToString(), "25.5");
}

TEST(AccountPnlOutcome, MissingAccountCurrencyDoesNotExposeThePnlAmount) {
  const aa::AccountPnlOutcome outcome(aa::PnlHaltReason::MissingAccountCurrency,
                                      param::AccountId::FromUint64(42),
                                      param::GroupId(7));

  EXPECT_EQ(outcome.Amount(), nullptr);
  ASSERT_TRUE(outcome.HaltReason().has_value());
  EXPECT_EQ(*outcome.HaltReason(), aa::PnlHaltReason::MissingAccountCurrency);
}

TEST(AccountOutcomeEntry, FirstPositionPnlHaltExposesTheReason) {
  aa::AccountOutcomeEntry outcome(openpit::param::Asset("USD"));
  outcome.realizedPnl = aa::PnlOutcome(aa::PnlHaltReason::MissingFx);

  ASSERT_TRUE(outcome.realizedPnl.has_value());
  ASSERT_TRUE(outcome.realizedPnl->HaltReason().has_value());
  EXPECT_EQ(*outcome.realizedPnl->HaltReason(), aa::PnlHaltReason::MissingFx);
}

TEST(PnlOutcome, MapsEveryHaltReason) {
  const std::array<aa::PnlHaltReason, 5> cases{
      aa::PnlHaltReason::MissingFx,
      aa::PnlHaltReason::MissingAccountCurrency,
      aa::PnlHaltReason::MissingInitialPnl,
      aa::PnlHaltReason::MissingCostBasis,
      aa::PnlHaltReason::ArithmeticOverflow,
  };

  for (const aa::PnlHaltReason expected : cases) {
    const aa::PnlOutcome outcome(expected);
    ASSERT_TRUE(outcome.HaltReason().has_value());
    EXPECT_EQ(*outcome.HaltReason(), expected);
  }
}

TEST(AccountPnlOperation, ValueAndHaltAreExclusive) {
  const aa::AccountPnlOperation value(param::Pnl::FromString("12.5"));
  ASSERT_NE(value.Value(), nullptr);
  EXPECT_EQ(value.Value()->ToString(), "12.5");
  EXPECT_FALSE(value.HaltReason().has_value());

  const aa::AccountPnlOperation halted(aa::PnlHaltReason::MissingFx);
  EXPECT_EQ(halted.Value(), nullptr);
  ASSERT_TRUE(halted.HaltReason().has_value());
  EXPECT_EQ(*halted.HaltReason(), aa::PnlHaltReason::MissingFx);
}

TEST(AccountPnlOperation, InvalidReasonThrowsThroughPublicEngineCall) {
  openpit::EngineBuilder builder(openpit::SyncPolicy::None);
  builder.Add(openpit::pretrade::policies::SpotFundsPolicy{});
  const openpit::Engine engine = builder.Build();
  const aa::AccountPnlOperation invalidCpp(static_cast<aa::PnlHaltReason>(255));
  aa::AccountAdjustment adjustment;
  adjustment.operation = aa::Operation::OfAccountPnl(invalidCpp);
  EXPECT_THROW(static_cast<void>(engine.ApplyAccountAdjustment(
                   param::AccountId::FromUint64(42),
                   std::vector<aa::AccountAdjustment>{adjustment})),
               openpit::Error);
}

}  // namespace
