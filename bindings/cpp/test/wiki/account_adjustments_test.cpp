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

// Source: Account-Adjustments.md
//
// Compiling mirror of the C++ snippets published on the Account-Adjustments
// wiki page. Each TEST runs the same user code shown in a wiki code block
// (modulo the minimal harness: engine setup is the snippet's own, asserts wrap
// the published assertions). Keep the bodies in sync with the published
// snippets whenever either side changes.

#include "openpit/accountadjustment/account_adjustment.hpp"
#include "openpit/engine.hpp"
#include "openpit/model.hpp"
#include "openpit/param.hpp"
#include "openpit/pretrade/custom_policy.hpp"
#include "openpit/pretrade/policies.hpp"
#include "openpit/reject.hpp"
#include "openpit/tx.hpp"

#include <gtest/gtest.h>

#include <cassert>
#include <cstdint>
#include <map>
#include <memory>
#include <optional>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

namespace {

struct CumulativeLimitState {
  std::map<std::string, openpit::param::PositionSize> totals;
  std::size_t commits = 0;
  std::size_t rollbacks = 0;
};

// Tracks the last accepted absolute balance per asset. Each accepted element
// mutates eagerly so the next element in the same batch sees it, then registers
// commit/rollback callbacks with the engine transaction.
class CumulativeLimitPolicy {
 public:
  CumulativeLimitPolicy(openpit::param::PositionSize maxCumulative,
                        std::shared_ptr<CumulativeLimitState> state)
      : m_maxCumulative(maxCumulative), m_state(std::move(state)) {}

  [[nodiscard]] std::string_view Name() const noexcept {
    return "CumulativeLimitPolicy";
  }

  [[nodiscard]] openpit::pretrade::PolicyAccountAdjustmentResult
  ApplyAccountAdjustment(
      const openpit::accountadjustment::Context& context,
      openpit::param::AccountId accountId,
      const openpit::accountadjustment::AccountAdjustment& adjustment,
      openpit::tx::Mutations& mutations,
      openpit::pretrade::AccountOutcomes& outcomes) const {
    static_cast<void>(context);
    static_cast<void>(accountId);
    static_cast<void>(outcomes);

    const auto* balance =
        adjustment.operation ? adjustment.operation->AsBalance() : nullptr;
    if (balance == nullptr || !balance->asset || !adjustment.amount ||
        !adjustment.amount->balance ||
        !adjustment.amount->balance->IsAbsolute()) {
      return {};
    }

    const std::string asset = *balance->asset;
    const openpit::param::PositionSize next =
        adjustment.amount->balance->Value();
    if (next > m_maxCumulative) {
      openpit::pretrade::PolicyAccountAdjustmentResult result;
      result.decision.Push(openpit::pretrade::Reject(
          std::string(Name()), openpit::pretrade::RejectScope::Account,
          openpit::pretrade::RejectCode::RiskLimitExceeded,
          "cumulative limit exceeded",
          asset + " absolute balance exceeds the configured limit"));
      return result;
    }

    std::optional<openpit::param::PositionSize> previous;
    if (const auto it = m_state->totals.find(asset);
        it != m_state->totals.end()) {
      previous = it->second;
    }
    m_state->totals.insert_or_assign(asset, next);

    const auto state = m_state;
    mutations.Push([state] { ++state->commits; },
                   [state, asset, previous] {
                     ++state->rollbacks;
                     if (previous) {
                       state->totals.insert_or_assign(asset, *previous);
                     } else {
                       state->totals.erase(asset);
                     }
                   });
    return {};
  }

