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

#include "duration.hpp"

#include <cctype>
#include <cmath>
#include <cstdint>
#include <iomanip>
#include <sstream>

namespace spot_table {
namespace {

using Nanos = std::chrono::nanoseconds;

constexpr std::int64_t kNanosPerMicro = 1000LL;
constexpr std::int64_t kNanosPerMilli = 1000LL * 1000LL;
constexpr std::int64_t kNanosPerSecond = 1000LL * 1000LL * 1000LL;
constexpr std::int64_t kNanosPerMinute = 60 * kNanosPerSecond;
constexpr std::int64_t kNanosPerHour = 60 * kNanosPerMinute;

// Renders a fractional unit value while trimming trailing zeros.
[[nodiscard]] std::string FormatFloat(std::int64_t value, std::int64_t unit,
                                      const char *suffix) {
  const std::int64_t whole = value / unit;
  const std::int64_t frac = value % unit;
  std::ostringstream out;
  out << whole;
  if (frac != 0) {
    // Print the fraction zero-padded to the unit's digit width, then trim.
    int width = 0;
    for (std::int64_t u = unit / 10; u >= 1; u /= 10) {
      ++width;
    }
    std::ostringstream frac_out;
    frac_out << std::setw(width) << std::setfill('0') << frac;
    std::string digits = frac_out.str();
    while (!digits.empty() && digits.back() == '0') {
      digits.pop_back();
    }
    if (!digits.empty()) {
      out << "." << digits;
    }
  }
  out << suffix;
  return out.str();
}

} // namespace

std::string FormatDuration(Nanos d) {
  std::int64_t ns = d.count();
  if (ns == 0) {
    return "0s";
  }
  const bool negative = ns < 0;
  if (negative) {
    ns = -ns;
  }

  std::string out;
  if (ns < kNanosPerSecond) {
    // Sub-second: pick the largest unit that yields a value >= 1.
    if (ns < kNanosPerMicro) {
      out = std::to_string(ns) + "ns";
    } else if (ns < kNanosPerMilli) {
      out = FormatFloat(ns, kNanosPerMicro, "µs");
    } else {
      out = FormatFloat(ns, kNanosPerMilli, "ms");
    }
  } else {
    std::ostringstream stream;
    const std::int64_t hours = ns / kNanosPerHour;
    ns %= kNanosPerHour;
    const std::int64_t minutes = ns / kNanosPerMinute;
    ns %= kNanosPerMinute;
    if (hours > 0) {
      stream << hours << "h";
    }
    if (hours > 0 || minutes > 0) {
      stream << minutes << "m";
    }
    stream << FormatFloat(ns, kNanosPerSecond, "s");
    out = stream.str();
  }
  return negative ? "-" + out : out;
}

Nanos RoundDuration(Nanos d, Nanos unit) {
  if (unit <= Nanos(0)) {
    return d;
  }
  const std::int64_t u = unit.count();
  const std::int64_t v = d.count();
  const std::int64_t r = v % u;
  // Compare the absolute remainder to half the rounding unit.
  const std::int64_t absR = r < 0 ? -r : r;
  std::int64_t rounded = v - r;
  if (absR + absR >= u) {
    rounded += (v < 0 ? -u : u);
  }
  return Nanos(rounded);
}

bool ParseDuration(const std::string &text, Nanos &out, std::string &err) {
  if (text.empty()) {
    err = "empty duration";
    return false;
  }
  std::size_t i = 0;
  bool negative = false;
  if (text[i] == '+' || text[i] == '-') {
    negative = text[i] == '-';
    ++i;
  }
  if (i >= text.size()) {
    err = "invalid duration \"" + text + "\"";
    return false;
  }
  // Accept "0" without a unit as zero duration.
  if (text.substr(i) == "0") {
    out = Nanos(0);
    return true;
  }

  long double total = 0.0L;
  bool sawComponent = false;
  while (i < text.size()) {
    // Parse the numeric magnitude (integer or fractional).
    std::size_t numStart = i;
    while (i < text.size() &&
           (std::isdigit(static_cast<unsigned char>(text[i])) != 0 ||
            text[i] == '.')) {
      ++i;
    }
    if (i == numStart) {
      err = "invalid duration \"" + text + "\"";
      return false;
    }
    long double magnitude = 0.0L;
    try {
      magnitude = std::stold(text.substr(numStart, i - numStart));
    } catch (...) {
      err = "invalid duration \"" + text + "\"";
      return false;
    }

    // Parse the unit.
    std::size_t unitStart = i;
    while (i < text.size() &&
           std::isalpha(static_cast<unsigned char>(text[i])) == 0 &&
           text[i] != '\xc2' && text[i] != '\xb5') {
      // stop: units are alphabetic / the µ bytes
      break;
    }
    while (i < text.size() &&
           (std::isalpha(static_cast<unsigned char>(text[i])) != 0 ||
            static_cast<unsigned char>(text[i]) == 0xC2 ||
            static_cast<unsigned char>(text[i]) == 0xB5)) {
      ++i;
    }
    const std::string unit = text.substr(unitStart, i - unitStart);
    long double unitNanos = 0.0L;
    if (unit == "ns") {
      unitNanos = 1.0L;
    } else if (unit == "us" || unit == "µs" || unit == "\xb5s") {
      unitNanos = static_cast<long double>(kNanosPerMicro);
    } else if (unit == "ms") {
      unitNanos = static_cast<long double>(kNanosPerMilli);
    } else if (unit == "s") {
      unitNanos = static_cast<long double>(kNanosPerSecond);
    } else if (unit == "m") {
      unitNanos = static_cast<long double>(kNanosPerMinute);
    } else if (unit == "h") {
      unitNanos = static_cast<long double>(kNanosPerHour);
    } else {
      err = "unknown unit \"" + unit + "\" in duration \"" + text + "\"";
      return false;
    }
    total += magnitude * unitNanos;
    sawComponent = true;
  }
  if (!sawComponent) {
    err = "invalid duration \"" + text + "\"";
    return false;
  }
  if (negative) {
    total = -total;
  }
  out = Nanos(static_cast<std::int64_t>(std::llround(total)));
  return true;
}

} // namespace spot_table
