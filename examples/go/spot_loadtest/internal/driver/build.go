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

package driver

import (
	"context"
	"fmt"
	"os"
	"time"

	openpit "go.openpit.dev/openpit"
	"go.openpit.dev/openpit/asyncengine"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/optional"
	"go.openpit.dev/openpit/pretrade"
	"go.openpit.dev/openpit/pretrade/policies"

	"openpit-loadtest-spot-funds-go/internal/generator"
)

// dispatchStrategy mirrors config.AsyncEngineStrategy in the driver package so
// the driver layer does not import config (dependency direction: driver -> config
// is already present via FromAppConfig; this keeps the build graph clean).
type dispatchStrategy int

const (
	dispatchDynamic dispatchStrategy = iota
	dispatchSharded
)

// engineDispatch carries the asyncengine builder knobs (resource limits, not
// synchronization semantics) into buildEngine.
type engineDispatch struct {
	// Strategy selects dynamic (default) or sharded dispatch.
	Strategy dispatchStrategy

	// --- Dynamic-only ---

	// MaxQueues is the Dynamic dispatch capacity (0 = unlimited). Sized to hold
	// the offered active working set with margin.
	MaxQueues int
	// IdleCleanup is the per-account queue retire delay. 0 disables cleanup.
	IdleCleanup time.Duration

	// --- Sharded-only ---

	// ShardedWorkers is the number of fixed shards (> 0, required for sharded).
	ShardedWorkers int

	// --- Both strategies ---

	// QueueCapacity is the buffered channel size (0 = engine default 1024).
	QueueCapacity int
	// SlowSubmitThreshold is the slow-submit observer threshold (0 = engine default 1m).
	SlowSubmitThreshold time.Duration
}

// buildEngine constructs the AccountSync engine with the built-in spot funds
// policy in limit-only mode (v1 drives spot LIMIT orders only, so no market-data
// service and no MarkPriceProvider are needed), then wraps it in an
// asyncengine.AsyncEngine with the configured dispatch strategy. All dispatch
// knobs are RESOURCE limits (like a connection cap), NOT sync semantics: the
// measured semantics stay per-account AccountSync regardless of strategy.
//
// When observer is non-nil it is installed as the dispatcher's diagnostic hook.
// The underlying *openpit.Engine is returned alongside the async facade so the
// caller can apply initial balance seeds SYNCHRONOUSLY (seeding is setup, not
// measured load) before the async run. The returned stop function releases the
// async dispatcher gracefully and then the underlying engine; callers invoke it
// once after draining all futures.
func buildEngine(observer asyncengine.Observer, dispatch engineDispatch) (*asyncengine.AsyncEngine, *openpit.Engine, func(context.Context), error) {
	engine, err := openpit.NewEngineBuilder().AccountSync().
		Builtin(policies.BuildSpotFunds()). // limit-only: market orders rejected
		Build()
	if err != nil {
		return nil, nil, nil, fmt.Errorf("driver: build engine: %w", err)
	}

	builder := asyncengine.NewBuilder(engine)
	if observer != nil {
		builder = builder.WithObserver(observer)
	}
	// Apply shared knobs (0 lets the builder use its own default for each).
	builder = builder.WithQueueCapacity(dispatch.QueueCapacity)
	builder = builder.WithSlowSubmitThreshold(dispatch.SlowSubmitThreshold)

	var ae *asyncengine.AsyncEngine
	switch dispatch.Strategy {
	case dispatchSharded:
		ae, err = builder.Sharded(dispatch.ShardedWorkers).Build()
	default: // dispatchDynamic
		// Dynamic dispatch sized to the offered concurrency (resource knob): hold
		// the active working set with margin (MaxQueues) and retire idle queues so
		// live queues track the active set, not the touched population.
		ae, err = builder.Dynamic().
			MaxQueues(dispatch.MaxQueues).
			IdleCleanupAfter(dispatch.IdleCleanup).
			Build()
	}
	if err != nil {
		engine.Stop()
		return nil, nil, nil, fmt.Errorf("driver: build async engine: %w", err)
	}

	// AsyncEngine.StopGraceful does not own the engine lifecycle here (we did not
	// wire WithStopUnderlying), so the caller must Stop the engine after the
	// dispatcher has drained.
	stop := func(ctx context.Context) {
		if err := ae.StopGraceful(ctx); err != nil {
			fmt.Fprintf(os.Stderr, "spot_loadtest: async engine graceful stop: %v\n", err)
		}
		engine.Stop()
	}
	return ae, engine, stop, nil
}

