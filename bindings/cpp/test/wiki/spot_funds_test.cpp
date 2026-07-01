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

// Source: Spot-Funds.md
//
// Compiling mirror of the C++ snippets published on the Spot-Funds wiki page.
// Each TEST runs the same user code shown in the corresponding C++ subsection,
// wrapped only in the minimal engine / market-data harness (setup + asserts)
// the snippet elides for readability. The published snippet body and the test
// body must stay in lock-step.

#include "openpit/account_adjustment.hpp"
#include "openpit/engine.hpp"
#include "openpit/marketdata.hpp"
#include "openpit/model.hpp"
#include "openpit/param.hpp"
#include "openpit/pretrade/pretrade.hpp"

#include <gtest/gtest.h>
#include <openpit.h>

#include <cassert>
#include <optional>
#include <utility>
#include <vector>

namespace {

//------------------------------------------------------------------------------
// Limit-Only Mode (Default)

TEST(SpotFundsWiki, LimitOnlyReservesSettlementOnBuy) {
  namespace policies = openpit::pretrade::policies;
  namespace aa = openpit::accountadjustment;
  using openpit::param::AccountId;
  using openpit::param::AdjustmentAmount;
  using openpit::param::PositionSize;
  using openpit::param::Price;
  using openpit::param::Quantity;

  // Limit-only spot funds: register first in the policy list.
  openpit::EngineBuilder builder(openpit::SyncPolicy::None);
  policies::SpotFundsPolicy{}.AddTo(builder);
  openpit::Engine engine = builder.Build();

  const AccountId accountId = AccountId::FromUint64(99224416);

  // Seed 10000 USD of available funds through the account-adjustment pipeline.
  aa::AccountAdjustment seed;
  aa::BalanceOperation balanceOp;
  balanceOp.asset = "USD";
  seed.operation = aa::Operation::OfBalance(std::move(balanceOp));
  aa::Amount seedAmount;
  seedAmount.balance =
      AdjustmentAmount::OfAbsolute(PositionSize::FromString("10000"));
  seed.amount = std::move(seedAmount);

  const openpit::AdjustmentResult seedResult = engine.ApplyAccountAdjustment(
      accountId, std::vector<aa::AccountAdjustment>{seed});
  assert(seedResult.Passed());

  // Buy 10 AAPL @ 200 holds 2000 USD; available drops to 8000.
  openpit::model::Order order = openpit::model::Order::Limit(
      openpit::model::Instrument("AAPL", "USD"), accountId,
      openpit::model::Side::Buy,
      openpit::model::TradeAmount::OfQuantity(Quantity::FromString("10")),
      Price::FromString("200"));

  openpit::pretrade::ExecuteResult result = engine.ExecutePreTrade(order);
  if (result.Passed()) {
    result.reservation->Commit();
  }

  // Harness assertions: seeding accepted, the priced buy passed and reserved.
  EXPECT_TRUE(seedResult.Passed());
  EXPECT_TRUE(result.Passed());
  EXPECT_TRUE(result.rejects.empty());
}

//------------------------------------------------------------------------------
// Market Orders

TEST(SpotFundsWiki, MarketBuyPricedFromMarkWithSlippage) {
  namespace md = openpit::marketdata;
  namespace policies = openpit::pretrade::policies;
  namespace aa = openpit::accountadjustment;
  using openpit::param::AccountId;
  using openpit::param::AdjustmentAmount;
  using openpit::param::PositionSize;
  using openpit::param::Price;
  using openpit::param::Quantity;

  // The engine builder fixes the sync mode; the market-data service is built to
  // match so the policy can read live quotes for market-order pricing.
  openpit::EngineBuilder builder(openpit::SyncPolicy::None);

  // A shared market-data service feeds the policy's market-order pricing. It
  // must outlive the engine, which prices each market order from its live
  // quotes.
  md::Service marketData =
      md::Builder::FromEngineSyncPolicy(md::QuoteTtl::Infinite(),
                                        openpit::SyncPolicy::None)
          .Build();
  const openpit::model::Instrument aapl("AAPL", "USD");
  const md::RegisterResult registration = marketData.Register(aapl);
  assert(registration.status == md::RegisterStatus::Ok);
  assert(registration.instrumentId.has_value());
  const md::InstrumentId aaplId = registration.instrumentId.value();
  assert(marketData.Push(aaplId, md::Quote().WithMark(Price::FromString(
                                     "200"))) == md::RegisterStatus::Ok);

  // Spot funds with market orders enabled at 1500 bps worst-case slippage,
  // priced from the quote mark.
  policies::SpotFundsPolicy{}
      .WithMarketOrders(marketData, 1500)
      .PricingSource(policies::SpotFundsPricingSource::Mark)
      .AddTo(builder);
  openpit::Engine engine = builder.Build();

  const AccountId accountId = AccountId::FromUint64(99224416);
  aa::AccountAdjustment seed;
  aa::BalanceOperation balanceOp;
  balanceOp.asset = "USD";
  seed.operation = aa::Operation::OfBalance(std::move(balanceOp));
  aa::Amount seedAmount;
  seedAmount.balance =
      AdjustmentAmount::OfAbsolute(PositionSize::FromString("10000"));
  seed.amount = std::move(seedAmount);

  const openpit::AdjustmentResult seedResult = engine.ApplyAccountAdjustment(
      accountId, std::vector<aa::AccountAdjustment>{seed});
  assert(seedResult.Passed());

  // Market buy (no price): priced at mark 200 + 15% = 230 per unit worst case.
  openpit::model::Order order = openpit::model::Order::Market(
      aapl, accountId, openpit::model::Side::Buy,
      openpit::model::TradeAmount::OfQuantity(Quantity::FromString("5")));

  openpit::pretrade::ExecuteResult result = engine.ExecutePreTrade(order);
  if (result.Passed()) {
    result.reservation->Commit();
  }

  // Harness assertions: seeding accepted, the market buy passed and reserved.
  EXPECT_TRUE(seedResult.Passed());
  EXPECT_TRUE(result.Passed());
  EXPECT_TRUE(result.rejects.empty());
}

//------------------------------------------------------------------------------
// Self-Computed PnL Kill Switch - Configuring Barriers

TEST(SpotFundsWiki, PnlBoundsKillSwitchBuilder) {
  namespace policies = openpit::pretrade::policies;
  using openpit::param::AccountId;
  using openpit::param::Pnl;

  const AccountId accountId = AccountId::FromUint64(99224416);

  // The PnL kill switch is a distinct spot-funds builder entry point; it
  // produces the same SpotFundsPolicy, registered under the same name.
  policies::SpotFundsPnlBoundsBarrier global("USD");
  global.lowerBound = Pnl::FromString("-1000");

  policies::SpotFundsPnlBoundsBarrier accountBarrier("USD");
  accountBarrier.lowerBound = Pnl::FromString("-250");

  openpit::EngineBuilder builder(openpit::SyncPolicy::None);
  builder.Add(
      policies::SpotFundsPnlBoundsKillSwitchPolicy{}
          .GlobalBarrier(std::move(global))
          .AccountBarrier(policies::SpotFundsPnlBoundsAccountBarrier(
              accountId, std::move(accountBarrier), Pnl::FromString("0"))));
  openpit::Engine engine = builder.Build();

  // Harness assertion: the builder accepted the SpotFunds PnL barriers.
  EXPECT_TRUE(static_cast<bool>(engine));
}

//------------------------------------------------------------------------------
// Self-Computed PnL Kill Switch - Runtime Reconfiguration

TEST(SpotFundsWiki, PnlBoundsKillSwitchRuntimeReconfiguration) {
  namespace policies = openpit::pretrade::policies;
  using openpit::param::AccountId;
  using openpit::param::Pnl;

  const AccountId accountId = AccountId::FromUint64(99224416);
  policies::SpotFundsPnlBoundsBarrier initial("USD");
  initial.lowerBound = Pnl::FromString("-1000");

  openpit::EngineBuilder builder(openpit::SyncPolicy::None);
  builder.Add(policies::SpotFundsPnlBoundsKillSwitchPolicy{}.GlobalBarrier(
      std::move(initial)));
  openpit::Engine engine = builder.Build();

  // Retune the account-currency PnL barriers; live accumulated PnL is
  // untouched.
  policies::SpotFundsPnlBoundsBarrier global("USD");
  global.lowerBound = Pnl::FromString("-500");
  engine.Configure().SpotFundsPnlBoundsKillSwitch(
      policies::SpotFundsPolicyName,
      std::vector<policies::SpotFundsPnlBoundsBarrier>{std::move(global)});

  // Force-set the live accumulated PnL for one (account, account currency).
  engine.Configure().SetSpotFundsAccountPnl(
      policies::SpotFundsPolicyName, accountId, "USD", Pnl::FromString("-120"));

  // Harness assertion: both runtime calls completed without throwing.
  EXPECT_TRUE(static_cast<bool>(engine));
}

}  // namespace
