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

#include "openpit/detail/native_access.hpp"
#include "openpit/param/account_id.hpp"
#include "openpit/param/param.hpp"
#include "openpit/pretrade/pre_trade_lock.hpp"

#include <openpit.h>

#include <cstdint>
#include <optional>
#include <string>
#include <utility>
#include <variant>

// Domain data types: order and execution-report payloads plus the instrument
// identity they reference.
//
// Optional groups and fields read as an empty `std::optional` when absent.
// Asset construction delegates validation to the SDK core. Financial fields
// are carried by the `openpit::param` value types, never by `double`.
//
// `openpit::Order` and `openpit::ExecutionReport` are the polymorphic bases the
// policy adapters (`openpit/pretrade/adapters.hpp`) downcast to. The concrete
// payloads
// `openpit::model::Order` / `openpit::model::ExecutionReport` derive from them.

namespace openpit::detail {

using RawOrder = ::OpenPitOrder;
using RawExecutionReport = ::OpenPitExecutionReport;

}  // namespace openpit::detail

namespace openpit {

namespace model {
class Order;
class ExecutionReport;
}  // namespace model

// Internal polymorphic seam for pre-trade order dispatch. Clients use
// `model::Order`, not this base: the engine uses it only for `dynamic_cast`,
// and its private `NativeView()` crosses the internal C ABI.
class Order {
 public:
  virtual ~Order() = default;

 private:
  friend class detail::NativeAccess;
  friend class model::Order;

  Order() = default;
  Order(const Order&) = default;
  Order(Order&&) = default;
  Order& operator=(const Order&) = default;
  Order& operator=(Order&&) = default;

  [[nodiscard]] detail::RawOrder Native() const noexcept {
    return NativeView();
  }

  [[nodiscard]] virtual detail::RawOrder NativeView() const noexcept = 0;
};

// Internal polymorphic seam for post-trade report dispatch. Clients use
// `model::ExecutionReport`, not this base: the engine uses it only for
// `dynamic_cast`, and its private `NativeView()` crosses the internal C ABI.
class ExecutionReport {
 public:
  virtual ~ExecutionReport() = default;

 private:
  friend class detail::NativeAccess;
  friend class model::ExecutionReport;

  ExecutionReport() = default;
  ExecutionReport(const ExecutionReport&) = default;
  ExecutionReport(ExecutionReport&&) = default;
  ExecutionReport& operator=(const ExecutionReport&) = default;
  ExecutionReport& operator=(ExecutionReport&&) = default;

  [[nodiscard]] detail::RawExecutionReport Native() const {
    return NativeView();
  }

  [[nodiscard]] virtual detail::RawExecutionReport NativeView() const = 0;
};

namespace detail {

// Thread-local pointer to the polymorphic order currently being submitted on
// this thread.
//
// The pre-trade pipeline runs policy callbacks synchronously on the invoking
// thread while the exact submitted `openpit::Order` is alive. In a deferred
// flow the request owns that object and installs the guard before the main
// stage, so the custom policy trampolines can preserve its dynamic type for
// `dynamic_cast` recovery instead of rebuilding only the base C POD view.
// `nullptr` when no submission is in flight (the trampolines then reconstruct
// the order from the C view).
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

// Thread-local report counterpart to `CurrentSubmittedOrder()`. It lets a
// synchronous post-trade callback recover the client report's dynamic type.
[[nodiscard]] inline const ExecutionReport*& CurrentSubmittedReport() noexcept {
  static thread_local const ExecutionReport* current = nullptr;
  return current;
}

class CurrentReportGuard {
 public:
  explicit CurrentReportGuard(const ExecutionReport& report) noexcept
      : m_previous(CurrentSubmittedReport()) {
    CurrentSubmittedReport() = &report;
  }

  CurrentReportGuard(const CurrentReportGuard&) = delete;
  CurrentReportGuard& operator=(const CurrentReportGuard&) = delete;

  ~CurrentReportGuard() { CurrentSubmittedReport() = m_previous; }

 private:
  const ExecutionReport* m_previous;
};

}  // namespace detail

}  // namespace openpit

