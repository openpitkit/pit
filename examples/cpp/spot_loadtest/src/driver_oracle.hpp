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
#include "spot_loadtest/generator/event.hpp"

#include "openpit/accountadjustment/account_adjustment.hpp"
#include "openpit/reject.hpp"

#include <cstdint>
#include <map>
#include <mutex>
#include <optional>
#include <string>
#include <vector>

// The strict per-op oracle (mirror of oracle.go).
//
// Checks every engine response against the generator's prediction and (at end
// of run) the aggregate fund-conservation / no-oversell invariants. Safe for
// concurrent use by the collector pool. The first divergence is recorded and
// returned; checking continues so a run reports a coherent total, but the run
// is considered failed once an error is set.

namespace spot_loadtest::driver::detail {

// The engine's response to one order-check.
struct OrderObservation {
  bool accepted = false;
  std::vector<::openpit::reject::Reject> rejects;
};

// The engine's response to one settlement.
struct SettleObservation {
  bool blocked = false;
  std::vector<::openpit::accountadjustment::Outcome> outcomes;
};

// The engine's response to one funding adjustment.
struct FundingObservation {
  bool rejected = false;
  std::vector<::openpit::accountadjustment::Outcome> outcomes;
};

class Oracle {
public:
  Oracle() = default;

  // The first divergence seen, or empty if the engine agreed with everything.
  [[nodiscard]] std::optional<std::string> Err() {
    std::lock_guard<std::mutex> lock(m_mutex);
    return m_firstErr;
  }

  // Records a non-prediction failure (build / transport / cancellation error).
  void FailExternal(const std::string &err) {
    std::lock_guard<std::mutex> lock(m_mutex);
    Fail(err);
  }

  // Verifies an order-check outcome against the event's prediction (decision +
  // reject code; balances are not observable on the order path).
  void CheckOrder(const generator::Event &ev, const OrderObservation &obs);
  // Verifies a settlement outcome: a clean settle plus the engine-volunteered
  // post-op balances of the affected legs.
  void CheckSettlement(const generator::Event &ev,
                       const SettleObservation &obs);
  // Verifies a funding adjustment: the decision and, on accept, the post-op
  // balance of the funded asset.
  void CheckFunding(const generator::Event &ev, const FundingObservation &obs);

  // Asserts the aggregate fund-conservation and no-oversell invariants over the
  // predicted end state. Called once after the run drains. Returns an error
  // string on a divergence.
  [[nodiscard]] std::optional<std::string>
  CheckInvariants(const std::vector<generator::Event> &events);

private:
  struct AssetKey {
    std::string account;
    std::string asset;
    [[nodiscard]] bool operator<(const AssetKey &o) const {
      if (account != o.account) {
        return account < o.account;
      }
      return asset < o.asset;
    }
  };
  struct Holdings {
    Decimal available;
    Decimal held;
  };

  void Fail(const std::string &err) {
    if (!m_firstErr) {
      m_firstErr = err;
    }
  }

  std::mutex m_mutex;
  std::optional<std::string> m_firstErr;
  std::uint64_t m_checked = 0;
};

} // namespace spot_loadtest::driver::detail
