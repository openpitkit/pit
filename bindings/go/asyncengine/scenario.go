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
	"fmt"
	"sync"

	"go.openpit.dev/openpit/accountadjustment"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/future"
	"go.openpit.dev/openpit/pretrade"
	"go.openpit.dev/openpit/reject"
)

// Decision resolves a pending pre-trade or drop-copy operation.
//
// Only DecisionCommit and DecisionRollback are valid return values from an
// accepted-operation hook. DecisionRollback closes the operation and ends the
// chain. An invalid value rolls the operation back and fails the chain.
type Decision uint8

const (
	decisionInvalid Decision = iota

	// DecisionCommit accepts the pending engine operation.
	DecisionCommit
	// DecisionRollback rejects the pending engine operation.
	DecisionRollback
)

// OperationResult provides the reads shared by accepted pre-trade operations.
// It has no lifecycle methods and is valid only while its accepted-operation
// hook is running.
type OperationResult interface {
	Lock() (pretrade.Lock, error)
	AccountAdjustments() ([]accountadjustment.Outcome, error)
}

// BlockingOperationResult provides the reads from an accepted operation that
// can also request an account block. It is valid only while its
// accepted-operation hook is running.
type BlockingOperationResult interface {
	OperationResult
	AccountBlock() (*reject.AccountBlock, error)
}

// DropCopyResult provides apply-time reads from an accepted drop-copy
// operation. It is valid only while the applied-operation hook is running.
type DropCopyResult interface {
	BlockingOperationResult
	IsAccountBlocked() (bool, error)
}

// OrderCheckResult provides the complete verdict of a non-mutating order
// check. Passed and Rejects remain readable after the hook returns; the
// operation reads are available only while the hook is running.
type OrderCheckResult interface {
	BlockingOperationResult
	Passed() bool
	Rejects() []reject.Reject
}

// ChainOutcomeStatus identifies how a chain ended.
type ChainOutcomeStatus uint8

const (
	// ChainOutcomeUnknown means the caller received no outcome: Await returned
	// ctx.Err() instead of a result (see Future.Await). It reports the wait,
	// not the run, and says nothing about the chain, which may still be
	// running or may already have finished. A chain never produces this status
	// itself.
	ChainOutcomeUnknown ChainOutcomeStatus = iota
	// ChainOutcomeCompleted means every ordinary chain step completed.
	ChainOutcomeCompleted
	// ChainOutcomeRejected means an engine reject or rollback ended the chain.
	ChainOutcomeRejected
	// ChainOutcomeFailed means the chain ended with Err.
	ChainOutcomeFailed
)

// String returns a human-readable representation of the status.
func (s ChainOutcomeStatus) String() string {
	switch s {
	case ChainOutcomeUnknown:
		return "unknown"
	case ChainOutcomeCompleted:
		return "completed"
	case ChainOutcomeRejected:
		return "rejected"
	case ChainOutcomeFailed:
		return "failed"
	default:
		return fmt.Sprintf("ChainOutcomeStatus(%d)", uint8(s))
	}
}

// ChainOutcome describes a chain outcome. The outcome passed to Finally
// describes the chain before that hook runs; the outcome resolved into the
// future describes the whole run, including Finally, and its Err agrees with
// the error returned by Await whenever Await returned an outcome.
//
// The zero value carries Status ChainOutcomeUnknown and is not a verdict: no
// chain produces it, Await yields it when it returns ctx.Err() instead of a
// result, and none of its fields describe a run. See the package documentation
// for reading the outcome a chain did produce.
//
// Chains are not atomic across steps, so callers track their progress in State.
// Err wraps ErrChainRetryUnsafe when the chain failed after becoming retry
// unsafe. Err is non-nil only when Status is ChainOutcomeFailed.
type ChainOutcome struct {
	Err    error
	Status ChainOutcomeStatus
	// RetryUnsafe conservatively reports that the chain entered an engine call
	// that can change state, not that a mutation is known to have happened. It
	// is meaningful on ChainOutcomeCompleted, ChainOutcomeRejected and
	// ChainOutcomeFailed, and says nothing on ChainOutcomeUnknown, where it is
	// part of a zero value the chain never produced.
	RetryUnsafe bool
}

type chainSource interface {
	model.Order | param.AccountID | param.AccountGroupID
}

// Chain starts one caller-defined lane chain. An order source routes the chain
// through its account and enables order steps. An account source enables
// account steps. An account-group source enables group steps and routes through
// that group's account-group routing key. A chain containing account-group
// membership steps instead routes through the first supplied account and cannot
// contain account-group-routing-key group steps. Begin creates the
// chain's one caller-owned State inside the selected lane.
func Chain[State any, Source chainSource](
	source Source,
	begin func(context.Context) (State, error),
) *ChainBuilder[State] {
	chain := &ChainBuilder[State]{begin: begin}
	switch source := any(source).(type) {
	case model.Order:
		chain.order = &source
	case param.AccountID:
		chain.accountID = source
	case param.AccountGroupID:
		chain.groupID = &source
	}
	return chain
}

