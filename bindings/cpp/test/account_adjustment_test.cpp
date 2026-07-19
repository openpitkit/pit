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
#include "openpit/model.hpp"
#include "openpit/param.hpp"
#include "openpit/pretrade/policies.hpp"

#include <gtest/gtest.h>
#include <openpit.h>

#include <cstdint>
#include <optional>
#include <string>
#include <utility>
#include <vector>

namespace {

namespace aa = openpit::accountadjustment;
namespace param = openpit::param;
namespace model = openpit::model;
namespace policies = openpit::pretrade::policies;

// Builds an `OpenPitParamAdjustmentAmount` C POD carrying `value` under the
// given kind, going through the macro-guaranteed `param::PositionSize` value
// type so the decimal is exact. Kept C-level so these tests exercise this
// module's marshaling without depending on a sibling agent's amount factories.
[[nodiscard]] OpenPitParamAdjustmentAmount MakeRawAmount(
    OpenPitParamAdjustmentAmountKind kind, const std::string& value) {
  OpenPitParamAdjustmentAmount raw{};
  raw.kind = kind;
  raw.value = param::PositionSize::FromString(value).Raw();
  return raw;
}

//------------------------------------------------------------------------------
// BalanceOperation

TEST(AccountAdjustmentBalanceOperation, FullRawRoundTripPreservesFields) {
  aa::BalanceOperation operation;
  operation.asset = "USD";
  operation.realizedPnl = param::Pnl::FromString("-12.75");
  operation.averageEntryPrice = param::Price::FromString("101.5");

  const aa::BalanceOperation restored =
      aa::BalanceOperation::FromRaw(operation.Raw());

  ASSERT_TRUE(restored.asset.has_value());
  EXPECT_EQ(*restored.asset, "USD");
  ASSERT_TRUE(restored.averageEntryPrice.has_value());
  EXPECT_EQ(restored.averageEntryPrice->ToString(), "101.5");
  ASSERT_TRUE(restored.realizedPnl.has_value());
  ASSERT_TRUE(std::holds_alternative<param::Pnl>(*restored.realizedPnl));
  EXPECT_EQ(std::get<param::Pnl>(*restored.realizedPnl).ToString(), "-12.75");
}

TEST(AccountAdjustmentBalanceOperation, AbsentFieldsReadAsEmptyOptional) {
  const aa::BalanceOperation restored =
      aa::BalanceOperation::FromRaw(aa::BalanceOperation().Raw());

  EXPECT_FALSE(restored.asset.has_value());
  EXPECT_FALSE(restored.realizedPnl.has_value());
  EXPECT_FALSE(restored.averageEntryPrice.has_value());
}

TEST(AccountAdjustmentBalanceOperation, HaltedPnlRoundTrips) {
  aa::BalanceOperation operation;
  operation.asset = "AAPL";
  operation.realizedPnl = aa::PnlHaltReason::MissingCostBasis;

  const aa::BalanceOperation restored =
      aa::BalanceOperation::FromRaw(operation.Raw());

  ASSERT_TRUE(restored.realizedPnl.has_value());
  ASSERT_TRUE(std::holds_alternative<aa::PnlHaltReason>(*restored.realizedPnl));
  EXPECT_EQ(std::get<aa::PnlHaltReason>(*restored.realizedPnl),
            aa::PnlHaltReason::MissingCostBasis);
}

//------------------------------------------------------------------------------
// PositionOperation

TEST(AccountAdjustmentPositionOperation, FullRawRoundTripPreservesFields) {
  aa::PositionOperation operation;
  operation.instrument = model::Instrument("AAPL", "USD");
  operation.collateralAsset = "USD";
  operation.averageEntryPrice = param::Price::FromString("102.25");
  operation.leverage = param::Leverage::FromUint16(4);  // raw 40 (4.0x)
  operation.mode = model::PositionMode::Hedged;

  const aa::PositionOperation restored =
      aa::PositionOperation::FromRaw(operation.Raw());

  ASSERT_TRUE(restored.instrument.has_value());
  EXPECT_EQ(restored.instrument->underlyingAsset, "AAPL");
  EXPECT_EQ(restored.instrument->settlementAsset, "USD");
  ASSERT_TRUE(restored.collateralAsset.has_value());
  EXPECT_EQ(*restored.collateralAsset, "USD");
  ASSERT_TRUE(restored.averageEntryPrice.has_value());
  EXPECT_EQ(restored.averageEntryPrice->ToString(), "102.25");
  ASSERT_TRUE(restored.leverage.has_value());
  EXPECT_EQ(restored.leverage->Raw(), 40u);
  EXPECT_EQ(restored.leverage->Value(), 4.0F);
  ASSERT_TRUE(restored.mode.has_value());
  EXPECT_EQ(*restored.mode, model::PositionMode::Hedged);
}

TEST(AccountAdjustmentPositionOperation, AbsentFieldsReadAsEmptyOptional) {
  const aa::PositionOperation restored =
      aa::PositionOperation::FromRaw(aa::PositionOperation().Raw());

  EXPECT_FALSE(restored.instrument.has_value());
  EXPECT_FALSE(restored.collateralAsset.has_value());
  EXPECT_FALSE(restored.averageEntryPrice.has_value());
  EXPECT_FALSE(restored.leverage.has_value());
  EXPECT_FALSE(restored.mode.has_value());
}

//------------------------------------------------------------------------------
// AccountPnlOperation

TEST(AccountAdjustmentAccountPnlOperation, ValueRoundTrips) {
  const aa::AccountPnlOperation operation(param::Pnl::FromString("-12.75"));

  const aa::AccountPnlOperation restored =
      aa::AccountPnlOperation::FromRaw(operation.Raw());

  ASSERT_TRUE(std::holds_alternative<param::Pnl>(restored.Get()));
  EXPECT_EQ(std::get<param::Pnl>(restored.Get()).ToString(), "-12.75");
}

//------------------------------------------------------------------------------
// Operation (discriminated)

TEST(AccountAdjustmentOperation, BalanceVariantRoundTrips) {
  aa::BalanceOperation balance;
  balance.asset = "BTC";
  const aa::Operation operation = aa::Operation::OfBalance(balance);
  EXPECT_TRUE(operation.IsBalance());
  EXPECT_FALSE(operation.IsPosition());
  EXPECT_FALSE(operation.IsAccountPnl());

  const std::optional<aa::Operation> restored =
      aa::Operation::FromRaw(operation.Raw());
  ASSERT_TRUE(restored.has_value());
  ASSERT_TRUE(restored->IsBalance());
  ASSERT_NE(restored->AsBalance(), nullptr);
  ASSERT_TRUE(restored->AsBalance()->asset.has_value());
  EXPECT_EQ(*restored->AsBalance()->asset, "BTC");
  EXPECT_EQ(restored->AsPosition(), nullptr);
  EXPECT_EQ(restored->AsAccountPnl(), nullptr);
}

TEST(AccountAdjustmentOperation, PositionVariantRoundTrips) {
  aa::PositionOperation position;
  position.collateralAsset = "USDT";
  const aa::Operation operation = aa::Operation::OfPosition(position);
  EXPECT_TRUE(operation.IsPosition());

  const std::optional<aa::Operation> restored =
      aa::Operation::FromRaw(operation.Raw());
  ASSERT_TRUE(restored.has_value());
  ASSERT_TRUE(restored->IsPosition());
  ASSERT_NE(restored->AsPosition(), nullptr);
  ASSERT_TRUE(restored->AsPosition()->collateralAsset.has_value());
  EXPECT_EQ(*restored->AsPosition()->collateralAsset, "USDT");
  EXPECT_EQ(restored->AsBalance(), nullptr);
}

TEST(AccountAdjustmentOperation, AccountPnlVariantRoundTrips) {
  const aa::Operation operation = aa::Operation::OfAccountPnl(
      aa::AccountPnlOperation(param::Pnl::FromString("15")));
  EXPECT_TRUE(operation.IsAccountPnl());

  const std::optional<aa::Operation> restored =
      aa::Operation::FromRaw(operation.Raw());
  ASSERT_TRUE(restored.has_value());
  ASSERT_TRUE(restored->IsAccountPnl());
  ASSERT_NE(restored->AsAccountPnl(), nullptr);
  ASSERT_TRUE(
      std::holds_alternative<param::Pnl>(restored->AsAccountPnl()->Get()));
  EXPECT_EQ(std::get<param::Pnl>(restored->AsAccountPnl()->Get()).ToString(),
            "15");
}

TEST(AccountAdjustmentOperation, AbsentKindReadsAsEmptyOptional) {
  OpenPitAccountAdjustmentOperation raw{};
  raw.kind = OPENPIT_ACCOUNT_ADJUSTMENT_OPERATION_KIND_ABSENT;
  EXPECT_FALSE(aa::Operation::FromRaw(raw).has_value());
}

//------------------------------------------------------------------------------
// Amount group

TEST(AccountAdjustmentAmount, DeltaAndAbsoluteComponentsRoundTripExactly) {
  OpenPitAccountAdjustmentAmount raw{};
  raw.balance = MakeRawAmount(OPENPIT_PARAM_ADJUSTMENT_AMOUNT_KIND_DELTA, "10");
  raw.incoming =
      MakeRawAmount(OPENPIT_PARAM_ADJUSTMENT_AMOUNT_KIND_ABSOLUTE, "250");
  // held intentionally left NotSet.

  const aa::Amount amount = aa::Amount::FromRaw(raw);
  ASSERT_TRUE(amount.balance.has_value());
  ASSERT_FALSE(amount.held.has_value());
  ASSERT_TRUE(amount.incoming.has_value());

  const OpenPitAccountAdjustmentAmount rebuilt = amount.Raw();
  EXPECT_EQ(rebuilt.balance.kind, OPENPIT_PARAM_ADJUSTMENT_AMOUNT_KIND_DELTA);
  EXPECT_EQ(rebuilt.held.kind, OPENPIT_PARAM_ADJUSTMENT_AMOUNT_KIND_NOT_SET);
  EXPECT_EQ(rebuilt.incoming.kind,
            OPENPIT_PARAM_ADJUSTMENT_AMOUNT_KIND_ABSOLUTE);
  EXPECT_EQ(param::PositionSize::FromRaw(rebuilt.balance.value).ToString(),
            "10");
  EXPECT_EQ(param::PositionSize::FromRaw(rebuilt.incoming.value).ToString(),
            "250");
}

TEST(AccountAdjustmentAmount, NegativeComponentIsPermitted) {
  OpenPitAccountAdjustmentAmount raw{};
  raw.balance =
      MakeRawAmount(OPENPIT_PARAM_ADJUSTMENT_AMOUNT_KIND_DELTA, "-42.5");

  const aa::Amount amount = aa::Amount::FromRaw(raw);
  ASSERT_TRUE(amount.balance.has_value());

  const OpenPitAccountAdjustmentAmount rebuilt = amount.Raw();
  EXPECT_EQ(rebuilt.balance.kind, OPENPIT_PARAM_ADJUSTMENT_AMOUNT_KIND_DELTA);
  EXPECT_EQ(param::PositionSize::FromRaw(rebuilt.balance.value).ToString(),
            "-42.5");
}

TEST(AccountAdjustmentAmount, AllNotSetComponentsReadAsEmptyOptional) {
  const aa::Amount amount =
      aa::Amount::FromRaw(OpenPitAccountAdjustmentAmount{});
  EXPECT_FALSE(amount.balance.has_value());
  EXPECT_FALSE(amount.held.has_value());
  EXPECT_FALSE(amount.incoming.has_value());
}

//------------------------------------------------------------------------------
// Bounds group

TEST(AccountAdjustmentBounds, PresentBoundsRoundTripExactly) {
  aa::Bounds bounds;
  bounds.balanceUpper = param::PositionSize::FromString("100");
  bounds.balanceLower = param::PositionSize::FromString("20");
  bounds.incomingUpper = param::PositionSize::FromString("50");
  bounds.incomingLower = param::PositionSize::FromString("-5");

  const aa::Bounds restored = aa::Bounds::FromRaw(bounds.Raw());

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
  const aa::Bounds restored = aa::Bounds::FromRaw(aa::Bounds().Raw());
  EXPECT_FALSE(restored.balanceUpper.has_value());
  EXPECT_FALSE(restored.balanceLower.has_value());
  EXPECT_FALSE(restored.heldUpper.has_value());
  EXPECT_FALSE(restored.heldLower.has_value());
  EXPECT_FALSE(restored.incomingUpper.has_value());
  EXPECT_FALSE(restored.incomingLower.has_value());
}

//------------------------------------------------------------------------------
// AccountAdjustment

TEST(AccountAdjustment, FullRawRoundTripPreservesEveryGroup) {
  aa::AccountAdjustment adjustment;

  aa::BalanceOperation balance;
  balance.asset = "USD";
  balance.averageEntryPrice = param::Price::FromString("101.5");
  adjustment.operation = aa::Operation::OfBalance(balance);

  OpenPitAccountAdjustmentAmount amountRaw{};
  amountRaw.balance =
      MakeRawAmount(OPENPIT_PARAM_ADJUSTMENT_AMOUNT_KIND_DELTA, "10");
  adjustment.amount = aa::Amount::FromRaw(amountRaw);

  aa::Bounds bounds;
  bounds.balanceUpper = param::PositionSize::FromString("100");
  adjustment.bounds = bounds;

  adjustment.userData = 99;

  const aa::AccountAdjustment restored =
      aa::AccountAdjustment::FromRaw(adjustment.Raw());

  ASSERT_TRUE(restored.operation.has_value());
  ASSERT_TRUE(restored.operation->IsBalance());
  ASSERT_TRUE(restored.operation->AsBalance()->asset.has_value());
  EXPECT_EQ(*restored.operation->AsBalance()->asset, "USD");
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
  const aa::AccountAdjustment restored =
      aa::AccountAdjustment::FromRaw(aa::AccountAdjustment().Raw());
  EXPECT_FALSE(restored.operation.has_value());
  EXPECT_FALSE(restored.amount.has_value());
  EXPECT_FALSE(restored.bounds.has_value());
  EXPECT_EQ(restored.userData, 0u);
}

//------------------------------------------------------------------------------
// OutcomeAmount / AccountOutcomeEntry / Outcome

TEST(AccountAdjustmentOutcomeAmount, RawRoundTripPreservesDeltaAndAbsolute) {
  const aa::OutcomeAmount amount(param::PositionSize::FromString("-3"),
                                 param::PositionSize::FromString("17"));

  const aa::OutcomeAmount restored = aa::OutcomeAmount::FromRaw(amount.Raw());
  EXPECT_EQ(restored.delta.ToString(), "-3");
  EXPECT_EQ(restored.absolute.ToString(), "17");
}

TEST(AccountAdjustmentPnlOutcomeAmount, RawRoundTripPreservesDeltaAndAbsolute) {
  const aa::PnlOutcomeAmount amount(param::Pnl::FromString("20"),
                                    param::Pnl::FromString("50"));

  const aa::PnlOutcomeAmount restored =
      aa::PnlOutcomeAmount::FromRaw(amount.Raw());
  EXPECT_EQ(restored.delta.ToString(), "20");
  EXPECT_EQ(restored.absolute.ToString(), "50");
}

TEST(AccountAdjustmentOutcomeEntry, PresentAndAbsentAmountsRoundTrip) {
  aa::AccountOutcomeEntry entry;
  entry.asset = "USD";
  entry.balance = aa::OutcomeAmount(param::PositionSize::FromString("5"),
                                    param::PositionSize::FromString("5"));
  entry.realizedPnl = aa::PnlOutcome{aa::PnlOutcomeAmount(
      param::Pnl::FromString("-2.5"), param::Pnl::FromString("7.5"))};
  entry.averageEntryPrice = param::Price::FromString("101.25");
  // held and incoming intentionally left absent.

  const aa::AccountOutcomeEntry restored =
      aa::AccountOutcomeEntry::FromRaw(entry.Raw());

  EXPECT_EQ(restored.asset, "USD");
  ASSERT_TRUE(restored.balance.has_value());
  EXPECT_EQ(restored.balance->delta.ToString(), "5");
  EXPECT_FALSE(restored.held.has_value());
  EXPECT_FALSE(restored.incoming.has_value());
  ASSERT_TRUE(restored.realizedPnl.has_value());
  const auto& pnl = std::get<aa::PnlOutcomeAmount>(restored.realizedPnl->Get());
  EXPECT_EQ(pnl.delta.ToString(), "-2.5");
  EXPECT_EQ(pnl.absolute.ToString(), "7.5");
  ASSERT_TRUE(restored.averageEntryPrice.has_value());
  EXPECT_EQ(restored.averageEntryPrice->ToString(), "101.25");
}

TEST(AccountAdjustmentOutcome, RawRoundTripPreservesGroupAndEntry) {
  aa::Outcome outcome;
  outcome.policyGroupId = param::GroupId(7);
  outcome.entry.asset = "ETH";
  outcome.entry.incoming =
      aa::OutcomeAmount(param::PositionSize::FromString("1"),
                        param::PositionSize::FromString("9"));

  const aa::Outcome restored = aa::Outcome::FromRaw(outcome.Raw());
  EXPECT_EQ(restored.policyGroupId, param::GroupId(7));
  EXPECT_EQ(restored.entry.asset, "ETH");
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
  seedOperation.asset = "AAPL";
  seedOperation.realizedPnl = param::Pnl::FromString("30");
  seed.operation = aa::Operation::OfBalance(std::move(seedOperation));

  const openpit::AdjustmentResult seedResult = engine.ApplyAccountAdjustment(
      accountId, std::vector<aa::AccountAdjustment>{seed});
  ASSERT_TRUE(seedResult.Passed());

  aa::AccountAdjustment forceSet;
  aa::BalanceOperation forceSetOperation;
  forceSetOperation.asset = "AAPL";
  forceSetOperation.realizedPnl = param::Pnl::FromString("50");
  forceSet.operation = aa::Operation::OfBalance(std::move(forceSetOperation));

  const openpit::AdjustmentResult result = engine.ApplyAccountAdjustment(
      accountId, std::vector<aa::AccountAdjustment>{forceSet});
  ASSERT_TRUE(result.Passed());
  ASSERT_EQ(result.accountAdjustmentOutcomes.size(), 1u);

  const aa::AccountOutcomeEntry& entry =
      result.accountAdjustmentOutcomes.front().entry;
  EXPECT_EQ(entry.asset, "AAPL");
  ASSERT_TRUE(entry.realizedPnl.has_value());
  const auto& pnl = std::get<aa::PnlOutcomeAmount>(entry.realizedPnl->Get());
  EXPECT_EQ(pnl.delta.ToString(), "20");
  EXPECT_EQ(pnl.absolute.ToString(), "50");
}

//------------------------------------------------------------------------------
// OutcomeList / BatchError (empty / null handle behavior)

TEST(AccountAdjustmentOutcomeList, DefaultIsEmptyAndNull) {
  const aa::OutcomeList list;
  EXPECT_FALSE(static_cast<bool>(list));
  EXPECT_EQ(list.Get(), nullptr);
  EXPECT_EQ(list.Size(), 0u);
  EXPECT_TRUE(list.Empty());
  EXPECT_FALSE(list.Get(0).has_value());
  EXPECT_TRUE(list.ToVector().empty());
}

TEST(AccountAdjustmentBatchError, DefaultIsNullWithNoRejects) {
  const aa::BatchError error;
  EXPECT_FALSE(static_cast<bool>(error));
  EXPECT_EQ(error.Get(), nullptr);
  EXPECT_EQ(error.FailedAdjustmentIndex(), 0u);
  EXPECT_TRUE(error.Rejects().empty());
}

}  // namespace
