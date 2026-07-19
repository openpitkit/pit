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

// Table-row translators. Each helper converts one validated row into the
// corresponding public `openpit` value type. `FillReport` keeps a final fill
// together with the pre-trade lock for its matching reservation.

#include "openpit/account_id.hpp"
#include "openpit/accountadjustment/account_adjustment.hpp"
#include "openpit/marketdata.hpp"
#include "openpit/model.hpp"
#include "openpit/param.hpp"
#include "openpit/pretrade/pre_trade_lock.hpp"

#include <stdexcept>
#include <string>

#include "marketdata.hpp"
#include "table.hpp"

namespace spot_table {

// Thrown when a row cannot be translated into a valid engine payload.
class BuildError : public std::runtime_error {
public:
  explicit BuildError(const std::string &message)
      : std::runtime_error(message) {}
};

// Converts a free-form account label to a stable engine-side `AccountId`.
[[nodiscard]] openpit::param::AccountId AccountIdOf(const std::string &s);

// Converts a free-form group label to a stable engine-side `AccountGroupId`.
[[nodiscard]] openpit::param::AccountGroupId
AccountGroupIdOf(const std::string &s);

// Turns `BASE/QUOTE` into an instrument. Throws on malformed text.
[[nodiscard]] openpit::model::Instrument ParseInstrument(const std::string &s);

// Parses a table side into the public side enum.
[[nodiscard]] openpit::model::Side ParseSide(const std::string &s);

// Turns a SEED row into an absolute balance adjustment.
[[nodiscard]] openpit::accountadjustment::AccountAdjustment
BuildSeedAdjustment(const Row &row);

// Turns an ORDER row into an order. Empty price selects a market order.
[[nodiscard]] openpit::model::Order BuildOrder(const Row &row,
                                               openpit::param::AccountId acc);

// A final execution report carrying its pre-trade lock. The owned
// `model::ExecutionReport` holds every field except the lock; the owned
// `PreTradeLock` carries the single default-group entry at the lock price.
class FillReport {
public:
  FillReport(openpit::model::ExecutionReport report,
             openpit::pretrade::PreTradeLock lock)
      : m_report(std::move(report)), m_lock(std::move(lock)) {}

  // The account this report addresses, for async per-account routing.
  [[nodiscard]] openpit::param::AccountId AccountId() const noexcept {
    return m_report.operation->accountId.value();
  }

  [[nodiscard]] const openpit::model::ExecutionReport &Report() const noexcept {
    return m_report;
  }

  [[nodiscard]] const openpit::pretrade::PreTradeLock &Lock() const noexcept {
    return m_lock;
  }

private:
  openpit::model::ExecutionReport m_report;
  openpit::pretrade::PreTradeLock m_lock;
};

// Turns a FILL row into a final `FillReport`. The price column on a FILL is the
// lock price; when empty the most recent quote pushed for the instrument is
// reused. Throws `BuildError` on malformed input or a missing price.
[[nodiscard]] FillReport BuildFillReport(const Row &row,
                                         openpit::param::AccountId acc,
                                         const MarketFeed &feed);

} // namespace spot_table
