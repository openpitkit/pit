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

// Example rate_pnl_killswitch demonstrates how an algorithmic trading desk can
// wrap OpenPit's RateLimit and PnlBoundsKillSwitch policies around a C++
// strategy so that a runaway strategy is halted before it floods the venue with
// orders or burns through the loss budget.
//
// What is illustrated:
//
//   - building an engine with two killswitch policies side-by-side
//   - feeding the engine via a single Event stream (orders + fills)
//   - separating venue/strategy side-effects behind a Reactor interface
//   - aggregating accepted/rejected counts, pre-trade latency, and cumulative
//     P&L over the run
//
// Audience: an algo trader who wants an independent supervisor that prevents
// the strategy from "going crazy".
//
// What you typically change to adapt this example to your own application:
//
//  1. Engine policies and limits - see BuildEngine() below.
//  2. The order/report stream - the ScenarioStream in main() is a one-shot
//     replay; real systems plug in a thread driven by venue and strategy
//     events.
//  3. The Reactor implementation - replace LoggingReactor with code that
//     actually submits orders to the venue, updates your strategy book, and
//     halts the strategy when a kill-switch account block is returned.

#pragma once

#include "openpit/engine.hpp"
#include "openpit/model.hpp"
#include "openpit/param.hpp"
#include "openpit/pretrade/policies.hpp"
#include "openpit/reject.hpp"

#include <openpit.h>

#include <chrono>
#include <cstddef>
#include <cstdint>
#include <optional>
#include <string>
#include <variant>
#include <vector>

namespace killswitch {

// =============================================================================
// Section 1 - Public extension points.
// These types and the Stats struct are what application code interacts with.
// Run() below is policy-agnostic; it only knows these types.
// =============================================================================

// Event is the discriminated input to the engine loop: exactly one of an order
// intent or an execution report. RealizedPnl carries the report's P&L so the
// example can track the running balance outside the engine - production code
// would read this from its strategy book instead.
struct OrderEvent {
  const openpit::model::Order *order;
};

struct ReportEvent {
  const openpit::model::ExecutionReport *report;
  openpit::param::Pnl realizedPnl;
};

using Event = std::variant<OrderEvent, ReportEvent>;

// EventStream is the input feed of the engine loop. Implementations:
//
//   - production: wrap a select over the strategy's order queue and the venue's
//     execution-report queue into a single Next() call;
//   - test: back it by a scripted state machine, as ScenarioStream below does.
class EventStream {
public:
  EventStream() = default;
  EventStream(const EventStream &) = default;
  EventStream(EventStream &&) = default;
  EventStream &operator=(const EventStream &) = default;
  EventStream &operator=(EventStream &&) = default;
  virtual ~EventStream() = default;

  // Next yields the next event, or std::nullopt to end the loop.
  [[nodiscard]] virtual std::optional<Event> Next() = 0;
};

// Reactor receives every engine verdict and converts it into a side effect.
// This interface is where the trading application plugs in.
class Reactor {
public:
  Reactor() = default;
  Reactor(const Reactor &) = default;
  Reactor(Reactor &&) = default;
  Reactor &operator=(const Reactor &) = default;
  Reactor &operator=(Reactor &&) = default;
  virtual ~Reactor() = default;

  // OnAccepted fires once pre-trade has reserved and committed the order.
  // Production code calls venue.SendOrder(order) here.
  virtual void OnAccepted(const openpit::model::Order &order) = 0;

  // OnRejected fires when one or more policies refused the order. Inspect
  // rejects[i].code to choose between retry / throttle / escalate.
  virtual void
  OnRejected(const openpit::model::Order &order,
             const std::vector<openpit::pretrade::Reject> &rejects) = 0;