// ChainBuilder assembles a typed, linear account or account-group lane chain.
//
// It intentionally exposes only caller state and narrow operation results to
// hooks. Hooks never receive an engine, driver, queue, future, or lifecycle
// object. The builder is not safe for concurrent mutation.
type ChainBuilder[State any] struct {
	steps                 []chainStep[State]
	begin                 func(context.Context) (State, error)
	order                 *model.Order
	accountID             param.AccountID
	groupID               *param.AccountGroupID
	finallyRunnerReturned bool
}

// PreTradeHooks handles both outcomes of ExecutePreTrade.
type PreTradeHooks[State any] struct {
	// OnRejected handles a rejected pre-trade operation and ends the chain.
	OnRejected func(context.Context, State, []reject.Reject) error
	// OnReserved resolves an accepted reservation.
	OnReserved func(context.Context, State, OperationResult) (Decision, error)
}

// DropCopyHooks handles both outcomes of ApplyDropCopy.
type DropCopyHooks[State any] struct {
	// OnRejected handles a rejected drop-copy operation and ends the chain.
	OnRejected func(context.Context, State, []reject.Reject) error
	// OnApplied resolves an accepted drop-copy operation.
	OnApplied func(context.Context, State, DropCopyResult) (Decision, error)
}

// ExecutionReportHooks prepares and handles ApplyExecutionReport.
type ExecutionReportHooks[State any] struct {
	// Report prepares the report from State inside the account lane.
	Report func(context.Context, State) (model.ExecutionReport, error)
	// OnSettled handles the canonical post-trade result.
	OnSettled func(context.Context, State, pretrade.PostTradeResult) error
}

// AccountAdjustmentHooks prepares and handles ApplyAccountAdjustment.
type AccountAdjustmentHooks[State any] struct {
	// Adjustments prepares the adjustments from State inside the account lane.
	Adjustments func(context.Context, State) ([]model.AccountAdjustment, error)
	// OnAdjusted handles the canonical adjustment result.
	OnAdjusted func(context.Context, State, accountadjustment.BatchResult) error
}

// Then runs caller work at this exact point in the accepted chain path. A
// rejection hook terminates its operation's branch, so later steps do not run.
func (c *ChainBuilder[State]) Then(
	fn func(context.Context, State) error,
) *ChainBuilder[State] {
	step := callerStep[State]{fn: fn}
	c.steps = append(c.steps, newChainStep(step.validate, step.execute))
	return c
}

// ExecutePreTrade adds the full pre-trade operation and both outcome hooks.
func (c *ChainBuilder[State]) ExecutePreTrade(
	hooks PreTradeHooks[State],
) *ChainBuilder[State] {
	step := preTradeStep[State]{hooks: hooks}
	c.steps = append(c.steps, newChainStep(step.validate, step.execute))
	return c
}

// ApplyDropCopy adds the drop-copy operation and both outcome hooks.
func (c *ChainBuilder[State]) ApplyDropCopy(
	hooks DropCopyHooks[State],
) *ChainBuilder[State] {
	step := dropCopyStep[State]{hooks: hooks}
	c.steps = append(c.steps, newChainStep(step.validate, step.execute))
	return c
}

// CheckOrder adds the asynchronous full-pipeline dry-run path for the source
// order. A failing verdict does not end the chain; the hook decides whether to
// continue or return an error. The result is closed after the hook returns.
// Start-stage dry run remains available only through the synchronous engine.
func (c *ChainBuilder[State]) CheckOrder(
	onChecked func(context.Context, State, OrderCheckResult) error,
) *ChainBuilder[State] {
	step := checkOrderStep[State]{onChecked: onChecked}
	c.steps = append(c.steps, newChainStep(step.validate, step.execute))
	return c
}

// ApplyExecutionReport prepares and applies a report, then handles its result.
// Report receives context and State only, never an operation result: derive
// result-dependent data while the pending operation can still be rolled back.
func (c *ChainBuilder[State]) ApplyExecutionReport(
	hooks ExecutionReportHooks[State],
) *ChainBuilder[State] {
	step := executionReportStep[State]{hooks: hooks}
	c.steps = append(c.steps, newChainStep(step.validate, step.execute))
	return c
}

