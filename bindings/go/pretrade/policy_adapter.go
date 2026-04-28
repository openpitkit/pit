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

package pretrade

import (
	"fmt"
	"unsafe"

	"go.openpit.dev/openpit/internal/callback"
	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/reject"
	"go.openpit.dev/openpit/tx"
)

// ClientOrder is the client-owned order shape accepted by ClientEngine.
//
// EngineOrder returns the standard order view used by the native engine. The
// original client value is carried separately as callback payload.
type ClientOrder interface {
	EngineOrder() model.Order
}

// ClientExecutionReport is the client-owned execution report shape accepted by
// ClientEngine.
//
// EngineExecutionReport returns the standard report view used by the native
// engine. The original client value is carried separately as callback payload.
type ClientExecutionReport interface {
	EngineExecutionReport() model.ExecutionReport
}

// ClientCheckPreTradeStartPolicy is a start-stage policy written against
// client-owned order and execution report types.
type ClientCheckPreTradeStartPolicy[Order ClientOrder, Report ClientExecutionReport] interface {
	Close()
	Name() string
	CheckPreTradeStart(Context, Order) []reject.Reject
	ApplyExecutionReport(Report) bool
}

// ClientPreTradePolicy is a main pre-trade policy written against client-owned
// order and execution report types.
type ClientPreTradePolicy[Order ClientOrder, Report ClientExecutionReport] interface {
	Close()
	Name() string
	PerformPreTradeCheck(Context, Order, tx.Mutations) []reject.Reject
	ApplyExecutionReport(Report) bool
}

// NewSafeClientCheckPreTradeStartPolicy adapts a client typed start policy to
// the standard policy interface with payload validation.
//
// Missing or mismatched order payloads become an order-scoped reject. Missing
// or mismatched report payloads return false.
func NewSafeClientCheckPreTradeStartPolicy[
	Order ClientOrder,
	Report ClientExecutionReport,
](
	policy ClientCheckPreTradeStartPolicy[Order, Report],
) CheckStartPolicy {
	return &safeClientCheckPreTradeStartPolicy[Order, Report]{policy: policy}
}

// NewUnsafeFastClientCheckPreTradeStartPolicy adapts a client typed start
// policy without payload validation.
//
// It is intended for SDK-controlled paths such as ClientEngine. A missing or
// wrong payload panics.
func NewUnsafeFastClientCheckPreTradeStartPolicy[
	Order ClientOrder,
	Report ClientExecutionReport,
](
	policy ClientCheckPreTradeStartPolicy[Order, Report],
) CheckStartPolicy {
	return &unsafeFastClientCheckPreTradeStartPolicy[Order, Report]{policy: policy}
}

// NewSafeClientPreTradePolicy adapts a client typed main pre-trade policy to
// the standard policy interface with payload validation.
//
// Missing or mismatched order payloads become an order-scoped reject. Missing
// or mismatched report payloads return false.
func NewSafeClientPreTradePolicy[
	Order ClientOrder,
	Report ClientExecutionReport,
](
	policy ClientPreTradePolicy[Order, Report],
) Policy {
	return &safeClientPreTradePolicy[Order, Report]{policy: policy}
}

// NewUnsafeFastClientPreTradePolicy adapts a client typed main pre-trade policy
// without payload validation.
//
// It is intended for SDK-controlled paths such as ClientEngine. A missing or
// wrong payload panics.
func NewUnsafeFastClientPreTradePolicy[
	Order ClientOrder,
	Report ClientExecutionReport,
](
	policy ClientPreTradePolicy[Order, Report],
) Policy {
	return &unsafeFastClientPreTradePolicy[Order, Report]{policy: policy}
}

type safeClientCheckPreTradeStartPolicy[
	Order ClientOrder,
	Report ClientExecutionReport,
] struct {
	policy ClientCheckPreTradeStartPolicy[Order, Report]
}

func (p *safeClientCheckPreTradeStartPolicy[Order, Report]) Close() {
	p.policy.Close()
}

func (p *safeClientCheckPreTradeStartPolicy[Order, Report]) Name() string {
	return p.policy.Name()
}

func (p *safeClientCheckPreTradeStartPolicy[Order, Report]) CheckPreTradeStart(
	ctx Context,
	engineOrder model.Order,
) []reject.Reject {
	order, ok := safeOrderPayload[Order](engineOrder)
	if !ok {
		return clientPayloadMismatchReject[Order](p.Name())
	}
	return p.policy.CheckPreTradeStart(ctx, order)
}

func (p *safeClientCheckPreTradeStartPolicy[Order, Report]) ApplyExecutionReport(
	engineReport model.ExecutionReport,
) bool {
	report, ok := safeReportPayload[Report](engineReport)
	if !ok {
		return false
	}
	return p.policy.ApplyExecutionReport(report)
}

