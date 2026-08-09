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

#pragma once

#include "spot_loadtest/decimal.hpp"

#include <cstdint>

// Pinned money/quantity/price precision.
//
// v1 (limit + quantity-denominated) deliberately restricts the value space so
// the only charge formula, q*p (Buy), is exact with no rounding: quantity is an
// integer lot count (scale 0) and price has at most priceScale (= 2) fractional
// digits. Then q*p has at most two fractional digits and reproduces the
// engine's position-size arithmetic exactly, keeping the shadow ledger strict.

namespace spot_loadtest::generator {

// The maximum number of fractional digits a price may carry.
inline constexpr int kPriceScale = 2;

// Converts an integer lot count to a scale-0 decimal.
[[nodiscard]] inline Decimal QuantityDecimal(std::uint64_t lots) {
  return Decimal::FromInt(static_cast<std::int64_t>(lots));
}

// Builds a price from an integer number of cents, yielding an exact scale-2
// decimal (e.g. 15000 -> 150.00).
[[nodiscard]] inline Decimal PriceFromCents(std::uint64_t cents) {
  return Decimal::FromCents(static_cast<std::int64_t>(cents));
}

} // namespace spot_loadtest::generator
