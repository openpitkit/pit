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

// Package configure provides runtime policy-settings updates bound to an engine.
package configure

import (
	"fmt"
	"runtime"

	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/optional"
	"go.openpit.dev/openpit/pkg/ptr"
	"go.openpit.dev/openpit/pretrade/policies"
	"go.openpit.dev/openpit/reject"
)

// Configurator updates the runtime settings of built-in policies registered on
// an engine. Obtain it from an engine's Configure accessor. It carries no
// state of its own: every call forwards to the engine it was created from,
// and it is valid for as long as that engine is.
type Configurator struct {
	engine native.Engine
}

// NewFromHandle wraps a native engine handle into a Configurator accessor.
func NewFromHandle(engine native.Engine) Configurator {
	return Configurator{engine: engine}
}

//------------------------------------------------------------------------------
// Error

// ErrorKind classifies an Error.
type ErrorKind uint32

const (
	// ErrorKindUnknown means no configurable policy carries the requested name.
	ErrorKindUnknown ErrorKind = ErrorKind(native.ConfigureErrorKindUnknown)
	// ErrorKindTypeMismatch means the policy name matched a policy of a
	// different type than the configure call targets.
	ErrorKindTypeMismatch ErrorKind = ErrorKind(native.ConfigureErrorKindTypeMismatch)
	// ErrorKindValidation means the supplied configuration values failed
	// validation.
	ErrorKindValidation ErrorKind = ErrorKind(native.ConfigureErrorKindValidation)
	// ErrorKindNestedConfiguration means configuration was re-entered on the
	// same thread while another configuration update was active.
	ErrorKindNestedConfiguration ErrorKind = ErrorKind(native.ConfigureErrorKindNestedConfiguration)
)

// Error is returned when a runtime configure call fails.
//
// Kind classifies the failure; Message provides a human-readable description.
type Error struct {
	// Message is the human-readable error description.
	Message string
	// Kind classifies the failure.
	Kind ErrorKind
}

// Error implements the error interface.
func (e *Error) Error() string {
	return e.Message
}

func newErrorFromHandle(handle native.ConfigureError) *Error {
	msg := native.ConfigureErrorGetMessage(handle)
	kind := native.ConfigureErrorGetKind(handle)
	native.DestroyConfigureError(handle)
	return &Error{
		Message: msg,
		Kind:    ErrorKind(kind),
	}
}

func rateLimitWindowNanoseconds(limit policies.RateLimit) int64 {
	return int64(limit.Window)
}

//------------------------------------------------------------------------------
// RateLimit

// RateLimit updates the runtime settings of the named rate-limit policy.
//
// broker, assets, accounts, and accountAssets mirror the axis types accepted by
// [policies.RateLimitReadyBuilder]. A non-nil axis replaces that axis
// wholesale; barriers can be added and removed at runtime. A barrier key that
// survives the replacement keeps its live counter (no reset). An empty non-nil
// slice clears the axis, subject to the policy's at-least-one-barrier rule.
// Nil axes and nil broker are left unchanged.
//
// Returns a *ConfigureError on a domain error (kind TypeMismatch when the name
// belongs to a different policy type, Validation when values are invalid).
func (c Configurator) RateLimit(
	name string,
	broker *policies.RateLimitBrokerBarrier,
	assets []policies.RateLimitAssetBarrier,
	accounts []policies.RateLimitAccountBarrier,
	accountAssets []policies.RateLimitAccountAssetBarrier,
) error {
	return c.RateLimitUpdate(
		name,
		optional.From(broker, broker != nil),
		assets,
		accounts,
		accountAssets,
	)
}

