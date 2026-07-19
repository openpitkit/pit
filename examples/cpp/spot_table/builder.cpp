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

#include "builder.hpp"

#include "openpit/account_id.hpp"
#include "openpit/error.hpp"

#include <utility>

namespace spot_table {

namespace param = openpit::param;
namespace model = openpit::model;
namespace adj = openpit::accountadjustment;

namespace {

[[nodiscard]] std::string ZeroIfEmpty(const std::string &s) {
  return s.empty() ? std::string("0") : s;
}

} // namespace

param::AccountId AccountIdOf(const std::string &s) {
  if (s.empty()) {
    throw BuildError("account is required");
  }
  try {
    return param::AccountId::FromString(s);
  } catch (const openpit::Error &err) {
    throw BuildError(err.what());
  }
}

param::AccountGroupId AccountGroupIdOf(const std::string &s) {
  if (s.empty()) {
    throw BuildError("group is required");
  }
  try {
    return param::AccountGroupId::FromString(s);
  } catch (const openpit::Error &err) {
    throw BuildError(err.what());
  }
}

model::Instrument ParseInstrument(const std::string &s) {
  // SplitInstrument throws FeedError on a malformed pair; surface it as a
  // BuildError so callers see one translation-error type.
  std::pair<std::string, std::string> parts;
  try {
    parts = SplitInstrument(s);
  } catch (const std::exception &err) {
    throw BuildError(err.what());
  }
  // The engine validates the asset codes; the binding takes them as-is.
  return model::Instrument(std::move(parts.first), std::move(parts.second));
}

model::Side ParseSide(const std::string &s) {
  if (s == "BUY") {
    return model::Side::Buy;
  }
  if (s == "SELL") {
    return model::Side::Sell;
  }
  throw BuildError("side must be BUY or SELL, got \"" + s + "\"");
}

adj::AccountAdjustment BuildSeedAdjustment(const Row &row) {
  param::PositionSize amount = param::PositionSize::FromString("0");
  try {
    amount = param::PositionSize::FromString(row.amount);
  } catch (const openpit::Error &err) {
    throw BuildError("amount \"" + row.amount + "\": " + err.what());
  }

  adj::BalanceOperation balanceOp;
  balanceOp.asset = row.asset; // The engine validates the asset code.

  adj::Amount amountGroup;
  amountGroup.balance = param::AdjustmentAmount::OfAbsolute(amount);

  adj::AccountAdjustment adjustment;
  adjustment.operation = adj::Operation::OfBalance(std::move(balanceOp));
  adjustment.amount = amountGroup;
  return adjustment;
}

namespace {

// Turns an ORDER row's qty or volume cell into a TradeAmount. Exactly one is
[[nodiscard]] model::TradeAmount BuildTradeAmount(const Row &row) {
  if (!row.volume.empty()) {
    try {
      return model::TradeAmount::OfVolume(
          param::Volume::FromString(row.volume));
    } catch (const openpit::Error &err) {
      throw BuildError("volume \"" + row.volume + "\": " + err.what());
    }
  }
  try {
    return model::TradeAmount::OfQuantity(param::Quantity::FromString(row.qty));
  } catch (const openpit::Error &err) {
    throw BuildError("qty \"" + row.qty + "\": " + err.what());
  }
}

} // namespace

model::Order BuildOrder(const Row &row, param::AccountId acc) {
  const model::Instrument inst = ParseInstrument(row.instrument);
  const model::Side side = ParseSide(row.side);
  const model::TradeAmount tradeAmount = BuildTradeAmount(row);

  model::OrderOperation op;
  op.instrument = inst;
  op.accountId = acc;
  op.side = side;
  op.tradeAmount = tradeAmount;
  if (!row.price.empty()) {
    try {
      op.price = param::Price::FromString(row.price);
    } catch (const openpit::Error &err) {
      throw BuildError("price \"" + row.price + "\": " + err.what());
    }
  }

  model::Order order;
  order.operation = std::move(op);
  return order;
}

FillReport BuildFillReport(const Row &row, param::AccountId acc,
                           const MarketFeed &feed) {
  const model::Instrument inst = ParseInstrument(row.instrument);
  const model::Side side = ParseSide(row.side);

  param::Quantity qty = param::Quantity::FromString("0");
  try {
    qty = param::Quantity::FromString(row.qty);
  } catch (const openpit::Error &err) {
    throw BuildError("qty \"" + row.qty + "\": " + err.what());
  }

  std::string priceStr = row.price;
  if (priceStr.empty()) {
    priceStr = feed.LatestPrice(row.instrument);
  }
  if (priceStr.empty()) {
    throw BuildError("FILL needs a price or a prior TICK for " +
                     row.instrument);
  }
  param::Price price = param::Price::FromString("0");
  try {
    price = param::Price::FromString(priceStr);
  } catch (const openpit::Error &err) {
    throw BuildError("price \"" + priceStr + "\": " + err.what());
  }

  param::Fee fee = param::Fee::FromString("0");
  try {
    fee = param::Fee::FromString(ZeroIfEmpty(row.fee));
  } catch (const openpit::Error &err) {
    throw BuildError("fee \"" + row.fee + "\": " + err.what());
  }

  param::Pnl pnl = param::Pnl::FromString("0");
  try {
    pnl = param::Pnl::FromString(ZeroIfEmpty(row.pnl));
  } catch (const openpit::Error &err) {
    throw BuildError("pnl \"" + row.pnl + "\": " + err.what());
  }

  const param::Quantity leaves = param::Quantity::FromString("0");

  // The fill carries the pre-trade lock that ties it back to the reservation
  // the matching ORDER committed: one entry under the default policy group.
  openpit::pretrade::PreTradeLock lock;
  try {
    lock.Push(openpit::param::DefaultPolicyGroupId, price);
  } catch (const openpit::Error &err) {
    throw BuildError(std::string("build fill lock: ") + err.what());
  }

  model::ExecutionReportOperation op;
  op.instrument = inst;
  op.accountId = acc;
  op.side = side;

  model::FinancialImpact impact;
  impact.pnl = pnl;
  impact.fee = fee;

  model::Fill fill;
  fill.lastTrade = model::Trade(price, qty);
  fill.leavesQuantity = leaves;
  fill.isFinal = true;

  model::ExecutionReport report;
  report.operation = std::move(op);
  report.financialImpact = impact;
  report.fill = fill;

  return FillReport(std::move(report), std::move(lock));
}

} // namespace spot_table
