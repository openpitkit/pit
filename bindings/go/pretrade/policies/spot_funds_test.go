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

package policies_test

import (
	"errors"
	"fmt"
	"strings"
	"testing"

	openpit "go.openpit.dev/openpit"
	"go.openpit.dev/openpit/accountadjustment"
	"go.openpit.dev/openpit/configure"
	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/marketdata"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/optional"
	"go.openpit.dev/openpit/pretrade"
	"go.openpit.dev/openpit/pretrade/policies"
	"go.openpit.dev/openpit/reject"
)

func mustMarketDataService(t *testing.T) *marketdata.Service {
	t.Helper()
	eb := openpit.NewEngineBuilder().FullSync()
	service, err := eb.MarketData(marketdata.InfiniteTTL()).Build()
	if err != nil {
		t.Fatalf("marketdata Build() error = %v", err)
	}
	return service
}

func mustAccountGroupID(t *testing.T, id uint32) param.AccountGroupID {
	t.Helper()
	g, err := param.NewAccountGroupIDFromUint32(id)
	if err != nil {
		t.Fatalf("NewAccountGroupIDFromUint32(%d) error = %v", id, err)
	}
	return g
}

func mustAsset(t *testing.T, symbol string) param.Asset {
	t.Helper()
	asset, err := param.NewAsset(symbol)
	if err != nil {
		t.Fatalf("NewAsset(%q) error = %v", symbol, err)
	}
	return asset
}

func mustPnl(t *testing.T, value string) param.Pnl {
	t.Helper()
	pnl, err := param.NewPnlFromString(value)
	if err != nil {
		t.Fatalf("NewPnlFromString(%q) error = %v", value, err)
	}
	return pnl
}

func mustPositionSize(t *testing.T, value string) param.PositionSize {
	t.Helper()
	positionSize, err := param.NewPositionSizeFromString(value)
	if err != nil {
		t.Fatalf("NewPositionSizeFromString(%q) error = %v", value, err)
	}
	return positionSize
}

func mustQuantity(t *testing.T, value string) param.Quantity {
	t.Helper()
	quantity, err := param.NewQuantityFromString(value)
	if err != nil {
		t.Fatalf("NewQuantityFromString(%q) error = %v", value, err)
	}
	return quantity
}

// mustPrice100 returns the fixed price "100" used by tests that need a price
// but do not care about its value.
func mustPrice100(t *testing.T) param.Price {
	t.Helper()
	price, err := param.NewPriceFromString("100")
	if err != nil {
		t.Fatalf("NewPriceFromString(100) error = %v", err)
	}
	return price
}

func spotFundsAbsoluteBalanceAdjustment(t *testing.T, asset param.Asset, amount string) model.AccountAdjustment {
	t.Helper()
	adjustment, err := model.NewAccountAdjustmentFromValues(model.AccountAdjustmentValues{
		BalanceOperation: optional.Some(
			model.NewAccountAdjustmentBalanceOperationFromValues(
				model.AccountAdjustmentBalanceOperationValues{
					Asset: optional.Some(asset),
				},
			),
		),
		Amount: optional.Some(
			model.NewAccountAdjustmentAmountFromValues(model.AccountAdjustmentAmountValues{
				Balance: optional.Some(
					param.NewAbsoluteAdjustmentAmount(mustPositionSize(t, amount)),
				),
			}),
		),
	})
	if err != nil {
		t.Fatalf("NewAccountAdjustmentFromValues() error = %v", err)
	}
	return adjustment
}

func seedSpotFundsLifecycleAccount(
	t *testing.T,
	engine *openpit.Engine,
	account param.AccountID,
	asset param.Asset,
) {
	t.Helper()
	if err := engine.Accounts().SetCurrency(account, asset); err != nil {
		t.Fatalf("Accounts().SetCurrency() error = %v", err)
	}
	adjustment := spotFundsAbsoluteBalanceAdjustment(t, asset, "1000")
	result, err := engine.ApplyAccountAdjustment(
		account,
		[]model.AccountAdjustment{adjustment},
	)
	if err != nil {
		t.Fatalf("ApplyAccountAdjustment() error = %v", err)
	}
	if result.BatchError.IsSet() {
		t.Fatalf("ApplyAccountAdjustment() batch error = %v, want none", result.BatchError)
	}
}

func spotFundsLifecycleOrder(t *testing.T, account param.AccountID) model.Order {
	t.Helper()
	order := model.NewOrder()
	operation := order.EnsureOperationView()
	operation.SetInstrument(param.NewInstrument(mustAsset(t, "AAPL"), mustAsset(t, "USD")))
	operation.SetAccountID(account)
	operation.SetSide(param.SideBuy)
	operation.SetTradeAmount(param.NewQuantityTradeAmount(mustQuantity(t, "1")))
	operation.SetPrice(mustPrice100(t))
	return order
}

func applySpotFundsLifecycleFill(
	t *testing.T,
	engine *openpit.Engine,
	account param.AccountID,
) openpit.PostTradeResult {
	return applySpotFundsLifecycleFillWithFee(
		t,
		engine,
		account,
		optional.None[param.MonetaryAmount](),
	)
}

func applySpotFundsLifecycleFillWithFee(
	t *testing.T,
	engine *openpit.Engine,
	account param.AccountID,
	fee optional.Option[param.MonetaryAmount],
) openpit.PostTradeResult {
	t.Helper()
	order := spotFundsLifecycleOrder(t, account)
	reservation, rejects, err := engine.ExecutePreTrade(order)
	if err != nil {
		t.Fatalf("ExecutePreTrade() error = %v", err)
	}
	if len(rejects) != 0 {
		t.Fatalf("ExecutePreTrade() rejects = %v, want none", rejects)
	}
	if reservation == nil {
		t.Fatal("ExecutePreTrade() reservation = nil, want non-nil")
	}
	lock, err := reservation.Lock()
	if err != nil {
		t.Fatalf("Reservation.Lock() error = %v", err)
	}
	reservation.CommitAndClose()

	report := model.NewExecutionReport()
	reportOperation := model.NewExecutionReportOperation()
	reportOperation.SetInstrument(
		param.NewInstrument(mustAsset(t, "AAPL"), mustAsset(t, "USD")),
	)
	reportOperation.SetAccountID(account)
	reportOperation.SetSide(param.SideBuy)
	report.SetOperation(reportOperation)
	fill := report.EnsureFillView()
	fill.SetLastTrade(model.NewExecutionReportTrade(mustPrice100(t), mustQuantity(t, "1")))
	if value, ok := fee.Get(); ok {
		fill.SetFee(value)
	}
	fill.SetRemainingReservedQuantity(mustQuantity(t, "0"))
	if err := fill.SetLock(lock.Bytes()); err != nil {
		t.Fatalf("SetLock() error = %v", err)
	}
	fill.SetIsFinal(true)

	result, err := engine.ApplyExecutionReport(report)
	if err != nil {
		t.Fatalf("ApplyExecutionReport() error = %v", err)
	}
	return result
}

func directSpotFundsFillReport(
	t *testing.T,
	account param.AccountID,
) model.ExecutionReport {
	t.Helper()
	report := model.NewExecutionReport()
	operation := model.NewExecutionReportOperation()
	operation.SetInstrument(
		param.NewInstrument(mustAsset(t, "AAPL"), mustAsset(t, "USD")),
	)
	operation.SetAccountID(account)
	operation.SetSide(param.SideBuy)
	report.SetOperation(operation)
	fill := report.EnsureFillView()
	fill.SetLastTrade(
		model.NewExecutionReportTrade(mustPrice100(t), mustQuantity(t, "1")),
	)
	lock, err := pretrade.NewLockFromEntries([]pretrade.Entry{
		{PolicyGroupID: model.DefaultPolicyGroupID, Price: mustPrice100(t)},
	})
	if err != nil {
		t.Fatalf("NewLockFromEntries() error = %v", err)
	}
	fill.SetRemainingReservedQuantity(mustQuantity(t, "0"))
	if err := fill.SetLock(lock.Bytes()); err != nil {
		t.Fatalf("SetLock() error = %v", err)
	}
	fill.SetIsFinal(true)
	return report
}

func realizedPnlForAsset(
	result openpit.PostTradeResult,
	asset param.Asset,
) (accountadjustment.PnlOutcome, bool) {
	for _, outcome := range result.AccountAdjustments {
		if outcome.Entry.Asset.Equal(asset) {
			return outcome.Entry.RealizedPnl.Get()
		}
	}
	return accountadjustment.PnlOutcome{}, false
}

func forceSpotFundsPositionPnl(
	t *testing.T,
	engine *openpit.Engine,
	account param.AccountID,
	asset param.Asset,
) {
	t.Helper()
	basisAdjustment, err := model.NewAccountAdjustmentFromValues(
		model.AccountAdjustmentValues{
			BalanceOperation: optional.Some(
				model.NewAccountAdjustmentBalanceOperationFromValues(
					model.AccountAdjustmentBalanceOperationValues{
						Asset:             optional.Some(asset),
						AverageEntryPrice: optional.Some(mustPrice100(t)),
					},
				),
			),
		},
	)
	if err != nil {
		t.Fatalf("NewAccountAdjustmentFromValues(basis) error = %v", err)
	}
	pnlAdjustment, err := model.NewAccountAdjustmentFromValues(
		model.AccountAdjustmentValues{
			BalanceOperation: optional.Some(
				model.NewAccountAdjustmentBalanceOperationFromValues(
					model.AccountAdjustmentBalanceOperationValues{
						Asset:       optional.Some(asset),
						RealizedPnl: optional.Some(model.NewPnlState(mustPnl(t, "0"))),
					},
				),
			),
		},
	)
	if err != nil {
		t.Fatalf("NewAccountAdjustmentFromValues(PnL) error = %v", err)
	}
	result, err := engine.ApplyAccountAdjustment(
		account,
		[]model.AccountAdjustment{basisAdjustment, pnlAdjustment},
	)
	if err != nil {
		t.Fatalf("ApplyAccountAdjustment() error = %v", err)
	}
	if result.BatchError.IsSet() {
		t.Fatalf("ApplyAccountAdjustment() batch error = %v", result.BatchError)
	}
}

func mustFee(t *testing.T, value string) param.Fee {
	t.Helper()
	fee, err := param.NewFeeFromString(value)
	if err != nil {
		t.Fatalf("NewFeeFromString(%q) error = %v", value, err)
	}
	return fee
}

func assertSpotFundsPnlPreTradeReject(
	t *testing.T,
	engine *openpit.Engine,
	account param.AccountID,
) {
	t.Helper()
	reservation, rejects, err := engine.ExecutePreTrade(spotFundsLifecycleOrder(t, account))
	if err != nil {
		t.Fatalf("ExecutePreTrade() error = %v", err)
	}
	if reservation != nil {
		reservation.RollbackAndClose()
		t.Fatal("ExecutePreTrade() reservation is set for an out-of-bounds account")
	}
	if len(rejects) != 1 || rejects[0].Code != reject.CodePnlKillSwitchTriggered {
		t.Fatalf("ExecutePreTrade() rejects = %v, want one PnL kill-switch reject", rejects)
	}
}

func TestSpotFundsBuilderLimitOnlyMode(t *testing.T) {
	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFunds()).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	engine.Stop()
}

func TestSpotFundsBuilderWithMarketOrders(t *testing.T) {
	service := mustMarketDataService(t)
	defer service.Close()

	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFunds().WithMarketOrders(service, 2000)).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	engine.Stop()
}

func TestSpotFundsBuilderWithMarketOrdersZeroSlippage(t *testing.T) {
	service := mustMarketDataService(t)
	defer service.Close()

	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFunds().WithMarketOrders(service, 0)).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	engine.Stop()
}

func TestSpotFundsBuilderWithMarketOrdersMaxSlippageAccepted(t *testing.T) {
	service := mustMarketDataService(t)
	defer service.Close()

	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFunds().WithMarketOrders(service, 10_000)).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	engine.Stop()
}

func TestSpotFundsBuilderBookTopWithInstrumentOverrides(t *testing.T) {
	service := mustMarketDataService(t)
	defer service.Close()

	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFunds().
			WithMarketOrders(service, 1500).
			PricingSource(policies.SpotFundsPricingSourceBookTop).
			Overrides(
				policies.SpotFundsOverrideEntry{
					Target:   policies.SpotFundsOverrideTargetInstrument{Instrument: marketdata.NewInstrumentIDFromUint64(1)},
					Override: policies.SpotFundsOverride{SlippageBps: optional.Some[uint16](500)},
				},
				policies.SpotFundsOverrideEntry{
					Target:   policies.SpotFundsOverrideTargetInstrument{Instrument: marketdata.NewInstrumentIDFromUint64(2)},
					Override: policies.SpotFundsOverride{SlippageBps: optional.None[uint16]()},
				},
			),
		).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	engine.Stop()
}

func TestSpotFundsBuilderOverrideAccountScoped(t *testing.T) {
	service := mustMarketDataService(t)
	defer service.Close()

	account := param.NewAccountIDFromUint64(42)

	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFunds().
			WithMarketOrders(service, 1500).
			Overrides(
				policies.SpotFundsOverrideEntry{
					Target: policies.SpotFundsOverrideTargetInstrumentAccount{
						Instrument: marketdata.NewInstrumentIDFromUint64(1),
						AccountID:  account,
					},
					Override: policies.SpotFundsOverride{SlippageBps: optional.Some[uint16](200)},
				},
			),
		).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	engine.Stop()
}

func TestSpotFundsBuilderOverrideGroupScoped(t *testing.T) {
	service := mustMarketDataService(t)
	defer service.Close()

	group := mustAccountGroupID(t, 7)

	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFunds().
			WithMarketOrders(service, 1500).
			Overrides(
				policies.SpotFundsOverrideEntry{
					Target: policies.SpotFundsOverrideTargetInstrumentAccountGroup{
						Instrument:     marketdata.NewInstrumentIDFromUint64(1),
						AccountGroupID: group,
					},
					Override: policies.SpotFundsOverride{SlippageBps: optional.Some[uint16](300)},
				},
			),
		).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	engine.Stop()
}

func TestSpotFundsBuilderOverrideInstrumentOnly(t *testing.T) {
	service := mustMarketDataService(t)
	defer service.Close()

	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFunds().
			WithMarketOrders(service, 1500).
			Overrides(
				policies.SpotFundsOverrideEntry{
					Target:   policies.SpotFundsOverrideTargetInstrument{Instrument: marketdata.NewInstrumentIDFromUint64(1)},
					Override: policies.SpotFundsOverride{SlippageBps: optional.Some[uint16](100)},
				},
			),
		).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	engine.Stop()
}

func TestSpotFundsBuilderPointerOverrideTargets(t *testing.T) {
	service := mustMarketDataService(t)
	defer service.Close()

	group := mustAccountGroupID(t, 7)
	entries := []policies.SpotFundsOverrideEntry{
		{
			Target: &policies.SpotFundsOverrideTargetInstrument{
				Instrument: marketdata.NewInstrumentIDFromUint64(1),
			},
			Override: policies.SpotFundsOverride{
				SlippageBps: optional.Some[uint16](100),
			},
		},
		{
			Target: &policies.SpotFundsOverrideTargetInstrumentAccount{
				Instrument: marketdata.NewInstrumentIDFromUint64(2),
				AccountID:  param.NewAccountIDFromUint64(42),
			},
			Override: policies.SpotFundsOverride{
				SlippageBps: optional.Some[uint16](200),
			},
		},
		{
			Target: &policies.SpotFundsOverrideTargetInstrumentAccountGroup{
				Instrument:     marketdata.NewInstrumentIDFromUint64(3),
				AccountGroupID: group,
			},
			Override: policies.SpotFundsOverride{
				SlippageBps: optional.Some[uint16](300),
			},
		},
	}

	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFunds().
			WithMarketOrders(service, 1500).
			Overrides(entries...),
		).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	engine.Stop()
}

func TestSpotFundsBuilderInvalidOverrideTargets(t *testing.T) {
	tests := []struct {
		name   string
		target policies.SpotFundsOverrideTarget
	}{
		{name: "nil interface"},
		{
			name:   "nil instrument pointer",
			target: (*policies.SpotFundsOverrideTargetInstrument)(nil),
		},
		{
			name:   "nil account pointer",
			target: (*policies.SpotFundsOverrideTargetInstrumentAccount)(nil),
		},
		{
			name: "nil account group pointer",
			target: (*policies.SpotFundsOverrideTargetInstrumentAccountGroup)(
				nil,
			),
		},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			_, err := openpit.NewEngineBuilder().NoSync().
				Builtin(policies.BuildSpotFunds().
					PolicyGroupID(0).
					Overrides(policies.SpotFundsOverrideEntry{
						Target: test.target,
					}),
				).Build()
			if err == nil {
				t.Fatal("Build() error = nil, want invalid target error")
			}
			if !strings.Contains(
				err.Error(),
				"spot funds override 0: target is nil",
			) {
				t.Fatalf("Build() error = %q, want indexed nil target error", err)
			}
		})
	}
}

func TestSpotFundsConfiguratorPointerOverrideTargets(t *testing.T) {
	service := mustMarketDataService(t)
	defer service.Close()

	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFunds().WithMarketOrders(service, 1500)).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	group := mustAccountGroupID(t, 7)
	err = engine.Configure().SpotFunds(
		policies.SpotFundsPolicyName,
		optional.None[uint16](),
		optional.None[policies.SpotFundsPricingSource](),
		[]policies.SpotFundsOverrideEntry{
			{
				Target: &policies.SpotFundsOverrideTargetInstrument{
					Instrument: marketdata.NewInstrumentIDFromUint64(1),
				},
			},
			{
				Target: &policies.SpotFundsOverrideTargetInstrumentAccount{
					Instrument: marketdata.NewInstrumentIDFromUint64(2),
					AccountID:  param.NewAccountIDFromUint64(42),
				},
			},
			{
				Target: &policies.SpotFundsOverrideTargetInstrumentAccountGroup{
					Instrument:     marketdata.NewInstrumentIDFromUint64(3),
					AccountGroupID: group,
				},
			},
		},
	)
	if err != nil {
		t.Fatalf("Configure().SpotFunds() error = %v", err)
	}
}

func TestSpotFundsConfiguratorInvalidOverrideTargets(t *testing.T) {
	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFunds()).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	tests := []struct {
		name   string
		target policies.SpotFundsOverrideTarget
	}{
		{name: "nil interface"},
		{
			name:   "nil instrument pointer",
			target: (*policies.SpotFundsOverrideTargetInstrument)(nil),
		},
		{
			name:   "nil account pointer",
			target: (*policies.SpotFundsOverrideTargetInstrumentAccount)(nil),
		},
		{
			name: "nil account group pointer",
			target: (*policies.SpotFundsOverrideTargetInstrumentAccountGroup)(
				nil,
			),
		},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			err := engine.Configure().SpotFunds(
				policies.SpotFundsPolicyName,
				optional.None[uint16](),
				optional.None[policies.SpotFundsPricingSource](),
				[]policies.SpotFundsOverrideEntry{{Target: test.target}},
			)
			if err == nil {
				t.Fatal(
					"Configure().SpotFunds() error = nil, want invalid target error",
				)
			}
			if !strings.Contains(
				err.Error(),
				"configure: spot funds override 0: target is nil",
			) {
				t.Fatalf(
					"Configure().SpotFunds() error = %q, want indexed nil target error",
					err,
				)
			}
		})
	}
}