namespace openpit::model {

//------------------------------------------------------------------------------
// Enums. An unset value is modeled as an empty `std::optional`.

// Buy/sell direction shared with the exact-value parameter module.
using Side = ::openpit::param::Side;

// Long/short exposure.
enum class PositionSide : std::uint8_t {
  Long = 1,
  Short = 2,
};

// Whether a trade opens or closes exposure.
enum class PositionEffect : std::uint8_t {
  Open = 1,
  Close = 2,
};

// Position accounting mode.
enum class PositionMode : std::uint8_t {
  Netting = 1,
  Hedged = 2,
};

// Selects how a trade-amount value is interpreted.
enum class TradeAmountKind : std::uint8_t {
  Quantity = 1,
  Volume = 2,
};

using ::openpit::param::ToString;

namespace detail {

// Maps a `*_NotSet`-or-value C enum byte to an optional set value.
template <typename Enum>
[[nodiscard]] inline std::optional<Enum> FromRawEnum(
    std::uint8_t raw, std::uint8_t notSet) noexcept {
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
  if (raw == OPENPIT_TRI_BOOL_NOT_SET) {
    return std::nullopt;
  }
  return raw == OPENPIT_TRI_BOOL_TRUE;
}

[[nodiscard]] inline OpenPitTriBool ToTriBool(
    const std::optional<bool>& value) {
  if (!value) {
    return OPENPIT_TRI_BOOL_NOT_SET;
  }
  return *value ? OPENPIT_TRI_BOOL_TRUE : OPENPIT_TRI_BOOL_FALSE;
}

}  // namespace detail

/// Formats a long/short exposure.
[[nodiscard]] inline std::string ToString(PositionSide side) {
  return ::openpit::detail::StringifyNative(
      static_cast<::OpenPitParamPositionSide>(side),
      ::openpit_param_position_side_to_string,
      "position side string conversion failed");
}

/// Formats an open/close position effect.
[[nodiscard]] inline std::string ToString(PositionEffect effect) {
  return ::openpit::detail::StringifyNative(
      static_cast<::OpenPitParamPositionEffect>(effect),
      ::openpit_param_position_effect_to_string,
      "position effect string conversion failed");
}

/// Formats a position accounting mode.
[[nodiscard]] inline std::string ToString(PositionMode mode) {
  return ::openpit::detail::StringifyNative(
      static_cast<::OpenPitParamPositionMode>(mode),
      ::openpit_param_position_mode_to_string,
      "position mode string conversion failed");
}

//------------------------------------------------------------------------------
// Instrument

// Trading instrument identity: an `underlying`/`settlement` asset pair. Absent
// when neither asset is set. A partial instrument is rejected by the core.
struct Instrument {
  param::Asset underlyingAsset;
  param::Asset settlementAsset;

  Instrument(param::Asset underlying, param::Asset settlement)
      : underlyingAsset(std::move(underlying)),
        settlementAsset(std::move(settlement)) {}

 private:
  friend class ::openpit::detail::NativeAccess;

  [[nodiscard]] static std::optional<Instrument> FromRaw(
      const OpenPitInstrument& raw) {
    const auto underlying =
        ::openpit::detail::FromNative<::openpit::StringView>(
            raw.underlying_asset);
    const auto settlement =
        ::openpit::detail::FromNative<::openpit::StringView>(
            raw.settlement_asset);
    if (underlying.Empty() && settlement.Empty()) {
      return std::nullopt;
    }
    return Instrument(
        ::openpit::detail::FromNative<param::Asset>(raw.underlying_asset),
        ::openpit::detail::FromNative<param::Asset>(raw.settlement_asset));
  }

  // Borrows this object's asset bytes; valid only while it stays alive.
  [[nodiscard]] OpenPitInstrument Native() const noexcept {
    OpenPitInstrument raw{};
    raw.underlying_asset = ::openpit::detail::Native(underlyingAsset);
    raw.settlement_asset = ::openpit::detail::Native(settlementAsset);
    return raw;
  }
};

//------------------------------------------------------------------------------
// TradeAmount

// A trade amount tagged by kind: either an instrument `Quantity` or a
// settlement `Volume`.
class TradeAmount {
 public:
  [[nodiscard]] static TradeAmount OfQuantity(param::Quantity quantity) {
    return TradeAmount(quantity);
  }

