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
	"testing"
	"time"

	"go.openpit.dev/openpit/marketdata"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/optional"
	"go.openpit.dev/openpit/pretrade/policies"
	"go.openpit.dev/openpit/reject"
)

// noGroupInfo is a minimal AccountInfo whose reading account has no group.
type noGroupInfo struct{}

func (noGroupInfo) AccountGroup() optional.Option[param.AccountGroupID] {
	return optional.None[param.AccountGroupID]()
}

// Mirrors public Go examples from:
// - ../pit.wiki/Market-Data.md
// - ../pit.wiki/Market-Data-TTL.md
// - ../pit.wiki/Market-Data-Pricing.md
// If this file changes, update every linked documentation snippet.

// Used in: pit.wiki/Market-Data.md - Pushing and Reading Quotes
func TestExampleWikiMarketDataPushAndRead(t *testing.T) {
	service, err := NewEngineBuilder().
		FullSync().
		MarketData(marketdata.InfiniteTTL()).
		Build()
	if err != nil {
		t.Fatalf("marketdata Build() error = %v", err)
	}
	defer service.Close()

	aapl, err := param.NewAsset("AAPL")
	if err != nil {
		t.Fatalf("NewAsset(AAPL) error = %v", err)
	}
	usd, err := param.NewAsset("USD")
	if err != nil {
		t.Fatalf("NewAsset(USD) error = %v", err)
	}
	instrument := param.NewInstrument(aapl, usd)

	aaplID, err := service.Register(instrument)
	if err != nil {
		t.Fatalf("Register() error = %v", err)
	}

	// Publish a full snapshot into the default ("everyone-else") bucket.
	mark, _ := param.NewPriceFromString("150")
	bid, _ := param.NewPriceFromString("149.5")
	ask, _ := param.NewPriceFromString("150.5")
	if err := service.Push(
		aaplID,
		marketdata.NewQuote().WithMark(mark).WithBid(bid).WithAsk(ask),
	); err != nil {
		t.Fatalf("Push() error = %v", err)
	}

	// Read for an account with no group: the lookup falls through to the
	// default bucket. Pass any marketdata.AccountInfo; in policy code this is
	// usually the pretrade.Context. The test mirror uses a no-group stub.
	accountID := param.NewAccountIDFromUint64(1)
	quote, ok := service.GetOptional(
		aaplID,
		accountID,
		noGroupInfo{},
		marketdata.QuoteResolutionAccountThenGroupThenDefault,
	).Get()
	if !ok {
		t.Fatal("GetOptional() ok = false, want true")
	}
	if got, _ := quote.Mark().Get(); !got.Equal(mark) {
		t.Fatalf("quote.Mark() = %v, want %v", got, mark)
	}
	if got, _ := quote.Bid().Get(); !got.Equal(bid) {
		t.Fatalf("quote.Bid() = %v, want %v", got, bid)
	}

	// Resolve recovers the id from the instrument name.
	resolved, ok := service.Resolve(instrument)
	if !ok || resolved.String() != aaplID.String() {
		t.Fatalf("Resolve() = (%v, %v), want (%v, true)", resolved, ok, aaplID)
	}
}

// Used in: pit.wiki/Market-Data.md - Replace Versus Patch
func TestExampleWikiMarketDataReplaceVersusPatch(t *testing.T) {
	service, err := NewEngineBuilder().
		FullSync().
		MarketData(marketdata.InfiniteTTL()).
		Build()
	if err != nil {
		t.Fatalf("marketdata Build() error = %v", err)
	}
	defer service.Close()

	aapl, err := param.NewAsset("AAPL")
	if err != nil {
		t.Fatalf("NewAsset(AAPL) error = %v", err)
	}
	usd, err := param.NewAsset("USD")
	if err != nil {
		t.Fatalf("NewAsset(USD) error = %v", err)
	}
	aaplID, err := service.Register(param.NewInstrument(aapl, usd))
	if err != nil {
		t.Fatalf("Register() error = %v", err)
	}

	mark, _ := param.NewPriceFromString("100")
	bid, _ := param.NewPriceFromString("99")
	ask, _ := param.NewPriceFromString("101")
	if err := service.Push(
		aaplID,
		marketdata.NewQuote().WithMark(mark).WithBid(bid).WithAsk(ask),
	); err != nil {
		t.Fatalf("Push() error = %v", err)
	}

	// Patch only the mark; bid and ask are preserved.
	newMark, _ := param.NewPriceFromString("105")
	if err := service.PushPatch(
		aaplID,
		marketdata.NewQuote().WithMark(newMark),
	); err != nil {
		t.Fatalf("PushPatch() error = %v", err)
	}

	accountID := param.NewAccountIDFromUint64(1)
	quote, ok := service.GetOptional(
		aaplID,
		accountID,
		noGroupInfo{},
		marketdata.QuoteResolutionAccountThenGroupThenDefault,
	).Get()
	if !ok {
		t.Fatal("GetOptional() ok = false, want true")
	}
	if got, _ := quote.Mark().Get(); !got.Equal(newMark) {
		t.Fatalf("quote.Mark() = %v, want %v", got, newMark)
	}
	if got, _ := quote.Bid().Get(); !got.Equal(bid) {
		t.Fatalf("quote.Bid() = %v, want %v", got, bid)
	}
	if got, _ := quote.Ask().Get(); !got.Equal(ask) {
		t.Fatalf("quote.Ask() = %v, want %v", got, ask)
	}
}