// TestSpotFundsBuilderOverrideBothAccountAndGroupIsError is no longer
// representable at the type level: the target variants
// SpotFundsOverrideTargetInstrumentAccount and
// SpotFundsOverrideTargetInstrumentAccountGroup are mutually exclusive by
// construction, so this scenario cannot be expressed in Go.
// The C ABI still enforces mutual exclusion, but the Go API prevents it.

func TestSpotFundsLimitModeDefaultIsEnforce(t *testing.T) {
	var zero policies.SpotFundsLimitMode
	if zero != policies.SpotFundsLimitModeEnforce {
		t.Fatalf("zero SpotFundsLimitMode = %v, want Enforce", zero)
	}
}

func TestSpotFundsLimitModeReviewHolds(t *testing.T) {
	for _, test := range []struct {
		mode             policies.SpotFundsLimitMode
		reviewMode       reject.RuleMode
		remainingReviews int
	}{
		{policies.SpotFundsLimitModeReviewWithHold, reject.RuleModeReviewWithHold, 1},
		{policies.SpotFundsLimitModeReviewWithoutHold, reject.RuleModeReviewWithoutHold, 0},
	} {
		t.Run(test.mode.String(), func(t *testing.T) {
			engine, err := openpit.NewEngineBuilder().NoSync().Builtin(policies.BuildSpotFunds()).Build()
			if err != nil {
				t.Fatalf("Build() error = %v", err)
			}
			defer engine.Stop()
			account := param.NewAccountIDFromUint64(42)
			seedSpotFundsLifecycleAccount(t, engine, account, mustAsset(t, "USD"))
			if err := engine.Configure().SpotFundsGlobalLimitMode(policies.SpotFundsPolicyName, test.mode); err != nil {
				t.Fatalf("configure review mode: %v", err)
			}
			checkOrder := func(quantity string, reviewCount int, commit bool) {
				t.Helper()
				order := spotFundsLifecycleOrder(t, account)
				operation := order.EnsureOperationView()
				operation.SetTradeAmount(param.NewQuantityTradeAmount(mustQuantity(t, quantity)))
				reservation, rejects, err := engine.ExecutePreTrade(order)
				if reservation != nil {
					defer reservation.RollbackAndClose()
				}
				if err != nil || len(rejects) != 0 || reservation == nil {
					t.Fatalf("quantity %s: expected pass without rejects, got %v, %v, %v", quantity, reservation, rejects, err)
				}
				reviews, err := reservation.Reviews()
				if err != nil || len(reviews) != reviewCount {
					t.Fatalf("quantity %s: reviews = %v, error = %v, want %d marks", quantity, reviews, err, reviewCount)
				}
				if reviewCount > 0 && (reviews[0].Code != reject.CodeInsufficientFunds || reviews[0].Mode != test.reviewMode) {
					t.Fatalf("quantity %s: review = %v, want InsufficientFunds with mode %v", quantity, reviews[0], test.reviewMode)
				}
				if commit {
					reservation.Commit()
				}
			}
			checkOrder("1", 0, false)
			checkOrder("11", 1, true)
			checkOrder("1", test.remainingReviews, false)
		})
	}
}

func TestSpotFundsLimitModeNativeRoundTrip(t *testing.T) {
	tests := []struct {
		name string
		mode policies.SpotFundsLimitMode
		want native.PretradePoliciesSpotFundsLimitMode
	}{
		{"enforce", policies.SpotFundsLimitModeEnforce, native.PretradePoliciesSpotFundsLimitModeEnforce},
		{"track-only", policies.SpotFundsLimitModeTrackOnly, native.PretradePoliciesSpotFundsLimitModeTrackOnly},
		{"review-with-hold", policies.SpotFundsLimitModeReviewWithHold, native.PretradePoliciesSpotFundsLimitModeReviewWithHold},
		{"review-without-hold", policies.SpotFundsLimitModeReviewWithoutHold, native.PretradePoliciesSpotFundsLimitModeReviewWithoutHold},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			if got := policies.NativeSpotFundsLimitMode(test.mode); got != test.want {
				t.Fatalf("NativeSpotFundsLimitMode(%v) = %v, want %v", test.mode, got, test.want)
			}
			if got := test.mode.Handle(); got != test.want {
				t.Fatalf("%v.Handle() = %v, want %v", test.mode, got, test.want)
			}
		})
	}
}

func TestSpotFundsLimitModeOptionPinsAndClears(t *testing.T) {
	mode, has := policies.NativeSpotFundsLimitModeOption(
		optional.Some(policies.SpotFundsLimitModeEnforce),
	)
	if !has {
		t.Fatal("Some(Enforce): hasMode = false, want true")
	}
	if mode != native.PretradePoliciesSpotFundsLimitModeEnforce {
		t.Fatalf("Some(Enforce): mode = %v, want Enforce", mode)
	}

	mode, has = policies.NativeSpotFundsLimitModeOption(
		optional.Some(policies.SpotFundsLimitModeTrackOnly),
	)
	if !has {
		t.Fatal("Some(TrackOnly): hasMode = false, want true")
	}
	if mode != native.PretradePoliciesSpotFundsLimitModeTrackOnly {
		t.Fatalf("Some(TrackOnly): mode = %v, want TrackOnly", mode)
	}

	_, has = policies.NativeSpotFundsLimitModeOption(
		optional.None[policies.SpotFundsLimitMode](),
	)
	if has {
		t.Fatal("None: hasMode = true, want false")
	}
}

func TestSpotFundsConfiguratorPropagatesFfiLimitModeValidation(t *testing.T) {
	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFunds()).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	invalidMode := policies.SpotFundsLimitMode(4)

	assertConfigureValidationError(
		t,
		"SpotFundsGlobalLimitMode(invalid)",
		engine.Configure().SpotFundsGlobalLimitMode(
			policies.SpotFundsPolicyName,
			invalidMode,
		),
	)

	assertConfigureValidationError(
		t,
		"SpotFundsAccountLimitMode(Some(invalid))",
		engine.Configure().SpotFundsAccountLimitMode(
			policies.SpotFundsPolicyName,
			param.NewAccountIDFromUint64(77002),
			optional.Some(invalidMode),
		),
	)

	assertConfigureValidationError(
		t,
		"SpotFundsAccountGroupLimitMode(Some(invalid))",
		engine.Configure().SpotFundsAccountGroupLimitMode(
			policies.SpotFundsPolicyName,
			mustAccountGroupID(t, 43),
			optional.Some(invalidMode),
		),
	)
}

func assertConfigureValidationError(t *testing.T, label string, err error) {
	t.Helper()
	if err == nil {
		t.Fatalf("%s error = nil, want validation error", label)
	}

	var configErr *configure.Error
	if !errors.As(err, &configErr) {
		t.Fatalf("%s error = %T, want *configure.Error", label, err)
	}
	if configErr.Kind != configure.ErrorKindValidation {
		t.Fatalf("%s kind = %v, want Validation", label, configErr.Kind)
	}
	const wantMessage = "spot funds limit_mode must be 0 (Enforce), 1 (TrackOnly), 2 (ReviewWithHold) or 3 (ReviewWithoutHold), got 4"
	if configErr.Message != wantMessage {
		t.Fatalf("%s message = %q, want %q", label, configErr.Message, wantMessage)
	}
}

func TestSpotFundsLimitModeString(t *testing.T) {
	if got := policies.SpotFundsLimitModeReviewWithHold.String(); got != "review-with-hold" {
		t.Fatalf("ReviewWithHold.String() = %q, want %q", got, "review-with-hold")
	}
	if got := policies.SpotFundsLimitModeReviewWithoutHold.String(); got != "review-without-hold" {
		t.Fatalf("ReviewWithoutHold.String() = %q, want %q", got, "review-without-hold")
	}
	if got := policies.SpotFundsLimitModeEnforce.String(); got != "enforce" {
		t.Fatalf("Enforce.String() = %q, want %q", got, "enforce")
	}
	if got := policies.SpotFundsLimitModeTrackOnly.String(); got != "track-only" {
		t.Fatalf("TrackOnly.String() = %q, want %q", got, "track-only")
	}
}

func TestSpotFundsBuilderGroupID(t *testing.T) {
	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFunds().PolicyGroupID(7)).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	engine.Stop()
}

func TestSpotFundsWithoutPnlBarriersExecutesNormalOrder(t *testing.T) {
	usd := mustAsset(t, "USD")
	account := param.NewAccountIDFromUint64(83010)
	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFunds()).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	seedSpotFundsLifecycleAccount(t, engine, account, usd)
	result := applySpotFundsLifecycleFill(t, engine, account)
	if len(result.AccountBlocks) != 0 {
		t.Fatalf("AccountBlocks = %v, want none", result.AccountBlocks)
	}
	if len(result.AccountPnls) != 0 {
		t.Fatalf("AccountPnls = %v, want none for an opening fill", result.AccountPnls)
	}
}

func TestSpotFundsAlreadyHeldDoesNotReserveAgain(t *testing.T) {
	account := param.NewAccountIDFromUint64(83024)
	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFunds()).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	seedSpotFundsLifecycleAccount(t, engine, account, mustAsset(t, "USD"))
	if err := engine.Configure().SpotFundsGlobalLimitMode(policies.SpotFundsPolicyName, policies.SpotFundsLimitModeReviewWithHold); err != nil {
		t.Fatal(err)
	}
	order := spotFundsLifecycleOrder(t, account)
	operation := order.EnsureOperationView()
	operation.SetTradeAmount(param.NewQuantityTradeAmount(mustQuantity(t, "11")))

	reserve := func(phase string, order model.Order) *pretrade.Reservation {
		t.Helper()
		reservation, rejects, err := engine.ExecutePreTrade(order)
		if err != nil || len(rejects) != 0 || reservation == nil {
			if reservation != nil {
				reservation.RollbackAndClose()
			}
			t.Fatalf("%s: expected a reservation without rejects, got reservation = %v, rejects = %v, error = %v",
				phase, reservation, rejects, err)
		}
		return reservation
	}
	first := reserve("initial 1100 USD order", order)
	marks, err := first.Reviews()
	if err != nil || len(marks) != 1 || marks[0].Code != reject.CodeInsufficientFunds || marks[0].Mode != reject.RuleModeReviewWithHold {
		t.Fatalf("initial reviews = %v, error = %v, want with-hold InsufficientFunds", marks, err)
	}
	lock, err := first.Lock()
	if err != nil {
		t.Fatal(err)
	}
	prices, err := lock.Prices()
	if err != nil || len(prices) != 1 || !prices[0].Equal(mustPrice100(t)) {
		t.Fatalf("first lock prices = %v, error = %v, want 100", prices, err)
	}
	first.CommitAndClose()
	before := spotFundsAmounts(t, engine, account, mustAsset(t, "USD"))
	assertSpotFundsAmounts(t, before, "-100", "1100", "0")
	assertSpotFundsAmounts(t, spotFundsAmounts(t, engine, account, mustAsset(t, "AAPL")), "0", "0", "11")

	review := order.EnsureReviewView()
	review.SetConfirmed(true)
	review.SetAlreadyHeld(true)
	rerun := reserve("confirmed order with already_held", order)
	marks, err = rerun.Reviews()
	if err != nil || len(marks) != 0 {
		t.Fatalf("rerun reviews = %v, error = %v, want none", marks, err)
	}
	rerunLock, err := rerun.Lock()
	if err != nil {
		t.Fatal(err)
	}
	empty, err := rerunLock.IsEmpty()
	if err != nil || !empty {
		t.Fatalf("rerun lock empty = %v, error = %v, want true", empty, err)
	}
	adjustments, err := rerun.AccountAdjustments()
	if err != nil || len(adjustments) != 0 {
		t.Fatalf("rerun adjustments = %v, error = %v, want none", adjustments, err)
	}
	rerun.CommitAndClose()
	assertSpotFundsSameAmounts(t, spotFundsAmounts(t, engine, account, mustAsset(t, "USD")), before)
	assertSpotFundsAmounts(t, spotFundsAmounts(t, engine, account, mustAsset(t, "AAPL")), "0", "0", "11")
	result := cancelSpotFundsOrder(t, engine, account, lock, "11")
	if len(result.AccountBlocks) != 0 {
		t.Fatalf("cancel blocks = %v, want none", result.AccountBlocks)
	}
	assertSpotFundsAmounts(t, spotFundsAmounts(t, engine, account, mustAsset(t, "USD")), "1000", "0", "0")
	assertSpotFundsAmounts(t, spotFundsAmounts(t, engine, account, mustAsset(t, "AAPL")), "0", "0", "0")
}

func spotFundsAmounts(t *testing.T, engine *openpit.Engine, account param.AccountID, asset param.Asset) [3]param.PositionSize {
	t.Helper()
	adjustment := model.NewAccountAdjustment()
	operation := adjustment.EnsureBalanceOperationView()
	operation.SetAsset(asset)
	amount := adjustment.EnsureAmountView()
	zero := param.NewDeltaAdjustmentAmount(mustPositionSize(t, "0"))
	amount.SetBalance(zero)
	amount.SetHeld(zero)
	amount.SetIncoming(zero)
	result, err := engine.ApplyAccountAdjustment(account, []model.AccountAdjustment{adjustment})
	if err != nil || result.BatchError.IsSet() || len(result.Outcomes) != 1 {
		t.Fatalf("read amounts = %v, error = %v", result, err)
	}
	entry := result.Outcomes[0].Entry
	var values [3]param.PositionSize
	for i, field := range []optional.Option[accountadjustment.OutcomeAmount]{entry.Balance, entry.Held, entry.Incoming} {
		value, ok := field.Get()
		if !ok {
			t.Fatalf("amount field %d missing", i)
		}
		values[i] = value.Absolute
	}
	return values
}

func assertSpotFundsAmounts(t *testing.T, got [3]param.PositionSize, available, held, incoming string) {
	t.Helper()
	for i, want := range []string{available, held, incoming} {
		if !got[i].Equal(mustPositionSize(t, want)) {
			t.Fatalf("amount field %d = %v, want %s", i, got[i], want)
		}
	}
}

func assertSpotFundsSameAmounts(t *testing.T, got, want [3]param.PositionSize) {
	t.Helper()
	for i := range got {
		if !got[i].Equal(want[i]) {
			t.Fatalf("amount field %d changed: %v, want %v", i, got[i], want[i])
		}
	}
}

func cancelSpotFundsOrder(t *testing.T, engine *openpit.Engine, account param.AccountID, lock pretrade.Lock, quantity string) openpit.PostTradeResult {
	t.Helper()
	report := model.NewExecutionReport()
	operation := report.EnsureOperationView()
	operation.SetInstrument(param.NewInstrument(mustAsset(t, "AAPL"), mustAsset(t, "USD")))
	operation.SetAccountID(account)
	operation.SetSide(param.SideBuy)
	fill := report.EnsureFillView()
	fill.SetRemainingReservedQuantity(mustQuantity(t, quantity))
	fill.SetIsFinal(true)
	if err := fill.SetLock(lock.Bytes()); err != nil {
		t.Fatal(err)
	}
	result, err := engine.ApplyExecutionReport(report)
	if err != nil {
		t.Fatal(err)
	}
	return result
}

func seedSpotFundsAlreadyHeldOrder(t *testing.T, mode policies.SpotFundsLimitMode, quantity string) (*openpit.Engine, param.AccountID, model.Order) {
	t.Helper()
	engine, err := openpit.NewEngineBuilder().NoSync().Builtin(policies.BuildSpotFunds()).Build()
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(engine.Stop)
	account := param.NewAccountIDFromUint64(83401)
	usd := mustAsset(t, "USD")
	seedSpotFundsLifecycleAccount(t, engine, account, usd)
	if err := engine.Configure().SpotFundsGlobalLimitMode(policies.SpotFundsPolicyName, mode); err != nil {
		t.Fatal(err)
	}
	order := spotFundsLifecycleOrder(t, account)
	operation := order.EnsureOperationView()
	operation.SetTradeAmount(param.NewQuantityTradeAmount(mustQuantity(t, quantity)))
	first, rejects, err := engine.ExecutePreTrade(order)
	if err != nil || first == nil || len(rejects) != 0 {
		t.Fatalf("first = %v, rejects = %v, error = %v", first, rejects, err)
	}
	first.CommitAndClose()
	return engine, account, order
}

func configureSpotFundsAlreadyHeldPnl(t *testing.T, engine *openpit.Engine, account param.AccountID, usd param.Asset, pnl string) {
	t.Helper()
	barriers := make([]policies.SpotFundsPnlBoundsBarrier, 0, 3)
	for _, mode := range []reject.RuleMode{reject.RuleModeReject, reject.RuleModeReviewWithHold, reject.RuleModeReviewWithoutHold} {
		bound := "-10"
		if mode == reject.RuleModeReject {
			bound = "-100"
		}
		barriers = append(barriers, policies.SpotFundsPnlBoundsBarrier{Mode: mode, Currency: usd, LowerBound: optional.Some(mustPnl(t, bound))})
	}
	if _, err := engine.Configure().SpotFundsPnlBoundsKillSwitch(policies.SpotFundsPolicyName, barriers, nil, nil); err != nil {
		t.Fatal(err)
	}
	if _, err := engine.Configure().SetSpotFundsAccountPnl(policies.SpotFundsPolicyName, account, model.NewPnlState(mustPnl(t, pnl))); err != nil {
		t.Fatal(err)
	}
}

func assertSpotFundsAlreadyHeldReviewMarks(t *testing.T, engine *openpit.Engine, order model.Order, code reject.Code, modes []reject.RuleMode) {
	t.Helper()
	report, err := engine.ExecutePreTradeDryRun(order)
	if err != nil {
		t.Fatal(err)
	}
	marks, err := report.Reviews()
	report.Close()
	if err != nil {
		t.Fatal(err)
	}
	for _, mode := range modes {
		count := 0
		for _, mark := range marks {
			if mark.Code == code && mark.Mode == mode {
				count++
			}
		}
		if count != 1 {
			t.Fatalf("pre-confirmation marks = %v, want one code %v mode %v", marks, code, mode)
		}
	}
}