// --- event -> engine object mapping (contract section 3) ---

// buildOrder maps an OrderCheck event to a limit, quantity-denominated
// model.Order. Decimals cross into engine value types via their string forms so
// the construction is precision-exact (contract section 3).
func buildOrder(ev *generator.Event) (model.Order, param.AccountID, error) {
	acc, err := param.NewAccountIDFromString(ev.Account)
	if err != nil {
		return model.Order{}, param.AccountID{}, fmt.Errorf("account %q: %w", ev.Account, err)
	}
	inst, err := instrumentOf(ev.Underlying, ev.Settlement)
	if err != nil {
		return model.Order{}, param.AccountID{}, err
	}
	side, err := sideOf(ev.Side)
	if err != nil {
		return model.Order{}, param.AccountID{}, err
	}
	qty, err := param.NewQuantityFromString(ev.Quantity.String())
	if err != nil {
		return model.Order{}, param.AccountID{}, fmt.Errorf("quantity %q: %w", ev.Quantity.String(), err)
	}
	price, err := param.NewPriceFromString(ev.Price.String())
	if err != nil {
		return model.Order{}, param.AccountID{}, fmt.Errorf("price %q: %w", ev.Price.String(), err)
	}

	order := model.NewOrder()
	op := order.EnsureOperationView()
	op.SetInstrument(inst)
	op.SetAccountID(acc)
	op.SetSide(side)
	op.SetTradeAmount(param.NewQuantityTradeAmount(qty))
	op.SetPrice(price) // limit price (always set in v1)
	return order, acc, nil
}

// buildReport maps a Settlement event to a full-fill (leaves = 0, is_final =
// true) model.ExecutionReport. The fill's Lock ties it back to the reservation
// the order committed: a single entry under the spot funds default policy group
// at the SAME price the order reserved at (contract section 3).
func buildReport(ev *generator.Event) (model.ExecutionReport, param.AccountID, error) {
	acc, err := param.NewAccountIDFromString(ev.Account)
	if err != nil {
		return model.ExecutionReport{}, param.AccountID{}, fmt.Errorf("account %q: %w", ev.Account, err)
	}
	inst, err := instrumentOf(ev.Underlying, ev.Settlement)
	if err != nil {
		return model.ExecutionReport{}, param.AccountID{}, err
	}
	side, err := sideOf(ev.Side)
	if err != nil {
		return model.ExecutionReport{}, param.AccountID{}, err
	}
	qty, err := param.NewQuantityFromString(ev.Quantity.String())
	if err != nil {
		return model.ExecutionReport{}, param.AccountID{}, fmt.Errorf("quantity %q: %w", ev.Quantity.String(), err)
	}
	price, err := param.NewPriceFromString(ev.Price.String())
	if err != nil {
		return model.ExecutionReport{}, param.AccountID{}, fmt.Errorf("price %q: %w", ev.Price.String(), err)
	}
	leaves, err := param.NewQuantityFromString("0")
	if err != nil {
		return model.ExecutionReport{}, param.AccountID{}, err
	}
	fee, err := param.NewFeeFromString("0")
	if err != nil {
		return model.ExecutionReport{}, param.AccountID{}, err
	}
	pnl, err := param.NewPnlFromString("0")
	if err != nil {
		return model.ExecutionReport{}, param.AccountID{}, err
	}
	lock, err := pretrade.NewLockFromEntries([]pretrade.Entry{
		{PolicyGroupID: model.DefaultPolicyGroupID, Price: price},
	})
	if err != nil {
		return model.ExecutionReport{}, param.AccountID{}, fmt.Errorf("build fill lock: %w", err)
	}
	report := model.NewExecutionReportFromValues(model.ExecutionReportValues{
		Operation: optional.Some(model.NewExecutionReportOperationFromValues(
			model.ExecutionReportOperationValues{
				Instrument: optional.Some(inst),
				AccountID:  optional.Some(acc),
				Side:       optional.Some(side),
			})),
		FinancialImpact: optional.Some(model.NewExecutionReportFinancialImpactFromValues(
			model.ExecutionReportFinancialImpactValues{
				Pnl: optional.Some(pnl),
				Fee: optional.Some(fee),
			})),
		Fill: optional.Some(model.NewExecutionReportFillFromValues(
			model.ExecutionReportFillValues{
				LastTrade:      optional.Some(model.NewExecutionReportTrade(price, qty)),
				LeavesQuantity: optional.Some(leaves),
				Lock:           lock.Bytes(),
				IsFinal:        optional.BoolSome(true),
			})),
	})
	return report, acc, nil
}

