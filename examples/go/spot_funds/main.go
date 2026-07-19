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

// Example spot_funds is the smallest end-to-end integration of OpenPit's
// built-in SpotFunds pre-trade policy: it shows how a buy order reserves
// settlement cash, how a second order is rejected because that cash is still
// held, and how a fill settles the held reservation.
//
// What is illustrated:
//
//   - building a limit-only engine with SpotFunds + OrderValidation
//   - seeding an account's available cash via ApplyAccountAdjustment
//   - the reservation mechanic: a committed BUY holds settlement funds, so a
//     follow-up BUY that needs the same cash is rejected with InsufficientFunds
//   - tying a fill back to its reservation by carrying the pre-trade lock on
//     the execution report, so SpotFunds settles the right held amount
//   - switching the policy to track-only at runtime so an order that would be
//     rejected for insufficient funds is instead recorded (available may go
//     negative)
//
// Audience: an integrator who wants to lift the SpotFunds call pattern into
// their own order/fill pipeline.
//
// What you typically change to adapt this example to your own application:
//
//  1. Engine policies - see buildEngine() below.
//  2. The seed balance and the orders - here they are hard-coded constants
//     chosen so the reservation mechanic is the lesson; your system feeds
//     real account state and strategy orders.
//  3. The print statements - replace them with your order-router and
//     fill-handler side effects.
//
// The example is deliberately flat: main() reads top-to-bottom as a story,
// and every engine call is factored into a small named helper that the smoke
// test reuses. For a table-driven / load-testing harness around the same
// policy, see ../spot_table.
package main

import (
	"fmt"
	"log"

	"go.openpit.dev/openpit"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/optional"
	"go.openpit.dev/openpit/pretrade/policies"
	"go.openpit.dev/openpit/reject"
)

// Scenario constants. The numbers are picked so the reservation is the whole
// point: two identical 60000-notional buys do not both fit inside a 100000
// balance, because the first one's funds stay held until it fills.
const (
	scenarioAccount     = uint64(99_224_416) // same account as rate_pnl_killswitch
	scenarioAssetTraded = "AAPL"             // underlying
	scenarioAssetSettle = "USD"              // settlement asset whose funds are reserved
	scenarioSeedFunds   = "100000"           // initial available USD
	scenarioOrderPrice  = "2000"             // limit price; also the lock/reservation price
	scenarioOrderQty    = "30"               // each buy is 30 * 2000 = 60000 USD notional
)

// Derived amounts used only in the narration below, named so the linter sees no
// magic numbers: one buy's notional (qty * price) and what stays available after
// the first buy's funds are held.
const (
	orderNotional      = 60_000 // scenarioOrderQty * scenarioOrderPrice
	availableAfterBuy1 = 40_000 // scenarioSeedFunds - orderNotional
)

func main() {
	if err := runExample(); err != nil {
		log.Fatal(err)
	}
}