func commitSpotFundsAlreadyHeldRerun(t *testing.T, engine *openpit.Engine, order model.Order) pretrade.Lock {
	t.Helper()
	rerun, rejects, err := engine.ExecutePreTrade(order)
	if err != nil {
		t.Fatal(err)
	}
	if rerun == nil || len(rejects) != 0 {
		t.Fatalf("rerun = %v, rejects = %v, want reservation", rerun, rejects)
	}
	marks, err := rerun.Reviews()
	if err != nil || len(marks) != 0 {
		t.Fatalf("already-held marks = %v, error = %v, want none", marks, err)
	}
	lock, err := rerun.Lock()
	if err != nil {
		t.Fatal(err)
	}
	empty, err := lock.IsEmpty()
	if err != nil || !empty {
		t.Fatalf("already-held lock empty = %v, error = %v", empty, err)
	}
	adjustments, err := rerun.AccountAdjustments()
	if err != nil || len(adjustments) != 0 {
		t.Fatalf("already-held adjustments = %v, error = %v", adjustments, err)
	}
	rerun.CommitAndClose()
	return lock
}

func TestSpotFundsAlreadyHeldRejectPnlKeepsFirstHold(t *testing.T) {
	engine, account, order := seedSpotFundsAlreadyHeldOrder(t, policies.SpotFundsLimitModeReviewWithHold, "11")
	usd := mustAsset(t, "USD")
	before := spotFundsAmounts(t, engine, account, usd)
	assertSpotFundsAmounts(t, before, "-100", "1100", "0")
	configureSpotFundsAlreadyHeldPnl(t, engine, account, usd, "-150")
	engine.Accounts().Unblock(account)
	review := order.EnsureReviewView()
	review.SetConfirmed(true)
	review.SetAlreadyHeld(true)
	rerun, rejects, err := engine.ExecutePreTrade(order)
	if err != nil {
		t.Fatal(err)
	}
	if rerun != nil {
		rerun.RollbackAndClose()
		t.Fatal("breached Reject barrier admitted already-held rerun")
	}
	if len(rejects) != 1 || rejects[0].Code != reject.CodePnlKillSwitchTriggered || rejects[0].Mode != reject.RuleModeReject {
		t.Fatalf("rerun rejects = %v, want Reject PnlKillSwitchTriggered", rejects)
	}
	assertSpotFundsSameAmounts(t, spotFundsAmounts(t, engine, account, usd), before)
}

func TestSpotFundsAlreadyHeldOmitsBreachedReviewPnl(t *testing.T) {
	engine, account, order := seedSpotFundsAlreadyHeldOrder(t, policies.SpotFundsLimitModeReviewWithHold, "11")
	usd := mustAsset(t, "USD")
	before := spotFundsAmounts(t, engine, account, usd)
	assertSpotFundsAmounts(t, before, "-100", "1100", "0")
	configureSpotFundsAlreadyHeldPnl(t, engine, account, usd, "-50")
	assertSpotFundsAlreadyHeldReviewMarks(t, engine, order, reject.CodePnlKillSwitchTriggered, []reject.RuleMode{reject.RuleModeReviewWithHold, reject.RuleModeReviewWithoutHold})
	review := order.EnsureReviewView()
	review.SetConfirmed(true)
	review.SetAlreadyHeld(true)
	commitSpotFundsAlreadyHeldRerun(t, engine, order)
	assertSpotFundsSameAmounts(t, spotFundsAmounts(t, engine, account, usd), before)
}

func TestSpotFundsAlreadyHeldOmitsBreachedReviewPosition(t *testing.T) {
	engine, account, order := seedSpotFundsAlreadyHeldOrder(t, policies.SpotFundsLimitModeReviewWithHold, "11")
	usd := mustAsset(t, "USD")
	before := spotFundsAmounts(t, engine, account, usd)
	assertSpotFundsAmounts(t, before, "-100", "1100", "0")
	pnl := "-50"
	if _, err := engine.Configure().SetSpotFundsAccountPnl(policies.SpotFundsPolicyName, account, model.NewPnlState(mustPnl(t, pnl))); err != nil {
		t.Fatal(err)
	}
	for _, mode := range []reject.RuleMode{reject.RuleModeReviewWithHold, reject.RuleModeReviewWithoutHold} {
		if err := engine.Configure().SpotFundsPositionLimit(policies.SpotFundsPolicyName, account, mustAsset(t, "AAPL"), mode, optional.Some(mustQuantity(t, "0"))); err != nil {
			t.Fatal(err)
		}
	}
	assertSpotFundsAlreadyHeldReviewMarks(t, engine, order, reject.CodePositionLimitExceeded, []reject.RuleMode{reject.RuleModeReviewWithoutHold})
	review := order.EnsureReviewView()
	review.SetConfirmed(true)
	review.SetAlreadyHeld(true)
	commitSpotFundsAlreadyHeldRerun(t, engine, order)
	assertSpotFundsSameAmounts(t, spotFundsAmounts(t, engine, account, usd), before)
}

func TestSpotFundsCancelWithRerunLockBlocksWithoutReleasingUSD(t *testing.T) {
	engine, account, order := seedSpotFundsAlreadyHeldOrder(t, policies.SpotFundsLimitModeReviewWithHold, "11")
	usd := mustAsset(t, "USD")
	before := spotFundsAmounts(t, engine, account, usd)
	assertSpotFundsAmounts(t, before, "-100", "1100", "0")
	review := order.EnsureReviewView()
	review.SetConfirmed(true)
	review.SetAlreadyHeld(true)
	lock := commitSpotFundsAlreadyHeldRerun(t, engine, order)
	result := cancelSpotFundsOrder(t, engine, account, lock, "11")
	if len(result.AccountBlocks) != 1 || result.AccountBlocks[0].Code != reject.CodeMissingRequiredField {
		t.Fatalf("empty-lock cancel blocks = %v, want MissingRequiredField", result.AccountBlocks)
	}
	assertSpotFundsSameAmounts(t, spotFundsAmounts(t, engine, account, usd), before)
}

func TestSpotFundsAlreadyHeldEnforceKeepsFirstHold(t *testing.T) {
	engine, account, order := seedSpotFundsAlreadyHeldOrder(t, policies.SpotFundsLimitModeEnforce, "6")
	usd := mustAsset(t, "USD")
	before := spotFundsAmounts(t, engine, account, usd)
	assertSpotFundsAmounts(t, before, "400", "600", "0")
	review := order.EnsureReviewView()
	review.SetConfirmed(true)
	duplicate, rejects, err := engine.ExecutePreTrade(order)
	if duplicate != nil {
		duplicate.RollbackAndClose()
	}
	if err != nil || duplicate != nil || len(rejects) != 1 || rejects[0].Code != reject.CodeInsufficientFunds {
		t.Fatalf("confirmed without already-held = %v, rejects = %v, error = %v", duplicate, rejects, err)
	}
	review.SetAlreadyHeld(true)
	commitSpotFundsAlreadyHeldRerun(t, engine, order)
	assertSpotFundsSameAmounts(t, spotFundsAmounts(t, engine, account, usd), before)
	remaining := spotFundsLifecycleOrder(t, account)
	operation := remaining.EnsureOperationView()
	operation.SetTradeAmount(param.NewQuantityTradeAmount(mustQuantity(t, "4")))
	reservation, rejects, err := engine.ExecutePreTrade(remaining)
	if err != nil || reservation == nil || len(rejects) != 0 {
		t.Fatalf("remaining 400 USD = %v, rejects = %v, error = %v", reservation, rejects, err)
	}
	reservation.RollbackAndClose()
	adjustment := spotFundsAbsoluteBalanceAdjustment(t, usd, "-1")
	adjusted, err := engine.ApplyAccountAdjustment(account, []model.AccountAdjustment{adjustment})
	if err != nil || adjusted.BatchError.IsSet() {
		t.Fatalf("shrink available = %v, error = %v", adjusted, err)
	}
	shrunk := spotFundsAmounts(t, engine, account, usd)
	assertSpotFundsAmounts(t, shrunk, "-1", "600", "0")
	rerun, rejects, err := engine.ExecutePreTrade(order)
	if rerun != nil {
		rerun.RollbackAndClose()
	}
	if err != nil || rerun != nil || len(rejects) != 1 || rejects[0].Code != reject.CodeInsufficientFunds || rejects[0].Mode != reject.RuleModeReject {
		t.Fatalf("shrunk already-held rerun = %v, rejects = %v, error = %v", rerun, rejects, err)
	}
	assertSpotFundsSameAmounts(t, spotFundsAmounts(t, engine, account, usd), shrunk)
}

func TestSpotFundsReviewPnlUncomputableStillBlocks(t *testing.T) {
	for _, scenario := range []string{"currency_mismatch", "halted"} {
		for _, mode := range []reject.RuleMode{reject.RuleModeReviewWithHold, reject.RuleModeReviewWithoutHold} {
			t.Run(fmt.Sprintf("%s/mode_%d", scenario, mode), func(t *testing.T) {
				account := param.NewAccountIDFromUint64(83402)
				usd := mustAsset(t, "USD")
				currency := usd
				if scenario == "currency_mismatch" {
					currency = mustAsset(t, "EUR")
				}
				barrier := policies.SpotFundsPnlBoundsAccountBarrier{AccountID: account, Barrier: policies.SpotFundsPnlBoundsBarrier{Mode: mode, Currency: currency, LowerBound: optional.Some(mustPnl(t, "-100"))}}
				engine, err := openpit.NewEngineBuilder().NoSync().Builtin(policies.BuildSpotFundsPnlBoundsKillSwitch().AccountBarriers(barrier)).Build()
				if err != nil {
					t.Fatal(err)
				}
				t.Cleanup(engine.Stop)
				if err := engine.Accounts().SetCurrency(account, usd); err != nil {
					t.Fatal(err)
				}
				state := model.NewPnlState(mustPnl(t, "0"))
				if scenario == "halted" {
					state, err = model.NewPnlHaltedState(model.PnlHaltReasonMissingFx)
					if err != nil {
						t.Fatal(err)
					}
				}
				applyPnl := func() (configure.PolicyConfigurationResult, error) {
					return engine.Configure().SetSpotFundsAccountPnl(policies.SpotFundsPolicyName, account, state)
				}
				result, err := applyPnl()
				if err != nil || len(result.AccountBlocks) != 1 || result.AccountBlocks[0].Code != reject.CodePnlKillSwitchTriggered {
					t.Fatalf("review-only blocks = %v, error = %v, want PnlKillSwitchTriggered", result.AccountBlocks, err)
				}
				order := spotFundsLifecycleOrder(t, account)
				cleared, err := engine.Configure().SpotFundsPnlBoundsKillSwitch(
					policies.SpotFundsPolicyName, nil, nil, []policies.SpotFundsPnlBoundsAccountBarrier{},
				)
				if err != nil || len(cleared.AccountBlocks) != 0 {
					t.Fatalf("clear barriers = %v, %v, want no blocks", cleared.AccountBlocks, err)
				}
				blocked, blockedRejects, err := engine.ExecutePreTrade(order)
				if blocked != nil {
					blocked.RollbackAndClose()
				}
				if err != nil || blocked != nil || len(blockedRejects) == 0 {
					t.Fatalf("recorded configuration block = %v, %v, %v, want rejection", blocked, blockedRejects, err)
				}
				engine.Accounts().Unblock(account)
				restored, err := engine.Configure().SpotFundsPnlBoundsKillSwitch(
					policies.SpotFundsPolicyName, nil, nil, []policies.SpotFundsPnlBoundsAccountBarrier{barrier},
				)
				if err != nil || len(restored.AccountBlocks) != 1 || restored.AccountBlocks[0].Block.Code != reject.CodePnlKillSwitchTriggered {
					t.Fatalf("restored barrier blocks = %v, %v, want PnlKillSwitchTriggered", restored.AccountBlocks, err)
				}
				result, err = applyPnl()
				if err != nil || len(result.AccountBlocks) != 1 || result.AccountBlocks[0].Code != reject.CodePnlKillSwitchTriggered {
					t.Fatalf("reapplied review-only blocks = %v, %v, want PnlKillSwitchTriggered", result.AccountBlocks, err)
				}
				review := order.EnsureReviewView()
				for _, confirmed := range []bool{true, false} {
					engine.Accounts().Unblock(account)
					review.SetConfirmed(confirmed)
					reservation, rejects, err := engine.ExecutePreTrade(order)
					if reservation != nil {
						reservation.RollbackAndClose()
					}
					if err != nil || reservation != nil || len(rejects) != 1 || rejects[0].Code != reject.CodePnlKillSwitchTriggered || rejects[0].Mode != reject.RuleModeReject {
						t.Fatalf("live order confirmed=%v = %v, rejects = %v, error = %v", confirmed, reservation, rejects, err)
					}
				}
				if scenario == "currency_mismatch" {
					barrier.Barrier.Currency = usd
					_, err = engine.Configure().SpotFundsPnlBoundsKillSwitch(policies.SpotFundsPolicyName, nil, nil, []policies.SpotFundsPnlBoundsAccountBarrier{barrier})
				} else {
					_, err = engine.Configure().SetSpotFundsAccountPnl(policies.SpotFundsPolicyName, account, model.NewPnlState(mustPnl(t, "0")))
				}
				if err != nil {
					t.Fatal(err)
				}
				reservation, rejects, err := engine.ExecutePreTrade(order)
				if reservation != nil {
					reservation.RollbackAndClose()
				}
				if err != nil || reservation != nil || len(rejects) != 1 || rejects[0].Code != reject.CodeAccountBlocked {
					t.Fatalf("latched block after cause removed = %v, rejects = %v, error = %v", reservation, rejects, err)
				}
			})
		}
	}
}

func TestSpotFundsWithoutPnlBarriersReturnsFeeInclusiveAccountPnl(t *testing.T) {
	usd := mustAsset(t, "USD")
	account := param.NewAccountIDFromUint64(83016)
	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFunds()).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	seedSpotFundsLifecycleAccount(t, engine, account, usd)
	result := applySpotFundsLifecycleFillWithFee(
		t,
		engine,
		account,
		optional.Some(param.NewMonetaryAmount(mustFee(t, "5"), usd)),
	)
	if len(result.AccountBlocks) != 0 {
		t.Fatalf("AccountBlocks = %v, want none", result.AccountBlocks)
	}
	if len(result.AccountPnls) != 1 {
		t.Fatalf("AccountPnls = %v, want one outcome", result.AccountPnls)
	}
	pnl, ok := result.AccountPnls[0].Amount()
	if !ok {
		haltReason, _ := result.AccountPnls[0].HaltReason()
		t.Fatalf("AccountPnls[0] halt reason = %v, want none", haltReason)
	}
	if !pnl.Delta.Equal(mustPnl(t, "-5")) {
		t.Fatalf(
			"AccountPnls[0].Amount() PnL delta = %v, want -5",
			pnl.Delta,
		)
	}
	if !pnl.Absolute.Equal(mustPnl(t, "-5")) {
		t.Fatalf(
			"AccountPnls[0].Amount() PnL absolute = %v, want -5",
			pnl.Absolute,
		)
	}
}

func TestSpotFundsAccountPnlHaltIsStickyUntilExactForceSet(t *testing.T) {
	usd := mustAsset(t, "USD")
	eur := mustAsset(t, "EUR")
	account := param.NewAccountIDFromUint64(83017)
	service := mustMarketDataService(t)
	defer service.Close()
	fxID, err := service.Register(param.NewInstrument(usd, eur))
	if err != nil {
		t.Fatalf("Register() error = %v", err)
	}
	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFunds().WithMarketOrders(service, 0)).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()
	if err := engine.Accounts().SetCurrency(account, eur); err != nil {
		t.Fatalf("Accounts().SetCurrency() error = %v", err)
	}

	// The fee is denominated in USD while the account is in EUR, so it is what
	// makes this fill's account line uncomputable until the FX quote arrives. A
	// fee-less opening fill would omit the account line instead.
	feeReport := directSpotFundsFillReport(t, account)
	fill := feeReport.EnsureFillView()
	fill.SetFee(
		param.NewMonetaryAmount(mustFee(t, "1"), usd),
	)
	first, err := engine.ApplyExecutionReport(feeReport)
	if err != nil {
		t.Fatalf("first ApplyExecutionReport() error = %v", err)
	}
	if len(first.AccountPnls) != 1 {
		t.Fatalf("first AccountPnls len = %d, want 1", len(first.AccountPnls))
	}
	reason, ok := first.AccountPnls[0].HaltReason()
	if !ok || reason != model.PnlHaltReasonMissingFx {
		t.Fatalf("first HaltReason() = (%v, %v), want MissingFx", reason, ok)
	}

	fxRate, err := param.NewPriceFromString("0.9")
	if err != nil {
		t.Fatalf("NewPriceFromString() error = %v", err)
	}
	if err := service.Push(fxID, marketdata.NewQuote().WithMark(fxRate), 0); err != nil {
		t.Fatalf("Push() error = %v", err)
	}
	second, err := engine.ApplyExecutionReport(feeReport)
	if err != nil {
		t.Fatalf("second ApplyExecutionReport() error = %v", err)
	}
	if len(second.AccountPnls) != 0 {
		t.Fatalf("second AccountPnls = %v, want omitted while halted", second.AccountPnls)
	}

	configuration, err := engine.Configure().SetSpotFundsAccountPnl(
		policies.SpotFundsPolicyName,
		account,
		model.NewPnlState(mustPnl(t, "10")),
	)
	if err != nil {
		t.Fatalf("SetSpotFundsAccountPnl() error = %v", err)
	}
	if len(configuration.AccountBlocks) != 0 {
		t.Fatalf("numeric force-set account blocks = %v, want none", configuration.AccountBlocks)
	}
	third, err := engine.ApplyExecutionReport(feeReport)
	if err != nil {
		t.Fatalf("third ApplyExecutionReport() error = %v", err)
	}
	if len(third.AccountPnls) != 1 {
		t.Fatalf("third AccountPnls len = %d, want 1", len(third.AccountPnls))
	}
	pnl, ok := third.AccountPnls[0].Amount()
	if !ok {
		t.Fatal("third account PnL is halted after exact force-set")
	}
	if !pnl.Absolute.Equal(mustPnl(t, "9.1")) {
		t.Fatalf("third AccountPnls absolute = %v, want 9.1", pnl.Absolute)
	}
}

