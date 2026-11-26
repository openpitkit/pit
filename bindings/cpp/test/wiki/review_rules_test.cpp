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

// Source: https://wiki.openpit.dev/Review-Rules/#c
// Keep this example in sync with the matching wiki example.

#include <gtest/gtest.h>
#include <openpit/openpit.hpp>

#include <stdexcept>

namespace {

TEST(ReviewRulesWiki, RoutesOrdersByScopeAndMode) {
  using namespace openpit;
  using namespace openpit::pretrade::policies;
  using openpit::reject::RejectCode;
  using openpit::reject::RuleMode;

  const auto group = param::AccountGroupId::FromUint32(7);
  const auto newClient = param::AccountId::FromUint64(1001);
  const auto restricted = param::AccountId::FromUint64(1002);
  const auto instrument = [](const char* symbol) {
    return model::Instrument(param::Asset(symbol), param::Asset("USD"));
  };
  const auto aapl = instrument("AAPL");
  const auto msft = instrument("MSFT");
  const auto tsla = instrument("TSLA");

  EngineBuilder builder(SyncPolicy::None);
  builder.Add(ReviewRulePolicy{}.AccountGroupRule(
                  {group, ReviewRuleInstruments::All()}),
              {"new-clients-review", RuleMode::ReviewWithHold});
  builder.Add(ReviewRulePolicy{}.AccountRule(
                  {restricted, ReviewRuleInstruments::Exact({msft})}),
              {"restricted-instruments", RuleMode::Reject});
  const auto engine = builder.Build();
  if (auto error = engine.Accounts().RegisterGroup({newClient}, group)) {
    throw std::runtime_error(error->message);
  }

  const auto run = [&](param::AccountId account,
                       const model::Instrument& symbol) {
    model::Order order;
    auto& operation = order.operation.emplace();
    operation.accountId = account;
    operation.instrument = symbol;
    operation.side = model::Side::Buy;
    operation.tradeAmount =
        model::TradeAmount::OfQuantity(param::Quantity::FromString("10"));
    operation.price = param::Price::FromString("25");
    return engine.ExecutePreTradeDryRun(order);
  };
  auto review = run(newClient, aapl);
  ASSERT_TRUE(review.Passed());
  EXPECT_TRUE(review.Rejects().empty());
  const auto marks = review.Reviews();
  ASSERT_EQ(marks.size(), 1u);
  EXPECT_EQ(marks.front().mode, RuleMode::ReviewWithHold);

  const auto checkRejected = [&](param::AccountId account,
                                 const model::Instrument& symbol) {
    auto result = run(account, symbol);
    ASSERT_FALSE(result.Passed());
    ASSERT_EQ(result.Rejects().size(), 1u);
    EXPECT_EQ(result.Rejects().front().code, RejectCode::ComplianceRestriction);
    EXPECT_TRUE(result.Reviews().empty());
  };
  checkRejected(restricted, msft);
  auto pass = run(restricted, aapl);
  ASSERT_TRUE(pass.Passed());
  EXPECT_TRUE(pass.Rejects().empty());
  EXPECT_TRUE(pass.Reviews().empty());

  engine.Configure().ReviewRule(
      "restricted-instruments",
      ReviewRuleGlobalRuleUpdate::Set(ReviewRuleInstruments::Exact({tsla})));
  checkRejected(param::AccountId::FromUint64(1003), tsla);
  checkRejected(restricted, msft);
}

}  // namespace
