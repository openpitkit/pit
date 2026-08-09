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

#include "spot_loadtest/decimal.hpp"

#include <chrono>
#include <cstdint>
#include <stdexcept>
#include <string>
#include <vector>

// Loads and validates the INI configuration file for the spot-limit load test.
//
// Validation is up-front and explicit for required generator sections.
// `instruments.settlement` defaults to USD, while `funding.seed` and
// `funding.top_up` default to `funding.amount`. Optional [arrival] and
// [report_delay] sections and keys retain their struct defaults when absent.

namespace spot_loadtest::config {

// Thrown on any configuration validation failure with a contextual message.
class ConfigError : public std::runtime_error {
public:
  using std::runtime_error::runtime_error;
};

// The distribution of the simulated TS round-trip (report-return) delay.
enum class ReportDelayDistribution { None, Lognormal, Fixed };

// Top-level knobs that govern a single load-test run.
struct Run {
  std::uint64_t seed = 0;
  std::uint64_t totalOps = 0;
  std::uint64_t window = 0;
  bool observer = false;
};

// The offered-rate model; inter-arrival spacing is always exponential.
struct Arrival {
  std::uint64_t offeredRate = 0;
};

// Models the simulated TS round-trip before report settlement.
struct ReportDelay {
  std::chrono::nanoseconds mean{0};
  ReportDelayDistribution distribution = ReportDelayDistribution::None;
  double sigma = 0.0;
};

// The target reject-rate controller knobs.
struct Reject {
  double targetRate = 0.0;
  double tolerance = 0.0;
};

// Account-pool sizing.
struct Accounts {
  std::uint64_t count = 0;
};

// The bounded-concurrency workload knobs. All values must be positive, and the
// submitter pool cannot exceed the active set.
struct Concurrency {
  std::uint64_t activeAccounts = 0;
  std::uint64_t submitterWorkers = 0;
  std::chrono::nanoseconds maxSubmitLag{0};
};

// The dispatch strategy for the async engine.
enum class AsyncEngineStrategy { Dynamic, Sharded };

[[nodiscard]] std::string ToString(AsyncEngineStrategy strategy);

// The asyncengine builder knobs. These are dispatch/resource limits, NOT
// synchronization semantics.
struct AsyncEngine {
  AsyncEngineStrategy strategy = AsyncEngineStrategy::Dynamic;
  std::uint64_t maxQueues = 0;
  std::chrono::nanoseconds idleCleanup{0};
  int shardedWorkers = 0;
  int queueCapacity = 0;
  std::chrono::nanoseconds slowSubmitThreshold{0};
};

// The tradable universe consumed by the generator.
struct Instruments {
  std::vector<std::string> symbols;
  std::string settlement;
};

// One discrete order-size weight: a quantity (in lots) and its selection
// weight.
struct SizeBucket {
  std::uint64_t quantity = 0;
  double weight = 0.0;
};

// How a cohort biases its symbol choice.
enum class SymbolSkew { Uniform, Zipf };

// One named population segment parsed from a [cohort.<name>] section.
struct Cohort {
  std::string name;
  double weight = 0.0;
  double activity = 0.0;
  double rejectPropensity = 0.0;
  std::uint64_t burstLen = 0;
  std::vector<SizeBucket> sizeWeights;
  SymbolSkew symbolSkew = SymbolSkew::Uniform;
  double zipfS = 0.0;
};

// Position state-machine transition probabilities, each in [0, 1].
struct Lifecycle {
  double pOpen = 0.0;
  double pAdd = 0.0;
  double pPartialClose = 0.0;
  double pFullClose = 0.0;
};

// When the generator injects a self-funding top-up.
enum class FundingTrigger { BalanceBelow };

// The self-funding trigger / amount consumed by the generator.
struct Funding {
  FundingTrigger trigger = FundingTrigger::BalanceBelow;
  Decimal threshold;
  Decimal seed;
  Decimal topUp;
};

// The fully validated, strongly-typed representation of one INI file.
struct Config {
  std::string path;
  std::string hash; // hex-encoded SHA-256 of the raw file bytes.

  Run run;
  Arrival arrival;
  ReportDelay reportDelay;
  Reject reject;
  Accounts accounts;
  Concurrency concurrency;
  AsyncEngine asyncEngine;
  Instruments instruments;
  Lifecycle lifecycle;
  Funding funding;
  // Sorted by name for deterministic iteration; must contain at least one.
  std::vector<Cohort> cohorts;
};

// Reads path, validates required fields, computes a content hash, and returns a
// fully populated Config. Any validation failure throws ConfigError with
// context.
[[nodiscard]] Config Load(const std::string &path);

// Parses raw INI content (already read from disk) plus its content hash and the
// originating path, validating every section. Exposed so tests can drive the
// validator without a temp file. Throws ConfigError on any failure.
[[nodiscard]] Config LoadFromString(const std::string &content,
                                    const std::string &path,
                                    const std::string &hash);

} // namespace spot_loadtest::config
