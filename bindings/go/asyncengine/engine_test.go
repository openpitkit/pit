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
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/future"
	"go.openpit.dev/openpit/pretrade"
	"go.openpit.dev/openpit/reject"
)

// fakeDriver records engine calls and lets tests insert ordering assertions
// without touching the native runtime.
type fakeDriver struct {
	mu                  sync.Mutex
	startCount          int64
	executeCount        int64
	reportCount         int64
	adjustmentCount     int64
	concurrentByAccount map[uint64]int64
	maxConcurrent       map[uint64]int64
	startDelay          time.Duration
	startHook           func()
}

func newFakeDriver() *fakeDriver {
	return &fakeDriver{
		concurrentByAccount: map[uint64]int64{},
		maxConcurrent:       map[uint64]int64{},
	}
}

func (d *fakeDriver) recordStart(accountID param.AccountID) func() {
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

func (d *fakeDriver) StartPreTrade(
	order model.Order,
) (*pretrade.Request, []reject.Reject, error) {
	op, _ := order.Operation().Get()
	accountID, _ := op.AccountID().Get()
	done := d.recordStart(accountID)
	defer done()
	if d.startDelay > 0 {
		time.Sleep(d.startDelay)
	}
	if d.startHook != nil {
		d.startHook()
	}
	atomic.AddInt64(&d.startCount, 1)
	return nil, []reject.Reject{}, nil
}

func (d *fakeDriver) ExecutePreTrade(
	order model.Order,
) (*pretrade.Reservation, []reject.Reject, error) {
	op, _ := order.Operation().Get()
	accountID, _ := op.AccountID().Get()
	done := d.recordStart(accountID)
	defer done()
	atomic.AddInt64(&d.executeCount, 1)
	return nil, []reject.Reject{}, nil
}

func (d *fakeDriver) ApplyExecutionReport(
	report model.ExecutionReport,
) (pretrade.PostTradeResult, error) {
	op, _ := report.Operation().Get()
	accountID, _ := op.AccountID().Get()
	done := d.recordStart(accountID)
	defer done()
	atomic.AddInt64(&d.reportCount, 1)
	return pretrade.PostTradeResult{}, nil
}

func (d *fakeDriver) ApplyAccountAdjustment(
	accountID param.AccountID,
	_ []model.AccountAdjustment,
) (accountadjustment.BatchResult, error) {
	done := d.recordStart(accountID)
	defer done()
	atomic.AddInt64(&d.adjustmentCount, 1)
	return accountadjustment.BatchResult{}, nil
}

func (*fakeDriver) Accounts() accounts.Accounts {
	return accounts.Accounts{}
}

func buildTestOrder(t *testing.T, accountID uint64) model.Order {
	t.Helper()
	order := model.NewOrder()
	op := order.EnsureOperationView()
	op.SetAccountID(param.NewAccountIDFromUint64(accountID))
	return order
}

func buildTestReport(t *testing.T, accountID uint64) model.ExecutionReport {
	t.Helper()
	report := model.NewExecutionReport()
	op := report.EnsureOperationView()
	op.SetAccountID(param.NewAccountIDFromUint64(accountID))
	return report
}

func TestAsyncEngineMissingAccountIDFailsFast(t *testing.T) {
	t.Parallel()
	driver := newFakeDriver()
	async, err := NewBuilder(driver).Sharded(2).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer func() {
		if err := async.StopGraceful(context.Background()); err != nil {
			t.Fatalf("StopGraceful() error = %v", err)
		}
	}()

	order := model.NewOrder()
	f := async.StartPreTrade(context.Background(), order)
	_, _, err = f.Await(context.Background())
	if !errors.Is(err, ErrMissingAccountID) {
		t.Fatalf("Await() err = %v, want ErrMissingAccountID", err)
	}
	if atomic.LoadInt64(&driver.startCount) != 0 {
		t.Fatalf("driver was called despite missing account ID")
	}
}

func TestAsyncEngineShardedSerializesPerAccount(t *testing.T) {
	t.Parallel()
	driver := newFakeDriver()
	driver.startDelay = 200 * time.Microsecond

	async, err := NewBuilder(driver).Sharded(4).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer func() {
		if err := async.StopGraceful(context.Background()); err != nil {
			t.Fatalf("StopGraceful() error = %v", err)
		}
	}()

	const accounts = 4
	const perAccount = 50
	orders := make([]model.Order, accounts)
	for i := range orders {
		orders[i] = buildTestOrder(t, uint64(i))
	}

	var wg sync.WaitGroup
	for i := 0; i < accounts; i++ {
		wg.Add(1)
		go func(idx int) {
			defer wg.Done()
			for j := 0; j < perAccount; j++ {
				f := async.StartPreTrade(context.Background(), orders[idx])
				if _, _, err := f.Await(context.Background()); err != nil {
					t.Errorf(
						"acc=%d call=%d Await() error = %v", idx, j, err,
					)
					return
				}
			}
		}(i)
	}
	wg.Wait()

	driver.mu.Lock()
	defer driver.mu.Unlock()
	for accountID, peak := range driver.maxConcurrent {
		if peak > 1 {
			t.Errorf(
				"account %d concurrency peak = %d, want <= 1",
				accountID, peak,
			)
		}
	}
	if got := atomic.LoadInt64(&driver.startCount); got != accounts*perAccount {
		t.Errorf(
			"startCount = %d, want %d", got, accounts*perAccount,
		)
	}
}

func TestAsyncEngineDynamicSerializesPerAccount(t *testing.T) {
	t.Parallel()
	driver := newFakeDriver()
	driver.startDelay = 200 * time.Microsecond

	async, err := NewBuilder(driver).
		Dynamic().
		MaxQueues(0).
		IdleCleanupAfter(0).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer func() {
		if err := async.StopGraceful(context.Background()); err != nil {
			t.Fatalf("StopGraceful() error = %v", err)
		}
	}()

	const accounts = 6
	const perAccount = 25
	orders := make([]model.Order, accounts)
	for i := range orders {
		orders[i] = buildTestOrder(t, uint64(i))
	}

	var wg sync.WaitGroup
	for i := 0; i < accounts; i++ {
		wg.Add(1)
		go func(idx int) {
			defer wg.Done()
			for j := 0; j < perAccount; j++ {
				f := async.StartPreTrade(context.Background(), orders[idx])
				if _, _, err := f.Await(context.Background()); err != nil {
					t.Errorf(
						"acc=%d call=%d Await() error = %v", idx, j, err,
					)
					return
				}
			}
		}(i)
	}
	wg.Wait()

	driver.mu.Lock()
	defer driver.mu.Unlock()
	for accountID, peak := range driver.maxConcurrent {
		if peak > 1 {
			t.Errorf(
				"account %d concurrency peak = %d, want <= 1",
				accountID, peak,
			)
		}
	}
	// T7: mirror the sharded variant — assert total dispatch count.
	if got := atomic.LoadInt64(&driver.startCount); got != accounts*perAccount {
		t.Errorf("startCount = %d, want %d", got, accounts*perAccount)
	}
}

func TestAsyncEngineDynamicMaxQueuesEnforced(t *testing.T) {
	t.Parallel()
	driver := newFakeDriver()
	// T6 de-flake: replace startDelay with gate-based determinism. The gate
	// keeps both workers hot until f3 has been submitted and its ErrQueueLimit
	// future has been awaited. A started channel ensures both workers have
	// entered the hook before f3 is submitted, removing the wall-clock race.
	gate := make(chan struct{})
	var startedCount atomic.Int64
	startedTwice := make(chan struct{})
	driver.startHook = func() {
		if startedCount.Add(1) == 2 {
			close(startedTwice)
		}
		<-gate
	}

	async, err := NewBuilder(driver).
		WithQueueCapacity(1).
		Dynamic().
		MaxQueues(2).
		IdleCleanupAfter(0).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer func() {
		if err := async.StopGraceful(
			context.Background(),
		); err != nil {
			t.Fatalf("StopGraceful() error = %v", err)
		}
	}()

	// Keep first two queues hot so they don't free a slot.
	f1 := async.StartPreTrade(
		context.Background(), buildTestOrder(t, 1),
	)
	f2 := async.StartPreTrade(
		context.Background(), buildTestOrder(t, 2),
	)

	// Wait until both workers have entered their hooks before submitting f3
	// so the ErrQueueLimit path is deterministic (both queues still live).
	select {
	case <-startedTwice:
	case <-time.After(5 * time.Second):
		t.Fatal("timeout waiting for both workers to start")
	}

	// Third account should hit the cap.
	f3 := async.StartPreTrade(
		context.Background(), buildTestOrder(t, 3),
	)

	_, _, err = f3.Await(context.Background())
	if !errors.Is(err, ErrQueueLimit) {
		t.Fatalf("f3 Await err = %v, want ErrQueueLimit", err)
	}

	close(gate)
	if _, _, err := f1.Await(context.Background()); err != nil {
		t.Fatalf("f1 Await error = %v", err)
	}
	if _, _, err := f2.Await(context.Background()); err != nil {
		t.Fatalf("f2 Await error = %v", err)
	}
}

func TestAsyncEngineStopGracefulDrains(t *testing.T) {
	t.Parallel()
	driver := newFakeDriver()
	async, err := NewBuilder(driver).Dynamic().Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}

	const total = 32
	const accounts = 4
	futures := make([]*future.Future2[*AsyncRequest, []reject.Reject], 0, total)
	for i := 0; i < total; i++ {
		futures = append(
			futures,
			async.StartPreTrade(
				context.Background(),
				buildTestOrder(t, uint64(i%accounts)),
			),
		)
	}
	if err := async.StopGraceful(context.Background()); err != nil {
		t.Fatalf("StopGraceful() error = %v", err)
	}
	for i, f := range futures {
		if _, _, err := f.Await(context.Background()); err != nil {
			t.Fatalf("futures[%d] Await error = %v", i, err)
		}
	}
	if got := atomic.LoadInt64(&driver.startCount); got != total {
		t.Fatalf("startCount = %d, want %d after graceful stop", got, total)
	}
	// T7: assert per-account peak concurrency == 1 (serialization invariant).
	driver.mu.Lock()
	defer driver.mu.Unlock()
	for accountID, peak := range driver.maxConcurrent {
		if peak > 1 {
			t.Errorf(
				"account %d concurrency peak = %d, want <= 1 (serialization violated)",
				accountID, peak,
			)
		}
	}
}

