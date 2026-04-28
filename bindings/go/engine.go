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

// Package pit exposes the Go binding for the OpenPit engine.
//
// Threading:
// The SDK never spawns OS threads: each public method call runs on the OS
// thread that invoked it. Concurrent invocation of public methods on the same
// engine handle is undefined behavior and must be prevented by the caller.
// Sequential calls on the same handle from different OS threads are supported.
// Goroutine migration between OS threads during one SDK call is supported.
// Callbacks invoked by the SDK back into Go may run on a different OS thread
// than the goroutine that initiated the call, so callback code must not rely
// on thread-local OS state.

package openpit

import (
	"fmt"

	"go.openpit.dev/openpit/accountadjustment"
	"go.openpit.dev/openpit/internal/custompolicy"
	"go.openpit.dev/openpit/internal/loader"
	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/optional"
	"go.openpit.dev/openpit/pretrade"
	"go.openpit.dev/openpit/reject"
)

//------------------------------------------------------------------------------
// Engine

type Engine struct{ handle native.Engine }

func newEngineFromHandle(handle native.Engine) *Engine {
	return &Engine{handle: handle}
}

// Stop signals the engine to halt internal evaluation, releases policies
// registered on the engine, and frees the underlying native resources.
//
// After Stop returns, the engine handle is no longer valid for any operation.
// The engine must no longer be passed to any other
// method (StartPreTrade, ExecutePreTrade, ApplyExecutionReport,
// ApplyAccountAdjustment); doing so is undefined behavior.
//
// Idempotency: safe to call more than once; subsequent calls are no-ops.
//
// Outstanding objects previously produced by this engine
// (pretrade.Request, pretrade.Reservation) remain owned by the caller and
// must be released independently.
func (e *Engine) Stop() {
	native.DestroyEngine(e.handle)
	e.handle = nil
}

// StartPreTrade runs the start stage of the pre-trade pipeline.
//
// Return contract:
//   - on accept, returns a non-nil *pretrade.Request; the caller takes
//     ownership and must release it with Request.Close when done (Execute
//     does not close the request — see Request.Execute);
//   - on reject, returns a non-nil []reject.Reject; no Request is produced;
//   - on transport error, returns a Go error; no Request is produced.
func (e *Engine) StartPreTrade(order model.Order) (*pretrade.Request, []reject.Reject, error) {
	request, startReject, err := native.EngineStartPreTrade(e.handle, order.Handle())
	if err != nil {
		return nil, nil, err
	}
	if startReject != nil {
		rejectResult, err := reject.NewListFromHandle(startReject)
		native.DestroyRejectList(startReject)
		if err != nil {
			return nil,
				nil,
				fmt.Errorf("failed to create reject list for rejected pre-trade start: %w", err)
		}
		return nil, rejectResult, nil
	}
	return pretrade.NewRequestFromHandle(request), nil, nil
}

// ExecutePreTrade runs the full pre-trade pipeline and, on accept, returns
// a reservation representing the reserved but not yet finalized state.
//
// Return contract:
//   - on accept, returns a non-nil *pretrade.Reservation; the caller takes
//     ownership and must resolve it exactly once via CommitAndClose,
//     RollbackAndClose, or Close (which rolls back any pending mutations
//     implicitly);
//   - on reject, returns a non-nil []reject.Reject; no Reservation is produced;
//   - on transport error, returns a Go error; no Reservation is produced.
func (e *Engine) ExecutePreTrade(order model.Order) (*pretrade.Reservation, []reject.Reject, error) {
	reservation, execRejects, err := native.EngineExecutePreTrade(e.handle, order.Handle())
	if err != nil {
		return nil, nil, err
	}
	if execRejects != nil {
		rejectResult, err := reject.NewListFromHandle(execRejects)
		native.DestroyRejectList(execRejects)
		if err != nil {
			return nil,
				nil,
				fmt.Errorf("failed to create reject list for rejected order: %w", err)
		}
		return nil, rejectResult, nil
	}
	return pretrade.NewReservationFromHandle(reservation), nil, nil
}

type PostTradeResult struct {
	KillSwitchTriggered bool
}