func TestSpotFundsPositionPnlHaltRearmsIndependently(t *testing.T) {
	aapl := mustAsset(t, "AAPL")
	eur := mustAsset(t, "EUR")
	usd := mustAsset(t, "USD")
	account := param.NewAccountIDFromUint64(83018)
	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFunds()).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()
	if err := engine.Accounts().SetCurrency(account, eur); err != nil {
		t.Fatalf("Accounts().SetCurrency() error = %v", err)
	}

	report := directSpotFundsFillReport(t, account)
	fill := report.EnsureFillView()
	fill.SetFee(param.NewMonetaryAmount(mustFee(t, "5"), usd))
	first, err := engine.ApplyExecutionReport(report)
	if err != nil {
		t.Fatalf("first ApplyExecutionReport() error = %v", err)
	}
	positionPnl, ok := realizedPnlForAsset(first, aapl)
	if !ok {
		t.Fatal("first position PnL is absent, want MissingFx")
	}
	reason, ok := positionPnl.HaltReason()
	if !ok || reason != model.PnlHaltReasonMissingFx {
		t.Fatalf("first position HaltReason() = (%v, %v)", reason, ok)
	}
	if len(first.AccountPnls) != 1 {
		t.Fatalf("first AccountPnls len = %d, want one account outcome", len(first.AccountPnls))
	}
	accountReason, ok := first.AccountPnls[0].HaltReason()
	if !ok || accountReason != model.PnlHaltReasonMissingFx {
		t.Fatalf("first account HaltReason() = (%v, %v), want MissingFx", accountReason, ok)
	}

	second, err := engine.ApplyExecutionReport(report)
	if err != nil {
		t.Fatalf("second ApplyExecutionReport() error = %v", err)
	}
	if _, ok := realizedPnlForAsset(second, aapl); ok {
		t.Fatal("second position PnL is emitted while halted")
	}
	if _, err := engine.Configure().SetSpotFundsAccountPnl(
		policies.SpotFundsPolicyName,
		account,
		model.NewPnlState(mustPnl(t, "10")),
	); err != nil {
		t.Fatalf("SetSpotFundsAccountPnl() error = %v", err)
	}
	third, err := engine.ApplyExecutionReport(report)
	if err != nil {
		t.Fatalf("third ApplyExecutionReport() error = %v", err)
	}
	if _, ok := realizedPnlForAsset(third, aapl); ok {
		t.Fatal("account force-set re-armed the halted position")
	}
	if len(third.AccountPnls) != 1 {
		t.Fatalf("third AccountPnls len = %d, want account re-halt", len(third.AccountPnls))
	}
	accountReason, ok = third.AccountPnls[0].HaltReason()
	if !ok || accountReason != model.PnlHaltReasonMissingFx {
		t.Fatalf("third account HaltReason() = (%v, %v), want MissingFx", accountReason, ok)
	}

	forceSpotFundsPositionPnl(t, engine, account, aapl)

	fourth, err := engine.ApplyExecutionReport(report)
	if err != nil {
		t.Fatalf("fourth ApplyExecutionReport() error = %v", err)
	}
	positionPnl, ok = realizedPnlForAsset(fourth, aapl)
	if !ok {
		t.Fatal("fourth position PnL is absent after position force-set")
	}
	reason, ok = positionPnl.HaltReason()
	if !ok || reason != model.PnlHaltReasonMissingFx {
		t.Fatalf("fourth position HaltReason() = (%v, %v)", reason, ok)
	}
	if len(fourth.AccountPnls) != 0 {
		t.Fatalf("fourth AccountPnls = %v, want no repeated account outcome", fourth.AccountPnls)
	}
}

func TestSpotFundsPnlBoundsKillSwitchBuilder(t *testing.T) {
	account := param.NewAccountIDFromUint64(83001)
	group := mustAccountGroupID(t, 83)

	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFundsPnlBoundsKillSwitch().
			GlobalBarriers(policies.SpotFundsPnlBoundsBarrier{
				Mode:       reject.RuleModeReject,
				Currency:   mustAsset(t, "USD"),
				LowerBound: optional.Some(mustPnl(t, "-100")),
			}).
			AccountGroupBarriers(policies.SpotFundsPnlBoundsAccountGroupBarrier{
				AccountGroupID: group,
				Barrier: policies.SpotFundsPnlBoundsBarrier{
					Mode:       reject.RuleModeReject,
					Currency:   mustAsset(t, "USD"),
					UpperBound: optional.Some(mustPnl(t, "250")),
				},
			}).
			AccountBarriers(policies.SpotFundsPnlBoundsAccountBarrier{
				AccountID: account,
				Barrier: policies.SpotFundsPnlBoundsBarrier{
					Mode:       reject.RuleModeReject,
					Currency:   mustAsset(t, "USD"),
					LowerBound: optional.Some(mustPnl(t, "-10")),
					UpperBound: optional.Some(mustPnl(t, "10")),
				},
			}),
		).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	engine.Stop()
}

func TestSpotFundsPnlBoundsBarrierIgnoresNonMatchingCurrency(t *testing.T) {
	usd := mustAsset(t, "USD")
	eur := mustAsset(t, "EUR")
	account := param.NewAccountIDFromUint64(83019)
	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFundsPnlBoundsKillSwitch().
			GlobalBarriers(policies.SpotFundsPnlBoundsBarrier{
				Mode:       reject.RuleModeReject,
				Currency:   eur,
				LowerBound: optional.Some(mustPnl(t, "-100")),
			}),
		).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	if err := engine.Accounts().SetCurrency(account, usd); err != nil {
		t.Fatalf("Accounts().SetCurrency() error = %v", err)
	}
	result, err := engine.Configure().SetSpotFundsAccountPnl(
		policies.SpotFundsPolicyName,
		account,
		model.NewPnlState(mustPnl(t, "-150")),
	)
	if err != nil {
		t.Fatalf("SetSpotFundsAccountPnl() error = %v", err)
	}
	if len(result.AccountBlocks) != 0 {
		t.Fatalf("AccountBlocks = %v, want none", result.AccountBlocks)
	}
}

func TestSpotFundsPnlBoundsAccountBarrierRejectsNonMatchingCurrency(t *testing.T) {
	usd := mustAsset(t, "USD")
	eur := mustAsset(t, "EUR")
	account := param.NewAccountIDFromUint64(83022)
	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFundsPnlBoundsKillSwitch().
			AccountBarriers(policies.SpotFundsPnlBoundsAccountBarrier{
				Barrier: policies.SpotFundsPnlBoundsBarrier{
					Mode:       reject.RuleModeReject,
					Currency:   eur,
					LowerBound: optional.Some(mustPnl(t, "-100")),
				},
				AccountID: account,
			}),
		).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	if err := engine.Accounts().SetCurrency(account, usd); err != nil {
		t.Fatalf("Accounts().SetCurrency() error = %v", err)
	}
	result, err := engine.Configure().SetSpotFundsAccountPnl(
		policies.SpotFundsPolicyName,
		account,
		model.NewPnlState(mustPnl(t, "0")),
	)
	if err != nil {
		t.Fatalf("SetSpotFundsAccountPnl() error = %v", err)
	}
	if len(result.AccountBlocks) != 1 {
		t.Fatalf("AccountBlocks = %v, want one mismatch block", result.AccountBlocks)
	}
	block := result.AccountBlocks[0]
	if block.Code != reject.CodePnlKillSwitchTriggered ||
		block.Reason != "pnl barrier currency mismatch" ||
		block.Details != "account currency USD, barrier currency EUR" {
		t.Fatalf("AccountBlocks[0] = %v, want exact currency mismatch block", block)
	}

	reservation, rejects, err := engine.ExecutePreTrade(
		spotFundsLifecycleOrder(t, account),
	)
	if err != nil {
		t.Fatalf("ExecutePreTrade() error = %v", err)
	}
	if reservation != nil {
		reservation.RollbackAndClose()
		t.Fatal("ExecutePreTrade() reservation is set for blocked account")
	}
	if len(rejects) != 1 ||
		rejects[0].Code != reject.CodePnlKillSwitchTriggered ||
		rejects[0].Reason != "pnl barrier currency mismatch" ||
		rejects[0].Details != "account currency USD, barrier currency EUR" {
		t.Fatalf("ExecutePreTrade() rejects = %v, want exact mismatch reject", rejects)
	}
}

func TestSpotFundsPnlBoundsBarrierAppliesMatchingCurrency(t *testing.T) {
	usd := mustAsset(t, "USD")
	account := param.NewAccountIDFromUint64(83020)
	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFundsPnlBoundsKillSwitch().
			GlobalBarriers(policies.SpotFundsPnlBoundsBarrier{
				Mode:       reject.RuleModeReject,
				Currency:   usd,
				LowerBound: optional.Some(mustPnl(t, "-100")),
			}),
		).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	if err := engine.Accounts().SetCurrency(account, usd); err != nil {
		t.Fatalf("Accounts().SetCurrency() error = %v", err)
	}
	result, err := engine.Configure().SetSpotFundsAccountPnl(
		policies.SpotFundsPolicyName,
		account,
		model.NewPnlState(mustPnl(t, "-150")),
	)
	if err != nil {
		t.Fatalf("SetSpotFundsAccountPnl() error = %v", err)
	}
	if len(result.AccountBlocks) != 1 ||
		result.AccountBlocks[0].Code != reject.CodePnlKillSwitchTriggered {
		t.Fatalf("AccountBlocks = %v, want one PnL block", result.AccountBlocks)
	}
}

func TestSpotFundsPnlBoundsRuntimeBarrierHonorsAccountCurrency(t *testing.T) {
	usd := mustAsset(t, "USD")
	eur := mustAsset(t, "EUR")
	account := param.NewAccountIDFromUint64(83021)
	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFunds()).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	seedSpotFundsLifecycleAccount(t, engine, account, usd)
	if _, err := engine.Configure().SetSpotFundsAccountPnl(
		policies.SpotFundsPolicyName,
		account,
		model.NewPnlState(mustPnl(t, "-150")),
	); err != nil {
		t.Fatalf("SetSpotFundsAccountPnl() error = %v", err)
	}

	mismatchedBarrier := policies.SpotFundsPnlBoundsBarrier{
		Mode:       reject.RuleModeReject,
		Currency:   eur,
		LowerBound: optional.Some(mustPnl(t, "-100")),
	}
	mismatched, err := engine.Configure().SpotFundsPnlBoundsKillSwitch(
		policies.SpotFundsPolicyName,
		[]policies.SpotFundsPnlBoundsBarrier{mismatchedBarrier},
		nil,
		nil,
	)
	if err != nil {
		t.Fatalf(
			"SpotFundsPnlBoundsKillSwitch() nonmatching retune error = %v",
			err,
		)
	}
	if len(mismatched.AccountBlocks) != 0 {
		t.Fatalf(
			"nonmatching retune AccountBlocks = %v, want none",
			mismatched.AccountBlocks,
		)
	}

	reservation, rejects, err := engine.ExecutePreTrade(
		spotFundsLifecycleOrder(t, account),
	)
	if err != nil {
		t.Fatalf(
			"nonmatching retune ExecutePreTrade() error = %v",
			err,
		)
	}
	if reservation == nil || len(rejects) != 0 {
		t.Fatalf(
			"nonmatching retune result = (%v, %v), want reservation",
			reservation,
			rejects,
		)
	}
	reservation.RollbackAndClose()

	matchingBarrier := mismatchedBarrier
	matchingBarrier.Currency = usd
	matching, err := engine.Configure().SpotFundsPnlBoundsKillSwitch(
		policies.SpotFundsPolicyName,
		[]policies.SpotFundsPnlBoundsBarrier{matchingBarrier},
		nil,
		nil,
	)
	if err != nil {
		t.Fatalf(
			"SpotFundsPnlBoundsKillSwitch() matching retune error = %v",
			err,
		)
	}
	if len(matching.AccountBlocks) != 1 ||
		matching.AccountBlocks[0].Block.Code != reject.CodePnlKillSwitchTriggered {
		t.Fatalf(
			"matching retune AccountBlocks = %v, want one PnL block",
			matching.AccountBlocks,
		)
	}
	assertSpotFundsPnlPreTradeReject(t, engine, account)
}

func TestSpotFundsPnlBoundsKillSwitchBuilderRequiresBarrier(t *testing.T) {
	_, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFundsPnlBoundsKillSwitch()).
		Build()
	if err == nil {
		t.Fatal("Build() error = nil, want at-least-one-barrier validation error")
	}
}

func TestSpotFundsPnlModesReachPolicy(t *testing.T) {
	rejecting := policies.SpotFundsPnlBoundsBarrier{
		Mode: reject.RuleModeReject, Currency: mustAsset(t, "USD"),
		LowerBound: optional.Some(mustPnl(t, "-100")),
	}
	reviewing := rejecting
	reviewing.Mode = reject.RuleModeReviewWithHold
	reviewing.LowerBound = optional.Some(mustPnl(t, "-10"))
	otherReview := reviewing
	otherReview.Mode = reject.RuleModeReviewWithoutHold
	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFundsPnlBoundsKillSwitch().GlobalBarriers(rejecting, reviewing, otherReview)).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	t.Cleanup(engine.Stop)
	account := param.NewAccountIDFromUint64(83301)
	seedSpotFundsLifecycleAccount(t, engine, account, rejecting.Currency)
	seeded, err := engine.Configure().SetSpotFundsAccountPnl(
		policies.SpotFundsPolicyName, account, model.NewPnlState(mustPnl(t, "-50")),
	)
	if err != nil || len(seeded.AccountBlocks) != 0 {
		t.Fatalf("review breach blocks = %v, error = %v, want none", seeded.AccountBlocks, err)
	}
	reservation, rejects, err := engine.ExecutePreTrade(spotFundsLifecycleOrder(t, account))
	if reservation != nil {
		defer reservation.RollbackAndClose()
	}
	if err != nil || reservation == nil || len(rejects) != 0 {
		t.Fatalf("review breach = (%v, %v, %v), want reservation without rejects", reservation, rejects, err)
	}
	reviews, err := reservation.Reviews()
	counts := map[reject.RuleMode]int{}
	for _, mark := range reviews {
		if mark.Code != reject.CodePnlKillSwitchTriggered {
			t.Fatalf("review = %v, want PnlKillSwitchTriggered", mark)
		}
		counts[mark.Mode]++
	}
	if err != nil || len(reviews) != 2 || counts[reviewing.Mode] != 1 || counts[otherReview.Mode] != 1 {
		t.Fatalf("reviews = %v, error = %v, want both PnL review modes", reviews, err)
	}
	reservation.RollbackAndClose()
	confirmed := spotFundsLifecycleOrder(t, account)
	confirmedReview := confirmed.EnsureReviewView()
	confirmedReview.SetConfirmed(true)
	reservation, rejects, err = engine.ExecutePreTrade(confirmed)
	if err != nil || reservation == nil || len(rejects) != 0 {
		t.Fatalf("confirmed review breach = %v, rejects = %v, error = %v", reservation, rejects, err)
	}
	marks, err := reservation.Reviews()
	if err != nil || len(marks) != 0 {
		t.Fatalf("confirmed PnL marks = %v, error = %v, want none", marks, err)
	}
	reservation.RollbackAndClose()
	breached, err := engine.Configure().SetSpotFundsAccountPnl(
		policies.SpotFundsPolicyName, account, model.NewPnlState(mustPnl(t, "-150")),
	)
	if err != nil || len(breached.AccountBlocks) != 1 || breached.AccountBlocks[0].Code != reject.CodePnlKillSwitchTriggered {
		t.Fatalf("reject breach blocks = %v, error = %v, want one PnL block", breached.AccountBlocks, err)
	}
	engine.Accounts().Unblock(account)
	reservation, rejects, err = engine.ExecutePreTrade(confirmed)
	if reservation != nil {
		reservation.RollbackAndClose()
	}
	if err != nil || reservation != nil || len(rejects) != 1 || rejects[0].Code != reject.CodePnlKillSwitchTriggered || rejects[0].Mode != reject.RuleModeReject {
		t.Fatalf("confirmed Reject barrier = %v, rejects = %v, error = %v", reservation, rejects, err)
	}
}

func TestSpotFundsPnlInvalidModesFailBuild(t *testing.T) {
	for _, mode := range []reject.RuleMode{3, 255} {
		for _, axis := range []string{"global", "account_group"} {
			t.Run(fmt.Sprintf("%s/mode_%d", axis, mode), func(t *testing.T) {
				barrier := policies.SpotFundsPnlBoundsBarrier{
					Mode: mode, Currency: mustAsset(t, "USD"), LowerBound: optional.Some(mustPnl(t, "-10")),
				}
				builder := policies.BuildSpotFundsPnlBoundsKillSwitch()
				var ready *policies.SpotFundsPnlBoundsKillSwitchReadyBuilder
				validBarrier := barrier
				validBarrier.Mode = reject.RuleModeReject
				var validReady *policies.SpotFundsPnlBoundsKillSwitchReadyBuilder
				if axis == "global" {
					ready = builder.GlobalBarriers(barrier)
					validReady = policies.BuildSpotFundsPnlBoundsKillSwitch().GlobalBarriers(validBarrier)
				} else {
					ready = builder.AccountGroupBarriers(policies.SpotFundsPnlBoundsAccountGroupBarrier{
						AccountGroupID: mustAccountGroupID(t, 42), Barrier: barrier,
					})
					validReady = policies.BuildSpotFundsPnlBoundsKillSwitch().AccountGroupBarriers(policies.SpotFundsPnlBoundsAccountGroupBarrier{
						AccountGroupID: mustAccountGroupID(t, 42), Barrier: validBarrier,
					})
				}
				engine, err := openpit.NewEngineBuilder().NoSync().Builtin(ready).Build()
				if engine != nil {
					engine.Stop()
				}
				if err == nil {
					t.Fatal("Build() succeeded with invalid mode")
				}
				validEngine, err := openpit.NewEngineBuilder().NoSync().Builtin(validReady).Build()
				if err != nil {
					t.Fatalf("valid mode Build() = %v", err)
				}
				validEngine.Stop()
			})
		}
	}
}

func TestSpotFundsPnlInvalidAccountModeBuildReportsTierAndIndex(t *testing.T) {
	for _, mode := range []reject.RuleMode{3, 255} {
		t.Run(fmt.Sprintf("mode_%d", mode), func(t *testing.T) {
			valid := policies.SpotFundsPnlBoundsAccountBarrier{
				AccountID: param.NewAccountIDFromUint64(42),
				Barrier: policies.SpotFundsPnlBoundsBarrier{
					Currency: mustAsset(t, "USD"), LowerBound: optional.Some(mustPnl(t, "-10")),
				},
			}
			ready := policies.BuildSpotFundsPnlBoundsKillSwitch().AccountBarriers(valid)
			first := valid
			valid.AccountID = param.NewAccountIDFromUint64(43)
			invalid := valid
			invalid.AccountID = param.NewAccountIDFromUint64(44)
			invalid.Barrier.Mode = mode
			ready.AccountBarriers(valid, invalid)
			engine, err := openpit.NewEngineBuilder().NoSync().Builtin(ready).Build()
			if engine != nil {
				engine.Stop()
			}
			if err == nil {
				t.Fatal("Build() succeeded with invalid account mode")
			}
			invalid.Barrier.Mode = reject.RuleModeReject
			validEngine, err := openpit.NewEngineBuilder().NoSync().Builtin(
				policies.BuildSpotFundsPnlBoundsKillSwitch().AccountBarriers(first, valid, invalid),
			).Build()
			if err != nil {
				t.Fatalf("valid account mode Build() = %v", err)
			}
			validEngine.Stop()
		})
	}
}