// RateLimitUpdate updates the named rate-limit policy and can also clear the
// broker barrier.
//
// broker uses three states: optional.None leaves the broker unchanged,
// optional.Some(nil) clears it, and optional.Some(&barrier) sets/replaces it.
// Slice arguments use the same semantics as [Configurator.RateLimit].
func (c Configurator) RateLimitUpdate(
	name string,
	broker optional.Option[*policies.RateLimitBrokerBarrier],
	assets []policies.RateLimitAssetBarrier,
	accounts []policies.RateLimitAccountBarrier,
	accountAssets []policies.RateLimitAccountAssetBarrier,
) error {
	var nativeBroker *native.PretradePoliciesRateLimitBrokerBarrier
	brokerValue, hasBroker := broker.Get()
	if hasBroker && brokerValue != nil {
		windowNanos := rateLimitWindowNanoseconds(brokerValue.Limit)
		b := native.NewPretradePoliciesRateLimitBrokerBarrier(
			brokerValue.Limit.MaxOrders,
			windowNanos,
		)
		nativeBroker = ptr.New(b)
	}

	var nativeAssets []native.PretradePoliciesRateLimitAssetBarrier
	if assets != nil {
		nativeAssets = make([]native.PretradePoliciesRateLimitAssetBarrier, 0, len(assets))
		for _, a := range assets {
			windowNanos := rateLimitWindowNanoseconds(a.Limit)
			nativeAssets = append(nativeAssets, native.NewPretradePoliciesRateLimitAssetBarrier(
				a.Limit.MaxOrders,
				windowNanos,
				a.SettlementAsset.Handle(),
			))
		}
	}

	var nativeAccounts []native.PretradePoliciesRateLimitAccountBarrier
	if accounts != nil {
		nativeAccounts = make([]native.PretradePoliciesRateLimitAccountBarrier, 0, len(accounts))
		for _, a := range accounts {
			windowNanos := rateLimitWindowNanoseconds(a.Limit)
			nativeAccounts = append(nativeAccounts, native.NewPretradePoliciesRateLimitAccountBarrier(
				a.AccountID.Handle(),
				a.Limit.MaxOrders,
				windowNanos,
			))
		}
	}

	var nativeAccountAssets []native.PretradePoliciesRateLimitAccountAssetBarrier
	if accountAssets != nil {
		nativeAccountAssets = make([]native.PretradePoliciesRateLimitAccountAssetBarrier, 0, len(accountAssets))
		for _, a := range accountAssets {
			windowNanos := rateLimitWindowNanoseconds(a.Limit)
			nativeAccountAssets = append(
				nativeAccountAssets,
				native.NewPretradePoliciesRateLimitAccountAssetBarrier(
					a.AccountID.Handle(),
					a.Limit.MaxOrders,
					windowNanos,
					a.SettlementAsset.Handle(),
				),
			)
		}
	}

	configErr := native.EngineConfigureRateLimit(
		c.engine,
		name,
		nativeBroker,
		hasBroker,
		nativeAssets,
		nativeAccounts,
		nativeAccountAssets,
	)
	if configErr != nil {
		return newErrorFromHandle(configErr)
	}
	return nil
}

//------------------------------------------------------------------------------
// PnlBoundsKillSwitch

// PnlBoundsKillSwitch updates the runtime settings of the named P&L bounds
// kill-switch policy.
//
// brokerBarriers mirrors the broker axis accepted by
// [policies.PnlBoundsKillSwitchReadyBuilder]. accountBarriers updates bounds
// without replacing the live P&L accumulated for each account and settlement
// asset. An axis passed as nil is left unchanged; an empty non-nil slice
// replaces the axis with an empty set (subject to the policy's
// at-least-one-barrier rule).
//
// Returns a *Error on a domain error.
func (c Configurator) PnlBoundsKillSwitch(
	name string,
	brokerBarriers []policies.PnlBoundsBrokerBarrier,
	accountBarriers []policies.PnlBoundsAccountAssetBarrierUpdate,
) error {
	var nativeBrokers []native.PretradePoliciesPnlBoundsBarrier
	if brokerBarriers != nil {
		nativeBrokers = make([]native.PretradePoliciesPnlBoundsBarrier, 0, len(brokerBarriers))
		for _, b := range brokerBarriers {
			nativeBrokers = append(nativeBrokers, native.NewPretradePoliciesPnlBoundsBarrier(
				b.SettlementAsset.Handle(),
				pnlOptionalToNative(b.LowerBound),
				pnlOptionalToNative(b.UpperBound),
			))
		}
	}

	var nativeAccounts []native.PretradePoliciesPnlBoundsAccountBarrierUpdate
	if accountBarriers != nil {
		nativeAccounts = make(
			[]native.PretradePoliciesPnlBoundsAccountBarrierUpdate,
			0,
			len(accountBarriers),
		)
		for _, a := range accountBarriers {
			nativeAccounts = append(
				nativeAccounts,
				native.NewPretradePoliciesPnlBoundsAccountBarrierUpdate(
					a.AccountID.Handle(),
					a.Barrier.SettlementAsset.Handle(),
					pnlOptionalToNative(a.Barrier.LowerBound),
					pnlOptionalToNative(a.Barrier.UpperBound),
				),
			)
		}
	}

	configErr := native.EngineConfigurePnlBoundsKillSwitch(
		c.engine,
		name,
		nativeBrokers,
		nativeAccounts,
	)
	if configErr != nil {
		return newErrorFromHandle(configErr)
	}
	return nil
}

