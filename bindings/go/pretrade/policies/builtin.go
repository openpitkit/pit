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
	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/pretrade"
)

var _ pretrade.BuiltinPolicy = (*checkStartPolicy)(nil)
var _ pretrade.BuiltinPolicy = (*policy)(nil)

//------------------------------------------------------------------------------
// CheckStartPolicy

type checkStartPolicy struct {
	handle native.PretradeCheckPreTradeStartPolicy
}

func newCheckStartPolicy(
	newPolicy func() native.PretradeCheckPreTradeStartPolicy,
) *checkStartPolicy {
	return &checkStartPolicy{handle: newPolicy()}
}

func newCheckPreTradeStartPolicyWithError(
	newPolicy func() (native.PretradeCheckPreTradeStartPolicy, error),
) (*checkStartPolicy, error) {
	handle, err := newPolicy()
	if err != nil {
		return &checkStartPolicy{}, err
	}
	return &checkStartPolicy{handle: handle}, nil
}

// Close releases this policy.
//
// If the policy has already been added to an EngineBuilder, the engine
// keeps the policy registered independently; closing the policy does not
// remove the policy from the engine.
//
// Idempotency: safe to call more than once; subsequent calls are no-ops.
func (p *checkStartPolicy) Close() {
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
func (p checkStartPolicy) Name() string {
	p.checkHandle()
	return native.PretradeCheckPreTradeStartPolicyGetName(p.handle).Safe()
}

// TakeHandle returns the native handle and transfers its ownership to the
// caller. After the handle has been taken, the wrapper is considered closed;
// a subsequent Close is a no-op and any other method call panics.
func (p *checkStartPolicy) TakeHandle() native.PretradeCheckPreTradeStartPolicy {
	p.checkHandle()
	result := p.handle
	p.handle = nil
	return result
}

func (p checkStartPolicy) checkHandle() {
	if p.handle == nil {
		panic("built-in policy is already closed")
	}
}

//------------------------------------------------------------------------------
// Policy

type policy struct {
	handle native.PretradePreTradePolicy
}

func newPolicyWithError(newPolicy func() (native.PretradePreTradePolicy, error)) (*policy, error) {
	handle, err := newPolicy()
	if err != nil {
		return &policy{}, err
	}
	return &policy{handle: handle}, nil
}

// Close releases this policy.
//
// If the policy has already been added to an EngineBuilder, the engine
// keeps the policy registered independently; closing the policy does not
// remove the policy from the engine.
//
// Idempotency: safe to call more than once; subsequent calls are no-ops.
func (p *policy) Close() {
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
func (p policy) Name() string {
	p.checkHandle()
	return native.PretradePreTradePolicyGetName(p.handle).Safe()
}

// TakeHandle returns the native handle and transfers its ownership to the
// caller. After the handle has been taken, the wrapper is considered closed;
// a subsequent Close is a no-op and any other method call panics.
func (p *policy) TakeHandle() native.PretradePreTradePolicy {
	p.checkHandle()
	result := p.handle
	p.handle = nil
	return result
}

func (p policy) checkHandle() {
	if p.handle == nil {
		panic("built-in policy is already closed")
	}
}

//------------------------------------------------------------------------------
