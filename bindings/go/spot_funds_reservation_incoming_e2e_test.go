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

package openpit

import (
	"strings"
	"testing"

	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/optional"
	"go.openpit.dev/openpit/pretrade/policies"
	"go.openpit.dev/openpit/reject"
)

// TestSpotFundsReservationIncoming_BuyBaseIncomingSurfacesOnReservation proves
// that a buy reservation emits a base-asset entry carrying Incoming (delta and
// absolute equal to the ordered quantity) alongside the settlement-asset Held
// entry. The incoming bucket is populated during reservation and is accessible
// through Reservation.AccountAdjustments() without any new binding code.
func TestSpotFundsReservationIncoming_BuyBaseIncomingSurfacesOnReservation(t *testing.T) {
	accountID := param.NewAccountIDFromUint64(99224416)

	engine, err := NewEngineBuilder().
		FullSync().
		Builtin(policies.BuildSpotFunds()).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	// Seed 5000 USD so the buy reserve can proceed.
	seedAdj, err := model.NewAccountAdjustmentFromValues(model.AccountAdjustmentValues{
		BalanceOperation: optional.Some(
			model.NewAccountAdjustmentBalanceOperationFromValues(
				model.AccountAdjustmentBalanceOperationValues{
					Asset: optional.Some(sfReservationAsset(t, "USD")),
				},
			),
		),
		Amount: optional.Some(
			model.NewAccountAdjustmentAmountFromValues(model.AccountAdjustmentAmountValues{
				Balance: optional.Some(
					param.NewAbsoluteAdjustmentAmount(sfReservationPositionSize(t, "5000")),
				),
			}),
		),
	})
	if err != nil {
		t.Fatalf("NewAccountAdjustmentFromValues() error = %v", err)
	}

	seedResult, err := engine.ApplyAccountAdjustment(accountID, []model.AccountAdjustment{seedAdj})
	if err != nil {
		t.Fatalf("ApplyAccountAdjustment() error = %v", err)
	}
	if seedResult.BatchError.IsSet() {
		t.Fatalf("ApplyAccountAdjustment() reject = %v, want none", seedResult.BatchError)
	}

	// Buy 3 AAPL @ 100: holds 300 USD (settlement) and reserves 3 AAPL incoming
	// (base), so that the expected inflow is tracked from the moment of
	// reservation.
	order := model.NewOrder()
	op := order.EnsureOperationView()
	op.SetInstrument(param.NewInstrument(sfReservationAsset(t, "AAPL"), sfReservationAsset(t, "USD")))
	op.SetAccountID(accountID)
	op.SetSide(param.SideBuy)
	op.SetTradeAmount(param.NewQuantityTradeAmount(sfReservationQuantity(t, "3")))
	op.SetPrice(sfReservationPrice(t, "100"))

	reservation, rejects, err := engine.ExecutePreTrade(order)
	if err != nil {
		t.Fatalf("ExecutePreTrade() error = %v", err)
	}
	if rejects != nil {
		t.Fatalf("ExecutePreTrade() rejects = %v, want nil", rejects)
	}
	if reservation == nil {
		t.Fatal("ExecutePreTrade() reservation = nil, want non-nil")
	}
	defer reservation.RollbackAndClose()

	adjustments := reservation.AccountAdjustments()

	// Two entries expected: [0] settlement (USD) held, [1] base (AAPL) incoming.
	if len(adjustments) != 2 {
		t.Fatalf("AccountAdjustments() len = %d, want 2", len(adjustments))
	}

	// Entry 0: settlement asset USD with Held set and Incoming absent.
	usdEntry := adjustments[0].Entry
	if !usdEntry.Asset.Equal(sfReservationAsset(t, "USD")) {
		t.Fatalf(
			"adjustments[0].Entry.Asset = %q, want %q",
			usdEntry.Asset, "USD",
		)
	}
	usdHeld, ok := usdEntry.Held.Get()
	if !ok {
		t.Fatal("adjustments[0].Entry.Held is unset, want set for settlement leg")
	}
	// Delta for a buy is +held_amount (positive: held increased by 300).
	want300 := sfReservationPositionSize(t, "300")
	if !usdHeld.Delta.Equal(want300) {
		t.Fatalf(
			"USD held delta = %v, want %v (price 100 * qty 3)",
			usdHeld.Delta, want300,
		)
	}
	if usdEntry.Incoming.IsSet() {
		t.Fatal("adjustments[0].Entry.Incoming is set, want unset for settlement entry")
	}

	// Entry 1: base asset AAPL with Incoming set and Held absent.
	aaplEntry := adjustments[1].Entry
	if !aaplEntry.Asset.Equal(sfReservationAsset(t, "AAPL")) {
		t.Fatalf(
			"adjustments[1].Entry.Asset = %q, want %q",
			aaplEntry.Asset, "AAPL",
		)
	}
	aaplIncoming, ok := aaplEntry.Incoming.Get()
	if !ok {
		t.Fatal("adjustments[1].Entry.Incoming is unset, want set for base leg of buy")
	}
	// Delta equals the ordered quantity (3 AAPL reserved as incoming).
	want3 := sfReservationPositionSize(t, "3")
	if !aaplIncoming.Delta.Equal(want3) {
		t.Fatalf(
			"AAPL incoming delta = %v, want %v (ordered qty)",
			aaplIncoming.Delta, want3,
		)
	}
	if !aaplIncoming.Absolute.Equal(want3) {
		t.Fatalf(
			"AAPL incoming absolute = %v, want %v",
			aaplIncoming.Absolute, want3,
		)
	}
	if aaplEntry.Held.IsSet() {
		t.Fatal("adjustments[1].Entry.Held is set, want unset for base entry")
	}
}

