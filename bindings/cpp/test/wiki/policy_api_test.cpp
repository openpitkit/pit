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

// Source: Policy-API.md
//
// Compiling mirror of the C++ snippets published on the Policy-API wiki page.
// Each TEST runs the same policy/engine code shown in a C++ wiki block (modulo
// the minimal harness: order/engine construction and assertions). Keep the
// published snippet and the corresponding test body in sync: when one changes,
// change the other.

// The pre-trade / reject headers define `RejectScope` / `RejectCode` with their
// fixed underlying types; they must precede `openpit/adapters.hpp`, whose
// opaque forward declarations of those scoped enums are only a compatible
// redeclaration when the full definition is already in scope.
#include "openpit/adapters.hpp"
#include "openpit/engine.hpp"
#include "openpit/model.hpp"
#include "openpit/pretrade/pretrade.hpp"
#include "openpit/reject.hpp"

#include <gtest/gtest.h>

#include <cassert>
#include <cstdint>
#include <memory>
#include <optional>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

namespace {

using openpit::param::Price;
using openpit::param::Quantity;
using openpit::param::Volume;
using openpit::pretrade::Context;
using openpit::pretrade::CustomPolicy;
using openpit::pretrade::PolicyDecision;
using openpit::pretrade::PushReject;
using openpit::pretrade::Reject;
using openpit::pretrade::RejectCode;
using openpit::pretrade::RejectScope;

//------------------------------------------------------------------------------
// Example: Custom Main-Stage Policy
//
// NotionalCapPolicy rejects any order whose requested settlement notional
// exceeds an absolute cap. The public C++ surface exposes the requested amount
// as an `openpit::model::TradeAmount`: a volume amount is already the notional,
// while a quantity amount is priced into one (notional = price * quantity).
// Absent amounts or an unpriceable quantity become explicit rejects rather
// than exceptions.

// >>> WIKI SNIPPET BEGIN: Custom Main-Stage Policy
// Computes settlement notional from a per-unit price and an instrument
// quantity (notional = price * quantity), crossing the exact-decimal C ABI so
// the result is bit-for-bit identical across language bindings. Returns
// nullopt when the engine reports the multiplication as a value error, which
// the caller turns into an explicit reject rather than an exception.
[[nodiscard]] std::optional<openpit::param::Volume> CalculateNotional(
    const openpit::param::Price& price,
    const openpit::param::Quantity& quantity) {
  OpenPitParamVolume raw{};
  OpenPitParamError* error = nullptr;
  if (!openpit_param_price_calculate_volume(price.Raw(), quantity.Raw(), &raw,
                                            &error)) {
    if (error != nullptr) {
      openpit_destroy_param_error(error);
    }
    return std::nullopt;
  }
  return openpit::param::Volume::FromRaw(raw);
}

class NotionalCapPolicy {
 public:
  // Policy-local config: reject any order above this absolute notional.
  explicit NotionalCapPolicy(openpit::param::Volume maxAbsNotional)
      : m_maxAbsNotional(maxAbsNotional) {}

  [[nodiscard]] std::string_view Name() const noexcept {
    return "NotionalCapPolicy";
  }

