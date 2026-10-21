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

#include "spot_loadtest/decimal.hpp"

#include <cstdint>
#include <limits>
#include <stdexcept>
#include <string>
#include <string_view>

namespace spot_loadtest {

Decimal Decimal::FromString(std::string_view text) {
  std::size_t i = 0;
  const std::size_t n = text.size();
  bool negative = false;
  if (i < n && (text[i] == '+' || text[i] == '-')) {
    negative = text[i] == '-';
    ++i;
  }
  if (i >= n) {
    throw std::invalid_argument("decimal: empty number");
  }

  // Every digit is appended to one magnitude in units of 0.01. Capping it at
  // the int64 maximum for either sign keeps the negation below defined and
  // never yields INT64_MIN, which ToString cannot negate.
  Coefficient magnitude = 0;
  const auto appendDigit = [&magnitude](Coefficient digit) {
    if (magnitude > (std::numeric_limits<Coefficient>::max() - digit) / 10) {
      throw std::invalid_argument("decimal: value out of range");
    }
    magnitude = magnitude * 10 + digit;
  };

  bool sawDigit = false;
  for (; i < n && text[i] != '.'; ++i) {
    const char c = text[i];
    if (c < '0' || c > '9') {
      throw std::invalid_argument("decimal: invalid integer digit");
    }
    appendDigit(static_cast<Coefficient>(c - '0'));
    sawDigit = true;
  }

  int fracDigits = 0;
  if (i < n && text[i] == '.') {
    ++i;
    for (; i < n; ++i) {
      const char c = text[i];
      if (c < '0' || c > '9') {
        throw std::invalid_argument("decimal: invalid fractional digit");
      }
      if (fracDigits >= 2) {
        throw std::invalid_argument("decimal: more than two fractional digits");
      }
      appendDigit(static_cast<Coefficient>(c - '0'));
      ++fracDigits;
      sawDigit = true;
    }
  }
  if (!sawDigit) {
    throw std::invalid_argument("decimal: no digits");
  }

  // Scale the fractional part up to exactly two digits (units of 0.01).
  for (; fracDigits < 2; ++fracDigits) {
    appendDigit(0);
  }
  return Decimal(negative ? -magnitude : magnitude);
}

std::int64_t Decimal::FloorDivToInt(const Decimal &divisor) const {
  if (divisor.m_coefficient == 0) {
    throw std::invalid_argument("decimal: divide by zero");
  }
  // Both operands are scale-2, so the ratio of coefficients is already the true
  // quotient. Truncation toward zero equals floor for the non-negative inputs
  // the harness produces.
  const Coefficient q = m_coefficient / divisor.m_coefficient;
  return static_cast<std::int64_t>(q);
}

std::string Decimal::ToString() const {
  Coefficient value = m_coefficient;
  const bool negative = value < 0;
  if (negative) {
    value = -value;
  }
  const Coefficient whole = value / 100;
  const int frac = static_cast<int>(value % 100);

  // Render the whole part of a (possibly 128-bit) magnitude.
  std::string digits;
  if (whole == 0) {
    digits = "0";
  } else {
    Coefficient w = whole;
    while (w > 0) {
      digits.push_back(static_cast<char>('0' + static_cast<int>(w % 10)));
      w /= 10;
    }
    for (std::size_t a = 0, b = digits.size() - 1; a < b; ++a, --b) {
      std::swap(digits[a], digits[b]);
    }
  }

  std::string out;
  if (negative) {
    out.push_back('-');
  }
  out += digits;
  if (frac != 0) {
    out.push_back('.');
    out.push_back(static_cast<char>('0' + frac / 10));
    if (frac % 10 != 0) {
      out.push_back(static_cast<char>('0' + frac % 10));
    }
  }
  return out;
}

} // namespace spot_loadtest
