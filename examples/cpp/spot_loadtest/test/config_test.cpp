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

// Config validation tests.
//

#include "spot_loadtest/config/config.hpp"

#include <gtest/gtest.h>

#include <array>
#include <chrono>
#include <limits>
#include <string>
#include <utility>
#include <vector>

namespace {

namespace config = spot_loadtest::config;

// A minimal but complete, valid config covering every section the generator
// consumes plus the required [run] block.
constexpr const char *kFullConfig = R"(
[run]
seed = 0xC0FFEE
total_ops = 1000
window = 100
window_unit = ops
observer = on

[arrival]
offered_rate = 50000

[report_delay]
distribution = lognormal
mean = 2ms
sigma = 0.5

[reject]
target_rate = 0.05
tolerance = 0.005

[accounts]
count = 100

[concurrency]
active_accounts = 32
submitter_workers = 8
max_submit_lag = 1ms

[async_engine]
strategy              = dynamic
max_queues            = 128
idle_cleanup          = 2s
sharded_workers       = 0
queue_capacity        = 0
slow_submit_threshold = 0

[instruments]
symbols = AAPL,SPX,MSFT
settlement = USD

[lifecycle]
p_open = 0.40
p_add = 0.15
p_partial_close = 0.25
p_full_close = 0.20

[funding]
trigger = balance_below
amount = 100000
seed = 1000000
top_up = 1000000

[cohort.chatty]
weight = 0.3
activity = 0.9
reject_propensity = 0.7
burst_len = 4
size_weights = 1:1,10:4,100:2
symbol_skew = zipf
zipf_s = 1.3

[cohort.steady]
weight = 0.7
activity = 0.5
reject_propensity = 0.25
burst_len = 2
size_weights = 1:2,10:3
symbol_skew = uniform
)";

// Replaces the first occurrence of `oldStr` in kFullConfig with `replacement`.
[[nodiscard]] std::string ReplaceLine(const std::string &oldStr,
                                      const std::string &replacement) {
  std::string s = kFullConfig;
  const std::size_t pos = s.find(oldStr);
  if (pos != std::string::npos) {
    s.replace(pos, oldStr.size(), replacement);
  }
  return s;
}

// Removes the first physical line containing `marker`.
[[nodiscard]] std::string RemoveLine(const std::string &marker) {
  std::string s = kFullConfig;
  const std::size_t pos = s.find(marker);
  if (pos == std::string::npos) {
    return s;
  }
  std::size_t lineStart = s.rfind('\n', pos);
  if (lineStart == std::string::npos) {
    lineStart = 0;
  }
  std::size_t lineEnd = s.find('\n', pos);
  if (lineEnd == std::string::npos) {
    lineEnd = s.size();
  }
  s.erase(lineStart, lineEnd - lineStart);
  return s;
}

// Removes every physical line containing any of the given markers (used to
// delete a whole section).
[[nodiscard]] std::string DropLines(std::string s,
                                    const std::vector<std::string> &markers) {
  std::string out;
  std::size_t i = 0;
  while (i < s.size()) {
    std::size_t end = s.find('\n', i);
    if (end == std::string::npos) {
      end = s.size();
    }
    const std::string line = s.substr(i, end - i);
    bool drop = false;
    for (const std::string &m : markers) {
      if (line.find(m) != std::string::npos) {
        drop = true;
        break;
      }
    }
    if (!drop) {
      out += line;
      if (end < s.size()) {
        out += '\n';
      }
    }
    i = end + 1;
  }
  return out;
}

[[nodiscard]] config::Config LoadString(const std::string &content) {
  return config::LoadFromString(content, "test.ini", "hash");
}

[[nodiscard]] bool Contains(const std::string &s, const std::string &sub) {
  return s.find(sub) != std::string::npos;
}

void ExpectConfigError(const std::string &content,
                       const std::string &expected) {
  try {
    (void)LoadString(content);
    ADD_FAILURE() << "expected ConfigError: " << expected;
  } catch (const config::ConfigError &e) {
    EXPECT_EQ(e.what(), expected);
  }
}