  void PerformPreTradeCheck(const openpit::model::Order& order,
                            const openpit::pretrade::Context& context,
                            openpit::tx::Mutations& mutations,
                            openpit::pretrade::Result& result,
                            openpit::pretrade::PolicyDecision& decision) const {
    static_cast<void>(context);
    static_cast<void>(mutations);
    static_cast<void>(result);
    if (!order.operation.has_value()) {
      openpit::pretrade::PushReject(
          decision,
          openpit::pretrade::Reject(
              std::string(Name()), openpit::pretrade::RejectScope::Order,
              openpit::pretrade::RejectCode::MissingRequiredField,
              "required order field missing", "operation is not set"));
      return;
    }
    const openpit::model::OrderOperation& operation = *order.operation;

    // Translate the public order surface into one number that this policy can
    // reason about: requested notional.
    if (!operation.tradeAmount.has_value()) {
      openpit::pretrade::PushReject(
          decision,
          openpit::pretrade::Reject(
              std::string(Name()), openpit::pretrade::RejectScope::Order,
              openpit::pretrade::RejectCode::MissingRequiredField,
              "required order field missing", "trade_amount is not set"));
      return;
    }
    const openpit::model::TradeAmount& tradeAmount = *operation.tradeAmount;

    // A volume trade amount is already the notional; a quantity trade amount
    // must be priced into a notional (notional = price * quantity).
    std::optional<openpit::param::Volume> requestedNotional =
        tradeAmount.AsVolume();
    if (!requestedNotional.has_value()) {
      const std::optional<openpit::param::Quantity> quantity =
          tradeAmount.AsQuantity();
      if (!operation.price.has_value()) {
        openpit::pretrade::PushReject(
            decision,
            openpit::pretrade::Reject(
                std::string(Name()), openpit::pretrade::RejectScope::Order,
                openpit::pretrade::RejectCode::OrderValueCalculationFailed,
                "order value calculation failed",
                "price not provided for evaluating notional"));
        return;
      }
      requestedNotional = CalculateNotional(*operation.price, *quantity);
      if (!requestedNotional.has_value()) {
        openpit::pretrade::PushReject(
            decision,
            openpit::pretrade::Reject(
                std::string(Name()), openpit::pretrade::RejectScope::Order,
                openpit::pretrade::RejectCode::OrderValueCalculationFailed,
                "order value calculation failed",
                "price and quantity could not be used to evaluate notional"));
        return;
      }
    }

    if (*requestedNotional > m_maxAbsNotional) {
      // Business validation failures should become explicit rejects.
      openpit::pretrade::PushReject(
          decision,
          openpit::pretrade::Reject(
              std::string(Name()), openpit::pretrade::RejectScope::Order,
              openpit::pretrade::RejectCode::RiskLimitExceeded,
              "strategy cap exceeded",
              "requested notional " + requestedNotional->ToString() +
                  ", max allowed: " + m_maxAbsNotional.ToString()));
      return;
    }

    // This policy only validates. It does not reserve mutable state.
  }

  [[nodiscard]] std::vector<openpit::accounts::AccountBlock>
  ApplyExecutionReport(const openpit::pretrade::PostTradeContext& context,
                       const openpit::ExecutionReport& report,
                       openpit::pretrade::PostTradeAdjustments& adjustments,
                       openpit::pretrade::PostTradePnls& pnls) const {
    static_cast<void>(context);
    static_cast<void>(report);
    static_cast<void>(adjustments);
    static_cast<void>(pnls);
    return {};
  }

