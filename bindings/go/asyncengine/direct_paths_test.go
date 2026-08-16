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
	"runtime"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"go.openpit.dev/openpit/accountadjustment"
	"go.openpit.dev/openpit/accounts"
	"go.openpit.dev/openpit/configure"
	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pretrade"
	"go.openpit.dev/openpit/reject"
)

// acceptingDriver is a fake driver that returns live handles and nil rejects.
type acceptingDriver struct {
	mu           sync.Mutex
	startCount   int64
	executeCount int64
	reportCount  int64
	adjustCount  int64
	startHook    func()
	rejectStart  bool

	concurrentByAccount map[uint64]int64
	maxConcurrent       map[uint64]int64
}

type acceptingDriverEngineResult struct {
	handle native.Engine
	err    error
}

// The fixture engine intentionally lives for the test process. Accepted
// handles outlive individual acceptingDriver values and remain caller-owned.
var acceptingDriverEngine = sync.OnceValue(func() acceptingDriverEngineResult {
	builder, err := native.CreateEngineBuilder(native.SyncPolicyAccount)
	if err != nil {
		return acceptingDriverEngineResult{err: err}
	}
	if err := native.EngineBuilderAddBuiltinOrderValidation(
		builder, native.DefaultPolicyGroupID,
	); err != nil {
		native.DestroyEngineBuilder(builder)
		return acceptingDriverEngineResult{err: err}
	}
	engine, buildErr, err := native.EngineBuilderBuild(builder)
	native.DestroyEngineBuilder(builder)
	if buildErr != nil {
		native.DestroyEngineBuildError(buildErr)
		return acceptingDriverEngineResult{
			err: errors.New("accepting driver engine returned a build error"),
		}
	}
	return acceptingDriverEngineResult{handle: engine, err: err}
})

func newAcceptingDriver() *acceptingDriver {
	return &acceptingDriver{
		concurrentByAccount: map[uint64]int64{},
		maxConcurrent:       map[uint64]int64{},
	}
}

func (d *acceptingDriver) recordStart(accountID param.AccountID) func() {
	id := uint64(accountID.Handle())
	d.mu.Lock()
	d.concurrentByAccount[id]++
	current := d.concurrentByAccount[id]
	if current > d.maxConcurrent[id] {
		d.maxConcurrent[id] = current
	}
	d.mu.Unlock()
	return func() {
		d.mu.Lock()
		d.concurrentByAccount[id]--
		d.mu.Unlock()
	}
}

func (d *acceptingDriver) StartPreTrade(
	order model.Order,
) (*pretrade.Request, []reject.Reject, error) {
	op, _ := order.Operation().Get()
	accountID, _ := op.AccountID().Get()
	done := d.recordStart(accountID)
	defer done()
	if d.startHook != nil {
		d.startHook()
	}
	atomic.AddInt64(&d.startCount, 1)
	if d.rejectStart {
		return nil, []reject.Reject{{}}, nil
	}
	engine := acceptingDriverEngine()
	if engine.err != nil {
		return nil, nil, engine.err
	}
	handle, rejects, err := native.EngineStartPreTrade(engine.handle, order.Handle())
	runtime.KeepAlive(order)
	if err != nil {
		return nil, nil, err
	}
	if rejects != nil {
		native.DestroyPretradeRejectList(rejects)
		return nil, []reject.Reject{{}}, nil
	}
	return pretrade.NewRequestFromHandle(handle), nil, nil
}

func (d *acceptingDriver) ExecutePreTrade(
	order model.Order,
) (*pretrade.Reservation, []reject.Reject, error) {
	op, _ := order.Operation().Get()
	accountID, _ := op.AccountID().Get()
	done := d.recordStart(accountID)
	defer done()
	atomic.AddInt64(&d.executeCount, 1)
	engine := acceptingDriverEngine()
	if engine.err != nil {
		return nil, nil, engine.err
	}
	handle, rejects, err := native.EngineExecutePreTrade(engine.handle, order.Handle())
	runtime.KeepAlive(order)
	if err != nil {
		return nil, nil, err
	}
	if rejects != nil {
		native.DestroyPretradeRejectList(rejects)
		return nil, []reject.Reject{{}}, nil
	}
	return pretrade.NewReservationFromHandle(handle), nil, nil
}

