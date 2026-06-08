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
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/pretrade"
)

//------------------------------------------------------------------------------
// ClientEngineBuilder

// ClientEngineBuilder is the initial stage of the client engine builder.
// Call one of FullSync, NoSync, or AccountSync to obtain a
// ClientSyncedEngineBuilder on which policies can be registered.
type ClientEngineBuilder[
	Order pretrade.ClientOrder,
	Report pretrade.ClientExecutionReport,
	Adjustment clientAccountAdjustment,
] struct {
	unsafeFastPayloadCallbacks bool
}

// NewClientEngineBuilder creates a builder for strategies that use custom
// order, execution report, and account-adjustment types.
//
// Call FullSync, NoSync, or AccountSync to select a sync policy
// and obtain a ClientSyncedEngineBuilder.
func NewClientEngineBuilder[
	Order pretrade.ClientOrder,
	Report pretrade.ClientExecutionReport,
	Adjustment clientAccountAdjustment,
](
	options ...ClientEngineOption,
) *ClientEngineBuilder[Order, Report, Adjustment] {
	config := clientEngineOptions{}
	for _, option := range options {
		option(&config)
	}
	return &ClientEngineBuilder[Order, Report, Adjustment]{
		unsafeFastPayloadCallbacks: config.unsafeFastPayloadCallbacks,
	}
}

// NewClientPreTradeEngineBuilder creates a client builder for custom order
// and execution report types while keeping account adjustments on the standard
// SDK model type.
func NewClientPreTradeEngineBuilder[
	Order pretrade.ClientOrder,
	Report pretrade.ClientExecutionReport,
](
	options ...ClientEngineOption,
) *ClientEngineBuilder[Order, Report, model.AccountAdjustment] {
	return NewClientEngineBuilder[
		Order, Report, model.AccountAdjustment,
	](options...)
}

// NewClientAccountAdjustmentEngineBuilder creates a client builder for custom
// account-adjustment types while keeping orders and execution reports on the
// standard SDK model types.
func NewClientAccountAdjustmentEngineBuilder[
	Adjustment clientAccountAdjustment,
](
	options ...ClientEngineOption,
) *ClientEngineBuilder[model.Order, model.ExecutionReport, Adjustment] {
	return NewClientEngineBuilder[
		model.Order, model.ExecutionReport, Adjustment,
	](options...)
}

// FullSync configures full thread-safety synchronization and returns a
// ClientSyncedEngineBuilder ready to accept policies.
func (
	b *ClientEngineBuilder[Order, Report, Adjustment],
) FullSync() *ClientSyncedEngineBuilder[Order, Report, Adjustment] {
	return &ClientSyncedEngineBuilder[Order, Report, Adjustment]{
		synced:                     NewEngineBuilder().FullSync(),
		unsafeFastPayloadCallbacks: b.unsafeFastPayloadCallbacks,
	}
}

// NoSync configures single-thread (no-sync) synchronization and returns
// a ClientSyncedEngineBuilder ready to accept policies.
func (
	b *ClientEngineBuilder[Order, Report, Adjustment],
) NoSync() *ClientSyncedEngineBuilder[Order, Report, Adjustment] {
	return &ClientSyncedEngineBuilder[Order, Report, Adjustment]{
		synced:                     NewEngineBuilder().NoSync(),
		unsafeFastPayloadCallbacks: b.unsafeFastPayloadCallbacks,
	}
}

// AccountSync configures account-sharded synchronization and returns a
// ClientSyncedEngineBuilder ready to accept policies. The resulting engine
// handle is safe for concurrent invocation when the caller pins each account
// to a single processing chain.
//
// To run an AccountSync ClientEngine concurrently, write your own
// per-account dispatch or build a thin wrapper around asyncengine.AsyncEngine
// pinned to the underlying *Engine. The bundled asyncengine package
// currently wraps the standard *Engine only; ClientEngine async wrapping
// is not part of the SDK and is left to the caller.
func (
	b *ClientEngineBuilder[Order, Report, Adjustment],
) AccountSync() *ClientSyncedEngineBuilder[Order, Report, Adjustment] {
	acc := NewEngineBuilder().AccountSync()
	return &ClientSyncedEngineBuilder[Order, Report, Adjustment]{
		synced:                     &acc.SyncedEngineBuilder,
		unsafeFastPayloadCallbacks: b.unsafeFastPayloadCallbacks,
	}
}

