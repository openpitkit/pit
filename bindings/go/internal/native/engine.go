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

extern const OpenPitPretradeRejectList * openpit_account_adjustment_batch_error_get_rejects(
    const OpenPitAccountAdjustmentBatchError * batch_error
);
*/
import "C"

import (
	"errors"
	"fmt"
	"unsafe"
)

//------------------------------------------------------------------------------
// Engine

type SyncPolicy = C.OpenPitSyncPolicy

const (
	SyncPolicyNone    SyncPolicy = C.OpenPitSyncPolicy_None
	SyncPolicyFull    SyncPolicy = C.OpenPitSyncPolicy_Full
	SyncPolicyAccount SyncPolicy = C.OpenPitSyncPolicy_Account
)

func CreateEngineBuilder(syncPolicy SyncPolicy) (EngineBuilder, error) {
	var outError SharedString
	builder := C.openpit_create_engine_builder(
		syncPolicy,
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	)
	if builder == nil {
		return nil, consumeSharedStringAsError(outError, "openpit_create_engine_builder failed")
	}
	return builder, nil
}

func DestroyEngineBuilder(builder EngineBuilder) {
	C.openpit_destroy_engine_builder(builder)
}

// EngineBuilderBuild constructs the engine from the builder.
//
// On a domain build failure it surfaces a non-nil EngineBuildError handle, which
// the caller owns and must release with DestroyEngineBuildError. On a boundary
// failure it surfaces the error from the string out-error. A null engine is
// treated as failure; on success both the handle and the error are nil.
func EngineBuilderBuild(builder EngineBuilder) (Engine, EngineBuildError, error) {
	var outBuildError EngineBuildError
	var outError SharedString
	e := C.openpit_engine_builder_build(
		builder,
		&outBuildError,
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	)
	if e == nil {
		if outBuildError != nil {
			return nil, outBuildError, nil
		}
		return nil, nil, consumeSharedStringAsError(outError, "openpit_engine_builder_build failed")
	}
	return e, nil, nil
}

func DestroyEngineBuildError(err EngineBuildError) {
	C.openpit_destroy_engine_build_error(err)
}

func EngineBuildErrorGetCode(err EngineBuildError) EngineBuildErrorCode {
	return C.openpit_engine_build_error_get_code(err)
}

func EngineBuildErrorGetPolicyName(err EngineBuildError) string {
	return newStringView(C.openpit_engine_build_error_get_policy_name(err)).Safe()
}

func EngineBuildErrorGetPolicyGroupID(err EngineBuildError) uint16 {
	return uint16(C.openpit_engine_build_error_get_policy_group_id(err))
}

func EngineBuilderAddPreTradePolicy(builder EngineBuilder, policy PretradePreTradePolicy) error {
	var outError SharedString
	if !C.openpit_engine_builder_add_pre_trade_policy(builder, policy, C.OpenPitOutError(&outError)) { //nolint:gocritic // CGo out-parameter requires address-of operator
		return consumeSharedStringAsError(outError, "openpit_engine_builder_add_pre_trade_policy failed")
	}
	return nil
}

func EngineBuilderAddBuiltinOrderValidation(builder EngineBuilder, groupID PolicyGroupID) error {
	var outError SharedString
	if !C.openpit_engine_builder_add_builtin_order_validation_policy(
		builder,
		groupID,
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	) {
		return consumeSharedStringAsError(
			outError,
			"openpit_engine_builder_add_builtin_order_validation_policy failed",
		)
	}
	return nil
}

func EngineBuilderAddBuiltinRateLimit(
	builder EngineBuilder,
	groupID PolicyGroupID,
	broker *PretradePoliciesRateLimitBrokerBarrier,
	assets []PretradePoliciesRateLimitAssetBarrier,
	accounts []PretradePoliciesRateLimitAccountBarrier,
	accountAssets []PretradePoliciesRateLimitAccountAssetBarrier,
) error {
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
		accountAssetsPtr = (*C.OpenPitPretradePoliciesRateLimitAccountAssetBarrier)(unsafe.Pointer(&accountAssets[0]))
	}

	var outError SharedString
	if !C.openpit_engine_builder_add_builtin_rate_limit_policy(
		builder,
		groupID,
		broker,
		assetsPtr,
		C.size_t(len(assets)),
		accountsPtr,
		C.size_t(len(accounts)),
		accountAssetsPtr,
		C.size_t(len(accountAssets)),
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	) {
		return consumeSharedStringAsError(
			outError,
			"openpit_engine_builder_add_builtin_rate_limit_policy failed",
		)
	}
	return nil
}

