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

package native

/*
#include "openpit.h"
*/
import "C"

import (
	"fmt"
	"runtime"
	"unsafe"
)

//------------------------------------------------------------------------------
// ConfigureError

func DestroyConfigureError(err ConfigureError) {
	C.openpit_destroy_configure_error(err)
}

func ConfigureErrorGetKind(err ConfigureError) ConfigureErrorKind {
	return C.openpit_configure_error_get_kind(err)
}

func ConfigureErrorGetMessage(err ConfigureError) string {
	return newStringView(C.openpit_configure_error_get_message(err)).Safe()
}

//------------------------------------------------------------------------------
// Engine configure operations

// EngineConfigureRateLimit reconfigures the named rate-limit policy at runtime.
//
// On success returns nil. On a domain error returns a non-nil ConfigureError
// (caller must release with DestroyConfigureError).
func EngineConfigureRateLimit(
	engine Engine,
	name string,
	brokers []PretradePoliciesRateLimitBrokerBarrier,
	assets []PretradePoliciesRateLimitAssetBarrier,
	accounts []PretradePoliciesRateLimitAccountBarrier,
	accountAssets []PretradePoliciesRateLimitAccountAssetBarrier,
) ConfigureError {
	var brokersPtr *C.OpenPitPretradePoliciesRateLimitBrokerBarrier
	if len(brokers) > 0 {
		brokersPtr = (*C.OpenPitPretradePoliciesRateLimitBrokerBarrier)(unsafe.Pointer(&brokers[0]))
	}
	var assetsPtr *C.OpenPitPretradePoliciesRateLimitAssetBarrier
	if len(assets) > 0 {
		assetsPtr = (*C.OpenPitPretradePoliciesRateLimitAssetBarrier)(unsafe.Pointer(&assets[0]))
	}
	var accountsPtr *C.OpenPitPretradePoliciesRateLimitAccountBarrier
	if len(accounts) > 0 {
		accountsPtr = (*C.OpenPitPretradePoliciesRateLimitAccountBarrier)(unsafe.Pointer(&accounts[0]))
	}
	var accountAssetsPtr *C.OpenPitPretradePoliciesRateLimitAccountAssetBarrier
	if len(accountAssets) > 0 {
		accountAssetsPtr = (*C.OpenPitPretradePoliciesRateLimitAccountAssetBarrier)(
			unsafe.Pointer(&accountAssets[0]),
		)
	}

	var outError ConfigureError
	ok := C.openpit_engine_configure_rate_limit(
		engine,
		importString(name),
		brokersPtr,
		C.size_t(len(brokers)),
		C.bool(brokers != nil),
		assetsPtr,
		C.size_t(len(assets)),
		C.bool(assets != nil),
		accountsPtr,
		C.size_t(len(accounts)),
		C.bool(accounts != nil),
		accountAssetsPtr,
		C.size_t(len(accountAssets)),
		C.bool(accountAssets != nil),
		&outError, //nolint:gocritic // CGo out-parameter requires address-of operator
	)
	runtime.KeepAlive(brokers)
	runtime.KeepAlive(assets)
	runtime.KeepAlive(accounts)
	runtime.KeepAlive(accountAssets)
	if !ok {
		return outError
	}
	return nil
}

// EngineConfigurePnlBoundsKillSwitch reconfigures the named P&L bounds
// kill-switch policy at runtime.
//
// On success returns nil. On a domain error returns a non-nil ConfigureError
// (caller must release with DestroyConfigureError).
func EngineConfigurePnlBoundsKillSwitch(
	engine Engine,
	name string,
	brokerBarriers []PretradePoliciesPnlBoundsBarrier,
	accountBarriers []PretradePoliciesPnlBoundsAccountBarrierUpdate,
) ConfigureError {
	var brokerPtr *C.OpenPitPretradePoliciesPnlBoundsBarrier
	if len(brokerBarriers) > 0 {
		brokerPtr = (*C.OpenPitPretradePoliciesPnlBoundsBarrier)(unsafe.Pointer(&brokerBarriers[0]))
	}
	var accountPtr *C.OpenPitPretradePoliciesPnlBoundsAccountBarrierUpdate
	if len(accountBarriers) > 0 {
		accountPtr = (*C.OpenPitPretradePoliciesPnlBoundsAccountBarrierUpdate)(
			unsafe.Pointer(&accountBarriers[0]),
		)
	}

	var outError ConfigureError
	ok := C.openpit_engine_configure_pnl_bounds_killswitch(
		engine,
		importString(name),
		brokerPtr,
		C.size_t(len(brokerBarriers)),
		C.bool(brokerBarriers != nil),
		accountPtr,
		C.size_t(len(accountBarriers)),
		C.bool(accountBarriers != nil),
		&outError, //nolint:gocritic // CGo out-parameter requires address-of operator
	)
	runtime.KeepAlive(brokerBarriers)
	runtime.KeepAlive(accountBarriers)
	if !ok {
		return outError
	}
	return nil
}

