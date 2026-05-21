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
// The SDK never spawns OS threads: each public method runs on the OS thread
// that invoked it. The engine handle's threading capability depends on the sync
// policy selected at builder time:
//
//   - FullSync - concurrent invocation of public methods on the same handle is
// 	   safe. Sequential cross-thread invocation is also safe.
//   - NoSync - the handle must stay on the OS thread that created the engine.
//   - AccountSync - concurrent invocation on the same handle is safe when the
//     caller pins each account to a single chain (one queue or one worker at a
//     time), so calls for the same account are never concurrent.
//
// Goroutine migration between OS threads during one SDK call is supported.
// Callbacks invoked by the SDK back into Go may run on a different OS thread
// than the goroutine that initiated the call, so callback code must not rely
// on thread-local OS state.

package openpit

import (
	"fmt"
	"runtime"

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
	runtime.KeepAlive(order)
	if err != nil {
		return nil, nil, err
	}
	if startReject != nil {
		rejectResult, err := reject.NewListFromHandle(startReject)
		native.DestroyPretradeRejectList(startReject)
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
func (e *Engine) ExecutePreTrade(
	order model.Order,
) (*pretrade.Reservation, []reject.Reject, error) {
	reservation, execRejects, err := native.EngineExecutePreTrade(e.handle, order.Handle())
	runtime.KeepAlive(order)
	if err != nil {
		return nil, nil, err
	}
	if execRejects != nil {
		rejectResult, err := reject.NewListFromHandle(execRejects)
		native.DestroyPretradeRejectList(execRejects)
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
	AccountBlocks []reject.AccountBlock
}

func (e *Engine) ApplyExecutionReport(report model.ExecutionReport) (PostTradeResult, error) {
	result, err := native.EngineApplyExecutionReport(e.handle, report.Handle())
	runtime.KeepAlive(report)
	if err != nil {
		return PostTradeResult{}, err
	}

	accountBlocks := make([]reject.AccountBlock, len(result.AccountBlocks))
	for i, b := range result.AccountBlocks {
		accountBlocks[i] = reject.NewAccountBlockFromHandle(b)
	}

	return PostTradeResult{AccountBlocks: accountBlocks}, nil
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
	runtime.KeepAlive(adjustments)
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

// Version is the SDK release version. It must match the runtime library
// version reported by the loaded native runtime; the compatibility check runs
// during initialization of the internal/native package.
const Version = loader.SDKVersion

// EngineBuilder is the initial stage of the engine builder. It only exposes
// sync-policy selection methods. Call FullSync, NoSync, or AccountSync to
// advance to SyncedEngineBuilder where policies can be registered.
type EngineBuilder struct{}

// NewEngineBuilder returns a new engine builder.
// Call FullSync, NoSync, or AccountSync to obtain a
// SyncedEngineBuilder on which policies can be registered.
func NewEngineBuilder() *EngineBuilder {
	return &EngineBuilder{}
}

// FullSync configures full thread-safety synchronization and returns a
// SyncedEngineBuilder ready to accept policies. The resulting engine handle is
// safe for concurrent invocation from multiple goroutines as well as sequential
// cross-thread access. Use this when the engine is shared across multiple
// goroutines or when goroutine migration patterns make sequential thread
// pinning impractical.
func (*EngineBuilder) FullSync() *SyncedEngineBuilder {
	return &SyncedEngineBuilder{syncPolicy: native.SyncPolicyFull}
}

// NoSync configures single-thread synchronization and returns a
// SyncedEngineBuilder ready to accept policies. The resulting engine handle
// must stay on the OS thread that created it; calls from any other OS thread
// are undefined behavior. Use this for single-threaded embeddings where
// synchronization overhead must be zero.
func (*EngineBuilder) NoSync() *SyncedEngineBuilder {
	return &SyncedEngineBuilder{syncPolicy: native.SyncPolicyLocal}
}

// AccountSync configures account-sharded synchronization and returns a
// SyncedEngineBuilder ready to accept policies. The resulting engine handle is
// safe for concurrent invocation when the caller pins each account to a single
// processing chain (one queue or one worker at a time), so calls for the same
// account are never concurrent.
func (*EngineBuilder) AccountSync() *SyncedEngineBuilder {
	return &SyncedEngineBuilder{syncPolicy: native.SyncPolicyAccount}
}

//------------------------------------------------------------------------------
// SyncedEngineBuilder

// SyncedEngineBuilder is the second stage of the engine builder chain,
// returned by EngineBuilder.FullSync, NoSync, or AccountSync. Add at least one
// policy to advance to ReadyEngineBuilder where Build is available.
type SyncedEngineBuilder struct {
	syncPolicy native.SyncPolicy
}

func (b *SyncedEngineBuilder) PreTrade(policy ...pretrade.Policy) *ReadyEngineBuilder {
	rb := newReadyEngineBuilder(b)
	for _, p := range policy {
		rb.addPreTradePolicy(p)
	}
	return rb
}

// Builtin registers a built-in entity on the builder.
func (b *SyncedEngineBuilder) Builtin(builtinReadyBuilder builtinReadyBuilder) *ReadyEngineBuilder {
	return newReadyEngineBuilder(b).Builtin(builtinReadyBuilder)
}

// ReadyEngineBuilder is the third stage of the engine builder chain, obtained
// by calling a policy-add method on SyncedEngineBuilder. Accepts additional
// policies, and builds the engine via Build.
type ReadyEngineBuilder struct {
	handle     native.EngineBuilder
	err        error
	unfinished []interface{ Close() }
}

func newReadyEngineBuilder(sb *SyncedEngineBuilder) *ReadyEngineBuilder {
	handle, err := native.CreateEngineBuilder(sb.syncPolicy)
	if err != nil {
		return &ReadyEngineBuilder{err: err}
	}
	return &ReadyEngineBuilder{handle: handle}
}

// Close releases the builder and any policies that were handed to it but
// never transferred to the engine. Safe to call more than once and safe to
// call after Build; subsequent calls are no-ops.
func (b *ReadyEngineBuilder) Close() {
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
func (b *ReadyEngineBuilder) Build() (*Engine, error) {
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

func (b *ReadyEngineBuilder) PreTrade(policy ...pretrade.Policy) *ReadyEngineBuilder {
	for _, p := range policy {
		// Every policy must go through addPolicy even after a previous failure
		// so that the builder takes responsibility for releasing it.
		b.addPreTradePolicy(p)
	}
	return b
}

// Builtin registers a built-in entity on the builder.
func (b *ReadyEngineBuilder) Builtin(builtinReadyBuilder builtinReadyBuilder) *ReadyEngineBuilder {
	if b.err != nil {
		return b
	}
	if err := builtinReadyBuilder.Build(b.handle); err != nil {
		b.err = err
	}
	return b
}

func (b *ReadyEngineBuilder) addPreTradePolicy(policy pretrade.Policy) {
	scheduleClose := func() {
		b.unfinished = append(b.unfinished, policy)
	}

	if b.err != nil {
		scheduleClose()
		return
	}

	handle, err := custompolicy.StartPreTrade(policy)
	if err != nil {
		b.err = newEngineBuilderPolicyAddError(err, policy.Name())
		scheduleClose()
		return
	}
	// The caller-owned reference must always be released. On success, the
	// engine keeps its own reference and will drive the eventual destruction
	// on Stop. On failure, dropping this last reference destroys the policy
	// immediately and, for custom policies, triggers free_user_data, which in
	// turn closes the user-provided implementation.
	defer native.DestroyPretradePreTradePolicy(handle)

	if err := native.EngineBuilderAddPreTradePolicy(b.handle, handle); err != nil {
		// No scheduleClose is needed here: the deferred release above drops
		// the last reference to the policy and the native Drop path takes
		// care of closing the user implementation via free_user_data.
		b.err = newEngineBuilderPolicyAddError(err, policy.Name())
	}
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

type builtinReadyBuilder interface {
	Build(native.EngineBuilder) error
}

//------------------------------------------------------------------------------