// ApplyAccountAdjustment prepares and applies adjustments, then handles the
// canonical result.
func (c *ChainBuilder[State]) ApplyAccountAdjustment(
	hooks AccountAdjustmentHooks[State],
) *ChainBuilder[State] {
	step := accountAdjustmentStep[State]{hooks: hooks}
	c.steps = append(c.steps, newChainStep(step.validate, step.execute))
	return c
}

// Finally runs exactly once after Begin succeeds and the chain starts, on
// completion, rejection, failure, or panic. It runs after native cleanup and
// before the future resolves. If Begin fails or panics, there is no State and
// Finally does not run. Validation and queue-submission failures, plus any
// pre-start abort including hard stop and idle-queue retirement or drain,
// invoke no chain callbacks: Begin, step hooks, and Finally do not run.
// AsyncEngine observers remain independent and may still be notified. Its
// returned ChainRunner is the only way to run the chain.
func (c *ChainBuilder[State]) Finally(
	fn func(context.Context, State, ChainOutcome) error,
) *ChainRunner[State] {
	c.finallyRunnerReturned = true
	return &ChainRunner[State]{plan: c.snapshot(fn)}
}

// Run validates and queues the whole chain as one lane task, and
// resolves the future with the whole-run ChainOutcome. It runs chains without
// a terminal hook; a chain whose Finally result was discarded fails with
// ErrChainIncomplete rather than running. See the Chains package documentation
// for the outcome and cancellation contract.
func (c *ChainBuilder[State]) Run(
	ctx context.Context,
	engine *AsyncEngine,
) *future.Future[ChainOutcome] {
	return runChain(ctx, engine, func(f *future.Future[ChainOutcome]) (
		*chainTask[State], routingKey, error,
	) {
		return c.newChainTask(ctx, engine, f)
	})
}

// ChainRunner is a completed chain shape that exposes only Run.
type ChainRunner[State any] struct {
	plan chainPlan[State]
}

// Run validates and queues the completed chain as one lane task, and
// resolves the future with the whole-run ChainOutcome. See the Chains package
// documentation for the outcome and cancellation contract.
func (r *ChainRunner[State]) Run(
	ctx context.Context,
	engine *AsyncEngine,
) *future.Future[ChainOutcome] {
	return runChain(ctx, engine, func(f *future.Future[ChainOutcome]) (
		*chainTask[State], routingKey, error,
	) {
		return r.newChainTask(ctx, engine, f)
	})
}

func runChain[State any](
	ctx context.Context,
	engine *AsyncEngine,
	newTask func(*future.Future[ChainOutcome]) (*chainTask[State], routingKey, error),
) *future.Future[ChainOutcome] {
	f := future.New[ChainOutcome]()
	if engine == nil {
		err := ErrChainEngineMissing
		f.Resolve(ChainOutcome{Status: ChainOutcomeFailed, Err: err}, err)
		return f
	}
	task, key, err := newTask(f)
	if err != nil {
		f.Resolve(ChainOutcome{Status: ChainOutcomeFailed, Err: err}, err)
		return f
	}
	if err := engine.submit(ctx, key, task); err != nil {
		f.Resolve(ChainOutcome{Status: ChainOutcomeFailed, Err: err}, err)
	}
	return f
}

func (c *ChainBuilder[State]) newChainTask(
	ctx context.Context,
	engine *AsyncEngine,
	f *future.Future[ChainOutcome],
) (*chainTask[State], routingKey, error) {
	if c != nil && c.finallyRunnerReturned {
		return nil, routingKey{}, fmt.Errorf(
			"%w: discarded Chain.Finally result", ErrChainIncomplete,
		)
	}
	return newChainTask(ctx, c, engine, f)
}

func (r *ChainRunner[State]) newChainTask(
	ctx context.Context,
	engine *AsyncEngine,
	f *future.Future[ChainOutcome],
) (*chainTask[State], routingKey, error) {
	if r == nil {
		return newChainTask[State](ctx, nil, engine, f)
	}
	return newChainTaskFromPlan(ctx, r.plan, true, engine, f)
}

func newChainTask[State any](
	ctx context.Context,
	builder *ChainBuilder[State],
	engine *AsyncEngine,
	f *future.Future[ChainOutcome],
) (*chainTask[State], routingKey, error) {
	if builder == nil {
		return nil, routingKey{}, fmt.Errorf("%w: Chain", ErrChainIncomplete)
	}
	return newChainTaskFromPlan(ctx, builder.snapshot(nil), false, engine, f)
}