// runExample is the linear integration story. It is split out from main() so
// that defer engine.Stop() runs before the process exits on error.
func runExample() error {
	account := param.NewAccountIDFromUint64(scenarioAccount)

	// Step 1 - build the engine. Limit-only SpotFunds plus OrderValidation;
	// do this once at platform start-up.
	engine, err := buildEngine()
	if err != nil {
		return err
	}
	defer engine.Stop()

	// Step 2 - seed the account's available settlement cash. SpotFunds has no
	// initial-balance builder option; the balance is established through the
	// account-adjustment pipeline, exactly as a deposit would be.
	if err := seedFunds(engine, account, scenarioSeedFunds); err != nil {
		return err
	}
	fmt.Printf("seeded account with %s %s available\n",
		scenarioSeedFunds, scenarioAssetSettle)

	// Step 3 - Buy #1: BUY 30 AAPL @ 2000 (60000 USD notional). It fits inside
	// the 100000 balance, so the pre-trade check accepts it. Committing the
	// reservation moves 60000 from available to held. We capture the
	// reservation's pre-trade lock first - the fill in Step 5 must carry it
	// back so SpotFunds settles this exact reservation.
	buy1, err := buildOrder(account)
	if err != nil {
		return err
	}
	lock1, rejects, err := placeOrder(engine, buy1)
	if err != nil {
		return err
	}
	if rejects != nil {
		return fmt.Errorf("buy #1 unexpectedly rejected: %s", describe(rejects))
	}
	fmt.Printf("buy #1 accepted: held %d %s, %d %s now available\n",
		orderNotional, scenarioAssetSettle, availableAfterBuy1, scenarioAssetSettle)

	// Step 4 - Buy #2: an identical BUY 30 AAPL @ 2000. This is the teaching
	// point. Only 40000 USD is available now (60000 is held by Buy #1), but the
	// order needs 60000, so SpotFunds rejects it with InsufficientFunds. A
	// rejected order produces no reservation - there is nothing to commit.
	buy2, err := buildOrder(account)
	if err != nil {
		return err
	}
	lock2, rejects, err := placeOrder(engine, buy2)
	if err != nil {
		return err
	}
	if lock2 != nil {
		return fmt.Errorf("buy #2 unexpectedly accepted")
	}
	if !containsCode(rejects, reject.CodeInsufficientFunds) {
		return fmt.Errorf("buy #2 rejected for the wrong reason: %s", describe(rejects))
	}
	fmt.Printf("buy #2 rejected: %s (held funds reduce what is available)\n", describe(rejects))

	// Step 5 - fill Buy #1 in full. The execution report carries the lock we
	// captured at commit time, so SpotFunds matches the fill to Buy #1's
	// reservation and settles the 60000 it was holding. No account block means
	// the settlement succeeded.
	fill, err := buildFillReport(account, lock1)
	if err != nil {
		return err
	}
	result, err := applyFill(engine, fill)
	if err != nil {
		return err
	}
	if len(result.AccountBlocks) > 0 {
		return fmt.Errorf("fill produced an unexpected account block")
	}
	fmt.Printf("buy #1 filled: %d %s reservation settled, no account block\n",
		orderNotional, scenarioAssetSettle)

	// Step 6 - switch the policy to track-only at runtime. In TrackOnly the
	// insufficient-funds gate is dropped: a reservation is always recorded and
	// available funds may go negative. After Step 5 only 40000 USD is back
	// available, yet an identical 60000-notional buy is now accepted instead of
	// rejected - the policy tracks the overshoot rather than blocking it.
	if err := enableTrackOnly(engine); err != nil {
		return err
	}
	buy3, err := buildOrder(account)
	if err != nil {
		return err
	}
	lock3, rejects, err := placeOrder(engine, buy3)
	if err != nil {
		return err
	}
	if lock3 == nil {
		return fmt.Errorf("buy #3 unexpectedly rejected: %s", describe(rejects))
	}
	fmt.Printf("buy #3 accepted in track-only: %d %s reserved, available may go negative\n",
		orderNotional, scenarioAssetSettle)

	return nil
}

// =============================================================================
// Shared helpers. main() and the smoke test both call these; each wraps one
// engine interaction so the flow above stays readable.
// =============================================================================

// buildEngine wires a limit-only engine with the SpotFunds policy. OrderValidation
// is registered first so the engine refuses malformed orders before SpotFunds
// sees them. SpotFunds is not given WithMarketOrders, so market orders (no limit
// price) are rejected with UnsupportedOrderType - this example only sends limit
// orders.
func buildEngine() (*openpit.Engine, error) {
	return openpit.NewEngineBuilder().
		FullSync().
		Builtin(policies.BuildOrderValidation()).
		Builtin(policies.BuildSpotFunds()).
		Build()
}

