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

#include "marketdata.hpp"

#include "openpit/error.hpp"
#include "openpit/param.hpp"

#include "builder.hpp"

namespace spot_table {

namespace md = openpit::marketdata;

std::pair<std::string, std::string> SplitInstrument(const std::string &s) {
  const std::string::size_type i = s.find('/');
  if (i == std::string::npos || i == 0 || i == s.size() - 1) {
    throw FeedError("instrument \"" + s + "\" must be BASE/QUOTE");
  }
  return {s.substr(0, i), s.substr(i + 1)};
}

void MarketFeed::RegisterInstruments(const std::vector<Row> &rows) {
  for (const Row &row : rows) {
    if (row.action != "TICK") {
      continue;
    }
    if (m_ids.count(row.instrument) != 0) {
      continue;
    }
    const openpit::model::Instrument instrument = [&row] {
      try {
        return ParseInstrument(row.instrument);
      } catch (const std::exception &err) {
        throw FeedError("line " + std::to_string(row.line) + ": " + err.what());
      }
    }();
    const md::RegisterResult registration = m_service->Register(instrument);
    if (registration.status != md::RegisterStatus::Ok ||
        !registration.instrumentId.has_value()) {
      throw FeedError("line " + std::to_string(row.line) + ": register " +
                      row.instrument + ": registration did not succeed");
    }
    m_ids.emplace(row.instrument, registration.instrumentId.value());
  }
}

void MarketFeed::Push(const std::string &instrument, const std::string &price) {
  const auto [id, quote] = MakeQuote(instrument, price);
  const md::RegisterStatus status = m_service->Push(id, quote);
  if (status != md::RegisterStatus::Ok) {
    throw FeedError("push " + instrument + ": publish did not succeed");
  }
  m_latest[instrument] = price;
}

void MarketFeed::PushFor(
    const std::string &instrument, const std::string &price,
    const std::vector<openpit::param::AccountId> &accounts,
    const std::vector<openpit::param::AccountGroupId> &groups) {
  const auto [id, quote] = MakeQuote(instrument, price);
  const md::RegisterStatus status =
      m_service->PushFor(id, quote, accounts, groups);
  if (status != md::RegisterStatus::Ok) {
    throw FeedError("push_for " + instrument + ": publish did not succeed");
  }
  m_latest[instrument] = price;
}

std::string MarketFeed::LatestPrice(const std::string &instrument) const {
  const auto it = m_latest.find(instrument);
  return it == m_latest.end() ? std::string() : it->second;
}

std::pair<md::InstrumentId, md::Quote>
MarketFeed::MakeQuote(const std::string &instrument,
                      const std::string &price) const {
  const auto it = m_ids.find(instrument);
  if (it == m_ids.end()) {
    throw FeedError("instrument " + instrument +
                    " is not registered (every TICK instrument must appear in "
                    "the table)");
  }
  try {
    const openpit::param::Price mark = openpit::param::Price::FromString(price);
    return {it->second, md::Quote().WithMark(mark)};
  } catch (const openpit::Error &err) {
    throw FeedError("price \"" + price + "\": " + err.what());
  }
}

} // namespace spot_table
