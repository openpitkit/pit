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
	"reflect"
	"runtime"
	"strings"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"go.openpit.dev/openpit/accountadjustment"
	"go.openpit.dev/openpit/internal/custompolicy"
	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/future"
	"go.openpit.dev/openpit/pkg/optional"
	"go.openpit.dev/openpit/pretrade"
	"go.openpit.dev/openpit/pretrade/policies"
	"go.openpit.dev/openpit/reject"
	"go.openpit.dev/openpit/tx"
)

func TestChainOutcomeStatusString(t *testing.T) {
	for _, test := range []struct {
		name   string
		status ChainOutcomeStatus
		want   string
	}{
		{
			name:   "unknown",
			status: ChainOutcomeUnknown,
			want:   "unknown",
		},
		{
			name:   "completed",
			status: ChainOutcomeCompleted,
			want:   "completed",
		},
		{
			name:   "rejected",
			status: ChainOutcomeRejected,
			want:   "rejected",
		},
		{
			name:   "failed",
			status: ChainOutcomeFailed,
			want:   "failed",
		},
		{
			name:   "undefined",
			status: ChainOutcomeStatus(7),
			want:   "ChainOutcomeStatus(7)",
		},
	} {
		t.Run(test.name, func(t *testing.T) {
			if got := test.status.String(); got != test.want {
				t.Errorf("String() = %q, want %q", got, test.want)
			}
		})
	}
}

// chainStopTimeout bounds the engine cleanup of every chain test. These tests
// park hooks on channels, so a regression can leave a task in flight, and an
// unbounded StopGraceful would then wait for it until the whole binary hits the
// go test timeout - long after the failure that explains it stopped being
// printable.
const chainStopTimeout = 5 * time.Second

// negativeProbeTimeout is only for negative-direction assertions.
const negativeProbeTimeout = 250 * time.Millisecond

func newChainEngine(t *testing.T, driver Driver) *AsyncEngine {
	t.Helper()
	engine, err := NewBuilder(driver).Dynamic().Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	t.Cleanup(func() {
		ctx, cancel := context.WithTimeout(context.Background(), chainStopTimeout)
		defer cancel()
		if err := engine.StopGraceful(ctx); err != nil {
			t.Errorf(
				"StopGraceful() error = %v, want the engine stopped within %v",
				err,
				chainStopTimeout,
			)
		}
	})
	return engine
}

// chainOrderAccountID owns every order buildChainCheckOrder produces, and
// chainOrderSettlement is exactly what one of those orders settles: quantity
// 100 at price 185.
const (
	chainOrderAccountID  uint64 = 42
	chainOrderSettlement        = "18500"
)

func buildChainCheckOrder(t *testing.T) model.Order {
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
	order := buildTestOrder(t, chainOrderAccountID)
	operation := order.EnsureOperationView()
	operation.SetInstrument(param.NewInstrument(aapl, usd))
	operation.SetSide(param.SideBuy)
	operation.SetTradeAmount(param.NewQuantityTradeAmount(qty))
	operation.SetPrice(price)
	return order
}

func newChainNativeEngine(t *testing.T) native.Engine {
	t.Helper()
	return buildChainNativeEngine(t, nil)
}

// newChainProbeEngine builds the same engine with probe registered next to
// order validation, so every reservation and drop-copy operation it produces
// carries the probe's mutation and runs its callbacks when the chain finalizes.
func newChainProbeEngine(t *testing.T, probe *chainMutationProbe) native.Engine {
	t.Helper()
	return buildChainNativeEngine(t, probe)
}

// buildChainNativeEngine builds the order-validating native engine the chain
// tests reserve against. A non-nil probe is registered as an additional
// pre-trade policy.
func buildChainNativeEngine(
	t *testing.T,
	probe *chainMutationProbe,
) native.Engine {
	t.Helper()
	builder, err := native.CreateEngineBuilder(native.SyncPolicyAccount)
	if err != nil {
		t.Fatalf("CreateEngineBuilder() error = %v", err)
	}
	if err := native.EngineBuilderAddBuiltinOrderValidation(
		builder, native.PolicyGroupID(model.DefaultPolicyGroupID),
	); err != nil {
		native.DestroyEngineBuilder(builder)
		t.Fatalf("EngineBuilderAddBuiltinOrderValidation() error = %v", err)
	}
	if probe != nil {
		addChainProbePolicy(t, builder, probe)
	}
	engine, buildErr, err := native.EngineBuilderBuild(builder)
	native.DestroyEngineBuilder(builder)
	if buildErr != nil {
		native.DestroyEngineBuildError(buildErr)
		t.Fatal("EngineBuilderBuild() returned a build error")
	}
	if err != nil {
		t.Fatalf("EngineBuilderBuild() error = %v", err)
	}
	t.Cleanup(func() { native.DestroyEngine(engine) })
	return engine
}

// addChainProbePolicy registers probe with builder. The caller-owned policy
// reference is released right away; the builder keeps its own.
func addChainProbePolicy(
	t *testing.T,
	builder native.EngineBuilder,
	probe *chainMutationProbe,
) {
	t.Helper()
	policy, err := custompolicy.StartPreTrade(probe)
	if err != nil {
		native.DestroyEngineBuilder(builder)
		t.Fatalf("StartPreTrade() error = %v", err)
	}
	addErr := native.EngineBuilderAddPreTradePolicy(builder, policy)
	native.DestroyPretradePreTradePolicy(policy)
	if addErr != nil {
		native.DestroyEngineBuilder(builder)
		t.Fatalf("EngineBuilderAddPreTradePolicy() error = %v", addErr)
	}
}

func newChainDryRunReport(t *testing.T, order model.Order) *pretrade.DryRunReport {
	t.Helper()
	engine := newChainNativeEngine(t)
	report, err := native.EngineExecutePreTradeDryRun(engine, order.Handle())
	runtime.KeepAlive(order)
	if err != nil {
		t.Fatalf("EngineExecutePreTradeDryRun() error = %v", err)
	}
	return pretrade.NewDryRunReportFromHandle(report)
}

// newChainSpotFundsEngine builds a native engine that tracks spot funds and
// credits the account with exactly what one chain order settles, so the account
// carries the decision the chain took: a committed reservation leaves nothing
// for the next order, a rolled back one leaves the balance whole.
func newChainSpotFundsEngine(t *testing.T) native.Engine {
	t.Helper()
	builder, err := native.CreateEngineBuilder(native.SyncPolicyAccount)
	if err != nil {
		t.Fatalf("CreateEngineBuilder() error = %v", err)
	}
	if err := policies.BuildSpotFunds().Build(builder); err != nil {
		native.DestroyEngineBuilder(builder)
		t.Fatalf("BuildSpotFunds() error = %v", err)
	}
	engine, buildErr, err := native.EngineBuilderBuild(builder)
	native.DestroyEngineBuilder(builder)
	if buildErr != nil {
		native.DestroyEngineBuildError(buildErr)
		t.Fatal("EngineBuilderBuild() returned a build error")
	}
	if err != nil {
		t.Fatalf("EngineBuilderBuild() error = %v", err)
	}
	t.Cleanup(func() { native.DestroyEngine(engine) })
	creditChainAccount(t, engine)
	return engine
}