// buildAdjustment maps a Funding event to a balance-operation
// model.AccountAdjustment on the funded asset's available leg (held is never
// touched), Absolute or Delta per the event's kind (contract section 2.4).
func buildAdjustment(ev *generator.Event) (model.AccountAdjustment, param.AccountID, error) {
	acc, err := param.NewAccountIDFromString(ev.Account)
	if err != nil {
		return model.AccountAdjustment{}, param.AccountID{}, fmt.Errorf("account %q: %w", ev.Account, err)
	}
	asset, err := param.NewAsset(ev.FundingAsset)
	if err != nil {
		return model.AccountAdjustment{}, param.AccountID{}, fmt.Errorf("asset %q: %w", ev.FundingAsset, err)
	}
	amount, err := param.NewPositionSizeFromString(ev.FundingAmount.String())
	if err != nil {
		return model.AccountAdjustment{}, param.AccountID{}, fmt.Errorf("amount %q: %w", ev.FundingAmount.String(), err)
	}
	var balance param.AdjustmentAmount
	if ev.FundingIsDelta() {
		balance = param.NewDeltaAdjustmentAmount(amount)
	} else {
		balance = param.NewAbsoluteAdjustmentAmount(amount)
	}
	adj, err := model.NewAccountAdjustmentFromValues(model.AccountAdjustmentValues{
		BalanceOperation: optional.Some(model.NewAccountAdjustmentBalanceOperationFromValues(
			model.AccountAdjustmentBalanceOperationValues{Asset: optional.Some(asset)})),
		Amount: optional.Some(model.NewAccountAdjustmentAmountFromValues(
			model.AccountAdjustmentAmountValues{Balance: optional.Some(balance)})),
	})
	if err != nil {
		return model.AccountAdjustment{}, param.AccountID{}, err
	}
	return adj, acc, nil
}

func instrumentOf(underlying, settlement string) (param.Instrument, error) {
	u, err := param.NewAsset(underlying)
	if err != nil {
		return param.Instrument{}, fmt.Errorf("underlying %q: %w", underlying, err)
	}
	s, err := param.NewAsset(settlement)
	if err != nil {
		return param.Instrument{}, fmt.Errorf("settlement %q: %w", settlement, err)
	}
	return param.NewInstrument(u, s), nil
}

func sideOf(s generator.Side) (param.Side, error) {
	switch s {
	case generator.SideBuy:
		return param.SideBuy, nil
	case generator.SideSell:
		return param.SideSell, nil
	default:
		return 0, fmt.Errorf("unknown side %v", s)
	}
}
