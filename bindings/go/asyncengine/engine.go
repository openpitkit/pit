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

package asyncengine

import (
	"context"
	"sync"

	"go.openpit.dev/openpit/accountadjustment"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/future"
	"go.openpit.dev/openpit/pkg/optional"
	"go.openpit.dev/openpit/pretrade"
	"go.openpit.dev/openpit/reject"
)

// AsyncEngine is a concurrent facade over an AccountSync engine. Every
// public method queues the corresponding engine operation behind the
// per-account dispatcher chosen at build time and returns a Future that
// resolves once the worker has run the call.
type AsyncEngine struct {
	driver         Driver
	strategy       strategy
	stopUnderlying func()
	stopOnce       sync.Once
}

func newAsyncEngine(driver Driver, stopUnderlying func(), strategy strategy) *AsyncEngine {
	return &AsyncEngine{
		driver:         driver,
		strategy:       strategy,
		stopUnderlying: stopUnderlying,
	}
}

// StartPreTrade enqueues a start-stage call for the order. The supplied
// order must have an account ID set on its operation view; otherwise the
// returned future is resolved immediately with ErrMissingAccountID.
//
// The future mirrors the synchronous start-stage tuple: on accept it
// resolves with a non-nil *AsyncRequest and nil rejects; on a policy reject
// it resolves with a nil request and non-nil rejects; on transport error
// both are nil and err is set. Finalize an accepted request via
// AsyncRequest.Execute or AsyncRequest.Close (both route through the same
// per-account queue).
func (e *AsyncEngine) StartPreTrade(
	ctx context.Context,
	order model.Order,
) *future.Future2[*AsyncRequest, []reject.Reject] {
	f := future.New2[*AsyncRequest, []reject.Reject]()
	accountID, err := extractOrderAccountID(order)
	if err != nil {
		f.Resolve(nil, nil, err)
		return f
	}
	task := &startPreTradeTask{f: f, engine: e, order: order, accountID: accountID}
	if err := e.strategy.submit(ctx, accountID, task); err != nil {
		f.Resolve(nil, nil, err)
	}
	return f
}

// startPreTradeTask carries one StartPreTrade call to its worker.
type startPreTradeTask struct {
	f         *future.Future2[*AsyncRequest, []reject.Reject]
	engine    *AsyncEngine
	order     model.Order
	accountID param.AccountID
}

func (t *startPreTradeTask) run() {
	request, rejects, err := t.engine.driver.StartPreTrade(t.order)
	if err != nil {
		t.f.Resolve(nil, nil, err)
		return
	}
	if rejects != nil {
		t.f.Resolve(nil, rejects, nil)
		return
	}
	t.f.Resolve(newAsyncRequest(request, t.engine, t.accountID), nil, nil)
}

func (t *startPreTradeTask) abort(err error) { t.f.Resolve(nil, nil, err) }

// ExecutePreTrade enqueues a full pre-trade pipeline call. Account-ID
// requirements and the future's tuple shape match StartPreTrade, except the
// accepted value is a non-nil *AsyncReservation.
func (e *AsyncEngine) ExecutePreTrade(
	ctx context.Context,
	order model.Order,
) *future.Future2[*AsyncReservation, []reject.Reject] {
	f := future.New2[*AsyncReservation, []reject.Reject]()
	accountID, err := extractOrderAccountID(order)
	if err != nil {
		f.Resolve(nil, nil, err)
		return f
	}
	task := &executePreTradeTask{f: f, engine: e, order: order, accountID: accountID}
	if err := e.strategy.submit(ctx, accountID, task); err != nil {
		f.Resolve(nil, nil, err)
	}
	return f
}

// executePreTradeTask carries one ExecutePreTrade call to its worker.
type executePreTradeTask struct {
	f         *future.Future2[*AsyncReservation, []reject.Reject]
	engine    *AsyncEngine
	order     model.Order
	accountID param.AccountID
}

func (t *executePreTradeTask) run() {
	reservation, rejects, err := t.engine.driver.ExecutePreTrade(t.order)
	if err != nil {
		t.f.Resolve(nil, nil, err)
		return
	}
	if rejects != nil {
		t.f.Resolve(nil, rejects, nil)
		return
	}
	t.f.Resolve(newAsyncReservation(reservation, t.engine, t.accountID), nil, nil)
}

func (t *executePreTradeTask) abort(err error) { t.f.Resolve(nil, nil, err) }

// ApplyExecutionReport enqueues a post-trade call for the report's
// account. The report must have an operation with an account ID set.
func (e *AsyncEngine) ApplyExecutionReport(
	ctx context.Context,
	report model.ExecutionReport,
) *future.Future[pretrade.PostTradeResult] {
	f := future.New[pretrade.PostTradeResult]()
	accountID, err := extractReportAccountID(report)
	if err != nil {
		f.Resolve(pretrade.PostTradeResult{}, err)
		return f
	}
	task := &applyReportTask{f: f, engine: e, report: report}
	if err := e.strategy.submit(ctx, accountID, task); err != nil {
		f.Resolve(pretrade.PostTradeResult{}, err)
	}
	return f
}