func EngineBuilderAddBuiltinOrderSizeLimit(
	builder EngineBuilder,
	groupID PolicyGroupID,
	broker *PretradePoliciesOrderSizeBrokerBarrier,
	assets []PretradePoliciesOrderSizeAssetBarrier,
	accountAssets []PretradePoliciesOrderSizeAccountAssetBarrier,
) error {
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

	var outError SharedString
	if !C.openpit_engine_builder_add_builtin_order_size_limit_policy(
		builder,
		groupID,
		broker,
		assetsPtr,
		C.size_t(len(assets)),
		accountAssetsPtr,
		C.size_t(len(accountAssets)),
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	) {
		return consumeSharedStringAsError(
			outError,
			"openpit_engine_builder_add_builtin_order_size_limit_policy failed",
		)
	}
	return nil
}

func EngineBuilderAddBuiltinPnlBoundsKillswitch(
	builder EngineBuilder,
	groupID PolicyGroupID,
	brokerBarriers []PretradePoliciesPnlBoundsBarrier,
	accountBarriers []PretradePoliciesPnlBoundsAccountBarrier,
) error {
	var brokerPtr *C.OpenPitPretradePoliciesPnlBoundsBarrier
	if len(brokerBarriers) > 0 {
		brokerPtr = (*C.OpenPitPretradePoliciesPnlBoundsBarrier)(unsafe.Pointer(&brokerBarriers[0]))
	}
	var accountPtr *C.OpenPitPretradePoliciesPnlBoundsAccountBarrier
	if len(accountBarriers) > 0 {
		accountPtr = (*C.OpenPitPretradePoliciesPnlBoundsAccountBarrier)(
			unsafe.Pointer(&accountBarriers[0]),
		)
	}

	var outError SharedString
	if !C.openpit_engine_builder_add_builtin_pnl_bounds_killswitch_policy(
		builder,
		groupID,
		brokerPtr,
		C.size_t(len(brokerBarriers)),
		accountPtr,
		C.size_t(len(accountBarriers)),
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	) {
		return consumeSharedStringAsError(
			outError,
			"openpit_engine_builder_add_builtin_pnl_bounds_killswitch_policy failed",
		)
	}
	return nil
}

// NewPretradePoliciesSpotFundsOverride builds an override POD. accountID and
// accountGroupID select the scope; both nil means instrument-level. When
// slippageBps is nil the entry is ignored and the cascade falls through to the
// next tier.
func NewPretradePoliciesSpotFundsOverride(
	instrumentID MarketDataInstrumentID,
	accountID *ParamAccountID,
	accountGroupID *ParamAccountGroupID,
	slippageBps *uint16,
) PretradePoliciesSpotFundsOverride {
	override := PretradePoliciesSpotFundsOverride{instrument_id: instrumentID}
	if accountID != nil {
		override.account_id = C.OpenPitParamAccountIdOptional{
			value:  *accountID,
			is_set: C.bool(true),
		}
	}
	if accountGroupID != nil {
		override.account_group_id = C.OpenPitParamAccountGroupIdOptional{
			value:  *accountGroupID,
			is_set: C.bool(true),
		}
	}
	if slippageBps != nil {
		override.slippage_bps = C.uint16_t(*slippageBps)
		override.has_slippage_bps = true
	}
	return override
}

func EngineBuilderAddBuiltinSpotFunds(
	builder EngineBuilder,
	marketData MarketDataService,
	marketSlippageBps *uint16,
	pricingSource uint8,
	instrumentOverrides []PretradePoliciesSpotFundsOverride,
	groupID PolicyGroupID,
) error {
	var slippagePtr *C.uint16_t
	if marketSlippageBps != nil {
		slippageValue := C.uint16_t(*marketSlippageBps)
		slippagePtr = &slippageValue
	}
	var overridesPtr *C.OpenPitPretradePoliciesSpotFundsOverride
	if len(instrumentOverrides) > 0 {
		overridesPtr = (*C.OpenPitPretradePoliciesSpotFundsOverride)(
			unsafe.Pointer(&instrumentOverrides[0]),
		)
	}

	var outError SharedString
	if !C.openpit_engine_builder_add_builtin_spot_funds_policy(
		builder,
		marketData,
		slippagePtr,
		C.uint8_t(pricingSource),
		overridesPtr,
		C.size_t(len(instrumentOverrides)),
		groupID,
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	) {
		return consumeSharedStringAsError(
			outError,
			"openpit_engine_builder_add_builtin_spot_funds_policy failed",
		)
	}
	return nil
}

func DestroyEngine(engine Engine) {
	C.openpit_destroy_engine(engine)
}

