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

#pragma once

#include "openpit/accountadjustment/account_adjustment.hpp"
#include "openpit/engine.hpp"
#include "openpit/error.hpp"
#include "openpit/model/model.hpp"
#include "openpit/param/account_id.hpp"
#include "openpit/param/param.hpp"
#include "openpit/pretrade/decision.hpp"
#include "openpit/pretrade/pretrade.hpp"

#include <cstddef>
#include <cstdint>
#include <memory>
#include <optional>
#include <string>
#include <vector>

// Shared helpers for the spot_funds example. main() and the gtest smoke test
// both call these; each wraps one engine interaction so the linear story in
// main() stays readable.

namespace spot_funds {

//------------------------------------------------------------------------------
// Scenario constants. The numbers are picked so the reservation is the whole
// point: two identical 60000-notional buys do not both fit inside a 100000
// balance, because the first one's funds stay held until it fills.

// Same account as the rate_pnl_killswitch example.
inline constexpr std::uint64_t kScenarioAccount = 99'224'416;
// Underlying asset.
inline constexpr const char *kScenarioAssetTraded = "AAPL";
// Settlement asset whose funds are reserved.
inline constexpr const char *kScenarioAssetSettle = "USD";
// Initial available USD.
inline constexpr const char *kScenarioSeedFunds = "100000";
// Limit price; also the lock/reservation price.
inline constexpr const char *kScenarioOrderPrice = "2000";
// Each buy is 30 * 2000 = 60000 USD notional.
inline constexpr const char *kScenarioOrderQty = "30";

// Derived amounts used only in the narration. One buy's notional (qty * price)
// and what stays available once the first buy's funds are held - the fill
// spends the held amount, so that figure still stands after settlement.
inline constexpr int kOrderNotional = 60'000;      // qty * price
inline constexpr int kAvailableAfterBuy1 = 40'000; // seed - notional

//------------------------------------------------------------------------------
// Outcome of placing an order: either a committed reservation's lock (the order
// was accepted) or the rejects (the order was rejected). Exactly one channel
// is populated.

struct PlaceResult {
  std::optional<::openpit::pretrade::PreTradeLock> lock;
  std::vector<::openpit::reject::Reject> rejects;

