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

// Example rate_pnl_killswitch demonstrates how an algorithmic trading desk can
// wrap OpenPit's RateLimit and PnlBoundsKillswitch policies around a Go
// strategy so that a runaway strategy is halted before it floods the venue
// with orders or burns through the loss budget.
//
// What is illustrated:
//
//   - building an engine with two killswitch policies side-by-side
//   - feeding the engine via a single Event stream (orders + fills)
//   - separating venue/strategy side-effects behind a Reactor interface
//   - aggregating accepted/rejected counts, pre-trade latency, and
//     cumulative P&L over the run
//
// Audience: an algo trader who wants an independent supervisor that prevents
// the strategy from "going crazy".
//
// What you typically change to adapt this example to your own application:
//
//  1. Engine policies and limits - see buildEngine() below.
//  2. The order/report stream - the scenarioStream in main() is a one-shot
//     replay; real systems plug in a goroutine driven by venue and strategy
//     events.
//  3. The Reactor implementation - replace loggingReactor with code that
//     actually submits orders to the venue, updates your strategy book, and
//     halts the strategy when a kill switch account block is returned.
package main

import (
	"fmt"
	"log"
	"math/big"
	"time"

	"go.openpit.dev/openpit"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/optional"
	"go.openpit.dev/openpit/pretrade/policies"
	"go.openpit.dev/openpit/reject"
)

// =============================================================================
// Section 1 - Public extension points.
// These three interfaces and the Stats struct are what application code
// interacts with. Run() below is policy-agnostic; it only knows these types.
// =============================================================================

// Event is the discriminated input to the engine loop. Exactly one of Order
// or Report is non-nil. RealizedPnl carries the report's P&L decimal string
// so the example can track the running balance outside the engine -
// production code would read this from its strategy book instead.
type Event struct {
	Order       *model.Order
	Report      *model.ExecutionReport
	RealizedPnl string
}

// EventStream is the input feed of the engine loop. Implementations:
//
//   - production: wrap a select over the strategy's order channel and the
//     venue's execution-report channel into a single Next() call;
//   - test: back it by a scripted state machine, as scenarioStream below does.
type EventStream interface {
	// Next yields the next event. Returning ok=false ends the loop.
	Next() (Event, bool)
}

// Reactor receives every engine verdict and converts it into a side effect.
// This interface is where the trading application plugs in.
type Reactor interface {
	// OnAccepted fires once pre-trade has reserved and committed the order.
	// Production code calls venue.SendOrder(order) here.
	OnAccepted(order model.Order)

	// OnRejected fires when one or more policies refused the order. Inspect
	// rejects[i].Code to choose between retry / throttle / escalate.
	OnRejected(order model.Order, rejects []reject.Reject)

	// OnReport fires after the engine has consumed a venue execution report.
	// When result.AccountBlocks is non-empty, the engine has permanently
	// blocked this account - your strategy must stop sending orders for it
	// until operators clear the state.
	OnReport(report model.ExecutionReport, result openpit.PostTradeResult)
}

// Stats aggregates timing and trading outcomes over a run.
type Stats struct {
	Accepted          int           // orders that passed pre-trade
	Rejected          int           // orders refused by a policy
	PreTradeCalls     int           // total pre-trade attempts
	Reports           int           // execution reports applied
	KillSwitch        bool          // true once the kill switch ever tripped
	KillSwitchOnTrade int           // 1-based index of the report that tripped it, 0 if never
	Pnl               *big.Rat      // cumulative realized P&L from reports
	TotalPreTrade     time.Duration // wall time spent inside ExecutePreTrade
	MinPreTrade       time.Duration
	MaxPreTrade       time.Duration
}

// AvgPreTrade returns the mean ExecutePreTrade duration over the run.
func (s Stats) AvgPreTrade() time.Duration {
	if s.PreTradeCalls == 0 {
		return 0
	}
	return s.TotalPreTrade / time.Duration(s.PreTradeCalls)
}

// =============================================================================
// Section 2 - Engine wiring.
// The two killswitch policies and the engine builder. Tune the limits to your
// risk tolerance.
// =============================================================================

// Limits gathers the killswitch parameters in one place so the call site reads
// like a risk-policy declaration.
type Limits struct {
	SettlementAsset string        // settlement asset, e.g. "USD"
	PnlLowerBound   string        // loss floor as a signed decimal, e.g. "-500"
	PnlUpperBound   string        // profit-taking ceiling, e.g. "500"
	MaxOrdersBurst  uint          // orders allowed inside the rate window
	RateWindow      time.Duration // length of the rate-limit window
}