func EngineStartPreTrade(
	engine Engine,
	order Order,
) (PretradePreTradeRequest, PretradeRejectList, error) {
	var request PretradePreTradeRequest
	var rejects PretradeRejectList
	var outError SharedString
	status := C.openpit_engine_start_pre_trade(
		engine,
		&order,
		&request,
		&rejects,
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	)

	switch status {
	case C.OpenPitPretradeStatus_Passed:
		return request, nil, nil
	case C.OpenPitPretradeStatus_Rejected:
		if rejects == nil {
			return nil, nil, errors.New("order rejected, but no reject reason provided")
		}
		return nil, rejects, nil
	case C.OpenPitPretradeStatus_Error:
		return nil, nil, consumeSharedStringAsError(outError, "openpit_engine_start_pre_trade failed")
	default:
		DestroyPretradePreTradeRequest(request)
		DestroyPretradeRejectList(rejects)
		DestroySharedString(outError)
		return nil,
			nil,
			fmt.Errorf("openpit_engine_start_pre_trade failed with unexpected status %d", status)
	}
}

func EngineExecutePreTrade(
	engine Engine,
	order Order,
) (PretradePreTradeReservation, PretradeRejectList, error) {
	var reservation PretradePreTradeReservation
	var rejects PretradeRejectList
	var outError SharedString
	status := C.openpit_engine_execute_pre_trade(
		engine,
		&order,
		&reservation,
		&rejects,
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	)

	switch status {
	case C.OpenPitPretradeStatus_Passed:
		return reservation, nil, nil
	case C.OpenPitPretradeStatus_Rejected:
		if rejects == nil {
			return nil, nil, errors.New("order rejected, but no reject reason provided")
		}
		return nil, rejects, nil
	case C.OpenPitPretradeStatus_Error:
		return nil, nil, consumeSharedStringAsError(outError, "openpit_engine_execute_pre_trade failed")
	default:
		DestroyPretradePreTradeReservation(reservation)
		DestroyPretradeRejectList(rejects)
		DestroySharedString(outError)
		return nil,
			nil,
			fmt.Errorf("openpit_engine_execute_pre_trade failed with unexpected status %d", status)
	}
}

type PretradePostTradeResult struct {
	AccountBlocks []PretradeAccountBlock
	// Outcomes is the native account-adjustment outcome list handle produced by
	// policies, or nil. The caller owns it and must release it with
	// DestroyAccountAdjustmentOutcomeList.
	Outcomes AccountAdjustmentOutcomeList
}

func EngineApplyExecutionReport(
	engine Engine,
	report ExecutionReport,
) (PretradePostTradeResult, error) {
	var outError SharedString
	var outBlocks PretradeAccountBlockList
	var outAdjustments AccountAdjustmentOutcomeList
	if !C.openpit_engine_apply_execution_report(
		engine,
		&report,
		&outBlocks,
		&outAdjustments,
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	) {
		return PretradePostTradeResult{},
			consumeSharedStringAsError(outError, "openpit_engine_apply_execution_report failed")
	}

	var blocks []PretradeAccountBlock
	if outBlocks != nil {
		n := PretradeAccountBlockListLen(outBlocks)
		blocks = make([]PretradeAccountBlock, n)
		for i := range blocks {
			blocks[i] = PretradeAccountBlockListGet(outBlocks, i)
		}
		DestroyPretradeAccountBlockList(outBlocks)
	}

	return PretradePostTradeResult{AccountBlocks: blocks, Outcomes: outAdjustments}, nil
}

// EngineApplyAccountAdjustment applies a batch of account adjustments.
//
// On the Applied status it returns the native account-adjustment outcome list
// handle (or nil), which the caller owns and must release with
// DestroyAccountAdjustmentOutcomeList.
func EngineApplyAccountAdjustment(
	engine Engine,
	accountID ParamAccountID,
	adjustments []AccountAdjustment,
) (AccountAdjustmentBatchError, AccountAdjustmentOutcomeList, error) {
	var adjustmentsPtr *C.OpenPitAccountAdjustment
	if len(adjustments) > 0 {
		adjustmentsPtr = (*C.OpenPitAccountAdjustment)(unsafe.Pointer(&adjustments[0]))
	}

	var reject AccountAdjustmentBatchError
	var outOutcomes AccountAdjustmentOutcomeList
	var outError SharedString
	status := C.openpit_engine_apply_account_adjustment(
		engine,
		accountID,
		adjustmentsPtr,
		C.size_t(len(adjustments)),
		&reject,
		&outOutcomes,
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	)

	switch status {
	case C.OpenPitAccountAdjustmentApplyStatus_Error:
		return nil, nil, consumeSharedStringAsError(outError, "openpit_engine_apply_account_adjustment failed")
	case C.OpenPitAccountAdjustmentApplyStatus_Applied:
		return nil, outOutcomes, nil
	case C.OpenPitAccountAdjustmentApplyStatus_Rejected:
		return reject, nil, nil
	default:
		DestroyAccountAdjustmentBatchError(reject)
		DestroySharedString(outError)
		return nil,
			nil,
			fmt.Errorf("openpit_engine_apply_account_adjustment failed with unexpected status %d", status)
	}
}