[[nodiscard]] std::string ConfigErrorMessage(const std::string &content) {
  try {
    (void)LoadString(content);
    ADD_FAILURE() << "expected ConfigError";
  } catch (const config::ConfigError &e) {
    return e.what();
  }
  return {};
}

TEST(Config, LoadFullConfigValid) {
  const config::Config cfg = LoadString(kFullConfig);
  EXPECT_EQ(cfg.accounts.count, 100u);
  EXPECT_EQ(cfg.instruments.settlement, "USD");
  ASSERT_EQ(cfg.cohorts.size(), 2u);
  // Cohorts are sorted by name: chatty < steady.
  EXPECT_EQ(cfg.cohorts[0].name, "chatty");
  EXPECT_EQ(cfg.cohorts[1].name, "steady");
  EXPECT_EQ(cfg.cohorts[0].symbolSkew, config::SymbolSkew::Zipf);
  EXPECT_DOUBLE_EQ(cfg.cohorts[0].zipfS, 1.3);
  EXPECT_EQ(cfg.cohorts[0].sizeWeights.size(), 3u);
  EXPECT_EQ(cfg.funding.seed, cfg.funding.topUp);
  EXPECT_EQ(cfg.funding.seed.ToString(), "1000000");
  EXPECT_EQ(cfg.concurrency.activeAccounts, 32u);
  EXPECT_EQ(cfg.concurrency.submitterWorkers, 8u);
  EXPECT_EQ(cfg.concurrency.maxSubmitLag, std::chrono::milliseconds(1));
  EXPECT_EQ(cfg.reportDelay.distribution,
            config::ReportDelayDistribution::Lognormal);
  EXPECT_EQ(cfg.reportDelay.mean, std::chrono::milliseconds(2));
  EXPECT_DOUBLE_EQ(cfg.reportDelay.sigma, 0.5);
  EXPECT_EQ(cfg.asyncEngine.strategy, config::AsyncEngineStrategy::Dynamic);
  EXPECT_EQ(cfg.asyncEngine.maxQueues, 128u);
  EXPECT_EQ(cfg.asyncEngine.idleCleanup, std::chrono::seconds(2));
  EXPECT_EQ(cfg.asyncEngine.shardedWorkers, 0);
  EXPECT_EQ(cfg.asyncEngine.queueCapacity, 0);
  EXPECT_EQ(cfg.asyncEngine.slowSubmitThreshold.count(), 0);
}

