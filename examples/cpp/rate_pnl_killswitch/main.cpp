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

#include "killswitch.hpp"

#include "openpit/engine.hpp"
#include "openpit/error.hpp"
#include "openpit/model.hpp"
#include "openpit/reject.hpp"

#include <chrono>
#include <cstdint>
#include <cstdio>
#include <iostream>
#include <string>
#include <vector>

namespace {

// =============================================================================
// Section 5 - The reactor.
// Plug your venue client and strategy book here.
// =============================================================================

// LoggingReactor prints rejects and kill-switch events to stdout. Production
// code routes these to your monitoring channel and to a strategy-halt signal.
class LoggingReactor final : public killswitch::Reactor {
public:
  explicit LoggingReactor(int rejectCap) : m_rejectCap(rejectCap) {}

  void OnAccepted(const openpit::model::Order &order) override {
    // In production: venue.SendOrder(order).
    static_cast<void>(order);
  }

  void
  OnRejected(const openpit::model::Order &order,
             const std::vector<openpit::pretrade::Reject> &rejects) override {
    static_cast<void>(order);
    // Cap noisy outputs in case a real run produces a long burst of rate-limit
    // rejects.
    if (m_rejectsPrinted >= m_rejectCap) {
      return;
    }
    for (const openpit::pretrade::Reject &item : rejects) {
      std::printf("rejected by %s [%d]: %s (%s)\n", item.policy.c_str(),
                  static_cast<int>(item.code), item.reason.c_str(),
                  item.details.c_str());
      m_rejectsPrinted++;
      if (m_rejectsPrinted >= m_rejectCap) {
        std::puts("... further rejects suppressed");
        return;
      }
    }
  }

  void OnReport(const openpit::model::ExecutionReport &report,
                const openpit::PostTradeResult &result) override {
    static_cast<void>(report);
    if (!result.accountBlocks.empty()) {
      std::puts("kill switch triggered - halt new orders until cleared");
    }
  }

private:
  int m_rejectsPrinted = 0;
  int m_rejectCap;
};

// Renders a duration the way Go's time.Duration.String() does, so the summary
// reads identically to the Go example. The values are wall-clock timings, so
// the exact figures vary run to run.
[[nodiscard]] std::string FormatDuration(std::chrono::nanoseconds value) {
  const std::int64_t ns = value.count();
  char buffer[64];
  if (ns >= 1'000'000'000) {
    std::snprintf(buffer, sizeof(buffer), "%gs",
                  static_cast<double>(ns) / 1'000'000'000.0);
  } else if (ns >= 1'000'000) {
    std::snprintf(buffer, sizeof(buffer), "%gms",
                  static_cast<double>(ns) / 1'000'000.0);
  } else if (ns >= 1'000) {
    std::snprintf(buffer, sizeof(buffer), "%gµs",
                  static_cast<double>(ns) / 1'000.0);
  } else {
    std::snprintf(buffer, sizeof(buffer), "%lldns", static_cast<long long>(ns));
  }
  return std::string(buffer);
}

// =============================================================================
// Section 6 - the integration flow.
// Read top-to-bottom for the integration flow.
// =============================================================================

void RunExample() {
  // Step 1 - declare the risk limits.
  const killswitch::Limits limits{
      /*settlementAsset=*/openpit::param::Asset(
          killswitch::kScenarioAssetSettle),
      /*pnlLowerBound=*/killswitch::kScenarioLowerBound,
      /*pnlUpperBound=*/killswitch::kScenarioUpperBound,
      /*maxOrdersBurst=*/killswitch::kScenarioMaxOrdersBurst,
      /*rateWindow=*/killswitch::kScenarioRateWindow,
  };

  // Step 2 - build the engine. Do this once at platform start-up. The engine is
  // RAII: it releases its state and policies when this scope exits.
  const openpit::Engine engine = killswitch::BuildEngine(limits);

  // Step 3 - assemble the event stream. In production this is your strategy +
  // venue listener; here it is a generator driven by the scenario constants
  // above.
  const openpit::model::Order order = killswitch::BuildOrder();
  const openpit::model::ExecutionReport report =
      killswitch::BuildReport(killswitch::kScenarioReportPnl);
  const openpit::model::ExecutionReport finalReport =
      killswitch::BuildReport(killswitch::kScenarioFinalReportPnl);
  killswitch::ScenarioStream stream(order, report, finalReport);

  // Step 4 - run the loop. Replace LoggingReactor with your venue client.
  constexpr int kExampleRejectCap = 4;
  LoggingReactor reactor(kExampleRejectCap);
  const killswitch::Stats stats = killswitch::Run(engine, stream, reactor);

  // Step 5 - report the outcome. In production you would push these to your
  // metrics backend.
  std::printf("\n--- run summary ---\n");
  const std::string settlementAsset(limits.settlementAsset.View());
  std::printf("pnl result   : %s %s\n", stats.pnl.ToString().c_str(),
              settlementAsset.c_str());
  std::printf("total trades : %d\n", stats.reports);
  std::printf("pre-trade avg: %s\n",
              FormatDuration(stats.AvgPreTrade()).c_str());
  std::printf("pre-trade min: %s\n", FormatDuration(stats.minPreTrade).c_str());
  std::printf("pre-trade max: %s\n", FormatDuration(stats.maxPreTrade).c_str());
  std::printf("pre-trade tot: %s\n",
              FormatDuration(stats.totalPreTrade).c_str());
  std::printf("accepted     : %d\n", stats.accepted);
  std::printf("rejected     : %d\n", stats.rejected);
  if (stats.killSwitch) {
    std::printf("kill switch  : TRIPPED on trade %d of %d\n",
                stats.killSwitchOnTrade, stats.reports);
  } else {
    std::puts("kill switch  : not triggered");
  }
}

} // namespace

int main() {
  try {
    RunExample();
  } catch (const openpit::Error &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
  return 0;
}