// seedFunds sets the account's available settlement balance to an absolute
// amount. An absolute adjustment overwrites the balance (unlike a relative
// delta), so it reads as "set available USD to funds".
func seedFunds(engine *openpit.Engine, account param.AccountID, funds string) error {
	asset, err := param.NewAsset(scenarioAssetSettle)
	if err != nil {
		return fmt.Errorf("settlement asset: %w", err)
	}
	amount, err := param.NewPositionSizeFromString(funds)
	if err != nil {
		return fmt.Errorf("seed amount %q: %w", funds, err)
	}
	adj, err := model.NewAccountAdjustmentFromValues(
		model.AccountAdjustmentValues{
			BalanceOperation: optional.Some(
				model.NewAccountAdjustmentBalanceOperationFromValues(
					model.AccountAdjustmentBalanceOperationValues{
						Asset: optional.Some(asset),
					},
				),
			),
			Amount: optional.Some(
				model.NewAccountAdjustmentAmountFromValues(
					model.AccountAdjustmentAmountValues{
						Balance: optional.Some(
							param.NewAbsoluteAdjustmentAmount(amount),
						),
					},
				),
			),
		},
	)
	if err != nil {
		return fmt.Errorf("build seed adjustment: %w", err)
	}
	result, err := engine.ApplyAccountAdjustment(
		account, []model.AccountAdjustment{adj},
	)
	if err != nil {
		return fmt.Errorf("apply seed adjustment: %w", err)
	}
	if result.BatchError.IsSet() {
		return fmt.Errorf("seed adjustment rejected")
	}
	return nil
}

// buildOrder assembles a BUY limit order for the scenario instrument. A real
// strategy builds this from a signal and current market data.
func buildOrder(account param.AccountID) (model.Order, error) {
	traded, err := param.NewAsset(scenarioAssetTraded)
	if err != nil {
		return model.Order{}, fmt.Errorf("traded asset: %w", err)
	}
	settle, err := param.NewAsset(scenarioAssetSettle)
	if err != nil {
		return model.Order{}, fmt.Errorf("settlement asset: %w", err)
	}
	price, err := param.NewPriceFromString(scenarioOrderPrice)
	if err != nil {
		return model.Order{}, fmt.Errorf("price %q: %w", scenarioOrderPrice, err)
	}
	qty, err := param.NewQuantityFromString(scenarioOrderQty)
	if err != nil {
		return model.Order{}, fmt.Errorf("qty %q: %w", scenarioOrderQty, err)
	}
	order := model.NewOrder()
	op := order.EnsureOperationView()
	op.SetInstrument(param.NewInstrument(traded, settle))
	op.SetAccountID(account)
	op.SetSide(param.SideBuy)
	op.SetTradeAmount(param.NewQuantityTradeAmount(qty))
	op.SetPrice(price)
	return order, nil
}

// placeOrder runs the pre-trade check for an order and, on accept, commits the
// reservation. It returns the committed reservation's pre-trade lock bytes so
// the caller can later attach them to the matching fill; on reject it returns
// nil lock bytes and the rejects. The lock MUST be read before CommitAndClose,
// because Reservation.Lock panics once the reservation is closed.
func placeOrder(
	engine *openpit.Engine, order model.Order,
) ([]byte, []reject.Reject, error) {
	reservation, rejects, err := engine.ExecutePreTrade(order)
	if err != nil {
		return nil, nil, fmt.Errorf("pre-trade: %w", err)
	}
	if rejects != nil {
		// A rejected order reserves nothing; there is no lock and nothing to
		// commit.
		return nil, rejects, nil
	}
	// Snapshot the lock the engine assigned to this reservation, then commit.
	// CommitAndClose moves the reserved settlement funds from available to
	// held; RollbackAndClose would release them instead.
	lock := reservation.Lock().Bytes()
	reservation.CommitAndClose()
	return lock, nil, nil
}

