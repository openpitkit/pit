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

package asyncengine

import (
	"context"
	"errors"
	"sync"
	"sync/atomic"

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
// keyed dispatcher chosen at build time and returns a Future that
// resolves once the worker has run the call.
//
// The facade adds whole-pipeline isolation beyond a fully synchronized direct
// engine. Operations routed by the same account ID share one queue drained by
// one worker, so their complete pipelines never overlap. Account-group
// membership changes use the first supplied account ID. Group blocking and
// group currency operations instead use an account-group key and do not join
// member-account lanes. Engine-wide operations use a third key kind. Follow-up
// calls on AsyncRequest, AsyncReservation, and AsyncDropCopyOperation re-enter
// their original account queue. Callers need no locking of their own on top.
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

type chainLaneContextKey struct{}

type chainLaneActivity struct {
	engine   *AsyncEngine
	previous *chainLaneActivity
	active   atomic.Bool
}

func withChainLaneContext(
	ctx context.Context,
	lane *chainLaneActivity,
) context.Context {
	lane.previous, _ = ctx.Value(chainLaneContextKey{}).(*chainLaneActivity)
	return context.WithValue(ctx, chainLaneContextKey{}, lane)
}

func (e *AsyncEngine) submit(
	ctx context.Context,
	key routingKey,
	task pendingTask,
) error {
	if e.reentrantLane(ctx) {
		return ErrReentrantLane
	}
	return e.strategy.submit(ctx, key, task)
}

func (e *AsyncEngine) reentrantLane(ctx context.Context) bool {
	lane, _ := ctx.Value(chainLaneContextKey{}).(*chainLaneActivity)
	for ; lane != nil; lane = lane.previous {
		if lane.engine == e && lane.active.Load() {
			return true
		}
	}
	return false
}

func validateDriverResult(
	hasHandle bool,
	hasLiveHandle bool,
	rejects []reject.Reject,
	err error,
) error {
	if hasHandle && !hasLiveHandle {
		return errors.Join(ErrDriverResult, err)
	}
	resultCount := 0
	if hasLiveHandle {
		resultCount++
	}
	if len(rejects) > 0 {
		resultCount++
	}
	if err != nil {
		resultCount++
	}
	if resultCount == 1 {
		return nil
	}
	return errors.Join(ErrDriverResult, err)
}

// StartPreTrade enqueues a start-stage call for the order. The supplied
// order must have an account ID set on its operation view; otherwise the
// returned future is resolved immediately with ErrMissingAccountID.
//
// The future mirrors the synchronous start-stage tuple: on accept it
// resolves with a non-nil *AsyncRequest and nil rejects; on a policy reject
// it resolves with a nil request and non-empty rejects; on transport error
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
	if err := e.submit(ctx, accountRoutingKey(accountID), task); err != nil {
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
	if shapeErr := validateDriverResult(
		request != nil,
		request != nil && !request.IsClosed(),
		rejects,
		err,
	); shapeErr != nil {
		if request != nil {
			request.Close()
		}
		t.f.Resolve(nil, nil, shapeErr)
		return
	}
	if err != nil {
		t.f.Resolve(nil, nil, err)
		return
	}
	if len(rejects) > 0 {
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
	if err := e.submit(ctx, accountRoutingKey(accountID), task); err != nil {
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
	resolveReservationDriverResult(
		t.f, reservation, rejects, err, t.engine, t.accountID,
	)
}

func resolveReservationDriverResult(
	f *future.Future2[*AsyncReservation, []reject.Reject],
	reservation *pretrade.Reservation,
	rejects []reject.Reject,
	err error,
	engine *AsyncEngine,
	accountID param.AccountID,
) {
	if shapeErr := validateDriverResult(
		reservation != nil,
		reservation != nil && !reservation.IsClosed(),
		rejects,
		err,
	); shapeErr != nil {
		if reservation != nil {
			reservation.Close()
		}
		f.Resolve(nil, nil, shapeErr)
		return
	}
	if err != nil {
		f.Resolve(nil, nil, err)
		return
	}
	if len(rejects) > 0 {
		f.Resolve(nil, rejects, nil)
		return
	}
	f.Resolve(newAsyncReservation(reservation, engine, accountID), nil, nil)
}

func (t *executePreTradeTask) abort(err error) { t.f.Resolve(nil, nil, err) }

// ApplyDropCopy enqueues a drop-copy call for a historical order. Existing
// account blocks and ordinary policy rejects are ignored. Account-ID
// requirements and the future's tuple shape match ExecutePreTrade, except the
// accepted value is a non-nil *AsyncDropCopyOperation and the rejects are the
// evaluation failures that prevented historical bookkeeping.
//
// Drop copy requires a readable account ID: the engine rejects an unreadable
// one with MissingRequiredField before any policy runs, so there is nothing to
// gain by queueing such an order.
//
// Serialization: the dispatcher pins every operation for one account to a
// single queue drained by one worker. This provides whole-pipeline isolation;
// callers do not need their own locking on top.
//
// If the context passed to Future2.Await is cancelled before resolution, the
// caller still owns this future. Await it again or use TryGet and close any
// eventual AsyncDropCopyOperation.
func (e *AsyncEngine) ApplyDropCopy(
	ctx context.Context,
	order model.Order,
) *future.Future2[*AsyncDropCopyOperation, []reject.Reject] {
	f := future.New2[*AsyncDropCopyOperation, []reject.Reject]()
	accountID, err := extractOrderAccountID(order)
	if err != nil {
		f.Resolve(nil, nil, err)
		return f
	}
	task := &applyDropCopyTask{f: f, engine: e, order: order, accountID: accountID}
	if err := e.submit(ctx, accountRoutingKey(accountID), task); err != nil {
		f.Resolve(nil, nil, err)
	}
	return f
}

// applyDropCopyTask carries one ApplyDropCopy call to its worker.
type applyDropCopyTask struct {
	f         *future.Future2[*AsyncDropCopyOperation, []reject.Reject]
	engine    *AsyncEngine
	order     model.Order
	accountID param.AccountID
}

func (t *applyDropCopyTask) run() {
	operation, rejects, err := t.engine.driver.ApplyDropCopy(t.order)
	if shapeErr := validateDriverResult(
		operation != nil,
		operation != nil && !operation.IsClosed(),
		rejects,
		err,
	); shapeErr != nil {
		if operation != nil {
			operation.Close()
		}
		t.f.Resolve(nil, nil, shapeErr)
		return
	}
	if err != nil {
		t.f.Resolve(nil, nil, err)
		return
	}
	if len(rejects) > 0 {
		t.f.Resolve(nil, rejects, nil)
		return
	}
	t.f.Resolve(newAsyncDropCopyOperation(operation, t.engine, t.accountID), nil, nil)
}

func (t *applyDropCopyTask) abort(err error) { t.f.Resolve(nil, nil, err) }

// ApplyExecutionReport enqueues a post-trade call for the report's
// account. The report must have an operation with an account ID set.
//
// ctx bounds only the wait for queue space. If it is canceled or expires
// before enqueue, the report is unapplied. For an execution that already
// occurred, its reservation remains unreleased and its spot funds unsettled.
// The caller MUST retry.
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
	if err := e.submit(ctx, accountRoutingKey(accountID), task); err != nil {
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
// If ctx is canceled or expires before queue space is available, the
// adjustment is not enqueued and remains unapplied. The caller MUST retry.
//
// The future resolves with the synchronous batch result.
func (e *AsyncEngine) ApplyAccountAdjustment(
	ctx context.Context,
	accountID param.AccountID,
	adjustments []model.AccountAdjustment,
) *future.Future[accountadjustment.BatchResult] {
	f := future.New[accountadjustment.BatchResult]()
	task := &applyAdjustmentTask{
		f:           f,
		engine:      e,
		adjustments: adjustments,
		accountID:   accountID,
	}
	if err := e.submit(ctx, accountRoutingKey(accountID), task); err != nil {
		f.Resolve(accountadjustment.BatchResult{}, err)
	}
	return f
}

// applyAdjustmentTask carries one ApplyAccountAdjustment call to its worker.
type applyAdjustmentTask struct {
	f           *future.Future[accountadjustment.BatchResult]
	engine      *AsyncEngine
	adjustments []model.AccountAdjustment
	accountID   param.AccountID
}

func (t *applyAdjustmentTask) run() {
	result, err := t.engine.driver.ApplyAccountAdjustment(t.accountID, t.adjustments)
	t.f.Resolve(result, err)
}

func (t *applyAdjustmentTask) abort(err error) {
	t.f.Resolve(accountadjustment.BatchResult{}, err)
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
	if err := e.submit(ctx, accountRoutingKey(accountID), task); err != nil {
		f.Resolve(struct{}{}, err)
	}
	return f
}

// Accounts returns an accessor for account-group management bound to this
// engine. Each accessor method queues behind the routing key selected for its
// account, account group, or engine-wide lane and returns a Future, mirroring
// the rest of the AsyncEngine surface.
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
// empty and ErrUninitializedAccountGroupID when group is uninitialized.
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
	if err := validateAccountGroupID(group); err != nil {
		f.Resolve(struct{}{}, err)
		return f
	}
	task := &registerGroupTask{f: f, engine: a.engine, accounts: accounts, group: group}
	if err := a.engine.submit(ctx, accountRoutingKey(accounts[0]), task); err != nil {
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
// is empty and ErrUninitializedAccountGroupID when group is uninitialized.
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
	if err := validateAccountGroupID(group); err != nil {
		f.Resolve(struct{}{}, err)
		return f
	}
	task := &unregisterGroupTask{f: f, engine: a.engine, accounts: accounts, group: group}
	if err := a.engine.submit(ctx, accountRoutingKey(accounts[0]), task); err != nil {
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
	if err := a.engine.submit(ctx, accountRoutingKey(account), task); err != nil {
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

// Block enqueues an account-block call routed through the queue of account.
//
// The future resolves with a nil error: blocking is infallible and re-blocking
// an already-blocked account keeps the first reason.
func (a AsyncAccounts) Block(
	ctx context.Context,
	account param.AccountID,
	reason string,
) *future.Future[struct{}] {
	f := future.New[struct{}]()
	task := &blockTask{f: f, engine: a.engine, account: account, reason: reason}
	if err := a.engine.submit(ctx, accountRoutingKey(account), task); err != nil {
		f.Resolve(struct{}{}, err)
	}
	return f
}

// blockTask carries one Block call to its worker.
type blockTask struct {
	f       *future.Future[struct{}]
	engine  *AsyncEngine
	account param.AccountID
	reason  string
}

func (t *blockTask) run() {
	t.engine.driver.Accounts().Block(t.account, t.reason)
	t.f.Resolve(struct{}{}, nil)
}

func (t *blockTask) abort(err error) { t.f.Resolve(struct{}{}, err) }

// Unblock enqueues an account-unblock call routed through the queue of account.
//
// The future resolves with a nil error: unblocking is infallible and unblocking
// an account that is not blocked is a no-op.
func (a AsyncAccounts) Unblock(
	ctx context.Context,
	account param.AccountID,
) *future.Future[struct{}] {
	f := future.New[struct{}]()
	task := &unblockTask{f: f, engine: a.engine, account: account}
	if err := a.engine.submit(ctx, accountRoutingKey(account), task); err != nil {
		f.Resolve(struct{}{}, err)
	}
	return f
}

// unblockTask carries one Unblock call to its worker.
type unblockTask struct {
	f       *future.Future[struct{}]
	engine  *AsyncEngine
	account param.AccountID
}

func (t *unblockTask) run() {
	t.engine.driver.Accounts().Unblock(t.account)
	t.f.Resolve(struct{}{}, nil)
}

func (t *unblockTask) abort(err error) { t.f.Resolve(struct{}{}, err) }

// UnblockAll enqueues an engine-wide unblock routed through the queue derived
// from engineWideRoutingKey, so engine-wide unblocks are serialized against
// each other.
//
// An engine-wide block never comes from Block or BlockGroup: the engine raises
// it itself, when a kill switch is reported for an execution report whose
// account cannot be read, or when a mutation finalizer registered by a custom
// policy fails - which every mutation registered from Go is.
//
// The future resolves with a nil error: unblocking is infallible, lifting an
// engine-wide block that is not active is a no-op, and accounts and groups
// blocked individually stay blocked.
func (a AsyncAccounts) UnblockAll(ctx context.Context) *future.Future[struct{}] {
	f := future.New[struct{}]()
	task := &unblockAllTask{f: f, engine: a.engine}
	if err := a.engine.submit(
		ctx, engineWideRoutingKey(), task,
	); err != nil {
		f.Resolve(struct{}{}, err)
	}
	return f
}

// unblockAllTask carries one UnblockAll call to its worker.
type unblockAllTask struct {
	f      *future.Future[struct{}]
	engine *AsyncEngine
}

func (t *unblockAllTask) run() {
	t.engine.driver.Accounts().UnblockAll()
	t.f.Resolve(struct{}{}, nil)
}

func (t *unblockAllTask) abort(err error) { t.f.Resolve(struct{}{}, err) }

// ReplaceBlockReason enqueues a block-reason replacement routed through the
// queue of account.
//
// The future resolves with a non-nil error (*reject.AccountBlockError with kind
// AccountNotBlocked) when account is not blocked, or on transport failure.
func (a AsyncAccounts) ReplaceBlockReason(
	ctx context.Context,
	account param.AccountID,
	reason string,
) *future.Future[struct{}] {
	f := future.New[struct{}]()
	task := &replaceBlockReasonTask{f: f, engine: a.engine, account: account, reason: reason}
	if err := a.engine.submit(ctx, accountRoutingKey(account), task); err != nil {
		f.Resolve(struct{}{}, err)
	}
	return f
}

// replaceBlockReasonTask carries one ReplaceBlockReason call to its worker.
type replaceBlockReasonTask struct {
	f       *future.Future[struct{}]
	engine  *AsyncEngine
	account param.AccountID
	reason  string
}

func (t *replaceBlockReasonTask) run() {
	t.f.Resolve(struct{}{}, t.engine.driver.Accounts().ReplaceBlockReason(t.account, t.reason))
}

func (t *replaceBlockReasonTask) abort(err error) { t.f.Resolve(struct{}{}, err) }

// BlockGroup enqueues a group-block call routed through the queue derived from
// group, so calls for the same group are serialized.
//
// The future resolves with a non-nil error (*reject.AccountBlockError with kind
// ReservedGroup) when group is the reserved param.DefaultAccountGroup,
// ErrUninitializedAccountGroupID when group is uninitialized, or a transport
// failure.
func (a AsyncAccounts) BlockGroup(
	ctx context.Context,
	group param.AccountGroupID,
	reason string,
) *future.Future[struct{}] {
	f := future.New[struct{}]()
	if err := validateAccountGroupID(group); err != nil {
		f.Resolve(struct{}{}, err)
		return f
	}
	task := &blockGroupTask{f: f, engine: a.engine, group: group, reason: reason}
	if err := a.engine.submit(
		ctx, groupRoutingKey(group), task,
	); err != nil {
		f.Resolve(struct{}{}, err)
	}
	return f
}

// blockGroupTask carries one BlockGroup call to its worker.
type blockGroupTask struct {
	f      *future.Future[struct{}]
	engine *AsyncEngine
	group  param.AccountGroupID
	reason string
}

func (t *blockGroupTask) run() {
	t.f.Resolve(struct{}{}, t.engine.driver.Accounts().BlockGroup(t.group, t.reason))
}

func (t *blockGroupTask) abort(err error) { t.f.Resolve(struct{}{}, err) }

// UnblockGroup enqueues a group-unblock call routed through the queue derived
// from group, so calls for the same group are serialized.
//
// The future resolves with a non-nil error (*reject.AccountBlockError with kind
// ReservedGroup) when group is the reserved param.DefaultAccountGroup,
// ErrUninitializedAccountGroupID when group is uninitialized, or a transport
// failure.
func (a AsyncAccounts) UnblockGroup(
	ctx context.Context,
	group param.AccountGroupID,
) *future.Future[struct{}] {
	f := future.New[struct{}]()
	if err := validateAccountGroupID(group); err != nil {
		f.Resolve(struct{}{}, err)
		return f
	}
	task := &unblockGroupTask{f: f, engine: a.engine, group: group}
	if err := a.engine.submit(
		ctx, groupRoutingKey(group), task,
	); err != nil {
		f.Resolve(struct{}{}, err)
	}
	return f
}

// unblockGroupTask carries one UnblockGroup call to its worker.
type unblockGroupTask struct {
	f      *future.Future[struct{}]
	engine *AsyncEngine
	group  param.AccountGroupID
}

func (t *unblockGroupTask) run() {
	t.f.Resolve(struct{}{}, t.engine.driver.Accounts().UnblockGroup(t.group))
}

func (t *unblockGroupTask) abort(err error) { t.f.Resolve(struct{}{}, err) }

// ReplaceGroupBlockReason enqueues a group block-reason replacement routed
// through the queue derived from group, so calls for the same group are
// serialized.
//
// The future resolves with a non-nil error (*reject.AccountBlockError with kind
// ReservedGroup when group is the reserved param.DefaultAccountGroup, or
// GroupNotBlocked when group is not blocked), ErrUninitializedAccountGroupID when
// group is uninitialized, or a transport failure.
func (a AsyncAccounts) ReplaceGroupBlockReason(
	ctx context.Context,
	group param.AccountGroupID,
	reason string,
) *future.Future[struct{}] {
	f := future.New[struct{}]()
	if err := validateAccountGroupID(group); err != nil {
		f.Resolve(struct{}{}, err)
		return f
	}
	task := &replaceGroupBlockReasonTask{f: f, engine: a.engine, group: group, reason: reason}
	if err := a.engine.submit(
		ctx, groupRoutingKey(group), task,
	); err != nil {
		f.Resolve(struct{}{}, err)
	}
	return f
}

// replaceGroupBlockReasonTask carries one ReplaceGroupBlockReason call to its
// worker.
type replaceGroupBlockReasonTask struct {
	f      *future.Future[struct{}]
	engine *AsyncEngine
	group  param.AccountGroupID
	reason string
}

func (t *replaceGroupBlockReasonTask) run() {
	t.f.Resolve(
		struct{}{},
		t.engine.driver.Accounts().ReplaceGroupBlockReason(t.group, t.reason),
	)
}

func (t *replaceGroupBlockReasonTask) abort(err error) { t.f.Resolve(struct{}{}, err) }

func groupRoutingKey(group param.AccountGroupID) routingKey {
	return routingKey{
		kind: routingKeyAccountGroup,
		id:   uint64(group.Handle()),
	}
}

func validateAccountGroupID(group param.AccountGroupID) error {
	if !group.IsInitialized() {
		return ErrUninitializedAccountGroupID
	}
	return nil
}

func engineWideRoutingKey() routingKey {
	return routingKey{kind: routingKeyEngineWide}
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
// Returns ErrReentrantLane when ctx is a chain hook context for this engine.
//
// After a successful return the wrapped engine has been released (when
// WithStopUnderlying was wired) and the AsyncEngine handle must not be
// used for any further operation.
func (e *AsyncEngine) StopGraceful(ctx context.Context) error {
	if e.reentrantLane(ctx) {
		return ErrReentrantLane
	}
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
// Returns ErrReentrantLane when ctx is a chain hook context for this engine.
func (e *AsyncEngine) StopHard(ctx context.Context) error {
	if e.reentrantLane(ctx) {
		return ErrReentrantLane
	}
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
