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

#include "openpit/account_id.hpp"
#include "openpit/param.hpp"

#include <openpit.h>

#include <cstdint>
#include <optional>
#include <string>
#include <utility>
#include <variant>

// Domain data types: order and execution-report payloads plus the instrument
// identity they reference.
//
// Each payload mirrors the native runtime POD view (`OpenPitOrder`, ...).
// Optional groups and fields read as an empty `std::optional` when absent,
// matching the native runtime `is_set` / `NotSet` convention and the native
// SDK; no validation beyond what the core performs is added here. `FromRaw`
// copies a borrowed C view into owned values; `Raw()` rebuilds a C view whose
// string fields borrow this object's storage and is therefore valid only while
// the object is alive and unchanged. Financial fields are carried by the
// `openpit::param` value types, never by `double`.
//
// `openpit::Order` and `openpit::ExecutionReport` are the polymorphic bases the
// policy adapters (`openpit/adapters.hpp`) downcast to. The concrete payloads
// `openpit::model::Order` / `openpit::model::ExecutionReport` derive from them.

namespace openpit {

// Polymorphic base for an order payload handed to a pre-trade policy. Client
// order types derive from this so `openpit/adapters.hpp` can recover the
// concrete type via `dynamic_cast`.
//
// `EngineRaw()` yields the native runtime order view the engine consumes when
// this order is submitted (`Engine::StartPreTrade` / `ExecutePreTrade`). The
// `openpit::model::Order` override returns its own `Raw()`; a client type that
// derives from `openpit::Order` directly and embeds an `openpit::model::Order`
// overrides it to forward that member's `Raw()`. The returned view borrows
// storage owned by this object and is valid only while this object stays alive.
// The base default yields an empty order, so a type used only
// to exercise an adapter in isolation (never submitted) need not override it.
class Order {
 public:
  Order() = default;
  Order(const Order&) = default;
  Order(Order&&) = default;
  Order& operator=(const Order&) = default;
  Order& operator=(Order&&) = default;
  virtual ~Order() = default;

  [[nodiscard]] virtual OpenPitOrder EngineRaw() const noexcept {
    return OpenPitOrder{};
  }
};

// Polymorphic base for an execution-report payload applied by a policy. Client
// report types derive from this so `openpit/adapters.hpp` can recover the
// concrete type via `dynamic_cast`.
class ExecutionReport {
 public:
  ExecutionReport() = default;
  ExecutionReport(const ExecutionReport&) = default;
  ExecutionReport(ExecutionReport&&) = default;
  ExecutionReport& operator=(const ExecutionReport&) = default;
  ExecutionReport& operator=(ExecutionReport&&) = default;
  virtual ~ExecutionReport() = default;
};

namespace detail {

// Thread-local pointer to the polymorphic order currently being submitted on
// this thread.
//
// The pre-trade pipeline runs policy callbacks synchronously on the submitting
// thread while the original `openpit::Order` is alive, so the custom-policy
// trampolines can hand the policy the original object - preserving its dynamic
// type for `dynamic_cast` recovery - instead of a base view rebuilt from the C
// POD. `nullptr` when no submission is in flight (the trampolines then fall
// back to reconstructing the order from the C view).
[[nodiscard]] inline const Order*& CurrentSubmittedOrder() noexcept {
  static thread_local const Order* current = nullptr;
  return current;
}

// Scoped setter for `CurrentSubmittedOrder`: installs `order` for the duration
// of a pre-trade native runtime call and restores the prior value on scope
// exit, so nested or re-entrant submissions stay balanced.
class CurrentOrderGuard {
 public:
  explicit CurrentOrderGuard(const Order& order) noexcept
      : m_previous(CurrentSubmittedOrder()) {
    CurrentSubmittedOrder() = &order;
  }

  CurrentOrderGuard(const CurrentOrderGuard&) = delete;
  CurrentOrderGuard& operator=(const CurrentOrderGuard&) = delete;

  ~CurrentOrderGuard() { CurrentSubmittedOrder() = m_previous; }

 private:
  const Order* m_previous;
};

}  // namespace detail

}  // namespace openpit