// creditChainAccount sets the account balance to what one chain order settles,
// in the settlement asset those orders trade against.
func creditChainAccount(t *testing.T, engine native.Engine) {
	t.Helper()
	usd, err := param.NewAsset("USD")
	if err != nil {
		t.Fatalf("NewAsset(USD) error = %v", err)
	}
	total, err := param.NewPositionSizeFromString(chainOrderSettlement)
	if err != nil {
		t.Fatalf(
			"NewPositionSizeFromString(%q) error = %v", chainOrderSettlement, err,
		)
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
	batchErr, outcomes, blocks, err := native.EngineApplyAccountAdjustment(
		engine,
		param.NewAccountIDFromUint64(chainOrderAccountID).Handle(),
		[]native.AccountAdjustment{adjustment.Handle()},
	)
	runtime.KeepAlive(adjustment)
	if err != nil {
		t.Fatalf("EngineApplyAccountAdjustment() error = %v", err)
	}
	if batchErr != nil {
		native.DestroyAccountAdjustmentBatchError(batchErr)
		t.Fatal("EngineApplyAccountAdjustment() rejected the funding batch")
	}
	if outcomes != nil {
		native.DestroyAccountAdjustmentOutcomeList(outcomes)
	}
	if blocks != nil {
		native.DestroyPretradeAccountBlockList(blocks)
	}
}

// chainOrderFits reports whether the account still funds the order. The probe
// reservation is rolled back, so the answer costs the account nothing.
func chainOrderFits(t *testing.T, engine native.Engine, order model.Order) bool {
	t.Helper()
	reservation, rejects, err := native.EngineExecutePreTrade(engine, order.Handle())
	runtime.KeepAlive(order)
	if err != nil {
		t.Fatalf("EngineExecutePreTrade() error = %v", err)
	}
	if rejects != nil {
		native.DestroyPretradeRejectList(rejects)
		return false
	}
	pretrade.NewReservationFromHandle(reservation).RollbackAndClose()
	return true
}

func newChainReservation(t *testing.T, order model.Order) *pretrade.Reservation {
	t.Helper()
	return newChainEngineReservation(t, newChainNativeEngine(t), order)
}

// newChainEngineReservation reserves against engine and hands the reservation
// to the caller. Nothing releases a reservation on the caller's behalf, so the
// helper registers the release itself: a test that gives up between here and
// finalization would otherwise leak the handle and hold the state reserved for
// the rest of the binary. Close is idempotent, and cleanups run last in first
// out, so it lands after the async engine stopped and before the native engine
// is destroyed.
func newChainEngineReservation(
	t *testing.T,
	engine native.Engine,
	order model.Order,
) *pretrade.Reservation {
	t.Helper()
	reservation, rejects, err := native.EngineExecutePreTrade(engine, order.Handle())
	runtime.KeepAlive(order)
	if rejects != nil {
		native.DestroyPretradeRejectList(rejects)
		t.Fatal("EngineExecutePreTrade() rejected a valid order")
	}
	if err != nil {
		t.Fatalf("EngineExecutePreTrade() error = %v", err)
	}
	result := pretrade.NewReservationFromHandle(reservation)
	t.Cleanup(result.Close)
	return result
}

func newChainDropCopyOperation(
	t *testing.T,
	order model.Order,
) *pretrade.DropCopyOperation {
	t.Helper()
	return newChainEngineDropCopyOperation(t, newChainNativeEngine(t), order)
}

// newChainEngineDropCopyOperation is the drop-copy counterpart of
// newChainEngineReservation and releases the operation the same way.
func newChainEngineDropCopyOperation(
	t *testing.T,
	engine native.Engine,
	order model.Order,
) *pretrade.DropCopyOperation {
	t.Helper()
	operation, rejects, err := native.EngineApplyDropCopy(engine, order.Handle())
	runtime.KeepAlive(order)
	if rejects != nil {
		native.DestroyPretradeRejectList(rejects)
		t.Fatal("EngineApplyDropCopy() rejected a valid order")
	}
	if err != nil {
		t.Fatalf("EngineApplyDropCopy() error = %v", err)
	}
	result := pretrade.NewDropCopyOperationFromHandle(operation)
	t.Cleanup(result.Close)
	return result
}

// chainMutationProbe is a pre-trade policy that registers one mutation per
// pre-trade check, so a chain decision runs a callback of this test's own
// inside the native commit or rollback.
//
// A finalizer callback has no right to fail: a failure - a panic included - is
// reported to the core, which arms the engine kill switch and blocks every
// account. A callback may therefore only record what it saw, never assert on
// it; the assertions run on the test goroutine.
type chainMutationProbe struct {
	// onCommit runs inside the native commit of the registered mutation. It is
	// set before the chain runs and read from the chain's own goroutine.
	onCommit func()
	// onRollback is the rollback counterpart of onCommit and carries the same
	// rules. The probe applies no tentative state, so a rollback has nothing to
	// reverse; the callback is there to be counted, not to undo anything.
	onRollback func()
}

func (*chainMutationProbe) Close() {}

func (*chainMutationProbe) Name() string { return "chain-mutation-probe" }

func (*chainMutationProbe) PolicyGroupID() model.PolicyGroupID {
	return model.DefaultPolicyGroupID
}

func (*chainMutationProbe) CheckPreTradeStart(
	pretrade.Context,
	model.Order,
) []reject.Reject {
	return nil
}

func (p *chainMutationProbe) PerformPreTradeCheck(
	_ pretrade.Context,
	_ model.Order,
	mutations tx.Mutations,
	_ pretrade.Result,
) []reject.Reject {
	if err := mutations.Push(
		func() {
			if p.onCommit != nil {
				p.onCommit()
			}
		},
		func() {
			if p.onRollback != nil {
				p.onRollback()
			}
		},
	); err != nil {
		return reject.NewSingleItemList(
			reject.CodeSystemUnavailable,
			"chain-mutation-probe",
			"mutation registration failed",
			err.Error(),
			reject.ScopeOrder,
		)
	}
	return nil
}

func (*chainMutationProbe) ApplyExecutionReport(
	pretrade.PostTradeContext,
	model.ExecutionReport,
	pretrade.PostTradeAdjustments,
	pretrade.PostTradePnls,
) []reject.AccountBlock {
	return nil
}

func (*chainMutationProbe) ApplyAccountAdjustment(
	accountadjustment.Context,
	param.AccountID,
	model.AccountAdjustment,
	tx.Mutations,
	pretrade.AccountOutcomes,
) (pretrade.PolicyAccountAdjustmentResult, []reject.Reject) {
	return pretrade.PolicyAccountAdjustmentResult{}, nil
}

// assertChainInvalidatedBeforeCommit requires that the read the commit callback
// took found the escaped result already invalidated.
func assertChainInvalidatedBeforeCommit(t *testing.T, reads <-chan error) {
	t.Helper()
	select {
	case err := <-reads:
		if !errors.Is(err, ErrChainResultUnavailable) {
			t.Errorf(
				"escaped result Lock() inside the commit callback = %v, "+
					"want ErrChainResultUnavailable",
				err,
			)
		}
	default:
		t.Fatal("the mutation commit callback did not run")
	}
}

// assertChainClosedReservation requires that the chain released the native
// handle, not only that it applied the decision. Nothing else observes the
// release: a committed reservation leaves the same engine state behind as one
// the chain committed without closing, so a finalizer that dropped its Close
// would leak the handle for the life of the process while every other
// assertion still passed. Call it before any cleanup: the helpers that hand
// out an operation register a Close of their own, and it would hide the leak.
func assertChainClosedReservation(t *testing.T, reservation *pretrade.Reservation) {
	t.Helper()
	if _, err := reservation.Lock(); !errors.Is(err, pretrade.ErrReservationClosed) {
		t.Errorf(
			"reservation Lock() after the chain error = %v, "+
				"want ErrReservationClosed",
			err,
		)
	}
}

// assertChainClosedDropCopyOperation is the drop-copy counterpart of
// assertChainClosedReservation and carries the same reasoning.
func assertChainClosedDropCopyOperation(
	t *testing.T,
	operation *pretrade.DropCopyOperation,
) {
	t.Helper()
	if _, err := operation.Lock(); !errors.Is(
		err, pretrade.ErrDropCopyOperationClosed,
	) {
		t.Errorf(
			"drop-copy operation Lock() after the chain error = %v, "+
				"want ErrDropCopyOperationClosed",
			err,
		)
	}
}

// The escaped result must stop answering before the chain closes the native
// handle it reads. The read that proves it is taken from inside the mutation
// commit callback: that callback runs inside the native commit, which is
// exactly the window between result.invalidate() and the finalizer, so the
// ordering is observed rather than raced for. With the invariant held the read
// reports ErrChainResultUnavailable; with the finalizer moved first it would
// still reach the reservation.
//
// The reservation is this test's own, so the test also observes the second
// half of the commit path: the chain must close the handle, not only commit
// it. That is what a rolled-back reservation is already held to, and nothing
// downstream of a commit would notice the difference.
//
// Under that reversed ordering this detector may surface as a crash instead of
// a failed assertion, because the read re-enters the FFI while the commit
// still holds the reservation. A crash here is this defect, not flakiness.
func TestChainInvalidatesReservationResultBeforeFinalizer(t *testing.T) {
	order := buildChainCheckOrder(t)
	probe := &chainMutationProbe{}
	reservation := newChainEngineReservation(
		t, newChainProbeEngine(t, probe), order,
	)
	engine := newChainEngine(t, &chainReservationDriver{
		acceptingDriver: newAcceptingDriver(),
		reservation:     reservation,
	})
	var escaped OperationResult
	commitReads := make(chan error, 1)
	probe.onCommit = func() {
		if escaped == nil {
			commitReads <- errors.New("the commit ran without an escaped result")
			return
		}
		_, err := escaped.Lock()
		commitReads <- err
	}
	chain := Chain(
		order,
		func(context.Context) (struct{}, error) { return struct{}{}, nil },
	).ExecutePreTrade(PreTradeHooks[struct{}]{
		OnRejected: func(context.Context, struct{}, []reject.Reject) error {
			return errors.New("OnRejected() called for accepted reservation")
		},
		OnReserved: func(
			_ context.Context,
			_ struct{},
			result OperationResult,
		) (Decision, error) {
			escaped = result
			return DecisionCommit, nil
		},
	})
	if _, err := chain.Run(context.Background(), engine).Await(
		context.Background(),
	); err != nil {
		t.Fatalf("chain Await() error = %v", err)
	}
	assertChainInvalidatedBeforeCommit(t, commitReads)
	assertChainClosedReservation(t, reservation)
}

// The drop-copy counterpart of the reservation ordering invariant, proven the
// same way and with the same crash caveat, and holding the committed operation
// to the same release.
func TestChainInvalidatesDropCopyResultBeforeFinalizer(t *testing.T) {
	order := buildChainCheckOrder(t)
	probe := &chainMutationProbe{}
	operation := newChainEngineDropCopyOperation(
		t, newChainProbeEngine(t, probe), order,
	)
	engine := newChainEngine(t, &chainDropCopyDriver{
		acceptingDriver: newAcceptingDriver(),
		operation:       operation,
	})
	var escaped DropCopyResult
	commitReads := make(chan error, 1)
	probe.onCommit = func() {
		if escaped == nil {
			commitReads <- errors.New("the commit ran without an escaped result")
			return
		}
		_, err := escaped.Lock()
		commitReads <- err
	}
	chain := Chain(
		order,
		func(context.Context) (struct{}, error) { return struct{}{}, nil },
	).ApplyDropCopy(DropCopyHooks[struct{}]{
		OnRejected: func(context.Context, struct{}, []reject.Reject) error {
			return errors.New("OnRejected() called for accepted drop copy")
		},
		OnApplied: func(
			_ context.Context,
			_ struct{},
			result DropCopyResult,
		) (Decision, error) {
			escaped = result
			return DecisionCommit, nil
		},
	})
	if _, err := chain.Run(context.Background(), engine).Await(
		context.Background(),
	); err != nil {
		t.Fatalf("chain Await() error = %v", err)
	}
	assertChainInvalidatedBeforeCommit(t, commitReads)
	assertChainClosedDropCopyOperation(t, operation)
}

// A hook error must both unwind the reservation and release its handle: the
// account keeps the funds the reservation held, and the reservation itself
// answers every accessor with ErrReservationClosed.
func TestChainHookFailureRollsBackAndCloses(t *testing.T) {
	order := buildChainCheckOrder(t)
	nativeEngine := newChainSpotFundsEngine(t)
	reservation := newChainEngineReservation(t, nativeEngine, order)
	engine := newChainEngine(t, &chainReservationDriver{
		acceptingDriver: newAcceptingDriver(),
		reservation:     reservation,
	})
	hookErr := errors.New("store write failed")
	chain := Chain(
		order,
		func(context.Context) (struct{}, error) { return struct{}{}, nil },
	).ExecutePreTrade(PreTradeHooks[struct{}]{
		OnRejected: func(context.Context, struct{}, []reject.Reject) error {
			return errors.New("OnRejected() called for accepted reservation")
		},
		OnReserved: func(context.Context, struct{}, OperationResult) (Decision, error) {
			return decisionInvalid, hookErr
		},
	})
	if _, err := chain.Run(context.Background(), engine).Await(
		context.Background(),
	); !errors.Is(err, hookErr) {
		t.Fatalf("chain Await() error = %v, want %v", err, hookErr)
	}
	if _, err := reservation.Lock(); !errors.Is(err, pretrade.ErrReservationClosed) {
		t.Errorf(
			"reservation Lock() error = %v, want ErrReservationClosed",
			err,
		)
	}
	if !chainOrderFits(t, nativeEngine, order) {
		t.Error("the account lost the funds of a reservation that failed its hook")
	}
}

// A panicking hook unwinds exactly as a returned error does.
func TestChainHookPanicRollsBackAndCloses(t *testing.T) {
	order := buildChainCheckOrder(t)
	nativeEngine := newChainSpotFundsEngine(t)
	reservation := newChainEngineReservation(t, nativeEngine, order)
	engine := newChainEngine(t, &chainReservationDriver{
		acceptingDriver: newAcceptingDriver(),
		reservation:     reservation,
	})
	chain := Chain(
		order,
		func(context.Context) (struct{}, error) { return struct{}{}, nil },
	).ExecutePreTrade(PreTradeHooks[struct{}]{
		OnRejected: func(context.Context, struct{}, []reject.Reject) error {
			return errors.New("OnRejected() called for accepted reservation")
		},
		OnReserved: func(context.Context, struct{}, OperationResult) (Decision, error) {
			panic("store callback panic")
		},
	})
	_, err := chain.Run(context.Background(), engine).Await(context.Background())
	if err == nil {
		t.Fatal("chain Await() error = nil, want execution panic error")
	}
	assertChainExecutionPanicLabel(t, err)
	if !strings.Contains(err.Error(), "store callback panic") {
		t.Errorf("chain Await() error = %v, want recovered panic value", err)
	}
	if _, err := reservation.Lock(); !errors.Is(err, pretrade.ErrReservationClosed) {
		t.Errorf(
			"reservation Lock() error = %v, want ErrReservationClosed",
			err,
		)
	}
	if !chainOrderFits(t, nativeEngine, order) {
		t.Error("the account lost the funds of a reservation whose hook panicked")
	}
}

type chainRejectDriver struct {
	*acceptingDriver
}

func (*chainRejectDriver) ExecutePreTrade(
	model.Order,
) (*pretrade.Reservation, []reject.Reject, error) {
	return nil, []reject.Reject{{}}, nil
}

type chainReservationDriver struct {
	*acceptingDriver
	reservation *pretrade.Reservation
}

func (d *chainReservationDriver) ExecutePreTrade(
	model.Order,
) (*pretrade.Reservation, []reject.Reject, error) {
	return d.reservation, nil, nil
}

type chainDropCopyDriver struct {
	*acceptingDriver
	operation *pretrade.DropCopyOperation
}

func (d *chainDropCopyDriver) ApplyDropCopy(
	model.Order,
) (*pretrade.DropCopyOperation, []reject.Reject, error) {
	return d.operation, nil, nil
}

func TestAsyncReservationAccessorsMatchReservationLifetime(t *testing.T) {
	order := buildChainCheckOrder(t)
	inner := newChainEngineReservation(t, newChainNativeEngine(t), order)
	engine := newChainEngine(t, &chainReservationDriver{
		acceptingDriver: newAcceptingDriver(),
		reservation:     inner,
	})
	waitCtx, cancel := context.WithTimeout(
		context.Background(), chainStopTimeout,
	)
	defer cancel()
	reservation, rejects, err := engine.ExecutePreTrade(
		context.Background(), order,
	).Await(waitCtx)
	if err != nil {
		t.Fatalf("ExecutePreTrade() error = %v", err)
	}
	if rejects != nil {
		t.Fatalf("ExecutePreTrade() rejects = %v, want nil", rejects)
	}
	if reservation == nil {
		t.Fatal("ExecutePreTrade() reservation = nil, want non-nil")
	}
	if _, err := reservation.Lock(); err != nil {
		t.Errorf("Lock() error = %v, want nil", err)
	}
	if _, err := reservation.AccountAdjustments(); err != nil {
		t.Errorf("AccountAdjustments() error = %v, want nil", err)
	}
	if _, err := reservation.Close(context.Background()).Await(waitCtx); err != nil {
		t.Fatalf("Close() error = %v", err)
	}
	if _, err := reservation.Lock(); !errors.Is(err, pretrade.ErrReservationClosed) {
		t.Errorf("Lock() after Close error = %v, want ErrReservationClosed", err)
	}
	if _, err := reservation.AccountAdjustments(); !errors.Is(
		err, pretrade.ErrReservationClosed,
	) {
		t.Errorf(
			"AccountAdjustments() after Close error = %v, "+
				"want ErrReservationClosed",
			err,
		)
	}
}

func TestAsyncReservationAccessorsFailDuringNativeFinalization(t *testing.T) {
	for _, test := range []struct {
		name   string
		access func(*AsyncReservation) error
	}{
		{
			name: "lock",
			access: func(reservation *AsyncReservation) error {
				_, err := reservation.Lock()
				return err
			},
		},
		{
			name: "account adjustments",
			access: func(reservation *AsyncReservation) error {
				_, err := reservation.AccountAdjustments()
				return err
			},
		},
	} {
		t.Run(test.name, func(t *testing.T) {
			accessErr := make(chan error, 1)
			var reservation *AsyncReservation
			probe := &chainMutationProbe{
				onRollback: func() {
					accessErr <- test.access(reservation)
				},
			}
			order := buildChainCheckOrder(t)
			inner := newChainEngineReservation(
				t, newChainProbeEngine(t, probe), order,
			)
			engine := newChainEngine(t, &chainReservationDriver{
				acceptingDriver: newAcceptingDriver(),
				reservation:     inner,
			})
			reservation = newAsyncReservation(
				inner, engine, param.NewAccountIDFromUint64(chainOrderAccountID),
			)
			waitCtx, cancel := context.WithTimeout(
				context.Background(), chainStopTimeout,
			)
			defer cancel()
			closeFuture := reservation.Close(context.Background())
			if _, err := closeFuture.Await(waitCtx); err != nil {
				t.Fatalf("Close() error = %v", err)
			}
			select {
			case err := <-accessErr:
				if !errors.Is(err, ErrFinalizationInProgress) {
					t.Errorf(
						"accessor inside native rollback error = %v, "+
							"want ErrFinalizationInProgress",
						err,
					)
				}
			case <-waitCtx.Done():
				t.Fatal("native rollback callback did not finish its accessor")
			}
			if err := test.access(reservation); !errors.Is(
				err, pretrade.ErrReservationClosed,
			) {
				t.Errorf(
					"accessor after Close error = %v, want ErrReservationClosed",
					err,
				)
			}
		})
	}
}

func TestAsyncDropCopyAccessorsFailDuringNativeFinalization(t *testing.T) {
	for _, test := range []struct {
		name   string
		access func(*AsyncDropCopyOperation) error
	}{
		{
			name: "lock",
			access: func(operation *AsyncDropCopyOperation) error {
				_, err := operation.Lock()
				return err
			},
		},
		{
			name: "account adjustments",
			access: func(operation *AsyncDropCopyOperation) error {
				_, err := operation.AccountAdjustments()
				return err
			},
		},
		{
			name: "account block",
			access: func(operation *AsyncDropCopyOperation) error {
				_, err := operation.AccountBlock()
				return err
			},
		},
		{
			name: "is account blocked",
			access: func(operation *AsyncDropCopyOperation) error {
				_, err := operation.IsAccountBlocked()
				return err
			},
		},
	} {
		t.Run(test.name, func(t *testing.T) {
			accessErr := make(chan error, 1)
			var operation *AsyncDropCopyOperation
			probe := &chainMutationProbe{
				onRollback: func() {
					accessErr <- test.access(operation)
				},
			}
			order := buildChainCheckOrder(t)
			inner := newChainEngineDropCopyOperation(
				t, newChainProbeEngine(t, probe), order,
			)
			engine := newChainEngine(t, newAcceptingDriver())
			operation = newAsyncDropCopyOperation(
				inner, engine, param.NewAccountIDFromUint64(chainOrderAccountID),
			)
			waitCtx, cancel := context.WithTimeout(
				context.Background(), chainStopTimeout,
			)
			defer cancel()
			if _, err := operation.Close(context.Background()).Await(waitCtx); err != nil {
				t.Fatalf("Close() error = %v", err)
			}
			select {
			case err := <-accessErr:
				if !errors.Is(err, ErrFinalizationInProgress) {
					t.Errorf(
						"accessor inside native rollback error = %v, "+
							"want ErrFinalizationInProgress",
						err,
					)
				}
			case <-waitCtx.Done():
				t.Fatal("native rollback callback did not finish its accessor")
			}
			if err := test.access(operation); !errors.Is(
				err, pretrade.ErrDropCopyOperationClosed,
			) {
				t.Errorf(
					"accessor after Close error = %v, "+
						"want ErrDropCopyOperationClosed",
					err,
				)
			}
		})
	}
}

func TestAsyncReservationReadDistinguishesFinalizingFromClosed(t *testing.T) {
	for _, test := range []struct {
		name        string
		setCallback func(*chainMutationProbe, func())
		finalize    func(*AsyncReservation) *future.Future[struct{}]
	}{
		{
			name: "commit",
			setCallback: func(probe *chainMutationProbe, callback func()) {
				probe.onCommit = callback
			},
			finalize: func(
				reservation *AsyncReservation,
			) *future.Future[struct{}] {
				return reservation.Commit(context.Background())
			},
		},
		{
			name: "rollback",
			setCallback: func(probe *chainMutationProbe, callback func()) {
				probe.onRollback = callback
			},
			finalize: func(
				reservation *AsyncReservation,
			) *future.Future[struct{}] {
				return reservation.Rollback(context.Background())
			},
		},
	} {
		t.Run(test.name, func(t *testing.T) {
			finalizerEntered := make(chan struct{})
			releaseFinalizer := make(chan struct{})
			released := false
			release := func() {
				if !released {
					close(releaseFinalizer)
					released = true
				}
			}
			defer release()
			probe := &chainMutationProbe{}
			test.setCallback(probe, func() {
				close(finalizerEntered)
				<-releaseFinalizer
			})
			order := buildChainCheckOrder(t)
			inner := newChainEngineReservation(
				t, newChainProbeEngine(t, probe), order,
			)
			engine := newChainEngine(t, newAcceptingDriver())
			reservation := newAsyncReservation(
				inner,
				engine,
				param.NewAccountIDFromUint64(chainOrderAccountID),
			)
			waitCtx, cancel := context.WithTimeout(
				context.Background(), chainStopTimeout,
			)
			defer cancel()
			finalization := test.finalize(reservation)
			select {
			case <-finalizerEntered:
			case <-waitCtx.Done():
				t.Fatal("native finalization callback did not start")
			}
			readDone := make(chan error, 1)
			go func() {
				_, err := reservation.Lock()
				readDone <- err
			}()
			select {
			case err := <-readDone:
				if !errors.Is(err, ErrFinalizationInProgress) {
					t.Errorf(
						"Lock() during finalization error = %v, "+
							"want ErrFinalizationInProgress",
						err,
					)
				}
			case <-waitCtx.Done():
				t.Fatal("Lock() blocked during native finalization")
			}
			release()
			if _, err := finalization.Await(waitCtx); err != nil {
				t.Fatalf("%s error = %v", test.name, err)
			}
			if _, err := reservation.Lock(); err != nil {
				t.Errorf("Lock() after %s error = %v, want nil", test.name, err)
			}
			if _, err := reservation.Close(context.Background()).Await(
				waitCtx,
			); err != nil {
				t.Fatalf("Close() error = %v", err)
			}
			if _, err := reservation.Lock(); !errors.Is(
				err, pretrade.ErrReservationClosed,
			) {
				t.Errorf(
					"Lock() after Close error = %v, want ErrReservationClosed",
					err,
				)
			}
		})
	}
}

func TestAsyncDropCopyReadDistinguishesFinalizingFromClosed(t *testing.T) {
	for _, test := range []struct {
		name        string
		setCallback func(*chainMutationProbe, func())
		finalize    func(*AsyncDropCopyOperation) *future.Future[struct{}]
	}{
		{
			name: "commit",
			setCallback: func(probe *chainMutationProbe, callback func()) {
				probe.onCommit = callback
			},
			finalize: func(
				operation *AsyncDropCopyOperation,
			) *future.Future[struct{}] {
				return operation.Commit(context.Background())
			},
		},
		{
			name: "rollback",
			setCallback: func(probe *chainMutationProbe, callback func()) {
				probe.onRollback = callback
			},
			finalize: func(
				operation *AsyncDropCopyOperation,
			) *future.Future[struct{}] {
				return operation.Rollback(context.Background())
			},
		},
	} {
		t.Run(test.name, func(t *testing.T) {
			finalizerEntered := make(chan struct{})
			releaseFinalizer := make(chan struct{})
			released := false
			release := func() {
				if !released {
					close(releaseFinalizer)
					released = true
				}
			}
			defer release()
			probe := &chainMutationProbe{}
			test.setCallback(probe, func() {
				close(finalizerEntered)
				<-releaseFinalizer
			})
			order := buildChainCheckOrder(t)
			inner := newChainEngineDropCopyOperation(
				t, newChainProbeEngine(t, probe), order,
			)
			engine := newChainEngine(t, newAcceptingDriver())
			operation := newAsyncDropCopyOperation(
				inner,
				engine,
				param.NewAccountIDFromUint64(chainOrderAccountID),
			)
			waitCtx, cancel := context.WithTimeout(
				context.Background(), chainStopTimeout,
			)
			defer cancel()
			finalization := test.finalize(operation)
			select {
			case <-finalizerEntered:
			case <-waitCtx.Done():
				t.Fatal("native finalization callback did not start")
			}
			readDone := make(chan error, 1)
			go func() {
				_, err := operation.Lock()
				readDone <- err
			}()
			select {
			case err := <-readDone:
				if !errors.Is(err, ErrFinalizationInProgress) {
					t.Errorf(
						"Lock() during finalization error = %v, "+
							"want ErrFinalizationInProgress",
						err,
					)
				}
			case <-waitCtx.Done():
				t.Fatal("Lock() blocked during native finalization")
			}
			release()
			if _, err := finalization.Await(waitCtx); err != nil {
				t.Fatalf("%s error = %v", test.name, err)
			}
			if _, err := operation.Lock(); err != nil {
				t.Errorf("Lock() after %s error = %v, want nil", test.name, err)
			}
			if _, err := operation.Close(context.Background()).Await(
				waitCtx,
			); err != nil {
				t.Fatalf("Close() error = %v", err)
			}
			if _, err := operation.Lock(); !errors.Is(
				err, pretrade.ErrDropCopyOperationClosed,
			) {
				t.Errorf(
					"Lock() after Close error = %v, "+
						"want ErrDropCopyOperationClosed",
					err,
				)
			}
		})
	}
}

func TestAsyncReservationLifecycleClosedReadWinsWhileFinalizing(t *testing.T) {
	reservation := &AsyncReservation{}
	reservation.mu.Lock()
	reservation.closed = true
	reservation.state = reservationFinalizing
	reservation.mu.Unlock()

	if err := reservation.beginRead(); !errors.Is(
		err, pretrade.ErrReservationClosed,
	) {
		t.Errorf(
			"beginRead() error = %v, want ErrReservationClosed",
			err,
		)
	}
}

func TestAsyncDropCopyLifecycleClosedReadWinsWhileFinalizing(t *testing.T) {
	operation := &AsyncDropCopyOperation{}
	operation.mu.Lock()
	operation.closed = true
	operation.state = dropCopyOperationFinalizing
	operation.mu.Unlock()

	if err := operation.beginRead(); !errors.Is(
		err, pretrade.ErrDropCopyOperationClosed,
	) {
		t.Errorf(
			"beginRead() error = %v, want ErrDropCopyOperationClosed",
			err,
		)
	}
}

func assertAdmittedReadDelaysFinalization(
	t *testing.T,
	methodMu *sync.Mutex,
	readerCount func() int,
	read func() error,
	finalize func(context.Context) *future.Future[struct{}],
) {
	t.Helper()
	waitCtx, cancel := context.WithTimeout(
		context.Background(), chainStopTimeout,
	)
	defer cancel()

	// This invariant has no public slow-read seam, so methodMu only gates the
	// public read after admission, the reader-count probe only observes it, and
	// both leave the lifecycle path unchanged.
	methodMu.Lock()
	gateHeld := true
	defer func() {
		if gateHeld {
			methodMu.Unlock()
		}
	}()
	readDone := make(chan error, 1)
	go func() {
		readDone <- read()
	}()
	for readerCount() != 1 {
		select {
		case err := <-readDone:
			t.Fatalf("read returned before admission with error %v", err)
		case <-waitCtx.Done():
			t.Fatalf("read admission never happened: %v", waitCtx.Err())
		default:
			runtime.Gosched()
		}
	}

	finalization := finalize(waitCtx)
	probe := time.NewTimer(negativeProbeTimeout)
	defer probe.Stop()
	select {
	case <-finalization.Wait():
		t.Error("finalizer resolved while the admitted read was parked")
	case <-probe.C:
	}

	methodMu.Unlock()
	gateHeld = false
	select {
	case err := <-readDone:
		if err != nil {
			t.Errorf("admitted read error = %v, want nil", err)
		}
	case <-waitCtx.Done():
		t.Errorf("admitted read did not finish: %v", waitCtx.Err())
	}
	if _, err := finalization.Await(waitCtx); err != nil {
		t.Errorf("finalizer error = %v, want nil", err)
	}
}

func TestAsyncReservationAdmittedReadDelaysFinalization(t *testing.T) {
	order := buildChainCheckOrder(t)
	engine := newChainEngine(t, newAcceptingDriver())
	reservation := newAsyncReservation(
		newChainReservation(t, order),
		engine,
		param.NewAccountIDFromUint64(chainOrderAccountID),
	)
	assertAdmittedReadDelaysFinalization(
		t,
		&reservation.methodMu,
		func() int {
			reservation.mu.Lock()
			defer reservation.mu.Unlock()
			return reservation.readers
		},
		func() error {
			_, err := reservation.Lock()
			return err
		},
		reservation.Close,
	)
}

func TestAsyncDropCopyAdmittedReadDelaysFinalization(t *testing.T) {
	order := buildChainCheckOrder(t)
	engine := newChainEngine(t, newAcceptingDriver())
	operation := newAsyncDropCopyOperation(
		newChainDropCopyOperation(t, order),
		engine,
		param.NewAccountIDFromUint64(chainOrderAccountID),
	)
	assertAdmittedReadDelaysFinalization(
		t,
		&operation.methodMu,
		func() int {
			operation.mu.Lock()
			defer operation.mu.Unlock()
			return operation.readers
		},
		func() error {
			_, err := operation.Lock()
			return err
		},
		operation.Close,
	)
}

func assertConcurrentFinalizersWithRacingReads(
	t *testing.T,
	closedErr error,
	lock func() (pretrade.Lock, error),
	accountAdjustments func() ([]accountadjustment.Outcome, error),
	commit, rollback, closeResource func(context.Context) *future.Future[struct{}],
) {
	t.Helper()
	waitCtx, cancel := context.WithTimeout(
		context.Background(), chainStopTimeout,
	)

	start := make(chan struct{})
	const racingReadCount = 8
	readerDone := make(chan struct{}, racingReadCount)
	type finalizerResult struct {
		name string
		err  error
	}
	finalizerDone := make(chan finalizerResult, 3)
	readersRemaining := racingReadCount
	finalizersRemaining := 3
	defer func() {
		cancel()
		for ; finalizersRemaining > 0; finalizersRemaining-- {
			<-finalizerDone
		}
		for ; readersRemaining > 0; readersRemaining-- {
			<-readerDone
		}
	}()

	readLock := func() error {
		_, err := lock()
		return err
	}
	readAdjustments := func() error {
		_, err := accountAdjustments()
		return err
	}
	reads := [...]func() error{readLock, readAdjustments}
	for index := 0; index < racingReadCount; index++ {
		read := reads[index%len(reads)]
		go func(read func() error) {
			defer func() { readerDone <- struct{}{} }()
			select {
			case <-start:
				for {
					select {
					case <-waitCtx.Done():
						return
					default:
					}
					if errors.Is(read(), closedErr) {
						return
					}
				}
			case <-waitCtx.Done():
			}
		}(read)
	}

	launchFinalizer := func(
		name string,
		finalize func(context.Context) *future.Future[struct{}],
	) {
		go func() {
			select {
			case <-start:
			case <-waitCtx.Done():
				finalizerDone <- finalizerResult{
					name: name,
					err:  waitCtx.Err(),
				}
				return
			}
			_, err := finalize(waitCtx).Await(waitCtx)
			finalizerDone <- finalizerResult{name: name, err: err}
		}()
	}
	launchFinalizer("Commit()", commit)
	launchFinalizer("Rollback()", rollback)
	launchFinalizer("Close()", closeResource)
	close(start)

	for range 3 {
		select {
		case result := <-finalizerDone:
			finalizersRemaining--
			if result.err != nil {
				t.Errorf("%s error = %v, want nil", result.name, result.err)
			}
		case <-waitCtx.Done():
			t.Errorf("finalizers did not resolve: %v", waitCtx.Err())
			return
		}
	}
	// Racing reads are drained only to exercise paths under the race detector.
	for range racingReadCount {
		select {
		case <-readerDone:
			readersRemaining--
		case <-waitCtx.Done():
			t.Errorf("racing reads did not finish: %v", waitCtx.Err())
			return
		}
	}

	assertClosed := func(name string, read func() error) {
		err := read()
		switch {
		case err == nil:
			t.Errorf("%s error = nil, want %v", name, closedErr)
		case errors.Is(err, ErrFinalizationInProgress):
			t.Errorf(
				"%s error = ErrFinalizationInProgress, want %v",
				name,
				closedErr,
			)
		case !errors.Is(err, closedErr):
			t.Errorf("%s error = %v, want %v", name, err, closedErr)
		}
	}
	assertClosed("Lock()", readLock)
	assertClosed("AccountAdjustments()", readAdjustments)
}

// Property/fuzz verdict: generation is not warranted because the state space
// is scheduler interleavings of three finalizers and N readers over a
// three-state machine, not generated input. Race-detector repetitions sample
// the interleavings; deterministic tests cover every state transition.
func TestAsyncReservationConcurrentFinalizersWithRacingReads(t *testing.T) {
	order := buildChainCheckOrder(t)
	accountID := param.NewAccountIDFromUint64(chainOrderAccountID)
	for iteration := 0; iteration < 5; iteration++ {
		t.Run(fmt.Sprintf("iteration %d", iteration+1), func(t *testing.T) {
			engine := newChainEngine(t, newAcceptingDriver())
			reservation := newAsyncReservation(
				newChainReservation(t, order), engine, accountID,
			)
			assertConcurrentFinalizersWithRacingReads(
				t,
				pretrade.ErrReservationClosed,
				reservation.Lock,
				reservation.AccountAdjustments,
				reservation.Commit,
				reservation.Rollback,
				reservation.Close,
			)
		})
	}
}

func TestAsyncDropCopyConcurrentFinalizersWithRacingReads(t *testing.T) {
	order := buildChainCheckOrder(t)
	accountID := param.NewAccountIDFromUint64(chainOrderAccountID)
	for iteration := 0; iteration < 5; iteration++ {
		t.Run(fmt.Sprintf("iteration %d", iteration+1), func(t *testing.T) {
			engine := newChainEngine(t, newAcceptingDriver())
			operation := newAsyncDropCopyOperation(
				newChainDropCopyOperation(t, order), engine, accountID,
			)
			assertConcurrentFinalizersWithRacingReads(
				t,
				pretrade.ErrDropCopyOperationClosed,
				operation.Lock,
				operation.AccountAdjustments,
				operation.Commit,
				operation.Rollback,
				operation.Close,
			)
		})
	}
}

func assertClosedReadRefusedBeforeAdmission(
	t *testing.T,
	resource string,
	methodMu *sync.Mutex,
	mu *sync.Mutex,
	readers *int,
	read func() error,
	closedErr error,
) {
	t.Helper()
	methodMu.Lock()
	readDone := make(chan error, 1)
	go func() { readDone <- read() }()
	deadline := time.NewTimer(chainStopTimeout)
	defer deadline.Stop()
	checkError := func(err error) {
		if !errors.Is(err, closedErr) {
			t.Errorf(
				"%s read error = %v, want %v",
				resource,
				err,
				closedErr,
			)
		}
		if errors.Is(err, ErrFinalizationInProgress) {
			t.Errorf(
				"%s read error = %v, do not want "+
					"ErrFinalizationInProgress",
				resource,
				err,
			)
		}
	}

	for {
		mu.Lock()
		readerCount := *readers
		mu.Unlock()
		if readerCount != 0 {
			methodMu.Unlock()
			err := <-readDone
			t.Errorf("%s readers = %d, want 0", resource, readerCount)
			checkError(err)
			return
		}

		select {
		case err := <-readDone:
			methodMu.Unlock()
			checkError(err)
			mu.Lock()
			readerCount = *readers
			mu.Unlock()
			if readerCount != 0 {
				t.Errorf("%s readers = %d, want 0", resource, readerCount)
			}
			return
		case <-deadline.C:
			methodMu.Unlock()
			t.Fatalf(
				"%s read was not refused before admission",
				resource,
			)
		default:
			runtime.Gosched()
		}
	}
}

func TestAsyncReservationStaysClosedAfterLaterCommit(t *testing.T) {
	order := buildChainCheckOrder(t)
	engine := newChainEngine(t, newAcceptingDriver())
	waitCtx, cancel := context.WithTimeout(
		context.Background(), chainStopTimeout,
	)
	defer cancel()
	reservation, rejects, err := engine.ExecutePreTrade(
		context.Background(), order,
	).Await(waitCtx)
	if err != nil {
		t.Fatalf("ExecutePreTrade() error = %v", err)
	}
	if rejects != nil {
		t.Fatalf("ExecutePreTrade() rejects = %v, want nil", rejects)
	}
	if reservation == nil {
		t.Fatal("ExecutePreTrade() reservation = nil, want non-nil")
	}
	if _, err := reservation.Close(context.Background()).Await(
		waitCtx,
	); err != nil {
		t.Fatalf("Close() error = %v", err)
	}
	if _, err := reservation.Commit(context.Background()).Await(
		waitCtx,
	); err != nil {
		t.Fatalf("Commit() after Close error = %v", err)
	}
	assertClosedReadRefusedBeforeAdmission(
		t,
		"reservation",
		&reservation.methodMu,
		&reservation.mu,
		&reservation.readers,
		func() error {
			_, err := reservation.Lock()
			return err
		},
		pretrade.ErrReservationClosed,
	)
}

func TestAsyncDropCopyStaysClosedAfterLaterCommit(t *testing.T) {
	order := buildChainCheckOrder(t)
	engine := newChainEngine(t, newAcceptingDriver())
	waitCtx, cancel := context.WithTimeout(
		context.Background(), chainStopTimeout,
	)
	defer cancel()
	operation, rejects, err := engine.ApplyDropCopy(
		context.Background(), order,
	).Await(waitCtx)
	if err != nil {
		t.Fatalf("ApplyDropCopy() error = %v", err)
	}
	if rejects != nil {
		t.Fatalf("ApplyDropCopy() rejects = %v, want nil", rejects)
	}
	if operation == nil {
		t.Fatal("ApplyDropCopy() operation = nil, want non-nil")
	}
	if _, err := operation.Close(context.Background()).Await(
		waitCtx,
	); err != nil {
		t.Fatalf("Close() error = %v", err)
	}
	if _, err := operation.Commit(context.Background()).Await(
		waitCtx,
	); err != nil {
		t.Fatalf("Commit() after Close error = %v", err)
	}
	assertClosedReadRefusedBeforeAdmission(
		t,
		"drop-copy operation",
		&operation.methodMu,
		&operation.mu,
		&operation.readers,
		func() error {
			_, err := operation.Lock()
			return err
		},
		pretrade.ErrDropCopyOperationClosed,
	)
}

func assertSecondReaderWaits(
	t *testing.T,
	beginRead func() error,
	endRead func(),
	methodMu *sync.Mutex,
	readerCount func() int,
	secondRead func() error,
) {
	t.Helper()
	if err := beginRead(); err != nil {
		t.Fatalf("first reader was not admitted: %v", err)
	}
	methodMu.Lock()
	released := false
	defer func() {
		if !released {
			methodMu.Unlock()
			endRead()
		}
	}()

	secondDone := make(chan error, 1)
	go func() { secondDone <- secondRead() }()
	deadline := time.NewTimer(chainStopTimeout)
	defer deadline.Stop()
	for readerCount() != 2 {
		select {
		case err := <-secondDone:
			t.Fatalf("second reader returned before admission wait with error %v", err)
		case <-deadline.C:
			t.Fatal("second reader was not admitted")
		default:
			runtime.Gosched()
		}
	}
	select {
	case err := <-secondDone:
		t.Fatalf("second reader returned while first held method lock with error %v", err)
	default:
	}

	methodMu.Unlock()
	endRead()
	released = true
	select {
	case err := <-secondDone:
		if err != nil {
			t.Fatalf("second reader error = %v", err)
		}
	case <-deadline.C:
		t.Fatal("second reader did not resume")
	}
}

func TestAsyncReservationSerializesAdmittedReaders(t *testing.T) {
	order := buildChainCheckOrder(t)
	inner := newChainReservation(t, order)
	engine := newChainEngine(t, newAcceptingDriver())
	reservation := newAsyncReservation(
		inner, engine, param.NewAccountIDFromUint64(chainOrderAccountID),
	)
	assertSecondReaderWaits(
		t,
		reservation.beginRead,
		reservation.endRead,
		&reservation.methodMu,
		func() int {
			reservation.mu.Lock()
			defer reservation.mu.Unlock()
			return reservation.readers
		},
		func() error {
			_, err := reservation.Lock()
			return err
		},
	)
	waitCtx, cancel := context.WithTimeout(context.Background(), chainStopTimeout)
	defer cancel()
	if _, err := reservation.Close(context.Background()).Await(waitCtx); err != nil {
		t.Fatalf("Close() error = %v", err)
	}
}

func TestAsyncDropCopySerializesAdmittedReaders(t *testing.T) {
	order := buildChainCheckOrder(t)
	inner := newChainDropCopyOperation(t, order)
	engine := newChainEngine(t, newAcceptingDriver())
	operation := newAsyncDropCopyOperation(
		inner, engine, param.NewAccountIDFromUint64(chainOrderAccountID),
	)
	assertSecondReaderWaits(
		t,
		operation.beginRead,
		operation.endRead,
		&operation.methodMu,
		func() int {
			operation.mu.Lock()
			defer operation.mu.Unlock()
			return operation.readers
		},
		func() error {
			_, err := operation.Lock()
			return err
		},
	)
	waitCtx, cancel := context.WithTimeout(context.Background(), chainStopTimeout)
	defer cancel()
	if _, err := operation.Close(context.Background()).Await(waitCtx); err != nil {
		t.Fatalf("Close() error = %v", err)
	}
}

type chainDryRunReportDriver struct {
	*acceptingDriver
	report *pretrade.DryRunReport
	err    error
}

type chainPreTradeInvalidResultDriver struct {
	*acceptingDriver
	reservation *pretrade.Reservation
	rejects     []reject.Reject
	err         error
}

func (d *chainPreTradeInvalidResultDriver) ExecutePreTrade(
	model.Order,
) (*pretrade.Reservation, []reject.Reject, error) {
	return d.reservation, d.rejects, d.err
}

type chainDropCopyInvalidResultDriver struct {
	*acceptingDriver
	operation *pretrade.DropCopyOperation
	rejects   []reject.Reject
	err       error
}

func (d *chainDropCopyInvalidResultDriver) ApplyDropCopy(
	model.Order,
) (*pretrade.DropCopyOperation, []reject.Reject, error) {
	return d.operation, d.rejects, d.err
}

func (d *chainDryRunReportDriver) ExecutePreTradeDryRun(
	model.Order,
) (*pretrade.DryRunReport, error) {
	return d.report, d.err
}

type chainExecutionReportErrorDriver struct {
	*acceptingDriver
	err error
}

func (d *chainExecutionReportErrorDriver) ApplyExecutionReport(
	model.ExecutionReport,
) (pretrade.PostTradeResult, error) {
	return pretrade.PostTradeResult{}, d.err
}

type chainAccountAdjustmentResultDriver struct {
	*acceptingDriver
	err    error
	result accountadjustment.BatchResult
}

func (d *chainAccountAdjustmentResultDriver) ApplyAccountAdjustment(
	param.AccountID,
	[]model.AccountAdjustment,
) (accountadjustment.BatchResult, error) {
	return d.result, d.err
}

type chainPanicCall uint8

const (
	chainPanicExecutePreTrade chainPanicCall = iota + 1
	chainPanicExecutePreTradeDryRun
	chainPanicApplyDropCopy
	chainPanicApplyExecutionReport
	chainPanicApplyAccountAdjustment
)

type chainPanicDriver struct {
	*acceptingDriver
	call     chainPanicCall
	panicErr error
}

func (d *chainPanicDriver) panicAfter(call chainPanicCall) {
	if d.call == call {
		panic(d.panicErr)
	}
}

func (d *chainPanicDriver) ExecutePreTrade(
	order model.Order,
) (*pretrade.Reservation, []reject.Reject, error) {
	reservation, rejects, err := d.acceptingDriver.ExecutePreTrade(order)
	d.panicAfter(chainPanicExecutePreTrade)
	return reservation, rejects, err
}

func (d *chainPanicDriver) ExecutePreTradeDryRun(
	order model.Order,
) (*pretrade.DryRunReport, error) {
	report, err := d.acceptingDriver.ExecutePreTradeDryRun(order)
	d.panicAfter(chainPanicExecutePreTradeDryRun)
	return report, err
}

func (d *chainPanicDriver) ApplyDropCopy(
	order model.Order,
) (*pretrade.DropCopyOperation, []reject.Reject, error) {
	operation, rejects, err := d.acceptingDriver.ApplyDropCopy(order)
	d.panicAfter(chainPanicApplyDropCopy)
	return operation, rejects, err
}

func (d *chainPanicDriver) ApplyExecutionReport(
	report model.ExecutionReport,
) (pretrade.PostTradeResult, error) {
	result, err := d.acceptingDriver.ApplyExecutionReport(report)
	d.panicAfter(chainPanicApplyExecutionReport)
	return result, err
}

func (d *chainPanicDriver) ApplyAccountAdjustment(
	accountID param.AccountID,
	adjustments []model.AccountAdjustment,
) (accountadjustment.BatchResult, error) {
	result, err := d.acceptingDriver.ApplyAccountAdjustment(accountID, adjustments)
	d.panicAfter(chainPanicApplyAccountAdjustment)
	return result, err
}

func TestChainCheckOrderClosesDryRunReportOnEveryExit(t *testing.T) {
	driverErr := errors.New("dry-run transport failed")
	panicErr := errors.New("checked hook panicked")
	for _, test := range []struct {
		name        string
		withReport  bool
		driverErr   error
		panicInHook bool
		wantChecked bool
		wantErr     error
	}{
		{
			name:      "driver error",
			driverErr: driverErr,
			wantErr:   driverErr,
		},
		{
			name:        "success",
			withReport:  true,
			wantChecked: true,
		},
		{
			name:        "checked hook panic",
			withReport:  true,
			panicInHook: true,
			wantChecked: true,
			wantErr:     panicErr,
		},
	} {
		t.Run(test.name, func(t *testing.T) {
			order := buildChainCheckOrder(t)
			var report *pretrade.DryRunReport
			if test.withReport {
				report = newChainDryRunReport(t, order)
				t.Cleanup(report.Close)
			}
			driver := &chainDryRunReportDriver{
				acceptingDriver: newAcceptingDriver(),
				report:          report,
				err:             test.driverErr,
			}
			engine := newChainEngine(t, driver)
			checked := false
			chain := Chain(
				order,
				func(context.Context) (struct{}, error) { return struct{}{}, nil },
			).CheckOrder(func(
				_ context.Context,
				_ struct{},
				_ OrderCheckResult,
			) error {
				checked = true
				if test.panicInHook {
					panic(panicErr)
				}
				return nil
			})
			_, err := chain.Run(context.Background(), engine).Await(context.Background())
			if test.wantErr == nil && err != nil {
				t.Fatalf("chain Await() error = %v, want nil", err)
			}
			if test.wantErr != nil && !errors.Is(err, test.wantErr) {
				t.Fatalf("chain Await() error = %v, want %v", err, test.wantErr)
			}
			if checked != test.wantChecked {
				t.Errorf("OnChecked() called = %v, want %v", checked, test.wantChecked)
			}
			if report != nil {
				if _, err := report.IsPass(); !errors.Is(
					err, pretrade.ErrDryRunReportClosed,
				) {
					t.Errorf(
						"dry-run report IsPass() error = %v, want ErrDryRunReportClosed",
						err,
					)
				}
			}
		})
	}
}

func TestChainCheckOrderRejectsInvalidDriverResults(t *testing.T) {
	driverErr := errors.New("dry-run driver error")
	for _, test := range []struct {
		name       string
		withReport bool
		close      bool
		err        error
	}{
		{name: "nil report"},
		{name: "closed report", withReport: true, close: true},
		{name: "report with error", withReport: true, err: driverErr},
		{
			name: "closed report with error", withReport: true, close: true,
			err: driverErr,
		},
	} {
		t.Run(test.name, func(t *testing.T) {
			order := buildChainCheckOrder(t)
			var report *pretrade.DryRunReport
			if test.withReport {
				report = newChainDryRunReport(t, order)
				t.Cleanup(report.Close)
				if test.close {
					report.Close()
				}
			}
			engine := newChainEngine(t, &chainDryRunReportDriver{
				acceptingDriver: newAcceptingDriver(),
				report:          report,
				err:             test.err,
			})
			checked := false
			chain := Chain(
				order,
				func(context.Context) (struct{}, error) { return struct{}{}, nil },
			).CheckOrder(func(
				context.Context,
				struct{},
				OrderCheckResult,
			) error {
				checked = true
				return nil
			})
			_, err := chain.Run(context.Background(), engine).Await(
				context.Background(),
			)
			if !errors.Is(err, ErrDriverResult) {
				t.Errorf("chain Await() error = %v, want ErrDriverResult", err)
			}
			if test.err != nil && !errors.Is(err, test.err) {
				t.Errorf("chain Await() error = %v, want driver error", err)
			}
			if checked {
				t.Error("OnChecked() called for invalid driver result")
			}
			if report != nil && !report.IsClosed() {
				t.Error("invalid dry-run report remains open")
			}
		})
	}
}

func TestOrderCheckResultRejectsReturnsDefensiveCopy(t *testing.T) {
	order := model.NewOrder()
	operation := order.EnsureOperationView()
	operation.SetAccountID(param.NewAccountIDFromUint64(chainOrderAccountID))
	report := newChainDryRunReport(t, order)
	t.Cleanup(report.Close)
	engine := newChainEngine(t, &chainDryRunReportDriver{
		acceptingDriver: newAcceptingDriver(),
		report:          report,
	})
	var escaped OrderCheckResult
	chain := Chain(
		order,
		func(context.Context) (struct{}, error) { return struct{}{}, nil },
	).CheckOrder(func(
		_ context.Context,
		_ struct{},
		result OrderCheckResult,
	) error {
		escaped = result
		first := result.Rejects()
		if len(first) == 0 {
			return errors.New("rejected dry run returned no rejects")
		}
		want := append([]reject.Reject(nil), first...)
		first[0] = reject.Reject{}
		if got := result.Rejects(); !reflect.DeepEqual(got, want) {
			return fmt.Errorf("Rejects() after caller mutation = %v, want %v", got, want)
		}
		return nil
	})
	if _, err := chain.Run(context.Background(), engine).Await(
		context.Background(),
	); err != nil {
		t.Fatalf("chain Await() error = %v", err)
	}
	first := escaped.Rejects()
	want := append([]reject.Reject(nil), first...)
	first[0] = reject.Reject{}
	if got := escaped.Rejects(); !reflect.DeepEqual(got, want) {
		t.Errorf("escaped Rejects() after caller mutation = %v, want %v", got, want)
	}
}

func TestChainRejectsInvalidDriverResults(t *testing.T) {
	driverErr := errors.New("driver returned an accepted handle and error")
	for _, test := range []struct {
		name         string
		dropCopy     bool
		accepted     bool
		withRejects  bool
		emptyRejects bool
		withError    bool
	}{
		{
			name:     "pre-trade accepted and rejected",
			accepted: true, withRejects: true,
		},
		{
			name:     "pre-trade accepted and errored",
			accepted: true, withError: true,
		},
		{
			name:        "pre-trade rejected and errored",
			withRejects: true, withError: true,
		},
		{
			name: "pre-trade neither accepted nor rejected",
		},
		{
			name:         "pre-trade empty rejects",
			emptyRejects: true,
		},
		{
			name:     "drop-copy accepted and rejected",
			dropCopy: true,
			accepted: true, withRejects: true,
		},
		{
			name:     "drop-copy accepted and errored",
			dropCopy: true,
			accepted: true, withError: true,
		},
		{
			name:        "drop-copy rejected and errored",
			dropCopy:    true,
			withRejects: true, withError: true,
		},
		{
			name:     "drop-copy neither accepted nor rejected",
			dropCopy: true,
		},
		{
			name:         "drop-copy empty rejects",
			dropCopy:     true,
			emptyRejects: true,
		},
	} {
		t.Run(test.name, func(t *testing.T) {
			order := buildChainCheckOrder(t)
			cleanupEntered := make(chan struct{})
			cleanupRelease := make(chan struct{})
			releaseCleanup := sync.OnceFunc(func() { close(cleanupRelease) })
			defer releaseCleanup()
			var rollbackCalls atomic.Int64
			probe := &chainMutationProbe{}
			probe.onRollback = func() {
				if rollbackCalls.Add(1) == 1 {
					close(cleanupEntered)
				}
				<-cleanupRelease
			}
			var resultErr error
			if test.withError {
				resultErr = driverErr
			}
			var rejects []reject.Reject
			if test.withRejects {
				rejects = []reject.Reject{{}}
			}
			if test.emptyRejects {
				rejects = []reject.Reject{}
			}
			var driver Driver
			var assertClosed func()
			if test.dropCopy {
				var operation *pretrade.DropCopyOperation
				if test.accepted {
					operation = newChainEngineDropCopyOperation(
						t, newChainProbeEngine(t, probe), order,
					)
					assertClosed = func() {
						assertChainClosedDropCopyOperation(t, operation)
					}
				}
				driver = &chainDropCopyInvalidResultDriver{
					acceptingDriver: newAcceptingDriver(),
					operation:       operation,
					rejects:         rejects,
					err:             resultErr,
				}
			} else {
				var reservation *pretrade.Reservation
				if test.accepted {
					reservation = newChainEngineReservation(
						t, newChainProbeEngine(t, probe), order,
					)
					assertClosed = func() {
						assertChainClosedReservation(t, reservation)
					}
				}
				driver = &chainPreTradeInvalidResultDriver{
					acceptingDriver: newAcceptingDriver(),
					reservation:     reservation,
					rejects:         rejects,
					err:             resultErr,
				}
			}
			engine := newChainEngine(t, driver)
			laterCalls := 0
			chain := Chain(
				order,
				func(context.Context) (struct{}, error) { return struct{}{}, nil },
			)
			if test.dropCopy {
				chain.ApplyDropCopy(DropCopyHooks[struct{}]{
					OnRejected: func(context.Context, struct{}, []reject.Reject) error {
						return errors.New("OnRejected() called for invalid driver result")
					},
					OnApplied: func(
						context.Context,
						struct{},
						DropCopyResult,
					) (Decision, error) {
						return decisionInvalid, errors.New("OnApplied() called for invalid driver result")
					},
				})
			} else {
				chain.ExecutePreTrade(PreTradeHooks[struct{}]{
					OnRejected: func(context.Context, struct{}, []reject.Reject) error {
						return errors.New("OnRejected() called for invalid driver result")
					},
					OnReserved: func(
						context.Context,
						struct{},
						OperationResult,
					) (Decision, error) {
						return decisionInvalid, errors.New("OnReserved() called for invalid driver result")
					},
				})
			}
			chain.Then(func(context.Context, struct{}) error {
				laterCalls++
				return nil
			})
			chainFuture := chain.Run(context.Background(), engine)
			if test.accepted {
				select {
				case <-cleanupEntered:
				case <-time.After(chainStopTimeout):
					t.Fatal("malformed accepted result cleanup did not start")
				}
				if chainFuture.Done() {
					t.Fatal("chain future resolved before native handle cleanup")
				}
				releaseCleanup()
			}
			waitCtx, cancel := context.WithTimeout(
				context.Background(), chainStopTimeout,
			)
			defer cancel()
			outcome, err := chainFuture.Await(waitCtx)
			assertChainRetryUnsafe(t, err, outcome, ErrDriverResult)
			if test.withError {
				if !errors.Is(err, driverErr) {
					t.Errorf("chain Await() error = %v, want driver error", err)
				}
				if !errors.Is(outcome.Err, driverErr) {
					t.Errorf("outcome error = %v, want driver error", outcome.Err)
				}
			}
			if laterCalls != 0 {
				t.Errorf("later step calls = %d, want 0", laterCalls)
			}
			if test.accepted {
				if got := rollbackCalls.Load(); got != 1 {
					t.Errorf("native rollback calls = %d, want 1", got)
				}
				assertClosed()
			}
		})
	}
}

func TestChainRejectsInvalidLifetimeObjects(t *testing.T) {
	driverErr := errors.New("driver returned an unusable handle and error")
	for _, mutation := range []string{"pre-trade", "drop copy"} {
		for _, state := range []string{
			"nil native handle", "closed native handle", "live native handle",
		} {
			for _, conflict := range []struct {
				name    string
				rejects []reject.Reject
				err     error
			}{
				{name: "without another channel"},
				{name: "with rejects", rejects: []reject.Reject{{}}},
				{name: "with error", err: driverErr},
			} {
				if state == "live native handle" &&
					conflict.name == "without another channel" {
					continue
				}
				t.Run(mutation+" with "+state+" "+conflict.name, func(t *testing.T) {
					order := buildChainCheckOrder(t)
					var driver Driver
					var isClosed func() bool
					if mutation == "drop copy" {
						operation := pretrade.NewDropCopyOperationFromHandle(nil)
						switch state {
						case "closed native handle":
							operation = newChainDropCopyOperation(t, order)
							operation.Close()
						case "live native handle":
							operation = newChainDropCopyOperation(t, order)
						}
						driver = &chainDropCopyInvalidResultDriver{
							acceptingDriver: newAcceptingDriver(),
							operation:       operation,
							rejects:         conflict.rejects,
							err:             conflict.err,
						}
						isClosed = operation.IsClosed
					} else {
						reservation := pretrade.NewReservationFromHandle(nil)
						switch state {
						case "closed native handle":
							reservation = newChainReservation(t, order)
							reservation.Close()
						case "live native handle":
							reservation = newChainReservation(t, order)
						}
						driver = &chainPreTradeInvalidResultDriver{
							acceptingDriver: newAcceptingDriver(),
							reservation:     reservation,
							rejects:         conflict.rejects,
							err:             conflict.err,
						}
						isClosed = reservation.IsClosed
					}

					engine := newChainEngine(t, driver)
					hookCalls := 0
					chain := Chain(
						order,
						func(context.Context) (struct{}, error) { return struct{}{}, nil },
					)
					if mutation == "drop copy" {
						chain.ApplyDropCopy(DropCopyHooks[struct{}]{
							OnRejected: func(context.Context, struct{}, []reject.Reject) error {
								hookCalls++
								return nil
							},
							OnApplied: func(
								context.Context,
								struct{},
								DropCopyResult,
							) (Decision, error) {
								hookCalls++
								return DecisionCommit, nil
							},
						})
					} else {
						chain.ExecutePreTrade(PreTradeHooks[struct{}]{
							OnRejected: func(context.Context, struct{}, []reject.Reject) error {
								hookCalls++
								return nil
							},
							OnReserved: func(
								context.Context,
								struct{},
								OperationResult,
							) (Decision, error) {
								hookCalls++
								return DecisionCommit, nil
							},
						})
					}
					outcome, err := chain.Run(context.Background(), engine).Await(
						context.Background(),
					)
					assertChainRetryUnsafe(t, err, outcome, ErrDriverResult)
					if conflict.err != nil {
						if !errors.Is(err, conflict.err) {
							t.Errorf("chain Await() error = %v, want driver error", err)
						}
						if !errors.Is(outcome.Err, conflict.err) {
							t.Errorf("outcome error = %v, want driver error", outcome.Err)
						}
					}
					if hookCalls != 0 {
						t.Errorf("mutation hook calls = %d, want 0", hookCalls)
					}
					if !isClosed() {
						t.Error("malformed lifetime object remains open")
					}
				})
			}
		}
	}
}

func TestChainMutationDriverFailuresMarkRetryUnsafe(t *testing.T) {
	driver := newTransportErrorDriver()
	engine := newChainEngine(t, driver)
	for _, test := range []struct {
		name     string
		dropCopy bool
	}{
		{name: "execute pre-trade"},
		{name: "apply drop copy", dropCopy: true},
	} {
		t.Run(test.name, func(t *testing.T) {
			chain := Chain(
				buildTestOrder(t, 42),
				func(context.Context) (struct{}, error) { return struct{}{}, nil },
			)
			if test.dropCopy {
				chain.ApplyDropCopy(DropCopyHooks[struct{}]{
					OnRejected: func(context.Context, struct{}, []reject.Reject) error {
						return errors.New("OnRejected() called after driver failure")
					},
					OnApplied: func(context.Context, struct{}, DropCopyResult) (Decision, error) {
						return DecisionRollback, errors.New("OnApplied() called after driver failure")
					},
				})
			} else {
				chain.ExecutePreTrade(PreTradeHooks[struct{}]{
					OnRejected: func(context.Context, struct{}, []reject.Reject) error {
						return errors.New("OnRejected() called after driver failure")
					},
					OnReserved: func(context.Context, struct{}, OperationResult) (Decision, error) {
						return DecisionRollback, errors.New("OnReserved() called after driver failure")
					},
				})
			}

			outcome, err := chain.Run(context.Background(), engine).Await(context.Background())
			assertChainRetryUnsafe(t, err, outcome, driver.executeErr)
		})
	}
}

func TestChainInvalidMutationDecisionsMarkRetryUnsafe(t *testing.T) {
	engine := newChainEngine(t, newAcceptingDriver())
	for _, test := range []struct {
		name     string
		dropCopy bool
	}{
		{name: "execute pre-trade"},
		{name: "apply drop copy", dropCopy: true},
	} {
		t.Run(test.name, func(t *testing.T) {
			chain := Chain(
				buildTestOrder(t, 42),
				func(context.Context) (struct{}, error) { return struct{}{}, nil },
			)
			if test.dropCopy {
				chain.ApplyDropCopy(DropCopyHooks[struct{}]{
					OnRejected: func(context.Context, struct{}, []reject.Reject) error {
						return errors.New("OnRejected() called for accepted drop copy")
					},
					OnApplied: func(context.Context, struct{}, DropCopyResult) (Decision, error) {
						return Decision(255), nil
					},
				})
			} else {
				chain.ExecutePreTrade(PreTradeHooks[struct{}]{
					OnRejected: func(context.Context, struct{}, []reject.Reject) error {
						return errors.New("OnRejected() called for accepted reservation")
					},
					OnReserved: func(context.Context, struct{}, OperationResult) (Decision, error) {
						return Decision(255), nil
					},
				})
			}

			outcome, err := chain.Run(context.Background(), engine).Await(context.Background())
			assertChainRetryUnsafe(t, err, outcome, ErrChainInvalidDecision)
		})
	}
}

func TestChainRejectsMissingDryRunReport(t *testing.T) {
	driver := &chainDryRunReportDriver{acceptingDriver: newAcceptingDriver()}
	engine := newChainEngine(t, driver)
	chain := Chain(
		buildChainCheckOrder(t),
		func(context.Context) (struct{}, error) { return struct{}{}, nil },
	).CheckOrder(func(context.Context, struct{}, OrderCheckResult) error {
		return errors.New("OnChecked() called without a report")
	})
	if _, err := chain.Run(context.Background(), engine).Await(
		context.Background(),
	); !errors.Is(err, ErrDriverResult) {
		t.Errorf("chain Await() error = %v, want ErrDriverResult", err)
	}
}

// The escaped result serializes its reads against the chain's own
// finalization. A reader that keeps reading across it may only see a
// successful read or ErrChainResultUnavailable; ErrReservationClosed or
// ErrDropCopyOperationClosed would mean it reached a native handle the chain
// had already released. Once the chain resolves, every further read reports
// ErrChainResultUnavailable.
//
// This is the mutex contract, and what -race exercises. The ordering invariant
// behind it is proven deterministically by
// TestChainInvalidatesReservationResultBeforeFinalizer and its drop-copy twin.
func TestChainInvalidatesEscapedOperationResultConcurrently(t *testing.T) {
	for _, test := range []struct {
		name     string
		dropCopy bool
	}{
		{name: "execute pre-trade"},
		{name: "apply drop copy", dropCopy: true},
	} {
		t.Run(test.name, func(t *testing.T) {
			order := buildChainCheckOrder(t)
			readerStarted := make(chan struct{})
			futureResolved := make(chan struct{})
			stopReader := sync.OnceFunc(func() { close(futureResolved) })
			var readers sync.WaitGroup
			// The reader must stop on every exit path, a failed assertion
			// included: it must never read a result whose handle the cleanups
			// are about to release.
			defer func() {
				stopReader()
				readers.Wait()
			}()
			inHookReads := make(chan error, 1)
			racingReads := make(chan error, 1)
			finalReads := make(chan error, 1)
			startReader := func(result OperationResult) {
				readers.Add(1)
				go func() {
					defer readers.Done()
					var inHookErr error
					for range 100 {
						if _, err := result.Lock(); err != nil && inHookErr == nil {
							inHookErr = err
						}
					}
					inHookReads <- inHookErr
					close(readerStarted)
					var closedErr error
					readAcrossFinalization := func() {
						_, err := result.Lock()
						if err == nil || errors.Is(err, ErrChainResultUnavailable) {
							return
						}
						if closedErr == nil {
							closedErr = err
						}
					}
					for {
						select {
						case <-futureResolved:
							readAcrossFinalization()
							racingReads <- closedErr
							_, err := result.Lock()
							finalReads <- err
							return
						default:
							readAcrossFinalization()
							runtime.Gosched()
						}
					}
				}()
				<-readerStarted
				runtime.Gosched()
			}
			var engine *AsyncEngine
			chain := Chain(
				order,
				func(context.Context) (struct{}, error) { return struct{}{}, nil },
			)
			if test.dropCopy {
				engine = newChainEngine(t, &chainDropCopyDriver{
					acceptingDriver: newAcceptingDriver(),
					operation:       newChainDropCopyOperation(t, order),
				})
				chain.ApplyDropCopy(DropCopyHooks[struct{}]{
					OnRejected: func(context.Context, struct{}, []reject.Reject) error {
						return errors.New("OnRejected() called for accepted drop copy")
					},
					OnApplied: func(
						_ context.Context,
						_ struct{},
						result DropCopyResult,
					) (Decision, error) {
						startReader(result)
						return DecisionCommit, nil
					},
				})
			} else {
				engine = newChainEngine(t, &chainReservationDriver{
					acceptingDriver: newAcceptingDriver(),
					reservation:     newChainReservation(t, order),
				})
				chain.ExecutePreTrade(PreTradeHooks[struct{}]{
					OnRejected: func(context.Context, struct{}, []reject.Reject) error {
						return errors.New("OnRejected() called for accepted reservation")
					},
					OnReserved: func(
						_ context.Context,
						_ struct{},
						result OperationResult,
					) (Decision, error) {
						startReader(result)
						return DecisionCommit, nil
					},
				})
			}
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
			case err := <-racingReads:
				if err != nil {
					t.Errorf(
						"escaped result Lock() across finalization error = %v, "+
							"want nil or ErrChainResultUnavailable",
						err,
					)
				}
			case <-time.After(5 * time.Second):
				t.Fatal("escaped result reader did not report its racing reads")
			}
			select {
			case err := <-finalReads:
				if !errors.Is(err, ErrChainResultUnavailable) {
					t.Errorf(
						"escaped result final Lock() error = %v, "+
							"want ErrChainResultUnavailable",
						err,
					)
				}
			case <-time.After(5 * time.Second):
				t.Fatal("escaped result reader did not finish")
			}
		})
	}
}