// EngineConfigureOrderSizeLimit reconfigures the named order-size-limit policy
// at runtime.
//
// On success returns nil. On a domain error returns a non-nil ConfigureError
// (caller must release with DestroyConfigureError).
func EngineConfigureOrderSizeLimit(
	engine Engine,
	name string,
	broker *PretradePoliciesOrderSizeBrokerBarrier,
	hasBroker bool,
	assets []PretradePoliciesOrderSizeAssetBarrier,
	accountAssets []PretradePoliciesOrderSizeAccountAssetBarrier,
) ConfigureError {
	var assetsPtr *C.OpenPitPretradePoliciesOrderSizeAssetBarrier
	if len(assets) > 0 {
		assetsPtr = (*C.OpenPitPretradePoliciesOrderSizeAssetBarrier)(unsafe.Pointer(&assets[0]))
	}
	var accountAssetsPtr *C.OpenPitPretradePoliciesOrderSizeAccountAssetBarrier
	if len(accountAssets) > 0 {
		accountAssetsPtr = (*C.OpenPitPretradePoliciesOrderSizeAccountAssetBarrier)(
			unsafe.Pointer(&accountAssets[0]),
		)
	}

	var outError ConfigureError
	ok := C.openpit_engine_configure_order_size_limit(
		engine,
		importString(name),
		broker,
		C.bool(hasBroker),
		assetsPtr,
		C.size_t(len(assets)),
		C.bool(assets != nil),
		accountAssetsPtr,
		C.size_t(len(accountAssets)),
		C.bool(accountAssets != nil),
		&outError, //nolint:gocritic // CGo out-parameter requires address-of operator
	)
	runtime.KeepAlive(assets)
	runtime.KeepAlive(accountAssets)
	if !ok {
		return outError
	}
	return nil
}

// EngineConfigureReviewRule replaces supplied rule levels atomically.
// hasGlobal=false keeps the global rule; true with nil global clears it.
// Nil group/account slices keep the level; non-nil empty slices clear it.
// A non-nil ConfigureError is caller-owned: use DestroyConfigureError.
func EngineConfigureReviewRule(
	engine Engine,
	name string,
	global *ReviewRuleGlobalRule,
	hasGlobal bool,
	groups []ReviewRuleAccountGroupRule,
	accounts []ReviewRuleAccountRule,
) ConfigureError {
	if !hasGlobal {
		global = nil
	}
	args := newReviewRuleArguments(global, groups, accounts)
	defer args.release()
	var outError ConfigureError
	if !C.openpit_engine_configure_review_rule(
		engine, importString(name), args.global, C.bool(hasGlobal),
		args.groups, C.size_t(len(groups)), C.bool(groups != nil),
		args.accounts, C.size_t(len(accounts)), C.bool(accounts != nil),
		&outError, //nolint:gocritic // CGo out-parameter requires address-of operator
	) {
		return outError
	}
	return nil
}

