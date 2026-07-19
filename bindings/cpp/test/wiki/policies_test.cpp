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

// Source: Policies.md
//
// Compiling mirror of the C++ snippets published on the Policies wiki page.
// Each TEST runs the exact user code shown in a
// `<details><summary>C++</summary>` block (modulo the minimal harness: the
// engine builder is named, and a build assertion stands in for the prose). When
// a snippet here changes, update the matching block in Policies.md and vice
// versa.

#include "openpit/openpit.hpp"
#include "openpit/pretrade/pretrade.hpp"

#include <gtest/gtest.h>

#include <utility>

namespace {

namespace policies = openpit::pretrade::policies;

// SpotFundsPolicy: limit-only spot funds, registered first in the policy list.
TEST(Policies, SpotFundsLimitOnly) {
  openpit::EngineBuilder builder(openpit::SyncPolicy::None);
  // Limit-only mode: no market-data service handle.
  builder.Add(policies::SpotFundsPolicy{});
  const openpit::Engine engine = builder.Build();

  EXPECT_TRUE(static_cast<bool>(engine));
}

// OrderValidationPolicy: validates basic order structure, needs no barriers.
TEST(Policies, OrderValidation) {
  openpit::EngineBuilder builder(openpit::SyncPolicy::None);
  builder.Add(policies::OrderValidationPolicy{});
  const openpit::Engine engine = builder.Build();

  EXPECT_TRUE(static_cast<bool>(engine));
}

// RateLimitPolicy: 100 orders per second on the broker axis.
TEST(Policies, RateLimitBrokerBarrier) {
  openpit::EngineBuilder builder(openpit::SyncPolicy::None);
  builder.Add(policies::RateLimitPolicy{}.BrokerBarrier(
      policies::RateLimitBrokerBarrier(policies::RateLimit(
          /*maxOrders=*/100, /*windowNanoseconds=*/1'000'000'000))));
  const openpit::Engine engine = builder.Build();

  EXPECT_TRUE(static_cast<bool>(engine));
}

// OrderSizeLimitPolicy: per-asset barrier plus an additive broker hard cap.
TEST(Policies, OrderSizeLimitAssetAndBroker) {
  const openpit::param::Quantity maxQty =
      openpit::param::Quantity::FromString("100");
  const openpit::param::Volume maxNotional =
      openpit::param::Volume::FromString("50000");

  openpit::EngineBuilder builder(openpit::SyncPolicy::None);
  builder.Add(policies::OrderSizeLimitPolicy{}
                  .AssetBarrier(policies::OrderSizeAssetBarrier(
                      policies::OrderSizeLimit(maxQty, maxNotional),
                      openpit::param::Asset("USD")))
                  .BrokerBarrier(policies::OrderSizeBrokerBarrier(
                      policies::OrderSizeLimit(maxQty, maxNotional))));
  const openpit::Engine engine = builder.Build();

  EXPECT_TRUE(static_cast<bool>(engine));
}

// PnlBoundsKillSwitchPolicy: broker-level P&L bounds for one settlement asset.
TEST(Policies, PnlBoundsKillSwitchBrokerBarrier) {
  policies::PnlBoundsBrokerBarrier barrier(openpit::param::Asset("USD"));
  barrier.lowerBound = openpit::param::Pnl::FromString("-1000");
  barrier.upperBound = openpit::param::Pnl::FromString("500");

  openpit::EngineBuilder builder(openpit::SyncPolicy::None);
  builder.Add(
      policies::PnlBoundsKillSwitchPolicy{}.BrokerBarrier(std::move(barrier)));
  const openpit::Engine engine = builder.Build();

  EXPECT_TRUE(static_cast<bool>(engine));
}

}  // namespace
