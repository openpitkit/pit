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

// Source: Market-Data-Pricing.md
//
// Compiling mirror of the C++ snippet published on the Market-Data-Pricing wiki
// page.  Each TEST runs the same user code shown in the corresponding C++
// subsection, wrapped only in the minimal engine / market-data harness (setup +
// asserts) the snippet elides for readability.  The published snippet body and
// the test body must stay in lock-step.

#include "openpit/accountadjustment/account_adjustment.hpp"
#include "openpit/engine.hpp"
#include "openpit/marketdata.hpp"
#include "openpit/model.hpp"
#include "openpit/param.hpp"
#include "openpit/pretrade/pretrade.hpp"

#include <gtest/gtest.h>
#include <openpit.h>

#include <cassert>
#include <utility>
#include <vector>

namespace {

namespace md = openpit::marketdata;
namespace policies = openpit::pretrade::policies;

using openpit::Engine;
using openpit::EngineBuilder;
using openpit::SyncPolicy;
using openpit::accountadjustment::AccountAdjustment;
using openpit::accountadjustment::Amount;
using openpit::accountadjustment::BalanceOperation;
using openpit::accountadjustment::Operation;
using openpit::accountadjustment::Outcome;
using openpit::param::AccountId;
using openpit::param::AdjustmentAmount;
using openpit::param::PositionSize;
using openpit::param::Price;
using openpit::param::Quantity;
using openpit::pretrade::RejectCode;

//------------------------------------------------------------------------------
// Pricing Market Orders

// Mirrors the "Pricing Market Orders" C++ subsection of Market-Data-Pricing.md:
// book-top pricing, per-instrument slippage override, and the BookTop
// no-fallback rejection when the ask is absent from the latest quote.
TEST(MarketDataPricingWiki, BookTopPricingAndMarkUnavailableReject) {
  // A shared market-data service feeds the policy's market-order pricing.
  EngineBuilder builder(SyncPolicy::None);
  md::Service marketData = md::Builder::FromEngineSyncPolicy(
                               md::QuoteTtl::Infinite(), SyncPolicy::None)
                               .Build();
  const openpit::model::Instrument aapl(::openpit::param::Asset("AAPL"),
                                        ::openpit::param::Asset("USD"));
  const md::RegisterResult registration = marketData.Register(aapl);
  ASSERT_EQ(registration.status, md::RegisterStatus::Ok);
  ASSERT_TRUE(registration.instrumentId.has_value());
  const md::InstrumentId aaplId = registration.instrumentId.value();
  ASSERT_EQ(marketData.Push(aaplId, md::Quote()
                                        .WithMark(Price::FromString("200"))
                                        .WithBid(Price::FromString("199.5"))
                                        .WithAsk(Price::FromString("200.5"))),
            md::RegisterStatus::Ok);

  // Price from the top of book; AAPL overrides the global 100 bps slippage to
  // zero, so a buy is priced exactly at the ask.
  policies::SpotFundsOverride aaplOverride(aaplId);
  aaplOverride.slippageBps = 0;
  policies::SpotFundsPolicy{}
      .WithMarketOrders(marketData, 100)
      .PricingSource(policies::SpotFundsPricingSource::BookTop)
      .Override(aaplOverride)
      .AddTo(builder);
  Engine engine = builder.Build();

  const AccountId accountId = AccountId::FromUint64(99224416);
  AccountAdjustment seed;
  BalanceOperation balanceOp;
  balanceOp.asset = ::openpit::param::Asset("USD");
  seed.operation = Operation::OfBalance(std::move(balanceOp));
  Amount seedAmount;
  seedAmount.balance =
      AdjustmentAmount::OfAbsolute(PositionSize::FromString("1000"));
  seed.amount = std::move(seedAmount);
  const openpit::AdjustmentResult seedResult = engine.ApplyAccountAdjustment(
      accountId, std::vector<AccountAdjustment>{seed});
  ASSERT_TRUE(seedResult.Passed());

  auto marketBuy = [&]() {
    return openpit::model::Order::Market(
        aapl, accountId, openpit::model::Side::Buy,
        openpit::model::TradeAmount::OfQuantity(Quantity::FromString("1")));
  };

  // Market buy (no price): priced at the ask 200.5, which the balance covers.
  openpit::pretrade::ExecuteResult first = engine.ExecutePreTrade(marketBuy());
  ASSERT_TRUE(first.Passed());
  first.reservation->Commit();

  // Replace with a mark-only quote: bid and ask are gone, so BookTop can no
  // longer price a buy and the next market order is rejected.
  ASSERT_EQ(
      marketData.Push(aaplId, md::Quote().WithMark(Price::FromString("215"))),
      md::RegisterStatus::Ok);
  openpit::pretrade::ExecuteResult second = engine.ExecutePreTrade(marketBuy());
  ASSERT_FALSE(second.Passed());
  ASSERT_FALSE(second.rejects.empty());
  EXPECT_EQ(second.rejects[0].code, RejectCode::MarkPriceUnavailable);
}

}  // namespace
