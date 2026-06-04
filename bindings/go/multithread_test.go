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

package openpit

import (
	"context"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"go.openpit.dev/openpit/asyncengine"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/future"
	"go.openpit.dev/openpit/pretrade/policies"
	"go.openpit.dev/openpit/reject"
)

const multithreadGoroutines = 8
const multithreadPerGoroutine = 1_000
const multithreadAccounts = 8

// multithreadTestOrder builds a minimal order for the given account ID.
// Reuses builtinTestAsset defined in engine_builtin_test.go.
func multithreadTestOrder(t *testing.T, accountID uint64) model.Order {
	t.Helper()
	underlying := builtinTestAsset(t, "AAPL")
	settlement := builtinTestAsset(t, "USD")
	order := model.NewOrder()
	op := order.EnsureOperationView()
	op.SetInstrument(param.NewInstrument(underlying, settlement))
	op.SetAccountID(param.NewAccountIDFromUint64(accountID))
	op.SetSide(param.SideBuy)
	qty, err := param.NewQuantityFromString("1")
	if err != nil {
		t.Fatalf("NewQuantityFromString() error = %v", err)
	}
	op.SetTradeAmount(param.NewQuantityTradeAmount(qty))
	return order
}

func buildFullSyncRateLimitEngine(t *testing.T, maxOrders uint) *Engine {
	t.Helper()
	engine, err := NewEngineBuilder().FullSync().
		Builtin(policies.BuildRateLimit().
			BrokerBarrier(policies.RateLimitBrokerBarrier{
				Limit: policies.RateLimit{
					MaxOrders: maxOrders,
					Window:    time.Minute,
				},
			}),
		).Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	return engine
}

func buildFullSyncAccountRateLimitEngine(t *testing.T, maxOrders uint, accounts int) *Engine {
	t.Helper()
	barriers := make([]policies.RateLimitAccountBarrier, accounts)
	for i := 0; i < accounts; i++ {
		barriers[i] = policies.RateLimitAccountBarrier{
			Limit: policies.RateLimit{
				MaxOrders: maxOrders,
				Window:    time.Minute,
			},
			AccountID: param.NewAccountIDFromUint64(uint64(i)),
		}
	}
	engine, err := NewEngineBuilder().FullSync().
		Builtin(policies.BuildRateLimit().AccountBarriers(barriers...)).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	return engine
}

func buildAccountSyncAccountRateLimitEngine(t *testing.T, maxOrders uint, accounts int) *Engine {
	t.Helper()
	barriers := make([]policies.RateLimitAccountBarrier, accounts)
	for i := 0; i < accounts; i++ {
		barriers[i] = policies.RateLimitAccountBarrier{
			Limit: policies.RateLimit{
				MaxOrders: maxOrders,
				Window:    time.Minute,
			},
			AccountID: param.NewAccountIDFromUint64(uint64(i)),
		}
	}
	engine, err := NewEngineBuilder().AccountSync().
		Builtin(policies.BuildRateLimit().AccountBarriers(barriers...)).
		Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	return engine
}

func runConcurrentStartPreTradeLoad(
	t *testing.T,
	engine *Engine,
	orders []model.Order,
	perGoroutine int,
) {
	t.Helper()
	var wg sync.WaitGroup
	for i := range orders {
		wg.Add(1)
		go func(tid int) {
			defer wg.Done()
			for j := 0; j < perGoroutine; j++ {
				req, rejects, err := engine.StartPreTrade(orders[tid])
				if req != nil {
					req.Close()
				}
				if err != nil {
					t.Errorf("goroutine %d call %d: StartPreTrade() error = %v", tid, j, err)
					return
				}
				if len(rejects) != 0 {
					t.Errorf("goroutine %d call %d: unexpected rejects = %v", tid, j, rejects)
					return
				}
			}
		}(i)
	}
	wg.Wait()
}

func runConcurrentUnshardedAccountLoad(
	t *testing.T,
	engine *Engine,
	orders []model.Order,
	totalCalls int,
	callers int,
) {
	t.Helper()
	var next atomic.Int64
	var wg sync.WaitGroup
	for i := 0; i < callers; i++ {
		wg.Add(1)
		go func(callerID int) {
			defer wg.Done()
			for {
				call := int(next.Add(1) - 1)
				if call >= totalCalls {
					return
				}
				accountIndex := call % len(orders)
				req, rejects, err := engine.StartPreTrade(orders[accountIndex])
				if req != nil {
					req.Close()
				}
				if err != nil {
					t.Errorf(
						"caller %d call %d: StartPreTrade() error = %v",
						callerID, call, err,
					)
					return
				}
				if len(rejects) != 0 {
					t.Errorf(
						"caller %d call %d: unexpected rejects = %v",
						callerID, call, rejects,
					)
					return
				}
			}
		}(i)
	}
	wg.Wait()
}

