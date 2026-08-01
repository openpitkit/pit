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

// Source: bindings/cpp/README.md - Quick Start. Keep this code in sync
// with the public README snippet.

#include <openpit/openpit.hpp>

#include <iostream>

int main() {
  namespace model = openpit::model;
  namespace param = openpit::param;
  namespace policies = openpit::pretrade::policies;

  // Build the engine once, at platform initialization.
  openpit::EngineBuilder builder(openpit::SyncPolicy::None);
  builder.Add(policies::OrderValidationPolicy{});
  openpit::Engine engine = builder.Build();

  // Describe the order: buy 100 AAPL at 185 USD.
  const model::Order order = model::Order::Limit(
      model::Instrument(param::Asset("AAPL"), param::Asset("USD")),
      param::AccountId::FromUint64(99224416), model::Side::Buy,
      model::TradeAmount::OfQuantity(param::Quantity::FromString("100")),
      param::Price::FromString("185"));

  // Run the pre-trade pipeline and read the verdict.
  openpit::pretrade::ExecuteResult result = engine.ExecutePreTrade(order);
  if (!result.Passed()) {
    for (const openpit::pretrade::Reject& rejection : result.rejects) {
      std::cout << "rejected by " << rejection.policy << ": "
                << rejection.reason << '\n';
    }
    return 1;
  }

  // The venue accepted the order, so the reserved state stays. Destroying an
  // unresolved reservation rolls it back instead.
  result.reservation->Commit();
  return 0;
}