func newChainTaskFromPlan[State any](
	ctx context.Context,
	plan chainPlan[State],
	finallyRequired bool,
	engine *AsyncEngine,
	f *future.Future[ChainOutcome],
) (*chainTask[State], routingKey, error) {
	if finallyRequired && plan.finally == nil {
		return nil, routingKey{}, fmt.Errorf(
			"%w: Chain.Finally", ErrChainIncomplete,
		)
	}
	key, accountID, err := plan.validate()
	if err != nil {
		return nil, routingKey{}, err
	}
	plan.accountID = accountID
	task := &chainTask[State]{plan: plan, ctx: ctx, engine: engine, f: f}
	return task, key, nil
}

type chainPlan[State any] struct {
	steps     []chainStep[State]
	begin     func(context.Context) (State, error)
	finally   func(context.Context, State, ChainOutcome) error
	order     *model.Order
	accountID param.AccountID
	groupID   *param.AccountGroupID
}

func (c *ChainBuilder[State]) snapshot(
	finally func(context.Context, State, ChainOutcome) error,
) chainPlan[State] {
	steps := make([]chainStep[State], len(c.steps))
	copy(steps, c.steps)
	plan := chainPlan[State]{
		steps: steps, begin: c.begin, finally: finally, accountID: c.accountID,
	}
	if c.order != nil {
		order := *c.order
		plan.order = &order
	}
	if c.groupID != nil {
		groupID := *c.groupID
		plan.groupID = &groupID
	}
	return plan
}

type chainSourceKind uint8

const (
	chainSourceAccount chainSourceKind = iota
	chainSourceOrder
	chainSourceAccountGroup
	chainSourceAccountGroupMembership
)

func (p chainPlan[State]) validate() (
	key routingKey,
	accountID param.AccountID,
	err error,
) {
	if p.begin == nil {
		return routingKey{}, param.AccountID{}, fmt.Errorf(
			"%w: Chain.Begin", ErrChainIncomplete,
		)
	}
	accountID = p.accountID
	sourceKind := chainSourceAccount
	key = accountRoutingKey(accountID)
	if p.order != nil {
		accountID, err = extractOrderAccountID(*p.order)
		if err != nil {
			return routingKey{}, param.AccountID{}, err
		}
		sourceKind = chainSourceOrder
		key = accountRoutingKey(accountID)
	} else if p.groupID != nil {
		if err := validateAccountGroupID(*p.groupID); err != nil {
			return routingKey{}, param.AccountID{}, err
		}
		sourceKind = chainSourceAccountGroup
		key = groupRoutingKey(*p.groupID)
		var membershipRoutingKey *param.AccountID
		for _, step := range p.steps {
			if !step.accountGroupMembership {
				continue
			}
			sourceKind = chainSourceAccountGroupMembership
			if step.routingAccountID == nil {
				continue
			}
			if membershipRoutingKey == nil {
				routingAccountID := *step.routingAccountID
				membershipRoutingKey = &routingAccountID
				continue
			}
			if *membershipRoutingKey != *step.routingAccountID {
				err = ErrChainAccountMismatch
			}
		}
		if membershipRoutingKey != nil {
			key = accountRoutingKey(*membershipRoutingKey)
		}
	}
	for _, step := range p.steps {
		if validateErr := step.validate(sourceKind); validateErr != nil {
			return routingKey{}, param.AccountID{}, validateErr
		}
	}
	if err != nil {
		return routingKey{}, param.AccountID{}, err
	}
	return key, accountID, nil
}

type chainStep[State any] struct {
	validate               func(chainSourceKind) error
	accountGroupMembership bool
	routingAccountID       *param.AccountID
	execute                func(
		context.Context, *chainTask[State], State, *pendingFinalizer,
	) (bool, error)
}

func newChainStep[State any](
	validate func(chainSourceKind) error,
	execute func(
		context.Context, *chainTask[State], State, *pendingFinalizer,
	) (bool, error),
) chainStep[State] {
	return chainStep[State]{validate: validate, execute: execute}
}

type callerStep[State any] struct {
	fn func(context.Context, State) error
}

func (s callerStep[State]) validate(chainSourceKind) error {
	if s.fn == nil {
		return fmt.Errorf("%w: Chain.Then", ErrChainIncomplete)
	}
	return nil
}

func (s callerStep[State]) execute(
	ctx context.Context,
	_ *chainTask[State],
	state State,
	_ *pendingFinalizer,
) (bool, error) {
	if err := s.fn(ctx, state); err != nil {
		return false, fmt.Errorf("async chain caller hook: %w", err)
	}
	return false, nil
}

type preTradeStep[State any] struct {
	hooks PreTradeHooks[State]
}

func (s preTradeStep[State]) validate(sourceKind chainSourceKind) error {
	if sourceKind != chainSourceOrder {
		return ErrChainOrderRequired
	}
	if s.hooks.OnRejected == nil {
		return fmt.Errorf(
			"%w: Chain.ExecutePreTrade.OnRejected", ErrChainIncomplete,
		)
	}
	if s.hooks.OnReserved == nil {
		return fmt.Errorf(
			"%w: Chain.ExecutePreTrade.OnReserved", ErrChainIncomplete,
		)
	}
	return nil
}