// applyReportTask carries one ApplyExecutionReport call to its worker.
type applyReportTask struct {
	f      *future.Future[pretrade.PostTradeResult]
	engine *AsyncEngine
	report model.ExecutionReport
}

func (t *applyReportTask) run() {
	result, err := t.engine.driver.ApplyExecutionReport(t.report)
	t.f.Resolve(result, err)
}

func (t *applyReportTask) abort(err error) {
	t.f.Resolve(pretrade.PostTradeResult{}, err)
}

// ApplyAccountAdjustment enqueues a batch adjustment call. The account
// ID is supplied explicitly because adjustments do not carry it.
//
// The future mirrors the synchronous adjustment tuple: the first value is the
// batch reject (set when the batch was rejected, in which case the outcomes
// slice is nil), the second is the per-adjustment outcomes (set on full
// acceptance).
func (e *AsyncEngine) ApplyAccountAdjustment(
	ctx context.Context,
	accountID param.AccountID,
	adjustments []model.AccountAdjustment,
) *future.Future2[
	optional.Option[reject.AccountAdjustmentBatchError],
	[]accountadjustment.Outcome,
] {
	f := future.New2[
		optional.Option[reject.AccountAdjustmentBatchError],
		[]accountadjustment.Outcome,
	]()
	task := &applyAdjustmentTask{
		f:           f,
		engine:      e,
		adjustments: adjustments,
		accountID:   accountID,
	}
	if err := e.strategy.submit(ctx, accountID, task); err != nil {
		f.Resolve(optional.None[reject.AccountAdjustmentBatchError](), nil, err)
	}
	return f
}

// applyAdjustmentTask carries one ApplyAccountAdjustment call to its worker.
type applyAdjustmentTask struct {
	f *future.Future2[
		optional.Option[reject.AccountAdjustmentBatchError],
		[]accountadjustment.Outcome,
	]
	engine      *AsyncEngine
	adjustments []model.AccountAdjustment
	accountID   param.AccountID
}

func (t *applyAdjustmentTask) run() {
	batchReject, outcomes, err := t.engine.driver.ApplyAccountAdjustment(
		t.accountID, t.adjustments,
	)
	t.f.Resolve(batchReject, outcomes, err)
}

func (t *applyAdjustmentTask) abort(err error) {
	t.f.Resolve(optional.None[reject.AccountAdjustmentBatchError](), nil, err)
}

// Submit enqueues an arbitrary caller-supplied function into the queue
// for accountID. Use it to run client-side work atomically with respect
// to engine calls on the same account (for example, "execute this start,
// then do my side-effect, then execute"). Splitting the work between two
// submits "surfaces" between operations, letting other tasks for the
// same account interleave.
//
// The returned future resolves with the value returned by fn. If the
// task is aborted (hard stop) fn is not called and the future resolves
// with ErrStopped. ctx is observed on the submit path; once the task
// runs, fn is responsible for observing its own ctx.
func (e *AsyncEngine) Submit(
	ctx context.Context,
	accountID param.AccountID,
	fn func() error,
) *future.Future[struct{}] {
	f := future.New[struct{}]()
	task := &submitTask{f: f, fn: fn}
	if err := e.strategy.submit(ctx, accountID, task); err != nil {
		f.Resolve(struct{}{}, err)
	}
	return f
}

// Accounts returns an accessor for account-group management bound to this
// engine. Each accessor method queues its operation behind the per-account
// dispatcher and returns a Future, mirroring the rest of the AsyncEngine
// surface.
func (e *AsyncEngine) Accounts() AsyncAccounts {
	return AsyncAccounts{engine: e}
}

// AsyncAccounts manages account-group membership for an AsyncEngine. Obtain it
// from AsyncEngine.Accounts. It carries no state of its own: every call routes
// through the engine it was created from.
type AsyncAccounts struct {
	engine *AsyncEngine
}

// RegisterGroup enqueues a group-registration call routed through the queue of
// the first account in accounts. Returns ErrMissingAccountID when accounts is
// empty.
//
// The future resolves with a non-nil error on a domain conflict
// (*reject.AccountGroupError) or a transport failure.
func (a AsyncAccounts) RegisterGroup(
	ctx context.Context,
	accounts []param.AccountID,
	group param.AccountGroupID,
) *future.Future[struct{}] {
	f := future.New[struct{}]()
	if len(accounts) == 0 {
		f.Resolve(struct{}{}, ErrMissingAccountID)
		return f
	}
	task := &registerGroupTask{f: f, engine: a.engine, accounts: accounts, group: group}
	if err := a.engine.strategy.submit(ctx, accounts[0], task); err != nil {
		f.Resolve(struct{}{}, err)
	}
	return f
}

// registerGroupTask carries one RegisterGroup call to its worker.
type registerGroupTask struct {
	f        *future.Future[struct{}]
	engine   *AsyncEngine
	accounts []param.AccountID
	group    param.AccountGroupID
}

