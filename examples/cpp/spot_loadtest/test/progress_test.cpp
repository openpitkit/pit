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

// Progress line formatting tests.
//

#include "spot_loadtest/measurement/sink.hpp"
#include "spot_loadtest/progress/progress.hpp"

#include <gtest/gtest.h>

#include <chrono>
#include <string>

namespace {

namespace progress = spot_loadtest::progress;
using spot_loadtest::measurement::LiveCounters;
using std::chrono::nanoseconds;
using std::chrono::seconds;

[[nodiscard]] bool Contains(const std::string &s, const std::string &sub) {
  return s.find(sub) != std::string::npos;
}

TEST(Progress, FormatLineContainsElapsed) {
  LiveCounters c{5000, 4800, 200};
  const std::string line = progress::FormatLine(
      c, 100000, std::chrono::duration_cast<nanoseconds>(seconds(10)));
  EXPECT_TRUE(Contains(line, "elapsed"));
}

TEST(Progress, FormatLineContainsDecided) {
  LiveCounters c{5000, 4800, 200};
  const std::string line = progress::FormatLine(
      c, 100000, std::chrono::duration_cast<nanoseconds>(seconds(10)));
  EXPECT_TRUE(Contains(line, "decided"));
}

TEST(Progress, FormatLineContainsInFlight) {
  LiveCounters c{5000, 4800, 200};
  const std::string line = progress::FormatLine(
      c, 100000, std::chrono::duration_cast<nanoseconds>(seconds(10)));
  EXPECT_TRUE(Contains(line, "in-flight"));
  EXPECT_TRUE(Contains(line, "200"));
}

TEST(Progress, FormatLineContainsRate) {
  LiveCounters c{50000, 50000, 0};
  const std::string line = progress::FormatLine(
      c, 100000, std::chrono::duration_cast<nanoseconds>(seconds(1)));
  EXPECT_TRUE(Contains(line, "rate"));
}

TEST(Progress, FormatLineContainsPercent) {
  LiveCounters c{50000, 50000, 0};
  const std::string line = progress::FormatLine(
      c, 100000, std::chrono::duration_cast<nanoseconds>(seconds(1)));
  EXPECT_TRUE(Contains(line, "%"));
}

TEST(Progress, FormatLineZeroTotal) {
  LiveCounters c{1000, 1000, 0};
  const std::string line = progress::FormatLine(
      c, 0, std::chrono::duration_cast<nanoseconds>(seconds(2)));
  EXPECT_TRUE(Contains(line, "?"));
}

TEST(Progress, FormatLineZeroDecided) {
  LiveCounters c{0, 0, 0};
  const std::string line = progress::FormatLine(c, 100000, nanoseconds(0));
  EXPECT_FALSE(line.empty());
}

TEST(Progress, FormatLineDurationRender) {
  LiveCounters c{0, 1000, 0};
  const std::string line =
      progress::FormatLine(c, 10000,
                           std::chrono::duration_cast<nanoseconds>(
                               std::chrono::minutes(2) + seconds(5)));
  EXPECT_TRUE(Contains(line, "2m"));
}

} // namespace