//------------------------------------------------------------------------------
// SetAccountPnl

// SetAccountPnl force-sets the live accumulated P&L for one
// (account, settlementAsset) entry of the named P&L bounds kill-switch policy.
//
// This is an absolute assignment (upsert): the entry is created if it does not
// exist yet, exactly as a construction-time seed would. It is distinct from
// [Configurator.PnlBoundsKillSwitch], which retunes bounds and never touches
// accumulated P&L. The new value is evaluated against the live bounds on the
// next check; forcing the accumulator past a bound trips the kill switch and
// latches an engine-level account block that this call never clears.
//
// Returns a *Error on a domain error (kind TypeMismatch when the name belongs
// to a different policy type, Unknown when no policy carries the name).
func (c Configurator) SetAccountPnl(
	name string,
	account param.AccountID,
	settlementAsset param.Asset,
	pnl param.Pnl,
) error {
	configErr := native.EngineSetAccountPnl(
		c.engine,
		name,
		account.Handle(),
		settlementAsset.Handle(),
		pnl.Handle(),
	)
	if configErr != nil {
		return newErrorFromHandle(configErr)
	}
	return nil
}

//------------------------------------------------------------------------------
// OrderSizeLimit

// OrderSizeLimit updates the runtime settings of the named order-size-limit
// policy.
//
// broker, assets, and accountAssets mirror the axis types accepted by
// [policies.OrderSizeLimitReadyBuilder]. An axis passed as nil is left
// unchanged; an empty non-nil slice replaces the axis with an empty set
// (subject to the policy's at-least-one-barrier rule). A nil broker leaves
// the broker barrier unchanged.
//
// Returns a *Error on a domain error.
func (c Configurator) OrderSizeLimit(
	name string,
	broker *policies.OrderSizeBrokerBarrier,
	assets []policies.OrderSizeAssetBarrier,
	accountAssets []policies.OrderSizeAccountAssetBarrier,
) error {
	return c.OrderSizeLimitUpdate(
		name,
		optional.From(broker, broker != nil),
		assets,
		accountAssets,
	)
}