func TestSpotFundsPnlInvalidModeConfigureReportsTierAndIndex(t *testing.T) {
	engine, err := openpit.NewEngineBuilder().NoSync().Builtin(policies.BuildSpotFunds()).Build()
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(engine.Stop)
	valid := policies.SpotFundsPnlBoundsAccountBarrier{
		AccountID: param.NewAccountIDFromUint64(42),
		Barrier: policies.SpotFundsPnlBoundsBarrier{
			Currency: mustAsset(t, "USD"), LowerBound: optional.Some(mustPnl(t, "-10")),
		},
	}
	for _, mode := range []reject.RuleMode{3, 255} {
		for _, axis := range []string{"global", "account_group", "account"} {
			t.Run(fmt.Sprintf("%s/mode_%d", axis, mode), func(t *testing.T) {
				invalid := valid
				invalid.AccountID = param.NewAccountIDFromUint64(43)
				invalid.Barrier.Mode = mode
				var globals []policies.SpotFundsPnlBoundsBarrier
				var groups []policies.SpotFundsPnlBoundsAccountGroupBarrier
				var accounts []policies.SpotFundsPnlBoundsAccountBarrier
				switch axis {
				case "global":
					globals = []policies.SpotFundsPnlBoundsBarrier{valid.Barrier, invalid.Barrier}
				case "account_group":
					groups = []policies.SpotFundsPnlBoundsAccountGroupBarrier{
						{AccountGroupID: mustAccountGroupID(t, 42), Barrier: valid.Barrier},
						{AccountGroupID: mustAccountGroupID(t, 43), Barrier: invalid.Barrier},
					}
				case "account":
					accounts = []policies.SpotFundsPnlBoundsAccountBarrier{valid, invalid}
				}
				_, err := engine.Configure().SpotFundsPnlBoundsKillSwitch(
					policies.SpotFundsPolicyName, globals, groups, accounts,
				)
				var configErr *configure.Error
				if !errors.As(err, &configErr) || configErr.Kind != configure.ErrorKindValidation {
					t.Fatalf("configure error = %v (%T), want Validation", err, err)
				}
				switch axis {
				case "global":
					globals[1].Mode = reject.RuleModeReviewWithHold
				case "account_group":
					groups[1].Barrier.Mode = reject.RuleModeReviewWithHold
				case "account":
					accounts[1].Barrier.Mode = reject.RuleModeReviewWithHold
				}
				if _, err := engine.Configure().SpotFundsPnlBoundsKillSwitch(policies.SpotFundsPolicyName, globals, groups, accounts); err != nil {
					t.Fatalf("valid mode configure = %v", err)
				}
			})
		}
	}
}

func TestSpotFundsPnlZeroModeIsReject(t *testing.T) {
	barrier := policies.SpotFundsPnlBoundsBarrier{
		Currency: mustAsset(t, "USD"), LowerBound: optional.Some(mustPnl(t, "-100")),
	}
	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFundsPnlBoundsKillSwitch().GlobalBarriers(barrier)).Build()
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(engine.Stop)
	account := param.NewAccountIDFromUint64(83303)
	if err := engine.Accounts().SetCurrency(account, barrier.Currency); err != nil {
		t.Fatal(err)
	}
	barrier.LowerBound = optional.Some(mustPnl(t, "-10"))
	if _, err := engine.Configure().SpotFundsPnlBoundsKillSwitch(
		policies.SpotFundsPolicyName, []policies.SpotFundsPnlBoundsBarrier{barrier}, nil, nil,
	); err != nil {
		t.Fatal(err)
	}
	result, err := engine.Configure().SetSpotFundsAccountPnl(
		policies.SpotFundsPolicyName, account, model.NewPnlState(mustPnl(t, "-20")),
	)
	if err != nil || len(result.AccountBlocks) != 1 || result.AccountBlocks[0].Code != reject.CodePnlKillSwitchTriggered {
		t.Fatalf("zero-mode breach blocks = %v, error = %v, want one PnL block", result.AccountBlocks, err)
	}
	reservation, rejects, err := engine.ExecutePreTrade(spotFundsLifecycleOrder(t, account))
	if reservation != nil {
		defer reservation.RollbackAndClose()
	}
	if err != nil || reservation != nil || len(rejects) != 1 || rejects[0].Mode != reject.RuleModeReject {
		t.Fatalf("zero-mode breach = (%v, %v, %v), want an effective Reject", reservation, rejects, err)
	}
}

func TestSpotFundsPnlGlobalListsPatchAllModes(t *testing.T) {
	rejecting := policies.SpotFundsPnlBoundsBarrier{
		Mode: reject.RuleModeReject, Currency: mustAsset(t, "USD"),
		LowerBound: optional.Some(mustPnl(t, "-100")),
	}
	reviewing := rejecting
	reviewing.Mode = reject.RuleModeReviewWithHold
	reviewing.LowerBound = optional.Some(mustPnl(t, "-10"))
	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFundsPnlBoundsKillSwitch().GlobalBarriers(rejecting)).Build()
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(engine.Stop)
	account := param.NewAccountIDFromUint64(83302)
	if err := engine.Accounts().SetCurrency(account, rejecting.Currency); err != nil {
		t.Fatal(err)
	}
	seeded, err := engine.Configure().SetSpotFundsAccountPnl(
		policies.SpotFundsPolicyName, account, model.NewPnlState(mustPnl(t, "-50")),
	)
	if err != nil || len(seeded.AccountBlocks) != 0 {
		t.Fatalf("seed blocks = %v, error = %v, want none", seeded.AccountBlocks, err)
	}
	order := spotFundsLifecycleOrder(t, account)
	checkReviews := func(count int) {
		t.Helper()
		report, err := engine.ExecutePreTradeDryRun(order)
		if err != nil {
			t.Fatal(err)
		}
		defer report.Close()
		passed, err := report.IsPass()
		if err != nil || !passed {
			t.Fatalf("dry run passed = %v, error = %v, want passing review or clean order", passed, err)
		}
		reviews, err := report.Reviews()
		if err != nil || len(reviews) != count {
			t.Fatalf("reviews = %v, error = %v, want %d marks", reviews, err, count)
		}
		if count > 0 && (reviews[0].Mode != reject.RuleModeReviewWithHold || reviews[0].Code != reject.CodePnlKillSwitchTriggered) {
			t.Fatalf("review = %v, want with-hold PnL mark", reviews[0])
		}
	}
	update := func(barriers []policies.SpotFundsPnlBoundsBarrier) {
		t.Helper()
		result, err := engine.Configure().SpotFundsPnlBoundsKillSwitch(
			policies.SpotFundsPolicyName, barriers, nil, nil,
		)
		if err != nil || len(result.AccountBlocks) != 0 {
			t.Fatalf("update blocks = %v, error = %v, want none", result.AccountBlocks, err)
		}
	}
	update([]policies.SpotFundsPnlBoundsBarrier{rejecting, reviewing})
	checkReviews(1)
	update(nil)
	checkReviews(1)
	invalid := reviewing
	invalid.Mode = 3
	update([]policies.SpotFundsPnlBoundsBarrier{reviewing})
	update([]policies.SpotFundsPnlBoundsBarrier{rejecting, reviewing})
	for _, barriers := range [][]policies.SpotFundsPnlBoundsBarrier{
		{invalid}, {reviewing, reviewing},
	} {
		_, err := engine.Configure().SpotFundsPnlBoundsKillSwitch(
			policies.SpotFundsPolicyName, barriers, nil, nil,
		)
		var configErr *configure.Error
		if !errors.As(err, &configErr) || configErr.Kind != configure.ErrorKindValidation {
			t.Fatalf("configure error = %v (%T), want Validation", err, err)
		}
		checkReviews(1)
	}
	update([]policies.SpotFundsPnlBoundsBarrier{rejecting})
	checkReviews(0)
	update([]policies.SpotFundsPnlBoundsBarrier{rejecting, reviewing})
	update([]policies.SpotFundsPnlBoundsBarrier{})
	checkReviews(0)
	cleared, err := engine.Configure().SetSpotFundsAccountPnl(
		policies.SpotFundsPolicyName, account, model.NewPnlState(mustPnl(t, "-150")),
	)
	if err != nil || len(cleared.AccountBlocks) != 0 {
		t.Fatalf("cleared blocks = %v, error = %v, want none", cleared.AccountBlocks, err)
	}
	checkReviews(0)
}

func TestSpotFundsReviewPnlDuplicateTiersAreAtomic(t *testing.T) {
	for _, tier := range []string{"group", "account"} {
		t.Run(tier, func(t *testing.T) {
			account := param.NewAccountIDFromUint64(83403)
			group := mustAccountGroupID(t, 87)
			barrier := policies.SpotFundsPnlBoundsBarrier{Mode: reject.RuleModeReviewWithHold, Currency: mustAsset(t, "USD"), LowerBound: optional.Some(mustPnl(t, "-10"))}
			builder := policies.BuildSpotFundsPnlBoundsKillSwitch()
			if tier == "group" {
				entry := policies.SpotFundsPnlBoundsAccountGroupBarrier{AccountGroupID: group, Barrier: barrier}
				builder.AccountGroupBarriers(entry, entry)
			} else {
				entry := policies.SpotFundsPnlBoundsAccountBarrier{AccountID: account, Barrier: barrier}
				builder.AccountBarriers(entry, entry)
			}
			invalid, err := openpit.NewEngineBuilder().NoSync().Builtin(builder).Build()
			if invalid != nil {
				invalid.Stop()
			}
			if err == nil {
				t.Fatal("duplicate tier build succeeded")
			}
			validBuilder := policies.BuildSpotFundsPnlBoundsKillSwitch()
			if tier == "group" {
				validBuilder.AccountGroupBarriers(policies.SpotFundsPnlBoundsAccountGroupBarrier{AccountGroupID: group, Barrier: barrier})
			} else {
				validBuilder.AccountBarriers(policies.SpotFundsPnlBoundsAccountBarrier{AccountID: account, Barrier: barrier})
			}
			engine, err := openpit.NewEngineBuilder().NoSync().Builtin(validBuilder).Build()
			if err != nil {
				t.Fatal(err)
			}
			t.Cleanup(engine.Stop)
			seedSpotFundsLifecycleAccount(t, engine, account, barrier.Currency)
			if err := engine.Accounts().RegisterGroup([]param.AccountID{account}, group); err != nil {
				t.Fatal(err)
			}
			if _, err := engine.Configure().SetSpotFundsAccountPnl(policies.SpotFundsPolicyName, account, model.NewPnlState(mustPnl(t, "-50"))); err != nil {
				t.Fatal(err)
			}
			var groups []policies.SpotFundsPnlBoundsAccountGroupBarrier
			var accounts []policies.SpotFundsPnlBoundsAccountBarrier
			barrier.LowerBound = optional.Some(mustPnl(t, "-1000"))
			if tier == "group" {
				entry := policies.SpotFundsPnlBoundsAccountGroupBarrier{AccountGroupID: group, Barrier: barrier}
				groups = []policies.SpotFundsPnlBoundsAccountGroupBarrier{entry, entry}
			} else {
				entry := policies.SpotFundsPnlBoundsAccountBarrier{AccountID: account, Barrier: barrier}
				accounts = []policies.SpotFundsPnlBoundsAccountBarrier{entry, entry}
			}
			_, err = engine.Configure().SpotFundsPnlBoundsKillSwitch(policies.SpotFundsPolicyName, nil, groups, accounts)
			var configErr *configure.Error
			if !errors.As(err, &configErr) || configErr.Kind != configure.ErrorKindValidation {
				t.Fatalf("duplicate tier error = %v, want Validation", err)
			}
			reservation, rejects, err := engine.ExecutePreTrade(spotFundsLifecycleOrder(t, account))
			if err != nil || reservation == nil || len(rejects) != 0 {
				t.Fatalf("after invalid update = %v, rejects = %v, error = %v", reservation, rejects, err)
			}
			defer reservation.RollbackAndClose()
			marks, err := reservation.Reviews()
			if err != nil || len(marks) != 1 || marks[0].Mode != barrier.Mode || marks[0].Code != reject.CodePnlKillSwitchTriggered {
				t.Fatalf("retained barrier marks = %v, error = %v", marks, err)
			}
		})
	}
}

func TestSpotFundsPnlDuplicateModeFailsBuild(t *testing.T) {
	barrier := policies.SpotFundsPnlBoundsBarrier{
		Mode: reject.RuleModeReviewWithHold, Currency: mustAsset(t, "USD"),
		LowerBound: optional.Some(mustPnl(t, "-10")),
	}
	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFundsPnlBoundsKillSwitch().GlobalBarriers(barrier).GlobalBarriers(barrier)).Build()
	if engine != nil {
		engine.Stop()
	}
	if err == nil {
		t.Fatalf("Build() error = %v, want duplicate mode error", err)
	}
	valid, err := openpit.NewEngineBuilder().NoSync().Builtin(
		policies.BuildSpotFundsPnlBoundsKillSwitch().GlobalBarriers(barrier),
	).Build()
	if err != nil {
		t.Fatalf("single barrier Build() = %v", err)
	}
	valid.Stop()
}

func TestSpotFundsGroupMembershipArmsEffectivePnlBarrier(t *testing.T) {
	account := param.NewAccountIDFromUint64(83010)
	group := mustAccountGroupID(t, 84)
	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFundsPnlBoundsKillSwitch().
			AccountGroupBarriers(policies.SpotFundsPnlBoundsAccountGroupBarrier{
				AccountGroupID: group,
				Barrier: policies.SpotFundsPnlBoundsBarrier{
					Mode:       reject.RuleModeReject,
					Currency:   mustAsset(t, "USD"),
					LowerBound: optional.Some(mustPnl(t, "1")),
				},
			}),
		).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	if err := engine.Accounts().RegisterGroup([]param.AccountID{account}, group); err != nil {
		t.Fatalf("Accounts().RegisterGroup() error = %v", err)
	}
	assertSpotFundsPnlPreTradeReject(t, engine, account)
}

func TestSpotFundsPnlBoundsRuntimeAxisReplacementAndClear(t *testing.T) {
	usd := mustAsset(t, "USD")
	group := mustAccountGroupID(t, 85)
	accountSpecific := param.NewAccountIDFromUint64(83011)
	accountGroup := param.NewAccountIDFromUint64(83012)
	accountSafe := param.NewAccountIDFromUint64(83013)
	accountGlobal := param.NewAccountIDFromUint64(83014)
	accountAfterClear := param.NewAccountIDFromUint64(83015)

	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFunds()).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	for _, account := range []param.AccountID{
		accountSpecific,
		accountGroup,
		accountGlobal,
		accountAfterClear,
		accountSafe,
	} {
		seedSpotFundsLifecycleAccount(t, engine, account, usd)
	}
	if err := engine.Accounts().RegisterGroup([]param.AccountID{accountGroup}, group); err != nil {
		t.Fatalf("Accounts().RegisterGroup() error = %v", err)
	}
	globalBarrier := policies.SpotFundsPnlBoundsBarrier{
		Mode:       reject.RuleModeReject,
		Currency:   usd,
		LowerBound: optional.Some(mustPnl(t, "-20")),
	}

	halted, err := model.NewPnlHaltedState(model.PnlHaltReasonMissingFx)
	if err != nil {
		t.Fatalf("NewPnlHaltedState() error = %v", err)
	}
	for _, account := range []struct {
		id    param.AccountID
		state model.PnlState
	}{
		{accountSpecific, model.NewPnlState(mustPnl(t, "-15"))},
		{accountGroup, model.NewPnlState(mustPnl(t, "-15"))},
		{accountGlobal, model.NewPnlState(mustPnl(t, "-25"))},
		{accountAfterClear, halted},
		{accountSafe, model.NewPnlState(mustPnl(t, "-5"))},
	} {
		if _, err := engine.Configure().SetSpotFundsAccountPnl(
			policies.SpotFundsPolicyName,
			account.id,
			account.state,
		); err != nil {
			t.Fatalf("SetSpotFundsAccountPnl() error = %v", err)
		}
	}

	// Arming the axes evaluates them at once: every seeded account already sits
	// past its new bound, so all four are blocked before this call returns.
	armed, err := engine.Configure().SpotFundsPnlBoundsKillSwitch(
		policies.SpotFundsPolicyName,
		[]policies.SpotFundsPnlBoundsBarrier{globalBarrier},
		[]policies.SpotFundsPnlBoundsAccountGroupBarrier{{
			AccountGroupID: group,
			Barrier: policies.SpotFundsPnlBoundsBarrier{
				Mode:       reject.RuleModeReject,
				Currency:   usd,
				LowerBound: optional.Some(mustPnl(t, "-10")),
			},
		}},
		[]policies.SpotFundsPnlBoundsAccountBarrier{{
			AccountID: accountSpecific,
			Barrier: policies.SpotFundsPnlBoundsBarrier{
				Mode:       reject.RuleModeReject,
				Currency:   usd,
				LowerBound: optional.Some(mustPnl(t, "-10")),
			},
		}},
	)
	if err != nil {
		t.Fatalf("SpotFundsPnlBoundsKillSwitch() setup error = %v", err)
	}
	if len(armed.AccountBlocks) != 4 {
		t.Fatalf("arming AccountBlocks = %v, want four breached accounts", armed.AccountBlocks)
	}
	type accountBlockPair struct {
		account param.AccountID
		code    reject.Code
	}
	wantOutcomes := []struct {
		pair   accountBlockPair
		reason string
	}{
		{
			pair: accountBlockPair{
				account: accountSpecific,
				code:    reject.CodePnlKillSwitchTriggered,
			},
			reason: "pnl kill switch triggered",
		},
		{
			pair: accountBlockPair{
				account: accountGroup,
				code:    reject.CodePnlKillSwitchTriggered,
			},
			reason: "pnl kill switch triggered",
		},
		{
			pair: accountBlockPair{
				account: accountGlobal,
				code:    reject.CodePnlKillSwitchTriggered,
			},
			reason: "pnl kill switch triggered",
		},
		{
			pair: accountBlockPair{
				account: accountAfterClear,
				code:    reject.CodePnlKillSwitchTriggered,
			},
			reason: "account pnl calculation halted",
		},
	}
	for index, want := range wantOutcomes {
		outcome := armed.AccountBlocks[index]
		gotPair := accountBlockPair{
			account: outcome.AccountID,
			code:    outcome.Block.Code,
		}
		if gotPair != want.pair {
			t.Fatalf(
				"AccountBlocks[%d] (account, code) = %v, want %v",
				index,
				gotPair,
				want.pair,
			)
		}
		if outcome.Block.Reason != want.reason {
			t.Fatalf(
				"AccountBlocks[%d].Block = %v, want reason %q",
				index,
				outcome.Block,
				want.reason,
			)
		}
	}
	for _, account := range []param.AccountID{
		accountSpecific,
		accountGroup,
		accountGlobal,
		accountAfterClear,
	} {
		assertSpotFundsPnlPreTradeReject(t, engine, account)
	}
	reservation, rejects, err := engine.ExecutePreTrade(
		spotFundsLifecycleOrder(t, accountSafe),
	)
	if err != nil {
		t.Fatalf("safe account ExecutePreTrade() error = %v", err)
	}
	if reservation == nil || len(rejects) != 0 {
		t.Fatalf("safe account result = (%v, %v), want reservation", reservation, rejects)
	}
	reservation.RollbackAndClose()
	// The operator lifts the arming blocks; the axes themselves stay in force,
	// so the cascade below is decided by the live barriers.
	for _, account := range []param.AccountID{
		accountSpecific,
		accountGroup,
		accountGlobal,
		accountAfterClear,
	} {
		engine.Accounts().Unblock(account)
	}

	// A non-nil empty account axis clears only per-account barriers. The
	// omitted global and group axes must keep affecting their respective keys.
	// Only accountSpecific changes tier, and the wider global bound admits it.
	cleared, err := engine.Configure().SpotFundsPnlBoundsKillSwitch(
		policies.SpotFundsPolicyName,
		nil,
		nil,
		[]policies.SpotFundsPnlBoundsAccountBarrier{},
	)
	if err != nil {
		t.Fatalf("SpotFundsPnlBoundsKillSwitch() account clear error = %v", err)
	}
	if len(cleared.AccountBlocks) != 0 {
		t.Fatalf("account-clear AccountBlocks = %v, want none", cleared.AccountBlocks)
	}

	if result := applySpotFundsLifecycleFill(t, engine, accountSpecific); len(result.AccountBlocks) != 0 {
		t.Fatalf("specific account AccountBlocks = %v, want none after clear", result.AccountBlocks)
	}
	assertSpotFundsPnlPreTradeReject(t, engine, accountGroup)
	assertSpotFundsPnlPreTradeReject(t, engine, accountGlobal)

	// Runtime updates may clear every axis. Unlike the explicit batch builder,
	// this is a patch operation and has no at-least-one-barrier requirement.
	if _, err := engine.Configure().SpotFundsPnlBoundsKillSwitch(
		policies.SpotFundsPolicyName,
		[]policies.SpotFundsPnlBoundsBarrier{},
		[]policies.SpotFundsPnlBoundsAccountGroupBarrier{},
		[]policies.SpotFundsPnlBoundsAccountBarrier{},
	); err != nil {
		t.Fatalf("SpotFundsPnlBoundsKillSwitch() full clear error = %v", err)
	}
	if result := applySpotFundsLifecycleFill(t, engine, accountAfterClear); len(result.AccountBlocks) != 0 {
		t.Fatalf("full-clear AccountBlocks = %v, want none", result.AccountBlocks)
	}
}

