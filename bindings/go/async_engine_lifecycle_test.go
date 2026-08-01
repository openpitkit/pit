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

package openpit

import (
	"context"
	"errors"
	"sync"
	"sync/atomic"
	"testing"

	"go.openpit.dev/openpit/accountadjustment"
	"go.openpit.dev/openpit/asyncengine"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pretrade"
	"go.openpit.dev/openpit/pretrade/policies"
	"go.openpit.dev/openpit/reject"
	"go.openpit.dev/openpit/tx"
)

// buildOrderValidationAccountSyncEngine creates an AccountSync engine with
// only the OrderValidation policy. OrderValidation accepts all well-formed
// orders, so every call in a happy-path lifecycle test succeeds.
func buildOrderValidationAccountSyncEngine(t *testing.T) *Engine {
	t.Helper()
	engine, err := NewEngineBuilder().AccountSync().
		Builtin(policies.BuildOrderValidation()).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	return engine
}

// TestAsyncEngineRequestExecuteCommitLifecycle exercises the full
// AsyncRequest/AsyncReservation wrapper lifecycle through a real engine.
//
// The test covers two paths:
//  1. StartPreTrade -> Execute -> CommitAndClose (commit path)
//  2. StartPreTrade -> Close without Execute (abandon path)
//
// Both paths must complete without error, confirming that the native
// cgo-handle lifecycle is properly managed by the wrapper types.
func TestAsyncEngineRequestExecuteCommitLifecycle(t *testing.T) {
	engine := buildOrderValidationAccountSyncEngine(t)
	asyncEngine, err := asyncengine.NewBuilder(engine).Sharded(1).Build()
	if err != nil {
		t.Fatalf("Sharded.Build() error = %v", err)
	}

	ctx := context.Background()
	order := multithreadTestOrder(t, 1)

	// Path 1: start -> execute -> commit.
	t.Run("execute-and-commit", func(t *testing.T) {
		request, rejects, err := asyncEngine.StartPreTrade(ctx, order).Await(ctx)
		if err != nil {
			t.Fatalf("StartPreTrade Await() error = %v", err)
		}
		if request == nil {
			t.Fatalf("StartPreTrade rejected: %v", rejects)
		}

		reservation, rejects, err := request.Execute(ctx).Await(ctx)
		if err != nil {
			t.Fatalf("Execute Await() error = %v", err)
		}
		if reservation == nil {
			t.Fatalf("Execute rejected: %v", rejects)
		}

		if _, err := reservation.CommitAndClose(ctx).Await(ctx); err != nil {
			t.Fatalf("CommitAndClose Await() error = %v", err)
		}
	})

	// Path 2: start -> close without execute (abandon).
	t.Run("close-without-execute", func(t *testing.T) {
		request, rejects, err := asyncEngine.StartPreTrade(ctx, order).Await(ctx)
		if err != nil {
			t.Fatalf("StartPreTrade Await() error = %v", err)
		}
		if request == nil {
			t.Fatalf("StartPreTrade rejected: %v", rejects)
		}

		if _, err := request.Close(ctx).Await(ctx); err != nil {
			t.Fatalf("Close Await() error = %v", err)
		}
	})

	if err := asyncEngine.StopGraceful(ctx); err != nil {
		t.Fatalf("StopGraceful() error = %v", err)
	}
	engine.Stop()
}

// buildDropCopyAsyncEngine wraps an AccountSync engine that runs one
// mutation-tracking policy, so a test can observe which mutation callbacks a
// queued drop-copy finalizer actually ran.
func buildDropCopyAsyncEngine(
	t *testing.T,
	policy pretrade.Policy,
) *asyncengine.AsyncEngine {
	t.Helper()
	engine, err := NewEngineBuilder().AccountSync().PreTrade(policy).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	t.Cleanup(engine.Stop)
	asyncEngine, err := asyncengine.NewBuilder(engine).Sharded(1).Build()
	if err != nil {
		t.Fatalf("Sharded.Build() error = %v", err)
	}
	return asyncEngine
}