type accountSyncTask struct {
	order        model.Order
	call         int
	accountIndex int
}

func runAccountShardedStartPreTradeLoad(
	t *testing.T,
	engine *Engine,
	orders []model.Order,
	perAccount int,
	shards int,
) {
	t.Helper()
	workers := make([]chan accountSyncTask, shards)
	var wg sync.WaitGroup
	for i := range workers {
		ch := make(chan accountSyncTask, 1024)
		workers[i] = ch
		wg.Add(1)
		go func(workerID int, ch <-chan accountSyncTask) {
			defer wg.Done()
			for task := range ch {
				req, rejects, err := engine.StartPreTrade(task.order)
				if req != nil {
					req.Close()
				}
				if err != nil {
					t.Errorf(
						"worker %d account %d call %d: StartPreTrade() error = %v",
						workerID, task.accountIndex, task.call, err,
					)
					continue
				}
				if len(rejects) != 0 {
					t.Errorf(
						"worker %d account %d call %d: unexpected rejects = %v",
						workerID, task.accountIndex, task.call, rejects,
					)
					continue
				}
			}
		}(i, ch)
	}

	for accountIndex, order := range orders {
		shard := accountIndex % shards
		for call := 0; call < perAccount; call++ {
			workers[shard] <- accountSyncTask{
				order:        order,
				call:         call,
				accountIndex: accountIndex,
			}
		}
	}
	for _, ch := range workers {
		close(ch)
	}
	wg.Wait()
}

func assertRateLimitProbeRejects(t *testing.T, engine *Engine, accountID uint64) {
	t.Helper()
	_, rejects, err := engine.StartPreTrade(multithreadTestOrder(t, accountID))
	if err != nil {
		t.Fatalf("probe StartPreTrade() error = %v", err)
	}
	if len(rejects) == 0 {
		t.Fatal("probe must reject")
	}
	if rejects[0].Code != reject.CodeRateLimitExceeded {
		t.Fatalf(
			"reject code = %v, want %v",
			rejects[0].Code, reject.CodeRateLimitExceeded,
		)
	}
}

func TestEngineFullSyncConcurrentStartPreTradeIsSafe(t *testing.T) {
	const totalGoroutines = multithreadGoroutines
	const perGoroutine = multithreadPerGoroutine
	const totalCalls = totalGoroutines * perGoroutine

	engine := buildFullSyncRateLimitEngine(t, uint(totalCalls))
	defer engine.Stop()

	orders := make([]model.Order, totalGoroutines)
	for i := 0; i < totalGoroutines; i++ {
		orders[i] = multithreadTestOrder(t, uint64(i))
	}

	runConcurrentStartPreTradeLoad(t, engine, orders, perGoroutine)
	assertRateLimitProbeRejects(t, engine, 0)
}

func TestEngineFullSyncConcurrentAccountRateLimitIsSafe(t *testing.T) {
	const totalCallers = 16
	const perAccount = 1_000
	const totalCalls = multithreadAccounts * perAccount

	engine := buildFullSyncAccountRateLimitEngine(t, uint(perAccount), multithreadAccounts)
	defer engine.Stop()

	orders := make([]model.Order, multithreadAccounts)
	for i := 0; i < multithreadAccounts; i++ {
		orders[i] = multithreadTestOrder(t, uint64(i))
	}

	runConcurrentUnshardedAccountLoad(t, engine, orders, totalCalls, totalCallers)
	for i := 0; i < multithreadAccounts; i++ {
		assertRateLimitProbeRejects(t, engine, uint64(i))
	}
}

