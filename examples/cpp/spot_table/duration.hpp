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

// Formats, rounds, and parses durations for the CLI's `-timeout`,
// `-min-duration`, and report latency and wall-clock fields.

#include <chrono>
#include <string>

namespace spot_table {

// Formats durations as "1m30s" or "1h2m3s", using ns/µs/ms below one second.
[[nodiscard]] std::string FormatDuration(std::chrono::nanoseconds d);

// Rounds `d` to the nearest multiple of `unit`, with ties away from zero.
// A non-positive `unit` returns `d` unchanged.
[[nodiscard]] std::chrono::nanoseconds
RoundDuration(std::chrono::nanoseconds d, std::chrono::nanoseconds unit);

// Parses the CLI duration units ns, us/µs, ms, s, m, and h.
// Returns false and writes `err` for malformed input.
[[nodiscard]] bool ParseDuration(const std::string &text,
                                 std::chrono::nanoseconds &out,
                                 std::string &err);

} // namespace spot_table