func TestSpotFundsPnlBoundsRuntimeAdditionRetainsLivePnl(t *testing.T) {
	usd := mustAsset(t, "USD")
	account := param.NewAccountIDFromUint64(83015)

	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFunds()).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	seedSpotFundsLifecycleAccount(t, engine, account, usd)
	if result := applySpotFundsLifecycleFill(t, engine, account); len(result.AccountBlocks) != 0 {
		t.Fatalf("initial AccountBlocks = %v, want none", result.AccountBlocks)
	}

	globalBarrier := policies.SpotFundsPnlBoundsBarrier{
		Mode:       reject.RuleModeReject,
		Currency:   usd,
		LowerBound: optional.Some(mustPnl(t, "-30")),
	}
	if _, err := engine.Configure().SpotFundsPnlBoundsKillSwitch(
		policies.SpotFundsPolicyName,
		[]policies.SpotFundsPnlBoundsBarrier{globalBarrier},
		nil,
		nil,
	); err != nil {
		t.Fatalf("SpotFundsPnlBoundsKillSwitch() add global error = %v", err)
	}
	configuration, err := engine.Configure().SetSpotFundsAccountPnl(
		policies.SpotFundsPolicyName,
		account,
		model.NewPnlState(mustPnl(t, "-40")),
	)
	if err != nil {
		t.Fatalf("SetSpotFundsAccountPnl() error = %v", err)
	}
	if len(configuration.AccountBlocks) != 1 ||
		configuration.AccountBlocks[0].Code != reject.CodePnlKillSwitchTriggered {
		t.Fatalf("SetSpotFundsAccountPnl() blocks = %v, want one PnL block", configuration.AccountBlocks)
	}

	// Replacing the global barrier must preserve the account accumulator instead
	// of reseeding it.
	if _, err := engine.Configure().SpotFundsPnlBoundsKillSwitch(
		policies.SpotFundsPolicyName,
		[]policies.SpotFundsPnlBoundsBarrier{globalBarrier},
		nil,
		nil,
	); err != nil {
		t.Fatalf("SpotFundsPnlBoundsKillSwitch() replace global error = %v", err)
	}
	assertSpotFundsPnlPreTradeReject(t, engine, account)
}

func TestSpotFundsPnlBoundsAxisSelector(t *testing.T) {
	tests := []struct {
		name string
		axis policies.SpotFundsPnlBoundsAxis
		want native.PretradePoliciesSpotFundsPnlBoundsAxis
	}{
		{"account-unrealized", policies.SpotFundsPnlBoundsAxisAccountUnrealized, native.PretradePoliciesSpotFundsPnlBoundsAxisAccountUnrealized},
		{"account-total", policies.SpotFundsPnlBoundsAxisAccountTotal, native.PretradePoliciesSpotFundsPnlBoundsAxisAccountTotal},
		{"position-unrealized", policies.SpotFundsPnlBoundsAxisPositionUnrealized, native.PretradePoliciesSpotFundsPnlBoundsAxisPositionUnrealized},
		{"position-total", policies.SpotFundsPnlBoundsAxisPositionTotal, native.PretradePoliciesSpotFundsPnlBoundsAxisPositionTotal},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			if got := native.PretradePoliciesSpotFundsPnlBoundsAxis(test.axis); got != test.want {
				t.Fatalf("axis value = %v, want native %v", got, test.want)
			}
			if got := test.axis.Handle(); got != test.want {
				t.Fatalf("Handle() = %v, want %v", got, test.want)
			}
			if got := test.axis.String(); got != test.name {
				t.Fatalf("String() = %q, want %q", got, test.name)
			}
		})
	}
	if got := policies.SpotFundsPnlBoundsAxis(4).String(); got != "SpotFundsPnlBoundsAxis(4)" {
		t.Fatalf("unknown String() = %q, want SpotFundsPnlBoundsAxis(4)", got)
	}
}

func newSpotFundsPnlAxisEngine(t *testing.T) (*openpit.Engine, *marketdata.Service) {
	t.Helper()
	service := mustMarketDataService(t)
	t.Cleanup(service.Close)
	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFunds().WithMarketOrders(service, 0)).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	t.Cleanup(engine.Stop)
	return engine, service
}

func setSpotFundsPnlAxisMark(t *testing.T, service *marketdata.Service, value string) {
	t.Helper()
	price, err := param.NewPriceFromString(value)
	if err != nil {
		t.Fatalf("NewPriceFromString(%q) error = %v", value, err)
	}
	if _, err := service.PushByInstrument(
		param.NewInstrument(mustAsset(t, "AAPL"), mustAsset(t, "USD")),
		marketdata.NewQuote().WithMark(price),
		0,
	); err != nil {
		t.Fatalf("PushByInstrument() error = %v", err)
	}
}

func TestSpotFundsPnlBoundsAxisRuntimeArming(t *testing.T) {
	tests := []struct {
		axis  policies.SpotFundsPnlBoundsAxis
		label string
	}{
		{policies.SpotFundsPnlBoundsAxisAccountUnrealized, "account unrealized pnl"},
		{policies.SpotFundsPnlBoundsAxisAccountTotal, "account total pnl"},
		{policies.SpotFundsPnlBoundsAxisPositionUnrealized, "position unrealized pnl"},
		{policies.SpotFundsPnlBoundsAxisPositionTotal, "position total pnl"},
	}
	for _, test := range tests {
		t.Run(test.axis.String(), func(t *testing.T) {
			engine, service := newSpotFundsPnlAxisEngine(t)
			account := param.NewAccountIDFromUint64(83101)
			usd := mustAsset(t, "USD")
			seedSpotFundsLifecycleAccount(t, engine, account, usd)
			setSpotFundsPnlAxisMark(t, service, "100")
			applySpotFundsLifecycleFill(t, engine, account)
			setSpotFundsPnlAxisMark(t, service, "90")
			barrier := policies.SpotFundsPnlBoundsBarrier{
				Mode:       reject.RuleModeReject,
				Currency:   usd,
				LowerBound: optional.Some(mustPnl(t, "-5")),
			}
			result, err := engine.Configure().SpotFundsPnlBoundsAxis(
				policies.SpotFundsPolicyName, test.axis, []policies.SpotFundsPnlBoundsBarrier{barrier}, nil, nil,
			)
			if err != nil {
				t.Fatalf("SpotFundsPnlBoundsAxis() error = %v", err)
			}
			if len(result.AccountBlocks) != 1 {
				t.Fatalf("arming AccountBlocks = %v, want one", result.AccountBlocks)
			}
			outcome := result.AccountBlocks[0]
			if outcome.AccountID != account || outcome.Block.Code != reject.CodePnlKillSwitchTriggered {
				t.Fatalf("arming outcome = %v, want account %v with PnlKillSwitchTriggered", outcome, account)
			}
			if !strings.Contains(outcome.Block.Details, test.label) {
				t.Fatalf("arming Details = %q, want axis label %q", outcome.Block.Details, test.label)
			}
		})
	}
}

func TestSpotFundsPnlBoundsAxisUncomputableBlocks(t *testing.T) {
	tests := []struct {
		axis  policies.SpotFundsPnlBoundsAxis
		label string
	}{
		{policies.SpotFundsPnlBoundsAxisAccountUnrealized, "account unrealized pnl"},
		{policies.SpotFundsPnlBoundsAxisAccountTotal, "account total pnl"},
		{policies.SpotFundsPnlBoundsAxisPositionUnrealized, "position unrealized pnl"},
		{policies.SpotFundsPnlBoundsAxisPositionTotal, "position total pnl"},
	}
	for _, test := range tests {
		t.Run(test.axis.String(), func(t *testing.T) {
			for _, scenario := range []string{"currency_mismatch", "halted"} {
				for _, mode := range []reject.RuleMode{reject.RuleModeReject, reject.RuleModeReviewWithHold, reject.RuleModeReviewWithoutHold} {
					for _, confirmed := range []bool{false, true} {
						t.Run(fmt.Sprintf("%s/mode_%d/confirmed_%t", scenario, mode, confirmed), func(t *testing.T) {
							engine, service := newSpotFundsPnlAxisEngine(t)
							account := param.NewAccountIDFromUint64(83101)
							usd := mustAsset(t, "USD")
							seedSpotFundsLifecycleAccount(t, engine, account, usd)
							setSpotFundsPnlAxisMark(t, service, "100")
							applySpotFundsLifecycleFill(t, engine, account)
							setSpotFundsPnlAxisMark(t, service, "90")
							barrier := policies.SpotFundsPnlBoundsBarrier{Mode: mode, Currency: usd, LowerBound: optional.Some(mustPnl(t, "-100"))}
							apply := func() (configure.AccountBlockOutcomes, error) {
								return engine.Configure().SpotFundsPnlBoundsAxis(policies.SpotFundsPolicyName, test.axis, nil, nil,
									[]policies.SpotFundsPnlBoundsAccountBarrier{{AccountID: account, Barrier: barrier}})
							}
							valid, err := apply()
							if err != nil || len(valid.AccountBlocks) != 0 {
								t.Fatalf("valid axis barrier = %v, %v, want no blocks", valid.AccountBlocks, err)
							}
							if scenario == "currency_mismatch" {
								barrier.Currency = mustAsset(t, "EUR")
							} else {
								_, err = service.PushByInstrument(param.NewInstrument(mustAsset(t, "AAPL"), usd), marketdata.NewQuote(), 0)
								if err != nil {
									t.Fatal(err)
								}
							}
							barrier.LowerBound = optional.Some(mustPnl(t, "-5"))
							blocked, err := apply()
							if err != nil || len(blocked.AccountBlocks) != 1 || blocked.AccountBlocks[0].AccountID != account || blocked.AccountBlocks[0].Block.Code != reject.CodePnlKillSwitchTriggered {
								t.Fatalf("uncomputable axis blocks = %v, %v, want PnlKillSwitchTriggered", blocked.AccountBlocks, err)
							}
							details := blocked.AccountBlocks[0].Block.Details
							if !strings.Contains(details, test.label) {
								t.Fatalf("axis details = %q, want %q", details, test.label)
							}
							if scenario == "halted" && (test.axis == policies.SpotFundsPnlBoundsAxisPositionUnrealized || test.axis == policies.SpotFundsPnlBoundsAxisPositionTotal) && !strings.Contains(details, "AAPL") {
								t.Fatalf("position halt details = %q, want asset", details)
							}
							clearAxis := func() {
								t.Helper()
								result, err := engine.Configure().SpotFundsPnlBoundsAxis(policies.SpotFundsPolicyName, test.axis, nil, nil, []policies.SpotFundsPnlBoundsAccountBarrier{})
								if err != nil || len(result.AccountBlocks) != 0 {
									t.Fatalf("clear uncomputable axis = %v, %v, want no blocks", result.AccountBlocks, err)
								}
							}
							clearAxis()
							order := spotFundsLifecycleOrder(t, account)
							reservation, rejects, err := engine.ExecutePreTrade(order)
							if reservation != nil {
								reservation.RollbackAndClose()
							}
							if err != nil || reservation != nil || len(rejects) == 0 {
								t.Fatalf("recorded axis block = %v, %v, %v, want rejection", reservation, rejects, err)
							}
							engine.Accounts().Unblock(account)
							blocked, err = apply()
							if err != nil || len(blocked.AccountBlocks) != 1 || blocked.AccountBlocks[0].Block.Code != reject.CodePnlKillSwitchTriggered {
								t.Fatalf("reapplied axis blocks = %v, %v", blocked.AccountBlocks, err)
							}
							engine.Accounts().Unblock(account)
							review := order.EnsureReviewView()
							review.SetConfirmed(confirmed)
							reservation, rejects, err = engine.ExecutePreTrade(order)
							if reservation != nil {
								reservation.RollbackAndClose()
							}
							if err != nil || reservation != nil || len(rejects) != 1 || rejects[0].Code != reject.CodePnlKillSwitchTriggered || rejects[0].Mode != reject.RuleModeReject {
								t.Fatalf("live uncomputable axis = %v, %v, %v, want PnlKillSwitchTriggered Reject", reservation, rejects, err)
							}
							clearAxis()
							reservation, rejects, err = engine.ExecutePreTrade(order)
							if reservation != nil {
								reservation.RollbackAndClose()
							}
							if err != nil || reservation != nil || len(rejects) != 1 || rejects[0].Code != reject.CodeAccountBlocked {
								t.Fatalf("live axis latch = %v, %v, %v, want AccountBlocked", reservation, rejects, err)
							}
						})
					}
				}
			}
		})
	}
}

func TestSpotFundsPnlBoundsAxisReviewMarks(t *testing.T) {
	tests := []struct {
		axis  policies.SpotFundsPnlBoundsAxis
		label string
	}{
		{policies.SpotFundsPnlBoundsAxisAccountUnrealized, "account unrealized pnl"},
		{policies.SpotFundsPnlBoundsAxisAccountTotal, "account total pnl"},
		{policies.SpotFundsPnlBoundsAxisPositionUnrealized, "position unrealized pnl"},
		{policies.SpotFundsPnlBoundsAxisPositionTotal, "position total pnl"},
	}
	for _, test := range tests {
		t.Run(test.axis.String(), func(t *testing.T) {
			engine, service := newSpotFundsPnlAxisEngine(t)
			account := param.NewAccountIDFromUint64(83101)
			usd := mustAsset(t, "USD")
			seedSpotFundsLifecycleAccount(t, engine, account, usd)
			setSpotFundsPnlAxisMark(t, service, "100")
			applySpotFundsLifecycleFill(t, engine, account)
			setSpotFundsPnlAxisMark(t, service, "90")
			barriers := make([]policies.SpotFundsPnlBoundsBarrier, 0, 3)
			for _, mode := range []reject.RuleMode{reject.RuleModeReject, reject.RuleModeReviewWithHold, reject.RuleModeReviewWithoutHold} {
				bound := "-5"
				if mode == reject.RuleModeReject {
					bound = "-100"
				}
				barriers = append(barriers, policies.SpotFundsPnlBoundsBarrier{Mode: mode, Currency: usd, LowerBound: optional.Some(mustPnl(t, bound))})
			}
			result, err := engine.Configure().SpotFundsPnlBoundsAxis(policies.SpotFundsPolicyName, test.axis, barriers, nil, nil)
			if err != nil || len(result.AccountBlocks) != 0 {
				t.Fatalf("review axis blocks = %v, error = %v", result.AccountBlocks, err)
			}
			for attempt := 0; attempt < 2; attempt++ {
				reservation, rejects, err := engine.ExecutePreTrade(spotFundsLifecycleOrder(t, account))
				if err != nil || reservation == nil || len(rejects) != 0 {
					t.Fatalf("axis attempt %d = %v, rejects = %v, error = %v", attempt, reservation, rejects, err)
				}
				marks, err := reservation.Reviews()
				reservation.RollbackAndClose()
				counts := map[reject.RuleMode]int{}
				for _, mark := range marks {
					if mark.Code != reject.CodePnlKillSwitchTriggered {
						t.Fatalf("axis mark = %v, want PnlKillSwitchTriggered", mark)
					}
					counts[mark.Mode]++
				}
				if err != nil || len(marks) != 2 || counts[reject.RuleModeReviewWithHold] != 1 || counts[reject.RuleModeReviewWithoutHold] != 1 {
					t.Fatalf("axis reviews = %v, error = %v, want both review modes", marks, err)
				}
			}
			cleared, err := engine.Configure().SpotFundsPnlBoundsAxis(
				policies.SpotFundsPolicyName, test.axis, []policies.SpotFundsPnlBoundsBarrier{}, nil, nil,
			)
			if err != nil || len(cleared.AccountBlocks) != 0 {
				t.Fatalf("clear axis = %v, %v, want no blocks", cleared.AccountBlocks, err)
			}
		})
	}
}

