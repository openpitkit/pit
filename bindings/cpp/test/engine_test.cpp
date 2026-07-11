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

#include "openpit/account_adjustment.hpp"
#include "openpit/model.hpp"
#include "openpit/pretrade/pretrade.hpp"
#include "openpit/reject.hpp"

#include <gtest/gtest.h>

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
// settled in USD at price 100, mirroring the Go `rateLimitTestOrder` helper.
[[nodiscard]] openpit::model::Order TestOrder(std::uint64_t accountId) {
  openpit::model::Order order;
  openpit::model::OrderOperation op;
  op.instrument = openpit::model::Instrument("AAPL", "USD");
  op.accountId = ::openpit::param::AccountId::FromUint64(accountId);
  op.side = openpit::model::Side::Buy;
  op.tradeAmount =
      openpit::model::TradeAmount::OfQuantity(Quantity::FromString("1"));
  op.price = Price::FromString("100");
  order.operation = std::move(op);
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
  EXPECT_TRUE(probe.AccountBlocks().empty());

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
// Request::Execute (deferred two-stage flow).

TEST(EngineRequest, ExecutePassesThenCommit) {
  Engine engine = SingleOrderEngine();

  openpit::pretrade::StartResult start = engine.StartPreTrade(TestOrder(1));
  ASSERT_TRUE(start.request.has_value());

  openpit::pretrade::ExecuteResult executed = start.request->Execute();
  EXPECT_TRUE(executed.Passed());
  ASSERT_TRUE(executed.reservation.has_value());
  EXPECT_NO_THROW(executed.reservation->Commit());
}

TEST(EngineRequest, SecondDeferredFlowIsRejectedAtStart) {
  Engine engine = SingleOrderEngine();

  // Drive the first order all the way to commit.
  openpit::pretrade::StartResult first = engine.StartPreTrade(TestOrder(1));
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

  openpit::pretrade::StartResult start = engine.StartPreTrade(TestOrder(1));
  ASSERT_TRUE(start.request.has_value());

  EXPECT_THROW(
      { static_cast<void>(start.request->Execute()); }, std::runtime_error);
}

TEST(EngineRequest, OwnsConcreteOrderUntilDeferredExecution) {
  const auto observedTag = std::make_shared<std::string>();
  EngineBuilder builder(SyncPolicy::Full);
  openpit::pretrade::CustomPolicy<DeferredOrderPolicy> policy(
      "DeferredOrderPolicy", DeferredOrderPolicy(observedTag));
  builder.Add(policy);
  Engine engine = builder.Build();

  std::optional<openpit::pretrade::StartResult> start;
  {
    DeferredOrder order;
    order.strategyTag = "survives-caller-scope";
    start.emplace(engine.StartPreTrade(order));
    ASSERT_TRUE(start->request.has_value());
  }

  openpit::pretrade::ExecuteResult executed = start->request->Execute();
  ASSERT_TRUE(executed.Passed());
  EXPECT_EQ(*observedTag, "survives-caller-scope");
}

//------------------------------------------------------------------------------
// ApplyExecutionReport.

// Builds an execution report carrying the operation identity for `accountId`.
[[nodiscard]] openpit::model::ExecutionReport TestReport(
    std::uint64_t accountId) {
  openpit::model::ExecutionReport report;
  openpit::model::ExecutionReportOperation op;
  op.instrument = openpit::model::Instrument("AAPL", "USD");
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
  EXPECT_TRUE(result.accountAdjustmentOutcomes.empty());
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
// The adjustment value type is authored by a parallel module
// (openpit/account_adjustment.hpp). The empty-batch path exercises the engine
// method end-to-end without depending on that type: a zero-length batch is
// accepted and applies cleanly. A minimal local stub satisfies the template's
// `Raw()` requirement; it is never invoked for an empty batch.

struct StubAdjustment {
  [[nodiscard]] OpenPitAccountAdjustment Raw() const noexcept {
    return OpenPitAccountAdjustment{};
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

  EXPECT_NO_THROW(engine.SetAccountCurrency(account, "USD"));
  EXPECT_NO_THROW(engine.ClearAccountCurrency(account));

  ASSERT_FALSE(engine.Accounts().RegisterGroup({account}, group).has_value());
  EXPECT_NO_THROW(engine.SetAccountGroupCurrency(group, "USD"));
  EXPECT_NO_THROW(engine.ClearAccountGroupCurrency(group));
}

TEST(EngineConfigure, RateLimitUpdateChangesRuntimeBudget) {
  Engine engine = SingleOrderEngine();

  engine.Configure().RateLimit(
      policies::RateLimitPolicyName,
      policies::RateLimitBrokerBarrier(policies::RateLimit(
          /*maxOrders=*/2, /*windowNanoseconds=*/60'000'000'000)));

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
        policies::RateLimitBrokerBarrier(policies::RateLimit(
            /*maxOrders=*/2, /*windowNanoseconds=*/60'000'000'000)));
    FAIL() << "Configure().RateLimit should have thrown";
  } catch (const openpit::ConfigureError& error) {
    EXPECT_EQ(error.Kind(), openpit::ConfigureErrorKind::Unknown);
  }
}

TEST(EngineConfigure, SpotFundsLimitModeUpdateBuildsThroughAccessor) {
  EngineBuilder builder(SyncPolicy::Full);
  builder.Add(policies::SpotFundsPolicy{});
  Engine engine = builder.Build();

  EXPECT_NO_THROW(engine.Configure().SpotFundsGlobalLimitMode(
      policies::SpotFundsPolicyName, policies::SpotFundsLimitMode::TrackOnly));
}

//------------------------------------------------------------------------------
// Account-group lookup via the engine read query.

TEST(EngineAccountGroup, AbsentForUngroupedAccount) {
  EngineBuilder builder(SyncPolicy::Full);
  builder.Add(policies::OrderValidationPolicy{});
  Engine engine = builder.Build();

  EXPECT_FALSE(engine.AccountGroup(AccountId::FromUint64(1)).has_value());
}

}  // namespace
