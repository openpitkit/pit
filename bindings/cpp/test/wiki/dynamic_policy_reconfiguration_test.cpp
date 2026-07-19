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

// Source: Dynamic-Policy-Reconfiguration.md
//
// Compiling mirror of the C++ snippets published on the Dynamic Policy
// Reconfiguration wiki page. Each TEST keeps the same user code as the
// corresponding C++ subsection, with only the harness assertions required to
// make the snippet executable.

#include <gtest/gtest.h>
#include <openpit/openpit.hpp>
#include <openpit/pretrade/policies.hpp>

#include <iostream>
#include <optional>
#include <utility>
#include <vector>

namespace {

namespace aa = openpit::accountadjustment;
namespace policies = openpit::pretrade::policies;
using openpit::param::AccountId;
using openpit::param::AdjustmentAmount;
using openpit::param::Pnl;
using openpit::param::PositionSize;
using openpit::param::Price;
using openpit::param::Quantity;

[[nodiscard]] openpit::model::Order WikiOrder(std::string quantity,
                                              std::string price) {
  openpit::model::Order order;
  openpit::model::OrderOperation op;
  op.instrument = openpit::model::Instrument(::openpit::param::Asset("AAPL"),
                                             ::openpit::param::Asset("USD"));
  op.accountId = AccountId::FromUint64(99224416);
  op.side = openpit::model::Side::Buy;
  op.tradeAmount =
      openpit::model::TradeAmount::OfQuantity(Quantity::FromString(quantity));
  op.price = Price::FromString(price);
  order.operation = std::move(op);
  return order;
}

// Used in: pit.wiki/Dynamic-Policy-Reconfiguration.md - Retune a Built-in
// Policy
TEST(DynamicPolicyReconfigurationWiki, RateLimit) {
  const openpit::model::Order order = WikiOrder("1", "100");

  // Register the rate-limit policy through the builder so the engine keeps a
  // handle to its settings; built-in policies are configurable by name.
  openpit::EngineBuilder builder(openpit::SyncPolicy::None);
  builder.Add(policies::RateLimitPolicy{}.BrokerBarrier(
      policies::RateLimitBrokerBarrier(
          policies::RateLimit(/*maxOrders=*/5,
                              /*windowNanoseconds=*/60'000'000'000))));
  openpit::Engine engine = builder.Build();

  // The generous limit of 5 admits the first three orders.
  for (int i = 0; i < 3; ++i) {
    openpit::pretrade::ExecuteResult result = engine.ExecutePreTrade(order);
    ASSERT_TRUE(result.Passed());
    result.reservation->Commit();
  }

  // Tighten the broker limit to 2 at runtime, without rebuilding the engine.
  // Built-in policies register under their type name (RateLimitPolicyName).
  engine.Configure().RateLimit(
      policies::RateLimitPolicyName,
      policies::RateLimitBrokerBarrier(policies::RateLimit(
          /*maxOrders=*/2, /*windowNanoseconds=*/60'000'000'000)));

  // The next order would have passed under the old limit of 5; the new limit
  // of 2 rejects it, proving the live policy reads the retuned value.
  const openpit::pretrade::ExecuteResult rejected =
      engine.ExecutePreTrade(order);
  ASSERT_FALSE(rejected.Passed());
  ASSERT_EQ(rejected.rejects.size(), 1u);
  std::cout << rejected.rejects[0].reason << "\n";
  EXPECT_EQ(rejected.rejects[0].reason, "rate limit exceeded: broker barrier");
}

// Used in: pit.wiki/Dynamic-Policy-Reconfiguration.md - Force-set Accumulated
// P&L
TEST(DynamicPolicyReconfigurationWiki, SetAccountPnl) {
  const AccountId account = AccountId::FromUint64(99224416);
  const openpit::model::Order order = WikiOrder("1", "100");

  // Register the kill-switch policy through the builder so the engine keeps a
  // handle to its accumulator; built-in policies are configurable by name.
  openpit::EngineBuilder builder(openpit::SyncPolicy::None);
  policies::PnlBoundsBrokerBarrier barrier{openpit::param::Asset("USD")};
  barrier.lowerBound = Pnl::FromString("-100");
  builder.Add(policies::PnlBoundsKillSwitchPolicy{}.BrokerBarrier(barrier));
  openpit::Engine engine = builder.Build();

  // With no P&L history the order passes against the lower bound of -100.
  openpit::pretrade::ExecuteResult first = engine.ExecutePreTrade(order);
  ASSERT_TRUE(first.Passed());
  first.reservation->Commit();

  // Force-set the account's accumulated P&L to -150 USD, below the bound.
  // Built-in policies register under their type name
  // (PnlBoundsKillSwitchPolicyName).
  engine.Configure().SetAccountPnl(policies::PnlBoundsKillSwitchPolicyName,
                                   account, openpit::param::Asset("USD"),
                                   Pnl::FromString("-150"));

  // The next order for that account breaches the lower bound and is rejected;
  // the breach also latches an engine-level block on the account.
  const openpit::pretrade::ExecuteResult rejected =
      engine.ExecutePreTrade(order);
  ASSERT_FALSE(rejected.Passed());
  ASSERT_EQ(rejected.rejects.size(), 1u);
  std::cout << rejected.rejects[0].reason << "\n";
  EXPECT_EQ(rejected.rejects[0].reason,
            "pnl kill switch triggered: broker barrier");
}

// Used in: pit.wiki/Dynamic-Policy-Reconfiguration.md - Spot Funds: Global
// Limit Mode
TEST(DynamicPolicyReconfigurationWiki, SpotFundsGlobalLimitMode) {
  openpit::EngineBuilder builder(openpit::SyncPolicy::None);
  builder.Add(policies::SpotFundsPolicy{});
  openpit::Engine engine = builder.Build();

  const AccountId account = AccountId::FromUint64(99224416);
  // Seed 1 000 USD - not enough for 10 AAPL @ 200 (= 2 000 notional).
  aa::AccountAdjustment seed;
  aa::BalanceOperation balanceOp;
  balanceOp.asset = ::openpit::param::Asset("USD");
  seed.operation = aa::Operation::OfBalance(std::move(balanceOp));
  aa::Amount seedAmount;
  seedAmount.balance =
      AdjustmentAmount::OfAbsolute(PositionSize::FromString("1000"));
  seed.amount = std::move(seedAmount);

  const openpit::AdjustmentResult seedResult = engine.ApplyAccountAdjustment(
      account, std::vector<aa::AccountAdjustment>{seed});
  ASSERT_TRUE(seedResult.Passed());

  openpit::model::Order order;
  openpit::model::OrderOperation op;
  op.instrument = openpit::model::Instrument(::openpit::param::Asset("AAPL"),
                                             ::openpit::param::Asset("USD"));
  op.accountId = account;
  op.side = openpit::model::Side::Buy;
  op.tradeAmount =
      openpit::model::TradeAmount::OfQuantity(Quantity::FromString("10"));
  op.price = Price::FromString("200");
  order.operation = std::move(op);

  // Default Enforce: 2 000 notional exceeds 1 000 available - rejected.
  openpit::pretrade::ExecuteResult rejected = engine.ExecutePreTrade(order);
  ASSERT_FALSE(rejected.Passed());
  ASSERT_FALSE(rejected.rejects.empty());
  std::cout << rejected.rejects[0].reason << "\n";
  EXPECT_EQ(rejected.rejects[0].reason, "spot funds insufficient");

  // Switch to TrackOnly: the same order now passes and reserves against
  // deficit.
  engine.Configure().SpotFundsGlobalLimitMode(
      policies::SpotFundsPolicyName, policies::SpotFundsLimitMode::TrackOnly);
  openpit::pretrade::ExecuteResult accepted = engine.ExecutePreTrade(order);
  ASSERT_TRUE(accepted.Passed());
  accepted.reservation->Commit();  // available: 1 000 - 2 000 = -1 000

  // Restore Enforce: available is negative - still rejected.
  engine.Configure().SpotFundsGlobalLimitMode(
      policies::SpotFundsPolicyName, policies::SpotFundsLimitMode::Enforce);
  rejected = engine.ExecutePreTrade(order);
  ASSERT_FALSE(rejected.Passed());
  ASSERT_FALSE(rejected.rejects.empty());
  std::cout << rejected.rejects[0].reason << "\n";
  EXPECT_EQ(rejected.rejects[0].reason, "spot funds insufficient");
}

// Used in: pit.wiki/Dynamic-Policy-Reconfiguration.md - Spot Funds: Per-Account
// Limit Mode
TEST(DynamicPolicyReconfigurationWiki, SpotFundsPerAccountLimitMode) {
  openpit::EngineBuilder builder(openpit::SyncPolicy::None);
  builder.Add(policies::SpotFundsPolicy{});
  openpit::Engine engine = builder.Build();

  const AccountId account = AccountId::FromUint64(99224416);
  // Seed 1 000 USD - not enough for 10 AAPL @ 200 (= 2 000 notional).
  aa::AccountAdjustment seed;
  aa::BalanceOperation balanceOp;
  balanceOp.asset = ::openpit::param::Asset("USD");
  seed.operation = aa::Operation::OfBalance(std::move(balanceOp));
  aa::Amount seedAmount;
  seedAmount.balance =
      AdjustmentAmount::OfAbsolute(PositionSize::FromString("1000"));
  seed.amount = std::move(seedAmount);

  const openpit::AdjustmentResult seedResult = engine.ApplyAccountAdjustment(
      account, std::vector<aa::AccountAdjustment>{seed});
  ASSERT_TRUE(seedResult.Passed());

  openpit::model::Order order;
  openpit::model::OrderOperation op;
  op.instrument = openpit::model::Instrument(::openpit::param::Asset("AAPL"),
                                             ::openpit::param::Asset("USD"));
  op.accountId = account;
  op.side = openpit::model::Side::Buy;
  op.tradeAmount =
      openpit::model::TradeAmount::OfQuantity(Quantity::FromString("10"));
  op.price = Price::FromString("200");
  order.operation = std::move(op);

  // Global Enforce: under-funded buy is rejected.
  openpit::pretrade::ExecuteResult rejected = engine.ExecutePreTrade(order);
  ASSERT_FALSE(rejected.Passed());
  ASSERT_FALSE(rejected.rejects.empty());
  std::cout << rejected.rejects[0].reason << "\n";
  EXPECT_EQ(rejected.rejects[0].reason, "spot funds insufficient");

  // Pin this account to TrackOnly: per-account override wins over global
  // Enforce.
  engine.Configure().SpotFundsAccountLimitMode(
      policies::SpotFundsPolicyName, account,
      policies::SpotFundsLimitMode::TrackOnly);
  openpit::pretrade::ExecuteResult accepted = engine.ExecutePreTrade(order);
  ASSERT_TRUE(accepted.Passed());
  // Reservation recorded despite insufficient funds.
  accepted.reservation->Commit();

  // Clear the per-account override: cascade falls back to global Enforce.
  engine.Configure().SpotFundsAccountLimitMode(policies::SpotFundsPolicyName,
                                               account, std::nullopt);
  rejected = engine.ExecutePreTrade(order);
  ASSERT_FALSE(rejected.Passed());
  ASSERT_FALSE(rejected.rejects.empty());
  std::cout << rejected.rejects[0].reason << "\n";
  EXPECT_EQ(rejected.rejects[0].reason, "spot funds insufficient");
}

}  // namespace