TEST(Config, StrictValidationRejectsBadValues) {
  struct Case {
    std::string name;
    std::string ini;
    std::string wantErr;
  };
  std::vector<Case> cases = {
      {"run total_ops overflow",
       ReplaceLine("total_ops = 1000", "total_ops = 18446744073709551616"),
       "total_ops"},
      {"run hexadecimal seed overflow",
       ReplaceLine("seed = 0xC0FFEE", "seed = 0x10000000000000000"), "seed"},
      {"arrival offered_rate malformed",
       ReplaceLine("offered_rate = 50000", "offered_rate = 50000x"),
       "offered_rate"},
      {"arrival offered_rate overflow",
       ReplaceLine("offered_rate = 50000",
                   "offered_rate = 18446744073709551616"),
       "offered_rate"},
      {"report delay unknown distribution",
       ReplaceLine("distribution = lognormal", "distribution = gaussian"),
       "distribution"},
      {"report delay malformed mean",
       ReplaceLine("mean = 2ms", "mean = quickly"), "mean"},
      {"report delay negative mean", ReplaceLine("mean = 2ms", "mean = -1ms"),
       "mean"},
      {"report delay overflowing mean",
       ReplaceLine("mean = 2ms", "mean = 999999999999999999999h"), "mean"},
      {"report delay malformed sigma",
       ReplaceLine("sigma = 0.5", "sigma = wide"), "sigma"},
      {"report delay non-finite sigma",
       ReplaceLine("sigma = 0.5", "sigma = nan"), "sigma"},
      {"report delay infinite sigma", ReplaceLine("sigma = 0.5", "sigma = inf"),
       "sigma"},
      {"report delay negative sigma",
       ReplaceLine("sigma = 0.5", "sigma = -0.1"), "sigma"},
      {"reject target_rate non-numeric",
       ReplaceLine("target_rate = 0.05", "target_rate = high"), "target_rate"},
      {"reject target_rate >= 1",
       ReplaceLine("target_rate = 0.05", "target_rate = 1.0"), "target_rate"},
      {"reject tolerance zero",
       ReplaceLine("tolerance = 0.005", "tolerance = 0"), "tolerance"},
      {"accounts count zero", ReplaceLine("count = 100", "count = 0"), "count"},
      {"concurrency active_accounts zero",
       ReplaceLine("active_accounts = 32", "active_accounts = 0"),
       "active_accounts"},
      {"concurrency active_accounts exceeds population",
       ReplaceLine("active_accounts = 32", "active_accounts = 101"),
       "exceeds accounts.count"},
      {"concurrency submitter_workers missing",
       RemoveLine("submitter_workers = 8"), "submitter_workers"},
      {"concurrency submitter_workers zero",
       ReplaceLine("submitter_workers = 8", "submitter_workers = 0"),
       "submitter_workers"},
      {"concurrency submitter_workers malformed",
       ReplaceLine("submitter_workers = 8", "submitter_workers = several"),
       "submitter_workers"},
      {"concurrency submitter_workers exceeds active set",
       ReplaceLine("submitter_workers = 8", "submitter_workers = 33"),
       "exceeds active_accounts"},
      {"concurrency max_submit_lag missing", RemoveLine("max_submit_lag = 1ms"),
       "max_submit_lag"},
      {"concurrency max_submit_lag zero",
       ReplaceLine("max_submit_lag = 1ms", "max_submit_lag = 0"),
       "max_submit_lag"},
      {"concurrency max_submit_lag negative",
       ReplaceLine("max_submit_lag = 1ms", "max_submit_lag = -1ns"),
       "max_submit_lag"},
      {"concurrency max_submit_lag malformed",
       ReplaceLine("max_submit_lag = 1ms", "max_submit_lag = eventually"),
       "max_submit_lag"},
      {"engine bad strategy",
       ReplaceLine("strategy              = dynamic",
                   "strategy              = turbocharged"),
       "strategy"},
      {"engine sharded without workers",
       ReplaceLine("strategy              = dynamic",
                   "strategy              = sharded"),
       "sharded_workers"},
      {"engine max_queues below active set",
       ReplaceLine("max_queues            = 128", "max_queues            = 16"),
       "max_queues"},
      {"engine max_queues non-numeric",
       ReplaceLine("max_queues            = 128",
                   "max_queues            = lots"),
       "max_queues"},
      {"engine idle_cleanup not a duration",
       ReplaceLine("idle_cleanup          = 2s",
                   "idle_cleanup          = soon"),
       "idle_cleanup"},
      {"engine idle_cleanup negative",
       ReplaceLine("idle_cleanup          = 2s", "idle_cleanup          = -1s"),
       "idle_cleanup"},
      {"engine idle_cleanup out of range",
       ReplaceLine("idle_cleanup          = 2s",
                   "idle_cleanup          = 999999999999999999999h"),
       "idle_cleanup"},
      {"engine sharded_workers int64 minimum",
       ReplaceLine("sharded_workers       = 0",
                   "sharded_workers       = -9223372036854775808"),
       "must fit a non-negative int"},
      {"engine sharded_workers below int64 minimum",
       ReplaceLine("sharded_workers       = 0",
                   "sharded_workers       = -9223372036854775809"),
       "non-negative integer"},
      {"engine sharded_workers exceeds int",
       ReplaceLine("sharded_workers       = 0",
                   "sharded_workers       = 2147483648"),
       "must fit a non-negative int"},
      {"engine queue_capacity negative",
       ReplaceLine("queue_capacity        = 0", "queue_capacity        = -1"),
       "queue_capacity"},
      {"engine queue_capacity exceeds int",
       ReplaceLine("queue_capacity        = 0",
                   "queue_capacity        = 2147483648"),
       "must fit an int"},
      {"engine slow_submit_threshold not a duration",
       ReplaceLine("slow_submit_threshold = 0", "slow_submit_threshold = soon"),
       "slow_submit_threshold"},
      {"engine slow_submit_threshold negative",
       ReplaceLine("slow_submit_threshold = 0", "slow_submit_threshold = -1s"),
       "slow_submit_threshold"},
      {"engine slow_submit_threshold out of range",
       ReplaceLine("slow_submit_threshold = 0",
                   "slow_submit_threshold = 999999999999999999999h"),
       "slow_submit_threshold"},
      {"accounts count non-numeric", ReplaceLine("count = 100", "count = many"),
       "count"},
      {"instruments symbols blank entry",
       ReplaceLine("symbols = AAPL,SPX,MSFT", "symbols = AAPL,,MSFT"), "blank"},
      {"instruments duplicate symbol",
       ReplaceLine("symbols = AAPL,SPX,MSFT", "symbols = AAPL,AAPL"),
       "duplicate"},
      {"settlement collides with underlying",
       ReplaceLine("settlement = USD", "settlement = AAPL"),
       "must not also be an underlying"},
      {"lifecycle probability out of range",
       ReplaceLine("p_open = 0.40", "p_open = 1.5"), "p_open"},
      {"lifecycle transition probabilities exceed one",
       ReplaceLine("p_add = 0.15", "p_add = 0.75"),
       "p_add + p_partial_close + p_full_close"},
      {"lifecycle probability non-finite",
       ReplaceLine("p_open = 0.40", "p_open = nan"), "p_open"},
      {"funding unknown trigger",
       ReplaceLine("trigger = balance_below", "trigger = on_tuesday"),
       "trigger"},
      {"funding amount non-positive",
       ReplaceLine("amount = 100000", "amount = 0"), "amount"},
      {"cohort weight non-positive", ReplaceLine("weight = 0.3", "weight = 0"),
       "weight"},
      {"cohort missing burst_len", RemoveLine("burst_len = 4"), "burst_len"},
      {"cohort bad size_weights",
       ReplaceLine("size_weights = 1:1,10:4,100:2", "size_weights = 1:1,bad"),
       "size_weights"},
      {"cohort zipf without zipf_s", RemoveLine("zipf_s = 1.3"), "zipf_s"},
      {"cohort zipf_s <= 1", ReplaceLine("zipf_s = 1.3", "zipf_s = 0.9"),
       "zipf_s"},
  };

  for (const Case &tc : cases) {
    try {
      (void)LoadString(tc.ini);
      ADD_FAILURE() << tc.name << ": expected error containing " << tc.wantErr;
    } catch (const config::ConfigError &e) {
      EXPECT_TRUE(Contains(e.what(), tc.wantErr))
          << tc.name << ": got " << e.what() << ", want " << tc.wantErr;
    }
  }
}