func assertChainRetryUnsafe(
	t *testing.T,
	err error,
	outcome ChainOutcome,
	wantErr error,
) {
	t.Helper()
	if !errors.Is(err, wantErr) {
		t.Errorf("chain Await() error = %v, want %v", err, wantErr)
	}
	if !errors.Is(err, ErrChainRetryUnsafe) {
		t.Errorf("chain Await() error = %v, want ErrChainRetryUnsafe", err)
	}
	if outcome.Status != ChainOutcomeFailed {
		t.Errorf("outcome status = %v, want ChainOutcomeFailed", outcome.Status)
	}
	if !outcome.RetryUnsafe {
		t.Error("outcome RetryUnsafe = false, want true")
	}
	if !errors.Is(outcome.Err, wantErr) {
		t.Errorf("outcome error = %v, want %v", outcome.Err, wantErr)
	}
	if !errors.Is(outcome.Err, ErrChainRetryUnsafe) {
		t.Errorf("outcome error = %v, want ErrChainRetryUnsafe", outcome.Err)
	}
}

func assertChainExecutionPanicLabel(t *testing.T, err error) {
	t.Helper()
	if !strings.Contains(err.Error(), "async chain execution panicked") {
		t.Errorf("chain error = %v, want neutral execution panic label", err)
	}
	if strings.Contains(err.Error(), "async chain hook panicked") {
		t.Errorf("chain error = %v, unexpectedly attributes panic to a hook", err)
	}
}