// applyAsyncDropCopy runs one drop copy through the account queue and returns
// the accepted operation.
func applyAsyncDropCopy(
	t *testing.T,
	asyncEngine *asyncengine.AsyncEngine,
) *asyncengine.AsyncDropCopyOperation {
	t.Helper()
	ctx := context.Background()
	operation, rejects, err := asyncEngine.ApplyDropCopy(
		ctx, newValidOrderForNativeE2E(t),
	).Await(ctx)
	if err != nil {
		t.Fatalf("ApplyDropCopy Await() error = %v", err)
	}
	if operation == nil {
		t.Fatalf("ApplyDropCopy rejected: %v", rejects)
	}
	return operation
}

// TestAsyncEngineDropCopyFinalizationLifecycle exercises every queued
// drop-copy finalizer on a real engine and asserts which mutation callbacks the
// worker ran: commit applies the prepared state, rollback compensates it, and
// an abandoned operation compensates implicitly.
func TestAsyncEngineDropCopyFinalizationLifecycle(t *testing.T) {
	ctx := context.Background()

	t.Run("commit-and-close", func(t *testing.T) {
		policy := &mutationTrackingPolicy{name: "async-drop-copy-commit"}
		operation := applyAsyncDropCopy(t, buildDropCopyAsyncEngine(t, policy))
		if _, err := operation.CommitAndClose(ctx).Await(ctx); err != nil {
			t.Fatalf("CommitAndClose Await() error = %v", err)
		}
		if policy.commitCalls != 1 || policy.rollbackCalls != 0 {
			t.Fatalf(
				"callbacks = commit %d, rollback %d; want 1, 0",
				policy.commitCalls, policy.rollbackCalls,
			)
		}
	})

	t.Run("commit-then-close", func(t *testing.T) {
		policy := &mutationTrackingPolicy{name: "async-drop-copy-commit-close"}
		operation := applyAsyncDropCopy(t, buildDropCopyAsyncEngine(t, policy))
		if _, err := operation.Commit(ctx).Await(ctx); err != nil {
			t.Fatalf("Commit Await() error = %v", err)
		}
		if _, err := operation.Close(ctx).Await(ctx); err != nil {
			t.Fatalf("Close Await() error = %v", err)
		}
		if policy.commitCalls != 1 || policy.rollbackCalls != 0 {
			t.Fatalf(
				"callbacks = commit %d, rollback %d; want 1, 0",
				policy.commitCalls, policy.rollbackCalls,
			)
		}
	})

	t.Run("rollback-and-close", func(t *testing.T) {
		policy := &mutationTrackingPolicy{name: "async-drop-copy-rollback"}
		operation := applyAsyncDropCopy(t, buildDropCopyAsyncEngine(t, policy))
		if _, err := operation.RollbackAndClose(ctx).Await(ctx); err != nil {
			t.Fatalf("RollbackAndClose Await() error = %v", err)
		}
		if policy.commitCalls != 0 || policy.rollbackCalls != 1 {
			t.Fatalf(
				"callbacks = commit %d, rollback %d; want 0, 1",
				policy.commitCalls, policy.rollbackCalls,
			)
		}
	})

	t.Run("rollback-then-close", func(t *testing.T) {
		policy := &mutationTrackingPolicy{name: "async-drop-copy-rollback-close"}
		operation := applyAsyncDropCopy(t, buildDropCopyAsyncEngine(t, policy))
		if _, err := operation.Rollback(ctx).Await(ctx); err != nil {
			t.Fatalf("Rollback Await() error = %v", err)
		}
		if _, err := operation.Close(ctx).Await(ctx); err != nil {
			t.Fatalf("Close Await() error = %v", err)
		}
		if policy.commitCalls != 0 || policy.rollbackCalls != 1 {
			t.Fatalf(
				"callbacks = commit %d, rollback %d; want 0, 1",
				policy.commitCalls, policy.rollbackCalls,
			)
		}
	})

	t.Run("close-abandons", func(t *testing.T) {
		policy := &mutationTrackingPolicy{name: "async-drop-copy-close"}
		operation := applyAsyncDropCopy(t, buildDropCopyAsyncEngine(t, policy))
		if _, err := operation.Close(ctx).Await(ctx); err != nil {
			t.Fatalf("Close Await() error = %v", err)
		}
		if policy.commitCalls != 0 || policy.rollbackCalls != 1 {
			t.Fatalf(
				"callbacks = commit %d, rollback %d; want 0, 1",
				policy.commitCalls, policy.rollbackCalls,
			)
		}
	})
}