func TestAsyncEngineStopHardRejectsNewSubmits(t *testing.T) {
	t.Parallel()
	driver := newFakeDriver()
	async, err := NewBuilder(driver).Sharded(2).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	if err := async.StopHard(context.Background()); err != nil {
		t.Fatalf("StopHard() error = %v", err)
	}
	f := async.StartPreTrade(
		context.Background(), buildTestOrder(t, 1),
	)
	if _, _, err := f.Await(context.Background()); !errors.Is(
		err, ErrStopped,
	) {
		t.Fatalf("Await() err = %v, want ErrStopped", err)
	}
}

func TestAsyncEngineStopHardAbortsPending(t *testing.T) {
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

	// First task occupies the single worker; wait until the worker has
	// actually entered the hook so that the buffered pending tasks are
	// guaranteed to be queued behind it.
	first := async.StartPreTrade(
		context.Background(), buildTestOrder(t, 1),
	)
	<-started

	pending := make([]*future.Future2[*AsyncRequest, []reject.Reject], 0, 8)
	for i := 0; i < 8; i++ {
		pending = append(
			pending,
			async.StartPreTrade(
				context.Background(), buildTestOrder(t, 1),
			),
		)
	}

	// Drive hard stop synchronously to its hard-stop signal before
	// releasing the worker. Workers cannot observe hardStopped until the
	// signal fires, so a goroutine that closes the gate too early would
	// race the assertion. We sequence by closing the gate only after the
	// hard stop call has had the chance to set hardStopCh.
	hardStopDone := make(chan error, 1)
	hardStopStarted := make(chan struct{})
	go func() {
		close(hardStopStarted)
		hardStopDone <- async.StopHard(context.Background())
	}()
	<-hardStopStarted
	// Yield so the StopHard goroutine reaches signalHardStopOnce.
	// Bounded spin: if hardStopped is never set the test fails rather
	// than looping forever.
	hardStopDeadline := time.Now().Add(5 * time.Second)
	for !isStrategyHardStopped(async.strategy) {
		if time.Now().After(hardStopDeadline) {
			t.Fatal("timeout waiting for strategy to reach hard-stopped state")
		}
		runtime.Gosched()
	}
	close(gate)
	if err := <-hardStopDone; err != nil {
		t.Fatalf("StopHard() error = %v", err)
	}
	if _, _, err := first.Await(context.Background()); err != nil {
		t.Fatalf("first Await() error = %v", err)
	}
	abortedCount := 0
	for _, f := range pending {
		_, _, err := f.Await(context.Background())
		if errors.Is(err, ErrStopped) {
			abortedCount++
		}
	}
	if abortedCount != len(pending) {
		t.Fatalf(
			"aborted %d of %d pending tasks on hard stop",
			abortedCount, len(pending),
		)
	}
}