namespace openpit::model {

//------------------------------------------------------------------------------
// Enums (each maps 1:1 from the native runtime; the `NotSet` variant is modeled
// as an empty std::optional, not a value).

// Buy/sell direction. Mirrors the set values of `OpenPitParamSide`.
enum class Side : std::uint8_t {
  Buy = OpenPitParamSide_Buy,
  Sell = OpenPitParamSide_Sell,
};

// Long/short exposure. Mirrors the set values of `OpenPitParamPositionSide`.
enum class PositionSide : std::uint8_t {
  Long = OpenPitParamPositionSide_Long,
  Short = OpenPitParamPositionSide_Short,
};

// Whether a trade opens or closes exposure. Mirrors the set values of
// `OpenPitParamPositionEffect`.
enum class PositionEffect : std::uint8_t {
  Open = OpenPitParamPositionEffect_Open,
  Close = OpenPitParamPositionEffect_Close,
};

// Position accounting mode. Mirrors the set values of
// `OpenPitParamPositionMode`.
enum class PositionMode : std::uint8_t {
  Netting = OpenPitParamPositionMode_Netting,
  Hedged = OpenPitParamPositionMode_Hedged,
};

// Selects how a trade-amount value is interpreted. Mirrors the set values of
// `OpenPitParamTradeAmountKind`.
enum class TradeAmountKind : std::uint8_t {
  Quantity = OpenPitParamTradeAmountKind_Quantity,
  Volume = OpenPitParamTradeAmountKind_Volume,
};

namespace detail {

// Maps a `*_NotSet`-or-value C enum byte to an optional set value.
template <typename Enum>
[[nodiscard]] inline std::optional<Enum> FromRawEnum(std::uint8_t raw,
                                                     std::uint8_t notSet) {
  if (raw == notSet) {
    return std::nullopt;
  }
  return static_cast<Enum>(raw);
}

template <typename Enum>
[[nodiscard]] inline std::uint8_t ToRawEnum(const std::optional<Enum>& value,
                                            std::uint8_t notSet) {
  return value ? static_cast<std::uint8_t>(*value) : notSet;
}

// Tri-state boolean: `NotSet` -> nullopt, `False`/`True` -> the bool.
[[nodiscard]] inline std::optional<bool> FromTriBool(OpenPitTriBool raw) {
  switch (raw) {
    case OpenPitTriBool_False:
      return false;
    case OpenPitTriBool_True:
      return true;
    default:
      return std::nullopt;
  }
}

[[nodiscard]] inline OpenPitTriBool ToTriBool(
    const std::optional<bool>& value) {
  if (!value) {
    return OpenPitTriBool_NotSet;
  }
  return *value ? OpenPitTriBool_True : OpenPitTriBool_False;
}

}  // namespace detail

//------------------------------------------------------------------------------
// Instrument

// Trading instrument identity: an `underlying`/`settlement` asset pair. Absent
// (empty optional from `FromRaw`) when neither asset is set; a single set asset
// is an invalid C payload and is reported by the core, not here.
struct Instrument {
  std::string underlyingAsset;
  std::string settlementAsset;

  Instrument() = default;

  Instrument(std::string underlying, std::string settlement)
      : underlyingAsset(std::move(underlying)),
        settlementAsset(std::move(settlement)) {}

  // Copies the borrowed asset views; nullopt when both views are unset.
  [[nodiscard]] static std::optional<Instrument> FromRaw(
      const OpenPitInstrument& raw) {
    const ::openpit::StringView underlying(raw.underlying_asset);
    const ::openpit::StringView settlement(raw.settlement_asset);
    if (underlying.Empty() && settlement.Empty()) {
      return std::nullopt;
    }
    return Instrument(underlying.ToString(), settlement.ToString());
  }

  // Borrows this object's asset bytes; valid only while it stays alive.
  [[nodiscard]] OpenPitInstrument Raw() const noexcept {
    OpenPitInstrument raw{};
    raw.underlying_asset = ::openpit::MakeStringView(underlyingAsset);
    raw.settlement_asset = ::openpit::MakeStringView(settlementAsset);
    return raw;
  }
};

//------------------------------------------------------------------------------
// TradeAmount

// A trade amount tagged by kind: either an instrument `Quantity` or a
// settlement `Volume`. The native runtime carries a raw decimal plus a kind
// byte; the kind selects which value type the decimal denotes.
class TradeAmount {
 public:
  [[nodiscard]] static TradeAmount OfQuantity(param::Quantity quantity) {
    return TradeAmount(quantity);
  }

