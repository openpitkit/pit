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
	"runtime"
	"runtime/cgo"
	"sync"

	"go.openpit.dev/openpit/accountadjustment"
	"go.openpit.dev/openpit/internal/callback"
	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/optional"
	"go.openpit.dev/openpit/pretrade"
	"go.openpit.dev/openpit/reject"
)

type ClientEngineOption func(*clientEngineOptions)

// UnsafeFastClientPayloadCallbacks selects callback adapters that trust every
// client payload reaching client policies to carry the builder's declared type.
//
// This mode removes safe adapter checks from every callback. A missing payload
// or a wrong payload type panics.
func UnsafeFastClientPayloadCallbacks() ClientEngineOption {
	return func(options *clientEngineOptions) {
		options.unsafeFastPayloadCallbacks = true
	}
}

type clientEngineOptions struct {
	unsafeFastPayloadCallbacks bool
}

// ClientEngine runs the standard engine through client-owned order, execution
// report, and account-adjustment types.
//
// The ordinary Engine API remains the zero-payload fast path. ClientEngine is
// the opt-in path that allocates a cgo.Handle per submitted client payload so
// callbacks can receive the original typed value.
//
// Threading: ClientEngine follows the same threading contract as Engine.
// Payload handles allocated by the SDK (cgo.Handle wrapped around the client
// value) are released synchronously inside the same call that created them, so
// callers do not need to extend payload lifetime beyond that call.
type ClientEngine[
	Order pretrade.ClientOrder,
	Report pretrade.ClientExecutionReport,
	Adjustment accountadjustment.ClientAccountAdjustment,
] struct {
	engine *Engine
}

// Stop releases the underlying engine.
func (e *ClientEngine[Order, Report, Adjustment]) Stop() {
	e.engine.Stop()
}

// StartPreTrade runs the start stage with a client order payload.
//
// On accept, the returned ClientRequest owns the payload handle and releases it
// when Execute or Close is called. On reject or error, the handle is released
// before StartPreTrade returns.
func (e *ClientEngine[Order, Report, Adjustment]) StartPreTrade(
	order Order,
) (*ClientRequest, []reject.Reject, error) {
	engineOrder, payload := newClientOrderPayload(order)
	request, rejects, err := e.engine.StartPreTrade(engineOrder)
	runtime.KeepAlive(order)
	if err != nil || rejects != nil {
		payload.release()
		return nil, rejects, err
	}
	return newClientRequest(request, payload), nil, nil
}

// ExecutePreTrade runs the full pre-trade pipeline with a client order payload.
//
// The payload handle is released before ExecutePreTrade returns because all
// order callbacks have completed by then.
func (e *ClientEngine[Order, Report, Adjustment]) ExecutePreTrade(
	order Order,
) (*pretrade.Reservation, []reject.Reject, error) {
	engineOrder, payload := newClientOrderPayload(order)
	defer payload.release()
	reservation, rejects, err := e.engine.ExecutePreTrade(engineOrder)
	runtime.KeepAlive(order)
	return reservation, rejects, err
}

// ApplyExecutionReport applies a client execution report payload.
//
// The payload handle is released before ApplyExecutionReport returns because
// report callbacks are synchronous.
func (e *ClientEngine[Order, Report, Adjustment]) ApplyExecutionReport(
	report Report,
) (PostTradeResult, error) {
	engineReport, payload := newClientReportPayload(report)
	defer payload.release()
	result, err := e.engine.ApplyExecutionReport(engineReport)
	runtime.KeepAlive(report)
	return result, err
}

// ApplyAccountAdjustment applies client account-adjustment payloads.
//
// Payload handles are released before ApplyAccountAdjustment returns because
// account-adjustment callbacks are synchronous.
func (e *ClientEngine[Order, Report, Adjustment]) ApplyAccountAdjustment(
	accountID param.AccountID,
	adjustments []Adjustment,
) (optional.Option[reject.AccountAdjustmentBatchError], error) {
	engineAdjustments, payloads := newClientAdjustmentPayloads(adjustments)
	defer payloads.release()
	rejects, err := e.engine.ApplyAccountAdjustment(accountID, engineAdjustments)
	runtime.KeepAlive(adjustments)
	return rejects, err
}

