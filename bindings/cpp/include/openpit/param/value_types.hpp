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
#include "openpit/error.hpp"
#include "openpit/param/detail/value.hpp"

#include <optional>
#include <string>
#include <string_view>
#include <type_traits>
#include <utility>

namespace openpit::param {

class Quantity;
class Volume;
class Pnl;
class Fee;
class PositionSize;
class CashFlow;
class Notional;
class Leverage;

/// Per-unit instrument price; may be negative in derivative markets.
class Price final : public detail::ExactValue<Price, detail::PriceTraits> {
  using Base = detail::ExactValue<Price, detail::PriceTraits>;

 public:
  /// Negates the price and throws `openpit::Error` on overflow.
  [[nodiscard]] Price CheckedNeg() const;
  /// Calculates settlement volume for a quantity.
  [[nodiscard]] Volume CalculateVolume(const Quantity& quantity) const;
  /// Calculates signed position size for a quantity.
  [[nodiscard]] PositionSize CalculatePositionSize(
      const Quantity& quantity) const;
  /// Calculates notional exposure for a quantity.
  [[nodiscard]] Notional CalculateNotional(const Quantity& quantity) const;

 private:
  friend Base;
  friend class ::openpit::detail::NativeAccess;
  explicit Price(detail::RawPrice value) noexcept : Base(value) {}
};

/// Non-negative instrument quantity.
class Quantity final
    : public detail::ExactValue<Quantity, detail::QuantityTraits> {
  using Base = detail::ExactValue<Quantity, detail::QuantityTraits>;

 public:
  /// Calculates settlement volume at a price.
  [[nodiscard]] Volume CalculateVolume(const Price& price) const;
  /// Calculates notional exposure at a price.
  [[nodiscard]] Notional CalculateNotional(const Price& price) const;
  /// Converts the quantity to a positive position size.
  [[nodiscard]] PositionSize ToPositionSize() const;
  /// Converts the quantity to a position size directed by `side`.
  [[nodiscard]] PositionSize ToPositionSize(Side side) const;

 private:
  friend Base;
  friend class ::openpit::detail::NativeAccess;
  explicit Quantity(detail::RawQuantity value) noexcept : Base(value) {}
};

/// Non-negative settlement volume.
class Volume final : public detail::ExactValue<Volume, detail::VolumeTraits> {
  using Base = detail::ExactValue<Volume, detail::VolumeTraits>;

 public:
  /// Calculates quantity as zero at a zero price, or volume divided by the
  /// absolute price otherwise.
  [[nodiscard]] Quantity CalculateQuantity(const Price& price) const;
  /// Converts the volume to a positive cash inflow.
  [[nodiscard]] CashFlow ToCashFlowInflow() const;
  /// Converts the volume to a negative cash outflow.
  [[nodiscard]] CashFlow ToCashFlowOutflow() const;
  /// Converts the volume to a positive position size.
  [[nodiscard]] PositionSize ToPositionSize() const;
  /// Creates volume from equivalent notional exposure.
  [[nodiscard]] static Volume FromNotional(const Notional& notional);
  /// Converts the volume to equivalent notional exposure.
  [[nodiscard]] Notional ToNotional() const;

 private:
  friend Base;
  friend class ::openpit::detail::NativeAccess;
  explicit Volume(detail::RawVolume value) noexcept : Base(value) {}
};

/// Signed profit-and-loss contribution.
class Pnl final : public detail::ExactValue<Pnl, detail::PnlTraits> {
  using Base = detail::ExactValue<Pnl, detail::PnlTraits>;

 public:
  /// Converts a fee contribution to its PnL effect.
  [[nodiscard]] static Pnl FromFee(const Fee& fee);

  /// Negates the PnL and throws `openpit::Error` on overflow.
  [[nodiscard]] Pnl CheckedNeg() const;
  /// Converts the PnL to an equivalent cash-flow contribution.
  [[nodiscard]] CashFlow ToCashFlow() const;
  /// Converts the PnL to an equivalent signed position size.
  [[nodiscard]] PositionSize ToPositionSize() const;