TEST(Config, NoCohortsIsError) {
  std::string ini = DropLines(
      kFullConfig,
      {"[cohort.chatty]", "[cohort.steady]", "weight = 0.3", "activity = 0.9",
       "reject_propensity = 0.7", "burst_len = 4",
       "size_weights = 1:1,10:4,100:2", "symbol_skew = zipf", "zipf_s = 1.3",
       "weight = 0.7", "activity = 0.5", "reject_propensity = 0.25",
       "burst_len = 2", "size_weights = 1:2,10:3", "symbol_skew = uniform"});
  try {
    (void)LoadString(ini);
    ADD_FAILURE() << "expected a missing-cohort error";
  } catch (const config::ConfigError &e) {
    EXPECT_TRUE(Contains(e.what(), "cohort"));
  }
}

TEST(Config, RunValidationNotRegressed) {
  EXPECT_THROW((void)LoadString(RemoveLine("window_unit = ops")),
               config::ConfigError);
  EXPECT_THROW(
      (void)LoadString(ReplaceLine("seed = 0xC0FFEE", "seed = notanumber")),
      config::ConfigError);
}

TEST(Config, IniSyntaxAndSchemaFailClosed) {
  struct Case {
    const char *name;
    std::string ini;
    std::vector<std::string> expected;
  };
  const std::vector<Case> cases = {
      {"malformed assignment",
       ReplaceLine("offered_rate = 50000", "offered_rate 50000"),
       {"config test.ini line ", "expected key = value or [section]"}},
      {"unknown optional key",
       ReplaceLine("offered_rate = 50000", "offered_reate = 50000"),
       {"config test.ini line ", "[arrival]: unknown key offered_reate"}},
      {"duplicate key",
       ReplaceLine("offered_rate = 50000",
                   "offered_rate = 50000\noffered_rate = 60000"),
       {"config test.ini line ", "[arrival]: duplicate key offered_rate",
        "first defined at line"}},
      {"duplicate section",
       ReplaceLine("[arrival]", "[arrival]\n[arrival]"),
       {"config test.ini line ", "duplicate section [arrival]",
        "first defined at line"}},
      {"unknown section",
       std::string(kFullConfig) + "\n[metrics]\n",
       {"config test.ini line ", "unknown section [metrics]"}},
      {"unknown run key",
       ReplaceLine("window = 100", "windwo = 100"),
       {"config test.ini line ", "[run]: unknown key windwo"}},
      {"unknown cohort key",
       ReplaceLine("activity = 0.9", "activty = 0.9"),
       {"config test.ini line ", "[cohort.chatty]: unknown key activty"}},
      {"empty cohort name",
       std::string(kFullConfig) + "\n[cohort.]\n",
       {"config test.ini line ", "unknown section [cohort.]"}},
  };

  for (const Case &testCase : cases) {
    const std::string error = ConfigErrorMessage(testCase.ini);
    for (const std::string &expected : testCase.expected) {
      EXPECT_TRUE(Contains(error, expected))
          << testCase.name << ": got " << error << ", missing " << expected;
    }
  }
}