func TestAsyncDropCopyOperationForwardsSnapshotAccessors(t *testing.T) {
	ctx := context.Background()
	operation := applyAsyncDropCopy(
		t,
		buildDropCopyAsyncEngine(t, &mutationTrackingPolicy{name: "async-accessors"}),
	)

	if _, err := operation.Lock(); err != nil {
		t.Fatalf("Lock() error = %v", err)
	}
	adjustments, err := operation.AccountAdjustments()
	if err != nil {
		t.Fatalf("AccountAdjustments() error = %v", err)
	}
	if len(adjustments) != 0 {
		t.Fatalf("AccountAdjustments() = %v, want empty", adjustments)
	}
	block, err := operation.AccountBlock()
	if err != nil {
		t.Fatalf("AccountBlock() error = %v", err)
	}
	if block != nil {
		t.Fatalf("AccountBlock() = %v, want nil", block)
	}
	blocked, err := operation.IsAccountBlocked()
	if err != nil {
		t.Fatalf("IsAccountBlocked() error = %v", err)
	}
	if blocked {
		t.Fatal("IsAccountBlocked() = true, want false")
	}

	if _, err := operation.Close(ctx).Await(ctx); err != nil {
		t.Fatalf("Close Await() error = %v", err)
	}
	if _, err := operation.Lock(); !errors.Is(err, pretrade.ErrDropCopyOperationClosed) {
		t.Fatalf("Lock() after Close error = %v, want ErrDropCopyOperationClosed", err)
	}
}

// TestAsyncEngineUnblockAllClearsMutationFinalizerBlock exercises the queued
// operator counterpart of the kill switch: a commit finalizer that fails during
// a queued finalization blocks every account, including one the failing order
// never touched, and UnblockAll lifts that engine-wide block.
func TestAsyncEngineUnblockAllClearsMutationFinalizerBlock(t *testing.T) {
	const untouchedAccountID uint64 = 2002
	ctx := context.Background()
	asyncEngine := buildDropCopyAsyncEngine(t, &mutationTrackingPolicy{
		name:             "async-mutation-failing-commit",
		commitPanicValue: "go mutation commit exploded",
	})

	operation := applyAsyncDropCopy(t, asyncEngine)
	if _, err := operation.CommitAndClose(ctx).Await(ctx); err != nil {
		t.Fatalf("CommitAndClose Await() error = %v", err)
	}

	request, rejects, err := asyncEngine.StartPreTrade(
		ctx, rateLimitTestOrder(t, untouchedAccountID),
	).Await(ctx)
	if err != nil {
		t.Fatalf("StartPreTrade Await() error = %v", err)
	}
	if request != nil {
		t.Fatal("StartPreTrade() request != nil, want kill-switch block")
	}
	if len(rejects) != 1 || rejects[0].Code != reject.CodeSystemUnavailable {
		t.Fatalf("StartPreTrade() rejects = %v, want SystemUnavailable", rejects)
	}

	if _, err := asyncEngine.Accounts().UnblockAll(ctx).Await(ctx); err != nil {
		t.Fatalf("UnblockAll Await() error = %v", err)
	}

	request, rejects, err = asyncEngine.StartPreTrade(
		ctx, rateLimitTestOrder(t, untouchedAccountID),
	).Await(ctx)
	if err != nil {
		t.Fatalf("StartPreTrade Await() after UnblockAll error = %v", err)
	}
	if request == nil {
		t.Fatalf("StartPreTrade() after UnblockAll rejects = %v, want request", rejects)
	}
	if _, err := request.Close(ctx).Await(ctx); err != nil {
		t.Fatalf("Close Await() error = %v", err)
	}
}