  [[nodiscard]] static TradeAmount OfVolume(param::Volume volume) {
    return TradeAmount(volume);
  }

  /// Formats the selected exact amount.
  [[nodiscard]] std::string ToString() const {
    return ::openpit::detail::StringifyNative(
        Native(), ::openpit_param_trade_amount_to_string,
        "trade amount string conversion failed");
  }

 private:
  friend class ::openpit::detail::NativeAccess;

  [[nodiscard]] static std::optional<TradeAmount> FromRaw(
      const OpenPitParamTradeAmount& raw) {
    switch (raw.kind) {
      case OPENPIT_PARAM_TRADE_AMOUNT_KIND_QUANTITY:
        return TradeAmount(::openpit::detail::FromNative<param::Quantity>(
            OpenPitParamQuantity{raw.value}));
      case OPENPIT_PARAM_TRADE_AMOUNT_KIND_VOLUME:
        return TradeAmount(::openpit::detail::FromNative<param::Volume>(
            OpenPitParamVolume{raw.value}));
      case OPENPIT_PARAM_TRADE_AMOUNT_KIND_NOT_SET:
      default:
        return std::nullopt;
    }
  }

  [[nodiscard]] OpenPitParamTradeAmount Native() const noexcept {
    OpenPitParamTradeAmount raw{};
    if (const auto* quantity = std::get_if<param::Quantity>(&m_value)) {
      raw.value = ::openpit::detail::Native(*quantity)._0;
      raw.kind = OPENPIT_PARAM_TRADE_AMOUNT_KIND_QUANTITY;
    } else if (const auto* volume = std::get_if<param::Volume>(&m_value)) {
      raw.value = ::openpit::detail::Native(*volume)._0;
      raw.kind = OPENPIT_PARAM_TRADE_AMOUNT_KIND_VOLUME;
    }
    return raw;
  }

 public:
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

 private:
  friend class ::openpit::detail::NativeAccess;

  [[nodiscard]] static OrderOperation FromRaw(
      const OpenPitOrderOperation& raw) {
    OrderOperation out;
    out.instrument = ::openpit::detail::FromNative<Instrument>(raw.instrument);
    out.tradeAmount =
        ::openpit::detail::FromNative<TradeAmount>(raw.trade_amount);
    if (raw.price.is_set) {
      out.price = ::openpit::detail::FromNative<param::Price>(raw.price.value);
    }
    if (raw.account_id.is_set) {
      out.accountId =
          ::openpit::detail::FromNative<param::AccountId>(raw.account_id.value);
    }
    out.side = detail::FromRawEnum<Side>(raw.side, OPENPIT_PARAM_SIDE_NOT_SET);
    return out;
  }

  [[nodiscard]] OpenPitOrderOperation Native() const noexcept {
    OpenPitOrderOperation raw{};
    if (instrument) {
      raw.instrument = ::openpit::detail::Native(*instrument);
    }
    if (tradeAmount) {
      raw.trade_amount = ::openpit::detail::Native(*tradeAmount);
    }
    if (price) {
      raw.price.value = ::openpit::detail::Native(*price);
      raw.price.is_set = true;
    }
    if (accountId) {
      raw.account_id.value = ::openpit::detail::Native(*accountId);
      raw.account_id.is_set = true;
    }
    raw.side = detail::ToRawEnum(side, OPENPIT_PARAM_SIDE_NOT_SET);
    return raw;
  }
};

// Optional position-management group of an order.
struct OrderPosition {
  std::optional<PositionSide> positionSide;
  std::optional<bool> reduceOnly;
  std::optional<bool> closePosition;

 private:
  friend class ::openpit::detail::NativeAccess;