func assertChainRetrySafe(
	t *testing.T,
	err error,
	outcome ChainOutcome,
	wantErr error,
) {
	t.Helper()
	if !errors.Is(err, wantErr) {
		t.Errorf("chain Await() error = %v, want %v", err, wantErr)
	}
	if errors.Is(err, ErrChainRetryUnsafe) {
		t.Errorf("chain Await() error = %v, unexpectedly marks retry unsafe", err)
	}
	if outcome.Status != ChainOutcomeFailed {
		t.Errorf("outcome status = %v, want ChainOutcomeFailed", outcome.Status)
	}
	if outcome.RetryUnsafe {
		t.Error("outcome RetryUnsafe = true, want false")
	}
	if !errors.Is(outcome.Err, wantErr) {
		t.Errorf("outcome error = %v, want %v", outcome.Err, wantErr)
	}
	if errors.Is(outcome.Err, ErrChainRetryUnsafe) {
		t.Errorf("outcome error = %v, unexpectedly marks retry unsafe", outcome.Err)
	}
}

// assertChainRejectedRetryUnsafe is the no-error sibling of
// assertChainRetryUnsafe. A chain that ends in a reject or a rollback produces
// no error at all, so the sentinel has nothing to travel in and RetryUnsafe is
// the only carrier left: the engine call was still entered, and rerunning the
// chain is still unsafe.
func assertChainRejectedRetryUnsafe(
	t *testing.T,
	err error,
	outcome ChainOutcome,
) {
	t.Helper()
	if err != nil {
		t.Errorf("chain Await() error = %v, want nil", err)
	}
	if outcome.Status != ChainOutcomeRejected {
		t.Errorf("outcome status = %v, want ChainOutcomeRejected", outcome.Status)
	}
	if outcome.Err != nil {
		t.Errorf("outcome error = %v, want nil", outcome.Err)
	}
	if !outcome.RetryUnsafe {
		t.Error("outcome RetryUnsafe = false, want true")
	}
}

