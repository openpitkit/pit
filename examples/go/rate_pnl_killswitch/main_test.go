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
// Please see https://github.com/openpitkit and the OWNERS file for details.

package main

import (
	"math/big"
	"testing"

	"go.openpit.dev/openpit"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/reject"
)

// recordingReactor collects engine verdicts for assertion. It is the
// test-side counterpart of loggingReactor.
type recordingReactor struct {
	accepted     int
	rejectCodes  []reject.Code
	killSwitched bool
}

func (r *recordingReactor) OnAccepted(_ model.Order) { r.accepted++ }
func (r *recordingReactor) OnRejected(_ model.Order, rj []reject.Reject) {
	for _, rej := range rj {
		r.rejectCodes = append(r.rejectCodes, rej.Code)
	}
}
func (r *recordingReactor) OnReport(_ model.ExecutionReport, res openpit.PostTradeResult) {
	if len(res.AccountBlocks) > 0 {
		r.killSwitched = true
	}
}

// TestScenarioTripsBothKillswitches is the assertion-driven counterpart of
// main(). The scripted feed first trips the rate limit on the tail of the
// burst (a handful of "too frequent" rejects), then the final execution
// report pushes cumulative P&L below the floor and trips the kill switch.
func TestScenarioTripsBothKillswitches(t *testing.T) {
	engine, err := buildEngine(Limits{
		SettlementAsset: scenarioAssetSettle,
		PnlLowerBound:   scenarioLowerBound,
		PnlUpperBound:   scenarioUpperBound,
		MaxOrdersBurst:  scenarioMaxOrdersBurst,
		RateWindow:      scenarioRateWindow,
	})
	if err != nil {
		t.Fatalf("buildEngine: %v", err)
	}
	defer engine.Stop()

	react := &recordingReactor{}
	order := buildOrder()
	report := buildReport(scenarioReportPnl)
	finalReport := buildReport(scenarioFinalReportPnl)
	stats, err := Run(engine, newScenarioStream(&order, &report, &finalReport), react)
	if err != nil {
		t.Fatalf("Run: %v", err)
	}

	const (
		wantAccepted = scenarioMaxOrdersBurst
		wantRejected = scenarioAttempts - scenarioMaxOrdersBurst
		wantReports  = scenarioAcceptedReports
		wantPreTrade = scenarioAttempts
	)
	if stats.Accepted != wantAccepted {
		t.Errorf("accepted = %d, want %d", stats.Accepted, wantAccepted)
	}
	if stats.Rejected != wantRejected {
		t.Errorf("rejected = %d, want %d", stats.Rejected, wantRejected)
	}
	if stats.Reports != wantReports {
		t.Errorf("reports = %d, want %d", stats.Reports, wantReports)
	}
	if stats.PreTradeCalls != wantPreTrade {
		t.Errorf("pre-trade calls = %d, want %d", stats.PreTradeCalls, wantPreTrade)
	}

	if !stats.KillSwitch || !react.killSwitched {
		t.Error("kill switch must trip on the final report (cumulative pnl crosses the floor)")
	}
	if stats.KillSwitchOnTrade != scenarioAcceptedReports {
		t.Errorf("kill switch on trade = %d, want %d (the final trade)",
			stats.KillSwitchOnTrade, scenarioAcceptedReports)
	}

	// 999 * (-0.05) + (-460) = -509.95, just past the -500 floor.
	want, _ := new(big.Rat).SetString("-509.95")
	if stats.Pnl.Cmp(want) != 0 {
		t.Errorf("pnl = %s, want -509.95", stats.Pnl.FloatString(2))
	}

	// Every reject in the scenario must be a rate-limit reject: the burst
	// overshoots the ceiling within the same rate-limit window, so the tail
	// hits "too frequent".
	if got := len(react.rejectCodes); got != wantRejected {
		t.Fatalf("recorded reject codes = %d, want %d", got, wantRejected)
	}
	for _, c := range react.rejectCodes {
		if c != reject.CodeRateLimitExceeded {
			t.Errorf("reject code = %v, want CodeRateLimitExceeded", c)
		}
	}

	if stats.TotalPreTrade <= 0 {
		t.Error("total pre-trade duration must be positive")
	}
	if stats.MinPreTrade < 0 {
		t.Errorf("min pre-trade duration = %s, must not be negative", stats.MinPreTrade)
	}
	if stats.MaxPreTrade < stats.MinPreTrade {
		t.Errorf("max %s < min %s", stats.MaxPreTrade, stats.MinPreTrade)
	}
}
