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

package asyncengine_test

import (
	"context"
	"errors"
	"fmt"
	"runtime"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	openpit "go.openpit.dev/openpit"
	"go.openpit.dev/openpit/accountadjustment"
	"go.openpit.dev/openpit/asyncengine"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/optional"
	"go.openpit.dev/openpit/pretrade"
	"go.openpit.dev/openpit/pretrade/policies"
	"go.openpit.dev/openpit/reject"
	"go.openpit.dev/openpit/tx"
)

func TestChainCheckOrderRunsWithoutCallerMutation(t *testing.T) {
	builder, err := openpit.NewEngineBuilder().
		AccountSync().
		Builtin(policies.BuildOrderValidation()).
		BuildAsync()
	if err != nil {
		t.Fatalf("BuildAsync() error = %v", err)
	}
	engine, err := builder.Dynamic().Build()
	if err != nil {
		t.Fatalf("AsyncEngine Build() error = %v", err)
	}
	defer func() {
		if err := engine.StopGraceful(context.Background()); err != nil {
			t.Fatalf("StopGraceful() error = %v", err)
		}
	}()

	order := checkChainOrder(t)
	checked := false
	var beginContext context.Context
	chain := asyncengine.Chain(order, func(ctx context.Context) (struct{}, error) {
		beginContext = ctx
		return struct{}{}, nil
	}).
		CheckOrder(func(
			ctx context.Context,
			_ struct{},
			result asyncengine.OrderCheckResult,
		) error {
			if ctx != beginContext {
				t.Error("OnChecked received another chain context")
			}
			if !result.Passed() {
				t.Error("OrderCheckResult.Passed() = false, want true")
			}
			if rejects := result.Rejects(); rejects != nil {
				t.Errorf("OrderCheckResult.Rejects() = %v, want nil", rejects)
			}
			block, err := result.AccountBlock()
			if err != nil {
				return err
			}
			if block != nil {
				t.Errorf("OrderCheckResult.AccountBlock() = %v, want nil", block)
			}
			checked = true
			return nil
		})
	if _, err := chain.Run(context.Background(), engine).Await(context.Background()); err != nil {
		t.Fatalf("chain Await() error = %v", err)
	}
	if !checked {
		t.Fatal("OnChecked() was not called")
	}
}

func TestChainFailedCheckReportsVerdictAndContinues(t *testing.T) {
	builder, err := openpit.NewEngineBuilder().
		AccountSync().
		Builtin(policies.BuildOrderValidation()).
		BuildAsync()
	if err != nil {
		t.Fatalf("BuildAsync() error = %v", err)
	}
	engine, err := builder.Dynamic().Build()
	if err != nil {
		t.Fatalf("AsyncEngine Build() error = %v", err)
	}
	defer func() {
		if err := engine.StopGraceful(context.Background()); err != nil {
			t.Fatalf("StopGraceful() error = %v", err)
		}
	}()

	order := model.NewOrder()
	operation := order.EnsureOperationView()
	operation.SetAccountID(
		param.NewAccountIDFromUint64(99224416),
	)
	checked := false
	continued := false
	chain := asyncengine.Chain(order, func(context.Context) (struct{}, error) {
		return struct{}{}, nil
	}).
		CheckOrder(func(
			_ context.Context,
			_ struct{},
			result asyncengine.OrderCheckResult,
		) error {
			checked = true
			if result.Passed() {
				t.Error("OrderCheckResult.Passed() = true, want false")
			}
			if rejects := result.Rejects(); len(rejects) == 0 {
				t.Error("OrderCheckResult.Rejects() is empty")
			}
			return nil
		}).
		Then(func(context.Context, struct{}) error {
			continued = true
			return nil
		})
	if _, err := chain.Run(context.Background(), engine).Await(
		context.Background(),
	); err != nil {
		t.Fatalf("chain Await() error = %v", err)
	}
	if !checked || !continued {
		t.Errorf("checked = %v, continued = %v, want true and true", checked, continued)
	}

	hookErr := errors.New("caller rejected failed check")
	stoppedChain := asyncengine.Chain(order, func(context.Context) (struct{}, error) {
		return struct{}{}, nil
	}).
		CheckOrder(func(
			_ context.Context,
			_ struct{},
			result asyncengine.OrderCheckResult,
		) error {
			if result.Passed() {
				t.Error("OrderCheckResult.Passed() = true, want false")
			}
			return hookErr
		}).
		Then(func(context.Context, struct{}) error {
			return errors.New("Then called after OnChecked returned an error")
		})
	if _, err := stoppedChain.Run(context.Background(), engine).Await(
		context.Background(),
	); !errors.Is(err, hookErr) {
		t.Fatalf("chain Await() error = %v, want %v", err, hookErr)
	}
}

