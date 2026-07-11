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

// Source: Custom-Cpp-Types.md
//
// Compiling mirror of the C++ snippets published on the Custom C++ Types wiki
// page. The custom payload types, the typed policy, the adapter aliases, and
// the engine composition below are the exact user code from the page (modulo
// the assertions that pin each outcome). When a snippet here changes, update
// the matching block in Custom-Cpp-Types.md and vice versa.

#include "openpit/adapters.hpp"
#include "openpit/engine.hpp"
#include "openpit/model.hpp"
#include "openpit/pretrade/pretrade.hpp"
#include "openpit/reject.hpp"

#include <gtest/gtest.h>

#include <cassert>
#include <memory>
#include <optional>
#include <string>
#include <string_view>
#include <type_traits>
#include <utility>
#include <vector>

namespace {

//------------------------------------------------------------------------------
// Step 1 - define the custom payload types

// A desk order carries the standard model fields plus a strategy tag the
// engine does not own. The inherited EngineRaw() submits the standard view.
struct StrategyOrder : public openpit::model::Order {
  std::string strategyTag;
};

// A desk report carries the standard model fields plus the venue execution id.
// Its inherited EngineRaw() lets ApplyExecutionReport preserve the dynamic
// type.
struct StrategyReport : public openpit::model::ExecutionReport {
  std::string venueExecId;
};

//------------------------------------------------------------------------------
// Step 2 - write the typed policy

class StrategyTagPolicy {
 public:
  explicit StrategyTagPolicy(std::shared_ptr<std::string> appliedVenueExecId)
      : m_appliedVenueExecId(std::move(appliedVenueExecId)) {}

  [[nodiscard]] std::string_view Name() const noexcept {
    return "StrategyTagPolicy";
  }

  // Start stage: reject a blocked strategy tag before the order enters the
  // pipeline. The typed StrategyOrder gives direct access to the project field.
  [[nodiscard]] std::optional<openpit::pretrade::Reject> CheckPreTradeStart(
      const StrategyOrder& order) const {
    if (order.strategyTag == "blocked") {
      return openpit::pretrade::Reject(
          std::string(Name()), openpit::pretrade::RejectScope::Order,
          openpit::pretrade::RejectCode::ComplianceRestriction,
          "strategy blocked", order.strategyTag);
    }
    return std::nullopt;
  }

  // Main stage: push a reject into the decision; an empty decision accepts.
  void PerformPreTradeCheck(const StrategyOrder& order,
                            const openpit::pretrade::Context& context,
                            openpit::tx::Mutations& mutations,
                            openpit::pretrade::Result& result,
                            openpit::pretrade::PolicyDecision& decision) const {
    static_cast<void>(context);
    static_cast<void>(mutations);
    static_cast<void>(result);
    if (order.strategyTag.empty()) {
      decision.Push(openpit::pretrade::Reject(
          std::string(Name()), openpit::pretrade::RejectScope::Order,
          openpit::pretrade::RejectCode::MissingRequiredField,
          "strategy tag is required", "strategyTag"));
    }
  }

  // Post-trade: typed metadata arrives next to the normalized standard fields.
  [[nodiscard]] std::vector<openpit::accounts::AccountBlock>
  ApplyExecutionReport(
      const openpit::pretrade::PostTradeContext& context,
      const StrategyReport& report,
      openpit::pretrade::PostTradeAdjustments& adjustments) const {
    static_cast<void>(context);
    static_cast<void>(adjustments);
    *m_appliedVenueExecId = report.venueExecId;
    return {};
  }