//------------------------------------------------------------------------------
// ClientSyncedEngineBuilder

// ClientSyncedEngineBuilder is the second stage of the client engine builder
// chain. Add at least one policy to advance to ClientReadyEngineBuilder where
// Build is available.
type ClientSyncedEngineBuilder[
	Order pretrade.ClientOrder,
	Report pretrade.ClientExecutionReport,
	Adjustment clientAccountAdjustment,
] struct {
	synced                     *SyncedEngineBuilder
	unsafeFastPayloadCallbacks bool
}

func (
	b *ClientSyncedEngineBuilder[Order, Report, Adjustment],
) newReady() *ClientReadyEngineBuilder[Order, Report, Adjustment] {
	return &ClientReadyEngineBuilder[Order, Report, Adjustment]{
		ready:                      newReadyEngineBuilder(b.synced),
		unsafeFastPayloadCallbacks: b.unsafeFastPayloadCallbacks,
	}
}

// PreTrade registers client pre-trade policies and advances the builder to
// ClientReadyEngineBuilder.
func (b *ClientSyncedEngineBuilder[Order, Report, Adjustment]) PreTrade(
	policy ...pretrade.ClientPreTradePolicy[Order, Report],
) *ClientReadyEngineBuilder[Order, Report, Adjustment] {
	rb := b.newReady()
	for _, p := range policy {
		rb.addPreTradePolicy(p)
	}
	return rb
}

// Builtin registers a built-in entity on the builder.
func (
	b *ClientSyncedEngineBuilder[Order, Report, Adjustment],
) Builtin(
	builtinReadyBuilder builtinReadyBuilder,
) *ClientReadyEngineBuilder[Order, Report, Adjustment] {
	return b.newReady().Builtin(builtinReadyBuilder)
}

//------------------------------------------------------------------------------
// ClientReadyEngineBuilder

// ClientReadyEngineBuilder is the third stage of the client engine builder
// chain. Accepts additional policies and builds the engine via Build.
type ClientReadyEngineBuilder[
	Order pretrade.ClientOrder,
	Report pretrade.ClientExecutionReport,
	Adjustment clientAccountAdjustment,
] struct {
	ready                      *ReadyEngineBuilder
	unsafeFastPayloadCallbacks bool
}

// Close releases the underlying builder and any policies it still owns.
func (b *ClientReadyEngineBuilder[Order, Report, Adjustment]) Close() {
	b.ready.Close()
}

// Build constructs a ClientEngine and transfers ownership of policies to it.
func (b *ClientReadyEngineBuilder[Order, Report, Adjustment]) Build() (
	*ClientEngine[Order, Report, Adjustment],
	error,
) {
	engine, err := b.ready.Build()
	if err != nil {
		return nil, err
	}
	return &ClientEngine[Order, Report, Adjustment]{engine: engine}, nil
}

// PreTrade appends additional client pre-trade policies to an already-ready builder.
func (b *ClientReadyEngineBuilder[Order, Report, Adjustment]) PreTrade(
	policy ...pretrade.ClientPreTradePolicy[Order, Report],
) *ClientReadyEngineBuilder[Order, Report, Adjustment] {
	for _, p := range policy {
		b.addPreTradePolicy(p)
	}
	return b
}

// Builtin registers a built-in entity on the builder.
func (
	b *ClientReadyEngineBuilder[Order, Report, Adjustment],
) Builtin(
	builtinReadyBuilder builtinReadyBuilder,
) *ClientReadyEngineBuilder[Order, Report, Adjustment] {
	b.ready.Builtin(builtinReadyBuilder)
	return b
}

func (
	b *ClientReadyEngineBuilder[Order, Report, Adjustment],
) addPreTradePolicy(
	p pretrade.ClientPreTradePolicy[Order, Report],
) {
	if b.unsafeFastPayloadCallbacks {
		b.ready.PreTrade(pretrade.NewUnsafeFastClientPreTradePolicy(p))
		return
	}
	b.ready.PreTrade(pretrade.NewSafeClientPreTradePolicy(p))
}