func TestChainCheckOrderVerdictOutlivesHook(t *testing.T) {
	builder, err := openpit.NewEngineBuilder().
		AccountSync().
		Builtin(policies.BuildOrderValidation()).
		BuildAsync()
	if err != nil {
		t.Fatalf("BuildAsync() error = %v", err)
	}
	engine, err := builder.Dynamic().Build()
	if err != nil {
		t.Fatalf("AsyncEngine Build() error = %v", err)
	}
	defer func() {
		if err := engine.StopGraceful(context.Background()); err != nil {
			t.Fatalf("StopGraceful() error = %v", err)
		}
	}()

	assertCached := func(
		result asyncengine.OrderCheckResult,
		wantPassed bool,
	) error {
		if result == nil {
			return errors.New("OrderCheckResult is nil")
		}
		if got := result.Passed(); got != wantPassed {
			return fmt.Errorf("Passed() = %v, want %v", got, wantPassed)
		}
		rejects := result.Rejects()
		if wantPassed && rejects != nil {
			return fmt.Errorf("Rejects() = %v, want nil", rejects)
		}
		if !wantPassed && len(rejects) == 0 {
			return errors.New("Rejects() is empty for a failed verdict")
		}
		if _, err := result.Lock(); !errors.Is(
			err, asyncengine.ErrChainResultUnavailable,
		) {
			return fmt.Errorf(
				"Lock() error = %w, want ErrChainResultUnavailable", err,
			)
		}
		if _, err := result.AccountAdjustments(); !errors.Is(
			err, asyncengine.ErrChainResultUnavailable,
		) {
			return fmt.Errorf(
				"AccountAdjustments() error = %w, "+
					"want ErrChainResultUnavailable",
				err,
			)
		}
		if _, err := result.AccountBlock(); !errors.Is(
			err, asyncengine.ErrChainResultUnavailable,
		) {
			return fmt.Errorf(
				"AccountBlock() error = %w, want ErrChainResultUnavailable",
				err,
			)
		}
		return nil
	}

	for _, test := range []struct {
		name       string
		wantPassed bool
		order      func(*testing.T) model.Order
	}{
		{
			name:       "passed",
			wantPassed: true,
			order:      checkChainOrder,
		},
		{
			name: "rejected",
			order: func(t *testing.T) model.Order {
				t.Helper()
				order := model.NewOrder()
				operation := order.EnsureOperationView()
				operation.SetAccountID(
					param.NewAccountIDFromUint64(99224416),
				)
				return order
			},
		},
	} {
		t.Run(test.name, func(t *testing.T) {
			var escaped asyncengine.OrderCheckResult
			laterChecked := false
			chain := asyncengine.Chain(
				test.order(t),
				func(context.Context) (struct{}, error) {
					return struct{}{}, nil
				},
			).CheckOrder(func(
				_ context.Context,
				_ struct{},
				result asyncengine.OrderCheckResult,
			) error {
				escaped = result
				return nil
			}).Then(func(context.Context, struct{}) error {
				laterChecked = true
				if err := assertCached(escaped, test.wantPassed); err != nil {
					return fmt.Errorf("later step: %w", err)
				}
				return nil
			})
			waitCtx, cancel := context.WithTimeout(
				context.Background(), 5*time.Second,
			)
			defer cancel()
			if _, err := chain.Run(
				context.Background(), engine,
			).Await(waitCtx); err != nil {
				t.Fatalf("chain Await() error = %v", err)
			}
			if !laterChecked {
				t.Fatal("later chain step did not check the cached verdict")
			}
			if err := assertCached(escaped, test.wantPassed); err != nil {
				t.Errorf("after future resolution: %v", err)
			}
		})
	}
}