func TestSpotFundsPnlBoundsAxisRuntimeTiers(t *testing.T) {
	for _, tier := range []string{"group", "account"} {
		t.Run(tier, func(t *testing.T) {
			engine, service := newSpotFundsPnlAxisEngine(t)
			account := param.NewAccountIDFromUint64(83102)
			other := param.NewAccountIDFromUint64(83103)
			group := mustAccountGroupID(t, 86)
			usd := mustAsset(t, "USD")
			setSpotFundsPnlAxisMark(t, service, "100")
			for _, id := range []param.AccountID{account, other} {
				seedSpotFundsLifecycleAccount(t, engine, id, usd)
				applySpotFundsLifecycleFill(t, engine, id)
			}
			if err := engine.Accounts().RegisterGroup([]param.AccountID{account}, group); err != nil {
				t.Fatalf("RegisterGroup() error = %v", err)
			}
			setSpotFundsPnlAxisMark(t, service, "90")
			barrier := policies.SpotFundsPnlBoundsBarrier{
				Mode:       reject.RuleModeReject,
				Currency:   usd,
				LowerBound: optional.Some(mustPnl(t, "-5")),
			}
			var groups []policies.SpotFundsPnlBoundsAccountGroupBarrier
			var accounts []policies.SpotFundsPnlBoundsAccountBarrier
			if tier == "group" {
				groups = []policies.SpotFundsPnlBoundsAccountGroupBarrier{{AccountGroupID: group, Barrier: barrier}}
			} else {
				accounts = []policies.SpotFundsPnlBoundsAccountBarrier{{AccountID: account, Barrier: barrier}}
			}
			result, err := engine.Configure().SpotFundsPnlBoundsAxis(
				policies.SpotFundsPolicyName, policies.SpotFundsPnlBoundsAxisPositionTotal,
				nil, groups, accounts,
			)
			if err != nil {
				t.Fatalf("SpotFundsPnlBoundsAxis() error = %v", err)
			}
			if len(result.AccountBlocks) != 1 {
				t.Fatalf("tier AccountBlocks = %v, want one", result.AccountBlocks)
			}
			outcome := result.AccountBlocks[0]
			if outcome.AccountID != account || outcome.Block.Code != reject.CodePnlKillSwitchTriggered {
				t.Fatalf("tier outcome = %v, want account %v with PnlKillSwitchTriggered", outcome, account)
			}
		})
	}
}

func TestSpotFundsPnlBoundsAxisRuntimePatch(t *testing.T) {
	for _, axis := range []policies.SpotFundsPnlBoundsAxis{policies.SpotFundsPnlBoundsAxisAccountUnrealized, policies.SpotFundsPnlBoundsAxisAccountTotal, policies.SpotFundsPnlBoundsAxisPositionUnrealized, policies.SpotFundsPnlBoundsAxisPositionTotal} {
		for _, tier := range []string{"global", "group", "account"} {
			for _, update := range []string{"unchanged", "clear", "duplicate"} {
				t.Run(axis.String()+"/"+tier+"/"+update, func(t *testing.T) {
					engine, service := newSpotFundsPnlAxisEngine(t)
					account := param.NewAccountIDFromUint64(83104)
					group := mustAccountGroupID(t, 87)
					usd := mustAsset(t, "USD")
					barrier := policies.SpotFundsPnlBoundsBarrier{
						Mode:       reject.RuleModeReject,
						Currency:   usd,
						LowerBound: optional.Some(mustPnl(t, "-5")),
					}
					var global []policies.SpotFundsPnlBoundsBarrier
					var groups []policies.SpotFundsPnlBoundsAccountGroupBarrier
					var accounts []policies.SpotFundsPnlBoundsAccountBarrier
					switch tier {
					case "global":
						global = []policies.SpotFundsPnlBoundsBarrier{barrier}
					case "group":
						groups = []policies.SpotFundsPnlBoundsAccountGroupBarrier{{AccountGroupID: group, Barrier: barrier}}
					case "account":
						accounts = []policies.SpotFundsPnlBoundsAccountBarrier{{AccountID: account, Barrier: barrier}}
					}
					if _, err := engine.Configure().SpotFundsPnlBoundsAxis(
						policies.SpotFundsPolicyName, axis,
						global, groups, accounts,
					); err != nil {
						t.Fatalf("initial retune error = %v", err)
					}
					global = nil
					groups = nil
					accounts = nil
					if update == "clear" {
						switch tier {
						case "global":
							global = []policies.SpotFundsPnlBoundsBarrier{}
						case "group":
							groups = []policies.SpotFundsPnlBoundsAccountGroupBarrier{}
						case "account":
							accounts = []policies.SpotFundsPnlBoundsAccountBarrier{}
						}
					}
					if update == "duplicate" {
						loose := barrier
						loose.LowerBound = optional.Some(mustPnl(t, "-100"))
						switch tier {
						case "global":
							global = []policies.SpotFundsPnlBoundsBarrier{loose}
						case "group":
							groups = []policies.SpotFundsPnlBoundsAccountGroupBarrier{{AccountGroupID: group, Barrier: loose}}
						case "account":
							accounts = []policies.SpotFundsPnlBoundsAccountBarrier{{AccountID: account, Barrier: loose}}
						}
						if _, err := engine.Configure().SpotFundsPnlBoundsAxis(policies.SpotFundsPolicyName, axis, global, groups, accounts); err != nil {
							t.Fatalf("single-key positive control = %v", err)
						}
						switch tier {
						case "global":
							global[0] = barrier
						case "group":
							groups[0].Barrier = barrier
						case "account":
							accounts[0].Barrier = barrier
						}
						if _, err := engine.Configure().SpotFundsPnlBoundsAxis(policies.SpotFundsPolicyName, axis, global, groups, accounts); err != nil {
							t.Fatalf("restore strict barrier = %v", err)
						}
						switch tier {
						case "global":
							global = []policies.SpotFundsPnlBoundsBarrier{loose, loose}
						case "group":
							groups = []policies.SpotFundsPnlBoundsAccountGroupBarrier{{AccountGroupID: group, Barrier: loose}, {AccountGroupID: group, Barrier: loose}}
						case "account":
							accounts = []policies.SpotFundsPnlBoundsAccountBarrier{{AccountID: account, Barrier: loose}, {AccountID: account, Barrier: loose}}
						}
					}
					_, err := engine.Configure().SpotFundsPnlBoundsAxis(
						policies.SpotFundsPolicyName, axis,
						global, groups, accounts,
					)
					if update == "duplicate" {
						var configErr *configure.Error
						if !errors.As(err, &configErr) || configErr.Kind != configure.ErrorKindValidation {
							t.Fatalf("duplicate axis error = %v, want Validation", err)
						}
					} else if err != nil {
						t.Fatalf("patch retune error = %v", err)
					}
					seedSpotFundsLifecycleAccount(t, engine, account, usd)
					if err := engine.Accounts().RegisterGroup([]param.AccountID{account}, group); err != nil {
						t.Fatalf("RegisterGroup() error = %v", err)
					}
					setSpotFundsPnlAxisMark(t, service, "100")
					applySpotFundsLifecycleFill(t, engine, account)
					setSpotFundsPnlAxisMark(t, service, "90")
					if update == "clear" {
						reservation, rejects, err := engine.ExecutePreTrade(spotFundsLifecycleOrder(t, account))
						if reservation != nil {
							reservation.RollbackAndClose()
						}
						if err != nil {
							t.Fatalf("ExecutePreTrade() error = %v", err)
						}
						if reservation == nil || len(rejects) != 0 {
							t.Fatalf("cleared tier result = (%v, %v), want reservation without rejects", reservation, rejects)
						}
					} else {
						assertSpotFundsPnlPreTradeReject(t, engine, account)
					}
				})
			}
		}
	}
}

func TestSpotFundsPnlBoundsAxisRuntimeInvalid(t *testing.T) {
	engine, service := newSpotFundsPnlAxisEngine(t)
	barrier := policies.SpotFundsPnlBoundsBarrier{
		Mode:       reject.RuleModeReject,
		Currency:   mustAsset(t, "USD"),
		LowerBound: optional.Some(mustPnl(t, "-5")),
	}
	result, err := engine.Configure().SpotFundsPnlBoundsAxis(
		policies.SpotFundsPolicyName, policies.SpotFundsPnlBoundsAxis(4),
		[]policies.SpotFundsPnlBoundsBarrier{barrier}, nil, nil,
	)
	var configErr *configure.Error
	if !errors.As(err, &configErr) || configErr.Kind != configure.ErrorKindValidation {
		t.Fatalf("invalid axis error = %v (%T), want *configure.Error with ErrorKindValidation", err, err)
	}
	if len(result.AccountBlocks) != 0 {
		t.Fatalf("invalid axis AccountBlocks = %v, want none", result.AccountBlocks)
	}
	account := param.NewAccountIDFromUint64(83105)
	seedSpotFundsLifecycleAccount(t, engine, account, barrier.Currency)
	setSpotFundsPnlAxisMark(t, service, "100")
	applySpotFundsLifecycleFill(t, engine, account)
	setSpotFundsPnlAxisMark(t, service, "90")
	reservation, rejects, err := engine.ExecutePreTrade(spotFundsLifecycleOrder(t, account))
	if reservation != nil {
		reservation.RollbackAndClose()
	}
	if err != nil || reservation == nil || len(rejects) != 0 {
		t.Fatalf("after invalid axis result = (%v, %v, %v), want reservation without rejects", reservation, rejects, err)
	}
}

func TestSpotFundsPnlBoundsConfiguratorRoundTrip(t *testing.T) {
	usd := mustAsset(t, "USD")
	account := param.NewAccountIDFromUint64(83002)
	group := mustAccountGroupID(t, 84)

	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFundsPnlBoundsKillSwitch().
			AccountBarriers(policies.SpotFundsPnlBoundsAccountBarrier{
				AccountID: account,
				Barrier: policies.SpotFundsPnlBoundsBarrier{
					Mode:       reject.RuleModeReject,
					Currency:   usd,
					LowerBound: optional.Some(mustPnl(t, "-10")),
					UpperBound: optional.Some(mustPnl(t, "10")),
				},
			}),
		).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()
	globalBarrier := policies.SpotFundsPnlBoundsBarrier{
		Mode:       reject.RuleModeReject,
		Currency:   usd,
		LowerBound: optional.Some(mustPnl(t, "-100")),
	}

	if _, err := engine.Configure().SpotFundsPnlBoundsKillSwitch(
		policies.SpotFundsPolicyName,
		[]policies.SpotFundsPnlBoundsBarrier{globalBarrier},
		[]policies.SpotFundsPnlBoundsAccountGroupBarrier{
			{
				AccountGroupID: group,
				Barrier: policies.SpotFundsPnlBoundsBarrier{
					Mode:       reject.RuleModeReject,
					Currency:   usd,
					UpperBound: optional.Some(mustPnl(t, "100")),
				},
			},
		},
		[]policies.SpotFundsPnlBoundsAccountBarrier{
			{
				AccountID: account,
				Barrier: policies.SpotFundsPnlBoundsBarrier{
					Mode:       reject.RuleModeReject,
					Currency:   usd,
					LowerBound: optional.Some(mustPnl(t, "-20")),
					UpperBound: optional.Some(mustPnl(t, "20")),
				},
			},
		},
	); err != nil {
		t.Fatalf("SpotFundsPnlBoundsKillSwitch error = %v", err)
	}

	configuration, err := engine.Configure().SetSpotFundsAccountPnl(
		policies.SpotFundsPolicyName,
		account,
		model.NewPnlState(mustPnl(t, "2.5")),
	)
	if err != nil {
		t.Fatalf("SetSpotFundsAccountPnl error = %v", err)
	}
	if len(configuration.AccountBlocks) != 0 {
		t.Fatalf("numeric force-set account blocks = %v, want none", configuration.AccountBlocks)
	}

	halted, err := model.NewPnlHaltedState(
		model.PnlHaltReasonMissingFx,
	)
	if err != nil {
		t.Fatalf("NewPnlHaltedState() error = %v", err)
	}
	configuration, err = engine.Configure().SetSpotFundsAccountPnl(
		policies.SpotFundsPolicyName,
		account,
		halted,
	)
	if err != nil {
		t.Fatalf("SetSpotFundsAccountPnl halted error = %v", err)
	}
	if len(configuration.AccountBlocks) != 1 {
		t.Fatalf("halted force-set account blocks = %v, want one", configuration.AccountBlocks)
	}
	if configuration.AccountBlocks[0].Code != reject.CodePnlKillSwitchTriggered {
		t.Fatalf(
			"halted force-set account block code = %v, want %v",
			configuration.AccountBlocks[0].Code,
			reject.CodePnlKillSwitchTriggered,
		)
	}
}

// TestSpotFundsConfiguratorGlobalLimitModeRoundTrip exercises the dlsym
// dispatch path for SpotFundsGlobalLimitMode against a live engine: sets
// TrackOnly then restores Enforce.
func TestSpotFundsConfiguratorGlobalLimitModeRoundTrip(t *testing.T) {
	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFunds()).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	if err := engine.Configure().SpotFundsGlobalLimitMode(
		policies.SpotFundsPolicyName,
		policies.SpotFundsLimitModeTrackOnly,
	); err != nil {
		t.Fatalf("SpotFundsGlobalLimitMode(TrackOnly) error = %v", err)
	}
	if err := engine.Configure().SpotFundsGlobalLimitMode(
		policies.SpotFundsPolicyName,
		policies.SpotFundsLimitModeEnforce,
	); err != nil {
		t.Fatalf("SpotFundsGlobalLimitMode(Enforce) error = %v", err)
	}
}

// TestSpotFundsConfiguratorAccountLimitModeRoundTrip exercises the dlsym
// dispatch path for SpotFundsAccountLimitMode: pins an account to TrackOnly,
// then clears the override.
func TestSpotFundsConfiguratorAccountLimitModeRoundTrip(t *testing.T) {
	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFunds()).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	accountID := param.NewAccountIDFromUint64(77001)

	if err := engine.Configure().SpotFundsAccountLimitMode(
		policies.SpotFundsPolicyName,
		accountID,
		optional.Some(policies.SpotFundsLimitModeTrackOnly),
	); err != nil {
		t.Fatalf("SpotFundsAccountLimitMode(Some(TrackOnly)) error = %v", err)
	}
	if err := engine.Configure().SpotFundsAccountLimitMode(
		policies.SpotFundsPolicyName,
		accountID,
		optional.None[policies.SpotFundsLimitMode](),
	); err != nil {
		t.Fatalf("SpotFundsAccountLimitMode(None) error = %v", err)
	}
}

func TestSpotFundsConfiguratorPositionLimitRoundTrip(t *testing.T) {
	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFunds()).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	accountID := param.NewAccountIDFromUint64(77002)
	aapl := mustAsset(t, "AAPL")
	seedSpotFundsLifecycleAccount(t, engine, accountID, mustAsset(t, "USD"))
	configurator := engine.Configure()

	if err := configurator.SpotFundsPositionLimit(
		policies.SpotFundsPolicyName, accountID, aapl,
		reject.RuleModeReject,
		optional.Some(mustQuantity(t, "0")),
	); err != nil {
		t.Fatalf("SpotFundsPositionLimit(Some(0)) error = %v", err)
	}
	reservation, rejects, err := engine.ExecutePreTrade(
		spotFundsLifecycleOrder(t, accountID),
	)
	if err != nil {
		t.Fatalf("ExecutePreTrade() with limit 0 error = %v", err)
	}
	if reservation != nil {
		reservation.RollbackAndClose()
		t.Fatal("ExecutePreTrade() with limit 0 returned a reservation")
	}
	if len(rejects) != 1 ||
		rejects[0].Code != reject.CodePositionLimitExceeded ||
		rejects[0].Scope != reject.ScopeOrder {
		t.Fatalf("ExecutePreTrade() with limit 0 rejects = %v, want one PositionLimitExceeded at Order scope", rejects)
	}

	if err := configurator.SpotFundsPositionLimit(
		policies.SpotFundsPolicyName, accountID, aapl,
		reject.RuleModeReject,
		optional.Some(mustQuantity(t, "1")),
	); err != nil {
		t.Fatalf("SpotFundsPositionLimit(Some(1)) error = %v", err)
	}
	reservation, rejects, err = engine.ExecutePreTrade(
		spotFundsLifecycleOrder(t, accountID),
	)
	if err != nil {
		t.Fatalf("ExecutePreTrade() with limit 1 error = %v", err)
	}
	if reservation == nil || len(rejects) != 0 {
		t.Fatalf("ExecutePreTrade() with limit 1 = (%v, %v), want reservation", reservation, rejects)
	}
	reservation.RollbackAndClose()

	twoQuantityOrder := spotFundsLifecycleOrder(t, accountID)
	operation := twoQuantityOrder.EnsureOperationView()
	operation.SetTradeAmount(
		param.NewQuantityTradeAmount(mustQuantity(t, "2")),
	)
	reservation, rejects, err = engine.ExecutePreTrade(twoQuantityOrder)
	if err != nil {
		t.Fatalf("ExecutePreTrade() with limit 1 and quantity 2 error = %v", err)
	}
	if reservation != nil {
		reservation.RollbackAndClose()
		t.Fatal("ExecutePreTrade() with limit 1 and quantity 2 returned a reservation")
	}
	if len(rejects) != 1 ||
		rejects[0].Code != reject.CodePositionLimitExceeded ||
		rejects[0].Scope != reject.ScopeOrder {
		t.Fatalf("ExecutePreTrade() with limit 1 and quantity 2 rejects = %v, want one PositionLimitExceeded at Order scope", rejects)
	}

	if err := configurator.SpotFundsPositionLimit(
		policies.SpotFundsPolicyName, accountID, aapl,
		reject.RuleModeReject,
		optional.None[param.Quantity](),
	); err != nil {
		t.Fatalf("SpotFundsPositionLimit(None) error = %v", err)
	}
	reservation, rejects, err = engine.ExecutePreTrade(twoQuantityOrder)
	if err != nil {
		t.Fatalf("ExecutePreTrade() after clear error = %v", err)
	}
	if reservation == nil || len(rejects) != 0 {
		t.Fatalf("ExecutePreTrade() after clear = (%v, %v), want reservation", reservation, rejects)
	}
	reservation.RollbackAndClose()

	if err := configurator.SpotFundsPositionLimit(
		"missing-spot-funds", accountID, aapl,
		reject.RuleModeReject,
		optional.Some(mustQuantity(t, "1")),
	); err == nil {
		t.Fatal("SpotFundsPositionLimit() with unknown policy error = nil")
	}
}

