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
using openpit::pretrade::MakeTypeMismatchReject;
using openpit::pretrade::PolicyDecision;
using openpit::pretrade::PreTradeLock;
using openpit::pretrade::PushReject;
using openpit::pretrade::Reject;
using openpit::pretrade::RejectCode;
using openpit::pretrade::RejectScope;

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
  PushReject(decision,
             MakeTypeMismatchReject("p", RejectScope::Order, RejectCode::Other,
                                    "reason", "Expected"));
  ASSERT_TRUE(decision.IsRejected());
  ASSERT_EQ(decision.rejects.size(), 1u);
  EXPECT_EQ(decision.rejects.front().policy, "p");
  EXPECT_EQ(decision.rejects.front().details, "Expected");
}

TEST(Reject, MakeTypeMismatchRejectCarriesFields) {
  const Reject reject = MakeTypeMismatchReject(
      "LossGuard", RejectScope::Account, RejectCode::InvalidFieldValue, "bad",
      "BrokerOrder");
  EXPECT_EQ(reject.policy, "LossGuard");
  EXPECT_EQ(reject.scope, RejectScope::Account);
  EXPECT_EQ(reject.code, RejectCode::InvalidFieldValue);
  EXPECT_EQ(reject.reason, "bad");
  EXPECT_EQ(reject.details, "BrokerOrder");
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
}

//------------------------------------------------------------------------------
// Custom policy via the adapter templates (SafeSlow).
//
// A client order payload deriving from openpit::Order, a client policy with the
// adapter-expected surface, and the PolicyAdapter / StartPolicyAdapter bridging
// them. The adapter produces a deterministic type-mismatch reject when the
// order is not the client type, and the happy path otherwise.

struct DeskOrder : public openpit::Order {
  std::uint32_t lots = 0;
};

class DeskPolicy {
 public:
  [[nodiscard]] std::string_view Name() const noexcept { return "DeskPolicy"; }

  [[nodiscard]] std::optional<Reject> CheckPreTradeStart(
      const DeskOrder& order) const {
    if (order.lots == 0) {
      return MakeTypeMismatchReject(Name(), RejectScope::Order,
                                    RejectCode::InvalidFieldValue,
                                    "lots must be non-zero", "non-zero lots");
    }
    return std::nullopt;
  }