// TestRateLimitAccountSyncConcurrentLoad verifies correct behavior under the
// AccountSync engine configuration when the client routes orders through
// account-sharded queues: calls for the same account are sequential, while
// different shards invoke the same engine concurrently.
//
// The dispatch is hand-rolled here (one channel per shard) to keep the
// test independent of the asyncengine helper. Production code is free to
// use the same pattern, but the bundled asyncengine subpackage already
// implements it with idle cleanup, observability hooks, and graceful or
// hard stop modes - see TestRateLimitAccountSyncConcurrentLoadViaAsyncEngine
// for the equivalent flow that delegates dispatch to asyncengine.
func TestRateLimitAccountSyncConcurrentLoad(t *testing.T) {
	const shards = 4
	const perAccount = 1_000

	engine := buildAccountSyncAccountRateLimitEngine(t, uint(perAccount), multithreadAccounts)
	defer engine.Stop()

	orders := make([]model.Order, multithreadAccounts)
	for i := 0; i < multithreadAccounts; i++ {
		orders[i] = multithreadTestOrder(t, uint64(i))
	}

	runAccountShardedStartPreTradeLoad(t, engine, orders, perAccount, shards)
	for i := 0; i < multithreadAccounts; i++ {
		assertRateLimitProbeRejects(t, engine, uint64(i))
	}
}

// TestRateLimitAccountSyncConcurrentLoadViaAsyncEngine is the same scenario
// as TestRateLimitAccountSyncConcurrentLoad but driven through the
// asyncengine helper. Compared with the hand-rolled sharded test, the
// dispatch, lifecycle management, and per-account serialization are all
// delegated to asyncengine.AsyncEngine.
func TestRateLimitAccountSyncConcurrentLoadViaAsyncEngine(t *testing.T) {
	const perAccount = 1_000

	engine := buildAccountSyncAccountRateLimitEngine(
		t, uint(perAccount), multithreadAccounts,
	)
	asyncEngine, err := asyncengine.NewBuilder(engine).Sharded(4).Build()
	if err != nil {
		t.Fatalf("Sharded.Build() error = %v", err)
	}

	orders := make([]model.Order, multithreadAccounts)
	for i := 0; i < multithreadAccounts; i++ {
		orders[i] = multithreadTestOrder(t, uint64(i))
	}

	runAsyncStartPreTradeLoad(t, asyncEngine, orders, perAccount)
	for i := 0; i < multithreadAccounts; i++ {
		assertAsyncRateLimitProbeRejects(t, asyncEngine, uint64(i))
	}

	if err := asyncEngine.StopGraceful(context.Background()); err != nil {
		t.Fatalf("StopGraceful() error = %v", err)
	}
	engine.Stop()
}

// runAsyncStartPreTradeLoad submits perAccount start-stage calls per order
// through the AsyncEngine in parallel and awaits every future.
func runAsyncStartPreTradeLoad(
	t *testing.T,
	async *asyncengine.AsyncEngine,
	orders []model.Order,
	perAccount int,
) {
	t.Helper()
	type pending struct {
		fut  *future.Future2[*asyncengine.AsyncRequest, []reject.Reject]
		acc  int
		call int
	}
	pendings := make([]pending, 0, len(orders)*perAccount)
	for accIdx, order := range orders {
		for call := 0; call < perAccount; call++ {
			pendings = append(pendings, pending{
				fut:  async.StartPreTrade(context.Background(), order),
				acc:  accIdx,
				call: call,
			})
		}
	}
	for _, p := range pendings {
		request, rejects, err := p.fut.Await(context.Background())
		if err != nil {
			t.Errorf(
				"acc=%d call=%d Await() error = %v", p.acc, p.call, err,
			)
			continue
		}
		if len(rejects) != 0 {
			t.Errorf(
				"acc=%d call=%d unexpected rejects = %v",
				p.acc, p.call, rejects,
			)
			continue
		}
		if request != nil {
			if _, err := request.Close(
				context.Background(),
			).Await(context.Background()); err != nil {
				t.Errorf(
					"acc=%d call=%d Close() error = %v",
					p.acc, p.call, err,
				)
			}
		}
	}
}

// assertAsyncRateLimitProbeRejects probes the rate-limit policy through the
// AsyncEngine, mirroring assertRateLimitProbeRejects.
func assertAsyncRateLimitProbeRejects(
	t *testing.T, async *asyncengine.AsyncEngine, accountID uint64,
) {
	t.Helper()
	_, rejects, err := async.StartPreTrade(
		context.Background(), multithreadTestOrder(t, accountID),
	).Await(context.Background())
	if err != nil {
		t.Fatalf("probe StartPreTrade() error = %v", err)
	}
	if len(rejects) == 0 {
		t.Fatal("probe must reject")
	}
	if rejects[0].Code != reject.CodeRateLimitExceeded {
		t.Fatalf(
			"reject code = %v, want %v",
			rejects[0].Code, reject.CodeRateLimitExceeded,
		)
	}
}
