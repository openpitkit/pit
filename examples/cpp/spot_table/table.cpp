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

#include "table.hpp"

#include <algorithm>
#include <cctype>
#include <cstdlib>
#include <fstream>
#include <initializer_list>
#include <sstream>
#include <utility>

namespace spot_table {
namespace {

// Headers every table must declare, in any order and case. The remaining
// recognized columns are not checked here; a missing one reads as an empty
// cell, so per-action validation still rejects a row whose action needs it -
// a TICK without an instrument or price column, a SEED without asset or
// amount.
const std::vector<std::string> kRequiredHeaders = {"account", "action",
                                                   "expect"};

enum class State {
  Start,
  FrontMatter,
  Body,
  AwaitDivider,
  Rows,
  Done,
};

[[nodiscard]] std::string TrimSpace(const std::string &s) {
  const auto is_space = [](unsigned char c) { return std::isspace(c) != 0; };
  std::size_t begin = 0;
  std::size_t end = s.size();
  while (begin < end && is_space(static_cast<unsigned char>(s[begin]))) {
    ++begin;
  }
  while (end > begin && is_space(static_cast<unsigned char>(s[end - 1]))) {
    --end;
  }
  return s.substr(begin, end - begin);
}

[[nodiscard]] std::string ToUpper(const std::string &s) {
  std::string out = s;
  std::transform(out.begin(), out.end(), out.begin(), [](unsigned char c) {
    return static_cast<char>(std::toupper(c));
  });
  return out;
}

[[nodiscard]] bool EqualFold(const std::string &a, const std::string &b) {
  if (a.size() != b.size()) {
    return false;
  }
  for (std::size_t i = 0; i < a.size(); ++i) {
    if (std::tolower(static_cast<unsigned char>(a[i])) !=
        std::tolower(static_cast<unsigned char>(b[i]))) {
      return false;
    }
  }
  return true;
}

[[nodiscard]] bool HasPrefix(const std::string &s, const std::string &prefix) {
  return s.size() >= prefix.size() && s.compare(0, prefix.size(), prefix) == 0;
}

[[nodiscard]] bool HasSuffix(const std::string &s, const std::string &suffix) {
  return s.size() >= suffix.size() &&
         s.compare(s.size() - suffix.size(), suffix.size(), suffix) == 0;
}

[[nodiscard]] bool IsTableRow(const std::string &s) {
  return HasPrefix(s, "|") && HasSuffix(s, "|");
}

[[nodiscard]] bool IsDividerRow(const std::string &s) {
  if (!IsTableRow(s)) {
    return false;
  }
  for (const char ch : s) {
    switch (ch) {
    case '|':
    case '-':
    case ':':
    case ' ':
    case '\t':
      break;
    default:
      return false;
    }
  }
  return true;
}

[[nodiscard]] std::vector<std::string> SplitRow(const std::string &s) {
  std::string inner = s;
  if (HasSuffix(inner, "|")) {
    inner = inner.substr(0, inner.size() - 1);
  }
  if (HasPrefix(inner, "|")) {
    inner = inner.substr(1);
  }
  std::vector<std::string> out;
  std::string::size_type start = 0;
  while (true) {
    const std::string::size_type pos = inner.find('|', start);
    if (pos == std::string::npos) {
      out.push_back(TrimSpace(inner.substr(start)));
      break;
    }
    out.push_back(TrimSpace(inner.substr(start, pos - start)));
    start = pos + 1;
  }
  return out;
}

[[nodiscard]] bool HasHeader(const std::vector<std::string> &headers,
                             const std::string &name) {
  for (const std::string &h : headers) {
    if (EqualFold(h, name)) {
      return true;
    }
  }
  return false;
}

void CheckHeaders(const std::vector<std::string> &got) {
  for (const std::string &want : kRequiredHeaders) {
    if (!HasHeader(got, want)) {
      std::ostringstream msg;
      msg << "missing required column \"" << want << "\" (required: ";
      for (std::size_t i = 0; i < kRequiredHeaders.size(); ++i) {
        if (i != 0) {
          msg << ",";
        }
        msg << kRequiredHeaders[i];
      }
      msg << ")";
      throw ParseError(msg.str());
    }
  }
}

// A column/value pair that an action must leave empty.
struct ForbidCell {
  std::string column;
  std::string value;
};

void Forbid(const std::string &action,
            std::initializer_list<ForbidCell> cells) {
  for (const ForbidCell &cell : cells) {
    if (!cell.value.empty()) {
      throw ParseError(action + " does not use the \"" + cell.column +
                       "\" column");
    }
  }
}

// Validates a row's expected outcome against the values allowed for its action.
void RequireExpect(const Row &row, const std::string &action,
                   std::initializer_list<const char *> allowed) {
  for (const char *a : allowed) {
    if (row.expect == a) {
      return;
    }
  }
  std::ostringstream msg;
  msg << action << " expect must be one of ";
  bool first = true;
  for (const char *a : allowed) {
    if (!first) {
      msg << "/";
    }
    first = false;
    msg << a;
  }
  msg << ", got \"" << row.expect << "\"";
  throw ParseError(msg.str());
}

void ValidateSeed(const Row &row) {
  RequireExpect(row, "SEED", {"OK", "REJECT"});
  if (row.account.empty()) {
    throw ParseError("SEED requires account");
  }
  if (row.asset.empty() || row.amount.empty()) {
    throw ParseError("SEED requires asset and amount");
  }
  Forbid("SEED", {{"instrument", row.instrument},
                  {"side", row.side},
                  {"qty", row.qty},
                  {"volume", row.volume},
                  {"price", row.price},
                  {"group", row.group}});
}

void ValidateTick(const Row &row) {
  RequireExpect(row, "TICK", {"OK"});
  if (row.instrument.empty() || row.price.empty()) {
    throw ParseError("TICK requires instrument and price");
  }
  // account and group are optional: empty = global push, set = addressed push.
  Forbid("TICK", {{"side", row.side},
                  {"qty", row.qty},
                  {"volume", row.volume},
                  {"asset", row.asset},
                  {"amount", row.amount},
                  {"fee", row.fee},
                  {"pnl", row.pnl},
                  {"reject", row.reject}});
}

void ValidateOrder(const Row &row) {
  RequireExpect(row, "ORDER", {"ACCEPT", "REJECT"});
  if (row.account.empty()) {
    throw ParseError("ORDER requires account");
  }
  if (row.instrument.empty() || row.side.empty()) {
    throw ParseError("ORDER requires instrument and side");
  }
  const bool hasQty = !row.qty.empty();
  const bool hasVolume = !row.volume.empty();
  if (hasQty && hasVolume) {
    throw ParseError("ORDER must set exactly one of qty or volume, not both");
  }
  if (!hasQty && !hasVolume) {
    throw ParseError("ORDER must set exactly one of qty or volume");
  }
  if (row.expect != "REJECT" && !row.reject.empty()) {
    throw ParseError("ORDER reject code is only valid with expect REJECT");
  }
  Forbid("ORDER", {{"asset", row.asset},
                   {"amount", row.amount},
                   {"fee", row.fee},
                   {"pnl", row.pnl},
                   {"group", row.group}});
}

void ValidateFill(const Row &row) {
  RequireExpect(row, "FILL", {"OK", "REJECT"});
  if (row.account.empty()) {
    throw ParseError("FILL requires account");
  }
  if (row.instrument.empty() || row.side.empty() || row.qty.empty() ||
      row.price.empty()) {
    throw ParseError("FILL requires instrument, side, qty and price");
  }
  if (row.expect != "REJECT" && !row.reject.empty()) {
    throw ParseError("FILL reject code is only valid with expect REJECT");
  }
  Forbid("FILL", {{"volume", row.volume},
                  {"asset", row.asset},
                  {"amount", row.amount},
                  {"group", row.group}});
}

void ValidateGroup(const Row &row) {
  RequireExpect(row, "GROUP", {"OK"});
  if (row.account.empty() || row.group.empty()) {
    throw ParseError("GROUP requires account and group");
  }
  Forbid("GROUP", {{"instrument", row.instrument},
                   {"side", row.side},
                   {"qty", row.qty},
                   {"volume", row.volume},
                   {"price", row.price},
                   {"asset", row.asset},
                   {"amount", row.amount},
                   {"fee", row.fee},
                   {"pnl", row.pnl},
                   {"reject", row.reject}});
}

// Dispatches validation to the handler for the row's action.
void ValidateRow(const Row &row) {
  if (row.action == "SEED") {
    ValidateSeed(row);
  } else if (row.action == "TICK") {
    ValidateTick(row);
  } else if (row.action == "ORDER") {
    ValidateOrder(row);
  } else if (row.action == "FILL") {
    ValidateFill(row);
  } else if (row.action == "GROUP") {
    ValidateGroup(row);
  } else {
    throw ParseError("unknown action \"" + row.action + "\"");
  }
}

// Builds a row from named fields, normalizes values, then validates it.
[[nodiscard]] Row BuildRow(const std::vector<std::string> &fields,
                           const std::vector<std::string> &headers,
                           int lineNo) {
  const auto cell = [&](const std::string &name) -> std::string {
    for (std::size_t i = 0; i < headers.size(); ++i) {
      if (EqualFold(headers[i], name)) {
        return i < fields.size() ? fields[i] : std::string();
      }
    }
    return std::string();
  };
  Row row;
  row.line = lineNo;
  row.step = cell("#");
  row.account = cell("account");
  row.action = ToUpper(cell("action"));
  row.instrument = cell("instrument");
  row.side = ToUpper(cell("side"));
  row.qty = cell("qty");
  row.volume = cell("volume");
  row.price = cell("price");
  row.asset = cell("asset");
  row.amount = cell("amount");
  row.fee = cell("fee");
  row.pnl = cell("pnl");
  row.group = cell("group");
  row.expect = ToUpper(cell("expect"));
  row.reject = cell("reject");
  row.note = cell("note");
  ValidateRow(row);
  return row;
}

// Skips empty and #-prefixed front-matter lines; applies `name` and
// `slippage_bps`. Missing colons, invalid slippage values, and unknown keys
// throw `ParseError`.
void ParseFrontMatterLine(Frontmatter &fm, const std::string &line, int lineNo,
                          const std::string &name) {
  if (line.empty() || HasPrefix(line, "#")) {
    return;
  }
  const std::string::size_type colon = line.find(':');
  if (colon == std::string::npos) {
    throw ParseError(name + ":" + std::to_string(lineNo) +
                     ": front-matter expects key: value, got \"" + line + "\"");
  }
  const std::string key = TrimSpace(line.substr(0, colon));
  const std::string value = TrimSpace(line.substr(colon + 1));
  if (key == "name") {
    fm.name = value;
  } else if (key == "slippage_bps") {
    char *endptr = nullptr;
    const unsigned long parsed = std::strtoul(value.c_str(), &endptr, 10);
    if (endptr == value.c_str() || *endptr != '\0' || parsed > 0xFFFFul) {
      throw ParseError(name + ":" + std::to_string(lineNo) +
                       ": slippage_bps: invalid uint16 \"" + value + "\"");
    }
    fm.slippageBps = static_cast<std::uint16_t>(parsed);
  } else {
    throw ParseError(name + ":" + std::to_string(lineNo) +
                     ": unknown front-matter key \"" + key + "\"");
  }
}

} // namespace

Table Parse(std::istream &in, const std::string &name) {
  Table t;
  int lineNo = 0;
  State state = State::Start;
  std::vector<std::string> headers;

  std::string raw;
  while (std::getline(in, raw)) {
    ++lineNo;
    if (!raw.empty() && raw.back() == '\r') {
      raw.pop_back();
    }
    const std::string trimmed = TrimSpace(raw);

    switch (state) {
    case State::Start:
      if (trimmed == "---") {
        state = State::FrontMatter;
        continue;
      }
      if (IsTableRow(trimmed)) {
        headers = SplitRow(trimmed);
        state = State::AwaitDivider;
        continue;
      }
      // other text - skip
      break;

    case State::FrontMatter:
      if (trimmed == "---") {
        state = State::Body;
        continue;
      }
      ParseFrontMatterLine(t.fm, trimmed, lineNo, name);
      break;

    case State::Body:
      if (IsTableRow(trimmed)) {
        headers = SplitRow(trimmed);
        state = State::AwaitDivider;
      }
      break;

    case State::AwaitDivider:
      if (!IsDividerRow(trimmed)) {
        throw ParseError(name + ":" + std::to_string(lineNo) +
                         ": expected table divider after header, got \"" +
                         trimmed + "\"");
      }
      try {
        CheckHeaders(headers);
      } catch (const ParseError &err) {
        throw ParseError(name + ":" + std::to_string(lineNo - 1) + ": " +
                         err.what());
      }
      state = State::Rows;
      break;

    case State::Rows: {
      if (!IsTableRow(trimmed)) {
        // table ended; v1 takes only the first table block.
        state = State::Done;
        continue;
      }
      const std::vector<std::string> fields = SplitRow(trimmed);
      try {
        t.rows.push_back(BuildRow(fields, headers, lineNo));
      } catch (const ParseError &err) {
        throw ParseError(name + ":" + std::to_string(lineNo) + ": " +
                         err.what());
      }
      break;
    }

    case State::Done:
      // ignore trailing prose
      break;
    }
  }
  if (in.bad()) {
    throw ParseError(name + ": read error");
  }
  if (state != State::Rows && state != State::Done) {
    throw ParseError(name + ": no table found");
  }
  if (t.rows.empty()) {
    throw ParseError(name + ": table has no rows");
  }
  return t;
}

Table ParseFile(const std::string &path) {
  std::ifstream file(path);
  if (!file.is_open()) {
    throw ParseError("open " + path + ": cannot open file");
  }
  return Parse(file, path);
}

} // namespace spot_table
