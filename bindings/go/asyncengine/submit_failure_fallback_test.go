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

// The tests in this file exercise the submit-failure fallback only: the path
// where strategy.submit refuses a task and AsyncRequest, AsyncReservation, or
// AsyncDropCopyOperation still release the underlying handle in account-lane
// order. They need real native handles - a fake driver hands out
// handle-less values whose Close and Execute are pure no-op reads, so no
// overlap on them is observable - which is why this is the package's external
// test package driving a real engine.

package asyncengine_test

import (
	"context"
	"errors"
	"runtime"
	"sync"
	"testing"
	"time"

	"go.openpit.dev/openpit"
	"go.openpit.dev/openpit/accountadjustment"
	"go.openpit.dev/openpit/asyncengine"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pretrade"
	"go.openpit.dev/openpit/pretrade/policies"
	"go.openpit.dev/openpit/reject"
	"go.openpit.dev/openpit/tx"
)

const (
	// fallbackOverlapWindow is how long the caller-side fallback is watched
	// while the worker still owns the native request.
	fallbackOverlapWindow = 100 * time.Millisecond
	// fallbackWorkerWait bounds the wait for the worker to reach the main
	// stage so a regression fails instead of hanging the suite.
	fallbackWorkerWait = 5 * time.Second

	fallbackStressAccounts   = 4
	fallbackStressIterations = 50
	// fallbackShards keeps several accounts on one shard while leaving more
	// than one shard in play, so cleanup queued by one account is measured
	// against the accounts sharing its shard and against those that do not.
	// A single shard would hide any cross-account interference behind the
	// serialization every account already has.
	fallbackShards = 2
)

// mainStageProbePolicy parks the main stage inside the native execute call so
// a test can hold a task on the account worker: entered is closed on the first
// PerformPreTradeCheck, and the hook returns only once release is closed.
// Constructing it and closing release immediately turns it into a plain
// window-widener for the stress test.
type mainStageProbePolicy struct {
	entered chan struct{}
	release chan struct{}
	once    sync.Once
}

func newMainStageProbePolicy() *mainStageProbePolicy {
	return &mainStageProbePolicy{
		entered: make(chan struct{}),
		release: make(chan struct{}),
	}
}

func (*mainStageProbePolicy) Close() {}

func (*mainStageProbePolicy) Name() string { return "MainStageProbePolicy" }

func (*mainStageProbePolicy) PolicyGroupID() model.PolicyGroupID {
	return model.DefaultPolicyGroupID
}

func (*mainStageProbePolicy) CheckPreTradeStart(
	_ pretrade.Context, _ model.Order,
) []reject.Reject {
	return nil
}

func (p *mainStageProbePolicy) PerformPreTradeCheck(
	_ pretrade.Context, _ model.Order, _ tx.Mutations, _ pretrade.Result,
) []reject.Reject {
	p.once.Do(func() { close(p.entered) })
	// Yield even when release is already closed: the worker must stay inside
	// the native execute long enough for a caller-side release to overlap it.
	runtime.Gosched()
	<-p.release
	return nil
}

func (*mainStageProbePolicy) ApplyExecutionReport(
	_ pretrade.PostTradeContext,
	_ model.ExecutionReport,
	_ pretrade.PostTradeAdjustments,
	_ pretrade.PostTradePnls,
) []reject.AccountBlock {
	return nil
}

func (*mainStageProbePolicy) ApplyAccountAdjustment(
	_ accountadjustment.Context,
	_ param.AccountID,
	_ model.AccountAdjustment,
	_ tx.Mutations,
	_ pretrade.AccountOutcomes,
) (pretrade.PolicyAccountAdjustmentResult, []reject.Reject) {
	return pretrade.PolicyAccountAdjustmentResult{}, nil
}

type fallbackStrategy struct {
	name  string
	build func(*asyncengine.Builder) (*asyncengine.AsyncEngine, error)
}

var fallbackStrategies = []fallbackStrategy{
	{
		name: "Dynamic",
		build: func(builder *asyncengine.Builder) (*asyncengine.AsyncEngine, error) {
			return builder.Dynamic().Build()
		},
	},
	{
		name: "Sharded",
		build: func(builder *asyncengine.Builder) (*asyncengine.AsyncEngine, error) {
			return builder.Sharded(fallbackShards).Build()
		},
	},
}

func buildFallbackAsyncEngineWithStrategy(
	t *testing.T,
	policy pretrade.Policy,
	build func(*asyncengine.Builder) (*asyncengine.AsyncEngine, error),
) *asyncengine.AsyncEngine {
	t.Helper()
	builder, err := openpit.NewEngineBuilder().AccountSync().
		Builtin(policies.BuildOrderValidation()).
		PreTrade(policy).
		BuildAsync()
	if err != nil {
		t.Fatalf("BuildAsync() error = %v", err)
	}
	async, err := build(builder)
	if err != nil {
		t.Fatalf("strategy Build() error = %v", err)
	}
	return async
}

