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
#include <stdlib.h>

extern const OpenPitPretradeRejectList * openpit_account_adjustment_batch_error_get_rejects(
    const OpenPitAccountAdjustmentBatchError * batch_error
);
*/
import "C"

import (
	"errors"
	"fmt"
	"runtime"
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
// On a domain build failure it surfaces a non-nil EngineBuildError handle,
// which the caller owns and must release with DestroyEngineBuildError. On a
// boundary failure it surfaces the error from the string out-error. A null
// engine is treated as failure; on success both the handle and the error are
// nil.
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

// PolicyInstance identifies a registration. An empty Name uses the policy name;
// the zero Mode is Reject.
type PolicyInstance struct {
	Name string
	Mode PretradeRuleMode
}

func newPolicyInstance(instance *PolicyInstance) (*C.OpenPitPolicyInstance, func(), error) {
	if instance == nil {
		return nil, func() {}, nil
	}
	mode, ok := checkedRuleModeToC(instance.Mode)
	if !ok {
		return nil, nil, fmt.Errorf("invalid instance mode: %d", instance.Mode)
	}
	var name C.OpenPitStringView
	if len(instance.Name) > 0 {
		name.ptr = (*C.uint8_t)(C.malloc(C.size_t(len(instance.Name))))
		name.len = C.size_t(len(instance.Name))
		copy(unsafe.Slice((*byte)(unsafe.Pointer(name.ptr)), len(instance.Name)), instance.Name)
	}
	return &C.OpenPitPolicyInstance{
		name: name,
		mode: mode,
	}, func() { C.free(unsafe.Pointer(name.ptr)) }, nil
}

func EngineBuilderAddPreTradePolicy(builder EngineBuilder, policy PretradePreTradePolicy, instance *PolicyInstance) error {
	cInstance, release, err := newPolicyInstance(instance)
	if err != nil {
		return err
	}
	defer release()
	var outError SharedString
	if !C.openpit_engine_builder_add_pre_trade_policy(builder, policy, cInstance, C.OpenPitOutError(&outError)) { //nolint:gocritic // CGo out-parameter requires address-of operator
		return consumeSharedStringAsError(outError, "openpit_engine_builder_add_pre_trade_policy failed")
	}
	return nil
}

func EngineBuilderAddBuiltinOrderValidation(builder EngineBuilder, groupID PolicyGroupID, instance *PolicyInstance) error {
	cInstance, release, err := newPolicyInstance(instance)
	if err != nil {
		return err
	}
	defer release()
	var outError SharedString
	if !C.openpit_engine_builder_add_builtin_order_validation_policy(
		builder,
		groupID,
		cInstance,
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
	brokers []PretradePoliciesRateLimitBrokerBarrier,
	assets []PretradePoliciesRateLimitAssetBarrier,
	accounts []PretradePoliciesRateLimitAccountBarrier,
	accountAssets []PretradePoliciesRateLimitAccountAssetBarrier,
	instance *PolicyInstance,
) error {
	cInstance, release, err := newPolicyInstance(instance)
	if err != nil {
		return err
	}
	defer release()
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
		accountAssetsPtr = (*C.OpenPitPretradePoliciesRateLimitAccountAssetBarrier)(unsafe.Pointer(&accountAssets[0]))
	}

	var outError SharedString
	ok := C.openpit_engine_builder_add_builtin_rate_limit_policy(
		builder,
		groupID,
		brokersPtr,
		C.size_t(len(brokers)),
		assetsPtr,
		C.size_t(len(assets)),
		accountsPtr,
		C.size_t(len(accounts)),
		accountAssetsPtr,
		C.size_t(len(accountAssets)),
		cInstance,
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	)
	runtime.KeepAlive(brokers)
	runtime.KeepAlive(assets)
	runtime.KeepAlive(accounts)
	runtime.KeepAlive(accountAssets)
	if !ok {
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
	instance *PolicyInstance,
) error {
	cInstance, release, err := newPolicyInstance(instance)
	if err != nil {
		return err
	}
	defer release()
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
	ok := C.openpit_engine_builder_add_builtin_order_size_limit_policy(
		builder,
		groupID,
		broker,
		assetsPtr,
		C.size_t(len(assets)),
		accountAssetsPtr,
		C.size_t(len(accountAssets)),
		cInstance,
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	)
	runtime.KeepAlive(assets)
	runtime.KeepAlive(accountAssets)
	if !ok {
		return consumeSharedStringAsError(
			outError,
			"openpit_engine_builder_add_builtin_order_size_limit_policy failed",
		)
	}
	return nil
}

// EngineBuilderAddBuiltinReviewRule copies the rules into the builder.
// Nil global omits the global rule; nil instrument lists cover all instruments.
func EngineBuilderAddBuiltinReviewRule(
	builder EngineBuilder,
	groupID PolicyGroupID,
	global *ReviewRuleGlobalRule,
	groups []ReviewRuleAccountGroupRule,
	accounts []ReviewRuleAccountRule,
	instance *PolicyInstance,
) error {
	cInstance, release, err := newPolicyInstance(instance)
	if err != nil {
		return err
	}
	defer release()
	args := newReviewRuleArguments(global, groups, accounts)
	defer args.release()
	var outError SharedString
	if !C.openpit_engine_builder_add_builtin_review_rule_policy(
		builder, groupID, args.global,
		args.groups, C.size_t(len(groups)),
		args.accounts, C.size_t(len(accounts)),
		cInstance,
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	) {
		return consumeSharedStringAsError(
			outError, "openpit_engine_builder_add_builtin_review_rule_policy failed",
		)
	}
	return nil
}

func EngineBuilderAddBuiltinPnlBoundsKillSwitch(
	builder EngineBuilder,
	groupID PolicyGroupID,
	brokerBarriers []PretradePoliciesPnlBoundsBarrier,
	accountBarriers []PretradePoliciesPnlBoundsAccountBarrier,
	instance *PolicyInstance,
) error {
	cInstance, release, err := newPolicyInstance(instance)
	if err != nil {
		return err
	}
	defer release()
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
	ok := C.openpit_engine_builder_add_builtin_pnl_bounds_killswitch_policy(
		builder,
		groupID,
		brokerPtr,
		C.size_t(len(brokerBarriers)),
		accountPtr,
		C.size_t(len(accountBarriers)),
		cInstance,
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	)
	runtime.KeepAlive(brokerBarriers)
	runtime.KeepAlive(accountBarriers)
	if !ok {
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
	switch {
	case accountID != nil:
		return NewPretradePoliciesSpotFundsInstrumentAccountOverride(
			instrumentID,
			*accountID,
			slippageBps,
		)
	case accountGroupID != nil:
		return NewPretradePoliciesSpotFundsInstrumentAccountGroupOverride(
			instrumentID,
			*accountGroupID,
			slippageBps,
		)
	default:
		return NewPretradePoliciesSpotFundsInstrumentOverride(
			instrumentID,
			slippageBps,
		)
	}
}

func EngineBuilderAddBuiltinSpotFunds(
	builder EngineBuilder,
	marketData MarketDataService,
	marketSlippageBps *uint16,
	pricingSource uint8,
	instrumentOverrides []PretradePoliciesSpotFundsOverride,
	groupID PolicyGroupID,
	instance *PolicyInstance,
) error {
	cInstance, release, err := newPolicyInstance(instance)
	if err != nil {
		return err
	}
	defer release()
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
	ok := C.openpit_engine_builder_add_builtin_spot_funds_policy(
		builder,
		marketData,
		slippagePtr,
		C.uint8_t(pricingSource),
		overridesPtr,
		C.size_t(len(instrumentOverrides)),
		groupID,
		cInstance,
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	)
	runtime.KeepAlive(instrumentOverrides)
	if !ok {
		return consumeSharedStringAsError(
			outError,
			"openpit_engine_builder_add_builtin_spot_funds_policy failed",
		)
	}
	return nil
}

func EngineBuilderAddBuiltinSpotFundsPnlBoundsKillSwitch(
	builder EngineBuilder,
	marketData MarketDataService,
	groupID PolicyGroupID,
	globalBarriers []PretradePoliciesSpotFundsPnlBoundsBarrier,
	accountGroupBarriers []PretradePoliciesSpotFundsPnlBoundsAccountGroupBarrier,
	accountBarriers []PretradePoliciesSpotFundsPnlBoundsAccountBarrier,
	instance *PolicyInstance,
) error {
	cInstance, release, err := newPolicyInstance(instance)
	if err != nil {
		return err
	}
	defer release()
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

	var outError SharedString
	ok := C.openpit_engine_builder_add_builtin_spot_funds_pnl_bounds_killswitch_policy(
		builder,
		marketData,
		groupID,
		globalPtr,
		C.size_t(len(globalBarriers)),
		accountGroupPtr,
		C.size_t(len(accountGroupBarriers)),
		accountPtr,
		C.size_t(len(accountBarriers)),
		cInstance,
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	)
	runtime.KeepAlive(globalBarriers)
	runtime.KeepAlive(accountGroupBarriers)
	runtime.KeepAlive(accountBarriers)
	if !ok {
		return consumeSharedStringAsError(
			outError,
			"openpit_engine_builder_add_builtin_spot_funds_pnl_bounds_killswitch_policy failed",
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

func EngineApplyDropCopy(
	engine Engine,
	order Order,
) (PretradeDropCopyOperation, PretradeRejectList, error) {
	var operation PretradeDropCopyOperation
	var rejects PretradeRejectList
	var outError SharedString
	status := C.openpit_engine_apply_drop_copy(
		engine,
		&order,
		&operation,
		&rejects,
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	)

	switch status {
	case C.OpenPitPretradeStatus_Passed:
		return operation, nil, nil
	case C.OpenPitPretradeStatus_Rejected:
		if rejects == nil {
			return nil, nil, errors.New("drop copy rejected, but no reject reason provided")
		}
		return nil, rejects, nil
	case C.OpenPitPretradeStatus_Error:
		return nil, nil, consumeSharedStringAsError(
			outError,
			"openpit_engine_apply_drop_copy failed",
		)
	default:
		DestroyPretradeDropCopyOperation(operation)
		DestroyPretradeRejectList(rejects)
		DestroySharedString(outError)
		return nil, nil, fmt.Errorf(
			"openpit_engine_apply_drop_copy failed with unexpected status %d",
			status,
		)
	}
}

// EngineApplyExecutionReport returns one native aggregate. Its lists are
// borrowed from the aggregate and must be copied before it is destroyed.

func EngineApplyExecutionReport(
	engine Engine,
	report ExecutionReport,
) (PostTradeResult, error) {
	var outError SharedString
	var outResult PostTradeResult
	if !C.openpit_engine_apply_execution_report(
		engine,
		&report,
		&outResult,
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	) {
		return nil,
			consumeSharedStringAsError(outError, "openpit_engine_apply_execution_report failed")
	}
	return outResult, nil
}

// DestroyPostTradeResult releases the aggregate and all lists it owns.
func DestroyPostTradeResult(result PostTradeResult) {
	C.openpit_destroy_post_trade_result(result)
}

func PostTradeResultGetAccountBlocks(result PostTradeResult) PretradeAccountBlockList {
	return C.openpit_post_trade_result_get_account_blocks(result)
}

func PostTradeResultGetAccountAdjustments(result PostTradeResult) AccountAdjustmentOutcomeList {
	return C.openpit_post_trade_result_get_account_adjustments(result)
}

func PostTradeResultGetAccountPnls(result PostTradeResult) AccountPnlOutcomeList {
	return C.openpit_post_trade_result_get_account_pnls(result)
}

// EngineApplyAccountAdjustment applies a batch of account adjustments.
//
// On the Applied status it returns the native account-adjustment outcome and
// account-block lists (either may be nil). The caller owns both handles.
func EngineApplyAccountAdjustment(
	engine Engine,
	accountID ParamAccountID,
	adjustments []AccountAdjustment,
) (AccountAdjustmentBatchError, AccountAdjustmentOutcomeList, PretradeAccountBlockList, error) {
	var adjustmentsPtr *C.OpenPitAccountAdjustment
	if len(adjustments) > 0 {
		adjustmentsPtr = (*C.OpenPitAccountAdjustment)(unsafe.Pointer(&adjustments[0]))
	}

	var reject AccountAdjustmentBatchError
	var outOutcomes AccountAdjustmentOutcomeList
	var outBlocks PretradeAccountBlockList
	var outError SharedString
	status := C.openpit_engine_apply_account_adjustment(
		engine,
		accountID,
		adjustmentsPtr,
		C.size_t(len(adjustments)),
		&reject,
		&outOutcomes,
		&outBlocks,
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	)
	runtime.KeepAlive(adjustments)

	switch status {
	case C.OpenPitAccountAdjustmentApplyStatus_Error:
		return nil, nil, nil,
			consumeSharedStringAsError(outError, "openpit_engine_apply_account_adjustment failed")
	case C.OpenPitAccountAdjustmentApplyStatus_Applied:
		return nil, outOutcomes, outBlocks, nil
	case C.OpenPitAccountAdjustmentApplyStatus_Rejected:
		return reject, nil, nil, nil
	default:
		DestroyAccountAdjustmentBatchError(reject)
		DestroyAccountAdjustmentOutcomeList(outOutcomes)
		DestroyPretradeAccountBlockList(outBlocks)
		DestroySharedString(outError)
		return nil, nil, nil,
			fmt.Errorf("openpit_engine_apply_account_adjustment failed with unexpected status %d", status)
	}
}

//------------------------------------------------------------------------------
// PretradePreTradeRequest

func PretradePreTradeRequestGetReviews(request PretradePreTradeRequest) (PretradeRejectList, error) {
	var outError SharedString
	list := C.openpit_pretrade_pre_trade_request_get_reviews(
		request, C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	)
	if list == nil {
		return nil, consumeSharedStringAsError(outError, "openpit_pretrade_pre_trade_request_get_reviews failed")
	}
	return list, nil
}

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
// PretradePreTradeDryRunReport

// EngineStartPreTradeDryRun runs the start stage as a non-mutating dry-run.
//
// On valid input always returns a non-nil report handle (the verdict is
// encoded inside); on invalid input returns an error.
func EngineStartPreTradeDryRun(
	engine Engine,
	order Order,
) (PretradePreTradeDryRunReport, error) {
	var report PretradePreTradeDryRunReport
	var outError SharedString
	if !C.openpit_engine_start_pre_trade_dry_run(
		engine,
		&order,
		&report,
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	) {
		return nil,
			consumeSharedStringAsError(outError, "openpit_engine_start_pre_trade_dry_run failed")
	}
	return report, nil
}

// EngineExecutePreTradeDryRun runs the full pre-trade pipeline as a
// non-mutating dry-run.
//
// On valid input always returns a non-nil report handle (the verdict is
// encoded inside); on invalid input returns an error.
func EngineExecutePreTradeDryRun(
	engine Engine,
	order Order,
) (PretradePreTradeDryRunReport, error) {
	var report PretradePreTradeDryRunReport
	var outError SharedString
	if !C.openpit_engine_execute_pre_trade_dry_run(
		engine,
		&order,
		&report,
		C.OpenPitOutError(&outError), //nolint:gocritic // CGo out-parameter requires address-of operator
	) {
		return nil,
			consumeSharedStringAsError(outError, "openpit_engine_execute_pre_trade_dry_run failed")
	}
	return report, nil
}

func PretradePreTradeDryRunReportIsPass(report PretradePreTradeDryRunReport) bool {
	return bool(C.openpit_pretrade_pre_trade_dry_run_report_is_pass(report))
}

// PretradePreTradeDryRunReportGetRejects returns a caller-owned reject list.
// Release it with DestroyPretradeRejectList.
func PretradePreTradeDryRunReportGetRejects(
	report PretradePreTradeDryRunReport,
) PretradeRejectList {
	return C.openpit_pretrade_pre_trade_dry_run_report_get_rejects(report)
}

func PretradePreTradeDryRunReportGetReviews(report PretradePreTradeDryRunReport) PretradeRejectList {
	return C.openpit_pretrade_pre_trade_dry_run_report_get_reviews(report)
}

// PretradePreTradeDryRunReportGetLock returns a caller-owned lock handle.
// Release it with DestroyPretradePreTradeLock.
func PretradePreTradeDryRunReportGetLock(
	report PretradePreTradeDryRunReport,
) PretradePreTradeLock {
	return C.openpit_pretrade_pre_trade_dry_run_report_get_lock(report)
}

// PretradePreTradeDryRunReportGetAccountAdjustments returns a caller-owned
// account-adjustment outcome list handle. Release it with
// DestroyAccountAdjustmentOutcomeList.
func PretradePreTradeDryRunReportGetAccountAdjustments(
	report PretradePreTradeDryRunReport,
) AccountAdjustmentOutcomeList {
	return C.openpit_pretrade_pre_trade_dry_run_report_get_account_adjustments(report)
}

// PretradePreTradeDryRunReportGetAccountBlock returns a caller-owned
// account-block list handle (0-or-1 element). Release it with
// DestroyPretradeAccountBlockList.
func PretradePreTradeDryRunReportGetAccountBlock(
	report PretradePreTradeDryRunReport,
) PretradeAccountBlockList {
	return C.openpit_pretrade_pre_trade_dry_run_report_get_account_block(report)
}

func DestroyPretradePreTradeDryRunReport(report PretradePreTradeDryRunReport) {
	C.openpit_destroy_pretrade_pre_trade_dry_run_report(report)
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

func PretradePreTradeReservationGetReviews(reservation PretradePreTradeReservation) PretradeRejectList {
	return C.openpit_pretrade_pre_trade_reservation_get_reviews(reservation)
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
// PretradeDropCopyOperation

func DestroyPretradeDropCopyOperation(operation PretradeDropCopyOperation) {
	C.openpit_destroy_pretrade_drop_copy_operation(operation)
}

func PretradeDropCopyOperationCommit(operation PretradeDropCopyOperation) {
	C.openpit_pretrade_drop_copy_operation_commit(operation)
}

func PretradeDropCopyOperationRollback(operation PretradeDropCopyOperation) {
	C.openpit_pretrade_drop_copy_operation_rollback(operation)
}

func PretradeDropCopyOperationGetLock(
	operation PretradeDropCopyOperation,
) PretradePreTradeLock {
	return C.openpit_pretrade_drop_copy_operation_get_lock(operation)
}

// PretradeDropCopyOperationGetAccountAdjustments returns the native
// account-adjustment outcome list produced by the operation. The caller owns it
// and must release it with DestroyAccountAdjustmentOutcomeList.
func PretradeDropCopyOperationGetAccountAdjustments(
	operation PretradeDropCopyOperation,
) AccountAdjustmentOutcomeList {
	return C.openpit_pretrade_drop_copy_operation_get_account_adjustments(operation)
}

// PretradeDropCopyOperationGetAccountBlock returns the native account-block
// list (0-or-1 element) requested by the operation. The caller owns it and must
// release it with DestroyPretradeAccountBlockList.
func PretradeDropCopyOperationGetAccountBlock(
	operation PretradeDropCopyOperation,
) PretradeAccountBlockList {
	return C.openpit_pretrade_drop_copy_operation_get_account_block(operation)
}

func PretradeDropCopyOperationIsAccountBlocked(
	operation PretradeDropCopyOperation,
) bool {
	return bool(C.openpit_pretrade_drop_copy_operation_is_account_blocked(operation))
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