func TestChainSubmitImmediateReadsBeforeCommitAndPersistsAfterSettlement(
	t *testing.T,
) {
	builder, err := openpit.NewEngineBuilder().
		AccountSync().
		Builtin(policies.BuildOrderValidation()).
		BuildAsync()
	if err != nil {
		t.Fatalf("BuildAsync() error = %v", err)
	}
	engine, err := builder.Dynamic().Build()
	if err != nil {
		t.Fatalf("AsyncEngine Build() error = %v", err)
	}
	defer func() {
		if err := engine.StopGraceful(context.Background()); err != nil {
			t.Fatalf("StopGraceful() error = %v", err)
		}
	}()

	type state struct {
		report               model.ExecutionReport
		settledAccountBlocks int
		adjustmentCount      int
		lock                 pretrade.Lock
		phase                int
	}
	var chainState *state
	order := checkChainOrder(t)
	chain := asyncengine.Chain(order, func(context.Context) (*state, error) {
		chainState = &state{phase: 1}
		return chainState, nil
	}).
		ExecutePreTrade(asyncengine.PreTradeHooks[*state]{
			OnRejected: func(context.Context, *state, []reject.Reject) error {
				return errors.New("OnRejected() called for accepted reservation")
			},
			OnReserved: func(
				_ context.Context,
				got *state,
				result asyncengine.OperationResult,
			) (asyncengine.Decision, error) {
				if got.phase != 1 {
					t.Errorf("reserved phase = %d, want 1", got.phase)
				}
				lock, err := result.Lock()
				if err != nil {
					return asyncengine.DecisionRollback, err
				}
				adjustments, err := result.AccountAdjustments()
				if err != nil {
					return asyncengine.DecisionRollback, err
				}
				report := model.NewExecutionReport()
				reportOperation := report.EnsureOperationView()
				reportOperation.SetAccountID(param.NewAccountIDFromUint64(99224416))
				got.lock = lock
				got.adjustmentCount = len(adjustments)
				got.report = report
				got.phase = 2
				return asyncengine.DecisionCommit, nil
			},
		}).
		ApplyExecutionReport(asyncengine.ExecutionReportHooks[*state]{
			Report: func(
				_ context.Context,
				got *state,
			) (model.ExecutionReport, error) {
				if got.phase != 2 {
					t.Errorf("report producer phase = %d, want 2", got.phase)
				}
				got.phase = 3
				return got.report, nil
			},
			OnSettled: func(
				_ context.Context,
				got *state,
				result pretrade.PostTradeResult,
			) error {
				if got.phase != 3 {
					t.Errorf("settled phase = %d, want 3", got.phase)
				}
				got.settledAccountBlocks = len(result.AccountBlocks)
				got.phase = 4
				return nil
			},
		}).
		Then(func(_ context.Context, got *state) error {
			if got.phase != 4 {
				t.Errorf("persistence phase = %d, want 4", got.phase)
			}
			got.phase = 5
			return nil
		})
	if _, err := chain.Run(context.Background(), engine).Await(context.Background()); err != nil {
		t.Fatalf("chain Await() error = %v", err)
	}
	if chainState.phase != 5 {
		t.Errorf("final phase = %d, want 5", chainState.phase)
	}
}

func TestChainDropCopyReportsAccountBlockOnlyDuringHook(t *testing.T) {
	builder, err := openpit.NewEngineBuilder().
		AccountSync().
		Builtin(policies.BuildOrderValidation()).
		BuildAsync()
	if err != nil {
		t.Fatalf("BuildAsync() error = %v", err)
	}
	engine, err := builder.Dynamic().Build()
	if err != nil {
		t.Fatalf("AsyncEngine Build() error = %v", err)
	}
	defer func() {
		if err := engine.StopGraceful(context.Background()); err != nil {
			t.Fatalf("StopGraceful() error = %v", err)
		}
	}()

	accountID := param.NewAccountIDFromUint64(99224416)
	if _, err := engine.Accounts().Block(
		context.Background(),
		accountID,
		"existing account block",
	).Await(context.Background()); err != nil {
		t.Fatalf("Block() error = %v", err)
	}

	type state struct {
		result asyncengine.DropCopyResult
	}
	chain := asyncengine.Chain(
		checkChainOrder(t),
		func(context.Context) (*state, error) { return &state{}, nil },
	).ApplyDropCopy(asyncengine.DropCopyHooks[*state]{
		OnRejected: func(context.Context, *state, []reject.Reject) error {
			return errors.New("drop copy was rejected")
		},
		OnApplied: func(
			_ context.Context,
			got *state,
			result asyncengine.DropCopyResult,
		) (asyncengine.Decision, error) {
			blocked, err := result.IsAccountBlocked()
			if err != nil {
				return asyncengine.DecisionRollback, err
			}
			if !blocked {
				return asyncengine.DecisionRollback, errors.New(
					"IsAccountBlocked() = false, want true",
				)
			}
			got.result = result
			return asyncengine.DecisionCommit, nil
		},
	}).Then(func(_ context.Context, got *state) error {
		_, err := got.result.IsAccountBlocked()
		if !errors.Is(err, asyncengine.ErrChainResultUnavailable) {
			return errors.New(
				"escaped IsAccountBlocked() did not return ErrChainResultUnavailable",
			)
		}
		return nil
	})
	if _, err := chain.Run(context.Background(), engine).Await(
		context.Background(),
	); err != nil {
		t.Fatalf("chain Await() error = %v", err)
	}
}