func fallbackTestOrder(t *testing.T, accountID uint64) model.Order {
	t.Helper()
	underlying, err := param.NewAsset("AAPL")
	if err != nil {
		t.Fatalf("NewAsset() error = %v", err)
	}
	settlement, err := param.NewAsset("USD")
	if err != nil {
		t.Fatalf("NewAsset() error = %v", err)
	}
	quantity, err := param.NewQuantityFromString("1")
	if err != nil {
		t.Fatalf("NewQuantityFromString() error = %v", err)
	}
	order := model.NewOrder()
	operation := order.EnsureOperationView()
	operation.SetInstrument(param.NewInstrument(underlying, settlement))
	operation.SetAccountID(param.NewAccountIDFromUint64(accountID))
	operation.SetSide(param.SideBuy)
	operation.SetTradeAmount(param.NewQuantityTradeAmount(quantity))
	return order
}

// cancelledContext returns a context whose cancellation is already observable,
// which makes strategy.submit refuse a task deterministically and schedule
// mandatory cleanup on the account lane.
func cancelledContext() context.Context {
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	return ctx
}

// TestAsyncRequestSubmitFailureFallbackWaitsForRunningWorker pins the
// submit-failure fallback of AsyncRequest, not async behaviour in general.
//
// The probe policy parks Execute on the account worker while it holds the
// native request, and Close is then called with an already-cancelled context.
// Its submit is refused, but cleanup must stay behind the worker: releasing the
// request while Execute owns it frees a live native handle. The test asserts
// that wait directly, that Execute still produces a usable reservation (its
// handle was never pulled out from under it), and that the reservation commits
// - a double free would abort the process before any of that is reached.
func TestAsyncRequestSubmitFailureFallbackWaitsForRunningWorker(t *testing.T) {
	for _, strategy := range fallbackStrategies {
		t.Run(strategy.name, func(t *testing.T) {
			testAsyncRequestSubmitFailureFallbackWaitsForRunningWorker(
				t, strategy.build,
			)
		})
	}
}

func testAsyncRequestSubmitFailureFallbackWaitsForRunningWorker(
	t *testing.T,
	build func(*asyncengine.Builder) (*asyncengine.AsyncEngine, error),
) {
	policy := newMainStageProbePolicy()
	async := buildFallbackAsyncEngineWithStrategy(t, policy, build)

	ctx := context.Background()
	request, rejects, err := async.StartPreTrade(
		ctx, fallbackTestOrder(t, 1),
	).Await(ctx)
	if err != nil {
		t.Fatalf("StartPreTrade Await() error = %v", err)
	}
	if request == nil {
		t.Fatalf("StartPreTrade rejected: %v", rejects)
	}

	execute := request.Execute(ctx)
	select {
	case <-policy.entered:
	case <-time.After(fallbackWorkerWait):
		t.Fatal("timeout waiting for the worker to enter the main stage")
	}

	closed := make(chan error, 1)
	go func() {
		_, closeErr := request.Close(cancelledContext()).Await(ctx)
		closed <- closeErr
	}()

	select {
	case closeErr := <-closed:
		t.Fatalf(
			"fallback released the request while the worker still owned it (err = %v)",
			closeErr,
		)
	case <-time.After(fallbackOverlapWindow):
	}

	close(policy.release)

	if closeErr := <-closed; !errors.Is(closeErr, context.Canceled) {
		t.Fatalf("Close() error = %v, want context.Canceled", closeErr)
	}

	reservation, rejects, err := execute.Await(ctx)
	if err != nil {
		t.Fatalf("Execute Await() error = %v", err)
	}
	if reservation == nil {
		t.Fatalf("Execute rejected: %v", rejects)
	}
	if _, err := reservation.CommitAndClose(ctx).Await(ctx); err != nil {
		t.Fatalf("CommitAndClose Await() error = %v", err)
	}

	if err := async.StopGraceful(ctx); err != nil {
		t.Fatalf("StopGraceful() error = %v", err)
	}
}

// TestAsyncDropCopySubmitFailureCleanupWaitsForAccountLane proves that a
// cancelled Close does not release the operation alongside unrelated work
// already running on the same account lane.
func TestAsyncDropCopySubmitFailureCleanupWaitsForAccountLane(t *testing.T) {
	for _, strategy := range fallbackStrategies {
		t.Run(strategy.name, func(t *testing.T) {
			testAsyncDropCopySubmitFailureCleanupWaitsForAccountLane(
				t, strategy.build,
			)
		})
	}
}

