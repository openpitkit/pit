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
#include "pit.h"
*/
import "C"

import (
	"errors"
	"unsafe"
)

//------------------------------------------------------------------------------
// OrderValidationPolicy

func CreatePretradePoliciesOrderValidationPolicy() PretradeCheckPreTradeStartPolicy {
	return C.pit_create_pretrade_policies_order_validation_policy()
}

//------------------------------------------------------------------------------
// RateLimitPolicy

func CreatePretradePoliciesRateLimitPolicy(
	maxOrders int,
	windowSeconds uint64,
) PretradeCheckPreTradeStartPolicy {
	return C.pit_create_pretrade_policies_rate_limit_policy(
		C.size_t(maxOrders),
		C.uint64_t(windowSeconds),
	)
}

//------------------------------------------------------------------------------
// PnlKillSwitchPolicy

func NewPretradePoliciesPnlKillSwitchParam(
	settlementAsset string,
	barrier ParamPnl,
) PretradePoliciesPnlKillSwitchParam {
	return PretradePoliciesPnlKillSwitchParam{
		settlement_asset: importString(settlementAsset),
		barrier:          barrier,
	}
}

func CreatePretradePoliciesPnlKillSwitchPolicy(
	params []PretradePoliciesPnlKillSwitchParam,
) (PretradeCheckPreTradeStartPolicy, error) {
	if len(params) == 0 {
		return nil, errors.New("parameter list is empty")
	}

	var outError SharedString
	p := C.pit_create_pretrade_policies_pnl_killswitch_policy(
		(*PretradePoliciesPnlKillSwitchParam)(unsafe.Pointer(&params[0])),
		C.size_t(len(params)),
		C.PitOutError(&outError), //nolint:gocritic
	)
	if p == nil {
		return nil,
			consumeSharedStringAsError(
				outError,
				"pit_create_pretrade_policies_pnl_killswitch_policy failed",
			)
	}
	return p, nil
}

//------------------------------------------------------------------------------
// OrderSizeLimitPolicy

func NewPretradePoliciesOrderSizeLimitParam(
	settlementAsset string,
	maxQuantity ParamQuantity,
	maxNotional ParamVolume,
) PretradePoliciesOrderSizeLimitParam {
	return PretradePoliciesOrderSizeLimitParam{
		settlement_asset: importString(settlementAsset),
		max_quantity:     maxQuantity,
		max_notional:     maxNotional,
	}
}

func CreatePretradePoliciesOrderSizeLimitPolicy(
	params []PretradePoliciesOrderSizeLimitParam,
) (PretradeCheckPreTradeStartPolicy, error) {
	if len(params) == 0 {
		return nil, errors.New("parameter list is empty")
	}

	var outError SharedString
	p := C.pit_create_pretrade_policies_order_size_limit_policy(
		(*PretradePoliciesOrderSizeLimitParam)(unsafe.Pointer(&params[0])),
		C.size_t(len(params)),
		C.PitOutError(&outError), //nolint:gocritic
	)
	if p == nil {
		return nil,
			consumeSharedStringAsError(
				outError,
				"pit_create_pretrade_policies_order_size_limit_policy failed",
			)
	}
	return p, nil
}

//------------------------------------------------------------------------------
// CheckPreTradeStartPolicy

func CreatePretradeCustomCheckPreTradeStartPolicy(
	name string,
	checkFnAddr unsafe.Pointer,
	applyExecutionReportFnAddr unsafe.Pointer,
	freeUserDataFnAddr unsafe.Pointer,
	userData unsafe.Pointer,
) (PretradeCheckPreTradeStartPolicy, error) {
	var outError SharedString
	p := C.pit_create_pretrade_custom_check_pre_trade_start_policy(
		importString(name),
		*(*C.PitPretradeCheckPreTradeStartPolicyCheckPreTradeStartFn)(checkFnAddr),
		*(*C.PitPretradeCheckPreTradeStartPolicyApplyExecutionReportFn)(applyExecutionReportFnAddr),
		*(*C.PitPretradeCheckPreTradeStartPolicyFreeUserDataFn)(freeUserDataFnAddr),
		userData,
		C.PitOutError(&outError), //nolint:gocritic
	)
	if p == nil {
		return nil,
			consumeSharedStringAsError(
				outError,
				"pit_create_pretrade_custom_check_pre_trade_start_policy failed",
			)
	}
	return p, nil
}

func DestroyPretradeCheckPreTradeStartPolicy(policy PretradeCheckPreTradeStartPolicy) {
	C.pit_destroy_pretrade_check_pre_trade_start_policy(policy)
}

func PretradeCheckPreTradeStartPolicyGetName(
	policy PretradeCheckPreTradeStartPolicy,
) StringView {
	return newStringView(C.pit_pretrade_check_pre_trade_start_policy_get_name(policy))
}

//------------------------------------------------------------------------------
// PreTradePolicy

func CreatePretradeCustomPreTradePolicy(
	name string,
	checkFnAddr unsafe.Pointer,
	applyFnAddr unsafe.Pointer,
	freeUserDataFnAddr unsafe.Pointer,
	userData unsafe.Pointer,
) (PretradePreTradePolicy, error) {
	var outError SharedString
	p := C.pit_create_pretrade_custom_pre_trade_policy(
		importString(name),
		*(*C.PitPretradePreTradePolicyCheckFn)(checkFnAddr),
		*(*C.PitPretradePreTradePolicyApplyExecutionReportFn)(applyFnAddr),
		*(*C.PitPretradePreTradePolicyFreeUserDataFn)(freeUserDataFnAddr),
		userData,
		C.PitOutError(&outError), //nolint:gocritic
	)
	if p == nil {
		return nil,
			consumeSharedStringAsError(outError, "pit_create_pretrade_custom_pre_trade_policy failed")
	}
	return p, nil
}

func DestroyPretradePreTradePolicy(policy PretradePreTradePolicy) {
	C.pit_destroy_pretrade_pre_trade_policy(policy)
}

func PretradePreTradePolicyGetName(policy PretradePreTradePolicy) StringView {
	return newStringView(C.pit_pretrade_pre_trade_policy_get_name(policy))
}

//------------------------------------------------------------------------------
// AccountAdjustmentPolicy

func CreateCustomAccountAdjustmentPolicy(
	name string,
	applyFnAddr unsafe.Pointer,
	freeUserDataFnAddr unsafe.Pointer,
	userData unsafe.Pointer,
) (AccountAdjustmentPolicy, error) {
	var outError SharedString
	p := C.pit_create_custom_account_adjustment_policy(
		importString(name),
		*(*C.PitAccountAdjustmentPolicyApplyFn)(applyFnAddr),
		*(*C.PitAccountAdjustmentPolicyFreeUserDataFn)(freeUserDataFnAddr),
		userData,
		C.PitOutError(&outError), //nolint:gocritic
	)
	if p == nil {
		return nil,
			consumeSharedStringAsError(outError, "pit_create_custom_account_adjustment_policy failed")
	}
	return p, nil
}

func DestroyAccountAdjustmentPolicy(policy AccountAdjustmentPolicy) {
	C.pit_destroy_account_adjustment_policy(policy)
}

func AccountAdjustmentPolicyGetName(policy AccountAdjustmentPolicy) StringView {
	return newStringView(C.pit_account_adjustment_policy_get_name(policy))
}

//------------------------------------------------------------------------------