// OrderSizeLimitUpdate updates the named order-size-limit policy and can also
// clear the broker barrier.
//
// broker uses three states: optional.None leaves the broker unchanged,
// optional.Some(nil) clears it, and optional.Some(&barrier) sets/replaces it.
// Slice arguments use the same semantics as [Configurator.OrderSizeLimit].
func (c Configurator) OrderSizeLimitUpdate(
	name string,
	broker optional.Option[*policies.OrderSizeBrokerBarrier],
	assets []policies.OrderSizeAssetBarrier,
	accountAssets []policies.OrderSizeAccountAssetBarrier,
) error {
	var nativeBroker *native.PretradePoliciesOrderSizeBrokerBarrier
	brokerValue, hasBroker := broker.Get()
	if hasBroker && brokerValue != nil {
		b := native.NewPretradePoliciesOrderSizeBrokerBarrier(
			native.NewPretradePoliciesOrderSizeLimit(
				brokerValue.Limit.MaxQuantity.Handle(),
				brokerValue.Limit.MaxNotional.Handle(),
			),
		)
		nativeBroker = ptr.New(b)
	}

	var nativeAssets []native.PretradePoliciesOrderSizeAssetBarrier
	if assets != nil {
		nativeAssets = make([]native.PretradePoliciesOrderSizeAssetBarrier, 0, len(assets))
		for _, a := range assets {
			nativeAssets = append(nativeAssets, native.NewPretradePoliciesOrderSizeAssetBarrier(
				native.NewPretradePoliciesOrderSizeLimit(
					a.Limit.MaxQuantity.Handle(),
					a.Limit.MaxNotional.Handle(),
				),
				a.SettlementAsset.Handle(),
			))
		}
	}

	var nativeAccountAssets []native.PretradePoliciesOrderSizeAccountAssetBarrier
	if accountAssets != nil {
		nativeAccountAssets = make([]native.PretradePoliciesOrderSizeAccountAssetBarrier, 0, len(accountAssets))
		for _, a := range accountAssets {
			nativeAccountAssets = append(
				nativeAccountAssets,
				native.NewPretradePoliciesOrderSizeAccountAssetBarrier(
					native.NewPretradePoliciesOrderSizeLimit(
						a.Limit.MaxQuantity.Handle(),
						a.Limit.MaxNotional.Handle(),
					),
					a.AccountID.Handle(),
					a.SettlementAsset.Handle(),
				),
			)
		}
	}

	configErr := native.EngineConfigureOrderSizeLimit(
		c.engine,
		name,
		nativeBroker,
		hasBroker,
		nativeAssets,
		nativeAccountAssets,
	)
	if configErr != nil {
		return newErrorFromHandle(configErr)
	}
	return nil
}

//------------------------------------------------------------------------------
// SpotFunds

// SpotFunds updates the runtime settings of the named spot-funds policy.
//
// globalSlippageBps and pricingSource are optional: pass None to leave them
// unchanged. Each override entry is applied individually (insert-or-clear):
// an entry whose SlippageBps is None clears any override at its target. A nil
// overrides slice leaves the cascade untouched; entries never replace the
// whole table.
//
// The overrides slice and [policies.SpotFundsOverrideEntry] type mirror those
// accepted by [policies.SpotFundsReadyBuilder.Overrides].
//
// Returns a *Error on a domain error.
func (c Configurator) SpotFunds(
	name string,
	globalSlippageBps optional.Option[uint16],
	pricingSource optional.Option[policies.SpotFundsPricingSource],
	overrides []policies.SpotFundsOverrideEntry,
) error {
	var slippagePtr *uint16
	if v, has := globalSlippageBps.Get(); has {
		slippagePtr = ptr.New(v)
	}

	var pricingSourcePtr *uint8
	if v, has := pricingSource.Get(); has {
		u := uint8(v)
		pricingSourcePtr = &u
	}

	var nativeOverrides []native.PretradePoliciesSpotFundsOverride
	if overrides != nil {
		nativeOverrides = make([]native.PretradePoliciesSpotFundsOverride, 0, len(overrides))
		for i, e := range overrides {
			var slippageBpsPtr *uint16
			if v, has := e.Override.SlippageBps.Get(); has {
				slippageBpsPtr = ptr.New(v)
			}
			override, err := policies.NewNativeSpotFundsOverride(e.Target, slippageBpsPtr)
			if err != nil {
				return &Error{
					Kind:    ErrorKindValidation,
					Message: fmt.Sprintf("configure: spot funds override %d: %v", i, err),
				}
			}
			nativeOverrides = append(nativeOverrides, override)
		}
	}

	configErr := native.EngineConfigureSpotFunds(
		c.engine,
		name,
		slippagePtr,
		pricingSourcePtr,
		nativeOverrides,
	)
	if configErr != nil {
		return newErrorFromHandle(configErr)
	}
	return nil
}