// TestSpotFundsReservationIncoming_SellQuoteIncomingSurfacesOnReservation
// proves that a priced sell reservation emits a settlement-asset entry carrying
// Incoming (expected proceeds = price * qty) in addition to the underlying-asset
// Held entry. The incoming bucket on the settlement leg is accessible through
// Reservation.AccountAdjustments() without any new binding code.
func TestSpotFundsReservationIncoming_SellQuoteIncomingSurfacesOnReservation(t *testing.T) {
	accountID := param.NewAccountIDFromUint64(99224416)

	engine, err := NewEngineBuilder().
		FullSync().
		Builtin(policies.BuildSpotFunds()).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	// Seed 5 AAPL as available position so the sell can proceed.
	seedAdj, err := model.NewAccountAdjustmentFromValues(model.AccountAdjustmentValues{
		BalanceOperation: optional.Some(
			model.NewAccountAdjustmentBalanceOperationFromValues(
				model.AccountAdjustmentBalanceOperationValues{
					Asset: optional.Some(sfReservationAsset(t, "AAPL")),
				},
			),
		),
		Amount: optional.Some(
			model.NewAccountAdjustmentAmountFromValues(model.AccountAdjustmentAmountValues{
				Balance: optional.Some(
					param.NewAbsoluteAdjustmentAmount(sfReservationPositionSize(t, "5")),
				),
			}),
		),
	})
	if err != nil {
		t.Fatalf("NewAccountAdjustmentFromValues() error = %v", err)
	}

	seedResult, err := engine.ApplyAccountAdjustment(accountID, []model.AccountAdjustment{seedAdj})
	if err != nil {
		t.Fatalf("ApplyAccountAdjustment() error = %v", err)
	}
	if seedResult.BatchError.IsSet() {
		t.Fatalf("ApplyAccountAdjustment() reject = %v, want none", seedResult.BatchError)
	}

	// Sell 2 AAPL @ 150: holds 2 AAPL (underlying) and reserves 300 USD incoming
	// (settlement proceeds = 150 * 2).
	order := model.NewOrder()
	op := order.EnsureOperationView()
	op.SetInstrument(param.NewInstrument(sfReservationAsset(t, "AAPL"), sfReservationAsset(t, "USD")))
	op.SetAccountID(accountID)
	op.SetSide(param.SideSell)
	op.SetTradeAmount(param.NewQuantityTradeAmount(sfReservationQuantity(t, "2")))
	op.SetPrice(sfReservationPrice(t, "150"))

	reservation, rejects, err := engine.ExecutePreTrade(order)
	if err != nil {
		t.Fatalf("ExecutePreTrade() error = %v", err)
	}
	if rejects != nil {
		t.Fatalf("ExecutePreTrade() rejects = %v, want nil", rejects)
	}
	if reservation == nil {
		t.Fatal("ExecutePreTrade() reservation = nil, want non-nil")
	}
	defer reservation.RollbackAndClose()

	adjustments := reservation.AccountAdjustments()

	// Two entries expected: [0] underlying (AAPL) held, [1] settlement (USD) incoming.
	if len(adjustments) != 2 {
		t.Fatalf("AccountAdjustments() len = %d, want 2", len(adjustments))
	}

	// Entry 0: underlying asset AAPL with Held set and Incoming absent.
	aaplEntry := adjustments[0].Entry
	if !aaplEntry.Asset.Equal(sfReservationAsset(t, "AAPL")) {
		t.Fatalf(
			"adjustments[0].Entry.Asset = %q, want %q",
			aaplEntry.Asset, "AAPL",
		)
	}
	aaplHeld, ok := aaplEntry.Held.Get()
	if !ok {
		t.Fatal("adjustments[0].Entry.Held is unset, want set for underlying sell leg")
	}
	want2 := sfReservationPositionSize(t, "2")
	if !aaplHeld.Delta.Equal(want2) {
		t.Fatalf("AAPL held delta = %v, want %v (qty sold)", aaplHeld.Delta, want2)
	}
	if aaplEntry.Incoming.IsSet() {
		t.Fatal("adjustments[0].Entry.Incoming is set, want unset for underlying entry")
	}

	// Entry 1: settlement asset USD with Incoming set and Held absent (price > 0).
	usdEntry := adjustments[1].Entry
	if !usdEntry.Asset.Equal(sfReservationAsset(t, "USD")) {
		t.Fatalf(
			"adjustments[1].Entry.Asset = %q, want %q",
			usdEntry.Asset, "USD",
		)
	}
	usdIncoming, ok := usdEntry.Incoming.Get()
	if !ok {
		t.Fatal("adjustments[1].Entry.Incoming is unset, want set for settlement leg of priced sell")
	}
	// Expected proceeds: price 150 * qty 2 = 300 USD.
	want300 := sfReservationPositionSize(t, "300")
	if !usdIncoming.Delta.Equal(want300) {
		t.Fatalf(
			"USD incoming delta = %v, want %v (price 150 * qty 2)",
			usdIncoming.Delta, want300,
		)
	}
	if !usdIncoming.Absolute.Equal(want300) {
		t.Fatalf(
			"USD incoming absolute = %v, want %v",
			usdIncoming.Absolute, want300,
		)
	}
	if usdEntry.Held.IsSet() {
		t.Fatal("adjustments[1].Entry.Held is set, want unset for settlement incoming entry (price > 0)")
	}
}