// buildEngine wires the engine with the two killswitch policies plus order
// validation. The combination answers a single question: "is my strategy
// trading too fast or losing too much?".
func buildEngine(l Limits) (*openpit.Engine, error) {
	asset, err := param.NewAsset(l.SettlementAsset)
	if err != nil {
		return nil, fmt.Errorf("settlement asset: %w", err)
	}
	lower, err := param.NewPnlFromString(l.PnlLowerBound)
	if err != nil {
		return nil, fmt.Errorf("pnl lower bound: %w", err)
	}
	upper, err := param.NewPnlFromString(l.PnlUpperBound)
	if err != nil {
		return nil, fmt.Errorf("pnl upper bound: %w", err)
	}

	return openpit.NewEngineBuilder().
		FullSync().
		// OrderValidation must be present so the engine refuses malformed
		// orders before the killswitch policies see them.
		Builtin(policies.BuildOrderValidation()).
		// PnL bounds halt the account permanently when realized P&L crosses
		// either edge of the corridor. Both bounds are optional - this
		// example configures both for completeness.
		Builtin(
			policies.BuildPnlBoundsKillswitch().BrokerBarriers(
				policies.PnlBoundsBrokerBarrier{
					SettlementAsset: asset,
					LowerBound:      optional.Some(lower),
					UpperBound:      optional.Some(upper),
				},
			),
		).
		// Rate limit catches a strategy stuck in a tight loop. The example
		// uses the broker (global) axis; see the Policies wiki page for
		// per-asset and per-account axes.
		Builtin(
			policies.BuildRateLimit().BrokerBarrier(
				policies.RateLimitBrokerBarrier{
					Limit: policies.RateLimit{
						MaxOrders: l.MaxOrdersBurst,
						Window:    l.RateWindow,
					},
				},
			),
		).
		Build()
}

// =============================================================================
// Section 3 - The engine loop.
// Run consumes the event stream, calls the engine, and notifies the reactor.
// This function is policy-agnostic - reuse it as-is in your code.
// =============================================================================

// Run drives the engine until the stream is exhausted. The engine is owned by
// the caller; Run does not stop it. Errors here are infrastructure failures,
// not business rejects (those go to Reactor.OnRejected).
func Run(engine *openpit.Engine, stream EventStream, react Reactor) (Stats, error) {
	stats := Stats{Pnl: new(big.Rat)}
	for {
		ev, ok := stream.Next()
		if !ok {
			return stats, nil
		}
		switch {
		case ev.Order != nil:
			if err := runPreTrade(engine, *ev.Order, &stats, react); err != nil {
				return stats, err
			}
		case ev.Report != nil:
			if err := runReport(engine, ev, &stats, react); err != nil {
				return stats, err
			}
		}
	}
}

func runPreTrade(engine *openpit.Engine, order model.Order, stats *Stats, react Reactor) error {
	start := time.Now()
	reservation, rejects, err := engine.ExecutePreTrade(order)
	elapsed := time.Since(start)

	stats.PreTradeCalls++
	stats.TotalPreTrade += elapsed
	if stats.PreTradeCalls == 1 || elapsed < stats.MinPreTrade {
		stats.MinPreTrade = elapsed
	}
	if elapsed > stats.MaxPreTrade {
		stats.MaxPreTrade = elapsed
	}

	if err != nil {
		return fmt.Errorf("pre-trade: %w", err)
	}
	if rejects != nil {
		stats.Rejected++
		react.OnRejected(order, rejects)
		return nil
	}
	// On accept, persist the reservation. CommitAndClose finalizes the
	// reserved state in one call; use RollbackAndClose to release the
	// reservation if you decide not to submit the order.
	reservation.CommitAndClose()
	stats.Accepted++
	react.OnAccepted(order)
	return nil
}

