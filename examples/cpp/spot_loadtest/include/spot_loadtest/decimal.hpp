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

#include <cstdint>
#include <stdexcept>
#include <string>
#include <string_view>

// Exact fixed-point money for the shadow ledger.
//
// The Go harness uses shopspring/decimal; here the value space is deliberately
// restricted exactly as the Go `money.go` is — quantity is an integer lot
// count (scale 0) and price has at most `priceScale = 2` fractional digits
// (classic equity ticks) — so every monetary value is exact at scale 2 and
// fits in a signed 128-bit fixed-point coefficient. This mirrors `q*p` being
// exact with no rounding, which is what keeps the shadow ledger bit-for-bit
// against the engine.
//
// `Decimal` carries a scale-2 integer coefficient (units of 0.01). The engine
// value types (`openpit::param`) are constructed from `ToString()`, so the
// crossing into the engine stays exact and deterministic across bindings.

namespace spot_loadtest {

// A scale-2 fixed-point decimal: the value is `coefficient / 100`.
class Decimal {
public:
  // Signed coefficient in units of 0.01 (scale 2).
  using Coefficient = std::int64_t;

  constexpr Decimal() noexcept = default;

  // Builds a scale-2 decimal from an integer count of whole units.
  [[nodiscard]] static Decimal FromInt(std::int64_t whole) noexcept {
    return Decimal(static_cast<Coefficient>(whole) * 100);
  }

  // Builds a scale-2 decimal from an integer count of cents (units of 0.01).
  [[nodiscard]] static Decimal FromCents(std::int64_t cents) noexcept {
    return Decimal(static_cast<Coefficient>(cents));
  }

  // Parses a decimal literal with at most two fractional digits (the pinned
  // price/amount value space). Throws std::invalid_argument otherwise.
  [[nodiscard]] static Decimal FromString(std::string_view text);

  [[nodiscard]] Coefficient RawCoefficient() const noexcept {
    return m_coefficient;
  }

  [[nodiscard]] bool IsZero() const noexcept { return m_coefficient == 0; }
  [[nodiscard]] bool IsPositive() const noexcept { return m_coefficient > 0; }
  [[nodiscard]] bool IsNegative() const noexcept { return m_coefficient < 0; }

  [[nodiscard]] Decimal operator+(const Decimal &other) const noexcept {
    return Decimal(m_coefficient + other.m_coefficient);
  }
  [[nodiscard]] Decimal operator-(const Decimal &other) const noexcept {
    return Decimal(m_coefficient - other.m_coefficient);
  }
  [[nodiscard]] Decimal operator-() const noexcept {
    return Decimal(-m_coefficient);
  }

  // Exact multiply: q (scale 0 integer lots) times this scale-2 value. Used for
  // the only charge formula in the harness, `q*p`, which is exact because q is
  // an integer lot count and p has at most two fractional digits.
  [[nodiscard]] Decimal MulInt(std::int64_t lots) const noexcept {
    return Decimal(m_coefficient * static_cast<Coefficient>(lots));
  }

  // Full scale-2 decimal multiply, the analogue of shopspring decimal.Mul. Both
  // operands carry a scale-2 coefficient (value = coeff/100), so the true
  // product is (c1*c2)/10000; rendered back at scale 2 the coefficient is
  // (c1*c2)/100. In the harness value space (one factor is an integer lot
  // count, the other a price with at most two fractional digits) the product
  // has at most two fractional digits, so c1*c2 is always a multiple of 100 and
  // the division is exact with no rounding. Unlike MulInt this does NOT
  // truncate a fractional factor first, so it stays correct for any
  // pinned-scale inputs.
  [[nodiscard]] Decimal Mul(const Decimal &other) const noexcept {
    return Decimal(m_coefficient * other.m_coefficient / 100);
  }

  // Floor division by another scale-2 decimal, returning the integer quotient.
  // Used to cap or oversize a buy to the available balance. Truncates toward
  // negative infinity for non-negative inputs (the only case the harness uses).
  [[nodiscard]] std::int64_t FloorDivToInt(const Decimal &divisor) const;

  // The whole-unit integer value, dropping any fractional part. Used where a
  // value is known to be a scale-0 integer (e.g. an integer lot count).
  [[nodiscard]] std::int64_t ToWholeInt() const noexcept {
    return static_cast<std::int64_t>(m_coefficient / 100);
  }

  [[nodiscard]] bool operator==(const Decimal &o) const noexcept {
    return m_coefficient == o.m_coefficient;
  }
  [[nodiscard]] bool operator!=(const Decimal &o) const noexcept {
    return m_coefficient != o.m_coefficient;
  }
  [[nodiscard]] bool operator<(const Decimal &o) const noexcept {
    return m_coefficient < o.m_coefficient;
  }
  [[nodiscard]] bool operator<=(const Decimal &o) const noexcept {
    return m_coefficient <= o.m_coefficient;
  }
  [[nodiscard]] bool operator>(const Decimal &o) const noexcept {
    return m_coefficient > o.m_coefficient;
  }
  [[nodiscard]] bool operator>=(const Decimal &o) const noexcept {
    return m_coefficient >= o.m_coefficient;
  }

  // Canonical string form: no trailing zeros, mirroring decimal.String() for
  // the value space the harness uses (integers render without a fractional
  // part, scale-2 values without trailing zeros). Crossed into the engine value
  // types verbatim so the construction stays exact.
  [[nodiscard]] std::string ToString() const;

private:
  explicit Decimal(Coefficient coefficient) noexcept
      : m_coefficient(coefficient) {}

  Coefficient m_coefficient = 0;
};

} // namespace spot_loadtest