//------------------------------------------------------------------------------
// PretradePreTradeRequest

func PretradePreTradeRequestExecute(
	request PretradePreTradeRequest,
) (PretradePreTradeReservation, PretradeRejectList, error) {
	var reservation PretradePreTradeReservation
	var rejects PretradeRejectList
	var outError SharedString
	status := C.openpit_pretrade_pre_trade_request_execute(
		request,
		&reservation,
		&rejects,
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	)

	switch status {
	case C.OpenPitPretradeStatus_Passed:
		return reservation, nil, nil
	case C.OpenPitPretradeStatus_Rejected:
		if rejects == nil {
			return nil, nil, errors.New("order rejected, but no reject reason provided")
		}
		return nil, rejects, nil
	case C.OpenPitPretradeStatus_Error:
		return nil,
			nil,
			consumeSharedStringAsError(outError, "openpit_pretrade_pre_trade_request_execute failed")
	default:
		DestroyPretradePreTradeReservation(reservation)
		DestroyPretradePreTradeRequest(request)
		DestroyPretradeRejectList(rejects)
		DestroySharedString(outError)
		return nil,
			nil,
			fmt.Errorf("openpit_pretrade_pre_trade_request_execute failed with unexpected status %d", status)
	}
}

func DestroyPretradePreTradeRequest(request PretradePreTradeRequest) {
	C.openpit_destroy_pretrade_pre_trade_request(request)
}

//------------------------------------------------------------------------------
// PretradePreTradeReservation

func DestroyPretradePreTradeReservation(reservation PretradePreTradeReservation) {
	C.openpit_destroy_pretrade_pre_trade_reservation(reservation)
}

func PretradePreTradeReservationCommit(reservation PretradePreTradeReservation) {
	C.openpit_pretrade_pre_trade_reservation_commit(reservation)
}

func PretradePreTradeReservationRollback(reservation PretradePreTradeReservation) {
	C.openpit_pretrade_pre_trade_reservation_rollback(reservation)
}

func PretradePreTradeReservationGetLock(
	reservation PretradePreTradeReservation,
) PretradePreTradeLock {
	return C.openpit_pretrade_pre_trade_reservation_get_lock(reservation)
}

// PretradePreTradeReservationGetAccountAdjustments returns the native
// account-adjustment outcome list produced by the reservation, or nil. The
// caller owns it and must release it with DestroyAccountAdjustmentOutcomeList.
func PretradePreTradeReservationGetAccountAdjustments(
	reservation PretradePreTradeReservation,
) AccountAdjustmentOutcomeList {
	return C.openpit_pretrade_pre_trade_reservation_get_account_adjustments(reservation)
}

//------------------------------------------------------------------------------
// AccountAdjustment

func DestroyAccountAdjustmentBatchError(handle AccountAdjustmentBatchError) {
	C.openpit_destroy_account_adjustment_batch_error(handle)
}

func AccountAdjustmentBatchErrorGetFailedAdjustmentIndex(
	handle AccountAdjustmentBatchError,
) int {
	return int(C.openpit_account_adjustment_batch_error_get_failed_adjustment_index(handle))
}

func AccountAdjustmentBatchErrorGetRejects(handle AccountAdjustmentBatchError) PretradeRejectList {
	return C.openpit_account_adjustment_batch_error_get_rejects(handle)
}

//------------------------------------------------------------------------------
// Mutations

func MutationsPush(
	mutations Mutations,
	commitFnAddr unsafe.Pointer,
	rollbackFnAddr unsafe.Pointer,
	userData unsafe.Pointer,
	freeFnAddr unsafe.Pointer,
) error {
	var outError SharedString
	if !C.openpit_mutations_push(
		mutations,
		*(*C.OpenPitMutationFn)(commitFnAddr),
		*(*C.OpenPitMutationFn)(rollbackFnAddr),
		userData,
		*(*C.OpenPitMutationFreeFn)(freeFnAddr),
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	) {
		return consumeSharedStringAsError(outError, "openpit_mutations_push failed")
	}
	return nil
}

//------------------------------------------------------------------------------