 private:
  openpit::param::Volume m_maxAbsNotional;
};
// <<< WIKI SNIPPET END: Custom Main-Stage Policy

// Wraps the policy in the SafeSlow main-stage adapter so the context order is
// recovered as `openpit::model::Order` before the check runs.
using NotionalCapAdapter = openpit::pretrade::PolicyAdapterWithSafeSlowArgType<
    NotionalCapPolicy, openpit::model::Order, openpit::ExecutionReport>;

template <typename Adapter>
void RunMainCheck(const Adapter& adapter, const Context& context,
                  PolicyDecision& decision) {
  openpit::tx::Mutations mutations(nullptr);
  openpit::pretrade::Result result(nullptr);
  adapter.PerformPreTradeCheck(context, mutations, result, decision);
}

// Builds a notional order carrying `volume` settlement notional on `accountId`.
[[nodiscard]] openpit::model::Order NotionalOrder(std::uint64_t accountId,
                                                  std::string_view volume) {
  openpit::model::Order order;
  openpit::model::OrderOperation op;
  op.instrument = openpit::model::Instrument(::openpit::param::Asset("AAPL"),
                                             ::openpit::param::Asset("USD"));
  op.accountId = ::openpit::param::AccountId::FromUint64(accountId);
  op.side = openpit::model::Side::Buy;
  op.tradeAmount =
      openpit::model::TradeAmount::OfVolume(Volume::FromString(volume));
  order.operation = std::move(op);
  return order;
}

// Builds an order whose trade amount is a quantity, optionally priced. The
// policy must price the quantity into a notional rather than reject it.
[[nodiscard]] openpit::model::Order QuantityOrder(
    std::uint64_t accountId, std::string_view quantity,
    std::optional<std::string_view> price) {
  openpit::model::Order order;
  openpit::model::OrderOperation op;
  op.instrument = openpit::model::Instrument(::openpit::param::Asset("AAPL"),
                                             ::openpit::param::Asset("USD"));
  op.accountId = ::openpit::param::AccountId::FromUint64(accountId);
  op.side = openpit::model::Side::Buy;
  op.tradeAmount =
      openpit::model::TradeAmount::OfQuantity(Quantity::FromString(quantity));
  if (price.has_value()) {
    op.price = Price::FromString(*price);
  }
  order.operation = std::move(op);
  return order;
}

TEST(PolicyApiCustomMainStage, UnderCapAccepts) {
  const NotionalCapAdapter adapter{
      NotionalCapPolicy{Volume::FromString("1000000")}};
  const openpit::model::Order order = NotionalOrder(99224416, "250000");
  const Context context(order);

  PolicyDecision decision;
  RunMainCheck(adapter, context, decision);
  EXPECT_FALSE(decision.IsRejected());
}

TEST(PolicyApiCustomMainStage, OverCapRejectsWithRiskLimit) {
  const NotionalCapAdapter adapter{
      NotionalCapPolicy{Volume::FromString("1000000")}};
  const openpit::model::Order order = NotionalOrder(99224416, "2500000");
  const Context context(order);

  PolicyDecision decision;
  RunMainCheck(adapter, context, decision);
  ASSERT_TRUE(decision.IsRejected());
  EXPECT_EQ(decision.rejects.front().code, RejectCode::RiskLimitExceeded);
  EXPECT_EQ(decision.rejects.front().policy, "NotionalCapPolicy");
}

TEST(PolicyApiCustomMainStage, MissingOperationRejects) {
  const NotionalCapAdapter adapter{
      NotionalCapPolicy{Volume::FromString("1000000")}};
  const openpit::model::Order order;  // no operation group set.
  const Context context(order);

  PolicyDecision decision;
  RunMainCheck(adapter, context, decision);
  ASSERT_TRUE(decision.IsRejected());
  EXPECT_EQ(decision.rejects.front().code, RejectCode::MissingRequiredField);
}

TEST(PolicyApiCustomMainStage, QuantityUnderCapAccepts) {
  // 1000 * 25 = 25000 settlement notional, under the 1,000,000 cap.
  const NotionalCapAdapter adapter{
      NotionalCapPolicy{Volume::FromString("1000000")}};
  const openpit::model::Order order = QuantityOrder(99224416, "1000", "25");
  const Context context(order);

  PolicyDecision decision;
  RunMainCheck(adapter, context, decision);
  EXPECT_FALSE(decision.IsRejected());
}

TEST(PolicyApiCustomMainStage, QuantityOverCapRejectsWithRiskLimit) {
  // 100000 * 25 = 2,500,000 settlement notional, over the 1,000,000 cap.
  const NotionalCapAdapter adapter{
      NotionalCapPolicy{Volume::FromString("1000000")}};
  const openpit::model::Order order = QuantityOrder(99224416, "100000", "25");
  const Context context(order);

  PolicyDecision decision;
  RunMainCheck(adapter, context, decision);
  ASSERT_TRUE(decision.IsRejected());
  EXPECT_EQ(decision.rejects.front().code, RejectCode::RiskLimitExceeded);
}

TEST(PolicyApiCustomMainStage, QuantityWithoutPriceRejectsWithValueCalc) {
  const NotionalCapAdapter adapter{
      NotionalCapPolicy{Volume::FromString("1000000")}};
  const openpit::model::Order order =
      QuantityOrder(99224416, "1000", std::nullopt);
  const Context context(order);

  PolicyDecision decision;
  RunMainCheck(adapter, context, decision);
  ASSERT_TRUE(decision.IsRejected());
  EXPECT_EQ(decision.rejects.front().code,
            RejectCode::OrderValueCalculationFailed);
}

//------------------------------------------------------------------------------
// Example: Rollback Safety Pattern
//
// The policy applies a tentative reservation to its own state eagerly, then
// validates it. In the C++ binding a main-stage reject is a value reported into
// the `PolicyDecision`; the engine discards a rejected decision without
// applying its reservation, so the policy restores its own tentative state when
// it decides to reject.

// >>> WIKI SNIPPET BEGIN: Rollback Safety Pattern
class ReserveThenValidatePolicy {
 public:
  ReserveThenValidatePolicy() = default;