func TestChainSerializesEscapedResultReadsWithFinalization(t *testing.T) {
	builder, err := openpit.NewEngineBuilder().
		AccountSync().
		Builtin(policies.BuildOrderValidation()).
		BuildAsync()
	if err != nil {
		t.Fatalf("BuildAsync() error = %v", err)
	}
	engine, err := builder.Dynamic().Build()
	if err != nil {
		t.Fatalf("AsyncEngine Build() error = %v", err)
	}
	defer func() {
		if err := engine.StopGraceful(context.Background()); err != nil {
			t.Fatalf("StopGraceful() error = %v", err)
		}
	}()

	readerStarted := make(chan struct{})
	futureResolved := make(chan struct{})
	stopReader := sync.OnceFunc(func() { close(futureResolved) })
	var readers sync.WaitGroup
	// The reader must stop on every exit path, a failed assertion included: it
	// must never keep reading a result whose handle the chain and the engine
	// shutdown below are about to release, and a spinning goroutine would burn
	// a core for the rest of the test binary.
	defer func() {
		stopReader()
		readers.Wait()
	}()
	inHookReads := make(chan error, 1)
	racingRead := make(chan error, 1)
	finalRead := make(chan error, 1)
	chain := asyncengine.Chain(
		checkChainOrder(t),
		func(context.Context) (struct{}, error) { return struct{}{}, nil },
	).ExecutePreTrade(asyncengine.PreTradeHooks[struct{}]{
		OnRejected: func(context.Context, struct{}, []reject.Reject) error {
			return errors.New("OnRejected() called for accepted reservation")
		},
		OnReserved: func(
			_ context.Context,
			_ struct{},
			result asyncengine.OperationResult,
		) (asyncengine.Decision, error) {
			readers.Add(1)
			go func() {
				defer readers.Done()
				_, inHookErr := result.Lock()
				for range 99 {
					if _, err := result.Lock(); err != nil && inHookErr == nil {
						inHookErr = err
					}
				}
				inHookReads <- inHookErr
				close(readerStarted)
				var unexpectedErr error
				readAcrossFinalization := func() {
					_, err := result.Lock()
					if err == nil || errors.Is(
						err, asyncengine.ErrChainResultUnavailable,
					) {
						return
					}
					if unexpectedErr == nil {
						unexpectedErr = err
					}
				}
				for {
					select {
					case <-futureResolved:
						readAcrossFinalization()
						racingRead <- unexpectedErr
						_, err := result.Lock()
						finalRead <- err
						return
					default:
						readAcrossFinalization()
						runtime.Gosched()
					}
				}
			}()
			<-readerStarted
			runtime.Gosched()
			return asyncengine.DecisionCommit, nil
		},
	})

	f := chain.Run(context.Background(), engine)
	select {
	case err := <-inHookReads:
		if err != nil {
			t.Errorf("escaped result in-hook Lock() error = %v, want nil", err)
		}
	case <-time.After(5 * time.Second):
		t.Fatal("escaped result reader did not finish its in-hook reads")
	}
	if _, err := f.Await(context.Background()); err != nil {
		t.Fatalf("chain Await() error = %v", err)
	}
	stopReader()
	select {
	case err := <-racingRead:
		if err != nil {
			t.Errorf(
				"escaped result Lock() across finalization error = %v, "+
					"want nil or ErrChainResultUnavailable",
				err,
			)
		}
	case <-time.After(5 * time.Second):
		t.Fatal("escaped result reader did not report finalization reads")
	}
	select {
	case err := <-finalRead:
		if !errors.Is(err, asyncengine.ErrChainResultUnavailable) {
			t.Errorf(
				"escaped result final Lock() error = %v, want ErrChainResultUnavailable",
				err,
			)
		}
	case <-time.After(5 * time.Second):
		t.Fatal("escaped result reader did not finish")
	}
}