// TestSpotFundsReservationIncoming_PricelessSellWithoutMarketDataBundleRejectsAtPreTrade
// proves that a quantity sell with no order price in limit-only mode is
// rejected at pre-trade with UnsupportedOrderType. Under the no-tolerance
// contract every accepted sell must resolve a price; a sell that cannot be
// priced is rejected rather than accepted with a dropped lock.
func TestSpotFundsReservationIncoming_PricelessSellWithoutMarketDataBundleRejectsAtPreTrade(t *testing.T) {
	accountID := param.NewAccountIDFromUint64(99224416)

	engine, err := NewEngineBuilder().
		FullSync().
		Builtin(policies.BuildSpotFunds()).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	// Seed 5 AAPL as available position.
	seedAdj, err := model.NewAccountAdjustmentFromValues(model.AccountAdjustmentValues{
		BalanceOperation: optional.Some(
			model.NewAccountAdjustmentBalanceOperationFromValues(
				model.AccountAdjustmentBalanceOperationValues{
					Asset: optional.Some(sfReservationAsset(t, "AAPL")),
				},
			),
		),
		Amount: optional.Some(
			model.NewAccountAdjustmentAmountFromValues(model.AccountAdjustmentAmountValues{
				Balance: optional.Some(
					param.NewAbsoluteAdjustmentAmount(sfReservationPositionSize(t, "5")),
				),
			}),
		),
	})
	if err != nil {
		t.Fatalf("NewAccountAdjustmentFromValues() error = %v", err)
	}

	seedResult, err := engine.ApplyAccountAdjustment(accountID, []model.AccountAdjustment{seedAdj})
	if err != nil {
		t.Fatalf("ApplyAccountAdjustment() error = %v", err)
	}
	if seedResult.BatchError.IsSet() {
		t.Fatalf("ApplyAccountAdjustment() reject = %v, want none", seedResult.BatchError)
	}

	// Sell 2 AAPL with no price in limit-only mode: must be rejected because
	// every accepted sell requires a resolvable price under the no-tolerance
	// contract.
	order := model.NewOrder()
	op := order.EnsureOperationView()
	op.SetInstrument(param.NewInstrument(sfReservationAsset(t, "AAPL"), sfReservationAsset(t, "USD")))
	op.SetAccountID(accountID)
	op.SetSide(param.SideSell)
	op.SetTradeAmount(param.NewQuantityTradeAmount(sfReservationQuantity(t, "2")))
	// No price set: must reject.

	reservation, rejects, err := engine.ExecutePreTrade(order)
	if err != nil {
		t.Fatalf("ExecutePreTrade() error = %v", err)
	}
	if reservation != nil {
		reservation.Close()
		t.Fatal("ExecutePreTrade() reservation != nil, want nil for unpriceable sell")
	}
	if len(rejects) != 1 {
		t.Fatalf("ExecutePreTrade() rejects len = %d, want 1", len(rejects))
	}
	if rejects[0].Code != reject.CodeUnsupportedOrderType {
		t.Fatalf(
			"reject code = %v, want %v",
			rejects[0].Code, reject.CodeUnsupportedOrderType,
		)
	}
}

