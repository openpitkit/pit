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

#include <cstdint>

namespace openpit::param {

enum class RoundingStrategy : std::uint8_t {
  MidpointNearestEven = 0,
  MidpointAwayFromZero = 1,
  Up = 2,
  Down = 3,
};

/// Buy/sell direction.
enum class Side : std::uint8_t {
  /// Buy or increase positive exposure.
  Buy = 1,
  /// Sell or increase negative exposure.
  Sell = 2,
};

enum class FillType : std::uint8_t {
  Trade = 1,
  Liquidation = 2,
  AutoDeleverage = 3,
  Settlement = 4,
  Funding = 5,
};

enum class Kind : std::uint8_t {
  Quantity = 1,
  Volume = 2,
  Notional = 3,
  Price = 4,
  Pnl = 5,
  CashFlow = 6,
  PositionSize = 7,
  Fee = 8,
  Leverage = 9,
};

}  // namespace openpit::param