func runReport(engine *openpit.Engine, ev Event, stats *Stats, react Reactor) error {
	result, err := engine.ApplyExecutionReport(*ev.Report)
	if err != nil {
		return fmt.Errorf("execution report: %w", err)
	}
	stats.Reports++
	if ev.RealizedPnl != "" {
		// Inputs are short constants produced by the strategy/example, not
		// untrusted external data, so the documented Rat.SetString
		// memory-consumption advisory does not apply here.
		if r, ok := new(big.Rat).SetString(ev.RealizedPnl); ok {
			stats.Pnl.Add(stats.Pnl, r)
		}
	}
	if len(result.AccountBlocks) > 0 && !stats.KillSwitch {
		stats.KillSwitch = true
		stats.KillSwitchOnTrade = stats.Reports
	}
	react.OnReport(*ev.Report, result)
	return nil
}

// =============================================================================
// Section 4 - The scenario.
// A scripted feed that exercises the kill-switch policies. In your own
// application this is the place you delete entirely - your real strategy
// produces events.
// =============================================================================

const (
	// The burst overshoots the rate-limit ceiling by a few orders so the
	// policy rejects the tail of the burst. The accepted orders then
	// produce a stream of small-loss reports, and the final report
	// contributes a large loss that pushes cumulative P&L past the lower
	// bound and trips the kill switch on the last trade.
	scenarioAttempts        = 1_005
	scenarioMaxOrdersBurst  = 1_000
	scenarioAcceptedReports = 1_000
	scenarioAccount         = uint64(99_224_416)
	// 999 * (-0.05) + (-460) = -509.95 < -500 - the kill switch fires on
	// the final report; every earlier report keeps cumulative P&L well
	// inside the corridor (-49.95 at worst).
	scenarioReportPnl      = "-0.05"
	scenarioFinalReportPnl = "-460"
	scenarioLowerBound     = "-500"
	scenarioUpperBound     = "500"
	scenarioRateWindow     = 10 * time.Second
	scenarioOrderPrice     = "185"
	scenarioOrderQty       = "100"
	scenarioAssetTraded    = "AAPL"
	scenarioAssetSettle    = "USD"
)

// buildOrder returns a buy-AAPL order intent. A real strategy assembles this
// from a signal and current market data.
func buildOrder() model.Order {
	order := model.NewOrder()
	op := order.EnsureOperationView()
	traded, _ := param.NewAsset(scenarioAssetTraded)
	settle, _ := param.NewAsset(scenarioAssetSettle)
	op.SetInstrument(param.NewInstrument(traded, settle))
	op.SetAccountID(param.NewAccountIDFromUint64(scenarioAccount))
	op.SetSide(param.SideBuy)
	price, _ := param.NewPriceFromString(scenarioOrderPrice)
	qty, _ := param.NewQuantityFromString(scenarioOrderQty)
	op.SetTradeAmount(param.NewQuantityTradeAmount(qty))
	op.SetPrice(price)
	return order
}

// buildReport returns a combined-mode execution report. "Combined" means the
// fee is embedded in pnl, so the fee field is set to zero; see the Policies
// wiki page for the alternative "separate" convention.
func buildReport(pnl string) model.ExecutionReport {
	report := model.NewExecutionReport()
	op := model.NewExecutionReportOperation()
	traded, _ := param.NewAsset(scenarioAssetTraded)
	settle, _ := param.NewAsset(scenarioAssetSettle)
	op.SetInstrument(param.NewInstrument(traded, settle))
	op.SetAccountID(param.NewAccountIDFromUint64(scenarioAccount))
	op.SetSide(param.SideBuy)
	report.SetOperation(op)
	p, _ := param.NewPnlFromString(pnl)
	fee, _ := param.NewFeeFromString("0")
	impact := model.NewExecutionReportFinancialImpact()
	impact.SetPnl(p)
	impact.SetFee(fee)
	report.SetFinancialImpact(impact)
	return report
}

// scenarioStream is the scripted feed expressed as a small state machine:
// three counters walked in order - order attempts, then small-loss reports,
// then one kill-switch report. Replace this implementation with a
// channel-driven stream that selects over your strategy and venue feeds.
type scenarioStream struct {
	order       *model.Order
	smallReport *model.ExecutionReport
	finalReport *model.ExecutionReport
	attempts    int // remaining order attempts
	small       int // remaining small-loss reports
	final       int // 1 until the final kill-switch report has been emitted
}

// newScenarioStream is the only place the scenario table is configured.
func newScenarioStream(order *model.Order, small, final *model.ExecutionReport) *scenarioStream {
	return &scenarioStream{
		order:       order,
		smallReport: small,
		finalReport: final,
		attempts:    scenarioAttempts,
		small:       scenarioAcceptedReports - 1,
		final:       1,
	}
}

