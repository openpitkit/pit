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

package policies

import (
	"fmt"
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

// SpotFundsOverrideTarget selects which accounts a SpotFundsOverride applies
// to within the slippage resolution cascade.
//
// Use one of the concrete types that implement this interface:
//   - [SpotFundsOverrideTargetInstrument] - instrument-level default
//   - [SpotFundsOverrideTargetInstrumentAccount] - scoped to one account
//   - [SpotFundsOverrideTargetInstrumentAccountGroup] - scoped to one account
//     group
type SpotFundsOverrideTarget interface {
	spotFundsOverrideTarget()
}

// SpotFundsOverrideTargetInstrument is an instrument-level default: applies
// when no account- or account-group-scoped override matches the order's
// account.
type SpotFundsOverrideTargetInstrument struct {
	Instrument marketdata.InstrumentID
}

// SpotFundsOverrideTargetInstrumentAccount applies to the instrument only for
// this exact account (highest priority in the cascade).
type SpotFundsOverrideTargetInstrumentAccount struct {
	Instrument marketdata.InstrumentID
	AccountID  param.AccountID
}

// SpotFundsOverrideTargetInstrumentAccountGroup applies to the instrument only
// for accounts in this account group.
type SpotFundsOverrideTargetInstrumentAccountGroup struct {
	Instrument     marketdata.InstrumentID
	AccountGroupID param.AccountGroupID
}

func (SpotFundsOverrideTargetInstrument) spotFundsOverrideTarget()             {}
func (SpotFundsOverrideTargetInstrumentAccount) spotFundsOverrideTarget()      {}
func (SpotFundsOverrideTargetInstrumentAccountGroup) spotFundsOverrideTarget() {}

// SpotFundsOverride is the override value applied at a
// [SpotFundsOverrideTarget]. When SlippageBps is None the entry is ignored
// and the cascade falls through to the next tier.
type SpotFundsOverride struct {
	// SlippageBps is the slippage applied at the target. None defers to the
	// next tier of the cascade (and ultimately the global slippage).
	SlippageBps optional.Option[uint16]
}

// SpotFundsOverrideEntry pairs a target with its override value. Passed to
// [SpotFundsReadyBuilder.Overrides].
//
// Resolution order: (instrument, account_id) ->
// (instrument, account_group_id) -> (instrument) -> global.
type SpotFundsOverrideEntry struct {
	Target   SpotFundsOverrideTarget
	Override SpotFundsOverride
}

// SpotFundsPnlBoundsBarrier defines self-computed account P&L bounds.
type SpotFundsPnlBoundsBarrier struct {
	// LowerBound is typically negative and represents the loss limit.
	LowerBound optional.Option[param.Pnl]
	// UpperBound is typically positive and represents the profit-taking limit.
	UpperBound optional.Option[param.Pnl]
}

// SpotFundsPnlBoundsAccountGroupBarrier defines a per-account-group P&L bounds
// override.
type SpotFundsPnlBoundsAccountGroupBarrier struct {
	Barrier        SpotFundsPnlBoundsBarrier
	AccountGroupID param.AccountGroupID
}

// SpotFundsPnlBoundsAccountBarrier defines a per-account P&L bounds override.
type SpotFundsPnlBoundsAccountBarrier struct {
	Barrier   SpotFundsPnlBoundsBarrier
	AccountID param.AccountID
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
	overrides         []SpotFundsOverrideEntry
	policyGroupID     model.PolicyGroupID
}

// SpotFundsPnlBoundsKillSwitchBuilder is the entry point for the spot-funds
// self-computed P&L bounds policy.
//
// This preset configures the funds-limit axis as TrackOnly: insufficient funds
// do not reject reservations, and available funds may become negative while
// the policy continues tracking balances. Arithmetic overflow is still an
// error. Market orders use mark pricing with zero slippage.
type SpotFundsPnlBoundsKillSwitchBuilder struct {
	builder *SpotFundsPnlBoundsKillSwitchReadyBuilder
}

// SpotFundsPnlBoundsKillSwitchReadyBuilder holds a fully-configured
// spot-funds self-computed P&L bounds policy.
type SpotFundsPnlBoundsKillSwitchReadyBuilder struct {
	marketData           *marketdata.Service
	globalBarrier        *native.PretradePoliciesSpotFundsPnlBoundsBarrier
	accountGroupBarriers []native.PretradePoliciesSpotFundsPnlBoundsAccountGroupBarrier
	accountBarriers      []native.PretradePoliciesSpotFundsPnlBoundsAccountBarrier
	policyGroupID        model.PolicyGroupID
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

// BuildSpotFundsPnlBoundsKillSwitch returns a new spot-funds P&L bounds
// kill-switch policy builder. The preset disables insufficient-funds gating by
// using TrackOnly; it tracks reservations and lets available funds go negative.
func BuildSpotFundsPnlBoundsKillSwitch() *SpotFundsPnlBoundsKillSwitchBuilder {
	return &SpotFundsPnlBoundsKillSwitchBuilder{
		builder: &SpotFundsPnlBoundsKillSwitchReadyBuilder{
			policyGroupID: model.DefaultPolicyGroupID,
		},
	}
}

// WithMarketData configures the market-data service used for FX conversion.
func (b *SpotFundsPnlBoundsKillSwitchBuilder) WithMarketData(
	service *marketdata.Service,
) *SpotFundsPnlBoundsKillSwitchReadyBuilder {
	b.builder.WithMarketData(service)
	return b.builder
}

// WithMarketData configures the market-data service used for FX conversion.
func (b *SpotFundsPnlBoundsKillSwitchReadyBuilder) WithMarketData(
	service *marketdata.Service,
) *SpotFundsPnlBoundsKillSwitchReadyBuilder {
	b.marketData = service
	return b
}

// PolicyGroupID assigns the policy to a pricing group and returns a ready
// builder. When not set the policy uses model.DefaultPolicyGroupID.
func (b *SpotFundsPnlBoundsKillSwitchBuilder) PolicyGroupID(
	groupID model.PolicyGroupID,
) *SpotFundsPnlBoundsKillSwitchReadyBuilder {
	b.builder.PolicyGroupID(groupID)
	return b.builder
}

// PolicyGroupID assigns the policy to a pricing group. When not set the policy
// uses model.DefaultPolicyGroupID.
func (b *SpotFundsPnlBoundsKillSwitchReadyBuilder) PolicyGroupID(
	groupID model.PolicyGroupID,
) *SpotFundsPnlBoundsKillSwitchReadyBuilder {
	b.policyGroupID = groupID
	return b
}

// GlobalBarrier sets the global account P&L bounds and returns a ready
// builder.
func (b *SpotFundsPnlBoundsKillSwitchBuilder) GlobalBarrier(
	barrier SpotFundsPnlBoundsBarrier,
) *SpotFundsPnlBoundsKillSwitchReadyBuilder {
	b.builder.GlobalBarrier(barrier)
	return b.builder
}

// GlobalBarrier sets the global account P&L bounds.
func (b *SpotFundsPnlBoundsKillSwitchReadyBuilder) GlobalBarrier(
	barrier SpotFundsPnlBoundsBarrier,
) *SpotFundsPnlBoundsKillSwitchReadyBuilder {
	nativeBarrier := newNativeSpotFundsPnlBoundsBarrier(barrier)
	b.globalBarrier = &nativeBarrier
	return b
}

// AccountGroupBarriers adds per-account-group account P&L bounds and
// returns a ready builder.
func (b *SpotFundsPnlBoundsKillSwitchBuilder) AccountGroupBarriers(
	barriers ...SpotFundsPnlBoundsAccountGroupBarrier,
) *SpotFundsPnlBoundsKillSwitchReadyBuilder {
	b.builder.AccountGroupBarriers(barriers...)
	return b.builder
}

// AccountGroupBarriers appends per-account-group account P&L bounds.
func (b *SpotFundsPnlBoundsKillSwitchReadyBuilder) AccountGroupBarriers(
	barriers ...SpotFundsPnlBoundsAccountGroupBarrier,
) *SpotFundsPnlBoundsKillSwitchReadyBuilder {
	for _, barrier := range barriers {
		nativeBarrier := newNativeSpotFundsPnlBoundsBarrier(barrier.Barrier)
		b.accountGroupBarriers = append(
			b.accountGroupBarriers,
			native.NewPretradePoliciesSpotFundsPnlBoundsAccountGroupBarrier(
				barrier.AccountGroupID.Handle(),
				nativeBarrier,
			),
		)
	}
	return b
}

// AccountBarriers adds per-account account P&L bounds and returns a
// ready builder.
func (b *SpotFundsPnlBoundsKillSwitchBuilder) AccountBarriers(
	barriers ...SpotFundsPnlBoundsAccountBarrier,
) *SpotFundsPnlBoundsKillSwitchReadyBuilder {
	b.builder.AccountBarriers(barriers...)
	return b.builder
}

// AccountBarriers appends per-account account P&L bounds.
func (b *SpotFundsPnlBoundsKillSwitchReadyBuilder) AccountBarriers(
	barriers ...SpotFundsPnlBoundsAccountBarrier,
) *SpotFundsPnlBoundsKillSwitchReadyBuilder {
	for _, barrier := range barriers {
		nativeBarrier := newNativeSpotFundsPnlBoundsBarrier(barrier.Barrier)
		b.accountBarriers = append(
			b.accountBarriers,
			native.NewPretradePoliciesSpotFundsPnlBoundsAccountBarrier(
				barrier.AccountID.Handle(),
				nativeBarrier,
			),
		)
	}
	return b
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

// Overrides sets per-instrument slippage overrides, each paired with a target
// that selects the scope. Calling it more than once replaces the previous
// overrides.
func (b *SpotFundsReadyBuilder) Overrides(
	overrides ...SpotFundsOverrideEntry,
) *SpotFundsReadyBuilder {
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
		for i, e := range b.overrides {
			var slippagePtr *uint16
			if v, has := e.Override.SlippageBps.Get(); has {
				slippagePtr = &v
			}
			override, err := NewNativeSpotFundsOverride(e.Target, slippagePtr)
			if err != nil {
				return fmt.Errorf("spot funds override %d: %w", i, err)
			}
			nativeOverrides[i] = override
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

// Build registers the built-in spot-funds P&L bounds kill-switch preset on the
// given engine builder. The preset uses TrackOnly and therefore does not reject
// reservations for insufficient funds.
func (b *SpotFundsPnlBoundsKillSwitchReadyBuilder) Build(builder native.EngineBuilder) error {
	var marketDataHandle native.MarketDataService
	if b.marketData != nil {
		marketDataHandle = b.marketData.Handle()
	}
	err := native.EngineBuilderAddBuiltinSpotFundsPnlBoundsKillSwitch(
		builder,
		marketDataHandle,
		native.PolicyGroupID(b.policyGroupID),
		b.globalBarrier,
		b.accountGroupBarriers,
		b.accountBarriers,
	)
	runtime.KeepAlive(b)
	return err
}

// Build registers the built-in spot-funds P&L bounds kill-switch preset on the
// given engine builder. The preset uses TrackOnly and therefore does not reject
// reservations for insufficient funds.
func (b *SpotFundsPnlBoundsKillSwitchBuilder) Build(builder native.EngineBuilder) error {
	return b.builder.Build(builder)
}

// NativeSpotFundsLimitMode translates a [SpotFundsLimitMode] into the
// native limit-mode value. It is a thin marshalling helper shared with the
// runtime configurator; the cascade and resolution logic live in the core.
func NativeSpotFundsLimitMode(mode SpotFundsLimitMode) native.PretradePoliciesSpotFundsLimitMode {
	return mode.Handle()
}

// nativeSpotFundsLimitModeIgnored is sent only when hasMode == false; the
// native ABI ignores the mode byte in that branch.
const nativeSpotFundsLimitModeIgnored native.PretradePoliciesSpotFundsLimitMode = 0

// NativeSpotFundsLimitModeOption translates a pin-or-clear limit mode into the
// native (mode, hasMode) pair the runtime setters expect: Some pins the mode
// (hasMode == true), None clears any existing override (hasMode == false, mode
// ignored). Shared with the runtime configurator; no business logic.
func NativeSpotFundsLimitModeOption(
	mode optional.Option[SpotFundsLimitMode],
) (native.PretradePoliciesSpotFundsLimitMode, bool) {
	if v, has := mode.Get(); has {
		return v.Handle(), true
	}
	return nativeSpotFundsLimitModeIgnored, false
}

func newNativeSpotFundsPnlBoundsBarrier(
	barrier SpotFundsPnlBoundsBarrier,
) native.PretradePoliciesSpotFundsPnlBoundsBarrier {
	return native.NewPretradePoliciesSpotFundsPnlBoundsBarrier(
		newParamPnlOptionalFromOptional(barrier.LowerBound),
		newParamPnlOptionalFromOptional(barrier.UpperBound),
	)
}

// NewNativeSpotFundsOverride translates a [SpotFundsOverrideTarget] and its
// slippage into the native tagged-union override. It is shared by the spot
// funds builder and the runtime configurator.
func NewNativeSpotFundsOverride(
	target SpotFundsOverrideTarget,
	slippageBps *uint16,
) (native.PretradePoliciesSpotFundsOverride, error) {
	var zero native.PretradePoliciesSpotFundsOverride

	switch target := target.(type) {
	case SpotFundsOverrideTargetInstrument:
		return native.NewPretradePoliciesSpotFundsOverride(
				target.Instrument.Handle(),
				nil,
				nil,
				slippageBps,
			),
			nil
	case *SpotFundsOverrideTargetInstrument:
		if target == nil {
			return zero, fmt.Errorf("target is nil")
		}
		return native.NewPretradePoliciesSpotFundsOverride(
				target.Instrument.Handle(),
				nil,
				nil,
				slippageBps,
			),
			nil
	case SpotFundsOverrideTargetInstrumentAccount:
		accountID := target.AccountID.Handle()
		return native.NewPretradePoliciesSpotFundsOverride(
				target.Instrument.Handle(),
				&accountID,
				nil,
				slippageBps,
			),
			nil
	case *SpotFundsOverrideTargetInstrumentAccount:
		if target == nil {
			return zero, fmt.Errorf("target is nil")
		}
		accountID := target.AccountID.Handle()
		return native.NewPretradePoliciesSpotFundsOverride(
				target.Instrument.Handle(),
				&accountID,
				nil,
				slippageBps,
			),
			nil
	case SpotFundsOverrideTargetInstrumentAccountGroup:
		accountGroupID := target.AccountGroupID.Handle()
		return native.NewPretradePoliciesSpotFundsOverride(
				target.Instrument.Handle(),
				nil,
				&accountGroupID,
				slippageBps,
			),
			nil
	case *SpotFundsOverrideTargetInstrumentAccountGroup:
		if target == nil {
			return zero, fmt.Errorf("target is nil")
		}
		accountGroupID := target.AccountGroupID.Handle()
		return native.NewPretradePoliciesSpotFundsOverride(
				target.Instrument.Handle(),
				nil,
				&accountGroupID,
				slippageBps,
			),
			nil
	case nil:
		return zero, fmt.Errorf("target is nil")
	default:
		return zero, fmt.Errorf(
			"unsupported target type %T",
			target,
		)
	}
}
