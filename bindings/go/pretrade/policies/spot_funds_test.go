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
					param.NewAbsoluteAdjustmentAmount(mustPositionSize(t, "1000")),
				),
			}),
		),
	})
	if err != nil {
		t.Fatalf("NewAccountAdjustmentFromValues() error = %v", err)
	}
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
	lock := reservation.Lock()
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
	fill.SetLeavesQuantity(mustQuantity(t, "0"))
	fill.SetLock(lock.Bytes())
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
	fill.SetLeavesQuantity(mustQuantity(t, "0"))
	fill.SetLock(lock.Bytes())
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

func TestSpotFundsLimitModeNativeRoundTrip(t *testing.T) {
	tests := []struct {
		name string
		mode policies.SpotFundsLimitMode
		want native.PretradePoliciesSpotFundsLimitMode
	}{
		{"enforce", policies.SpotFundsLimitModeEnforce, native.PretradePoliciesSpotFundsLimitModeEnforce},
		{"track-only", policies.SpotFundsLimitModeTrackOnly, native.PretradePoliciesSpotFundsLimitModeTrackOnly},
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

	invalidMode := policies.SpotFundsLimitMode(99)

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
	const wantMessage = "spot funds limit_mode must be 0 (Enforce) or 1 (TrackOnly), got 99"
	if configErr.Message != wantMessage {
		t.Fatalf("%s message = %q, want %q", label, configErr.Message, wantMessage)
	}
}

func TestSpotFundsLimitModeString(t *testing.T) {
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
	if len(result.AccountPnls) != 1 {
		t.Fatalf("AccountPnls = %v, want one outcome", result.AccountPnls)
	}
	outcome := result.AccountPnls[0]
	if outcome.AccountID != account {
		t.Fatalf("AccountPnls[0].AccountID = %v, want %v", outcome.AccountID, account)
	}
	pnl, ok := outcome.Amount()
	if !ok {
		haltReason, _ := outcome.HaltReason()
		t.Fatalf("AccountPnls[0] halt reason = %v, want none", haltReason)
	}
	if !pnl.Delta.Equal(mustPnl(t, "0")) {
		t.Fatalf(
			"AccountPnls[0].Amount() PnL delta = %v, want 0",
			pnl.Delta,
		)
	}
	if !pnl.Absolute.Equal(mustPnl(t, "0")) {
		t.Fatalf(
			"AccountPnls[0].Amount() PnL absolute = %v, want 0",
			pnl.Absolute,
		)
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
	account := param.NewAccountIDFromUint64(83017)
	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFunds()).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	report := directSpotFundsFillReport(t, account)
	// The fee has to be denominated in the account currency, so it is what
	// makes this fill's account line uncomputable without one. A fee-less
	// opening fill would contribute a computable zero instead.
	feeReport := directSpotFundsFillReport(t, account)
	feeFill := feeReport.EnsureFillView()
	feeFill.SetFee(param.NewMonetaryAmount(mustFee(t, "1"), usd))
	first, err := engine.ApplyExecutionReport(feeReport)
	if err != nil {
		t.Fatalf("first ApplyExecutionReport() error = %v", err)
	}
	if len(first.AccountPnls) != 1 {
		t.Fatalf("first AccountPnls len = %d, want 1", len(first.AccountPnls))
	}
	reason, ok := first.AccountPnls[0].HaltReason()
	if !ok || reason != model.PnlHaltReasonMissingAccountCurrency {
		t.Fatalf("first HaltReason() = (%v, %v)", reason, ok)
	}
	second, err := engine.ApplyExecutionReport(report)
	if err != nil {
		t.Fatalf("second ApplyExecutionReport() error = %v", err)
	}
	if len(second.AccountPnls) != 0 {
		t.Fatalf("second AccountPnls = %v, want omitted", second.AccountPnls)
	}

	if err := engine.Accounts().SetCurrency(account, usd); err != nil {
		t.Fatalf("Accounts().SetCurrency() error = %v", err)
	}
	aapl := mustAsset(t, "AAPL")
	forceSpotFundsPositionPnl(t, engine, account, aapl)
	third, err := engine.ApplyExecutionReport(report)
	if err != nil {
		t.Fatalf("third ApplyExecutionReport() error = %v", err)
	}
	if len(third.AccountPnls) != 0 {
		t.Fatalf("third AccountPnls = %v, want still omitted", third.AccountPnls)
	}
	foundAapl := false
	for _, outcome := range third.AccountAdjustments {
		if !outcome.Entry.Asset.Equal(aapl) {
			continue
		}
		foundAapl = true
		if _, ok := outcome.Entry.RealizedPnl.Get(); ok {
			t.Fatal("third position PnL is emitted for a zero-realized fill")
		}
		if _, ok := outcome.Entry.AverageEntryPrice.Get(); !ok {
			t.Fatal("third average entry price is absent while position tracking is active")
		}
		break
	}
	if !foundAapl {
		t.Fatal("third AAPL account-adjustment outcome is absent")
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
	fourth, err := engine.ApplyExecutionReport(report)
	if err != nil {
		t.Fatalf("fourth ApplyExecutionReport() error = %v", err)
	}
	if len(fourth.AccountPnls) != 1 {
		t.Fatalf("fourth AccountPnls len = %d, want 1", len(fourth.AccountPnls))
	}
	if _, ok := fourth.AccountPnls[0].Amount(); !ok {
		t.Fatal("fourth account PnL is halted after exact force-set")
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
			GlobalBarrier(policies.SpotFundsPnlBoundsBarrier{
				LowerBound: optional.Some(mustPnl(t, "-100")),
			}).
			AccountGroupBarriers(policies.SpotFundsPnlBoundsAccountGroupBarrier{
				AccountGroupID: group,
				Barrier: policies.SpotFundsPnlBoundsBarrier{
					UpperBound: optional.Some(mustPnl(t, "250")),
				},
			}).
			AccountBarriers(policies.SpotFundsPnlBoundsAccountBarrier{
				AccountID: account,
				Barrier: policies.SpotFundsPnlBoundsBarrier{
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

func TestSpotFundsPnlBoundsKillSwitchBuilderRequiresBarrier(t *testing.T) {
	_, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFundsPnlBoundsKillSwitch()).
		Build()
	if err == nil {
		t.Fatal("Build() error = nil, want at-least-one-barrier validation error")
	}
}

func TestSpotFundsPnlBoundsRuntimeAxisReplacementAndClear(t *testing.T) {
	usd := mustAsset(t, "USD")
	group := mustAccountGroupID(t, 85)
	accountSpecific := param.NewAccountIDFromUint64(83011)
	accountGroup := param.NewAccountIDFromUint64(83012)
	accountGlobal := param.NewAccountIDFromUint64(83013)
	accountAfterClear := param.NewAccountIDFromUint64(83014)

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
	} {
		seedSpotFundsLifecycleAccount(t, engine, account, usd)
	}
	if err := engine.Accounts().RegisterGroup([]param.AccountID{accountGroup}, group); err != nil {
		t.Fatalf("Accounts().RegisterGroup() error = %v", err)
	}
	globalBarrier := policies.SpotFundsPnlBoundsBarrier{
		LowerBound: optional.Some(mustPnl(t, "-20")),
	}

	for _, account := range []struct {
		id  param.AccountID
		pnl string
	}{
		{accountSpecific, "-15"},
		{accountGroup, "-15"},
		{accountGlobal, "-25"},
		{accountAfterClear, "-25"},
	} {
		if _, err := engine.Configure().SetSpotFundsAccountPnl(
			policies.SpotFundsPolicyName,
			account.id,
			model.NewPnlState(mustPnl(t, account.pnl)),
		); err != nil {
			t.Fatalf("SetSpotFundsAccountPnl() error = %v", err)
		}
	}

	if err := engine.Configure().SpotFundsPnlBoundsKillSwitch(
		policies.SpotFundsPolicyName,
		optional.Some(&globalBarrier),
		[]policies.SpotFundsPnlBoundsAccountGroupBarrier{{
			AccountGroupID: group,
			Barrier: policies.SpotFundsPnlBoundsBarrier{
				LowerBound: optional.Some(mustPnl(t, "-10")),
			},
		}},
		[]policies.SpotFundsPnlBoundsAccountBarrier{{
			AccountID: accountSpecific,
			Barrier: policies.SpotFundsPnlBoundsBarrier{
				LowerBound: optional.Some(mustPnl(t, "-10")),
			},
		}},
	); err != nil {
		t.Fatalf("SpotFundsPnlBoundsKillSwitch() setup error = %v", err)
	}

	// A non-nil empty account axis clears only per-account barriers. The
	// omitted global and group axes must keep affecting their respective keys.
	if err := engine.Configure().SpotFundsPnlBoundsKillSwitch(
		policies.SpotFundsPolicyName,
		optional.None[*policies.SpotFundsPnlBoundsBarrier](),
		nil,
		[]policies.SpotFundsPnlBoundsAccountBarrier{},
	); err != nil {
		t.Fatalf("SpotFundsPnlBoundsKillSwitch() account clear error = %v", err)
	}

	if result := applySpotFundsLifecycleFill(t, engine, accountSpecific); len(result.AccountBlocks) != 0 {
		t.Fatalf("specific account AccountBlocks = %v, want none after clear", result.AccountBlocks)
	}
	assertSpotFundsPnlPreTradeReject(t, engine, accountGroup)
	assertSpotFundsPnlPreTradeReject(t, engine, accountGlobal)

	// Runtime updates may clear every axis. Unlike the explicit batch builder,
	// this is a patch operation and has no at-least-one-barrier requirement.
	if err := engine.Configure().SpotFundsPnlBoundsKillSwitch(
		policies.SpotFundsPolicyName,
		optional.Some[*policies.SpotFundsPnlBoundsBarrier](nil),
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
		LowerBound: optional.Some(mustPnl(t, "-30")),
	}
	if err := engine.Configure().SpotFundsPnlBoundsKillSwitch(
		policies.SpotFundsPolicyName,
		optional.Some(&globalBarrier),
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
	if err := engine.Configure().SpotFundsPnlBoundsKillSwitch(
		policies.SpotFundsPolicyName,
		optional.Some(&globalBarrier),
		nil,
		nil,
	); err != nil {
		t.Fatalf("SpotFundsPnlBoundsKillSwitch() replace global error = %v", err)
	}
	assertSpotFundsPnlPreTradeReject(t, engine, account)
}

func TestSpotFundsPnlBoundsConfiguratorRoundTrip(t *testing.T) {
	account := param.NewAccountIDFromUint64(83002)
	group := mustAccountGroupID(t, 84)

	engine, err := openpit.NewEngineBuilder().NoSync().
		Builtin(policies.BuildSpotFundsPnlBoundsKillSwitch().
			AccountBarriers(policies.SpotFundsPnlBoundsAccountBarrier{
				AccountID: account,
				Barrier: policies.SpotFundsPnlBoundsBarrier{
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
		LowerBound: optional.Some(mustPnl(t, "-100")),
	}

	if err := engine.Configure().SpotFundsPnlBoundsKillSwitch(
		policies.SpotFundsPolicyName,
		optional.Some(&globalBarrier),
		[]policies.SpotFundsPnlBoundsAccountGroupBarrier{
			{
				AccountGroupID: group,
				Barrier: policies.SpotFundsPnlBoundsBarrier{
					UpperBound: optional.Some(mustPnl(t, "100")),
				},
			},
		},
		[]policies.SpotFundsPnlBoundsAccountBarrier{
			{
				AccountID: account,
				Barrier: policies.SpotFundsPnlBoundsBarrier{
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