// isStrategyHardStopped peeks at the strategy's hardStopCh. It exists for
// tests that need to sequence around hard stop signaling and is not part
// of the public API.
func isStrategyHardStopped(s strategy) bool {
	switch impl := s.(type) {
	case *shardedStrategy:
		return impl.hardStopped()
	case *dynamicStrategy:
		return impl.hardStopped()
	default:
		return false
	}
}

func TestAsyncEngineSubmitOrdersWithEngineCalls(t *testing.T) {
	t.Parallel()
	driver := newFakeDriver()
	async, err := NewBuilder(driver).Dynamic().Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer func() {
		if err := async.StopGraceful(
			context.Background(),
		); err != nil {
			t.Fatalf("StopGraceful() error = %v", err)
		}
	}()

	accountID := param.NewAccountIDFromUint64(42)
	var step int64
	report := buildTestReport(t, 42)
	order := buildTestOrder(t, 42)

	f1 := async.StartPreTrade(context.Background(), order)
	custom := async.Submit(
		context.Background(), accountID,
		func() error {
			if !atomic.CompareAndSwapInt64(&step, 0, 1) {
				return errors.New("custom ran out of order")
			}
			return nil
		},
	)
	f3 := async.ApplyExecutionReport(context.Background(), report)

	if _, _, err := f1.Await(context.Background()); err != nil {
		t.Fatalf("f1 Await error = %v", err)
	}
	if _, err := custom.Await(context.Background()); err != nil {
		t.Fatalf("custom Await error = %v", err)
	}
	if _, err := f3.Await(context.Background()); err != nil {
		t.Fatalf("f3 Await error = %v", err)
	}
	if atomic.LoadInt64(&step) != 1 {
		t.Fatalf("step = %d, want 1", step)
	}
}