// TestAsyncEngineDropCopyCloseFallbackReleasesOperation pins the caller-side
// fallback of a Close-flavored finalizer: when the queue refuses the task, the
// operation is released on the caller's goroutine - which compensates it - and
// the future still reports why the submit failed.
func TestAsyncEngineDropCopyCloseFallbackReleasesOperation(t *testing.T) {
	ctx := context.Background()

	t.Run("cancelled-submit", func(t *testing.T) {
		policy := &mutationTrackingPolicy{name: "async-drop-copy-cancelled"}
		operation := applyAsyncDropCopy(t, buildDropCopyAsyncEngine(t, policy))
		cancelled, cancel := context.WithCancel(ctx)
		cancel()
		if _, err := operation.Close(cancelled).Await(ctx); !errors.Is(
			err, context.Canceled,
		) {
			t.Fatalf("Close Await() error = %v, want context.Canceled", err)
		}
		if policy.commitCalls != 0 || policy.rollbackCalls != 1 {
			t.Fatalf(
				"fallback callbacks = commit %d, rollback %d; want 0, 1",
				policy.commitCalls, policy.rollbackCalls,
			)
		}
	})

	t.Run("stopped-engine", func(t *testing.T) {
		policy := &mutationTrackingPolicy{name: "async-drop-copy-stopped"}
		asyncEngine := buildDropCopyAsyncEngine(t, policy)
		operation := applyAsyncDropCopy(t, asyncEngine)
		if err := asyncEngine.StopGraceful(ctx); err != nil {
			t.Fatalf("StopGraceful() error = %v", err)
		}
		if _, err := operation.CommitAndClose(ctx).Await(ctx); !errors.Is(
			err, asyncengine.ErrStopped,
		) {
			t.Fatalf("CommitAndClose Await() error = %v, want ErrStopped", err)
		}
		if policy.commitCalls != 0 || policy.rollbackCalls != 1 {
			t.Fatalf(
				"fallback callbacks = commit %d, rollback %d; want 0, 1",
				policy.commitCalls, policy.rollbackCalls,
			)
		}
	})
}

// TestAsyncEngineWrapperConcurrentLifecyclesNoRace runs the full
// StartPreTrade -> Execute -> CommitAndClose lifecycle from many goroutines
// concurrently through a sharded async engine wrapping a real engine. The
// goal is to expose data races (run under -race) in the wrapper types and
// the native cgo-handle machinery.
func TestAsyncEngineWrapperConcurrentLifecyclesNoRace(t *testing.T) {
	const accounts = 8
	const perAccount = 100

	engine := buildOrderValidationAccountSyncEngine(t)
	asyncEngine, err := asyncengine.NewBuilder(engine).Sharded(4).Build()
	if err != nil {
		t.Fatalf("Sharded.Build() error = %v", err)
	}

	orders := make([]model.Order, accounts)
	for i := range orders {
		orders[i] = multithreadTestOrder(t, uint64(i))
	}

	ctx := context.Background()
	var wg sync.WaitGroup
	for accIdx, order := range orders {
		wg.Add(1)
		go func(acc int, ord model.Order) {
			defer wg.Done()
			for j := 0; j < perAccount; j++ {
				request, rejects, err := asyncEngine.StartPreTrade(
					ctx, ord,
				).Await(ctx)
				if err != nil {
					t.Errorf("acc=%d call=%d StartPreTrade Await() error = %v", acc, j, err)
					return
				}
				if request == nil {
					t.Errorf(
						"acc=%d call=%d StartPreTrade rejected: %v",
						acc, j, rejects,
					)
					return
				}
				reservation, rejects, err := request.Execute(ctx).Await(ctx)
				if err != nil {
					t.Errorf("acc=%d call=%d Execute Await() error = %v", acc, j, err)
					return
				}
				if reservation == nil {
					t.Errorf(
						"acc=%d call=%d Execute rejected: %v",
						acc, j, rejects,
					)
					return
				}
				if _, err := reservation.CommitAndClose(
					ctx,
				).Await(ctx); err != nil {
					t.Errorf(
						"acc=%d call=%d CommitAndClose Await() error = %v",
						acc, j, err,
					)
					return
				}
			}
		}(accIdx, order)
	}
	wg.Wait()

	if err := asyncEngine.StopGraceful(ctx); err != nil {
		t.Fatalf("StopGraceful() error = %v", err)
	}
	engine.Stop()
}