type ClientEngineBuilder[
	Order pretrade.ClientOrder,
	Report pretrade.ClientExecutionReport,
	Adjustment accountadjustment.ClientAccountAdjustment,
] struct {
	builder                    *EngineBuilder
	unsafeFastPayloadCallbacks bool
}

// NewClientEngineBuilder creates a builder for strategies that use custom
// order, execution report, and account-adjustment types.
//
// Policies added to this builder receive client types directly. The builder
// adapts them to the standard engine policies and keeps payload handles entirely
// inside the SDK boundary.
func NewClientEngineBuilder[
	Order pretrade.ClientOrder,
	Report pretrade.ClientExecutionReport,
	Adjustment accountadjustment.ClientAccountAdjustment,
](options ...ClientEngineOption) (*ClientEngineBuilder[Order, Report, Adjustment], error) {
	builder, err := NewEngineBuilder()
	if err != nil {
		return nil, err
	}

	config := clientEngineOptions{}
	for _, option := range options {
		option(&config)
	}

	return &ClientEngineBuilder[Order, Report, Adjustment]{
		builder:                    builder,
		unsafeFastPayloadCallbacks: config.unsafeFastPayloadCallbacks,
	}, nil
}

// NewClientPreTradeEngineBuilder creates a client builder for custom order and
// execution report types while keeping account adjustments on the standard SDK
// model type.
func NewClientPreTradeEngineBuilder[
	Order pretrade.ClientOrder,
	Report pretrade.ClientExecutionReport,
](
	options ...ClientEngineOption,
) (*ClientEngineBuilder[Order, Report, model.AccountAdjustment], error) {
	return NewClientEngineBuilder[Order, Report, model.AccountAdjustment](options...)
}

// NewClientAccountAdjustmentEngineBuilder creates a client builder for custom
// account-adjustment types while keeping orders and execution reports on the
// standard SDK model types.
func NewClientAccountAdjustmentEngineBuilder[
	Adjustment accountadjustment.ClientAccountAdjustment,
](options ...ClientEngineOption) (
	*ClientEngineBuilder[model.Order, model.ExecutionReport, Adjustment],
	error,
) {
	return NewClientEngineBuilder[model.Order, model.ExecutionReport, Adjustment](options...)
}

// Close releases the underlying builder and any policies it still owns.
func (b *ClientEngineBuilder[Order, Report, Adjustment]) Close() {
	b.builder.Close()
}

// Build constructs a ClientEngine and transfers ownership of policies to it.
func (b *ClientEngineBuilder[Order, Report, Adjustment]) Build() (
	*ClientEngine[Order, Report, Adjustment],
	error,
) {
	engine, err := b.builder.Build()
	if err != nil {
		return nil, err
	}
	return &ClientEngine[Order, Report, Adjustment]{engine: engine}, nil
}

// CheckPreTradeStartPolicy adds client typed start policies.
func (b *ClientEngineBuilder[Order, Report, Adjustment]) CheckPreTradeStartPolicy(
	policy ...pretrade.ClientCheckPreTradeStartPolicy[Order, Report],
) *ClientEngineBuilder[Order, Report, Adjustment] {
	for _, p := range policy {
		if b.unsafeFastPayloadCallbacks {
			b.builder.CheckPreTradeStartPolicy(
				pretrade.NewUnsafeFastClientCheckPreTradeStartPolicy(p),
			)
			continue
		}
		b.builder.CheckPreTradeStartPolicy(pretrade.NewSafeClientCheckPreTradeStartPolicy(p))
	}
	return b
}

// PreTradePolicy adds client typed main pre-trade policies.
func (b *ClientEngineBuilder[Order, Report, Adjustment]) PreTradePolicy(
	policy ...pretrade.ClientPreTradePolicy[Order, Report],
) *ClientEngineBuilder[Order, Report, Adjustment] {
	for _, p := range policy {
		if b.unsafeFastPayloadCallbacks {
			b.builder.PreTradePolicy(pretrade.NewUnsafeFastClientPreTradePolicy(p))
			continue
		}
		b.builder.PreTradePolicy(pretrade.NewSafeClientPreTradePolicy(p))
	}
	return b
}

