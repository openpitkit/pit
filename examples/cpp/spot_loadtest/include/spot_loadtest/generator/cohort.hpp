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

#include "spot_loadtest/config/config.hpp"
#include "spot_loadtest/generator/rng.hpp"

#include <cstdint>
#include <functional>
#include <memory>
#include <string>
#include <vector>

// The partitioned account/instrument universe.
//
// Account assignment is deterministic: accounts are dealt to cohorts in
// proportion to weight using a largest-remainder split, then laid out in cohort
// order, so account i's cohort depends only on the config (not on the RNG).

namespace spot_loadtest::generator {

// The runtime form of a configured cohort: the validated config plus
// precomputed selection tables.
struct Cohort {
  config::Cohort cfg;
  std::vector<double> sizeCum;   // cumulative weights over cfg.sizeWeights.
  std::vector<double> symbolCum; // cumulative symbol weights for the skew.
};

// One population member bound to a cohort.
struct Account {
  std::string id; // engine-facing account key string.
  int cohort = 0; // index into Population::cohorts.
};

class Population {
public:
  // Partitions accounts across the configured cohorts by weight and precomputes
  // every selection table. Throws std::runtime_error on a degenerate config.
  [[nodiscard]] static std::unique_ptr<Population>
  Build(const config::Config &cfg);

  // Draws one account index to admit into the active working set, weighted by
  // cohort admission weight; advances within a cohort to the next inactive
  // account, falling back to a global scan. Returns -1 only when all are
  // active.
  [[nodiscard]] int AdmitAccount(Rng &g,
                                 const std::function<bool(int)> &active) const;

  // A symbol index for the cohort using its skew table.
  [[nodiscard]] int PickSymbol(Rng &g, int cohortIdx) const;
  // The order quantity (lots) for the cohort using its size distribution.
  [[nodiscard]] std::uint64_t PickSize(Rng &g, int cohortIdx) const;

  [[nodiscard]] const std::vector<Cohort> &cohorts() const { return m_cohorts; }
  [[nodiscard]] const std::vector<Account> &accounts() const {
    return m_accounts;
  }
  [[nodiscard]] const std::vector<std::string> &symbols() const {
    return m_symbols;
  }

private:
  [[nodiscard]] int ScanInactive(const std::function<bool(int)> &active) const;

  std::vector<Cohort> m_cohorts;
  std::vector<double> m_cohortCum;
  std::vector<Account> m_accounts;
  std::vector<std::string> m_symbols;
  std::vector<double> m_rejectCum;
  std::vector<std::vector<int>> m_byCohort;
  std::vector<double> m_admitCum;

public:
  // The cumulative reject-propensity table, used by the reject controller to
  // pick which cohort absorbs a forced reject. (Exposed read-only.)
  [[nodiscard]] const std::vector<double> &rejectCum() const {
    return m_rejectCum;
  }
};

} // namespace spot_loadtest::generator