 private:
  friend Base;
  friend class ::openpit::detail::NativeAccess;
  explicit Pnl(detail::RawPnl value) noexcept : Base(value) {}
};

/// Signed fee or rebate contribution.
class Fee final : public detail::ExactValue<Fee, detail::FeeTraits> {
  using Base = detail::ExactValue<Fee, detail::FeeTraits>;

 public:
  /// Negates the fee and throws `openpit::Error` on overflow.
  [[nodiscard]] Fee CheckedNeg() const;
  /// Converts the fee to its PnL effect.
  [[nodiscard]] Pnl ToPnl() const;
  /// Converts the fee to its signed position-size effect.
  [[nodiscard]] PositionSize ToPositionSize() const;
  /// Converts the fee to its cash-flow effect.
  [[nodiscard]] CashFlow ToCashFlow() const;

 private:
  friend Base;
  friend class ::openpit::detail::NativeAccess;
  explicit Fee(detail::RawFee value) noexcept : Base(value) {}
};

/// Signed position quantity: positive is long and negative is short.
class PositionSize final
    : public detail::ExactValue<PositionSize, detail::PositionSizeTraits> {
  using Base = detail::ExactValue<PositionSize, detail::PositionSizeTraits>;

 public:
  /// Converts a PnL contribution to an equivalent position size.
  [[nodiscard]] static PositionSize FromPnl(const Pnl& pnl);
  /// Converts a fee contribution to its position-size effect.
  [[nodiscard]] static PositionSize FromFee(const Fee& fee);
  /// Creates a signed position size from quantity and direction.
  [[nodiscard]] static PositionSize FromQuantityAndSide(
      const Quantity& quantity, Side side);

  /// Negates the position size and throws `openpit::Error` on overflow.
  [[nodiscard]] PositionSize CheckedNeg() const;
  /// Returns the absolute opening quantity and its direction.
  [[nodiscard]] std::pair<Quantity, Side> OpenQuantity() const;
  /// Returns the absolute closing quantity and optional direction.
  [[nodiscard]] std::pair<Quantity, std::optional<Side>> CloseQuantity() const;
  /// Applies a directed quantity and throws `openpit::Error` on failure.
  [[nodiscard]] PositionSize CheckedAddQuantity(const Quantity& quantity,
                                                Side side) const;

 private:
  friend Base;
  friend class ::openpit::detail::NativeAccess;
  explicit PositionSize(detail::RawPositionSize value) noexcept : Base(value) {}
};

/// Signed cash-flow contribution: positive is inflow and negative is outflow.
class CashFlow final
    : public detail::ExactValue<CashFlow, detail::CashFlowTraits> {
  using Base = detail::ExactValue<CashFlow, detail::CashFlowTraits>;

 public:
  /// Converts a PnL contribution to an equivalent cash flow.
  [[nodiscard]] static CashFlow FromPnl(const Pnl& pnl);
  /// Converts a fee contribution to its cash-flow effect.
  [[nodiscard]] static CashFlow FromFee(const Fee& fee);
  /// Creates a positive cash inflow from a volume.
  [[nodiscard]] static CashFlow FromVolumeInflow(const Volume& volume);
  /// Creates a negative cash outflow from a volume.
  [[nodiscard]] static CashFlow FromVolumeOutflow(const Volume& volume);

  /// Negates the cash flow and throws `openpit::Error` on overflow.
  [[nodiscard]] CashFlow CheckedNeg() const;

 private:
  friend Base;
  friend class ::openpit::detail::NativeAccess;
  explicit CashFlow(detail::RawCashFlow value) noexcept : Base(value) {}
};

/// Non-negative monetary position exposure used for margin and risk.
class Notional final
    : public detail::ExactValue<Notional, detail::NotionalTraits> {
  using Base = detail::ExactValue<Notional, detail::NotionalTraits>;

 public:
  /// Creates notional exposure from an equivalent volume.
  [[nodiscard]] static Notional FromVolume(const Volume& volume);
  /// Converts notional exposure to an equivalent volume.
  [[nodiscard]] Volume ToVolume() const;
  /// Calculates required margin at the supplied leverage.
  [[nodiscard]] Notional CalculateMarginRequired(
      const Leverage& leverage) const;

 private:
  friend Base;
  friend class ::openpit::detail::NativeAccess;
  explicit Notional(detail::RawNotional value) noexcept : Base(value) {}
};

namespace detail {

class ValueOperations final {
 private:
  template <typename Result, typename Function, typename Left, typename Right>
  [[nodiscard]] static Result Calculate(Function function, const Left& left,
                                        const Right& right,
                                        const char* fallback) {
    using NativeResult = std::decay_t<decltype(::openpit::detail::Native(
        std::declval<const Result&>()))>;
    NativeResult native{};
    ::OpenPitParamError* error = nullptr;
    if (!function(::openpit::detail::Native(left),
                  ::openpit::detail::Native(right), &native, &error)) {
      ::openpit::detail::ThrowFromParamError(error, fallback);
    }
    return ::openpit::detail::FromNative<Result>(native);
  }

