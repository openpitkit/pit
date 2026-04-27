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

package accountadjustment

import (
	"runtime/cgo"
	"testing"

	"github.com/openpitkit/pit-go/internal/callback"
	"github.com/openpitkit/pit-go/internal/native"
	"github.com/openpitkit/pit-go/model"
	"github.com/openpitkit/pit-go/param"
	"github.com/openpitkit/pit-go/reject"
	"github.com/openpitkit/pit-go/tx"
)

type clientPayloadTestAdjustment struct {
	model.AccountAdjustment
	Source string
}

func TestSafeClientPolicyRejectsMissingAdjustmentPayload(t *testing.T) {
	wrapped := NewSafeClientPolicy(&clientPayloadTestPolicy{})

	rejects := wrapped.ApplyAccountAdjustment(
		Context{},
		param.NewAccountIDFromInt(1),
		model.NewAccountAdjustment(),
		tx.Mutations{},
	)
	if len(rejects) != 1 {
		t.Fatalf("ApplyAccountAdjustment() reject len = %d, want 1", len(rejects))
	}
	if rejects[0].Code != reject.CodeOther {
		t.Fatalf("reject code = %v, want %v", rejects[0].Code, reject.CodeOther)
	}
	if rejects[0].Scope != reject.ScopeAccount {
		t.Fatalf("reject scope = %v, want %v", rejects[0].Scope, reject.ScopeAccount)
	}
}

func TestSafeClientPolicyCastsAdjustmentPayload(t *testing.T) {
	policy := &clientPayloadTestPolicy{}
	wrapped := NewSafeClientPolicy(policy)
	adjustment := clientPayloadTestAdjustment{
		AccountAdjustment: model.NewAccountAdjustment(),
		Source:            "reconciliation",
	}

	rejects := wrapped.ApplyAccountAdjustment(
		Context{},
		param.NewAccountIDFromInt(1),
		adjustmentWithPayload(t, adjustment),
		tx.Mutations{},
	)
	if len(rejects) != 0 {
		t.Fatalf("ApplyAccountAdjustment() rejects = %v, want none", rejects)
	}
	if policy.adjustment.Source != adjustment.Source {
		t.Fatalf("adjustment source = %q, want %q", policy.adjustment.Source, adjustment.Source)
	}
}

func TestUnsafeFastClientPolicyCastsAdjustmentPayload(t *testing.T) {
	policy := &clientPayloadTestPolicy{}
	wrapped := NewUnsafeFastClientPolicy(policy)
	adjustment := clientPayloadTestAdjustment{
		AccountAdjustment: model.NewAccountAdjustment(),
		Source:            "fast-reconciliation",
	}

	rejects := wrapped.ApplyAccountAdjustment(
		Context{},
		param.NewAccountIDFromInt(1),
		adjustmentWithPayload(t, adjustment),
		tx.Mutations{},
	)
	if len(rejects) != 0 {
		t.Fatalf("ApplyAccountAdjustment() rejects = %v, want none", rejects)
	}
	if policy.adjustment.Source != adjustment.Source {
		t.Fatalf("adjustment source = %q, want %q", policy.adjustment.Source, adjustment.Source)
	}
}

type clientPayloadTestPolicy struct {
	adjustment clientPayloadTestAdjustment
	closeCalls int
}

func (p *clientPayloadTestPolicy) Close() { p.closeCalls++ }

func (p *clientPayloadTestPolicy) Name() string {
	return "client-payload-test"
}

func (p *clientPayloadTestPolicy) ApplyAccountAdjustment(
	_ Context,
	_ param.AccountID,
	adjustment clientPayloadTestAdjustment,
	_ tx.Mutations,
) reject.List {
	p.adjustment = adjustment
	return nil
}

func TestSafeClientPolicyName(t *testing.T) {
	policy := &clientPayloadTestPolicy{}
	wrapped := NewSafeClientPolicy(policy)
	if got := wrapped.Name(); got != policy.Name() {
		t.Fatalf("Name() = %q, want %q", got, policy.Name())
	}
}

func TestSafeClientPolicyClose(t *testing.T) {
	policy := &clientPayloadTestPolicy{}
	wrapped := NewSafeClientPolicy(policy)
	wrapped.Close()
	if policy.closeCalls != 1 {
		t.Fatalf("closeCalls = %d, want 1", policy.closeCalls)
	}
}

func TestUnsafeFastClientPolicyName(t *testing.T) {
	policy := &clientPayloadTestPolicy{}
	wrapped := NewUnsafeFastClientPolicy(policy)
	if got := wrapped.Name(); got != policy.Name() {
		t.Fatalf("Name() = %q, want %q", got, policy.Name())
	}
}

func TestUnsafeFastClientPolicyClose(t *testing.T) {
	policy := &clientPayloadTestPolicy{}
	wrapped := NewUnsafeFastClientPolicy(policy)
	wrapped.Close()
	if policy.closeCalls != 1 {
		t.Fatalf("closeCalls = %d, want 1", policy.closeCalls)
	}
}

func TestSafeAdjustmentPayloadReturnsFalseForInvalidHandlePointer(t *testing.T) {
	handle := cgo.NewHandle(42)
	userData := callback.NewUserDataFromHandle(handle)
	handle.Delete()

	nativeAdjustment := native.NewAccountAdjustment()
	native.AccountAdjustmentSetUserData(&nativeAdjustment, userData)

	adjustment, ok := safeAdjustmentPayload[clientPayloadTestAdjustment](
		model.NewAccountAdjustmentFromNative(nativeAdjustment),
	)
	if ok {
		t.Fatal("safeAdjustmentPayload() ok = true, want false")
	}
	if adjustment.Source != "" {
		t.Fatalf("safeAdjustmentPayload() source = %q, want empty", adjustment.Source)
	}
}

func adjustmentWithPayload(
	t *testing.T,
	adjustment clientPayloadTestAdjustment,
) model.AccountAdjustment {
	t.Helper()

	nativeAdjustment := adjustment.EngineAccountAdjustment().Native()
	handle := cgo.NewHandle(adjustment)
	t.Cleanup(handle.Delete)
	native.AccountAdjustmentSetUserData(
		&nativeAdjustment,
		callback.NewUserDataFromHandle(handle),
	)
	return model.NewAccountAdjustmentFromNative(nativeAdjustment)
}