func TestChainPanicDriverMutationMarksRetryUnsafe(t *testing.T) {
	panicErr := errors.New("driver panicked")
	for _, test := range []struct {
		name  string
		call  chainPanicCall
		build func(*ChainOutcome) *ChainRunner[struct{}]
	}{
		{
			name: "execute pre-trade", call: chainPanicExecutePreTrade,
			build: func(outcome *ChainOutcome) *ChainRunner[struct{}] {
				return Chain(
					buildTestOrder(t, 42),
					func(context.Context) (struct{}, error) { return struct{}{}, nil },
				).ExecutePreTrade(PreTradeHooks[struct{}]{
					OnRejected: func(context.Context, struct{}, []reject.Reject) error {
						return errors.New("OnRejected() called for accepted reservation")
					},
					OnReserved: func(
						context.Context,
						struct{},
						OperationResult,
					) (Decision, error) {
						return DecisionCommit, nil
					},
				}).Finally(func(_ context.Context, _ struct{}, got ChainOutcome) error {
					*outcome = got
					return nil
				})
			},
		},
		{
			name: "apply drop copy", call: chainPanicApplyDropCopy,
			build: func(outcome *ChainOutcome) *ChainRunner[struct{}] {
				return Chain(
					buildTestOrder(t, 42),
					func(context.Context) (struct{}, error) { return struct{}{}, nil },
				).ApplyDropCopy(DropCopyHooks[struct{}]{
					OnRejected: func(context.Context, struct{}, []reject.Reject) error {
						return errors.New("OnRejected() called for accepted drop copy")
					},
					OnApplied: func(
						context.Context,
						struct{},
						DropCopyResult,
					) (Decision, error) {
						return DecisionCommit, nil
					},
				}).Finally(func(_ context.Context, _ struct{}, got ChainOutcome) error {
					*outcome = got
					return nil
				})
			},
		},
		{
			name: "apply execution report", call: chainPanicApplyExecutionReport,
			build: func(outcome *ChainOutcome) *ChainRunner[struct{}] {
				accountID := param.NewAccountIDFromUint64(42)
				report := buildTestReport(t, 42)
				return Chain(
					accountID,
					func(context.Context) (struct{}, error) { return struct{}{}, nil },
				).ApplyExecutionReport(ExecutionReportHooks[struct{}]{
					Report: func(context.Context, struct{}) (model.ExecutionReport, error) {
						return report, nil
					},
					OnSettled: func(
						context.Context,
						struct{},
						pretrade.PostTradeResult,
					) error {
						return errors.New("OnSettled() called after driver panic")
					},
				}).Finally(func(_ context.Context, _ struct{}, got ChainOutcome) error {
					*outcome = got
					return nil
				})
			},
		},
		{
			name: "apply non-empty account adjustment",
			call: chainPanicApplyAccountAdjustment,
			build: func(outcome *ChainOutcome) *ChainRunner[struct{}] {
				return Chain(
					param.NewAccountIDFromUint64(42),
					func(context.Context) (struct{}, error) { return struct{}{}, nil },
				).ApplyAccountAdjustment(AccountAdjustmentHooks[struct{}]{
					Adjustments: func(
						context.Context,
						struct{},
					) ([]model.AccountAdjustment, error) {
						return []model.AccountAdjustment{model.NewAccountAdjustment()}, nil
					},
					OnAdjusted: func(
						context.Context,
						struct{},
						accountadjustment.BatchResult,
					) error {
						return errors.New("OnAdjusted() called after driver panic")
					},
				}).Finally(func(_ context.Context, _ struct{}, got ChainOutcome) error {
					*outcome = got
					return nil
				})
			},
		},
	} {
		t.Run(test.name, func(t *testing.T) {
			driver := &chainPanicDriver{
				acceptingDriver: newAcceptingDriver(),
				call:            test.call,
				panicErr:        panicErr,
			}
			engine := newChainEngine(t, driver)
			var outcome ChainOutcome
			_, err := test.build(&outcome).Run(context.Background(), engine).Await(
				context.Background(),
			)
			assertChainRetryUnsafe(t, err, outcome, panicErr)
			assertChainExecutionPanicLabel(t, err)
		})
	}
}

func TestChainPanicInEmptyAdjustmentDriverCallMarksRetryUnsafe(t *testing.T) {
	panicErr := errors.New("driver panicked")
	driver := &chainPanicDriver{
		acceptingDriver: newAcceptingDriver(),
		call:            chainPanicApplyAccountAdjustment,
		panicErr:        panicErr,
	}
	engine := newChainEngine(t, driver)
	var outcome ChainOutcome
	runner := Chain(
		param.NewAccountIDFromUint64(42),
		func(context.Context) (struct{}, error) { return struct{}{}, nil },
	).ApplyAccountAdjustment(AccountAdjustmentHooks[struct{}]{
		Adjustments: func(context.Context, struct{}) ([]model.AccountAdjustment, error) {
			return nil, nil
		},
		OnAdjusted: func(
			context.Context,
			struct{},
			accountadjustment.BatchResult,
		) error {
			return errors.New("OnAdjusted() called after driver panic")
		},
	}).Finally(func(_ context.Context, _ struct{}, got ChainOutcome) error {
		outcome = got
		return nil
	})
	_, err := runner.Run(context.Background(), engine).Await(context.Background())
	assertChainRetryUnsafe(t, err, outcome, panicErr)
	assertChainExecutionPanicLabel(t, err)
}

func TestChainPanicInCheckOrderDriverCallDoesNotMarkRetryUnsafe(t *testing.T) {
	panicErr := errors.New("driver panicked")
	driver := &chainPanicDriver{
		acceptingDriver: newAcceptingDriver(),
		call:            chainPanicExecutePreTradeDryRun,
		panicErr:        panicErr,
	}
	engine := newChainEngine(t, driver)
	var outcome ChainOutcome
	runner := Chain(
		buildTestOrder(t, 42),
		func(context.Context) (struct{}, error) { return struct{}{}, nil },
	).CheckOrder(func(context.Context, struct{}, OrderCheckResult) error {
		return errors.New("CheckOrder() hook called after driver panic")
	}).Finally(func(_ context.Context, _ struct{}, got ChainOutcome) error {
		outcome = got
		return nil
	})
	_, err := runner.Run(context.Background(), engine).Await(context.Background())
	assertChainRetrySafe(t, err, outcome, panicErr)
	assertChainExecutionPanicLabel(t, err)
}

func TestChainMarksExecutionReportHookFailureRetryUnsafe(t *testing.T) {
	hookErr := errors.New("state update failed")
	for _, test := range []struct {
		name  string
		panic bool
	}{
		{name: "error"},
		{name: "panic", panic: true},
	} {
		t.Run(test.name, func(t *testing.T) {
			engine := newChainEngine(t, newAcceptingDriver())
			accountID := param.NewAccountIDFromUint64(42)
			report := buildTestReport(t, 42)
			var outcome ChainOutcome
			runner := Chain(
				accountID,
				func(context.Context) (struct{}, error) { return struct{}{}, nil },
			).ApplyExecutionReport(ExecutionReportHooks[struct{}]{
				Report: func(context.Context, struct{}) (model.ExecutionReport, error) {
					return report, nil
				},
				OnSettled: func(
					context.Context,
					struct{},
					pretrade.PostTradeResult,
				) error {
					if test.panic {
						panic(hookErr)
					}
					return hookErr
				},
			}).Finally(func(
				_ context.Context,
				_ struct{},
				got ChainOutcome,
			) error {
				outcome = got
				return nil
			})
			_, err := runner.Run(context.Background(), engine).Await(
				context.Background(),
			)
			assertChainRetryUnsafe(t, err, outcome, hookErr)
		})
	}
}

func TestChainExecutionReportDriverFailureMarksRetryUnsafe(t *testing.T) {
	driverErr := errors.New("engine rejected report")
	driver := &chainExecutionReportErrorDriver{
		acceptingDriver: newAcceptingDriver(),
		err:             driverErr,
	}
	engine := newChainEngine(t, driver)
	accountID := param.NewAccountIDFromUint64(42)
	report := buildTestReport(t, 42)
	settled := false
	var outcome ChainOutcome
	runner := Chain(
		accountID,
		func(context.Context) (struct{}, error) { return struct{}{}, nil },
	).ApplyExecutionReport(ExecutionReportHooks[struct{}]{
		Report: func(context.Context, struct{}) (model.ExecutionReport, error) {
			return report, nil
		},
		OnSettled: func(context.Context, struct{}, pretrade.PostTradeResult) error {
			settled = true
			return nil
		},
	}).Finally(func(_ context.Context, _ struct{}, got ChainOutcome) error {
		outcome = got
		return nil
	})
	_, err := runner.Run(context.Background(), engine).Await(context.Background())
	if !errors.Is(err, driverErr) {
		t.Fatalf("chain Await() error = %v, want %v", err, driverErr)
	}
	assertChainRetryUnsafe(t, err, outcome, driverErr)
	if settled {
		t.Error("OnSettled() called after driver failure")
	}
}

func TestChainMarksAccountAdjustmentHookFailureRetryUnsafe(t *testing.T) {
	hookErr := errors.New("account persistence failed")
	engine := newChainEngine(t, newAcceptingDriver())
	accountID := param.NewAccountIDFromUint64(42)
	var outcome ChainOutcome
	runner := Chain(
		accountID,
		func(context.Context) (struct{}, error) { return struct{}{}, nil },
	).ApplyAccountAdjustment(AccountAdjustmentHooks[struct{}]{
		Adjustments: func(context.Context, struct{}) ([]model.AccountAdjustment, error) {
			return []model.AccountAdjustment{model.NewAccountAdjustment()}, nil
		},
		OnAdjusted: func(
			context.Context,
			struct{},
			accountadjustment.BatchResult,
		) error {
			return hookErr
		},
	}).Finally(func(_ context.Context, _ struct{}, got ChainOutcome) error {
		outcome = got
		return nil
	})
	_, err := runner.Run(context.Background(), engine).Await(context.Background())
	assertChainRetryUnsafe(t, err, outcome, hookErr)
}

func TestChainEmptyAdjustmentBatchMarksRetryUnsafe(t *testing.T) {
	laterErr := errors.New("later step failed")
	driver := &chainAccountAdjustmentResultDriver{
		acceptingDriver: newAcceptingDriver(),
	}
	engine := newChainEngine(t, driver)
	var outcome ChainOutcome
	runner := Chain(
		param.NewAccountIDFromUint64(42),
		func(context.Context) (struct{}, error) { return struct{}{}, nil },
	).ApplyAccountAdjustment(AccountAdjustmentHooks[struct{}]{
		Adjustments: func(context.Context, struct{}) ([]model.AccountAdjustment, error) {
			return nil, nil
		},
		OnAdjusted: func(
			context.Context,
			struct{},
			accountadjustment.BatchResult,
		) error {
			return nil
		},
	}).Then(func(context.Context, struct{}) error {
		return laterErr
	}).Finally(func(_ context.Context, _ struct{}, got ChainOutcome) error {
		outcome = got
		return nil
	})

	_, err := runner.Run(context.Background(), engine).Await(context.Background())
	assertChainRetryUnsafe(t, err, outcome, laterErr)
}

func TestChainRejectedAdjustmentBatchMarksRetryUnsafe(t *testing.T) {
	laterErr := errors.New("later step failed")
	driver := &chainAccountAdjustmentResultDriver{
		acceptingDriver: newAcceptingDriver(),
		result: accountadjustment.BatchResult{
			BatchError: optional.Some(reject.AccountAdjustmentBatchError{}),
		},
	}
	engine := newChainEngine(t, driver)
	var outcome ChainOutcome
	runner := Chain(
		param.NewAccountIDFromUint64(42),
		func(context.Context) (struct{}, error) { return struct{}{}, nil },
	).ApplyAccountAdjustment(AccountAdjustmentHooks[struct{}]{
		Adjustments: func(context.Context, struct{}) ([]model.AccountAdjustment, error) {
			return []model.AccountAdjustment{model.NewAccountAdjustment()}, nil
		},
		OnAdjusted: func(
			context.Context,
			struct{},
			accountadjustment.BatchResult,
		) error {
			return nil
		},
	}).Then(func(context.Context, struct{}) error {
		return laterErr
	}).Finally(func(_ context.Context, _ struct{}, got ChainOutcome) error {
		outcome = got
		return nil
	})

	_, err := runner.Run(context.Background(), engine).Await(context.Background())
	assertChainRetryUnsafe(t, err, outcome, laterErr)
}

func TestChainAccountAdjustmentDriverFailureMarksRetryUnsafe(t *testing.T) {
	driverErr := errors.New("account adjustment transport failed")
	for _, test := range []struct {
		name        string
		adjustments []model.AccountAdjustment
	}{
		{
			name:        "non-empty batch",
			adjustments: []model.AccountAdjustment{model.NewAccountAdjustment()},
		},
		{name: "empty batch"},
	} {
		t.Run(test.name, func(t *testing.T) {
			driver := &chainAccountAdjustmentResultDriver{
				acceptingDriver: newAcceptingDriver(),
				err:             driverErr,
			}
			engine := newChainEngine(t, driver)
			adjusted := false
			var outcome ChainOutcome
			runner := Chain(
				param.NewAccountIDFromUint64(42),
				func(context.Context) (struct{}, error) { return struct{}{}, nil },
			).ApplyAccountAdjustment(AccountAdjustmentHooks[struct{}]{
				Adjustments: func(
					context.Context,
					struct{},
				) ([]model.AccountAdjustment, error) {
					return test.adjustments, nil
				},
				OnAdjusted: func(
					context.Context,
					struct{},
					accountadjustment.BatchResult,
				) error {
					adjusted = true
					return nil
				},
			}).Finally(func(_ context.Context, _ struct{}, got ChainOutcome) error {
				outcome = got
				return nil
			})

			_, err := runner.Run(context.Background(), engine).Await(
				context.Background(),
			)
			if adjusted {
				t.Error("OnAdjusted() called after driver failure")
			}
			assertChainRetryUnsafe(t, err, outcome, driverErr)
		})
	}
}

func TestChainMarksLaterFailureAfterReservationCommitRetryUnsafe(t *testing.T) {
	laterErr := errors.New("later step failed")
	engine := newChainEngine(t, newAcceptingDriver())
	var outcome ChainOutcome
	runner := Chain(
		buildTestOrder(t, 42),
		func(context.Context) (struct{}, error) { return struct{}{}, nil },
	).ExecutePreTrade(PreTradeHooks[struct{}]{
		OnRejected: func(context.Context, struct{}, []reject.Reject) error {
			return errors.New("OnRejected() called for accepted reservation")
		},
		OnReserved: func(
			context.Context,
			struct{},
			OperationResult,
		) (Decision, error) {
			return DecisionCommit, nil
		},
	}).Then(func(context.Context, struct{}) error {
		return laterErr
	}).Finally(func(_ context.Context, _ struct{}, got ChainOutcome) error {
		outcome = got
		return nil
	})
	_, err := runner.Run(context.Background(), engine).Await(context.Background())
	assertChainRetryUnsafe(t, err, outcome, laterErr)
}

func TestChainSuccessfulMutationReturnsRetryUnsafeOutcome(t *testing.T) {
	engine := newChainEngine(t, newAcceptingDriver())
	accountID := param.NewAccountIDFromUint64(42)
	report := buildTestReport(t, 42)
	var outcome ChainOutcome
	runner := Chain(
		accountID,
		func(context.Context) (struct{}, error) { return struct{}{}, nil },
	).ApplyExecutionReport(ExecutionReportHooks[struct{}]{
		Report: func(context.Context, struct{}) (model.ExecutionReport, error) {
			return report, nil
		},
		OnSettled: func(context.Context, struct{}, pretrade.PostTradeResult) error {
			return nil
		},
	}).Finally(func(_ context.Context, _ struct{}, got ChainOutcome) error {
		outcome = got
		return nil
	})
	_, err := runner.Run(context.Background(), engine).Await(context.Background())
	if err != nil {
		t.Fatalf("chain Await() error = %v, want nil", err)
	}
	if errors.Is(err, ErrChainRetryUnsafe) {
		t.Errorf("chain Await() error = %v, unexpectedly marks retry unsafe", err)
	}
	if outcome.Status != ChainOutcomeCompleted {
		t.Errorf("outcome status = %v, want ChainOutcomeCompleted", outcome.Status)
	}
	if outcome.Err != nil {
		t.Errorf("outcome error = %v, want nil", outcome.Err)
	}
	if !outcome.RetryUnsafe {
		t.Error("outcome RetryUnsafe = false, want true")
	}
}

func TestChainRollbackMarksRetryUnsafe(t *testing.T) {
	engine := newChainEngine(t, newAcceptingDriver())
	var outcome ChainOutcome
	runner := Chain(
		buildTestOrder(t, 42),
		func(context.Context) (struct{}, error) { return struct{}{}, nil },
	).ExecutePreTrade(PreTradeHooks[struct{}]{
		OnRejected: func(context.Context, struct{}, []reject.Reject) error {
			return errors.New("OnRejected() called for accepted reservation")
		},
		OnReserved: func(
			context.Context,
			struct{},
			OperationResult,
		) (Decision, error) {
			return DecisionRollback, nil
		},
	}).Finally(func(_ context.Context, _ struct{}, got ChainOutcome) error {
		outcome = got
		return nil
	})
	_, err := runner.Run(context.Background(), engine).Await(context.Background())
	if err != nil {
		t.Fatalf("chain Await() error = %v, want nil", err)
	}
	if outcome.Status != ChainOutcomeRejected {
		t.Errorf("outcome status = %v, want ChainOutcomeRejected", outcome.Status)
	}
	if outcome.Err != nil {
		t.Errorf("outcome error = %v, want nil", outcome.Err)
	}
	if !outcome.RetryUnsafe {
		t.Error("outcome RetryUnsafe = false, want true")
	}
}

func TestChainRejectedDriverReturnsRetryUnsafeOutcome(t *testing.T) {
	for _, test := range []struct {
		name     string
		dropCopy bool
	}{
		{name: "execute pre-trade"},
		{name: "apply drop copy", dropCopy: true},
	} {
		t.Run(test.name, func(t *testing.T) {
			engine := newChainEngine(t, newRejectDriver())
			calledReject := false
			calledAccepted := false
			chain := Chain(
				buildTestOrder(t, 42),
				func(context.Context) (struct{}, error) { return struct{}{}, nil },
			)
			if test.dropCopy {
				chain.ApplyDropCopy(DropCopyHooks[struct{}]{
					OnRejected: func(context.Context, struct{}, []reject.Reject) error {
						calledReject = true
						return nil
					},
					OnApplied: func(context.Context, struct{}, DropCopyResult) (Decision, error) {
						calledAccepted = true
						return DecisionCommit, nil
					},
				})
			} else {
				chain.ExecutePreTrade(PreTradeHooks[struct{}]{
					OnRejected: func(context.Context, struct{}, []reject.Reject) error {
						calledReject = true
						return nil
					},
					OnReserved: func(context.Context, struct{}, OperationResult) (Decision, error) {
						calledAccepted = true
						return DecisionCommit, nil
					},
				})
			}

			outcome, err := chain.Run(context.Background(), engine).Await(context.Background())
			if err != nil {
				t.Fatalf("chain Await() error = %v, want nil", err)
			}
			if outcome.Status != ChainOutcomeRejected {
				t.Errorf("outcome status = %v, want ChainOutcomeRejected", outcome.Status)
			}
			if outcome.Err != nil {
				t.Errorf("outcome error = %v, want nil", outcome.Err)
			}
			if !outcome.RetryUnsafe {
				t.Error("outcome RetryUnsafe = false, want true")
			}
			if !calledReject {
				t.Error("OnRejected() was not called")
			}
			if calledAccepted {
				t.Error("accepted hook was called for a rejected operation")
			}
		})
	}
}