  // OnReport fires after the engine has consumed a venue execution report. When
  // result.accountBlocks is non-empty, the engine has permanently blocked this
  // account - your strategy must stop sending orders for it until operators
  // clear the state.
  virtual void OnReport(const openpit::model::ExecutionReport &report,
                        const openpit::PostTradeResult &result) = 0;
};

// Stats aggregates timing and trading outcomes over a run.
struct Stats {
  int accepted = 0;          // orders that passed pre-trade
  int rejected = 0;          // orders refused by a policy
  int preTradeCalls = 0;     // total pre-trade attempts
  int reports = 0;           // execution reports applied
  bool killSwitch = false;   // true once the kill switch ever tripped
  int killSwitchOnTrade = 0; // 1-based index of the tripping report, 0 if never
  openpit::param::Pnl pnl = openpit::param::Pnl::FromInt64(0); // cumulative P&L
  std::chrono::nanoseconds totalPreTrade{
      0}; // time spent inside ExecutePreTrade
  std::chrono::nanoseconds minPreTrade{0};
  std::chrono::nanoseconds maxPreTrade{0};

  // AvgPreTrade returns the mean ExecutePreTrade duration over the run.
  [[nodiscard]] std::chrono::nanoseconds AvgPreTrade() const {
    if (preTradeCalls == 0) {
      return std::chrono::nanoseconds{0};
    }
    return totalPreTrade / preTradeCalls;
  }
};

// =============================================================================
// Section 2 - Engine wiring.
// The two killswitch policies and the engine builder. Tune the limits to your
// risk tolerance.
// =============================================================================

// Limits gathers the killswitch parameters in one place so the call site reads
// like a risk-policy declaration.
struct Limits {
  openpit::param::Asset settlementAsset; // settlement asset, e.g. "USD"
  std::string pnlLowerBound;      // loss floor as a signed decimal, e.g. "-500"
  std::string pnlUpperBound;      // profit-taking ceiling, e.g. "500"
  std::size_t maxOrdersBurst = 0; // orders allowed inside the rate window
  std::chrono::nanoseconds rateWindow{0}; // length of the rate-limit window
};

// BuildEngine wires the engine with the two killswitch policies plus order
// validation. The combination answers a single question: "is my strategy
// trading too fast or losing too much?".
[[nodiscard]] inline openpit::Engine BuildEngine(const Limits &limits) {
  namespace policies = openpit::pretrade::policies;

  openpit::EngineBuilder builder(openpit::SyncPolicy::Full);
  // OrderValidation must be present so the engine refuses malformed orders
  // before the killswitch policies see them.
  builder.Add(policies::OrderValidationPolicy{});
  // PnL bounds halt the account permanently when realized P&L crosses either
  // edge of the corridor. Both bounds are optional - this example configures
  // both for completeness.
  policies::PnlBoundsBrokerBarrier pnlBarrier(limits.settlementAsset);
  pnlBarrier.lowerBound = openpit::param::Pnl::FromString(limits.pnlLowerBound);
  pnlBarrier.upperBound = openpit::param::Pnl::FromString(limits.pnlUpperBound);
  builder.Add(policies::PnlBoundsKillSwitchPolicy{}.BrokerBarrier(
      std::move(pnlBarrier)));
  // Rate limit catches a strategy stuck in a tight loop. The example uses the
  // broker (global) axis; see the Policies wiki page for per-asset and
  // per-account axes.
  builder.Add(policies::RateLimitPolicy{}.BrokerBarrier(
      policies::RateLimitBrokerBarrier(policies::RateLimit(
          limits.maxOrdersBurst,
          static_cast<std::uint64_t>(limits.rateWindow.count())))));
  return builder.Build();
}

// =============================================================================
// Section 3 - The engine loop.
// Run consumes the event stream, calls the engine, and notifies the reactor.
// This function is policy-agnostic - reuse it as-is in your code.
// =============================================================================

namespace detail {

// Exact P&L accumulation across the C boundary, matching the engine's decimal
// semantics. Inputs are short constants produced by the strategy/example, not
// untrusted external data.
inline void AddPnl(openpit::param::Pnl &accumulator,
                   const openpit::param::Pnl &delta) {
  OpenPitParamPnl out{};
  OpenPitParamError *error = nullptr;
  if (!openpit_param_pnl_checked_add(accumulator.Raw(), delta.Raw(), &out,
                                     &error)) {
    ::openpit::detail::ThrowFromParamError(
        error, "openpit_param_pnl_checked_add failed");
  }
  accumulator = openpit::param::Pnl::FromRaw(out);
}

inline void RunPreTrade(const openpit::Engine &engine,
                        const openpit::model::Order &order, Stats &stats,
                        Reactor &reactor) {
  const auto start = std::chrono::steady_clock::now();
  openpit::pretrade::ExecuteResult result = engine.ExecutePreTrade(order);
  const std::chrono::nanoseconds elapsed =
      std::chrono::steady_clock::now() - start;

  stats.preTradeCalls++;
  stats.totalPreTrade += elapsed;
  if (stats.preTradeCalls == 1 || elapsed < stats.minPreTrade) {
    stats.minPreTrade = elapsed;
  }
  if (elapsed > stats.maxPreTrade) {
    stats.maxPreTrade = elapsed;
  }

  if (!result.Passed()) {
    stats.rejected++;
    reactor.OnRejected(order, result.rejects);
    return;
  }
  // On accept, persist the reservation. Commit() finalizes the reserved state;
  // use Rollback() to release the reservation if you decide not to submit the
  // order.
  result.reservation->Commit();
  stats.accepted++;
  reactor.OnAccepted(order);
}

inline void RunReport(const openpit::Engine &engine, const ReportEvent &event,
                      Stats &stats, Reactor &reactor) {
  const openpit::PostTradeResult result =
      engine.ApplyExecutionReport(*event.report);
  stats.reports++;
  AddPnl(stats.pnl, event.realizedPnl);
  if (!result.accountBlocks.empty() && !stats.killSwitch) {
    stats.killSwitch = true;
    stats.killSwitchOnTrade = stats.reports;
  }
  reactor.OnReport(*event.report, result);
}

} // namespace detail

// Run drives the engine until the stream is exhausted. The engine is owned by
// the caller; Run does not stop it. Exceptions thrown here are infrastructure
// failures, not business rejects (those go to Reactor::OnRejected).
[[nodiscard]] inline Stats Run(const openpit::Engine &engine,
                               EventStream &stream, Reactor &reactor) {
  Stats stats;
  while (std::optional<Event> event = stream.Next()) {
    if (const auto *orderEvent = std::get_if<OrderEvent>(&*event)) {
      detail::RunPreTrade(engine, *orderEvent->order, stats, reactor);
    } else {
      detail::RunReport(engine, std::get<ReportEvent>(*event), stats, reactor);
    }
  }
  return stats;
}

// =============================================================================
// Section 4 - The scenario.
// A scripted feed that exercises the kill-switch policies. In your own
// application this is the place you delete entirely - your real strategy
// produces events.
// =============================================================================

// The burst overshoots the rate-limit ceiling by a few orders so the policy
// rejects the tail of the burst. The accepted orders then produce a stream of
// small-loss reports, and the final report contributes a large loss that pushes
// cumulative P&L past the lower bound and trips the kill switch on the last
// trade.
inline constexpr int kScenarioAttempts = 1'005;
inline constexpr std::size_t kScenarioMaxOrdersBurst = 1'000;
inline constexpr int kScenarioAcceptedReports = 1'000;
inline constexpr std::uint64_t kScenarioAccount = 99'224'416;
// 999 * (-0.05) + (-460) = -509.95 < -500 - the kill switch fires on the final
// report; every earlier report keeps cumulative P&L well inside the corridor
// (-49.95 at worst).
inline constexpr const char *kScenarioReportPnl = "-0.05";
inline constexpr const char *kScenarioFinalReportPnl = "-460";
inline constexpr const char *kScenarioLowerBound = "-500";
inline constexpr const char *kScenarioUpperBound = "500";
inline constexpr std::chrono::nanoseconds kScenarioRateWindow =
    std::chrono::seconds{10};
inline constexpr const char *kScenarioOrderPrice = "185";
inline constexpr const char *kScenarioOrderQty = "100";
inline constexpr const char *kScenarioAssetTraded = "AAPL";
inline constexpr const char *kScenarioAssetSettle = "USD";

// BuildOrder returns a buy-AAPL order intent. A real strategy assembles this
// from a signal and current market data.
[[nodiscard]] inline openpit::model::Order BuildOrder() {
  openpit::model::Order order;
  openpit::model::OrderOperation op;
  op.instrument =
      openpit::model::Instrument(openpit::param::Asset(kScenarioAssetTraded),
                                 openpit::param::Asset(kScenarioAssetSettle));
  op.accountId = ::openpit::param::AccountId::FromUint64(kScenarioAccount);
  op.side = openpit::model::Side::Buy;
  op.tradeAmount = openpit::model::TradeAmount::OfQuantity(
      openpit::param::Quantity::FromString(kScenarioOrderQty));
  op.price = openpit::param::Price::FromString(kScenarioOrderPrice);
  order.operation = std::move(op);
  return order;
}

// BuildReport returns a combined-mode execution report. "Combined" means the
// fee is embedded in pnl, so the fee field is set to zero; see the Policies
// wiki page for the alternative "separate" convention.
[[nodiscard]] inline openpit::model::ExecutionReport
BuildReport(const std::string &pnl) {
  openpit::model::ExecutionReport report;
  openpit::model::ExecutionReportOperation op;
  op.instrument =
      openpit::model::Instrument(openpit::param::Asset(kScenarioAssetTraded),
                                 openpit::param::Asset(kScenarioAssetSettle));
  op.accountId = ::openpit::param::AccountId::FromUint64(kScenarioAccount);
  op.side = openpit::model::Side::Buy;
  report.operation = std::move(op);
  openpit::model::FinancialImpact impact;
  impact.pnl = openpit::param::Pnl::FromString(pnl);
  impact.fee = openpit::param::Fee::FromString("0");
  report.financialImpact = std::move(impact);
  return report;
}

// ScenarioStream is the scripted feed expressed as a small state machine: three
// counters walked in order - order attempts, then small-loss reports, then one
// kill-switch report. Replace this implementation with a queue-driven stream
// that selects over your strategy and venue feeds.
class ScenarioStream final : public EventStream {
public:
  // The only place the scenario table is configured.
  ScenarioStream(const openpit::model::Order &order,
                 const openpit::model::ExecutionReport &smallReport,
                 const openpit::model::ExecutionReport &finalReport)
      : m_order(&order), m_smallReport(&smallReport),
        m_finalReport(&finalReport), m_attempts(kScenarioAttempts),
        m_small(kScenarioAcceptedReports - 1), m_final(1) {}

  // Returns the next scripted event.
  [[nodiscard]] std::optional<Event> Next() override {
    if (m_attempts > 0) {
      m_attempts--;
      return Event{OrderEvent{m_order}};
    }
    if (m_small > 0) {
      m_small--;
      return Event{ReportEvent{
          m_smallReport, openpit::param::Pnl::FromString(kScenarioReportPnl)}};
    }
    if (m_final > 0) {
      m_final--;
      return Event{ReportEvent{m_finalReport, openpit::param::Pnl::FromString(
                                                  kScenarioFinalReportPnl)}};
    }
    return std::nullopt;
  }

private:
  const openpit::model::Order *m_order;
  const openpit::model::ExecutionReport *m_smallReport;
  const openpit::model::ExecutionReport *m_finalReport;
  int m_attempts; // remaining order attempts
  int m_small;    // remaining small-loss reports
  int m_final;    // 1 until the final kill-switch report has been emitted
};

} // namespace killswitch