// SpotFundsGlobalLimitMode sets the global limit mode of the named spot-funds
// policy at runtime. Enforce rejects a reservation that exceeds available
// funds; TrackOnly always records it and lets available go negative.
//
// The mode is the global tier of the cascade; per-account and
// per-account-group overrides set via [Configurator.SpotFundsAccountLimitMode]
// and [Configurator.SpotFundsAccountGroupLimitMode] still take precedence.
//
// Returns a *Error on a domain error.
func (c Configurator) SpotFundsGlobalLimitMode(
	name string,
	mode policies.SpotFundsLimitMode,
) error {
	configErr := native.EngineConfigureSpotFundsGlobalLimitMode(
		c.engine,
		name,
		policies.NativeSpotFundsLimitMode(mode),
	)
	if configErr != nil {
		return newErrorFromHandle(configErr)
	}
	return nil
}

// SpotFundsAccountLimitMode pins or clears the per-account limit mode of the
// named spot-funds policy at runtime. The per-account override wins over the
// account-group and global tiers.
//
// optional.Some pins the account to that mode; optional.None clears any
// existing per-account override so the cascade falls through to the
// account-group and global tiers.
//
// Returns a *Error on a domain error.
func (c Configurator) SpotFundsAccountLimitMode(
	name string,
	accountID param.AccountID,
	mode optional.Option[policies.SpotFundsLimitMode],
) error {
	nativeMode, hasMode := policies.NativeSpotFundsLimitModeOption(mode)
	configErr := native.EngineConfigureSpotFundsAccountLimitMode(
		c.engine,
		name,
		accountID.Handle(),
		nativeMode,
		hasMode,
	)
	if configErr != nil {
		return newErrorFromHandle(configErr)
	}
	return nil
}

// SpotFundsAccountGroupLimitMode pins or clears the per-account-group limit
// mode of the named spot-funds policy at runtime. The override applies to every
// account in the group that has no per-account override.
//
// optional.Some pins the group to that mode; optional.None clears any existing
// per-account-group override so the cascade falls through to the global tier.
//
// Returns a *Error on a domain error.
func (c Configurator) SpotFundsAccountGroupLimitMode(
	name string,
	accountGroupID param.AccountGroupID,
	mode optional.Option[policies.SpotFundsLimitMode],
) error {
	nativeMode, hasMode := policies.NativeSpotFundsLimitModeOption(mode)
	configErr := native.EngineConfigureSpotFundsAccountGroupLimitMode(
		c.engine,
		name,
		accountGroupID.Handle(),
		nativeMode,
		hasMode,
	)
	if configErr != nil {
		return newErrorFromHandle(configErr)
	}
	return nil
}