  [[nodiscard]] std::string_view Name() const noexcept {
    return "ReserveThenValidatePolicy";
  }

  void PerformPreTradeCheck(const openpit::model::Order& order,
                            const openpit::pretrade::Context& context,
                            openpit::tx::Mutations& mutations,
                            openpit::pretrade::Result& result,
                            openpit::pretrade::PolicyDecision& decision) const {
    static_cast<void>(order);
    static_cast<void>(context);
    static_cast<void>(mutations);
    static_cast<void>(result);

    // Pretend that this request needs a temporary reservation of 100. We apply
    // it eagerly because downstream logic wants to observe the tentative state
    // immediately.
    const openpit::param::Volume prevReserved = m_reserved;
    const openpit::param::Volume nextReserved =
        openpit::param::Volume::FromString("100");
    m_reserved = nextReserved;

    if (m_reserved > m_limit) {
      // The decision is rejected, so the engine will not apply this request:
      // restore the previous state before returning the reject.
      m_reserved = prevReserved;
      openpit::pretrade::PushReject(
          decision,
          openpit::pretrade::Reject(
              std::string(Name()), openpit::pretrade::RejectScope::Order,
              openpit::pretrade::RejectCode::RiskLimitExceeded,
              "temporary reservation exceeds limit",
              "reserved " + nextReserved.ToString() +
                  ", limit: " + m_limit.ToString()));
    }
  }

  [[nodiscard]] std::vector<openpit::accounts::AccountBlock>
  ApplyExecutionReport(const openpit::pretrade::PostTradeContext& context,
                       const openpit::ExecutionReport& report,
                       openpit::pretrade::PostTradeAdjustments& adjustments,
                       openpit::pretrade::PostTradePnls& pnls) const {
    static_cast<void>(context);
    static_cast<void>(report);
    static_cast<void>(adjustments);
    static_cast<void>(pnls);
    return {};
  }

 private:
  mutable openpit::param::Volume m_reserved =
      openpit::param::Volume::FromString("0");
  openpit::param::Volume m_limit = openpit::param::Volume::FromString("50");
};
// <<< WIKI SNIPPET END: Rollback Safety Pattern

using ReserveThenValidateAdapter =
    openpit::pretrade::PolicyAdapterWithSafeSlowArgType<
        ReserveThenValidatePolicy, openpit::model::Order,
        openpit::ExecutionReport>;

TEST(PolicyApiRollbackSafety, OverLimitRejectsAndRestoresState) {
  const ReserveThenValidateAdapter adapter{ReserveThenValidatePolicy{}};
  const openpit::model::Order order = NotionalOrder(99224416, "10");
  const Context context(order);

  PolicyDecision decision;
  RunMainCheck(adapter, context, decision);
  ASSERT_TRUE(decision.IsRejected());
  EXPECT_EQ(decision.rejects.front().code, RejectCode::RiskLimitExceeded);
}

//------------------------------------------------------------------------------
// Custom `Order` and `Execution Report` Models
//
// Client order/report types carry project-specific metadata on the concrete
// standard models. The SafeSlow adapter recovers each dynamic type before
// invoking the typed start/post-trade hook in a live engine.

// >>> WIKI SNIPPET BEGIN: Custom Order and Execution Report Models
// StrategyOrder carries project-specific metadata alongside the standard order.
struct StrategyOrder : public openpit::model::Order {
  std::string strategyTag;
};

// StrategyReport carries project-specific metadata alongside the standard
// report.
struct StrategyReport : public openpit::model::ExecutionReport {
  std::string venueExecId;
};

// StrategyTagPolicy rejects orders from blocked strategy tags.
class StrategyTagPolicy {
 public:
  explicit StrategyTagPolicy(std::shared_ptr<std::string> appliedVenueExecId)
      : m_appliedVenueExecId(std::move(appliedVenueExecId)) {}

  [[nodiscard]] std::string_view Name() const noexcept {
    return "StrategyTagPolicy";
  }

  [[nodiscard]] std::optional<openpit::pretrade::Reject> CheckPreTradeStart(
      const StrategyOrder& order) const {
    if (order.strategyTag == "blocked") {
      return openpit::pretrade::Reject(
          std::string(Name()), openpit::pretrade::RejectScope::Order,
          openpit::pretrade::RejectCode::ComplianceRestriction,
          "strategy blocked",
          "strategy tag \"" + order.strategyTag + "\" is not allowed");
    }
    return std::nullopt;
  }

