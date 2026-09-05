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
#include "spot_loadtest/generator/event.hpp"

#include <map>
#include <string>
#include <utility>

// The shadow fund model.
//
// An INDEPENDENT reimplementation of the spot-funds arithmetic that never
// imports the engine - which is what makes the per-op oracle non-circular. It
// reproduces policy-contract section 2 exactly, including the engine's
// prune-when-zero behaviour: a slot whose available and held both reach zero is
// removed, so a missing slot and an all-zero slot are indistinguishable.

namespace spot_loadtest::generator {

// One (account, asset) balance: total = available + held, both >= 0 normally.
struct Holdings {
  Decimal available;
  Decimal held;
};

// The predicted outcome of an order check plus the predicted post-op balance of
// the charge asset.
struct PreTradeResult {
  RejectReason reason = RejectReason::None;
  std::string chargeAsset;
  Decimal chargeAmount;
  Decimal postAvailable;
  Decimal postHeld;

  [[nodiscard]] bool Accepted() const noexcept {
    return reason == RejectReason::None;
  }
};

// The predicted post-settlement balance of both affected assets after a full
// fill.
struct SettlementResult {
  std::string heldAsset;
  std::string creditAsset;
  Holdings heldPost;
  Holdings creditPost;
  Decimal creditAmount;
  bool error =
      false; // settlement underflow (the generator must never schedule it).
};

// The predicted post-funding balance of the funded asset.
struct FundingResult {
  RejectReason reason = RejectReason::None;
  Holdings post;
  bool rejected = false;
};

class Ledger {
public:
  Ledger() = default;

  // Returns the slot and whether it exists; a missing slot reads as zero.
  [[nodiscard]] std::pair<Holdings, bool> Get(const std::string &account,
                                              const std::string &asset) const;

  // The available balance for (account, asset); 0 if absent.
  [[nodiscard]] Decimal Available(const std::string &account,
                                  const std::string &asset) const;

  // Mirrors SpotFundsPolicy::execute_pre_trade for one order: compute the
  // charge, then try_hold on the charge asset. A zero charge is a clean no-op
  // accept; on accept available -= charge, held += charge.
  [[nodiscard]] PreTradeResult PreTrade(const std::string &account, Side side,
                                        const std::string &underlying,
                                        const std::string &settlement,
                                        const Decimal &quantity,
                                        const Decimal &price);

  // Mirrors SpotFundsPolicy fill handling for a full fill: consumes the held
  // reservation and credits the opposite leg's available.
  [[nodiscard]] SettlementResult
  SettleFullFill(const std::string &account, Side side,
                 const std::string &underlying, const std::string &settlement,
                 const Decimal &quantity, const Decimal &price);

  // Mirrors the AccountAdjustment balance operation on available; held is never
  // touched. Absolute sets, Delta adds, both unconditionally (negative results
  // are accepted).
  [[nodiscard]] FundingResult ApplyFunding(const std::string &account,
                                           const std::string &asset,
                                           FundingKind kind,
                                           const Decimal &amount);

private:
  struct AssetKey {
    std::string account;
    std::string asset;
    [[nodiscard]] bool operator<(const AssetKey &o) const {
      if (account != o.account) {
        return account < o.account;
      }
      return asset < o.asset;
    }
  };

  // Stores a slot, pruning it when both legs are zero (mirrors remove_if_zero).
  void Set(const std::string &account, const std::string &asset, Holdings h);

  std::map<AssetKey, Holdings> m_holdings;
};

// The settlement-asset charge for a Buy order: q*p. Quantity is an integer lot
// count (scale 0) so the product is exact at the price scale.
[[nodiscard]] inline Decimal ChargeForBuy(const Decimal &quantity,
                                          const Decimal &price) {
  return price.MulInt(quantity.ToWholeInt());
}

} // namespace spot_loadtest::generator