  [[nodiscard]] static TradeAmount OfVolume(param::Volume volume) {
    return TradeAmount(volume);
  }

  // nullopt when the kind is `NotSet`.
  [[nodiscard]] static std::optional<TradeAmount> FromRaw(
      const OpenPitParamTradeAmount& raw) {
    switch (raw.kind) {
      case OpenPitParamTradeAmountKind_Quantity:
        return TradeAmount(
            param::Quantity::FromRaw(OpenPitParamQuantity{raw.value}));
      case OpenPitParamTradeAmountKind_Volume:
        return TradeAmount(
            param::Volume::FromRaw(OpenPitParamVolume{raw.value}));
      default:
        return std::nullopt;
    }
  }

  [[nodiscard]] OpenPitParamTradeAmount Raw() const noexcept {
    OpenPitParamTradeAmount raw{};
    if (const auto* quantity = std::get_if<param::Quantity>(&m_value)) {
      raw.value = quantity->Decimal();
      raw.kind = OpenPitParamTradeAmountKind_Quantity;
    } else {
      const auto& volume = std::get<param::Volume>(m_value);
      raw.value = volume.Decimal();
      raw.kind = OpenPitParamTradeAmountKind_Volume;
    }
    return raw;
  }

  [[nodiscard]] TradeAmountKind Kind() const noexcept {
    return std::holds_alternative<param::Quantity>(m_value)
               ? TradeAmountKind::Quantity
               : TradeAmountKind::Volume;
  }

  // The quantity value; present only when `Kind()` is `Quantity`.
  [[nodiscard]] std::optional<param::Quantity> AsQuantity() const {
    if (const auto* quantity = std::get_if<param::Quantity>(&m_value)) {
      return *quantity;
    }
    return std::nullopt;
  }

  // The volume value; present only when `Kind()` is `Volume`.
  [[nodiscard]] std::optional<param::Volume> AsVolume() const {
    if (const auto* volume = std::get_if<param::Volume>(&m_value)) {
      return *volume;
    }
    return std::nullopt;
  }

 private:
  explicit TradeAmount(param::Quantity quantity) : m_value(quantity) {}
  explicit TradeAmount(param::Volume volume) : m_value(volume) {}

  std::variant<param::Quantity, param::Volume> m_value;
};

//------------------------------------------------------------------------------
// Order sub-groups

// Optional operation group of an order: what is traded, at what price, on whose
// account, in which direction.
struct OrderOperation {
  std::optional<Instrument> instrument;
  std::optional<TradeAmount> tradeAmount;
  std::optional<param::Price> price;
  std::optional<param::AccountId> accountId;
  std::optional<Side> side;

  [[nodiscard]] static OrderOperation FromRaw(
      const OpenPitOrderOperation& raw) {
    OrderOperation out;
    out.instrument = Instrument::FromRaw(raw.instrument);
    out.tradeAmount = TradeAmount::FromRaw(raw.trade_amount);
    if (raw.price.is_set) {
      out.price = param::Price::FromRaw(raw.price.value);
    }
    if (raw.account_id.is_set) {
      out.accountId = param::AccountId::FromRaw(raw.account_id.value);
    }
    out.side = detail::FromRawEnum<Side>(raw.side, OpenPitParamSide_NotSet);
    return out;
  }

  [[nodiscard]] OpenPitOrderOperation Raw() const noexcept {
    OpenPitOrderOperation raw{};
    if (instrument) {
      raw.instrument = instrument->Raw();
    }
    if (tradeAmount) {
      raw.trade_amount = tradeAmount->Raw();
    }
    if (price) {
      raw.price.value = price->Raw();
      raw.price.is_set = true;
    }
    if (accountId) {
      raw.account_id.value = accountId->Raw();
      raw.account_id.is_set = true;
    }
    raw.side = detail::ToRawEnum(side, OpenPitParamSide_NotSet);
    return raw;
  }
};

// Optional position-management group of an order.
struct OrderPosition {
  std::optional<PositionSide> positionSide;
  std::optional<bool> reduceOnly;
  std::optional<bool> closePosition;