func TestSpotFundsPositionLimitModes(t *testing.T) {
	for _, mode := range []reject.RuleMode{reject.RuleModeReviewWithHold, reject.RuleModeReviewWithoutHold} {
		t.Run(fmt.Sprintf("mode_%d", mode), func(t *testing.T) {
			engine, err := openpit.NewEngineBuilder().NoSync().
				Builtin(policies.BuildSpotFunds()).Build()
			if err != nil {
				t.Fatal(err)
			}
			t.Cleanup(engine.Stop)
			account := param.NewAccountIDFromUint64(77003)
			asset := mustAsset(t, "AAPL")
			seedSpotFundsLifecycleAccount(t, engine, account, mustAsset(t, "USD"))
			configureLimit := func(mode reject.RuleMode, limit optional.Option[param.Quantity]) {
				t.Helper()
				if err := engine.Configure().SpotFundsPositionLimit(
					policies.SpotFundsPolicyName, account, asset, mode, limit,
				); err != nil {
					t.Fatalf("SpotFundsPositionLimit(mode %v) error = %v", mode, err)
				}
			}
			confirmed := false
			checkOrder := func(quantity string, passed bool, reviewCount int) {
				t.Helper()
				order := spotFundsLifecycleOrder(t, account)
				operation := order.EnsureOperationView()
				operation.SetTradeAmount(
					param.NewQuantityTradeAmount(mustQuantity(t, quantity)),
				)
				if confirmed {
					review := order.EnsureReviewView()
					review.SetConfirmed(true)
				}
				reservation, rejects, err := engine.ExecutePreTrade(order)
				if reservation != nil {
					defer reservation.RollbackAndClose()
				}
				if err != nil || (reservation != nil) != passed {
					t.Fatalf("quantity %s: reservation = %v, rejects = %v, error = %v, want passed %v", quantity, reservation, rejects, err, passed)
				}
				if !passed {
					if len(rejects) != 1 || rejects[0].Code != reject.CodePositionLimitExceeded || rejects[0].Mode != reject.RuleModeReject || rejects[0].Scope != reject.ScopeOrder {
						t.Fatalf("rejects = %v, want one Reject PositionLimitExceeded at Order scope", rejects)
					}
					return
				}
				if len(rejects) != 0 {
					t.Fatalf("review breach rejects = %v, want none", rejects)
				}
				reviews, err := reservation.Reviews()
				if err != nil || len(reviews) != reviewCount {
					t.Fatalf("reviews = %v, error = %v, want %d marks", reviews, err, reviewCount)
				}
				if reviewCount > 0 && (reviews[0].Mode != mode || reviews[0].Code != reject.CodePositionLimitExceeded || reviews[0].Scope != reject.ScopeOrder) {
					t.Fatalf("review = %v, want PositionLimitExceeded with mode %v at Order scope", reviews[0], mode)
				}
			}
			configureLimit(reject.RuleModeReject, optional.Some(mustQuantity(t, "2")))
			configureLimit(mode, optional.Some(mustQuantity(t, "0")))
			checkOrder("1", true, 1)
			checkOrder("3", false, 0)
			confirmed = true
			checkOrder("1", true, 0)
			checkOrder("3", false, 0)
			confirmed = false
			configureLimit(0, optional.Some(mustQuantity(t, "1")))
			checkOrder("1", true, 1)
			checkOrder("2", false, 0)
			configureLimit(0, optional.None[param.Quantity]())
			checkOrder("3", true, 1)
			configureLimit(0, optional.Some(mustQuantity(t, "2")))
			for _, invalidMode := range []reject.RuleMode{3, 255} {
				for _, limit := range []optional.Option[param.Quantity]{optional.Some(mustQuantity(t, "0")), optional.None[param.Quantity]()} {
					err := engine.Configure().SpotFundsPositionLimit(
						policies.SpotFundsPolicyName, account, asset, invalidMode, limit,
					)
					var configErr *configure.Error
					if !errors.As(err, &configErr) || configErr.Kind != configure.ErrorKindValidation {
						t.Fatalf("mode %v: error = %v (%T), want Validation", invalidMode, err, err)
					}
					checkOrder("1", true, 1)
					checkOrder("3", false, 0)
					configureLimit(mode, limit)
					configureLimit(mode, optional.Some(mustQuantity(t, "0")))
				}
			}
			configureLimit(mode, optional.None[param.Quantity]())
			checkOrder("1", true, 0)
			checkOrder("3", false, 0)
			configureLimit(mode, optional.Some(mustQuantity(t, "0")))
			configureLimit(reject.RuleModeReject, optional.None[param.Quantity]())
			checkOrder("3", true, 1)
		})
	}
}

func TestSpotFundsReviewPositionOverflowRejectsEveryMode(t *testing.T) {
	for _, mode := range []reject.RuleMode{reject.RuleModeReject, reject.RuleModeReviewWithHold, reject.RuleModeReviewWithoutHold} {
		t.Run(fmt.Sprintf("mode_%d", mode), func(t *testing.T) {
			engine, err := openpit.NewEngineBuilder().NoSync().Builtin(policies.BuildSpotFunds()).Build()
			if err != nil {
				t.Fatal(err)
			}
			t.Cleanup(engine.Stop)
			account := param.NewAccountIDFromUint64(83405)
			asset := mustAsset(t, "AAPL")
			seedSpotFundsLifecycleAccount(t, engine, account, mustAsset(t, "USD"))
			maximum := "79228162514264337593543950335"
			adjustment := spotFundsAbsoluteBalanceAdjustment(t, asset, maximum)
			result, err := engine.ApplyAccountAdjustment(account, []model.AccountAdjustment{adjustment})
			if err != nil || result.BatchError.IsSet() {
				t.Fatalf("seed maximum = %v, error = %v", result, err)
			}
			if err := engine.Configure().SpotFundsPositionLimit(policies.SpotFundsPolicyName, account, asset, mode, optional.Some(mustQuantity(t, maximum))); err != nil {
				t.Fatal(err)
			}
			order := spotFundsLifecycleOrder(t, account)
			report, err := engine.ExecutePreTradeDryRun(order)
			if err != nil {
				t.Fatal(err)
			}
			passed, err := report.IsPass()
			if err != nil || passed {
				t.Fatalf("dry-run overflow pass = %v, error = %v, want false", passed, err)
			}
			dryRejects, err := report.Rejects()
			if err != nil || len(dryRejects) != 1 || dryRejects[0].Code != reject.CodeArithmeticOverflow || dryRejects[0].Mode != reject.RuleModeReject {
				t.Fatalf("dry-run overflow rejects = %v, error = %v, want effective ArithmeticOverflow", dryRejects, err)
			}
			marks, err := report.Reviews()
			if err != nil || len(marks) != 0 {
				t.Fatalf("dry-run overflow marks = %v, error = %v, want none", marks, err)
			}
			report.Close()
			reservation, rejects, err := engine.ExecutePreTrade(order)
			if reservation != nil {
				reservation.RollbackAndClose()
			}
			if err != nil || reservation != nil || len(rejects) != 1 || rejects[0].Code != reject.CodeArithmeticOverflow || rejects[0].Mode != reject.RuleModeReject {
				t.Fatalf("overflow = %v, rejects = %v, error = %v, want effective ArithmeticOverflow", reservation, rejects, err)
			}
		})
	}
}

func TestSpotFundsReviewConfirmedAndDropCopyTrackFunds(t *testing.T) {
	for _, mode := range []policies.SpotFundsLimitMode{policies.SpotFundsLimitModeReviewWithHold, policies.SpotFundsLimitModeReviewWithoutHold} {
		for _, scenario := range []string{"confirmed", "drop_copy"} {
			t.Run(mode.String()+"/"+scenario, func(t *testing.T) {
				engine, err := openpit.NewEngineBuilder().NoSync().Builtin(policies.BuildSpotFunds()).Build()
				if err != nil {
					t.Fatal(err)
				}
				t.Cleanup(engine.Stop)
				account := param.NewAccountIDFromUint64(83406)
				usd := mustAsset(t, "USD")
				seedSpotFundsLifecycleAccount(t, engine, account, usd)
				if err := engine.Configure().SpotFundsGlobalLimitMode(policies.SpotFundsPolicyName, mode); err != nil {
					t.Fatal(err)
				}
				order := spotFundsLifecycleOrder(t, account)
				operation := order.EnsureOperationView()
				operation.SetTradeAmount(param.NewQuantityTradeAmount(mustQuantity(t, "11")))
				var lock pretrade.Lock
				var adjustments []accountadjustment.Outcome
				if scenario == "confirmed" {
					review := order.EnsureReviewView()
					review.SetConfirmed(true)
					reservation, rejects, err := engine.ExecutePreTrade(order)
					if err != nil || reservation == nil || len(rejects) != 0 {
						t.Fatalf("confirmed = %v, rejects = %v, error = %v", reservation, rejects, err)
					}
					marks, err := reservation.Reviews()
					if err != nil || len(marks) != 0 {
						t.Fatalf("confirmed marks = %v, error = %v", marks, err)
					}
					lock, err = reservation.Lock()
					if err != nil {
						t.Fatal(err)
					}
					adjustments, err = reservation.AccountAdjustments()
					if err != nil {
						t.Fatal(err)
					}
					reservation.CommitAndClose()
				} else {
					if err := engine.Configure().SpotFundsPositionLimit(policies.SpotFundsPolicyName, account, mustAsset(t, "AAPL"), reject.RuleModeReject, optional.Some(mustQuantity(t, "0"))); err != nil {
						t.Fatal(err)
					}
					drop, rejects, err := engine.ApplyDropCopy(order)
					if err != nil || drop == nil || len(rejects) != 0 {
						t.Fatalf("drop-copy = %v, rejects = %v, error = %v", drop, rejects, err)
					}
					lock, err = drop.Lock()
					if err != nil {
						t.Fatal(err)
					}
					adjustments, err = drop.AccountAdjustments()
					if err != nil {
						t.Fatal(err)
					}
					drop.CommitAndClose()
				}
				prices, err := lock.Prices()
				if err != nil || len(prices) != 1 || !prices[0].Equal(mustPrice100(t)) {
					t.Fatalf("tracked lock = %v, error = %v", prices, err)
				}
				if len(adjustments) != 2 {
					t.Fatalf("tracked adjustments = %v, want both legs", adjustments)
				}
				seen := map[string]int{}
				for _, outcome := range adjustments {
					entry := outcome.Entry
					if entry.Asset.Equal(usd) {
						balance, hasBalance := entry.Balance.Get()
						held, hasHeld := entry.Held.Get()
						if !hasBalance || !hasHeld || !balance.Delta.Equal(mustPositionSize(t, "-1100")) || !held.Delta.Equal(mustPositionSize(t, "1100")) {
							t.Fatalf("settlement adjustment = %v, want -1100 available, +1100 held", entry)
						}
						seen["USD"]++
					} else if entry.Asset.Equal(mustAsset(t, "AAPL")) {
						incoming, ok := entry.Incoming.Get()
						if !ok || !incoming.Delta.Equal(mustPositionSize(t, "11")) {
							t.Fatalf("underlying adjustment = %v, want +11 incoming", entry)
						}
						seen["AAPL"]++
					}
				}
				if seen["USD"] != 1 || seen["AAPL"] != 1 {
					t.Fatalf("adjustment membership = %v, want one of each leg", seen)
				}
				assertSpotFundsAmounts(t, spotFundsAmounts(t, engine, account, usd), "-100", "1100", "0")
				assertSpotFundsAmounts(t, spotFundsAmounts(t, engine, account, mustAsset(t, "AAPL")), "0", "0", "11")
				if scenario == "drop_copy" {
					if err := engine.Configure().SpotFundsPositionLimit(policies.SpotFundsPolicyName, account, mustAsset(t, "AAPL"), reject.RuleModeReject, optional.None[param.Quantity]()); err != nil {
						t.Fatal(err)
					}
				}
				probe, rejects, err := engine.ExecutePreTrade(spotFundsLifecycleOrder(t, account))
				if err != nil || probe == nil || len(rejects) != 0 {
					t.Fatalf("later order = %v, rejects = %v, error = %v", probe, rejects, err)
				}
				defer probe.RollbackAndClose()
				marks, err := probe.Reviews()
				if err != nil || len(marks) != 1 || marks[0].Code != reject.CodeInsufficientFunds {
					t.Fatalf("later marks = %v, error = %v, want InsufficientFunds", marks, err)
				}
			})
		}
	}
}

func TestSpotFundsReviewLimitModeCascade(t *testing.T) {
	engine, err := openpit.NewEngineBuilder().NoSync().Builtin(policies.BuildSpotFunds()).Build()
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(engine.Stop)
	account := param.NewAccountIDFromUint64(83407)
	group := mustAccountGroupID(t, 88)
	usd := mustAsset(t, "USD")
	seedSpotFundsLifecycleAccount(t, engine, account, usd)
	if err := engine.Accounts().RegisterGroup([]param.AccountID{account}, group); err != nil {
		t.Fatal(err)
	}
	if err := engine.Configure().SpotFundsGlobalLimitMode(policies.SpotFundsPolicyName, policies.SpotFundsLimitModeReviewWithHold); err != nil {
		t.Fatal(err)
	}
	if err := engine.Configure().SpotFundsAccountGroupLimitMode(policies.SpotFundsPolicyName, group, optional.Some(policies.SpotFundsLimitModeReviewWithoutHold)); err != nil {
		t.Fatal(err)
	}
	order := spotFundsLifecycleOrder(t, account)
	operation := order.EnsureOperationView()
	operation.SetTradeAmount(param.NewQuantityTradeAmount(mustQuantity(t, "11")))
	if err := engine.Configure().SpotFundsAccountLimitMode(policies.SpotFundsPolicyName, account, optional.Some(policies.SpotFundsLimitModeReviewWithHold)); err != nil {
		t.Fatal(err)
	}
	accountReservation, accountRejects, err := engine.ExecutePreTrade(order)
	if err != nil || accountReservation == nil || len(accountRejects) != 0 {
		t.Fatalf("account review = %v, rejects = %v, error = %v", accountReservation, accountRejects, err)
	}
	accountMarks, err := accountReservation.Reviews()
	if err != nil || len(accountMarks) != 1 || accountMarks[0].Mode != reject.RuleModeReviewWithHold || accountMarks[0].Code != reject.CodeInsufficientFunds {
		t.Fatalf("account tier marks = %v, error = %v", accountMarks, err)
	}
	assertSpotFundsAmounts(t, spotFundsAmounts(t, engine, account, usd), "-100", "1100", "0")
	accountReservation.RollbackAndClose()
	if err := engine.Configure().SpotFundsAccountLimitMode(policies.SpotFundsPolicyName, account, optional.Some(policies.SpotFundsLimitModeEnforce)); err != nil {
		t.Fatal(err)
	}
	reservation, rejects, err := engine.ExecutePreTrade(order)
	if reservation != nil {
		reservation.RollbackAndClose()
	}
	if err != nil || reservation != nil || len(rejects) != 1 || rejects[0].Code != reject.CodeInsufficientFunds || rejects[0].Mode != reject.RuleModeReject {
		t.Fatalf("account Enforce = %v, rejects = %v, error = %v", reservation, rejects, err)
	}
	if err := engine.Configure().SpotFundsAccountLimitMode(policies.SpotFundsPolicyName, account, optional.None[policies.SpotFundsLimitMode]()); err != nil {
		t.Fatal(err)
	}
	for _, mode := range []reject.RuleMode{reject.RuleModeReviewWithoutHold, reject.RuleModeReviewWithHold} {
		reservation, rejects, err := engine.ExecutePreTrade(order)
		if err != nil || reservation == nil || len(rejects) != 0 {
			t.Fatalf("cascade mode %v = %v, rejects = %v, error = %v", mode, reservation, rejects, err)
		}
		marks, err := reservation.Reviews()
		if err != nil || len(marks) != 1 || marks[0].Mode != mode || marks[0].Code != reject.CodeInsufficientFunds {
			t.Fatalf("cascade marks = %v, error = %v, want mode %v", marks, err, mode)
		}
		if mode == reject.RuleModeReviewWithHold {
			assertSpotFundsAmounts(t, spotFundsAmounts(t, engine, account, usd), "-100", "1100", "0")
		} else {
			assertSpotFundsAmounts(t, spotFundsAmounts(t, engine, account, usd), "1000", "0", "0")
		}
		reservation.RollbackAndClose()
		assertSpotFundsAmounts(t, spotFundsAmounts(t, engine, account, usd), "1000", "0", "0")
		if err := engine.Configure().SpotFundsAccountGroupLimitMode(policies.SpotFundsPolicyName, group, optional.None[policies.SpotFundsLimitMode]()); err != nil {
			t.Fatal(err)
		}
	}
}

// TestSpotFundsConfiguratorAccountGroupLimitModeRoundTrip exercises the dlsym
// dispatch path for SpotFundsAccountGroupLimitMode: pins a group to TrackOnly,
// then clears the override.
func TestSpotFundsConfiguratorAccountGroupLimitModeRoundTrip(t *testing.T) {
	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFunds()).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	groupID := mustAccountGroupID(t, 42)

	if err := engine.Configure().SpotFundsAccountGroupLimitMode(
		policies.SpotFundsPolicyName,
		groupID,
		optional.Some(policies.SpotFundsLimitModeTrackOnly),
	); err != nil {
		t.Fatalf("SpotFundsAccountGroupLimitMode(Some(TrackOnly)) error = %v", err)
	}
	if err := engine.Configure().SpotFundsAccountGroupLimitMode(
		policies.SpotFundsPolicyName,
		groupID,
		optional.None[policies.SpotFundsLimitMode](),
	); err != nil {
		t.Fatalf("SpotFundsAccountGroupLimitMode(None) error = %v", err)
	}
}

// TestSpotFundsFullEngineWithLocalMDServiceIsRejected verifies that a
// Full-sync engine builder rejects a no-sync market-data service
// with a descriptive mismatch error.
func TestSpotFundsFullEngineWithLocalMDServiceIsRejected(t *testing.T) {
	// Build a no-sync MD service: derive from a NoSync engine builder and do NOT
	// call FullSync on the MD builder.
	noSyncEB := openpit.NewEngineBuilder().NoSync()
	localService, err := noSyncEB.MarketData(marketdata.InfiniteTTL()).Build()
	if err != nil {
		t.Fatalf("marketdata Build() error = %v", err)
	}
	defer localService.Close()

	// A Full-sync engine must reject the no-sync MD service.
	_, buildErr := openpit.NewEngineBuilder().FullSync().
		Builtin(policies.BuildSpotFunds().WithMarketOrders(localService, 100)).
		Build()
	if buildErr == nil {
		t.Fatal("expected Build() to fail for Full engine + no-sync MD service, but it succeeded")
	}
}

// TestSpotFundsLocalEngineWithFullMDServiceIsAccepted verifies that a
// no-sync engine builder accepts a Full-sync market-data service.
func TestSpotFundsLocalEngineWithFullMDServiceIsAccepted(t *testing.T) {
	// Build a Full MD service: derive from a NoSync engine builder then upgrade
	// the MD builder to FullSync before building.
	noSyncEB := openpit.NewEngineBuilder().NoSync()
	fullService, err := noSyncEB.MarketData(marketdata.InfiniteTTL()).FullSync().Build()
	if err != nil {
		t.Fatalf("marketdata Build() error = %v", err)
	}
	defer fullService.Close()

	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFunds().WithMarketOrders(fullService, 100)).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	engine.Stop()
}
