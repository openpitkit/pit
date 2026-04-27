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

package pit

import (
	"testing"

	"github.com/openpitkit/pit-go/model"
	"github.com/openpitkit/pit-go/param"
	"github.com/openpitkit/pit-go/pretrade"
	"github.com/openpitkit/pit-go/reject"
	"github.com/openpitkit/pit-go/tx"
)

func TestMutationsNativeE2E_CommitAndRollbackCallbacks(t *testing.T) {
	policy := &mutationTrackingPolicy{name: "mutation-tracking"}
	engine := newEngineWithPreTradePolicyForNativeE2E(t, policy)
	defer engine.Stop()

	reservation, rejects, err := engine.ExecutePreTrade(newValidOrderForNativeE2E(t))
	if err != nil {
		t.Fatalf("ExecutePreTrade() first error = %v", err)
	}
	if rejects != nil {
		t.Fatalf("ExecutePreTrade() first rejects = %v, want nil", rejects)
	}
	if reservation == nil {
		t.Fatal("ExecutePreTrade() first reservation = nil, want non-nil")
	}
	reservation.CommitAndClose()

	if policy.commitCalls != 1 {
		t.Fatalf("commitCalls after first reservation = %d, want 1", policy.commitCalls)
	}
	if policy.rollbackCalls != 0 {
		t.Fatalf("rollbackCalls after first reservation = %d, want 0", policy.rollbackCalls)
	}

	secondReservation, secondRejects, err := engine.ExecutePreTrade(newValidOrderForNativeE2E(t))
	if err != nil {
		t.Fatalf("ExecutePreTrade() second error = %v", err)
	}
	if secondRejects != nil {
		t.Fatalf("ExecutePreTrade() second rejects = %v, want nil", secondRejects)
	}
	if secondReservation == nil {
		t.Fatal("ExecutePreTrade() second reservation = nil, want non-nil")
	}
	secondReservation.RollbackAndClose()

	if policy.commitCalls != 1 {
		t.Fatalf("commitCalls after second reservation = %d, want 1", policy.commitCalls)
	}
	if policy.rollbackCalls != 1 {
		t.Fatalf("rollbackCalls after second reservation = %d, want 1", policy.rollbackCalls)
	}
}

func TestMutationsNativeE2E_RejectingPolicyReturnsRejectList(t *testing.T) {
	engine := newEngineWithPreTradePolicyForNativeE2E(
		t,
		&mutationTrackingPolicy{
			name:         "mutation-reject",
			shouldReject: true,
		},
	)
	defer engine.Stop()

	reservation, rejects, err := engine.ExecutePreTrade(newValidOrderForNativeE2E(t))
	if err != nil {
		t.Fatalf("ExecutePreTrade() error = %v", err)
	}
	if reservation != nil {
		reservation.Close()
		t.Fatal("ExecutePreTrade() reservation != nil, want nil")
	}
	if rejects == nil {
		t.Fatal("ExecutePreTrade() rejects = nil, want non-nil")
	}
	if len(rejects) != 1 {
		t.Fatalf("ExecutePreTrade() rejects len = %d, want 1", len(rejects))
	}
	if rejects[0].Policy != "mutation-reject" {
		t.Fatalf("reject policy = %q, want %q", rejects[0].Policy, "mutation-reject")
	}
}

type mutationTrackingPolicy struct {
	name          string
	commitCalls   int
	rollbackCalls int
	shouldReject  bool
}

func (p *mutationTrackingPolicy) Close() {}

func (p *mutationTrackingPolicy) Name() string {
	return p.name
}

func (p *mutationTrackingPolicy) PerformPreTradeCheck(
	_ pretrade.Context,
	_ model.Order,
	mutations tx.Mutations,
) reject.List {
	if p.shouldReject {
		return reject.NewSingleItemList(
			reject.CodeOther,
			p.name,
			"forced reject",
			"forced in test",
			reject.ScopeOrder,
		)
	}
	if err := mutations.Push(
		func() { p.commitCalls++ },
		func() { p.rollbackCalls++ },
	); err != nil {
		return reject.NewSingleItemList(
			reject.CodeOther,
			p.name,
			"mutation registration failed",
			err.Error(),
			reject.ScopeOrder,
		)
	}
	return nil
}

func (p *mutationTrackingPolicy) ApplyExecutionReport(model.ExecutionReport) bool {
	return false
}

func newEngineWithPreTradePolicyForNativeE2E(
	t *testing.T,
	policy pretrade.PreTradePolicy,
) *Engine {
	t.Helper()

	builder, err := NewEngineBuilder()
	if err != nil {
		t.Fatalf("NewEngineBuilder() error = %v", err)
	}
	builder.PreTradePolicy(policy)

	engine, err := builder.Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	return engine
}

func newValidOrderForNativeE2E(t *testing.T) model.Order {
	t.Helper()

	order := model.NewOrder()
	operation := order.EnsureOperationView()
	operation.SetInstrument(param.NewInstrument(param.NewAsset("AAPL"), param.NewAsset("USD")))
	operation.SetAccountID(param.NewAccountIDFromInt(1001))
	operation.SetSide(param.SideBuy)
	operation.SetTradeAmount(param.NewQuantityTradeAmount(mustOrderNativeQuantity(t, "1")))
	operation.SetPrice(mustOrderNativePrice(t, "100"))
	return order
}
