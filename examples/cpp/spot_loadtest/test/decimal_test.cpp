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

// Decimal parsing tests: the accepted value space and its range boundary.

#include "spot_loadtest/decimal.hpp"

#include <gtest/gtest.h>

#include <limits>
#include <stdexcept>
#include <string>

namespace {

using spot_loadtest::Decimal;

constexpr Decimal::Coefficient kMaxCoefficient =
    std::numeric_limits<Decimal::Coefficient>::max();

TEST(Decimal, ParsesPinnedScaleValues) {
  EXPECT_EQ(Decimal::FromString("12.3").RawCoefficient(), 1230);
  EXPECT_EQ(Decimal::FromString("-0.05").RawCoefficient(), -5);
  EXPECT_EQ(Decimal::FromString("+7").RawCoefficient(), 700);
  EXPECT_EQ(Decimal::FromString(".25").RawCoefficient(), 25);
}

TEST(Decimal, AcceptsMaxMagnitude) {
  const Decimal value = Decimal::FromString("92233720368547758.07");
  EXPECT_EQ(value.RawCoefficient(), kMaxCoefficient);
  EXPECT_EQ(value.ToString(), "92233720368547758.07");
}

TEST(Decimal, AcceptsNegatedMaxMagnitude) {
  const Decimal value = Decimal::FromString("-92233720368547758.07");
  EXPECT_EQ(value.RawCoefficient(), -kMaxCoefficient);
  EXPECT_EQ(value.ToString(), "-92233720368547758.07");
}

TEST(Decimal, AcceptsLeadingZeros) {
  EXPECT_EQ(Decimal::FromString("000000000000000000001.00").RawCoefficient(),
            100);
  EXPECT_EQ(Decimal::FromString(std::string(64, '0') + "92233720368547758.07")
                .RawCoefficient(),
            kMaxCoefficient);
}

TEST(Decimal, RefusesOutOfRange) {
  for (const std::string &text : {
           std::string("92233720368547758.08"),
           std::string("-92233720368547758.08"),
           std::string("92233720368547759"),
           std::string("-92233720368547759"),
           std::string("184467440737095516.16"),
           std::string("100000000000000000000"),
           std::string(400, '9'),
       }) {
    EXPECT_THROW((void)Decimal::FromString(text), std::invalid_argument)
        << text;
  }
}

TEST(Decimal, RefusesMalformedInput) {
  for (const char *text :
       {"", "-", "+", ".", "-.", "a", "1a", "1.2x", "1.234", "1..2"}) {
    EXPECT_THROW((void)Decimal::FromString(text), std::invalid_argument)
        << text;
  }
}

} // namespace