// TestAsyncEngineSubmitCtxCancel asserts that a submit whose ctx is cancelled
// while blocked on a full queue returns ctx.Err() on the future. The test is
// deterministic: an observer's OnQueueFullBlocked callback is used as the
// gate that tells us f3's submit goroutine is blocked, so the ctx cancel
// happens at exactly that point without any wall-clock sleep.
func TestAsyncEngineSubmitCtxCancel(t *testing.T) {
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

	// queueBlocked is closed the first time OnQueueFullBlocked fires,
	// signalling that f3's submit is stuck in the slow path.
	queueBlocked := make(chan struct{})
	var queueBlockedOnce sync.Once
	obs := &recordingObserver{
		OnQueueFullBlockedFunc: func() {
			queueBlockedOnce.Do(func() { close(queueBlocked) })
		},
	}

	async, err := NewBuilder(driver).
		WithObserver(obs).
		WithQueueCapacity(1).
		WithSlowSubmitThreshold(time.Nanosecond).
		Sharded(1).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}

	// Task 1: worker picks it up and blocks on the hook.
	f1 := async.StartPreTrade(
		context.Background(), buildTestOrder(t, 1),
	)
	<-started

	// Task 2: fills the buffer.
	f2 := async.StartPreTrade(
		context.Background(), buildTestOrder(t, 1),
	)

	// Task 3: submit blocks because the buffer is full. StartPreTrade is
	// synchronous on the submit path when the queue is full, so we run it in a
	// goroutine to avoid blocking the test goroutine. The queueBlocked gate
	// tells us f3's goroutine is stuck; then we cancel its ctx.
	f3Ctx, f3Cancel := context.WithCancel(context.Background())
	defer f3Cancel()
	f3resultCh := make(chan *future.Future2[*AsyncRequest, []reject.Reject], 1)
	go func() {
		f3resultCh <- async.StartPreTrade(f3Ctx, buildTestOrder(t, 1))
	}()

	// Wait for the queue-full notification (deterministic gate) then cancel.
	select {
	case <-queueBlocked:
	case <-time.After(5 * time.Second):
		t.Fatal("timeout waiting for OnQueueFullBlocked (f3 not stuck?)")
	}
	f3Cancel()

	// Receive the future (the goroutine may need a moment to unblock after cancel).
	var f3 *future.Future2[*AsyncRequest, []reject.Reject]
	select {
	case f3 = <-f3resultCh:
	case <-time.After(5 * time.Second):
		t.Fatal("timeout waiting for f3 StartPreTrade to return after cancel")
	}

	if _, _, err := f3.Await(context.Background()); !errors.Is(
		err, context.Canceled,
	) {
		t.Fatalf("f3 Await err = %v, want context.Canceled", err)
	}

	close(gate)
	if _, _, err := f1.Await(context.Background()); err != nil {
		t.Fatalf("f1 Await error = %v", err)
	}
	if _, _, err := f2.Await(context.Background()); err != nil {
		t.Fatalf("f2 Await error = %v", err)
	}
	if err := async.StopGraceful(context.Background()); err != nil {
		t.Fatalf("StopGraceful() error = %v", err)
	}
}