// EngineConfigureSpotFunds reconfigures the named spot-funds policy at runtime.
//
// globalSlippageBps and pricingSource are optional (nil means "do not update").
// instrumentOverrides is optional (nil slice means "do not update").
//
// On success returns nil. On a domain error returns a non-nil ConfigureError
// (caller must release with DestroyConfigureError).
func EngineConfigureSpotFunds(
	engine Engine,
	name string,
	globalSlippageBps *uint16,
	pricingSource *uint8,
	instrumentOverrides []PretradePoliciesSpotFundsOverride,
) ConfigureError {
	var slippage C.uint16_t
	if globalSlippageBps != nil {
		slippage = C.uint16_t(*globalSlippageBps)
	}
	var pricingSourceVal C.uint8_t
	if pricingSource != nil {
		pricingSourceVal = C.uint8_t(*pricingSource)
	}
	var overridesPtr *C.OpenPitPretradePoliciesSpotFundsOverride
	if len(instrumentOverrides) > 0 {
		overridesPtr = (*C.OpenPitPretradePoliciesSpotFundsOverride)(
			unsafe.Pointer(&instrumentOverrides[0]),
		)
	}

	var outError ConfigureError
	ok := C.openpit_engine_configure_spot_funds(
		engine,
		importString(name),
		slippage,
		C.bool(globalSlippageBps != nil),
		pricingSourceVal,
		C.bool(pricingSource != nil),
		overridesPtr,
		C.size_t(len(instrumentOverrides)),
		C.bool(instrumentOverrides != nil),
		&outError, //nolint:gocritic // CGo out-parameter requires address-of operator
	)
	runtime.KeepAlive(instrumentOverrides)
	if !ok {
		return outError
	}
	return nil
}

// EngineConfigureSpotFundsPnlBoundsKillSwitch reconfigures the realized P&L
// bounds of the named spot-funds policy at runtime.
//
// Nil slices leave a tier untouched; non-nil empty slices clear that tier.
//
// On success it returns a caller-owned account-block-outcome list, possibly
// empty. The caller must release it with
// DestroyPretradeAccountBlockOutcomeList.
func EngineConfigureSpotFundsPnlBoundsKillSwitch(
	engine Engine,
	name string,
	globalBarriers []PretradePoliciesSpotFundsPnlBoundsBarrier,
	accountGroupBarriers []PretradePoliciesSpotFundsPnlBoundsAccountGroupBarrier,
	accountBarriers []PretradePoliciesSpotFundsPnlBoundsAccountBarrier,
) (PretradeAccountBlockOutcomeList, ConfigureError) {
	tiers := prepareSpotFundsPnlBoundsTiers(
		globalBarriers, accountGroupBarriers, accountBarriers,
	)
	var outError ConfigureError
	blocks := C.openpit_engine_configure_spot_funds_pnl_bounds_killswitch(
		engine,
		importString(name),
		tiers.global,
		tiers.globalLen,
		tiers.hasGlobal,
		tiers.accountGroup,
		tiers.accountGroupLen,
		tiers.hasAccountGroup,
		tiers.account,
		tiers.accountLen,
		tiers.hasAccount,
		&outError, //nolint:gocritic // CGo out-parameter requires address-of operator
	)
	runtime.KeepAlive(globalBarriers)
	runtime.KeepAlive(accountGroupBarriers)
	runtime.KeepAlive(accountBarriers)
	if blocks == nil {
		return nil, outError
	}
	return blocks, nil
}

// EngineConfigureSpotFundsPnlBoundsAxis retunes only the selected P&L-bounds
// measure of the named spot-funds policy at runtime. Realized bounds and P&L
// stay unchanged. An unknown axis returns a validation error before any change.
//
// Nil slices leave their tiers untouched; non-nil empty slices clear them.
// A non-empty list replaces every mode of its tier.
//
// On success it returns a caller-owned account-block-outcome list, possibly
// empty. The caller must release it with
// DestroyPretradeAccountBlockOutcomeList.
func EngineConfigureSpotFundsPnlBoundsAxis(
	engine Engine,
	name string,
	axis PretradePoliciesSpotFundsPnlBoundsAxis,
	globalBarriers []PretradePoliciesSpotFundsPnlBoundsBarrier,
	accountGroupBarriers []PretradePoliciesSpotFundsPnlBoundsAccountGroupBarrier,
	accountBarriers []PretradePoliciesSpotFundsPnlBoundsAccountBarrier,
) (PretradeAccountBlockOutcomeList, ConfigureError) {
	tiers := prepareSpotFundsPnlBoundsTiers(
		globalBarriers, accountGroupBarriers, accountBarriers,
	)
	var outError ConfigureError
	blocks := C.openpit_engine_configure_spot_funds_pnl_bounds_axis(
		engine,
		importString(name),
		axis,
		tiers.global,
		tiers.globalLen,
		tiers.hasGlobal,
		tiers.accountGroup,
		tiers.accountGroupLen,
		tiers.hasAccountGroup,
		tiers.account,
		tiers.accountLen,
		tiers.hasAccount,
		&outError, //nolint:gocritic // CGo out-parameter requires address-of operator
	)
	runtime.KeepAlive(globalBarriers)
	runtime.KeepAlive(accountGroupBarriers)
	runtime.KeepAlive(accountBarriers)
	if blocks == nil {
		return nil, outError
	}
	return blocks, nil
}

