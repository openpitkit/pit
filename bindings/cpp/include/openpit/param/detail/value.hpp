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
#include "openpit/param/decimal.hpp"
#include "openpit/param/detail/native.hpp"
#include "openpit/param/enums.hpp"
#include "openpit/string.hpp"

#include <cstdint>
#include <string>
#include <string_view>

namespace openpit::param::detail {

[[nodiscard]] inline ::OpenPitParamRoundingStrategy NativeRounding(
    RoundingStrategy strategy) noexcept {
  return static_cast<::OpenPitParamRoundingStrategy>(strategy);
}

template <typename Derived, typename Traits>
class ExactValue {
 public:
  /// Parses an exact decimal string.
  [[nodiscard]] static Derived FromString(std::string_view value) {
    return Derived(Parse(value));
  }

  /// Validates an exact decimal representation.
  [[nodiscard]] static Derived FromDecimal(Decimal value) {
    NativeType native{};
    ::OpenPitParamError* error = nullptr;
    if (!Traits::FromDecimal(NativeDecimal(value), &native, &error)) {
      Throw(error, "construction from decimal");
    }
    return Derived(native);
  }

  /// Creates a value from a signed integer.
  [[nodiscard]] static Derived FromInt64(std::int64_t value) {
    NativeType native{};
    ::OpenPitParamError* error = nullptr;
    if (!Traits::FromInt64(value, &native, &error)) {
      Throw(error, "construction from int64");
    }
    return Derived(native);
  }

  /// Creates a value from an unsigned integer.
  [[nodiscard]] static Derived FromUint64(std::uint64_t value) {
    NativeType native{};
    ::OpenPitParamError* error = nullptr;
    if (!Traits::FromUint64(value, &native, &error)) {
      Throw(error, "construction from uint64");
    }
    return Derived(native);
  }

  /// Creates a value from an imprecise boundary `double`.
  /// Prefer `FromString` or `FromDecimal` for financial input.
  [[nodiscard]] static Derived FromDouble(double value) {
    NativeType native{};
    ::OpenPitParamError* error = nullptr;
    if (!Traits::FromDouble(value, &native, &error)) {
      Throw(error, "construction from double");
    }
    return Derived(native);
  }

  /// Parses and rounds an exact decimal string to `scale` digits.
  [[nodiscard]] static Derived FromStringRounded(std::string_view value,
                                                 std::uint32_t scale,
                                                 RoundingStrategy rounding) {
    NativeType native{};
    ::OpenPitParamError* error = nullptr;
    if (!Traits::FromStringRounded(::openpit::detail::MakeStringView(value),
                                   scale, NativeRounding(rounding), &native,
                                   &error)) {
      Throw(error, "rounded construction from string");
    }
    return Derived(native);
  }

  /// Converts and rounds an imprecise boundary `double`.
  [[nodiscard]] static Derived FromDoubleRounded(double value,
                                                 std::uint32_t scale,
                                                 RoundingStrategy rounding) {
    NativeType native{};
    ::OpenPitParamError* error = nullptr;
    if (!Traits::FromDoubleRounded(value, scale, NativeRounding(rounding),
                                   &native, &error)) {
      Throw(error, "rounded construction from double");
    }
    return Derived(native);
  }

  /// Rounds an exact decimal representation to `scale` digits.
  [[nodiscard]] static Derived FromDecimalRounded(Decimal value,
                                                  std::uint32_t scale,
                                                  RoundingStrategy rounding) {
    NativeType native{};
    ::OpenPitParamError* error = nullptr;
    if (!Traits::FromDecimalRounded(NativeDecimal(value), scale,
                                    NativeRounding(rounding), &native,
                                    &error)) {
      Throw(error, "rounded construction from decimal");
    }
    return Derived(native);
  }

  /// Returns the lossless decimal representation.
  [[nodiscard]] ::openpit::param::Decimal Decimal() const noexcept {
    const ::OpenPitParamDecimal native = Traits::Decimal(m_value);
    return {native.mantissa_lo, native.mantissa_hi, native.scale};
  }

  /// Formats the exact value as a decimal string.
  [[nodiscard]] std::string ToString() const {
    ::OpenPitParamError* error = nullptr;
    ::OpenPitSharedString* handle = Traits::String(m_value, &error);
    if (handle == nullptr) {
      Throw(error, "string conversion");
    }
    return ::openpit::detail::FromNative<::openpit::SharedString>(handle)
        .ToString();
  }

  /// Converts to an imprecise boundary `double`.
  [[nodiscard]] double ToDouble() const {
    double result = 0.0;
    ::OpenPitParamError* error = nullptr;
    if (!Traits::ToDouble(m_value, &result, &error)) {
      Throw(error, "double conversion");
    }
    return result;
  }

  /// Returns whether the value is exactly zero.
  [[nodiscard]] bool IsZero() const {
    bool result = false;
    ::OpenPitParamError* error = nullptr;
    if (!Traits::Zero(m_value, &result, &error)) {
      Throw(error, "zero comparison");
    }
    return result;
  }

  /// Compares this value with another value of the same domain type.
  [[nodiscard]] int Compare(const Derived& other) const {
    std::int8_t result = 0;
    ::OpenPitParamError* error = nullptr;
    if (!Traits::CompareValues(m_value, ::openpit::detail::Native(other),
                               &result, &error)) {
      Throw(error, "comparison");
    }
    return static_cast<int>(result);
  }