func (s preTradeStep[State]) execute(
	ctx context.Context,
	task *chainTask[State],
	state State,
	pending *pendingFinalizer,
) (bool, error) {
	return task.executePreTrade(ctx, state, s.hooks, pending)
}

type dropCopyStep[State any] struct {
	hooks DropCopyHooks[State]
}

func (s dropCopyStep[State]) validate(sourceKind chainSourceKind) error {
	if sourceKind != chainSourceOrder {
		return ErrChainOrderRequired
	}
	if s.hooks.OnRejected == nil {
		return fmt.Errorf(
			"%w: Chain.ApplyDropCopy.OnRejected", ErrChainIncomplete,
		)
	}
	if s.hooks.OnApplied == nil {
		return fmt.Errorf(
			"%w: Chain.ApplyDropCopy.OnApplied", ErrChainIncomplete,
		)
	}
	return nil
}

func (s dropCopyStep[State]) execute(
	ctx context.Context,
	task *chainTask[State],
	state State,
	pending *pendingFinalizer,
) (bool, error) {
	return task.applyDropCopy(ctx, state, s.hooks, pending)
}

type checkOrderStep[State any] struct {
	onChecked func(context.Context, State, OrderCheckResult) error
}

func (s checkOrderStep[State]) validate(sourceKind chainSourceKind) error {
	if sourceKind != chainSourceOrder {
		return ErrChainOrderRequired
	}
	if s.onChecked == nil {
		return fmt.Errorf("%w: Chain.CheckOrder", ErrChainIncomplete)
	}
	return nil
}

func (s checkOrderStep[State]) execute(
	ctx context.Context,
	task *chainTask[State],
	state State,
	_ *pendingFinalizer,
) (bool, error) {
	return false, task.checkOrder(ctx, state, s.onChecked)
}

type executionReportStep[State any] struct {
	hooks ExecutionReportHooks[State]
}

func (s executionReportStep[State]) validate(sourceKind chainSourceKind) error {
	if sourceKind == chainSourceAccountGroup ||
		sourceKind == chainSourceAccountGroupMembership {
		return ErrChainAccountRequired
	}
	if s.hooks.Report == nil {
		return fmt.Errorf(
			"%w: Chain.ApplyExecutionReport.Report", ErrChainIncomplete,
		)
	}
	if s.hooks.OnSettled == nil {
		return fmt.Errorf(
			"%w: Chain.ApplyExecutionReport.OnSettled", ErrChainIncomplete,
		)
	}
	return nil
}

func (s executionReportStep[State]) execute(
	ctx context.Context,
	task *chainTask[State],
	state State,
	_ *pendingFinalizer,
) (bool, error) {
	return false, task.applyExecutionReport(ctx, state, s.hooks)
}

type accountAdjustmentStep[State any] struct {
	hooks AccountAdjustmentHooks[State]
}

func (s accountAdjustmentStep[State]) validate(sourceKind chainSourceKind) error {
	if sourceKind == chainSourceAccountGroup ||
		sourceKind == chainSourceAccountGroupMembership {
		return ErrChainAccountRequired
	}
	if s.hooks.Adjustments == nil {
		return fmt.Errorf(
			"%w: Chain.ApplyAccountAdjustment.Adjustments", ErrChainIncomplete,
		)
	}
	if s.hooks.OnAdjusted == nil {
		return fmt.Errorf(
			"%w: Chain.ApplyAccountAdjustment.OnAdjusted", ErrChainIncomplete,
		)
	}
	return nil
}

func (s accountAdjustmentStep[State]) execute(
	ctx context.Context,
	task *chainTask[State],
	state State,
	_ *pendingFinalizer,
) (bool, error) {
	return false, task.applyAccountAdjustment(ctx, state, s.hooks)
}

type pendingFinalizer interface {
	CommitAndClose()
	RollbackAndClose()
}

type reservationFinalizer struct {
	reservation *pretrade.Reservation
}

func (f reservationFinalizer) CommitAndClose() { f.reservation.CommitAndClose() }

func (f reservationFinalizer) RollbackAndClose() { f.reservation.RollbackAndClose() }

type dropCopyFinalizer struct {
	operation *pretrade.DropCopyOperation
}

func (f dropCopyFinalizer) CommitAndClose() { f.operation.CommitAndClose() }

func (f dropCopyFinalizer) RollbackAndClose() { f.operation.RollbackAndClose() }