TEST(Config, UnsupportedRunModesFailClosed) {
  ExpectConfigError(
      ReplaceLine("total_ops = 1000", "total_ops = 1000\nduration = 1s"),
      "config test.ini [run]: duration: unsupported; use total_ops as the "
      "run bound");
  ExpectConfigError(
      ReplaceLine("total_ops = 1000", "duration = 1s"),
      "config test.ini [run]: duration: unsupported; use total_ops as the "
      "run bound");
  ExpectConfigError(
      ReplaceLine("window_unit = ops", "window_unit = wall"),
      "config test.ini [run]: window_unit: wall is unsupported; use ops");
}

TEST(Config, OptionalWorkloadSectionsRetainDefaultsOnlyWhenAbsent) {
  const std::string withoutSections = DropLines(
      kFullConfig, {"[arrival]", "offered_rate = 50000", "[report_delay]",
                    "distribution = lognormal", "mean = 2ms", "sigma = 0.5"});
  const std::string withoutKeys = DropLines(
      kFullConfig, {"offered_rate = 50000", "distribution = lognormal",
                    "mean = 2ms", "sigma = 0.5"});
  for (const std::string &ini : {withoutSections, withoutKeys}) {
    const config::Config cfg = LoadString(ini);
    EXPECT_EQ(cfg.arrival.offeredRate, 0u);
    EXPECT_EQ(cfg.reportDelay.distribution,
              config::ReportDelayDistribution::None);
    EXPECT_EQ(cfg.reportDelay.mean.count(), 0);
    EXPECT_DOUBLE_EQ(cfg.reportDelay.sigma, 0.0);
  }
}