func TestChainMarksAppliedAdjustmentHookFailureRetryUnsafe(t *testing.T) {
	builder, err := openpit.NewEngineBuilder().
		AccountSync().
		Builtin(policies.BuildOrderValidation()).
		Builtin(policies.BuildSpotFunds()).
		BuildAsync()
	if err != nil {
		t.Fatalf("BuildAsync() error = %v", err)
	}
	engine, err := builder.Dynamic().Build()
	if err != nil {
		t.Fatalf("AsyncEngine Build() error = %v", err)
	}
	defer func() {
		if err := engine.StopGraceful(context.Background()); err != nil {
			t.Fatalf("StopGraceful() error = %v", err)
		}
	}()

	accountID := param.NewAccountIDFromUint64(99224416)
	adjustment := checkChainBalanceAdjustment(t, "1000")
	hookErr := errors.New("account persistence failed")
	var outcome asyncengine.ChainOutcome
	runner := asyncengine.Chain(
		accountID,
		func(context.Context) (struct{}, error) { return struct{}{}, nil },
	).ApplyAccountAdjustment(asyncengine.AccountAdjustmentHooks[struct{}]{
		Adjustments: func(
			context.Context,
			struct{},
		) ([]model.AccountAdjustment, error) {
			return []model.AccountAdjustment{adjustment}, nil
		},
		OnAdjusted: func(
			_ context.Context,
			_ struct{},
			result accountadjustment.BatchResult,
		) error {
			if result.BatchError.IsSet() {
				return errors.New("account adjustment batch was rejected")
			}
			return hookErr
		},
	}).Finally(func(
		_ context.Context,
		_ struct{},
		got asyncengine.ChainOutcome,
	) error {
		outcome = got
		return nil
	})

	_, err = runner.Run(context.Background(), engine).Await(context.Background())
	if !errors.Is(err, hookErr) {
		t.Errorf("chain Await() error = %v, want %v", err, hookErr)
	}
	if !errors.Is(err, asyncengine.ErrChainRetryUnsafe) {
		t.Errorf("chain Await() error = %v, want ErrChainRetryUnsafe", err)
	}
	if !errors.Is(outcome.Err, hookErr) {
		t.Errorf("outcome error = %v, want %v", outcome.Err, hookErr)
	}
	if !errors.Is(outcome.Err, asyncengine.ErrChainRetryUnsafe) {
		t.Errorf("outcome error = %v, want ErrChainRetryUnsafe", outcome.Err)
	}
}

// The decision the chain applied is read from the mutation the engine
// finalized: commit and rollback each run exactly one of the probe's
// callbacks, and a decision that was never finalized runs neither. Account
// state cannot answer this on its own - a spot-funds hold is written
// synchronously during the check and its commit is a documented no-op, so a
// committed reservation and one the chain forgot to finalize leave the same
// balance behind.
//
// The account is still asserted next to the mutation: it is the only check
// that engine state matches the decision. A committed reservation keeps the
// funds it held out of the next order's reach; a rolled back one hands them
// back.
//
// Direction and funds are all this test can observe, and that is deliberate:
// the engine creates and owns the reservation, the test never holds it, and no
// exported accessor reaches it from outside, so a chain that committed without
// releasing the native handle would leave every assertion here intact. The
// release is proven in the package's internal tests, where the driver hands
// the chain a reservation the test itself created and can therefore ask
// whether it was closed.
func TestChainPreTradeDecisionFinalizesReservation(t *testing.T) {
	for _, test := range []struct {
		name          string
		decision      asyncengine.Decision
		wantCommits   int64
		wantRollbacks int64
		wantFits      bool
	}{
		{
			name:        "commit",
			decision:    asyncengine.DecisionCommit,
			wantCommits: 1,
		},
		{
			name:          "rollback",
			decision:      asyncengine.DecisionRollback,
			wantRollbacks: 1,
			wantFits:      true,
		},
	} {
		t.Run(test.name, func(t *testing.T) {
			engine, probe := newCheckChainFundedEngine(t)
			order := checkChainOrder(t)
			reserved := false
			chain := asyncengine.Chain(
				order,
				func(context.Context) (struct{}, error) { return struct{}{}, nil },
			).ExecutePreTrade(asyncengine.PreTradeHooks[struct{}]{
				OnRejected: func(context.Context, struct{}, []reject.Reject) error {
					return errors.New("OnRejected() called for accepted reservation")
				},
				OnReserved: func(
					context.Context,
					struct{},
					asyncengine.OperationResult,
				) (asyncengine.Decision, error) {
					reserved = true
					return test.decision, nil
				},
			})
			if _, err := chain.Run(context.Background(), engine).Await(
				context.Background(),
			); err != nil {
				t.Fatalf("chain Await() error = %v", err)
			}
			if !reserved {
				t.Fatal("OnReserved() was not called")
			}
			// Read the counters before the funds probe: the probe is an
			// ordinary pre-trade too, so it registers and rolls back a
			// mutation of its own.
			probe.assertCalls(t, test.wantCommits, test.wantRollbacks)
			if got := checkChainOrderFits(t, engine, order); got != test.wantFits {
				t.Errorf(
					"account still funds the next order = %v, want %v",
					got,
					test.wantFits,
				)
			}
		})
	}
}

