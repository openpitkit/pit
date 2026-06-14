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

package native

/*
#include "openpit.h"
*/
import "C"

import "unsafe"

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
	broker *PretradePoliciesRateLimitBrokerBarrier,
	assets []PretradePoliciesRateLimitAssetBarrier,
	accounts []PretradePoliciesRateLimitAccountBarrier,
	accountAssets []PretradePoliciesRateLimitAccountAssetBarrier,
) ConfigureError {
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
	if !C.openpit_engine_configure_rate_limit(
		engine,
		importString(name),
		broker,
		C.bool(broker != nil),
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
	) {
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
	if !C.openpit_engine_configure_pnl_bounds_killswitch(
		engine,
		importString(name),
		brokerPtr,
		C.size_t(len(brokerBarriers)),
		C.bool(brokerBarriers != nil),
		accountPtr,
		C.size_t(len(accountBarriers)),
		C.bool(accountBarriers != nil),
		&outError, //nolint:gocritic // CGo out-parameter requires address-of operator
	) {
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
	if !C.openpit_engine_configure_order_size_limit(
		engine,
		importString(name),
		broker,
		C.bool(broker != nil),
		assetsPtr,
		C.size_t(len(assets)),
		C.bool(assets != nil),
		accountAssetsPtr,
		C.size_t(len(accountAssets)),
		C.bool(accountAssets != nil),
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
	if !C.openpit_engine_configure_spot_funds(
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
	if !C.openpit_engine_configure_set_account_pnl(
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
