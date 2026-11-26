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

// The pre-trade / reject headers define `RejectScope` / `RejectCode` with their
// fixed underlying types; they must precede `openpit/pretrade/adapters.hpp`,
// whose
// opaque forward declarations of those scoped enums are only a compatible
// redeclaration when the full definition is already in scope.
#include "openpit/pretrade/pretrade.hpp"

#include "openpit/engine.hpp"
#include "openpit/marketdata.hpp"
#include "openpit/model/model.hpp"
#include "openpit/pretrade/adapters.hpp"
#include "openpit/pretrade/decision.hpp"

#include <gmock/gmock.h>
#include <gtest/gtest.h>

#include <algorithm>
#include <cstdint>
#include <memory>
#include <optional>
#include <stdexcept>
#include <string>
#include <string_view>
#include <type_traits>
#include <typeinfo>
#include <utility>
#include <vector>

namespace {

using openpit::param::Price;
using openpit::param::Quantity;
using openpit::param::Volume;
using openpit::pretrade::Context;
using openpit::pretrade::ContextOrder;
using openpit::pretrade::CustomPolicy;
using openpit::pretrade::LockEntry;
using openpit::pretrade::PolicyDecision;
using openpit::pretrade::PreTradeLock;
using openpit::pretrade::PushReject;
using openpit::pretrade::Reject;
using openpit::pretrade::RejectCode;
using openpit::pretrade::RejectScope;
using openpit::pretrade::RuleMode;

static_assert(!std::is_move_constructible_v<openpit::tx::Mutations>);
static_assert(
    !std::is_move_constructible_v<openpit::accountadjustment::Context>);
static_assert(!std::is_move_constructible_v<openpit::pretrade::Context>);
static_assert(!std::is_move_constructible_v<openpit::pretrade::Result>);
static_assert(
    !std::is_copy_constructible_v<openpit::pretrade::DropCopyOperation>);
static_assert(
    std::is_nothrow_move_constructible_v<openpit::pretrade::DropCopyOperation>);
static_assert(
    !std::is_move_constructible_v<openpit::pretrade::PostTradeContext>);
static_assert(
    !std::is_move_constructible_v<openpit::pretrade::PostTradeAdjustments>);
static_assert(!std::is_move_constructible_v<openpit::pretrade::PostTradePnls>);
static_assert(
    !std::is_move_constructible_v<openpit::pretrade::AccountOutcomes>);

namespace policies = openpit::pretrade::policies;

constexpr std::uint16_t kDefaultGroup = OPENPIT_DEFAULT_POLICY_GROUP_ID;
constexpr std::uint16_t kGroupSeven = 7;

//------------------------------------------------------------------------------
// PreTradeLock

void ExpectHandlelessLockOperationsThrow(PreTradeLock& lock) {
  EXPECT_NO_THROW(static_cast<void>(static_cast<bool>(lock)));
  EXPECT_FALSE(lock);
  EXPECT_THROW(static_cast<void>(lock.Clone()), openpit::Error);
  EXPECT_THROW(static_cast<void>(lock.Len()), openpit::Error);
  EXPECT_THROW(static_cast<void>(lock.IsEmpty()), openpit::Error);
  EXPECT_THROW(lock.Push(kDefaultGroup, Price::FromString("1")),
               openpit::Error);
  const std::vector<LockEntry> entries = {
      LockEntry(kDefaultGroup, Price::FromString("1")),
  };
  EXPECT_THROW(lock.PushMany(entries), openpit::Error);
  PreTradeLock src;
  EXPECT_THROW(lock.Merge(src), openpit::Error);
  EXPECT_THROW(static_cast<void>(lock.Entries()), openpit::Error);
  EXPECT_THROW(static_cast<void>(lock.Prices()), openpit::Error);
  EXPECT_THROW(static_cast<void>(lock.PricesOf(kDefaultGroup)), openpit::Error);
  EXPECT_THROW(static_cast<void>(lock.ToMsgpack()), openpit::Error);
  EXPECT_THROW(static_cast<void>(lock.ToCbor()), openpit::Error);
  EXPECT_THROW(static_cast<void>(lock.ToJson()), openpit::Error);
}

TEST(PreTradeLock, NewLockIsEmpty) {
  const PreTradeLock lock;
  EXPECT_TRUE(lock.IsEmpty());
  EXPECT_EQ(lock.Len(), 0u);
  EXPECT_TRUE(lock.Entries().empty());
  EXPECT_TRUE(lock.Prices().empty());
}

TEST(PreTradeLock, PushAccumulatesPricesExactly) {
  PreTradeLock lock;
  lock.Push(kDefaultGroup, Price::FromString("185.25"));
  lock.Push(kGroupSeven, Price::FromString("0.10"));

  EXPECT_FALSE(lock.IsEmpty());
  EXPECT_EQ(lock.Len(), 2u);

  const std::vector<Price> defaultPrices = lock.PricesOf(kDefaultGroup);
  ASSERT_EQ(defaultPrices.size(), 1u);
  EXPECT_EQ(defaultPrices.front().ToString(), "185.25");

  const std::vector<Price> groupPrices = lock.PricesOf(kGroupSeven);
  ASSERT_EQ(groupPrices.size(), 1u);
  EXPECT_EQ(groupPrices.front().ToString(), "0.10");
}

TEST(PreTradeLock, PushManyPreservesOrderAndDecimals) {
  PreTradeLock lock;
  const std::vector<LockEntry> entries = {
      LockEntry(kDefaultGroup, Price::FromString("1.5")),
      LockEntry(kDefaultGroup, Price::FromString("2.25")),
      LockEntry(kGroupSeven, Price::FromString("3.125")),
  };
  lock.PushMany(entries);

  EXPECT_EQ(lock.Len(), 3u);

  const std::vector<LockEntry> snapshot = lock.Entries();
  ASSERT_EQ(snapshot.size(), 3u);
  // Iteration order: default-group records first, then non-default in
  // insertion order.
  EXPECT_EQ(snapshot[0].policyGroupId, kDefaultGroup);
  EXPECT_EQ(snapshot[0].price.ToString(), "1.5");
  EXPECT_EQ(snapshot[1].policyGroupId, kDefaultGroup);
  EXPECT_EQ(snapshot[1].price.ToString(), "2.25");
  EXPECT_EQ(snapshot[2].policyGroupId, kGroupSeven);
  EXPECT_EQ(snapshot[2].price.ToString(), "3.125");

  const std::vector<Price> prices = lock.Prices();
  ASSERT_EQ(prices.size(), 3u);
  EXPECT_EQ(prices[0].ToString(), "1.5");
  EXPECT_EQ(prices[2].ToString(), "3.125");
}

TEST(PreTradeLock, PricesOfAbsentGroupIsEmpty) {
  PreTradeLock lock;
  lock.Push(kDefaultGroup, Price::FromString("10"));
  EXPECT_TRUE(lock.PricesOf(kGroupSeven).empty());
}

TEST(PreTradeLock, MergeAppendsSourceRecords) {
  PreTradeLock dst;
  dst.Push(kDefaultGroup, Price::FromString("1"));

  PreTradeLock src;
  src.Push(kGroupSeven, Price::FromString("2"));
  src.Push(kGroupSeven, Price::FromString("3"));

  dst.Merge(src);

  EXPECT_EQ(dst.Len(), 3u);
  // Source is unchanged.
  EXPECT_EQ(src.Len(), 2u);

  const std::vector<Price> merged = dst.PricesOf(kGroupSeven);
  ASSERT_EQ(merged.size(), 2u);
  EXPECT_EQ(merged[0].ToString(), "2");
  EXPECT_EQ(merged[1].ToString(), "3");
}

TEST(PreTradeLock, CloneIsIndependent) {
  PreTradeLock lock;
  lock.Push(kDefaultGroup, Price::FromString("5"));

  PreTradeLock copy = lock.Clone();
  copy.Push(kDefaultGroup, Price::FromString("6"));

  EXPECT_EQ(lock.Len(), 1u);
  EXPECT_EQ(copy.Len(), 2u);
}

TEST(PreTradeLock, CopyConstructionIsIndependent) {
  PreTradeLock source;
  source.Push(kDefaultGroup, Price::FromString("5"));

  PreTradeLock copy(source);
  copy.Push(kDefaultGroup, Price::FromString("6"));

  EXPECT_EQ(source.Len(), 1u);
  EXPECT_EQ(copy.Len(), 2u);
}

TEST(PreTradeLock, CopyAssignmentReplacesContentsIndependently) {
  PreTradeLock source;
  source.Push(kDefaultGroup, Price::FromString("5"));

  PreTradeLock copy;
  copy.Push(kDefaultGroup, Price::FromString("99"));
  copy = source;
  copy.Push(kDefaultGroup, Price::FromString("6"));

  const std::vector<Price> prices = copy.PricesOf(kDefaultGroup);
  ASSERT_EQ(prices.size(), 2u);
  EXPECT_EQ(prices[0].ToString(), "5");
  EXPECT_EQ(prices[1].ToString(), "6");
  EXPECT_EQ(source.Len(), 1u);
}

TEST(PreTradeLock, SelfAssignmentKeepsContents) {
  PreTradeLock lock;
  lock.Push(kDefaultGroup, Price::FromString("5"));

  PreTradeLock* const sameLock = &lock;
  lock = *sameLock;

  const std::vector<Price> prices = lock.PricesOf(kDefaultGroup);
  ASSERT_EQ(prices.size(), 1u);
  EXPECT_EQ(prices.front().ToString(), "5");
}

TEST(PreTradeLock, CopyingMovedFromLockYieldsMovedFromLock) {
  PreTradeLock lock;
  PreTradeLock moved = std::move(lock);

  ASSERT_TRUE(moved);
  EXPECT_NO_THROW({
    const PreTradeLock copy(lock);
    EXPECT_FALSE(copy);
  });
}

TEST(PreTradeLock, MovedFromOperationsThrow) {
  PreTradeLock lock;
  PreTradeLock moved = std::move(lock);

  ASSERT_TRUE(moved);
  ExpectHandlelessLockOperationsThrow(lock);
}

TEST(PreTradeLock, CopiedMovedFromOperationsThrow) {
  PreTradeLock lock;
  PreTradeLock moved = std::move(lock);
  PreTradeLock copy(lock);

  ASSERT_TRUE(moved);
  ASSERT_FALSE(copy);
  ExpectHandlelessLockOperationsThrow(copy);
}

TEST(PreTradeLock, HandlelessMergeSourceThrows) {
  PreTradeLock source;
  PreTradeLock moved = std::move(source);
  PreTradeLock target;

  ASSERT_TRUE(moved);
  ASSERT_FALSE(source);
  EXPECT_THROW(target.Merge(source), openpit::Error);
}

TEST(ExecutionReport, CopyWithFillLockIsIndependent) {
  openpit::model::ExecutionReport report;
  report.fill.emplace();
  report.fill->lock.emplace();
  report.fill->lock->Push(kDefaultGroup, Price::FromString("5"));

  openpit::model::ExecutionReport copy = report;
  copy.fill->lock->Push(kDefaultGroup, Price::FromString("6"));

  EXPECT_EQ(report.fill->lock->Len(), 1u);
  EXPECT_EQ(copy.fill->lock->Len(), 2u);
}

TEST(FillSerialization, MovedFromLockFailsBeforeEngineSubmission) {
  openpit::model::Fill fill;
  fill.lock.emplace();
  PreTradeLock detached = std::move(*fill.lock);
  ASSERT_TRUE(detached);
  ASSERT_TRUE(fill.lock.has_value());
  EXPECT_FALSE(*fill.lock);

  openpit::model::ExecutionReport report;
  report.fill = std::move(fill);
  openpit::EngineBuilder builder(openpit::SyncPolicy::None);
  builder.Add(policies::OrderValidationPolicy{});
  const openpit::Engine engine = builder.Build();

  EXPECT_THROW(static_cast<void>(engine.ApplyExecutionReport(report)),
               openpit::Error);
}

TEST(Reservation, EmptyHandleOperationsThrow) {
  openpit::pretrade::Reservation reservation;

  EXPECT_THROW(reservation.Commit(), openpit::Error);
  EXPECT_THROW(reservation.Rollback(), openpit::Error);
  EXPECT_THROW(static_cast<void>(reservation.Lock()), openpit::Error);
  EXPECT_THROW(static_cast<void>(reservation.AccountAdjustments()),
               openpit::Error);
}

TEST(DropCopyOperation, EmptyHandleOperationsThrow) {
  openpit::pretrade::DropCopyOperation operation;

  EXPECT_THROW(operation.Commit(), openpit::Error);
  EXPECT_THROW(operation.Rollback(), openpit::Error);
  EXPECT_THROW(static_cast<void>(operation.Lock()), openpit::Error);
  EXPECT_THROW(static_cast<void>(operation.AccountAdjustments()),
               openpit::Error);
  EXPECT_THROW(static_cast<void>(operation.AccountBlock()), openpit::Error);
  EXPECT_THROW(static_cast<void>(operation.IsAccountBlocked()), openpit::Error);
}

TEST(DryRunReport, EmptyHandleObserversThrow) {
  openpit::pretrade::DryRunReport report;

  EXPECT_THROW(static_cast<void>(report.Passed()), openpit::Error);
  EXPECT_THROW(static_cast<void>(report.Rejects()), openpit::Error);
  EXPECT_THROW(static_cast<void>(report.Lock()), openpit::Error);
  EXPECT_THROW(static_cast<void>(report.AccountAdjustments()), openpit::Error);
  EXPECT_THROW(static_cast<void>(report.AccountBlock()), openpit::Error);
}

TEST(PreTradeLock, JsonRoundTripPreservesRecords) {
  PreTradeLock lock;
  lock.Push(kDefaultGroup, Price::FromString("42.5"));

  const std::string json = lock.ToJson();
  EXPECT_FALSE(json.empty());

  const PreTradeLock restored = PreTradeLock::FromJson(json);
  ASSERT_EQ(restored.PricesOf(kDefaultGroup).size(), 1u);
  EXPECT_EQ(restored.PricesOf(kDefaultGroup).front().ToString(), "42.5");
}

TEST(PreTradeLock, MsgpackAndCborRoundTrip) {
  PreTradeLock lock;
  lock.Push(kGroupSeven, Price::FromString("7.77"));

  const PreTradeLock fromMsgpack = PreTradeLock::FromMsgpack(lock.ToMsgpack());
  ASSERT_EQ(fromMsgpack.PricesOf(kGroupSeven).size(), 1u);
  EXPECT_EQ(fromMsgpack.PricesOf(kGroupSeven).front().ToString(), "7.77");

  const PreTradeLock fromCbor = PreTradeLock::FromCbor(lock.ToCbor());
  ASSERT_EQ(fromCbor.PricesOf(kGroupSeven).size(), 1u);
  EXPECT_EQ(fromCbor.PricesOf(kGroupSeven).front().ToString(), "7.77");
}

//------------------------------------------------------------------------------
// Reject / PolicyDecision

TEST(PolicyDecision, EmptyDecisionAccepts) {
  const PolicyDecision decision;
  EXPECT_FALSE(decision.IsRejected());
}

TEST(PolicyDecision, PushRejectMakesItRejected) {
  PolicyDecision decision;
  PushReject(decision, Reject("p", RejectScope::Order, RejectCode::Other,
                              "reason", "Expected"));
  ASSERT_TRUE(decision.IsRejected());
  ASSERT_EQ(decision.rejects.size(), 1u);
  EXPECT_EQ(decision.rejects.front().policy, "p");
  EXPECT_EQ(decision.rejects.front().details, "Expected");
}

//------------------------------------------------------------------------------
// Context

TEST(Context, ContextOrderRecoversConcreteOrder) {
  openpit::model::Order order;
  order.userData = 17;
  const Context context(order);

  const openpit::Order& base = ContextOrder(context);
  const auto* recovered = dynamic_cast<const openpit::model::Order*>(&base);
  ASSERT_NE(recovered, nullptr);
  EXPECT_EQ(recovered->userData, 17u);
}

TEST(Context, AccountGroupIsAbsentWithoutNativeContext) {
  const openpit::model::Order order;
  const Context context(order);
  EXPECT_FALSE(context.AccountGroup().has_value());
  EXPECT_FALSE(context.AccountControl().has_value());
  EXPECT_THROW(static_cast<void>(context.AccountId()), openpit::Error);
}

//------------------------------------------------------------------------------
// Custom policy via the adapter templates (SafeSlow).
//
// A client order payload deriving from openpit::model::Order, a client policy
// with the adapter-expected surface, and the PolicyAdapter / StartPolicyAdapter
// bridging them. The adapter throws on a type mismatch and accepts the happy
// path otherwise.

struct DeskOrder : public openpit::model::Order {
  std::uint32_t lots = 0;
};

class DeskPolicy {
 public:
  [[nodiscard]] std::string_view Name() const noexcept { return "DeskPolicy"; }

  [[nodiscard]] std::optional<Reject> CheckPreTradeStart(
      const DeskOrder& order) const {
    if (order.lots == 0) {
      return Reject(std::string(Name()), RejectScope::Order,
                    RejectCode::InvalidFieldValue, "lots must be non-zero",
                    "non-zero lots");
    }
    return std::nullopt;
  }

  void PerformPreTradeCheck(const DeskOrder& order, const Context& context,
                            PolicyDecision& decision) const {
    static_cast<void>(context);
    if (order.lots > 100) {
      PushReject(decision, Reject(std::string(Name()), RejectScope::Order,
                                  RejectCode::OrderQtyExceedsLimit,
                                  "lots exceed limit", "max 100 lots"));
    }
  }

  [[nodiscard]] bool ApplyExecutionReport(
      const openpit::ExecutionReport& report) const {
    static_cast<void>(report);
    return false;
  }
};

using DeskMainAdapter = openpit::pretrade::PolicyAdapterWithSafeSlowArgType<
    DeskPolicy, DeskOrder, openpit::ExecutionReport>;
using DeskStartAdapter =
    openpit::pretrade::StartPolicyAdapterWithSafeSlowArgType<
        DeskPolicy, DeskOrder, openpit::ExecutionReport>;

// A context-taking start hook is outside the adapter contract, in either
// argument order and whether it takes `const Context&` or `Context&`, so both
// adapters reject it at compile time instead of exposing no such hook and
// losing it silently.
struct OrderContextStartDeskPolicy {
  [[nodiscard]] std::optional<Reject> CheckPreTradeStart(const DeskOrder&,
                                                         const Context&) const;
  [[nodiscard]] std::optional<Reject> CheckPreTradeStartDryRun(const DeskOrder&,
                                                               Context&) const;
};

struct ContextOrderStartDeskPolicy {
  [[nodiscard]] std::optional<Reject> CheckPreTradeStart(
      Context&, const DeskOrder&) const;
  [[nodiscard]] std::optional<Reject> CheckPreTradeStartDryRun(
      const Context&, const DeskOrder&) const;
};

struct OrderOnlyStartDeskPolicy {
  [[nodiscard]] std::optional<Reject> CheckPreTradeStart(
      const DeskOrder&) const;
  [[nodiscard]] std::optional<Reject> CheckPreTradeStartDryRun(
      const DeskOrder&) const;
};

static_assert(openpit::pretrade::detail::HasStartWithContext<
              OrderContextStartDeskPolicy, DeskOrder>::value);
static_assert(openpit::pretrade::detail::HasStartDryRunWithContext<
              OrderContextStartDeskPolicy, DeskOrder>::value);
static_assert(openpit::pretrade::detail::HasStartWithContext<
              ContextOrderStartDeskPolicy, DeskOrder>::value);
static_assert(openpit::pretrade::detail::HasStartDryRunWithContext<
              ContextOrderStartDeskPolicy, DeskOrder>::value);

// The supported order-only forms must not trip the detectors.
static_assert(!openpit::pretrade::detail::HasStartWithContext<
              OrderOnlyStartDeskPolicy, DeskOrder>::value);
static_assert(!openpit::pretrade::detail::HasStartDryRunWithContext<
              OrderOnlyStartDeskPolicy, DeskOrder>::value);

TEST(PolicyAdapter, SafeSlowMainStageHappyPathAccepts) {
  DeskMainAdapter adapter{DeskPolicy{}};
  DeskOrder order;
  order.lots = 10;
  const Context context(order);

  PolicyDecision decision;
  adapter.PerformPreTradeCheck(context, decision);
  EXPECT_FALSE(decision.IsRejected());
}

TEST(PolicyAdapter, SafeSlowMainStageRejectsOnBusinessRule) {
  DeskMainAdapter adapter{DeskPolicy{}};
  DeskOrder order;
  order.lots = 250;
  const Context context(order);

  PolicyDecision decision;
  adapter.PerformPreTradeCheck(context, decision);
  ASSERT_TRUE(decision.IsRejected());
  EXPECT_EQ(decision.rejects.front().code, RejectCode::OrderQtyExceedsLimit);
}

TEST(PolicyAdapter, SafeSlowMainStageThrowsOnTypeMismatch) {
  DeskMainAdapter adapter{DeskPolicy{}};
  // A plain model::Order is not a DeskOrder, so SafeSlow must fail before the
  // policy can run.
  openpit::model::Order foreign;
  const Context context(foreign);

  PolicyDecision decision;
  try {
    adapter.PerformPreTradeCheck(context, decision);
    FAIL() << "expected the mismatched policy adapter to throw";
  } catch (const openpit::Error& error) {
    EXPECT_THAT(error.what(), testing::HasSubstr("DeskPolicy"));
    EXPECT_THAT(error.what(), testing::HasSubstr(typeid(DeskOrder).name()));
    EXPECT_THAT(error.what(), testing::HasSubstr(typeid(foreign).name()));
  }
  EXPECT_FALSE(decision.IsRejected());
}

TEST(StartPolicyAdapter, SafeSlowStartStageHappyPathAndTypeMismatch) {
  DeskStartAdapter adapter{DeskPolicy{}};

  DeskOrder order;
  order.lots = 5;
  EXPECT_FALSE(adapter.CheckPreTradeStart(order).has_value());

  openpit::model::Order foreign;
  try {
    static_cast<void>(adapter.CheckPreTradeStart(foreign));
    FAIL() << "expected the mismatched policy adapter to throw";
  } catch (const openpit::Error& error) {
    EXPECT_THAT(error.what(), testing::HasSubstr("DeskPolicy"));
    EXPECT_THAT(error.what(), testing::HasSubstr(typeid(DeskOrder).name()));
    EXPECT_THAT(error.what(), testing::HasSubstr(typeid(foreign).name()));
  }
}

struct DeskReport : public openpit::model::ExecutionReport {};

class DeskReportPolicy {
 public:
  [[nodiscard]] std::string_view Name() const noexcept {
    return "DeskReportPolicy";
  }

  void PerformPreTradeCheck(const openpit::model::Order& order,
                            const Context& context,
                            PolicyDecision& decision) const {
    static_cast<void>(order);
    static_cast<void>(context);
    static_cast<void>(decision);
  }