// The same for a drop copy, and read the same way: the mutation the engine
// finalized names the decision, and the account confirms that engine state
// followed it. Only a committed operation settles against the account, and
// only a committed one lets the chain continue. Handle release is out of reach
// here for the reason given above, and is asserted internally instead.
//
// Every row here enters ApplyDropCopy, so every row must report RetryUnsafe on
// its resolved outcome, whatever status it ended with: the engine consumed
// budget and may have armed an account block before the operation existed, and
// a rejected or rolled back outcome carries no error to wrap the sentinel in.
func TestChainDropCopyDecisionFinalizesOperation(t *testing.T) {
	hookErr := errors.New("drop-copy hook failed")
	panicErr := errors.New("drop-copy hook panicked")
	var noDecision asyncengine.Decision
	for _, test := range []struct {
		name          string
		decision      asyncengine.Decision
		hookErr       error
		wantErr       error
		wantStatus    asyncengine.ChainOutcomeStatus
		panicInHook   bool
		wantFits      bool
		wantLater     int
		wantCommits   int64
		wantRollbacks int64
	}{
		{
			name:        "commit",
			decision:    asyncengine.DecisionCommit,
			wantStatus:  asyncengine.ChainOutcomeCompleted,
			wantLater:   1,
			wantCommits: 1,
		},
		{
			name:          "rollback",
			decision:      asyncengine.DecisionRollback,
			wantStatus:    asyncengine.ChainOutcomeRejected,
			wantFits:      true,
			wantRollbacks: 1,
		},
		{
			name:          "hook error",
			hookErr:       hookErr,
			wantErr:       hookErr,
			wantStatus:    asyncengine.ChainOutcomeFailed,
			wantFits:      true,
			wantRollbacks: 1,
		},
		{
			name:          "hook panic",
			panicInHook:   true,
			wantErr:       panicErr,
			wantStatus:    asyncengine.ChainOutcomeFailed,
			wantFits:      true,
			wantRollbacks: 1,
		},
		{
			name:          "invalid decision",
			decision:      asyncengine.Decision(255),
			wantErr:       asyncengine.ErrChainInvalidDecision,
			wantStatus:    asyncengine.ChainOutcomeFailed,
			wantFits:      true,
			wantRollbacks: 1,
		},
	} {
		t.Run(test.name, func(t *testing.T) {
			engine, probe := newCheckChainFundedEngine(t)
			order := checkChainOrder(t)
			laterCalls := 0
			chain := asyncengine.Chain(
				order,
				func(context.Context) (struct{}, error) { return struct{}{}, nil },
			).ApplyDropCopy(asyncengine.DropCopyHooks[struct{}]{
				OnRejected: func(context.Context, struct{}, []reject.Reject) error {
					return errors.New("OnRejected() called for accepted drop copy")
				},
				OnApplied: func(
					context.Context,
					struct{},
					asyncengine.DropCopyResult,
				) (asyncengine.Decision, error) {
					if test.panicInHook {
						panic(panicErr)
					}
					if test.hookErr != nil {
						return noDecision, test.hookErr
					}
					return test.decision, nil
				},
			}).Then(func(context.Context, struct{}) error {
				laterCalls++
				return nil
			})
			outcome, err := chain.Run(context.Background(), engine).Await(
				context.Background(),
			)
			if test.wantErr == nil && err != nil {
				t.Errorf("chain Await() error = %v, want nil", err)
			}
			if test.wantErr != nil && !errors.Is(err, test.wantErr) {
				t.Errorf("chain Await() error = %v, want %v", err, test.wantErr)
			}
			if outcome.Status != test.wantStatus {
				t.Errorf(
					"outcome status = %v, want %v",
					outcome.Status,
					test.wantStatus,
				)
			}
			if test.wantErr == nil && outcome.Err != nil {
				t.Errorf("outcome error = %v, want nil", outcome.Err)
			}
			if test.wantErr != nil && !errors.Is(outcome.Err, test.wantErr) {
				t.Errorf("outcome error = %v, want %v", outcome.Err, test.wantErr)
			}
			if !outcome.RetryUnsafe {
				t.Error("outcome RetryUnsafe = false, want true")
			}
			if laterCalls != test.wantLater {
				t.Errorf("later step calls = %d, want %d", laterCalls, test.wantLater)
			}
			// Read the counters before the funds probe: the probe is an
			// ordinary pre-trade too, so it registers and rolls back a
			// mutation of its own.
			probe.assertCalls(t, test.wantCommits, test.wantRollbacks)
			if got := checkChainOrderFits(t, engine, order); got != test.wantFits {
				t.Errorf(
					"account still funds the next order = %v, want %v",
					got,
					test.wantFits,
				)
			}
		})
	}
}

