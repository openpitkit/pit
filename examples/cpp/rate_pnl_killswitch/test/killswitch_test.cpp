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

// Source: examples/cpp/rate_pnl_killswitch/README.md - Usage
//
// Assertion-driven counterpart of main(). The scripted feed first trips the
// rate limit on the tail of the burst (a handful of "too frequent" rejects),
// then the final execution report pushes cumulative P&L below the floor and
// trips the kill switch. Keep this mirror in sync with the README example.

#include "killswitch.hpp"

#include "openpit/engine.hpp"
#include "openpit/model.hpp"
#include "openpit/param.hpp"
#include "openpit/reject.hpp"

#include <gtest/gtest.h>

#include <chrono>
#include <vector>

namespace {

using openpit::pretrade::RejectCode;

// recordingReactor collects engine verdicts for assertion. It is the test-side
// counterpart of LoggingReactor.
class RecordingReactor final : public killswitch::Reactor {
public:
  int accepted = 0;
  std::vector<RejectCode> rejectCodes;
  bool killSwitched = false;

  void OnAccepted(const openpit::model::Order &order) override {
    static_cast<void>(order);
    accepted++;
  }

  void
  OnRejected(const openpit::model::Order &order,
             const std::vector<openpit::pretrade::Reject> &rejects) override {
    static_cast<void>(order);
    for (const openpit::pretrade::Reject &reject : rejects) {
      rejectCodes.push_back(reject.code);
    }
  }

  void OnReport(const openpit::model::ExecutionReport &report,
                const openpit::PostTradeResult &result) override {
    static_cast<void>(report);
    if (!result.accountBlocks.empty()) {
      killSwitched = true;
    }
  }
};

TEST(Scenario, TripsBothKillswitches) {
  const openpit::Engine engine = killswitch::BuildEngine(killswitch::Limits{
      /*settlementAsset=*/openpit::param::Asset(
          killswitch::kScenarioAssetSettle),
      /*pnlLowerBound=*/killswitch::kScenarioLowerBound,
      /*pnlUpperBound=*/killswitch::kScenarioUpperBound,
      /*maxOrdersBurst=*/killswitch::kScenarioMaxOrdersBurst,
      /*rateWindow=*/killswitch::kScenarioRateWindow,
  });

  RecordingReactor reactor;
  const openpit::model::Order order = killswitch::BuildOrder();
  const openpit::model::ExecutionReport report =
      killswitch::BuildReport(killswitch::kScenarioReportPnl);
  const openpit::model::ExecutionReport finalReport =
      killswitch::BuildReport(killswitch::kScenarioFinalReportPnl);
  killswitch::ScenarioStream stream(order, report, finalReport);
  const killswitch::Stats stats = killswitch::Run(engine, stream, reactor);

  constexpr int kWantAccepted =
      static_cast<int>(killswitch::kScenarioMaxOrdersBurst);
  constexpr int kWantRejected =
      killswitch::kScenarioAttempts -
      static_cast<int>(killswitch::kScenarioMaxOrdersBurst);
  constexpr int kWantReports = killswitch::kScenarioAcceptedReports;
  constexpr int kWantPreTrade = killswitch::kScenarioAttempts;

  EXPECT_EQ(stats.accepted, kWantAccepted);
  EXPECT_EQ(stats.rejected, kWantRejected);
  EXPECT_EQ(stats.reports, kWantReports);
  EXPECT_EQ(stats.preTradeCalls, kWantPreTrade);

  // Kill switch must trip on the final report (cumulative pnl crosses the
  // floor).
  EXPECT_TRUE(stats.killSwitch);
  EXPECT_TRUE(reactor.killSwitched);
  EXPECT_EQ(stats.killSwitchOnTrade, killswitch::kScenarioAcceptedReports);

  // 999 * (-0.05) + (-460) = -509.95, just past the -500 floor.
  EXPECT_EQ(stats.pnl, openpit::param::Pnl::FromString("-509.95"));

  // Every reject in the scenario must be a rate-limit reject: the burst
  // overshoots the ceiling within the same rate-limit window, so the tail hits
  // "too frequent".
  ASSERT_EQ(static_cast<int>(reactor.rejectCodes.size()), kWantRejected);
  for (const RejectCode code : reactor.rejectCodes) {
    EXPECT_EQ(code, RejectCode::RateLimitExceeded);
  }

  EXPECT_GT(stats.totalPreTrade, std::chrono::nanoseconds{0});
  EXPECT_GE(stats.minPreTrade, std::chrono::nanoseconds{0});
  EXPECT_GE(stats.maxPreTrade, stats.minPreTrade);
}

} // namespace
