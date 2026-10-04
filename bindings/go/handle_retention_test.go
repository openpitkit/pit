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
	"bytes"
	"fmt"
	"runtime"
	"strings"
	"sync"
	"testing"
	"time"

	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/optional"
	"go.openpit.dev/openpit/pretrade"
	"go.openpit.dev/openpit/pretrade/policies"
	"go.openpit.dev/openpit/reject"
)

type handleRetentionFinalizerSentinel struct {
	value [16]byte
}

func TestHandleRetentionPolicyBuilders(t *testing.T) {
	t.Run("rate limit", func(t *testing.T) {
		builder := newEphemeralRateLimitBuilder(t)
		forceHandleRetentionFinalizers(t)
		replacements := overwriteHandleRetentionAssets(t, 3)

		engine, err := NewEngineBuilder().NoSync().Builtin(builder).Build()
		if err != nil {
			t.Fatalf("Build() error = %v", err)
		}
		defer engine.Stop()

		assertHandleRetentionRateLimit(
			t,
			engine,
			orderSizeTestOrder(t, 1, "USD", "1"),
		)
		assertHandleRetentionRateLimit(
			t,
			engine,
			orderSizeTestOrder(t, 2, "EUR", "1"),
		)
		runtime.KeepAlive(replacements)
	})

	t.Run("order size limit", func(t *testing.T) {
		builder := newEphemeralOrderSizeLimitBuilder(t)
		forceHandleRetentionFinalizers(t)
		replacements := overwriteHandleRetentionAssets(t, 3)

		engine, err := NewEngineBuilder().NoSync().Builtin(builder).Build()
		if err != nil {
			t.Fatalf("Build() error = %v", err)
		}
		defer engine.Stop()

		_, rejects, err := engine.StartPreTrade(
			orderSizeTestOrder(t, 1, "USD", "2"),
		)
		if err != nil {
			t.Fatalf("StartPreTrade() error = %v", err)
		}
		if len(rejects) != 1 ||
			rejects[0].Code != reject.CodeOrderNotionalExceedsLimit {
			t.Fatalf("StartPreTrade() rejects = %v, want notional limit", rejects)
		}

		_, rejects, err = engine.StartPreTrade(
			orderSizeTestOrder(t, 2, "EUR", "2"),
		)
		if err != nil {
			t.Fatalf("account StartPreTrade() error = %v", err)
		}
		if len(rejects) != 1 ||
			rejects[0].Code != reject.CodeOrderNotionalExceedsLimit {
			t.Fatalf("account rejects = %v, want notional limit", rejects)
		}
		runtime.KeepAlive(replacements)
	})

	t.Run("pnl bounds kill switch", func(t *testing.T) {
		builder := newEphemeralPnlBoundsKillSwitchBuilder(t)
		forceHandleRetentionFinalizers(t)
		replacements := overwriteHandleRetentionAssets(t, 3)

		engine, err := NewEngineBuilder().NoSync().Builtin(builder).Build()
		if err != nil {
			t.Fatalf("Build() error = %v", err)
		}
		defer engine.Stop()

		request, rejects, err := engine.StartPreTrade(pnlTestOrder(t, 1))
		if err != nil {
			t.Fatalf("StartPreTrade() error = %v", err)
		}
		if len(rejects) != 0 {
			t.Fatalf("StartPreTrade() rejects = %v, want none", rejects)
		}
		request.Close()

		result, err := engine.ApplyExecutionReport(
			pnlTestReport(t, 1, "AAPL", "USD", "-600"),
		)
		if err != nil {
			t.Fatalf("ApplyExecutionReport() error = %v", err)
		}
		if len(result.AccountBlocks) == 0 ||
			result.AccountBlocks[0].Code != reject.CodePnlKillSwitchTriggered {
			t.Fatalf("AccountBlocks = %v, want kill switch block", result.AccountBlocks)
		}

		request, rejects, err = engine.StartPreTrade(
			orderSizeTestOrder(t, 2, "EUR", "1"),
		)
		if err != nil {
			t.Fatalf("account StartPreTrade() error = %v", err)
		}
		if len(rejects) != 0 {
			t.Fatalf("account StartPreTrade() rejects = %v, want none", rejects)
		}
		request.Close()

		result, err = engine.ApplyExecutionReport(
			pnlTestReport(t, 2, "MSFT", "EUR", "-600"),
		)
		if err != nil {
			t.Fatalf("account ApplyExecutionReport() error = %v", err)
		}
		if len(result.AccountBlocks) == 0 ||
			result.AccountBlocks[0].Code != reject.CodePnlKillSwitchTriggered {
			t.Fatalf("account blocks = %v, want kill switch block", result.AccountBlocks)
		}
		runtime.KeepAlive(replacements)
	})
}

