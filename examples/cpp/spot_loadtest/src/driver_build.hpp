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

#include "spot_loadtest/generator/event.hpp"

#include "openpit/account_id.hpp"
#include "openpit/accountadjustment/account_adjustment.hpp"
#include "openpit/accounts.hpp"
#include "openpit/asyncengine/typed.hpp"
#include "openpit/engine.hpp"
#include "openpit/model.hpp"
#include "openpit/param.hpp"
#include "openpit/pretrade/policies.hpp"
#include "openpit/pretrade/pre_trade_lock.hpp"
#include "openpit/reject.hpp"

#include <memory>
#include <optional>
#include <stdexcept>
#include <string>
#include <utility>
#include <vector>

// Internal driver build helpers.
//
// Event-to-engine object mapping plus the engine driver. The
// order/report/adjustment construction uses only public `openpit` value types.
//
// Lock-bearing settlement keeps the report and pre-trade lock together. The
// engine overload attaches the lock while applying the report, so the policy
// can reconcile a BUY fill against the reserved price.

namespace spot_loadtest::driver::detail {

namespace ae = ::openpit::asyncengine;

// Maps a generator side to a model side.
[[nodiscard]] inline ::openpit::model::Side SideOf(generator::Side s) {
  switch (s) {
  case generator::Side::Buy:
    return ::openpit::model::Side::Buy;
  case generator::Side::Sell:
    return ::openpit::model::Side::Sell;
  }
  throw std::runtime_error("driver: unknown side");
}

// Builds the (underlying, settlement) instrument identity.
[[nodiscard]] inline ::openpit::model::Instrument
InstrumentOf(const std::string &underlying, const std::string &settlement) {
  return ::openpit::model::Instrument(::openpit::param::Asset(underlying),
                                      ::openpit::param::Asset(settlement));
}

// Maps an OrderCheck event to a limit, quantity-denominated model::Order.
[[nodiscard]] inline ::openpit::model::Order
BuildOrder(const generator::Event &ev,
           ::openpit::param::AccountId &outAccount) {
  outAccount = ::openpit::param::AccountId::FromString(ev.account);
  ::openpit::model::Order order;
  ::openpit::model::OrderOperation op;
  op.instrument = InstrumentOf(ev.underlying, ev.settlement);
  op.accountId = outAccount;
  op.side = SideOf(ev.side);
  op.tradeAmount = ::openpit::model::TradeAmount::OfQuantity(
      ::openpit::param::Quantity::FromString(ev.quantity.ToString()));
  op.price = ::openpit::param::Price::FromString(ev.price.ToString());
  order.operation = std::move(op);
  return order;
}

// An execution-report payload paired with its pre-trade lock. The lock pins the
// reserved price under the default policy group so the spot-funds policy
// resolves a BUY fill's held leg.
//
// Move-only because it owns the `PreTradeLock` handle.
class ReportWithLock {
public:
  ReportWithLock(::openpit::model::ExecutionReport report,
                 ::openpit::pretrade::PreTradeLock lock)
      : m_report(std::move(report)), m_lock(std::move(lock)) {}

  ReportWithLock(ReportWithLock &&) noexcept = default;
  ReportWithLock &operator=(ReportWithLock &&) noexcept = default;
  ReportWithLock(const ReportWithLock &) = delete;
  ReportWithLock &operator=(const ReportWithLock &) = delete;

  [[nodiscard]] const ::openpit::model::ExecutionReport &
  Report() const noexcept {
    return m_report;
  }