type chainTask[State any] struct {
	plan        chainPlan[State]
	f           *future.Future[ChainOutcome]
	ctx         context.Context
	engine      *AsyncEngine
	retryUnsafe bool
}

func (t *chainTask[State]) run() { t.execute() }

func (t *chainTask[State]) execute() {
	var pending pendingFinalizer
	var runErr error
	var state State
	stateReady := false
	lane := &chainLaneActivity{engine: t.engine}
	lane.active.Store(true)
	laneContext := withChainLaneContext(context.WithoutCancel(t.ctx), lane)
	status := ChainOutcomeCompleted
	defer func() {
		if recovered := recover(); recovered != nil {
			runErr = chainPanicError("async chain execution panicked", recovered)
		}
		if pending != nil {
			runErr = errors.Join(runErr, rollbackPending(pending))
		}
		if runErr != nil && t.retryUnsafe {
			runErr = errors.Join(runErr, ErrChainRetryUnsafe)
		}
		outcome := t.resolvedOutcome(status, runErr)
		if stateReady && t.plan.finally != nil {
			finallyErr := t.callFinally(laneContext, state, outcome)
			if runErr == nil && finallyErr != nil && t.retryUnsafe {
				finallyErr = errors.Join(finallyErr, ErrChainRetryUnsafe)
			}
			runErr = errors.Join(runErr, finallyErr)
			// Finally is part of the run, so a hook that fails an otherwise
			// completed or rejected chain turns the resolved outcome into a
			// failure.
			outcome = t.resolvedOutcome(status, runErr)
		}
		lane.active.Store(false)
		t.f.Resolve(outcome, runErr)
	}()

	var err error
	state, err = t.plan.begin(laneContext)
	if err != nil {
		runErr = fmt.Errorf("async chain begin: %w", err)
		return
	}
	stateReady = true
	for _, step := range t.plan.steps {
		ended, err := step.execute(laneContext, t, state, &pending)
		if err != nil {
			runErr = err
			return
		}
		if ended {
			status = ChainOutcomeRejected
			return
		}
	}
}

// resolvedOutcome renders the chain state reached so far as an outcome. Any
// error is a failure whatever the steps reported, and the retry-unsafe mark
// travels with every status the chain itself produces.
func (t *chainTask[State]) resolvedOutcome(
	status ChainOutcomeStatus,
	runErr error,
) ChainOutcome {
	if runErr != nil {
		status = ChainOutcomeFailed
	}
	return ChainOutcome{
		Status: status, Err: runErr, RetryUnsafe: t.retryUnsafe,
	}
}

func (t *chainTask[State]) executePreTrade(
	ctx context.Context,
	state State,
	hooks PreTradeHooks[State],
	pending *pendingFinalizer,
) (bool, error) {
	var reservation *pretrade.Reservation
	var rejects []reject.Reject
	var err error
	t.retryUnsafe = true
	reservation, rejects, err = t.engine.driver.ExecutePreTrade(*t.plan.order)
	if reservation != nil {
		*pending = reservationFinalizer{reservation: reservation}
	}
	if shapeErr := validateDriverResult(
		reservation != nil,
		reservation != nil && !reservation.IsClosed(),
		rejects,
		err,
	); shapeErr != nil {
		return false, fmt.Errorf("async chain execute pre-trade: %w", shapeErr)
	}
	if err != nil {
		return false, fmt.Errorf("async chain execute pre-trade: %w", err)
	}
	if len(rejects) > 0 {
		if err := hooks.OnRejected(ctx, state, rejects); err != nil {
			return false, fmt.Errorf("async chain reject hook: %w", err)
		}
		return true, nil
	}
	if reservation == nil {
		return false, ErrDriverResult
	}
	result := &reservationOperationResult{
		reservation: reservation,
		available:   true,
	}
	defer result.invalidate()
	decision, err := hooks.OnReserved(ctx, state, result)
	result.invalidate()
	if err != nil {
		return false, fmt.Errorf("async chain reserved hook: %w", err)
	}
	err = finalizeDecision(decision, *pending)
	if err != nil {
		*pending = nil
		return false, err
	}
	*pending = nil
	return decision == DecisionRollback, nil
}