// TestSpotFundsReservationIncoming_SellFillWithoutLockBlocksAccount proves that
// a sell fill arriving without its pre-trade lock blocks the account with
// MissingRequiredField and leaves no residual settlement incoming. This is the
// binding-level observable of the no-tolerance post-trade sell-lock contract.
func TestSpotFundsReservationIncoming_SellFillWithoutLockBlocksAccount(t *testing.T) {
	accountID := param.NewAccountIDFromUint64(99224416)

	engine, err := NewEngineBuilder().
		FullSync().
		Builtin(policies.BuildSpotFunds()).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	// Seed 5 AAPL so the sell can proceed.
	seedAdj, err := model.NewAccountAdjustmentFromValues(model.AccountAdjustmentValues{
		BalanceOperation: optional.Some(
			model.NewAccountAdjustmentBalanceOperationFromValues(
				model.AccountAdjustmentBalanceOperationValues{
					Asset: optional.Some(sfReservationAsset(t, "AAPL")),
				},
			),
		),
		Amount: optional.Some(
			model.NewAccountAdjustmentAmountFromValues(model.AccountAdjustmentAmountValues{
				Balance: optional.Some(
					param.NewAbsoluteAdjustmentAmount(sfReservationPositionSize(t, "5")),
				),
			}),
		),
	})
	if err != nil {
		t.Fatalf("NewAccountAdjustmentFromValues() error = %v", err)
	}

	seedResult, err := engine.ApplyAccountAdjustment(accountID, []model.AccountAdjustment{seedAdj})
	if err != nil {
		t.Fatalf("ApplyAccountAdjustment() error = %v", err)
	}
	if seedResult.BatchError.IsSet() {
		t.Fatalf("ApplyAccountAdjustment() reject = %v, want none", seedResult.BatchError)
	}

	// Sell 2 AAPL @ 150: priced sell, reservation succeeds and records a lock.
	order := model.NewOrder()
	op := order.EnsureOperationView()
	op.SetInstrument(param.NewInstrument(sfReservationAsset(t, "AAPL"), sfReservationAsset(t, "USD")))
	op.SetAccountID(accountID)
	op.SetSide(param.SideSell)
	op.SetTradeAmount(param.NewQuantityTradeAmount(sfReservationQuantity(t, "2")))
	op.SetPrice(sfReservationPrice(t, "150"))

	reservation, rejects, err := engine.ExecutePreTrade(order)
	if err != nil {
		t.Fatalf("ExecutePreTrade() error = %v", err)
	}
	if rejects != nil {
		t.Fatalf("ExecutePreTrade() rejects = %v, want nil", rejects)
	}
	if reservation == nil {
		t.Fatal("ExecutePreTrade() reservation = nil, want non-nil")
	}
	reservation.CommitAndClose()

	// Deliver the fill WITHOUT attaching the lock. The engine must block the
	// account with MissingRequiredField before touching any holdings.
	fillReport := model.NewExecutionReportFromValues(
		model.ExecutionReportValues{
			Operation: optional.Some(
				model.NewExecutionReportOperationFromValues(
					model.ExecutionReportOperationValues{
						Instrument: optional.Some(
							param.NewInstrument(
								sfReservationAsset(t, "AAPL"),
								sfReservationAsset(t, "USD"),
							),
						),
						AccountID: optional.Some(accountID),
						Side:      optional.Some(param.SideSell),
					},
				),
			),
			Fill: optional.Some(
				model.NewExecutionReportFillFromValues(
					model.ExecutionReportFillValues{
						LastTrade: optional.Some(
							model.NewExecutionReportTrade(
								sfReservationPrice(t, "150"),
								sfReservationQuantity(t, "2"),
							),
						),
						LeavesQuantity: optional.Some(sfReservationQuantity(t, "0")),
						// Lock intentionally absent: the engine must block the account.
						Lock:    nil,
						IsFinal: optional.BoolSome(true),
					},
				),
			),
		},
	)

	result, err := engine.ApplyExecutionReport(fillReport)
	if err != nil {
		t.Fatalf("ApplyExecutionReport() error = %v", err)
	}
	if len(result.AccountBlocks) != 1 {
		t.Fatalf(
			"AccountBlocks len = %d, want 1 (missing-lock sell fill must block account)",
			len(result.AccountBlocks),
		)
	}
	if result.AccountBlocks[0].Code != reject.CodeMissingRequiredField {
		t.Fatalf(
			"AccountBlocks[0].Code = %v, want %v",
			result.AccountBlocks[0].Code, reject.CodeMissingRequiredField,
		)
	}
}