// concurrencyProbePolicy tracks the peak number of simultaneously active
// policy calls per account. It instruments both CheckPreTradeStart (start
// stage) and PerformPreTradeCheck (main/execute stage) so that any overlap
// across the Start->Execute boundary is visible.
//
// All policy methods are required by pretrade.Policy. The probe only
// instruments the two pre-trade check methods; other methods are no-ops.
type concurrencyProbePolicy struct {
	mu                  sync.Mutex
	concurrentByAccount map[uint64]int64
	maxConcurrent       map[uint64]int64
}

func newConcurrencyProbePolicy() *concurrencyProbePolicy {
	return &concurrencyProbePolicy{
		concurrentByAccount: make(map[uint64]int64),
		maxConcurrent:       make(map[uint64]int64),
	}
}

func (*concurrencyProbePolicy) Close() {}

func (*concurrencyProbePolicy) Name() string { return "ConcurrencyProbePolicy" }

func (*concurrencyProbePolicy) PolicyGroupID() model.PolicyGroupID {
	return model.DefaultPolicyGroupID
}

// enter increments the per-account in-flight counter and records the peak.
// It returns a done function that must be deferred by the caller.
func (p *concurrencyProbePolicy) enter(accountID uint64) func() {
	p.mu.Lock()
	p.concurrentByAccount[accountID]++
	if cur := p.concurrentByAccount[accountID]; cur > p.maxConcurrent[accountID] {
		p.maxConcurrent[accountID] = cur
	}
	p.mu.Unlock()
	return func() {
		p.mu.Lock()
		p.concurrentByAccount[accountID]--
		p.mu.Unlock()
	}
}

func (p *concurrencyProbePolicy) CheckPreTradeStart(
	_ pretrade.Context, order model.Order,
) []reject.Reject {
	op, ok := order.Operation().Get()
	if !ok {
		return nil
	}
	id, ok := op.AccountID().Get()
	if !ok {
		return nil
	}
	done := p.enter(uint64(id.Handle()))
	defer done()
	return nil
}

func (p *concurrencyProbePolicy) PerformPreTradeCheck(
	_ pretrade.Context, order model.Order, _ tx.Mutations, _ pretrade.Result,
) []reject.Reject {
	op, ok := order.Operation().Get()
	if !ok {
		return nil
	}
	id, ok := op.AccountID().Get()
	if !ok {
		return nil
	}
	done := p.enter(uint64(id.Handle()))
	defer done()
	return nil
}

func (*concurrencyProbePolicy) ApplyExecutionReport(
	_ pretrade.PostTradeContext,
	_ model.ExecutionReport,
	_ pretrade.PostTradeAdjustments,
	_ pretrade.PostTradePnls,
) []reject.AccountBlock {
	return nil
}

func (*concurrencyProbePolicy) ApplyAccountAdjustment(
	_ accountadjustment.Context,
	_ param.AccountID,
	_ model.AccountAdjustment,
	_ tx.Mutations,
	_ pretrade.AccountOutcomes,
) (pretrade.PolicyAccountAdjustmentResult, []reject.Reject) {
	return pretrade.PolicyAccountAdjustmentResult{}, nil
}