func (t *chainTask[State]) applyDropCopy(
	ctx context.Context,
	state State,
	hooks DropCopyHooks[State],
	pending *pendingFinalizer,
) (bool, error) {
	var operation *pretrade.DropCopyOperation
	var rejects []reject.Reject
	var err error
	t.retryUnsafe = true
	operation, rejects, err = t.engine.driver.ApplyDropCopy(*t.plan.order)
	if operation != nil {
		*pending = dropCopyFinalizer{operation: operation}
	}
	if shapeErr := validateDriverResult(
		operation != nil,
		operation != nil && !operation.IsClosed(),
		rejects,
		err,
	); shapeErr != nil {
		return false, fmt.Errorf("async chain apply drop copy: %w", shapeErr)
	}
	if err != nil {
		return false, fmt.Errorf("async chain apply drop copy: %w", err)
	}
	if len(rejects) > 0 {
		if err := hooks.OnRejected(ctx, state, rejects); err != nil {
			return false, fmt.Errorf("async chain reject hook: %w", err)
		}
		return true, nil
	}
	if operation == nil {
		return false, ErrDriverResult
	}
	result := &dropCopyOperationResult{
		operation: operation,
		available: true,
	}
	defer result.invalidate()
	decision, err := hooks.OnApplied(ctx, state, result)
	result.invalidate()
	if err != nil {
		return false, fmt.Errorf("async chain applied hook: %w", err)
	}
	err = finalizeDecision(decision, *pending)
	if err != nil {
		*pending = nil
		return false, err
	}
	*pending = nil
	return decision == DecisionRollback, nil
}

func (t *chainTask[State]) checkOrder(
	ctx context.Context,
	state State,
	onChecked func(context.Context, State, OrderCheckResult) error,
) error {
	report, err := t.engine.driver.ExecutePreTradeDryRun(*t.plan.order)
	if report != nil {
		defer report.Close()
	}
	if shapeErr := validateDriverResult(
		report != nil,
		report != nil && !report.IsClosed(),
		nil,
		err,
	); shapeErr != nil {
		return fmt.Errorf("async chain check order: %w", shapeErr)
	}
	if err != nil {
		return fmt.Errorf("async chain check order: %w", err)
	}
	passed, err := report.IsPass()
	if err != nil {
		return fmt.Errorf("async chain check order verdict: %w", err)
	}
	var rejects []reject.Reject
	if !passed {
		rejects, err = report.Rejects()
		if err != nil {
			return fmt.Errorf("async chain check order rejects: %w", err)
		}
		if rejects == nil {
			return ErrDriverResult
		}
	}
	result := &dryRunOperationResult{
		rejects: rejects, report: report, available: true, passed: passed,
	}
	defer result.invalidate()
	err = onChecked(ctx, state, result)
	result.invalidate()
	if err != nil {
		return fmt.Errorf("async chain checked hook: %w", err)
	}
	return nil
}

func (t *chainTask[State]) applyExecutionReport(
	ctx context.Context,
	state State,
	hooks ExecutionReportHooks[State],
) error {
	report, err := hooks.Report(ctx, state)
	if err != nil {
		return fmt.Errorf("async chain execution report input: %w", err)
	}
	reportAccountID, err := extractReportAccountID(report)
	if err != nil {
		return err
	}
	if reportAccountID != t.plan.accountID {
		return ErrChainAccountMismatch
	}
	t.retryUnsafe = true
	result, err := t.engine.driver.ApplyExecutionReport(report)
	if err != nil {
		return fmt.Errorf("async chain apply execution report: %w", err)
	}
	if err := hooks.OnSettled(ctx, state, result); err != nil {
		return fmt.Errorf("async chain settled hook: %w", err)
	}
	return nil
}

func (t *chainTask[State]) applyAccountAdjustment(
	ctx context.Context,
	state State,
	hooks AccountAdjustmentHooks[State],
) error {
	adjustments, err := hooks.Adjustments(ctx, state)
	if err != nil {
		return fmt.Errorf("async chain account adjustment input: %w", err)
	}
	// Driver is a public interface, so whether an empty batch changes anything
	// is a fact about one implementation, not about the contract: entering the
	// call is what marks the chain.
	t.retryUnsafe = true
	result, err := t.engine.driver.ApplyAccountAdjustment(
		t.plan.accountID, adjustments,
	)
	if err != nil {
		return fmt.Errorf("async chain apply account adjustment: %w", err)
	}
	if err := hooks.OnAdjusted(ctx, state, result); err != nil {
		return fmt.Errorf("async chain adjusted hook: %w", err)
	}
	return nil
}

func (t *chainTask[State]) abort(err error) {
	t.f.Resolve(ChainOutcome{Status: ChainOutcomeFailed, Err: err}, err)
}

func (t *chainTask[State]) callFinally(
	ctx context.Context,
	state State,
	outcome ChainOutcome,
) (err error) {
	defer func() {
		if recovered := recover(); recovered != nil {
			err = chainPanicError("async chain final hook panicked", recovered)
		}
	}()
	if err := t.plan.finally(ctx, state, outcome); err != nil {
		return fmt.Errorf("async chain final hook: %w", err)
	}
	return nil
}