func (*acceptingDriver) ExecutePreTradeDryRun(
	model.Order,
) (*pretrade.DryRunReport, error) {
	return pretrade.NewDryRunReportFromHandle(nil), nil
}

func (d *acceptingDriver) ApplyDropCopy(
	order model.Order,
) (*pretrade.DropCopyOperation, []reject.Reject, error) {
	op, _ := order.Operation().Get()
	accountID, _ := op.AccountID().Get()
	done := d.recordStart(accountID)
	defer done()
	atomic.AddInt64(&d.executeCount, 1)
	engine := acceptingDriverEngine()
	if engine.err != nil {
		return nil, nil, engine.err
	}
	handle, rejects, err := native.EngineApplyDropCopy(engine.handle, order.Handle())
	runtime.KeepAlive(order)
	if err != nil {
		return nil, nil, err
	}
	if rejects != nil {
		native.DestroyPretradeRejectList(rejects)
		return nil, []reject.Reject{{}}, nil
	}
	return pretrade.NewDropCopyOperationFromHandle(handle), nil, nil
}

func (d *acceptingDriver) ApplyExecutionReport(
	report model.ExecutionReport,
) (pretrade.PostTradeResult, error) {
	op, _ := report.Operation().Get()
	accountID, _ := op.AccountID().Get()
	done := d.recordStart(accountID)
	defer done()
	atomic.AddInt64(&d.reportCount, 1)
	return pretrade.PostTradeResult{}, nil
}

func (d *acceptingDriver) ApplyAccountAdjustment(
	accountID param.AccountID,
	_ []model.AccountAdjustment,
) (accountadjustment.BatchResult, error) {
	done := d.recordStart(accountID)
	defer done()
	atomic.AddInt64(&d.adjustCount, 1)
	return accountadjustment.BatchResult{}, nil
}

func (*acceptingDriver) Accounts() accounts.Accounts {
	return accounts.Accounts{}
}

func (*acceptingDriver) Configure() configure.Configurator {
	return configure.Configurator{}
}

// TestAsyncEngineExecutePreTradeHappyPath tests that ExecutePreTrade returns a
// live *AsyncReservation when the driver succeeds, and that executeCount
// increments.
func TestAsyncEngineExecutePreTradeHappyPath(t *testing.T) {
	t.Parallel()
	driver := newAcceptingDriver()
	async, err := NewBuilder(driver).Dynamic().Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer func() {
		if err := async.StopGraceful(context.Background()); err != nil {
			t.Fatalf("StopGraceful() error = %v", err)
		}
	}()

	f := async.ExecutePreTrade(context.Background(), buildTestOrder(t, 42))
	res, rejects, err := f.Await(context.Background())
	if err != nil {
		t.Fatalf("Await() err = %v", err)
	}
	if res == nil {
		t.Fatalf("reservation = nil, want non-nil on accept")
	}
	if rejects != nil {
		t.Errorf("rejects = %v, want nil on accept", rejects)
	}
	if res.AccountID() != param.NewAccountIDFromUint64(42) {
		t.Errorf("reservation.AccountID() = %v, want %v",
			res.AccountID(), param.NewAccountIDFromUint64(42))
	}
	if got := atomic.LoadInt64(&driver.executeCount); got != 1 {
		t.Errorf("executeCount = %d, want 1", got)
	}
	if _, err := res.Close(context.Background()).Await(context.Background()); err != nil {
		t.Fatalf("Close() error = %v", err)
	}
}