TEST(Config, ReportDelayRejectsExplicitFieldsWithoutDistribution) {
  const std::string meanOnly =
      DropLines(kFullConfig, {"distribution = lognormal", "sigma = 0.5"});
  const std::string sigmaOnly =
      DropLines(kFullConfig, {"distribution = lognormal", "mean = 2ms"});
  constexpr const char *kExpected =
      "config test.ini [report_delay]: distribution: required when mean or "
      "sigma is set";
  ExpectConfigError(meanOnly, kExpected);
  ExpectConfigError(sigmaOnly, kExpected);
}

TEST(Config, ReportDelayRejectsFieldsIgnoredByDistribution) {
  const std::string noneWithMean =
      DropLines(ReplaceLine("distribution = lognormal", "distribution = none"),
                {"sigma = 0.5"});
  const std::string noneWithSigma =
      DropLines(ReplaceLine("distribution = lognormal", "distribution = none"),
                {"mean = 2ms"});
  constexpr const char *kNoneExpected =
      "config test.ini [report_delay]: mean and sigma: must be omitted when "
      "distribution is none";
  ExpectConfigError(noneWithMean, kNoneExpected);
  ExpectConfigError(noneWithSigma, kNoneExpected);

  ExpectConfigError(
      ReplaceLine("distribution = lognormal", "distribution = fixed"),
      "config test.ini [report_delay]: sigma: must be omitted when "
      "distribution is fixed");

  constexpr const char *kZeroMeanExpected =
      "config test.ini [report_delay]: sigma: must be omitted when lognormal "
      "mean is zero";
  ExpectConfigError(ReplaceLine("mean = 2ms", "mean = 0"), kZeroMeanExpected);
  ExpectConfigError(DropLines(kFullConfig, {"mean = 2ms"}), kZeroMeanExpected);
}

TEST(Config, ReportDelayDefaultsRemainValidWhenFieldsAreOmitted) {
  const config::Config none = LoadString(
      DropLines(ReplaceLine("distribution = lognormal", "distribution = none"),
                {"mean = 2ms", "sigma = 0.5"}));
  EXPECT_EQ(none.reportDelay.distribution,
            config::ReportDelayDistribution::None);
  EXPECT_EQ(none.reportDelay.mean.count(), 0);
  EXPECT_DOUBLE_EQ(none.reportDelay.sigma, 0.0);

  const config::Config fixed = LoadString(
      DropLines(ReplaceLine("distribution = lognormal", "distribution = fixed"),
                {"sigma = 0.5"}));
  EXPECT_EQ(fixed.reportDelay.distribution,
            config::ReportDelayDistribution::Fixed);
  EXPECT_EQ(fixed.reportDelay.mean, std::chrono::milliseconds(2));
  EXPECT_DOUBLE_EQ(fixed.reportDelay.sigma, 0.0);

  const config::Config lognormal =
      LoadString(DropLines(kFullConfig, {"mean = 2ms", "sigma = 0.5"}));
  EXPECT_EQ(lognormal.reportDelay.distribution,
            config::ReportDelayDistribution::Lognormal);
  EXPECT_EQ(lognormal.reportDelay.mean.count(), 0);
  EXPECT_DOUBLE_EQ(lognormal.reportDelay.sigma, 0.0);
}