  [[nodiscard]] static OrderPosition FromRaw(const OpenPitOrderPosition& raw) {
    OrderPosition out;
    out.positionSide = detail::FromRawEnum<PositionSide>(
        raw.position_side, OpenPitParamPositionSide_NotSet);
    out.reduceOnly = detail::FromTriBool(raw.reduce_only);
    out.closePosition = detail::FromTriBool(raw.close_position);
    return out;
  }

  [[nodiscard]] OpenPitOrderPosition Raw() const noexcept {
    OpenPitOrderPosition raw{};
    raw.position_side =
        detail::ToRawEnum(positionSide, OpenPitParamPositionSide_NotSet);
    raw.reduce_only = detail::ToTriBool(reduceOnly);
    raw.close_position = detail::ToTriBool(closePosition);
    return raw;
  }
};

// Optional margin group of an order.
struct OrderMargin {
  std::optional<std::string> collateralAsset;
  std::optional<param::Leverage> leverage;
  std::optional<bool> autoBorrow;

  [[nodiscard]] static OrderMargin FromRaw(const OpenPitOrderMargin& raw) {
    OrderMargin out;
    const ::openpit::StringView collateral(raw.collateral_asset);
    if (!collateral.Empty()) {
      out.collateralAsset = collateral.ToString();
    }
    out.autoBorrow = detail::FromTriBool(raw.auto_borrow);
    out.leverage = param::Leverage::FromRawOption(raw.leverage);
    return out;
  }

  [[nodiscard]] OpenPitOrderMargin Raw() const noexcept {
    OpenPitOrderMargin raw{};
    if (collateralAsset) {
      raw.collateral_asset = ::openpit::MakeStringView(*collateralAsset);
    }
    raw.auto_borrow = detail::ToTriBool(autoBorrow);
    raw.leverage = param::Leverage::RawOption(leverage);
    return raw;
  }
};

//------------------------------------------------------------------------------
// Order

// Full order payload mirroring the native runtime `OpenPitOrder`. Every group
// is optional; `userData` is an opaque caller token the SDK never inspects
// (zero means unset). Derives from `openpit::Order` so it is usable wherever
// the policy adapters expect the polymorphic base.
class Order : public ::openpit::Order {
 public:
  std::optional<OrderOperation> operation;
  std::optional<OrderMargin> margin;
  std::optional<OrderPosition> position;
  std::uintptr_t userData = 0;

  Order() = default;

  /// \brief Builds a market order with the required operation fields set.
  [[nodiscard]] static Order Market(Instrument instrument,
                                    param::AccountId accountId, Side side,
                                    TradeAmount tradeAmount) {
    Order order;
    OrderOperation op;
    op.instrument = std::move(instrument);
    op.accountId = accountId;
    op.side = side;
    op.tradeAmount = tradeAmount;
    order.operation = std::move(op);
    return order;
  }

  /// \brief Builds a limit order with the required operation fields set.
  [[nodiscard]] static Order Limit(Instrument instrument,
                                   param::AccountId accountId, Side side,
                                   TradeAmount tradeAmount,
                                   param::Price price) {
    Order order = Market(std::move(instrument), accountId, side, tradeAmount);
    order.operation->price = price;
    return order;
  }

  [[nodiscard]] static Order FromRaw(const OpenPitOrder& raw) {
    Order out;
    if (raw.operation.is_set) {
      out.operation = OrderOperation::FromRaw(raw.operation.value);
    }
    if (raw.margin.is_set) {
      out.margin = OrderMargin::FromRaw(raw.margin.value);
    }
    if (raw.position.is_set) {
      out.position = OrderPosition::FromRaw(raw.position.value);
    }
    out.userData = reinterpret_cast<std::uintptr_t>(raw.user_data);
    return out;
  }

