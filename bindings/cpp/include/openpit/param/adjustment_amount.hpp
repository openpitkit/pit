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

#include <optional>
#include <variant>

namespace openpit::param {

/// One exact balance adjustment expressed as either a delta or an absolute.
///
/// Factory methods create the valid alternatives.
class AdjustmentAmount final {
 public:
  /// Creates a relative adjustment.
  [[nodiscard]] static AdjustmentAmount Delta(PositionSize value) noexcept {
    return AdjustmentAmount(DeltaValue{value});
  }

  /// Creates an absolute replacement value.
  [[nodiscard]] static AdjustmentAmount Absolute(PositionSize value) noexcept {
    return AdjustmentAmount(AbsoluteValue{value});
  }

  [[nodiscard]] std::optional<PositionSize> AsDelta() const noexcept {
    if (const auto* value = std::get_if<DeltaValue>(&m_value)) {
      return value->value;
    }
    return std::nullopt;
  }

  [[nodiscard]] std::optional<PositionSize> AsAbsolute() const noexcept {
    if (const auto* value = std::get_if<AbsoluteValue>(&m_value)) {
      return value->value;
    }
    return std::nullopt;
  }

  /// Formats the adjustment using the native exact-value representation.
  [[nodiscard]] std::string ToString() const {
    if (std::holds_alternative<UnknownValue>(m_value)) {
      return "not set";
    }
    return ::openpit::detail::StringifyNative(
        Native(), ::openpit_param_adjustment_amount_to_string,
        "adjustment amount string conversion failed");
  }

 private:
  struct DeltaValue {
    PositionSize value;
  };

  struct AbsoluteValue {
    PositionSize value;
  };

  struct UnknownValue {
    detail::RawAdjustmentAmount native;
  };

  friend class ::openpit::detail::NativeAccess;

  explicit AdjustmentAmount(DeltaValue value) noexcept : m_value(value) {}

  explicit AdjustmentAmount(AbsoluteValue value) noexcept : m_value(value) {}

  using Value = std::variant<DeltaValue, AbsoluteValue, UnknownValue>;

  explicit AdjustmentAmount(const detail::RawAdjustmentAmount& native)
      : m_value(FromRaw(native)) {}

  [[nodiscard]] static Value FromRaw(
      const detail::RawAdjustmentAmount& native) {
    switch (native.kind) {
      case OPENPIT_PARAM_ADJUSTMENT_AMOUNT_KIND_DELTA:
        return DeltaValue{
            ::openpit::detail::FromNative<PositionSize>(native.value)};
      case OPENPIT_PARAM_ADJUSTMENT_AMOUNT_KIND_ABSOLUTE:
        return AbsoluteValue{
            ::openpit::detail::FromNative<PositionSize>(native.value)};
      default:
        return UnknownValue{native};
    }
  }

  [[nodiscard]] detail::RawAdjustmentAmount Native() const {
    detail::RawAdjustmentAmount native{};
    if (const auto* delta = std::get_if<DeltaValue>(&m_value)) {
      native.value = ::openpit::detail::Native(delta->value);
      native.kind = OPENPIT_PARAM_ADJUSTMENT_AMOUNT_KIND_DELTA;
    } else if (const auto* absolute = std::get_if<AbsoluteValue>(&m_value)) {
      native.value = ::openpit::detail::Native(absolute->value);
      native.kind = OPENPIT_PARAM_ADJUSTMENT_AMOUNT_KIND_ABSOLUTE;
    } else {
      native = std::get<UnknownValue>(m_value).native;
    }
    return native;
  }

  Value m_value;
};

namespace detail {

class AdjustmentAmountAccess final {
 private:
  [[nodiscard]] static std::optional<AdjustmentAmount> FromNative(
      const RawAdjustmentAmount& native) {
    if (native.kind == OPENPIT_PARAM_ADJUSTMENT_AMOUNT_KIND_NOT_SET) {
      return std::nullopt;
    }
    return ::openpit::detail::FromNative<AdjustmentAmount>(native);
  }

  friend struct ::openpit::accountadjustment::Amount;
};

}  // namespace detail
}  // namespace openpit::param