// runDynamicStopRace stresses the Dynamic queue-creation path against a
// concurrent stop. Producers submit a stream of distinct, ever-increasing
// account IDs to force continuous getOrCreate while stop runs. A regression
// in the stop/create fencing surfaces either as a "WaitGroup misuse" panic
// (Add raced Wait) or as a worker that never exits (stop returns
// DeadlineExceeded instead of nil within the timeout).
func runDynamicStopRace(
	t *testing.T,
	idleCleanupAfter time.Duration,
	stop func(*AsyncEngine, context.Context) error,
) {
	t.Helper()
	driver := newFakeDriver()
	async, err := NewBuilder(driver).
		WithQueueCapacity(2).
		Dynamic().
		MaxQueues(0).
		IdleCleanupAfter(idleCleanupAfter).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}

	const producers = 16
	const perProducer = 200
	var nextID atomic.Uint64
	// started is signalled once after the first submit so the stop is
	// triggered only while producers are running, exercising the race
	// window without a fixed sleep.
	started := make(chan struct{}, 1)

	type future2 = future.Future2[*AsyncRequest, []reject.Reject]
	var wg sync.WaitGroup
	collected := make([][]*future2, producers)
	for p := 0; p < producers; p++ {
		wg.Add(1)
		go func(p int) {
			defer wg.Done()
			futures := make([]*future2, 0, perProducer)
			for i := 0; i < perProducer; i++ {
				id := nextID.Add(1)
				order := buildTestOrder(t, id)
				futures = append(
					futures,
					async.StartPreTrade(context.Background(), order),
				)
				if i == 0 {
					select {
					case started <- struct{}{}:
					default:
					}
				}
			}
			collected[p] = futures
		}(p)
	}

	ctx, cancel := context.WithTimeout(
		context.Background(), 5*time.Second,
	)
	defer cancel()
	<-started
	if err := stop(async, ctx); err != nil {
		t.Fatalf("stop returned error = %v, want nil", err)
	}

	wg.Wait()
	for _, futures := range collected {
		for _, f := range futures {
			// Racing submits may be rejected once stop wins; only
			// unexpected errors fail the test.
			if _, _, err := f.Await(context.Background()); err != nil &&
				!errors.Is(err, ErrStopped) &&
				!errors.Is(err, ErrQueueLimit) &&
				!errors.Is(err, context.Canceled) &&
				!errors.Is(err, context.DeadlineExceeded) {
				t.Fatalf("submit Await unexpected error = %v", err)
			}
		}
	}
}

