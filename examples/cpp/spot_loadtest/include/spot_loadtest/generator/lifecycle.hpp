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
#include <map>
#include <string>

// Position lifecycle state machine.
//
// Chooses the next action from the current state so transitions are always
// valid (no close beyond the position, no add to a flat book).

namespace spot_loadtest::generator {

// The lifecycle transition chosen for one wake of an (account, instrument)
// pair.
enum class Action : std::uint8_t {
  Open,
  Add,
  PartialClose,
  FullClose,
  Idle,
};

class Lifecycle {
public:
  explicit Lifecycle(const config::Lifecycle &cfg) : m_cfg(cfg) {}

  // Picks a valid action for the pair given its current state, using the
  // configured transition probabilities (one weighted draw per decision).
  [[nodiscard]] Action Decide(Rng &g, const std::string &account,
                              const std::string &underlying);

  // Records lots added by an accepted Buy fill.
  void ApplyOpenOrAdd(const std::string &account, const std::string &underlying,
                      std::uint64_t lots);
  // Records lots removed by an accepted Sell fill (must not exceed position).
  void ApplyClose(const std::string &account, const std::string &underlying,
                  std::uint64_t lots);
  // The number of lots to sell for a partial or full close.
  [[nodiscard]] std::uint64_t CloseLots(Rng &g, const std::string &account,
                                        const std::string &underlying,
                                        bool full);

private:
  struct PosKey {
    std::string account;
    std::string underlying;
    [[nodiscard]] bool operator<(const PosKey &o) const {
      if (account != o.account) {
        return account < o.account;
      }
      return underlying < o.underlying;
    }
  };

  [[nodiscard]] std::uint64_t StateQty(const std::string &account,
                                       const std::string &underlying) const;

  config::Lifecycle m_cfg;
  std::map<PosKey, std::uint64_t> m_positions;
};

} // namespace spot_loadtest::generator