// peakFor returns the observed peak concurrent call count for the given
// account handle value.
func (p *concurrencyProbePolicy) peakFor(accountHandle uint64) int64 {
	p.mu.Lock()
	defer p.mu.Unlock()
	return p.maxConcurrent[accountHandle]
}

// TestAsyncEngineSerializationAcrossExecuteBoundary verifies that
// per-account serialization is preserved across the Start -> Execute
// boundary when the caller uses AsyncRequest.Execute on a real engine.
//
// A custom probe policy increments a per-account counter on entry and
// decrements on exit in both CheckPreTradeStart and PerformPreTradeCheck.
// Because the async facade serializes all per-account operations through
// the same queue, a peak concurrent count > 1 for any account would
// indicate two policy calls for the same account overlapping - a
// correctness violation.
func TestAsyncEngineSerializationAcrossExecuteBoundary(t *testing.T) {
	const accounts = 4
	// Multiple concurrent submitters per account are required: with a single
	// submitter the driver itself never issues overlapping operations for one
	// account, so the probe could never observe peak > 1 even if the wrapper
	// bypassed the queue. Several submitters hammering the same account force
	// the async facade to be the only thing preventing overlap.
	const submittersPerAccount = 4
	const perSubmitter = 30

	probe := newConcurrencyProbePolicy()
	engine, err := NewEngineBuilder().AccountSync().PreTrade(probe).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	asyncEngine, err := asyncengine.NewBuilder(engine).Sharded(4).Build()
	if err != nil {
		t.Fatalf("Sharded.Build() error = %v", err)
	}

	orders := make([]model.Order, accounts)
	for i := range orders {
		orders[i] = multithreadTestOrder(t, uint64(i))
	}

	// accountHandles maps order-index to the handle value used as map key
	// inside the probe, so the assertion can look up the right bucket.
	accountHandles := make([]uint64, accounts)
	for i, order := range orders {
		op, _ := order.Operation().Get()
		id, _ := op.AccountID().Get()
		accountHandles[i] = uint64(id.Handle())
	}

	ctx := context.Background()
	var rejectCount atomic.Int64
	var wg sync.WaitGroup

	for accIdx, order := range orders {
		for s := 0; s < submittersPerAccount; s++ {
			wg.Add(1)
			go func(acc int, ord model.Order) {
				defer wg.Done()
				for j := 0; j < perSubmitter; j++ {
					request, _, err := asyncEngine.StartPreTrade(
						ctx, ord,
					).Await(ctx)
					if err != nil {
						t.Errorf("acc=%d call=%d StartPreTrade Await() error = %v", acc, j, err)
						return
					}
					if request == nil {
						// Policy rejected - not an error for this test.
						rejectCount.Add(1)
						continue
					}
					reservation, _, err := request.Execute(ctx).Await(ctx)
					if err != nil {
						t.Errorf("acc=%d call=%d Execute Await() error = %v", acc, j, err)
						return
					}
					if reservation == nil {
						// Policy rejected at main stage.
						rejectCount.Add(1)
						continue
					}
					if _, err := reservation.CommitAndClose(
						ctx,
					).Await(ctx); err != nil {
						t.Errorf(
							"acc=%d call=%d CommitAndClose Await() error = %v",
							acc, j, err,
						)
						return
					}
				}
			}(accIdx, order)
		}
	}
	wg.Wait()

	if err := asyncEngine.StopGraceful(ctx); err != nil {
		t.Fatalf("StopGraceful() error = %v", err)
	}
	engine.Stop()

	// Per-account serialization must hold: the probe must never have seen
	// two policy calls for the same account running concurrently.
	for i, handle := range accountHandles {
		if peak := probe.peakFor(handle); peak > 1 {
			t.Errorf(
				"account index %d: peak concurrent policy calls = %d, want <= 1",
				i, peak,
			)
		}
	}
}
