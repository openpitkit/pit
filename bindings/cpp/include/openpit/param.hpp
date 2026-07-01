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

#include "openpit/error.hpp"
#include "openpit/string.hpp"

#include <openpit.h>

#include <cmath>
#include <cstdint>
#include <optional>
#include <string>
#include <string_view>
#include <utility>

// Financial value types.
//
// Each type wraps the validated native runtime POD (`OpenPitParamPrice`, ...)
// which stores an exact `OpenPitParamDecimal` (i128 mantissa + scale).
// Construction and arithmetic cross the C boundary so results stay bit-for-bit
// identical across OpenPit SDKs.
//
// `FromString` is the exact, deterministic constructor for monetary input.
// `FromDouble` exists only for boundary adapters and tests that must mirror
// floating-point inputs. Float input is inherently imprecise and is NOT
// cross-platform deterministic. Never use double for money in business logic.
//
// Construction/arithmetic failures are boundary failures and throw
// `openpit::Error` (carrying the C param error code). Reading accessors on an
// already-validated value cannot fail.

namespace openpit::param {

// Decimal rounding strategy mapped 1:1 from the native runtime enum.
enum class RoundingStrategy : std::uint8_t {
  MidpointNearestEven = OpenPitParamRoundingStrategy_MidpointNearestEven,
  MidpointAwayFromZero = OpenPitParamRoundingStrategy_MidpointAwayFromZero,
  Up = OpenPitParamRoundingStrategy_Up,
  Down = OpenPitParamRoundingStrategy_Down,
};

using AccountGroupIdOptional = OpenPitParamAccountGroupIdOptional;
using AccountIdOptional = OpenPitParamAccountIdOptional;
using CashFlowOptional = OpenPitParamCashFlowOptional;
using FeeOptional = OpenPitParamFeeOptional;
using MonetaryAmountOptional = OpenPitParamMonetaryAmountOptional;
using NotionalOptional = OpenPitParamNotionalOptional;
using PnlOptional = OpenPitParamPnlOptional;
using PositionSizeOptional = OpenPitParamPositionSizeOptional;
using PriceOptional = OpenPitParamPriceOptional;
using QuantityOptional = OpenPitParamQuantityOptional;
using VolumeOptional = OpenPitParamVolumeOptional;

[[nodiscard]] inline OpenPitParamRoundingStrategy ToRaw(
    RoundingStrategy strategy) noexcept {
  return static_cast<OpenPitParamRoundingStrategy>(strategy);
}

// Defines one validated value type wrapping the C POD `CType`, built from /
// converted to the native runtime exact-decimal handles. `Name` is the C++
// class; `Prefix` is the C symbol infix, e.g. `price` for
// `openpit_create_param_price`.
#define OPENPIT_PARAM_DEFINE_VALUE_TYPE(Name, CType, Prefix)                   \
  class Name {                                                                 \
   public:                                                                     \
    /* Exact decimal-string constructor; preferred for monetary input.         \
     * Throws openpit::Error when the string is not a valid value. */          \
    explicit Name(std::string_view value) : m_value(ParseString(value)) {}     \
                                                                               \
    /* Named alias of the string constructor for call-site clarity. */         \
    [[nodiscard]] static Name FromString(std::string_view value) {             \
      return Name(value);                                                      \
    }                                                                          \
                                                                               \
    [[nodiscard]] static Name FromInt64(std::int64_t value) {                  \
      Name out;                                                                \
      OpenPitParamError* error = nullptr;                                      \
      if (!openpit_create_param_##Prefix##_from_int64(value, &out.m_value,     \
                                                      &error)) {               \
        ::openpit::detail::ThrowFromParamError(                                \
            error, "openpit_create_param_" #Prefix "_from_int64 failed");      \
      }                                                                        \
      return out;                                                              \
    }                                                                          \
                                                                               \
    [[nodiscard]] static Name FromUint64(std::uint64_t value) {                \
      Name out;                                                                \
      OpenPitParamError* error = nullptr;                                      \
      if (!openpit_create_param_##Prefix##_from_uint64(value, &out.m_value,    \
                                                       &error)) {              \
        ::openpit::detail::ThrowFromParamError(                                \
            error, "openpit_create_param_" #Prefix "_from_uint64 failed");     \
      }                                                                        \
      return out;                                                              \
    }                                                                          \
                                                                               \
    /* Boundary/test convenience only; double is imprecise and not             \
     * deterministic. Prefer FromString for monetary data. */                  \
    [[nodiscard]] static Name FromDouble(double value) {                       \
      Name out;                                                                \
      OpenPitParamError* error = nullptr;                                      \
      if (!openpit_create_param_##Prefix##_from_f64(value, &out.m_value,       \
                                                    &error)) {                 \
        ::openpit::detail::ThrowFromParamError(                                \
            error, "openpit_create_param_" #Prefix "_from_f64 failed");        \
      }                                                                        \
      return out;                                                              \
    }                                                                          \
                                                                               \
    [[nodiscard]] static Name FromStringRounded(std::string_view value,        \
                                                std::uint32_t scale,           \
                                                RoundingStrategy rounding) {   \
      Name out;                                                                \
      OpenPitParamError* error = nullptr;                                      \
      if (!openpit_create_param_##Prefix##_from_string_rounded(                \
              MakeStringView(value), scale, ToRaw(rounding), &out.m_value,     \
              &error)) {                                                       \
        ::openpit::detail::ThrowFromParamError(error,                          \
                                               "openpit_create_param_" #Prefix \
                                               "_from_string_rounded failed"); \
      }                                                                        \
      return out;                                                              \
    }                                                                          \
                                                                               \
    /* Adopts a validated C POD (e.g. read from an engine payload). */         \
    [[nodiscard]] static Name FromRaw(CType raw) noexcept {                    \
      Name out;                                                                \
      out.m_value = raw;                                                       \
      return out;                                                              \
    }                                                                          \
                                                                               \
    [[nodiscard]] CType Raw() const noexcept { return m_value; }               \
                                                                               \
    [[nodiscard]] OpenPitParamDecimal Decimal() const noexcept {               \
      return openpit_param_##Prefix##_get_decimal(m_value);                    \
    }                                                                          \
                                                                               \
    [[nodiscard]] std::string ToString() const {                               \
      OpenPitParamError* error = nullptr;                                      \
      OpenPitSharedString* handle =                                            \
          openpit_param_##Prefix##_to_string(m_value, &error);                 \
      if (handle == nullptr) {                                                 \
        ::openpit::detail::ThrowFromParamError(                                \
            error, "openpit_param_" #Prefix "_to_string failed");              \
      }                                                                        \
      std::string result = ::openpit::SharedStringView(handle).ToString();     \
      openpit_destroy_shared_string(handle);                                   \
      return result;                                                           \
    }                                                                          \
                                                                               \
    /* Boundary/test convenience only; see FromDouble. */                      \
    [[nodiscard]] double ToDouble() const {                                    \
      double result = 0.0;                                                     \
      OpenPitParamError* error = nullptr;                                      \
      if (!openpit_param_##Prefix##_to_f64(m_value, &result, &error)) {        \
        ::openpit::detail::ThrowFromParamError(                                \
            error, "openpit_param_" #Prefix "_to_f64 failed");                 \
      }                                                                        \
      return result;                                                           \
    }                                                                          \
                                                                               \
    [[nodiscard]] bool IsZero() const {                                        \
      bool result = false;                                                     \
      OpenPitParamError* error = nullptr;                                      \
      if (!openpit_param_##Prefix##_is_zero(m_value, &result, &error)) {       \
        ::openpit::detail::ThrowFromParamError(                                \
            error, "openpit_param_" #Prefix "_is_zero failed");                \
      }                                                                        \
      return result;                                                           \
    }                                                                          \
                                                                               \
    /* Returns -1, 0, or 1. */                                                 \
    [[nodiscard]] int Compare(const Name& other) const {                       \
      std::int8_t result = 0;                                                  \
      OpenPitParamError* error = nullptr;                                      \
      if (!openpit_param_##Prefix##_compare(m_value, other.m_value, &result,   \
                                            &error)) {                         \
        ::openpit::detail::ThrowFromParamError(                                \
            error, "openpit_param_" #Prefix "_compare failed");                \
      }                                                                        \
      return static_cast<int>(result);                                         \
    }                                                                          \
                                                                               \
    [[nodiscard]] bool operator==(const Name& other) const {                   \
      return Compare(other) == 0;                                              \
    }                                                                          \
    [[nodiscard]] bool operator!=(const Name& other) const {                   \
      return Compare(other) != 0;                                              \
    }                                                                          \
    [[nodiscard]] bool operator<(const Name& other) const {                    \
      return Compare(other) < 0;                                               \
    }                                                                          \
    [[nodiscard]] bool operator<=(const Name& other) const {                   \
      return Compare(other) <= 0;                                              \
    }                                                                          \
    [[nodiscard]] bool operator>(const Name& other) const {                    \
      return Compare(other) > 0;                                               \
    }                                                                          \
    [[nodiscard]] bool operator>=(const Name& other) const {                   \
      return Compare(other) >= 0;                                              \
    }                                                                          \
                                                                               \
   private:                                                                    \
    Name() = default;                                                          \
                                                                               \
    /* Parses an exact decimal value, throwing on failure. */                  \
    [[nodiscard]] static CType ParseString(std::string_view value) {           \
      CType out{};                                                             \
      OpenPitParamError* error = nullptr;                                      \
      if (!openpit_create_param_##Prefix##_from_string(MakeStringView(value),  \
                                                       &out, &error)) {        \
        ::openpit::detail::ThrowFromParamError(                                \
            error, "openpit_create_param_" #Prefix "_from_string failed");     \
      }                                                                        \
      return out;                                                              \
    }                                                                          \
                                                                               \
    CType m_value{};                                                           \
  }

// Per-unit instrument price; may be negative in some derivative markets.
OPENPIT_PARAM_DEFINE_VALUE_TYPE(Price, OpenPitParamPrice, price);
// Instrument quantity; non-negative amount in instrument units.
OPENPIT_PARAM_DEFINE_VALUE_TYPE(Quantity, OpenPitParamQuantity, quantity);
// Settlement notional volume; non-negative amount in settlement units.
OPENPIT_PARAM_DEFINE_VALUE_TYPE(Volume, OpenPitParamVolume, volume);
// Profit-and-loss contribution; may be negative.
OPENPIT_PARAM_DEFINE_VALUE_TYPE(Pnl, OpenPitParamPnl, pnl);
// Fee or rebate contribution carried by an execution report; may be negative.
OPENPIT_PARAM_DEFINE_VALUE_TYPE(Fee, OpenPitParamFee, fee);
// Signed balance/position quantity used by account adjustments and their
// bounds/outcomes; may be negative.
OPENPIT_PARAM_DEFINE_VALUE_TYPE(PositionSize, OpenPitParamPositionSize,
                                position_size);
// Cash-flow contribution; may be negative (an outflow).
OPENPIT_PARAM_DEFINE_VALUE_TYPE(CashFlow, OpenPitParamCashFlow, cash_flow);
// Monetary position exposure (|price| x quantity) used for margin and risk;
// non-negative amount in settlement units.
OPENPIT_PARAM_DEFINE_VALUE_TYPE(Notional, OpenPitParamNotional, notional);

#undef OPENPIT_PARAM_DEFINE_VALUE_TYPE

/// Signed fee-style amount paired with the currency it is denominated in.
///
/// The native runtime carries this as `OpenPitParamMonetaryAmount`: an exact
/// `Fee` value plus a borrowed currency string. The C++ wrapper owns the
/// currency string and rebuilds the borrowed C view only inside `Raw()`.
class MonetaryAmount {
 public:
  /// Builds a monetary amount from a signed amount and currency.
  MonetaryAmount(Fee amount, std::string currency)
      : m_currency(std::move(currency)), m_amount(amount) {}

  /// Adopts a native monetary amount view into owned C++ storage.
  [[nodiscard]] static MonetaryAmount FromRaw(
      const OpenPitParamMonetaryAmount& raw) {
    return MonetaryAmount(Fee::FromRaw(raw.amount),
                          ::openpit::StringView(raw.currency).ToString());
  }

  /// Maps a native optional monetary amount to `std::optional`.
  [[nodiscard]] static std::optional<MonetaryAmount> FromRawOption(
      const MonetaryAmountOptional& raw) {
    if (!raw.is_set) {
      return std::nullopt;
    }
    return FromRaw(raw.value);
  }

  /// Lowers an optional monetary amount to the native optional C payload.
  [[nodiscard]] static MonetaryAmountOptional RawOption(
      const std::optional<MonetaryAmount>& value) noexcept {
    MonetaryAmountOptional raw{};
    if (value) {
      raw.value = value->Raw();
      raw.is_set = true;
    }
    return raw;
  }

  /// Rebuilds the native borrowed C view.
  [[nodiscard]] OpenPitParamMonetaryAmount Raw() const noexcept {
    OpenPitParamMonetaryAmount raw{};
    raw.amount = m_amount.Raw();
    raw.currency = ::openpit::MakeStringView(m_currency);
    return raw;
  }

  /// Returns the signed amount.
  [[nodiscard]] Fee Amount() const noexcept { return m_amount; }

  /// Returns the owned currency code.
  [[nodiscard]] const std::string& Currency() const noexcept {
    return m_currency;
  }

  /// Reports whether both amount and currency are equal.
  [[nodiscard]] bool operator==(const MonetaryAmount& other) const {
    return m_amount == other.m_amount && m_currency == other.m_currency;
  }

  /// Reports whether either amount or currency differs.
  [[nodiscard]] bool operator!=(const MonetaryAmount& other) const {
    return !(*this == other);
  }

 private:
  std::string m_currency;
  Fee m_amount;
};

// Selects how an `AdjustmentAmount` value is interpreted. Mirrors the set
// values of `OpenPitParamAdjustmentAmountKind`; the `NotSet` variant is modeled
// as an empty `std::optional<AdjustmentAmount>`, not a value here.
enum class AdjustmentAmountKind : std::uint8_t {
  // Change the current state by the carried signed amount.
  Delta = OpenPitParamAdjustmentAmountKind_Delta,
  // Set the current state to the carried signed amount.
  Absolute = OpenPitParamAdjustmentAmountKind_Absolute,
};

// A signed account-adjustment amount tagged by kind: the carried
// `PositionSize` is interpreted as either a `Delta` (relative change) or an
// `Absolute` (target value). The native runtime carries the value as an exact
// `OpenPitParamPositionSize` plus a kind byte; the kind selects how the engine
// applies it. The value may be negative.
class AdjustmentAmount {
 public:
  // A relative change to the current component value.
  [[nodiscard]] static AdjustmentAmount OfDelta(PositionSize value) noexcept {
    return AdjustmentAmount(AdjustmentAmountKind::Delta, value);
  }

  // A new absolute value for the component.
  [[nodiscard]] static AdjustmentAmount OfAbsolute(
      PositionSize value) noexcept {
    return AdjustmentAmount(AdjustmentAmountKind::Absolute, value);
  }

  // nullopt when the C kind is `NotSet`.
  [[nodiscard]] static std::optional<AdjustmentAmount> FromRaw(
      const OpenPitParamAdjustmentAmount& raw) {
    switch (raw.kind) {
      case OpenPitParamAdjustmentAmountKind_Delta:
        return AdjustmentAmount(AdjustmentAmountKind::Delta,
                                PositionSize::FromRaw(raw.value));
      case OpenPitParamAdjustmentAmountKind_Absolute:
        return AdjustmentAmount(AdjustmentAmountKind::Absolute,
                                PositionSize::FromRaw(raw.value));
      default:
        return std::nullopt;
    }
  }

  [[nodiscard]] OpenPitParamAdjustmentAmount Raw() const noexcept {
    OpenPitParamAdjustmentAmount raw{};
    raw.value = m_value.Raw();
    raw.kind = static_cast<OpenPitParamAdjustmentAmountKind>(m_kind);
    return raw;
  }

  [[nodiscard]] AdjustmentAmountKind Kind() const noexcept { return m_kind; }

  [[nodiscard]] bool IsDelta() const noexcept {
    return m_kind == AdjustmentAmountKind::Delta;
  }

  [[nodiscard]] bool IsAbsolute() const noexcept {
    return m_kind == AdjustmentAmountKind::Absolute;
  }

  // The carried signed value, interpreted according to `Kind()`.
  [[nodiscard]] PositionSize Value() const noexcept { return m_value; }

 private:
  AdjustmentAmount(AdjustmentAmountKind kind, PositionSize value) noexcept
      : m_value(value), m_kind(kind) {}

  PositionSize m_value;
  AdjustmentAmountKind m_kind;
};

//------------------------------------------------------------------------------
// Leverage

/// \brief Fixed-point leverage multiplier transport wrapper.
//
// Fixed-point leverage multiplier (transport wrapper over the native runtime
// `OpenPitParamLeverage`, a `uint16_t` in scale `Scale`). One decimal place is
// encoded in the raw integer: `10` is `1.0x`, `11` is `1.1x`, `1005` is
// `100.5x`. The sentinel `NotSet` (raw `0`) means "leverage not specified" and
// is the value of a default-constructed `Leverage`.
//
// This wrapper performs NO business validation: out-of-range or off-step values
// are accepted as payload and rejected later by the operations that consume
// them. Absence is modeled either as the `NotSet` sentinel or, where the owning
// payload uses an optional, as an empty `std::optional<Leverage>` produced by
// `FromRawOption` / mapped back through `RawOption`.
class Leverage {
 public:
  // Fixed-point scale of the raw payload (raw == multiplier * Scale).
  static constexpr OpenPitParamLeverage Scale = OPENPIT_PARAM_LEVERAGE_SCALE;
  // Sentinel raw value meaning "leverage not set".
  static constexpr OpenPitParamLeverage NotSet = OPENPIT_PARAM_LEVERAGE_NOT_SET;
  // Minimum business leverage multiplier in whole units.
  static constexpr std::uint16_t Min = OPENPIT_PARAM_LEVERAGE_MIN;
  // Maximum business leverage multiplier in whole units.
  static constexpr std::uint16_t Max = OPENPIT_PARAM_LEVERAGE_MAX;
  // Fractional increment between adjacent leverage values.
  static constexpr float Step = OPENPIT_PARAM_LEVERAGE_STEP;

  // Default value is the `NotSet` sentinel.
  constexpr Leverage() noexcept = default;

  // Builds leverage from a whole-unit multiplier via fixed-point encoding
  // (raw = multiplier * Scale). No business validation.
  [[nodiscard]] static constexpr Leverage FromUint16(
      std::uint16_t multiplier) noexcept {
    return Leverage(static_cast<OpenPitParamLeverage>(
        static_cast<OpenPitParamLeverage>(multiplier) * Scale));
  }

  // Builds leverage from a float multiplier. Boundary/test convenience only;
  // prefer FromUint16 for exact whole-unit multipliers. Throws openpit::Error
  // when the value is not finite, is outside the native leverage range, or is
  // not aligned to the supported 0.1x step.
  [[nodiscard]] static Leverage FromFloat(float multiplier) {
    if (!std::isfinite(multiplier)) {
      throw ::openpit::Error("leverage multiplier must be finite");
    }
    const double scaled =
        static_cast<double>(multiplier) * static_cast<double>(Scale);
    const double rounded = std::round(scaled);
    if (std::fabs(scaled - rounded) > 1e-6) {
      throw ::openpit::Error("leverage multiplier must align to 0.1 step");
    }
    const long raw = static_cast<long>(rounded);
    if (raw < static_cast<long>(Min) * static_cast<long>(Scale) ||
        raw > static_cast<long>(Max) * static_cast<long>(Scale)) {
      throw ::openpit::Error("leverage multiplier out of range");
    }
    return Leverage(static_cast<OpenPitParamLeverage>(raw));
  }

  // Adopts a raw fixed-point payload (e.g. read from an engine payload). No
  // validation; `noexcept`.
  [[nodiscard]] static constexpr Leverage FromRaw(
      OpenPitParamLeverage raw) noexcept {
    return Leverage(raw);
  }

  // Maps a raw payload to an optional: the `NotSet` sentinel becomes
  // `std::nullopt`, any other value an engaged `Leverage`.
  [[nodiscard]] static constexpr std::optional<Leverage> FromRawOption(
      OpenPitParamLeverage raw) noexcept {
    if (raw == NotSet) {
      return std::nullopt;
    }
    return Leverage(raw);
  }

  // Lowers an optional back into a raw payload: `std::nullopt` becomes the
  // `NotSet` sentinel, otherwise the contained raw value.
  [[nodiscard]] static constexpr OpenPitParamLeverage RawOption(
      const std::optional<Leverage>& value) noexcept {
    return value ? value->Raw() : NotSet;
  }

  // Returns the underlying fixed-point payload. `constexpr` and `noexcept`.
  [[nodiscard]] constexpr OpenPitParamLeverage Raw() const noexcept {
    return m_value;
  }

  // Reports whether leverage is explicitly set (raw != `NotSet`).
  [[nodiscard]] constexpr bool IsSet() const noexcept {
    return m_value != NotSet;
  }

  // Returns the multiplier as a float (raw 1005 -> 100.5).
  [[nodiscard]] constexpr float Value() const noexcept {
    return static_cast<float>(m_value) / static_cast<float>(Scale);
  }

  // Renders the normalized decimal multiplier: no trailing ".0" for integer
  // multipliers, one decimal digit for fractional ones.
  [[nodiscard]] std::string ToString() const {
    const auto integer = static_cast<unsigned>(m_value / Scale);
    const auto fractional = static_cast<unsigned>(m_value % Scale);
    if (fractional == 0) {
      return std::to_string(integer);
    }
    return std::to_string(integer) + "." + std::to_string(fractional);
  }

  [[nodiscard]] constexpr bool operator==(
      const Leverage& other) const noexcept {
    return m_value == other.m_value;
  }
  [[nodiscard]] constexpr bool operator!=(
      const Leverage& other) const noexcept {
    return m_value != other.m_value;
  }

 private:
  explicit constexpr Leverage(OpenPitParamLeverage raw) noexcept
      : m_value(raw) {}

  OpenPitParamLeverage m_value = NotSet;
};

}  // namespace openpit::param