  void PerformPreTradeCheck(const DeskOrder& order, const Context& context,
                            PolicyDecision& decision) const {
    static_cast<void>(context);
    if (order.lots > 100) {
      PushReject(decision,
                 MakeTypeMismatchReject(Name(), RejectScope::Order,
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

TEST(PolicyAdapter, SafeSlowMainStageRejectsOnTypeMismatch) {
  DeskMainAdapter adapter{DeskPolicy{}};
  // A plain model::Order is NOT a DeskOrder, so SafeSlow must produce a
  // deterministic type-mismatch reject rather than dispatch to the policy.
  openpit::model::Order foreign;
  const Context context(foreign);

  PolicyDecision decision;
  adapter.PerformPreTradeCheck(context, decision);
  ASSERT_TRUE(decision.IsRejected());
  EXPECT_EQ(decision.rejects.front().scope, RejectScope::Order);
  EXPECT_EQ(decision.rejects.front().code, RejectCode::Other);
  EXPECT_EQ(decision.rejects.front().policy, "DeskPolicy");
}

TEST(StartPolicyAdapter, SafeSlowStartStageHappyPathAndTypeMismatch) {
  DeskStartAdapter adapter{DeskPolicy{}};

  DeskOrder order;
  order.lots = 5;
  EXPECT_FALSE(adapter.CheckPreTradeStart(order).has_value());

  openpit::model::Order foreign;
  const std::optional<Reject> reject = adapter.CheckPreTradeStart(foreign);
  ASSERT_TRUE(reject.has_value());
  EXPECT_EQ(reject->code, RejectCode::Other);
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
    EXPECT_STREQ(error.what(), "reject scope is invalid");
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
  [[nodiscard]] std::vector<openpit::accounts::AccountBlock>
  ApplyExecutionReport(const openpit::pretrade::PostTradeContext& context,
                       const openpit::ExecutionReport& report,
                       openpit::pretrade::PostTradeAdjustments& adjustments,
                       openpit::pretrade::PostTradePnls& pnls) const {
    static_cast<void>(context);
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

  const openpit::model::ExecutionReport report;
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
    PushReject(decision,
               MakeTypeMismatchReject("DryRunHookPolicy", RejectScope::Order,
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
               MakeTypeMismatchReject(
                   "DropCopyBlockingPolicy", RejectScope::Account,
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
               MakeTypeMismatchReject("MissingFieldPolicy", RejectScope::Order,
                                      RejectCode::MissingRequiredField,
                                      "missing limit price",
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
    return MakeTypeMismatchReject(
        "DropCopyStartMutationPolicy", RejectScope::Order,
        RejectCode::MissingRequiredField, "missing field",
        "forced after start-stage mutation");
  }

 private:
  int* m_value;
  bool* m_sawDropCopy;
  bool m_fatal;
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
  EXPECT_EQ(counters.startDryRun, 1u);
  EXPECT_EQ(counters.mainDryRun, 1u);
  EXPECT_EQ(counters.start, 0u);
  EXPECT_EQ(counters.main, 0u);

  const openpit::pretrade::ExecuteResult result =
      engine.ExecutePreTrade(MakeDryRunHookOrder());
  EXPECT_FALSE(result.Passed());
  ASSERT_EQ(result.rejects.size(), 1u);
  EXPECT_EQ(result.rejects.front().code, RejectCode::Custom);
  EXPECT_EQ(counters.start, 1u);
  EXPECT_EQ(counters.main, 1u);
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

//------------------------------------------------------------------------------
// Built-in policy configuration construction + registration.

[[nodiscard]] openpit::model::Order SpotFundsLifecycleOrder(
    const openpit::param::AccountId accountId) {
  return openpit::model::Order::Limit(
      openpit::model::Instrument(::openpit::param::Asset("AAPL"),
                                 ::openpit::param::Asset("USD")),
      accountId, openpit::model::Side::Buy,
      openpit::model::TradeAmount::OfQuantity(Quantity::FromString("1")),
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

  openpit::model::ExecutionReportOperation operation;
  operation.instrument = openpit::model::Instrument(
      ::openpit::param::Asset("AAPL"), ::openpit::param::Asset("USD"));
  operation.accountId = accountId;
  operation.side = openpit::model::Side::Buy;

  openpit::model::Fill fill;
  fill.lock =
      std::make_shared<openpit::pretrade::PreTradeLock>(std::move(lock));
  fill.lastTrade = openpit::model::Trade(Price::FromString("100"),
                                         Quantity::FromString("1"));
  fill.leavesQuantity = Quantity::FromString("0");
  fill.isFinal = true;

  openpit::model::ExecutionReport report;
  report.operation = std::move(operation);
  report.fill = std::move(fill);
  return engine.ApplyExecutionReport(report).accountBlocks;
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
      policies::OrderSizeBrokerBarrier(policies::OrderSizeLimit(
          Quantity::FromString("100"), Volume::FromString("1000000"))));
  config.AddTo(builder);
  EXPECT_NO_THROW({ openpit::Engine engine = builder.Build(); });
}

TEST(BuiltinPolicy, OrderSizeLimitWithoutBarrierThrows) {
  openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
  const policies::OrderSizeLimitPolicy config;
  EXPECT_THROW(config.AddTo(builder), openpit::Error);
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
  policies::SpotFundsPnlBoundsBarrier barrier;
  barrier.lowerBound = openpit::param::Pnl::FromString("-1000");
  barrier.upperBound = openpit::param::Pnl::FromString("250");

  ASSERT_TRUE(barrier.lowerBound.has_value());
  ASSERT_TRUE(barrier.upperBound.has_value());
  EXPECT_EQ(barrier.lowerBound->ToString(), "-1000");
  EXPECT_EQ(barrier.upperBound->ToString(), "250");
}

TEST(BuiltinPolicy, SpotFundsPnlBoundsPolicyBuildsWithAllBarrierAxes) {
  openpit::EngineBuilder builder(openpit::SyncPolicy::Full);

  policies::SpotFundsPnlBoundsBarrier global;
  global.lowerBound = openpit::param::Pnl::FromString("-1000");

  policies::SpotFundsPnlBoundsBarrier group;
  group.upperBound = openpit::param::Pnl::FromString("1000");

  policies::SpotFundsPnlBoundsBarrier account;
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
  policies::SpotFundsPnlBoundsBarrier barrier;
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

  policies::SpotFundsPnlBoundsBarrier barrier;
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
  // The fee has to be denominated in the account currency, so it is what
  // makes this fill's account line uncomputable without one. A fee-less
  // opening fill would omit the account line instead.
  fill.fee = openpit::param::MonetaryAmount(
      openpit::param::Fee::FromString("0.25"), openpit::param::Asset("USD"));
  fill.leavesQuantity = Quantity::FromString("0");
  fill.isFinal = true;
  fill.lock =
      std::make_shared<openpit::pretrade::PreTradeLock>(std::move(lock));
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

  engine.Accounts().SetCurrency(account, openpit::param::Asset("USD"));
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
  fill.lock =
      std::make_shared<openpit::pretrade::PreTradeLock>(std::move(lock));
  fill.lastTrade = openpit::model::Trade(Price::FromString("100"),
                                         Quantity::FromString("1"));
  fill.fee = fee;
  fill.leavesQuantity = Quantity::FromString("0");
  fill.isFinal = true;
  openpit::model::ExecutionReport report;
  report.operation = std::move(operation);
  report.fill = std::move(fill);
  return engine.ApplyExecutionReport(report);
}

[[nodiscard]] openpit::Engine SpotFundsPnlKillSwitchEngine() {
  policies::SpotFundsPnlBoundsBarrier barrier;
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
  using GlobalBarrierUpdate = policies::SpotFundsPnlBoundsGlobalBarrierUpdate;
  static_assert(!std::is_default_constructible_v<GlobalBarrierUpdate>);
  static_assert(!std::is_constructible_v<GlobalBarrierUpdate, std::nullopt_t>);
  static_assert(!std::is_constructible_v<GlobalBarrierUpdate,
                                         policies::SpotFundsPnlBoundsBarrier>);

  openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
  policies::SpotFundsPnlBoundsBarrier account;
  account.lowerBound = openpit::param::Pnl::FromString("-10");
  builder.Add(policies::SpotFundsPnlBoundsKillSwitchPolicy{}.AccountBarrier(
      policies::SpotFundsPnlBoundsAccountBarrier(
          openpit::param::AccountId::FromUint64(99224416),
          std::move(account))));
  openpit::Engine engine = builder.Build();

  policies::SpotFundsPnlBoundsBarrier global;
  global.lowerBound = openpit::param::Pnl::FromString("-100");
  policies::SpotFundsPnlBoundsBarrier group;
  group.upperBound = openpit::param::Pnl::FromString("100");
  policies::SpotFundsPnlBoundsBarrier update;
  update.lowerBound = openpit::param::Pnl::FromString("-20");
  update.upperBound = openpit::param::Pnl::FromString("20");

  EXPECT_NO_THROW({
    const auto retune = engine.Configure().SpotFundsPnlBoundsKillSwitch(
        policies::SpotFundsPolicyName,
        GlobalBarrierUpdate::Set(std::move(global)),
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

  policies::SpotFundsPnlBoundsBarrier global;
  global.lowerBound = openpit::param::Pnl::FromString("-20");
  policies::SpotFundsPnlBoundsBarrier groupBarrier;
  groupBarrier.lowerBound = openpit::param::Pnl::FromString("-10");
  policies::SpotFundsPnlBoundsBarrier accountBarrier;
  accountBarrier.lowerBound = openpit::param::Pnl::FromString("-10");
  const auto armed = engine.Configure().SpotFundsPnlBoundsKillSwitch(
      policies::SpotFundsPolicyName,
      policies::SpotFundsPnlBoundsGlobalBarrierUpdate::Set(global),
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
      policies::SpotFundsPolicyName,
      policies::SpotFundsPnlBoundsGlobalBarrierUpdate::Unchanged(),
      std::nullopt, std::vector<policies::SpotFundsPnlBoundsAccountBarrier>{});
  EXPECT_TRUE(cleared.accountBlocks.empty());
  EXPECT_TRUE(ApplySpotFundsLifecycleFill(engine, accountSpecific).empty());
  ExpectSpotFundsPnlPreTradeReject(engine, accountGroup);
  ExpectSpotFundsPnlPreTradeReject(engine, accountGlobal);

  // Runtime patches may clear every axis, unlike the explicit PnL batch
  // builder that requires at least one barrier at construction time. Clearing
  // creates no block.
  const auto fullClear = engine.Configure().SpotFundsPnlBoundsKillSwitch(
      policies::SpotFundsPolicyName,
      policies::SpotFundsPnlBoundsGlobalBarrierUpdate::Clear(),
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
  policies::SpotFundsPnlBoundsBarrier initial;
  initial.lowerBound = openpit::param::Pnl::FromString("-30");
  const auto armed = engine.Configure().SpotFundsPnlBoundsKillSwitch(
      policies::SpotFundsPolicyName,
      policies::SpotFundsPnlBoundsGlobalBarrierUpdate::Set(initial));
  EXPECT_EQ(armed.accountBlocks.size(), 1U);
  engine.Accounts().Unblock(account);

  // Replacing the global bound must preserve the live accumulator instead of
  // resetting it.
  policies::SpotFundsPnlBoundsBarrier replacement;
  replacement.lowerBound = openpit::param::Pnl::FromString("-20");
  const auto replaced = engine.Configure().SpotFundsPnlBoundsKillSwitch(
      policies::SpotFundsPolicyName,
      policies::SpotFundsPnlBoundsGlobalBarrierUpdate::Set(replacement));
  EXPECT_EQ(replaced.accountBlocks.size(), 1U);
  ExpectSpotFundsPnlPreTradeReject(engine, account);
}

//------------------------------------------------------------------------------
// A gmock seam over the policy callback: confirms the adapter dispatches the
// main-stage check to the underlying policy object exactly once on the happy
// path, and not at all on a type mismatch (SafeSlow short-circuits).
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
  adapter.PerformPreTradeCheck(context, decision);
  EXPECT_TRUE(decision.IsRejected());
}

}  // namespace
