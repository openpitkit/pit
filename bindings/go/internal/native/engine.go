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
	SyncPolicyFull    SyncPolicy = C.OpenPitSyncPolicy_Full
	SyncPolicyLocal   SyncPolicy = C.OpenPitSyncPolicy_Local
	SyncPolicyAccount SyncPolicy = C.OpenPitSyncPolicy_Account
)

func CreateEngineBuilder(syncPolicy SyncPolicy) (EngineBuilder, error) {
	var outError SharedString
	builder := C.openpit_create_engine_builder(
		syncPolicy,
		C.OpenPitOutError(&outError), //nolint:gocritic
	)
	if builder == nil {
		return nil, consumeSharedStringAsError(outError, "openpit_create_engine_builder failed")
	}
	return builder, nil
}

func DestroyEngineBuilder(builder EngineBuilder) {
	C.openpit_destroy_engine_builder(builder)
}

func EngineBuilderBuild(builder EngineBuilder) (Engine, error) {
	var outError SharedString
	e := C.openpit_engine_builder_build(builder, C.OpenPitOutError(&outError)) //nolint:gocritic
	if e == nil {
		return nil, consumeSharedStringAsError(outError, "openpit_engine_builder_build failed")
	}
	return e, nil
}

func EngineBuilderAddPreTradePolicy(builder EngineBuilder, policy PretradePreTradePolicy) error {
	var outError SharedString
	if !C.openpit_engine_builder_add_pre_trade_policy(builder, policy, C.OpenPitOutError(&outError)) { //nolint:gocritic
		return consumeSharedStringAsError(outError, "openpit_engine_builder_add_pre_trade_policy failed")
	}
	return nil
}

func EngineBuilderAddBuiltinOrderValidation(builder EngineBuilder) error {
	var outError SharedString
	if !C.openpit_engine_builder_add_builtin_order_validation_policy(
		builder,
		C.OpenPitOutError(&outError), //nolint:gocritic
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
		broker,
		assetsPtr,
		C.size_t(len(assets)),
		accountsPtr,
		C.size_t(len(accounts)),
		accountAssetsPtr,
		C.size_t(len(accountAssets)),
		C.OpenPitOutError(&outError), //nolint:gocritic
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
		broker,
		assetsPtr,
		C.size_t(len(assets)),
		accountAssetsPtr,
		C.size_t(len(accountAssets)),
		C.OpenPitOutError(&outError), //nolint:gocritic
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
		brokerPtr,
		C.size_t(len(brokerBarriers)),
		accountPtr,
		C.size_t(len(accountBarriers)),
		C.OpenPitOutError(&outError), //nolint:gocritic
	) {
		return consumeSharedStringAsError(
			outError,
			"openpit_engine_builder_add_builtin_pnl_bounds_killswitch_policy failed",
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
		C.OpenPitOutError(&outError), //nolint:gocritic
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
		C.OpenPitOutError(&outError), //nolint:gocritic
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
}

func EngineApplyExecutionReport(
	engine Engine,
	report ExecutionReport,
) (PretradePostTradeResult, error) {
	var outError SharedString
	var outBlocks PretradeAccountBlockList
	if !C.openpit_engine_apply_execution_report(
		engine,
		&report,
		&outBlocks,
		C.OpenPitOutError(&outError), //nolint:gocritic
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

	return PretradePostTradeResult{AccountBlocks: blocks}, nil
}

func EngineApplyAccountAdjustment(
	engine Engine,
	accountID ParamAccountID,
	adjustments []AccountAdjustment,
) (AccountAdjustmentBatchError, error) {
	var adjustmentsPtr *C.OpenPitAccountAdjustment
	if len(adjustments) > 0 {
		adjustmentsPtr = (*C.OpenPitAccountAdjustment)(unsafe.Pointer(&adjustments[0]))
	}

	var reject AccountAdjustmentBatchError
	var outError SharedString
	status := C.openpit_engine_apply_account_adjustment(
		engine,
		accountID,
		adjustmentsPtr,
		C.size_t(len(adjustments)),
		&reject,
		C.OpenPitOutError(&outError), //nolint:gocritic
	)

	switch status {
	case C.OpenPitAccountAdjustmentApplyStatus_Error:
		return nil, consumeSharedStringAsError(outError, "openpit_engine_apply_account_adjustment failed")
	case C.OpenPitAccountAdjustmentApplyStatus_Applied:
		return nil, nil
	case C.OpenPitAccountAdjustmentApplyStatus_Rejected:
		return reject, nil
	default:
		DestroyAccountAdjustmentBatchError(reject)
		DestroySharedString(outError)
		return nil,
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
		C.OpenPitOutError(&outError), //nolint:gocritic
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

//------------------------------------------------------------------------------
// PretradePreTradeLock

func NewPretradePreTradeLock() PretradePreTradeLock {
	return PretradePreTradeLock{}
}

func PretradePreTradeLockReset(lock *PretradePreTradeLock) {
	*lock = NewPretradePreTradeLock()
}

func PretradePreTradeLockGetPrice(lock PretradePreTradeLock) ParamPriceOptional {
	return lock.price
}

func PretradePreTradeLockSetPrice(lock *PretradePreTradeLock, price ParamPrice) {
	lock.price.value = price
	lock.price.is_set = true
}

func PretradePreTradeLockUnsetPrice(lock *PretradePreTradeLock) {
	lock.price = ParamPriceOptional{}
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
		C.OpenPitOutError(&outError), //nolint:gocritic
	) {
		return consumeSharedStringAsError(outError, "openpit_mutations_push failed")
	}
	return nil
}

//------------------------------------------------------------------------------