// TestSpotFundsReservationIncoming_SellCancelWithoutLockBlocksAccount proves
// that a sell cancel (is_final, leaves_quantity > 0, no fill trade) arriving
// without its pre-trade lock blocks the account with MissingRequiredField,
// exactly like a sell fill. The lock is mandatory on both paths.
func TestSpotFundsReservationIncoming_SellCancelWithoutLockBlocksAccount(t *testing.T) {
	accountID := param.NewAccountIDFromUint64(99224416)

	engine, err := NewEngineBuilder().
		FullSync().
		Builtin(policies.BuildSpotFunds()).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	// Seed 5 AAPL so the sell can proceed.
	seedAdj, err := model.NewAccountAdjustmentFromValues(model.AccountAdjustmentValues{
		BalanceOperation: optional.Some(
			model.NewAccountAdjustmentBalanceOperationFromValues(
				model.AccountAdjustmentBalanceOperationValues{
					Asset: optional.Some(sfReservationAsset(t, "AAPL")),
				},
			),
		),
		Amount: optional.Some(
			model.NewAccountAdjustmentAmountFromValues(model.AccountAdjustmentAmountValues{
				Balance: optional.Some(
					param.NewAbsoluteAdjustmentAmount(sfReservationPositionSize(t, "5")),
				),
			}),
		),
	})
	if err != nil {
		t.Fatalf("NewAccountAdjustmentFromValues() error = %v", err)
	}

	seedResult, err := engine.ApplyAccountAdjustment(accountID, []model.AccountAdjustment{seedAdj})
	if err != nil {
		t.Fatalf("ApplyAccountAdjustment() error = %v", err)
	}
	if seedResult.BatchError.IsSet() {
		t.Fatalf("ApplyAccountAdjustment() reject = %v, want none", seedResult.BatchError)
	}

	// Sell 2 AAPL @ 150: priced sell, reservation succeeds and records a lock.
	order := model.NewOrder()
	op := order.EnsureOperationView()
	op.SetInstrument(param.NewInstrument(sfReservationAsset(t, "AAPL"), sfReservationAsset(t, "USD")))
	op.SetAccountID(accountID)
	op.SetSide(param.SideSell)
	op.SetTradeAmount(param.NewQuantityTradeAmount(sfReservationQuantity(t, "2")))
	op.SetPrice(sfReservationPrice(t, "150"))

	reservation, rejects, err := engine.ExecutePreTrade(order)
	if err != nil {
		t.Fatalf("ExecutePreTrade() error = %v", err)
	}
	if rejects != nil {
		t.Fatalf("ExecutePreTrade() rejects = %v, want nil", rejects)
	}
	if reservation == nil {
		t.Fatal("ExecutePreTrade() reservation = nil, want non-nil")
	}
	reservation.CommitAndClose()

	// Deliver a cancel (is_final, full leaves_quantity, no last_trade) WITHOUT
	// attaching the lock. The engine must block the account with
	// MissingRequiredField before releasing any reservation.
	cancelReport := model.NewExecutionReportFromValues(
		model.ExecutionReportValues{
			Operation: optional.Some(
				model.NewExecutionReportOperationFromValues(
					model.ExecutionReportOperationValues{
						Instrument: optional.Some(
							param.NewInstrument(
								sfReservationAsset(t, "AAPL"),
								sfReservationAsset(t, "USD"),
							),
						),
						AccountID: optional.Some(accountID),
						Side:      optional.Some(param.SideSell),
					},
				),
			),
			Fill: optional.Some(
				model.NewExecutionReportFillFromValues(
					model.ExecutionReportFillValues{
						// No LastTrade: this is a cancel, not a partial fill.
						LeavesQuantity: optional.Some(sfReservationQuantity(t, "2")),
						// Lock intentionally absent: the engine must block the account.
						Lock:    nil,
						IsFinal: optional.BoolSome(true),
					},
				),
			),
		},
	)

	result, err := engine.ApplyExecutionReport(cancelReport)
	if err != nil {
		t.Fatalf("ApplyExecutionReport() error = %v", err)
	}
	if len(result.AccountBlocks) != 1 {
		t.Fatalf(
			"AccountBlocks len = %d, want 1 (missing-lock sell cancel must block account)",
			len(result.AccountBlocks),
		)
	}
	if result.AccountBlocks[0].Code != reject.CodeMissingRequiredField {
		t.Fatalf(
			"AccountBlocks[0].Code = %v, want %v",
			result.AccountBlocks[0].Code, reject.CodeMissingRequiredField,
		)
	}
}

