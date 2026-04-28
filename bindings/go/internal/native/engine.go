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

extern const PitRejectList * pit_account_adjustment_batch_error_get_rejects(
    const PitAccountAdjustmentBatchError * batch_error
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

func CreateEngineBuilder() EngineBuilder {
	return C.pit_create_engine_builder()
}

func DestroyEngineBuilder(builder EngineBuilder) {
	C.pit_destroy_engine_builder(builder)
}

func EngineBuilderBuild(builder EngineBuilder) (Engine, error) {
	var outError SharedString
	e := C.pit_engine_builder_build(builder, C.PitOutError(&outError)) //nolint:gocritic
	if e == nil {
		return nil, consumeSharedStringAsError(outError, "pit_engine_builder_build failed")
	}
	return e, nil
}

func EngineBuilderAddCheckPreTradeStartPolicy(
	builder EngineBuilder,
	policy PretradeCheckPreTradeStartPolicy,
) error {
	var outError SharedString
	if !C.pit_engine_builder_add_check_pre_trade_start_policy(
		builder,
		policy,
		C.PitOutError(&outError), //nolint:gocritic
	) {
		return consumeSharedStringAsError(
			outError,
			"pit_engine_builder_add_check_pre_trade_start_policy failed",
		)
	}
	return nil
}

func EngineBuilderAddPreTradePolicy(builder EngineBuilder, policy PretradePreTradePolicy) error {
	var outError SharedString
	if !C.pit_engine_builder_add_pre_trade_policy(builder, policy, C.PitOutError(&outError)) { //nolint:gocritic
		return consumeSharedStringAsError(outError, "pit_engine_builder_add_pre_trade_policy failed")
	}
	return nil
}

func EngineBuilderAddAccountAdjustmentPolicy(
	builder EngineBuilder,
	policy AccountAdjustmentPolicy,
) error {
	var outError SharedString
	if !C.pit_engine_builder_add_account_adjustment_policy(
		builder,
		policy,
		C.PitOutError(&outError), //nolint:gocritic
	) {
		return consumeSharedStringAsError(
			outError,
			"pit_engine_builder_add_account_adjustment_policy failed",
		)
	}
	return nil
}

func DestroyEngine(engine Engine) {
	C.pit_destroy_engine(engine)
}

func EngineStartPreTrade(engine Engine, order Order) (PretradePreTradeRequest, RejectList, error) {
	var request PretradePreTradeRequest
	var rejects RejectList
	var outError SharedString
	status := C.pit_engine_start_pre_trade(
		engine,
		&order,
		&request,
		&rejects,
		C.PitOutError(&outError), //nolint:gocritic
	)

	switch status {
	case C.PitPretradeStatus_Passed:
		return request, nil, nil
	case C.PitPretradeStatus_Rejected:
		if rejects == nil {
			return nil, nil, errors.New("order rejected, but no reject reason provided")
		}
		return nil, rejects, nil
	case C.PitPretradeStatus_Error:
		return nil, nil, consumeSharedStringAsError(outError, "pit_engine_start_pre_trade failed")
	default:
		DestroyPretradePreTradeRequest(request)
		DestroyRejectList(rejects)
		DestroySharedString(outError)
		return nil,
			nil,
			fmt.Errorf("pit_engine_start_pre_trade failed with unexpected status %d", status)
	}
}

func EngineExecutePreTrade(
	engine Engine,
	order Order,
) (PretradePreTradeReservation, RejectList, error) {
	var reservation PretradePreTradeReservation
	var rejects RejectList
	var outError SharedString
	status := C.pit_engine_execute_pre_trade(
		engine,
		&order,
		&reservation,
		&rejects,
		C.PitOutError(&outError), //nolint:gocritic
	)

	switch status {
	case C.PitPretradeStatus_Passed:
		return reservation, nil, nil
	case C.PitPretradeStatus_Rejected:
		if rejects == nil {
			return nil, nil, errors.New("order rejected, but no reject reason provided")
		}
		return nil, rejects, nil
	case C.PitPretradeStatus_Error:
		return nil, nil, consumeSharedStringAsError(outError, "pit_engine_execute_pre_trade failed")
	default:
		DestroyPretradePreTradeReservation(reservation)
		DestroyRejectList(rejects)
		DestroySharedString(outError)
		return nil,
			nil,
			fmt.Errorf("pit_engine_execute_pre_trade failed with unexpected status %d", status)
	}
}

type PretradePostTradeResult struct {
	KillSwitchTriggered bool
}

func EngineApplyExecutionReport(
	engine Engine,
	report ExecutionReport,
) (PretradePostTradeResult, error) {
	var outError SharedString
	result := C.pit_engine_apply_execution_report(engine, &report, C.PitOutError(&outError)) //nolint:gocritic
	if result.is_error {
		return PretradePostTradeResult{},
			consumeSharedStringAsError(outError, "pit_engine_apply_execution_report failed")
	}
	return PretradePostTradeResult{
			KillSwitchTriggered: bool(result.post_trade_result.kill_switch_triggered),
		},
		nil
}

func EngineApplyAccountAdjustment(
	engine Engine,
	accountID ParamAccountID,
	adjustments []AccountAdjustment,
) (AccountAdjustmentBatchError, error) {
	if len(adjustments) == 0 {
		return nil, errors.New("adjustment list is empty")
	}

	var reject AccountAdjustmentBatchError
	var outError SharedString
	status := C.pit_engine_apply_account_adjustment(
		engine,
		accountID,
		(*C.PitAccountAdjustment)(unsafe.Pointer(&adjustments[0])),
		C.size_t(len(adjustments)),
		&reject,
		C.PitOutError(&outError), //nolint:gocritic
	)

	switch status {
	case C.PitAccountAdjustmentApplyStatus_Error:
		return nil, consumeSharedStringAsError(outError, "pit_engine_apply_account_adjustment failed")
	case C.PitAccountAdjustmentApplyStatus_Applied:
		return nil, nil
	case C.PitAccountAdjustmentApplyStatus_Rejected:
		return reject, nil
	default:
		DestroyAccountAdjustmentBatchError(reject)
		DestroySharedString(outError)
		return nil,
			fmt.Errorf("pit_engine_apply_account_adjustment failed with unexpected status %d", status)
	}
}

//------------------------------------------------------------------------------
// PretradePreTradeRequest

func PretradePreTradeRequestExecute(
	request PretradePreTradeRequest,
) (PretradePreTradeReservation, RejectList, error) {
	var reservation PretradePreTradeReservation
	var rejects RejectList
	var outError SharedString
	status := C.pit_pretrade_pre_trade_request_execute(
		request,
		&reservation,
		&rejects,
		C.PitOutError(&outError), //nolint:gocritic
	)

	switch status {
	case C.PitPretradeStatus_Passed:
		return reservation, nil, nil
	case C.PitPretradeStatus_Rejected:
		if rejects == nil {
			return nil, nil, errors.New("order rejected, but no reject reason provided")
		}
		return nil, rejects, nil
	case C.PitPretradeStatus_Error:
		return nil,
			nil,
			consumeSharedStringAsError(outError, "pit_pretrade_pre_trade_request_execute failed")
	default:
		DestroyPretradePreTradeReservation(reservation)
		DestroyPretradePreTradeRequest(request)
		DestroyRejectList(rejects)
		DestroySharedString(outError)
		return nil,
			nil,
			fmt.Errorf("pit_pretrade_pre_trade_request_execute failed with unexpected status %d", status)
	}
}

func DestroyPretradePreTradeRequest(request PretradePreTradeRequest) {
	C.pit_destroy_pretrade_pre_trade_request(request)
}

//------------------------------------------------------------------------------
// PretradePreTradeReservation

func DestroyPretradePreTradeReservation(reservation PretradePreTradeReservation) {
	C.pit_destroy_pretrade_pre_trade_reservation(reservation)
}

func PretradePreTradeReservationCommit(reservation PretradePreTradeReservation) {
	C.pit_pretrade_pre_trade_reservation_commit(reservation)
}

func PretradePreTradeReservationRollback(reservation PretradePreTradeReservation) {
	C.pit_pretrade_pre_trade_reservation_rollback(reservation)
}

func PretradePreTradeReservationGetLock(
	reservation PretradePreTradeReservation,
) PretradePreTradeLock {
	return C.pit_pretrade_pre_trade_reservation_get_lock(reservation)
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
	C.pit_destroy_account_adjustment_batch_error(handle)
}

func AccountAdjustmentBatchErrorGetFailedAdjustmentIndex(
	handle AccountAdjustmentBatchError,
) int {
	return int(C.pit_account_adjustment_batch_error_get_failed_adjustment_index(handle))
}

func AccountAdjustmentBatchErrorGetRejects(handle AccountAdjustmentBatchError) RejectList {
	return C.pit_account_adjustment_batch_error_get_rejects(handle)
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
	if !C.pit_mutations_push(
		mutations,
		*(*C.PitMutationFn)(commitFnAddr),
		*(*C.PitMutationFn)(rollbackFnAddr),
		userData,
		*(*C.PitMutationFreeFn)(freeFnAddr),
		C.PitOutError(&outError), //nolint:gocritic
	) {
		return consumeSharedStringAsError(outError, "pit_mutations_push failed")
	}
	return nil
}

//------------------------------------------------------------------------------