// buildFillReport assembles a full, final execution report for a buy order and
// attaches the pre-trade lock captured when its reservation was committed.
// Carrying that lock is what ties the fill back to the reservation: SpotFunds
// reads the lock to find which held funds to settle. Reusing the stored bytes
// is more faithful than rebuilding the lock - they are exactly what the engine
// produced - but an equivalent lock can be reconstructed with
// pretrade.NewLockFromEntries({DefaultPolicyGroupID, lockPrice}) when the
// caller did not keep the reservation's bytes (see ../spot_table).
func buildFillReport(
	account param.AccountID, lock []byte,
) (model.ExecutionReport, error) {
	traded, err := param.NewAsset(scenarioAssetTraded)
	if err != nil {
		return model.ExecutionReport{}, fmt.Errorf("traded asset: %w", err)
	}
	settle, err := param.NewAsset(scenarioAssetSettle)
	if err != nil {
		return model.ExecutionReport{}, fmt.Errorf("settlement asset: %w", err)
	}
	price, err := param.NewPriceFromString(scenarioOrderPrice)
	if err != nil {
		return model.ExecutionReport{}, fmt.Errorf(
			"price %q: %w", scenarioOrderPrice, err,
		)
	}
	qty, err := param.NewQuantityFromString(scenarioOrderQty)
	if err != nil {
		return model.ExecutionReport{}, fmt.Errorf(
			"qty %q: %w", scenarioOrderQty, err,
		)
	}
	// A full fill of a 30-lot order leaves nothing outstanding.
	leaves, err := param.NewQuantityFromString("0")
	if err != nil {
		return model.ExecutionReport{}, err
	}
	// Combined-mode impact: the fee is embedded in pnl, so both are zero for a
	// plain settlement. See the SpotFunds wiki page for the "separate" fee
	// convention.
	fee, err := param.NewFeeFromString("0")
	if err != nil {
		return model.ExecutionReport{}, err
	}
	pnl, err := param.NewPnlFromString("0")
	if err != nil {
		return model.ExecutionReport{}, err
	}
	return model.NewExecutionReportFromValues(
		model.ExecutionReportValues{
			Operation: optional.Some(
				model.NewExecutionReportOperationFromValues(
					model.ExecutionReportOperationValues{
						Instrument: optional.Some(param.NewInstrument(traded, settle)),
						AccountID:  optional.Some(account),
						Side:       optional.Some(param.SideBuy),
					},
				),
			),
			FinancialImpact: optional.Some(
				model.NewExecutionReportFinancialImpactFromValues(
					model.ExecutionReportFinancialImpactValues{
						Pnl: optional.Some(pnl),
						Fee: optional.Some(fee),
					},
				),
			),
			Fill: optional.Some(
				model.NewExecutionReportFillFromValues(
					model.ExecutionReportFillValues{
						LastTrade: optional.Some(
							model.NewExecutionReportTrade(price, qty),
						),
						LeavesQuantity: optional.Some(leaves),
						Lock:           lock,
						IsFinal:        optional.BoolSome(true),
					},
				),
			),
		},
	), nil
}

// applyFill feeds a completed execution report to the engine. The returned
// PostTradeResult.AccountBlocks is empty when settlement succeeds; a non-empty
// slice would mean a policy permanently blocked the account.
func applyFill(
	engine *openpit.Engine, report model.ExecutionReport,
) (openpit.PostTradeResult, error) {
	result, err := engine.ApplyExecutionReport(report)
	if err != nil {
		return openpit.PostTradeResult{}, fmt.Errorf("execution report: %w", err)
	}
	return result, nil
}

// enableTrackOnly switches the SpotFunds policy to global track-only mode at
// runtime. TrackOnly drops the insufficient-funds reject: reservations are
// always recorded and available funds may go negative. The change is applied
// through the runtime configurator, keyed by the policy's registration name.
func enableTrackOnly(engine *openpit.Engine) error {
	if err := engine.Configure().SpotFundsGlobalLimitMode(
		policies.SpotFundsPolicyName,
		policies.SpotFundsLimitModeTrackOnly,
	); err != nil {
		return fmt.Errorf("enable track-only: %w", err)
	}
	return nil
}

// containsCode reports whether the rejects include the given business code.
func containsCode(rejects []reject.Reject, want reject.Code) bool {
	for _, r := range rejects {
		if r.Code == want {
			return true
		}
	}
	return false
}

// describe renders rejects as "Reason (Details)" pairs for a one-line message.
func describe(rejects []reject.Reject) string {
	if len(rejects) == 0 {
		return "no rejects"
	}
	parts := make([]string, 0, len(rejects))
	for _, r := range rejects {
		parts = append(parts, fmt.Sprintf("%s (%s)", r.Reason, r.Details))
	}
	out := parts[0]
	for _, p := range parts[1:] {
		out += "; " + p
	}
	return out
}
