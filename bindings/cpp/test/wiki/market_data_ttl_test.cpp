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

// Source: Market-Data-TTL.md
//
// Compiling mirror of the C++ snippets published on the Market-Data-TTL wiki
// page. Each TEST runs the same user code shown in the corresponding C++
// subsection, wrapped only in the minimal harness the snippet elides for
// readability. The published snippet body and the test body must stay in
// lock-step.

#include "openpit/account_id.hpp"
#include "openpit/marketdata.hpp"
#include "openpit/model.hpp"
#include "openpit/param.hpp"

#include <gtest/gtest.h>

#include <cassert>
#include <chrono>
#include <optional>
#include <thread>

namespace {

namespace md = openpit::marketdata;

using openpit::param::AccountGroupId;
using openpit::param::AccountId;
using openpit::param::Price;

// AccountInfo for a reading account that belongs to no group.
struct NoGroupInfo {
  [[nodiscard]] std::optional<AccountGroupId> AccountGroup() const {
    return std::nullopt;
  }
};

//------------------------------------------------------------------------------
// Quote Freshness
// Mirrors the "Quote Freshness" C++ snippet from Market-Data-TTL.md.

TEST(MarketDataTtlWiki, QuoteFreshnessFiniteTtlExpiresAndFreshPushRestores) {
  // A 50 ms service-wide lifetime: quotes older than that read as absent.
  md::Service service =
      md::Builder(md::QuoteTtl::Within(std::chrono::milliseconds(50)),
                  md::SyncPolicy::None)
          .Build();

  const md::RegisterResult registration =
      service.Register(openpit::model::Instrument(
          ::openpit::param::Asset("AAPL"), ::openpit::param::Asset("USD")));
  ASSERT_EQ(registration.status, md::RegisterStatus::Ok);
  ASSERT_TRUE(registration.instrumentId.has_value());
  const md::InstrumentId aaplId = registration.instrumentId.value();

  const AccountId accountId = AccountId::FromUint64(1);

  auto read = [&]() -> std::optional<md::Quote> {
    return service.Find(aaplId, accountId, NoGroupInfo{},
                        md::QuoteResolution::AccountThenGroupThenDefault);
  };

  ASSERT_EQ(
      service.Push(aaplId, md::Quote().WithMark(Price::FromString("200"))),
      md::RegisterStatus::Ok);
  ASSERT_TRUE(read().has_value());

  // After the lifetime elapses the quote reads as absent.
  std::this_thread::sleep_for(std::chrono::milliseconds(80));
  EXPECT_FALSE(read().has_value());

  // A fresh push restores visibility.
  ASSERT_EQ(
      service.Push(aaplId, md::Quote().WithMark(Price::FromString("205"))),
      md::RegisterStatus::Ok);
  const std::optional<md::Quote> quote = read();
  ASSERT_TRUE(quote.has_value());
  EXPECT_EQ(quote->Mark()->ToString(), "205");
}

}  // namespace