func TestHandleRetentionModelChildren(t *testing.T) {
	t.Run("execution report fill lock", func(t *testing.T) {
		price, err := param.NewPriceFromString("124")
		if err != nil {
			t.Fatalf("NewPriceFromString() error = %v", err)
		}
		lock, err := pretrade.NewLockFromEntries([]pretrade.Entry{
			{PolicyGroupID: model.DefaultPolicyGroupID, Price: price},
		})
		if err != nil {
			t.Fatalf("NewLockFromEntries() error = %v", err)
		}
		want := lock.Bytes()
		fill := func() model.ExecutionReportFill {
			fill := model.NewExecutionReportFill()
			fill.SetLock(want)
			report := model.NewExecutionReport()
			report.SetFill(fill)
			value, ok := report.Fill().Get()
			if !ok {
				t.Fatal("Fill().IsSet() = false, want true")
			}
			return value
		}()

		forceHandleRetentionFinalizers(t)
		overwriteHandleRetentionLocks(t)
		if got := fill.Lock(); !bytes.Equal(got, want) {
			t.Fatalf("Fill().Lock() after report GC = %v, want %v", got, want)
		}
	})

	t.Run("execution report clone from borrowed fill lock", func(t *testing.T) {
		price, err := param.NewPriceFromString("124")
		if err != nil {
			t.Fatalf("NewPriceFromString() error = %v", err)
		}
		lock, err := pretrade.NewLockFromEntries([]pretrade.Entry{
			{PolicyGroupID: model.DefaultPolicyGroupID, Price: price},
		})
		if err != nil {
			t.Fatalf("NewLockFromEntries() error = %v", err)
		}
		want := lock.Bytes()
		clone := func() model.ExecutionReport {
			fill := model.NewExecutionReportFill()
			fill.SetLock(want)
			src := model.NewExecutionReport()
			src.SetFill(fill)
			rebuilt := model.NewExecutionReportFromHandle(src.Handle())
			clone := model.NewExecutionReportFromValues(rebuilt.Values())
			runtime.KeepAlive(src)
			return clone
		}()

		forceHandleRetentionFinalizers(t)
		overwriteHandleRetentionLocks(t)
		fill, ok := clone.Fill().Get()
		if !ok {
			t.Fatal("clone Fill().IsSet() = false, want true")
		}
		if got := fill.Lock(); !bytes.Equal(got, want) {
			t.Fatalf("clone Fill().Lock() after source GC = %v, want %v", got, want)
		}
	})

	t.Run("order operation", func(t *testing.T) {
		operation := newEphemeralOrderOperation(t)
		forceHandleRetentionFinalizers(t)
		replacements := overwriteHandleRetentionAssets(t, 3, 4)

		assertHandleRetentionInstrument(t, operation.Instrument(), "AAPL", "USD")
		assertHandleRetentionOrderOperation(t, operation)
		runtime.KeepAlive(replacements)
	})

	t.Run("order margin", func(t *testing.T) {
		margin := newEphemeralOrderMargin(t)
		forceHandleRetentionFinalizers(t)
		replacements := overwriteHandleRetentionAssets(t, 3)

		assertHandleRetentionAsset(t, margin.CollateralAsset(), "EUR")
		runtime.KeepAlive(replacements)
	})

	t.Run("account adjustment balance operation", func(t *testing.T) {
		operation := newEphemeralBalanceOperation(t)
		forceHandleRetentionFinalizers(t)
		replacements := overwriteHandleRetentionAssets(t, 3)

		assertHandleRetentionAsset(t, operation.Asset(), "CHF")
		runtime.KeepAlive(replacements)
	})

	t.Run("account adjustment position operation", func(t *testing.T) {
		operation := newEphemeralPositionOperation(t)
		forceHandleRetentionFinalizers(t)
		replacements := overwriteHandleRetentionAssets(t, 3, 4)

		assertHandleRetentionInstrument(t, operation.Instrument(), "NVDA", "CAD")
		assertHandleRetentionAsset(t, operation.CollateralAsset(), "AUD")
		runtime.KeepAlive(replacements)
	})
}

