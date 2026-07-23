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

#include <limits>
#include <optional>
#include <string>
#include <type_traits>

namespace {

using openpit::param::AccountGroupId;
using openpit::param::Asset;
using openpit::param::Fee;
using openpit::param::GroupId;
using openpit::param::Leverage;
using openpit::param::MonetaryAmount;
using openpit::param::Pnl;
using openpit::param::Price;
using openpit::param::Quantity;
using openpit::param::Volume;

namespace model = openpit::model;

static_assert(std::is_copy_constructible_v<Asset>);
static_assert(std::is_copy_assignable_v<Asset>);

//------------------------------------------------------------------------------
// Asset

TEST(ParamAsset, ValidatesOwnsAndCopiesValue) {
  const Asset asset("AAPL");
  const Asset copied(asset);
  Asset assigned("MSFT");
  assigned = asset;

  EXPECT_EQ(asset.View(), "AAPL");
  EXPECT_EQ(copied, asset);
  EXPECT_EQ(assigned, asset);
}

TEST(ParamAsset, EmptyAndWhitespaceOnlyValuesThrowStructuredError) {
  EXPECT_THROW({ (void)Asset(""); }, openpit::Error);

  try {
    (void)Asset(" \t");
    FAIL() << "expected whitespace-only asset to fail";
  } catch (const openpit::Error& error) {
    ASSERT_TRUE(error.Code().has_value());
    EXPECT_EQ(*error.Code(), openpit::ParamErrorCode::AssetEmpty);
  }
}

//------------------------------------------------------------------------------
// Instrument

TEST(ModelInstrument, PreservesAssets) {
  const model::Instrument instrument(::openpit::param::Asset("SPX"),
                                     ::openpit::param::Asset("USD"));
  EXPECT_EQ(instrument.underlyingAsset.View(), "SPX");
  EXPECT_EQ(instrument.settlementAsset.View(), "USD");
}

//------------------------------------------------------------------------------
// TradeAmount

TEST(ModelTradeAmount, QuantityKindPreservesExactValue) {
  const model::TradeAmount amount =
      model::TradeAmount::OfQuantity(Quantity::FromString("2.5"));
  EXPECT_EQ(amount.Kind(), model::TradeAmountKind::Quantity);

  ASSERT_EQ(amount.Kind(), model::TradeAmountKind::Quantity);
  const std::optional<Quantity> quantity = amount.AsQuantity();
  ASSERT_TRUE(quantity.has_value());
  EXPECT_EQ(quantity->ToString(), "2.5");
  EXPECT_FALSE(amount.AsVolume().has_value());
}

TEST(ModelTradeAmount, VolumeKindPreservesExactValue) {
  const model::TradeAmount amount =
      model::TradeAmount::OfVolume(Volume::FromString("1500"));
  EXPECT_EQ(amount.Kind(), model::TradeAmountKind::Volume);

  ASSERT_EQ(amount.Kind(), model::TradeAmountKind::Volume);
  const std::optional<Volume> volume = amount.AsVolume();
  ASSERT_TRUE(volume.has_value());
  EXPECT_EQ(volume->ToString(), "1500");
}

//------------------------------------------------------------------------------
// Order

TEST(ModelOrder, LimitFactorySetsRequiredOperationFields) {
  const model::Order order = model::Order::Limit(
      model::Instrument(::openpit::param::Asset("AAPL"),
                        ::openpit::param::Asset("USD")),
      ::openpit::param::AccountId::FromUint64(7), model::Side::Buy,
      model::TradeAmount::OfQuantity(Quantity::FromString("3")),
      Price::FromString("185.25"));

  ASSERT_TRUE(order.operation.has_value());
  ASSERT_TRUE(order.operation->instrument.has_value());
  EXPECT_EQ(order.operation->instrument->underlyingAsset.View(), "AAPL");
  EXPECT_EQ(order.operation->instrument->settlementAsset.View(), "USD");
  ASSERT_TRUE(order.operation->accountId.has_value());
  EXPECT_EQ(order.operation->accountId->ToString(), "7");
  EXPECT_EQ(order.operation->side,
            std::optional<model::Side>(model::Side::Buy));
  ASSERT_TRUE(order.operation->tradeAmount.has_value());
  EXPECT_EQ(order.operation->tradeAmount->AsQuantity()->ToString(), "3");
  ASSERT_TRUE(order.operation->price.has_value());
  EXPECT_EQ(order.operation->price->ToString(), "185.25");
}

TEST(ModelOrder, FullModelPreservesEveryGroup) {
  model::Order order;
  model::OrderOperation operation;
  operation.instrument = model::Instrument(::openpit::param::Asset("AAPL"),
                                           ::openpit::param::Asset("USD"));
  operation.tradeAmount =
      model::TradeAmount::OfQuantity(Quantity::FromString("3"));
  operation.price = Price::FromString("185.25");
  operation.accountId = ::openpit::param::AccountId::FromUint64(7);
  operation.side = model::Side::Buy;
  order.operation = operation;

  model::OrderPosition position;
  position.positionSide = model::PositionSide::Long;
  position.reduceOnly = true;
  position.closePosition = false;
  order.position = position;

  model::OrderMargin margin;
  margin.collateralAsset = ::openpit::param::Asset("USD");
  margin.autoBorrow = true;
  margin.leverage = Leverage::FromUint16(20);  // raw 200 (20.0x)
  order.margin = margin;

  order.userData = 42;

  const model::Order& restored = order;

  ASSERT_TRUE(restored.operation.has_value());
  ASSERT_TRUE(restored.operation->instrument.has_value());
  EXPECT_EQ(restored.operation->instrument->underlyingAsset.View(), "AAPL");
  EXPECT_EQ(restored.operation->instrument->settlementAsset.View(), "USD");
  ASSERT_TRUE(restored.operation->tradeAmount.has_value());
  ASSERT_TRUE(restored.operation->tradeAmount->AsQuantity().has_value());
  EXPECT_EQ(restored.operation->tradeAmount->AsQuantity()->ToString(), "3");
  ASSERT_TRUE(restored.operation->price.has_value());
  EXPECT_EQ(restored.operation->price->ToString(), "185.25");
  ASSERT_TRUE(restored.operation->accountId.has_value());
  EXPECT_EQ(restored.operation->accountId->ToString(), "7");
  ASSERT_TRUE(restored.operation->side.has_value());
  EXPECT_EQ(*restored.operation->side, model::Side::Buy);

  ASSERT_TRUE(restored.position.has_value());
  ASSERT_TRUE(restored.position->positionSide.has_value());
  EXPECT_EQ(*restored.position->positionSide, model::PositionSide::Long);
  EXPECT_EQ(restored.position->reduceOnly, std::optional<bool>(true));
  EXPECT_EQ(restored.position->closePosition, std::optional<bool>(false));

  ASSERT_TRUE(restored.margin.has_value());
  ASSERT_TRUE(restored.margin->collateralAsset.has_value());
  EXPECT_EQ(restored.margin->collateralAsset->View(), "USD");
  EXPECT_EQ(restored.margin->autoBorrow, std::optional<bool>(true));
  ASSERT_TRUE(restored.margin->leverage.has_value());
  EXPECT_EQ(restored.margin->leverage->Value(), 20.0F);

  EXPECT_EQ(restored.userData, 42u);
}

TEST(ModelOrder, EmptyOrderHasNoGroups) {
  const model::Order restored;
  EXPECT_FALSE(restored.operation.has_value());
  EXPECT_FALSE(restored.margin.has_value());
  EXPECT_FALSE(restored.position.has_value());
  EXPECT_EQ(restored.userData, 0u);
}

TEST(ModelOrder, PresentFalseBooleanGroupsRemainPresent) {
  model::Order order;
  model::OrderMargin margin;
  margin.autoBorrow = false;  // present-but-false, not absent
  order.margin = margin;
  model::OrderPosition position;
  position.reduceOnly = false;
  position.closePosition = false;
  order.position = position;

  const model::Order& restored = order;
  ASSERT_TRUE(restored.margin.has_value());
  EXPECT_EQ(restored.margin->autoBorrow, std::optional<bool>(false));
  EXPECT_FALSE(restored.margin->collateralAsset.has_value());
  EXPECT_FALSE(restored.margin->leverage.has_value());
  ASSERT_TRUE(restored.position.has_value());
  EXPECT_EQ(restored.position->reduceOnly, std::optional<bool>(false));
  EXPECT_FALSE(restored.position->positionSide.has_value());
}

TEST(ModelOrder, IsUsableAsPolymorphicBase) {
  model::Order concrete;
  concrete.userData = 9;
  const openpit::Order& base = concrete;
  const auto* recovered = dynamic_cast<const model::Order*>(&base);
  ASSERT_NE(recovered, nullptr);
  EXPECT_EQ(recovered->userData, 9u);
}

//------------------------------------------------------------------------------
// ExecutionReport

TEST(ModelExecutionReport, FullModelPreservesEveryGroup) {
  model::ExecutionReport report;

  model::ExecutionReportOperation operation;
  operation.instrument = model::Instrument(::openpit::param::Asset("BTC"),
                                           ::openpit::param::Asset("USD"));
  operation.accountId = ::openpit::param::AccountId::FromUint64(3);
  operation.side = model::Side::Sell;
  report.operation = operation;

  model::FinancialImpact financialImpact;
  financialImpact.pnl = Pnl::FromString("-12.50");
  financialImpact.fee = Fee::FromString("0.75");
  report.financialImpact = financialImpact;

  model::Fill fill;
  fill.lastTrade =
      model::Trade(Price::FromString("100.5"), Quantity::FromString("1"));
  fill.fee =
      MonetaryAmount(Fee::FromString("0.25"), ::openpit::param::Asset("USD"));
  fill.leavesQuantity = Quantity::FromString("2");
  fill.isFinal = true;
  report.fill = fill;

  model::PositionImpact positionImpact;
  positionImpact.positionEffect = model::PositionEffect::Open;
  positionImpact.positionSide = model::PositionSide::Short;
  report.positionImpact = positionImpact;

  report.userData = 5;

  const model::ExecutionReport& restored = report;

  ASSERT_TRUE(restored.operation.has_value());
  ASSERT_TRUE(restored.operation->instrument.has_value());
  EXPECT_EQ(restored.operation->instrument->underlyingAsset.View(), "BTC");
  EXPECT_EQ(restored.operation->accountId->ToString(), "3");
  EXPECT_EQ(*restored.operation->side, model::Side::Sell);

  ASSERT_TRUE(restored.financialImpact.has_value());
  ASSERT_TRUE(restored.financialImpact->pnl.has_value());
  EXPECT_EQ(restored.financialImpact->pnl->ToString(), "-12.50");
  ASSERT_TRUE(restored.financialImpact->fee.has_value());
  EXPECT_EQ(restored.financialImpact->fee->ToString(), "0.75");

  ASSERT_TRUE(restored.fill.has_value());
  ASSERT_TRUE(restored.fill->lastTrade.has_value());
  EXPECT_EQ(restored.fill->lastTrade->price.ToString(), "100.5");
  EXPECT_EQ(restored.fill->lastTrade->quantity.ToString(), "1");
  ASSERT_TRUE(restored.fill->fee.has_value());
  EXPECT_EQ(restored.fill->fee->Amount().ToString(), "0.25");
  EXPECT_EQ(restored.fill->fee->Currency().View(), "USD");
  ASSERT_TRUE(restored.fill->leavesQuantity.has_value());
  EXPECT_EQ(restored.fill->leavesQuantity->ToString(), "2");
  EXPECT_EQ(restored.fill->isFinal, std::optional<bool>(true));

  ASSERT_TRUE(restored.positionImpact.has_value());
  EXPECT_EQ(*restored.positionImpact->positionEffect,
            model::PositionEffect::Open);
  EXPECT_EQ(*restored.positionImpact->positionSide, model::PositionSide::Short);

  EXPECT_EQ(restored.userData, 5u);
}

TEST(ModelExecutionReport, EmptyReportHasNoGroups) {
  const model::ExecutionReport restored;
  EXPECT_FALSE(restored.operation.has_value());
  EXPECT_FALSE(restored.financialImpact.has_value());
  EXPECT_FALSE(restored.fill.has_value());
  EXPECT_FALSE(restored.positionImpact.has_value());
}

TEST(ModelExecutionReport, StructuredFillFeePreservesValue) {
  model::Fill fill;
  fill.fee =
      MonetaryAmount(Fee::FromString("1.25"), ::openpit::param::Asset("EUR"));

  ASSERT_TRUE(fill.fee.has_value());
  EXPECT_EQ(fill.fee->Amount().ToString(), "1.25");
  EXPECT_EQ(fill.fee->Currency().View(), "EUR");
}

TEST(ModelExecutionReport, IsUsableAsPolymorphicBase) {
  model::ExecutionReport concrete;
  concrete.userData = 11;
  const openpit::ExecutionReport& base = concrete;
  const auto* recovered = dynamic_cast<const model::ExecutionReport*>(&base);
  ASSERT_NE(recovered, nullptr);
  EXPECT_EQ(recovered->userData, 11u);
}

//------------------------------------------------------------------------------
// GroupId

TEST(ParamGroupId, DefaultsToReservedZero) {
  const GroupId group;
  EXPECT_EQ(group.Value(), openpit::param::DefaultPolicyGroupId);
  EXPECT_EQ(group.Value(), 0u);
}

TEST(ParamGroupId, CarriesExplicitValue) {
  const GroupId group(7);
  EXPECT_EQ(group.Value(), 7u);
  EXPECT_NE(group, GroupId(8));
  EXPECT_EQ(group, GroupId(7));
}

TEST(ParamLeverage, FloatConversionUsesRepresentableFixedPointRange) {
  EXPECT_EQ(Leverage::FromFloat(2999.9F).Value(), 2999.9F);
  EXPECT_EQ(Leverage::FromFloat(1.26F).Value(), 1.3F);
  EXPECT_THROW(
      static_cast<void>(Leverage::FromFloat(std::numeric_limits<float>::max())),
      openpit::Error);
  EXPECT_THROW(static_cast<void>(Leverage::FromUint16(6554)), openpit::Error);
}

TEST(ParamMonetaryAmount, PreservesAmountAndCurrency) {
  const MonetaryAmount amount(Fee::FromString("-0.125"),
                              ::openpit::param::Asset("USD"));
  EXPECT_EQ(amount.Amount().ToString(), "-0.125");
  EXPECT_EQ(amount.Currency().View(), "USD");
}

//------------------------------------------------------------------------------
// AccountGroupId

TEST(ParamAccountGroupId, FromUint32IsStablePassthrough) {
  const AccountGroupId group = AccountGroupId::FromUint32(42);
  EXPECT_FALSE(group.IsDefault());
  EXPECT_EQ(group.ToString(), "42");
}

TEST(ParamAccountGroupId, FromUint32RejectsReservedDefault) {
  EXPECT_THROW({ (void)AccountGroupId::FromUint32(0); }, openpit::Error);
}

TEST(ParamAccountGroupId, FromStringIsDeterministicAndNonZero) {
  const AccountGroupId first = AccountGroupId::FromString("desk-1");
  const AccountGroupId second = AccountGroupId::FromString("desk-1");
  EXPECT_EQ(first, second);
  EXPECT_FALSE(first.IsDefault());
  EXPECT_NE(first, AccountGroupId::FromString("desk-2"));
}

TEST(ParamAccountGroupId, FromStringRejectsEmpty) {
  EXPECT_THROW({ (void)AccountGroupId::FromString(""); }, openpit::Error);
}

TEST(ParamAccountGroupId, DefaultAccountGroupIsReservedZero) {
  EXPECT_TRUE(openpit::param::DefaultAccountGroup.IsDefault());
  EXPECT_EQ(openpit::param::DefaultAccountGroup.ToString(), "0");
}

}  // namespace