  template <typename Result, typename Function, typename Value>
  [[nodiscard]] static Result Convert(Function function, const Value& value,
                                      const char* fallback) {
    using NativeResult = std::decay_t<decltype(::openpit::detail::Native(
        std::declval<const Result&>()))>;
    NativeResult native{};
    ::OpenPitParamError* error = nullptr;
    if (!function(::openpit::detail::Native(value), &native, &error)) {
      ::openpit::detail::ThrowFromParamError(error, fallback);
    }
    return ::openpit::detail::FromNative<Result>(native);
  }

  friend class ::openpit::param::CashFlow;
  friend class ::openpit::param::Fee;
  friend class ::openpit::param::Notional;
  friend class ::openpit::param::Pnl;
  friend class ::openpit::param::PositionSize;
  friend class ::openpit::param::Price;
  friend class ::openpit::param::Quantity;
  friend class ::openpit::param::Volume;
};

}  // namespace detail

inline Price Price::CheckedNeg() const {
  return detail::ValueOperations::Convert<Price>(
      detail::PriceTraits::CheckedNeg, *this, "price negation failed");
}

inline Volume Price::CalculateVolume(const Quantity& quantity) const {
  return detail::ValueOperations::Calculate<Volume>(
      ::openpit_param_price_calculate_volume, *this, quantity,
      "price volume calculation failed");
}

inline Notional Price::CalculateNotional(const Quantity& quantity) const {
  return detail::ValueOperations::Calculate<Notional>(
      ::openpit_param_price_calculate_notional, *this, quantity,
      "price notional calculation failed");
}

inline PositionSize Price::CalculatePositionSize(
    const Quantity& quantity) const {
  return detail::ValueOperations::Calculate<PositionSize>(
      ::openpit_param_price_calculate_position_size, *this, quantity,
      "price position-size calculation failed");
}

inline Volume Quantity::CalculateVolume(const Price& price) const {
  return detail::ValueOperations::Calculate<Volume>(
      ::openpit_param_quantity_calculate_volume, *this, price,
      "quantity volume calculation failed");
}

inline Notional Quantity::CalculateNotional(const Price& price) const {
  return detail::ValueOperations::Calculate<Notional>(
      ::openpit_param_quantity_calculate_notional, *this, price,
      "quantity notional calculation failed");
}

inline PositionSize Quantity::ToPositionSize() const {
  return detail::ValueOperations::Convert<PositionSize>(
      ::openpit_param_quantity_to_position_size, *this,
      "quantity position-size conversion failed");
}

inline PositionSize Quantity::ToPositionSize(Side side) const {
  detail::RawPositionSize native{};
  ::OpenPitParamError* error = nullptr;
  if (!::openpit_param_position_size_from_quantity_and_side(
          ::openpit::detail::Native(*this),
          static_cast<::OpenPitParamSide>(side), &native, &error)) {
    ::openpit::detail::ThrowFromParamError(
        error, "quantity/side position-size conversion failed");
  }
  return ::openpit::detail::FromNative<PositionSize>(native);
}

inline Quantity Volume::CalculateQuantity(const Price& price) const {
  return detail::ValueOperations::Calculate<Quantity>(
      ::openpit_param_volume_calculate_quantity, *this, price,
      "volume quantity calculation failed");
}

inline CashFlow Volume::ToCashFlowInflow() const {
  return detail::ValueOperations::Convert<CashFlow>(
      ::openpit_param_volume_to_cash_flow_inflow, *this,
      "volume inflow cash-flow conversion failed");
}

inline CashFlow Volume::ToCashFlowOutflow() const {
  return detail::ValueOperations::Convert<CashFlow>(
      ::openpit_param_volume_to_cash_flow_outflow, *this,
      "volume outflow cash-flow conversion failed");
}

inline PositionSize Volume::ToPositionSize() const {
  return detail::ValueOperations::Convert<PositionSize>(
      ::openpit_param_volume_to_position_size, *this,
      "volume position-size conversion failed");
}

inline Volume Volume::FromNotional(const Notional& notional) {
  return detail::ValueOperations::Convert<Volume>(
      ::openpit_param_volume_from_notional, notional,
      "notional volume conversion failed");
}

inline Notional Volume::ToNotional() const {
  return detail::ValueOperations::Convert<Notional>(
      ::openpit_param_notional_from_volume, *this,
      "volume notional conversion failed");
}

inline Pnl Pnl::FromFee(const Fee& fee) {
  return detail::ValueOperations::Convert<Pnl>(
      ::openpit_param_pnl_from_fee, fee, "fee pnl conversion failed");
}

inline Pnl Pnl::CheckedNeg() const {
  return detail::ValueOperations::Convert<Pnl>(detail::PnlTraits::CheckedNeg,
                                               *this, "pnl negation failed");
}

inline CashFlow Pnl::ToCashFlow() const {
  return detail::ValueOperations::Convert<CashFlow>(
      ::openpit_param_pnl_to_cash_flow, *this,
      "pnl cash-flow conversion failed");
}

inline PositionSize Pnl::ToPositionSize() const {
  return detail::ValueOperations::Convert<PositionSize>(
      ::openpit_param_pnl_to_position_size, *this,
      "pnl position-size conversion failed");
}

inline Fee Fee::CheckedNeg() const {
  return detail::ValueOperations::Convert<Fee>(detail::FeeTraits::CheckedNeg,
                                               *this, "fee negation failed");
}

inline Pnl Fee::ToPnl() const {
  return detail::ValueOperations::Convert<Pnl>(
      ::openpit_param_fee_to_pnl, *this, "fee pnl conversion failed");
}

inline PositionSize Fee::ToPositionSize() const {
  return detail::ValueOperations::Convert<PositionSize>(
      ::openpit_param_fee_to_position_size, *this,
      "fee position-size conversion failed");
}

inline CashFlow Fee::ToCashFlow() const {
  return detail::ValueOperations::Convert<CashFlow>(
      ::openpit_param_fee_to_cash_flow, *this,
      "fee cash-flow conversion failed");
}

inline PositionSize PositionSize::FromPnl(const Pnl& pnl) {
  return detail::ValueOperations::Convert<PositionSize>(
      ::openpit_param_position_size_from_pnl, pnl,
      "pnl position-size conversion failed");
}

inline PositionSize PositionSize::FromFee(const Fee& fee) {
  return detail::ValueOperations::Convert<PositionSize>(
      ::openpit_param_position_size_from_fee, fee,
      "fee position-size conversion failed");
}

inline PositionSize PositionSize::FromQuantityAndSide(const Quantity& quantity,
                                                      Side side) {
  return quantity.ToPositionSize(side);
}

inline PositionSize PositionSize::CheckedNeg() const {
  return detail::ValueOperations::Convert<PositionSize>(
      detail::PositionSizeTraits::CheckedNeg, *this,
      "position-size negation failed");
}

inline std::pair<Quantity, Side> PositionSize::OpenQuantity() const {
  detail::RawQuantity quantity{};
  ::OpenPitParamSide side = OPENPIT_PARAM_SIDE_NOT_SET;
  ::OpenPitParamError* error = nullptr;
  if (!::openpit_param_position_size_to_open_quantity(
          ::openpit::detail::Native(*this), &quantity, &side, &error)) {
    ::openpit::detail::ThrowFromParamError(
        error, "position-size open-quantity conversion failed");
  }
  return {::openpit::detail::FromNative<Quantity>(quantity),
          static_cast<Side>(side)};
}

inline std::pair<Quantity, std::optional<Side>> PositionSize::CloseQuantity()
    const {
  detail::RawQuantity quantity{};
  ::OpenPitParamSide side = OPENPIT_PARAM_SIDE_NOT_SET;
  ::OpenPitParamError* error = nullptr;
  if (!::openpit_param_position_size_to_close_quantity(
          ::openpit::detail::Native(*this), &quantity, &side, &error)) {
    ::openpit::detail::ThrowFromParamError(
        error, "position-size close-quantity conversion failed");
  }
  std::optional<Side> resultSide;
  if (side != OPENPIT_PARAM_SIDE_NOT_SET) {
    resultSide = static_cast<Side>(side);
  }
  return {::openpit::detail::FromNative<Quantity>(quantity), resultSide};
}

inline PositionSize PositionSize::CheckedAddQuantity(const Quantity& quantity,
                                                     Side side) const {
  detail::RawPositionSize native{};
  ::OpenPitParamError* error = nullptr;
  if (!::openpit_param_position_size_checked_add_quantity(
          ::openpit::detail::Native(*this), ::openpit::detail::Native(quantity),
          static_cast<::OpenPitParamSide>(side), &native, &error)) {
    ::openpit::detail::ThrowFromParamError(
        error, "position-size quantity addition failed");
  }
  return ::openpit::detail::FromNative<PositionSize>(native);
}

inline CashFlow CashFlow::FromPnl(const Pnl& pnl) {
  return detail::ValueOperations::Convert<CashFlow>(
      ::openpit_param_cash_flow_from_pnl, pnl,
      "pnl cash-flow conversion failed");
}

inline CashFlow CashFlow::FromFee(const Fee& fee) {
  return detail::ValueOperations::Convert<CashFlow>(
      ::openpit_param_cash_flow_from_fee, fee,
      "fee cash-flow conversion failed");
}

inline CashFlow CashFlow::FromVolumeInflow(const Volume& volume) {
  return detail::ValueOperations::Convert<CashFlow>(
      ::openpit_param_cash_flow_from_volume_inflow, volume,
      "volume inflow cash-flow conversion failed");
}

inline CashFlow CashFlow::FromVolumeOutflow(const Volume& volume) {
  return detail::ValueOperations::Convert<CashFlow>(
      ::openpit_param_cash_flow_from_volume_outflow, volume,
      "volume outflow cash-flow conversion failed");
}

inline CashFlow CashFlow::CheckedNeg() const {
  return detail::ValueOperations::Convert<CashFlow>(
      detail::CashFlowTraits::CheckedNeg, *this, "cash-flow negation failed");
}

inline Notional Notional::FromVolume(const Volume& volume) {
  return detail::ValueOperations::Convert<Notional>(
      ::openpit_param_notional_from_volume, volume,
      "volume notional conversion failed");
}

inline Volume Notional::ToVolume() const {
  return detail::ValueOperations::Convert<Volume>(
      ::openpit_param_notional_to_volume, *this,
      "notional volume conversion failed");
}

/// Formats a buy/sell direction.
[[nodiscard]] inline std::string ToString(Side side) {
  return ::openpit::detail::StringifyNative(
      static_cast<::OpenPitParamSide>(side), ::openpit_param_side_to_string,
      "side string conversion failed");
}

}  // namespace openpit::param