func TestHandleRetentionAccessorLastReceiver(t *testing.T) {
	const (
		readers     = 64
		assetLength = 64 * 1024
	)

	tests := []struct {
		name string
		read func(param.Asset, func()) optional.Option[param.Asset]
	}{
		{
			name: "value_receiver",
			read: func(asset param.Asset, wait func()) optional.Option[param.Asset] {
				margin := model.NewOrderMargin()
				margin.SetCollateralAsset(asset)
				wait()
				return margin.CollateralAsset()
			},
		},
		{
			name: "view",
			read: func(asset param.Asset, wait func()) optional.Option[param.Asset] {
				order := model.NewOrder()
				view := order.EnsureMarginView()
				view.SetCollateralAsset(asset)
				wait()
				return view.CollateralAsset()
			},
		},
		{
			name: "parent_to_child",
			read: func(asset param.Asset, wait func()) optional.Option[param.Asset] {
				operation := model.NewExecutionReportOperation()
				operation.SetInstrument(param.NewInstrument(asset, asset))
				report := model.NewExecutionReport()
				report.SetOperation(operation)
				wait()
				child, ok := report.Operation().Get()
				if !ok {
					return optional.None[param.Asset]()
				}
				instrument, ok := child.Instrument().Get()
				if !ok {
					return optional.None[param.Asset]()
				}
				return optional.Some(instrument.UnderlyingAsset)
			},
		},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			want := strings.Repeat("A", assetLength)
			overwrite := strings.Repeat("Z", assetLength)
			start := make(chan struct{})
			results := make(chan string, readers)
			var ready sync.WaitGroup
			ready.Add(readers)
			wait := func() {
				ready.Done()
				<-start
			}
			for range readers {
				asset := builtinTestAsset(t, want)
				go func(asset param.Asset) {
					value, ok := test.read(asset, wait).Get()
					if !ok {
						results <- ""
						return
					}
					got := strings.Clone(value.Handle())
					runtime.KeepAlive(value)
					results <- got
				}(asset)
			}

			ready.Wait()
			close(start)
			replacementsDone := make(chan []*native.String, 1)
			go func() {
				replacements := make([]*native.String, 1024)
				for i := range replacements {
					replacements[i] = native.NewString(overwrite)
				}
				replacementsDone <- replacements
			}()
			forceHandleRetentionFinalizers(t)
			replacements := <-replacementsDone
			for range readers {
				if got := <-results; got != want {
					prefix := got
					if len(prefix) > 64 {
						prefix = prefix[:64]
					}
					t.Fatalf(
						"accessor prefix = %q, length = %d, want A bytes",
						prefix,
						len(got),
					)
				}
			}
			runtime.KeepAlive(replacements)
		})
	}
}

func TestHandleRetentionAssetCopyLastReceiver(t *testing.T) {
	const (
		readers     = 64
		assetLength = 64 * 1024
	)

	want := strings.Repeat("A", assetLength)
	overwrite := strings.Repeat("Z", assetLength)
	wantHash := builtinTestAsset(t, want).Hash()
	tests := []struct {
		name  string
		check func(param.Asset) string
	}{
		{
			name: "Safe",
			check: func(asset param.Asset) string {
				if got := asset.Safe(); got != want {
					prefix := got
					if len(prefix) > 64 {
						prefix = prefix[:64]
					}
					return fmt.Sprintf(
						"Safe() prefix = %q, length = %d, want A bytes",
						prefix,
						len(got),
					)
				}
				return ""
			},
		},
		{
			name: "Equal",
			check: func(asset param.Asset) string {
				argument, err := param.NewAsset(want)
				if err != nil {
					return fmt.Sprintf("NewAsset() error = %v", err)
				}
				if !asset.Equal(argument) {
					return "Equal() = false, want true"
				}
				return ""
			},
		},
		{
			name: "Hash",
			check: func(asset param.Asset) string {
				if got := asset.Hash(); got != wantHash {
					return fmt.Sprintf("Hash() = %d, want %d", got, wantHash)
				}
				return ""
			},
		},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			start := make(chan struct{})
			results := make(chan string, readers)
			var ready sync.WaitGroup
			ready.Add(readers)
			for range readers {
				asset := builtinTestAsset(t, want)
				go func(asset param.Asset) {
					ready.Done()
					<-start
					results <- test.check(asset)
				}(asset)
			}

			ready.Wait()
			close(start)
			replacementsDone := make(chan []*native.String, 1)
			go func() {
				replacements := make([]*native.String, 1024)
				for i := range replacements {
					replacements[i] = native.NewString(overwrite)
				}
				replacementsDone <- replacements
			}()
			forceHandleRetentionFinalizers(t)
			replacements := <-replacementsDone
			for range readers {
				if mismatch := <-results; mismatch != "" {
					t.Fatal(mismatch)
				}
			}
			runtime.KeepAlive(replacements)
		})
	}
}