// Both stateful driver calls mark the chain retry unsafe before they are
// entered, and every way such a call can end has to keep the mark: the engine
// consumes rate-limit budget and can arm an account block before any handle
// exists, so a caller that reruns the chain spends it twice. The mark has two
// carriers and each outcome uses the ones it has - a failed chain wraps
// ErrChainRetryUnsafe in its error and reports RetryUnsafe, a rejected or
// rolled back one has no error at all and reports RetryUnsafe alone.
//
// The matrix asserts the outcome the future resolved with. The single-purpose
// tests around it read the outcome Finally received instead, and that is the
// distinction worth keeping: an implementation that computed the mark for the
// terminal hook and dropped it on the way to the future would satisfy them and
// fail here.
func TestChainStatefulOperationOutcomesMarkRetryUnsafe(t *testing.T) {
	rejectHookErr := errors.New("reject hook failed")
	acceptedHookErr := errors.New("accepted hook failed")
	panicErr := errors.New("accepted hook panicked")
	laterErr := errors.New("later step failed")
	for _, operation := range []struct {
		name     string
		dropCopy bool
	}{
		{name: "execute pre-trade"},
		{name: "apply drop copy", dropCopy: true},
	} {
		t.Run(operation.name, func(t *testing.T) {
			for _, test := range []struct {
				rejectHookErr error
				acceptedErr   error
				wantErr       error
				name          string
				decision      Decision
				rejects       bool
				panicInHook   bool
				later         bool
				wantRejected  int
				wantAccepted  int
				wantLater     int
			}{
				{
					name:          "reject hook error",
					rejects:       true,
					rejectHookErr: rejectHookErr,
					wantErr:       rejectHookErr,
					wantRejected:  1,
				},
				{
					name:         "accepted hook error",
					acceptedErr:  acceptedHookErr,
					wantErr:      acceptedHookErr,
					wantAccepted: 1,
				},
				{
					name:         "accepted hook panic",
					panicInHook:  true,
					wantErr:      panicErr,
					wantAccepted: 1,
				},
				{
					name:         "accepted hook rollback",
					decision:     DecisionRollback,
					wantAccepted: 1,
				},
				{
					name:         "later failure after commit",
					decision:     DecisionCommit,
					later:        true,
					wantErr:      laterErr,
					wantAccepted: 1,
					wantLater:    1,
				},
			} {
				t.Run(test.name, func(t *testing.T) {
					driver := Driver(newAcceptingDriver())
					if test.rejects {
						driver = newRejectDriver()
					}
					engine := newChainEngine(t, driver)
					rejectedCalls := 0
					acceptedCalls := 0
					laterCalls := 0
					onRejected := func(
						context.Context,
						struct{},
						[]reject.Reject,
					) error {
						rejectedCalls++
						return test.rejectHookErr
					}
					decide := func() (Decision, error) {
						acceptedCalls++
						if test.panicInHook {
							panic(panicErr)
						}
						return test.decision, test.acceptedErr
					}
					chain := Chain(
						buildTestOrder(t, 42),
						func(context.Context) (struct{}, error) {
							return struct{}{}, nil
						},
					)
					if operation.dropCopy {
						chain.ApplyDropCopy(DropCopyHooks[struct{}]{
							OnRejected: onRejected,
							OnApplied: func(
								context.Context,
								struct{},
								DropCopyResult,
							) (Decision, error) {
								return decide()
							},
						})
					} else {
						chain.ExecutePreTrade(PreTradeHooks[struct{}]{
							OnRejected: onRejected,
							OnReserved: func(
								context.Context,
								struct{},
								OperationResult,
							) (Decision, error) {
								return decide()
							},
						})
					}
					if test.later {
						chain.Then(func(context.Context, struct{}) error {
							laterCalls++
							return laterErr
						})
					}

					outcome, err := chain.Run(context.Background(), engine).Await(
						context.Background(),
					)
					if test.wantErr != nil {
						assertChainRetryUnsafe(t, err, outcome, test.wantErr)
					} else {
						assertChainRejectedRetryUnsafe(t, err, outcome)
					}
					if rejectedCalls != test.wantRejected {
						t.Errorf(
							"reject hook calls = %d, want %d",
							rejectedCalls,
							test.wantRejected,
						)
					}
					if acceptedCalls != test.wantAccepted {
						t.Errorf(
							"accepted hook calls = %d, want %d",
							acceptedCalls,
							test.wantAccepted,
						)
					}
					if laterCalls != test.wantLater {
						t.Errorf(
							"later step calls = %d, want %d",
							laterCalls,
							test.wantLater,
						)
					}
				})
			}
		})
	}
}

func TestChainFinallyRunsOnceForEveryStartedOutcome(t *testing.T) {
	hookErr := errors.New("hook failed")
	panicErr := errors.New("hook panicked")
	for _, test := range []struct {
		name       string
		path       string
		wantStatus ChainOutcomeStatus
		wantErr    error
		wantUnsafe bool
	}{
		{name: "completion", path: "completion", wantStatus: ChainOutcomeCompleted},
		{
			name: "rejection", path: "rejection", wantStatus: ChainOutcomeRejected,
			wantUnsafe: true,
		},
		{
			name: "hook error", path: "hook error",
			wantStatus: ChainOutcomeFailed, wantErr: hookErr,
		},
		{
			name: "hook panic", path: "hook panic",
			wantStatus: ChainOutcomeFailed, wantErr: panicErr,
		},
		{
			name: "invalid decision", path: "invalid decision",
			wantStatus: ChainOutcomeFailed, wantErr: ErrChainInvalidDecision, wantUnsafe: true,
		},
	} {
		t.Run(test.name, func(t *testing.T) {
			driver := Driver(newAcceptingDriver())
			if test.path == "rejection" {
				driver = &chainRejectDriver{newAcceptingDriver()}
			}
			engine := newChainEngine(t, driver)
			var beginContext context.Context
			finallyCount := 0
			var gotOutcome ChainOutcome
			chain := Chain(
				buildTestOrder(t, 42),
				func(ctx context.Context) (int, error) {
					beginContext = ctx
					return 7, nil
				},
			)
			switch test.path {
			case "completion":
				chain.Then(func(ctx context.Context, state int) error {
					if ctx != beginContext || state != 7 {
						t.Error("Then received another chain context or State")
					}
					return nil
				})
			case "rejection":
				chain.ExecutePreTrade(PreTradeHooks[int]{
					OnRejected: func(
						ctx context.Context, state int, _ []reject.Reject,
					) error {
						if ctx != beginContext || state != 7 {
							t.Error("OnRejected received another chain context or State")
						}
						return nil
					},
					OnReserved: func(
						context.Context, int, OperationResult,
					) (Decision, error) {
						return DecisionRollback, errors.New(
							"OnReserved called for rejected operation",
						)
					},
				})
			case "hook error":
				chain.Then(func(context.Context, int) error { return hookErr })
			case "hook panic":
				chain.Then(func(context.Context, int) error { panic(panicErr) })
			case "invalid decision":
				chain.ExecutePreTrade(PreTradeHooks[int]{
					OnRejected: func(
						context.Context, int, []reject.Reject,
					) error {
						return errors.New("OnRejected called for accepted operation")
					},
					OnReserved: func(
						context.Context, int, OperationResult,
					) (Decision, error) {
						return decisionInvalid, nil
					},
				})
			}
			var f *future.Future[ChainOutcome]
			runner := chain.Finally(func(
				ctx context.Context, state int, outcome ChainOutcome,
			) error {
				finallyCount++
				gotOutcome = outcome
				if ctx != beginContext || state != 7 {
					t.Error("Finally received another chain context or State")
				}
				if f.Done() {
					t.Error("future resolved before Finally")
				}
				return nil
			})
			f = future.New[ChainOutcome]()
			task, _, err := runner.newChainTask(context.Background(), engine, f)
			if err != nil {
				t.Fatalf("newChainTask() error = %v", err)
			}
			task.run()
			_, awaitErr := f.Await(context.Background())
			if test.wantErr == nil && awaitErr != nil {
				t.Fatalf("chain Await() error = %v", awaitErr)
			}
			if test.wantErr != nil && !errors.Is(awaitErr, test.wantErr) {
				t.Fatalf("chain Await() error = %v, want %v", awaitErr, test.wantErr)
			}
			if finallyCount != 1 {
				t.Errorf("Finally count = %d, want 1", finallyCount)
			}
			if gotOutcome.Status != test.wantStatus {
				t.Errorf(
					"Finally status = %v, want %v",
					gotOutcome.Status, test.wantStatus,
				)
			}
			if test.wantErr == nil && gotOutcome.Err != nil {
				t.Errorf("Finally outcome error = %v, want nil", gotOutcome.Err)
			}
			if test.wantErr != nil && !errors.Is(gotOutcome.Err, test.wantErr) {
				t.Errorf(
					"Finally outcome error = %v, want %v",
					gotOutcome.Err, test.wantErr,
				)
			}
			if gotOutcome.RetryUnsafe != test.wantUnsafe {
				t.Errorf(
					"Finally RetryUnsafe = %v, want %v",
					gotOutcome.RetryUnsafe,
					test.wantUnsafe,
				)
			}
		})
	}
}

func TestChainFinallyRunsAfterPendingCleanup(t *testing.T) {
	order := buildChainCheckOrder(t)
	reservation := newChainReservation(t, order)
	engine := newChainEngine(t, &chainReservationDriver{
		acceptingDriver: newAcceptingDriver(),
		reservation:     reservation,
	})
	hookErr := errors.New("reservation hook failed")
	var f *future.Future[ChainOutcome]
	chain := Chain(
		order,
		func(context.Context) (struct{}, error) { return struct{}{}, nil },
	).ExecutePreTrade(PreTradeHooks[struct{}]{
		OnRejected: func(context.Context, struct{}, []reject.Reject) error {
			return errors.New("OnRejected called for accepted operation")
		},
		OnReserved: func(
			context.Context, struct{}, OperationResult,
		) (Decision, error) {
			return decisionInvalid, hookErr
		},
	}).
		Finally(func(context.Context, struct{}, ChainOutcome) error {
			if _, err := reservation.Lock(); !errors.Is(
				err, pretrade.ErrReservationClosed,
			) {
				t.Errorf(
					"reservation Lock() in Finally error = %v, "+
						"want ErrReservationClosed",
					err,
				)
			}
			if f.Done() {
				t.Error("future resolved before Finally")
			}
			return nil
		})
	f = future.New[ChainOutcome]()
	task, _, err := chain.newChainTask(context.Background(), engine, f)
	if err != nil {
		t.Fatalf("newChainTask() error = %v", err)
	}
	task.run()
	if _, err := f.Await(context.Background()); !errors.Is(err, hookErr) {
		t.Fatalf("chain Await() error = %v, want %v", err, hookErr)
	}
}

func TestChainFinallyReportsItsErrorWithoutHidingOriginal(t *testing.T) {
	for _, test := range []struct {
		name  string
		panic bool
	}{
		{name: "returned error"},
		{name: "panic", panic: true},
	} {
		t.Run(test.name, func(t *testing.T) {
			engine := newChainEngine(t, newAcceptingDriver())
			originalErr := errors.New("chain failed")
			finallyErr := errors.New("finalization failed")
			chain := Chain(
				param.NewAccountIDFromUint64(42),
				func(context.Context) (struct{}, error) {
					return struct{}{}, nil
				},
			).
				Then(func(context.Context, struct{}) error { return originalErr }).
				Finally(func(context.Context, struct{}, ChainOutcome) error {
					if test.panic {
						panic(finallyErr)
					}
					return finallyErr
				})
			outcome, err := chain.Run(context.Background(), engine).Await(
				context.Background(),
			)
			if !errors.Is(err, originalErr) || !errors.Is(err, finallyErr) {
				t.Fatalf(
					"chain Await() error = %v, want original and final errors",
					err,
				)
			}
			if outcome.Status != ChainOutcomeFailed {
				t.Errorf(
					"outcome status = %v, want ChainOutcomeFailed",
					outcome.Status,
				)
			}
			if !errors.Is(outcome.Err, originalErr) ||
				!errors.Is(outcome.Err, finallyErr) {
				t.Errorf(
					"outcome error = %v, want original and final errors",
					outcome.Err,
				)
			}
		})
	}
}

func TestChainFinallyFailureReturnsFailedOutcome(t *testing.T) {
	finallyErr := errors.New("finalization failed")
	for _, test := range []struct {
		name     string
		panic    bool
		rejected bool
	}{
		{name: "returned error after mutation"},
		{name: "panic after mutation", panic: true},
		{name: "returned error after rejection", rejected: true},
	} {
		t.Run(test.name, func(t *testing.T) {
			engine := newChainEngine(t, newAcceptingDriver())
			var beforeFinally ChainOutcome
			finally := func(
				_ context.Context,
				_ struct{},
				outcome ChainOutcome,
			) error {
				beforeFinally = outcome
				if test.panic {
					panic(finallyErr)
				}
				return finallyErr
			}
			var runner *ChainRunner[struct{}]
			if test.rejected {
				engine = newChainEngine(t, newRejectDriver())
				runner = Chain(
					buildTestOrder(t, 42),
					func(context.Context) (struct{}, error) {
						return struct{}{}, nil
					},
				).ExecutePreTrade(PreTradeHooks[struct{}]{
					OnRejected: func(
						context.Context,
						struct{},
						[]reject.Reject,
					) error {
						return nil
					},
					OnReserved: func(
						context.Context,
						struct{},
						OperationResult,
					) (Decision, error) {
						return DecisionRollback, errors.New(
							"OnReserved called for rejected operation",
						)
					},
				}).Finally(finally)
			} else {
				report := buildTestReport(t, 42)
				runner = Chain(
					param.NewAccountIDFromUint64(42),
					func(context.Context) (struct{}, error) {
						return struct{}{}, nil
					},
				).ApplyExecutionReport(ExecutionReportHooks[struct{}]{
					Report: func(
						context.Context,
						struct{},
					) (model.ExecutionReport, error) {
						return report, nil
					},
					OnSettled: func(
						context.Context,
						struct{},
						pretrade.PostTradeResult,
					) error {
						return nil
					},
				}).Finally(finally)
			}

			outcome, err := runner.Run(context.Background(), engine).Await(
				context.Background(),
			)
			if !errors.Is(err, finallyErr) {
				t.Errorf("chain Await() error = %v, want terminal-hook error", err)
			}
			if !errors.Is(err, ErrChainRetryUnsafe) {
				t.Errorf(
					"chain Await() error = %v, want ErrChainRetryUnsafe",
					err,
				)
			}
			if outcome.Status != ChainOutcomeFailed {
				t.Errorf(
					"resolved outcome status = %v, want ChainOutcomeFailed",
					outcome.Status,
				)
			}
			if !errors.Is(outcome.Err, finallyErr) {
				t.Errorf(
					"resolved outcome error = %v, want terminal-hook error",
					outcome.Err,
				)
			}
			if !errors.Is(outcome.Err, ErrChainRetryUnsafe) {
				t.Errorf(
					"resolved outcome error = %v, want ErrChainRetryUnsafe",
					outcome.Err,
				)
			}
			if !outcome.RetryUnsafe {
				t.Error("resolved outcome RetryUnsafe = false, want true")
			}

			wantBeforeFinallyStatus := ChainOutcomeCompleted
			if test.rejected {
				wantBeforeFinallyStatus = ChainOutcomeRejected
			}
			if beforeFinally.Status != wantBeforeFinallyStatus {
				t.Errorf(
					"Finally outcome status = %v, want %v",
					beforeFinally.Status,
					wantBeforeFinallyStatus,
				)
			}
			if beforeFinally.Err != nil {
				t.Errorf(
					"Finally outcome error = %v, want nil",
					beforeFinally.Err,
				)
			}
			if !beforeFinally.RetryUnsafe {
				t.Error("Finally outcome RetryUnsafe = false, want true")
			}
		})
	}
}

func TestChainFinallyDoesNotRunWhenBeginFails(t *testing.T) {
	engine := newChainEngine(t, newAcceptingDriver())
	beginErr := errors.New("begin failed")
	finallyCount := 0
	chain := Chain(
		param.NewAccountIDFromUint64(42),
		func(context.Context) (struct{}, error) {
			return struct{}{}, beginErr
		},
	).
		Finally(func(context.Context, struct{}, ChainOutcome) error {
			finallyCount++
			return nil
		})
	if _, err := chain.Run(context.Background(), engine).Await(
		context.Background(),
	); !errors.Is(err, beginErr) {
		t.Fatalf("chain Await() error = %v, want %v", err, beginErr)
	}
	if finallyCount != 0 {
		t.Errorf("Finally count = %d, want 0", finallyCount)
	}
}

func TestChainHardStopAbortDoesNotRunCallerCode(t *testing.T) {
	driver := newAcceptingDriver()
	engine, err := NewBuilder(driver).Sharded(1).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	accountID := param.NewAccountIDFromUint64(42)
	entered := make(chan struct{})
	release := make(chan struct{})
	first := engine.Submit(context.Background(), accountID, func() error {
		close(entered)
		<-release
		return nil
	})
	<-entered

	var beginCount atomic.Int64
	var callerCount atomic.Int64
	var finallyCount atomic.Int64
	chainFuture := Chain(
		buildTestOrder(t, 42),
		func(context.Context) (struct{}, error) {
			beginCount.Add(1)
			return struct{}{}, nil
		},
	).
		Then(func(context.Context, struct{}) error {
			callerCount.Add(1)
			return nil
		}).
		ExecutePreTrade(PreTradeHooks[struct{}]{
			OnRejected: func(context.Context, struct{}, []reject.Reject) error {
				callerCount.Add(1)
				return nil
			},
			OnReserved: func(
				context.Context, struct{}, OperationResult,
			) (Decision, error) {
				callerCount.Add(1)
				return DecisionRollback, nil
			},
		}).
		Finally(func(context.Context, struct{}, ChainOutcome) error {
			finallyCount.Add(1)
			return nil
		}).
		Run(context.Background(), engine)

	hardStopDone := make(chan error, 1)
	go func() { hardStopDone <- engine.StopHard(context.Background()) }()
	deadline := time.Now().Add(5 * time.Second)
	for !isStrategyHardStopped(engine.strategy) {
		if time.Now().After(deadline) {
			t.Fatal("timeout waiting for hard stop signal")
		}
	}
	close(release)
	if _, err := first.Await(context.Background()); err != nil {
		t.Fatalf("first Await() error = %v", err)
	}
	outcome, err := chainFuture.Await(context.Background())
	if !errors.Is(err, ErrStopped) {
		t.Fatalf("chain Await() error = %v, want ErrStopped", err)
	}
	if outcome.Status != ChainOutcomeFailed {
		t.Errorf("outcome status = %v, want ChainOutcomeFailed", outcome.Status)
	}
	if !errors.Is(outcome.Err, ErrStopped) {
		t.Errorf("outcome error = %v, want ErrStopped", outcome.Err)
	}
	if outcome.RetryUnsafe {
		t.Error("outcome RetryUnsafe = true, want false")
	}
	if err := <-hardStopDone; err != nil {
		t.Fatalf("StopHard() error = %v", err)
	}
	if got := beginCount.Load(); got != 0 {
		t.Errorf("Begin count = %d, want 0", got)
	}
	if got := callerCount.Load(); got != 0 {
		t.Errorf("Then count = %d, want 0", got)
	}
	if got := finallyCount.Load(); got != 0 {
		t.Errorf("Finally count = %d, want 0", got)
	}
	if got := atomic.LoadInt64(&driver.executeCount); got != 0 {
		t.Errorf("ExecutePreTrade count = %d, want 0 on abort", got)
	}
}

func TestChainQueueRetireDrainAbortDoesNotRunCallerCode(t *testing.T) {
	driver := newAcceptingDriver()
	engine := newChainEngine(t, driver)
	accountID := param.NewAccountIDFromUint64(42)
	var beginCount atomic.Int64
	var callerCount atomic.Int64
	var finallyCount atomic.Int64
	chain := Chain(
		buildTestOrder(t, 42),
		func(context.Context) (struct{}, error) {
			beginCount.Add(1)
			return struct{}{}, nil
		},
	).
		Then(func(context.Context, struct{}) error {
			callerCount.Add(1)
			return nil
		}).
		ExecutePreTrade(PreTradeHooks[struct{}]{
			OnRejected: func(context.Context, struct{}, []reject.Reject) error {
				callerCount.Add(1)
				return nil
			},
			OnReserved: func(
				context.Context, struct{}, OperationResult,
			) (Decision, error) {
				callerCount.Add(1)
				return DecisionRollback, nil
			},
		}).
		Finally(func(context.Context, struct{}, ChainOutcome) error {
			finallyCount.Add(1)
			return nil
		})
	f := future.New[ChainOutcome]()
	task, _, err := chain.newChainTask(context.Background(), engine, f)
	if err != nil {
		t.Fatalf("newChainTask() error = %v", err)
	}
	q := newKeyQueue(1)
	q.ch <- queuedTask{task: task, key: accountRoutingKey(accountID)}
	b := newBase(baseConfig{}, false)
	drained := make(chan struct{})
	go func() {
		b.drainAndAbort(q)
		close(drained)
	}()
	if _, err := f.Await(context.Background()); !errors.Is(err, ErrStopped) {
		t.Fatalf("chain Await() error = %v, want ErrStopped", err)
	}
	<-drained
	if got := beginCount.Load(); got != 0 {
		t.Errorf("Begin count = %d, want 0", got)
	}
	if got := callerCount.Load(); got != 0 {
		t.Errorf("Then count = %d, want 0", got)
	}
	if got := finallyCount.Load(); got != 0 {
		t.Errorf("Finally count = %d, want 0", got)
	}
	if got := atomic.LoadInt64(&driver.executeCount); got != 0 {
		t.Errorf("ExecutePreTrade count = %d, want 0 on abort", got)
	}
}