func (t *registerGroupTask) run() {
	t.f.Resolve(struct{}{}, t.engine.driver.Accounts().RegisterGroup(t.accounts, t.group))
}

func (t *registerGroupTask) abort(err error) { t.f.Resolve(struct{}{}, err) }

// UnregisterGroup enqueues a group-unregistration call routed through the queue
// of the first account in accounts. Returns ErrMissingAccountID when accounts
// is empty.
//
// The future resolves with a non-nil error on a domain conflict
// (*reject.AccountGroupError) or a transport failure.
func (a AsyncAccounts) UnregisterGroup(
	ctx context.Context,
	accounts []param.AccountID,
	group param.AccountGroupID,
) *future.Future[struct{}] {
	f := future.New[struct{}]()
	if len(accounts) == 0 {
		f.Resolve(struct{}{}, ErrMissingAccountID)
		return f
	}
	task := &unregisterGroupTask{f: f, engine: a.engine, accounts: accounts, group: group}
	if err := a.engine.strategy.submit(ctx, accounts[0], task); err != nil {
		f.Resolve(struct{}{}, err)
	}
	return f
}

// unregisterGroupTask carries one UnregisterGroup call to its worker.
type unregisterGroupTask struct {
	f        *future.Future[struct{}]
	engine   *AsyncEngine
	accounts []param.AccountID
	group    param.AccountGroupID
}

func (t *unregisterGroupTask) run() {
	t.f.Resolve(struct{}{}, t.engine.driver.Accounts().UnregisterGroup(t.accounts, t.group))
}

func (t *unregisterGroupTask) abort(err error) { t.f.Resolve(struct{}{}, err) }

// GroupOf enqueues an account-group lookup routed through the queue of account.
// The future resolves with the group id wrapped in an option, empty when the
// account is not in any group.
func (a AsyncAccounts) GroupOf(
	ctx context.Context,
	account param.AccountID,
) *future.Future[optional.Option[param.AccountGroupID]] {
	f := future.New[optional.Option[param.AccountGroupID]]()
	task := &groupOfTask{f: f, engine: a.engine, account: account}
	if err := a.engine.strategy.submit(ctx, account, task); err != nil {
		f.Resolve(optional.None[param.AccountGroupID](), err)
	}
	return f
}

// groupOfTask carries one GroupOf lookup to its worker.
type groupOfTask struct {
	f       *future.Future[optional.Option[param.AccountGroupID]]
	engine  *AsyncEngine
	account param.AccountID
}

func (t *groupOfTask) run() {
	t.f.Resolve(t.engine.driver.Accounts().GroupOf(t.account), nil)
}

func (t *groupOfTask) abort(err error) {
	t.f.Resolve(optional.None[param.AccountGroupID](), err)
}

// submitTask carries one caller-supplied Submit closure to its worker.
type submitTask struct {
	f  *future.Future[struct{}]
	fn func() error
}

func (t *submitTask) run() { t.f.Resolve(struct{}{}, t.fn()) }

func (t *submitTask) abort(err error) { t.f.Resolve(struct{}{}, err) }

// StopGraceful refuses new submissions and waits for every queued task
// to run to completion. Returns ctx.Err() if ctx fires before workers
// finish; in that case the engine is partially stopped and StopHard may
// be called to complete the shutdown.
//
// After a successful return the wrapped engine has been released (when
// WithStopUnderlying was wired) and the AsyncEngine handle must not be
// used for any further operation.
func (e *AsyncEngine) StopGraceful(ctx context.Context) error {
	err := e.strategy.stopGraceful(ctx)
	if err == nil {
		e.releaseUnderlying()
	}
	return err
}

// StopHard refuses new submissions, aborts every task that has not yet
// started with ErrStopped, and waits for the currently running task in
// each worker to finish. Returns ctx.Err() if ctx fires before the
// in-flight tasks finish.
func (e *AsyncEngine) StopHard(ctx context.Context) error {
	err := e.strategy.stopHard(ctx)
	if err == nil {
		e.releaseUnderlying()
	}
	return err
}

func (e *AsyncEngine) releaseUnderlying() {
	e.stopOnce.Do(
		func() {
			if e.stopUnderlying != nil {
				e.stopUnderlying()
			}
		},
	)
}

// extractOrderAccountID reads the account ID from the order's operation
// view without mutating the caller's order.
func extractOrderAccountID(order model.Order) (param.AccountID, error) {
	op, ok := order.Operation().Get()
	if !ok {
		return param.AccountID{}, ErrMissingAccountID
	}
	id, ok := op.AccountID().Get()
	if !ok {
		return param.AccountID{}, ErrMissingAccountID
	}
	return id, nil
}

// extractReportAccountID reads the account ID from the execution
// report's operation.
func extractReportAccountID(report model.ExecutionReport) (param.AccountID, error) {
	op, ok := report.Operation().Get()
	if !ok {
		return param.AccountID{}, ErrMissingAccountID
	}
	id, ok := op.AccountID().Get()
	if !ok {
		return param.AccountID{}, ErrMissingAccountID
	}
	return id, nil
}