// AccountAdjustmentPolicy adds client typed account-adjustment policies.
func (b *ClientEngineBuilder[Order, Report, Adjustment]) AccountAdjustmentPolicy(
	policy ...accountadjustment.ClientPolicy[Adjustment],
) *ClientEngineBuilder[Order, Report, Adjustment] {
	for _, p := range policy {
		if b.unsafeFastPayloadCallbacks {
			b.builder.AccountAdjustmentPolicy(accountadjustment.NewUnsafeFastClientPolicy(p))
			continue
		}
		b.builder.AccountAdjustmentPolicy(accountadjustment.NewSafeClientPolicy(p))
	}
	return b
}

// ClientRequest is a deferred pre-trade request that keeps the original client
// order payload alive until the request is executed or closed.
type ClientRequest struct {
	request *pretrade.Request
	payload *clientPayloadHandle
}

func newClientRequest(request *pretrade.Request, payload *clientPayloadHandle) *ClientRequest {
	return &ClientRequest{request: request, payload: payload}
}

// Close releases the request and the client order payload.
func (r *ClientRequest) Close() {
	if r.request != nil {
		r.request.Close()
		r.request = nil
	}
	r.payload.release()
}

// Execute runs the deferred pre-trade request and releases the client order
// payload after callbacks complete.
//
// Execute does not close the underlying request; call Close after Execute just
// as with a standard pretrade.Request.
func (r *ClientRequest) Execute() (*pretrade.Reservation, []reject.Reject, error) {
	reservation, rejects, err := r.request.Execute()
	r.payload.release()
	return reservation, rejects, err
}

type clientPayloadHandle struct {
	handle cgo.Handle
	once   sync.Once
}

func newClientPayloadHandle(value any) *clientPayloadHandle {
	return &clientPayloadHandle{handle: cgo.NewHandle(value)}
}

func (h *clientPayloadHandle) release() {
	if h == nil {
		return
	}
	h.once.Do(func() { h.handle.Delete() })
}

type clientPayloadHandles []*clientPayloadHandle

func (handles clientPayloadHandles) release() {
	for _, payload := range handles {
		payload.release()
	}
}

func newClientOrderPayload[Order pretrade.ClientOrder](
	order Order,
) (model.Order, *clientPayloadHandle) {
	engineOrder := order.EngineOrder()
	nativeOrder := engineOrder.Handle()
	payload := newClientPayloadHandle(order)
	native.OrderSetUserData(&nativeOrder, callback.NewUserDataFromHandle(payload.handle))
	return model.NewOrderFromHandle(nativeOrder), payload
}

func newClientReportPayload[Report pretrade.ClientExecutionReport](
	report Report,
) (model.ExecutionReport, *clientPayloadHandle) {
	engineReport := report.EngineExecutionReport()
	nativeReport := engineReport.Handle()
	payload := newClientPayloadHandle(report)
	native.ExecutionReportSetUserData(&nativeReport, callback.NewUserDataFromHandle(payload.handle))
	return model.NewExecutionReportFromHandle(nativeReport), payload
}

func newClientAdjustmentPayloads[Adjustment accountadjustment.ClientAccountAdjustment](
	adjustments []Adjustment,
) ([]model.AccountAdjustment, clientPayloadHandles) {
	engineAdjustments := make([]model.AccountAdjustment, len(adjustments))
	payloads := make(clientPayloadHandles, len(adjustments))
	for i, adjustment := range adjustments {
		engineAdjustments[i], payloads[i] = newClientAdjustmentPayload(adjustment)
	}
	return engineAdjustments, payloads
}

func newClientAdjustmentPayload[Adjustment accountadjustment.ClientAccountAdjustment](
	adjustment Adjustment,
) (model.AccountAdjustment, *clientPayloadHandle) {
	engineAdjustment := adjustment.EngineAccountAdjustment()
	nativeAdjustment := engineAdjustment.Handle()
	payload := newClientPayloadHandle(adjustment)
	native.AccountAdjustmentSetUserData(
		&nativeAdjustment,
		callback.NewUserDataFromHandle(payload.handle),
	)
	return model.NewAccountAdjustmentFromHandle(nativeAdjustment), payload
}