  [[nodiscard]] bool Accepted() const noexcept { return lock.has_value(); }
};

// Outcome of applying a fill: the account blocks a policy produced. Empty when
// settlement succeeded.
struct FillResult {
  std::vector<::openpit::accounts::AccountBlock> accountBlocks;
};

//------------------------------------------------------------------------------
// BuildEngine wires a limit-only engine with the SpotFunds policy.
// OrderValidation is registered first so the engine refuses malformed orders
// before SpotFunds sees them. SpotFunds is not given WithMarketOrders, so
// market orders (no limit price) are rejected with UnsupportedOrderType - this
// example only sends limit orders.
[[nodiscard]] inline ::openpit::Engine BuildEngine() {
  ::openpit::EngineBuilder builder(::openpit::SyncPolicy::Full);
  builder.Add(::openpit::pretrade::policies::OrderValidationPolicy{});
  builder.Add(::openpit::pretrade::policies::SpotFundsPolicy{});
  return builder.Build();
}

// SeedFunds sets the account's available settlement balance to an absolute
// amount. An absolute adjustment overwrites the balance (unlike a relative
// delta), so it reads as "set available USD to funds". SpotFunds has no
// initial-balance builder option; the balance is established through the
// account-adjustment pipeline, exactly as a deposit would be.
inline void SeedFunds(const ::openpit::Engine &engine,
                      ::openpit::param::AccountId account,
                      const std::string &funds) {
  namespace adj = ::openpit::accountadjustment;

  adj::BalanceOperation balance;
  balance.asset = ::openpit::param::Asset(kScenarioAssetSettle);

  adj::Amount amount;
  amount.balance = ::openpit::param::AdjustmentAmount::Absolute(
      ::openpit::param::PositionSize::FromString(funds));

  adj::AccountAdjustment adjustment;
  adjustment.operation = adj::Operation::OfBalance(std::move(balance));
  adjustment.amount = std::move(amount);

  std::vector<adj::AccountAdjustment> batch;
  batch.push_back(std::move(adjustment));

  const ::openpit::AdjustmentResult result =
      engine.ApplyAccountAdjustment(account, batch);
  if (!result.Passed()) {
    throw ::openpit::Error("seed adjustment rejected");
  }
}

// BuildOrder assembles a BUY limit order for the scenario instrument. A real
// strategy builds this from a signal and current market data.
[[nodiscard]] inline ::openpit::model::Order
BuildOrder(::openpit::param::AccountId account) {
  ::openpit::model::OrderOperation op;
  op.instrument = ::openpit::model::Instrument(
      ::openpit::param::Asset(kScenarioAssetTraded),
      ::openpit::param::Asset(kScenarioAssetSettle));
  op.accountId = account;
  op.side = ::openpit::model::Side::Buy;
  op.tradeAmount = ::openpit::model::TradeAmount::OfQuantity(
      ::openpit::param::Quantity::FromString(kScenarioOrderQty));
  op.price = ::openpit::param::Price::FromString(kScenarioOrderPrice);

  ::openpit::model::Order order;
  order.operation = std::move(op);
  return order;
}

// PlaceOrder runs the pre-trade check for an order and, on accept, commits the
// reservation. It returns that reservation's pre-trade lock - an owned snapshot
// detached from the reservation - so the caller can later attach it to the
// matching fill; on reject it returns no lock and the rejects.
[[nodiscard]] inline PlaceResult
PlaceOrder(const ::openpit::Engine &engine,
           const ::openpit::model::Order &order) {
  ::openpit::pretrade::ExecuteResult result = engine.ExecutePreTrade(order);
  PlaceResult out;
  if (!result.Passed()) {
    // A rejected order reserves nothing; there is no lock and nothing to
    // commit.
    out.rejects = std::move(result.rejects);
    return out;
  }

  // Read the lock the engine assigned to this reservation, then commit. The
  // snapshot is detached, so it outlives the reservation it came from. Commit
  // moves the reserved settlement funds from available to held; Rollback would
  // release them instead.
  out.lock = result.reservation->Lock();
  result.reservation->Commit();
  return out;
}

// BuildFillReport assembles a full, final execution report for a buy order.
// Carrying the pre-trade lock is what ties the fill back to the reservation:
// SpotFunds reads the lock to find which held funds to settle. This helper
// leaves model::Fill::lock unset; ApplyFill sets it on its own copy of the
// report, so a report can be built before its reservation's lock is known.
[[nodiscard]] inline ::openpit::model::ExecutionReport
BuildFillReport(::openpit::param::AccountId account) {
  ::openpit::model::ExecutionReportOperation op;
  op.instrument = ::openpit::model::Instrument(
      ::openpit::param::Asset(kScenarioAssetTraded),
      ::openpit::param::Asset(kScenarioAssetSettle));
  op.accountId = account;
  op.side = ::openpit::model::Side::Buy;

  // Combined-mode impact: the fee is embedded in pnl, so both are zero for a
  // plain settlement. See the SpotFunds wiki page for the "separate" fee
  // convention.
  ::openpit::model::FinancialImpact impact;
  impact.pnl = ::openpit::param::Pnl::FromString("0");
  impact.fee = ::openpit::param::Fee::FromString("0");

  ::openpit::model::Fill fill;
  fill.lastTrade = ::openpit::model::Trade(
      ::openpit::param::Price::FromString(kScenarioOrderPrice),
      ::openpit::param::Quantity::FromString(kScenarioOrderQty));
  // A full fill of a 30-lot order leaves nothing outstanding.
  fill.leavesQuantity = ::openpit::param::Quantity::FromString("0");
  fill.isFinal = true;

  ::openpit::model::ExecutionReport report;
  report.operation = std::move(op);
  report.financialImpact = std::move(impact);
  report.fill = std::move(fill);
  return report;
}

// ApplyFill feeds a completed execution report to the engine, carrying the
// pre-trade lock captured when the order's reservation was committed so
// SpotFunds matches the fill to that reservation and settles the held amount.
[[nodiscard]] inline FillResult
ApplyFill(const ::openpit::Engine &engine,
          const ::openpit::model::ExecutionReport &report,
          const ::openpit::pretrade::PreTradeLock &lock) {
  ::openpit::model::ExecutionReport reportWithLock = report;
  reportWithLock.fill->lock = lock.Clone();
  ::openpit::PostTradeResult result =
      engine.ApplyExecutionReport(reportWithLock);
  FillResult out;
  out.accountBlocks = std::move(result.accountBlocks);
  return out;
}

// EnableTrackOnly switches the SpotFunds policy to global track-only mode
// without rebuilding the engine. TrackOnly drops the insufficient-funds gate:
// the pre-trade check always records a reservation, so holdings keep being
// tracked and available funds are allowed to go negative. Failures - an unknown
// policy name above all - surface as a thrown openpit::Error.
inline void EnableTrackOnly(const ::openpit::Engine &engine) {
  namespace policies = ::openpit::pretrade::policies;
  engine.Configure().SpotFundsGlobalLimitMode(
      policies::SpotFundsPolicyName, policies::SpotFundsLimitMode::TrackOnly);
}

//------------------------------------------------------------------------------
// ContainsCode reports whether the rejects include the given business code.
[[nodiscard]] inline bool
ContainsCode(const std::vector<::openpit::reject::Reject> &rejects,
             ::openpit::reject::RejectCode want) {
  for (const ::openpit::reject::Reject &reject : rejects) {
    if (reject.code == want) {
      return true;
    }
  }
  return false;
}

// Describe renders rejects as "reason (details)" pairs for a one-line message.
[[nodiscard]] inline std::string
Describe(const std::vector<::openpit::reject::Reject> &rejects) {
  if (rejects.empty()) {
    return "no rejects";
  }
  std::string out;
  for (std::size_t i = 0; i < rejects.size(); ++i) {
    if (i != 0) {
      out += "; ";
    }
    out += rejects[i].reason + " (" + rejects[i].details + ")";
  }
  return out;
}

} // namespace spot_funds
