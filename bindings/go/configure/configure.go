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

// Package configure provides runtime policy-settings updates bound to an
// engine.
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
//
// The SpotFunds retune methods - SpotFunds, SpotFundsPnlBoundsKillSwitch,
// SpotFundsPnlBoundsAxis, SpotFundsGlobalLimitMode, SpotFundsAccountLimitMode,
// SpotFundsPositionLimit, and SpotFundsAccountGroupLimitMode - are exclusive
// operations in every mode, including FullSync:
// run each with no other call on the engine in flight, as
// asyncengine.AsyncEngine.Exclusive does.
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
// brokers, assets, accounts, and accountAssets mirror the axis types accepted
// by [policies.RateLimitReadyBuilder]. A non-nil axis replaces that axis across
// every mode; barriers can be added and removed at runtime. A (key, mode) that
// survives the replacement keeps its live counter (no reset). An empty non-nil
// slice clears every mode on the axis. Clearing every axis admits every order.
// A repeated (key, mode) in one call returns a *Error with kind
// ErrorKindValidation and leaves the settings unchanged. Clearing an account or
// account+asset barrier keeps its sliding-window log; re-adding the key resumes
// counting orders still inside its window. Nil axes are left unchanged.
//
// Returns a *Error on a domain error (kind TypeMismatch when the name belongs
// to a different policy type, Validation when values are invalid).
func (c Configurator) RateLimit(
	name string,
	brokers []policies.RateLimitBrokerBarrier,
	assets []policies.RateLimitAssetBarrier,
	accounts []policies.RateLimitAccountBarrier,
	accountAssets []policies.RateLimitAccountAssetBarrier,
) error {
	var nativeBrokers []native.PretradePoliciesRateLimitBrokerBarrier
	if brokers != nil {
		nativeBrokers = make([]native.PretradePoliciesRateLimitBrokerBarrier, 0, len(brokers))
		for i, b := range brokers {
			if err := native.ValidateRuleMode(native.PretradeRuleMode(b.Mode), "broker", i); err != nil {
				return &Error{Message: err.Error(), Kind: ErrorKindValidation}
			}
			windowNanos := rateLimitWindowNanoseconds(b.Limit)
			nativeBrokers = append(nativeBrokers, native.NewPretradePoliciesRateLimitBrokerBarrier(
				native.PretradeRuleMode(b.Mode),
				b.Limit.MaxOrders,
				windowNanos,
			))
		}
	}

	var nativeAssets []native.PretradePoliciesRateLimitAssetBarrier
	if assets != nil {
		nativeAssets = make([]native.PretradePoliciesRateLimitAssetBarrier, 0, len(assets))
		for i, a := range assets {
			if err := native.ValidateRuleMode(native.PretradeRuleMode(a.Mode), "asset", i); err != nil {
				return &Error{Message: err.Error(), Kind: ErrorKindValidation}
			}
			windowNanos := rateLimitWindowNanoseconds(a.Limit)
			nativeAssets = append(nativeAssets, native.NewPretradePoliciesRateLimitAssetBarrier(
				native.PretradeRuleMode(a.Mode),
				a.Limit.MaxOrders,
				windowNanos,
				a.SettlementAsset.Handle(),
			))
		}
	}

	var nativeAccounts []native.PretradePoliciesRateLimitAccountBarrier
	if accounts != nil {
		nativeAccounts = make([]native.PretradePoliciesRateLimitAccountBarrier, 0, len(accounts))
		for i, a := range accounts {
			if err := native.ValidateRuleMode(native.PretradeRuleMode(a.Mode), "account", i); err != nil {
				return &Error{Message: err.Error(), Kind: ErrorKindValidation}
			}
			windowNanos := rateLimitWindowNanoseconds(a.Limit)
			nativeAccounts = append(nativeAccounts, native.NewPretradePoliciesRateLimitAccountBarrier(
				native.PretradeRuleMode(a.Mode),
				a.AccountID.Handle(),
				a.Limit.MaxOrders,
				windowNanos,
			))
		}
	}

	var nativeAccountAssets []native.PretradePoliciesRateLimitAccountAssetBarrier
	if accountAssets != nil {
		nativeAccountAssets = make([]native.PretradePoliciesRateLimitAccountAssetBarrier, 0, len(accountAssets))
		for i, a := range accountAssets {
			if err := native.ValidateRuleMode(native.PretradeRuleMode(a.Mode), "account_asset", i); err != nil {
				return &Error{Message: err.Error(), Kind: ErrorKindValidation}
			}
			windowNanos := rateLimitWindowNanoseconds(a.Limit)
			nativeAccountAssets = append(
				nativeAccountAssets,
				native.NewPretradePoliciesRateLimitAccountAssetBarrier(
					native.PretradeRuleMode(a.Mode),
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
		nativeBrokers,
		nativeAssets,
		nativeAccounts,
		nativeAccountAssets,
	)
	runtime.KeepAlive(assets)
	runtime.KeepAlive(accountAssets)
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
// Mode (Barrier.Mode for account barriers) is part of the key. Its zero
// value is Reject. Each (asset, mode) or (account, asset, mode) may occur only
// once per call. Reject and review barriers on one asset coexist.
// See [policies.PnlBoundsKillSwitchReadyBuilder] for how breaches are handled.
// Invalid modes or duplicate keys return an Error with
// ErrorKindValidation and leave the previous barriers unchanged.
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
				native.PretradeRuleMode(b.Mode),
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
					native.PretradeRuleMode(a.Barrier.Mode),
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
	runtime.KeepAlive(brokerBarriers)
	runtime.KeepAlive(accountBarriers)
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
// next check. This call never clears an existing account block.
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
	runtime.KeepAlive(settlementAsset)
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
// unchanged; an empty non-nil slice clears the axis. Clearing every axis is
// valid and admits every order. A nil broker leaves
// the broker barrier unchanged. Quantity caps resolve by underlying asset and
// notional caps by settlement asset. An absent cap constrains nothing. Within
// the account+asset then asset chain for a metric, a matching barrier without
// that cap is skipped. Broker caps apply to every order in addition to those
// chains. An explicitly set zero cap rejects positive metric values and admits
// a value of exactly zero.
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
				quantityOptionalToNative(brokerValue.Limit.MaxQuantity),
				volumeOptionalToNative(brokerValue.Limit.MaxNotional),
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
					quantityOptionalToNative(a.Limit.MaxQuantity),
					volumeOptionalToNative(a.Limit.MaxNotional),
				),
				a.Asset.Handle(),
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
						quantityOptionalToNative(a.Limit.MaxQuantity),
						volumeOptionalToNative(a.Limit.MaxNotional),
					),
					a.AccountID.Handle(),
					a.Asset.Handle(),
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
	runtime.KeepAlive(assets)
	runtime.KeepAlive(accountAssets)
	if configErr != nil {
		return newErrorFromHandle(configErr)
	}
	return nil
}

//------------------------------------------------------------------------------
// ReviewRule

// ReviewRule updates the named review-rule policy atomically.
//
// An absent global option keeps the global rule. A present nil clears it, and
// a present rule sets it. A nil groups or accounts slice keeps that level; an
// empty non-nil slice clears it. Other slices replace their whole level.
// Returns a *Error on a domain error.
func (c Configurator) ReviewRule(
	name string,
	global optional.Option[*policies.ReviewRuleGlobalRule],
	groups []policies.ReviewRuleAccountGroupRule,
	accounts []policies.ReviewRuleAccountRule,
) error {
	var nativeGlobal *native.ReviewRuleGlobalRule
	if rule, hasGlobal := global.Get(); hasGlobal && rule != nil {
		nativeGlobal = ptr.New(rule.Handle())
	}
	var nativeGroups []native.ReviewRuleAccountGroupRule
	if groups != nil {
		nativeGroups = make([]native.ReviewRuleAccountGroupRule, len(groups))
		for i, rule := range groups {
			nativeGroups[i] = rule.Handle()
		}
	}
	var nativeAccounts []native.ReviewRuleAccountRule
	if accounts != nil {
		nativeAccounts = make([]native.ReviewRuleAccountRule, len(accounts))
		for i, rule := range accounts {
			nativeAccounts[i] = rule.Handle()
		}
	}
	configErr := native.EngineConfigureReviewRule(
		c.engine, name, nativeGlobal, global.IsSet(), nativeGroups, nativeAccounts,
	)
	runtime.KeepAlive(global)
	runtime.KeepAlive(groups)
	runtime.KeepAlive(accounts)
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
// ReviewWithHold requests review on a shortfall and records the full hold;
// ReviewWithoutHold requests review without a hold. With sufficient funds,
// both review modes record a normal hold without a review mark.
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

// SpotFundsPositionLimit pins, replaces, or clears the inclusive long and short
// position magnitude limit for one account, asset and mode in the named
// SpotFunds policy. The checked position starts with recorded available + held,
// including a negative held residual. A long projection adds open positive
// incoming; a short projection subtracts open positive held. Only the side the
// order moves toward is checked, so a position-reducing order passes while that
// projection stays within the limit. A Reject breach rejects with
// PositionLimitExceeded at Order scope. A review breach adds a
// PositionLimitExceeded review mark with the limit's mode. A projection that
// cannot be computed exactly rejects with ArithmeticOverflow at Order scope in
// every mode. A change applies from the next order and does not re-evaluate
// open reservations.
//
// optional.Some pins or replaces this mode's limit; optional.None clears only
// this mode's limit. Other modes' limits are unchanged. The modes are
// RuleModeReject, RuleModeReviewWithHold and RuleModeReviewWithoutHold.
// Zero means Reject; unknown modes return an Error with ErrorKindValidation.
//
// Returns a *Error on a domain error.
func (c Configurator) SpotFundsPositionLimit(
	name string,
	accountID param.AccountID,
	asset param.Asset,
	mode reject.RuleMode,
	limit optional.Option[param.Quantity],
) error {
	var nativeLimit native.ParamQuantity
	value, hasLimit := limit.Get()
	if hasLimit {
		nativeLimit = value.Handle()
	}
	configErr, err := native.EngineConfigureSpotFundsPositionLimit(
		c.engine,
		name,
		accountID.Handle(),
		asset.Handle(),
		native.PretradeRuleMode(mode),
		nativeLimit,
		hasLimit,
	)
	runtime.KeepAlive(asset)
	if err != nil {
		return &Error{Kind: ErrorKindValidation, Message: err.Error()}
	}
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

// SpotFundsPnlBoundsKillSwitch updates the realized account P&L bounds of
// the named spot-funds policy.
//
// globalBarriers uses three states: nil leaves the global slot unchanged,
// a non-nil empty slice clears it, and a list replaces every mode.
// Nil slices leave their tiers untouched; non-nil empty slices clear them.
// Barrier updates preserve live accumulated P&L.
// With a known effective account currency, an account-tier mismatch fails
// closed and blocks the account; account-group and global mismatches are
// skipped. A known-currency account with no account barrier and no matching
// fallback has no effective barrier, but its P&L keeps accumulating and
// publishing. Without an effective currency, the first in-scope barrier
// applies. Bounds are compared as stored and are never FX-converted.
//
// An account whose effective barrier changed is evaluated against its stored
// account P&L before this call returns: an already halted account, or one
// beyond a new Reject-mode barrier, is blocked here rather than at its next
// fill, and appears in the returned AccountBlocks. Each outcome identifies the
// affected account together with its newly inserted block. Removing the last
// effective barrier reports no block and does not release an existing block.
// Clearing an override can expose a fallback barrier; the fallback is evaluated
// normally and may record and report a block.
//
// Review-mode breaches yield review marks during evaluation without blocking.
// Evaluation failures block in every mode. Returns a *Error on a domain error.
// Invalid or repeated (key, mode) entries return a *Error with
// ErrorKindValidation and leave settings unchanged.
func (c Configurator) SpotFundsPnlBoundsKillSwitch(
	name string,
	globalBarriers []policies.SpotFundsPnlBoundsBarrier,
	accountGroupBarriers []policies.SpotFundsPnlBoundsAccountGroupBarrier,
	accountBarriers []policies.SpotFundsPnlBoundsAccountBarrier,
) (AccountBlockOutcomes, error) {
	nativeGlobal, nativeAccountGroups, nativeAccounts, err := nativeSpotFundsPnlBoundsTiers(
		globalBarriers, accountGroupBarriers, accountBarriers,
	)
	if err != nil {
		return AccountBlockOutcomes{}, &Error{Kind: ErrorKindValidation, Message: err.Error()}
	}
	blocks, configErr := native.EngineConfigureSpotFundsPnlBoundsKillSwitch(
		c.engine,
		name,
		nativeGlobal,
		nativeAccountGroups,
		nativeAccounts,
	)
	runtime.KeepAlive(globalBarriers)
	runtime.KeepAlive(accountGroupBarriers)
	runtime.KeepAlive(accountBarriers)
	if configErr != nil {
		return AccountBlockOutcomes{}, newErrorFromHandle(configErr)
	}
	return newAccountBlockOutcomes(blocks), nil
}

// SpotFundsPnlBoundsAxis retunes one P&L-bounds measure of the named spot-funds
// policy: account unrealized, account total, position unrealized, or position
// total P&L. Only the selected axis changes; realized bounds and P&L stay
// unchanged.
//
// globalBarriers uses three states: nil leaves the global slot unchanged,
// a non-nil empty slice clears it, and a list replaces every mode.
// Nil slices leave their tiers untouched; non-nil empty slices clear them.
// The currency rules of SpotFundsPnlBoundsBarrier apply.
//
// With an effective barrier, a Reject-mode breach or a halt in any mode blocks
// the account with PnlKillSwitchTriggered at pre-trade, dry run, a fill, the
// account P&L force-set, and a retune that changes the effective barrier. A
// fill checks axes for a trade or non-zero fee when the report gave no block of
// its own. A change of an account's group membership does not check the axes;
// the next order catches the breach. A review-mode breach yields a review mark
// without blocking.
//
// Accounts whose effective barrier changes are evaluated before return. Each
// returned outcome pairs the affected account with its recorded block. Removing
// the last effective barrier reports no block and does not release an existing
// block. Clearing an override can expose a lower-tier barrier that is evaluated
// and may record and report a block.
//
// Returns a *Error on a domain error. An unknown axis, invalid mode, or
// repeated (key, mode) returns ErrorKindValidation. On any error, settings
// stay unchanged.
func (c Configurator) SpotFundsPnlBoundsAxis(
	name string,
	axis policies.SpotFundsPnlBoundsAxis,
	globalBarriers []policies.SpotFundsPnlBoundsBarrier,
	accountGroupBarriers []policies.SpotFundsPnlBoundsAccountGroupBarrier,
	accountBarriers []policies.SpotFundsPnlBoundsAccountBarrier,
) (AccountBlockOutcomes, error) {
	nativeGlobal, nativeAccountGroups, nativeAccounts, err := nativeSpotFundsPnlBoundsTiers(
		globalBarriers, accountGroupBarriers, accountBarriers,
	)
	if err != nil {
		return AccountBlockOutcomes{}, &Error{Kind: ErrorKindValidation, Message: err.Error()}
	}
	blocks, configErr := native.EngineConfigureSpotFundsPnlBoundsAxis(
		c.engine,
		name,
		axis.Handle(),
		nativeGlobal,
		nativeAccountGroups,
		nativeAccounts,
	)
	runtime.KeepAlive(globalBarriers)
	runtime.KeepAlive(accountGroupBarriers)
	runtime.KeepAlive(accountBarriers)
	if configErr != nil {
		return AccountBlockOutcomes{}, newErrorFromHandle(configErr)
	}
	return newAccountBlockOutcomes(blocks), nil
}

func nativeSpotFundsPnlBoundsTiers(
	globalBarriers []policies.SpotFundsPnlBoundsBarrier,
	accountGroupBarriers []policies.SpotFundsPnlBoundsAccountGroupBarrier,
	accountBarriers []policies.SpotFundsPnlBoundsAccountBarrier,
) (
	[]native.PretradePoliciesSpotFundsPnlBoundsBarrier,
	[]native.PretradePoliciesSpotFundsPnlBoundsAccountGroupBarrier,
	[]native.PretradePoliciesSpotFundsPnlBoundsAccountBarrier,
	error,
) {
	var nativeGlobal []native.PretradePoliciesSpotFundsPnlBoundsBarrier
	if globalBarriers != nil {
		nativeGlobal = make([]native.PretradePoliciesSpotFundsPnlBoundsBarrier, 0, len(globalBarriers))
		for index, barrier := range globalBarriers {
			nativeBarrier, err := nativeSpotFundsPnlBoundsBarrier(barrier, "global", index)
			if err != nil {
				return nil, nil, nil, err
			}
			nativeGlobal = append(nativeGlobal, nativeBarrier)
		}
	}

	var nativeAccountGroups []native.PretradePoliciesSpotFundsPnlBoundsAccountGroupBarrier
	if accountGroupBarriers != nil {
		nativeAccountGroups = make(
			[]native.PretradePoliciesSpotFundsPnlBoundsAccountGroupBarrier,
			0,
			len(accountGroupBarriers),
		)
		for index, b := range accountGroupBarriers {
			barrier, err := nativeSpotFundsPnlBoundsBarrier(b.Barrier, "account_group", index)
			if err != nil {
				return nil, nil, nil, err
			}
			nativeAccountGroups = append(
				nativeAccountGroups,
				native.NewPretradePoliciesSpotFundsPnlBoundsAccountGroupBarrier(
					b.AccountGroupID.Handle(),
					barrier,
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
		for index, b := range accountBarriers {
			barrier, err := nativeSpotFundsPnlBoundsBarrier(b.Barrier, "account", index)
			if err != nil {
				return nil, nil, nil, err
			}
			nativeAccounts = append(
				nativeAccounts,
				native.NewPretradePoliciesSpotFundsPnlBoundsAccountBarrier(
					b.AccountID.Handle(),
					barrier,
				),
			)
		}
	}

	return nativeGlobal, nativeAccountGroups, nativeAccounts, nil
}

// AccountBlockOutcome is a block inserted for an account selected by the
// engine.
type AccountBlockOutcome struct {
	// AccountID identifies the account for which the engine inserted Block.
	AccountID param.AccountID
	// Block is the account block inserted into engine state.
	Block reject.AccountBlock
}

// AccountBlockOutcomes contains newly inserted blocks for accounts selected by
// the engine.
type AccountBlockOutcomes struct {
	// AccountBlocks pairs every newly inserted block with its affected account.
	AccountBlocks []AccountBlockOutcome
}

// newAccountBlockOutcomes copies a caller-owned native outcome list into the
// Go result and releases it.
func newAccountBlockOutcomes(
	outcomes native.PretradeAccountBlockOutcomeList,
) AccountBlockOutcomes {
	defer native.DestroyPretradeAccountBlockOutcomeList(outcomes)
	accountBlocks := make(
		[]AccountBlockOutcome,
		native.PretradeAccountBlockOutcomeListLen(outcomes),
	)
	for index := range accountBlocks {
		outcome := native.PretradeAccountBlockOutcomeListGet(outcomes, index)
		accountBlocks[index] = AccountBlockOutcome{
			AccountID: param.NewAccountIDFromHandle(
				native.PretradeAccountBlockOutcomeGetAccountID(outcome),
			),
			Block: reject.NewAccountBlockFromHandle(
				native.PretradeAccountBlockOutcomeGetBlock(outcome),
			),
		}
	}
	return AccountBlockOutcomes{AccountBlocks: accountBlocks}
}

// SetSpotFundsAccountPnl force-sets the live accumulated account P&L state for
// one account entry of the named spot-funds policy.
//
// This is an absolute assignment (upsert). It is distinct from
// [Configurator.SpotFundsPnlBoundsKillSwitch], which retunes bounds and never
// touches accumulated P&L.
//
// On success it returns PolicyConfigurationResult. A numeric value beyond an
// effective Reject-mode bound returns the policy-reported block. A breach of a
// review-mode bound returns no block and no review mark: configuration does not
// issue review marks. A halted state under an effective barrier of any mode
// returns the block. Either block is returned even when the account is already
// blocked. The engine processes the block request before returning and
// preserves the existing first cause. A configuration failure returns the
// existing configure error.
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
	return newPolicyConfigurationResult(blocks), nil
}

// newPolicyConfigurationResult copies a caller-owned native account-block list
// into the Go result and releases it.
func newPolicyConfigurationResult(
	blocks native.PretradeAccountBlockList,
) PolicyConfigurationResult {
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
	return PolicyConfigurationResult{AccountBlocks: accountBlocks}
}

// PolicyConfigurationResult describes an accepted runtime policy update.
//
// AccountBlocks contains policy-reported blocks for the account supplied to the
// operation, without repeating that identity. SetSpotFundsAccountPnl exposes a
// breach or halt block even when the account already has a block; the engine
// processes it before returning and preserves the existing first cause.
type PolicyConfigurationResult struct {
	AccountBlocks []reject.AccountBlock
}

//------------------------------------------------------------------------------
// Helpers

func quantityOptionalToNative(
	value optional.Option[param.Quantity],
) native.ParamQuantityOptional {
	if v, has := value.Get(); has {
		return native.NewParamQuantityOptional(v.Handle())
	}
	return native.ParamQuantityOptional{}
}

func volumeOptionalToNative(
	value optional.Option[param.Volume],
) native.ParamVolumeOptional {
	if v, has := value.Get(); has {
		return native.NewParamVolumeOptional(v.Handle())
	}
	return native.ParamVolumeOptional{}
}

func pnlOptionalToNative(value optional.Option[param.Pnl]) native.ParamPnlOptional {
	if v, has := value.Get(); has {
		return native.NewParamPnlOptional(v.Handle())
	}
	return native.ParamPnlOptional{}
}

func nativeSpotFundsPnlBoundsBarrier(
	barrier policies.SpotFundsPnlBoundsBarrier,
	label string,
	index int,
) (native.PretradePoliciesSpotFundsPnlBoundsBarrier, error) {
	return native.NewPretradePoliciesSpotFundsPnlBoundsBarrier(
		native.PretradeRuleMode(barrier.Mode),
		label,
		index,
		barrier.Currency.Handle(),
		pnlOptionalToNative(barrier.LowerBound),
		pnlOptionalToNative(barrier.UpperBound),
	)
}
