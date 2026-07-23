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
#include "openpit/param/detail/native.hpp"
#include "openpit/param/value_types.hpp"

#include <cmath>
#include <cstdint>
#include <limits>
#include <optional>
#include <string>

namespace openpit::param {

/// Fixed-point leverage multiplier with one decimal place.
class Leverage final {
 public:
  static constexpr std::uint16_t Scale = 10;
  static constexpr std::uint16_t NotSet = 0;
  static constexpr std::uint16_t Min = 1;
  static constexpr std::uint16_t Max = 3000;
  static constexpr float Step = 0.1F;

  constexpr Leverage() noexcept = default;

  /// Creates an integral leverage multiplier.
  [[nodiscard]] static Leverage FromUint16(std::uint16_t multiplier) {
    constexpr auto maxRaw = std::numeric_limits<detail::RawLeverage>::max();
    if (multiplier > maxRaw / Scale) {
      throw ::openpit::Error(
          "leverage multiplier exceeds the fixed-point representation");
    }
    return Leverage(static_cast<detail::RawLeverage>(multiplier * Scale));
  }

  /// Creates a finite multiplier rounded to the fixed-point scale.
  [[nodiscard]] static Leverage FromFloat(float multiplier) {
    if (!std::isfinite(multiplier)) {
      throw ::openpit::Error("leverage multiplier must be finite");
    }
    const double scaled =
        static_cast<double>(multiplier) * static_cast<double>(Scale);
    const double rounded = std::round(scaled);
    if (rounded < 0.0 ||
        rounded > static_cast<double>(
                      std::numeric_limits<detail::RawLeverage>::max())) {
      throw ::openpit::Error(
          "leverage multiplier exceeds the fixed-point representation");
    }
    return Leverage(static_cast<detail::RawLeverage>(rounded));
  }

  [[nodiscard]] constexpr bool IsSet() const noexcept {
    return m_value != NotSet;
  }

  [[nodiscard]] constexpr float Value() const noexcept {
    return static_cast<float>(m_value) / static_cast<float>(Scale);
  }

  [[nodiscard]] std::string ToString() const {
    const auto integer = static_cast<unsigned>(m_value / Scale);
    const auto fractional = static_cast<unsigned>(m_value % Scale);
    if (fractional == 0) {
      return std::to_string(integer);
    }
    return std::to_string(integer) + "." + std::to_string(fractional);
  }

  /// Calculates margin required for the supplied notional.
  [[nodiscard]] Notional CalculateMarginRequired(
      const Notional& notional) const {
    detail::RawNotional native{};
    ::OpenPitParamError* error = nullptr;
    if (!::openpit_param_leverage_calculate_margin_required(
            m_value, ::openpit::detail::Native(notional), &native, &error)) {
      ::openpit::detail::ThrowFromParamError(
          error, "leverage margin calculation failed");
    }
    return ::openpit::detail::FromNative<Notional>(native);
  }

  [[nodiscard]] constexpr bool operator==(
      const Leverage& other) const noexcept {
    return m_value == other.m_value;
  }

  [[nodiscard]] constexpr bool operator!=(
      const Leverage& other) const noexcept {
    return !(*this == other);
  }

  [[nodiscard]] constexpr bool operator<(const Leverage& other) const noexcept {
    return m_value < other.m_value;
  }

  [[nodiscard]] constexpr bool operator<=(
      const Leverage& other) const noexcept {
    return !(other < *this);
  }

  [[nodiscard]] constexpr bool operator>(const Leverage& other) const noexcept {
    return other < *this;
  }

  [[nodiscard]] constexpr bool operator>=(
      const Leverage& other) const noexcept {
    return !(*this < other);
  }

 private:
  friend class ::openpit::detail::NativeAccess;

  explicit constexpr Leverage(detail::RawLeverage native) noexcept
      : m_value(native) {}

  [[nodiscard]] constexpr detail::RawLeverage Native() const noexcept {
    return m_value;
  }

  detail::RawLeverage m_value = NotSet;
};

inline Notional Notional::CalculateMarginRequired(
    const Leverage& leverage) const {
  detail::RawNotional native{};
  ::OpenPitParamError* error = nullptr;
  if (!::openpit_param_notional_calculate_margin_required(
          ::openpit::detail::Native(*this), ::openpit::detail::Native(leverage),
          &native, &error)) {
    ::openpit::detail::ThrowFromParamError(
        error, "notional margin calculation failed");
  }
  return ::openpit::detail::FromNative<Notional>(native);
}

namespace detail {

class LeverageAccess final {
 private:
  [[nodiscard]] static constexpr std::optional<Leverage> FromNative(
      RawLeverage native) noexcept {
    if (native == Leverage::NotSet) {
      return std::nullopt;
    }
    return ::openpit::detail::FromNative<Leverage>(native);
  }

  [[nodiscard]] static constexpr RawLeverage Native(
      const std::optional<Leverage>& value) noexcept {
    return value ? ::openpit::detail::Native(*value) : Leverage::NotSet;
  }

  friend struct ::openpit::accountadjustment::PositionOperation;
  friend struct ::openpit::model::OrderMargin;
};

}  // namespace detail
}  // namespace openpit::param
