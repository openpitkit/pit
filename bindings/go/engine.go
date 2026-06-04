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
//     safe. Sequential cross-thread invocation is also safe.
//   - NoSync - the handle must stay on the OS thread that created the engine.
//   - AccountSync - concurrent invocation on the same handle is safe when the
//     caller pins each account to a single chain (one queue or one worker at a
//     time), so calls for the same account are never concurrent. The
//     asyncengine subpackage provides a ready-made dispatcher; see
//     AccountSyncReadyEngineBuilder.BuildAsync.
//
// Goroutine migration between OS threads during one SDK call is supported.
// Callbacks invoked by the SDK back into Go may run on a different OS thread
// than the goroutine that initiated the call, so callback code must not rely
// on thread-local OS state.

package openpit

import (
	"fmt"
	"runtime"

	"go.openpit.dev/openpit/accountadjustment"
	"go.openpit.dev/openpit/accounts"
	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/optional"
	"go.openpit.dev/openpit/pretrade"
	"go.openpit.dev/openpit/reject"
)

// Engine wraps a native pre-trade risk engine handle.
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
//     does not close the request - see Request.Execute);
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

// PostTradeResult holds the outcome of a post-trade operation. The canonical
// type lives in the pretrade package and is aliased here; asyncengine aliases
// the same type, so *Engine satisfies the asyncengine.Builder driver contract.
//
// On success ApplyExecutionReport returns a PostTradeResult carrying both the
// account blocks and the account-adjustment outcomes that policies produced.
type PostTradeResult = pretrade.PostTradeResult

// ApplyExecutionReport updates engine state from a completed execution report.
//
// On success it returns a PostTradeResult carrying both the account blocks and
// the account-adjustment outcomes that policies produced.
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

	var outcomes []accountadjustment.Outcome
	if result.Outcomes != nil {
		outcomes = accountadjustment.NewListFromHandle(result.Outcomes)
		native.DestroyAccountAdjustmentOutcomeList(result.Outcomes)
	}

	return PostTradeResult{
		AccountBlocks:             accountBlocks,
		AccountAdjustmentOutcomes: outcomes,
	}, nil
}

// ApplyAccountAdjustment applies balance/position adjustments for an account.
//
// On accept it returns None for the batch error and the slice of
// account-adjustment outcomes policies produced. On reject it returns the batch
// error and a nil outcome slice.
func (e *Engine) ApplyAccountAdjustment(
	accountID param.AccountID,
	adjustments []model.AccountAdjustment,
) (
	optional.Option[reject.AccountAdjustmentBatchError],
	[]accountadjustment.Outcome,
	error,
) {
	nativeAdjustments := make([]native.AccountAdjustment, len(adjustments))
	for i, adjustment := range adjustments {
		nativeAdjustments[i] = adjustment.Handle()
	}

	adjustmentReject, outcomeList, err := native.EngineApplyAccountAdjustment(
		e.handle,
		accountID.Handle(),
		nativeAdjustments,
	)
	runtime.KeepAlive(adjustments)
	if err != nil {
		return optional.None[reject.AccountAdjustmentBatchError](), nil, err
	}

	if adjustmentReject != nil {
		rejectResult, err := reject.NewAccountAdjustmentBatchErrorFromHandle(adjustmentReject)
		native.DestroyAccountAdjustmentBatchError(adjustmentReject)
		if err != nil {
			return optional.None[reject.AccountAdjustmentBatchError](),
				nil,
				fmt.Errorf("failed to create reject list for rejected account adjustment: %w", err)
		}
		return optional.Some(rejectResult), nil, nil
	}

	var outcomes []accountadjustment.Outcome
	if outcomeList != nil {
		outcomes = accountadjustment.NewListFromHandle(outcomeList)
		native.DestroyAccountAdjustmentOutcomeList(outcomeList)
	}

	return optional.None[reject.AccountAdjustmentBatchError](), outcomes, nil
}

// Accounts returns an accessor for account-group management bound to this
// engine. The returned value is a thin handle; it is valid for as long as the
// engine is.
func (e *Engine) Accounts() accounts.Accounts {
	return accounts.NewFromHandle(e.handle)
}