// Used in: pit.wiki/Market-Data-TTL.md - Quote Freshness
func TestExampleWikiMarketDataFiniteTTLHidesStaleQuote(t *testing.T) {
	// A 50 ms service-wide lifetime: quotes older than that read as absent.
	service, err := NewEngineBuilder().
		FullSync().
		MarketData(marketdata.WithinTTL(50 * time.Millisecond)).
		Build()
	if err != nil {
		t.Fatalf("marketdata Build() error = %v", err)
	}
	defer service.Close()

	aapl, _ := param.NewAsset("AAPL")
	usd, _ := param.NewAsset("USD")
	aaplID, err := service.Register(param.NewInstrument(aapl, usd))
	if err != nil {
		t.Fatalf("Register() error = %v", err)
	}

	mark, _ := param.NewPriceFromString("200")
	if err := service.Push(aaplID, marketdata.NewQuote().WithMark(mark)); err != nil {
		t.Fatalf("Push() error = %v", err)
	}

	accountID := param.NewAccountIDFromUint64(1)
	if _, ok := service.GetOptional(
		aaplID, accountID, noGroupInfo{},
		marketdata.QuoteResolutionAccountThenGroupThenDefault,
	).Get(); !ok {
		t.Fatal("GetOptional() ok = false right after push, want true")
	}

	// After the lifetime elapses the quote reads as absent.
	time.Sleep(80 * time.Millisecond)
	if _, ok := service.GetOptional(
		aaplID, accountID, noGroupInfo{},
		marketdata.QuoteResolutionAccountThenGroupThenDefault,
	).Get(); ok {
		t.Fatal("GetOptional() ok = true after TTL, want false")
	}

	// A fresh push restores visibility.
	fresh, _ := param.NewPriceFromString("205")
	if err := service.Push(aaplID, marketdata.NewQuote().WithMark(fresh)); err != nil {
		t.Fatalf("Push() error = %v", err)
	}
	quote, ok := service.GetOptional(
		aaplID, accountID, noGroupInfo{},
		marketdata.QuoteResolutionAccountThenGroupThenDefault,
	).Get()
	if !ok {
		t.Fatal("GetOptional() ok = false after fresh push, want true")
	}
	if got, _ := quote.Mark().Get(); !got.Equal(fresh) {
		t.Fatalf("quote.Mark() = %v, want %v", got, fresh)
	}
}