type spotFundsPnlBoundsTiers struct {
	global          *C.OpenPitPretradePoliciesSpotFundsPnlBoundsBarrier
	globalLen       C.size_t
	hasGlobal       C.bool
	accountGroup    *C.OpenPitPretradePoliciesSpotFundsPnlBoundsAccountGroupBarrier
	accountGroupLen C.size_t
	hasAccountGroup C.bool
	account         *C.OpenPitPretradePoliciesSpotFundsPnlBoundsAccountBarrier
	accountLen      C.size_t
	hasAccount      C.bool
}

func prepareSpotFundsPnlBoundsTiers(
	globalBarriers []PretradePoliciesSpotFundsPnlBoundsBarrier,
	accountGroupBarriers []PretradePoliciesSpotFundsPnlBoundsAccountGroupBarrier,
	accountBarriers []PretradePoliciesSpotFundsPnlBoundsAccountBarrier,
) spotFundsPnlBoundsTiers {
	var globalPtr *C.OpenPitPretradePoliciesSpotFundsPnlBoundsBarrier
	if len(globalBarriers) > 0 {
		globalPtr = (*C.OpenPitPretradePoliciesSpotFundsPnlBoundsBarrier)(
			unsafe.Pointer(&globalBarriers[0]),
		)
	}
	var accountGroupPtr *C.OpenPitPretradePoliciesSpotFundsPnlBoundsAccountGroupBarrier
	if len(accountGroupBarriers) > 0 {
		accountGroupPtr = (*C.OpenPitPretradePoliciesSpotFundsPnlBoundsAccountGroupBarrier)(
			unsafe.Pointer(&accountGroupBarriers[0]),
		)
	}
	var accountPtr *C.OpenPitPretradePoliciesSpotFundsPnlBoundsAccountBarrier
	if len(accountBarriers) > 0 {
		accountPtr = (*C.OpenPitPretradePoliciesSpotFundsPnlBoundsAccountBarrier)(
			unsafe.Pointer(&accountBarriers[0]),
		)
	}

	return spotFundsPnlBoundsTiers{
		global:          globalPtr,
		globalLen:       C.size_t(len(globalBarriers)),
		hasGlobal:       C.bool(globalBarriers != nil),
		accountGroup:    accountGroupPtr,
		accountGroupLen: C.size_t(len(accountGroupBarriers)),
		hasAccountGroup: C.bool(accountGroupBarriers != nil),
		account:         accountPtr,
		accountLen:      C.size_t(len(accountBarriers)),
		hasAccount:      C.bool(accountBarriers != nil),
	}
}

// EngineSetSpotFundsAccountPnl force-sets the live accumulated account P&L
// state for one account entry of the named spot-funds policy.
//
// On success it returns a caller-owned account-block list, possibly empty.
// The caller must release it with DestroyPretradeAccountBlockList.
func EngineSetSpotFundsAccountPnl(
	engine Engine,
	name string,
	accountID ParamAccountID,
	state PnlState,
) (PretradeAccountBlockList, ConfigureError) {
	var outError ConfigureError
	blocks := C.openpit_engine_configure_spot_funds_set_account_pnl(
		engine,
		importString(name),
		accountID,
		state,
		&outError, //nolint:gocritic // CGo out-parameter requires address-of operator
	)
	if blocks == nil {
		return nil, outError
	}
	return blocks, nil
}

