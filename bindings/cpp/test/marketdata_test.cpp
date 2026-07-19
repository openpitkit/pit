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

#include "openpit/marketdata.hpp"

#include "openpit/account_id.hpp"
#include "openpit/engine.hpp"
#include "openpit/error.hpp"
#include "openpit/model.hpp"
#include "openpit/param.hpp"
#include "openpit/reference_book.hpp"

#include <gtest/gtest.h>
#include <openpit.h>

#include <chrono>
#include <optional>
#include <thread>
#include <type_traits>
#include <vector>

namespace {

namespace md = openpit::marketdata;

using openpit::param::AccountGroupId;
using openpit::param::AccountId;
using openpit::param::DefaultAccountGroup;
using openpit::param::Price;

//------------------------------------------------------------------------------
// AccountInfo stubs

// A reading account that belongs to no group; resolution falls through to the
// default bucket.
struct NoGroupInfo {
  [[nodiscard]] std::optional<AccountGroupId> AccountGroup() const {
    return std::nullopt;
  }
};

// A reading account pinned to a fixed group.
struct FixedGroupInfo {
  AccountGroupId group;
  [[nodiscard]] std::optional<AccountGroupId> AccountGroup() const {
    return group;
  }
};

//------------------------------------------------------------------------------
// Helpers

[[nodiscard]] md::Service BuildService(md::SyncPolicy policy) {
  return md::Builder(md::QuoteTtl::Infinite(), policy).Build();
}

[[nodiscard]] md::InstrumentId Register(
    md::Service& service, const openpit::model::Instrument& instrument) {
  const md::RegisterResult result = service.Register(instrument);
  EXPECT_EQ(result.status, md::RegisterStatus::Ok);
  EXPECT_TRUE(result.instrumentId.has_value());
  return result.instrumentId.value();
}

//------------------------------------------------------------------------------
// InstrumentId

TEST(MarketDataInstrumentId, FromUint64RoundTrips) {
  const md::InstrumentId id = md::InstrumentId::FromUint64(42);
  EXPECT_EQ(id.Raw(), 42u);
  EXPECT_EQ(id.ToString(), "42");
  EXPECT_EQ(id, md::InstrumentId::FromUint64(42));
  EXPECT_NE(id, md::InstrumentId::FromUint64(43));
}

TEST(CoreInstrumentId, MarketDataAliasIsTheRootType) {
  static_assert(std::is_same_v<openpit::InstrumentId, md::InstrumentId>);

  const openpit::InstrumentId root = openpit::InstrumentId::FromUint64(42);
  const md::InstrumentId marketData = root;
  EXPECT_EQ(marketData.Raw(), 42u);
}

//------------------------------------------------------------------------------
// ReferenceBook

TEST(ReferenceBook, ResolvesInstrumentAndStoresSettlementScheme) {
  openpit::ReferenceBook book;
  const openpit::model::Instrument instrument("AAPL", "USD");
  const openpit::InstrumentId id = openpit::InstrumentId::FromUint64(42);

  const openpit::ReferenceBookRegisterResult registration =
      book.Register(instrument, id);
  ASSERT_TRUE(registration.Ok());
  ASSERT_TRUE(registration.instrumentId.has_value());
  EXPECT_EQ(*registration.instrumentId, id);
  EXPECT_EQ(book.Resolve(instrument), id);

  const auto absent = book.SettlementSchemeFor(id);
  EXPECT_EQ(absent.status, openpit::ReferenceBookStatus::Ok);
  EXPECT_FALSE(absent.settlementScheme.has_value());

  const openpit::SettlementScheme scheme{
      openpit::SettlementLag{2, openpit::SettlementUnit::BusinessDays},
      openpit::SettlementLag{1, openpit::SettlementUnit::CalendarDays},
  };
  EXPECT_EQ(book.SetSettlementScheme(id, scheme),
            openpit::ReferenceBookStatus::Ok);
  const auto configured = book.SettlementSchemeFor(id);
  EXPECT_EQ(configured.status, openpit::ReferenceBookStatus::Ok);
  ASSERT_TRUE(configured.settlementScheme.has_value());
  EXPECT_EQ(*configured.settlementScheme, scheme);

  const auto unknown =
      book.SettlementSchemeFor(openpit::InstrumentId::FromUint64(99));
  EXPECT_EQ(unknown.status, openpit::ReferenceBookStatus::UnknownInstrument);
  EXPECT_FALSE(unknown.settlementScheme.has_value());
}

TEST(ReferenceBook, ReportsDuplicateAndUnknownInputs) {
  openpit::ReferenceBook book;
  const openpit::model::Instrument aapl("AAPL", "USD");
  const openpit::model::Instrument msft("MSFT", "USD");
  const openpit::InstrumentId id = openpit::InstrumentId::FromUint64(42);

  EXPECT_TRUE(book.Register(aapl, id).Ok());
  const auto duplicateId = book.Register(msft, id);
  EXPECT_EQ(duplicateId.status,
            openpit::ReferenceBookRegisterStatus::DuplicateId);
  ASSERT_TRUE(duplicateId.conflictingInstrumentId.has_value());
  EXPECT_EQ(*duplicateId.conflictingInstrumentId, id);

  const auto duplicateInstrument =
      book.Register(aapl, openpit::InstrumentId::FromUint64(43));
  EXPECT_EQ(duplicateInstrument.status,
            openpit::ReferenceBookRegisterStatus::DuplicateInstrument);
  ASSERT_TRUE(duplicateInstrument.conflictingInstrument.has_value());
  EXPECT_EQ(duplicateInstrument.conflictingInstrument->underlyingAsset, "AAPL");
  EXPECT_EQ(book.SetSettlementScheme(openpit::InstrumentId::FromUint64(99),
                                     openpit::SettlementScheme::Uniform(1)),
            openpit::ReferenceBookStatus::UnknownInstrument);
}

TEST(ReferenceBook, UniformSettlementUsesBusinessDays) {
  const openpit::SettlementScheme scheme =
      openpit::SettlementScheme::Uniform(2);
  EXPECT_EQ(scheme.delivery,
            (openpit::SettlementLag{2, openpit::SettlementUnit::BusinessDays}));
  EXPECT_EQ(scheme.payment,
            (openpit::SettlementLag{2, openpit::SettlementUnit::BusinessDays}));
  EXPECT_EQ(openpit::SettlementLag{}.unit,
            openpit::SettlementUnit::BusinessDays);
}

//------------------------------------------------------------------------------
// Quote

TEST(MarketDataQuote, EmptyQuoteHasNoFields) {
  const md::Quote quote;
  EXPECT_FALSE(quote.Mark().has_value());
  EXPECT_FALSE(quote.Bid().has_value());
  EXPECT_FALSE(quote.Ask().has_value());
}

TEST(MarketDataQuote, WithSettersCarryExactDecimalPrices) {
  const md::Quote quote = md::Quote()
                              .WithMark(Price::FromString("150.25"))
                              .WithBid(Price::FromString("149.5"))
                              .WithAsk(Price::FromString("150.75"));

  ASSERT_TRUE(quote.Mark().has_value());
  ASSERT_TRUE(quote.Bid().has_value());
  ASSERT_TRUE(quote.Ask().has_value());
  EXPECT_EQ(quote.Mark()->ToString(), "150.25");
  EXPECT_EQ(quote.Bid()->ToString(), "149.5");
  EXPECT_EQ(quote.Ask()->ToString(), "150.75");
}

TEST(MarketDataQuote, WithSettersAreImmutableCopies) {
  const md::Quote base = md::Quote().WithMark(Price::FromString("100"));
  const md::Quote patched = base.WithBid(Price::FromString("99"));

  // `base` is unchanged by deriving `patched`.
  EXPECT_FALSE(base.Bid().has_value());
  ASSERT_TRUE(patched.Bid().has_value());
  EXPECT_EQ(patched.Bid()->ToString(), "99");
  EXPECT_EQ(patched.Mark()->ToString(), "100");
}

TEST(MarketDataQuote, RawRoundTripPreservesSetAndUnsetFields) {
  const md::Quote quote = md::Quote().WithMark(Price::FromString("12.34"));
  const md::Quote restored = md::Quote::FromRaw(quote.Raw());
  ASSERT_TRUE(restored.Mark().has_value());
  EXPECT_EQ(restored.Mark()->ToString(), "12.34");
  EXPECT_FALSE(restored.Bid().has_value());
}

//------------------------------------------------------------------------------
// QuoteTtl

TEST(MarketDataQuoteTtl, InfiniteIsInfinite) {
  const OpenPitMarketDataQuoteTtl raw = md::QuoteTtl::Infinite().Raw();
  EXPECT_TRUE(raw.is_infinite);
}

TEST(MarketDataQuoteTtl, WithinCarriesSecondsAndNanos) {
  const OpenPitMarketDataQuoteTtl raw = md::QuoteTtl::Within(3, 500).Raw();
  EXPECT_FALSE(raw.is_infinite);
  EXPECT_EQ(raw.secs, 3u);
  EXPECT_EQ(raw.nanos, 500u);
}

TEST(MarketDataQuoteTtl, WithinFromChronoSplitsSecondsAndNanos) {
  const OpenPitMarketDataQuoteTtl raw =
      md::QuoteTtl::Within(std::chrono::milliseconds(1500)).Raw();
  EXPECT_FALSE(raw.is_infinite);
  EXPECT_EQ(raw.secs, 1u);
  EXPECT_EQ(raw.nanos, 500'000'000u);
}

TEST(MarketDataQuoteTtl, WithinClampsNegativeToZero) {
  const OpenPitMarketDataQuoteTtl raw =
      md::QuoteTtl::Within(std::chrono::seconds(-5)).Raw();
  EXPECT_FALSE(raw.is_infinite);
  EXPECT_EQ(raw.secs, 0u);
  EXPECT_EQ(raw.nanos, 0u);
}

//------------------------------------------------------------------------------
// Builder / sync model

TEST(MarketDataBuilder, BuildsForBothSyncModes) {
  EXPECT_NO_THROW(
      { md::Service service = BuildService(md::SyncPolicy::None); });
  EXPECT_NO_THROW(
      { md::Service service = BuildService(md::SyncPolicy::Full); });
}

TEST(MarketDataBuilder, FullSyncUpgradeBuildsConcurrentSafeService) {
  // A no-sync builder upgraded to Full yields a usable, synchronized service.
  md::Service service =
      md::Builder(md::QuoteTtl::Infinite(), md::SyncPolicy::None)
          .FullSync()
          .Build();
  EXPECT_TRUE(static_cast<bool>(service));

  const md::InstrumentId id =
      Register(service, openpit::model::Instrument("AAPL", "USD"));
  EXPECT_EQ(service.Push(id, md::Quote().WithMark(Price::FromString("10"))),
            md::RegisterStatus::Ok);
}

TEST(MarketDataBuilder, FromEngineSyncPolicyDerivesMode) {
  // A None engine yields a no-sync, upgradable builder; a Full/Account engine
  // yields a full-sync builder. All three build a usable service.
  EXPECT_NO_THROW({
    md::Service s = md::Builder::FromEngineSyncPolicy(md::QuoteTtl::Infinite(),
                                                      openpit::SyncPolicy::None)
                        .Build();
  });
  EXPECT_NO_THROW({
    md::Service s = md::Builder::FromEngineSyncPolicy(md::QuoteTtl::Infinite(),
                                                      openpit::SyncPolicy::Full)
                        .Build();
  });
  EXPECT_NO_THROW({
    md::Service s = md::Builder::FromEngineSyncPolicy(
                        md::QuoteTtl::Infinite(), openpit::SyncPolicy::Account)
                        .Build();
  });

  // A None-engine builder can still be upgraded to Full before building.
  EXPECT_NO_THROW({
    md::Service s = md::Builder::FromEngineSyncPolicy(md::QuoteTtl::Infinite(),
                                                      openpit::SyncPolicy::None)
                        .FullSync()
                        .Build();
  });
}

TEST(MarketDataBuilder, NoSyncServicePushAndReadOnSameThread) {
  // A no-sync service (free no-op locks) supports push and read on one thread.
  md::Service service = BuildService(md::SyncPolicy::None);
  const md::InstrumentId id =
      Register(service, openpit::model::Instrument("BTC", "USD"));

  ASSERT_EQ(service.Push(id, md::Quote().WithMark(Price::FromString("42000"))),
            md::RegisterStatus::Ok);

  const std::optional<md::Quote> quote =
      service.Find(id, AccountId::FromUint64(1), NoGroupInfo{},
                   md::QuoteResolution::AccountThenGroupThenDefault);
  ASSERT_TRUE(quote.has_value());
  EXPECT_EQ(quote->Mark()->ToString(), "42000");
}

//------------------------------------------------------------------------------
// Register / Resolve

TEST(MarketDataService, RegisterTwiceReportsAlreadyRegistered) {
  md::Service service = BuildService(md::SyncPolicy::None);
  const openpit::model::Instrument instrument("AAPL", "USD");

  const md::RegisterResult first = service.Register(instrument);
  ASSERT_EQ(first.status, md::RegisterStatus::Ok);
  ASSERT_TRUE(first.instrumentId.has_value());

  const md::RegisterResult second = service.Register(instrument);
  EXPECT_EQ(second.status, md::RegisterStatus::AlreadyRegistered);
  EXPECT_FALSE(second.instrumentId.has_value());
  ASSERT_TRUE(second.conflictingInstrument.has_value());
  EXPECT_EQ(second.conflictingInstrument->underlyingAsset, "AAPL");
  EXPECT_EQ(second.conflictingInstrument->settlementAsset, "USD");
}

TEST(MarketDataService, RegisterWithExplicitIdRejectsDuplicateId) {
  md::Service service = BuildService(md::SyncPolicy::None);
  const md::InstrumentId id = md::InstrumentId::FromUint64(7);

  const md::RegisterResult out =
      service.Register(openpit::model::Instrument("AAPL", "USD"), id);
  ASSERT_EQ(out.status, md::RegisterStatus::Ok);
  ASSERT_TRUE(out.instrumentId.has_value());
  EXPECT_EQ(out.instrumentId.value(), id);

  const md::RegisterResult dup =
      service.Register(openpit::model::Instrument("MSFT", "USD"), id);
  EXPECT_EQ(dup.status, md::RegisterStatus::DuplicateId);
  EXPECT_FALSE(dup.instrumentId.has_value());
  ASSERT_TRUE(dup.conflictingInstrumentId.has_value());
  EXPECT_EQ(dup.conflictingInstrumentId.value(), id);
}

TEST(MarketDataService, RegisterExplicitIdRejectsDuplicateInstrument) {
  md::Service service = BuildService(md::SyncPolicy::None);
  const openpit::model::Instrument instrument("AAPL", "USD");

  const md::RegisterResult out =
      service.Register(instrument, md::InstrumentId::FromUint64(1));
  ASSERT_EQ(out.status, md::RegisterStatus::Ok);
  ASSERT_TRUE(out.instrumentId.has_value());

  const md::RegisterResult dup =
      service.Register(instrument, md::InstrumentId::FromUint64(2));
  EXPECT_EQ(dup.status, md::RegisterStatus::DuplicateInstrument);
  EXPECT_FALSE(dup.instrumentId.has_value());
  ASSERT_TRUE(dup.conflictingInstrument.has_value());
  EXPECT_EQ(dup.conflictingInstrument->underlyingAsset, "AAPL");
  EXPECT_EQ(dup.conflictingInstrument->settlementAsset, "USD");
}

TEST(MarketDataService, ResolveRecoversIdFromName) {
  md::Service service = BuildService(md::SyncPolicy::None);
  const openpit::model::Instrument instrument("AAPL", "USD");
  const md::InstrumentId id = Register(service, instrument);

  const std::optional<md::InstrumentId> resolved = service.Resolve(instrument);
  ASSERT_TRUE(resolved.has_value());
  EXPECT_EQ(*resolved, id);
}

TEST(MarketDataService, ResolveUnknownInstrumentIsNullopt) {
  md::Service service = BuildService(md::SyncPolicy::None);
  EXPECT_FALSE(
      service.Resolve(openpit::model::Instrument("NOPE", "USD")).has_value());
}

TEST(MarketDataService, PushByInstrumentCreatesSlotAndReturnsId) {
  md::Service service = BuildService(md::SyncPolicy::None);
  const openpit::model::Instrument instrument("ETH", "USD");

  const md::InstrumentId id = service.PushByInstrument(
      instrument, md::Quote().WithMark(Price::FromString("2000")));

  const std::optional<md::InstrumentId> resolved = service.Resolve(instrument);
  ASSERT_TRUE(resolved.has_value());
  EXPECT_EQ(*resolved, id);
}

//------------------------------------------------------------------------------
// Push / Get round-trips with exact decimals

TEST(MarketDataService, PushReplaceRoundTripsExactPrices) {
  md::Service service = BuildService(md::SyncPolicy::None);
  const md::InstrumentId id =
      Register(service, openpit::model::Instrument("AAPL", "USD"));

  ASSERT_EQ(service.Push(id, md::Quote()
                                 .WithMark(Price::FromString("150"))
                                 .WithBid(Price::FromString("149.5"))
                                 .WithAsk(Price::FromString("150.5"))),
            md::RegisterStatus::Ok);

  const std::optional<md::Quote> quote =
      service.Find(id, AccountId::FromUint64(1), NoGroupInfo{},
                   md::QuoteResolution::AccountThenGroupThenDefault);
  ASSERT_TRUE(quote.has_value());
  EXPECT_EQ(quote->Mark()->ToString(), "150");
  EXPECT_EQ(quote->Bid()->ToString(), "149.5");
  EXPECT_EQ(quote->Ask()->ToString(), "150.5");
}

TEST(MarketDataService, PushPatchPreservesUnsetFields) {
  md::Service service = BuildService(md::SyncPolicy::None);
  const md::InstrumentId id =
      Register(service, openpit::model::Instrument("AAPL", "USD"));

  ASSERT_EQ(service.Push(id, md::Quote()
                                 .WithMark(Price::FromString("100"))
                                 .WithBid(Price::FromString("99"))
                                 .WithAsk(Price::FromString("101"))),
            md::RegisterStatus::Ok);

  // Patch only the mark; bid and ask are preserved.
  ASSERT_EQ(
      service.PushPatch(id, md::Quote().WithMark(Price::FromString("105"))),
      md::RegisterStatus::Ok);

  const std::optional<md::Quote> quote =
      service.Find(id, AccountId::FromUint64(1), NoGroupInfo{},
                   md::QuoteResolution::AccountThenGroupThenDefault);
  ASSERT_TRUE(quote.has_value());
  EXPECT_EQ(quote->Mark()->ToString(), "105");
  EXPECT_EQ(quote->Bid()->ToString(), "99");
  EXPECT_EQ(quote->Ask()->ToString(), "101");
}

TEST(MarketDataService, PushUnknownInstrumentReportsUnknown) {
  md::Service service = BuildService(md::SyncPolicy::None);
  EXPECT_EQ(service.Push(md::InstrumentId::FromUint64(999),
                         md::Quote().WithMark(Price::FromString("1"))),
            md::RegisterStatus::UnknownInstrument);
}

//------------------------------------------------------------------------------
// Get status outcomes

TEST(MarketDataService, GetUnknownInstrumentIsNullopt) {
  md::Service service = BuildService(md::SyncPolicy::None);
  EXPECT_FALSE(service
                   .Find(md::InstrumentId::FromUint64(123),
                         AccountId::FromUint64(1), NoGroupInfo{},
                         md::QuoteResolution::AccountThenGroupThenDefault)
                   .has_value());
}

TEST(MarketDataService, GetStatusDistinguishesUnknownFromUnavailable) {
  md::Service service = BuildService(md::SyncPolicy::None);

  // Unknown: never registered.
  EXPECT_EQ(
      service
          .Get(md::InstrumentId::FromUint64(5), AccountId::FromUint64(1),
               NoGroupInfo{}, md::QuoteResolution::AccountThenGroupThenDefault)
          .status,
      md::GetStatus::UnknownInstrument);

  // Unavailable: registered, but no quote was ever pushed.
  const md::InstrumentId id =
      Register(service, openpit::model::Instrument("AAPL", "USD"));
  EXPECT_EQ(service
                .Get(id, AccountId::FromUint64(1), NoGroupInfo{},
                     md::QuoteResolution::AccountThenGroupThenDefault)
                .status,
            md::GetStatus::Unavailable);
}

TEST(MarketDataService, ClearHidesQuoteButKeepsRegistration) {
  md::Service service = BuildService(md::SyncPolicy::None);
  const md::InstrumentId id =
      Register(service, openpit::model::Instrument("AAPL", "USD"));
  const AccountId account = AccountId::FromUint64(1);

  ASSERT_EQ(service.Push(id, md::Quote().WithMark(Price::FromString("200"))),
            md::RegisterStatus::Ok);
  ASSERT_TRUE(service
                  .Find(id, account, NoGroupInfo{},
                        md::QuoteResolution::AccountThenGroupThenDefault)
                  .has_value());

  service.Clear(id);
  EXPECT_FALSE(service
                   .Find(id, account, NoGroupInfo{},
                         md::QuoteResolution::AccountThenGroupThenDefault)
                   .has_value());

  // Pushing again restores visibility for the same id.
  ASSERT_EQ(service.Push(id, md::Quote().WithMark(Price::FromString("210"))),
            md::RegisterStatus::Ok);
  const std::optional<md::Quote> recovered =
      service.Find(id, account, NoGroupInfo{},
                   md::QuoteResolution::AccountThenGroupThenDefault);
  ASSERT_TRUE(recovered.has_value());
  EXPECT_EQ(recovered->Mark()->ToString(), "210");
}

//------------------------------------------------------------------------------
// Targeted fan-out and resolution fallback

TEST(MarketDataService, PushForEmptyTargetsReportsNoTarget) {
  md::Service service = BuildService(md::SyncPolicy::None);
  const md::InstrumentId id =
      Register(service, openpit::model::Instrument("AAPL", "USD"));

  EXPECT_EQ(
      service.PushFor(id, md::Quote().WithMark(Price::FromString("1")), {}, {}),
      md::RegisterStatus::NoTarget);
}

TEST(MarketDataService, PushForReachesPerAccountBucketUnderAccountOnly) {
  md::Service service = BuildService(md::SyncPolicy::None);
  const md::InstrumentId id =
      Register(service, openpit::model::Instrument("AAPL", "USD"));

  const std::vector<AccountId> accounts{AccountId::FromUint64(10),
                                        AccountId::FromUint64(11)};
  ASSERT_EQ(service.PushFor(id, md::Quote().WithMark(Price::FromString("150")),
                            accounts, {}),
            md::RegisterStatus::Ok);

  // Account 10 sees its per-account quote under AccountOnly.
  const std::optional<md::Quote> hit =
      service.Find(id, AccountId::FromUint64(10), NoGroupInfo{},
                   md::QuoteResolution::AccountOnly);
  ASSERT_TRUE(hit.has_value());
  EXPECT_EQ(hit->Mark()->ToString(), "150");

  // A different account has no per-account quote under AccountOnly.
  EXPECT_FALSE(service
                   .Find(id, AccountId::FromUint64(99), NoGroupInfo{},
                         md::QuoteResolution::AccountOnly)
                   .has_value());
}

TEST(MarketDataService, ResolutionFallsThroughToGroupBucket) {
  md::Service service = BuildService(md::SyncPolicy::None);
  const md::InstrumentId id =
      Register(service, openpit::model::Instrument("AAPL", "USD"));

  const AccountGroupId group = AccountGroupId::FromUint32(7);
  const std::vector<AccountGroupId> groups{group};
  ASSERT_EQ(service.PushFor(id, md::Quote().WithMark(Price::FromString("321")),
                            {}, groups),
            md::RegisterStatus::Ok);

  // The account has no per-account quote; the resolver supplies group 7, so the
  // read falls through to the group bucket.
  const FixedGroupInfo info{group};
  const std::optional<md::Quote> quote =
      service.Find(id, AccountId::FromUint64(55), info,
                   md::QuoteResolution::AccountThenGroup);
  ASSERT_TRUE(quote.has_value());
  EXPECT_EQ(quote->Mark()->ToString(), "321");

  // AccountOnly does not consult the group bucket.
  EXPECT_FALSE(service
                   .Find(id, AccountId::FromUint64(55), info,
                         md::QuoteResolution::AccountOnly)
                   .has_value());
}

TEST(MarketDataService, DefaultGroupBucketServesEveryoneElse) {
  md::Service service = BuildService(md::SyncPolicy::None);
  const md::InstrumentId id =
      Register(service, openpit::model::Instrument("AAPL", "USD"));

  // Targeting the default account group writes the "everyone-else" bucket.
  const std::vector<AccountGroupId> groups{DefaultAccountGroup};
  ASSERT_EQ(service.PushFor(id, md::Quote().WithMark(Price::FromString("7.5")),
                            {}, groups),
            md::RegisterStatus::Ok);

  const std::optional<md::Quote> quote =
      service.Find(id, AccountId::FromUint64(1), NoGroupInfo{},
                   md::QuoteResolution::AccountThenGroupThenDefault);
  ASSERT_TRUE(quote.has_value());
  EXPECT_EQ(quote->Mark()->ToString(), "7.5");
}

//------------------------------------------------------------------------------
// TTL freshness

TEST(MarketDataService, FiniteTtlExpiresQuote) {
  // A 1 ns lifetime: the quote ages out effectively immediately.
  md::Service service =
      md::Builder(md::QuoteTtl::Within(0, 1), md::SyncPolicy::None).Build();
  const md::InstrumentId id =
      Register(service, openpit::model::Instrument("AAPL", "USD"));
  const AccountId account = AccountId::FromUint64(1);

  ASSERT_EQ(service.Push(id, md::Quote().WithMark(Price::FromString("200"))),
            md::RegisterStatus::Ok);

  // After the (sub-nanosecond) lifetime elapses the quote reads as absent.
  std::this_thread::sleep_for(std::chrono::milliseconds(5));
  const md::GetResult expired =
      service.Get(id, account, NoGroupInfo{},
                  md::QuoteResolution::AccountThenGroupThenDefault);
  EXPECT_EQ(expired.status, md::GetStatus::QuoteExpired);
  ASSERT_TRUE(expired.quote.has_value());
  ASSERT_TRUE(expired.quote->Mark().has_value());
  EXPECT_EQ(expired.quote->Mark()->ToString(), "200");
  EXPECT_FALSE(service
                   .Find(id, account, NoGroupInfo{},
                         md::QuoteResolution::AccountThenGroupThenDefault)
                   .has_value());

  // An infinite instrument-level override keeps the next quote visible.
  ASSERT_EQ(service.SetInstrumentTtl(id, md::QuoteTtl::Infinite()),
            md::RegisterStatus::Ok);
  ASSERT_EQ(service.Push(id, md::Quote().WithMark(Price::FromString("205"))),
            md::RegisterStatus::Ok);
  std::this_thread::sleep_for(std::chrono::milliseconds(5));
  const std::optional<md::Quote> quote =
      service.Find(id, account, NoGroupInfo{},
                   md::QuoteResolution::AccountThenGroupThenDefault);
  ASSERT_TRUE(quote.has_value());
  EXPECT_EQ(quote->Mark()->ToString(), "205");
}

TEST(MarketDataService, SetInstrumentTtlOnUnknownReportsUnknown) {
  md::Service service = BuildService(md::SyncPolicy::None);
  EXPECT_EQ(service.SetInstrumentTtl(md::InstrumentId::FromUint64(404),
                                     md::QuoteTtl::Infinite()),
            md::RegisterStatus::UnknownInstrument);
  EXPECT_EQ(service.ClearInstrumentTtl(md::InstrumentId::FromUint64(404)),
            md::RegisterStatus::UnknownInstrument);
}

//------------------------------------------------------------------------------
// Clone shares state

TEST(MarketDataService, CloneSharesUnderlyingState) {
  md::Service feed = BuildService(md::SyncPolicy::Full);
  const md::InstrumentId id =
      Register(feed, openpit::model::Instrument("AAPL", "USD"));

  md::Service reader = feed.Clone();
  ASSERT_EQ(feed.Push(id, md::Quote().WithMark(Price::FromString("314.15"))),
            md::RegisterStatus::Ok);

  // A quote pushed through one handle is visible through the other.
  const std::optional<md::Quote> quote =
      reader.Find(id, AccountId::FromUint64(1), NoGroupInfo{},
                  md::QuoteResolution::AccountThenGroupThenDefault);
  ASSERT_TRUE(quote.has_value());
  EXPECT_EQ(quote->Mark()->ToString(), "314.15");
}

}  // namespace