func (e *Engine) ApplyExecutionReport(report model.ExecutionReport) (PostTradeResult, error) {
	result, err := native.EngineApplyExecutionReport(e.handle, report.Handle())
	if err != nil {
		return PostTradeResult{}, err
	}

	return PostTradeResult{
		KillSwitchTriggered: result.KillSwitchTriggered,
	}, nil
}

func (e *Engine) ApplyAccountAdjustment(
	accountID param.AccountID,
	adjustments []model.AccountAdjustment,
) (optional.Option[reject.AccountAdjustmentBatchError], error) {
	nativeAdjustments := make([]native.AccountAdjustment, len(adjustments))
	for i, adjustment := range adjustments {
		nativeAdjustments[i] = adjustment.Handle()
	}

	adjustmentReject, err := native.EngineApplyAccountAdjustment(
		e.handle,
		accountID.Handle(),
		nativeAdjustments,
	)
	if err != nil {
		return optional.None[reject.AccountAdjustmentBatchError](), err
	}

	if adjustmentReject != nil {
		rejectResult, err := reject.NewAccountAdjustmentBatchErrorFromHandle(adjustmentReject)
		native.DestroyAccountAdjustmentBatchError(adjustmentReject)
		if err != nil {
			return optional.None[reject.AccountAdjustmentBatchError](),
				fmt.Errorf("failed to create reject list for rejected account adjustment: %w", err)
		}
		return optional.Some(rejectResult), nil
	}

	return optional.None[reject.AccountAdjustmentBatchError](), nil
}

//------------------------------------------------------------------------------
// EngineBuilder

type EngineBuilder struct {
	handle native.EngineBuilder
	err    error

	// Policies that were accepted by the builder but never handed off to the
	// engine. The builder must close them on Close to release their resources.
	unfinished []interface{ Close() }
}

// NewEngineBuilder returns a new engine builder.
// The returned builder must be released by calling either Close or Build
// after use.
func NewEngineBuilder() (*EngineBuilder, error) {
	if err := loader.EnsureRuntimeLoaded(); err != nil {
		return nil, err
	}
	return &EngineBuilder{handle: native.CreateEngineBuilder()}, nil
}

// Close releases the builder and any policies that were handed to it but
// never transferred to the engine. Safe to call more than once and safe to
// call after Build; subsequent calls are no-ops.
func (b *EngineBuilder) Close() {
	{
		for _, entity := range b.unfinished {
			entity.Close()
		}
		b.unfinished = nil
	}
	if b.handle != nil {
		native.DestroyEngineBuilder(b.handle)
		b.handle = nil
	}
}

// Build constructs the engine and releases the builder. The builder is
// closed on both success and failure, so an explicit Close afterwards is a
// no-op. On failure, any policies that were accepted by the builder but not
// transferred to the engine are closed by the builder. On success, ownership
// of the returned engine passes to the caller, who must release it by
// calling Stop. Behavior is undefined if Build is called more than once on
// the same builder.
func (b *EngineBuilder) Build() (*Engine, error) {
	defer b.Close()

	if b.err != nil {
		return nil, b.err
	}

	handle, err := native.EngineBuilderBuild(b.handle)
	if err != nil {
		return nil, err
	}
	return newEngineFromHandle(handle), nil
}

func (b *EngineBuilder) CheckPreTradeStartPolicy(
	policy ...pretrade.CheckStartPolicy,
) *EngineBuilder {
	for _, p := range policy {
		// Every policy must go through addPolicy even after a previous failure
		// so that the builder takes responsibility for releasing it.
		b.addCheckPreTradeStartPolicy(p)
	}
	return b
}

func (b *EngineBuilder) BuiltinCheckPreTradeStartPolicy(
	policy ...pretrade.BuiltinPolicy,
) *EngineBuilder {
	for _, p := range policy {
		b.addBuiltinCheckPreTradeStartPolicy(p)
	}
	return b
}

func (b *EngineBuilder) PreTradePolicy(policy ...pretrade.Policy) *EngineBuilder {
	for _, p := range policy {
		// Every policy must go through addPolicy even after a previous failure
		// so that the builder takes responsibility for releasing it.
		b.addPreTradePolicy(p)
	}
	return b
}