// Used in: pit.wiki/Market-Data.md - Clearing a Quote
func TestExampleWikiMarketDataClearThenRecover(t *testing.T) {
	service, err := NewEngineBuilder().
		FullSync().
		MarketData(marketdata.InfiniteTTL()).
		Build()
	if err != nil {
		t.Fatalf("marketdata Build() error = %v", err)
	}
	defer service.Close()

	aapl, _ := param.NewAsset("AAPL")
	usd, _ := param.NewAsset("USD")
	aaplID, err := service.Register(param.NewInstrument(aapl, usd))
	if err != nil {
		t.Fatalf("Register() error = %v", err)
	}

	mark, _ := param.NewPriceFromString("200")
	if err := service.Push(aaplID, marketdata.NewQuote().WithMark(mark)); err != nil {
		t.Fatalf("Push() error = %v", err)
	}

	// Clear hides the quote but keeps the instrument registered.
	service.Clear(aaplID)
	accountID := param.NewAccountIDFromUint64(1)
	if _, ok := service.GetOptional(
		aaplID, accountID, noGroupInfo{},
		marketdata.QuoteResolutionAccountThenGroupThenDefault,
	).Get(); ok {
		t.Fatal("GetOptional() ok = true after Clear, want false")
	}

	// Pushing again restores a quote for the same id.
	recovered, _ := param.NewPriceFromString("210")
	if err := service.Push(aaplID, marketdata.NewQuote().WithMark(recovered)); err != nil {
		t.Fatalf("Push() error = %v", err)
	}
	quote, ok := service.GetOptional(
		aaplID, accountID, noGroupInfo{},
		marketdata.QuoteResolutionAccountThenGroupThenDefault,
	).Get()
	if !ok {
		t.Fatal("GetOptional() ok = false after recovery push, want true")
	}
	if got, _ := quote.Mark().Get(); !got.Equal(recovered) {
		t.Fatalf("quote.Mark() = %v, want %v", got, recovered)
	}
}

// Used in: pit.wiki/Market-Data-Pricing.md - Pricing Market Orders
func TestExampleWikiMarketDataMarketOrdersBookTopOverride(t *testing.T) {
	// Obtain the market-data builder from the engine builder so the sync mode
	// is derived automatically.
	eb := NewEngineBuilder().FullSync()
	// A shared market-data service feeds the policy's market-order pricing.
	marketData, err := eb.MarketData(marketdata.InfiniteTTL()).Build()
	if err != nil {
		t.Fatalf("marketdata Build() error = %v", err)
	}
	defer marketData.Close()

	aapl, _ := param.NewAsset("AAPL")
	usd, _ := param.NewAsset("USD")
	instrument := param.NewInstrument(aapl, usd)

	aaplID, err := marketData.Register(instrument)
	if err != nil {
		t.Fatalf("Register() error = %v", err)
	}
	mark, _ := param.NewPriceFromString("200")
	bid, _ := param.NewPriceFromString("199.5")
	ask, _ := param.NewPriceFromString("200.5")
	if err := marketData.Push(
		aaplID,
		marketdata.NewQuote().WithMark(mark).WithBid(bid).WithAsk(ask),
	); err != nil {
		t.Fatalf("Push() error = %v", err)
	}

	// Price from the top of book; AAPL overrides the global 100 bps slippage to
	// zero, so a buy is priced exactly at the ask. An instrument-level override
	// leaves the account and group ids unset.
	engine, err := eb.
		Builtin(
			policies.BuildSpotFunds().
				WithMarketOrders(marketData, 100).
				PricingSource(policies.SpotFundsPricingSourceBookTop).
				Overrides(policies.SpotFundsOverrideEntry{
					Target:   policies.SpotFundsOverrideTargetInstrument{Instrument: aaplID},
					Override: policies.SpotFundsOverride{SlippageBps: optional.Some(uint16(0))},
				}),
		).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()

	accountID := param.NewAccountIDFromUint64(99224416)
	seed := marketDataSeedBalance(t, "1000")
	if result, err := engine.ApplyAccountAdjustment(
		accountID,
		[]model.AccountAdjustment{seed},
	); err != nil {
		t.Fatalf("ApplyAccountAdjustment() error = %v", err)
	} else if result.BatchError.IsSet() {
		t.Fatalf("ApplyAccountAdjustment() rejects = %v, want none", result.BatchError)
	}

	// Market buy (no price): priced at the ask 200.5 because the override pins
	// slippage to zero. The seeded balance covers it, so it passes.
	reservation, execRejects, err := engine.ExecutePreTrade(marketDataBuyOrder(t, instrument, accountID, "1"))
	if err != nil {
		t.Fatalf("ExecutePreTrade() error = %v", err)
	}
	if execRejects != nil {
		t.Fatalf("ExecutePreTrade() unexpected rejects: %v", execRejects)
	}
	reservation.CommitAndClose()

	// A full replace that carries only the mark drops bid and ask. With the
	// BookTop source there is no ask to price a buy, so it is rejected.
	replaced, _ := param.NewPriceFromString("215")
	if err := marketData.Push(aaplID, marketdata.NewQuote().WithMark(replaced)); err != nil {
		t.Fatalf("Push() error = %v", err)
	}
	// (No Get call after this push - the engine reads quotes internally.)
	_, execRejects, err = engine.ExecutePreTrade(marketDataBuyOrder(t, instrument, accountID, "1"))
	if err != nil {
		t.Fatalf("ExecutePreTrade() error = %v", err)
	}
	if len(execRejects) == 0 {
		t.Fatal("ExecutePreTrade() rejects = none, want MarkPriceUnavailable")
	}
	if execRejects[0].Code != reject.CodeMarkPriceUnavailable {
		t.Fatalf("reject code = %v, want %v", execRejects[0].Code, reject.CodeMarkPriceUnavailable)
	}
}

