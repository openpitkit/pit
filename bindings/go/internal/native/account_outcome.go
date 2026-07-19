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

//------------------------------------------------------------------------------
// PostTradeAccountPnlList

func PostTradeAccountPnlListPush(
	list PostTradeAccountPnlList,
	outcome AccountPnlOutcome,
) error {
	var outError SharedString
	if !C.openpit_pretrade_post_trade_account_pnl_list_push(
		list,
		outcome,
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	) {
		return consumeSharedStringAsError(
			outError,
			"openpit_pretrade_post_trade_account_pnl_list_push failed",
		)
	}
	return nil
}

//------------------------------------------------------------------------------
// PretradeAccountAdjustmentResult

func PretradeAccountAdjustmentResultPushAccountOutcome(
	result PretradeAccountAdjustmentResult,
	entry AccountOutcomeEntry,
) error {
	var outError SharedString
	if !C.openpit_pretrade_account_adjustment_result_push_account_outcome(
		result,
		entry,
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	) {
		return consumeSharedStringAsError(
			outError,
			"openpit_pretrade_account_adjustment_result_push_account_outcome failed",
		)
	}
	return nil
}

func PretradeAccountAdjustmentResultPushAccountBlock(
	result PretradeAccountAdjustmentResult,
	block PretradeAccountBlock,
) {
	C.openpit_pretrade_account_adjustment_result_push_account_block(result, block)
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

func NewAccountAdjustmentOutcome(
	policyGroupID PolicyGroupID,
	entry AccountOutcomeEntry,
) AccountAdjustmentOutcome {
	return AccountAdjustmentOutcome{
		policy_group_id: policyGroupID,
		entry:           entry,
	}
}

func DestroyAccountAdjustmentOutcomeList(list AccountAdjustmentOutcomeList) {
	C.openpit_destroy_account_adjustment_outcome_list(list)
}

//------------------------------------------------------------------------------
// AccountPnlOutcomeList

func AccountPnlOutcomeListLen(list AccountPnlOutcomeList) int {
	return int(C.openpit_account_pnl_outcome_list_len(list))
}

func AccountPnlOutcomeListGet(list AccountPnlOutcomeList, index int) AccountPnlOutcome {
	var out AccountPnlOutcome
	if !C.openpit_account_pnl_outcome_list_get(list, C.size_t(index), &out) {
		return AccountPnlOutcome{}
	}
	return out
}

func AccountPnlOutcomeGetAccountID(outcome AccountPnlOutcome) ParamAccountID {
	return outcome.account_id
}

func AccountPnlOutcomeGetPolicyGroupID(outcome AccountPnlOutcome) PolicyGroupID {
	return outcome.policy_group_id
}

func AccountPnlOutcomeGetHaltReason(outcome AccountPnlOutcome) PnlHaltReason {
	return outcome.halt_reason
}

func AccountPnlOutcomeGetAmount(outcome AccountPnlOutcome) PnlOutcomeAmount {
	return outcome.amount.value
}

func AccountPnlOutcomeGetPnlOutcome(outcome AccountPnlOutcome) PnlOutcome {
	return PnlOutcome{
		halt_reason: outcome.halt_reason,
		amount:      outcome.amount,
	}
}

func NewAccountPnlOutcome(
	accountID ParamAccountID,
	policyGroupID PolicyGroupID,
	pnl PnlOutcome,
) AccountPnlOutcome {
	return AccountPnlOutcome{
		account_id:      accountID,
		policy_group_id: policyGroupID,
		halt_reason:     pnl.halt_reason,
		amount:          pnl.amount,
	}
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

func NewPnlOutcomeAmount(delta, absolute ParamPnl) PnlOutcomeAmount {
	return PnlOutcomeAmount{delta: delta, absolute: absolute}
}

func PnlOutcomeAmountGetDelta(amount PnlOutcomeAmount) ParamPnl {
	return amount.delta
}

func PnlOutcomeAmountGetAbsolute(amount PnlOutcomeAmount) ParamPnl {
	return amount.absolute
}

func NewPnlOutcomeAmountOptionalSet(value PnlOutcomeAmount) PnlOutcomeAmountOptional {
	return PnlOutcomeAmountOptional{value: value, is_set: true}
}

func NewPnlOutcomeAmountOptionalUnset() PnlOutcomeAmountOptional {
	return PnlOutcomeAmountOptional{}
}

func PnlOutcomeAmountOptionalIsSet(optional PnlOutcomeAmountOptional) bool {
	return bool(optional.is_set)
}

func PnlOutcomeAmountOptionalGet(optional PnlOutcomeAmountOptional) PnlOutcomeAmount {
	return optional.value
}

func NewPnlOutcome(
	haltReason PnlHaltReason,
	amount PnlOutcomeAmountOptional,
) PnlOutcome {
	return PnlOutcome{halt_reason: haltReason, amount: amount}
}

func PnlOutcomeGetHaltReason(outcome PnlOutcome) PnlHaltReason {
	return outcome.halt_reason
}

func PnlOutcomeGetAmount(outcome PnlOutcome) PnlOutcomeAmount {
	return outcome.amount.value
}

func NewPnlOutcomeOptionalSet(value PnlOutcome) PnlOutcomeOptional {
	return PnlOutcomeOptional{value: value, is_set: true}
}

func NewPnlOutcomeOptionalUnset() PnlOutcomeOptional {
	return PnlOutcomeOptional{}
}

func PnlOutcomeOptionalIsSet(optional PnlOutcomeOptional) bool {
	return bool(optional.is_set)
}

func PnlOutcomeOptionalGet(optional PnlOutcomeOptional) PnlOutcome {
	return optional.value
}

func NewAccountOutcomeEntry(
	asset StringView,
	balance, held, incoming OutcomeAmountOptional,
	realizedPnl PnlOutcomeOptional,
	averageEntryPrice ParamPriceOptional,
) AccountOutcomeEntry {
	return AccountOutcomeEntry{
		asset:               asset.value,
		balance:             balance,
		held:                held,
		incoming:            incoming,
		realized_pnl:        realizedPnl,
		average_entry_price: averageEntryPrice,
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

func AccountOutcomeEntryGetRealizedPnl(entry AccountOutcomeEntry) PnlOutcomeOptional {
	return entry.realized_pnl
}

func AccountOutcomeEntryGetAverageEntryPrice(entry AccountOutcomeEntry) ParamPriceOptional {
	return entry.average_entry_price
}

func AccountAdjustmentOutcomeGetPolicyGroupID(outcome AccountAdjustmentOutcome) PolicyGroupID {
	return outcome.policy_group_id
}

func AccountAdjustmentOutcomeGetEntry(outcome AccountAdjustmentOutcome) AccountOutcomeEntry {
	return outcome.entry
}

//------------------------------------------------------------------------------