// newCheckChainFundedEngine builds an async engine that tracks spot funds and
// credits the account with exactly what one checkChainOrder settles, so the
// balance that is left reports whether the chain kept the reserved funds. The
// returned probe reports the decision the chain applied to the engine.
func newCheckChainFundedEngine(
	t *testing.T,
) (*asyncengine.AsyncEngine, *checkChainDecisionProbe) {
	t.Helper()
	probe := &checkChainDecisionProbe{}
	builder, err := openpit.NewEngineBuilder().
		AccountSync().
		Builtin(policies.BuildOrderValidation()).
		Builtin(policies.BuildSpotFunds()).
		PreTrade(probe).
		BuildAsync()
	if err != nil {
		t.Fatalf("BuildAsync() error = %v", err)
	}
	engine, err := builder.Dynamic().Build()
	if err != nil {
		t.Fatalf("AsyncEngine Build() error = %v", err)
	}
	t.Cleanup(func() {
		if err := engine.StopGraceful(context.Background()); err != nil {
			t.Fatalf("StopGraceful() error = %v", err)
		}
	})
	result, err := engine.ApplyAccountAdjustment(
		context.Background(),
		param.NewAccountIDFromUint64(99224416),
		[]model.AccountAdjustment{checkChainBalanceAdjustment(t, "18500")},
	).Await(context.Background())
	if err != nil {
		t.Fatalf("ApplyAccountAdjustment() error = %v", err)
	}
	if result.BatchError.IsSet() {
		t.Fatal("ApplyAccountAdjustment() rejected the funding batch")
	}
	return engine, probe
}

// checkChainDecisionProbe registers one mutation per pre-trade check and counts
// how the engine finalized it. The counters name the decision the chain took:
// a commit runs the commit callback, a rollback runs the rollback callback, and
// a decision the chain never finalized runs neither.
//
// The callbacks only touch atomic counters. A finalizer callback has no right
// to fail: a failure - a panic included - arms the engine kill switch and
// blocks every account, so the assertions run on the test goroutine instead.
type checkChainDecisionProbe struct {
	commits   atomic.Int64
	rollbacks atomic.Int64
}

// assertCalls reports the counters the probe collected so far.
func (p *checkChainDecisionProbe) assertCalls(
	t *testing.T,
	wantCommits, wantRollbacks int64,
) {
	t.Helper()
	if got := p.commits.Load(); got != wantCommits {
		t.Errorf("mutation commit calls = %d, want %d", got, wantCommits)
	}
	if got := p.rollbacks.Load(); got != wantRollbacks {
		t.Errorf("mutation rollback calls = %d, want %d", got, wantRollbacks)
	}
}

func (*checkChainDecisionProbe) Close() {}

func (*checkChainDecisionProbe) Name() string { return "check-chain-decision-probe" }

func (*checkChainDecisionProbe) PolicyGroupID() model.PolicyGroupID {
	return model.DefaultPolicyGroupID
}

func (*checkChainDecisionProbe) CheckPreTradeStart(
	pretrade.Context,
	model.Order,
) []reject.Reject {
	return nil
}