  [[nodiscard]] const ::openpit::pretrade::PreTradeLock &Lock() const noexcept {
    return m_lock;
  }

private:
  ::openpit::model::ExecutionReport m_report;
  ::openpit::pretrade::PreTradeLock m_lock;
};

// Maps a Settlement event to a full-fill (leaves = 0, is_final = true) report
// plus the matching reserved-price lock under the default policy group.
[[nodiscard]] inline ReportWithLock
BuildReport(const generator::Event &ev,
            ::openpit::param::AccountId &outAccount) {
  outAccount = ::openpit::param::AccountId::FromString(ev.account);
  const ::openpit::param::Price price =
      ::openpit::param::Price::FromString(ev.price.ToString());
  const ::openpit::param::Quantity qty =
      ::openpit::param::Quantity::FromString(ev.quantity.ToString());
  const ::openpit::param::Quantity leaves =
      ::openpit::param::Quantity::FromString("0");
  const ::openpit::param::Fee fee = ::openpit::param::Fee::FromString("0");
  const ::openpit::param::Pnl pnl = ::openpit::param::Pnl::FromString("0");

  ::openpit::model::ExecutionReport report;
  ::openpit::model::ExecutionReportOperation op;
  op.instrument = InstrumentOf(ev.underlying, ev.settlement);
  op.accountId = outAccount;
  op.side = SideOf(ev.side);
  report.operation = std::move(op);

  ::openpit::model::FinancialImpact fin;
  fin.pnl = pnl;
  fin.fee = fee;
  report.financialImpact = std::move(fin);

  ::openpit::model::Fill fill;
  fill.lastTrade = ::openpit::model::Trade(price, qty);
  fill.leavesQuantity = leaves;
  fill.isFinal = true;
  report.fill = std::move(fill);

  // The fill lock ties the report back to the reservation the order committed:
  // one entry under the default policy group at the reserved price.
  ::openpit::pretrade::PreTradeLock lock;
  lock.Push(::openpit::param::DefaultPolicyGroupId, price);

  return ReportWithLock(std::move(report), std::move(lock));
}

// Maps a Funding event to a balance-operation adjustment on the funded asset's
// available leg (held is never touched), Absolute or Delta per the event's
// kind.
[[nodiscard]] inline ::openpit::accountadjustment::AccountAdjustment
BuildAdjustment(const generator::Event &ev,
                ::openpit::param::AccountId &outAccount) {
  outAccount = ::openpit::param::AccountId::FromString(ev.account);
  const ::openpit::param::PositionSize amount =
      ::openpit::param::PositionSize::FromString(ev.fundingAmount.ToString());

  ::openpit::param::AdjustmentAmount balance =
      ev.FundingIsDelta()
          ? ::openpit::param::AdjustmentAmount::OfDelta(amount)
          : ::openpit::param::AdjustmentAmount::OfAbsolute(amount);

  ::openpit::accountadjustment::BalanceOperation balanceOp;
  balanceOp.asset = ::openpit::param::Asset(ev.fundingAsset);

  ::openpit::accountadjustment::Amount amountGroup;
  amountGroup.balance = balance;

  ::openpit::accountadjustment::AccountAdjustment adj;
  adj.operation =
      ::openpit::accountadjustment::Operation::OfBalance(std::move(balanceOp));
  adj.amount = std::move(amountGroup);
  return adj;
}

// A zero-value adjustment on a probe account (the harness self-overhead probe).
[[nodiscard]] inline ::openpit::accountadjustment::AccountAdjustment
BuildProbeAdjustment() {
  const ::openpit::param::PositionSize zero =
      ::openpit::param::PositionSize::FromString("0");
  ::openpit::accountadjustment::BalanceOperation balanceOp;
  balanceOp.asset = ::openpit::param::Asset("USD");
  ::openpit::accountadjustment::Amount amountGroup;
  amountGroup.balance = ::openpit::param::AdjustmentAmount::OfDelta(zero);
  ::openpit::accountadjustment::AccountAdjustment adj;
  adj.operation =
      ::openpit::accountadjustment::Operation::OfBalance(std::move(balanceOp));
  adj.amount = std::move(amountGroup);
  return adj;
}

// An engine driver mirroring asyncengine::EngineAdapter but routing settlement
// through ReportWithLock so the lock reaches the policy. It satisfies the
// TypedAsyncEngine driver seam (the five members) used by the harness:
// ExecutePreTrade, ApplyExecutionReport, ApplyAccountAdjustment (plus the
// unused StartPreTrade / Accounts to complete the seam).
class LockingEngineAdapter {
public:
  explicit LockingEngineAdapter(const ::openpit::Engine &engine) noexcept
      : m_engine(&engine) {}

  [[nodiscard]] ::openpit::pretrade::StartResult
  StartPreTrade(const ::openpit::model::Order &order) const {
    return m_engine->StartPreTrade(order);
  }

  [[nodiscard]] ::openpit::pretrade::ExecuteResult
  ExecutePreTrade(const ::openpit::model::Order &order) const {
    return m_engine->ExecutePreTrade(order);
  }

  // Applies a report carrying a fill lock, so a BUY fill's held leg resolves.
  [[nodiscard]] ::openpit::PostTradeResult
  ApplyExecutionReport(const ReportWithLock &report) const {
    return m_engine->ApplyExecutionReport(report.Report(), report.Lock());
  }

  template <typename Adjustment>
  [[nodiscard]] ::openpit::AdjustmentResult
  ApplyAccountAdjustment(::openpit::param::AccountId accountId,
                         const std::vector<Adjustment> &adjustments) const {
    return m_engine->ApplyAccountAdjustment(accountId, adjustments);
  }

  [[nodiscard]] ::openpit::accounts::Accounts Accounts() const noexcept {
    return m_engine->Accounts();
  }

private:
  const ::openpit::Engine *m_engine;
};

} // namespace spot_loadtest::driver::detail