func TestAsyncEngineApplyDropCopyHappyPath(t *testing.T) {
	t.Parallel()
	driver := newAcceptingDriver()
	async, err := NewBuilder(driver).Dynamic().Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer func() {
		if err := async.StopGraceful(context.Background()); err != nil {
			t.Fatalf("StopGraceful() error = %v", err)
		}
	}()

	f := async.ApplyDropCopy(
		context.Background(), buildTestOrder(t, 42),
	)
	operation, rejects, err := f.Await(context.Background())
	if err != nil {
		t.Fatalf("Await() err = %v", err)
	}
	if operation == nil {
		t.Fatal("operation = nil, want non-nil")
	}
	if rejects != nil {
		t.Errorf("rejects = %v, want nil on accept", rejects)
	}
	if operation.AccountID() != param.NewAccountIDFromUint64(42) {
		t.Errorf("operation.AccountID() = %v, want %v",
			operation.AccountID(), param.NewAccountIDFromUint64(42))
	}
	if _, err := operation.Close(context.Background()).Await(context.Background()); err != nil {
		t.Fatalf("Close() error = %v", err)
	}
}

// TestAsyncDropCopyFinalizersRunOnTheAccountWorker walks every queued finalizer
// through a real worker: each future must resolve without error, which is the
// path a hard-stop abort or a refused submit never reaches.
func TestAsyncDropCopyFinalizersRunOnTheAccountWorker(t *testing.T) {
	t.Parallel()
	async, err := NewBuilder(newAcceptingDriver()).Dynamic().Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer func() {
		if err := async.StopGraceful(context.Background()); err != nil {
			t.Fatalf("StopGraceful() error = %v", err)
		}
	}()

	ctx := context.Background()
	finalizers := []struct {
		finalize func(*AsyncDropCopyOperation) error
		name     string
	}{
		{name: "Commit", finalize: func(o *AsyncDropCopyOperation) error {
			_, err := o.Commit(ctx).Await(ctx)
			return err
		}},
		{name: "CommitAndClose", finalize: func(o *AsyncDropCopyOperation) error {
			_, err := o.CommitAndClose(ctx).Await(ctx)
			return err
		}},
		{name: "Rollback", finalize: func(o *AsyncDropCopyOperation) error {
			_, err := o.Rollback(ctx).Await(ctx)
			return err
		}},
		{name: "RollbackAndClose", finalize: func(o *AsyncDropCopyOperation) error {
			_, err := o.RollbackAndClose(ctx).Await(ctx)
			return err
		}},
		{name: "Close", finalize: func(o *AsyncDropCopyOperation) error {
			_, err := o.Close(ctx).Await(ctx)
			return err
		}},
	}
	for _, finalizer := range finalizers {
		operation, _, err := async.ApplyDropCopy(ctx, buildTestOrder(t, 42)).Await(ctx)
		if err != nil {
			t.Fatalf("%s: ApplyDropCopy Await() error = %v", finalizer.name, err)
		}
		if err := finalizer.finalize(operation); err != nil {
			t.Fatalf("%s: Await() error = %v", finalizer.name, err)
		}
		// Close after any finalizer is a no-op or the release itself.
		if _, err := operation.Close(ctx).Await(ctx); err != nil {
			t.Fatalf("%s: Close() error = %v", finalizer.name, err)
		}
	}
}

func TestAsyncEngineApplyDropCopyWaitsForSameAccountLane(t *testing.T) {
	t.Parallel()
	driver := newAcceptingDriver()
	driver.rejectStart = true
	gate := make(chan struct{})
	started := make(chan struct{})
	var once sync.Once
	driver.startHook = func() {
		once.Do(func() { close(started) })
		<-gate
	}
	async, err := NewBuilder(driver).Dynamic().Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer func() {
		if err := async.StopGraceful(context.Background()); err != nil {
			t.Fatalf("StopGraceful() error = %v", err)
		}
	}()

	account := uint64(42)
	first := async.StartPreTrade(context.Background(), buildTestOrder(t, account))
	<-started
	dropCopy := async.ApplyDropCopy(context.Background(), buildTestOrder(t, account))

	deadline := time.Now().Add(100 * time.Millisecond)
	for !dropCopy.Done() && time.Now().Before(deadline) {
		time.Sleep(time.Millisecond)
	}
	if dropCopy.Done() {
		t.Fatal("drop-copy completed before the same account lane was released")
	}

	close(gate)
	request, rejects, err := first.Await(context.Background())
	if err != nil {
		t.Fatalf("start Await() error = %v", err)
	}
	if request != nil {
		t.Fatal("start request != nil, want rejected start")
	}
	if len(rejects) == 0 {
		t.Fatal("start rejects are empty, want non-empty rejects")
	}
	operation, _, err := dropCopy.Await(context.Background())
	if err != nil {
		t.Fatalf("drop-copy Await() error = %v", err)
	}
	if operation == nil {
		t.Fatal("drop-copy operation = nil, want non-nil")
	}
	if _, err := operation.Close(context.Background()).Await(context.Background()); err != nil {
		t.Fatalf("Close() error = %v", err)
	}
}