  [[nodiscard]] std::vector<openpit::accounts::AccountBlock>
  ApplyExecutionReport(const openpit::pretrade::PostTradeContext&,
                       const DeskReport&,
                       openpit::pretrade::PostTradeAdjustments&,
                       openpit::pretrade::PostTradePnls&) const {
    return {};
  }
};

using DeskReportAdapter = openpit::pretrade::PolicyAdapterWithSafeSlowArgType<
    DeskReportPolicy, openpit::model::Order, DeskReport>;

TEST(PolicyAdapter, SafeSlowPostTradeMismatchRethrowsThroughEngine) {
  openpit::EngineBuilder builder(openpit::SyncPolicy::None);
  CustomPolicy<DeskReportAdapter> policy("DeskReportPolicy",
                                         DeskReportAdapter{DeskReportPolicy{}});
  builder.Add(policy);
  const openpit::Engine engine = builder.Build();
  const openpit::model::ExecutionReport foreign;

  try {
    static_cast<void>(engine.ApplyExecutionReport(foreign));
    FAIL() << "expected the mismatched policy adapter to throw";
  } catch (const openpit::Error& error) {
    EXPECT_THAT(error.what(), testing::HasSubstr("DeskReportPolicy"));
    EXPECT_THAT(error.what(), testing::HasSubstr(typeid(DeskReport).name()));
    EXPECT_THAT(error.what(), testing::HasSubstr(typeid(foreign).name()));
  }
}

TEST(PolicyAdapter, UnifiedAdapterDelegatesOptionalStartHook) {
  DeskMainAdapter adapter{DeskPolicy{}};

  DeskOrder good;
  good.lots = 5;
  EXPECT_FALSE(adapter.CheckPreTradeStart(good).has_value());

  DeskOrder rejected;
  const std::optional<Reject> reject = adapter.CheckPreTradeStart(rejected);
  ASSERT_TRUE(reject.has_value());
  EXPECT_EQ(reject->code, RejectCode::InvalidFieldValue);
}

TEST(CustomPolicy, WrapsAdapterAndRegistersOnBuilder) {
  openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
  CustomPolicy<DeskMainAdapter> policy("DeskPolicy",
                                       DeskMainAdapter{DeskPolicy{}});
  EXPECT_EQ(policy.Name(), "DeskPolicy");

  // Registration retains its own reference; the engine then builds.
  builder.Add(policy);
  EXPECT_NO_THROW({ openpit::Engine engine = builder.Build(); });
}

TEST(CustomPolicy, AdapterNameMismatchThrows) {
  EXPECT_THROW(
      {
        CustomPolicy<DeskMainAdapter> policy("", DeskMainAdapter{DeskPolicy{}});
      },
      openpit::Error);
}

class InvalidRejectScopePolicy {
 public:
  [[nodiscard]] std::optional<Reject> CheckPreTradeStart(
      const openpit::Order&) const {
    return Reject("InvalidRejectScopePolicy", static_cast<RejectScope>(0),
                  RejectCode::Other, "invalid scope", "scope zero is invalid");
  }
};

TEST(CustomPolicy, RejectListPushFailureRethrowsInvalidScope) {
  openpit::EngineBuilder builder(openpit::SyncPolicy::None);
  CustomPolicy<InvalidRejectScopePolicy> policy("InvalidRejectScopePolicy",
                                                InvalidRejectScopePolicy{});
  builder.Add(policy);
  const openpit::Engine engine = builder.Build();
  const openpit::model::Order order;

  try {
    static_cast<void>(engine.StartPreTrade(order));
    FAIL() << "expected an invalid_argument for the unknown reject scope";
  } catch (const std::invalid_argument& error) {
    EXPECT_STREQ(error.what(), "reject scope or mode is invalid");
  }
}

struct LegacyThreeArgumentPostTradePolicy {
  [[nodiscard]] std::vector<openpit::accounts::AccountBlock>
  ApplyExecutionReport(const openpit::pretrade::PostTradeContext&,
                       const openpit::ExecutionReport&,
                       openpit::pretrade::PostTradeAdjustments&) const;
};

static_assert(openpit::pretrade::detail::HasLegacyApplyExecutionReportFull<
              LegacyThreeArgumentPostTradePolicy>::value);
static_assert(!openpit::pretrade::detail::HasApplyExecutionReportFull<
              LegacyThreeArgumentPostTradePolicy>::value);

// The adapters carry the same diagnostic: the removed three-argument form
// matches neither adapter report hook, so it must be rejected loudly instead
// of SFINAE-removing the hook and losing it silently.
static_assert(
    openpit::pretrade::detail::HasLegacyReportFull<
        LegacyThreeArgumentPostTradePolicy, openpit::ExecutionReport>::value);
static_assert(
    !openpit::pretrade::detail::HasReportFull<
        LegacyThreeArgumentPostTradePolicy, openpit::ExecutionReport>::value);
static_assert(
    !openpit::pretrade::detail::HasReportLegacy<
        LegacyThreeArgumentPostTradePolicy, openpit::ExecutionReport>::value);

class SplitPostTradePolicy {
 public:
  bool unreadableAccount = false;
  void PerformPreTradeCheck(const openpit::Order&, const Context& context,
                            PolicyDecision&) const {
    if (unreadableAccount) {
      EXPECT_THROW(static_cast<void>(context.AccountId()), openpit::Error);
    } else {
      EXPECT_EQ(context.AccountId(), openpit::param::AccountId::FromUint64(42));
    }
  }

  [[nodiscard]] std::vector<openpit::accounts::AccountBlock>
  ApplyExecutionReport(const openpit::pretrade::PostTradeContext& context,
                       const openpit::ExecutionReport& report,
                       openpit::pretrade::PostTradeAdjustments& adjustments,
                       openpit::pretrade::PostTradePnls& pnls) const {
    if (unreadableAccount) {
      EXPECT_THROW(static_cast<void>(context.AccountId()), openpit::Error);
    } else {
      EXPECT_EQ(context.AccountId(), openpit::param::AccountId::FromUint64(42));
    }
    static_cast<void>(report);
    pnls.Push(openpit::accountadjustment::AccountPnlOutcome{
        openpit::accountadjustment::PnlOutcomeAmount(
            openpit::param::Pnl::FromString("-12.5"),
            openpit::param::Pnl::FromString("87.5")),
        openpit::param::AccountId::FromUint64(42),
        openpit::param::GroupId(kGroupSeven),
    });
    openpit::accountadjustment::AccountOutcomeEntry adjustment(
        openpit::param::Asset("USD"));
    adjustment.balance = openpit::accountadjustment::OutcomeAmount(
        openpit::param::PositionSize::FromString("-5"),
        openpit::param::PositionSize::FromString("95"));
    adjustments.Push(openpit::param::GroupId(kGroupSeven), adjustment);
    return {openpit::accounts::AccountBlock(
        RejectCode::RiskLimitExceeded, "SplitPostTradePolicy",
        "account PnL limit reached", "account 42")};
  }
};

// The current four-argument form must not trip the legacy detector.
static_assert(!openpit::pretrade::detail::HasLegacyReportFull<
              SplitPostTradePolicy, openpit::ExecutionReport>::value);
static_assert(openpit::pretrade::detail::HasReportFull<
              SplitPostTradePolicy, openpit::ExecutionReport>::value);

TEST(CustomPolicy, ProducesSplitPostTradeOutputs) {
  openpit::EngineBuilder builder(openpit::SyncPolicy::None);
  CustomPolicy<SplitPostTradePolicy> policy("SplitPostTradePolicy",
                                            SplitPostTradePolicy{});
  builder.Add(policy);
  const openpit::Engine engine = builder.Build();

  openpit::model::ExecutionReport report;
  report.operation = openpit::model::ExecutionReportOperation{};
  report.operation->accountId = openpit::param::AccountId::FromUint64(42);
  const openpit::PostTradeResult result = engine.ApplyExecutionReport(report);

  ASSERT_EQ(result.accountBlocks.size(), 1u);
  EXPECT_EQ(result.accountBlocks.front().policy, "SplitPostTradePolicy");
  EXPECT_EQ(result.accountBlocks.front().code, RejectCode::RiskLimitExceeded);

  ASSERT_EQ(result.accountPnls.size(), 1u);
  const auto& outcome = result.accountPnls.front();
  EXPECT_EQ(outcome.accountId, openpit::param::AccountId::FromUint64(42));
  EXPECT_EQ(outcome.policyGroupId, openpit::param::GroupId(kGroupSeven));
  ASSERT_NE(outcome.Amount(), nullptr);
  const auto& amount = *outcome.Amount();
  EXPECT_EQ(amount.delta, openpit::param::Pnl::FromString("-12.5"));
  EXPECT_EQ(amount.absolute, openpit::param::Pnl::FromString("87.5"));

  ASSERT_EQ(result.accountAdjustments.size(), 1u);
  const auto& adjustment = result.accountAdjustments.front();
  EXPECT_EQ(adjustment.policyGroupId, openpit::param::GroupId(kGroupSeven));
  EXPECT_EQ(adjustment.entry.asset.View(), "USD");
  ASSERT_TRUE(adjustment.entry.balance.has_value());
  EXPECT_EQ(adjustment.entry.balance->delta,
            openpit::param::PositionSize::FromString("-5"));
  EXPECT_EQ(adjustment.entry.balance->absolute,
            openpit::param::PositionSize::FromString("95"));
}

TEST(CustomPolicy, OperationAccountContextAccessorsReportUnreadableAccounts) {
  for (const bool unreadable : {false, true}) {
    openpit::EngineBuilder builder(openpit::SyncPolicy::None);
    CustomPolicy<SplitPostTradePolicy> policy("operation-account",
                                              SplitPostTradePolicy{unreadable});
    builder.Add(policy);
    const openpit::Engine engine = builder.Build();
    openpit::model::Order order;
    openpit::model::ExecutionReport report;
    if (!unreadable) {
      order.operation = openpit::model::OrderOperation{};
      order.operation->accountId = openpit::param::AccountId::FromUint64(42);
      report.operation = openpit::model::ExecutionReportOperation{};
      report.operation->accountId = openpit::param::AccountId::FromUint64(42);
    }
    EXPECT_TRUE(engine.ExecutePreTrade(order).rejects.empty());
    EXPECT_EQ(engine.ApplyExecutionReport(report).accountPnls.size(), 1u);
  }
}

struct DryRunHookCounters {
  std::uint32_t start = 0;
  std::uint32_t startDryRun = 0;
  std::uint32_t main = 0;
  std::uint32_t mainDryRun = 0;
};

class DryRunHookPolicy {
 public:
  explicit DryRunHookPolicy(DryRunHookCounters* counters)
      : m_counters(counters) {}

  [[nodiscard]] std::optional<Reject> CheckPreTradeStart(
      const openpit::Order& order) const {
    static_cast<void>(order);
    ++m_counters->start;
    return std::nullopt;
  }

  [[nodiscard]] std::optional<Reject> CheckPreTradeStartDryRun(
      const openpit::Order& order) const {
    static_cast<void>(order);
    ++m_counters->startDryRun;
    return std::nullopt;
  }

  void PerformPreTradeCheck(const Context& context,
                            PolicyDecision& decision) const {
    static_cast<void>(context);
    ++m_counters->main;
    PushReject(decision, Reject("DryRunHookPolicy", RejectScope::Order,
                                RejectCode::Custom, "real path rejects",
                                "dry-run should not use this hook"));
  }

  void PerformPreTradeCheckDryRun(const Context& context,
                                  PolicyDecision& decision) const {
    static_cast<void>(context);
    static_cast<void>(decision);
    ++m_counters->mainDryRun;
  }

 private:
  DryRunHookCounters* m_counters;
};

// Its only dry-run hook is the context-taking start dry-run.
class ContextStartDryRunHookPolicy {
 public:
  explicit ContextStartDryRunHookPolicy(DryRunHookCounters* counters)
      : m_counters(counters) {}

  [[nodiscard]] std::optional<Reject> CheckPreTradeStart(
      const Context& context, const openpit::Order& order) const {
    static_cast<void>(context);
    static_cast<void>(order);
    ++m_counters->start;
    return std::nullopt;
  }

  [[nodiscard]] std::optional<Reject> CheckPreTradeStartDryRun(
      const Context& context, const openpit::Order& order) const {
    static_cast<void>(context);
    static_cast<void>(order);
    ++m_counters->startDryRun;
    return std::nullopt;
  }

 private:
  DryRunHookCounters* m_counters;
};

class DropCopyBlockingPolicy {
 public:
  void PerformPreTradeCheck(const Context& context,
                            openpit::tx::Mutations& mutations,
                            openpit::pretrade::Result& result,
                            PolicyDecision& decision) const {
    static_cast<void>(context);
    static_cast<void>(mutations);
    result.PushLockPrice(Price::FromString("13"));
    result.PushAccountAdjustment(
        ::openpit::accountadjustment::AccountOutcomeEntry(
            ::openpit::param::Asset("USD")));
    PushReject(decision,
               Reject("DropCopyBlockingPolicy", RejectScope::Account,
                      RejectCode::RiskLimitExceeded, "forced account block",
                      "drop-copy must retain the block"));
  }
};

// Accepts every order; an engine needs at least one registered policy.
class AcceptingPolicy {
 public:
  void PerformPreTradeCheck(const Context& context,
                            PolicyDecision& decision) const {
    static_cast<void>(context);
    static_cast<void>(decision);
  }
};

class MissingFieldPolicy {
 public:
  void PerformPreTradeCheck(const Context& context,
                            PolicyDecision& decision) const {
    static_cast<void>(context);
    PushReject(decision,
               Reject("MissingFieldPolicy", RejectScope::Order,
                      RejectCode::MissingRequiredField, "missing limit price",
                      "policy needs a limit price"));
  }
};

class DropCopyStartMutationPolicy {
 public:
  DropCopyStartMutationPolicy(int* value, bool* sawDropCopy, bool fatal)
      : m_value(value), m_sawDropCopy(sawDropCopy), m_fatal(fatal) {}

  [[nodiscard]] std::optional<Reject> CheckPreTradeStart(
      const Context& context, const openpit::Order& order) const {
    static_cast<void>(order);
    *m_sawDropCopy = context.IsDropCopy();
    const int previous = *m_value;
    ++*m_value;
    context.RecordDropCopyStartMutation(
        [] {}, [value = m_value, previous] { *value = previous; });
    if (!m_fatal) {
      return std::nullopt;
    }
    return Reject("DropCopyStartMutationPolicy", RejectScope::Order,
                  RejectCode::MissingRequiredField, "missing field",
                  "forced after start-stage mutation");
  }

 private:
  int* m_value;
  bool* m_sawDropCopy;
  bool m_fatal;
};

struct DropCopyGatedState {
  std::vector<bool> startModes;
  std::vector<bool> mainModes;
  int commits = 0;
  int rollbacks = 0;
};

// Registers the main-stage mutation and account adjustment only in drop copy,
// and returns the same business reject in both modes.
class DropCopyGatedRejectPolicy {
 public:
  explicit DropCopyGatedRejectPolicy(DropCopyGatedState* state)
      : m_state(state) {}

  [[nodiscard]] std::optional<Reject> CheckPreTradeStart(
      const Context& context, const openpit::Order& order) const {
    static_cast<void>(order);
    m_state->startModes.push_back(context.IsDropCopy());
    return std::nullopt;
  }

  void PerformPreTradeCheck(const Context& context,
                            openpit::tx::Mutations& mutations,
                            openpit::pretrade::Result& result,
                            PolicyDecision& decision) const {
    m_state->mainModes.push_back(context.IsDropCopy());
    if (context.IsDropCopy()) {
      mutations.Push([state = m_state] { ++state->commits; },
                     [state = m_state] { ++state->rollbacks; });
      result.PushAccountAdjustment(
          ::openpit::accountadjustment::AccountOutcomeEntry(
              ::openpit::param::Asset("EUR")));
    }
    PushReject(decision, Reject("DropCopyGatedRejectPolicy", RejectScope::Order,
                                RejectCode::Other, "forced business reject",
                                "returned in both modes"));
  }

 private:
  DropCopyGatedState* m_state;
};

[[nodiscard]] openpit::model::Order MakeDryRunHookOrder() {
  openpit::model::Order order;
  openpit::model::OrderOperation op;
  op.instrument = openpit::model::Instrument(::openpit::param::Asset("AAPL"),
                                             ::openpit::param::Asset("USD"));
  op.accountId = ::openpit::param::AccountId::FromUint64(42);
  op.side = openpit::model::Side::Buy;
  op.tradeAmount =
      openpit::model::TradeAmount::OfQuantity(Quantity::FromString("1"));
  op.price = Price::FromString("100");
  order.operation = std::move(op);
  return order;
}

// Applies a drop copy and commits an accepted operation, so the caller can
// assert on state that must survive finalization. A rejected result is returned
// untouched.
[[nodiscard]] openpit::pretrade::DropCopyResult ApplyDropCopyAndCommit(
    const openpit::Engine& engine, const openpit::Order& order) {
  openpit::pretrade::DropCopyResult result = engine.ApplyDropCopy(order);
  if (result.Passed()) {
    result.operation->Commit();
  }
  return result;
}

TEST(CustomPolicy, UsesExplicitDryRunHooksForDryRunPipeline) {
  DryRunHookCounters counters;
  openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
  CustomPolicy<DryRunHookPolicy> policy("DryRunHookPolicy",
                                        DryRunHookPolicy{&counters});
  builder.Add(policy);
  openpit::Engine engine = builder.Build();

  const openpit::pretrade::DryRunReport probe =
      engine.ExecutePreTradeDryRun(MakeDryRunHookOrder());
  EXPECT_TRUE(probe.Passed());
  EXPECT_TRUE(probe.Rejects().empty());
  EXPECT_TRUE(probe.Reviews().empty());
  EXPECT_EQ(counters.startDryRun, 1u);
  EXPECT_EQ(counters.mainDryRun, 1u);
  EXPECT_EQ(counters.start, 0u);
  EXPECT_EQ(counters.main, 0u);

  const openpit::pretrade::ExecuteResult result =
      engine.ExecutePreTrade(MakeDryRunHookOrder());
  EXPECT_FALSE(result.Passed());
  ASSERT_EQ(result.rejects.size(), 1u);
  EXPECT_EQ(result.rejects.front().code, RejectCode::Custom);
  EXPECT_EQ(result.rejects.front().mode, RuleMode::Reject);
  EXPECT_EQ(counters.start, 1u);
  EXPECT_EQ(counters.main, 1u);
}

struct ReviewMutationState {
  int start = 0;
  int main = 0;
  int commits = 0;
  int rollbacks = 0;
  bool failRollback = false;
};

struct ReviewPolicy {
  std::optional<RuleMode> startMode;
  std::optional<RuleMode> mainMode;
  ReviewMutationState* state = nullptr;
  const ReviewMutationState* previous = nullptr;
  int expectedPreviousRollbacks = 0;
  RejectScope scope = RejectScope::Order;
  RejectCode code = RejectCode::Custom;
  const char* policyName = "ReviewPolicy";
  const char* reason = "review order";
  const char* price = "100";
  const char* asset = "USD";

  [[nodiscard]] std::optional<Reject> CheckPreTradeStart(
      const openpit::Order&) const {
    if (state) {
      ++state->start;
    }
    if (!startMode) {
      return std::nullopt;
    }
    Reject mark(policyName, scope, code, reason, "start stage");
    mark.mode = *startMode;
    return mark;
  }

  [[nodiscard]] std::optional<Reject> CheckPreTradeStartDryRun(
      const openpit::Order& order) const {
    return CheckPreTradeStart(order);
  }

  void PerformPreTradeCheck(const Context&, openpit::tx::Mutations& mutations,
                            openpit::pretrade::Result& result,
                            PolicyDecision& decision) const {
    if (previous) {
      EXPECT_EQ(previous->rollbacks, expectedPreviousRollbacks);
    }
    if (state) {
      ++state->main;
      mutations.Push([value = state] { ++value->commits; },
                     [value = state] {
                       ++value->rollbacks;
                       if (value->failRollback) {
                         throw std::runtime_error("review rollback exploded");
                       }
                     });
    }
    result.PushLockPrice(Price::FromString(price));
    result.PushAccountAdjustment(
        openpit::accountadjustment::AccountOutcomeEntry(
            openpit::param::Asset(asset)));
    if (mainMode) {
      Reject mark(policyName, scope, code, reason, "main stage");
      mark.mode = *mainMode;
      decision.Push(std::move(mark));
    }
  }

