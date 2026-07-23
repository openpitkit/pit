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

// wrapper that replays TICK rows against it.
//
// Each execution mode owns one feed over its own service: the runner registers
// every instrument that any TICK row mentions up front, then pushes quotes live
// at each TICK's row position. The feed also remembers the last price pushed
// per instrument so a FILL row may omit its price and reuse the latest quote as
// the lock price.

#include "openpit/marketdata.hpp"
#include "openpit/param/account_id.hpp"

#include <openpit.h>

#include <map>
#include <stdexcept>
#include <string>
#include <utility>
#include <vector>

#include "table.hpp"

namespace spot_table {

// Splits "BASE/QUOTE" into its two parts. Throws `FeedError` when the input is
[[nodiscard]] std::pair<std::string, std::string>
SplitInstrument(const std::string &s);

// Thrown on any market-feed failure (registration, push, or a malformed
class FeedError : public std::runtime_error {
public:
  explicit FeedError(const std::string &message)
      : std::runtime_error(message) {}
};

// Wraps a live `marketdata::Service` and replays TICK rows against it. The
// caller retains ownership of the service and is responsible for closing it
class MarketFeed {
public:
  // Wraps an already-built market-data service handle. Borrowing only: the
  // caller owns the service.
  explicit MarketFeed(openpit::marketdata::Service &service)
      : m_service(&service) {}

  // Registers every instrument named by a TICK row so later live pushes
  // resolve. Throws `FeedError` on a malformed instrument or a registration
  void RegisterInstruments(const std::vector<Row> &rows);

  // Publishes a global mark-price snapshot for `instrument`. Throws `FeedError`
  void Push(const std::string &instrument, const std::string &price);

  // Publishes an addressed mark-price snapshot for `instrument` to each listed
  // account and account group only. At least one target must be supplied.
  void PushFor(const std::string &instrument, const std::string &price,
               const std::vector<openpit::param::AccountId> &accounts,
               const std::vector<openpit::param::AccountGroupId> &groups);

  // The last price string pushed for `instrument`, or "" when none yet. Mirrors
  [[nodiscard]] std::string LatestPrice(const std::string &instrument) const;

private:
  // Resolves the registered id and builds the mark-price quote. Throws
  // `FeedError` when the instrument is unregistered or the price is invalid.
  [[nodiscard]] std::pair<openpit::marketdata::InstrumentId,
                          openpit::marketdata::Quote>
  MakeQuote(const std::string &instrument, const std::string &price) const;

  openpit::marketdata::Service *m_service;
  std::map<std::string, openpit::marketdata::InstrumentId> m_ids;
  std::map<std::string, std::string> m_latest;
};

} // namespace spot_table
