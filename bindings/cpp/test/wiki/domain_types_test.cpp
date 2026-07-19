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

// Source: Domain-Types.md
//
// Each TEST runs the exact user code published in a C++ snippet of the wiki
// page, plus the minimal asserts that accompany it. Keep the snippet bodies and
// these test bodies in sync.

#include "openpit/openpit.hpp"

#include <gtest/gtest.h>

#include <cassert>
#include <string>

namespace {

namespace model = openpit::model;

using openpit::param::Asset;
using openpit::param::Leverage;
using openpit::param::Pnl;
using openpit::param::Price;
using openpit::param::Quantity;

//------------------------------------------------------------------------------
// Example: Create Validated Values

TEST(WikiDomainTypes, CreateValidatedValues) {
  // Build validated value objects at the integration boundary. Asset codes and
  // the monetary fields are all exact, validated value types.
  const auto asset = Asset("AAPL");
  const auto quantity = Quantity::FromString("10.5");
  const auto price = Price::FromString("185");
  const auto pnl = Pnl::FromString("-12.5");

  // The wrappers normalize formatting while preserving domain meaning.
  EXPECT_EQ(asset.View(), "AAPL");
  EXPECT_EQ(quantity.ToString(), "10.5");
  EXPECT_EQ(price.ToString(), "185");
  EXPECT_EQ(pnl.ToString(), "-12.5");
}

//------------------------------------------------------------------------------
// Example: Work With Directional Types

TEST(WikiDomainTypes, WorkWithDirectionalTypes) {
  // Directional enums keep side logic explicit instead of comparing raw
  // strings.
  const model::Side side = model::Side::Buy;
  const model::PositionSide positionSide = model::PositionSide::Long;

  EXPECT_EQ(side, model::Side::Buy);
  EXPECT_EQ(positionSide, model::PositionSide::Long);
}

//------------------------------------------------------------------------------
// Example: Create Leverage

TEST(WikiDomainTypes, CreateLeverage) {
  // Pick the constructor that matches the upstream representation you receive.
  const Leverage fromMultiplier = Leverage::FromUint16(100);
  const Leverage fromFloat = Leverage::FromFloat(100.5F);

  // Both constructors end up with the same strongly typed leverage wrapper.
  EXPECT_EQ(fromMultiplier.Value(), 100.0F);
  EXPECT_EQ(fromFloat.Value(), 100.5F);
}

}  // namespace