  void PerformPreTradeCheckDryRun(const Context& context,
                                  openpit::tx::Mutations& mutations,
                                  openpit::pretrade::Result& result,
                                  PolicyDecision& decision) const {
    PerformPreTradeCheck(context, mutations, result, decision);
  }
};

TEST(CustomPolicy, StartReviewReachesRequestReservationAndDryRun) {
  openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
  builder.Add(CustomPolicy<ReviewPolicy>(
      "ReviewPolicy", ReviewPolicy{RuleMode::ReviewWithHold, std::nullopt}));
  const openpit::Engine engine = builder.Build();
  const auto order = MakeDryRunHookOrder();

  const auto dryRun = engine.StartPreTradeDryRun(order);
  ASSERT_TRUE(dryRun.Passed());
  ASSERT_EQ(dryRun.Reviews().size(), 1u);
  EXPECT_EQ(dryRun.Reviews().front().mode, RuleMode::ReviewWithHold);

  auto start = engine.StartPreTrade(order);
  ASSERT_TRUE(start.Passed());
  const auto startReviews = start.request->Reviews();
  ASSERT_EQ(startReviews.size(), 1u);
  EXPECT_EQ(startReviews.front().mode, RuleMode::ReviewWithHold);

  auto executed = start.request->Execute();
  ASSERT_TRUE(executed.Passed());
  const auto reviews = executed.reservation->Reviews();
  ASSERT_EQ(reviews.size(), 1u);
  EXPECT_EQ(reviews.front().mode, RuleMode::ReviewWithHold);
}

TEST(CustomPolicy, MainReviewPreservesPolicyResultOnlyWithHold) {
  for (const auto mode :
       {RuleMode::ReviewWithHold, RuleMode::ReviewWithoutHold}) {
    SCOPED_TRACE(static_cast<int>(mode));
    const bool holds = mode == RuleMode::ReviewWithHold;
    openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
    builder.Add(CustomPolicy<ReviewPolicy>("ReviewPolicy",
                                           ReviewPolicy{std::nullopt, mode}));
    const openpit::Engine engine = builder.Build();
    const auto order = MakeDryRunHookOrder();

    const auto report = engine.ExecutePreTradeDryRun(order);
    ASSERT_TRUE(report.Passed());
    const auto dryReviews = report.Reviews();
    ASSERT_EQ(dryReviews.size(), 1u);
    EXPECT_EQ(dryReviews.front().mode, mode);
    EXPECT_EQ(report.Lock().IsEmpty(), !holds);
    EXPECT_EQ(report.AccountAdjustments().empty(), !holds);

    auto executed = engine.ExecutePreTrade(order);
    ASSERT_TRUE(executed.Passed());
    const auto reviews = executed.reservation->Reviews();
    ASSERT_EQ(reviews.size(), 1u);
    EXPECT_EQ(reviews.front().mode, mode);
    EXPECT_EQ(executed.reservation->Lock().IsEmpty(), !holds);
    EXPECT_EQ(executed.reservation->AccountAdjustments().empty(), !holds);
  }
}

TEST(CustomPolicy, InvalidModeFailsLiveAndDryRun) {
  openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
  builder.Add(CustomPolicy<ReviewPolicy>(
      "ReviewPolicy", ReviewPolicy{std::nullopt, static_cast<RuleMode>(4)}));
  const openpit::Engine engine = builder.Build();
  const auto order = MakeDryRunHookOrder();

  EXPECT_THROW(static_cast<void>(engine.ExecutePreTradeDryRun(order)),
               std::invalid_argument);
  try {
    static_cast<void>(engine.ExecutePreTrade(order));
    FAIL() << "expected invalid_argument for the unknown reject mode";
  } catch (const std::invalid_argument& error) {
    EXPECT_STREQ(error.what(), "reject scope or mode is invalid");
  }
}

TEST(CustomPolicy, ReviewInstanceMainSegments) {
  for (const auto mode :
       {RuleMode::ReviewWithHold, RuleMode::ReviewWithoutHold}) {
    for (const bool recordMode : {false, true}) {
      SCOPED_TRACE(static_cast<int>(mode));
      SCOPED_TRACE(recordMode);
      ReviewMutationState review, keep;
      ReviewPolicy reviewing{std::nullopt,
                             recordMode ? mode : RuleMode::Reject};
      reviewing.state = &review;
      reviewing.price = "99";
      ReviewPolicy keeping{std::nullopt, std::nullopt};
      keeping.state = &keep;
      keeping.previous = &review;
      keeping.expectedPreviousRollbacks = recordMode ? 0 : 1;
      keeping.price = "101";
      keeping.asset = "EUR";
      openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
      builder.Add(CustomPolicy<ReviewPolicy>("ReviewPolicy", reviewing),
                  openpit::PolicyInstance{
                      "review", recordMode ? RuleMode::Reject : mode});
      builder.Add(CustomPolicy<ReviewPolicy>("ReviewPolicy", keeping),
                  openpit::PolicyInstance{"keep"});
      const auto engine = builder.Build();
      auto result = engine.ExecutePreTrade(MakeDryRunHookOrder());
      ASSERT_TRUE(result.Passed());
      const auto marks = result.reservation->Reviews();
      ASSERT_EQ(marks.size(), 1u);
      EXPECT_EQ(marks.front().mode, mode);
      EXPECT_EQ(marks.front().policy, "ReviewPolicy");
      const bool holds = mode == RuleMode::ReviewWithHold;
      const auto prices = result.reservation->Lock().Prices();
      const std::size_t expected = holds ? (recordMode ? 2u : 1u) : 0u;
      ASSERT_EQ(prices.size(), expected);
      EXPECT_EQ(result.reservation->AccountAdjustments().size(), expected);
      if (holds) {
        const auto outcomes = result.reservation->AccountAdjustments();
        ASSERT_EQ(outcomes.size(), expected);
        std::vector<openpit::param::Asset> assets;
        for (const auto& outcome : outcomes) {
          assets.push_back(outcome.entry.asset);
        }
        if (recordMode) {
          EXPECT_THAT(prices,
                      testing::UnorderedElementsAre(Price::FromString("99"),
                                                    Price::FromString("101")));
          EXPECT_THAT(assets, testing::UnorderedElementsAre(
                                  openpit::param::Asset("USD"),
                                  openpit::param::Asset("EUR")));
        } else {
          EXPECT_THAT(prices,
                      testing::UnorderedElementsAre(Price::FromString("101")));
          EXPECT_THAT(assets, testing::UnorderedElementsAre(
                                  openpit::param::Asset("EUR")));
        }
      }
      result.reservation->Commit();
      EXPECT_EQ(keep.commits, holds ? 1 : 0);
      EXPECT_EQ(keep.rollbacks, holds ? 0 : 1);
      EXPECT_EQ(review.commits, holds && recordMode ? 1 : 0);
      EXPECT_EQ(review.rollbacks, holds && recordMode ? 0 : 1);
    }
  }
}

TEST(CustomPolicy, ReviewInstanceWithoutHoldWins) {
  for (const bool across : {false, true}) {
    SCOPED_TRACE(across);
    ReviewMutationState with, without, keep;
    ReviewPolicy withPolicy{
        across ? std::optional<RuleMode>(RuleMode::Reject) : std::nullopt,
        across ? std::nullopt : std::optional<RuleMode>(RuleMode::Reject)};
    withPolicy.state = &with;
    ReviewPolicy withoutPolicy{std::nullopt, RuleMode::Reject};
    withoutPolicy.state = &without;
    ReviewPolicy keepPolicy{std::nullopt, std::nullopt};
    keepPolicy.state = &keep;
    openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
    builder.Add(CustomPolicy<ReviewPolicy>("ReviewPolicy", withPolicy),
                openpit::PolicyInstance{"with", RuleMode::ReviewWithHold});
    builder.Add(
        CustomPolicy<ReviewPolicy>("ReviewPolicy", withoutPolicy),
        openpit::PolicyInstance{"without", RuleMode::ReviewWithoutHold});
    builder.Add(CustomPolicy<ReviewPolicy>("ReviewPolicy", keepPolicy),
                openpit::PolicyInstance{"keep"});
    const auto engine = builder.Build();
    auto result = engine.ExecutePreTrade(MakeDryRunHookOrder());
    ASSERT_TRUE(result.Passed());
    const auto marks = result.reservation->Reviews();
    ASSERT_EQ(marks.size(), 2u);
    EXPECT_THAT((std::vector{marks.front().mode, marks.back().mode}),
                testing::UnorderedElementsAre(RuleMode::ReviewWithHold,
                                              RuleMode::ReviewWithoutHold));
    EXPECT_TRUE(result.reservation->Lock().IsEmpty());
    EXPECT_TRUE(result.reservation->AccountAdjustments().empty());
    result.reservation->Commit();
    EXPECT_EQ(keep.commits, 0);
    EXPECT_EQ(keep.rollbacks, 1);
    EXPECT_EQ(with.commits, 0);
    EXPECT_EQ(without.commits, 0);
    EXPECT_EQ(with.rollbacks, 1);
    EXPECT_EQ(without.rollbacks, 1);
  }
}

TEST(CustomPolicy, ReviewInstanceEffectiveRejectRollsBack) {
  ReviewMutationState review, deny;
  ReviewPolicy reviewPolicy{std::nullopt, RuleMode::Reject};
  reviewPolicy.state = &review;
  ReviewPolicy denyPolicy{std::nullopt, RuleMode::Reject};
  denyPolicy.state = &deny;
  openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
  builder.Add(CustomPolicy<ReviewPolicy>("ReviewPolicy", reviewPolicy),
              openpit::PolicyInstance{"review", RuleMode::ReviewWithHold});
  builder.Add(CustomPolicy<ReviewPolicy>("ReviewPolicy", denyPolicy),
              openpit::PolicyInstance{"deny"});
  const auto engine = builder.Build();
  const auto result = engine.ExecutePreTrade(MakeDryRunHookOrder());
  EXPECT_FALSE(result.Passed());
  EXPECT_FALSE(result.reservation.has_value());
  ASSERT_EQ(result.rejects.size(), 1u);
  EXPECT_EQ(result.rejects.front().mode, RuleMode::Reject);
  EXPECT_EQ(result.rejects.front().code, RejectCode::Custom);
  const auto report = engine.ExecutePreTradeDryRun(MakeDryRunHookOrder());
  EXPECT_TRUE(report.Reviews().empty());
  ASSERT_EQ(report.Rejects().size(), 1u);
  EXPECT_EQ(report.Rejects().front().mode, RuleMode::Reject);
  EXPECT_EQ(review.rollbacks, 1);
  EXPECT_EQ(deny.rollbacks, 1);
  EXPECT_EQ(review.commits + deny.commits, 0);
}

TEST(CustomPolicy, ReviewInstanceDryRunAccountMark) {
  for (const auto mode : {RuleMode::Reject, RuleMode::ReviewWithHold,
                          RuleMode::ReviewWithoutHold}) {
    SCOPED_TRACE(static_cast<int>(mode));
    ReviewPolicy policy{std::nullopt, RuleMode::Reject};
    policy.scope = RejectScope::Account;
    openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
    builder.Add(CustomPolicy<ReviewPolicy>("ReviewPolicy", policy),
                openpit::PolicyInstance{"review", mode});
    const auto engine = builder.Build();
    const auto report = engine.ExecutePreTradeDryRun(MakeDryRunHookOrder());
    EXPECT_EQ(report.Passed(), mode != RuleMode::Reject);
    EXPECT_EQ(report.AccountBlock().has_value(), mode == RuleMode::Reject);
    const auto marks = report.Reviews();
    if (mode == RuleMode::Reject) {
      EXPECT_TRUE(marks.empty());
    } else {
      ASSERT_EQ(marks.size(), 1u);
      EXPECT_EQ(marks.front().scope, RejectScope::Account);
      EXPECT_EQ(marks.front().mode, mode);
      EXPECT_TRUE(report.Rejects().empty());
    }
  }
}

openpit::pretrade::ExecuteResult ExecuteReviewOrder(
    const openpit::Engine& engine, bool viaRequest) {
  if (!viaRequest) {
    return engine.ExecutePreTrade(MakeDryRunHookOrder());
  }
  auto start = engine.StartPreTrade(MakeDryRunHookOrder());
  EXPECT_TRUE(start.Passed());
  return start.request.value().Execute();
}

TEST(CustomPolicy, ReviewInstanceRollbackFailure) {
  for (const auto [rollbackAll, viaRequest] :
       {std::pair{false, false}, std::pair{false, true}, std::pair{true, false},
        std::pair{true, true}}) {
    SCOPED_TRACE(testing::Message() << "rollbackAll=" << rollbackAll
                                    << " viaRequest=" << viaRequest);
    ReviewMutationState review, later;
    review.failRollback = true;
    ReviewPolicy policy{std::nullopt, rollbackAll ? RuleMode::ReviewWithoutHold
                                                  : RuleMode::Reject};
    policy.state = &review;
    ReviewPolicy laterPolicy{std::nullopt, std::nullopt};
    laterPolicy.state = &later;
    openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
    builder.Add(CustomPolicy<ReviewPolicy>("ReviewPolicy", policy),
                openpit::PolicyInstance{
                    "review",
                    rollbackAll ? RuleMode::Reject : RuleMode::ReviewWithHold});
    builder.Add(CustomPolicy<ReviewPolicy>("ReviewPolicy", laterPolicy),
                openpit::PolicyInstance{"later"});
    const auto engine = builder.Build();
    EXPECT_NO_THROW({
      const auto result = ExecuteReviewOrder(engine, viaRequest);
      EXPECT_FALSE(result.Passed());
      EXPECT_FALSE(result.reservation.has_value());
      ASSERT_EQ(result.rejects.size(), 1u);
      EXPECT_EQ(result.rejects.front().policy, "Engine");
      EXPECT_EQ(result.rejects.front().scope, RejectScope::Order);
      EXPECT_EQ(result.rejects.front().code, RejectCode::SystemUnavailable);
      EXPECT_EQ(result.rejects.front().reason, "review rollback failed");
      EXPECT_EQ(result.rejects.front().mode, RuleMode::Reject);
    });
    EXPECT_EQ(review.rollbacks, 1);
    EXPECT_EQ(later.main, 1);
    auto order = MakeDryRunHookOrder();
    order.operation->accountId = openpit::param::AccountId::FromUint64(43);
    const auto blocked = engine.StartPreTrade(order);
    EXPECT_FALSE(blocked.Passed());
    ASSERT_EQ(blocked.rejects.size(), 1u);
    EXPECT_EQ(blocked.rejects.front().code, RejectCode::SystemUnavailable);
    EXPECT_EQ(blocked.rejects.front().policy, "Engine");
    EXPECT_EQ(blocked.rejects.front().reason, "mutation finalizer failed");
    EXPECT_EQ(blocked.rejects.front().mode, RuleMode::Reject);
    EXPECT_EQ(review.rollbacks, 1);
    EXPECT_EQ(review.commits, 0);
    EXPECT_EQ(later.rollbacks, 1);
    EXPECT_EQ(later.commits, 0);
  }
}

struct ReviewCallbackError : std::logic_error {
  using std::logic_error::logic_error;
};

struct ThrowingReviewPolicy : ReviewPolicy {
  void PerformPreTradeCheck(const Context&, openpit::tx::Mutations&,
                            openpit::pretrade::Result&, PolicyDecision&) const {
    throw ReviewCallbackError("policy callback failed");
  }
};

TEST(CustomPolicy, EarlierPolicyExceptionWinsOverReviewRollbackFailure) {
  for (const bool viaRequest : {false, true}) {
    SCOPED_TRACE(viaRequest);
    ReviewMutationState review;
    review.failRollback = true;
    ReviewPolicy policy{std::nullopt, RuleMode::Reject};
    policy.state = &review;
    openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
    builder.Add(CustomPolicy<ThrowingReviewPolicy>("ThrowingReviewPolicy",
                                                   ThrowingReviewPolicy{}));
    builder.Add(CustomPolicy<ReviewPolicy>("ReviewPolicy", policy),
                openpit::PolicyInstance{"review", RuleMode::ReviewWithHold});
    const auto engine = builder.Build();
    EXPECT_THROW(static_cast<void>(ExecuteReviewOrder(engine, viaRequest)),
                 ReviewCallbackError);
    EXPECT_EQ(review.rollbacks, 1);
  }
}

TEST(CustomPolicy, LaterPolicyExceptionWinsOverReviewRollbackFailure) {
  for (const bool viaRequest : {false, true}) {
    SCOPED_TRACE(viaRequest);
    ReviewMutationState review;
    review.failRollback = true;
    ReviewPolicy policy{std::nullopt, RuleMode::Reject};
    policy.state = &review;
    openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
    builder.Add(CustomPolicy<ReviewPolicy>("ReviewPolicy", policy),
                openpit::PolicyInstance{"review", RuleMode::ReviewWithHold});
    builder.Add(CustomPolicy<ThrowingReviewPolicy>("ThrowingReviewPolicy",
                                                   ThrowingReviewPolicy{}));
    const auto engine = builder.Build();
    EXPECT_THROW(static_cast<void>(ExecuteReviewOrder(engine, viaRequest)),
                 ReviewCallbackError);
    EXPECT_EQ(review.rollbacks, 1);
  }
}

TEST(CustomPolicy, OrdinaryRejectRollbackFailureKeepsException) {
  for (const bool viaRequest : {false, true}) {
    SCOPED_TRACE(viaRequest);
    ReviewMutationState state;
    state.failRollback = true;
    ReviewPolicy policy{std::nullopt, RuleMode::Reject};
    policy.state = &state;
    openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
    builder.Add(CustomPolicy<ReviewPolicy>("ReviewPolicy", policy));
    const auto engine = builder.Build();
    try {
      static_cast<void>(ExecuteReviewOrder(engine, viaRequest));
      FAIL() << "expected the ordinary rollback exception";
    } catch (const std::runtime_error& error) {
      EXPECT_STREQ(error.what(), "review rollback exploded");
    }
    EXPECT_EQ(state.rollbacks, 1);
    auto order = MakeDryRunHookOrder();
    order.operation->accountId = openpit::param::AccountId::FromUint64(43);
    const auto blocked = engine.StartPreTrade(order);
    ASSERT_EQ(blocked.rejects.size(), 1u);
    EXPECT_EQ(blocked.rejects.front().policy, "Engine");
    EXPECT_EQ(blocked.rejects.front().code, RejectCode::SystemUnavailable);
    EXPECT_EQ(blocked.rejects.front().reason, "mutation finalizer failed");
  }
}

TEST(CustomPolicy, NearReviewRollbackRejectKeepsException) {
  for (const auto& field : {"reason", "scope", "code"}) {
    SCOPED_TRACE(field);
    const std::string_view changedField(field);
    ReviewMutationState state;
    state.failRollback = true;
    ReviewPolicy policy{std::nullopt, RuleMode::Reject};
    policy.state = &state;
    policy.policyName = "Engine";
    policy.reason = changedField == "reason" ? "different rollback failure"
                                             : "review rollback failed";
    policy.scope =
        changedField == "scope" ? RejectScope::Account : RejectScope::Order;
    policy.code = changedField == "code" ? RejectCode::Custom
                                         : RejectCode::SystemUnavailable;
    openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
    builder.Add(CustomPolicy<ReviewPolicy>("Engine", policy));
    const auto engine = builder.Build();
    try {
      static_cast<void>(engine.ExecutePreTrade(MakeDryRunHookOrder()));
      FAIL() << "expected the near-identity rollback exception";
    } catch (const std::runtime_error& error) {
      EXPECT_STREQ(error.what(), "review rollback exploded");
    }
    EXPECT_EQ(state.rollbacks, 1);
  }
}

TEST(CustomPolicy, ReviewAndRejectRollbackFailuresReturnFullRejectList) {
  for (const bool viaRequest : {false, true}) {
    SCOPED_TRACE(viaRequest);
    ReviewMutationState review, later;
    review.failRollback = later.failRollback = true;
    ReviewPolicy policy{std::nullopt, RuleMode::Reject};
    policy.state = &review;
    ReviewPolicy laterPolicy{std::nullopt, RuleMode::Reject};
    laterPolicy.state = &later;
    openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
    builder.Add(CustomPolicy<ReviewPolicy>("ReviewPolicy", policy),
                openpit::PolicyInstance{"review", RuleMode::ReviewWithHold});
    builder.Add(CustomPolicy<ReviewPolicy>("ReviewPolicy", laterPolicy),
                openpit::PolicyInstance{"later"});
    const auto engine = builder.Build();
    EXPECT_NO_THROW({
      const auto result = ExecuteReviewOrder(engine, viaRequest);
      EXPECT_FALSE(result.Passed());
      EXPECT_FALSE(result.reservation.has_value());
      ASSERT_EQ(result.rejects.size(), 2u);
      EXPECT_EQ(result.rejects[0].policy, "Engine");
      EXPECT_EQ(result.rejects[0].scope, RejectScope::Order);
      EXPECT_EQ(result.rejects[0].code, RejectCode::SystemUnavailable);
      EXPECT_EQ(result.rejects[0].reason, "review rollback failed");
      EXPECT_EQ(result.rejects[0].mode, RuleMode::Reject);
      EXPECT_EQ(result.rejects[1].policy, "ReviewPolicy");
      EXPECT_EQ(result.rejects[1].scope, RejectScope::Order);
      EXPECT_EQ(result.rejects[1].code, RejectCode::Custom);
      EXPECT_EQ(result.rejects[1].reason, "review order");
      EXPECT_EQ(result.rejects[1].mode, RuleMode::Reject);
    });
    EXPECT_EQ(review.rollbacks, 1);
    EXPECT_EQ(later.rollbacks, 1);
  }
}

TEST(CustomPolicy, ReviewInstanceDropCopy) {
  for (const auto mode : {RuleMode::Reject, RuleMode::ReviewWithHold,
                          RuleMode::ReviewWithoutHold}) {
    for (const bool fatal : {false, true}) {
      SCOPED_TRACE(static_cast<int>(mode));
      SCOPED_TRACE(fatal);
      ReviewMutationState state;
      ReviewPolicy policy{std::nullopt, mode == RuleMode::Reject
                                            ? RuleMode::ReviewWithHold
                                            : RuleMode::Reject};
      policy.state = &state;
      policy.scope = RejectScope::Account;
      if (fatal) {
        policy.code = RejectCode::OrderValueCalculationFailed;
      }
      openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
      builder.Add(CustomPolicy<ReviewPolicy>("ReviewPolicy", policy),
                  openpit::PolicyInstance{"review", mode});
      const auto engine = builder.Build();
      auto result = engine.ApplyDropCopy(MakeDryRunHookOrder());
      if (fatal) {
        EXPECT_FALSE(result.operation.has_value());
        ASSERT_EQ(result.rejects.size(), 1u);
        EXPECT_EQ(result.rejects.front().code,
                  RejectCode::OrderValueCalculationFailed);
        EXPECT_EQ(result.rejects.front().mode, RuleMode::Reject);
        EXPECT_EQ(state.rollbacks, 1);
        EXPECT_EQ(state.commits, 0);
      } else {
        ASSERT_TRUE(result.operation.has_value());
        EXPECT_TRUE(result.rejects.empty());
        EXPECT_FALSE(result.operation->AccountBlock().has_value());
        EXPECT_FALSE(result.operation->IsAccountBlocked());
        const auto prices = result.operation->Lock().Prices();
        ASSERT_EQ(prices.size(), 1u);
        EXPECT_EQ(prices.front(), Price::FromString(policy.price));
        const auto outcomes = result.operation->AccountAdjustments();
        ASSERT_EQ(outcomes.size(), 1u);
        EXPECT_EQ(outcomes.front().entry.asset,
                  openpit::param::Asset(policy.asset));
        result.operation->Commit();
        EXPECT_EQ(state.commits, 1);
        EXPECT_EQ(state.rollbacks, 0);
      }
    }
  }
}

TEST(CustomPolicy, ReviewInstanceBlockBetweenStages) {
  ReviewMutationState state;
  ReviewPolicy policy{RuleMode::Reject, std::nullopt};
  policy.state = &state;
  openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
  builder.Add(CustomPolicy<ReviewPolicy>("ReviewPolicy", policy),
              openpit::PolicyInstance{"review", RuleMode::ReviewWithoutHold});
  const auto engine = builder.Build();
  auto start = engine.StartPreTrade(MakeDryRunHookOrder());
  ASSERT_TRUE(start.Passed());
  const auto marks = start.request->Reviews();
  ASSERT_EQ(marks.size(), 1u);
  EXPECT_EQ(marks.front().code, RejectCode::Custom);
  EXPECT_EQ(marks.front().mode, RuleMode::ReviewWithoutHold);
  engine.Accounts().Block(openpit::param::AccountId::FromUint64(42),
                          "between stages");
  const auto result = start.request->Execute();
  EXPECT_FALSE(result.Passed());
  EXPECT_FALSE(result.reservation.has_value());
  ASSERT_EQ(result.rejects.size(), 1u);
  EXPECT_EQ(result.rejects.front().code, RejectCode::AccountBlocked);
  EXPECT_EQ(result.rejects.front().mode, RuleMode::Reject);
  EXPECT_EQ(state.main, 0);
}

TEST(CustomPolicy, ReviewInstanceConfirmedStartRunsMain) {
  ReviewMutationState state;
  ReviewPolicy policy{RuleMode::Reject, std::nullopt};
  policy.state = &state;
  openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
  builder.Add(CustomPolicy<ReviewPolicy>("ReviewPolicy", policy),
              openpit::PolicyInstance{"review", RuleMode::ReviewWithoutHold});
  const auto engine = builder.Build();
  auto order = MakeDryRunHookOrder();
  order.review = openpit::model::OrderReview{true, true};
  auto result = engine.ExecutePreTrade(order);
  ASSERT_TRUE(result.Passed());
  EXPECT_TRUE(result.reservation->Reviews().empty());
  EXPECT_FALSE(result.reservation->Lock().IsEmpty());
  ASSERT_EQ(result.reservation->AccountAdjustments().size(), 1u);
  result.reservation->Commit();
  EXPECT_EQ(state.start, 1);
  EXPECT_EQ(state.main, 1);
  EXPECT_EQ(state.commits, 1);
  EXPECT_EQ(state.rollbacks, 0);
}

TEST(CustomPolicy, UsesContextStartDryRunHookForDryRunPipeline) {
  DryRunHookCounters counters;
  openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
  CustomPolicy<ContextStartDryRunHookPolicy> policy(
      "ContextStartDryRunHookPolicy", ContextStartDryRunHookPolicy{&counters});
  builder.Add(policy);
  openpit::Engine engine = builder.Build();

  const openpit::pretrade::DryRunReport probe =
      engine.ExecutePreTradeDryRun(MakeDryRunHookOrder());
  EXPECT_TRUE(probe.Passed());
  EXPECT_EQ(counters.startDryRun, 1u);
  EXPECT_EQ(counters.start, 0u);

  const openpit::pretrade::ExecuteResult result =
      engine.ExecutePreTrade(MakeDryRunHookOrder());
  EXPECT_TRUE(result.Passed());
  EXPECT_EQ(counters.start, 1u);
  EXPECT_EQ(counters.startDryRun, 1u);
}

TEST(CustomPolicy, DropCopySurfacesAppliedAccountBlock) {
  openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
  CustomPolicy<DropCopyBlockingPolicy> policy("DropCopyBlockingPolicy",
                                              DropCopyBlockingPolicy{});
  builder.Add(policy);
  openpit::Engine engine = builder.Build();

  const openpit::pretrade::DropCopyResult result =
      ApplyDropCopyAndCommit(engine, MakeDryRunHookOrder());
  ASSERT_TRUE(result.Passed());
  const openpit::pretrade::DropCopyOperation& operation = *result.operation;
  const std::optional<openpit::accounts::AccountBlock> block =
      operation.AccountBlock();

  ASSERT_TRUE(block.has_value());
  EXPECT_EQ(block->policy, "DropCopyBlockingPolicy");
  EXPECT_EQ(block->code, RejectCode::AccountBlocked);
  EXPECT_EQ(block->reason, "forced account block");
  EXPECT_TRUE(operation.IsAccountBlocked());
  const std::vector<Price> prices = operation.Lock().Prices();
  ASSERT_EQ(prices.size(), 1u);
  EXPECT_EQ(prices.front().ToString(), "13");
  EXPECT_FALSE(engine.StartPreTrade(MakeDryRunHookOrder()).Passed());
}

TEST(CustomPolicy, DropCopyCommitPreservesAppliedState) {
  openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
  CustomPolicy<DropCopyBlockingPolicy> policy("DropCopyBlockingPolicy",
                                              DropCopyBlockingPolicy{});
  builder.Add(policy);
  openpit::Engine engine = builder.Build();

  const openpit::pretrade::DropCopyResult result =
      ApplyDropCopyAndCommit(engine, MakeDryRunHookOrder());

  ASSERT_TRUE(result.Passed());
  ASSERT_TRUE(result.operation->AccountBlock().has_value());
  EXPECT_FALSE(engine.StartPreTrade(MakeDryRunHookOrder()).Passed());
}

TEST(CustomPolicy, DropCopyReturnsNoBlockWhenNoneWasProduced) {
  openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
  CustomPolicy<AcceptingPolicy> policy("AcceptingPolicy", AcceptingPolicy{});
  builder.Add(policy);
  openpit::Engine engine = builder.Build();

  const openpit::pretrade::DropCopyResult result =
      ApplyDropCopyAndCommit(engine, MakeDryRunHookOrder());

  ASSERT_TRUE(result.Passed());
  EXPECT_FALSE(result.operation->AccountBlock().has_value());
}

TEST(CustomPolicy, DropCopyRunsOnPreBlockedAccountAndReportsFinalState) {
  openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
  CustomPolicy<AcceptingPolicy> policy("AcceptingPolicy", AcceptingPolicy{});
  builder.Add(policy);
  openpit::Engine engine = builder.Build();
  const auto account = openpit::param::AccountId::FromUint64(42);
  engine.Accounts().Block(account, "existing account block");

  const openpit::pretrade::DropCopyResult result =
      ApplyDropCopyAndCommit(engine, MakeDryRunHookOrder());

  ASSERT_TRUE(result.Passed());
  EXPECT_FALSE(result.operation->AccountBlock().has_value());
  EXPECT_TRUE(result.operation->IsAccountBlocked());
}

TEST(CustomPolicy, DropCopyRunsOnPreBlockedAccountGroup) {
  openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
  CustomPolicy<AcceptingPolicy> policy("AcceptingPolicy", AcceptingPolicy{});
  builder.Add(policy);
  openpit::Engine engine = builder.Build();
  const auto account = openpit::param::AccountId::FromUint64(42);
  const auto group = openpit::param::AccountGroupId::FromUint32(7);
  ASSERT_FALSE(engine.Accounts().RegisterGroup({account}, group).has_value());
  ASSERT_FALSE(
      engine.Accounts().BlockGroup(group, "existing group block").has_value());

  const openpit::pretrade::DropCopyResult result =
      ApplyDropCopyAndCommit(engine, MakeDryRunHookOrder());

  ASSERT_TRUE(result.Passed());
  EXPECT_FALSE(result.operation->AccountBlock().has_value());
  EXPECT_TRUE(result.operation->IsAccountBlocked());
}

TEST(CustomPolicy, DropCopyReportsGroupTaggedAdjustments) {
  openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
  CustomPolicy<DropCopyBlockingPolicy> policy("DropCopyBlockingPolicy",
                                              DropCopyBlockingPolicy{}, 7);
  builder.Add(policy);
  openpit::Engine engine = builder.Build();

  const openpit::pretrade::DropCopyResult result =
      ApplyDropCopyAndCommit(engine, MakeDryRunHookOrder());
  ASSERT_TRUE(result.Passed());
  const std::vector<::openpit::accountadjustment::Outcome> adjustments =
      result.operation->AccountAdjustments();

  ASSERT_EQ(adjustments.size(), 1u);
  EXPECT_EQ(adjustments.front().policyGroupId.Value(), 7u);
  EXPECT_EQ(adjustments.front().entry.asset.View(), "USD");
}

TEST(CustomPolicy, DropCopyMarketOrderRunsPriceIndependentPolicy) {
  openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
  CustomPolicy<AcceptingPolicy> policy("AcceptingPolicy", AcceptingPolicy{});
  builder.Add(policy);
  openpit::Engine engine = builder.Build();
  openpit::model::Order market = MakeDryRunHookOrder();
  market.operation->price.reset();

  const openpit::pretrade::DropCopyResult result =
      ApplyDropCopyAndCommit(engine, market);

  EXPECT_TRUE(result.Passed());
  EXPECT_TRUE(result.rejects.empty());
}

TEST(CustomPolicy, DropCopyOrderWithoutAccountRejectsBeforePolicies) {
  openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
  CustomPolicy<AcceptingPolicy> policy("AcceptingPolicy", AcceptingPolicy{});
  builder.Add(policy);
  openpit::Engine engine = builder.Build();
  openpit::model::Order anonymous = MakeDryRunHookOrder();
  anonymous.operation->accountId.reset();

  const openpit::pretrade::DropCopyResult result =
      ApplyDropCopyAndCommit(engine, anonymous);

  EXPECT_FALSE(result.Passed());
  EXPECT_FALSE(result.operation.has_value());
  ASSERT_EQ(result.rejects.size(), 1u);
  EXPECT_EQ(result.rejects.front().code, RejectCode::MissingRequiredField);
}

TEST(CustomPolicy, DropCopyReturnsMissingFieldAsBusinessReject) {
  openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
  CustomPolicy<MissingFieldPolicy> policy("MissingFieldPolicy",
                                          MissingFieldPolicy{});
  builder.Add(policy);
  openpit::Engine engine = builder.Build();

  const openpit::pretrade::DropCopyResult result =
      ApplyDropCopyAndCommit(engine, MakeDryRunHookOrder());

  EXPECT_FALSE(result.Passed());
  EXPECT_FALSE(result.operation.has_value());
  ASSERT_EQ(result.rejects.size(), 1u);
  EXPECT_EQ(result.rejects.front().code, RejectCode::MissingRequiredField);
}

TEST(CustomPolicy, DropCopyStartContextCommitsRegisteredMutation) {
  int value = 0;
  bool sawDropCopy = false;
  openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
  CustomPolicy<DropCopyStartMutationPolicy> policy(
      "DropCopyStartMutationPolicy",
      DropCopyStartMutationPolicy{&value, &sawDropCopy, false});
  builder.Add(policy);
  openpit::Engine engine = builder.Build();

  const openpit::pretrade::DropCopyResult result =
      ApplyDropCopyAndCommit(engine, MakeDryRunHookOrder());

  EXPECT_TRUE(result.Passed());
  EXPECT_TRUE(sawDropCopy);
  EXPECT_EQ(value, 1);
}

TEST(CustomPolicy, DropCopyOperationFinalizationIsIdempotent) {
  int value = 0;
  bool sawDropCopy = false;
  openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
  CustomPolicy<DropCopyStartMutationPolicy> policy(
      "DropCopyStartMutationPolicy",
      DropCopyStartMutationPolicy{&value, &sawDropCopy, false});
  builder.Add(policy);
  openpit::Engine engine = builder.Build();

  openpit::pretrade::DropCopyResult committed =
      engine.ApplyDropCopy(MakeDryRunHookOrder());
  ASSERT_TRUE(committed.Passed());
  EXPECT_EQ(value, 1);
  committed.operation->Commit();
  committed.operation->Commit();
  committed.operation->Rollback();
  EXPECT_EQ(value, 1);

  openpit::pretrade::DropCopyResult rolledBack =
      engine.ApplyDropCopy(MakeDryRunHookOrder());
  ASSERT_TRUE(rolledBack.Passed());
  EXPECT_EQ(value, 2);
  rolledBack.operation->Rollback();
  rolledBack.operation->Rollback();
  rolledBack.operation->Commit();
  EXPECT_EQ(value, 1);
}

TEST(CustomPolicy, DropCopyOperationDestructorRollsBackImplicitly) {
  int value = 0;
  bool sawDropCopy = false;
  openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
  CustomPolicy<DropCopyStartMutationPolicy> policy(
      "DropCopyStartMutationPolicy",
      DropCopyStartMutationPolicy{&value, &sawDropCopy, false});
  builder.Add(policy);
  openpit::Engine engine = builder.Build();

  {
    const openpit::pretrade::DropCopyResult result =
        engine.ApplyDropCopy(MakeDryRunHookOrder());
    EXPECT_TRUE(result.Passed());
    EXPECT_EQ(value, 1);
  }

  EXPECT_EQ(value, 0);
}

TEST(CustomPolicy, DropCopyStartContextRollsBackRegisteredMutation) {
  int value = 0;
  bool sawDropCopy = false;
  openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
  CustomPolicy<DropCopyStartMutationPolicy> policy(
      "DropCopyStartMutationPolicy",
      DropCopyStartMutationPolicy{&value, &sawDropCopy, true});
  builder.Add(policy);
  openpit::Engine engine = builder.Build();

  const openpit::pretrade::DropCopyResult result =
      ApplyDropCopyAndCommit(engine, MakeDryRunHookOrder());

  EXPECT_FALSE(result.Passed());
  EXPECT_TRUE(sawDropCopy);
  EXPECT_EQ(value, 0);
}

TEST(CustomPolicy, DropCopyKeepsGatedBookkeepingUnderBusinessReject) {
  DropCopyGatedState state;
  openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
  CustomPolicy<DropCopyGatedRejectPolicy> policy(
      "DropCopyGatedRejectPolicy", DropCopyGatedRejectPolicy{&state});
  builder.Add(policy);
  openpit::Engine engine = builder.Build();

  const openpit::pretrade::ExecuteResult ordinary =
      engine.ExecutePreTrade(MakeDryRunHookOrder());
  EXPECT_FALSE(ordinary.Passed());
  ASSERT_EQ(ordinary.rejects.size(), 1u);
  EXPECT_EQ(ordinary.rejects.front().code, RejectCode::Other);
  EXPECT_EQ(state.startModes, std::vector<bool>{false});
  EXPECT_EQ(state.mainModes, std::vector<bool>{false});
  EXPECT_EQ(state.commits, 0);
  EXPECT_EQ(state.rollbacks, 0);

  openpit::pretrade::DropCopyResult result =
      engine.ApplyDropCopy(MakeDryRunHookOrder());
  ASSERT_TRUE(result.Passed());
  EXPECT_EQ(state.startModes, (std::vector<bool>{false, true}));
  EXPECT_EQ(state.mainModes, (std::vector<bool>{false, true}));
  const std::vector<::openpit::accountadjustment::Outcome> adjustments =
      result.operation->AccountAdjustments();
  ASSERT_EQ(adjustments.size(), 1u);
  EXPECT_EQ(adjustments.front().entry.asset.View(), "EUR");
  result.operation->Commit();
  EXPECT_EQ(state.commits, 1);
  EXPECT_EQ(state.rollbacks, 0);
}

//------------------------------------------------------------------------------
// Built-in policy configuration construction + registration.

openpit::model::ExecutionReport SpotFundsReport(
    openpit::param::AccountId account) {
  openpit::model::ExecutionReport report;
  auto& operation = report.operation.emplace();
  operation.instrument = openpit::model::Instrument(
      openpit::param::Asset("AAPL"), openpit::param::Asset("USD"));
  operation.accountId = account;
  operation.side = openpit::model::Side::Buy;
  return report;
}

openpit::accountadjustment::AccountOutcomeEntry AdjustSpotFundsBalances(
    const openpit::Engine& engine, openpit::param::AccountId account,
    std::string_view asset, std::string_view balanceDelta) {
  openpit::accountadjustment::BalanceOperation balance;
  balance.asset = openpit::param::Asset(asset);
  openpit::accountadjustment::AccountAdjustment adjustment;
  adjustment.operation =
      openpit::accountadjustment::Operation::OfBalance(std::move(balance));
  openpit::accountadjustment::Amount amount;
  amount.balance = openpit::param::AdjustmentAmount::Delta(
      openpit::param::PositionSize::FromString(balanceDelta));
  amount.held = openpit::param::AdjustmentAmount::Delta(
      openpit::param::PositionSize::FromString("0"));
  amount.incoming = amount.held;
  adjustment.amount = std::move(amount);
  const auto result = engine.ApplyAccountAdjustment(
      account,
      std::vector<openpit::accountadjustment::AccountAdjustment>{adjustment});
  EXPECT_TRUE(result.Passed());
  EXPECT_EQ(result.accountAdjustmentOutcomes.size(), 1U);
  return result.accountAdjustmentOutcomes.at(0).entry;
}

struct SpotFundsAssetBalances {
  std::string balance;
  std::string held;
  std::string incoming;