// EngineConfigureSpotFundsGlobalLimitMode sets the global spot-funds limit mode
// of the named spot-funds policy at runtime.
//
// On success returns nil. On a domain error returns a non-nil ConfigureError
// (caller must release with DestroyConfigureError).
func EngineConfigureSpotFundsGlobalLimitMode(
	engine Engine,
	name string,
	mode PretradePoliciesSpotFundsLimitMode,
) ConfigureError {
	var outError ConfigureError
	if !C.openpit_engine_configure_spot_funds_global_limit_mode(
		engine,
		importString(name),
		mode,
		&outError, //nolint:gocritic // CGo out-parameter requires address-of operator
	) {
		return outError
	}
	return nil
}

// EngineConfigureSpotFundsAccountLimitMode pins or clears the spot-funds limit
// mode for one account of the named spot-funds policy at runtime.
//
// When hasMode is true the account is pinned to mode; when false any existing
// per-account override is cleared and mode is ignored.
//
// On success returns nil. On a domain error returns a non-nil ConfigureError
// (caller must release with DestroyConfigureError).
func EngineConfigureSpotFundsAccountLimitMode(
	engine Engine,
	name string,
	accountID ParamAccountID,
	mode PretradePoliciesSpotFundsLimitMode,
	hasMode bool,
) ConfigureError {
	var outError ConfigureError
	if !C.openpit_engine_configure_spot_funds_account_limit_mode(
		engine,
		importString(name),
		accountID,
		mode,
		C.bool(hasMode),
		&outError, //nolint:gocritic // CGo out-parameter requires address-of operator
	) {
		return outError
	}
	return nil
}

// EngineConfigureSpotFundsPositionLimit pins or clears one limit keyed by
// account, asset and mode. The limit is ignored when hasLimit is false.
// On success returns nil, nil.
// An invalid mode returns a Go error. On a native failure, the caller owns the
// returned ConfigureError and must release it with DestroyConfigureError.
func EngineConfigureSpotFundsPositionLimit(
	engine Engine,
	name string,
	accountID ParamAccountID,
	asset string,
	mode PretradeRuleMode,
	limit ParamQuantity,
	hasLimit bool,
) (ConfigureError, error) {
	cMode, ok := checkedRuleModeToC(mode)
	if !ok {
		return nil, fmt.Errorf("position_limit has invalid mode %d", mode)
	}
	var outError ConfigureError
	if !C.openpit_engine_configure_spot_funds_position_limit(
		engine,
		importString(name),
		accountID,
		importString(asset),
		cMode,
		limit,
		C.bool(hasLimit),
		&outError, //nolint:gocritic // CGo out-parameter requires address-of operator
	) {
		return outError, nil
	}
	return nil, nil //nolint:nilnil // both nil signals success
}

// EngineConfigureSpotFundsAccountGroupLimitMode pins or clears the spot-funds
// limit mode for one account group of the named spot-funds policy at runtime.
//
// When hasMode is true the group is pinned to mode; when false any existing
// per-account-group override is cleared and mode is ignored.
//
// On success returns nil. On a domain error returns a non-nil ConfigureError
// (caller must release with DestroyConfigureError).
func EngineConfigureSpotFundsAccountGroupLimitMode(
	engine Engine,
	name string,
	accountGroupID ParamAccountGroupID,
	mode PretradePoliciesSpotFundsLimitMode,
	hasMode bool,
) ConfigureError {
	var outError ConfigureError
	if !C.openpit_engine_configure_spot_funds_account_group_limit_mode(
		engine,
		importString(name),
		accountGroupID,
		mode,
		C.bool(hasMode),
		&outError, //nolint:gocritic // CGo out-parameter requires address-of operator
	) {
		return outError
	}
	return nil
}

// EngineSetAccountPnl force-sets the live accumulated P&L for one
// (account, settlement asset) entry of the named P&L bounds kill-switch policy.
//
// On success returns nil. On a domain error returns a non-nil ConfigureError
// (caller must release with DestroyConfigureError).
func EngineSetAccountPnl(
	engine Engine,
	name string,
	accountID ParamAccountID,
	settlementAsset string,
	pnl ParamPnl,
) ConfigureError {
	var outError ConfigureError
	if !C.openpit_engine_configure_pnl_bounds_killswitch_set_account_pnl(
		engine,
		importString(name),
		accountID,
		importString(settlementAsset),
		pnl,
		&outError, //nolint:gocritic // CGo out-parameter requires address-of operator
	) {
		return outError
	}
	return nil
}
