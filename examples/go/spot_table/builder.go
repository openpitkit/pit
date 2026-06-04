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
	"fmt"

	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/optional"
	"go.openpit.dev/openpit/pretrade"
)

// accountID converts a free-form table account string to a stable
// engine-side AccountID. The engine hashes the string via FNV-1a; the
// runner keeps the source string for diagnostics.
func accountID(s string) (param.AccountID, error) {
	if s == "" {
		return param.AccountID{}, fmt.Errorf("account is required")
	}
	return param.NewAccountIDFromString(s)
}

// accountGroupID converts a free-form table group label to a stable
// engine-side AccountGroupID. The engine hashes the string via FNV-1a.
func accountGroupID(s string) (param.AccountGroupID, error) {
	if s == "" {
		return param.AccountGroupID{}, fmt.Errorf("group is required")
	}
	return param.NewAccountGroupIDFromString(s)
}

// parseInstrument turns "BASE/QUOTE" into an engine Instrument.
func parseInstrument(s string) (param.Instrument, error) {
	under, settle, err := splitInstrument(s)
	if err != nil {
		return param.Instrument{}, err
	}
	u, err := param.NewAsset(under)
	if err != nil {
		return param.Instrument{}, fmt.Errorf(
			"underlying %q: %w", under, err,
		)
	}
	s2, err := param.NewAsset(settle)
	if err != nil {
		return param.Instrument{}, fmt.Errorf(
			"settlement %q: %w", settle, err,
		)
	}
	return param.NewInstrument(u, s2), nil
}

// parseSide converts BUY/SELL to a Side enum.
func parseSide(s string) (param.Side, error) {
	switch s {
	case "BUY":
		return param.SideBuy, nil
	case "SELL":
		return param.SideSell, nil
	default:
		return 0, fmt.Errorf("side must be BUY or SELL, got %q", s)
	}
}

// buildSeedAdjustment turns a SEED row into an AccountAdjustment that
// the spot policy accepts as an absolute starting balance for asset.
func buildSeedAdjustment(row Row) (model.AccountAdjustment, error) {
	asset, err := param.NewAsset(row.Asset)
	if err != nil {
		return model.AccountAdjustment{}, fmt.Errorf(
			"asset %q: %w", row.Asset, err,
		)
	}
	amount, err := param.NewPositionSizeFromString(row.Amount)
	if err != nil {
		return model.AccountAdjustment{}, fmt.Errorf(
			"amount %q: %w", row.Amount, err,
		)
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
		return model.AccountAdjustment{}, err
	}
	return adj, nil
}

// buildTradeAmount turns an ORDER row's qty or volume cell into a TradeAmount.
// Exactly one of the two is set; the parser already enforced that, so this only
// converts the value that is present.
func buildTradeAmount(row Row) (param.TradeAmount, error) {
	if row.Volume != "" {
		volume, err := param.NewVolumeFromString(row.Volume)
		if err != nil {
			return param.TradeAmount{}, fmt.Errorf(
				"volume %q: %w", row.Volume, err,
			)
		}
		return param.NewVolumeTradeAmount(volume), nil
	}
	qty, err := param.NewQuantityFromString(row.Qty)
	if err != nil {
		return param.TradeAmount{}, fmt.Errorf("qty %q: %w", row.Qty, err)
	}
	return param.NewQuantityTradeAmount(qty), nil
}

// buildOrder turns an ORDER row into a model.Order. Empty price means
// market order; the trade amount is denominated by quantity or volume.
func buildOrder(row Row, acc param.AccountID) (model.Order, error) {
	inst, err := parseInstrument(row.Instrument)
	if err != nil {
		return model.Order{}, err
	}
	side, err := parseSide(row.Side)
	if err != nil {
		return model.Order{}, err
	}
	tradeAmount, err := buildTradeAmount(row)
	if err != nil {
		return model.Order{}, err
	}
	order := model.NewOrder()
	op := order.EnsureOperationView()
	op.SetInstrument(inst)
	op.SetAccountID(acc)
	op.SetSide(side)
	op.SetTradeAmount(tradeAmount)
	if row.Price != "" {
		price, err := param.NewPriceFromString(row.Price)
		if err != nil {
			return model.Order{}, fmt.Errorf("price %q: %w", row.Price, err)
		}
		op.SetPrice(price)
	}
	return order, nil
}

// buildFillReport turns a FILL row into a final ExecutionReport. The price
// column on a FILL is the lock price (limit price for limit orders, mark price
// for market orders). When it is empty the most recent quote pushed for the
// instrument is reused.
func buildFillReport(
	row Row, acc param.AccountID, feed *MarketFeed,
) (model.ExecutionReport, error) {
	inst, err := parseInstrument(row.Instrument)
	if err != nil {
		return model.ExecutionReport{}, err
	}
	side, err := parseSide(row.Side)
	if err != nil {
		return model.ExecutionReport{}, err
	}
	qty, err := param.NewQuantityFromString(row.Qty)
	if err != nil {
		return model.ExecutionReport{}, fmt.Errorf(
			"qty %q: %w", row.Qty, err,
		)
	}
	priceStr := row.Price
	if priceStr == "" {
		priceStr = feed.LatestPrice(row.Instrument)
	}
	if priceStr == "" {
		return model.ExecutionReport{}, fmt.Errorf(
			"FILL needs a price or a prior TICK for %s", row.Instrument,
		)
	}
	price, err := param.NewPriceFromString(priceStr)
	if err != nil {
		return model.ExecutionReport{}, fmt.Errorf(
			"price %q: %w", priceStr, err,
		)
	}
	fee, err := param.NewFeeFromString(zeroIfEmpty(row.Fee))
	if err != nil {
		return model.ExecutionReport{}, fmt.Errorf(
			"fee %q: %w", row.Fee, err,
		)
	}
	pnl, err := param.NewPnlFromString(zeroIfEmpty(row.Pnl))
	if err != nil {
		return model.ExecutionReport{}, fmt.Errorf(
			"pnl %q: %w", row.Pnl, err,
		)
	}
	leaves, err := param.NewQuantityFromString("0")
	if err != nil {
		return model.ExecutionReport{}, err
	}
	// The fill carries the pre-trade lock that ties it back to the reservation
	// the matching ORDER committed: one entry under the spot funds policy's
	// default group at the lock/reservation price. The bytes round-trip with
	// pretrade.Lock exactly as the engine's own reservation lock would.
	lock, err := pretrade.NewLockFromEntries([]pretrade.Entry{
		{PolicyGroupID: model.DefaultPolicyGroupID, Price: price},
	})
	if err != nil {
		return model.ExecutionReport{}, fmt.Errorf("build fill lock: %w", err)
	}
	return model.NewExecutionReportFromValues(
		model.ExecutionReportValues{
			Operation: optional.Some(
				model.NewExecutionReportOperationFromValues(
					model.ExecutionReportOperationValues{
						Instrument: optional.Some(inst),
						AccountID:  optional.Some(acc),
						Side:       optional.Some(side),
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
						Lock:           lock.Bytes(),
						IsFinal:        optional.BoolSome(true),
					},
				),
			),
		},
	), nil
}

func zeroIfEmpty(s string) string {
	if s == "" {
		return "0"
	}
	return s
}