// TestAsyncEngineDynamicStopRacesQueueCreation groups the three stop-race
// variants into a single table-driven test over (idleCleanupAfter, stopMode).
// Each sub-test is parallel so they exercise the race window concurrently.
func TestAsyncEngineDynamicStopRacesQueueCreation(t *testing.T) {
	t.Parallel()
	cases := []struct {
		name             string
		idleCleanupAfter time.Duration
		stop             func(*AsyncEngine, context.Context) error
	}{
		{
			name:             "GracefulNoCleanup",
			idleCleanupAfter: 0,
			stop: func(e *AsyncEngine, ctx context.Context) error {
				return e.StopGraceful(ctx)
			},
		},
		{
			name:             "HardNoCleanup",
			idleCleanupAfter: 0,
			stop: func(e *AsyncEngine, ctx context.Context) error {
				return e.StopHard(ctx)
			},
		},
		{
			name:             "GracefulWithCleanup",
			idleCleanupAfter: 10 * time.Millisecond,
			stop: func(e *AsyncEngine, ctx context.Context) error {
				return e.StopGraceful(ctx)
			},
		},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()
			runDynamicStopRace(t, tc.idleCleanupAfter, tc.stop)
		})
	}
}

// Legacy names kept as thin wrappers so external references remain valid.
// The real logic lives in TestAsyncEngineDynamicStopRacesQueueCreation above.

func TestAsyncEngineDynamicStopGracefulRacesQueueCreation(t *testing.T) {
	t.Parallel()
	runDynamicStopRace(t, 0, func(e *AsyncEngine, ctx context.Context) error {
		return e.StopGraceful(ctx)
	})
}

func TestAsyncEngineDynamicStopHardRacesQueueCreation(t *testing.T) {
	t.Parallel()
	runDynamicStopRace(t, 0, func(e *AsyncEngine, ctx context.Context) error {
		return e.StopHard(ctx)
	})
}

func TestAsyncEngineDynamicStopRacesQueueCreationWithCleanup(t *testing.T) {
	t.Parallel()
	runDynamicStopRace(
		t, 10*time.Millisecond,
		func(e *AsyncEngine, ctx context.Context) error {
			return e.StopGraceful(ctx)
		},
	)
}

// TestAsyncEngineDynamicNoRetireDuringInFlightTask guards the AccountSync
// invariant against idle cleanup retiring a queue whose worker has dequeued
// a task (so the channel is empty) but is still running it for longer than
// idleCleanupAfter. If cleanup retired that queue, a fresh submit for the
// same account would create a second queue and a second worker that runs
// concurrently with the first - two operations for one account inside the
// engine at once.
//
// The test drives cleanupIdle directly rather than waiting on the
// background loop: with a sub-second idle window the loop's scan period
// floors to the default cadence, so a manual call is the deterministic way
// to fire retirement at the exact moment a long task is in flight.
func TestAsyncEngineDynamicNoRetireDuringInFlightTask(t *testing.T) {
	t.Parallel()
	const idleCleanupAfter = time.Millisecond
	driver := newFakeDriver()
	gate := make(chan struct{})
	entered := make(chan struct{}, 4)
	driver.startHook = func() {
		entered <- struct{}{}
		<-gate
	}

	async, err := NewBuilder(driver).
		Dynamic().
		MaxQueues(0).
		IdleCleanupAfter(idleCleanupAfter).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	strategy, ok := async.strategy.(*dynamicStrategy)
	if !ok {
		t.Fatalf("strategy type = %T, want *dynamicStrategy", async.strategy)
	}

	const account = uint64(7)
	accountID := param.NewAccountIDFromUint64(account)

	// First task occupies the worker and blocks in the hook with the
	// channel drained.
	first := async.StartPreTrade(context.Background(), buildTestOrder(t, account))
	<-entered

	strategy.mu.RLock()
	q := strategy.queues[accountID]
	strategy.mu.RUnlock()
	if q == nil {
		t.Fatalf("queue for account %d not created", account)
	}

	// Wait until the queue looks idle to cleanup (empty channel, lastActive
	// older than the idle window) so retireIfIdle reaches its gate check.
	for time.Now().Before(q.lastActiveAt().Add(idleCleanupAfter)) {
		runtime.Gosched()
	}
	strategy.cleanupIdle()

	if q.closed.Load() {
		t.Fatalf("queue retired while a task was in flight")
	}
	strategy.mu.RLock()
	stillMapped := strategy.queues[accountID] == q
	strategy.mu.RUnlock()
	if !stillMapped {
		t.Fatalf("queue replaced in map while a task was in flight")
	}

	// A second submit for the same account must reuse the live queue, not
	// spin up a competing worker.
	second := async.StartPreTrade(context.Background(), buildTestOrder(t, account))

	close(gate)
	if _, _, err := first.Await(context.Background()); err != nil {
		t.Fatalf("first Await() error = %v", err)
	}
	if _, _, err := second.Await(context.Background()); err != nil {
		t.Fatalf("second Await() error = %v", err)
	}
	if err := async.StopGraceful(context.Background()); err != nil {
		t.Fatalf("StopGraceful() error = %v", err)
	}

	driver.mu.Lock()
	peak := driver.maxConcurrent[account]
	driver.mu.Unlock()
	if peak > 1 {
		t.Fatalf("account %d concurrency peak = %d, want <= 1", account, peak)
	}
}

