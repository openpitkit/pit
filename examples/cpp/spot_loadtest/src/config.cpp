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

#include "spot_loadtest/config/config.hpp"

#include "spot_loadtest/decimal.hpp"
#include "spot_loadtest/sha256.hpp"

#include <algorithm>
#include <cctype>
#include <cmath>
#include <cstdint>
#include <fstream>
#include <limits>
#include <map>
#include <optional>
#include <set>
#include <sstream>
#include <string>
#include <utility>
#include <vector>

namespace spot_loadtest::config {
namespace {

// A parsed INI: ordered sections, each an ordered list of key=value pairs.
struct Section {
  std::string name;
  std::map<std::string, std::string> keys;
  std::map<std::string, std::size_t> keyLines;
  std::size_t line = 0;
};

struct IniFile {
  std::vector<Section> sections;

  [[nodiscard]] const Section *Find(const std::string &name) const {
    for (const Section &s : sections) {
      if (s.name == name) {
        return &s;
      }
    }
    return nullptr;
  }
};

[[nodiscard]] std::string Trim(const std::string &s) {
  std::size_t a = 0;
  std::size_t b = s.size();
  while (a < b && std::isspace(static_cast<unsigned char>(s[a]))) {
    ++a;
  }
  while (b > a && std::isspace(static_cast<unsigned char>(s[b - 1]))) {
    --b;
  }
  return s.substr(a, b - a);
}

// Strips an inline ';' or '#' comment from a value. The baseline config
// relies on inline ';' comments.
[[nodiscard]] std::string StripInlineComment(const std::string &s) {
  for (std::size_t i = 0; i < s.size(); ++i) {
    if (s[i] == ';' || s[i] == '#') {
      return s.substr(0, i);
    }
  }
  return s;
}

[[nodiscard]] std::string LineLabel(const std::string &path, std::size_t line) {
  return "config " + path + " line " + std::to_string(line);
}

[[nodiscard]] const std::set<std::string> *
AllowedKeys(const std::string &section) {
  static const std::map<std::string, std::set<std::string>> kSchema = {
      {"run",
       {"seed", "total_ops", "duration", "window", "window_unit", "observer"}},
      {"arrival", {"offered_rate"}},
      {"report_delay", {"distribution", "mean", "sigma"}},
      {"reject", {"target_rate", "tolerance"}},
      {"accounts", {"count"}},
      {"concurrency",
       {"active_accounts", "submitter_workers", "max_submit_lag"}},
      {"async_engine",
       {"strategy", "max_queues", "idle_cleanup", "sharded_workers",
        "queue_capacity", "slow_submit_threshold"}},
      {"instruments", {"symbols", "settlement"}},
      {"lifecycle", {"p_open", "p_add", "p_partial_close", "p_full_close"}},
      {"funding", {"trigger", "amount", "seed", "top_up"}},
  };
  static const std::set<std::string> kCohortKeys = {
      "weight",    "activity",     "reject_propensity",
      "burst_len", "size_weights", "symbol_skew",
      "zipf_s",
  };

  const auto schema = kSchema.find(section);
  if (schema != kSchema.end()) {
    return &schema->second;
  }
  constexpr const char *kCohortPrefix = "cohort.";
  if (section.rfind(kCohortPrefix, 0) == 0 &&
      section.size() > std::char_traits<char>::length(kCohortPrefix)) {
    return &kCohortKeys;
  }
  return nullptr;
}

void ValidateSchema(const IniFile &file, const std::string &path) {
  for (const Section &section : file.sections) {
    const std::set<std::string> *allowed = AllowedKeys(section.name);
    if (allowed == nullptr) {
      throw ConfigError(LineLabel(path, section.line) + ": unknown section [" +
                        section.name + "]");
    }
    for (const auto &[key, line] : section.keyLines) {
      if (allowed->count(key) == 0) {
        throw ConfigError(LineLabel(path, line) + " [" + section.name +
                          "]: unknown key " + key);
      }
    }
  }
}

[[nodiscard]] IniFile ParseIni(const std::string &content,
                               const std::string &path) {
  IniFile file;
  Section current;
  bool haveCurrent = false;
  std::map<std::string, std::size_t> sectionLines;
  std::istringstream in(content);
  std::string line;
  std::size_t lineNumber = 0;
  while (std::getline(in, line)) {
    ++lineNumber;
    std::string trimmed = Trim(line);
    if (trimmed.empty() || trimmed[0] == ';' || trimmed[0] == '#') {
      continue;
    }
    if (trimmed.front() == '[' && trimmed.back() == ']') {
      if (haveCurrent) {
        file.sections.push_back(std::move(current));
      }
      current = Section{};
      current.name = Trim(trimmed.substr(1, trimmed.size() - 2));
      current.line = lineNumber;
      if (current.name.empty()) {
        throw ConfigError(LineLabel(path, lineNumber) +
                          ": section name must not be empty");
      }
      const auto [first, inserted] =
          sectionLines.emplace(current.name, lineNumber);
      if (!inserted) {
        throw ConfigError(LineLabel(path, lineNumber) +
                          ": duplicate section [" + current.name +
                          "] first defined at line " +
                          std::to_string(first->second));
      }
      haveCurrent = true;
      continue;
    }
    const std::size_t eq = trimmed.find('=');
    if (eq == std::string::npos) {
      throw ConfigError(LineLabel(path, lineNumber) +
                        ": expected key = value or [section]");
    }
    if (!haveCurrent) {
      throw ConfigError(LineLabel(path, lineNumber) +
                        ": key appears before any section");
    }
    std::string key = Trim(trimmed.substr(0, eq));
    std::string value = Trim(StripInlineComment(trimmed.substr(eq + 1)));
    if (key.empty()) {
      throw ConfigError(LineLabel(path, lineNumber) + " [" + current.name +
                        "]: key name must not be empty");
    }
    const auto [first, inserted] = current.keyLines.emplace(key, lineNumber);
    if (!inserted) {
      throw ConfigError(LineLabel(path, lineNumber) + " [" + current.name +
                        "]: duplicate key " + key + " first defined at line " +
                        std::to_string(first->second));
    }
    current.keys.emplace(std::move(key), std::move(value));
  }
  if (haveCurrent) {
    file.sections.push_back(std::move(current));
  }
  ValidateSchema(file, path);
  return file;
}

// Parses an unsigned integer: a leading "0x" or "0X" is hexadecimal,
// "0o" is octal, "0b" is binary, and every other value is decimal. Returns
// nullopt on malformed input.
[[nodiscard]] std::optional<std::uint64_t> ParseUint(const std::string &raw) {
  std::string s = Trim(raw);
  if (s.empty()) {
    return std::nullopt;
  }
  int base = 10;
  std::size_t i = 0;
  if (s.size() > 2 && s[0] == '0') {
    const char c =
        static_cast<char>(std::tolower(static_cast<unsigned char>(s[1])));
    if (c == 'x') {
      base = 16;
      i = 2;
    } else if (c == 'o') {
      base = 8;
      i = 2;
    } else if (c == 'b') {
      base = 2;
      i = 2;
    }
  }
  if (i >= s.size()) {
    return std::nullopt;
  }
  std::uint64_t value = 0;
  for (; i < s.size(); ++i) {
    const char c =
        static_cast<char>(std::tolower(static_cast<unsigned char>(s[i])));
    int digit = 0;
    if (c >= '0' && c <= '9') {
      digit = c - '0';
    } else if (c >= 'a' && c <= 'f') {
      digit = 10 + (c - 'a');
    } else {
      return std::nullopt;
    }
    if (digit >= base) {
      return std::nullopt;
    }
    const std::uint64_t unsignedDigit = static_cast<std::uint64_t>(digit);
    const std::uint64_t unsignedBase = static_cast<std::uint64_t>(base);
    if (value > (std::numeric_limits<std::uint64_t>::max() - unsignedDigit) /
                    unsignedBase) {
      return std::nullopt;
    }
    value = value * unsignedBase + unsignedDigit;
  }
  return value;
}

[[nodiscard]] std::optional<std::int64_t> ParseInt(const std::string &raw) {
  std::string s = Trim(raw);
  if (s.empty()) {
    return std::nullopt;
  }
  bool negative = false;
  std::size_t i = 0;
  if (s[0] == '+' || s[0] == '-') {
    negative = s[0] == '-';
    i = 1;
  }
  if (i >= s.size()) {
    return std::nullopt;
  }
  const std::uint64_t limit =
      negative ? static_cast<std::uint64_t>(
                     std::numeric_limits<std::int64_t>::max()) +
                     1
               : static_cast<std::uint64_t>(
                     std::numeric_limits<std::int64_t>::max());
  std::uint64_t value = 0;
  for (; i < s.size(); ++i) {
    if (s[i] < '0' || s[i] > '9') {
      return std::nullopt;
    }
    const std::uint64_t digit = static_cast<std::uint64_t>(s[i] - '0');
    if (value > (limit - digit) / 10) {
      return std::nullopt;
    }
    value = value * 10 + digit;
  }
  if (!negative) {
    return static_cast<std::int64_t>(value);
  }
  if (value == limit) {
    return std::numeric_limits<std::int64_t>::min();
  }
  return -static_cast<std::int64_t>(value);
}

[[nodiscard]] std::optional<double> ParseDouble(const std::string &raw) {
  std::string s = Trim(raw);
  if (s.empty()) {
    return std::nullopt;
  }
  try {
    std::size_t pos = 0;
    const double v = std::stod(s, &pos);
    if (pos != s.size() || !std::isfinite(v)) {
      return std::nullopt;
    }
    return v;
  } catch (...) {
    return std::nullopt;
  }
}

struct DecimalNanoseconds {
  std::vector<unsigned char> fractionalDigits;
  std::uint64_t whole = 0;
};

[[nodiscard]] std::optional<std::pair<std::string, std::size_t>>
ParseDecimalDigits(const std::string &number) {
  std::string digits;
  digits.reserve(number.size());
  std::size_t scale = 0;
  bool sawDecimalPoint = false;
  for (char c : number) {
    if (c >= '0' && c <= '9') {
      digits.push_back(c);
      if (sawDecimalPoint) {
        ++scale;
      }
    } else if (c == '.' && !sawDecimalPoint) {
      sawDecimalPoint = true;
    } else {
      return std::nullopt;
    }
  }
  if (digits.empty()) {
    return std::nullopt;
  }
  return std::make_pair(std::move(digits), scale);
}

[[nodiscard]] std::string MultiplyDecimalDigits(const std::string &digits,
                                                std::uint64_t multiplier) {
  std::string product;
  product.reserve(digits.size() + 16);
  std::uint64_t carry = 0;
  for (auto it = digits.rbegin(); it != digits.rend(); ++it) {
    const std::uint64_t value =
        static_cast<std::uint64_t>(*it - '0') * multiplier + carry;
    product.push_back(static_cast<char>('0' + value % 10));
    carry = value / 10;
  }
  while (carry != 0) {
    product.push_back(static_cast<char>('0' + carry % 10));
    carry /= 10;
  }
  std::reverse(product.begin(), product.end());
  const std::size_t firstNonzero = product.find_first_not_of('0');
  return firstNonzero == std::string::npos ? "0" : product.substr(firstNonzero);
}

[[nodiscard]] bool AddDecimalNanoseconds(DecimalNanoseconds &total,
                                         const std::string &number,
                                         std::uint64_t unitNanoseconds,
                                         std::uint64_t limit) {
  const auto parsed = ParseDecimalDigits(number);
  if (!parsed) {
    return false;
  }
  const std::string product =
      MultiplyDecimalDigits(parsed->first, unitNanoseconds);
  const std::size_t scale = parsed->second;
  const std::size_t wholeDigits =
      product.size() > scale ? product.size() - scale : 0;

  const std::uint64_t remaining = limit - total.whole;
  std::uint64_t componentWhole = 0;
  for (std::size_t i = 0; i < wholeDigits; ++i) {
    const std::uint64_t digit = static_cast<std::uint64_t>(product[i] - '0');
    if (componentWhole > remaining / 10 ||
        (componentWhole == remaining / 10 && digit > remaining % 10)) {
      return false;
    }
    componentWhole = componentWhole * 10 + digit;
  }
  total.whole += componentWhole;

  if (scale == 0) {
    return true;
  }
  std::vector<unsigned char> componentFraction(scale, 0);
  const std::size_t copiedDigits = std::min(scale, product.size());
  const std::size_t productStart = product.size() - copiedDigits;
  const std::size_t fractionStart = scale - copiedDigits;
  for (std::size_t i = 0; i < copiedDigits; ++i) {
    componentFraction[fractionStart + i] =
        static_cast<unsigned char>(product[productStart + i] - '0');
  }

  total.fractionalDigits.resize(
      std::max(total.fractionalDigits.size(), componentFraction.size()), 0);
  unsigned int carry = 0;
  for (std::size_t i = total.fractionalDigits.size(); i > 0; --i) {
    const std::size_t index = i - 1;
    unsigned int digit = total.fractionalDigits[index] + carry;
    if (index < componentFraction.size()) {
      digit += componentFraction[index];
    }
    total.fractionalDigits[index] = static_cast<unsigned char>(digit % 10);
    carry = digit / 10;
  }
  if (carry != 0) {
    if (total.whole == limit) {
      return false;
    }
    ++total.whole;
  }
  return true;
}

// Parses an optional-sign sequence of decimal number/unit components. Units
// are ns, us, µ or µs, ms, s, m, and h; components may be combined. A bare 0
// is accepted, but every nonzero number needs a unit. Malformed input or a
// total outside the int64 nanosecond range returns nullopt.
[[nodiscard]] std::optional<std::chrono::nanoseconds>
ParseDuration(const std::string &raw) {
  std::string s = Trim(raw);
  if (s.empty()) {
    return std::nullopt;
  }
  bool negative = false;
  std::size_t i = 0;
  if (s[0] == '+' || s[0] == '-') {
    negative = s[0] == '-';
    i = 1;
  }
  if (i >= s.size()) {
    return std::nullopt;
  }
  if (s.substr(i) == "0") {
    return std::chrono::nanoseconds(0);
  }
  constexpr std::uint64_t kPositiveLimit =
      static_cast<std::uint64_t>(std::numeric_limits<std::int64_t>::max());
  constexpr std::uint64_t kNegativeLimit = kPositiveLimit + 1;
  const std::uint64_t limit = negative ? kNegativeLimit : kPositiveLimit;
  DecimalNanoseconds total;
  bool sawComponent = false;
  while (i < s.size()) {
    // Number (integer or decimal).
    const std::size_t numStart = i;
    while (i < s.size() && ((s[i] >= '0' && s[i] <= '9') || s[i] == '.')) {
      ++i;
    }
    if (i == numStart) {
      return std::nullopt;
    }
    const std::string numStr = s.substr(numStart, i - numStart);
    // Unit.
    const std::size_t unitStart = i;
    // Multi-byte 'µ' (U+00B5, bytes 0xC2 0xB5) for microseconds.
    std::string unit;
    if (i + 1 < s.size() && static_cast<unsigned char>(s[i]) == 0xC2 &&
        static_cast<unsigned char>(s[i + 1]) == 0xB5) {
      unit = "us";
      i += 2;
      if (i < s.size() && s[i] == 's') {
        ++i; // "µs"
      }
    } else {
      while (i < s.size() && !(s[i] >= '0' && s[i] <= '9') && s[i] != '.') {
        unit.push_back(s[i]);
        ++i;
      }
    }
    if (i == unitStart && unit.empty()) {
      return std::nullopt;
    }
    std::uint64_t unitNs = 0;
    if (unit == "ns") {
      unitNs = 1;
    } else if (unit == "us" || unit == "µs") {
      unitNs = 1'000;
    } else if (unit == "ms") {
      unitNs = 1'000'000;
    } else if (unit == "s") {
      unitNs = 1'000'000'000;
    } else if (unit == "m") {
      unitNs = 60'000'000'000;
    } else if (unit == "h") {
      unitNs = 3'600'000'000'000;
    } else {
      return std::nullopt;
    }
    if (!AddDecimalNanoseconds(total, numStr, unitNs, limit)) {
      return std::nullopt;
    }
    sawComponent = true;
  }
  if (!sawComponent) {
    return std::nullopt;
  }
  const bool hasFraction =
      std::any_of(total.fractionalDigits.begin(), total.fractionalDigits.end(),
                  [](unsigned char digit) { return digit != 0; });
  if (!negative) {
    return std::chrono::nanoseconds(static_cast<std::int64_t>(total.whole));
  }
  if (total.whole == kNegativeLimit) {
    if (hasFraction) {
      return std::nullopt;
    }
    return std::chrono::nanoseconds(std::numeric_limits<std::int64_t>::min());
  }
  return std::chrono::nanoseconds(-static_cast<std::int64_t>(total.whole));
}

[[nodiscard]] std::optional<Decimal>
ParsePositiveDecimal(const std::string &raw) {
  try {
    const Decimal d = Decimal::FromString(Trim(raw));
    if (!d.IsPositive()) {
      return std::nullopt;
    }
    return d;
  } catch (...) {
    return std::nullopt;
  }
}

// Reads a required key from a section, throwing on absence.
[[nodiscard]] const std::string &RequireKey(const Section &sec,
                                            const std::string &sectionLabel,
                                            const std::string &key) {
  auto it = sec.keys.find(key);
  if (it == sec.keys.end()) {
    throw ConfigError(sectionLabel + ": key " + key + " is required");
  }
  return it->second;
}

[[nodiscard]] const Section &RequireSection(const IniFile &file,
                                            const std::string &path,
                                            const std::string &name) {
  const Section *sec = file.Find(name);
  if (sec == nullptr) {
    throw ConfigError("config " + path + ": section [" + name +
                      "] is required");
  }
  return *sec;
}

[[nodiscard]] double RequireFloat(const Section &sec, const std::string &label,
                                  const std::string &key) {
  const std::string &raw = RequireKey(sec, label, key);
  std::optional<double> v = ParseDouble(raw);
  if (!v) {
    throw ConfigError(label + ": " + key + ": must be a number, got \"" + raw +
                      "\"");
  }
  return *v;
}

[[nodiscard]] double RequireUnitFloat(const Section &sec,
                                      const std::string &label,
                                      const std::string &key) {
  const double v = RequireFloat(sec, label, key);
  if (v < 0 || v > 1) {
    throw ConfigError(label + ": " + key + ": must be in [0, 1]");
  }
  return v;
}

[[nodiscard]] double RequirePositiveFloat(const Section &sec,
                                          const std::string &label,
                                          const std::string &key) {
  const double v = RequireFloat(sec, label, key);
  if (v <= 0) {
    throw ConfigError(label + ": " + key + ": must be > 0");
  }
  return v;
}

void LoadRun(const IniFile &file, const std::string &path, Run &r) {
  const Section &sec = RequireSection(file, path, "run");
  const std::string label = "config " + path + " [run]";

  std::optional<std::uint64_t> seed = ParseUint(RequireKey(sec, label, "seed"));
  if (!seed) {
    throw ConfigError(label + ": seed: must be a non-negative integer");
  }
  r.seed = *seed;

  if (sec.keys.count("duration") != 0) {
    throw ConfigError(
        label + ": duration: unsupported; use total_ops as the run bound");
  }
  std::optional<std::uint64_t> totalOps =
      ParseUint(RequireKey(sec, label, "total_ops"));
  if (!totalOps || *totalOps == 0) {
    throw ConfigError(label + ": total_ops: must be a positive integer");
  }
  r.totalOps = *totalOps;

  std::optional<std::uint64_t> window =
      ParseUint(RequireKey(sec, label, "window"));
  if (!window || *window == 0) {
    throw ConfigError(label + ": window: must be a positive integer");
  }
  if (*window >
      static_cast<std::uint64_t>(std::numeric_limits<std::int64_t>::max())) {
    throw ConfigError(label + ": window: must be <= " +
                      std::to_string(std::numeric_limits<std::int64_t>::max()));
  }
  r.window = *window;

  const std::string wu = Trim(RequireKey(sec, label, "window_unit"));
  if (wu == "wall") {
    throw ConfigError(label + ": window_unit: wall is unsupported; use ops");
  }
  if (wu != "ops") {
    throw ConfigError(label + ": window_unit: must be ops");
  }

  const std::string obs = Trim(RequireKey(sec, label, "observer"));
  if (obs == "on") {
    r.observer = true;
  } else if (obs == "off") {
    r.observer = false;
  } else {
    throw ConfigError(label + ": observer: must be on or off");
  }
}

void LoadArrival(const IniFile &file, const std::string &path, Arrival &a) {
  const Section *sec = file.Find("arrival");
  if (sec == nullptr) {
    return;
  }
  const std::string label = "config " + path + " [arrival]";
  auto it = sec->keys.find("offered_rate");
  if (it != sec->keys.end()) {
    const std::optional<std::uint64_t> n = ParseUint(it->second);
    if (!n) {
      throw ConfigError(label +
                        ": offered_rate: must be a non-negative integer");
    }
    a.offeredRate = *n;
  }
}

void LoadReportDelay(const IniFile &file, const std::string &path,
                     ReportDelay &d) {
  const Section *sec = file.Find("report_delay");
  if (sec == nullptr) {
    return;
  }
  const std::string label = "config " + path + " [report_delay]";
  const auto distributionIt = sec->keys.find("distribution");
  const auto meanIt = sec->keys.find("mean");
  const auto sigmaIt = sec->keys.find("sigma");
  const bool hasDistribution = distributionIt != sec->keys.end();
  const bool hasMean = meanIt != sec->keys.end();
  const bool hasSigma = sigmaIt != sec->keys.end();
  if (!hasDistribution && (hasMean || hasSigma)) {
    throw ConfigError(label +
                      ": distribution: required when mean or sigma is set");
  }
  if (hasDistribution) {
    const std::string v = Trim(distributionIt->second);
    if (v == "none") {
      d.distribution = ReportDelayDistribution::None;
    } else if (v == "lognormal") {
      d.distribution = ReportDelayDistribution::Lognormal;
    } else if (v == "fixed") {
      d.distribution = ReportDelayDistribution::Fixed;
    } else {
      throw ConfigError(label +
                        ": distribution: must be none, lognormal, or fixed");
    }
  }
  if (hasMean) {
    const std::optional<std::chrono::nanoseconds> parsedMean =
        ParseDuration(meanIt->second);
    if (!parsedMean) {
      throw ConfigError(label +
                        ": mean: must be a non-negative duration (e.g. 2ms)");
    }
    if (parsedMean->count() < 0) {
      throw ConfigError(label + ": mean: must be >= 0");
    }
    d.mean = *parsedMean;
  }
  if (hasSigma) {
    const std::optional<double> parsedSigma = ParseDouble(sigmaIt->second);
    if (!parsedSigma) {
      throw ConfigError(label + ": sigma: must be a finite number >= 0");
    }
    if (*parsedSigma < 0) {
      throw ConfigError(label + ": sigma: must be >= 0");
    }
    d.sigma = *parsedSigma;
  }
  if (d.distribution == ReportDelayDistribution::None &&
      (hasMean || hasSigma)) {
    throw ConfigError(
        label + ": mean and sigma: must be omitted when distribution is none");
  }
  if (d.distribution == ReportDelayDistribution::Fixed && hasSigma) {
    throw ConfigError(label +
                      ": sigma: must be omitted when distribution is fixed");
  }
  if (d.distribution == ReportDelayDistribution::Lognormal && hasSigma &&
      d.mean.count() == 0) {
    throw ConfigError(label +
                      ": sigma: must be omitted when lognormal mean is zero");
  }
}

void LoadReject(const IniFile &file, const std::string &path, Reject &r) {
  const Section &sec = RequireSection(file, path, "reject");
  const std::string label = "config " + path + " [reject]";
  const double rate = RequireUnitFloat(sec, label, "target_rate");
  if (rate >= 1) {
    throw ConfigError(label + ": target_rate: must be < 1");
  }
  r.targetRate = rate;
  const double tol = RequireFloat(sec, label, "tolerance");
  if (tol <= 0 || tol > 1) {
    throw ConfigError(label + ": tolerance: must be in (0, 1]");
  }
  r.tolerance = tol;
}

void LoadAccounts(const IniFile &file, const std::string &path, Accounts &a) {
  const Section &sec = RequireSection(file, path, "accounts");
  const std::string label = "config " + path + " [accounts]";
  std::optional<std::uint64_t> n = ParseUint(RequireKey(sec, label, "count"));
  if (!n || *n == 0) {
    throw ConfigError(label + ": count: must be a positive integer");
  }
  a.count = *n;
}

void LoadConcurrency(const IniFile &file, const std::string &path,
                     Concurrency &c, std::uint64_t population) {
  const Section &sec = RequireSection(file, path, "concurrency");
  const std::string label = "config " + path + " [concurrency]";
  std::optional<std::uint64_t> n =
      ParseUint(RequireKey(sec, label, "active_accounts"));
  if (!n || *n == 0) {
    throw ConfigError(label + ": active_accounts: must be a positive integer");
  }
  if (*n > population) {
    throw ConfigError(label + ": active_accounts: " + std::to_string(*n) +
                      " exceeds accounts.count " + std::to_string(population) +
                      " (active set cannot exceed the population)");
  }
  c.activeAccounts = *n;

  std::optional<std::uint64_t> submitterWorkers =
      ParseUint(RequireKey(sec, label, "submitter_workers"));
  if (!submitterWorkers || *submitterWorkers == 0) {
    throw ConfigError(label +
                      ": submitter_workers: must be a positive integer");
  }
  if (*submitterWorkers > c.activeAccounts) {
    throw ConfigError(
        label + ": submitter_workers: " + std::to_string(*submitterWorkers) +
        " exceeds active_accounts " + std::to_string(c.activeAccounts));
  }
  c.submitterWorkers = *submitterWorkers;

  std::optional<std::chrono::nanoseconds> maxSubmitLag =
      ParseDuration(RequireKey(sec, label, "max_submit_lag"));
  if (!maxSubmitLag || maxSubmitLag->count() <= 0) {
    throw ConfigError(label +
                      ": max_submit_lag: must be a positive finite duration");
  }
  c.maxSubmitLag = *maxSubmitLag;
}

void LoadAsyncEngine(const IniFile &file, const std::string &path,
                     AsyncEngine &e, std::uint64_t activeAccounts) {
  const Section &sec = RequireSection(file, path, "async_engine");
  const std::string label = "config " + path + " [async_engine]";

  const std::string strat = Trim(RequireKey(sec, label, "strategy"));
  if (strat == "dynamic") {
    e.strategy = AsyncEngineStrategy::Dynamic;
  } else if (strat == "sharded") {
    e.strategy = AsyncEngineStrategy::Sharded;
  } else {
    throw ConfigError(label + ": strategy: must be \"dynamic\" or \"sharded\"");
  }

  std::optional<std::uint64_t> mq =
      ParseUint(RequireKey(sec, label, "max_queues"));
  if (!mq) {
    throw ConfigError(label + ": max_queues: must be a non-negative integer "
                              "(0 = unlimited)");
  }
  if (e.strategy == AsyncEngineStrategy::Dynamic) {
    if (*mq != 0 && *mq < activeAccounts) {
      throw ConfigError(
          label + ": max_queues: " + std::to_string(*mq) +
          " must be 0 (unlimited) or >= concurrency.active_accounts " +
          std::to_string(activeAccounts) + " (dynamic strategy)");
    }
  }
  e.maxQueues = *mq;

  const std::string &icRaw = RequireKey(sec, label, "idle_cleanup");
  std::optional<std::chrono::nanoseconds> idle = ParseDuration(icRaw);
  if (!idle) {
    throw ConfigError(label + ": idle_cleanup: must be a duration (e.g. 5s)");
  }
  if (idle->count() < 0) {
    throw ConfigError(label + ": idle_cleanup: must be >= 0 (0 = disabled)");
  }
  e.idleCleanup = *idle;

  std::optional<std::int64_t> sw =
      ParseInt(RequireKey(sec, label, "sharded_workers"));
  if (!sw) {
    throw ConfigError(label +
                      ": sharded_workers: must be a non-negative integer");
  }
  if (*sw < 0 ||
      *sw > static_cast<std::int64_t>(std::numeric_limits<int>::max())) {
    throw ConfigError(label + ": sharded_workers: must fit a non-negative int");
  }
  if (e.strategy == AsyncEngineStrategy::Sharded && *sw <= 0) {
    throw ConfigError(label +
                      ": sharded_workers: must be > 0 when strategy = sharded");
  }
  e.shardedWorkers = static_cast<int>(*sw);

  std::optional<std::int64_t> qc =
      ParseInt(RequireKey(sec, label, "queue_capacity"));
  if (!qc) {
    throw ConfigError(label +
                      ": queue_capacity: must be a non-negative integer "
                      "(0 = engine default 1024)");
  }
  if (*qc < 0) {
    throw ConfigError(label + ": queue_capacity: must be >= 0");
  }
  if (*qc > static_cast<std::int64_t>(std::numeric_limits<int>::max())) {
    throw ConfigError(label + ": queue_capacity: must fit an int");
  }
  e.queueCapacity = static_cast<int>(*qc);

  std::string sstRaw = Trim(RequireKey(sec, label, "slow_submit_threshold"));
  if (sstRaw == "0") {
    sstRaw = "0s";
  }
  std::optional<std::chrono::nanoseconds> sst = ParseDuration(sstRaw);
  if (!sst) {
    throw ConfigError(label +
                      ": slow_submit_threshold: must be a duration (e.g. 1m) "
                      "or 0 (engine default)");
  }
  if (sst->count() < 0) {
    throw ConfigError(label + ": slow_submit_threshold: must be >= 0 "
                              "(0 = engine default 1m)");
  }
  e.slowSubmitThreshold = *sst;
}

void LoadInstruments(const IniFile &file, const std::string &path,
                     Instruments &inst) {
  const Section &sec = RequireSection(file, path, "instruments");
  const std::string label = "config " + path + " [instruments]";
  const std::string &raw = RequireKey(sec, label, "symbols");

  std::vector<std::string> symbols;
  std::map<std::string, bool> seen;
  std::stringstream ss(raw);
  std::string part;
  int idx = 0;
  while (std::getline(ss, part, ',')) {
    ++idx;
    const std::string s = Trim(part);
    if (s.empty()) {
      throw ConfigError(label + ": symbols: entry " + std::to_string(idx) +
                        " is blank");
    }
    if (seen.count(s) != 0) {
      throw ConfigError(label + ": symbols: duplicate entry \"" + s + "\"");
    }
    seen[s] = true;
    symbols.push_back(s);
  }
  if (symbols.empty()) {
    throw ConfigError(label + ": symbols: must list at least one instrument");
  }
  inst.symbols = std::move(symbols);

  std::string settlement = "USD";
  if (auto it = sec.keys.find("settlement"); it != sec.keys.end()) {
    settlement = Trim(it->second);
    if (settlement.empty()) {
      throw ConfigError(label + ": settlement: must be a non-empty asset code");
    }
  }
  if (seen.count(settlement) != 0) {
    throw ConfigError(label + ": settlement \"" + settlement +
                      "\" must not also be an underlying symbol");
  }
  inst.settlement = settlement;
}

void LoadLifecycle(const IniFile &file, const std::string &path,
                   Lifecycle &lc) {
  const Section &sec = RequireSection(file, path, "lifecycle");
  const std::string label = "config " + path + " [lifecycle]";
  lc.pOpen = RequireUnitFloat(sec, label, "p_open");
  lc.pAdd = RequireUnitFloat(sec, label, "p_add");
  lc.pPartialClose = RequireUnitFloat(sec, label, "p_partial_close");
  lc.pFullClose = RequireUnitFloat(sec, label, "p_full_close");

  constexpr double kSumTolerance = 4.0 * std::numeric_limits<double>::epsilon();
  const double transitionSum = lc.pAdd + lc.pPartialClose + lc.pFullClose;
  if (transitionSum > 1.0 + kSumTolerance) {
    throw ConfigError(label +
                      ": p_add + p_partial_close + p_full_close: must be <= 1");
  }
}

void LoadFunding(const IniFile &file, const std::string &path, Funding &fd) {
  const Section &sec = RequireSection(file, path, "funding");
  const std::string label = "config " + path + " [funding]";
  const std::string trig = Trim(RequireKey(sec, label, "trigger"));
  if (trig != "balance_below") {
    throw ConfigError(label + ": trigger: must be \"balance_below\"");
  }
  fd.trigger = FundingTrigger::BalanceBelow;

  const std::string &amountRaw = RequireKey(sec, label, "amount");
  std::optional<Decimal> threshold = ParsePositiveDecimal(amountRaw);
  if (!threshold) {
    throw ConfigError(label + ": amount: must be a decimal > 0");
  }
  fd.threshold = *threshold;

  fd.seed = *threshold;
  if (auto it = sec.keys.find("seed"); it != sec.keys.end()) {
    std::optional<Decimal> v = ParsePositiveDecimal(it->second);
    if (!v) {
      throw ConfigError(label + ": seed: must be a decimal > 0");
    }
    fd.seed = *v;
  }
  fd.topUp = *threshold;
  if (auto it = sec.keys.find("top_up"); it != sec.keys.end()) {
    std::optional<Decimal> v = ParsePositiveDecimal(it->second);
    if (!v) {
      throw ConfigError(label + ": top_up: must be a decimal > 0");
    }
    fd.topUp = *v;
  }
}

[[nodiscard]] std::vector<SizeBucket>
ParseSizeWeights(const Section &sec, const std::string &label) {
  const std::string &raw = RequireKey(sec, label, "size_weights");
  std::vector<SizeBucket> buckets;
  std::stringstream ss(raw);
  std::string part;
  int idx = 0;
  while (std::getline(ss, part, ',')) {
    ++idx;
    const std::string p = Trim(part);
    if (p.empty()) {
      throw ConfigError(label + ": size_weights: entry " + std::to_string(idx) +
                        " is blank");
    }
    const std::size_t colon = p.find(':');
    if (colon == std::string::npos) {
      throw ConfigError(label + ": size_weights: entry \"" + p +
                        "\" must be qty:weight");
    }
    std::optional<std::uint64_t> qty = ParseUint(Trim(p.substr(0, colon)));
    if (!qty || *qty == 0) {
      throw ConfigError(label + ": size_weights: quantity in \"" + p +
                        "\" must be a positive integer");
    }
    std::optional<double> w = ParseDouble(Trim(p.substr(colon + 1)));
    if (!w || *w <= 0) {
      throw ConfigError(label + ": size_weights: weight in \"" + p +
                        "\" must be a positive number");
    }
    buckets.push_back(SizeBucket{*qty, *w});
  }
  if (buckets.empty()) {
    throw ConfigError(label +
                      ": size_weights: must list at least one qty:weight "
                      "bucket");
  }
  return buckets;
}

[[nodiscard]] Cohort ParseCohort(const Section &sec, const std::string &name) {
  const std::string label = "[cohort." + name + "]";
  Cohort c;
  c.name = name;
  c.weight = RequirePositiveFloat(sec, label, "weight");
  c.activity = RequireUnitFloat(sec, label, "activity");
  c.rejectPropensity = RequireUnitFloat(sec, label, "reject_propensity");

  std::optional<std::uint64_t> burst =
      ParseUint(RequireKey(sec, label, "burst_len"));
  if (!burst || *burst == 0) {
    throw ConfigError(label + ": burst_len: must be a positive integer");
  }
  c.burstLen = *burst;

  c.sizeWeights = ParseSizeWeights(sec, label);

  const std::string skew = Trim(RequireKey(sec, label, "symbol_skew"));
  if (skew == "uniform") {
    c.symbolSkew = SymbolSkew::Uniform;
  } else if (skew == "zipf") {
    c.symbolSkew = SymbolSkew::Zipf;
    const double s = RequireFloat(sec, label, "zipf_s");
    if (s <= 1) {
      throw ConfigError(label + ": zipf_s: must be > 1");
    }
    c.zipfS = s;
  } else {
    throw ConfigError(label + ": symbol_skew: must be \"uniform\" or \"zipf\"");
  }
  return c;
}

void LoadCohorts(const IniFile &file, const std::string &path, Config &cfg) {
  std::vector<Cohort> cohorts;
  for (const Section &sec : file.sections) {
    const std::string prefix = "cohort.";
    if (sec.name.rfind(prefix, 0) != 0) {
      continue;
    }
    const std::string name = Trim(sec.name.substr(prefix.size()));
    if (name.empty()) {
      throw ConfigError("config " + path +
                        ": [cohort.]: cohort name must not be empty");
    }
    cohorts.push_back(ParseCohort(sec, name));
  }
  if (cohorts.empty()) {
    throw ConfigError("config " + path +
                      ": at least one [cohort.<name>] section is required");
  }
  std::sort(cohorts.begin(), cohorts.end(),
            [](const Cohort &a, const Cohort &b) { return a.name < b.name; });
  cfg.cohorts = std::move(cohorts);
}

} // namespace

std::string ToString(AsyncEngineStrategy strategy) {
  return strategy == AsyncEngineStrategy::Dynamic ? "dynamic" : "sharded";
}

Config LoadFromString(const std::string &content, const std::string &path,
                      const std::string &hash) {
  const IniFile file = ParseIni(content, path);

  Config cfg;
  cfg.path = path;
  cfg.hash = hash;

  LoadRun(file, path, cfg.run);
  LoadArrival(file, path, cfg.arrival);
  LoadReportDelay(file, path, cfg.reportDelay);
  LoadReject(file, path, cfg.reject);
  LoadAccounts(file, path, cfg.accounts);
  LoadConcurrency(file, path, cfg.concurrency, cfg.accounts.count);
  LoadAsyncEngine(file, path, cfg.asyncEngine, cfg.concurrency.activeAccounts);
  LoadInstruments(file, path, cfg.instruments);
  LoadLifecycle(file, path, cfg.lifecycle);
  LoadFunding(file, path, cfg.funding);
  LoadCohorts(file, path, cfg);

  return cfg;
}

Config Load(const std::string &path) {
  std::ifstream in(path, std::ios::binary);
  if (!in) {
    throw ConfigError("config: read \"" + path + "\": cannot open file");
  }
  std::ostringstream buffer;
  buffer << in.rdbuf();
  const std::string content = buffer.str();
  const std::string hash = Sha256Hex(content);
  return LoadFromString(content, path, hash);
}

} // namespace spot_loadtest::config