  [[nodiscard]] std::vector<openpit::accounts::AccountBlock>
  ApplyExecutionReport(const openpit::pretrade::PostTradeContext& context,
                       const StrategyReport& report,
                       openpit::pretrade::PostTradeAdjustments& adjustments,
                       openpit::pretrade::PostTradePnls& pnls) const {
    static_cast<void>(context);
    static_cast<void>(adjustments);
    static_cast<void>(pnls);
    *m_appliedVenueExecId = report.venueExecId;
    return {};
  }

 private:
  std::shared_ptr<std::string> m_appliedVenueExecId;
};
// <<< WIKI SNIPPET END: Custom Order and Execution Report Models

using StrategyStartAdapter =
    openpit::pretrade::StartPolicyAdapterWithSafeSlowArgType<
        StrategyTagPolicy, StrategyOrder, StrategyReport>;

TEST(PolicyApiCustomModels, AllowedStrategyTagPassesPipeline) {
  // >>> WIKI SNIPPET BEGIN: Custom Models driver
  const auto appliedVenueExecId = std::make_shared<std::string>();
  openpit::pretrade::CustomPolicy<StrategyStartAdapter> policy(
      "StrategyTagPolicy",
      StrategyStartAdapter{StrategyTagPolicy{appliedVenueExecId}});

  openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
  builder.Add(policy);
  openpit::Engine engine = builder.Build();

  StrategyOrder order;
  openpit::model::OrderOperation op;
  op.instrument = openpit::model::Instrument(::openpit::param::Asset("AAPL"),
                                             ::openpit::param::Asset("USD"));
  op.accountId = ::openpit::param::AccountId::FromUint64(99224416);
  op.side = openpit::model::Side::Buy;
  op.tradeAmount = openpit::model::TradeAmount::OfQuantity(
      openpit::param::Quantity::FromString("10"));
  op.price = openpit::param::Price::FromString("25");
  order.operation = std::move(op);
  order.strategyTag = "alpha";

  openpit::pretrade::StartResult start = engine.StartPreTrade(order);
  assert(start.Passed());

  openpit::pretrade::ExecuteResult execute = start.request->Execute();
  assert(execute.Passed());
  execute.reservation->Commit();

  StrategyReport report;
  openpit::model::ExecutionReportOperation reportOp;
  reportOp.instrument = openpit::model::Instrument(
      ::openpit::param::Asset("AAPL"), ::openpit::param::Asset("USD"));
  reportOp.accountId = ::openpit::param::AccountId::FromUint64(99224416);
  reportOp.side = openpit::model::Side::Buy;
  report.operation = std::move(reportOp);
  report.venueExecId = "venue-42";

  const openpit::PostTradeResult post = engine.ApplyExecutionReport(report);
  assert(post.accountBlocks.empty());
  assert(*appliedVenueExecId == "venue-42");
  // <<< WIKI SNIPPET END: Custom Models driver
}

TEST(PolicyApiCustomModels, BlockedStrategyTagRejectsAtStart) {
  CustomPolicy<StrategyStartAdapter> policy(
      "StrategyTagPolicy",
      StrategyStartAdapter{StrategyTagPolicy{std::make_shared<std::string>()}});

  openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
  builder.Add(policy);
  openpit::Engine engine = builder.Build();

  StrategyOrder order;
  openpit::model::OrderOperation op;
  op.instrument = openpit::model::Instrument(::openpit::param::Asset("AAPL"),
                                             ::openpit::param::Asset("USD"));
  op.accountId = ::openpit::param::AccountId::FromUint64(99224416);
  op.side = openpit::model::Side::Buy;
  op.tradeAmount = openpit::model::TradeAmount::OfQuantity(
      openpit::param::Quantity::FromString("10"));
  op.price = openpit::param::Price::FromString("25");
  order.operation = std::move(op);
  order.strategyTag = "blocked";

  openpit::pretrade::StartResult start = engine.StartPreTrade(order);
  EXPECT_FALSE(start.Passed());
  ASSERT_EQ(start.rejects.size(), 1u);
  EXPECT_EQ(start.rejects.front().code, RejectCode::ComplianceRestriction);
}

//------------------------------------------------------------------------------
// Example: Block an Account from an Adjustment Callback
//
// The callback reports a block while accepting the adjustment. The engine
// records it before returning the batch result; every later start stage for
// that account is then rejected with ACCOUNT_BLOCKED.

// >>> WIKI SNIPPET BEGIN: Block an Account from an Adjustment Callback
// BlockOnAdjustmentPolicy accepts the adjustment and reports an account block.
class BlockOnAdjustmentPolicy {
 public:
  [[nodiscard]] std::string_view Name() const noexcept {
    return "BlockOnAdjustmentPolicy";
  }