TEST(Config, DurationNanosecondRangeIsExact) {
  const config::Config maximum =
      LoadString(ReplaceLine("mean = 2ms", "mean = 9223372036854775807ns"));
  EXPECT_EQ(maximum.reportDelay.mean.count(),
            std::numeric_limits<std::int64_t>::max());

  const config::Config fractionalMaximum = LoadString(
      ReplaceLine("mean = 2ms", "mean = 9223372036854775807ns0.9ns"));
  EXPECT_EQ(fractionalMaximum.reportDelay.mean.count(),
            std::numeric_limits<std::int64_t>::max());

  constexpr const char *kExpected =
      "config test.ini [report_delay]: mean: must be a non-negative duration "
      "(e.g. 2ms)";
  ExpectConfigError(ReplaceLine("mean = 2ms", "mean = 9223372036854775808ns"),
                    kExpected);
  ExpectConfigError(
      ReplaceLine("mean = 2ms", "mean = 9223372036854775807ns0.5ns0.5ns"),
      kExpected);
}

TEST(Config, DurationCompositeDecimalsTruncateAfterSumming) {
  const config::Config carried =
      LoadString(ReplaceLine("mean = 2ms", "mean = 1.5ns0.5ns"));
  EXPECT_EQ(carried.reportDelay.mean, std::chrono::nanoseconds(2));

  const config::Config composite =
      LoadString(ReplaceLine("mean = 2ms", "mean = 1.5ms250.75us0.5ns"));
  EXPECT_EQ(composite.reportDelay.mean, std::chrono::nanoseconds(1'750'750));
}

TEST(Config, ZeroOfferedRateIsExplicitlyUnpaced) {
  const config::Config cfg =
      LoadString(ReplaceLine("offered_rate = 50000", "offered_rate = 0"));
  EXPECT_EQ(cfg.arrival.offeredRate, 0u);
}

TEST(Config, UnsignedMaximumIsAcceptedWithoutWrapping) {
  const config::Config cfg =
      LoadString(ReplaceLine("seed = 0xC0FFEE", "seed = 18446744073709551615"));
  EXPECT_EQ(cfg.run.seed, std::numeric_limits<std::uint64_t>::max());
}

TEST(Config, RunWindowFitsSignedMeasurementRange) {
  const config::Config cfg =
      LoadString(ReplaceLine("window = 100", "window = 9223372036854775807"));
  EXPECT_EQ(cfg.run.window, static_cast<std::uint64_t>(
                                std::numeric_limits<std::int64_t>::max()));

  ExpectConfigError(
      ReplaceLine("window = 100", "window = 9223372036854775808"),
      "config test.ini [run]: window: must be <= 9223372036854775807");
}

TEST(Config, LifecycleTransitionSimplexInvariant) {
  struct Point {
    const char *add;
    const char *partialClose;
    const char *fullClose;
    bool valid;
  };
  constexpr std::array<Point, 7> points = {{
      {"0", "0", "0", true},
      {"1", "0", "0", true},
      {"0.1", "0.2", "0.7", true},
      {"0.3333333333333333", "0.3333333333333333", "0.3333333333333333", true},
      {"0.6", "0.6", "0", false},
      {"1", "0.000001", "0", false},
      {"0.1", "0.2", "0.70000000000001", false},
  }};

  // Transition weights are normalised downstream. Checking representative
  // simplex points as one invariant prevents invalid configs from being
  // silently rescaled into a different lifecycle.
  for (const Point &point : points) {
    std::string ini =
        ReplaceLine("p_add = 0.15", "p_add = " + std::string(point.add));
    const auto replace = [](std::string value, const std::string &oldText,
                            const std::string &newText) {
      const std::size_t pos = value.find(oldText);
      EXPECT_NE(pos, std::string::npos);
      if (pos != std::string::npos) {
        value.replace(pos, oldText.size(), newText);
      }
      return value;
    };
    ini = replace(std::move(ini), "p_partial_close = 0.25",
                  "p_partial_close = " + std::string(point.partialClose));
    ini = replace(std::move(ini), "p_full_close = 0.20",
                  "p_full_close = " + std::string(point.fullClose));
    ini = replace(std::move(ini), "p_open = 0.40", "p_open = 1");

    if (point.valid) {
      EXPECT_NO_THROW((void)LoadString(ini));
    } else {
      EXPECT_THROW((void)LoadString(ini), config::ConfigError);
    }
  }
}

