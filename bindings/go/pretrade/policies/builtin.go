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

package policies

import (
	"github.com/openpitkit/pit-go/internal/native"
	"github.com/openpitkit/pit-go/model"
	"github.com/openpitkit/pit-go/pretrade"
	"github.com/openpitkit/pit-go/reject"
	"github.com/openpitkit/pit-go/tx"
)

const (
	builtinPolicyOnlyReason  = "built-in policy is engine-managed only"
	builtinPolicyOnlyDetails = "this built-in policy wrapper is intended for engine registration only"
)

//------------------------------------------------------------------------------
// CheckPreTradeStartPolicy

type checkPreTradeStartPolicy struct {
	handle native.PretradeCheckPreTradeStartPolicy
}

func newCheckPreTradeStartPolicy(
	newPolicy func() native.PretradeCheckPreTradeStartPolicy,
) *checkPreTradeStartPolicy {
	return &checkPreTradeStartPolicy{handle: newPolicy()}
}

func newCheckPreTradeStartPolicyWithError(
	newPolicy func() (native.PretradeCheckPreTradeStartPolicy, error),
) (*checkPreTradeStartPolicy, error) {
	handle, err := newPolicy()
	if err != nil {
		return &checkPreTradeStartPolicy{}, err
	}
	return &checkPreTradeStartPolicy{handle: handle}, nil
}

// Close releases this policy.
//
// If the policy has already been added to an EngineBuilder, the engine
// keeps the policy registered independently; closing the policy does not
// remove the policy from the engine.
//
// Idempotency: safe to call more than once; subsequent calls are no-ops.
func (p *checkPreTradeStartPolicy) Close() {
	if p.handle == nil {
		return
	}
	native.DestroyPretradeCheckPreTradeStartPolicy(p.handle)
	p.handle = nil
}

// Name returns stable policy name.
//
// Policy names must be unique across all policies registered in the same
// engine instance.
func (p checkPreTradeStartPolicy) Name() string {
	p.checkHandle()
	return native.PretradeCheckPreTradeStartPolicyGetName(p.handle).Safe()
}

// TakeNative returns the native handle and transfers its ownership to the
// caller. After the handle has been taken, the wrapper is considered closed;
// a subsequent Close is a no-op and any other method call panics.
func (p *checkPreTradeStartPolicy) TakeNative() native.PretradeCheckPreTradeStartPolicy {
	p.checkHandle()
	result := p.handle
	p.handle = nil
	return result
}

// CheckPreTradeStart performs start-stage checks against an order.
//
// Built-in policies are executed by the native runtime inside the engine
// pipeline and are not intended to run through this Go callback path. So
// this always returns a reject.
func (p checkPreTradeStartPolicy) CheckPreTradeStart(
	pretrade.Context,
	model.Order,
) reject.List {
	p.checkHandle()
	return reject.NewSingleItemList(
		reject.CodeOther,
		p.Name(),
		builtinPolicyOnlyReason,
		builtinPolicyOnlyDetails,
		reject.ScopeOrder,
	)
}

// ApplyExecutionReport applies post-trade updates from execution reports.
//
// Built-in policies are executed by the native runtime inside the engine
// pipeline and are not intended to run through this Go callback path. So
// this always returns false.
func (p checkPreTradeStartPolicy) ApplyExecutionReport(model.ExecutionReport) bool {
	p.checkHandle()
	return false
}

func (p checkPreTradeStartPolicy) checkHandle() {
	if p.handle == nil {
		panic("built-in policy is already closed")
	}
}

//------------------------------------------------------------------------------
// PreTradePolicy

type preTradePolicy struct {
	handle native.PretradePreTradePolicy
}

func newPreTradePolicyWithError(
	newPolicy func() (native.PretradePreTradePolicy, error),
) (*preTradePolicy, error) {
	handle, err := newPolicy()
	if err != nil {
		return &preTradePolicy{}, err
	}
	return &preTradePolicy{handle: handle}, nil
}

// Close releases this policy.
//
// If the policy has already been added to an EngineBuilder, the engine
// keeps the policy registered independently; closing the policy does not
// remove the policy from the engine.
//
// Idempotency: safe to call more than once; subsequent calls are no-ops.
func (p *preTradePolicy) Close() {
	if p.handle == nil {
		return
	}
	native.DestroyPretradePreTradePolicy(p.handle)
	p.handle = nil
}

// Name returns stable policy name.
//
// Policy names must be unique across all policies registered in the same
// engine instance.
func (p preTradePolicy) Name() string {
	p.checkHandle()
	return native.PretradePreTradePolicyGetName(p.handle).Safe()
}

// TakeNative returns the native handle and transfers its ownership to the
// caller. After the handle has been taken, the wrapper is considered closed;
// a subsequent Close is a no-op and any other method call panics.
func (p *preTradePolicy) TakeNative() native.PretradePreTradePolicy {
	p.checkHandle()
	result := p.handle
	p.handle = nil
	return result
}

// PerformPreTradeCheck performs main-stage checks and can emit mutations or rejects.
//
// Built-in policies are executed by the native runtime inside the engine
// pipeline and are not intended to run through this Go callback path. So
// this always returns a reject.
func (p preTradePolicy) PerformPreTradeCheck(
	_ pretrade.Context,
	_ model.Order,
	_ tx.Mutations,
) reject.List {
	p.checkHandle()
	return reject.NewSingleItemList(
		reject.CodeOther,
		p.Name(),
		builtinPolicyOnlyReason,
		builtinPolicyOnlyDetails,
		reject.ScopeOrder,
	)
}

// ApplyExecutionReport applies post-trade updates from execution reports.
//
// Built-in policies are executed by the native runtime inside the engine
// pipeline and are not intended to run through this Go callback path. So
// this always returns false.
func (p preTradePolicy) ApplyExecutionReport(model.ExecutionReport) bool {
	p.checkHandle()
	return false
}

func (p preTradePolicy) checkHandle() {
	if p.handle == nil {
		panic("built-in policy is already closed")
	}
}

//------------------------------------------------------------------------------