type safeClientPreTradePolicy[
	Order ClientOrder,
	Report ClientExecutionReport,
] struct {
	policy ClientPreTradePolicy[Order, Report]
}

func (p *safeClientPreTradePolicy[Order, Report]) Close() {
	p.policy.Close()
}

func (p *safeClientPreTradePolicy[Order, Report]) Name() string {
	return p.policy.Name()
}

func (p *safeClientPreTradePolicy[Order, Report]) PerformPreTradeCheck(
	ctx Context,
	engineOrder model.Order,
	mutations tx.Mutations,
) []reject.Reject {
	order, ok := safeOrderPayload[Order](engineOrder)
	if !ok {
		return clientPayloadMismatchReject[Order](p.Name())
	}
	return p.policy.PerformPreTradeCheck(ctx, order, mutations)
}

func (p *safeClientPreTradePolicy[Order, Report]) ApplyExecutionReport(
	engineReport model.ExecutionReport,
) bool {
	report, ok := safeReportPayload[Report](engineReport)
	if !ok {
		return false
	}
	return p.policy.ApplyExecutionReport(report)
}

type unsafeFastClientCheckPreTradeStartPolicy[
	Order ClientOrder,
	Report ClientExecutionReport,
] struct {
	policy ClientCheckPreTradeStartPolicy[Order, Report]
}

func (p *unsafeFastClientCheckPreTradeStartPolicy[Order, Report]) Close() {
	p.policy.Close()
}

func (p *unsafeFastClientCheckPreTradeStartPolicy[Order, Report]) Name() string {
	return p.policy.Name()
}

func (p *unsafeFastClientCheckPreTradeStartPolicy[Order, Report]) CheckPreTradeStart(
	ctx Context,
	engineOrder model.Order,
) []reject.Reject {
	return p.policy.CheckPreTradeStart(ctx, unsafeFastOrderPayload[Order](engineOrder))
}

func (p *unsafeFastClientCheckPreTradeStartPolicy[Order, Report]) ApplyExecutionReport(
	engineReport model.ExecutionReport,
) bool {
	return p.policy.ApplyExecutionReport(unsafeFastReportPayload[Report](engineReport))
}

type unsafeFastClientPreTradePolicy[
	Order ClientOrder,
	Report ClientExecutionReport,
] struct {
	policy ClientPreTradePolicy[Order, Report]
}

func (p *unsafeFastClientPreTradePolicy[Order, Report]) Close() {
	p.policy.Close()
}

func (p *unsafeFastClientPreTradePolicy[Order, Report]) Name() string {
	return p.policy.Name()
}

func (p *unsafeFastClientPreTradePolicy[Order, Report]) PerformPreTradeCheck(
	ctx Context,
	engineOrder model.Order,
	mutations tx.Mutations,
) []reject.Reject {
	return p.policy.PerformPreTradeCheck(ctx, unsafeFastOrderPayload[Order](engineOrder), mutations)
}

func (p *unsafeFastClientPreTradePolicy[Order, Report]) ApplyExecutionReport(
	engineReport model.ExecutionReport,
) bool {
	return p.policy.ApplyExecutionReport(unsafeFastReportPayload[Report](engineReport))
}

func safeOrderPayload[Order ClientOrder](order model.Order) (value Order, ok bool) {
	return safePayload[Order](native.OrderGetUserData(order.Handle()))
}

func safeReportPayload[Report ClientExecutionReport](
	report model.ExecutionReport,
) (value Report, ok bool) {
	return safePayload[Report](native.ExecutionReportGetUserData(report.Handle()))
}

func safePayload[Payload any](userData unsafe.Pointer) (value Payload, ok bool) {
	if userData == nil {
		return value, false
	}
	defer func() {
		if recover() != nil {
			var zero Payload
			value = zero
			ok = false
		}
	}()
	payload := callback.NewHandleFromUserData(userData).Value()
	value, ok = payload.(Payload)
	return value, ok
}

func unsafeFastOrderPayload[Order ClientOrder](order model.Order) Order {
	return unsafeFastPayload[Order](native.OrderGetUserData(order.Handle()))
}

func unsafeFastReportPayload[Report ClientExecutionReport](report model.ExecutionReport) Report {
	return unsafeFastPayload[Report](native.ExecutionReportGetUserData(report.Handle()))
}

func unsafeFastPayload[Payload any](userData unsafe.Pointer) Payload {
	return callback.NewHandleFromUserData(userData).Value().(Payload)
}

func clientPayloadMismatchReject[Order ClientOrder](policyName string) []reject.Reject {
	return reject.NewSingleItemList(
		reject.CodeOther,
		policyName,
		"client order payload mismatch",
		fmt.Sprintf("expected client order payload type %T", *new(Order)),
		reject.ScopeOrder,
	)
}