TEST(Config, EngineMaxQueuesUnlimited) {
  const config::Config cfg = LoadString(
      ReplaceLine("max_queues            = 128", "max_queues            = 0"));
  EXPECT_EQ(cfg.asyncEngine.maxQueues, 0u);
}

TEST(Config, EngineShardedStrategy) {
  std::string ini = ReplaceLine("strategy              = dynamic",
                                "strategy              = sharded");
  ini.replace(ini.find("sharded_workers       = 0"),
              std::string("sharded_workers       = 0").size(),
              "sharded_workers       = 4");
  const config::Config cfg = LoadString(ini);
  EXPECT_EQ(cfg.asyncEngine.strategy, config::AsyncEngineStrategy::Sharded);
  EXPECT_EQ(cfg.asyncEngine.shardedWorkers, 4);

  // sharded_workers = 0 with strategy = sharded is an error.
  EXPECT_THROW((void)LoadString(ReplaceLine("strategy              = dynamic",
                                            "strategy              = sharded")),
               config::ConfigError);
}

TEST(Config, EngineSharedKnobs) {
  std::string ini =
      ReplaceLine("queue_capacity        = 0", "queue_capacity        = 512");
  ini.replace(ini.find("slow_submit_threshold = 0"),
              std::string("slow_submit_threshold = 0").size(),
              "slow_submit_threshold = 250ms");
  const config::Config cfg = LoadString(ini);
  EXPECT_EQ(cfg.asyncEngine.queueCapacity, 512);
  EXPECT_EQ(cfg.asyncEngine.slowSubmitThreshold,
            std::chrono::milliseconds(250));
}

TEST(Config, ConcurrencyAndEngineSectionsRequired) {
  const std::string withoutConcurrency =
      DropLines(kFullConfig, {"[concurrency]", "active_accounts = 32",
                              "submitter_workers = 8", "max_submit_lag = 1ms"});
  EXPECT_THROW((void)LoadString(withoutConcurrency), config::ConfigError);

  const std::string withoutEngine = DropLines(
      kFullConfig, {"[async_engine]", "strategy              = dynamic",
                    "max_queues            = 128", "idle_cleanup          = 2s",
                    "sharded_workers       = 0", "queue_capacity        = 0",
                    "slow_submit_threshold = 0"});
  EXPECT_THROW((void)LoadString(withoutEngine), config::ConfigError);
}

// Loads the committed configs/baseline.ini through the strict parser, so the
// shipped reference config can never drift out of sync with the validation.
TEST(Config, BaselineConfigLoads) {
  const config::Config cfg = config::Load(SPOT_LOADTEST_BASELINE_INI);
  EXPECT_FALSE(cfg.cohorts.empty());
  EXPECT_FALSE(cfg.instruments.settlement.empty());
  EXPECT_FALSE(cfg.instruments.symbols.empty());
  EXPECT_EQ(cfg.run.seed, 0xC0FFEEu);
  EXPECT_EQ(cfg.run.totalOps, 2'000'000u);
  EXPECT_EQ(cfg.accounts.count, 10'000u);
  EXPECT_EQ(cfg.concurrency.activeAccounts, 1024u);
  EXPECT_EQ(cfg.concurrency.submitterWorkers, 16u);
  EXPECT_EQ(cfg.asyncEngine.strategy, config::AsyncEngineStrategy::Dynamic);
  EXPECT_EQ(cfg.asyncEngine.maxQueues, 0u);
  EXPECT_EQ(cfg.asyncEngine.idleCleanup, std::chrono::seconds(5));
  EXPECT_EQ(cfg.cohorts.size(), 3u);
  EXPECT_EQ(cfg.instruments.symbols.size(), 10u);
  EXPECT_EQ(cfg.instruments.settlement, "USD");
  EXPECT_TRUE(cfg.run.observer);
  EXPECT_FALSE(cfg.hash.empty());
}

} // namespace