  bool operator==(const SpotFundsAssetBalances& other) const {
    return balance == other.balance && held == other.held &&
           incoming == other.incoming;
  }
};

struct SpotFundsBalanceSnapshot {
  SpotFundsAssetBalances usd;
  SpotFundsAssetBalances aapl;

  bool operator==(const SpotFundsBalanceSnapshot& other) const {
    return usd == other.usd && aapl == other.aapl;
  }
};

SpotFundsBalanceSnapshot SpotFundsBalances(const openpit::Engine& engine,
                                           openpit::param::AccountId account) {
  const auto amounts = [&](std::string_view asset) {
    const auto entry = AdjustSpotFundsBalances(engine, account, asset, "0");
    return SpotFundsAssetBalances{entry.balance.value().absolute.ToString(),
                                  entry.held.value().absolute.ToString(),
                                  entry.incoming.value().absolute.ToString()};
  };
  return {amounts("USD"), amounts("AAPL")};
}

openpit::model::ExecutionReport SpotFundsCancelReport(
    openpit::param::AccountId account, Quantity remaining, PreTradeLock lock) {
  auto report = SpotFundsReport(account);
  auto& fill = report.fill.emplace();
  fill.lock = std::move(lock);
  fill.remainingReservedQuantity = remaining;
  fill.isFinal = true;
  return report;
}

[[nodiscard]] openpit::model::Order SpotFundsLifecycleOrder(
    const openpit::param::AccountId accountId,
    const Quantity quantity = Quantity::FromString("1")) {
  return openpit::model::Order::Limit(
      openpit::model::Instrument(::openpit::param::Asset("AAPL"),
                                 ::openpit::param::Asset("USD")),
      accountId, openpit::model::Side::Buy,
      openpit::model::TradeAmount::OfQuantity(quantity),
      Price::FromString("100"));
}

void SeedSpotFundsLifecycleAccount(const openpit::Engine& engine,
                                   const openpit::param::AccountId accountId) {
  engine.Accounts().SetCurrency(accountId, openpit::param::Asset("USD"));

  openpit::accountadjustment::AccountAdjustment seed;
  openpit::accountadjustment::BalanceOperation balance;
  balance.asset = ::openpit::param::Asset("USD");
  seed.operation =
      openpit::accountadjustment::Operation::OfBalance(std::move(balance));
  openpit::accountadjustment::Amount amount;
  amount.balance = openpit::param::AdjustmentAmount::Absolute(
      openpit::param::PositionSize::FromString("1000"));
  seed.amount = std::move(amount);

  const openpit::AdjustmentResult result = engine.ApplyAccountAdjustment(
      accountId,
      std::vector<openpit::accountadjustment::AccountAdjustment>{seed});
  ASSERT_TRUE(result.Passed());
}

[[nodiscard]] openpit::model::Order SpotFundsSellLifecycleOrder(
    const openpit::param::AccountId accountId) {
  return openpit::model::Order::Limit(
      openpit::model::Instrument(::openpit::param::Asset("AAPL"),
                                 ::openpit::param::Asset("USD")),
      accountId, openpit::model::Side::Sell,
      openpit::model::TradeAmount::OfQuantity(Quantity::FromString("2")),
      Price::FromString("150"));
}

[[nodiscard]] std::vector<openpit::accounts::AccountBlock>
ApplySpotFundsLifecycleFill(const openpit::Engine& engine,
                            const openpit::param::AccountId accountId) {
  openpit::pretrade::ExecuteResult execution =
      engine.ExecutePreTrade(SpotFundsLifecycleOrder(accountId));
  if (!execution.Passed()) {
    ADD_FAILURE() << "ExecutePreTrade() rejects = " << execution.rejects.size()
                  << ", want none";
    return {};
  }

  // Adopt the reservation lock into the public fill model.
  openpit::pretrade::PreTradeLock lock = execution.reservation->Lock();
  execution.reservation->Commit();

  openpit::model::Fill fill;
  fill.lock = std::move(lock);
  fill.lastTrade = openpit::model::Trade(Price::FromString("100"),
                                         Quantity::FromString("1"));
  fill.remainingReservedQuantity = Quantity::FromString("0");
  fill.isFinal = true;

  auto report = SpotFundsReport(accountId);
  report.fill = std::move(fill);
  return engine.ApplyExecutionReport(report).accountBlocks;
}

struct SpotFundsBlockedFillSetup {
  openpit::Engine engine;
  openpit::model::ExecutionReportOperation operation;
};

[[nodiscard]] std::optional<SpotFundsBlockedFillSetup>
MakeSpotFundsBlockedFillSetup() {
  const openpit::param::AccountId account =
      openpit::param::AccountId::FromUint64(83023);
  openpit::EngineBuilder builder(openpit::SyncPolicy::None);
  builder.Add(policies::SpotFundsPolicy{});
  openpit::Engine engine = builder.Build();
  engine.Accounts().SetCurrency(account, openpit::param::Asset("USD"));

  openpit::accountadjustment::AccountAdjustment seed;
  openpit::accountadjustment::BalanceOperation balance;
  balance.asset = ::openpit::param::Asset("AAPL");
  seed.operation =
      openpit::accountadjustment::Operation::OfBalance(std::move(balance));
  openpit::accountadjustment::Amount amount;
  amount.balance = openpit::param::AdjustmentAmount::Absolute(
      openpit::param::PositionSize::FromString("5"));
  seed.amount = std::move(amount);
  const openpit::AdjustmentResult adjustment = engine.ApplyAccountAdjustment(
      account,
      std::vector<openpit::accountadjustment::AccountAdjustment>{seed});
  if (!adjustment.Passed()) {
    ADD_FAILURE() << "ApplyAccountAdjustment() failed";
    return std::nullopt;
  }

  openpit::pretrade::ExecuteResult execution =
      engine.ExecutePreTrade(SpotFundsSellLifecycleOrder(account));
  if (!execution.Passed()) {
    ADD_FAILURE() << "ExecutePreTrade() rejects = " << execution.rejects.size()
                  << ", want none";
    return std::nullopt;
  }
  execution.reservation->Commit();

  openpit::model::ExecutionReportOperation operation;
  operation.instrument = openpit::model::Instrument(
      ::openpit::param::Asset("AAPL"), ::openpit::param::Asset("USD"));
  operation.accountId = account;
  operation.side = openpit::model::Side::Sell;
  return SpotFundsBlockedFillSetup{std::move(engine), std::move(operation)};
}

void ExpectSpotFundsPnlPreTradeReject(
    const openpit::Engine& engine, const openpit::param::AccountId accountId) {
  const openpit::pretrade::ExecuteResult execution =
      engine.ExecutePreTrade(SpotFundsLifecycleOrder(accountId));
  EXPECT_FALSE(execution.Passed());
  ASSERT_EQ(execution.rejects.size(), 1u);
  EXPECT_EQ(execution.rejects.front().code,
            openpit::reject::RejectCode::PnlKillSwitchTriggered);
}

TEST(BuiltinPolicy, OrderValidationRegistersAndBuilds) {
  openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
  builder.Add(policies::OrderValidationPolicy{});
  EXPECT_NO_THROW({ openpit::Engine engine = builder.Build(); });
}

TEST(BuiltinPolicy, OrderSizeLimitBrokerBarrierBuilds) {
  openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
  policies::OrderSizeLimitPolicy config;
  config.BrokerBarrier(
      policies::OrderSizeBrokerBarrier(policies::OrderSizeLimit::Both(
          Quantity::FromString("100"), Volume::FromString("1000000"))));
  config.AddTo(builder);
  EXPECT_NO_THROW({ openpit::Engine engine = builder.Build(); });
}

TEST(BuiltinPolicy, InstanceModeControlsOrderSizeLimit) {
  policies::OrderSizeLimitPolicy config;
  config.BrokerBarrier(policies::OrderSizeBrokerBarrier(
      policies::OrderSizeLimit::Quantity(Quantity::FromString("0"))));
  const auto order = MakeDryRunHookOrder();
  {
    openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
    builder.Add(config);
    const auto engine = builder.Build();
    const auto start = engine.StartPreTrade(order);
    EXPECT_FALSE(start.Passed());
    ASSERT_EQ(start.rejects.size(), 1u);
    EXPECT_EQ(start.rejects.front().mode, RuleMode::Reject);
  }
  for (const auto mode :
       {RuleMode::ReviewWithHold, RuleMode::ReviewWithoutHold}) {
    policies::OrderSizeLimitPolicy reviewConfig;
    reviewConfig.BrokerBarrier(policies::OrderSizeBrokerBarrier(
        policies::OrderSizeLimit::Quantity(Quantity::FromString("100"))));
    openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
    builder.Add(config, openpit::PolicyInstance{"size-reject"});
    builder.Add(reviewConfig, openpit::PolicyInstance{"size-review", mode});
    const auto engine = builder.Build();
    using BrokerUpdate = policies::OrderSizeBrokerBarrierUpdate;
    const auto barrier = [](const char* quantity) {
      return policies::OrderSizeBrokerBarrier(
          policies::OrderSizeLimit::Quantity(Quantity::FromString(quantity)));
    };
    engine.Configure().OrderSizeLimit("size-reject",
                                      BrokerUpdate::Set(barrier("100")));
    engine.Configure().OrderSizeLimit("size-review",
                                      BrokerUpdate::Set(barrier("0")));
    const auto start = engine.StartPreTrade(order);
    ASSERT_TRUE(start.Passed());
    ASSERT_TRUE(start.request);
    const auto reviews = start.request->Reviews();
    ASSERT_EQ(reviews.size(), 1u);
    EXPECT_EQ(reviews.front().mode, mode);
    EXPECT_EQ(reviews.front().code, RejectCode::OrderQtyExceedsLimit);
    EXPECT_EQ(reviews.front().scope, RejectScope::Order);
    EXPECT_EQ(reviews.front().policy, "OrderSizeLimitPolicy");
    auto confirmedOrder = order;
    confirmedOrder.review = openpit::model::OrderReview{true, std::nullopt};
    const auto confirmed = engine.ExecutePreTrade(confirmedOrder);
    ASSERT_TRUE(confirmed.Passed());
    EXPECT_TRUE(confirmed.reservation->Reviews().empty());
    engine.Configure().OrderSizeLimit("size-reject",
                                      BrokerUpdate::Set(barrier("0")));
    const auto rejected = engine.StartPreTrade(order);
    EXPECT_FALSE(rejected.Passed());
    ASSERT_EQ(rejected.rejects.size(), 1u);
    EXPECT_EQ(rejected.rejects.front().mode, RuleMode::Reject);
    EXPECT_EQ(rejected.rejects.front().code, RejectCode::OrderQtyExceedsLimit);
    const auto dry = engine.ExecutePreTradeDryRun(order);
    EXPECT_TRUE(dry.Reviews().empty());
    ASSERT_EQ(dry.Rejects().size(), 1u);
    const auto confirmedReject = engine.StartPreTrade(confirmedOrder);
    EXPECT_FALSE(confirmedReject.Passed());
    ASSERT_EQ(confirmedReject.rejects.size(), 1u);
    EXPECT_EQ(confirmedReject.rejects.front().mode, RuleMode::Reject);
    EXPECT_EQ(confirmedReject.rejects.front().code,
              RejectCode::OrderQtyExceedsLimit);
  }
}

TEST(BuiltinPolicy, ReviewInstanceExplicitRejectEqualsAbsent) {
  for (const bool explicitMode : {false, true}) {
    SCOPED_TRACE(explicitMode);
    policies::OrderSizeLimitPolicy config;
    config.BrokerBarrier(policies::OrderSizeBrokerBarrier(
        policies::OrderSizeLimit::Quantity(Quantity::FromString("0"))));
    openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
    if (explicitMode) {
      builder.Add(config, openpit::PolicyInstance{"named", RuleMode::Reject});
    } else {
      builder.Add(config);
    }
    const auto engine = builder.Build();
    const auto result = engine.StartPreTrade(MakeDryRunHookOrder());
    EXPECT_FALSE(result.Passed());
    ASSERT_EQ(result.rejects.size(), 1u);
    EXPECT_EQ(result.rejects.front().mode, RuleMode::Reject);
    EXPECT_EQ(result.rejects.front().code, RejectCode::OrderQtyExceedsLimit);
    EXPECT_EQ(result.rejects.front().scope, RejectScope::Order);
    EXPECT_EQ(result.rejects.front().policy, "OrderSizeLimitPolicy");
  }
}

TEST(CustomPolicy, ReviewInstanceInvalidModeThrows) {
  openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
  EXPECT_THROW(
      builder.Add(CustomPolicy<ReviewPolicy>(
                      "ReviewPolicy", ReviewPolicy{std::nullopt, std::nullopt}),
                  openpit::PolicyInstance{"invalid", static_cast<RuleMode>(4)}),
      openpit::Error);
}

TEST(CustomPolicy, ReviewInstanceDuplicateExplicitName) {
  openpit::EngineBuilder valid(openpit::SyncPolicy::Full);
  valid.Add(CustomPolicy<ReviewPolicy>(
                "ReviewPolicy", ReviewPolicy{std::nullopt, std::nullopt}),
            openpit::PolicyInstance{"duplicate"});
  valid.Add(CustomPolicy<ReviewPolicy>(
                "ReviewPolicy", ReviewPolicy{std::nullopt, std::nullopt}),
            openpit::PolicyInstance{"distinct", RuleMode::ReviewWithHold});
  EXPECT_NO_THROW(static_cast<void>(valid.Build()));

  openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
  builder.Add(CustomPolicy<ReviewPolicy>(
                  "ReviewPolicy", ReviewPolicy{std::nullopt, std::nullopt}),
              openpit::PolicyInstance{"duplicate"});
  builder.Add(CustomPolicy<ReviewPolicy>(
                  "ReviewPolicy", ReviewPolicy{std::nullopt, std::nullopt}),
              openpit::PolicyInstance{"duplicate", RuleMode::ReviewWithHold});
  try {
    static_cast<void>(builder.Build());
    FAIL() << "duplicate instance name must fail the build";
  } catch (const openpit::EngineBuildError& error) {
    EXPECT_EQ(error.Code(), openpit::EngineBuildErrorCode::DuplicatePolicyName);
  }
}

TEST(BuiltinPolicy, ReviewInstanceOrderSizeEvaluationFailure) {
  for (const auto mode :
       {RuleMode::ReviewWithHold, RuleMode::ReviewWithoutHold}) {
    SCOPED_TRACE(static_cast<int>(mode));
    policies::OrderSizeLimitPolicy config;
    config.BrokerBarrier(policies::OrderSizeBrokerBarrier(
        policies::OrderSizeLimit::Notional(Volume::FromString("100"))));
    openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
    builder.Add(config, openpit::PolicyInstance{"review", mode});
    const auto engine = builder.Build();
    auto order = MakeDryRunHookOrder();
    order.operation->price.reset();
    const auto result = engine.StartPreTrade(order);
    EXPECT_FALSE(result.Passed());
    ASSERT_EQ(result.rejects.size(), 1u);
    EXPECT_EQ(result.rejects.front().code,
              RejectCode::OrderValueCalculationFailed);
    EXPECT_EQ(result.rejects.front().mode, RuleMode::Reject);
    EXPECT_EQ(result.rejects.front().scope, RejectScope::Order);
    EXPECT_EQ(result.rejects.front().policy, "OrderSizeLimitPolicy");
  }
}

TEST(BuiltinPolicy, OrderSizeLimitWithoutCapsSurfacesCoreError) {
  openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
  policies::OrderSizeLimit empty =
      policies::OrderSizeLimit::Quantity(Quantity::FromString("1"));
  empty.maxQuantity.reset();
  policies::OrderSizeLimitPolicy config;
  config.BrokerBarrier(policies::OrderSizeBrokerBarrier(std::move(empty)));

  try {
    config.AddTo(builder);
    FAIL() << "an order-size limit without caps should fail";
  } catch (const openpit::Error& error) {
    EXPECT_THAT(error.Message(),
                testing::HasSubstr(
                    "at least one of max_quantity or max_notional must be "
                    "configured"));
  }
}

TEST(BuiltinPolicy, DuplicateOrderSizeAssetSurfacesCoreError) {
  openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
  policies::OrderSizeLimitPolicy config;
  config.AssetBarrier(policies::OrderSizeAssetBarrier(
      policies::OrderSizeLimit::Quantity(Quantity::FromString("10")),
      openpit::param::Asset("AAPL")));
  config.AssetBarrier(policies::OrderSizeAssetBarrier(
      policies::OrderSizeLimit::Notional(Volume::FromString("1000")),
      openpit::param::Asset("AAPL")));

  try {
    config.AddTo(builder);
    FAIL() << "a duplicate order-size asset should fail";
  } catch (const openpit::Error& error) {
    EXPECT_THAT(error.Message(),
                testing::HasSubstr("duplicate asset barrier for asset AAPL"));
  }
}

TEST(BuiltinPolicy, OrderSizeLimitWithoutBarrierBuilds) {
  openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
  const policies::OrderSizeLimitPolicy config;
  EXPECT_NO_THROW(config.AddTo(builder));
  EXPECT_NO_THROW({ openpit::Engine engine = builder.Build(); });
}

TEST(BuiltinPolicy, RateLimitAccountBarrierBuilds) {
  openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
  policies::RateLimitPolicy config;
  config.AccountBarrier(policies::RateLimitAccountBarrier(
      policies::RateLimit(/*maxOrders=*/10,
                          /*windowNanoseconds=*/1'000'000'000),
      ::openpit::param::AccountId::FromUint64(42)));
  config.AddTo(builder);
  EXPECT_NO_THROW({ openpit::Engine engine = builder.Build(); });
}

TEST(BuiltinPolicy, PnlBoundsKillSwitchBrokerBarrierBuilds) {
  openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
  policies::PnlBoundsBrokerBarrier barrier(openpit::param::Asset("USD"));
  barrier.lowerBound = openpit::param::Pnl::FromString("-1000");
  policies::PnlBoundsKillSwitchPolicy config;
  config.BrokerBarrier(std::move(barrier));
  config.AddTo(builder);
  EXPECT_NO_THROW({ openpit::Engine engine = builder.Build(); });
}

TEST(BuiltinPolicy, PnlBoundsKillSwitchDefaultsToReject) {
  const auto account = openpit::param::AccountId::FromUint64(42);
  const policies::PnlBoundsBrokerBarrier broker(openpit::param::Asset("USD"));
  const policies::PnlBoundsAccountBarrier accountBarrier(
      account, openpit::param::Asset("USD"),
      openpit::param::Pnl::FromString("0"));
  const policies::PnlBoundsAccountBarrierUpdate update(
      account, openpit::param::Asset("USD"));
  EXPECT_EQ(broker.mode, RuleMode::Reject);
  EXPECT_EQ(accountBarrier.mode, RuleMode::Reject);
  EXPECT_EQ(update.mode, RuleMode::Reject);
}

TEST(BuiltinPolicy, PnlBoundsKillSwitchPreTradeModes) {
  for (const auto mode :
       {static_cast<RuleMode>(0), RuleMode::Reject, RuleMode::ReviewWithHold,
        RuleMode::ReviewWithoutHold}) {
    const bool rejects =
        mode == static_cast<RuleMode>(0) || mode == RuleMode::Reject;
    for (const bool accountAxis : {false, true}) {
      SCOPED_TRACE(static_cast<int>(mode));
      SCOPED_TRACE(accountAxis);
      const auto account = openpit::param::AccountId::FromUint64(42);
      const auto loss = openpit::param::Pnl::FromString("-150");
      openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
      policies::PnlBoundsKillSwitchPolicy config;
      if (accountAxis) {
        policies::PnlBoundsAccountBarrier barrier(
            account, openpit::param::Asset("USD"), loss);
        barrier.mode = mode;
        barrier.lowerBound = openpit::param::Pnl::FromString("-100");
        config.AccountBarrier(barrier);
      } else {
        policies::PnlBoundsBrokerBarrier barrier(openpit::param::Asset("USD"));
        barrier.mode = mode;
        barrier.lowerBound = openpit::param::Pnl::FromString("-100");
        config.BrokerBarrier(barrier);
      }
      builder.Add(config);
      const auto engine = builder.Build();
      if (!accountAxis) {
        engine.Configure().SetAccountPnl(
            policies::PnlBoundsKillSwitchPolicyName, account,
            openpit::param::Asset("USD"), loss);
      }

      {
        auto start = engine.StartPreTrade(MakeDryRunHookOrder());
        if (rejects) {
          ASSERT_FALSE(start.Passed());
          ASSERT_EQ(start.rejects.size(), 1u);
          EXPECT_EQ(start.rejects.front().code,
                    RejectCode::PnlKillSwitchTriggered);
        } else {
          ASSERT_TRUE(start.Passed());
          EXPECT_TRUE(start.rejects.empty());
          const auto reviews = start.request->Reviews();
          ASSERT_EQ(reviews.size(), 1u);
          EXPECT_EQ(reviews.front().mode, mode);
          EXPECT_EQ(reviews.front().code, RejectCode::PnlKillSwitchTriggered);
        }
      }
      engine.Configure().SetAccountPnl(policies::PnlBoundsKillSwitchPolicyName,
                                       account, openpit::param::Asset("USD"),
                                       openpit::param::Pnl::FromString("0"));
      const auto following = engine.StartPreTrade(MakeDryRunHookOrder());
      EXPECT_EQ(following.Passed(), !rejects);
      if (rejects) {
        ASSERT_EQ(following.rejects.size(), 1u);
        EXPECT_EQ(following.rejects.front().code, RejectCode::AccountBlocked);
      }
    }
  }
}

TEST(BuiltinPolicy, PnlBoundsKillSwitchMixedBarriers) {
  for (const auto accountAxis : {false, true}) {
    for (const auto confirmed : {false, true}) {
      SCOPED_TRACE(accountAxis);
      SCOPED_TRACE(confirmed);
      auto order = MakeDryRunHookOrder();
      const auto account = order.operation->accountId.value();
      const openpit::param::Asset usd("USD");
      const auto zero = openpit::param::Pnl::FromString("0");
      policies::PnlBoundsKillSwitchPolicy config;
      for (const auto barrierMode :
           {RuleMode::ReviewWithHold, RuleMode::ReviewWithoutHold,
            RuleMode::Reject}) {
        const auto lower = openpit::param::Pnl::FromString(
            barrierMode == RuleMode::Reject ? "-200" : "-100");
        if (accountAxis) {
          policies::PnlBoundsAccountBarrier barrier(account, usd, zero);
          barrier.lowerBound = lower;
          barrier.mode = barrierMode;
          config.AccountBarrier(barrier);
        } else {
          policies::PnlBoundsBrokerBarrier barrier(usd);
          barrier.lowerBound = lower;
          barrier.mode = barrierMode;
          config.BrokerBarrier(barrier);
        }
      }
      openpit::EngineBuilder builder(openpit::SyncPolicy::None);
      builder.Add(config);
      const auto engine = builder.Build();
      const auto setPnl = [&](const char* value) {
        engine.Configure().SetAccountPnl(
            policies::PnlBoundsKillSwitchPolicyName, account, usd,
            openpit::param::Pnl::FromString(value));
      };
      setPnl("-150");
      {
        const auto first = engine.StartPreTrade(order);
        ASSERT_TRUE(first.Passed());
        const auto marks = first.request->Reviews();
        ASSERT_EQ(marks.size(), 2u);
        for (const auto mode :
             {RuleMode::ReviewWithHold, RuleMode::ReviewWithoutHold}) {
          EXPECT_EQ(
              std::count_if(marks.begin(), marks.end(),
                            [mode](const auto& mark) {
                              return mark.code ==
                                         RejectCode::PnlKillSwitchTriggered &&
                                     mark.mode == mode &&
                                     mark.scope == RejectScope::Account;
                            }),
              1);
        }
      }
      {
        const auto following = engine.StartPreTrade(order);
        ASSERT_TRUE(following.Passed());
      }
      if (confirmed) {
        order.review.emplace().confirmed = true;
        const auto rerun = engine.StartPreTrade(order);
        ASSERT_TRUE(rerun.Passed());
        EXPECT_TRUE(rerun.request->Reviews().empty());
      }
      setPnl("-250");
      if (!confirmed) {
        const auto probe = engine.StartPreTradeDryRun(order);
        EXPECT_FALSE(probe.Passed());
        EXPECT_TRUE(probe.Reviews().empty());
        const auto rejects = probe.Rejects();
        ASSERT_EQ(rejects.size(), 1u);
        EXPECT_EQ(rejects.front().mode, RuleMode::Reject);
        EXPECT_EQ(rejects.front().code, RejectCode::PnlKillSwitchTriggered);
        EXPECT_EQ(rejects.front().scope, RejectScope::Account);
      }
      const auto rejected = engine.StartPreTrade(order);
      ASSERT_FALSE(rejected.Passed());
      ASSERT_EQ(rejected.rejects.size(), 1u);
      EXPECT_EQ(rejected.rejects.front().mode, RuleMode::Reject);
      EXPECT_EQ(rejected.rejects.front().code,
                RejectCode::PnlKillSwitchTriggered);
      EXPECT_EQ(rejected.rejects.front().scope, RejectScope::Account);
      setPnl("0");
      const auto blocked = engine.StartPreTrade(order);
      ASSERT_FALSE(blocked.Passed());
      ASSERT_EQ(blocked.rejects.size(), 1u);
      EXPECT_EQ(blocked.rejects.front().code, RejectCode::AccountBlocked);
    }
  }
}

TEST(BuiltinPolicy, PnlBoundsKillSwitchReviewOrderMissingInstrumentRejects) {
  for (const auto mode :
       {RuleMode::ReviewWithHold, RuleMode::ReviewWithoutHold}) {
    SCOPED_TRACE(static_cast<int>(mode));
    policies::PnlBoundsBrokerBarrier barrier(openpit::param::Asset("USD"));
    barrier.mode = mode;
    barrier.lowerBound = openpit::param::Pnl::FromString("-100");
    policies::PnlBoundsKillSwitchPolicy config;
    config.BrokerBarrier(std::move(barrier));
    openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
    builder.Add(config);
    const auto engine = builder.Build();

    auto order = MakeDryRunHookOrder();
    order.operation->instrument.reset();
    const auto start = engine.StartPreTrade(order);
    EXPECT_FALSE(start.Passed());
    ASSERT_EQ(start.rejects.size(), 1u);
    EXPECT_EQ(start.rejects.front().code, RejectCode::MissingRequiredField);
    EXPECT_EQ(start.rejects.front().mode, RuleMode::Reject);
    EXPECT_EQ(start.rejects.front().policy,
              policies::PnlBoundsKillSwitchPolicyName);
  }
}

TEST(BuiltinPolicy, PnlBoundsKillSwitchExecutionReportModesAndMissingFee) {
  for (const auto mode : {RuleMode::Reject, RuleMode::ReviewWithHold,
                          RuleMode::ReviewWithoutHold}) {
    for (const bool accountAxis : {false, true}) {
      for (const bool missingFee : {false, true}) {
        SCOPED_TRACE(static_cast<int>(mode));
        SCOPED_TRACE(accountAxis);
        SCOPED_TRACE(missingFee);
        const auto account = openpit::param::AccountId::FromUint64(42);
        openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
        policies::PnlBoundsKillSwitchPolicy config;
        if (accountAxis) {
          policies::PnlBoundsAccountBarrier barrier(
              account, openpit::param::Asset("USD"),
              openpit::param::Pnl::FromString("0"));
          barrier.mode = mode;
          barrier.lowerBound = openpit::param::Pnl::FromString("-100");
          config.AccountBarrier(barrier);
        } else {
          policies::PnlBoundsBrokerBarrier barrier(
              openpit::param::Asset("USD"));
          barrier.mode = mode;
          barrier.lowerBound = openpit::param::Pnl::FromString("-100");
          config.BrokerBarrier(barrier);
        }
        builder.Add(config);
        const auto engine = builder.Build();
        openpit::model::ExecutionReportOperation operation;
        operation.accountId = account;
        operation.instrument = openpit::model::Instrument(
            openpit::param::Asset("AAPL"), openpit::param::Asset("USD"));
        operation.side = openpit::model::Side::Buy;
        openpit::model::ExecutionReport report;
        report.operation = std::move(operation);
        openpit::model::FinancialImpact impact;
        impact.pnl = openpit::param::Pnl::FromString("-150");
        if (!missingFee) {
          impact.fee = openpit::param::Fee::FromString("0");
        }
        report.financialImpact = std::move(impact);

        const auto result = engine.ApplyExecutionReport(report);
        const bool blocks = missingFee || mode == RuleMode::Reject;
        if (blocks) {
          ASSERT_EQ(result.accountBlocks.size(), 1u);
          EXPECT_EQ(result.accountBlocks.front().code,
                    missingFee ? RejectCode::MissingRequiredField
                               : RejectCode::PnlKillSwitchTriggered);
          if (missingFee) {
            EXPECT_THAT(result.accountBlocks.front().reason,
                        testing::HasSubstr("fee"));
          }
        } else {
          EXPECT_TRUE(result.accountBlocks.empty());
        }
        // Remove the breach so a refusal proves a latched account block.
        engine.Configure().SetAccountPnl(
            policies::PnlBoundsKillSwitchPolicyName, account,
            openpit::param::Asset("USD"), openpit::param::Pnl::FromString("0"));
        const auto following = engine.StartPreTrade(MakeDryRunHookOrder());
        EXPECT_EQ(following.Passed(), !blocks);
        if (blocks) {
          EXPECT_FALSE(following.rejects.empty());
        }
        if (!blocks) {
          report.financialImpact->pnl =
              openpit::param::Pnl::FromString("79228162514264337593543950335");
          EXPECT_TRUE(
              engine.ApplyExecutionReport(report).accountBlocks.empty());
          report.financialImpact->pnl = openpit::param::Pnl::FromString("1");
          const auto overflow = engine.ApplyExecutionReport(report);
          ASSERT_EQ(overflow.accountBlocks.size(), 1u);
          const auto& block = overflow.accountBlocks.front();
          EXPECT_TRUE(openpit::reject::IsEvaluationFailure(block.code));

          engine.Configure().SetAccountPnl(
              policies::PnlBoundsKillSwitchPolicyName, account,
              openpit::param::Asset("USD"),
              openpit::param::Pnl::FromString("0"));
          const auto blocked = engine.StartPreTrade(MakeDryRunHookOrder());
          ASSERT_FALSE(blocked.Passed());
          ASSERT_EQ(blocked.rejects.size(), 1u);
          EXPECT_EQ(blocked.rejects.front().code, block.code);
          EXPECT_EQ(blocked.rejects.front().reason, block.reason);
          EXPECT_EQ(blocked.rejects.front().details, block.details);
          EXPECT_EQ(blocked.rejects.front().mode, RuleMode::Reject);
        }
      }
    }
  }
}

TEST(BuiltinPolicy, PnlBoundsKillSwitchBuildRejectsDuplicateKeys) {
  for (const auto mode : {RuleMode::Reject, RuleMode::ReviewWithHold,
                          RuleMode::ReviewWithoutHold}) {
    SCOPED_TRACE(static_cast<int>(mode));
    policies::PnlBoundsBrokerBarrier broker(openpit::param::Asset("USD"));
    broker.mode = mode;
    broker.lowerBound = openpit::param::Pnl::FromString("-100");
    policies::PnlBoundsAccountBarrier account(
        openpit::param::AccountId::FromUint64(42), openpit::param::Asset("USD"),
        openpit::param::Pnl::FromString("0"));
    account.mode = mode;
    account.lowerBound = broker.lowerBound;
    auto duplicateBroker = broker;
    auto duplicateAccount = account;
    if (mode == RuleMode::Reject) {
      duplicateBroker.mode = static_cast<RuleMode>(0);
      duplicateAccount.mode = static_cast<RuleMode>(0);
    }
    openpit::EngineBuilder brokerBuilder(openpit::SyncPolicy::Full);
    EXPECT_THROW(brokerBuilder.Add(policies::PnlBoundsKillSwitchPolicy{}
                                       .BrokerBarrier(broker)
                                       .BrokerBarrier(duplicateBroker)),
                 openpit::Error);
    openpit::EngineBuilder validBroker(openpit::SyncPolicy::Full);
    validBroker.Add(
        policies::PnlBoundsKillSwitchPolicy{}.BrokerBarrier(broker));
    EXPECT_NO_THROW(static_cast<void>(validBroker.Build()));
    openpit::EngineBuilder accountBuilder(openpit::SyncPolicy::Full);
    EXPECT_THROW(accountBuilder.Add(policies::PnlBoundsKillSwitchPolicy{}
                                        .AccountBarrier(account)
                                        .AccountBarrier(duplicateAccount)),
                 openpit::Error);
    openpit::EngineBuilder validAccount(openpit::SyncPolicy::Full);
    validAccount.Add(
        policies::PnlBoundsKillSwitchPolicy{}.AccountBarrier(account));
    EXPECT_NO_THROW(static_cast<void>(validAccount.Build()));
  }
}

TEST(BuiltinPolicy, PnlBoundsKillSwitchBuildAllowsModesWithEqualSeeds) {
  for (const auto mode :
       {RuleMode::ReviewWithHold, RuleMode::ReviewWithoutHold}) {
    SCOPED_TRACE(static_cast<int>(mode));
    policies::PnlBoundsBrokerBarrier broker(openpit::param::Asset("USD"));
    broker.lowerBound = openpit::param::Pnl::FromString("-100");
    auto reviewBroker = broker;
    reviewBroker.mode = mode;
    policies::PnlBoundsAccountBarrier account(
        openpit::param::AccountId::FromUint64(42), openpit::param::Asset("USD"),
        openpit::param::Pnl::FromString("0"));
    account.lowerBound = broker.lowerBound;
    auto reviewAccount = account;
    reviewAccount.mode = mode;
    openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
    EXPECT_NO_THROW(builder.Add(policies::PnlBoundsKillSwitchPolicy{}
                                    .BrokerBarrier(broker)
                                    .BrokerBarrier(reviewBroker)
                                    .AccountBarrier(account)
                                    .AccountBarrier(reviewAccount)));
    EXPECT_NO_THROW({ const auto engine = builder.Build(); });

    openpit::EngineBuilder equalSeeds(openpit::SyncPolicy::Full);
    equalSeeds.Add(policies::PnlBoundsKillSwitchPolicy{}
                       .AccountBarrier(account)
                       .AccountBarrier(reviewAccount));
    EXPECT_NO_THROW(static_cast<void>(equalSeeds.Build()));
    reviewAccount.initialPnl = openpit::param::Pnl::FromString("1");
    openpit::EngineBuilder conflicting(openpit::SyncPolicy::Full);
    EXPECT_THROW(conflicting.Add(policies::PnlBoundsKillSwitchPolicy{}
                                     .AccountBarrier(account)
                                     .AccountBarrier(reviewAccount)),
                 openpit::Error);
  }
}

TEST(BuiltinPolicy, PnlBoundsKillSwitchBuildRejectsInvalidModes) {
  policies::PnlBoundsBrokerBarrier broker(openpit::param::Asset("USD"));
  broker.mode = static_cast<RuleMode>(4);
  broker.lowerBound = openpit::param::Pnl::FromString("-100");
  openpit::EngineBuilder brokerBuilder(openpit::SyncPolicy::Full);
  EXPECT_THROW(brokerBuilder.Add(
                   policies::PnlBoundsKillSwitchPolicy{}.BrokerBarrier(broker)),
               openpit::Error);
  broker.mode = RuleMode::Reject;
  openpit::EngineBuilder validBroker(openpit::SyncPolicy::Full);
  validBroker.Add(policies::PnlBoundsKillSwitchPolicy{}.BrokerBarrier(broker));
  EXPECT_NO_THROW(static_cast<void>(validBroker.Build()));
  policies::PnlBoundsAccountBarrier account(
      openpit::param::AccountId::FromUint64(42), openpit::param::Asset("USD"),
      openpit::param::Pnl::FromString("0"));
  account.mode = static_cast<RuleMode>(4);
  account.lowerBound = broker.lowerBound;
  openpit::EngineBuilder accountBuilder(openpit::SyncPolicy::Full);
  EXPECT_THROW(
      accountBuilder.Add(
          policies::PnlBoundsKillSwitchPolicy{}.AccountBarrier(account)),
      openpit::Error);
  account.mode = RuleMode::Reject;
  openpit::EngineBuilder validAccount(openpit::SyncPolicy::Full);
  validAccount.Add(
      policies::PnlBoundsKillSwitchPolicy{}.AccountBarrier(account));
  EXPECT_NO_THROW(static_cast<void>(validAccount.Build()));
}

TEST(BuiltinPolicy, PnlBoundsKillSwitchConfigurePreservesModes) {
  for (const auto mode :
       {static_cast<RuleMode>(0), RuleMode::Reject, RuleMode::ReviewWithHold,
        RuleMode::ReviewWithoutHold}) {
    for (const bool accountAxis : {false, true}) {
      SCOPED_TRACE(static_cast<int>(mode));
      SCOPED_TRACE(accountAxis);
      const auto account = openpit::param::AccountId::FromUint64(42);
      policies::PnlBoundsBrokerBarrier broker(openpit::param::Asset("USD"));
      broker.lowerBound = openpit::param::Pnl::FromString("-1000");
      openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
      builder.Add(policies::PnlBoundsKillSwitchPolicy{}.BrokerBarrier(broker));
      const auto engine = builder.Build();
      engine.Configure().SetAccountPnl(policies::PnlBoundsKillSwitchPolicyName,
                                       account, openpit::param::Asset("USD"),
                                       openpit::param::Pnl::FromString("-150"));
      if (accountAxis) {
        policies::PnlBoundsAccountBarrierUpdate update(
            account, openpit::param::Asset("USD"));
        update.mode = mode;
        update.lowerBound = openpit::param::Pnl::FromString("-100");
        engine.Configure().PnlBoundsKillSwitch(
            policies::PnlBoundsKillSwitchPolicyName, std::nullopt,
            std::vector<policies::PnlBoundsAccountBarrierUpdate>{update});
      } else {
        broker.mode = mode;
        broker.lowerBound = openpit::param::Pnl::FromString("-100");
        engine.Configure().PnlBoundsKillSwitch(
            policies::PnlBoundsKillSwitchPolicyName,
            std::vector<policies::PnlBoundsBrokerBarrier>{broker});
      }
      const auto start = engine.StartPreTrade(MakeDryRunHookOrder());
      if (mode == static_cast<RuleMode>(0) || mode == RuleMode::Reject) {
        ASSERT_FALSE(start.Passed());
        ASSERT_EQ(start.rejects.size(), 1u);
        EXPECT_EQ(start.rejects.front().code,
                  RejectCode::PnlKillSwitchTriggered);
        const auto following = engine.StartPreTrade(MakeDryRunHookOrder());
        ASSERT_EQ(following.rejects.size(), 1u);
        EXPECT_EQ(following.rejects.front().code, RejectCode::AccountBlocked);
      } else {
        ASSERT_TRUE(start.Passed());
        EXPECT_TRUE(start.rejects.empty());
        const auto reviews = start.request->Reviews();
        ASSERT_EQ(reviews.size(), 1u);
        EXPECT_EQ(reviews.front().mode, mode);
      }
    }
  }
}

TEST(BuiltinPolicy,
     PnlBoundsKillSwitchConfigureValidationKeepsPreviousBarriers) {
  for (const bool accountAxis : {false, true}) {
    for (const bool invalidMode : {false, true}) {
      SCOPED_TRACE(accountAxis);
      SCOPED_TRACE(invalidMode);
      const auto account = openpit::param::AccountId::FromUint64(42);
      policies::PnlBoundsBrokerBarrier broker(openpit::param::Asset("USD"));
      broker.lowerBound = openpit::param::Pnl::FromString("-100");
      policies::PnlBoundsAccountBarrier initial(
          account, openpit::param::Asset("USD"),
          openpit::param::Pnl::FromString("-150"));
      initial.lowerBound = broker.lowerBound;
      openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
      policies::PnlBoundsKillSwitchPolicy config;
      if (accountAxis) {
        config.AccountBarrier(initial);
      } else {
        config.BrokerBarrier(broker);
      }
      builder.Add(config);
      const auto engine = builder.Build();
      if (!accountAxis) {
        engine.Configure().SetAccountPnl(
            policies::PnlBoundsKillSwitchPolicyName, account,
            openpit::param::Asset("USD"),
            openpit::param::Pnl::FromString("-150"));
      }
      broker.lowerBound = openpit::param::Pnl::FromString("-200");
      policies::PnlBoundsAccountBarrierUpdate update(
          account, openpit::param::Asset("USD"));
      update.lowerBound = broker.lowerBound;
      if (invalidMode) {
        broker.mode = static_cast<RuleMode>(4);
        update.mode = static_cast<RuleMode>(4);
      }
      try {
        if (accountAxis) {
          engine.Configure().PnlBoundsKillSwitch(
              policies::PnlBoundsKillSwitchPolicyName, std::nullopt,
              invalidMode
                  ? std::vector<policies::PnlBoundsAccountBarrierUpdate>{update}
                  : std::vector<policies::PnlBoundsAccountBarrierUpdate>{
                        update, update});
        } else {
          engine.Configure().PnlBoundsKillSwitch(
              policies::PnlBoundsKillSwitchPolicyName,
              invalidMode
                  ? std::vector<policies::PnlBoundsBrokerBarrier>{broker}
                  : std::vector<policies::PnlBoundsBrokerBarrier>{broker,
                                                                  broker});
        }
        FAIL() << "invalid P&L barriers should fail configuration";
      } catch (const openpit::ConfigureError& error) {
        EXPECT_EQ(error.Kind(), openpit::ConfigureErrorKind::Validation);
      }
      const auto start = engine.StartPreTrade(MakeDryRunHookOrder());
      ASSERT_FALSE(start.Passed());
      ASSERT_EQ(start.rejects.size(), 1u);
      EXPECT_EQ(start.rejects.front().code, RejectCode::PnlKillSwitchTriggered);
      const auto following = engine.StartPreTrade(MakeDryRunHookOrder());
      ASSERT_EQ(following.rejects.size(), 1u);
      EXPECT_EQ(following.rejects.front().code, RejectCode::AccountBlocked);
      broker.mode = RuleMode::Reject;
      update.mode = RuleMode::Reject;
      if (accountAxis) {
        EXPECT_NO_THROW(engine.Configure().PnlBoundsKillSwitch(
            policies::PnlBoundsKillSwitchPolicyName, std::nullopt,
            std::vector{update}));
      } else {
        EXPECT_NO_THROW(engine.Configure().PnlBoundsKillSwitch(
            policies::PnlBoundsKillSwitchPolicyName, std::vector{broker}));
      }
    }
  }
}

TEST(BuiltinPolicy, SpotFundsLimitOnlyBuilds) {
  openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
  // No market-data service: limit-only mode, which needs no MD handle.
  policies::SpotFundsPolicy config;
  config.AddTo(builder);
  EXPECT_NO_THROW({ openpit::Engine engine = builder.Build(); });
}

TEST(BuiltinPolicy, SpotFundsWithoutPnlBarriersExecutesNormalOrder) {
  openpit::EngineBuilder builder(openpit::SyncPolicy::None);
  builder.Add(policies::SpotFundsPolicy{});
  const openpit::Engine engine = builder.Build();
  const openpit::param::AccountId account =
      openpit::param::AccountId::FromUint64(83010);

  SeedSpotFundsLifecycleAccount(engine, account);
  EXPECT_TRUE(ApplySpotFundsLifecycleFill(engine, account).empty());
}

TEST(BuiltinPolicy, SpotFundsAlreadyHeldDoesNotReserveAgain) {
  openpit::EngineBuilder builder(openpit::SyncPolicy::None);
  builder.Add(policies::SpotFundsPolicy{});
  const openpit::Engine engine = builder.Build();
  const openpit::param::AccountId account =
      openpit::param::AccountId::FromUint64(83024);
  ASSERT_NO_FATAL_FAILURE(SeedSpotFundsLifecycleAccount(engine, account));

  engine.Configure().SpotFundsGlobalLimitMode(
      policies::SpotFundsPolicyName,
      policies::SpotFundsLimitMode::ReviewWithHold);
  openpit::model::Order order =
      SpotFundsLifecycleOrder(account, Quantity::FromString("11"));
  auto initial = engine.ExecutePreTrade(order);
  ASSERT_TRUE(initial.Passed());
  const auto marks = initial.reservation->Reviews();
  ASSERT_EQ(marks.size(), 1U);
  EXPECT_EQ(marks.front().code, RejectCode::InsufficientFunds);
  EXPECT_EQ(marks.front().mode, RuleMode::ReviewWithHold);
  auto firstLock = initial.reservation->Lock();
  EXPECT_FALSE(firstLock.IsEmpty());
  EXPECT_THAT(firstLock.Prices(),
              testing::UnorderedElementsAre(Price::FromString("100")));
  initial.reservation->Commit();
  const auto before = SpotFundsBalances(engine, account);
  EXPECT_EQ(before.usd.balance, "-100");
  EXPECT_EQ(before.usd.held, "1100");
  EXPECT_EQ(before.usd.incoming, "0");
  EXPECT_EQ(before.aapl.balance, "0");
  EXPECT_EQ(before.aapl.held, "0");
  EXPECT_EQ(before.aapl.incoming, "11");

  auto& review = order.review.emplace();
  review.confirmed = true;
  review.alreadyHeld = true;
  auto held = engine.ExecutePreTrade(order);
  ASSERT_TRUE(held.Passed()) << "Expected the already-held order to pass";
  EXPECT_TRUE(held.reservation->Lock().IsEmpty());
  EXPECT_TRUE(held.reservation->Reviews().empty());
  EXPECT_TRUE(held.reservation->AccountAdjustments().empty());
  held.reservation->Commit();
  EXPECT_EQ(SpotFundsBalances(engine, account), before);
  const auto cancelled = engine.ApplyExecutionReport(SpotFundsCancelReport(
      account, Quantity::FromString("11"), std::move(firstLock)));
  EXPECT_TRUE(cancelled.accountBlocks.empty());
  EXPECT_EQ(SpotFundsBalances(engine, account),
            (SpotFundsBalanceSnapshot{{"1000", "0", "0"}, {"0", "0", "0"}}));
}

TEST(BuiltinPolicy, SpotFundsAlreadyHeldRejectPreservesFirstHold) {
  for (const bool pnlBreach : {false, true}) {
    SCOPED_TRACE(pnlBreach);
    openpit::EngineBuilder builder(openpit::SyncPolicy::None);
    policies::SpotFundsPnlBoundsBarrier barrier(openpit::param::Asset("USD"));
    barrier.lowerBound = openpit::param::Pnl::FromString("-100");
    builder.Add(
        policies::SpotFundsPnlBoundsKillSwitchPolicy{}.GlobalBarrier(barrier));
    const auto engine = builder.Build();
    const auto account = openpit::param::AccountId::FromUint64(83025);
    SeedSpotFundsLifecycleAccount(engine, account);
    engine.Configure().SpotFundsGlobalLimitMode(
        policies::SpotFundsPolicyName, policies::SpotFundsLimitMode::Enforce);
    auto order = SpotFundsLifecycleOrder(account, Quantity::FromString("6"));
    auto first = engine.ExecutePreTrade(order);
    ASSERT_TRUE(first.Passed());
    first.reservation->Commit();
    if (!pnlBreach) {
      const auto beforeConfirmed = SpotFundsBalances(engine, account);
      order.review.emplace().confirmed = true;
      const auto confirmed = engine.ExecutePreTrade(order);
      EXPECT_FALSE(confirmed.Passed());
      EXPECT_FALSE(confirmed.reservation.has_value());
      ASSERT_EQ(confirmed.rejects.size(), 1U);
      EXPECT_EQ(confirmed.rejects.front().code, RejectCode::InsufficientFunds);
      EXPECT_EQ(confirmed.rejects.front().mode, RuleMode::Reject);
      EXPECT_EQ(SpotFundsBalances(engine, account), beforeConfirmed);

      const auto beforeHeld = SpotFundsBalances(engine, account);
      order.review->alreadyHeld = true;
      auto held = engine.ExecutePreTrade(order);
      ASSERT_TRUE(held.Passed());
      EXPECT_TRUE(held.reservation->Lock().IsEmpty());
      EXPECT_TRUE(held.reservation->Reviews().empty());
      EXPECT_TRUE(held.reservation->AccountAdjustments().empty());
      held.reservation->Commit();
      EXPECT_EQ(SpotFundsBalances(engine, account), beforeHeld);
    }
    if (pnlBreach) {
      const auto change = engine.Configure().SetSpotFundsAccountPnl(
          policies::SpotFundsPolicyName, account,
          openpit::param::Pnl::FromString("-150"));
      ASSERT_EQ(change.accountBlocks.size(), 1U);
      engine.Accounts().Unblock(account);
    } else {
      AdjustSpotFundsBalances(engine, account, "USD", "-500");
    }
    const auto before = SpotFundsBalances(engine, account);
    order.review.emplace().confirmed = true;
    order.review->alreadyHeld = true;
    const auto rerun = engine.ExecutePreTrade(order);
    EXPECT_EQ(before, pnlBreach ? (SpotFundsBalanceSnapshot{{"400", "600", "0"},
                                                            {"0", "0", "6"}})
                                : (SpotFundsBalanceSnapshot{
                                      {"-100", "600", "0"}, {"0", "0", "6"}}));
    ASSERT_FALSE(rerun.Passed());
    ASSERT_EQ(rerun.rejects.size(), 1U);
    EXPECT_EQ(rerun.rejects.front().code,
              pnlBreach ? RejectCode::PnlKillSwitchTriggered
                        : RejectCode::InsufficientFunds);
    EXPECT_EQ(rerun.rejects.front().mode, RuleMode::Reject);
    EXPECT_EQ(SpotFundsBalances(engine, account), before);
  }
}

TEST(BuiltinPolicy, SpotFundsAlreadyHeldCancelNeedsFirstLock) {
  openpit::EngineBuilder builder(openpit::SyncPolicy::None);
  builder.Add(policies::SpotFundsPolicy{});
  const auto engine = builder.Build();
  const auto account = openpit::param::AccountId::FromUint64(83026);
  SeedSpotFundsLifecycleAccount(engine, account);
  auto order = SpotFundsLifecycleOrder(account);
  auto first = engine.ExecutePreTrade(order);
  ASSERT_TRUE(first.Passed());
  first.reservation->Commit();
  order.review.emplace().confirmed = true;
  order.review->alreadyHeld = true;
  auto rerun = engine.ExecutePreTrade(order);
  ASSERT_TRUE(rerun.Passed());
  auto lock = rerun.reservation->Lock();
  ASSERT_TRUE(lock.IsEmpty());
  EXPECT_TRUE(rerun.reservation->AccountAdjustments().empty());
  rerun.reservation->Commit();
  const auto before = AdjustSpotFundsBalances(engine, account, "USD", "0");
  EXPECT_EQ(before.balance.value().absolute.ToString(), "900");
  EXPECT_EQ(before.held.value().absolute.ToString(), "100");
  EXPECT_EQ(before.incoming.value().absolute.ToString(), "0");
  const auto cancelled = engine.ApplyExecutionReport(SpotFundsCancelReport(
      account, Quantity::FromString("1"), std::move(lock)));
  ASSERT_EQ(cancelled.accountBlocks.size(), 1U);
  EXPECT_EQ(cancelled.accountBlocks.front().code,
            RejectCode::MissingRequiredField);
  const auto after = AdjustSpotFundsBalances(engine, account, "USD", "0");
  EXPECT_EQ(after.balance.value().absolute, before.balance.value().absolute);
  EXPECT_EQ(after.held.value().absolute, before.held.value().absolute);
  EXPECT_EQ(after.incoming.value().absolute, before.incoming.value().absolute);
}

TEST(BuiltinPolicy, SpotFundsAlreadyHeldReviewRuleMarksUntilConfirmed) {
  for (const auto mode :
       {RuleMode::ReviewWithHold, RuleMode::ReviewWithoutHold}) {
    SCOPED_TRACE(static_cast<int>(mode));
    policies::SpotFundsPnlBoundsBarrier barrier(openpit::param::Asset("USD"));
    barrier.mode = mode;
    barrier.lowerBound = openpit::param::Pnl::FromString("-100");
    openpit::EngineBuilder builder(openpit::SyncPolicy::None);
    builder.Add(
        policies::SpotFundsPnlBoundsKillSwitchPolicy{}.GlobalBarrier(barrier));
    const auto engine = builder.Build();
    const auto account = openpit::param::AccountId::FromUint64(83027);
    SeedSpotFundsLifecycleAccount(engine, account);
    auto order = SpotFundsLifecycleOrder(account);
    auto first = engine.ExecutePreTrade(order);
    ASSERT_TRUE(first.Passed());
    first.reservation->Commit();
    const auto configured = engine.Configure().SetSpotFundsAccountPnl(
        policies::SpotFundsPolicyName, account,
        openpit::param::Pnl::FromString("-150"));
    ASSERT_TRUE(configured.accountBlocks.empty());
    engine.Configure().SpotFundsPositionLimit(
        policies::SpotFundsPolicyName, account, openpit::param::Asset("AAPL"),
        mode == RuleMode::ReviewWithHold ? RuleMode::ReviewWithoutHold
                                         : RuleMode::ReviewWithHold,
        Quantity::FromString("0"));
    const auto probe = engine.ExecutePreTradeDryRun(order);
    ASSERT_TRUE(probe.Passed());
    const auto marks = probe.Reviews();
    ASSERT_EQ(marks.size(), 2u);
    EXPECT_EQ(std::count_if(marks.begin(), marks.end(),
                            [mode](const auto& mark) {
                              return mark.code ==
                                         RejectCode::PnlKillSwitchTriggered &&
                                     mark.mode == mode;
                            }),
              1);
    EXPECT_EQ(std::count_if(marks.begin(), marks.end(),
                            [mode](const auto& mark) {
                              return mark.code ==
                                         RejectCode::PositionLimitExceeded &&
                                     mark.mode != mode &&
                                     (mark.mode == RuleMode::ReviewWithHold ||
                                      mark.mode == RuleMode::ReviewWithoutHold);
                            }),
              1);
    const auto before = SpotFundsBalances(engine, account);
    order.review.emplace().alreadyHeld = true;
    const auto heldDry = engine.ExecutePreTradeDryRun(order);
    ASSERT_TRUE(heldDry.Passed());
    const auto heldMarks = heldDry.Reviews();
    ASSERT_EQ(heldMarks.size(), 2u);
    EXPECT_TRUE(heldDry.Lock().IsEmpty());
    EXPECT_TRUE(heldDry.AccountAdjustments().empty());
    EXPECT_EQ(SpotFundsBalances(engine, account), before);
    auto unconfirmed = engine.ExecutePreTrade(order);
    ASSERT_TRUE(unconfirmed.Passed());
    const auto liveMarks = unconfirmed.reservation->Reviews();
    ASSERT_EQ(liveMarks.size(), 2u);
    for (std::size_t index = 0; index < marks.size(); ++index) {
      EXPECT_EQ(heldMarks[index].code, marks[index].code);
      EXPECT_EQ(heldMarks[index].mode, marks[index].mode);
      EXPECT_EQ(liveMarks[index].code, marks[index].code);
      EXPECT_EQ(liveMarks[index].mode, marks[index].mode);
    }
    EXPECT_TRUE(unconfirmed.reservation->Lock().IsEmpty());
    EXPECT_TRUE(unconfirmed.reservation->AccountAdjustments().empty());
    unconfirmed.reservation->Commit();
    EXPECT_EQ(SpotFundsBalances(engine, account), before);
    order.review.emplace().confirmed = true;
    order.review->alreadyHeld = true;
    auto rerun = engine.ExecutePreTrade(order);
    ASSERT_TRUE(rerun.Passed());
    EXPECT_TRUE(rerun.reservation->Lock().IsEmpty());
    EXPECT_TRUE(rerun.reservation->Reviews().empty());
    EXPECT_TRUE(rerun.reservation->AccountAdjustments().empty());
    rerun.reservation->Commit();
    EXPECT_EQ(SpotFundsBalances(engine, account), before);
  }
}

TEST(BuiltinPolicy, SpotFundsConfirmedReviewModesRecordNewHold) {
  for (const auto mode : {policies::SpotFundsLimitMode::ReviewWithHold,
                          policies::SpotFundsLimitMode::ReviewWithoutHold}) {
    SCOPED_TRACE(static_cast<int>(mode));
    openpit::EngineBuilder builder(openpit::SyncPolicy::None);
    builder.Add(policies::SpotFundsPolicy{});
    const auto engine = builder.Build();
    const auto account = openpit::param::AccountId::FromUint64(83028);
    SeedSpotFundsLifecycleAccount(engine, account);
    engine.Configure().SpotFundsGlobalLimitMode(policies::SpotFundsPolicyName,
                                                mode);
    auto order = SpotFundsLifecycleOrder(account, Quantity::FromString("11"));
    order.review.emplace().confirmed = true;
    auto result = engine.ExecutePreTrade(order);
    ASSERT_TRUE(result.Passed());
    EXPECT_TRUE(result.reservation->Reviews().empty());
    EXPECT_FALSE(result.reservation->Lock().IsEmpty());
    result.reservation->Commit();
    EXPECT_EQ(
        SpotFundsBalances(engine, account),
        (SpotFundsBalanceSnapshot{{"-100", "1100", "0"}, {"0", "0", "11"}}));
  }
}

TEST(BuiltinPolicy, SpotFundsReviewModesDropCopyTracksWithoutLimits) {
  for (const auto mode : {policies::SpotFundsLimitMode::ReviewWithHold,
                          policies::SpotFundsLimitMode::ReviewWithoutHold}) {
    SCOPED_TRACE(static_cast<int>(mode));
    openpit::EngineBuilder builder(openpit::SyncPolicy::None);
    builder.Add(policies::SpotFundsPolicy{});
    const auto engine = builder.Build();
    const auto account = openpit::param::AccountId::FromUint64(83029);
    SeedSpotFundsLifecycleAccount(engine, account);
    engine.Configure().SpotFundsGlobalLimitMode(policies::SpotFundsPolicyName,
                                                mode);
    engine.Configure().SpotFundsPositionLimit(
        policies::SpotFundsPolicyName, account, openpit::param::Asset("AAPL"),
        RuleMode::Reject, Quantity::FromString("0"));
    const auto ordinaryBefore = engine.ExecutePreTradeDryRun(
        SpotFundsLifecycleOrder(account, Quantity::FromString("11")));
    ASSERT_EQ(ordinaryBefore.Rejects().size(), 1U);
    EXPECT_EQ(ordinaryBefore.Rejects().front().code,
              RejectCode::PositionLimitExceeded);
    EXPECT_EQ(ordinaryBefore.Rejects().front().mode, RuleMode::Reject);
    auto result = engine.ApplyDropCopy(
        SpotFundsLifecycleOrder(account, Quantity::FromString("11")));
    ASSERT_TRUE(result.Passed());
    EXPECT_TRUE(result.rejects.empty());
    ASSERT_TRUE(result.operation.has_value());
    EXPECT_FALSE(result.operation->IsAccountBlocked());
    auto lock = result.operation->Lock();
    EXPECT_FALSE(lock.IsEmpty());
    EXPECT_THAT(lock.Prices(),
                testing::UnorderedElementsAre(Price::FromString("100")));
    const auto adjustments = result.operation->AccountAdjustments();
    ASSERT_EQ(adjustments.size(), 2U);
    std::vector<openpit::param::Asset> assets;
    for (const auto& adjustment : adjustments) {
      const auto& entry = adjustment.entry;
      assets.push_back(entry.asset);
      if (entry.asset == openpit::param::Asset("USD")) {
        ASSERT_TRUE(entry.balance.has_value());
        ASSERT_TRUE(entry.held.has_value());
        EXPECT_EQ(entry.balance->delta.ToString(), "-1100");
        EXPECT_EQ(entry.held->delta.ToString(), "1100");
      } else if (entry.asset == openpit::param::Asset("AAPL")) {
        ASSERT_TRUE(entry.incoming.has_value());
        EXPECT_EQ(entry.incoming->delta.ToString(), "11");
      }
    }
    EXPECT_THAT(assets,
                testing::UnorderedElementsAre(openpit::param::Asset("USD"),
                                              openpit::param::Asset("AAPL")));
    result.operation->Commit();
    EXPECT_EQ(
        SpotFundsBalances(engine, account),
        (SpotFundsBalanceSnapshot{{"-100", "1100", "0"}, {"0", "0", "11"}}));
    engine.Configure().SpotFundsPositionLimit(
        policies::SpotFundsPolicyName, account, openpit::param::Asset("AAPL"),
        RuleMode::Reject, std::nullopt);
    auto ordinary = engine.ExecutePreTrade(SpotFundsLifecycleOrder(account));
    ASSERT_TRUE(ordinary.Passed());
    ASSERT_EQ(ordinary.reservation->Reviews().size(), 1U);
    EXPECT_EQ(ordinary.reservation->Reviews().front().code,
              RejectCode::InsufficientFunds);
    EXPECT_EQ(ordinary.reservation->Reviews().front().mode,
              mode == policies::SpotFundsLimitMode::ReviewWithHold
                  ? RuleMode::ReviewWithHold
                  : RuleMode::ReviewWithoutHold);
    ordinary.reservation->Rollback();
    EXPECT_TRUE(engine
                    .ApplyExecutionReport(SpotFundsCancelReport(
                        account, Quantity::FromString("11"), std::move(lock)))
                    .accountBlocks.empty());
    EXPECT_EQ(SpotFundsBalances(engine, account),
              (SpotFundsBalanceSnapshot{{"1000", "0", "0"}, {"0", "0", "0"}}));
  }
}

TEST(BuiltinPolicy, SpotFundsReviewLimitModeCascade) {
  for (const bool reverse : {false, true}) {
    SCOPED_TRACE(reverse);
    openpit::EngineBuilder builder(openpit::SyncPolicy::None);
    builder.Add(policies::SpotFundsPolicy{});
    const auto engine = builder.Build();
    const auto account = openpit::param::AccountId::FromUint64(83030);
    const auto group = openpit::param::AccountGroupId::FromUint32(83031);
    ASSERT_FALSE(engine.Accounts().RegisterGroup({account}, group).has_value());
    SeedSpotFundsLifecycleAccount(engine, account);
    const auto global = reverse
                            ? policies::SpotFundsLimitMode::ReviewWithoutHold
                            : policies::SpotFundsLimitMode::ReviewWithHold;
    const auto grouped = reverse
                             ? policies::SpotFundsLimitMode::ReviewWithHold
                             : policies::SpotFundsLimitMode::ReviewWithoutHold;
    engine.Configure().SpotFundsGlobalLimitMode(policies::SpotFundsPolicyName,
                                                global);
    engine.Configure().SpotFundsAccountGroupLimitMode(
        policies::SpotFundsPolicyName, group, grouped);
    engine.Configure().SpotFundsAccountLimitMode(policies::SpotFundsPolicyName,
                                                 account, global);
    const auto check = [&](policies::SpotFundsLimitMode expected) {
      auto result = engine.ExecutePreTrade(
          SpotFundsLifecycleOrder(account, Quantity::FromString("11")));
      ASSERT_TRUE(result.Passed());
      ASSERT_EQ(result.reservation->Reviews().size(), 1U);
      const bool holds =
          expected == policies::SpotFundsLimitMode::ReviewWithHold;
      EXPECT_EQ(result.reservation->Reviews().front().mode,
                holds ? RuleMode::ReviewWithHold : RuleMode::ReviewWithoutHold);
      EXPECT_EQ(SpotFundsBalances(engine, account),
                holds ? (SpotFundsBalanceSnapshot{{"-100", "1100", "0"},
                                                  {"0", "0", "11"}})
                      : (SpotFundsBalanceSnapshot{{"1000", "0", "0"},
                                                  {"0", "0", "0"}}));
      result.reservation->Rollback();
      EXPECT_EQ(
          SpotFundsBalances(engine, account),
          (SpotFundsBalanceSnapshot{{"1000", "0", "0"}, {"0", "0", "0"}}));
    };
    check(global);
    engine.Configure().SpotFundsAccountLimitMode(policies::SpotFundsPolicyName,
                                                 account, std::nullopt);
    check(grouped);
    engine.Configure().SpotFundsAccountGroupLimitMode(
        policies::SpotFundsPolicyName, group, std::nullopt);
    check(global);
  }
}

TEST(BuiltinPolicy, SpotFundsAbsentFillLockBlocksAccount) {
  std::optional<SpotFundsBlockedFillSetup> setup =
      MakeSpotFundsBlockedFillSetup();
  ASSERT_TRUE(setup.has_value());

  openpit::model::Fill fill;
  EXPECT_FALSE(fill.lock.has_value());
  fill.lastTrade = openpit::model::Trade(Price::FromString("150"),
                                         Quantity::FromString("2"));
  fill.remainingReservedQuantity = Quantity::FromString("0");
  fill.isFinal = true;

  openpit::model::ExecutionReport report;
  report.operation = std::move(setup->operation);
  report.fill = std::move(fill);

  const openpit::PostTradeResult result =
      setup->engine.ApplyExecutionReport(report);
  ASSERT_EQ(result.accountBlocks.size(), 1u);
  EXPECT_EQ(result.accountBlocks.front().code,
            RejectCode::MissingRequiredField);
}

TEST(BuiltinPolicy, SpotFundsEmptyFillLockBlocksAccount) {
  std::optional<SpotFundsBlockedFillSetup> setup =
      MakeSpotFundsBlockedFillSetup();
  ASSERT_TRUE(setup.has_value());

  openpit::model::Fill fill;
  fill.lock.emplace();
  ASSERT_TRUE(fill.lock.has_value());
  EXPECT_TRUE(fill.lock->IsEmpty());
  fill.lastTrade = openpit::model::Trade(Price::FromString("150"),
                                         Quantity::FromString("2"));
  fill.remainingReservedQuantity = Quantity::FromString("0");
  fill.isFinal = true;

  openpit::model::ExecutionReport report;
  report.operation = std::move(setup->operation);
  report.fill = std::move(fill);

  const openpit::PostTradeResult result =
      setup->engine.ApplyExecutionReport(report);
  ASSERT_EQ(result.accountBlocks.size(), 1u);
  EXPECT_EQ(result.accountBlocks.front().code,
            RejectCode::MissingRequiredField);
}

TEST(BuiltinPolicy, SpotFundsOverridesBuildWithInstrumentIdWrapper) {
  namespace md = openpit::marketdata;
  openpit::EngineBuilder builder(openpit::SyncPolicy::Full);

  policies::SpotFundsPolicy config;
  policies::SpotFundsOverride instrument(md::InstrumentId::FromUint64(55));
  instrument.slippageBps = 1500;
  config.Override(instrument);
  config.Override(policies::SpotFundsOverride(
      md::InstrumentId::FromUint64(56),
      openpit::param::AccountId::FromUint64(99224416)));
  config.Override(policies::SpotFundsOverride(
      md::InstrumentId::FromUint64(57),
      openpit::param::AccountGroupId::FromUint32(7)));

  builder.Add(config);
  EXPECT_NO_THROW({ openpit::Engine engine = builder.Build(); });
}

TEST(BuiltinPolicy, SpotFundsMarketOrdersAcceptServiceWrapper) {
  namespace md = openpit::marketdata;
  openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
  md::Service marketData =
      md::Builder::FromEngineSyncPolicy(md::QuoteTtl::Infinite(),
                                        openpit::SyncPolicy::Full)
          .Build();

  builder.Add(policies::SpotFundsPolicy{}.WithMarketOrders(marketData, 1500));
  EXPECT_NO_THROW({ openpit::Engine engine = builder.Build(); });
}

TEST(BuiltinPolicy, SpotFundsPnlBoundsBarrierPreservesBounds) {
  policies::SpotFundsPnlBoundsBarrier barrier(openpit::param::Asset("USD"));
  EXPECT_EQ(barrier.mode, RuleMode::Reject);
  barrier.lowerBound = openpit::param::Pnl::FromString("-1000");
  barrier.upperBound = openpit::param::Pnl::FromString("250");

  ASSERT_TRUE(barrier.lowerBound.has_value());
  ASSERT_TRUE(barrier.upperBound.has_value());
  EXPECT_EQ(barrier.lowerBound->ToString(), "-1000");
  EXPECT_EQ(barrier.upperBound->ToString(), "250");
  for (const auto invalidMode : {4, 255}) {
    barrier.mode = static_cast<RuleMode>(invalidMode);
    openpit::EngineBuilder builder(openpit::SyncPolicy::None);
    EXPECT_THROW(builder.Add(policies::SpotFundsPnlBoundsKillSwitchPolicy{}
                                 .GlobalBarrier(barrier)),
                 openpit::Error);
  }
}

TEST(BuiltinPolicy, SpotFundsPnlModesReachPolicy) {
  for (const auto mode :
       {RuleMode::ReviewWithHold, RuleMode::ReviewWithoutHold}) {
    SCOPED_TRACE(static_cast<int>(mode));
    policies::SpotFundsPnlBoundsBarrier rejecting(openpit::param::Asset("USD"));
    rejecting.lowerBound = openpit::param::Pnl::FromString("-100");
    auto reviewing = rejecting;
    reviewing.mode = mode;
    reviewing.lowerBound = openpit::param::Pnl::FromString("-10");
    openpit::EngineBuilder builder(openpit::SyncPolicy::None);
    builder.Add(policies::SpotFundsPnlBoundsKillSwitchPolicy{}
                    .GlobalBarrier(rejecting)
                    .GlobalBarrier(reviewing));
    const auto engine = builder.Build();
    const auto account = openpit::param::AccountId::FromUint64(83301);
    SeedSpotFundsLifecycleAccount(engine, account);
    const auto seeded = engine.Configure().SetSpotFundsAccountPnl(
        policies::SpotFundsPolicyName, account,
        openpit::param::Pnl::FromString("-50"));
    ASSERT_TRUE(seeded.accountBlocks.empty());
    const auto report =
        engine.ExecutePreTradeDryRun(SpotFundsLifecycleOrder(account));
    ASSERT_TRUE(report.Passed());
    const auto reviews = report.Reviews();
    ASSERT_EQ(reviews.size(), 1U);
    EXPECT_EQ(reviews.front().mode, mode);
    EXPECT_EQ(reviews.front().code, RejectCode::PnlKillSwitchTriggered);
    auto confirmed = SpotFundsLifecycleOrder(account);
    confirmed.review.emplace().confirmed = true;
    const auto confirmedReport = engine.ExecutePreTradeDryRun(confirmed);
    ASSERT_TRUE(confirmedReport.Passed());
    EXPECT_TRUE(confirmedReport.Reviews().empty());
    auto confirmedLive = engine.ExecutePreTrade(confirmed);
    ASSERT_TRUE(confirmedLive.Passed());
    EXPECT_TRUE(confirmedLive.reservation->Reviews().empty());
    confirmedLive.reservation->Rollback();
    const auto breached = engine.Configure().SetSpotFundsAccountPnl(
        policies::SpotFundsPolicyName, account,
        openpit::param::Pnl::FromString("-150"));
    ASSERT_EQ(breached.accountBlocks.size(), 1U);
    EXPECT_EQ(breached.accountBlocks.front().code,
              RejectCode::PnlKillSwitchTriggered);
    engine.Accounts().Unblock(account);
    const auto rejectedReport =
        engine.ExecutePreTradeDryRun(SpotFundsLifecycleOrder(account));
    ASSERT_FALSE(rejectedReport.Passed());
    ASSERT_EQ(rejectedReport.Rejects().size(), 1U);
    EXPECT_EQ(rejectedReport.Rejects().front().code,
              RejectCode::PnlKillSwitchTriggered);
    EXPECT_EQ(rejectedReport.Rejects().front().mode, RuleMode::Reject);
    EXPECT_TRUE(rejectedReport.Reviews().empty());
    for (const bool isConfirmed : {false, true}) {
      engine.Accounts().Unblock(account);
      auto order = SpotFundsLifecycleOrder(account);
      order.review.emplace().confirmed = isConfirmed;
      const auto rejected = engine.ExecutePreTrade(order);
      ASSERT_FALSE(rejected.Passed());
      ASSERT_EQ(rejected.rejects.size(), 1U);
      EXPECT_EQ(rejected.rejects.front().code,
                RejectCode::PnlKillSwitchTriggered);
      EXPECT_EQ(rejected.rejects.front().mode, RuleMode::Reject);
      const auto blocked = engine.ExecutePreTrade(order);
      ASSERT_EQ(blocked.rejects.size(), 1U);
      EXPECT_EQ(blocked.rejects.front().code, RejectCode::AccountBlocked);
    }
  }
}

TEST(BuiltinPolicy, SpotFundsPnlTierDuplicatesAreAtomic) {
  const auto account = openpit::param::AccountId::FromUint64(83303);
  const auto group = openpit::param::AccountGroupId::FromUint32(83304);
  for (const bool accountTier : {false, true}) {
    SCOPED_TRACE(accountTier);
    policies::SpotFundsPnlBoundsBarrier barrier(openpit::param::Asset("USD"));
    barrier.mode = RuleMode::ReviewWithHold;
    barrier.lowerBound = openpit::param::Pnl::FromString("-10");
    const policies::SpotFundsPnlBoundsAccountGroupBarrier groupBarrier(group,
                                                                       barrier);
    const policies::SpotFundsPnlBoundsAccountBarrier accountBarrier(account,
                                                                    barrier);
    openpit::EngineBuilder invalidBuilder(openpit::SyncPolicy::None);
    auto duplicate = policies::SpotFundsPnlBoundsKillSwitchPolicy{};
    if (accountTier) {
      duplicate.AccountBarrier(accountBarrier).AccountBarrier(accountBarrier);
    } else {
      duplicate.AccountGroupBarrier(groupBarrier)
          .AccountGroupBarrier(groupBarrier);
    }
    EXPECT_THROW(duplicate.AddTo(invalidBuilder), openpit::Error);

    auto policy = policies::SpotFundsPnlBoundsKillSwitchPolicy{};
    if (accountTier) {
      policy.AccountBarrier(accountBarrier);
    } else {
      policy.AccountGroupBarrier(groupBarrier);
    }
    openpit::EngineBuilder builder(openpit::SyncPolicy::None);
    builder.Add(policy);
    const auto engine = builder.Build();
    ASSERT_FALSE(engine.Accounts().RegisterGroup({account}, group).has_value());
    SeedSpotFundsLifecycleAccount(engine, account);
    ASSERT_TRUE(
        engine.Configure()
            .SetSpotFundsAccountPnl(policies::SpotFundsPolicyName, account,
                                    openpit::param::Pnl::FromString("-50"))
            .accountBlocks.empty());
    const auto check = [&] {
      const auto report =
          engine.ExecutePreTradeDryRun(SpotFundsLifecycleOrder(account));
      ASSERT_TRUE(report.Passed());
      ASSERT_EQ(report.Reviews().size(), 1U);
      EXPECT_EQ(report.Reviews().front().mode, RuleMode::ReviewWithHold);
      EXPECT_EQ(report.Reviews().front().code,
                RejectCode::PnlKillSwitchTriggered);
    };
    check();
    auto replacement = barrier;
    replacement.lowerBound = openpit::param::Pnl::FromString("-100");
    try {
      if (accountTier) {
        static_cast<void>(engine.Configure().SpotFundsPnlBoundsKillSwitch(
            policies::SpotFundsPolicyName, std::nullopt, std::nullopt,
            std::vector<policies::SpotFundsPnlBoundsAccountBarrier>{
                {account, replacement}, {account, replacement}}));
      } else {
        static_cast<void>(engine.Configure().SpotFundsPnlBoundsKillSwitch(
            policies::SpotFundsPolicyName, std::nullopt,
            std::vector<policies::SpotFundsPnlBoundsAccountGroupBarrier>{
                {group, replacement}, {group, replacement}}));
      }
      FAIL() << "duplicate tier key must fail";
    } catch (const openpit::ConfigureError& error) {
      EXPECT_EQ(error.Kind(), openpit::ConfigureErrorKind::Validation);
    }
    check();
  }
}

TEST(BuiltinPolicy, SpotFundsReviewOnlyPnlEvaluationFailuresBlock) {
  for (const auto mode :
       {RuleMode::ReviewWithHold, RuleMode::ReviewWithoutHold}) {
    for (const bool mismatch : {false, true}) {
      for (const bool confirmed : {false, true}) {
        SCOPED_TRACE(testing::Message() << static_cast<int>(mode) << ":"
                                        << mismatch << ":" << confirmed);
        const auto account = openpit::param::AccountId::FromUint64(83305);
        policies::SpotFundsPnlBoundsBarrier barrier(
            openpit::param::Asset(mismatch ? "EUR" : "USD"));
        barrier.mode = mode;
        barrier.lowerBound = openpit::param::Pnl::FromString("-100");
        openpit::EngineBuilder builder(openpit::SyncPolicy::None);
        builder.Add(
            policies::SpotFundsPnlBoundsKillSwitchPolicy{}.AccountBarrier(
                {account, barrier}));
        const auto engine = builder.Build();
        SeedSpotFundsLifecycleAccount(engine, account);
        const auto before = SpotFundsBalances(engine, account);
        const auto configured =
            mismatch ? engine.Configure().SetSpotFundsAccountPnl(
                           policies::SpotFundsPolicyName, account,
                           openpit::param::Pnl::FromString("0"))
                     : engine.Configure().SetSpotFundsAccountPnl(
                           policies::SpotFundsPolicyName, account,
                           openpit::accountadjustment::PnlHaltReason::
                               MissingCostBasis);
        ASSERT_EQ(configured.accountBlocks.size(), 1U);
        EXPECT_EQ(configured.accountBlocks.front().code,
                  RejectCode::PnlKillSwitchTriggered);
        engine.Accounts().Unblock(account);
        auto order = SpotFundsLifecycleOrder(account);
        order.review.emplace().confirmed = confirmed;
        const auto result = engine.ExecutePreTrade(order);
        ASSERT_FALSE(result.Passed());
        EXPECT_FALSE(result.reservation.has_value());
        ASSERT_EQ(result.rejects.size(), 1U);
        EXPECT_EQ(result.rejects.front().code,
                  RejectCode::PnlKillSwitchTriggered);
        EXPECT_EQ(result.rejects.front().mode, RuleMode::Reject);
        if (mismatch) {
          barrier.currency = openpit::param::Asset("USD");
          EXPECT_TRUE(
              engine.Configure()
                  .SpotFundsPnlBoundsKillSwitch(
                      policies::SpotFundsPolicyName, std::nullopt, std::nullopt,
                      std::vector<policies::SpotFundsPnlBoundsAccountBarrier>{
                          {account, barrier}})
                  .accountBlocks.empty());
        } else {
          EXPECT_TRUE(engine.Configure()
                          .SetSpotFundsAccountPnl(
                              policies::SpotFundsPolicyName, account,
                              openpit::param::Pnl::FromString("0"))
                          .accountBlocks.empty());
        }
        const auto blocked = engine.ExecutePreTrade(order);
        ASSERT_EQ(blocked.rejects.size(), 1U);
        EXPECT_EQ(blocked.rejects.front().code, RejectCode::AccountBlocked);
        EXPECT_EQ(SpotFundsBalances(engine, account), before);
      }
    }
  }
}

TEST(BuiltinPolicy, SpotFundsPnlDuplicateModeFailsBuild) {
  policies::SpotFundsPnlBoundsBarrier barrier(openpit::param::Asset("USD"));
  barrier.mode = RuleMode::ReviewWithHold;
  barrier.lowerBound = openpit::param::Pnl::FromString("-10");
  openpit::EngineBuilder validBuilder(openpit::SyncPolicy::None);
  const auto valid =
      policies::SpotFundsPnlBoundsKillSwitchPolicy{}.GlobalBarrier(barrier);
  ASSERT_NO_THROW(valid.AddTo(validBuilder));
  ASSERT_NO_THROW(static_cast<void>(validBuilder.Build()));
  openpit::EngineBuilder builder(openpit::SyncPolicy::None);
  const auto config = policies::SpotFundsPnlBoundsKillSwitchPolicy{}
                          .GlobalBarrier(barrier)
                          .GlobalBarrier(barrier);
  EXPECT_THROW(config.AddTo(builder), openpit::Error);
}

TEST(BuiltinPolicy, SpotFundsPnlGlobalListsPatchAllModes) {
  policies::SpotFundsPnlBoundsBarrier rejecting(openpit::param::Asset("USD"));
  rejecting.lowerBound = openpit::param::Pnl::FromString("-100");
  auto reviewing = rejecting;
  reviewing.mode = RuleMode::ReviewWithHold;
  reviewing.lowerBound = openpit::param::Pnl::FromString("-10");
  openpit::EngineBuilder builder(openpit::SyncPolicy::None);
  builder.Add(
      policies::SpotFundsPnlBoundsKillSwitchPolicy{}.GlobalBarrier(rejecting));
  const auto engine = builder.Build();
  const auto account = openpit::param::AccountId::FromUint64(83302);
  SeedSpotFundsLifecycleAccount(engine, account);
  const auto seeded = engine.Configure().SetSpotFundsAccountPnl(
      policies::SpotFundsPolicyName, account,
      openpit::param::Pnl::FromString("-50"));
  ASSERT_TRUE(seeded.accountBlocks.empty());
  const auto checkReviews = [&](std::size_t count) {
    const auto report =
        engine.ExecutePreTradeDryRun(SpotFundsLifecycleOrder(account));
    ASSERT_TRUE(report.Passed());
    const auto reviews = report.Reviews();
    ASSERT_EQ(reviews.size(), count);
    if (count != 0) {
      EXPECT_EQ(reviews.front().mode, RuleMode::ReviewWithHold);
    }
  };
  const auto update =
      [&](std::optional<std::vector<policies::SpotFundsPnlBoundsBarrier>>
              barriers) {
        const auto result = engine.Configure().SpotFundsPnlBoundsKillSwitch(
            policies::SpotFundsPolicyName, std::move(barriers));
        EXPECT_TRUE(result.accountBlocks.empty());
      };
  update(
      std::vector<policies::SpotFundsPnlBoundsBarrier>{rejecting, reviewing});
  checkReviews(1);
  update(std::nullopt);
  checkReviews(1);
  update(std::vector<policies::SpotFundsPnlBoundsBarrier>{reviewing});
  update(
      std::vector<policies::SpotFundsPnlBoundsBarrier>{rejecting, reviewing});
  try {
    update(
        std::vector<policies::SpotFundsPnlBoundsBarrier>{reviewing, reviewing});
    FAIL() << "duplicate mode must fail configure";
  } catch (const openpit::ConfigureError& error) {
    EXPECT_EQ(error.Kind(), openpit::ConfigureErrorKind::Validation);
  }
  checkReviews(1);
  const auto survived = engine.Configure().SetSpotFundsAccountPnl(
      policies::SpotFundsPolicyName, account,
      openpit::param::Pnl::FromString("-150"));
  ASSERT_EQ(survived.accountBlocks.size(), 1U);
  EXPECT_EQ(survived.accountBlocks.front().code,
            RejectCode::PnlKillSwitchTriggered);
  const auto restored = engine.Configure().SetSpotFundsAccountPnl(
      policies::SpotFundsPolicyName, account,
      openpit::param::Pnl::FromString("-50"));
  EXPECT_TRUE(restored.accountBlocks.empty());
  engine.Accounts().Unblock(account);
  update(std::vector<policies::SpotFundsPnlBoundsBarrier>{rejecting});
  checkReviews(0);
  update(
      std::vector<policies::SpotFundsPnlBoundsBarrier>{rejecting, reviewing});
  update(std::vector<policies::SpotFundsPnlBoundsBarrier>{});
  checkReviews(0);
  const auto cleared = engine.Configure().SetSpotFundsAccountPnl(
      policies::SpotFundsPolicyName, account,
      openpit::param::Pnl::FromString("-150"));
  EXPECT_TRUE(cleared.accountBlocks.empty());
  checkReviews(0);
}

TEST(BuiltinPolicy, SpotFundsPnlBoundsBarrierIgnoresNonMatchingCurrency) {
  const openpit::param::AccountId account =
      openpit::param::AccountId::FromUint64(83019);
  policies::SpotFundsPnlBoundsBarrier barrier(openpit::param::Asset("EUR"));
  barrier.lowerBound = openpit::param::Pnl::FromString("-100");
  openpit::EngineBuilder builder(openpit::SyncPolicy::None);
  builder.Add(
      policies::SpotFundsPnlBoundsKillSwitchPolicy{}.GlobalBarrier(barrier));
  const openpit::Engine engine = builder.Build();
  engine.Accounts().SetCurrency(account, openpit::param::Asset("USD"));

  const auto result = engine.Configure().SetSpotFundsAccountPnl(
      policies::SpotFundsPolicyName, account,
      openpit::param::Pnl::FromString("-150"));
  EXPECT_TRUE(result.accountBlocks.empty());
}

TEST(BuiltinPolicy,
     SpotFundsPnlBoundsAccountBarrierRejectsNonMatchingCurrency) {
  const openpit::param::AccountId account =
      openpit::param::AccountId::FromUint64(83022);
  policies::SpotFundsPnlBoundsBarrier barrier(openpit::param::Asset("EUR"));
  barrier.lowerBound = openpit::param::Pnl::FromString("-100");
  openpit::EngineBuilder builder(openpit::SyncPolicy::None);
  builder.Add(policies::SpotFundsPnlBoundsKillSwitchPolicy{}.AccountBarrier(
      policies::SpotFundsPnlBoundsAccountBarrier(account, barrier)));
  const openpit::Engine engine = builder.Build();
  engine.Accounts().SetCurrency(account, openpit::param::Asset("USD"));

  const auto result = engine.Configure().SetSpotFundsAccountPnl(
      policies::SpotFundsPolicyName, account,
      openpit::param::Pnl::FromString("0"));
  ASSERT_EQ(result.accountBlocks.size(), 1U);
  EXPECT_EQ(result.accountBlocks[0].code, RejectCode::PnlKillSwitchTriggered);
  EXPECT_EQ(result.accountBlocks[0].reason, "pnl barrier currency mismatch");

  const openpit::pretrade::ExecuteResult blocked =
      engine.ExecutePreTrade(SpotFundsLifecycleOrder(account));
  EXPECT_FALSE(blocked.Passed());
  ASSERT_EQ(blocked.rejects.size(), 1U);
  EXPECT_EQ(blocked.rejects[0].code, RejectCode::PnlKillSwitchTriggered);
  EXPECT_EQ(blocked.rejects[0].reason, "pnl barrier currency mismatch");
}

TEST(BuiltinPolicy, SpotFundsPnlBoundsBarrierAppliesMatchingCurrency) {
  const openpit::param::AccountId account =
      openpit::param::AccountId::FromUint64(83020);
  policies::SpotFundsPnlBoundsBarrier barrier(openpit::param::Asset("USD"));
  barrier.lowerBound = openpit::param::Pnl::FromString("-100");
  openpit::EngineBuilder builder(openpit::SyncPolicy::None);
  builder.Add(
      policies::SpotFundsPnlBoundsKillSwitchPolicy{}.GlobalBarrier(barrier));
  const openpit::Engine engine = builder.Build();
  engine.Accounts().SetCurrency(account, openpit::param::Asset("USD"));

  const auto result = engine.Configure().SetSpotFundsAccountPnl(
      policies::SpotFundsPolicyName, account,
      openpit::param::Pnl::FromString("-150"));
  ASSERT_EQ(result.accountBlocks.size(), 1U);
  EXPECT_EQ(result.accountBlocks[0].code, RejectCode::PnlKillSwitchTriggered);
}

TEST(BuiltinPolicy, SpotFundsPnlBoundsPolicyBuildsWithAllBarrierAxes) {
  openpit::EngineBuilder builder(openpit::SyncPolicy::Full);

  policies::SpotFundsPnlBoundsBarrier global(openpit::param::Asset("USD"));
  global.lowerBound = openpit::param::Pnl::FromString("-1000");

  policies::SpotFundsPnlBoundsBarrier group(openpit::param::Asset("USD"));
  group.upperBound = openpit::param::Pnl::FromString("1000");

  policies::SpotFundsPnlBoundsBarrier account(openpit::param::Asset("USD"));
  account.lowerBound = openpit::param::Pnl::FromString("-250");

  builder.Add(
      policies::SpotFundsPnlBoundsKillSwitchPolicy{}
          .GlobalBarrier(std::move(global))
          .AccountGroupBarrier(policies::SpotFundsPnlBoundsAccountGroupBarrier(
              openpit::param::AccountGroupId::FromUint32(7), std::move(group)))
          .AccountBarrier(policies::SpotFundsPnlBoundsAccountBarrier(
              openpit::param::AccountId::FromUint64(99224416),
              std::move(account))));

  EXPECT_NO_THROW({ openpit::Engine engine = builder.Build(); });
}

TEST(BuiltinPolicy, SpotFundsPnlBoundsPolicyRequiresBarrier) {
  openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
  EXPECT_THROW(
      { builder.Add(policies::SpotFundsPnlBoundsKillSwitchPolicy{}); },
      openpit::Error);
}

TEST(BuiltinPolicy, SpotFundsGroupMembershipArmsEffectivePnlBarrier) {
  const openpit::param::AccountId account =
      openpit::param::AccountId::FromUint64(83010);
  const openpit::param::AccountGroupId group =
      openpit::param::AccountGroupId::FromUint32(84);
  policies::SpotFundsPnlBoundsBarrier barrier(openpit::param::Asset("USD"));
  barrier.lowerBound = openpit::param::Pnl::FromString("1");
  openpit::EngineBuilder builder(openpit::SyncPolicy::None);
  builder.Add(
      policies::SpotFundsPnlBoundsKillSwitchPolicy{}.AccountGroupBarrier(
          policies::SpotFundsPnlBoundsAccountGroupBarrier(group, barrier)));
  const openpit::Engine engine = builder.Build();

  ASSERT_FALSE(engine.Accounts().RegisterGroup({account}, group).has_value());
  ExpectSpotFundsPnlPreTradeReject(engine, account);
}

TEST(BuiltinPolicy, SpotFundsPnlHaltBlocksAndNumericSetRearms) {
  const openpit::param::AccountId account =
      openpit::param::AccountId::FromUint64(83016);

  policies::SpotFundsPnlBoundsBarrier barrier(openpit::param::Asset("USD"));
  barrier.lowerBound = openpit::param::Pnl::FromString("-100");
  openpit::EngineBuilder builder(openpit::SyncPolicy::None);
  builder.Add(
      policies::SpotFundsPnlBoundsKillSwitchPolicy{}.GlobalBarrier(barrier));
  const openpit::Engine engine = builder.Build();

  openpit::pretrade::ExecuteResult execution =
      engine.ExecutePreTrade(SpotFundsLifecycleOrder(account));
  ASSERT_TRUE(execution.Passed());
  openpit::pretrade::PreTradeLock lock = execution.reservation->Lock();
  execution.reservation->Commit();

  openpit::model::ExecutionReportOperation operation;
  operation.instrument = openpit::model::Instrument(
      ::openpit::param::Asset("AAPL"), ::openpit::param::Asset("USD"));
  operation.accountId = account;
  operation.side = openpit::model::Side::Buy;
  openpit::model::Fill fill;
  fill.lastTrade = openpit::model::Trade(Price::FromString("100"),
                                         Quantity::FromString("1"));
  // The fee has to be denominated in the account currency, so it is what makes
  // this fill's account line uncomputable without one. A fee-less opening fill
  // would omit the account line instead.
  fill.fee = openpit::param::MonetaryAmount(
      openpit::param::Fee::FromString("0.25"), openpit::param::Asset("USD"));
  fill.remainingReservedQuantity = Quantity::FromString("0");
  fill.isFinal = true;
  fill.lock = std::move(lock);
  openpit::model::ExecutionReport report;
  report.operation = std::move(operation);
  report.fill = std::move(fill);

  const openpit::PostTradeResult halted = engine.ApplyExecutionReport(report);
  ASSERT_EQ(halted.accountPnls.size(), 1u);
  ASSERT_TRUE(halted.accountPnls.front().HaltReason().has_value());
  EXPECT_EQ(*halted.accountPnls.front().HaltReason(),
            openpit::accountadjustment::PnlHaltReason::MissingAccountCurrency);
  ASSERT_EQ(halted.accountBlocks.size(), 1u);
  ExpectSpotFundsPnlPreTradeReject(engine, account);

  const openpit::PolicyConfigurationResult rearmed =
      engine.Configure().SetSpotFundsAccountPnl(
          policies::SpotFundsPolicyName, account,
          openpit::param::Pnl::FromString("0"));
  EXPECT_TRUE(rearmed.accountBlocks.empty());

  // Re-arming restores the accumulator; the separately latched account block
  // is lifted explicitly before the next order.
  engine.Accounts().Unblock(account);
  openpit::pretrade::ExecuteResult accepted =
      engine.ExecutePreTrade(SpotFundsLifecycleOrder(account));
  ASSERT_TRUE(accepted.Passed());
  accepted.reservation->Rollback();
}

TEST(BuiltinPolicy, SpotFundsPnlBoundsRuntimeBarrierRechecksChangedCurrency) {
  const openpit::param::AccountId account =
      openpit::param::AccountId::FromUint64(83021);
  openpit::EngineBuilder builder(openpit::SyncPolicy::None);
  builder.Add(policies::SpotFundsPolicy{});
  const openpit::Engine engine = builder.Build();
  engine.Accounts().SetCurrency(account, openpit::param::Asset("USD"));

  const auto seeded = engine.Configure().SetSpotFundsAccountPnl(
      policies::SpotFundsPolicyName, account,
      openpit::param::Pnl::FromString("-150"));
  EXPECT_TRUE(seeded.accountBlocks.empty());

  policies::SpotFundsPnlBoundsBarrier mismatched(openpit::param::Asset("EUR"));
  mismatched.lowerBound = openpit::param::Pnl::FromString("-100");
  const auto first = engine.Configure().SpotFundsPnlBoundsKillSwitch(
      policies::SpotFundsPolicyName,
      std::vector<policies::SpotFundsPnlBoundsBarrier>{mismatched});
  EXPECT_TRUE(first.accountBlocks.empty());

  policies::SpotFundsPnlBoundsBarrier matching(openpit::param::Asset("USD"));
  matching.lowerBound = openpit::param::Pnl::FromString("-100");
  const auto second = engine.Configure().SpotFundsPnlBoundsKillSwitch(
      policies::SpotFundsPolicyName,
      std::vector<policies::SpotFundsPnlBoundsBarrier>{matching});
  ASSERT_EQ(second.accountBlocks.size(), 1U);
  EXPECT_EQ(second.accountBlocks[0].block.code,
            RejectCode::PnlKillSwitchTriggered);
}

// Applies one AAPL/USD buy fill through the full reserve/commit lifecycle.
// A nonzero `fee` engages the account line's need for an account currency.
[[nodiscard]] openpit::PostTradeResult ApplySpotFundsBuyFill(
    const openpit::Engine& engine, const openpit::param::AccountId accountId,
    const std::optional<openpit::param::MonetaryAmount>& fee) {
  openpit::pretrade::ExecuteResult execution =
      engine.ExecutePreTrade(SpotFundsLifecycleOrder(accountId));
  if (!execution.Passed()) {
    ADD_FAILURE() << "ExecutePreTrade() rejects = " << execution.rejects.size()
                  << ", want none";
    return {};
  }
  openpit::pretrade::PreTradeLock lock = execution.reservation->Lock();
  execution.reservation->Commit();

  openpit::model::ExecutionReportOperation operation;
  operation.instrument = openpit::model::Instrument(
      ::openpit::param::Asset("AAPL"), ::openpit::param::Asset("USD"));
  operation.accountId = accountId;
  operation.side = openpit::model::Side::Buy;
  openpit::model::Fill fill;
  fill.lock = std::move(lock);
  fill.lastTrade = openpit::model::Trade(Price::FromString("100"),
                                         Quantity::FromString("1"));
  fill.fee = fee;
  fill.remainingReservedQuantity = Quantity::FromString("0");
  fill.isFinal = true;
  openpit::model::ExecutionReport report;
  report.operation = std::move(operation);
  report.fill = std::move(fill);
  return engine.ApplyExecutionReport(report);
}

[[nodiscard]] openpit::Engine SpotFundsPnlKillSwitchEngine() {
  policies::SpotFundsPnlBoundsBarrier barrier(openpit::param::Asset("USD"));
  barrier.lowerBound = openpit::param::Pnl::FromString("-100");
  openpit::EngineBuilder builder(openpit::SyncPolicy::None);
  builder.Add(
      policies::SpotFundsPnlBoundsKillSwitchPolicy{}.GlobalBarrier(barrier));
  return builder.Build();
}

[[nodiscard]] const openpit::accountadjustment::AccountOutcomeEntry*
FindAssetEntry(const openpit::PostTradeResult& result,
               const std::string_view asset) {
  const auto outcome = std::find_if(
      result.accountAdjustments.begin(), result.accountAdjustments.end(),
      [asset](const openpit::accountadjustment::Outcome& candidate) {
        return candidate.entry.asset.View() == asset;
      });
  if (outcome == result.accountAdjustments.end()) {
    return nullptr;
  }
  return &outcome->entry;
}

// The account line and the position ledger halt independently. A fee-less
// opening fill does not engage the account line, while the position ledger
// must still denominate a cost basis and therefore halts. The ledger halt is
// also emitted exactly once: a second identical fill stays halted without
// republishing the reason.
TEST(BuiltinPolicy, SpotFundsOpeningFillHaltsPositionLedgerOnly) {
  const openpit::param::AccountId account =
      openpit::param::AccountId::FromUint64(83018);
  const openpit::Engine engine = SpotFundsPnlKillSwitchEngine();

  const openpit::PostTradeResult result =
      ApplySpotFundsBuyFill(engine, account, std::nullopt);
  EXPECT_TRUE(result.accountPnls.empty());
  EXPECT_TRUE(result.accountBlocks.empty());

  const openpit::accountadjustment::AccountOutcomeEntry* entry =
      FindAssetEntry(result, "AAPL");
  ASSERT_NE(entry, nullptr);
  ASSERT_TRUE(entry->realizedPnl.has_value());
  ASSERT_TRUE(entry->realizedPnl->HaltReason().has_value());
  EXPECT_EQ(*entry->realizedPnl->HaltReason(),
            openpit::accountadjustment::PnlHaltReason::MissingAccountCurrency);
  // A position ledger halted before its account-currency cost basis can be
  // computed cannot retain an authoritative average price.
  EXPECT_FALSE(entry->averageEntryPrice.has_value());

  // The account line remains unset, so the account is still tradable and the
  // next fill is accepted rather than rejected by the kill switch.
  const openpit::PostTradeResult second =
      ApplySpotFundsBuyFill(engine, account, std::nullopt);
  EXPECT_TRUE(second.accountPnls.empty());
  EXPECT_TRUE(second.accountBlocks.empty());

  // The ledger stays halted, so it does not emit the reason a second time.
  const openpit::accountadjustment::AccountOutcomeEntry* secondEntry =
      FindAssetEntry(second, "AAPL");
  ASSERT_NE(secondEntry, nullptr);
  EXPECT_FALSE(secondEntry->realizedPnl.has_value());
}

TEST(BuiltinPolicy, SpotFundsPnlBoundsConfiguratorUpdatesAxesAndPnl) {
  using GlobalBarriers =
      std::optional<std::vector<policies::SpotFundsPnlBoundsBarrier>>;
  static_assert(!std::is_constructible_v<GlobalBarriers,
                                         policies::SpotFundsPnlBoundsBarrier>);

  openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
  policies::SpotFundsPnlBoundsBarrier account(openpit::param::Asset("USD"));
  account.lowerBound = openpit::param::Pnl::FromString("-10");
  builder.Add(policies::SpotFundsPnlBoundsKillSwitchPolicy{}.AccountBarrier(
      policies::SpotFundsPnlBoundsAccountBarrier(
          openpit::param::AccountId::FromUint64(99224416),
          std::move(account))));
  openpit::Engine engine = builder.Build();

  policies::SpotFundsPnlBoundsBarrier global(openpit::param::Asset("USD"));
  global.lowerBound = openpit::param::Pnl::FromString("-100");
  policies::SpotFundsPnlBoundsBarrier group(openpit::param::Asset("USD"));
  group.upperBound = openpit::param::Pnl::FromString("100");
  policies::SpotFundsPnlBoundsBarrier update(openpit::param::Asset("USD"));
  update.lowerBound = openpit::param::Pnl::FromString("-20");
  update.upperBound = openpit::param::Pnl::FromString("20");

  EXPECT_NO_THROW({
    const auto retune = engine.Configure().SpotFundsPnlBoundsKillSwitch(
        policies::SpotFundsPolicyName,
        std::vector<policies::SpotFundsPnlBoundsBarrier>{std::move(global)},
        std::vector<policies::SpotFundsPnlBoundsAccountGroupBarrier>{
            policies::SpotFundsPnlBoundsAccountGroupBarrier(
                openpit::param::AccountGroupId::FromUint32(7),
                std::move(group))},
        std::vector<policies::SpotFundsPnlBoundsAccountBarrier>{
            policies::SpotFundsPnlBoundsAccountBarrier(
                openpit::param::AccountId::FromUint64(99224416),
                std::move(update))});
    // No account carries a stored P&L yet, so the retune blocks nothing.
    EXPECT_TRUE(retune.accountBlocks.empty());
  });
  const auto numericResult = engine.Configure().SetSpotFundsAccountPnl(
      policies::SpotFundsPolicyName,
      openpit::param::AccountId::FromUint64(99224416),
      openpit::param::Pnl::FromString("2.5"));
  EXPECT_TRUE(numericResult.accountBlocks.empty());

  const auto haltedResult = engine.Configure().SetSpotFundsAccountPnl(
      policies::SpotFundsPolicyName,
      openpit::param::AccountId::FromUint64(99224416),
      openpit::accountadjustment::PnlHaltReason::MissingFx);
  ASSERT_EQ(haltedResult.accountBlocks.size(), 1U);
  EXPECT_EQ(haltedResult.accountBlocks[0].code,
            RejectCode::PnlKillSwitchTriggered);
}

TEST(BuiltinPolicy, SpotFundsPnlBoundsRuntimeAxisReplacementAndClear) {
  const openpit::param::AccountGroupId group =
      openpit::param::AccountGroupId::FromUint32(85);
  const openpit::param::AccountId accountSpecific =
      openpit::param::AccountId::FromUint64(83011);
  const openpit::param::AccountId accountGroup =
      openpit::param::AccountId::FromUint64(83012);
  const openpit::param::AccountId accountSafe =
      openpit::param::AccountId::FromUint64(83013);
  const openpit::param::AccountId accountGlobal =
      openpit::param::AccountId::FromUint64(83014);
  const openpit::param::AccountId accountAfterClear =
      openpit::param::AccountId::FromUint64(83015);

  openpit::EngineBuilder builder(openpit::SyncPolicy::None);
  builder.Add(policies::SpotFundsPolicy{});
  const openpit::Engine engine = builder.Build();
  for (const openpit::param::AccountId account :
       {accountSpecific, accountGroup, accountGlobal, accountAfterClear,
        accountSafe}) {
    SeedSpotFundsLifecycleAccount(engine, account);
  }
  ASSERT_FALSE(
      engine.Accounts().RegisterGroup({accountGroup}, group).has_value());

  static_cast<void>(engine.Configure().SetSpotFundsAccountPnl(
      policies::SpotFundsPolicyName, accountSpecific,
      openpit::param::Pnl::FromString("-15")));
  static_cast<void>(engine.Configure().SetSpotFundsAccountPnl(
      policies::SpotFundsPolicyName, accountGroup,
      openpit::param::Pnl::FromString("-15")));
  static_cast<void>(engine.Configure().SetSpotFundsAccountPnl(
      policies::SpotFundsPolicyName, accountGlobal,
      openpit::param::Pnl::FromString("-25")));
  static_cast<void>(engine.Configure().SetSpotFundsAccountPnl(
      policies::SpotFundsPolicyName, accountAfterClear,
      openpit::accountadjustment::PnlHaltReason::MissingFx));
  static_cast<void>(engine.Configure().SetSpotFundsAccountPnl(
      policies::SpotFundsPolicyName, accountSafe,
      openpit::param::Pnl::FromString("-5")));

  policies::SpotFundsPnlBoundsBarrier global(openpit::param::Asset("USD"));
  global.lowerBound = openpit::param::Pnl::FromString("-20");
  policies::SpotFundsPnlBoundsBarrier groupBarrier(
      openpit::param::Asset("USD"));
  groupBarrier.lowerBound = openpit::param::Pnl::FromString("-10");
  policies::SpotFundsPnlBoundsBarrier accountBarrier(
      openpit::param::Asset("USD"));
  accountBarrier.lowerBound = openpit::param::Pnl::FromString("-10");
  const auto armed = engine.Configure().SpotFundsPnlBoundsKillSwitch(
      policies::SpotFundsPolicyName,
      std::vector<policies::SpotFundsPnlBoundsBarrier>{global},
      std::vector<policies::SpotFundsPnlBoundsAccountGroupBarrier>{
          policies::SpotFundsPnlBoundsAccountGroupBarrier(group, groupBarrier)},
      std::vector<policies::SpotFundsPnlBoundsAccountBarrier>{
          policies::SpotFundsPnlBoundsAccountBarrier(accountSpecific,
                                                     accountBarrier)});
  // Four accounts sit past their effective bounds; the safe fifth account is
  // part of the sweep but remains unblocked.
  ASSERT_EQ(armed.accountBlocks.size(), 4U);
  const std::vector<std::pair<openpit::param::AccountId, RejectCode>>
      expectedPairs{
          {accountSpecific, RejectCode::PnlKillSwitchTriggered},
          {accountGroup, RejectCode::PnlKillSwitchTriggered},
          {accountGlobal, RejectCode::PnlKillSwitchTriggered},
          {accountAfterClear, RejectCode::PnlKillSwitchTriggered},
      };
  const std::vector<std::string_view> expectedReasons{
      "pnl kill switch triggered",
      "pnl kill switch triggered",
      "pnl kill switch triggered",
      "account pnl calculation halted",
  };
  for (std::size_t index = 0; index < expectedPairs.size(); ++index) {
    const auto actualPair = std::pair{armed.accountBlocks[index].accountId,
                                      armed.accountBlocks[index].block.code};
    EXPECT_EQ(actualPair, expectedPairs[index]);
    EXPECT_EQ(armed.accountBlocks[index].block.reason, expectedReasons[index]);
  }
  for (const openpit::param::AccountId account :
       {accountSpecific, accountGroup, accountGlobal, accountAfterClear}) {
    ExpectSpotFundsPnlPreTradeReject(engine, account);
  }
  openpit::pretrade::ExecuteResult safe =
      engine.ExecutePreTrade(SpotFundsLifecycleOrder(accountSafe));
  ASSERT_TRUE(safe.Passed());
  ASSERT_TRUE(safe.reservation.has_value());
  safe.reservation->Rollback();
  // The operator lifts the arming blocks; the axes stay in force, so the
  // cascade below is decided by the live barriers.
  for (const openpit::param::AccountId account :
       {accountSpecific, accountGroup, accountGlobal, accountAfterClear}) {
    engine.Accounts().Unblock(account);
  }

  // An engaged empty account axis clears only per-account barriers. The
  // omitted global and group axes remain in force for their respective keys.
  // Only accountSpecific changes tier, and the wider global bound admits it.
  const auto cleared = engine.Configure().SpotFundsPnlBoundsKillSwitch(
      policies::SpotFundsPolicyName, std::nullopt, std::nullopt,
      std::vector<policies::SpotFundsPnlBoundsAccountBarrier>{});
  EXPECT_TRUE(cleared.accountBlocks.empty());
  EXPECT_TRUE(ApplySpotFundsLifecycleFill(engine, accountSpecific).empty());
  ExpectSpotFundsPnlPreTradeReject(engine, accountGroup);
  ExpectSpotFundsPnlPreTradeReject(engine, accountGlobal);

  // Runtime patches may clear every axis, unlike the explicit PnL batch
  // builder that requires at least one barrier at construction time. Clearing
  // creates no block.
  const auto fullClear = engine.Configure().SpotFundsPnlBoundsKillSwitch(
      policies::SpotFundsPolicyName,
      std::vector<policies::SpotFundsPnlBoundsBarrier>{},
      std::vector<policies::SpotFundsPnlBoundsAccountGroupBarrier>{},
      std::vector<policies::SpotFundsPnlBoundsAccountBarrier>{});
  EXPECT_TRUE(fullClear.accountBlocks.empty());
  EXPECT_TRUE(ApplySpotFundsLifecycleFill(engine, accountAfterClear).empty());
}

TEST(BuiltinPolicy, SpotFundsPnlBoundsRuntimeAdditionRetainsLivePnl) {
  const openpit::param::AccountId account =
      openpit::param::AccountId::FromUint64(83015);
  openpit::EngineBuilder builder(openpit::SyncPolicy::None);
  builder.Add(policies::SpotFundsPolicy{});
  const openpit::Engine engine = builder.Build();

  SeedSpotFundsLifecycleAccount(engine, account);
  EXPECT_TRUE(ApplySpotFundsLifecycleFill(engine, account).empty());

  const auto setResult = engine.Configure().SetSpotFundsAccountPnl(
      policies::SpotFundsPolicyName, account,
      openpit::param::Pnl::FromString("-40"));
  EXPECT_TRUE(setResult.accountBlocks.empty());

  // Arming the bound decides the already-stored -40 immediately.
  policies::SpotFundsPnlBoundsBarrier initial(openpit::param::Asset("USD"));
  initial.lowerBound = openpit::param::Pnl::FromString("-30");
  const auto armed = engine.Configure().SpotFundsPnlBoundsKillSwitch(
      policies::SpotFundsPolicyName,
      std::vector<policies::SpotFundsPnlBoundsBarrier>{initial});
  EXPECT_EQ(armed.accountBlocks.size(), 1U);
  engine.Accounts().Unblock(account);

  // Replacing the global bound must preserve the live accumulator instead of
  // resetting it.
  policies::SpotFundsPnlBoundsBarrier replacement(openpit::param::Asset("USD"));
  replacement.lowerBound = openpit::param::Pnl::FromString("-20");
  const auto replaced = engine.Configure().SpotFundsPnlBoundsKillSwitch(
      policies::SpotFundsPolicyName,
      std::vector<policies::SpotFundsPnlBoundsBarrier>{replacement});
  EXPECT_EQ(replaced.accountBlocks.size(), 1U);
  ExpectSpotFundsPnlPreTradeReject(engine, account);
}

//------------------------------------------------------------------------------
// A gmock seam over the policy callback: confirms the adapter dispatches the
// main-stage check to the underlying policy object exactly once on the happy
// path, and not at all on a type mismatch (SafeSlow throws first).
//
// gmock mock objects are neither copyable nor movable, while the adapter stores
// its policy by value, so the policy stored in the adapter is a thin movable
// handle forwarding to a separately-owned mock.

class CheckSink {
 public:
  MOCK_METHOD(void, OnCheck, (std::uint32_t lots), (const));
};

class MockBackedPolicy {
 public:
  explicit MockBackedPolicy(const CheckSink* sink) noexcept : m_sink(sink) {}

  [[nodiscard]] std::string_view Name() const noexcept { return "MockDesk"; }

  void PerformPreTradeCheck(const DeskOrder& order, const Context& context,
                            PolicyDecision& decision) const {
    static_cast<void>(context);
    static_cast<void>(decision);
    m_sink->OnCheck(order.lots);
  }

  [[nodiscard]] bool ApplyExecutionReport(
      const openpit::ExecutionReport& report) const {
    static_cast<void>(report);
    return false;
  }

 private:
  const CheckSink* m_sink;
};

using MockBackedAdapter = openpit::pretrade::PolicyAdapterWithSafeSlowArgType<
    MockBackedPolicy, DeskOrder, openpit::ExecutionReport>;

TEST(PolicyAdapterMock, DispatchesToPolicyOnMatch) {
  CheckSink sink;
  EXPECT_CALL(sink, OnCheck(testing::Eq(33u))).Times(1);

  MockBackedAdapter adapter{MockBackedPolicy{&sink}};
  DeskOrder order;
  order.lots = 33;
  const Context context(order);
  PolicyDecision decision;
  adapter.PerformPreTradeCheck(context, decision);
}

TEST(PolicyAdapterMock, DoesNotDispatchOnTypeMismatch) {
  CheckSink sink;
  EXPECT_CALL(sink, OnCheck(testing::_)).Times(0);

  MockBackedAdapter adapter{MockBackedPolicy{&sink}};
  openpit::model::Order foreign;
  const Context context(foreign);
  PolicyDecision decision;
  EXPECT_THROW(adapter.PerformPreTradeCheck(context, decision), openpit::Error);
}

}  // namespace