func (b *EngineBuilder) BuiltinPreTradePolicy(policy ...pretrade.BuiltinPolicy) *EngineBuilder {
	_ = policy
	return b
}

func (b *EngineBuilder) AccountAdjustmentPolicy(policy ...accountadjustment.Policy) *EngineBuilder {
	for _, p := range policy {
		// Every policy must go through addPolicy even after a previous failure
		// so that the builder takes responsibility for releasing it.
		b.addAccountAdjustmentPolicy(p)
	}
	return b
}

func (b *EngineBuilder) BuiltinAccountAdjustmentPolicy(
	policy ...pretrade.BuiltinPolicy,
) *EngineBuilder {
	_ = policy
	return b
}

func (b *EngineBuilder) addCheckPreTradeStartPolicy(policy pretrade.CheckStartPolicy) {
	addPolicy(
		b,
		policy,
		custompolicy.StartCheckPreTradeStart,
		native.DestroyPretradeCheckPreTradeStartPolicy,
		native.EngineBuilderAddCheckPreTradeStartPolicy,
	)
}

func (b *EngineBuilder) addBuiltinCheckPreTradeStartPolicy(policy pretrade.BuiltinPolicy) {
	addPolicy(
		b,
		policy,
		nil,
		native.DestroyPretradeCheckPreTradeStartPolicy,
		native.EngineBuilderAddCheckPreTradeStartPolicy,
	)
}

func (b *EngineBuilder) addPreTradePolicy(policy pretrade.Policy) {
	addPolicy(
		b,
		policy,
		custompolicy.StartPreTrade,
		native.DestroyPretradePreTradePolicy,
		native.EngineBuilderAddPreTradePolicy,
	)
}

func (b *EngineBuilder) addAccountAdjustmentPolicy(policy accountadjustment.Policy) {
	addPolicy(
		b,
		policy,
		custompolicy.StartAccountAdjustment,
		native.DestroyAccountAdjustmentPolicy,
		native.EngineBuilderAddAccountAdjustmentPolicy,
	)
}

func addPolicy[
	Policy interface {
		Name() string
		Close()
	},
	Handle any,
](
	builder *EngineBuilder,
	policy Policy,
	startCustomPolicy func(Policy) (Handle, error),
	destroyPolicyHandle func(Handle),
	add func(native.EngineBuilder, Handle) error,
) {
	if builder.err != nil {
		builder.scheduleClose(policy)
		return
	}

	var handle Handle
	if builtinPolicy, isBuiltin := any(policy).(builtinPolicyWithNative[Handle]); isBuiltin {
		// Ownership of the native handle is transferred out of the built-in
		// wrapper; after this point a later Close on the wrapper is a no-op.
		handle = builtinPolicy.TakeHandle()
	} else {
		var err error
		if handle, err = startCustomPolicy(policy); err != nil {
			builder.err = newEngineBuilderPolicyAddError(err, policy.Name())
			builder.scheduleClose(policy)
			return
		}
	}
	// The caller-owned reference must always be released. On success, the
	// engine keeps its own reference and will drive the eventual destruction
	// on Stop. On failure, dropping this last reference destroys the policy
	// immediately and, for custom policies, triggers free_user_data, which in
	// turn closes the user-provided implementation.
	defer destroyPolicyHandle(handle)

	if err := add(builder.handle, handle); err != nil {
		// No scheduleClose is needed here: the deferred release above drops
		// the last reference to the policy and the native Drop path takes
		// care of closing the user implementation via free_user_data.
		builder.err = newEngineBuilderPolicyAddError(err, policy.Name())
	}
}

func (b *EngineBuilder) scheduleClose(entity interface{ Close() }) {
	b.unfinished = append(b.unfinished, entity)
}

type engineBuilderPolicyAddError struct {
	err        error
	policyName string
}

func newEngineBuilderPolicyAddError(err error, policyName string) engineBuilderPolicyAddError {
	return engineBuilderPolicyAddError{err: err, policyName: policyName}
}

func (e engineBuilderPolicyAddError) Error() string {
	return fmt.Sprintf("failed to add policy %q: %v", e.policyName, e.err)
}

type builtinPolicyWithNative[Handle any] interface {
	TakeHandle() Handle
}

//------------------------------------------------------------------------------