  // Borrows this object's string storage; valid only while it stays alive.
  [[nodiscard]] OpenPitOrder Raw() const noexcept {
    OpenPitOrder raw{};
    if (operation) {
      raw.operation.value = operation->Raw();
      raw.operation.is_set = true;
    }
    if (margin) {
      raw.margin.value = margin->Raw();
      raw.margin.is_set = true;
    }
    if (position) {
      raw.position.value = position->Raw();
      raw.position.is_set = true;
    }
    raw.user_data = reinterpret_cast<void*>(userData);
    return raw;
  }

  [[nodiscard]] OpenPitOrder EngineRaw() const noexcept override {
    return Raw();
  }
};

//------------------------------------------------------------------------------
// ExecutionReport sub-groups

// Operation-identification group of an execution report.
struct ExecutionReportOperation {
  std::optional<Instrument> instrument;
  std::optional<param::AccountId> accountId;
  std::optional<Side> side;

  [[nodiscard]] static ExecutionReportOperation FromRaw(
      const OpenPitExecutionReportOperation& raw) {
    ExecutionReportOperation out;
    out.instrument = Instrument::FromRaw(raw.instrument);
    if (raw.account_id.is_set) {
      out.accountId = param::AccountId::FromRaw(raw.account_id.value);
    }
    out.side = detail::FromRawEnum<Side>(raw.side, OpenPitParamSide_NotSet);
    return out;
  }

  [[nodiscard]] OpenPitExecutionReportOperation Raw() const noexcept {
    OpenPitExecutionReportOperation raw{};
    if (instrument) {
      raw.instrument = instrument->Raw();
    }
    if (accountId) {
      raw.account_id.value = accountId->Raw();
      raw.account_id.is_set = true;
    }
    raw.side = detail::ToRawEnum(side, OpenPitParamSide_NotSet);
    return raw;
  }
};

// Financial-impact group of an execution report: realized pnl and fee.
struct FinancialImpact {
  std::optional<param::Pnl> pnl;
  std::optional<param::Fee> fee;

  [[nodiscard]] static FinancialImpact FromRaw(
      const OpenPitFinancialImpact& raw) {
    FinancialImpact out;
    if (raw.pnl.is_set) {
      out.pnl = param::Pnl::FromRaw(raw.pnl.value);
    }
    if (raw.fee.is_set) {
      out.fee = param::Fee::FromRaw(raw.fee.value);
    }
    return out;
  }

  [[nodiscard]] OpenPitFinancialImpact Raw() const noexcept {
    OpenPitFinancialImpact raw{};
    if (pnl) {
      raw.pnl.value = pnl->Raw();
      raw.pnl.is_set = true;
    }
    if (fee) {
      raw.fee.value = fee->Raw();
      raw.fee.is_set = true;
    }
    return raw;
  }
};

// A single executed trade: price and quantity. Both are always present.
struct Trade {
  param::Price price;
  param::Quantity quantity;

  Trade(param::Price tradePrice, param::Quantity tradeQuantity)
      : price(tradePrice), quantity(tradeQuantity) {}

  [[nodiscard]] static Trade FromRaw(const OpenPitExecutionReportTrade& raw) {
    return Trade(param::Price::FromRaw(raw.price),
                 param::Quantity::FromRaw(raw.quantity));
  }

  [[nodiscard]] OpenPitExecutionReportTrade Raw() const noexcept {
    OpenPitExecutionReportTrade raw{};
    raw.price = price.Raw();
    raw.quantity = quantity.Raw();
    return raw;
  }
};

// Fill-details group of an execution report.
//
// The native runtime `lock` field (a borrowed pre-trade-lock pointer) is
// intentionally not surfaced here; pre-trade locks are a separate handle type
// owned by their own binding slice. A `Raw()` view leaves it null (no lock).
struct Fill {
  std::optional<Trade> lastTrade;
  // Structured fee amount and currency reported for this fill.
  std::optional<param::MonetaryAmount> fee;
  std::optional<param::Quantity> leavesQuantity;
  std::optional<bool> isFinal;

  [[nodiscard]] static Fill FromRaw(const OpenPitExecutionReportFill& raw) {
    Fill out;
    if (raw.last_trade.is_set) {
      out.lastTrade = Trade::FromRaw(raw.last_trade.value);
    }
    out.fee = param::MonetaryAmount::FromRawOption(raw.fee);
    if (raw.leaves_quantity.is_set) {
      out.leavesQuantity = param::Quantity::FromRaw(raw.leaves_quantity.value);
    }
    if (raw.is_final.is_set) {
      out.isFinal = raw.is_final.value;
    }
    return out;
  }

