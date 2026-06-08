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

//------------------------------------------------------------------------------
// PretradePreTradeResult

func PretradePreTradeResultPushLockPrice(
	result PretradePreTradeResult,
	price ParamPrice,
) error {
	var outError SharedString
	if !C.openpit_pretrade_pre_trade_result_push_lock_price(
		result,
		price,
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	) {
		return consumeSharedStringAsError(
			outError,
			"openpit_pretrade_pre_trade_result_push_lock_price failed",
		)
	}
	return nil
}

func PretradePreTradeResultPushAccountAdjustment(
	result PretradePreTradeResult,
	entry AccountOutcomeEntry,
) error {
	var outError SharedString
	if !C.openpit_pretrade_pre_trade_result_push_account_adjustment(
		result,
		entry,
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	) {
		return consumeSharedStringAsError(
			outError,
			"openpit_pretrade_pre_trade_result_push_account_adjustment failed",
		)
	}
	return nil
}

func DestroyPretradePreTradeResult(result PretradePreTradeResult) {
	C.openpit_destroy_pretrade_pre_trade_result(result)
}

//------------------------------------------------------------------------------
// PostTradeAdjustmentList

func PostTradeAdjustmentListPush(
	list PostTradeAdjustmentList,
	groupID PolicyGroupID,
	entry AccountOutcomeEntry,
) error {
	var outError SharedString
	if !C.openpit_pretrade_post_trade_adjustment_list_push(
		list,
		groupID,
		entry,
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	) {
		return consumeSharedStringAsError(
			outError,
			"openpit_pretrade_post_trade_adjustment_list_push failed",
		)
	}
	return nil
}

func DestroyPostTradeAdjustmentList(list PostTradeAdjustmentList) {
	C.openpit_destroy_post_trade_adjustment_list(list)
}

//------------------------------------------------------------------------------
// AccountOutcomeEntryList

func AccountOutcomeEntryListPush(
	list AccountOutcomeEntryList,
	entry AccountOutcomeEntry,
) error {
	var outError SharedString
	if !C.openpit_account_outcome_entry_list_push(
		list,
		entry,
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	) {
		return consumeSharedStringAsError(
			outError,
			"openpit_account_outcome_entry_list_push failed",
		)
	}
	return nil
}

func DestroyAccountOutcomeEntryList(list AccountOutcomeEntryList) {
	C.openpit_destroy_account_outcome_entry_list(list)
}

//------------------------------------------------------------------------------
// AccountAdjustmentOutcomeList

func AccountAdjustmentOutcomeListLen(list AccountAdjustmentOutcomeList) int {
	return int(C.openpit_account_adjustment_outcome_list_len(list))
}

func AccountAdjustmentOutcomeListGet(
	list AccountAdjustmentOutcomeList,
	index int,
) AccountAdjustmentOutcome {
	var out AccountAdjustmentOutcome
	if !C.openpit_account_adjustment_outcome_list_get(list, C.size_t(index), &out) { //nolint:gocritic // CGo out-parameter requires address-of operator
		return AccountAdjustmentOutcome{}
	}
	return out
}

func DestroyAccountAdjustmentOutcomeList(list AccountAdjustmentOutcomeList) {
	C.openpit_destroy_account_adjustment_outcome_list(list)
}

//------------------------------------------------------------------------------
// AccountOutcomeEntry / OutcomeAmount accessors

func NewOutcomeAmount(delta, absolute ParamPositionSize) OutcomeAmount {
	return OutcomeAmount{delta: delta, absolute: absolute}
}

func OutcomeAmountGetDelta(amount OutcomeAmount) ParamPositionSize {
	return amount.delta
}

func OutcomeAmountGetAbsolute(amount OutcomeAmount) ParamPositionSize {
	return amount.absolute
}

func NewOutcomeAmountOptionalSet(value OutcomeAmount) OutcomeAmountOptional {
	return OutcomeAmountOptional{value: value, is_set: true}
}

func NewOutcomeAmountOptionalUnset() OutcomeAmountOptional {
	return OutcomeAmountOptional{}
}

func OutcomeAmountOptionalIsSet(optional OutcomeAmountOptional) bool {
	return bool(optional.is_set)
}

func OutcomeAmountOptionalGet(optional OutcomeAmountOptional) OutcomeAmount {
	return optional.value
}

func NewAccountOutcomeEntry(
	asset StringView,
	balance, held, incoming OutcomeAmountOptional,
) AccountOutcomeEntry {
	return AccountOutcomeEntry{
		asset:    asset.value,
		balance:  balance,
		held:     held,
		incoming: incoming,
	}
}

func AccountOutcomeEntryGetAsset(entry AccountOutcomeEntry) StringView {
	return newStringView(entry.asset)
}

func AccountOutcomeEntryGetBalance(entry AccountOutcomeEntry) OutcomeAmountOptional {
	return entry.balance
}

func AccountOutcomeEntryGetHeld(entry AccountOutcomeEntry) OutcomeAmountOptional {
	return entry.held
}

func AccountOutcomeEntryGetIncoming(entry AccountOutcomeEntry) OutcomeAmountOptional {
	return entry.incoming
}

func AccountAdjustmentOutcomeGetPolicyGroupID(outcome AccountAdjustmentOutcome) PolicyGroupID {
	return outcome.policy_group_id
}

func AccountAdjustmentOutcomeGetEntry(outcome AccountAdjustmentOutcome) AccountOutcomeEntry {
	return outcome.entry
}

//------------------------------------------------------------------------------