// Used in: pit.wiki/Market-Data.md - Targeted Fan-Out: push for
func TestExampleWikiMarketDataPushForFanOut(t *testing.T) {
	service, err := NewEngineBuilder().
		FullSync().
		MarketData(marketdata.InfiniteTTL()).
		Build()
	if err != nil {
		t.Fatalf("marketdata Build() error = %v", err)
	}
	defer service.Close()

	aapl, _ := param.NewAsset("AAPL")
	usd, _ := param.NewAsset("USD")
	aaplID, err := service.Register(param.NewInstrument(aapl, usd))
	if err != nil {
		t.Fatalf("Register() error = %v", err)
	}
	mark, _ := param.NewPriceFromString("150")

	groupID, _ := param.NewAccountGroupIDFromUint32(7)

	// Fan out to two accounts and one group simultaneously.
	if err := service.PushFor(
		aaplID,
		marketdata.NewQuote().WithMark(mark),
		[]param.AccountID{
			param.NewAccountIDFromUint64(10),
			param.NewAccountIDFromUint64(11),
		},
		[]param.AccountGroupID{groupID},
	); err != nil {
		t.Fatalf("PushFor() error = %v", err)
	}

	// Read back for account 10 under AccountOnly - hits the per-account bucket.
	quote, ok := service.GetOptional(
		aaplID,
		param.NewAccountIDFromUint64(10),
		noGroupInfo{},
		marketdata.QuoteResolutionAccountOnly,
	).Get()
	if !ok {
		t.Fatal("GetOptional(account 10) ok = false, want true")
	}
	if got, _ := quote.Mark().Get(); !got.Equal(mark) {
		t.Fatalf("account 10 quote.Mark() = %v, want %v", got, mark)
	}
}

// marketDataSeedBalance builds an absolute USD balance seed adjustment.
func marketDataSeedBalance(t *testing.T, amount string) model.AccountAdjustment {
	t.Helper()

	usd, err := param.NewAsset("USD")
	if err != nil {
		t.Fatalf("NewAsset(USD) error = %v", err)
	}
	total, err := param.NewPositionSizeFromString(amount)
	if err != nil {
		t.Fatalf("NewPositionSizeFromString(%q) error = %v", amount, err)
	}

	adj, err := model.NewAccountAdjustmentFromValues(model.AccountAdjustmentValues{
		BalanceOperation: optional.Some(
			model.NewAccountAdjustmentBalanceOperationFromValues(
				model.AccountAdjustmentBalanceOperationValues{
					Asset: optional.Some(usd),
				},
			),
		),
		Amount: optional.Some(
			model.NewAccountAdjustmentAmountFromValues(model.AccountAdjustmentAmountValues{
				Balance: optional.Some(param.NewAbsoluteAdjustmentAmount(total)),
			}),
		),
	})
	if err != nil {
		t.Fatalf("NewAccountAdjustmentFromValues() error = %v", err)
	}
	return adj
}

// marketDataBuyOrder builds a market buy order (no limit price).
func marketDataBuyOrder(
	t *testing.T,
	instrument param.Instrument,
	accountID param.AccountID,
	quantity string,
) model.Order {
	t.Helper()

	order := model.NewOrder()
	op := order.EnsureOperationView()
	op.SetInstrument(instrument)
	op.SetAccountID(accountID)
	op.SetSide(param.SideBuy)
	qty, err := param.NewQuantityFromString(quantity)
	if err != nil {
		t.Fatalf("NewQuantityFromString(%q) error = %v", quantity, err)
	}
	op.SetTradeAmount(param.NewQuantityTradeAmount(qty))
	return order
}