func testAsyncDropCopySubmitFailureCleanupWaitsForAccountLane(
	t *testing.T,
	build func(*asyncengine.Builder) (*asyncengine.AsyncEngine, error),
) {
	policy := newMainStageProbePolicy()
	close(policy.release)
	async := buildFallbackAsyncEngineWithStrategy(t, policy, build)

	ctx := context.Background()
	operation, rejects, err := async.ApplyDropCopy(
		ctx, fallbackTestOrder(t, 1),
	).Await(ctx)
	if err != nil {
		t.Fatalf("ApplyDropCopy Await() error = %v", err)
	}
	if operation == nil {
		t.Fatalf("ApplyDropCopy rejected: %v", rejects)
	}

	workerStarted := make(chan struct{})
	releaseWorker := make(chan struct{})
	worker := async.Submit(
		ctx,
		param.NewAccountIDFromUint64(1),
		func() error {
			close(workerStarted)
			<-releaseWorker
			return nil
		},
	)
	<-workerStarted

	closed := make(chan error, 1)
	go func() {
		_, closeErr := operation.Close(cancelledContext()).Await(ctx)
		closed <- closeErr
	}()

	select {
	case closeErr := <-closed:
		t.Fatalf(
			"cleanup overtook work on the account lane (err = %v)",
			closeErr,
		)
	case <-time.After(fallbackOverlapWindow):
	}

	close(releaseWorker)
	if _, err := worker.Await(ctx); err != nil {
		t.Fatalf("Submit Await() error = %v", err)
	}
	if closeErr := <-closed; !errors.Is(closeErr, context.Canceled) {
		t.Fatalf("Close() error = %v, want context.Canceled", closeErr)
	}
	if err := async.StopGraceful(ctx); err != nil {
		t.Fatalf("StopGraceful() error = %v", err)
	}
}

// TestAsyncSubmitFailureFallbackOverlapUnderStress hammers the same
// submit-failure fallback - on both AsyncRequest and AsyncReservation - from
// several account goroutines at once. Each iteration starts a worker-bound
// operation and a cancelled close on the same handle simultaneously, so under
// -race any unsynchronized inner call shows up as a data race on the handle
// rather than as a rare crash. The close must remain ordered behind the lane;
// only a freed live handle is not a legitimate outcome.
func TestAsyncSubmitFailureFallbackOverlapUnderStress(t *testing.T) {
	for _, strategy := range fallbackStrategies {
		t.Run(strategy.name, func(t *testing.T) {
			testAsyncSubmitFailureFallbackOverlapUnderStress(t, strategy.build)
		})
	}
}

func testAsyncSubmitFailureFallbackOverlapUnderStress(
	t *testing.T,
	build func(*asyncengine.Builder) (*asyncengine.AsyncEngine, error),
) {
	policy := newMainStageProbePolicy()
	// No parking: the stress test wants many short overlaps, not one long one.
	close(policy.release)
	async := buildFallbackAsyncEngineWithStrategy(t, policy, build)

	ctx := context.Background()
	orders := make([]model.Order, fallbackStressAccounts)
	for i := range orders {
		orders[i] = fallbackTestOrder(t, uint64(i))
	}

	var accounts sync.WaitGroup
	for _, order := range orders {
		accounts.Add(1)
		go func() {
			defer accounts.Done()
			for i := 0; i < fallbackStressIterations; i++ {
				request, _, err := async.StartPreTrade(ctx, order).Await(ctx)
				if err != nil {
					t.Errorf("call=%d StartPreTrade Await() error = %v", i, err)
					return
				}
				if request == nil {
					continue
				}
				raceRequestFallback(ctx, request)
			}
		}()
	}
	accounts.Wait()

	if err := async.StopGraceful(ctx); err != nil {
		t.Fatalf("StopGraceful() error = %v", err)
	}
}

// raceRequestFallback runs Execute (which lands on the worker) against a Close
// whose submit is refused and must therefore queue cleanup behind Execute.
func raceRequestFallback(
	ctx context.Context,
	request *asyncengine.AsyncRequest,
) {
	var (
		both        sync.WaitGroup
		reservation *asyncengine.AsyncReservation
		executeErr  error
	)
	start := make(chan struct{})
	both.Add(2)
	go func() {
		defer both.Done()
		<-start
		reservation, _, executeErr = request.Execute(ctx).Await(ctx)
	}()
	go func() {
		defer both.Done()
		<-start
		request.Close(cancelledContext())
	}()
	close(start)
	both.Wait()

	if executeErr != nil || reservation == nil {
		return
	}
	raceReservationFallback(ctx, reservation)
}

// raceReservationFallback runs Commit (which lands on the worker) against a
// Close whose submit is refused and must queue cleanup behind Commit. Close
// always releases the reservation on one side or the other, so no native handle
// survives the call.
func raceReservationFallback(
	ctx context.Context,
	reservation *asyncengine.AsyncReservation,
) {
	var both sync.WaitGroup
	start := make(chan struct{})
	both.Add(2)
	go func() {
		defer both.Done()
		<-start
		_, _ = reservation.Commit(ctx).Await(ctx)
	}()
	go func() {
		defer both.Done()
		<-start
		reservation.Close(cancelledContext())
	}()
	close(start)
	both.Wait()
}