func (p *checkChainDecisionProbe) PerformPreTradeCheck(
	_ pretrade.Context,
	_ model.Order,
	mutations tx.Mutations,
	_ pretrade.Result,
) []reject.Reject {
	if err := mutations.Push(
		func() { p.commits.Add(1) },
		func() { p.rollbacks.Add(1) },
	); err != nil {
		return reject.NewSingleItemList(
			reject.CodeSystemUnavailable,
			"check-chain-decision-probe",
			"mutation registration failed",
			err.Error(),
			reject.ScopeOrder,
		)
	}
	return nil
}

func (*checkChainDecisionProbe) ApplyExecutionReport(
	pretrade.PostTradeContext,
	model.ExecutionReport,
	pretrade.PostTradeAdjustments,
	pretrade.PostTradePnls,
) []reject.AccountBlock {
	return nil
}

func (*checkChainDecisionProbe) ApplyAccountAdjustment(
	accountadjustment.Context,
	param.AccountID,
	model.AccountAdjustment,
	tx.Mutations,
	pretrade.AccountOutcomes,
) (pretrade.PolicyAccountAdjustmentResult, []reject.Reject) {
	return pretrade.PolicyAccountAdjustmentResult{}, nil
}

// checkChainOrderFits reports whether the account still funds the order. The
// probe reservation is rolled back, so asking costs the account nothing.
func checkChainOrderFits(
	t *testing.T,
	engine *asyncengine.AsyncEngine,
	order model.Order,
) bool {
	t.Helper()
	fits := false
	probe := asyncengine.Chain(
		order,
		func(context.Context) (struct{}, error) { return struct{}{}, nil },
	).ExecutePreTrade(asyncengine.PreTradeHooks[struct{}]{
		OnRejected: func(context.Context, struct{}, []reject.Reject) error {
			return nil
		},
		OnReserved: func(
			context.Context,
			struct{},
			asyncengine.OperationResult,
		) (asyncengine.Decision, error) {
			fits = true
			return asyncengine.DecisionRollback, nil
		},
	})
	if _, err := probe.Run(context.Background(), engine).Await(
		context.Background(),
	); err != nil {
		t.Fatalf("probe chain Await() error = %v", err)
	}
	return fits
}

func checkChainOrder(t *testing.T) model.Order {
	t.Helper()
	aapl, err := param.NewAsset("AAPL")
	if err != nil {
		t.Fatalf("NewAsset(AAPL) error = %v", err)
	}
	usd, err := param.NewAsset("USD")
	if err != nil {
		t.Fatalf("NewAsset(USD) error = %v", err)
	}
	qty, err := param.NewQuantityFromString("100")
	if err != nil {
		t.Fatalf("NewQuantityFromString() error = %v", err)
	}
	price, err := param.NewPriceFromString("185")
	if err != nil {
		t.Fatalf("NewPriceFromString() error = %v", err)
	}
	order := model.NewOrder()
	operation := order.EnsureOperationView()
	operation.SetInstrument(param.NewInstrument(aapl, usd))
	operation.SetAccountID(param.NewAccountIDFromUint64(99224416))
	operation.SetSide(param.SideBuy)
	operation.SetTradeAmount(param.NewQuantityTradeAmount(qty))
	operation.SetPrice(price)
	return order
}

func checkChainBalanceAdjustment(
	t *testing.T,
	amount string,
) model.AccountAdjustment {
	t.Helper()
	usd, err := param.NewAsset("USD")
	if err != nil {
		t.Fatalf("NewAsset(USD) error = %v", err)
	}
	total, err := param.NewPositionSizeFromString(amount)
	if err != nil {
		t.Fatalf("NewPositionSizeFromString(%q) error = %v", amount, err)
	}
	adjustment, err := model.NewAccountAdjustmentFromValues(
		model.AccountAdjustmentValues{
			BalanceOperation: optional.Some(
				model.NewAccountAdjustmentBalanceOperationFromValues(
					model.AccountAdjustmentBalanceOperationValues{
						Asset: optional.Some(usd),
					},
				),
			),
			Amount: optional.Some(
				model.NewAccountAdjustmentAmountFromValues(
					model.AccountAdjustmentAmountValues{
						Balance: optional.Some(
							param.NewAbsoluteAdjustmentAmount(total),
						),
					},
				),
			),
		},
	)
	if err != nil {
		t.Fatalf("NewAccountAdjustmentFromValues() error = %v", err)
	}
	return adjustment
}
