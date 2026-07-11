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
// fixed underlying types; they must precede `openpit/adapters.hpp`, whose
// opaque forward declarations of those scoped enums are only a compatible
// redeclaration when the full definition is already in scope.
#include "openpit/pretrade/pretrade.hpp"

#include "openpit/adapters.hpp"
#include "openpit/engine.hpp"
#include "openpit/marketdata.hpp"
#include "openpit/model.hpp"
#include "openpit/reject.hpp"

#include <gmock/gmock.h>
#include <gtest/gtest.h>

#include <cstdint>
#include <optional>
#include <string>
#include <string_view>
#include <type_traits>
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
    !std::is_move_constructible_v<openpit::pretrade::PostTradeContext>);
static_assert(
    !std::is_move_constructible_v<openpit::pretrade::PostTradeAdjustments>);
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

TEST(PreTradeLock, RawRoundTripPreservesRecords) {
  PreTradeLock lock;
  lock.Push(kDefaultGroup, Price::FromString("99.99"));
  lock.Push(kGroupSeven, Price::FromString("0.001"));

  const std::vector<std::uint8_t> raw = lock.ToRaw();
  ASSERT_FALSE(raw.empty());

  const PreTradeLock restored = PreTradeLock::FromRaw(raw);
  EXPECT_EQ(restored.Len(), 2u);
  ASSERT_EQ(restored.PricesOf(kDefaultGroup).size(), 1u);
  EXPECT_EQ(restored.PricesOf(kDefaultGroup).front().ToString(), "99.99");
  ASSERT_EQ(restored.PricesOf(kGroupSeven).size(), 1u);
  EXPECT_EQ(restored.PricesOf(kGroupSeven).front().ToString(), "0.001");
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

[[nodiscard]] openpit::model::Order MakeDryRunHookOrder() {
  openpit::model::Order order;
  openpit::model::OrderOperation op;
  op.instrument = openpit::model::Instrument("AAPL", "USD");
  op.accountId = ::openpit::param::AccountId::FromUint64(42);
  op.side = openpit::model::Side::Buy;
  op.tradeAmount =
      openpit::model::TradeAmount::OfQuantity(Quantity::FromString("1"));
  op.price = Price::FromString("100");
  order.operation = std::move(op);
  return order;
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

//------------------------------------------------------------------------------
// Built-in policy configuration construction + registration.

[[nodiscard]] openpit::model::Order SpotFundsLifecycleOrder(
    const openpit::param::AccountId accountId) {
  return openpit::model::Order::Limit(
      openpit::model::Instrument("AAPL", "USD"), accountId,
      openpit::model::Side::Buy,
      openpit::model::TradeAmount::OfQuantity(Quantity::FromString("1")),
      Price::FromString("100"));
}

void SeedSpotFundsLifecycleAccount(const openpit::Engine& engine,
                                   const openpit::param::AccountId accountId) {
  engine.SetAccountCurrency(accountId, "USD");

  openpit::accountadjustment::AccountAdjustment seed;
  openpit::accountadjustment::BalanceOperation balance;
  balance.asset = "USD";
  seed.operation =
      openpit::accountadjustment::Operation::OfBalance(std::move(balance));
  openpit::accountadjustment::Amount amount;
  amount.balance = openpit::param::AdjustmentAmount::OfAbsolute(
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

  // The execution-report lock is a borrowed C ABI pointer. This follows the
  // existing PreTradeLock test fixture to carry the reservation snapshot into
  // the matching fill without changing the pending binding lock surface.
  openpit::pretrade::PreTradeLock lock(
      openpit_pretrade_pre_trade_reservation_get_lock(
          execution.reservation->Get()));
  execution.reservation->Commit();

  const openpit::model::Instrument instrument("AAPL", "USD");
  OpenPitExecutionReport report{};
  report.operation.is_set = true;
  report.operation.value.instrument = instrument.Raw();
  report.operation.value.account_id.value = accountId.Raw();
  report.operation.value.account_id.is_set = true;
  report.operation.value.side = OpenPitParamSide_Buy;
  report.fill.is_set = true;
  report.fill.value.last_trade.is_set = true;
  report.fill.value.last_trade.value.price = Price::FromString("100").Raw();
  report.fill.value.last_trade.value.quantity = Quantity::FromString("1").Raw();
  report.fill.value.leaves_quantity.is_set = true;
  report.fill.value.leaves_quantity.value = Quantity::FromString("0").Raw();
  report.fill.value.lock = lock.Get();
  report.fill.value.is_final.is_set = true;
  report.fill.value.is_final.value = true;

  OpenPitPretradeAccountBlockList* blocks = nullptr;
  OpenPitAccountAdjustmentOutcomeList* outcomes = nullptr;
  OpenPitSharedString* error = nullptr;
  const bool ok = openpit_engine_apply_execution_report(
      engine.Get(), &report, &blocks, &outcomes, &error);
  if (!ok) {
    if (blocks != nullptr) {
      openpit_pretrade_destroy_account_block_list(blocks);
    }
    if (outcomes != nullptr) {
      openpit_destroy_account_adjustment_outcome_list(outcomes);
    }
    openpit_destroy_shared_string(error);
    ADD_FAILURE() << "openpit_engine_apply_execution_report() failed";
    return {};
  }

  std::vector<openpit::accounts::AccountBlock> result;
  if (blocks != nullptr) {
    const std::size_t count = openpit_pretrade_account_block_list_len(blocks);
    result.reserve(count);
    for (std::size_t index = 0; index < count; ++index) {
      OpenPitPretradeAccountBlock block{};
      if (openpit_pretrade_account_block_list_get(blocks, index, &block)) {
        result.push_back(openpit::accounts::AccountBlock::FromRaw(block));
      }
    }
    openpit_pretrade_destroy_account_block_list(blocks);
  }
  openpit_destroy_account_adjustment_outcome_list(outcomes);
  return result;
}

void ExpectSpotFundsPnlBlock(
    const std::vector<openpit::accounts::AccountBlock>& blocks) {
  ASSERT_EQ(blocks.size(), 1u);
  EXPECT_EQ(blocks.front().code,
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
  policies::PnlBoundsBrokerBarrier barrier("USD");
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

TEST(BuiltinPolicy, SpotFundsPnlBoundsBarrierRawUsesAccountCurrency) {
  policies::SpotFundsPnlBoundsBarrier barrier("USD");
  barrier.lowerBound = openpit::param::Pnl::FromString("-1000");
  barrier.upperBound = openpit::param::Pnl::FromString("250");

  const OpenPitPretradePoliciesSpotFundsPnlBoundsBarrier raw = barrier.Raw();
  EXPECT_EQ(openpit::StringView(raw.account_currency).ToString(), "USD");
  ASSERT_TRUE(raw.lower_bound.is_set);
  ASSERT_TRUE(raw.upper_bound.is_set);
  EXPECT_EQ(openpit::param::Pnl::FromRaw(raw.lower_bound.value).ToString(),
            "-1000");
  EXPECT_EQ(openpit::param::Pnl::FromRaw(raw.upper_bound.value).ToString(),
            "250");
}

TEST(BuiltinPolicy, SpotFundsPnlBoundsPolicyBuildsWithAllBarrierAxes) {
  openpit::EngineBuilder builder(openpit::SyncPolicy::Full);

  policies::SpotFundsPnlBoundsBarrier global("USD");
  global.lowerBound = openpit::param::Pnl::FromString("-1000");

  policies::SpotFundsPnlBoundsBarrier group("USD");
  group.upperBound = openpit::param::Pnl::FromString("1000");

  policies::SpotFundsPnlBoundsBarrier account("USD");
  account.lowerBound = openpit::param::Pnl::FromString("-250");

  builder.Add(
      policies::SpotFundsPnlBoundsKillSwitchPolicy{}
          .GlobalBarrier(std::move(global))
          .AccountGroupBarrier(policies::SpotFundsPnlBoundsAccountGroupBarrier(
              openpit::param::AccountGroupId::FromUint32(7), std::move(group)))
          .AccountBarrier(policies::SpotFundsPnlBoundsAccountBarrier(
              openpit::param::AccountId::FromUint64(99224416),
              std::move(account), openpit::param::Pnl::FromString("0"))));

  EXPECT_NO_THROW({ openpit::Engine engine = builder.Build(); });
}

TEST(BuiltinPolicy, SpotFundsPnlBoundsPolicyRequiresBarrier) {
  openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
  EXPECT_THROW(
      { builder.Add(policies::SpotFundsPnlBoundsKillSwitchPolicy{}); },
      openpit::Error);
}

TEST(BuiltinPolicy, SpotFundsPnlBoundsConfiguratorUpdatesAxesAndPnl) {
  openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
  policies::SpotFundsPnlBoundsBarrier account("USD");
  account.lowerBound = openpit::param::Pnl::FromString("-10");
  builder.Add(policies::SpotFundsPnlBoundsKillSwitchPolicy{}.AccountBarrier(
      policies::SpotFundsPnlBoundsAccountBarrier(
          openpit::param::AccountId::FromUint64(99224416), std::move(account),
          openpit::param::Pnl::FromString("0"))));
  openpit::Engine engine = builder.Build();

  policies::SpotFundsPnlBoundsBarrier global("USD");
  global.lowerBound = openpit::param::Pnl::FromString("-100");
  policies::SpotFundsPnlBoundsBarrier group("USD");
  group.upperBound = openpit::param::Pnl::FromString("100");
  policies::SpotFundsPnlBoundsBarrier update("USD");
  update.lowerBound = openpit::param::Pnl::FromString("-20");
  update.upperBound = openpit::param::Pnl::FromString("20");

  EXPECT_NO_THROW({
    engine.Configure().SpotFundsPnlBoundsKillSwitch(
        policies::SpotFundsPolicyName,
        std::vector<policies::SpotFundsPnlBoundsBarrier>{std::move(global)},
        std::vector<policies::SpotFundsPnlBoundsAccountGroupBarrier>{
            policies::SpotFundsPnlBoundsAccountGroupBarrier(
                openpit::param::AccountGroupId::FromUint32(7),
                std::move(group))},
        std::vector<policies::SpotFundsPnlBoundsAccountBarrierUpdate>{
            policies::SpotFundsPnlBoundsAccountBarrierUpdate(
                openpit::param::AccountId::FromUint64(99224416),
                std::move(update))});
  });
  EXPECT_NO_THROW({
    engine.Configure().SetSpotFundsAccountPnl(
        policies::SpotFundsPolicyName,
        openpit::param::AccountId::FromUint64(99224416), "USD",
        openpit::param::Pnl::FromString("2.5"));
  });
}

TEST(BuiltinPolicy, SpotFundsPnlBoundsRuntimeAxisReplacementAndClear) {
  const openpit::param::AccountGroupId group =
      openpit::param::AccountGroupId::FromUint32(85);
  const openpit::param::AccountId accountSpecific =
      openpit::param::AccountId::FromUint64(83011);
  const openpit::param::AccountId accountGroup =
      openpit::param::AccountId::FromUint64(83012);
  const openpit::param::AccountId accountGlobal =
      openpit::param::AccountId::FromUint64(83013);
  const openpit::param::AccountId accountAfterClear =
      openpit::param::AccountId::FromUint64(83014);

  openpit::EngineBuilder builder(openpit::SyncPolicy::None);
  builder.Add(policies::SpotFundsPolicy{});
  const openpit::Engine engine = builder.Build();
  for (const openpit::param::AccountId account :
       {accountSpecific, accountGroup, accountGlobal, accountAfterClear}) {
    SeedSpotFundsLifecycleAccount(engine, account);
  }
  ASSERT_FALSE(
      engine.Accounts().RegisterGroup({accountGroup}, group).has_value());

  policies::SpotFundsPnlBoundsBarrier global("USD");
  global.lowerBound = openpit::param::Pnl::FromString("-20");
  policies::SpotFundsPnlBoundsBarrier groupBarrier("USD");
  groupBarrier.lowerBound = openpit::param::Pnl::FromString("-10");
  policies::SpotFundsPnlBoundsBarrier accountBarrier("USD");
  accountBarrier.lowerBound = openpit::param::Pnl::FromString("-10");
  engine.Configure().SpotFundsPnlBoundsKillSwitch(
      policies::SpotFundsPolicyName,
      std::vector<policies::SpotFundsPnlBoundsBarrier>{global},
      std::vector<policies::SpotFundsPnlBoundsAccountGroupBarrier>{
          policies::SpotFundsPnlBoundsAccountGroupBarrier(group, groupBarrier)},
      std::vector<policies::SpotFundsPnlBoundsAccountBarrierUpdate>{
          policies::SpotFundsPnlBoundsAccountBarrierUpdate(accountSpecific,
                                                           accountBarrier)});

  engine.Configure().SetSpotFundsAccountPnl(
      policies::SpotFundsPolicyName, accountSpecific, "USD",
      openpit::param::Pnl::FromString("-15"));
  engine.Configure().SetSpotFundsAccountPnl(
      policies::SpotFundsPolicyName, accountGroup, "USD",
      openpit::param::Pnl::FromString("-15"));
  engine.Configure().SetSpotFundsAccountPnl(
      policies::SpotFundsPolicyName, accountGlobal, "USD",
      openpit::param::Pnl::FromString("-25"));
  engine.Configure().SetSpotFundsAccountPnl(
      policies::SpotFundsPolicyName, accountAfterClear, "USD",
      openpit::param::Pnl::FromString("-25"));

  // An engaged empty account axis clears only per-account barriers. The
  // omitted global and group axes remain in force for their respective keys.
  engine.Configure().SpotFundsPnlBoundsKillSwitch(
      policies::SpotFundsPolicyName, std::nullopt, std::nullopt,
      std::vector<policies::SpotFundsPnlBoundsAccountBarrierUpdate>{});
  EXPECT_TRUE(ApplySpotFundsLifecycleFill(engine, accountSpecific).empty());
  ExpectSpotFundsPnlBlock(ApplySpotFundsLifecycleFill(engine, accountGroup));
  ExpectSpotFundsPnlBlock(ApplySpotFundsLifecycleFill(engine, accountGlobal));

  // Runtime patches may clear every axis, unlike the explicit PnL batch
  // builder that requires at least one barrier at construction time.
  engine.Configure().SpotFundsPnlBoundsKillSwitch(
      policies::SpotFundsPolicyName,
      std::vector<policies::SpotFundsPnlBoundsBarrier>{},
      std::vector<policies::SpotFundsPnlBoundsAccountGroupBarrier>{},
      std::vector<policies::SpotFundsPnlBoundsAccountBarrierUpdate>{});
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

  policies::SpotFundsPnlBoundsBarrier usd("USD");
  usd.lowerBound = openpit::param::Pnl::FromString("-30");
  engine.Configure().SpotFundsPnlBoundsKillSwitch(
      policies::SpotFundsPolicyName,
      std::vector<policies::SpotFundsPnlBoundsBarrier>{usd});
  engine.Configure().SetSpotFundsAccountPnl(
      policies::SpotFundsPolicyName, account, "USD",
      openpit::param::Pnl::FromString("-40"));

  // Replacing the global axis with the existing USD key plus a new EUR key
  // must preserve the USD accumulator instead of reseeding it.
  policies::SpotFundsPnlBoundsBarrier eur("EUR");
  eur.lowerBound = openpit::param::Pnl::FromString("-1");
  engine.Configure().SpotFundsPnlBoundsKillSwitch(
      policies::SpotFundsPolicyName,
      std::vector<policies::SpotFundsPnlBoundsBarrier>{usd, eur});
  ExpectSpotFundsPnlBlock(ApplySpotFundsLifecycleFill(engine, account));
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