// TestAsyncDropCopyFinalizerAbortsOnHardStop pins the abort branch of a queued
// finalizer: a task that never reaches its worker resolves with ErrStopped.
func TestAsyncDropCopyFinalizerAbortsOnHardStop(t *testing.T) {
	t.Parallel()
	async, err := NewBuilder(newAcceptingDriver()).
		WithQueueCapacity(8).
		Sharded(1).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	accountID := param.NewAccountIDFromUint64(42)
	started := make(chan struct{})
	release := make(chan struct{})
	first := async.Submit(context.Background(), accountID, func() error {
		close(started)
		<-release
		return nil
	})
	<-started

	operation := newAsyncDropCopyOperation(
		pretrade.NewDropCopyOperationFromHandle(nil), async, accountID,
	)
	closeFuture := operation.RollbackAndClose(context.Background())
	hardStopDone := make(chan error, 1)
	go func() {
		hardStopDone <- async.StopHard(context.Background())
	}()
	hardStopDeadline := time.Now().Add(5 * time.Second)
	for !isStrategyHardStopped(async.strategy) {
		if time.Now().After(hardStopDeadline) {
			t.Fatal("timeout waiting for hard-stopped strategy")
		}
		time.Sleep(time.Microsecond)
	}
	close(release)
	if err := <-hardStopDone; err != nil {
		t.Fatalf("StopHard() error = %v", err)
	}
	if _, err := first.Await(context.Background()); err != nil {
		t.Fatalf("first Submit() error = %v", err)
	}
	if _, err := closeFuture.Await(context.Background()); !errors.Is(
		err, ErrStopped,
	) {
		t.Fatalf("RollbackAndClose() error = %v, want ErrStopped", err)
	}
}

// Drop copy requires a readable account ID: the core rejects an unreadable one
// with MissingRequiredField before any policy runs, so the facade refuses the
// order up front rather than queueing a guaranteed failure.
func TestAsyncEngineApplyDropCopyFailsFastWithoutAccount(t *testing.T) {
	t.Parallel()
	driver := newAcceptingDriver()
	async, err := NewBuilder(driver).Sharded(1).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer func() {
		if err := async.StopGraceful(context.Background()); err != nil {
			t.Fatalf("StopGraceful() error = %v", err)
		}
	}()

	operation, rejects, err := async.ApplyDropCopy(
		context.Background(), model.NewOrder(),
	).Await(context.Background())
	if !errors.Is(err, ErrMissingAccountID) {
		t.Fatalf("Await() err = %v, want ErrMissingAccountID", err)
	}
	if operation != nil {
		t.Fatalf("operation = %v, want nil", operation)
	}
	if rejects != nil {
		t.Fatalf("rejects = %v, want nil", rejects)
	}
}

