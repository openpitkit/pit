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

import (
	"unsafe"
)

//------------------------------------------------------------------------------
// BuiltinRateLimitPolicy

func NewPretradePoliciesRateLimitBrokerBarrier(
	maxOrders uint,
	windowNanoseconds uint64,
) PretradePoliciesRateLimitBrokerBarrier {
	return PretradePoliciesRateLimitBrokerBarrier{
		max_orders:         C.size_t(maxOrders),
		window_nanoseconds: C.uint64_t(windowNanoseconds),
	}
}

func NewPretradePoliciesRateLimitAssetBarrier(
	maxOrders uint,
	windowNanoseconds uint64,
	settlementAsset string,
) PretradePoliciesRateLimitAssetBarrier {
	return PretradePoliciesRateLimitAssetBarrier{
		max_orders:         C.size_t(maxOrders),
		window_nanoseconds: C.uint64_t(windowNanoseconds),
		settlement_asset:   importString(settlementAsset),
	}
}

func NewPretradePoliciesRateLimitAccountBarrier(
	accountID ParamAccountID,
	maxOrders uint,
	windowNanoseconds uint64,
) PretradePoliciesRateLimitAccountBarrier {
	return PretradePoliciesRateLimitAccountBarrier{
		account_id:         accountID,
		max_orders:         C.size_t(maxOrders),
		window_nanoseconds: C.uint64_t(windowNanoseconds),
	}
}

func NewPretradePoliciesRateLimitAccountAssetBarrier(
	accountID ParamAccountID,
	maxOrders uint,
	windowNanoseconds uint64,
	settlementAsset string,
) PretradePoliciesRateLimitAccountAssetBarrier {
	return PretradePoliciesRateLimitAccountAssetBarrier{
		account_id:         accountID,
		max_orders:         C.size_t(maxOrders),
		window_nanoseconds: C.uint64_t(windowNanoseconds),
		settlement_asset:   importString(settlementAsset),
	}
}

//------------------------------------------------------------------------------
// BuiltinOrderSizeLimitPolicy

func NewPretradePoliciesOrderSizeLimit(
	maxQuantity ParamQuantity,
	maxNotional ParamVolume,
) PretradePoliciesOrderSizeLimit {
	return PretradePoliciesOrderSizeLimit{
		max_quantity: maxQuantity,
		max_notional: maxNotional,
	}
}

func NewPretradePoliciesOrderSizeBrokerBarrier(
	limit PretradePoliciesOrderSizeLimit,
) PretradePoliciesOrderSizeBrokerBarrier {
	return PretradePoliciesOrderSizeBrokerBarrier{limit: limit}
}

func NewPretradePoliciesOrderSizeAssetBarrier(
	limit PretradePoliciesOrderSizeLimit,
	settlementAsset string,
) PretradePoliciesOrderSizeAssetBarrier {
	return PretradePoliciesOrderSizeAssetBarrier{
		limit:            limit,
		settlement_asset: importString(settlementAsset),
	}
}

func NewPretradePoliciesOrderSizeAccountAssetBarrier(
	limit PretradePoliciesOrderSizeLimit,
	accountID ParamAccountID,
	settlementAsset string,
) PretradePoliciesOrderSizeAccountAssetBarrier {
	return PretradePoliciesOrderSizeAccountAssetBarrier{
		limit:            limit,
		account_id:       accountID,
		settlement_asset: importString(settlementAsset),
	}
}

//------------------------------------------------------------------------------
// BuiltinPnlBoundsKillswitchPolicy

func NewParamPnlOptional(value ParamPnl) ParamPnlOptional {
	var out ParamPnlOptional
	out.value = value
	out.is_set = true
	return out
}

func NewPretradePoliciesPnlBoundsBarrier(
	settlementAsset string,
	lowerBound ParamPnlOptional,
	upperBound ParamPnlOptional,
) PretradePoliciesPnlBoundsBarrier {
	return PretradePoliciesPnlBoundsBarrier{
		settlement_asset: importString(settlementAsset),
		lower_bound:      lowerBound,
		upper_bound:      upperBound,
	}
}

func NewPretradePoliciesPnlBoundsAccountBarrier(
	accountID ParamAccountID,
	settlementAsset string,
	lowerBound ParamPnlOptional,
	upperBound ParamPnlOptional,
	initialPnl ParamPnl,
) PretradePoliciesPnlBoundsAccountBarrier {
	return PretradePoliciesPnlBoundsAccountBarrier{
		account_id:       accountID,
		settlement_asset: importString(settlementAsset),
		lower_bound:      lowerBound,
		upper_bound:      upperBound,
		initial_pnl:      initialPnl,
	}
}

//------------------------------------------------------------------------------
// PreTradePolicy

func CreatePretradeCustomPreTradePolicy(
	name string,
	groupID PolicyGroupID,
	checkPreTradeStartFnAddr unsafe.Pointer,
	performPreTradeCheckFnAddr unsafe.Pointer,
	applyExecutionReportFnAddr unsafe.Pointer,
	applyAccountAdjustmentFnAddr unsafe.Pointer,
	freeUserDataFnAddr unsafe.Pointer,
	userData unsafe.Pointer,
) (PretradePreTradePolicy, error) {
	var outError SharedString
	p := C.openpit_create_pretrade_custom_pre_trade_policy(
		importString(name),
		groupID,
		*(*C.OpenPitPretradePreTradePolicyCheckPreTradeStartFn)(checkPreTradeStartFnAddr),
		*(*C.OpenPitPretradePreTradePolicyPerformPreTradeCheckFn)(performPreTradeCheckFnAddr),
		*(*C.OpenPitPretradePreTradePolicyApplyExecutionReportFn)(applyExecutionReportFnAddr),
		*(*C.OpenPitPretradePreTradePolicyApplyAccountAdjustmentFn)(applyAccountAdjustmentFnAddr),
		*(*C.OpenPitPretradePreTradePolicyFreeUserDataFn)(freeUserDataFnAddr),
		userData,
		C.OpenPitOutError(&outError), //nolint:gocritic
	)
	if p == nil {
		return nil,
			consumeSharedStringAsError(outError, "openpit_create_pretrade_custom_pre_trade_policy failed")
	}
	return p, nil
}

func DestroyPretradePreTradePolicy(policy PretradePreTradePolicy) {
	C.openpit_destroy_pretrade_pre_trade_policy(policy)
}

func PretradePreTradePolicyGetName(policy PretradePreTradePolicy) StringView {
	return newStringView(C.openpit_pretrade_pre_trade_policy_get_name(policy))
}

//------------------------------------------------------------------------------