func TestAsyncEnginePreCancelledCtxFailsBeforeDriver(t *testing.T) {
	t.Parallel()
	driver := newFakeDriver()
	async, err := NewBuilder(driver).Sharded(1).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer func() {
		if err := async.StopGraceful(context.Background()); err != nil {
			t.Fatalf("StopGraceful() error = %v", err)
		}
	}()

	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	f := async.StartPreTrade(ctx, buildTestOrder(t, 1))
	if _, _, err := f.Await(context.Background()); !errors.Is(
		err, context.Canceled,
	) {
		t.Fatalf("Await() err = %v, want context.Canceled", err)
	}
	if got := atomic.LoadInt64(&driver.startCount); got != 0 {
		t.Fatalf("startCount = %d, want 0 for a pre-cancelled ctx", got)
	}
}

// recordingStrategy records the accountID of each submit and returns nil
// without running task.run or task.abort, so the wrapper objects' nil inner
// handles are never dereferenced. It lets the accept-path routing be proven
// without the native runtime.
type recordingStrategy struct {
	mu       sync.Mutex
	accounts []param.AccountID
}

func (s *recordingStrategy) submit(
	_ context.Context, accountID param.AccountID, _ pendingTask,
) error {
	s.mu.Lock()
	s.accounts = append(s.accounts, accountID)
	s.mu.Unlock()
	return nil
}

func (*recordingStrategy) stopGraceful(context.Context) error { return nil }

func (*recordingStrategy) stopHard(context.Context) error { return nil }

func (s *recordingStrategy) last() (param.AccountID, bool) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if len(s.accounts) == 0 {
		return param.AccountID{}, false
	}
	return s.accounts[len(s.accounts)-1], true
}

// TestAsyncEngineWrapperObjectsRouteToPinnedAccount asserts that wrapper
// objects (AsyncRequest, AsyncReservation) route their callbacks to the
// account they were pinned to. The recordingStrategy drops tasks without
// running them; as a result every returned future remains unresolved — this is
// the explicit invariant tested below: routing is proven, not execution.
func TestAsyncEngineWrapperObjectsRouteToPinnedAccount(t *testing.T) {
	t.Parallel()
	strategy := &recordingStrategy{}
	eng := newAsyncEngine(newFakeDriver(), nil, strategy)
	acc := param.NewAccountIDFromUint64(99)
	ctx := context.Background()

	assertRouted := func(label string, f interface{ Done() bool }) {
		t.Helper()
		got, ok := strategy.last()
		if !ok {
			t.Fatalf("%s: strategy recorded no submit", label)
		}
		if got != acc {
			t.Fatalf("%s: submit account = %v, want %v", label, got, acc)
		}
		// The recording strategy never calls run or abort, so the future must
		// remain unresolved — assert that explicitly.
		if f.Done() {
			t.Errorf(
				"%s: future resolved, want unresolved (strategy drops tasks)",
				label,
			)
		}
	}

	req := newAsyncRequest(nil, eng, acc)
	assertRouted("AsyncRequest.Execute", req.Execute(ctx))
	assertRouted("AsyncRequest.Close", req.Close(ctx))

	res := newAsyncReservation(nil, eng, acc)
	assertRouted("AsyncReservation.Commit", res.Commit(ctx))
	assertRouted("AsyncReservation.CommitAndClose", res.CommitAndClose(ctx))
	assertRouted("AsyncReservation.Rollback", res.Rollback(ctx))
	assertRouted("AsyncReservation.RollbackAndClose", res.RollbackAndClose(ctx))
	assertRouted("AsyncReservation.Close", res.Close(ctx))
}