func TestAsyncReservationFinalizerSerializesSameAccountLane(t *testing.T) {
	callbackEntered := make(chan struct{})
	callbackRelease := make(chan struct{})
	releaseCallback := sync.OnceFunc(func() { close(callbackRelease) })
	defer releaseCallback()
	probe := &chainMutationProbe{
		onRollback: func() {
			close(callbackEntered)
			<-callbackRelease
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
	waitCtx, cancel := context.WithTimeout(context.Background(), chainStopTimeout)
	defer cancel()
	reservation, rejects, err := engine.ExecutePreTrade(
		context.Background(), order,
	).Await(waitCtx)
	if err != nil {
		t.Fatalf("ExecutePreTrade() error = %v", err)
	}
	if len(rejects) != 0 {
		t.Fatalf("ExecutePreTrade() rejects = %v, want none", rejects)
	}
	if reservation == nil {
		t.Fatal("ExecutePreTrade() reservation = nil, want non-nil")
	}

	finalizer := reservation.RollbackAndClose(context.Background())
	select {
	case <-callbackEntered:
	case <-waitCtx.Done():
		t.Fatal("reservation rollback callback did not start")
	}
	competingRan := make(chan struct{})
	competing := engine.Submit(
		context.Background(),
		param.NewAccountIDFromUint64(chainOrderAccountID),
		func() error {
			close(competingRan)
			return nil
		},
	)
	if competing.Done() {
		t.Fatal("same-account task completed while finalizer callback was blocked")
	}
	select {
	case <-competingRan:
		t.Fatal("same-account task ran while finalizer callback was blocked")
	default:
	}

	releaseCallback()
	if _, err := finalizer.Await(waitCtx); err != nil {
		t.Fatalf("RollbackAndClose() error = %v", err)
	}
	if _, err := competing.Await(waitCtx); err != nil {
		t.Fatalf("same-account task error = %v", err)
	}
	if _, err := reservation.Close(context.Background()).Await(waitCtx); err != nil {
		t.Fatalf("Close() after RollbackAndClose error = %v", err)
	}
}

// TestAsyncEngineApplyAccountAdjustmentHappyPath tests the happy path of
// ApplyAccountAdjustment, asserting the batch-result future and that
// adjustmentCount is incremented.
func TestAsyncEngineApplyAccountAdjustmentHappyPath(t *testing.T) {
	t.Parallel()
	driver := newFakeDriver()
	async, err := NewBuilder(driver).Dynamic().Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer func() {
		if err := async.StopGraceful(context.Background()); err != nil {
			t.Fatalf("StopGraceful() error = %v", err)
		}
	}()

	accountID := param.NewAccountIDFromUint64(7)
	f := async.ApplyAccountAdjustment(context.Background(), accountID, nil)
	result, err := f.Await(context.Background())
	if err != nil {
		t.Fatalf("Await() err = %v", err)
	}
	if result.BatchError.IsSet() {
		t.Errorf("BatchError.IsSet() = true, want false on accept")
	}
	if result.Outcomes != nil {
		t.Errorf("Outcomes = %v, want nil (fake driver returns nil)", result.Outcomes)
	}
	if got := atomic.LoadInt64(&driver.adjustmentCount); got != 1 {
		t.Errorf("adjustmentCount = %d, want 1", got)
	}
}

// TestAsyncEngineApplyAccountAdjustmentAbortPath tests that a hard-stopped
// engine aborts a pending adjustment task with ErrStopped. This exercises the
// abort path: the future should resolve with an empty batch result and
// ErrStopped.
func TestAsyncEngineApplyAccountAdjustmentAbortPath(t *testing.T) {
	t.Parallel()
	driver := newFakeDriver()
	gate := make(chan struct{})
	started := make(chan struct{}, 1)
	driver.startHook = func() {
		select {
		case started <- struct{}{}:
		default:
		}
		<-gate
	}

	async, err := NewBuilder(driver).
		WithQueueCapacity(128).
		Sharded(1).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}

	// Block the worker with a first task.
	blockingOrder := async.StartPreTrade(context.Background(), buildTestOrder(t, 1))
	<-started

	// Queue an adjustment for the same account (same shard).
	accountID := param.NewAccountIDFromUint64(1)
	fAdj := async.ApplyAccountAdjustment(context.Background(), accountID, nil)

	// Trigger hard stop; wait for it to set the hardStop signal.
	hardStopDone := make(chan error, 1)
	go func() {
		hardStopDone <- async.StopHard(context.Background())
	}()
	hardStopDeadline := time.Now().Add(5 * time.Second)
	for !isStrategyHardStopped(async.strategy) {
		if time.Now().After(hardStopDeadline) {
			t.Fatal("timeout waiting for strategy to reach hard-stopped state")
		}
		<-tAfter1us()
	}
	close(gate)
	if err := <-hardStopDone; err != nil {
		t.Fatalf("StopHard() error = %v", err)
	}
	if _, _, err := blockingOrder.Await(context.Background()); err != nil {
		t.Fatalf("blockingOrder Await error = %v", err)
	}

	result, err := fAdj.Await(context.Background())
	if !errors.Is(err, ErrStopped) {
		t.Fatalf("fAdj Await err = %v, want ErrStopped", err)
	}
	if result.BatchError.IsSet() {
		t.Errorf("BatchError.IsSet() = true, want false on abort")
	}
	if result.Outcomes != nil {
		t.Errorf("Outcomes = %v, want nil on abort", result.Outcomes)
	}

	// adjustmentCount must be 0 because the task was aborted, not run.
	if got := atomic.LoadInt64(&driver.adjustmentCount); got != 0 {
		t.Errorf("adjustmentCount = %d, want 0 on abort path", got)
	}
}

// tAfter1us is a tiny helper to yield without a literal sleep in the
// hard-stop spin loop above.
func tAfter1us() <-chan struct{} {
	ch := make(chan struct{})
	go func() { close(ch) }()
	return ch
}

// TestAsyncEngineApplyExecutionReportHappyPath asserts that
// ApplyExecutionReport runs through the worker and increments reportCount.
func TestAsyncEngineApplyExecutionReportHappyPath(t *testing.T) {
	t.Parallel()
	driver := newFakeDriver()
	async, err := NewBuilder(driver).Dynamic().Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer func() {
		if err := async.StopGraceful(context.Background()); err != nil {
			t.Fatalf("StopGraceful() error = %v", err)
		}
	}()

	report := buildTestReport(t, 55)
	f := async.ApplyExecutionReport(context.Background(), report)
	_, err = f.Await(context.Background())
	if err != nil {
		t.Fatalf("Await() err = %v", err)
	}
	if got := atomic.LoadInt64(&driver.reportCount); got != 1 {
		t.Errorf("reportCount = %d, want 1", got)
	}
}

// TestAsyncEngineInstrumentationCountersAllDriverMethods verifies that all
// three previously-dead counters (executeCount/reportCount/adjustmentCount)
// are incremented correctly when the corresponding driver methods are invoked.
func TestAsyncEngineInstrumentationCountersAllDriverMethods(t *testing.T) {
	t.Parallel()
	driver := newFakeDriver()
	async, err := NewBuilder(driver).Dynamic().MaxQueues(0).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer func() {
		if err := async.StopGraceful(context.Background()); err != nil {
			t.Fatalf("StopGraceful() error = %v", err)
		}
	}()

	const account = uint64(99)
	accountID := param.NewAccountIDFromUint64(account)

	// Execute pre-trade (executeCount).
	fExec := async.ExecutePreTrade(context.Background(), buildTestOrder(t, account))
	if _, _, err := fExec.Await(context.Background()); err != nil {
		t.Fatalf("ExecutePreTrade Await error = %v", err)
	}

	// Apply execution report (reportCount).
	fReport := async.ApplyExecutionReport(
		context.Background(), buildTestReport(t, account),
	)
	if _, err := fReport.Await(context.Background()); err != nil {
		t.Fatalf("ApplyExecutionReport Await error = %v", err)
	}

	// Apply account adjustment (adjustmentCount).
	fAdj := async.ApplyAccountAdjustment(context.Background(), accountID, nil)
	if _, err := fAdj.Await(context.Background()); err != nil {
		t.Fatalf("ApplyAccountAdjustment Await error = %v", err)
	}

	if got := atomic.LoadInt64(&driver.executeCount); got != 1 {
		t.Errorf("executeCount = %d, want 1", got)
	}
	if got := atomic.LoadInt64(&driver.reportCount); got != 1 {
		t.Errorf("reportCount = %d, want 1", got)
	}
	if got := atomic.LoadInt64(&driver.adjustmentCount); got != 1 {
		t.Errorf("adjustmentCount = %d, want 1", got)
	}
}

type malformedLifetimeDriver struct {
	*acceptingDriver
	request     *pretrade.Request
	reservation *pretrade.Reservation
	operation   *pretrade.DropCopyOperation
	rejects     []reject.Reject
	err         error
}

func (d *malformedLifetimeDriver) StartPreTrade(
	model.Order,
) (*pretrade.Request, []reject.Reject, error) {
	return d.request, d.rejects, d.err
}

func (d *malformedLifetimeDriver) ExecutePreTrade(
	model.Order,
) (*pretrade.Reservation, []reject.Reject, error) {
	return d.reservation, d.rejects, d.err
}

func (d *malformedLifetimeDriver) ApplyDropCopy(
	model.Order,
) (*pretrade.DropCopyOperation, []reject.Reject, error) {
	return d.operation, d.rejects, d.err
}

func newDirectPathRequest(
	t *testing.T,
	engine native.Engine,
	order model.Order,
) *pretrade.Request {
	t.Helper()
	handle, rejects, err := native.EngineStartPreTrade(engine, order.Handle())
	runtime.KeepAlive(order)
	if err != nil {
		t.Fatalf("EngineStartPreTrade() error = %v", err)
	}
	if rejects != nil {
		native.DestroyPretradeRejectList(rejects)
		t.Fatal("EngineStartPreTrade() rejected a valid order")
	}
	request := pretrade.NewRequestFromHandle(handle)
	t.Cleanup(request.Close)
	return request
}

func TestAsyncEngineRejectsInvalidLifetimeResults(t *testing.T) {
	driverErr := errors.New("driver returned an unusable handle and error")
	for _, path := range []string{"start", "execute", "drop copy"} {
		for _, invalid := range []struct {
			name    string
			state   string
			rejects []reject.Reject
			err     error
		}{
			{name: "missing handle"},
			{name: "empty rejects", rejects: []reject.Reject{}},
			{name: "nil native handle", state: "nil native handle"},
			{
				name: "nil native handle with rejects", state: "nil native handle",
				rejects: []reject.Reject{{}},
			},
			{
				name: "nil native handle with error", state: "nil native handle",
				err: driverErr,
			},
			{name: "closed native handle", state: "closed native handle"},
			{
				name: "closed native handle with rejects", state: "closed native handle",
				rejects: []reject.Reject{{}},
			},
			{
				name: "closed native handle with error", state: "closed native handle",
				err: driverErr,
			},
		} {
			t.Run(path+" with "+invalid.name, func(t *testing.T) {
				driver := &malformedLifetimeDriver{
					acceptingDriver: newAcceptingDriver(),
					rejects:         invalid.rejects,
					err:             invalid.err,
				}
				order := buildChainCheckOrder(t)
				var assertClosed func()
				switch invalid.state {
				case "nil native handle":
					switch path {
					case "start":
						driver.request = pretrade.NewRequestFromHandle(nil)
						assertClosed = func() {
							if !driver.request.IsClosed() {
								t.Error("request with nil native handle remains open")
							}
						}
					case "execute":
						driver.reservation = pretrade.NewReservationFromHandle(nil)
						assertClosed = func() {
							if !driver.reservation.IsClosed() {
								t.Error("reservation with nil native handle remains open")
							}
						}
					case "drop copy":
						driver.operation = pretrade.NewDropCopyOperationFromHandle(nil)
						assertClosed = func() {
							if !driver.operation.IsClosed() {
								t.Error("drop-copy operation with nil native handle remains open")
							}
						}
					}
				case "closed native handle":
					switch path {
					case "start":
						driver.request = newDirectPathRequest(
							t, newChainNativeEngine(t), order,
						)
						driver.request.Close()
						assertClosed = func() {
							if !driver.request.IsClosed() {
								t.Error("request remains open")
							}
						}
					case "execute":
						driver.reservation = newChainReservation(t, order)
						driver.reservation.Close()
						assertClosed = func() {
							if !driver.reservation.IsClosed() {
								t.Error("reservation remains open")
							}
						}
					case "drop copy":
						driver.operation = newChainDropCopyOperation(t, order)
						driver.operation.Close()
						assertClosed = func() {
							if !driver.operation.IsClosed() {
								t.Error("drop-copy operation remains open")
							}
						}
					}
				}
				engine := newChainEngine(t, driver)
				var resultPresent bool
				var rejects []reject.Reject
				var err error
				switch path {
				case "start":
					var request *AsyncRequest
					request, rejects, err = engine.StartPreTrade(
						context.Background(), order,
					).Await(context.Background())
					resultPresent = request != nil
				case "execute":
					var reservation *AsyncReservation
					reservation, rejects, err = engine.ExecutePreTrade(
						context.Background(), order,
					).Await(context.Background())
					resultPresent = reservation != nil
				case "drop copy":
					var operation *AsyncDropCopyOperation
					operation, rejects, err = engine.ApplyDropCopy(
						context.Background(), order,
					).Await(context.Background())
					resultPresent = operation != nil
				}
				if !errors.Is(err, ErrDriverResult) {
					t.Errorf("%s error = %v, want ErrDriverResult", path, err)
				}
				if invalid.err != nil && !errors.Is(err, invalid.err) {
					t.Errorf("%s error = %v, want driver error", path, err)
				}
				if resultPresent {
					t.Errorf("%s result is non-nil, want nil", path)
				}
				if rejects != nil {
					t.Errorf("%s rejects = %v, want nil", path, rejects)
				}
				if assertClosed != nil {
					assertClosed()
				}
			})
		}
	}
}

func TestAsyncEngineClosesContradictoryDriverHandles(t *testing.T) {
	driverErr := errors.New("driver returned a handle and an error")
	for _, path := range []string{"start", "execute", "drop copy"} {
		for _, conflict := range []string{"rejects", "error"} {
			t.Run(path+" with "+conflict, func(t *testing.T) {
				order := buildChainCheckOrder(t)
				driver := &malformedLifetimeDriver{
					acceptingDriver: newAcceptingDriver(),
				}
				if conflict == "rejects" {
					driver.rejects = []reject.Reject{{}}
				} else {
					driver.err = driverErr
				}
				var assertClosed func()
				switch path {
				case "start":
					driver.request = newDirectPathRequest(
						t, newChainNativeEngine(t), order,
					)
					assertClosed = func() {
						_, _, err := driver.request.Execute()
						if !errors.Is(err, pretrade.ErrRequestClosed) {
							t.Errorf(
								"request Execute() after invalid result error = %v, "+
									"want ErrRequestClosed",
								err,
							)
						}
					}
				case "execute":
					driver.reservation = newChainReservation(t, order)
					assertClosed = func() {
						assertChainClosedReservation(t, driver.reservation)
					}
				case "drop copy":
					driver.operation = newChainDropCopyOperation(t, order)
					assertClosed = func() {
						assertChainClosedDropCopyOperation(t, driver.operation)
					}
				}
				engine := newChainEngine(t, driver)
				var err error
				switch path {
				case "start":
					_, _, err = engine.StartPreTrade(
						context.Background(), order,
					).Await(context.Background())
				case "execute":
					_, _, err = engine.ExecutePreTrade(
						context.Background(), order,
					).Await(context.Background())
				case "drop copy":
					_, _, err = engine.ApplyDropCopy(
						context.Background(), order,
					).Await(context.Background())
				}
				if !errors.Is(err, ErrDriverResult) {
					t.Errorf("%s error = %v, want ErrDriverResult", path, err)
				}
				if conflict == "error" && !errors.Is(err, driverErr) {
					t.Errorf("%s error = %v, want driver error", path, err)
				}
				assertClosed()
			})
		}
	}
}