func TestChainDropCopyPreparesReportBeforeCommitAndSettles(t *testing.T) {
	driver := newAcceptingDriver()
	engine := newChainEngine(t, driver)
	var beginContext context.Context
	type state struct {
		report model.ExecutionReport
		phase  int
	}
	chain := Chain(
		buildTestOrder(t, 42),
		func(ctx context.Context) (*state, error) {
			beginContext = ctx
			return &state{phase: 1}, nil
		},
	).ApplyDropCopy(DropCopyHooks[*state]{
		OnRejected: func(context.Context, *state, []reject.Reject) error {
			return errors.New("OnRejected() called for accepted drop copy")
		},
		OnApplied: func(ctx context.Context, got *state, _ DropCopyResult) (Decision, error) {
			if ctx != beginContext {
				t.Error("OnApplied received another chain context")
			}
			if got.phase != 1 {
				t.Errorf("OnApplied state phase = %d, want 1", got.phase)
			}
			got.report = buildTestReport(t, 42)
			got.phase = 2
			return DecisionCommit, nil
		},
	}).ApplyExecutionReport(ExecutionReportHooks[*state]{
		Report: func(ctx context.Context, got *state) (model.ExecutionReport, error) {
			if ctx != beginContext {
				t.Error("ApplyExecutionReport received another chain context")
			}
			if got.phase != 2 {
				t.Errorf("ApplyExecutionReport state phase = %d, want 2", got.phase)
			}
			got.phase = 3
			return got.report, nil
		},
		OnSettled: func(ctx context.Context, got *state, _ pretrade.PostTradeResult) error {
			if ctx != beginContext {
				t.Error("OnSettled received another chain context")
			}
			if got.phase != 3 {
				t.Errorf("OnSettled state phase = %d, want 3", got.phase)
			}
			got.phase = 4
			return nil
		},
	})
	if _, err := chain.Run(context.Background(), engine).Await(context.Background()); err != nil {
		t.Fatalf("chain Await() error = %v", err)
	}
	if got := atomic.LoadInt64(&driver.executeCount); got != 1 {
		t.Errorf("drop-copy count = %d, want 1", got)
	}
	if got := atomic.LoadInt64(&driver.reportCount); got != 1 {
		t.Errorf("execution-report count = %d, want 1", got)
	}
}

func TestChainRollbackEndsBeforeLaterSteps(t *testing.T) {
	driver := newAcceptingDriver()
	engine := newChainEngine(t, driver)
	chain := Chain(
		buildTestOrder(t, 42),
		func(context.Context) (struct{}, error) { return struct{}{}, nil },
	).ExecutePreTrade(PreTradeHooks[struct{}]{
		OnRejected: func(context.Context, struct{}, []reject.Reject) error {
			return errors.New("OnRejected() called for accepted reservation")
		},
		OnReserved: func(context.Context, struct{}, OperationResult) (Decision, error) {
			return DecisionRollback, nil
		},
	}).ApplyExecutionReport(ExecutionReportHooks[struct{}]{
		Report: func(context.Context, struct{}) (model.ExecutionReport, error) {
			return model.ExecutionReport{}, errors.New(
				"ApplyExecutionReport() called after rollback",
			)
		},
		OnSettled: func(context.Context, struct{}, pretrade.PostTradeResult) error {
			return errors.New("OnSettled() called after rollback")
		},
	})
	if _, err := chain.Run(context.Background(), engine).Await(context.Background()); err != nil {
		t.Fatalf("chain Await() error = %v", err)
	}
	if got := atomic.LoadInt64(&driver.reportCount); got != 0 {
		t.Errorf("execution-report count = %d, want 0", got)
	}
}

func TestChainInvalidatesEscapedOperationResultAfterDecision(t *testing.T) {
	engine := newChainEngine(t, newAcceptingDriver())
	type state struct {
		result OperationResult
	}
	chain := Chain(
		buildTestOrder(t, 42),
		func(context.Context) (*state, error) { return &state{}, nil },
	).ExecutePreTrade(PreTradeHooks[*state]{
		OnRejected: func(context.Context, *state, []reject.Reject) error {
			return errors.New("OnRejected() called for accepted reservation")
		},
		OnReserved: func(_ context.Context, got *state, result OperationResult) (Decision, error) {
			got.result = result
			return DecisionCommit, nil
		},
	}).
		Then(func(_ context.Context, got *state) error {
			_, err := got.result.Lock()
			if !errors.Is(err, ErrChainResultUnavailable) {
				return fmt.Errorf(
					"escaped result Lock() error = %w, want ErrChainResultUnavailable",
					err,
				)
			}
			return nil
		})
	if _, err := chain.Run(context.Background(), engine).Await(context.Background()); err != nil {
		t.Fatalf("chain Await() error = %v", err)
	}
}

func TestChainApplyExecutionReportSupportsCallerWorkAroundIt(t *testing.T) {
	driver := newAcceptingDriver()
	engine := newChainEngine(t, driver)
	accountID := param.NewAccountIDFromUint64(42)
	type state struct{ phase int }
	chain := Chain(
		accountID,
		func(context.Context) (*state, error) { return &state{phase: 1}, nil },
	).
		Then(func(_ context.Context, got *state) error {
			if got.phase != 1 {
				t.Errorf("before report phase = %d, want 1", got.phase)
			}
			got.phase = 2
			return nil
		}).
		ApplyExecutionReport(ExecutionReportHooks[*state]{
			Report: func(_ context.Context, got *state) (model.ExecutionReport, error) {
				if got.phase != 2 {
					t.Errorf("report producer phase = %d, want 2", got.phase)
				}
				got.phase = 3
				return buildTestReport(t, 42), nil
			},
			OnSettled: func(_ context.Context, got *state, _ pretrade.PostTradeResult) error {
				if got.phase != 3 {
					t.Errorf("settled phase = %d, want 3", got.phase)
				}
				got.phase = 4
				return nil
			},
		})
	if _, err := chain.Run(context.Background(), engine).Await(context.Background()); err != nil {
		t.Fatalf("chain Await() error = %v", err)
	}
	if got := atomic.LoadInt64(&driver.reportCount); got != 1 {
		t.Errorf("execution-report count = %d, want 1", got)
	}
}

func TestChainApplyAccountAdjustmentKeepsCallerReadAndPersistenceTogether(t *testing.T) {
	driver := newFakeDriver()
	engine := newChainEngine(t, driver)
	accountID := param.NewAccountIDFromUint64(42)
	type state struct{ phase int }
	var beginContext context.Context
	chain := Chain(
		accountID,
		func(ctx context.Context) (*state, error) {
			beginContext = ctx
			return &state{phase: 1}, nil
		},
	).
		Then(func(ctx context.Context, got *state) error {
			if ctx != beginContext {
				t.Error("Then received another chain context")
			}
			if got.phase != 1 {
				t.Errorf("before adjustment phase = %d, want 1", got.phase)
			}
			got.phase = 2
			return nil
		}).
		ApplyAccountAdjustment(AccountAdjustmentHooks[*state]{
			Adjustments: func(ctx context.Context, got *state) ([]model.AccountAdjustment, error) {
				if ctx != beginContext {
					t.Error("ApplyAccountAdjustment received another chain context")
				}
				if got.phase != 2 {
					t.Errorf("adjustment input phase = %d, want 2", got.phase)
				}
				got.phase = 3
				return nil, nil
			},
			OnAdjusted: func(ctx context.Context, got *state, _ accountadjustment.BatchResult) error {
				if ctx != beginContext {
					t.Error("OnAdjusted received another chain context")
				}
				if got.phase != 3 {
					t.Errorf("adjusted phase = %d, want 3", got.phase)
				}
				got.phase = 4
				return nil
			},
		}).
		Then(func(_ context.Context, got *state) error {
			if got.phase != 4 {
				t.Errorf("after adjustment phase = %d, want 4", got.phase)
			}
			got.phase = 5
			return nil
		})
	if _, err := chain.Run(context.Background(), engine).Await(context.Background()); err != nil {
		t.Fatalf("chain Await() error = %v", err)
	}
	if got := atomic.LoadInt64(&driver.adjustmentCount); got != 1 {
		t.Errorf("adjustment count = %d, want 1", got)
	}
}

func TestChainRejectsNilHookAtRun(t *testing.T) {
	engine := newChainEngine(t, newAcceptingDriver())
	chain := Chain(
		buildTestOrder(t, 42),
		func(context.Context) (struct{}, error) { return struct{}{}, nil },
	).ExecutePreTrade(PreTradeHooks[struct{}]{
		OnRejected: func(context.Context, struct{}, []reject.Reject) error {
			return nil
		},
	})
	outcome, err := chain.Run(context.Background(), engine).Await(context.Background())
	if !errors.Is(err, ErrChainIncomplete) {
		t.Fatalf("chain Await() error = %v, want ErrChainIncomplete", err)
	}
	if outcome.Status != ChainOutcomeFailed {
		t.Errorf("outcome status = %v, want ChainOutcomeFailed", outcome.Status)
	}
	if !errors.Is(outcome.Err, ErrChainIncomplete) {
		t.Errorf("outcome error = %v, want ErrChainIncomplete", outcome.Err)
	}
	if outcome.RetryUnsafe {
		t.Error("outcome RetryUnsafe = true, want false")
	}
}

func TestChainRejectsNilBeginAtRun(t *testing.T) {
	engine := newChainEngine(t, newAcceptingDriver())
	chain := Chain[struct{}](param.NewAccountIDFromUint64(42), nil)
	if _, err := chain.Run(context.Background(), engine).Await(
		context.Background(),
	); !errors.Is(err, ErrChainIncomplete) {
		t.Fatalf("chain Await() error = %v, want ErrChainIncomplete", err)
	}
}

func TestChainRejectsOrderOperationForAccountSourceAtRun(t *testing.T) {
	engine := newChainEngine(t, newAcceptingDriver())
	chain := Chain(
		param.NewAccountIDFromUint64(42),
		func(context.Context) (struct{}, error) { return struct{}{}, nil },
	).CheckOrder(func(context.Context, struct{}, OrderCheckResult) error {
		return nil
	})
	if _, err := chain.Run(context.Background(), engine).Await(
		context.Background(),
	); !errors.Is(err, ErrChainOrderRequired) {
		t.Fatalf("chain Await() error = %v, want ErrChainOrderRequired", err)
	}
}

func TestChainRunSnapshotsBuilderBeforeQueueExecution(t *testing.T) {
	engine := newChainEngine(t, newAcceptingDriver())
	accountID := param.NewAccountIDFromUint64(42)
	entered := make(chan struct{})
	release := make(chan struct{})
	blocking := engine.Submit(context.Background(), accountID, func() error {
		close(entered)
		<-release
		return nil
	})
	<-entered

	var calls atomic.Int64
	chain := Chain(
		accountID,
		func(context.Context) (struct{}, error) { return struct{}{}, nil },
	).Then(func(context.Context, struct{}) error {
		calls.Add(1)
		return nil
	})
	chainFuture := chain.Run(context.Background(), engine)
	chain.Then(func(context.Context, struct{}) error {
		calls.Add(100)
		return nil
	})

	close(release)
	if _, err := blocking.Await(context.Background()); err != nil {
		t.Fatalf("blocking Submit() error = %v", err)
	}
	if _, err := chainFuture.Await(context.Background()); err != nil {
		t.Fatalf("chain Await() error = %v", err)
	}
	if got := calls.Load(); got != 1 {
		t.Errorf("snapshotted chain call count = %d, want 1", got)
	}
}

func TestChainFinallyFreezesRunnerPlan(t *testing.T) {
	engine := newChainEngine(t, newAcceptingDriver())
	accountID := param.NewAccountIDFromUint64(42)
	var calls atomic.Int64
	builder := Chain(
		accountID,
		func(context.Context) (struct{}, error) { return struct{}{}, nil },
	).Then(func(context.Context, struct{}) error {
		calls.Add(1)
		return nil
	})
	runner := builder.Finally(func(
		context.Context,
		struct{},
		ChainOutcome,
	) error {
		calls.Add(10)
		return nil
	})
	builder.Then(func(context.Context, struct{}) error {
		calls.Add(100)
		return nil
	})

	for range 2 {
		if _, err := runner.Run(context.Background(), engine).Await(
			context.Background(),
		); err != nil {
			t.Fatalf("chain Await() error = %v", err)
		}
	}
	if got := calls.Load(); got != 22 {
		t.Errorf("frozen runner call count = %d, want 22", got)
	}
}

func TestChainBuilderRunRejectsDiscardedFinallyRunner(t *testing.T) {
	engine := newChainEngine(t, newAcceptingDriver())
	var calls atomic.Int64
	chain := Chain(
		param.NewAccountIDFromUint64(42),
		func(context.Context) (struct{}, error) {
			calls.Add(1)
			return struct{}{}, nil
		},
	).Then(func(context.Context, struct{}) error {
		calls.Add(10)
		return nil
	})
	chain.Finally(func(context.Context, struct{}, ChainOutcome) error {
		calls.Add(100)
		return nil
	})

	_, err := chain.Run(context.Background(), engine).Await(context.Background())
	if !errors.Is(err, ErrChainIncomplete) {
		t.Fatalf("chain Await() error = %v, want ErrChainIncomplete", err)
	}
	if !strings.Contains(err.Error(), "discarded Chain.Finally result") {
		t.Errorf("chain Await() error = %v, want discarded Finally result", err)
	}
	if got := calls.Load(); got != 0 {
		t.Errorf("chain call count = %d, want 0", got)
	}
}

func TestChainBuilderRunWithoutFinally(t *testing.T) {
	engine := newChainEngine(t, newAcceptingDriver())
	var calls atomic.Int64
	chain := Chain(
		param.NewAccountIDFromUint64(42),
		func(context.Context) (struct{}, error) {
			calls.Add(1)
			return struct{}{}, nil
		},
	).Then(func(context.Context, struct{}) error {
		calls.Add(10)
		return nil
	})

	if _, err := chain.Run(context.Background(), engine).Await(context.Background()); err != nil {
		t.Fatalf("chain Await() error = %v", err)
	}
	if got := calls.Load(); got != 11 {
		t.Errorf("chain call count = %d, want 11", got)
	}
}

func TestChainRunnerRunExecutesFinally(t *testing.T) {
	engine := newChainEngine(t, newAcceptingDriver())
	var calls atomic.Int64
	runner := Chain(
		param.NewAccountIDFromUint64(42),
		func(context.Context) (struct{}, error) {
			calls.Add(1)
			return struct{}{}, nil
		},
	).Then(func(context.Context, struct{}) error {
		calls.Add(10)
		return nil
	}).Finally(func(context.Context, struct{}, ChainOutcome) error {
		calls.Add(100)
		return nil
	})

	if _, err := runner.Run(context.Background(), engine).Await(context.Background()); err != nil {
		t.Fatalf("chain Await() error = %v", err)
	}
	if got := calls.Load(); got != 111 {
		t.Errorf("chain call count = %d, want 111", got)
	}
}

func TestChainRunRejectsNilEngine(t *testing.T) {
	for _, test := range []struct {
		name string
		run  func(*atomic.Int64) *future.Future[ChainOutcome]
	}{
		{
			name: "builder",
			run: func(calls *atomic.Int64) *future.Future[ChainOutcome] {
				chain := Chain(
					param.NewAccountIDFromUint64(42),
					func(context.Context) (struct{}, error) {
						calls.Add(1)
						return struct{}{}, nil
					},
				).Then(func(context.Context, struct{}) error {
					calls.Add(10)
					return nil
				})
				return chain.Run(context.Background(), nil)
			},
		},
		{
			name: "runner",
			run: func(calls *atomic.Int64) *future.Future[ChainOutcome] {
				runner := Chain(
					param.NewAccountIDFromUint64(42),
					func(context.Context) (struct{}, error) {
						calls.Add(1)
						return struct{}{}, nil
					},
				).Then(func(context.Context, struct{}) error {
					calls.Add(10)
					return nil
				}).Finally(func(context.Context, struct{}, ChainOutcome) error {
					calls.Add(100)
					return nil
				})
				return runner.Run(context.Background(), nil)
			},
		},
	} {
		t.Run(test.name, func(t *testing.T) {
			var calls atomic.Int64
			outcome, err := test.run(&calls).Await(context.Background())
			if !errors.Is(
				err,
				ErrChainEngineMissing,
			) {
				t.Errorf("chain Await() error = %v, want ErrChainEngineMissing", err)
			}
			if outcome.Status != ChainOutcomeFailed {
				t.Errorf("outcome status = %v, want ChainOutcomeFailed", outcome.Status)
			}
			if !errors.Is(outcome.Err, ErrChainEngineMissing) {
				t.Errorf("outcome error = %v, want ErrChainEngineMissing", outcome.Err)
			}
			if outcome.RetryUnsafe {
				t.Error("outcome RetryUnsafe = true, want false")
			}
			if got := calls.Load(); got != 0 {
				t.Errorf("chain callback count = %d, want 0", got)
			}
		})
	}
}

func TestChainRunReturnsRetrySafeOutcomeWhenSubmissionFails(t *testing.T) {
	engine := newChainEngine(t, newAcceptingDriver())
	if err := engine.StopHard(context.Background()); err != nil {
		t.Fatalf("StopHard() error = %v", err)
	}
	chain := Chain(
		param.NewAccountIDFromUint64(42),
		func(context.Context) (struct{}, error) { return struct{}{}, nil },
	).Then(func(context.Context, struct{}) error { return nil })

	outcome, err := chain.Run(context.Background(), engine).Await(context.Background())
	if !errors.Is(err, ErrStopped) {
		t.Fatalf("chain Await() error = %v, want ErrStopped", err)
	}
	if outcome.Status != ChainOutcomeFailed {
		t.Errorf("outcome status = %v, want ChainOutcomeFailed", outcome.Status)
	}
	if !errors.Is(outcome.Err, ErrStopped) {
		t.Errorf("outcome error = %v, want ErrStopped", outcome.Err)
	}
	if outcome.RetryUnsafe {
		t.Error("outcome RetryUnsafe = true, want false")
	}
}

func TestChainRejectsReportAccountMismatchAtRun(t *testing.T) {
	engine := newChainEngine(t, newAcceptingDriver())
	chain := Chain(
		param.NewAccountIDFromUint64(42),
		func(context.Context) (struct{}, error) { return struct{}{}, nil },
	).ApplyExecutionReport(ExecutionReportHooks[struct{}]{
		Report: func(context.Context, struct{}) (model.ExecutionReport, error) {
			return buildTestReport(t, 99), nil
		},
		OnSettled: func(context.Context, struct{}, pretrade.PostTradeResult) error {
			return nil
		},
	})
	outcome, err := chain.Run(context.Background(), engine).Await(context.Background())
	if !errors.Is(err, ErrChainAccountMismatch) {
		t.Fatalf("chain Await() error = %v, want ErrChainAccountMismatch", err)
	}
	if outcome.Status != ChainOutcomeFailed {
		t.Errorf("outcome status = %v, want ChainOutcomeFailed", outcome.Status)
	}
	if !errors.Is(outcome.Err, ErrChainAccountMismatch) {
		t.Errorf("outcome error = %v, want ErrChainAccountMismatch", outcome.Err)
	}
	if outcome.RetryUnsafe {
		t.Error("outcome RetryUnsafe = true, want false")
	}
}

func TestChainReentrantEngineSubmitFailsInsteadOfQueueing(t *testing.T) {
	engine := newChainEngine(t, newAcceptingDriver())
	accountID := param.NewAccountIDFromUint64(42)
	type state struct{}
	var beginContext context.Context
	chain := Chain(
		buildTestOrder(t, 42),
		func(ctx context.Context) (*state, error) {
			beginContext = ctx
			return &state{}, nil
		},
	).ExecutePreTrade(PreTradeHooks[*state]{
		OnRejected: func(context.Context, *state, []reject.Reject) error {
			return errors.New("OnRejected() called for accepted reservation")
		},
		OnReserved: func(ctx context.Context, _ *state, _ OperationResult) (Decision, error) {
			if ctx != beginContext {
				return decisionInvalid, errors.New("OnReserved received another context")
			}
			_, err := engine.Submit(ctx, accountID, func() error {
				return errors.New("reentrant task was queued")
			}).Await(context.Background())
			if !errors.Is(err, ErrReentrantLane) {
				return DecisionRollback, fmt.Errorf(
					"reentrant Submit() error = %w, want ErrReentrantLane",
					err,
				)
			}
			return DecisionCommit, nil
		},
	})
	if _, err := chain.Run(context.Background(), engine).Await(context.Background()); err != nil {
		t.Fatalf("chain Await() error = %v", err)
	}
}

