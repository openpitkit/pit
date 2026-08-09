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

// A scenario file is a Markdown document with an optional `---`-delimited
// front-matter block and one GFM pipe-table. `ParseFile` reads the file,
// `Parse` parses an in-memory stream; both yield a `Table` of validated rows.
// Per-action validation runs at parse time so the runner can assume well-formed
// rows. A parse or validation failure is reported by throwing `ParseError`.

#include <cstdint>
#include <istream>
#include <stdexcept>
#include <string>
#include <vector>

namespace spot_table {

struct Frontmatter {
  std::string name;
  std::uint16_t slippageBps = 0;
};

// One parsed table row. Empty fields mean "not applicable to this action";
// per-action validation enforces which cells each action requires or forbids.
struct Row {
  int line = 0;
  std::string step;
  std::string account;
  std::string action;
  std::string instrument;
  std::string side;
  std::string qty;
  std::string volume;
  std::string price;
  std::string asset;
  std::string amount;
  std::string fee;
  std::string pnl;
  std::string group;
  std::string expect;
  std::string reject;
  std::string note;
};

struct Table {
  Frontmatter fm;
  std::vector<Row> rows;
};

// Thrown on any parse or per-row validation failure.
class ParseError : public std::runtime_error {
public:
  explicit ParseError(const std::string &message)
      : std::runtime_error(message) {}
};

// Reads and parses a table file. Throws `ParseError` on failure.
[[nodiscard]] Table ParseFile(const std::string &path);

// Parses the table from `in`. `name` is used in error messages. Throws
// `ParseError` on failure.
[[nodiscard]] Table Parse(std::istream &in, const std::string &name);

} // namespace spot_table