  /// Adds another value and throws `openpit::Error` on overflow.
  [[nodiscard]] Derived CheckedAdd(const Derived& other) const {
    NativeType result{};
    ::OpenPitParamError* error = nullptr;
    if (!Traits::CheckedAdd(m_value, ::openpit::detail::Native(other), &result,
                            &error)) {
      Throw(error, "addition");
    }
    return Derived(result);
  }

  /// Subtracts another value and throws `openpit::Error` on failure.
  [[nodiscard]] Derived CheckedSubtract(const Derived& other) const {
    NativeType result{};
    ::OpenPitParamError* error = nullptr;
    if (!Traits::CheckedSubtract(m_value, ::openpit::detail::Native(other),
                                 &result, &error)) {
      Throw(error, "subtraction");
    }
    return Derived(result);
  }

  /// Multiplies by a signed integer and throws `openpit::Error` on failure.
  [[nodiscard]] Derived CheckedMulInt(std::int64_t scalar) const {
    return ApplyScalar(Traits::CheckedMulInt, scalar, "integer multiplication");
  }

  /// Multiplies by an unsigned integer and throws `openpit::Error` on failure.
  [[nodiscard]] Derived CheckedMulUint(std::uint64_t scalar) const {
    return ApplyScalar(Traits::CheckedMulUint, scalar,
                       "unsigned integer multiplication");
  }

  /// Multiplies by an imprecise boundary `double`.
  [[nodiscard]] Derived CheckedMulFloat(double scalar) const {
    return ApplyScalar(Traits::CheckedMulFloat, scalar,
                       "floating-point multiplication");
  }

  /// Divides by a signed integer and throws `openpit::Error` on failure.
  [[nodiscard]] Derived CheckedDivInt(std::int64_t divisor) const {
    return ApplyScalar(Traits::CheckedDivInt, divisor, "integer division");
  }

  /// Divides by an unsigned integer and throws `openpit::Error` on failure.
  [[nodiscard]] Derived CheckedDivUint(std::uint64_t divisor) const {
    return ApplyScalar(Traits::CheckedDivUint, divisor,
                       "unsigned integer division");
  }

  /// Divides by an imprecise boundary `double`.
  [[nodiscard]] Derived CheckedDivFloat(double divisor) const {
    return ApplyScalar(Traits::CheckedDivFloat, divisor,
                       "floating-point division");
  }

  /// Computes the remainder for a signed integer divisor.
  [[nodiscard]] Derived CheckedRemInt(std::int64_t divisor) const {
    return ApplyScalar(Traits::CheckedRemInt, divisor, "integer remainder");
  }

  /// Computes the remainder for an unsigned integer divisor.
  [[nodiscard]] Derived CheckedRemUint(std::uint64_t divisor) const {
    return ApplyScalar(Traits::CheckedRemUint, divisor,
                       "unsigned integer remainder");
  }

  /// Computes the remainder for an imprecise boundary `double` divisor.
  [[nodiscard]] Derived CheckedRemFloat(double divisor) const {
    return ApplyScalar(Traits::CheckedRemFloat, divisor,
                       "floating-point remainder");
  }

  [[nodiscard]] bool operator==(const Derived& other) const {
    return Compare(other) == 0;
  }

  [[nodiscard]] bool operator!=(const Derived& other) const {
    return Compare(other) != 0;
  }

  [[nodiscard]] bool operator<(const Derived& other) const {
    return Compare(other) < 0;
  }

  [[nodiscard]] bool operator<=(const Derived& other) const {
    return Compare(other) <= 0;
  }

  [[nodiscard]] bool operator>(const Derived& other) const {
    return Compare(other) > 0;
  }

  [[nodiscard]] bool operator>=(const Derived& other) const {
    return Compare(other) >= 0;
  }

 private:
  friend Derived;
  friend class ::openpit::detail::NativeAccess;

  using NativeType = typename Traits::Native;

  explicit ExactValue(std::string_view value) : m_value(Parse(value)) {}
  explicit ExactValue(NativeType native) noexcept : m_value(native) {}

  [[nodiscard]] NativeType Native() const noexcept { return m_value; }

  [[nodiscard]] static ::OpenPitParamDecimal NativeDecimal(
      ::openpit::param::Decimal value) noexcept {
    return {value.mantissaLo, value.mantissaHi, value.scale};
  }

  template <typename Function, typename Scalar>
  [[nodiscard]] Derived ApplyScalar(Function function, Scalar scalar,
                                    std::string_view operation) const {
    NativeType result{};
    ::OpenPitParamError* error = nullptr;
    if (!function(m_value, scalar, &result, &error)) {
      Throw(error, operation);
    }
    return Derived(result);
  }

  [[nodiscard]] static NativeType Parse(std::string_view value) {
    NativeType native{};
    ::OpenPitParamError* error = nullptr;
    if (!Traits::FromString(::openpit::detail::MakeStringView(value), &native,
                            &error)) {
      Throw(error, "construction from string");
    }
    return native;
  }

  [[noreturn]] static void Throw(::OpenPitParamError* error,
                                 std::string_view operation) {
    const std::string fallback =
        std::string(Traits::Name) + " " + std::string(operation) + " failed";
    ::openpit::detail::ThrowFromParamError(error, fallback.c_str());
  }

  NativeType m_value{};
};

}  // namespace openpit::param::detail
