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

package policies

import (
	"runtime"

	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/marketdata"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/optional"
)

// SpotFundsPricingSource selects the base price the spot funds policy uses to
// size market-order reservations.
type SpotFundsPricingSource uint8

const (
	// SpotFundsPricingSourceMark uses the mark price.
	SpotFundsPricingSourceMark SpotFundsPricingSource = 0
	// SpotFundsPricingSourceBookTop uses the top-of-book price.
	SpotFundsPricingSourceBookTop SpotFundsPricingSource = 1
)

// SpotFundsOverride overrides the market slippage for a registered instrument,
// optionally narrowed to a single account or an account group. AccountID and
// AccountGroupID are mutually exclusive; setting both makes Build return an
// error. When SlippageBps is None the entry is ignored (defers to the next
// cascade tier). Resolution order: (instrument, account_id) ->
// (instrument, account_group_id) -> (instrument) -> global.
type SpotFundsOverride struct {
	Instrument     marketdata.InstrumentID
	AccountID      optional.Option[param.AccountID]
	AccountGroupID optional.Option[param.AccountGroupID]
	SlippageBps    optional.Option[uint16]
}

//------------------------------------------------------------------------------
// SpotFundsBuilder

// SpotFundsBuilder is the entry point for the spot funds policy.
//
// By default, market orders (orders without a limit price, executed at the
// prevailing market price) are rejected with UnsupportedOrderType and the
// policy operates in limit-only mode. Call [SpotFundsBuilder.WithMarketOrders]
// to enable market orders, supplying a market-data service that provides live
// quotes and the worst-case slippage.
type SpotFundsBuilder struct {
	builder *SpotFundsReadyBuilder
}

// SpotFundsReadyBuilder holds a fully-configured spot funds policy.
type SpotFundsReadyBuilder struct {
	marketData        *marketdata.Service
	marketSlippageBps *uint16
	pricingSource     SpotFundsPricingSource
	overrides         []SpotFundsOverride
	policyGroupID     model.PolicyGroupID
}

// BuildSpotFunds returns a new spot funds policy builder.
//
// Initial balances are seeded through the account-adjustment pipeline, not via
// the builder.
func BuildSpotFunds() *SpotFundsBuilder {
	return &SpotFundsBuilder{
		builder: &SpotFundsReadyBuilder{policyGroupID: model.DefaultPolicyGroupID},
	}
}

// WithMarketOrders enables market orders (orders submitted without a limit
// price, executed at the prevailing market price) and configures how their
// reservations are sized.
//
// `service` is the market-data service the policy reads live quotes from.
//
// `slippageBps` is the worst-case slippage applied to a market-order
// reservation, expressed in basis points (1 bps = 0.01%):
//   - `0` — no slippage; the reservation uses the base price as-is.
//   - `1500` — 15% (the typical conservative default).
//   - `10000` — 100% (reserve up to double the base price).
//
// Without calling this method, market orders are rejected with
// UnsupportedOrderType (limit-only mode).
func (b *SpotFundsBuilder) WithMarketOrders(
	service *marketdata.Service,
	slippageBps uint16,
) *SpotFundsReadyBuilder {
	b.builder.WithMarketOrders(service, slippageBps)
	return b.builder
}

// PolicyGroupID assigns the policy to a pricing group and returns a ready
// builder. When not set the policy uses model.DefaultPolicyGroupID.
func (b *SpotFundsBuilder) PolicyGroupID(groupID model.PolicyGroupID) *SpotFundsReadyBuilder {
	b.builder.PolicyGroupID(groupID)
	return b.builder
}

// WithMarketOrders enables market orders and configures their reservation
// sizing. See [SpotFundsBuilder.WithMarketOrders] for the full contract.
// Calling it more than once replaces both the market-data service and the
// global slippage.
func (b *SpotFundsReadyBuilder) WithMarketOrders(
	service *marketdata.Service,
	slippageBps uint16,
) *SpotFundsReadyBuilder {
	b.marketData = service
	v := slippageBps
	b.marketSlippageBps = &v
	return b
}

// PricingSource selects the base price the policy uses to size market-order
// reservations. The default is SpotFundsPricingSourceMark.
func (b *SpotFundsReadyBuilder) PricingSource(
	source SpotFundsPricingSource,
) *SpotFundsReadyBuilder {
	b.pricingSource = source
	return b
}

// Overrides sets per-instrument slippage overrides, each optionally scoped to
// a single account or an account group. Calling it more than once replaces the
// previous overrides.
func (b *SpotFundsReadyBuilder) Overrides(overrides ...SpotFundsOverride) *SpotFundsReadyBuilder {
	b.overrides = overrides
	return b
}

// PolicyGroupID assigns the policy to a pricing group. When not set the
// policy uses model.DefaultPolicyGroupID.
func (b *SpotFundsReadyBuilder) PolicyGroupID(groupID model.PolicyGroupID) *SpotFundsReadyBuilder {
	b.policyGroupID = groupID
	return b
}

// Build registers the built-in spot funds policy on the given engine builder.
func (b *SpotFundsReadyBuilder) Build(builder native.EngineBuilder) error {
	var marketDataHandle native.MarketDataService
	if b.marketData != nil {
		marketDataHandle = b.marketData.Handle()
	}

	var nativeOverrides []native.PretradePoliciesSpotFundsOverride
	if len(b.overrides) > 0 {
		nativeOverrides = make([]native.PretradePoliciesSpotFundsOverride, len(b.overrides))
		for i, o := range b.overrides {
			var accountPtr *native.ParamAccountID
			if v, has := o.AccountID.Get(); has {
				h := v.Handle()
				accountPtr = &h
			}
			var groupPtr *native.ParamAccountGroupID
			if v, has := o.AccountGroupID.Get(); has {
				h := v.Handle()
				groupPtr = &h
			}
			var slippagePtr *uint16
			if v, has := o.SlippageBps.Get(); has {
				slippagePtr = &v
			}
			nativeOverrides[i] = native.NewPretradePoliciesSpotFundsOverride(
				o.Instrument.Handle(),
				accountPtr,
				groupPtr,
				slippagePtr,
			)
		}
	}

	err := native.EngineBuilderAddBuiltinSpotFunds(
		builder,
		marketDataHandle,
		b.marketSlippageBps,
		uint8(b.pricingSource),
		nativeOverrides,
		native.PolicyGroupID(b.policyGroupID),
	)
	runtime.KeepAlive(b)
	return err
}

// Build registers the built-in spot funds policy on the given engine builder in
// limit-only mode (market orders rejected with UnsupportedOrderType).
// Equivalent to building without calling [SpotFundsBuilder.WithMarketOrders].
func (b *SpotFundsBuilder) Build(builder native.EngineBuilder) error {
	return b.builder.Build(builder)
}