// TestSpotFundsReservationIncoming_InsufficientFundsRejectDoesNotLeakAccountID
// proves that an insufficient-funds reject surfaced through the Go binding
// carries no account id in its Reason or Details. The digit run 424242 is the
// sentinel account id; the order operands never contain it.
func TestSpotFundsReservationIncoming_InsufficientFundsRejectDoesNotLeakAccountID(t *testing.T) {
	accountID := param.NewAccountIDFromUint64(424242)

	engine, err := NewEngineBuilder().
		FullSync().
		Builtin(policies.BuildSpotFunds()).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	// Buy 4 AAPL @ 200 = 800 notional against an unfunded account: rejects with
	// insufficient funds.
	order := model.NewOrder()
	op := order.EnsureOperationView()
	op.SetInstrument(param.NewInstrument(sfReservationAsset(t, "AAPL"), sfReservationAsset(t, "USD")))
	op.SetAccountID(accountID)
	op.SetSide(param.SideBuy)
	op.SetTradeAmount(param.NewQuantityTradeAmount(sfReservationQuantity(t, "4")))
	op.SetPrice(sfReservationPrice(t, "200"))

	reservation, rejects, err := engine.ExecutePreTrade(order)
	if err != nil {
		t.Fatalf("ExecutePreTrade() error = %v", err)
	}
	if reservation != nil {
		reservation.Close()
		t.Fatal("ExecutePreTrade() reservation != nil, want nil for under-funded buy")
	}
	if len(rejects) != 1 {
		t.Fatalf("ExecutePreTrade() rejects len = %d, want 1", len(rejects))
	}
	if rejects[0].Code != reject.CodeInsufficientFunds {
		t.Fatalf("reject code = %v, want %v", rejects[0].Code, reject.CodeInsufficientFunds)
	}
	if strings.Contains(rejects[0].Reason, "424242") {
		t.Fatalf("reject reason leaked account id: %q", rejects[0].Reason)
	}
	if strings.Contains(rejects[0].Details, "424242") {
		t.Fatalf("reject details leaked account id: %q", rejects[0].Details)
	}
}

// --- Helpers ---

func sfReservationAsset(t *testing.T, symbol string) param.Asset {
	t.Helper()
	a, err := param.NewAsset(symbol)
	if err != nil {
		t.Fatalf("NewAsset(%q) error = %v", symbol, err)
	}
	return a
}

func sfReservationPositionSize(t *testing.T, value string) param.PositionSize {
	t.Helper()
	v, err := param.NewPositionSizeFromString(value)
	if err != nil {
		t.Fatalf("NewPositionSizeFromString(%q) error = %v", value, err)
	}
	return v
}

func sfReservationQuantity(t *testing.T, value string) param.Quantity {
	t.Helper()
	v, err := param.NewQuantityFromString(value)
	if err != nil {
		t.Fatalf("NewQuantityFromString(%q) error = %v", value, err)
	}
	return v
}

func sfReservationPrice(t *testing.T, value string) param.Price {
	t.Helper()
	v, err := param.NewPriceFromString(value)
	if err != nil {
		t.Fatalf("NewPriceFromString(%q) error = %v", value, err)
	}
	return v
}
