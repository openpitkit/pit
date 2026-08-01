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

#include <cstdint>
#include <string>
#include <type_traits>

namespace {

static_assert(!std::is_invocable_v<decltype(openpit::detail::Native),
                                   const openpit::param::Price&>);
static_assert(!std::is_invocable_v<
              decltype(openpit::detail::FromNative<openpit::param::Price>),
              OpenPitParamPrice>);

TEST(Runtime, VersionIsNonEmpty) {
  const std::string version = openpit::GetVersion();
  EXPECT_FALSE(version.empty());
}

TEST(Runtime, BuildProfileReportsKnownKeys) {
  const std::string profile = openpit::GetBuildProfile();
  EXPECT_NE(profile.find("version="), std::string::npos);
  EXPECT_NE(profile.find("debug_assertions="), std::string::npos);
}

TEST(Reject, EvaluationFailureClassificationIsExposed) {
  using openpit::pretrade::IsEvaluationFailure;
  using openpit::pretrade::RejectCode;

  EXPECT_TRUE(IsEvaluationFailure(RejectCode::MissingRequiredField));
  EXPECT_TRUE(IsEvaluationFailure(RejectCode::MarkPriceUnavailable));
  EXPECT_TRUE(IsEvaluationFailure(RejectCode::ArithmeticOverflow));
  EXPECT_FALSE(IsEvaluationFailure(RejectCode::InsufficientFunds));
  EXPECT_FALSE(IsEvaluationFailure(RejectCode::PnlKillSwitchTriggered));
  EXPECT_FALSE(IsEvaluationFailure(RejectCode::Other));
  EXPECT_FALSE(IsEvaluationFailure(static_cast<RejectCode>(65535)));
}

TEST(Param, PriceRoundTripsExactDecimalFromString) {
  const openpit::param::Price price =
      openpit::param::Price::FromString("185.25");
  EXPECT_EQ(price.ToString(), "185.25");
  EXPECT_FALSE(price.IsZero());
}

TEST(Param, PriceComparesExactly) {
  const openpit::param::Price low = openpit::param::Price::FromString("0.10");
  const openpit::param::Price high = openpit::param::Price::FromString("0.20");
  EXPECT_LT(low, high);
  EXPECT_NE(low, high);
  EXPECT_EQ(low.Compare(low), 0);
}

TEST(Param, QuantityZeroIsZero) {
  const openpit::param::Quantity zero = openpit::param::Quantity::FromInt64(0);
  EXPECT_TRUE(zero.IsZero());
}

TEST(Param, InvalidDecimalStringThrowsError) {
  EXPECT_THROW(
      { (void)openpit::param::Price::FromString("not-a-number"); },
      openpit::Error);
}

template <typename Value>
void ExpectExactValueArithmeticSurface() {
  using openpit::param::Decimal;
  using openpit::param::RoundingStrategy;

  const Value value = Value::FromString("12");
  const Decimal decimal = value.Decimal();
  EXPECT_EQ(Value::FromDecimal(decimal).ToString(), "12");
  EXPECT_EQ(
      Value::FromDoubleRounded(1.25, 1, RoundingStrategy::MidpointAwayFromZero)
          .ToString(),
      "1.3");
  EXPECT_EQ(Value::FromDecimalRounded(Decimal{125, 0, 2}, 1,
                                      RoundingStrategy::MidpointAwayFromZero)
                .ToString(),
            "1.3");
  EXPECT_EQ(value.CheckedMulInt(-2).CheckedNeg().ToString(), "24");
  EXPECT_EQ(value.CheckedMulUint(2).ToString(), "24");
  EXPECT_EQ(value.CheckedMulFloat(2.0).ToString(), "24");
  EXPECT_EQ(value.CheckedDivInt(2).ToString(), "6");
  EXPECT_EQ(value.CheckedDivUint(2).ToString(), "6");
  EXPECT_EQ(value.CheckedDivFloat(2.0).ToString(), "6");
  EXPECT_EQ(value.CheckedRemInt(5).ToString(), "2");
  EXPECT_EQ(value.CheckedRemUint(5).ToString(), "2");
  EXPECT_EQ(value.CheckedRemFloat(5.0).ToString(), "2");
  EXPECT_THROW(static_cast<void>(value.CheckedDivInt(0)), openpit::Error);
}

template <typename Value>
void ExpectUnsignedExactValueArithmeticSurface() {
  using openpit::param::Decimal;
  using openpit::param::RoundingStrategy;

  const Value value = Value::FromString("12");
  EXPECT_EQ(Value::FromDecimal(value.Decimal()).ToString(), "12");
  EXPECT_EQ(
      Value::FromDoubleRounded(1.25, 1, RoundingStrategy::MidpointAwayFromZero)
          .ToString(),
      "1.3");
  EXPECT_EQ(Value::FromDecimalRounded(Decimal{125, 0, 2}, 1,
                                      RoundingStrategy::MidpointAwayFromZero)
                .ToString(),
            "1.3");
  EXPECT_EQ(value.CheckedMulInt(2).ToString(), "24");
  EXPECT_EQ(value.CheckedMulUint(2).ToString(), "24");
  EXPECT_EQ(value.CheckedMulFloat(2.0).ToString(), "24");
  EXPECT_EQ(value.CheckedDivInt(2).ToString(), "6");
  EXPECT_EQ(value.CheckedDivUint(2).ToString(), "6");
  EXPECT_EQ(value.CheckedDivFloat(2.0).ToString(), "6");
  EXPECT_EQ(value.CheckedRemInt(5).ToString(), "2");
  EXPECT_EQ(value.CheckedRemUint(5).ToString(), "2");
  EXPECT_EQ(value.CheckedRemFloat(5.0).ToString(), "2");
  EXPECT_THROW(static_cast<void>(value.CheckedDivInt(0)), openpit::Error);
}

TEST(Param, ExactValueTypesExposeCompleteArithmeticSurface) {
  ExpectExactValueArithmeticSurface<openpit::param::Price>();
  ExpectUnsignedExactValueArithmeticSurface<openpit::param::Quantity>();
  ExpectUnsignedExactValueArithmeticSurface<openpit::param::Volume>();
  ExpectExactValueArithmeticSurface<openpit::param::Pnl>();
  ExpectExactValueArithmeticSurface<openpit::param::Fee>();
  ExpectExactValueArithmeticSurface<openpit::param::PositionSize>();
  ExpectExactValueArithmeticSurface<openpit::param::CashFlow>();
  ExpectUnsignedExactValueArithmeticSurface<openpit::param::Notional>();
}

TEST(Param, DomainValueConversionsExposeCompleteOopSurface) {
  using namespace openpit::param;

  static_assert(std::is_same_v<openpit::model::Side, Side>);
  const Price price = Price::FromString("2");
  const Quantity quantity = Quantity::FromString("3");
  const Volume volume = Volume::FromString("6");
  const Fee fee = Fee::FromString("-1");
  const Pnl pnl = Pnl::FromString("-2");

  EXPECT_EQ(price.CalculateVolume(quantity).ToString(), "6");
  EXPECT_EQ(price.CalculatePositionSize(quantity).ToString(), "6");
  EXPECT_EQ(price.CalculateNotional(quantity).ToString(), "6");
  EXPECT_EQ(quantity.CalculateVolume(price).ToString(), "6");
  EXPECT_EQ(quantity.CalculateNotional(price).ToString(), "6");
  EXPECT_EQ(quantity.ToPositionSize().ToString(), "3");
  EXPECT_EQ(quantity.ToPositionSize(Side::Sell).ToString(), "-3");
  EXPECT_EQ(volume.CalculateQuantity(price).ToString(), "3");
  EXPECT_EQ(volume.ToCashFlowInflow().ToString(), "6");
  EXPECT_EQ(volume.ToCashFlowOutflow().ToString(), "-6");
  EXPECT_EQ(volume.ToPositionSize().ToString(), "6");
  EXPECT_EQ(volume.ToNotional().ToString(), "6");
  EXPECT_EQ(Volume::FromNotional(Notional::FromString("6")).ToString(), "6");
  EXPECT_EQ(Pnl::FromFee(fee).ToString(), "1");
  EXPECT_EQ(pnl.ToCashFlow().ToString(), "-2");
  EXPECT_EQ(pnl.ToPositionSize().ToString(), "-2");
  EXPECT_EQ(fee.ToPnl().ToString(), "1");
  EXPECT_EQ(fee.ToPositionSize().ToString(), "1");
  EXPECT_EQ(fee.ToCashFlow().ToString(), "1");
  EXPECT_EQ(PositionSize::FromPnl(pnl).ToString(), "-2");
  EXPECT_EQ(PositionSize::FromFee(fee).ToString(), "1");
  EXPECT_EQ(PositionSize::FromQuantityAndSide(quantity, Side::Buy).ToString(),
            "3");
  EXPECT_EQ(CashFlow::FromPnl(pnl).ToString(), "-2");
  EXPECT_EQ(CashFlow::FromFee(fee).ToString(), "1");
  EXPECT_EQ(CashFlow::FromVolumeInflow(volume).ToString(), "6");
  EXPECT_EQ(CashFlow::FromVolumeOutflow(volume).ToString(), "-6");
  EXPECT_EQ(Notional::FromVolume(volume).ToVolume().ToString(), "6");
  EXPECT_EQ(Notional::FromVolume(volume)
                .CalculateMarginRequired(Leverage::FromUint16(2))
                .ToString(),
            "3");
  EXPECT_EQ(ToString(Side::Buy), "BUY");
  EXPECT_EQ(openpit::model::ToString(openpit::model::PositionSide::Long),
            "LONG");
  EXPECT_EQ(openpit::model::ToString(openpit::model::PositionEffect::Close),
            "CLOSE");
  EXPECT_EQ(openpit::model::ToString(openpit::model::PositionMode::Hedged),
            "hedged");
  EXPECT_FALSE(
      openpit::model::TradeAmount::OfQuantity(quantity).ToString().empty());

  const PositionSize shortPosition =
      PositionSize::FromQuantityAndSide(quantity, Side::Sell);
  const auto [openQuantity, openSide] = shortPosition.OpenQuantity();
  EXPECT_EQ(openQuantity.ToString(), "3");
  EXPECT_EQ(openSide, Side::Sell);
  const auto [closeQuantity, closeSide] = shortPosition.CloseQuantity();
  EXPECT_EQ(closeQuantity.ToString(), "3");
  ASSERT_TRUE(closeSide.has_value());
  EXPECT_EQ(*closeSide, Side::Buy);
  EXPECT_EQ(
      shortPosition.CheckedAddQuantity(Quantity::FromString("1"), Side::Buy)
          .ToString(),
      "-2");

  const auto [zeroQuantity, zeroCloseSide] =
      PositionSize::FromInt64(0).CloseQuantity();
  EXPECT_TRUE(zeroQuantity.IsZero());
  EXPECT_FALSE(zeroCloseSide.has_value());
}

TEST(Engine, BuilderConstructsForEverySyncPolicy) {
  EXPECT_NO_THROW(
      { openpit::EngineBuilder builder(openpit::SyncPolicy::None); });
  EXPECT_NO_THROW(
      { openpit::EngineBuilder builder(openpit::SyncPolicy::Full); });
  EXPECT_NO_THROW(
      { openpit::EngineBuilder builder(openpit::SyncPolicy::Account); });
}

TEST(Engine, BuildWithoutPoliciesThrows) {
  openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
  // The slice does not register policies yet; the boundary failure surfaces as
  // a thrown openpit::Error. The builder handle is released by RAII regardless.
  EXPECT_THROW({ openpit::Engine engine = builder.Build(); }, openpit::Error);
}

TEST(Reject, CarriesScopeAndCode) {
  const openpit::reject::Reject reject(
      "order_size_limit", openpit::reject::RejectScope::Order,
      openpit::reject::RejectCode::OrderQtyExceedsLimit, "qty too large",
      "max 100");
  EXPECT_EQ(reject.scope, openpit::reject::RejectScope::Order);
  EXPECT_EQ(reject.code, openpit::reject::RejectCode::OrderQtyExceedsLimit);
  EXPECT_EQ(reject.policy, "order_size_limit");
}

}  // namespace
