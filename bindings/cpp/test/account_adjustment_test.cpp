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

#include "openpit/accountadjustment/account_adjustment.hpp"

#include "openpit/engine.hpp"
#include "openpit/model/model.hpp"
#include "openpit/param/param.hpp"
#include "openpit/pretrade/policies.hpp"

#include <gtest/gtest.h>
#include <openpit.h>

#include <cstdint>
#include <iterator>
#include <optional>
#include <string>
#include <type_traits>
#include <utility>
#include <vector>

namespace {

namespace aa = openpit::accountadjustment;
namespace param = openpit::param;
namespace model = openpit::model;
namespace policies = openpit::pretrade::policies;

//------------------------------------------------------------------------------
// BalanceOperation

TEST(AccountAdjustmentBalanceOperation, PreservesFields) {
  aa::BalanceOperation operation;
  operation.asset = ::openpit::param::Asset("USD");
  operation.realizedPnl = aa::PnlState(param::Pnl::FromString("-12.75"));
  operation.averageEntryPrice = param::Price::FromString("101.5");

  const aa::BalanceOperation& restored = operation;

  ASSERT_TRUE(restored.asset.has_value());
  EXPECT_EQ(restored.asset->View(), "USD");
  ASSERT_TRUE(restored.averageEntryPrice.has_value());
  EXPECT_EQ(restored.averageEntryPrice->ToString(), "101.5");
  ASSERT_TRUE(restored.realizedPnl.has_value());
  ASSERT_NE(restored.realizedPnl->Value(), nullptr);
  EXPECT_EQ(restored.realizedPnl->Value()->ToString(), "-12.75");
  EXPECT_FALSE(restored.realizedPnl->HaltReason().has_value());
}

TEST(AccountAdjustmentBalanceOperation, AbsentFieldsReadAsEmptyOptional) {
  const aa::BalanceOperation restored;

  EXPECT_FALSE(restored.asset.has_value());
  EXPECT_FALSE(restored.realizedPnl.has_value());
  EXPECT_FALSE(restored.averageEntryPrice.has_value());
}

TEST(AccountAdjustmentBalanceOperation, HaltedPnlPreservesReason) {
  aa::BalanceOperation operation;
  operation.asset = ::openpit::param::Asset("AAPL");
  operation.realizedPnl = aa::PnlState(aa::PnlHaltReason::MissingCostBasis);

  const aa::BalanceOperation& restored = operation;

  ASSERT_TRUE(restored.realizedPnl.has_value());
  EXPECT_EQ(restored.realizedPnl->Value(), nullptr);
  ASSERT_TRUE(restored.realizedPnl->HaltReason().has_value());
  EXPECT_EQ(*restored.realizedPnl->HaltReason(),
            aa::PnlHaltReason::MissingCostBasis);
}

//------------------------------------------------------------------------------
// PositionOperation

TEST(AccountAdjustmentPositionOperation, PreservesFields) {
  aa::PositionOperation operation;
  operation.instrument = model::Instrument(::openpit::param::Asset("AAPL"),
                                           ::openpit::param::Asset("USD"));
  operation.collateralAsset = ::openpit::param::Asset("USD");
  operation.averageEntryPrice = param::Price::FromString("102.25");
  operation.leverage = param::Leverage::FromUint16(4);  // raw 40 (4.0x)
  operation.mode = model::PositionMode::Hedged;

  const aa::PositionOperation& restored = operation;

  ASSERT_TRUE(restored.instrument.has_value());
  EXPECT_EQ(restored.instrument->underlyingAsset.View(), "AAPL");
  EXPECT_EQ(restored.instrument->settlementAsset.View(), "USD");
  ASSERT_TRUE(restored.collateralAsset.has_value());
  EXPECT_EQ(restored.collateralAsset->View(), "USD");
  ASSERT_TRUE(restored.averageEntryPrice.has_value());
  EXPECT_EQ(restored.averageEntryPrice->ToString(), "102.25");
  ASSERT_TRUE(restored.leverage.has_value());
  EXPECT_EQ(restored.leverage->Value(), 4.0F);
  ASSERT_TRUE(restored.mode.has_value());
  EXPECT_EQ(*restored.mode, model::PositionMode::Hedged);
}

TEST(AccountAdjustmentPositionOperation, AbsentFieldsReadAsEmptyOptional) {
  const aa::PositionOperation restored;

  EXPECT_FALSE(restored.instrument.has_value());
  EXPECT_FALSE(restored.collateralAsset.has_value());
  EXPECT_FALSE(restored.averageEntryPrice.has_value());
  EXPECT_FALSE(restored.leverage.has_value());
  EXPECT_FALSE(restored.mode.has_value());
}

//------------------------------------------------------------------------------
// AccountPnlOperation

TEST(AccountAdjustmentAccountPnlOperation, ValueIsAvailable) {
  const aa::AccountPnlOperation operation(param::Pnl::FromString("-12.75"));
  ASSERT_NE(operation.Value(), nullptr);
  EXPECT_EQ(operation.Value()->ToString(), "-12.75");
  EXPECT_FALSE(operation.HaltReason().has_value());
}

//------------------------------------------------------------------------------
// Operation (discriminated)

TEST(AccountAdjustmentOperation, BalanceVariantIsSelected) {
  aa::BalanceOperation balance;
  balance.asset = ::openpit::param::Asset("BTC");
  const aa::Operation operation = aa::Operation::OfBalance(balance);
  ASSERT_NE(operation.AsBalance(), nullptr);
  ASSERT_TRUE(operation.AsBalance()->asset.has_value());
  EXPECT_EQ(operation.AsBalance()->asset->View(), "BTC");
  EXPECT_EQ(operation.AsPosition(), nullptr);
  EXPECT_EQ(operation.AsAccountPnl(), nullptr);
}

TEST(AccountAdjustmentOperation, PositionVariantIsSelected) {
  aa::PositionOperation position;
  position.collateralAsset = ::openpit::param::Asset("USDT");
  const aa::Operation operation = aa::Operation::OfPosition(position);
  ASSERT_NE(operation.AsPosition(), nullptr);
  ASSERT_TRUE(operation.AsPosition()->collateralAsset.has_value());
  EXPECT_EQ(operation.AsPosition()->collateralAsset->View(), "USDT");
  EXPECT_EQ(operation.AsBalance(), nullptr);
}

TEST(AccountAdjustmentOperation, AccountPnlVariantIsSelected) {
  const aa::Operation operation = aa::Operation::OfAccountPnl(
      aa::AccountPnlOperation(param::Pnl::FromString("15")));
  ASSERT_NE(operation.AsAccountPnl(), nullptr);
  ASSERT_NE(operation.AsAccountPnl()->Value(), nullptr);
  EXPECT_EQ(operation.AsAccountPnl()->Value()->ToString(), "15");
}

//------------------------------------------------------------------------------
// Amount group

TEST(AccountAdjustmentAmount, DeltaAndAbsoluteComponentsPreserveValues) {
  aa::Amount amount;
  amount.balance =
      param::AdjustmentAmount::Delta(param::PositionSize::FromString("10"));
  amount.incoming =
      param::AdjustmentAmount::Absolute(param::PositionSize::FromString("250"));

  ASSERT_TRUE(amount.balance.has_value());
  ASSERT_FALSE(amount.held.has_value());
  ASSERT_TRUE(amount.incoming.has_value());
  EXPECT_EQ(amount.balance->ToString(), "delta: 10");
  EXPECT_EQ(amount.incoming->ToString(), "sz: 250");
}

TEST(AccountAdjustmentAmount, NegativeComponentIsPermitted) {
  aa::Amount amount;
  amount.balance =
      param::AdjustmentAmount::Delta(param::PositionSize::FromString("-42.5"));
  ASSERT_TRUE(amount.balance.has_value());
  EXPECT_EQ(amount.balance->ToString(), "delta: -42.5");
}

TEST(AccountAdjustmentAmount, FormatsSelectedAlternative) {
  const param::AdjustmentAmount delta =
      param::AdjustmentAmount::Delta(param::PositionSize::FromString("-3.5"));
  const param::AdjustmentAmount absolute =
      param::AdjustmentAmount::Absolute(param::PositionSize::FromString("12"));

  EXPECT_EQ(delta.ToString(), "delta: -3.5");
  EXPECT_EQ(absolute.ToString(), "sz: 12");
}

TEST(AccountAdjustmentAmount, AllNotSetComponentsReadAsEmptyOptional) {
  const aa::Amount amount;
  EXPECT_FALSE(amount.balance.has_value());
  EXPECT_FALSE(amount.held.has_value());
  EXPECT_FALSE(amount.incoming.has_value());
}

//------------------------------------------------------------------------------
// Bounds group

TEST(AccountAdjustmentBounds, PresentBoundsPreserveValues) {
  aa::Bounds bounds;
  bounds.balanceUpper = param::PositionSize::FromString("100");
  bounds.balanceLower = param::PositionSize::FromString("20");
  bounds.incomingUpper = param::PositionSize::FromString("50");
  bounds.incomingLower = param::PositionSize::FromString("-5");

  const aa::Bounds& restored = bounds;

  ASSERT_TRUE(restored.balanceUpper.has_value());
  EXPECT_EQ(restored.balanceUpper->ToString(), "100");
  ASSERT_TRUE(restored.balanceLower.has_value());
  EXPECT_EQ(restored.balanceLower->ToString(), "20");
  ASSERT_TRUE(restored.incomingUpper.has_value());
  EXPECT_EQ(restored.incomingUpper->ToString(), "50");
  ASSERT_TRUE(restored.incomingLower.has_value());
  EXPECT_EQ(restored.incomingLower->ToString(), "-5");
  // Unset bounds stay absent.
  EXPECT_FALSE(restored.heldUpper.has_value());
  EXPECT_FALSE(restored.heldLower.has_value());
}

TEST(AccountAdjustmentBounds, AllUnsetBoundsReadAsEmptyOptional) {
  const aa::Bounds restored;
  EXPECT_FALSE(restored.balanceUpper.has_value());
  EXPECT_FALSE(restored.balanceLower.has_value());
  EXPECT_FALSE(restored.heldUpper.has_value());
  EXPECT_FALSE(restored.heldLower.has_value());
  EXPECT_FALSE(restored.incomingUpper.has_value());
  EXPECT_FALSE(restored.incomingLower.has_value());
}

//------------------------------------------------------------------------------
// AccountAdjustment

TEST(AccountAdjustment, FullModelPreservesEveryGroup) {
  aa::AccountAdjustment adjustment;

  aa::BalanceOperation balance;
  balance.asset = ::openpit::param::Asset("USD");
  balance.averageEntryPrice = param::Price::FromString("101.5");
  adjustment.operation = aa::Operation::OfBalance(balance);

  aa::Amount amount;
  amount.balance =
      param::AdjustmentAmount::Delta(param::PositionSize::FromString("10"));
  adjustment.amount = amount;

  aa::Bounds bounds;
  bounds.balanceUpper = param::PositionSize::FromString("100");
  adjustment.bounds = bounds;

  adjustment.userData = 99;

  const aa::AccountAdjustment& restored = adjustment;

  ASSERT_TRUE(restored.operation.has_value());
  ASSERT_NE(restored.operation->AsBalance(), nullptr);
  ASSERT_TRUE(restored.operation->AsBalance()->asset.has_value());
  EXPECT_EQ(restored.operation->AsBalance()->asset->View(), "USD");
  EXPECT_EQ(restored.operation->AsBalance()->averageEntryPrice->ToString(),
            "101.5");

  ASSERT_TRUE(restored.amount.has_value());
  ASSERT_TRUE(restored.amount->balance.has_value());

  ASSERT_TRUE(restored.bounds.has_value());
  ASSERT_TRUE(restored.bounds->balanceUpper.has_value());
  EXPECT_EQ(restored.bounds->balanceUpper->ToString(), "100");

  EXPECT_EQ(restored.userData, 99u);
}

TEST(AccountAdjustment, EmptyAdjustmentHasNoGroups) {
  const aa::AccountAdjustment restored;
  EXPECT_FALSE(restored.operation.has_value());
  EXPECT_FALSE(restored.amount.has_value());
  EXPECT_FALSE(restored.bounds.has_value());
  EXPECT_EQ(restored.userData, 0u);
}

//------------------------------------------------------------------------------
// OutcomeAmount / AccountOutcomeEntry / Outcome

TEST(AccountAdjustmentOutcomeAmount, PreservesDeltaAndAbsolute) {
  const aa::OutcomeAmount amount(param::PositionSize::FromString("-3"),
                                 param::PositionSize::FromString("17"));

  EXPECT_EQ(amount.delta.ToString(), "-3");
  EXPECT_EQ(amount.absolute.ToString(), "17");
}

TEST(AccountAdjustmentPnlOutcomeAmount, PreservesDeltaAndAbsolute) {
  const aa::PnlOutcomeAmount amount(param::Pnl::FromString("20"),
                                    param::Pnl::FromString("50"));

  EXPECT_EQ(amount.delta.ToString(), "20");
  EXPECT_EQ(amount.absolute.ToString(), "50");
}

TEST(AccountAdjustmentOutcomeEntry, PreservesPresentAndAbsentAmounts) {
  aa::AccountOutcomeEntry entry(param::Asset("USD"));
  entry.balance = aa::OutcomeAmount(param::PositionSize::FromString("5"),
                                    param::PositionSize::FromString("5"));
  entry.realizedPnl = aa::PnlOutcome{aa::PnlOutcomeAmount(
      param::Pnl::FromString("-2.5"), param::Pnl::FromString("7.5"))};
  entry.averageEntryPrice = param::Price::FromString("101.25");
  // held and incoming intentionally left absent.

  const aa::AccountOutcomeEntry& restored = entry;

  EXPECT_EQ(restored.asset.View(), "USD");
  ASSERT_TRUE(restored.balance.has_value());
  EXPECT_EQ(restored.balance->delta.ToString(), "5");
  EXPECT_FALSE(restored.held.has_value());
  EXPECT_FALSE(restored.incoming.has_value());
  ASSERT_TRUE(restored.realizedPnl.has_value());
  ASSERT_NE(restored.realizedPnl->Amount(), nullptr);
  const auto& pnl = *restored.realizedPnl->Amount();
  EXPECT_EQ(pnl.delta.ToString(), "-2.5");
  EXPECT_EQ(pnl.absolute.ToString(), "7.5");
  ASSERT_TRUE(restored.averageEntryPrice.has_value());
  EXPECT_EQ(restored.averageEntryPrice->ToString(), "101.25");
}

TEST(AccountAdjustmentOutcome, PreservesGroupAndEntry) {
  aa::Outcome outcome(param::GroupId(7),
                      aa::AccountOutcomeEntry(param::Asset("ETH")));
  outcome.entry.incoming =
      aa::OutcomeAmount(param::PositionSize::FromString("1"),
                        param::PositionSize::FromString("9"));

  const aa::Outcome& restored = outcome;
  EXPECT_EQ(restored.policyGroupId, param::GroupId(7));
  EXPECT_EQ(restored.entry.asset.View(), "ETH");
  ASSERT_TRUE(restored.entry.incoming.has_value());
  EXPECT_EQ(restored.entry.incoming->absolute.ToString(), "9");
  EXPECT_FALSE(restored.entry.realizedPnl.has_value());
  EXPECT_FALSE(restored.entry.averageEntryPrice.has_value());
}

TEST(AccountAdjustmentEngine, ForceSetPositionPnlSurfacesOutcome) {
  openpit::EngineBuilder builder(openpit::SyncPolicy::None);
  builder.Add(policies::SpotFundsPolicy{});
  const openpit::Engine engine = builder.Build();

  const param::AccountId accountId = param::AccountId::FromUint64(99224416);

  aa::AccountAdjustment seed;
  aa::BalanceOperation seedOperation;
  seedOperation.asset = ::openpit::param::Asset("AAPL");
  seedOperation.realizedPnl = aa::PnlState(param::Pnl::FromString("30"));
  seed.operation = aa::Operation::OfBalance(std::move(seedOperation));

  const openpit::AdjustmentResult seedResult = engine.ApplyAccountAdjustment(
      accountId, std::vector<aa::AccountAdjustment>{seed});
  ASSERT_TRUE(seedResult.Passed());

  aa::AccountAdjustment forceSet;
  aa::BalanceOperation forceSetOperation;
  forceSetOperation.asset = ::openpit::param::Asset("AAPL");
  forceSetOperation.realizedPnl = aa::PnlState(param::Pnl::FromString("50"));
  forceSet.operation = aa::Operation::OfBalance(std::move(forceSetOperation));

  const openpit::AdjustmentResult result = engine.ApplyAccountAdjustment(
      accountId, std::vector<aa::AccountAdjustment>{forceSet});
  ASSERT_TRUE(result.Passed());
  ASSERT_EQ(result.accountAdjustmentOutcomes.size(), 1u);

  const aa::AccountOutcomeEntry& entry =
      result.accountAdjustmentOutcomes.front().entry;
  EXPECT_EQ(entry.asset.View(), "AAPL");
  ASSERT_TRUE(entry.realizedPnl.has_value());
  ASSERT_NE(entry.realizedPnl->Amount(), nullptr);
  const auto& pnl = *entry.realizedPnl->Amount();
  EXPECT_EQ(pnl.delta.ToString(), "20");
  EXPECT_EQ(pnl.absolute.ToString(), "50");
}

//------------------------------------------------------------------------------
// OutcomeList / BatchError (empty / null handle behavior)

TEST(AccountAdjustmentOutcomeList, DefaultIsEmptyAndNull) {
  static_assert(
      std::is_same_v<typename std::iterator_traits<
                         aa::OutcomeList::const_iterator>::iterator_category,
                     std::input_iterator_tag>);

  const aa::OutcomeList list;
  EXPECT_FALSE(static_cast<bool>(list));
  EXPECT_EQ(list.size(), 0u);
  EXPECT_TRUE(list.empty());
  EXPECT_THROW(static_cast<void>(list.at(0)), std::out_of_range);
  EXPECT_EQ(list.begin(), list.end());
  EXPECT_TRUE(list.ToVector().empty());
}

TEST(AccountAdjustmentBatchError, DefaultIsNullWithNoRejects) {
  const aa::BatchError error;
  EXPECT_FALSE(static_cast<bool>(error));
  EXPECT_EQ(error.FailedAdjustmentIndex(), 0u);
  EXPECT_TRUE(error.Rejects().empty());
}

}  // namespace