func TestNestedChainContextPreservesOuterLaneMarker(t *testing.T) {
	engineA := newChainEngine(t, newAcceptingDriver())
	engineB := newChainEngine(t, newAcceptingDriver())
	accountA := param.NewAccountIDFromUint64(42)
	accountB := param.NewAccountIDFromUint64(84)
	waitCtx, cancel := context.WithTimeout(
		context.Background(), chainStopTimeout,
	)
	defer cancel()
	var reentrantTaskRan atomic.Bool
	outer := Chain(
		accountA,
		func(context.Context) (struct{}, error) { return struct{}{}, nil },
	).Then(func(outerCtx context.Context, _ struct{}) error {
		inner := Chain(
			accountB,
			func(context.Context) (struct{}, error) { return struct{}{}, nil },
		).Then(func(innerCtx context.Context, _ struct{}) error {
			future := engineA.Submit(innerCtx, accountA, func() error {
				reentrantTaskRan.Store(true)
				return nil
			})
			reentryWait, cancelReentry := context.WithTimeout(
				context.Background(), 250*time.Millisecond,
			)
			defer cancelReentry()
			_, err := future.Await(reentryWait)
			if !errors.Is(err, ErrReentrantLane) {
				return fmt.Errorf(
					"submit to outer engine error = %w, want ErrReentrantLane",
					err,
				)
			}
			return nil
		})
		_, err := inner.Run(outerCtx, engineB).Await(waitCtx)
		return err
	})
	if _, err := outer.Run(context.Background(), engineA).Await(waitCtx); err != nil {
		t.Fatalf("outer chain Await() error = %v", err)
	}
	if reentrantTaskRan.Load() {
		t.Fatal("submission to the active outer lane was queued")
	}
	for _, engine := range []*AsyncEngine{engineA, engineB} {
		var ran atomic.Bool
		if _, err := engine.Submit(
			context.Background(), accountA, func() error {
				ran.Store(true)
				return nil
			},
		).Await(waitCtx); err != nil {
			t.Fatalf("Submit() after nested chains error = %v", err)
		}
		if !ran.Load() {
			t.Fatal("engine did not run a task after nested chains")
		}
	}
}

// Property/fuzz verdict: a generated test is not warranted because activity
// and engine identity are independent predicates, and generation across this
// linked-list walk would only redraw the equivalence classes below.
// FuzzChainStateMachine installs only one active marker, so it exercises this
// walk at depth one but never the nested-stack classes covered below.
func TestChainLaneContextFindsActiveMarkerAtAnyDepth(t *testing.T) {
	target := newChainEngine(t, newAcceptingDriver())
	otherA := newChainEngine(t, newAcceptingDriver())
	otherB := newChainEngine(t, newAcceptingDriver())
	accountID := param.NewAccountIDFromUint64(42)
	newMarker := func(engine *AsyncEngine, active bool) *chainLaneActivity {
		marker := &chainLaneActivity{engine: engine}
		marker.active.Store(active)
		return marker
	}

	for _, test := range []struct {
		name        string
		stack       []*chainLaneActivity
		wantRefused bool
		wantRan     bool
	}{
		{
			name: "depth three, active target outermost",
			stack: []*chainLaneActivity{
				newMarker(target, true),
				newMarker(otherA, true),
				newMarker(otherB, true),
			},
			wantRefused: true,
			wantRan:     false,
		},
		{
			name: "inactive inner marker for target",
			stack: []*chainLaneActivity{
				newMarker(target, true),
				newMarker(target, false),
			},
			wantRefused: true,
			wantRan:     false,
		},
		{
			name: "active target in the middle",
			stack: []*chainLaneActivity{
				newMarker(otherA, false),
				newMarker(target, true),
				newMarker(otherB, true),
			},
			wantRefused: true,
			wantRan:     false,
		},
		{
			name: "inactive target at depth two under an active other",
			stack: []*chainLaneActivity{
				newMarker(target, false),
				newMarker(otherA, true),
			},
			wantRefused: false,
			wantRan:     true,
		},
		{
			name: "inactive target at depth three under mixed others",
			stack: []*chainLaneActivity{
				newMarker(target, false),
				newMarker(otherA, true),
				newMarker(otherB, false),
			},
			wantRefused: false,
			wantRan:     true,
		},
	} {
		t.Run(test.name, func(t *testing.T) {
			ctx := context.Background()
			for _, marker := range test.stack {
				ctx = withChainLaneContext(ctx, marker)
			}
			waitCtx, cancel := context.WithTimeout(
				context.Background(), chainStopTimeout,
			)
			defer cancel()
			var ran atomic.Bool
			_, err := target.Submit(ctx, accountID, func() error {
				ran.Store(true)
				return nil
			}).Await(waitCtx)
			if test.wantRefused {
				if !errors.Is(err, ErrReentrantLane) {
					t.Errorf("Submit() error = %v, want ErrReentrantLane", err)
				}
			} else if err != nil {
				t.Errorf("Submit() error = %v, want nil", err)
			}
			if got := ran.Load(); got != test.wantRan {
				t.Errorf("submitted function ran = %t, want %t", got, test.wantRan)
			}
		})
	}
}

func TestChainBeginAndFinallyRejectReentrantLane(t *testing.T) {
	for _, phase := range []string{"begin", "finally"} {
		t.Run(phase, func(t *testing.T) {
			engine := newChainEngine(t, newAcceptingDriver())
			accountID := param.NewAccountIDFromUint64(42)
			request := newAsyncRequest(
				pretrade.NewRequestFromHandle(nil), engine, accountID,
			)
			waitCtx, cancel := context.WithTimeout(
				context.Background(), chainStopTimeout,
			)
			defer cancel()
			begin := func(ctx context.Context) (struct{}, error) {
				if phase != "begin" {
					return struct{}{}, nil
				}
				_, err := engine.Submit(ctx, accountID, func() error {
					return errors.New("reentrant begin task was queued")
				}).Await(waitCtx)
				if !errors.Is(err, ErrReentrantLane) {
					return struct{}{}, fmt.Errorf(
						"Begin Submit() error = %w, want ErrReentrantLane",
						err,
					)
				}
				return struct{}{}, nil
			}
			runner := Chain(accountID, begin).Finally(func(
				ctx context.Context,
				_ struct{},
				_ ChainOutcome,
			) error {
				if phase != "finally" {
					return nil
				}
				_, err := request.Close(ctx).Await(waitCtx)
				if !errors.Is(err, ErrReentrantLane) {
					return fmt.Errorf(
						"Finally Close() error = %w, want ErrReentrantLane",
						err,
					)
				}
				return nil
			})
			if _, err := runner.Run(
				context.Background(), engine,
			).Await(waitCtx); err != nil {
				t.Fatalf("chain Await() error = %v", err)
			}
			var ran atomic.Bool
			if _, err := engine.Submit(
				context.Background(), accountID, func() error {
					ran.Store(true)
					return nil
				},
			).Await(waitCtx); err != nil {
				t.Fatalf("Submit() after %s error = %v", phase, err)
			}
			if !ran.Load() {
				t.Fatalf("engine did not run a task after %s", phase)
			}
		})
	}
}

func TestChainLifecycleCallsRejectReentrantLane(t *testing.T) {
	engine := newChainEngine(t, newAcceptingDriver())
	accountID := param.NewAccountIDFromUint64(42)
	request := newAsyncRequest(
		pretrade.NewRequestFromHandle(nil), engine, accountID,
	)
	reservation := newAsyncReservation(
		pretrade.NewReservationFromHandle(nil), engine, accountID,
	)
	operation := newAsyncDropCopyOperation(
		pretrade.NewDropCopyOperationFromHandle(nil), engine, accountID,
	)
	for _, test := range []struct {
		name string
		call func(context.Context) error
	}{
		{
			name: "request execute",
			call: func(ctx context.Context) error {
				_, _, err := request.Execute(ctx).Await(context.Background())
				return err
			},
		},
		{
			name: "reservation commit and close",
			call: func(ctx context.Context) error {
				_, err := reservation.CommitAndClose(ctx).Await(context.Background())
				return err
			},
		},
		{
			name: "reservation rollback and close",
			call: func(ctx context.Context) error {
				_, err := reservation.RollbackAndClose(ctx).Await(context.Background())
				return err
			},
		},
		{
			name: "drop-copy commit and close",
			call: func(ctx context.Context) error {
				_, err := operation.CommitAndClose(ctx).Await(context.Background())
				return err
			},
		},
		{
			name: "drop-copy rollback and close",
			call: func(ctx context.Context) error {
				_, err := operation.RollbackAndClose(ctx).Await(context.Background())
				return err
			},
		},
	} {
		t.Run(test.name, func(t *testing.T) {
			chain := Chain(
				accountID,
				func(context.Context) (struct{}, error) { return struct{}{}, nil },
			).Then(func(ctx context.Context, _ struct{}) error {
				if err := test.call(ctx); !errors.Is(err, ErrReentrantLane) {
					return fmt.Errorf(
						"lifecycle call error = %w, want ErrReentrantLane",
						err,
					)
				}
				return nil
			})
			if _, err := chain.Run(context.Background(), engine).Await(
				context.Background(),
			); err != nil {
				t.Errorf("chain Await() error = %v", err)
			}
		})
	}
}

func TestChainReentrantStopFailsWithoutStoppingEngine(t *testing.T) {
	for _, test := range []struct {
		name string
		stop func(*AsyncEngine, context.Context) error
	}{
		{
			name: "graceful",
			stop: func(engine *AsyncEngine, ctx context.Context) error {
				return engine.StopGraceful(ctx)
			},
		},
		{
			name: "hard",
			stop: func(engine *AsyncEngine, ctx context.Context) error {
				return engine.StopHard(ctx)
			},
		},
	} {
		t.Run(test.name, func(t *testing.T) {
			engine := newChainEngine(t, newAcceptingDriver())
			accountID := param.NewAccountIDFromUint64(42)
			chain := Chain(
				accountID,
				func(context.Context) (struct{}, error) { return struct{}{}, nil },
			).
				Then(func(ctx context.Context, _ struct{}) error {
					if !errors.Is(test.stop(engine, ctx), ErrReentrantLane) {
						return errors.New("reentrant stop did not return ErrReentrantLane")
					}
					return nil
				})
			if _, err := chain.Run(context.Background(), engine).Await(
				context.Background(),
			); err != nil {
				t.Fatalf("chain Await() error = %v", err)
			}
			var ran atomic.Bool
			if _, err := engine.Submit(context.Background(), accountID, func() error {
				ran.Store(true)
				return nil
			}).Await(context.Background()); err != nil {
				t.Fatalf("Submit() after rejected Stop%s() error = %v", test.name, err)
			}
			if !ran.Load() {
				t.Errorf("Submit() after rejected Stop%s() did not run", test.name)
			}
			if err := test.stop(engine, context.Background()); err != nil {
				t.Fatalf("Stop%s() error = %v", test.name, err)
			}
		})
	}
}

func TestChainHookContextIgnoresSubmissionCancellation(t *testing.T) {
	engine := newChainEngine(t, newAcceptingDriver())
	accountID := param.NewAccountIDFromUint64(42)
	entered := make(chan struct{})
	release := make(chan struct{})
	blocking := engine.Submit(context.Background(), accountID, func() error {
		close(entered)
		<-release
		return nil
	})
	<-entered

	type submissionValueKey struct{}
	const submissionValue = "retained"
	submissionCtx, cancel := context.WithCancel(
		context.WithValue(context.Background(), submissionValueKey{}, submissionValue),
	)
	defer cancel()
	var hookContext context.Context
	phase := 0
	checkHookContext := func(ctx context.Context) error {
		if ctx.Err() != nil {
			return errors.New("chain hook context was cancelled")
		}
		if got := ctx.Value(submissionValueKey{}); got != submissionValue {
			return errors.New("chain hook context lost submission value")
		}
		if hookContext == nil {
			hookContext = ctx
		} else if ctx != hookContext {
			return errors.New("chain hooks received different contexts")
		}
		phase++
		return nil
	}
	report := buildTestReport(t, 42)
	chain := Chain(
		buildTestOrder(t, 42),
		func(ctx context.Context) (struct{}, error) {
			return struct{}{}, checkHookContext(ctx)
		},
	).
		Then(func(ctx context.Context, _ struct{}) error {
			return checkHookContext(ctx)
		}).
		ExecutePreTrade(PreTradeHooks[struct{}]{
			OnRejected: func(context.Context, struct{}, []reject.Reject) error {
				return errors.New("OnRejected() called for accepted reservation")
			},
			OnReserved: func(ctx context.Context, _ struct{}, _ OperationResult) (Decision, error) {
				if err := checkHookContext(ctx); err != nil {
					return decisionInvalid, err
				}
				return DecisionCommit, nil
			},
		}).
		Then(func(ctx context.Context, _ struct{}) error {
			return checkHookContext(ctx)
		}).
		ApplyExecutionReport(ExecutionReportHooks[struct{}]{
			Report: func(ctx context.Context, _ struct{}) (model.ExecutionReport, error) {
				if err := checkHookContext(ctx); err != nil {
					return model.ExecutionReport{}, err
				}
				return report, nil
			},
			OnSettled: func(
				ctx context.Context,
				_ struct{},
				_ pretrade.PostTradeResult,
			) error {
				return checkHookContext(ctx)
			},
		}).
		Then(func(ctx context.Context, _ struct{}) error {
			return checkHookContext(ctx)
		})
	chainFuture := chain.Run(submissionCtx, engine)
	cancel()
	close(release)
	if _, err := blocking.Await(context.Background()); err != nil {
		t.Fatalf("blocking Submit() error = %v", err)
	}
	if _, err := chainFuture.Await(context.Background()); err != nil {
		t.Fatalf("chain Await() error = %v", err)
	}
	if phase != 7 {
		t.Errorf("hook phase count = %d, want 7", phase)
	}
}

// TestChainAwaitCancellationYieldsUnknownOutcomeWithoutAffectingTheChain pins
// the behavior Future.Await documents: cancelling the wait context before the
// chain's future resolves must not be mistaken for a chain verdict. A chain
// blocked in a Then hook is awaited with an already-cancelled context; the
// returned outcome must be the zero ChainOutcome - Status ChainOutcomeUnknown,
// Err nil, RetryUnsafe false - with the context's own error, while the chain
// itself keeps running untouched. Once the chain is allowed to finish,
// awaiting the same future again must return the real outcome it produced.
func TestChainAwaitCancellationYieldsUnknownOutcomeWithoutAffectingTheChain(
	t *testing.T,
) {
	engine := newChainEngine(t, newAcceptingDriver())
	entered := make(chan struct{})
	release := make(chan struct{})
	chainFuture := Chain(
		buildTestOrder(t, 42),
		func(context.Context) (struct{}, error) { return struct{}{}, nil },
	).
		Then(func(context.Context, struct{}) error {
			close(entered)
			<-release
			return nil
		}).
		Run(context.Background(), engine)
	// The hook holds the chain's lane and the engine cleanup stops that lane
	// gracefully, so every exit path below - assertion failure, panic, early
	// return - has to release it. Otherwise a regression hangs the test binary
	// instead of failing it.
	releaseChain := sync.OnceFunc(func() { close(release) })
	defer releaseChain()
	select {
	case <-entered:
	case <-time.After(5 * time.Second):
		t.Fatal("chain did not reach its blocking Then hook")
	}

	waitCtx, cancel := context.WithCancel(context.Background())
	cancel()
	// Returning at once is the contract under test, so the wait that checks it
	// cannot rely on it: an Await that stopped honouring its context would block
	// here with the hook still parked and the deferred release not yet reached.
	var outcome ChainOutcome
	var err error
	cancelled := make(chan struct{})
	go func() {
		defer close(cancelled)
		outcome, err = chainFuture.Await(waitCtx)
	}()
	select {
	case <-cancelled:
	case <-time.After(5 * time.Second):
		t.Fatal("Await() with a cancelled wait context did not return")
	}
	if !errors.Is(err, context.Canceled) {
		t.Fatalf(
			"Await() with a cancelled wait context error = %v, want context.Canceled",
			err,
		)
	}
	if outcome.Status != ChainOutcomeUnknown {
		t.Errorf(
			"outcome status = %v, want ChainOutcomeUnknown", outcome.Status,
		)
	}
	if outcome.Err != nil {
		t.Errorf("outcome error = %v, want nil", outcome.Err)
	}
	if outcome.RetryUnsafe {
		t.Error("outcome RetryUnsafe = true, want false")
	}

	// Releasing here, and not only through the deferred call, keeps the path
	// this test exists for: the chain finishes and a second Await returns the
	// outcome it produced.
	releaseChain()
	var finalOutcome ChainOutcome
	var finalErr error
	resolved := make(chan struct{})
	go func() {
		defer close(resolved)
		finalOutcome, finalErr = chainFuture.Await(context.Background())
	}()
	select {
	case <-resolved:
	case <-time.After(5 * time.Second):
		t.Fatal("chain did not resolve after its hook was released")
	}
	if finalErr != nil {
		t.Fatalf("chain Await() error = %v", finalErr)
	}
	if finalOutcome.Status != ChainOutcomeCompleted {
		t.Errorf(
			"outcome status = %v, want ChainOutcomeCompleted", finalOutcome.Status,
		)
	}
	if finalOutcome.Err != nil {
		t.Errorf("outcome error = %v, want nil", finalOutcome.Err)
	}
}

func TestChainLaneContextDeactivatesAfterCompletion(t *testing.T) {
	engine := newChainEngine(t, newAcceptingDriver())
	accountID := param.NewAccountIDFromUint64(42)
	request := newAsyncRequest(
		pretrade.NewRequestFromHandle(nil), engine, accountID,
	)
	var hookContext context.Context
	chain := Chain(
		accountID,
		func(ctx context.Context) (struct{}, error) {
			hookContext = ctx
			return struct{}{}, nil
		},
	).
		Then(func(ctx context.Context, _ struct{}) error {
			if _, err := request.Close(ctx).Await(context.Background()); !errors.Is(
				err, ErrReentrantLane,
			) {
				return errors.New("chain hook Close did not return ErrReentrantLane")
			}
			return nil
		})
	if _, err := chain.Run(context.Background(), engine).Await(
		context.Background(),
	); err != nil {
		t.Fatalf("chain Await() error = %v", err)
	}
	if hookContext == nil {
		t.Fatal("chain did not provide a hook context")
	}
	if _, err := engine.Submit(hookContext, accountID, func() error {
		return nil
	}).Await(context.Background()); err != nil {
		t.Fatalf("Submit() with expired lane context error = %v", err)
	}
	if _, err := request.Close(hookContext).Await(context.Background()); err != nil {
		t.Fatalf("Close() with expired lane context error = %v", err)
	}
}

func TestChainKeepsCompetingTaskOutsideItsStages(t *testing.T) {
	engine := newChainEngine(t, newAcceptingDriver())
	accountID := param.NewAccountIDFromUint64(42)
	entered := make(chan struct{})
	release := make(chan struct{})
	var phase int64
	chain := Chain(
		buildTestOrder(t, 42),
		func(context.Context) (struct{}, error) { return struct{}{}, nil },
	).ExecutePreTrade(PreTradeHooks[struct{}]{
		OnRejected: func(context.Context, struct{}, []reject.Reject) error {
			return errors.New("OnRejected() called for accepted reservation")
		},
		OnReserved: func(context.Context, struct{}, OperationResult) (Decision, error) {
			if !atomic.CompareAndSwapInt64(&phase, 0, 1) {
				return decisionInvalid, errors.New("chain started out of order")
			}
			close(entered)
			<-release
			if !atomic.CompareAndSwapInt64(&phase, 1, 2) {
				return decisionInvalid, errors.New("competing task interleaved")
			}
			return DecisionCommit, nil
		},
	}).Then(func(context.Context, struct{}) error {
		if !atomic.CompareAndSwapInt64(&phase, 2, 3) {
			return errors.New("competing task interleaved between chain steps")
		}
		return nil
	})
	chainFuture := chain.Run(context.Background(), engine)
	<-entered
	competingFuture := engine.Submit(context.Background(), accountID, func() error {
		if !atomic.CompareAndSwapInt64(&phase, 3, 4) {
			return errors.New("competing task ran before chain completion")
		}
		return nil
	})
	if competingFuture.Done() {
		t.Fatal("competing task completed while the chain hook was blocked")
	}
	close(release)
	if _, err := chainFuture.Await(context.Background()); err != nil {
		t.Fatalf("chain Await() error = %v", err)
	}
	if _, err := competingFuture.Await(context.Background()); err != nil {
		t.Fatalf("competing task Await() error = %v", err)
	}
	if got := atomic.LoadInt64(&phase); got != 4 {
		t.Errorf("phase = %d, want 4", got)
	}
}