 private:
  openpit::param::PositionSize m_maxCumulative;
  std::shared_ptr<CumulativeLimitState> m_state;
};

[[nodiscard]] openpit::accountadjustment::AccountAdjustment AbsoluteBalance(
    std::string asset, std::string_view value) {
  openpit::accountadjustment::BalanceOperation balance;
  balance.asset = std::move(asset);

  openpit::accountadjustment::Amount amount;
  amount.balance = openpit::param::AdjustmentAmount::OfAbsolute(
      openpit::param::PositionSize::FromString(value));

  openpit::accountadjustment::AccountAdjustment adjustment;
  adjustment.operation =
      openpit::accountadjustment::Operation::OfBalance(std::move(balance));
  adjustment.amount = std::move(amount);
  return adjustment;
}

// Mirrors the "Example: Balance Limit Policy" C++ block. The first batch
// commits one value. In the second batch, the first element mutates policy
// state, the second rejects, and the engine rolls the earlier mutation back.
TEST(AccountAdjustmentsWiki, CumulativeBalanceLimitCommitsAndRollsBack) {
  const auto state = std::make_shared<CumulativeLimitState>();
  openpit::EngineBuilder builder(openpit::SyncPolicy::None);
  openpit::pretrade::CustomPolicy<CumulativeLimitPolicy> policy(
      "CumulativeLimitPolicy",
      CumulativeLimitPolicy(openpit::param::PositionSize::FromString("1000000"),
                            state));
  builder.Add(policy);
  const openpit::Engine engine = builder.Build();

  const openpit::param::AccountId accountId =
      openpit::param::AccountId::FromUint64(99224416);

  const openpit::AdjustmentResult accepted = engine.ApplyAccountAdjustment(
      accountId, std::vector<openpit::accountadjustment::AccountAdjustment>{
                     AbsoluteBalance("USD", "100")});
  assert(accepted.Passed());
  assert(state->totals.at("USD").ToString() == "100");
  assert(state->commits == 1);

  const openpit::AdjustmentResult rejected = engine.ApplyAccountAdjustment(
      accountId,
      std::vector<openpit::accountadjustment::AccountAdjustment>{
          AbsoluteBalance("USD", "200"), AbsoluteBalance("USD", "2000000")});
  assert(!rejected.Passed());
  assert(rejected.batchError->FailedAdjustmentIndex() == 1);
  assert(state->totals.at("USD").ToString() == "100");
  assert(state->commits == 1);
  assert(state->rollbacks == 1);

  EXPECT_FALSE(rejected.Passed());
}

// Mirrors the "Examples" mixed balance/position batch block. Builds one batch
// that sets a USD cash balance and an SPX/USD hedged position by absolute
// value, then applies it as a single atomic engine call and asserts acceptance.
TEST(AccountAdjustmentsWiki, MixedBalanceAndPositionBatchApplies) {
  namespace aa = openpit::accountadjustment;
  namespace param = openpit::param;
  namespace policies = openpit::pretrade::policies;

  // Build one batch that mixes balance and position adjustments.
  const param::AccountId accountId = param::AccountId::FromUint64(99224416);

  aa::AccountAdjustment cashAdj;
  {
    aa::BalanceOperation balance;
    balance.asset = "USD";
    cashAdj.operation = aa::Operation::OfBalance(std::move(balance));
    aa::Amount amount;
    amount.balance = param::AdjustmentAmount::OfAbsolute(
        param::PositionSize::FromString("10000"));
    cashAdj.amount = std::move(amount);
  }

  aa::AccountAdjustment posAdj;
  {
    aa::PositionOperation position;
    position.instrument = openpit::model::Instrument("SPX", "USD");
    position.collateralAsset = "USD";
    position.averageEntryPrice = param::Price::FromString("95000");
    position.mode = openpit::model::PositionMode::Hedged;
    posAdj.operation = aa::Operation::OfPosition(std::move(position));
    aa::Amount amount;
    amount.balance = param::AdjustmentAmount::OfAbsolute(
        param::PositionSize::FromString("-3"));
    posAdj.amount = std::move(amount);
  }

  const std::vector<aa::AccountAdjustment> adjustments{std::move(cashAdj),
                                                       std::move(posAdj)};

  // The engine validates the whole batch atomically.
  openpit::EngineBuilder builder(openpit::SyncPolicy::None);
  builder.Add(policies::OrderValidationPolicy{});
  const openpit::Engine engine = builder.Build();

  // On accept the result passes and carries the per-asset account-adjustment
  // outcomes.
  const openpit::AdjustmentResult result =
      engine.ApplyAccountAdjustment(accountId, adjustments);
  assert(result.Passed());
  assert(result.accountBlocks.empty());

  EXPECT_TRUE(result.Passed());
}

}  // namespace