  [[nodiscard]] static OrderPosition FromRaw(const OpenPitOrderPosition& raw) {
    OrderPosition out;
    out.positionSide = detail::FromRawEnum<PositionSide>(
        raw.position_side, OPENPIT_PARAM_POSITION_SIDE_NOT_SET);
    out.reduceOnly = detail::FromTriBool(raw.reduce_only);
    out.closePosition = detail::FromTriBool(raw.close_position);
    return out;
  }

  [[nodiscard]] OpenPitOrderPosition Native() const noexcept {
    OpenPitOrderPosition raw{};
    raw.position_side =
        detail::ToRawEnum(positionSide, OPENPIT_PARAM_POSITION_SIDE_NOT_SET);
    raw.reduce_only = detail::ToTriBool(reduceOnly);
    raw.close_position = detail::ToTriBool(closePosition);
    return raw;
  }
};

// Optional margin group of an order.
struct OrderMargin {
  std::optional<param::Asset> collateralAsset;
  std::optional<param::Leverage> leverage;
  std::optional<bool> autoBorrow;

 private:
  friend class ::openpit::detail::NativeAccess;

  [[nodiscard]] static OrderMargin FromRaw(const OpenPitOrderMargin& raw) {
    OrderMargin out;
    const auto collateral =
        ::openpit::detail::FromNative<::openpit::StringView>(
            raw.collateral_asset);
    if (!collateral.Empty()) {
      out.collateralAsset =
          ::openpit::detail::FromNative<param::Asset>(raw.collateral_asset);
    }
    out.autoBorrow = detail::FromTriBool(raw.auto_borrow);
    out.leverage = param::detail::LeverageAccess::FromNative(raw.leverage);
    return out;
  }

  [[nodiscard]] OpenPitOrderMargin Native() const noexcept {
    OpenPitOrderMargin raw{};
    if (collateralAsset) {
      raw.collateral_asset = ::openpit::detail::Native(*collateralAsset);
    }
    raw.auto_borrow = detail::ToTriBool(autoBorrow);
    raw.leverage = param::detail::LeverageAccess::Native(leverage);
    return raw;
  }
};

//------------------------------------------------------------------------------
// Order

// Full order payload. Every group is optional; `userData` is an opaque caller
// token the SDK never inspects (zero means unset). Derives from
// `openpit::Order` so it is usable wherever the policy adapters expect the
// polymorphic base.
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

 private:
  friend class ::openpit::detail::NativeAccess;

  [[nodiscard]] static Order FromRaw(const OpenPitOrder& raw) {
    Order out;
    if (raw.operation.is_set) {
      out.operation =
          ::openpit::detail::FromNative<OrderOperation>(raw.operation.value);
    }
    if (raw.margin.is_set) {
      out.margin = ::openpit::detail::FromNative<OrderMargin>(raw.margin.value);
    }
    if (raw.position.is_set) {
      out.position =
          ::openpit::detail::FromNative<OrderPosition>(raw.position.value);
    }
    out.userData = reinterpret_cast<std::uintptr_t>(raw.user_data);
    return out;
  }

  // Borrows this object's string storage; valid only while it stays alive.
  // Routing model fields and engine native view must not diverge.
  [[nodiscard]] OpenPitOrder NativeView() const noexcept final {
    OpenPitOrder raw{};
    if (operation) {
      raw.operation.value = ::openpit::detail::Native(*operation);
      raw.operation.is_set = true;
    }
    if (margin) {
      raw.margin.value = ::openpit::detail::Native(*margin);
      raw.margin.is_set = true;
    }
    if (position) {
      raw.position.value = ::openpit::detail::Native(*position);
      raw.position.is_set = true;
    }
    raw.user_data = reinterpret_cast<void*>(userData);
    return raw;
  }
};

//------------------------------------------------------------------------------
// ExecutionReport sub-groups

// Operation-identification group of an execution report.
struct ExecutionReportOperation {
  std::optional<Instrument> instrument;
  std::optional<param::AccountId> accountId;
  std::optional<Side> side;

 private:
  friend class ::openpit::detail::NativeAccess;

