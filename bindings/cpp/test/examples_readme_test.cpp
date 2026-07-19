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

// Source: bindings/cpp/README.md - Usage. Keep this code in sync with the
// public README snippet.

#include <openpit/openpit.hpp>

#include <stdexcept>
#include <string>

int main() {
  namespace model = openpit::model;
  namespace policies = openpit::pretrade::policies;

  openpit::EngineBuilder builder(openpit::SyncPolicy::None);
  builder.Add(policies::OrderValidationPolicy{});
  openpit::Engine engine = builder.Build();

  model::Order order = model::Order::Limit(
      model::Instrument(::openpit::param::Asset("AAPL"),
                        ::openpit::param::Asset("USD")),
      openpit::param::AccountId::FromUint64(99224416), model::Side::Buy,
      model::TradeAmount::OfQuantity(
          openpit::param::Quantity::FromString("100")),
      openpit::param::Price::FromString("185"));

  openpit::pretrade::StartResult start = engine.StartPreTrade(order);
  if (!start.Passed()) {
    const std::string reason = start.rejects.empty()
                                   ? "pre-trade start rejected"
                                   : start.rejects.front().reason;
    throw std::runtime_error(reason);
  }

  openpit::pretrade::ExecuteResult execute = start.request->Execute();
  if (!execute.Passed()) {
    const std::string reason = execute.rejects.empty()
                                   ? "pre-trade execute rejected"
                                   : execute.rejects.front().reason;
    throw std::runtime_error(reason);
  }

  execute.reservation->Commit();
  return 0;
}