  [[nodiscard]] openpit::pretrade::PolicyAccountAdjustmentResult
  ApplyAccountAdjustment(
      const openpit::accountadjustment::Context& context,
      openpit::param::AccountId accountId,
      const openpit::accountadjustment::AccountAdjustment& adjustment,
      openpit::tx::Mutations& mutations,
      openpit::pretrade::AccountOutcomes& outcomes) const {
    static_cast<void>(accountId);
    static_cast<void>(adjustment);
    static_cast<void>(mutations);
    static_cast<void>(outcomes);
    static_cast<void>(context);
    openpit::pretrade::PolicyAccountAdjustmentResult result;
    result.accountBlocks.emplace_back(
        openpit::pretrade::RejectCode::AccountBlocked, std::string(Name()),
        "blocked by account-adjustment policy",
        "custom policy reported an account block from a callback");
    return result;
  }
};

// Builds the canonical single-leg order for `accountId`.
[[nodiscard]] openpit::model::Order AccountOrder(std::uint64_t accountId) {
  openpit::model::Order order;
  openpit::model::OrderOperation op;
  op.instrument = openpit::model::Instrument(::openpit::param::Asset("AAPL"),
                                             ::openpit::param::Asset("USD"));
  op.accountId = ::openpit::param::AccountId::FromUint64(accountId);
  op.side = openpit::model::Side::Buy;
  op.tradeAmount =
      openpit::model::TradeAmount::OfQuantity(Quantity::FromString("10"));
  op.price = Price::FromString("25");
  order.operation = std::move(op);
  return order;
}

TEST(PolicyApiBlockAccount, BlockedAccountIsRejectedWithAccountBlocked) {
  openpit::EngineBuilder builder(openpit::SyncPolicy::None);
  openpit::pretrade::CustomPolicy<BlockOnAdjustmentPolicy> policy(
      "BlockOnAdjustmentPolicy", BlockOnAdjustmentPolicy{});
  builder.Add(policy);
  openpit::Engine engine = builder.Build();

  const openpit::param::AccountId accountId =
      openpit::param::AccountId::FromUint64(99224416);

  // The accepted adjustment reports a block that the engine has already
  // recorded.
  openpit::accountadjustment::BalanceOperation balanceOp;
  balanceOp.asset = ::openpit::param::Asset("USD");
  openpit::accountadjustment::AccountAdjustment adjustment;
  adjustment.operation =
      openpit::accountadjustment::Operation::OfBalance(std::move(balanceOp));
  openpit::accountadjustment::Amount amount;
  amount.balance = openpit::param::AdjustmentAmount::OfAbsolute(
      openpit::param::PositionSize::FromString("0"));
  adjustment.amount = std::move(amount);
  const openpit::AdjustmentResult adjustmentResult =
      engine.ApplyAccountAdjustment(
          accountId, std::vector<openpit::accountadjustment::AccountAdjustment>{
                         adjustment});
  assert(adjustmentResult.Passed());
  assert(adjustmentResult.accountBlocks.size() == 1);

  // A later order on the same account is rejected with ACCOUNT_BLOCKED, without
  // any start-check involvement.
  openpit::pretrade::StartResult blocked =
      engine.StartPreTrade(AccountOrder(99224416));
  // <<< WIKI SNIPPET END: Block an Account from an Adjustment Callback

  ASSERT_FALSE(blocked.Passed());
  ASSERT_EQ(blocked.rejects.size(), 1u);
  EXPECT_EQ(blocked.rejects.front().code, RejectCode::AccountBlocked);
}

}  // namespace