  [[nodiscard]] static ExecutionReportOperation FromRaw(
      const OpenPitExecutionReportOperation& raw) {
    ExecutionReportOperation out;
    out.instrument = ::openpit::detail::FromNative<Instrument>(raw.instrument);
    if (raw.account_id.is_set) {
      out.accountId =
          ::openpit::detail::FromNative<param::AccountId>(raw.account_id.value);
    }
    out.side = detail::FromRawEnum<Side>(raw.side, OPENPIT_PARAM_SIDE_NOT_SET);
    return out;
  }

  [[nodiscard]] OpenPitExecutionReportOperation Native() const noexcept {
    OpenPitExecutionReportOperation raw{};
    if (instrument) {
      raw.instrument = ::openpit::detail::Native(*instrument);
    }
    if (accountId) {
      raw.account_id.value = ::openpit::detail::Native(*accountId);
      raw.account_id.is_set = true;
    }
    raw.side = detail::ToRawEnum(side, OPENPIT_PARAM_SIDE_NOT_SET);
    return raw;
  }
};

// Financial-impact group of an execution report: realized pnl and fee.
struct FinancialImpact {
  std::optional<param::Pnl> pnl;
  std::optional<param::Fee> fee;

 private:
  friend class ::openpit::detail::NativeAccess;

  [[nodiscard]] static FinancialImpact FromRaw(
      const OpenPitFinancialImpact& raw) {
    FinancialImpact out;
    if (raw.pnl.is_set) {
      out.pnl = ::openpit::detail::FromNative<param::Pnl>(raw.pnl.value);
    }
    if (raw.fee.is_set) {
      out.fee = ::openpit::detail::FromNative<param::Fee>(raw.fee.value);
    }
    return out;
  }