 private:
  std::shared_ptr<std::string> m_appliedVenueExecId;
};

//------------------------------------------------------------------------------
// Step 3 - bridge with an adapter (choose a cast mode)

// SafeSlow: the adapter uses dynamic_cast and produces a deterministic
// type-mismatch reject (start/main stage) or no account blocks (report stage)
// when the arriving payload is not the client type. Safe default at the
// boundary.
using StrategyAdapter = openpit::pretrade::PolicyAdapterWithSafeSlowArgType<
    StrategyTagPolicy, StrategyOrder, StrategyReport>;

// UnsafeFast: direct static_cast, no runtime type check. A mismatched payload
// is undefined behavior, so this is only for closed, statically paired wiring.
using StrategyAdapterFast =
    openpit::pretrade::PolicyAdapterWithUnsafeFastArgType<
        StrategyTagPolicy, StrategyOrder, StrategyReport>;

//------------------------------------------------------------------------------
// Step 1: the custom payloads derive from the concrete standard models and
// carry project fields next to the inherited model groups.

TEST(WikiCustomCppTypes, DefineCustomPayloadTypes) {
  StrategyOrder order;
  order.strategyTag = "alpha";

  StrategyReport report;
  report.venueExecId = "EX-1";

  // Each derives from the adapter's polymorphic base.
  const openpit::Order& orderBase = order;
  const openpit::ExecutionReport& reportBase = report;
  static_cast<void>(orderBase);
  static_cast<void>(reportBase);

  EXPECT_EQ(order.strategyTag, "alpha");
  EXPECT_EQ(report.venueExecId, "EX-1");
}

//------------------------------------------------------------------------------
// Step 2 / Step 3: the SafeSlow start adapter dispatches to the typed policy on
// a matching payload and emits a deterministic reject on a foreign one.

TEST(WikiCustomCppTypes, StartAdapterTypedDispatchAndMismatch) {
  StrategyAdapter adapter{StrategyTagPolicy{std::make_shared<std::string>()}};

  StrategyOrder good;
  good.strategyTag = "alpha";
  EXPECT_FALSE(adapter.CheckPreTradeStart(good).has_value());

  StrategyOrder blocked;
  blocked.strategyTag = "blocked";
  const std::optional<openpit::pretrade::Reject> reject =
      adapter.CheckPreTradeStart(blocked);
  ASSERT_TRUE(reject.has_value());
  EXPECT_EQ(reject->code, openpit::pretrade::RejectCode::ComplianceRestriction);

  // A plain model order is NOT a StrategyOrder: SafeSlow yields a type-mismatch
  // reject rather than dispatching to the policy.
  openpit::model::Order foreign;
  const std::optional<openpit::pretrade::Reject> mismatch =
      adapter.CheckPreTradeStart(foreign);
  ASSERT_TRUE(mismatch.has_value());
  EXPECT_EQ(mismatch->code, openpit::pretrade::RejectCode::Other);
}

//------------------------------------------------------------------------------
// Step 3: the main-stage SafeSlow adapter pushes the policy's reject into the
// decision on a typed payload.

TEST(WikiCustomCppTypes, MainAdapterTypedDispatch) {
  StrategyAdapter adapter{StrategyTagPolicy{std::make_shared<std::string>()}};

  StrategyOrder missingTag;  // empty strategyTag -> policy rejects.
  const openpit::pretrade::Context context(missingTag);

  openpit::tx::Mutations mutations(nullptr);
  openpit::pretrade::Result result(nullptr);
  openpit::pretrade::PolicyDecision decision;
  adapter.PerformPreTradeCheck(context, mutations, result, decision);
  ASSERT_TRUE(decision.IsRejected());
  EXPECT_EQ(decision.rejects.front().code,
            openpit::pretrade::RejectCode::MissingRequiredField);
}

//------------------------------------------------------------------------------
// Step 4 / Step 5: compose a typed engine, submit a custom order, and apply a
// custom execution report through their inherited standard model views.

TEST(WikiCustomCppTypes, ComposeEngineAndSubmit) {
  // --- begin wiki snippet (Step 4) ---
  const auto appliedVenueExecId = std::make_shared<std::string>();
  openpit::EngineBuilder builder(openpit::SyncPolicy::None);

  // The CustomPolicy adopts the adapter (which owns the client policy) and
  // wires the detected hooks into the C ABI custom-policy vtable.
  openpit::pretrade::CustomPolicy<StrategyAdapter> policy(
      "StrategyTagPolicy",
      StrategyAdapter{StrategyTagPolicy{appliedVenueExecId}});
  builder.Add(policy);

  const openpit::Engine engine = builder.Build();
  // --- end wiki snippet (Step 4) ---

  ASSERT_TRUE(static_cast<bool>(engine));

  // --- begin wiki snippet (Step 5) ---
  StrategyOrder order;
  openpit::model::OrderOperation op;
  op.instrument = openpit::model::Instrument("AAPL", "USD");
  op.accountId = ::openpit::param::AccountId::FromUint64(1);
  op.side = openpit::model::Side::Buy;
  op.tradeAmount = openpit::model::TradeAmount::OfQuantity(
      openpit::param::Quantity::FromString("1"));
  op.price = openpit::param::Price::FromString("100");
  order.operation = std::move(op);
  order.strategyTag = "alpha";

  // Submit the typed order; inherited EngineRaw() supplies the standard view,
  // while the adapter preserves the StrategyOrder dynamic type.
  openpit::pretrade::ExecuteResult result = engine.ExecutePreTrade(order);
  if (result.Passed()) {
    result.reservation->Commit();
  }

  StrategyOrder blockedOrder = order;
  blockedOrder.strategyTag = "blocked";
  const openpit::pretrade::StartResult blocked =
      engine.StartPreTrade(std::move(blockedOrder));
  assert(!blocked.Passed());
  assert(blocked.rejects.front().code ==
         openpit::pretrade::RejectCode::ComplianceRestriction);

  StrategyOrder missingTagOrder = order;
  missingTagOrder.strategyTag.clear();
  const openpit::pretrade::ExecuteResult missingTag =
      engine.ExecutePreTrade(std::move(missingTagOrder));
  assert(!missingTag.Passed());
  assert(missingTag.rejects.front().code ==
         openpit::pretrade::RejectCode::MissingRequiredField);

  StrategyReport report;
  openpit::model::ExecutionReportOperation reportOp;
  reportOp.instrument = openpit::model::Instrument("AAPL", "USD");
  reportOp.accountId = ::openpit::param::AccountId::FromUint64(1);
  reportOp.side = openpit::model::Side::Buy;
  report.operation = std::move(reportOp);
  report.venueExecId = "EX-1";

  const openpit::PostTradeResult post = engine.ApplyExecutionReport(report);
  assert(post.accountBlocks.empty());
  assert(*appliedVenueExecId == "EX-1");
  // --- end wiki snippet (Step 5) ---

  EXPECT_TRUE(result.Passed());
}

// Keeps the UnsafeFast alias referenced so it is type-checked even though the
// happy-path tests above exercise the SafeSlow adapters.
static_assert(
    std::is_same_v<StrategyAdapterFast,
                   openpit::pretrade::PolicyAdapterWithUnsafeFastArgType<
                       StrategyTagPolicy, StrategyOrder, StrategyReport>>,
    "UnsafeFast alias must resolve to the UnsafeFast adapter specialization");

}  // namespace