  [[nodiscard]] OpenPitExecutionReportFill Raw() const noexcept {
    OpenPitExecutionReportFill raw{};
    if (lastTrade) {
      raw.last_trade.value = lastTrade->Raw();
      raw.last_trade.is_set = true;
    }
    raw.fee = param::MonetaryAmount::RawOption(fee);
    if (leavesQuantity) {
      raw.leaves_quantity.value = leavesQuantity->Raw();
      raw.leaves_quantity.is_set = true;
    }
    raw.lock = nullptr;
    if (isFinal) {
      raw.is_final.value = *isFinal;
      raw.is_final.is_set = true;
    }
    return raw;
  }
};

// Position-impact group of an execution report.
struct PositionImpact {
  std::optional<PositionEffect> positionEffect;
  std::optional<PositionSide> positionSide;

  [[nodiscard]] static PositionImpact FromRaw(
      const OpenPitExecutionReportPositionImpact& raw) {
    PositionImpact out;
    out.positionEffect = detail::FromRawEnum<PositionEffect>(
        raw.position_effect, OpenPitParamPositionEffect_NotSet);
    out.positionSide = detail::FromRawEnum<PositionSide>(
        raw.position_side, OpenPitParamPositionSide_NotSet);
    return out;
  }

  [[nodiscard]] OpenPitExecutionReportPositionImpact Raw() const noexcept {
    OpenPitExecutionReportPositionImpact raw{};
    raw.position_effect =
        detail::ToRawEnum(positionEffect, OpenPitParamPositionEffect_NotSet);
    raw.position_side =
        detail::ToRawEnum(positionSide, OpenPitParamPositionSide_NotSet);
    return raw;
  }
};

//------------------------------------------------------------------------------
// ExecutionReport

// Full execution-report payload mirroring the native runtime
// `OpenPitExecutionReport`. Every group is optional; `userData` is an opaque
// caller token the SDK never inspects (zero means unset). Derives from
// `openpit::ExecutionReport` so it is usable wherever the policy adapters
// expect the polymorphic base.
class ExecutionReport : public ::openpit::ExecutionReport {
 public:
  std::optional<ExecutionReportOperation> operation;
  std::optional<FinancialImpact> financialImpact;
  std::optional<Fill> fill;
  std::optional<PositionImpact> positionImpact;
  std::uintptr_t userData = 0;

  ExecutionReport() = default;

  [[nodiscard]] static ExecutionReport FromRaw(
      const OpenPitExecutionReport& raw) {
    ExecutionReport out;
    if (raw.operation.is_set) {
      out.operation = ExecutionReportOperation::FromRaw(raw.operation.value);
    }
    if (raw.financial_impact.is_set) {
      out.financialImpact =
          FinancialImpact::FromRaw(raw.financial_impact.value);
    }
    if (raw.fill.is_set) {
      out.fill = Fill::FromRaw(raw.fill.value);
    }
    if (raw.position_impact.is_set) {
      out.positionImpact = PositionImpact::FromRaw(raw.position_impact.value);
    }
    out.userData = reinterpret_cast<std::uintptr_t>(raw.user_data);
    return out;
  }

  // Borrows this object's string storage; valid only while it stays alive. The
  // produced fill (if any) carries a null lock.
  [[nodiscard]] OpenPitExecutionReport Raw() const noexcept {
    OpenPitExecutionReport raw{};
    if (operation) {
      raw.operation.value = operation->Raw();
      raw.operation.is_set = true;
    }
    if (financialImpact) {
      raw.financial_impact.value = financialImpact->Raw();
      raw.financial_impact.is_set = true;
    }
    if (fill) {
      raw.fill.value = fill->Raw();
      raw.fill.is_set = true;
    }
    if (positionImpact) {
      raw.position_impact.value = positionImpact->Raw();
      raw.position_impact.is_set = true;
    }
    raw.user_data = reinterpret_cast<void*>(userData);
    return raw;
  }
};

}  // namespace openpit::model
