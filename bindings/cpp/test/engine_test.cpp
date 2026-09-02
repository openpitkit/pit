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

#include "openpit/engine.hpp"

#include "openpit/accountadjustment/account_adjustment.hpp"
#include "openpit/model/model.hpp"
#include "openpit/pretrade/decision.hpp"
#include "openpit/pretrade/pretrade.hpp"

#include <gtest/gtest.h>

#include <array>
#include <cstdint>
#include <memory>
#include <optional>
#include <stdexcept>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

namespace {

using openpit::Engine;
using openpit::EngineBuilder;
using openpit::SyncPolicy;
using openpit::param::AccountId;
using openpit::param::Price;
using openpit::param::Quantity;
using openpit::pretrade::Reject;
using openpit::pretrade::RejectCode;

namespace policies = openpit::pretrade::policies;

// Builds the canonical single-leg test order for `accountId`: a buy of one AAPL
// settled in USD at price 100.
[[nodiscard]] openpit::model::Order TestOrder(std::uint64_t accountId) {
  openpit::model::Order order;
  openpit::model::OrderOperation op;
  op.instrument = openpit::model::Instrument(::openpit::param::Asset("AAPL"),
                                             ::openpit::param::Asset("USD"));
  op.accountId = ::openpit::param::AccountId::FromUint64(accountId);
  op.side = openpit::model::Side::Buy;
  op.tradeAmount =
      openpit::model::TradeAmount::OfQuantity(Quantity::FromString("1"));
  op.price = Price::FromString("100");
  order.operation = std::move(op);
  return order;
}

[[nodiscard]] openpit::model::Order SizedOrder(std::string_view quantity,
                                               std::string_view price,
                                               std::uint64_t accountId = 1) {
  openpit::model::Order order = TestOrder(accountId);
  order.operation->tradeAmount =
      openpit::model::TradeAmount::OfQuantity(Quantity::FromString(quantity));
  order.operation->price = Price::FromString(price);
  return order;
}

// A rate-limit engine that admits a single order on the broker axis: the second
// pre-trade for any account is rejected with RateLimitExceeded.
[[nodiscard]] Engine SingleOrderEngine() {
  EngineBuilder builder(SyncPolicy::None);
  policies::RateLimitPolicy config;
  config.BrokerBarrier(policies::RateLimitBrokerBarrier(policies::RateLimit(
      /*maxOrders=*/1, /*windowNanoseconds=*/60'000'000'000)));
  config.AddTo(builder);
  return builder.Build();
}

[[nodiscard]] Engine SingleQuantityEngine() {
  EngineBuilder builder(SyncPolicy::None);
  policies::OrderSizeLimitPolicy config;
  config.BrokerBarrier(policies::OrderSizeBrokerBarrier(
      policies::OrderSizeLimit::Quantity(Quantity::FromString("1"))));
  config.AssetBarrier(policies::OrderSizeAssetBarrier(
      policies::OrderSizeLimit::Quantity(Quantity::FromString("3")),
      ::openpit::param::Asset("AAPL")));
  config.AddTo(builder);
  return builder.Build();
}

[[nodiscard]] Engine BrokerOrderSizeEngine(policies::OrderSizeLimit limit) {
  EngineBuilder builder(SyncPolicy::None);
  policies::OrderSizeLimitPolicy config;
  config.BrokerBarrier(policies::OrderSizeBrokerBarrier(std::move(limit)));
  config.AddTo(builder);
  return builder.Build();
}

[[nodiscard]] Engine SettlementAssetOrderSizeEngine(
    policies::OrderSizeLimit assetLimit,
    policies::OrderSizeLimit accountAssetLimit) {
  EngineBuilder builder(SyncPolicy::None);
  policies::OrderSizeLimitPolicy config;
  config.AssetBarrier(policies::OrderSizeAssetBarrier(
      std::move(assetLimit), ::openpit::param::Asset("USD")));
  config.AccountAssetBarrier(policies::OrderSizeAccountAssetBarrier(
      std::move(accountAssetLimit), AccountId::FromUint64(2),
      ::openpit::param::Asset("USD")));
  config.AddTo(builder);
  return builder.Build();
}

[[nodiscard]] Engine BrokerAndAssetRateLimitEngine() {
  EngineBuilder builder(SyncPolicy::None);
  policies::RateLimitPolicy config;
  config.BrokerBarrier(policies::RateLimitBrokerBarrier(policies::RateLimit(
      /*maxOrders=*/1, /*windowNanoseconds=*/60'000'000'000)));
  config.AssetBarrier(policies::RateLimitAssetBarrier(
      policies::RateLimit(/*maxOrders=*/3,
                          /*windowNanoseconds=*/60'000'000'000),
      ::openpit::param::Asset("USD")));
  config.AddTo(builder);
  return builder.Build();
}

[[nodiscard]] openpit::model::Order TwoQuantityOrder() {
  openpit::model::Order order = TestOrder(1);
  order.operation->tradeAmount =
      openpit::model::TradeAmount::OfQuantity(Quantity::FromString("2"));
  return order;
}

template <typename Handler>
[[nodiscard]] Engine CustomPolicyEngine(Handler handler) {
  EngineBuilder builder(SyncPolicy::Full);
  const std::string name(handler.Name());
  openpit::pretrade::CustomPolicy<Handler> policy(name, std::move(handler));
  builder.Add(policy);
  return builder.Build();
}

class ThrowingStartPolicy {
 public:
  [[nodiscard]] std::string_view Name() const noexcept {
    return "ThrowingStartPolicy";
  }

  [[nodiscard]] std::optional<Reject> CheckPreTradeStart(
      const openpit::Order& /*order*/) const {
    throw std::runtime_error("start callback failed");
  }
};

class ThrowingMainPolicy {
 public:
  [[nodiscard]] std::string_view Name() const noexcept {
    return "ThrowingMainPolicy";
  }

  void PerformPreTradeCheck(
      const openpit::pretrade::Context& /*context*/,
      openpit::pretrade::PolicyDecision& /*decision*/) const {
    throw std::runtime_error("main callback failed");
  }
};

class ThrowingMutationCommitPolicy {
 public:
  explicit ThrowingMutationCommitPolicy(
      std::shared_ptr<std::array<bool, 2>> state)
      : m_state(std::move(state)) {}

  [[nodiscard]] std::string_view Name() const noexcept {
    return "ThrowingMutationCommitPolicy";
  }

  void PerformPreTradeCheck(
      const openpit::pretrade::Context& /*context*/,
      openpit::tx::Mutations& mutations, openpit::pretrade::Result& /*result*/,
      openpit::pretrade::PolicyDecision& /*decision*/) const {
    const auto first = m_state;
    mutations.Push([first] { (*first)[0] = true; },
                   [first] { (*first)[0] = false; });
    const auto second = m_state;
    mutations.Push(
        [second] {
          (*second)[1] = true;
          throw std::runtime_error("mutation commit failed");
        },
        [second] { (*second)[1] = false; });
  }

 private:
  std::shared_ptr<std::array<bool, 2>> m_state;
};

// Throws a non-`std::exception` from a mutation commit callback, so the
// trampoline has no `what()` to copy into the native error string.
class ThrowingIntMutationCommitPolicy {
 public:
  [[nodiscard]] std::string_view Name() const noexcept {
    return "ThrowingIntMutationCommitPolicy";
  }

  void PerformPreTradeCheck(
      const openpit::pretrade::Context& /*context*/,
      openpit::tx::Mutations& mutations, openpit::pretrade::Result& /*result*/,
      openpit::pretrade::PolicyDecision& /*decision*/) const {
    mutations.Push([] { throw 17; }, [] {});
  }
};

// Compensation runs in reverse order: the failing rollback is captured by the
// outer Rollback scope first, and the earlier mutation's rollback then calls a
// different engine, which creates a nested callback scope and must not clear
// the already captured outer exception.
class NestedCallDuringRollbackPolicy {
 public:
  explicit NestedCallDuringRollbackPolicy(const Engine* nested)
      : m_nested(nested) {}

  [[nodiscard]] std::string_view Name() const noexcept {
    return "NestedCallDuringRollbackPolicy";
  }

  void PerformPreTradeCheck(
      const openpit::pretrade::Context& /*context*/,
      openpit::tx::Mutations& mutations, openpit::pretrade::Result& /*result*/,
      openpit::pretrade::PolicyDecision& /*decision*/) const {
    const Engine* nested = m_nested;
    mutations.Push(
        [] {},
        [nested] { static_cast<void>(nested->StartPreTrade(TestOrder(2))); });
    mutations.Push(
        [] {},
        [] { throw std::runtime_error("outer rollback callback failed"); });
  }

 private:
  const Engine* m_nested;
};

class FatalRollbackFailurePolicy {
 public:
  [[nodiscard]] std::string_view Name() const noexcept {
    return "FatalRollbackFailurePolicy";
  }

  void PerformPreTradeCheck(const openpit::pretrade::Context& /*context*/,
                            openpit::tx::Mutations& mutations,
                            openpit::pretrade::Result& /*result*/,
                            openpit::pretrade::PolicyDecision& decision) const {
    mutations.Push(
        [] {}, [] { throw std::runtime_error("drop-copy rollback failed"); });
    decision.Push(Reject(std::string(Name()),
                         openpit::pretrade::RejectScope::Order,
                         RejectCode::MissingRequiredField,
                         "fatal evaluation failure", "forced failure"));
  }
};

class AppliedRollbackFailurePolicy {
 public:
  [[nodiscard]] std::string_view Name() const noexcept {
    return "AppliedRollbackFailurePolicy";
  }

  void PerformPreTradeCheck(
      const openpit::pretrade::Context& /*context*/,
      openpit::tx::Mutations& mutations, openpit::pretrade::Result& /*result*/,
      openpit::pretrade::PolicyDecision& /*decision*/) const {
    mutations.Push(
        [] {},
        [] { throw std::runtime_error("explicit drop-copy rollback failed"); });
  }
};

class ThrowingDryRunPolicy {
 public:
  [[nodiscard]] std::string_view Name() const noexcept {
    return "ThrowingDryRunPolicy";
  }

  [[nodiscard]] std::optional<Reject> CheckPreTradeStartDryRun(
      const openpit::Order& /*order*/) const {
    throw 17;
  }
};

class ThrowingReportPolicy {
 public:
  [[nodiscard]] std::string_view Name() const noexcept {
    return "ThrowingReportPolicy";
  }

  [[nodiscard]] bool ApplyExecutionReport(
      const openpit::ExecutionReport& /*report*/) const {
    throw std::runtime_error("report callback failed");
  }
};

struct DeferredOrder : public openpit::model::Order {
  std::unique_ptr<std::string> strategyTag;
};

struct CopyableDeferredOrder : public openpit::model::Order {
  std::string strategyTag;
};

class DeferredOrderPolicy {
 public:
  explicit DeferredOrderPolicy(std::shared_ptr<std::string> observedTag)
      : m_observedTag(std::move(observedTag)) {}

  void PerformPreTradeCheck(
      const openpit::pretrade::Context& context,
      openpit::pretrade::PolicyDecision& /*decision*/) const {
    const auto* order = dynamic_cast<const DeferredOrder*>(&context.Order());
    if (order == nullptr) {
      throw std::runtime_error("deferred order type was not preserved");
    }
    if (!order->strategyTag) {
      throw std::runtime_error("deferred order tag was moved unexpectedly");
    }
    *m_observedTag = *order->strategyTag;
  }

 private:
  std::shared_ptr<std::string> m_observedTag;
};

class CopyableDeferredOrderPolicy {
 public:
  explicit CopyableDeferredOrderPolicy(std::shared_ptr<std::string> observedTag)
      : m_observedTag(std::move(observedTag)) {}

  void PerformPreTradeCheck(
      const openpit::pretrade::Context& context,
      openpit::pretrade::PolicyDecision& /*decision*/) const {
    const auto* order =
        dynamic_cast<const CopyableDeferredOrder*>(&context.Order());
    if (order == nullptr) {
      throw std::runtime_error("deferred order type was not preserved");
    }
    *m_observedTag = order->strategyTag;
  }

 private:
  std::shared_ptr<std::string> m_observedTag;
};

//------------------------------------------------------------------------------
// Builder -> build with policies.

TEST(EngineBuilder, BuildsWithBuiltinPolicy) {
  EngineBuilder builder(SyncPolicy::Full);
  builder.Add(policies::OrderValidationPolicy{});
  Engine engine = builder.Build();
  EXPECT_TRUE(static_cast<bool>(engine));
}

TEST(EngineBuilder, BuildWithoutPoliciesThrows) {
  EngineBuilder builder(SyncPolicy::Full);
  EXPECT_THROW({ Engine engine = builder.Build(); }, openpit::Error);
}

TEST(EngineBuilder, DuplicatePolicyGroupIdThrowsBuildError) {
  EngineBuilder builder(SyncPolicy::Full);
  // Two builtin policies forced onto the same non-default group id collide.
  builder.Add(policies::OrderValidationPolicy{}.PolicyGroupId(7));
  builder.Add(policies::RateLimitPolicy{}.PolicyGroupId(7).BrokerBarrier(
      policies::RateLimitBrokerBarrier(
          policies::RateLimit(1, 60'000'000'000))));
  EXPECT_THROW({ Engine engine = builder.Build(); }, openpit::EngineBuildError);
}

//------------------------------------------------------------------------------
// StartPreTrade.

TEST(EngineStartPreTrade, HappyPathReturnsRequest) {
  Engine engine = SingleOrderEngine();

  openpit::pretrade::StartResult result = engine.StartPreTrade(TestOrder(1));
  EXPECT_TRUE(result.Passed());
  EXPECT_TRUE(result.rejects.empty());
  ASSERT_TRUE(result.request.has_value());
  EXPECT_TRUE(static_cast<bool>(*result.request));
}

TEST(EngineStartPreTrade, RejectPathReturnsRejectsNotRequest) {
  Engine engine = SingleOrderEngine();

  // First order consumes the single-order budget.
  openpit::pretrade::StartResult first = engine.StartPreTrade(TestOrder(1));
  ASSERT_TRUE(first.Passed());

  // Second order is rejected by the rate limiter.
  openpit::pretrade::StartResult second = engine.StartPreTrade(TestOrder(1));
  EXPECT_FALSE(second.Passed());
  EXPECT_FALSE(second.request.has_value());
  ASSERT_EQ(second.rejects.size(), 1u);
  EXPECT_EQ(second.rejects.front().code, RejectCode::RateLimitExceeded);
}

TEST(EngineStartPreTrade, AbiFailureThrows) {
  // A default-constructed (null-handle) engine is an invalid pointer to the C
  // ABI: the boundary failure must surface as a thrown openpit::Error, never a
  // reject value.
  const Engine engine;
  EXPECT_THROW(
      { auto result = engine.StartPreTrade(TestOrder(1)); }, openpit::Error);
}

TEST(EngineStartPreTrade, CallbackExceptionRethrowsOriginalType) {
  Engine engine = CustomPolicyEngine(ThrowingStartPolicy{});

  EXPECT_THROW(
      { static_cast<void>(engine.StartPreTrade(TestOrder(1))); },
      std::runtime_error);
}

//------------------------------------------------------------------------------
// Non-mutating dry-run.

TEST(EngineDryRun, StartDryRunDoesNotConsumeRateLimitBudget) {
  Engine engine = SingleOrderEngine();

  const openpit::pretrade::DryRunReport probe =
      engine.StartPreTradeDryRun(TestOrder(1));
  EXPECT_TRUE(probe.Passed());
  EXPECT_TRUE(probe.Rejects().empty());
  EXPECT_TRUE(probe.Lock().IsEmpty());
  EXPECT_TRUE(probe.AccountAdjustments().empty());
  EXPECT_FALSE(probe.AccountBlock().has_value());

  EXPECT_TRUE(engine.StartPreTrade(TestOrder(1)).Passed());
  const openpit::pretrade::StartResult second =
      engine.StartPreTrade(TestOrder(1));
  EXPECT_FALSE(second.Passed());
  ASSERT_EQ(second.rejects.size(), 1u);
  EXPECT_EQ(second.rejects.front().code, RejectCode::RateLimitExceeded);
}

TEST(EngineDryRun, ExecuteDryRunDoesNotConsumeRateLimitBudget) {
  Engine engine = SingleOrderEngine();

  const openpit::pretrade::DryRunReport probe =
      engine.ExecutePreTradeDryRun(TestOrder(1));
  EXPECT_TRUE(probe.Passed());
  EXPECT_TRUE(probe.Rejects().empty());

  const openpit::pretrade::ExecuteResult first =
      engine.ExecutePreTrade(TestOrder(1));
  EXPECT_TRUE(first.Passed());

  const openpit::pretrade::ExecuteResult second =
      engine.ExecutePreTrade(TestOrder(1));
  EXPECT_FALSE(second.Passed());
  ASSERT_EQ(second.rejects.size(), 1u);
  EXPECT_EQ(second.rejects.front().code, RejectCode::RateLimitExceeded);
}

TEST(EngineDryRun, NonStandardCallbackExceptionIsRethrown) {
  Engine engine = CustomPolicyEngine(ThrowingDryRunPolicy{});

  try {
    static_cast<void>(engine.StartPreTradeDryRun(TestOrder(1)));
    FAIL() << "expected the callback's integer exception";
  } catch (int value) {
    EXPECT_EQ(value, 17);
  } catch (...) {
    FAIL() << "callback exception type was not preserved";
  }
}

//------------------------------------------------------------------------------
// ExecutePreTrade + reservation resolution.

TEST(EngineExecutePreTrade, HappyPathReturnsReservation) {
  Engine engine = SingleOrderEngine();

  openpit::pretrade::ExecuteResult result =
      engine.ExecutePreTrade(TestOrder(1));
  EXPECT_TRUE(result.Passed());
  EXPECT_TRUE(result.rejects.empty());
  ASSERT_TRUE(result.reservation.has_value());
  // Resolving the reservation must not throw.
  EXPECT_NO_THROW(result.reservation->Commit());
}

TEST(EngineExecutePreTrade, ReservationResolutionIsIdempotent) {
  Engine engine = SingleOrderEngine();
  openpit::pretrade::ExecuteResult result =
      engine.ExecutePreTrade(TestOrder(1));
  ASSERT_TRUE(result.reservation.has_value());

  EXPECT_NO_THROW(result.reservation->Commit());
  EXPECT_NO_THROW(result.reservation->Commit());
  EXPECT_NO_THROW(result.reservation->Rollback());
}

// ReservationResolutionIsIdempotent only reaches Rollback() after two Commit()
// calls, so it cannot catch a regression confined to the rollback-after-
// rollback path (the finalization flag guarding against a double finalization
// reaching the core, which panics across the C boundary). This test drives
// Rollback() twice in a row with no preceding Commit() to close that gap.
TEST(EngineExecutePreTrade, ReservationRepeatedRollbackIsIdempotent) {
  Engine engine = SingleOrderEngine();
  openpit::pretrade::ExecuteResult result =
      engine.ExecutePreTrade(TestOrder(1));
  ASSERT_TRUE(result.reservation.has_value());

  EXPECT_NO_THROW(result.reservation->Rollback());
  EXPECT_NO_THROW(result.reservation->Rollback());
}

// A spot-funds insufficient-funds reject surfaced through the C++ binding must
// not carry the account id in its reason or details. 424242 is the sentinel
// account id; the order operands never contain it.
TEST(EngineExecutePreTrade,
     SpotFundsInsufficientFundsRejectDoesNotLeakAccountId) {
  EngineBuilder builder(SyncPolicy::None);
  policies::SpotFundsPolicy{}.AddTo(builder);
  Engine engine = builder.Build();

  // Buy 1 AAPL @ 100 = 100 notional against an unfunded sentinel account.
  const openpit::pretrade::ExecuteResult result =
      engine.ExecutePreTrade(TestOrder(424242));

  EXPECT_FALSE(result.Passed());
  ASSERT_EQ(result.rejects.size(), 1u);
  EXPECT_EQ(result.rejects.front().code, RejectCode::InsufficientFunds);
  EXPECT_EQ(result.rejects.front().reason.find("424242"), std::string::npos);
  EXPECT_EQ(result.rejects.front().details.find("424242"), std::string::npos);
}

TEST(EngineExecutePreTrade, RollbackDoesNotReleaseRateLimitBudget) {
  Engine engine = SingleOrderEngine();

  // The rate-limit budget is consumed by the start stage, not by the main-stage
  // reservation: every start attempt (even rejected ones) permanently counts in
  // the window. Rolling back the reservation only unwinds main-stage mutations,
  // so it cannot return the consumed rate-limit slot. With a single-order
  // limit, the first execute consumes the slot and a subsequent execute is
  // rejected regardless of the rollback.
  {
    openpit::pretrade::ExecuteResult first =
        engine.ExecutePreTrade(TestOrder(1));
    ASSERT_TRUE(first.reservation.has_value());
    first.reservation->Rollback();
  }
  openpit::pretrade::ExecuteResult second =
      engine.ExecutePreTrade(TestOrder(1));
  EXPECT_FALSE(second.Passed());
  ASSERT_EQ(second.rejects.size(), 1u);
  EXPECT_EQ(second.rejects.front().code, RejectCode::RateLimitExceeded);
}

TEST(EngineExecutePreTrade, CommitConsumesReservedBudget) {
  Engine engine = SingleOrderEngine();

  {
    openpit::pretrade::ExecuteResult first =
        engine.ExecutePreTrade(TestOrder(1));
    ASSERT_TRUE(first.reservation.has_value());
    first.reservation->Commit();
  }
  // The committed first order used the single-order budget; the second rejects.
  openpit::pretrade::ExecuteResult second =
      engine.ExecutePreTrade(TestOrder(1));
  EXPECT_FALSE(second.Passed());
  ASSERT_EQ(second.rejects.size(), 1u);
  EXPECT_EQ(second.rejects.front().code, RejectCode::RateLimitExceeded);
}

TEST(EngineExecutePreTrade, AbiFailureThrows) {
  const Engine engine;
  EXPECT_THROW(
      { auto result = engine.ExecutePreTrade(TestOrder(1)); }, openpit::Error);
}

TEST(EngineExecutePreTrade, CallbackExceptionRethrowsOriginalType) {
  Engine engine = CustomPolicyEngine(ThrowingMainPolicy{});

  EXPECT_THROW(
      { static_cast<void>(engine.ExecutePreTrade(TestOrder(1))); },
      std::runtime_error);
}

//------------------------------------------------------------------------------
// ApplyDropCopy + operation resolution.

TEST(EngineDropCopy, HappyPathReturnsOperation) {
  Engine engine = SingleOrderEngine();

  openpit::pretrade::DropCopyResult result = engine.ApplyDropCopy(TestOrder(1));
  EXPECT_TRUE(result.Passed());
  EXPECT_TRUE(result.rejects.empty());
  ASSERT_TRUE(result.operation.has_value());
  // Resolving the operation must not throw.
  EXPECT_NO_THROW(result.operation->Commit());
}

TEST(EngineDropCopy, OperationResolutionIsIdempotent) {
  Engine engine = SingleOrderEngine();
  openpit::pretrade::DropCopyResult result = engine.ApplyDropCopy(TestOrder(1));
  ASSERT_TRUE(result.operation.has_value());

  EXPECT_NO_THROW(result.operation->Commit());
  EXPECT_NO_THROW(result.operation->Commit());
  EXPECT_NO_THROW(result.operation->Rollback());
}

// Mirrors ReservationRepeatedRollbackIsIdempotent: drive Rollback() twice with
// no preceding Commit() so a regression confined to the rollback-after-rollback
// path cannot hide behind the commit-first ordering.
TEST(EngineDropCopy, OperationRepeatedRollbackIsIdempotent) {
  Engine engine = SingleOrderEngine();
  openpit::pretrade::DropCopyResult result = engine.ApplyDropCopy(TestOrder(1));
  ASSERT_TRUE(result.operation.has_value());

  EXPECT_NO_THROW(result.operation->Rollback());
  EXPECT_NO_THROW(result.operation->Rollback());
}

// The rate-limit attempt is consumed by the drop-copy call itself and stays
// outside the finalization boundary, so abandoning the operation at scope exit
// cannot return the consumed slot.
TEST(EngineDropCopy, AbandonedOperationKeepsRateLimitBudgetConsumed) {
  Engine engine = SingleOrderEngine();

  {
    openpit::pretrade::DropCopyResult applied =
        engine.ApplyDropCopy(TestOrder(1));
    ASSERT_TRUE(applied.Passed());
  }
  const openpit::pretrade::StartResult blocked =
      engine.StartPreTrade(TestOrder(1));
  EXPECT_FALSE(blocked.Passed());
  ASSERT_EQ(blocked.rejects.size(), 1u);
  EXPECT_EQ(blocked.rejects.front().code, RejectCode::RateLimitExceeded);
}

TEST(EngineDropCopy, AbiFailureThrows) {
  const Engine engine;
  EXPECT_THROW(
      { auto result = engine.ApplyDropCopy(TestOrder(1)); }, openpit::Error);
}

TEST(EngineDropCopy, CallbackExceptionRethrowsOriginalType) {
  Engine engine = CustomPolicyEngine(ThrowingMainPolicy{});

  EXPECT_THROW(
      { static_cast<void>(engine.ApplyDropCopy(TestOrder(1))); },
      std::runtime_error);
}

// Prepared mutations are committed by the operation, not by the drop-copy call,
// so a failing commit callback surfaces from Commit() with its original type.
TEST(EngineDropCopy, MutationCommitExceptionRethrowsOriginalType) {
  const auto state = std::make_shared<std::array<bool, 2>>();
  Engine engine = CustomPolicyEngine(ThrowingMutationCommitPolicy{state});

  openpit::pretrade::DropCopyResult result = engine.ApplyDropCopy(TestOrder(1));
  ASSERT_TRUE(result.Passed());
  EXPECT_EQ(*state, (std::array<bool, 2>{false, false}));

  try {
    result.operation->Commit();
    FAIL() << "expected the mutation commit exception";
  } catch (const std::runtime_error& cause) {
    EXPECT_STREQ(cause.what(), "mutation commit failed");
  }
}

// A non-standard exception has no `what()` to copy, so the trampoline falls
// back to its own text. The failure must still reach the caller rather than
// being swallowed by a silent commit.
TEST(EngineDropCopy, NonStandardMutationCommitExceptionStillFails) {
  Engine engine = CustomPolicyEngine(ThrowingIntMutationCommitPolicy{});

  openpit::pretrade::DropCopyResult result = engine.ApplyDropCopy(TestOrder(1));
  ASSERT_TRUE(result.Passed());

  try {
    result.operation->Commit();
    FAIL() << "expected the mutation commit exception";
  } catch (int value) {
    EXPECT_EQ(value, 17);
  } catch (...) {
    FAIL() << "callback exception type was not preserved";
  }
}

TEST(EngineDropCopy, NestedOperationPreservesOuterCallbackException) {
  Engine nested = SingleOrderEngine();
  Engine engine = CustomPolicyEngine(NestedCallDuringRollbackPolicy{&nested});

  openpit::pretrade::DropCopyResult result = engine.ApplyDropCopy(TestOrder(1));
  ASSERT_TRUE(result.Passed());

  try {
    result.operation->Rollback();
    FAIL() << "expected the outer rollback exception";
  } catch (const std::runtime_error& cause) {
    EXPECT_STREQ(cause.what(), "outer rollback callback failed");
  }
}

// A fatal evaluation reject aborts the operation and compensates the prepared
// mutations before ApplyDropCopy returns. A failing compensation callback is
// reported to the caller and safety-blocks the account.
TEST(EngineDropCopy, FatalRejectRollbackFailureSafetyBlocksAccount) {
  Engine engine = CustomPolicyEngine(FatalRollbackFailurePolicy{});

  try {
    static_cast<void>(engine.ApplyDropCopy(TestOrder(1)));
    FAIL() << "expected the rollback callback exception";
  } catch (const std::runtime_error& cause) {
    EXPECT_STREQ(cause.what(), "drop-copy rollback failed");
  }

  const openpit::pretrade::StartResult blocked =
      engine.StartPreTrade(TestOrder(1));
  ASSERT_FALSE(blocked.Passed());
  ASSERT_EQ(blocked.rejects.size(), 1u);
  EXPECT_EQ(blocked.rejects.front().code, RejectCode::SystemUnavailable);
}

TEST(EngineDropCopy, ExplicitRollbackFailureRethrowsOriginalType) {
  Engine engine = CustomPolicyEngine(AppliedRollbackFailurePolicy{});
  openpit::pretrade::DropCopyResult result = engine.ApplyDropCopy(TestOrder(1));
  ASSERT_TRUE(result.Passed());

  try {
    result.operation->Rollback();
    FAIL() << "expected the explicit rollback callback exception";
  } catch (const std::runtime_error& cause) {
    EXPECT_STREQ(cause.what(), "explicit drop-copy rollback failed");
  }

  EXPECT_NO_THROW(result.operation->Rollback());
}

TEST(EngineDropCopy, DestructorSuppressesRollbackFailure) {
  Engine engine = CustomPolicyEngine(AppliedRollbackFailurePolicy{});

  EXPECT_NO_THROW({
    const openpit::pretrade::DropCopyResult result =
        engine.ApplyDropCopy(TestOrder(1));
    EXPECT_TRUE(result.Passed());
  });
}

//------------------------------------------------------------------------------
// Request::Execute (deferred two-stage flow).

TEST(EngineRequest, ExecutePassesThenCommit) {
  Engine engine = SingleOrderEngine();
  const openpit::model::Order order = TestOrder(1);

  openpit::pretrade::StartResult start = engine.StartPreTrade(order);
  ASSERT_TRUE(start.request.has_value());

  openpit::pretrade::ExecuteResult executed = start.request->Execute();
  EXPECT_TRUE(executed.Passed());
  ASSERT_TRUE(executed.reservation.has_value());
  EXPECT_NO_THROW(executed.reservation->Commit());
}

TEST(EngineRequest, SecondDeferredFlowIsRejectedAtStart) {
  Engine engine = SingleOrderEngine();

  // Drive the first order all the way to commit.
  const openpit::model::Order firstOrder = TestOrder(1);
  openpit::pretrade::StartResult first = engine.StartPreTrade(firstOrder);
  ASSERT_TRUE(first.request.has_value());
  openpit::pretrade::ExecuteResult firstExec = first.request->Execute();
  ASSERT_TRUE(firstExec.reservation.has_value());
  firstExec.reservation->Commit();

  // The rate limiter runs in the start stage, so the second order is rejected
  // by StartPreTrade itself: no deferred request is produced and the reject is
  // surfaced immediately rather than at the main stage.
  openpit::pretrade::StartResult second = engine.StartPreTrade(TestOrder(1));
  EXPECT_FALSE(second.Passed());
  EXPECT_FALSE(second.request.has_value());
  ASSERT_EQ(second.rejects.size(), 1u);
  EXPECT_EQ(second.rejects.front().code, RejectCode::RateLimitExceeded);
}

TEST(EngineRequest, CallbackExceptionRethrowsOriginalType) {
  Engine engine = CustomPolicyEngine(ThrowingMainPolicy{});
  const openpit::model::Order order = TestOrder(1);

  openpit::pretrade::StartResult start = engine.StartPreTrade(order);
  ASSERT_TRUE(start.request.has_value());

  EXPECT_THROW(
      { static_cast<void>(start.request->Execute()); }, std::runtime_error);
}

TEST(EngineRequest, OwnsRvalueConcreteOrderUntilDeferredExecution) {
  const auto observedTag = std::make_shared<std::string>();
  EngineBuilder builder(SyncPolicy::Full);
  openpit::pretrade::CustomPolicy<DeferredOrderPolicy> policy(
      "DeferredOrderPolicy", DeferredOrderPolicy(observedTag));
  builder.Add(policy);
  Engine engine = builder.Build();

  std::optional<openpit::pretrade::StartResult> start;
  {
    DeferredOrder order;
    order.strategyTag = std::make_unique<std::string>("request-owned");
    start.emplace(engine.StartPreTrade(std::move(order)));
    EXPECT_EQ(order.strategyTag, nullptr);
  }
  ASSERT_TRUE(start->request.has_value());

  openpit::pretrade::ExecuteResult executed = start->request->Execute();
  ASSERT_TRUE(executed.Passed());
  EXPECT_EQ(*observedTag, "request-owned");
}

TEST(EngineRequest, CopiesLvalueConcreteOrderUntilDeferredExecution) {
  const auto observedTag = std::make_shared<std::string>();
  EngineBuilder builder(SyncPolicy::Full);
  openpit::pretrade::CustomPolicy<CopyableDeferredOrderPolicy> policy(
      "CopyableDeferredOrderPolicy", CopyableDeferredOrderPolicy(observedTag));
  builder.Add(policy);
  Engine engine = builder.Build();

  CopyableDeferredOrder order;
  order.strategyTag = "caller-owned";
  openpit::pretrade::StartResult start = engine.StartPreTrade(order);
  ASSERT_TRUE(start.request.has_value());
  EXPECT_EQ(order.strategyTag, "caller-owned");

  order.strategyTag = "caller-mutated";
  const openpit::pretrade::ExecuteResult executed = start.request->Execute();
  ASSERT_TRUE(executed.Passed());
  EXPECT_EQ(*observedTag, "caller-owned");
}

TEST(EngineRequest, BaseTypedReferenceToDerivedThrowsInsteadOfSlicing) {
  Engine engine = SingleOrderEngine();
  DeferredOrder derived;
  const openpit::model::Order& order = derived;

  EXPECT_THROW(
      { static_cast<void>(engine.StartPreTrade(order)); }, openpit::Error);
}

TEST(EngineRequest, TypeErasedUniquePtrPreservesDynamicType) {
  const auto observedTag = std::make_shared<std::string>();
  EngineBuilder builder(SyncPolicy::Full);
  openpit::pretrade::CustomPolicy<DeferredOrderPolicy> policy(
      "DeferredOrderPolicy", DeferredOrderPolicy(observedTag));
  builder.Add(policy);
  Engine engine = builder.Build();

  auto concrete = std::make_unique<DeferredOrder>();
  concrete->strategyTag = std::make_unique<std::string>("type-erased");
  std::unique_ptr<const openpit::Order> order = std::move(concrete);
  openpit::pretrade::StartResult start = engine.StartPreTrade(std::move(order));
  ASSERT_TRUE(start.request.has_value());

  openpit::pretrade::ExecuteResult executed = start.request->Execute();
  ASSERT_TRUE(executed.Passed());
  EXPECT_EQ(*observedTag, "type-erased");
}

TEST(EngineRequest, NullUniquePtrThrows) {
  Engine engine = SingleOrderEngine();
  std::unique_ptr<const openpit::Order> order;

  EXPECT_THROW(
      { static_cast<void>(engine.StartPreTrade(std::move(order))); },
      openpit::Error);
}

//------------------------------------------------------------------------------
// ApplyExecutionReport.

// Builds an execution report carrying the operation identity for `accountId`.
[[nodiscard]] openpit::model::ExecutionReport TestReport(
    std::uint64_t accountId) {
  openpit::model::ExecutionReport report;
  openpit::model::ExecutionReportOperation op;
  op.instrument = openpit::model::Instrument(::openpit::param::Asset("AAPL"),
                                             ::openpit::param::Asset("USD"));
  op.accountId = ::openpit::param::AccountId::FromUint64(accountId);
  op.side = openpit::model::Side::Buy;
  report.operation = std::move(op);
  return report;
}

TEST(EngineApplyExecutionReport, OrderValidationProducesNoBlocks) {
  EngineBuilder builder(SyncPolicy::Full);
  builder.Add(policies::OrderValidationPolicy{});
  Engine engine = builder.Build();

  // OrderValidation neither blocks nor produces adjustment outcomes; the report
  // applies cleanly with empty result channels.
  const openpit::PostTradeResult result =
      engine.ApplyExecutionReport(TestReport(1));
  EXPECT_TRUE(result.accountBlocks.empty());
  EXPECT_TRUE(result.accountPnls.empty());
  EXPECT_TRUE(result.accountAdjustments.empty());
}

TEST(EngineApplyExecutionReport, AbiFailureThrows) {
  const Engine engine;
  EXPECT_THROW(
      { auto result = engine.ApplyExecutionReport(TestReport(1)); },
      openpit::Error);
}

TEST(EngineApplyExecutionReport, CallbackExceptionRethrowsOriginalType) {
  Engine engine = CustomPolicyEngine(ThrowingReportPolicy{});

  EXPECT_THROW(
      { static_cast<void>(engine.ApplyExecutionReport(TestReport(1))); },
      std::runtime_error);
}

//------------------------------------------------------------------------------
// ApplyAccountAdjustment.
//
// The adjustment value type is authored in
// `openpit/accountadjustment/account_adjustment.hpp`. The empty-batch path
// exercises the engine method end-to-end without depending on that type: a
// zero-length batch is accepted and applies cleanly. A minimal local stub opts
// into the private native bridge; it is never invoked for an empty batch.

struct StubAdjustment {
 private:
  friend class openpit::detail::NativeAccess;

  [[nodiscard]] openpit::accountadjustment::detail::RawAccountAdjustment
  Native() const noexcept {
    return {};
  }
};

TEST(EngineApplyAccountAdjustment, EmptyBatchApplies) {
  EngineBuilder builder(SyncPolicy::Full);
  builder.Add(policies::OrderValidationPolicy{});
  Engine engine = builder.Build();

  const openpit::AdjustmentResult result =
      engine.ApplyAccountAdjustment<StubAdjustment>(AccountId::FromUint64(1),
                                                    /*adjustments=*/{});
  EXPECT_TRUE(result.Passed());
  EXPECT_TRUE(result.accountAdjustmentOutcomes.empty());
}

TEST(EngineApplyAccountAdjustment, AbiFailureThrows) {
  const Engine engine;
  EXPECT_THROW(
      {
        auto result = engine.ApplyAccountAdjustment<StubAdjustment>(
            AccountId::FromUint64(1), /*adjustments=*/{});
        static_cast<void>(result);
      },
      openpit::Error);
}

//------------------------------------------------------------------------------
// Account currency and runtime configuration.

TEST(EngineAccountCurrency, SetAndClearAccountAndGroupCurrency) {
  EngineBuilder builder(SyncPolicy::Full);
  builder.Add(policies::OrderValidationPolicy{});
  Engine engine = builder.Build();

  const AccountId account = AccountId::FromUint64(42);
  const openpit::param::AccountGroupId group =
      openpit::param::AccountGroupId::FromUint32(7);

  EXPECT_NO_THROW(
      engine.Accounts().SetCurrency(account, openpit::param::Asset("USD")));
  EXPECT_NO_THROW(engine.Accounts().ClearCurrency(account));

  ASSERT_FALSE(engine.Accounts().RegisterGroup({account}, group).has_value());
  EXPECT_NO_THROW(
      engine.Accounts().SetGroupCurrency(group, openpit::param::Asset("USD")));
  EXPECT_NO_THROW(engine.Accounts().ClearGroupCurrency(group));
}

TEST(EngineConfigure, RateLimitUpdateChangesRuntimeBudget) {
  Engine engine = SingleOrderEngine();

  engine.Configure().RateLimit(
      policies::RateLimitPolicyName,
      policies::RateLimitBrokerBarrierUpdate::Set(
          policies::RateLimitBrokerBarrier(policies::RateLimit(
              /*maxOrders=*/2,
              /*windowNanoseconds=*/60'000'000'000))));

  EXPECT_TRUE(engine.StartPreTrade(TestOrder(1)).Passed());
  EXPECT_TRUE(engine.StartPreTrade(TestOrder(1)).Passed());

  const openpit::pretrade::StartResult third =
      engine.StartPreTrade(TestOrder(1));
  EXPECT_FALSE(third.Passed());
  ASSERT_EQ(third.rejects.size(), 1u);
  EXPECT_EQ(third.rejects.front().code, RejectCode::RateLimitExceeded);
}

TEST(EngineConfigure, UnknownPolicyThrowsStructuredConfigureError) {
  Engine engine = SingleOrderEngine();

  try {
    engine.Configure().RateLimit(
        "MissingPolicy",
        policies::RateLimitBrokerBarrierUpdate::Set(
            policies::RateLimitBrokerBarrier(policies::RateLimit(
                /*maxOrders=*/2,
                /*windowNanoseconds=*/60'000'000'000))));
    FAIL() << "Configure().RateLimit should have thrown";
  } catch (const openpit::ConfigureError& error) {
    EXPECT_EQ(error.Kind(), openpit::ConfigureErrorKind::Unknown);
  }
}

TEST(EngineConfigure, RateLimitBrokerUpdateCanClearOrRemainUnchanged) {
  Engine unchanged = BrokerAndAssetRateLimitEngine();
  EXPECT_TRUE(unchanged.StartPreTrade(TestOrder(1)).Passed());
  unchanged.Configure().RateLimit(policies::RateLimitPolicyName);
  EXPECT_FALSE(unchanged.StartPreTrade(TestOrder(1)).Passed());

  Engine cleared = BrokerAndAssetRateLimitEngine();
  cleared.Configure().RateLimit(
      policies::RateLimitPolicyName,
      policies::RateLimitBrokerBarrierUpdate::Clear());
  EXPECT_TRUE(cleared.StartPreTrade(TestOrder(1)).Passed());
  EXPECT_TRUE(cleared.StartPreTrade(TestOrder(1)).Passed());
}

TEST(EngineConfigure, OrderSizeBrokerUpdateCanClearOrRemainUnchanged) {
  Engine unchanged = SingleQuantityEngine();
  unchanged.Configure().OrderSizeLimit(policies::OrderSizeLimitPolicyName);
  EXPECT_FALSE(unchanged.StartPreTrade(TwoQuantityOrder()).Passed());

  Engine cleared = SingleQuantityEngine();
  cleared.Configure().OrderSizeLimit(
      policies::OrderSizeLimitPolicyName,
      policies::OrderSizeBrokerBarrierUpdate::Clear());
  EXPECT_TRUE(cleared.StartPreTrade(TwoQuantityOrder()).Passed());
}

TEST(EngineConfigure, OrderSizeBrokerUpdatePreservesAbsentAndZeroCaps) {
  Engine engine = SingleQuantityEngine();
  engine.Configure().OrderSizeLimit(
      policies::OrderSizeLimitPolicyName,
      policies::OrderSizeBrokerBarrierUpdate::Set(
          policies::OrderSizeBrokerBarrier(policies::OrderSizeLimit::Notional(
              ::openpit::param::Volume::FromString("1000")))));

  EXPECT_TRUE(engine.StartPreTrade(SizedOrder("1", "1000")).Passed());
  const openpit::pretrade::StartResult aboveNotional =
      engine.StartPreTrade(SizedOrder("1", "1001"));
  EXPECT_FALSE(aboveNotional.Passed());
  ASSERT_EQ(aboveNotional.rejects.size(), 1u);
  EXPECT_EQ(aboveNotional.rejects.front().code,
            RejectCode::OrderNotionalExceedsLimit);

  engine.Configure().OrderSizeLimit(
      policies::OrderSizeLimitPolicyName,
      policies::OrderSizeBrokerBarrierUpdate::Set(
          policies::OrderSizeBrokerBarrier(
              policies::OrderSizeLimit::Quantity(Quantity::FromString("0")))));
  const openpit::pretrade::StartResult zeroQuantity =
      engine.StartPreTrade(SizedOrder("1", "1"));
  EXPECT_FALSE(zeroQuantity.Passed());
  ASSERT_EQ(zeroQuantity.rejects.size(), 1u);
  EXPECT_EQ(zeroQuantity.rejects.front().code,
            RejectCode::OrderQtyExceedsLimit);
}

TEST(EngineConfigure,
     OrderSizeSettlementAssetUpdatesPreserveAbsentAndZeroCaps) {
  Engine engine = SettlementAssetOrderSizeEngine(
      policies::OrderSizeLimit::Notional(
          ::openpit::param::Volume::FromString("5000")),
      policies::OrderSizeLimit::Notional(
          ::openpit::param::Volume::FromString("6000")));
  engine.Configure().OrderSizeLimit(
      policies::OrderSizeLimitPolicyName,
      policies::OrderSizeBrokerBarrierUpdate::Unchanged(),
      std::vector<policies::OrderSizeAssetBarrier>{
          policies::OrderSizeAssetBarrier(
              policies::OrderSizeLimit::Notional(
                  ::openpit::param::Volume::FromString("1000")),
              ::openpit::param::Asset("USD"))},
      std::vector<policies::OrderSizeAccountAssetBarrier>{
          policies::OrderSizeAccountAssetBarrier(
              policies::OrderSizeLimit::Notional(
                  ::openpit::param::Volume::FromString("2000")),
              AccountId::FromUint64(2), ::openpit::param::Asset("USD"))});

  EXPECT_TRUE(engine.StartPreTrade(SizedOrder("1000", "1")).Passed());
  const openpit::pretrade::StartResult aboveAsset =
      engine.StartPreTrade(SizedOrder("1001", "1"));
  EXPECT_FALSE(aboveAsset.Passed());
  ASSERT_EQ(aboveAsset.rejects.size(), 1u);
  EXPECT_EQ(aboveAsset.rejects.front().code,
            RejectCode::OrderNotionalExceedsLimit);

  EXPECT_TRUE(engine.StartPreTrade(SizedOrder("2000", "1", 2)).Passed());
  const openpit::pretrade::StartResult aboveAccountAsset =
      engine.StartPreTrade(SizedOrder("2001", "1", 2));
  EXPECT_FALSE(aboveAccountAsset.Passed());
  ASSERT_EQ(aboveAccountAsset.rejects.size(), 1u);
  EXPECT_EQ(aboveAccountAsset.rejects.front().code,
            RejectCode::OrderNotionalExceedsLimit);
  EXPECT_TRUE(engine.StartPreTrade(SizedOrder("2000", "1", 2)).Passed());

  engine.Configure().OrderSizeLimit(
      policies::OrderSizeLimitPolicyName,
      policies::OrderSizeBrokerBarrierUpdate::Unchanged(),
      std::vector<policies::OrderSizeAssetBarrier>{
          policies::OrderSizeAssetBarrier(
              policies::OrderSizeLimit::Notional(
                  ::openpit::param::Volume::FromString("0")),
              ::openpit::param::Asset("USD"))},
      std::vector<policies::OrderSizeAccountAssetBarrier>{
          policies::OrderSizeAccountAssetBarrier(
              policies::OrderSizeLimit::Notional(
                  ::openpit::param::Volume::FromString("0")),
              AccountId::FromUint64(2), ::openpit::param::Asset("USD"))});
  const openpit::pretrade::StartResult zeroAsset =
      engine.StartPreTrade(SizedOrder("1", "1"));
  EXPECT_FALSE(zeroAsset.Passed());
  ASSERT_EQ(zeroAsset.rejects.size(), 1u);
  EXPECT_EQ(zeroAsset.rejects.front().code,
            RejectCode::OrderNotionalExceedsLimit);
  const openpit::pretrade::StartResult zeroAccountAsset =
      engine.StartPreTrade(SizedOrder("1", "1", 2));
  EXPECT_FALSE(zeroAccountAsset.Passed());
  ASSERT_EQ(zeroAccountAsset.rejects.size(), 1u);
  EXPECT_EQ(zeroAccountAsset.rejects.front().code,
            RejectCode::OrderNotionalExceedsLimit);
}

// The next four probes look like tautologies - every cap is above the order -
// and they are not. Quantity resolves by the instrument's underlying asset
// (AAPL) and notional by its settlement asset (USD), so a quantity-only USD
// barrier and a notional-only AAPL barrier each leave absent exactly the cap
// that its own key would make the engine consult. If a conversion site
// marshalled an absent cap as is_set = true, the core would receive Some(0) on
// that live chain and reject the order. Keep the asymmetry: giving either
// barrier both caps, keying both on one asset, or swapping which barrier
// carries which cap destroys the discrimination silently.
TEST(EngineConfigure,
     OrderSizeAssetUpdatesPreserveAbsenceOnTheChainEachKeyFeeds) {
  Engine engine = BrokerOrderSizeEngine(policies::OrderSizeLimit::Both(
      Quantity::FromString("100"),
      ::openpit::param::Volume::FromString("100000")));
  engine.Configure().OrderSizeLimit(
      policies::OrderSizeLimitPolicyName,
      policies::OrderSizeBrokerBarrierUpdate::Unchanged(),
      std::vector<policies::OrderSizeAssetBarrier>{
          policies::OrderSizeAssetBarrier(
              policies::OrderSizeLimit::Quantity(Quantity::FromString("10")),
              ::openpit::param::Asset("USD")),
          policies::OrderSizeAssetBarrier(
              policies::OrderSizeLimit::Notional(
                  ::openpit::param::Volume::FromString("1000")),
              ::openpit::param::Asset("AAPL"))},
      std::vector<policies::OrderSizeAccountAssetBarrier>{});

  EXPECT_TRUE(engine.StartPreTrade(SizedOrder("5", "100")).Passed());
}

TEST(EngineConfigure,
     OrderSizeAccountAssetUpdatesPreserveAbsenceOnTheChainEachKeyFeeds) {
  Engine engine = BrokerOrderSizeEngine(policies::OrderSizeLimit::Both(
      Quantity::FromString("100"),
      ::openpit::param::Volume::FromString("100000")));
  engine.Configure().OrderSizeLimit(
      policies::OrderSizeLimitPolicyName,
      policies::OrderSizeBrokerBarrierUpdate::Unchanged(),
      std::vector<policies::OrderSizeAssetBarrier>{},
      std::vector<policies::OrderSizeAccountAssetBarrier>{
          policies::OrderSizeAccountAssetBarrier(
              policies::OrderSizeLimit::Quantity(Quantity::FromString("10")),
              AccountId::FromUint64(2), ::openpit::param::Asset("USD")),
          policies::OrderSizeAccountAssetBarrier(
              policies::OrderSizeLimit::Notional(
                  ::openpit::param::Volume::FromString("1000")),
              AccountId::FromUint64(2), ::openpit::param::Asset("AAPL"))});

  EXPECT_TRUE(engine.StartPreTrade(SizedOrder("5", "100", 2)).Passed());
}

TEST(OrderSizeLimit, QuantityOnlyBrokerEnforcesBoundaryAndPreservesAbsence) {
  const Engine engine = BrokerOrderSizeEngine(
      policies::OrderSizeLimit::Quantity(Quantity::FromString("10")));

  const openpit::pretrade::StartResult above =
      engine.StartPreTrade(SizedOrder("11", "1"));
  EXPECT_FALSE(above.Passed());
  ASSERT_EQ(above.rejects.size(), 1u);
  EXPECT_EQ(above.rejects.front().code, RejectCode::OrderQtyExceedsLimit);
  EXPECT_TRUE(engine.StartPreTrade(SizedOrder("10", "1")).Passed());
}

TEST(OrderSizeLimit, NotionalOnlyBrokerEnforcesBoundaryAndPreservesAbsence) {
  const Engine engine =
      BrokerOrderSizeEngine(policies::OrderSizeLimit::Notional(
          ::openpit::param::Volume::FromString("1000")));

  const openpit::pretrade::StartResult above =
      engine.StartPreTrade(SizedOrder("1", "1001"));
  EXPECT_FALSE(above.Passed());
  ASSERT_EQ(above.rejects.size(), 1u);
  EXPECT_EQ(above.rejects.front().code, RejectCode::OrderNotionalExceedsLimit);
  EXPECT_TRUE(engine.StartPreTrade(SizedOrder("1", "1000")).Passed());
}

TEST(OrderSizeLimit,
     SettlementAssetNotionalLimitsEnforceBoundariesAndPreserveAbsence) {
  const Engine engine = SettlementAssetOrderSizeEngine(
      policies::OrderSizeLimit::Notional(
          ::openpit::param::Volume::FromString("1000")),
      policies::OrderSizeLimit::Notional(
          ::openpit::param::Volume::FromString("2000")));

  EXPECT_TRUE(engine.StartPreTrade(SizedOrder("1000", "1")).Passed());
  const openpit::pretrade::StartResult aboveAsset =
      engine.StartPreTrade(SizedOrder("1001", "1"));
  EXPECT_FALSE(aboveAsset.Passed());
  ASSERT_EQ(aboveAsset.rejects.size(), 1u);
  EXPECT_EQ(aboveAsset.rejects.front().code,
            RejectCode::OrderNotionalExceedsLimit);

  EXPECT_TRUE(engine.StartPreTrade(SizedOrder("2000", "1", 2)).Passed());
  const openpit::pretrade::StartResult aboveAccountAsset =
      engine.StartPreTrade(SizedOrder("2001", "1", 2));
  EXPECT_FALSE(aboveAccountAsset.Passed());
  ASSERT_EQ(aboveAccountAsset.rejects.size(), 1u);
  EXPECT_EQ(aboveAccountAsset.rejects.front().code,
            RejectCode::OrderNotionalExceedsLimit);
}

// Same mechanism as the EngineConfigure probes above, on the registration route
// instead of the retune one: the absent cap of each barrier sits on the chain
// that barrier's key feeds, so marshalling absence as is_set = true would
// supply Some(0) there and reject. Do not "simplify" the asymmetry away.
TEST(OrderSizeLimit,
     AssetBarrierRegistrationPreservesAbsenceOnTheChainEachKeyFeeds) {
  EngineBuilder builder(SyncPolicy::None);
  policies::OrderSizeLimitPolicy config;
  config.AssetBarrier(policies::OrderSizeAssetBarrier(
      policies::OrderSizeLimit::Quantity(Quantity::FromString("10")),
      ::openpit::param::Asset("USD")));
  config.AssetBarrier(policies::OrderSizeAssetBarrier(
      policies::OrderSizeLimit::Notional(
          ::openpit::param::Volume::FromString("1000")),
      ::openpit::param::Asset("AAPL")));
  config.AddTo(builder);
  const Engine engine = builder.Build();

  EXPECT_TRUE(engine.StartPreTrade(SizedOrder("5", "100")).Passed());
}

TEST(OrderSizeLimit,
     AccountAssetBarrierRegistrationPreservesAbsenceOnTheChainEachKeyFeeds) {
  EngineBuilder builder(SyncPolicy::None);
  policies::OrderSizeLimitPolicy config;
  config.AccountAssetBarrier(policies::OrderSizeAccountAssetBarrier(
      policies::OrderSizeLimit::Quantity(Quantity::FromString("10")),
      AccountId::FromUint64(2), ::openpit::param::Asset("USD")));
  config.AccountAssetBarrier(policies::OrderSizeAccountAssetBarrier(
      policies::OrderSizeLimit::Notional(
          ::openpit::param::Volume::FromString("1000")),
      AccountId::FromUint64(2), ::openpit::param::Asset("AAPL")));
  config.AddTo(builder);
  const Engine engine = builder.Build();

  EXPECT_TRUE(engine.StartPreTrade(SizedOrder("5", "100", 2)).Passed());
}

TEST(OrderSizeLimit, ExplicitZeroSettlementAssetLimitsRemainPresent) {
  const Engine engine = SettlementAssetOrderSizeEngine(
      policies::OrderSizeLimit::Notional(
          ::openpit::param::Volume::FromString("0")),
      policies::OrderSizeLimit::Notional(
          ::openpit::param::Volume::FromString("0")));

  const openpit::pretrade::StartResult asset =
      engine.StartPreTrade(SizedOrder("1", "1"));
  EXPECT_FALSE(asset.Passed());
  ASSERT_EQ(asset.rejects.size(), 1u);
  EXPECT_EQ(asset.rejects.front().code, RejectCode::OrderNotionalExceedsLimit);
  const openpit::pretrade::StartResult accountAsset =
      engine.StartPreTrade(SizedOrder("1", "1", 2));
  EXPECT_FALSE(accountAsset.Passed());
  ASSERT_EQ(accountAsset.rejects.size(), 1u);
  EXPECT_EQ(accountAsset.rejects.front().code,
            RejectCode::OrderNotionalExceedsLimit);
}

TEST(OrderSizeLimit, ExplicitZeroQuantityRejectsEveryPositiveOrder) {
  const Engine engine = BrokerOrderSizeEngine(
      policies::OrderSizeLimit::Quantity(Quantity::FromString("0")));

  const openpit::pretrade::StartResult result =
      engine.StartPreTrade(SizedOrder("1", "1"));
  EXPECT_FALSE(result.Passed());
  ASSERT_EQ(result.rejects.size(), 1u);
  EXPECT_EQ(result.rejects.front().code, RejectCode::OrderQtyExceedsLimit);
}

TEST(EngineConfigure, SpotFundsLimitModeUpdateBuildsThroughAccessor) {
  EngineBuilder builder(SyncPolicy::Full);
  builder.Add(policies::SpotFundsPolicy{});
  Engine engine = builder.Build();

  EXPECT_NO_THROW(engine.Configure().SpotFundsGlobalLimitMode(
      policies::SpotFundsPolicyName, policies::SpotFundsLimitMode::TrackOnly));
}

//------------------------------------------------------------------------------
// Account-group lookup via the Accounts view.

TEST(EngineAccountGroup, AbsentForUngroupedAccount) {
  EngineBuilder builder(SyncPolicy::Full);
  builder.Add(policies::OrderValidationPolicy{});
  Engine engine = builder.Build();

  EXPECT_FALSE(engine.Accounts().GroupOf(AccountId::FromUint64(1)).has_value());
}

}  // namespace