func newEphemeralRateLimitBuilder(t *testing.T) *policies.RateLimitReadyBuilder {
	t.Helper()
	return policies.BuildRateLimit().AssetBarriers(policies.RateLimitAssetBarrier{
		Limit: policies.RateLimit{
			MaxOrders: 1,
			Window:    60 * time.Second,
		},
		SettlementAsset: builtinTestAsset(t, "USD"),
	}).AccountAssetBarriers(policies.RateLimitAccountAssetBarrier{
		Limit: policies.RateLimit{
			MaxOrders: 1,
			Window:    60 * time.Second,
		},
		AccountID:       param.NewAccountIDFromUint64(2),
		SettlementAsset: builtinTestAsset(t, "EUR"),
	})
}

func newEphemeralOrderSizeLimitBuilder(
	t *testing.T,
) *policies.OrderSizeLimitReadyBuilder {
	t.Helper()
	return policies.BuildOrderSizeLimit().
		BrokerBarrier(hugeOrderSizeLimit(t)).
		AssetBarriers(policies.OrderSizeAssetBarrier{
			Asset: builtinTestAsset(t, "USD"),
			Limit: policies.OrderSizeLimit{
				MaxNotional: optional.Some(orderSizeTestVol(t, "100")),
			},
		}).
		AccountAssetBarriers(policies.OrderSizeAccountAssetBarrier{
			AccountID: param.NewAccountIDFromUint64(2),
			Asset:     builtinTestAsset(t, "EUR"),
			Limit: policies.OrderSizeLimit{
				MaxNotional: optional.Some(orderSizeTestVol(t, "100")),
			},
		})
}

func newEphemeralPnlBoundsKillSwitchBuilder(
	t *testing.T,
) *policies.PnlBoundsKillSwitchReadyBuilder {
	t.Helper()
	return policies.BuildPnlBoundsKillSwitch().
		BrokerBarriers(policies.PnlBoundsBrokerBarrier{
			SettlementAsset: builtinTestAsset(t, "USD"),
			LowerBound:      optional.Some(pnlTestPnl(t, "-500")),
		}).
		AccountBarriers(policies.PnlBoundsAccountAssetBarrier{
			Barrier: policies.PnlBoundsBrokerBarrier{
				SettlementAsset: builtinTestAsset(t, "EUR"),
				LowerBound:      optional.Some(pnlTestPnl(t, "-500")),
			},
			AccountID:  param.NewAccountIDFromUint64(2),
			InitialPnl: pnlTestPnl(t, "0"),
		})
}

func assertHandleRetentionRateLimit(t *testing.T, engine *Engine, order model.Order) {
	t.Helper()
	request, rejects, err := engine.StartPreTrade(order)
	if err != nil {
		t.Fatalf("first StartPreTrade() error = %v", err)
	}
	if len(rejects) != 0 {
		t.Fatalf("first StartPreTrade() rejects = %v, want none", rejects)
	}
	request.Close()

	_, rejects, err = engine.StartPreTrade(order)
	if err != nil {
		t.Fatalf("second StartPreTrade() error = %v", err)
	}
	if len(rejects) != 1 || rejects[0].Code != reject.CodeRateLimitExceeded {
		t.Fatalf("second StartPreTrade() rejects = %v, want rate limit", rejects)
	}
}

func newEphemeralOrderOperation(t *testing.T) model.OrderOperation {
	t.Helper()
	parent := rateLimitTestOrder(t, 1)
	operation, ok := parent.Operation().Get()
	if !ok {
		t.Fatal("Operation() is unset")
	}
	return operation
}

func newEphemeralOrderMargin(t *testing.T) model.OrderMargin {
	t.Helper()
	margin := model.NewOrderMargin()
	margin.SetCollateralAsset(builtinTestAsset(t, "EUR"))
	parent := model.NewOrder()
	parent.SetMargin(margin)
	child, ok := parent.Margin().Get()
	if !ok {
		t.Fatal("Margin() is unset")
	}
	return child
}

func newEphemeralBalanceOperation(
	t *testing.T,
) model.AccountAdjustmentBalanceOperation {
	t.Helper()
	operation := model.NewAccountAdjustmentBalanceOperation()
	operation.SetAsset(builtinTestAsset(t, "CHF"))
	parent := model.NewAccountAdjustment()
	parent.SetBalanceOperationAndUnsetOtherOperations(operation)
	child, ok := parent.BalanceOperation().Get()
	if !ok {
		t.Fatal("BalanceOperation() is unset")
	}
	return child
}