// Next returns the next scripted event.
func (s *scenarioStream) Next() (Event, bool) {
	switch {
	case s.attempts > 0:
		s.attempts--
		return Event{Order: s.order}, true
	case s.small > 0:
		s.small--
		return Event{Report: s.smallReport, RealizedPnl: scenarioReportPnl}, true
	case s.final > 0:
		s.final--
		return Event{Report: s.finalReport, RealizedPnl: scenarioFinalReportPnl}, true
	default:
		return Event{}, false
	}
}

// =============================================================================
// Section 5 - The reactor.
// Plug your venue client and strategy book here.
// =============================================================================

// loggingReactor prints rejects and kill-switch events to stdout. Production
// code routes these to your monitoring channel and to a strategy-halt signal.
type loggingReactor struct {
	rejectsPrinted int
	rejectCap      int
}

func (*loggingReactor) OnAccepted(_ model.Order) {
	// In production: venue.SendOrder(order).
}

func (r *loggingReactor) OnRejected(_ model.Order, rj []reject.Reject) {
	// Cap noisy outputs in case a real run produces a long burst of
	// rate-limit rejects.
	if r.rejectsPrinted >= r.rejectCap {
		return
	}
	for _, item := range rj {
		fmt.Printf("rejected by %s [%d]: %s (%s)\n", item.Policy, item.Code, item.Reason, item.Details)
		r.rejectsPrinted++
		if r.rejectsPrinted >= r.rejectCap {
			fmt.Println("... further rejects suppressed")
			return
		}
	}
}

func (*loggingReactor) OnReport(_ model.ExecutionReport, res openpit.PostTradeResult) {
	if len(res.AccountBlocks) > 0 {
		fmt.Println("kill switch triggered - halt new orders until cleared")
	}
}

// =============================================================================
// Section 6 - main().
// The application entry point. Read top-to-bottom for the integration flow.
// =============================================================================

func main() {
	if err := runExample(); err != nil {
		log.Fatal(err)
	}
}

// runExample is the integration flow. It is split out from main() so that
// defer engine.Stop() runs before the process exits on error.
func runExample() error {
	// Step 1 - declare the risk limits.
	limits := Limits{
		SettlementAsset: scenarioAssetSettle,
		PnlLowerBound:   scenarioLowerBound,
		PnlUpperBound:   scenarioUpperBound,
		MaxOrdersBurst:  scenarioMaxOrdersBurst,
		RateWindow:      scenarioRateWindow,
	}

	// Step 2 - build the engine. Do this once at platform start-up.
	engine, err := buildEngine(limits)
	if err != nil {
		return err
	}
	defer engine.Stop()

	// Step 3 - assemble the event stream. In production this is your
	// strategy + venue listener; here it is a generator driven by the
	// scenario constants above.
	order := buildOrder()
	report := buildReport(scenarioReportPnl)
	finalReport := buildReport(scenarioFinalReportPnl)
	stream := newScenarioStream(&order, &report, &finalReport)

	// Step 4 - run the loop. Replace loggingReactor with your venue client.
	const exampleRejectCap = 4
	stats, err := Run(engine, stream, &loggingReactor{rejectCap: exampleRejectCap})
	if err != nil {
		return err
	}

	// Step 5 - report the outcome. In production you would push these to
	// your metrics backend.
	fmt.Printf("\n--- run summary ---\n")
	fmt.Printf("pnl result   : %s %s\n", stats.Pnl.FloatString(2), limits.SettlementAsset)
	fmt.Printf("total trades : %d\n", stats.Reports)
	fmt.Printf("pre-trade avg: %s\n", stats.AvgPreTrade())
	fmt.Printf("pre-trade min: %s\n", stats.MinPreTrade)
	fmt.Printf("pre-trade max: %s\n", stats.MaxPreTrade)
	fmt.Printf("pre-trade tot: %s\n", stats.TotalPreTrade)
	fmt.Printf("accepted     : %d\n", stats.Accepted)
	fmt.Printf("rejected     : %d\n", stats.Rejected)
	if stats.KillSwitch {
		fmt.Printf("kill switch  : TRIPPED on trade %d of %d\n",
			stats.KillSwitchOnTrade, stats.Reports)
	} else {
		fmt.Println("kill switch  : not triggered")
	}
	return nil
}
