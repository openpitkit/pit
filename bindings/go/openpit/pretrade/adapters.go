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

// Adapter wrappers for client-defined policy types.
//
// This file demonstrates how to bridge client order/report payload types to
// openpit policy contracts with explicit cast strategy selection.
package pretrade

import (
	"fmt"

	"github.com/openpitkit/pit/bindings/go/openpit"
)

// StartPolicyAdapter is start-stage adapter contract for client-specific types.
//
// Why this adapter exists:
// - lets client policy keep domain-native payload types;
// - bridges callbacks to openpit interfaces;
// - centralizes cast strategy in one explicit wrapper.
type StartPolicyAdapter[O openpit.Order, R openpit.ExecutionReport] interface {
	// Name returns stable policy name.
	Name() string

	// CheckPreTradeStart validates one order in start stage.
	CheckPreTradeStart(order O) *Reject

	// ApplyExecutionReport updates policy state after execution report.
	ApplyExecutionReport(report R) bool
}

// PolicyAdapter is main-stage adapter contract for client-specific policy logic.
//
// Why this adapter exists:
// - keeps client main-stage policy API typed to client payloads;
// - bridges Context/PolicyDecision callbacks;
// - defines cast behavior explicitly through selected constructor.
type PolicyAdapter[O openpit.Order, R openpit.ExecutionReport] interface {
	// Name returns stable policy name.
	Name() string

	// PerformPreTradeCheck evaluates one request in main stage.
	PerformPreTradeCheck(order O, context Context, decision *PolicyDecision)

	// ApplyExecutionReport updates policy state after execution report.
	ApplyExecutionReport(report R) bool
}

// Cast strategy has no default on purpose.
//
// Choose wrapper explicitly:
//   - NewStartPolicyAdapterWithSafeSlowArgType:
//     runtime type assertion + deterministic mismatch reject.
//   - NewStartPolicyAdapterWithUnsafeFastArgType:
//     direct assertion, panic on wrong wiring.
//
// SafeSlow for start-stage adapter:
// - validates runtime type with assertion
// - rejects mismatch deterministically
// - one runtime assertion cost per callback
func NewStartPolicyAdapterWithSafeSlowArgType[
	O openpit.Order,
	R openpit.ExecutionReport,
](
	policy StartPolicyAdapter[O, R],
) CheckPreTradeStartPolicy {
	return &safeSlowStartPolicyAdapter[O, R]{m_policy: policy}
}

// UnsafeFast for start-stage adapter:
// - uses direct assertion
// - no mismatch branch
// - wrong wiring panics at runtime
func NewStartPolicyAdapterWithUnsafeFastArgType[
	O openpit.Order,
	R openpit.ExecutionReport,
](
	policy StartPolicyAdapter[O, R],
) CheckPreTradeStartPolicy {
	return &unsafeFastStartPolicyAdapter[O, R]{m_policy: policy}
}

// SafeSlow for main-stage adapter:
// - validates runtime type with assertion
// - pushes deterministic mismatch reject
// - one runtime assertion cost per callback
func NewPolicyAdapterWithSafeSlowArgType[
	O openpit.Order,
	R openpit.ExecutionReport,
](
	policy PolicyAdapter[O, R],
) Policy {
	return &safeSlowPolicyAdapter[O, R]{m_policy: policy}
}

// UnsafeFast for main-stage adapter:
// - uses direct assertion
// - no mismatch branch
// - wrong wiring panics at runtime
func NewPolicyAdapterWithUnsafeFastArgType[
	O openpit.Order,
	R openpit.ExecutionReport,
](
	policy PolicyAdapter[O, R],
) Policy {
	return &unsafeFastPolicyAdapter[O, R]{m_policy: policy}
}

type safeSlowStartPolicyAdapter[O openpit.Order, R openpit.ExecutionReport] struct {
	m_policy StartPolicyAdapter[O, R]
}

func (v *safeSlowStartPolicyAdapter[O, R]) Name() string { return v.m_policy.Name() }

func (v *safeSlowStartPolicyAdapter[O, R]) CheckPreTradeStart(order openpit.Order) *Reject {
	concreteOrder, ok := order.(O)
	if !ok {
		return &Reject{
			Policy:  v.Name(),
			Scope:   RejectScopeOrder,
			Code:    RejectCodeOther,
			Reason:  "order type mismatch",
			Details: fmt.Sprintf("expected %T", *new(O)),
		}
	}
	return v.m_policy.CheckPreTradeStart(concreteOrder)
}

func (v *safeSlowStartPolicyAdapter[O, R]) ApplyExecutionReport(
	report openpit.ExecutionReport,
) bool {
	concreteReport, ok := report.(R)
	if !ok {
		return false
	}
	return v.m_policy.ApplyExecutionReport(concreteReport)
}

type unsafeFastStartPolicyAdapter[O openpit.Order, R openpit.ExecutionReport] struct {
	m_policy StartPolicyAdapter[O, R]
}

func (v *unsafeFastStartPolicyAdapter[O, R]) Name() string { return v.m_policy.Name() }

func (v *unsafeFastStartPolicyAdapter[O, R]) CheckPreTradeStart(order openpit.Order) *Reject {
	concreteOrder := order.(O)
	return v.m_policy.CheckPreTradeStart(concreteOrder)
}

func (v *unsafeFastStartPolicyAdapter[O, R]) ApplyExecutionReport(
	report openpit.ExecutionReport,
) bool {
	concreteReport := report.(R)
	return v.m_policy.ApplyExecutionReport(concreteReport)
}

type safeSlowPolicyAdapter[O openpit.Order, R openpit.ExecutionReport] struct {
	m_policy PolicyAdapter[O, R]
}

func (v *safeSlowPolicyAdapter[O, R]) Name() string { return v.m_policy.Name() }

func (v *safeSlowPolicyAdapter[O, R]) PerformPreTradeCheck(
	context Context,
	decision *PolicyDecision,
) {
	concreteOrder, ok := context.Order().(O)
	if !ok {
		decision.Rejects = append(decision.Rejects, Reject{
			Policy:  v.Name(),
			Scope:   RejectScopeOrder,
			Code:    RejectCodeOther,
			Reason:  "order type mismatch",
			Details: fmt.Sprintf("expected %T", *new(O)),
		})
		return
	}
	v.m_policy.PerformPreTradeCheck(concreteOrder, context, decision)
}

func (v *safeSlowPolicyAdapter[O, R]) ApplyExecutionReport(report openpit.ExecutionReport) bool {
	concreteReport, ok := report.(R)
	if !ok {
		return false
	}
	return v.m_policy.ApplyExecutionReport(concreteReport)
}

type unsafeFastPolicyAdapter[O openpit.Order, R openpit.ExecutionReport] struct {
	m_policy PolicyAdapter[O, R]
}

func (v *unsafeFastPolicyAdapter[O, R]) Name() string { return v.m_policy.Name() }

func (v *unsafeFastPolicyAdapter[O, R]) PerformPreTradeCheck(
	context Context,
	decision *PolicyDecision,
) {
	concreteOrder := context.Order().(O)
	v.m_policy.PerformPreTradeCheck(concreteOrder, context, decision)
}

func (v *unsafeFastPolicyAdapter[O, R]) ApplyExecutionReport(
	report openpit.ExecutionReport,
) bool {
	concreteReport := report.(R)
	return v.m_policy.ApplyExecutionReport(concreteReport)
}