func newEphemeralPositionOperation(
	t *testing.T,
) model.AccountAdjustmentPositionOperation {
	t.Helper()
	operation := model.NewAccountAdjustmentPositionOperation()
	operation.SetInstrument(param.NewInstrument(
		builtinTestAsset(t, "NVDA"),
		builtinTestAsset(t, "CAD"),
	))
	operation.SetCollateralAsset(builtinTestAsset(t, "AUD"))
	parent := model.NewAccountAdjustment()
	parent.SetPositionOperationAndUnsetOtherOperations(operation)
	child, ok := parent.PositionOperation().Get()
	if !ok {
		t.Fatal("PositionOperation() is unset")
	}
	return child
}

func assertHandleRetentionOrderOperation(
	t *testing.T,
	operation model.OrderOperation,
) {
	t.Helper()
	usd := builtinTestAsset(t, "USD")
	engine, err := NewEngineBuilder().NoSync().
		Builtin(policies.BuildRateLimit().
			AssetBarriers(policies.RateLimitAssetBarrier{
				Limit: policies.RateLimit{
					MaxOrders: 1,
					Window:    60 * time.Second,
				},
				SettlementAsset: usd,
			}),
		).Build()
	runtime.KeepAlive(usd)
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	order := model.NewOrder()
	order.SetOperation(operation)
	request, rejects, err := engine.StartPreTrade(order)
	if err != nil {
		t.Fatalf("first StartPreTrade() error = %v", err)
	}
	if len(rejects) != 0 {
		t.Fatalf("first StartPreTrade() rejects = %v, want none", rejects)
	}
	request.Close()

	_, rejects, err = engine.StartPreTrade(order)
	if err != nil {
		t.Fatalf("second StartPreTrade() error = %v", err)
	}
	if len(rejects) != 1 || rejects[0].Code != reject.CodeRateLimitExceeded {
		t.Fatalf("second StartPreTrade() rejects = %v, want rate limit", rejects)
	}
}

func assertHandleRetentionInstrument(
	t *testing.T,
	value optional.Option[param.Instrument],
	wantUnderlying string,
	wantSettlement string,
) {
	t.Helper()
	instrument, ok := value.Get()
	if !ok {
		t.Fatal("Instrument() is unset")
	}
	if got := instrument.UnderlyingAsset.Handle(); got != wantUnderlying {
		t.Fatalf("underlying asset = %q, want %q", got, wantUnderlying)
	}
	if got := instrument.SettlementAsset.Handle(); got != wantSettlement {
		t.Fatalf("settlement asset = %q, want %q", got, wantSettlement)
	}
}

func assertHandleRetentionAsset(
	t *testing.T,
	value optional.Option[param.Asset],
	want string,
) {
	t.Helper()
	asset, ok := value.Get()
	if !ok {
		t.Fatal("asset is unset")
	}
	if got := asset.Handle(); got != want {
		t.Fatalf("asset = %q, want %q", got, want)
	}
}

func forceHandleRetentionFinalizers(t *testing.T) {
	t.Helper()
	deadline := time.NewTimer(10 * time.Second)
	defer deadline.Stop()

	// Finalizers for objects unreachable before this call have run on return.
	// GC completes sweeping, but runFinalizers can take a LIFO batch mid-sweep.
	// Each sentinel is allocated only after the previous one's finalizer runs.
	// By the second sentinel, all pre-call finalizers are queued; the third
	// must be in a later batch, so the second's entire batch has finished.
	for i := 0; i < 3; i++ {
		finalized := make(chan struct{})
		sentinel := &handleRetentionFinalizerSentinel{value: [16]byte{1}}
		runtime.SetFinalizer(sentinel, func(*handleRetentionFinalizerSentinel) {
			close(finalized)
		})

	waitFinalizer:
		for {
			runtime.GC()
			select {
			case <-finalized:
				break waitFinalizer
			case <-deadline.C:
				t.Fatal("timed out waiting for finalizers")
			default:
				runtime.Gosched()
			}
		}
	}
	runtime.GC()
}

func overwriteHandleRetentionAssets(t *testing.T, lengths ...int) []param.Asset {
	t.Helper()
	const allocationsPerLength = 1024
	replacements := make([]param.Asset, 0, len(lengths)*allocationsPerLength)
	for _, length := range lengths {
		value := strings.Repeat("Z", length)
		for range allocationsPerLength {
			replacements = append(replacements, builtinTestAsset(t, value))
		}
	}
	return replacements
}

func overwriteHandleRetentionLocks(t *testing.T) {
	t.Helper()
	for range 32 {
		lock := native.CreatePretradePreTradeLock()
		t.Cleanup(func() { native.DestroyPretradePreTradeLock(lock) })
	}
}
