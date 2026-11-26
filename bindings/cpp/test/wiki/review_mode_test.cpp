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

// Source: https://wiki.openpit.dev/Review-Mode/#c
// Keep this example in sync with the matching wiki example.

#include <gtest/gtest.h>
#include <openpit/openpit.hpp>
#include <openpit/pretrade/adapters.hpp>

#include <stdexcept>
#include <string_view>

namespace {

struct ApprovalPolicy {
  [[nodiscard]] std::string_view Name() const { return "ApprovalPolicy"; }

  void PerformPreTradeCheck(const openpit::model::Order&,
                            const openpit::pretrade::Context&,
                            openpit::tx::Mutations&, openpit::pretrade::Result&,
                            openpit::pretrade::PolicyDecision& decision) const {
    const openpit::pretrade::Reject entry(
        "ApprovalPolicy", openpit::pretrade::RejectScope::Order,
        openpit::pretrade::RejectCode::ComplianceRestriction,
        "approval required", "obtain a human decision before sending");
    openpit::pretrade::PushReject(decision, entry);
  }
};

TEST(ReviewModeWiki, SendsOnlyAfterConfirmation) {
  using Adapter = openpit::pretrade::PolicyAdapterWithSafeSlowArgType<
      ApprovalPolicy, openpit::model::Order, openpit::ExecutionReport>;

  openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
  const auto policy = openpit::pretrade::CustomPolicy<Adapter>(
      "ApprovalPolicy", Adapter{ApprovalPolicy{}});
  const openpit::PolicyInstance instance{
      "approval", openpit::reject::RuleMode::ReviewWithoutHold};
  builder.Add(policy, instance);
  const openpit::Engine engine = builder.Build();

  openpit::model::Order order;
  auto& operation = order.operation.emplace();
  operation.instrument = openpit::model::Instrument(
      openpit::param::Asset("AAPL"), openpit::param::Asset("USD"));
  operation.accountId = openpit::param::AccountId::FromUint64(99224416);
  operation.side = openpit::model::Side::Buy;
  operation.tradeAmount = openpit::model::TradeAmount::OfQuantity(
      openpit::param::Quantity::FromString("10"));
  operation.price = openpit::param::Price::FromString("25");

  unsigned sent = 0;
  const auto run = [&](const openpit::model::Order& orderToCheck) {
    auto result = engine.ExecutePreTrade(orderToCheck);
    if (!result.Passed()) {
      throw std::runtime_error(result.rejects.front().reason);
    }
    auto& reservation = *result.reservation;
    auto reviews = reservation.Reviews();
    if (!reviews.empty()) {
      reservation.Commit();
      return reviews;
    }

    // Stand-in for a successful venue send.
    ++sent;
    reservation.Commit();
    return reviews;
  };

  const auto reviews = run(order);
  if (reviews.size() != 1 ||
      reviews.front().mode != openpit::pretrade::RuleMode::ReviewWithoutHold) {
    throw std::logic_error("expected one review mark without hold");
  }
  if (sent != 0) {
    throw std::logic_error("expected an unsent order");
  }

  // A human approved the same order.
  order.review.emplace().confirmed = true;
  const auto confirmedReviews = run(order);
  if (!confirmedReviews.empty() || sent != 1) {
    throw std::logic_error(
        "expected the confirmed order to pass and be sent once");
  }
}

}  // namespace