  [[nodiscard]] OpenPitFinancialImpact Native() const noexcept {
    OpenPitFinancialImpact raw{};
    if (pnl) {
      raw.pnl.value = ::openpit::detail::Native(*pnl);
      raw.pnl.is_set = true;
    }
    if (fee) {
      raw.fee.value = ::openpit::detail::Native(*fee);
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

 private:
  friend class ::openpit::detail::NativeAccess;

  [[nodiscard]] static Trade FromRaw(const OpenPitExecutionReportTrade& raw) {
    return Trade(::openpit::detail::FromNative<param::Price>(raw.price),
                 ::openpit::detail::FromNative<param::Quantity>(raw.quantity));
  }

  [[nodiscard]] OpenPitExecutionReportTrade Native() const noexcept {
    OpenPitExecutionReportTrade raw{};
    raw.price = ::openpit::detail::Native(price);
    raw.quantity = ::openpit::detail::Native(quantity);
    return raw;
  }
};

// Fill-details group of an execution report.
//
struct Fill {
  // An absent lock maps to the C ABI null pointer and is a normal state.
  // Lock requirements belong to the configured policies, not this binding.
  std::optional<::openpit::pretrade::PreTradeLock> lock{};
  std::optional<Trade> lastTrade{};
  // Structured fee amount and currency reported for this fill.
  std::optional<param::MonetaryAmount> fee{};
  std::optional<param::Quantity> leavesQuantity{};
  std::optional<bool> isFinal{};

 private:
  friend class ::openpit::detail::NativeAccess;

  [[nodiscard]] static Fill FromRaw(const OpenPitExecutionReportFill& raw) {
    Fill out{};
    if (raw.lock != nullptr) {
      OpenPitPretradePreTradeLock* clonedRaw =
          openpit_pretrade_pre_trade_lock_clone(raw.lock);
      if (clonedRaw == nullptr) {
        throw ::openpit::Error("pre-trade lock clone failed");
      }
      out.lock =
          ::openpit::detail::FromNative<pretrade::PreTradeLock>(clonedRaw);
    }
    if (raw.last_trade.is_set) {
      out.lastTrade =
          ::openpit::detail::FromNative<Trade>(raw.last_trade.value);
    }
    out.fee = param::detail::MonetaryAmountAccess::FromNative(raw.fee);
    if (raw.leaves_quantity.is_set) {
      out.leavesQuantity = ::openpit::detail::FromNative<param::Quantity>(
          raw.leaves_quantity.value);
    }
    if (raw.is_final.is_set) {
      out.isFinal = raw.is_final.value;
    }
    return out;
  }

  [[nodiscard]] OpenPitExecutionReportFill Native() const {
    OpenPitExecutionReportFill raw{};
    if (lock) {
      if (!*lock) {
        throw ::openpit::Error("fill.lock was moved from");
      }
      raw.lock = ::openpit::detail::Native(*lock);
    }
    if (lastTrade) {
      raw.last_trade.value = ::openpit::detail::Native(*lastTrade);
      raw.last_trade.is_set = true;
    }
    raw.fee = param::detail::MonetaryAmountAccess::Native(fee);
    if (leavesQuantity) {
      raw.leaves_quantity.value = ::openpit::detail::Native(*leavesQuantity);
      raw.leaves_quantity.is_set = true;
    }
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

 private:
  friend class ::openpit::detail::NativeAccess;

  [[nodiscard]] static PositionImpact FromRaw(
      const OpenPitExecutionReportPositionImpact& raw) {
    PositionImpact out;
    out.positionEffect = detail::FromRawEnum<PositionEffect>(
        raw.position_effect, OPENPIT_PARAM_POSITION_EFFECT_NOT_SET);
    out.positionSide = detail::FromRawEnum<PositionSide>(
        raw.position_side, OPENPIT_PARAM_POSITION_SIDE_NOT_SET);
    return out;
  }

  [[nodiscard]] OpenPitExecutionReportPositionImpact Native() const noexcept {
    OpenPitExecutionReportPositionImpact raw{};
    raw.position_effect = detail::ToRawEnum(
        positionEffect, OPENPIT_PARAM_POSITION_EFFECT_NOT_SET);
    raw.position_side =
        detail::ToRawEnum(positionSide, OPENPIT_PARAM_POSITION_SIDE_NOT_SET);
    return raw;
  }
};

//------------------------------------------------------------------------------
// ExecutionReport

// Full execution-report payload. Every group is optional; `userData` is an
// opaque caller token the SDK never inspects (zero means unset). Derives from
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

 private:
  friend class ::openpit::detail::NativeAccess;

  [[nodiscard]] static ExecutionReport FromRaw(
      const OpenPitExecutionReport& raw) {
    ExecutionReport out;
    if (raw.operation.is_set) {
      out.operation = ::openpit::detail::FromNative<ExecutionReportOperation>(
          raw.operation.value);
    }
    if (raw.financial_impact.is_set) {
      out.financialImpact = ::openpit::detail::FromNative<FinancialImpact>(
          raw.financial_impact.value);
    }
    if (raw.fill.is_set) {
      out.fill = ::openpit::detail::FromNative<Fill>(raw.fill.value);
    }
    if (raw.position_impact.is_set) {
      out.positionImpact = ::openpit::detail::FromNative<PositionImpact>(
          raw.position_impact.value);
    }
    out.userData = reinterpret_cast<std::uintptr_t>(raw.user_data);
    return out;
  }

  // Borrows this object's string storage; valid only while it stays alive. A
  // produced fill carries its own lock through to the native view.
  // Routing model fields and engine native view must not diverge.
  [[nodiscard]] OpenPitExecutionReport NativeView() const final {
    OpenPitExecutionReport raw{};
    if (operation) {
      raw.operation.value = ::openpit::detail::Native(*operation);
      raw.operation.is_set = true;
    }
    if (financialImpact) {
      raw.financial_impact.value = ::openpit::detail::Native(*financialImpact);
      raw.financial_impact.is_set = true;
    }
    if (fill) {
      raw.fill.value = ::openpit::detail::Native(*fill);
      raw.fill.is_set = true;
    }
    if (positionImpact) {
      raw.position_impact.value = ::openpit::detail::Native(*positionImpact);
      raw.position_impact.is_set = true;
    }
    raw.user_data = reinterpret_cast<void*>(userData);
    return raw;
  }
};

}  // namespace openpit::model
