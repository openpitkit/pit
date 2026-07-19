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

// Source: Market-Data.md
//
// Compiling mirror of the C++ snippets published on the Market-Data wiki page.
// Each TEST runs the same user code shown in the corresponding C++ subsection,
// wrapped only in the minimal market-data harness (service build, register,
// asserts) the snippet elides for readability. The published snippet body and
// the test body must stay in lock-step.

#include "openpit/account_id.hpp"
#include "openpit/engine.hpp"
#include "openpit/marketdata.hpp"
#include "openpit/model.hpp"
#include "openpit/param.hpp"

#include <gtest/gtest.h>

#include <optional>
#include <vector>

namespace {

namespace md = openpit::marketdata;

using openpit::param::AccountGroupId;
using openpit::param::AccountId;
using openpit::param::Price;

// A reading account that belongs to no group: resolution falls through to the
// default bucket. `Service::Get` accepts any object exposing
// `std::optional<AccountGroupId> AccountGroup() const`; in policy code this is
// usually the pre-trade context. The mirror uses this no-group stub.
struct NoGroupInfo {
  [[nodiscard]] std::optional<AccountGroupId> AccountGroup() const {
    return std::nullopt;
  }
};

//------------------------------------------------------------------------------
// Pushing and Reading Quotes

TEST(MarketDataWiki, PushReadAndResolve) {
  // The engine builder fixes the sync mode; the market-data service derives it
  // via the engine-builder path. Here a no-sync engine yields a no-sync,
  // single-threaded service whose locks are a free no-op.
  md::Service service = md::Builder::FromEngineSyncPolicy(
                            md::QuoteTtl::Infinite(), openpit::SyncPolicy::None)
                            .Build();

  const openpit::model::Instrument aapl(::openpit::param::Asset("AAPL"),
                                        ::openpit::param::Asset("USD"));
  const md::RegisterResult registration = service.Register(aapl);
  ASSERT_EQ(registration.status, md::RegisterStatus::Ok);
  ASSERT_TRUE(registration.instrumentId.has_value());
  const md::InstrumentId aaplId = registration.instrumentId.value();

  // Publish a full snapshot into the default ("everyone-else") bucket.
  const Price mark = Price::FromString("150");
  const Price bid = Price::FromString("149.5");
  const Price ask = Price::FromString("150.5");
  ASSERT_EQ(service.Push(aaplId,
                         md::Quote().WithMark(mark).WithBid(bid).WithAsk(ask)),
            md::RegisterStatus::Ok);

  // Read for an account with no group: the lookup falls through to the default
  // bucket. Pass any AccountInfo; in policy code this is usually the pre-trade
  // context. The mirror uses a no-group stub.
  const AccountId accountId = AccountId::FromUint64(1);
  const std::optional<md::Quote> quote =
      service.Find(aaplId, accountId, NoGroupInfo{},
                   md::QuoteResolution::AccountThenGroupThenDefault);
  ASSERT_TRUE(quote.has_value());
  EXPECT_EQ(quote->Mark(), mark);
  EXPECT_EQ(quote->Bid(), bid);

  // Resolve recovers the id from the instrument name.
  EXPECT_EQ(service.Resolve(aapl), aaplId);
}

//------------------------------------------------------------------------------
// Targeted Fan-Out: push for

TEST(MarketDataWiki, PushForFansOutToAccountsAndGroup) {
  md::Service service = md::Builder::FromEngineSyncPolicy(
                            md::QuoteTtl::Infinite(), openpit::SyncPolicy::None)
                            .Build();
  const md::RegisterResult registration =
      service.Register(openpit::model::Instrument(
          ::openpit::param::Asset("AAPL"), ::openpit::param::Asset("USD")));
  ASSERT_EQ(registration.status, md::RegisterStatus::Ok);
  ASSERT_TRUE(registration.instrumentId.has_value());
  const md::InstrumentId aaplId = registration.instrumentId.value();

  const Price mark = Price::FromString("150");
  const AccountGroupId groupId = AccountGroupId::FromUint32(7);

  // Fan out to two accounts and one group simultaneously.
  ASSERT_EQ(
      service.PushFor(aaplId, md::Quote().WithMark(mark),
                      {AccountId::FromUint64(10), AccountId::FromUint64(11)},
                      {groupId}),
      md::RegisterStatus::Ok);

  // Read back for account 10 under AccountOnly - hits the per-account bucket.
  const std::optional<md::Quote> quote =
      service.Find(aaplId, AccountId::FromUint64(10), NoGroupInfo{},
                   md::QuoteResolution::AccountOnly);
  ASSERT_TRUE(quote.has_value());
  EXPECT_EQ(quote->Mark(), mark);
}

//------------------------------------------------------------------------------
// Replace Versus Patch

TEST(MarketDataWiki, PushPatchPreservesUnsetFields) {
  md::Service service = md::Builder::FromEngineSyncPolicy(
                            md::QuoteTtl::Infinite(), openpit::SyncPolicy::None)
                            .Build();
  const md::RegisterResult registration =
      service.Register(openpit::model::Instrument(
          ::openpit::param::Asset("AAPL"), ::openpit::param::Asset("USD")));
  ASSERT_EQ(registration.status, md::RegisterStatus::Ok);
  ASSERT_TRUE(registration.instrumentId.has_value());
  const md::InstrumentId aaplId = registration.instrumentId.value();

  const Price bid = Price::FromString("99");
  const Price ask = Price::FromString("101");
  ASSERT_EQ(service.Push(aaplId, md::Quote()
                                     .WithMark(Price::FromString("100"))
                                     .WithBid(bid)
                                     .WithAsk(ask)),
            md::RegisterStatus::Ok);

  // Patch only the mark; bid and ask are preserved.
  const Price newMark = Price::FromString("105");
  ASSERT_EQ(service.PushPatch(aaplId, md::Quote().WithMark(newMark)),
            md::RegisterStatus::Ok);

  const AccountId accountId = AccountId::FromUint64(1);
  const std::optional<md::Quote> quote =
      service.Find(aaplId, accountId, NoGroupInfo{},
                   md::QuoteResolution::AccountThenGroupThenDefault);
  ASSERT_TRUE(quote.has_value());
  EXPECT_EQ(quote->Mark(), newMark);
  EXPECT_EQ(quote->Bid(), bid);
  EXPECT_EQ(quote->Ask(), ask);
}

//------------------------------------------------------------------------------
// Clearing a Quote

TEST(MarketDataWiki, ClearHidesQuoteThenPushRestores) {
  md::Service service = md::Builder::FromEngineSyncPolicy(
                            md::QuoteTtl::Infinite(), openpit::SyncPolicy::None)
                            .Build();
  const md::RegisterResult registration =
      service.Register(openpit::model::Instrument(
          ::openpit::param::Asset("AAPL"), ::openpit::param::Asset("USD")));
  ASSERT_EQ(registration.status, md::RegisterStatus::Ok);
  ASSERT_TRUE(registration.instrumentId.has_value());
  const md::InstrumentId aaplId = registration.instrumentId.value();

  const AccountId accountId = AccountId::FromUint64(1);
  ASSERT_EQ(
      service.Push(aaplId, md::Quote().WithMark(Price::FromString("200"))),
      md::RegisterStatus::Ok);

  // Clear hides the quote but keeps the instrument registered.
  service.Clear(aaplId);
  EXPECT_FALSE(service
                   .Find(aaplId, accountId, NoGroupInfo{},
                         md::QuoteResolution::AccountThenGroupThenDefault)
                   .has_value());

  // Pushing again restores a quote for the same id.
  ASSERT_EQ(
      service.Push(aaplId, md::Quote().WithMark(Price::FromString("210"))),
      md::RegisterStatus::Ok);
  EXPECT_TRUE(service
                  .Find(aaplId, accountId, NoGroupInfo{},
                        md::QuoteResolution::AccountThenGroupThenDefault)
                  .has_value());
}

}  // namespace