func rollbackPending(pending pendingFinalizer) (err error) {
	defer func() {
		if recovered := recover(); recovered != nil {
			err = chainPanicError("async chain native cleanup panicked", recovered)
		}
	}()
	pending.RollbackAndClose()
	return nil
}

func chainPanicError(message string, recovered any) error {
	if err, ok := recovered.(error); ok {
		return fmt.Errorf("%s: %w", message, err)
	}
	return fmt.Errorf("%s: %v", message, recovered)
}

func finalizeDecision(decision Decision, pending pendingFinalizer) error {
	if pending == nil {
		return ErrDriverResult
	}
	switch decision {
	case DecisionCommit:
		pending.CommitAndClose()
		return nil
	case DecisionRollback:
		pending.RollbackAndClose()
		return nil
	default:
		pending.RollbackAndClose()
		return ErrChainInvalidDecision
	}
}

type reservationOperationResult struct {
	mu          sync.Mutex
	reservation *pretrade.Reservation
	available   bool
}

func (r *reservationOperationResult) invalidate() {
	r.mu.Lock()
	r.available = false
	r.mu.Unlock()
}

func (r *reservationOperationResult) Lock() (pretrade.Lock, error) {
	r.mu.Lock()
	defer r.mu.Unlock()
	if !r.available || r.reservation == nil {
		return pretrade.Lock{}, ErrChainResultUnavailable
	}
	return r.reservation.Lock()
}

func (r *reservationOperationResult) AccountAdjustments() ([]accountadjustment.Outcome, error) {
	r.mu.Lock()
	defer r.mu.Unlock()
	if !r.available || r.reservation == nil {
		return nil, ErrChainResultUnavailable
	}
	return r.reservation.AccountAdjustments()
}

type dropCopyOperationResult struct {
	mu        sync.Mutex
	operation *pretrade.DropCopyOperation
	available bool
}

func (r *dropCopyOperationResult) invalidate() {
	r.mu.Lock()
	r.available = false
	r.mu.Unlock()
}

func (r *dropCopyOperationResult) Lock() (pretrade.Lock, error) {
	r.mu.Lock()
	defer r.mu.Unlock()
	if !r.available || r.operation == nil {
		return pretrade.Lock{}, ErrChainResultUnavailable
	}
	return r.operation.Lock()
}

func (r *dropCopyOperationResult) AccountAdjustments() ([]accountadjustment.Outcome, error) {
	r.mu.Lock()
	defer r.mu.Unlock()
	if !r.available || r.operation == nil {
		return nil, ErrChainResultUnavailable
	}
	return r.operation.AccountAdjustments()
}

func (r *dropCopyOperationResult) AccountBlock() (*reject.AccountBlock, error) {
	r.mu.Lock()
	defer r.mu.Unlock()
	if !r.available || r.operation == nil {
		return nil, ErrChainResultUnavailable
	}
	return r.operation.AccountBlock()
}

func (r *dropCopyOperationResult) IsAccountBlocked() (bool, error) {
	r.mu.Lock()
	defer r.mu.Unlock()
	if !r.available || r.operation == nil {
		return false, ErrChainResultUnavailable
	}
	return r.operation.IsAccountBlocked()
}

type dryRunOperationResult struct {
	mu        sync.Mutex
	rejects   []reject.Reject
	report    *pretrade.DryRunReport
	available bool
	passed    bool
}

func (r *dryRunOperationResult) invalidate() {
	r.mu.Lock()
	r.available = false
	r.mu.Unlock()
}

func (r *dryRunOperationResult) Passed() bool {
	r.mu.Lock()
	defer r.mu.Unlock()
	return r.passed
}

func (r *dryRunOperationResult) Rejects() []reject.Reject {
	r.mu.Lock()
	defer r.mu.Unlock()
	return append([]reject.Reject(nil), r.rejects...)
}

func (r *dryRunOperationResult) Lock() (pretrade.Lock, error) {
	r.mu.Lock()
	defer r.mu.Unlock()
	if !r.available || r.report == nil {
		return pretrade.Lock{}, ErrChainResultUnavailable
	}
	return r.report.Lock()
}

func (r *dryRunOperationResult) AccountAdjustments() ([]accountadjustment.Outcome, error) {
	r.mu.Lock()
	defer r.mu.Unlock()
	if !r.available || r.report == nil {
		return nil, ErrChainResultUnavailable
	}
	return r.report.AccountAdjustments()
}

func (r *dryRunOperationResult) AccountBlock() (*reject.AccountBlock, error) {
	r.mu.Lock()
	defer r.mu.Unlock()
	if !r.available || r.report == nil {
		return nil, ErrChainResultUnavailable
	}
	return r.report.AccountBlock()
}