// SpotFundsPnlBoundsKillSwitch updates the account P&L bounds axis of
// the named spot-funds policy.
//
// globalBarrier uses three states: optional.None leaves the global barrier
// unchanged, optional.Some(nil) clears it, and optional.Some(&barrier) sets it.
// Nil slices leave their axes untouched; non-nil empty slices clear them.
// Barrier updates preserve live accumulated P&L.
//
// Returns a *Error on a domain error.
func (c Configurator) SpotFundsPnlBoundsKillSwitch(
	name string,
	globalBarrier optional.Option[*policies.SpotFundsPnlBoundsBarrier],
	accountGroupBarriers []policies.SpotFundsPnlBoundsAccountGroupBarrier,
	accountBarriers []policies.SpotFundsPnlBoundsAccountBarrier,
) error {
	var nativeGlobal *native.PretradePoliciesSpotFundsPnlBoundsBarrier
	globalValue, hasGlobal := globalBarrier.Get()
	if hasGlobal && globalValue != nil {
		barrier := nativeSpotFundsPnlBoundsBarrier(*globalValue)
		nativeGlobal = &barrier
	}

	var nativeAccountGroups []native.PretradePoliciesSpotFundsPnlBoundsAccountGroupBarrier
	if accountGroupBarriers != nil {
		nativeAccountGroups = make(
			[]native.PretradePoliciesSpotFundsPnlBoundsAccountGroupBarrier,
			0,
			len(accountGroupBarriers),
		)
		for _, b := range accountGroupBarriers {
			nativeAccountGroups = append(
				nativeAccountGroups,
				native.NewPretradePoliciesSpotFundsPnlBoundsAccountGroupBarrier(
					b.AccountGroupID.Handle(),
					nativeSpotFundsPnlBoundsBarrier(b.Barrier),
				),
			)
		}
	}

	var nativeAccounts []native.PretradePoliciesSpotFundsPnlBoundsAccountBarrier
	if accountBarriers != nil {
		nativeAccounts = make(
			[]native.PretradePoliciesSpotFundsPnlBoundsAccountBarrier,
			0,
			len(accountBarriers),
		)
		for _, b := range accountBarriers {
			nativeAccounts = append(
				nativeAccounts,
				native.NewPretradePoliciesSpotFundsPnlBoundsAccountBarrier(
					b.AccountID.Handle(),
					nativeSpotFundsPnlBoundsBarrier(b.Barrier),
				),
			)
		}
	}

	configErr := native.EngineConfigureSpotFundsPnlBoundsKillSwitch(
		c.engine,
		name,
		nativeGlobal,
		hasGlobal,
		nativeAccountGroups,
		nativeAccounts,
	)
	runtime.KeepAlive(globalBarrier)
	runtime.KeepAlive(accountGroupBarriers)
	runtime.KeepAlive(accountBarriers)
	if configErr != nil {
		return newErrorFromHandle(configErr)
	}
	return nil
}

// SetSpotFundsAccountPnl force-sets the live accumulated account P&L state for
// one account entry of the named spot-funds policy.
//
// This is an absolute assignment (upsert). It is distinct from
// [Configurator.SpotFundsPnlBoundsKillSwitch], which retunes bounds and never
// touches accumulated P&L.
//
// On success it returns PolicyConfigurationResult. AccountBlocks is non-empty
// when the assignment immediately causes the configured P&L kill switch to
// block the account, including a numeric value beyond a barrier. A
// configuration failure returns the existing configure error.
func (c Configurator) SetSpotFundsAccountPnl(
	name string,
	account param.AccountID,
	state model.PnlState,
) (PolicyConfigurationResult, error) {
	blocks, configErr := native.EngineSetSpotFundsAccountPnl(
		c.engine,
		name,
		account.Handle(),
		state.Handle(),
	)
	if configErr != nil {
		return PolicyConfigurationResult{}, newErrorFromHandle(configErr)
	}
	defer native.DestroyPretradeAccountBlockList(blocks)
	accountBlocks := make(
		[]reject.AccountBlock,
		native.PretradeAccountBlockListLen(blocks),
	)
	for index := range accountBlocks {
		accountBlocks[index] = reject.NewAccountBlockFromHandle(
			native.PretradeAccountBlockListGet(blocks, index),
		)
	}
	return PolicyConfigurationResult{AccountBlocks: accountBlocks}, nil
}

// PolicyConfigurationResult describes an accepted runtime policy update.
//
// AccountBlocks is non-empty when the update immediately caused the engine to
// block an account. An empty list means the update was accepted without an
// account block.
type PolicyConfigurationResult struct {
	AccountBlocks []reject.AccountBlock
}

//------------------------------------------------------------------------------
// Helpers

func pnlOptionalToNative(value optional.Option[param.Pnl]) native.ParamPnlOptional {
	if v, has := value.Get(); has {
		return native.NewParamPnlOptional(v.Handle())
	}
	return native.ParamPnlOptional{}
}

func nativeSpotFundsPnlBoundsBarrier(
	barrier policies.SpotFundsPnlBoundsBarrier,
) native.PretradePoliciesSpotFundsPnlBoundsBarrier {
	return native.NewPretradePoliciesSpotFundsPnlBoundsBarrier(
		pnlOptionalToNative(barrier.LowerBound),
		pnlOptionalToNative(barrier.UpperBound),
	)
}
